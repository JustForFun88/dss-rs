use super::*;

use num_complex::Complex64;

use crate::elements::control::control_elem::CtrlCtx;
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::base::DssObject;
use crate::solution::{LoadSolutionModel, SolveMode};

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
        iteration: 0,
        in_show_results: false,
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
    rc.ccd.controlled_element = Some(ElemId::new(0, 0));
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
    rc.end_edit(&crate::elements::traits::SysCtx::parse_default());
    let errs = rc.ccd.cd.obj.take_errors();
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("Transformer Element is not set"));
}

// --- Sample / DoPendingAction (WP5.5) ---

use crate::elements::pd::transformer::ControlledTransformer;
use crate::solution::{ControlMode, ControlQueue, EventLog};

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
    /// Through-power `Power[ElementTerminal].re` (W) the mock reports; the reg
    /// reads `FwdPower = -Power`. Default 0.
    power_re: f64,
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
            power_re: 0.0,
        }
    }

    /// Set the reported through-power (W) so `FwdPower = -power_re`.
    fn with_power(mut self, power_re: f64) -> Self {
        self.power_re = power_re;
        self
    }
}

impl ControlledTransformer for MockTransformer {
    fn name(&self) -> &str {
        &self.name
    }
    fn full_name(&self) -> String {
        format!("Transformer.{}", self.name)
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
        self.power_re
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
    fn ctx(&mut self, control_mode: ControlMode) -> CtrlCtx<'_> {
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
            self_ref: ElemId::new(0, 0),
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
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();

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
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();
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
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();
    assert!((rc.pending_tap_change - (-0.05)).abs() < 1e-12);

    // ControlMode::Static moves 70% of the pending change, at least one tap:
    // trunc(0.7*0.05/0.00625) = trunc(5.6) = 5 taps down → −0.03125.
    rc.do_pending_action(
        RegControlAction::TapChange.ordinal(),
        &mut tr,
        &mut sc.ctx(ControlMode::Static),
    );
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
    rc.do_pending_action(
        RegControlAction::TapChange.ordinal(),
        &mut tr,
        &mut sc.ctx(ControlMode::EventDriven),
    );
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
    rc.do_pending_action(
        RegControlAction::TapChange.ordinal(),
        &mut tr,
        &mut sc.ctx(ControlMode::Static),
    );
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
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();
    assert_eq!(rc.pending_tap_change, 0.0);
    assert!(sc.queue.is_empty());
}

// --- C5 (r4086, 8a898cba): signed thresholds + idle zones ---

#[test]
fn defaults_are_signed_thresholds() {
    // r4086: RevPowerThreshold −100 kW, FwdPowerThreshold +100 kW.
    let rc = RegControl::new("r1");
    assert_eq!(rc.rev_power_threshold, -100_000.0);
    assert_eq!(rc.fwd_power_threshold, 100_000.0);
}

#[test]
fn rev_only_edit_fallback_is_sign_preserving() {
    // EPRI r4133 RegControl.pas:499-507: a rev-only edit falls back to the band
    // around 0 kW with `Fwd := Rev; Rev := -Rev` — SIGN-PRESERVING (no abs).
    // A NEGATIVE revThreshold=-500 kW must give Fwd=-500 kW, Rev=+500 kW (the
    // inverted band both gating oracles produce). dss_capi 0.15.x `8a898cba`'s
    // `abs` would instead give Fwd=+500 kW, Rev=-500 kW — the D14 divergence the
    // 0.15.x-adoption sweep fixed (DIVERGENCES C5).
    let mut rc = RegControl::new("r1");
    rc.rev_power_threshold = -500_000.0; // -500 kW (scale already applied)
    rc.ccd.cd.obj.set_as_next_seq(prop::REVTHRESHOLD); // edited this edit; Fwd not
    rc.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert_eq!(rc.fwd_power_threshold, -500_000.0);
    assert_eq!(rc.rev_power_threshold, 500_000.0);

    // A POSITIVE rev-only edit is unchanged by dropping the abs (all corpus
    // decks use positive revThresholds): revThreshold=+800 kW → Fwd=+800, Rev=-800.
    let mut rc2 = RegControl::new("r2");
    rc2.rev_power_threshold = 800_000.0;
    rc2.ccd.cd.obj.set_as_next_seq(prop::REVTHRESHOLD);
    rc2.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert_eq!(rc2.fwd_power_threshold, 800_000.0);
    assert_eq!(rc2.rev_power_threshold, -800_000.0);
}

#[test]
fn idle_no_load_zone_suppresses_out_of_band_tap() {
    // Out-of-band high (125 vs 120±1.5) would arm a downward tap, but idle=yes
    // on a reversible reg drops it: FwdPower 0 lies inside the bounded
    // −100/+100 kW no-load zone (r4133 AND) → "idle in no-load zone".
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    rc.is_reversible = true;
    rc.idle_enabled = true;
    let mut tr = MockTransformer::wye_2wdg(125.0); // power_re 0 → FwdPower 0
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();
    assert_eq!(rc.pending_tap_change, 0.0);
    assert!(!rc.armed);
    assert!(sc.queue.is_empty());
}

#[test]
fn idle_no_load_zone_still_taps_when_power_out_of_band() {
    // r4133 AND: the no-load zone is BOUNDED. FwdPower +200 kW is above
    // FwdPowerThreshold (+100 kW), so idle=yes does NOT suppress — the
    // out-of-band reg arms just like the non-idle case. This is the exact
    // regression the OR tautology hid (STATUS "BUG WP regcontrol_idle"): under
    // the old `or` this armed nothing for ANY load.
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    rc.is_reversible = true;
    rc.idle_enabled = true;
    // FwdPower = -power_re = +200 kW ⇒ power_re = -200 kW (deep import).
    let mut tr = MockTransformer::wye_2wdg(125.0).with_power(-200_000.0);
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();
    assert!((rc.pending_tap_change - (-0.05)).abs() < 1e-12);
    assert!(rc.armed);
}

#[test]
fn idle_without_reversible_or_cogen_still_taps() {
    // IdleEnabled alone (no reversible/cogen) does NOT gate the idle zone, so
    // the out-of-band reg still arms exactly like the non-idle case.
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    rc.idle_enabled = true; // but is_reversible = cogen_enabled = false
    let mut tr = MockTransformer::wye_2wdg(125.0);
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();
    assert!((rc.pending_tap_change - (-0.05)).abs() < 1e-12);
    assert!(rc.armed);
}

#[test]
fn idle_forward_zone_suppresses_when_exporting_hard() {
    // idleForward + reversible: FwdPower above FwdPowerThreshold (+100 kW) →
    // idle in the forward zone (FwdPower = -power_re = 200 kW here).
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    rc.is_reversible = true;
    rc.idle_forward_enabled = true;
    let mut tr = MockTransformer::wye_2wdg(125.0).with_power(-200_000.0);
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();
    assert_eq!(rc.pending_tap_change, 0.0);
    assert!(!rc.armed);
}

#[test]
fn signed_rev_threshold_arms_reverse_pending_below_default() {
    // FwdPower −150 kW < RevPowerThreshold (−100 kW) → schedule ACTION_REVERSE.
    // Pins the sign move: the guard is `FwdPower < RevPowerThreshold` (no unary −).
    let mut rc = RegControl::new("r1");
    rc.pt_ratio = 1.0;
    rc.ccd.cd.nphases = 1;
    rc.is_reversible = true;
    // FwdPower = -power_re = -150 kW ⇒ power_re = +150 kW.
    let mut tr = MockTransformer::wye_2wdg(120.0).with_power(150_000.0);
    let mut sc = Scratch::new();
    rc.sample(&mut tr, &mut sc.ctx(ControlMode::Static))
        .unwrap();
    assert!(rc.reverse_pending);
    assert_eq!(sc.queue.queue_size(), 1);
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemId};
    use crate::obj::base::DssObject;

