//! The impedance/admittance numerics (`RecalcElementData`, `stamp_series`,
//! `CalcYPrim`) and the `impl CktElement` for [`Reactor`].

use num_complex::Complex64;

use super::{Reactor, ReactorSpecType};
use crate::elements::ckt::CktElementData;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::support::cmatrix::{CMatrix, StampBl};
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
            ReactorSpecType::Kvar => {
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
            ReactorSpecType::RplusJx => {
                // Nothing much to do.
                self.l = self.z.im / w;
            }
            // Pascal's `case` has an empty `3:` arm and no `4:` arm at all
            // (`Reactor.pas:661-665`): matrices / sym components are handled in
            // `CalcYPrim`.
            ReactorSpecType::Matrices | ReactorSpecType::SymComponents => {}
        }

        if self.rp_specified && self.rp != 0.0 {
            self.gp = 1.0 / self.rp;
        } else {
            self.gp = 0.0; // default to 0 if Rp = 0
        }

        if self.is_parallel && self.spec_type == ReactorSpecType::Matrices {
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
        // `StampBl::Direct` places the bottom-left block at `(i+n, j)`, NOT
        // `(j+n, i)` (Pascal `YPrimTemp[i + Fnphases, j] := -Value`,
        // `Reactor.pas:936`, the SpecType-3/4 stamp). For a **symmetric** series Y
        // (R+X, R/X matrices) the two coincide; the **asymmetric** sym-components Y
        // (`SpecType=4`, Z1≠Z2 — the induction-motor model) is transposed by
        // `(j+n, i)`, corrupting the bottom-left YPrim block (invisible to a
        // balanced solve, wrong under unbalance). Pinned by `dump_reactor_symcomp`.
        work.stamp_two_terminal_block(nphases, StampBl::Direct, |i, j| zmat.get(i, j));
    }
}

impl CktElement for Reactor {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// Pascal `TPDElement.CalcFltRate` (base): `Faultrate · pctperm · 0.01`.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: 0.0,
            fault_rate: self.fault_rate,
            pct_perm: self.pct_perm,
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
            ReactorSpecType::Kvar | ReactorSpecType::RplusJx => {
                // Some form of R and X specified. Adjust for frequency: when
                // assigned, RCurve/LCurve scale R/L by GetYValue(FYprimFreq) — the
                // curve's X axis is Hz, not the frequency multiplier (Pascal
                // `Reactor.pas` `CalcYPrim`: `RValue := Z.re *
                // RCurveObj.GetYValue(FYprimFreq)`).
                let r_value = match self.r_curve.as_mut() {
                    Some(c) => z.re * c.get_y_value(yprim_freq),
                    None => z.re,
                };
                let l_value = match self.l_curve.as_mut() {
                    Some(c) => self.l * c.get_y_value(yprim_freq),
                    None => self.l,
                };
                let mut value = Complex64::new(r_value, l_value * two_pi * yprim_freq).inv();
                if self.rp_specified {
                    value += self.gp;
                }

                if self.connection == 1 {
                    // Delta (line-line); AddElement accumulates.
                    work.stamp_delta_series(nphases, nconds, value);
                } else {
                    // Wye: elements only on the diagonals.
                    work.stamp_two_terminal_diag(nphases, nphases, value);
                }
            }
            ReactorSpecType::Matrices => {
                if self.is_parallel {
                    let g = self
                        .gmatrix
                        .as_ref()
                        .expect("parallel SpecType 3 has Gmatrix");
                    let b = self
                        .bmatrix
                        .as_ref()
                        .expect("parallel SpecType 3 has Bmatrix");
                    work.stamp_two_terminal_block(nphases, StampBl::Transposed, |i, j| {
                        // Pascal reads the transposed source index
                        // `Gmatrix[(j-1)*Fnphases + (i-1)]`.
                        let idx = j * nphases + i;
                        if freq_multiplier > 0.0 {
                            Complex64::new(g[idx], b[idx] / freq_multiplier)
                        } else {
                            Complex64::new(g[idx], 0.0)
                        }
                    });
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
            ReactorSpecType::SymComponents => {
                // Symmetrical-component Z's specified.
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
                    //
                    // Escaped with the twin at `util::CALPHA`, measured in Stage
                    // F.3z: the exact value (both sites) costs 33 of the 520
                    // gated corpus cases and the `dump_reactor_symcomp` golden.
                    // See that constant's note for the number and the reasoning.
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

    /// Pascal `TReactorObj.MakePosSequence` (Reactor.pas:1052-1115). Collapse a
    /// reactor to its positive-sequence single-phase form. Always wraps the
    /// edit in `BeginEdit`/`EndEdit`; what happens inside depends on `SpecType`:
    /// - 2 (R+jX) / 4 (Z1): just `Phases := 1`.
    /// - 1 (kvar): kvar/3 per phase, kV per the connection/phase rule.
    /// - 3 (matrices, only when multi-phase): average the self/mutual of
    ///   `RMatrix`/`XMatrix` into `R1`/`X1` (the Pascal mutual loop includes the
    ///   2..N diagonal terms — reproduced verbatim).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        use super::prop::*;

        let nphases = self.cd.nphases;
        let mut actions = vec![PosSeqAction::BeginEdit];

        match self.spec_type {
            ReactorSpecType::RplusJx | ReactorSpecType::SymComponents => {
                actions.push(PosSeqAction::SetI32(PHASES, 1));
            }
            ReactorSpecType::Kvar => {
                // Divide among 3 phases.
                let kvar_per_phase = self.kvarrating / 3.0;
                let phase_kv = if nphases > 1 || self.connection != 0 {
                    self.kvrating / sqrt3()
                } else {
                    self.kvrating
                };
                actions.push(PosSeqAction::SetI32(PHASES, 1));
                actions.push(PosSeqAction::SetF64(KV, phase_kv));
                actions.push(PosSeqAction::SetF64(KVAR, kvar_per_phase));
                // Leave R as specified.
            }
            ReactorSpecType::Matrices => {
                if nphases > 1 {
                    // Average the self/mutual of RMatrix and XMatrix. `avg`
                    // mirrors the Pascal loops exactly (`i := 2..N`, `j := i..N`
                    // — the mutual sum picks up the (2,2)..(N,N) diagonals).
                    let avg = |m: &[f64]| -> f64 {
                        let np = nphases;
                        let npf = np as f64;
                        let mut rs = 0.0; // Avg Self
                        for i in 0..np {
                            rs += m[i * np + i];
                        }
                        rs /= npf;
                        let mut rm = 0.0; // Avg mutual
                        for i0 in 1..np {
                            for j0 in i0..np {
                                rm += m[i0 * np + j0];
                            }
                        }
                        rm /= npf * (npf - 1.0) / 2.0;
                        rs - rm
                    };
                    let r = avg(self.rmatrix.as_deref().expect("SpecType 3 RMatrix"));
                    let x = avg(self.xmatrix.as_deref().expect("SpecType 3 XMatrix"));
                    actions.push(PosSeqAction::SetI32(PHASES, 1));
                    actions.push(PosSeqAction::SetF64(R, r));
                    actions.push(PosSeqAction::SetF64(X, x));
                }
            }
        }

        actions.push(PosSeqAction::EndEdit);
        PosSeqPlan::with_actions(actions)
    }
}
