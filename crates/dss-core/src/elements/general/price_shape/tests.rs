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
    let (_cls, mut obj, errs) = edited(&[("npts", "3"), ("hour", "1 2 4"), ("price", "10 20 40")]);
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
fn binary_file_props_queue_a_binary_file_load() {
    // WPG.1: SngFile/DblFile are ported (deferred FileLoad, like CSVFile).
    for name in ["sngfile", "dblfile"] {
        let (_cls, mut obj, errs) = edited(&[("npts", "4"), ("interval", "1"), (name, "p.bin")]);
        assert!(errs.is_empty(), "{name}: {errs:?}");
        let loads = obj.take_file_loads();
        assert_eq!(loads.len(), 1, "{name}");
        assert_eq!(loads[0].filename, "p.bin", "{name}");
        assert!(loads[0].binary, "{name}");
    }
}

#[test]
fn read_sng_file_fixed_interval() {
    let (cls, mut obj, _) = edited(&[("npts", "4"), ("interval", "1")]);
    let mut bytes = Vec::new();
    for v in [32.0f32, 30.5, 41.0, 55.5] {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    obj.core.read_sng_file(&bytes);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "NPts"), "4");
    assert_eq!(get(&cls, &obj, "Price"), "[ 32 30.5 41 55.5]");
}

#[test]
fn read_dbl_file_variable_interval() {
    let (cls, mut obj, _) = edited(&[("npts", "2"), ("interval", "0")]);
    let mut bytes = Vec::new();
    for (h, v) in [(0.0f64, 32.0f64), (3.0, 55.5)] {
        bytes.extend_from_slice(&h.to_le_bytes());
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    obj.core.read_dbl_file(&bytes);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "Hour"), "[ 0 3]");
    assert_eq!(get(&cls, &obj, "Price"), "[ 32 55.5]");
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
