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

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: 1,
        mode: SolveMode::Snapshot,
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
    }
}

/// A 1-terminal mock carrying explicit per-phase currents and voltages plus a
/// fixed terminal power, so every Relay sub-type's sensing can be driven
/// directly. `get_currents` fills the terminal currents; `get_term_voltages`
/// the terminal voltages; `terminal_power` the < 3-phase / RevPower quantity.
struct MockElem {
    cd: CktElementData,
    iph: Vec<Complex64>,
    vph: Vec<Complex64>,
    power: Complex64,
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
        }
    }
    /// Equal real current on every phase (residual sum = nphases·mag).
    fn with_current(mut self, mag: f64) -> Self {
        for c in self.iph.iter_mut() {
            *c = Complex64::new(mag, 0.0);
        }
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

/// Build a populated `TccCurveObj` through the property engine.
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
/// `PhaseTrip=1`, controlled element a 3-phase line, event log on.
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
    assert_eq!(r.present_state, CTRL_CLOSE);
    assert_eq!(r.normal_state, CTRL_CLOSE);
    assert!(!r.normal_state_set);
    assert_eq!(r.pickup_amps46, 20.0); // BaseAmps46·PctPickup46·0.01
    assert_eq!(r.doc_trip_set_high, -1.0);
    assert!(r.doc_p1_blocking);
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
    assert!(r.armed_for_open);
    assert!(r.armed_for_close);
    assert!(r.phase_target);
    assert_eq!(r.relay_target, "Ph");
    assert_eq!(sc.queue.queue_size(), 2); // OPEN + reclose CLOSE
}

/// Pin the queued OPEN / reclose times: `TripTime = TDPhase·GetTCCTime(10) =
/// 2·0.1 = 0.2`, OPEN at `0.2 + BreakerTime(0.05) = 0.25`, reclose at
/// `+ RecloseIntervals[0] = +0.5 = 0.75`.
#[test]
fn overcurrent_queues_trip_and_reclose_at_correct_times() {
    let mut r = armed_relay();
    r.breaker_time = 0.05;
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
    assert!(r.armed_for_open);
    assert!(r.ground_target);
    assert!(!r.phase_target);
    assert_eq!(r.relay_target, " Gnd");
}

#[test]
fn overcurrent_phase_inst_first_operation_only() {
    let make = || {
        let mut r = armed_relay();
        r.phase_curve = Some(build_tcc("2", "100 200", "1 0.1")); // never picks up at 10 A
        r.phase_inst = 5.0;
        r
    };
    {
        let mut r = make();
        let mut ctrl = MockElem::new(3);
        let mut mon = MockElem::new(3).with_current(10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(r.armed_for_open, "inst trips on the first operation");
    }
    {
        let mut r = make();
        r.operation_count = 2; // inst only fires when operation_count == 1
        let mut ctrl = MockElem::new(3);
        let mut mon = MockElem::new(3).with_current(10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(!r.armed_for_open, "inst is first-operation only");
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
    assert_eq!(r.present_state, CTRL_OPEN);
    assert!(!r.armed_for_open);
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
    // Current falls below pickup → disarm and queue a RESET.
    let mut mon = MockElem::new(3); // zero current
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 1.0));
    assert!(!r.armed_for_open);
    assert!(!r.armed_for_close);
    assert!(!r.phase_target);
    assert_eq!(sc.queue.queue_size(), 3); // the prior two stay + a RESET
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
    r.do_pending_action(CTRL_OPEN, &mut ctrl, &mut sc.ctx(0, 0.1));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // opened
    assert!(!r.armed_for_open);
    assert!(sc.y_changed);
    assert!(
        log_has(&sc, "OPENED ON PH"),
        "log = {:?}",
        sc.events.entries()
    );
    assert!(log_has(&sc, "PHASE TARGET"));
    assert!(!r.locked_out); // operation_count 1 ≤ NumReclose 3
}

#[test]
fn do_pending_open_locks_out_after_last_shot() {
    let mut r = armed_relay();
    r.operation_count = 4; // > NumReclose 3
    r.armed_for_open = true;
    r.relay_target = "Ph".into();
    let mut ctrl = MockElem::new(3);
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_OPEN, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1));
    assert!(r.locked_out);
    assert!(log_has(&sc, "LOCKED OUT"));
}

