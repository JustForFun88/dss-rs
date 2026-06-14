//! Carson line-constants engine, port of `General/LineConstants.pas`
//! (`TLineConstants`). Computes the series impedance `Z` (ohms/m) and shunt
//! capacitance admittance `Yc` (siemens/m) of a multi-conductor line from its
//! geometry via Carson's equations, with the earth-return term selected by the
//! `EarthModel` (Carson / FullCarson / Deri). Frequency is a parameter of
//! [`LineConstants::calc`]: power flow uses the base frequency; harmonics
//! re-`calc` per harmonic (WP7.6).
//!
//! Indices are 0-based here (Pascal used 1..FNumConds); the ground-node
//! convention is unaffected — these are conductor indices.

use crate::support::cmatrix::CMatrix;
use crate::support::line_units::LineUnits;
use crate::support::mathutil::{bessel_i0, bessel_i1};
use num_complex::Complex64;

pub mod oh;
pub use oh::OhLineConstants;

/// Earth-model codes (Pascal `DSSGlobals` constants).
pub const SIMPLE_CARSON: i32 = 1;
pub const FULL_CARSON: i32 = 2;
pub const DERI: i32 = 3;

// Pascal `LineConstants` unit constants.
//
// TODO(compat): these reproduce the upstream truncated literals exactly —
// `mu0` and `Twopi` are short of the precise values, and `Twopi` is used as a
// distinct quantity from `2*pi` (the FullCarson/Zint earth terms use the full
// `PI`). Goldens pin them; the clean fix (precise constants) lands in the
// post-port precision pass.
const E0: f64 = 8.854e-12; // dielectric constant F/m
const MU0: f64 = 12.56637e-7; // hy/m
#[allow(clippy::approx_constant)] // TODO(compat): truncated upstream `Twopi`, not `TAU`
const TWOPI: f64 = 6.283185307;

