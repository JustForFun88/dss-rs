use super::*;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::control::control_elem::{ControlAction, RefSnapshot};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::exec::Dss;
use crate::obj::base::DssObject;
use crate::solution::control_queue::TimeRec;
use crate::solution::{ControlMode, ControlQueue, EventLog, LoadSolutionModel, SolveMode};

/// The ganged operation slot (`IdxMultiPh = NPhases+1`, frozen at 4 for the
/// `Create`-time 3-phase relay).
const G: usize = 4;

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: LoadSolutionModel::PowerFlow,
        mode: SolveMode::Snapshot,
        active_load_shape_class: crate::solution::USENONE,
        load_multiplier: 1.0,
        gen_multiplier: 1.0,
        generator_dispatch_reference: 0.0,
        price_signal: 25.0,
        default_growth_factor: 1.0,
        year: 0,
        dbl_hour: 0.0,
        solution_count: 0,
        loads_need_updating: false,
        neglect_load_y: false,
        long_line_correction: false,
        positive_sequence: false,
        time_of_day: 0.0,
        dyna_h: 0.0,
        dyna_t: 0.0,
        iteration_flag: crate::support::dynamics::IterationFlag::NewTimeStep,
        last_solution_was_direct: false,
    }
}

/// A 1-terminal mock carrying explicit per-phase currents and voltages plus a
/// fixed terminal power.
struct MockElem {
    cd: CktElementData,
    iph: Vec<Complex64>,
    vph: Vec<Complex64>,
    power: Complex64,
    vars: Vec<(String, f64)>,
}
impl MockElem {
    fn new(nphases: usize) -> Self {
        let mut cd = CktElementData::new("ln", 1);
        cd.nphases = nphases;
        cd.nconds = nphases;
        cd.set_nterms(1);
        cd.yorder = nphases;
        Self {
            cd,
            iph: vec![Complex64::ZERO; nphases],
            vph: vec![Complex64::ZERO; nphases],
            power: Complex64::ZERO,
            vars: Vec::new(),
        }
    }
    fn with_current(mut self, mag: f64) -> Self {
        for c in self.iph.iter_mut() {
            *c = Complex64::new(mag, 0.0);
        }
        self
    }
    fn with_var(mut self, name: &str, value: f64) -> Self {
        self.vars.push((name.to_string(), value));
        self
    }
}
impl CktElement for MockElem {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }
    fn calc_yprim(&mut self, _sys: &SysCtx) {}
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
        for (c, i) in curr.iter_mut().zip(self.iph.iter()) {
            *c = *i;
        }
    }
    fn get_term_voltages(&self, _iterm: usize, _node_v: &[Complex64], vbuffer: &mut [Complex64]) {
        for (v, s) in vbuffer.iter_mut().zip(self.vph.iter()) {
            *v = *s;
        }
    }
    fn terminal_power(&mut self, _sys: &SysCtx, _node_v: &[Complex64], _idx: usize) -> Complex64 {
        self.power
    }
    fn num_variables(&self) -> usize {
        self.vars.len()
    }
    fn variable_name(&self, i: usize) -> String {
        self.vars
            .get(i.wrapping_sub(1))
            .map(|(n, _)| n.clone())
            .unwrap_or_default()
    }
    fn get_all_variables(&mut self, _sys: &SysCtx, _node_v: &[Complex64], states: &mut [f64]) {
        for (s, (_, v)) in states.iter_mut().zip(self.vars.iter()) {
            *s = *v;
        }
    }
}

struct Scratch {
    queue: ControlQueue,
    events: EventLog,
    errors: crate::diag::ErrorLog,
    y_changed: bool,
    sys: SysCtx,
}
impl Scratch {
    fn new() -> Self {
        Self {
            queue: ControlQueue::new(),
            events: EventLog::new(),
            errors: crate::diag::ErrorLog::new(),
            y_changed: false,
            sys: test_sys(),
        }
    }
    fn ctx(&mut self, int_hour: i32, t: f64) -> CtrlCtx<'_> {
        CtrlCtx {
            node_v: &[],
            sys: &self.sys,
            queue: &mut self.queue,
            events: &mut self.events,
            errors: &mut self.errors,
            system_y_changed: &mut self.y_changed,
            control_mode: ControlMode::TimeDriven,
            int_hour,
            t,
            dbl_hour: int_hour as f64 + t / 3600.0,
            control_iter: 1,
            self_ref: ElemId::new(0, 0),
        }
    }
}

fn build_tcc(npts: &str, c: &str, t: &str) -> TccCurveObj {
    use crate::elements::general::tcc_curve::class_props;
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::PropEngine;
    use dss_parser::{Parser, ParserVars};
    let cls = class_props(&EnumRegistry::new());
    let mut obj = TccCurveObj::new("fc");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let enums = EnumRegistry::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (n, v) in [("npts", npts), ("C_array", c), ("T_array", t)] {
        let idx = cls.property_index(n).unwrap();
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(&mut obj, idx, v, &mut eng).unwrap();
    }
    assert!(errors.is_empty(), "curve build errors: {errors:?}");
    obj
}

/// A 3-phase overcurrent relay with a simple `c=[1,10] t=[1,0.1]` phase curve,
/// `PhPickup=1`, controlled element a 3-phase line, event log on.
fn armed_relay() -> Relay {
    let mut r = Relay::new("r1");
    r.control_type = RelayControlType::Current;
    r.phase_curve = Some(build_tcc("2", "1 10", "1 0.1"));
    r.phase_trip = 1.0;
    r.ccd.show_event_log = true;
    r.ctrl_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    r
}

fn log_has(sc: &Scratch, needle: &str) -> bool {
    sc.events.entries().iter().any(|e| e.contains(needle))
}

/// Count non-`Debug Sample` event-log lines — the trace lines are noise for the
/// protection assertions, and the filter keeps them so whether or not the relay
/// under test has `DebugTrace` on.
fn non_debug_lines(sc: &Scratch) -> usize {
    sc.events
        .entries()
        .iter()
        .filter(|e| !e.contains("DEBUG SAMPLE"))
        .count()
}

#[test]
fn default_is_3ph_closed_relay() {
    let r = Relay::new("r1");
    assert_eq!(r.ccd.cd.nphases, 3);
    assert_eq!(r.ccd.cd.nconds, 3);
    assert_eq!(r.ccd.cd.nterms, 1);
    assert_eq!(r.control_type, RelayControlType::Current);
    assert_eq!(r.phase_trip, 1.0);
    assert_eq!(r.num_reclose, 3); // Shots default 4
    assert_eq!(r.reset_time, 15.0);
    assert_eq!(r.reclose_intervals[..3], [0.5, 2.0, 2.0]);
    assert_eq!(r.present_state[1], ControlAction::Close);
    assert_eq!(r.normal_state[1], ControlAction::Close);
    assert!(!r.normal_state_set);
    assert_eq!(r.idx_multi_ph, G);
    assert_eq!(r.pickup_amps46, 20.0);
    assert_eq!(r.doc_trip_set_high, -1.0);
    assert!(r.doc_p1_blocking);
    assert!(!r.single_ph_trip);
    assert!(r.ccd.cd.yprim.is_none());
}

// EXPECTED-VALUE-PIN(RELAY_SAMPLE_TRACE_IGNORES_DEBUGTRACE): the per-`Sample`
// state trace is a debug trace, so it is written exactly when `DebugTrace` says
// so — in both lanes, since `GOLDEN_REBASE_PLAN.md` G2.2d tore the row down.
/// Expected-value pin: the `Debug Sample` line of `TRelayObj.Sample` follows
/// `DebugTrace`, the guard its Recloser twin (`Recloser.pas:1044`) and its own
/// siblings (`Relay.pas:1822`, `:1845`) carry and r4133 dropped from this one
/// line only (`Relay.pas:1325`).
///
/// Asserted in all four combinations, so the fix cannot degrade into "the line
/// is gone": with `DebugTrace=yes` it **must** be written — that is the whole
/// content of a trace flag — and `ShowEventLog` must not gate it either way,
/// which is what distinguishes this line from every protection event around it.
#[test]
fn sample_state_trace_follows_debugtrace() {
    let trace = "Element=Debug Sample: Relay.r1,";
    for debug_trace in [false, true] {
        for show_event_log in [false, true] {
            let mut r = armed_relay();
            r.debug_trace = debug_trace;
            r.ccd.show_event_log = show_event_log;
            let mut ctrl = MockElem::new(3);
            let mut mon = MockElem::new(3).with_current(0.1); // below pickup: no event
            let mut sc = Scratch::new();
            r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
            assert_eq!(
                log_has(&sc, trace),
                debug_trace,
                "debug_trace={debug_trace} show_event_log={show_event_log}: log = {:?}",
                sc.events.entries()
            );
        }
    }
}

