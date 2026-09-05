//! Shared golden-comparison harness for integration tests.
//!
//! Goldens are produced by `tools/golden/generate.py` from dss-python (the
//! same dss_capi engine the Pascal source in `.inputs` builds) and committed
//! under `tests/golden/` at the repository root. See PORTING_PLAN.md §4.
//!
//! The structs mirror the full golden schema; each integration-test binary
//! uses only a subset, so dead-code analysis is suppressed module-wide.
//!
//! **Stage F (`DE_PASCALIZE_PLAN.md` Part IV.2):** the suite runs in two lanes.
//! Everything lane-dependent lives in [`lane`] — the drift-model table turned
//! into code (report goldens, iteration counts) — and every golden driver goes
//! through it instead of reading the feature cfg itself. The comparators in
//! *this* file are lane-independent by design: the continuous floors
//! ([`tol_for`]) and the discrete-state compares are identical in both lanes,
//! which is exactly what the drift model prescribes. The one exception is
//! [`field_eq`]'s `Key=Value` fallback, marked in place.
#![allow(dead_code)]

/// Command-replay scenario gate shared by `golden_line_constants.rs`,
/// `golden_der_controls.rs`, and `golden_harmonics.rs` (the split of the former
/// Phase-7 golden bucket).
pub mod scenario;

/// Stage F two-lane test policy (parity vs default build).
pub mod lane;

/// `R4133_PROPS_PLAN.md` RP2.1/RP2.3: the channel-scoped, value-preserving
/// property normalization table (`PROPS_NORM_R4133`) and the echo-exclusion
/// table (`PROPS_ECHO_R4133`, filled by RP2.3). Consulted only through
/// [`PropsPolicy::normalize`] and [`PropsPolicy::echo_excluded`], and only on
/// [`PropsChannel::R4133`].
pub mod props_norm;

/// Self-golden regeneration rails: the `DSS_UPDATE_GOLDENS` knob plus the
/// anchor and producing-lane write guards of `GOLDEN_REBASE_PLAN.md` §1.2.
/// No driver calls them yet — that is WP-G3.
pub mod regen;

/// The rails' entry points at the spelling the plan (§G0.2), TESTING.md and the
/// WP-G3 call sites use: `harness::regen()` / `harness::snapshot_text()` /
/// `harness::snapshot_bytes()`. The module keeps its own name (type namespace)
/// alongside the function (value namespace).
///
/// `allow(unused_imports)` for the same reason this module allows `dead_code`:
/// it is compiled into every golden test binary and each uses a subset — and
/// until WP-G3 wires the first driver, that subset is empty everywhere.
#[allow(unused_imports)]
pub use regen::{regen, snapshot_bytes, snapshot_text};

/// `GOLDEN_REBASE_PLAN.md` WP-G1 rails (G1.0): the "flag set but the oracle
/// returned nothing" guard every flag-gated corpus comparator calls before it
/// compares, so an absent/empty capture fails the case instead of silently
/// comparing nothing.
pub mod capture_guard;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use dss_core::exec::{Dss, ElementSnapshot};
use num_complex::Complex64;
use serde::Deserialize;

/// Schema of `tests/golden/<case>.json` (schema version 1).
#[derive(Debug, Deserialize)]
pub struct Golden {
    pub schema: u32,
    pub name: String,
    pub master: String,
    pub post: Vec<String>,
    pub solution: GoldenSolution,
    pub circuit: GoldenCircuit,
    /// Node names in Y-matrix order, e.g. `"SOURCEBUS.1"`.
    pub node_order: Vec<String>,
    /// Node voltages in volts, re/im interleaved, same order as `node_order`.
    pub node_voltages: Vec<f64>,
    pub total_power_kw_kvar: Vec<f64>,
    pub losses_w_var: Vec<f64>,
    pub transformers: BTreeMap<String, GoldenTransformer>,
    pub regcontrols: BTreeMap<String, GoldenRegControl>,
    pub capacitors: BTreeMap<String, GoldenCapacitor>,
    pub elements: BTreeMap<String, GoldenElement>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenSolution {
    pub converged: bool,
    pub iterations: u32,
    pub mode: i32,
    pub tolerance: f64,
}

#[derive(Debug, Deserialize)]
pub struct GoldenCircuit {
    pub num_buses: u32,
    pub num_nodes: u32,
    pub num_ckt_elements: u32,
}

#[derive(Debug, Deserialize)]
pub struct GoldenTransformer {
    pub taps: Vec<f64>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenRegControl {
    pub tap_number: i32,
}

#[derive(Debug, Deserialize)]
pub struct GoldenCapacitor {
    pub states: Vec<i32>,
}

#[derive(Debug, Deserialize)]
pub struct GoldenElement {
    pub enabled: bool,
    pub bus_names: Vec<String>,
    /// kW/kvar interleaved per conductor and terminal.
    pub powers: Vec<f64>,
    /// Amps, re/im interleaved per conductor and terminal.
    pub currents: Vec<f64>,
    /// Full property dump (name → display string); present when the case was
    /// generated with `props: true`.
    #[serde(default)]
    pub properties: BTreeMap<String, String>,
}

impl Golden {
    /// Load `tests/golden/<name>.json` from the repository root.
    pub fn load(name: &str) -> Golden {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "tests",
            "golden",
            &format!("{name}.json"),
        ]
        .iter()
        .collect();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read golden {}: {e}", path.display()));
        let golden: Golden = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("cannot parse golden {}: {e}", path.display()));
        assert_eq!(golden.schema, 1, "golden schema version mismatch");
        golden
    }
}

/// Split a value string into its non-numeric "skeleton" (each number replaced
/// by `#`) and the list of numbers it contains — the property-dump comparator
/// (PORTING_PLAN.md §4: numbers with tolerance, structure exactly).
pub fn numeric_skeleton(s: &str) -> (String, Vec<f64>) {
    let (skeleton, nums) = numeric_skeleton_texts(s);
    (skeleton, nums.into_iter().map(|(v, _)| v).collect())
}

/// [`numeric_skeleton`], keeping each number's **source text** next to its
/// value.
///
/// The two are the same scan — this is the primitive and `numeric_skeleton` is
/// its value-only projection, so there is exactly one number scanner in the
/// harness. RP2.4's display floor needs the text because the digits r4133
/// actually printed are what say at which precision it printed
/// (`props_norm::display_is_render`), and a parsed `f64` cannot tell `'19000'`
/// (five printed digits) from `1.9e4`.
pub fn numeric_skeleton_texts(s: &str) -> (String, Vec<(f64, &str)>) {
    // Strip a *leading* UTF-8 BOM only. The official r4133 Oddie DLL prefixes
    // some captured strings with one (spurious export cruft, never real value
    // data), and a 3-byte `﻿` at index 0 would make the `&s[i..]` slices below cut
    // inside the char → panic. This is the exact Oddie failure mode (WP-U2.5). A
    // BOM anywhere ELSE is deliberately NOT stripped: it flows into the skeleton
    // via the char-wise else-branch (which never mid-slices a multibyte char), so
    // a spurious mid-string BOM on one side surfaces as a structure mismatch
    // instead of being silently swallowed — and DSS ASCII output never carries a
    // legitimate interior BOM.
    let s = s.strip_prefix('\u{feff}').unwrap_or(s);
    let mut skeleton = String::new();
    let mut nums = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if let Some((value, len)) = scan_number(&s[i..]) {
            nums.push((value, &s[i..i + len]));
            skeleton.push('#');
            i += len;
        } else {
            // Not a number start: copy one whole char. A bytewise advance would
            // split a multibyte char and panic the next `&s[i..]`; `scan_number`
            // only ever consumes ASCII numeric bytes, so `i` stays on a boundary.
            let ch = s[i..].chars().next().unwrap();
            skeleton.push(ch);
            i += ch.len_utf8();
        }
    }
    (skeleton, nums)
}

/// Longest numeric prefix of `s` that parses as `f64` (and contains a digit).
fn scan_number(s: &str) -> Option<(f64, usize)> {
    let bytes = s.as_bytes();
    let first = bytes[0];
    if !(first.is_ascii_digit() || first == b'.' || first == b'+' || first == b'-') {
        return None;
    }
    let mut end = 0;
    while end < bytes.len() {
        let c = bytes[end];
        if c.is_ascii_digit() || matches!(c, b'.' | b'+' | b'-' | b'e' | b'E') {
            end += 1;
        } else {
            break;
        }
    }
    while end > 0 {
        let cand = &s[..end];
        if cand.bytes().any(|b| b.is_ascii_digit())
            && let Ok(v) = cand.parse::<f64>()
        {
            return Some((v, end));
        }
        end -= 1;
    }
    None
}

/// The verdict of ONE value-string comparison at the plain (un-normalized)
/// `rel`/`abs` floor — the single decision [`assert_value_matches_tol`] panics
/// on and the `DSS_PROPS_CENSUS` walk ([`collect_prop_divergences`]) records.
///
/// Both consumers read the same function so the census can never drift into a
/// second, slightly different comparator (`R4133_PROPS_PLAN.md` RP0.2: the
/// census is the measurement of *this* gate's compare, and RP2.1's disposition
/// mode extends the same seam rather than cloning it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ValueVerdict {
    /// Byte-equal, or every number inside `abs + rel * |expected|`.
    Match,
    /// The non-numeric skeletons differ (a rendering/structure divergence).
    Skeleton,
    /// Skeletons agree but the two sides carry a different number count.
    NumberCount,
    /// At least one number is outside the floor. `index`/`actual`/`expected`
    /// describe the FIRST offender (exactly what the assert reports), while
    /// `max_rel` is the largest relative gap **among the offenders** —
    /// `|a − e| / |e|`, or `|a − e|` when `e == 0`. That is the census's
    /// `max_rel` column.
    ///
    /// **Verification breadth** (re-measured 2026-08-22, RP0.2 audit — the
    /// earlier "94 203 multi-number rows" reading of the census was wrong and is
    /// retracted): the formula reproduces **all 95 317** `value_numeric` rows of
    /// the 2026-08-08 census, each at its own case's tier floor. But that is a
    /// far weaker cross-check than the row count suggests — 95 180 of those rows
    /// carry a SINGLE number, where "max over the offenders" and "max over all
    /// numbers" cannot differ, and only **137** carry more than one (counts
    /// 2:92, 3:26, 4:4, 6:14, 10:1). Of those 137 exactly ONE discriminates the
    /// two readings: `Line.l1` `cmatrix` on
    /// `modes:upgrade/upgrade_spacing_ratings.dss`, recorded as
    /// `5.833551103975013e-8` — the offender-only maximum at that deck's `micro`
    /// floor — and NOT `1.1866505782192496e-7`, the maximum over all six numbers.
    /// So the semantics are *decided* by one cell and merely *consistent with*
    /// every other row; the pin below carries that cell's shape.
    Numeric {
        index: usize,
        actual: f64,
        expected: f64,
        max_rel: f64,
    },
}

/// Classify two value strings at the `rel`/`abs` floor. See [`ValueVerdict`].
pub fn value_verdict(actual: &str, expected: &str, rel: f64, abs: f64) -> ValueVerdict {
    // Byte-identical strings are always a pass — the WHOLE-string guard, so it
    // fires only when the two sides are literally equal. This is the correct
    // home for the machine-generated EPRI bus name `0x008e1248`, whose numeric
    // token the skeleton scanner reads as `8e1248` = inf: identical strings
    // must pass, but a per-number `(inf - inf)` is NaN and fails `<= allowed`.
    // Crucially this does NOT accept two DIFFERENT strings whose tokens both
    // overflow to inf (`…008e1248` vs `…018e1248`) — those fall through to the
    // number compare below and correctly FAIL. Not a loosening: exactly-equal is
    // the tightest possible match.
    if actual == expected {
        return ValueVerdict::Match;
    }
    let (askel, anums) = numeric_skeleton(actual);
    let (eskel, enums) = numeric_skeleton(expected);
    if askel != eskel {
        return ValueVerdict::Skeleton;
    }
    if anums.len() != enums.len() {
        return ValueVerdict::NumberCount;
    }
    let mut first: Option<(usize, f64, f64)> = None;
    let mut max_rel = 0.0f64;
    for (i, (a, e)) in anums.iter().zip(&enums).enumerate() {
        let allowed = abs + rel * e.abs();
        if (a - e).abs() <= allowed {
            continue;
        }
        if first.is_none() {
            first = Some((i, *a, *e));
        }
        let r = if *e == 0.0 {
            (a - e).abs()
        } else {
            (a - e).abs() / e.abs()
        };
        if r > max_rel {
            max_rel = r;
        }
    }
    match first {
        None => ValueVerdict::Match,
        Some((index, a, e)) => ValueVerdict::Numeric {
            index,
            actual: a,
            expected: e,
            max_rel,
        },
    }
}

/// Assert two value strings match: identical skeletons, numbers within
/// `rel`/`abs` tolerance.
///
/// The decision itself lives in [`value_verdict`]; the failure arms re-derive
/// the operands so the panic text stays byte-for-byte what the `assert_eq!` /
/// `assert!` forms have always produced (the extra work happens only on the
/// path that is about to abort the test).
pub fn assert_value_matches_tol(actual: &str, expected: &str, rel: f64, abs: f64, ctx: &str) {
    match value_verdict(actual, expected, rel, abs) {
        ValueVerdict::Match => {}
        ValueVerdict::Skeleton => {
            let (askel, _) = numeric_skeleton(actual);
            let (eskel, _) = numeric_skeleton(expected);
            assert_eq!(
                askel, eskel,
                "{ctx}: structure differs (actual {actual:?} vs expected {expected:?})"
            );
        }
        ValueVerdict::NumberCount => {
            let (_, anums) = numeric_skeleton(actual);
            let (_, enums) = numeric_skeleton(expected);
            assert_eq!(
                anums.len(),
                enums.len(),
                "{ctx}: number count differs (actual {actual:?} vs expected {expected:?})"
            );
        }
        ValueVerdict::Numeric {
            index: i,
            actual: a,
            expected: e,
            ..
        } => {
            // `assert!(cond, "msg")` panics with exactly the formatted message
            // (no `assert_eq!`-style left/right preamble), so a plain `panic!`
            // with the same string is byte-identical to the old failure text.
            panic!(
                "{ctx}: number {i} differs: actual {a} vs expected {e} \
                 (from {actual:?} vs {expected:?})"
            );
        }
    }
}

#[cfg(test)]
mod comparator_tests {
    use super::{assert_value_matches_tol, numeric_skeleton};

    /// A *leading* Oddie BOM is stripped (so `﻿100` compares equal to `100`),
    /// but an *interior* BOM is preserved into the skeleton — so a spurious
    /// mid-string BOM on one side alone is FLAGGED as a structure mismatch, not
    /// silently swallowed (the audit-narrowed strip; WP-U2.5 audit fix).
    #[test]
    fn bom_strip_is_leading_only() {
        // Leading BOM: stripped, so both sides skeletonize identically.
        assert_value_matches_tol("\u{feff}100", "100", 1e-9, 1e-9, "leading-bom");
        // Interior BOM: kept in the skeleton, so it participates in structure.
        let (sk_bom, _) = numeric_skeleton("1\u{feff}00");
        let (sk_plain, _) = numeric_skeleton("100");
        assert_ne!(
            sk_bom, sk_plain,
            "an interior BOM must change the skeleton (be flagged), not vanish"
        );
        // And a one-sided interior BOM fails the value compare (no panic).
        let r = std::panic::catch_unwind(|| {
            assert_value_matches_tol("1\u{feff}00", "100", 1e-9, 1e-9, "interior-bom");
        });
        assert!(r.is_err(), "one-sided interior BOM must fail, not match");
    }

    /// Byte-identical strings pass even when a token overflows to inf; two
    /// DISTINCT strings differing only in such a token FAIL (the per-number
    /// `a == e` shortcut removed in the audit was too loose — it let
    /// `…008e1248` and `…018e1248` compare equal because both parse to inf).
    #[test]
    fn value_match_identity_vs_distinct_inf_tokens() {
        // Identical (incl. an inf-overflowing token) → pass.
        assert_value_matches_tol("a_0x008e1248", "a_0x008e1248", 1e-6, 1e-6, "identity");
        // Distinct strings whose tokens both overflow to inf → must FAIL.
        let r = std::panic::catch_unwind(|| {
            assert_value_matches_tol("a_0x008e1248", "a_0x018e1248", 1e-6, 1e-6, "distinct-inf");
        });
        assert!(
            r.is_err(),
            "distinct inf-token strings must NOT compare equal"
        );
        // Ordinary numeric tolerance still works.
        assert_value_matches_tol("v=1.0000001", "v=1.0", 1e-5, 1e-9, "tol");
        let bad = std::panic::catch_unwind(|| {
            assert_value_matches_tol("v=2.0", "v=1.0", 1e-9, 1e-12, "toobig");
        });
        assert!(bad.is_err(), "out-of-tolerance numbers must FAIL");
    }

    /// [`value_verdict`] is what [`assert_value_matches_tol`] and the
    /// `DSS_PROPS_CENSUS` walk share, so its classification and its `max_rel`
    /// are pinned directly. Every expectation here is a row of the vendored
    /// 2026-08-08 census (`tests/corpus/props_r4133/`): the `max_rel` formula was
    /// re-derived from it and reproduces all 95 317 `value_numeric` rows — of
    /// which only 137 carry more than one number and exactly one discriminates
    /// "max over the offenders" from "max over all numbers" (see
    /// [`ValueVerdict::Numeric`]'s breadth note; the last case below is that
    /// cell's shape).
    #[test]
    fn value_verdict_classifies_and_sizes_like_the_census() {
        use super::{ValueVerdict, value_verdict};
        let feeder = |a: &str, e: &str| value_verdict(a, e, 1e-7, 1e-5);

        // Byte-equal → Match, whatever the tolerance.
        assert_eq!(feeder("Yes", "Yes"), ValueVerdict::Match);
        // `value_structure`: the skeletons differ (census `windgen.enabled`).
        assert_eq!(feeder("Yes", "true"), ValueVerdict::Skeleton);
        // …and the array forms of `line.ratings`.
        assert_eq!(feeder("[ 400]", "[400,]"), ValueVerdict::Skeleton);
        // A differing number COUNT surfaces as `Skeleton`, because every number
        // becomes exactly one `#`: `[ # #]` vs `[ # # #]`. `NumberCount` is the
        // defensive backstop the original `assert_eq!(anums.len(), enums.len())`
        // always was — kept, and kept unreachable, rather than dropped.
        assert_eq!(feeder("[ 1 2]", "[ 1 2 3]"), ValueVerdict::Skeleton);
        // Inside the floor → Match even though the strings differ.
        assert_eq!(feeder("400.0000001", "400"), ValueVerdict::Match);

        // `value_numeric`: max_rel = |a-e|/|e| over the numbers that FAIL.
        // Census row: Capacitor.cpin8 `cuf`, max_rel 1.356840117583752e-6.
        let ValueVerdict::Numeric { max_rel, index, .. } =
            value_verdict("[ 287.82360946885]", "[ 287.824]", 1e-9, 1e-9)
        else {
            panic!("expected a numeric verdict");
        };
        assert_eq!(index, 0);
        assert!(
            (max_rel - 1.356_840_117_583_752e-6).abs() < 1e-18,
            "{max_rel}"
        );

        // e == 0 → the ABSOLUTE gap is the census's max_rel (row: Fault.fa
        // `pctperm` 100 vs 0 → 100.0; WindGen.w1 `kvar` 986.05… vs 0).
        let ValueVerdict::Numeric { max_rel, .. } = value_verdict("100", "0", 1e-9, 1e-9) else {
            panic!("expected a numeric verdict");
        };
        assert_eq!(max_rel, 100.0);

        // The maximum is over the FAILING numbers only, and the reported index is
        // the first OFFENDER, not the first cell. Number 0 here is inside the
        // floor yet has the LARGER relative gap (5e-6, absorbed by `abs`=1e-5);
        // number 1 is the only offender at rel 2e-7. This is exactly the census's
        // `line.cmatrix` shape — it reports 5.83e-8 while the largest gap over
        // ALL of that row's numbers is 1.19e-7.
        let ValueVerdict::Numeric { max_rel, index, .. } =
            value_verdict("1.000005 1000000.2", "1 1000000", 1e-7, 1e-5)
        else {
            panic!("expected a numeric verdict");
        };
        assert_eq!(
            index, 1,
            "the first OFFENDER is reported, not the first cell"
        );
        assert!(
            (max_rel - 2e-7).abs() < 1e-14,
            "max_rel must ignore the in-floor 5e-6 gap of the first number, got {max_rel}"
        );
    }
}

#[cfg(test)]
mod eventlog_mask_tests {
    use super::{EVENTLOG_MASKS, EventLogMask, apply_eventlog_masks};

    /// The §1.3-3 mask mechanism: a documented rule for a target rev folds a
    /// cosmetic delta on the matched spec only; the default oracle (`None`) and
    /// non-matching specs pass through untouched.
    #[test]
    fn masks_fold_documented_delta_on_matching_spec_only() {
        const T: &[(&str, &[EventLogMask])] = &[(
            "testrev",
            &[EventLogMask {
                note: "self-test only",
                find: "opened, delayed",
                to: "opened on ph slow (3ph trip)",
            }],
        )];
        assert_eq!(
            apply_eventlog_masks(
                T,
                Some("testrev"),
                "Action=OPENED, DELAYED".to_lowercase().as_str()
            ),
            "action=opened on ph slow (3ph trip)"
        );
        // Non-matching spec / default oracle → unchanged.
        assert_eq!(
            apply_eventlog_masks(T, Some("r4133"), "action=opened, delayed"),
            "action=opened, delayed"
        );
        assert_eq!(
            apply_eventlog_masks(T, None, "action=opened, delayed"),
            "action=opened, delayed"
        );
    }

    /// The shipped r4133 table has NO rows — the WP-U2.2 Recloser port
    /// reproduces the r4133 wording exactly, so every line passes through the
    /// mask untouched (the empty table is the proof).
    #[test]
    fn shipped_r4133_masks_are_empty_passthrough() {
        let line = "Hour=0, Sec=0.2, ControlIter=1, Element=Recloser.r, Action=PHASE 1 OPENED ON PH FAST (3PH TRIP)";
        assert_eq!(
            apply_eventlog_masks(EVENTLOG_MASKS, Some("r4133"), line),
            line
        );
    }
}

#[cfg(test)]
mod props_015x_tests {
    use super::compare_prop_lists;

    /// Build a `(name, value)` list from string slices.
    fn props(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    /// Synthetic allowlist for the self-tests — never a real shipped row.
    const TEST_ALLOW: &[(&str, &[&str])] = &[("Foo", &["NewTrail", "NewMid"])];

    /// The shape walk is channel-independent, so these drivers pin it on the
    /// capi channel (the shipped gate policy for every 0.14.5 capture).
    fn capi() -> super::PropsPolicy {
        super::PropsPolicy::for_channel(super::PropsChannel::CapiV0145)
    }

    fn run(actual: &[(&str, &str)], oracle: &[(&str, &str)]) {
        compare_prop_lists(
            "Foo.x",
            "Foo",
            &props(actual),
            &props(oracle),
            TEST_ALLOW,
            capi(),
            false,
            1e-9,
            1e-9,
            "self",
        );
    }

    fn expect_panic(actual: &[(&str, &str)], oracle: &[(&str, &str)]) {
        let a = props(actual);
        let o = props(oracle);
        let r = std::panic::catch_unwind(|| {
            compare_prop_lists(
                "Foo.x",
                "Foo",
                &a,
                &o,
                TEST_ALLOW,
                capi(),
                false,
                1e-9,
                1e-9,
                "self",
            );
        });
        assert!(r.is_err(), "expected a panic but the compare passed");
    }

    /// A trailing allowlisted extra (absent from the oracle) is excluded → pass.
    #[test]
    fn trailing_allowlisted_extra_passes() {
        run(
            &[("A", "1"), ("B", "2"), ("NewTrail", "9")],
            &[("A", "1"), ("B", "2")],
        );
    }

    /// An INSERTED allowlisted extra is excluded, and the order contract around
    /// it is preserved (B still lines up with the oracle's B).
    #[test]
    fn inserted_allowlisted_extra_passes() {
        run(
            &[("A", "1"), ("NewMid", "9"), ("B", "2")],
            &[("A", "1"), ("B", "2")],
        );
    }

    /// A non-allowlisted trailing extra still fails (count mismatch).
    #[test]
    fn non_allowlisted_extra_panics() {
        expect_panic(
            &[("A", "1"), ("B", "2"), ("Bogus", "9")],
            &[("A", "1"), ("B", "2")],
        );
    }

    /// An allowlisted prop PRESENT in the oracle capture (capi015) is NOT
    /// excluded — full value compare applies, so a value mismatch panics.
    #[test]
    fn allowlisted_present_in_oracle_value_mismatch_panics() {
        expect_panic(
            &[("A", "1"), ("NewTrail", "9")],
            &[("A", "1"), ("NewTrail", "7")],
        );
        // …and matches when the value agrees.
        run(
            &[("A", "1"), ("NewTrail", "9")],
            &[("A", "1"), ("NewTrail", "9")],
        );
    }

    /// A missing prop (Rust lacks one the oracle has, none allowlisted) fails.
    #[test]
    fn missing_prop_panics() {
        expect_panic(&[("A", "1")], &[("A", "1"), ("B", "2")]);
    }

    /// A misordered (swapped) pair fails on the name check even at equal count.
    #[test]
    fn misordered_prop_panics() {
        expect_panic(&[("B", "2"), ("A", "1")], &[("A", "1"), ("B", "2")]);
    }

    /// The one SHIPPED row that runs backwards (R4133_PROPS RP1.4): the port's
    /// `GenDispatcher.Weights` is present in the 0.14.5/capi015 tables and
    /// **missing from r4133's**, which loses the name to a registration
    /// off-by-one (`Controls/GenDispatcher.pas:92,133`). Pinned here because the
    /// row's whole defence is that it is INERT wherever the oracle knows the
    /// name.
    ///
    /// What this test discriminates (RP1.4 audit round — the original comment
    /// claimed a failure mode it could not tell apart, and both auditors
    /// mutation-proved it):
    ///  * a regression that turned the row into a **value mask** (a
    ///    [`SKIP_PROPS`]-shaped skip) — nothing else goes red for that, because a
    ///    value skip panics nowhere;
    ///  * a regression that made [`filter_015x`] **unconditional** (drop the
    ///    allowlisted prop even when the capture knows it) — the panic-message
    ///    assert below is what separates it from the value panic this test wants,
    ///    since a bare `is_err()` is satisfied by either. That regression is also
    ///    caught loudly, and first, by
    ///    [`allowlisted_present_in_oracle_value_mismatch_panics`] on the
    ///    synthetic table: it fails on the count assert, never silently;
    ///  * deletion of the row itself (the `prop_015x` assert below) — today the
    ///    only exerciser of the row, the RP2.1 replay being unlanded.
    #[test]
    fn shipped_gendispatcher_weights_row_is_inert_when_the_oracle_knows_it() {
        use super::{PROPS_015X, prop_015x};
        assert!(
            prop_015x(PROPS_015X, "gendispatcher", "Weights"),
            "the RP1.4 row must be in the shipped table (case-insensitively)"
        );
        // Same drivers as above, but against the SHIPPED table and the real class.
        let shipped = |actual: &[(&str, &str)], oracle: &[(&str, &str)]| {
            compare_prop_lists(
                "GenDispatcher.gd1",
                "GenDispatcher",
                &props(actual),
                &props(oracle),
                PROPS_015X,
                capi(),
                false,
                1e-9,
                1e-9,
                "self",
            );
        };
        // r4133-shaped name list (slot 7 is `basefreq`, no `weights`): the Rust
        // extra is dropped and the shape walk lines up.
        shipped(
            &[
                ("GenList", "g1, g2"),
                ("Weights", "[3, 1]"),
                ("basefreq", "60"),
            ],
            &[("GenList", "g1, g2"), ("basefreq", "60")],
        );
        // capi-shaped name list (it HAS `weights`): nothing is dropped, so a
        // wrong value still panics — the row buys no value relief there.
        let a = props(&[("GenList", "g1, g2"), ("Weights", "[3, 1]")]);
        let o = props(&[("GenList", "g1, g2"), ("Weights", "[1, 1]")]);
        let r = std::panic::catch_unwind(|| {
            compare_prop_lists(
                "GenDispatcher.gd1",
                "GenDispatcher",
                &a,
                &o,
                PROPS_015X,
                capi(),
                false,
                1e-9,
                1e-9,
                "self",
            );
        });
        // The panic must be the VALUE compare's, not the count assert's: an
        // unconditional `filter_015x` would drop `Weights` here too and panic on
        // the count instead, which a bare `is_err()` cannot tell apart.
        let msg = r.expect_err(
            "the row must not relieve a VALUE compare on a capture that knows the name",
        );
        let msg = msg
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| msg.downcast_ref::<&str>().copied())
            .unwrap_or("")
            .to_string();
        assert!(
            msg.contains("property Weights"),
            "expected the VALUE compare on `Weights` to panic; got {msg:?} — a \
             \"property count differs\" panic here means `filter_015x` dropped the \
             allowlisted prop even though the capture carries it"
        );
    }
}

/// Decode a COM-style interleaved re/im `f64` array (the shape dss-python /
/// the EPRI DLL / the golden JSON files speak) into the natural complex shape.
/// The interleave is a *boundary* encoding: it stops here, at the comparator.
pub fn deinterleave(v: &[f64]) -> Vec<Complex64> {
    v.chunks_exact(2)
        .map(|c| Complex64::new(c[0], c[1]))
        .collect()
}

/// Compare two interleaved re/im arrays element-wise: passes when
/// `|actual − expected| ≤ abs_floor + rel · |expected|` per complex entry.
/// Panics with the first offending index and values. Thin decode wrapper over
/// [`assert_complex_close_c`] for the oracle-shaped channels.
pub fn assert_complex_close(
    actual: &[f64],
    expected: &[f64],
    rel: f64,
    abs_floor: f64,
    what: &str,
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{what}: length mismatch ({} vs {})",
        actual.len(),
        expected.len()
    );
    assert_eq!(actual.len() % 2, 0, "{what}: odd interleaved length");
    assert_complex_close_c(
        &deinterleave(actual),
        &deinterleave(expected),
        rel,
        abs_floor,
        what,
    );
}

/// Compare two complex arrays element-wise: passes when
/// `|actual − expected| ≤ abs_floor + rel · |expected|` per entry. Panics with
/// the first offending index and values.
pub fn assert_complex_close_c(
    actual: &[Complex64],
    expected: &[Complex64],
    rel: f64,
    abs_floor: f64,
    what: &str,
) {
    assert_eq!(
        actual.len(),
        expected.len(),
        "{what}: length mismatch ({} vs {})",
        actual.len(),
        expected.len()
    );
    for (i, (a, e)) in actual.iter().zip(expected).enumerate() {
        let (ar, ai) = (a.re, a.im);
        let (er, ei) = (e.re, e.im);
        let diff = ((ar - er).powi(2) + (ai - ei).powi(2)).sqrt();
        let mag = (er * er + ei * ei).sqrt();
        let allowed = abs_floor + rel * mag;
        assert!(
            diff <= allowed,
            "{what}: entry {i} differs: actual ({ar}, {ai}) vs expected ({er}, {ei}); \
             |diff| = {diff:e} > allowed {allowed:e}",
        );
    }
}

// ---------------------------------------------------------------------------
// Assembled-model comparison layer.
//
// Shared by the checkpointed-model gate (`golden_checkpoints.rs`, against
// committed schema-2 goldens) and the live corpus gate (`corpus_live.rs`,
// against the pinned oracle at test time). Both compare the captured electrical
// model — system Y, element YPrim, injection vector, element currents/powers,
// and discrete control state — using the same tolerance policy
// (`Tolerances`/`tol_for`). See tests/TOLERANCE_NOTES.md for the per-field rationale.
// ---------------------------------------------------------------------------

/// Full assembled-Y CSC coordinates (micro/feeder scenarios); `(rows[k],
/// cols[k]) -> re[k] + j*im[k]`, 0-based, row i ↔ node order index i.
#[derive(Debug, Deserialize)]
pub struct YMat {
    pub n: usize,
    pub rows: Vec<usize>,
    pub cols: Vec<usize>,
    pub re: Vec<f64>,
    pub im: Vec<f64>,
}

/// A selected element's YPrim block, column-major flat (`out[col*yorder+row]`),
/// re/im split — the raw `CktElement.Yprim` / `CMatrix` storage order.
#[derive(Debug, Deserialize)]
pub struct YPrim {
    pub name: String,
    pub yorder: usize,
    pub re: Vec<f64>,
    pub im: Vec<f64>,
}

/// Compact fingerprint of the assembled system Y (large feeders): nnz above a
/// floor, Frobenius norm, complex trace, max|diagonal|.
#[derive(Debug, Deserialize)]
pub struct YFingerprint {
    pub nnz: usize,
    pub frob: f64,
    pub tr_re: f64,
    pub tr_im: f64,
    pub maxdiag: f64,
}

/// An element's terminal currents (A, re/im) and powers (kW/kvar).
/// `loss_w` (W, var — the oracle `CktElement.Losses`, i.e. the engine's own
/// `Get_Losses` path) is captured by the live gate only; committed checkpoint
/// goldens predate it and leave it empty (skipped).
#[derive(Debug, Deserialize)]
pub struct ElementCap {
    pub name: String,
    pub i_re: Vec<f64>,
    pub i_im: Vec<f64>,
    pub p_kw: Vec<f64>,
    pub p_kvar: Vec<f64>,
    #[serde(default)]
    pub loss_w: Vec<f64>,
}

/// The node injection-current vector (RHS of Y*V=I), nodes 1..n.
#[derive(Debug, Deserialize)]
pub struct Injection {
    pub re: Vec<f64>,
    pub im: Vec<f64>,
}

/// Tolerance policy (PORTING_PLAN.md §4; per-field rationale in
/// tests/TOLERANCE_NOTES.md): the per-scenario-kind relative/absolute tolerances,
/// built by [`tol_for`]. Discrete state (taps, switch positions, iteration
/// counts) is always compared exactly and never goes through this.
///
/// DO NOT loosen these to make a failing oracle comparison pass. Every floor is
/// calibrated to *proven* f64/f32/faer-vs-KLU reality; a Rust↔oracle gap above its
/// floor is a porting **bug** — find the cause, never widen the band to hide it
/// (CLAUDE.md §"conditioning"). They tighten with proof, never loosen to mask a
/// divergence.
pub struct Tolerances {
    pub v_rel: f64,
    pub v_abs: f64,
    pub y_rel: f64,
    pub y_abs: f64,
    /// Currents (A) and powers (kW/kvar) of elements + the injection vector.
    pub i_rel: f64,
    pub i_abs: f64,
    /// EnergyMeter register accumulations (kWh/kvarh/...): the integration policy
    /// (PORTING_PLAN §4), looser than the per-step electrical quantities because
    /// rounding accumulates over the run.
    pub energy_rel: f64,
    pub energy_abs: f64,
}

