//! Dynamics-mode machinery: `InitStateVars`, `IntegrateStates`, `DoDynamicMode`
//! and the 22 state variables. Unlike the classic behind-Xd' generator, the
//! electrical injection comes from the embedded [`Wtg3Model`](super::wtg3):
//! `DoDynamicMode` hands it the terminal V/I and reads back a Norton current.
//! The classic shaft swing (`Theta`/`Speed`) is still integrated (as upstream)
//! but is not a readable variable — it exists only so the per-step
//! `WindModelDyn.Integrate()` side effect fires.

use num_complex::Complex64;

use crate::elements::pc::dyneq_pce::DynEqPceData;
use crate::elements::traits::{CktElement, SysCtx};
use crate::support::complexutil::cang;
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::{SymComp, terminal_power_in};

use super::WindGen;

const TWO_PI: f64 = 2.0 * std::f64::consts::PI;

impl WindGen {
    /// Pascal `TWindGenObj.InitStateVars` — seed the WTG3 model from the present
    /// (power-flow) operating point.
    pub(super) fn init_state_vars_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.yprim_invalid = true; // force rebuild of YPrims

        self.yeq = self.wind_model_dyn.zthev.inv();

        if !self.gen_on {
            self.vthev = Complex64::ZERO;
            self.theta = 0.0;
            self.dtheta = 0.0;
            self.w0 = 0.0;
            self.speed = 0.0;
            self.dspeed = 0.0;
            return;
        }

        self.compute_iterminal(sys, node_v);

