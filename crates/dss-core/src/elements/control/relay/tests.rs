use super::*;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::control::control_elem::{CTRL_CLOSE, CTRL_OPEN, CTRL_RESET, RefSnapshot};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::exec::Dss;
use crate::obj::base::DssObject;
use crate::solution::control_queue::TimeRec;
use crate::solution::{ControlQueue, EventLog, SolveMode};

/// The ganged operation slot (`IdxMultiPh = NPhases+1`, frozen at 4 for the
/// `Create`-time 3-phase relay).
const G: usize = 4;

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: 1,
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
    fn recalc_element_data(&mut self, _sys: &SysCtx) {}
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
    errors: Vec<String>,
    y_changed: bool,
    sys: SysCtx,
}
impl Scratch {
    fn new() -> Self {
        Self {
            queue: ControlQueue::new(),
            events: EventLog::new(),
            errors: Vec::new(),
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
            control_mode: 2, // TIMEDRIVEN
            int_hour,
            t,
            dbl_hour: int_hour as f64 + t / 3600.0,
            control_iter: 1,
            self_ref: ElemRef { cls: 0, idx: 0 },
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
    let mut errors = Vec::new();
    for (n, v) in [("npts", npts), ("C_array", c), ("T_array", t)] {
        let idx = cls.property_index(n).unwrap();
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
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
    r.control_type = ctype::CURRENT;
    r.phase_curve = Some(build_tcc("2", "1 10", "1 0.1"));
    r.phase_trip = 1.0;
    r.ccd.show_event_log = true;
    r.ctrl_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    r
}

fn log_has(sc: &Scratch, needle: &str) -> bool {
    sc.events.entries().iter().any(|e| e.contains(needle))
}

/// Count non-`Debug Sample` event-log lines (the r4133 unconditional sample line
/// is noise for these assertions).
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
    assert_eq!(r.control_type, ctype::CURRENT);
    assert_eq!(r.phase_trip, 1.0);
    assert_eq!(r.num_reclose, 3); // Shots default 4
    assert_eq!(r.reset_time, 15.0);
    assert_eq!(r.reclose_intervals[..3], [0.5, 2.0, 2.0]);
    assert_eq!(r.present_state[1], CTRL_CLOSE);
    assert_eq!(r.normal_state[1], CTRL_CLOSE);
    assert!(!r.normal_state_set);
    assert_eq!(r.idx_multi_ph, G);
    assert_eq!(r.pickup_amps46, 20.0);
    assert_eq!(r.doc_trip_set_high, -1.0);
    assert!(r.doc_p1_blocking);
    assert!(!r.single_ph_trip);
    assert!(r.ccd.cd.yprim.is_none());
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
    assert_eq!(open.code, CTRL_OPEN);
    assert!((t_open - 0.25).abs() < 1e-9, "open time = {t_open}");
    let (close, t_close) = sc.queue.pop_time(far, false).unwrap();
    assert_eq!(close.code, CTRL_CLOSE);
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
    assert_eq!(open.code, CTRL_OPEN);
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
    assert_eq!(r.present_state[1], CTRL_OPEN);
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
    r.do_pending_action(CTRL_OPEN, 1, &mut ctrl, &mut sc.ctx(0, 0.0));
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
    r.do_pending_action(CTRL_OPEN, 1, &mut ctrl, &mut sc.ctx(0, 0.0));
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
    r.do_pending_action(CTRL_OPEN, 0, &mut ctrl, &mut sc.ctx(0, 0.1));
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
    r.do_pending_action(CTRL_OPEN, 0, &mut ctrl, &mut sc.ctx(0, 0.0));
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
    r.do_pending_action(CTRL_OPEN, 0, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // still opened
    assert!(sc.y_changed);
    assert_eq!(sc.events.entries().len(), 0); // gated (no Debug Sample from do_pending)
}

#[test]
fn do_pending_close_recloses_and_counts() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.present_state[i] = CTRL_OPEN;
    }
    r.armed_for_close[G] = true;
    r.operation_count[G] = 1;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // currently open
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_CLOSE, 0, &mut ctrl, &mut sc.ctx(0, 0.0));
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
            r.present_state[i] = CTRL_OPEN;
        }
        r.armed_for_open[G] = true;
        let mut ctrl = MockElem::new(3);
        ctrl.cd.set_terminal_closed(1, false);
        let mut sc = Scratch::new();
        r.do_pending_action(CTRL_OPEN, 0, &mut ctrl, &mut sc.ctx(0, 0.0));
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
        r.do_pending_action(CTRL_CLOSE, 0, &mut ctrl, &mut sc.ctx(0, 0.0));
        assert!(ctrl.cd.terminal_all_phases_closed(1));
        assert!(!sc.y_changed);
        assert_eq!(non_debug_lines(&sc), 0);
    }
}

