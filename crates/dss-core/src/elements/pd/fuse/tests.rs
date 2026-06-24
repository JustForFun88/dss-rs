use super::*;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::control::control_elem::{CTRL_CLOSE, CTRL_OPEN};
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
        time_of_day: 0.0,
        dyna_h: 0.0,
    }
}

/// A 1-terminal mock that carries a fixed per-phase current magnitude (the
/// monitored role) and per-conductor closed state (the controlled role).
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

/// Build a populated `TccCurveObj` through the property engine (the fields are
/// private to the tcc_curve module).
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

/// A 3-phase fuse armed with a simple `c=[1,10] t=[1,0.1]` link and rated 1 A,
/// its controlled element a 3-phase line.
fn armed_fuse() -> Fuse {
    let mut f = Fuse::new("f1");
    f.fuse_curve_obj = Some(build_tcc("2", "1 10", "1 0.1"));
    f.rated_current = 1.0;
    f.ctrl_snap = Some(crate::elements::control::control_elem::RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    f.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    f
}

#[test]
fn default_is_3ph_closed_fuse() {
    let f = Fuse::new("f1");
    assert_eq!(f.ccd.cd.nphases, 3);
    assert_eq!(f.ccd.cd.nconds, 3);
    assert_eq!(f.ccd.cd.nterms, 1);
    assert_eq!(f.ccd.element_terminal, 1);
    assert_eq!(f.monitored_element_terminal, 1);
    assert_eq!(f.fuse_curve_name, "tlink");
    assert_eq!(f.rated_current, 1.0);
    assert_eq!(f.delay_time, 0.0);
    assert!(f.present_state.iter().all(|&s| s == CTRL_CLOSE));
    assert!(f.normal_state.iter().all(|&s| s == CTRL_CLOSE));
    assert!(!f.normal_state_set);
    assert!(f.ccd.cd.yprim.is_none());
}

#[test]
fn sample_arms_each_overcurrent_phase() {
    let mut f = armed_fuse();
    f.ccd.cd.nphases = 3;
    let mut ctrl = MockLine::new(3, 0.0); // closed by default
    let mut mon = MockLine::new(3, 10.0); // 10 A → ratio 10 → trip_time 0.1 s
    let mut sc = Scratch::new();
    f.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    // Each of the three closed phases arms a queued open action.
    assert!(f.ready_to_blow[..3].iter().all(|&b| b));
    assert_eq!(sc.queue.queue_size(), 3);
    assert!(f.present_state[..3].iter().all(|&s| s == CTRL_CLOSE));
}

#[test]
fn sample_disarms_when_current_drops_below_pickup() {
    let mut f = armed_fuse();
    f.ccd.cd.nphases = 3;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    // Arm with a 10 A overcurrent.
    {
        let mut mon = MockLine::new(3, 10.0);
        f.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    assert_eq!(sc.queue.queue_size(), 3);
    // Current falls below the first curve point (ratio < 1) → disarm + delete.
    let mut mon = MockLine::new(3, 0.0);
    f.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 1.0));
    assert!(f.ready_to_blow[..3].iter().all(|&b| !b));
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn sample_divides_current_by_rated_current() {
    // Pins the `Cmag / RatedCurrent` divisor: 5 A at rated 10 A is a 0.5 pu
    // multiple — below the first curve point (c[0] = 1) → no operation. A dropped
    // or inverted divisor would push the multiple ≥ 1 and (wrongly) arm.
    let mut f = armed_fuse();
    f.ccd.cd.nphases = 3;
    f.rated_current = 10.0;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut mon = MockLine::new(3, 5.0); // 5 A / 10 A = 0.5 pu < pickup
    let mut sc = Scratch::new();
    f.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    assert!(f.ready_to_blow[..3].iter().all(|&b| !b));
    assert_eq!(sc.queue.queue_size(), 0);
}

#[test]
fn do_pending_action_blows_one_phase_and_logs() {
    let mut f = armed_fuse();
    f.ccd.cd.nphases = 3;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    {
        let mut mon = MockLine::new(3, 10.0);
        f.sample(&mut ctrl, &mut mon, &mut sc.ctx(0, 0.0));
    }
    // Blow phase 2 (the queue `code`).
    f.do_pending_action(2, &mut ctrl, &mut sc.ctx(0, 0.1));
    assert!(!ctrl.cd.conductor_closed(1, 2)); // phase 2 opened
    assert!(ctrl.cd.conductor_closed(1, 1)); // phases 1, 3 still closed
    assert!(ctrl.cd.conductor_closed(1, 3));
    assert_eq!(f.h_action[1], 0); // handle cleared
    assert!(sc.y_changed); // conductor flip raises SystemYChanged
    // The full normalized line (the event log uppercases the action and names the
    // controlling element — Pascal AppendtoEventLog).
    assert!(
        sc.events
            .entries()
            .iter()
            .any(|e| e.contains("Element=Fuse.f1, Action=PHASE 2 BLOWN")),
        "log = {:?}",
        sc.events.entries()
    );
}

#[test]
fn do_pending_action_ignores_disarmed_phase() {
    let mut f = armed_fuse();
    f.ccd.cd.nphases = 3;
    let mut ctrl = MockLine::new(3, 0.0);
    let mut sc = Scratch::new();
    // Never armed (no sample): DoPendingAction must be a no-op.
    f.do_pending_action(1, &mut ctrl, &mut sc.ctx(0, 0.0));
    assert!(ctrl.cd.conductor_closed(1, 1)); // untouched
    assert!(!sc.y_changed);
    assert_eq!(sc.events.entries().len(), 0);
}

#[test]
fn reset_with_restores_each_phase_to_normal() {
    let mut f = armed_fuse();
    f.normal_state[0] = CTRL_CLOSE;
    f.normal_state[1] = CTRL_OPEN; // phase 2 normally open
    f.normal_state[2] = CTRL_CLOSE;
    let mut ctrl = MockLine::new(3, 0.0);
    ctrl.cd.set_terminal_closed(1, false); // start fully blown
    let rebuild = f.reset_with(&mut ctrl);
    assert!(rebuild);
    assert!(ctrl.cd.conductor_closed(1, 1)); // restored closed
    assert!(!ctrl.cd.conductor_closed(1, 2)); // normal-open stays open
    assert!(ctrl.cd.conductor_closed(1, 3));
    assert_eq!(f.present_state[0], CTRL_CLOSE);
    assert_eq!(f.present_state[1], CTRL_OPEN);
    assert!(f.ready_to_blow[..3].iter().all(|&b| !b));
}

/// Fail-on-regression guard for the WP7.2 step-2a dirty edge. The seed is chosen
/// so the all-or-nothing aggregate `terminal_all_phases_closed` reads **the same
/// (false) before and after** the reset, yet real phases flip — exactly where a
/// reintroduced `was_all_closed != want_all_closed` gate would compute
/// `false != false` and *skip* the rebuild, reusing a stale system Y:
///   terminal `[open, closed, closed]` (aggregate false) →
///   normal `[CLOSE, OPEN, CLOSE]` ⇒ `[closed, open, closed]` (aggregate still false),
/// while phase 0 (open→closed) and phase 1 (closed→open) actually change.
/// `reset_with` must report the rebuild unconditionally; reintroducing the
/// aggregate gate makes this test fail (the earlier "all-closed target" seed did
/// not — `was(false) != want(true)` matched the correct result).
#[test]
fn reset_with_partial_open_terminal_still_forces_rebuild() {
    let mut f = armed_fuse();
    f.normal_state[..3].copy_from_slice(&[CTRL_CLOSE, CTRL_OPEN, CTRL_CLOSE]);
    let mut ctrl = MockLine::new(3, 0.0);
    ctrl.cd.terminals[0].conductors_closed[0] = false; // phase 0 blown
    ctrl.cd.terminals[0].conductors_closed[1] = true; // phase 1 closed
    ctrl.cd.terminals[0].conductors_closed[2] = true; // phase 2 closed
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // aggregate false before

    let rebuild = f.reset_with(&mut ctrl);
    assert!(
        rebuild,
        "reset must force a Y rebuild even when the aggregate is unchanged"
    );
    // The aggregate is still false after, but real phases flipped.
    assert!(!ctrl.cd.terminal_all_phases_closed(1)); // aggregate false after too
    assert!(ctrl.cd.conductor_closed(1, 1)); // phase 0: open → closed (the missed change)
    assert!(!ctrl.cd.conductor_closed(1, 2)); // phase 1: closed → open
}

#[test]
fn state_array_short_input_sets_leading_phases() {
    // `state=[open]` on a 3-phase fuse opens only phase 1 (per-conductor), the
    // rest keep their default closed (oracle-probed: `[open, closed, closed, ]`).
    let mut f = Fuse::new("f1");
    f.ccd.cd.nphases = 3;
    f.ctrl_snap = Some(crate::elements::control::control_elem::RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    f.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    f.set_enum_array(prop::STATE, &[CTRL_OPEN]); // only phase 1
    f.state_side_effect();
    assert_eq!(f.present_state[0], CTRL_OPEN);
    assert_eq!(f.present_state[1], CTRL_CLOSE);
    assert_eq!(f.present_state[2], CTRL_CLOSE);
    // The deferred per-conductor force matches the present state.
    let actions = f.take_ref_actions();
    assert_eq!(actions.len(), 1);
    let crate::obj::base::RefAction::SetConductorsClosed {
        closed, terminal, ..
    } = &actions[0]
    else {
        panic!("expected SetConductorsClosed");
    };
    assert_eq!(*terminal, 1);
    assert_eq!(closed, &vec![false, true, true]);
}

#[test]
fn make_like_copies_fuse_state() {
    let mut base = Fuse::new("base");
    base.ccd.cd.nphases = 1;
    base.ccd.cd.set_nconds(1);
    base.ccd.element_terminal = 2;
    base.monitored_element_terminal = 2;
    base.rated_current = 25.0;
    base.delay_time = 0.5;
    base.fuse_curve_name = "klink".into();
    base.ctrl_snap = Some(crate::elements::control::control_elem::RefSnapshot {
        full_name: "Line.l9".into(),
        nphases: 1,
        nterms: 1,
        buses: vec!["x".into()],
    });
    base.present_state[0] = CTRL_OPEN;
    base.normal_state[0] = CTRL_OPEN;

    let mut f = Fuse::new("f1");
    f.make_like(&base);
    assert_eq!(f.ccd.cd.nphases, 1);
    assert_eq!(f.ccd.element_terminal, 2);
    assert_eq!(f.monitored_element_terminal, 2);
    assert_eq!(f.rated_current, 25.0);
    // Pascal `TFuseObj.MakeLike` copies neither `DelayTime` — it stays at the
    // Create default — nor `NormalStateSet`.
    assert_eq!(f.delay_time, 0.0);
    assert_eq!(f.fuse_curve_name, "klink");
    assert_eq!(f.present_state[0], CTRL_OPEN);
    assert_eq!(f.normal_state[0], CTRL_OPEN);
}

// ---- executive-driven integration tests ----

/// The default `Normal`/`State` dump is a per-phase enum array sized by the
/// controlled element's phase count: `[closed, closed, closed, ]` (oracle).
#[test]
fn default_state_arrays_dump_per_phase() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47",
        "new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1",
        "new fuse.f1 monitoredobj=line.l1",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    let dump = |dss: &mut Dss, prop: &str| {
        dss.command(&format!("? fuse.f1.{prop}"));
        dss.result().to_string()
    };
    assert_eq!(dump(&mut dss, "Normal"), "[closed, closed, closed, ]");
    assert_eq!(dump(&mut dss, "State"), "[closed, closed, closed, ]");
    // The default fuse link resolves to the built-in `tlink` curve.
    assert_eq!(dump(&mut dss, "FuseCurve"), "tlink");
    // SwitchedObj defaults to the monitored element.
    assert_eq!(dump(&mut dss, "SwitchedObj"), "Line.l1");
}

/// A fuse with no monitored/switched element raises Pascal error 405 ("CktElement
/// for SwitchedObj is not set") at EndEdit (oracle-confirmed `#405`).
#[test]
fn bare_fuse_reports_missing_switched_element() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t basekv=12.47");
    dss.command("new fuse.f1");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("CktElement for SwitchedObj is not set")),
        "errors = {:?}",
        dss.errors()
    );
}

