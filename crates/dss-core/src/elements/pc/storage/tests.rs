//! Spec-pinned unit tests for the Storage element (`TStorageObj`). The numeric
//! oracle pinning lives in the integration goldens (`der_controls/storage*`) and the
//! live corpus gate; these pin the ported Pascal bodies that the oracle does not
//! expose directly (Create defaults, the state machine, `ComputePresentkW`, the
//! inverter clamp, the `%stored` read/write).

use crate::elements::pc::inv_based_pce::{Connection, InvBasedPce};
use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use crate::support::cmatrix::CMatrix;
use num_complex::Complex64;

use super::*;

fn ctx() -> crate::elements::traits::SysCtx {
    crate::elements::pc::generator::default_recalc_ctx()
}

/// The harmonic-mode YPrim is the Thevenin admittance behind `%R`/`%X`
/// (`Yeq := 1/(Rthev + j·Xthev)`, then `Y.im /= h`) that `InitHarmonics` sets —
/// NOT the state-dependent power-flow admittance. Pins the harmonic
/// `CalcYPrimMatrix` branch entry-by-entry and discriminates it from the
/// power-flow stamping. Oracle-independent backstop for the
/// `harmonics/harmonics_storage_h5` golden.
#[test]
fn harmonic_yprim_is_thevenin_admittance_not_powerflow() {
    let mut st = Storage::new("s1");
    // A representative power-flow discharge admittance (the state-dependent
    // YeqDischarge the power-flow branch would stamp).
    st.yeq_discharge = Complex64::new(0.006, -0.0004);
    let pf_yeq = st.yeq_discharge;
    // `InitHarmonics`: Yeq := 1/(Rthev + j·Xthev) (representative ohms).
    let z_thev = Complex64::new(25.0, 25.0);
    st.r_thev = z_thev.re;
    st.x_thev = z_thev.im;
    st.base.yeq = z_thev.inv();

    let h = 5.0_f64;
    let sys = SysCtx {
        frequency: 60.0 * h,
        fundamental: 60.0,
        is_harmonic_model: true,
        ..ctx()
    };
    let mut ym = CMatrix::new(st.cd.yorder);
    st.calc_yprim_matrix(&mut ym, &sys);
    let actual = ym.get(0, 0); // wye phase-A diagonal

    let mut expected = z_thev.inv();
    expected.im /= h;
    assert!(
        (actual - expected).norm() < 1e-12,
        "harmonic YPrim {actual} != Thevenin admittance {expected}"
    );
    // Discriminator: the discharging power-flow stamping (−YeqDischarge, freq-scaled).
    let mut naive = -pf_yeq;
    naive.im /= h;
    assert!(
        (actual - naive).norm() > 0.1 * naive.norm(),
        "harmonic YPrim {actual} indistinguishable from the power-flow path {naive}"
    );
}

/// Pascal `TStorageObj.Create` defaults.
#[test]
fn create_defaults() {
    let st = Storage::new("s1");
    assert_eq!(st.cd.nphases, 3);
    assert_eq!(st.cd.nconds, 4); // wye
    assert_eq!(st.base.connection, Connection::Wye);
    assert_eq!(st.base.voltage_model, 1);
    assert_eq!(st.kv_storage_base, 12.47);
    assert_eq!(st.kw_rating, 25.0);
    assert_eq!(st.f_kva_rating, 25.0);
    assert_eq!(st.kwh_rating, 50.0);
    assert_eq!(st.kwh_stored, 50.0);
    assert_eq!(st.pct_reserve, 20.0);
    assert_eq!(st.kwh_reserve, 10.0); // kWhRating·pctReserve/100
    assert_eq!(st.f_state, STORE_IDLING);
    assert_eq!(st.dispatch_mode, STORE_DEFAULT);
    assert_eq!(st.pct_kw_out, 100.0);
    assert_eq!(st.pct_kw_in, 100.0);
    assert_eq!(st.pct_idle_kw, 1.0);
    assert_eq!(st.pct_charge_eff, 90.0);
    assert_eq!(st.pct_discharge_eff, 90.0);
    assert_eq!(st.charge_time, 2.0);
    assert_eq!(st.base.pct_r, 0.0);
    assert_eq!(st.base.pct_x, 50.0);
    assert_eq!(st.base.vminpu, 0.90);
    assert_eq!(st.base.vmaxpu, 1.10);
    assert_eq!(st.base.var_mode, VARMODE_PF);
    assert!(st.base.inverter_on);
    assert_eq!(st.base.pf_nominal, 1.0);
    assert_eq!(st.f_kvar_limit, 25.0); // = FkVArating
    assert_eq!(st.f_kvar_limit_neg, 25.0);
    // dynVars Create overrides.
    assert_eq!(st.base.dyn_vars.rated_vdc, 8000.0);
    assert_eq!(st.base.dyn_vars.sm_threshold, 80.0);
    assert_eq!(st.base.dyn_vars.kp, 0.00001);
    assert_eq!(st.base.dyn_vars.i_limit, -1.0);
    assert_eq!(st.base.dyn_vars.v_error, 0.8);
    // The inverter virtual hook.
    assert!(st.is_storage());
    // PIdling = %IdlingkW·kWrating/100 = 0.25; ideal inverter → kWOutIdling = 0.25.
    assert_eq!(st.p_idling, 0.25);
    assert_eq!(st.kw_out_idling, 0.25);
}

