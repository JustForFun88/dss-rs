//! Dynamics-mode machinery for PVSystem: `InitStateVars`, `IntegrateStates`,
//! `DoDynamicMode` and the state-variable interface (`NumVariables` /
//! `VariableName` / `GetAllVariables`).
//!
//! The GFL (grid-following) inverter acts as a controlled current source whose
//! magnitude tracks the panel DC power and whose angle follows the grid voltage
//! at each terminal node. The per-phase modulation factor `m` and current `it`
//! are advanced by a PI controller (`TInvDynamicVars.SolveDynamicStep`) using
//! the trapezoidal predictor/corrector in `SolveDynamic`.
//!
//! Scope: the classic `DynamicEqObj = NIL` / `UserModel.Exists = FALSE` path
//! only. The external `DynamicEqObj` / `DynamicExp` path, the user-written DLL
//! model (`UserModel`, VoltageModel=3) and the grid-forming (GFM) inverter mode
//! are NOT_PORTED — see guards below.

use num_complex::Complex64;

use crate::elements::pc::inv_based_pce::{InvDynamicVars, NUM_INV_DYN_VARS};
use crate::elements::traits::{CktElement, SysCtx};
use crate::support::complexutil::{c_to_polar, pclx, to_polar};
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::PiCtrl;
use crate::util::CDOUBLEONE;

use super::PVSystem;

/// Pascal `NumBasePVSystemVariables = 13` / `NumPVSystemVariables = 22`
/// (PVsystem.pas l.30-31).
pub(super) const NUM_BASE_PV_VARS: usize = 13;
pub(super) const NUM_PV_VARS: usize = NUM_BASE_PV_VARS + NUM_INV_DYN_VARS; // = 22

impl PVSystem {
    /// Pascal `TPVsystemObj.InitStateVars` (l.2170) — seed the GFL inverter
    /// state from the present power-flow operating point. Ports only the
    /// `DynamicEqObj = NIL` / `UserModel.Exists = FALSE` path.
    pub(super) fn init_state_vars_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.yprim_invalid = true; // force rebuild of YPrims

        let nphases = self.cd.nphases;

        // Pascal `if (Length(PICtrl)=0) or (Length(PICtrl)<Fnphases)`:
        // create / resize the PI-controller array.
        if self.base.pi_ctrl.len() < nphases {
            self.base.pi_ctrl.resize_with(nphases, PiCtrl::new);
            for pi in &mut self.base.pi_ctrl {
                pi.kp = self.base.dyn_vars.kp;
                pi.k_num = 0.9502;
                pi.k_den = 0.04979;
            }
        }

        self.base.dyn_vars.safe_mode = false;

        // In dynamics mode `ActiveLoadShapeClass` is always `USENONE`, so the
        // Pascal `else ShapeFactor := CDOUBLEONE` branch always fires. The
        // USEDAILY/USEYEARLY/USEDUTY cases are UNREACHABLE here (SolveMode
        // is Dynamic → no load-shape dispatch). Mirrors the `SolveMode::Dynamic
        // => {}` no-op in `nominal.rs::set_nominal_der_output`.
        self.base.shape_factor = CDOUBLEONE;
        self.t_shape_value = self.f_temperature;

        self.compute_panel_power();

        // Pascal `NumPhases := Fnphases; NumConductors := Fnconds; Conn := Connection`
        // (these are the PVSystemVars "publicdata" fields; in the port the struct
        // fields *are* the data directly — no separate record to sync).

        // Size and zero the per-phase dynamics arrays.
        self.base.dyn_vars.init_dyn_arrays(nphases);

        // BasekV: L-N for multi-phase, kV directly for single-phase.
        let present_kv = self.kv_pvsystem_base;
        self.base.dyn_vars.base_kv = if nphases > 1 {
            present_kv / 3.0_f64.sqrt()
        } else {
            present_kv
        };

