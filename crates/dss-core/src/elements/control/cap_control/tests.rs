use super::*;

use num_complex::Complex64;

use crate::elements::control::control_elem::CtrlCtx;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::DssObject;

#[test]
fn default_shape_is_3ph_1term() {
    let cc = CapControl::new("cc1");
    assert_eq!(cc.ccd.cd.nphases, 3);
    assert_eq!(cc.ccd.cd.nconds, 3);
    assert_eq!(cc.ccd.cd.nterms, 1);
    assert_eq!(cc.control_type, CapControlType::Current);
    assert_eq!(cc.pt_ratio, 60.0);
    assert_eq!(cc.ct_ratio, 60.0);
    assert_eq!(cc.on_value, 300.0);
    assert_eq!(cc.off_value, 200.0);
    assert_eq!(cc.on_delay, 15.0);
    assert_eq!(cc.off_delay, 15.0);
    assert_eq!(cc.dead_time, 300.0);
    assert_eq!(cc.vmax, 126.0);
    assert_eq!(cc.vmin, 115.0);
    assert_eq!(cc.fpct_minkvar, 50.0);
    assert!(cc.ccd.cd.yprim.is_none());
}

#[test]
fn time_control_requires_monitored_element() {
    // dss_capi `b9bc87b8`: TIMECONTROL now REQUIRES a monitored element, like
    // every non-FOLLOW type. capi015 (0.15.0b4) probe: `type=time terminal=2`
    // with no `element=` errors `CapControl.cc1: "Element" is not set,
    // aborting.` (only FOLLOWCONTROL falls back to the capacitor + terminal 1).
    // The port keeps the base 0.14.5 message form (unquoted `Element is not
    // set`); b9bc87b8 ports the guard drop, not the separate 0.15.x error-
    // quoting change — so the substring below matches only the emitted 0.14.5
    // form (the quoted form has `"Element" is not set`).
    let mut cc = CapControl::new("cc1");
    cc.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    cc.ctrl_snap = Some(RefSnapshot {
        full_name: "Capacitor.cap1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["b2.1.2.3".into(), "b2.0.0.0".into()],
    });
    cc.control_type = CapControlType::Time;
    cc.ccd.element_terminal = 2;
    cc.recalc();
    let errs = cc.ccd.cd.obj.take_errors();
    assert!(
        errs.iter().any(|e| e.contains("Element is not set")),
        "expected the aborting Element error, got {errs:?}"
    );
}

#[test]
fn time_control_uses_monitored_element_terminal() {
    // capi015 probe: `type=time element=line.l1 terminal=2` keeps Terminal = 2
    // (NOT forced to 1) and binds to the *monitored* element's terminal-2 bus
    // (effElement = MonitoredElement, `CapControl.pas:585`).
    let mut cc = CapControl::new("cc1");
    cc.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    cc.ctrl_snap = Some(RefSnapshot {
        full_name: "Capacitor.cap1".into(),
        nphases: 3,
        nterms: 1,
        buses: vec!["capbus.1.2.3".into()],
    });
    cc.mon_snap = Some(RefSnapshot {
        full_name: "Line.l1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["sb.1.2.3".into(), "b2.1.2.3".into()],
    });
    cc.control_type = CapControlType::Time;
    cc.ccd.element_terminal = 2;
    cc.recalc();
    assert_eq!(cc.ccd.element_terminal, 2);
    assert_eq!(cc.ccd.cd.get_bus(1), "b2.1.2.3");
    assert!(cc.ccd.cd.obj.take_errors().is_empty());
}

#[test]
fn pf_mode_translates_on_off_settings() {
    // Probed: type=pf onsetting=0.97 offsetting=-0.99 keeps the raw dump
    // values; internally PFON=0.97, PFOFF=2-0.99=1.01.
    let mut cc = CapControl::new("cc1");
    cc.control_type = CapControlType::Pf;
    cc.side_effects(prop::TYPE, 0);
    assert_eq!(cc.pfon_value, 0.95);
    assert_eq!(cc.pfoff_value, 1.05);
    cc.on_value = 0.97;
    cc.side_effects(prop::ONSETTING, 0);
    assert!((cc.pfon_value - 0.97).abs() < 1e-12);
    cc.off_value = -0.99;
    cc.side_effects(prop::OFFSETTING, 0);
    assert!((cc.pfoff_value - 1.01).abs() < 1e-12);
    assert_eq!(cc.on_value, 0.97);
    assert_eq!(cc.off_value, -0.99);
}