/// Map a scenario `kind` to its tolerance class.
pub fn tol_for(kind: &str) -> Tolerances {
    match kind {
        // Micro circuits: everything to ~1e-9 rel; small abs floors below
        // physical significance (volts / siemens / amps).
        "micro" => Tolerances {
            v_rel: 1e-9,
            v_abs: 1e-6,
            y_rel: 1e-9,
            y_abs: 1e-6,
            i_rel: 1e-9,
            i_abs: 1e-6,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // Standard feeders (IEEE13/37/123 and the simpler Test/ circuits): the
        // assembled Y is deterministic (1e-8); voltages hold 1e-8 and
        // currents/powers 1e-7 rel / 1e-5 abs — empirically verified over every
        // feeder-kind corpus case (corpus_live). The stiff minority that can't make
        // this floor is reclassified `large` (below): 4Bus-YYD voltages match only
        // ~7e-8, and the PVSystem `CurrentkvarLimite` `Vsource` current differs
        // ~1.2e-4 A.
        "feeder" => Tolerances {
            v_rel: 1e-8,
            v_abs: 1e-6,
            y_rel: 1e-8,
            y_abs: 1e-6,
            i_rel: 1e-7,
            i_abs: 1e-5,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // The floating-delta zero-sequence class (currently the IEEE123 GFM
        // snapshot deck): a bus fed by a delta transformer winding with a delta
        // DER has no zero-sequence path to ground — its common-mode voltage is
        // pinned only by the winding's anti-float adder (−j1.4468e-6 S = 2·Y_PPM)
        // against a ~452 S diagonal, a κ≈3.1e8 subspace inside an otherwise
        // well-conditioned solve. That common mode is solver-junk: on
        // bit-identical (Y, I) KLU / scipy / faer each leave a *different* stable
        // value (spreads 1.2e-5 … 6.6e-5 V; the engines land 9.03e-5 V apart at
        // step 0 — 100% common mode, differential remainder ≤5.9e-8 V). `v_abs`
        // absorbs exactly that junk: 5e-4 ≈ 5.5× the measured worst, still only
        // ~1.8e-6 rel at the 277 V DER buses. Everything else keeps the `large`
        // floors — in particular the DER elements' currents/powers are functions
        // of the L-L (differential) voltages, immune to the common mode, so real
        // model bugs at these buses stay caught. Empirical proof (CLAUDE.md: a
        // floor changes only with proof by decomposition): tests/TOLERANCE_NOTES.md
        // §floating-delta; fix owner RESONANCE_PLAN.md WP-R1 (iterative refinement
        // lands the faer junk 3× under the `large` band → retighten this tier to
        // `large` then).
        "large_floating_delta" => Tolerances {
            v_rel: 1e-7,
            v_abs: 5e-4,
            y_rel: 1e-8,
            y_abs: 1e-6,
            i_rel: 1e-6,
            i_abs: 1e-4,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // A-Diakoptics torn circuits stitched with deliberate ultra-switches
        // (`Line.other_feeders` r1=1e-8 Ω → Y≈1e8 S; EPRI_Ckt7-G `Line.333`
        // alike): the pseudo-switch current is Y·(V1−V2) where the engines'
        // (V1−V2) agree to <2 f64-ulps of the ~2e4 V node voltage — measured
        // dI 6.5e-4 A on a 375 A flow (ckt24) = exactly 1.8 ulp × 1e8 S, an
        // arithmetic bit-floor, not a model gap (the f32-looking values are
        // the coarse dyadic grid such near-cancellation differences live on).
        // Only `i_abs` widens (2e-3 = worst 6.5e-4 ×3); voltages hold the full
        // `large` floors and Y stays tight — a real stitching/model bug still
        // shows at ampere scale or in Y.
        "large_ultra_switch" => Tolerances {
            v_rel: 1e-7,
            v_abs: 1e-6,
            y_rel: 1e-8,
            y_abs: 1e-6,
            i_rel: 1e-6,
            i_abs: 2e-3,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // The floating zero-sequence class, weak-pinning members (proof per
        // deck in tests/TOLERANCE_NOTES.md §floating-zeroseq): a subsystem fed
        // ONLY through delta windings has no zero-seq ground path; its common
        // mode is pinned by the ppm anti-float adders alone, with measured
        // amplification 1.4e10 (TestDDRegulator REGBUS2) / 4.2e11 (DG_Prot_Fdr
        // dead-end BG) / ~1e8 across the whole delta-delta-fed 13.8 kV system
        // of LVTestCaseNorthAmerican (and the DOCTechNote/{1_1,1_2,2_1,2_2}
        // decks built on it: an LV SLG / MV L-L fault / breaker-open never
        // adds an MV zero-seq ground path, so the 13.8 kV system still floats;
        // common mode 1.8e-3…2.85e-3 V, L-L agreement ≤1.4e-10 rel, 0/1170
        // nodes above `large` after removing the per-bus shift). The V gap is
        // 100% common mode (per-bus differential ≤1e-5 V, within the `large`
        // floors; DG_Prot bit-level ≤1.3e-13), Y agrees to ≤1.3e-15 rel (libm
        // last-ulp on geometry decks; bit-identical on DDReg/DG_Prot),
        // injections/iterations match.
        // `v_abs` 3e-2 = worst measured common mode (1.06e-2, DG_Prot) ×2.8;
        // at the smallest affected buses (346 V) that is still 8.7e-5 rel.
        // Element currents/powers are functions of the DIFFERENTIAL voltages —
        // immune to the common mode — so the tier keeps every other floor at
        // `large` and real model bugs stay caught (same argument as
        // `large_floating_delta`, whose GFMSnap member is more strongly pinned
        // and keeps its tighter 5e-4 band).
        "large_floating_zeroseq" => Tolerances {
            v_rel: 1e-7,
            v_abs: 3e-2,
            y_rel: 1e-8,
            y_abs: 1e-6,
            i_rel: 1e-6,
            i_abs: 1e-4,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // The AutoTrans near-ideal-source family (κ≈1e12: mvasc3=2e6 source +
        // 1e-6 Ω switches + floating delta tertiary). Its V gap is the PROVEN
        // cross-solver junk floor — proof by decomposition, not by sweep
        // (tests/TOLERANCE_NOTES.md §near-ideal-source): assembled Y and every
        // element's Currents/Powers formula are bit-identical on identical
        // inputs; the single 1-ulp RHS component measures 1.5e-11 V; residual
        // parity ‖Y·V−I‖ KLU 7.1e-2 vs faer 1.3e-1 (the oracle is no cleaner);
        // scipy lands 51.6 V from BOTH engines (the equivalent-solution set
        // spans ~51 V — the engines' 3.7e-3 V gap is 4 orders tighter). The
        // floor propagates LINEARLY into the stiff-entry small currents
        // (dI = Y_src·dV, measured 6.2e-2…9.4e-2 A against a 0.15 A no-load
        // current) and powers (dS = V·dI ≈ 12–19 kVA), so `i_abs` must absorb
        // exactly that image (0.1 A, user-set — ×1.07 over the measured worst
        // 9.375e-2 A; a future faer/pin bump tripping it is a re-triage
        // signal, NOT a widen-the-band signal); the voltage-scaled power floor
        // maps it to the powers channel automatically. Large currents
        // (fault/full-load checks, hundreds of A…kA) still hold `i_rel` = the
        // `large` floor, and `v_rel` 5e-6 covers the V junk with no `v_abs`
        // change. What keeps real element bugs caught despite the wide
        // `i_abs`: the family's unique surface is YPrim assembly, and the Y
        // channel KEEPS the tight `large` floors (bit-identical today — any
        // Y-level drift is a real regression); the report formulas are
        // corpus-shared and pinned tight elsewhere. Retighten under WP-R1
        // only if refinement is proven to shrink the measured junk.
        "large_near_ideal_source" => Tolerances {
            v_rel: 5e-6,
            v_abs: 1e-6,
            y_rel: 1e-8,
            y_abs: 1e-6,
            i_rel: 1e-6,
            i_abs: 0.1,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // Large / numerically-stiff networks (EPRI ckt5, 8500-node, A-Diakoptics
        // torn zones, inverter cases, 4Bus-YYD): voltages 1e-7; the deterministic Y
        // still holds 1e-8. Currents/powers stay 1e-6 rel / 1e-4 abs — their
        // faer-vs-KLU floor (EPRI ckt5 `Line.mdv201_c_1_266_abc8079` conductor power
        // ~1.3e-6; a PVSystem `Vsource` current ~1.2e-4 A; high-voltage near-zero
        // through-currents ×20 kV). The abs floors absorb dead-end / cancellation
        // quantities (µA branch currents, kW that sum to ~0). Also the safe default
        // WP-U1.8 — the WindGen WTG3 dynamics sub-cycle floor. PROVEN by
        // decomposition (CLAUDE.md: a floor changes only with proof, never a
        // sweep), not a widen-the-band: the snapshot operating point that seeds
        // dynamics matches the oracle to the solver floor (node V ≤3.7e-9 abs on
        // the 398 V L-N buses, feeder-tight), and every WTG3 state variable that
        // is NOT touched by the phase-locked loop matches to ≤2e-7 (Pcmd 4.8e-8,
        // Vref 1.2e-7, Vmag 4.6e-7, WtAct 1.9e-8, thetaPitch 1.5e-7; Pg/Ps/Pr/s
        // bit-exact). Only the three PLL-fed quantities are loose — `dOmg`
        // (9.1e-6), `Pgen` (9.7e-6), `Qgen` (3.4e-6) — because `PllLogic`'s
        // derivative term `KpPLL·(Vq−VqOld)/deltSim = 60·Δ/0.001 = 60000·Δ`
        // amplifies the last-ulp difference in `Vq` (itself the small imaginary
        // residual ~3.5e-3 of a voltage the PLL rotates onto the real axis — a
        // near-cancellation) by 6e4×; the amplified `dOmg` then feeds the whole
        // trajectory (this is the WPG.13/dSpeed cancellation-floor class). The
        // gap DECAYS as the startup transient settles (node V |Δ| 8e-5 @ 1 step
        // → 4.4e-6 @ 20 → 1.6e-6 @ 100), so it is not a divergent state-leak.
        // The amplified injection current (≈1e-5 rel) reaches the WindGen
        // TERMINAL bus voltage through the small series line (`V_term = V_src −
        // I·Z_line`); the far SOURCE bus stays feeder-tight (3.5e-8 rel) because
        // the ideal source buffers it. Two members, calibrated to the binding
        // (fault) case × ~2:
        //   • `windgen_dyn` (healthy): wbus |Δ|3.9e-4 V (9.6e-7 rel), dOmg/Pgen/
        //     Qgen |Δ| 9.1e-6/9.7e-6/3.4e-6.
        //   • `windgen_dyn_fault` (sustained 3φ fault → LVPL/LVQL ride-through,
        //     the high-gain low-voltage logic on top of the PLL): wbus |Δ|1.23e-3
        //     V (3.4e-6 rel), dOmg/Pgen/Qgen |Δ| 4.2e-5/3.4e-5/1.8e-5. srcbus
        //     stays 3.5e-8 rel.
        // `v_rel` 8e-6 covers the fault wbus 3.4e-6 rel (×2.3); `i_abs` 1e-4
        // covers the per-unit small-variable amplification (`dOmg` 0.08, |Δ|
        // 4.2e-5); `i_rel` 2e-5 covers the ~1e-5-rel amplified per-unit/ampere
        // currents. `y_rel`/`y_abs` stay tight (the dynamic Norton YPrim is a
        // deterministic closed-form). A real WTG3 model bug moves the non-PLL
        // variables (all ≤2e-6 here) or the assembled Y far past these floors.
        "micro_wtg3_dynamics" => Tolerances {
            v_rel: 8e-6,
            v_abs: 1e-6,
            y_rel: 1e-9,
            y_abs: 1e-6,
            i_rel: 2e-5,
            i_abs: 1e-4,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
        // for an unrecognized kind.
        _ => Tolerances {
            v_rel: 1e-7,
            v_abs: 1e-6,
            y_rel: 1e-8,
            y_abs: 1e-6,
            i_rel: 1e-6,
            i_abs: 1e-4,
            energy_rel: 1e-4,
            energy_abs: 1e-4,
        },
    }
}

/// Coordinate map of a captured Y matrix, keyed by (row, col).
fn y_coord_map(y: &YMat) -> BTreeMap<(usize, usize), Complex64> {
    let mut m = BTreeMap::new();
    for i in 0..y.rows.len() {
        m.insert((y.rows[i], y.cols[i]), Complex64::new(y.re[i], y.im[i]));
    }
    m
}

/// Compare the assembled system Y entry-by-entry over the union of both
/// patterns. Reports the worst offenders (largest |rel|) with node names, the
/// nnz on each side, and whether the sparsity pattern changed.
pub fn compare_system_y(
    dss: &mut Dss,
    exp: &YMat,
    node_order: &[String],
    tol: &Tolerances,
    ctx: &str,
) {
    let (n, coords) = dss
        .system_y_csc()
        .unwrap_or_else(|| panic!("{ctx}: Rust has no assembled system Y"));
    assert_eq!(n, exp.n, "{ctx}: system Y order differs ({n} vs {})", exp.n);

    let exp_map = y_coord_map(exp);
    let mut act_map: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    for (r, c, v) in coords {
        // A CSC dump should not repeat (r,c), but sum defensively.
        *act_map.entry((r, c)).or_insert(Complex64::ZERO) += v;
    }

    // Union of keys.
    let mut keys: Vec<(usize, usize)> = exp_map.keys().copied().collect();
    for k in act_map.keys() {
        if !exp_map.contains_key(k) {
            keys.push(*k);
        }
    }

    let name = |i: usize| node_order.get(i).map(String::as_str).unwrap_or("?");
    // Count nonzeros above the abs floor on each side (drops cancellation zeros).
    let above = |m: &BTreeMap<(usize, usize), Complex64>| {
        m.values().filter(|v| v.norm() > tol.y_abs).count()
    };
    let nnz_act = above(&act_map);
    let nnz_exp = above(&exp_map);

    // Collect every offender; report the worst few sorted by relative error.
    let mut offenders: Vec<(f64, String)> = Vec::new();
    let mut pattern_diffs = 0usize;
    for (r, c) in keys {
        let a = act_map.get(&(r, c)).copied().unwrap_or(Complex64::ZERO);
        let e = exp_map.get(&(r, c)).copied().unwrap_or(Complex64::ZERO);
        let diff = (a - e).norm();
        let allowed = tol.y_abs + tol.y_rel * e.norm();
        if diff > allowed {
            let in_exp = exp_map.contains_key(&(r, c));
            let in_act = act_map.contains_key(&(r, c));
            let pattern = if in_exp == in_act {
                "match"
            } else {
                pattern_diffs += 1;
                if in_act {
                    "EXTRA in actual"
                } else {
                    "MISSING in actual"
                }
            };
            let rel = diff / e.norm().max(f64::MIN_POSITIVE);
            offenders.push((
                rel,
                format!(
                    "Y[{r},{c}] ({}, {}): actual ({:.6e},{:.6e}) vs oracle ({:.6e},{:.6e})  \
                     |abs|={diff:.3e} |rel|={rel:.3e} > allowed {allowed:.3e} (pattern {pattern})",
                    name(r),
                    name(c),
                    a.re,
                    a.im,
                    e.re,
                    e.im,
                ),
            ));
        }
    }
    if offenders.is_empty() {
        return;
    }
    offenders.sort_by(|a, b| b.0.total_cmp(&a.0));
    let top: String = offenders
        .iter()
        .take(5)
        .map(|(_, m)| format!("\n    {m}"))
        .collect();
    panic!(
        "{ctx}: system Y mismatch — {} entries out of tolerance ({pattern_diffs} structural); \
         nnz actual={nnz_act} oracle={nnz_exp}{}\n  top offenders:{top}",
        offenders.len(),
        if nnz_act != nnz_exp {
            "  <-- SPARSITY PATTERN CHANGED"
        } else {
            ""
        },
    );
}

/// Compare the assembled-Y fingerprint: nnz (above floor) exact, Frobenius
/// norm / trace / max|diag| at `y_rel`. The cheap structural+magnitude guard.
pub fn compare_fingerprint(dss: &mut Dss, exp: &YFingerprint, tol: &Tolerances, ctx: &str) {
    const FLOOR: f64 = 1e-9;
    let (_n, coords) = dss
        .system_y_csc()
        .unwrap_or_else(|| panic!("{ctx}: Rust has no assembled system Y"));
    let mut nnz = 0usize;
    let mut frob = 0.0f64;
    let mut tr = Complex64::ZERO;
    let mut maxdiag = 0.0f64;
    for (r, c, v) in &coords {
        let m = v.norm();
        if m > FLOOR {
            nnz += 1;
        }
        frob += m * m;
        if r == c {
            tr += *v;
            maxdiag = maxdiag.max(m);
        }
    }
    let frob = frob.sqrt();
    assert_eq!(
        nnz, exp.nnz,
        "{ctx}: Y fingerprint nnz differs (actual {nnz} vs oracle {}) — SPARSITY PATTERN CHANGED",
        exp.nnz
    );
    let close = |a: f64, e: f64, what: &str| {
        let allowed = tol.y_abs + tol.y_rel * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: Y fingerprint {what} differs: actual {a:.9e} vs oracle {e:.9e} \
             (|diff|={:.3e} > allowed {allowed:.3e})",
            (a - e).abs()
        );
    };
    close(frob, exp.frob, "Frobenius norm");
    close(tr.re, exp.tr_re, "trace.re");
    close(tr.im, exp.tr_im, "trace.im");
    close(maxdiag, exp.maxdiag, "max|diag|");
}

/// Compare a selected element's YPrim block (column-major flat, same layout on
/// both sides).
pub fn compare_yprim(dss: &Dss, exp: &YPrim, tol: &Tolerances, ctx: &str) {
    let (yorder, flat) = dss
        .element_yprim(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no element {} (or no Yprim)", exp.name));
    assert_eq!(
        yorder, exp.yorder,
        "{ctx}: {} yorder differs ({yorder} vs {})",
        exp.name, exp.yorder
    );
    assert_eq!(
        flat.len(),
        exp.re.len(),
        "{ctx}: {} Yprim length differs",
        exp.name
    );
    for (k, a) in flat.iter().enumerate() {
        let e = Complex64::new(exp.re[k], exp.im[k]);
        let diff = (a - e).norm();
        let allowed = tol.y_abs + tol.y_rel * e.norm();
        let row = k % yorder;
        let col = k / yorder;
        assert!(
            diff <= allowed,
            "{ctx}: {} Yprim[{row},{col}] differs: actual ({:.6e},{:.6e}) vs oracle ({:.6e},{:.6e}); \
             |diff|={diff:.3e} > allowed {allowed:.3e}",
            exp.name,
            a.re,
            a.im,
            e.re,
            e.im
        );
    }
}

/// Compare the node injection-current vector (nodes 1..n; slot 0 = ground).
pub fn compare_injection(dss: &Dss, exp: &Injection, tol: &Tolerances, ctx: &str) {
    let cur = dss.node_injection_currents();
    let n = exp.re.len();
    assert!(
        cur.len() > n,
        "{ctx}: injection vector too short ({} nodes, need > {n})",
        cur.len(),
    );
    let expected: Vec<Complex64> = exp
        .re
        .iter()
        .zip(&exp.im)
        .map(|(re, im)| Complex64::new(*re, *im))
        .collect();
    assert_complex_close_c(
        &cur[1..=n],
        &expected,
        tol.i_rel,
        tol.i_abs,
        &format!("{ctx} injection currents"),
    );
}

/// Compare terminal powers (kW/kvar) per conductor with a **voltage-scaled**
/// absolute floor, the image of the current floor through `P = V·conj(I)`.
///
/// A tolerated terminal-current error `abs_floor` (amps) maps to a power error of
/// `|V_terminal|·abs_floor` (VA) — so comparing power to a *flat* `abs_floor` kW
/// floor while comparing current to a flat `abs_floor` A floor is internally
/// inconsistent (they agree only at |V| = 1 V). Near-zero-impedance connector
/// lines (1.5 m `BUSBAR` segments, `switch=y`; |Yprim| ~ 1e6) make this bite: the
/// through-current `I = Yprim·(V1−V2)` is a catastrophic cancellation whose
/// ~1e-8-rel voltage roundoff (faer vs KLU on an ill-conditioned Y, cond ~ 1e7)
/// surfaces as a ~3e-6-rel current/power error. The current's own `abs_floor`
/// absorbs it; a flat kW power floor does not. So the power abs floor is
/// `abs_floor · max(1, |V_kv|)` with `|V_kv| = |P_kW| / |I_A|` (the terminal
/// voltage in kV, recovered from the captured power and current — self-consistent
/// under positive-sequence ×3, where `|P|` and the accepted `δP` scale together).
/// `max(1, …)` never tightens below the established floor. See
/// tests/TOLERANCE_NOTES.md.
pub fn assert_power_close(
    actual: &[Complex64],
    exp: &ElementCap,
    rel: f64,
    abs_floor: f64,
    what: &str,
) {
    let (p_kw, p_kvar) = (&exp.p_kw, &exp.p_kvar);
    let (i_re, i_im) = (&exp.i_re, &exp.i_im);
    assert_eq!(
        actual.len(),
        p_kw.len(),
        "{what}: power length mismatch ({} vs {})",
        actual.len(),
        p_kw.len()
    );
    assert_eq!(p_kw.len(), p_kvar.len(), "{what}: kW/kvar length mismatch");
    assert_eq!(
        p_kw.len(),
        i_re.len(),
        "{what}: power/current length mismatch"
    );
    for k in 0..p_kw.len() {
        let (ar, ai) = (actual[k].re, actual[k].im);
        let (er, ei) = (p_kw[k], p_kvar[k]);
        let diff = ((ar - er).powi(2) + (ai - ei).powi(2)).sqrt();
        let p_mag = (er * er + ei * ei).sqrt();
        let i_mag = (i_re[k] * i_re[k] + i_im[k] * i_im[k]).sqrt();
        // |V_kv| = |P_kW| / |I_A| (terminal kV); the power error is |V|·(δI).
        let vkv = if i_mag > 1e-12 { p_mag / i_mag } else { 1.0 };
        let allowed = abs_floor * vkv.max(1.0) + rel * p_mag;
        assert!(
            diff <= allowed,
            "{what}: conductor {k} differs: actual ({ar}, {ai}) vs expected ({er}, {ei}); \
             |diff| = {diff:e} > allowed {allowed:e} (|V| = {vkv:.4} kV)",
        );
    }
}

/// Which sub-channels of an element capture [`compare_element_channels`] checks.
///
/// Exists for the lane policy: a *deliberate* divergence (the engine's
/// post-Newton `Powers`/`Losses`, recomputed at the converged `NodeV` where
/// every oracle channel reports the one-step-stale current — CLAUDE.md upstream
/// bug 5, torn down in both lanes by `GOLDEN_REBASE_PLAN.md` G2.3) is excluded
/// **field-by-field**, never case-by-case — the element name set, terminal
/// currents, node voltages, discrete state and iteration count of such a case
/// stay fully oracle-gated.
/// [`lane::elem_channels_for`] is the only thing that ever returns a value other
/// than [`ElemChannels::ALL`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ElemChannels {
    pub currents: bool,
    pub powers: bool,
    pub losses: bool,
}

impl ElemChannels {
    /// Every sub-channel — what every caller but the Stage F lane policy uses.
    pub const ALL: Self = Self {
        currents: true,
        powers: true,
        losses: true,
    };
    /// Currents only: the `S = V·conj(I)` channels are a deliberate divergence
    /// in this lane and are pinned by their own expected-value test instead.
    pub const CURRENTS_ONLY: Self = Self {
        currents: true,
        powers: false,
        losses: false,
    };
}

/// Compare one element's terminal currents and powers against a capture.
pub fn compare_element(snaps: &[ElementSnapshot], exp: &ElementCap, tol: &Tolerances, ctx: &str) {
    compare_element_channels(snaps, exp, tol, ctx, ElemChannels::ALL);
}

/// [`compare_element`] restricted to `channels` — see [`ElemChannels`].
pub fn compare_element_channels(
    snaps: &[ElementSnapshot],
    exp: &ElementCap,
    tol: &Tolerances,
    ctx: &str,
    channels: ElemChannels,
) {
    // The element must exist in the Rust snapshot under EVERY channel policy:
    // excluding a value channel must never excuse a missing element.
    let snap = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&exp.name))
        .unwrap_or_else(|| panic!("{ctx}: no element {}", exp.name));
    // …and neither may it excuse a SHAPE mismatch: conductor counts are
    // structure, not value, so they are asserted under every channel policy
    // (`assert_power_close` also checks them, but only when it runs).
    assert_eq!(
        snap.powers.len(),
        exp.p_kw.len(),
        "{ctx} {}: power length mismatch",
        exp.name
    );
    assert_eq!(
        snap.currents.len(),
        exp.i_re.len(),
        "{ctx} {}: current length mismatch",
        exp.name
    );
    let ei: Vec<Complex64> = exp
        .i_re
        .iter()
        .zip(&exp.i_im)
        .map(|(re, im)| Complex64::new(*re, *im))
        .collect();
    if channels.currents {
        assert_complex_close_c(
            &snap.currents,
            &ei,
            tol.i_rel,
            tol.i_abs,
            &format!("{ctx} {} currents", exp.name),
        );
    }
    // Powers use a terminal-voltage-scaled abs floor (see `assert_power_close`):
    // `P = V·conj(I)` so the power floor must be the current floor times |V|, or
    // high-voltage near-cancellation through-power (switch/busbar connectors)
    // fails on solver roundoff the current floor already absorbs.
    if channels.powers {
        assert_power_close(
            &snap.powers,
            exp,
            tol.i_rel,
            tol.i_abs,
            &format!("{ctx} {} powers", exp.name),
        );
    }
    // Losses (`Get_Losses` — the engine's own losses path, distinct from the
    // per-conductor powers above even though mathematically it is their sum).
    // Captured by the live gate only; old checkpoint goldens leave it empty.
    // The allowed error is the exact accumulation of the per-conductor power
    // tolerance: losses = Σ_k S_k, so |δ(losses)| ≤ Σ_k (abs·|V_k| + rel·|S_k|)
    // — no new tolerance class, just the conductor policy summed.
    if channels.losses && exp.loss_w.len() == 2 {
        let mut allowed_kw = 0.0;
        for k in 0..exp.p_kw.len() {
            let p_mag = (exp.p_kw[k].powi(2) + exp.p_kvar[k].powi(2)).sqrt();
            let i_mag = (exp.i_re[k].powi(2) + exp.i_im[k].powi(2)).sqrt();
            let vkv = if i_mag > 1e-12 { p_mag / i_mag } else { 1.0 };
            allowed_kw += tol.i_abs * vkv.max(1.0) + tol.i_rel * p_mag;
        }
        let allowed_w = allowed_kw * 1000.0;
        let (ar, ai) = snap.loss_w;
        let (er, ei) = (exp.loss_w[0], exp.loss_w[1]);

        // The oracle's `Get_Losses` must be self-consistent with its OWN captured
        // per-conductor powers (`losses = Σ_k S_k`, W) before we trust it as a
        // cross-engine reference. capi015 (dss_capi 0.15.x) has a stale-cache
        // quirk here: in a multi-step *daily* run `CktElement.Losses` freezes at
        // the step-0 value while `Powers` scales correctly (probed 2026-07-12 on a
        // plain grid-connected daily Load AND the islanded GFM decks — general,
        // unrelated to GFM/B5; DIVERGENCES.md §capi015-daily-losses). Rust — like
        // 0.14.5 and EPRI r4133 — recomputes losses fresh, so comparing the two is
        // a §1.2-forbidden deliberate mismatch. When the oracle's own
        // losses≠Σpowers we skip ONLY this redundant channel (the per-conductor
        // Powers above already pin the same physics); a self-consistent oracle
        // still fully gates the `Get_Losses` path.
        let (osum_re, osum_im): (f64, f64) = exp
            .p_kw
            .iter()
            .zip(&exp.p_kvar)
            .fold((0.0, 0.0), |(r, i), (p, q)| {
                (r + p * 1000.0, i + q * 1000.0)
            });
        let oracle_self_gap = ((er - osum_re).powi(2) + (ei - osum_im).powi(2)).sqrt();
        let oracle_losses_trustworthy = oracle_self_gap <= allowed_w.max(1e-6);

        let diff = ((ar - er).powi(2) + (ai - ei).powi(2)).sqrt();
        assert!(
            !oracle_losses_trustworthy || diff <= allowed_w,
            "{ctx} {} losses differ: actual ({ar}, {ai}) W vs oracle ({er}, {ei}) W; \
             |diff| = {diff:e} > allowed {allowed_w:e}",
            exp.name
        );
    }
}

/// One element-specific state probe: the value string of `element`'s property
/// `prop` — oracle `Properties(p).Val` vs the Rust `?` query (both render via
/// the class property surface). Compared as a numeric skeleton (numbers by
/// value at the case tolerance, text case-insensitively), so number *rendering*
/// is not load-bearing but every digit-bearing state (taps, kWh, counters) and
/// every enum/state word is.
#[derive(Debug, Deserialize)]
pub struct ProbeCap {
    pub element: String,
    pub prop: String,
    pub value: String,
}

/// Compare one property probe via the Rust `?` query (`do_query_cmd`).
pub fn compare_probe(dss: &mut Dss, exp: &ProbeCap, tol: &Tolerances, ctx: &str) {
    dss.command(&format!("? {}.{}", exp.element, exp.prop));
    let actual = dss.result().to_string();
    assert!(
        !actual.eq_ignore_ascii_case("Property Unknown"),
        "{ctx}: probe {}.{}: property unknown to the port",
        exp.element,
        exp.prop
    );
    assert_value_matches_tol(
        &actual.to_lowercase(),
        &exp.value.to_lowercase(),
        tol.i_rel,
        tol.i_abs,
        &format!("{ctx}: probe {}.{}", exp.element, exp.prop),
    );
}

/// WP8.5b corpus property parity: every property of one circuit element —
/// oracle `Properties(p).Val` over `AllPropertyNames` (read via `? name.prop`,
/// the WPG.1-safe probe path) as ordered `(name, value)` pairs. Compared
/// against the Rust `?`-surface (`Dss::element_properties`) property-for-property
/// by [`compare_all_properties`]: the property-index order is the contract.
#[derive(Debug, Deserialize)]
pub struct PropsCap {
    pub element: String,
    /// `[[prop_name, value_string], ...]` in `AllPropertyNames` order.
    pub props: Vec<(String, String)>,
}

/// `(class, prop)` pairs (matched case-insensitively) whose value is provably
/// NOT comparable property-for-property between the Rust `?`-surface and the
/// pinned oracle — a comparability EXCLUSION whose proof is cited in
/// `tests/TOLERANCE_NOTES.md` (§"WP8.5b property parity"), NEVER a tolerance
/// loosening. The property NAME is still order-checked (only the VALUE compare
/// is skipped). Populated only after the Phase-A pilot triage proves a prop
/// non-comparable (path echo / RNG / oracle UB); empty until then.
///
/// **Known structural gap — no liveness guard** (recorded by the RP3.8 audit
/// settlement, 2026-09-02; pre-dates it). `skip_props_disposition_tests`
/// enforces that every row is *decided* and that the two channel lists partition
/// it, but nothing checks that a row still masks a real cell. A row that has become dead
/// is accepted silently — and for the (g) rows that matters in a specific way:
/// their failure mode is the engine reverting to `''`, after which the capi
/// compare AGREES and the gate stays green over five dead masks. What covers
/// that today is out-of-harness: the expected-value pins
/// (`props_r4133_pins::indmach012_pf_renders_the_live_power_factor` /
/// `…storagecontroller_fleet_aggregates_render_the_live_fleet`, the unit pins,
/// and `props_r4133_replay::the_rp38_pairs_are_superseded_by_the_live_render`),
/// each of which reds on exactly that revert — plus the r4133 channel, which
/// value-compares all five. A per-row mask counter threaded through the corpus
/// gate would close it in-harness for all 17 rows; that is harness work no
/// sub-step owns (checked against §RP3.9-§RP4.1), not RP3.8's.
const SKIP_PROPS: &[(&str, &str)] = &[
    // (class, prop) — each row is a proven comparability exclusion cited in
    // tests/TOLERANCE_NOTES.md §"WP8.5b property parity"; NEVER a tolerance
    // loosening. Grouped by cause:
    //
    // (a) DoubleSymMatrixProperty getter reads uninitialized memory (a dss_capi
    //     bug, DSSObjectHelper.pas:2296-2313; the port renders the stored lower
    //     triangle in BOTH lanes per r4133 — Fault.pas:695-717, DSSObject.pas:112-115
    //     — pinned by compat_quirks::sym_matrix_text_getter_renders_the_stored_matrix).
    //     The oracle returns nondeterministic garbage (denormals ~1e-310 OR huge
    //     ~1e123, process-dependent) — oracle UB, never compared (CLAUDE.md).
    //
    //     r4133 DISPOSITION (RP2.1, [`SKIP_PROPS_BOTH_CHANNELS`]) for the three
    //     Capacitor/Reactor rows: **skipped on both**, for a different upstream
    //     fact. r4133 has no typed property table and no arm for these indices —
    //     `TCapacitorObj.GetPropertyValue` (`Version8/Source/PDElements/
    //     Capacitor.pas:1084-1113`) skips `cmatrix` (index 7) and
    //     `TReactorObj.GetPropertyValue` (`Reactor.pas:1087-1103`) skips
    //     `rmatrix`/`xmatrix` (7/8) — so all three fall through to
    //     `TDSSObject.GetPropertyValue`, i.e. the `PropertyValue[]` STORE
    //     (`General/DSSObject.pas:112-115`), initialised `''`
    //     (`Capacitor.pas:760`, `Reactor.pas:1113-1114`). r4133 therefore answers
    //     the deck's parse string, or `''` where the deck never wrote one — an
    //     ECHO of the input text, never the live matrix the port renders. Not a
    //     value compare on either channel, so the row stays. It is an echo-class
    //     exclusion in `PROPS_ECHO_R4133`'s sense (plan §1.2) and RP2.3 may
    //     re-file it there; the cells are invisible to the vendored census (its
    //     walk ran with this skip active), so the row is validated by the RP4.1
    //     live run, not by the RP2.1 replay. Measured (the RP2.1 probe census,
    //     [`SKIP_PROPS_CAPI_ONLY`]): unmasked on r4133 the three would diverge on
    //     1 059 + 670 + 670 cells, every one of them Rust
    //     `'(0 |0 0 |0 0 0 )'` against r4133 `''` — the echo, exactly as read
    //     off the Pascal.
    //
    //     RP2.3 DID NOT re-file them (2026-08-23, part A finding F7): while the
    //     skip stands, an echo row here would exempt nothing on either side —
    //     see the identical disposition at the `FaultRate` rows of note (d)
    //     below for the full argument and the RP4.1-or-later schedule.
    ("Capacitor", "CMatrix"),
    ("Reactor", "RMatrix"),
    ("Reactor", "XMatrix"),
    //     r4133 DISPOSITION for the Fault row: **skipped on both**, but for a
    //     third distinct reason, and this one was found by measurement rather
    //     than by reading. r4133 does NOT share the uninitialized read — its
    //     `TFaultObj.GetPropertyValue` renders the stored lower triangle by hand
    //     (`Version8/Source/PDElements/Fault.pas:695-717`), the very code this
    //     port implements down to the trailing space and the `|` row separator
    //     (`compat_quirks::sym_matrix_text_getter_renders_the_stored_matrix`
    //     pins the exact `(2.8 |-0.6 2.8 |-0.6 -0.6 2.8 )`). So this row looked
    //     like a capi-only one — and the RP2.1 probe census says otherwise:
    //     unmasked on r4133 it diverges on **386 cells**, all of the same shape,
    //     Rust `'(0 )'` against r4133 `'()'`. The cause is upstream's
    //     `If Assigned(Gmatrix)` guard: an `r=`-specified fault leaves the
    //     pointer nil and r4133 prints the bare parentheses, where this port
    //     renders a materialised zero matrix. That is a real rendering question
    //     about an UNSET array — the family of `generator.dynout` / `autotrans.
    //     bhcurrent` (`''` vs `[]`) — not a UB mask and not something RP2.1's
    //     value-preserving rules may fold, so the row keeps both channels and
    //     the pair is handed to RP2.2's triage (plan §0 lets it open an RP3.5+
    //     sub-step). Unmasking it here would put 386 unowned cells into RP4.1's
    //     residual.
    //
    //     **RP2.2 TRIAGE (2026-08-23): the row stays exactly as it is, and no
    //     RP3.5+ sub-step opens for it.** Reading the getter settles the
    //     question: `Fault.pas:695-718` emits `'('`, fills the lower triangle
    //     only `If Assigned(Gmatrix)` (`:703`) and closes with `')'`, and an
    //     `r=`-specified fault never allocates `Gmatrix` at all. The proof of
    //     that last step is the pointer's life cycle, not the field comment
    //     (`:76-77`) the first draft of this paragraph cited: `Create` sets
    //     `Gmatrix := nil` (`:411`) and the ONLY two writers are `DoGmatrix`
    //     (`:196-209`, reached from Edit arm 6 at `:286`) and `MakeLike`
    //     (`:364-367`, which itself re-nils when the source has none). So both
    //     renders denote the SAME state, "no G matrix specified" — r4133 by
    //     printing nothing between the parens, this port by materialising the
    //     zero matrix. That is a rendering convention on an UNSET array, the
    //     family of `generator.dynout` and `autotrans.bhcurrent`, not a
    //     live-value or modelling divergence: there is no port behavior to
    //     change, hence no sub-step. It is equally not an `EnumSynonym` (the two
    //     sides are not two spellings of a value the table can name) and RP2.2
    //     must not unmask it (plan §1.1(e) staging). The 386 cells therefore
    //     stay value-skipped on both channels under this very row and RP4.1 owes
    //     no ledger entry for them.
    //
    //     **What that skip still owes, and now has (RP2.2 audit settlement,
    //     2026-08-23).** `both_channel_rows_stay_skipped_everywhere` only
    //     asserts the skip is still configured; it says nothing about what the
    //     two sides render, so the triage's factual claim was unpinned — while
    //     its sibling in the same family, `generator.dynout`, is routed to RP2.3
    //     as a *tagged* exclusion PLUS an expected-value pin. CLAUDE.md's rule
    //     is that a deliberate divergence is excluded field-by-field AND pinned
    //     by its own expected-value test, so the port's half is pinned by
    //     `fault_gmatrix_renders_a_materialised_zero_matrix_when_unset` below —
    //     the render this row masks, at 1 and at 3 phases, against the
    //     `DoGmatrix`-specified render that `compat_quirks::
    //     sym_matrix_text_getter_renders_the_stored_matrix` already pins.
    ("Fault", "GMatrix"),
    // (b) Near-zero winding-current angle: WdgCurrents renders `mag, (angle)`
    //     pairs; a ~1e-12 A (numerically-zero) winding current's angle is
    //     faer-vs-KLU noise (a cancellation floor). The magnitudes and the
    //     non-degenerate angles match; only the zero-magnitude angle diverges.
    //
    //     r4133 DISPOSITION (RP2.1, [`SKIP_PROPS_BOTH_CHANNELS`]): **skipped on
    //     both**. The fact is about the
    //     PORT's own numbers — a cancellation floor between this engine's faer
    //     solve and any KLU-solved oracle — so it is channel-independent by
    //     construction; r4133 is a second KLU-solved engine, not a reason for the
    //     angle of a 1e-12 A current to become comparable. Measured (the RP2.1
    //     probe census): unmasked on r4133 it diverges on 571 cells at max_rel
    //     1.80e+02 — the near-zero angles, plus the landed-tap differences of the
    //     RegControl-driven decks that the `autotrans.wdgcurrents` note below
    //     describes. (The census's
    //     `autotrans.wdgcurrents` bin-7 pair is a different, out-of-scope matter:
    //     a landed-tap difference on capi-only decks, `tests/corpus/props_r4133/
    //     README.md` §"RP1.2".)
    ("Transformer", "WdgCurrents"),
    // (c) The transformer ActiveWinding cursor group is NOT unconditionally
    //     skipped — see [`TRANSFORMER_CURSOR_PROPS`], which skips it only on
    //     3+-winding transformers (2-winding stays fully compared).
    //
    // (d) Reliability inputs on shunt PD elements read uninitialized in the
    //     oracle inside a metered deck (nondeterministic across processes —
    //     proven UB). Rust keeps the correct defaults (FaultRate 0.0005,
    //     pctperm 100). The same properties on Line/Transformer are clean and
    //     stay compared, so the Double-property render path is still covered.
    //
    //     r4133 DISPOSITION (RP2.1): the four rows SPLIT, and the split is
    //     measured (the probe census of [`SKIP_PROPS_CAPI_ONLY`]). Common
    //     mechanism: neither class overrides the PD tail past
    //     `normamps`/`emergamps` (`Version8/Source/PDElements/
    //     Capacitor.pas:1084-1113`, `Reactor.pas:1087-1103`), so both props fall
    //     through to the `PropertyValue[]` store, which `InitPropertyValues`
    //     freezes at Create as `Str_Real(FaultRate, 0)` / `Str_Real(PctPerm, 0)`
    //     (`Capacitor.pas:777-778`, `Reactor.pas:1135-1136`) — an echo, not the
    //     live field. What differs is what the echo SAYS:
    //      * `pctperm` — the frozen string is `'100'`, which is exactly what the
    //        port renders, so both rows measured **zero divergent cells** over
    //        the full live population: [`SKIP_PROPS_CAPI_ONLY`], they compare on
    //        r4133. The exclusion was always a statement about the capi oracle's
    //        garbage read and nothing else.
    //      * `faultrate` — `Str_Real(x, 0)` renders the 0.0005 default with ZERO
    //        decimals, so r4133 answers `'0'` against the port's `'0.0005'`:
    //        1 059 cells on Capacitor and 670 on Reactor (rel 5.00e-04, above
    //        any display floor this plan may derive). That is an `EchoDefault`
    //        of the same family as bin 7's `transformer.pctperm/repair` and
    //        `fault.pctperm` (plan §1.1) and it belongs in `PROPS_ECHO_R4133`
    //        with a citation and a pin — which RP2.3 cannot source from the
    //        vendored extracts, since this very skip kept the cells out of the
    //        census. So those two rows keep BOTH channels
    //        ([`SKIP_PROPS_BOTH_CHANNELS`]) and hand the echo case to RP2.3,
    //        rather than putting 1 729 unowned cells into RP4.1's residual.
    //
    //     RP2.3 AS EXECUTED (2026-08-23, part A finding F7): **no row landed**,
    //     and the deferral moves to RP4.1-or-later. Landing one now would be
    //     dead at both ends — the two rows stay in
    //     [`SKIP_PROPS_BOTH_CHANNELS`], so the r4133 compare never reaches the
    //     pair (no live hit, and `props_norm::assert_echo_rows_are_live` would
    //     have to exempt it) and the frozen extracts hold no example row for it,
    //     which `props_r4133_replay::every_echo_row_claims_at_least_one_example
    //     _row` reports as a table row exempting nothing. The measurement above
    //     stands and is what a later sub-step re-uses: unmasking these two rows
    //     for r4133 is an RP4.1-or-later decision that lands the unmask, the
    //     `EchoDefault` row and its pin **in one commit**, so neither half is
    //     ever dead. §1.1(e) staging is why RP2.3 did not move the mask itself.
    //     The three `Capacitor.CMatrix`/`Reactor.RMatrix`/`Reactor.XMatrix` rows
    //     above, whose (a) comment offers RP2.3 the same re-file, are deferred
    //     on the same reasoning and the same schedule.
    ("Capacitor", "FaultRate"),
    ("Capacitor", "pctperm"),
    ("Reactor", "FaultRate"),
    ("Reactor", "pctperm"),
    // (e) A 0.15.x-CHANGED DEFAULT (not UB): WP-U1.6 C5 (`8a898cba`, r4086) flips
    //     RegControl's `RevThreshold` default from +100 kW (0.14.5) to the signed
    //     −100 kW. `compare_all_properties` runs ONLY on the 0.14.5 oracle
    //     (corpus_live.rs:1276 gates it on `oracle.is_none()`), where −100 vs +100
    //     is a deliberate version mismatch (§1.2), so the value is excluded here.
    //     It is fully pinned elsewhere: the capi015 props golden
    //     (`regcontrol.json`, all scenarios incl. the −150/−50 EndEdit-fallback
    //     values) and the capi015 `regcontrol_idle.dss` live probe. New siblings
    //     `FwdThreshold`/`Idle*` are absent from the 0.14.5 capture → handled by
    //     the [`PROPS_015X`] allowlist, not here.
    //
    //     r4133 DISPOSITION (RP2.1, [`SKIP_PROPS_CAPI_ONLY`]): it **compares** on
    //     r4133. The exclusion is a statement about the 0.14.5 capture and
    //     nothing else — r4133 IS the rev the port took the signed default from
    //     (`Version8/Source/Controls/RegControl.pas`), and
    //     `tests/TOLERANCE_NOTES.md:1214-1220` pins the r4133-side values and
    //     forbids masking them there.
    //     What the r4133 channel then SEES is an echo, and the RP2.1 probe
    //     census measured it: **888 cells** of Rust `'-100'` against r4133
    //     `'100'`. r4133's `TRegControlObj.GetPropertyValue` overrides index 28
    //     (`TapNum`) and nothing else (`RegControl.pas:820-827`), so
    //     `revThreshold` (index 23) answers the `PropertyValue[]` store frozen at
    //     `'100'` by `InitPropertyValues` (`:1448`) while the live field carries
    //     the signed default — the identical shape as its sibling
    //     `remoteptratio` (`PropertyValue[27] := '60'`, `:1452`), which plan
    //     §1.1 already lists as a bin-7 echo. So this row's cells are an
    //     `EchoDefault` row for RP2.3 (with `examples_supplement.txt` rows from
    //     part C, the `regcontrol.fwdthreshold` route — the pair is absent from
    //     the vendored census for exactly this reason), never a reason to mask
    //     the only channel that can witness the r4133 default live.
    ("RegControl", "RevThreshold"),
    // (f) r4133-CHANGED DEFAULTS (WP-U2.1, delta D1): the Fuse overhaul repurposed
    //     `RatedCurrent` (default 1.0 → 0.0, informational) and moved the default
    //     `FuseCurve` `tlink` → `none`. `compare_all_properties` runs ONLY on the
    //     0.14.5 oracle (which still reports RatedCurrent 1 / FuseCurve tlink),
    //     where none-vs-tlink and 0-vs-1 are deliberate version mismatches (§1.2) —
    //     the value is excluded here (name still order-checked). Fully pinned on
    //     the r4133 side: the r4133 props golden (`fuse.json`) and the r4133
    //     controls decks (`fuse_curvemult_blow`, `fuse_legacy_noblow`). The new
    //     `CurveMultiplier`/`InterruptingRating` props (absent from the 0.14.5
    //     capture) are handled by the [`PROPS_015X`] allowlist, not here.
    //
    //     r4133 DISPOSITION (RP2.1, [`SKIP_PROPS_CAPI_ONLY`]): both rows
    //     **compare** on r4133 — same argument as (e), and
    //     `tests/TOLERANCE_NOTES.md:1214-1220` says it outright ("The r4133 values
    //     are pinned on the r4133 side …, never masked there"). r4133 is where
    //     the new defaults come from, so masking them on that channel would mask
    //     the only channel that can witness them live. Measured (the RP2.1 probe
    //     census): neither row produces a single divergent cell on r4133 —
    //     unlike (e), these two cost the unmask nothing.
    ("Fuse", "FuseCurve"),
    ("Fuse", "RatedCurrent"),
    // (g) THE FIVE `SilentReadOnly` READ-ONLY RESULTS r4133 RENDERS **LIVE**
    //     (`R4133_PROPS_PLAN.md` §RP3.8, landed 2026-09-02). Neither UB nor a
    //     changed default: the pinned 0.14.5 oracle renders `''` where BOTH
    //     r4133 and this engine render a live computed number, so every cell is
    //     a `value_structure` difference (a number vs `''`) that no value
    //     compare and no spelling rule can bridge.
    //
    //     THE CAPI MECHANISM, read off the vendored source. dss_capi flags all
    //     five `[TPropertyFlag.SilentReadOnly, TPropertyFlag.ReadByFunction]`
    //     (`.inputs/dss_capi/src/PCElements/IndMach012.pas:288-289`, read
    //     function `PowerFactorProperty` `:264-267`;
    //     `src/Controls/StorageController.pas:416-423`, read functions
    //     `:309-338`) and never assigns their `PropertyOffset`, which therefore
    //     stays `-1` — so `TDSSClassHelper.GetObjPropertyValue`
    //     (`src/General/DSSObjectHelper.pas:2189`) short-circuits at its outer
    //     guard `(PropertyOffset[Index] <> -1)` (`:2203-2204`) and leaves the
    //     `out PropStr` at `''`. Both text surfaces a capture can be read
    //     through — `? name.prop` and `Properties(p).Val` — are that one path,
    //     so the oracle answers `''` on a SOLVED circuit too, at every step
    //     (measured on six decks, RP3.8 P0 probe §4). There is no capi reading
    //     of these five that could ever match a number.
    //
    //     WHAT THE ENGINE NOW RENDERS, and why. r4133 — the authority — renders
    //     the live quantity: `Version8/Source/PCElements/IndMach012.pas:1790`
    //     (`Format('%.6g',[PowerFactor(Power[1,ActiveActor])])`, `PowerFactor` =
    //     `Common/Utilities.pas:1821`) and `Version8/Source/Controls/
    //     StorageController.pas:991-994` -> `GetkWhTotal`/`GetkWTotal`/
    //     `GetkWhActual`/`GetkWActual` (`:1162-1197`, all `Format('%-.8g',…)`).
    //     Under the 2026-08-02 policy the port follows r4133 in BOTH lanes:
    //     `PropFlags::RENDERS_LIVE_RESULT` (`obj/props/prop_flags.rs`) rides
    //     alongside `SILENT_READ_ONLY` on exactly these five `PropDef`s and the
    //     render gate (`obj/props/class_props/value.rs`) stops suppressing them.
    //     The three OTHER `SILENT_READ_ONLY` surfaces are untouched by design
    //     (JSON export omission, JSON set refusal, schema `readOnly`), so no
    //     JSON or schema golden moves. r4133's own read-writes-state is NOT
    //     reproduced: `GetkWhTotal(Var Sum)`/`GetkWTotal(Var Sum)` are handed
    //     the object's `TotalkWhCapacity`/`TotalkWCapacity` (`:991-992`), and a
    //     whole-tree grep finds nothing that ever reads those two fields, so the
    //     write-back is a dead store and the port renders the same number from a
    //     pure read (probe §1.1/§3.2; dss_capi commented the fields out at
    //     `src/Controls/StorageController.pas:158-159`).
    //
    //     FOOTPRINT, measured over the whole population in BOTH lanes (RP3.8
    //     P2a sweep, `tmp/rp38/sweep_list.md`; the two lanes' failure lists
    //     diff empty): **24 gating cases / 84 distinct (case, element, property)
    //     cells / 1 006 (cell x step) comparisons**, all on the capi channel —
    //     20 StorageController cases x 4 properties + 4 IndMach012 cases x `PF`.
    //     `StoCtrl_Current_PeakShave/master.dss` holds a StorageController on
    //     the `both` channel and is NOT in that set: it is `kind: "large"`, so
    //     `corpus_gate/scheduler.rs` never property-compares it. These rows are
    //     per `(class, property)`, so they cover it too if that ever changes.
    //
    //     r4133 DISPOSITION (RP3.8, [`SKIP_PROPS_CAPI_ONLY`]): all five
    //     **compare** on r4133. The exclusion is a statement about the 0.14.5
    //     capture and nothing else, and r4133 is the engine the render was
    //     ported from, so masking it there would mask the only channel that can
    //     witness it live — the same argument `tests/TOLERANCE_NOTES.md:1214-1220`
    //     makes for (e)'s `RevThreshold`. Measured with the §1.1(e) mask bypassed
    //     (`DSS_PROPS_CENSUS=claims`, 2026-09-02, 27 cases covering every case
    //     that holds either class): the five pairs together leave **105**
    //     divergent cells on the r4133 channel (89 in scope), of which **103**
    //     are claimed by RP2.4's display floor — the port renders
    //     `float_to_str_ex`, r4133 its `%.6g`/`%-.8g` of the same double, worst
    //     **4.03e-08** rel, four orders under
    //     `props_norm::R4133_DISPLAY_FLOOR = 2e-4`. Two of the five pairs
    //     produce **no divergent cell at all**: `kWhTotal` (integer nameplates
    //     everywhere in the population) and `IndMach012.PF`, whose value is
    //     bounded by 1 by construction, so r4133's `%.6g` is at most 5e-07
    //     ABSOLUTE from ours — inside the `i_abs` = 1e-6 the property compare
    //     uses at every tier, before any display floor is consulted. The
    //     remaining **2** cells sit on
    //     `modes:makeposseq/makeposseq_ctrl.dss`, a `capi_v0145`-only case the
    //     r4133 channel never gates (§1.3) — r4133's `TStorageObj.
    //     MakePosSequence` emits `kWrating=` for a property named `kWrated`
    //     (`Version8/Source/PCElements/Storage.pas:3979-3985` vs `:647`) and so
    //     never scales the kW rating; the port follows dss_capi's ordinal fix
    //     (`.inputs/dss_capi/src/PCElements/Storage.pas:3340,3349`). An upstream
    //     r4133 bug the port does not reproduce, newly observable only because
    //     these aggregates now render; RP4.1's to exclude and pin if that case
    //     is ever gated on r4133.
    //
    //     Each pair carries its own expected-value pin against the r4133 DLL's
    //     own bytes in `crates/dss-core/tests/props_r4133_pins.rs`
    //     (`indmach012_pf_renders_the_live_power_factor`,
    //     `storagecontroller_fleet_aggregates_render_the_live_fleet`) plus the
    //     capi-side witness `the_silent_readonly_capture_cells_are_empty`, and
    //     `props_r4133_replay::RP38_SUPERSEDED` accounts the 181 frozen census
    //     example rows the fix supersedes.
    ("IndMach012", "PF"),
    ("StorageController", "kWhTotal"),
    ("StorageController", "kWTotal"),
    ("StorageController", "kWhActual"),
    ("StorageController", "kWActual"),
];

/// The [`SKIP_PROPS`] rows whose justification is a **capi-channel fact** and
/// therefore does NOT survive onto the r4133 channel: they are excluded from the
/// value compare against the pinned 0.14.5 capture and **compared in full**
/// against r4133 (`R4133_PROPS_PLAN.md` §1.2, RP2.1).
///
/// Three causes, all spelled out at the rows themselves:
///  * the three **changed-default** rows (e)/(f) — the mismatch is 0.14.5 vs
///    r4133 by construction, and `tests/TOLERANCE_NOTES.md:1214-1220` forbids
///    masking the r4133 side;
///  * the two `pctperm` rows of (d) — the uninitialized read is the dss_capi
///    oracle's, and r4133 answers a deterministic `'100'` that MATCHES the
///    port, measured over the whole live population;
///  * the five **live read-only result** rows of (g), RP3.8 — 0.14.5 renders
///    `''` because it never assigns their `PropertyOffset`
///    (`DSSObjectHelper.pas:2203-2204`), while r4133 and this engine both
///    render the live computed number. Masking them on r4133 would mask the
///    only channel that can witness the render the sub-step ported.
///
/// **Every row here was measured, not assumed** (RP2.1 probe: a full
/// `DSS_PROPS_CENSUS=1` walk, 2026-08-23, 439 cases × 2 channels, run with all
/// twelve [`SKIP_PROPS`] rows unmasked on r4133 so the census could see what
/// each one does there). Result on the r4133 channel: `Fuse.FuseCurve`,
/// `Fuse.RatedCurrent`, `Capacitor.pctperm` and `Reactor.pctperm` produce **no
/// divergent cell at all**; `RegControl.RevThreshold` produces 888 cells of
/// `'-100'` vs `'100'` — an `EchoDefault` (see its row), which is an RP2.3
/// echo-table row and its pin, never a reason to mask the channel.
///
/// **RP3.8's five (g) rows were measured the same way** and separately
/// (`DSS_PROPS_CENSUS=claims` over the affected families, 2026-09-02, with the
/// §1.1(e) property mask bypassed): on r4133 they leave only the display-class
/// cells RP2.4's floor claims, plus the two `modes:makeposseq/
/// makeposseq_ctrl.dss` cells of an upstream r4133 `MakePosSequence` bug on a
/// `capi_v0145`-only case the r4133 channel never gates. The exact counts are
/// at the (g) row group.
///
/// Rows are matched case-insensitively and must also appear in [`SKIP_PROPS`];
/// [`skip_props_disposition_tests`] pins that this list and
/// [`SKIP_PROPS_BOTH_CHANNELS`] PARTITION `SKIP_PROPS`, so a new skip row cannot
/// land without an r4133 disposition.
const SKIP_PROPS_CAPI_ONLY: &[(&str, &str)] = &[
    ("Capacitor", "pctperm"),
    ("Reactor", "pctperm"),
    ("RegControl", "RevThreshold"),
    ("Fuse", "FuseCurve"),
    ("Fuse", "RatedCurrent"),
    // RP3.8's five (row group (g)): the 0.14.5 `''` is an artifact of
    // `PropertyOffset = -1` and exists on no other channel, so the r4133 side
    // stays fully compared — measured with the mask bypassed, the five pairs'
    // r4133 cells are the display class RP2.4's floor already claims.
    ("IndMach012", "PF"),
    ("StorageController", "kWhTotal"),
    ("StorageController", "kWTotal"),
    ("StorageController", "kWhActual"),
    ("StorageController", "kWActual"),
];

/// The [`SKIP_PROPS`] rows that stay skipped on **both** channels — the value
/// compare is not meaningful against either oracle (`R4133_PROPS_PLAN.md` §1.2,
/// RP2.1). The per-row comments in [`SKIP_PROPS`] carry the r4133-side evidence;
/// the causes are the port's own cancellation floor (b) and, for the rest, an
/// r4133 `PropertyValue[]` echo or nil-pointer render where dss_capi has a
/// garbage read — different upstream facts, same conclusion that the cell is not
/// a live-value compare on either channel.
///
/// **Measured, not assumed** — the same RP2.1 probe census
/// ([`SKIP_PROPS_CAPI_ONLY`]) shows what each of these would cost on r4133 if
/// unmasked: `Capacitor.CMatrix` 1 059 cells, `Reactor.RMatrix`/`XMatrix` 670
/// each (all three `'(0 |0 0 |0 0 0 )'` vs `''`), `Capacitor.FaultRate` 1 059
/// and `Reactor.FaultRate` 670 (`'0.0005'` vs `'0'`), `Fault.GMatrix` 386
/// (`'(0 )'` vs `'()'`), `Transformer.WdgCurrents` 571 (max_rel 1.80e+02).
///
/// These cells are invisible in the vendored census (its walk ran with the skips
/// active), so nothing here is validated by the RP2.1 replay; the echo-shaped
/// ones are candidates for `PROPS_ECHO_R4133`, and `Fault.GMatrix` — the one
/// row RP2.1 handed to RP2.2's triage — was **settled there** (2026-08-23):
/// both renders denote "no G matrix specified", so the row stays on both
/// channels, no RP3.5+ sub-step opens and RP4.1 owes it no ledger entry. The
/// evidence and the argument are at the row itself, and the port's half of the
/// claim is pinned by
/// [`skip_props_disposition_tests::fault_gmatrix_renders_a_materialised_zero_matrix_when_unset`].
const SKIP_PROPS_BOTH_CHANNELS: &[(&str, &str)] = &[
    ("Capacitor", "CMatrix"),
    ("Reactor", "RMatrix"),
    ("Reactor", "XMatrix"),
    ("Fault", "GMatrix"),
    ("Transformer", "WdgCurrents"),
    ("Capacitor", "FaultRate"),
    ("Reactor", "FaultRate"),
];

/// The one property whose **value** this gate excludes as a deliberate
/// divergence from the capture, not as a comparability problem.
///
/// `Monitor.BaseFreq` — upstream's `TMonitorObj.Create` hard-pins 60.0 over the
/// inherited `ActiveCircuit.Fundamental` (Monitor.pas:472 == r4133:552; the
/// value is what selects mode-4 flicker's lamp curve), and this engine inherits
/// like every other element. Both gating oracles report the 60.0, so the
/// exclusion applies in **both** lanes since GOLDEN_REBASE G2.2b (it was
/// default-lane-only while the parity lane still reproduced the hard pin). The
/// property *name* and its index order are still checked in both lanes, and the
/// value is pinned by its own expected-value test
/// `exec::tests::base_frequency::monitor_basefreq_inherits_the_fundamental`,
/// which asserts the inherited 50 on a 50 Hz deck, the unchanged 60 on a 60 Hz
/// one, and that an explicit `basefreq=` still overrides.
///
/// Keyed by `(class, prop)` rather than by case, so it drops the value compare
/// on every case and not just the one that needs it. That is a real if small
/// coverage loss,
/// bounded by measurement: exactly one gated deck sets a 50 Hz fundamental
/// (`electricdss-tst/Version8/Distrib/IEEETestCases/LVTestCase/Master.dss`) and
/// every mode-4 flicker deck in the corpus is 60 Hz, so no Pst output moves;
/// and what the exclusion gives up on the 60 Hz decks — that the engine still
/// reports 60 — is exactly what the pin above asserts directly.
///
/// **r4133 DISPOSITION (RP2.1): deliberately channel-BLIND.** Unlike every
/// [`SKIP_PROPS`] row, this one needs no per-channel decision: r4133 carries the
/// identical hard pin (`Monitor.pas` r4133:552 == the 0.14.5 `:472`), so both
/// gating oracles report 60.0 against the engine's inherited fundamental and the
/// exclusion is as necessary on r4133 as it is on capi. The row is therefore
/// consulted before the channel is ever looked at, in [`skip_prop`].
const LANE_SKIP_PROPS: &[(&str, &str)] = &[("Monitor", "BaseFreq")];

/// Whether the VALUE compare of `class.prop` is skipped on `channel` (the name
/// is still order-checked either way).
///
/// Two halves, and only the second is channel-scoped:
///  * [`LANE_SKIP_PROPS`] — an upstream bug BOTH gating oracles share, so the
///    exclusion is channel-blind (see that table's r4133 disposition);
///  * [`SKIP_PROPS`] — a comparability exclusion against ONE capture, so RP2.1
///    splits it by [`SKIP_PROPS_CAPI_ONLY`] / [`SKIP_PROPS_BOTH_CHANNELS`].
pub fn skip_prop(class: &str, prop: &str, channel: PropsChannel) -> bool {
    // LANE-EXCLUSION(monitor_base_frequency): both lanes inherit the circuit
    // fundamental now, so both drop the value compare on `Monitor.BaseFreq`.
    // Under the split this list was consulted in the default lane only, behind
    // a lane read on this very line.
    let lane_skipped = LANE_SKIP_PROPS
        .iter()
        .any(|(c, p)| class.eq_ignore_ascii_case(c) && prop.eq_ignore_ascii_case(p));
    lane_skipped || skip_prop_ub_on(class, prop, channel)
}

/// [`skip_prop_ub`] scoped to one channel (`R4133_PROPS_PLAN.md` §1.2, RP2.1):
/// a [`SKIP_PROPS_CAPI_ONLY`] row is dropped from the skip set on
/// [`PropsChannel::R4133`], where it compares in full. Every other row keeps the
/// channel-blind behavior.
fn skip_prop_ub_on(class: &str, prop: &str, channel: PropsChannel) -> bool {
    if !skip_prop_ub(class, prop) {
        return false;
    }
    !(channel == PropsChannel::R4133
        && SKIP_PROPS_CAPI_ONLY
            .iter()
            .any(|(c, p)| class.eq_ignore_ascii_case(c) && prop.eq_ignore_ascii_case(p)))
}

/// The r4133 disposition of every value-skip row (`R4133_PROPS_PLAN.md` §1.2,
/// RP2.1). "No row left undecided" is enforced here, not by convention.
#[cfg(test)]
mod skip_props_disposition_tests {
    use super::{
        Dss, LANE_SKIP_PROPS, PropsChannel, SKIP_PROPS, SKIP_PROPS_BOTH_CHANNELS,
        SKIP_PROPS_CAPI_ONLY, skip_prop, skip_prop_ub, skip_whole_element,
    };

    /// Every [`SKIP_PROPS`] row is dispositioned for r4133 **exactly once**: the
    /// two lists partition the table. A row added without a disposition — or
    /// listed in both, or listed but deleted from `SKIP_PROPS` — fails here, so
    /// a future skip cannot silently inherit "masked on r4133 too" (which after
    /// RP4.1 would value-mask the r4133 channel by accident, plan §1.2).
    #[test]
    fn every_skip_props_row_has_an_r4133_disposition() {
        let key = |(c, p): &(&str, &str)| (c.to_lowercase(), p.to_lowercase());
        let mut union: Vec<(String, String)> = SKIP_PROPS_CAPI_ONLY
            .iter()
            .chain(SKIP_PROPS_BOTH_CHANNELS)
            .map(key)
            .collect();
        let n_union = union.len();
        union.sort();
        union.dedup();
        assert_eq!(
            union.len(),
            n_union,
            "a row is dispositioned twice: {SKIP_PROPS_CAPI_ONLY:?} vs {SKIP_PROPS_BOTH_CHANNELS:?}"
        );
        let mut table: Vec<(String, String)> = SKIP_PROPS.iter().map(key).collect();
        table.sort();
        assert_eq!(
            union,
            table,
            "the two disposition lists must PARTITION SKIP_PROPS ({} rows today: \
             {} capi-only + {} both-channels) — a new row needs its r4133 \
             disposition and the comment that justifies it",
            SKIP_PROPS.len(),
            SKIP_PROPS_CAPI_ONLY.len(),
            SKIP_PROPS_BOTH_CHANNELS.len()
        );
    }

    /// The capi-only rows COMPARE on r4133 — the three changed defaults, whose
    /// r4133 values (`RevThreshold`, Fuse) `tests/TOLERANCE_NOTES.md:1214-1220`
    /// forbids masking there, plus the two `pctperm` rows RP2.1 measured clean.
    #[test]
    fn capi_only_rows_compare_on_r4133() {
        for (class, prop) in SKIP_PROPS_CAPI_ONLY {
            assert!(
                skip_prop(class, prop, PropsChannel::CapiV0145),
                "{class}.{prop} must still be excluded from the 0.14.5 compare"
            );
            assert!(
                !skip_prop(class, prop, PropsChannel::R4133),
                "{class}.{prop} must COMPARE on r4133 (RP2.1 disposition)"
            );
        }
        // Spelled-out spot checks, case-insensitively, so a silent table edit
        // cannot pass by emptying the list.
        assert!(!skip_prop("fuse", "fusecurve", PropsChannel::R4133));
        assert!(!skip_prop("Fuse", "RatedCurrent", PropsChannel::R4133));
        assert!(!skip_prop(
            "RegControl",
            "RevThreshold",
            PropsChannel::R4133
        ));
        assert!(!skip_prop("Capacitor", "PCTPERM", PropsChannel::R4133));
        assert!(skip_prop("Capacitor", "pctperm", PropsChannel::CapiV0145));
        // RP3.8's five live-render rows, spelled out for the same reason: the
        // 0.14.5 `''` is the ONLY thing they mask, so the r4133 channel — the
        // one that shares the render — must keep comparing every one of them.
        for (class, prop) in [
            ("indmach012", "pf"),
            ("StorageController", "KWHTOTAL"),
            ("storagecontroller", "kwtotal"),
            ("StorageController", "kWhActual"),
            ("storagecontroller", "kWACTUAL"),
        ] {
            assert!(
                skip_prop(class, prop, PropsChannel::CapiV0145),
                "{class}.{prop} must be excluded from the 0.14.5 compare (it renders '')"
            );
            assert!(
                !skip_prop(class, prop, PropsChannel::R4133),
                "{class}.{prop} must COMPARE on r4133 (RP3.8 disposition)"
            );
        }
    }

    /// The both-channel rows stay skipped on r4133 as well — the port's own
    /// cancellation floor, the r4133 `PropertyValue[]` echoes where dss_capi has
    /// a garbage read, and `Fault.GMatrix`'s unset-array render (citations and
    /// the measured cell counts at the rows).
    #[test]
    fn both_channel_rows_stay_skipped_everywhere() {
        for (class, prop) in SKIP_PROPS_BOTH_CHANNELS {
            for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
                assert!(
                    skip_prop(class, prop, ch),
                    "{class}.{prop} must stay value-skipped on {}",
                    ch.tag()
                );
            }
        }
        assert!(skip_prop("Capacitor", "CMatrix", PropsChannel::R4133));
        assert!(skip_prop("Capacitor", "FaultRate", PropsChannel::R4133));
        assert!(skip_prop("Fault", "GMatrix", PropsChannel::R4133));
        assert!(skip_prop("Transformer", "WdgCurrents", PropsChannel::R4133));
    }

    /// **The expected-value pin the `Fault.GMatrix` both-channel skip owes**
    /// (RP2.2 audit settlement, 2026-08-23; the row's comment carries the
    /// argument).
    ///
    /// The skip drops 386 cells from the value compare on BOTH channels on the
    /// strength of one factual claim: that an `r=`-specified fault renders a
    /// **materialised zero** lower triangle here, where r4133 prints the bare
    /// parentheses of its `If Assigned(Gmatrix)` guard
    /// (`Version8/Source/PDElements/Fault.pas:703`; the pointer is `nil` from
    /// `Create`, `:411`, until `DoGmatrix`, `:196-209`). Nothing else asserts
    /// that render — `both_channel_rows_stay_skipped_everywhere` above only
    /// asserts the skip is configured — so a regression in the port's readback
    /// would be invisible on the very channels this row silences. CLAUDE.md's
    /// rule for a deliberate divergence is exclusion **plus** an expected-value
    /// test; this is that test.
    ///
    /// It pins both halves of the shape, because the divergence is exactly
    /// about the unset one: order comes from `phases`, so the zero triangle
    /// grows with it, and a `gmatrix=`-specified fault still renders the stored
    /// numbers (the specified case is pinned in full by `dss_core::exec::tests::
    /// compat_quirks::sym_matrix_text_getter_renders_the_stored_matrix`).
    #[test]
    fn fault_gmatrix_renders_a_materialised_zero_matrix_when_unset() {
        let query = |cmds: &[&str]| -> String {
            let mut dss = Dss::new();
            dss.command("clear");
            dss.command("new circuit.gmatrixprobe");
            for c in cmds {
                dss.command(c);
            }
            assert!(dss.errors().is_empty(), "{:?}", dss.errors());
            dss.command("? Fault.f1.GMatrix");
            dss.result().to_string()
        };

        // 1 phase, `r=` specified: `Gmatrix` never allocated upstream.
        assert_eq!(
            query(&["new Fault.f1 bus1=sourcebus.1 phases=1 r=1.0"]),
            "(0 )",
            "the unset GMatrix renders a materialised zero triangle — the render \
             the both-channel skip masks against r4133's bare '()'"
        );
        // …and it grows with `phases`, so the divergence is the whole triangle,
        // not one stray token.
        assert_eq!(
            query(&["new Fault.f1 bus1=sourcebus phases=3 r=1.0"]),
            "(0 |0 0 |0 0 0 )"
        );
        // The specified case is NOT zeros — so the pin above really discriminates
        // "unset" from "the getter is broken".
        assert_eq!(
            query(&["new Fault.f1 bus1=sourcebus phases=2 gmatrix=(1.5 | -0.5 2.5)"]),
            "(1.5 |-0.5 2.5 )"
        );
    }

    /// [`LANE_SKIP_PROPS`] is channel-BLIND by decision, not by omission: r4133
    /// hard-pins `Monitor.BaseFreq` to 60.0 exactly as 0.14.5 does
    /// (`Monitor.pas` r4133:552), so both channels drop the value.
    #[test]
    fn the_monitor_basefreq_exclusion_is_channel_blind() {
        assert_eq!(LANE_SKIP_PROPS, &[("Monitor", "BaseFreq")]);
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            assert!(skip_prop("Monitor", "BaseFreq", ch));
        }
        // …and it is NOT in the UB half, whose consumer is the dump artifact.
        assert!(!skip_prop_ub("Monitor", "BaseFreq"));
    }

    /// The dump artifact's predicate keeps the WHOLE table, channel-blind — its
    /// stated rationale is that the stripped set is order-dependent by
    /// construction, which a channel scope would quietly change.
    #[test]
    fn skip_prop_ub_stays_channel_blind() {
        for (class, prop) in SKIP_PROPS {
            assert!(
                skip_prop_ub(class, prop),
                "{class}.{prop} must stay in the channel-blind UB set"
            );
        }
    }

    /// Recloser/Relay are dropped WHOLE on capi (their tables are r4133's and
    /// cannot match a 0.14.5 capture's shape) and compare in full on r4133.
    #[test]
    fn recloser_and_relay_are_whole_element_skipped_on_capi_only() {
        for class in ["Recloser", "Relay", "relay", "RECLOSER"] {
            assert!(
                skip_whole_element(class, PropsChannel::CapiV0145),
                "{class} must stay whole-element skipped against the 0.14.5 capture"
            );
            assert!(
                !skip_whole_element(class, PropsChannel::R4133),
                "{class} must compare in FULL on r4133 (plan §1.2) — otherwise the \
                 census's 22 relay/recloser pairs are dead on the live path"
            );
        }
        for class in ["Fuse", "Capacitor", ""] {
            for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
                assert!(!skip_whole_element(class, ch));
            }
        }
    }
}

/// The capi-invariance contract of the RP2.1 seam ([`PropsPolicy`]).
#[cfg(test)]
mod props_policy_tests {
    use super::{PropsChannel, PropsPolicy, ValueVerdict, props_norm, value_verdict};

    /// Spellings the r4133 rules DO fold — `BoolFold` (bin 1), `CaseFold`
    /// (bin 2, case-only), `CaseFold`'s trim half (bin 2, the two upstream
    /// trailing blanks), `ArrayForm` (bin 4) and, since **RP2.2**,
    /// `EnumSynonym` (bin 3's source sequence selectors, and the live
    /// `voltage_curvex_ref` getter whose pair is bin-2-labelled). Every rule
    /// kind the table ships is represented, so the two channel tests below
    /// cover the whole mechanism rather than part of it. Each entry is a real
    /// census spelling from `tests/corpus/props_r4133/examples_full.txt`.
    ///
    /// Part A drafted a `("Load", "ZIPV", …)` row here for the array bin; part
    /// B corrected it to `EnergyMeter.Option` — `load.zipv` is a **bin-5** pair
    /// (`bins.tsv`), so it takes no `ArrayForm` row and would have made the
    /// positive test below assert a fold that must not happen.
    const FOLDABLE: &[(&str, &str, &str, &str)] = &[
        ("Recloser", "EventLog", "Yes", "true"),
        ("Generator", "Status", "Variable", "variable"),
        ("Transformer", "Conn", "wye", "wye "),
        ("EnergyMeter", "Option", "[E, R, C]", "(E, R, C)"),
        ("Vsource", "ScanType", "Positive", "Pos"),
        ("InvControl", "voltage_curvex_ref", "RAvg", "avgrated"),
    ];

    /// **The capi channel never normalizes** — plan mechanic (b),
    /// capi-invariance, as a test rather than a promise. Every pair below is a
    /// spelling part B folds on r4133; on the capi channel the seam must hand
    /// [`assert_value_matches_tol`] the two raw strings, so the 0.14.5 property
    /// compare stays byte/skeleton-exact.
    ///
    /// [`assert_value_matches_tol`]: super::assert_value_matches_tol
    #[test]
    fn the_capi_channel_never_normalizes() {
        let capi = PropsPolicy::for_channel(PropsChannel::CapiV0145);
        for (class, prop, rust, oracle) in FOLDABLE {
            let (a, e) = capi.normalize(class, prop, rust, oracle);
            assert_eq!(
                (a.as_ref(), e.as_ref()),
                (*rust, *oracle),
                "{class}.{prop}: the capi channel must compare the RAW strings"
            );
        }
    }

    /// The other half of the same contract, which makes the one above mean
    /// something: on the **r4133** channel the armed policy DOES fold every
    /// spelling in [`FOLDABLE`], to our side's spelling. Without this the capi
    /// test would pass just as well against a seam that never normalizes
    /// anything at all.
    #[test]
    fn the_r4133_channel_folds_the_documented_spellings() {
        let r4133 = PropsPolicy::for_channel(PropsChannel::R4133);
        for (class, prop, rust, oracle) in FOLDABLE {
            let (a, e) = r4133.normalize(class, prop, rust, oracle);
            assert_eq!(
                (a.as_ref(), e.as_ref()),
                (*rust, *rust),
                "{class}.{prop}: r4133 must fold {oracle:?} onto our spelling"
            );
        }
    }

    /// The census knob's PLAIN mode is the un-normalized baseline forever
    /// (RP0.2), on **either** channel: a rule landing in part B must not shrink
    /// what a plain re-census reports.
    #[test]
    fn the_plain_policy_never_normalizes_on_either_channel() {
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let plain = PropsPolicy::plain(ch);
            assert_eq!(plain.channel(), ch, "the plain policy keeps its channel");
            for (class, prop, rust, oracle) in FOLDABLE {
                let (a, e) = plain.normalize(class, prop, rust, oracle);
                assert_eq!(
                    (a.as_ref(), e.as_ref()),
                    (*rust, *oracle),
                    "{class}.{prop}: plain census mode must compare the RAW strings on {}",
                    ch.tag()
                );
            }
            // …and it reaches RP2.3's exclusion seam no more than it reaches
            // the normalization one: a plain re-census must report exactly the
            // population RP0.1 measured, echo rows or not.
            for (class, prop, rust, oracle) in EXCLUDED {
                assert!(
                    !plain.echo_excluded(class, prop, rust, oracle),
                    "{class}.{prop}: plain census mode must not exclude on {}",
                    ch.tag()
                );
            }
        }
    }

    /// Cells a shipped [`PROPS_ECHO_R4133`] row excludes on r4133 — one per
    /// category, each a real census spelling
    /// (`tests/corpus/props_r4133/examples_full.txt` /
    /// `examples_supplement.txt`), so the two channel tests below cover the
    /// whole mechanism rather than one row of it.
    ///
    /// [`PROPS_ECHO_R4133`]: super::props_norm::PROPS_ECHO_R4133
    const EXCLUDED: &[(&str, &str, &str, &str)] = &[
        ("RegControl", "Idle", "No", ""),       // EchoDefault
        ("Relay", "Reset", "No", "0.20"),       // EchoParse
        ("Transformer", "BHCurrent", "", "[]"), // EmptyCollectionRender
        ("Generator", "D", "1", "0"),           // LiveSemanticsDiffer
    ];

    /// **The capi channel never excludes** — plan mechanic (b) applied to
    /// RP2.3's link. Every cell below is one the r4133 arm masks; on capi the
    /// comparator must still assert it, because the capi compare IS the witness
    /// most of those rows cite.
    #[test]
    fn the_capi_channel_never_excludes() {
        let capi = PropsPolicy::for_channel(PropsChannel::CapiV0145);
        for (class, prop, rust, oracle) in EXCLUDED {
            assert!(
                !capi.echo_excluded(class, prop, rust, oracle),
                "{class}.{prop}: the capi channel must compare the value"
            );
        }
    }

    /// The other half: on **r4133** the armed policy really does exclude every
    /// cell in [`EXCLUDED`] — without this the capi test would pass just as
    /// well against a seam that excludes nothing at all — and it leaves a pair
    /// with no row alone.
    #[test]
    fn the_r4133_channel_excludes_the_cited_echo_pairs() {
        let r4133 = PropsPolicy::for_channel(PropsChannel::R4133);
        for (class, prop, rust, oracle) in EXCLUDED {
            assert!(
                r4133.echo_excluded(class, prop, rust, oracle),
                "{class}.{prop}: a cited echo row must drop the value compare"
            );
        }
        assert!(
            !r4133.echo_excluded("RegControl", "Band", "2", "3"),
            "a pair with no echo row must still compare"
        );
        assert!(
            !r4133.echo_excluded("IndMach012", "PF", "", "0.908391"),
            "the SilentReadOnly family takes no row (RP2.3's kill ruling)"
        );
    }

    /// **Cells the RP2.4 display floor claims on r4133** — every one a real
    /// census spelling (`tests/corpus/props_r4133/examples_full.txt`), chosen to
    /// cover each `%[-].Ng` formatter and each *render shape* the floor has to
    /// handle, so the channel tests below cover the mechanism and not one cell
    /// of it. The measured gap is stated per row; the derivation and the Pascal
    /// site table live on `props_norm::R4133_DISPLAY_FLOOR`.
    const UNDER_FLOOR: &[(&str, &str, &str, &str)] = &[
        // THE WORST cell the floor claims, 6.431124e-05 — `%-.4g`,
        // `PCElements/Load.pas:2345`. If the floor ever stops claiming this
        // one, the derivation is wrong, not the row.
        ("Load", "pf", "0.747651914485831", "0.7477"),
        // The `%-.5g` family: `PCElements/Vsource.pas:1326-1341` (a plain
        // double and an exponent render), `PDElements/Transformer.pas:1842`.
        ("Vsource", "R1", "0.2138355760737", "0.21384"),
        ("Vsource", "Isc3", "3346958.0822587", "3.347E006"),
        ("Transformer", "NormAmps", "381.944444444444", "381.94"),
        // `%-.6g` inside a bracketed vector — `Common/Utilities.pas:2600-2607`
        // `GetDSSArray_Real`. A scalar-only floor would leave this unclaimed.
        ("Capacitor", "cuf", "[ 287.82360946885]", "[ 287.824]"),
        // …and inside a `|`-separated MATRIX render (three rows, six numbers).
        (
            "Line",
            "CMatrix",
            "[72.7194481250695 |0 72.7194481250695 |0 0 72.7194481250695 ]",
            "[72.71945 |0 72.71945 |0 0 72.71945 ]",
        ),
        // RP2.3's hand-off: the one cell `props_norm::ECHO_CARVE_OUTS` takes
        // back out of `reactor.kvar`'s echo row, because r4133's own
        // `MakePosSequence` round-trips the live value through
        // `Format(' kvar=%-.5g')` (`PDElements/Reactor.pas:1145-1201`). It is
        // this link's, and this is where that is asserted at the seam.
        ("Reactor", "kvar", "66.6666666666667", "66.667"),
    ];

    /// **The capi channel never applies the display floor** — plan mechanic (b)
    /// applied to RP2.4's link, and the arm where a leak would cost the most:
    /// the capi property compare runs at the case's tier floors, orders under
    /// this floor on every kind but two
    /// ([`the_capi_property_compare_runs_at_the_case_tier_floors`]), so a floor
    /// reaching it would relax every numeric property of every
    /// `engines: "both"` case to 2e-4 at once. Every cell below is one the r4133
    /// arm claims.
    #[test]
    fn the_capi_channel_never_applies_the_display_floor() {
        let capi = PropsPolicy::for_channel(PropsChannel::CapiV0145);
        for (class, prop, rust, oracle) in UNDER_FLOOR {
            assert!(
                !capi.under_display_floor(rust, oracle),
                "{class}.{prop}: the capi channel must compare {rust:?} against {oracle:?} exactly"
            );
        }
        // …and the plain census policy reaches it on NEITHER channel, so a
        // plain re-census still reports the RP0.1 population (RP0.2's baseline).
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let plain = PropsPolicy::plain(ch);
            for (class, prop, rust, oracle) in UNDER_FLOOR {
                assert!(
                    !plain.under_display_floor(rust, oracle),
                    "{class}.{prop}: plain census mode must not apply the floor on {}",
                    ch.tag()
                );
            }
        }
    }

    /// **What the capi channel's property compare actually costs** — the bound
    /// the display floor's justification rests on, measured instead of asserted
    /// in prose (RP2.4 audit round, which caught "the capi property compare is
    /// exact / at zero tolerance" in five places; it is not — `compare_prop_lists`
    /// gets `tol.i_rel`/`tol.i_abs` from [`compare_all_properties`], and
    /// `value_verdict` passes a number when `|a−e| <= abs + rel*|e|`).
    ///
    /// So the honest statement is a *tier* statement, and this test is what
    /// keeps it true: on the kinds that carry the corpus the capi floors are
    /// orders under the 2e-4 display floor, and the two loosest kinds are named
    /// with the magnitude below which they stop bounding it.
    ///
    /// [`compare_all_properties`]: super::compare_all_properties
    #[test]
    fn the_capi_property_compare_runs_at_the_case_tier_floors() {
        let floor = props_norm::display_floor().expect("RP2.4 derived the floor");
        // The tiers the property compare runs at, and how they stand against
        // the r4133-only display floor.
        for (kind, i_rel, i_abs) in [
            ("micro", 1e-9, 1e-6),
            ("feeder", 1e-7, 1e-5),
            ("micro_wtg3_dynamics", 2e-5, 1e-4),
            // `midi` has no arm of its own and takes the fallback — 10 corpus
            // cases (asymmetric/controls/modes manifests) run here.
            ("midi", 1e-6, 1e-4),
        ] {
            let tol = super::tol_for(kind);
            assert_eq!((tol.i_rel, tol.i_abs), (i_rel, i_abs), "{kind}");
            assert!(
                tol.i_rel < floor,
                "{kind}: a capi tier at or above the display floor would stop bounding it"
            );
        }
        // …and the part the bound does NOT cover, stated as a number rather
        // than left implicit: with `i_abs` at 1e-4 a value under
        // `i_abs / floor` can move by the whole display floor and still pass on
        // capi, so on those two kinds the `both`-case cross-check is not the
        // backstop. 0.5 for `midi`/`micro_wtg3_dynamics`, 0.05 for `feeder`.
        for (kind, magnitude) in [
            ("midi", 0.5),
            ("micro_wtg3_dynamics", 0.5),
            ("feeder", 0.05),
        ] {
            let tol = super::tol_for(kind);
            assert!(
                (tol.i_abs / floor - magnitude).abs() < 1e-12,
                "{kind}: i_abs {} / floor {floor} is not {magnitude}",
                tol.i_abs
            );
            assert_eq!(
                value_verdict(
                    &format!("{}", magnitude * (1.0 + floor)),
                    &format!("{magnitude}"),
                    tol.i_rel,
                    tol.i_abs
                ),
                ValueVerdict::Match,
                "{kind}: a value this small moving by the whole display floor is inside the \
                 capi tier — the documented hole in bound (a)"
            );
        }
    }

    /// The other half, which makes the capi test mean something: on **r4133**
    /// the armed policy really claims every cell of [`UNDER_FLOOR`] — and
    /// refuses, at the same seam, everything the floor must never swallow.
    ///
    /// The refusals are the load-bearing half. A floor is a tolerance, so what
    /// keeps it a *classification* is the list of shapes it declines: a gap
    /// above it at any magnitude, a 0-vs-nonzero pair, a differing non-numeric
    /// skeleton (an enum, a boolean, an empty render), and — since the RP2.4
    /// audit settlement — **any gap, however small, that is not our value
    /// rounded to the digits r4133 printed**. That last clause is what covers
    /// an integer-spelled value differing by one (`'4'` vs `'3'`, but also
    /// `'5001'` vs `'5000'` at 2.0e-4, which the metric alone would fold) and
    /// the 55 round-trip-residue spellings RP3.9 owns.
    #[test]
    fn the_r4133_channel_claims_the_measured_display_cells() {
        let r4133 = PropsPolicy::for_channel(PropsChannel::R4133);
        for (class, prop, rust, oracle) in UNDER_FLOOR {
            assert!(
                r4133.under_display_floor(rust, oracle),
                "{class}.{prop}: the floor must claim {rust:?} vs {oracle:?}"
            );
        }
        // The boundary, from both sides, in the shape the mechanism produces —
        // OUR long value against r4133's shorter render of it, so only the
        // floor value decides: 1.8996e-4 in, 2.0996e-4 out.
        assert!(r4133.under_display_floor("1000.19", "1000"));
        assert!(!r4133.under_display_floor("1000.21", "1000"));
        // A gap above the floor, at three magnitudes.
        assert!(!r4133.under_display_floor("0.747651914485831", "0.7484477"));
        assert!(!r4133.under_display_floor("3346958.0822587", "3.35E006"));
        assert!(!r4133.under_display_floor("[ 287.82360946885]", "[ 287.9]"));
        // 0 vs non-zero is rel 1 at any magnitude — bin 7's frozen-default
        // echoes (`invcontrol.lpftau`) can never be mistaken for a render.
        assert!(!r4133.under_display_floor("0.001", "0.0"));
        assert!(!r4133.under_display_floor("0", "-1"));
        // Not one numeric shape: a boolean, an enum, an empty render, a
        // different token count, an RPN source text.
        assert!(!r4133.under_display_floor("Yes", "true"));
        assert!(!r4133.under_display_floor("Positive", "Pos"));
        assert!(!r4133.under_display_floor("", "[]"));
        assert!(!r4133.under_display_floor("[ 400]", "[400, 400, 400]"));
        assert!(!r4133.under_display_floor("17", "1 16 +"));
        // A discrete value spelled as a number is still a value difference —
        // and so is a one-unit difference at a magnitude where the metric alone
        // would have folded it (`5001` vs `5000` is 2.0e-4, inside).
        assert!(!r4133.under_display_floor("4", "3"));
        assert!(!r4133.under_display_floor("5001", "5000"));
        // The mechanism clause on real census spellings the metric would take:
        // `load.kva` 1.2e-6 apart and `vsource.puz1` 1.6e-5 apart, both refused
        // because no `%.Ng` render of our value produces r4133's number (RP3.9,
        // `props_r4133_replay::RP39_ROUTING`).
        assert!(!r4133.under_display_floor("105.263157894737", "105.26302971129"));
        assert!(!r4133.under_display_floor(
            "[5.78774785226323, 17.3632435567897]",
            "[5.787655, 17.362965]"
        ));
    }

    /// **Non-vacuity through the REAL comparator** (plan RP2.4 acceptance).
    /// The two tests above drive the seam; this one drives
    /// [`compare_prop_lists`] itself on a property list carrying a
    /// floor-claimed cell next to an ordinary one, and pins the corners that
    /// make the floor a **per-cell predicate and not a pair mask** — the RP2.3
    /// audit settlement's lesson, applied at design time:
    ///
    /// * r4133 + the display cell → **passes** (that is the floor);
    /// * r4133 + the SAME property, 4.7e-4 apart → **fails** (nothing is masked
    ///   by name; a bigger divergence on a floor-claimed pair still aborts).
    ///   The oracle spelling is deliberately still a render of ours, so this
    ///   corner is the METRIC clause alone;
    /// * r4133 + the same property 1.1e-5 apart but no `%.Ng` render of our
    ///   value → **fails** — the MECHANISM clause alone, the corner the RP2.4
    ///   audit settlement added;
    /// * r4133 + the same property rendered non-numerically → **fails raw**;
    /// * r4133 + a neighbour property above the floor → **fails** (the floor is
    ///   not element-scoped either);
    /// * capi + the display cell → **fails** (channel-scoped);
    /// * the property NAME walk is untouched on r4133;
    /// * and the seam's live counters move for the r4133 run, which is what
    ///   makes `props_norm::display_floor_counters` "what the gate saw".
    ///
    /// `Load.pf` and `Load.kW` deliberately have **no** `PROPS_NORM_R4133` and
    /// no `PROPS_ECHO_R4133` row (`bins.tsv`: both are numeric bin-6 pairs), so
    /// driving the real comparator here moves no row counter and cannot make
    /// `assert_norm_rows_are_live` order-dependent — the trap RP2.1 part D
    /// measured. The floor's own counters have no per-row staleness to rot.
    ///
    /// [`compare_prop_lists`]: super::compare_prop_lists
    #[test]
    fn the_display_floor_drops_only_the_cell_it_claims_only_on_r4133() {
        let run = |channel, actual: &[(&str, &str)], oracle: &[(&str, &str)]| {
            run_props(channel, "Load.floor1", "Load", actual, oracle)
        };
        let rust = [("pf", "0.747651914485831"), ("kW", "1000.21")];
        // 6.431124e-05 apart — the worst cell the floor claims.
        let display = [("pf", "0.7477"), ("kW", "1000.21")];
        // 4.65e-04 apart on the very same property, and still a `%-.3g` render
        // of ours: 2.3x the floor, refused by the metric alone.
        let too_far = [("pf", "0.748"), ("kW", "1000.21")];
        // 1.08e-05 apart — deep INSIDE the floor, and refused all the same
        // because `0.74766` is not our value rounded to five digits (that would
        // be `0.74765`). The mechanism clause, at the real comparator.
        let not_a_render = [("pf", "0.74766"), ("kW", "1000.21")];
        // The same property, not a number on the oracle side.
        let non_numeric = [("pf", ""), ("kW", "1000.21")];
        // A neighbour property, 2.1e-4 apart: just over the floor.
        let neighbour = [("pf", "0.7477"), ("kW", "1000")];
        let renamed = [("pf", "0.7477"), ("kilowatts", "1000.21")];
        let (visits, hits) = props_norm::display_floor_counters();
        assert!(
            run(PropsChannel::R4133, &rust, &display),
            "the floor must drop the value compare of a display cell on r4133"
        );
        let (visits_after, hits_after) = props_norm::display_floor_counters();
        assert!(
            visits_after > visits && hits_after > hits,
            "the comparator must reach the COUNTING seam (props_norm::under_display_floor_r4133), \
             not the offline twin"
        );
        assert!(
            !run(PropsChannel::R4133, &rust, &too_far),
            "a 4.7e-4 error on a floor-CLAIMED property must still fail on r4133 — the floor is \
             a cell predicate, not a mask on `load.pf`"
        );
        assert!(
            !run(PropsChannel::R4133, &rust, &not_a_render),
            "a gap inside the floor that no `%.Ng` render of our value explains must still fail"
        );
        assert!(
            !run(PropsChannel::R4133, &rust, &non_numeric),
            "a non-numeric cell on a floor-claimed property must still fail raw"
        );
        assert!(
            !run(PropsChannel::R4133, &rust, &neighbour),
            "a neighbour property just over the floor must still fail on r4133"
        );
        assert!(
            !run(PropsChannel::R4133, &rust, &renamed),
            "the property NAME walk is untouched by the floor"
        );
        assert!(
            !run(PropsChannel::CapiV0145, &rust, &display),
            "the capi channel must still fail on the very cell the r4133 floor claims"
        );
    }

    /// **Non-vacuity through the REAL comparator.** The two tests above drive
    /// the seam; this one drives [`compare_prop_lists`] itself, on an element
    /// whose property list holds an echo-excluded prop next to an ordinary one,
    /// and pins all four corners:
    ///
    /// * r4133 + the excluded prop divergent → **passes** (that is the row);
    /// * r4133 + a NON-excluded prop of the same element divergent → **fails**
    ///   (the row is field-scoped, not element-scoped — the mask cannot hide a
    ///   neighbour);
    /// * capi + either → **fails** (the exclusion is channel-scoped);
    /// * and the property NAME is still walked on r4133, so a renamed or
    ///   reordered property fails there too, exactly as `SKIP_PROPS` behaves.
    ///
    /// **Both props are deliberately pairs `PROPS_NORM_R4133` has no row for**
    /// (`regcontrol.idleforward` is one of the five pure-echo bin-1 pairs;
    /// `regcontrol.band` takes no row at all). This test drives the REAL
    /// comparator, so a prop with a normalization row would move that row's
    /// process-global visit counter without ever folding anything, and
    /// `assert_norm_rows_are_live()` — which runs once at the end of the gate,
    /// in this same binary — would then report the row as stale. That is the
    /// test-ordering trap RP2.1 part D removed from three tests in
    /// `props_norm::tests`; measured here on the first full-workspace run.
    /// The ECHO counters this test does move all end with `hits > 0`, which is
    /// what their own guard asks.
    ///
    /// [`compare_prop_lists`]: super::compare_prop_lists
    #[test]
    fn an_echo_row_drops_only_its_own_value_only_on_r4133() {
        use super::{PROPS_015X, compare_prop_lists};
        let props = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect()
        };
        let run = |channel, actual: &[(&str, &str)], oracle: &[(&str, &str)]| {
            let (a, o) = (props(actual), props(oracle));
            std::panic::catch_unwind(move || {
                compare_prop_lists(
                    "RegControl.r1",
                    "RegControl",
                    &a,
                    &o,
                    PROPS_015X,
                    PropsPolicy::for_channel(channel),
                    false,
                    1e-9,
                    1e-9,
                    "self",
                )
            })
            .is_ok()
        };
        // r4133-shaped lists (its own table carries `IdleForward`, so
        // `PROPS_015X` keeps it and the walk compares it).
        let echo_cell = [("IdleForward", "No"), ("Band", "2")];
        let echo_oracle = [("IdleForward", ""), ("Band", "2")];
        let both_bad = [("IdleForward", ""), ("Band", "3")];
        let renamed = [("IdleForward", ""), ("BandWidth", "2")];
        assert!(
            run(PropsChannel::R4133, &echo_cell, &echo_oracle),
            "the echo row must drop `IdleForward`'s value compare on r4133"
        );
        assert!(
            !run(PropsChannel::R4133, &echo_cell, &both_bad),
            "a NON-echo property of the same element must still fail on r4133"
        );
        assert!(
            !run(PropsChannel::R4133, &echo_cell, &renamed),
            "the property NAME walk is untouched by an echo row"
        );
        assert!(
            !run(PropsChannel::CapiV0145, &echo_cell, &echo_oracle),
            "the capi channel must still fail on the very cell r4133 excludes"
        );
    }

    /// Drive [`compare_prop_lists`] on a hand-built property list of any class,
    /// the way [`an_echo_row_drops_only_its_own_value_only_on_r4133`] drives a
    /// `RegControl` one.
    ///
    /// `load.yearly` and `reactor.bus2` are two of the 20 MIXED pairs — each
    /// holds a `CaseFold` normalization row AND an echo row — which is what the
    /// two tests below need. The neighbour prop each list carries (`kW`, `kV`)
    /// deliberately has NO row of either kind, so driving it moves no
    /// process-global counter: the trap RP2.1 part D measured, re-measured here
    /// (the first draft used `Daily`, which DOES have a `CaseFold` row, and it
    /// reddened `assert_norm_rows_are_live` in the corpus-gate binary).
    ///
    /// The two tests use **different** pairs on purpose, and each leaves every
    /// row it touches with `hits > 0`: `cargo test` runs them concurrently with
    /// each other and with the gate's own `assert_*_rows_are_live()` epilogue, so
    /// a shared pair would make both the exact-delta assertions and the liveness
    /// guard order-dependent.
    ///
    /// [`compare_prop_lists`]: super::compare_prop_lists
    fn run_props(
        channel: PropsChannel,
        element: &str,
        class: &str,
        actual: &[(&str, &str)],
        oracle: &[(&str, &str)],
    ) -> bool {
        use super::{PROPS_015X, compare_prop_lists};
        let props = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect()
        };
        let (a, o) = (props(actual), props(oracle));
        let (element, class) = (element.to_string(), class.to_string());
        std::panic::catch_unwind(move || {
            compare_prop_lists(
                &element,
                &class,
                &a,
                &o,
                PROPS_015X,
                PropsPolicy::for_channel(channel),
                false,
                1e-9,
                1e-9,
                "self",
            )
        })
        .is_ok()
    }

    /// Shorthand for the `load.yearly` list.
    fn run_load(channel: PropsChannel, actual: &[(&str, &str)], oracle: &[(&str, &str)]) -> bool {
        run_props(channel, "Load.l1", "Load", actual, oracle)
    }

    /// **The normalization seam really runs before the exclusion** — the order
    /// `PropsPolicy::echo_excluded`'s doc calls the mechanism, asserted at the
    /// SHIPPED seam instead of only in the two offline copies of the chain
    /// (`props_norm::claim_value`, `props_r4133_replay::Link::ORDER`).
    ///
    /// The observable is the mixed pair's own accounting: swapping the two lines
    /// in `compare_prop_lists` leaves `reactor.bus2`'s normalization counters
    /// still, which after RP4.1 silently disarms `assert_norm_rows_are_live` for
    /// all 20 mixed rows (it is silent when `visits == 0`). Before the RP2.3
    /// audit settlement that swap left the entire suite green — the order was
    /// pinned in the two offline copies of the chain and nowhere at the seam.
    ///
    /// **What RP4.1 P1's narrowing changed here.** `reactor.bus2` is one of the
    /// 20 narrowed rows, so the echo seam now counts only the cells the row
    /// covers: the foldable cell arrives at the exclusion already folded
    /// (`b2.0` on both sides), which is not one of the row's measured spellings,
    /// so it is no longer an echo visit. The swap is still caught, by the other
    /// half of the same accounting — under echo-first the row's own measured
    /// cell would be excluded before the typed rule ever saw it, and the
    /// normalization counters below would move by one instead of two.
    ///
    /// The measured spelling is `rust 'b2.0'` vs `r4133 'b2.0.0.0'`
    /// (`tests/corpus/props_r4133/examples_full.txt:545`) — r4133 answers the
    /// padded `GetBus(2)` snapshot, the port the live terminal — and this test
    /// drives it in that orientation since the narrowing made the orientation
    /// load-bearing.
    ///
    /// Counter hygiene: this test owns `reactor.bus2` (no other test drives it)
    /// and leaves both of its rows with `hits > 0` — the foldable cell for the
    /// norm row, the echo cell for the echo row.
    #[test]
    fn the_normalization_seam_runs_before_the_exclusion_on_a_mixed_pair() {
        let run = |actual: &[(&str, &str)], oracle: &[(&str, &str)]| {
            run_props(PropsChannel::R4133, "Reactor.r1", "Reactor", actual, oracle)
        };
        let norm_before =
            super::props_norm::norm_counters("reactor", "bus2").expect("a CaseFold row");
        let echo_before = super::props_norm::echo_counters("reactor", "bus2").expect("an echo row");
        // A foldable cell: the same terminal spelling in a different case. The
        // typed rule must SEE it (visit) and CLAIM it (hit) before the
        // pair-scoped exclusion drops the compare.
        assert!(run(
            &[("Bus2", "b2.0"), ("kV", "12.47")],
            &[("Bus2", "B2.0"), ("kV", "12.47")],
        ));
        assert_eq!(
            super::props_norm::norm_counters("reactor", "bus2"),
            Some((norm_before.0 + 1, norm_before.1 + 1)),
            "the normalization seam must see and fold the mixed pair's foldable cell BEFORE the \
             echo row is consulted — that hit is what keeps the row provably live after RP4.1"
        );
        // The echo row saw the same cell — already folded to `b2.0` on both
        // sides — and since RP4.1 P1 that is not one of its measured spellings,
        // so it is neither a visit nor a hit.
        assert_eq!(
            super::props_norm::echo_counters("reactor", "bus2"),
            Some(echo_before)
        );
        // …and the pair's real echo cell — r4133's stale `GetBus(2)` snapshot —
        // is a hit, which is what the liveness guard needs from this binary.
        assert!(run(
            &[("Bus2", "b2.0"), ("kV", "12.47")],
            &[("Bus2", "b2.0.0.0"), ("kV", "12.47")],
        ));
        assert_eq!(
            super::props_norm::echo_counters("reactor", "bus2"),
            Some((echo_before.0 + 1, echo_before.1 + 1))
        );
        assert_eq!(
            super::props_norm::norm_counters("reactor", "bus2"),
            Some((norm_before.0 + 2, norm_before.1 + 1)),
            "the echo cell reaches the normalization seam too — it is a visit there, and only \
             the fold above makes the row non-stale"
        );
    }

    /// **A mixed pair's exclusion covers only the spellings it measured** — the
    /// replacement RP2.3's audit settlement (2026-08-23) asked for and RP4.1's
    /// precondition 1 landed (2026-09-03).
    ///
    /// `load.yearly`'s `CaseFold` row cannot fold `'day'` against `'night'` —
    /// that is a genuine divergence, the kind the row exists to keep comparable.
    /// Until the narrowing the pair-scoped echo row masked it on r4133 all the
    /// same, because the exclusion was asked about the PAIR, and the capi
    /// channel was the whole of the port's live protection on those 20 pairs.
    /// Now `props_norm::ECHO_NARROWED` holds the row to the 23 spellings the
    /// census credits to it (`examples_full.txt:267-306`, all of the form
    /// `'<shape>'` vs `''`), so `'day'` vs `'night'` falls out of the exclusion
    /// and the r4133 channel fails on it too.
    ///
    /// This is the test the settlement wrote to be *replaced, not deleted*: its
    /// first assertion is the flipped one, and the capi and neighbour assertions
    /// are unchanged, so the diff shows exactly which behaviour moved.
    #[test]
    fn a_mixed_pairs_echo_row_masks_the_cells_its_rule_refuses() {
        // Counter hygiene first, and it is per TEST, not per binary: this one
        // drives `load.yearly`'s normalization row on cells it cannot fold, and
        // the gate's `assert_norm_rows_are_live()` may run before or after it in
        // the same process. One foldable cell up front leaves that row with
        // `hits > 0` whatever the ordering. The echo row gets its own hit from
        // the measured-spelling assertion at the end.
        assert!(run_load(
            PropsChannel::R4133,
            &[("Yearly", "day"), ("kW", "10")],
            &[("Yearly", "DAY"), ("kW", "10")],
        ));
        let divergent = (
            [("Yearly", "day"), ("kW", "10")],
            [("Yearly", "night"), ("kW", "10")],
        );
        assert!(
            !run_load(PropsChannel::R4133, &divergent.0, &divergent.1),
            "the narrowed echo row covers only its measured spellings, so a refused cell of a \
             mixed pair is compared on r4133 too (RP4.1 precondition 1)"
        );
        assert!(
            !run_load(PropsChannel::CapiV0145, &divergent.0, &divergent.1),
            "…and the capi channel, which excludes nothing, still fails on it as well"
        );
        // The row's own measured spelling is still excluded on r4133 — the
        // narrowing removed the over-breadth, not the exclusion.
        assert!(run_load(
            PropsChannel::R4133,
            &[("Yearly", "day"), ("kW", "10")],
            &[("Yearly", ""), ("kW", "10")],
        ));
        // The neighbour is untouched either way: the mask is field-scoped.
        assert!(!run_load(
            PropsChannel::R4133,
            &divergent.0,
            &[("Yearly", "night"), ("kW", "11")],
        ));
    }
}

