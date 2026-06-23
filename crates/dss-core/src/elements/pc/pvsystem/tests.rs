//! Spec-pinned unit tests for the PVSystem element (`TPVsystemObj`). The
//! numeric oracle pinning lives in the integration goldens (`phase7/pvsystem*`)
//! and the live corpus gate; these pin the ported Pascal bodies that the oracle
//! does not expose directly (Create defaults, the inverter clamp branches,
//! `ComputePanelPower`, the YEQ derivation).

use crate::elements::pc::inv_based_pce::{Connection, InvBasedPce};
use crate::obj::base::DssObject;

use super::*;

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

/// The kVA clamp (no PF/Watt priority): with a kvar request that would push the
/// apparent power past the rating, kW is backed off so that kW² + kvar² = kVA².
#[test]
fn kva_clamp_backs_off_kw() {
    let mut pv = PVSystem::new("pv1");
    pv.set_i32(prop::CONN, 0);
    // Request kvar; switch to kvar mode.
    pv.set_f64(prop::KVAR, 400.0);
    pv.side_effects(prop::KVAR, 0);
    pv.recalc(&crate::elements::pc::generator::default_recalc_ctx());
    let kva = (pv.base.kw_out.powi(2) + pv.base.kvar_out.powi(2)).sqrt();
    assert!(kva <= pv.f_kva_rating + 1e-6, "kVA {kva} exceeds rating");
}
