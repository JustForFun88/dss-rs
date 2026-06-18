//! Solve-time electrical machinery for `TLoadObj`: the Yprim matrix build, the
//! per-phase load-model currents (`DoConstantPQLoad` .. `DoZIPVModel`), the
//! injection-current assembly (`CalcInjCurrentArray`/`CalcLoadModelContribution`
//! and the `StickCurrInTerminalArray` distribution), and the EnergyMeter
//! reliability criteria (`ExceedsNormal`/`Unserved`).

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::support::cmatrix::CMatrix;

use super::{Connection, Load, LoadModel};

impl Load {
    /// Pascal `CalcYPrimMatrix` (power-flow path; the harmonic series-RL
    /// split arrives in Phase 7).
    pub(super) fn calc_yprim_matrix(&mut self, ymatrix: &mut CMatrix, sys: &SysCtx) {
        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let mut y = self.yeq;
        y.im /= freq_multiplier; // correct reactive part for frequency

        let yij = -y;
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        match self.connection {
            Connection::Wye => {
                for i in 0..nphases {
                    ymatrix.set(i, i, y);
                    ymatrix.add(nconds - 1, nconds - 1, y);
                    ymatrix.set(i, nconds - 1, yij);
                    ymatrix.set(nconds - 1, i, yij);
                }
                ymatrix.add(nconds - 1, nconds - 1, self.y_neut); // neutral

                // If the neutral is floating, keep a small connection to
                // ground by increasing the last diagonal slightly.
                if self.rneut < 0.0 {
                    let v = ymatrix.get(nconds - 1, nconds - 1) * 1.000001;
                    ymatrix.set(nconds - 1, nconds - 1, v);
                }
            }
            Connection::Delta => {
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 };
                    ymatrix.add(i, i, y);
                    ymatrix.add(j, j, y);
                    ymatrix.add_sym(i, j, yij);
                }
            }
        }
    }

    /// Pascal `StickCurrInTerminalArray` — `kind` selects the target array
    /// since Rust can't alias `&mut` fields. `i` is the 0-based phase.
    fn stick_curr(&mut self, into_iterminal: bool, curr: Complex64, i: usize) {
        let nconds = self.cd.nconds;
        let arr = if into_iterminal {
            &mut self.cd.iterminal
        } else {
            &mut self.cd.inj_current
        };
        match self.connection {
            Connection::Wye => {
                arr[i] -= curr;
                arr[nconds - 1] += curr; // neutral
            }
            Connection::Delta => {
                arr[i] -= curr;
                let j = if i + 1 >= nconds { 0 } else { i + 1 };
                arr[j] += curr;
            }
        }
    }

    /// Pascal `CalcVTerminalPhase`: phase voltages via `VDiff`.
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
        self.load_solution_count = sys.solution_count;
    }

    /// Lowest per-unit phase voltage (Pascal `TakeSample` voltage criterion used
    /// by `ExceedsNormal`/`Unserved`): refresh the phase voltages if stale, then
    /// `min(|Vterminal[i]|) / Vbase`.
    fn min_phase_vpu(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        if self.load_solution_count != sys.solution_count {
            self.calc_vterminal_phase(sys, node_v);
        }
        let mut vpu = self.v_base;
        for i in 0..self.cd.nphases {
            let vmag = self.cd.vterminal[i].norm();
            if vmag < vpu {
                vpu = vmag;
            }
        }
        vpu / self.v_base
    }

    /// Pascal `TLoadObj.ExceedsNormal` (Load.pas l.2057): true if a critical
    /// line is overloaded (`EEN_Factor > 0`) or the lowest phase voltage is
    /// below the normal-minimum criterion; in the latter case it (re)computes
    /// `EEN_Factor`. `norm_min`/`emerg_min` are the circuit defaults.
    pub fn exceeds_normal(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        norm_min: f64,
        emerg_min: f64,
    ) -> bool {
        if self.een_factor > 0.0 {
            return true;
        }
        let vpu = self.min_phase_vpu(sys, node_v);
        let norm_crit = if self.vmin_normal != 0.0 {
            self.vmin_normal
        } else {
            norm_min
        };
        let emerg_crit = if self.vmin_emerg != 0.0 {
            self.vmin_emerg
        } else {
            emerg_min
        };
        if vpu < norm_crit {
            self.een_factor = (norm_crit - vpu) / (norm_crit - emerg_crit);
            true
        } else {
            false
        }
    }

    /// Pascal `TLoadObj.Unserved` (Load.pas l.2004): true if a critical line is
    /// overloaded (`UE_Factor > 0`) or the lowest phase voltage is below the
    /// emergency-minimum criterion; in the latter case it (re)computes
    /// `UE_Factor`.
    pub fn unserved(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        norm_min: f64,
        emerg_min: f64,
    ) -> bool {
        if self.ue_factor > 0.0 {
            return true;
        }
        let vpu = self.min_phase_vpu(sys, node_v);
        let norm_crit = if self.vmin_normal != 0.0 {
            self.vmin_normal
        } else {
            norm_min
        };
        let emerg_crit = if self.vmin_emerg != 0.0 {
            self.vmin_emerg
        } else {
            emerg_min
        };
        if vpu < emerg_crit {
            self.ue_factor = (emerg_crit - vpu) / (norm_crit - emerg_crit);
            true
        } else {
            false
        }
    }

    /// Pascal `CalcYPrimContribution`: `InjCurrent = Yprim · V(node)`.
    fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
        }
    }

    /// Pascal `InterpolateY95_YLow`.
    fn interpolate_y95_ylow(&self, vmag: f64) -> Complex64 {
        (self.i_low + self.m95 * (vmag - self.v_base_low)) / vmag
    }

    /// Pascal `InterpolateY95I_YLow`.
    fn interpolate_y95i_ylow(&self, vmag: f64) -> Complex64 {
        (self.i_low + self.m95i * (vmag - self.v_base_low)) / vmag
    }

    /// The per-phase current of each load model at voltage `v` — the bodies
    /// of `DoConstantPQLoad` .. `DoZIPVModel`, factored on the shared
    /// "below VBaseLow → linear Yeq" / interpolation-zone scaffolding.
    fn model_current(&self, v: Complex64, errors: &mut Vec<String>) -> Complex64 {
        let vmag = v.norm();
        match self.load_model {
            LoadModel::ConstPQ => {
                if vmag <= self.v_base_low {
                    self.yeq * v // below VbaseZ: linear, equal to Yprim contribution
                } else if vmag <= self.v_base95 {
                    self.interpolate_y95_ylow(vmag) * v
                } else if vmag > self.v_base105 {
                    self.yeq105 * v // above 105%: impedance model
                } else {
                    (Complex64::new(self.w_nominal, self.var_nominal) / v).conj()
                }
            }
            LoadModel::ConstZ => self.yeq * v,
            LoadModel::Motor => {
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    self.interpolate_y95_ylow(vmag) * v
                } else if vmag > self.v_base105 {
                    self.yeq105 * v
                } else {
                    // Constant P above 95%, plus Q as constant impedance.
                    let mut curr = (Complex64::new(self.w_nominal, 0.0) / v).conj();
                    curr += Complex64::new(0.0, self.yeq.im) * v;
                    curr
                }
            }
            LoadModel::Cvr => {
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    self.interpolate_y95_ylow(vmag) * v
                } else if vmag > self.v_base105 {
                    self.yeq105 * v
                } else {
                    let vratio = vmag / self.v_base; // L-N for wye, L-L for delta
                    let watt_factor = if self.cvr_watt_factor != 1.0 {
                        vratio.powf(self.cvr_watt_factor)
                    } else {
                        vratio
                    };
                    let curr = if watt_factor > 0.0 {
                        (Complex64::new(self.w_nominal * watt_factor, 0.0) / v).conj()
                    } else {
                        Complex64::ZERO
                    };
                    let cvar = if vmag == 0.0 {
                        Complex64::ZERO // trap divide by zero
                    } else if self.cvr_var_factor == 2.0 {
                        Complex64::new(0.0, self.yeq.im) * v // same as constant Z
                    } else if self.cvr_var_factor == 3.0 {
                        let var_factor = vratio * vratio * vratio;
                        (Complex64::new(0.0, self.var_nominal * var_factor) / v).conj()
                    } else {
                        let var_factor = vratio.powf(self.cvr_var_factor);
                        (Complex64::new(0.0, self.var_nominal * var_factor) / v).conj()
                    };
                    curr + cvar
                }
            }
            LoadModel::ConstI => {
                // Injection = [S / (Vbase · V/|V|)]*
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    self.interpolate_y95i_ylow(vmag) * v
                } else if vmag > self.v_base105 {
                    self.yeq105i * v
                } else {
                    (Complex64::new(self.w_nominal, self.var_nominal) / ((v / vmag) * self.v_base))
                        .conj()
                }
            }
            LoadModel::ConstPFixedQ => {
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    Complex64::new(self.yeq95.re, self.yq_fixed) * v
                } else if vmag > self.v_base105 {
                    Complex64::new(self.yeq105.re, self.yq_fixed) * v
                } else {
                    (Complex64::new(self.w_nominal, self.var_base) / v).conj()
                }
            }
            LoadModel::ConstPFixedX => {
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    Complex64::new(self.yeq95.re, self.yq_fixed) * v
                } else if vmag > self.v_base105 {
                    Complex64::new(self.yeq105.re, self.yq_fixed) * v
                } else {
                    let mut curr = (Complex64::new(self.w_nominal, 0.0) / v).conj();
                    curr += Complex64::new(0.0, self.yq_fixed) * v;
                    curr
                }
            }
            LoadModel::Zipv => {
                if !self.zipv_set {
                    errors.push("ZIPV is not set. Aborting...".to_string());
                    return Complex64::ZERO;
                }
                if vmag <= self.v_base_low {
                    return self.yeq * v;
                }
                let z = &self.zipv;
                let (mut curr_z, mut curr_i, mut curr_p) =
                    (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO);
                let mut curr = if vmag <= self.v_base95 {
                    if z[0] != 0.0 || z[3] != 0.0 {
                        curr_z = Complex64::new(self.yeq.re * z[0], self.yeq.im * z[3]);
                    }
                    if z[2] != 0.0 || z[5] != 0.0 {
                        let y = self.interpolate_y95_ylow(vmag);
                        curr_p = Complex64::new(y.re * z[2], y.im * z[5]);
                    }
                    if z[1] != 0.0 || z[4] != 0.0 {
                        let y = self.interpolate_y95i_ylow(vmag);
                        curr_i = Complex64::new(y.re * z[1], y.im * z[4]);
                    }
                    (curr_z + curr_i + curr_p) * v
                } else if vmag > self.v_base105 {
                    if z[0] != 0.0 || z[3] != 0.0 {
                        curr_z = Complex64::new(self.yeq.re * z[0], self.yeq.im * z[3]);
                    }
                    if z[2] != 0.0 || z[5] != 0.0 {
                        curr_p = Complex64::new(self.yeq105.re * z[2], self.yeq105.im * z[5]);
                    }
                    if z[1] != 0.0 || z[4] != 0.0 {
                        curr_i = Complex64::new(self.yeq105i.re * z[1], self.yeq105i.im * z[4]);
                    }
                    (curr_z + curr_i + curr_p) * v
                } else {
                    if z[0] != 0.0 || z[3] != 0.0 {
                        curr_z = Complex64::new(self.yeq.re * z[0], self.yeq.im * z[3]) * v;
                    }
                    if z[1] != 0.0 || z[4] != 0.0 {
                        curr_i = (Complex64::new(self.w_nominal * z[1], self.var_nominal * z[4])
                            / ((v / v.norm()) * self.v_base))
                            .conj();
                    }
                    if z[2] != 0.0 || z[5] != 0.0 {
                        curr_p = (Complex64::new(self.w_nominal * z[2], self.var_nominal * z[5])
                            / v)
                            .conj();
                    }
                    curr_z + curr_i + curr_p
                };

                // Low-voltage drop-out.
                if z[6] > 0.0 {
                    let vx = 500.0 * (vmag / self.v_base - z[6]);
                    if vx < 20.0 {
                        // ≥ 20 ⇒ yv is 1 for an f64
                        let evx = (2.0 * vx).exp();
                        let yv = 0.5 * (1.0 + (evx - 1.0) / (evx + 1.0));
                        curr *= yv;
                    }
                }
                curr
            }
        }
    }

    /// Pascal `CalcLoadModelContribution` (power-flow modes): compute total
    /// load currents and add them into `InjCurrent`/`ITerminal`.
    pub(super) fn calc_load_model_contribution(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        self.cd.iterminal_updated = false;
        // Harmonic mode arrives in Phase 7.

        self.calc_yprim_contribution(node_v); // init InjCurrent array
        self.calc_vterminal_phase(sys, node_v); // actual voltage across each phase
        self.cd.zero_iterminal();

        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let curr = self.model_current(v, errors);

            // Save in case the Load value differs from the terminal value.
            self.phase_curr[i] = curr;

            self.stick_curr(true, -curr, i); // into ITerminal
            self.cd.iterminal_updated = true;
            self.cd.iterminal_solution_count = sys.solution_count;
            self.stick_curr(false, curr, i); // into InjCurrent
        }
    }

    /// Pascal `CalcInjCurrentArray`.
    pub(super) fn calc_inj_current_array(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        if self.cd.all_conductors_closed() {
            self.calc_load_model_contribution(sys, node_v, errors);
            return;
        }

        // Open terminals: use the admittance model for injection
        // ("THIS MAY NOT WORK !!! WATCH FOR BAD RESULTS" — ported as-is).
        if self.open_load_solution_count != sys.solution_count {
            let mut y = CMatrix::new(self.cd.yorder);
            self.calc_yprim_matrix(&mut y, sys);
            let mut k = 0usize;
            for t in 0..self.cd.nterms {
                for j in 0..self.cd.nconds {
                    if !self.cd.terminals[t].conductors_closed[j] {
                        y.zero_row(j + k);
                        y.zero_col(j + k);
                        y.set(j + k, j + k, Complex64::new(1.0e-12, 0.0));
                    }
                }
                k += self.cd.nconds;
            }
            self.yprim_open_cond = Some(y);
            self.open_load_solution_count = sys.solution_count;
        }
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(y) = &self.yprim_open_cond {
            y.mv_mult(&mut cd.complex_buffer, &cd.vterminal);
        }
        for v in cd.complex_buffer.iter_mut() {
            *v = -*v;
        }
    }
}