// --- Overcurrent (Type=Current) ---------------------------------------------

#[test]
fn overcurrent_arms_open_then_reclose() {
    let mut r = armed_relay();
    let mut ctrl = MockElem::new(3); // closed
    let mut mon = MockElem::new(3).with_current(10.0); // ratio 10 → trip 0.1 s
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert!(r.armed_for_close[G]);
    assert!(r.phase_target[G]);
    assert_eq!(r.relay_target[G], "Ph Curve"); // E2 descriptive target
    assert_eq!(sc.queue.queue_size(), 2); // OPEN + reclose CLOSE
}

/// Pin the queued OPEN / reclose times: `TripTime = TDPh·GetTCCTime(10) = 2·0.1 =
/// 0.2`, OPEN at `0.2 + MechanicalDelay(0.05) = 0.25`, reclose at `+
/// RecloseIntervals[0] = +0.5 = 0.75`.
#[test]
fn overcurrent_queues_trip_and_reclose_at_correct_times() {
    let mut r = armed_relay();
    r.mechanical_delay = 0.05;
    r.td_phase = 2.0;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3).with_current(10.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    let far = TimeRec {
        hour: 999,
        sec: 0.0,
    };
    let (open, t_open) = sc.queue.pop_time(far, false).unwrap();
    assert_eq!(open.code, ControlAction::Open.ordinal());
    assert!((t_open - 0.25).abs() < 1e-9, "open time = {t_open}");
    let (close, t_close) = sc.queue.pop_time(far, false).unwrap();
    assert_eq!(close.code, ControlAction::Close.ordinal());
    assert!((t_close - 0.75).abs() < 1e-9, "reclose time = {t_close}");
}

#[test]
fn overcurrent_ground_trip_on_residual_sum() {
    let mut r = armed_relay();
    r.phase_curve = None; // isolate the ground path
    r.ground_curve = Some(build_tcc("2", "1 10", "1 0.1"));
    r.ground_trip = 1.0;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3).with_current(10.0); // residual 30 A
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert!(r.ground_target);
    assert!(!r.phase_target[G]);
    assert_eq!(r.relay_target[G], "Gnd Curve"); // E2 descriptive target
}

/// **D3:** the instantaneous trip time is a bare `0.01` (the `MechanicalDelay` is
/// added once, at the push) — `0.01 + MechanicalDelay`, not r4088's `0.01 +
/// 2·MechanicalDelay`.
#[test]
fn overcurrent_inst_single_count_d3() {
    let mut r = armed_relay();
    r.phase_curve = Some(build_tcc("2", "100 200", "1 0.1")); // never picks up at 10 A
    r.phase_inst = 5.0;
    r.mechanical_delay = 0.05;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3).with_current(10.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G], "inst trips on the first operation");
    assert_eq!(r.relay_target[G], "Ph Instantaneous");
    let far = TimeRec {
        hour: 999,
        sec: 0.0,
    };
    let (open, t_open) = sc.queue.pop_time(far, false).unwrap();
    assert_eq!(open.code, ControlAction::Open.ordinal());
    // D3: 0.01 + MechanicalDelay(0.05) = 0.06 (NOT 0.01 + 2·0.05 = 0.11).
    assert!((t_open - 0.06).abs() < 1e-9, "inst open time = {t_open}");
}

#[test]
fn overcurrent_phase_inst_first_operation_only() {
    let make = || {
        let mut r = armed_relay();
        r.phase_curve = Some(build_tcc("2", "100 200", "1 0.1"));
        r.phase_inst = 5.0;
        r
    };
    {
        let mut r = make();
        let mut ctrl = MockElem::new(3);
        let mut mon = MockElem::new(3).with_current(10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(r.armed_for_open[G], "inst trips on the first operation");
    }
    {
        let mut r = make();
        r.operation_count[G] = 2; // inst only fires when MaxOperatingCount == 1
        let mut ctrl = MockElem::new(3);
        let mut mon = MockElem::new(3).with_current(10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(!r.armed_for_open[G], "inst is first-operation only");
    }
}

#[test]
fn overcurrent_skips_when_terminal_open() {
    let mut r = armed_relay();
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // controlled terminal open
    let mut mon = MockElem::new(3).with_current(10.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert_eq!(r.present_state[1], ControlAction::Open);
    assert!(!r.armed_for_open[G]);
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn overcurrent_disarms_and_resets_when_current_drops() {
    let mut r = armed_relay();
    let mut ctrl = MockElem::new(3);
    let mut sc = Scratch::new();
    {
        let mut mon = MockElem::new(3).with_current(10.0);
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    assert_eq!(sc.queue.queue_size(), 2);
    let mut mon = MockElem::new(3); // zero current
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 1.0));
    assert!(!r.armed_for_open[G]);
    assert!(!r.armed_for_close[G]);
    assert!(!r.phase_target[G]);
    assert_eq!(sc.queue.queue_size(), 3); // the prior two stay + a RESET
}

// --- Single-phase trip (B1 / SinglePhTrip) ----------------------------------

/// With `SinglePhTrip`, only the over-current phase(s) arm; the phase index rides
/// the control-queue proxy handle.
#[test]
fn single_phase_trip_arms_only_faulted_phase() {
    let mut r = armed_relay();
    r.single_ph_trip = true;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.iph = vec![Complex64::new(10.0, 0.0), Complex64::ZERO, Complex64::ZERO]; // phase 1 only
    r.ground_trip = 0.0; // isolate the phase path (no residual ground trip)
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[1], "faulted phase 1 arms");
    assert!(!r.armed_for_open[2], "phase 2 stays disarmed");
    assert!(!r.armed_for_open[3], "phase 3 stays disarmed");
    // OPEN carries the phase index (proxy 1) + a reclose CLOSE.
    assert_eq!(sc.queue.queue_size(), 2);
}

/// `DoPendingAction(CTRL_OPEN, proxy=1)` opens phase 1 only (single-phase trip).
#[test]
fn single_phase_do_open_opens_only_that_phase() {
    let mut r = armed_relay();
    r.single_ph_trip = true;
    r.armed_for_open[1] = true;
    r.relay_target[1] = "Ph Curve".into();
    let mut ctrl = MockElem::new(3);
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Open.ordinal(),
        1,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert!(!ctrl.cd.conductor_closed(1, 1), "phase 1 opened");
    assert!(ctrl.cd.conductor_closed(1, 2), "phase 2 stays closed");
    assert!(ctrl.cd.conductor_closed(1, 3), "phase 3 stays closed");
    assert!(log_has(&sc, "PHASE 1 OPENED ON PH CURVE (1PH TRIP)"));
}

/// `SinglePhLockout=No` escalates a last-shot single-phase trip to a 3-phase
/// lockout: the faulted phase locks out AND every other phase opens+locks out.
#[test]
fn single_phase_lockout_escalates_to_3ph() {
    let mut r = armed_relay();
    r.single_ph_trip = true;
    r.single_ph_lockout = false; // 3-phase lockout escalation
    r.operation_count[1] = 4; // > NumReclose 3 ⇒ lockout
    r.armed_for_open[1] = true;
    r.relay_target[1] = "Ph Curve".into();
    let mut ctrl = MockElem::new(3);
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Open.ordinal(),
        1,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert!(r.locked_out[1]);
    assert!(r.locked_out[2], "other phases lock out on 3ph escalation");
    assert!(r.locked_out[3]);
    assert!(!ctrl.cd.conductor_closed(1, 2));
    assert!(log_has(&sc, "LOCKED OUT (3PH LOCKOUT)"));
}

// --- DoPendingAction --------------------------------------------------------

#[test]
fn do_pending_open_trips_logs_target() {
    let mut r = armed_relay();
    let mut ctrl = MockElem::new(3);
    let mut sc = Scratch::new();
    {
        let mut mon = MockElem::new(3).with_current(10.0);
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    r.do_pending_action(
        ControlAction::Open.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.1),
    );
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // opened
    assert!(!r.armed_for_open[G]);
    assert!(sc.y_changed);
    assert!(
        log_has(&sc, "OPENED ON PH CURVE (3PH TRIP)"),
        "log = {:?}",
        sc.events.entries()
    );
    assert!(!r.locked_out[G]); // operation_count 1 ≤ NumReclose 3
}

#[test]
fn do_pending_open_locks_out_after_last_shot() {
    let mut r = armed_relay();
    r.operation_count[G] = 4; // > NumReclose 3
    r.armed_for_open[G] = true;
    r.relay_target[G] = "Ph Curve".into();
    let mut ctrl = MockElem::new(3);
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Open.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert!(!ctrl.cd.terminal_all_phases_closed(1));
    assert!(r.locked_out[G]);
    assert!(log_has(&sc, "LOCKED OUT (3PH LOCKOUT)"));
}

#[test]
fn do_pending_open_gated_on_show_event_log() {
    let mut r = armed_relay();
    r.ccd.show_event_log = false;
    r.armed_for_open[G] = true;
    r.relay_target[G] = "Ph Curve".into();
    let mut ctrl = MockElem::new(3);
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Open.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // still opened
    assert!(sc.y_changed);
    assert_eq!(sc.events.entries().len(), 0); // gated (no Debug Sample from do_pending)
}

#[test]
fn do_pending_close_recloses_and_counts() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.present_state[i] = ControlAction::Open;
    }
    r.armed_for_close[G] = true;
    r.operation_count[G] = 1;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // currently open
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Close.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // reclosed
    assert_eq!(r.operation_count[G], 2);
    assert!(!r.armed_for_close[G]);
    assert!(sc.y_changed);
    assert!(log_has(&sc, "CLOSED (3PH RECLOSING)"));
}

