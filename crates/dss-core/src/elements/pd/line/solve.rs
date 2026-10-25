//! The impedance/admittance numerics: building per-unit-length `Z`/`Yc` from
//! the symmetrical components (`recalc`), the geometry total-matrix build
//! (`make_z_from_geometry`), and the `impl CktElement` — `CalcYPrim`,
//! `GetSeqLosses`, the reliability/rating accessors.

use num_complex::Complex64;

use crate::compat;
use crate::elements::ckt::CktElementData;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::obj::base::DssObject;
use crate::support::cmatrix::{CMatrix, StampBl};
use crate::support::line_constants::csqrt_fpc;
use crate::support::line_units::{LineUnits, convert_line_units};
use crate::support::mathutil::SymComp;

use super::{Line, prop};

/// Pascal `CAP_EPSILON` (Line.pas): "5 kvar of capacitive reactance at
/// 345 kV to avoid open line problem", added to the series Yprim diagonal.
const CAP_EPSILON: Complex64 = Complex64::new(0.0, 4.2e-8);

/// Pascal `EPSILON` (DSSGlobals.pas:86): default tiny floating point.
const EPSILON: f64 = 1.0e-12;

/// FPC `ucomplex` `cinv` — `1/z` as the **naive** `conj(z)/|z|²`
/// (`re/(re²+im²)`, `-im/(re²+im²)`), NOT Smith's form (that is `compat::cdiv`).
/// Used by `TLineObj.DoLongLine` for `Cinv(ExpP)` / `Cinv(Zc)`.
#[inline]
fn cinv_fpc(z: Complex64) -> Complex64 {
    let denom = z.re * z.re + z.im * z.im;
    Complex64::new(z.re / denom, -z.im / denom)
}

impl Line {
    /// Refuse the Y build of a Line whose series impedance has no admittance:
    /// the message is logged, the build fails and the next solve builds and
    /// refuses again, so no solve goes on without the Line.
    fn refuse_singular_impedance(&mut self) {
        let msg = crate::diag::DssDiagnostic::msg(
            format!(
                "Matrix Inversion Error for Line \"{}\". \
                 Invalid impedance specified. Aborting solution.",
                self.cd.obj.name()
            ),
            Some(183),
        );
        self.cd.refuse_yprim(msg);
    }

    /// Invert `z` in place into the admittance the Line stamps, refusing the
    /// Y build when it has none: a singular impedance
    /// ([`Self::refuse_singular_impedance`], also when `strict` finds it
    /// numerically singular), or an inverse with an entry that is not a finite
    /// number, named by its inputs ([`Self::impedance_input_text`]).
    fn invert_impedance(&mut self, z: &mut CMatrix, strict: bool) -> bool {
        if (strict && z.is_numerically_singular()) || z.invert().is_err() {
            self.refuse_singular_impedance();
            return false;
        }
        if z.values()
            .iter()
            .all(|v| v.re.is_finite() && v.im.is_finite())
        {
            return true;
        }
        let msg = self
            .cd
            .refusal_message("Line", &self.impedance_input_text(), "admittance");
        self.cd.refuse_yprim(msg);
        false
    }

    /// The inputs of the series impedance, as `?` reads them: the geometry,
    /// spacing or LineCode it is taken from, else the sequence impedances or
    /// the impedance matrices, then the length.
    fn impedance_input_text(&self) -> String {
        use crate::elements::pd::input_text;
        let matrix = |name: &str, z: &CMatrix, real: bool| {
            let mut s = format!("{name}=[");
            for i in 0..z.order() {
                if i > 0 {
                    s.push('|');
                }
                for j in 0..=i {
                    let v = z.get(i, j);
                    let part = if real { v.re } else { v.im };
                    s.push_str(&crate::util::float_to_str_ex(part / self.units_convert));
                    s.push(' ');
                }
            }
            s.push(']');
            s
        };
        let source = if self.geometry_obj.is_some() {
            format!("Geometry={}", self.geometry_name)
        } else if self.spacing_specified {
            let name = self
                .line_spacing_obj
                .as_ref()
                .map(|s| s.data().name().to_string())
                .unwrap_or_default();
            format!("Spacing={name}")
        } else if self.line_code_specified {
            format!("LineCode={}", self.line_code_name)
        } else if self.sym_components_model {
            let read = |name: &str, v: f64| input_text(name, v / self.units_convert);
            let mut texts = vec![read("R1", self.r1), read("X1", self.x1)];
            if self.cd.nphases > 1 {
                texts.extend([read("R0", self.r0), read("X0", self.x0)]);
            }
            texts.join(" ")
        } else {
            match &self.z {
                Some(z) => format!(
                    "{} {}",
                    matrix("RMatrix", z, true),
                    matrix("XMatrix", z, false)
                ),
                None => String::new(),
            }
        };
        format!("{source} {}", input_text("Length", self.len))
    }

