use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use dss_parser::{Parser, ParserVars};

fn edited(edits: &[(&str, &str)]) -> (ClassProps, GrowthShapeObj, Vec<String>) {
    let cls = class_props();
    let mut obj = GrowthShapeObj::new("gs");
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
    obj.end_edit();
    errors.extend(obj.data_mut().take_errors());
    (cls, obj, errors)
}

fn get(cls: &ClassProps, obj: &GrowthShapeObj, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

#[test]
fn defaults_match_oracle() {
    // Oracle (dss-python 0.15.7) `new growthshape.x`: NPts=0, empty arrays.
    let (cls, obj, errs) = edited(&[]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "NPts"), "0");
    assert_eq!(get(&cls, &obj, "Year"), "");
    assert_eq!(get(&cls, &obj, "Mult"), "");
}

#[test]
fn year_is_rounded_and_arrays_round_trip() {
    let (cls, obj, errs) = edited(&[
        ("npts", "5"),
        ("year", "1999 2000 2001 2005 2010"),
        ("mult", "1.10 1.07 1.05 1.025 1.01"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "NPts"), "5");
    assert_eq!(get(&cls, &obj, "Year"), "[ 1999 2000 2001 2005 2010]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 1.1 1.07 1.05 1.025 1.01]");
}

#[test]
fn file_props_are_not_ported() {
    // NOT_PORTED returns a hard parse error (not a deferred-error push).
    let cls = class_props();
    let mut obj = GrowthShapeObj::new("gs");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let enums = EnumRegistry::new();
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
        .edit_property(&mut obj, idx, "growth.csv", &mut eng)
        .unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("not ported"),
        "{err}"
    );
}

#[test]
fn get_mult_compounds_from_base_year() {
    // The example from the Pascal header: fast start tapering to 1%.
    let (_cls, mut obj, errs) = edited(&[
        ("npts", "5"),
        ("year", "1999 2000 2001 2005 2010"),
        ("mult", "1.10 1.07 1.05 1.025 1.01"),
    ]);
    assert!(errs.is_empty(), "{errs:?}");
    // The base year (and any earlier year) returns 1.0 — the multipliers
    // apply to the *following* years (Pascal `if Index > 0`).
    assert_eq!(obj.get_mult(1998), 1.0);
    assert_eq!(obj.get_mult(1999), 1.0);
    // 2000 = first multiplier; 2001 compounds the second; 2002 the third.
    assert!((obj.get_mult(2000) - 1.10).abs() < 1e-12);
    assert!((obj.get_mult(2001) - 1.10 * 1.07).abs() < 1e-12);
    assert!((obj.get_mult(2002) - 1.10 * 1.07 * 1.05).abs() < 1e-12);
    // Between listed years the last rate persists: 2003 keeps ×1.05.
    let m2002 = obj.get_mult(2002);
    assert!((obj.get_mult(2003) - m2002 * 1.05).abs() < 1e-12);
}

#[test]
fn get_mult_grows_table_past_initial_nyears() {
    // Default NYears=30; query well beyond it to exercise the regrow path.
    let (_cls, mut obj, errs) =
        edited(&[("npts", "2"), ("year", "2000 2001"), ("mult", "1.0 1.02")]);
    assert!(errs.is_empty(), "{errs:?}");
    // get_mult(2000+n) == YearMult[n-1]; YearMult[0]=1.0 (base mult) and
    // each later year compounds ×1.02, so get_mult(2050) == 1.02^49.
    let expected = 1.02_f64.powi(49);
    assert!((obj.get_mult(2050) - expected).abs() < 1e-9);
}

#[test]
fn make_like_copies_curve() {
    let (cls, base, _) = edited(&[("npts", "2"), ("year", "2000 2010"), ("mult", "1.05 1.02")]);
    let mut obj = GrowthShapeObj::new("derived");
    obj.make_like(&base);
    assert_eq!(get(&cls, &obj, "NPts"), "2");
    assert_eq!(get(&cls, &obj, "Year"), "[ 2000 2010]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 1.05 1.02]");
}
