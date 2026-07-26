//! `PriceShape` — per-hour price curve.
//! Port of Pascal `General/PriceShape.pas` (`TPriceShapeObj`). A `DSS_OBJECT`
//! class whose curve feeds `ckt.PriceSignal` in `SolveDaily` (wired in WP5.7).
//! The data storage and the hour lookup / mean-std-dev / CSV reader live on the
//! shared [`ScalarShapeCore`]; this file is the property table and the
//! class-specific `PropertySideEffects`.
//!
//! Differs from its sibling [`super::temp_shape`] in two ways that the Pascal
//! `DefineProperties`/`PropertySideEffects` spell out: PriceShape's `Interval`/
//! `SInterval`/`MInterval` are **not** flagged `NonNegative`, and setting `Hour`
//! auto-clears `Interval` to 0 (a variable-interval curve) while setting a
//! positive `Interval` drops the hour array.
//!
//! `CSVFile`/`SngFile`/`DblFile` are all read via the deferred [`FileLoad`]
//! path (WPG.1 for the binary pair); `Action=DblSave/SngSave` (binary output)
//! stays `NOT_PORTED`.

#[cfg(test)]
mod tests;

use super::scalar_shape::{ScalarShapeCore, store_array};
use crate::obj::base::{DssObject, FileLoad};
use crate::obj::props::{PropDef, PropFlags, define_properties};

// Pascal `TPriceShapeProp` ordinals (1..12) + the property table.
define_properties! {
    class "PriceShape", abbrev true, enums enums;
    1  NPTS      => PropDef::integer("NPts").flags(PropFlags::SUPPRESS_JSON);
    2  INTERVAL  => PropDef::double("Interval").flags(PropFlags::REQUIRED_IN_SPEC_SET);
    3  PRICE     => PropDef::double_array("Price", NPTS).flags(PropFlags::REQUIRED_IN_SPEC_SET);
    4  HOUR      => PropDef::double_array("Hour", NPTS).flags(PropFlags::REQUIRED_IN_SPEC_SET);
    5  MEAN      => PropDef::double("Mean").flags(PropFlags::DYNAMIC_DEFAULT);
    6  STDDEV    => PropDef::double("StdDev").flags(PropFlags::DYNAMIC_DEFAULT);
    7  CSVFILE   => PropDef::string("CSVFile").size_prop(NPTS).flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    8  SNGFILE   => PropDef::string("SngFile").size_prop(NPTS).flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    9  DBLFILE   => PropDef::string("DblFile").size_prop(NPTS).flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    10 SINTERVAL => PropDef::double("SInterval")
        .scale(1.0 / 3600.0)
        .flags(PropFlags::REDUNDANT);
    11 MINTERVAL => PropDef::double("MInterval")
        .scale(1.0 / 60.0)
        .flags(PropFlags::REDUNDANT);
    12 ACTION    => PropDef::action("Action", enums.price_shape_action);
}

use prop::{
    CSVFILE, DBLFILE, HOUR, INTERVAL, MEAN, MINTERVAL, NPTS, PRICE, SINTERVAL, SNGFILE, STDDEV,
};

/// A `PriceShape` instance (`TPriceShapeObj`).
#[derive(Debug, Clone)]
pub struct PriceShapeObj {
    core: ScalarShapeCore,
}

impl PriceShapeObj {
    /// Pascal `TPriceShapeObj.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            core: ScalarShapeCore::new(name, prop::NUM_PROPS),
        }
    }

    /// Pascal `TPriceShapeObj.GetPrice`.
    pub fn get_price(&mut self, hr: f64) -> f64 {
        self.core.get_value_at_hour(hr)
    }

    /// Pascal `TPriceShapeObj.Price(i)`: the price at 1-based curve index `i`
    /// (`SolveLD1`/`SolveLD2`'s `ckt.PriceCurveObj.Price(N)`).
    pub fn price(&mut self, i: i32) -> f64 {
        self.core.value_at(i)
    }
}

impl PriceShapeObj {
    /// Pascal `TPriceShapeObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        let o = other;
        {
            self.core.make_like_from(&o.core);
        }
    }
}

