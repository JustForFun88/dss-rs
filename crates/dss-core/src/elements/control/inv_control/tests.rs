//! Unit tests for the InvControl parse-only skeleton (WP7.5 step 2a): `Create`
//! defaults and the `PropertySideEffects` guards that don't need a resolved DER
//! fleet or curve. The full property round-trip (incl. `ValidateXYCurve` curve
//! nulling and `MakeLike`) is oracle-pinned by `tests/golden/props/invcontrol.json`
//! via `props_roundtrip.rs`. These are spec-pinned (Pascal is the spec): the
//! oracle exposes none of these internals outside a full InvControl solve.

use super::*;
use crate::exec::Dss;
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

#[test]
fn interval_units_bad_unit_logs_error_and_keeps_default() {
    // A bad time-unit suffix on an IntervalUnits property logs the Pascal error
    // (DSSObjectHelper l.347: 2020035) and leaves the field at its default
    // (AvgWindowLen = 1), matching Pascal's `Exit` (field unchanged). The happy
    // path (2m -> 120) is oracle-pinned by props/invcontrol.json; this pins the
    // error wiring end-to-end through the parse engine.
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.t");
    dss.command("new InvControl.ic avgwindowlen=2x");
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("Units can only be h, m, or s")),
        "expected an IntervalUnits error, got: {:?}",
        dss.errors()
    );
    dss.command("? InvControl.ic.AvgWindowLen");
    assert_eq!(
        dss.result().trim(),
        "1",
        "AvgWindowLen must keep its default on a bad unit suffix"
    );
}

// --- WP7.5 step 2b/2c: the dispatch math, pinned through a mock env ---
//
// The full end-to-end convergence is oracle-pinned by the
// `tests/golden/phase7/invcontrol_{voltvar,voltwatt,vv_vw}.json` goldens (the live
// corpus volt-var/volt-watt families migrate them too). These mock-env tests pin
// the *per-call* arithmetic — the fleet build, the Sample triggers, `Calc_QHeadRoom`/
// `Calc_PBase`, and the first DoPendingAction curve→clamp→delta step — independent
// of the PVSystem injection model.
mod dispatch {
    use super::super::compute::{DerSnap, FleetFind, InvDispatchEnv, MonitorVar};
    use super::super::{InvControl, prop};
    use crate::elements::traits::ElemRef;
    use crate::obj::base::DssObject;

    /// One mock DER (a PVSystem-shaped inverter with var headroom).
    #[derive(Clone)]
    struct MockDer {
        name: String,
        enabled: bool,
        is_storage: bool, // der_snap reports is_pvsystem = !is_storage
        vmag: f64,        // per-phase terminal voltage magnitude (balanced)
        vbase: f64,
        present_kw: f64,
        kva_rating: f64,
        kvar_limit: f64,
        kvar_limit_neg: f64,
        p_priority: bool,
        pf_priority: bool,
        /// The last `der_set_kvar_requested` value (post-`SetNominalDEROutput`
        /// readback is modeled ideal: the requested kvar clamped to ±kvarLimit).
        requested_kvar: f64,
        /// The last `der_set_pf_wp_nominal` value (WATTPF; PVSystem only).
        pf_wp_nominal: f64,
        // --- volt-watt fields (Calc_PBase / Check_Plimits) ---
        pmpp: f64,       // FDCkWRated
        pu_pmpp: f64,    // FpctDCkWRated
        eff_factor: f64, // FEffFactor
        panel_kw: f64,   // FDCkW
        /// The last `der_set_kw_requested` value (ideal readback for `der_present_kw`).
        requested_kw: f64,
    }
    impl MockDer {
        fn new(name: &str, vpu: f64, present_kw: f64) -> Self {
            let vbase = 7200.0;
            Self {
                name: name.into(),
                enabled: true,
                is_storage: false,
                vmag: vpu * vbase,
                vbase,
                present_kw,
                kva_rating: 600.0,
                kvar_limit: 600.0,
                kvar_limit_neg: 600.0,
                p_priority: false,
                pf_priority: false,
                requested_kvar: 0.0,
                pf_wp_nominal: 1.0,
                pmpp: 600.0,
                pu_pmpp: 1.0,
                eff_factor: 1.0,
                panel_kw: present_kw,
                requested_kw: present_kw,
            }
        }
    }

