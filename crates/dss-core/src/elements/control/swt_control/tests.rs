use super::*;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::control::control_elem::{CTRL_CLOSE, CTRL_LOCK, CTRL_NONE, CTRL_OPEN};
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

/// A minimal switched element (a 2-terminal PD device) so `DoPendingAction`'s
/// `set_terminal_closed` has real terminals to flip.
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
    fn recalc_element_data(&mut self, _sys: &SysCtx) {}
    fn calc_yprim(&mut self, _sys: &SysCtx) {}
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
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

#[test]
fn default_is_3ph_closed_switch() {
    let sw = SwtControl::new("sw1");
    assert_eq!(sw.ccd.cd.nphases, 3);
    assert_eq!(sw.ccd.cd.nconds, 3);
    assert_eq!(sw.ccd.cd.nterms, 1);
    assert_eq!(sw.ccd.element_terminal, 1);
    assert_eq!(sw.ccd.time_delay, 120.0);
    assert_eq!(sw.present_state, CTRL_CLOSE);
    assert_eq!(sw.normal_state, CTRL_NONE);
    assert_eq!(sw.current_action, CTRL_CLOSE);
    assert!(!sw.locked);
    assert!(!sw.armed);
    assert!(sw.ccd.cd.yprim.is_none());
}

#[test]
fn sample_arms_when_action_differs() {
    let mut sw = SwtControl::new("sw1");
    sw.current_action = CTRL_OPEN; // commanded open, switch still closed
    let mut sc = Scratch::new();
    sw.sample(&mut sc.ctx(0, 0.0));
    assert!(sw.armed);
    assert_eq!(sc.queue.queue_size(), 1);
}

#[test]
fn sample_does_not_arm_when_matching() {
    let mut sw = SwtControl::new("sw1");
    // current_action == present_state (both CLOSE) → nothing to do.
    let mut sc = Scratch::new();
    sw.sample(&mut sc.ctx(0, 0.0));
    assert!(!sw.armed);
    assert!(sc.queue.is_empty());
}

#[test]
fn do_pending_open_opens_terminal_and_logs() {
    let mut sw = SwtControl::new("sw1");
    sw.present_state = CTRL_CLOSE;
    let mut ms = MockSwitch::new(3);
    let mut sc = Scratch::new();
    sw.do_pending_action(CTRL_OPEN, &mut ms, &mut sc.ctx(0, 1.0));
    assert!(!ms.cd.terminal_all_phases_closed(1)); // terminal 1 opened
    assert_eq!(sw.present_state, CTRL_OPEN);
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
    sw.present_state = CTRL_OPEN;
    let mut ms = MockSwitch::new(3);
    ms.cd.set_terminal_closed(1, false); // start open
    let mut sc = Scratch::new();
    sw.do_pending_action(CTRL_CLOSE, &mut ms, &mut sc.ctx(0, 1.0));
    assert!(ms.cd.terminal_all_phases_closed(1));
    assert_eq!(sw.present_state, CTRL_CLOSE);
    assert!(sc.y_changed);
    assert!(sc.events.entries()[0].contains("Action=CLOSED"));
}

#[test]
fn do_pending_lock_then_open_is_blocked() {
    let mut sw = SwtControl::new("sw1");
    sw.present_state = CTRL_CLOSE;
    let mut ms = MockSwitch::new(3);
    let mut sc = Scratch::new();
    // Lock first, then an open action must be ignored (still closed, no event).
    sw.do_pending_action(CTRL_LOCK, &mut ms, &mut sc.ctx(0, 0.0));
    assert!(sw.locked);
    sw.do_pending_action(CTRL_OPEN, &mut ms, &mut sc.ctx(0, 1.0));
    assert!(ms.cd.terminal_all_phases_closed(1)); // still closed
    assert_eq!(sw.present_state, CTRL_CLOSE);
    assert!(sc.events.is_empty());
}

