use super::*;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::control::control_elem::{ControlAction, RefSnapshot};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::exec::Dss;
use crate::solution::control_queue::TimeRec;
use crate::solution::{ControlMode, ControlQueue, EventLog, LoadSolutionModel, SolveMode};

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

/// A 1-terminal mock carrying a fixed per-phase current magnitude (monitored) and
/// per-conductor closed state (controlled). Phases are deliberately all equal (in
/// phase) so the residual sum is `nphases·imag` (a live ground path).
struct MockLine {
    cd: CktElementData,
    imag: f64,
}
impl MockLine {
    fn new(nphases: usize, imag: f64) -> Self {
        let mut cd = CktElementData::new("ln", 1);
        cd.nphases = nphases;
        cd.nconds = nphases;
        cd.set_nterms(1);
        cd.yorder = nphases;
        Self { cd, imag }
    }
}
impl CktElement for MockLine {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }
    fn calc_yprim(&mut self, _sys: &SysCtx) {}
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
        for c in curr.iter_mut().take(self.cd.nphases) {
            *c = Complex64::new(self.imag, 0.0);
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
    let mut errors = crate::diag::ErrorLog::new();
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

/// A 3-phase recloser with an explicit fast phase curve (r4133 defaults are
/// inert), `PhFastPickup=1`, its controlled element a 3-phase line.
fn armed_recloser() -> Recloser {
    let mut r = Recloser::new("r1");
    r.ph_fast = Some(build_tcc("2", "1 10", "1 0.1"));
    r.ph_slow = Some(build_tcc("2", "1 10", "2 0.2"));
    r.ph_fast_pickup = 1.0;
    r.ph_slow_pickup = 1.0;
    r.ccd.show_event_log = true; // exercise the r4133 event-log wording
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

/// The ganged operation slot (Pascal `IdxMultiPh = NPhases+1`).
const G: usize = 4;

#[test]
fn default_is_inert_3ph_closed_recloser() {
    let r = Recloser::new("r1");
    assert_eq!(r.ccd.cd.nphases, 3);
    assert_eq!(r.ccd.cd.nconds, 3);
    assert_eq!(r.ccd.cd.nterms, 1);
    assert_eq!(r.ccd.element_terminal, 1);
    // D2: default curves removed -> inert.
    assert_eq!(r.ph_fast_name, "none");
    assert_eq!(r.ph_slow_name, "none");
    assert!(r.ph_fast.is_none());
    assert_eq!(r.num_fast, 1);
    assert_eq!(r.num_reclose, 3); // Shots default 4
    assert_eq!(r.reset_time, 15.0);
    assert_eq!(r.reclose_intervals[..3], [0.5, 2.0, 2.0]);
    assert_eq!(r.present_state[1..=3], [ControlAction::Close; 3]);
    assert_eq!(r.normal_state[1..=3], [ControlAction::Close; 3]);
    assert!(!r.normal_state_set);
    assert_eq!(r.operation_count[1..=G], [1; 4]);
    assert_eq!(r.idx_multi_ph, G);
    // r4133 `ShowEventLog := EventLogDefault` (global False) — no override.
    assert!(!r.ccd.show_event_log);
    assert!(r.ccd.cd.yprim.is_none());
}

/// A default (no-curve) recloser never arms — the D2 breaking default.
#[test]
fn inert_default_recloser_never_arms() {
    let mut r = Recloser::new("r1");
    r.ctrl_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    r.ccd.controlled_element = Some(ElemId::new(0, 0));
    let mut ctrl = MockLine::new(3, 0.0);
    let mut mon = MockLine::new(3, 1000.0); // huge overcurrent
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(!r.armed_for_open[G], "no curves => inert => never arms");
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn sample_arms_open_then_reclose_on_overcurrent() {
    let mut r = armed_recloser();
    let mut ctrl = MockLine::new(3, 0.0); // closed
    let mut mon = MockLine::new(3, 10.0); // ratio 10 -> fast trip 0.1 s
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert!(r.armed_for_close[G]);
    assert!(r.phase_target[G]);
    // An OPEN action plus a reclose CLOSE (operation_count 1 <= NumReclose 3).
    assert_eq!(sc.queue.queue_size(), 2);
}

/// Pins the queued OPEN / reclose CLOSE times and the D3 inst-delay single-count:
/// `TripTime = TDPhFast·GetTCCTime(10) = 2·0.1 = 0.2`, OPEN at
/// `TripTime + MechanicalDelay = 0.25`, reclose at `+ RecloseIntervals[0] = 0.75`.
#[test]
fn sample_queues_trip_and_reclose_at_correct_times() {
    let mut r = armed_recloser();
    r.mechanical_delay = 0.05;
    r.td_ph_fast = 2.0;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut mon = MockLine::new(3, 10.0);
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

/// D3: an instantaneous trip fires at a bare `0.01` + one `MechanicalDelay` (not
/// the r4088 `0.01 + 2·delay`).
#[test]
fn inst_trip_single_counts_the_mechanical_delay() {
    let mut r = armed_recloser();
    r.ph_fast = Some(build_tcc("2", "100 200", "1 0.1")); // curve never picks up at 10 A
    r.ph_inst = 5.0; // 10 A >= 5 A
    r.mechanical_delay = 0.03;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut mon = MockLine::new(3, 10.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    let far = TimeRec {
        hour: 999,
        sec: 0.0,
    };
    let (open, t_open) = sc.queue.pop_time(far, false).unwrap();
    assert_eq!(open.code, ControlAction::Open.ordinal());
    // 0.01 (bare) + 0.03 (once) = 0.04.
    assert!((t_open - 0.04).abs() < 1e-9, "inst open time = {t_open}");
}

#[test]
fn sample_no_reclose_queued_when_no_shots_left() {
    let mut r = armed_recloser();
    r.operation_count[G] = 4; // > NumReclose 3 -> final trip, no reclose
    let mut ctrl = MockLine::new(3, 0.0);
    let mut mon = MockLine::new(3, 10.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert_eq!(sc.queue.queue_size(), 1); // only the OPEN, no reclose
}

#[test]
fn sample_uses_fast_then_slow_curve_by_operation_count() {
    // Slow curve never picks up at 10 A (c starts at 100); the fast one does.
    let make = || {
        let mut r = armed_recloser();
        r.ph_slow = Some(build_tcc("2", "100 200", "2 0.2"));
        r
    };
    {
        let mut r = make(); // operation_count 1 <= NumFast 1 -> fast -> trips
        let mut ctrl = MockLine::new(3, 0.0);
        let mut mon = MockLine::new(3, 10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(r.armed_for_open[G], "fast curve should trip at 10 A");
    }
    {
        let mut r = make();
        r.operation_count[G] = 2; // > NumFast 1 -> slow -> no pickup at 10 A
        let mut ctrl = MockLine::new(3, 0.0);
        let mut mon = MockLine::new(3, 10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(!r.armed_for_open[G], "slow curve pickup (100) not reached");
        assert_eq!(sc.queue.queue_size(), 0);
    }
}

#[test]
fn sample_ground_trip_on_residual_sum() {
    let mut r = armed_recloser();
    r.ph_fast = None; // isolate the ground path
    r.ph_slow = None;
    r.gnd_fast = Some(build_tcc("2", "1 10", "1 0.1"));
    r.gnd_fast_pickup = 1.0;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut mon = MockLine::new(3, 10.0); // residual sum = 30 A -> ratio 30
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[G]);
    assert!(r.ground_target);
    assert!(!r.phase_target[G]);
}

#[test]
fn sample_skips_when_all_phases_open() {
    let mut r = armed_recloser();
    let mut ctrl = MockLine::new(3, 0.0);
    ctrl.cd.set_terminal_closed(1, false); // controlled terminal open
    let mut mon = MockLine::new(3, 10.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert_eq!(r.present_state[1..=3], [ControlAction::Open; 3]);
    assert!(!r.armed_for_open[G]);
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn do_pending_open_ganged_trips_and_logs_3ph() {
    let mut r = armed_recloser();
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    {
        let mut mon = MockLine::new(3, 10.0);
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
        log_has(&sc, "OPENED ON PH FAST (3PH TRIP)"),
        "log = {:?}",
        sc.events.entries()
    );
    assert!(!r.locked_out[G]); // operation_count 1 <= NumReclose 3
}

#[test]
fn do_pending_open_ganged_locks_out_after_last_shot() {
    let mut r = armed_recloser();
    r.operation_count[G] = 4; // > NumReclose 3
    r.armed_for_open[G] = true;
    r.recloser_target[G] = "Ph Fast".to_string();
    let mut ctrl = MockLine::new(3, 0.0); // closed
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
fn do_pending_close_ganged_recloses_and_counts() {
    let mut r = armed_recloser();
    for i in 1..=3 {
        r.present_state[i] = ControlAction::Open; // the prior Sample saw the open terminal
    }
    r.armed_for_close[G] = true;
    r.operation_count[G] = 1;
    let mut ctrl = MockLine::new(3, 0.0);
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
fn do_pending_close_ganged_blocked_when_locked_out() {
    let mut r = armed_recloser();
    for i in 1..=3 {
        r.present_state[i] = ControlAction::Open;
    }
    r.armed_for_close[G] = true;
    r.locked_out[G] = true; // lockout blocks the reclose
    let mut ctrl = MockLine::new(3, 0.0);
    ctrl.cd.set_terminal_closed(1, false);
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Close.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // stays open
    // Pascal `Inc(OperationCount[PhIdx])` is UNCONDITIONAL in the ganged CLOSE
    // (outside the per-phase loop), so a blocked reclose still advances the count.
    assert_eq!(r.operation_count[G], 2);
}

#[test]
fn do_pending_reset_ganged_clears_operation_count_when_disarmed() {
    let mut r = armed_recloser();
    r.operation_count[G] = 3;
    r.armed_for_open[G] = false;
    let mut ctrl = MockLine::new(3, 0.0); // closed
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Reset.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert_eq!(r.operation_count[G], 1);
    assert!(log_has(&sc, "PHASE ALL RESET (3PH RESET)"));
}

#[test]
fn do_pending_reset_ganged_skipped_when_rearmed() {
    let mut r = armed_recloser();
    r.operation_count[G] = 3;
    r.armed_for_open[G] = true; // re-armed -> don't reset
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Reset.ordinal(),
        0,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert_eq!(r.operation_count[G], 3); // unchanged
}

// ---- single-phase trip machinery ----

#[test]
fn single_phase_trip_arms_only_the_faulted_phase() {
    let mut r = armed_recloser();
    r.single_ph_trip = true;
    // Only phase 1 carries the overcurrent; phases 2/3 are below pickup.
    let mut ctrl = MockLine::new(3, 0.0);
    // craft per-phase currents: phase 1 = 10 A, others 0.
    struct PerPhase {
        cd: CktElementData,
    }
    impl CktElement for PerPhase {
        fn cd(&self) -> &CktElementData {
            &self.cd
        }
        fn cd_mut(&mut self) -> &mut CktElementData {
            &mut self.cd
        }
        fn calc_yprim(&mut self, _s: &SysCtx) {}
        fn get_currents(&mut self, _s: &SysCtx, _v: &[Complex64], curr: &mut [Complex64]) {
            curr.fill(Complex64::ZERO);
            curr[0] = Complex64::new(10.0, 0.0); // phase 1 only
        }
    }
    let mut mon_pp = PerPhase {
        cd: {
            let mut cd = CktElementData::new("ln", 1);
            cd.nphases = 3;
            cd.nconds = 3;
            cd.set_nterms(1);
            cd.yorder = 3;
            cd
        },
    };
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon_pp, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open[1], "phase 1 should arm");
    assert!(!r.armed_for_open[2], "phase 2 below pickup");
    assert!(!r.armed_for_open[3], "phase 3 below pickup");
    // The queued OPEN carries the phase index in its proxy handle.
    let far = TimeRec {
        hour: 999,
        sec: 0.0,
    };
    let (open, _t) = sc.queue.pop_time(far, false).unwrap();
    assert_eq!(open.code, ControlAction::Open.ordinal());
    assert_eq!(open.proxy, 1, "single-phase OPEN carries phase index 1");
}

#[test]
fn single_phase_open_then_lockout_escalation() {
    // SinglePhTrip with SinglePhLockout=false -> a 1-ph final trip escalates to a
    // 3-ph lockout (opens the other phases too).
    let mut r = armed_recloser();
    r.single_ph_trip = true;
    r.single_ph_lockout = false;
    r.operation_count[1] = 4; // phase 1 exhausted its shots
    r.armed_for_open[1] = true;
    r.recloser_target[1] = "Ph Fast".to_string();
    let mut ctrl = MockLine::new(3, 0.0); // all closed
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Open.ordinal(),
        1,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert!(r.locked_out[1]);
    // Escalation opened & locked the other phases.
    assert!(!ctrl.cd.conductor_closed(1, 1));
    assert!(!ctrl.cd.conductor_closed(1, 2));
    assert!(!ctrl.cd.conductor_closed(1, 3));
    assert!(r.locked_out[2] && r.locked_out[3]);
    assert!(log_has(&sc, "LOCKED OUT (3PH LOCKOUT)"));
    assert!(log_has(&sc, "3PH LOCKOUT (1PH TRIP)"));
}

#[test]
fn single_phase_lockout_keeps_other_phases_closed() {
    // SinglePhLockout=true -> a 1-ph final trip locks out ONLY that phase.
    let mut r = armed_recloser();
    r.single_ph_trip = true;
    r.single_ph_lockout = true;
    r.operation_count[1] = 4;
    r.armed_for_open[1] = true;
    r.recloser_target[1] = "Ph Fast".to_string();
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    r.do_pending_action(
        ControlAction::Open.ordinal(),
        1,
        &mut ctrl,
        &mut sc.ctx(0, 0.0),
    );
    assert!(!ctrl.cd.conductor_closed(1, 1)); // phase 1 open
    assert!(ctrl.cd.conductor_closed(1, 2)); // phase 2 still closed
    assert!(ctrl.cd.conductor_closed(1, 3)); // phase 3 still closed
    assert!(r.locked_out[1]);
    assert!(!r.locked_out[2] && !r.locked_out[3]);
    assert!(log_has(&sc, "LOCKED OUT (1PH LOCKOUT)"));
}

#[test]
fn reset_with_restores_closed_normal_state() {
    let mut r = armed_recloser();
    for i in 1..=3 {
        r.normal_state[i] = ControlAction::Close;
        r.present_state[i] = ControlAction::Open;
    }
    r.operation_count[1] = 4;
    r.locked_out[1] = true;
    let mut ctrl = MockLine::new(3, 0.0);
    ctrl.cd.set_terminal_closed(1, false); // start open
    let rebuild = r.reset_with(&mut ctrl);
    assert!(rebuild);
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // restored closed
    assert_eq!(r.present_state[1..=3], [ControlAction::Close; 3]);
    assert!(!r.locked_out[1]);
    assert_eq!(r.operation_count[1], 1);
}

#[test]
fn reset_with_open_normal_state_locks_out() {
    let mut r = armed_recloser();
    for i in 1..=3 {
        r.normal_state[i] = ControlAction::Open;
    }
    let mut ctrl = MockLine::new(3, 0.0); // start closed
    let rebuild = r.reset_with(&mut ctrl);
    assert!(rebuild);
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // forced open
    assert_eq!(r.present_state[1..=3], [ControlAction::Open; 3]);
    assert!(r.locked_out[1] && r.locked_out[2] && r.locked_out[3]);
    assert_eq!(r.operation_count[1], r.num_reclose + 1);
}

/// A `Locked` recloser does not reset (Pascal `if not Locked`).
#[test]
fn locked_recloser_does_not_reset() {
    let mut r = armed_recloser();
    r.f_locked = true;
    for i in 1..=3 {
        r.normal_state[i] = ControlAction::Open;
        r.present_state[i] = ControlAction::Close;
    }
    let mut ctrl = MockLine::new(3, 0.0);
    let rebuild = r.reset_with(&mut ctrl);
    assert!(!rebuild, "a locked recloser must not force the element");
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // untouched
    assert_eq!(r.present_state[1..=3], [ControlAction::Close; 3]); // untouched
}

#[test]
fn make_like_copies_recloser_state_not_time_dials() {
    let mut base = Recloser::new("base");
    base.num_fast = 2;
    base.ph_fast_pickup = 700.0;
    base.ph_slow_pickup = 700.0;
    base.gnd_fast_pickup = 400.0;
    base.reset_time = 22.0;
    base.mechanical_delay = 0.9;
    base.num_reclose = 2;
    base.reclose_intervals[..2].copy_from_slice(&[0.5, 1.0]);
    base.single_ph_trip = true;
    base.rated_current = 630.0;
    for i in 1..=3 {
        base.normal_state[i] = ControlAction::Open;
    }
    base.td_ph_fast = 1.5; // NOT copied
    base.ph_fast = Some(build_tcc("2", "1 10", "1 0.1"));
    base.gnd_fast = Some(build_tcc("2", "5 50", "2 0.2"));

    let mut r = Recloser::new("r1");
    r.make_like(&base);
    assert_eq!(r.num_fast, 2);
    assert_eq!(r.ph_fast_pickup, 700.0);
    assert_eq!(r.gnd_fast_pickup, 400.0);
    assert_eq!(r.reset_time, 22.0);
    assert_eq!(r.mechanical_delay, 0.9); // r4133 DOES copy MechanicalDelay
    assert_eq!(r.num_reclose, 2);
    assert_eq!(r.reclose_intervals[..2], [0.5, 1.0]);
    assert!(r.single_ph_trip);
    assert_eq!(r.rated_current, 630.0);
    assert_eq!(r.normal_state[1..=3], [ControlAction::Open; 3]);
    // r4133 MakeLike does NOT copy NormalStateSet (only the state values).
    assert!(!r.normal_state_set);
    assert!(r.ph_fast.is_some());
    assert!(r.gnd_fast.is_some());
    assert!(r.ph_slow.is_none()); // base's was None -> stays None
    // Pascal MakeLike omits the TD* dials -> Create defaults.
    assert_eq!(r.td_ph_fast, 1.0);
}

// ---- executive-driven integration tests ----

fn dump(dss: &mut Dss, prop: &str) -> String {
    dss.command(&format!("? recloser.r1.{prop}"));
    dss.result().to_string()
}

/// The r4133 default property dump (oracle-probed on r4133): curves default to
/// `none` (inert), and `State`/`Normal` render the `[closed, closed, closed, ]`
/// per-phase form.
#[test]
fn default_dump_matches_r4133() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47",
        "new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
        "new recloser.r1 monitoredobj=line.l1 monitoredterm=1",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "PhFastCurve"), "none");
    assert_eq!(dump(&mut dss, "PhSlowCurve"), "none");
    assert_eq!(dump(&mut dss, "GndFastCurve"), "none");
    assert_eq!(dump(&mut dss, "Shots"), "4");
    assert_eq!(dump(&mut dss, "State"), "[closed, closed, closed, ]");
    assert_eq!(dump(&mut dss, "Normal"), "[closed, closed, closed, ]");
    assert_eq!(dump(&mut dss, "SinglePhTrip"), "No");
    assert_eq!(dump(&mut dss, "Lock"), "No");
    // Deprecated aliases resolve to the same fields.
    assert_eq!(dump(&mut dss, "PhaseFast"), "none");
}

/// Legacy alias parsing: `phasefast`/`phasetrip`/`delay`/`tdgrfast` set the same
/// state as the canonical `phfastcurve`/`phfastpickup`/`mechanicaldelay`/`tdgndfast`.
#[test]
fn legacy_alias_parsing_round_trips() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47",
        "new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
        "new tcc_curve.pf npts=2 c_array=[1,10] t_array=[1,0.1]",
        // Legacy names: phasefast/phasetrip/delay/tdgrfast/phaseinst.
        "new recloser.r1 monitoredobj=line.l1 phasefast=pf phasetrip=800 delay=0.1 tdgrfast=1.3 phaseinst=2000",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    // PhaseTrip sets BOTH fast & slow pickups (canonical readback).
    assert_eq!(
        dump(&mut dss, "PhFastPickup").parse::<f64>().unwrap(),
        800.0
    );
    assert_eq!(
        dump(&mut dss, "PhSlowPickup").parse::<f64>().unwrap(),
        800.0
    );
    assert_eq!(
        dump(&mut dss, "MechanicalDelay").parse::<f64>().unwrap(),
        0.1
    );
    assert_eq!(dump(&mut dss, "TDGndFast").parse::<f64>().unwrap(), 1.3);
    assert_eq!(dump(&mut dss, "PhInst").parse::<f64>().unwrap(), 2000.0);
    assert_eq!(dump(&mut dss, "PhFastCurve"), "pf");
}

/// `state=open` (ganged) forces every phase open at parse and defaults `Normal`.
#[test]
fn state_open_forces_all_phases_open_at_parse() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new line.l2 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=300",
        "new recloser.r1 monitoredobj=line.l1 switchedobj=line.l2 state=open",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "State"), "[open, open, open, ]");
    assert_eq!(dump(&mut dss, "Normal"), "[open, open, open, ]"); // defaulted from State
    assert!(
        line_term1_max_current(&mut dss, "Line.l2") < 1.0,
        "state=open should force l2 open at parse"
    );
    assert!(line_term1_max_current(&mut dss, "Line.l1") > 1.0);
}

/// End-to-end: an explicit-curve recloser on an overloaded line trips it open and
/// logs the r4133 per-phase wording.
#[test]
fn recloser_trips_overloaded_line() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=5000",
        // Explicit fast curve (r4133 has no default curves) + low pickup + a
        // single shot so the first trip locks out open deterministically.
        "new recloser.r1 monitoredobj=line.l1 phasefast=a phasetrip=1 shots=1 eventlog=yes",
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
        dss.event_log().iter().any(|s| s.contains("3PH TRIP")),
        "expected a 3ph trip; log = {:?}",
        dss.event_log()
    );
    assert!(
        line_term1_max_current(&mut dss, "Line.l1") < 1.0,
        "the tripped recloser should open the line"
    );
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
    fn resyncs_to_monitored() {
        let mut r = Recloser::new("r1");
        r.ccd.monitored_element = Some(ElemId::new(1, 0));
        r.monitored_element_terminal = 2;
        let ctx = PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                yorder: 2,
                bus_names: vec!["b1".into(), "b2".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = r.make_pos_sequence(&ctx);
        assert_eq!(r.ccd.cd.nphases, 1);
        assert_eq!(r.ccd.cd.nconds, 1);
        assert_eq!(r.get_bus_name(1), "b2");
        assert!(plan.run_base && plan.actions.is_empty());
        assert_eq!(r.monitored_element_ref(), Some(ElemId::new(1, 0)));
    }

    #[test]
    fn nil_monitored_runs_base_only() {
        let mut r = Recloser::new("r1");
        let np = r.ccd.cd.nphases;
        let plan = r.make_pos_sequence(&PosSeqCtx::default());
        assert_eq!(r.ccd.cd.nphases, np);
        assert!(plan.run_base);
    }
}