impl DssObject for PriceShapeObj {
    fn data(&self) -> &crate::obj::base::DssObjData {
        &self.core.data
    }
    fn data_mut(&mut self) -> &mut crate::obj::base::DssObjData {
        &mut self.core.data
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            NPTS => self.core.num_points,
            _ => unreachable!("PriceShape has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NPTS => self.core.num_points = value,
            _ => unreachable!("PriceShape has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            INTERVAL | SINTERVAL | MINTERVAL => self.core.interval,
            MEAN => self.core.mean(),
            STDDEV => self.core.std_dev(),
            _ => unreachable!("PriceShape has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            INTERVAL | SINTERVAL | MINTERVAL => self.core.interval = value,
            // Pascal `SetMean`/`SetStdDev`: mark as externally provided.
            MEAN => {
                self.core.f_mean = value;
                self.core.std_dev_calculated = true;
            }
            STDDEV => {
                self.core.f_std_dev = value;
                self.core.std_dev_calculated = true;
            }
            _ => unreachable!("PriceShape has no double property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            CSVFILE => self.core.csvfile.clone(),
            SNGFILE => self.core.sngfile.clone(),
            DBLFILE => self.core.dblfile.clone(),
            _ => unreachable!("PriceShape has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            CSVFILE => self.core.csvfile = value,
            SNGFILE => self.core.sngfile = value,
            DBLFILE => self.core.dblfile = value,
            _ => unreachable!("PriceShape has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            PRICE => self.core.values.as_deref(),
            HOUR => self.core.hours.as_deref(),
            _ => unreachable!("PriceShape has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        let stored = store_array(value);
        match idx {
            PRICE => self.core.values = stored,
            HOUR => self.core.hours = stored,
            _ => unreachable!("PriceShape has no array property {idx}"),
        }
    }

    /// Pascal `StringEnumActionProperty` for `Action` (`TPriceShapeAction`:
    /// DblSave=0, SngSave=1 — `PriceShape.pas:149-150`). Queues the binary write
    /// (Pascal `SaveToDblFile`/`SaveToSngFile`, `PriceShape.pas:547/568`).
    fn do_action(&mut self, ordinal: i32, errors: &mut crate::diag::ErrorLog) {
        let full_name = format!("PriceShape.{}", self.core.data.name());
        self.core
            .queue_shape_save(ordinal == 1, "Price", &full_name, "Prices", errors);
    }

    fn take_file_loads(&mut self) -> Vec<FileLoad> {
        std::mem::take(&mut self.core.pending_file_loads)
    }

    fn take_shape_saves(&mut self) -> Vec<crate::obj::base::ShapeSave> {
        self.core.take_shape_saves()
    }

    /// Apply a resolved `CSVFile` (Pascal `DoCSVFile`).
    fn apply_file_load(
        &mut self,
        load: &FileLoad,
        content: &str,
        _errors: &mut crate::diag::ErrorLog,
    ) {
        if load.prop == CSVFILE {
            self.core.read_csv_file(content);
        }
    }

    /// Apply a resolved `SngFile`/`DblFile` (Pascal `DoSngFile`/`DoDblFile`).
    fn apply_binary_file_load(
        &mut self,
        load: &FileLoad,
        content: &[u8],
        _errors: &mut crate::diag::ErrorLog,
    ) {
        match load.prop {
            SNGFILE => self.core.read_sng_file(content),
            DBLFILE => self.core.read_dbl_file(content),
            _ => {}
        }
    }

    /// Pascal `TPriceShapeObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            // Pascal `3, 7, 8, 9`: invalidate the cached statistics.
            PRICE => self.core.std_dev_calculated = false,
            CSVFILE => {
                self.core.std_dev_calculated = false;
                self.core
                    .pending_file_loads
                    .push(FileLoad::text(CSVFILE, self.core.csvfile.clone()));
            }
            SNGFILE => {
                self.core.std_dev_calculated = false;
                self.core
                    .pending_file_loads
                    .push(FileLoad::binary(SNGFILE, self.core.sngfile.clone()));
            }
            DBLFILE => {
                self.core.std_dev_calculated = false;
                self.core
                    .pending_file_loads
                    .push(FileLoad::binary(DBLFILE, self.core.dblfile.clone()));
            }
            // `ord(interval): if Interval > 0.0 then ReallocMem(Hours, 0)`.
            INTERVAL => {
                if self.core.interval > 0.0 {
                    self.core.hours = None;
                }
            }
            // `ord(hour): Interval := 0` — a variable-interval curve.
            HOUR => self.core.interval = 0.0,
            // `ord(mean), ord(stddev): FStdDevCalculated := TRUE` (already set by
            // the setters; kept for fidelity).
            MEAN | STDDEV => self.core.std_dev_calculated = true,
            _ => {}
        }
    }
}
