//! Dynamics-mode machinery: `InitStateVars`, `IntegrateStates`, `DoDynamicMode`
//! and the state-variable interface (`NumVariables`/`VariableName`/
//! `GetAllVariables`). The generator behind Xd' acts as a voltage source whose
//! magnitude/angle (the Thevenin `Vthev`) is driven by the shaft swing equation,
//! integrated by the trapezoidal predictor/corrector in `SolveDynamic`.
//!
//! Scope: the built-in shaft model (`DynamicEqObj = NIL`) and the external
//! `DynamicExp` integration (`DynamicEqObj <> NIL`, WP7.7 step 3b). The
//! user-written `UserModel`/`ShaftModel` DLLs are NOT_PORTED (never). The
//! synchronous Generator has no grid-forming mode — GFM is an inverter-based
//! (PVSystem/Storage) feature (`generator.pas` carries no GFM code, WPG.13).

use num_complex::Complex64;

use crate::elements::pc::dyneq_pce::DynEqPceData;
use crate::elements::traits::{CktElement, SysCtx};
use crate::support::complexutil::{cang, pclx};
use crate::support::dynamics::IterationFlag;
use crate::support::mathutil::{SymComp, terminal_power_in};

use super::{Connection, Generator};

// Pascal `DSSGlobals.TwoPi = 2.0 * PI` and `RadiansToDegrees = 180.0 / PI` —
// both full precision in the vendored 0.14.5 source (the `57.2957795130823...`
// alternative on the line above is commented out there). NOT a TODO(compat):
// the truncated `57.29577951` lives only in `DSSUcomplex` (`cdang`/`pdeg`),
// which `Get_Variable` does not use.
const TWO_PI: f64 = 2.0 * std::f64::consts::PI;
const RADIANS_TO_DEGREES: f64 = 180.0 / std::f64::consts::PI;

impl Generator {
    /// Pascal `TGeneratorObj.InitStateVars` — seed the shaft/Thevenin state from
    /// the present (power-flow) operating point (both the built-in shaft model and
    /// the `DynamicEqObj <> NIL` user-equation seeding).
    pub(super) fn init_state_vars_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.yprim_invalid = true; // Force rebuild of YPrims

        // Zthev: inverter (model 7) uses Xd' as a pure resistance; machines use
        // Xd' with the assumed X/R. Yeq (L-N) overwrites the power-flow Yeq and
        // feeds the dynamic `CalcYPrimMatrix` `Y := Yeq` branch.
        self.zthev = if self.gen_model == 7 {
            Complex64::new(self.xdp, 0.0)
        } else {
            Complex64::new(self.xdp / self.xrdp, self.xdp)
        };
        self.yeq = self.zthev.inv();

