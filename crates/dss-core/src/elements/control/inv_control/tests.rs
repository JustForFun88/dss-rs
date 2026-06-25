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

// --- WP7.5 step 2b: the VOLTVAR dispatch math, pinned through a mock env ---
//
// The full end-to-end VOLTVAR convergence is oracle-pinned by
// `tests/golden/phase7/invcontrol_voltvar.json` (the live corpus volt-var family
// migrates it too). These mock-env tests pin the *per-call* arithmetic — the
// fleet build, the Sample trigger, `Calc_QHeadRoom`, and the first DoPendingAction
// curve→clamp→delta-Q step — independent of the PVSystem injection model.
mod voltvar {
    use super::super::compute::{DerSnap, FleetFind, InvDispatchEnv, MonitorVar};
    use super::super::{InvControl, prop};
    use crate::elements::traits::ElemRef;
    use crate::obj::base::DssObject;

    /// One mock DER (a PVSystem-shaped inverter with var headroom).
    #[derive(Clone)]
    struct MockDer {
        name: String,
        enabled: bool,
        vmag: f64, // per-phase terminal voltage magnitude (balanced)
        vbase: f64,
        present_kw: f64,
        kva_rating: f64,
        kvar_limit: f64,
        kvar_limit_neg: f64,
        p_priority: bool,
        /// The last `der_set_kvar_requested` value (post-`SetNominalDEROutput`
        /// readback is modeled ideal: the requested kvar clamped to ±kvarLimit).
        requested_kvar: f64,
    }
    impl MockDer {
        fn new(name: &str, vpu: f64, present_kw: f64) -> Self {
            let vbase = 7200.0;
            Self {
                name: name.into(),
                enabled: true,
                vmag: vpu * vbase,
                vbase,
                present_kw,
                kva_rating: 600.0,
                kvar_limit: 600.0,
                kvar_limit_neg: 600.0,
                p_priority: false,
                requested_kvar: 0.0,
            }
        }
    }