/// `state=[open]` forces only the named controlled conductor open at parse time
/// (per-phase `SetConductorsClosed`), so a snapshot solve already shows phase 1
/// open while phases 2,3 still carry current.
#[test]
fn state_open_forces_controlled_phase_open_at_parse() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new line.l2 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=300",
        "new fuse.f1 monitoredobj=line.l1 state=[open,open,open]",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    assert!(
        line_term1_max_current(&mut dss, "Line.l1") < 1.0,
        "state=open should force l1 open at parse"
    );
    assert!(line_term1_max_current(&mut dss, "Line.l2") > 1.0);
}

/// End-to-end: a fuse on an overloaded line blows its phases on overcurrent,
/// opening the line (event log `Phase N Blown`, oracle-probed message).
#[test]
fn fuse_blows_phases_on_overcurrent() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=5000",
        // rated 1 A → a huge overcurrent ratio → trips on the tlink tail.
        "new fuse.f1 monitoredobj=line.l1 ratedcurrent=1",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "set controlmode=time",
        "set mode=duty number=5 stepsize=1 hour=0",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "errors: {:?}", dss.errors());
    let blown = dss.event_log().iter().any(|s| s.contains("PHASE 1 BLOWN"));
    assert!(blown, "expected a blown phase; log = {:?}", dss.event_log());
    // The line is opened by the blown fuse → its through-current collapses.
    assert!(
        line_term1_max_current(&mut dss, "Line.l1") < 1.0,
        "blown fuse should open the line"
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