#[test]
fn do_pending_sets_controlled_active_terminal_even_for_lock() {
    // Pascal sets ControlledElement.ActiveTerminalIdx := ElementTerminal before
    // the case — for every code, including LOCK (which touches no conductor).
    let mut sw = SwtControl::new("sw1");
    sw.ccd.element_terminal = 2;
    let mut ms = MockSwitch::new(3); // 2 terminals
    let mut sc = Scratch::new();
    sw.do_pending_action(CTRL_LOCK, &mut ms, &mut sc.ctx(0, 0.0));
    assert!(sw.locked);
    assert_eq!(ms.cd.active_terminal, 1); // terminal 2, 0-based
}

#[test]
fn lock_side_effect_queues_lock_command_pushed_on_sample() {
    let mut sw = SwtControl::new("sw1");
    sw.locked = true;
    sw.side_effects(prop::LOCK, 0);
    assert_eq!(sw.lock_command, CTRL_LOCK);
    let mut sc = Scratch::new();
    sw.sample(&mut sc.ctx(0, 0.0));
    assert_eq!(sw.lock_command, CTRL_NONE); // consumed
    assert_eq!(sc.queue.queue_size(), 1);
}

#[test]
fn locked_ignores_action_write() {
    // ConditionalReadOnly on Locked: a write to Action while locked is dropped.
    let mut sw = SwtControl::new("sw1");
    sw.locked = true;
    sw.set_i32(prop::ACTION, CTRL_OPEN);
    assert_eq!(sw.current_action, CTRL_CLOSE); // unchanged
    // unlocked: the write lands.
    sw.locked = false;
    sw.set_i32(prop::ACTION, CTRL_OPEN);
    assert_eq!(sw.current_action, CTRL_OPEN);
}

#[test]
fn normal_side_effect_syncs_current_action_from_normal_state() {
    // D12 (WP-U1.6, `bb9c9785`): `Normal=` writes `NormalState` (offset), then
    // the side effect syncs `CurrentAction := NormalState` (0.14.5 did the
    // reverse, `NormalState := CurrentAction`). Crucially, `Normal=` no longer
    // clobbers `PresentState` — the field the old shared-`CurrentAction` mapping
    // conflated.
    let mut sw = SwtControl::new("sw1");
    sw.set_i32(prop::NORMAL, CTRL_OPEN); // offset write → NormalState
    sw.side_effects(prop::NORMAL, 0);
    assert_eq!(sw.normal_state, CTRL_OPEN);
    assert_eq!(sw.current_action, CTRL_OPEN); // synced from NormalState
    assert_eq!(sw.present_state, CTRL_CLOSE); // untouched (D12 fix)
}

#[test]
fn state_side_effect_sets_present_and_queues_force() {
    // D12: `State=` writes `PresentState` (offset), then the side effect syncs
    // `CurrentAction := PresentState` and forces the controlled element.
    let mut sw = SwtControl::new("sw1");
    sw.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    sw.set_i32(prop::STATE, CTRL_OPEN); // offset write → PresentState
    sw.side_effects(prop::STATE, 0);
    assert_eq!(sw.present_state, CTRL_OPEN);
    assert_eq!(sw.current_action, CTRL_OPEN); // synced from PresentState
    assert_eq!(sw.normal_state, CTRL_OPEN); // was CTRL_NONE
    // A deferred element force (open) was queued.
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match actions[0] {
        crate::obj::base::RefAction::SetSwitchClosed {
            closed, terminal, ..
        } => {
            assert!(!closed);
            assert_eq!(terminal, 1);
        }
        _ => panic!("expected SetSwitchClosed"),
    }
}

#[test]
fn d12_normal_and_state_readbacks_are_independent() {
    // D12 feature-sensitivity: 0.14.5 mapped `Action`/`Normal`/`State` all onto
    // the single `CurrentAction`, so a write to one changed the others' readback.
    // The fix gives each its own field. Set `State=open` then `Normal=closed`;
    // both readbacks must survive independently (0.14.5 → both `closed`).
    let mut sw = SwtControl::new("sw1");
    sw.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    sw.set_i32(prop::STATE, CTRL_OPEN);
    sw.side_effects(prop::STATE, 0);
    sw.take_ref_actions();
    sw.set_i32(prop::NORMAL, CTRL_CLOSE);
    sw.side_effects(prop::NORMAL, 0);
    assert_eq!(sw.get_i32(prop::STATE), CTRL_OPEN);
    assert_eq!(sw.get_i32(prop::NORMAL), CTRL_CLOSE);
}

