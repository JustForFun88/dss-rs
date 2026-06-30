//! Concentric-neutral cable specialization, port of
//! `General/CNLineConstants.pas` (`TCNLineConstants`, deriving from
//! `TCableConstants`). It adds the stranded-neutral data and overrides `Calc`:
//! the concentric neutrals are appended as extra conductors, the impedance is
//! built with the strand resistance/GMR, the neutrals are Kron-reduced out, and
//! the capacitance is built directly as the coaxial insulation admittance.

use super::{E0, LineConstants, MU0, TWOPI, cmplx};
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::LineUnits;

/// `TCNLineConstants`: a [`LineConstants`] in the concentric-neutral kind.
pub type CnLineConstants = LineConstants;

impl LineConstants {
    /// `Set_kStrand` — number of concentric-neutral strands.
    pub fn set_k_strand(&mut self, i: usize, value: i32) {
        if i < self.num_conds {
            self.fk_strand[i] = value;
        }
    }

    /// `Set_DiaStrand` — diameter of a neutral strand.
    pub fn set_dia_strand(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fdia_strand[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `Set_GmrStrand` — GMR of a neutral strand.
    pub fn set_gmr_strand(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fgmr_strand[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `Set_RStrand` — AC resistance of a neutral strand.
    pub fn set_r_strand(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.frstrand[i] = value * LineUnits::from_code(units).to_per_meter();
        }
    }

    /// `TCNLineConstants.Calc(f, EarthModel)`.
    pub(super) fn calc_cn(&mut self, f: f64, earth_model: i32) {
        self.set_frequency(f); // side effects

        let reduced_size = self.fz_reduced.as_ref().map_or(0, |z| z.order());
        self.fz_reduced = None;
        self.fyc_reduced = None;

        self.fz_matrix.clear();
        self.fyc_matrix.clear();

        // Add concentric neutrals to the end of the conductor list; they are
        // always reduced out below.
        let n = self.num_conds + self.nphases;
        let mut zmat = CMatrix::new(n);

        // For less than 1 kHz use GMR to better match published data.
        let lfactor = cmplx(0.0, self.fw * MU0 / TWOPI);
        let power_freq = f < 1000.0 && f > 40.0;

        // Self impedances - CN cores and bare neutrals.
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

        // CN self impedances.
        for i in 0..self.nphases {
            let k = self.fk_strand[i] as f64;
            let res_cn = self.frstrand[i] / k;
            let rad_cn = 0.5 * (self.fdia_cable[i] - self.fdia_strand[i]);
            let gmr_cn = (self.fgmr_strand[i] * k * rad_cn.powf(k - 1.0)).powf(1.0 / k);
            let zspacing = lfactor * (1.0 / gmr_cn).ln();
            let zi = cmplx(res_cn, 0.0);
            let idxi = i + self.num_conds;
            zmat.set(idxi, idxi, zi + zspacing + self.get_ze(i, i, earth_model));
        }

        // Mutual impedances - between CN cores and bare neutrals.
        for i in 0..self.num_conds {
            for j in 0..i {
                let dij =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(i, j, z);
                zmat.set(j, i, z);
            }
        }

        // Mutual impedances - CN to other CN, cores, and bare neutrals.
        for i in 0..self.nphases {
            let idxi = i + self.num_conds;
            for j in 0..i {
                // CN to other CN
                let idxj = j + self.num_conds;
                let dij =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(idxi, idxj, z);
                zmat.set(idxj, idxi, z);
            }
            for j in 0..self.num_conds {
                // CN to cores and bare neutrals
                let rad_cn = 0.5 * (self.fdia_cable[i] - self.fdia_strand[i]);
                let dij = if i == j {
                    // CN to its own phase core
                    rad_cn
                } else {
                    // CN to another phase or bare neutral
                    let d = ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2))
                        .sqrt();
                    let k = self.fk_strand[i] as f64;
                    (d.powf(k) - rad_cn.powf(k)).powf(1.0 / k)
                };
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(idxi, j, z);
                zmat.set(j, idxi, z);
            }
        }

        // Reduce out the CN (always the last rows/cols).
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
