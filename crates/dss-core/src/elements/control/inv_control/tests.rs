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
// `tests/golden/der_controls/invcontrol_{voltvar,voltwatt,vv_vw}.json` goldens (the live
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
        /// The DER `Varmode` (Pascal default VARMODE_PF=0); set to VARMODE_KVAR=1 by
        /// `der_set_var_mode` — the Storage var-mode fix the dispatch must apply.
        var_mode: i32,
        /// The last `der_set_pf_wp_nominal` value (WATTPF; PVSystem only).
        pf_wp_nominal: f64,
        // --- volt-watt fields (Calc_PBase / Check_Plimits) ---
        pmpp: f64,       // FDCkWRated
        pu_pmpp: f64,    // FpctDCkWRated
        eff_factor: f64, // FEffFactor
        panel_kw: f64,   // FDCkW
        /// The last `der_set_kw_requested` value (ideal readback for `der_present_kw`).
        requested_kw: f64,
        // --- Storage volt-watt state (WPG.10; ignored when `!is_storage`) ---
        storage_state: i32,       // TStorageObj.StorageState
        vw_state_requested: bool, // TStorageObj.FVWStateRequested
        storage_dckw: f64,        // TStorageObj.DCkW (Calc_PBase %Available base)
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
                var_mode: 0, // VARMODE_PF
                pf_wp_nominal: 1.0,
                pmpp: 600.0,
                pu_pmpp: 1.0,
                eff_factor: 1.0,
                panel_kw: present_kw,
                requested_kw: present_kw,
                // A discharging Storage by default (the common VW test scenario);
                // ignored unless `is_storage` is set on the mock DER.
                storage_state: crate::elements::pc::storage::STORE_DISCHARGING,
                vw_state_requested: false,
                storage_dckw: 0.0,
            }
        }
    }

    struct MockEnv {
        ders: Vec<MockDer>,
        pushes: Vec<i32>,
        errors: Vec<String>,
        control_iter: i32,
        /// Per-monitored-bus complex node voltages for the explicit-`MonBus` path:
        /// `mon_bus_v[j][node-1]` is the voltage at monitored bus `j`, node `node`.
        mon_bus_v: Vec<Vec<num_complex::Complex64>>,
    }
    impl MockEnv {
        fn new(ders: Vec<MockDer>) -> Self {
            Self {
                ders,
                pushes: Vec::new(),
                errors: Vec::new(),
                control_iter: 1,
                mon_bus_v: Vec::new(),
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
                // Pascal UpdateDERParameters: FDCkW := 0.0 for Storage (l.1749, "not
                // using it") — so its WATTPF/WATTVAR curves read at panel pu = 0.
                dckw: if d.is_storage { 0.0 } else { d.panel_kw },
                dckw_rated: d.pmpp,
                pct_dckw_rated: d.pu_pmpp,
                eff_factor: d.eff_factor,
                storage_state: d.storage_state,
                vw_state_requested: d.vw_state_requested,
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
        fn mon_bus_node_v(&self, j: usize, node: i32) -> num_complex::Complex64 {
            self.mon_bus_v
                .get(j)
                .filter(|_| node >= 1)
                .and_then(|v| v.get((node - 1) as usize))
                .copied()
                .unwrap_or(num_complex::Complex64::ZERO)
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
        fn der_set_avr_mode(&mut self, _r: ElemRef, _value: bool) {}
        fn der_set_var_mode(&mut self, r: ElemRef, mode: i32) {
            self.ders[Self::idx(r)].var_mode = mode;
        }
        fn der_requested_kvar(&self, r: ElemRef) -> f64 {
            self.ders[Self::idx(r)].requested_kvar
        }
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
        fn der_storage_dckw(&mut self, r: ElemRef) -> f64 {
            self.ders[Self::idx(r)].storage_dckw
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
        fn set_loads_need_updating(&mut self) {}
        fn der_gfm_mode(&self, _r: ElemRef) -> bool {
            false
        }
        fn der_storage_state(&self, _r: ElemRef) -> i32 {
            0
        }
        fn der_ilimit(&self, _r: ElemRef) -> f64 {
            -1.0
        }
        fn der_reset_ibr(&self, _r: ElemRef) -> bool {
            false
        }
        fn der_check_amps_limit(&mut self, _r: ElemRef) -> bool {
            false
        }
        fn der_check_ol_inverter(&mut self, _r: ElemRef) -> bool {
            false
        }
        fn der_set_gfm_mode(&mut self, _r: ElemRef, _value: bool) {}
        fn der_set_reset_ibr(&mut self, _r: ElemRef, _value: bool) {}
        fn der_set_storage_state_off(&mut self, _r: ElemRef) {}
        fn is_dynamic_model(&self) -> bool {
            false
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
    fn exponential_control_model_runs_pi_controller() {
        // WPG.9: the Exponential ControlModel (`ControlModel=1`) runs the `TPICtrl`
        // PI controller in `CalcVoltVar_vars` — `Sample` must NOT reject it, and it
        // must NOT freeze the var output at the "stay put" level. Same V=1.05 absorb
        // scenario as the Linear test: QDesireVVpu=-0.625, QHeadRoom(VARMAX)=600, so
        // the PI setpoint DeltaQ = -0.625*600 = -375 (the *full* product — Exponential
        // does not subtract QOldVV). `kDen`/`kNum` are recomputed from |FdeltaQ_factor|
        // = 0.2 each call (InvControl.pas l.2706-2707). Both filter taps start at 0, so
        // the first `SolvePI` output (den[1] = num[0]*kNum + den[0]*kDen) is exactly 0.
        let mut ic = voltvar_ic();
        ic.set_i32(prop::CONTROL_MODEL, 1); // Exponential
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap(); // ported — must not error
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        let k_den = (-0.2_f64).exp();
        assert!(
            (cv.pi_ctrl.k_den - k_den).abs() < 1e-12,
            "kDen = {}",
            cv.pi_ctrl.k_den
        );
        assert!(
            (cv.pi_ctrl.k_num - (1.0 - k_den)).abs() < 1e-12,
            "kNum = {}",
            cv.pi_ctrl.k_num
        );
        // First PI step over the zeroed filter → exactly 0.0 (not the -1.0 stay-put
        // value the old unreachable branch would have produced).
        assert_eq!(
            cv.q_desired_vv, 0.0,
            "QDesiredVV first PI step = {}",
            cv.q_desired_vv
        );
        assert_eq!(env.ders[0].requested_kvar, 0.0);
    }

    #[test]
    fn pi_ctrl_solve_pi_two_step_sequence() {
        // Direct pin of Pascal `TPICtrl.SolvePI` (mathutil.pas l.81) as InvControl
        // drives it: Kp=1, kDen=exp(-|deltaQ_factor|), kNum=1-kDen recomputed each
        // call. Feeding a constant setpoint s: step 1 → 0 (zeroed taps), step 2 →
        // s*kNum, step 3 → s*kNum + s*kNum*kDen (the rising response).
        let mut pi = super::PICtrl {
            kp: 1.0,
            ..super::PICtrl::default()
        };
        let k_den = (-0.4_f64).exp();
        let k_num = 1.0 - k_den;
        let s = -375.0;
        pi.k_den = k_den;
        pi.k_num = k_num;
        let o1 = pi.solve_pi(s);
        pi.k_den = k_den;
        pi.k_num = k_num;
        let o2 = pi.solve_pi(s);
        pi.k_den = k_den;
        pi.k_num = k_num;
        let o3 = pi.solve_pi(s);
        assert_eq!(o1, 0.0, "step 1 = {o1}");
        assert!((o2 - s * k_num).abs() < 1e-9, "step 2 = {o2}");
        assert!(
            (o3 - (s * k_num + s * k_num * k_den)).abs() < 1e-9,
            "step 3 = {o3}"
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
    fn voltwatt_storage_dispatches_kw() {
        // WPG.10: a discharging Storage in VOLTWATT now dispatches (no NOT_PORTED).
        // Discharging + not VWStateRequested reads the main `voltwatt_curve`, and
        // yaxis=%Pmpp gives PBase = FDCkWRated = 600 — so the InvControl-side math
        // equals the PVSystem case: V=1.05 → curve y=0.625, PLimitEndpu=0.625,
        // CalcVoltWatt_watts (iter 1, requesting region) → PLimitVW=498.75. The
        // set-point reaches the DER via `kWRequested`.
        let mut ic = voltwatt_ic();
        let mut der = MockDer::new("pv", 1.05, 600.0);
        der.is_storage = true; // discharging by default
        let mut env = MockEnv::new(vec![der]);
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
            (cv.p_limit_vw - 498.75).abs() < 1e-9,
            "PLimitVW = {} (expected 498.75)",
            cv.p_limit_vw
        );
        assert!((env.ders[0].requested_kw - 498.75).abs() < 1e-9);
    }

    #[test]
    fn voltwatt_storage_charging_selects_ch_curve() {
        // WPG.10: the Storage-specific `CalcPVWcurve_limitpu` branch — a CHARGING
        // Storage with a `voltwattCH_curve` (and no VWStateRequested flip) reads the
        // CH curve, NOT the discharge `voltwatt_curve`. Distinct flat curves make the
        // pick observable: CH y=0.5 everywhere vs the discharge curve's 0.625 at 1.05.
        let mut ic = voltwatt_ic();
        ic.voltwattch_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vwch",
            &[0.5, 1.5],
            &[0.5, 0.5],
        ));
        let mut der = MockDer::new("pv", 1.05, -300.0); // charging (kW < 0)
        der.is_storage = true;
        der.storage_state = crate::elements::pc::storage::STORE_CHARGING;
        let mut env = MockEnv::new(vec![der]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.p_limit_vw_pu - 0.5).abs() < 1e-9,
            "PLimitVWpu = {} (expected the CH curve's 0.5, not 0.625)",
            cv.p_limit_vw_pu
        );
    }

    #[test]
    fn voltwatt_storage_yaxis_available_reads_live_dckw() {
        // WPG.10 coverage: VoltWattYAxis=0 (%Available) on a Storage takes the
        // `Calc_PBase` branch that reads the LIVE `TStorageObj.DCkW` via
        // `der_storage_dckw` — Pascal `Calc_PBase` sets FDCkW:=0 for a Storage and
        // reads the DCkW property instead, so PBase = DCkW * EffFactor. The
        // storage decks all use the default yaxis=1 (%Pmpp), so this path is
        // otherwise uncovered. Distinct DCkW (800) vs FDCkWRated (600) makes the
        // branch observable: yaxis=0 gives 720, yaxis=1 would give 600.
        let mut ic = voltwatt_ic();
        ic.set_i32(prop::VOLTWATT_YAXIS, 0); // %Available (PAVAILABLEPU)
        let mut der = MockDer::new("pv", 1.05, 600.0);
        der.is_storage = true; // discharging by default
        der.storage_dckw = 800.0; // live TStorageObj.DCkW (!= FDCkWRated)
        der.eff_factor = 0.9; // FEffFactor
        der.pmpp = 600.0; // FDCkWRated — the yaxis=1 base, for contrast
        let mut env = MockEnv::new(vec![der]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.f_eff_factor - 0.9).abs() < 1e-12,
            "EffFactor = {}",
            cv.f_eff_factor
        );
        // PBase = live DCkW * EffFactor = 800 * 0.9 = 720 (NOT FDCkWRated = 600).
        assert!(
            (cv.p_base - 720.0).abs() < 1e-9,
            "PBase = {} (expected live DCkW*EffFactor = 720, not FDCkWRated = 600)",
            cv.p_base
        );
    }

    #[test]
    fn voltwatt_storage_vw_state_requested_swaps_curves() {
        // WPG.10 coverage: `CalcPVWcurve_limitpu`'s FVWStateRequested swap (Pascal
        // InvControl.pas l.2960-2961 / 2969-2970). Once the VW function has
        // requested a state flip, the curve selection SWAPS vs the normal pick: a
        // DISCHARGING storage reads the CH curve, a CHARGING storage reads the
        // discharge curve. Distinct flat curves make the pick observable: the
        // discharge `vw` = 0.3 everywhere, the `vwch` = 0.7 everywhere.
        let discharge_y = 0.3;
        let charge_y = 0.7;
        let vw = || {
            crate::elements::general::xy_curve::XyCurveObj::from_points(
                "vw",
                &[0.5, 1.5],
                &[discharge_y, discharge_y],
            )
        };
        let vwch = || {
            crate::elements::general::xy_curve::XyCurveObj::from_points(
                "vwch",
                &[0.5, 1.5],
                &[charge_y, charge_y],
            )
        };

        // Discharging + VWStateRequested → reads the CH curve (0.7), NOT vw (0.3).
        let mut ic = voltwatt_ic();
        ic.voltwatt_curve = Some(vw());
        ic.voltwattch_curve = Some(vwch());
        let mut der = MockDer::new("pv", 1.05, 600.0);
        der.is_storage = true;
        der.storage_state = crate::elements::pc::storage::STORE_DISCHARGING;
        der.vw_state_requested = true;
        let mut env = MockEnv::new(vec![der]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        assert!(
            (ic.ctrl_vars[0].p_limit_vw_pu - charge_y).abs() < 1e-9,
            "discharging+VWStateRequested PLimitVWpu = {} (expected the swapped CH curve {charge_y})",
            ic.ctrl_vars[0].p_limit_vw_pu
        );

        // Charging + VWStateRequested → reads the discharge curve (0.3), NOT CH (0.7).
        let mut ic = voltwatt_ic();
        ic.voltwatt_curve = Some(vw());
        ic.voltwattch_curve = Some(vwch());
        let mut der = MockDer::new("pv", 1.05, -300.0); // charging (kW < 0)
        der.is_storage = true;
        der.storage_state = crate::elements::pc::storage::STORE_CHARGING;
        der.vw_state_requested = true;
        let mut env = MockEnv::new(vec![der]);
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env);
        assert!(
            (ic.ctrl_vars[0].p_limit_vw_pu - discharge_y).abs() < 1e-9,
            "charging+VWStateRequested PLimitVWpu = {} (expected the swapped discharge curve {discharge_y})",
            ic.ctrl_vars[0].p_limit_vw_pu
        );
    }

    #[test]
    fn vv_vw_storage_dispatches_both() {
        // WPG.10: a discharging Storage in the VV_VW combi dispatches BOTH the
        // volt-watt kW limit and the volt-var kvar in one DoPendingAction (no
        // NOT_PORTED). Same curves/scenario as the PVSystem VV_VW test, so the
        // set-points match: PLimitVW=498.75 (VW) and QDesiredVV=-75.8 (VV).
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
        let mut der = MockDer::new("pv", 1.05, 600.0);
        der.is_storage = true; // discharging by default
        let mut env = MockEnv::new(vec![der]);
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
        assert!((env.ders[0].requested_kw - 498.75).abs() < 1e-9);
        assert!((env.ders[0].requested_kvar - (-75.8)).abs() < 1e-9);
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
    // *daily* `der_controls/invcontrol_drc` golden, not a snapshot).

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
    fn drc_exponential_control_model_runs_pi_controller() {
        // WPG.9: ControlModel=1 (Exponential) drives CalcDRC_vars's else branch (the
        // TPICtrl PI controller over the *full* DeltaQ, InvControl.pas l.2804-2809)
        // instead of the Linear QOldDRC-relative formula. Same absorb scenario as
        // `drc_absorbs_on_rising_voltage` (V=1.05pu, window seeded at 1.0pu ->
        // deltaV=+0.05 -> QDesireDRCpu=-2.5, clamped by Check_Qlimits to
        // QDesireEndpu=-1.0), so the PI setpoint is the *unclamped-by-QOldDRC*
        // product DeltaQ = -1.0*QHeadRoomNeg(600) = -600 every call (no dependence on
        // QOldDRC), unlike Linear's -599. kDen/kNum are recomputed from
        // |FdeltaQ_factor|=0.2 each call. Both filter taps start at 0 so the first
        // `SolvePI` output is exactly 0 (a wrong q_desired_* field would coincide
        // with this default); a second Sample+DoPendingAction call (repeat trigger:
        // control_iter stays 1) exercises the delayed PI tap and pins the non-zero
        // -600*kNum response — this is what catches a wrong DeltaQ or a dropped
        // per-call kDen/kNum recompute.
        let mut ic = drc_ic();
        ic.set_i32(prop::CONTROL_MODEL, 1); // Exponential
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.ctrl_vars[0]
            .f_drc_roll_avg_window
            .add(7200.0, 3600.0, 2.0);
        ic.do_pending_action(&mut env); // first PI step
        let k_den = (-0.2_f64).exp();
        let k_num = 1.0 - k_den;
        {
            let cv = &ic.ctrl_vars[0];
            assert!(
                (cv.pi_ctrl.k_den - k_den).abs() < 1e-12,
                "kDen = {}",
                cv.pi_ctrl.k_den
            );
            assert!(
                (cv.pi_ctrl.k_num - k_num).abs() < 1e-12,
                "kNum = {}",
                cv.pi_ctrl.k_num
            );
            assert_eq!(
                cv.q_desired_drc, 0.0,
                "QDesiredDRC first PI step = {}",
                cv.q_desired_drc
            );
            assert!(
                (cv.q_desire_drcpu - (-2.5)).abs() < 1e-9,
                "QDesireDRCpu = {}",
                cv.q_desire_drcpu
            );
        }
        ic.sample(&mut env).unwrap(); // control_iter==1 -> re-triggers
        ic.do_pending_action(&mut env); // second PI step
        let cv = &ic.ctrl_vars[0];
        let expected = -600.0 * k_num;
        assert!(
            (cv.q_desired_drc - expected).abs() < 1e-6,
            "QDesiredDRC second PI step = {} expected {expected}",
            cv.q_desired_drc
        );
        assert!((env.ders[0].requested_kvar - expected).abs() < 1e-6);
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
    fn vv_drc_exponential_control_model_runs_pi_controller() {
        // WPG.9: ControlModel=1 (Exponential) drives CalcVVDRC_vars's else branch
        // (the TPICtrl PI controller over the *full* DeltaQ, InvControl.pas
        // l.2840-2845). Same combi scenario as `vv_drc_sums_curve_and_drc_q`
        // (V=1.01pu, window seeded at 1.0pu): QDesireVVpu=-0.125,
        // QDesireDRCpu=-0.5, q_sum=-0.625 (unclamped) -> QDesireEndpu=-0.625, so the
        // PI setpoint DeltaQ = -0.625*QHeadRoomNeg(600) = -375 every call (the full
        // product, not decremented by QOldVVDRC). kDen/kNum recomputed from
        // |FdeltaQ_factor|=0.2 each call; first PI step (zeroed taps) = 0. A second
        // Sample+DoPendingAction call (control_iter stays 1 -> repeat trigger) pins
        // the delayed-tap response -375*kNum, catching a wrong DeltaQ / wrong
        // q_desired_* field / a dropped per-call kDen/kNum recompute.
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::COMBI_MODE, super::super::VV_DRC);
        ic.set_i32(prop::CONTROL_MODEL, 1); // Exponential
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
        ic.do_pending_action(&mut env); // first PI step
        let k_den = (-0.2_f64).exp();
        let k_num = 1.0 - k_den;
        {
            let cv = &ic.ctrl_vars[0];
            assert!(
                (cv.pi_ctrl.k_den - k_den).abs() < 1e-12,
                "kDen = {}",
                cv.pi_ctrl.k_den
            );
            assert!(
                (cv.pi_ctrl.k_num - k_num).abs() < 1e-12,
                "kNum = {}",
                cv.pi_ctrl.k_num
            );
            assert_eq!(
                cv.q_desired_vvdrc, 0.0,
                "QDesiredVVDRC first PI step = {}",
                cv.q_desired_vvdrc
            );
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
        }
        ic.sample(&mut env).unwrap(); // control_iter==1 -> re-triggers
        ic.do_pending_action(&mut env); // second PI step
        let cv = &ic.ctrl_vars[0];
        let expected = -375.0 * k_num;
        assert!(
            (cv.q_desired_vvdrc - expected).abs() < 1e-6,
            "QDesiredVVDRC second PI step = {} expected {expected}",
            cv.q_desired_vvdrc
        );
        assert!((env.ders[0].requested_kvar - expected).abs() < 1e-6);
    }

    #[test]
    fn gfm_mode_is_ported_and_inert_when_der_not_grid_forming() {
        // GFM (mode ordinal 7) is ported (WPG.13): Sample no longer rejects it.
        // The GFM arm is a no-op for a DER that is not itself in grid-forming mode
        // (`der_gfm_mode == false` in the mock), so Sample succeeds and queues
        // nothing. (The live amps-limiter path is gate-verified by the
        // `gfm_invcontrol.dss` corpus deck, which drives `CheckAmpsLimit`.)
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, 7); // GFM
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env)
            .expect("GFM sample is ported (no NOT_PORTED error)");
        assert_eq!(
            ic.ctrl_vars[0].f_pending_change,
            super::super::CHANGE_NONE,
            "no control action queued for a non-grid-forming DER"
        );
    }

    /// An AVR control (Vsetpoint=0.98, VARMAX) over the named `pv` fleet. No curve.
    fn avr_ic() -> InvControl {
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::AVR);
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        ic.set_f64(prop::VSETPOINT, 0.98);
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        ic
    }

    #[test]
    fn avr_iter1_seeds_half_headroom() {
        // Control iteration 1: AVR seeds the prior-voltage baselines and pushes
        // QHeadRoom/2 kvar (Pascal l.1063-1073). VARMAX → QHeadRoom = kvarLimit = 600,
        // so the requested kvar is 300; FAvgpAVRVpuPrior latches the present pu (1.009).
        let mut ic = avr_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.009, 200.0)]);
        env.control_iter = 1;
        ic.sample(&mut env).unwrap();
        assert_eq!(env.pushes, vec![super::super::CHANGEVARLEVEL]);
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.q_headroom - 600.0).abs() < 1e-9,
            "QHeadRoom = {}",
            cv.q_headroom
        );
        assert!(
            (cv.f_avgp_avr_vpu_prior - 1.009).abs() < 1e-9,
            "FAvgpAVRVpuPrior = {}",
            cv.f_avgp_avr_vpu_prior
        );
        assert!(
            (env.ders[0].requested_kvar - 300.0).abs() < 1e-9,
            "requested kvar = {}",
            env.ders[0].requested_kvar
        );
    }

    #[test]
    fn avr_iter2_estimates_dqdv() {
        // Iteration 1 seeds kvar=300 at v=1.009; the solve drops the voltage to 1.0;
        // iteration 2 estimates DQDV = |Presentkvar / QHeadRoom / (Vpresent − Vprior)|
        // = |300 / 600 / (1.0 − 1.009)| = 0.5/0.009 ≈ 55.5556 (Pascal l.1075-1082).
        let mut ic = avr_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.009, 200.0)]);
        env.control_iter = 1;
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // iter 1 → kvar 300
        // The re-solve drops the bus voltage (mock the new terminal magnitude).
        env.ders[0].vmag = 1.0 * env.ders[0].vbase;
        env.control_iter = 2;
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // iter 2 → DQDV
        let cv = &ic.ctrl_vars[0];
        let expected = (300.0_f64 / 600.0 / (1.0 - 1.009)).abs();
        assert!(
            (cv.dqdv - expected).abs() < 1e-6,
            "DQDV = {} expected {expected}",
            cv.dqdv
        );
    }

    #[test]
    fn avr_iter3_regulator_step_clamped_by_dqmax() {
        // Iteration 3 runs the regulator: DQ = FdeltaQFactor·DQDV·(Vsetpoint − v) is
        // clamped to ±DQmax (= 0.1·kvarLimit/QHeadRoomNeg = 0.1), so QDesireAVRpu lands
        // at −0.1; CalcAVR_vars then sets QDesiredAVR = QOldAVR(0) + 0.2·(−0.1·600) = −12.
        let mut ic = avr_ic();
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.009, 200.0)]);
        env.control_iter = 1;
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // iter 1
        env.ders[0].vmag = 1.0 * env.ders[0].vbase;
        env.control_iter = 2;
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // iter 2 → DQDV ≈ 55.5556
        env.control_iter = 3;
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // iter 3 → regulator
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.q_desire_avrpu - (-0.1)).abs() < 1e-9,
            "QDesireAVRpu = {} (expected the −DQmax clamp −0.1)",
            cv.q_desire_avrpu
        );
        assert!(
            (cv.q_desired_avr - (-12.0)).abs() < 1e-9,
            "QDesiredAVR = {} (expected −12)",
            cv.q_desired_avr
        );
        assert!(
            (env.ders[0].requested_kvar - (-12.0)).abs() < 1e-9,
            "requested kvar = {}",
            env.ders[0].requested_kvar
        );
        // The setpoint-limited voltage is the configured setpoint (the request is far
        // from the kvar limit, so the |QEnd − QLimited| < 0.05 branch is not taken).
        assert!(
            (cv.f_v_setpoint_limited - 0.98).abs() < 1e-9,
            "Fv_setpointLimited = {}",
            cv.f_v_setpoint_limited
        );
    }

    #[test]
    fn avr_storage_dispatches_in_kvar_mode() {
        // A Storage in AVR regulates like a PVSystem (the converged kvar is oracle-
        // pinned by der_controls/invcontrol_avr_storage). Here the mock pins the var-mode
        // fix: iter-1 sets the DER `Varmode := VARMODEKVAR` (so `set_nominal` applies
        // the request, not its VARMODE_PF default) and pushes QHeadRoom/2 = 300 kvar.
        let mut ic = avr_ic();
        let mut der = MockDer::new("pv", 1.009, 200.0);
        der.is_storage = true;
        let mut env = MockEnv::new(vec![der]);
        env.control_iter = 1;
        ic.sample(&mut env).unwrap(); // no error — Storage AVR is supported
        ic.do_pending_action(&mut env);
        assert_eq!(
            env.ders[0].var_mode, 1,
            "Storage Varmode must be VARMODE_KVAR"
        );
        assert!(
            (env.ders[0].requested_kvar - 300.0).abs() < 1e-9,
            "Storage AVR iter-1 kvar = {}",
            env.ders[0].requested_kvar
        );
    }

    #[test]
    fn avr_exponential_control_model_runs_pi_controller() {
        // WPG.9: ControlModel=1 (Exponential) drives CalcAVR_vars's else branch (the
        // TPICtrl PI controller over the *full* DeltaQ, InvControl.pas l.2746-2751),
        // not the Linear literal-0.2-over-QOldAVR formula. Same iter1->iter2->iter3
        // path as `avr_iter3_regulator_step_clamped_by_dqmax` (iter1 seeds
        // QHeadRoom/2=300 kvar at v=1.009; the solve drops v to 1.0; iter2 estimates
        // DQDV≈55.5556); iter3's regulator computes QDesireAVRpu=-0.1 (the DQmax
        // clamp), so the PI setpoint DeltaQ = -0.1*QHeadRoomNeg(600) = -60. kDen/kNum
        // recomputed from |FdeltaQ_factor|=0.2 each call; first PI step (zeroed taps)
        // = 0. A second Sample+DoPendingAction call at iter3 re-triggers (the
        // f_v_setpoint_limited/Qoutput mismatch keeps AVR firing) and exercises the
        // delayed PI tap: the response to the *first* call's setpoint, -60*kNum —
        // this is what catches a wrong DeltaQ / wrong q_desired_avr field / a dropped
        // per-call kDen/kNum recompute.
        let mut ic = avr_ic();
        ic.set_i32(prop::CONTROL_MODEL, 1); // Exponential
        ic.set_f64(prop::DELTA_Q_FACTOR, 0.2);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.009, 200.0)]);
        env.control_iter = 1;
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // iter 1
        env.ders[0].vmag = 1.0 * env.ders[0].vbase;
        env.control_iter = 2;
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // iter 2 → DQDV ≈ 55.5556
        env.control_iter = 3;
        ic.sample(&mut env).unwrap();
        ic.do_pending_action(&mut env); // iter 3, call #1 → first PI step
        let k_den = (-0.2_f64).exp();
        let k_num = 1.0 - k_den;
        {
            let cv = &ic.ctrl_vars[0];
            assert!(
                (cv.pi_ctrl.k_den - k_den).abs() < 1e-12,
                "kDen = {}",
                cv.pi_ctrl.k_den
            );
            assert!(
                (cv.pi_ctrl.k_num - k_num).abs() < 1e-12,
                "kNum = {}",
                cv.pi_ctrl.k_num
            );
            assert_eq!(
                cv.q_desired_avr, 0.0,
                "QDesiredAVR first PI step = {}",
                cv.q_desired_avr
            );
            assert!(
                (cv.q_desire_avrpu - (-0.1)).abs() < 1e-9,
                "QDesireAVRpu = {} (expected the -DQmax clamp -0.1)",
                cv.q_desire_avrpu
            );
        }
        ic.sample(&mut env).unwrap(); // iter 3, call #2 → re-triggers
        ic.do_pending_action(&mut env); // second PI step
        let cv = &ic.ctrl_vars[0];
        let expected = -60.0 * k_num;
        assert!(
            (cv.q_desired_avr - expected).abs() < 1e-6,
            "QDesiredAVR second PI step = {} expected {expected}",
            cv.q_desired_avr
        );
        assert!((env.ders[0].requested_kvar - expected).abs() < 1e-6);
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

    #[test]
    fn wattpf_storage_dispatches_in_kvar_mode() {
        // A Storage in WATTPF regulates (the converged magnitude is oracle-pinned by
        // der_controls/invcontrol_wattpf_storage). FDCkW=0 for Storage so the wattpf curve is
        // read at panel-pu 0; with a non-unity pf there (-0.95) and WattPriority the
        // watt term `p = kW_out_desired` (= present 400) is non-zero, so the Storage
        // absorbs Q = -400·tan(acos(0.95)) = -131.47 kvar. Pins the var-mode fix AND a
        // non-degenerate kvar (not just var_mode==1).
        let mut ic = wattpf_ic();
        ic.wattpf_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "wpf",
            &[0.0, 0.5, 1.0],
            &[-0.95, -0.95, -0.9],
        ));
        let mut der = MockDer::new("pv", 1.0, 400.0);
        der.is_storage = true;
        der.p_priority = true; // WattPriority → p = kW_out_desired (non-zero)
        let mut env = MockEnv::new(vec![der]);
        ic.sample(&mut env).unwrap(); // no error — Storage WATTPF is supported
        ic.do_pending_action(&mut env);
        assert_eq!(
            env.ders[0].var_mode, 1,
            "Storage Varmode must be VARMODE_KVAR"
        );
        let expected = -400.0 * (1.0 / 0.95_f64.powi(2) - 1.0).sqrt();
        assert!(
            (env.ders[0].requested_kvar - expected).abs() < 1e-9,
            "Storage WATTPF kvar = {} (expected {expected})",
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
        // The PVSystem-only kW push: PLimitEndpu·min(kVArating, DCkWrated). Here
        // |Q|=240 is below the kvar limit so FWVOperation != 0.2 → PLimitEndpu = 1.0
        // and (P,Q)=(600,-240) stays inside the 1000-kVA circle (no quadratic), so
        // kW = 1.0·min(1000, 600) = 600.
        assert!(
            (env.ders[0].requested_kw - 600.0).abs() < 1e-9,
            "wattvar kW {} != 600",
            env.ders[0].requested_kw
        );
    }

    #[test]
    fn wattvar_storage_dispatches_in_kvar_mode() {
        // A Storage in WATTVAR regulates: FDCkW=0 means the wattvar curve is read at
        // panel-pu 0, so a curve with a non-zero y(0) drives a real kvar request. Here
        // y(0)=-0.3 → QDesireWVpu=-0.3 → QDesiredWV = -0.3·QHeadRoom(=600) = -180. The
        // fix under test: `Varmode := VARMODE_KVAR` so the request is applied (a Storage
        // would otherwise keep VARMODE_PF and discard it). (The converged value is
        // oracle-pinned by der_controls/invcontrol_wattvar_storage.)
        let mut ic = wattvar_ic();
        // Override the curve so y(0) = -0.3 (the curve point Storage actually reads).
        ic.wattvar_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "wv",
            &[0.0, 0.5, 1.0],
            &[-0.3, -0.3, -0.4],
        ));
        // present_kw 400 ≠ the would-be kW-push value (PLimitEndpu·min(kVA,DCkWrated)
        // = 600), so the "no kW push for Storage" gate is observable below.
        let mut der = MockDer::new("pv", 1.0, 400.0);
        der.is_storage = true;
        let mut env = MockEnv::new(vec![der]);
        ic.sample(&mut env).unwrap(); // no error — Storage WATTVAR is supported
        ic.do_pending_action(&mut env);
        assert_eq!(
            env.ders[0].var_mode, 1,
            "Storage Varmode must be VARMODE_KVAR"
        );
        assert!(
            (env.ders[0].requested_kvar - (-180.0)).abs() < 1e-9,
            "Storage WATTVAR kvar = {} (expected -180)",
            env.ders[0].requested_kvar
        );
        // The WATTVAR kW push is PVSystem-only; a Storage's kW request is untouched
        // (Pascal l.1206-1212). A dropped `if is_pv` gate would overwrite it with 600.
        assert!(
            (env.ders[0].requested_kw - 400.0).abs() < 1e-9,
            "Storage WATTVAR must not push kW (requested_kw = {})",
            env.ders[0].requested_kw
        );
    }

    /// A VOLTVAR control monitoring an explicit `MonBus` (3 single-node phases),
    /// `monVoltageCalc=AVG`. The DER's curve x-ref is `rated`.
    fn monbus_voltvar(buses: Vec<String>, vbase: Vec<f64>) -> InvControl {
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::VOLTVAR);
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        ic.vvc_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vv",
            &[0.5, 0.92, 1.0, 1.08, 1.5],
            &[1.0, 1.0, 0.0, -1.0, -1.0],
        ));
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        ic.set_string_list(prop::MON_BUS, buses);
        ic.side_effects(prop::MON_BUS, 0);
        ic.set_f64_array(prop::MON_BUSES_VBASE, vbase);
        ic
    }

    #[test]
    fn monbus_single_node_avg_overrides_self_voltage() {
        // MonBus monitors bus m at 1.02 pu (3 single-node phases); the DER's own
        // terminal sits at 0.95 pu. With voltage_curvex_ref=rated, FPresentVpu must
        // follow the MonBus average (1.02), NOT the self-monitored 0.95 — a regression
        // that ignored MonBus (read the DER terminal) would land on 0.95.
        let mut ic = monbus_voltvar(
            vec!["m.1".into(), "m.2".into(), "m.3".into()],
            vec![7200.0, 7200.0, 7200.0], // L-N base = DER vbase → scale = 1.0
        );
        let mut env = MockEnv::new(vec![MockDer::new("pv", 0.95, 300.0)]);
        // m.1 reads node 1 of bus j=0, m.2 node 2 of j=1, m.3 node 3 of j=2 (each at
        // 1.02 pu → 7344 V); the mock indexes `mon_bus_v[j][node-1]`.
        let v = num_complex::Complex64::new(1.02 * 7200.0, 0.0);
        env.mon_bus_v = vec![vec![v], vec![v, v], vec![v, v, v]];
        ic.sample(&mut env).unwrap();
        assert!(
            (ic.ctrl_vars[0].f_present_vpu - 1.02).abs() < 1e-9,
            "FPresentVpu = {} (expected 1.02 from MonBus, not 0.95 self)",
            ic.ctrl_vars[0].f_present_vpu
        );
    }

    #[test]
    fn make_like_preserves_monbus_path() {
        // Pascal `RecalcElementData` l.925 keys `FUsingMonBuses` off the parsed
        // `FMonBuses` (which MakeLike l.788 copies), NOT `MonBusesNameList` (which
        // MakeLike does NOT copy). So a `like=`-derived MonBus control must still take
        // the MonBus path. (The derived control's MonBusesVbase is empty — the
        // upstream-pathological `like=`-with-MonBus case — so we assert the path
        // selection, not the numeric voltage.)
        let a = monbus_voltvar(vec!["m.1".into()], vec![7200.0]);
        let mut b = InvControl::new("b");
        b.make_like(&a);
        assert_eq!(
            b.mon_buses,
            vec!["m".to_string()],
            "MakeLike must copy FMonBuses"
        );
        assert_eq!(
            b.mon_buses_nodes,
            vec![vec![1]],
            "MakeLike must copy FMonBusesNodes"
        );
        b.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        b.side_effects(prop::DER_LIST, 0);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 0.95, 300.0)]);
        let _ = b.sample(&mut env);
        assert!(
            b.f_using_mon_buses,
            "a like=-derived MonBus control must take the MonBus path (keyed off FMonBuses)"
        );
    }

    #[test]
    fn monbus_line_to_line_takes_node_difference() {
        // A 2-node MonBus entry (`m.1.2`): the monitored voltage is the |node1 − node2|
        // difference, scaled by basekv·1000 / vbase. node1 = 8000, node2 = 2000 →
        // |diff| = 6000; vbase = 6000 → cBuffer = 6000·(7200/6000) = 7200 →
        // FPresentVpu = 7200 / 7200 = 1.0. Reading only node1 (a regression dropping the
        // subtraction) would give 8000·(7200/6000)/7200 = 1.333.
        let mut ic = monbus_voltvar(vec!["m.1.2".into()], vec![6000.0]);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 0.95, 300.0)]);
        env.mon_bus_v = vec![vec![
            num_complex::Complex64::new(8000.0, 0.0),
            num_complex::Complex64::new(2000.0, 0.0),
        ]];
        ic.sample(&mut env).unwrap();
        assert!(
            (ic.ctrl_vars[0].f_present_vpu - 1.0).abs() < 1e-9,
            "FPresentVpu = {} (expected 1.0 from the L-L difference, not 1.333)",
            ic.ctrl_vars[0].f_present_vpu
        );
    }

    #[test]
    fn monbus_max_min_reduce_folds_the_buffer() {
        // Three single-node monitored buses at 1.00 / 1.05 / 0.98 pu. monVoltageCalc=MAX
        // → 1.05, MIN → 0.98 (AVG would give 1.01 — so each fold is discriminated). The
        // MonBus reduce path (reduce_mon_phase MAX/MIN over the complex cBuffer); the 3
        // migrated corpus cases + the AVG mocks only cover AVGPHASES.
        let probe = |phase: i32| {
            let mut ic = monbus_voltvar(
                vec!["m.1".into(), "m.2".into(), "m.3".into()],
                vec![7200.0, 7200.0, 7200.0],
            );
            ic.mon_buses_phase = phase;
            let c = |pu: f64| num_complex::Complex64::new(pu * 7200.0, 0.0);
            let mut env = MockEnv::new(vec![MockDer::new("pv", 0.90, 300.0)]);
            // m.1 → bus j=0 node 1, m.2 → j=1 node 2, m.3 → j=2 node 3.
            env.mon_bus_v = vec![
                vec![c(1.00)],
                vec![c(0.0), c(1.05)],
                vec![c(0.0), c(0.0), c(0.98)],
            ];
            ic.sample(&mut env).unwrap();
            ic.ctrl_vars[0].f_present_vpu
        };
        assert!(
            (probe(super::super::MAXPHASE) - 1.05).abs() < 1e-9,
            "MAX reduce = {} (expected 1.05)",
            probe(super::super::MAXPHASE)
        );
        assert!(
            (probe(super::super::MINPHASE) - 0.98).abs() < 1e-9,
            "MIN reduce = {} (expected 0.98)",
            probe(super::super::MINPHASE)
        );
    }

    #[test]
    fn monbus_specific_phase_indexes_buffer_zero_based() {
        // A numeric monVoltageCalc (a specific phase) reads `Cabs(cBuffer[FMonBusesPhase])`,
        // and in the MonBus branch cBuffer is 0-based — so phase 2 reads cBuffer[2] = the
        // 3rd monitored bus (0.98 pu), NOT the 2nd (the verbatim Pascal indexing quirk).
        let mut ic = monbus_voltvar(
            vec!["m.1".into(), "m.2".into(), "m.3".into()],
            vec![7200.0, 7200.0, 7200.0],
        );
        ic.mon_buses_phase = 2;
        let c = |pu: f64| num_complex::Complex64::new(pu * 7200.0, 0.0);
        let mut env = MockEnv::new(vec![MockDer::new("pv", 0.90, 300.0)]);
        env.mon_bus_v = vec![
            vec![c(1.00)],
            vec![c(0.0), c(1.05)],
            vec![c(0.0), c(0.0), c(0.98)],
        ];
        ic.sample(&mut env).unwrap();
        assert!(
            (ic.ctrl_vars[0].f_present_vpu - 0.98).abs() < 1e-9,
            "specific-phase reduce = {} (expected cBuffer[2] = 0.98)",
            ic.ctrl_vars[0].f_present_vpu
        );
    }

    /// A VOLTVAR control with rate-of-change limiting (`mode`/`limit` set directly).
    fn roc_voltvar(mode: i32) -> InvControl {
        let mut ic = InvControl::new("ic1");
        ic.set_i32(prop::MODE, super::super::VOLTVAR);
        ic.set_i32(prop::REF_REACTIVE_POWER, super::super::REAC_POWER_VARMAX);
        ic.vvc_curve = Some(crate::elements::general::xy_curve::XyCurveObj::from_points(
            "vv",
            &[0.5, 0.92, 1.0, 1.08, 1.5],
            &[1.0, 1.0, 0.0, -1.0, -1.0],
        ));
        ic.rate_of_change_mode = mode;
        ic.set_string_list(prop::DER_LIST, vec!["PVSystem.pv".into()]);
        ic.side_effects(prop::DER_LIST, 0);
        ic
    }

    #[test]
    fn lpf_smooths_desired_q_against_prior_option() {
        // RateofChangeMode=LPF, LPFTau=2 s, mock dyna_h=1 s → α = exp(−1/2) = 0.606531.
        // At V=1.05 pu the curve gives QDesireVVpu = −0.625 (linear 1.0→1.08 maps 0→−1).
        // Seeding the prior option at −0.2: QDesireOptionpu = −0.625·(1−α) + −0.2·α.
        let mut ic = roc_voltvar(super::super::ROC_LPF);
        ic.lpf_tau = 2.0;
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.ctrl_vars[0].f_prior_q_desire_optionpu = -0.2;
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.q_desire_vvpu - (-0.625)).abs() < 1e-9,
            "QDesireVVpu = {}",
            cv.q_desire_vvpu
        );
        let alpha = (-0.5f64).exp();
        let expected = -0.625 * (1.0 - alpha) + (-0.2) * alpha;
        assert!(
            (cv.q_desire_optionpu - expected).abs() < 1e-9,
            "QDesireOptionpu = {} (expected {expected})",
            cv.q_desire_optionpu
        );
    }

    #[test]
    fn risefall_ramps_desired_q_against_prior_option() {
        // RateofChangeMode=RiseFall, RiseFallLimit=0.1, mock dyna_h=1 → per-step cap 0.1.
        // QDesireVVpu = −0.625 (V=1.05); prior option = −0.2. The change −0.425 exceeds
        // the downward cap (−0.425 < −0.1), so QDesireOptionpu ramps to prior − 0.1 = −0.3,
        // NOT the full −0.625 (a regression dropping the rate limit would land there).
        let mut ic = roc_voltvar(super::super::ROC_RISEFALL);
        ic.rise_fall_limit = 0.1;
        let mut env = MockEnv::new(vec![MockDer::new("pv", 1.05, 300.0)]);
        ic.sample(&mut env).unwrap();
        ic.ctrl_vars[0].f_prior_q_desire_optionpu = -0.2;
        ic.do_pending_action(&mut env);
        let cv = &ic.ctrl_vars[0];
        assert!(
            (cv.q_desire_optionpu - (-0.3)).abs() < 1e-9,
            "QDesireOptionpu = {} (expected −0.3, the rate-limited ramp)",
            cv.q_desire_optionpu
        );
    }
}