#[test]
fn do_pending_open_gated_on_show_event_log() {
    // ShowEventLog off: the trip still acts, but emits no event-log lines.
    let mut r = armed_relay();
    r.ccd.show_event_log = false;
    r.armed_for_open = true;
    r.relay_target = "Ph".into();
    let mut ctrl = MockElem::new(3);
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_OPEN, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // still opened
    assert!(sc.y_changed);
    assert_eq!(sc.events.entries().len(), 0); // gated
}

#[test]
fn do_pending_close_recloses_and_counts() {
    let mut r = armed_relay();
    r.present_state = CTRL_OPEN;
    r.armed_for_close = true;
    r.operation_count = 1;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // currently open
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_CLOSE, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // reclosed
    assert_eq!(r.operation_count, 2);
    assert!(!r.armed_for_close);
    assert!(sc.y_changed);
    assert!(log_has(&sc, "CLOSED"));
}

#[test]
fn do_pending_open_close_are_wrong_state_no_ops() {
    // OPEN when already open.
    {
        let mut r = armed_relay();
        r.present_state = CTRL_OPEN;
        r.armed_for_open = true;
        let mut ctrl = MockElem::new(3);
        ctrl.cd.set_terminal_closed(1, false);
        let mut sc = Scratch::new();
        r.do_pending_action(CTRL_OPEN, &mut ctrl, &mut sc.ctx(0, 0.0));
        assert!(!ctrl.cd.terminal_all_phases_closed(1));
        assert!(!sc.y_changed);
        assert_eq!(sc.events.entries().len(), 0);
    }
    // CLOSE when already closed.
    {
        let mut r = armed_relay();
        r.present_state = CTRL_CLOSE;
        r.armed_for_close = true;
        let mut ctrl = MockElem::new(3);
        let mut sc = Scratch::new();
        r.do_pending_action(CTRL_CLOSE, &mut ctrl, &mut sc.ctx(0, 0.0));
        assert!(ctrl.cd.terminal_all_phases_closed(1));
        assert!(!sc.y_changed);
        assert_eq!(sc.events.entries().len(), 0);
    }
}

/// The queue-driven `DoPendingAction(CTRL_RESET)` entry (gated `ArmedForClose &&
/// !LockedOut`): logs "Reset", then runs the full `Reset()` — re-forces the
/// element to NormalState (raising SystemYChanged) and logs "Resetting".
#[test]
fn do_pending_reset_runs_full_reset_when_armed() {
    let mut r = armed_relay();
    r.armed_for_close = true;
    r.locked_out = false;
    r.normal_state = CTRL_CLOSE;
    r.present_state = CTRL_OPEN;
    r.operation_count = 3;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // open
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_RESET, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // re-forced closed
    assert_eq!(r.present_state, CTRL_CLOSE);
    assert_eq!(r.operation_count, 1);
    assert!(sc.y_changed);
    assert!(log_has(&sc, "RESETTING"));
}

#[test]
fn do_pending_reset_skipped_when_not_armed_for_close() {
    let mut r = armed_relay();
    r.armed_for_close = false; // the gate fails
    r.present_state = CTRL_OPEN;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false);
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_RESET, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // untouched
    assert!(!sc.y_changed);
    assert_eq!(sc.events.entries().len(), 0);
}

// --- Reset ------------------------------------------------------------------

#[test]
fn reset_with_restores_closed_normal_state_and_logs() {
    let mut r = armed_relay();
    r.normal_state = CTRL_CLOSE;
    r.present_state = CTRL_OPEN;
    r.operation_count = 4;
    r.locked_out = true;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // start open
    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // restored closed
    assert_eq!(r.present_state, CTRL_CLOSE);
    assert!(!r.locked_out);
    assert_eq!(r.operation_count, 1);
    assert!(sc.y_changed);
    assert!(log_has(&sc, "RESETTING"));
}

#[test]
fn reset_with_open_normal_state_locks_out() {
    let mut r = armed_relay();
    r.normal_state = CTRL_OPEN;
    let mut ctrl = MockElem::new(3); // start closed
    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // forced open
    assert_eq!(r.present_state, CTRL_OPEN);
    assert!(r.locked_out);
    assert_eq!(r.operation_count, r.num_reclose + 1);
    assert!(sc.y_changed);
}

