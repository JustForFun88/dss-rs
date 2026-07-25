//! Solve-time electrical machinery: `CalcYPrimMatrix`, the four `DoXxxGen` model
//! currents (const-PQ / const-Z / const-P fixed-Q / const-P fixed-X), the
//! injection assembly, and the disabled harmonics path. The model-current signs
//! are the reverse of the load's because the generator pushes power into the
//! node.

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::support::cmatrix::CMatrix;
use crate::util::{EPSILON, sqrt3};

use super::{Connection, WindGen};

impl WindGen {
    /// Pascal `CalcYPrimMatrix` (power-flow + harmonic/dynamic paths).
    pub(super) fn calc_yprim_matrix(&mut self, ymatrix: &mut CMatrix, sys: &SysCtx) {
        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        if sys.is_dynamic_model || sys.is_harmonic_model {
            // Dynamic/harmonic YPrim: a small equivalent admittance behind the
            // WTG impedance (`WTGZLV`), `EPSILON` when off.
            let mut y = if self.gen_on {
                let wtg_zlv = self.kv_windgen_base.powi(2) * 1e3 / self.kva_rating;
                Complex64::new(
                    EPSILON,
                    -(self.wind_model_dyn.n_wtg as f64) / (self.wind_model_dyn.zthev.im * wtg_zlv),
                )
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

        // Regular power-flow model: Yeq is L-N; negate for generation.
        let mut y = -self.yeq;
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
    fn calc_vterminal_phase(&mut self, node_v: &[Complex64]) {
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
    }

    /// Pascal `CalcYPrimContribution`: `InjCurrent = Yprim · V(node)`.
    pub(super) fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
        }
    }

    /// The shared tail of every `DoXxxGen`: terminal/injection bookkeeping.
    fn put_curr(&mut self, sys: &SysCtx, curr: Complex64, i: usize) {
        self.stick_curr(true, -curr, i); // into ITerminal
        self.cd.iterminal_updated = true;
        self.cd.mark_iterminal_solved(sys.solution_count);
        self.stick_curr(false, curr, i); // into InjCurrent
    }

    /// Pascal `DoConstantPQGen` (model 1).
    fn do_constant_pq_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.cd.zero_iterminal();
        self.calc_vterminal_phase(node_v);

        let s = Complex64::new(self.p_nominal_per_phase, self.q_nominal_per_phase);
        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let curr = match self.connection {
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
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoConstantZGen` (model 2).
    fn do_constant_z_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(node_v);
        self.cd.zero_iterminal();
        let yeq2 = match self.connection {
            Connection::Wye => self.yeq,
            Connection::Delta => self.yeq / 3.0,
        };
        for i in 0..self.cd.nphases {
            let curr = yeq2 * self.cd.vterminal[i];
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoFixedQGen` (model 4: constant P, fixed Q = kvarBase).
    fn do_fixed_q_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(node_v);
        self.cd.zero_iterminal();

        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let s = Complex64::new(self.p_nominal_per_phase, self.var_base);
            let curr = match self.connection {
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
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoFixedQZGen` (model 5: constant P, fixed Q as a fixed Z).
    fn do_fixed_qz_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(node_v);
        self.cd.zero_iterminal();

        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let p = Complex64::new(self.p_nominal_per_phase, 0.0);
            let curr = match self.connection {
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
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoHarmonicMode` — WindGen's harmonics model is **disabled
    /// upstream in 0.15.x** (a loud abort). Reproduced 1:1: emit the exact
    /// message + request a solution abort; do not invent injection behavior.
    fn do_harmonic_mode(&mut self) {
        self.cd.obj.push_error_abort(format!(
            "WindGen.{}: WindGen harmonics model is not fully implemented. \
             Please use the Generator model instead.",
            self.cd.obj.name()
        ));
    }

    /// Pascal `CalcGenModelContribution`.
    pub(super) fn calc_gen_model_contribution(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut crate::diag::ErrorLog,
    ) {
        self.cd.iterminal_updated = false;
        if sys.is_dynamic_model {
            self.do_dynamic_mode(sys, node_v, errors);
            return;
        }
        if sys.is_harmonic_model && sys.frequency != sys.fundamental {
            self.do_harmonic_mode();
            return;
        }
        match self.gen_model {
            1 => self.do_constant_pq_gen(sys, node_v),
            2 => self.do_constant_z_gen(sys, node_v),
            4 => self.do_fixed_q_gen(sys, node_v),
            5 => self.do_fixed_qz_gen(sys, node_v),
            _ => self.do_constant_pq_gen(sys, node_v), // for now, until other models
        }
    }

    /// Pascal `InjCurrents` inner: switch-open → zero, else the model.
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

    /// Pascal `TWindGenObj.InitHarmonics` — also **disabled upstream** (loud
    /// abort). Reproduced 1:1.
    pub(super) fn init_harmonics_impl(&mut self) {
        self.cd.obj.push_error_abort(format!(
            "WindGen.{}: WindGen harmonics model is not fully implemented. \
             Please use the Generator model instead.",
            self.cd.obj.name()
        ));
    }
}
