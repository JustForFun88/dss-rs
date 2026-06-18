use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use dss_parser::{Parser, ParserVars};

fn edited(edits: &[(&str, &str)]) -> (ClassProps, XyCurveObj, Vec<String>) {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = XyCurveObj::new("c1");
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

fn get(cls: &ClassProps, obj: &XyCurveObj, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

#[test]
fn arrays_and_points_round_trip() {
    // Oracle (dss-python 0.15.7): points reads back interleaved.
    let (cls, obj, errs) = edited(&[
        ("npts", "4"),
        ("yarray", "10 20 30 40"),
        ("xarray", "1 2 3 4"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "npts"), "4");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 1 2 3 4]");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 10 20 30 40]");
    assert_eq!(get(&cls, &obj, "points"), "[ 1 10 2 20 3 30 4 40]");
    // First point feeds the X/Y accessors.
    assert_eq!(get(&cls, &obj, "x"), "1");
    assert_eq!(get(&cls, &obj, "y"), "10");
}

#[test]
fn get_y_value_interpolates_and_extrapolates() {
    // Values transcribed from the oracle (probe_xy3.py).
    let (_cls, mut obj, errs) = edited(&[
        ("npts", "4"),
        ("xarray", "1 2 3 4"),
        ("yarray", "10 20 30 40"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    for (x, y) in [
        (0.5, 5.0),
        (1.0, 10.0),
        (1.5, 15.0),
        (2.5, 25.0),
        (4.0, 40.0),
        (5.0, 50.0),
    ] {
        assert!((obj.get_y_value(x) - y).abs() < 1e-9, "GetY({x})");
    }
}

#[test]
fn x_setter_syncs_y_with_shift_and_scale() {
    // Oracle (probe_xy2.py): scales/shifts applied, then `x=2` → y=20.
    let (cls, mut obj, errs) = edited(&[
        ("npts", "3"),
        ("xarray", "0 1 2"),
        ("yarray", "0 10 20"),
        ("xscale", "2"),
        ("yscale", "3"),
        ("xshift", "1"),
        ("yshift", "5"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "x"), "1");
    assert_eq!(get(&cls, &obj, "y"), "5");
    // edit x=2
    let enums = EnumRegistry::new();
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = Vec::new();
    let idx = cls.property_index("x").unwrap();
    let mut eng = PropEngine {
        parser: &mut parser,
        vars: &vars,
        enums: &enums,
        errors: &mut errors,
        foreign: None,
    };
    cls.edit_property(&mut obj, idx, "2", &mut eng).unwrap();
    assert_eq!(get(&cls, &obj, "x"), "2");
    assert_eq!(get(&cls, &obj, "y"), "20");
}

#[test]
fn set_points_splits_pairs() {
    // The oracle crashes on `points=`, so this path is Rust-only.
    let (cls, obj, errs) = edited(&[("points", "1 10 2 20 3 30")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "npts"), "3");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 1 2 3]");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 10 20 30]");
    assert_eq!(get(&cls, &obj, "points"), "[ 1 10 2 20 3 30]");
}

#[test]
fn file_props_are_not_ported() {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = XyCurveObj::new("c1");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = Vec::new();
    let idx = cls.property_index("csvfile").unwrap();
    let mut eng = PropEngine {
        parser: &mut parser,
        vars: &vars,
        enums: &enums,
        errors: &mut errors,
        foreign: None,
    };
    let err = cls
        .edit_property(&mut obj, idx, "curve.csv", &mut eng)
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("not ported"),
        "{err}"
    );
}

#[test]
fn make_like_copies_curve() {
    let (cls, base, _) = edited(&[("npts", "2"), ("xarray", "0 1"), ("yarray", "5 7")]);
    let mut obj = XyCurveObj::new("derived");
    obj.make_like(&base);
    assert_eq!(get(&cls, &obj, "npts"), "2");
    assert_eq!(get(&cls, &obj, "xarray"), "[ 0 1]");
    assert_eq!(get(&cls, &obj, "yarray"), "[ 5 7]");
}
