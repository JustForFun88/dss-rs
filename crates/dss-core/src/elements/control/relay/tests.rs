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
    /// State variables (name, value) exposed as a PC element (Generic relay).
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
    /// Equal real current on every phase (residual sum = nphases·mag).
    fn with_current(mut self, mag: f64) -> Self {
        for c in self.iph.iter_mut() {
            *c = Complex64::new(mag, 0.0);
        }
        self
    }
    /// Add a named state variable (for the Generic relay).
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

// --- Generic (PC state-variable relay) --------------------------------------

/// Pascal `LookupVariable`: case-insensitive *prefix* match, 1-based, −1 if none.
#[test]
fn lookup_variable_prefix_match() {
    let names = vec!["Frequency".to_string(), "Vd".to_string()];
    assert_eq!(Relay::lookup_variable(&names, "frequency"), 1); // full, case-insensitive
    assert_eq!(Relay::lookup_variable(&names, "freq"), 1); // prefix
    assert_eq!(Relay::lookup_variable(&names, "VD"), 2);
    assert_eq!(Relay::lookup_variable(&names, "theta"), -1); // absent
    assert_eq!(Relay::lookup_variable(&names, "frequencyX"), -1); // longer than name
}

/// A Generic relay reading state variable 1, band `[0.8, 1.2]`.
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
    let mut mon = MockElem::new(3).with_var("Frequency", 1.5); // > 1.2
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
    assert_eq!(r.relay_target, "Frequency"); // VariableName(MonitorVarIndex)
    assert_eq!(r.operation_count, r.num_reclose + 1); // one-shot lockout
    assert_eq!(sc.queue.queue_size(), 1); // OPEN only (no reclose)
}

#[test]
fn generic_trips_below_undertrip() {
    let mut r = generic_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3).with_var("Frequency", 0.5); // < 0.8
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
}

#[test]
fn generic_no_trip_within_band() {
    let mut r = generic_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3).with_var("Frequency", 1.0); // in [0.8, 1.2]
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open);
    assert_eq!(sc.queue.queue_size(), 0);
}

/// `recalc` resolves `MonitorVarIndex` from the captured variable names, erroring
/// 386 when the named variable is absent.
#[test]
fn generic_recalc_resolves_and_errors_on_missing_var() {
    // Present: resolves to index 1.
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

    // Absent: index < 1 and an error 386 is recorded.
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
    assert!(
        errs.iter().any(|e| e.contains("386")),
        "expected error 386, got {errs:?}"
    );
    // Error 386 uses `DoSimpleMsg` → record-only, NOT a solution abort.
    assert!(
        !r2.ccd.cd.obj.take_abort(),
        "error 386 (DoSimpleMsg) must not request a solution abort"
    );
}

/// `recalc` with a monitored terminal out of range records error 384 AND
/// requests a solution abort: Pascal `DoErrorMsg` (`Relay.pas:813`) sets
/// `SolutionAbort := True` (`DSSGlobals.pas:265`), unlike the `DoSimpleMsg`
/// errors 385/386 (record-only). The executive lifts the queued flag into
/// `Solution.SolutionAbort`.
#[test]
fn recalc_out_of_range_terminal_errors_384_and_requests_abort() {
    let mut r = Relay::new("r384");
    r.monitored_element_terminal = 5; // the monitored element has only 1 terminal
    r.mon_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    r.recalc();
    assert!(
        r.ccd.cd.obj.take_abort(),
        "error 384 (DoErrorMsg) must request a solution abort"
    );
    let errs = r.ccd.cd.obj.take_errors();
    assert!(
        errs.iter().any(|e| e.contains("384")),
        "expected error 384, got {errs:?}"
    );
}

/// `recalc` with no controlled (switched) element records error 387 AND
/// requests a solution abort: Pascal `DoErrorMsg` (`Relay.pas:889`) sets
/// `SolutionAbort := True` (`DSSGlobals.pas:265`) — same class as 384/388,
/// unlike the `DoSimpleMsg` errors 385/386 (record-only).
#[test]
fn recalc_missing_switched_element_errors_387_and_requests_abort() {
    // No monitored snapshot and no controlled element resolved: recalc skips
    // the mon block and hits the "SwitchedObj not set" branch.
    let mut r = Relay::new("r387");
    r.recalc();
    assert!(
        r.ccd.cd.obj.take_abort(),
        "error 387 (DoErrorMsg) must request a solution abort"
    );
    let errs = r.ccd.cd.obj.take_errors();
    assert!(
        errs.iter().any(|e| e.contains("387")),
        "expected error 387, got {errs:?}"
    );
}

// --- TD21 (differential time-distance, 21) ----------------------------------

/// A TD21 relay with a resistive reach (`Z1=1∠0`, `K0=0`, `M=1`), `PhaseTrip=1`.
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

/// The first dynamics `Sample` sizes the ring buffer: `round(1/60/0.001 + 0.5) =
/// 17` samples, stride `2·Nphases = 6`, quiet `pt + 1 = 18`.
#[test]
fn td21_allocates_ring_buffer_on_first_sample() {
    let mut r = td21_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3];
    mon.iph = vec![Complex64::new(0.1, 0.0); 3];
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.001; // dynamics step (Frequency 60)
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert_eq!(r.td21_pt, 17);
    assert_eq!(r.td21_stride, 6);
    assert_eq!(r.td21_h.len(), 17 * 6);
    assert_eq!(r.td21_quiet, 17); // 18, decremented once on this new-time-step
}