/// Fail-on-regression guard for the WP7.2 step-2a Reset dirty edge (`d0addb4` /
/// `d1f48231`): `reset_with` must raise `SystemYChanged` **unconditionally**,
/// never gated on an all-or-nothing `terminal_all_phases_closed` aggregate. Seed
/// `[closed, open, open]` (aggregate false) with `normal=OPEN` ⇒ `[open, open,
/// open]` (aggregate still false), yet phase 0 flips closed→open — a real change
/// a `was_all_closed != want_all_closed` gate would miss.
#[test]
fn reset_with_partial_open_terminal_still_forces_rebuild() {
    let mut r = armed_relay();
    r.normal_state = CTRL_OPEN;
    let mut ctrl = MockElem::new(3);
    ctrl.cd.terminals[0].conductors_closed[0] = true; // phase 0 closed
    ctrl.cd.terminals[0].conductors_closed[1] = false; // phase 1 open
    ctrl.cd.terminals[0].conductors_closed[2] = false; // phase 2 open
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // aggregate false before

    let mut sc = Scratch::new();
    r.reset_with(&mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(
        sc.y_changed,
        "reset must force a Y rebuild even when the aggregate is unchanged"
    );
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // aggregate false after too
    assert!(!ctrl.cd.conductor_closed(1, 1)); // phase 0: closed → open (the missed change)
}

// --- One-shot sub-types (RevPower / 46 / 47) --------------------------------

#[test]
fn rev_power_trips_on_reverse_locks_out() {
    let mut r = armed_relay();
    r.control_type = ctype::REVPOWER;
    r.phase_inst = 1.0; // threshold 1 kW = 1000 W
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.power = Complex64::new(-5000.0, 0.0); // reverse 5 kW > 1 kW
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
    assert_eq!(r.operation_count, r.num_reclose + 1); // forced lockout
    assert_eq!(r.relay_target, "Rev P");
}

#[test]
fn rev_power_forward_no_trip() {
    let mut r = armed_relay();
    r.control_type = ctype::REVPOWER;
    r.phase_inst = 1.0;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.power = Complex64::new(5000.0, 0.0); // forward
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open);
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn neg_seq46_trips_on_unbalanced_current() {
    let mut r = armed_relay();
    r.control_type = ctype::NEGCURRENT;
    // Single-phase current ⇒ |I2| = |Ia|/3 = 100/3 ≈ 33.3 ≥ pickup 20.
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.iph = vec![Complex64::new(100.0, 0.0), Complex64::ZERO, Complex64::ZERO];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
    assert_eq!(r.operation_count, r.num_reclose + 1); // one-shot lockout
    assert_eq!(r.relay_target, "-Seq Curr");
}

#[test]
fn neg_seq47_trips_on_unbalanced_voltage() {
    let mut r = armed_relay();
    r.control_type = ctype::NEGVOLTAGE;
    r.pickup_volts47 = 100.0;
    // Single-phase voltage ⇒ |V2| = |Va|/3 = 1000/3 ≈ 333 ≥ pickup 100.
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![
        Complex64::new(1000.0, 0.0),
        Complex64::ZERO,
        Complex64::ZERO,
    ];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
    assert_eq!(r.operation_count, r.num_reclose + 1); // one-shot lockout
    assert_eq!(r.relay_target, "-Seq V");
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
    assert!(r.armed_for_open);
    assert_eq!(r.relay_target, "OV");
    assert_eq!(sc.queue.queue_size(), 1); // a single absolute-time OPEN
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
    assert!(r.armed_for_open);
    assert_eq!(r.relay_target, "UV");
}

/// Present state OPEN: once the voltage recovers above 0.9 pu the relay arms a
/// reclose (the `VoltageLogic` `else` branch), queuing a CLOSE.
#[test]
fn voltage_recloses_when_voltage_recovers() {
    let mut r = armed_relay();
    r.control_type = ctype::VOLTAGE;
    r.vbase = 1000.0;
    r.operation_count = 1; // ≤ num_reclose
    let mut ctrl = MockElem::new(3);
    ctrl.cd.set_terminal_closed(1, false); // present_state OPEN
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(1000.0, 0.0); 3]; // 1.0 pu > 0.9 ⇒ reclose
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert_eq!(r.present_state, CTRL_OPEN);
    assert!(r.armed_for_close);
    assert_eq!(sc.queue.queue_size(), 1); // a single reclose CLOSE
}

// --- DOC directional decision tree ------------------------------------------