// --- Sample / DoPendingAction (WP5.6) ---

use crate::elements::ckt::CktElementData;
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

/// A controlled capacitor stub: holds the bank's step/closed state and the
/// scalars CapControl reads, with the Pascal `AddStep`/`SubtractStep`
/// semantics.
struct MockCap {
    name: String,
    num_steps: i32,
    last_step: i32,
    total_kvar: f64,
    conn: i32,
    closed: bool,
}
impl MockCap {
    fn one_step(closed: bool) -> Self {
        Self {
            name: "cc".into(),
            num_steps: 1,
            last_step: if closed { 1 } else { 0 },
            total_kvar: 600.0,
            conn: 0,
            closed,
        }
    }
}
impl ControlledCapacitor for MockCap {
    fn full_name(&self) -> String {
        format!("Capacitor.{}", self.name)
    }
    fn num_steps(&self) -> i32 {
        self.num_steps
    }
    fn available_steps(&self) -> i32 {
        self.num_steps - self.last_step
    }
    fn total_kvar(&self) -> f64 {
        self.total_kvar
    }
    fn connection(&self) -> i32 {
        self.conn
    }
    fn is_closed(&self) -> bool {
        self.closed
    }
    fn set_closed(&mut self, value: bool) {
        self.closed = value;
    }
    fn add_step(&mut self) -> bool {
        if self.last_step == self.num_steps {
            false
        } else {
            self.last_step += 1;
            true
        }
    }
    fn subtract_step(&mut self) -> bool {
        if self.last_step == 0 {
            false
        } else {
            self.last_step -= 1;
            self.last_step != 0
        }
    }
}

/// A monitored element stub returning canned measurements, so the CapControl
/// decision logic is testable without a node-wired circuit.
struct MockMon {
    cd: CktElementData,
    power: Complex64,
    currents: Vec<Complex64>,
    voltages: Vec<Complex64>,
}
impl MockMon {
    fn new(nphases: usize) -> Self {
        let mut cd = CktElementData::new("mon", 1);
        cd.nphases = nphases;
        cd.nconds = nphases;
        cd.set_nterms(1);
        cd.yorder = nphases;
        Self {
            cd,
            power: Complex64::ZERO,
            currents: Vec::new(),
            voltages: Vec::new(),
        }
    }
}
impl CktElement for MockMon {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }
    fn recalc_element_data(&mut self, _sys: &SysCtx) {}
    fn calc_yprim(&mut self, _sys: &SysCtx) {}
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        for (i, c) in curr.iter_mut().enumerate() {
            *c = self.currents.get(i).copied().unwrap_or(Complex64::ZERO);
        }
    }
    fn terminal_power(
        &mut self,
        _sys: &SysCtx,
        _node_v: &[Complex64],
        _idx_term: usize,
    ) -> Complex64 {
        self.power
    }
    fn get_term_voltages(&self, _iterm: usize, _node_v: &[Complex64], vbuffer: &mut [Complex64]) {
        for (i, v) in vbuffer.iter_mut().enumerate() {
            *v = self.voltages.get(i).copied().unwrap_or(Complex64::ZERO);
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
    fn ctx(&mut self, control_mode: i32, int_hour: i32, t: f64) -> CtrlCtx<'_> {
        CtrlCtx {
            node_v: &[],
            sys: &self.sys,
            queue: &mut self.queue,
            events: &mut self.events,
            errors: &mut self.errors,
            system_y_changed: &mut self.y_changed,
            control_mode,
            int_hour,
            t,
            dbl_hour: int_hour as f64 + t / 3600.0,
            control_iter: 1,
            self_ref: ElemRef { cls: 0, idx: 0 },
        }
    }
}

