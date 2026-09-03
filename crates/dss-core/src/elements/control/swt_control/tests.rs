//! RP3.7 A1 pin set: the per-phase state model and r4133
//! `InterpretSwitchState` mechanics, the per-phase drive of the controlled
//! element, the ganged/homogeneous equivalence with the pre-change behavior
//! (every corpus deck is ganged), and the scalar render invariance A2 flips.
//! Probe transcript: `tmp/rp37/probe.md`; every expected value cites the r4133
//! Pascal (`Version8/Source/Controls/SwtControl.pas`) line it re-derives from.

use super::*;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::control::control_elem::{ControlAction, RefSnapshot};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::exec::Dss;
use crate::obj::base::{DssObject, RefAction};
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
        ncim: false,
    }
}

/// A minimal switched element (a 2-terminal PD device) so the per-phase drive
/// and `DoPendingAction` have real terminals/conductors to flip.
struct MockSwitch {
    cd: CktElementData,
}
impl MockSwitch {
    fn new(nphases: usize) -> Self {
        let mut cd = CktElementData::new("sw", 1);
        cd.nphases = nphases;
        cd.nconds = nphases;
        cd.set_nterms(2);
        cd.yorder = 2 * nphases;
        Self { cd }
    }
}
impl CktElement for MockSwitch {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }
    fn calc_yprim(&mut self, _sys: &SysCtx) {}
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
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

/// A control wired to a `MockSwitch`-shaped snapshot so `recalc`'s per-phase
/// drive has a phase count to bound itself by (the probe's 3-phase default).
fn sw_with_snap(nphases: usize) -> SwtControl {
    let mut sw = SwtControl::new("sw1");
    sw.ccd.controlled_element = Some(ElemId::new(0, 0));
    sw.ctrl_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases,
        nterms: 1,
        buses: vec!["b".into()],
    });
    sw
}

/// The REAL property seam: [`ClassProps::edit_property`], i.e. `parse_into`
/// (which routes `Normal`/`State` through
/// [`DssObject::set_enum_array_raw`](crate::obj::base::DssObject::set_enum_array_raw)),
/// then `SetAsNextSeq`, then `PropertySideEffects` — carrying the outer
/// parser's `WasQuoted` flag exactly as the executive does
/// (`exec/command.rs:1428`). Nothing here bypasses the engine's own applier.
fn edit_prop(sw: &mut SwtControl, name: &str, value: &str, was_quoted: bool) {
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::PropEngine;
    use dss_parser::{Parser, ParserVars};
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let idx = cls
        .property_index(name)
        .unwrap_or_else(|| panic!("no prop {name}"));
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    let mut eng = PropEngine {
        parser: &mut parser,
        vars: &vars,
        enums: &enums,
        errors: &mut errors,
        foreign: None,
        was_quoted,
    };
    cls.edit_property(sw, idx, value, &mut eng)
        .unwrap_or_else(|e| panic!("{name}={value}: {e}"));
    assert!(errors.is_empty(), "{name}={value}: {:?}", errors.texts());
}

/// The rendered property bytes — [`ClassProps::get_value`], the one renderer
/// `?`, `Dump`, `Save` and the props golden all go through.
fn render(sw: &SwtControl, name: &str) -> String {
    use crate::obj::dss_enum::EnumRegistry;
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let idx = cls
        .property_index(name)
        .unwrap_or_else(|| panic!("no prop {name}"));
    cls.get_value(sw, idx, &enums)
}

// ---------------------------------------------------------------------------
// Create defaults (r4133 `TSwtControlObj.Create`, SwtControl.pas:279-315)
// ---------------------------------------------------------------------------

#[test]
fn default_is_3ph_all_closed_arrays() {
    // r4133 Create (:287-307): NPhases/Nconds 3, both state arrays all-CLOSED
    // (:302-305), NormalStateSet = FALSE (:307). Reverting the arrays to the
    // pre-RP3.7 scalar (present Close / normal None) breaks every slot assert.
    let sw = SwtControl::new("sw1");
    assert_eq!(sw.ccd.cd.nphases, 3);
    assert_eq!(sw.ccd.cd.nconds, 3);
    assert_eq!(sw.ccd.cd.nterms, 1);
    assert_eq!(sw.ccd.element_terminal, 1);
    assert_eq!(sw.ccd.time_delay, 120.0);
    assert_eq!(
        sw.present_state[1..=SW_MAX],
        [ControlAction::Close; SW_MAX],
        "present slots all closed"
    );
    assert_eq!(
        sw.normal_state[1..=SW_MAX],
        [ControlAction::Close; SW_MAX],
        "normal slots all closed"
    );
    assert!(!sw.normal_state_set);
    assert_eq!(sw.current_action, ControlAction::Close);
    assert!(!sw.locked);
    assert!(!sw.armed);
    assert!(sw.ccd.cd.yprim.is_none());
}

// ---------------------------------------------------------------------------
// The r4133 interpreter mechanics (InterpretSwitchState, SwtControl.pas:410-482)
// ---------------------------------------------------------------------------

#[test]
fn interpreter_quoted_list_writes_phase_by_phase() {
    // r4133 `state=(open, closed, closed)` (:453-480): one slot per token, in
    // order. Probe P1: render `[open, closed, closed, ]`, slots 4..6 keep
    // their prior value (all-closed from Create). Reverting to the scalar
    // model collapses the three slots into one value.
    let mut sw = SwtControl::new("sw1");
    sw.interpret_switch_state(SwtStateProp::State, "open, closed, closed", true);
    assert_eq!(sw.present_state[1], ControlAction::Open);
    assert_eq!(sw.present_state[2], ControlAction::Close);
    assert_eq!(sw.present_state[3], ControlAction::Close);
    for i in 4..=SW_MAX {
        assert_eq!(
            sw.present_state[i],
            ControlAction::Close,
            "unlisted slot {i}"
        );
    }
}

#[test]
fn interpreter_quoted_list_leaves_unlisted_slots_unchanged() {
    // A short list touches only its own slots (r4133 `While ... Do` stops when
    // the tokens run out, :461) — a ganged pre-write survives in slot 3.
    let mut sw = SwtControl::new("sw1");
    sw.interpret_switch_state(SwtStateProp::State, "open", false); // ganged all open
    sw.interpret_switch_state(SwtStateProp::State, "closed closed", true);
    assert_eq!(sw.present_state[1], ControlAction::Close);
    assert_eq!(sw.present_state[2], ControlAction::Close);
    assert_eq!(sw.present_state[3], ControlAction::Open, "slot 3 unlisted");
}

#[test]
fn interpreter_per_phase_caps_at_five_tokens() {
    // r4133 loop bound `i < SWTCONTROLMAXDIM` (:461): at most FIVE tokens are
    // honored, a 6th is silently dropped (probe §11.2 — the plan's reading of
    // :453-480 did not call this out). Reverting the bound to `<=` writes the
    // 6th slot and fails the final assert.
    let mut sw = SwtControl::new("sw1");
    sw.interpret_switch_state(
        SwtStateProp::State,
        "open, open, open, open, open, open",
        true,
    );
    assert_eq!(
        sw.present_state[1..=5],
        [ControlAction::Open; 5],
        "five tokens honored"
    );
    assert_eq!(
        sw.present_state[6],
        ControlAction::Close,
        "6th token dropped"
    );
}

#[test]
fn interpreter_tokens_match_first_char_only() {
    // r4133 `case LowerCase(DataStr2)[1] of 'o': CTRL_OPEN; 'c': CTRL_CLOSE`
    // with NO else arm (:464-467): any other first character leaves the slot
    // unchanged (probe §9: `state=bogus` silent, untouched).
    let mut sw = SwtControl::new("sw1");
    sw.interpret_switch_state(SwtStateProp::State, "o xyz c open bogus closed", true);
    assert_eq!(sw.present_state[1], ControlAction::Open);
    assert_eq!(
        sw.present_state[2],
        ControlAction::Close,
        "'xyz' leaves Create's closed"
    );
    assert_eq!(sw.present_state[3], ControlAction::Close);
    assert_eq!(sw.present_state[4], ControlAction::Open);
    assert_eq!(
        sw.present_state[5],
        ControlAction::Close,
        "'bogus' leaves the slot"
    );
    // 6th token ('closed') dropped by the 5-token cap — slot 6 untouched.
    assert_eq!(sw.present_state[6], ControlAction::Close);
}

#[test]
fn interpreter_ganged_bare_token_fills_all_six_slots() {
    // r4133 ganged path (:433-451): `for i := 1 to SWTCONTROLMAXDIM` — every
    // slot, not just the controlled element's phases. Probe P1 ganged
    // contrast: bare `state=open` → `[open, open, open, ]` and ALL conductors
    // zero.
    let mut sw = SwtControl::new("sw1");
    sw.interpret_switch_state(SwtStateProp::State, "open", false);
    assert_eq!(
        sw.present_state[1..=SW_MAX],
        [ControlAction::Open; SW_MAX],
        "ganged slots"
    );
    sw.interpret_switch_state(SwtStateProp::Normal, "closed", false);
    assert_eq!(
        sw.normal_state[1..=SW_MAX],
        [ControlAction::Close; SW_MAX],
        "ganged normal slots"
    );
}

