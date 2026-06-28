//! Dynamics-mode machinery for Storage: `InitStateVars`, `IntegrateStates`,
//! `DoDynamicMode` and the state-variable interface (`NumVariables` /
//! `VariableName` / `GetAllVariables`).
//!
//! The GFL (grid-following) inverter acts as a controlled current source. In
//! the discharging state the current magnitude tracks `kW_out`; in idling or
//! charging states it tracks `PIdling`. The per-phase modulation factor `m` and
//! current `it` are advanced by a PI controller using the trapezoidal
//! predictor/corrector in `SolveDynamic`.
//!
//! Scope: the classic GFL path and the external `DynamicEqObj` / `DynamicExp`
//! integration (WP7.7 step 3b — the user equation replaces `SolveDynamicStep`).
//! NOT_PORTED defers: GFM, DynaModel/UserModel DLLs.

use num_complex::Complex64;

use crate::elements::pc::dyneq_pce::DynEqPceData;
use crate::elements::pc::inv_based_pce::{InvDynamicVars, NUM_INV_DYN_VARS};
use crate::elements::traits::{CktElement, SysCtx};
use crate::support::complexutil::{c_to_polar, pclx, to_polar};
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::PiCtrl;
use crate::util::CDOUBLEONE;

use super::{STORE_DISCHARGING, STORE_IDLING, Storage};

/// Pascal `NumBaseStorageVariables = 25` / `NumStorageVariables = 34`
/// (Storage.pas l.29).
pub(super) const NUM_BASE_STORAGE_VARS: usize = 25;
pub(super) const NUM_STORAGE_VARS: usize = NUM_BASE_STORAGE_VARS + NUM_INV_DYN_VARS; // = 34

impl Storage {
    /// Pascal `TStorageObj.InitStateVars` (l.2742) — seed the GFL inverter
    /// state from the present power-flow operating point (+ the `DynamicEqObj <> NIL`
    /// derivative zero-out at the tail). Only runs the discharging path (Pascal
    /// `if FState <> STORE_DISCHARGING then Exit`). NOT_PORTED: DynaModel.
    pub(super) fn init_state_vars_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let _ = node_v; // used below via self.cd.node_ref
        self.cd.yprim_invalid = true;

        let nphases = self.cd.nphases;

        // Pascal `if (Length(PICtrl)=0) or (Length(PICtrl)<Fnphases)`.
        if self.base.pi_ctrl.len() < nphases {
            self.base.pi_ctrl.resize_with(nphases, PiCtrl::new);
            for pi in &mut self.base.pi_ctrl {
                pi.kp = self.base.dyn_vars.kp;
                pi.k_num = 0.9502;
                pi.k_den = 0.04979;
            }
        }

        // Pascal: `ZThev := Cmplx(RThev, XThev); Yeq := Cinv(ZThev)` (for state-var init).
        let z_thev = Complex64::new(self.r_thev, self.x_thev);
        self.base.yeq = z_thev.inv();

        // NOT_PORTED: DynaModel.Exists branch — WP7.7 never.

        // Pascal `if FState <> STORE_DISCHARGING then Exit`.
        if self.f_state != STORE_DISCHARGING {
            return;
        }

        let present_kv = self.kv_storage_base;

        // BasekV: L-N for multi-phase, kV for single-phase.
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
        let z_thev2 = Complex64::new(rs, x_thev);
        self.base.yeq = z_thev2.inv(); // always L-N

        self.compute_present_kw();

        // Pascal `LS := ZThev.im / (2 * PI * DefaultBaseFreq)`.
        // `sys.fundamental` == `DSS.DefaultBaseFreq` in dynamics.
        let two_pi_f = 2.0 * std::f64::consts::PI * sys.fundamental;
        self.base.dyn_vars.ls = x_thev / two_pi_f;

        // Size and zero the per-phase dynamics arrays.
        self.base.dyn_vars.init_dyn_arrays(nphases);