/// **D4:** the queued `DoPendingAction(CTRL_RESET)` no longer runs the full
/// `Reset` — it only resets `OperationCount` to 1 for closed phases, does NOT
/// force the element back to normal state, and logs (upstream bug) as
/// `Recloser.<name>`.
#[test]
fn do_pending_reset_only_resets_opcount_d4() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.present_state[i] = CTRL_CLOSE; // closed phase
    }
    r.armed_for_open[G] = false;
    r.operation_count[G] = 3;
    let mut ctrl = MockElem::new(3); // closed
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_RESET, 0, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert_eq!(r.operation_count[G], 1, "opcount reset to 1");
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // element NOT forced (still closed)
    assert!(!sc.y_changed, "D4 reset does not force the element / Y");
    // TODO(compat): reset event logged as Recloser.<name> (upstream copy-paste;
    // the element name keeps its case, only the Action is uppercased).
    assert!(
        log_has(&sc, "Recloser.r1"),
        "log = {:?}",
        sc.events.entries()
    );
    assert!(log_has(&sc, "PHASE ALL RESET (3PH RESET)"));
}

#[test]
fn do_pending_reset_skipped_when_all_open() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.present_state[i] = CTRL_OPEN; // no closed phase ⇒ no reset
    }
    r.operation_count[G] = 3;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false);
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_RESET, 0, &mut ctrl, &mut sc.ctx(0, 0.0));
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
        r.normal_state[i] = CTRL_CLOSE;
        r.present_state[i] = CTRL_OPEN;
    }
    r.operation_count[G] = 4;
    r.locked_out[G] = true;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // start open
    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // restored closed
    assert_eq!(r.present_state[1], CTRL_CLOSE);
    assert!(!r.locked_out[1]);
    assert_eq!(r.operation_count[1], 1);
    assert!(sc.y_changed);
    assert!(log_has(&sc, "RESETTING"));
}

#[test]
fn reset_with_open_normal_state_locks_out() {
    let mut r = armed_relay();
    for i in 1..=3 {
        r.normal_state[i] = CTRL_OPEN;
    }
    let mut ctrl = MockElem::new(3); // start closed
    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // forced open
    assert_eq!(r.present_state[1], CTRL_OPEN);
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
        r.normal_state[i] = CTRL_CLOSE;
        r.present_state[i] = CTRL_OPEN;
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
        r.normal_state[i] = CTRL_OPEN;
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
    r.control_type = ctype::REVPOWER;
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
    r.control_type = ctype::REVPOWER;
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
    r.control_type = ctype::NEGCURRENT;
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
    r.control_type = ctype::NEGVOLTAGE;
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
    r.control_type = ctype::VOLTAGE;
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
    r.control_type = ctype::VOLTAGE;
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
    r.control_type = ctype::VOLTAGE;
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

#[test]
fn voltage_recloses_when_voltage_recovers() {
    let mut r = armed_relay();
    r.control_type = ctype::VOLTAGE;
    r.vbase = 1000.0;
    r.operation_count[G] = 1;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // present_state OPEN
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(1000.0, 0.0); 3];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert_eq!(r.present_state[1], CTRL_OPEN);
    assert!(r.armed_for_close[G]);
    assert_eq!(sc.queue.queue_size(), 1);
}