        let base_kv = self.base.dyn_vars.base_kv;
        let base_zt = 0.01 * (present_kv * present_kv / self.f_kva_rating) * 1000.0;
        let sm_thresh = self.base.dyn_vars.sm_threshold;
        self.base.dyn_vars.max_vs = (2.0 - sm_thresh / 100.0) * base_kv * 1000.0;
        self.base.dyn_vars.min_vs = (sm_thresh / 100.0) * base_kv * 1000.0;
        self.base.dyn_vars.min_amps =
            (self.base.fpct_cut_out / 100.0) * ((self.f_kva_rating / base_kv) / nphases as f64);
        self.base.dyn_vars.reset_ibr = false;
        self.base.dyn_vars.i_max_p_phase = (self.f_kva_rating / base_kv) / nphases as f64;

        if self.base.pct_x == 0.0 {
            self.base.pct_x = 50.0; // Pascal forces 50% in dynamics if not given
        }

        let x_thev = self.base.pct_x * base_zt;
        let rs = self.base.pct_r * base_zt;
        self.base.dyn_vars.rs = rs;
        let z_thev = Complex64::new(rs, x_thev);
        self.base.yeq = z_thev.inv(); // used for current calcs; always L-N

        self.compute_iterminal(sys, node_v);

        // `LS = XThev / (2 * PI * DefaultBaseFreq)`. `sys.fundamental` is the
        // circuit base frequency, equal to `DSS.DefaultBaseFreq` in dynamics.
        let two_pi_f = 2.0 * std::f64::consts::PI * sys.fundamental;
        self.base.dyn_vars.ls = x_thev / two_pi_f;

        let nphases_f = nphases as f64;
        let panel_kw = self.panel_kw;
        let rs = self.base.dyn_vars.rs;
        let rated_vdc = self.base.dyn_vars.rated_vdc;