    /// Pascal `TLineObj.RecalcElementData`: compute the per-unit-length
    /// `Z`/`Yc` from the symmetrical components. Only called when those have
    /// changed.
    pub(super) fn recalc(&mut self, positive_sequence: bool) {
        let nphases = self.cd.nphases;
        let mut z = CMatrix::new(nphases);
        let mut yc = CMatrix::new(nphases);

        let ztemp = Complex64::new(self.r1, self.x1) * 2.0;
        // Handle special case for 1-phase line and/or pos-seq model:
        // zero sequence the same as positive sequence.
        if nphases == 1 || positive_sequence {
            self.r0 = self.r1;
            self.x0 = self.x1;
            self.c0 = self.c1;
        }

        let zs = (ztemp + Complex64::new(self.r0, self.x0)) / 3.0;
        let zm = (Complex64::new(self.r0, self.x0) - Complex64::new(self.r1, self.x1)) / 3.0;

        let two_pi = 2.0 * std::f64::consts::PI;
        let yc1 = two_pi * self.cd.base_frequency * self.c1;
        let yc0 = two_pi * self.cd.base_frequency * self.c0;

        let ys = (Complex64::new(0.0, yc1) * 2.0 + Complex64::new(0.0, yc0)) / 3.0;
        let ym = (Complex64::new(0.0, yc0) - Complex64::new(0.0, yc1)) / 3.0;

        for i in 0..nphases {
            z.set(i, i, zs);
            yc.set(i, i, ys);
            for j in 0..i {
                z.set(i, j, zm);
                z.set(j, i, zm);
                yc.set(i, j, ym);
                yc.set(j, i, ym);
            }
        }

        self.z = Some(z);
        self.yc = Some(yc);
        self.sym_components_changed = false;
        // values in ohms per unit length
    }

    /// Build the total `Z`/`Yc` (length and units folded in) from the attached
    /// geometry at frequency `f`, returning the last build when `f` is its
    /// frequency. The geometry takes this Line's earth model. Returns the
    /// geometry's error, which refuses the Line at the Y build. The guard for a
    /// Line that holds no geometry is defensive: both callers, `calc_yprim`
    /// and `make_pos_sequence`, take this path only when the Line holds one.
    fn make_z_from_geometry(&mut self, f: f64) -> Result<(), String> {
        if f == self.fz_frequency {
            return Ok(()); // already done for this frequency
        }
        let len = self.len;
        let units = self.length_units.code();
        let earth_model = self.earth_model;
        let (eps, h_off, h_unit) = (self.eps_r_medium, self.height_offset, self.height_units);
        let Some(geom) = self.geometry_obj.as_mut() else {
            return Err(format!(
                "Line.{} has no LineGeometry to build its impedance from.",
                self.cd.obj.name()
            ));
        };
        // dss_capi 0.15.x (Line.pas:2049-2051): push EpsRMedium/HeightOffset/
        // HeightUnit into the geometry's engine *before* the matrix read (which
        // recomputes because these flag `rhoChanged`).
        geom.set_line_constants_medium(eps, h_off, h_unit);
        let z = geom.z_matrix(f, len, units, earth_model)?;
        let yc = geom.yc_matrix(f, len, units, earth_model)?;
        self.z = Some(z);
        self.yc = Some(yc);
        self.fz_frequency = f;
        Ok(())
    }

