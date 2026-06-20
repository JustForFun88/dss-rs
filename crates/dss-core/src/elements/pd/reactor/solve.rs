//! The impedance/admittance numerics (`RecalcElementData`, `stamp_series`,
//! `CalcYPrim`) and the `impl CktElement` for [`Reactor`].

use num_complex::Complex64;

use super::Reactor;
use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::support::cmatrix::CMatrix;
use crate::support::mathutil::etk_invert;
use crate::util::{EPSILON, sqrt3};

impl Reactor {
    /// Pascal per-phase voltage selection (`RecalcElementData`): delta uses the
    /// coil rating; wye assumes a three-phase line-line rating for 2/3 phases.
    fn phase_kv(&self) -> f64 {
        if self.connection == 1 {
            self.kvrating // delta: line-line
        } else {
            match self.cd.nphases {
                2 | 3 => self.kvrating / sqrt3(),
                _ => self.kvrating,
            }
        }
    }

    /// Pascal `RecalcElementData`: derive `Z.im`/`L` from the spec, set `Gp` from
    /// `Rp`, build the inverted parallel `Gmatrix`/`Bmatrix` when needed, and
    /// (unless overridden) the default Norm/Emerg current ratings.
    pub(super) fn recalc(&mut self) {
        let two_pi = 2.0 * std::f64::consts::PI;
        let w = two_pi * self.cd.base_frequency;

        match self.spec_type {
            1 => {
                // kvar
                let kvar_per_phase = self.kvarrating / self.cd.nphases as f64;
                let phase_kv = self.phase_kv();
                self.z.im = phase_kv * phase_kv * 1000.0 / kvar_per_phase;
                self.l = self.z.im / w;
                // Leave R as specified.
                if !self.norm_amps_specified {
                    self.norm_amps = kvar_per_phase / phase_kv;
                }
                if !self.emerg_amps_specified {
                    self.emerg_amps = kvar_per_phase / phase_kv * 1.35;
                }
            }
            2 => {
                // R + jX: nothing much to do.
                self.l = self.z.im / w;
            }
            _ => {} // matrices / sym components: handled in CalcYPrim
        }

        if self.rp_specified && self.rp != 0.0 {
            self.gp = 1.0 / self.rp;
        } else {
            self.gp = 0.0; // default to 0 if Rp = 0
        }

        if self.is_parallel && self.spec_type == 3 {
            let nphases = self.cd.nphases;
            let n2 = nphases * nphases;
            // Copy Rmatrix to Gmatrix and invert (Pascal comment notes the source
            // bug where Rmatrix was inverted in place; the ported code inverts the
            // copy, matching the shipped binary).
            let mut g = self.rmatrix.clone().unwrap_or_else(|| vec![0.0; n2]);
            if etk_invert(&mut g, nphases).is_err() {
                self.cd.obj.push_error(format!(
                    "Error inverting R Matrix for \"{}\" - G is zeroed.",
                    self.cd.obj.name()
                ));
                g.iter_mut().for_each(|v| *v = 0.0);
            }
            self.gmatrix = Some(g);

            // Copy -Xmatrix to Bmatrix and invert.
            let mut b: Vec<f64> = self
                .xmatrix
                .as_deref()
                .unwrap_or(&vec![0.0; n2])
                .iter()
                .map(|v| -v)
                .collect();
            if etk_invert(&mut b, nphases).is_err() {
                self.cd.obj.push_error(format!(
                    "Error inverting X Matrix for \"{}\" - B is zeroed.",
                    self.cd.obj.name()
                ));
                b.iter_mut().for_each(|v| *v = 0.0);
            }
            self.bmatrix = Some(b);
        }
    }

