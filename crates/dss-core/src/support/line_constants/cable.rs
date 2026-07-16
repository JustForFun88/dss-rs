//! The merged coaxial-cable engine, port of `General/CableConstants.pas`
//! (`TCableConstants`). dss_capi 0.15.x merged the former
//! `TCNLineConstants`/`TTSLineConstants` into this single class: the CN-vs-TS
//! choice is per-conductor (`FCondType[i]`, [`super::ConductorType`]) rather
//! than a whole-engine kind, so one engine carries mixed wire/CN/TS conductors
//! (Kersting mixed-conductor model). This module owns the cable-data setters,
//! the per-conductor `SetCondType`/`SetSemiconLayer` state, the merged
//! `Calc(f, EarthModel)`, and the cable `ConductorsInSameSpace`.
//!
//! `TCableConstants.Kron` is byte-for-byte identical to the base `Kron`, so it
//! is not re-implemented — [`LineConstants::kron`] serves both.
//!
//! Numerics: a pure-CN (all `FCondType=CN`) or pure-TS geometry reproduces the
//! former `TCNLineConstants`/`TTSLineConstants.Calc` bit-for-bit — the merged
//! `Calc` restricted to a single conductor type is the old `Calc`, and the new
//! `GetDij` helper equals the old raw `sqrt` distance in the default
//! (non-equivalent-spacing) path.

use super::{ConductorType, E0, LineConstants, MU0, TWOPI, cmplx};
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::LineUnits;

// For TS: resistivity of a copper tape shield (ohm-m), Pascal `RhoTS`.
const RHO_TS: f64 = 2.3718e-8;
// TODO(compat): upstream truncated `1/pi` literal in the tape-shield resistance
// formula (Pascal `0.3183`). The clean fix is `FRAC_1_PI` in the precision pass.
#[allow(clippy::approx_constant)]
const TS_RES_INV_PI: f64 = 0.3183;

impl LineConstants {
    /// `SetEpsR` — relative permittivity of the insulation (dimensionless).
    pub fn set_eps_r(&mut self, i: usize, value: f64) {
        if i < self.num_conds {
            self.feps_r[i] = value;
        }
    }