    /// Build the total `Z`/`Yc` from the attached `LineSpacing` and conductor
    /// list through a throwaway `LineGeometry` at frequency `f`, returning the
    /// last build when `f` is its frequency. A conductor edit, a `spacing=`
    /// miss and a switch to the spacing model drop the last build (see
    /// `side_effects`), and `make_pos_sequence` drops it before its own build,
    /// so each of them builds afresh. Other edits leave the last build in
    /// place until the frequency changes. Length and units are folded in by
    /// `LineGeometryObj::z_matrix`, so the result is total, as on the geometry
    /// path. Returns the geometry's error, which refuses the Line at the Y
    /// build. A Line whose `spacing=` named no LineSpacing that exists after
    /// its wires were given holds no spacing and gets an error naming it.
    fn make_z_from_spacing(&mut self, f: f64) -> Result<(), String> {
        if f == self.fz_frequency {
            return Ok(()); // already done for this frequency
        }
        let len = self.len;
        let units = self.length_units.code();
        let earth_model = self.earth_model;
        let (eps, h_off, h_unit) = (self.eps_r_medium, self.height_offset, self.height_units);

        // Pascal builds a temporary `TLineGeometryObj` named after the Line and
        // loads the spacing + conductors into it (`LoadSpacingAndWires` runs the
        // Carson calc under this Line's `FEarthModel`). dss_capi 0.15.x
        // (Line.pas:2111-2113) pushes EpsRMedium/HeightOffset/HeightUnit into the
        // temporary geometry's engine *before* the Carson calc — threaded through
        // `load_spacing_and_wires` so they are applied before its internal
        // `update_line_geometry_data` (which folds `heightOffset` into the
        // equivalent-spacing average heights).
        let mut pgeo = LineGeometryObj::new(self.cd.obj.name().to_string());
        {
            let Some(spc) = self.line_spacing_obj.as_ref() else {
                return Err(format!(
                    "Line.{} has no LineSpacing to build its impedance from.",
                    self.cd.obj.name()
                ));
            };
            pgeo.name_errors_after_line(self.cd.obj.name(), spc.data().name());
            pgeo.load_spacing_and_wires(
                spc,
                &self.line_wire_data,
                f,
                earth_model,
                eps,
                h_off,
                h_unit,
            )?;
        }
        // r4133 Line.pas:2213-2219: after `LoadSpacingAndWires` compacts out the
        // `none` slots, the geometry's phase count may drop below the Line's
        // `phases=` (a `none` at a phase position removed a phase conductor). r4133
        // (and 0.14.5) raise #181021 and abort rather than silently re-phasing —
        // reproduce (both gating oracles reject; the user must set `phases=`
        // explicitly). A `none` in a NEUTRAL position leaves the phase count intact
        // and solves (the compaction path above).
        if self.cd.nphases as i32 != pgeo.nphases() {
            return Err(format!(
                "The number of valid phase conductors (not \"None\") in Line.{} is different \
                 than the number defined in its spacing. In this case, you must set the phases \
                 parameter of the line to the correct number (phases={}).",
                self.cd.obj.name(),
                pgeo.nphases()
            ));
        }
        // A `rho=` on the Line overrides the geometry's earth resistivity.
        if self.cd.obj.prp_specified(prop::RHO) {
            pgeo.set_rho_earth(self.rho);
        }
        let z = pgeo.z_matrix(f, len, units, earth_model)?;
        let yc = pgeo.yc_matrix(f, len, units, earth_model)?;
        self.z = Some(z);
        self.yc = Some(yc);
        self.fz_frequency = f;
        Ok(())
    }

    /// Pascal `TLineObj.DoLongLine` (Line.pas:1046). Long-line (distributed-
    /// parameter) correction of one sequence's per-unit-length `R/X/C` at the
    /// solution `frequency`, via the exact-PI hyperbolic factors
    /// `Zm = Zc·sinh(γl)`, `Ym = (2/Zc)·(cosh(γl)−1)/sinh(γl)`. `R`/`X`/`C` are
    /// the base-frequency per-unit-length inputs; the returned `_h` values are
    /// per-unit-length again (`Zm/Len`, `Ym.im/Len/2πf`), already carrying the
    /// frequency adjustment. Uses the RTL-faithful `Csqrt`/`Cinv`/Smith-`/`
    /// (`csqrt_fpc`/`cinv_fpc`/`compat::cdiv`) so the corrected YPrim matches the
    /// oracle bit-for-bit. Returns `(r_h, x_h, c_h, g_h)`.
    fn do_long_line(&self, frequency: f64, r: f64, x: f64, c: f64) -> (f64, f64, f64, f64) {
        let len = self.len;
        let two_pi = 2.0 * std::f64::consts::PI;
        // A tiny conductance so a C=0 line is not skipped (Pascal `G_h := EPSILON`).
        let g_h = EPSILON;
        let zs = Complex64::new(r * len, x * len * frequency / self.cd.base_frequency);
        let ys = Complex64::new(g_h, two_pi * frequency * c * len);

        let gamma_l = csqrt_fpc(zs * ys);
        let zc = csqrt_fpc(compat::cdiv(zs, ys));
        let exp_p = Complex64::new(gamma_l.im.cos(), gamma_l.im.sin()) * gamma_l.re.exp();
        let exp_m = cinv_fpc(exp_p);

        let sinh_gl = (exp_p - exp_m) * 0.5;
        let cosh_gl = (exp_p + exp_m) * 0.5;
        let zm = zc * sinh_gl;
        let ym = cinv_fpc(zc) * compat::cdiv(cosh_gl - 1.0, sinh_gl) * 2.0;

        let r_h = zm.re / len;
        let x_h = zm.im / len; // already at the desired frequency
        let c_h = ym.im / len / two_pi / frequency;
        (r_h, x_h, c_h, ym.re)
    }
}

