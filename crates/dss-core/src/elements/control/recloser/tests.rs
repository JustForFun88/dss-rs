use super::*;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::control::control_elem::{CTRL_CLOSE, CTRL_OPEN, CTRL_RESET, RefSnapshot};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::exec::Dss;
use crate::obj::base::DssObject;
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
    }
}

/// A 1-terminal mock that carries a fixed per-phase current magnitude (the
/// monitored role) and per-conductor closed state (the controlled role). The
/// phases are deliberately all equal (not 120° apart) so the residual sum is
/// `nphases·imag`, which lets a ground-curve test see a non-zero sum.
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
    fn recalc_element_data(&mut self, _sys: &SysCtx) {}
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

/// A 3-phase recloser with a simple `c=[1,10] t=[1,0.1]` fast phase curve (and a
/// slower delayed one), rated `PhaseTrip=1`, its controlled element a 3-phase
/// line.
fn armed_recloser() -> Recloser {
    let mut r = Recloser::new("r1");
    r.phase_fast = Some(build_tcc("2", "1 10", "1 0.1"));
    r.phase_delayed = Some(build_tcc("2", "1 10", "2 0.2"));
    r.phase_trip = 1.0;
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
fn default_is_3ph_closed_recloser() {
    let r = Recloser::new("r1");
    assert_eq!(r.ccd.cd.nphases, 3);
    assert_eq!(r.ccd.cd.nconds, 3);
    assert_eq!(r.ccd.cd.nterms, 1);
    assert_eq!(r.ccd.element_terminal, 1);
    assert_eq!(r.monitored_element_terminal, 1);
    assert_eq!(r.phase_fast_name, "a");
    assert_eq!(r.phase_delayed_name, "d");
    assert_eq!(r.ground_fast_name, "");
    assert_eq!(r.num_fast, 1);
    assert_eq!(r.num_reclose, 3); // Shots default 4
    assert_eq!(r.reset_time, 15.0);
    assert_eq!(r.reclose_intervals[..3], [0.5, 2.0, 2.0]);
    assert_eq!(r.present_state, CTRL_CLOSE);
    assert_eq!(r.normal_state, CTRL_CLOSE);
    assert!(!r.normal_state_set);
    assert_eq!(r.operation_count, 1);
    assert!(r.ccd.cd.yprim.is_none());
}

#[test]
fn sample_arms_open_then_reclose_on_overcurrent() {
    let mut r = armed_recloser();
    let mut ctrl = MockLine::new(3, 0.0); // closed
    let mut mon = MockLine::new(3, 10.0); // 10 A → ratio 10 → fast trip 0.1 s
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
    assert!(r.armed_for_close);
    assert!(r.phase_target);
    // An OPEN action plus a reclose CLOSE (operation_count 1 ≤ NumReclose 3).
    assert_eq!(sc.queue.queue_size(), 2);
}

#[test]
fn sample_no_reclose_queued_when_no_shots_left() {
    let mut r = armed_recloser();
    r.operation_count = 4; // > NumReclose 3 → final trip, no reclose
    let mut ctrl = MockLine::new(3, 0.0);
    let mut mon = MockLine::new(3, 10.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
    assert_eq!(sc.queue.queue_size(), 1); // only the OPEN, no reclose
}

#[test]
fn sample_disarms_and_resets_when_current_drops() {
    let mut r = armed_recloser();
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    {
        let mut mon = MockLine::new(3, 10.0);
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    assert_eq!(sc.queue.queue_size(), 2);
    r.phase_target = true;
    // Current falls below pickup (ratio < 1) → disarm and queue a RESET.
    let mut mon = MockLine::new(3, 0.0);
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 1.0));
    assert!(!r.armed_for_open);
    assert!(!r.armed_for_close);
    assert!(!r.phase_target);
    // Pascal pushes a single CTRL_RESET (the prior two stay queued — Recloser
    // never deletes them, unlike the Fuse).
    assert_eq!(sc.queue.queue_size(), 3);
}

#[test]
fn sample_uses_fast_then_delayed_curve_by_operation_count() {
    // Delayed curve never picks up at 10 A (c starts at 100); the fast one does.
    let make = || {
        let mut r = armed_recloser();
        r.phase_delayed = Some(build_tcc("2", "100 200", "2 0.2"));
        r
    };
    // operation_count 1 ≤ NumFast 1 → fast curve → trips.
    {
        let mut r = make();
        let mut ctrl = MockLine::new(3, 0.0);
        let mut mon = MockLine::new(3, 10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(r.armed_for_open, "fast curve should trip at 10 A");
    }
    // operation_count 2 > NumFast 1 → delayed curve → no pickup at 10 A.
    {
        let mut r = make();
        r.operation_count = 2;
        let mut ctrl = MockLine::new(3, 0.0);
        let mut mon = MockLine::new(3, 10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(!r.armed_for_open, "delayed curve pickup (100) not reached");
        assert_eq!(sc.queue.queue_size(), 0);
    }
}

#[test]
fn sample_phase_inst_trips_on_first_operation_only() {
    // The phase curve never picks up (c starts at 100); only the instantaneous
    // element can arm, and only on operation_count == 1.
    let make = || {
        let mut r = armed_recloser();
        // Neither the fast nor the delayed curve picks up at 10 A (c starts at
        // 100), so only the instantaneous element can arm.
        r.phase_fast = Some(build_tcc("2", "100 200", "1 0.1"));
        r.phase_delayed = Some(build_tcc("2", "100 200", "2 0.2"));
        r.phase_inst = 5.0; // 10 A ≥ 5 A
        r
    };
    {
        let mut r = make();
        let mut ctrl = MockLine::new(3, 0.0);
        let mut mon = MockLine::new(3, 10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(r.armed_for_open, "inst should trip on the first operation");
    }
    {
        let mut r = make();
        r.operation_count = 2; // inst only fires when operation_count == 1
        let mut ctrl = MockLine::new(3, 0.0);
        let mut mon = MockLine::new(3, 10.0);
        let mut sc = Scratch::new();
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
        assert!(!r.armed_for_open, "inst is first-operation only");
    }
}

#[test]
fn sample_ground_trip_on_residual_sum() {
    let mut r = armed_recloser();
    r.phase_fast = None; // isolate the ground path
    r.phase_delayed = None;
    r.ground_fast = Some(build_tcc("2", "1 10", "1 0.1"));
    r.ground_trip = 1.0;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut mon = MockLine::new(3, 10.0); // residual sum = 30 A → ratio 30
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(r.armed_for_open);
    assert!(r.ground_target);
    assert!(!r.phase_target);
}

#[test]
fn sample_skips_when_terminal_open() {
    let mut r = armed_recloser();
    let mut ctrl = MockLine::new(3, 0.0);
    ctrl.cd.set_terminal_closed(1, false); // controlled terminal open
    let mut mon = MockLine::new(3, 10.0);
    let mut sc = Scratch::new();
    r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert_eq!(r.present_state, CTRL_OPEN);
    assert!(!r.armed_for_open);
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn do_pending_open_trips_logs_fast_and_target() {
    let mut r = armed_recloser();
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    {
        let mut mon = MockLine::new(3, 10.0);
        r.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    r.do_pending_action(CTRL_OPEN, &mut ctrl, &mut sc.ctx(0, 0.1));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // opened
    assert!(!r.armed_for_open);
    assert!(sc.y_changed);
    assert!(
        log_has(&sc, "OPENED, FAST"),
        "log = {:?}",
        sc.events.entries()
    );
    assert!(log_has(&sc, "PHASE TARGET"));
    assert!(!r.locked_out); // operation_count 1 ≤ NumReclose 3
}

#[test]
fn do_pending_open_locks_out_after_last_shot() {
    let mut r = armed_recloser();
    r.operation_count = 4; // > NumReclose 3
    r.armed_for_open = true;
    let mut ctrl = MockLine::new(3, 0.0); // closed
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_OPEN, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1));
    assert!(r.locked_out);
    assert!(log_has(&sc, "OPENED, LOCKED OUT"));
}

#[test]
fn do_pending_open_logs_delayed_after_numfast() {
    let mut r = armed_recloser();
    r.operation_count = 2; // > NumFast 1, ≤ NumReclose 3
    r.armed_for_open = true;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_OPEN, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(log_has(&sc, "OPENED, DELAYED"));
    assert!(!r.locked_out);
}

#[test]
fn do_pending_close_recloses_and_counts() {
    let mut r = armed_recloser();
    r.present_state = CTRL_OPEN; // the prior Sample saw the open terminal
    r.armed_for_close = true;
    r.operation_count = 1;
    let mut ctrl = MockLine::new(3, 0.0);
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
fn do_pending_close_blocked_when_locked_out() {
    let mut r = armed_recloser();
    r.present_state = CTRL_OPEN;
    r.armed_for_close = true;
    r.locked_out = true; // lockout blocks the reclose
    let mut ctrl = MockLine::new(3, 0.0);
    ctrl.cd.set_terminal_closed(1, false);
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_CLOSE, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // stays open
    assert_eq!(r.operation_count, 1);
}

#[test]
fn do_pending_reset_clears_operation_count_when_disarmed() {
    let mut r = armed_recloser();
    r.operation_count = 3;
    r.armed_for_open = false;
    let mut ctrl = MockLine::new(3, 0.0); // closed → present_state CLOSE in DoPendingAction? no: read happens in Sample
    let mut sc = Scratch::new();
    // present_state must be CLOSE for RESET to fire; the Sample-read happens in
    // the loop, here we set it explicitly.
    r.present_state = CTRL_CLOSE;
    r.do_pending_action(CTRL_RESET, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert_eq!(r.operation_count, 1);
}

#[test]
fn do_pending_reset_skipped_when_rearmed() {
    let mut r = armed_recloser();
    r.operation_count = 3;
    r.armed_for_open = true; // re-armed → don't reset
    r.present_state = CTRL_CLOSE;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    r.do_pending_action(CTRL_RESET, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert_eq!(r.operation_count, 3); // unchanged
}

#[test]
fn reset_with_restores_closed_normal_state() {
    let mut r = armed_recloser();
    r.normal_state = CTRL_CLOSE;
    r.present_state = CTRL_OPEN;
    r.operation_count = 4;
    r.locked_out = true;
    let mut ctrl = MockLine::new(3, 0.0);
    ctrl.cd.set_terminal_closed(1, false); // start open
    let rebuild = r.reset_with(&mut ctrl);
    assert!(rebuild);
    assert!(ctrl.cd.terminal_all_phases_closed(1)); // restored closed
    assert_eq!(r.present_state, CTRL_CLOSE);
    assert!(!r.locked_out);
    assert_eq!(r.operation_count, 1);
}

#[test]
fn reset_with_open_normal_state_locks_out() {
    let mut r = armed_recloser();
    r.normal_state = CTRL_OPEN;
    let mut ctrl = MockLine::new(3, 0.0); // start closed
    let rebuild = r.reset_with(&mut ctrl);
    assert!(rebuild);
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // forced open
    assert_eq!(r.present_state, CTRL_OPEN);
    assert!(r.locked_out);
    assert_eq!(r.operation_count, r.num_reclose + 1);
}

#[test]
fn make_like_copies_recloser_state_not_time_dials() {
    let mut base = Recloser::new("base");
    base.num_fast = 2;
    base.phase_trip = 700.0;
    base.ground_trip = 400.0;
    base.reset_time = 22.0;
    base.num_reclose = 2;
    base.reclose_intervals[..2].copy_from_slice(&[0.5, 1.0]);
    base.normal_state = CTRL_OPEN;
    base.normal_state_set = true;
    base.delay_time = 0.9; // NOT copied
    base.td_ph_fast = 1.5; // NOT copied

    let mut r = Recloser::new("r1");
    r.make_like(&base);
    assert_eq!(r.num_fast, 2);
    assert_eq!(r.phase_trip, 700.0);
    assert_eq!(r.ground_trip, 400.0);
    assert_eq!(r.reset_time, 22.0);
    assert_eq!(r.num_reclose, 2);
    assert_eq!(r.reclose_intervals[..2], [0.5, 1.0]);
    assert_eq!(r.normal_state, CTRL_OPEN);
    assert!(r.normal_state_set);
    // Pascal MakeLike omits DelayTime and the TD* dials → Create defaults.
    assert_eq!(r.delay_time, 0.0);
    assert_eq!(r.td_ph_fast, 1.0);
}

// ---- executive-driven integration tests ----

fn dump(dss: &mut Dss, prop: &str) -> String {
    dss.command(&format!("? recloser.r1.{prop}"));
    dss.result().to_string()
}

/// The default property dump (oracle-probed): the curves default to the built-in
/// `a`/`d`, `Shots=4` (NumReclose 3), `RecloseIntervals=[ 0.5 2 2]`, and
/// Action/Normal/State render `close`/`closed`/`closed`. SwitchedObj defaults to
/// the monitored element.
#[test]
fn default_dump_matches_oracle() {
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
    assert_eq!(dump(&mut dss, "SwitchedObj"), "Line.l1");
    assert_eq!(dump(&mut dss, "PhaseFast"), "a");
    assert_eq!(dump(&mut dss, "PhaseDelayed"), "d");
    assert_eq!(dump(&mut dss, "GroundFast"), "");
    assert_eq!(dump(&mut dss, "Shots"), "4");
    assert_eq!(dump(&mut dss, "RecloseIntervals"), "[ 0.5 2 2]");
    assert_eq!(dump(&mut dss, "Action"), "close");
    assert_eq!(dump(&mut dss, "Normal"), "closed");
    assert_eq!(dump(&mut dss, "State"), "closed");
}

/// `Shots` and `RecloseIntervals` both write `NumReclose`; the last one wins
/// (oracle: `shots=2 recloseintervals=(1 3)` ⇒ Shots=3, RecloseIntervals=[ 1 3]).
#[test]
fn shots_and_reclose_intervals_alias_num_reclose() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47",
        "new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
        "new recloser.r1 monitoredobj=line.l1 shots=2 recloseintervals=(1.0 3.0)",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert_eq!(dump(&mut dss, "Shots"), "3");
    assert_eq!(dump(&mut dss, "RecloseIntervals"), "[ 1 3]");
    // shots=1 ⇒ NumReclose=0 ⇒ empty array.
    dss.command("new recloser.r2 monitoredobj=line.l1 shots=1");
    dss.command("? recloser.r2.recloseintervals");
    assert_eq!(dss.result(), "[]");
}

/// `state=open` (and the `trip` alias) drive `FPresentState`, default
/// `NormalState`, and (via RecalcElementData) force the controlled terminal open.
#[test]
fn state_open_forces_controlled_terminal_open_at_parse() {
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
    assert_eq!(dump(&mut dss, "State"), "open");
    assert_eq!(dump(&mut dss, "Normal"), "open"); // defaulted from State
    assert!(
        line_term1_max_current(&mut dss, "Line.l2") < 1.0,
        "state=open should force l2 open at parse"
    );
    assert!(line_term1_max_current(&mut dss, "Line.l1") > 1.0);
}

/// End-to-end: a recloser on an overloaded line trips its controlled terminal
/// open on overcurrent (the default `a` fast curve), logging `Opened, Fast`.
#[test]
fn recloser_trips_overloaded_line() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=5000",
        // PhaseTrip low → the real line current is a large overcurrent multiple.
        "new recloser.r1 monitoredobj=line.l1 phasetrip=1",
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
        dss.event_log().iter().any(|s| s.contains("OPENED")),
        "expected a trip; log = {:?}",
        dss.event_log()
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
        let re = s.currents[2 * k];
        let im = s.currents[2 * k + 1];
        m = m.max((re * re + im * im).sqrt());
    }
    m
}
