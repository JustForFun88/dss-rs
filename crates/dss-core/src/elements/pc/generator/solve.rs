//! Solve-time electrical machinery: `CalcYPrimMatrix`, the six `DoXxxGen` model
//! currents (`DoConstantPQGen` .. `DoCurrentLimitedPQ`) and the injection /
//! terminal current assembly. The model-current signs are the reverse of the
//! load's because the generator pushes power into the node.

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::support::cmatrix::CMatrix;
use crate::support::mathutil::SymComp;
use crate::util::sqrt3;

use super::{Connection, Generator};

impl Generator {
    /// Pascal `CalcYPrimMatrix` (power-flow path).
    pub(super) fn calc_yprim_matrix(&mut self, ymatrix: &mut CMatrix, sys: &SysCtx) {
        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        // Regular power-flow generator model: Yeq is L-N; negate for generation.
        let mut y = -self.yeq;
        if self.gen_model == 3 {
            y /= 100.0; // Type-3: only put 1% in Yprim
        }
        y.im /= freq_multiplier;

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
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
    fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
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
        errors: &mut Vec<String>,
    ) {
        self.cd.iterminal_updated = false;
        // Dynamics/harmonics models are Phase 7.
        match self.gen_model {
            1 => self.do_constant_pq_gen(sys, node_v),
            2 => self.do_constant_z_gen(sys, node_v),
            3 => self.do_pv_type_gen(sys, node_v),
            4 => self.do_fixed_q_gen(sys, node_v),
            5 => self.do_fixed_qz_gen(sys, node_v),
            6 => {
                // User-written model DLL — never ported. Pascal inits InjCurrent
                // then records error 567.
                self.calc_yprim_contribution(node_v);
                errors.push(format!(
                    "{}.{} model designated to use user-written model, but user-written \
                     model is not defined.",
                    "Generator",
                    self.cd.obj.name()
                ));
            }
            7 => self.do_current_limited_pq(sys, node_v),
            _ => self.do_constant_pq_gen(sys, node_v),
        }
    }

    /// Pascal `CalcInjCurrentArray`.
    pub(super) fn calc_inj_current_array(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        if self.gen_switch_open {
            self.cd.inj_current.fill(Complex64::ZERO);
        } else {
            self.calc_gen_model_contribution(sys, node_v, errors);
        }
    }
}
