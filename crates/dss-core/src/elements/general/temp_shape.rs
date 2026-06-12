//! `TShape` (TempShape) — per-hour temperature curve.
//! Port of Pascal `General/TempShape.pas` (`TTShapeObj`; registered class name
//! `TShape`). A `DSS_OBJECT` class consumed by temperature-dependent loads
//! (Phase 7). The data storage and the hour lookup / mean-std-dev / CSV reader
//! live on the shared [`ScalarShapeCore`]; this file is the property table and
//! the class-specific `PropertySideEffects` (TempShape has no `hour→interval`
//! coupling — variable interval requires an explicit `interval=0`).
//!
//! `CSVFile` is read via the deferred [`FileLoad`] path (like LoadShape);
//! `SngFile`/`DblFile` (binary input) and `Action=DblSave/SngSave` (binary
//! output) stay `NOT_PORTED`.

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
    5  MEAN      => PropDef::double("Mean");
    6  STDDEV    => PropDef::double("StdDev");
    7  CSVFILE   => PropDef::string("CSVFile").flags(
        PropFlags::IS_FILENAME | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    8  SNGFILE   => PropDef::string("SngFile").flags(
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME
            | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
    );
    9  DBLFILE   => PropDef::string("DblFile").flags(
        PropFlags::NOT_PORTED | PropFlags::IS_FILENAME
            | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::GLOBAL_COUNT,
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

    /// Pascal `StringEnumActionProperty` for `Action` (DblSave/SngSave only).
    fn do_action(&mut self, _ordinal: i32, errors: &mut Vec<String>) {
        errors.push(format!(
            "TShape.{}: Action=DblSave/SngSave (binary file output) is not ported.",
            self.core.data.name()
        ));
    }

    fn take_file_loads(&mut self) -> Vec<FileLoad> {
        std::mem::take(&mut self.core.pending_file_loads)
    }

    /// Apply a resolved `CSVFile` (Pascal `DoCSVFile`).
    fn apply_file_load(&mut self, load: &FileLoad, content: &str, _errors: &mut Vec<String>) {
        if load.prop == CSVFILE {
            self.core.read_csv_file(content);
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
                self.core.pending_file_loads.push(FileLoad {
                    prop: CSVFILE,
                    filename: self.core.csvfile.clone(),
                });
            }
            _ => {}
        }
    }

    /// Pascal `TTShapeObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        if let Some(o) = other.as_any().downcast_ref::<TShapeObj>() {
            self.core.make_like_from(&o.core);
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::{ClassProps, PropEngine};
    use dss_parser::{Parser, ParserVars};

    fn edited(edits: &[(&str, &str)]) -> (ClassProps, TShapeObj, Vec<String>) {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = TShapeObj::new("t");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        for (name, value) in edits {
            let idx = cls.property_index(name).expect("known property");
            let mut eng = PropEngine {
                parser: &mut parser,
                vars: &vars,
                enums: &enums,
                errors: &mut errors,
                foreign: None,
            };
            cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
        }
        obj.end_edit();
        errors.extend(obj.data_mut().take_errors());
        (cls, obj, errors)
    }

    fn get(cls: &ClassProps, obj: &TShapeObj, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    #[test]
    fn defaults_match_oracle() {
        // Oracle (dss-python 0.15.7) `new tshape.t`: empty data, Mean/StdDev 0
        // (no error, unlike LoadShape), Interval 1, SInterval 3600.
        let (cls, obj, errs) = edited(&[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "NPts"), "0");
        assert_eq!(get(&cls, &obj, "Interval"), "1");
        assert_eq!(get(&cls, &obj, "Temp"), "");
        assert_eq!(get(&cls, &obj, "Hour"), "");
        assert_eq!(get(&cls, &obj, "Mean"), "0");
        assert_eq!(get(&cls, &obj, "StdDev"), "0");
        assert_eq!(get(&cls, &obj, "SInterval"), "3600");
        assert_eq!(get(&cls, &obj, "MInterval"), "60");
        assert_eq!(get(&cls, &obj, "Action"), "");
    }

    #[test]
    fn fixed_interval_temps_and_stats() {
        // mult=(20 30 50 80) interval=1 → Mean 45, StdDev computed.
        let (cls, obj, errs) = edited(&[("npts", "4"), ("interval", "1"), ("temp", "20 30 50 80")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Temp"), "[ 20 30 50 80]");
        assert!((get(&cls, &obj, "Mean").parse::<f64>().unwrap() - 45.0).abs() < 1e-12);
        // RCDMeanAndStdDev is the sample std-dev (n-1 denominator): the variance
        // of [20,30,50,80] (mean 45) is (25²+15²+5²+35²)/3 = 2100/3 = 700, so the
        // std-dev is sqrt(700) (oracle prints 26.4575131106459).
        let want_std = 700.0_f64.sqrt();
        assert!((get(&cls, &obj, "StdDev").parse::<f64>().unwrap() - want_std).abs() < 1e-9);
    }

    #[test]
    fn second_and_minute_interval_aliases() {
        let (cls, obj, errs) = edited(&[("npts", "4"), ("sinterval", "900"), ("temp", "1 2 4 8")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Interval"), "0.25");
        assert_eq!(get(&cls, &obj, "SInterval"), "900");
        assert_eq!(get(&cls, &obj, "MInterval"), "15");
    }

    #[test]
    fn explicit_mean_stddev_override_computation() {
        let (cls, obj, errs) = edited(&[
            ("npts", "4"),
            ("interval", "1"),
            ("temp", "20 30 50 80"),
            ("mean", "5"),
            ("stddev", "2"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Mean"), "5");
        assert_eq!(get(&cls, &obj, "StdDev"), "2");
    }

    #[test]
    fn get_temperature_fixed_interval_wraps() {
        // Hand-traced from the legacy even-interval branch. round(hr) wraps.
        let (_cls, mut obj, errs) =
            edited(&[("npts", "4"), ("interval", "1"), ("temp", "10 20 30 40")]);
        assert!(errs.is_empty(), "{errs:?}");
        for (hr, want) in [
            (0.0, 40.0), // round(0)=0 → wraps to NumPoints → TValues[4]
            (1.0, 10.0),
            (2.0, 20.0),
            (4.0, 40.0),
            (4.25, 40.0),
            (5.0, 10.0), // 5 mod 4 = 1 → TValues[1]
        ] {
            assert!(
                (obj.get_temperature(hr) - want).abs() < 1e-12,
                "hr {hr}: {} != {want}",
                obj.get_temperature(hr)
            );
        }
    }

    #[test]
    fn get_temperature_variable_interval_interpolates() {
        // hour=[1,2,4], temp=[10,20,40]; interval=0. Legacy 1-based lookup.
        let (_cls, mut obj, errs) = edited(&[
            ("npts", "3"),
            ("interval", "0"),
            ("hour", "1 2 4"),
            ("temp", "10 20 40"),
        ]);
        assert!(errs.is_empty(), "{errs:?}");
        for (hr, want) in [
            (1.0, 10.0),
            (1.5, 15.0),
            (2.0, 20.0),
            (3.0, 30.0),
            (4.0, 40.0),
        ] {
            assert!(
                (obj.get_temperature(hr) - want).abs() < 1e-12,
                "hr {hr}: {} != {want}",
                obj.get_temperature(hr)
            );
        }
        // hr=5 wraps: 5 - trunc(5/4)*4 = 1 → exact point TValues[1]=10.
        assert!((obj.get_temperature(5.0) - 10.0).abs() < 1e-12);
    }

    #[test]
    fn make_like_copies() {
        let (cls, base, errs) = edited(&[("npts", "3"), ("interval", "2"), ("temp", "11 22 33")]);
        assert!(errs.is_empty(), "{errs:?}");
        let mut obj = TShapeObj::new("t");
        obj.make_like(&base);
        obj.end_edit();
        assert_eq!(get(&cls, &obj, "NPts"), "3");
        assert_eq!(get(&cls, &obj, "Interval"), "2");
        assert_eq!(get(&cls, &obj, "Temp"), "[ 11 22 33]");
    }

    #[test]
    fn binary_file_props_are_not_ported() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        for name in ["sngfile", "dblfile"] {
            let mut obj = TShapeObj::new("t");
            let mut parser = Parser::new();
            let vars = ParserVars::new();
            let mut errors = Vec::new();
            let idx = cls.property_index(name).unwrap();
            let mut eng = PropEngine {
                parser: &mut parser,
                vars: &vars,
                enums: &enums,
                errors: &mut errors,
                foreign: None,
            };
            let err = cls
                .edit_property(&mut obj, idx, "shape.bin", &mut eng)
                .unwrap_err();
            assert!(
                err.to_string().to_lowercase().contains("not ported"),
                "{name}: {err}"
            );
        }
    }

    #[test]
    fn csvfile_queues_a_file_load() {
        let (_cls, mut obj, errs) =
            edited(&[("npts", "6"), ("interval", "1"), ("csvfile", "temp.csv")]);
        let loads = obj.take_file_loads();
        assert_eq!(loads.len(), 1);
        assert_eq!(loads[0].prop, CSVFILE);
        assert_eq!(loads[0].filename, "temp.csv");
        assert!(errs.is_empty(), "{errs:?}");
    }

    #[test]
    fn action_dblsave_is_not_ported() {
        let (_cls, _obj, errs) = edited(&[
            ("npts", "4"),
            ("interval", "1"),
            ("temp", "20 30 50 80"),
            ("action", "dblsave"),
        ]);
        assert!(
            errs.iter().any(|e| e.to_lowercase().contains("not ported")),
            "{errs:?}"
        );
    }
}