impl CktElement for Line {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// The solve itself, run on a clone at the base frequency: the first
    /// message it queues, the singular series impedance (code 183) or the
    /// impedance a geometry or spacing cannot build. A geometry's message
    /// names the geometry alone, so this Line's name leads it, as
    /// `exec::tests::make_pos_seq::refusals::a_line_whose_geometry_cannot_build_its_impedance_stops_makeposseq`
    /// pins. The probe builds the impedance afresh and leaves the long-line
    /// correction off, which keeps a singular `Z` singular and a regular one
    /// regular, so it refuses what a solve that builds the impedance refuses.
    fn pos_seq_refusal(&self) -> Option<String> {
        let base = self.cd.base_frequency;
        let sys = SysCtx {
            frequency: base,
            fundamental: base,
            ..SysCtx::parse_default()
        };
        let mut probe = self.clone();
        probe.fz_frequency = -1.0;
        let _ = probe.cd.obj.take_errors();
        probe.calc_yprim(&sys);
        let first = probe.cd.obj.take_errors().into_iter().next()?;
        if first.yprim_unbuilt && self.geometry_obj.is_some() {
            return Some(format!("Line.{}: {}", self.cd.obj.name(), first.message));
        }
        Some(first.message)
    }

    fn line_length_km(&self) -> Option<f64> {
        Some(self.len * convert_line_units(self.length_units, LineUnits::Km))
    }

    /// Pascal `TLineObj.CalcFltRate` (l.1129): the base rate scaled by line
    /// length (`Faultrate · pctperm · 0.01 · Len`, faultrate in per-unit-length
    /// terms). `MilesThisLine` is maintained by the length/units side effects.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01 * self.len,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: self.miles_this_line,
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
    fn num_amp_ratings(&self) -> i32 {
        self.num_amp_ratings
    }
    fn amp_ratings(&self) -> &[crate::obj::Rating] {
        &self.amp_ratings
    }

    /// Pascal `TLineObj.GetSeqLosses` (Line.pas l.1495): pos/neg/zero-mode
    /// losses summed over both terminals — 3-phase branches only.
    fn get_seq_losses(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> (Complex64, Complex64, Complex64) {
        let mut pos = Complex64::ZERO;
        let mut neg = Complex64::ZERO;
        let mut zero = Complex64::ZERO;
        if self.cd.nphases == 3 {
            self.compute_iterminal(sys, node_v);
            let cd = &self.cd;
            let np = cd.nphases;
            let sc = SymComp::default();
            for i in 0..2 {
                let k = i * np;
                let vph: [Complex64; 3] = [
                    node_v[cd.node_ref[k]],
                    node_v[cd.node_ref[k + 1]],
                    node_v[cd.node_ref[k + 2]],
                ];
                let mut v012 = [Complex64::ZERO; 3];
                let mut i012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&vph, &mut v012);
                sc.phase_to_sym(&cd.iterminal[k..k + 3], &mut i012);
                pos += v012[1] * i012[1].conj();
                neg += v012[2] * i012[2].conj();
                zero += v012[0] * i012[0].conj();
            }
            pos *= 3.0;
            neg *= 3.0;
            zero *= 3.0;
        }
        (pos, neg, zero)
    }

