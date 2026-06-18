use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use dss_parser::{Parser, ParserVars};

/// Drive a sequence of `name=value` edits through the engine the way the
/// executive's `Edit` loop will, then return the all-properties dump.
fn edit_and_dump(edits: &[(&str, &str)]) -> Vec<(String, String)> {
    let cls = class_props(&EnumRegistry::new());
    let mut obj = TccCurveObj::new("test");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let enums = EnumRegistry::new();
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
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");

    (1..=cls.num_properties())
        .map(|i| {
            (
                cls.property_name(i).to_string(),
                cls.get_value(&obj, i, &enums),
            )
        })
        .collect()
}

#[test]
fn defaults_match_oracle() {
    // Oracle: NPts='0', C_Array='', T_Array='', Like=''
    let dump = edit_and_dump(&[]);
    assert_eq!(
        dump,
        vec![
            ("NPts".into(), "0".into()),
            ("C_Array".into(), "".into()),
            ("T_Array".into(), "".into()),
            ("Like".into(), "".into()),
        ]
    );
}

#[test]
fn full_spec_matches_oracle() {
    // Oracle: npts=3 c_array=(1 2 3) t_array=(0.1 0.2 0.3)
    //   => NPts='3', C_Array='[ 1 2 3]', T_Array='[ 0.1 0.2 0.3]'
    let dump = edit_and_dump(&[
        ("npts", "3"),
        ("C_array", "1 2 3"),
        ("T_array", "0.1 0.2 0.3"),
    ]);
    assert_eq!(dump[0], ("NPts".into(), "3".into()));
    assert_eq!(dump[1], ("C_Array".into(), "[ 1 2 3]".into()));
    assert_eq!(dump[2], ("T_Array".into(), "[ 0.1 0.2 0.3]".into()));
}

#[test]
fn short_array_zero_fills() {
    // Oracle "npts then part c": npts=3, C_array=(5 6) => '[ 5 6 0]'
    let dump = edit_and_dump(&[("npts", "3"), ("C_array", "5 6")]);
    assert_eq!(dump[1], ("C_Array".into(), "[ 5 6 0]".into()));
}

#[test]
fn shrinking_npts_truncates() {
    // Oracle "npts=2 after 3": keeps the first two points.
    let dump = edit_and_dump(&[
        ("npts", "3"),
        ("C_array", "1 2 3"),
        ("T_array", "7 8 9"),
        ("npts", "2"),
    ]);
    assert_eq!(dump[0], ("NPts".into(), "2".into()));
    assert_eq!(dump[1], ("C_Array".into(), "[ 1 2]".into()));
    assert_eq!(dump[2], ("T_Array".into(), "[ 7 8]".into()));
}

#[test]
fn abbreviations_and_order_independence() {
    // Set arrays before npts, using abbreviations; the final dump must be
    // identical to the canonical order (the engine resizes on npts and the
    // arrays re-read against the count).
    let canonical = edit_and_dump(&[("npts", "2"), ("C_array", "10 20"), ("T_array", "1 2")]);
    let odd = edit_and_dump(&[("np", "2"), ("c", "10 20"), ("t", "1 2")]);
    assert_eq!(canonical, odd);
}

#[test]
fn log_points_track_c_array() {
    // CalcLogPoints side effect: log_c[i] = ln(c[i]).
    edit_and_dump(&[("npts", "2"), ("C_array", "1 100")]);
    let cls = class_props(&EnumRegistry::new());
    let mut obj = TccCurveObj::new("t");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let enums = EnumRegistry::new();
    let mut errors = Vec::new();
    for (n, v) in [("npts", "2"), ("C_array", "1 100")] {
        let idx = cls.property_index(n).unwrap();
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut obj, idx, v, &mut eng).unwrap();
    }
    let logs = obj.log_c.as_ref().unwrap();
    assert!((logs[0] - 0.0).abs() < 1e-12); // ln(1) = 0
    assert!((logs[1] - 100.0_f64.ln()).abs() < 1e-12);
}
