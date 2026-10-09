//! The impedance/admittance numerics (`RecalcElementData`, `stamp_series`,
//! `CalcYPrim`) and the `impl CktElement` for [`Reactor`].

use num_complex::Complex64;

use super::{Reactor, ReactorSpecType};
use crate::elements::ckt::CktElementData;
use crate::elements::pd::matrix_order_refusal;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan, pos_seq_self_term};
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
            let fits = |m: &Option<Vec<f64>>| m.as_ref().is_none_or(|m| m.len() == n2);
            if !(fits(&self.rmatrix) && fits(&self.xmatrix)) {
                // A matrix of another order than the phases has nothing to
                // invert: the solve refuses the reactor (`refusal`).
                self.gmatrix = None;
                self.bmatrix = None;
                return;
            }
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

    /// The `R`, `X` and `Rp` writes of a matrix reactor's positive-sequence
    /// reduction, two or more phases. The value is the positive-sequence self
    /// term [`pos_seq_self_term`] of the matrix the element stamps:
    /// - series form, two or three phases: `R + jX = 1 / S(Y)` with
    ///   `Y = (R + jX)⁻¹` inverted as the stamp inverts it;
    /// - parallel form, two or three phases: `R = 0`, `X = −1 / S(B)` and
    ///   `Rp = 1 / S(G)`, from the `G = R⁻¹` and `B = −X⁻¹` the stamp uses;
    /// - above three phases: `S(rmatrix)` and `S(xmatrix)` (user ruling
    ///   2026-10-04), as `R` and `X` for the series form, as `Rp` and `X` with
    ///   `R = 0` for the parallel form.
    ///
    /// A zero self term has no finite element and takes the open fallback of
    /// the stamp, at every phase count: a parallel `X` of `1 / EPSILON`, and a
    /// series `R` of `1 / EPSILON` with `X = 0` (pinned by
    /// `make_pos_sequence_four_conductor_zero_self_term_stays_finite`).
    ///
    /// The R + jX element the reduction leaves stamps `Rp` whenever one was
    /// given, while the matrix stamp never reads it, so a reduction that writes
    /// no `Rp` of its own writes 0 over a given one.
    fn matrix_pos_seq(&self) -> Vec<PosSeqAction> {
        use super::prop::*;
        let n = self.cd.nphases;
        let zeros = vec![0.0; n * n];
        let rm = self.rmatrix.as_deref().unwrap_or(&zeros);
        let xm = self.xmatrix.as_deref().unwrap_or(&zeros);
        let (r, x, rp) = if self.is_parallel {
            let (x, rp) = if n <= 3 {
                let g = pos_seq_self_term(self.gmatrix.as_deref().unwrap_or(&zeros), n);
                let b = pos_seq_self_term(self.bmatrix.as_deref().unwrap_or(&zeros), n);
                let x = if b != 0.0 { -1.0 / b } else { 1.0 / EPSILON };
                (x, if g != 0.0 { 1.0 / g } else { 0.0 })
            } else {
                let x = pos_seq_self_term(xm, n);
                let x = if x != 0.0 { x } else { 1.0 / EPSILON };
                (x, pos_seq_self_term(rm, n))
            };
            (0.0, x, rp)
        } else if n <= 3 {
            let mut y = self.series_zmatrix(1.0).unwrap_or_else(|| CMatrix::new(n));
            Self::invert_series(&mut y, n);
            let y: Vec<Complex64> = (0..n * n).map(|k| y.get(k / n, k % n)).collect();
            let s = pos_seq_self_term(&y, n);
            let z1 = if s != Complex64::ZERO {
                1.0 / s
            } else {
                Complex64::new(1.0 / EPSILON, 0.0)
            };
            (z1.re, z1.im, 0.0)
        } else {
            let (r, x) = (pos_seq_self_term(rm, n), pos_seq_self_term(xm, n));
            if r == 0.0 && x == 0.0 {
                (1.0 / EPSILON, 0.0, 0.0)
            } else {
                (r, x, 0.0)
            }
        };
        let mut out = vec![PosSeqAction::SetF64(R, r), PosSeqAction::SetF64(X, x)];
        if rp != 0.0 || self.rp_specified {
            out.push(PosSeqAction::SetF64(RP, rp));
        }
        out
    }

    /// Invert the series impedance `zmat` in place into its admittance. On an
    /// inversion error it becomes a tiny series conductance on the diagonal.
    /// The solve refuses a series `rmatrix`/`xmatrix` pair that does not
    /// invert at base frequency ([`Self::refusal`]), so the fallback is left to
    /// a symmetrical-component matrix with a zero sequence impedance and to a
    /// series matrix at another frequency (a GIC solve stamps `rmatrix` alone).
    fn invert_series(zmat: &mut CMatrix, nphases: usize) {
        if zmat.invert().is_err() {
            zmat.clear();
            for i in 0..nphases {
                zmat.set(i, i, Complex64::new(EPSILON, 0.0));
            }
        }
    }

    /// The first given matrix of the parallel form, `rmatrix` then `xmatrix`,
    /// that does not invert. A matrix nobody gave stamps no branch.
    fn singular_parallel_matrix(&self) -> Option<&'static str> {
        let n = self.cd.nphases;
        [("rmatrix", &self.rmatrix), ("xmatrix", &self.xmatrix)]
            .into_iter()
            .find(|(_, m)| {
                m.as_ref()
                    .is_some_and(|m| etk_invert(&mut m.clone(), n).is_err())
            })
            .map(|(prop, _)| prop)
    }

    /// The series-form impedance matrix `R + jX` at base frequency, when both
    /// matrices are given.
    fn series_zmatrix(&self, freq_multiplier: f64) -> Option<CMatrix> {
        let (rm, xm) = (self.rmatrix.as_ref()?, self.xmatrix.as_ref()?);
        let nphases = self.cd.nphases;
        let mut zmat = CMatrix::new(nphases);
        for i in 0..nphases {
            for j in 0..nphases {
                let k = i * nphases + j;
                zmat.set(i, j, Complex64::new(rm[k], xm[k] * freq_multiplier));
            }
        }
        Some(zmat)
    }

    /// Why the solve refuses this reactor, if it does: a delta matrix or
    /// symmetrical-component reactor of two or more phases has no defined
    /// stamp, a given matrix must be `nphases × nphases`, and the series matrix
    /// form needs both matrices, whose impedance `R + jX` must invert (a
    /// singular one has a current pattern that meets no impedance, and so no
    /// admittance, pinned by `a_singular_series_matrix_refuses_the_solve`).
    /// The parallel form stamps `R⁻¹` and `−X⁻¹`, so a given `rmatrix` or
    /// `xmatrix` must invert for the same reason (pinned by
    /// `a_singular_parallel_matrix_refuses_the_solve`).
    fn refusal(&self) -> Option<String> {
        let full = format!("Reactor.{}", self.cd.obj.name());
        let nphases = self.cd.nphases;
        let delta_multi = self.connection == 1 && nphases >= 2;
        match self.spec_type {
            ReactorSpecType::Matrices | ReactorSpecType::SymComponents if delta_multi => {
                let form = if self.spec_type == ReactorSpecType::Matrices {
                    "rmatrix/xmatrix"
                } else {
                    "z1/z2/z0"
                };
                Some(format!(
                    "{full}: a reactor of {nphases} phases given by {form} cannot be \
                     connected in delta. Specify it with conn=wye. Aborting solution."
                ))
            }
            ReactorSpecType::Matrices => {
                for (prop, m) in [("rmatrix", &self.rmatrix), ("xmatrix", &self.xmatrix)] {
                    let order = m.as_ref().and_then(|m| {
                        matrix_order_refusal(&full, nphases, prop, m.len(), "rmatrix and xmatrix")
                    });
                    if order.is_some() {
                        return order;
                    }
                }
                if self.is_parallel {
                    return self.singular_parallel_matrix().map(|prop| {
                        format!(
                            "{full}: {prop} is singular, so the parallel reactor has no \
                             admittance. Specify an {prop} that can be inverted. Aborting solution."
                        )
                    });
                }
                let missing = match (&self.rmatrix, &self.xmatrix) {
                    (None, _) => "rmatrix",
                    (_, None) => "xmatrix",
                    _ => {
                        let z = self.series_zmatrix(1.0);
                        return z.filter(|z| z.clone().invert().is_err()).map(|_| {
                            format!(
                                "{full}: the impedance rmatrix + j xmatrix is singular and has \
                                 no admittance. Specify matrices that can be inverted. \
                                 Aborting solution."
                            )
                        });
                    }
                };
                Some(format!(
                    "{full}: {missing} is missing. A reactor given by matrices needs both \
                     rmatrix and xmatrix. Aborting solution."
                ))
            }
            _ => None,
        }
    }

    /// Invert the series impedance `zmat` ([`Self::invert_series`]) and stamp
    /// the admittance into the four quadrants of the two-terminal `work`
    /// matrix. `zmat[i][j]` is `nphases²` row-major.
    fn stamp_series(work: &mut CMatrix, zmat: &mut CMatrix, nphases: usize) {
        Self::invert_series(zmat, nphases);
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

    /// The total, load and no-load losses. A shunt reactor's no-load loss is
    /// the loss of the `Rp` its stamp holds (the kvar and R + jX forms), `V²/Rp`
    /// at the voltage the stamp puts across each `Rp`: the leg voltages of a
    /// delta, node to node or node to ground as the legs run, and terminal 1 to
    /// terminal 2 of a wye (pinned by `a_delta_reactor_loses_its_rp_across_the_legs`
    /// and `a_wye_reactor_loses_its_rp_between_its_terminals`). The matrix
    /// and Z1 stamps read no `Rp`, so a given one adds nothing, and the whole
    /// loss of a matrix reactor, `Parallel=yes` included, is load loss (pinned
    /// by `the_no_load_loss_is_the_parallel_branch_the_stamp_holds`). A
    /// positive-sequence circuit reports three times the one phase.
    fn get_losses_split(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            return (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO);
        }
        let total = self.losses(sys, node_v);
        let cd = &self.cd;
        let n = cd.nphases;
        let stamps_rp = matches!(
            self.spec_type,
            ReactorSpecType::Kvar | ReactorSpecType::RplusJx
        );
        let mut no_load = if self.is_shunt && stamps_rp && self.rp_specified && self.rp != 0.0 {
            let nconds = cd.nconds;
            // The two conductors of each `Rp`, as `calc_yprim` stamps them.
            let across = |i: usize| -> (usize, usize) {
                if self.connection == 1 {
                    (i, if i + 1 < nconds { i + 1 } else { 0 })
                } else {
                    (i, n + i)
                }
            };
            (0..n).fold(0.0, |acc, i| {
                let (a, b) = across(i);
                let v = node_v[cd.node_ref[a]] - node_v[cd.node_ref[b]];
                acc + v.norm_sqr() / self.rp
            })
        } else {
            0.0
        };
        if sys.positive_sequence {
            no_load *= 3.0;
        }
        let no_load = Complex64::new(no_load, 0.0);
        (total, total - no_load, no_load)
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

        if let Some(msg) = self.refusal() {
            // An enabled reactor refuses the solve (pinned by
            // `a_delta_matrix_reactor_refuses_the_solve`,
            // `a_matrix_reactor_without_both_matrices_refuses_the_solve` and
            // `a_matrix_of_another_phase_count_refuses_the_solve`). A disabled
            // one is out of the model and keeps a zero stamp (pinned by
            // `a_refused_reactor_set_aside_by_disable_leaves_the_solve_to_the_rest`).
            if self.cd.enabled {
                self.cd.obj.push_error(msg);
                self.cd.yprim_refused = true;
            }
            self.cd.yprim_series = Some(CMatrix::new(yorder));
            self.cd.yprim_shunt = Some(CMatrix::new(yorder));
            self.cd.yprim = Some(work);
            self.cd.yprim_invalid = false;
            return;
        }

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
                    // Series R and X: build Z, invert, stamp. `refusal` has
                    // checked that both matrices are given, `nphases²` each.
                    let mut zmat = self
                        .series_zmatrix(freq_multiplier)
                        .expect("refusal checks both matrices");
                    Self::stamp_series(&mut work, &mut zmat, nphases);
                }
            }
            ReactorSpecType::SymComponents => {
                // Symmetrical-component Z's specified.
                let mut zmat = CMatrix::new(nphases);
                // Diagonal — all the same: `Z1` on one phase (pinned by
                // `a_single_phase_z1_reactor_stamps_z1`), `(Z0 + Z1 + Z2) / 3`
                // on more.
                let mut value = if nphases == 1 {
                    self.z1
                } else {
                    self.z2 + self.z1 + self.z0
                };
                value.im *= freq_multiplier;
                if nphases > 1 {
                    value /= 3.0;
                }
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

    /// Collapse a reactor to its positive-sequence single-phase form, keeping
    /// its power under a balanced voltage (the positive-sequence circuit
    /// reports three times the one phase). The edit sits in
    /// `BeginEdit`/`EndEdit`. Inside it, by `SpecType`:
    /// - 2 (R+jX) / 4 (Z1): `Phases := 1`. A closed three-phase delta R+jX
    ///   reactor also writes its wye equivalent, a third of the leg `R`, `X`
    ///   and `Rp`.
    /// - 1 (kvar): kvar/3 per phase, kV per the connection/phase rule. A closed
    ///   three-phase delta also writes a third of the leg `R` and `Rp`.
    /// - 3 (matrices, two or more phases): see [`Self::matrix_pos_seq`]. The
    ///   matrix stamp reads no `RCurve`/`LCurve`, so the reduction drops them
    ///   (pinned by `make_pos_sequence_matrix_drops_the_curves_it_never_read`).
    ///
    /// A delta matrix or Z1 reactor of two or more phases, a matrix of another
    /// order than the phases, and a series matrix reactor missing one of its
    /// matrices or with a singular impedance get no actions: the solve refuses
    /// them. Pinned by the
    /// `make_pos_sequence_*` and `*_refuses_the_solve` tests of
    /// `elements::pd::reactor::tests`.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        use super::prop::*;

        if self.cd.nphases >= 2 && self.refusal().is_some() {
            return PosSeqPlan::base();
        }

        let nphases = self.cd.nphases;
        let closed_delta = self.connection == 1 && nphases == 3 && self.cd.nconds == 3;
        let mut actions = vec![PosSeqAction::BeginEdit];

        match self.spec_type {
            ReactorSpecType::RplusJx if closed_delta => {
                actions.push(PosSeqAction::SetI32(PHASES, 1));
                actions.push(PosSeqAction::SetF64(R, self.z.re / 3.0));
                actions.push(PosSeqAction::SetF64(X, self.z.im / 3.0));
                if self.rp_specified {
                    actions.push(PosSeqAction::SetF64(RP, self.rp / 3.0));
                }
            }
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
                // kV and kvar already give the wye X of a delta leg.
                if closed_delta {
                    if self.z.re != 0.0 {
                        actions.push(PosSeqAction::SetF64(R, self.z.re / 3.0));
                    }
                    if self.rp_specified {
                        actions.push(PosSeqAction::SetF64(RP, self.rp / 3.0));
                    }
                }
            }
            ReactorSpecType::Matrices => {
                if nphases > 1 {
                    actions.push(PosSeqAction::SetI32(PHASES, 1));
                    actions.extend(self.matrix_pos_seq());
                    // The R + jX stamp reads `RCurve`/`LCurve`, the matrix
                    // stamp does not: the reduced reactor keeps none.
                    self.r_curve_name.clear();
                    self.r_curve = None;
                    self.l_curve_name.clear();
                    self.l_curve = None;
                    self.cd.obj.clear_seq(RCURVE);
                    self.cd.obj.clear_seq(LCURVE);
                }
            }
        }

        actions.push(PosSeqAction::EndEdit);
        PosSeqPlan::with_actions(actions)
    }
}