#[test]
fn interpreter_action_is_always_ganged() {
    // r4133 (:419-429): `action` matches the WHOLE param's first character and
    // writes every slot — even when the value arrives quoted like a per-phase
    // list. Reverting to per-phase parsing for Action fails slot 2.
    let mut sw = SwtControl::new("sw1");
    sw.interpret_switch_state(SwtStateProp::Action, "open, closed, closed", true);
    assert_eq!(
        sw.present_state[1..=SW_MAX],
        [ControlAction::Open; SW_MAX],
        "action ganged slots"
    );
}

#[test]
fn interpreter_lock_guard_follows_the_property_name() {
    // r4133 (:416-417): `if Locked and ((LowerCase(property_name[1]) = 'a')
    // or (... = 's')) Then Exit` — "Only allowed to change normal state if
    // locked". Action/State writes exit untouched; Normal applies, ganged AND
    // quoted per-phase (probe P3: locked `normal=open` and
    // `normal=(open, closed, open)` both apply). RP3.7 A1 ports the guard as
    // r4133 reads it; the observable property seam keeps the scalar-era
    // all-three refusal until A2 re-points it (the `locked_ignores_*` pins
    // below). Probe §10 (a2), plan §RP3.7 acceptance.
    let mut sw = SwtControl::new("sw1");
    sw.locked = true;
    sw.interpret_switch_state(SwtStateProp::State, "open", false);
    sw.interpret_switch_state(SwtStateProp::Action, "open", true);
    assert_eq!(
        sw.present_state[1..=SW_MAX],
        [ControlAction::Close; SW_MAX],
        "locked state/action no-op"
    );
    // Normal passes the guard — ganged...
    sw.interpret_switch_state(SwtStateProp::Normal, "open", false);
    assert_eq!(
        sw.normal_state[1..=SW_MAX],
        [ControlAction::Open; SW_MAX],
        "locked normal ganged"
    );
    // ...and quoted per-phase (the guard keys on the NAME, not the value shape).
    sw.interpret_switch_state(SwtStateProp::Normal, "closed, open, closed", true);
    assert_eq!(sw.normal_state[1], ControlAction::Close);
    assert_eq!(sw.normal_state[2], ControlAction::Open);
    assert_eq!(sw.normal_state[3], ControlAction::Close);
}

// ---------------------------------------------------------------------------
// Per-phase application to the controlled element (RecalcElementData drive,
// SwtControl.pas:347-355)
// ---------------------------------------------------------------------------

/// Apply the control's deferred `RefAction`s to a `MockSwitch` exactly the way
/// the executive applies `SetConductorsClosed` (`exec/command.rs`).
fn apply_ref_actions(sw: &mut SwtControl, ms: &mut MockSwitch) {
    for action in sw.take_ref_actions() {
        match action {
            RefAction::SetConductorsClosed {
                terminal, closed, ..
            } => {
                for (i, &c) in closed.iter().enumerate() {
                    ms.cd_mut().set_conductor_closed(terminal, i + 1, c);
                }
            }
            a => panic!("unexpected ref action {a:?}"),
        }
    }
}

#[test]
fn per_phase_write_drives_each_conductor_of_the_controlled_element() {
    // r4133 RecalcElementData (:347-355) drives `ControlledElement.Closed[i]`
    // per phase from FPresentState at end of Edit. One open slot must open
    // exactly its own conductor — reverting the drive to the whole-terminal
    // `SetSwitchClosed` (pre-RP3.7) opens all three and fails the per-phase
    // asserts.
    let mut sw = sw_with_snap(3);
    let mut ms = MockSwitch::new(3);
    sw.interpret_switch_state(SwtStateProp::State, "open, closed, closed", true);
    sw.side_effects(prop::STATE, 0);
    sw.recalc();
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1, "recalc queues the end-of-Edit drive");
    match &actions[0] {
        RefAction::SetConductorsClosed {
            terminal, closed, ..
        } => {
            assert_eq!(*terminal, 1);
            assert_eq!(closed, &vec![false, true, true], "per-phase closed flags");
        }
        a => panic!("expected SetConductorsClosed, got {a:?}"),
    }
    // Re-queue (taken above) and apply like the executive.
    sw.recalc();
    apply_ref_actions(&mut sw, &mut ms);
    assert!(
        !ms.cd.conductor_closed(1, 1),
        "phase 1 open breaks its path"
    );
    assert!(ms.cd.conductor_closed(1, 2), "phase 2 stays closed");
    assert!(ms.cd.conductor_closed(1, 3), "phase 3 stays closed");
}

#[test]
fn recalc_redrives_after_a_phase_count_change() {
    // What this pin proves: the BOUND ARITHMETIC. `recalc` drives
    // `1..state_size()` off the controlled-element snapshot, so once the
    // snapshot says one phase the drive is one conductor while the array keeps
    // its other slots (the makeposseq shape, probe §6).
    //
    // What it does NOT prove (RP3.7 FIX-A1, verify-A1 finding F8): r4133's
    // LIVE re-read. `RecalcElementData` reads `ControlledElement.NPhases`
    // itself on every run (`SwtControl.pas:333/:347`); the port's stand-in is
    // `ctrl_snap`, refreshed when `switchedobj=` is (re-)resolved and by
    // `make_pos_sequence` — which is why this test has to assign the snapshot
    // by hand. The live-count path is covered end-to-end by
    // `the_render_bound_follows_makeposseq`; the residual staleness (a bare
    // `edit line.x phases=N` on the controlled element, with no touch of the
    // control, leaves the old bound) is a recorded, architecture-level
    // divergence shared with Relay — zero corpus exposure, no deck edits a
    // controlled element's phase count after wiring the control.
    let mut sw = sw_with_snap(3);
    let mut ms = MockSwitch::new(3);
    sw.interpret_switch_state(SwtStateProp::State, "open, closed, closed", true);
    sw.recalc();
    sw.take_ref_actions();

    // The controlled element's phase count changes (snapshot re-captured at
    // the next switchedobj= resolution, or the pos-seq sweep).
    sw.ctrl_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 1,
        nterms: 1,
        buses: vec!["b".into()],
    });
    sw.recalc();
    assert_eq!(sw.ccd.cd.nphases, 1, "control follows the element's count");
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        RefAction::SetConductorsClosed { closed, .. } => {
            assert_eq!(closed.len(), 1, "drive bounds at the new phase count");
            assert!(!closed[0], "slot 1 (open) still drives");
        }
        a => panic!("expected SetConductorsClosed, got {a:?}"),
    }
    // The per-phase array kept its state for a future phase-count restore.
    assert_eq!(sw.present_state[2], ControlAction::Close);
    assert_eq!(sw.present_state[3], ControlAction::Close);
    apply_ref_actions(&mut sw, &mut ms);
}

#[test]
fn reset_yes_drives_the_conductors_per_phase_from_normal() {
    // r4133 Reset (:625-646): per-phase Present := Normal + drive Closed from
    // Normal, `1..Min(6, ControlledElement.Nphases)`. Probe P3: after
    // `normal=(open, closed, open)` + `reset=yes`, phases 1 and 3 open, phase
    // 2 carries. At parse time the force is deferred (module-doc drive model).
    let mut sw = sw_with_snap(3);
    sw.interpret_switch_state(SwtStateProp::Normal, "open, closed, open", true);
    sw.side_effects(prop::NORMAL, 0);
    sw.set_bool(prop::RESET, true); // reset=yes: unlock + Reset
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        RefAction::SetConductorsClosed { closed, .. } => {
            assert_eq!(closed, &vec![false, true, false], "normal per phase");
        }
        a => panic!("expected SetConductorsClosed, got {a:?}"),
    }
    assert_eq!(
        sw.present_state[1],
        ControlAction::Open,
        "present follows normal"
    );
    assert_eq!(sw.present_state[2], ControlAction::Close);
    assert_eq!(sw.present_state[3], ControlAction::Open);
}

