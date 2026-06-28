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
    assert_eq!(cc.control_type, 0); // Current
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
fn time_control_forces_terminal_1() {
    // Probed: `type=time terminal=2` dumps Terminal = 1.
    let mut cc = CapControl::new("cc1");
    cc.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    cc.ctrl_snap = Some(RefSnapshot {
        full_name: "Capacitor.cap1".into(),
        nphases: 3,
        nterms: 2,
        buses: vec!["b2.1.2.3".into(), "b2.0.0.0".into()],
    });
    cc.control_type = ctrl_type::TIME;
    cc.ccd.element_terminal = 2;
    cc.recalc();
    assert_eq!(cc.ccd.element_terminal, 1);
    assert_eq!(cc.ccd.cd.get_bus(1), "b2.1.2.3");
    assert!(cc.ccd.cd.obj.take_errors().is_empty());
}

#[test]
fn pf_mode_translates_on_off_settings() {
    // Probed: type=pf onsetting=0.97 offsetting=-0.99 keeps the raw dump
    // values; internally PFON=0.97, PFOFF=2-0.99=1.01.
    let mut cc = CapControl::new("cc1");
    cc.control_type = ctrl_type::PF;
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
        iteration_flag: crate::support::dynamics::IterationFlag::NewTimeStep,
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
    cc.control_type = ctrl_type::KVAR;
    cc.on_value = 150.0;
    cc.off_value = -225.0;
    let mut cap = MockCap::one_step(false); // bank open → PresentState OPEN
    let mut mon = MockMon::new(3);
    mon.power = Complex64::new(0.0, 200_000.0); // 200 kvar inductive
    let mut sc = Scratch::new();
    cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
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
    cc.control_type = ctrl_type::KVAR;
    cc.on_value = 150.0;
    cc.off_value = -225.0;
    let mut cap = MockCap::one_step(true); // bank closed → PresentState CLOSE
    let mut mon = MockMon::new(3);
    mon.power = Complex64::new(0.0, -300_000.0); // -300 kvar (too leading)
    let mut sc = Scratch::new();
    cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
    assert_eq!(cc.pending_change, CTRL_OPEN);
    assert!(cc.armed);
    assert_eq!(cc.ccd.time_delay, 15.0); // OFFDelay
}

#[test]
fn kvar_in_band_does_not_switch() {
    let mut cc = CapControl::new("cc");
    cc.control_type = ctrl_type::KVAR;
    cc.on_value = 150.0;
    cc.off_value = -225.0;
    let mut cap = MockCap::one_step(true);
    let mut mon = MockMon::new(3);
    mon.power = Complex64::new(0.0, -50_000.0); // -50 kvar: between off and on
    let mut sc = Scratch::new();
    cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
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
    cc.control_type = ctrl_type::TIME;
    cc.on_value = 6.0;
    cc.off_value = 21.0;
    let mut cap = MockCap::one_step(false); // open at start
    let mut mon = MockMon::new(3);
    let mut sc = Scratch::new();
    // 12:00 is inside [6, 21) → close.
    cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 12, 0.0));
    assert_eq!(cc.pending_change, CTRL_CLOSE);
    assert!(cc.armed);
}

#[test]
fn pf_control_closes_when_leading_room_remains() {
    let mut cc = CapControl::new("cc");
    cc.control_type = ctrl_type::PF;
    cc.pfon_value = 0.95;
    cc.fpct_minkvar = 50.0;
    let mut cap = MockCap::one_step(false); // open
    cap.total_kvar = 50.0;
    let mut mon = MockMon::new(3);
    // 100 kW + 50 kvar → PF1to2 = 0.894 < 0.95; 50 kvar > 50·50·0.01 = 25.
    mon.power = Complex64::new(100_000.0, 50_000.0);
    let mut sc = Scratch::new();
    cc.sample(&mut cap, &mut mon, &mut sc.ctx(0, 0, 0.0));
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