#[test]
fn d6_action_forces_present_state_and_element_like_state() {
    // WP-U2.4 D6 (EPRI r4133 `SwtControl.pas` `InterpretSwitchState`): the
    // deprecated `Action` now sets the ACTUAL state — like `State` — instead of
    // only the normal state, and fires the same first-set normal-default side
    // effect. Probed on r4133: `New SwtControl.x action=open` → state reads
    // `[open, open, open, ]` (immediately forced), and (no prior `normal`) normal
    // defaults to `[open, open, open, ]`.
    let mut sw = SwtControl::new("sw1");
    sw.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    sw.set_i32(prop::ACTION, CTRL_OPEN); // offset write → CurrentAction
    sw.side_effects(prop::ACTION, 0);
    assert_eq!(sw.present_state, CTRL_OPEN); // D6: Action forces present state
    assert_eq!(sw.current_action, CTRL_OPEN);
    assert_eq!(sw.normal_state, CTRL_OPEN); // first-set default (was CTRL_NONE)
    // A deferred element force (open) was queued — the switch operates now, with
    // no control-queue delay (r4133 has no Sample-time queue for Action).
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match actions[0] {
        crate::obj::base::RefAction::SetSwitchClosed { closed, .. } => assert!(!closed),
        _ => panic!("expected SetSwitchClosed(open)"),
    }
}

#[test]
fn d6_action_after_declared_normal_leaves_normal_unchanged() {
    // r4133-probed: `New SwtControl.x normal=closed` then `action=open` leaves
    // `normal=[closed, closed, closed, ]` (the first-set default already fired on
    // `normal=`) while `state` flips to `[open, open, open, ]`. Mirrors the
    // civanlar/swtcontrol_time pattern.
    let mut sw = SwtControl::new("sw1");
    sw.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    // normal=closed → NormalState set, so NormalStateSet is effectively TRUE.
    sw.set_i32(prop::NORMAL, CTRL_CLOSE);
    sw.side_effects(prop::NORMAL, 0);
    // action=open forces the present state open, normal stays closed.
    sw.set_i32(prop::ACTION, CTRL_OPEN);
    sw.side_effects(prop::ACTION, 0);
    assert_eq!(sw.present_state, CTRL_OPEN);
    assert_eq!(sw.normal_state, CTRL_CLOSE); // unchanged — not defaulted again
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1); // only the action's open force
    match actions[0] {
        crate::obj::base::RefAction::SetSwitchClosed { closed, .. } => assert!(!closed),
        _ => panic!("expected SetSwitchClosed(open)"),
    }
}

#[test]
fn d6_locked_action_does_not_force_element() {
    // r4133-probed: with `lock=yes`, `action=open` is ignored (the
    // InterpretSwitchState `if Locked and (property in {a,s}) then Exit`) — the
    // switch stays closed, no element force queued.
    let mut sw = SwtControl::new("sw1");
    sw.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    sw.locked = true;
    sw.set_i32(prop::ACTION, CTRL_OPEN); // ignored (ConditionalReadOnly)
    sw.side_effects(prop::ACTION, 0); // early-return on locked
    assert_eq!(sw.present_state, CTRL_CLOSE); // untouched
    assert_eq!(sw.current_action, CTRL_CLOSE);
    assert!(sw.take_ref_actions().is_empty()); // no force
}