/// The probe's P1 micro-deck (`tmp/rp37/decks/p1_perphase.dss`): a 3-phase
/// switched line feeding a grounded-wye load — each phase is an independent
/// line-neutral path, so opening ONE conductor zeros exactly that phase's
/// current. RP3.7 A1 drives the r4133 mechanics on the arena object (A2 wires
/// them into the property parser) and applies the deferred force exactly like
/// the executive; the expected currents are the r4133 DLL's solved values
/// (`tmp/rp37/probe.md` §3): phase 1 = 0.0, phases 2/3
/// `-6.953452-12.035939j` / `-6.935750+12.030079j` (|I| 13.90016 / 13.88623).
#[test]
fn per_phase_open_zeros_only_its_phase_currents_on_a_micro_deck() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.p1 basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new line.feed bus1=src bus2=sw phases=3 r1=0.05 x1=0.05 c1=0 length=1 units=km",
        "new line.swk bus1=sw bus2=ld phases=3 r1=0.01 x1=0.01 c1=0 length=1 units=km",
        "new swtcontrol.sw1 switchedobj=line.swk switchedterm=1",
        "new load.ld bus1=ld.1.2.3.0 phases=3 conn=wye kv=12.47 kw=300 pf=1.0 model=1",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());

    // `edit swtcontrol.sw1 state=(open, closed, closed)` — the full r4133
    // Edit sequence: InterpretSwitchState (quoted → per-phase), the Edit
    // supplemental (Normal defaults to Present on the first State write,
    // :221-228), RecalcElementData's per-phase drive (:234 → :347-355).
    let closed = {
        let classes = dss.registered_classes_mut();
        let cls = classes
            .iter_mut()
            .find(|c| c.props.class_name() == "SwtControl")
            .unwrap();
        let &oi = cls.name_to_idx.get("sw1").unwrap();
        let sw = cls.arena.get_mut::<SwtControl>(oi).unwrap();
        sw.interpret_switch_state(SwtStateProp::State, "open, closed, closed", true);
        sw.side_effects(prop::STATE, 0);
        sw.recalc();
        let actions = sw.take_ref_actions();
        assert_eq!(actions.len(), 1, "the end-of-Edit drive");
        match &actions[0] {
            RefAction::SetConductorsClosed {
                terminal, closed, ..
            } => {
                assert_eq!(*terminal, 1);
                assert_eq!(closed, &vec![false, true, true]);
                closed.clone()
            }
            a => panic!("expected SetConductorsClosed, got {a:?}"),
        }
    };

    // Apply the deferred force exactly like the executive
    // (`exec/command.rs` SetConductorsClosed arm).
    {
        let classes = dss.registered_classes_mut();
        let cls = classes
            .iter_mut()
            .find(|c| c.props.class_name() == "Line")
            .unwrap();
        let &oi = cls.name_to_idx.get("swk").unwrap();
        let line = cls
            .arena
            .get_mut::<crate::elements::pd::line::Line>(oi)
            .unwrap();
        for (i, &c) in closed.iter().enumerate() {
            line.cd_mut().set_conductor_closed(1, i + 1, c);
        }
    }
    // The executive raises SystemYChanged off the target's yprim_invalid
    // right after applying the ref actions; mirror it so the solve rebuilds Y.
    dss.circuit_mut().unwrap().solution.system_y_changed = true;

    dss.command("solve");
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());

    let snaps = dss.snapshot_elements();
    let s = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("Line.swk"))
        .expect("line.swk in the snapshot");
    let mag = |k: usize| (s.currents[k].re.powi(2) + s.currents[k].im.powi(2)).sqrt();
    // Phase 1 open → its current is zero (r4133 probe: '0.000000 +0.000000j');
    // a ganged drive (pre-RP3.7) would zero ALL six entries, and a wrong-phase
    // drive would leave phase 1 at its balanced ~13.89 A.
    // **Band** (RP3.7 FIX-A1, verify-A1 finding F7): the calibrated element-current
    // floor `abs 1e-6` (`tests/TOLERANCE_NOTES.md` §tiers, "element currents /
    // powers", tight tier), which also brackets the transcript's own resolution —
    // the r4133 DLL prints each component to six decimals, so a magnitude derived
    // from it carries up to `sqrt(2)*5e-7 = 7.1e-7` of print quantization.
    // MEASURED on this tree: phase 1 = 7.20e-9 (r4133 prints exactly
    // `0.000000 +0.000000j`), phase 2 |Δ| = 3.66e-7, phase 3 |Δ| = 2.75e-7 — the
    // largest is 2.6e-8 RELATIVE. This replaced an ad-hoc `5e-4` relative band
    // (~4 orders looser than the floor, blind to a ~7 mA per-phase drive error).
    const IFLOOR: f64 = 1e-6;
    assert!(
        mag(0) < IFLOOR,
        "phase-1 current must be ~0, got {}",
        mag(0)
    );
    // The expected magnitudes are `|-6.953452-12.035939j|` / `|-6.935750+12.030079j|`
    // at full precision — the r4133 DLL's own components, not a port capture.
    for (k, e) in [(1usize, 13.900155478555806_f64), (2, 13.886231627361722)] {
        assert!(
            (mag(k) - e).abs() < IFLOOR,
            "phase-{} |I| = {} vs r4133 {} (|delta| = {:e})",
            k + 1,
            mag(k),
            e,
            (mag(k) - e).abs()
        );
    }

    // The first State write latched Normal per phase (Edit supplemental
    // :221-228) — the reset target is heterogeneous too.
    {
        let classes = dss.registered_classes_mut();
        let cls = classes
            .iter_mut()
            .find(|c| c.props.class_name() == "SwtControl")
            .unwrap();
        let &oi = cls.name_to_idx.get("sw1").unwrap();
        let sw = cls.arena.get_mut::<SwtControl>(oi).unwrap();
        assert!(sw.normal_state_set);
        assert_eq!(sw.normal_state[1], ControlAction::Open);
        assert_eq!(sw.normal_state[2], ControlAction::Close);
        assert_eq!(sw.normal_state[3], ControlAction::Close);
    }
    // The switch operated at edit time, not through the queue (r4133 has no
    // Sample-time queue for state writes — no OPENED event, probe §3).
    assert!(
        !dss.event_log().iter().any(|s| s.contains("Action=OPENED")),
        "per-phase force applies at edit time; log = {:?}",
        dss.event_log()
    );
}

/// The executive twin of the pin above: the SAME probe deck and the SAME r4133
/// solved currents, but driven end-to-end by
/// `edit swtcontrol.sw1 state=(open, closed, closed)` through
/// [`Dss::command`] — parser `WasQuoted` → `set_enum_array_raw` →
/// `InterpretSwitchState` → the end-of-Edit `RecalcElementData` drive → the
/// executive's `SetConductorsClosed` application → `SystemYChanged` → solve.
/// A1 could only assert this on the arena object because the property seam was
/// still scalar; RP3.7 A2 makes the script path the real one, so the pin now
/// covers the plumbing A1 had to bypass. r4133 (`tmp/rp37/probe.md` §3):
/// phase 1 = 0, phases 2/3 |I| = 13.90016 / 13.88623 A.
#[test]
fn per_phase_state_write_through_the_executive_opens_only_its_phase() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.p1 basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new line.feed bus1=src bus2=sw phases=3 r1=0.05 x1=0.05 c1=0 length=1 units=km",
        "new line.swk bus1=sw bus2=ld phases=3 r1=0.01 x1=0.01 c1=0 length=1 units=km",
        "new swtcontrol.sw1 switchedobj=line.swk switchedterm=1",
        "new load.ld bus1=ld.1.2.3.0 phases=3 conn=wye kv=12.47 kw=300 pf=1.0 model=1",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "edit swtcontrol.sw1 state=(open, closed, closed)",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[open, closed, closed, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, closed, closed, ]"
    );

    let snaps = dss.snapshot_elements();
    let s = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case("Line.swk"))
        .expect("line.swk in the snapshot");
    let mag = |k: usize| (s.currents[k].re.powi(2) + s.currents[k].im.powi(2)).sqrt();
    // **Band** (RP3.7 FIX-A1, verify-A1 finding F7): the calibrated element-current
    // floor `abs 1e-6` (`tests/TOLERANCE_NOTES.md` §tiers, "element currents /
    // powers", tight tier), which also brackets the transcript's own resolution —
    // the r4133 DLL prints each component to six decimals, so a magnitude derived
    // from it carries up to `sqrt(2)*5e-7 = 7.1e-7` of print quantization.
    // MEASURED on this tree: phase 1 = 7.20e-9 (r4133 prints exactly
    // `0.000000 +0.000000j`), phase 2 |Δ| = 3.66e-7, phase 3 |Δ| = 2.75e-7 — the
    // largest is 2.6e-8 RELATIVE. This replaced an ad-hoc `5e-4` relative band
    // (~4 orders looser than the floor, blind to a ~7 mA per-phase drive error).
    const IFLOOR: f64 = 1e-6;
    assert!(
        mag(0) < IFLOOR,
        "phase-1 current must be ~0, got {}",
        mag(0)
    );
    // The expected magnitudes are `|-6.953452-12.035939j|` / `|-6.935750+12.030079j|`
    // at full precision — the r4133 DLL's own components, not a port capture.
    for (k, e) in [(1usize, 13.900155478555806_f64), (2, 13.886231627361722)] {
        assert!(
            (mag(k) - e).abs() < IFLOOR,
            "phase-{} |I| = {} vs r4133 {} (|delta| = {:e})",
            k + 1,
            mag(k),
            e,
            (mag(k) - e).abs()
        );
    }
    // r4133 forces at edit time, not through the control queue.
    assert!(
        !dss.event_log().iter().any(|s| s.contains("Action=OPENED")),
        "log = {:?}",
        dss.event_log()
    );
}

// ---------------------------------------------------------------------------
// Ganged/homogeneous equivalence with the pre-change behavior (the corpus
// decks' shape) — full-engine pins; these are the "corpus gate must not move"
// witnesses at unit level.
// ---------------------------------------------------------------------------

/// Find a snapshot element by full name and return its terminal-1 max
/// current magnitude (re/im interleaved over conductors).
fn term1_max_current(dss: &mut Dss, name: &str) -> f64 {
    let snaps = dss.snapshot_elements();
    let s = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("no element {name}"));
    let nph = 3usize;
    let mut m = 0.0_f64;
    for k in 0..nph {
        let re = s.currents[k].re;
        let im = s.currents[k].im;
        m = m.max((re * re + im * im).sqrt());
    }
    m
}