/// A default (idling, fully-charged) Storage draws only its idling losses:
/// `kW_out = −kWOutIdling = −0.25`, matching the oracle `? kW` = −0.25.
#[test]
fn idle_draws_only_idling_losses() {
    let st = Storage::new("s1");
    assert_eq!(st.f_state, STORE_IDLING);
    assert!((st.base.kw_out + 0.25).abs() < 1e-12);
    assert!((st.present_kw() + 0.25).abs() < 1e-9);
    assert_eq!(st.present_kvar(), 0.0);
}

/// Setting `State=Discharging` delivers `kWrating·%Discharge = 25` at the
/// terminal (default `%Discharge = 100`, ideal inverter, no cut-in/out).
#[test]
fn discharging_delivers_rated_kw() {
    let mut st = Storage::new("s1");
    st.set_i32(prop::STATE, STORE_DISCHARGING);
    st.recalc(&ctx());
    assert_eq!(st.f_state, STORE_DISCHARGING);
    assert!(
        (st.base.kw_out - 25.0).abs() < 1e-9,
        "kw_out = {}",
        st.base.kw_out
    );
    assert!((st.present_kw() - 25.0).abs() < 1e-9);
}

/// `Set_kW(value)` sets the state + dispatch %: a positive kW discharges,
/// `%Discharge = kW/kWrating·100`. kW=10 → 40% → terminal kW = 10.
#[test]
fn set_kw_positive_discharges() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::KW, 10.0); // Pascal SetkW
    assert_eq!(st.f_state, STORE_DISCHARGING);
    assert_eq!(st.pct_kw_out, 40.0);
    st.recalc(&ctx());
    assert!((st.present_kw() - 10.0).abs() < 1e-9);
}

/// A negative kW charges (absorbs) when the battery is not full: with `%Stored`
/// dropped to 50 (kWhStored=25 < 50) and kW=−5, the terminal kW is −5.
#[test]
fn set_kw_negative_charges_when_not_full() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::PCT_STORED, 50.0); // kWhStored = 25
    assert_eq!(st.kwh_stored, 25.0);
    st.set_f64(prop::KW, -5.0);
    assert_eq!(st.f_state, STORE_CHARGING);
    assert_eq!(st.pct_kw_in, 20.0);
    st.recalc(&ctx());
    assert!(
        (st.present_kw() + 5.0).abs() < 1e-9,
        "present_kw = {}",
        st.present_kw()
    );
}

/// A charge command on a *full* battery (default kWhStored = kWhRating) falls
/// straight back to idling in `ComputePresentkW`.
#[test]
fn charging_full_battery_falls_to_idling() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::KW, -5.0);
    assert_eq!(st.f_state, STORE_CHARGING);
    st.recalc(&ctx()); // kWhStored = kWhRating → state flips to idling
    assert_eq!(st.f_state, STORE_IDLING);
    assert!((st.base.kw_out + 0.25).abs() < 1e-9);
}

/// `%Stored` round-trips through `kWhStored`: write `%Stored=40` →
/// `kWhStored = 0.40·kWhRating = 20`; read back 40.
#[test]
fn pct_stored_round_trip() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::PCT_STORED, 40.0);
    assert_eq!(st.kwh_stored, 20.0);
    assert!((st.get_f64(prop::PCT_STORED) - 40.0).abs() < 1e-9);
}

/// `kWhRated` side effect: setting the energy rating refills the battery and
/// recomputes the reserve (`kWhStored := kWhRating`, `kWhReserve := kWhRating·
/// %Reserve/100`).
#[test]
fn kwh_rated_side_effect_recharges() {
    let mut st = Storage::new("s1");
    st.set_f64(prop::KWH_RATED, 100.0);
    st.side_effects(prop::KWH_RATED, 0);
    assert_eq!(st.kwh_stored, 100.0);
    assert_eq!(st.kwh_before_update, 100.0);
    assert_eq!(st.kwh_reserve, 20.0); // 100·20/100
}

/// The discharging kVA clamp: with PF=0.8 requested (varMode=PF) on a kVA=25
/// inverter discharging at 25 kW, the desired kvar = 25·√(1/0.8²−1) = 18.75
/// gives kVA √(25²+18.75²)=31.25 > 25, so the default Q-priority path backs off
/// **kW** to √(25²−18.75²) = 16.5359… while **kvar stays 18.75**. Both legs are
/// pinned exactly (not just the apparent power) so a regression that backs off
/// the wrong leg — e.g. a PF-priority kW=20/kvar=15, which also lands on the
/// kVA=25 circle with both legs positive — cannot pass. Oracle: `? kW`
/// = 16.5359456941537, `? kvar` = 18.75.
#[test]
fn kva_clamp_backs_off_kw_on_pf() {
    let mut st = Storage::new("s1");
    st.set_i32(prop::STATE, STORE_DISCHARGING);
    st.set_f64(prop::PF, 0.8);
    st.side_effects(prop::PF, 0);
    st.recalc(&ctx());
    assert!(
        (st.base.kw_out - 16.535_945_694_153_7).abs() < 1e-9,
        "kw_out = {}",
        st.base.kw_out
    );
    assert!(
        (st.base.kvar_out - 18.75).abs() < 1e-9,
        "kvar_out = {}",
        st.base.kvar_out
    );
}

/// `ControlMode=GFM` round-trips but its solve behavior is WP7.7; `gfm_mode` is
/// set and the per-element solve guard rejects it. (The solve-time error is
/// exercised by the exec integration test.)
#[test]
fn control_mode_gfm_sets_flag() {
    let mut st = Storage::new("s1");
    st.set_i32(prop::CONTROL_MODE, 1);
    st.side_effects(prop::CONTROL_MODE, 0);
    assert!(st.base.gfm_mode);
}