#[inline]
fn cmplx(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

/// Pascal `TLineConstants`: the conductor coordinate/parameter arrays plus the
/// computed `Z`/`Yc` matrices. Conductor parameters are stored internally in
/// meters / ohms-per-meter (the setters convert from the supplied units).
pub struct LineConstants {
    num_conds: usize,
    nphases: usize,

    fx: Vec<f64>,
    fy: Vec<f64>,
    frdc: Vec<f64>,       // ohms/m
    frac: Vec<f64>,       // ohms/m
    fgmr: Vec<f64>,       // m
    fradius: Vec<f64>,    // m
    fcapradius: Vec<f64>, // m; <0 ⇒ defaults to fradius

    fz_matrix: CMatrix,  // ohms/m
    fyc_matrix: CMatrix, // siemens/m  (jwC)

    fz_reduced: Option<CMatrix>, // exist only after a Kron reduction
    fyc_reduced: Option<CMatrix>,

    ffrequency: f64, // frequency for which impedances are computed
    fw: f64,         // 2*pi*f (truncated twopi)
    frho_earth: f64, // ohm-m
    fme: Complex64,  // factor for earth impedance
    frho_changed: bool,
}

impl LineConstants {
    /// `TLineConstants.Create(NumConductors)`.
    pub fn new(num_conductors: usize) -> Self {
        let n = num_conductors;
        LineConstants {
            num_conds: n,
            nphases: n,
            fx: vec![0.0; n],
            fy: vec![0.0; n],
            // Pascal initializes these four to "not set" (-1.0); FRac/FX/FY are
            // left zero by Allocmem.
            frdc: vec![-1.0; n],
            frac: vec![0.0; n],
            fgmr: vec![-1.0; n],
            fradius: vec![-1.0; n],
            fcapradius: vec![-1.0; n],
            fz_matrix: CMatrix::new(n),
            fyc_matrix: CMatrix::new(n),
            fz_reduced: None,
            fyc_reduced: None,
            ffrequency: -1.0, // not computed
            fw: 0.0,
            frho_earth: 100.0, // default value
            fme: Complex64::ZERO,
            frho_changed: true,
        }
    }

    pub fn num_conductors(&self) -> usize {
        self.num_conds
    }

    pub fn nphases(&self) -> usize {
        self.nphases
    }

    /// `Nphases` write (`set_Nphases`).
    pub fn set_nphases(&mut self, value: usize) {
        self.nphases = value;
    }

    pub fn rho_earth(&self) -> f64 {
        self.frho_earth
    }

    // ---- per-conductor setters (units → meters / per-meter) ----
    // `units` is a Pascal `LineUnits` integer code. Indices are 0-based.

    pub fn set_x(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fx[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    pub fn set_y(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fy[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    pub fn set_rdc(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.frdc[i] = value * LineUnits::from_code(units).to_per_meter();
        }
    }

    pub fn set_rac(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.frac[i] = value * LineUnits::from_code(units).to_per_meter();
        }
    }

    /// `Set_GMR`: also defaults the radius to the equivalent round conductor
    /// when radius is still unset.
    pub fn set_gmr(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fgmr[i] = value * LineUnits::from_code(units).to_meters();
            if self.fradius[i] < 0.0 {
                self.fradius[i] = self.fgmr[i] / 0.7788; // equivalent round conductor
            }
        }
    }

    /// `Set_radius`: also defaults the GMR to the round-conductor value when GMR
    /// is still unset.
    pub fn set_radius(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fradius[i] = value * LineUnits::from_code(units).to_meters();
            if self.fgmr[i] < 0.0 {
                self.fgmr[i] = self.fradius[i] * 0.7788; // default to round conductor
            }
        }
    }

    pub fn set_capradius(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fcapradius[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `Set_Frequency`: side effects on `Fw` and the earth factor `Fme`.
    fn set_frequency(&mut self, value: f64) {
        self.ffrequency = value;
        self.fw = TWOPI * self.ffrequency;
        self.fme = cmplx(0.0, self.fw * MU0 / self.frho_earth).sqrt();
    }

    /// `Set_Frhoearth`.
    pub fn set_rho_earth(&mut self, value: f64) {
        if value != self.frho_earth {
            self.frho_changed = true;
        }
        self.frho_earth = value;
        if self.ffrequency >= 0.0 {
            self.fme = cmplx(0.0, self.fw * MU0 / self.frho_earth).sqrt();
        }
    }

    /// `Get_Zint(i, EarthModel)`: internal impedance of conductor `i`.
    fn get_zint(&self, i: usize, earth_model: i32) -> Complex64 {
        match earth_model {
            // SimpleCarson / FullCarson: no skin effect.
            FULL_CARSON => cmplx(self.frac[i], self.fw * MU0 / (8.0 * std::f64::consts::PI)),
            DERI => {
                // with skin effect model; assume round conductor
                let c1_j1 = cmplx(1.0, 1.0);
                let alpha = c1_j1 * (self.ffrequency * MU0 / self.frdc[i]).sqrt();
                let i0i1 = if alpha.norm() > 35.0 {
                    Complex64::new(1.0, 0.0)
                } else {
                    bessel_i0(alpha) / bessel_i1(alpha)
                };
                c1_j1 * i0i1 * ((self.frdc[i] * self.ffrequency * MU0).sqrt() / 2.0)
            }
            // SIMPLECARSON and any other code take the no-skin-effect branch.
            _ => cmplx(self.frac[i], self.fw * MU0 / (8.0 * std::f64::consts::PI)),
        }
    }

    /// `Get_Ze(i, j, EarthModel)`: earth-return impedance for the `ij` element.
    fn get_ze(&self, i: usize, j: usize, earth_model: i32) -> Complex64 {
        let pi = std::f64::consts::PI;
        let fyi = self.fy[i].abs();
        let fyj = self.fy[j].abs();

        match earth_model {
            SIMPLE_CARSON => cmplx(
                self.fw * MU0 / 8.0,
                (self.fw * MU0 / TWOPI) * (658.5 * (self.frho_earth / self.ffrequency).sqrt()).ln(),
            ),
            FULL_CARSON => {
                // notation from Tleis, Power System Modelling and Fault Analysis
                let b1 = 1.0 / (3.0 * 2.0_f64.sqrt());
                let b2 = 1.0 / 16.0;
                let b3 = (1.0 / (3.0 * 2.0_f64.sqrt())) / 3.0 / 5.0;
                let b4 = (1.0 / 16.0) / 4.0 / 6.0;
                let d2 = (1.0 / 16.0) * pi / 4.0;
                let d4 = ((1.0 / 16.0) / 4.0 / 6.0) * pi / 4.0;
                let c2: f64 = 1.3659315;
                let c4: f64 = 1.3659315 + 1.0 / 4.0 + 1.0 / 6.0;

                let (thetaij, dij) = if i == j {
                    (0.0, 2.0 * fyi)
                } else {
                    let dij = ((fyi + fyj).powi(2) + (self.fx[i] - self.fx[j]).powi(2)).sqrt();
                    (((fyi + fyj) / dij).acos(), dij)
                };
                let mij = 2.8099e-3 * dij * (self.ffrequency / self.frho_earth).sqrt();

                let re = pi / 8.0 - b1 * mij * thetaij.cos()
                    + b2 * mij.powi(2)
                        * ((c2.exp() / mij).ln() * (2.0 * thetaij).cos()
                            + thetaij * (2.0 * thetaij).sin())
                    + b3 * mij * mij * mij * (3.0 * thetaij).cos()
                    - d4 * mij * mij * mij * mij * (4.0 * thetaij).cos();

                let term1 = 0.5 * (1.85138 / mij).ln();
                let term2 = b1 * mij * thetaij.cos();
                let term3 = -d2 * mij.powi(2) * (2.0 * thetaij).cos();
                let term4 = b3 * mij * mij * mij * (3.0 * thetaij).cos();
                let term5 = -b4
                    * mij
                    * mij
                    * mij
                    * mij
                    * ((c4.exp() / mij).ln() * (4.0 * thetaij).cos()
                        + thetaij * (4.0 * thetaij).sin());
                let mut im = term1 + term2 + term3 + term4 + term5;
                im += 0.5 * dij.ln(); // correction term to work with DSS structure

                cmplx(re, im) * (self.fw * MU0 / pi)
            }
            // DERI (and default)
            _ => {
                if i != j {
                    let hterm = cmplx(fyi + fyj, 0.0) + self.fme.inv() * 2.0;
                    let xterm = cmplx(self.fx[i] - self.fx[j], 0.0);
                    let ln_arg = (hterm * hterm + xterm * xterm).sqrt();
                    cmplx(0.0, self.fw * MU0 / TWOPI) * ln_arg.ln()
                } else {
                    let hterm = cmplx(fyi, 0.0) + self.fme.inv();
                    cmplx(0.0, self.fw * MU0 / TWOPI) * (hterm * 2.0).ln()
                }
            }
        }
    }

    /// `Calc(f, EarthModel)`: compute base `Z` and `Yc` matrices (ohms/m,
    /// siemens/m) for this frequency and earth impedance.
    pub fn calc(&mut self, f: f64, earth_model: i32) {
        self.set_frequency(f); // side effects

        // Free any reduced matrices, remembering the reduced size to redo it.
        let reduced_size = self.fz_reduced.as_ref().map_or(0, |z| z.order());
        self.fz_reduced = None;
        self.fyc_reduced = None;

        self.fz_matrix.clear();
        self.fyc_matrix.clear();

        // For less than 1 kHz use GMR to better match published data.
        let lfactor = cmplx(0.0, self.fw * MU0 / TWOPI);
        let power_freq = f < 1000.0 && f > 40.0;

        // Self impedances
        for i in 0..self.num_conds {
            let mut zi = self.get_zint(i, earth_model);
            let zspacing = if power_freq {
                zi.im = 0.0; // for less than 1 kHz, use published GMR
                lfactor * (1.0 / self.fgmr[i]).ln()
            } else {
                lfactor * (1.0 / self.fradius[i]).ln()
            };
            self.fz_matrix
                .set(i, i, zi + zspacing + self.get_ze(i, i, earth_model));
        }

        // Mutual impedances
        for i in 0..self.num_conds {
            for j in 0..i {
                let dij =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                self.fz_matrix.set(i, j, z);
                self.fz_matrix.set(j, i, z);
            }
        }

        // Capacitance matrix: construct P matrix then invert.
        let pfactor = -1.0 / TWOPI / E0 / self.fw; // include frequency

        // Self uses capradius, which defaults to actual conductor radius.
        for i in 0..self.num_conds {
            let r = if self.fcapradius[i] < 0.0 {
                self.fradius[i]
            } else {
                self.fcapradius[i]
            };
            self.fyc_matrix
                .set(i, i, cmplx(0.0, pfactor * (2.0 * self.fy[i] / r).ln()));
        }
        for i in 0..self.num_conds {
            for j in 0..i {
                let dij =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
                // distance to image j
                let dijp =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] + self.fy[j]).powi(2)).sqrt();
                let v = cmplx(0.0, pfactor * (dijp / dij).ln());
                self.fyc_matrix.set(i, j, v);
                self.fyc_matrix.set(j, i, v);
            }
        }

        // now should be nodal C matrix (ignore singularity like Pascal does)
        let _ = self.fyc_matrix.invert();

        if reduced_size > 0 {
            self.kron(reduced_size); // was reduced, so reduce again to same size
        }

        self.frho_changed = false;
    }

    /// `Kron(Norder)`: Kron-reduce to leave the first `norder` rows/cols.
    pub fn kron(&mut self, norder: usize) {
        if self.ffrequency >= 0.0 && norder > 0 && norder < self.num_conds {
            // Reduce the computed Z matrix one row/col (always the last) at a
            // time until it is `norder`.
            let mut ztemp = self.fz_matrix.clone();
            while ztemp.order() > norder {
                // Pascal eliminates the last row (1-based Order ⇒ 0-based last).
                ztemp = ztemp.kron(ztemp.order() - 1).expect("kron order > 1");
            }
            self.fz_reduced = Some(ztemp);

            // Extract the norder x norder portion of the Yc matrix.
            let mut ycr = CMatrix::new(norder);
            for i in 0..norder {
                for j in 0..norder {
                    ycr.set(i, j, self.fyc_matrix.get(i, j));
                }
            }
            self.fyc_reduced = Some(ycr);
        }
    }

    /// `Reduce`: Kron-reduce down to the phase conductors only.
    pub fn reduce(&mut self) {
        self.kron(self.nphases);
    }

    /// `Get_Zmatrix(f, Lngth, Units, EarthModel)`: a new Z matrix corrected for
    /// length and units; recalcs if the frequency or earth rho changed. Uses the
    /// reduced matrix when present.
    pub fn z_matrix(&mut self, f: f64, length: f64, units: i32, earth_model: i32) -> CMatrix {
        if f != self.ffrequency || self.frho_changed {
            self.calc(f, earth_model);
        }
        let z = self.fz_reduced.as_ref().unwrap_or(&self.fz_matrix);
        let mut result = z.clone();
        let conv = LineUnits::from_code(units).from_per_meter() * length;
        for v in result.values_mut() {
            *v *= conv;
        }
        result
    }

    /// `Get_YCmatrix(f, Lngth, Units)`: a new Yc matrix corrected for length and
    /// units. Uses the reduced matrix when present.
    pub fn yc_matrix(&self, length: f64, units: i32) -> CMatrix {
        let yc = self.fyc_reduced.as_ref().unwrap_or(&self.fyc_matrix);
        let mut result = yc.clone();
        let conv = LineUnits::from_code(units).from_per_meter() * length;
        for v in result.values_mut() {
            *v *= conv;
        }
        result
    }

    /// Read access to the base (unreduced) Z matrix (ohms/m), for tests and the
    /// frequency-sweep path.
    pub fn z_base(&self) -> &CMatrix {
        &self.fz_matrix
    }

    /// Read access to the base (unreduced) Yc matrix (siemens/m).
    pub fn yc_base(&self) -> &CMatrix {
        &self.fyc_matrix
    }

    /// `Fw` (2*pi*f, truncated twopi) for the present frequency.
    pub fn omega(&self) -> f64 {
        self.fw
    }

    /// `ConductorsInSameSpace`: validates geometry; returns an error message
    /// when a conductor height is ≤ 0 or two conductors overlap.
    pub fn conductors_in_same_space(&self) -> Option<String> {
        // Check for 0 (or negative) Y coordinate.
        for i in 0..self.num_conds {
            if self.fy[i] <= 0.0 {
                return Some(format!("Conductor {} height must be  > 0. ", i + 1));
            }
        }
        // Check for overlapping conductors.
        for i in 0..self.num_conds {
            for j in (i + 1)..self.num_conds {
                let dij =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
                if dij < (self.fradius[i] + self.fradius[j]) {
                    return Some(format!(
                        "Conductors {} and {} occupy the same space.",
                        i + 1,
                        j + 1
                    ));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