#[test]
fn kvar_open_arms_close_above_onsetting() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Kvar;
    cc.on_value = 150.0;
    cc.off_value = -225.0;
    let mut cap = MockCap::one_step(false); // bank open → PresentState OPEN
    let mut mon = MockMon::new(3);
    mon.power = Complex64::new(0.0, 200_000.0); // 200 kvar inductive
    let mut sc = Scratch::new();
    let _ = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert_eq!(cc.pending_change, CTRL_CLOSE);
    assert!(cc.should_switch);
    assert!(cc.armed);
    assert_eq!(sc.queue.queue_size(), 1);
    // pending CLOSE; dead time already elapsed → ONDelay.
    assert_eq!(cc.ccd.time_delay, 15.0);
}

#[test]
fn kvar_closed_arms_open_below_offsetting() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Kvar;
    cc.on_value = 150.0;
    cc.off_value = -225.0;
    let mut cap = MockCap::one_step(true); // bank closed → PresentState CLOSE
    let mut mon = MockMon::new(3);
    mon.power = Complex64::new(0.0, -300_000.0); // -300 kvar (too leading)
    let mut sc = Scratch::new();
    let _ = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert_eq!(cc.pending_change, CTRL_OPEN);
    assert!(cc.armed);
    assert_eq!(cc.ccd.time_delay, 15.0); // OFFDelay
}

#[test]
fn kvar_in_band_does_not_switch() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Kvar;
    cc.on_value = 150.0;
    cc.off_value = -225.0;
    let mut cap = MockCap::one_step(true);
    let mut mon = MockMon::new(3);
    mon.power = Complex64::new(0.0, -50_000.0); // -50 kvar: between off and on
    let mut sc = Scratch::new();
    let _ = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert_eq!(cc.pending_change, CTRL_NONE);
    assert!(!cc.armed);
    assert!(sc.queue.is_empty());
}

#[test]
fn do_pending_open_single_step_opens_bank() {
    let mut cc = CapControl::new("cc");
    cc.present_state = CTRL_CLOSE;
    cc.set_pending_change(CTRL_OPEN);
    cc.armed = true;
    let mut cap = MockCap::one_step(true);
    let mut sc = Scratch::new();
    cc.do_pending_action(&mut cap, &mut sc.ctx(0, 0, 0.0));
    assert!(!cap.closed);
    assert_eq!(cap.last_step, 0);
    assert_eq!(cc.present_state, CTRL_OPEN);
    assert!(sc.y_changed);
    assert!(!cc.armed);
}

#[test]
fn do_pending_close_single_step_closes_bank() {
    let mut cc = CapControl::new("cc");
    cc.present_state = CTRL_OPEN;
    cc.set_pending_change(CTRL_CLOSE);
    cc.armed = true;
    let mut cap = MockCap::one_step(false);
    let mut sc = Scratch::new();
    cc.do_pending_action(&mut cap, &mut sc.ctx(0, 0, 0.0));
    assert!(cap.closed);
    assert_eq!(cap.last_step, 1);
    assert_eq!(cc.present_state, CTRL_CLOSE);
    assert!(sc.y_changed);
}

#[test]
fn do_pending_open_multistep_steps_down() {
    let mut cc = CapControl::new("cc");
    cc.present_state = CTRL_CLOSE;
    cc.set_pending_change(CTRL_OPEN);
    let mut cap = MockCap {
        name: "cc".into(),
        num_steps: 4,
        last_step: 4,
        total_kvar: 1200.0,
        conn: 0,
        closed: true,
    };
    let mut sc = Scratch::new();
    cc.do_pending_action(&mut cap, &mut sc.ctx(0, 0, 0.0));
    // One step down: still partly closed, bank stays Closed.
    assert_eq!(cap.last_step, 3);
    assert!(cap.closed);
    assert_eq!(cc.present_state, CTRL_CLOSE);
    assert!(sc.y_changed);
}