/// WP-U2.4 D6 (EPRI r4133 `SwtControl.pas`): the deprecated `Action=open`
/// forces the switched element open **immediately at parse time** — like
/// `State=open` — with NO control-queue delay and NO `OPENED` event. The
/// per-phase rewrite keeps this ganged observable bit-identical (r4133
/// `action=` is always ganged, :419-429).
#[test]
fn action_open_forces_line_open_at_parse_no_event() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        // two parallel feeds so opening l1 leaves l2 feeding the load.
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1 switch=y",
        "new line.l2 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=300",
        // delay=2 is now vestigial for Action (r4133 forces immediately).
        "new swtcontrol.sw1 switchedobj=line.l1 switchedterm=1 normal=closed delay=2",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "edit swtcontrol.sw1 action=open",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    assert!(
        term1_max_current(&mut dss, "Line.l1") < 1.0,
        "action=open should force the switch open at parse (D6)"
    );
    assert!(
        term1_max_current(&mut dss, "Line.l2") > 1.0,
        "parallel line should carry the load"
    );
    assert!(
        !dss.event_log().iter().any(|s| s.contains("Action=OPENED")),
        "D6: action forces at parse, so no queued OPENED event; log = {:?}",
        dss.event_log()
    );
}

/// `State=open` (ganged, the corpus shape) forces the switched element open at
/// parse time through the deferred per-phase drive — every phase this time.
#[test]
fn state_open_forces_line_open_at_parse() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1 switch=y",
        "new line.l2 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=300",
        "new swtcontrol.sw1 switchedobj=line.l1 switchedterm=1 state=open",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    assert!(
        term1_max_current(&mut dss, "Line.l1") < 1.0,
        "state=open should force the line open at parse"
    );
    assert!(term1_max_current(&mut dss, "Line.l2") > 1.0);
}

/// `Reset` (DoResetControls) drives the dispatch Reset arm: it restores the
/// switch to `NormalState` per phase and re-forces the controlled element.
/// With `normal=closed`, a line opened via `state=open` is re-closed by
/// `reset` (oracle-probed: `reset` ⇒ l1 closed again).
#[test]
fn reset_restores_switch_to_normal_via_dispatch() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.t basekv=12.47 phases=3 bus1=src basefreq=60",
        "new line.l1 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1 switch=y",
        "new line.l2 bus1=src bus2=b phases=3 r1=0.3 x1=0.6 length=1",
        "new load.ld bus1=b phases=3 kv=12.47 kw=300",
        "new swtcontrol.sw1 switchedobj=line.l1 switchedterm=1 normal=closed state=open",
        "set voltagebases=[12.47]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    assert!(
        term1_max_current(&mut dss, "Line.l1") < 1.0,
        "state=open should leave l1 open before reset"
    );
    dss.command("reset");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    assert!(
        term1_max_current(&mut dss, "Line.l1") > 1.0,
        "reset (normal=closed) should re-close the switched line"
    );
}

// ---------------------------------------------------------------------------
// The r4133 per-phase render (RP3.7 A2). Every expected byte string below is
// quoted from the live r4133 DLL transcript (`tmp/rp37/probe.md` §4/§5/§6,
// `tmp/rp37/out_nil_controlled.txt`, `tmp/rp37/out_a2a.txt`) — never from this
// port's own output.
// ---------------------------------------------------------------------------

/// Build a circuit with one `switch=y` line of `nphases` phases and a
/// SwtControl on it, through the ordinary executive.
fn micro_dss(nphases: usize) -> Dss {
    let mut dss = Dss::new();
    for c in [
        "clear".to_string(),
        "new circuit.c".to_string(),
        format!("new line.l1 bus1=b1 bus2=b2 phases={nphases} r1=0.3 x1=0.6 length=1 switch=y"),
        "new swtcontrol.sw1 switchedobj=line.l1 switchedterm=1".to_string(),
    ] {
        dss.command(&c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss
}

fn ask(dss: &mut Dss, q: &str) -> String {
    dss.command(q);
    dss.result().trim().to_string()
}

/// r4133 `GetPropertyValue` 6/7 (`SwtControl.pas:573-623`): `'['` + one
/// `open`/`closed` token **plus a trailing `', '`** per controlled-element
/// phase + `']'`. Measured on the r4133 DLL for 1, 3 and 4 phases (probe §4
/// table, P2(iii)/P2(i)/P2(iv-b)). A fresh control renders the all-CLOSED
/// array for `Normal` too — `Create` initializes it (`:299-307`) — which is
/// where the pre-RP3.7 port's `''` (scalar `None`) was wrong against BOTH
/// oracles (probe §11.5). Reverting the array render, the trailing `', '`, or
/// the `Create` initialization each breaks a literal here.
#[test]
fn render_is_one_token_per_controlled_element_phase() {
    let mut dss = micro_dss(3);
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[closed, closed, closed, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[closed, closed, closed, ]"
    );
    // The scalar `Action` seam is untouched by the flip (r4133 has no getter
    // arm for 3; the port keeps the 0.14.5 `CurrentAction` readback).
    assert_eq!(ask(&mut dss, "? swtcontrol.sw1.action"), "close");

    let mut dss1 = micro_dss(1);
    assert_eq!(ask(&mut dss1, "? swtcontrol.sw1.normal"), "[closed, ]");
    assert_eq!(ask(&mut dss1, "? swtcontrol.sw1.state"), "[closed, ]");
    dss1.command("edit swtcontrol.sw1 state=open");
    assert_eq!(ask(&mut dss1, "? swtcontrol.sw1.state"), "[open, ]");

    // Four phases: r4133 renders four tokens off a 4-phase controlled element
    // (probe P2(iv-b)) even though `Create` allocated three (`:299-305`) — the
    // port answers the same four from its six in-bounds slots. The COUNT is the
    // pinned observable; the 4th token's VALUE is an out-of-bounds heap read
    // upstream, which happens to read `closed` on this shape (re-measured
    // 2026-09-02) but is not a defined r4133 answer.
    let mut dss4 = micro_dss(4);
    assert_eq!(
        ask(&mut dss4, "? swtcontrol.sw1.state"),
        "[closed, closed, closed, closed, ]"
    );
    dss4.command("edit swtcontrol.sw1 state=(open, closed, closed, closed)");
    assert_eq!(
        ask(&mut dss4, "? swtcontrol.sw1.state"),
        "[open, closed, closed, closed, ]"
    );
    assert_eq!(
        ask(&mut dss4, "? swtcontrol.sw1.normal"),
        "[open, closed, closed, closed, ]",
        "the first state write latches Normal per phase"
    );
}

/// `ControlledElement = NIL` renders the bare `'[]'` — r4133's getters skip
/// the loop entirely (`:589`/`:600`), measured on the orphan control
/// (`tmp/rp37/out_nil_controlled.txt`: `'[]'` for both `state` and `normal`,
/// after the #387 create error). A render bound that fell back to the
/// control's own 3 phases would print three tokens here.
#[test]
fn nil_controlled_element_renders_the_empty_array() {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.nilc basekv=12.47 phases=3 bus1=src");
    dss.command("new swtcontrol.orphan"); // r4133: error #387, object still created
    assert_eq!(ask(&mut dss, "? swtcontrol.orphan.state"), "[]");
    assert_eq!(ask(&mut dss, "? swtcontrol.orphan.normal"), "[]");
}

/// A QUOTED value goes phase by phase, a BARE token is ganged — including for
/// a single token, which is the only place the two shapes are visually
/// identical. Measured on the r4133 DLL (`tmp/rp37/out_a2a.txt` A2a(1)):
/// `state=(open)` on an all-closed 3-phase control gives
/// `[open, closed, closed, ]` while `state=open` gives `[open, open, open, ]`,
/// and `normal=(closed)` over an all-open normal gives `[closed, open, open, ]`.
/// Dropping the `WasQuoted` plumbing (`exec/command.rs` →
/// [`PropEngine::was_quoted`](crate::obj::props::PropEngine::was_quoted)) makes
/// the first and third assertions read like the ganged ones.
#[test]
fn a_quoted_single_token_is_per_phase_a_bare_one_is_ganged() {
    let mut dss = micro_dss(3);
    dss.command("edit swtcontrol.sw1 state=(open)");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[open, closed, closed, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, closed, closed, ]"
    );
    dss.command("edit swtcontrol.sw1 state=closed");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[closed, closed, closed, ]"
    );
    dss.command("edit swtcontrol.sw1 state=open");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[open, open, open, ]"
    );
    dss.command("edit swtcontrol.sw1 normal=open");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, open, open, ]"
    );
    dss.command("edit swtcontrol.sw1 normal=(closed)");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[closed, open, open, ]"
    );
}

/// The heterogeneous write through both real seams: the executive
/// (`edit swtcontrol.sw1 state=(open, closed, closed)`) and the property
/// applier called directly with `was_quoted = true`. Expected bytes: r4133
/// probe §4 / P1 (`'[open, closed, closed, ]'`, Normal following on the first
/// state write).
#[test]
fn per_phase_write_renders_the_r4133_bytes_through_both_seams() {
    let mut dss = micro_dss(3);
    dss.command("edit swtcontrol.sw1 state=(open, closed, closed)");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[open, closed, closed, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, closed, closed, ]"
    );

    let mut sw = sw_with_snap(3);
    edit_prop(&mut sw, "State", "open, closed, closed", true);
    assert_eq!(render(&sw, "State"), "[open, closed, closed, ]");
    assert_eq!(render(&sw, "Normal"), "[open, closed, closed, ]");
}

