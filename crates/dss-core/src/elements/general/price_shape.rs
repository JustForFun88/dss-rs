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
//! `CSVFile` is read via the deferred [`FileLoad`] path; `SngFile`/`DblFile`
//! (binary input) and `Action=DblSave/SngSave` (binary output) stay
//! `NOT_PORTED`.

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
}

impl DssObject for PriceShapeObj {
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

    /// Pascal `StringEnumActionProperty` for `Action` (DblSave/SngSave only).
    fn do_action(&mut self, _ordinal: i32, errors: &mut Vec<String>) {
        errors.push(format!(
            "PriceShape.{}: Action=DblSave/SngSave (binary file output) is not ported.",
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

    /// Pascal `TPriceShapeObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            // Pascal `3, 7, 8, 9`: invalidate the cached statistics.
            PRICE => self.core.std_dev_calculated = false,
            CSVFILE => {
                self.core.std_dev_calculated = false;
                self.core.pending_file_loads.push(FileLoad {
                    prop: CSVFILE,
                    filename: self.core.csvfile.clone(),
                });
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

    /// Pascal `TPriceShapeObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        if let Some(o) = other.as_any().downcast_ref::<PriceShapeObj>() {
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

    fn edited(edits: &[(&str, &str)]) -> (ClassProps, PriceShapeObj, Vec<String>) {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        let mut obj = PriceShapeObj::new("d");
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

    fn get(cls: &ClassProps, obj: &PriceShapeObj, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    #[test]
    fn defaults_match_oracle() {
        let (cls, obj, errs) = edited(&[]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "NPts"), "0");
        assert_eq!(get(&cls, &obj, "Interval"), "1");
        assert_eq!(get(&cls, &obj, "Price"), "");
        assert_eq!(get(&cls, &obj, "Mean"), "0");
        assert_eq!(get(&cls, &obj, "StdDev"), "0");
        assert_eq!(get(&cls, &obj, "SInterval"), "3600");
        assert_eq!(get(&cls, &obj, "Action"), "");
    }

    #[test]
    fn fixed_interval_prices_and_stats() {
        let (cls, obj, errs) = edited(&[("npts", "4"), ("interval", "1"), ("price", "2 4 6 8")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Price"), "[ 2 4 6 8]");
        assert!((get(&cls, &obj, "Mean").parse::<f64>().unwrap() - 5.0).abs() < 1e-12);
        // Sample std-dev (n-1): variance of [2,4,6,8] (mean 5) = (9+1+1+9)/3 =
        // 20/3, so std-dev = sqrt(20/3) (oracle prints 2.58198889747161).
        let want_std = (20.0_f64 / 3.0).sqrt();
        assert!((get(&cls, &obj, "StdDev").parse::<f64>().unwrap() - want_std).abs() < 1e-9);
    }

    #[test]
    fn hour_auto_sets_variable_interval() {
        // Unlike TempShape, setting Hour clears Interval to 0 (no explicit
        // interval=0 needed). Trapezoid mean over the curve.
        let (cls, obj, errs) = edited(&[("npts", "3"), ("hour", "1 2 4"), ("price", "1 2 4")]);
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "Interval"), "0");
        assert_eq!(get(&cls, &obj, "Hour"), "[ 1 2 4]");
        assert!((get(&cls, &obj, "Mean").parse::<f64>().unwrap() - 2.5).abs() < 1e-12);
    }

    #[test]
    fn get_price_variable_interval_interpolates() {
        let (_cls, mut obj, errs) =
            edited(&[("npts", "3"), ("hour", "1 2 4"), ("price", "10 20 40")]);
        assert!(errs.is_empty(), "{errs:?}");
        for (hr, want) in [
            (1.0, 10.0),
            (1.5, 15.0),
            (2.0, 20.0),
            (3.0, 30.0),
            (4.0, 40.0),
        ] {
            assert!(
                (obj.get_price(hr) - want).abs() < 1e-12,
                "hr {hr}: {} != {want}",
                obj.get_price(hr)
            );
        }
        assert!((obj.get_price(5.0) - 10.0).abs() < 1e-12); // wraps
    }

    #[test]
    fn make_like_copies() {
        let (cls, base, errs) = edited(&[("npts", "3"), ("interval", "2"), ("price", "11 22 33")]);
        assert!(errs.is_empty(), "{errs:?}");
        let mut obj = PriceShapeObj::new("d");
        obj.make_like(&base);
        obj.end_edit();
        assert_eq!(get(&cls, &obj, "NPts"), "3");
        assert_eq!(get(&cls, &obj, "Interval"), "2");
        assert_eq!(get(&cls, &obj, "Price"), "[ 11 22 33]");
    }

    #[test]
    fn binary_file_props_are_not_ported() {
        let enums = EnumRegistry::new();
        let cls = class_props(&enums);
        for name in ["sngfile", "dblfile"] {
            let mut obj = PriceShapeObj::new("d");
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
            edited(&[("npts", "6"), ("interval", "1"), ("csvfile", "price.csv")]);
        let loads = obj.take_file_loads();
        assert_eq!(loads.len(), 1);
        assert_eq!(loads[0].prop, CSVFILE);
        assert_eq!(loads[0].filename, "price.csv");
        assert!(errs.is_empty(), "{errs:?}");
    }
}