/// The corpus DOC characteristic (`DOC_TiltAngleLow=95`, `DOC_TripSettingLow`,
/// no curves/circle/high-line): a current phasor on the trip side of the
/// straight line returns a 0 s definite trip; the other side returns -1 ("no op").
#[test]
fn doc_phase_time_test_directional_split() {
    let mut r = Relay::new("r1");
    r.control_type = ctype::DOC;
    r.doc_tilt_angle_low = 95.0;
    r.doc_trip_set_low = 3500.0;
    r.delay_time = 0.0;
    // tan(95°) < 0; cb=(-4000, 0): im(0) < tan95·(-4000+3500) ⇒ trip, t=0.
    let trip = Complex64::new(-4000.0, 0.0);
    assert_eq!(r.doc_phase_time_test(trip, trip.norm()), 0.0);
    // cb=(4000, 0): im(0) not < tan95·(7500) (very negative) ⇒ no op.
    let no = Complex64::new(4000.0, 0.0);
    assert_eq!(r.doc_phase_time_test(no, no.norm()), -1.0);
}

/// `DOC_P1Blocking` (default): forward net-balanced active power blocks the
/// element entirely (early return, no trip), while reverse power lets it proceed.
#[test]
fn doc_p1_blocking_blocks_on_forward_power() {
    let mut r = armed_relay();
    r.control_type = ctype::DOC;
    r.doc_p1_blocking = true;
    // 1-phase monitored element ⇒ GetControlPower = terminal_power.
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(1);
    mon.power = Complex64::new(5000.0, 0.0); // forward ⇒ blocked
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open);
    assert_eq!(sc.queue.queue_size(), 0);
}

/// Full DOC sample path on a 3-phase element: reverse net-balanced power gets
/// past the `DOC_P1Blocking` block, the voltage-referenced angle shift
/// (`cdang(I) − cdang(V)`) puts the current on the trip side of the default
/// (90°, trip-low 0) characteristic, and an OPEN is queued.
#[test]
fn doc_reverse_power_trips_through_full_sample() {
    let mut r = Relay::new("r1");
    r.control_type = ctype::DOC; // default characteristic (tilt 90, trip-low 0)
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    // Balanced positive-sequence voltages; currents 180° out ⇒ reverse power.
    let v = |deg: f64| Complex64::from_polar(1000.0, f64::to_radians(deg));
    mon.vph = vec![v(0.0), v(-120.0), v(120.0)];
    mon.iph = vec![v(180.0), v(60.0), v(-60.0)]; // = -vph
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(
        r.armed_for_open,
        "reverse power should trip the DOC element"
    );
    assert_eq!(r.relay_target, "DOC");
    assert!(sc.queue.queue_size() >= 1); // at least the OPEN
}

/// 3-phase forward net-balanced power blocks the DOC element (the
/// `Phase2SymComp` `GetControlPower` branch, complementing the 1-phase block).
#[test]
fn doc_three_phase_forward_power_blocks() {
    let mut r = Relay::new("r1");
    r.control_type = ctype::DOC;
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    let v = |deg: f64| Complex64::from_polar(1000.0, f64::to_radians(deg));
    mon.vph = vec![v(0.0), v(-120.0), v(120.0)];
    mon.iph = mon.vph.clone(); // in-phase ⇒ forward power ⇒ blocked
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open);
    assert_eq!(sc.queue.queue_size(), 0);
}

// --- Distance (21) ----------------------------------------------------------

/// A distance relay with a purely-resistive reach (`Z1=1∠0`, `Z0=Z1` ⇒ `K0=0`,
/// `Mground=Mphase=1`), set up directly (recalc would derive the same).
fn distance_relay() -> Relay {
    let mut r = Relay::new("r1");
    r.control_type = ctype::DISTANCE;
    r.dist_z1 = Complex64::new(1.0, 0.0);
    r.dist_z0 = Complex64::new(1.0, 0.0);
    r.dist_k0 = Complex64::ZERO; // (Z0-Z1)/3 / Z1
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
    // Zloop = V/I = 0.5/1.0 = 0.5+j0 ⇒ inside the (1+j0) reach.
    mon.vph = vec![Complex64::new(0.5, 0.0); 3];
    mon.iph = vec![Complex64::new(1.0, 0.0); 3];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
    assert!(
        r.relay_target.starts_with("21 "),
        "target = {}",
        r.relay_target
    );
    assert!(r.relay_target.contains("G1"));
}

#[test]
fn distance_no_trip_when_out_of_reach() {
    let mut r = distance_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    // Zloop = 2.0/1.0 = 2+j0 ⇒ beyond the (1+j0) reach.
    mon.vph = vec![Complex64::new(2.0, 0.0); 3];
    mon.iph = vec![Complex64::new(1.0, 0.0); 3];
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open);
}

