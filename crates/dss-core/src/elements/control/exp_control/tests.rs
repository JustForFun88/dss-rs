//! Unit tests for ExpControl (WP7.5 step 3). `Create` defaults and the
//! PVSystemList ↔ DERList sync are spec-pinned (Pascal is the spec); the full
//! property round-trip is oracle-pinned by `tests/golden/props/expcontrol.json`
//! (props_roundtrip.rs). The dispatch math (the adaptive-`Vreg` volt-var
//! curve→clamp→delta step, the `Vreg` slew, the not-injecting / `PreferQ`
//! branches) is pinned per-call through a mock env, independent of the PVSystem
//! injection model; the end-to-end convergence is oracle-pinned by the targeted
//! `phase7/expcontrol_*` golden + the live corpus.

use super::*;
use crate::obj::base::DssObject;

#[test]
fn create_defaults_match_pascal() {
    let ec = ExpControl::new("e1");
    assert_eq!(ec.f_vreg_init, 1.0);
    assert_eq!(ec.q_v_slope, 50.0);
    assert_eq!(ec.vreg_tau, 1200.0);
    assert_eq!(ec.f_qbias, 0.0);
    assert_eq!(ec.vreg_min, 0.95);
    assert_eq!(ec.vreg_max, 1.05);
    assert_eq!(ec.qmax_lead, 0.44);
    assert_eq!(ec.qmax_lag, 0.44);
    assert_eq!(ec.f_delta_q_factor, 0.7);
    assert!(!ec.f_prefer_q);
    assert_eq!(ec.tresponse, 0.0);
    assert_eq!(ec.f_open_tau, 0.0);
    // "no user adjustment" hard-coded tolerances.
    assert_eq!(ec.f_voltage_change_tolerance, 0.0001);
    assert_eq!(ec.f_var_change_tolerance, 0.0001);
    assert!(!ec.ccd.show_event_log);
    // Control elements are 3-phase/3-conductor, single terminal.
    assert_eq!(ec.ccd.cd.nphases, 3);
    assert_eq!(ec.ccd.cd.nconds, 3);
    assert_eq!(ec.ccd.element_terminal, 1);
}

#[test]
fn recalc_derives_open_tau() {
    // FOpenTau := Tresponse / 2.3026 (the truncated ln(10); derived in
    // RecalcElementData). Tresponse=23.026 → FOpenTau = 10 (the literal divisor
    // lives behind `LN10_TRUNCATED` so it isn't repeated here).
    let mut ec = ExpControl::new("e1");
    ec.set_f64(prop::TRESPONSE, 23.026);
    ec.recalc();
    assert!((ec.f_open_tau - 10.0).abs() < 1e-12);
}

#[test]
fn pvsystemlist_syncs_derlist() {
    // PVSystemList (bare names) populates the class-prefixed DERList.
    let mut ec = ExpControl::new("e1");
    ec.set_string_list(prop::PVSYSTEM_LIST, vec!["pv1".into(), "pv2".into()]);
    ec.side_effects(prop::PVSYSTEM_LIST, 0);
    assert_eq!(ec.f_list_size, 2);
    assert_eq!(ec.der_name_list, vec!["PVSystem.pv1", "PVSystem.pv2"]);
    assert_eq!(ec.pvsystem_name_list, vec!["pv1", "pv2"]);
}

#[test]
fn derlist_syncs_pvsystemlist() {
    // DERList (class-prefixed) strips back to the bare PVSystemList.
    let mut ec = ExpControl::new("e1");
    ec.set_string_list(
        prop::DER_LIST,
        vec!["PVSystem.pvA".into(), "PVSystem.pvB".into()],
    );
    ec.side_effects(prop::DER_LIST, 0);
    assert_eq!(ec.f_list_size, 2);
    assert_eq!(ec.pvsystem_name_list, vec!["pvA", "pvB"]);
    assert_eq!(ec.der_name_list, vec!["PVSystem.pvA", "PVSystem.pvB"]);
}

// --- WP7.5 step 3: the dispatch math, pinned through a mock env ---
mod dispatch {
    use super::super::compute::{ExpDispatchEnv, PvFind, PvSnap};
    use super::super::{ExpControl, prop};
    use crate::elements::pc::pvsystem::VARMODE_KVAR;
    use crate::elements::traits::ElemRef;
    use crate::obj::base::DssObject;
    use crate::solution::CTRLSTATIC;

