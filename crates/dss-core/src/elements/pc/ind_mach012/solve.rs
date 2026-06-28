//! Solve-time electrical machinery: `CalcYPrimMatrix`, the equivalent-circuit
//! power-flow (`Get_PFlowModelCurrent` + slip-Newton) and dynamic
//! (`Get_DynamicModelCurrent`) current models in symmetrical components, the
//! harmonic injection, and the injection-current assembly.

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::solution::SolveMode;
use crate::support::cmatrix::CMatrix;
use crate::support::complexutil::rotate_phasor_deg;
use crate::support::mathutil::SymComp;
use crate::util::EPSILON;

use super::{Connection, IndMach012};

impl IndMach012 {
    /// Pascal `TIndMach012Obj.CalcYPrimMatrix`. Dynamics/harmonics use the
    /// constant equivalent `Yeq`; power flow uses the vars-only `Yeq` (`-j/ZBase`).
    /// A **wye** induction machine has no neutral, so the wye branch stamps only
    /// the diagonal.
    pub(super) fn calc_yprim_matrix(&mut self, ymatrix: &mut CMatrix, sys: &SysCtx) {
        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        if sys.is_dynamic_model || sys.is_harmonic_model {
            // Constant equivalent Y (the L-N value from InitStateVars/InitHarmonics).
            let mut y = if self.machine_on {
                self.yeq
            } else {
                Complex64::new(EPSILON, 0.0)
            };
            if self.connection == Connection::Delta {
                y /= 3.0; // convert to delta impedance
            }
            y.im /= freq_multiplier;
            let yij = -y;
            for i in 0..nphases {
                match self.connection {
                    Connection::Wye => ymatrix.set(i, i, y),
                    Connection::Delta => {
                        // Pascal floating-delta trick: a little extra on the
                        // diagonal to prevent a singular delta.
                        let yadder = y * 1.000001;
                        ymatrix.set(i, i, y + yadder);
                        ymatrix.add(i, i, y); // put it in again
                        for j in 0..i {
                            ymatrix.set(i, j, yij);
                            ymatrix.set(j, i, yij);
                        }
                    }
                }
            }
            return;
        }

        // Regular power-flow model: Yeq is L-N (vars only).
        let mut y = self.yeq;
        y.im /= freq_multiplier;
        match self.connection {
            Connection::Wye => {
                // Induction-machine wye has no neutral: diagonal only.
                for i in 0..nphases {
                    ymatrix.set(i, i, y);
                }
            }
            Connection::Delta => {
                let y = y / 3.0; // convert to delta impedance
                let yij = -y;
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 }; // wrap around
                    ymatrix.add(i, i, y);
                    ymatrix.add(j, j, y);
                    ymatrix.add_sym(i, j, yij);
                }
            }
        }
    }

    /// Pascal `TIndMach012Obj.CalcYPrim`.
    pub(super) fn calc_yprim_impl(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let mut yp_shunt = CMatrix::new(yorder);
        self.calc_yprim_matrix(&mut yp_shunt, sys);

        let mut yp_series = CMatrix::new(yorder);
        for i in 0..yorder {
            yp_series.set(i, i, yp_shunt.get(i, i) * 1.0e-10);
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_shunt);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim = Some(yprim);
        self.cd.inj_current = vec![Complex64::ZERO; yorder];

        self.cd.apply_yprim_open_conductor_calcs();
    }

    /// Pascal `CalcYPrimContribution`: `InjCurrent = Yprim · V(node)`.
    pub(super) fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
        }
    }

    /// Pascal `Get_PFlowModelCurrent(V, S)` → (Istator, Irotor) from the
    /// equivalent circuit.
    pub(super) fn get_pflow_model_current(&self, v: Complex64, s: f64) -> (Complex64, Complex64) {
        let rl = if s != 0.0 {
            self.zr.re * (1.0 - s) / s
        } else {
            self.zr.re * 1.0e6
        };
        let z_rotor = self.zr + Complex64::new(rl, 0.0);
        let numerator = self.zm * z_rotor;
        let z_motor = self.zs + numerator / (z_rotor + self.zm);
        let istator = v / z_motor;
        let irotor = istator - (v - self.zs * istator) / self.zm;
        (istator, irotor)
    }

    /// Pascal `Get_DynamicModelCurrent`: stator/rotor currents from the internal
    /// voltages `E1`/`E2` behind `Zsp`.
    fn get_dynamic_model_current(&mut self) {
        self.is1 = (self.v1 - self.e1) / self.zsp; // I = (V-E')/Z'
        self.is2 = (self.v2 - self.e2) / self.zsp;
        // Rotor current Ir = Is - Vm/jXm.
        self.ir1 = self.is1 - (self.v1 - self.is1 * self.zsp) / self.zm;
        self.ir2 = self.is2 - (self.v2 - self.is2 * self.zsp) / self.zm;
    }

    /// Pascal `TIndMach012Obj.CalcPFlow`: the slip-Newton power-flow current.
    fn calc_pflow(&mut self, v012: &[Complex64; 3], i012: &mut [Complex64; 3]) {
        self.v1 = v012[1];
        self.v2 = v012[2];
        self.in_dynamics = false;

        if self.first_iteration {
            let (is1, ir1) = self.get_pflow_model_current(self.v1, self.s1); // init Is1
            self.is1 = is1;
            self.ir1 = ir1;
            self.first_iteration = false;
        }

        // If FixedSlip is set, use the user value; else step the slip.
        if !self.fixed_slip {
            let p_error = self.p_nominal_per_phase - (self.v1 * self.is1.conj()).re;
            self.set_local_slip(self.s1 + self.dsdp * p_error); // new slip guess
        }

        let (is1, ir1) = self.get_pflow_model_current(self.v1, self.s1);
        self.is1 = is1;
        self.ir1 = ir1;
        let (is2, ir2) = self.get_pflow_model_current(self.v2, self.s2);
        self.is2 = is2;
        self.ir2 = ir2;

        i012[1] = self.is1;
        i012[2] = self.is2;
        i012[0] = Complex64::ZERO;
    }

    /// Pascal `TIndMach012Obj.CalcDynamic`: slip from the shaft speed, then the
    /// dynamic-model current.
    fn calc_dynamic(&mut self, v012: &[Complex64; 3], i012: &mut [Complex64; 3]) {
        self.in_dynamics = true;
        self.v1 = v012[1];
        self.v2 = v012[2];

        self.set_local_slip(-self.speed / self.w0); // slip from shaft speed
        self.get_dynamic_model_current();

        i012[1] = self.is1;
        i012[2] = self.is2;
        i012[0] = Complex64::ZERO;
    }

    /// Pascal `TIndMach012Obj.CalcModel`: ABC → 012, the per-mode model, 012 → ABC.
    /// The symmetrical-component conversion assumes a 3-phase machine (the only
    /// dynamics-supported topology besides 1-phase); fewer/extra conductors are
    /// zero-padded into the 3-element symmetrical-component arrays.
    fn calc_model(&mut self, sys: &SysCtx) {
        let sc = SymComp::default();
        let yorder = self.cd.yorder;

        // Pascal `Phase2SymComp` always reads 3 elements; for a 1-/2-phase machine
        // it reads past `Vterminal` (upstream UB). Zero-pad instead — well-defined
        // and unreachable in the corpus (3-phase default; dynamics is 1-/3-phase
        // only, mirroring the documented 1-phase guard in `init_state_vars_impl`).
        let mut vph = [Complex64::ZERO; 3];
        for (i, v) in vph.iter_mut().enumerate() {
            if i < yorder {
                *v = self.cd.vterminal[i];
            }
        }
        let mut v012 = [Complex64::ZERO; 3];
        sc.phase_to_sym(&vph, &mut v012);

        let mut i012 = [Complex64::ZERO; 3];
        if sys.mode == SolveMode::Dynamic {
            self.calc_dynamic(&v012, &mut i012);
        } else {
            self.calc_pflow(&v012, &mut i012);
        }

        let mut iph = [Complex64::ZERO; 3];
        sc.sym_to_phase(&i012, &mut iph);
        for (i, &c) in iph.iter().enumerate() {
            if i < yorder {
                self.cd.iterminal[i] = c;
            }
        }
    }

    /// Pascal `TIndMach012Obj.DoIndMach012Model` / `DoDynamicMode` (identical
    /// bodies bar the debug trace): `Inj = Yprim·V − Iterminal`.
    fn do_indmach_model(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v); // init InjCurrent
        self.calc_model(sys);
        // Pascal `IterminalUpdated := TRUE` goes through the `TPCElement`
        // property setter `set_ITerminalUpdated`, which also stamps
        // `IterminalSolutionCount := SolutionCount`. That stamp is what lets a
        // later `ComputeIterminal`/`GetCurrents` in the same solution reuse the
        // cached terminal current instead of recomputing the model — and for
        // IndMach012 the recompute is *stateful*: `CalcPFlow` advances the
        // fixed-slope slip-Newton by one step. Without the stamp the post-solve
        // power/current read fired a spurious extra slip step, landing the motor
        // one step past the oracle's operating point (~5e-4 in P). Mirror the
        // Generator/Load/PVSystem/Storage `put_curr` bookkeeping exactly.
        self.cd.iterminal_updated = true;
        self.cd.iterminal_solution_count = sys.solution_count;
        for i in 0..self.cd.nphases {
            self.cd.inj_current[i] -= self.cd.iterminal[i];
        }
        // NOT_PORTED: DebugTrace record.
    }

    /// Pascal `TIndMach012Obj.DoHarmonicMode`. The spectrum-scaled internal
    /// voltage `E` is **commented out upstream** (left at 0), so the injection is
    /// `Yprim·buffer` with `buffer = 0` (plus the wye no-injection-neutral term).
    /// Ported verbatim, including that quirk.
    fn do_harmonic_mode(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        // Pascal: E stays 0 (the spectrum lines are commented out upstream).
        let mut e = Complex64::ZERO;
        let gen_harmonic = sys.frequency / self.cd.base_frequency;

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        let mut buffer = vec![Complex64::ZERO; nconds];
        for (i, slot) in buffer.iter_mut().enumerate().take(nphases) {
            *slot = e;
            if i < nphases - 1 {
                e = rotate_phasor_deg(e, gen_harmonic, -120.0); // assume 3-phase
            }
        }
        // Wye: assume no neutral injection voltage.
        if self.connection == Connection::Wye {
            buffer[nconds - 1] = self.cd.vterminal[nconds - 1];
        }

        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &buffer);
        }
    }

    /// Pascal `TIndMach012Obj.CalcIndMach012ModelContribution`.
    pub(super) fn calc_model_contribution(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.iterminal_updated = false;
        if sys.is_dynamic_model {
            self.do_indmach_model(sys, node_v); // DoDynamicMode body == DoIndMach012Model
        } else if sys.is_harmonic_model && sys.frequency != sys.fundamental {
            self.do_harmonic_mode(sys, node_v);
        } else {
            self.do_indmach_model(sys, node_v);
        }
    }

    /// Pascal `TIndMach012Obj.InjCurrents` body (the `CalcInjCurrentArray` half).
    pub(super) fn calc_inj_current_array(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        if self.ind_mach_switch_open {
            self.cd.zero_inj_current();
        } else {
            self.calc_model_contribution(sys, node_v);
        }
    }
}
