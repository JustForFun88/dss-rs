//! Spec-pinned unit tests for the PVSystem element (`TPVsystemObj`). The
//! numeric oracle pinning lives in the integration goldens (`phase7/pvsystem*`)
//! and the live corpus gate; these pin the ported Pascal bodies that the oracle
//! does not expose directly (Create defaults, the inverter clamp branches,
//! `ComputePanelPower`, the YEQ derivation).

use crate::elements::pc::generator::default_recalc_ctx;
use crate::elements::pc::inv_based_pce::{Connection, InvBasedPce};
use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use crate::support::cmatrix::CMatrix;
use num_complex::Complex64;

use super::*;

/// The harmonic-mode YPrim is the Thevenin admittance behind `%R`/`%X`
/// (`Yeq := 1/(Rthev + j·Xthev)`, then `Y.im /= h`) that `InitHarmonics` sets —
/// NOT the (negated) power-flow admittance. Pins the harmonic `CalcYPrimMatrix`
/// branch entry-by-entry and, as a discriminator, asserts it is far from the
/// power-flow stamping. Oracle-independent backstop for the
/// `phase7/harmonics_pvsystem_h5` golden.
#[test]
fn harmonic_yprim_is_thevenin_admittance_not_powerflow() {
    let mut pv = PVSystem::new("pv1");
    // A representative power-flow Yeq (what SetNominalDEROutput leaves before harmonics).
    pv.base.yeq = Complex64::new(0.006, -0.0004);
    let pf_yeq = pv.base.yeq;
    // `InitHarmonics`: Yeq := 1/(Rthev + j·Xthev) (representative ohms).
    let z_thev = Complex64::new(25.0, 25.0);
    pv.r_thev = z_thev.re;
    pv.x_thev = z_thev.im;
    pv.base.yeq = z_thev.inv();

    let h = 5.0_f64;
    let sys = SysCtx {
        frequency: 60.0 * h,
        fundamental: 60.0,
        is_harmonic_model: true,
        ..default_recalc_ctx()
    };
    let mut ym = CMatrix::new(pv.cd.yorder);
    pv.calc_yprim_matrix(&mut ym, &sys);
    let actual = ym.get(0, 0); // wye phase-A diagonal

    let mut expected = z_thev.inv();
    expected.im /= h;
    assert!(
        (actual - expected).norm() < 1e-12,
        "harmonic YPrim {actual} != Thevenin admittance {expected}"
    );
    let mut naive = -pf_yeq;
    naive.im /= h;
    assert!(
        (actual - naive).norm() > 0.1 * naive.norm(),
        "harmonic YPrim {actual} indistinguishable from the power-flow path {naive}"
    );
}

/// Pascal `TPVsystemObj.Create` defaults.
#[test]
fn create_defaults() {
    let pv = PVSystem::new("pv1");
    assert_eq!(pv.cd.nphases, 3);
    assert_eq!(pv.cd.nconds, 4); // wye
    assert_eq!(pv.base.connection, Connection::Wye);
    assert_eq!(pv.base.voltage_model, 1);
    assert_eq!(pv.kv_pvsystem_base, 12.47);
    assert_eq!(pv.f_kva_rating, 500.0);
    assert_eq!(pv.f_pmpp, 500.0);
    assert_eq!(pv.f_pu_pmpp, 1.0);
    assert_eq!(pv.f_irradiance, 1.0);
    assert_eq!(pv.f_temperature, 25.0);
    assert_eq!(pv.base.vminpu, 0.90);
    assert_eq!(pv.base.vmaxpu, 1.10);
    assert_eq!(pv.base.var_mode, VARMODE_PF);
    assert!(pv.base.inverter_on);
    assert_eq!(pv.base.pf_nominal, 1.0);
    assert_eq!(pv.base.pct_r, 50.0);
    assert_eq!(pv.base.pct_x, 0.0);
    assert_eq!(pv.base.fpct_cut_in, 20.0);
    assert_eq!(pv.base.fpct_cut_out, 20.0);
    // dynVars Create overrides.
    assert_eq!(pv.base.dyn_vars.rated_vdc, 8000.0);
    assert_eq!(pv.base.dyn_vars.sm_threshold, 80.0);
    assert_eq!(pv.base.dyn_vars.kp, 0.00001);
    // InvBasedPCE base dynVars defaults.
    assert_eq!(pv.base.dyn_vars.i_limit, -1.0);
    assert_eq!(pv.base.dyn_vars.v_error, 0.8);
    // The inverter virtual hook.
    assert!(pv.is_pvsystem());
}