/// A TD21 relay sampled on a time step LARGER than one 60 Hz cycle (error 388)
/// requests a solution abort: Pascal `DoErrorMsg` (`Relay.pas:1460`) sets
/// `SolutionAbort := True` (`DSSGlobals.pas:265`). `Sample` returns the request;
/// the dispatch layer lifts it into `Solution.SolutionAbort` so the run halts
/// where the oracle halts (instead of solving on).
#[test]
fn td21_coarse_step_requests_solution_abort() {
    let mut r = td21_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3];
    mon.iph = vec![Complex64::new(0.1, 0.0); 3];
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.02; // > 1/60 (one cycle ≈ 0.0167 s) → error 388
    let abort = r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(abort, "coarse-step TD21 must request a solution abort");
    assert!(
        sc.errors.iter().any(|e| e.contains("388")),
        "expected error 388, got {:?}",
        sc.errors
    );
}

/// The same relay on a fine step (`dt <= 1/60`) records no error and does NOT
/// request an abort — the guard is a strict `>` (Pascal `dt > 1/Frequency`).
#[test]
fn td21_fine_step_does_not_request_abort() {
    let mut r = td21_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3];
    mon.iph = vec![Complex64::new(0.1, 0.0); 3];
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.001; // << one cycle → no error 388
    let abort = r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!abort, "fine-step TD21 must not request an abort");
    assert!(sc.errors.is_empty(), "unexpected errors: {:?}", sc.errors);
}

/// After a full pre-fault cycle (drains `td21_quiet`) the ring holds the
/// pre-fault reference; a forward differential fault then picks up and arms a
/// definite-time trip. Increments chosen so `Zdir=1+j0.5` (forward) and
/// `|Uhsd|²/|Uref|²≈1.48 > 1`.
#[test]
fn td21_trips_on_forward_differential_fault() {
    let mut r = td21_relay();
    let mut ctrl = MockElem::new(3);
    let mut mon = MockElem::new(3);
    mon.vph = vec![Complex64::new(10.0, 0.0); 3]; // pre-fault steady state
    mon.iph = vec![Complex64::new(0.1, 0.0); 3]; // below PhaseTrip ⇒ no fault
    let mut sc = Scratch::new();
    sc.sys.dyna_h = 0.001;

    // 18 pre-fault samples: fill the ring and drain quiet (18 → 0).
    for _ in 0..18 {
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    assert!(!r.armed_for_open, "no trip on the pre-fault steady state");
    assert_eq!(r.td21_quiet, 0);

    // Fault sample: V collapses+rotates, I rises above PhaseTrip.
    mon.vph = vec![Complex64::new(4.1, -2.95); 3];
    mon.iph = vec![Complex64::new(6.0, 0.0); 3];
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open, "forward differential fault must arm");
    assert!(
        r.relay_target.starts_with("TD21 "),
        "target = {}",
        r.relay_target
    );
    assert!(r.relay_target.contains("G1"), "target = {}", r.relay_target);
}

/// `Dist_Reverse` negates the monitored currents, so the same forward fault is
/// seen as reverse (`Zdir.re < 0`) and does not pick up.
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
    assert!(
        !r.armed_for_open,
        "reverse relay must not trip a forward fault"
    );
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

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemRef};
    use crate::obj::base::DssObject;

    /// Pascal `TRelayObj.MakePosSequence` (Relay.pas:915): monitored resync
    /// (incl. the Distance/TD21/DOC cvBuffer path) then the Vbase/PickupVolts47
    /// recompute. 1-phase → Vbase = kVBase·1000.
    #[test]
    fn resyncs_monitored_and_recomputes_vbase() {
        let mut r = Relay::new("r1");
        r.ccd.monitored_element = Some(ElemRef { cls: 1, idx: 0 });
        r.monitored_element_terminal = 1;
        r.kv_base = 12.47;
        r.pct_pickup47 = 2.0;
        r.control_type = ctype::DISTANCE; // exercises the cvBuffer branch
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
        assert!((r.vbase - 12_470.0).abs() < 1e-9); // 1-phase: kVBase·1000
        assert!((r.pickup_volts47 - 249.4).abs() < 1e-9);
        assert!(plan.run_base);
        assert_eq!(r.monitored_element_ref(), Some(ElemRef { cls: 1, idx: 0 }));
    }

    /// The Vbase/PickupVolts47 recompute sits OUTSIDE the NIL guard: with no
    /// monitored element the default 3-phase Vbase = kVBase/√3·1000 is written.
    #[test]
    fn vbase_recomputed_outside_nil_guard() {
        let mut r = Relay::new("r1"); // default nphases = 3
        r.kv_base = 12.47;
        r.pct_pickup47 = 2.0;
        r.make_pos_sequence(&PosSeqCtx::default());
        let expected = 12.47 / crate::util::sqrt3() * 1000.0;
        assert!((r.vbase - expected).abs() < 1e-9);
        assert!((r.pickup_volts47 - expected * 2.0 * 0.01).abs() < 1e-9);
    }
}