/// The [`SKIP_PROPS`] half of [`skip_prop`] **only** — historically the
/// properties whose upstream getter renders uninitialized heap memory, and since
/// the changed-default rows (e)/(f) and RP3.8's live-render rows (g) a superset
/// of them: those eight are deterministic on both sides, and the dump artifact
/// below nulls their values too (a deliberate, recorded loss — the artifact's
/// argument is about the UB rows it MUST strip, not about the ones it may).
/// Channel-BLIND on purpose:
/// its consumer is the contamination artifact below, which wants the whole
/// table; the channel-scoped form the two property walks use is
/// [`skip_prop_ub_on`].
///
/// Kept separate because [`skip_prop`] has a second consumer that wants a
/// different question answered: `corpus_gate::write_gate_dump` nulls these
/// values in the three-way contamination artifact, and its whole argument is
/// that the stripped set is order-dependent *by construction*. A Stage F lane
/// exclusion is not — `Monitor.BaseFreq` is a deterministic function of the
/// deck — so folding it in would quietly drop a deterministic property from the
/// bit-diff and leave that artifact's stated rationale describing a set it no
/// longer had (F-settle W4).
pub fn skip_prop_ub(class: &str, prop: &str) -> bool {
    SKIP_PROPS
        .iter()
        .any(|(c, p)| class.eq_ignore_ascii_case(c) && prop.eq_ignore_ascii_case(p))
}