#[test]
fn rated_current_parses_and_reads_back() {
    // WP-U2.4 C4 (r4133 `SwtControl.pas` prop 9): informational continuous
    // rating. `New SwtControl.x ratedcurrent=250.5` stores 250.5; default 0.0.
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

#[test]
fn reset_with_restores_normal_state_and_forces_element() {
    let mut sw = SwtControl::new("sw1");
    sw.normal_state = CTRL_CLOSE;
    sw.present_state = CTRL_OPEN;
    sw.current_action = CTRL_OPEN;
    sw.armed = true;
    let mut ms = MockSwitch::new(3);
    ms.cd.set_terminal_closed(1, false); // start fully open
    let rebuild = sw.reset_with(&mut ms);
    assert!(rebuild); // a force was applied → caller raises SystemYChanged
    assert!(ms.cd.terminal_all_phases_closed(1)); // forced closed (NormalState)
    assert_eq!(sw.present_state, CTRL_CLOSE);
    assert_eq!(sw.current_action, CTRL_CLOSE);
    assert!(!sw.armed);
    // locked → Reset is a no-op, no rebuild, controlled element untouched.
    sw.locked = true;
    sw.present_state = CTRL_OPEN;
    let mut ms2 = MockSwitch::new(3);
    assert!(!sw.reset_with(&mut ms2));
    assert_eq!(sw.present_state, CTRL_OPEN); // untouched
    assert!(ms2.cd.terminal_all_phases_closed(1)); // not forced
}

/// Fail-on-regression guard for the partial-open dirty edge: the *old* Reset
/// gated `system_y_changed` on `terminal_all_phases_closed(was) != want` — an
/// all-or-nothing check. A **partially**-open terminal (phase 0 closed, 1&2
/// open) reads `was = "all closed" = false`; resetting to `NormalState=OPEN`
/// forces all phases open, flipping phase 0 — a real Y change — yet the old
/// check computed `was == want == false` and skipped the rebuild, leaving a
/// stale system Y (the Y build is gated solely on `system_y_changed`). The
/// fixed `reset_with` reports the rebuild unconditionally.
///
/// (Reintroducing the `was != want` gate makes `reset_with` return `false`
/// here, failing the `assert!(rebuild)` — this test would catch that.)
#[test]
fn reset_with_partial_open_terminal_still_forces_rebuild() {
    let mut sw = SwtControl::new("sw1");
    sw.normal_state = CTRL_OPEN; // reset target = open
    let mut ms = MockSwitch::new(3);
    ms.cd.terminals[0].conductors_closed[0] = true; // phase 0 still closed
    ms.cd.terminals[0].conductors_closed[1] = false;
    ms.cd.terminals[0].conductors_closed[2] = false;
    // The old gate's premise — "are all phases closed?" — is already false,
    // even though phase 0 IS closed: this is exactly where it misfires.
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
fn locked_ignores_normal_and_state_writes() {
    // ConditionalReadOnly on Locked applies to Normal and State too (not just
    // Action): a locked write is dropped and its side effect is skipped.
    let mut sw = SwtControl::new("sw1");
    sw.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    sw.locked = true;
    // Normal: write rejected, NormalState untouched (stays CTRL_NONE).
    sw.set_i32(prop::NORMAL, CTRL_OPEN);
    sw.side_effects(prop::NORMAL, 0);
    assert_eq!(sw.current_action, CTRL_CLOSE);
    assert_eq!(sw.normal_state, CTRL_NONE);
    // State: write rejected, no PresentState change and no deferred force queued.
    sw.set_i32(prop::STATE, CTRL_OPEN);
    sw.side_effects(prop::STATE, 0);
    assert_eq!(sw.current_action, CTRL_CLOSE);
    assert_eq!(sw.present_state, CTRL_CLOSE);
    assert!(sw.take_ref_actions().is_empty());
}

#[test]
fn reset_yes_unlocks_and_restores_with_force() {
    // Pascal DoReset: Locked := FALSE, then Reset (restore + element force).
    let mut sw = SwtControl::new("sw1");
    sw.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    sw.locked = true;
    sw.normal_state = CTRL_CLOSE;
    sw.present_state = CTRL_OPEN;
    sw.current_action = CTRL_OPEN;
    sw.armed = true;
    sw.set_bool(prop::RESET, true); // Reset=yes
    assert!(!sw.locked); // unlocked first
    assert_eq!(sw.present_state, CTRL_CLOSE);
    assert_eq!(sw.current_action, CTRL_CLOSE);
    assert!(!sw.armed);
    // A deferred close-force on the controlled element was queued.
    let actions = sw.take_ref_actions();
    assert_eq!(actions.len(), 1);
    match actions[0] {
        crate::obj::base::RefAction::SetSwitchClosed { closed, .. } => assert!(closed),
        _ => panic!("expected SetSwitchClosed"),
    }
}

#[test]
fn make_like_copies_switch_state() {
    let mut base = SwtControl::new("base");
    base.ccd.cd.nphases = 1;
    base.ccd.cd.set_nconds(1);
    base.ccd.element_terminal = 2;
    base.ccd.time_delay = 45.0;
    base.locked = true;
    base.present_state = CTRL_OPEN;
    base.normal_state = CTRL_OPEN;
    base.current_action = CTRL_OPEN;

    let mut sw = SwtControl::new("sw1");
    sw.make_like(&base);
    assert_eq!(sw.ccd.cd.nphases, 1);
    assert_eq!(sw.ccd.element_terminal, 2);
    assert_eq!(sw.ccd.time_delay, 45.0);
    assert!(sw.locked);
    assert_eq!(sw.present_state, CTRL_OPEN);
    assert_eq!(sw.normal_state, CTRL_OPEN);
    assert_eq!(sw.current_action, CTRL_OPEN);
}

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
        let re = s.currents[2 * k];
        let im = s.currents[2 * k + 1];
        m = m.max((re * re + im * im).sqrt());
    }
    m
}