#[test]
fn time_control_closes_inside_window() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Time;
    cc.on_value = 6.0;
    cc.off_value = 21.0;
    let mut cap = MockCap::one_step(false); // open at start
    let mut mon = MockMon::new(3);
    let mut sc = Scratch::new();
    // 12:00 is inside [6, 21) → close.
    let _ = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 12, 0.0));
    assert_eq!(cc.pending_change, CTRL_CLOSE);
    assert!(cc.armed);
}

#[test]
fn pf_control_closes_when_leading_room_remains() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Pf;
    cc.pfon_value = 0.95;
    cc.fpct_minkvar = 50.0;
    let mut cap = MockCap::one_step(false); // open
    cap.total_kvar = 50.0;
    let mut mon = MockMon::new(3);
    // 100 kW + 50 kvar → PF1to2 = 0.894 < 0.95; 50 kvar > 50·50·0.01 = 25.
    mon.power = Complex64::new(100_000.0, 50_000.0);
    let mut sc = Scratch::new();
    let _ = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert_eq!(cc.pending_change, CTRL_CLOSE);
    assert!(cc.armed);
}

#[test]
fn event_log_records_close_when_enabled() {
    let mut cc = CapControl::new("cc");
    cc.ccd.show_event_log = true;
    cc.present_state = CTRL_OPEN;
    cc.set_pending_change(CTRL_CLOSE);
    let mut cap = MockCap::one_step(false);
    let mut sc = Scratch::new();
    cc.do_pending_action(&mut cap, &mut sc.ctx(0, 0, 0.0));
    assert_eq!(sc.events.len(), 1);
    let line = &sc.events.entries()[0];
    assert!(line.contains("Element=Capacitor.cc"));
    assert!(line.contains("**CLOSED**"));
}

// --- FOLLOWCONTROL sample arm (CF-C Port 1; CapControl.pas Sample l.1151) ---

use crate::elements::general::load_shape::LoadShapeObj;

/// FOLLOW with no ControlSignal aborts the solve (Pascal `DoSimpleMsg` 10362 +
/// `DSS.SolutionAbort := True`): `sample` returns `true` (the dispatcher lifts
/// it to `solution_abort`), records the message, and does not switch.
#[test]
fn follow_without_control_signal_requests_abort() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Follow;
    cc.ctrl_signal_shape = None;
    let mut cap = MockCap::one_step(true);
    let mut mon = MockMon::new(3);
    let mut sc = Scratch::new();
    let abort = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert!(abort, "no ControlSignal must request solution abort");
    assert!(!cc.should_switch);
    assert_eq!(sc.errors.len(), 1);
    assert!(sc.errors[0].contains("Aborting solution"));
    assert!(sc.errors[0].contains("Follow"));
}

/// FOLLOW signal nonzero (wants ON) while the bank is OPEN → arm CLOSE.
#[test]
fn follow_signal_on_arms_close_when_open() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Follow;
    cc.ctrl_signal_shape = Some(LoadShapeObj::fixed_interval_for_test("s", 1.0, vec![1.0]));
    let mut cap = MockCap::one_step(false); // bank open → PresentState OPEN
    let mut mon = MockMon::new(3);
    let mut sc = Scratch::new();
    let abort = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert!(!abort);
    assert_eq!(cc.pending_change, CTRL_CLOSE);
    assert!(cc.should_switch);
    assert!(cc.armed);
    assert_eq!(cc.ccd.time_delay, 15.0); // ONDelay
}

/// FOLLOW signal zero (wants OFF) while the bank is CLOSED → arm OPEN.
#[test]
fn follow_signal_off_arms_open_when_closed() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Follow;
    cc.ctrl_signal_shape = Some(LoadShapeObj::fixed_interval_for_test("s", 1.0, vec![0.0]));
    let mut cap = MockCap::one_step(true); // bank closed → PresentState CLOSE
    let mut mon = MockMon::new(3);
    let mut sc = Scratch::new();
    let _ = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert_eq!(cc.pending_change, CTRL_OPEN);
    assert!(cc.should_switch);
    assert_eq!(cc.ccd.time_delay, 15.0); // OFFDelay
}