/// The per-phase parse honors at most FIVE tokens (`:461`
/// `While (Length(DataStr2)>0) and (i<SWTCONTROLMAXDIM)`) while the render
/// loops up to `Min(6, NPhases)` — the two bounds are deliberately different.
/// Driven through the property seam (`was_quoted = true`), so a regression that
/// re-routed the write to the generic `array_size`-bounded tokenizer would let
/// the sixth token through.
#[test]
fn the_property_seam_caps_the_per_phase_parse_at_five_tokens() {
    let mut sw = sw_with_snap(6);
    edit_prop(&mut sw, "State", "open, open, open, open, open, open", true);
    assert_eq!(
        render(&sw, "State"),
        "[open, open, open, open, open, closed, ]",
        "slots 1..5 written, the 6th token dropped, slot 6 still rendered"
    );
    assert_eq!(sw.present_state[6], ControlAction::Close);
}

/// The value-string seam that carries no outer parser — JSON import renders a
/// `["open","closed","closed"]` member as the space-joined token list and
/// applies it through `edit_property` with `was_quoted = false`
/// (`obj/props/class_props/json_set.rs::json_to_value_string`,
/// `exec/json_import.rs:249`). [`SwtControl::value_implies_quoted`] recovers
/// the per-phase reading from the token count, so a JSON round trip keeps a
/// heterogeneous switch heterogeneous instead of ganging it to its first token.
#[test]
fn a_multi_token_value_without_the_quote_flag_is_still_per_phase() {
    let mut sw = sw_with_snap(3);
    edit_prop(&mut sw, "State", "open closed closed", false);
    assert_eq!(render(&sw, "State"), "[open, closed, closed, ]");
    // A value that still carries its bracket (nothing strips it on these
    // seams) is quoted too.
    let mut sw2 = sw_with_snap(3);
    edit_prop(&mut sw2, "State", "[open, closed, closed, ]", false);
    assert_eq!(render(&sw2, "State"), "[open, closed, closed, ]");
    // …and a single bare token stays ganged (`normal=closed`, `action=o` — the
    // only spelling any corpus deck writes, probe §7).
    let mut sw3 = sw_with_snap(3);
    edit_prop(&mut sw3, "State", "open", false);
    assert_eq!(render(&sw3, "State"), "[open, open, open, ]");
}

/// The render bound follows the controlled element through `MakePosSequence`,
/// which is plan §RP3.7(b)'s resync arriving for free on the SwtControl side:
/// r4133's getters loop the LIVE `ControlledElement.NPhases`, so on the
/// `p4_makeposseq.dss` shape (`tmp/rp37/decks/`) the control renders
/// `'[closed, closed, closed, ]'` before `makeposseq` and `'[closed, ]'` after
/// (probe §6/§11.4, measured on the r4133 DLL). The trailing `edit`+`?` is the
/// staleness guard: `recalc` re-reads the controlled-element snapshot, so a
/// `make_pos_sequence` that refreshed only `Nphases` and left the snapshot
/// frozen would resurrect the three-token render on the next edit.
#[test]
fn the_render_bound_follows_makeposseq() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.psq basekv=115 pu=1.0 phases=3 bus1=src",
        "new line.l2 bus1=src bus2=b1 phases=3 r1=0.25 x1=0.6 c1=3 length=1 units=km",
        "new line.sw bus1=b1 bus2=b2 phases=3 switch=yes",
        "new swtcontrol.swc switchedobj=line.sw switchedterm=1 action=close",
        "new load.ld bus1=b2 phases=3 kv=115 kw=2000 pf=0.95 model=1",
        "set voltagebases=[115]",
        "calcvoltagebases",
        "solve",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        ask(&mut dss, "? swtcontrol.swc.normal"),
        "[closed, closed, closed, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.swc.state"),
        "[closed, closed, closed, ]"
    );
    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(ask(&mut dss, "? line.sw.phases"), "1");
    assert_eq!(ask(&mut dss, "? swtcontrol.swc.normal"), "[closed, ]");
    assert_eq!(ask(&mut dss, "? swtcontrol.swc.state"), "[closed, ]");
    // A later edit re-runs RecalcElementData off the snapshot — still 1 phase.
    dss.command("edit swtcontrol.swc delay=30");
    assert_eq!(ask(&mut dss, "? swtcontrol.swc.state"), "[closed, ]");
}

/// The ordinal twin [`DssObject::set_enum_array`] — unreachable in production
/// (the raw hook consumes every write) but kept semantically identical, so a
/// future caller cannot silently get ganged-vs-per-phase or the lock rule
/// wrong. Same three rules: per-phase fill, five-slot cap, `Keep` = unchanged.
///
/// **RP3.7 audit settlement (2026-09-02): the identity is ENFORCED here, not
/// claimed in prose.** Each row below drives two identical controls — one
/// through the ordinal setter, one through
/// [`SwtControl::interpret_switch_state`] with the equivalent quoted spelling —
/// and asserts BOTH the r4133 bytes and that the two renders agree. The
/// previous version asserted literals only: mutating the interpreter's lock
/// guard to a whole-object `if self.locked { return; }` left it green while
/// the two paths disagreed about `Normal`.
///
/// The `Keep` ordinal's spelling on the interpreter side is any token whose
/// first character is neither `o` nor `c` — r4133's `case` has no else arm
/// (`:464-467`), so such a slot is left unchanged exactly as `Keep` is.
#[test]
fn the_ordinal_array_setter_matches_the_interpreter() {
    /// phases, locked, property, ordinals, the quoted spelling, r4133 bytes.
    type Case<'a> = (usize, bool, usize, &'a [i32], &'a str, &'a str);
    let keep = ControlAction::Keep.ordinal();
    let open = ControlAction::Open.ordinal();
    let cases: [Case<'_>; 5] = [
        (
            3,
            false,
            prop::STATE,
            &[open, keep, open],
            "open, keep, open",
            "[open, closed, open, ]",
        ),
        // The five-slot cap on both paths (`take(SW_MAX - 1)` vs `:461`).
        (
            6,
            false,
            prop::STATE,
            &[open; 6],
            "open, open, open, open, open, open",
            "[open, open, open, open, open, closed, ]",
        ),
        // The lock guard keys on the property NAME (`:416-417`).
        (
            3,
            true,
            prop::STATE,
            &[open; 3],
            "open, open, open",
            "[closed, closed, closed, ]",
        ),
        (
            3,
            true,
            prop::NORMAL,
            &[open; 3],
            "open, open, open",
            "[open, open, open, ]",
        ),
        (
            3,
            false,
            prop::NORMAL,
            &[keep, open, keep],
            "keep, open, keep",
            "[closed, open, closed, ]",
        ),
    ];
    for (phases, locked, idx, ords, spelling, expected) in cases {
        let (name, sprop) = if idx == prop::STATE {
            ("State", SwtStateProp::State)
        } else {
            ("Normal", SwtStateProp::Normal)
        };
        let mut by_ordinal = sw_with_snap(phases);
        by_ordinal.locked = locked;
        by_ordinal.set_enum_array(idx, ords);

        let mut by_interpreter = sw_with_snap(phases);
        by_interpreter.locked = locked;
        by_interpreter.interpret_switch_state(sprop, spelling, true);

        let a = render(&by_ordinal, name);
        let b = render(&by_interpreter, name);
        assert_eq!(a, expected, "{name} locked={locked}: r4133 bytes");
        assert_eq!(
            a, b,
            "{name} locked={locked}: the ordinal setter drifted from the interpreter"
        );
    }
}

/// r4133 authority over the scalar seam (2026-08-02 policy): a non-matching
/// bare token (`state=bogus`) leaves the state unchanged, silently — the
/// `Keep` default the registry now carries (r4133's `case` has no else arm,
/// probe §9). Pre-RP3.7 the port raised `Could not match enum`; capi silently
/// closes (its DefaultValue=CTRL_CLOSE) — both refused for r4133's no-op. The
/// Edit supplemental still latches Normal from Present on the (no-op) first
/// State write, exactly as r4133 runs it outside the interpreter (:221-228).
#[test]
fn non_matching_state_token_leaves_the_state_untouched() {
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.c",
        "new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y",
        "new swtcontrol.sw1 switchedobj=line.l1 switchedterm=1",
    ] {
        dss.command(c);
    }
    dss.command("edit swtcontrol.sw1 state=bogus");
    assert!(
        dss.errors().is_empty(),
        "r4133 is silent: {:?}",
        dss.errors()
    );
    dss.command("? swtcontrol.sw1.state");
    assert_eq!(
        dss.result().trim(),
        "[closed, closed, closed, ]",
        "state unchanged"
    );
    dss.command("? swtcontrol.sw1.normal");
    assert_eq!(
        dss.result().trim(),
        "[closed, closed, closed, ]",
        "supplemental latched closed"
    );
}

// ---------------------------------------------------------------------------
// The (a2) lock rule — r4133's guard is property-NAME-conditional
// (`SwtControl.pas:416-417`, under the comment "Only allowed to change normal
// state if locked"), NOT 0.14.5's `ConditionalReadOnly` refusal of all three.
// ---------------------------------------------------------------------------

