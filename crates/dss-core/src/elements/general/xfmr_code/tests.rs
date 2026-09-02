use super::*;
use crate::obj::base::DssObject;
use crate::obj::props::PropEngine;
use dss_parser::{Parser, ParserVars};

/// Drive a sequence of `(prop, value)` edits through the property engine,
/// then run `end_edit`. Mirrors the executive's edit loop (sans foreign
/// class resolution, which XfmrCode does not use).
fn edited(edits: &[(&str, &str)]) -> (ClassProps, XfmrCodeObj) {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = XfmrCodeObj::new("xc");
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
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert!(errors.is_empty(), "{errors:?}");
    (cls, obj)
}

fn get(cls: &ClassProps, obj: &XfmrCodeObj, name: &str) -> String {
    let enums = EnumRegistry::new();
    let idx = cls.property_index(name).unwrap();
    cls.get_value(obj, idx, &enums)
}

/// Assert a property's dump matches `expected` the way the golden harness
/// does: the non-numeric skeleton exactly, the numbers within tolerance
/// (FPC `FloatToStr` ≠ Rust `{}`, so raw strings must not be diffed — see
/// `util::float_to_str`).
fn check(cls: &ClassProps, obj: &XfmrCodeObj, name: &str, expected: &str) {
    let actual = get(cls, obj, name);
    let split = |s: &str| -> (String, Vec<f64>) {
        let mut skel = String::new();
        let mut nums = Vec::new();
        let mut tok = String::new();
        let flush = |tok: &mut String, skel: &mut String, nums: &mut Vec<f64>| {
            if let Ok(v) = tok.parse::<f64>() {
                nums.push(v);
                skel.push('#');
            } else {
                skel.push_str(tok);
            }
            tok.clear();
        };
        for c in s.chars() {
            if c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e' | 'E') {
                tok.push(c);
            } else {
                flush(&mut tok, &mut skel, &mut nums);
                skel.push(c);
            }
        }
        flush(&mut tok, &mut skel, &mut nums);
        (skel, nums)
    };
    let (askel, anums) = split(&actual);
    let (eskel, enums) = split(expected);
    assert_eq!(askel, eskel, "{name}: {actual:?} vs {expected:?}");
    assert_eq!(
        anums.len(),
        enums.len(),
        "{name}: {actual:?} vs {expected:?}"
    );
    for (a, e) in anums.iter().zip(&enums) {
        assert!(
            (a - e).abs() <= 1e-9 + 1e-9 * e.abs(),
            "{name}: {a} vs {e} ({actual:?} vs {expected:?})"
        );
    }
}

#[test]
fn defaults_match_oracle() {
    let (cls, obj) = edited(&[]);
    // Oracle (dss-python 0.15.7) `new xfmrcode.x`.
    check(&cls, &obj, "Windings", "2");
    check(&cls, &obj, "kV", "12.47");
    check(&cls, &obj, "%R", "0.2");
    check(&cls, &obj, "%LoadLoss", "0.4");
    check(&cls, &obj, "XHL", "7");
    check(&cls, &obj, "XSCArray", "[ 0]");
    check(&cls, &obj, "Conns", "[wye, wye, ]");
    check(&cls, &obj, "RDCOhms", "0.26435153");
}

#[test]
fn wdg_sequencing_writes_active_winding() {
    // `wdg=2` selects the active winding; scalars then write through to it.
    let (cls, obj) = edited(&[
        ("windings", "2"),
        ("wdg", "1"),
        ("kv", "7.2"),
        ("wdg", "2"),
        ("kv", "0.24"),
    ]);
    check(&cls, &obj, "kVs", "[7.2, 0.24, ]");
    // The active winding is left at 2 (the last `wdg=`).
    check(&cls, &obj, "Wdg", "2");
    check(&cls, &obj, "kV", "0.24");
}

#[test]
fn plural_arrays_leave_active_at_last_winding() {
    let (cls, obj) = edited(&[
        ("windings", "3"),
        ("conns", "delta, wye, wye"),
        ("kvs", "115, 12.47, 4.16"),
        ("kvas", "5000, 5000, 5000"),
        ("xhl", "8"),
        ("xht", "10"),
        ("xlt", "9"),
    ]);
    check(&cls, &obj, "Conns", "[delta, wye, wye, ]");
    check(&cls, &obj, "Wdg", "3");
    // XHL/XHT/XLT flow into the XSC slots in EndEdit (≤ 3 windings).
    check(&cls, &obj, "XSCArray", "[ 8 10 9]");
    // kVAs default the norm/emerg ratings to 1.1×/1.5× winding-1 kVA.
    check(&cls, &obj, "NormHkVA", "5500");
    check(&cls, &obj, "EmergHkVA", "7500");
}

#[test]
fn pct_loadloss_splits_across_windings_one_and_two() {
    // %LoadLoss is split evenly between windings 1 and 2 (→ %R = 1.2 each).
    let (cls, obj) = edited(&[("windings", "2"), ("%loadloss", "2.4")]);
    check(&cls, &obj, "%Rs", "[1.2, 1.2, ]");
}

#[test]
fn xscarray_explicit_overrides_default() {
    let (cls, obj) = edited(&[("windings", "3"), ("xscarray", "8 10 9")]);
    check(&cls, &obj, "XSCArray", "[ 8 10 9]");
    // No XHL/XHT/XLT edit → no EndEdit overwrite; XHL stays at default.
    check(&cls, &obj, "XHL", "7");
}

#[test]
fn seasons_then_ratings_round_trip() {
    let (cls, obj) = edited(&[("seasons", "3"), ("ratings", "600, 700, 800")]);
    check(&cls, &obj, "Seasons", "3");
    check(&cls, &obj, "Ratings", "[ 600 700 800]");
}

#[test]
fn make_like_copies_winding_web() {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let (_, base) = edited(&[
        ("windings", "2"),
        ("kvs", "115, 4.16"),
        ("kvas", "3000, 3000"),
        ("xhl", "7"),
    ]);
    let mut obj = XfmrCodeObj::new("derived");
    obj.make_like(&base);
    check(&cls, &obj, "kVs", "[115, 4.16, ]");
    check(&cls, &obj, "NormHkVA", "3300");
    check(&cls, &obj, "XSCArray", "[ 7]");
}