    /// `SetInsLayer` — thickness of the insulation layer.
    pub fn set_ins_layer(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fins_layer[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `SetDiaIns` — diameter over the insulation.
    pub fn set_dia_ins(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fdia_ins[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `SetDiaCable` — diameter over the cable.
    pub fn set_dia_cable(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fdia_cable[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `SetCondType` — assign conductor `i`'s cable kind (CN/TS/Bare). A plain
    /// overhead wire conductor is left `INVALID` (no cable branch), as Pascal
    /// only calls `SetCondType` for `TCNDataObj`/`TTSDataObj` conductors.
    pub fn set_cond_type(&mut self, i: usize, value: ConductorType) {
        if i < self.num_conds {
            self.fcond_type[i] = value;
        }
    }

    /// `SetSemiconLayer` — per-conductor semicon-layer flag (CN capacitance
    /// branch). `CNData` defaults it `true` (the classic `ln(RadOut/RadIn)`
    /// formula); `false` selects the Synergi / Kersting no-semicon formula.
    pub fn set_semicon_layer(&mut self, i: usize, value: bool) {
        if i < self.num_conds {
            self.fsemicon_layer[i] = value;
        }
    }

    /// Pascal local `GetDij(i, j)` in `TCableConstants.Calc`: the conductor
    /// spacing distance. In the default (detailed) model it is the Euclidean
    /// coordinate distance; under equivalent spacing it is the equivalent
    /// phase-neutral distance for a phase-to-neutral pair, else the phase-phase
    /// distance (0-based: conductor `k` is a phase iff `k < nphases`).
    fn cable_dij(&self, i: usize, j: usize) -> f64 {
        if !self.equivalent_spacing {
            return ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
        }
        if j < self.nphases && i >= self.nphases {
            return self.eq_dist_ph_n;
        }
        self.eq_dist_ph_ph
    }

    /// `TCableConstants.Calc(f, EarthModel)` — merged CN/TS impedance and
    /// coaxial capacitance, branching per conductor on `FCondType[i]`. Compute
    /// base `Z` (ohms/m) and `Yc` (siemens/m).
    pub(super) fn calc_cable(&mut self, f: f64, earth_model: i32) {
        self.set_frequency(f); // side effects

        let reduced_size = self.fz_reduced.as_ref().map_or(0, |z| z.order());
        self.fz_reduced = None;
        self.fyc_reduced = None;

        self.fz_matrix.clear();
        self.fyc_matrix.clear();

        // Add concentric neutrals or tape shields to the end of the conductor
        // list; they are always reduced out below.
        let n = self.num_conds + self.nphases;
        let mut zmat = CMatrix::new(n);

        // For less than 1 kHz use GMR to better match published data.
        let lfactor = cmplx(0.0, self.fw * MU0 / TWOPI);
        let power_freq = f < 1000.0 && f > 40.0;

        // Pascal `Dij` is a function-level local: in the mutual `case
        // FCondType[i]` an `INVALID`/`Bare` phase leaves it at its previous
        // value (a degenerate config never exercised by real decks — a bare
        // wire is always a neutral, never a phase). A shared `mut` (0-seeded)
        // reproduces the persistence; every real (CN/TS/wire-neutral) path sets
        // it before use.
        let mut dij: f64 = 0.0;

        // Self impedances - CN/TS cores and bare neutrals.
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

        // CN/TS self impedances (per conductor type).
        for i in 0..self.nphases {
            match self.fcond_type[i] {
                ConductorType::Cn => {
                    let k = self.fk_strand[i] as f64;
                    let res_cn = self.frstrand[i] / k;
                    let rad_cn = 0.5 * (self.fdia_cable[i] - self.fdia_strand[i]);
                    let gmr_cn = (self.fgmr_strand[i] * k * rad_cn.powf(k - 1.0)).powf(1.0 / k);
                    let zspacing = lfactor * (1.0 / gmr_cn).ln();
                    let zi = cmplx(res_cn, 0.0);
                    let idxi = i + self.num_conds;
                    zmat.set(idxi, idxi, zi + zspacing + self.get_ze(i, i, earth_model));
                }
                ConductorType::Ts => {
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
                ConductorType::Invalid | ConductorType::Bare => {}
            }
        }

        // Mutual impedances - between cable cores and bare neutrals.
        for i in 0..self.num_conds {
            for j in 0..i {
                dij = self.cable_dij(i, j);
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(i, j, z);
                zmat.set(j, i, z);
            }
        }

        // Mutual impedances - CN/TS to other CN/TS, cores, and bare neutrals.
        for i in 0..self.nphases {
            let idxi = i + self.num_conds;
            for j in 0..i {
                // CN/TS to other CN/TS
                let idxj = j + self.num_conds;
                dij = self.cable_dij(i, j);
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(idxi, idxj, z);
                zmat.set(idxj, idxi, z);
            }
            for j in 0..self.num_conds {
                // CN/TS to cores and bare neutrals
                match self.fcond_type[i] {
                    ConductorType::Cn => {
                        let rad_cn = 0.5 * (self.fdia_cable[i] - self.fdia_strand[i]);
                        if i == j {
                            // CN to its own phase core
                            dij = rad_cn;
                        } else {
                            // CN to another phase or bare neutral
                            let d = self.cable_dij(i, j);
                            let k = self.fk_strand[i] as f64;
                            dij = (d.powf(k) - rad_cn.powf(k)).powf(1.0 / k);
                        }
                    }
                    ConductorType::Ts => {
                        let gmr_ts = 0.5 * (self.fdia_shield[i] - self.ftape_layer[i]);
                        if i == j {
                            // TS to its own phase core
                            dij = gmr_ts;
                        } else {
                            // TS to another phase or bare neutral
                            dij = self.cable_dij(i, j);
                        }
                    }
                    // INVALID/Bare phase: `dij` keeps its previous value (Pascal).
                    ConductorType::Invalid | ConductorType::Bare => {}
                }
                let z = lfactor * (1.0 / dij).ln() + self.get_ze(i, j, earth_model);
                zmat.set(idxi, j, z);
                zmat.set(j, idxi, z);
            }
        }

        // Reduce out the CN/TS (always the last rows/cols).
        while zmat.order() > self.num_conds {
            zmat = zmat.kron(zmat.order() - 1).expect("kron order > 1");
        }
        self.fz_matrix.copy_from(&zmat);

        // For shielded cables, build the capacitance matrix directly; assumes
        // the insulation may lie between semicon layers. Pascal `Denom` is a
        // function-level local: an INVALID/Bare phase reuses the previous value.
        let mut denom: f64 = 0.0;
        for i in 0..self.nphases {
            let yfactor = TWOPI * E0 * self.feps_r[i] * self.fw; // includes f so C ⇒ Y
            let rad_out = 0.5 * self.fdia_ins[i];
            let rad_in = rad_out - self.fins_layer[i];
            match self.fcond_type[i] {
                ConductorType::Cn => {
                    if self.fsemicon_layer[i] {
                        // semicon layer (default)
                        denom = (rad_out / rad_in).ln();
                    } else {
                        // No semicon layer (Synergi and Kersting/Kerestes' book)
                        let rad_cn = 0.5 * (self.fdia_cable[i] - self.fdia_strand[i]);
                        let rad_strand = 0.5 * self.fdia_strand[i];
                        let k = self.fk_strand[i] as f64;
                        denom = (rad_cn / rad_in).ln() - (1.0 / k) * (k * rad_strand / rad_cn).ln();
                    }
                }
                ConductorType::Ts => {
                    denom = (rad_out / rad_in).ln();
                }
                ConductorType::Invalid | ConductorType::Bare => {}
            }
            self.fyc_matrix.set(i, i, cmplx(0.0, yfactor / denom));
        }

        if reduced_size > 0 {
            self.kron(reduced_size); // was reduced, so reduce again to same size
        }

        self.frho_changed = false;
    }

    /// `TCableConstants.ConductorsInSameSpace`: no height check (cable depth is
    /// irrelevant); phase conductors use their core `radius`, neutral
    /// conductors use `0.5 * DiaCable`. dss_capi 0.15.x adds an
    /// equivalent-spacing branch (distances from the spacing rather than
    /// coordinates).
    pub(super) fn cisp_cable(&self) -> Option<String> {
        if self.equivalent_spacing {
            for i in 0..self.num_conds {
                let ri = if i < self.nphases {
                    self.fradius[i]
                } else {
                    0.5 * self.fdia_cable[i]
                };
                for j in (i + 1)..self.num_conds {
                    let rj = if j < self.nphases {
                        self.fradius[j]
                    } else {
                        0.5 * self.fdia_cable[j]
                    };
                    let dij = if i < self.nphases && j >= self.nphases {
                        self.eq_dist_ph_n
                    } else {
                        self.eq_dist_ph_ph
                    };
                    if dij < (ri + rj) {
                        return Some(format!(
                            "Cable conductors {} and {} occupy the same space.",
                            i + 1,
                            j + 1
                        ));
                    }
                }
            }
            return None;
        }

        for i in 0..self.num_conds {
            let ri = if i < self.nphases {
                self.fradius[i]
            } else {
                0.5 * self.fdia_cable[i]
            };
            for j in (i + 1)..self.num_conds {
                let rj = if j < self.nphases {
                    self.fradius[j]
                } else {
                    0.5 * self.fdia_cable[j]
                };
                let dij =
                    ((self.fx[i] - self.fx[j]).powi(2) + (self.fy[i] - self.fy[j]).powi(2)).sqrt();
                if dij < (ri + rj) {
                    return Some(format!(
                        "Cable conductors {} and {} occupy the same space.",
                        i + 1,
                        j + 1
                    ));
                }
            }
        }
        None
    }
}