/// RP3.7 (a2), the plan's headline fix, in both lanes. Byte-for-byte the r4133
/// DLL sequence of `tmp/rp37/probe.md` §5 on a locked 3-phase control:
/// `normal=open` APPLIES (`[open, open, open, ]`) and so does a quoted
/// per-phase `normal=(open, closed, open)` (`[open, closed, open, ]`), while
/// `state=open` and `action=open` move nothing — the switch stays closed
/// throughout. Then `lock=no` lets `state=open` through, and `reset=yes`
/// restores State from Normal per phase.
///
/// Reverting to the scalar-era gate (a locked write to any of the three is
/// dropped, `docs/upgrade/DIVERGENCES.md` §D12's old claim) fails on the very
/// first `normal=` assertion.
#[test]
fn locked_normal_applies_locked_state_and_action_do_not() {
    let mut dss = micro_dss(3);
    dss.command("edit swtcontrol.sw1 lock=yes");
    assert_eq!(ask(&mut dss, "? swtcontrol.sw1.lock"), "Yes");

    dss.command("edit swtcontrol.sw1 normal=open");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, open, open, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[closed, closed, closed, ]"
    );

    dss.command("edit swtcontrol.sw1 state=open");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, open, open, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[closed, closed, closed, ]",
        "a locked state= write is refused"
    );

    dss.command("edit swtcontrol.sw1 action=open");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, open, open, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[closed, closed, closed, ]",
        "a locked action= write is refused"
    );

    dss.command("edit swtcontrol.sw1 normal=(open, closed, open)");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, closed, open, ]",
        "the guard keys on the property NAME, not the value shape"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[closed, closed, closed, ]"
    );

    dss.command("edit swtcontrol.sw1 lock=no");
    dss.command("edit swtcontrol.sw1 state=open");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[open, open, open, ]"
    );
    dss.command("edit swtcontrol.sw1 reset=yes");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[open, closed, open, ]",
        "reset restores State from Normal per phase"
    );
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
}

/// The Edit supplemental (`SwtControl.pas:219-228`) runs even when the lock
/// guard refused the write — it sits OUTSIDE `InterpretSwitchState`, in the
/// `{Supplemental Actions} case ParamPointer of 3, 7:` block.
///
/// The discriminator, measured on the r4133 DLL: with a lock in place, a
/// refused `state=open` (and likewise `action=open`) still latches
/// `NormalStateSet`, so the LATER unlocked `state=open` may not copy Present
/// into Normal — `Normal` stays `[closed, closed, closed, ]`
/// (`tmp/rp37/out_a2a2.txt` A2a(4), `tmp/rp37/out_a2a.txt` A2a(3)) — whereas
/// the identical sequence with no lock carries it to `[open, open, open, ]`
/// (A2a(5)). Restoring the old `if self.locked { return; }` early-return in
/// `side_effects` flips the locked case onto the unlocked answer.
#[test]
fn a_locked_state_write_still_runs_the_normal_defaults_supplemental() {
    for refused in ["state=open", "action=open"] {
        let mut dss = micro_dss(3);
        dss.command("edit swtcontrol.sw1 lock=yes");
        dss.command(&format!("edit swtcontrol.sw1 {refused}"));
        assert_eq!(
            ask(&mut dss, "? swtcontrol.sw1.state"),
            "[closed, closed, closed, ]"
        );
        assert_eq!(
            ask(&mut dss, "? swtcontrol.sw1.normal"),
            "[closed, closed, closed, ]"
        );
        dss.command("edit swtcontrol.sw1 lock=no");
        dss.command("edit swtcontrol.sw1 state=open");
        assert_eq!(
            ask(&mut dss, "? swtcontrol.sw1.state"),
            "[open, open, open, ]"
        );
        assert_eq!(
            ask(&mut dss, "? swtcontrol.sw1.normal"),
            "[closed, closed, closed, ]",
            "the refused {refused} already latched NormalStateSet"
        );
    }
    // A2a(5): the same first-write sequence with no lock DOES carry Normal.
    let mut dss = micro_dss(3);
    dss.command("edit swtcontrol.sw1 state=open");
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.state"),
        "[open, open, open, ]"
    );
    assert_eq!(
        ask(&mut dss, "? swtcontrol.sw1.normal"),
        "[open, open, open, ]"
    );
}

#[test]
fn locked_ignores_action_write() {
    // r4133 `:416-417`: `action` starts with 'a', so a locked write exits
    // before touching `States` — the port routes `set_i32(ACTION)` through the
    // same guard. Unlocked, the write is ganged across every slot (`:419-429`).
    let mut sw = SwtControl::new("sw1");
    sw.locked = true;
    sw.set_i32(prop::ACTION, ControlAction::Open.ordinal());
    sw.side_effects(prop::ACTION, 0);
    assert_eq!(sw.current_action, ControlAction::Close); // unchanged
    assert_eq!(sw.present_state[1..=SW_MAX], [ControlAction::Close; SW_MAX]);
    // unlocked: the write lands — ganged across every slot.
    sw.locked = false;
    sw.set_i32(prop::ACTION, ControlAction::Open.ordinal());
    sw.side_effects(prop::ACTION, 0);
    assert_eq!(sw.current_action, ControlAction::Open);
    assert_eq!(sw.present_state[1..=SW_MAX], [ControlAction::Open; SW_MAX]);
}

// ---------------------------------------------------------------------------
// The 0.14.5 queue machinery (Sample / DoPendingAction) on the per-phase model
// ---------------------------------------------------------------------------

#[test]
fn sample_arms_when_action_differs() {
    let mut sw = SwtControl::new("sw1");
    sw.current_action = ControlAction::Open; // commanded open, switch still closed
    let mut sc = Scratch::new();
    sw.sample(&mut sc.ctx(0, 0.0));
    assert!(sw.armed);
    assert_eq!(sc.queue.queue_size(), 1);
}

#[test]
fn sample_does_not_arm_when_matching() {
    let mut sw = SwtControl::new("sw1");
    // current_action == present view (both CLOSE) → nothing to do.
    let mut sc = Scratch::new();
    sw.sample(&mut sc.ctx(0, 0.0));
    assert!(!sw.armed);
    assert!(sc.queue.is_empty());
}

#[test]
fn sample_stays_inert_after_a_state_write() {
    // The D6 invariant on the per-phase model: after a State/Action write the
    // side effect syncs `current_action` to the arrays' ganged view, so the
    // queue branch is false (r4133's Sample is inert anyway; the capi-lane
    // swtcontrol_lock deck pins exactly this absence of an action push).
    let mut sw = sw_with_snap(3);
    edit_prop(&mut sw, "State", "open", false);
    sw.take_ref_actions();
    let mut sc = Scratch::new();
    sw.sample(&mut sc.ctx(0, 0.0));
    assert!(!sw.armed);
    assert!(sc.queue.is_empty(), "no action push after a state write");
}

/// The other side of the D6 invariant, and a **tripwire on a live divergence
/// from the behavioral authority** (RP3.7 FIX-A1, verify-A1 finding F2): after
/// a plain `normal=open` the retained 0.14.5 glue leaves
/// `current_action = Open` against an all-CLOSED `present_state`, which is
/// exactly [`SwtControl::sample`]'s arming condition — so the port queues an
/// action and OPENS a switch that r4133 leaves closed (r4133 comments out both
/// `Sample` and `DoPendingAction`; measured end-to-end in
/// `tmp/rp37/out_port_probe10.txt` vs the r4133 DLL in
/// `tmp/rp37/out_fixa1.txt` §F2).
///
/// The **locked** write arms it too, and that half is new with RP3.7 (audit
/// settlement, 2026-09-02): before (a2) a locked `normal=` was refused outright
/// by the scalar-era `ConditionalReadOnly` gates in `set_i32`/`side_effects`, so
/// it could not reach the glue; r4133's own guard lets `n`ormal through
/// (`:416-417`), the port now applies it, and `side_effects(NORMAL)` runs with
/// it. The switch still does not move — [`SwtControl::do_pending_action`] is
/// `!locked`-guarded — but the queue push and the `armed` latch happen, and the
/// control queue is a compared surface (`compare_ctrlqueue`). Corpus exposure
/// stays zero on this path as well: `swtcontrol_lock.dss` types
/// `normal=closed` BEFORE `lock=yes` on the same `New`.
///
/// This pin asserts the CURRENT, divergent behavior on purpose: corpus exposure
/// is zero (every corpus `normal=` is a ganged `normal=closed` over an
/// all-closed state, so the ganged views agree and nothing arms), and retiring
/// the 0.14.5 `Sample` body needs `swtcontrol_lock.dss` re-gated off the capi
/// channel (`ORPHANED_GAPS.md` §1.16). When that happens this test must be
/// DELETED, not re-baselined — it exists so the gap cannot be forgotten or
/// deepened silently.
#[test]
fn sample_arms_on_a_normal_write_the_retained_capi_channel() {
    let mut sw = sw_with_snap(3);
    edit_prop(&mut sw, "Normal", "open", false);
    sw.take_ref_actions();
    assert_eq!(
        sw.present_state[1..=3],
        [ControlAction::Close; 3],
        "r4133: a normal= write moves FNormalState only"
    );
    let mut sc = Scratch::new();
    sw.sample(&mut sc.ctx(0, 0.0));
    assert!(sw.armed, "the capi glue arms off `normal=`");
    assert_eq!(
        sc.queue.queue_size(),
        1,
        "queued an action r4133 never queues"
    );

    // The locked twin: `lock=yes` first, then `normal=open`. The write applies
    // (r4133's guard is on the property name), so the same glue arms — one
    // CTRL_LOCK push from the lock write plus the spurious action push.
    let mut lk = sw_with_snap(3);
    edit_prop(&mut lk, "Lock", "yes", false);
    edit_prop(&mut lk, "Normal", "open", false);
    lk.take_ref_actions();
    assert_eq!(
        render(&lk, "Normal"),
        "[open, open, open, ]",
        "r4133 :416-417"
    );
    assert_eq!(
        render(&lk, "State"),
        "[closed, closed, closed, ]",
        "the switch itself never moves while locked"
    );
    let mut sc2 = Scratch::new();
    lk.sample(&mut sc2.ctx(0, 0.0));
    assert!(lk.armed, "the capi glue arms off a LOCKED `normal=` too");
    assert_eq!(
        sc2.queue.queue_size(),
        2,
        "CTRL_LOCK + an action push r4133 never queues"
    );
}