#[test]
fn do_pending_open_close_are_wrong_state_no_ops() {
    // OPEN when already open.
    {
        let mut r = armed_relay();
        for i in 1..=3 {
            r.present_state[i] = ControlAction::Open;
        }
        r.armed_for_open[G] = true;
        let mut ctrl = MockElem::new(3);
        ctrl.cd.set_terminal_closed(1, false);
        let mut sc = Scratch::new();
        r.do_pending_action(
            ControlAction::Open.ordinal(),
            0,
            &mut ctrl,
            &mut sc.ctx(0, 0.0),
        );
        assert!(!ctrl.cd.terminal_all_phases_closed(1));
        assert!(!sc.y_changed);
        assert_eq!(non_debug_lines(&sc), 0);
    }
    // CLOSE when already closed.
    {
        let mut r = armed_relay();
        r.armed_for_close[G] = true;
        let mut ctrl = MockElem::new(3);
        let mut sc = Scratch::new();
        r.do_pending_action(
            ControlAction::Close.ordinal(),
            0,
            &mut ctrl,
            &mut sc.ctx(0, 0.0),
        );
        assert!(ctrl.cd.terminal_all_phases_closed(1));
        assert!(!sc.y_changed);
        assert_eq!(non_debug_lines(&sc), 0);
    }
}

// EXPECTED-VALUE-PIN(RELAY_RESET_EVENT_IS_LABELLED_RECLOSER): the reset event
// names the class that emitted it, in both lanes, since `GOLDEN_REBASE_PLAN.md`
// G2.2d tore the row down.
/// **D4:** the queued `DoPendingAction(CTRL_RESET)` no longer runs the full
/// `Reset` — it only resets `OperationCount` to 1 for closed phases, does NOT
/// force the element back to normal state, and logs the event as
/// `Relay.<name>`.
#[test]
fn do_pending_reset_only_resets_opcount_d4() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.present_state[i] = ControlAction::Close; // closed phase
    }
    r.armed_for_open[G] = false;
    r.operation_count[G] = 3;
    let mut ctrl = MockElem::new(3); // closed
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Reset.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert_eq!(r.operation_count[G], 1, "opcount reset to 1");
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // element NOT forced (still closed)
    assert!(!sc.y_changed, "D4 reset does not force the element / Y");
    // The label half of the pin. Upstream copy-pasted `'Recloser.' + Self.Name`
    // out of `Recloser.pas:909`/`:924` into both `CTRL_RESET` arms
    // (`Relay.pas:1196`, `:1212`); the event belongs to the relay that emitted
    // it, and both r4088 (`:971`) and 0.14.5 (`:1003`) label it that way. The
    // element name keeps its case; only the Action is uppercased. Asserted in
    // both directions, so the donor's class name cannot creep back in.
    assert!(
        log_has(&sc, "Element=Relay.r1,"),
        "log = {:?}",
        sc.events.entries()
    );
    assert!(
        !log_has(&sc, "Element=Recloser.r1,"),
        "log = {:?}",
        sc.events.entries()
    );
    assert!(log_has(&sc, "PHASE ALL RESET (3PH RESET)"));
}

#[test]
fn do_pending_reset_skipped_when_all_open() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.present_state[i] = ControlAction::Open; // no closed phase ⇒ no reset
    }
    r.operation_count[G] = 3;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false);
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Reset.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert_eq!(
        r.operation_count[G], 3,
        "opcount untouched (no closed phase)"
    );
    assert_eq!(non_debug_lines(&sc), 0);
}

// --- Reset ------------------------------------------------------------------

#[test]
fn reset_with_restores_closed_normal_state_and_logs() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.normal_state[i] = ControlAction::Close;
        r.present_state[i] = ControlAction::Open;
    }
    r.operation_count[G] = 4;
    r.locked_out[G] = true;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // start open
    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // restored closed
    assert_eq!(r.present_state[1], ControlAction::Close);
    assert!(!r.locked_out[1]);
    assert_eq!(r.operation_count[1], 1);
    assert!(sc.y_changed);
    assert!(log_has(&sc, "RESETTING"));
}

#[test]
fn reset_with_open_normal_state_locks_out() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.normal_state[i] = ControlAction::Open;
    }
    let mut ctrl = MockElem::new(3); // start closed
    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // forced open
    assert_eq!(r.present_state[1], ControlAction::Open);
    assert!(r.locked_out[1]);
    assert_eq!(r.operation_count[1], r.num_reclose + 1);
    assert!(sc.y_changed);
}

/// A `Locked` relay does NOT reset (r4133 D4/lock semantics).
#[test]
fn reset_with_blocked_while_locked() {
    let mut r = armed_relay();
    r.f_locked = true;
    for i in 1..=3 {
        r.normal_state[i] = ControlAction::Close;
        r.present_state[i] = ControlAction::Open;
    }
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false);
    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // untouched — still open
    assert!(!sc.y_changed);
    assert_eq!(non_debug_lines(&sc), 0);
}

/// Fail-on-regression guard for the WP7.2 step-2a Reset dirty edge: `reset_with`
/// must raise `SystemYChanged` and flip each phase per-phase.
#[test]
fn reset_with_partial_open_terminal_still_forces_rebuild() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.normal_state[i] = ControlAction::Open;
    }
    let mut ctrl = MockElem::new(3);
    ctrl.cd.terminals[0].conductors_closed[0] = true; // phase 0 closed
    ctrl.cd.terminals[0].conductors_closed[1] = false;
    ctrl.cd.terminals[0].conductors_closed[2] = false;
    assert!(!ctrl.cd.terminal_all_phases_closed(1));

    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(sc.y_changed);
    assert!(!ctrl.cd.conductor_closed(1, 1)); // phase 0: closed → open
}

// --- One-shot sub-types (RevPower / 46 / 47) --------------------------------

#[test]
fn rev_power_trips_on_reverse_locks_out() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::RevPower;
    r.phase_inst = 1.0;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.power = Complex64::new(-5000.0, 0.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert_eq!(r.operation_count[G], r.num_reclose + 1);
    assert_eq!(r.relay_target[G], "Rev P");
}

#[test]
fn rev_power_forward_no_trip() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::RevPower;
    r.phase_inst = 1.0;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.power = Complex64::new(5000.0, 0.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open[G]);
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn neg_seq46_trips_on_unbalanced_current() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::NegCurrent;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.iph = vec![Complex64::new(100.0, 0.0), Complex64::ZERO, Complex64::ZERO];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert_eq!(r.operation_count[G], r.num_reclose + 1);
    assert_eq!(r.relay_target[G], "-Seq Curr");
}

