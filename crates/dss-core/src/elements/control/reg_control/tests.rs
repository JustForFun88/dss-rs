use super::*;

use num_complex::Complex64;

use crate::elements::control::control_elem::CtrlCtx;
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::DssObject;
use crate::solution::SolveMode;

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

#[test]
fn default_shape_is_3ph_1term_no_yprim() {
    let mut rc = RegControl::new("r1");
    assert_eq!(rc.ccd.cd.nphases, 3);
    assert_eq!(rc.ccd.cd.nconds, 3);
    assert_eq!(rc.ccd.cd.nterms, 1);
    assert_eq!(rc.vreg, 120.0);
    assert_eq!(rc.bandwidth, 3.0);
    assert_eq!(rc.pt_ratio, 60.0);
    assert_eq!(rc.remote_pt_ratio, 60.0);
    assert_eq!(rc.ct_rating, 300.0);
    assert_eq!(rc.ccd.time_delay, 15.0);
    assert_eq!(rc.tap_limit_per_change, 16);
    // CalcYPrim is a no-op: YPrim stays None so BuildYMatrix skips it.
    rc.calc_yprim(&test_sys());
    assert!(rc.ccd.cd.yprim.is_none());
    // GetCurrents is always zero.
    let mut curr = vec![Complex64::new(1.0, 1.0); 3];
    rc.get_currents(&test_sys(), &[], &mut curr);
    assert!(curr.iter().all(|c| *c == Complex64::ZERO));
}

#[test]
fn tapnum_maps_tap_to_integer_and_back() {
    let mut rc = RegControl::new("r1");
    rc.ccd.controlled_element = Some(ElemRef { cls: 0, idx: 0 });
    rc.tap_winding = 2;
    // (PresentTap, MaxTap, MinTap, TapIncrement) — the 32-tap default.
    rc.tap_snap = vec![(1.0, 1.1, 0.9, 0.00625), (1.0, 1.1, 0.9, 0.00625)];
    assert_eq!(rc.get_tap_num(), 0);

    rc.set_tap_num(5); // → 1.03125 (probed: oracle taps [1, 1.03125])
    assert_eq!(rc.get_tap_num(), 5);
    let actions = rc.take_ref_actions();
    assert_eq!(actions.len(), 1);
    let RefAction::SetTransformerTap { winding, tap, .. } = &actions[0] else {
        panic!("expected SetTransformerTap, got {:?}", actions[0]);
    };
    assert_eq!(*winding, 2);
    assert!((tap - 1.03125).abs() < 1e-12);

    rc.set_tap_num(-3); // → 0.98125 (probed)
    assert_eq!(rc.get_tap_num(), -3);
}

#[test]
fn recalc_without_transformer_records_error_124() {
    let mut rc = RegControl::new("r1");
    rc.end_edit();
    let errs = rc.ccd.cd.obj.take_errors();
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("Transformer Element is not set"));
}

// --- Sample / DoPendingAction (WP5.5) ---

use crate::elements::pd::transformer::ControlledTransformer;
use crate::solution::{CTRLSTATIC, ControlQueue, EVENTDRIVEN, EventLog};

/// A lightweight `ControlledTransformer` returning canned winding voltages
/// and per-winding tap data, so the regulator decision logic is testable
/// without a node-wired transformer.
struct MockTransformer {
    name: String,
    nphases: usize,
    nconds: usize,
    yorder: usize,
    present: Vec<f64>,
    maxt: Vec<f64>,
    mint: Vec<f64>,
    inc: Vec<f64>,
    base_v: Vec<f64>,
    conn: Vec<i32>,
    wv: Vec<Complex64>,
}

impl MockTransformer {
    /// A 2-winding wye regulator with the canonical 32-tap range and a
    /// single regulated phase voltage `vph` (volts, secondary base).
    fn wye_2wdg(vph: f64) -> Self {
        Self {
            name: "reg1".into(),
            nphases: 1,
            nconds: 2,
            yorder: 4,
            present: vec![1.0, 1.0],
            maxt: vec![1.1, 1.1],
            mint: vec![0.9, 0.9],
            inc: vec![0.00625, 0.00625],
            base_v: vec![100.0, 100.0],
            conn: vec![0, 0],
            wv: vec![Complex64::new(vph, 0.0)],
        }
    }
}

impl ControlledTransformer for MockTransformer {
    fn name(&self) -> &str {
        &self.name
    }
    fn n_phases(&self) -> usize {
        self.nphases
    }
    fn n_conds(&self) -> usize {
        self.nconds
    }
    fn y_order(&self) -> usize {
        self.yorder
    }
    fn wdg_connection(&self, term: usize) -> i32 {
        self.conn[term - 1]
    }
    fn rotate_phases(&self, iphs: usize) -> usize {
        iphs
    }
    fn base_voltage(&self, term: usize) -> f64 {
        self.base_v[term - 1]
    }
    fn present_tap(&self, w: usize) -> f64 {
        self.present[w - 1]
    }
    fn min_tap(&self, w: usize) -> f64 {
        self.mint[w - 1]
    }
    fn max_tap(&self, w: usize) -> f64 {
        self.maxt[w - 1]
    }
    fn tap_increment(&self, w: usize) -> f64 {
        self.inc[w - 1]
    }
    fn set_present_tap(&mut self, w: usize, value: f64) -> bool {
        let v = value.clamp(self.mint[w - 1], self.maxt[w - 1]);
        if v != self.present[w - 1] {
            self.present[w - 1] = v;
            true
        } else {
            false
        }
    }
    fn power_into_re(&mut self, _term: usize, _node_v: &[Complex64], _sys: &SysCtx) -> f64 {
        0.0
    }
    fn winding_voltages(&mut self, _term: usize, _node_v: &[Complex64], vbuffer: &mut [Complex64]) {
        for (i, v) in vbuffer.iter_mut().take(self.nphases).enumerate() {
            *v = self.wv[i];
        }
    }
    fn terminal_currents(
        &mut self,
        _node_v: &[Complex64],
        _sys: &SysCtx,
        cbuffer: &mut [Complex64],
    ) {
        cbuffer.fill(Complex64::ZERO);
    }
}