#[test]
fn do_pending_open_opens_terminal_and_logs() {
    // The 0.14.5 queue machinery stays whole-terminal (ganged): every slot
    // follows the operated state.
    let mut sw = SwtControl::new("sw1");
    let mut ms = MockSwitch::new(3);
    let mut sc = Scratch::new();
    sw.do_pending_action(ControlAction::Open.ordinal(), &mut ms, &mut sc.ctx(0, 1.0));
    assert!(!ms.cd.terminal_all_phases_closed(1)); // terminal 1 opened
    assert_eq!(
        sw.present_state[1..=SW_MAX],
        [ControlAction::Open; SW_MAX],
        "ganged slots"
    );
    assert!(!sw.armed);
    assert!(sc.y_changed);
    assert_eq!(sc.events.len(), 1);
    let line = &sc.events.entries()[0];
    assert!(line.contains("Element=SwtControl.sw1"), "{line}");
    assert!(line.contains("Action=OPENED"), "{line}");
}

#[test]
fn do_pending_close_closes_terminal_and_logs() {
    let mut sw = SwtControl::new("sw1");
    sw.present_state[1] = ControlAction::Open; // any open slot flips the view
    let mut ms = MockSwitch::new(3);
    ms.cd.set_terminal_closed(1, false); // start open
    let mut sc = Scratch::new();
    sw.do_pending_action(ControlAction::Close.ordinal(), &mut ms, &mut sc.ctx(0, 1.0));
    assert!(ms.cd.terminal_all_phases_closed(1));
    assert_eq!(sw.present_state[1], ControlAction::Close);
    assert!(sc.y_changed);
    assert!(sc.events.entries()[0].contains("Action=CLOSED"));
}

#[test]
fn do_pending_lock_then_open_is_blocked() {
    let mut sw = SwtControl::new("sw1");
    let mut ms = MockSwitch::new(3);
    let mut sc = Scratch::new();
    // Lock first, then an open action must be ignored (still closed, no event).
    sw.do_pending_action(ControlAction::Lock.ordinal(), &mut ms, &mut sc.ctx(0, 0.0));
    assert!(sw.locked);
    sw.do_pending_action(ControlAction::Open.ordinal(), &mut ms, &mut sc.ctx(0, 1.0));
    assert!(ms.cd.terminal_all_phases_closed(1)); // still closed
    assert_eq!(sw.present_state[1..=SW_MAX], [ControlAction::Close; SW_MAX]);
    assert!(sc.events.is_empty());
}

#[test]
fn do_pending_sets_controlled_active_terminal_even_for_lock() {
    // Pascal sets `ControlledElement.ActiveTerminalIdx := ElementTerminal` before
    // the case — for every code, including LOCK (which touches no conductor).
    let mut sw = SwtControl::new("sw1");
    sw.ccd.element_terminal = 2;
    let mut ms = MockSwitch::new(3); // 2 terminals
    let mut sc = Scratch::new();
    sw.do_pending_action(ControlAction::Lock.ordinal(), &mut ms, &mut sc.ctx(0, 0.0));
    assert!(sw.locked);
    assert_eq!(ms.cd.active_terminal, 1); // terminal 2, 0-based
}

#[test]
fn lock_side_effect_queues_lock_command_pushed_on_sample() {
    let mut sw = SwtControl::new("sw1");
    sw.locked = true;
    sw.side_effects(prop::LOCK, 0);
    assert_eq!(sw.lock_command, ControlAction::Lock);
    let mut sc = Scratch::new();
    sw.sample(&mut sc.ctx(0, 0.0));
    assert_eq!(sw.lock_command, ControlAction::None); // consumed
    assert_eq!(sc.queue.queue_size(), 1);
}

// ---------------------------------------------------------------------------
// The property side effects (D12/D6 shape on the per-phase model)
// ---------------------------------------------------------------------------

#[test]
fn normal_side_effect_syncs_current_action_from_normal_state() {
    // D12 (WP-U1.6) + the capi015 props golden: `Normal=` latches
    // `NormalStateSet` (r4133 Edit arm 6 tail, SwtControl.pas:203) and syncs
    // the 0.14.5 `CurrentAction` glue the `Action` readback renders — while
    // leaving the present array untouched (r4133 normal= never drives).
    let mut sw = SwtControl::new("sw1");
    edit_prop(&mut sw, "Normal", "open", false); // ganged normal slots
    assert!(sw.normal_state_set);
    assert_eq!(
        sw.normal_state[1..=SW_MAX],
        [ControlAction::Open; SW_MAX],
        "normal slots"
    );
    assert_eq!(
        sw.present_state[1..=SW_MAX],
        [ControlAction::Close; SW_MAX],
        "present untouched"
    );
    assert_eq!(sw.current_action, ControlAction::Open); // glued from Normal
}

#[test]
fn state_side_effect_latches_normal_per_phase_and_syncs() {
    // The Edit supplemental (:221-228) per phase over the control's own
    // FNPhases, then the glue sync; the element force is recalc's (the
    // module-doc drive model), not the side effect's.
    let mut sw = sw_with_snap(3);
    edit_prop(&mut sw, "State", "open", false);
    assert!(sw.normal_state_set);
    assert_eq!(
        sw.normal_state[1..=3],
        [ControlAction::Open; 3],
        "latched slots"
    );
    assert_eq!(sw.current_action, ControlAction::Open);
    // The force lands at recalc (end of Edit), per phase.
    sw.recalc();
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        RefAction::SetConductorsClosed { closed, .. } => {
            assert_eq!(closed, &vec![false, false, false], "ganged open drive");
        }
        a => panic!("expected SetConductorsClosed, got {a:?}"),
    }
}

#[test]
fn d6_action_forces_present_state_and_element_like_state() {
    // WP-U2.4 D6 on the per-phase model: `Action` is r4133's ganged `States`
    // write — present slots flip, the first-set supplemental latches Normal,
    // and the end-of-Edit drive forces the element. Reverting D6 (action
    // writes only Normal) leaves the present slots closed.
    let mut sw = sw_with_snap(3);
    sw.set_i32(prop::ACTION, ControlAction::Open.ordinal());
    sw.side_effects(prop::ACTION, 0);
    assert_eq!(
        sw.present_state[1..=SW_MAX],
        [ControlAction::Open; SW_MAX],
        "D6 ganged slots"
    );
    assert_eq!(sw.normal_state[1], ControlAction::Open); // first-set default
    assert!(sw.normal_state_set);
    assert_eq!(sw.current_action, ControlAction::Open);
    sw.recalc();
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        RefAction::SetConductorsClosed { closed, .. } => assert_eq!(closed, &vec![false; 3]),
        a => panic!("expected SetConductorsClosed(open), got {a:?}"),
    }
}

#[test]
fn d6_action_after_declared_normal_leaves_normal_unchanged() {
    // r4133-probed: `normal=closed` then `action=open` leaves normal closed
    // (NormalStateSet already latched on the normal= write) while the present
    // flips. Mirrors the civanlar/swtcontrol_time pattern.
    let mut sw = sw_with_snap(3);
    edit_prop(&mut sw, "Normal", "closed", false);
    sw.set_i32(prop::ACTION, ControlAction::Open.ordinal());
    sw.side_effects(prop::ACTION, 0);
    assert_eq!(sw.present_state[1..=SW_MAX], [ControlAction::Open; SW_MAX]);
    assert_eq!(
        sw.normal_state[1..=SW_MAX],
        [ControlAction::Close; SW_MAX],
        "normal unchanged"
    );
    sw.recalc();
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1); // only the end-of-Edit drive
    match &actions[0] {
        RefAction::SetConductorsClosed { closed, .. } => assert_eq!(closed, &vec![false; 3]),
        a => panic!("expected SetConductorsClosed(open), got {a:?}"),
    }
}

#[test]
fn d6_locked_action_does_not_force_element() {
    // r4133-probed: with `lock=yes`, `action=open` is ignored — the switch
    // stays closed, no element force queued (A1's scalar seam gate; A2's r4133
    // guard refuses it for the same observable on this property).
    let mut sw = sw_with_snap(3);
    sw.locked = true;
    sw.set_i32(prop::ACTION, ControlAction::Open.ordinal()); // ignored
    sw.side_effects(prop::ACTION, 0); // early-return on locked
    assert_eq!(sw.present_state[1..=SW_MAX], [ControlAction::Close; SW_MAX]);
    assert_eq!(sw.current_action, ControlAction::Close);
    sw.recalc(); // recalc still drives — from the unchanged (closed) array
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        RefAction::SetConductorsClosed { closed, .. } => assert_eq!(closed, &vec![true; 3]),
        a => panic!("expected the unchanged-state drive, got {a:?}"),
    }
}

