use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use dss_parser::{Parser, ParserVars};

fn edited(edits: &[(&str, &str)]) -> (ClassProps, GrowthShapeObj, crate::diag::ErrorLog) {
    let cls = class_props();
    let mut obj = GrowthShapeObj::new("gs");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let enums = EnumRegistry::new();
    let mut errors = crate::diag::ErrorLog::new();
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
fn file_props_queue_a_deferred_load() {
    // WPG.1: CSVFile/SngFile/DblFile are ported (deferred FileLoad).
    for (name, prop, binary) in [
        ("csvfile", prop::CSVFILE, false),
        ("sngfile", prop::SNGFILE, true),
        ("dblfile", prop::DBLFILE, true),
    ] {
        let (_cls, mut obj, errs) = edited(&[("npts", "2"), (name, "growth.bin")]);
        assert!(errs.is_empty(), "{name}: {errs:?}");
        let loads = obj.take_file_loads();
        assert_eq!(loads.len(), 1, "{name}");
        assert_eq!(loads[0].prop, prop, "{name}");
        assert_eq!(loads[0].filename, "growth.bin", "{name}");
        assert_eq!(loads[0].binary, binary, "{name}");
    }
}

#[test]
fn read_csv_file_keeps_fractional_years_and_shrinks_npts() {
    // Pascal `DoCSVFile` ignores its `RoundA` argument (the rounding loop
    // exists only in `DoSngFile`/`DoDblFile`) — oracle-proven 2026-07-07:
    // a CSV of `2000.6, 2005.4, 2010.7` reports `year = [2000.6 2005.4
    // 2010.7]` while the same values via SngFile round to `[2001 2005 2011]`.
    let (cls, mut obj, _) = edited(&[("npts", "5")]);
    obj.read_csv_file("1999.4, 1.10\n2000.6, 1.07\n2001, 1.05\n");
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "NPts"), "3");
    assert_eq!(get(&cls, &obj, "Year"), "[ 1999.4 2000.6 2001]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 1.1 1.07 1.05]");
}

#[test]
fn read_sng_and_dbl_file_round_trip() {
    // Mult values are exactly representable in f32 (0.25 steps) so the
    // f32->f64 widen round-trips exactly.
    let (cls, mut obj, _) = edited(&[("npts", "3")]);
    let mut bytes = Vec::new();
    for (y, m) in [(2000.0f32, 1.25f32), (2005.0, 1.5), (2010.0, 1.0)] {
        bytes.extend_from_slice(&y.to_le_bytes());
        bytes.extend_from_slice(&m.to_le_bytes());
    }
    obj.read_sng_file(&bytes);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "Year"), "[ 2000 2005 2010]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 1.25 1.5 1]");

    let (cls, mut obj, _) = edited(&[("npts", "2")]);
    let mut bytes = Vec::new();
    for (y, m) in [(2000.0f64, 1.05f64), (2010.0, 1.0)] {
        bytes.extend_from_slice(&y.to_le_bytes());
        bytes.extend_from_slice(&m.to_le_bytes());
    }
    obj.read_dbl_file(&bytes);
    obj.end_edit();
    assert_eq!(get(&cls, &obj, "Year"), "[ 2000 2010]");
    assert_eq!(get(&cls, &obj, "Mult"), "[ 1.05 1]");
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
fn get_year_and_mult_idx_are_one_based() {
    // B3-r3723: `Load.GrowthFactor` Year=0 path reads `GetYear(1)` (the raw
    // first year, its `firstY` probe) and `GetMultIdx(1)` (the first cumulative
    // `YearMult`, not the raw multiplier).
    let (_cls, obj, errs) = edited(&[("npts", "3"), ("year", "0 1 2"), ("mult", "1.2 1.5 2.0")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(obj.get_year(1), 0.0);
    assert_eq!(obj.get_year(2), 1.0);
    // GetMultIdx(1) = YearMult[0] = first cumulative = first raw mult (1.2).
    assert!((obj.get_mult_idx(1) - 1.2).abs() < 1e-12);
    // GetMultIdx(2) compounds the second rate: 1.2 * 1.5 = 1.8.
    assert!((obj.get_mult_idx(2) - 1.8).abs() < 1e-12);
    // Empty / out-of-range guards return 0.0.
    assert_eq!(obj.get_year(0), 0.0);
    assert_eq!(obj.get_mult_idx(0), 0.0);
    let (_c, empty, _e) = edited(&[]);
    assert_eq!(empty.get_year(1), 0.0);
    assert_eq!(empty.get_mult_idx(1), 0.0);
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