/// Transformer per-winding SINGULAR getters that index the ActiveWinding cursor
/// (`windings[aw()]`). Their value is `windings[ActiveWinding]`'s, so the two
/// engines compare validly ONLY when both cursors point to the SAME winding. The
/// oracle's own `gc.capture_discrete` `Transformers.Wdg=i` walk forces
/// ActiveWinding to NumWindings before the sweep, while Rust keeps the deck's
/// trailing `wdg=k` — usually also NumWindings (array-form parse), but not for a
/// 3-winding `t3w` (ends `wdg=2`) or a 2-winding `YgD-Test.tr1` (rewired
/// `wdg=1`). So the skip is gated on cursor DISAGREEMENT (not winding count):
/// whenever the cursors match — the common case — every singular form is
/// compared, including `RDCOhms`, which has NO array-form backstop (unlike
/// `%R`→`%Rs`, bus/conn/kV/kVA/tap→array forms, RNeut/XNeut→YPrim, tap
/// limits→the now-live-gated `TapNum`). See tests/TOLERANCE_NOTES.md.
const TRANSFORMER_CURSOR_PROPS: &[&str] = &[
    "Wdg", "Bus", "Conn", "kV", "kVA", "Tap", "%R", "RNeut", "XNeut", "MaxTap", "MinTap",
    "NumTaps", "RDCOhms",
];

/// Whether property `prop` of the named transformer is cursor-contaminated and
/// must be skipped — true only when the two engines' ActiveWinding cursors
/// disagree (the singular forms then read different windings).
fn skip_transformer_cursor(class: &str, prop: &str, cursors_disagree: bool) -> bool {
    cursors_disagree
        && class.eq_ignore_ascii_case("Transformer")
        && TRANSFORMER_CURSOR_PROPS
            .iter()
            .any(|p| p.eq_ignore_ascii_case(prop))
}

/// 0.15.x-only properties (`class` → property names, matched case-insensitively)
/// that the pinned **0.14.5** default oracle capture CANNOT contain. This is a
/// §1.3-style property-table *shape* relaxation, NEVER a value-tolerance change:
/// a Rust-side property whose `(class, name)` is in this table **and** whose name
/// is absent from the oracle capture's name list is excluded from the
/// count/order/name walk in [`compare_all_properties`] (a 0.14.5 oracle cannot
/// know a 0.15.x prop). Everything else keeps the exact existing semantics.
///
/// Rules (documented in tests/TOLERANCE_NOTES.md §"0.15.x property-table
/// allowlist"):
///  * If the oracle capture DOES contain the prop (a capi015-regenerated
///    capture), it is NOT excluded — full name+value compare applies. capi015
///    decks therefore keep pinning the new props' values; only 0.14.5-oracle
///    decks skip their existence.
///  * Inserted props are handled, not only trailing ones (e.g. LoadShape `Mode`
///    lands at index 22, shifting `Interpolation` 22→23 — the walk compares the
///    Rust list with `Mode` removed against the 0.14.5 capture).
///  * A NON-allowlisted extra/missing/misordered prop still fails exactly as it
///    does without this table.
///  * Every row must cite its upstream commit / UPGRADE_PLAN row in a comment
///    (same documentation style as [`SKIP_PROPS`]).
///  * The mechanism is channel-agnostic — it asks only whether THIS capture's
///    name list carries the prop — so a row may also relieve a prop the **r4133**
///    oracle cannot report while the 0.14.5 one can. Exactly one such row exists
///    (`GenDispatcher.weights`, R4133_PROPS_PLAN RP1.4: an r4133 registration
///    bug, not a version delta); it is inert on the capi channel for the same
///    reason the 0.15.x rows are inert on a capi015 capture.
///
/// Ships EMPTY: rows land with the WP that ports each 0.15.x property. Keep it
/// one class per line so parallel WP branches each add a line without conflict
/// (duplicate class rows are fine — the predicate ORs every matching row).
pub const PROPS_015X: &[(&str, &[&str])] = &[
    // dss_capi 0.15.x Line.pas:59-62 (SVN r3913-era) — WP-U1.4.
    // `Conductors` (Line.pas:62, prop 34) — the merged mixed wire/CN/TS
    // object-reference-array (wt-u14cond).
    (
        "Line",
        &["EpsRMedium", "HeightOffset", "HeightUnit", "Conductors"],
    ),
    // WP-U1.4 (wt-u14cond): LineGeometry.pas Conductors=20, inserted.
    ("LineGeometry", &["Conductors"]),
    // WP-U1.4 (wt-u14cnts): CNData.pas SemiconLayer=5, inserted.
    ("CNData", &["SemiconLayer"]),
    // WP-U1.6 C5 (dss_capi 0.15.x r4086, commit 8a898cba): RegControl gains the
    // idle-zone flags + the signed forward-power threshold.
    (
        "RegControl",
        &["Idle", "IdleReverse", "IdleForward", "FwdThreshold"],
    ),
    // WP-U1.6 C6 (dss_capi 0.15.x r4064, commit 90962ae8): GICharm BH-curve
    // `Unused` data props on BOTH transformer classes.
    ("Transformer", &["BHpoints", "BHcurrent", "BHflux"]),
    ("AutoTrans", &["BHpoints", "BHcurrent", "BHflux"]),
    // WP-U2.1 (EPRI r4133, delta C3/D1): Fuse props 10 -> 12 — the new TCC divisor
    // and interrupting rating, absent from the 0.14.5 capture. (This is a Rung-2
    // r4133 addition, not a 0.15.x-line one, but the mechanism — a post-0.14.5 prop
    // the 0.14.5 oracle cannot report — is identical.)
    ("Fuse", &["CurveMultiplier", "InterruptingRating"]),
    // WP-U2.4 C4 (EPRI r4133 `Controls/SwtControl.pas`, props 8->9): the new
    // informational `RatedCurrent`. r4133-only (absent from BOTH the 0.14.5 and
    // capi015 property tables). This name-based row (not the `PropFlags::HIDE_R4133`
    // flag, dropped at U2.5 when the SwtControl surface went full-r4133) is what
    // excludes it from the count/order/name walk on every default-oracle capture
    // (0.14.5 and capi015). Since R4133_PROPS RP4.1 (2026-09-03) r4133-oracle
    // cases DO property-compare, and there this row is inert in exactly the way
    // the `Generator` row below is: the r4133 capture's own name list carries
    // `RatedCurrent`, so `filter_015x` keeps it and it compares in full.
    ("SwtControl", &["RatedCurrent"]),
    // R4133_PROPS_PLAN RP1.1 (EPRI r4133 upstream stubs, `PropFlags::UPSTREAM_STUB`):
    // Generator props 48 -> 50 — `Rneut`/`Xneut` at display slots 16/17
    // (`Version8/Source/PCElements/generator.pas:441-442`), which dss_capi deleted
    // outright (no `TGeneratorProp` member in `.inputs/dss_capi/src/PCElements/
    // Generator.pas`). Like the SwtControl row above this is an r4133-only
    // addition, absent from BOTH the 0.14.5 and the capi015 tables; on the r4133
    // channel the oracle's own name list carries them, so `filter_015x` keeps them
    // and they compare in full.
    ("Generator", &["Rneut", "Xneut"]),
    // R4133_PROPS_PLAN RP1.1: Sensor props 15 -> 16 — `Action`
    // (`Version8/Source/Meters/Sensor.pas:183`), commented out in dss_capi 0.14.5
    // (`.inputs/dss_capi/src/Meters/Sensor.pas:39,55`).
    ("Sensor", &["Action"]),
    // R4133_PROPS_PLAN RP1.2 (a real port, not a stub): AutoTrans props 52 -> 53
    // — `XfmrCode` at slot 39 (`Version8/Source/PDElements/AutoTrans.pas:329`,
    // `TAutoTransObj.FetchXfmrCode` `:2339-2396`), which dss_capi 0.14.5 removed
    // outright (`.inputs/dss_capi/src/PDElements/AutoTrans.pas:76,125` —
    // `//XfmrCode=39, // removed, unused`) and capi015 never restored. Same
    // relief as the rows above: `prop_015x` drops a Rust-side prop only when the
    // oracle's own name list lacks it, so this row is active on the 0.14.5/capi015
    // captures and inert on r4133, whose list carries `XfmrCode`.
    ("AutoTrans", &["XfmrCode"]),
    // R4133_PROPS_PLAN RP1.4 — the one row that runs the OTHER way: an r4133
    // *registration bug* hides a property the 0.14.5 oracle reports fine.
    // `TGenDispatcher.DefineProperties` names seven properties but declares
    // `NumPropsThisClass = 6` (`Version8/Source/Controls/GenDispatcher.pas:92`),
    // so `PropertyName^[7] := 'Weights'` (`:133`) is overwritten by
    // `TCktElementClass.DefineProperties`' `PropertyName^[ActiveProperty + 1] :=
    // 'basefreq'` (`Common/CktElementClass.pas:98`) — measured on the r4133 DLL:
    // `AllPropertyNames` returns 9 names without `weights`, `? gd.weights` is
    // "Property Unknown", `weights=` is error #364, and `basefreq=` parses as the
    // weights vector (the `Edit` CASE arm 7 was never renumbered, `:200-206`).
    // Upstream report: `investigations/to_opendss/
    // 40-gendispatcher-weights-registration-off-by-one.md`; the fix there is
    // `NumPropsThisClass = 7`. The port is correct and matches dss_capi, which
    // counts the enum (`.inputs/dss_capi/src/Controls/GenDispatcher.pas:106`), so
    // the row is INERT on the capi channel (its name list has `weights` →
    // `filter_015x` keeps it, full name+value compare) and active on r4133 only.
    // **Dormant until a gendispatcher deck gates r4133**: all three
    // `controls:gendispatcher/*` decks are `engines: "capi_v0145"` and stay so —
    // measured 2026-08-23 (RP1.4 STATUS record), r4133 cannot receive their
    // `weights=[3, 1]` at all, so it dispatches the equal split and the whole
    // solved state moves (12-step `gendispatcher.dss`, kW: 48 % apart at the
    // worst of steps 1-11 and 54.5x at step 0, where r4133 holds both machines
    // at the `Max(1.0, …)` floor while the port dispatches 55.52 kW — the
    // census's `generator.kw` `max_rel` 5.45e+01; kvar reaches 1.59e+00), which
    // is not a `property`-scoped divergence any pin could cover. A NEW deck
    // without `weights=` would behave identically on both engines and could gate
    // `both` — measured, and deliberately not taken inside RP1.4 (STATUS RP1.4
    // audit settlement, item 8); a future one must also keep `basefreq=` off
    // its dispatcher, since r4133's slot 7 IS the weights arm and would compare
    // the port's base frequency against r4133's stored weights string.
    // `PROPS_015X` rows carry no live counters (§1.1(d)), so nothing goes stale;
    // the row's exercisers are
    // `props_015x_tests::shipped_gendispatcher_weights_row_is_inert_when_the_oracle_knows_it`
    // (the capi-side inertness) and, since RP2.1 part C, the offline replay over
    // the full `shape.txt` — `crates/dss-core/tests/props_r4133_replay.rs`,
    // `the_shape_allowlist_rows_are_exercised_by_shape_txt`, which is where this
    // row is the ONE that fires on the r4133 side.
    ("GenDispatcher", &["weights"]),
    // Further rows land here with their porting WP.
];

/// Whether property `prop` of `class` is a 0.15.x-only property in `allowlist`
/// (matched case-insensitively across every row, so duplicate class rows OR).
pub fn prop_015x(allowlist: &[(&str, &[&str])], class: &str, prop: &str) -> bool {
    allowlist.iter().any(|(c, props)| {
        class.eq_ignore_ascii_case(c) && props.iter().any(|p| prop.eq_ignore_ascii_case(p))
    })
}

/// The oracle capture's property names, lowercased — the "does the pinned
/// oracle know this prop?" set that gates the [`PROPS_015X`] exclusion.
///
/// Shared verbatim by the gating walk ([`compare_prop_lists`]) and the census
/// walk ([`collect_element_divergences`]) so the two cannot drift apart
/// (`R4133_PROPS_PLAN.md` RP0.2 audit: the pre-walk policy used to be written
/// twice).
fn oracle_name_set(oracle: &[(String, String)]) -> BTreeSet<String> {
    oracle.iter().map(|(n, _)| n.to_lowercase()).collect()
}

/// Drop the Rust-side props the pinned capture cannot know: a name in
/// `allowlist` for this `class` that is ALSO absent from `oracle_names`. One
/// the capture DOES carry stays in and is fully compared. Shared by both walks
/// (see [`oracle_name_set`]).
fn filter_015x<'a>(
    class: &str,
    actual: &'a [(String, String)],
    oracle_names: &BTreeSet<String>,
    allowlist: &[(&str, &[&str])],
) -> Vec<&'a (String, String)> {
    actual
        .iter()
        .filter(|(n, _)| {
            !prop_015x(allowlist, class, n) || oracle_names.contains(&n.to_lowercase())
        })
        .collect()
}

/// Whether the whole ELEMENT is dropped before a single cell is looked at.
///
/// Recloser and Relay: their property TABLES moved to the r4133 surface
/// (WP-U2.2/U2.3 — 24→46 and 50→71 props, renames plus the `Normal`/`State`
/// per-phase arrays), so they cannot match the pinned **0.14.5** capture's shape
/// at all; skipping the element beats masking 46 individual rows. That argument
/// is about the capi channel and only it, so RP2.1 made the skip
/// **channel-scoped** (`R4133_PROPS_PLAN.md` §1.2): on
/// [`PropsChannel::R4133`] both classes compare in FULL — the justification
/// inverts, because the Rust tables are the r4133 tables. Without this the
/// census's 22 relay/recloser pairs would be dead on the live path and RP4.1's
/// closure unreachable.
///
/// Shared by the gating walk ([`compare_all_properties`]) and the census walk
/// ([`collect_prop_divergences`], which counts every skip into
/// [`CensusBlindSpots`]) so the two cannot drift (see [`oracle_name_set`]).
fn skip_whole_element(class: &str, channel: PropsChannel) -> bool {
    channel == PropsChannel::CapiV0145
        && (class.eq_ignore_ascii_case("Recloser") || class.eq_ignore_ascii_case("Relay"))
}

/// Whether the two sides' Transformer `ActiveWinding` cursors point at different
/// windings — the gate on [`skip_transformer_cursor`]. Non-Transformer classes
/// are always `false`. Shared by both walks (see [`oracle_name_set`]).
///
/// Channel-BLIND, and that is a disposition too (RP2.1): the cursor
/// contamination is a property of the CAPTURE procedure — the oracle's
/// `Transformers.Wdg=i` discrete sweep leaves the cursor at NumWindings while
/// Rust keeps the deck's trailing `wdg=k` — and the r4133 bridge captures the
/// same way (`tests/corpus/props_r4133/README.md` §"Corrections measured after
/// freezing", correction 2: the knob drops the same 13 cursor props on both
/// channels). Nothing about it is 0.14.5-specific.
fn transformer_cursors_disagree(
    class: &str,
    actual: &[(String, String)],
    oracle: &[(String, String)],
) -> bool {
    if !class.eq_ignore_ascii_case("Transformer") {
        return false;
    }
    let cursor_of = |props: &[(String, String)]| -> Option<String> {
        props
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("Wdg"))
            .map(|(_, v)| v.trim().to_string())
    };
    cursor_of(actual) != cursor_of(oracle)
}