#[cfg(test)]
mod make_pos_seq_tests {
    use super::super::*;
    use crate::elements::pos_seq::{PosSeqCtx, PosSeqElemInfo};
    use crate::elements::traits::{CktElement, ElemRef};

    /// Pascal `TInvControlObj.MakePosSequence` (InvControl.pas:943): the empty
    /// DER-list config is a NIL-deref hazard (Access violation #303, probe `1`).
    /// The defined `FNphases := 3; Nconds := 3` still applies; the NIL-deref
    /// `Setbus` is safe-skipped (no bus change, no panic).
    #[test]
    fn empty_der_list_applies_3phase_and_safe_skips_setbus() {
        let mut ic = InvControl::new("ic1");
        let bus = ic.ccd.cd.get_bus(1).to_string();
        let plan = ic.make_pos_sequence(&PosSeqCtx::default()); // monitored None
        assert_eq!(ic.ccd.cd.nphases, 3);
        assert_eq!(ic.ccd.cd.nconds, 3);
        assert_eq!(ic.ccd.cd.get_bus(1), bus); // Setbus safe-skipped
        assert!(plan.run_base);
    }

    /// Populated path: monitored resolved to the 1st DER ⇒ adopt its Firstbus /
    /// phase count (overriding the 3/3 default).
    #[test]
    fn populated_der_adopts_first_der_bus_and_phases() {
        let mut ic = InvControl::new("ic1");
        ic.ccd.monitored_element = Some(ElemRef { cls: 3, idx: 7 });
        let ctx = PosSeqCtx {
            monitored: Some(PosSeqElemInfo {
                nphases: 1,
                nconds: 1,
                bus_names: vec!["derbus".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        ic.make_pos_sequence(&ctx);
        assert_eq!(ic.ccd.cd.nphases, 1);
        assert_eq!(ic.ccd.cd.nconds, 1);
        assert_eq!(ic.ccd.cd.get_bus(1), "derbus"); // MonitoredElement.Firstbus
        assert_eq!(ic.monitored_element_ref(), Some(ElemRef { cls: 3, idx: 7 }));
    }
}
