use super::*;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropEngine};
use dss_parser::{Parser, ParserVars};

fn apply(
    cls: &ClassProps,
    obj: &mut dyn DssObject,
    edits: &[(&str, &str)],
) -> crate::diag::ErrorLog {
    let enums = EnumRegistry::new();
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    errors.extend(obj.data_mut().take_errors());
    errors
}

fn get(cls: &ClassProps, obj: &dyn DssObject, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

#[test]
fn defaults() {
    // Three conductors and three phases in feet, no position given.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let obj = LineSpacingObj::new("ls");
    assert_eq!(get(&cls, &obj, "nconds"), "3");
    assert_eq!(get(&cls, &obj, "nphases"), "3");
    assert_eq!(get(&cls, &obj, "units"), "ft");
    assert_eq!(get(&cls, &obj, "x"), "");
    assert_eq!(get(&cls, &obj, "h"), "");
}

#[test]
fn nconds_resizes_arrays_and_resets_units() {
    // The data after `nconds=` is applied, and a second `nconds=` empties the
    // lists and resets Units to ft.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LineSpacingObj::new("ls");
    let errs = apply(
        &cls,
        &mut obj,
        &[("nconds", "3"), ("units", "m"), ("x", "1 2 3")],
    );
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "units"), "m");
    assert_eq!(get(&cls, &obj, "x"), "[ 1 2 3]");
    let errs = apply(&cls, &mut obj, &[("nconds", "2")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "nconds"), "2");
    assert_eq!(get(&cls, &obj, "units"), "ft");
    assert_eq!(get(&cls, &obj, "x"), "");
}

#[test]
fn nconds_keeps_no_position_and_returns_to_feet() {
    // Each `nconds=` keeps no position, whatever the count: X and H read
    // empty and Units ft. A count of 2000000000 is stored with no buffer, and
    // a list given after it keeps the values written.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LineSpacingObj::new("ls");
    let errs = apply(
        &cls,
        &mut obj,
        &[
            ("nconds", "3"),
            ("x", "1 2 3"),
            ("h", "10 11 12"),
            ("units", "m"),
        ],
    );
    assert!(errs.is_empty(), "{errs:?}");
    let errs = apply(&cls, &mut obj, &[("nconds", "5")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "nconds"), "5");
    assert_eq!(get(&cls, &obj, "x"), "");
    assert_eq!(get(&cls, &obj, "h"), "");
    assert_eq!(get(&cls, &obj, "units"), "ft");

    let errs = apply(&cls, &mut obj, &[("nconds", "2000000000")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "nconds"), "2000000000");
    assert_eq!(get(&cls, &obj, "x"), "");
    let errs = apply(&cls, &mut obj, &[("x", "1 2")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "x"), "[ 1 2]");
    assert_eq!(obj.xcoord(), [1.0, 2.0]);
}

#[test]
fn nconds_zero_reads_empty_string() {
    // `nconds=0` keeps no coordinate, so X/H read as the empty string `''`,
    // not `'[]'`, as the `linespacing_zero_nconds` golden reads.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LineSpacingObj::new("ls");
    let errs = apply(&cls, &mut obj, &[("nconds", "0")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "nconds"), "0");
    assert_eq!(get(&cls, &obj, "x"), "");
    assert_eq!(get(&cls, &obj, "h"), "");
}

#[test]
fn nconds_negative_clamps_to_empty() {
    // A negative `nconds` (dss_capi raises 303 on it) keeps no
    // coordinate: NConds reads back the raw negative value and X/H read `''`.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LineSpacingObj::new("ls");
    let errs = apply(&cls, &mut obj, &[("nconds", "-1")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "nconds"), "-1");
    assert_eq!(get(&cls, &obj, "x"), "");
    assert_eq!(get(&cls, &obj, "h"), "");
}

#[test]
fn nconds_past_the_integer_range_is_stored_as_converted() {
    // A count past the integer range is stored with no message and keeps no
    // coordinate: the coordinates and the unit given before it are dropped.
    // The default lane's integer conversion saturates at 2147483647, the
    // parity lane's wraps the count modulo 2^32 to 1215752191, the value the
    // pinned dss-python stores (measured 2026-10-23).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LineSpacingObj::new("ls");
    let errs = apply(
        &cls,
        &mut obj,
        &[
            ("nconds", "3"),
            ("x", "1 2 3"),
            ("h", "4 5 6"),
            ("units", "m"),
            ("nconds", "99999999999"),
        ],
    );
    assert!(errs.is_empty(), "{errs:?}");
    let stored = if crate::compat::ORACLE_PARITY {
        "1215752191"
    } else {
        "2147483647"
    };
    assert_eq!(get(&cls, &obj, "nconds"), stored);
    assert_eq!(get(&cls, &obj, "x"), "");
    assert_eq!(get(&cls, &obj, "h"), "");
    assert_eq!(get(&cls, &obj, "units"), "ft");
}