/// WP-U2.4 D6 (EPRI r4133 `SwtControl.pas`): the deprecated `Action=open` now
/// forces the switched element open **immediately at parse time** — like
/// `State=open` — with NO control-queue delay and NO `OPENED` event. This is the
/// r4088/0.14.5 → r4133 flip (was: queued, opens after `delay`, logs `OPENED`).
/// Probed on r4133: after `edit swtcontrol.sw1 action=open` (even with `delay=2`)
/// the switch is open on the very next solve and the event log is empty.
/// Feature-sensitive: if `Action` regressed to setting only the normal state
/// (r4088), l1 would stay closed and share current with l2.
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
    // l1 (the switch) is open at parse → near-zero current; l2 carries the load.
    assert!(
        term1_max_current(&mut dss, "Line.l1") < 1.0,
        "action=open should force the switch open at parse (D6)"
    );
    assert!(
        term1_max_current(&mut dss, "Line.l2") > 1.0,
        "parallel line should carry the load"
    );
    // No control-queue OPENED event — the switch operates at edit time, not via
    // Sample/DoPendingAction (r4133 Sample is inert).
    assert!(
        !dss.event_log().iter().any(|s| s.contains("Action=OPENED")),
        "D6: action forces at parse, so no queued OPENED event; log = {:?}",
        dss.event_log()
    );
}

/// `State=open` forces the switched element open at parse time (the deferred
/// `SetSwitchClosed` RefAction), so a plain snapshot solve already shows the
/// line open without the control loop acting (PresentState == CurrentAction).
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
/// switch to `NormalState` and re-forces the controlled element. With
/// `normal=closed`, a line opened via `state=open` is re-closed by `reset`
/// (oracle-probed: `reset` ⇒ l1 closed again).
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
    // Reset restores to NormalState (closed) and re-forces the controlled line.
    dss.command("reset");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "engine errors: {:?}", dss.errors());
    assert!(
        term1_max_current(&mut dss, "Line.l1") > 1.0,
        "reset (normal=closed) should re-close the switched line"
    );
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemRef};
    use crate::obj::base::DssObject;

    /// Pascal `TSwtControlObj.MakePosSequence` (SwtControl.pas:306): phases/conds
    /// + bus from the controlled (switched) element at ElementTerminal.
    #[test]
    fn resyncs_to_controlled() {
        let mut sw = SwtControl::new("sw1");
        sw.ccd.controlled_element = Some(ElemRef { cls: 1, idx: 0 });
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