/// FOLLOW signal matching the present state does NOT switch, and — unlike every
/// other control type — leaves `PendingChange` untouched (Pascal has no `else`
/// resetting it to CTRL_NONE in the FOLLOW arm). Seed a non-NONE pending and
/// assert it survives.
#[test]
fn follow_signal_matching_state_leaves_pending_untouched() {
    let mut cc = CapControl::new("cc");
    cc.control_type = CapControlType::Follow;
    cc.ctrl_signal_shape = Some(LoadShapeObj::fixed_interval_for_test("s", 1.0, vec![1.0]));
    cc.set_pending_change(CTRL_CLOSE); // sentinel that the FOLLOW arm must not clear
    let mut cap = MockCap::one_step(true); // closed; signal wants ON → no switch
    let mut mon = MockMon::new(3);
    let mut sc = Scratch::new();
    let _ = cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert!(!cc.should_switch);
    // The no-`else` quirk: pending stays CTRL_CLOSE (not reset to CTRL_NONE).
    // With should_switch false and pending != NONE, the arm/disarm block also
    // leaves the queue empty (armed was false).
    assert_eq!(cc.pending_change, CTRL_CLOSE);
    assert!(!cc.armed);
    assert!(sc.queue.is_empty());
}

#[test]
fn pf_1to2_maps_leading_above_one() {
    // Lagging (im>0): PF in [0,1]. Leading (im<0): PF in [1,2].
    assert!((pf_1to2(Complex64::new(100.0, 0.0)) - 1.0).abs() < 1e-12);
    assert!((pf_1to2(Complex64::ZERO) - 1.0).abs() < 1e-12);
    let lag = pf_1to2(Complex64::new(80.0, 60.0)); // 0.8 lagging
    assert!((lag - 0.8).abs() < 1e-12);
    let lead = pf_1to2(Complex64::new(80.0, -60.0)); // 0.8 leading → 1.2
    assert!((lead - 1.2).abs() < 1e-12);
}

/// A capacitor stub with *per-phase* conductor state, so the partial-open dirty
/// edge in `reset_with` is representable (the production [`MockCap`] collapses
/// the bank to a single bool and so cannot express a partially-closed terminal).
struct MockPhaseCap {
    conductors: [bool; 3],
}
impl ControlledCapacitor for MockPhaseCap {
    fn full_name(&self) -> String {
        "Capacitor.pc".into()
    }
    fn num_steps(&self) -> i32 {
        1
    }
    fn available_steps(&self) -> i32 {
        0
    }
    fn total_kvar(&self) -> f64 {
        600.0
    }
    fn connection(&self) -> i32 {
        0
    }
    fn is_closed(&self) -> bool {
        self.conductors.iter().all(|&c| c) // "all phases closed?"
    }
    fn set_closed(&mut self, value: bool) {
        self.conductors = [value; 3];
    }
    fn add_step(&mut self) -> bool {
        false
    }
    fn subtract_step(&mut self) -> bool {
        false
    }
}