/// The property-list comparison core of [`compare_all_properties`], factored out
/// so the [`PROPS_015X`] allowlist can be injected for the self-tests (the
/// shipped table is empty). `actual` is the Rust `?`-surface list, `oracle` the
/// capture; both are `(name, value)` in property-index order. A Rust-side prop
/// that is 0.15.x-only (`allowlist`) AND absent from the `oracle` capture is
/// dropped before the count/order/name walk; every surviving prop is compared
/// exactly as before (name in order, value via [`assert_value_matches_tol`],
/// with the [`SKIP_PROPS`]/[`skip_transformer_cursor`] value-skip gates).
///
/// `policy` says which oracle is on the other side and carries the RP2.1
/// normalization seam ([`PropsPolicy`]); it travels as a parameter for the same
/// testability reason `allowlist` does. On [`PropsChannel::CapiV0145`] every
/// step below is the pre-RP2.1 one.
#[allow(clippy::too_many_arguments)]
fn compare_prop_lists(
    element: &str,
    class: &str,
    actual: &[(String, String)],
    oracle: &[(String, String)],
    allowlist: &[(&str, &[&str])],
    policy: PropsPolicy,
    cursors_disagree: bool,
    rel: f64,
    abs: f64,
    ctx: &str,
) {
    // Oracle capture's property names (case-insensitive) — the "does the pinned
    // oracle know this prop?" set that gates the 0.15.x exclusion. Exclude
    // Rust-side 0.15.x-only props the 0.14.5 capture cannot contain; if the
    // capture DOES contain the prop (capi015), keep it → full compare. Both
    // steps are the SHARED helpers the census walk calls too.
    let oracle_names = oracle_name_set(oracle);
    let filtered = filter_015x(class, actual, &oracle_names, allowlist);
    assert_eq!(
        filtered.len(),
        oracle.len(),
        "{ctx}: {element} property count differs (rust {} -> {} after PROPS_015X allowlist \
         vs oracle {}) — property-table shape changed. A NON-allowlisted extra/missing prop \
         fails here; a deliberately ported 0.15.x prop must be added to PROPS_015X in \
         tests/harness/mod.rs",
        actual.len(),
        filtered.len(),
        oracle.len()
    );
    for (i, (a, e)) in filtered.iter().zip(oracle).enumerate() {
        let (aname, aval) = (&a.0, &a.1);
        let (ename, eval) = (&e.0, &e.1);
        assert!(
            aname.eq_ignore_ascii_case(ename),
            "{ctx}: {element} property {i} name differs: rust {aname:?} vs oracle {ename:?} \
             (property-index order is the contract)"
        );
        if skip_prop(class, ename, policy.channel())
            || skip_transformer_cursor(class, ename, cursors_disagree)
        {
            continue;
        }
        // THE NORMALIZATION SEAM (plan §1.2): the channel-scoped, strictly
        // value-preserving re-spelling of both sides. Identity on the capi
        // channel — always, by contract — so the assert below sees exactly the
        // strings it saw before RP2.1.
        let (aval, eval) = policy.normalize(class, ename, aval, eval);
        // THE EXCLUSION SEAM (plan §1.2, RP2.3), the chain's third link: a
        // cited `PROPS_ECHO_R4133` pair whose two renderings are not two
        // spellings of one value drops the VALUE assert here — name and order
        // are already checked above. r4133 only, and AFTER the normalization
        // seam, so a mixed pair's foldable cells are seen (and COUNTED) by their
        // typed rule first; the exclusion itself is pair-scoped, so it then
        // covers the whole pair, folded cells included (harmless — they are
        // equal by now) and refused ones too (not harmless — see
        // `props_norm::PROPS_ECHO_R4133`'s doc and the RP4.1 precondition).
        // Identity on the capi channel, always.
        if policy.echo_excluded(class, ename, &aval, &eval) {
            continue;
        }
        // THE DISPLAY-FLOOR SEAM (plan §1.2, RP2.4), the chain's fourth and
        // last link: two renders of ONE number that differ only in how many
        // significant digits r4133's `Format('%[-].Ng', …)` getter printed.
        // Per CELL, never per pair — a larger divergence on the same property
        // still fails below, and a non-numeric cell never reaches the floor at
        // all. r4133 only, and AFTER the exclusion, so a cell an earlier link
        // owns is credited to that link. Identity on the capi channel, always.
        if policy.under_display_floor(&aval, &eval) {
            continue;
        }
        // Case-EXACT compare (no lowercasing): every DSS enum getter renders the
        // Pascal-faithful case — `ordinal_to_string` returns the exact registry
        // strings (`wye`/`delta` lowercase, `Variable`/`Fixed` capitalized,
        // booleans `Yes`/`No`) that the oracle's `Val` emits, so a case
        // divergence is a real rendering regression this gate must catch, not a
        // formatting artifact to smooth over.
        assert_value_matches_tol(
            &aval,
            &eval,
            rel,
            abs,
            &format!("{ctx}: {element} property {ename}"),
        );
    }
}

/// Compare EVERY property of EVERY captured element (WP8.5b): the property-NAME
/// lists must be equal IN ORDER (case-insensitive — pins the property-table
/// shape), then each value through [`assert_value_matches_tol`] (the same
/// `compare_probe` numeric-skeleton semantics). A `(class, prop)` in
/// [`SKIP_PROPS`] is excluded from the VALUE compare only (its name is still
/// order-checked). A Rust-side property that is 0.15.x-only ([`PROPS_015X`]) and
/// absent from the (0.14.5-pinned) oracle capture is excluded from the whole walk
/// — a property-table *shape* relaxation, never a value-tolerance change. This
/// catches latent property-rendering/port bugs the live-model gate (Y/V/I/P)
/// cannot see.
///
/// `channel` (RP2.1) says which oracle produced `exp`. It selects the
/// whole-element skips ([`skip_whole_element`]), the value-skip set
/// ([`skip_prop`]) and the normalization seam ([`PropsPolicy`]); on
/// [`PropsChannel::CapiV0145`] all three are what they were before RP2.1.
pub fn compare_all_properties(
    dss: &mut Dss,
    exp: &[PropsCap],
    tol: &Tolerances,
    channel: PropsChannel,
    ctx: &str,
) {
    let policy = PropsPolicy::for_channel(channel);
    // The r4133 half of this walk is what RP4.1 unmasked, and nothing else in
    // the tree can say whether it ran: see
    // [`props_norm::assert_r4133_props_compare_ran`], the gate-epilogue guard
    // this counter feeds. Counted here, at the ONE gating entry point, rather
    // than off the tables' `(visits, hits)` — see that helper's doc for why the
    // table sums cannot answer the question.
    let mut compared_elements = 0usize;
    for pc in exp {
        let class = pc.element.split('.').next().unwrap_or("");
        // The channel-scoped Recloser/Relay whole-element skip: their tables are
        // the r4133 ones and cannot match a 0.14.5 capture's shape, so the skip
        // holds on the capi channel and INVERTS on r4133, where both classes
        // compare in full. Rationale and citations: [`skip_whole_element`].
        if skip_whole_element(class, channel) {
            continue;
        }
        let actual = dss
            .element_properties(&pc.element)
            .unwrap_or_else(|| panic!("{ctx}: no element {} (all_properties)", pc.element));
        // Transformer cursor-skip gate (see [`skip_transformer_cursor`]): the
        // singular per-winding forms compare only when both engines' ActiveWinding
        // (the `Wdg` value) point to the same winding. Read Rust's from `actual`
        // and the oracle's from the capture. Shared with the census walk.
        let cursors_disagree = transformer_cursors_disagree(class, &actual, &pc.props);
        compare_prop_lists(
            &pc.element,
            class,
            &actual,
            &pc.props,
            PROPS_015X,
            policy,
            cursors_disagree,
            tol.i_rel,
            tol.i_abs,
            ctx,
        );
        compared_elements += 1;
    }
    if channel == PropsChannel::R4133 {
        props_norm::record_r4133_props_walk(compared_elements);
    }
}

// ---------------------------------------------------------------------------
// The census walk (R4133_PROPS_PLAN.md RP0.2) — the same compare, collecting.
// ---------------------------------------------------------------------------

/// Which oracle channel a property compare is running against, expressed in a
/// type the harness itself owns.
///
/// `corpus_gate`'s `EngineChannel` is `pub(crate)` to that one test binary while
/// `harness/` compiles into ~20 others, so the channel cannot travel as that
/// type (plan §1.2, the channel-threading trap). The corpus_gate call sites map
/// `EngineChannel` onto this (`EngineChannel::props_channel`). **Both** property
/// walks read it since RP2.1: the gating [`compare_all_properties`] (through
/// [`PropsPolicy`]) and the census [`collect_prop_divergences`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropsChannel {
    /// The pinned dss-python oracle (dss_capi 0.14.5).
    CapiV0145,
    /// The official EPRI OpenDSS r4133 DLL, through `dss-epri`.
    R4133,
}

impl PropsChannel {
    /// Channel tag as the census artifacts spell it.
    pub fn tag(self) -> &'static str {
        match self {
            PropsChannel::CapiV0145 => "capi_v0145",
            PropsChannel::R4133 => "r4133",
        }
    }
}

/// The channel-scoped VALUE policy both property walks consult: which oracle is
/// on the other side of the compare, and — from RP2.1 part B on — the
/// normalization table that may re-SPELL a cell before the value assert
/// (`R4133_PROPS_PLAN.md` §1.2).
///
/// It travels as a **parameter** of [`compare_prop_lists`] for the same reason
/// the shape allowlist does — testability: the shipped policy is derived from
/// the channel ([`PropsPolicy::for_channel`], which is what
/// [`compare_all_properties`] builds), while a self-test can hand either walk
/// any policy it likes.
///
/// **Capi-invariance (plan §1.1(b)) is structural, not a promise:** every
/// behavior this plan adds hangs off [`PropsPolicy::is_r4133`], so a
/// `CapiV0145` policy walks the pre-RP2.1 code path — same shape relief, same
/// skip set, same value assert on the raw strings. The A/B run recorded in the
/// RP2.1 STATUS record measures that; [`props_policy_tests`] pins it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropsPolicy {
    channel: PropsChannel,
    /// Whether the channel's VALUE policy — [`PropsPolicy::normalize`], and
    /// after RP2.3/RP2.4 the echo table and the display floor — is ARMED.
    ///
    /// The gate always arms it. The census knob's **plain** mode
    /// (`DSS_PROPS_CENSUS=1`) deliberately does not: plain is "the
    /// un-normalized comparator, RP0.1's measurement", and RP0.2 fixed it as the
    /// knob's baseline forever, so a rule landing in part B must not quietly
    /// shrink what a re-census reports. Its `claims` mode arms the policy again
    /// (that is the whole point of the mode) and calls this same seam.
    ///
    /// The SKIP set is NOT on this axis: [`skip_whole_element`] and
    /// [`skip_prop`] read the raw channel, because the census has always
    /// honored them and its extracts are measured with them active.
    value_policy: bool,
}

impl PropsPolicy {
    /// The shipped, fully-ARMED policy for `channel` — what the gate builds
    /// (and, from RP2.1's disposition mode on, the `claims` census).
    pub fn for_channel(channel: PropsChannel) -> Self {
        Self {
            channel,
            value_policy: true,
        }
    }

    /// The channel's policy with the VALUE half disarmed: the skip/shape rules
    /// of `channel`, but the raw un-normalized value compare. The plain census
    /// mode's policy (see [`PropsPolicy::value_policy`]).
    pub fn plain(channel: PropsChannel) -> Self {
        Self {
            channel,
            value_policy: false,
        }
    }

    /// The channel this policy speaks for.
    pub fn channel(self) -> PropsChannel {
        self.channel
    }

    /// Whether the r4133 value arms are live. The single gate on every VALUE
    /// behavior `R4133_PROPS_PLAN.md` adds to the comparator.
    fn is_r4133(self) -> bool {
        self.value_policy && self.channel == PropsChannel::R4133
    }

    /// **The normalization seam** (plan §1.2, RP2.1): the last chance to
    /// re-spell the two sides of one cell before
    /// [`assert_value_matches_tol`] sees them.
    ///
    /// The contract is plan mechanic (c), **value-preserving normalization
    /// only** — a rule may change how a value is SPELLED, never WHICH value it
    /// is (the `lane::expected_rerounded` discipline, `lane.rs:546-592`).
    /// Anything that cannot satisfy that is an exclusion (RP2.3's echo table),
    /// never a rule here.
    ///
    /// On r4133 the seam consults the typed
    /// [`PROPS_NORM_R4133`](props_norm::PROPS_NORM_R4133) rules
    /// (`BoolFold`/`CaseFold`/`ArrayForm`/`EnumSynonym`, RP2.1 part B): a rule
    /// that recognises the two sides as the same value answers with OUR
    /// spelling on both, and anything it does not claim comes back raw, so a
    /// real divergence still fails with both original spellings in the message.
    ///
    /// On capi the seam is the identity. That arm is not "not written yet" —
    /// it is the permanent capi-invariance contract, pinned by
    /// [`props_policy_tests::the_capi_channel_never_normalizes`].
    fn normalize<'v>(
        self,
        class: &str,
        prop: &str,
        rust: &'v str,
        oracle: &'v str,
    ) -> (Cow<'v, str>, Cow<'v, str>) {
        if self.is_r4133() {
            return props_norm::normalize_r4133(class, prop, rust, oracle);
        }
        (Cow::Borrowed(rust), Cow::Borrowed(oracle))
    }

    /// **The exclusion seam** (plan §1.2, RP2.3): the chain's third link, asked
    /// *after* [`PropsPolicy::normalize`] has had its chance and immediately
    /// before [`assert_value_matches_tol`] would run.
    ///
    /// `true` drops the VALUE compare of this cell — the property's name and
    /// index order have already been asserted, exactly [`SKIP_PROPS`]' shape.
    /// Which pairs, and the r4133 citation plus witness each one carries, is
    /// [`props_norm::PROPS_ECHO_R4133`]; the one cell a row deliberately does
    /// not cover is `props_norm::ECHO_CARVE_OUTS`.
    ///
    /// **The order matters, and here is exactly what it does.** Normalization
    /// runs first, so on a mixed pair (20 of the 82 rows) a typed rule sees the
    /// cell and — when it folds it — records its own hit, which is what keeps
    /// that row provably live and what makes the claims census disposition
    /// `normalized-by-<rule>` instead of `echo-row`. Asking this seam first
    /// would make every mixed pair's normalization row look dead.
    ///
    /// What the order does **not** do is keep those cells in the compare: this
    /// answer is pair-scoped, so once it says `true` the value assert is dropped
    /// for the folded cells (harmless — normalization has already made the two
    /// sides equal) and for the ones the rule refused (a real divergence, caught
    /// on the capi channel only). Narrowing the mixed rows per cell is an RP4.1
    /// precondition; see [`props_norm::PROPS_ECHO_R4133`] and
    /// [`props_policy_tests::a_mixed_pairs_echo_row_masks_the_cells_its_rule_refuses`].
    ///
    /// On capi the seam is `false` for everything. That arm is the permanent
    /// capi-invariance contract, not an unfinished one: capi is where most of
    /// these rows' witness lives, so an echo row leaking onto that channel
    /// would delete the evidence the row cites. Pinned by
    /// [`props_policy_tests::the_capi_channel_never_excludes`].
    ///
    /// [`assert_value_matches_tol`]: super::assert_value_matches_tol
    fn echo_excluded(self, class: &str, prop: &str, rust: &str, oracle: &str) -> bool {
        self.is_r4133() && props_norm::echo_excluded_r4133(class, prop, rust, oracle)
    }

    /// **The display-floor seam** (plan §1.2, RP2.4): the chain's fourth and
    /// last link, asked after [`PropsPolicy::echo_excluded`] has had its chance
    /// and immediately before [`assert_value_matches_tol`] would run.
    ///
    /// `true` drops the VALUE compare of this cell because the two sides are
    /// **one number, printed to different precision** — r4133's
    /// `Format('%[-].Ng', …)` getter against the port's full render, within
    /// `props_norm::R4133_DISPLAY_FLOOR` (`2e-4` relative; the derivation, the
    /// measured band and the `%[-].Ng` site table are on that constant).
    /// The property's name and index order have already been asserted.
    ///
    /// **It is a cell predicate, not a pair mask**, and that is the whole
    /// difference from the seam above it — the RP2.3 audit settlement's lesson
    /// applied at design time. It reads only the two values, so:
    ///
    /// * a *bigger* divergence on the very same `(class, prop)` still fails;
    /// * a cell whose two sides are not one numeric shape (an enum, a boolean,
    ///   `''` against a value, an array of another length) still fails raw;
    /// * nothing is excluded by name, so there is no row to go stale.
    ///
    /// It runs last for the same reason the exclusion runs after normalization:
    /// a cell an earlier link claims must be credited to that link, or its
    /// liveness accounting would rot behind a tolerance
    /// (`props_r4133_replay::first_match_returns_the_earliest_link`).
    ///
    /// On capi the seam is `false` for everything — the permanent
    /// capi-invariance contract, pinned by
    /// [`props_policy_tests::the_capi_channel_never_applies_the_display_floor`].
    /// This is the one arm where that matters most numerically: the capi
    /// channel's property compare runs at the case's own tier floors
    /// (`compare_all_properties` passes `tol.i_rel`/`tol.i_abs`, 1e-9/1e-6 on
    /// `micro` and 1e-7/1e-5 on `feeder` — orders under this floor), and a floor
    /// leaking onto it would relax every numeric property of every `both` case
    /// to 2e-4 at once. "Byte-exact" is what this comment used to say and it was
    /// wrong (RP2.4 audit round); the tier floors are pinned by
    /// [`props_policy_tests::the_capi_property_compare_runs_at_the_case_tier_floors`].
    ///
    /// [`assert_value_matches_tol`]: super::assert_value_matches_tol
    fn under_display_floor(self, rust: &str, oracle: &str) -> bool {
        self.is_r4133() && props_norm::under_display_floor_r4133(rust, oracle)
    }
}

/// One divergent cell (or shape gap) found by [`collect_prop_divergences`].
///
/// The variants are exactly the census `kind`s of the 2026-08-08 G1.1 measurement
/// vendored at `tests/corpus/props_r4133/` (`value_structure` / `value_numeric` /
/// `shape_count`), plus one defensive variant for a capture the Rust engine has
/// no element for — the census never produced such a row, and the knob must
/// still record rather than panic.
#[derive(Debug, Clone)]
pub enum PropCensusRow {
    /// The two property-NAME sets differ (count and/or membership).
    Shape {
        element: String,
        rust_count: usize,
        oracle_count: usize,
        /// Lowercased names the Rust table has and the capture does not.
        rust_only: Vec<String>,
        /// Lowercased names the capture has and the Rust table does not.
        oracle_only: Vec<String>,
    },
    /// A value cell whose two renderings do not match at the case tier floor.
    Value {
        element: String,
        prop: String,
        rust: String,
        oracle: String,
        /// `Some` for a numeric divergence (skeletons agree), `None` for a
        /// structural one.
        max_rel: Option<f64>,
    },
    /// The capture names an element the Rust engine does not have.
    MissingElement { element: String },
}

/// What one census walk could NOT look at, reported alongside the rows it did
/// produce. Metadata, never a census row — but never silent either: an
/// uncounted skip would make a partial census read like a complete one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CensusBlindSpots {
    /// Cells no index-ordered comparison could reach because the two name lists
    /// had desynchronized at that position — a property-table shape gap earlier
    /// in the element, or a pure re-ordering — plus the tail the shorter list
    /// cannot reach. An insertion at property *k* hides every cell after *k* on
    /// that element, which is what makes the vendored census's value population
    /// of the five shape-gap classes a LOWER BOUND rather than a total. WP-RP1
    /// closes the shape gaps and with them this blind spot.
    pub unaligned_cells: usize,
    /// Elements dropped WHOLE before a single cell was looked at — the
    /// channel-scoped Recloser/Relay skip below. Their cells appear neither in
    /// the rows nor in `unaligned_cells`, so `unaligned_cells == 0` on a channel
    /// with skips does NOT mean that channel's census is complete.
    pub skipped_elements: usize,
    /// The oracle-side property cells those skipped elements carried — the size
    /// of the hole `skipped_elements` leaves.
    pub skipped_element_cells: usize,
}

impl CensusBlindSpots {
    /// Fold another walk's blind spots in (the scheduler accumulates per case).
    pub fn add(&mut self, other: CensusBlindSpots) {
        self.unaligned_cells += other.unaligned_cells;
        self.skipped_elements += other.skipped_elements;
        self.skipped_element_cells += other.skipped_element_cells;
    }
}

/// Walk one checkpoint's property capture and COLLECT every divergence instead
/// of asserting on the first — the measurement half of the `DSS_PROPS_CENSUS`
/// knob (`R4133_PROPS_PLAN.md` RP0.2).
///
/// This function owns only the per-ELEMENT framing (the channel-scoped
/// whole-element skips and the engine lookup); the comparison itself is
/// [`collect_element_divergences`], the collecting twin of
/// [`compare_prop_lists`].
///
///  * **The Recloser/Relay whole-element skips are channel-scoped**: they exist
///    because those tables moved to the r4133 surface and cannot match a 0.14.5
///    capture (see [`compare_all_properties`]), which is an argument about the
///    capi channel only. On [`PropsChannel::R4133`] both classes are compared —
///    plan §1.2; without this the census's 22 relay/recloser pairs would not
///    exist. Every skip is COUNTED into [`CensusBlindSpots`]. Since RP2.1 the
///    predicate itself is the shared [`skip_whole_element`], so the gate and the
///    census cannot drift apart on it.
///
/// Never panics on a divergence and never asserts.
pub fn collect_prop_divergences(
    dss: &mut Dss,
    exp: &[PropsCap],
    tol: &Tolerances,
    channel: PropsChannel,
    out: &mut Vec<PropCensusRow>,
) -> CensusBlindSpots {
    // PLAIN policy: the census's baseline mode compares raw values (RP0.2), and
    // the skip/shape rules of the channel still apply — see
    // [`PropsPolicy::value_policy`]. The `claims` mode passes an armed policy.
    let policy = PropsPolicy::plain(channel);
    let mut blind = CensusBlindSpots::default();
    for pc in exp {
        let class = pc.element.split('.').next().unwrap_or("");
        if skip_whole_element(class, channel) {
            blind.skipped_elements += 1;
            blind.skipped_element_cells += pc.props.len();
            continue;
        }
        let Some(actual) = dss.element_properties(&pc.element) else {
            out.push(PropCensusRow::MissingElement {
                element: pc.element.clone(),
            });
            continue;
        };
        let cursors_disagree = transformer_cursors_disagree(class, &actual, &pc.props);
        blind.unaligned_cells += collect_element_divergences(
            &pc.element,
            class,
            &actual,
            &pc.props,
            PROPS_015X,
            policy,
            cursors_disagree,
            tol.i_rel,
            tol.i_abs,
            out,
        );
    }
    blind
}

/// The collecting twin of [`compare_prop_lists`]: the SAME policy chain — the
/// same [`oracle_name_set`]/[`filter_015x`] shape relief, the same index walk
/// ("property-index order is the contract"), the same
/// [`skip_prop`]/[`skip_transformer_cursor`] value-skip gates, the same
/// [`value_verdict`] floor — with every abort turned into a row.
///
/// The `allowlist` and `policy` parameters exist for the same reason
/// [`compare_prop_lists`]'s do: so the self-tests can inject a synthetic
/// table / any channel (the shipped [`PROPS_015X`] and the plain policy of the
/// walked channel are what [`collect_prop_divergences`] passes). The two walks
/// are pinned against each other by
/// `census_walk_tests::the_two_walks_agree_on_every_input` — **per channel**,
/// under matched policies: the gate panics on an input **iff** this walk reports
/// a row or a blind spot.
///
/// The one deliberate difference is what turns an *assert* into a *census*, and
/// it reproduces the 2026-08-08 G1.1 walk vendored at
/// `tests/corpus/props_r4133/`: **nothing aborts.** The gate's `assert_eq!` on
/// the property count and its `assert!` on each name stop the element dead;
/// here a count/membership mismatch becomes a [`PropCensusRow::Shape`] row and
/// the index walk continues, comparing every position whose two names still
/// agree. A position whose names do NOT agree is uncomparable — the return
/// value counts those cells (see [`CensusBlindSpots::unaligned_cells`]).
///
/// A second, smaller asymmetry follows from that: the gate asserts the property
/// COUNT and then catches everything else through the per-index name assert,
/// while this walk needs the name-SET comparison too, because it has to decide
/// up front whether to emit a shape row for an element whose counts happen to
/// match. The two statements coincide on every input (pinned by the same test).
#[allow(clippy::too_many_arguments)]
fn collect_element_divergences(
    element: &str,
    class: &str,
    actual: &[(String, String)],
    oracle: &[(String, String)],
    allowlist: &[(&str, &[&str])],
    policy: PropsPolicy,
    cursors_disagree: bool,
    rel: f64,
    abs: f64,
    out: &mut Vec<PropCensusRow>,
) -> usize {
    let mut unaligned = 0usize;
    let oracle_names = oracle_name_set(oracle);
    let filtered = filter_015x(class, actual, &oracle_names, allowlist);
    let rust_names: BTreeSet<String> = filtered.iter().map(|(n, _)| n.to_lowercase()).collect();
    if filtered.len() != oracle.len() || rust_names != oracle_names {
        // The two name lists are printed in PROPERTY-INDEX order (what the
        // extracts show and what a reader needs to locate the insertion),
        // not sorted.
        out.push(PropCensusRow::Shape {
            element: element.to_string(),
            rust_count: filtered.len(),
            oracle_count: oracle.len(),
            rust_only: filtered
                .iter()
                .map(|(n, _)| n.to_lowercase())
                .filter(|n| !oracle_names.contains(n))
                .collect(),
            oracle_only: oracle
                .iter()
                .map(|(n, _)| n.to_lowercase())
                .filter(|n| !rust_names.contains(n))
                .collect(),
        });
        unaligned += filtered.len().abs_diff(oracle.len());
    }
    for (a, e) in filtered.iter().zip(oracle) {
        let (aname, aval) = (&a.0, &a.1);
        let (ename, eval) = (&e.0, &e.1);
        if !aname.eq_ignore_ascii_case(ename) {
            // The lists desynchronized (a shape gap earlier in the table, or a
            // re-ordering). The gate would have failed here; the census records
            // the cell as uncomparable and keeps walking in case they re-align.
            unaligned += 1;
            continue;
        }
        if skip_prop(class, ename, policy.channel())
            || skip_transformer_cursor(class, ename, cursors_disagree)
        {
            continue;
        }
        // The same normalization seam the gate applies, so the two walks stay
        // the biconditional pair below under ANY policy. Identity under the
        // plain census policy this walk is normally handed. Only the VERDICT
        // reads it: a census row always records what the two engines actually
        // rendered, never a re-spelled surrogate.
        let (na, ne) = policy.normalize(class, ename, aval, eval);
        let max_rel = match value_verdict(&na, &ne, rel, abs) {
            ValueVerdict::Match => continue,
            ValueVerdict::Skeleton | ValueVerdict::NumberCount => None,
            ValueVerdict::Numeric { max_rel, .. } => Some(max_rel),
        };
        out.push(PropCensusRow::Value {
            element: element.to_string(),
            prop: ename.clone(),
            rust: aval.clone(),
            oracle: eval.clone(),
            max_rel,
        });
    }
    unaligned
}

#[cfg(test)]
mod census_walk_tests {
    use super::{PropsChannel, PropsPolicy, collect_element_divergences, compare_prop_lists};

    /// Synthetic allowlist — the same one `props_015x_tests` injects, so both
    /// walks see identical shape relief.
    const TEST_ALLOW: &[(&str, &[&str])] = &[("Foo", &["NewTrail", "NewMid"])];

    fn props(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(n, v)| (n.to_string(), v.to_string()))
            .collect()
    }

    /// `(name, rust list, oracle list, the gate is expected to panic)`.
    type WalkCase = (
        &'static str,
        &'static [(&'static str, &'static str)],
        &'static [(&'static str, &'static str)],
        bool,
    );

    /// **The anti-drift pin** (`R4133_PROPS_PLAN.md` RP0.2 audit). The gating
    /// walk and the census walk state the same policy in two shapes — one
    /// aborting, one collecting — so nothing but a test can stop them drifting.
    /// Over a table that exercises every branch of both (equal lists, a value
    /// mismatch, an allowlisted extra trailing and inserted, a non-allowlisted
    /// extra, a missing prop, a pure re-ordering, an empty pair of lists), this
    /// asserts the exact biconditional:
    ///
    /// > `compare_prop_lists` panics **iff** `collect_element_divergences`
    /// > reports at least one row or at least one unaligned cell.
    ///
    /// The re-ordering case is why the right-hand side needs the blind-spot
    /// term: a permutation leaves the counts and the name SETS equal, so the
    /// census emits no row at all — it reports the two positions as
    /// uncomparable instead, while the gate panics on the name assert.
    ///
    /// RP2.1 runs the whole table on **both channels** under matched policies:
    /// the two walks now share a channel-scoped skip set and a normalization
    /// seam, and either one reaching the wrong arm is exactly the drift this
    /// pin exists to catch.
    ///
    /// **The scope of the biconditional, stated since RP2.4.** Two of the
    /// gate's four chain links have no census twin *by design* — the exclusion
    /// ([`PropsPolicy::echo_excluded`]) and the display floor
    /// ([`PropsPolicy::under_display_floor`]) drop a value assert the census
    /// still records as a row, because a census row must say what the two
    /// engines really rendered and the *disposition* (`echo-row` /
    /// `under-floor`, `props_norm::claim_value`) is what says who claims it.
    /// So this table's inputs stay outside both: `Foo` has no echo row, and
    /// every numeric case below is deliberately far outside the `2e-4` floor —
    /// including one placed just above it, which is what keeps the floor's
    /// refusal in this pin instead of only in `props_policy_tests`.
    #[test]
    fn the_two_walks_agree_on_every_input() {
        let cases: &[WalkCase] = &[
            (
                "identical",
                &[("A", "1"), ("B", "2")],
                &[("A", "1"), ("B", "2")],
                false,
            ),
            (
                "value mismatch",
                &[("A", "1"), ("B", "9")],
                &[("A", "1"), ("B", "2")],
                true,
            ),
            (
                "trailing allowlisted extra",
                &[("A", "1"), ("NewTrail", "9")],
                &[("A", "1")],
                false,
            ),
            (
                "inserted allowlisted extra",
                &[("A", "1"), ("NewMid", "9"), ("B", "2")],
                &[("A", "1"), ("B", "2")],
                false,
            ),
            (
                "allowlisted, present in oracle, value differs",
                &[("A", "1"), ("NewTrail", "9")],
                &[("A", "1"), ("NewTrail", "7")],
                true,
            ),
            (
                "non-allowlisted extra",
                &[("A", "1"), ("B", "2"), ("Bogus", "9")],
                &[("A", "1"), ("B", "2")],
                true,
            ),
            (
                "missing prop",
                &[("A", "1")],
                &[("A", "1"), ("B", "2")],
                true,
            ),
            (
                "pure re-ordering",
                &[("B", "2"), ("A", "1")],
                &[("A", "1"), ("B", "2")],
                true,
            ),
            (
                "case-insensitive names still match",
                &[("a", "1"), ("b", "2")],
                &[("A", "1"), ("B", "2")],
                false,
            ),
            // 4.998e-4 relative — 2.5x OVER the RP2.4 display floor, so both
            // walks must still see a divergence on BOTH channels. Placed here
            // deliberately close to the floor: a floor that crept upward, or
            // one that leaked onto capi, reds this row.
            (
                "numeric gap just above the display floor",
                &[("A", "1.0005")],
                &[("A", "1")],
                true,
            ),
            ("both empty", &[], &[], false),
        ];
        for channel in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            // Matched policies: whatever the gate applies to a cell, the census
            // twin applies too. (The census's own PLAIN mode disarms the value
            // half — that asymmetry is the knob's, deliberately outside this
            // biconditional, and `Foo` has no skip or normalization row anyway.)
            let policy = PropsPolicy::for_channel(channel);
            for (name, rust, oracle, gate_panics) in cases {
                let name = format!("{name} [{}]", channel.tag());
                let a = props(rust);
                let o = props(oracle);
                let gate = std::panic::catch_unwind(|| {
                    compare_prop_lists(
                        "Foo.x", "Foo", &a, &o, TEST_ALLOW, policy, false, 1e-9, 1e-9, "self",
                    );
                });
                assert_eq!(
                    gate.is_err(),
                    *gate_panics,
                    "{name}: the gate walk's verdict changed — update the table only \
                     with a deliberate policy change"
                );
                let mut rows = Vec::new();
                let unaligned = collect_element_divergences(
                    "Foo.x",
                    "Foo",
                    &props(rust),
                    &props(oracle),
                    TEST_ALLOW,
                    policy,
                    false,
                    1e-9,
                    1e-9,
                    &mut rows,
                );
                let census_flags = !rows.is_empty() || unaligned > 0;
                assert_eq!(
                    census_flags,
                    gate.is_err(),
                    "{name}: the two walks disagree — gate panicked = {}, census rows = {rows:?}, \
                     unaligned = {unaligned}",
                    gate.is_err()
                );
            }
        }
    }
}

/// A PC element's state variables (oracle `AllVariableNames`/`AllVariableValues`
/// — the live f64 state; names travel along for diagnostics only).
#[derive(Debug, Deserialize)]
pub struct VariablesCap {
    pub name: String,
    pub var_names: Vec<String>,
    pub values: Vec<f64>,
}