    /// Pascal `TLineObj.CalcYPrim` (sym-component, matrix and geometry paths,
    /// including the SymComponentsModel long-line correction — see `do_long_line`
    /// and the `long_line` branches). Below `0.51 Hz` (GIC) the inverted series Z
    /// collapses to its positive-sequence resistance (`ConvertZinvToPosSeqR`) and
    /// the shunt capacitance is skipped.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;

        // Pascal `ClearYPrim` (Line.pas:1170, body at l.2061): zero out both the
        // Series and Shunt YPrims up front, so every early-exit below (a geometry
        // build error, or the singular-matrix abort) leaves this element
        // contributing nothing to Y. Pascal zeroes member matrices of order
        // `Yorder`; adding a zero primitive equals adding none, so we model the
        // cleared state as `None` (which `BuildYMatrix` skips). The success path
        // overwrites all three at the end.
        self.cd.yprim_series = None;
        self.cd.yprim_shunt = None;
        self.cd.yprim = None;

        // Build Z, Yc and the to-be-inverted Zinv. Two paths:
        //  - geometry: `FMakeZFromGeometry` makes the *total* Z/Yc (length and
        //    units already folded into the geometry's `Zmatrix[f, len, units]`),
        //    so Zinv = Z directly and the shunt needs no length/freq scaling.
        //  - sym/linecode: Z/Yc are per-unit-length at base frequency; scale by
        //    length/frequency and the earth-return Rg/Xg before inverting.
        // The geometry and spacing paths both produce a *total* Z/Yc (length +
        // units folded in by the geometry's `Zmatrix[f, len, units]`); the
        // sym/linecode path produces per-unit-length data scaled below.
        let geometry_path = self.geometry_obj.is_some();
        let spacing_path = self.spacing_specified;
        let total_z_path = geometry_path || spacing_path;
        // Pascal: long-line correction only enters for the SymComponentsModel
        // (per-unit-length) path (Line.pas:1199/1369). It is mutually exclusive
        // with the geometry/spacing total-Z path (which clears
        // sym_components_model).
        let long_line = self.sym_components_model && sys.long_line_correction;
        let mut length_multiplier = 1.0;
        let mut freq_multiplier = 1.0;

