//! Unit tests for the InvControl parse-only skeleton (WP7.5 step 2a): `Create`
//! defaults and the `PropertySideEffects` guards that don't need a resolved DER
//! fleet or curve. The full property round-trip (incl. `ValidateXYCurve` curve
//! nulling and `MakeLike`) is oracle-pinned by `tests/golden/props/invcontrol.json`
//! via `props_roundtrip.rs`. These are spec-pinned (Pascal is the spec): the
//! oracle exposes none of these internals outside a full InvControl solve.

use super::*;
use crate::obj::base::DssObject;

#[test]
fn create_defaults_match_pascal() {
    let ic = InvControl::new("ic1");
    // Modes default to NONE (the docs say "VoltVar" but Create sets NONE_MODE).
    assert_eq!(ic.control_mode, NONE_MODE);
    assert_eq!(ic.combi_mode, NONE_COMBMODE);
    // Convergence sentinels (FLAGDELTAQ/FLAGDELTAP = -1.0, "not set").
    assert_eq!(ic.delta_q_factor, FLAGDELTAQ);
    assert_eq!(ic.delta_p_factor, FLAGDELTAP);
    assert_eq!(ic.voltage_change_tolerance, 0.0001);
    assert_eq!(ic.var_change_tolerance, 0.025);
    assert_eq!(ic.active_p_change_tolerance, 0.01);
    // DRC dead-band + slopes.
    assert_eq!(ic.dbv_min, 0.95);
    assert_eq!(ic.dbv_max, 1.05);
    assert_eq!(ic.ar_gra_low_v, 0.1);
    assert_eq!(ic.ar_gra_hi_v, 0.1);
    // Rolling-average window lengths (docs list 0; Create sets 1).
    assert_eq!(ic.roll_avg_window_length, 1);
    assert_eq!(ic.drc_roll_avg_window_length, 1);
    // Rate-of-change off; LPFTau/RiseFall default to 0.001 (docs list 0 / -1).
    assert_eq!(ic.rate_of_change_mode, ROC_INACTIVE);
    assert_eq!(ic.lpf_tau, 0.001);
    assert_eq!(ic.rise_fall_limit, 0.001);
    // Smart-inverter defaults.
    assert_eq!(ic.voltage_curvex_ref, 0); // Rated
    assert_eq!(ic.reac_power_ref, REAC_POWER_VARAVAL);
    assert_eq!(ic.voltwatt_yaxis, 1); // %Pmpp
    assert_eq!(ic.vvc_curve_offset, 0.0);
    assert_eq!(ic.mon_buses_phase, AVGPHASES);
    assert_eq!(ic.v_setpoint, 1.0);
    assert_eq!(ic.ctrl_model, MODEL_LINEAR);
    assert!(!ic.ccd.show_event_log);
    // Control elements are 3-phase/3-conductor, single terminal.
    assert_eq!(ic.ccd.cd.nphases, 3);
    assert_eq!(ic.ccd.cd.nconds, 3);
    assert_eq!(ic.ccd.element_terminal, 1);
}

#[test]
fn mode_clears_combi_mode() {
    let mut ic = InvControl::new("ic1");
    ic.combi_mode = 1; // VV_VW
    ic.set_i32(prop::MODE, VOLTWATT); // any mode set clears CombiMode
    ic.side_effects(prop::MODE, 0);
    assert_eq!(ic.combi_mode, NONE_COMBMODE);
}

#[test]
fn dbvmin_above_dbvmax_resets_to_zero_with_error() {
    let mut ic = InvControl::new("ic1");
    // DbVMax stays 1.05; set DbVMin above it.
    ic.set_f64(prop::DBV_MIN, 1.20);
    ic.side_effects(prop::DBV_MIN, 0);
    assert_eq!(ic.dbv_min, 0.0);
    let errs = ic.ccd.cd.obj.take_errors();
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("Minimum dead-band voltage"));
}

#[test]
fn dbvmax_below_dbvmin_resets_to_zero_with_error() {
    let mut ic = InvControl::new("ic1");
    // DbVMin stays 0.95; set DbVMax below it.
    ic.set_f64(prop::DBV_MAX, 0.50);
    ic.side_effects(prop::DBV_MAX, 0);
    assert_eq!(ic.dbv_max, 0.0);
    let errs = ic.ccd.cd.obj.take_errors();
    assert_eq!(errs.len(), 1);
    assert!(errs[0].contains("Maximum dead-band voltage"));
}

#[test]
fn nonpositive_lpf_tau_inactivates_rate_of_change() {
    let mut ic = InvControl::new("ic1");
    ic.rate_of_change_mode = 1; // LPF
    ic.set_f64(prop::LPF_TAU, 0.0);
    ic.side_effects(prop::LPF_TAU, 0);
    assert_eq!(ic.rate_of_change_mode, ROC_INACTIVE);
}

#[test]
fn nonpositive_rise_fall_inactivates_rate_of_change() {
    let mut ic = InvControl::new("ic1");
    ic.rate_of_change_mode = 2; // RiseFall
    ic.set_f64(prop::RISE_FALL_LIMIT, -1.0);
    ic.side_effects(prop::RISE_FALL_LIMIT, 0);
    assert_eq!(ic.rate_of_change_mode, ROC_INACTIVE);
}

#[test]
fn derlist_sets_list_size() {
    let mut ic = InvControl::new("ic1");
    ic.set_string_list(
        prop::DER_LIST,
        vec!["pvsystem.pv1".into(), "storage.st1".into()],
    );
    ic.side_effects(prop::DER_LIST, 0);
    assert_eq!(ic.f_list_size, 2);
}

#[test]
fn pvsystemlist_prepends_class_and_sizes_list() {
    // The deprecated PVSystemList assumes bare PVSystem names and prepends the
    // class, then re-runs the DERList side effect.
    let mut ic = InvControl::new("ic1");
    ic.set_string_list(prop::PVSYSTEM_LIST, vec!["pv1".into(), "pv2".into()]);
    ic.side_effects(prop::PVSYSTEM_LIST, 0);
    assert_eq!(ic.der_name_list, vec!["PVSystem.pv1", "PVSystem.pv2"]);
    assert_eq!(ic.f_list_size, 2);
    // PVSystemList shares the DERList backing, so it dumps the same list.
    assert_eq!(
        ic.get_string_list(prop::DER_LIST),
        ic.get_string_list(prop::PVSYSTEM_LIST)
    );
}