#[test]
fn neg_seq47_trips_on_unbalanced_voltage() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::NegVoltage;
    r.pickup_volts47 = 100.0;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![
        Complex64::new(1000.0, 0.0),
        Complex64::ZERO,
        Complex64::ZERO,
    ];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert_eq!(r.operation_count[G], r.num_reclose + 1);
    assert_eq!(r.relay_target[G], "-Seq V");
}

// --- Voltage (27/59) --------------------------------------------------------

#[test]
fn voltage_over_voltage_trips() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::Voltage;
    r.vbase = 1000.0;
    r.ov_curve = Some(build_tcc("2", "1.1 1.5", "5 0.1"));
    r.uv_curve = Some(build_tcc("2", "0.5 0.9", "0.1 5"));
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(1200.0, 0.0); 3]; // 1.2 pu ⇒ OV
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert_eq!(r.relay_target[G], "OV");
    assert_eq!(sc.queue.queue_size(), 1);
}

#[test]
fn voltage_under_voltage_trips() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::Voltage;
    r.vbase = 1000.0;
    r.ov_curve = Some(build_tcc("2", "1.1 1.5", "5 0.1"));
    r.uv_curve = Some(build_tcc("2", "0.5 0.9", "0.1 5"));
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(700.0, 0.0); 3]; // 0.7 pu ⇒ UV
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert_eq!(r.relay_target[G], "UV");
}

/// **B2:** the OV trip uses `Vmax_closed` (extrema over *closed* phases only). A
/// relay with one phase open ignores an over-voltage on the open phase.
#[test]
fn voltage_ov_uses_closed_phase_extrema_b2() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::Voltage;
    r.vbase = 1000.0;
    r.ov_curve = Some(build_tcc("2", "1.1 1.5", "5 0.1"));
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_conductor_closed(1, 1, false); // phase 1 open
    let mut mon = MockElem::new(3);
    // Phase 1 (open) at 1.4 pu OV; the closed phases at 1.0 pu ⇒ no closed-phase OV.
    mon.vph = vec![
        Complex64::new(1400.0, 0.0),
        Complex64::new(1000.0, 0.0),
        Complex64::new(1000.0, 0.0),
    ];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(
        !r.armed_for_open[G],
        "OV on an OPEN phase must not trip (closed-phase-only extrema)"
    );
}

/// **59NRelayDemo regression (WP-U2.6):** a `type=voltage` relay whose MONITORED
/// element is a 1-phase broken-delta PT (measuring 3V0 across an open point to
/// ground) but whose SWITCHED element is a 3-phase line. Pascal
/// `RecalcElementData` forces `Nphases := MonitoredElement.NPhases` (=1, used only
/// for `vbase`/`cBuffer`/`CondOffset`), while every per-phase state-array loop —
/// `VoltageLogic` (Relay.pas:2852), `GetPropertyValue` 39/40, `Sample`, `Reset` —
/// iterates `Min(RELAYCONTROLMAXDIM, ControlledElement.NPhases)` (=3). With the
/// wrong count=1 the loop reads only `cBuffer[1]` (the high 3V0) and trips on the
/// ground overvoltage; with the correct count=3 the loop's final *phantom* phase
/// (beyond the 1-phase PT terminal's conductors) reads 0, so `Vmag` ends 0, the
/// `IF Vmag > 0` guard fails, `OVTime` stays -1 and the relay correctly does NOT
/// trip — matching oddie:r4133. (For the usual mon==ctrl-phase relay the two
/// counts coincide, so this is the only path that distinguishes them.)
///
/// The no-trip outcome is NOT self-pinned: on the r4133 engine itself the deck's
/// own `Relay.State` property reads `[closed, closed, closed, ]` (byte-identical
/// to the `render_state_array()` asserted below) and Line.line1 still carries
/// ~1381 A. Reproduce via `tools/opendss/probe_59n.py`, which drives the official
/// r4133 DLL through the in-house `epri-worker` bridge (crates/dss-epri; the
/// original Oddie/dss-python probe was retired with that channel) — exits 0 iff
/// r4133 reads all-closed / no trip. Re-verified through the bridge 2026-07-19.
#[test]
fn voltage_relay_open_point_sizes_state_by_controlled_nphases_59n() {
    let mut r = armed_relay(); // ctrl_snap = 3-phase line
    r.control_type = RelayControlType::Voltage;
    r.vbase = 277.0; // kvbase 0.277 kV, 1-phase ⇒ line-neutral
    r.ov_curve = Some(build_tcc("1", ".3", ".1")); // 3V0: trip above 0.3 pu
    // Pascal RecalcElementData: relay's own Nphases := MonitoredElement.NPhases.
    r.ccd.cd.nphases = 1;
    assert_eq!(
        r.state_size(),
        3,
        "state array must be sized by ControlledElement.NPhases (3), not the \
         relay's own Nphases (= MonitoredElement.NPhases = 1)"
    );

    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(1); // 1-phase PT
    mon.cd.nconds = 2; // PT term 2 has 2 conductors (Delta.3, Delta.2)
    mon.vph = vec![Complex64::new(438.0, 0.0)]; // 3V0 ≈ 1.58 pu ≫ 0.3 pu pickup
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.3));

    assert!(
        !r.armed_for_open[G],
        "the phantom phase makes Vmag=0 ⇒ the `IF Vmag>0` guard fails ⇒ no OV trip \
         (with the buggy count=1 it would read 438 V and trip)"
    );
    assert_eq!(
        sc.queue.queue_size(),
        0,
        "no trip queued across the broken-delta open point"
    );
    assert_eq!(
        r.render_state_array(),
        "[closed, closed, closed, ]",
        "State renders ControlledElement.NPhases (=3) entries"
    );
}

/// **MakeLike per-phase state count (WP-U2.6 audit).** Pascal `TRelayObj.MakeLike`
/// (Relay.pas:683) copies `FPresentState`/`FNormalState` over
/// `Min(RELAYCONTROLMAXDIM, ControlledElement.Nphases)` — the SWITCHED element's
/// phase count, resolved before the copy — NOT the relay's own `FNPhases`
/// (= MonitoredElement.NPhases). For an asymmetric `like=` source (1-phase
/// monitored PT, 3-phase switched line) an OPEN state latched on a high phase must
/// survive the copy; the old `other.ccd.cd.nphases`-bounded loop dropped it.
#[test]
fn make_like_copies_state_by_controlled_nphases() {
    let mut src = armed_relay(); // ctrl_snap = 3-phase line
    src.ccd.cd.nphases = 1; // relay's own count = MonitoredElement.NPhases
    src.present_state[3] = ControlAction::Open;
    src.normal_state[3] = ControlAction::Open;

    let mut dst = Relay::new("r2");
    dst.make_like(&src);

    assert_eq!(
        dst.state_size(),
        3,
        "MakeLike sets ctrl_snap (3-phase) + own nphases (1); state sizes by ctrl"
    );
    assert_eq!(
        dst.present_state[3],
        ControlAction::Open,
        "phase-3 present_state must copy over ControlledElement.Nphases (=3), \
         not the relay's own Nphases (=1)"
    );
    assert_eq!(
        dst.normal_state[3],
        ControlAction::Open,
        "phase-3 normal_state must copy over ControlledElement.Nphases"
    );
}

#[test]
fn voltage_recloses_when_voltage_recovers() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::Voltage;
    r.vbase = 1000.0;
    r.operation_count[G] = 1;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // present_state OPEN
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(1000.0, 0.0); 3];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert_eq!(r.present_state[1], ControlAction::Open);
    assert!(r.armed_for_close[G]);
    assert_eq!(sc.queue.queue_size(), 1);
}

// --- DOC directional decision tree ------------------------------------------

#[test]
fn doc_phase_time_test_directional_split() {
    let mut r = Relay::new("r1");
    r.control_type = RelayControlType::Doc;
    r.doc_tilt_angle_low = 95.0;
    r.doc_trip_set_low = 3500.0;
    r.definite_time_delay = 0.0;
    let trip = Complex64::new(-4000.0, 0.0);
    assert_eq!(r.doc_phase_time_test(trip, trip.norm()), 0.0);
    let no = Complex64::new(4000.0, 0.0);
    assert_eq!(r.doc_phase_time_test(no, no.norm()), -1.0);
}

