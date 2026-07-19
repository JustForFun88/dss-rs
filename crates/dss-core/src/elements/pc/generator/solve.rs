//! Solve-time electrical machinery: `CalcYPrimMatrix`, the six `DoXxxGen` model
//! currents (`DoConstantPQGen` .. `DoCurrentLimitedPQ`) and the injection /
//! terminal current assembly. The model-current signs are the reverse of the
//! load's because the generator pushes power into the node.

use num_complex::Complex64;

use crate::elements::traits::{CktElement, SysCtx};
use crate::support::cmatrix::CMatrix;
use crate::support::complexutil::{cang, rotate_phasor_deg, rotate_phasor_rad};
use crate::support::mathutil::SymComp;
use crate::util::{EPSILON, sqrt3};

use super::{Connection, Generator};

impl Generator {
    /// Pascal `CalcYPrimMatrix` (power-flow + harmonic/dynamic paths).
    pub(super) fn calc_yprim_matrix(&mut self, ymatrix: &mut CMatrix, sys: &SysCtx) {
        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        if sys.is_dynamic_model || sys.is_harmonic_model {
            // Harmonic/dynamic YPrim (Pascal `CalcYPrimMatrix` l.1280:
            // `IsDynamicModel or IsHarmonicModel`): `Y := Yeq` (the L-N admittance
            // set in `InitHarmonics`/`InitStateVars`), `EPSILON` if the generator
            // is off; positive (not negated like the power-flow path).
            let mut y = if self.gen_on {
                self.yeq
            } else {
                Complex64::new(EPSILON, 0.0)
            };
            if self.connection == Connection::Delta {
                y /= 3.0; // convert to delta impedance
            }
            y.im /= freq_multiplier;
            let yij = -y;
            match self.connection {
                Connection::Wye => {
                    for i in 0..nphases {
                        ymatrix.set(i, i, y);
                        ymatrix.add(nconds - 1, nconds - 1, y);
                        ymatrix.set(i, nconds - 1, yij);
                        ymatrix.set(nconds - 1, i, yij);
                    }
                }
                Connection::Delta => {
                    for i in 0..nphases {
                        ymatrix.set(i, i, y);
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

        // Regular power-flow generator model: Yeq is L-N; negate for generation.
        let mut y = -self.yeq;
        if self.gen_model == 3 {
            y /= 100.0; // Type-3: only put 1% in Yprim
        }
        y.im /= freq_multiplier;

        match self.connection {
            Connection::Wye => {
                let yij = -y;
                for i in 0..nphases {
                    ymatrix.set(i, i, y);
                    ymatrix.add(nconds - 1, nconds - 1, y);
                    ymatrix.set(i, nconds - 1, yij);
                    ymatrix.set(nconds - 1, i, yij);
                }
            }
            Connection::Delta => {
                let y = y / 3.0; // convert to delta impedance
                let yij = -y;
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 };
                    ymatrix.add(i, i, y);
                    ymatrix.add(j, j, y);
                    ymatrix.add_sym(i, j, yij);
                }
            }
        }
    }

    /// Pascal `StickCurrInTerminalArray` (reverse of the load: signs switched).
    fn stick_curr(&mut self, into_iterminal: bool, curr: Complex64, i: usize) {
        let nconds = self.cd.nconds;
        let arr = if into_iterminal {
            &mut self.cd.iterminal
        } else {
            &mut self.cd.inj_current
        };
        match self.connection {
            Connection::Wye => {
                arr[i] += curr;
                arr[nconds - 1] -= curr; // neutral
            }
            Connection::Delta => {
                arr[i] += curr;
                let j = if i + 1 >= nconds { 0 } else { i + 1 };
                arr[j] -= curr;
            }
        }
    }

    /// Pascal `CalcVTerminalPhase`.
    fn calc_vterminal_phase(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        match self.connection {
            Connection::Wye => {
                for i in 0..nphases {
                    self.cd.vterminal[i] =
                        node_v[self.cd.node_ref[i]] - node_v[self.cd.node_ref[nconds - 1]];
                }
            }
            Connection::Delta => {
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 };
                    self.cd.vterminal[i] =
                        node_v[self.cd.node_ref[i]] - node_v[self.cd.node_ref[j]];
                }
            }
        }
        self.gen_solution_count = sys.solution_count;
    }

