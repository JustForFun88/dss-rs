//! The impedance/admittance numerics (`RecalcElementData`, `stamp_series`,
//! `CalcYPrim`) and the `impl CktElement` for [`Reactor`].

use num_complex::Complex64;

use super::{Reactor, ReactorSpecType};
use crate::elements::ckt::CktElementData;
use crate::elements::pd::matrix_order_refusal;
use crate::elements::pos_seq::{
    PosSeqAction, PosSeqCtx, PosSeqLeg, PosSeqLegKind, PosSeqPlan, legs, pos_seq_self_term,
};
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

    /// Derive `X`/`L` from the spec, `Gp` from `Rp`, the inverted parallel
    /// `Gmatrix`/`Bmatrix` when needed, and (unless typed) the normal and
    /// emergency current ratings. When an input makes the reactance, the
    /// admittance or a derived rating not a finite number nothing is committed: the reactor
    /// keeps what its last good recalc derived and refuses the build
    /// ([`CktElementData::refuse_recalc`]). The matrix and symmetrical-component
    /// forms are checked on the matrices they stamp
    /// ([`Self::matrix_refused_input`], [`Self::sym_refused_input`]). A
    /// `kvar=0` reactor is an open device ([`Self::is_open`]): its infinite
    /// reactance stamps nothing.
    pub(super) fn recalc(&mut self) {
        use crate::obj::Rating;
        let two_pi = 2.0 * std::f64::consts::PI;
        let w = two_pi * self.cd.base_frequency;

        let mut x = self.z.im;
        let mut l = self.l;
        let mut norm = Ok(self.norm_amps);
        let mut emerg = Ok(self.emerg_amps);
        match self.spec_type {
            ReactorSpecType::Kvar => {
                let kvar_per_phase = self.kvarrating / self.cd.nphases as f64;
                let phase_kv = self.phase_kv();
                x = phase_kv * phase_kv * 1000.0 / kvar_per_phase;
                l = x / w;
                // Leave R as specified.
                if !self.norm_amps_specified {
                    norm = Rating::derived(kvar_per_phase / phase_kv);
                }
                if !self.emerg_amps_specified {
                    emerg = Rating::derived(kvar_per_phase / phase_kv * 1.35);
                }
            }
            ReactorSpecType::RplusJx => l = x / w,
            // Matrices and symmetrical components are stamped from what was
            // typed.
            ReactorSpecType::Matrices | ReactorSpecType::SymComponents => {}
        }
        let gp = if self.rp_specified && self.rp != 0.0 {
            1.0 / self.rp
        } else {
            0.0 // no parallel resistance when Rp = 0
        };

        let all_finite = |m: &Option<Vec<f64>>| m.iter().flatten().all(|v| v.is_finite());
        let stamp_finite = gp.is_finite()
            && match self.spec_type {
                ReactorSpecType::Kvar | ReactorSpecType::RplusJx => {
                    let y = Complex64::new(self.z.re, l * w).inv();
                    self.is_open()
                        || (y.re.is_finite()
                            && y.im.is_finite()
                            && self.curve_refused_input().is_none())
                }
                ReactorSpecType::Matrices => {
                    all_finite(&self.rmatrix)
                        && all_finite(&self.xmatrix)
                        && self.matrix_refused_input().is_none()
                }
                ReactorSpecType::SymComponents => self.sym_refused_input().is_none(),
            };
        // An infinite series reactance has an admittance of zero, as the open
        // `kvar=0` reactor, so the reactance is the quantity refused.
        let reactance_only = matches!(
            self.spec_type,
            ReactorSpecType::Kvar | ReactorSpecType::RplusJx
        ) && !self.is_open()
            && !(l * w).is_finite()
            && self.z.re.is_finite()
            && gp.is_finite();
        let quantity = if reactance_only {
            "reactance"
        } else if !stamp_finite {
            "admittance"
        } else if norm.is_err() {
            "normal current rating"
        } else if emerg.is_err() {
            "emergency current rating"
        } else {
            ""
        };
        match (norm, emerg) {
            (Ok(norm), Ok(emerg)) if quantity.is_empty() => {
                self.cd.recalc_refusal = None;
                self.z.im = x;
                self.l = l;
                self.gp = gp;
                self.norm_amps = norm;
                self.emerg_amps = emerg;
            }
            _ => {
                let input = self.refused_input();
                self.cd.refuse_recalc("Reactor", &input, quantity);
                return;
            }
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

    /// A reactor of no size (`kvar=0`): its reactance is infinite, so it is
    /// an open device that stamps nothing and whose derived ratings are not
    /// set (pinned by `a_zero_kvar_reactor_is_an_open_device`).
    fn is_open(&self) -> bool {
        self.spec_type == ReactorSpecType::Kvar && self.kvarrating == 0.0
    }

    /// The input a refused recalc names: the first input the reactor's form
    /// reads that is not a finite number, else the zero that breaks its
    /// formula (`kV`, or `R` with the reactance input together), else the
    /// divisor of the overflow: `kvar` when the reactance `kV² 1000 / kvar`
    /// overflows on it, else `kV`. The reactance input is `LmH` when it was
    /// given after `X`. The matrix form names a matrix with an entry that is
    /// not finite, else what [`Self::matrix_refused_input`] names, and the
    /// symmetrical-component form what [`Self::sym_refused_input`] names.
    fn refused_input(&self) -> String {
        use super::prop;
        use crate::elements::pd::{input_text, sym_matrix_input_text};
        let nphases = self.cd.nphases;
        let rp = self.rp_specified.then_some(("Rp", self.rp));
        let obj = &self.cd.obj;
        let reactance = if obj.seq_mark(prop::LMH) > obj.seq_mark(prop::X) {
            ("LmH", self.l / 1.0e-3)
        } else {
            ("X", self.z.im)
        };
        let scalars: Vec<(&str, f64)> = match self.spec_type {
            ReactorSpecType::Kvar => [
                ("kvar", self.kvarrating),
                ("kV", self.kvrating),
                ("R", self.z.re),
            ]
            .into_iter()
            .chain(rp)
            .collect(),
            // Property order: `R`, `X`, `Rp`, then `LmH`.
            ReactorSpecType::RplusJx if reactance.0 == "X" => [("R", self.z.re), reactance]
                .into_iter()
                .chain(rp)
                .collect(),
            ReactorSpecType::RplusJx => std::iter::once(("R", self.z.re))
                .chain(rp)
                .chain(std::iter::once(reactance))
                .collect(),
            ReactorSpecType::Matrices => {
                for (name, m) in [("RMatrix", &self.rmatrix), ("XMatrix", &self.xmatrix)] {
                    if let Some(m) = m.as_ref().filter(|m| m.iter().any(|v| !v.is_finite())) {
                        return sym_matrix_input_text(name, m, nphases, 1.0);
                    }
                }
                if let Some(text) = self.matrix_refused_input() {
                    return text;
                }
                rp.into_iter().collect()
            }
            ReactorSpecType::SymComponents => {
                // Only `Rp` besides the sequence impedances reaches the check.
                return match rp.filter(|(_, v)| !v.is_finite()) {
                    Some((name, v)) => input_text(name, v),
                    None => self
                        .sym_refused_input()
                        .unwrap_or_else(|| input_text("Rp", self.rp)),
                };
            }
        };
        if let Some((name, v)) = scalars.iter().find(|(_, v)| !v.is_finite()) {
            return input_text(name, *v);
        }
        if let Some(text) = self.curve_refused_input() {
            return text;
        }
        match self.spec_type {
            ReactorSpecType::RplusJx => format!(
                "{} {}",
                input_text("R", self.z.re),
                input_text(reactance.0, reactance.1)
            ),
            ReactorSpecType::Kvar => {
                let phase_kv = self.phase_kv();
                let numerator = phase_kv * phase_kv * 1000.0;
                let x = numerator / (self.kvarrating / nphases as f64);
                if self.kvrating != 0.0 && numerator.is_finite() && !x.is_finite() {
                    input_text("kvar", self.kvarrating)
                } else {
                    input_text("kV", self.kvrating)
                }
            }
            _ => input_text("kV", self.kvrating),
        }
    }

    /// The first of `RCurve` and `LCurve` that the series stamp reads and that
    /// holds a point that is not a finite number, as the input text a refusal
    /// names. The stamp scales its resistance and its inductance by the
    /// curves at each frequency it is built at, so every point is an input
    /// (pinned by
    /// `exec::tests::rating_refusals::a_curve_point_that_is_not_a_number_refuses_its_element`).
    fn curve_refused_input(&self) -> Option<String> {
        if !matches!(
            self.spec_type,
            ReactorSpecType::Kvar | ReactorSpecType::RplusJx
        ) || self.is_open()
        {
            return None;
        }
        [
            ("RCurve", &self.r_curve_name, &self.r_curve),
            ("LCurve", &self.l_curve_name, &self.l_curve),
        ]
        .into_iter()
        .find(|(_, _, c)| c.as_ref().is_some_and(|c| c.has_point_not_finite()))
        .map(|(prop, name, _)| format!("{prop}={name}"))
    }

    /// The sequence impedances the symmetrical-component stamp reads, by name:
    /// `Z1` alone on one phase, else `Z1`, `Z2` and `Z0`.
    fn sym_inputs(&self) -> Vec<(&'static str, Complex64)> {
        if self.cd.nphases == 1 {
            vec![("Z1", self.z1)]
        } else {
            vec![("Z1", self.z1), ("Z2", self.z2), ("Z0", self.z0)]
        }
    }

    /// Why the symmetrical-component stamp has no finite admittance, `None`
    /// when it has one, as the input text a refusal names: the first sequence
    /// impedance it reads that is not a finite number, else on one and three
    /// phases the read impedances with no finite reciprocal, else, when the
    /// matrix at base frequency ([`Self::sym_zmatrix`]) does not invert to
    /// finite entries, those impedances, or all the read ones when each has a
    /// reciprocal (their mean is the zero). Each is written as `?` reads a
    /// complex, `Z1=[0, 0]`. On one and three phases each read impedance is an
    /// eigenvalue of the phase matrix, so a zero one is a bolted short in its
    /// sequence with no finite admittance, also where the truncated rotation
    /// of [`Self::sym_zmatrix`] leaves the matrix invertible. On two or more
    /// than three phases the stamp reads only their mean, so one zero
    /// impedance builds and a zero mean is refused (pinned by
    /// `a_sequence_impedance_reactor_of_two_or_four_phases_stamps_their_mean`).
    fn sym_refused_input(&self) -> Option<String> {
        let finite = |z: Complex64| z.re.is_finite() && z.im.is_finite();
        let text = |(name, z): &(&str, Complex64)| {
            format!(
                "{name}=[{}, {}]",
                crate::util::float_to_str_ex(z.re),
                crate::util::float_to_str_ex(z.im)
            )
        };
        let inputs = self.sym_inputs();
        if let Some(input) = inputs.iter().find(|(_, z)| !finite(*z)) {
            return Some(text(input));
        }
        let n = self.cd.nphases;
        let infinite: Vec<String> = inputs
            .iter()
            .filter(|(_, z)| !(1.0 / z.norm()).is_finite())
            .map(text)
            .collect();
        if matches!(n, 1 | 3) && !infinite.is_empty() {
            return Some(infinite.join(" "));
        }
        let mut y = self.sym_zmatrix(1.0);
        if y.invert().is_ok() && (0..n * n).all(|k| finite(y.get(k / n, k % n))) {
            return None;
        }
        let named = if infinite.is_empty() {
            inputs.iter().map(text).collect()
        } else {
            infinite
        };
        Some(named.join(" "))
    }

    /// Why the matrix stamp has no finite admittance, `None` when it has one,
    /// as the input text a refusal names. The series form stamps the inverse
    /// of `RMatrix + j XMatrix` at base frequency and names both matrices when
    /// that impedance cannot be told from a singular one in f64
    /// ([`CMatrix::is_numerically_singular`]) or inverts to entries that are
    /// not finite. The parallel form stamps `R⁻¹` and `−X⁻¹` and names the
    /// first given matrix that fails the same test (a singular one meets a
    /// current pattern with no resistance or reactance, a short). A matrix
    /// nobody gave, or of another order than the phases, is left to the solve
    /// ([`Self::refusal`]).
    fn matrix_refused_input(&self) -> Option<String> {
        use crate::elements::pd::sym_matrix_input_text;
        fn given(m: &Option<Vec<f64>>, n: usize) -> Option<&[f64]> {
            m.as_deref().filter(|m| m.len() == n * n)
        }
        let n = self.cd.nphases;
        let text = |name: &str, m: &[f64]| sym_matrix_input_text(name, m, n, 1.0);
        if self.is_parallel {
            return [("RMatrix", &self.rmatrix), ("XMatrix", &self.xmatrix)]
                .into_iter()
                .find_map(|(name, m)| {
                    let m = given(m, n)?;
                    let mut typed = CMatrix::new(n);
                    for (k, &v) in m.iter().enumerate() {
                        typed.set(k / n, k % n, Complex64::new(v, 0.0));
                    }
                    let mut inv = m.to_vec();
                    let ok = !typed.is_numerically_singular()
                        && etk_invert(&mut inv, n).is_ok()
                        && inv.iter().all(|v| v.is_finite());
                    (!ok).then(|| text(name, m))
                });
        }
        let (rm, xm) = (given(&self.rmatrix, n)?, given(&self.xmatrix, n)?);
        let mut y = self.series_zmatrix(1.0)?;
        let finite = |z: Complex64| z.re.is_finite() && z.im.is_finite();
        if !y.is_numerically_singular()
            && y.invert().is_ok()
            && (0..n * n).all(|k| finite(y.get(k / n, k % n)))
        {
            return None;
        }
        Some(format!("{} {}", text("RMatrix", rm), text("XMatrix", xm)))
    }

    /// The symmetrical-component impedance matrix in phase quantities, its
    /// reactances scaled by `freq_multiplier`: `Z1` on one phase, the mean
    /// `(Z0 + Z1 + Z2) / 3` on the diagonal of more, and on three phases the
    /// mutual terms of the sequence-to-phase transform.
    fn sym_zmatrix(&self, freq_multiplier: f64) -> CMatrix {
        let nphases = self.cd.nphases;
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
            // TODO(compat): the rotation is the truncated phasor
            // (-0.5, -0.866025), conjugated to 1∠+120°. The clean fix uses the
            // exact 120° rotation here and at its twin `util::CALPHA`, whose
            // note gives the measurement that keeps it.
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
        zmat
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
            if !Self::invert_series(&mut y, false) {
                y.clear();
                for i in 0..n {
                    y.set(i, i, Complex64::new(EPSILON, 0.0));
                }
            }
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

    /// Invert the series impedance `zmat` in place into its admittance, false
    /// when it has none: `strict`, the test of the matrix form, refuses one
    /// that cannot be told from a singular one ([`CMatrix::is_numerically_singular`]),
    /// and either form one that does not invert to finite entries. The recalc
    /// refuses such a matrix at base frequency ([`Self::matrix_refused_input`],
    /// [`Self::sym_refused_input`]), so the Y build meets one at another
    /// frequency, as a GIC solve that zeroes the reactance of a lossless one.
    fn invert_series(zmat: &mut CMatrix, strict: bool) -> bool {
        if strict && zmat.is_numerically_singular() {
            return false;
        }
        zmat.invert().is_ok()
            && zmat
                .values()
                .iter()
                .all(|z| z.re.is_finite() && z.im.is_finite())
    }

    /// Refuse the Y build of a series stamp with no admittance at the solve
    /// frequency, as a lossless matrix at dc, a short the stamp cannot carry
    /// (pinned by
    /// `exec::tests::rating_refusals::a_typed_impedance_matrix_singular_within_its_rounding_is_refused`).
    fn refuse_series_at_this_frequency(&mut self) {
        let msg = self
            .cd
            .refusal_message("Reactor", &self.series_input_text(), "admittance");
        self.cd.refuse_yprim(msg);
    }

    /// The inputs of the series stamp, as a refusal at the Y build names
    /// them: both matrices of the matrix form, the resistance and reactance
    /// inputs of the R + jX and kvar forms with the curves that scale them,
    /// else the sequence impedances.
    fn series_input_text(&self) -> String {
        use crate::elements::pd::{input_text, sym_matrix_input_text};
        let n = self.cd.nphases;
        let curves = || {
            [
                ("RCurve", &self.r_curve_name, self.r_curve.is_some()),
                ("LCurve", &self.l_curve_name, self.l_curve.is_some()),
            ]
            .into_iter()
            .filter(|(_, _, held)| *held)
            .map(|(prop, name, _)| format!(" {prop}={name}"))
            .collect::<String>()
        };
        match (self.spec_type, &self.rmatrix, &self.xmatrix) {
            (ReactorSpecType::Matrices, Some(rm), Some(xm)) => format!(
                "{} {}",
                sym_matrix_input_text("RMatrix", rm, n, 1.0),
                sym_matrix_input_text("XMatrix", xm, n, 1.0)
            ),
            (ReactorSpecType::RplusJx, _, _) => {
                let obj = &self.cd.obj;
                let reactance = if obj.seq_mark(super::prop::LMH) > obj.seq_mark(super::prop::X) {
                    input_text("LmH", self.l / 1.0e-3)
                } else {
                    input_text("X", self.z.im)
                };
                format!("{} {reactance}{}", input_text("R", self.z.re), curves())
            }
            (ReactorSpecType::Kvar, _, _) => format!(
                "{} {} {}{}",
                input_text("kvar", self.kvarrating),
                input_text("kV", self.kvrating),
                input_text("R", self.z.re),
                curves()
            ),
            _ => self
                .sym_inputs()
                .iter()
                .map(|(name, z)| {
                    format!(
                        "{name}=[{}, {}]",
                        crate::util::float_to_str_ex(z.re),
                        crate::util::float_to_str_ex(z.im)
                    )
                })
                .collect::<Vec<_>>()
                .join(" "),
        }
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

    /// Why the solve refuses this reactor, if it does: the input its last
    /// recalc refused (a singular matrix among them, pinned by
    /// `a_singular_series_matrix_refuses_the_solve` and
    /// `a_singular_parallel_matrix_refuses_the_solve`), a delta matrix or
    /// symmetrical-component reactor of two or more phases has no defined
    /// stamp, a given matrix must be `nphases × nphases`, and the series matrix
    /// form needs both matrices.
    fn refusal(&self) -> Option<String> {
        if let Some(msg) = &self.cd.recalc_refusal {
            return Some(msg.clone());
        }
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
                    return None;
                }
                let missing = match (&self.rmatrix, &self.xmatrix) {
                    (None, _) => "rmatrix",
                    (_, None) => "xmatrix",
                    _ => return None,
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
    /// matrix, false with nothing stamped when it has none. `zmat[i][j]` is
    /// `nphases²` row-major.
    fn stamp_series(work: &mut CMatrix, zmat: &mut CMatrix, nphases: usize, strict: bool) -> bool {
        if !Self::invert_series(zmat, strict) {
            return false;
        }
        // `StampBl::Direct` places the bottom-left block at `(i+n, j)`, NOT
        // `(j+n, i)` (Pascal `YPrimTemp[i + Fnphases, j] := -Value`,
        // `Reactor.pas:936`, the SpecType-3/4 stamp). For a **symmetric** series Y
        // (R+X, R/X matrices) the two coincide; the **asymmetric** sym-components Y
        // (`SpecType=4`, Z1≠Z2 — the induction-motor model) is transposed by
        // `(j+n, i)`, corrupting the bottom-left YPrim block (invisible to a
        // balanced solve, wrong under unbalance). Pinned by `dump_reactor_symcomp`.
        work.stamp_two_terminal_block(nphases, StampBl::Direct, |i, j| zmat.get(i, j));
        true
    }
}

impl CktElement for Reactor {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// Wye as branches from terminal 1 to terminal 2, delta as delta legs,
    /// rated at `kV`. A `kvar` reactor draws `+j 1000 kvar / N` VA a leg, one
    /// given by impedance nothing.
    fn pos_seq_legs(&self) -> Option<Vec<PosSeqLeg>> {
        let cd = &self.cd;
        let n = cd.nphases;
        let delta = self.connection == 1;
        let v = legs::v_leg_of(self.kvrating, n, delta);
        let s = match self.spec_type {
            ReactorSpecType::Kvar => Complex64::new(0.0, 1000.0 * self.kvarrating / n as f64),
            _ => Complex64::ZERO,
        };
        Some(if delta {
            legs::delta(n, PosSeqLegKind::Lumped, v, s)
        } else {
            legs::branch(n, 0, cd.nconds, PosSeqLegKind::Lumped, v, s)
        })
    }

    fn pos_seq_refusal(&self) -> Option<String> {
        self.refusal()
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

    fn norm_amps(&self) -> crate::obj::Rating {
        self.norm_amps
    }
    fn emerg_amps(&self) -> crate::obj::Rating {
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
                let mut value = if self.is_open() {
                    Complex64::new(0.0, 0.0)
                } else {
                    Complex64::new(r_value, l_value * two_pi * yprim_freq).inv()
                };
                // A curve that zeroes the impedance at this frequency leaves
                // a short the stamp cannot carry (pinned by
                // `exec::tests::rating_refusals::a_reactor_curve_that_zeroes_its_impedance_refuses_it_at_that_frequency`).
                if !(value.re.is_finite() && value.im.is_finite()) {
                    self.refuse_series_at_this_frequency();
                    return;
                }
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
                    if !Self::stamp_series(&mut work, &mut zmat, nphases, true) {
                        self.refuse_series_at_this_frequency();
                        return;
                    }
                }
            }
            ReactorSpecType::SymComponents => {
                let mut zmat = self.sym_zmatrix(freq_multiplier);
                if !Self::stamp_series(&mut work, &mut zmat, nphases, false) {
                    self.refuse_series_at_this_frequency();
                    return;
                }
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