    /// Pascal `TRegControlObj.MakePosSequence` (RegControl.pas:1266): Enabled +
    /// phases from the controlled transformer, terminal bus at ElementTerminal.
    #[test]
    fn resyncs_to_controlled_transformer() {
        let mut rc = RegControl::new("rc1");
        rc.ccd.controlled_element = Some(ElemId::new(1, 0));
        rc.ccd.element_terminal = 2;
        rc.using_regulated_bus = false;
        let ctx = PosSeqCtx {
            controlled: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                enabled: false,
                bus_names: vec!["w1".into(), "w2".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let plan = rc.make_pos_sequence(&ctx);
        assert_eq!(rc.ccd.cd.nphases, 1);
        assert_eq!(rc.ccd.cd.nconds, 1);
        assert!(!rc.ccd.cd.enabled); // FEnabled := ControlledElement.Enabled
        assert_eq!(rc.get_bus_name(1), "w2"); // GetBus(ElementTerminal=2)
        assert!(plan.run_base && plan.actions.is_empty());
    }

    /// UsingRegulatedBus ⇒ FNphases := 1, Nconds := 1, Setbus to RegulatedBus.
    #[test]
    fn regulated_bus_forces_single_phase() {
        let mut rc = RegControl::new("rc1");
        rc.ccd.controlled_element = Some(ElemId::new(1, 0));
        rc.using_regulated_bus = true;
        rc.regulated_bus = "remotebus".into();
        let ctx = PosSeqCtx {
            controlled: Some(PosSeqElemInfo {
                nphases: 3,
                nconds: 3,
                enabled: true,
                bus_names: vec!["w1".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        rc.make_pos_sequence(&ctx);
        assert_eq!(rc.ccd.cd.nphases, 1);
        assert_eq!(rc.ccd.cd.nconds, 1);
        assert_eq!(rc.get_bus_name(1), "remotebus");
    }
}

#[test]
fn reg_control_action_pins_queue_codes() {
    assert_eq!(RegControlAction::TapChange.ordinal(), 0);
    assert_eq!(RegControlAction::Reverse.ordinal(), 1);
    assert_eq!(
        RegControlAction::from_ordinal(0),
        Some(RegControlAction::TapChange)
    );
    assert_eq!(
        RegControlAction::from_ordinal(1),
        Some(RegControlAction::Reverse)
    );
    assert_eq!(RegControlAction::from_ordinal(2), None);
}

/// RP3.11 P4 — `TRegcontrolObj.SaveWrite` (r4133
/// `Version8/Source/Controls/RegControl.pas:1399-1421`) writes `transformer=`
/// **first**, outside the property chain, and then skips index 1 in the walk.
/// Pascal's own reason: *"Write Transformer name out first so that it is set for
/// later operations"* — `winding=`/`tapnum=`/`vreg=`/`ptratio=` are all resolved
/// against the controlled transformer as they are parsed.
///
/// Measured before this override, on a deck typing `transformer=` last, the port
/// wrote `New "RegControl.rc1" Winding=2 VReg=122 Band=3 PTRatio=20
/// Transformer=t1`; r4133 writes `New "RegControl.rc1" transformer=t1 winding=2
/// tapwinding=2 vreg=122 band=3 ptratio=20` (epri-worker probe,
/// OpenDSSDirect.dll 11.0.0.1 r4133, RP3.11 I1). The `tapwinding` in r4133's
/// line is a separate *sequence* divergence, decided in RP3.11 §3.3 and pinned
/// there: r4133 stamps it as a side effect of `winding=`
/// (`RegControl.pas:480-483`), dss_capi 0.14.5 dropped that stamp
/// (`CAPI:Controls/RegControl.pas:417`, *"not really required"*) and this port
/// followed — round-trip-safe, since re-parsing `Winding=2` re-fires the same
/// `TapWinding := winding` side effect.
#[test]
fn regcontrol_save_write_puts_the_transformer_first() {
    use crate::exec::Dss;

    let dir = std::env::temp_dir().join(format!("dss_rp311_rc_{}", std::process::id()));
    std::fs::remove_dir_all(&dir).ok();
    let mut dss = Dss::new();
    for c in [
        "clear",
        "new circuit.rp311rc basekv=12.47 pu=1.0 phases=3 bus1=src",
        "new transformer.t1 windings=2 buses=[src b] conns=[wye wye] kvs=[12.47 4.16] \
         kvas=[1000 1000] xhl=6",
        // `transformer=` typed LAST
        "new regcontrol.rc1 winding=2 vreg=122 band=3 ptratio=20 transformer=t1",
    ] {
        dss.command(c);
    }
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    dss.command(&format!(
        "save circuit dir=\"{}\"",
        dir.to_string_lossy().replace('\\', "/")
    ));
    assert!(dss.errors().is_empty(), "save errors: {:?}", dss.errors());

    let text = std::fs::read_to_string(dir.join("RegControl.dss")).expect("RegControl.dss");
    let line = text
        .lines()
        .find(|l| l.contains("RegControl.rc1"))
        .unwrap_or_else(|| panic!("no RegControl.rc1 line in {text:?}"));
    assert_eq!(
        line, "New \"RegControl.rc1\" Transformer=t1 Winding=2 VReg=122 Band=3 PTRatio=20",
        "r4133 writes `New \"RegControl.rc1\" transformer=t1 winding=2 tapwinding=2 \
         vreg=122 band=3 ptratio=20`"
    );
    assert_eq!(
        line.matches("Transformer=").count(),
        1,
        "the transformer must be written exactly once: {line:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}