    /// Pascal `CalcYPrimContribution`: `InjCurrent = Yprim · V(node)`.
    pub(super) fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
        }
    }

    /// Pascal `DoConstantPQGen` (model 1).
    fn do_constant_pq_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.cd.zero_iterminal();
        self.calc_vterminal_phase(sys, node_v);

        let s = Complex64::new(self.p_nominal_per_phase, self.q_nominal_per_phase);
        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let mut curr = match self.connection {
                Connection::Wye => {
                    if vmag <= self.v_base95 {
                        self.yeq95 * v
                    } else if vmag > self.v_base105 {
                        self.yeq105 * v
                    } else {
                        (s / v).conj()
                    }
                }
                Connection::Delta => {
                    let vm = match self.cd.nphases {
                        2 | 3 => vmag / sqrt3(),
                        _ => vmag,
                    };
                    if vm <= self.v_base95 {
                        (self.yeq95 / 3.0) * v
                    } else if vm > self.v_base105 {
                        (self.yeq105 / 3.0) * v
                    } else {
                        (s / v).conj()
                    }
                }
            };
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoConstantZGen` (model 2).
    fn do_constant_z_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);
        self.cd.zero_iterminal();
        let yeq2 = match self.connection {
            Connection::Wye => self.yeq,
            Connection::Delta => self.yeq / 3.0,
        };
        for i in 0..self.cd.nphases {
            let mut curr = yeq2 * self.cd.vterminal[i];
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoPVTypeGen` (model 3: constant P, |V|).
    fn do_pv_type_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);
        self.cd.zero_iterminal();

        // Guess a new var output value.
        let mut v_avg = 0.0;
        for i in 0..self.cd.nphases {
            v_avg += self.cd.vterminal[i].norm();
        }
        let nphases = self.cd.nphases as f64;
        v_avg = match self.connection {
            Connection::Delta => v_avg / (sqrt3() * nphases),
            Connection::Wye => v_avg / nphases,
        };
        self.v_avg = v_avg;

        let mut dq = self.pv_factor * self.dqdv * (self.v_target - v_avg); // Vtarget is L-N
        if dq.abs() > self.delta_q_max {
            dq = if dq < 0.0 {
                -self.delta_q_max
            } else {
                self.delta_q_max
            };
        }
        self.q_nominal_per_phase += dq;
        if self.q_nominal_per_phase > self.var_max {
            self.q_nominal_per_phase = self.var_max;
        } else if self.q_nominal_per_phase < self.var_min {
            self.q_nominal_per_phase = self.var_min;
        }

        for i in 0..self.cd.nphases {
            let s = Complex64::new(self.p_nominal_per_phase, self.q_nominal_per_phase);
            let mut curr = (s / self.cd.vterminal[i]).conj();
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoFixedQGen` (model 4: constant P, fixed Q = kvarBase).
    fn do_fixed_q_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);
        self.cd.zero_iterminal();

        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let s = Complex64::new(self.p_nominal_per_phase, self.var_base);
            let mut curr = match self.connection {
                Connection::Wye => {
                    if vmag <= self.v_base95 {
                        Complex64::new(self.yeq95.re, self.yq_fixed) * v
                    } else if vmag > self.v_base105 {
                        Complex64::new(self.yeq105.re, self.yq_fixed) * v
                    } else {
                        (s / v).conj()
                    }
                }
                Connection::Delta => {
                    let vm = match self.cd.nphases {
                        2 | 3 => vmag / sqrt3(),
                        _ => vmag,
                    };
                    if vm <= self.v_base95 {
                        Complex64::new(self.yeq95.re / 3.0, self.yq_fixed / 3.0) * v
                    } else if vm > self.v_base105 {
                        Complex64::new(self.yeq105.re / 3.0, self.yq_fixed / 3.0) * v
                    } else {
                        (s / v).conj()
                    }
                }
            };
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoFixedQZGen` (model 5: constant P, fixed Q as a fixed Z).
    fn do_fixed_qz_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);
        self.cd.zero_iterminal();

        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let p = Complex64::new(self.p_nominal_per_phase, 0.0);
            let mut curr = match self.connection {
                Connection::Wye => {
                    if vmag <= self.v_base95 {
                        Complex64::new(self.yeq95.re, self.yq_fixed) * v
                    } else if vmag > self.v_base105 {
                        Complex64::new(self.yeq105.re, self.yq_fixed) * v
                    } else {
                        (p / v).conj() + Complex64::new(0.0, self.yq_fixed) * v
                    }
                }
                Connection::Delta => {
                    let vm = match self.cd.nphases {
                        2 | 3 => vmag / sqrt3(),
                        _ => vmag,
                    };
                    if vm <= self.v_base95 {
                        Complex64::new(self.yeq95.re / 3.0, self.yq_fixed / 3.0) * v
                    } else if vm > self.v_base105 {
                        Complex64::new(self.yeq105.re / 3.0, self.yq_fixed / 3.0) * v
                    } else {
                        (p / v).conj() + Complex64::new(0.0, self.yq_fixed / 3.0) * v
                    }
                }
            };
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoCurrentLimitedPQ` (model 7: PQ limited to max current below
    /// Vminpu).
    fn do_current_limited_pq(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);

        if self.force_balanced && self.cd.nphases == 3 {
            // Convert to pos-seq only.
            let sc = SymComp::default();
            let mut v012 = [Complex64::ZERO; 3];
            sc.phase_to_sym(&self.cd.vterminal[..3], &mut v012);
            v012[0] = Complex64::ZERO;
            v012[2] = Complex64::ZERO;
            let mut vph = [Complex64::ZERO; 3];
            sc.sym_to_phase(&v012, &mut vph);
            self.cd.vterminal[..3].copy_from_slice(&vph);
        }

        self.cd.zero_iterminal();
        let s = Complex64::new(self.p_nominal_per_phase, self.q_nominal_per_phase);
        for i in 0..self.cd.nphases {
            match self.connection {
                Connection::Wye => {
                    let vln = self.cd.vterminal[i];
                    let vmag_ln = vln.norm();
                    let mut phase_curr = (s / vln).conj();
                    if phase_curr.norm() > self.model7_max_phase_curr {
                        phase_curr = (self.phase_current_limit / (vln / vmag_ln)).conj();
                    }
                    // (Pascal omits the fuel check on the wye branch.)
                    self.put_curr(sys, phase_curr, i);
                }
                Connection::Delta => {
                    let vll = self.cd.vterminal[i];
                    let vmag_ll = vll.norm();
                    let mut delta_curr = match self.cd.nphases {
                        2 | 3 => {
                            let mut dc = (s / vll).conj();
                            if dc.norm() * sqrt3() > self.model7_max_phase_curr {
                                dc =
                                    (self.phase_current_limit / (vll / (vmag_ll / sqrt3()))).conj();
                            }
                            dc
                        }
                        _ => {
                            let mut dc = (s / vll).conj();
                            if dc.norm() > self.model7_max_phase_curr {
                                dc = (self.phase_current_limit / (vll / vmag_ll)).conj();
                            }
                            dc
                        }
                    };
                    if self.use_fuel && !self.gen_active {
                        delta_curr = Complex64::ZERO;
                    }
                    self.put_curr(sys, delta_curr, i);
                }
            }
        }
    }

    /// The shared tail of every `DoXxxGen`: terminal/injection bookkeeping.
    fn put_curr(&mut self, sys: &SysCtx, curr: Complex64, i: usize) {
        self.stick_curr(true, -curr, i); // into ITerminal
        self.cd.iterminal_updated = true;
        self.cd.iterminal_solution_count = sys.solution_count;
        self.stick_curr(false, curr, i); // into InjCurrent
    }

    /// Pascal `CalcGenModelContribution`.
    pub(super) fn calc_gen_model_contribution(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut crate::diag::ErrorLog,
    ) {
        self.cd.iterminal_updated = false;
        // Pascal `CalcGenModelContribution` dispatches `DoDynamicMode` first when
        // the solution is in dynamics mode (the generator behind Xd' as a voltage
        // source), before the harmonic check and the power-flow models.
        if sys.is_dynamic_model {
            self.do_dynamic_mode(sys, node_v, errors);
            return;
        }
        // Above the fundamental, harmonics mode injects the spectrum-scaled
        // Thevenin source through YPrim instead of the power-flow model (Pascal
        // `if IsHarmonicModel and (Frequency <> Fundamental) then DoHarmonicMode`).
        if sys.is_harmonic_model && sys.frequency != sys.fundamental {
            self.do_harmonic_mode(sys, node_v);
            return;
        }
        match self.gen_model {
            1 => self.do_constant_pq_gen(sys, node_v),
            2 => self.do_constant_z_gen(sys, node_v),
            3 => self.do_pv_type_gen(sys, node_v),
            4 => self.do_fixed_q_gen(sys, node_v),
            5 => self.do_fixed_qz_gen(sys, node_v),
            6 => self.do_user_model(sys, node_v, errors),
            7 => self.do_current_limited_pq(sys, node_v),
            _ => self.do_constant_pq_gen(sys, node_v),
        }
    }

    /// Pascal `TGeneratorObj.DoUserModel` (`generator.pas:1816-1835`): the
    /// power-flow terminal current from a `Model=User` (`GenModel=6`) WASM user
    /// model. Init `InjCurrent` from Yprim, run `UserModel.FCalc(Vterminal,
    /// Iterminal)`, and negate the returned terminal currents into `InjCurrent`.
    /// A missing model records #567 and falls back to Yprim only.
    pub(super) fn do_user_model(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut crate::diag::ErrorLog,
    ) {
        self.calc_yprim_contribution(node_v); // init InjCurrent + Vterminal
        if self.user_model_fcalc(sys, node_v, errors) {
            // Pascal `IterminalUpdated := TRUE` (the setter also stamps
            // `IterminalSolutionCount`, as in `DoDynamicMode`).
            self.cd.iterminal_updated = true;
            self.cd.iterminal_solution_count = sys.solution_count;
            let nconds = self.cd.nconds;
            for i in 0..nconds {
                self.cd.inj_current[i] -= self.cd.iterminal[i];
            }
        } else {
            errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "Generator.{} model designated to use user-written model, but user-written \
                     model is not defined.",
                    self.cd.obj.name()
                ),
                Some(567),
            ));
        }
    }

    /// Pascal `CalcInjCurrentArray`.
    pub(super) fn calc_inj_current_array(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut crate::diag::ErrorLog,
    ) {
        if self.gen_switch_open {
            self.cd.inj_current.fill(Complex64::ZERO);
        } else {
            self.calc_gen_model_contribution(sys, node_v, errors);
        }
    }

    /// Pascal `TGeneratorObj.InitHarmonics`: capture the Thevenin voltage behind
    /// Xd" from the present fundamental terminal current as the harmonic injection
    /// base. `Yeq` becomes the L-N subtransient admittance the harmonic
    /// `CalcYPrimMatrix` branch consumes.
    pub(super) fn init_harmonics_impl(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.yprim_invalid = true; // force YPrim rebuild
        self.gen_fundamental = sys.frequency; // whatever the frequency is on entry
        self.yeq = Complex64::new(0.0, self.xdpp).inv(); // L-N, used for current calcs

        if !self.gen_on {
            self.v_thev_harm = 0.0;
            self.theta_harm = 0.0;
            return;
        }
        self.compute_iterminal(sys, node_v); // present value of current
        let nconds = self.cd.nconds;
        let va = match self.connection {
            // wye — neutral is explicit
            Connection::Wye => node_v[self.cd.node_ref[0]] - node_v[self.cd.node_ref[nconds - 1]],
            // delta — assume neutral is at zero
            Connection::Delta => node_v[self.cd.node_ref[0]],
        };
        let e = va - self.cd.iterminal[0] * Complex64::new(0.0, self.xdpp);
        self.v_thev_harm = e.norm(); // base mag
        self.theta_harm = cang(e); // base angle (radians)
    }

    /// Pascal `TGeneratorObj.DoHarmonicMode`: the generator as a voltage source
    /// behind Xd" — the spectrum-scaled, phase-rotated Thevenin voltage pushed
    /// through YPrim to the injection current. `IterminalUpdated` stays false (the
    /// terminal current is derived from the network in `GetTerminalCurrents`).
    fn do_harmonic_mode(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let gen_harmonic = sys.frequency / self.gen_fundamental;
        let mult = self
            .spectrum_obj
            .as_ref()
            .map(|s| s.get_mult(gen_harmonic))
            .unwrap_or(Complex64::ZERO);
        let mut e = mult * self.v_thev_harm; // base harmonic magnitude
        e = rotate_phasor_rad(e, gen_harmonic, self.theta_harm); // fundamental phase shift

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        let mut buffer = vec![Complex64::ZERO; nconds];
        for (i, slot) in buffer.iter_mut().enumerate().take(nphases) {
            *slot = e;
            if i < nphases - 1 {
                e = rotate_phasor_deg(e, gen_harmonic, -120.0); // assume 3-phase
            }
        }
        // Handle wye connection: assume no neutral injection voltage.
        if self.connection == Connection::Wye {
            buffer[nconds - 1] = self.cd.vterminal[nconds - 1];
        }

        // InjCurrent = YPrim · buffer.
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &buffer);
        }
    }
}
