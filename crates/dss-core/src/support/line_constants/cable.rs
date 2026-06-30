//! Cable-data state shared by the concentric-neutral and tape-shield
//! specializations, port of `General/CableConstants.pas` (`TCableConstants` —
//! the abstract base of `TCNLineConstants`/`TTSLineConstants`, never
//! instantiated on its own). It adds the insulation/cable geometry arrays and
//! overrides `ConductorsInSameSpace` (cable height is irrelevant — the height
//! check is dropped; a neutral conductor's radius is `0.5 * DiaCable`).
//!
//! `TCableConstants.Kron` is byte-for-byte identical to the base `Kron`, so it
//! is not re-implemented — [`LineConstants::kron`] serves both.

use super::LineConstants;
use crate::support::line_units::LineUnits;

impl LineConstants {
    /// `Set_EpsR` — relative permittivity of the insulation (dimensionless).
    pub fn set_eps_r(&mut self, i: usize, value: f64) {
        if i < self.num_conds {
            self.feps_r[i] = value;
        }
    }

    /// `Set_InsLayer` — thickness of the insulation layer.
    pub fn set_ins_layer(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fins_layer[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `Set_DiaIns` — diameter over the insulation.
    pub fn set_dia_ins(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fdia_ins[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `Set_DiaCable` — diameter over the cable.
    pub fn set_dia_cable(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fdia_cable[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `TCableConstants.ConductorsInSameSpace`: no height check (cable depth is
    /// irrelevant); phase conductors use their core `radius`, neutral
    /// conductors use `0.5 * DiaCable`.
    pub(super) fn cisp_cable(&self) -> Option<String> {
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