        let mut zinv = if total_z_path {
            // Pascal `FMakeZFromGeometry`/`FMakeZFromSpacing(Solution.Frequency)`.
            let res = if geometry_path {
                self.make_z_from_geometry(sys.frequency)
            } else {
                self.make_z_from_spacing(sys.frequency)
            };
            if let Err(msg) = res {
                // The impedance cannot be built: no YPrim, and a message that
                // the Y build (`build_y_matrix`) reports in the solve's one
                // error. Pinned by
                // `exec::tests::line_geometry_rules::a_y_build_abort_is_the_only_error_of_the_solve`.
                self.cd
                    .obj
                    .push_error(crate::diag::DssDiagnostic::yprim_unbuilt(msg));
                return;
            }
            // Pascal leaves `FYprimFreq` untouched in these branches (it is set
            // only in the per-unit-length else-block); they use no
            // `freq_multiplier`, so leave it likewise.
            self.z.as_ref().expect("make_z_from_* set Z").clone()
        } else {
            if self.sym_components_changed {
                // Catch inadvertent user error when C1/C0 were never specified:
                // adjust the kft-based defaults for the new length units.
                if !self.cap_specified {
                    self.c1 /= convert_line_units(LineUnits::Kft, self.length_units);
                    self.c0 /= convert_line_units(LineUnits::Kft, self.length_units);
                    self.cap_specified = true;
                }
                self.recalc(sys.positive_sequence);
            }

            // Z is from line data (per unit length, base frequency).
            length_multiplier = self.len / self.units_convert;
            self.cd.yprim_freq = sys.frequency;
            freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

            // Long-line correction (Line.pas:1199-1254): recompute the
            // per-unit-length Z/Yc at the solution frequency with the exact-PI
            // hyperbolic factors, sequence-by-sequence, and rebuild the phase
            // Z/Yc from the corrected symmetrical components. Frequency is
            // already folded into the corrected X_h/Yc_h here.
            if long_line {
                let f = self.cd.yprim_freq;
                let (r1h, x1h, c1h, g1h) = self.do_long_line(f, self.r1, self.x1, self.c1);
                let (r0h, x0h, c0h, g0h) = if nphases > 1 && !sys.positive_sequence {
                    self.do_long_line(f, self.r0, self.x0, self.c0)
                } else {
                    // Zero sequence the same as positive sequence.
                    (r1h, x1h, c1h, g1h)
                };

                let ztemp = Complex64::new(r1h, x1h) * 2.0;
                let zs = (ztemp + Complex64::new(r0h, x0h)) / 3.0;
                let zm = (Complex64::new(r0h, x0h) - Complex64::new(r1h, x1h)) / 3.0;

                let two_pi = 2.0 * std::f64::consts::PI;
                let yc1 = two_pi * f * c1h;
                let yc0 = two_pi * f * c0h;
                let ys = (Complex64::new(g1h, yc1) * 2.0 + Complex64::new(g0h, yc0)) / 3.0;
                let ym = (Complex64::new(g0h, yc0) - Complex64::new(g1h, yc1)) / 3.0;

                let mut z = CMatrix::new(nphases);
                let mut yc = CMatrix::new(nphases);
                for i in 0..nphases {
                    z.set(i, i, zs);
                    yc.set(i, i, ys);
                    for j in 0..i {
                        z.set(i, j, zm);
                        z.set(j, i, zm);
                        yc.set(i, j, ym);
                        yc.set(j, i, ym);
                    }
                }
                self.z = Some(z);
                self.yc = Some(yc);
            }

            // Put in series RL, corrected for length and frequency: Rg increases
            // with frequency, Xg is modified by ln of sqrt(1/f).
            let xgmod = if self.xg != 0.0 {
                0.5 * self.kxg * freq_multiplier.ln()
            } else {
                0.0
            };

            let z = self.z.as_ref().expect("recalc ran in the constructor");
            let mut zinv = CMatrix::new(nphases);
            for i in 0..nphases {
                for j in 0..nphases {
                    let zv = z.get(i, j);
                    // The long-line branch already applied the frequency
                    // adjustment to X_h, so freq_multiplier here scales ONLY the
                    // earth-return Xgmod term (Line.pas:1269 vs :1287).
                    let im = if long_line {
                        (zv.im - xgmod * freq_multiplier) * length_multiplier
                    } else {
                        (zv.im - xgmod) * length_multiplier * freq_multiplier
                    };
                    zinv.set(
                        i,
                        j,
                        Complex64::new(
                            (zv.re + self.rg * (freq_multiplier - 1.0)) * length_multiplier,
                            im,
                        ),
                    );
                }
            }
            zinv
        };

        // Build buffers; assigned to the element's YPrim only on success.
        let mut yp_series = CMatrix::new(yorder);
        let mut yp_shunt = CMatrix::new(yorder);
        let mut yprim = CMatrix::new(yorder);

        // A series impedance that cannot be told from a singular one in f64,
        // or whose inverse is not finite, has no admittance to stamp, so the
        // Line refuses every Y build until its impedance is valid (pinned by
        // `exec::tests::rating_refusals::a_typed_impedance_matrix_singular_within_its_rounding_is_refused`
        // and `a_line_whose_admittance_overflows_is_refused_at_every_solve`).
        if !self.invert_impedance(&mut zinv, true) {
            return;
        }

        // Pascal `TLineObj.ConvertZinvToPosSeqR` (Line.pas:1297/2086): for a GIC
        // (~dc) solution use only the positive-sequence *resistance* — re-invert
        // Zinv back to Z (length included), average the diagonal and (upper
        // triangle) off-diagonal elements, `Z1 = Zs − Zm` with the X part
        // dropped, then rebuild Zinv as the diagonal-only inverse. Cross-phase
        // coupling vanishes (matches the oracle's 0.1 Hz Line YPrim; the
        // pre-port Rust build kept the r0≠r1 coupling — a proven 33 % YPrim
        // divergence on `autotrans_gic`'s original switch line).
        if sys.frequency < 0.51 {
            // Re-invert Zinv back to Z with length included.
            if !self.invert_impedance(&mut zinv, false) {
                return;
            }
            let zs = zinv.avg_diagonal();
            let zm = zinv.avg_off_diagonal();
            let z1 = Complex64::new((zs - zm).re, 0.0); // ignore X part
            zinv.clear();
            for i in 0..zinv.order() {
                zinv.set(i, i, z1);
            }
            // Back to Zinv for inserting in Yprim: a Line with no resistance
            // is a short at dc.
            if !self.invert_impedance(&mut zinv, true) {
                return;
            }
        }