        match self.cd.nphases {
            3 => {
                let sc = SymComp::default();
                let mut i012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&self.cd.iterminal[..3], &mut i012);
                let mut vabc = [Complex64::ZERO; 3];
                for (i, v) in vabc.iter_mut().enumerate() {
                    *v = node_v[self.cd.node_ref[i]]; // wye voltage
                }
                let mut v012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&vabc, &mut v012);
                let edp = v012[1] - i012[1] * self.wind_model_dyn.zthev; // pos sequence
                self.edp = edp;
                self.v_thev_mag = edp.norm();
            }
            _ => {
                // Pascal `case Fnphases of 1: 3: else DoSimpleMsg(...5672)` accepts
                // 1-phase (computes Edp) then still calls `WindModelDyn.Init` — but
                // the embedded WTG3 model is 3-phase-only: `Instrumentation`
                // (WTG3_Model.pas:494) and every per-step `CalcDynamic` read
                // `V[1..3]`/`i[1..3]`, so a 1-phase terminal (2 conductors)
                // over-reads the terminal array = heap UB upstream. Per CLAUDE.md
                // (UB-class quirks are NOT reproduced — gate around them), abort
                // for anything but 3 phases rather than reproduce the over-read.
                // `do_dynamic_mode` calls `calc_dynamic` unconditionally (even on
                // the DynamicEq path), so there is no valid non-3-phase dynamics
                // path; this init-time abort sets `solution_abort`, which
                // `solve_dynamic_body` honors before any step runs.
                self.cd.obj.push_error_abort(format!(
                    "Dynamics mode requires a 3-phase WindGen (the WTG3 model is \
                     3-phase-only). WindGen.{} has {} phases.",
                    self.cd.obj.name(),
                    self.cd.nphases
                ));
                return;
            }
        }

        // DynamicEqObj <> NIL: seed the user equation's memory and exit.
        if self.dyneq.has_dynamic_eq() {
            for row in self.dyneq.dynamic_eq_vals.iter_mut() {
                row[1] = 0.0;
            }
            let num_pairs = self.dyneq.dynamic_eq_pair.len() / 2;
            for i in 0..num_pairs {
                let var_idx = self.dyneq.dynamic_eq_pair[i * 2] as usize;
                let code = self.dyneq.dynamic_eq_pair[i * 2 + 1];
                if !DynEqPceData::is_init_val(code) {
                    continue;
                }
                if code == 9 {
                    let val = cang(self.edp);
                    self.dyneq.dynamic_eq_vals[var_idx][0] = val;
                } else {
                    let val = self.get_pce_value(sys, node_v, code);
                    self.dyneq.dynamic_eq_vals[var_idx][0] = val;
                }
            }
            return;
        }

        // DynamicEqObj = NIL path — the classic shaft swing seed (vestigial for
        // WindGen: Theta/Speed are not readable variables, but they are still
        // integrated so the per-step `WindModelDyn.Integrate()` fires).
        self.theta = cang(self.edp);
        self.dtheta = 0.0;
        self.w0 = TWO_PI * sys.frequency;
        self.m_mass = 2.0 * self.h_mass * self.kva_rating * 1000.0 / self.w0; // M = W-sec
        self.d_damping = self.dpu * self.kva_rating * 1000.0 / self.w0; // Dpu = 0 → D = 0
        self.p_shaft = -self.terminal_power(sys, node_v, 1).re;
        self.speed = 0.0;
        self.dspeed = 0.0;

        // Seed the WTG3 model (uses the present terminal V/I; writes the initial
        // Norton current back into ITerminal for the first CalcDynamic to read).
        self.cd.compute_vterminal(node_v);
        let vterm = self.cd.vterminal.clone();
        self.wind_model_dyn.init(&vterm, &mut self.cd.iterminal);
    }

    /// Pascal `TWindGenObj.IntegrateStates` — advance the classic shaft swing by
    /// one trapezoidal half-step, then the WTG3 integrator.
    pub(super) fn integrate_states_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.compute_iterminal(sys, node_v);
        let h = sys.dyna_h;

        // DynamicEqObj <> NIL: integrate the user equation.
        if self.dyneq.has_dynamic_eq() {
            let out0 = self.dyneq.dyn_out[0];
            let out1 = self.dyneq.dyn_out[1];
            if sys.iteration_flag == IterationFlag::NewTimeStep {
                self.speed_history = self.dyneq.dynamic_eq_vals[out0][0]
                    + 0.5 * h * self.dyneq.dynamic_eq_vals[out0][1];
                self.theta_history = self.dyneq.dynamic_eq_vals[out1][0]
                    + 0.5 * h * self.dyneq.dynamic_eq_vals[out1][1];
            }
            let num_pairs = self.dyneq.dynamic_eq_pair.len() / 2;
            for i in 0..num_pairs {
                let var_idx = self.dyneq.dynamic_eq_pair[i * 2] as usize;
                let code = self.dyneq.dynamic_eq_pair[i * 2 + 1];
                if DynEqPceData::is_init_val(code) {
                    continue;
                }
                let val = match code {
                    0 => {
                        -terminal_power_in(&self.cd.vterminal, &self.cd.iterminal, self.cd.nphases)
                            .re
                    }
                    1 => {
                        -terminal_power_in(&self.cd.vterminal, &self.cd.iterminal, self.cd.nphases)
                            .im
                    }
                    _ => self.get_pce_value(sys, node_v, code),
                };
                self.dyneq.dynamic_eq_vals[var_idx][0] = val;
            }
            self.dyneq.solve_eq();
            self.speed = self.speed_history + 0.5 * h * self.dyneq.dynamic_eq_vals[out0][1];
            self.theta = self.theta_history + 0.5 * h * self.dyneq.dynamic_eq_vals[out1][1];
            self.dyneq.dynamic_eq_vals[out0][0] = self.speed;
            self.dyneq.dynamic_eq_vals[out1][0] = self.theta;
            return;
        }

        // DynamicEqObj = NIL path.
        if sys.iteration_flag == IterationFlag::NewTimeStep {
            self.theta_history = self.theta + 0.5 * h * self.dtheta;
            self.speed_history = self.speed + 0.5 * h * self.dspeed;
        }

        let trace_power =
            terminal_power_in(&self.cd.vterminal, &self.cd.iterminal, self.cd.nphases);
        self.dspeed = (self.p_shaft + trace_power.re - self.d_damping * self.speed) / self.m_mass;
        self.dtheta = self.speed;

        self.speed = self.speed_history + 0.5 * h * self.dspeed;
        self.theta = self.theta_history + 0.5 * h * self.dtheta;

        // Advance the WTG3 integrator (the upstream per-step side effect).
        self.wind_model_dyn.integrate();
    }

    /// Pascal `TWindGenObj.DoDynamicMode` — hand the WTG3 model the terminal V/I,
    /// read back its Norton injection.
    pub(super) fn do_dynamic_mode(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        self.cd.compute_vterminal(node_v);

        // The WTG3 model is 3-phase-only: `calc_dynamic` → `instrumentation`
        // reads V[1..3]/i[1..3], so a non-3-phase terminal (e.g. a 1-phase
        // WindGen's 2 conductors) would over-read the terminal array = heap UB
        // upstream (WindGen.pas:1868/1905). Per CLAUDE.md that UB is NOT
        // reproduced. `init_state_vars` already aborts the dynamics entry, but
        // an external `solve` clears `solution_abort` (Text_Set_Command reset)
        // before the step loop, so guard the per-step path too — record the
        // error and inject nothing (mirrors Generator `do_dynamic_mode`).
        if self.cd.nphases != 3 {
            errors.push(format!(
                "Dynamics mode requires a 3-phase WindGen (the WTG3 model is \
                 3-phase-only). WindGen.{} has {} phases.",
                self.cd.obj.name(),
                self.cd.nphases
            ));
            for c in self.cd.inj_current.iter_mut() {
                *c = Complex64::ZERO;
            }
            return;
        }

        let vterm = self.cd.vterminal.clone();
        self.wind_model_dyn.calc_dynamic(
            &vterm,
            &mut self.cd.iterminal,
            sys.dyna_h,
            sys.dyna_t,
            sys.iteration_flag,
        );

        self.cd.iterminal_updated = true;
        self.cd.iterminal_solution_count = sys.solution_count;

        // Inj = -Itotal (in). Direct assignment (Pascal replaces, no sum).
        let nconds = self.cd.nconds;
        for i in 0..nconds {
            self.cd.inj_current[i] = -self.cd.iterminal[i];
        }
    }

    /// Pascal `TDSSCktElement.Get_PCE_Value(1, ValType)` — the model-derived value
    /// a `DynamicExp` operand refers to, at terminal 1.
    fn get_pce_value(&mut self, sys: &SysCtx, node_v: &[Complex64], code: i32) -> f64 {
        match code {
            0 | 7 => -self.terminal_power(sys, node_v, 1).re, // P, P0
            1 | 8 => -self.terminal_power(sys, node_v, 1).im, // Q, Q0
            6 => self.terminal_power(sys, node_v, 1).norm(),  // S
            2..=5 => {
                self.compute_iterminal(sys, node_v);
                let mut max_curr = 0.0_f64;
                let mut max_phase = 0usize;
                for i in 0..self.cd.nphases {
                    let mag = self.cd.iterminal[i].norm();
                    if mag > max_curr {
                        max_curr = mag;
                        max_phase = i;
                    }
                }
                match code {
                    2 => node_v[self.cd.node_ref[max_phase]].norm(),
                    3 => cang(node_v[self.cd.node_ref[max_phase]]),
                    4 => max_curr,
                    _ => cang(self.cd.iterminal[max_phase]),
                }
            }
            _ => 0.0,
        }
    }

    /// Pascal `TWindGenObj.NumVariables` classic path (`= NumWGenVariables`).
    pub(super) fn num_wgen_variables(&self) -> usize {
        22
    }

    /// Pascal `TWindGenObj.VariableName(i)` (1-based, the 22 classic names, from
    /// `GetEnumName(VarInfo, i)`).
    pub(super) fn wgen_variable_name(&self, i: usize) -> String {
        match i {
            1 => "userTrip",
            2 => "wtgTrip",
            3 => "Pcurtail",
            4 => "Pcmd",
            5 => "Pgen",
            6 => "Qcmd",
            7 => "Qgen",
            8 => "Vref",
            9 => "Vmag",
            10 => "vwind",
            11 => "WtRef",
            12 => "WtAct",
            13 => "dOmg",
            14 => "dFrqPuTest",
            15 => "QMode",
            16 => "Qref",
            17 => "PFref",
            18 => "thetaPitch",
            19 => "Pg",
            20 => "Ps",
            21 => "Pr",
            22 => "s",
            _ => "ERROR",
        }
        .to_string()
    }

    /// Pascal `TWindGenObj.GetVariable(i)` (1-based classic path).
    pub(super) fn get_wgen_variable(&self, i: usize) -> f64 {
        let w = &self.wind_model_dyn;
        match i {
            1 => w.user_trip as f64,
            2 => w.wtg_trip as f64,
            3 => w.pcurtail,
            4 => w.pcmd,
            5 => w.pgen,
            6 => w.qcmd,
            7 => w.qgen,
            8 => w.vref,
            9 => w.vmag,
            10 => w.vwind,
            11 => w.wt_ref,
            12 => w.wt,
            13 => w.d_omg,
            14 => w.d_frq_pu_test,
            15 => w.q_mode as f64,
            16 => w.qref,
            17 => w.pf_ref,
            18 => w.theta_pitch,
            19 => self.pg,
            20 => self.ps,
            21 => self.pr,
            22 => self.s,
            _ => 0.0,
        }
    }

    /// Pascal `TWindGenObj.GetAllVariables` classic path.
    pub(super) fn get_wgen_variables(&self, states: &mut [f64]) {
        for (i, s) in states.iter_mut().enumerate().take(22) {
            *s = self.get_wgen_variable(i + 1);
        }
    }

    /// Pascal `TWindGenObj.SetVariable(i, Value)` (classic subset).
    pub(super) fn set_wgen_variable(&mut self, i: usize, value: f64) {
        let w = &mut self.wind_model_dyn;
        match i {
            1 => w.user_trip = value.round() as i32,
            3 => w.pcurtail = value,
            10 => w.vwind = value,
            14 => w.d_frq_pu_test = value,
            15 => w.q_mode = value.round() as i32,
            16 => w.qref = value,
            17 => w.pf_ref = value,
            19 => self.pg = value,
            20 => self.ps = value,
            21 => self.pr = value,
            22 => self.s = value,
            _ => {}
        }
    }
}