        // Compute nominal positive-sequence voltage behind the transient reactance.
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
            1 => {
                let nr = &self.cd.node_ref;
                // Edp = V(term1) - V(term2) - I·Zthev
                let edp = node_v[nr[0]] - node_v[nr[1]] - self.cd.iterminal[0] * self.zthev;
                self.edp = edp;
                self.v_thev_mag = edp.norm();
            }
            3 => {
                // Edp from the positive sequence only.
                let sc = SymComp::default();
                let mut i012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&self.cd.iterminal[..3], &mut i012);
                let mut vabc = [Complex64::ZERO; 3];
                for (i, v) in vabc.iter_mut().enumerate() {
                    *v = node_v[self.cd.node_ref[i]]; // wye voltage
                }
                let mut v012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&vabc, &mut v012);
                let vxd = i012[1] * self.zthev; // voltage drop through machine
                let edp = v012[1] - vxd; // pos sequence
                self.edp = edp;
                self.v_thev_mag = edp.norm();
            }
            _ => {
                // Pascal sets DSS.SolutionAbort := TRUE (msg 5672) here, but
                // `init_state_vars` has no error channel in this port. The machine
                // is left zero-initialized, so `w0`/`m_mass` stay 0 and a following
                // `integrate_states` would divide by `m_mass = 0` (NaN) — worse than
                // Pascal's clean abort, but unreachable: the vendored dynamics corpus
                // is all 1-/3-phase, and `do_dynamic_mode` records the >3-phase error
                // at inject time. On-demand (retagged at the WP8.8 sweep): surface a
                // real abort (init_state_vars needs an error channel) if a >3-phase
                // dynamics case appears.
                return;
            }
        }

        // DynamicEqObj <> NIL: seed the user equation's memory and exit (the
        // shaft GenVars Theta/w0/Mmass/D/Pshaft/Speed are unused — the equation
        // drives Speed/Theta through DynOut). Pascal generator.pas l.2400.
        if self.dyneq.has_dynamic_eq() {
            // Initialize the derivative column of every variable to 0.
            for row in self.dyneq.dynamic_eq_vals.iter_mut() {
                row[1] = 0.0;
            }
            // Apply initializations that use calculated values (P0/Q0/edp).
            let num_pairs = self.dyneq.dynamic_eq_pair.len() / 2;
            for i in 0..num_pairs {
                let var_idx = self.dyneq.dynamic_eq_pair[i * 2] as usize;
                let code = self.dyneq.dynamic_eq_pair[i * 2 + 1];
                if !DynEqPceData::is_init_val(code) {
                    continue;
                }
                if code == 9 {
                    // edp: the angle of the pos-seq voltage behind Xd'.
                    let val = cang(self.edp);
                    self.dyneq.dynamic_eq_vals[var_idx][0] = val;
                    if self.gen_model == 7 {
                        self.model7_last_angle = val;
                    }
                } else {
                    let val = self.get_pce_value(sys, node_v, code);
                    self.dyneq.dynamic_eq_vals[var_idx][0] = val;
                }
            }
            return;
        }

        // DynamicEqObj = NIL path.
        // Theta is the angle of Edp relative to the system reference.
        self.theta = cang(self.edp);
        if self.gen_model == 7 {
            self.model7_last_angle = self.theta;
        }
        self.dtheta = 0.0;
        self.w0 = TWO_PI * sys.frequency;
        // Recalc Mmass and D in case the frequency has changed.
        self.m_mass = 2.0 * self.h_mass * self.kva_rating * 1000.0 / self.w0; // M = W-sec
        self.d_damping = self.dpu * self.kva_rating * 1000.0 / self.w0;
        // Initialize Pshaft to the present power output: -Power[1].re.
        self.p_shaft = -self.terminal_power(sys, node_v, 1).re;
        self.speed = 0.0; // relative to synchronous speed
        self.dspeed = 0.0;

        // NOT_PORTED: GenModel = 6 UserModel/ShaftModel FInit (user-written DLLs).
    }

    /// Pascal `TGeneratorObj.IntegrateStates` — advance the shaft state by one
    /// trapezoidal half-step (both the built-in shaft model and the
    /// `DynamicEqObj <> NIL` user-equation integration).
    pub(super) fn integrate_states_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        // Compute derivatives and then integrate.
        self.compute_iterminal(sys, node_v);

        let h = sys.dyna_h;

        // DynamicEqObj <> NIL: integrate the user equation (Pascal generator.pas
        // l.2483). DynOut[0] is the speed variable, DynOut[1] the angle.
        if self.dyneq.has_dynamic_eq() {
            let out0 = self.dyneq.dyn_out[0];
            let out1 = self.dyneq.dyn_out[1];
            if sys.iteration_flag == IterationFlag::NewTimeStep {
                // First iteration of a new time step.
                self.speed_history = self.dyneq.dynamic_eq_vals[out0][0]
                    + 0.5 * h * self.dyneq.dynamic_eq_vals[out0][1];
                self.theta_history = self.dyneq.dynamic_eq_vals[out1][0]
                    + 0.5 * h * self.dyneq.dynamic_eq_vals[out1][1];
            }

            // Load calculated values (P/Q/VMag/...) that are not initializations.
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

            // Solve the differential equation with the loaded values.
            self.dyneq.solve_eq();

            // Trapezoidal method — write the results back into GenVars so the
            // injection (DoDynamicMode → CalcVthev_Dyn) reads the new angle.
            self.speed = self.speed_history + 0.5 * h * self.dyneq.dynamic_eq_vals[out0][1];
            self.theta = self.theta_history + 0.5 * h * self.dyneq.dynamic_eq_vals[out1][1];

            // Save the integrated values back into the memory space.
            self.dyneq.dynamic_eq_vals[out0][0] = self.speed;
            self.dyneq.dynamic_eq_vals[out1][0] = self.theta;
            return;
        }

        // DynamicEqObj = NIL path.
        if sys.iteration_flag == IterationFlag::NewTimeStep {
            // First iteration of a new time step.
            self.theta_history = self.theta + 0.5 * h * self.dtheta;
            self.speed_history = self.speed + 0.5 * h * self.dspeed;
        }

        // Compute shaft dynamics.
        let trace_power =
            terminal_power_in(&self.cd.vterminal, &self.cd.iterminal, self.cd.nphases);
        self.dspeed = (self.p_shaft + trace_power.re - self.d_damping * self.speed) / self.m_mass;
        self.dtheta = self.speed;

        // Trapezoidal method.
        self.speed = self.speed_history + 0.5 * h * self.dspeed;
        self.theta = self.theta_history + 0.5 * h * self.dtheta;

        // NOT_PORTED: DebugTrace trace record; GenModel = 6 UserModel/ShaftModel
        // Integrate (user-written DLLs).
    }

    /// Pascal `TGeneratorObj.DoDynamicMode` — total dynamic current into the
    /// injection array (the generator as a voltage source behind Zthev).
    pub(super) fn do_dynamic_mode(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut crate::diag::ErrorLog,
    ) {
        // Init InjCurrent array and compute VTerminal (L-N). Inj = -Itotal - Yprim·Vtemp.
        self.calc_yprim_contribution(node_v);

        if self.gen_model == 6 {
            // NOT_PORTED: user-written dynamics model DLL. Pascal sets
            // `DSS.SolutionAbort := TRUE` (msg 5671); this records the error
            // best-effort, exactly like the model-6 *power-flow* path
            // (`calc_gen_model_contribution`), but the generator's `inj_currents`
            // drops its local `errors` vec, so the abort is not surfaced. A bare
            // `Model=6` generator in dynamics is unreachable in the vendored corpus
            // (no UserModel can be configured — the prop is NOT_PORTED). On-demand
            // (retagged at the WP8.8 sweep): surface as a loud abort if a corpus
            // case ever needs it.
            errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "{}.{} model designated to use user-written dynamics model, but \
                     user-written model is not defined.",
                    "Generator",
                    self.cd.obj.name()
                ),
                // Pascal `DoSimpleMsg('Dynamics model missing for %s ', 5671)`
                // (generator.pas:1904).
                Some(5671),
            ));
        } else {
            let sc = SymComp::default();
            match self.cd.nphases {
                1 => {
                    // 1-phase generators have 2 conductors.
                    if self.gen_model == 7 {
                        // Simple inverter model: stay in phase with the terminal V.
                        let v = self.cd.vterminal[0] - self.cd.vterminal[1];
                        self.calc_vthev_dyn_mod7(v);
                    } else {
                        self.calc_vthev_dyn(); // update for the latest phase angle
                    }

                    // ZThev is based on Xd'.
                    let mut it =
                        (self.cd.vterminal[0] - self.vthev - self.cd.vterminal[1]) / self.zthev;
                    if self.gen_model == 7 && it.norm() > self.model7_max_phase_curr {
                        // Limit the current but keep the phase angle.
                        it = pclx(self.model7_max_phase_curr, cang(it));
                    }
                    self.cd.iterminal[0] = it;
                    self.cd.iterminal[1] = -it;
                }
                3 => {
                    let mut v012 = [Complex64::ZERO; 3];
                    sc.phase_to_sym(&self.cd.vterminal[..3], &mut v012);

                    let mut i012 = [Complex64::ZERO; 3];
                    if self.gen_model == 7 {
                        // Simple inverter model: stay in phase with the pos-seq
                        // voltage and limit the pos-seq current.
                        self.calc_vthev_dyn_mod7(v012[1]);
                        let mut i1 = (v012[1] - self.vthev) / self.zthev; // ZThev based on Xd'
                        if i1.norm() > self.model7_max_phase_curr {
                            // Limit the current but keep the phase angle.
                            i1 = pclx(self.model7_max_phase_curr, cang(i1));
                        }
                        i012[1] = i1;
                        // Negative-sequence current.
                        i012[2] = if self.force_balanced {
                            Complex64::ZERO
                        } else {
                            v012[2] / self.zthev // for inverter ZThev is (Xd' + j0)
                        };
                    } else {
                        self.calc_vthev_dyn(); // update for the latest phase angle
                        i012[1] = (v012[1] - self.vthev) / self.zthev; // ZThev based on Xd'
                        i012[2] = v012[2] / Complex64::new(0.0, self.xdpp); // machine uses Xd"
                    }

                    // Adjust for the generator connection (zero sequence).
                    i012[0] = if self.connection == Connection::Delta || self.force_balanced {
                        Complex64::ZERO
                    } else {
                        v012[0] / Complex64::new(0.0, self.xdpp)
                    };

                    let mut iph = [Complex64::ZERO; 3];
                    sc.sym_to_phase(&i012, &mut iph); // back to phase components
                    self.cd.iterminal[..3].copy_from_slice(&iph);

                    // Neutral current.
                    if self.connection == Connection::Wye {
                        let nconds = self.cd.nconds;
                        self.cd.iterminal[nconds - 1] = -i012[0] * 3.0;
                    }
                }
                _ => {
                    // Pascal sets DSS.SolutionAbort := TRUE (msg 5671,
                    // generator.pas:1984 — this is the phases-else of
                    // `DoDynamicMode`; the identical-text message at
                    // generator.pas:2357 belongs to a *different* procedure,
                    // `InitStateVars`, and carries the separate code 5672
                    // (ported at `init_state_vars_impl`).
                    errors.push(crate::diag::DssDiagnostic::msg(
                        format!(
                            "Dynamics mode is implemented only for 1- or 3-phase Generators. \
                             {}.{} has {} phases.",
                            "Generator",
                            self.cd.obj.name(),
                            self.cd.nphases
                        ),
                        Some(5671),
                    ));
                }
            }
        }

        // Pascal `TGeneratorObj.DoDynamicMode` ends with `IterminalUpdated := TRUE`
        // (generator.pas:1990), which through the `TPCElement` property setter also
        // stamps `IterminalSolutionCount := SolutionCount`, so a post-solve
        // `ComputeIterminal`/`GetCurrents` reuses the cached terminal current instead
        // of recomputing the model (mirrors `put_curr` and Storage/PVSystem dynamics).
        self.cd.iterminal_updated = true;
        self.cd.iterminal_solution_count = sys.solution_count;

        // Add it into the inj current array.
        let nconds = self.cd.nconds;
        for i in 0..nconds {
            self.cd.inj_current[i] -= self.cd.iterminal[i];
        }

        // NOT_PORTED: GenModel = 6 ShaftModel FCalc (mech power to shaft).
    }

    /// Pascal `TDSSCktElement.Get_PCE_Value(1, ValType)` (CktElement.pas l.828):
    /// the model-derived value a `DynamicExp` operand refers to (P/Q/Vmag/.../S),
    /// at the active terminal (terminal 1). Generators are not transformers, so the
    /// `MaxVoltage` branch reads the node voltage at the max-current phase directly.
    fn get_pce_value(&mut self, sys: &SysCtx, node_v: &[Complex64], code: i32) -> f64 {
        match code {
            0 | 7 => -self.terminal_power(sys, node_v, 1).re, // P, P0
            1 | 8 => -self.terminal_power(sys, node_v, 1).im, // Q, Q0
            6 => self.terminal_power(sys, node_v, 1).norm(),  // S
            2..=5 => {
                // VMag / VAng / IMag / IAng — the phase carrying the max current.
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

    /// Pascal `TGeneratorObj.CalcVthev_Dyn`: `Vthev = pclx(VthevMag, Theta)`.
    fn calc_vthev_dyn(&mut self) {
        if self.gen_switch_open {
            self.v_thev_mag = 0.0;
        }
        self.vthev = pclx(self.v_thev_mag, self.theta);
    }

    /// Pascal `TGeneratorObj.CalcVthev_Dyn_Mod7(V)`: adjust Vthev to be in phase
    /// with `V` when possible (PLL-like angle hold below ~20% of Vbase). For
    /// Phases = 1, Vbase is the terminal-to-terminal voltage; else it is L-N.
    fn calc_vthev_dyn_mod7(&mut self, v: Complex64) {
        if self.gen_switch_open {
            self.v_thev_mag = 0.0;
        }
        let angle = if v.norm() > 0.2 * self.v_base {
            cang(v)
        } else {
            self.model7_last_angle
        };
        self.vthev = pclx(self.v_thev_mag, angle);
        self.model7_last_angle = angle;
    }

    /// Pascal `TGeneratorObj.NumVariables` (`= NumGenVariables`, the classic
    /// path; inherited DynamicExp `NumVariables` returns 0 here, and the
    /// UserModel/ShaftModel contributions are NOT_PORTED).
    pub(super) fn num_gen_variables(&self) -> usize {
        6
    }

    /// Pascal `TGeneratorObj.VariableName(i)` (1-based, the 6 classic names).
    /// Pascal seeds `Result := 'ERROR'` and returns it for any out-of-range index
    /// (the monitor header only ever asks for `1..=NumVariables`, so this is the
    /// unreachable guard value, matched here for fidelity).
    pub(super) fn gen_variable_name(&self, i: usize) -> String {
        match i {
            1 => "Frequency",
            2 => "Theta (Deg)",
            3 => "Vd",
            4 => "PShaft",
            5 => "dSpeed (Deg/sec)",
            6 => "dTheta (Deg)",
            _ => "ERROR",
        }
        .to_string()
    }

    /// Pascal `TGeneratorObj.Get_Variable` for the 6 classic GenVars, filled into
    /// `states[0..6]` (the `GetAllVariables` loop). The `DynamicEqObj` memory dump is
    /// handled by the `get_all_variables` accessor short-circuit; UserModel/ShaftModel
    /// variables are NOT_PORTED.
    pub(super) fn get_gen_variables(&mut self, states: &mut [f64]) {
        states[0] = (self.w0 + self.speed) / TWO_PI; // Frequency, Hz
        states[1] = self.theta * RADIANS_TO_DEGREES; // Theta, deg
        states[2] = self.vthev.norm() / self.v_base; // Vd, pu
        states[3] = self.p_shaft; // PShaft
        states[4] = self.dspeed * RADIANS_TO_DEGREES; // dSpeed, deg/sec
        states[5] = self.dtheta; // dTheta
    }
}