    struct MockEnv {
        ders: Vec<MockDer>,
        pushes: Vec<i32>,
        errors: Vec<String>,
        control_iter: i32,
    }
    impl MockEnv {
        fn new(ders: Vec<MockDer>) -> Self {
            Self {
                ders,
                pushes: Vec::new(),
                errors: Vec::new(),
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
                is_pvsystem: !d.is_storage,
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
                pf_priority: d.pf_priority,
                dckw: d.panel_kw,
                dckw_rated: d.pmpp,
                pct_dckw_rated: d.pu_pmpp,
                eff_factor: d.eff_factor,
            }
        }
        fn der_is_pvsystem(&self, r: ElemRef) -> bool {
            !self.ders[Self::idx(r)].is_storage
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
        fn der_set_vv_mode(&mut self, _r: ElemRef, _value: bool) {}
        fn der_set_vw_mode(&mut self, _r: ElemRef, _value: bool) {}
        fn der_set_drc_mode(&mut self, _r: ElemRef, _value: bool) {}
        fn der_set_wp_mode(&mut self, _r: ElemRef, _value: bool) {}
        fn der_set_wv_mode(&mut self, _r: ElemRef, _value: bool) {}
        fn der_set_pf_wp_nominal(&mut self, r: ElemRef, value: f64) {
            self.ders[Self::idx(r)].pf_wp_nominal = value;
        }
        fn der_set_kvar_requested(&mut self, r: ElemRef, q: f64) {
            let d = &mut self.ders[Self::idx(r)];
            // Model SetNominalDEROutput's kvar clamp to the limit band.
            d.requested_kvar = q.clamp(-d.kvar_limit_neg, d.kvar_limit);
        }
        fn der_set_kw_requested(&mut self, r: ElemRef, p: f64) {
            self.ders[Self::idx(r)].requested_kw = p;
        }
        fn der_set_nominal(&mut self, _r: ElemRef) {}
        fn der_present_kvar(&self, r: ElemRef) -> f64 {
            self.ders[Self::idx(r)].requested_kvar
        }
        fn der_present_kw(&self, r: ElemRef) -> f64 {
            // Ideal readback: the requested kW limit (the VW set-point).
            self.ders[Self::idx(r)].requested_kw
        }
        fn der_set_monitor_var(&mut self, _r: ElemRef, _kind: MonitorVar, _value: f64) {}
        fn push_change(&mut self, _delay: f64, code: i32) {
            self.pushes.push(code);
        }
        fn append_event(&mut self, _der: &str, _msg: &str) {}
        fn control_iteration(&self) -> i32 {
            self.control_iter
        }
        fn dyna_h(&self) -> f64 {
            1.0
        }
        fn dbl_hour(&self) -> f64 {
            0.0
        }
        fn dyna_t(&self) -> f64 {
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
    fn do_pending_action_injects_below_deadband() {
        // V = 0.90 pu → curve y = +1.0 (inject); QHeadRoom(VARMAX) = 600.
        // Check_Qlimits clamps QDesireEndpu to the current kvar limit (1.0 pu), then
        // CalcVoltVar_vars: DeltaQ = 1.0*600 - (-1) = 601; QDesiredVV = -1 + 601*0.2
        // = +119.2 — the positive (injecting) branch, exercising `QHeadRoom` (not
        // `QHeadRoomNeg`). Mirrors the absorb test in the opposite direction.
        let mut ic = voltvar_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 0.90, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.q_desire_vvpu - 1.0).abs() < 1e-9,
            "QDesireVVpu = {}",
            cv.q_desire_vvpu
        );
        assert!(
            cv.q_desired_vv > 0.0 && (cv.q_desired_vv - 119.2).abs() < 1e-9,
            "QDesiredVV = {} (expected +119.2, injecting)",
            cv.q_desired_vv
        );
    }

