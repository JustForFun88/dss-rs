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
    // Pascal Create: nconds=3, nphases=3, units=ft, x/h all zero.
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let obj = LineSpacingObj::new("ls");
    assert_eq!(get(&cls, &obj, "nconds"), "3");
    assert_eq!(get(&cls, &obj, "nphases"), "3");
    assert_eq!(get(&cls, &obj, "units"), "ft");
    assert_eq!(get(&cls, &obj, "x"), "[ 0 0 0]");
    assert_eq!(get(&cls, &obj, "h"), "[ 0 0 0]");
}

#[test]
fn nconds_resizes_arrays_and_resets_units() {
    // Setting nconds reallocates X/H to the new length and resets Units to
    // ft; X/H read with the new count (zero-filled here).
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = LineSpacingObj::new("ls");
    let errs = apply(&cls, &mut obj, &[("units", "m"), ("nconds", "2")]);
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "nconds"), "2");
    assert_eq!(get(&cls, &obj, "units"), "ft"); // reset by the nconds side effect
    assert_eq!(get(&cls, &obj, "x"), "[ 0 0]");
}

#[test]
fn nconds_grow_preserves_leading_and_zero_fills_tail() {
    // Growing `nconds` re-runs the realloc side effect: the leading entries
    // are preserved and the grown tail reads as zero. Pascal's `ReAllocmem`
    // leaves that tail uninitialized (nondeterministic heap), so this is a
    // Rust-only invariant — not oracle-pinnable — locking the zero-fill
    // choice documented on `realloc_conductors`. The grow also resets units.
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
    assert_eq!(get(&cls, &obj, "x"), "[ 1 2 3 0 0]"); // leading kept, tail zeroed
    assert_eq!(get(&cls, &obj, "h"), "[ 10 11 12 0 0]");
    assert_eq!(get(&cls, &obj, "units"), "ft"); // realloc side effect resets units
}

#[test]
fn nconds_zero_reads_empty_string() {
    // `nconds=0` frees the coordinate buffers (Pascal `ReAllocmem(FX, 0)`
    // nils the pointer), so X/H read as the empty string `''`, not `'[]'`.
    // Oracle-confirmed and pinned by the `linespacing_zero_nconds` golden;
    // kept here as a unit-level guard on the empty-array → nil mapping.
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
    // Pascal's `ReAllocmem(FX, FNConds)` *raises an exception* for a negative
    // `nconds` (the oracle reports DSSException 303), so the degenerate path
    // cannot be oracle-pinned. We clamp the allocation size to zero rather
    // than panic: NConds reads back the raw negative value, the coordinate
    // buffers are empty, and X/H therefore read `''`. Rust-only invariant
    // locking that graceful clamp.
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
fn x_h_arrays_sized_by_nconds() {
    // X/H are DoubleVArray sized by FNConds: extra tokens are dropped and
    // missing tokens zero-fill (Pascal `InterpretDblArray`).
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
            ("h", "28"),             // 1 token, rest zero-filled
            ("units", "ft"),
        ],
    );
    assert!(errs.is_empty(), "{errs:?}");
    assert_eq!(get(&cls, &obj, "x"), "[ -1.2 0 1.2]");
    assert_eq!(get(&cls, &obj, "h"), "[ 28 0 0]");
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
    dst.make_like(&src);
    assert_eq!(get(&cls, &dst, "nconds"), "4");
    assert_eq!(get(&cls, &dst, "nphases"), "3");
    assert_eq!(get(&cls, &dst, "units"), "m"); // Other.Units overrides the ft reset
    assert_eq!(get(&cls, &dst, "x"), "[ -1.2 0 1.2 0]");
    assert_eq!(get(&cls, &dst, "h"), "[ 28 28 28 24]");
}

/// The Stage F [`LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING`] row, pinned by
/// expected value in both lanes.
///
/// `TLineSpacingObj.MakeLike` copies `NConds`/`NPhases`/`FX`/`FY`/`Units` and
/// stops, so the five equivalent-spacing fields stay at their `Create` defaults
/// (`detailed = true`, the rest `0.0`) even though the base class has already
/// copied the `PrpSequence` that marks them set. **Parity lane**: the defaults,
/// what both gating oracles report. **Default lane**: the source's values.
///
/// [`LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING`]: crate::compat::LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING
#[test]
fn make_like_equivalent_spacing_is_the_lane_kernel() {
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

    // Derived from the *lane*, never from the row's own alias — see
    // `isource::tests` for why (F-settle W4).
    let parity = crate::compat::ORACLE_PARITY;
    assert_eq!(
        dst.detailed, parity,
        "parity keeps `Create`'s detailed = true (MakeLike never copies it); \
         the default lane copies the source's false"
    );
    let expect = |source: f64| if parity { 0.0 } else { source };
    assert_eq!(dst.eq_dist_ph_ph, expect(4.5));
    assert_eq!(dst.eq_dist_ph_n, expect(3.25));
    assert_eq!(dst.avg_phase_height, expect(28.0));
    assert_eq!(dst.avg_neutral_height, expect(24.0));
}