    /// A non-static control mode (so the static-init `find Vreg` branch is off).
    const TIMEDRIVEN: i32 = 1;

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    /// One mock PVSystem (the `Presentkvar`/`PresentkW` are the *achieved* reads,
    /// fixed inputs — the mock's `SetNominalDEROutput` does not recompute them, so
    /// they model the prior solve's converged values like Pascal).
    #[derive(Clone)]
    struct MockPv {
        name: String,
        enabled: bool,
        nphases: usize,
        vmag: f64,       // per-phase terminal voltage magnitude
        bus_kvbase: f64, // kV
        inverter_on: bool,
        var_follow_inverter: bool,
        kva_rating: f64,
        kvar_limit: f64,
        pmpp: f64,
        present_kw: f64,   // achieved
        present_kvar: f64, // achieved
        // --- dispatch outputs ---
        avr_mode: bool,
        vw_mode: bool,
        var_mode: i32,
        requested_kw: f64,
        pu_pmpp: f64,
        requested_kvar: f64,
        vreg: f64,
    }
    impl MockPv {
        fn new(name: &str, vpu: f64, present_kw: f64) -> Self {
            let bus_kvbase = 7.2;
            Self {
                name: name.into(),
                enabled: true,
                nphases: 3,
                vmag: vpu * bus_kvbase * 1000.0,
                bus_kvbase,
                inverter_on: true,
                var_follow_inverter: false,
                kva_rating: 600.0,
                kvar_limit: 600.0,
                pmpp: 600.0,
                present_kw,
                present_kvar: 0.0,
                avr_mode: false,
                vw_mode: true,
                var_mode: 0, // VARMODE_PF
                requested_kw: present_kw,
                pu_pmpp: 1.0,
                requested_kvar: 0.0,
                vreg: 0.0,
            }
        }
    }

    struct MockExpEnv {
        pvs: Vec<MockPv>,
        pushes: Vec<i32>,
        control_mode: i32,
        control_iter: i32,
        dyna_h: f64,
        loads_need_updating: bool,
    }
    impl MockExpEnv {
        fn new(pvs: Vec<MockPv>) -> Self {
            Self {
                pvs,
                pushes: Vec::new(),
                control_mode: TIMEDRIVEN,
                control_iter: 1,
                dyna_h: 1.0,
                loads_need_updating: false,
            }
        }
        fn idx(r: ElemRef) -> usize {
            r.idx
        }
    }
    impl ExpDispatchEnv for MockExpEnv {
        fn find_pvsystem(&self, name: &str) -> PvFind {
            match self
                .pvs
                .iter()
                .position(|d| d.name.eq_ignore_ascii_case(name))
            {
                None => PvFind::NotFound,
                Some(i) if self.pvs[i].enabled => PvFind::Found(ElemRef { cls: 0, idx: i }),
                Some(_) => PvFind::Disabled,
            }
        }
        fn all_pvsystems(&self) -> Vec<(String, ElemRef, bool)> {
            self.pvs
                .iter()
                .enumerate()
                .map(|(i, d)| (d.name.clone(), ElemRef { cls: 0, idx: i }, d.enabled))
                .collect()
        }
        fn pv_snap(&self, r: ElemRef) -> PvSnap {
            let d = &self.pvs[Self::idx(r)];
            PvSnap {
                name: d.name.clone(),
                nphases: d.nphases,
                inverter_on: d.inverter_on,
                var_follow_inverter: d.var_follow_inverter,
                kva_rating: d.kva_rating,
                kvar_limit: d.kvar_limit,
                pmpp: d.pmpp,
                bus_kvbase: d.bus_kvbase,
            }
        }
        fn pv_vterminal_mags(&mut self, r: ElemRef) -> Vec<f64> {
            let d = &self.pvs[Self::idx(r)];
            vec![d.vmag; d.nphases]
        }
        fn pv_present_kvar(&self, r: ElemRef) -> f64 {
            self.pvs[Self::idx(r)].present_kvar
        }
        fn pv_present_kw(&self, r: ElemRef) -> f64 {
            self.pvs[Self::idx(r)].present_kw
        }
        fn pv_set_avr_mode(&mut self, r: ElemRef, value: bool) {
            self.pvs[Self::idx(r)].avr_mode = value;
        }
        fn pv_set_vw_mode(&mut self, r: ElemRef, value: bool) {
            self.pvs[Self::idx(r)].vw_mode = value;
        }
        fn pv_set_var_mode(&mut self, r: ElemRef, mode: i32) {
            self.pvs[Self::idx(r)].var_mode = mode;
        }
        fn pv_set_nominal(&mut self, _r: ElemRef) {}
        fn pv_set_present_kw(&mut self, r: ElemRef, value: f64) {
            self.pvs[Self::idx(r)].requested_kw = value;
        }
        fn pv_set_pu_pmpp(&mut self, r: ElemRef, value: f64) {
            self.pvs[Self::idx(r)].pu_pmpp = value;
        }
        fn pv_set_present_kvar(&mut self, r: ElemRef, value: f64) {
            self.pvs[Self::idx(r)].requested_kvar = value;
        }
        fn pv_set_vreg_var(&mut self, r: ElemRef, value: f64) {
            self.pvs[Self::idx(r)].vreg = value;
        }
        fn push_change(&mut self, _delay: f64, code: i32) {
            self.pushes.push(code);
        }
        fn append_event(&mut self, _sender: &str, _msg: &str) {}
        fn control_mode(&self) -> i32 {
            self.control_mode
        }
        fn control_iteration(&self) -> i32 {
            self.control_iter
        }
        fn dyna_h(&self) -> f64 {
            self.dyna_h
        }
        fn set_loads_need_updating(&mut self) {
            self.loads_need_updating = true;
        }
    }