/// Build a `CtrlCtx` over freshly-owned queue/event/error/flag scratch.
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
    fn ctx(&mut self, control_mode: i32) -> CtrlCtx<'_> {
        CtrlCtx {
            node_v: &[],
            sys: &self.sys,
            queue: &mut self.queue,
            events: &mut self.events,
            errors: &mut self.errors,
            system_y_changed: &mut self.y_changed,
            control_mode,
            int_hour: 0,
            t: 0.0,
            dbl_hour: 0.0,
            control_iter: 1,
            self_ref: ElemRef { cls: 0, idx: 0 },
        }
    }
}

#[test]
fn sample_out_of_band_high_arms_a_downward_tap() {
    // Vactual 125 V vs Vreg 120 ± 1.5 → out of band high → boost negative.
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1; // regulator senses one phase
    let mut tr = MockTransformer::wye_2wdg(125.0);
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(CTRLSTATIC));

    // boost_needed = (120-125)*1/100 = -0.05; /0.00625 = -8 → -0.05 pu.
    assert!((rc.pending_tap_change - (-0.05)).abs() < 1e-12);
    assert!(rc.armed);
    assert_eq!(sc.queue.queue_size(), 1); // armed ACTION_TAPCHANGE
}

#[test]
fn sample_in_band_disarms_and_clears() {
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    // Pre-arm the control as if a prior sample queued a change.
    rc.armed = true;
    rc.control_action_handle = 999;
    let mut tr = MockTransformer::wye_2wdg(120.5); // within ±1.5 of 120
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(CTRLSTATIC));
    assert_eq!(rc.pending_tap_change, 0.0);
    assert!(!rc.armed);
}

#[test]
fn ctrlstatic_action_applies_at_least_one_tap_and_marks_y() {
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    let mut tr = MockTransformer::wye_2wdg(125.0);
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(CTRLSTATIC));
    assert!((rc.pending_tap_change - (-0.05)).abs() < 1e-12);

    // CTRLSTATIC moves 70% of the pending change, at least one tap:
    // trunc(0.7*0.05/0.00625) = trunc(5.6) = 5 taps down → −0.03125.
    rc.do_pending_action(ACTION_TAPCHANGE, &mut tr, &mut sc.ctx(CTRLSTATIC));
    assert_eq!(rc.last_change, -5);
    assert!((tr.present_tap(1) - 0.96875).abs() < 1e-12);
    assert!(sc.y_changed);
    assert_eq!(rc.pending_tap_change, 0.0);
    assert!(!rc.armed);
}

#[test]
fn eventdriven_action_moves_one_tap_and_repushes() {
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    let mut tr = MockTransformer::wye_2wdg(125.0);
    let mut sc = Scratch::new();
    // Pretend two taps are pending downward.
    rc.set_pending_tap_change(-2.0 * 0.00625);
    rc.do_pending_action(ACTION_TAPCHANGE, &mut tr, &mut sc.ctx(EVENTDRIVEN));
    assert_eq!(rc.last_change, -1); // one tap toward the change
    assert!((tr.present_tap(1) - (1.0 - 0.00625)).abs() < 1e-12);
    assert!((rc.pending_tap_change - (-0.00625)).abs() < 1e-12); // remainder
    assert_eq!(sc.queue.queue_size(), 1); // re-pushed for the next tap
}

#[test]
fn event_log_records_tap_change_when_enabled() {
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    rc.ccd.show_event_log = true;
    rc.set_pending_tap_change(-0.05);
    let mut tr = MockTransformer::wye_2wdg(125.0);
    let mut sc = Scratch::new();
    rc.do_pending_action(ACTION_TAPCHANGE, &mut tr, &mut sc.ctx(CTRLSTATIC));
    assert_eq!(sc.events.len(), 1);
    let line = &sc.events.entries()[0];
    assert!(line.contains("Element=Regulator.reg1"));
    assert!(line.contains("CHANGED -5 TAPS TO"));
}

#[test]
fn compute_time_delay_fixed_vs_inverse() {
    let mut rc = RegControl::new("r1");
    rc.ccd.time_delay = 15.0;
    assert_eq!(rc.compute_time_delay(118.0), 15.0); // fixed by default
    rc.inverse_time = true;
    rc.vreg = 120.0;
    rc.bandwidth = 3.0;
    // 2*|120-118|/3 = 1.333 < 10 → 15 / 1.333 = 11.25.
    assert!((rc.compute_time_delay(118.0) - 11.25).abs() < 1e-9);
}

#[test]
fn maxtapchange_zero_zeroes_pending_and_exits() {
    let mut rc = RegControl::new("r1");
    rc.tap_limit_per_change = 0;
    rc.set_pending_tap_change(0.5);
    let mut tr = MockTransformer::wye_2wdg(150.0);
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(CTRLSTATIC));
    assert_eq!(rc.pending_tap_change, 0.0);
    assert!(sc.queue.is_empty());
}