        yp_series.stamp_two_terminal_block(nphases, StampBl::Transposed, |i, j| zinv.get(i, j));

        yprim.copy_from(&yp_series); // initialize YPrim for series impedances

        // Increase diagonals of the series Yprim to avoid isolated buses.
        for i in 0..yorder {
            yp_series.add(i, i, CAP_EPSILON);
        }

        // Shunt: half the capacitive admittance at each end (skip for GIC).
        if sys.frequency > 0.51 {
            let yc = self.yc.as_ref().expect("Z/Yc built above");
            for j in 0..nphases {
                for i in 0..nphases {
                    let ycv = yc.get(i, j);
                    let value = if total_z_path {
                        // Already total (length + frequency folded in); halve it.
                        Complex64::new(ycv.re / 2.0, ycv.im / 2.0)
                    } else if long_line {
                        // Frequency already applied above during the Yc
                        // recalculation; keep the conductance real part and scale
                        // the susceptance by length only (Line.pas:1372).
                        Complex64::new(ycv.re / 2.0, ycv.im * length_multiplier / 2.0)
                    } else {
                        Complex64::new(0.0, ycv.im * length_multiplier * freq_multiplier / 2.0)
                    };
                    yp_shunt.add(i, j, value);
                    yp_shunt.add(i + nphases, j + nphases, value);
                }
            }
        }

        yprim.add_from(&yp_shunt);

        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim = Some(yprim);

        // Account for open conductors.
        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Collapse a multi-phase line to its positive-sequence single-phase
    /// equivalent. A line already single-phase is left alone and only the
    /// base bus rename runs. The property writes are returned as a
    /// [`PosSeqPlan`], and the set-order marks of the impedance properties are
    /// cleared here, for a cleaner save.
    ///
    /// Three source branches:
    /// - a switch: fixed switch constants (R1=1, X1=1, C1=1.1 nF, Phases=1,
    ///   Length=0.001).
    /// - the symmetrical components: keep the existing Z1 (R1, X1), C1 in nF.
    /// - matrix, geometry or spacing: Z1 is the mean diagonal minus the mean
    ///   off-diagonal term of `Z`. C1 is the positive-sequence self term of
    ///   `Yc`: `(Σ C_ii − Σ_{i<j} C_ij) / 3` for two or three phases (the
    ///   matrix embedded in three phases), the mean diagonal minus the mean
    ///   off-diagonal term above three. A matrix given per unit length is
    ///   divided by the length-units factor, which takes it to the Line's
    ///   length unit. The total matrices of a geometry or a spacing, built in
    ///   the Line's own unit, are divided by the length they hold alone, as
    ///   the `units=` leg of
    ///   `exec::tests::make_pos_seq::line_data::a_line_is_reduced_from_its_present_data_at_the_base_frequency`
    ///   pins. Pinned by
    ///   `elements::pd::line::tests::make_pos_sequence_matrix_branch` (three
    ///   phases, to the bit) and its two-phase and four-conductor siblings.
    ///
    /// A geometry or spacing Line is reduced from the impedance its present
    /// data give at the base frequency, built afresh here whatever the last
    /// build left: no build, a build before an edit of the conductors, the
    /// spacing, the length or its unit, or a build at another frequency (a
    /// solve at another frequency, a harmonics study). The pins are the legs of
    /// `exec::tests::make_pos_seq::line_data::a_line_is_reduced_from_its_present_data_at_the_base_frequency`,
    /// `a_line_no_solve_built_is_reduced_from_its_conductors` and
    /// `a_wires_edit_gives_the_line_of_those_wires`.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        use prop::*;

        // If already single phase, let alone: only the base bus rename runs.
        if self.cd.nphases <= 1 {
            return PosSeqPlan::base();
        }

