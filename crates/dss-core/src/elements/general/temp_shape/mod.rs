//! `TShape` (TempShape) — per-hour temperature curve.
//! Port of Pascal `General/TempShape.pas` (`TTShapeObj`; registered class name
//! `TShape`). A `DSS_OBJECT` class consumed by temperature-dependent loads
//! (Phase 7). The data storage and the hour lookup / mean-std-dev / CSV reader
//! live on the shared [`ScalarShapeCore`]; this file is the property table and
//! the class-specific `PropertySideEffects` (TempShape has no `hour→interval`
//! coupling — variable interval requires an explicit `interval=0`).
//!
//! `CSVFile`/`SngFile`/`DblFile` are all read via the deferred [`FileLoad`]
//! path (like LoadShape; WPG.1 for the binary pair); `Action=DblSave/SngSave`
//! (binary output) stays `NOT_PORTED`.

#[cfg(test)]
mod tests;

use super::scalar_shape::{ScalarShapeCore, store_array};
use crate::obj::base::{DssObject, FileLoad};
use crate::obj::props::{PropDef, PropFlags, define_properties};

// Pascal `TTShapeProp` ordinals (1..12) + the property table.
define_properties! {
    class "TShape", abbrev true, enums enums;
    1  NPTS      => PropDef::integer("NPts").flags(PropFlags::SUPPRESS_JSON);
    2  INTERVAL  => PropDef::double("Interval")
        .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NON_NEGATIVE);
    3  TEMP      => PropDef::double_array("Temp", NPTS).flags(PropFlags::REQUIRED_IN_SPEC_SET);
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
        .flags(PropFlags::REDUNDANT | PropFlags::NON_NEGATIVE);
    11 MINTERVAL => PropDef::double("MInterval")
        .scale(1.0 / 60.0)
        .flags(PropFlags::REDUNDANT | PropFlags::NON_NEGATIVE);
    12 ACTION    => PropDef::action("Action", enums.t_shape_action);
}

use prop::{
    CSVFILE, DBLFILE, HOUR, INTERVAL, MEAN, MINTERVAL, NPTS, SINTERVAL, SNGFILE, STDDEV, TEMP,
};

/// A `TShape` instance (`TTShapeObj`).
#[derive(Debug, Clone)]
pub struct TShapeObj {
    core: ScalarShapeCore,
}

impl TShapeObj {
    /// Pascal `TTShapeObj.Create`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            core: ScalarShapeCore::new(name, prop::NUM_PROPS),
        }
    }

    /// Pascal `TTShapeObj.GetTemperature`.
    pub fn get_temperature(&mut self, hr: f64) -> f64 {
        self.core.get_value_at_hour(hr)
    }
}

impl TShapeObj {
    /// Pascal `TTShapeObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        let o = other;
        {
            self.core.make_like_from(&o.core);
        }
    }
}

impl DssObject for TShapeObj {
    fn data(&self) -> &crate::obj::base::DssObjData {
        &self.core.data
    }
    fn data_mut(&mut self) -> &mut crate::obj::base::DssObjData {
        &mut self.core.data
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            NPTS => self.core.num_points,
            _ => unreachable!("TShape has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NPTS => self.core.num_points = value,
            _ => unreachable!("TShape has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            INTERVAL | SINTERVAL | MINTERVAL => self.core.interval,
            MEAN => self.core.mean(),
            STDDEV => self.core.std_dev(),
            _ => unreachable!("TShape has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            INTERVAL | SINTERVAL | MINTERVAL => self.core.interval = value,
            // Pascal `Set_Mean`/`Set_StdDev`: mark as externally provided.
            MEAN => {
                self.core.f_mean = value;
                self.core.std_dev_calculated = true;
            }
            STDDEV => {
                self.core.f_std_dev = value;
                self.core.std_dev_calculated = true;
            }
            _ => unreachable!("TShape has no double property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            CSVFILE => self.core.csvfile.clone(),
            SNGFILE => self.core.sngfile.clone(),
            DBLFILE => self.core.dblfile.clone(),
            _ => unreachable!("TShape has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            CSVFILE => self.core.csvfile = value,
            SNGFILE => self.core.sngfile = value,
            DBLFILE => self.core.dblfile = value,
            _ => unreachable!("TShape has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            TEMP => self.core.values.as_deref(),
            HOUR => self.core.hours.as_deref(),
            _ => unreachable!("TShape has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        let stored = store_array(value);
        match idx {
            TEMP => self.core.values = stored,
            HOUR => self.core.hours = stored,
            _ => unreachable!("TShape has no array property {idx}"),
        }
    }

    /// Pascal `StringEnumActionProperty` for `Action` (`TTShapeAction`:
    /// DblSave=0, SngSave=1 — `TempShape.pas:152-154`). Queues the binary write
    /// (Pascal `SaveToDblFile`/`SaveToSngFile`, `TempShape.pas:528/548`).
    fn do_action(&mut self, ordinal: i32, errors: &mut crate::diag::ErrorLog) {
        let full_name = format!("TShape.{}", self.core.data.name());
        self.core
            .queue_shape_save(ordinal == 1, "Temp", &full_name, "Temperatures", errors);
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

    /// Pascal `TTShapeObj.PropertySideEffects`. Note: unlike PriceShape there is
    /// no `hour→Interval:=0` coupling — variable interval needs `interval=0`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            // Pascal `3, 7, 8, 9`: setting Temp/CSVFile/SngFile/DblFile
            // invalidates the cached statistics.
            TEMP => self.core.std_dev_calculated = false,
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
            _ => {}
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