#[test]
fn doc_p1_blocking_blocks_on_forward_power() {
    let mut r = armed_relay();
    r.control_type = RelayControlType::Doc;
    r.doc_p1_blocking = true;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(1);
    mon.power = Complex64::new(5000.0, 0.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open[G]);
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn doc_reverse_power_trips_through_full_sample() {
    let mut r = Relay::new("r1");
    r.control_type = RelayControlType::Doc;
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    let v = |deg: f64| Complex64::from_polar(1000.0, f64::to_radians(deg));
    mon.vph = vec![v(0.0), v(-120.0), v(120.0)];
    mon.iph = vec![v(180.0), v(60.0), v(-60.0)];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(
        r.armed_for_open[G],
        "reverse power should trip the DOC element"
    );
    assert_eq!(r.relay_target[G], "DOC");
    assert!(sc.queue.queue_size() >= 1);
}

#[test]
fn doc_three_phase_forward_power_blocks() {
    let mut r = Relay::new("r1");
    r.control_type = RelayControlType::Doc;
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    let v = |deg: f64| Complex64::from_polar(1000.0, f64::to_radians(deg));
    mon.vph = vec![v(0.0), v(-120.0), v(120.0)];
    mon.iph = mon.vph.clone();
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open[G]);
    assert_eq!(sc.queue.queue_size(), 0);
}

// --- Distance (21) ----------------------------------------------------------

fn distance_relay() -> Relay {
    let mut r = Relay::new("r1");
    r.control_type = RelayControlType::Distance;
    r.dist_z1 = Complex64::new(1.0, 0.0);
    r.dist_z0 = Complex64::new(1.0, 0.0);
    r.dist_k0 = Complex64::ZERO;
    r.mground = 1.0;
    r.mphase = 1.0;
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    r
}

#[test]
fn distance_trips_when_loop_impedance_in_reach() {
    let mut r = distance_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(0.5, 0.0); 3];
    mon.iph = vec![Complex64::new(1.0, 0.0); 3];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert!(
        r.relay_target[G].starts_with("21 "),
        "target = {}",
        r.relay_target[G]
    );
    assert!(r.relay_target[G].contains("G1"));
}

#[test]
fn distance_no_trip_when_out_of_reach() {
    let mut r = distance_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(2.0, 0.0); 3];
    mon.iph = vec![Complex64::new(1.0, 0.0); 3];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open[G]);
}

#[test]
fn distance_reverse_negates_current() {
    let mut r = distance_relay();
    r.dist_reverse = true;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(0.5, 0.0); 3];
    mon.iph = vec![Complex64::new(1.0, 0.0); 3];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open[G]);
}

// --- Generic (PC state-variable relay) --------------------------------------

#[test]
fn lookup_variable_prefix_match() {
    let names = vec!["Frequency".to_string(), "Vd".to_string()];
    assert_eq!(Relay::lookup_variable(&names, "frequency"), 1);
    assert_eq!(Relay::lookup_variable(&names, "freq"), 1);
    assert_eq!(Relay::lookup_variable(&names, "VD"), 2);
    assert_eq!(Relay::lookup_variable(&names, "theta"), -1);
    assert_eq!(Relay::lookup_variable(&names, "frequencyX"), -1);
}

fn generic_relay() -> Relay {
    let mut r = Relay::new("g1");
    r.control_type = RelayControlType::Generic;
    r.monitor_var_index = 1;
    r.over_trip = 1.2;
    r.under_trip = 0.8;
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    r
}

#[test]
fn generic_trips_above_overtrip_and_locks_out() {
    let mut r = generic_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3).with_var("Frequency", 1.5);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert_eq!(r.relay_target[G], "Frequency");
    assert_eq!(r.operation_count[G], r.num_reclose + 1);
    assert_eq!(sc.queue.queue_size(), 1);
}

#[test]
fn generic_trips_below_undertrip() {
    let mut r = generic_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3).with_var("Frequency", 0.5);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
}

#[test]
fn generic_no_trip_within_band() {
    let mut r = generic_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3).with_var("Frequency", 1.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open[G]);
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn generic_recalc_resolves_and_errors_on_missing_var() {
    let mut r = Relay::new("g1");
    r.control_type = RelayControlType::Generic;
    r.monitor_variable = "vd".to_string();
    r.monitor_var_names = vec!["Frequency".into(), "Vd".into()];
    r.mon_snap = Some(RefSnapshot {
        full_name: "Generator.g".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    r.recalc();
    assert_eq!(r.monitor_var_index, 2);

    let mut r2 = Relay::new("g2");
    r2.control_type = RelayControlType::Generic;
    r2.monitor_variable = "nosuch".to_string();
    r2.monitor_var_names = vec!["Frequency".into()];
    r2.mon_snap = Some(RefSnapshot {
        full_name: "Generator.g".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r2.ccd.controlled_element = Some(ElemId::new(0, 0));
    r2.recalc();
    assert_eq!(r2.monitor_var_index, -1);
    let errs = r2.ccd.cd.obj.take_errors();
    assert!(errs.iter().any(|e| e.contains("386")), "got {errs:?}");
    assert!(!r2.ccd.cd.obj.take_abort());
}

#[test]
fn recalc_out_of_range_terminal_errors_384_and_requests_abort() {
    let mut r = Relay::new("r384");
    r.monitored_element_terminal = 5;
    r.mon_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    r.recalc();
    assert!(r.ccd.cd.obj.take_abort());
    let errs = r.ccd.cd.obj.take_errors();
    assert!(errs.iter().any(|e| e.contains("384")), "got {errs:?}");
}

#[test]
fn recalc_missing_switched_element_errors_387_and_requests_abort() {
    let mut r = Relay::new("r387");
    r.recalc();
    assert!(r.ccd.cd.obj.take_abort());
    let errs = r.ccd.cd.obj.take_errors();
    assert!(errs.iter().any(|e| e.contains("387")), "got {errs:?}");
}

// --- TD21 (differential time-distance, 21) ----------------------------------

fn td21_relay() -> Relay {
    let mut r = Relay::new("t1");
    r.control_type = RelayControlType::Td21;
    r.dist_z1 = Complex64::new(1.0, 0.0);
    r.dist_k0 = Complex64::ZERO;
    r.mground = 1.0;
    r.mphase = 1.0;
    r.phase_trip = 1.0;
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    r
}

#[test]
fn td21_allocates_ring_buffer_on_first_sample() {
    let mut r = td21_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3];
    mon.iph = vec![Complex64::new(0.1, 0.0); 3];
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.001;
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert_eq!(r.td21_pt, 17);
    assert_eq!(r.td21_stride, 6);
    assert_eq!(r.td21_h.len(), 17 * 6);
    assert_eq!(r.td21_quiet, 17);
}

#[test]
fn td21_coarse_step_requests_solution_abort() {
    let mut r = td21_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3];
    mon.iph = vec![Complex64::new(0.1, 0.0); 3];
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.02;
    let abort = r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(abort);
    assert!(
        sc.errors.iter().any(|e| e.contains("388")),
        "{:?}",
        sc.errors
    );
}

#[test]
fn td21_fine_step_does_not_request_abort() {
    let mut r = td21_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3];
    mon.iph = vec![Complex64::new(0.1, 0.0); 3];
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.001;
    let abort = r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!abort);
    assert!(sc.errors.is_empty(), "{:?}", sc.errors);
}

#[test]
fn td21_trips_on_forward_differential_fault() {
    let mut r = td21_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3];
    mon.iph = vec![Complex64::new(0.1, 0.0); 3];
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.001;
    for _ in 0..18 {
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    assert!(!r.armed_for_open[G]);
    assert_eq!(r.td21_quiet, 0);
    mon.vph = vec![Complex64::new(4.1, -2.95); 3];
    mon.iph = vec![Complex64::new(6.0, 0.0); 3];
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert!(
        r.relay_target[G].starts_with("TD21 "),
        "target = {}",
        r.relay_target[G]
    );
    assert!(r.relay_target[G].contains("G1"));
}

#[test]
fn td21_reverse_does_not_trip_forward_fault() {
    let mut r = td21_relay();
    r.dist_reverse = true;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3];
    mon.iph = vec![Complex64::new(0.1, 0.0); 3];
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.001;
    for _ in 0..18 {
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    mon.vph = vec![Complex64::new(4.1, -2.95); 3];
    mon.iph = vec![Complex64::new(6.0, 0.0); 3];
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open[G]);
}