        if !self.is_switch
            && !self.sym_components_model
            && (self.geometry_obj.is_some() || self.spacing_specified)
        {
            let base = self.cd.base_frequency;
            self.fz_frequency = -1.0;
            let built = if self.geometry_obj.is_some() {
                self.make_z_from_geometry(base)
            } else {
                self.make_z_from_spacing(base)
            };
            // The analysis built the same impedance on a clone and refused the
            // Line on any error (`pos_seq_refusal`), so the build succeeds.
            if let Err(message) = built {
                debug_assert!(false, "{message}");
                self.cd
                    .obj
                    .push_error(crate::diag::DssDiagnostic::yprim_unbuilt(message));
            }
        }

        // The ratings and the length units, read before the writes reset them.
        let norm_amps0 = self.norm_amps;
        let emerg_amps0 = self.emerg_amps;
        let length_units0 = self.length_units.code();

        let mut actions = vec![PosSeqAction::BeginEdit];

        // Clear the set-order marks of the impedance properties for a cleaner
        // save.
        for p in [LINECODE, R1, X1, R0, X0, C1, C0, RMATRIX, XMATRIX, CMATRIX] {
            self.cd.obj.clear_seq(p);
        }

        // A geometry or a spacing gives total matrices in the Line's own
        // length unit, which hold the length.
        let total = self.geometry_obj.is_some() || self.spacing_specified;
        let length_mult = if total { self.len } else { 1.0 };

        if self.is_switch {
            actions.push(PosSeqAction::SetF64(R1, 1.0));
            actions.push(PosSeqAction::SetF64(X1, 1.0));
            actions.push(PosSeqAction::SetF64(C1, 1.1));
            actions.push(PosSeqAction::SetI32(PHASES, 1));
            actions.push(PosSeqAction::SetF64(LENGTH, 0.001));
        } else {
            let (z1, c1_new) = if self.sym_components_model {
                // keep the same Z1 and C1
                (Complex64::new(self.r1, self.x1), self.c1 * 1.0e9)
            } else {
                // matrix was input directly, or built from physical data:
                // average the diagonal and off-diagonal elements.
                let z = self.z.as_ref().expect("Z built by the recalc or the build");
                let yc = self
                    .yc
                    .as_ref()
                    .expect("Yc built by the recalc or the build");
                let np = self.cd.nphases;
                let npf = np as f64;

                let mut zs = Complex64::new(0.0, 0.0);
                for i in 0..np {
                    zs += z.get(i, i);
                }
                zs /= npf * length_mult;
                let mut zm = Complex64::new(0.0, 0.0);
                for i in 0..np - 1 {
                    for j in i + 1..np {
                        zm += z.get(i, j);
                    }
                }
                zm /= length_mult * npf * (npf - 1.0) / 2.0;
                let mut z1 = zs - zm;

                // Do same for Capacitances.
                let mut cs = 0.0;
                for i in 0..np {
                    cs += yc.get(i, i).im;
                }
                let mut cm = 0.0;
                for i in 0..np - 1 {
                    for j in i + 1..np {
                        cm += yc.get(i, j).im;
                    }
                }
                let two_pi = 2.0 * std::f64::consts::PI;
                let mut c1_new = if np <= 3 {
                    // The self term of the matrix embedded in three phases.
                    (cs - cm) / two_pi / self.cd.base_frequency / (length_mult * 3.0) * 1.0e9
                } else {
                    // Mean self minus mean mutual.
                    (cs / npf - cm / (npf * (npf - 1.0) / 2.0))
                        / two_pi
                        / self.cd.base_frequency
                        / length_mult
                        * 1.0e9
                }; // nanofarads

                // A matrix per unit length takes the Line's length unit.
                if !total {
                    z1 /= self.units_convert;
                    c1_new /= self.units_convert;
                }
                (z1, c1_new)
            };
            actions.push(PosSeqAction::SetF64(R1, z1.re));
            actions.push(PosSeqAction::SetF64(X1, z1.im));
            actions.push(PosSeqAction::SetF64(C1, c1_new));
            actions.push(PosSeqAction::SetI32(PHASES, 1));
        }

        // Conductor Current Ratings (PD-element prop pair).
        actions.push(PosSeqAction::SetRating(NORMAMPS, norm_amps0));
        actions.push(PosSeqAction::SetRating(EMERGAMPS, emerg_amps0));
        // Repeat the Length Units to compensate for unexpected reset.
        actions.push(PosSeqAction::SetI32(UNITS, length_units0));
        actions.push(PosSeqAction::EndEdit);

        // The base bus rename runs afterwards.
        PosSeqPlan::with_actions(actions)
    }
}
