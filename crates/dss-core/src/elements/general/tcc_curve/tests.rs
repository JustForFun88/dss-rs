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

/// Build a `TccCurveObj` from explicit npts/c/t edits (for the trip-time tests).
fn build_curve(npts: &str, c: &str, t: &str) -> TccCurveObj {
    let cls = class_props(&EnumRegistry::new());
    let mut obj = TccCurveObj::new("c");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let enums = EnumRegistry::new();
    let mut errors = Vec::new();
    for (n, v) in [("npts", npts), ("C_array", c), ("T_array", t)] {
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
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    obj
}

#[test]
fn get_tcc_time_log_log_interpolates_tlink() {
    // The built-in `tlink` curve (Executive.pas CreateDefaultDSSItems).
    let mut tlink = build_curve("7", "2 2.1 3 4 6 22 50", "300 100 10.1 4 1.4 0.1 0.02");
    // Below the first point: no operation.
    assert_eq!(tlink.get_tcc_time(1.5), -1.0);
    // First point (exact, via the interpolation branch with zero numerator).
    assert!((tlink.get_tcc_time(2.0) - 300.0).abs() < 1e-9);
    // Direct hits.
    assert!((tlink.get_tcc_time(3.0) - 10.1).abs() < 1e-12);
    assert!((tlink.get_tcc_time(22.0) - 0.1).abs() < 1e-12);
    // Log-log interpolation between bracketing points (Python reference).
    assert!((tlink.get_tcc_time(5.0) - 2.2446184565202234).abs() < 1e-12);
    assert!((tlink.get_tcc_time(8.0) - 0.780471273839859).abs() < 1e-12);
    // At/above the last point: the last value.
    assert!((tlink.get_tcc_time(100.0) - 0.02).abs() < 1e-12);
}

#[test]
fn get_tcc_time_single_point_and_empty() {
    // Npts=1: always the single time (when at/above the point).
    let mut one = build_curve("1", "2", "5");
    assert_eq!(one.get_tcc_time(1.0), -1.0); // below the point
    assert!((one.get_tcc_time(2.0) - 5.0).abs() < 1e-12);
    assert!((one.get_tcc_time(50.0) - 5.0).abs() < 1e-12);
    // Npts=0: never operates.
    let mut empty = TccCurveObj::new("e");
    assert_eq!(empty.get_tcc_time(99.0), -1.0);
}

#[test]
fn get_ov_time_definite_time_scan() {
    // Over-voltage definite-time curve (pickup ascending in per-unit).
    let ov = build_curve("3", "1.0 1.1 1.2", "10 5 1");
    assert_eq!(ov.get_ov_time(1.0), -1.0); // at/below first point: no op
    assert_eq!(ov.get_ov_time(0.5), -1.0);
    assert!((ov.get_ov_time(1.05) - 10.0).abs() < 1e-12); // first bin
    assert!((ov.get_ov_time(1.15) - 5.0).abs() < 1e-12); // second bin
    assert!((ov.get_ov_time(1.25) - 1.0).abs() < 1e-12); // above last → last time
    // Single point: any voltage above it returns the single time.
    let one = build_curve("1", "1.0", "7");
    assert_eq!(one.get_ov_time(0.5), -1.0);
    assert!((one.get_ov_time(1.5) - 7.0).abs() < 1e-12);
}

#[test]
fn get_uv_time_definite_time_scan() {
    // Under-voltage definite-time curve (pickup ascending; scanned backward).
    let uv = build_curve("3", "0.8 0.9 1.0", "10 5 1");
    assert_eq!(uv.get_uv_time(1.0), -1.0); // at/above last point: no op
    assert_eq!(uv.get_uv_time(1.5), -1.0);
    assert!((uv.get_uv_time(0.95) - 1.0).abs() < 1e-12); // Pascal T_Values[i+1]
    assert!((uv.get_uv_time(0.85) - 5.0).abs() < 1e-12);
    assert!((uv.get_uv_time(0.75) - 10.0).abs() < 1e-12); // below first → first time
    // Single point: any voltage below it returns the single time.
    let one = build_curve("1", "1.0", "7");
    assert_eq!(one.get_uv_time(1.5), -1.0);
    assert!((one.get_uv_time(0.5) - 7.0).abs() < 1e-12);
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