// --- MakeLike ---------------------------------------------------------------

#[test]
fn make_like_copies_settings_including_delay_and_mech() {
    let mut base = Relay::new("base");
    base.control_type = RelayControlType::Distance;
    base.phase_trip = 700.0;
    base.reset_time = 22.0;
    base.num_reclose = 2;
    base.reclose_intervals[..2].copy_from_slice(&[0.5, 1.0]);
    base.normal_state[1] = ControlAction::Open;
    base.normal_state_set = true;
    base.definite_time_delay = 0.3;
    base.mechanical_delay = 0.04;
    base.z1mag = 0.8;
    base.dist_reverse = true;
    base.single_ph_trip = true;
    base.phase_curve = Some(build_tcc("2", "1 10", "1 0.1"));

    let mut r = Relay::new("r1");
    r.make_like(&base);
    assert_eq!(r.control_type, RelayControlType::Distance);
    assert_eq!(r.phase_trip, 700.0);
    assert_eq!(r.reset_time, 22.0);
    assert_eq!(r.num_reclose, 2);
    assert_eq!(r.reclose_intervals[..2], [0.5, 1.0]);
    assert_eq!(r.normal_state[1], ControlAction::Open);
    assert!(r.normal_state_set);
    assert_eq!(r.definite_time_delay, 0.3);
    assert_eq!(r.mechanical_delay, 0.04);
    assert_eq!(r.z1mag, 0.8);
    assert!(r.dist_reverse);
    assert!(r.single_ph_trip);
    assert!(r.phase_curve.is_some());
}

// ---- executive-driven integration tests ----

fn dump(dss: &mut Dss, prop: &str) -> String {
    dss.command(&format!("? relay.r1.{prop}"));
    dss.result().to_string()
}

/// The default property dump (r4133 renames + array forms): SwitchedObj defaults
/// to the monitored element, Type=Current, Shots=4, RecloseIntervals=[ 0.5 2 2],
/// Action dumps empty (deprecated), Normal/State render the per-phase arrays.
#[test]
fn default_dump_matches_oracle() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47",
        "new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
        "new relay.r1 monitoredobj=line.l1 monitoredterm=1 type=current",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "SwitchedObj"), "Line.l1");
    assert_eq!(dump(&mut dss, "Type"), "Current");
    assert_eq!(dump(&mut dss, "Shots"), "4");
    assert_eq!(dump(&mut dss, "RecloseIntervals"), "[ 0.5 2 2]");
    assert_eq!(dump(&mut dss, "Action"), "");
    assert_eq!(dump(&mut dss, "Normal"), "[closed, closed, closed, ]");
    assert_eq!(dump(&mut dss, "State"), "[closed, closed, closed, ]");
    // Deprecated aliases still parse/render the canonical field.
    assert_eq!(dump(&mut dss, "PhaseTrip"), dump(&mut dss, "PhPickup"));
}

/// `state=open` (ganged) drives every phase's `FPresentState`, defaults
/// `NormalState`, and forces the controlled terminal open at parse time.
#[test]
fn state_open_forces_controlled_terminal_open_at_parse() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new line.l2 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=300",
        "new relay.r1 monitoredobj=line.l1 switchedobj=line.l2 state=open",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "State"), "[open, open, open, ]");
    assert_eq!(dump(&mut dss, "Normal"), "[open, open, open, ]");
    assert!(
        line_term1_max_current(&mut dss, "Line.l2") < 1.0,
        "state=open should force l2 open at parse"
    );
    assert!(line_term1_max_current(&mut dss, "Line.l1") > 1.0);
}

/// r4133 `InterpretRelayState` is FIRST-CHARACTER only (`'o'`/`'c'`); any other
/// spelling leaves the state array UNCHANGED, silently — it is NOT the old
/// dss_capi 0.14.5 `trip`->open alias. Pinned against oddie:r4133, where
/// `normal=trip` and `normal=xyz` both leave Normal at `[closed,closed,closed]`,
/// `normal=openZ` sets open (leading char), and a bracketed list goes
/// phase-by-phase.
#[test]
fn state_parse_is_first_char_only_r4133() {
    let base = [
        "clear",
        "new circuit.t basekv=12.47",
        "new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
    ];
    let closed = "[closed, closed, closed, ]";
    let open = "[open, open, open, ]";

    // `trip` / arbitrary non-o/c: leaves every phase unchanged (default closed).
    for spec in ["normal=trip", "normal=xyz", "state=trip"] {
        let mut dss = Dss::new();
        for c in base {
            dss.command(c);
        }
        dss.command(&format!("new relay.r1 monitoredobj=line.l1 {spec}"));
        assert!(dss.errors().is_empty(), "{spec}: errors {:?}", dss.errors());
        assert_eq!(dump(&mut dss, "Normal"), closed, "{spec} Normal");
        assert_eq!(dump(&mut dss, "State"), closed, "{spec} State");
    }

    // Leading 'o' wins regardless of the tail (`openZ`, `o`).
    for spec in ["normal=openZ", "normal=o"] {
        let mut dss = Dss::new();
        for c in base {
            dss.command(c);
        }
        dss.command(&format!("new relay.r1 monitoredobj=line.l1 {spec}"));
        assert!(dss.errors().is_empty(), "{spec}: errors {:?}", dss.errors());
        assert_eq!(dump(&mut dss, "Normal"), open, "{spec} Normal");
    }

    // Bracketed list: phase-by-phase, a non-o/c token keeps that phase.
    let mut dss = Dss::new();
    for c in base {
        dss.command(c);
    }
    dss.command("new relay.r1 monitoredobj=line.l1 normal=[open trip closed]");
    assert!(dss.errors().is_empty(), "errors {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "Normal"), "[open, closed, closed, ]");
}

/// End-to-end: an overcurrent relay on an overloaded line trips its controlled
/// terminal open, logging a per-phase `Opened on Ph Definite Time` and actually
/// opening the line.
#[test]
fn relay_trips_overloaded_line() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=5000",
        "new relay.r1 monitoredobj=line.l1 type=current phasetrip=1 delay=0.1 shots=1 eventlog=yes",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set controlmode=time",
        "set mode=duty number=5 stepsize=1 hour=0",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert!(
        dss.event_log().iter().any(|s| s.contains("OPENED ON PH")),
        "expected a phase trip; log = {:?}",
        dss.event_log()
    );
    assert!(
        line_term1_max_current(&mut dss, "Line.l1") < 1.0,
        "the tripped relay should open the line"
    );
}

#[test]
fn recloseintervals_none_clears_the_array() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47",
        "new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
        "new relay.r1 monitoredobj=line.l1 recloseintervals=NONE",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "RecloseIntervals"), "[NONE]");
    assert_eq!(dump(&mut dss, "Shots"), "1");
}

/// The r4133 property table has 71 class props (+ BaseFreq/Enabled tail = 73
/// defs, NumProps 74).
#[test]
fn r4133_property_table_has_71_props() {
    use crate::obj::dss_enum::EnumRegistry;
    let cls = super::class_props(&EnumRegistry::new());
    assert_eq!(
        cls.property_index("SinglePhTrip"),
        Some(prop::SINGLE_PH_TRIP)
    );
    assert_eq!(cls.property_index("PhCurve"), Some(prop::PH_CURVE));
    assert_eq!(
        cls.property_index("OC_GndPickup"),
        Some(prop::OC_GND_PICKUP)
    );
    assert_eq!(
        cls.property_index("MechanicalDelay"),
        Some(prop::MECHANICAL_DELAY)
    );
    assert_eq!(
        cls.property_index("Undervoltcurve"),
        Some(prop::UNDERVOLT_CURVE)
    );
}

