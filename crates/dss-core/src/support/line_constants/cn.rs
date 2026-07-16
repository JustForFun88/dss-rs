//! Concentric-neutral cable setters, port of the `TCNLineConstants`-era
//! stranded-neutral properties, now folded into the merged `TCableConstants`
//! (CableConstants.pas). The impedance/capacitance computation lives in the
//! per-conductor merged [`super::cable`] `calc_cable`; this module holds only
//! the CN strand setters. A conductor uses these when its `FCondType` is `CN`.

use super::LineConstants;
use crate::support::line_units::LineUnits;

/// `TCNLineConstants`: retained alias for a merged cable [`LineConstants`].
pub type CnLineConstants = LineConstants;

impl LineConstants {
    /// `SetkStrand` — number of concentric-neutral strands.
    pub fn set_k_strand(&mut self, i: usize, value: i32) {
        if i < self.num_conds {
            self.fk_strand[i] = value;
        }
    }

    /// `SetDiaStrand` — diameter of a neutral strand.
    pub fn set_dia_strand(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fdia_strand[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `SetGmrStrand` — GMR of a neutral strand.
    pub fn set_gmr_strand(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.fgmr_strand[i] = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `SetRStrand` — AC resistance of a neutral strand.
    pub fn set_r_strand(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.frstrand[i] = value * LineUnits::from_code(units).to_per_meter();
        }
    }
}