/// Fail-on-regression guard for the partial-open dirty edge in `reset_with` (the
/// same one fixed for SwtControl). The *old* code gated `SystemYChanged` on
/// `want != was_closed`, where `was_closed = is_closed()` is the all-or-nothing
/// "are all phases closed?". A bank with phase 0 closed and 1&2 open reads
/// `is_closed() = false`; with `InitialState = OPEN`, reset forces all phases
/// open — flipping phase 0 (a real Y change) — yet the old check computed
/// `want(false) == was(false)` and skipped the rebuild, leaving a stale system Y
/// (the Y build is gated solely on `SystemYChanged`). Pascal `Reset` does
/// `Closed[0] := FALSE` unconditionally; `reset_with` now reports the rebuild
/// whenever a force is applied.
///
/// (Reintroducing the `want != was_closed` gate makes `reset_with` return
/// `false` here, failing `assert!(rebuild)`.)
#[test]
fn reset_with_partial_open_bank_still_forces_rebuild() {
    let mut cc = CapControl::new("cc");
    cc.initial_state = CTRL_OPEN; // reset target = open
    let mut cap = MockPhaseCap {
        conductors: [true, false, false], // phase 0 still closed
    };
    // The old gate's premise — "all phases closed?" — is already false, even
    // though phase 0 IS closed: exactly where it misfires.
    assert!(!cap.is_closed());

    let rebuild = cc.reset_with(&mut cap);
    assert!(
        rebuild,
        "reset must force a Y rebuild even from a partial-open bank"
    );
    // The reset really flipped phase 0 closed→open — the change the old gate missed.
    assert!(!cap.conductors[0]);
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemRef};
    use crate::obj::base::DssObject;

    /// Pascal `TCapControlObj.MakePosSequence` (CapControl.pas:643): Enabled /
    /// phases / conds from the controlled cap; effElement = monitored (when set)
    /// supplies the terminal bus. Cross-check `makeposseq_ctrl.dss`: phases=1.
    #[test]
    fn resyncs_controlled_and_monitored_bus() {
        let mut cc = CapControl::new("cc1");
        cc.ccd.controlled_element = Some(ElemRef { cls: 1, idx: 1 });
        cc.ccd.monitored_element = Some(ElemRef { cls: 2, idx: 2 });
        cc.ccd.element_terminal = 2;
        let ctx = PosSeqCtx {
            controlled: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                enabled: false,
                bus_names: vec!["cbus".into()],
                ..Default::default()
            }),
            monitored: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                enabled: true,
                bus_names: vec!["mb1".into(), "mb2".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = cc.make_pos_sequence(&ctx);
        assert_eq!(cc.ccd.cd.nphases, 1);
        assert_eq!(cc.ccd.cd.nconds, 1);
        assert!(!cc.ccd.cd.enabled); // Enabled := ControlledElement.Enabled
        assert_eq!(cc.get_bus_name(1), "mb2"); // effElement=monitored, GetBus(2)
        assert_eq!(cc.ccd.element_terminal, 2); // unchanged (monitored present)
        assert!(plan.run_base && plan.actions.is_empty());
        assert_eq!(cc.monitored_element_ref(), Some(ElemRef { cls: 2, idx: 2 }));
    }

    /// No monitored element ⇒ effElement = controlled, ElementTerminal forced 1.
    #[test]
    fn no_monitored_forces_terminal_one() {
        let mut cc = CapControl::new("cc1");
        cc.ccd.controlled_element = Some(ElemRef { cls: 1, idx: 1 });
        cc.ccd.monitored_element = None;
        cc.ccd.element_terminal = 3;
        let ctx = PosSeqCtx {
            controlled: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                enabled: true,
                bus_names: vec!["cbus".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        cc.make_pos_sequence(&ctx);
        assert_eq!(cc.ccd.element_terminal, 1); // forced to 1
        assert_eq!(cc.get_bus_name(1), "cbus"); // controlled GetBus(1)
    }
}

#[test]
fn cap_control_type_pins_enum_ordinals() {
    for (variant, ord) in [
        (CapControlType::Current, 0),
        (CapControlType::Voltage, 1),
        (CapControlType::Kvar, 2),
        (CapControlType::Time, 3),
        (CapControlType::Pf, 4),
        (CapControlType::Follow, 5),
    ] {
        assert_eq!(variant.ordinal(), ord);
        assert_eq!(CapControlType::from_ordinal(ord), Some(variant));
    }
    assert_eq!(CapControlType::from_ordinal(6), None);
    assert_eq!(CapControlType::from_ordinal(-1), None);
}

#[test]
fn set_i32_type_keeps_value_on_unregistered_ordinal() {
    // USERCONTROL=6 is unregistered (only reached via a user-model DLL, which is
    // NOT_PORTED), so the setter can never see it; pin the deliberate keep-old
    // fallback `from_ordinal(value).unwrap_or(self.control_type)` regardless.
    let mut cc = CapControl::new("cc1");
    cc.set_i32(prop::TYPE, CapControlType::Kvar.ordinal()); // 2 -> Kvar
    assert_eq!(cc.control_type, CapControlType::Kvar);
    cc.set_i32(prop::TYPE, 6); // unregistered USERCONTROL ordinal -> keep Kvar
    assert_eq!(cc.control_type, CapControlType::Kvar);
    assert_eq!(cc.get_i32(prop::TYPE), 2);
}
