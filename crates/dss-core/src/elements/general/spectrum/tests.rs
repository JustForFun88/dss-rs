use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use dss_parser::{Parser, ParserVars};

fn edit_and_dump(edits: &[(&str, &str)]) -> Vec<(String, String)> {
    let cls = class_props(&EnumRegistry::new());
    let mut obj = SpectrumObj::new("s");
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
    // Oracle: NumHarm='0', Harmonic='', %Mag='', Angle='', CSVFile='', Like=''
    let dump = edit_and_dump(&[]);
    let expected = [
        ("NumHarm", "0"),
        ("Harmonic", ""),
        ("%Mag", ""),
        ("Angle", ""),
        ("CSVFile", ""),
        ("Like", ""),
    ];
    for (got, (name, val)) in dump.iter().zip(expected) {
        assert_eq!(got.0, name);
        assert_eq!(got.1, val);
    }
}

/// Edit a spectrum through the property machinery, then run `EndEdit` (which
/// builds `MultArray`), returning the object.
fn build(edits: &[(&str, &str)]) -> SpectrumObj {
    let cls = class_props(&EnumRegistry::new());
    let mut obj = SpectrumObj::new("s");
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
    obj.end_edit();
    obj
}

#[test]
fn get_mult_rotates_fundamental_to_zero() {
    // FundAngle is the fundamental's angle (30 deg); every harmonic phasor is
    // shifted by `angle - harmonic*FundAngle`. Chosen so all phasors land on the
    // real axis exactly (no truncated-pi rounding): h1: 30-1*30=0, h3: 90-3*30=0.
    let s = build(&[
        ("NumHarm", "2"),
        ("harmonic", "1 3"),
        ("%mag", "100 50"),
        ("angle", "30 90"),
    ]);
    let m1 = s.get_mult(1.0);
    let m3 = s.get_mult(3.0);
    assert!((m1.re - 1.0).abs() < 1e-12 && m1.im.abs() < 1e-12, "{m1:?}");
    assert!((m3.re - 0.5).abs() < 1e-12 && m3.im.abs() < 1e-12, "{m3:?}");
    // Harmonics not present (or off the 0.01 window) return zero.
    assert_eq!(s.get_mult(5.0), Complex64::ZERO);
    assert_eq!(s.get_mult(2.0), Complex64::ZERO);
}

#[test]
fn get_mult_matches_within_nearest_hundredth() {
    let s = build(&[
        ("NumHarm", "2"),
        ("harmonic", "1 5"),
        ("%mag", "100 20"),
        ("angle", "0 0"),
    ]);
    // |h - HarmArray| < 0.01 matches; 5.009 hits harmonic 5, 5.02 does not.
    assert!((s.get_mult(5.009).re - 0.20).abs() < 1e-12);
    assert_eq!(s.get_mult(5.02), Complex64::ZERO);
}

#[test]
fn get_mult_is_zero_before_end_edit() {
    // Without EndEdit, MultArray is unbuilt and every lookup is zero.
    let cls = class_props(&EnumRegistry::new());
    let mut obj = SpectrumObj::new("s");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let enums = EnumRegistry::new();
    let mut errors = Vec::new();
    for (name, value) in [
        ("NumHarm", "1"),
        ("harmonic", "1"),
        ("%mag", "100"),
        ("angle", "0"),
    ] {
        let idx = cls.property_index(name).unwrap();
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    assert_eq!(obj.get_mult(1.0), Complex64::ZERO);
}

#[test]
fn pct_mag_scale_round_trips() {
    // %Mag stored per-unit (×0.01), displayed as percent (÷0.01).
    let dump = edit_and_dump(&[
        ("NumHarm", "3"),
        ("harmonic", "1 5 7"),
        ("%mag", "100 20 50"),
        ("angle", "0 0 0"),
    ]);
    assert_eq!(dump[2], ("%Mag".into(), "[ 100 20 50]".into()));
    assert_eq!(dump[1], ("Harmonic".into(), "[ 1 5 7]".into()));
}
