//! Tape-shield cable specialization, port of `General/TSLineConstants.pas`
//! (`TTSLineConstants`, deriving from `TCableConstants`). It adds the
//! tape-shield data and overrides `Calc`: each tape shield is appended as an
//! extra conductor with its shield resistance/GMR, the shields are Kron-reduced
//! out, and the capacitance is the coaxial insulation admittance.

use super::{E0, LineConstants, MU0, TWOPI, cmplx};
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::LineUnits;

/// `TTSLineConstants`: a [`LineConstants`] in the tape-shield kind.
pub type TsLineConstants = LineConstants;

// Resistivity of a copper tape shield (ohm-m), Pascal `RhoTS`.
const RHO_TS: f64 = 2.3718e-8;
// TODO(compat): upstream truncated `1/pi` literal in the tape-shield resistance
// formula (Pascal `0.3183`). The clean fix is `FRAC_1_PI` in the precision pass.
#[allow(clippy::approx_constant)]
const TS_RES_INV_PI: f64 = 0.3183;

impl LineConstants {
    /// `Set_DiaShield` — diameter over the tape shield.
    pub fn set_dia_shield(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fdia_shield[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `Set_TapeLayer` — thickness of the tape-shield layer.
    pub fn set_tape_layer(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.ftape_layer[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `Set_TapeLap` — tape-shield overlap, in percent (dimensionless).
    pub fn set_tape_lap(&mut self, i: usize, value: f64) {
        if i < self.num_conds {
            self.ftape_lap[i] = value;
        }
    }

    /// `TTSLineConstants.Calc(f, EarthModel)`.
    pub(super) fn calc_ts(&mut self, f: f64, earth_model: i32) {
        self.set_frequency(f); // side effects

        let reduced_size = self.fz_reduced.as_ref().map_or(0, |z| z.order());
        self.fz_reduced = None;
        self.fyc_reduced = None;

        self.fz_matrix.clear();
        self.fyc_matrix.clear();

        // Add the tape shields to the end of the conductor list; they are
        // always reduced out below.
        let n = self.num_conds + self.nphases;
        let mut zmat = CMatrix::new(n);

        // For less than 1 kHz use GMR to better match published data.
        let lfactor = cmplx(0.0, self.fw * MU0 / TWOPI);
        let power_freq = f < 1000.0 && f > 40.0;

        // Self impedances - TS cores and bare neutrals.
        for i in 0..self.num_conds {
            let mut zi = self.get_zint(i, earth_model);
            let zspacing = if power_freq {
                zi.im = 0.0;
                lfactor * (1.0 / self.fgmr[i]).ln()
            } else {
                lfactor * (1.0 / self.fradius[i]).ln()
            };
            zmat.set(i, i, zi + zspacing + self.get_ze(i, i, earth_model));
        }

        // TS self impedances.
        for i in 0..self.nphases {
            let res_ts = TS_RES_INV_PI * RHO_TS
                / (self.fdia_shield[i]
                    * self.ftape_layer[i]
                    * (50.0 / (100.0 - self.ftape_lap[i])).sqrt());
            // per Kersting, to center of the tape shield
            let gmr_ts = 0.5 * (self.fdia_shield[i] - self.ftape_layer[i]);
            let zspacing = lfactor * (1.0 / gmr_ts).ln();
            let zi = cmplx(res_ts, 0.0);
            let idxi = i + self.num_conds;
            zmat.set(idxi, idxi, zi + zspacing + self.get_ze(i, i, earth_model));
        }

        // Mutual impedances - between TS cores and bare neutrals.
        for i in 0..self.num_conds {
            for j in 0..i {
                let dij =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(i, j, z);
                zmat.set(j, i, z);
            }
        }

        // Mutual impedances - TS to other TS, cores, and bare neutrals.
        for i in 0..self.nphases {
            let idxi = i + self.num_conds;
            for j in 0..i {
                // TS to other TS
                let idxj = j + self.num_conds;
                let dij =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(idxi, idxj, z);
                zmat.set(idxj, idxi, z);
            }
            for j in 0..self.num_conds {
                // TS to cores and bare neutrals
                let gmr_ts = 0.5 * (self.fdia_shield[i] - self.ftape_layer[i]);
                let dij = if i == j {
                    // TS to its own phase core
                    gmr_ts
                } else {
                    // TS to another phase or bare neutral
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt()
                };
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(idxi, j, z);
                zmat.set(j, idxi, z);
            }
        }

        // Reduce out the tape shields (always the last rows/cols).
        while zmat.order() > self.num_conds {
            zmat = zmat.kron(zmat.order() - 1).expect("kron order > 1");
        }
        self.fz_matrix.copy_from(&zmat);

        // For shielded cables, build the capacitance matrix directly; assumes
        // the insulation may lie between semicon layers.
        for i in 0..self.nphases {
            let yfactor = TWOPI * E0 * self.feps_r[i] * self.fw; // includes f so C ⇒ Y
            let rad_out = 0.5 * self.fdia_ins[i];
            let rad_in = rad_out - self.fins_layer[i];
            let denom = (rad_out / rad_in).ln();
            self.fyc_matrix.set(i, i, cmplx(0.0, yfactor / denom));
        }

        if reduced_size > 0 {
            self.kron(reduced_size); // was reduced, so reduce again to same size
        }

        self.frho_changed = false;
    }
}