/// `DistReverse` negates the monitored currents, flipping a forward in-reach
/// fault out of the positive-resistance characteristic ⇒ no trip.
#[test]
fn distance_reverse_negates_current() {
    let mut r = distance_relay();
    r.dist_reverse = true;
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(0.5, 0.0); 3];
    mon.iph = vec![Complex64::new(1.0, 0.0); 3]; // negated ⇒ Zloop.re < 0 ⇒ out
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(
        !r.armed_for_open,
        "reverse negation should drop the forward fault"
    );
}

// --- Deferred sub-types record NOT_PORTED -----------------------------------

#[test]
fn generic_and_td21_sample_record_not_ported() {
    for ty in [ctype::GENERIC, ctype::TD21] {
        let mut r = armed_relay();
        r.control_type = ty;
        let mut ctrl = MockElem::new(3);
        let mut mon = MockElem::new(3).with_current(10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(
            sc.errors.iter().any(|e| e.contains("NOT_PORTED")),
            "errors = {:?}",
            sc.errors
        );
        assert!(!r.armed_for_open);
    }
}

// --- MakeLike ---------------------------------------------------------------

#[test]
fn make_like_copies_settings_including_delay_and_breaker() {
    let mut base = Relay::new("base");
    base.control_type = ctype::DISTANCE;
    base.phase_trip = 700.0;
    base.reset_time = 22.0;
    base.num_reclose = 2;
    base.reclose_intervals[..2].copy_from_slice(&[0.5, 1.0]);
    base.normal_state = CTRL_OPEN;
    base.normal_state_set = true;
    base.delay_time = 0.3; // Relay MakeLike DOES copy this (unlike the Recloser)
    base.breaker_time = 0.04; // and this
    base.z1mag = 0.8;
    base.dist_reverse = true;
    base.phase_curve = Some(build_tcc("2", "1 10", "1 0.1"));

    let mut r = Relay::new("r1");
    r.make_like(&base);
    assert_eq!(r.control_type, ctype::DISTANCE);
    assert_eq!(r.phase_trip, 700.0);
    assert_eq!(r.reset_time, 22.0);
    assert_eq!(r.num_reclose, 2);
    assert_eq!(r.reclose_intervals[..2], [0.5, 1.0]);
    assert_eq!(r.normal_state, CTRL_OPEN);
    assert!(r.normal_state_set);
    assert_eq!(r.delay_time, 0.3);
    assert_eq!(r.breaker_time, 0.04);
    assert_eq!(r.z1mag, 0.8);
    assert!(r.dist_reverse);
    assert!(r.phase_curve.is_some());
}

// ---- executive-driven integration tests ----

fn dump(dss: &mut Dss, prop: &str) -> String {
    dss.command(&format!("? relay.r1.{prop}"));
    dss.result().to_string()
}

/// The default property dump (oracle-probed): SwitchedObj defaults to the
/// monitored element, Type=Current, Shots=4, RecloseIntervals=[ 0.5 2 2], and
/// Action/Normal/State render close/closed/closed.
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
    assert_eq!(dump(&mut dss, "Action"), "close");
    assert_eq!(dump(&mut dss, "Normal"), "closed");
    assert_eq!(dump(&mut dss, "State"), "closed");
}

/// `state=open` drives `FPresentState`, defaults `NormalState`, and (via
/// RecalcElementData) forces the controlled terminal open at parse time.
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
    assert_eq!(dump(&mut dss, "State"), "open");
    assert_eq!(dump(&mut dss, "Normal"), "open"); // defaulted from State
    assert!(
        line_term1_max_current(&mut dss, "Line.l2") < 1.0,
        "state=open should force l2 open at parse"
    );
    assert!(line_term1_max_current(&mut dss, "Line.l1") > 1.0);
}

/// End-to-end: an overcurrent relay on an overloaded line trips its controlled
/// terminal open, logging `Opened on Ph` and actually opening the line.
#[test]
fn relay_trips_overloaded_line() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=5000",
        // A Relay's PhaseCurve defaults to NIL, so use the definite-time path
        // (Delay>0, the IEEE13_CDPSM corpus shape). shots=1 ⇒ trips once and
        // locks out (no reclose), so the line stays open through the run.
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

/// `recloseintervals=NONE` (the AllowNone parse) clears the array ⇒ NumReclose
/// 0, Shots dumps 1, and RecloseIntervals dumps `[NONE]` (not `[]`).
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

/// Terminal-1 max current magnitude of a snapshot element (re/im interleaved).
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