#[test]
fn d12_normal_and_state_readbacks_are_independent() {
    // D12 feature-sensitivity on the per-phase model: set `State=open` then
    // `Normal=closed`; both scalar readbacks survive independently.
    let mut sw = sw_with_snap(3);
    edit_prop(&mut sw, "State", "open", false);
    edit_prop(&mut sw, "Normal", "closed", false);
    assert_eq!(render(&sw, "State"), "[open, open, open, ]");
    assert_eq!(render(&sw, "Normal"), "[closed, closed, closed, ]");
}

// ---------------------------------------------------------------------------
// Reset (control-side and full)
// ---------------------------------------------------------------------------

#[test]
fn reset_with_restores_normal_state_and_forces_element() {
    let mut sw = SwtControl::new("sw1");
    sw.normal_state = [ControlAction::Close; ARR];
    sw.present_state = [ControlAction::Open; ARR];
    sw.current_action = ControlAction::Open;
    sw.armed = true;
    let mut ms = MockSwitch::new(3);
    ms.cd.set_terminal_closed(1, false); // start fully open
    let rebuild = sw.reset_with(&mut ms);
    assert!(rebuild); // a force was applied → caller raises SystemYChanged
    assert!(ms.cd.terminal_all_phases_closed(1)); // forced closed (NormalState)
    assert_eq!(sw.present_state[1..=SW_MAX], [ControlAction::Close; SW_MAX]);
    assert_eq!(sw.current_action, ControlAction::Close);
    assert!(!sw.armed);
    // locked → Reset is a no-op, no rebuild, controlled element untouched.
    sw.locked = true;
    sw.present_state = [ControlAction::Open; ARR];
    let mut ms2 = MockSwitch::new(3);
    assert!(!sw.reset_with(&mut ms2));
    assert_eq!(sw.present_state[1], ControlAction::Open); // untouched
    assert!(ms2.cd.terminal_all_phases_closed(1)); // not forced
}

/// Fail-on-regression guard for the partial-open dirty edge: the *old* Reset
/// gated `system_y_changed` on `terminal_all_phases_closed(was) != want` — an
/// all-or-nothing check. A **partially**-open terminal (phase 0 closed, 1&2
/// open) reads `was = "all closed" = false`; resetting to `NormalState=OPEN`
/// forces all phases open, flipping phase 0 — a real Y change — yet the old
/// check computed `was == want == false` and skipped the rebuild, leaving a
/// stale system Y (the Y build is gated solely on `system_y_changed`). The
/// per-phase rewrite keeps the unconditional rebuild.
#[test]
fn reset_with_partial_open_terminal_still_forces_rebuild() {
    let mut sw = SwtControl::new("sw1");
    sw.normal_state = [ControlAction::Open; ARR]; // reset target = open
    let mut ms = MockSwitch::new(3);
    ms.cd.terminals[0].conductors_closed[0] = true; // phase 0 still closed
    ms.cd.terminals[0].conductors_closed[1] = false;
    ms.cd.terminals[0].conductors_closed[2] = false;
    assert!(!ms.cd.terminal_all_phases_closed(1));

    let rebuild = sw.reset_with(&mut ms);
    assert!(
        rebuild,
        "reset must force a Y rebuild even from a partial-open terminal"
    );
    // The reset really flipped phase 0 closed→open — the change the old gate missed.
    assert!(!ms.cd.terminals[0].conductors_closed[0]);
}

#[test]
fn reset_yes_unlocks_and_restores_with_force() {
    // Pascal DoReset: Locked := FALSE, then Reset (restore + per-phase force).
    let mut sw = sw_with_snap(3);
    sw.locked = true;
    sw.normal_state = [ControlAction::Close; ARR];
    sw.present_state = [ControlAction::Open; ARR];
    sw.current_action = ControlAction::Open;
    sw.armed = true;
    sw.set_bool(prop::RESET, true); // Reset=yes
    assert!(!sw.locked); // unlocked first
    assert_eq!(sw.present_state[1..=SW_MAX], [ControlAction::Close; SW_MAX]);
    assert_eq!(sw.current_action, ControlAction::Close);
    assert!(!sw.armed);
    // A deferred per-phase close-force on the controlled element was queued.
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match &actions[0] {
        RefAction::SetConductorsClosed { closed, .. } => assert_eq!(closed, &vec![true; 3]),
        a => panic!("expected SetConductorsClosed, got {a:?}"),
    }
}

// ---------------------------------------------------------------------------
// MakeLike (r4133 SwtControl.pas:239-273)
// ---------------------------------------------------------------------------

#[test]
fn make_like_copies_switch_state() {
    let mut base = SwtControl::new("base");
    base.ccd.cd.nphases = 1;
    base.ccd.cd.set_nconds(1);
    base.ccd.element_terminal = 2;
    base.ccd.time_delay = 45.0;
    base.locked = true;
    base.present_state[1] = ControlAction::Open;
    base.normal_state[1] = ControlAction::Open;
    base.normal_state_set = true;
    base.current_action = ControlAction::Open;

    let mut sw = SwtControl::new("sw1");
    sw.make_like(&base);
    assert_eq!(sw.ccd.cd.nphases, 1);
    assert_eq!(sw.ccd.element_terminal, 2);
    assert_eq!(sw.ccd.time_delay, 45.0);
    assert!(sw.locked);
    assert_eq!(sw.present_state[1], ControlAction::Open);
    assert_eq!(sw.normal_state[1], ControlAction::Open);
    assert_eq!(sw.current_action, ControlAction::Open);
    // The flag copies with the arrays (the accessors' doc: r4133's omission is
    // an upstream oversight; the capi015 `swtcontrol_makelike` props golden
    // renders the carried normal). Reverting the copy makes the clone's
    // `Normal` readback render the unset view.
    assert!(sw.normal_state_set);
}

#[test]
fn make_like_fresh_clone_keeps_the_unset_normal_latch() {
    // A `like=` clone of a control that never wrote a state keeps
    // `NormalStateSet` FALSE, so the clone's own first `State=` write still
    // defaults Normal from Present (the supplemental, `:221-228`). The RENDER
    // is r4133's all-closed array either way — `Create` initializes both
    // arrays (`:299-307`), the flag only gates the copy.
    let base = SwtControl::new("base");
    let mut sw = sw_with_snap(3);
    sw.make_like(&base);
    assert!(!sw.normal_state_set);
    sw.ctrl_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["b".into()],
    });
    sw.ccd.controlled_element = Some(ElemId::new(0, 0));
    assert_eq!(render(&sw, "Normal"), "[closed, closed, closed, ]");
    edit_prop(&mut sw, "State", "open", false);
    assert_eq!(render(&sw, "Normal"), "[open, open, open, ]");
}

#[test]
fn rated_current_parses_and_reads_back() {
    // WP-U2.4 C4 (r4133 `SwtControl.pas` prop 9): informational continuous
    // rating. Untouched by the per-phase rewrite.
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.c");
    dss.command("new line.l1 bus1=b1 bus2=b2 phases=3 r1=0.3 x1=0.6 length=1 switch=y");
    dss.command("new swtcontrol.sw switchedobj=line.l1 switchedterm=1 ratedcurrent=250.5");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command("? swtcontrol.sw.ratedcurrent");
    assert_eq!(dss.result().trim().parse::<f64>().unwrap(), 250.5);
    // default is 0.0
    dss.command("new swtcontrol.sw2 switchedobj=line.l1 switchedterm=1");
    dss.command("? swtcontrol.sw2.ratedcurrent");
    assert_eq!(dss.result().trim().parse::<f64>().unwrap(), 0.0);
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemId};
    use crate::obj::base::DssObject;

    /// Pascal `TSwtControlObj.MakePosSequence` (SwtControl.pas:367-375):
    /// phases, conds and bus from the controlled (switched) element at
    /// ElementTerminal. The state arrays are NOT touched — the render resync
    /// the probe measured is purely the getter's live phase count (probe §6,
    /// A2's render bound).
    #[test]
    fn resyncs_to_controlled() {
        let mut sw = SwtControl::new("sw1");
        sw.ccd.controlled_element = Some(ElemId::new(1, 0));
        sw.ccd.element_terminal = 2;
        let ctx = PosSeqCtx {
            controlled: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                bus_names: vec!["b1".into(), "b2".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = sw.make_pos_sequence(&ctx);
        assert_eq!(sw.ccd.cd.nphases, 1);
        assert_eq!(sw.ccd.cd.nconds, 1);
        assert_eq!(sw.get_bus_name(1), "b2");
        assert!(plan.run_base && plan.actions.is_empty());
        // The per-phase state survives untouched.
        assert_eq!(sw.present_state[1..=SW_MAX], [ControlAction::Close; SW_MAX]);
    }

    #[test]
    fn nil_controlled_runs_base_only() {
        let mut sw = SwtControl::new("sw1");
        let np = sw.ccd.cd.nphases;
        let plan = sw.make_pos_sequence(&PosSeqCtx::default());
        assert_eq!(sw.ccd.cd.nphases, np);
        assert!(plan.run_base);
    }
}
