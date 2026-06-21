//! The impedance/admittance numerics: building per-unit-length `Z`/`Yc` from
//! the symmetrical components (`recalc`), the geometry total-matrix build
//! (`make_z_from_geometry`), and the `impl CktElement` — `CalcYPrim`,
//! `GetSeqLosses`, the reliability/rating accessors.

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, ReliabilityData, SysCtx};
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::{LineUnits, convert_line_units};
use crate::support::mathutil::SymComp;

use super::Line;

/// Pascal `CAP_EPSILON` (Line.pas): "5 kvar of capacitive reactance at
/// 345 kV to avoid open line problem", added to the series Yprim diagonal.
const CAP_EPSILON: Complex64 = Complex64::new(0.0, 4.2e-8);

impl Line {
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

    /// Pascal `TLineObj.FMakeZFromGeometry` (Line.pas:1929): build the total
    /// `Z`/`Yc` (length + units already folded in) from the attached geometry at
    /// frequency `f`, recomputing only when `f` differs from the last build. The
    /// geometry's `ActiveEarthModel` is this Line's `FEarthModel`. Returns the
    /// geometry's error (Pascal raises `ELineGeometryProblem` + `SolutionAbort`).
    fn make_z_from_geometry(&mut self, f: f64) -> Result<(), String> {
        if f == self.fz_frequency {
            return Ok(()); // already done for this frequency
        }
        let len = self.len;
        let units = self.length_units.code();
        let earth_model = self.earth_model;
        let geom = self
            .geometry_obj
            .as_mut()
            .expect("make_z_from_geometry called without a geometry");
        let z = geom.z_matrix(f, len, units, earth_model)?;
        let yc = geom.yc_matrix(f, len, units, earth_model)?;
        self.z = Some(z);
        self.yc = Some(yc);
        self.fz_frequency = f;
        Ok(())
    }
}

impl CktElement for Line {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys.positive_sequence);
    }

    /// Pascal `TLineObj.CalcFltRate` (l.1129): the base rate scaled by line
    /// length (`Faultrate · pctperm · 0.01 · Len`, faultrate in per-unit-length
    /// terms). `MilesThisLine` is maintained by the length/units side effects.
    fn reliability_data(&self) -> ReliabilityData {
        ReliabilityData {
            branch_flt_rate: self.fault_rate * self.pct_perm * 0.01 * self.len,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: self.miles_this_line,
        }
    }

    fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    fn emerg_amps(&self) -> f64 {
        self.emerg_amps
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

    /// Pascal `TLineObj.CalcYPrim` (sym-component, matrix and geometry paths;
    /// the long-line correction and the <0.51 Hz GIC conversion are Phase 7+).
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
        let geometry_path = self.geometry_obj.is_some();
        let mut length_multiplier = 1.0;
        let mut freq_multiplier = 1.0;

        let mut zinv = if geometry_path {
            // Pascal `FMakeZFromGeometry(Solution.Frequency)`.
            if let Err(msg) = self.make_z_from_geometry(sys.frequency) {
                // Pascal: the geometry getter raised `ELineGeometryProblem` and
                // set `SolutionAbort`, so `CalcYPrim` exits without building YPrim.
                // Record the message as a deferred error; the Y-build loop
                // (`build_y_matrix`) drains it, surfaces it, and sets
                // `solution_abort` — the faithful equivalent of the upstream
                // `SolutionAbort` + `Exit`.
                self.cd.obj.push_error(msg);
                return;
            }
            // Pascal leaves `FYprimFreq` untouched in the geometry branch (it is
            // set only in the per-unit-length else-block); this path uses no
            // `freq_multiplier`, so leave it likewise.
            self.z.as_ref().expect("make_z_from_geometry set Z").clone()
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
                    zinv.set(
                        i,
                        j,
                        Complex64::new(
                            (zv.re + self.rg * (freq_multiplier - 1.0)) * length_multiplier,
                            (zv.im - xgmod) * length_multiplier * freq_multiplier,
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

        if zinv.invert().is_err() {
            // Pascal error 183 (TLineObj.CalcYPrim, Line.pas:1300): a singular
            // series impedance. `DoErrorMsg` sets `SolutionAbort := True`
            // *unconditionally* (DSSGlobals.pas:265), so the solve aborts in
            // both `DSS_CAPI_EARLY_ABORT` modes — and `BuildYMatrix` then Exits
            // on `SolutionAbort` before adding any primitive. The oracle default
            // is `EARLY_ABORT = True` (DSSGlobals.pas:781: env <> '0'), whose
            // branch emits "Aborting solution." and exits without building YPrim
            // (confirmed live: both modes raise the #183 "Y matrix build aborted"
            // exception).
            //
            // NOT_PORTED: the `EARLY_ABORT = False` branch (DoErrorMsg "Replaced
            // with tiny conductance." then embed `epsilon·I`) is dead weight —
            // that YPrim is discarded by the abort — so we model only the abort.
            // YPrim was already cleared above (Pascal `ClearYPrim`). Record the
            // message; the Y-build loop drains it and sets `solution_abort`.
            self.cd.obj.push_error(format!(
                "Matrix Inversion Error for Line \"{}\". \
                 Invalid impedance specified. Aborting solution.",
                self.cd.obj.name()
            ));
            return;
        }
        for i in 0..nphases {
            for j in 0..nphases {
                let value = zinv.get(i, j);
                yp_series.set(i, j, value);
                yp_series.set(i + nphases, j + nphases, value);
                yp_series.set(i, j + nphases, -value);
                yp_series.set(j + nphases, i, -value);
            }
        }

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
                    let value = if geometry_path {
                        // Already total (length + frequency folded in); halve it.
                        Complex64::new(ycv.re / 2.0, ycv.im / 2.0)
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
}