        // Pascal loop `for i := 0 to (NPhases-1)` — 0-based; `NodeRef[i+1]`
        // in Pascal == `self.cd.node_ref[i]` in Rust (Pascal NodeRef is 1-based).
        for i in 0..nphases {
            self.base.dyn_vars.dit[i] = 0.0;
            self.base.dyn_vars.vgrid[i] = c_to_polar(node_v[self.cd.node_ref[i]]);

            // GFM branch is NOT_PORTED — WP7.7 GFM step. GFL only:
            let vg_mag = self.base.dyn_vars.vgrid[i].mag;
            self.base.dyn_vars.it[i] = ((panel_kw * 1000.0) / vg_mag) / nphases_f;

            let mut m_i = ((rs * self.base.dyn_vars.it[i]) + vg_mag) / rated_vdc;
            if m_i > 1.0 {
                m_i = 1.0;
            }
            self.base.dyn_vars.m[i] = m_i;
            self.base.dyn_vars.isp_delta[i] = 0.0;
            self.base.dyn_vars.ang_delta[i] = 0.0;
        }
        // NOT_PORTED: DynamicEqObj <> NIL init loop — WP7.7 step 3 (DynEqPCE).
    }

    /// Pascal `TPVsystemObj.IntegrateStates` (l.2264) — advance the GFL
    /// inverter state by one trapezoidal half-step. Ports only the
    /// `DynamicEqObj = NIL` / `UserModel.Exists = FALSE` / non-GFM path.
    pub(super) fn integrate_states_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.compute_iterminal(sys, node_v);

        // NOT_PORTED: UserModel.Exists branch — user-written DLL never ported.

        // In dynamics mode ActiveLoadShapeClass == USENONE → ShapeFactor := CDOUBLEONE.
        self.base.shape_factor = CDOUBLEONE;

        self.compute_panel_power();

        let nphases = self.cd.nphases;
        let nphases_f = nphases as f64;
        let h = sys.dyna_h;
        let iteration_flag = sys.iteration_flag;

        let panel_kw = self.panel_kw;
        let min_vs = self.base.dyn_vars.min_vs;

        // Update iMaxPPhase from current panel power.
        let base_kv = self.base.dyn_vars.base_kv;
        self.base.dyn_vars.i_max_p_phase = (panel_kw / base_kv) / nphases_f;
        let i_max_p_phase = self.base.dyn_vars.i_max_p_phase;

        for i in 0..nphases {
            if iteration_flag == IterationFlag::NewTimeStep {
                // First iteration of a new time step — predictor half-step.
                self.base.dyn_vars.it_history[i] =
                    self.base.dyn_vars.it[i] + 0.5 * h * self.base.dyn_vars.dit[i];
            }

            self.base.dyn_vars.vgrid[i] = c_to_polar(node_v[self.cd.node_ref[i]]);
            let vg_mag = self.base.dyn_vars.vgrid[i].mag;

            // NOT_PORTED: GFM_Mode branch (ISPDelta, VDelta, FixPhaseAngle) —
            // WP7.7 GFM step. GFL only:
            self.base.dyn_vars.isp = ((panel_kw * 1000.0) / vg_mag) / nphases_f;
            if self.base.dyn_vars.isp > i_max_p_phase {
                self.base.dyn_vars.isp = i_max_p_phase;
            }
            if vg_mag < min_vs {
                self.base.dyn_vars.isp = 0.01; // turn off the inverter
            }

            // NOT_PORTED: DynamicEqObj <> NIL branch — WP7.7 step 3.

            // Borrow-checker note: `pi_ctrl` and `dyn_vars` are both on `base`;
            // use mem::take to borrow them disjointly.
            let mut pi = std::mem::take(&mut self.base.pi_ctrl[i]);
            self.base
                .dyn_vars
                .solve_dynamic_step(iteration_flag, i, &mut pi);
            self.base.pi_ctrl[i] = pi;

            // Trapezoidal integration.
            self.base.dyn_vars.it[i] =
                self.base.dyn_vars.it_history[i] + 0.5 * h * self.base.dyn_vars.dit[i];
        }
        let _ = (i_max_p_phase, min_vs); // suppress unused-variable lint
    }

    /// Pascal `TPVsystemObj.DoDynamicMode` (l.1838) — inject the GFL current
    /// into the InjCurrent array. If `gfm_mode` is true, records a NOT_PORTED
    /// error and returns immediately.
    pub(super) fn do_dynamic_mode(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        if self.base.gfm_mode {
            // NOT_PORTED: GFM path (CalcGFMVoltage / CalcGFMYprim) — WP7.7 GFM step.
            errors.push(format!(
                "PVSystem.{}: grid-forming inverter mode (ControlMode=GFM) dynamics \
                 is not ported yet (Phase 7 WP7.7 GFM step).",
                self.cd.obj.name()
            ));
            return;
        }

        self.calc_yprim_contribution(node_v);

        match self.base.voltage_model {
            3 => {
                // NOT_PORTED: VoltageModel=3 user-written DLL dynamics model.
                // Pascal records error 5671 and sets SolutionAbort.
                errors.push(format!(
                    "PVSystem.{}: VoltageModel=3 user-written dynamics model is not \
                     defined (NOT_PORTED — safe Rust).",
                    self.cd.obj.name()
                ));
                return;
            }
            _ => {
                let nphases = self.cd.nphases;
                let nconds = self.cd.nconds;
                let i_max = self.base.dyn_vars.i_max_p_phase;

                let mut neut_amps = Complex64::ZERO;

                for i in 0..nphases {
                    let i_actual = if self.base.dyn_vars.it[i] <= i_max {
                        self.base.dyn_vars.it[i]
                    } else {
                        i_max
                    };

                    // Pascal `Iterminal[i] := -ptocomplex(topolar(iActual, Vgrid[i-1].ang))`.
                    let polar_n = to_polar(i_actual, self.base.dyn_vars.vgrid[i].ang);
                    self.cd.iterminal[i] = -pclx(polar_n.mag, polar_n.ang);
                    neut_amps -= self.cd.iterminal[i];
                }
                if nconds > nphases {
                    self.cd.iterminal[nconds - 1] = neut_amps;
                }
            }
        }

        self.cd.iterminal_updated = true;

        // Add into inj current array (`InjCurrent[i] -= Iterminal[i]`).
        let nconds = self.cd.nconds;
        for i in 0..nconds {
            self.cd.inj_current[i] -= self.cd.iterminal[i];
        }
        let _ = sys;
    }

    /// Pascal `TPVsystemObj.NumVariables` (l.2562) — 22 classic variables
    /// (13 base + 9 InvDynVars). Inherited `DynamicEqObj.NumVariables` returns
    /// 0 (DynamicEqObj = NIL); `UserModel.FNumVars` is NOT_PORTED (= 0).
    pub(super) fn num_pv_variables(&self) -> usize {
        NUM_PV_VARS // = 22
    }

    /// Pascal `TPVsystemObj.VariableName` (l.2575) (1-based).
    /// Returns an empty string for out-of-range `i` (like `inherited` returning
    /// `''`), matching the Pascal `if Length(Result) <> 0 then Exit` fallback.
    pub(super) fn pv_variable_name(&self, i: usize) -> String {
        match i {
            1 => "Irradiance",
            2 => "PanelkW",
            3 => "P_TFactor",
            4 => "Efficiency",
            5 => "Vreg",
            6 => "Vavg (DRC)",
            7 => "volt-var",
            8 => "volt-watt",
            9 => "DRC",
            10 => "VV_DRC",
            11 => "watt-pf",
            12 => "watt-var",
            13 => "kW_out_desired",
            14..=22 => InvDynamicVars::get_inv_dyn_name(i - NUM_BASE_PV_VARS - 1),
            _ => "", // out-of-range → empty (Pascal inherited returns '')
        }
        .to_string()
    }

    /// Pascal `TPVsystemObj.Get_Variable` (l.2397) (1-based).
    /// Returns -9999.99 for out-of-range `i`.
    pub(super) fn get_pv_variable(&self, i: usize) -> f64 {
        let nphases = self.cd.nphases;
        // NOT_PORTED: DynamicEqObj <> NIL path — WP7.7 step 3.
        match i {
            1 => self.f_irradiance,
            2 => self.panel_kw,
            3 => self.temp_factor,
            4 => self.eff_factor,
            5 => self.vreg,
            6 => self.vavg,
            7 => self.vv_operation,
            8 => self.vw_operation,
            9 => self.drc_operation,
            10 => self.vv_drc_operation,
            11 => self.wp_operation,
            12 => self.wv_operation,
            13 => self.panel_kw * self.eff_factor,
            14..=22 => self
                .base
                .dyn_vars
                .get_inv_dyn_value(i - NUM_BASE_PV_VARS - 1, nphases),
            _ => -9999.99,
        }
    }

    /// Pascal `TPVsystemObj.GetAllVariables` (l.2543): fill `states[0..21]`
    /// (0-based) with `Variable[1..22]` (1-based).
    /// NOT_PORTED: DynamicEqObj and UserModel paths.
    pub(super) fn get_all_pv_variables(&self, states: &mut [f64]) {
        for i in 1..=NUM_PV_VARS {
            if i - 1 < states.len() {
                states[i - 1] = self.get_pv_variable(i);
            }
        }
    }

    /// Pascal `TPVsystemObj.Set_Variable` (l.2489) (1-based, internal use).
    /// `pub(super)` — only used when a control writes to our state variables
    /// (e.g. InvControl writes `Vreg`, `Vavg`, `*Operation`).
    /// NOT_PORTED: DynamicEqObj and UserModel paths.
    #[allow(dead_code)]
    pub(super) fn set_pv_variable(&mut self, i: usize, value: f64) {
        match i {
            1 => self.f_irradiance = value,
            2..=4 => {} // read-only in Pascal
            5 => self.vreg = value,
            6 => self.vavg = value,
            7 => self.vv_operation = value,
            8 => self.vw_operation = value,
            9 => self.drc_operation = value,
            10 => self.vv_drc_operation = value,
            11 => self.wp_operation = value,
            12 => self.wv_operation = value,
            13 => {} // read-only
            14..=22 => self
                .base
                .dyn_vars
                .set_inv_dyn_value(i - NUM_BASE_PV_VARS - 1, value),
            _ => {}
        }
    }
}