#[test]
fn x_h_arrays_sized_by_nconds() {
    // X/H take at most nconds values: extra tokens are dropped, and a short
    // list keeps the values given, nothing padded.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LineSpacingObj::new("ls");
    let errs = apply(
        &cls,
        &mut obj,
        &[
            ("nconds", "3"),
            ("nphases", "3"),
            ("x", "-1.2 0 1.2 9.9"), // 4 tokens, only 3 kept
            ("h", "28"),             // 1 token, conductors 2 and 3 have none
            ("units", "ft"),
        ],
    );
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "x"), "[ -1.2 0 1.2]");
    assert_eq!(obj.xcoord(), [-1.2, 0.0, 1.2]);
    assert_eq!(get(&cls, &obj, "h"), "[ 28]");
    assert_eq!(obj.ycoord(), [28.0]);
}

#[test]
fn make_like_copies_geometry() {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut src = LineSpacingObj::new("s1");
    apply(
        &cls,
        &mut src,
        &[
            ("nconds", "4"),
            ("nphases", "3"),
            ("x", "-1.2 0 1.2 0"),
            ("h", "28 28 28 24"),
            ("units", "m"),
        ],
    );
    let mut dst = LineSpacingObj::new("s2");
    assert!(dst.refuse_property(prop::X, "X").is_some());
    dst.make_like(&src);
    // The copy holds the source's given `nconds=`, so it takes data at once.
    assert_eq!(dst.refuse_property(prop::X, "X"), None);
    assert_eq!(get(&cls, &dst, "nconds"), "4");
    assert_eq!(get(&cls, &dst, "nphases"), "3");
    assert_eq!(get(&cls, &dst, "units"), "m"); // Other.Units overrides the ft reset
    assert_eq!(get(&cls, &dst, "x"), "[ -1.2 0 1.2 0]");
    assert_eq!(get(&cls, &dst, "h"), "[ 28 28 28 24]");

    // A short list is copied as given, and a list never given stays empty.
    let mut short = LineSpacingObj::new("s3");
    apply(
        &cls,
        &mut short,
        &[("nconds", "4"), ("x", "-1.2 0"), ("units", "m")],
    );
    let mut dst = LineSpacingObj::new("s4");
    dst.make_like(&short);
    assert_eq!(get(&cls, &dst, "x"), "[ -1.2 0]");
    assert_eq!(get(&cls, &dst, "h"), "");
    assert_eq!(dst.xcoord(), [-1.2, 0.0]);
    assert!(dst.ycoord().is_empty());
}

/// `Like=` copies the equivalent-spacing block, pinned by expected value in
/// both lanes.
///
/// The authority copies all five fields explicitly: r4133
/// `TLineSpacing.MakeLike` assigns `FEquivalentSpacing`, `FEqDistPhPh`,
/// `FEqDistPhN`, `FAvgHeightPh` and `FAvgHeightN` right after the X/Y arrays
/// (`Version8/Source/General/LineSpacing.pas:251-255`). Only dss_capi 0.15.x
/// stops after `Units`, leaving the clone at its `Create` defaults while the
/// base class has already copied the `PrpSequence` marking them set — a
/// regression this port does not reproduce in either lane.
#[test]
fn make_like_copies_the_equivalent_spacing_block() {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut src = LineSpacingObj::new("s1");
    apply(
        &cls,
        &mut src,
        &[
            ("nconds", "4"),
            ("nphases", "3"),
            ("x", "-1.2 0 1.2 0"),
            ("h", "28 28 28 24"),
            ("units", "m"),
        ],
    );
    // Put the equivalent-spacing block off its `Create` defaults on the source.
    src.detailed = false;
    src.eq_dist_ph_ph = 4.5;
    src.eq_dist_ph_n = 3.25;
    src.avg_phase_height = 28.0;
    src.avg_neutral_height = 24.0;

    let mut dst = LineSpacingObj::new("s2");
    dst.make_like(&src);

    // The geometry proper is copied in both lanes (unchanged contract).
    assert_eq!(get(&cls, &dst, "nconds"), "4");
    assert_eq!(get(&cls, &dst, "x"), "[ -1.2 0 1.2 0]");

    // The source's values, in both lanes (r4133 `LineSpacing.pas:251-255`).
    assert!(
        !dst.detailed,
        "`MakeLike` copies `FEquivalentSpacing`, so the clone leaves \
         `Create`'s detailed = true behind"
    );
    assert_eq!(dst.eq_dist_ph_ph, 4.5);
    assert_eq!(dst.eq_dist_ph_n, 3.25);
    assert_eq!(dst.avg_phase_height, 28.0);
    assert_eq!(dst.avg_neutral_height, 24.0);
}