    #[test]
    fn exponential_control_model_aborts_not_silently() {
        // The Exponential ControlModel runs the (unported) PICtrl PI controller in
        // CalcVoltVar_vars; Sample must reject it with an explicit error, never run
        // the silent "stay put" branch (the deferral-is-never-a-silent-skip rule).
        let mut ic = voltvar_ic();
        ic.set_i32(prop::CONTROL_MODEL, 1); // Exponential
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        let err = ic.sample(&mut env).unwrap_err();
        assert!(
            err.contains("Exponential ControlModel"),
            "expected an Exponential NOT_PORTED error, got: {err}"
        );
    }

    #[test]
    fn named_missing_does_not_re_error_when_a_valid_der_precedes_it() {
        // Pascal re-runs MakeDERList only when FDERPointerList.Count = 0; a partial
        // named list `[valid, missing]` keeps its valid prefix (Count > 0), so the
        // 14403 fires once — not every Sample.
        let mut ic = voltvar_ic();
        ic.set_string_list(
            prop::DER_LIST,
            vec!["PVSystem.pv".into(), "PVSystem.gone".into()],
        );
        ic.side_effects(prop::DER_LIST, 0);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.sample(&mut env).unwrap(); // second Sample must NOT re-error
        assert_eq!(env.errors.len(), 1, "errors: {:?}", env.errors);
        assert_eq!(ic.fleet.len(), 1); // the valid prefix is retained
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

    // --- WP7.5 step 2c: VOLTWATT + VV_VW ---

    /// A VOLTWATT control with a volt-watt curve that limits above 1.02 pu,
    /// VoltWattYAxis=%Pmpp (default, PBase = Pmpp), DeltaP_factor=0.45 (a *set*
    /// factor → used directly each iteration), named-list fleet `pv`.
    fn voltwatt_ic() -> InvControl {
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::VOLTWATT);
        ic.set_f64(prop::DELTA_P_FACTOR, 0.45);
        let curve = crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vw",
            &[1.0, 1.02, 1.1],
            &[1.0, 1.0, 0.0],
        );
        ic.voltwatt_curve = Some(curve);
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        ic
    }