/// RP3.7(b) — the r4133 `WasQuoted` split at the property seam
/// (`Relay.pas:1256-1306`): a BARE single token is ganged, a QUOTED single token
/// writes phase 1 only. The port's pre-RP3.7 `set_enum_array` keyed the split on
/// `values.len() == 1`, so `state=(open)` filled all three phases.
///
/// Bytes measured on the vendored r4133 DLL through `epri-worker`
/// (`tmp/rp37/probe_b1.py` -> `out_b1.txt` B1(1), deck `decks/p4_relay3.dss`):
/// `state=(open)` -> `[open, closed, closed, ]` (Normal follows through the
/// supplemental), `state=open` -> `[open, open, open, ]`, and `normal=(closed)`
/// over an all-open Normal -> `[closed, open, open, ]`.
///
/// Non-vacuity: reverting the seam to the length heuristic renders
/// `[open, open, open, ]` on the first assertion and `[closed, closed, closed, ]`
/// on the last.
#[test]
fn a_quoted_single_token_is_per_phase_a_bare_one_is_ganged() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.p4a basekv=115 pu=1.0 phases=3 bus1=src",
        "new line.l1 bus1=src bus2=b1 phases=3 r1=0.25 x1=0.6 c1=3 length=1 units=km",
        "new relay.r1 monitoredobj=line.l1 monitoredterm=1 type=current phasetrip=800",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "State"), "[closed, closed, closed, ]");

    // Quoted single token -> slot 1 only; Normal follows on this first write.
    dss.command("edit relay.r1 state=(open)");
    assert_eq!(dump(&mut dss, "State"), "[open, closed, closed, ]");
    assert_eq!(dump(&mut dss, "Normal"), "[open, closed, closed, ]");

    // Bare single token -> ganged.
    dss.command("edit relay.r1 state=closed");
    assert_eq!(dump(&mut dss, "State"), "[closed, closed, closed, ]");
    dss.command("edit relay.r1 state=open");
    assert_eq!(dump(&mut dss, "State"), "[open, open, open, ]");

    // The same split on Normal, over an all-open Normal.
    dss.command("edit relay.r1 normal=open");
    assert_eq!(dump(&mut dss, "Normal"), "[open, open, open, ]");
    dss.command("edit relay.r1 normal=(closed)");
    assert_eq!(dump(&mut dss, "Normal"), "[closed, open, open, ]");
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
}

/// RP3.7(b) — the per-phase parse honors at most FIVE tokens
/// (`Relay.pas:1286` `While (Length(DataStr2)>0) and (i < RELAYCONTROLMAXDIM)`),
/// while the render prints one token per controlled-element phase. The cap is
/// only observable on a >5-phase controlled element, so the pin drives a 6-phase
/// line; the generic ordinal tokenizer the seam used before RP3.7 read
/// `array_size()` = six tokens and applied the sixth.
///
/// Measured on the r4133 DLL (`tmp/rp37/probe_b1b.py` -> `out_b1b.txt` B1(3b),
/// deck `decks/b1_relay6.dss`): from an all-closed ganged baseline,
/// `state=(closed, closed, closed, closed, closed, open)` leaves
/// `[closed, closed, closed, closed, closed, closed, ]`.
///
/// r4133's own FRESH 6-phase render is `[closed, closed, closed, open, open,
/// open, ]` (`out_b1.txt` B1(3)) — `Create` allocates `FPresentState` with
/// `FNPhases` (3) entries and initializes only those (`Relay.pas:829-843`), so
/// slots 4..6 are an out-of-bounds heap read. That defect is NOT reproduced
/// (2026-08-02 policy, A1 D8/A2a D7): the port initializes all six in-bounds
/// slots to closed, which is why the pin ganges to a known baseline first.
#[test]
fn the_property_seam_caps_the_per_phase_parse_at_five_tokens() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.b1 basekv=115 pu=1.0 phases=3 bus1=src",
        "new line.l6 bus1=src.1.2.3.1.2.3 bus2=b1.1.2.3.1.2.3 phases=6 r1=0.25 x1=0.6 c1=3 length=1 units=km",
        "new relay.r6 monitoredobj=line.l6 monitoredterm=1 type=current phasetrip=800",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());
    let closed6 = "[closed, closed, closed, closed, closed, closed, ]";
    let get = |dss: &mut Dss, prop: &str| {
        dss.command(&format!("? relay.r6.{prop}"));
        dss.result().to_string()
    };
    // One token per controlled-element phase (six), all in-bounds and closed.
    assert_eq!(get(&mut dss, "State"), closed6);

    dss.command("edit relay.r6 state=closed");
    assert_eq!(get(&mut dss, "State"), closed6);
    dss.command("edit relay.r6 state=(closed, closed, closed, closed, closed, open)");
    assert_eq!(
        get(&mut dss, "State"),
        closed6,
        "the 6th token must be dropped"
    );

    // Five tokens are all honored (the boundary the cap sits on).
    dss.command("edit relay.r6 state=(open, closed, open, closed, open)");
    assert_eq!(
        get(&mut dss, "State"),
        "[open, closed, open, closed, open, closed, ]"
    );
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
}

/// RP3.7(b) — the Edit supplemental (`Relay.pas:616-619`, `CASE PropertyIdxMap
/// OF 19, 40:`) sits OUTSIDE `InterpretRelayState`, so a `State`/`Action` write
/// latches `NormalStateSet` (copying Present into Normal) even when the
/// interpreter refused it while `Locked` (`:1244`) or matched no `o`/`c` token.
/// The port ran the latch for `State` (side effects are unconditional) but not
/// for `Action`, whose early returns skipped it — so a locked `action=open`
/// followed by an unlocked `state=open` carried Normal to open.
///
/// Measured on the r4133 DLL (`tmp/rp37/out_b1.txt` B1(2a)/(2b),
/// `out_b1b.txt` B1(2e)/(2f)): in all four sequences the later unlocked
/// `state=open` leaves `Normal` at `[closed, closed, closed, ]`, while the
/// control sequence with no earlier write (B1(2c)) carries it to
/// `[open, open, open, ]`.
///
/// Non-vacuity: restoring `state_side_effect()` inside `do_action`'s guarded
/// path makes the two `action` rows render `[open, open, open, ]`.
#[test]
fn a_refused_or_unmatched_action_still_runs_the_normal_defaults_supplemental() {
    let build = || {
        let mut dss = Dss::new();
        for c in [
            "clear",
            "new circuit.p4a basekv=115 pu=1.0 phases=3 bus1=src",
            "new line.l1 bus1=src bus2=b1 phases=3 r1=0.25 x1=0.6 c1=3 length=1 units=km",
            "new relay.r1 monitoredobj=line.l1 monitoredterm=1 type=current phasetrip=800",
        ] {
            dss.command(c);
        }
        dss
    };
    let closed = "[closed, closed, closed, ]";
    let open = "[open, open, open, ]";

    // Every first write that reaches the supplemental latches Normal at the
    // pre-write (all-closed) Present, refused or unmatched.
    for first in [
        &["edit relay.r1 lock=yes", "edit relay.r1 state=open"][..],
        &["edit relay.r1 lock=yes", "edit relay.r1 action=open"][..],
        &["edit relay.r1 action=xyz"][..],
        &["edit relay.r1 state=xyz"][..],
    ] {
        let mut dss = build();
        for c in first {
            dss.command(c);
        }
        assert_eq!(dump(&mut dss, "State"), closed, "{first:?}: State moved");
        assert_eq!(dump(&mut dss, "Normal"), closed, "{first:?}: Normal moved");
        dss.command("edit relay.r1 lock=no");
        dss.command("edit relay.r1 state=open");
        assert_eq!(dump(&mut dss, "State"), open, "{first:?}: State");
        assert_eq!(
            dump(&mut dss, "Normal"),
            closed,
            "{first:?}: the supplemental already latched NormalStateSet"
        );
        assert!(dss.errors().is_empty(), "{first:?}: {:?}", dss.errors());
    }

    // Control (r4133 B1(2c)): with no earlier write, the first state= carries
    // Normal with it.
    let mut dss = build();
    dss.command("edit relay.r1 state=open");
    assert_eq!(dump(&mut dss, "State"), open);
    assert_eq!(dump(&mut dss, "Normal"), open);
}