/// After `RecalcElementData`, the default PV (PF = 1, irradiance = 1, Pmpp =
/// 500, ideal inverter) delivers its full panel power at unity PF: kW_out = 500,
/// kvar_out = 0, and YEQ = (P − jQ)/Vbase² with Q = 0.
#[test]
fn default_recalc_yields_full_kw_unity_pf() {
    let pv = PVSystem::new("pv1");
    // Panel kW = irradiance(1) · shape(1) · Pmpp(500) · tempfactor(1) = 500.
    assert_eq!(pv.panel_kw, 500.0);
    assert_eq!(pv.base.kw_out, 500.0);
    assert_eq!(pv.base.kvar_out, 0.0);
    assert!(pv.base.inverter_on);
    let nphases = pv.cd.nphases as f64;
    assert_eq!(pv.base.p_nominal_per_phase, 1000.0 * 500.0 / nphases);
    assert_eq!(pv.base.q_nominal_per_phase, 0.0);
    // VBase L-N = 12.47 kV / sqrt(3) · 1000.
    let vbase = pv.base.v_base;
    let expect_yeq = num_complex::Complex64::new(pv.base.p_nominal_per_phase, 0.0) / vbase.powi(2);
    assert!((pv.base.yeq.re - expect_yeq.re).abs() < 1e-9);
    assert_eq!(pv.base.yeq.im, 0.0);
}

/// `Get_PresentkW` = Pnominalperphase · 0.001 · nphases (= the nominal kW).
#[test]
fn present_kw_kvar_round_trip() {
    let pv = PVSystem::new("pv1");
    assert!((pv.present_kw() - 500.0).abs() < 1e-9);
    assert_eq!(pv.present_kvar(), 0.0);
}

/// The inverter cut-out: when the panel power drops below CutOutkW
/// (= %Cutout·kVA/100 = 20%·500/100 = 100 kW), the inverter turns OFF and
/// kW_out collapses to 0. Driven by setting a tiny irradiance and recomputing.
#[test]
fn inverter_cuts_out_below_threshold() {
    let mut pv = PVSystem::new("pv1");
    assert!(pv.base.inverter_on);
    // CutOutkW = 100; drive panel power well below it.
    pv.set_f64(prop::IRRADIANCE, 0.1); // 0.1·500 = 50 kW < 100
    pv.recalc(&crate::elements::pc::generator::default_recalc_ctx());
    assert!(!pv.base.inverter_on);
    assert_eq!(pv.base.kw_out, 0.0);
}

/// The non-priority kVA clamp: with kvar=400 requested (varMode=KVAR) on a
/// kVA=500 inverter and full panel power (kw would be 500), apparent power
/// 640 > 500 forces the no-priority back-off `kW_out := sqrt(kVA²−kvar²)` → kW =
/// sqrt(500²−400²) = **300**, kvar **stays 400**. (Pinned exactly — not just an
/// upper bound — so a regression that zeroed the output or backed off the wrong
/// leg can't pass; the oracle pins the same state in golden `phase7/pvsystem_clamps`
/// element `pva`.)
#[test]
fn kva_clamp_backs_off_kw() {
    let mut pv = PVSystem::new("pv1");
    pv.set_i32(prop::CONN, 0);
    // Request kvar; switch to kvar mode.
    pv.set_f64(prop::KVAR, 400.0);
    pv.side_effects(prop::KVAR, 0);
    pv.recalc(&crate::elements::pc::generator::default_recalc_ctx());
    assert!(
        (pv.base.kw_out - 300.0).abs() < 1e-9,
        "kw_out = {}",
        pv.base.kw_out
    );
    assert!(
        (pv.base.kvar_out - 400.0).abs() < 1e-9,
        "kvar_out = {}",
        pv.base.kvar_out
    );
}

/// The negative-kvar absorption clamp + back-off: kvar=−400 requested with
/// `kvarMaxAbs`=300 clamps to kvar_out=−300 (absorption limit), then the kVA
/// back-off sets kW = sqrt(500²−300²) = **400**. Pins the absorption direction
/// (the `kvarNEG` corpus sibling that would cover it is deferred at ~4e-6;
/// the oracle pins this state in golden `phase7/pvsystem_clamps` element `pvc`).
#[test]
fn kvar_absorption_clamp_then_backoff() {
    let mut pv = PVSystem::new("pv1");
    pv.set_i32(prop::CONN, 0);
    pv.set_f64(prop::KVAR_MAX_ABS, 300.0);
    pv.side_effects(prop::KVAR_MAX_ABS, 0);
    pv.set_f64(prop::KVAR, -400.0);
    pv.side_effects(prop::KVAR, 0);
    pv.recalc(&crate::elements::pc::generator::default_recalc_ctx());
    assert!(
        (pv.base.kvar_out + 300.0).abs() < 1e-9,
        "kvar_out = {}",
        pv.base.kvar_out
    );
    assert!(
        (pv.base.kw_out - 400.0).abs() < 1e-9,
        "kw_out = {}",
        pv.base.kw_out
    );
}