    struct MockEnv {
        ders: Vec<MockDer>,
        pushes: Vec<i32>,
        errors: Vec<String>,
        events: Vec<String>,
        control_iter: i32,
    }
    impl MockEnv {
        fn new(ders: Vec<MockDer>) -> Self {
            Self {
                ders,
                pushes: Vec::new(),
                errors: Vec::new(),
                events: Vec::new(),
                control_iter: 1,
            }
        }
        fn idx(r: ElemRef) -> usize {
            r.idx
        }
    }
    impl InvDispatchEnv for MockEnv {
        fn find_pvsystem(&self, name: &str) -> FleetFind {
            match self
                .ders
                .iter()
                .position(|d| d.name.eq_ignore_ascii_case(name))
            {
                None => FleetFind::NotFound,
                Some(i) if self.ders[i].enabled => FleetFind::Found(ElemRef { cls: 0, idx: i }),
                Some(_) => FleetFind::Disabled,
            }
        }
        fn find_storage(&self, _name: &str) -> FleetFind {
            FleetFind::NotFound
        }
        fn all_pvsystems(&self) -> Vec<(String, ElemRef, bool)> {
            self.ders
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    (
                        format!("PVSystem.{}", d.name),
                        ElemRef { cls: 0, idx: i },
                        d.enabled,
                    )
                })
                .collect()
        }
        fn all_storages(&self) -> Vec<(String, ElemRef, bool)> {
            Vec::new()
        }
        fn push_error(&mut self, msg: String) {
            self.errors.push(msg);
        }
        fn der_snap(&self, r: ElemRef) -> DerSnap {
            let d = &self.ders[Self::idx(r)];
            DerSnap {
                is_pvsystem: true,
                nphases: 3,
                nterms: 1,
                nconds: 4,
                vbase: d.vbase,
                var_follow_inverter: false,
                inverter_on: true,
                present_kw: d.present_kw,
                kva_rating: d.kva_rating,
                present_kvar: d.requested_kvar,
                kvar_limit: d.kvar_limit,
                kvar_limit_neg: d.kvar_limit_neg,
                // Pascal `ComputeInverterPower` sets CurrentkvarLimit := kvarLimit
                // in the normal (non-Pmin) path.
                current_kvar_limit: d.kvar_limit,
                current_kvar_limit_neg: d.kvar_limit_neg,
                p_priority: d.p_priority,
            }
        }
        fn der_vterminal_mags(&mut self, r: ElemRef) -> Vec<f64> {
            let v = self.ders[Self::idx(r)].vmag;
            vec![v, v, v]
        }
        fn der_bus_vbase(&self, r: ElemRef) -> f64 {
            self.ders[Self::idx(r)].vbase
        }
        fn der_full_name(&self, r: ElemRef) -> String {
            format!("PVSystem.{}", self.ders[Self::idx(r)].name)
        }
        fn der_set_pf_priority(&mut self, r: ElemRef, value: bool) {
            self.ders[Self::idx(r)].p_priority = value;
        }
        fn der_set_modes(&mut self, _r: ElemRef, _vw: bool, _vv: bool, _var_mode: i32) {}
        fn der_set_kvar_requested(&mut self, r: ElemRef, q: f64) {
            let d = &mut self.ders[Self::idx(r)];
            // Model SetNominalDEROutput's kvar clamp to the limit band.
            d.requested_kvar = q.clamp(-d.kvar_limit_neg, d.kvar_limit);
        }
        fn der_set_nominal(&mut self, _r: ElemRef) {}
        fn der_present_kvar(&self, r: ElemRef) -> f64 {
            self.ders[Self::idx(r)].requested_kvar
        }
        fn der_set_monitor_var(&mut self, _r: ElemRef, _kind: MonitorVar, _value: f64) {}
        fn push_change(&mut self, _delay: f64, code: i32) {
            self.pushes.push(code);
        }
        fn append_event(&mut self, der: &str, msg: &str) {
            self.events.push(format!("{der}: {msg}"));
        }
        fn control_iteration(&self) -> i32 {
            self.control_iter
        }
        fn dyna_h(&self) -> f64 {
            1.0
        }
        fn dbl_hour(&self) -> f64 {
            0.0
        }
    }

    /// A VOLTVAR control over a `vvc_curve` that absorbs above 1.0 pu, named-list
    /// fleet `pv`, RefReactivePower=VARMAX, deltaQ_factor=0.2.
    fn voltvar_ic() -> InvControl {
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::VOLTVAR);
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        ic.set_f64(prop::DELTA_Q_FACTOR, 0.2);
        // The volt-var curve (same shape as the corpus Standard cases).
        let curve = crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vv",
            &[0.5, 0.92, 1.0, 1.08, 1.5],
            &[1.0, 1.0, 0.0, -1.0, -1.0],
        );
        ic.vvc_curve = Some(curve);
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        ic
    }

    #[test]
    fn make_der_list_named_missing_errors_14403() {
        let mut ic = voltvar_ic();
        let mut env = MockEnv::new(vec![MockDer::new("other", 1.0, 300.0)]);
        // The named "pv" is absent → Pascal 14403 + an empty fleet.
        assert!(ic.sample(&mut env).is_ok());
        assert_eq!(env.errors.len(), 1);
        assert!(env.errors[0].contains("PVSystem Element \"PVSystem.pv\" not found"));
        assert!(ic.fleet.is_empty());
    }

    #[test]
    fn empty_der_list_auto_populates_from_circuit() {
        let mut ic = voltvar_ic();
        // Clear the named list → the empty-list branch scans the circuit.
        ic.set_string_list(prop::DER_LIST, Vec::new());
        ic.side_effects(prop::DER_LIST, 0);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.0, 300.0)]);
        let _ = ic.sample(&mut env);
        assert_eq!(ic.fleet.len(), 1);
        assert_eq!(ic.der_name_list, vec!["PVSystem.pv"]);
    }

    #[test]
    fn sample_first_control_iteration_pushes_changevarlevel() {
        let mut ic = voltvar_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        env.control_iter = 1;
        ic.sample(&mut env).unwrap();
        // ControlIteration 1 always pushes a CHANGEVARLEVEL action.
        assert_eq!(env.pushes, vec![super::super::CHANGEVARLEVEL]);
    }

    #[test]
    fn do_pending_action_absorbs_above_deadband() {
        // V = 1.05 pu → curve y = -0.625; QHeadRoom(VARMAX) = kvarLimit = 600.
        // QDesireEndpu = -0.625; CalcVoltVar_vars (Linear, deltaQ=0.2, QOldVV=-1):
        //   DeltaQ = -0.625*600 - (-1) = -374; QDesiredVV = -1 + (-374)*0.2 = -75.8.
        let mut ic = voltvar_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.q_headroom - 600.0).abs() < 1e-9,
            "QHeadRoom = {}",
            cv.q_headroom
        );
        assert!(
            (cv.q_desire_vvpu - (-0.625)).abs() < 1e-9,
            "QDesireVVpu = {}",
            cv.q_desire_vvpu
        );
        assert!(
            (cv.q_desired_vv - (-75.8)).abs() < 1e-9,
            "QDesiredVV = {}",
            cv.q_desired_vv
        );
        assert!((env.ders[0].requested_kvar - (-75.8)).abs() < 1e-9);
    }

    #[test]
    fn calc_qheadroom_varaval_uses_kva_circle() {
        // VARAVAL: QHeadRoom = sqrt(kVA^2 - presentkW^2) = sqrt(600^2-300^2).
        let mut ic = voltvar_ic();
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARAVAL);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        let expected = (600.0_f64.powi(2) - 300.0_f64.powi(2)).sqrt();
        assert!(
            (ic.ctrl_vars[0].q_headroom - expected).abs() < 1e-9,
            "QHeadRoom = {} expected {expected}",
            ic.ctrl_vars[0].q_headroom
        );
    }
}
