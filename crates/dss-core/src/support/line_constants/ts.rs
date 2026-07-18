//! Tape-shield cable setters, port of the `TTSLineConstants`-era shield/tape
//! properties, now folded into the merged `TCableConstants`
//! (CableConstants.pas). The impedance/capacitance computation lives in the
//! per-conductor merged [`super::cable`] `calc_cable`; this module holds only
//! the TS shield/tape setters. A conductor uses these when its `FCondType` is
//! `TS`.

use super::LineConstants;
use crate::support::line_units::LineUnits;

/// `TTSLineConstants`: retained alias for a merged cable [`LineConstants`].
pub type TsLineConstants = LineConstants;

impl LineConstants {
    /// `SetDiaShield` — diameter over the tape shield.
    pub fn set_dia_shield(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.cable_mut(i).dia_shield = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `SetTapeLayer` — thickness of the tape-shield layer.
    pub fn set_tape_layer(&mut self, i: usize, units: i32, value: f64) {
        if i < self.num_conds {
            self.cable_mut(i).tape_layer = value * LineUnits::from_code(units).to_meters();
        }
    }

    /// `SetTapeLap` — tape-shield overlap, in percent (dimensionless).
    pub fn set_tape_lap(&mut self, i: usize, value: f64) {
        if i < self.num_conds {
            self.cable_mut(i).tape_lap = value;
        }
    }
}