/// Compare a PC element's state variables (`Dss::element_variables`).
///
/// `skip` is the ledger hook (`R4133_PROPS_PLAN.md` RP3.10): it is asked about
/// every variable, by the name the **port** gives that index, and returns `true`
/// when this (case, channel) does not compare that one. The granularity is
/// per-variable on purpose — the only alternative to it, dropping the element
/// from the manifest's `variables` list, would mask the deck's whole dynamics
/// surface (22 state variables) to excuse the 3-4 an engine fix moves. Pass
/// `&|_| false` where no ledger is in play.
///
/// Two guarantees hold whatever `skip` answers:
///
/// * the **count** assertion is unconditional — an excluded variable is still a
///   variable the port must expose, so a dropped, added or reordered surface
///   still reds the gate;
/// * an index `skip` drops must carry the SAME name on both engines
///   (ASCII-case-insensitive). The mask selects by name, so a rename on either
///   side would otherwise slide it silently onto a clean channel; the assertion
///   is what makes an exclusion mean the variable it names.
pub fn compare_variables(
    dss: &mut Dss,
    exp: &VariablesCap,
    tol: &Tolerances,
    ctx: &str,
    skip: &dyn Fn(&str) -> bool,
) {
    let act = dss
        .element_variables(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no element {} (variables)", exp.name));
    assert_eq!(
        act.len(),
        exp.values.len(),
        "{ctx}: {} variable count differs (oracle names: {:?})",
        exp.name,
        exp.var_names
    );
    let names = dss
        .element_variable_names(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no element {} (variable names)", exp.name));
    assert_eq!(
        names.len(),
        act.len(),
        "{ctx}: {} exposes {} variable value(s) but {} name(s)",
        exp.name,
        act.len(),
        names.len()
    );
    for (i, (a, e)) in act.iter().zip(&exp.values).enumerate() {
        let port_name = names[i].as_str();
        let oracle_name = exp.var_names.get(i).map(String::as_str).unwrap_or("?");
        if skip(port_name) {
            assert!(
                port_name.eq_ignore_ascii_case(oracle_name),
                "{ctx}: {} variable {} is excluded by name ({port_name:?}), but the \
                 oracle calls that index {oracle_name:?} — the exclusion would drop \
                 a different variable than the one it names",
                exp.name,
                i + 1
            );
            continue;
        }
        let allowed = tol.i_abs + tol.i_rel * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: {} variable {} ({}) differs: {a} vs {e} (|diff|={:.3e} > allowed {allowed:.3e})",
            exp.name,
            i + 1,
            oracle_name,
            (a - e).abs()
        );
    }
}

#[cfg(test)]
mod variables_exclusion_name_safety {
    use super::{VariablesCap, compare_variables};
    use dss_core::exec::Dss;

    /// A WindGen solved once — the element the RP3.10 `variables`
    /// exclusions are written against; it exposes the 22 WTG3 state
    /// variables in both lanes.
    fn solved() -> (Dss, Vec<f64>, Vec<String>) {
        let mut dss = Dss::new();
        for c in [
            "clear",
            "new circuit.nm basekv=0.69 phases=3 bus1=srcbus",
            "new line.l1 bus1=srcbus bus2=wbus phases=3 r1=0.005 x1=0.02 length=1",
            "new windgen.w1 bus1=wbus phases=3 kv=0.69 kW=1500 kva=1800 conn=wye \
             model=1 vss=1 pss=1 qss=0 vwind=12",
            "set voltagebases=[0.69]",
            "calcvoltagebases",
            "solve",
        ] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        let values = dss.element_variables("WindGen.w1").expect("variables");
        let names = dss.element_variable_names("WindGen.w1").expect("names");
        (dss, values, names)
    }

    /// The negative drive of the name-safety assert in
    /// [`super::compare_variables`] (RP3.10 audit settlement, finding AT-4):
    /// when the ledger excludes an index by the PORT's name for it, and the
    /// oracle calls that same index something else, the exclusion would drop
    /// a different variable than the one it names — that must abort the
    /// comparison, not slide onto a clean channel. Asserted here by driving
    /// the guard with a `var_names` list whose excluded index disagrees.
    #[test]
    #[should_panic(expected = "the exclusion would drop a different variable")]
    fn an_excluded_index_whose_oracle_name_differs_aborts() {
        let (mut dss, values, names) = solved();
        let mut oracle_names = names.clone();
        oracle_names[6] = "SomethingElse".to_string();
        let exp = VariablesCap {
            name: "WindGen.w1".to_string(),
            var_names: oracle_names,
            values,
        };
        compare_variables(
            &mut dss,
            &exp,
            &super::tol_for("micro_wtg3_dynamics"),
            "name-safety drive",
            &|n: &str| n.eq_ignore_ascii_case("Qgen"),
        );
    }

    /// The same drive with the names in agreement passes — the guard is
    /// about a MISMATCH, not about excluding at all.
    #[test]
    fn an_excluded_index_whose_names_agree_is_dropped_quietly() {
        let (mut dss, values, names) = solved();
        let mut corrupted = values.clone();
        corrupted[6] = 1.0e9; // would red loudly if it were still compared
        let exp = VariablesCap {
            name: "WindGen.w1".to_string(),
            var_names: names,
            values: corrupted,
        };
        compare_variables(
            &mut dss,
            &exp,
            &super::tol_for("micro_wtg3_dynamics"),
            "name-safety drive",
            &|n: &str| n.eq_ignore_ascii_case("Qgen"),
        );
    }
}

/// One documented per-rev event-log normalization (§1.3-3): a literal substring
/// `find` replaced by `to` on **both** engines' lines before comparison. `note`
/// cites the `tests/TOLERANCE_NOTES.md` §"r4133 event-log masks" entry that
/// justifies it. A mask only folds a COSMETIC text delta — it never drops or
/// reorders a line, so the compared sequence (order / hours / devices / actions)
/// is never relaxed.
pub struct EventLogMask {
    /// Human tag → TOLERANCE_NOTES.md justification.
    pub note: &'static str,
    /// Literal substring to normalize.
    pub find: &'static str,
    /// Its replacement (applied to both the Rust and oracle line).
    pub to: &'static str,
}

/// Per-oracle-spec event-log masks (§1.3-3, created by WP-U2.2). Keyed by the
/// case's `oracle` manifest spec; applied to both the Rust and oracle line
/// before the numeric-skeleton comparison in [`compare_eventlog`].
///
/// **Ships with NO r4133 rows.** The WP-U2.2 Recloser per-phase rewrite
/// reproduces the r4133 event-log wording byte-for-byte (proven against the
/// oracle's `export eventlog` — `Phase %d opened on %s (…trip) …` etc.), so no
/// recloser mask is required — the empty `r4133` table is the *proof* the port
/// is exact. The mechanism exists so WP-U2.3 (Relay) and later revs can add
/// documented rows here without restructuring (union-mergeable, one row per
/// line). Rules: cite the delta row / TOLERANCE_NOTES entry in `note`; never add
/// a row that would drop or reorder a line (that is a real divergence, not a
/// format delta — fix the port instead).
const EVENTLOG_MASKS: &[(&str, &[EventLogMask])] = &[("r4133", &[])];

/// Apply the [`EVENTLOG_MASKS`] rows for `oracle_spec` to a single event-log
/// line (no-op when `oracle_spec` is `None`/the default oracle, or the rev has
/// no rows).
fn apply_eventlog_masks(
    masks: &[(&str, &[EventLogMask])],
    oracle_spec: Option<&str>,
    line: &str,
) -> String {
    let Some(spec) = oracle_spec else {
        return line.to_string();
    };
    let mut s = line.to_string();
    for (rev, rules) in masks {
        if rev.eq_ignore_ascii_case(spec) {
            for m in *rules {
                if !m.find.is_empty() {
                    s = s.replace(m.find, m.to);
                }
            }
        }
    }
    s
}

/// Compare the event log line-for-line (normalized numeric skeleton at 1e-6
/// rel — the exact policy `golden_protection.rs` pins trip/reclose
/// sequences with). The log is cumulative, so a per-step compare pins *when*
/// each control action happened, not just the final set. `oracle_spec` selects
/// the §1.3-3 per-rev [`EVENTLOG_MASKS`] applied to both sides (`None` = the
/// pinned 0.14.5 oracle, no masking).
pub fn compare_eventlog(dss: &Dss, exp: &[String], oracle_spec: Option<&str>, ctx: &str) {
    compare_eventlog_masked(dss, exp, oracle_spec, EVENTLOG_MASKS, ctx);
}

/// The masked comparison core (factored out so the self-test can inject a table).
fn compare_eventlog_masked(
    dss: &Dss,
    exp: &[String],
    oracle_spec: Option<&str>,
    masks: &[(&str, &[EventLogMask])],
    ctx: &str,
) {
    let log = dss.event_log();
    // Masks never drop lines, so the length check stays a pure sequence-length
    // gate (order/count never relaxed).
    assert_eq!(
        log.len(),
        exp.len(),
        "{ctx}: event log length differs:\n  actual:\n    {}\n  oracle:\n    {}",
        log.join("\n    "),
        exp.join("\n    ")
    );
    for (i, (a, e)) in log.iter().zip(exp).enumerate() {
        let am = apply_eventlog_masks(masks, oracle_spec, a);
        let em = apply_eventlog_masks(masks, oracle_spec, e);
        assert_value_matches_tol(&am, &em, 1e-6, 1e-9, &format!("{ctx}: event-log line {i}"));
    }
}

/// Compare the pending control-action queue (oracle `CtrlQueue.Queue` rows vs
/// `Dss::control_queue_rows`). Rows are trimmed and compared as numeric
/// skeletons: Pascal's `%.9g` time rendering and its trailing space are not
/// load-bearing, the handle/hour/sec/code/device content is. Meaningful only in
/// time/dynamics modes where future-scheduled actions (e.g. recloser reclose
/// shots) survive the solve; a drained queue compares as empty = empty.
pub fn compare_ctrlqueue(dss: &Dss, exp: &[String], ctx: &str) {
    let rows = dss.control_queue_rows();
    assert_eq!(
        rows.len(),
        exp.len(),
        "{ctx}: control queue length differs:\n  actual:\n    {}\n  oracle:\n    {}",
        rows.join("\n    "),
        exp.join("\n    ")
    );
    for (i, (a, e)) in rows.iter().zip(exp).enumerate() {
        assert_value_matches_tol(
            a.trim().to_lowercase().as_str(),
            e.trim().to_lowercase().as_str(),
            1e-6,
            1e-9,
            &format!("{ctx}: control-queue row {i}"),
        );
    }
}

/// Compare per-step discrete control state EXACTLY: transformer taps (1e-12 rel
/// — see the feeder-gate note), RegControl tap numbers and capacitor states by
/// value. The three maps are the oracle capture keyed by element name.
pub fn compare_discrete(
    dss: &Dss,
    transformers: &BTreeMap<String, Vec<f64>>,
    regcontrols: &BTreeMap<String, i32>,
    capacitors: &BTreeMap<String, Vec<i32>>,
    ctx: &str,
) {
    let taps = dss.transformer_taps();
    assert_eq!(
        taps.len(),
        transformers.len(),
        "{ctx}: transformer count differs"
    );
    for (name, tr_taps) in &taps {
        let exp = transformers
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: oracle has no transformer {name}"));
        assert_eq!(
            tr_taps.len(),
            exp.len(),
            "{ctx}: transformer {name} winding count differs"
        );
        for (w, (a, e)) in tr_taps.iter().zip(exp).enumerate() {
            assert!(
                (a - e).abs() <= 1e-12 * e.abs().max(1.0),
                "{ctx}: transformer {name} winding {} tap: {a} vs {e}",
                w + 1
            );
        }
    }
    let tap_numbers = dss.regcontrol_tap_numbers();
    assert_eq!(
        tap_numbers.len(),
        regcontrols.len(),
        "{ctx}: regcontrol count differs"
    );
    for (name, num) in &tap_numbers {
        let exp = regcontrols
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: oracle has no regcontrol {name}"));
        assert_eq!(num, exp, "{ctx}: regcontrol {name} tap number differs");
    }
    let states = dss.capacitor_states();
    assert_eq!(
        states.len(),
        capacitors.len(),
        "{ctx}: capacitor count differs"
    );
    for (name, cap_states) in &states {
        let exp = capacitors
            .get(name)
            .unwrap_or_else(|| panic!("{ctx}: oracle has no capacitor {name}"));
        assert_eq!(cap_states, exp, "{ctx}: capacitor {name} states differ");
    }
}

/// A monitor's data-channel header, sample count, and per-channel sample arrays
/// (oracle capture; `header` is the data channels only — no leading hour/sec).
#[derive(Debug, Deserialize)]
pub struct MonitorCap {
    pub name: String,
    pub header: Vec<String>,
    pub sample_count: i32,
    pub channels: Vec<Vec<f64>>,
    /// 0-based channel indices to skip (e.g. the mode-5 wall-clock timing
    /// channels, which the port records as 0 — see `golden_metering_monitors.rs`). Empty
    /// for the live gate, which only captures deterministic monitor modes.
    #[serde(default)]
    pub skip_channels: Vec<usize>,
}

/// An EnergyMeter's register names/values and zone branch/end/PCE counts.
/// `branches`/`ends`/`pce` are the zone member name lists; when non-empty (the
/// live gate captures them) membership is compared as a case-insensitive set,
/// strengthening the bare count check. `golden_metering_monitors.rs`'s meter golden leaves
/// them empty (it pins micro-zone membership separately, ordered).
#[derive(Debug, Deserialize)]
pub struct MeterCap {
    pub name: String,
    pub register_names: Vec<String>,
    pub register_values: Vec<f64>,
    pub n_branches: usize,
    pub n_ends: usize,
    pub n_pce: usize,
    #[serde(default)]
    pub branches: Vec<String>,
    #[serde(default)]
    pub ends: Vec<String>,
    #[serde(default)]
    pub pce: Vec<String>,
}

/// Compare a monitor's header (data channels, exact), sample count (exact), and
/// every channel's sample array (`tol.i_rel`/`i_abs` on the f32 samples — the
/// same policy `golden_metering_monitors.rs` uses). Channels listed in `exp.skip_channels`
/// (the mode-5 wall-clock timings) are skipped; the live gate leaves it empty
/// (it captures only deterministic modes, so every channel is compared).
pub fn compare_monitor(dss: &Dss, exp: &MonitorCap, tol: &Tolerances, ctx: &str) {
    let view = dss
        .monitor_view(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no monitor {}", exp.name));
    // Our header leads with the hour / t(sec) columns; the oracle Header is the
    // data channels only (`Channel(i)` already skips the time slots).
    //
    // Normalize away the official-EPRI (Oddie) monitor CSV rendering artifact
    // before comparing: Delphi `TMonitorObj` writes the header row with a leading
    // space after each comma separator (`' VAngle1'`) and a trailing comma, so
    // `Monitors.Header` comes back as `['V1',' VAngle1',…,'']` (leading-space
    // columns + a trailing empty). Normalize **only the expected side** — the
    // artifact originates there. The Rust `monitor_view().header` is built
    // structurally (`header.push("V1")` / `push(format!("P{i}W{j}"))`, never a
    // CSV round-trip; `monitor/header.rs`), so it is clean by construction and
    // stays strict, keeping the golden monitor-header check (dss_capi golden,
    // also clean — this comparator is shared with `golden_metering_monitors.rs`)
    // exactly as tight as before combo restore: a genuine Rust-side header defect
    // (leading space / phantom trailing column) still fails here. The channel
    // COUNT is also asserted independently below and every channel's samples are
    // compared numerically. The product-side header normalization (what the Rust
    // engine *emits*) is WP-U1.5 E1.
    let norm = |cols: &[String]| -> Vec<String> {
        let mut v: Vec<String> = cols.iter().map(|s| s.trim().to_string()).collect();
        while v.last().is_some_and(String::is_empty) {
            v.pop();
        }
        v
    };
    let exp_header = norm(&exp.header);
    assert_eq!(
        &view.header[2..],
        exp_header.as_slice(),
        "{ctx}: monitor {} header differs",
        exp.name
    );
    assert_eq!(
        view.sample_count, exp.sample_count,
        "{ctx}: monitor {} sample count differs",
        exp.name
    );
    assert_eq!(
        view.channels.len(),
        exp.channels.len(),
        "{ctx}: monitor {} channel count differs",
        exp.name
    );
    for (ch, (act, e)) in view.channels.iter().zip(&exp.channels).enumerate() {
        if exp.skip_channels.contains(&ch) {
            continue;
        }
        // The capture as its reader means it: identical to it except for the
        // client-side unflushed-stream `[0.0]` placeholder, which both oracle
        // channels emit and both lanes drop (`lane::expected_monitor_channel`).
        let e = &lane::expected_monitor_channel(view.flushed_records, e);
        assert_eq!(
            act.len(),
            e.len(),
            "{ctx}: monitor {} channel {} sample count differs",
            exp.name,
            ch + 1
        );
        // Polar ANGLE channel (`VAngle<n>`/`IAngle<n>`, VIpolar modes): the angle
        // of a near-cancellation magnitude carries the magnitude floor amplified
        // by 1/|mag| — see the band derivation at the sample loop below.
        let angle_mag_channel = exp_header
            .get(ch)
            .is_some_and(|h| polar_angle_channel(h))
            .then(|| exp.channels.get(ch.wrapping_sub(1)))
            .flatten();
        for (k, (a, ev)) in act.iter().zip(e).enumerate() {
            let a = *a as f64;
            // Monitor channels are **f32 recordings** (Pascal `MonBuffer:
            // pSingleArray`), so the honest comparison floor is one f32 ULP of
            // the recorded value (tests/TOLERANCE_NOTES.md §monitor-f32-floor):
            // two f64 trajectories agreeing far inside the f64 floors still
            // record one-ulp-apart f32 samples whenever they straddle an f32
            // rounding midpoint. Proven by decomposition on the InductionMachine
            // r4133 twin (indmach_dyn): at the straddle sample the live f64
            // |V(B4.2)| differed by 2.8e-11 rel (350x inside the feeder v_rel
            // floor) yet the stored f32s differ by a full ulp (9.77e-4 V at
            // 8.9 kV); over 490k samples 99.25% of diffs were exactly 1 ulp with
            // no growth in time. A real defect is >= 2 ulps or visible in the
            // f64 surfaces (node V / element currents / variables), which keep
            // their full tier floors — this floor widens nothing above the
            // information content of the f32 data itself.
            let mut allowed = (tol.i_abs + tol.i_rel * ev.abs()).max(ulp_f32(*ev));
            // Angle-of-near-zero-magnitude: the polar angle is atan2 of a
            // near-cancellation pair, so the already-accepted magnitude floor
            // maps to `rad2deg * (i_abs + i_rel*|mag|)/|mag|` degrees — the
            // exact angular image of the magnitude band (same construction as
            // the voltage-scaled power floor, tests/TOLERANCE_NOTES.md). At
            // healthy magnitudes the image is far tighter than the base band
            // (1.7e-5 deg at 50 A); it only opens where the magnitude — still
            // tightly compared on its own channel — carries no angular
            // information (indmach_dyn: 0.10-0.24 A residual phase-3 current
            // during the SLG fault, dI=1.3e-7 A -> 3-6e-5 deg).
            if let Some(mags) = angle_mag_channel {
                let mag = mags.get(k).copied().unwrap_or(0.0).abs();
                if mag > 0.0 {
                    allowed = allowed.max(57.29577951308232 * (tol.i_abs + tol.i_rel * mag) / mag);
                }
            }
            assert!(
                (a - ev).abs() <= allowed,
                "{ctx}: monitor {} channel {} ({}) sample {k} differs: {a} vs {ev} \
                 (|diff|={:.3e} > allowed {allowed:.3e})",
                exp.name,
                ch + 1,
                exp.header.get(ch).map(String::as_str).unwrap_or("?"),
                (a - ev).abs()
            );
        }
    }
}

/// One f32 ULP of `x` — the spacing of the f32 grid the monitor recorded `x`
/// on (`compare_monitor`; TOLERANCE_NOTES §monitor-f32-floor). 0 for
/// non-finite input (falls back to the base band).
fn ulp_f32(x: f64) -> f64 {
    let ax = x.abs() as f32;
    if !ax.is_finite() {
        return 0.0;
    }
    (f32::from_bits(ax.to_bits() + 1) - ax) as f64
}

/// Is this monitor header column a VIpolar ANGLE channel (`VAngle<n>` /
/// `IAngle<n>`)? Deliberately exact — mode-3 state-variable names like
/// `Theta (deg)` or `Angle (deg)` must NOT match (their magnitudes live in
/// unrelated channels).
fn polar_angle_channel(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('V').or_else(|| name.strip_prefix('I')) else {
        return false;
    };
    let Some(digits) = rest.strip_prefix("Angle") else {
        return false;
    };
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Compare an EnergyMeter's registers (names exact, values within the
/// energy-accumulation policy `tol.energy_rel`/`energy_abs` — PORTING_PLAN §4) and
/// zone branch/end/PCE counts (exact).
pub fn compare_meter(dss: &Dss, exp: &MeterCap, tol: &Tolerances, ctx: &str) {
    let regs = dss
        .meter_registers(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no meter {}", exp.name));
    assert_eq!(
        regs.len(),
        exp.register_names.len(),
        "{ctx}: meter {} register count differs",
        exp.name
    );
    for (i, (name, value)) in regs.iter().enumerate() {
        assert_eq!(
            name, &exp.register_names[i],
            "{ctx}: meter {} register {i} name differs",
            exp.name
        );
        let e = exp.register_values[i];
        let allowed = tol.energy_abs + tol.energy_rel * e.abs();
        assert!(
            (value - e).abs() <= allowed,
            "{ctx}: meter {} register {name} differs: {value} vs {e} \
             (|diff|={:.3e} > allowed {allowed:.3e})",
            exp.name,
            (value - e).abs()
        );
    }
    let zone = dss
        .meter_zone(&exp.name)
        .unwrap_or_else(|| panic!("{ctx}: no meter zone {}", exp.name));
    assert_eq!(
        zone.all_branches_in_zone.len(),
        exp.n_branches,
        "{ctx}: meter {} branch count differs",
        exp.name
    );
    assert_eq!(
        zone.all_end_elements.len(),
        exp.n_ends,
        "{ctx}: meter {} end count differs",
        exp.name
    );
    assert_eq!(
        zone.zone_pce.len(),
        exp.n_pce,
        "{ctx}: meter {} PCE count differs",
        exp.name
    );
    // Membership (case-insensitive set) when the capture carries the lists — a
    // strictly stronger check than the counts above, order-independent so a
    // different zone-walk order is not a spurious failure.
    let set = |v: &[String]| -> BTreeSet<String> { v.iter().map(|s| s.to_lowercase()).collect() };
    let cmp_members = |act: &[String], e: &[String], what: &str| {
        if e.is_empty() {
            return;
        }
        let (a, b) = (set(act), set(e));
        assert!(
            a == b,
            "{ctx}: meter {} {what} membership differs (Rust∖oracle={:?}, oracle∖Rust={:?})",
            exp.name,
            a.difference(&b).collect::<Vec<_>>(),
            b.difference(&a).collect::<Vec<_>>(),
        );
    };
    cmp_members(&zone.all_branches_in_zone, &exp.branches, "branch");
    cmp_members(&zone.all_end_elements, &exp.ends, "end");
    cmp_members(&zone.zone_pce, &exp.pce, "PCE");
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE_PLAN.md WP-G1 sub-step G1.4a: the per-bus voltage surface.
//
// The bus flavours of the solved node voltages both oracles publish —
// `Bus.puVoltages`, `Bus.VMagAngle`, `Bus.puVMagAngle` and the circuit-level
// `Circuit.AllBusVmagPu` (the fastdss harness dumps the whole `IBus`/`ICircuit`
// `_columns` set, `origin/fastdss` `tests/save_outputs.py:348,351`). Captured by
// `tools/oracle/oracle_server.py::capture_all_buses` (capi channel) and
// `crates/dss-epri/src/capture.rs::capture_all_buses` (r4133 channel); read back
// from the engine through `Dss::all_bus_voltages` / `Dss::all_bus_vmag_pu`
// (`crates/dss-core/src/exec/view.rs`).
//
// **Three ordering conventions meet here and must never be mixed.**
//   1. ascending node NUMBER, per bus — the three `BusCap` value arrays: the
//      `repeat NodeIdx := FindIdx(jj); inc(jj) until NodeIdx > 0` walk both
//      engines run (`CAPI/CAPI_Alt.pas:2251-2280` == r4133
//      `Version8/Source/DDLL/DBus.pas:399-430`);
//   2. bus-list order x the bus's INTERNAL node index — `all_bus_vmag_pu`
//      (`CAPI_Circuit.pas:521-548` == `DCircuit.pas:481-500`);
//   3. `YNodeOrder` — the node-voltage channel's own permutation, gated
//      separately in `corpus_gate/runner.rs`.
// `BusVoltageView::nodes` keeps the bus's INSERTION order, a fourth one; both
// oracles report `Bus.Nodes` ascending (`CAPI_Alt.pas:2143-2163` ==
// `DBus.pas:319-345`), so `compare_bus` sorts before it compares.
//
// **Tolerances: this surface adds no new constant.** Every band is the exact
// image of the already-calibrated node-voltage band `v_abs + v_rel*|V|` (the
// `assert_complex_close` in `runner.rs`, over the same `Solution.NodeV` these
// quantities are read from) under an engine-identical exact transformation.
// Derivations: tests/TOLERANCE_NOTES.md §"Bus voltage surface (GOLDEN_REBASE
// G1.4a)".
// ---------------------------------------------------------------------------

/// One bus's captured voltage surface — the `compare_bus` wire shape both
/// transports emit, GOLDEN_REBASE_PLAN.md WP-G1 G1.4a. Parity target
/// `origin/fastdss` `dss/IBus.py:19-53` `_columns`.
///
/// All three value arrays are `2 * nodes.len()` doubles (interleaved re/im for
/// `pu_voltages`, `(magnitude, angle°)` pairs for the two polar ones) and share
/// ONE ordering convention: **ascending node number** (see the module block
/// above). `nodes` is ascending too, on both channels.
///
/// The six short-circuit arrays below (G1.5, `#[serde(default)]` so a G1.4a
/// capture still deserializes) are the OTHER convention: they are indexed by
/// the bus's **internal (insertion) node index**, because both engines read
/// `GetRef(i)` / `Zsc.GetElement(i, j)` straight off `TDSSBus`
/// (`CAPI_Alt.pas:2202-2365` == `DBus.pas:351-518`). They are compared by
/// [`compare_bus_short_circuit`], which therefore must NOT sort — see its doc
/// block for how that order is itself pinned.
///
/// The divergent bus quantities (`SeqVoltages`/`CplxSeqVoltages`, `VLL`/`puVLL`)
/// are G1.4c's and are deliberately absent (coordinator decision D8); they
/// arrive as further `#[serde(default)]` fields, so an older capture stays
/// readable.
#[derive(Debug, Deserialize)]
pub struct BusCap {
    /// `Bus.Name`, in `Circuit.AllBusNames` (= `BusList`) order.
    pub name: String,
    /// `Bus.kVBase` in kV; `<= 0` = not set.
    pub kv_base: f64,
    /// `Bus.Nodes` — the bus's node NUMBERS, ascending
    /// (`CAPI_Alt.pas:2143-2163` == r4133 `DBus.pas:319-345`).
    pub nodes: Vec<i32>,
    /// `NodeV / BaseFactor`, interleaved (re, im).
    pub pu_voltages: Vec<f64>,
    /// Interleaved (magnitude in V, angle in degrees)
    /// (`CAPI_Alt.pas:2573-2597` == r4133 `DBus.pas:659-689`).
    pub vmag_angle: Vec<f64>,
    /// Interleaved (magnitude in per unit, angle in degrees) — only the
    /// magnitude is divided by `BaseFactor`
    /// (`CAPI_Alt.pas:2540-2571` == r4133 `DBus.pas:690-723`).
    pub pu_vmag_angle: Vec<f64>,
    /// `Bus.Zsc1` = `Zs − Zm`, ONE complex = 2 doubles, always — both engines
    /// write the 1-element array unconditionally (`CAPI_Alt.pas:2294-2303` ==
    /// r4133 `DBus.pas:461-474`), `cZERO` while `Zsc` is unassigned
    /// (`Common/Bus.pas:222-229`). G1.5.
    #[serde(default)]
    pub zsc1: Vec<f64>,
    /// `Bus.Zsc0` = `Zs + 2·Zm`, same shape and guard
    /// (`CAPI_Alt.pas:2283-2292` == `DBus.pas:476-489`, `Common/Bus.pas:215-220`).
    #[serde(default)]
    pub zsc0: Vec<f64>,
    /// `Bus.ZscMatrix` — `2*n*n` doubles, **row-major** (`i` outer, `j` inner:
    /// `CAPI_Alt.pas:2316-2330` == `DBus.pas:445-450`), or this channel's own
    /// not-run sentinel ([`sc_sentinel_len`]).
    #[serde(default)]
    pub zsc: Vec<f64>,
    /// `Bus.YscMatrix` = `Zsc⁻¹`, same shape and sentinel
    /// (`CAPI_Alt.pas:2336-2365` == `DBus.pas:491-518`).
    #[serde(default)]
    pub ysc: Vec<f64>,
    /// `Bus.Isc` — `BusCurrent`, `2*n` doubles
    /// (`CAPI_Alt.pas:2202-2224` == `DBus.pas:374-397`).
    #[serde(default)]
    pub isc: Vec<f64>,
    /// `Bus.Voc` — `VBus`, `2*n` doubles
    /// (`CAPI_Alt.pas:2227-2249` == `DBus.pas:351-372`).
    #[serde(default)]
    pub voc: Vec<f64>,
}

/// The `BaseFactor` both engines divide the per-unit bus quantities by:
/// `1000 * kVBase`, or `1.0` when the bus has no base (`CAPI_Alt.pas:2262-2265`
/// == `DBus.pas:413-414` == `CAPI_Circuit.pas:538-541` == `DCircuit.pas:493`).
/// Taken from the ORACLE's `kv_base`, which [`compare_bus`] has already pinned
/// exactly against the port's — so the two engines scale by the same bits and
/// the divide contributes no error of its own.
fn bus_base_factor(kv_base: f64) -> f64 {
    if kv_base > 0.0 { 1000.0 * kv_base } else { 1.0 }
}

/// Fold a degree difference into `(-180, 180]` — the range Pascal `ctopolardeg`
/// (`Shared/Ucomplex.pas`, via `arctan2`) reports angles in. A phasor sitting on
/// the ±180° seam flips sign between the engines on a 1-ulp difference in its
/// imaginary part, so a raw subtraction there reads ~360° for what is physically
/// the same angle.
///
/// *Dedup at merge (D7):* the element lane's G1.3a builds the same wrap-aware
/// compare as `polar_close`; keep ONE of the two when the lanes merge.
fn wrap_deg(d: f64) -> f64 {
    let r = d % 360.0;
    if r > 180.0 {
        r - 360.0
    } else if r <= -180.0 {
        r + 360.0
    } else {
        r
    }
}

/// The angular band a magnitude band implies — the **exact** image, in degrees.
///
/// The oracle publishes `(|V|, arg V)` of the same `Solution.NodeV[k]` the
/// node-voltage channel compares at `|dV| <= v_abs + v_rel*|V|`. The set of
/// phasors within `eps` of `V` subtends a half-angle `asin(eps/|V|)` about
/// `arg V` while `eps < |V|`, and the whole circle once `eps >= |V|`, so
///
/// ```text
/// allowed_deg = if eps >= |V| { 180 } else { degrees(asin(eps / |V|)) },
/// eps = v_abs + v_rel * |V|
/// ```
///
/// is the tightest band that cannot red on a voltage the node channel accepts.
/// `f64::to_degrees` multiplies by `180/PI = 57.29577951308232`, the same
/// full-precision constant [`compare_monitor`]'s polar-angle band uses; that
/// band is this one linearized (`asin x ≈ x`), which agrees to `<2e-3` relative
/// while `eps/|V| <= 0.1` — the whole healthy regime — but *under*-estimates the
/// image as `eps/|V| → 1`. Bus magnitudes legitimately reach the absolute floor
/// (unenergized buses; the `NEVTestCase` neutral-earth buses sit at ~2 V), so
/// the exact arcsine is used here rather than its linearization. Saturating at
/// 180° is not a free pass: the magnitude channel still pins `|V|` itself inside
/// `eps` on its own row, so a bus whose angle is unconstrained is exactly a bus
/// whose voltage is at or below the floor in both engines.
fn bus_angle_band_deg(mag_v: f64, tol: &Tolerances) -> f64 {
    if mag_v > 0.0 {
        let ratio = (tol.v_abs + tol.v_rel * mag_v) / mag_v;
        if ratio >= 1.0 {
            180.0
        } else {
            ratio.asin().to_degrees()
        }
    } else {
        // A zero (or non-finite) magnitude carries no angular information at
        // all; the whole wrapped range is the honest band. `NaN > 0.0` is
        // false, so it lands here too.
        180.0
    }
}

/// Wrap-aware polar-angle compare against [`bus_angle_band_deg`]`(mag_v)`.
/// `mag_v` is the SAME sample's magnitude **in volts** (the pu channel multiplies
/// its per-unit magnitude back by `BaseFactor`), so both polar flavours share one
/// physical band.
fn bus_polar_close(actual_deg: f64, expected_deg: f64, mag_v: f64, tol: &Tolerances, what: &str) {
    let d = wrap_deg(actual_deg - expected_deg);
    let allowed = bus_angle_band_deg(mag_v, tol);
    assert!(
        d.abs() <= allowed,
        "{what}: angle differs: {actual_deg} vs {expected_deg} deg \
         (wrapped |diff| = {:.3e} > allowed {allowed:.3e} at |V| = {mag_v:e} V)",
        d.abs()
    );
}

/// Compare every bus's captured voltage surface against the engine's, in
/// `BusList` order (GOLDEN_REBASE_PLAN.md G1.4a).
///
/// Structure per bus, strongest first:
/// * the bus **name sequence** — the port's `BusList` order against the oracle's
///   `AllBusNames`, case-insensitively. An independent pin: `YNodeOrder` (gated
///   in `runner.rs`) is a different permutation and never witnesses a bus that
///   carries no node.
/// * `nodes` — exact integer equality after sorting the port's insertion-order
///   list (both oracles publish ascending node numbers; see the module block).
/// * `kv_base` — compared **exactly**. Both engines derive it by the same
///   `SetVoltageBases` walk (`Solution.pas:1103` == r4133 `:2541`,
///   `kVBase := NearestBasekV/SQRT3`) from the same legal-base list, so a
///   disagreement is a finding, not a floor. NOTE: the base *search* scale is a
///   Stage-F lane row (`compat::kv_base_search_scale`, truncated `0.001732` in
///   the parity lane vs `SQRT3/1000` in the default lane) that can only pick a
///   different legal base when the estimate sits within 2.93e-5 of a tie between
///   two adjacent bases — a lane-dependent failure here is that knife edge, not
///   a floor to widen.
/// * the three value arrays — images of the node-voltage band; see
///   [`bus_angle_band_deg`] and tests/TOLERANCE_NOTES.md.
///
/// `voltages_excluded` is set by the caller when the divergence ledger already
/// excludes this case/channel's `voltages` field DECK-WIDE
/// (`LedgerView::bus_arrays_suppressed(step)` — a `voltages` scope that names a
/// node subset excludes fewer nodes than the arrays cover and suppresses
/// nothing here). The bus quantities are exact
/// images of exactly those node voltages, so re-comparing them would re-raise a
/// divergence that has already been triaged and pinned — one structural rule
/// instead of a ledger row per case. It suppresses ONLY the three continuous
/// arrays: the bus count, the name sequence, `nodes`, `kv_base` and every array
/// length stay compared, so the surface's own content (the orderings, the bus
/// identity, the voltage bases) is never unwitnessed.
pub fn compare_bus(
    dss: &Dss,
    exp: &[BusCap],
    tol: &Tolerances,
    voltages_excluded: bool,
    ctx: &str,
) {
    let views = dss.all_bus_voltages();
    assert_eq!(
        views.len(),
        exp.len(),
        "{ctx}: bus count differs ({} vs {})",
        views.len(),
        exp.len()
    );
    for (i, (v, e)) in views.iter().zip(exp).enumerate() {
        assert!(
            v.name.eq_ignore_ascii_case(&e.name),
            "{ctx}: bus {i} name differs: {} vs {}",
            v.name,
            e.name
        );
        let mut nodes = v.nodes.clone();
        nodes.sort_unstable();
        assert_eq!(
            nodes, e.nodes,
            "{ctx}: bus {} nodes differ (port insertion order {:?})",
            e.name, v.nodes
        );
        assert_eq!(v.kv_base, e.kv_base, "{ctx}: bus {} kVBase differs", e.name);
        let n = nodes.len();
        for (arr, what) in [
            (&e.pu_voltages, "puVoltages"),
            (&e.vmag_angle, "VMagAngle"),
            (&e.pu_vmag_angle, "puVMagAngle"),
        ] {
            assert_eq!(
                arr.len(),
                2 * n,
                "{ctx}: bus {} {what} length {} is not 2*{n}",
                e.name,
                arr.len()
            );
        }
        if voltages_excluded {
            continue;
        }
        let base_factor = bus_base_factor(e.kv_base);
        // `puVoltages` = `NodeV / BaseFactor` componentwise
        // (`CAPI_Alt.pas:2277-2280` == `DBus.pas:423`, `cdivreal`): the
        // node-voltage band divided by the same exact constant.
        let actual: Vec<f64> = v.pu_voltages.iter().flat_map(|c| [c.re, c.im]).collect();
        assert_complex_close(
            &actual,
            &e.pu_voltages,
            tol.v_rel,
            tol.v_abs / base_factor,
            &format!("{ctx}: bus {} puVoltages", e.name),
        );
        for (k, node) in nodes.iter().enumerate() {
            // `VMagAngle`: magnitude in volts — the reverse triangle inequality
            // maps the node-voltage band onto it unchanged.
            let (am, aa) = v.vmag_angle[k];
            let (em, ea) = (e.vmag_angle[2 * k], e.vmag_angle[2 * k + 1]);
            let allowed = tol.v_abs + tol.v_rel * em.abs();
            assert!(
                (am - em).abs() <= allowed,
                "{ctx}: bus {} node {node} VMagAngle magnitude differs: {am} vs {em} \
                 (|diff| = {:.3e} > allowed {allowed:.3e})",
                e.name,
                (am - em).abs()
            );
            bus_polar_close(
                aa,
                ea,
                em.abs(),
                tol,
                &format!("{ctx}: bus {} node {node} VMagAngle", e.name),
            );
            // `puVMagAngle`: the same magnitude over `BaseFactor`, same angle.
            let (apm, apa) = v.pu_vmag_angle[k];
            let (epm, epa) = (e.pu_vmag_angle[2 * k], e.pu_vmag_angle[2 * k + 1]);
            let allowed = tol.v_abs / base_factor + tol.v_rel * epm.abs();
            assert!(
                (apm - epm).abs() <= allowed,
                "{ctx}: bus {} node {node} puVMagAngle magnitude differs: {apm} vs {epm} \
                 (|diff| = {:.3e} > allowed {allowed:.3e})",
                e.name,
                (apm - epm).abs()
            );
            bus_polar_close(
                apa,
                epa,
                epm.abs() * base_factor,
                tol,
                &format!("{ctx}: bus {} node {node} puVMagAngle", e.name),
            );
        }
    }
}

/// Compare `Circuit.AllBusVmagPu` — every NODE's per-unit voltage magnitude in
/// **convention 2** (bus-list order × the bus's internal node index, i.e. the
/// `AllNodeNames` permutation), `CAPI_Circuit.pas:521-548` ==
/// `DCircuit.pas:481-500`.
///
/// The band is per-entry `v_abs / BaseFactor(bus) + v_rel * |expected|`, the
/// image of the node-voltage band under that bus's own scaling. The per-entry
/// `BaseFactor` is rebuilt from the port's bus list — the same walk the engine
/// accessor uses, whose per-bus `kv_base` [`compare_bus`] pins exactly against
/// the oracle in the same block; the length identity
/// `Σ nodes == len(AllBusVmagPu)` is asserted here, so the per-bus walk
/// (convention 1) and the circuit walk (convention 2) cannot drift apart
/// silently.
///
/// `voltages_excluded`: see [`compare_bus`] — the oracle length compare and the
/// port-internal length identity both still run; only the per-entry values are
/// dropped.
pub fn compare_all_bus_vmag_pu(
    dss: &Dss,
    exp: &[f64],
    tol: &Tolerances,
    voltages_excluded: bool,
    ctx: &str,
) {
    let actual = dss.all_bus_vmag_pu();
    assert_eq!(
        actual.len(),
        exp.len(),
        "{ctx}: AllBusVmagPu length differs ({} vs {})",
        actual.len(),
        exp.len()
    );
    // Convention 2 walks each bus's nodes by INTERNAL index, so the label below
    // is the insertion-order node number — deliberately not the sorted one
    // `compare_bus` uses. Built BEFORE the `voltages_excluded` return: the
    // port-internal identity `Σ nodes == len(AllBusVmagPu)` (the circuit walk
    // against the per-bus walk) is not an oracle comparison and holds on a
    // suppressed case too (G1.4a audit settlement AC-7).
    let mut base: Vec<(String, i32, f64)> = Vec::with_capacity(actual.len());
    for v in dss.all_bus_voltages() {
        let bf = bus_base_factor(v.kv_base);
        for node in &v.nodes {
            base.push((v.name.clone(), *node, bf));
        }
    }
    assert_eq!(
        base.len(),
        actual.len(),
        "{ctx}: AllBusVmagPu length {} disagrees with the bus-list node total {}",
        actual.len(),
        base.len()
    );
    if voltages_excluded {
        return;
    }
    for (k, (a, e)) in actual.iter().zip(exp).enumerate() {
        let (name, node, bf) = &base[k];
        let allowed = tol.v_abs / bf + tol.v_rel * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: AllBusVmagPu entry {k} ({name}.{node}) differs: {a} vs {e} \
             (|diff| = {:.3e} > allowed {allowed:.3e})",
            (a - e).abs()
        );
    }
}

// ---------------------------------------------------------------------------
// GOLDEN_REBASE_PLAN.md WP-G1 sub-step G1.5: the per-bus SHORT-CIRCUIT surface.
//
// `Bus.Zsc1`, `Bus.Zsc0`, `Bus.ZscMatrix`, `Bus.YscMatrix`, `Bus.Isc` and
// `Bus.Voc` — the `IBus._columns` entries the fastdss harness dumps
// (`origin/fastdss` `dss/IBus.py:30`/`:39`/`:41-44`, walked from
// `tests/save_outputs.py:350`) that neither G1.4a nor G1.4c owns. Captured by
// the SAME per-bus walk `compare_bus` rides — `oracle_server.py`'s
// `capture_all_buses(ckt, want_sc)` and `dss-epri`'s
// `capture_all_buses(engine, want_sc)`, six reads appended after the five
// voltage ones — and read back from the engine through
// `Dss::all_bus_short_circuit` (`crates/dss-core/src/exec/view.rs`).
//
// **Two things make this surface different from G1.4a's.**
//
// 1. *Ordering.* Every array here is indexed by the bus's INTERNAL (insertion)
//    node index, because both engines read `GetRef(i)` / `Zsc.GetElement(i, j)`
//    straight off `TDSSBus` (`CAPI/CAPI_Alt.pas:2202-2365` == r4133
//    `Version8/Source/DDLL/DBus.pas:351-518`) — never the `repeat FindIdx(jj)`
//    ascending-node walk `BusCap`'s three voltage arrays use. So this
//    comparator must NOT sort, and `compare_bus`'s sort must not leak in here.
//    That the port's insertion order IS the oracles' internal order is pinned
//    from both ends: `compare_all_bus_vmag_pu` (G1.4a) gates
//    `Circuit.AllBusVmagPu`, which is exactly bus-list order x internal node
//    index, and `exec::view::bus_sc_tests::`
//    `the_short_circuit_arrays_are_indexed_by_internal_node_index` pins the
//    ohms on a `b2.2.1.3` bus whose 1-phase shunt makes the two orders
//    observably different.
// 2. *`Zsc`/`Ysc` are a shape, not a value.* `TDSSBus.Zsc` stays unassigned
//    until `AllocateAllSCParms` runs inside the FaultStudy solve
//    (`Common/SolutionAlgs.pas:773-781`), and the two channels publish
//    DIFFERENT not-run sentinels ([`CAPI_SC_SENTINEL_LEN`] vs
//    [`R4133_SC_SENTINEL_LEN`]). The comparator therefore compares the discrete
//    "study ran" bit first and normalizes the two sentinel shapes to "no
//    matrix" — a comparator-level normalization plus a pin, per coordinator
//    decision D4, never a ledger row.
//
// **Tolerances: this surface adds no new constant either.** `Zsc`'s columns are
// node-voltage solves of `Y*V = e_i` at exactly 1+0j A, so the node-voltage band
// carries over numerically unchanged (as ohms); `Ysc = Zsc^-1` takes the
// admittance band and `Isc = Ysc*Voc` the current one. Derivations:
// tests/TOLERANCE_NOTES.md §"Short-circuit surface (GOLDEN_REBASE G1.5)".
// ---------------------------------------------------------------------------

/// What the **capi 0.14.5** channel publishes for `Bus.ZscMatrix` /
/// `Bus.YscMatrix` while the bus has no short-circuit matrix (`pBus.Zsc = NIL`):
/// `DefaultResult`'s single `0.0` — ONE double
/// (`CAPI/CAPI_Utils.pas:212-221`, the `DSS_CAPI_COM_DEFAULTS` branch, which is
/// on in the pinned 0.15.7 build).
const CAPI_SC_SENTINEL_LEN: usize = 1;

/// What the **r4133** channel publishes in that state: the
/// `setlength(myCmplxArray, 1); myCmplxArray[0] := CZero` prelude every `BUSV`
/// arm opens with and the `If Assigned(Zsc)` guard then leaves in place — TWO
/// doubles (`DDLL/DBus.pas:433-434` for `Zsc`, `:493-494` for `Ysc`).
///
/// The same two also arrive for `Isc`/`Voc` at a **0-node** bus, where r4133's
/// `Reallocmem(VBus, 0)` frees the pointer (`Common/Bus.pas:246-260`) and the
/// `If VBus <> nil` guard at `DBus.pas:353`/`:376` fails, while capi's
/// `AllocMem(0)` block is non-nil (`Common/Bus.pas:250-256`) so capi walks the
/// normal path and writes ZERO doubles. Measured on the corpus's only two
/// 0-node buses (`loadbus2` of `Test/REACTORTest.DSS` and of
/// `Test/Source012Test.dss`): capi `(isc, voc) = (0, 0)`, r4133 `(2, 2)`.
const R4133_SC_SENTINEL_LEN: usize = 2;

/// The not-run sentinel length of one oracle channel — the ONE cross-channel
/// normalization this surface needs (coordinator decision D4: a sentinel-SHAPE
/// difference is a comparator-level normalization plus a pin, never a ledger
/// row). Both numbers are pinned literally by
/// `the_two_channels_publish_different_zsc_sentinels`.
fn sc_sentinel_len(channel: PropsChannel) -> usize {
    match channel {
        PropsChannel::CapiV0145 => CAPI_SC_SENTINEL_LEN,
        PropsChannel::R4133 => R4133_SC_SENTINEL_LEN,
    }
}

/// Which shape an oracle's `ZscMatrix`/`YscMatrix` payload arrived in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScMatrix {
    /// `2*n*n` doubles — the study ran and the matrix is on the wire.
    Full,
    /// This channel's not-run sentinel — the bus has no matrix.
    Sentinel,
    /// `2*n*n == SENTINEL_LEN`: the **r4133** channel at a **1-node** bus,
    /// where a real 1x1 matrix and the `CZero` prelude are both 2 doubles.
    /// Resolved from the port's own `Option` — which the circuit-level study
    /// bit has already pinned against this oracle on a bus with `n >= 2` — and,
    /// when the port has no matrix, closed positively by asserting the oracle's
    /// two doubles ARE `CZero` (a real `Zsc[0][0]` is never exactly zero).
    Ambiguous,
}

/// Classify one oracle matrix payload; panics on a length that is neither
/// shape, which is a transport/shape defect rather than a value divergence.
fn sc_matrix_shape(
    len: usize,
    n: usize,
    sentinel: usize,
    what: &str,
    bus: &str,
    channel: PropsChannel,
    ctx: &str,
) -> ScMatrix {
    let full = 2 * n * n;
    if full == sentinel {
        assert_eq!(
            len,
            full,
            "{ctx}: bus {bus} {what} length {len} is neither 2*{n}*{n} = {full} nor the \
             {} not-run sentinel {sentinel} (the two coincide at this bus)",
            channel.tag()
        );
        return ScMatrix::Ambiguous;
    }
    if len == full {
        ScMatrix::Full
    } else if len == sentinel {
        ScMatrix::Sentinel
    } else {
        panic!(
            "{ctx}: bus {bus} {what} length {len} is neither 2*{n}*{n} = {full} nor the \
             {} not-run sentinel {sentinel}",
            channel.tag()
        )
    }
}

/// Interleave a complex slice into the oracle's re/im wire shape.
fn interleave(v: &[Complex64]) -> Vec<f64> {
    v.iter().flat_map(|c| [c.re, c.im]).collect()
}

// ---------------------------------------------------------------------------
// The GLOBAL guard: was a real short-circuit matrix ever compared? (G1.5 audit
// settlement, T1)
// ---------------------------------------------------------------------------

/// Gating short-circuit compares that carried a real matrix — one per
/// [`compare_bus_short_circuit`] call, made from the corpus gate's runner, that
/// value-compared at least one full `n x n` `ZscMatrix`/`YscMatrix`.
static SC_STUDY_WALKS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// …and the buses those walks compared a full matrix on.
static SC_STUDY_BUSES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// What a full-population gate run must reach: `(walks, buses)` over which a
/// real `n x n` matrix was value-compared, re-derived on every run and asserted
/// EXACTLY, so the number fails on a drop **and** on a growth.
///
/// It is the surface's fail-on-stale, and it exists because the comparator's
/// own content gate is `assert_eq!(port_ran, oracle_ran)`, which is satisfied
/// when BOTH sides report "no study": drop `set mode=faultstudy` from a deck
/// and every remaining assertion — sentinel lengths, `Zsc1`/`Zsc0` = 0, a zero
/// `Isc` — still passes, leaving the whole non-trivial half of the surface
/// green over nothing. `population.lock.json` cannot see it either: it
/// fingerprints the manifest FLAG (`zsc=1`) and `MODES_REQUIRED` the deck
/// PATH, never that the deck still solves a study.
///
/// Five cases carry a matrix, all `engines: "both"`, all one gated step —
/// `IEEE123Master-SC`, `ieee34Mod2_SC_Case_II`, `ieee37_SC_Currents`,
/// `NEVTestCase/Run_NEV` and `modes:faultstudy/faultstudy_micro` — so the walk
/// count is ten, and the bus figure is those five decks' bus counts summed over
/// both channels. Both halves are MEASURED, not predicted: the live run prints
/// them on its own `corpus_gate short-circuit:` line, and moving either one is
/// a deliberate act (2026-09-05, lane `lane-b`, 525/525 green in both lanes).
const SC_STUDY_POPULATION: (usize, usize) = (10, 646);

/// Record one gating short-circuit compare that carried `buses` full matrices.
///
/// The single caller is `corpus_gate/runner.rs`'s `compare_zsc` block — the
/// gate's one entry point into this comparator. The harness' own unit drives
/// call [`compare_bus_short_circuit`] directly and are deliberately NOT
/// counted: they run in the same process as the gate in this test binary, and
/// counting them would make the exact population above unstable (and green
/// under exactly the corpus regression it exists to catch).
pub fn record_sc_study_compare(buses: usize) {
    if buses > 0 {
        SC_STUDY_WALKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        SC_STUDY_BUSES.fetch_add(buses, std::sync::atomic::Ordering::Relaxed);
    }
}

/// What [`record_sc_study_compare`] has counted in this process, as
/// `(walks, buses)`.
pub fn sc_study_counters() -> (usize, usize) {
    (
        SC_STUDY_WALKS.load(std::sync::atomic::Ordering::Relaxed),
        SC_STUDY_BUSES.load(std::sync::atomic::Ordering::Relaxed),
    )
}

/// **Fail-on-stale for the short-circuit surface's non-trivial half**, called
/// once from the corpus gate's epilogue. Silent under `DSS_GATE_ONLY` — a
/// filtered run legitimately holds none of the five decks — exactly like its
/// neighbours [`props_norm::assert_r4133_props_compare_ran`] and
/// [`lane::assert_reround_cells_are_live`].
///
/// [`props_norm::assert_r4133_props_compare_ran`]: props_norm::assert_r4133_props_compare_ran
/// [`lane::assert_reround_cells_are_live`]: lane::assert_reround_cells_are_live
pub fn assert_sc_study_compare_ran() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let (walks, buses) = sc_study_counters();
    check_sc_study_compare_ran(walks, buses);
}

/// The rule itself, over **injected** counters — split off for the reason
/// `props_norm::check_r4133_props_compare_ran`'s twin is: the shipped statics
/// cannot be rewound from a test once the gate has moved them, so both
/// directions are pinned offline in `corpus_gate.rs`.
pub fn check_sc_study_compare_ran(walks: usize, buses: usize) {
    assert_eq!(
        (walks, buses),
        SC_STUDY_POPULATION,
        "the short-circuit surface compared {walks} walk(s) / {buses} bus(es) with a real \
         n x n matrix, not {SC_STUDY_POPULATION:?}. Fewer means a deck stopped solving its \
         fault study (or lost a step, or a channel) — which every other assertion in \
         `compare_bus_short_circuit` survives, because `port_ran == oracle_ran` is also \
         true when NEITHER ran; more means a new study-running case arrived. Re-derive \
         the pair from the run's own `corpus_gate short-circuit:` line and move it \
         deliberately (GOLDEN_REBASE G1.5 audit settlement, T1)."
    );
}

/// Compare every bus's captured short-circuit surface against the engine's, in
/// `BusList` order (GOLDEN_REBASE_PLAN.md G1.5).
///
/// Structure, **discrete first, numbers second**:
/// * the per-circuit **"study ran" bit** — `any(zsc.is_some())` on the port
///   against the oracle's payload SHAPE at the first bus with `n >= 2` (where
///   `2*n*n` and the sentinel cannot coincide). A port that silently forgot to
///   allocate `Zsc`, or allocated it without a study, reds here before a single
///   ohm is compared;
/// * per bus, by index: the name (case-insensitively) and the node COUNT — the
///   node set itself is `compare_bus`'s row, and this surface's own vector is
///   the insertion-order one, which no oracle publishes;
/// * every array LENGTH, against that channel's own Pascal shape — including
///   under `voltages_excluded`;
/// * the values, per entry, at `abs + rel*|expected|` bands taken unchanged
///   from the existing tier constants: `v_*` for `Zsc1`/`Zsc0`/`ZscMatrix`
///   (a `Y*V = e_i` solve at 1 A, read as ohms) and for `Voc` (a copy of
///   `NodeV`), `y_*` for `YscMatrix` (`= Zsc^-1`), `i_*` for `Isc`
///   (`= Ysc*Voc`). No new constant, nothing widened; see
///   tests/TOLERANCE_NOTES.md §"Short-circuit surface".
///
/// `channel` selects the not-run sentinel shape ([`sc_sentinel_len`]) and
/// nothing else: the two oracles run the same algorithm on this surface, so no
/// VALUE here is channel-dependent. It travels as [`PropsChannel`] — despite
/// the name, the harness' only channel type (`corpus_gate`'s `EngineChannel` is
/// `pub(crate)` to one test binary while `harness/` compiles into ~20; the
/// runner already maps it for every non-property surface's capture guard).
///
/// `voltages_excluded` is the caller's D11(2) flag, **narrowed** for this
/// surface: it suppresses only the `Voc` and `Isc` values. `Voc` is a snapshot
/// of the very `Solution.NodeV` the ledger already triaged
/// (`Common/Solution.pas:4070-4083` `UpdateVBus`) and `Isc = Ysc*Voc` is its
/// image, so re-comparing them re-raises a pinned cause; `Zsc`, `Ysc`, `Zsc1`
/// and `Zsc0` are functions of `Y` alone, independent of the solution vector,
/// and stay fully compared there — as do the study bit, the bus identity and
/// every length. Every suppressed case is printed next to the ledger entry that
/// caused it by `corpus_gate.rs`' run report
/// (`LedgerRuntime::bus_array_suppressions`).
///
/// Returns the number of buses whose full `n x n` `ZscMatrix`/`YscMatrix` this
/// call actually value-compared — the surface's non-trivial half, which the
/// gate's runner feeds to [`record_sc_study_compare`] so the epilogue can fail
/// on a corpus that quietly stopped running fault studies (T1).
pub fn compare_bus_short_circuit(
    dss: &Dss,
    exp: &[BusCap],
    tol: &Tolerances,
    channel: PropsChannel,
    voltages_excluded: bool,
    ctx: &str,
) -> usize {
    let mut full_matrices = 0usize;
    let views = dss.all_bus_short_circuit();
    assert_eq!(
        views.len(),
        exp.len(),
        "{ctx}: bus count differs ({} vs {})",
        views.len(),
        exp.len()
    );
    let sentinel = sc_sentinel_len(channel);

    // (1) The discrete study bit, per circuit, before any number. Derived from
    // a bus with `n >= 2` on purpose: there `2*n*n >= 8` can never collide with
    // either channel's sentinel, so the shape is unambiguous. A circuit with no
    // such bus falls through to the per-bus rules below, where the r4133
    // 1-node collision is closed by the `Ambiguous` arm's `CZero` assertion.
    let port_ran = views.iter().any(|v| v.zsc.is_some());
    if let Some(e) = exp.iter().find(|e| e.nodes.len() >= 2) {
        let n = e.nodes.len();
        let full = 2 * n * n;
        let oracle_ran = match e.zsc.len() {
            l if l == full => true,
            l if l == sentinel => false,
            l => panic!(
                "{ctx}: bus {} ZscMatrix length {l} is neither 2*{n}*{n} = {full} nor the \
                 {} not-run sentinel {sentinel}",
                e.name,
                channel.tag()
            ),
        };
        assert_eq!(
            port_ran,
            oracle_ran,
            "{ctx}: the fault-study state differs — the port has {} short-circuit matrix, \
             the {} oracle has {} (witness bus {}, {n} nodes, ZscMatrix length {})",
            if port_ran { "a" } else { "NO" },
            channel.tag(),
            if oracle_ran { "one" } else { "none" },
            e.name,
            e.zsc.len()
        );
    }

    for (i, (v, e)) in views.iter().zip(exp).enumerate() {
        assert!(
            v.name.eq_ignore_ascii_case(&e.name),
            "{ctx}: bus {i} name differs: {} vs {}",
            v.name,
            e.name
        );
        let n = e.nodes.len();
        assert_eq!(
            v.nodes.len(),
            n,
            "{ctx}: bus {} node count differs ({} vs {n})",
            e.name,
            v.nodes.len()
        );

        // `Zsc1`/`Zsc0` are not node-indexed: ONE complex, always, on both
        // channels (`CAPI_Alt.pas:2283-2303` == `DBus.pas:461-489` write the
        // 1-element array unconditionally), `cZERO` while `Zsc` is unassigned
        // (`Common/Bus.pas:215-229`). Compared on every bus, study or not.
        for (arr, what) in [(&e.zsc1, "Zsc1"), (&e.zsc0, "Zsc0")] {
            assert_eq!(
                arr.len(),
                2,
                "{ctx}: bus {} {what} length {} is not 2 (one complex)",
                e.name,
                arr.len()
            );
        }
        assert_complex_close(
            &interleave(&[v.zsc1]),
            &e.zsc1,
            tol.v_rel,
            tol.v_abs,
            &format!("{ctx}: bus {} Zsc1", e.name),
        );
        assert_complex_close(
            &interleave(&[v.zsc0]),
            &e.zsc0,
            tol.v_rel,
            tol.v_abs,
            &format!("{ctx}: bus {} Zsc0", e.name),
        );

        if n == 0 {
            // A 0-node bus indexes nothing, and the two channels legitimately
            // publish different empty shapes (see [`R4133_SC_SENTINEL_LEN`]).
            // Shapes only — normalized, not compared as values (D4).
            for (arr, what) in [
                (&e.zsc, "ZscMatrix"),
                (&e.ysc, "YscMatrix"),
                (&e.isc, "Isc"),
                (&e.voc, "Voc"),
            ] {
                assert!(
                    arr.is_empty() || arr.len() == sentinel,
                    "{ctx}: bus {} (0 nodes) {what} length {} is neither empty nor the \
                     {} sentinel {sentinel}",
                    e.name,
                    arr.len(),
                    channel.tag()
                );
            }
            if let Some(z) = &v.zsc {
                assert!(
                    z.is_empty(),
                    "{ctx}: bus {} (0 nodes) port ZscMatrix has {} entries",
                    e.name,
                    z.len()
                );
            }
            if let Some(y) = &v.ysc {
                assert!(
                    y.is_empty(),
                    "{ctx}: bus {} (0 nodes) port YscMatrix has {} entries",
                    e.name,
                    y.len()
                );
            }
            assert!(
                v.isc.is_empty() && v.vbus.is_empty(),
                "{ctx}: bus {} (0 nodes) port Isc/Voc are not empty ({} / {})",
                e.name,
                v.isc.len(),
                v.vbus.len()
            );
            continue;
        }

        let zsc_shape =
            sc_matrix_shape(e.zsc.len(), n, sentinel, "ZscMatrix", &e.name, channel, ctx);
        let ysc_shape =
            sc_matrix_shape(e.ysc.len(), n, sentinel, "YscMatrix", &e.name, channel, ctx);
        assert_eq!(
            zsc_shape, ysc_shape,
            "{ctx}: bus {}: ZscMatrix and YscMatrix disagree on whether the bus has a \
             matrix ({zsc_shape:?} vs {ysc_shape:?}) — `ComputeYsc` writes both or neither \
             (`Common/SolutionAlgs.pas:800-832`)",
            e.name
        );
        let port_present = v.zsc.is_some();
        assert_eq!(
            v.ysc.is_some(),
            port_present,
            "{ctx}: bus {}: the port has one of Zsc/Ysc and not the other",
            e.name
        );
        match zsc_shape {
            ScMatrix::Full => assert!(
                port_present,
                "{ctx}: bus {}: the {} oracle published a full {n}x{n} ZscMatrix, the port \
                 has none",
                e.name,
                channel.tag()
            ),
            ScMatrix::Sentinel => assert!(
                !port_present,
                "{ctx}: bus {}: the {} oracle published the not-run sentinel, the port has \
                 a {n}x{n} ZscMatrix",
                e.name,
                channel.tag()
            ),
            ScMatrix::Ambiguous if !port_present => {
                // r4133 at a 1-node bus with no matrix on the port side: the
                // oracle's two doubles are then either the `CZero` prelude
                // (agreement) or a real 1x1 `Zsc`, which is never exactly zero
                // on a solved network — so this is a positive assertion of
                // agreement, not a skip.
                assert_eq!(
                    e.zsc,
                    vec![0.0, 0.0],
                    "{ctx}: bus {}: the port has no ZscMatrix while the {} oracle published \
                     a non-`CZero` 1x1 one",
                    e.name,
                    channel.tag()
                );
                assert_eq!(
                    e.ysc,
                    vec![0.0, 0.0],
                    "{ctx}: bus {}: the port has no YscMatrix while the {} oracle published \
                     a non-`CZero` 1x1 one",
                    e.name,
                    channel.tag()
                );
            }
            ScMatrix::Ambiguous => {}
        }
        if let (Some(z), Some(y)) = (&v.zsc, &v.ysc) {
            assert_eq!(
                z.len(),
                n * n,
                "{ctx}: bus {} port ZscMatrix has {} entries, not {n}*{n}",
                e.name,
                z.len()
            );
            assert_eq!(
                y.len(),
                n * n,
                "{ctx}: bus {} port YscMatrix has {} entries, not {n}*{n}",
                e.name,
                y.len()
            );
            // Row-major (`i` outer, `j` inner) on all three engines — the port
            // flattens in the oracles' read order
            // (`exec::view::flatten_row_major`), NOT in `CMatrix`'s
            // column-major storage order.
            assert_complex_close(
                &interleave(z),
                &e.zsc,
                tol.v_rel,
                tol.v_abs,
                &format!("{ctx}: bus {} ZscMatrix", e.name),
            );
            assert_complex_close(
                &interleave(y),
                &e.ysc,
                tol.y_rel,
                tol.y_abs,
                &format!("{ctx}: bus {} YscMatrix", e.name),
            );
            full_matrices += 1;
        }

        for (arr, what) in [(&e.isc, "Isc"), (&e.voc, "Voc")] {
            assert_eq!(
                arr.len(),
                2 * n,
                "{ctx}: bus {} {what} length {} is not 2*{n}",
                e.name,
                arr.len()
            );
        }
        assert_eq!(
            v.isc.len(),
            n,
            "{ctx}: bus {} port Isc has {} entries, not {n}",
            e.name,
            v.isc.len()
        );
        assert_eq!(
            v.vbus.len(),
            n,
            "{ctx}: bus {} port Voc has {} entries, not {n}",
            e.name,
            v.vbus.len()
        );
        if voltages_excluded {
            continue;
        }
        assert_complex_close(
            &interleave(&v.isc),
            &e.isc,
            tol.i_rel,
            tol.i_abs,
            &format!("{ctx}: bus {} Isc", e.name),
        );
        assert_complex_close(
            &interleave(&v.vbus),
            &e.voc,
            tol.v_rel,
            tol.v_abs,
            &format!("{ctx}: bus {} Voc", e.name),
        );
    }
    full_matrices
}

#[cfg(test)]
mod bus_comparator_tests {
    use super::{
        BusCap, Tolerances, bus_angle_band_deg, compare_all_bus_vmag_pu, compare_bus, tol_for,
        wrap_deg,
    };
    use dss_core::exec::Dss;

    /// The deck of `exec/view.rs::bus_voltage_tests` (G1.4a F1), which exercises
    /// all three orderings at once: `b3` is declared `.2.1.3` (insertion order
    /// ≠ ascending node number) and `b1` gains nodes 2 and 3 only after `b2`
    /// was handed its node ref (convention 2 ≠ `YNodeOrder`).
    fn solved() -> Dss {
        let mut dss = Dss::new();
        for c in [
            "clear",
            "New circuit.busview basekv=12.47 pu=1.0 phases=3 bus1=sourcebus",
            "New Line.l1 bus1=sourcebus.1 bus2=b1.1 phases=1 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l2 bus1=b1.1 bus2=b2.1 phases=1 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l3 bus1=sourcebus.2.3 bus2=b1.2.3 phases=2 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l4 bus1=sourcebus.1.2.3 bus2=b3.2.1.3 phases=3 r1=0.1 x1=0.3 c1=0 length=1",
            "New Load.ld bus1=b1.1 phases=1 kv=7.2 kw=500 pf=0.95",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve",
        ] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    /// Build the capture the oracles would send for this deck out of the port's
    /// own view — the positive control every negative drive below perturbs.
    fn capture(dss: &Dss) -> Vec<BusCap> {
        dss.all_bus_voltages()
            .iter()
            .map(|v| {
                let mut nodes = v.nodes.clone();
                nodes.sort_unstable();
                BusCap {
                    name: v.name.clone(),
                    kv_base: v.kv_base,
                    nodes,
                    pu_voltages: v.pu_voltages.iter().flat_map(|c| [c.re, c.im]).collect(),
                    vmag_angle: v.vmag_angle.iter().flat_map(|p| [p.0, p.1]).collect(),
                    pu_vmag_angle: v.pu_vmag_angle.iter().flat_map(|p| [p.0, p.1]).collect(),
                    // A G1.4a capture carries no G1.5 short-circuit arrays:
                    // the six fields are `#[serde(default)]`, so a pre-G1.5
                    // wire payload still deserializes. The short-circuit tests
                    // build their own capture.
                    zsc1: Vec::new(),
                    zsc0: Vec::new(),
                    zsc: Vec::new(),
                    ysc: Vec::new(),
                    isc: Vec::new(),
                    voc: Vec::new(),
                }
            })
            .collect()
    }

    /// [`wrap_deg`] folds the ±180° seam (a phasor whose angle the two engines
    /// report as +179.999… and −179.999… is 2e-3° apart, not 360°), and
    /// [`bus_angle_band_deg`] is the exact arcsine image of the magnitude band,
    /// saturating at 180° once the band swallows the magnitude.
    #[test]
    fn the_bus_angle_band_wraps_the_seam_and_images_the_magnitude_band() {
        assert_eq!(wrap_deg(0.0), 0.0);
        assert_eq!(wrap_deg(180.0), 180.0);
        assert_eq!(wrap_deg(-180.0), 180.0);
        // The seam: +179.999 vs -179.999 is 0.002 deg apart.
        let d = wrap_deg(179.999 - (-179.999));
        assert!((d.abs() - 0.002).abs() < 1e-9, "{d}");
        // …and it stays exact through a full turn.
        assert_eq!(wrap_deg(360.0), 0.0);

        let tol: Tolerances = tol_for("feeder"); // v_abs 1e-6 V, v_rel 1e-8
        // Healthy magnitude: the band is the arcsine image, and within 2e-3
        // relative of the linearized `compare_monitor` construction.
        let mag = 7199.557856794634_f64;
        let eps = tol.v_abs + tol.v_rel * mag;
        let got = bus_angle_band_deg(mag, &tol);
        assert_eq!(got, (eps / mag).asin().to_degrees());
        let linear = 57.29577951308232 * eps / mag;
        assert!((got / linear - 1.0).abs() < 2e-3, "{got} vs {linear}");
        // At/below the absolute floor the angle carries no information: the
        // band saturates at the full wrapped range (the magnitude itself is
        // still pinned inside `eps` on its own row).
        assert_eq!(bus_angle_band_deg(tol.v_abs / 2.0, &tol), 180.0);
        assert_eq!(bus_angle_band_deg(0.0, &tol), 180.0);
    }

    /// Positive control: the comparator accepts the engine's own surface, and
    /// still accepts it when every value is nudged by (just under) its band —
    /// so the bands below are live, not vacuously wide.
    #[test]
    fn compare_bus_accepts_the_engines_own_surface_and_its_band() {
        let dss = solved();
        let tol = tol_for("feeder");
        let exp = capture(&dss);
        compare_bus(&dss, &exp, &tol, false, "positive control");
        compare_all_bus_vmag_pu(
            &dss,
            &dss.all_bus_vmag_pu(),
            &tol,
            false,
            "positive control",
        );

        // Nudge each channel by 90% of its own allowed band.
        let nudged: Vec<BusCap> = exp
            .iter()
            .map(|e| {
                let bf = if e.kv_base > 0.0 {
                    1000.0 * e.kv_base
                } else {
                    1.0
                };
                let pu_step = 0.9 * tol.v_abs / bf;
                let v_step = 0.9 * tol.v_abs;
                BusCap {
                    name: e.name.clone(),
                    kv_base: e.kv_base,
                    nodes: e.nodes.clone(),
                    // The complex band is on |Δ|, so split the nudge over re/im.
                    pu_voltages: e
                        .pu_voltages
                        .iter()
                        .map(|x| x + pu_step / std::f64::consts::SQRT_2)
                        .collect(),
                    vmag_angle: e
                        .vmag_angle
                        .iter()
                        .enumerate()
                        .map(|(i, x)| if i % 2 == 0 { x + v_step } else { *x })
                        .collect(),
                    pu_vmag_angle: e
                        .pu_vmag_angle
                        .iter()
                        .enumerate()
                        .map(|(i, x)| if i % 2 == 0 { x + pu_step } else { *x })
                        .collect(),
                    zsc1: Vec::new(),
                    zsc0: Vec::new(),
                    zsc: Vec::new(),
                    ysc: Vec::new(),
                    isc: Vec::new(),
                    voc: Vec::new(),
                }
            })
            .collect();
        compare_bus(&dss, &nudged, &tol, false, "band drive");
    }

    /// Non-vacuity (§1.1(f) corruption (i)): a per-unit base off by the
    /// `1000×` factor — the divide that makes `puVoltages` per-unit — reds this
    /// comparator while node voltages, Y, elements and `node_order` all stay
    /// green (they never see `BaseFactor` at all).
    #[test]
    #[should_panic(expected = "puVoltages")]
    fn a_pu_base_off_by_the_kv_factor_reds_the_bus_comparator() {
        let dss = solved();
        let mut exp = capture(&dss);
        for x in &mut exp[1].pu_voltages {
            *x *= 1000.0;
        }
        compare_bus(&dss, &exp, &tol_for("feeder"), false, "corruption (i)");
    }

    /// Non-vacuity (§1.1(f) corruption (ii)): the per-bus arrays delivered in
    /// the bus's INSERTION order instead of ascending node number. Only
    /// convention 1 sees it — `b3` is declared `.2.1.3`, and 83 767 corpus buses
    /// carry such a non-prefix node set.
    #[test]
    #[should_panic(expected = "VMagAngle magnitude")]
    fn insertion_order_instead_of_ascending_node_number_reds_the_bus_comparator() {
        let dss = solved();
        let mut exp = capture(&dss);
        let b3 = exp
            .iter_mut()
            .find(|b| b.name == "b3")
            .expect("bus b3 declared .2.1.3");
        assert_eq!(b3.nodes, vec![1, 2, 3]);
        // Insertion order 2,1,3 => swap the first two node slots (pairs).
        for arr in [&mut b3.vmag_angle, &mut b3.pu_vmag_angle] {
            arr.swap(0, 2);
            arr.swap(1, 3);
        }
        compare_bus(&dss, &exp, &tol_for("feeder"), false, "corruption (ii)");
    }

    /// `voltages_excluded` suppresses the three continuous arrays and NOTHING
    /// else: a corrupted `kv_base` (a discrete, solution-independent quantity)
    /// still reds, so the rule cannot hide a bus-identity or voltage-base defect.
    #[test]
    #[should_panic(expected = "kVBase differs")]
    fn the_voltage_exclusion_still_pins_kv_base() {
        let dss = solved();
        let mut exp = capture(&dss);
        for x in &mut exp[1].pu_voltages {
            *x *= 1000.0; // would red — but is excluded
        }
        exp[1].kv_base += 1.0; // …this must not be
        compare_bus(&dss, &exp, &tol_for("feeder"), true, "exclusion drive");
    }

    /// Non-vacuity of the ANGLE channel (G1.4a audit settlement T2): the
    /// magnitude rows are the ones the two committed corruptions above trip, so
    /// the wrap-aware angle compare needs its own drive. 1e-3° is ~1700× the
    /// band at 7.2 kV (`asin(eps/|V|)` ≈ 5.8e-7°).
    #[test]
    #[should_panic(expected = "VMagAngle: angle differs")]
    fn an_out_of_band_angle_reds_the_bus_comparator() {
        let dss = solved();
        let mut exp = capture(&dss);
        let b3 = exp
            .iter_mut()
            .find(|b| b.name == "b3")
            .expect("bus b3 declared .2.1.3");
        b3.vmag_angle[1] += 1e-3; // node 1's angle only — magnitudes untouched
        compare_bus(&dss, &exp, &tol_for("feeder"), false, "angle drive");
    }

    /// …and the per-unit angle channel is compared too: the same nudge on
    /// `puVMagAngle` alone (both magnitudes and the volt-angle left exact) reds.
    #[test]
    #[should_panic(expected = "puVMagAngle: angle differs")]
    fn an_out_of_band_pu_angle_reds_the_bus_comparator() {
        let dss = solved();
        let mut exp = capture(&dss);
        let b3 = exp
            .iter_mut()
            .find(|b| b.name == "b3")
            .expect("bus b3 declared .2.1.3");
        b3.pu_vmag_angle[1] += 1e-3;
        compare_bus(&dss, &exp, &tol_for("feeder"), false, "pu angle drive");
    }

    /// The ±180° seam is folded, not compared raw: an oracle that reports every
    /// angle a full turn away is accepted (`wrap_deg`), while the 1e-3° drives
    /// above still red — so the fold is not a free pass.
    #[test]
    fn a_full_turn_of_angle_is_accepted_by_the_bus_comparator() {
        let dss = solved();
        let mut exp = capture(&dss);
        for b in &mut exp {
            for arr in [&mut b.vmag_angle, &mut b.pu_vmag_angle] {
                for (i, x) in arr.iter_mut().enumerate() {
                    if i % 2 == 1 {
                        *x -= 360.0;
                    }
                }
            }
        }
        compare_bus(&dss, &exp, &tol_for("feeder"), false, "seam drive");
    }

    /// Non-vacuity of `compare_all_bus_vmag_pu` (G1.4a audit settlement T1): the
    /// circuit-level walk (convention 2 — bus-list order × INTERNAL node index)
    /// has its own accessor and its own comparator, and neither corruption above
    /// touches it. 1e-3 pu is ~1e5× its per-entry band.
    #[test]
    #[should_panic(expected = "AllBusVmagPu entry")]
    fn a_corrupted_all_bus_vmag_pu_entry_reds_the_comparator() {
        let dss = solved();
        let mut exp = dss.all_bus_vmag_pu();
        assert!(exp.len() > 3, "deck must have several nodes");
        exp[3] += 1e-3;
        compare_all_bus_vmag_pu(&dss, &exp, &tol_for("feeder"), false, "vmagpu drive");
    }

    /// …and under `voltages_excluded` the length compare against the oracle
    /// still runs (only the per-entry values are dropped), so a node the oracle
    /// does not report cannot slip through a suppressed case.
    #[test]
    #[should_panic(expected = "AllBusVmagPu length differs")]
    fn the_voltage_exclusion_still_pins_the_all_bus_vmag_pu_length() {
        let dss = solved();
        let mut exp = dss.all_bus_vmag_pu();
        exp.pop();
        compare_all_bus_vmag_pu(&dss, &exp, &tol_for("feeder"), true, "exclusion drive");
    }
}

#[cfg(test)]
mod bus_short_circuit_tests {
    use super::{
        BusCap, CAPI_SC_SENTINEL_LEN, PropsChannel, R4133_SC_SENTINEL_LEN, Tolerances,
        compare_bus_short_circuit, sc_sentinel_len, tol_for,
    };
    use dss_core::exec::Dss;
    use num_complex::Complex64;

    /// The G1.5 short-circuit deck, the circuit
    /// `exec::view::bus_sc_tests::sc_micro` drives plus two structural buses
    /// this comparator needs and that one does not:
    ///
    /// * `b2` is reached as `bus2=b2.2.1.3`, so its INSERTION order `[2, 1, 3]`
    ///   is not its ascending node order `[1, 2, 3]`, and the 1-phase
    ///   `reactor.rsh` on `b2.1` makes the two observably different (a
    ///   position-dependent `Zsc` diagonal) rather than merely nominally so;
    /// * `b3` is a **1-node** spur — the bus where r4133's `2*n*n = 2` collides
    ///   with its own not-run sentinel ([`R4133_SC_SENTINEL_LEN`]);
    /// * `b0` is a **0-node** bus (`bus2=b0.0.0.0`, every conductor grounded —
    ///   the `Test/REACTORTest.DSS:31` shape), where the two channels publish
    ///   different empty shapes for `Isc`/`Voc`.
    ///
    /// Returned solved but *before* the fault study, so each test drives the
    /// study itself.
    fn sc_deck() -> Dss {
        let mut dss = Dss::new();
        for c in [
            "clear",
            "Set DefaultBaseFrequency=60",
            "new circuit.scharness basekv=12.47 pu=1.0 phases=3 bus1=sourcebus \
             r1=0.5 x1=1.5 r0=1.0 x0=3.0",
            "new linecode.lc3 nphases=3 r1=0.1 x1=0.3 r0=0.3 x0=0.9 c1=0 c0=0 units=km",
            "new linecode.lc1 nphases=1 r1=0.4 x1=1.2 c1=0 units=km",
            "new line.l1 bus1=sourcebus bus2=b1 linecode=lc3 length=1 units=km",
            "new line.l2 bus1=b1.1.2.3 bus2=b2.2.1.3 linecode=lc3 length=1 units=km",
            "new line.l3 bus1=b2.1 bus2=b3.1 linecode=lc1 length=1 units=km",
            "new reactor.rsh bus1=b2.1 phases=1 R=5 X=0",
            "new reactor.rgnd bus1=b1.1.2.3 bus2=b0.0.0.0 phases=3 R=1000 X=0",
            "new load.ld1 bus1=b2.2.1.3 phases=3 conn=wye kv=12.47 kw=100 pf=0.95 model=1",
            "set voltagebases=[12.47]",
            "calcvoltagebases",
            "solve",
        ] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        // The three structural buses the drives below rely on.
        let views = dss.all_bus_short_circuit();
        let n_of = |name: &str| {
            views
                .iter()
                .find(|v| v.name == name)
                .unwrap_or_else(|| panic!("bus {name} missing"))
                .nodes
                .len()
        };
        assert_eq!(n_of("b2"), 3, "b2 must carry three nodes");
        assert_eq!(n_of("b3"), 1, "b3 must be the 1-node spur");
        assert_eq!(n_of("b0"), 0, "b0 must be the 0-node bus");
        dss
    }

    fn fault_study(dss: &mut Dss) {
        dss.command("solve mode=faultstudy");
        assert!(dss.errors().is_empty(), "faultstudy: {:?}", dss.errors());
    }

    /// Build the capture THAT channel's transport would send for this circuit
    /// out of the port's own view — the positive control every drive below
    /// perturbs. It reproduces each transport's Pascal shape rule exactly (the
    /// same rules `tools/oracle/oracle_server.py::capture_all_buses` and
    /// `dss-epri::capture::capture_all_buses` assert against live payloads):
    /// a matrix is `2*n*n` doubles or the channel's not-run sentinel, and at a
    /// 0-node bus `Isc`/`Voc` are empty on capi but the 2-double `CZero`
    /// prelude on r4133.
    ///
    /// The four `compare_bus` fields are left empty: they are that
    /// comparator's rows, and nothing here reads them.
    fn capture(dss: &Dss, channel: PropsChannel) -> Vec<BusCap> {
        let sentinel = sc_sentinel_len(channel);
        dss.all_bus_short_circuit()
            .iter()
            .map(|v| {
                let n = v.nodes.len();
                let mut nodes = v.nodes.clone();
                nodes.sort_unstable();
                let mat = |m: &Option<Vec<Complex64>>| -> Vec<f64> {
                    match m {
                        Some(x) => x.iter().flat_map(|c| [c.re, c.im]).collect(),
                        None => vec![0.0; sentinel],
                    }
                };
                let node_arr = |a: &[Complex64]| -> Vec<f64> {
                    if n == 0 && channel == PropsChannel::R4133 {
                        vec![0.0; R4133_SC_SENTINEL_LEN]
                    } else {
                        a.iter().flat_map(|c| [c.re, c.im]).collect()
                    }
                };
                BusCap {
                    name: v.name.clone(),
                    kv_base: 0.0,
                    nodes,
                    pu_voltages: Vec::new(),
                    vmag_angle: Vec::new(),
                    pu_vmag_angle: Vec::new(),
                    zsc1: vec![v.zsc1.re, v.zsc1.im],
                    zsc0: vec![v.zsc0.re, v.zsc0.im],
                    zsc: mat(&v.zsc),
                    ysc: mat(&v.ysc),
                    isc: node_arr(&v.isc),
                    voc: node_arr(&v.vbus),
                }
            })
            .collect()
    }

    fn bus_mut<'a>(exp: &'a mut [BusCap], name: &str) -> &'a mut BusCap {
        exp.iter_mut()
            .find(|b| b.name == name)
            .unwrap_or_else(|| panic!("bus {name} missing from the capture"))
    }

    /// Run a compare that is expected to fail and return the panic message.
    ///
    /// `AssertUnwindSafe` because the closure only borrows a `&Dss` the caller
    /// built and never mutates it: nothing observable survives the unwind.
    ///
    /// The silencing hook is scoped to THIS thread and delegates every other
    /// panic to the previous hook — `lane::passes`' pattern, for its reason:
    /// this module compiles into the corpus-gate binary, where libtest runs
    /// these drives in parallel with `corpus_gate_all_cases_match_engines`, and
    /// a blanket no-op hook can eat that test's failure report.
    ///
    /// Generic in the closure's result so the drives can call
    /// [`compare_bus_short_circuit`] directly: it returns the count of full
    /// matrices it compared (for the gate's fail-on-stale, G1.5 audit
    /// settlement T1) and every drive here discards it — these are negative
    /// drives, and the harness' own calls are deliberately not counted.
    fn reds<T: std::fmt::Debug>(f: impl FnOnce() -> T) -> String {
        thread_local! {
            static SILENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        static HOOK: std::sync::Once = std::sync::Once::new();
        HOOK.call_once(|| {
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                if !SILENCE.with(std::cell::Cell::get) {
                    prev(info);
                }
            }));
        });
        SILENCE.with(|s| s.set(true));
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        SILENCE.with(|s| s.set(false));
        let msg = out.expect_err("the comparator accepted a corrupted capture");
        msg.downcast_ref::<String>()
            .cloned()
            .or_else(|| msg.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    }

    /// **The one cross-channel normalization of this surface** (coordinator
    /// decision D4), pinned with both numbers: while a bus has no
    /// short-circuit matrix, capi 0.14.5 publishes `DefaultResult`'s **1**
    /// double (`CAPI/CAPI_Utils.pas:212-221`) and r4133 the `CZero` prelude's
    /// **2** (`DDLL/DBus.pas:433-434`); at a 0-node bus the same split hits
    /// `Isc`/`Voc`, where capi writes **0** doubles (`AllocMem(0)` is non-nil,
    /// `Common/Bus.pas:250-256`) and r4133 **2** (`Reallocmem(VBus, 0)` frees
    /// the pointer, `Common/Bus.pas:246-260`). The comparator normalizes both
    /// shapes to "no matrix"/"no nodes" — and the last arm proves the
    /// normalization is per channel and not a blanket "any short array is a
    /// sentinel": r4133's 2-double payload offered to the capi channel reds.
    #[test]
    fn the_two_channels_publish_different_zsc_sentinels() {
        assert_eq!(CAPI_SC_SENTINEL_LEN, 1);
        assert_eq!(R4133_SC_SENTINEL_LEN, 2);
        assert_eq!(sc_sentinel_len(PropsChannel::CapiV0145), 1);
        assert_eq!(sc_sentinel_len(PropsChannel::R4133), 2);

        let dss = sc_deck(); // no fault study: every bus is in the sentinel state
        let tol = tol_for("micro");
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let exp = capture(&dss, ch);
            assert_eq!(exp.len(), 5, "sourcebus, b1, b2, b3, b0");
            let b2 = exp.iter().find(|b| b.name == "b2").expect("b2");
            assert_eq!(b2.zsc.len(), sc_sentinel_len(ch));
            assert_eq!(b2.zsc1, vec![0.0, 0.0], "cZERO while Zsc is unassigned");
            let b0 = exp.iter().find(|b| b.name == "b0").expect("b0");
            assert_eq!(
                b0.voc.len(),
                if ch == PropsChannel::R4133 { 2 } else { 0 },
                "0-node `Voc` shape on {}",
                ch.tag()
            );
            compare_bus_short_circuit(&dss, &exp, &tol, ch, false, "sentinel control");
        }

        // Per channel, not blanket: r4133's 2-double sentinel is not a shape
        // the capi channel may publish at a 3-node bus (2*3*3 = 18, sentinel 1).
        let r4133_shaped = capture(&dss, PropsChannel::R4133);
        let msg = reds(|| {
            compare_bus_short_circuit(
                &dss,
                &r4133_shaped,
                &tol_for("micro"),
                PropsChannel::CapiV0145,
                false,
                "cross-channel drive",
            )
        });
        assert!(
            msg.contains("not-run sentinel 1"),
            "expected the capi sentinel length in the failure; got {msg:?}"
        );
    }

    /// Positive control on both channels, before and after the study, and the
    /// bands are not zero-width: every value nudged by 90% of its own allowed
    /// band is still accepted.
    #[test]
    fn compare_bus_short_circuit_accepts_the_engines_own_surface_and_its_band() {
        let mut dss = sc_deck();
        let tol = tol_for("micro");
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            compare_bus_short_circuit(&dss, &capture(&dss, ch), &tol, ch, false, "before");
        }
        fault_study(&mut dss);
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let exp = capture(&dss, ch);
            let b2 = exp.iter().find(|b| b.name == "b2").expect("b2");
            assert_eq!(b2.zsc.len(), 2 * 3 * 3, "the study allocated a 3x3 Zsc");
            compare_bus_short_circuit(&dss, &exp, &tol, ch, false, "after");

            // 90% of `abs + rel*|expected|`, split over re/im so the modulus of
            // the nudge is exactly that fraction of the band.
            let nudged: Vec<BusCap> = exp
                .iter()
                .map(|e| {
                    let bump = |arr: &[f64], abs: f64, rel: f64| -> Vec<f64> {
                        arr.chunks_exact(2)
                            .flat_map(|c| {
                                let step = 0.9 * (abs + rel * Complex64::new(c[0], c[1]).norm())
                                    / std::f64::consts::SQRT_2;
                                [c[0] + step, c[1] + step]
                            })
                            .collect()
                    };
                    BusCap {
                        name: e.name.clone(),
                        kv_base: e.kv_base,
                        nodes: e.nodes.clone(),
                        pu_voltages: Vec::new(),
                        vmag_angle: Vec::new(),
                        pu_vmag_angle: Vec::new(),
                        zsc1: bump(&e.zsc1, tol.v_abs, tol.v_rel),
                        zsc0: bump(&e.zsc0, tol.v_abs, tol.v_rel),
                        // The sentinel arrays are odd-length: leave them alone.
                        zsc: if e.zsc.len() % 2 == 0 {
                            bump(&e.zsc, tol.v_abs, tol.v_rel)
                        } else {
                            e.zsc.clone()
                        },
                        ysc: if e.ysc.len() % 2 == 0 {
                            bump(&e.ysc, tol.y_abs, tol.y_rel)
                        } else {
                            e.ysc.clone()
                        },
                        isc: bump(&e.isc, tol.i_abs, tol.i_rel),
                        voc: bump(&e.voc, tol.v_abs, tol.v_rel),
                    }
                })
                .collect();
            compare_bus_short_circuit(&dss, &nudged, &tol, ch, false, "band drive");
        }
    }

    /// Non-vacuity, `ZscMatrix`: 1e-3 ohm on one entry is ~1000x the `micro`
    /// band (`v_abs + v_rel*|Z|` ~ 1e-6 at |Z| ~ 1 ohm).
    ///
    /// Driven on **both** channels (G1.5 audit settlement, T6). The value path
    /// is channel-independent by construction — `channel` selects only
    /// [`sc_sentinel_len`] — but "by construction" is the claim, so each of
    /// these drives states it as a measurement instead.
    #[test]
    fn a_corrupted_zsc_entry_reds_the_short_circuit_comparator() {
        let mut dss = sc_deck();
        fault_study(&mut dss);
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let mut exp = capture(&dss, ch);
            bus_mut(&mut exp, "b2").zsc[6] += 1e-3;
            let msg = reds(|| {
                compare_bus_short_circuit(&dss, &exp, &tol_for("micro"), ch, false, "zsc drive")
            });
            assert!(
                msg.contains("ZscMatrix: entry"),
                "{}: expected a ZscMatrix entry failure, got {msg:?}",
                ch.tag()
            );
        }
    }

    /// Non-vacuity, `YscMatrix` — its own band (`y_*`), its own array: the
    /// `Zsc` drive above cannot reach it. Both channels (T6).
    #[test]
    fn a_corrupted_ysc_entry_reds_the_short_circuit_comparator() {
        let mut dss = sc_deck();
        fault_study(&mut dss);
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let mut exp = capture(&dss, ch);
            bus_mut(&mut exp, "b2").ysc[2] += 1e-3;
            let msg = reds(|| {
                compare_bus_short_circuit(&dss, &exp, &tol_for("micro"), ch, false, "ysc drive")
            });
            assert!(
                msg.contains("YscMatrix: entry"),
                "{}: expected a YscMatrix entry failure, got {msg:?}",
                ch.tag()
            );
        }
    }

    /// Non-vacuity, `Zsc1` — the averaged sequence impedances are their own
    /// arm and are compared on every bus, study or not. Both channels (T6).
    #[test]
    fn a_corrupted_zsc1_reds_the_short_circuit_comparator() {
        let mut dss = sc_deck();
        fault_study(&mut dss);
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let mut exp = capture(&dss, ch);
            bus_mut(&mut exp, "b2").zsc1[0] += 1e-3;
            let msg = reds(|| {
                compare_bus_short_circuit(&dss, &exp, &tol_for("micro"), ch, false, "zsc1 drive")
            });
            assert!(
                msg.contains("Zsc1: entry"),
                "{}: expected a Zsc1 entry failure, got {msg:?}",
                ch.tag()
            );
        }
    }

    /// Non-vacuity, `Isc` (the `i_*` band) and `Voc` (the `v_*` one) — the two
    /// arrays `voltages_excluded` suppresses, proven live when it does not.
    /// Both channels (T6).
    #[test]
    fn a_corrupted_isc_or_voc_reds_the_short_circuit_comparator() {
        let mut dss = sc_deck();
        fault_study(&mut dss);
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            for (what, mutate) in [("Isc: entry", 0usize), ("Voc: entry", 1usize)] {
                let mut exp = capture(&dss, ch);
                let b2 = bus_mut(&mut exp, "b2");
                if mutate == 0 {
                    b2.isc[0] += 1.0;
                } else {
                    b2.voc[0] += 1.0;
                }
                let msg = reds(|| {
                    compare_bus_short_circuit(
                        &dss,
                        &exp,
                        &tol_for("micro"),
                        ch,
                        false,
                        "isc/voc drive",
                    )
                });
                assert!(
                    msg.contains(what),
                    "{}: expected {what} in {msg:?}",
                    ch.tag()
                );
            }
        }
    }

    /// **The ordering convention, driven** (spec §4 demo 2): the same six
    /// numbers delivered in ascending node order instead of the bus's internal
    /// (insertion) index. `b2` is `.2.1.3` with the 5 ohm shunt on node 1, so
    /// the odd `Zsc` diagonal sits at internal index 1 and the permutation
    /// moves it to index 0 — a ~0.6 ohm move, six orders past any band.
    #[test]
    fn ascending_node_order_instead_of_the_internal_index_reds_the_short_circuit_comparator() {
        let mut dss = sc_deck();
        fault_study(&mut dss);
        let views = dss.all_bus_short_circuit();
        let b2v = views.iter().find(|v| v.name == "b2").expect("b2");
        assert_eq!(b2v.nodes, vec![2, 1, 3], "b2 is declared .2.1.3");
        // ascending slot k <- internal slot perm[k]
        let mut perm: Vec<usize> = (0..3).collect();
        perm.sort_by_key(|&i| b2v.nodes[i]);
        assert_eq!(perm, vec![1, 0, 2]);

        // Both channels (T6): the permutation is a wiring defect either
        // transport could ship, and neither sentinel rule can absorb it.
        for ch in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let mut exp = capture(&dss, ch);
            let b2 = bus_mut(&mut exp, "b2");
            let old = b2.zsc.clone();
            for (a, &pa) in perm.iter().enumerate() {
                for (b, &pb) in perm.iter().enumerate() {
                    b2.zsc[2 * (3 * a + b)] = old[2 * (3 * pa + pb)];
                    b2.zsc[2 * (3 * a + b) + 1] = old[2 * (3 * pa + pb) + 1];
                }
            }
            assert_ne!(b2.zsc, old, "the permutation must actually move something");
            let msg = reds(|| {
                compare_bus_short_circuit(
                    &dss,
                    &exp,
                    &tol_for("micro"),
                    ch,
                    false,
                    "ordering drive",
                )
            });
            assert!(
                msg.contains("ZscMatrix: entry"),
                "{}: expected a ZscMatrix entry failure, got {msg:?}",
                ch.tag()
            );
        }
    }

    /// The **discrete study bit** is compared before any number, in both
    /// directions: an oracle that ran the study while the port did not (and
    /// the reverse) reds on the bit, not on an ohm. Both channels (T6) — the
    /// bit is derived from the payload SHAPE, which is the one thing that IS
    /// channel-dependent here, so each channel's sentinel must carry it.
    #[test]
    fn the_study_bit_is_compared_before_any_number() {
        let mut dss = sc_deck();
        let no_study: Vec<(PropsChannel, Vec<BusCap>)> =
            [PropsChannel::CapiV0145, PropsChannel::R4133]
                .into_iter()
                .map(|ch| (ch, capture(&dss, ch)))
                .collect();
        fault_study(&mut dss);

        for (ch, exp) in &no_study {
            // port ran, oracle did not
            let msg = reds(|| {
                compare_bus_short_circuit(
                    &dss,
                    exp,
                    &tol_for("micro"),
                    *ch,
                    false,
                    "study bit drive",
                )
            });
            assert!(
                msg.contains("the fault-study state differs")
                    && msg.contains("the port has a short-circuit matrix"),
                "{}: {msg:?}",
                ch.tag()
            );

            // …and the reverse, on a circuit that never ran one.
            let fresh = sc_deck();
            let with_study = capture(&dss, *ch);
            let msg = reds(|| {
                compare_bus_short_circuit(
                    &fresh,
                    &with_study,
                    &tol_for("micro"),
                    *ch,
                    false,
                    "study bit drive",
                )
            });
            assert!(
                msg.contains("the fault-study state differs")
                    && msg.contains("the port has NO short-circuit matrix"),
                "{}: {msg:?}",
                ch.tag()
            );
        }
    }

    /// The r4133 1-node collision (`2*n*n == 2 == `[`R4133_SC_SENTINEL_LEN`]) is
    /// closed **positively**, not skipped: with no matrix on the port side the
    /// oracle's two doubles must BE `CZero`, so a real 1x1 `Zsc` there reds.
    #[test]
    fn the_r4133_one_node_ambiguity_is_closed_by_a_czero_assertion() {
        let dss = sc_deck(); // no study: the port has no matrix anywhere
        let mut exp = capture(&dss, PropsChannel::R4133);
        let b3 = bus_mut(&mut exp, "b3");
        assert_eq!(b3.zsc.len(), 2, "1-node bus: sentinel and 2*1*1 coincide");
        b3.zsc = vec![0.75, 2.5]; // a plausible 1x1 Zsc instead of CZero
        let msg = reds(|| {
            compare_bus_short_circuit(
                &dss,
                &exp,
                &tol_for("micro"),
                PropsChannel::R4133,
                false,
                "ambiguity drive",
            )
        });
        assert!(
            msg.contains("non-`CZero` 1x1"),
            "expected the CZero assertion; got {msg:?}"
        );
    }

    /// `voltages_excluded` (coordinator decision D11(2), **narrowed** for this
    /// surface) drops the `Voc`/`Isc` VALUES and nothing else: the same capture
    /// that reds without it is accepted with it…
    #[test]
    fn the_voltage_exclusion_drops_only_the_voc_and_isc_values() {
        let mut dss = sc_deck();
        fault_study(&mut dss);
        let mut exp = capture(&dss, PropsChannel::CapiV0145);
        {
            let b2 = bus_mut(&mut exp, "b2");
            b2.voc[0] += 1e3;
            b2.isc[1] += 1e3;
        }
        let msg = reds(|| {
            compare_bus_short_circuit(
                &dss,
                &exp,
                &tol_for("micro"),
                PropsChannel::CapiV0145,
                false,
                "not excluded",
            )
        });
        assert!(
            msg.contains("Isc: entry") || msg.contains("Voc: entry"),
            "{msg:?}"
        );
        compare_bus_short_circuit(
            &dss,
            &exp,
            &tol_for("micro"),
            PropsChannel::CapiV0145,
            true,
            "excluded",
        );
    }

    /// …while `Zsc`/`Ysc` — functions of `Y` alone, independent of the
    /// solution vector the ledger triaged — stay fully compared under it, as do
    /// the study bit and every length. Three drives, one per claim.
    #[test]
    fn the_voltage_exclusion_still_pins_zsc_ysc_and_the_lengths() {
        let mut dss = sc_deck();
        fault_study(&mut dss);
        let base = || capture(&dss, PropsChannel::CapiV0145);

        let mut exp = base();
        bus_mut(&mut exp, "b2").zsc[6] += 1e-3;
        let msg = reds(|| {
            compare_bus_short_circuit(
                &dss,
                &exp,
                &tol_for("micro"),
                PropsChannel::CapiV0145,
                true,
                "excluded zsc drive",
            )
        });
        assert!(msg.contains("ZscMatrix: entry"), "{msg:?}");

        let mut exp = base();
        bus_mut(&mut exp, "b2").ysc[2] += 1e-3;
        let msg = reds(|| {
            compare_bus_short_circuit(
                &dss,
                &exp,
                &tol_for("micro"),
                PropsChannel::CapiV0145,
                true,
                "excluded ysc drive",
            )
        });
        assert!(msg.contains("YscMatrix: entry"), "{msg:?}");

        let mut exp = base();
        bus_mut(&mut exp, "b2").voc.pop();
        let msg = reds(|| {
            compare_bus_short_circuit(
                &dss,
                &exp,
                &tol_for("micro"),
                PropsChannel::CapiV0145,
                true,
                "excluded length drive",
            )
        });
        assert!(msg.contains("Voc length 5 is not 2*3"), "{msg:?}");
    }

    /// A shape neither full nor sentinel is a transport defect and fails
    /// loudly, never "compares" as a short array.
    #[test]
    #[should_panic(expected = "is neither 2*3*3 = 18 nor the")]
    fn an_unrecognized_matrix_length_fails_loudly() {
        let mut dss = sc_deck();
        fault_study(&mut dss);
        let mut exp = capture(&dss, PropsChannel::CapiV0145);
        bus_mut(&mut exp, "b2").zsc.truncate(8);
        compare_bus_short_circuit(
            &dss,
            &exp,
            &tol_for("micro"),
            PropsChannel::CapiV0145,
            false,
            "shape drive",
        );
    }

    /// The `micro`/`feeder` bands this surface reuses are the existing tier
    /// constants, unchanged — the derivation lives in
    /// `tests/TOLERANCE_NOTES.md` §"Short-circuit surface". Pinned here so a
    /// silent widening of the tier moves a test in this file too.
    #[test]
    fn the_short_circuit_surface_adds_no_tolerance_constant() {
        let m: Tolerances = tol_for("micro");
        assert_eq!((m.v_abs, m.v_rel), (1e-6, 1e-9));
        assert_eq!((m.y_abs, m.y_rel), (1e-6, 1e-9));
        assert_eq!((m.i_abs, m.i_rel), (1e-6, 1e-9));
        let f: Tolerances = tol_for("feeder");
        assert_eq!((f.v_abs, f.v_rel), (1e-6, 1e-8));
        assert_eq!((f.y_abs, f.y_rel), (1e-6, 1e-8));
        assert_eq!((f.i_abs, f.i_rel), (1e-5, 1e-7));
    }
}