    #[test]
    fn voltwatt_limits_kw_above_curve_knee() {
        // V = 1.05 pu → curve y = 1 - (1.05-1.02)/(1.1-1.02) = 0.625 → PLimitVWpu.
        // PBase(%Pmpp) = Pmpp = 600. kW_out_desiredpu = presentkW/PBase = 600/600 = 1.
        // Check_Plimits: no kVA/pctPmpp clamp (var priority headroom 600 > 0.625*600=375;
        //   pctPmpp 600 > 375) → PLimitLimitedpu = 1, PLimitEndpu = 0.625.
        // CalcVoltWatt_watts (iter 1, requesting region): POldVWpu = |1| = 1;
        //   DeltaP = 0.625 - 1 = -0.375; PLimitVW = (1 + (-0.375)*0.45)*600 = 498.75.
        let mut ic = voltwatt_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 600.0)]);
        ic.sample(&mut env).unwrap();
        assert_eq!(env.pushes, vec![super::super::CHANGEWATTLEVEL]);
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.p_limit_vw_pu - 0.625).abs() < 1e-9,
            "PLimitVWpu = {}",
            cv.p_limit_vw_pu
        );
        assert!(
            (cv.p_limit_endpu - 0.625).abs() < 1e-9,
            "PLimitEndpu = {}",
            cv.p_limit_endpu
        );
        assert!(
            (cv.p_limit_vw - 498.75).abs() < 1e-9,
            "PLimitVW = {} (expected 498.75)",
            cv.p_limit_vw
        );
        assert!((env.ders[0].requested_kw - 498.75).abs() < 1e-9);
    }

    #[test]
    fn voltwatt_no_limit_below_curve_knee() {
        // V = 1.0 pu → curve y = 1.0 (no limit). PLimitEndpu = 1.0; not in the
        // requesting region (1.0 < 1.0 is false) → PLimitVW = PLimitEndpu*PBase = 600.
        let mut ic = voltwatt_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.0, 600.0)]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.p_limit_vw_pu - 1.0).abs() < 1e-9,
            "PLimitVWpu = {}",
            cv.p_limit_vw_pu
        );
        assert!(
            (cv.p_limit_vw - 600.0).abs() < 1e-9,
            "PLimitVW = {}",
            cv.p_limit_vw
        );
    }

    #[test]
    fn voltwatt_storage_is_deferred_not_silent() {
        // The Storage VOLTWATT/VV_VW dispatch is deferred with an explicit error
        // (the YPrim-state-flip propagation gap; PVSystem volt-watt is ported).
        let mut ic = voltwatt_ic();
        let mut der = MockDer::new("pv", 1.05, 600.0);
        der.is_storage = true;
        let mut env = MockEnv::new(vec![der]);
        let err = ic.sample(&mut env).unwrap_err();
        assert!(
            err.contains("Storage VOLTWATT/VV_VW"),
            "expected a Storage VW NOT_PORTED error, got: {err}"
        );
    }

    #[test]
    fn vv_vw_storage_is_deferred_not_silent() {
        // Symmetry with the VOLTWATT case: a Storage in the VV_VW combi also errors
        // (the shared `guard_storage_vw`), never silently dispatching.
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::COMBI_MODE, super::super::VV_VW);
        ic.voltwatt_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vw",
            &[1.0, 1.02, 1.1],
            &[1.0, 1.0, 0.0],
        ));
        ic.vvc_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vv",
            &[0.5, 1.0, 1.5],
            &[1.0, 0.0, -1.0],
        ));
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        let mut der = MockDer::new("pv", 1.05, 600.0);
        der.is_storage = true;
        let mut env = MockEnv::new(vec![der]);
        let err = ic.sample(&mut env).unwrap_err();
        assert!(
            err.contains("Storage VOLTWATT/VV_VW"),
            "expected a Storage VV_VW NOT_PORTED error, got: {err}"
        );
    }

    #[test]
    fn vv_vw_dispatches_both_kw_and_kvar() {
        // CombiMode=VV_VW over a PV at 1.05 pu: the single DoPendingAction sets BOTH
        // the volt-watt kW limit (curve y=0.625 → PLimitVW=498.75, as in the VW test)
        // AND a volt-var kvar (curve y=-0.625, VARMAX → QDesireEndpu=-0.625; QHeadRoom
        // =600; QOldVV=-1 → QDesiredVV = -1 + (-0.625*600 - (-1))*0.2 = -75.8).
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::COMBI_MODE, super::super::VV_VW);
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        ic.set_f64(prop::DELTA_Q_FACTOR, 0.2);
        ic.set_f64(prop::DELTA_P_FACTOR, 0.45);
        ic.voltwatt_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vw",
            &[1.0, 1.02, 1.1],
            &[1.0, 1.0, 0.0],
        ));
        ic.vvc_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vv",
            &[0.5, 0.92, 1.0, 1.08, 1.5],
            &[1.0, 1.0, 0.0, -1.0, -1.0],
        ));
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);

        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 600.0)]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.p_limit_vw - 498.75).abs() < 1e-9,
            "PLimitVW = {} (expected 498.75)",
            cv.p_limit_vw
        );
        assert!(
            (cv.q_desired_vv - (-75.8)).abs() < 1e-9,
            "QDesiredVV = {} (expected -75.8)",
            cv.q_desired_vv
        );
        // Both set-points reached the DER (one SetNominalDEROutput each).
        assert!((env.ders[0].requested_kw - 498.75).abs() < 1e-9);
        assert!((env.ders[0].requested_kvar - (-75.8)).abs() < 1e-9);
    }

    #[test]
    fn vv_vw_double_push_dispatches_once_via_pending_reset() {
        // The VV_VW Sample fires BOTH the volt-watt and the volt-var trigger on
        // ControlIteration 1, queuing CHANGEWATTVARLEVEL twice. DoPendingAction must
        // dispatch the DER only ONCE (Pascal resets FPendingChange to NONE at the end
        // of the loop body, so the second queued action is a no-op). Asserted by the
        // two pushes + a single net convergence step (POldVWpu advanced once: from the
        // iter-1 seed |kW_out_desiredpu|=1 toward PLimitEndpu, not twice).
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::COMBI_MODE, super::super::VV_VW);
        ic.set_f64(prop::DELTA_P_FACTOR, 0.45);
        ic.voltwatt_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vw",
            &[1.0, 1.02, 1.1],
            &[1.0, 1.0, 0.0],
        ));
        ic.vvc_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vv",
            &[0.5, 0.92, 1.0, 1.08, 1.5],
            &[1.0, 1.0, 0.0, -1.0, -1.0],
        ));
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);

        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 600.0)]);
        ic.sample(&mut env).unwrap();
        // Both triggers fired → two queued actions.
        assert_eq!(
            env.pushes,
            vec![
                super::super::CHANGEWATTVARLEVEL,
                super::super::CHANGEWATTVARLEVEL
            ]
        );
        // Drive DoPendingAction twice (the queue would pop both). The pending-change
        // reset makes the second call a no-op, so PLimitVW is the single-step result.
        ic.do_pending_action(&mut env);
        let after_first = ic.ctrl_vars[0].p_limit_vw;
        ic.do_pending_action(&mut env);
        let after_second = ic.ctrl_vars[0].p_limit_vw;
        assert_eq!(
            ic.ctrl_vars[0].f_pending_change,
            super::super::CHANGE_NONE,
            "pending change must be reset after dispatch"
        );
        assert_eq!(
            after_first, after_second,
            "the second DoPendingAction must be a no-op (pending reset), not a second step"
        );
        assert!(
            (after_first - 498.75).abs() < 1e-9,
            "PLimitVW = {after_first}"
        );
    }

    // --- WP7.5 step 2d: DRC + VV_DRC ---
    //
    // DRC has no curve: `CalcQDRC_desiredpu` derives the desired var from the
    // per-step voltage *change* vs the DRC rolling-average window, so these mocks
    // seed the window directly (the window is fed only by the time-step cleanup,
    // which the mock env does not run — exactly why the end-to-end gate is the
    // *daily* `phase7/invcontrol_drc` golden, not a snapshot).

    /// A DRC control: no curve, zero-width deadband (DbVMin=DbVMax=1.0), steep
    /// slopes (ArGra=50), VARMAX, deltaQ_factor=0.2, named-list fleet `pv`.
    fn drc_ic() -> InvControl {
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::DRC);
        ic.dbv_min = 1.0;
        ic.dbv_max = 1.0;
        ic.ar_gra_low_v = 50.0;
        ic.ar_gra_hi_v = 50.0;
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        ic.set_f64(prop::DELTA_Q_FACTOR, 0.2);
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        ic
    }

    #[test]
    fn drc_absorbs_on_rising_voltage() {
        // V = 1.05 pu; seed the DRC window at 1.0 pu (vmag 7200 on the 7200 V base)
        // -> deltaV = +0.05. V > DbVMax(1.0) -> QDesireDRCpu = -deltaV*ArGraHiV =
        // -0.05*50 = -2.5. Check_Qlimits clamps QDesireEndpu to -1.0 (kvar limit pu);
        // CalcDRC_vars (deltaQ=0.2, QOldDRC=-1): QDesiredDRC = -1 + (-1*600 - (-1))*0.2
        // = -120.8.
        let mut ic = drc_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap(); // builds the fleet; iter 1 queues CHANGEVARLEVEL
        ic.ctrl_vars[0]
            .f_drc_roll_avg_window
            .add(7200.0, 3600.0, 2.0);
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.q_desire_drcpu - (-2.5)).abs() < 1e-9,
            "QDesireDRCpu = {}",
            cv.q_desire_drcpu
        );
        assert!(
            (cv.q_desired_drc - (-120.8)).abs() < 1e-9,
            "QDesiredDRC = {} (expected -120.8)",
            cv.q_desired_drc
        );
        assert!((env.ders[0].requested_kvar - (-120.8)).abs() < 1e-9);
    }

    #[test]
    fn drc_is_a_noop_when_window_empty() {
        // The pure-snapshot case: with the DRC rolling-average window empty
        // (AvgVal = 0) the law forces deltaV -> 0, so QDesireDRCpu = 0 (no
        // dynamic-reactive-current request). This is why DRC must be gated by a
        // multi-step golden, not a snapshot.
        let mut ic = drc_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // window never seeded
        assert_eq!(
            ic.ctrl_vars[0].q_desire_drcpu, 0.0,
            "DRC must request 0 vars with an empty window"
        );
    }

    #[test]
    fn vv_drc_sums_curve_and_drc_q() {
        // CombiMode=VV_DRC at V = 1.01 pu (just above the deadband) with the window
        // seeded at 1.0 pu (deltaV = +0.01):
        //   QDesireVVpu  = vv curve @1.01 = -0.125 (linear 1.0->1.08 maps 0->-1),
        //   QDesireDRCpu = -deltaV*ArGraHiV = -0.01*50 = -0.5,
        //   q_sum = -0.625 (NOT clamped: |q| < kvar-limit pu 1.0),
        //   QDesireEndpu = -0.625; CalcVVDRC_vars (deltaQ=0.2, QOldVVDRC=-1):
        //   QDesiredVVDRC = -1 + (-0.625*600 - (-1))*0.2 = -75.8.
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::COMBI_MODE, super::super::VV_DRC);
        ic.dbv_min = 1.0;
        ic.dbv_max = 1.0;
        ic.ar_gra_low_v = 50.0;
        ic.ar_gra_hi_v = 50.0;
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        ic.set_f64(prop::DELTA_Q_FACTOR, 0.2);
        ic.vvc_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vv",
            &[0.5, 0.92, 1.0, 1.08, 1.5],
            &[1.0, 1.0, 0.0, -1.0, -1.0],
        ));
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);

        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.01, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.ctrl_vars[0]
            .f_drc_roll_avg_window
            .add(7200.0, 3600.0, 2.0);
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.q_desire_vvpu - (-0.125)).abs() < 1e-9,
            "QDesireVVpu = {}",
            cv.q_desire_vvpu
        );
        assert!(
            (cv.q_desire_drcpu - (-0.5)).abs() < 1e-9,
            "QDesireDRCpu = {}",
            cv.q_desire_drcpu
        );
        assert!(
            (cv.q_desired_vvdrc - (-75.8)).abs() < 1e-9,
            "QDesiredVVDRC = {} (expected -75.8)",
            cv.q_desired_vvdrc
        );
        assert!((env.ders[0].requested_kvar - (-75.8)).abs() < 1e-9);
    }

    #[test]
    fn vv_drc_pushes_changedrcvvarlevel() {
        // The VV_DRC Sample queues CHANGEDRCVVARLEVEL (4), not CHANGEVARLEVEL — the
        // dedicated combi action code the joint DoPendingAction dispatches on.
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::COMBI_MODE, super::super::VV_DRC);
        ic.vvc_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vv",
            &[0.5, 1.0, 1.5],
            &[1.0, 0.0, -1.0],
        ));
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap();
        // Both the DRC and the volt-var trigger fire on iter 1 (the latter via the
        // ControlIteration==1 arm); both queue CHANGEDRCVVARLEVEL.
        assert!(
            env.pushes
                .iter()
                .all(|&c| c == super::super::CHANGEDRCVVARLEVEL),
            "expected only CHANGEDRCVVARLEVEL, got {:?}",
            env.pushes
        );
        assert!(!env.pushes.is_empty());
    }

    #[test]
    fn avr_mode_aborts_not_silently() {
        // AVR is still deferred (step 2e-ii); Sample must reject it loudly, never
        // silently no-op (the deferral-is-never-a-silent-skip rule). (WATTPF/WATTVAR
        // are now ported — see `wattpf_*`/`wattvar_*` below.)
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::AVR);
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        let err = ic.sample(&mut env).unwrap_err();
        assert!(
            err.contains("deferred to 2e-ii"),
            "expected an AVR NOT_PORTED error, got: {err}"
        );
    }

    /// A WATTPF control over a `wattpf_curve`, RefReactivePower=VARMAX.
    fn wattpf_ic() -> InvControl {
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::WATTPF);
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        // pf vs panel-pu: at full output the inverter runs pf = -0.9 (absorbing).
        let curve = crate::elements::general::xy_curve::XyCurveObj::from_points(
            "wpf",
            &[0.0, 0.5, 1.0],
            &[1.0, 1.0, -0.9],
        );
        ic.wattpf_curve = Some(curve);
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        ic
    }

    #[test]
    fn wattpf_first_step_curve_to_kvar() {
        // The watt-pf curve gives pf = -0.9 at the panel pu (1.0). With neither P-
        // nor PF-priority, p = panel power = FDCkW·FEffFactor·FpctDCkWRated. The
        // desired kvar is p·tan(acos(0.9))·sign(-0.9) = -600·0.484123 = -290.47,
        // within the ±600 kvar limit, so it is pushed straight through.
        let mut ic = wattpf_ic();
        // panel_kw = pmpp = 600, eff = 1, pu_pmpp = 1 → panel pu = 1.0, p = 600.
        let mut der = MockDer::new("pv", 1.0, 600.0);
        der.panel_kw = 600.0;
        der.pmpp = 600.0;
        der.kva_rating = 1000.0; // room so the kVA clamp does not bite
        let mut env = MockEnv::new(vec![der]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);

        // pf_wp_nominal recorded on the (PVSystem) DER.
        assert!((env.ders[0].pf_wp_nominal - (-0.9)).abs() < 1e-12);
        // p·tan(acos(0.9)) absorbed (pf = -0.9 → negative kvar).
        let expected_q = -600.0 * (1.0 / 0.81 - 1.0_f64).sqrt();
        assert!(
            (env.ders[0].requested_kvar - expected_q).abs() < 1e-9,
            "wattpf kvar {} != {expected_q}",
            env.ders[0].requested_kvar
        );
    }

    /// A WATTVAR control over a `wattvar_curve`, RefReactivePower=VARMAX.
    fn wattvar_ic() -> InvControl {
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::WATTVAR);
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        // watt-var: at full output, absorb -0.4 pu of headroom.
        let curve = crate::elements::general::xy_curve::XyCurveObj::from_points(
            "wv",
            &[0.0, 0.5, 1.0],
            &[0.0, 0.0, -0.4],
        );
        ic.wattvar_curve = Some(curve);
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        ic
    }

    #[test]
    fn wattvar_first_step_curve_to_kvar() {
        // The watt-var curve gives Q = -0.4 pu of headroom at panel pu = 1.0. With a
        // 600-kvar VARMAX headroom that is QDesireEndpu = -0.4 (within the kvar
        // limit, not kVA-bound since kVArating = 1000 leaves room), so
        // QDesiredWV = -0.4·600 = -240 kvar.
        let mut ic = wattvar_ic();
        let mut der = MockDer::new("pv", 1.0, 600.0);
        der.panel_kw = 600.0;
        der.pmpp = 600.0;
        der.kva_rating = 1000.0;
        let mut env = MockEnv::new(vec![der]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        assert!(
            (env.ders[0].requested_kvar - (-240.0)).abs() < 1e-9,
            "wattvar kvar {} != -240",
            env.ders[0].requested_kvar
        );
    }
}