        let rs = self.base.dyn_vars.rs;
        let rated_vdc = self.base.dyn_vars.rated_vdc;

        // Pascal loop `for i := 0 to (NPhases-1)`:
        for i in 0..nphases {
            self.base.dyn_vars.vgrid[i] = c_to_polar(node_v[self.cd.node_ref[i]]);
            self.base.dyn_vars.dit[i] = 0.0;
            self.base.dyn_vars.it[i] = 0.0;
            let vg_mag = self.base.dyn_vars.vgrid[i].mag;
            let mut m_i = ((rs * self.base.dyn_vars.it[i]) + vg_mag) / rated_vdc;
            if m_i > 1.0 {
                m_i = 1.0;
            }
            self.base.dyn_vars.m[i] = m_i;
            self.base.dyn_vars.isp_delta[i] = 0.0;
            self.base.dyn_vars.ang_delta[i] = 0.0;
        }

        // DynamicEqObj <> NIL: zero the derivative column of the equation memory
        // (Pascal Storage.pas l.2834). Reached only for a discharging unit (the
        // `FState <> STORE_DISCHARGING` early-return above); the per-phase `it`/
        // `Vgrid`/`m` seeded by the classic loop *are* the state.
        if self.base.dyneq.has_dynamic_eq() {
            for row in self.base.dyneq.dynamic_eq_vals.iter_mut() {
                row[1] = 0.0;
            }
        }
    }

    /// Pascal `TStorageObj.IntegrateStates` (l.2840) — advance the GFL
    /// inverter state by one trapezoidal half-step (dispatching to
    /// `integrate_dyn_eq_phase` per phase when a `DynamicExp` is linked).
    /// NOT_PORTED: DynaModel, GFM, DebugTrace.
    pub(super) fn integrate_states_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.compute_iterminal(sys, node_v);

        // A Storage that is NOT discharging at dynamics entry skips `InitDynArrays`
        // (Pascal `InitStateVars` l.2786 `if FState <> STORE_DISCHARGING then Exit`),
        // leaving the per-phase arrays empty. Pascal then dereferences nil arrays
        // (access violation) if such an element is integrated, so there is no oracle
        // baseline; no-op instead of panicking. (A unit that trips to idling *during*
        // the run kept the arrays sized at its discharging init, so that path runs.)
        if self.base.dyn_vars.it.len() < self.cd.nphases {
            return;
        }

        // NOT_PORTED: DynaModel.Exists branch.

        // In dynamics mode ActiveLoadShapeClass == USENONE → ShapeFactor = CDOUBLEONE.
        self.base.shape_factor = CDOUBLEONE;

        self.compute_present_kw();

        let nphases = self.cd.nphases;
        let nphases_f = nphases as f64;
        let h = sys.dyna_h;
        let iteration_flag = sys.iteration_flag;

        let base_kv = self.base.dyn_vars.base_kv;
        let kw_out = self.base.kw_out;
        let i_max_phase = (kw_out / base_kv) / nphases_f;
        let min_vs = self.base.dyn_vars.min_vs;
        let max_vs = self.base.dyn_vars.max_vs;
        let i_max_p_phase = self.base.dyn_vars.i_max_p_phase;
        let p_idling = self.p_idling;
        let reset_ibr = self.base.dyn_vars.reset_ibr;

        for i in 0..nphases {
            if self.f_state == STORE_DISCHARGING {
                if iteration_flag == IterationFlag::NewTimeStep {
                    self.base.dyn_vars.it_history[i] =
                        self.base.dyn_vars.it[i] + 0.5 * h * self.base.dyn_vars.dit[i];
                }

                self.base.dyn_vars.vgrid[i] = c_to_polar(node_v[self.cd.node_ref[i]]);
                let vg_mag = self.base.dyn_vars.vgrid[i].mag;

                // NOT_PORTED: GFM_Mode branch — WP7.7 GFM step.
                // GFL only:
                if vg_mag < min_vs || vg_mag > max_vs {
                    self.base.dyn_vars.isp = 0.01; // turn off the inverter
                    self.f_state = STORE_IDLING;
                    if vg_mag > max_vs {
                        self.base.dyn_vars.vgrid[i].mag = max_vs;
                    }
                } else {
                    self.base.dyn_vars.isp = ((kw_out * 1000.0) / vg_mag) / nphases_f;
                }
                if self.base.dyn_vars.isp > i_max_p_phase {
                    self.base.dyn_vars.isp = i_max_p_phase;
                }

                if self.base.dyneq.has_dynamic_eq() {
                    // DynamicEqObj <> NIL: integrate the user equation in place of
                    // `SolveDynamicStep` (Pascal Storage.pas l.2919).
                    self.integrate_dyn_eq_phase(sys, node_v, i, iteration_flag);
                } else {
                    let mut pi = std::mem::take(&mut self.base.pi_ctrl[i]);
                    self.base
                        .dyn_vars
                        .solve_dynamic_step(iteration_flag, i, &mut pi);
                    self.base.pi_ctrl[i] = pi;
                }

                // Trapezoidal integration (common to both paths).
                self.base.dyn_vars.it[i] =
                    self.base.dyn_vars.it_history[i] + 0.5 * h * self.base.dyn_vars.dit[i];
            } else {
                // Not discharging (charging or idling).
                //
                // TODO(compat): Pascal leaves `OFFVal` uninitialized in the `else`
                // branch (Vgrid.mag < MinVS AND NOT ResetIBR) — FPC local var is
                // indeterminate; we use 0.0. Unreachable in the gated corpus
                // (idling storages stay >= MinVS when the circuit is energized).
                let off_val = if self.base.dyn_vars.vgrid[i].mag >= min_vs || reset_ibr {
                    p_idling / self.base.dyn_vars.vgrid[i].mag // to match idling losses
                } else {
                    0.0
                };
                self.base.dyn_vars.it[i] = off_val;
            }
        }
        let _ = (
            i_max_phase,
            i_max_p_phase,
            min_vs,
            max_vs,
            p_idling,
            reset_ibr,
        );
    }

    /// Pascal `TStorageObj.IntegrateStates`'s `DynamicEqObj <> NIL` body for one
    /// phase `i` (l.2919-2951): load `it[i]`/`dit[i]` into the `DynOut[0]` memory
    /// slot, load the calculated values the equation refers to, `SolveEq`, and read
    /// the integrated derivative back into `dit[i]`. Identical to the PVSystem body
    /// (the shared inverter calc-value cases: Vgrid per phase, RatedVDC, modulation).
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
    /// the model-derived value a `DynamicExp` operand refers to, at terminal 1.
    /// The inverter `IntegrateStates` intercepts codes 2/4/10/11, so only P/Q/Vang/
    /// Iang/S reach here; ported in full for fidelity. Storage is not a transformer,
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

    /// Pascal `TStorageObj.DoDynamicMode` (l.2119) — inject the GFL current.
    /// NOT_PORTED: DynaModel, GFM.
    pub(super) fn do_dynamic_mode(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        // NOT_PORTED: DynaModel.Exists branch — user-written dynamics DLL, never ported.
        // In this port DynaModel.Exists is always false; if somehow reached:

        if self.base.gfm_mode {
            // NOT_PORTED: GFM path (CalcGFMVoltage) — WP7.7 GFM step.
            errors.push(format!(
                "Storage.{}: grid-forming inverter mode (ControlMode=GFM) dynamics \
                 is not ported yet (Phase 7 WP7.7 GFM step).",
                self.cd.obj.name()
            ));
            return;
        }

        // Non-discharging-at-entry Storage skipped `InitDynArrays` (empty per-phase
        // arrays) — Pascal derefs nil here too; no-op rather than panic (see
        // `integrate_states_impl`).
        if self.base.dyn_vars.it.len() < self.cd.nphases {
            return;
        }

        self.calc_yprim_contribution(node_v);

        // Pascal `ZeroITerminal`.
        for c in self.cd.iterminal.iter_mut() {
            *c = Complex64::ZERO;
        }

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        let i_max = self.base.dyn_vars.i_max_p_phase;
        let min_amps = self.base.dyn_vars.min_amps;
        let p_idling = self.p_idling;
        let f_state = self.f_state;

        let mut neut_amps = Complex64::ZERO;
        for i in 0..nphases {
            let ang_cmp = 0.0_f64; // Pascal `AngCmp := 0` in GFL path

            let mut i_actual = if self.base.dyn_vars.it[i] <= i_max {
                self.base.dyn_vars.it[i]
            } else {
                i_max
            };

            if i_actual < min_amps {
                i_actual = 0.0; // match %CutOut
            }

            if f_state != STORE_DISCHARGING {
                i_actual = (p_idling / self.base.dyn_vars.vgrid[i].mag) / nphases as f64;
            }

            let polar_n = to_polar(i_actual, self.base.dyn_vars.vgrid[i].ang + ang_cmp);
            let curr = -pclx(polar_n.mag, polar_n.ang);
            neut_amps -= curr;
            self.cd.iterminal[i] = curr;
        }

        if nconds > nphases {
            self.cd.iterminal[nconds - 1] = neut_amps;
        }

        // Add into inj current array.
        let nconds = self.cd.nconds;
        for i in 0..nconds {
            self.cd.inj_current[i] -= self.cd.iterminal[i];
        }

        // Pascal `set_ITerminalUpdated(TRUE)` (Storage.pas, end of `DoDynamicMode`)
        // sets the flag *and* stamps `IterminalSolutionCount := SolutionCount`, so a
        // post-solve `ComputeIterminal`/`GetCurrents` reuses the cached terminal
        // current instead of recomputing the model (mirrors `put_curr`).
        self.cd.iterminal_updated = true;
        self.cd.iterminal_solution_count = sys.solution_count;
    }

    // -----------------------------------------------------------------------
    // Loss getters used by Get_Variable (Pascal Storage.pas l.2640-2694).
    // These take (sys, node_v) to call terminal_power / dckw.
    // -----------------------------------------------------------------------

    /// Pascal `TStorageObj.Get_kWTotalLosses` (l.2640).
    fn get_kw_total_losses(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        self.kw_idling_losses(sys, node_v)
            + self.get_inverter_losses(sys, node_v)
            + self.get_kw_chdch_losses(sys, node_v)
    }

    /// Pascal `TStorageObj.Get_InverterLosses` (l.2645).
    fn get_inverter_losses(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        let p1 = self.terminal_power(sys, node_v, 1).re * 0.001; // Power[1] in kW
        let dc = self.dckw(sys, node_v);
        match self.f_state {
            STORE_IDLING => p1.abs() - dc.abs(),
            super::STORE_CHARGING => p1.abs() - dc.abs(),
            STORE_DISCHARGING => dc - p1.abs(),
            _ => 0.0,
        }
    }

    /// Pascal `TStorageObj.Get_kWChDchLosses` (l.2673).
    fn get_kw_chdch_losses(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        let dc = self.dckw(sys, node_v).abs();
        let p_idling = self.p_idling;
        match self.f_state {
            STORE_IDLING => 0.0,
            super::STORE_CHARGING => {
                if dc - p_idling > 0.0 {
                    (dc - p_idling) * (1.0 - 0.01 * self.pct_charge_eff)
                } else {
                    -(dc - p_idling) * (1.0 / (0.01 * self.pct_discharge_eff) - 1.0)
                }
            }
            STORE_DISCHARGING => (dc + p_idling) * (1.0 / (0.01 * self.pct_discharge_eff) - 1.0),
            _ => 0.0,
        }
    }

    /// Pascal `TStorageObj.Get_kWDesired` (l.2626).
    fn get_kw_desired(&self) -> f64 {
        match self.state_desired {
            super::STORE_CHARGING => -self.pct_kw_in * self.kw_rating / 100.0,
            STORE_DISCHARGING => self.pct_kw_out * self.kw_rating / 100.0,
            _ => 0.0,
        }
    }

    /// Pascal `TStorageObj.Update_EfficiencyFactor` (l.2696).
    fn update_efficiency_factor(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        if self.base.inverter_curve_obj.is_none() {
            self.eff_factor = 1.0;
        } else {
            let dc_abs = self.dckw(sys, node_v).abs();
            if let Some(c) = self.base.inverter_curve_obj.as_mut() {
                self.eff_factor = c.get_y_value(dc_abs / self.f_kva_rating);
            }
        }
        self.eff_factor
    }

    // -----------------------------------------------------------------------
    // State-variable interface
    // -----------------------------------------------------------------------

    /// Pascal `TStorageObj.NumVariables` (l.3203) — the 34 classic variables
    /// (25 base + 9 InvDynVars). The linked-`DynamicExp` count is dispatched ahead
    /// of this in the `num_variables` accessor; UserModel/DynaModel are NOT_PORTED.
    pub(super) fn num_storage_variables(&self) -> usize {
        NUM_STORAGE_VARS // = 34
    }

    /// Pascal `TStorageObj.VariableName` (l.3220) (1-based).
    /// Pascal seeds `Result := 'ERROR'` then `Result := inherited VariableName(i)`
    /// (`''` for `i >= 1` with no DynamicEq): so `i < 1` returns `'ERROR'`, but an
    /// out-of-range high index (`i > 34`, no UserModel) returns `''`.
    pub(super) fn storage_variable_name(&self, i: usize) -> String {
        match i {
            1 => "kWh",
            2 => "State",
            3 => "kWOut",
            4 => "kWIn",
            5 => "kvarOut",
            6 => "DCkW",
            7 => "kWTotalLosses",
            8 => "kWInvLosses",
            9 => "kWIdlingLosses",
            10 => "kWChDchLosses",
            11 => "kWh Chng",
            12 => "InvEff",
            13 => "InverterON",
            14 => "Vref",
            15 => "Vavg (DRC)",
            16 => "VV Oper",
            17 => "VW Oper",
            18 => "DRC Oper",
            19 => "VV_DRC Oper",
            20 => "WP Oper",
            21 => "WV Oper",
            22 => "kWDesired",
            23 => "kW VW Limit",
            24 => "Limit kWOut Function",
            25 => "kVA Exceeded",
            26..=34 => InvDynamicVars::get_inv_dyn_name(i - NUM_BASE_STORAGE_VARS - 1),
            0 => "ERROR", // Pascal `i < 1` exits with the 'ERROR' seed.
            _ => "",      // i > 34 (no UserModel): the inherited '' (not 'ERROR').
        }
        .to_string()
    }

    /// Pascal `TStorageObj.Get_Variable` (l.2977) (1-based).
    /// Returns -9999.99 for out-of-range `i`. UserModel/DynaModel are NOT_PORTED.
    pub(super) fn get_storage_variable(
        &mut self,
        i: usize,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> f64 {
        let nphases = self.cd.nphases;
        // DynamicEqObj <> NIL: read the equation memory directly (Pascal l.2989).
        // The `1..=` guard avoids the `i = 0` underflow Pascal leaves as UB; an
        // out-of-range index returns the same sentinel as the classic path.
        if self.base.dyneq.has_dynamic_eq() {
            if (1..=self.base.dyneq.num_variables()).contains(&i) {
                return self.base.dyneq.get_dynamic_eq_val(i - 1);
            }
            return -9999.99;
        }
        match i {
            1 => self.kwh_stored,
            2 => {
                // Non-GFM: report FState directly.
                // NOT_PORTED: GFM_Mode CheckIfDelivering path — WP7.7 GFM.
                self.f_state as f64
            }
            3 | 4 => {
                // Pascal `A := GFM_mode and CheckIfDelivering(); B := (FState=DISCH) and (not GFM_mode); A:=A or B`.
                // GFM_Mode always false in this port, so: A = B = (FState == DISCHARGING).
                let mut a = self.f_state == STORE_DISCHARGING;
                if i == 4 {
                    a = !a;
                }
                if a {
                    self.terminal_power(sys, node_v, 1).re.abs() * 0.001
                } else {
                    0.0
                }
            }
            5 => -self.terminal_power(sys, node_v, 1).im * 0.001,
            6 => self.dckw(sys, node_v),
            7 => self.get_kw_total_losses(sys, node_v),
            8 => self.get_inverter_losses(sys, node_v),
            9 => self.kw_idling_losses(sys, node_v),
            10 => self.get_kw_chdch_losses(sys, node_v),
            11 => self.kwh_stored - self.kwh_before_update,
            12 => self.update_efficiency_factor(sys, node_v),
            13 => {
                if self.base.inverter_on {
                    1.0
                } else {
                    0.0
                }
            }
            14 => self.vreg,
            15 => self.vavg,
            16 => self.vv_operation,
            17 => self.vw_operation,
            18 => self.drc_operation,
            19 => self.vv_drc_operation,
            20 => self.wp_operation,
            21 => self.wv_operation,
            22 => self.get_kw_desired(),
            23 => {
                if !self.base.vw_mode {
                    9999.0
                } else {
                    self.kw_requested
                }
            }
            24 => self.pct_kw_rated * self.kw_rating,
            25 => {
                if self.kva_exceeded {
                    1.0
                } else {
                    0.0
                }
            }
            26..=34 => self
                .base
                .dyn_vars
                .get_inv_dyn_value(i - NUM_BASE_STORAGE_VARS - 1, nphases),
            _ => -9999.99,
        }
    }

    /// Pascal `TStorageObj.GetAllVariables` (l.3176): fill `states[0..33]`
    /// (0-based) with `Variable[1..34]` (1-based). The `DynamicEqObj` memory dump is
    /// handled by the `get_all_variables` accessor short-circuit; UserModel/DynaModel
    /// are NOT_PORTED.
    pub(super) fn get_all_storage_variables(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        states: &mut [f64],
    ) {
        for i in 1..=NUM_STORAGE_VARS {
            if i - 1 < states.len() {
                states[i - 1] = self.get_storage_variable(i, sys, node_v);
            }
        }
    }

    /// Pascal `TStorageObj.Set_Variable` (l.3110) (1-based). The write side of the
    /// state-variable interface, reached via the `set_variable` trait method.
    /// A linked `DynamicExp` makes every state variable read-only (msg 566, below);
    /// UserModel/DynaModel are NOT_PORTED.
    pub(super) fn set_storage_variable(&mut self, i: usize, value: f64) {
        // DynamicEqObj <> NIL: state variables are read-only — the equation drives
        // them (Pascal Set_Variable, msg 566).
        if self.base.dyneq.has_dynamic_eq() {
            self.cd.obj.push_error(format!(
                "Storage.{}: cannot set state variable when using DynamicEq.",
                self.cd.obj.name()
            ));
            return;
        }
        match i {
            1 => self.kwh_stored = value,
            2 => self.f_state = value.trunc() as i32,
            3..=13 | 22..=25 => {} // read-only in Pascal
            14 => self.vreg = value,
            15 => self.vavg = value,
            16 => self.vv_operation = value,
            17 => self.vw_operation = value,
            18 => self.drc_operation = value,
            19 => self.vv_drc_operation = value,
            20 => self.wp_operation = value,
            21 => self.wv_operation = value,
            26..=34 => self
                .base
                .dyn_vars
                .set_inv_dyn_value(i - NUM_BASE_STORAGE_VARS - 1, value),
            _ => {}
        }
    }
}