// ---------------------------------------------------------------------------
// Phase 8: text/CSV report comparison (PHASE8_PLAN §2.3).
//
// Compares an oracle-written report file against the Rust-written one after
// parsing numbers out (never a raw float-string diff): a fixed header block is
// matched verbatim, then each data row is split on the report's separator and
// compared field-by-field — numbers within tolerance, identifiers
// case-insensitively. Element/row ordering is the report's contract (Pascal
// iteration order is observable) unless `RustSubsetByKey` is used.
// ---------------------------------------------------------------------------

/// Row-set matching policy for [`compare_export`].
pub enum RowPolicy {
    /// Rust and oracle data rows are identical and in the same order (the
    /// report's contract — Pascal iteration order is observable).
    ExactOrdered,
    /// The Rust file's rows must be a **subset** of the oracle's, matched by the
    /// `key`-th field, with matching value fields. Used by `Export Counts`
    /// during the port: the Rust class registry is a *proper subset* of the
    /// oracle's (only a subset of classes is ported), so this pins every ported
    /// class's count against the oracle without failing on the classes we do not
    /// yet register. Documented in `tests/TOLERANCE_NOTES.md`.
    ///
    /// `require` lists lowercased key values that **must** appear in the Rust
    /// rows — without it the subset check (which iterates only the Rust rows)
    /// would silently pass an empty/under-reporting Rust body (a dropped class
    /// is just absent, not a mismatch). Pass the deck-created + default-item keys
    /// so a registry-walk regression cannot hide (audit-tests WP8.1).
    ///
    /// `allow_extra` lists lowercased keys the Rust engine emits that the PINNED
    /// 0.14.5 oracle CANNOT have — the upgrade-era 0.15.x-only classes the port
    /// now registers (e.g. `windgen`, WP-U1.8). A Rust key in this set that is
    /// absent from the oracle is skipped instead of failing the subset check;
    /// its surface is gated against the `capi015` oracle (the live corpus decks)
    /// + the class's props round-trip, not the 0.14.5 report golden.
    RustSubsetByKey {
        key: usize,
        require: Vec<String>,
        allow_extra: Vec<String>,
    },
}

