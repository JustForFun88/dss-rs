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
//! Scope: the classic GFL path, the grid-forming (GFM) black-start droop
//! (WPG.17 — `it := 0` init, the `IMaxPPhase`/`ISPDelta`/`FixPhaseAngle` ramp,
//! and the `DoDynamicMode` internal-voltage-source injection), and the external
//! `DynamicEqObj` / `DynamicExp` integration (WP7.7 step 3b — the user equation
//! replaces `SolveDynamicStep`). The user-written DLL model (`UserModel`,
//! VoltageModel=3) is NOT_PORTED — see the guard below.

use num_complex::Complex64;

use crate::elements::pc::dyneq_pce::DynEqPceData;
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
    /// state from the present power-flow operating point (+ the `DynamicEqObj <> NIL`
    /// derivative zero-out at the tail). `UserModel.Exists` is NOT_PORTED.
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
        // Pascal's USENONE branch sets only `ShapeFactor := CDOUBLEONE`; it does
        // NOT touch `TShapeValue` (`ComputePanelPower` reuses the prior value,
        // which on a normal snapshot→dynamics entry already equals `FTemperature`).
        self.base.shape_factor = CDOUBLEONE;

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
        let gfm_mode = self.base.gfm_mode;
        for i in 0..nphases {
            self.base.dyn_vars.dit[i] = 0.0;
            self.base.dyn_vars.vgrid[i] = c_to_polar(node_v[self.cd.node_ref[i]]);

            // Pascal PVsystem.pas l.2246-2249: GFM seeds `it[i] := 0` (black start
            // from zero current); GFL seeds it to the panel-power current target.
            let vg_mag = self.base.dyn_vars.vgrid[i].mag;
            self.base.dyn_vars.it[i] = if gfm_mode {
                0.0
            } else {
                ((panel_kw * 1000.0) / vg_mag) / nphases_f
            };

            let mut m_i = ((rs * self.base.dyn_vars.it[i]) + vg_mag) / rated_vdc;
            if m_i > 1.0 {
                m_i = 1.0;
            }
            self.base.dyn_vars.m[i] = m_i;
            self.base.dyn_vars.isp_delta[i] = 0.0;
            self.base.dyn_vars.ang_delta[i] = 0.0;
        }

        // DynamicEqObj <> NIL: zero the derivative column of the equation memory
        // (Pascal PVsystem.pas l.2258). Unlike the Generator, the inverter applies
        // no init-value seeding here — the per-phase `it`/`Vgrid`/`m` seeded by the
        // classic loop above *are* the state, loaded into `DynOut[0]` each step.
        if self.base.dyneq.has_dynamic_eq() {
            for row in self.base.dyneq.dynamic_eq_vals.iter_mut() {
                row[1] = 0.0;
            }
        }
    }

    /// Pascal `TPVsystemObj.IntegrateStates` (l.2264) — advance the GFL
    /// inverter state by one trapezoidal half-step (dispatching to
    /// `integrate_dyn_eq_phase` per phase when a `DynamicExp` is linked). The
    /// `UserModel.Exists` / non-GFM-only restrictions stay NOT_PORTED.
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

        // Update iMaxPPhase from current panel power (Pascal PVsystem.pas l.2305 —
        // PVSystem *overwrites* `dynVars.iMaxPPhase`, unlike Storage's local var).
        let base_kv = self.base.dyn_vars.base_kv;
        self.base.dyn_vars.i_max_p_phase = (panel_kw / base_kv) / nphases_f;
        let i_max_p_phase = self.base.dyn_vars.i_max_p_phase;
        let gfm_mode = self.base.gfm_mode;
        let reset_ibr = self.base.dyn_vars.reset_ibr;
        let ctrl_tol = self.base.dyn_vars.ctrl_tol;
        let kp = self.base.dyn_vars.kp;
        let i_limit = self.base.dyn_vars.i_limit;
        let v_error = self.base.dyn_vars.v_error;

        for i in 0..nphases {
            if iteration_flag == IterationFlag::NewTimeStep {
                // First iteration of a new time step — predictor half-step.
                self.base.dyn_vars.it_history[i] =
                    self.base.dyn_vars.it[i] + 0.5 * h * self.base.dyn_vars.dit[i];
            }

            self.base.dyn_vars.vgrid[i] = c_to_polar(node_v[self.cd.node_ref[i]]);
            let vg_mag = self.base.dyn_vars.vgrid[i].mag;

            if gfm_mode {
                // Pascal `TPVsystemObj.IntegrateStates` GFM sub-branch (PVsystem.pas
                // l.2324-2355): identical droop to Storage but the ramp/clamp uses
                // `IMaxPPhase` (the overwritten panel-power value) instead of a local.
                self.base.dyn_vars.v_delta[i] = if reset_ibr {
                    (0.001 - (vg_mag / 1000.0)) / base_kv
                } else {
                    (base_kv - (vg_mag / 1000.0)) / base_kv
                };

                let mut gfm_update = true;
                // ILimit>0 current-limit path (dormant unless `AmpLimit` set).
                if i_limit > 0.0 {
                    let mut curr = vec![Complex64::ZERO; self.cd.yorder];
                    self.get_currents(sys, node_v, &mut curr);
                    for c in curr.iter().take(nphases) {
                        gfm_update = gfm_update && (c.norm() < (i_limit * v_error));
                    }
                }

                if self.base.dyn_vars.v_delta[i].abs() > ctrl_tol && gfm_update {
                    self.base.dyn_vars.isp_delta[i] +=
                        (i_max_p_phase * self.base.dyn_vars.v_delta[i]) * kp * 100.0;
                    if self.base.dyn_vars.isp_delta[i] > i_max_p_phase {
                        self.base.dyn_vars.isp_delta[i] = i_max_p_phase;
                    } else if self.base.dyn_vars.isp_delta[i] < 0.0 {
                        self.base.dyn_vars.isp_delta[i] = 0.01;
                    }
                }
                self.base.dyn_vars.isp = self.base.dyn_vars.isp_delta[i];
                self.base.dyn_vars.fix_phase_angle(i);
            } else {
                // GFL: track the panel-power current target, off below MinVS.
                self.base.dyn_vars.isp = ((panel_kw * 1000.0) / vg_mag) / nphases_f;
                if self.base.dyn_vars.isp > i_max_p_phase {
                    self.base.dyn_vars.isp = i_max_p_phase;
                }
                if vg_mag < min_vs {
                    self.base.dyn_vars.isp = 0.01; // turn off the inverter
                }
            }

            if self.base.dyneq.has_dynamic_eq() {
                // DynamicEqObj <> NIL: integrate the user equation in place of
                // `SolveDynamicStep` (Pascal PVsystem.pas l.2356). Bring the present
                // per-phase current into the `DynOut[0]` slot, load the calculated
                // values, solve, then read the derivative back into `dit[i]`.
                self.integrate_dyn_eq_phase(sys, node_v, i, iteration_flag);
            } else {
                // Borrow-checker note: `pi_ctrl` and `dyn_vars` are both on `base`;
                // use mem::take to borrow them disjointly.
                let mut pi = std::mem::take(&mut self.base.pi_ctrl[i]);
                self.base
                    .dyn_vars
                    .solve_dynamic_step(iteration_flag, i, &mut pi);
                self.base.pi_ctrl[i] = pi;
            }

            // Trapezoidal integration (common to both paths).
            self.base.dyn_vars.it[i] =
                self.base.dyn_vars.it_history[i] + 0.5 * h * self.base.dyn_vars.dit[i];
        }
    }

    /// Pascal `TPVsystemObj.IntegrateStates`'s `DynamicEqObj <> NIL` body for one
    /// phase `i` (l.2356-2391): load `it[i]`/`dit[i]` into the `DynOut[0]` memory
    /// slot, load the calculated values the equation refers to, `SolveEq`, and read
    /// the integrated derivative back into `dit[i]`. The calc-value cases mirror
    /// the inverter overrides (Vgrid per phase, RatedVDC, the modulation `m[i]`);
    /// everything else falls through to the generic `Get_PCE_Value`.
    fn integrate_dyn_eq_phase(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        i: usize,
        iteration_flag: IterationFlag,
    ) {
        let out0 = self.base.dyneq.dyn_out[0];
        self.base.dyneq.dynamic_eq_vals[out0][0] = self.base.dyn_vars.it[i];
        self.base.dyneq.dynamic_eq_vals[out0][1] = self.base.dyn_vars.dit[i];

        let num_pairs = self.base.dyneq.dynamic_eq_pair.len() / 2;
        for j in 0..num_pairs {
            let var_idx = self.base.dyneq.dynamic_eq_pair[j * 2] as usize;
            let code = self.base.dyneq.dynamic_eq_pair[j * 2 + 1];
            if DynEqPceData::is_init_val(code) {
                continue; // initialization value — applied only in InitStateVars
            }
            match code {
                2 => self.base.dyneq.dynamic_eq_vals[var_idx][0] = self.base.dyn_vars.vgrid[i].mag,
                4 => {} // nothing for this object — the current is the DynOut[0] state
                10 => self.base.dyneq.dynamic_eq_vals[var_idx][0] = self.base.dyn_vars.rated_vdc,
                11 => {
                    let mut pi = std::mem::take(&mut self.base.pi_ctrl[i]);
                    self.base
                        .dyn_vars
                        .solve_modulation(iteration_flag, i, &mut pi);
                    self.base.pi_ctrl[i] = pi;
                    self.base.dyneq.dynamic_eq_vals[var_idx][0] = self.base.dyn_vars.m[i];
                }
                _ => {
                    let val = self.get_pce_value(sys, node_v, code);
                    self.base.dyneq.dynamic_eq_vals[var_idx][0] = val;
                }
            }
        }

        self.base.dyneq.solve_eq();
        self.base.dyn_vars.dit[i] = self.base.dyneq.dynamic_eq_vals[out0][1];
    }

    /// Pascal `TDSSCktElement.Get_PCE_Value(1, ValType)` (CktElement.pas l.828):
    /// the model-derived value a `DynamicExp` operand refers to, at the active
    /// terminal (terminal 1). The inverter `IntegrateStates` intercepts codes
    /// 2/4/10/11 (Vgrid/current/RatedVDC/modulation) before this, so only P/Q/Vang/
    /// Iang/S reach here; ported in full for fidelity. PVSystem is not a transformer,
    /// so `MaxVoltage` reads the node voltage at the max-current phase directly.
    fn get_pce_value(&mut self, sys: &SysCtx, node_v: &[Complex64], code: i32) -> f64 {
        match code {
            0 | 7 => -self.terminal_power(sys, node_v, 1).re, // P, P0
            1 | 8 => -self.terminal_power(sys, node_v, 1).im, // Q, Q0
            6 => self.terminal_power(sys, node_v, 1).norm(),  // S
            2..=5 => {
                self.compute_iterminal(sys, node_v);
                let mut max_curr = 0.0_f64;
                let mut max_phase = 0usize;
                for k in 0..self.cd.nphases {
                    let mag = self.cd.iterminal[k].norm();
                    if mag > max_curr {
                        max_curr = mag;
                        max_phase = k;
                    }
                }
                match code {
                    2 => node_v[self.cd.node_ref[max_phase]].norm(),
                    3 => crate::support::complexutil::cang(node_v[self.cd.node_ref[max_phase]]),
                    4 => max_curr,
                    _ => crate::support::complexutil::cang(self.cd.iterminal[max_phase]),
                }
            }
            _ => 0.0,
        }
    }

    /// Pascal `TPVsystemObj.DoDynamicMode` (l.1838) — inject the dynamics-mode
    /// current. GFL is a controlled current source; GFM (WPG.17) is an internal
    /// balanced voltage source scaled by the integrated filter current `it[0]`.
    pub(super) fn do_dynamic_mode(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        if self.base.gfm_mode {
            // Pascal `TPVsystemObj.DoDynamicMode` GFM arm (PVsystem.pas l.1870-1876):
            // internal balanced voltage source at `BaseV` (scaled by `it[0]/IMaxPPhase`
            // for the black-start ramp) behind `CalcGFMYprim`. `InjCurrent = YPrim ·
            // Vterminal(internal)`, `ITerminalUpdated := FALSE` (GetCurrents override
            // recomputes). The empty-array guard mirrors Storage (InitStateVars always
            // runs for PVSystem, so `it` is sized whenever this is reached).
            if self.base.dyn_vars.it.len() < self.cd.nphases {
                return;
            }
            self.base.dyn_vars.base_v = self.base.dyn_vars.base_kv
                * 1000.0
                * (self.base.dyn_vars.it[0] / self.base.dyn_vars.i_max_p_phase);
            let nphases = self.cd.nphases;
            self.base
                .dyn_vars
                .calc_gfm_voltage(nphases, &mut self.cd.vterminal);
            let cd = &mut self.cd;
            if let Some(yprim) = &cd.yprim {
                yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
            }
            self.cd.iterminal_updated = false;
            let _ = errors;
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

        // Pascal `set_ITerminalUpdated(TRUE)` (PVsystem.pas, end of `DoDynamicMode`)
        // sets the flag *and* stamps `IterminalSolutionCount := SolutionCount`, so a
        // post-solve `ComputeIterminal`/`GetCurrents` reuses the cached terminal
        // current instead of recomputing the model (mirrors `put_curr`).
        self.cd.iterminal_updated = true;
        self.cd.iterminal_solution_count = sys.solution_count;

        // Add into inj current array (`InjCurrent[i] -= Iterminal[i]`).
        let nconds = self.cd.nconds;
        for i in 0..nconds {
            self.cd.inj_current[i] -= self.cd.iterminal[i];
        }
        let _ = sys;
    }

    /// Pascal `TPVsystemObj.NumVariables` (l.2562) — the 22 classic variables
    /// (13 base + 9 InvDynVars). The linked-`DynamicExp` count is dispatched ahead
    /// of this in the `num_variables` accessor; `UserModel.FNumVars` is NOT_PORTED.
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
        // DynamicEqObj <> NIL: read the equation memory directly (Pascal l.2409).
        // The `1..=` guard avoids the `i = 0` underflow Pascal leaves as UB; an
        // out-of-range index returns the same sentinel as the classic path (Pascal
        // pushes msg 565 and exits with the seeded `Result`, no error channel here).
        if self.base.dyneq.has_dynamic_eq() {
            if (1..=self.base.dyneq.num_variables()).contains(&i) {
                return self.base.dyneq.get_dynamic_eq_val(i - 1);
            }
            return -9999.99;
        }
        match i {
            // Pascal `PresentIrradiance = FIrradiance * ShapeFactor.re` (l.2421/2132).
            1 => self.present_irradiance(),
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
    /// (0-based) with `Variable[1..22]` (1-based). The `DynamicEqObj` memory dump is
    /// handled by the `get_all_variables` accessor short-circuit; UserModel is
    /// NOT_PORTED.
    pub(super) fn get_all_pv_variables(&self, states: &mut [f64]) {
        for i in 1..=NUM_PV_VARS {
            if i - 1 < states.len() {
                states[i - 1] = self.get_pv_variable(i);
            }
        }
    }

    /// Pascal `TPVsystemObj.Set_Variable` (l.2489) (1-based). The write side of
    /// the state-variable interface, reached via the `set_variable` trait method.
    /// A linked `DynamicExp` makes every state variable read-only (msg 566, below);
    /// UserModel is NOT_PORTED.
    pub(super) fn set_pv_variable(&mut self, i: usize, value: f64) {
        // DynamicEqObj <> NIL: state variables are read-only — the equation drives
        // them (Pascal Set_Variable l.2498, msg 566).
        if self.base.dyneq.has_dynamic_eq() {
            self.cd.obj.push_error(format!(
                "PVSystem.{}: cannot set state variable when using DynamicEq.",
                self.cd.obj.name()
            ));
            return;
        }
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