    /// Build the series-impedance `ZMatrix` (already inverted to a Y matrix) for
    /// `SpecType = 3` series and `SpecType = 4`, then stamp it into the four
    /// quadrants of the two-terminal `work` matrix. `zmat[i][j]` is `nphases²`
    /// row-major. Mirrors the shared Pascal stamping loop.
    fn stamp_series(work: &mut CMatrix, zmat: &mut CMatrix, nphases: usize) {
        if zmat.invert().is_err() {
            // Inversion error: tiny series conductance on the diagonal.
            zmat.clear();
            for i in 0..nphases {
                zmat.set(i, i, Complex64::new(EPSILON, 0.0));
            }
        }
        for i in 0..nphases {
            for j in 0..nphases {
                let value = zmat.get(i, j);
                work.set(i, j, value);
                work.set(i + nphases, j + nphases, value);
                work.set(i, j + nphases, -value);
                work.set(j + nphases, i, -value);
            }
        }
    }
}

impl CktElement for Reactor {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TPDElement.CalcFltRate` (base): `Faultrate · pctperm · 0.01`.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: 0.0,
        }
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `TReactorObj.GetLosses` (Reactor.pas l.1017): no-load losses are
    /// `V²/Rp` across the shunt — only when `Rp` is specified on a shunt
    /// reactor; otherwise the default element behavior.
    fn get_losses_split(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO);
        }
        if self.rp_specified && self.is_shunt && self.rp != 0.0 {
            let total = self.losses(sys, node_v);
            let mut no_load = 0.0_f64;
            let cd = &self.cd;
            for i in 0..cd.nphases {
                let v = node_v[cd.node_ref[i]];
                no_load += (v.re * v.re + v.im * v.im) / self.rp;
            }
            if sys.positive_sequence {
                no_load *= 3.0;
            }
            let no_load = Complex64::new(no_load, 0.0);
            (total, total - no_load, no_load)
        } else {
            let total = self.losses(sys, node_v);
            (total, total, Complex64::ZERO)
        }
    }

    /// Pascal `TPDElement.IsShunt` (set by the Bus1/Bus2 side effects).
    fn is_shunt(&self) -> bool {
        self.is_shunt
    }

    /// Pascal `TReactorObj.CalcYPrim`: stamp the reactor admittance by spec type
    /// into the shunt (or series) primitive, then mirror tiny diagonals into the
    /// other matrix so `CalcVoltages` never sees an all-zero row.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let two_pi = 2.0 * std::f64::consts::PI;
        let yorder = self.cd.yorder;
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        let mut yprim_freq = sys.frequency;
        let mut freq_multiplier = yprim_freq / self.cd.base_frequency;
        let mut z = self.z; // local copy (the GIC path may adjust Z.re)

        // If GIC simulation (< 0.5 Hz), resistance only.
        if sys.frequency < 0.51 {
            if z.im > 0.0 && z.re <= 0.0 {
                z.re = z.im / 50.0; // assume X/R = 50
            }
            yprim_freq = 0.0;
            freq_multiplier = 0.0;
        }
        self.cd.yprim_freq = yprim_freq;

        let mut work = CMatrix::new(yorder);

        match self.spec_type {
            1 | 2 => {
                // Some form of R and X specified. RCurve/LCurve are NOT_PORTED, so
                // R(f)/L(f) always use the stored values (unity curve).
                let r_value = z.re;
                let l_value = self.l;
                let mut value = Complex64::new(r_value, l_value * two_pi * yprim_freq).inv();
                if self.rp_specified {
                    value += self.gp;
                }
                let value2 = -value;

                if self.connection == 1 {
                    // Delta (line-line); AddElement accumulates.
                    for i in 1..=nphases {
                        let mut j = i + 1;
                        if j > nconds {
                            j = 1;
                        }
                        work.add(i - 1, i - 1, value);
                        work.add(j - 1, j - 1, value);
                        work.add(i - 1, j - 1, value2);
                        work.add(j - 1, i - 1, value2);
                    }
                } else {
                    // Wye: elements only on the diagonals.
                    for i in 1..=nphases {
                        let j = i + nphases;
                        work.set(i - 1, i - 1, value);
                        work.set(j - 1, j - 1, value);
                        work.set(i - 1, j - 1, value2);
                        work.set(j - 1, i - 1, value2);
                    }
                }
            }
            3 => {
                // R/X matrices.
                if self.is_parallel {
                    let g = self
                        .gmatrix
                        .as_ref()
                        .expect("parallel SpecType 3 has Gmatrix");
                    let b = self
                        .bmatrix
                        .as_ref()
                        .expect("parallel SpecType 3 has Bmatrix");
                    for i in 1..=nphases {
                        for j in 1..=nphases {
                            let idx = (j - 1) * nphases + (i - 1);
                            let value = if freq_multiplier > 0.0 {
                                Complex64::new(g[idx], b[idx] / freq_multiplier)
                            } else {
                                Complex64::new(g[idx], 0.0)
                            };
                            work.set(i - 1, j - 1, value);
                            work.set(i - 1 + nphases, j - 1 + nphases, value);
                            work.set(i - 1, j - 1 + nphases, -value);
                            work.set(j - 1 + nphases, i - 1, -value);
                        }
                    }
                } else {
                    // Series R and X: build Z, invert, stamp.
                    let rm = self.rmatrix.as_ref().expect("SpecType 3 has Rmatrix");
                    let xm = self.xmatrix.as_ref().expect("SpecType 3 has Xmatrix");
                    let mut zmat = CMatrix::new(nphases);
                    for i in 0..nphases {
                        for j in 0..nphases {
                            let k = i * nphases + j;
                            zmat.set(i, j, Complex64::new(rm[k], xm[k] * freq_multiplier));
                        }
                    }
                    Self::stamp_series(&mut work, &mut zmat, nphases);
                }
            }
            _ => {
                // Symmetrical-component Z's specified (SpecType 4).
                let mut zmat = CMatrix::new(nphases);
                // Diagonal — all the same.
                let mut value = if nphases == 1 {
                    self.z1
                } else {
                    self.z2 + self.z1 + self.z0
                };
                value.im *= freq_multiplier;
                value /= 3.0;
                for i in 0..nphases {
                    zmat.set(i, i, value);
                }

                if nphases == 3 {
                    // TODO(compat): Pascal `CALPHA` is the truncated literal
                    // (-0.5, -0.866025) (DSSGlobals.pas:74), not the exact 1∠-120°.
                    // `Calpha1 := cong(Calpha)` then flips it to 1∠+120° "to agree
                    // with textbooks". The clean fix uses an exact 120° rotation.
                    let calpha = Complex64::new(-0.5, -0.866025);
                    let calpha1 = calpha.conj();
                    let calpha2 = calpha1 * calpha1;
                    let mut value2 = calpha2 * self.z2 + calpha1 * self.z1 + self.z0;
                    let mut value1 = calpha2 * self.z1 + calpha1 * self.z2 + self.z0;
                    value1.im *= freq_multiplier;
                    value2.im *= freq_multiplier;
                    value1 /= 3.0;
                    value2 /= 3.0;
                    // Lower triangle.
                    zmat.set(1, 0, value1);
                    zmat.set(2, 0, value2);
                    zmat.set(2, 1, value1);
                    // Upper triangle.
                    zmat.set(0, 1, value2);
                    zmat.set(0, 2, value1);
                    zmat.set(1, 2, value2);
                }

                Self::stamp_series(&mut work, &mut zmat, nphases);
            }
        }

        // Distribute the work matrix into shunt/series and mirror diagonals so
        // CalcVoltages doesn't fail.
        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        if self.is_shunt {
            yp_shunt.copy_from(&work);
            // 1-phase non-positive-sequence: assume a neutral/grounding reactor,
            // leave the full diagonal in the circuit; otherwise scale it down.
            let factor = if nphases == 1 && !sys.positive_sequence {
                1.0
            } else {
                1.0e-10
            };
            for i in 0..yorder {
                yp_series.set(i, i, work.get(i, i) * factor);
            }
        } else {
            yp_series.copy_from(&work);
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&work);

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }
}