/// How a [`ColTol`] selects the columns it applies to.
#[derive(Clone)]
pub enum ColSel {
    /// Columns whose (trimmed, lowercased) header name starts with this prefix.
    /// Used where every value column is named (the full-header reports — the
    /// `%…` ratio columns of `SeqCurrents`/`SeqVoltages`, the `Angle%d` columns of
    /// `Voltages`).
    Prefix(String),
    /// Columns at index `>= start` with `(index − start) % 2 == parity`. Used for
    /// the **truncated-header** paired magnitude/angle reports
    /// (`Currents`/`ElemCurrents`/`ElemVoltages`, whose header names only the
    /// first pair, e.g. `…, I_1, Ang_1, ...`): `parity = 1` selects the (mostly
    /// unnamed) angle columns, `parity = 0` the magnitudes.
    Parity { start: usize, parity: usize },
    /// The single column at this exact index. Used to target a specific
    /// non-deterministic column by position (the `Summary` `DateTime` column 0,
    /// masked via [`GateSpec::Mask`]).
    Index(usize),
    /// The column **immediately following** a token equal to `glyph` in the row.
    /// A *content-relative* selector (needs the row fields, not just the header):
    /// used for the fixed-width `Show` angle columns, which sit right after the
    /// `/_` angle glyph but at a **row-dependent index** — the element voltage form
    /// splits the parenthesised `(pu)`/`(nref)` into two tokens (`(` + `n)`) so the
    /// angle's absolute index varies, and a fixed `Index`/`Parity` cannot target it.
    /// Selecting "the column after `/_`" pins it regardless of the leading-token
    /// shift.
    AfterToken(String),
    /// The column `n` positions from the **end** of the row (`FromEnd(0)` = the
    /// last token). Another content-relative selector, for reports whose leading
    /// name column varies in token count between rows — `Show Mismatch`'s
    /// `"System Ground"` splits into two tokens while a bus name is one, shifting
    /// every column by 1, but the value columns are stable *from the end*
    /// (`Max Current` = last, `%error` = `FromEnd(1)`, `Current Sum` = `FromEnd(2)`).
    FromEnd(usize),
}

impl ColSel {
    fn matches(&self, j: usize, colnames: &[String], fields: &[String]) -> bool {
        match self {
            ColSel::Prefix(p) => colnames
                .get(j)
                .is_some_and(|n| n.trim().to_lowercase().starts_with(&p.to_lowercase())),
            ColSel::Parity { start, parity } => j >= *start && (j - start) % 2 == *parity,
            ColSel::Index(i) => j == *i,
            ColSel::AfterToken(glyph) => {
                j > 0 && fields.get(j - 1).is_some_and(|f| f.trim() == glyph)
            }
            ColSel::FromEnd(n) => !fields.is_empty() && j == fields.len().wrapping_sub(1 + n),
        }
    }
}

/// A per-column tolerance override for [`compare_export`], selecting columns by
/// [`ColSel`] (name prefix or index parity). The first matching override wins;
/// columns with no match use the policy's default `rel`/`abs`.
///
/// Used for the fixed-decimal `Angle` columns of the voltage/current reports:
/// `%.1f`/`%.2f` formatting plus two independent solves round the last printed
/// digit independently, a purely *additive* floor far coarser than the `%g`
/// magnitude/pu columns (so the override is `rel = 0`, `abs ≈ 0.11` / `0.011`). A
/// *formatting* floor (documented in `tests/TOLERANCE_NOTES.md`), NOT a
/// relaxation of the magnitude/pu checks — those stay tight. The angle is
/// `arg(V)`/`arg(I)`, independent of the magnitude; its engine-physics
/// correctness is gated by the live model compare (`corpus_live.rs`, which
/// compares the complex node voltages / terminal currents directly), so here it
/// is only a report-layout / printing-floor check.
pub struct ColTol {
    pub sel: ColSel,
    pub rel: f64,
    pub abs: f64,
    /// Optional **denominator gate**: skip this cell only when the oracle's
    /// denominator is a *near-zero but nonzero* cancellation residual (see
    /// [`GateSpec`]). Used where a value is a ratio (`%I2/I1`) or the phase angle
    /// (`AngResid`) of a near-zero quantity — a faer-vs-KLU noise form carrying no
    /// information. The **`= 0` case is deliberately NOT gated**: an exactly-zero
    /// denominator prints as `0` (Pascal `if I1 > 0`) / the angle of an exact zero
    /// is `0.00`, which `0 == 0` checks perfectly — so only genuine-noise rows are
    /// skipped, not the many exactly-zero rows. The magnitude columns stay tightly
    /// checked on every row. A proven cancellation floor, NOT a relaxation
    /// (tests/TOLERANCE_NOTES.md).
    pub gate: Option<GateSpec>,
}

/// The denominator a [`ColTol::gate`] tests to decide whether to skip a cell.
#[derive(Clone, Copy)]
pub enum GateSpec {
    /// Skip when the oracle's value in a **fixed** column `col` is a near-zero
    /// residual `0 < |oracle[col]| < threshold`. For the symmetrical-component
    /// ratio columns (`%I2/I1`, `%I0/I1`, `%NEMA`) whose denominator `I1` sits in
    /// one fixed column.
    Col(usize, f64),
    /// Skip when `min(|oracle[a]|, |oracle[b]|) < threshold` (**including exact
    /// zero**). For the **power factor** column, which is a defined-but-degenerate
    /// value when the power is near-purely-reactive (`P ≈ 0`) or near-purely-real
    /// (`Q ≈ 0`): the oracle's `S.re`/`S.im` is *exactly* 0 there → `PowerFactor`
    /// returns unity `1.0000` (Pascal `Utilities.PowerFactor`'s `else` branch),
    /// while a faer-vs-KLU cancellation residual makes the tiny part nonzero and
    /// prints its own near-zero/sign-flipped PF. Gated on `min(|kW|, |kvar|)` — the
    /// PF is only meaningful when **both** P and Q are substantial.
    MinCols(usize, usize, f64),
    /// Skip when the oracle's value in the **immediately preceding** column is a
    /// near-zero residual `0 < |oracle[j-1]| < threshold`. For the paired
    /// magnitude/angle exports (`I, Ang, I, Ang, …`): the angle of a near-zero
    /// current/voltage (a residual or an open-terminal conductor) is faer-vs-KLU
    /// noise, gated on its own magnitude in the column just before it.
    PrevCol(f64),
    /// Skip when the oracle's value in column `col` is **above** `threshold` —
    /// the mirror of [`GateSpec::Col`], and **not** a tolerance concept at all.
    ///
    /// Its one use is a **deliberate divergence** confined to identifiable
    /// rows: the captured `Export SeqCurrents` carries an upstream indexing bug
    /// that prints *terminal 1's* residual on every terminal row, and both
    /// lanes fix it (GOLDEN_REBASE G2.2a) — so exactly the rows with
    /// `Terminal ≥ 2` are excluded (gate `ColAbove(1, 1.5)`), while every
    /// terminal-1 cell stays compared against the oracle. The excluded cells
    /// are pinned instead by their own expected-value test
    /// (`export_seqcurrents_iresidual_sums_the_rows_own_terminal`), which is
    /// why this is an *exclusion with a replacement gate*, not a relaxation.
    ColAbove(usize, f64),
    /// **Always** skip the matched column — a non-deterministic column that
    /// carries no comparable value (a wall-clock timestamp or an absolute path).
    /// Not a tolerance relaxation of any *value*: the column is genuinely
    /// unpinnable (`Summary`'s `DateTimeToStr(Now)`), documented in
    /// `tests/TOLERANCE_NOTES.md`.
    ///
    /// It carries a **second** role, the whole-column sibling of
    /// [`GateSpec::ColAbove`]: a column every one of whose cells diverges from
    /// the capture because a torn-down bug row moved it in both lanes, and
    /// whose rows no key in the report can separate. `export_busreliability`'s
    /// multi-meter `Duration` is the one such use (GOLDEN_REBASE G2.2a — each
    /// meter's duration loop now stays in its own zone, while the capture, from
    /// both gating engines, carries the cross-zone overwrite). That column is
    /// neither non-deterministic nor unpinnable: it is replaced bus-by-bus by
    /// its own expected-value test
    /// (`export_busreliability_multimeter_duration_stays_in_the_meters_zone`),
    /// which is what keeps this an exclusion-with-a-replacement rather than
    /// silence.
    Mask,
}

/// Policy for [`compare_export`].
pub struct ExportPolicy {
    /// Field separator: `','` for CSV, `'='` for the `Counts` key=value text.
    pub sep: char,
    /// Leading non-blank lines compared **verbatim** (fixed headers / column
    /// row), no number parsing.
    pub header_lines: usize,
    /// Row-set policy.
    pub rows: RowPolicy,
    /// Per-number relative / absolute tolerance for value fields (the default;
    /// overridden per column by [`ExportPolicy::col_tol`]).
    pub rel: f64,
    pub abs: f64,
    /// Per-column tolerance overrides, selected by [`ColSel`].
    pub col_tol: Vec<ColTol>,
}

impl ExportPolicy {
    /// The (`rel`, `abs`) tolerance for the field in column `j`: the first
    /// [`ColTol`] whose [`ColSel`] matches, else the default. `colnames` is the
    /// last header line split on the separator.
    fn tol_for_col(&self, j: usize, colnames: &[String], oracle_fields: &[String]) -> (f64, f64) {
        for ct in &self.col_tol {
            if ct.sel.matches(j, colnames, oracle_fields) {
                return (ct.rel, ct.abs);
            }
        }
        (self.rel, self.abs)
    }

    /// Whether column `j`'s cell should be skipped for this row because its
    /// [`ColTol::gate`] denominator (the oracle's `fields[col]`) is below the
    /// gate threshold — a ratio/angle of near-zero cancellation residuals (see
    /// [`ColTol::gate`]). Only the *matching* `ColTol`'s gate applies.
    fn skip_col(&self, j: usize, colnames: &[String], oracle_fields: &[String]) -> bool {
        for ct in &self.col_tol {
            if ct.sel.matches(j, colnames, oracle_fields) {
                // Band-limit: skip only a *nonzero* sub-threshold denominator
                // (`0 < |v| < thresh`). An exactly-zero denominator prints the
                // ratio as `0` (Pascal `if I1 > 0`) / the angle of an exact zero as
                // `0.00`, which `0 == 0` checks — so those rows stay verified.
                let num = |col: usize| {
                    oracle_fields
                        .get(col)
                        .and_then(|f| f.trim().parse::<f64>().ok())
                };
                let (col, thresh) = match ct.gate {
                    Some(GateSpec::Col(col, thresh)) => (col, thresh),
                    Some(GateSpec::PrevCol(thresh)) => (j.wrapping_sub(1), thresh),
                    Some(GateSpec::MinCols(a, b, thresh)) => {
                        return match (num(a), num(b)) {
                            (Some(x), Some(y)) => x.abs().min(y.abs()) < thresh,
                            _ => false,
                        };
                    }
                    Some(GateSpec::ColAbove(col, thresh)) => {
                        return num(col).is_some_and(|v| v > thresh);
                    }
                    Some(GateSpec::Mask) => return true,
                    None => return false,
                };
                return num(col).is_some_and(|v| v != 0.0 && v.abs() < thresh);
            }
        }
        false
    }
}

/// Split a report line into fields for [`compare_export`].
///
/// * `sep == ' '` selects the **fixed-width `Show` table** mode: tokenize on any
///   run of whitespace **or** commas, dropping empty tokens. Pascal's
///   `ShowResults` reports are space-padded columns (`Pad`/`PadDots`) with the
///   occasional glued trailing comma (`ExportLosses`-style `%.5f,` fields), so a
///   single-char split cannot tokenize them — this collapses the padding and
///   strips the comma so each numeric cell parses cleanly (PHASE8_PLAN §2.3).
/// * any other `sep` (`','` CSV, `'='` Counts key=value) splits on exactly that
///   char, trimming each field — the `Export`/CSV path, unchanged.
fn split_fields(line: &str, sep: char) -> Vec<String> {
    if sep == ' ' {
        line.split(|c: char| c.is_whitespace() || c == ',')
            // Drop empties **and pure dot-runs**: the `Show` reports pad name
            // columns with `PadDots` (`SOURCEBUS ..`, `650 ........`, the powers
            // `TERMINAL TOTAL ....`), and the pad width is `MaxBusNameLength`, a
            // backend quirk that differs per report (ShowVoltages floors it at 12,
            // ShowPowers at ~5 — even in isolation) and is *not* faithfully
            // reproducible with a single value. A dot-run is pure padding
            // punctuation carrying no data, so dropping it makes the token compare
            // immune to the quirk (tests/TOLERANCE_NOTES.md).
            .filter(|f| !f.is_empty() && !f.bytes().all(|b| b == b'.'))
            .map(|f| f.to_string())
            .collect()
    } else {
        line.split(sep).map(|f| f.trim().to_string()).collect()
    }
}

/// Split a report into non-blank, `\r`-stripped lines.
fn report_lines(s: &str) -> Vec<String> {
    s.lines()
        .map(|l| l.trim_end().to_string())
        .filter(|l| !l.trim().is_empty())
        .collect()
}

/// A field is numeric iff it parses *whole* as `f64` (so a class name like
/// `IndMach012` stays text while a count `2` is a number).
///
/// Stage F: a non-numeric field that is a **`Key=Value` script token** — the
/// shape of the `Dump`/`Save` DSS text, which a whitespace tokenizer cannot
/// split further — gets one extra chance in the **default lane** under an
/// **exact-value policy** only (see [`kv_value_eq`]).
fn field_eq(actual: &str, expected: &str, rel: f64, abs: f64, ctx: &str) {
    let (a, e) = (actual.trim(), expected.trim());
    match (a.parse::<f64>(), e.parse::<f64>()) {
        (Ok(_), Ok(_)) => assert_value_matches_tol(a, e, rel, abs, ctx),
        _ => {
            if a.eq_ignore_ascii_case(e) {
                return;
            }
            // Scoped to the *exact-value* policies, not merely to the lane.
            // `field_eq` is the leaf of every `compare_export`, including the
            // corpus gate's `global_result_policy` (rel 1e-10) and
            // `autoadd_log_policy` (energy floors); gated on the lane alone,
            // the first report combining a physical floor with `key=value`
            // cells would silently turn a verbatim text compare into a
            // tolerant numeric one. With `rel == abs == 0` the fallback means
            // exactly what its doc says: the f64 must be bit-identical, only
            // its spelling may move.
            if !lane::PARITY && rel == 0.0 && abs == 0.0 && kv_value_eq(a, e, rel, abs, ctx) {
                return;
            }
            panic!("{ctx}: text field differs (actual {a:?} vs expected {e:?})");
        }
    }
}

/// Stage F **default-lane** fallback for a `Key=Value` token (`~ R=1.1`,
/// `kV=12.47`): compare the key verbatim (case-insensitively) and the value as
/// a number, so a pure *rendering* change (F-FMT, Part IV.2) passes while the
/// key, the token structure and the value itself stay pinned — the byte-golden
/// policies pass `rel = abs = 0`, so "as a number" means bit-identical `f64`.
///
/// Returns `false` (→ the caller's verbatim compare fails, as before) unless
/// **both** sides are `key=<number>` with matching keys; a differing value
/// panics from here with the key in the context. The parity lane never calls
/// this — it keeps the verbatim token compare.
fn kv_value_eq(actual: &str, expected: &str, rel: f64, abs: f64, ctx: &str) -> bool {
    let (Some((ak, av)), Some((ek, ev))) = (actual.split_once('='), expected.split_once('='))
    else {
        return false;
    };
    if !ak.eq_ignore_ascii_case(ek) {
        return false;
    }
    let (av, ev) = (av.trim(), ev.trim());
    if av.parse::<f64>().is_err() || ev.parse::<f64>().is_err() {
        return false;
    }
    assert_value_matches_tol(av, ev, rel, abs, &format!("{ctx} [{ak}=]"));
    true
}

/// Compare two report bodies (oracle vs Rust) per [`ExportPolicy`] (§2.3).
pub fn compare_export(oracle: &str, rust: &str, policy: &ExportPolicy, ctx: &str) {
    let ol = report_lines(oracle);
    let rl = report_lines(rust);
    assert!(
        ol.len() >= policy.header_lines && rl.len() >= policy.header_lines,
        "{ctx}: file shorter than the {} header line(s)",
        policy.header_lines
    );
    // Fixed header block: verbatim.
    for i in 0..policy.header_lines {
        assert_eq!(rl[i], ol[i], "{ctx}: header line {i} differs");
    }
    let split = |line: &str| -> Vec<String> { split_fields(line, policy.sep) };
    // Column names = the last header line split on the separator (for the
    // per-column tolerance lookup). Empty when the report has no header block.
    let colnames: Vec<String> = if policy.header_lines >= 1 {
        split(&ol[policy.header_lines - 1])
    } else {
        Vec::new()
    };
    let odata = &ol[policy.header_lines..];
    let rdata = &rl[policy.header_lines..];

    match &policy.rows {
        RowPolicy::ExactOrdered => {
            assert_eq!(
                rdata.len(),
                odata.len(),
                "{ctx}: data row count differs (rust {} vs oracle {})",
                rdata.len(),
                odata.len()
            );
            for (i, (r, o)) in rdata.iter().zip(odata).enumerate() {
                let (rf, of) = (split(r), split(o));
                assert_eq!(rf.len(), of.len(), "{ctx}: row {i} field count differs");
                for (j, (a, e)) in rf.iter().zip(&of).enumerate() {
                    if policy.skip_col(j, &colnames, &of) {
                        continue;
                    }
                    let (rel, abs) = policy.tol_for_col(j, &colnames, &of);
                    field_eq(a, e, rel, abs, &format!("{ctx}: row {i} field {j}"));
                }
            }
        }
        RowPolicy::RustSubsetByKey {
            key,
            require,
            allow_extra,
        } => {
            // Oracle rows keyed by the lowercased key field.
            let mut omap: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for o in odata {
                let f = split(o);
                if let Some(k) = f.get(*key) {
                    omap.insert(k.to_lowercase(), f);
                }
            }
            let mut rust_keys: BTreeSet<String> = BTreeSet::new();
            for (i, r) in rdata.iter().enumerate() {
                let rf = split(r);
                let k = rf
                    .get(*key)
                    .unwrap_or_else(|| panic!("{ctx}: rust row {i} has no key field"));
                rust_keys.insert(k.to_lowercase());
                // A 0.15.x-only class the pinned 0.14.5 oracle cannot have
                // (WP-U1.8 WindGen): skip it here — gated against capi015 instead.
                if allow_extra.contains(&k.to_lowercase()) && !omap.contains_key(&k.to_lowercase())
                {
                    continue;
                }
                let of = omap.get(&k.to_lowercase()).unwrap_or_else(|| {
                    panic!("{ctx}: rust row {i} key {k:?} absent from the oracle file")
                });
                assert_eq!(
                    rf.len(),
                    of.len(),
                    "{ctx}: row key {k:?} field count differs"
                );
                for (j, (a, e)) in rf.iter().zip(of).enumerate() {
                    if policy.skip_col(j, &colnames, of) {
                        continue;
                    }
                    let (rel, abs) = policy.tol_for_col(j, &colnames, of);
                    field_eq(a, e, rel, abs, &format!("{ctx}: row {k:?} field {j}"));
                }
            }
            // Presence guard: a subset compare that iterates only the Rust rows
            // cannot see a *dropped* row, so an empty/under-reporting Rust body
            // would pass silently. Require the must-emit keys explicitly.
            for k in require {
                assert!(
                    rust_keys.contains(&k.to_lowercase()),
                    "{ctx}: required key {k:?} missing from the Rust output \
                     (a dropped class / empty report body)"
                );
            }
        }
    }
}