/// RP3.7(b), the census cell — `GetPropertyValue` 39/40 loop the **live**
/// `ControlledElement.NPhases` (`Relay.pas:1407-1428`), so after `makeposseq`
/// reduces the switched line to one conductor the relay renders ONE token.
/// `Sample` (`:1318`), `Reset` (`:1454`) and `RecalcElementData` (`:965`) use
/// the same live `Min(RELAYCONTROLMAXDIM, ControlledElement.Nphases)` bound.
///
/// The port reads that count off the controlled-element snapshot, which
/// `make_pos_sequence` now refreshes from the live `PosSeqCtx`; before RP3.7 the
/// frozen parse-time 3 survived, so the render printed three tokens AND the
/// `Sample` resync read conductors 2..3 past the end of the 1-conductor element
/// as *open* — the port's pre-fix `State = [closed, open, open, ]`, which is the
/// `props_r4133` census row for `relay.state` on
/// `modes/makeposseq/makeposseq_ctrl.dss` (that deck's relay statement is
/// reproduced here; `relay.normal` is its sibling row).
///
/// Bytes measured on the r4133 DLL (`tmp/rp37/probe.md` §6 P4(ii), re-measured
/// `out_b1.txt` B1(4)): `[closed, closed, closed, ]` before, `[closed, ]` after
/// — and still `[closed, ]` after a later unrelated `edit`, whose `recalc`
/// re-reads the snapshot.
#[test]
fn the_render_bound_follows_makeposseq() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.psq basekv=115 pu=1.0 phases=3 bus1=src",
        "new line.l2 bus1=src bus2=b1 phases=3 r1=0.25 x1=0.6 c1=3 length=1 units=km",
        "new line.sw bus1=b1 bus2=b2 phases=3 switch=yes",
        "new relay.rel monitoredobj=line.l2 monitoredterm=1 type=current phasetrip=800",
        "new load.ld bus1=b2 phases=3 kv=115 kw=2000 pf=0.95 model=1",
        "set voltagebases=[115]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "setup: {:?}", dss.errors());
    let get = |dss: &mut Dss, prop: &str| {
        dss.command(&format!("? relay.rel.{prop}"));
        dss.result().to_string()
    };
    assert_eq!(get(&mut dss, "State"), "[closed, closed, closed, ]");
    assert_eq!(get(&mut dss, "Normal"), "[closed, closed, closed, ]");

    dss.command("makeposseq");
    dss.command("solve"); // runs Sample against the now-1-conductor element
    assert!(dss.errors().is_empty(), "makeposseq: {:?}", dss.errors());
    assert_eq!(get(&mut dss, "State"), "[closed, ]");
    assert_eq!(get(&mut dss, "Normal"), "[closed, ]");

    // A later edit re-runs `recalc`, which re-reads the snapshot: the
    // pre-pos-seq bound must not come back.
    dss.command("edit relay.rel phasetrip=900");
    assert_eq!(get(&mut dss, "State"), "[closed, ]");
    assert_eq!(get(&mut dss, "Normal"), "[closed, ]");
}

/// RP3.7(b) — the per-phase write reaches the CONDUCTORS through the new raw
/// seam, not just the render: `state=(open, closed, closed)` on a 3-phase
/// controlled line zeroes phase 1's current and leaves phases 2/3 carrying load.
///
/// The r4133 DLL's own terminal currents for this deck+edit
/// (`tmp/rp37/probe.md` §6 P4(i-a), `decks/p4_relay3.dss`) are
/// `['0.000000 -0.000000j', '-7.824715 -7.067886j', '-2.219181 +10.328951j', …]`.
/// The DLL prints six decimals, so the comparison band is that print's own
/// rounding (5e-7 absolute per component) — the port lands inside it on every
/// component (`-7.824714600188 -7.067886356275j`,
/// `-2.219180813961 +10.328950858660j`, `tmp/rp37/out_b1_port_fixed.txt` /
/// `probe_b1c`), i.e. the two engines agree far below the faer-vs-KLU floor.
///
/// Non-vacuity: a ganged interpretation of the quoted list (the pre-RP3.7 seam
/// on a single token, or a `set_all_present` here) zeroes phases 2 and 3 too.
#[test]
fn per_phase_state_write_through_the_executive_opens_only_its_phase() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.p4a basekv=115 pu=1.0 phases=3 bus1=src",
        "new line.l1 bus1=src bus2=b1 phases=3 r1=0.25 x1=0.6 c1=3 length=1 units=km",
        "new relay.r1 monitoredobj=line.l1 monitoredterm=1 type=current phasetrip=800",
        "new load.ld bus1=b1 phases=3 kv=115 kw=2000 pf=0.95 model=1",
        "set voltagebases=[115]",
        "calcvoltagebases",
        "solve",
        "edit relay.r1 state=(open, closed, closed)",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "State"), "[open, closed, closed, ]");

    let snaps = dss.snapshot_elements();
    let l1 = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("Line.l1"))
        .expect("no Line.l1");
    // r4133's six-decimal print of terminal 1, phases 1..3.
    for (k, want) in [
        (0usize, Complex64::new(0.0, 0.0)),
        (1, Complex64::new(-7.824_715, -7.067_886)),
        (2, Complex64::new(-2.219_181, 10.328_951)),
    ] {
        let got = l1.currents[k];
        assert!(
            (got.re - want.re).abs() < 5e-7 && (got.im - want.im).abs() < 5e-7,
            "phase {} current {got} vs r4133 {want}",
            k + 1
        );
    }
    // Phases 2/3 really are carrying load (the open phase is not the whole line).
    assert!(l1.currents[1].norm() > 10.0 && l1.currents[2].norm() > 10.0);
}

fn line_term1_max_current(dss: &mut Dss, name: &str) -> f64 {
    let snaps = dss.snapshot_elements();
    let s = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no element {name}"));
    let mut m = 0.0_f64;
    for k in 0..3 {
        let re = s.currents[k].re;
        let im = s.currents[k].im;
        m = m.max((re * re + im * im).sqrt());
    }
    m
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemId};
    use crate::obj::base::DssObject;

    #[test]
    fn resyncs_monitored_and_recomputes_vbase() {
        let mut r = Relay::new("r1");
        r.ccd.monitored_element = Some(ElemId::new(1, 0));
        r.monitored_element_terminal = 1;
        r.kv_base = 12.47;
        r.pct_pickup47 = 2.0;
        r.control_type = RelayControlType::Distance;
        let ctx = PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                yorder: 2,
                bus_names: vec!["b1".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = r.make_pos_sequence(&ctx);
        assert_eq!(r.ccd.cd.nphases, 1);
        assert_eq!(r.get_bus_name(1), "b1");
        assert!((r.vbase - 12_470.0).abs() < 1e-9);
        assert!((r.pickup_volts47 - 249.4).abs() < 1e-9);
        assert!(plan.run_base);
        assert_eq!(r.monitored_element_ref(), Some(ElemId::new(1, 0)));
    }

    #[test]
    fn vbase_recomputed_outside_nil_guard() {
        let mut r = Relay::new("r1");
        r.kv_base = 12.47;
        r.pct_pickup47 = 2.0;
        r.make_pos_sequence(&PosSeqCtx::default());
        let expected = 12.47 / crate::util::sqrt3() * 1000.0;
        assert!((r.vbase - expected).abs() < 1e-9);
        assert!((r.pickup_volts47 - expected * 2.0 * 0.01).abs() < 1e-9);
    }
}

#[test]
fn relay_control_type_pins_enum_ordinals() {
    // RelayTypeEnum ordinals (Relay.pas): the `2` ordinal is deliberately unused
    // upstream, so `from_ordinal(2)` is None (kept-value fallback at the setter).
    for (variant, ord) in [
        (RelayControlType::Current, 0),
        (RelayControlType::Voltage, 1),
        (RelayControlType::RevPower, 3),
        (RelayControlType::NegCurrent, 4),
        (RelayControlType::NegVoltage, 5),
        (RelayControlType::Generic, 6),
        (RelayControlType::Distance, 7),
        (RelayControlType::Td21, 8),
        (RelayControlType::Doc, 9),
    ] {
        assert_eq!(variant.ordinal(), ord);
        assert_eq!(RelayControlType::from_ordinal(ord), Some(variant));
    }
    assert_eq!(RelayControlType::from_ordinal(2), None);
    assert_eq!(RelayControlType::from_ordinal(10), None);
}

#[test]
fn set_i32_type_keeps_value_on_unregistered_ordinal() {
    // The `2` (and any out-of-range) ordinal is never produced by the
    // RelayTypeEnum parse, but pin the deliberate keep-old fallback at the
    // setter: `from_ordinal(value).unwrap_or(self.control_type)`.
    let mut r = Relay::new("r1");
    r.set_i32(prop::TYP, RelayControlType::Distance.ordinal()); // 7 -> Distance
    assert_eq!(r.control_type, RelayControlType::Distance);
    r.set_i32(prop::TYP, 2); // unregistered gap ordinal -> keep Distance
    assert_eq!(r.control_type, RelayControlType::Distance);
    assert_eq!(r.get_i32(prop::TYP), 7);
}