// --- DOC directional decision tree ------------------------------------------

#[test]
fn doc_phase_time_test_directional_split() {
    let mut r = Relay::new("r1");
    r.control_type = ctype::DOC;
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
    r.control_type = ctype::DOC;
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
    r.control_type = ctype::DOC;
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
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
    r.control_type = ctype::DOC;
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
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
    r.control_type = ctype::DISTANCE;
    r.dist_z1 = Complex64::new(1.0, 0.0);
    r.dist_z0 = Complex64::new(1.0, 0.0);
    r.dist_k0 = Complex64::ZERO;
    r.mground = 1.0;
    r.mphase = 1.0;
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
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
    r.control_type = ctype::GENERIC;
    r.monitor_var_index = 1;
    r.over_trip = 1.2;
    r.under_trip = 0.8;
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
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
    r.control_type = ctype::GENERIC;
    r.monitor_variable = "vd".to_string();
    r.monitor_var_names = vec!["Frequency".into(), "Vd".into()];
    r.mon_snap = Some(RefSnapshot {
        full_name: "Generator.g".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    r.recalc();
    assert_eq!(r.monitor_var_index, 2);

    let mut r2 = Relay::new("g2");
    r2.control_type = ctype::GENERIC;
    r2.monitor_variable = "nosuch".to_string();
    r2.monitor_var_names = vec!["Frequency".into()];
    r2.mon_snap = Some(RefSnapshot {
        full_name: "Generator.g".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r2.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
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
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
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
    r.control_type = ctype::TD21;
    r.dist_z1 = Complex64::new(1.0, 0.0);
    r.dist_k0 = Complex64::ZERO;
    r.mground = 1.0;
    r.mphase = 1.0;
    r.phase_trip = 1.0;
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
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
    base.control_type = ctype::DISTANCE;
    base.phase_trip = 700.0;
    base.reset_time = 22.0;
    base.num_reclose = 2;
    base.reclose_intervals[..2].copy_from_slice(&[0.5, 1.0]);
    base.normal_state[1] = CTRL_OPEN;
    base.normal_state_set = true;
    base.definite_time_delay = 0.3;
    base.mechanical_delay = 0.04;
    base.z1mag = 0.8;
    base.dist_reverse = true;
    base.single_ph_trip = true;
    base.phase_curve = Some(build_tcc("2", "1 10", "1 0.1"));

    let mut r = Relay::new("r1");
    r.make_like(&base);
    assert_eq!(r.control_type, ctype::DISTANCE);
    assert_eq!(r.phase_trip, 700.0);
    assert_eq!(r.reset_time, 22.0);
    assert_eq!(r.num_reclose, 2);
    assert_eq!(r.reclose_intervals[..2], [0.5, 1.0]);
    assert_eq!(r.normal_state[1], CTRL_OPEN);
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

fn line_term1_max_current(dss: &mut Dss, name: &str) -> f64 {
    let snaps = dss.snapshot_elements();
    let s = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no element {name}"));
    let mut m = 0.0_f64;
    for k in 0..3 {
        let re = s.currents[2 * k];
        let im = s.currents[2 * k + 1];
        m = m.max((re * re + im * im).sqrt());
    }
    m
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemRef};
    use crate::obj::base::DssObject;

    #[test]
    fn resyncs_monitored_and_recomputes_vbase() {
        let mut r = Relay::new("r1");
        r.ccd.monitored_element = Some(ElemRef { cls: 1, idx: 0 });
        r.monitored_element_terminal = 1;
        r.kv_base = 12.47;
        r.pct_pickup47 = 2.0;
        r.control_type = ctype::DISTANCE;
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
        assert_eq!(r.monitored_element_ref(), Some(ElemRef { cls: 1, idx: 0 }));
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