    /// A named-fleet ExpControl over PVSystem `pv1`.
    fn named_ec() -> ExpControl {
        let mut ec = ExpControl::new("e1");
        ec.set_string_list(prop::PVSYSTEM_LIST, vec!["pv1".into()]);
        ec.side_effects(prop::PVSYSTEM_LIST, 0);
        ec
    }

    #[test]
    fn make_list_named_builds_fleet_and_sets_avrmode() {
        let mut ec = named_ec();
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 1.0, 300.0)]);
        ec.sample(&mut env);
        assert_eq!(ec.fleet.len(), 1);
        assert!(env.pvs[0].avr_mode, "MakePVSystemList sets AVRmode := TRUE");
    }

    #[test]
    fn make_list_named_skips_missing_and_disabled() {
        let mut ec = ExpControl::new("e1");
        ec.set_string_list(
            prop::PVSYSTEM_LIST,
            vec!["pv1".into(), "ghost".into(), "pvoff".into()],
        );
        ec.side_effects(prop::PVSYSTEM_LIST, 0);
        let mut off = MockPv::new("pvoff", 1.0, 100.0);
        off.enabled = false;
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 1.0, 300.0), off]);
        ec.sample(&mut env);
        // Only the enabled, found member joins the fleet (no error on missing).
        assert_eq!(ec.fleet.len(), 1);
    }

    #[test]
    fn make_list_empty_scans_all_and_sets_avrmode() {
        let mut ec = ExpControl::new("e1"); // empty list → scan
        let mut env = MockExpEnv::new(vec![
            MockPv::new("pvA", 1.0, 200.0),
            MockPv::new("pvB", 1.0, 200.0),
        ]);
        ec.sample(&mut env);
        assert_eq!(ec.fleet.len(), 2);
        assert!(env.pvs[0].avr_mode && env.pvs[1].avr_mode);
        // The empty scan repopulates the bare name list.
        assert_eq!(ec.pvsystem_name_list, vec!["pvA", "pvB"]);
        assert_eq!(ec.f_list_size, 2);
    }

    #[test]
    fn sample_triggers_then_do_pending_sets_target_q() {
        // Vpu=1.02 above Vreg=1.0, Slope=50 → Qpu = -50*(1.02-1.0) = -1.0,
        // clamped by the dynamic headroom sqrt(1-(300/600)^2)=0.8660 then by
        // QmaxLead=0.44 → Qpu=-0.44 → TargetQ = 600*-0.44 = -264.0.
        // DeltaQ_Factor=0.7: Qset = -1 + (-264-(-1))*0.7 = -185.1.
        let mut ec = named_ec();
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 1.02, 300.0)]);
        ec.sample(&mut env);
        assert_eq!(env.pushes, vec![super::super::CHANGEVARLEVEL]);
        assert!(!ec.ctrl_vars[0].f_within_tol);

        ec.do_pending_action(&mut env);
        approx(ec.ctrl_vars[0].f_target_q, -264.0);
        approx(ec.ctrl_vars[0].f_last_iter_q, -185.1);
        approx(env.pvs[0].requested_kvar, -185.1);
        approx(ec.ctrl_vars[0].f_prior_vpu, 1.02);
        assert_eq!(env.pvs[0].var_mode, VARMODE_KVAR);
        assert!(!env.pvs[0].vw_mode);
        assert!(env.loads_need_updating);
        // Pending cleared after the action.
        assert_eq!(ec.ctrl_vars[0].f_pending_change, super::super::NONE);
    }

    #[test]
    fn not_injecting_tracks_vreg_and_skips_action() {
        // Inverter off + VarFollowInverter: no action; with VregTau>0 and Vreg<=0,
        // Vreg wakes up to the present voltage.
        let mut ec = ExpControl::new("e1");
        ec.f_vreg_init = 0.0; // ctrl_vars init Vreg=0 (<=0)
        ec.set_string_list(prop::PVSYSTEM_LIST, vec!["pv1".into()]);
        ec.side_effects(prop::PVSYSTEM_LIST, 0);
        let mut pv = MockPv::new("pv1", 1.03, 300.0);
        pv.inverter_on = false;
        pv.var_follow_inverter = true;
        let mut env = MockExpEnv::new(vec![pv]);
        ec.sample(&mut env);
        assert!(env.pushes.is_empty(), "no action when not injecting");
        approx(ec.ctrl_vars[0].f_vregs, 1.03); // woke to the grid voltage
    }

    #[test]
    fn update_exp_control_slews_vreg() {
        let mut ec = named_ec();
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 1.02, 300.0)]);
        ec.sample(&mut env); // builds fleet + sets present_vpu=1.02, Vreg=1.0
        ec.update_exp_control(&mut env);
        // Verr = 1.02 - 1.0; Vreg += Verr*(1-exp(-dt/VregTau)), dt=1, tau=1200.
        let expected = 1.0 + 0.02 * (1.0 - (-1.0_f64 / 1200.0).exp());
        approx(ec.ctrl_vars[0].f_vregs, expected);
        approx(env.pvs[0].vreg, expected); // written back via Set_Variable(5,…)
        // LastStepQ snapshots the achieved kvar (0 here).
        approx(ec.ctrl_vars[0].f_last_step_q, 0.0);
    }

    #[test]
    fn prefer_q_curtails_kw() {
        // PreferQ → Qmaxpu=1.0 (no dynamic headroom), so Qpu=-1.0 clamps only at
        // QmaxLead=0.44 → Plimit = 600*sqrt(1-0.44^2) = 538.99… < PresentkW=600 →
        // kW curtailed, puPmpp scaled.
        let mut ec = named_ec();
        ec.f_prefer_q = true;
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 1.02, 600.0)]);
        ec.sample(&mut env);
        ec.do_pending_action(&mut env);
        let plimit = 600.0 * (1.0 - 0.44_f64 * 0.44).sqrt();
        approx(env.pvs[0].requested_kw, plimit);
        approx(env.pvs[0].pu_pmpp, plimit / 600.0);
    }

    #[test]
    fn within_tolerance_no_action() {
        // Second control iteration, Verr/Qerr both 0 → within tol, no push.
        let mut ec = named_ec();
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 1.0, 300.0)]);
        env.control_iter = 1;
        ec.sample(&mut env); // iter 1 always triggers
        // Move prior to present so Verr=0 on the next sample.
        ec.ctrl_vars[0].f_prior_vpu = ec.ctrl_vars[0].f_present_vpu;
        env.pushes.clear();
        env.control_iter = 2;
        ec.sample(&mut env);
        assert!(env.pushes.is_empty());
        assert!(ec.ctrl_vars[0].f_within_tol);
    }

    #[test]
    fn static_init_finds_vreg_from_voltage() {
        // CTRLSTATIC + FVregInit<=0: Vreg is found from the present voltage,
        // clamped into [VregMin, VregMax]; an out-of-band hit nudges FVregInit.
        let mut ec = ExpControl::new("e1");
        ec.f_vreg_init = 0.0;
        ec.set_string_list(prop::PVSYSTEM_LIST, vec!["pv1".into()]);
        ec.side_effects(prop::PVSYSTEM_LIST, 0);
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 1.08, 300.0)]);
        env.control_mode = CTRLSTATIC;
        ec.sample(&mut env);
        // 1.08 > VregMax=1.05 → clamped to 1.05 and FVregInit nudged to 0.01.
        approx(ec.ctrl_vars[0].f_vregs, 1.05);
        approx(ec.f_vreg_init, 0.01);
    }

    #[test]
    fn static_init_clamps_low_and_keeps_in_band() {
        // The other two static-init arms (the >VregMax arm is above):
        //  - Vpu < VregMin → clamp UP to VregMin, nudge FVregInit to 0.01.
        let mut ec = ExpControl::new("e1");
        ec.f_vreg_init = 0.0;
        ec.set_string_list(prop::PVSYSTEM_LIST, vec!["pv1".into()]);
        ec.side_effects(prop::PVSYSTEM_LIST, 0);
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 0.90, 300.0)]);
        env.control_mode = CTRLSTATIC;
        ec.sample(&mut env);
        approx(ec.ctrl_vars[0].f_vregs, 0.95);
        approx(ec.f_vreg_init, 0.01);

        //  - Vpu in [VregMin, VregMax] → FVregs := Vpu, FVregInit NOT nudged.
        let mut ec2 = ExpControl::new("e1");
        ec2.f_vreg_init = 0.0;
        ec2.set_string_list(prop::PVSYSTEM_LIST, vec!["pv1".into()]);
        ec2.side_effects(prop::PVSYSTEM_LIST, 0);
        let mut env2 = MockExpEnv::new(vec![MockPv::new("pv1", 1.00, 300.0)]);
        env2.control_mode = CTRLSTATIC;
        ec2.sample(&mut env2);
        approx(ec2.ctrl_vars[0].f_vregs, 1.00);
        approx(ec2.f_vreg_init, 0.0); // in-band: no nudge
    }

    #[test]
    fn fopen_tau_lpf_lags_target_in_timedriven_mode() {
        // The FOpenTau low-pass filter (ExpControl.pas l.505-510) fires ONLY when
        // ControlMode<>CTRLSTATIC — dormant in the daily goldens (which run CTRLSTATIC),
        // so this is the per-call FOpenTau gate (the duty golden is the end-to-end one).
        // Tresponse=23.026 → FOpenTau=10; dt=1 → blend (1-exp(-0.1))=0.09516. The raw
        // target (as in sample_triggers) is -264.0; FLastStepQ seeds at -1.0, so the
        // filtered target = -1 + (-264-(-1))·0.09516 ≈ -26.03 — far from the unfiltered
        // -264.0 (a regression dropping the LPF lands on -264.0 and fails).
        let mut ec = named_ec();
        ec.set_f64(prop::TRESPONSE, 23.026);
        ec.recalc(); // derive FOpenTau = Tresponse / 2.3026 = 10
        let mut env = MockExpEnv::new(vec![MockPv::new("pv1", 1.02, 300.0)]);
        env.control_mode = TIMEDRIVEN;
        ec.sample(&mut env);
        ec.do_pending_action(&mut env);
        let factor = 1.0 - (-1.0_f64 / 10.0).exp();
        let expected = -1.0 + (-264.0 - (-1.0)) * factor;
        approx(ec.ctrl_vars[0].f_target_q, expected);
        assert!(
            (ec.ctrl_vars[0].f_target_q - (-264.0)).abs() > 100.0,
            "the LPF must move the target far from the unfiltered -264.0"
        );
    }

    #[test]
    fn do_pending_dispatches_each_fleet_member_independently() {
        // A 2-PV fleet at different voltages: each member gets its OWN slope-crossing
        // Q (no cross-DER state sharing in the per-DER loops — the only multi-member
        // dispatch coverage). pv1 at 1.02 absorbs (-264.0), pv2 at 0.98 injects (+264.0).
        let mut ec = ExpControl::new("e1");
        ec.set_string_list(prop::PVSYSTEM_LIST, vec!["pv1".into(), "pv2".into()]);
        ec.side_effects(prop::PVSYSTEM_LIST, 0);
        let mut env = MockExpEnv::new(vec![
            MockPv::new("pv1", 1.02, 300.0),
            MockPv::new("pv2", 0.98, 300.0),
        ]);
        ec.sample(&mut env);
        assert_eq!(ec.fleet.len(), 2);
        ec.do_pending_action(&mut env);
        approx(ec.ctrl_vars[0].f_target_q, -264.0);
        approx(ec.ctrl_vars[1].f_target_q, 264.0);
        // The deltaQ step off the -1.0 seed: pv1 -1+(-264-(-1))·0.7=-185.1;
        // pv2 -1+(264-(-1))·0.7=184.5 — distinct kvar pushed to each PV.
        approx(env.pvs[0].requested_kvar, -185.1);
        approx(env.pvs[1].requested_kvar, 184.5);
    }
}
