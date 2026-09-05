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

/// `GOLDEN_REBASE_PLAN.md` WP-G1 sub-step G1.9: the five `Circuit` aggregates
/// and the ten `Solution` scalars — capture structs, the shared per-element
/// loss envelope ([`aggregates::element_loss_allowance_kw`], which
/// [`compare_element_channels`] itself calls) and the two live comparators.
pub mod aggregates;

/// `GOLDEN_REBASE_PLAN.md` WP-G1 sub-step G1.7: the `ITopology` interface —
/// `NumLoops`, the two isolation counts and the three identifier lists — as a
/// capture struct, the gate-side statement of the two transport shape
/// normalizations and one fully discrete, zero-tolerance live comparator.
pub mod topology;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
// GOLDEN_REBASE G1.6b: the PDElements skip-row and per-channel walk counters
// (`PD_SKIP_VISITS` …), spelled like `props_norm`'s.
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrd};

use dss_core::exec::{Dss, ElementSnapshot, MeterReliabilityView, PdElementView};
use dss_core::support::complexutil::Polar;
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
///
/// The last seven fields are the `GOLDEN_REBASE_PLAN.md` G1.3a **derived polar
/// channels**, present only when the case's `compare_derived` manifest flag is
/// on — `enabled` for every element, the six arrays for *enabled* elements only
/// (a never-enabled element has no `NodeRef`, where r4133 `VoltagesMagAng`
/// dereferences nil and capi answers a one-element sentinel). Both transports
/// emit exactly this shape (`tools/oracle/oracle_server.py`,
/// `crates/dss-epri/src/capture.rs`), de-interleaved into magnitude and angle
/// the way `i_re`/`i_im` already are, so the comparator never does stride-2
/// index arithmetic. All seven are `serde(default)`: an off-flag reply and
/// every committed checkpoint golden simply leave them absent.
///
/// The **last five** are the G1.3d(i) **discrete index/name extras**, present
/// only when the case's `compare_element_extras` manifest flag is on — the three
/// counts and `EnergyMeter` for *every* element (all four are pure field reads
/// on both engines), `NodeOrder` only for an element that is `Enabled` **and**
/// has `NumTerminals > 0` (neither transport survives a nil `NodeRef`). They are
/// `serde(default)` for the same reason as the seven above, and they are
/// compared exactly, by [`compare_element_extras`], with no tolerance and no
/// ledger sub-channel.
///
/// The **last seven** are the G1.3d(ii) additions, riding the same
/// `compare_element_extras` flag: `PhaseLosses` (`pl_kw`/`pl_kvar` — the one
/// *numeric* member of the group, compared by
/// [`compare_element_phase_losses`] at a band derived from the per-conductor
/// power band) and the five control-derived discrete scalars, which
/// [`compare_element_extras`] compares exactly beside the other five. Each is
/// `serde(default)` for the same reason as everything above it.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct ElementCap {
    pub name: String,
    pub i_re: Vec<f64>,
    pub i_im: Vec<f64>,
    pub p_kw: Vec<f64>,
    pub p_kvar: Vec<f64>,
    #[serde(default)]
    pub loss_w: Vec<f64>,
    /// `CktElement.Enabled` (r4133 `DDLL/DCktElement.pas:263`, the `CktElementI`
    /// read arm) — captured for EVERY element under the flag, so the
    /// enabled-only polar capture can never silently drop one. Structure, not a
    /// value channel: [`compare_element_derived`] compares it exactly and no
    /// sub-channel selector can mask it.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// `CktElement.CurrentsMagAng` magnitudes (A), conductor-minor inside
    /// terminal-major — r4133 `DDLL/DCktElement.pas:1058` (mode `18`), capi
    /// `CAPI/CAPI_Alt.pas:1043`; a fastdss `_columns` surface
    /// (`dss/ICktElement.py:64` on `origin/fastdss`).
    #[serde(default)]
    pub cma_mag: Vec<f64>,
    /// `CurrentsMagAng` angles (degrees, `(-180, 180]` — `CDANG`, r4133
    /// `Shared/Ucomplex.pas:118`).
    #[serde(default)]
    pub cma_ang: Vec<f64>,
    /// `CktElement.Residuals` magnitudes (A), one per **terminal** — r4133
    /// `DDLL/DCktElement.pas:827` (mode `11`, the `k := (i-1)*Nconds` offset at
    /// `:842`), capi `CAPI/CAPI_CktElement.pas:541` (offset `:562`); fastdss
    /// `dss/ICktElement.py:67`.
    #[serde(default)]
    pub res_mag: Vec<f64>,
    /// `Residuals` angles (degrees).
    #[serde(default)]
    pub res_ang: Vec<f64>,
    /// `CktElement.VoltagesMagAng` magnitudes (V), same conductor layout as
    /// `cma_mag` — r4133 `DDLL/DCktElement.pas:1082` (mode `19`), capi
    /// `CAPI/CAPI_Alt.pas:1072`; fastdss `dss/ICktElement.py:58`.
    #[serde(default)]
    pub vma_mag: Vec<f64>,
    /// `VoltagesMagAng` angles (degrees).
    #[serde(default)]
    pub vma_ang: Vec<f64>,
    /// `CktElement.NumTerminals` (`NTerms`) — r4133
    /// `DDLL/DCktElement.pas:139` (`CktElementI` mode `0`), capi
    /// `CAPI/CAPI_CktElement.pas:202`; a fastdss `_columns` surface
    /// (`dss/ICktElement.py` on `origin/fastdss`). Captured for EVERY element
    /// under the flag, which is why its absence is what
    /// [`compare_element_extras`] reads as "this element was captured without
    /// the flag" (the element-level twin of [`capture_guard::require_capture`]).
    #[serde(default)]
    pub n_terms: Option<i32>,
    /// `CktElement.NumConductors` (`NConds`) — r4133 `DDLL/DCktElement.pas:144`
    /// (mode `1`), capi `CAPI/CAPI_CktElement.pas:182`.
    #[serde(default)]
    pub n_conds: Option<i32>,
    /// `CktElement.NumPhases` (`NPhases`) — r4133 `DDLL/DCktElement.pas:149`
    /// (mode `2`), capi `CAPI/CAPI_CktElement.pas:192`. Not derivable from the
    /// other two: `NConds` is `NPhases` plus the neutral conductors.
    #[serde(default)]
    pub n_phases: Option<i32>,
    /// `CktElement.EnergyMeter`, **raw**: each transport's own "no meter"
    /// spelling is preserved here and normalized by [`oracle_meter_name`] —
    /// capi returns `NIL` (`CAPI/CAPI_CktElement.pas:672-687`), which the pinned
    /// dss-python renders as the empty string, and r4133 returns the
    /// `CktElementS` family default `'0'` (`DDLL/DCktElement.pas:421`, left
    /// untouched by arm `4` at `:442-449` when `HasEnergyMeter` is clear).
    #[serde(default)]
    pub energy_meter: Option<String>,
    /// `CktElement.NodeOrder`: the bus-local node number of every conductor
    /// slot, conductor-minor inside terminal-major (length `NTerms · NConds`),
    /// ground = `0` — r4133 `DDLL/DCktElement.pas:1032` (`CktElementV` mode
    /// `17`, the `GetNodeNum(NodeRef^[j])` map at `:1048`), capi
    /// `CAPI/CAPI_CktElement.pas:885`. Emitted only for an element that is `Enabled`
    /// **and** has `NumTerminals > 0`; [`compare_element_extras`] asserts the
    /// resulting shape on both sides rather than assuming it.
    ///
    /// Unrelated to the checkpoint-level `node_order` of
    /// `crates/dss-epri/src/capture.rs::CaseResult`, which is the **Y node name
    /// order** of the whole circuit; the two live in different JSON objects and
    /// share nothing but the word.
    #[serde(default)]
    pub node_order: Vec<i32>,
    /// `CktElement.PhaseLosses` active halves (**kW**), de-interleaved the way
    /// `p_kw`/`p_kvar` already are: the per-**phase** complex loss
    /// `Σ_terminals NodeV[NodeRef[k]]·conj(Iterminal[k])` at `k = j·NConds + i`,
    /// neutral conductors ignored, ×3 under positive sequence — r4133
    /// `Common/CktElement.pas:1078-1120` (`TDSSCktElement.GetPhaseLosses`),
    /// capi `src/Common/CktElement.pas:879-912`. A fastdss `_columns` surface
    /// (`dss/ICktElement.py:69` on `origin/fastdss`; its harness subtracts only
    /// `Handle`, `IsIsolated` and `HasOCPDevice`, `tests/save_outputs.py:169`).
    ///
    /// **kW/kvar here, W/var in the engine.** Both transports scale by `0.001`
    /// at the API boundary — r4133 `DDLL/DCktElement.pas:637-658` (`CktElementV`
    /// mode `6`, the `cmulreal(…, 0.001)` at `:651`), capi
    /// `CAPI/CAPI_Alt.pas:449-467` (the `*= 0.001` loop at `:462-465`, facade
    /// `CAPI/CAPI_CktElement.pas:327-338`) — so the conversion lives in exactly
    /// one place, [`compare_element_phase_losses`], like the re/im interleave.
    ///
    /// Length `NPhases`, and **empty is a reading rather than an absence**: a
    /// 0-phase element (`UPFCControl`, r4133
    /// `Controls/UPFCControl.pas:230-246`) legitimately has none — which is why
    /// the pair is `Vec` and not `Option`, and why the runner's non-vacuity rail
    /// counts the elements whose `pl_kw` is *non-empty*.
    #[serde(default)]
    pub pl_kw: Vec<f64>,
    /// `PhaseLosses` reactive halves (**kvar**) — see [`Self::pl_kw`]. The two
    /// lengths are asserted equal on the oracle side, so the capi
    /// `DefaultResult` one-element sentinel (`CAPI/CAPI_Utils.pas:212-221`,
    /// reachable through `Alt_CE_Get_PhaseLosses`' `MissingSolution` guard,
    /// `CAPI/CAPI_Alt.pas:455-459`) fails loudly instead of de-interleaving into
    /// a silent `mag = [0.0]`, `ang = []` pair. It was never observed on a
    /// solved case, so it is deliberately not normalized.
    #[serde(default)]
    pub pl_kvar: Vec<f64>,
    /// `CktElement.NumControls` = `ControlElementList.ListSize`, with **no**
    /// `Enabled` filter on either channel — r4133 `DDLL/DCktElement.pas:237-241`
    /// (`CktElementI` mode `9`), capi `CAPI/CAPI_CktElement.pas:939-948`;
    /// fastdss `dss/ICktElement.py:38`. Captured for EVERY element under the
    /// flag.
    #[serde(default)]
    pub num_controls: Option<i32>,
    /// `CktElement.OCPDevIndex` — the **1-based** position in
    /// `ControlElementList` of the first Fuse/Recloser/Relay, `0` when there is
    /// none: r4133 `DDLL/DCktElement.pas:242-258` (mode `10`), capi
    /// `CAPI/CAPI_CktElement.pas:951-976`; fastdss `dss/ICktElement.py:50`.
    #[serde(default)]
    pub ocp_dev_index: Option<i32>,
    /// `CktElement.OCPDevType` — `GetOCPDeviceType`'s code for that same first
    /// OCP member (`1` Fuse, `2` Recloser, `3` Relay, `0` none): r4133
    /// `Common/Utilities.pas:3165-3184` reached from
    /// `DDLL/DCktElement.pas:259-262` (mode `11`), capi
    /// `CAPI/CAPI_CktElement.pas:978-988`; fastdss `dss/ICktElement.py:49`.
    /// Neither scan has an `Enabled` test, so a **disabled** OCP control still
    /// wins it (measured on both channels, G1.3d(ii) §1.3-1).
    #[serde(default)]
    pub ocp_dev_type: Option<i32>,
    /// `CktElement.HasVoltControl` — "any member of `ControlElementList` is a
    /// `CAP_CONTROL` or a `REG_CONTROL`": r4133 `DDLL/DCktElement.pas:222-236`
    /// (mode `8`), capi `CAPI/CAPI_CktElement.pas:689-710`; fastdss
    /// `dss/ICktElement.py:46`.
    #[serde(default)]
    pub has_volt_control: Option<bool>,
    /// `CktElement.HasSwitchControl` — "any member is a `SWT_CONTROL`": r4133
    /// `DDLL/DCktElement.pas:207-221` (mode `7`), capi
    /// `CAPI/CAPI_CktElement.pas:713-734`; fastdss `dss/ICktElement.py:47`.
    #[serde(default)]
    pub has_switch_control: Option<bool>,
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
    /// `CurrentsMagAng` — the polar rendering of `currents`
    /// (`GOLDEN_REBASE_PLAN.md` G1.3a).
    pub currents_mag_ang: bool,
    /// `VoltagesMagAng` — `NodeV` read through the element's own `NodeRef`.
    pub voltages_mag_ang: bool,
    /// `Residuals` — the per-terminal conductor sum of `currents`.
    pub residuals: bool,
    /// `PhaseLosses` — the same `S = V·conj(I)` product as `powers`, bucketed by
    /// phase (`GOLDEN_REBASE_PLAN.md` G1.3d(ii)). Shares `powers`/`losses`'
    /// exclusion cause, not the polar channels' exemption: see
    /// [`Self::CURRENTS_ONLY`].
    pub phase_losses: bool,
}

impl ElemChannels {
    /// Every sub-channel — what every caller but the Stage F lane policy uses.
    pub const ALL: Self = Self {
        currents: true,
        powers: true,
        losses: true,
        currents_mag_ang: true,
        voltages_mag_ang: true,
        residuals: true,
        phase_losses: true,
    };
    /// Currents only: the `S = V·conj(I)` channels are a deliberate divergence
    /// in this lane and are pinned by their own expected-value test instead.
    ///
    /// The three G1.3a derived channels stay **on** here, deliberately: the
    /// Newton staleness lives in the cache-aware read path
    /// (`Get_Powers`/`Get_Losses` reusing `ComputeIterminal`, r4133
    /// `Common/CktElement.pas:632-640`), while `Currents` — and therefore
    /// `CurrentsMagAng` and `Residuals`, which are renderings of it — come from
    /// a fresh `GetCurrents`, and `VoltagesMagAng` reads `NodeV` and never
    /// touches `Iterminal` at all. So the two `newton*` decks *gain* three
    /// compared channels.
    ///
    /// # G1.3d(ii): `PhaseLosses` DOES join the exclusion
    ///
    /// `GetPhaseLosses` opens with the very same cache-aware `ComputeIterminal`
    /// (r4133 `Common/CktElement.pas:1090`, capi `src/Common/CktElement.pas:896`)
    /// that `Get_Powers`/`Get_Losses` reuse, and forms the identical
    /// `NodeV·conj(Iterminal)` products — merely bucketed by phase instead of
    /// summed. So no oracle channel reports it at the converged `NodeV` on those
    /// two decks either, and it is dropped here for the same
    /// `POWERS_REUSE_STALE_NEWTON_ITERMINAL` teardown row, in **both** lanes. Only the *value* compare is dropped:
    /// [`compare_element_phase_losses`] asserts the array shapes under every
    /// channel policy, and the port-side identity `Σ_i PhaseLosses[i] = Losses`
    /// is pinned in-engine by
    /// `dss_core::exec::tests::element_extras::phase_losses_are_watts_and_sum_to_get_losses`
    /// beside the row's own `newton_powers_match_the_normal_algorithm`.
    pub const CURRENTS_ONLY: Self = Self {
        currents: true,
        powers: false,
        losses: false,
        currents_mag_ang: true,
        voltages_mag_ang: true,
        residuals: true,
        phase_losses: false,
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
    // — no new tolerance class, just the conductor policy summed. Since
    // GOLDEN_REBASE G1.9 that sum lives in
    // `aggregates::element_loss_allowance_kw`, so the circuit-aggregate
    // comparator propagates the identical envelope instead of a second one.
    if channels.losses && exp.loss_w.len() == 2 {
        let allowed_kw = aggregates::element_loss_allowance_kw(exp, tol);
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

/// Degrees per radian at **full** f64 precision.
///
/// A tolerance is not a printed value: the truncated `57.29577951` that `CDANG`
/// multiplies by (r4133 `Shared/Ucomplex.pas:118`, `TRUNCATED_RAD_TO_DEG` in
/// `dss_core::support::complexutil`) belongs inside the kernel that *renders* an
/// angle, never inside the band that *judges* one. This is the same constant the
/// `compare_monitor` angle companion term uses (tests/TOLERANCE_NOTES.md
/// §monitor-f32-floor).
const POLAR_RAD_TO_DEG: f64 = 57.29577951308232;

/// `|a − b|` for two angles in degrees, taken **on the circle**: the raw
/// difference is folded into `[−180, 180]` first.
///
/// `CDANG` returns `(−180, 180]`, so a phasor sitting astride the negative real
/// axis reads `+179.99999999032846 °` on one engine and `−179.99999999032846 °`
/// on the other for an imaginary part of ±1e-18 — a raw gap of
/// `359.9999999806569 °` for a physical gap of `1.9343133317306638e-08 °`
/// (measured, pinned by `the_angle_comparison_is_wrap_aware`). Wrapping is not a
/// relaxation, it is what "angle" means: a genuine sign flip still measures a
/// full `180 °`, which no band this module emits can admit — the angle band's
/// ceiling is [`POLAR_RAD_TO_DEG`] (see [`polar_angle_band`]).
///
/// NaN propagates (`NaN <= band` is false), so a NaN angle fails loudly instead
/// of slipping through.
pub fn wrapped_deg(diff: f64) -> f64 {
    ((diff + 180.0).rem_euclid(360.0) - 180.0).abs()
}

/// The angular image of an accepted magnitude band: how far the *argument* of a
/// phasor may move when its magnitude is known only to ±`allowed_mag`.
///
/// The set inherited here is a **disc**, not a per-component rectangle:
/// [`assert_complex_close_c`] bands the *modulus* of the complex difference
/// (`|Δz| ≤ abs + rel·|z|`, the `diff`/`allowed` lines of that function), so
/// `arg` maps it onto exactly `±asin(allowed_mag/|z|)` radians. What this
/// function returns is that image's **linearization**,
/// `rad2deg · allowed_mag/|z|` degrees — the same construction as the
/// voltage-scaled power floor (`assert_power_close`) and the `compare_monitor`
/// angle companion term. Since `asin(x) ≥ x` the linearization is never
/// *looser* than the exact image: the angle channel can only be stricter than
/// the complex band it renders, never more permissive (pinned by
/// `the_angle_band_is_the_conservative_linearization_of_its_exact_image`; the
/// gap `asin(x)/x − 1 = x²/6 + O(x⁴)` is under one f64 ulp at every magnitude
/// the corpus judges). At healthy magnitudes the band is far *tighter* than any
/// base band (6.6e-6 ° at 683 A on the feeder tier).
///
/// Returns `None` when `|oracle_mag| ≤ allowed_mag`: there the phasor is
/// indistinguishable from zero at the accepted precision and its angle carries
/// **no** information — the two oracle channels report angles up to 180 ° apart
/// for one and the same ~1e-12 A current. The magnitude is never masked, so the
/// channel stays two-sided. And because the mask fires exactly where the image
/// would reach `rad2deg · 1`, the emitted band can never exceed
/// [`POLAR_RAD_TO_DEG`] by construction (pinned by
/// `the_angle_band_never_exceeds_one_radian_in_degrees`).
pub fn polar_angle_band(allowed_mag: f64, oracle_mag: f64) -> Option<f64> {
    let m = oracle_mag.abs();
    (m > allowed_mag).then(|| POLAR_RAD_TO_DEG * allowed_mag / m)
}

/// One polar channel compared sample-by-sample against its de-interleaved oracle
/// arrays, with a caller-supplied magnitude band per sample.
///
/// Lengths are asserted here as well as in [`compare_element_derived`]: this
/// helper is the one place a mismatched pair could otherwise compare a prefix.
fn polar_close_with(
    actual: &[Polar],
    om: &[f64],
    oa: &[f64],
    allowed_mag: impl Fn(usize) -> f64,
    ctx: &str,
) {
    assert_eq!(
        om.len(),
        oa.len(),
        "{ctx}: the oracle magnitude ({}) and angle ({}) arrays disagree",
        om.len(),
        oa.len()
    );
    assert_eq!(
        actual.len(),
        om.len(),
        "{ctx}: length mismatch (rust {} vs oracle {})",
        actual.len(),
        om.len()
    );
    for (k, (a, (m, ang))) in actual.iter().zip(om.iter().zip(oa)).enumerate() {
        let allowed = allowed_mag(k);
        let dm = (a.mag - m).abs();
        assert!(
            dm <= allowed,
            "{ctx} [{k}]: magnitude {} vs oracle {m} (|diff| = {dm:e} > allowed {allowed:e})",
            a.mag
        );
        // Below its own magnitude band the phasor has no argument to compare —
        // documented mask, tests/TOLERANCE_NOTES.md §G1.3a.
        let Some(allowed_ang) = polar_angle_band(allowed, *m) else {
            continue;
        };
        let da = wrapped_deg(a.ang - ang);
        assert!(
            da <= allowed_ang,
            "{ctx} [{k}]: angle {} deg vs oracle {ang} deg (wrapped |diff| = {da:e} > \
             allowed {allowed_ang:e} at |mag| = {m:e})",
            a.ang
        );
    }
}

/// A polar channel whose re/im original is already gated at
/// `|Δz| ≤ abs + rel·|z|` — the **disc** [`assert_complex_close_c`] admits, not
/// a per-component rectangle (pinned by
/// `the_inherited_current_band_is_a_disc_not_a_rectangle`).
///
/// The magnitude inherits that band exactly — `||a| − |b|| ≤ |a − b|` (reverse
/// triangle inequality), and the bound is attained at `a = z(1 ± ρ/|z|)`, so the
/// magnitude channel admits neither more nor less than the disc does — and the
/// angle gets its angular image ([`polar_angle_band`]). Nothing is calibrated
/// here; both floors are images of the already-gated `i_rel/i_abs` and
/// `v_rel/v_abs` tiers (tests/TOLERANCE_NOTES.md §G1.3a).
pub fn polar_close(actual: &[Polar], om: &[f64], oa: &[f64], rel: f64, abs: f64, ctx: &str) {
    polar_close_with(actual, om, oa, |k| abs + rel * om[k].abs(), ctx);
}

/// The magnitude band of terminal `t`'s `Residuals` sample: the residual is the
/// **sum** of that terminal's `nconds` conductor currents
/// (r4133 `DDLL/DCktElement.pas:842`), so its band is the sum of their bands —
/// `|δ(Σ_c I_c)| ≤ Σ_c (abs + rel·|I_c|) = nconds·abs + rel·Σ_c|I_c|`.
///
/// Exactly the derivation `compare_element_channels` already uses for
/// `Get_Losses` (`losses = Σ_k S_k`), transplanted to the current sum: no new
/// tolerance class, just the conductor policy summed. The bound is *exact*, not
/// slack — the Minkowski sum of the conductors' discs is the disc of the summed
/// radius, attained when their errors are collinear. It matters because the
/// residual is a near-cancellation by construction (≈0 on a balanced terminal).
pub fn residual_band(exp: &ElementCap, t: usize, nconds: usize, rel: f64, abs: f64) -> f64 {
    let sum: f64 = (0..nconds)
        .map(|c| {
            let k = t * nconds + c;
            exp.i_re[k].hypot(exp.i_im[k])
        })
        .sum();
    nconds as f64 * abs + rel * sum
}

/// `Residuals` compared per terminal at the conductor-sum band
/// ([`residual_band`]), angles wrap-aware and masked below their own band.
pub fn residual_close(actual: &[Polar], exp: &ElementCap, rel: f64, abs: f64, ctx: &str) {
    let nterms = exp.res_mag.len();
    assert!(nterms > 0, "{ctx}: the oracle reports no terminal");
    assert_eq!(
        exp.i_re.len() % nterms,
        0,
        "{ctx}: {} conductor slots do not divide into {nterms} terminals",
        exp.i_re.len()
    );
    let nconds = exp.i_re.len() / nterms;
    polar_close_with(
        actual,
        &exp.res_mag,
        &exp.res_ang,
        |t| residual_band(exp, t, nconds, rel, abs),
        ctx,
    );
}

/// Does this *oracle* polar channel say "nothing to report"?
///
/// Two shapes mean that, and they are the two the transports actually produce:
///
/// * **empty** — r4133's answer and the engine's. r4133 has no nil-`NodeRef`
///   guard on mode 19, but it sizes the result at `NConds·Nterms` and its
///   `for i := 1 to numcond` loop never runs when that is 0
///   (`DDLL/DCktElement.pas:1082-1100`), so a 0-terminal element yields a
///   0-length array;
/// * the capi **`DefaultResult` sentinel** — with `DSS_CAPI_COM_DEFAULTS` on
///   (dss-python's default) a guarded read hands back a **one-element `[0.0]`**
///   array (`CAPI/CAPI_Utils.pas:212-221`), which de-interleaves into
///   `mag = [0.0]`, `ang = []`. `Alt_CE_Get_VoltagesMagAng` takes that path
///   whenever `elem.NodeRef = NIL` (`CAPI/CAPI_Alt.pas:1080-1084`) — which on an
///   element with no terminals is unconditional — and
///   `Alt_CE_Get_CurrentsMagAng` (`:1043-1052`) / `Alt_CE_Get_Residuals` reach
///   the same sentinel through their `MissingSolution` guard.
///
/// A capture-boundary **sentinel shape**, therefore, not a divergence: it is
/// normalized here (coordinator decision D4, the `PROPS_NORM_R4133` precedent)
/// for 0 ledger rows, and pinned by
/// `derived_polar_floors::the_capi_default_result_sentinel_reads_as_no_payload`.
/// It is consulted **only** where a real payload would have length
/// `yorder = 0`, so the sentinel can never hide a reading, and only on the
/// oracle side — the engine has no sentinel and must be strictly empty.
fn no_polar_payload(mag: &[f64], ang: &[f64]) -> bool {
    (mag.is_empty() || mag == [0.0]) && ang.is_empty()
}

/// Compare one element's **derived polar channels** against a capture:
/// `Enabled`, `CurrentsMagAng`, `VoltagesMagAng`, `Residuals`
/// (`GOLDEN_REBASE_PLAN.md` WP-G1 G1.3a).
///
/// Runs only when the case's `compare_derived` manifest flag is on, *alongside*
/// — never instead of — [`compare_element_channels`].
///
/// **Structure, asserted under every channel policy** (a value exclusion may
/// never excuse a shape or an existence miss — the rule
/// [`compare_element_channels`] already follows): the element exists in the Rust
/// snapshot; `Enabled` matches exactly; a **disabled** element carries no oracle
/// payload at all (asserted here — the *port* keeps the shape and reads zero
/// there, which is not an oracle-comparable fact and is pinned in-engine by
/// `exec::tests::derived_polar::a_never_enabled_element_has_no_polar_payload`);
/// a **0-terminal** element, which `UPFCControl` legitimately is (r4133
/// `Version8/Source/Controls/UPFCControl.pas:230-246`), carries no payload on
/// **either** side, up to the capi sentinel shape ([`no_polar_payload`]); and
/// every array length matches on both sides. The
/// enabled-only capture is what makes the two oracle transports agree in shape —
/// a never-enabled element has no `NodeRef`, where r4133 dereferences nil
/// (`DDLL/DCktElement.pas:1099`, no guard) and capi returns a one-element
/// sentinel (`CAPI/CAPI_Alt.pas:1081`) — so no sentinel normalization is owed,
/// and `enabled` being compared exactly is what keeps that skip honest.
///
/// **Floors** (derivations in tests/TOLERANCE_NOTES.md §G1.3a):
/// `CurrentsMagAng` inherits the already-gated complex current band
/// (`i_abs + i_rel·|I|`), `VoltagesMagAng` the node-voltage band
/// (`v_abs + v_rel·|V|`), `Residuals` the conductor-sum band
/// ([`residual_band`]); every angle gets the angular image of its own magnitude
/// band, wrap-aware. No tolerance is calibrated here.
pub fn compare_element_derived(
    snaps: &[ElementSnapshot],
    exp: &ElementCap,
    tol: &Tolerances,
    ctx: &str,
    channels: ElemChannels,
) {
    let snap = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&exp.name))
        .unwrap_or_else(|| panic!("{ctx}: no element {}", exp.name));
    // `enabled` is emitted for EVERY element under the flag; its absence means
    // the capture ran without the flag, which the comparator must never paper
    // over (the element-level twin of `capture_guard::require_capture`).
    let enabled = exp.enabled.unwrap_or_else(|| {
        panic!(
            "{ctx} {}: the derived capture carries no `enabled` field — the case's \
             `compare_derived` flag is on but this element was captured without it",
            exp.name
        )
    });
    assert_eq!(
        snap.enabled, enabled,
        "{ctx} {}: Enabled differs (rust {} vs oracle {enabled})",
        exp.name, snap.enabled
    );
    if !enabled {
        assert!(
            exp.cma_mag.is_empty()
                && exp.cma_ang.is_empty()
                && exp.vma_mag.is_empty()
                && exp.vma_ang.is_empty()
                && exp.res_mag.is_empty()
                && exp.res_ang.is_empty(),
            "{ctx} {}: a disabled element must carry no polar payload \
             (cma {}/{}, vma {}/{}, res {}/{}) — the capture read channels it \
             must skip",
            exp.name,
            exp.cma_mag.len(),
            exp.cma_ang.len(),
            exp.vma_mag.len(),
            exp.vma_ang.len(),
            exp.res_mag.len(),
            exp.res_ang.len()
        );
        return;
    }
    let yorder = exp.i_re.len();
    let nterms = snap.bus_names.len();
    // A **0-terminal** element is legitimate, and every engine agrees on it:
    // `TUPFCControlObj.Create` (r4133 `Version8/Source/Controls/UPFCControl.pas:230-246`
    // — capi 0.14.5 `src/Controls/UPFCControl.pas:151-164` is the same code) never
    // assigns `Nterms`/`Nphases`/`Setbus`, unlike every other control class
    // (`Controls/CapControl.pas:481-483`: `Nterms := 1; // this forces allocation
    // of terminals and conductors in base class`) — that class sets them only in
    // `MakePosSequence` (`UPFCControl.pas:284-293`). So an enabled `UPFCControl`
    // carries `Nterms = 0`, `Yorder = 0` and six legitimately empty channels.
    // Accepted **two-sidedly**, like the disabled branch above: an element that
    // grows a payload on either side still fails, so the acceptance can never
    // swallow a real capture. The one shape the oracle side may add is the capi
    // `DefaultResult` sentinel — see [`no_polar_payload`].
    if yorder == 0 && nterms == 0 {
        assert!(
            no_polar_payload(&exp.cma_mag, &exp.cma_ang)
                && no_polar_payload(&exp.vma_mag, &exp.vma_ang)
                && no_polar_payload(&exp.res_mag, &exp.res_ang)
                && snap.currents_mag_ang.is_empty()
                && snap.voltages_mag_ang.is_empty()
                && snap.residuals.is_empty(),
            "{ctx} {}: a 0-terminal element must carry no polar payload on either \
             side (oracle cma {}/{}, vma {}/{}, res {}/{}; rust cma {}, vma {}, \
             res {})",
            exp.name,
            exp.cma_mag.len(),
            exp.cma_ang.len(),
            exp.vma_mag.len(),
            exp.vma_ang.len(),
            exp.res_mag.len(),
            exp.res_ang.len(),
            snap.currents_mag_ang.len(),
            snap.voltages_mag_ang.len(),
            snap.residuals.len()
        );
        return;
    }
    assert!(
        nterms > 0 && yorder.is_multiple_of(nterms),
        "{ctx} {}: {yorder} conductor slots do not divide into {nterms} terminals",
        exp.name
    );
    let shape = |what: &str, om: &[f64], oa: &[f64], rust: usize, want: usize| {
        assert_eq!(
            om.len(),
            want,
            "{ctx} {}: oracle {what} magnitude length {} != {want}",
            exp.name,
            om.len()
        );
        assert_eq!(
            oa.len(),
            want,
            "{ctx} {}: oracle {what} angle length {} != {want}",
            exp.name,
            oa.len()
        );
        assert_eq!(
            rust, want,
            "{ctx} {}: rust {what} length {rust} != {want}",
            exp.name
        );
    };
    shape(
        "CurrentsMagAng",
        &exp.cma_mag,
        &exp.cma_ang,
        snap.currents_mag_ang.len(),
        yorder,
    );
    shape(
        "VoltagesMagAng",
        &exp.vma_mag,
        &exp.vma_ang,
        snap.voltages_mag_ang.len(),
        yorder,
    );
    shape(
        "Residuals",
        &exp.res_mag,
        &exp.res_ang,
        snap.residuals.len(),
        nterms,
    );

    if channels.currents_mag_ang {
        polar_close(
            &snap.currents_mag_ang,
            &exp.cma_mag,
            &exp.cma_ang,
            tol.i_rel,
            tol.i_abs,
            &format!("{ctx} {} CurrentsMagAng", exp.name),
        );
    }
    if channels.voltages_mag_ang {
        polar_close(
            &snap.voltages_mag_ang,
            &exp.vma_mag,
            &exp.vma_ang,
            tol.v_rel,
            tol.v_abs,
            &format!("{ctx} {} VoltagesMagAng", exp.name),
        );
    }
    if channels.residuals {
        residual_close(
            &snap.residuals,
            exp,
            tol.i_rel,
            tol.i_abs,
            &format!("{ctx} {} Residuals", exp.name),
        );
    }
}

/// The three floor derivations of the G1.3a polar channels, pinned directly
/// (no oracle) the way `harness_power_floor.rs` pins the voltage-scaled power
/// floor: the wrap, the angular image and its ceiling, and the conductor-sum
/// residual band. Each carries its rejection leg, so none of them is a one-sided
/// "accepts everything" green.
#[cfg(test)]
mod derived_polar_floors {
    use super::{
        ElemChannels, ElementCap, ElementSnapshot, POLAR_RAD_TO_DEG, Polar, assert_complex_close_c,
        compare_element_derived, no_polar_payload, polar_angle_band, polar_close, residual_band,
        residual_close, tol_for, wrapped_deg,
    };
    use dss_core::support::complexutil::cdang;
    use num_complex::Complex64;
    use std::f64::consts::SQRT_2;

    /// The feeder tier's current floors — the ones the `cma`/`res` channels use.
    const REL: f64 = 1e-7;
    const ABS: f64 = 1e-5;

    /// A phasor astride the negative real axis reads at both ends of `CDANG`'s
    /// `(−180, 180]` range: on the line the two readings are a full turn apart,
    /// on the circle they are 1.9e-8 ° apart. The comparator must use the second
    /// number — and must still see a genuine sign flip as a half turn.
    #[test]
    fn the_angle_comparison_is_wrap_aware() {
        let plus = cdang(Complex64::new(-1.0, 1e-18));
        let minus = cdang(Complex64::new(-1.0, -1e-18));
        assert_eq!(plus, 179.99999999032846);
        assert_eq!(minus, -179.99999999032846);
        assert_eq!((plus - minus).abs(), 359.9999999806569);
        assert_eq!(wrapped_deg(plus - minus), 1.9343133317306638e-08);
        // So the real comparator accepts the pair on a healthy 1 A magnitude at
        // the *micro* tier (1e-9 rel / 1e-6 abs → a 5.7e-5 ° angle band).
        polar_close(
            &[Polar {
                mag: 1.0,
                ang: plus,
            }],
            &[1.0],
            &[minus],
            1e-9,
            1e-6,
            "branch cut",
        );
        // Wrapping is not a blanket relaxation: a sign flip still measures a
        // full half turn, from either side of the cut.
        assert_eq!(wrapped_deg(180.0), 180.0);
        assert_eq!(wrapped_deg(-180.0), 180.0);
        assert!(wrapped_deg(f64::NAN).is_nan());
    }

    /// The rejection leg of the wrap: 180 ° is above the angle band's ceiling
    /// (`POLAR_RAD_TO_DEG`), so a flipped phasor fails however it is wrapped.
    #[test]
    #[should_panic(expected = "angle")]
    fn a_sign_flipped_angle_still_fails_the_band() {
        polar_close(
            &[Polar { mag: 1.0, ang: 0.0 }],
            &[1.0],
            &[180.0],
            1e-9,
            1e-6,
            "flip",
        );
    }

    /// The angular image of a magnitude band is bounded by one radian in
    /// degrees, and the mask fires exactly at the boundary `|mag| ≤ band` — i.e.
    /// the angle is skipped precisely where it would otherwise be compared at
    /// ≥ 57.3 °, which is no comparison at all.
    #[test]
    fn the_angle_band_never_exceeds_one_radian_in_degrees() {
        let mut engaged = 0;
        for e in -12..=6 {
            let mag = 10f64.powi(e);
            let allowed = ABS + REL * mag;
            match polar_angle_band(allowed, mag) {
                None => assert!(
                    mag <= allowed,
                    "the mask fired at |mag| = {mag:e} > allowed {allowed:e}"
                ),
                Some(b) => {
                    assert!(
                        mag > allowed,
                        "the mask failed to fire at |mag| = {mag:e} <= allowed {allowed:e}"
                    );
                    assert!(
                        b > 0.0 && b <= POLAR_RAD_TO_DEG,
                        "band {b:e} deg outside (0, {POLAR_RAD_TO_DEG}] at |mag| = {mag:e}"
                    );
                    engaged += 1;
                }
            }
        }
        assert!(
            engaged >= 6,
            "the sweep never left the mask ({engaged} banded samples)"
        );
        // The ceiling is approached only from below, one ulp outside the mask…
        let b = polar_angle_band(1.0, 1.0 + f64::EPSILON).expect("just outside the mask");
        assert_eq!(b, 57.29577951308231);
        assert!(b < POLAR_RAD_TO_DEG);
        // …and exactly at the boundary the angle is skipped, never compared at a
        // 57.3 ° band.
        assert_eq!(polar_angle_band(1.0, 1.0), None);
        assert_eq!(polar_angle_band(1.0, -1.0), None);
        // At a healthy magnitude the image is far tighter than the base band:
        // 683 A at the feeder tier gets 6.57e-6 °.
        assert_eq!(
            polar_angle_band(ABS + REL * 683.0, 683.0),
            Some(6.5684619851747365e-06)
        );
    }

    /// One synthetic 3-conductor terminal: the residual band is the **sum** of
    /// the three conductor bands, not one conductor's band, because the residual
    /// is their sum.
    #[test]
    fn the_residual_floor_is_the_sum_of_the_conductor_bands() {
        let exp = synthetic_terminal();
        // 3·1e-5 + 1e-7·600 = 9e-5 A (in f64: 8.999999999999999e-05), where the
        // single-sample band would be 1e-5 + 1e-7·600 = 7e-5 A.
        let band = residual_band(&exp, 0, 3, REL, ABS);
        assert_eq!(band, 8.999999999999999e-05);
        assert_eq!(ABS + REL * 600.0, 7e-05);
        assert!(band > ABS + REL * 600.0);
        // Accepted just inside the band (8.900000000267028e-05 A of error)…
        residual_close(
            &[Polar {
                mag: 600.0 + 8.9e-5,
                ang: 0.0,
            }],
            &exp,
            REL,
            ABS,
            "inside",
        );
        // …and the angle that comes with it is banded by the image of THIS band.
        assert_eq!(polar_angle_band(band, 600.0), Some(8.594366926962348e-6));
    }

    /// The rejection leg of the residual band: 9.099999999762076e-05 A of error
    /// on the same terminal is outside 8.999999999999999e-05 A and fails.
    #[test]
    #[should_panic(expected = "magnitude")]
    fn a_residual_above_the_conductor_sum_band_fails() {
        residual_close(
            &[Polar {
                mag: 600.0 + 9.1e-5,
                ang: 0.0,
            }],
            &synthetic_terminal(),
            REL,
            ABS,
            "outside",
        );
    }

    /// One terminal, three conductors carrying 100/200/300 A real, so
    /// `Σ_c |I_c| = 600 A` exactly and the residual is 600 A at 0 °.
    fn synthetic_terminal() -> ElementCap {
        ElementCap {
            name: "line.synthetic".to_string(),
            i_re: vec![100.0, 200.0, 300.0],
            i_im: vec![0.0, 0.0, 0.0],
            res_mag: vec![600.0],
            res_ang: vec![0.0],
            ..ElementCap::default()
        }
    }

    /// The set the polar channels are images **of** is a disc, not a rectangle:
    /// [`assert_complex_close_c`] (`harness/mod.rs:799-801`) bands the *modulus*
    /// of the complex difference, `|Δz| ≤ abs + rel·|z|`. A per-component band
    /// would admit a rectangle whose modulus reaches `√2·abs + rel·|z|` at 45 °
    /// — this test rejects exactly that error, which is why no `√2` appears in
    /// any of the derivations of tests/TOLERANCE_NOTES.md §G1.3a.
    #[test]
    fn the_inherited_current_band_is_a_disc_not_a_rectangle() {
        // 600 A at 45 °, feeder tier: rho = 1e-5 + 1e-7·600 = 7e-5 A.
        let m = 600.0;
        let z = Complex64::new(m / SQRT_2, m / SQRT_2);
        let rho = ABS + REL * m;
        assert_eq!(rho, 7e-5);
        // A diagonal error of the disc radius is admitted…
        let d = rho * (1.0 - 1e-9) / SQRT_2;
        assert_complex_close_c(&[z + Complex64::new(d, d)], &[z], REL, ABS, "disc");
        // …and the Minkowski bound of the per-component rectangle at 45 ° —
        // `√2·abs + rel·|z|` = 7.414213562373095e-5 A, 1.059× the disc radius —
        // is NOT (rejection leg below). That bound is attained: it is the modulus
        // of the rectangle's own corner, to the last ulp.
        let rect = SQRT_2 * ABS + REL * m;
        assert_eq!(rect, 7.414213562373095e-5);
        assert_eq!(rect / rho, 1.0591733660532994);
        let corner = Complex64::new(ABS + REL * m / SQRT_2, ABS + REL * m / SQRT_2).norm();
        assert_eq!(corner, 7.414213562373094e-5);
        assert!((rect - corner).abs() <= f64::EPSILON * rect);
    }

    /// The rejection leg: an error of the rectangle's diagonal reach fails the
    /// disc band, so a later "the gate bands re and im separately" reading
    /// cannot be reintroduced silently.
    #[test]
    #[should_panic(expected = "entry 0 differs")]
    fn the_rectangles_diagonal_reach_fails_the_disc_band() {
        let m = 600.0;
        let z = Complex64::new(m / SQRT_2, m / SQRT_2);
        let d = (SQRT_2 * ABS + REL * m) / SQRT_2;
        assert_complex_close_c(&[z + Complex64::new(d, d)], &[z], REL, ABS, "disc");
    }

    /// The angle band is the **linearization** of the exact angular image of the
    /// disc: `arg` maps `D(z, ρ)` onto `±asin(ρ/|z|)`, and `asin(x) ≥ x`, so
    /// `rad2deg·ρ/|z|` is never *looser* than the exact image — the channel can
    /// only be stricter than the band it inherits, never more permissive.
    #[test]
    fn the_angle_band_is_the_conservative_linearization_of_its_exact_image() {
        // Everywhere the gap is above f64 noise, the band is strictly tighter.
        for e in -6..=0 {
            let x = 10f64.powi(e);
            let band = POLAR_RAD_TO_DEG * x;
            let exact = POLAR_RAD_TO_DEG * x.asin();
            assert!(
                band < exact,
                "band {band:e} !< exact image {exact:e} at x = {x:e}"
            );
        }
        // At the widest band the live corpus ever emitted (37.0083678180735 ° on
        // `midi_fuse` residuals, x = 0.6459178692144925) the exact image is
        // 40.23452908031853 ° — the emitted band is 8.7 % tighter, with no
        // consequence: the whole `midi_fuse` residual-angle channel measures
        // ≤ 3.9428730418000316e-7 of its band.
        let x = 37.0083678180735 / POLAR_RAD_TO_DEG;
        assert_eq!(x, 0.6459178692144925);
        assert_eq!(POLAR_RAD_TO_DEG * x.asin(), 40.23452908031853);
        // At the magnitudes the corpus actually judges (x ≈ 1.6e-9) the two are
        // the same f64 to within one ulp of the multiplication order: the
        // `combo_mesh_asym` micro-tier sample of §G1.3a.
        let m = 1.4616566273058197e3;
        let rho = 1e-6 + 1e-9 * m;
        let band = polar_angle_band(rho, m).expect("far outside the mask");
        assert_eq!(band, 9.649498570331597e-8);
        let exact = POLAR_RAD_TO_DEG * (rho / m).asin();
        assert_eq!(exact, 9.649498570331596e-8);
        assert!((band - exact).abs() <= f64::EPSILON * exact);
    }

    /// An **enabled** `UPFCControl`: `TUPFCControlObj.Create` (r4133
    /// `Version8/Source/Controls/UPFCControl.pas:230-246`, capi 0.14.5
    /// `src/Controls/UPFCControl.pas:151-164`) never assigns
    /// `Nterms`/`Nphases`/`Setbus`, so the element carries 0 terminals, 0
    /// conductor slots and six empty channels on every engine.
    fn zero_terminal_pair() -> (Vec<ElementSnapshot>, ElementCap) {
        let snap = ElementSnapshot {
            name: "UPFCControl.myupfcctrl".to_string(),
            enabled: true,
            bus_names: Vec::new(),
            powers: Vec::new(),
            currents: Vec::new(),
            loss_w: (0.0, 0.0),
            currents_mag_ang: Vec::new(),
            voltages_mag_ang: Vec::new(),
            residuals: Vec::new(),
            n_terms: 0,
            n_conds: 0,
            n_phases: 0,
            node_order: Vec::new(),
            energy_meter: None,
            phase_losses: Vec::new(),
            num_controls: 0,
            ocp_dev_index: 0,
            ocp_dev_type: 0,
            has_volt_control: false,
            has_switch_control: false,
        };
        let cap = ElementCap {
            name: "UPFCControl.myupfcctrl".to_string(),
            enabled: Some(true),
            ..ElementCap::default()
        };
        (vec![snap], cap)
    }

    /// The empty acceptance itself: with both sides empty the comparator returns
    /// instead of tripping the `yorder % nterms` shape assert (which read
    /// `0 conductor slots do not divide into 0 terminals` on all nine
    /// `UPFCControl` corpus cases).
    #[test]
    fn a_zero_terminal_element_is_accepted_when_both_sides_are_empty() {
        let (snaps, cap) = zero_terminal_pair();
        compare_element_derived(&snaps, &cap, &tol_for("feeder"), "upfc", ElemChannels::ALL);
    }

    /// Rejection leg 1 — the **oracle** grows a payload the port does not have.
    #[test]
    #[should_panic(expected = "0-terminal element must carry no polar payload")]
    fn a_zero_terminal_element_with_an_oracle_payload_fails() {
        let (snaps, mut cap) = zero_terminal_pair();
        cap.cma_mag = vec![1.0];
        cap.cma_ang = vec![0.0];
        compare_element_derived(&snaps, &cap, &tol_for("feeder"), "upfc", ElemChannels::ALL);
    }

    /// The one shape the oracle side may add on a 0-terminal element: the capi
    /// `DefaultResult` COM sentinel. `Alt_CE_Get_VoltagesMagAng` returns
    /// one-element `[0.0]` whenever `elem.NodeRef = NIL`
    /// (`CAPI/CAPI_Alt.pas:1080-1084` → `CAPI/CAPI_Utils.pas:212-221`), which
    /// de-interleaves into `mag = [0.0]`, `ang = []`; r4133 and the engine both
    /// report an empty array. Measured live on all nine `UPFCControl` corpus
    /// cases, CapiV0145 channel only, as `vma 1/0`.
    #[test]
    fn the_capi_default_result_sentinel_reads_as_no_payload() {
        assert!(no_polar_payload(&[], &[]));
        assert!(no_polar_payload(&[0.0], &[]));
        // …and nothing else does: a real one-sample reading, a non-zero
        // sentinel-shaped magnitude, or an angle without a magnitude all fail.
        assert!(!no_polar_payload(&[0.0], &[0.0]));
        assert!(!no_polar_payload(&[1.0], &[]));
        assert!(!no_polar_payload(&[0.0, 0.0], &[]));
        assert!(!no_polar_payload(&[], &[0.0]));
        // End to end: the sentinel on the channel that actually carries it.
        let (snaps, mut cap) = zero_terminal_pair();
        cap.vma_mag = vec![0.0];
        compare_element_derived(&snaps, &cap, &tol_for("feeder"), "upfc", ElemChannels::ALL);
    }

    /// The sentinel acceptance is a *shape* rule, not a value one: a one-element
    /// magnitude that is not the sentinel's `0.0` still fails.
    #[test]
    #[should_panic(expected = "0-terminal element must carry no polar payload")]
    fn a_sentinel_shaped_but_non_zero_oracle_payload_fails() {
        let (snaps, mut cap) = zero_terminal_pair();
        cap.vma_mag = vec![1e-30];
        compare_element_derived(&snaps, &cap, &tol_for("feeder"), "upfc", ElemChannels::ALL);
    }

    /// Rejection leg 2 — the **port** grows a payload the oracle does not have,
    /// which is the direction a one-sided `is_empty()` check on `exp` would miss.
    #[test]
    #[should_panic(expected = "0-terminal element must carry no polar payload")]
    fn a_zero_terminal_element_with_a_port_payload_fails() {
        let (mut snaps, cap) = zero_terminal_pair();
        snaps[0].residuals = vec![Polar { mag: 1.0, ang: 0.0 }];
        compare_element_derived(&snaps, &cap, &tol_for("feeder"), "upfc", ElemChannels::ALL);
    }

    /// The acceptance is scoped to `Yorder == 0 && Nterms == 0`: conductor slots
    /// without terminals still hit the shape assert, so the branch cannot be
    /// used to wave a real shape miss through.
    #[test]
    #[should_panic(expected = "conductor slots do not divide into 0 terminals")]
    fn conductor_slots_without_terminals_still_fail_the_shape_assert() {
        let (snaps, mut cap) = zero_terminal_pair();
        cap.i_re = vec![1.0, 2.0];
        cap.i_im = vec![0.0, 0.0];
        compare_element_derived(&snaps, &cap, &tol_for("feeder"), "upfc", ElemChannels::ALL);
    }

    /// …and the mirror — terminals without conductor slots — falls through to the
    /// per-channel length asserts, which catch it on `Residuals` (one entry per
    /// terminal).
    #[test]
    #[should_panic(expected = "oracle Residuals magnitude length 0 != 1")]
    fn terminals_without_conductor_slots_still_fail_the_length_asserts() {
        let (mut snaps, cap) = zero_terminal_pair();
        snaps[0].bus_names = vec!["b1".to_string()];
        compare_element_derived(&snaps, &cap, &tol_for("feeder"), "upfc", ElemChannels::ALL);
    }
}

/// The oracle's **raw** `CktElement.EnergyMeter` string, read as "the bare name
/// of the meter that meters this element, or `None` when none does".
///
/// The two transports spell "no meter" differently, which is a capture-boundary
/// **sentinel shape** rather than a divergence — coordinator decision D4, the
/// [`props_norm`] and [`no_polar_payload`] precedent — so it is normalized here
/// for **0 ledger rows** and pinned by
/// `element_extras_pins::the_no_meter_sentinel_is_normalized_on_both_channels`:
///
/// * **capi** returns `NIL` (`CAPI/CAPI_CktElement.pas:672-687`: `Result := NIL`
///   unless `Flg.HasEnergyMeter in elem.Flags`, `pd.MeterObj.Name` at `:685`),
///   which the pinned dss-python renders as the empty string;
/// * **r4133** returns `'0'` — the `CktElementS` family default assigned before
///   the `case` (`DDLL/DCktElement.pas:421`) and left untouched by arm `4`
///   (`:442-449`), whose `MeterObj.Name` read is itself guarded by
///   `HasEnergyMeter` at `:444`.
///
/// **Each channel's own sentinel, not both** (G1.3d(i) audit settlement,
/// 2026-09-05). Folding `'0'` on the capi channel too would have been free
/// blindness: capi never spells "no meter" that way, so there `0` is a name
/// like any other and a port that lost a meter named `0` reds
/// (`a_meter_named_zero_reds_instead_of_passing`). Symmetrically, an empty
/// string from r4133 is not a sentinel — it is an unexpected transport answer,
/// and it reds rather than being absorbed.
///
/// On the **r4133** channel a meter literally *named* `0` remains undecidable,
/// and the ambiguity cuts **both** ways: a port that lost such a meter would
/// compare `None == None` and pass. The earlier claim here — "can only produce a
/// false failure, never a false pass" — was true in one direction only. What
/// makes the collision unreachable is the corpus census
/// (`extras_population::no_corpus_energymeter_is_named_zero` in
/// `crates/dss-core/tests/corpus_manifest.rs`: 1 310 decks, 92 distinct meter
/// names, none of them `0`), and the residual itself is asserted rather than
/// assumed by
/// `element_extras_pins::the_r4133_zero_sentinel_is_undecidable_and_the_census_is_the_guard`.
fn oracle_meter_name(raw: &str, channel: PropsChannel) -> Option<&str> {
    match (channel, raw) {
        (PropsChannel::CapiV0145, "") | (PropsChannel::R4133, "0") => None,
        (_, name) => Some(name),
    }
}

/// Compare one element's **discrete index/name extras** against a capture:
/// `NumTerminals`, `NumConductors`, `NumPhases`, `EnergyMeter` and `NodeOrder`
/// (`GOLDEN_REBASE_PLAN.md` WP-G1 sub-step G1.3d(i)), plus the five
/// control-derived scalars `NumControls`, `OCPDevIndex`, `OCPDevType`,
/// `HasVoltControl` and `HasSwitchControl` (sub-step G1.3d(ii)).
///
/// The sixth G1.3d(ii) field, `PhaseLosses`, is **numeric** and is therefore
/// compared by [`compare_element_phase_losses`] instead — kept out of this
/// function precisely so its "no tolerance anywhere" contract below stays true.
///
/// Runs when the case's `compare_element_extras` manifest flag is on,
/// *alongside* — never instead of — [`compare_element_channels`] and
/// [`compare_element_derived`].
///
/// **Everything here is discrete, so everything is compared exactly.** There is
/// no `Tolerances` argument, no [`ElemChannels`] selector and no ledger
/// sub-channel: a mismatch on an index or a name is a port bug or an upstream
/// defect, never a floor, and the ledger's `divergence` envelope
/// (`max_abs`/`max_rel`) has no meaning for it. Consequence, stated so it is not
/// discovered by surprise: the first live mismatch **stops** the sub-step and the
/// mechanism is designed then, rather than shipping exclusion machinery that
/// `every_exclusion_field_is_honoured_by_the_runtime` could never exercise. A
/// committed `element` scope cannot reach these fields either — the ledger's
/// `clone_element_cap` is a full `ec.clone()`
/// (`corpus_gate/ledger.rs:1695-1697`) and `rewrite_element_selected` (`:1702`)
/// writes only its six named value channels (`currents`, `powers`, `losses`,
/// `currents_mag_ang`, `voltages_mag_ang`, `residuals`).
///
/// **Structure, asserted under every policy** (the rule [`compare_element_channels`]
/// already follows: a value exclusion may never excuse a shape or an existence
/// miss):
///
/// * the element exists in the Rust snapshot, and the capture carries all four
///   scalars plus `Enabled` — their absence means the flag is on and the element
///   was captured without them, the element-level twin of
///   [`capture_guard::require_capture`];
/// * `Enabled` matches exactly. It is re-asserted here rather than borrowed from
///   [`compare_element_derived`], because the two flags are independent and the
///   `NodeOrder` capture predicate rests on this bit;
/// * the three counts match exactly, and two internal ties bind them to shapes
///   that were *already* gated: the port's own `bus_names` list (which
///   [`compare_element_derived`] uses as its terminal count) and the oracle's
///   `Currents` length (`Yorder = NTerms · NConds`);
/// * the five control-derived scalars match exactly, with **no `Enabled`
///   filter** anywhere — upstream's list membership and its OCP scan have none
///   (r4133 `Controls/ControlElem.pas:113-131`, `Common/Utilities.pas:3165-3184`),
///   so a *disabled* OCP control still occupies its slot and still wins
///   (measured on both channels; pinned in-engine by
///   `exec::tests::element_extras::a_disabled_ocp_control_still_wins_the_ocp_scan`).
///   Two internal ties bind them the way the `Yorder` tie binds the counts, and
///   both are asserted on the **oracle** side, where they are properties of the
///   Pascal scan rather than of the port: `OCPDevIndex == 0` exactly when
///   `OCPDevType == 0` (one `repeat … until (i > listSize) or (Result > 0)`
///   walk produces both — r4133 `DDLL/DCktElement.pas:242-258`), and
///   `OCPDevIndex <= NumControls` (the index is a position in the list whose
///   size `NumControls` is);
/// * `NodeOrder` is compared slot by slot when the element is enabled with at
///   least one terminal; on a **0-terminal** element both sides must be empty
///   (two-sided, so a payload on either side still fails); on a **disabled**
///   element only the *oracle* side must be empty — the capture skips the read
///   there, while the port legitimately keeps the mapping it was last given (an
///   element disabled after a solve; pinned in-engine by
///   `exec::tests::element_extras::a_disabled_element_keeps_the_node_order_it_was_given`).
///
/// Capture predicate, from the sources rather than from caution: mode `17`
/// dereferences `NodeRef^[j]` with no nil guard (r4133
/// `DDLL/DCktElement.pas:1048`) and capi raises 15013 from its nil-`NodeRef`
/// guard (`CAPI/CAPI_CktElement.pas:900-906`), so a never-enabled element is not read;
/// and on a 0-terminal element (`UPFCControl` never assigns `Nterms` — r4133
/// `Controls/UPFCControl.pas:230-246`) r4133 would answer a 0-length array where
/// capi raises, so not issuing the read removes that shape asymmetry instead of
/// normalizing it.
///
/// `channel` is read for exactly one thing: [`oracle_meter_name`] folds the
/// "no meter" sentinel of the channel the capture came from, and only that one
/// (the capi `''` and the r4133 `'0'` are different spellings of the same state,
/// coordinator decision D4). Nothing else here is channel-dependent — every
/// field is compared identically on both.
///
/// No tolerance is introduced or consulted anywhere in this function
/// (tests/TOLERANCE_NOTES.md §G1.3d(i)).
pub fn compare_element_extras(
    snaps: &[ElementSnapshot],
    exp: &ElementCap,
    channel: PropsChannel,
    ctx: &str,
) {
    let snap = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&exp.name))
        .unwrap_or_else(|| panic!("{ctx}: no element {}", exp.name));
    fn missing(ctx: &str, name: &str, field: &str) -> String {
        format!(
            "{ctx} {name}: the extras capture carries no `{field}` field — the \
             case's `compare_element_extras` flag is on but this element was \
             captured without it"
        )
    }
    let n_terms = exp
        .n_terms
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "n_terms")));
    let n_conds = exp
        .n_conds
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "n_conds")));
    let n_phases = exp
        .n_phases
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "n_phases")));
    let raw_meter = exp
        .energy_meter
        .as_deref()
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "energy_meter")));
    let enabled = exp
        .enabled
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "enabled")));
    // The five G1.3d(ii) control-derived scalars: emitted for EVERY element
    // under the same flag (neither transport makes one conditional), so an
    // absent one means the element was captured without the flag.
    let num_controls = exp
        .num_controls
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "num_controls")));
    let ocp_dev_index = exp
        .ocp_dev_index
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "ocp_dev_index")));
    let ocp_dev_type = exp
        .ocp_dev_type
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "ocp_dev_type")));
    let has_volt_control = exp
        .has_volt_control
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "has_volt_control")));
    let has_switch_control = exp
        .has_switch_control
        .unwrap_or_else(|| panic!("{}", missing(ctx, &exp.name, "has_switch_control")));

    assert_eq!(
        snap.enabled, enabled,
        "{ctx} {}: Enabled differs (rust {} vs oracle {enabled})",
        exp.name, snap.enabled
    );

    let count = |what: &str, rust: usize, oracle: i32| {
        let want = usize::try_from(oracle)
            .unwrap_or_else(|_| panic!("{ctx} {}: oracle {what} is negative ({oracle})", exp.name));
        assert_eq!(
            rust, want,
            "{ctx} {}: {what} differs (rust {rust} vs oracle {oracle})",
            exp.name
        );
    };
    count("NumTerminals", snap.n_terms, n_terms);
    count("NumConductors", snap.n_conds, n_conds);
    count("NumPhases", snap.n_phases, n_phases);

    // The two ties. Neither is an independent measurement — `bus_names` is built
    // over `1..=nterms` (`exec/view.rs`) and `Yorder` is `NTerms · NConds` on
    // every engine — but they are what makes the newly gated counts do work for
    // channels that were already gated: from here on the oracle's `NumTerminals`
    // stands behind `compare_element_derived`'s terminal count, and its
    // `NumConductors` behind the length of every per-conductor array.
    assert_eq!(
        snap.bus_names.len(),
        snap.n_terms,
        "{ctx} {}: the port's bus-name list ({}) disagrees with its own \
         NumTerminals ({})",
        exp.name,
        snap.bus_names.len(),
        snap.n_terms
    );
    if enabled && !exp.i_re.is_empty() {
        assert_eq!(
            snap.n_terms * snap.n_conds,
            exp.i_re.len(),
            "{ctx} {}: NTerms·NConds ({} · {}) != the oracle's Currents length ({})",
            exp.name,
            snap.n_terms,
            snap.n_conds,
            exp.i_re.len()
        );
    }

    // Exact, with no case folding: both engines lowercase the name in the
    // EnergyMeter constructor (r4133 `Meters/EnergyMeter.pas:921`
    // `Name := LowerCase(...)`, capi `src/Meters/EnergyMeter.pas:952`
    // `AnsiLowerCase`) and so does the port (`elements/ckt.rs:258`).
    assert_eq!(
        snap.energy_meter.as_deref(),
        oracle_meter_name(raw_meter, channel),
        "{ctx} {}: EnergyMeter differs (rust {:?} vs oracle {raw_meter:?})",
        exp.name,
        snap.energy_meter
    );

    // The five control-derived scalars. Discrete, exact, and with no `Enabled`
    // filter: the port recomputes all five on every read from the derived
    // `ControlElementList` (`dss_core::circuit::controls::derive_control_lists`),
    // the way both oracles do, rather than from the registration-time
    // `CktElementData::ocp_device_type` latch the reliability sweep uses.
    count("NumControls", snap.num_controls, num_controls);
    count("OCPDevIndex", snap.ocp_dev_index, ocp_dev_index);
    assert_eq!(
        snap.ocp_dev_type, ocp_dev_type,
        "{ctx} {}: OCPDevType differs (rust {} vs oracle {ocp_dev_type})",
        exp.name, snap.ocp_dev_type
    );
    assert_eq!(
        snap.has_volt_control, has_volt_control,
        "{ctx} {}: HasVoltControl differs (rust {} vs oracle {has_volt_control})",
        exp.name, snap.has_volt_control
    );
    assert_eq!(
        snap.has_switch_control, has_switch_control,
        "{ctx} {}: HasSwitchControl differs (rust {} vs oracle {has_switch_control})",
        exp.name, snap.has_switch_control
    );

    // The two OCP ties, asserted on the ORACLE side: both are properties of the
    // single Pascal walk that produces the pair (`repeat … until (i > listSize)
    // or (Result > 0)`, r4133 `DDLL/DCktElement.pas:242-258`; `GetOCPDeviceType`
    // scans the same list, `Common/Utilities.pas:3165-3184`), so a capture in
    // which they disagree is a transport bug rather than an engine divergence
    // — and the port is held to the same shape by the `assert_eq!`s above.
    assert_eq!(
        ocp_dev_index == 0,
        ocp_dev_type == 0,
        "{ctx} {}: the oracle's OCPDevIndex ({ocp_dev_index}) and OCPDevType \
         ({ocp_dev_type}) must be zero together — one list scan produces both",
        exp.name
    );
    assert!(
        ocp_dev_index <= num_controls,
        "{ctx} {}: the oracle's OCPDevIndex ({ocp_dev_index}) exceeds its own \
         NumControls ({num_controls}) — the index is a 1-based position in \
         that list",
        exp.name
    );

    if snap.n_terms == 0 {
        assert!(
            exp.node_order.is_empty() && snap.node_order.is_empty(),
            "{ctx} {}: a 0-terminal element must carry no NodeOrder on either \
             side (oracle {}, rust {})",
            exp.name,
            exp.node_order.len(),
            snap.node_order.len()
        );
        return;
    }
    if !enabled {
        assert!(
            exp.node_order.is_empty(),
            "{ctx} {}: a disabled element must carry no oracle NodeOrder ({} \
             slots) — the capture read a channel it must skip",
            exp.name,
            exp.node_order.len()
        );
        return;
    }
    let want = snap.n_terms * snap.n_conds;
    assert_eq!(
        exp.node_order.len(),
        want,
        "{ctx} {}: oracle NodeOrder length {} != NTerms·NConds ({want})",
        exp.name,
        exp.node_order.len()
    );
    assert_eq!(
        snap.node_order.len(),
        want,
        "{ctx} {}: rust NodeOrder length {} != NTerms·NConds ({want})",
        exp.name,
        snap.node_order.len()
    );
    assert_eq!(
        snap.node_order, exp.node_order,
        "{ctx} {}: NodeOrder differs (rust {:?} vs oracle {:?})",
        exp.name, snap.node_order, exp.node_order
    );
}

// ---------------------------------------------------------------------------
// The multi-control census (G1.3d(ii) audit settlement, 2026-09-05)
// ---------------------------------------------------------------------------

/// Oracle `NumControls` values the **gating** extras compare has seen…
static CONTROL_CENSUS_SEEN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
/// …how many of them carried at least one control…
static CONTROL_CENSUS_CONTROLLED: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// …how many carried **two or more**…
static CONTROL_CENSUS_MULTI: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);
/// …and how many of THOSE also carry an OCP member (`OCPDevType != 0`), the
/// shape in which the list order becomes observable.
static CONTROL_CENSUS_MULTI_OCP: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

/// The measured `(multi, multi_ocp)` population, 2026-09-05 — see
/// [`assert_no_multi_control_element`] for the two deck families behind it and
/// why every one of those lists is Relay-only. Fail-on-stale in both directions.
const CONTROL_CENSUS_MULTI_MEASURED: (usize, usize) = (18, 18);

/// Record one gated element's oracle `NumControls` / `OCPDevType`.
///
/// One call per compared element **per channel per step** — the counters are row
/// counts over the gating population, not distinct-element counts.
///
/// The single caller is the corpus gate's extras loop
/// (`corpus_gate/runner.rs`, the `c.compare_element_extras` block), so the
/// census counts the gating population and nothing else.
pub fn record_control_census(num_controls: Option<i32>, ocp_dev_type: Option<i32>) {
    use std::sync::atomic::Ordering::Relaxed;
    let Some(n) = num_controls else { return };
    CONTROL_CENSUS_SEEN.fetch_add(1, Relaxed);
    if n >= 1 {
        CONTROL_CENSUS_CONTROLLED.fetch_add(1, Relaxed);
    }
    if n >= 2 {
        CONTROL_CENSUS_MULTI.fetch_add(1, Relaxed);
        if ocp_dev_type.is_some_and(|t| t != 0) {
            CONTROL_CENSUS_MULTI_OCP.fetch_add(1, Relaxed);
        }
    }
}

/// What [`record_control_census`] has counted, as
/// `(elements, with ≥ 1 control, with ≥ 2, with ≥ 2 incl. an OCP member)`.
pub fn control_census_counters() -> (usize, usize, usize, usize) {
    use std::sync::atomic::Ordering::Relaxed;
    (
        CONTROL_CENSUS_SEEN.load(Relaxed),
        CONTROL_CENSUS_CONTROLLED.load(Relaxed),
        CONTROL_CENSUS_MULTI.load(Relaxed),
        CONTROL_CENSUS_MULTI_OCP.load(Relaxed),
    )
}

/// **The population guard behind D-ii-1's zero ledger rows.**
///
/// `GOLDEN_REBASE_PLAN.md` G1.3d(ii) settles a real r4133↔capi divergence — r4133
/// re-runs `TControlElem.Set_ControlledElement` inside every control's
/// `RecalcElementData` and moves the control to the END of its element's
/// `ControlElementList` (`Controls/ControlElem.pas:113-131`,
/// `Controls/Relay.pas:955` from `:626`), while capi 0.14.5 only re-attaches on a
/// `SwitchedObj` write (`Controls/Relay.pas:439-441`) — with **no** ledger row,
/// because `OCPDevIndex`/`OCPDevType` can only move when a reordering changes
/// *which* member is the first OCP one. The port follows r4133
/// (`DIVERGENCES.md` L9), so that population fact is load-bearing: the day a deck
/// gives one element an OCP control **and** a control of another class, the capi
/// channel can start disagreeing with the port on a **discrete** field.
///
/// This is that fact, re-derived on every full run instead of stated in prose
/// (the D15/D16 precedent, and the `extras_population::no_corpus_energymeter_is_
/// named_zero` shape one sub-step earlier) — and writing it corrected the
/// sub-step's own claim, whose live census had covered only
/// `tests/corpus/controls/**`. Multi-control elements **do** exist in the gated
/// population: **18** (case, channel, step, element) rows, all of them from two
/// families — `Line.thev` under `Relay.21src` + `Relay.21rev` in the eight
/// Distance/TD21 relay decks (`Test/{,Reverse}{Distance,TD21}RelayTest.DSS` and
/// their `Version8/Distrib/Examples/DistanceRelays/` twins) and `Line.motorleads`
/// under `Relay.{mfrov/uv,mfr46,mfr47}` in `controls:fuse/indmach_r4133/
/// indmach_{snap,dyn}.dss`. **Every member of both lists is a Relay**, so every
/// permutation answers the same `OCPDevIndex = 1`, `OCPDevType = 3`, and the other
/// three scalars are order-free by construction (`.len()` and two `any()` walks).
/// The counts are therefore pinned exactly, fail-on-stale in **both** directions:
/// one more (or one fewer) multi-control element must be re-triaged against D-ii-1
/// before this constant moves.
///
/// Silent under `DSS_GATE_ONLY` for the reason
/// [`props_norm::assert_r4133_props_compare_ran`] is: a filtered run is not the
/// population. The mandatory gate never sets the variable.
pub fn assert_no_multi_control_element() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let (seen, controlled, multi, multi_ocp) = control_census_counters();
    eprintln!(
        "corpus_gate control census: {seen} element(s), {controlled} with a control, \
         {multi} with two or more, {multi_ocp} of those with an OCP device"
    );
    check_control_census(seen, controlled, multi, multi_ocp);
}

/// The rule itself, over **injected** counters — split out for the reason
/// [`props_norm::check_r4133_props_compare_ran`]'s twin is: the shipped statics
/// cannot be rewound once the gate has moved them, so both directions are pinned
/// offline (`element_extras_pins::the_control_census_*`).
///
/// The two `(multi, multi_ocp)` counts are exact (see
/// [`assert_no_multi_control_element`]); `seen`/`controlled` are floors, loose on
/// purpose (another lane may add or retire a deck) but non-vacuous: a census that
/// stopped counting — the extras request re-masked, the recording call dropped —
/// fails here instead of greening. Measured 2026-09-05 over the full gated
/// population: 298 565 / 3 184 / 18 / 18.
fn check_control_census(seen: usize, controlled: usize, multi: usize, multi_ocp: usize) {
    assert_eq!(
        (multi, multi_ocp),
        CONTROL_CENSUS_MULTI_MEASURED,
        "the multi-control population moved (now {multi} element row(s) with two \
         or more controls, {multi_ocp} of them carrying an OCP device, out of \
         {seen} gated rows). GOLDEN_REBASE_PLAN.md G1.3d(ii)'s D-ii-1 costs ZERO \
         ledger rows only while every such list is homogeneous: r4133's per-edit \
         re-attach (which the port follows) reorders the list against capi \
         0.14.5's, and with an OCP member beside a control of another class that \
         moves OCPDevIndex/OCPDevType — DISCRETE fields, compared exactly on both \
         channels and maskable by no ledger scope. Re-triage the deck that moved \
         this count (are its list's members all of one control class?) before \
         updating the constant."
    );
    assert!(
        seen > 0 && controlled >= 1_000,
        "the control census looks broken: {seen} element row(s), {controlled} with \
         a control (measured 2026-09-05 over the full gated population: \
         298 565 / 3 184). A zero `seen` means the gating extras compare never ran \
         at all."
    );
}

/// The magnitude band of phase `i`'s `PhaseLosses` sample (kW): the per-conductor
/// power band of [`assert_power_close`], summed over exactly the conductors
/// `GetPhaseLosses` sums.
///
/// `PhaseLosses[i] = Σ_{j<nterms} NodeV[NodeRef[k]]·conj(Iterminal[k])` at
/// `k = j·nconds + i` (r4133 `Common/CktElement.pas:1093-1112`), i.e. the very
/// products the capture already reports as `Powers[k]` — so the accepted error
/// is `Σ_j (abs·max(1, |V_k|) + rel·|S_k|)` with `|V_k| = |P_kW[k]| / |I_A[k]|`,
/// the same voltage-scaled floor and the same recovery of `|V|` that
/// [`assert_power_close`] uses per conductor.
///
/// **No new tolerance class and no new constant**: this is the identical
/// construction [`compare_element_channels`] already applies to `Get_Losses`
/// (`losses = Σ_k S_k` over *all* conductors) and [`residual_band`] applies to a
/// terminal's current sum, restricted to one phase's `nterms` conductors. Being
/// a strict subset of the `Get_Losses` sum, it is strictly **tighter** than that
/// band (pinned by
/// `phase_loss_bands::the_phase_bands_sum_to_at_most_the_get_losses_band`).
/// Derivation in tests/TOLERANCE_NOTES.md §G1.3d(ii).
pub fn phase_loss_band(
    exp: &ElementCap,
    phase: usize,
    nterms: usize,
    nconds: usize,
    rel: f64,
    abs: f64,
) -> f64 {
    (0..nterms)
        .map(|j| {
            let k = j * nconds + phase;
            let p_mag = (exp.p_kw[k].powi(2) + exp.p_kvar[k].powi(2)).sqrt();
            let i_mag = (exp.i_re[k].powi(2) + exp.i_im[k].powi(2)).sqrt();
            // |V_kv| = |P_kW| / |I_A| (terminal kV), exactly as
            // `assert_power_close` recovers it; `max(1, …)` never tightens
            // below the established floor.
            let vkv = if i_mag > 1e-12 { p_mag / i_mag } else { 1.0 };
            abs * vkv.max(1.0) + rel * p_mag
        })
        .sum()
}

/// Compare one element's `PhaseLosses` against a capture — the numeric member of
/// the G1.3d(ii) extras group (`GOLDEN_REBASE_PLAN.md` WP-G1 sub-step G1.3d(ii)).
///
/// Runs when the case's `compare_element_extras` manifest flag is on,
/// *alongside* — never instead of — [`compare_element_extras`],
/// [`compare_element_channels`] and [`compare_element_derived`], on the same
/// (possibly ledger-rewritten) capture the other three see.
///
/// **The ×0.001 lives here and nowhere else.** The engine reports W/var
/// (`ElementSnapshot::phase_losses`, like `loss_w`); both oracle surfaces scale
/// by `0.001` at the API boundary — r4133 `DDLL/DCktElement.pas:637-658`
/// (`CktElementV` mode `6`, `cmulreal(cBuffer^[i], 0.001)` at `:651`), capi
/// `CAPI/CAPI_Alt.pas:449-467` (`Result[i] *= 0.001` at `:464-467`) — so the
/// kW/kvar rendering is a capture-boundary encoding, converted at this one site
/// exactly as the interleaved re/im pair is de-interleaved at one site.
///
/// **Structure, asserted under every channel policy** (the rule
/// [`compare_element_channels`] and [`compare_element_derived`] already follow:
/// a value exclusion may never excuse a shape or an existence miss): the element
/// exists in the Rust snapshot; the oracle's kW and kvar halves are the same
/// length — which is what makes the capi `DefaultResult` one-element sentinel
/// (`CAPI/CAPI_Utils.pas:212-221`, reachable through `Alt_CE_Get_PhaseLosses`'
/// `MissingSolution` guard, `CAPI/CAPI_Alt.pas:455-459`) fail loudly rather than
/// de-interleave into a silent `[0.0]`/`[]` pair; both sides have exactly
/// `NumPhases` samples, including the **zero** of a 0-phase element
/// (`UPFCControl` never assigns `Nphases`, r4133
/// `Controls/UPFCControl.pas:230-246`; measured `n = 0` on r4133 and `[]` on
/// capi, so this is a two-sided emptiness and not a normalization); and a
/// **disabled** element still carries `NumPhases` zeros on every engine, because
/// neither oracle skips the read — r4133 zero-fills in the `Else` arm
/// (`Common/CktElement.pas:1117-1119`) and capi in its
/// `(not FEnabled) or (NodeRef = NIL)` guard (`src/Common/CktElement.pas:890-894`),
/// as does the port.
///
/// **Value**, gated on `channels.phase_losses` — false only on the two `newton*`
/// decks, in both lanes, for the
/// `POWERS_REUSE_STALE_NEWTON_ITERMINAL` teardown row, whose marker lives beside
/// [`lane::elem_channels_for`] ([`ElemChannels::CURRENTS_ONLY`]): each phase is banded by
/// [`phase_loss_band`], the per-conductor power band summed over that phase's
/// conductors. No tolerance is calibrated here.
///
/// **The `phase_losses` ledger sub-channel exists because the live drive
/// measured it** (G1.3d(ii) F4, 2026-09-05), not on speculation: the unfiltered
/// 523-case gate reported eight (case, channel) pairs failing here, every one of
/// them on a deck that already carries an `element` scope selecting
/// `powers`/`losses` for the same cause. `corpus_gate/ledger.rs` therefore
/// declares `phase_losses` in `SUBCHANNEL_FIELDS` and honours it in both
/// handlers — `rewrite_element_selected` writes `pl_kw`/`pl_kvar` back from the
/// snapshot (×0.001) and `envelope_element` bands them with
/// [`phase_loss_band`]. **Nothing was widened that was not measured failing**:
/// the four `r4133-*-injection-ulp` divergences select `losses` and were
/// measured NOT to fail here. What is gate-ENFORCED and what is not, precisely:
/// the per-sub-channel staleness check (`Scope::dead_channels`,
/// `corpus_gate/ledger.rs:399-421`) fails the gate on a name that masks nothing
/// only for a `divergence` entry — an `exclusion` fetches no verdict
/// (`LedgerView::excluded`) and so is never policed by it. All ten widened
/// entries are `exclusion`s, so what stands behind each of them is the
/// per-entry measurement recorded in its own
/// `measured.g13d2_phase_losses_first_failure`, and the enforced half was
/// proved live on a deliberate eleventh widening of a `divergence` entry
/// (F4 M5, reverted).
pub fn compare_element_phase_losses(
    snaps: &[ElementSnapshot],
    exp: &ElementCap,
    tol: &Tolerances,
    ctx: &str,
    channels: ElemChannels,
) {
    let snap = snaps
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(&exp.name))
        .unwrap_or_else(|| panic!("{ctx}: no element {}", exp.name));
    let n_phases = exp.n_phases.unwrap_or_else(|| {
        panic!(
            "{ctx} {}: the extras capture carries no `n_phases` field — the \
             case's `compare_element_extras` flag is on but this element was \
             captured without it",
            exp.name
        )
    });
    assert_eq!(
        exp.pl_kw.len(),
        exp.pl_kvar.len(),
        "{ctx} {}: the oracle's PhaseLosses kW ({}) and kvar ({}) halves \
         disagree — an odd-length payload de-interleaved (the capi \
         `DefaultResult` sentinel is one such)",
        exp.name,
        exp.pl_kw.len(),
        exp.pl_kvar.len()
    );
    let want = usize::try_from(n_phases).unwrap_or_else(|_| {
        panic!(
            "{ctx} {}: oracle NumPhases is negative ({n_phases})",
            exp.name
        )
    });
    assert_eq!(
        exp.pl_kw.len(),
        want,
        "{ctx} {}: oracle PhaseLosses length {} != NumPhases ({want})",
        exp.name,
        exp.pl_kw.len()
    );
    assert_eq!(
        snap.phase_losses.len(),
        want,
        "{ctx} {}: rust PhaseLosses length {} != NumPhases ({want})",
        exp.name,
        snap.phase_losses.len()
    );
    if want == 0 || !channels.phase_losses {
        return;
    }

    // The conductor layout the band indexes through. Both counts come from the
    // same capture and are compared exactly by [`compare_element_extras`]; the
    // `Yorder` tie below is what keeps `k = j·nconds + i` inside the captured
    // arrays — asserted, never assumed.
    let nterms = usize::try_from(exp.n_terms.unwrap_or_else(|| {
        panic!(
            "{ctx} {}: the extras capture carries no `n_terms` field",
            exp.name
        )
    }))
    .unwrap_or_else(|_| panic!("{ctx} {}: oracle NumTerminals is negative", exp.name));
    let nconds = usize::try_from(exp.n_conds.unwrap_or_else(|| {
        panic!(
            "{ctx} {}: the extras capture carries no `n_conds` field",
            exp.name
        )
    }))
    .unwrap_or_else(|_| panic!("{ctx} {}: oracle NumConductors is negative", exp.name));
    assert!(
        want <= nconds,
        "{ctx} {}: NumPhases ({want}) exceeds NumConductors ({nconds})",
        exp.name
    );
    assert_eq!(
        nterms * nconds,
        exp.p_kw.len(),
        "{ctx} {}: NTerms·NConds ({nterms} · {nconds}) != the oracle's Powers \
         length ({}) — the PhaseLosses band is built from those conductors",
        exp.name,
        exp.p_kw.len()
    );
    assert_eq!(
        exp.p_kw.len(),
        exp.p_kvar.len(),
        "{ctx} {}: the oracle's Powers kW/kvar halves disagree",
        exp.name
    );
    assert_eq!(
        exp.p_kw.len(),
        exp.i_re.len(),
        "{ctx} {}: the oracle's Powers and Currents lengths disagree",
        exp.name
    );
    assert_eq!(
        exp.i_re.len(),
        exp.i_im.len(),
        "{ctx} {}: the oracle's Currents re/im halves disagree",
        exp.name
    );

    for i in 0..want {
        let allowed = phase_loss_band(exp, i, nterms, nconds, tol.i_rel, tol.i_abs);
        // W/var → kW/kvar: componentwise on `Complex64`, the same two
        // multiplications `snapshot_elements` applies to `Powers`.
        let a = snap.phase_losses[i] * 0.001;
        let (er, ei) = (exp.pl_kw[i], exp.pl_kvar[i]);
        let diff = ((a.re - er).powi(2) + (a.im - ei).powi(2)).sqrt();
        assert!(
            diff <= allowed,
            "{ctx} {}: PhaseLosses phase {i} differs: actual ({}, {}) kW/kvar vs \
             expected ({er}, {ei}); |diff| = {diff:e} > allowed {allowed:e}",
            exp.name,
            a.re,
            a.im
        );
    }
}

/// The G1.3d(ii) `PhaseLosses` pins: the derived band and the comparator's
/// structural rules, pinned directly (no oracle) the way `derived_polar_floors`
/// pins the polar bands. Every acceptance carries its rejection leg.
#[cfg(test)]
mod phase_loss_bands {
    use super::{
        ElemChannels, ElementCap, ElementSnapshot, Tolerances, compare_element_phase_losses,
        phase_loss_band, tol_for,
    };
    use num_complex::Complex64;

    /// A physically consistent 2-terminal / 3-conductor / 3-phase line at
    /// 7.2 kV L-N carrying ~118 A with a small series loss: `Powers[k] =
    /// V_k·conj(I_k)·0.001` and `PhaseLosses[i] = Σ_j V_k·conj(I_k)` at
    /// `k = j·3 + i` — the exact identity `GetPhaseLosses` computes (r4133
    /// `Common/CktElement.pas:1093-1112`), so the port side is the oracle side
    /// ×1000 and the two agree to zero.
    ///
    /// The fixture is synthetic (the live numbers are what the corpus gate
    /// compares); what it pins is the band algebra and the shape rules, at
    /// magnitudes where the loss is a **near-cancellation** of the two terminal
    /// powers — 1.25 kW out of two ~863 kVA summands. That is the physical
    /// situation the band is derived for, and it is why the accepted error is
    /// set by the summands (3.16e-4 kW) and not by the answer.
    fn line_pair() -> (Vec<ElementSnapshot>, ElementCap) {
        let pl_w = [
            (1248.3880943264812, 1412.3963200937724),
            (1196.3719237295445, 1353.5464734231937),
            (1227.58162608766, 1388.8563814254594),
        ];
        let snap = ElementSnapshot {
            name: "Line.l1".to_string(),
            enabled: true,
            bus_names: vec!["src.1.2.3".to_string(), "b1.1.2.3".to_string()],
            powers: vec![Complex64::new(0.0, 0.0); 6],
            currents: vec![Complex64::new(0.0, 0.0); 6],
            loss_w: (0.0, 0.0),
            currents_mag_ang: Vec::new(),
            voltages_mag_ang: Vec::new(),
            residuals: Vec::new(),
            n_terms: 2,
            n_conds: 3,
            n_phases: 3,
            node_order: Vec::new(),
            energy_meter: None,
            phase_losses: pl_w.iter().map(|&(r, i)| Complex64::new(r, i)).collect(),
            num_controls: 0,
            ocp_dev_index: 0,
            ocp_dev_type: 0,
            has_volt_control: false,
            has_switch_control: false,
        };
        let cap = ElementCap {
            name: "Line.l1".to_string(),
            i_re: vec![
                108.756934444398,
                -94.20248509323407,
                -10.284377644223671,
                -108.756934444398,
                94.20248509323407,
                10.284377644223671,
            ],
            i_im: vec![
                -50.714191408883934,
                -65.96129018037028,
                117.55097437482597,
                50.714191408883934,
                65.96129018037028,
                -117.55097437482597,
            ],
            p_kw: vec![
                783.0499279996657,
                750.422847666346,
                769.9990958663378,
                -781.8015399053392,
                -749.2264757426165,
                -768.7715142402501,
            ],
            p_kvar: vec![
                365.1421781439643,
                349.9279207212994,
                359.0564751748979,
                -363.72978182387055,
                -348.5743742478762,
                -357.66761879347246,
            ],
            enabled: Some(true),
            n_terms: Some(2),
            n_conds: Some(3),
            n_phases: Some(3),
            pl_kw: pl_w.iter().map(|&(r, _)| r * 0.001).collect(),
            pl_kvar: pl_w.iter().map(|&(_, i)| i * 0.001).collect(),
            ..ElementCap::default()
        };
        (vec![snap], cap)
    }

    /// The same three phases with a fourth, **neutral** conductor per terminal —
    /// the shape `GetPhaseLosses` ignores by construction (`i` runs to
    /// `Fnphases`, not `FNconds`).
    fn line_cap_with_neutral() -> ElementCap {
        let (_, base) = line_pair();
        let ins = |v: &[f64], n: (f64, f64)| -> Vec<f64> {
            let mut out = v[0..3].to_vec();
            out.push(n.0);
            out.extend_from_slice(&v[3..6]);
            out.push(n.1);
            out
        };
        ElementCap {
            i_re: ins(&base.i_re, (3.5, -3.5)),
            i_im: ins(&base.i_im, (1.25, -1.25)),
            p_kw: ins(&base.p_kw, (0.047, -0.04225)),
            p_kvar: ins(&base.p_kvar, (-0.001, 0.0032500000000000003)),
            n_conds: Some(4),
            ..base
        }
    }

    fn feeder() -> Tolerances {
        tol_for("feeder")
    }

    /// The acceptance leg: a port that reproduces the identity exactly compares
    /// clean, and the band it was judged against is the measured
    /// `Σ_j (i_abs·max(1, |V_k|) + i_rel·|S_k|)` of that phase's two conductors.
    #[test]
    fn the_measured_line_compares_clean_at_the_derived_band() {
        let (snaps, cap) = line_pair();
        compare_element_phase_losses(&snaps, &cap, &feeder(), "phase losses", ElemChannels::ALL);
        let t = feeder();
        let bands: Vec<f64> = (0..3)
            .map(|i| phase_loss_band(&cap, i, 2, 3, t.i_rel, t.i_abs))
            .collect();
        assert_eq!(
            bands,
            vec![
                0.00031648320000000007,
                0.00030929039999999996,
                0.00031360607999999997
            ]
        );
        // 3.16e-4 kW against a 1.25 kW loss is 2.5e-4 *relative to the answer* —
        // the near-cancellation the fixture doc describes. Relative to the
        // summands it is the tier's own 1e-7 rel + 1e-5 abs·|V|, and nothing
        // else: no floor is calibrated here.
        assert!(bands[0] / cap.pl_kw[0] > 2.5e-4 && bands[0] / cap.pl_kw[0] < 2.6e-4);
    }

    /// The band is the `Get_Losses` band restricted to the phase's conductors —
    /// so summing it over **every** conductor slot reproduces the whole-element
    /// band `compare_element_channels` uses (same helper, no second copy of the
    /// formula), and summing it over the phases alone is *at most* that: equal
    /// when `NConds == NPhases`, strictly less as soon as a neutral exists.
    #[test]
    fn the_phase_bands_sum_to_at_most_the_get_losses_band() {
        let t = feeder();
        let (_, cap) = line_pair();
        let phases: f64 = (0..3)
            .map(|i| phase_loss_band(&cap, i, 2, 3, t.i_rel, t.i_abs))
            .sum();
        let all: f64 = (0..3)
            .map(|c| phase_loss_band(&cap, c, 2, 3, t.i_rel, t.i_abs))
            .sum();
        // `NConds == NPhases`: the same set of conductors, hence the same sum.
        assert_eq!(phases, all);
        assert_eq!(phases, 0.00093937968);

        let cap4 = line_cap_with_neutral();
        let phases4: f64 = (0..3)
            .map(|i| phase_loss_band(&cap4, i, 2, 4, t.i_rel, t.i_abs))
            .sum();
        let all4: f64 = (0..4)
            .map(|c| phase_loss_band(&cap4, c, 2, 4, t.i_rel, t.i_abs))
            .sum();
        // The phase conductors are the same three, so their band is unchanged…
        assert_eq!(phases4, phases);
        // …and it is strictly under the whole-element band by the neutrals'
        // contribution — the sense in which this band is the tighter one.
        // (The 4-conductor whole-element band, summed conductor-group by
        // conductor-group as `phase_loss_band` groups them.)
        assert_eq!(all4, 0.0009593886185452729);
        assert!(phases4 < all4);
    }

    /// The two-sided calibration: an error just inside the band passes and one
    /// just outside it fails, on the same fixture.
    ///
    /// The perturbation is expressed **in units of the band**, not as a relative
    /// error of the answer: at a 1.25 kW loss built from two ~863 kVA summands a
    /// 1e-6 *relative* error is 400× under the band and would prove nothing. The
    /// band's own scale is the only honest yardstick here.
    #[test]
    fn an_error_inside_the_band_passes_and_one_outside_it_fails() {
        let t = feeder();
        let (_, probe) = line_pair();
        let band = phase_loss_band(&probe, 0, 2, 3, t.i_rel, t.i_abs);
        assert_eq!(band, 0.00031648320000000007);

        // Inside: half a band, in W on the port side.
        let (mut snaps, cap) = line_pair();
        snaps[0].phase_losses[0] += Complex64::new(0.5 * band * 1000.0, 0.0);
        compare_element_phase_losses(&snaps, &cap, &t, "inside", ElemChannels::ALL);

        // One ulp: indistinguishable, obviously inside.
        let (mut snaps, cap) = line_pair();
        let re = snaps[0].phase_losses[0].re;
        snaps[0].phase_losses[0].re = f64::from_bits(re.to_bits() + 1);
        compare_element_phase_losses(&snaps, &cap, &t, "one ulp", ElemChannels::ALL);

        // Outside: 1.5 bands.
        let (mut snaps, cap) = line_pair();
        snaps[0].phase_losses[0] += Complex64::new(0.0, 1.5 * band * 1000.0);
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compare_element_phase_losses(&snaps, &cap, &t, "outside", ElemChannels::ALL);
        }))
        .expect_err("an error of 1.5 bands must fail");
        let msg = err
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(
            msg.contains("PhaseLosses phase 0 differs"),
            "the panic reads {msg:?}"
        );
    }

    /// Every phase is banded in its own right — a corrupted *third* phase is not
    /// hidden by two clean ones.
    #[test]
    #[should_panic(expected = "PhaseLosses phase 2 differs")]
    fn each_phase_is_compared_separately() {
        let (mut snaps, cap) = line_pair();
        snaps[0].phase_losses[2] += Complex64::new(1.0, 0.0);
        compare_element_phase_losses(&snaps, &cap, &feeder(), "phase 2", ElemChannels::ALL);
    }

    /// The lane exclusion drops the **value** compare only: with
    /// `phase_losses = false` a wildly wrong value passes (that is what the
    /// exclusion means, and saying it out loud is the point of the pin) while a
    /// shape mismatch still fails.
    #[test]
    fn the_lane_exclusion_drops_the_value_but_never_the_shape() {
        let (mut snaps, cap) = line_pair();
        snaps[0].phase_losses[0] = Complex64::new(1e9, -1e9);
        compare_element_phase_losses(
            &snaps,
            &cap,
            &feeder(),
            "newton",
            ElemChannels::CURRENTS_ONLY,
        );
        const { assert!(!ElemChannels::CURRENTS_ONLY.phase_losses) };

        let (mut snaps, cap) = line_pair();
        snaps[0].phase_losses.pop();
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compare_element_phase_losses(
                &snaps,
                &cap,
                &feeder(),
                "newton",
                ElemChannels::CURRENTS_ONLY,
            );
        }))
        .expect_err("a shape mismatch must fail under every channel policy");
        let msg = err
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(
            msg.contains("rust PhaseLosses length 2 != NumPhases (3)"),
            "the panic reads {msg:?}"
        );
    }

    /// Both sides must carry exactly `NumPhases` samples — the oracle side too,
    /// which is what catches a capture that answered with the capi
    /// `DefaultResult` sentinel or with an off-flag empty array.
    #[test]
    fn both_sides_must_carry_numphases_samples() {
        for (what, wreck) in [
            ("oracle PhaseLosses length 2", 0usize),
            ("oracle PhaseLosses length 0", 1usize),
            ("kW (3) and kvar (2) halves", 2usize),
        ] {
            let (snaps, mut cap) = line_pair();
            match wreck {
                0 => {
                    cap.pl_kw.pop();
                    cap.pl_kvar.pop();
                }
                1 => {
                    cap.pl_kw.clear();
                    cap.pl_kvar.clear();
                }
                _ => {
                    cap.pl_kvar.pop();
                }
            }
            let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compare_element_phase_losses(&snaps, &cap, &feeder(), "shape", ElemChannels::ALL);
            }))
            .expect_err(&format!("a capture whose {what} is wrong must fail"));
            let msg = err
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| err.downcast_ref::<&str>().copied())
                .unwrap_or("");
            assert!(msg.contains(what), "the panic reads {msg:?}");
        }
    }

    /// A 0-phase element (`UPFCControl`) carries no `PhaseLosses` on either
    /// side, accepted two-sidedly — measured `n = 0` on r4133 and `[]` on capi,
    /// so this is a real emptiness and not a sentinel normalization.
    #[test]
    fn a_zero_phase_element_is_empty_on_both_sides() {
        let (mut snaps, mut cap) = line_pair();
        snaps[0].phase_losses.clear();
        snaps[0].n_phases = 0;
        cap.pl_kw.clear();
        cap.pl_kvar.clear();
        cap.n_phases = Some(0);
        compare_element_phase_losses(&snaps, &cap, &feeder(), "upfc", ElemChannels::ALL);
    }

    /// The element must exist in the snapshot — an existence miss is never a
    /// silent skip, under any channel policy.
    #[test]
    #[should_panic(expected = "no element Line.l1")]
    fn an_element_missing_from_the_snapshot_fails_the_phase_loss_compare() {
        let (_, cap) = line_pair();
        compare_element_phase_losses(&[], &cap, &feeder(), "missing", ElemChannels::CURRENTS_ONLY);
    }

    /// `NumPhases` is required here too: an element captured without the
    /// `compare_element_extras` flag must fail the case rather than compare an
    /// empty pair against an empty pair.
    #[test]
    #[should_panic(expected = "carries no `n_phases` field")]
    fn the_comparator_requires_the_capture_to_carry_numphases() {
        let (snaps, mut cap) = line_pair();
        cap.n_phases = None;
        compare_element_phase_losses(&snaps, &cap, &feeder(), "no flag", ElemChannels::ALL);
    }

    /// The band is built from the oracle's own `Powers`/`Currents`, so the
    /// capture must explain its conductor layout: `NTerms·NConds` slots.
    #[test]
    #[should_panic(expected = "the oracle's Powers")]
    fn the_band_requires_a_consistent_conductor_layout() {
        let (snaps, mut cap) = line_pair();
        cap.p_kw.push(0.0);
        cap.p_kvar.push(0.0);
        compare_element_phase_losses(&snaps, &cap, &feeder(), "layout", ElemChannels::ALL);
    }
}

/// The G1.3d(i) discrete-extras pins: the no-meter sentinel normalization and
/// the comparator's structural rules, pinned directly (no oracle) the way
/// `derived_polar_floors` pins the polar bands. Every acceptance carries its
/// rejection leg, so none of them is a one-sided "accepts everything" green.
#[cfg(test)]
mod element_extras_pins {
    use super::{
        ElementCap, ElementSnapshot, PropsChannel, compare_element_extras, oracle_meter_name,
    };

    /// The measured `Line.l1` of the in-engine fixture deck
    /// (`exec::tests::element_extras`): 2 terminals × 3 conductors, 3 phases,
    /// metered by `EnergyMeter.Feeder`, `NodeOrder = [1, 2, 3, 1, 2, 3]`. The
    /// oracle side is spelled the way the **capi** transport spells it (a bare
    /// name, no sentinel).
    ///
    /// No control targets it in that deck, so all five G1.3d(ii) control
    /// scalars read zero/false — the *absence* half of the surface, which
    /// [`controlled_line`] complements with the presence half.
    fn metered_line() -> (Vec<ElementSnapshot>, ElementCap) {
        let snap = ElementSnapshot {
            name: "Line.l1".to_string(),
            enabled: true,
            bus_names: vec!["src.1.2.3".to_string(), "b1.1.2.3".to_string()],
            powers: vec![num_complex::Complex64::new(0.0, 0.0); 6],
            currents: vec![num_complex::Complex64::new(0.0, 0.0); 6],
            loss_w: (0.0, 0.0),
            currents_mag_ang: Vec::new(),
            voltages_mag_ang: Vec::new(),
            residuals: Vec::new(),
            n_terms: 2,
            n_conds: 3,
            n_phases: 3,
            node_order: vec![1, 2, 3, 1, 2, 3],
            energy_meter: Some("feeder".to_string()),
            phase_losses: vec![num_complex::Complex64::new(0.0, 0.0); 3],
            num_controls: 0,
            ocp_dev_index: 0,
            ocp_dev_type: 0,
            has_volt_control: false,
            has_switch_control: false,
        };
        let cap = ElementCap {
            name: "Line.l1".to_string(),
            i_re: vec![0.0; 6],
            i_im: vec![0.0; 6],
            enabled: Some(true),
            n_terms: Some(2),
            n_conds: Some(3),
            n_phases: Some(3),
            energy_meter: Some("feeder".to_string()),
            node_order: vec![1, 2, 3, 1, 2, 3],
            pl_kw: vec![0.0; 3],
            pl_kvar: vec![0.0; 3],
            num_controls: Some(0),
            ocp_dev_index: Some(0),
            ocp_dev_type: Some(0),
            has_volt_control: Some(false),
            has_switch_control: Some(false),
            ..ElementCap::default()
        };
        (vec![snap], cap)
    }

    /// An **enabled** `UPFCControl` (r4133
    /// `Version8/Source/Controls/UPFCControl.pas:230-246` never assigns
    /// `Nterms`): 0 terminals, 0 conductors, no `NodeOrder` read on either
    /// transport. The oracle side carries the **r4133** no-meter sentinel `'0'`
    /// here, so the fixture exercises the other spelling too.
    fn zero_terminal() -> (Vec<ElementSnapshot>, ElementCap) {
        let snap = ElementSnapshot {
            name: "UPFCControl.myupfcctrl".to_string(),
            enabled: true,
            bus_names: Vec::new(),
            powers: Vec::new(),
            currents: Vec::new(),
            loss_w: (0.0, 0.0),
            currents_mag_ang: Vec::new(),
            voltages_mag_ang: Vec::new(),
            residuals: Vec::new(),
            n_terms: 0,
            n_conds: 0,
            n_phases: 0,
            node_order: Vec::new(),
            energy_meter: None,
            phase_losses: Vec::new(),
            num_controls: 0,
            ocp_dev_index: 0,
            ocp_dev_type: 0,
            has_volt_control: false,
            has_switch_control: false,
        };
        let cap = ElementCap {
            name: "UPFCControl.myupfcctrl".to_string(),
            enabled: Some(true),
            n_terms: Some(0),
            n_conds: Some(0),
            n_phases: Some(0),
            energy_meter: Some("0".to_string()),
            num_controls: Some(0),
            ocp_dev_index: Some(0),
            ocp_dev_type: Some(0),
            has_volt_control: Some(false),
            has_switch_control: Some(false),
            ..ElementCap::default()
        };
        (vec![snap], cap)
    }

    /// The measured `Line.l1` of the control fixture
    /// (`exec::tests::element_extras::control_fixture`): a 3-phase line carrying
    /// a `Relay`, a `Fuse` and a `SwtControl`, i.e. `NumControls = 3`,
    /// `OCPDevIndex = 1`, `OCPDevType = 3` (Relay), `HasVoltControl = false`,
    /// `HasSwitchControl = true` — the same five values both oracle channels
    /// reported for it (G1.3d(ii) §1.3-1). The *presence* half of the surface;
    /// [`metered_line`] carries the absence half.
    fn controlled_line() -> (Vec<ElementSnapshot>, ElementCap) {
        let (mut snaps, mut cap) = metered_line();
        snaps[0].num_controls = 3;
        snaps[0].ocp_dev_index = 1;
        snaps[0].ocp_dev_type = 3;
        snaps[0].has_switch_control = true;
        cap.num_controls = Some(3);
        cap.ocp_dev_index = Some(1);
        cap.ocp_dev_type = Some(3);
        cap.has_switch_control = Some(true);
        (snaps, cap)
    }

    /// The acceptance leg: the measured fixture compares clean end to end.
    #[test]
    fn the_measured_fixture_element_compares_clean() {
        let (snaps, cap) = metered_line();
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "fixture");
    }

    /// Both transports' "no meter" spellings mean the same thing — capi's `''`
    /// (`Result := NIL`, `CAPI/CAPI_CktElement.pas:677`) and r4133's `'0'` (the
    /// `CktElementS` pre-`case` default, `DDLL/DCktElement.pas:421`) — but each
    /// is folded **only on its own channel**, and nothing else is normalized: a
    /// real name, including a numeric one the corpus actually carries
    /// (`EnergyMeter.25607`), stays itself on both.
    #[test]
    fn the_no_meter_sentinel_is_normalized_on_both_channels() {
        use PropsChannel::{CapiV0145 as CAPI, R4133 as R4};
        assert_eq!(oracle_meter_name("", CAPI), None);
        assert_eq!(oracle_meter_name("0", R4), None);
        // Each channel folds its OWN spelling only (audit settlement 2026-09-05):
        // capi never says `'0'`, so there it is a name; r4133 never says `''`, so
        // there an empty answer is unexpected and reds instead of being absorbed.
        assert_eq!(oracle_meter_name("0", CAPI), Some("0"));
        assert_eq!(oracle_meter_name("", R4), Some(""));
        for ch in [CAPI, R4] {
            assert_eq!(oracle_meter_name("feeder", ch), Some("feeder"));
            assert_eq!(oracle_meter_name("25607", ch), Some("25607"));
            assert_eq!(oracle_meter_name("00", ch), Some("00"));
            assert_eq!(oracle_meter_name("0.0", ch), Some("0.0"));
            assert_eq!(oracle_meter_name(" ", ch), Some(" "));
        }
        // End to end, on both spellings, against a port that says `None`.
        let (snaps, mut cap) = zero_terminal();
        compare_element_extras(&snaps, &cap, PropsChannel::R4133, "r4133 sentinel");
        cap.energy_meter = Some(String::new());
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "capi sentinel");
    }

    /// The collision leg: on the **r4133** channel a meter literally *named* `0`
    /// is indistinguishable from the sentinel, and a port that HAS the name reds
    /// against the folded `None` rather than passing silently. (The corpus
    /// carries no such name; `extras_population::no_corpus_energymeter_is_named_zero`
    /// in `crates/dss-core/tests/corpus_manifest.rs` is the census.)
    #[test]
    #[should_panic(expected = "EnergyMeter differs")]
    fn a_meter_named_zero_reds_instead_of_passing() {
        let (mut snaps, mut cap) = metered_line();
        snaps[0].energy_meter = Some("0".to_string());
        cap.energy_meter = Some("0".to_string());
        compare_element_extras(&snaps, &cap, PropsChannel::R4133, "collision");
    }

    /// …and the **other** direction of that same collision, asserted rather than
    /// assumed (G1.3d(i) audit settlement, 2026-09-05): a port that *lost* a
    /// meter named `0` compares `None == None` on r4133 and **passes**. The fold
    /// is therefore not self-detecting, and the corpus census — not the fold —
    /// is what keeps the case unreachable. On **capi** the same pair reds,
    /// because that channel's sentinel is `''` and `0` is just a name there.
    #[test]
    fn the_r4133_zero_sentinel_is_undecidable_and_the_census_is_the_guard() {
        let (mut snaps, mut cap) = metered_line();
        snaps[0].energy_meter = None;
        cap.energy_meter = Some("0".to_string());
        // r4133: indistinguishable from "no meter" — this passes, and saying so
        // out loud is the point of the pin.
        compare_element_extras(&snaps, &cap, PropsChannel::R4133, "r4133 undecidable");
        // capi: the same capture is a real name, and losing it is a failure.
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "capi decidable");
        }))
        .expect_err("a port that lost a meter named `0` must fail on the capi channel");
        let msg = err
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| err.downcast_ref::<&str>().copied())
            .unwrap_or("");
        assert!(
            msg.contains("EnergyMeter differs"),
            "the capi panic reads {msg:?}"
        );
    }

    /// The name is compared with **no** case folding — both engines and the port
    /// lowercase it in the constructor (r4133 `Meters/EnergyMeter.pas:921`,
    /// capi `src/Meters/EnergyMeter.pas:952`, port `elements/ckt.rs:258`), so a
    /// capture that came back in the deck's spelling is a finding, not noise.
    #[test]
    #[should_panic(expected = "EnergyMeter differs")]
    fn the_meter_name_is_compared_without_case_folding() {
        let (snaps, mut cap) = metered_line();
        cap.energy_meter = Some("Feeder".to_string());
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "case");
    }

    /// A metered element whose meter the port failed to resolve must red, in the
    /// direction a one-sided "the oracle said nothing" check would miss.
    #[test]
    #[should_panic(expected = "EnergyMeter differs")]
    fn a_port_that_lost_the_meter_name_fails() {
        let (mut snaps, cap) = metered_line();
        snaps[0].energy_meter = None;
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "lost meter");
    }

    /// The element-level twin of `capture_guard::require_capture`: all four
    /// scalars **and** `Enabled` are emitted for every element under the flag, so
    /// a missing one means the element was captured without it. Never papered
    /// over — each absence names its own field.
    #[test]
    fn the_extras_comparator_requires_the_capture_to_carry_them() {
        /// Removes one required field from an otherwise complete capture.
        type Drop = fn(&mut ElementCap);
        let drop: [(&str, Drop); 10] = [
            ("n_terms", |c| c.n_terms = None),
            ("n_conds", |c| c.n_conds = None),
            ("n_phases", |c| c.n_phases = None),
            ("energy_meter", |c| c.energy_meter = None),
            ("enabled", |c| c.enabled = None),
            ("num_controls", |c| c.num_controls = None),
            ("ocp_dev_index", |c| c.ocp_dev_index = None),
            ("ocp_dev_type", |c| c.ocp_dev_type = None),
            ("has_volt_control", |c| c.has_volt_control = None),
            ("has_switch_control", |c| c.has_switch_control = None),
        ];
        for (field, remove) in drop {
            let (snaps, mut cap) = metered_line();
            remove(&mut cap);
            let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "no-capture");
            }))
            .expect_err(&format!("a capture without `{field}` must fail the case"));
            let msg = err
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| err.downcast_ref::<&str>().copied())
                .unwrap_or("");
            assert!(
                msg.contains(&format!("carries no `{field}` field")),
                "the panic for a missing `{field}` reads {msg:?}"
            );
        }
    }

    /// The three counts are compared exactly, each in its own right —
    /// `NumPhases` in particular is the one no other channel's shape implies.
    #[test]
    fn the_counts_are_compared_exactly() {
        for (what, corrupt) in [
            ("NumTerminals", 0usize),
            ("NumConductors", 1usize),
            ("NumPhases", 2usize),
        ] {
            let (mut snaps, cap) = metered_line();
            match corrupt {
                0 => snaps[0].n_terms += 1,
                1 => snaps[0].n_conds += 1,
                _ => snaps[0].n_phases += 1,
            }
            let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "count");
            }))
            .expect_err(&format!("a corrupted {what} must fail"));
            let msg = err
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| err.downcast_ref::<&str>().copied())
                .unwrap_or("");
            assert!(
                msg.contains(&format!("{what} differs")),
                "the panic for a corrupted {what} reads {msg:?}"
            );
        }
    }

    /// The `Yorder` tie: the oracle's own counts must explain the length of the
    /// currents array it sent under the same capture.
    #[test]
    #[should_panic(expected = "the oracle's Currents length")]
    fn the_counts_must_explain_the_oracle_currents_length() {
        let (snaps, mut cap) = metered_line();
        cap.i_re.push(0.0);
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "yorder");
    }

    /// `NodeOrder` is compared slot by slot: a rotation keeps every value and the
    /// length and still fails.
    #[test]
    #[should_panic(expected = "NodeOrder differs")]
    fn the_node_order_is_compared_slot_by_slot() {
        let (snaps, mut cap) = metered_line();
        cap.node_order = vec![2, 3, 1, 1, 2, 3];
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "rotate");
    }

    /// …and its length must be `NTerms · NConds` on **both** sides.
    #[test]
    #[should_panic(expected = "oracle NodeOrder length 5")]
    fn a_short_oracle_node_order_fails() {
        let (snaps, mut cap) = metered_line();
        cap.node_order.pop();
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "short oracle");
    }

    #[test]
    #[should_panic(expected = "rust NodeOrder length 5")]
    fn a_short_port_node_order_fails() {
        let (mut snaps, cap) = metered_line();
        snaps[0].node_order.pop();
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "short rust");
    }

    /// A 0-terminal element carries no `NodeOrder` on either side (the read is
    /// never issued), accepted two-sidedly.
    #[test]
    fn a_zero_terminal_element_has_no_node_order_on_either_side() {
        let (snaps, cap) = zero_terminal();
        compare_element_extras(&snaps, &cap, PropsChannel::R4133, "upfc");
    }

    #[test]
    #[should_panic(expected = "0-terminal element must carry no NodeOrder")]
    fn a_zero_terminal_element_with_an_oracle_node_order_fails() {
        let (snaps, mut cap) = zero_terminal();
        cap.node_order = vec![1];
        compare_element_extras(&snaps, &cap, PropsChannel::R4133, "upfc");
    }

    #[test]
    #[should_panic(expected = "0-terminal element must carry no NodeOrder")]
    fn a_zero_terminal_element_with_a_port_node_order_fails() {
        let (mut snaps, cap) = zero_terminal();
        snaps[0].node_order = vec![1];
        compare_element_extras(&snaps, &cap, PropsChannel::R4133, "upfc");
    }

    /// A **disabled** element: the capture skips the `NodeOrder` read (a
    /// never-enabled element has no `NodeRef`), so only the *oracle* side must be
    /// empty. The port keeps the mapping it was last given — an element disabled
    /// **after** a solve still carries its full `NodeOrder`, measured and pinned
    /// in-engine by
    /// `exec::tests::element_extras::a_disabled_element_keeps_the_node_order_it_was_given`
    /// — so requiring the port to be empty here would red on every deck that
    /// switches an element out.
    #[test]
    fn a_disabled_element_keeps_its_port_node_order_while_the_oracle_stays_silent() {
        let (mut snaps, mut cap) = metered_line();
        snaps[0].enabled = false;
        cap.enabled = Some(false);
        cap.node_order = Vec::new();
        // The port still carries [1, 2, 3, 1, 2, 3] — and the counts, the meter
        // and `Enabled` itself are all still compared.
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "disabled");
    }

    #[test]
    #[should_panic(expected = "disabled element must carry no oracle NodeOrder")]
    fn a_disabled_element_with_an_oracle_node_order_fails() {
        let (mut snaps, mut cap) = metered_line();
        snaps[0].enabled = false;
        cap.enabled = Some(false);
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "disabled");
    }

    /// `Enabled` is re-asserted here, not borrowed from
    /// [`super::compare_element_derived`]: the two manifest flags are
    /// independent, and this bit is the `NodeOrder` capture predicate.
    #[test]
    #[should_panic(expected = "Enabled differs")]
    fn enabled_is_compared_by_this_comparator_too() {
        let (mut snaps, cap) = metered_line();
        snaps[0].enabled = false;
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "enabled");
    }

    /// The acceptance leg of the G1.3d(ii) five: the measured control-bearing
    /// element compares clean end to end.
    #[test]
    fn the_measured_controlled_element_compares_clean() {
        let (snaps, cap) = controlled_line();
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "controlled");
    }

    /// …and each of the five is compared in its own right: corrupting exactly
    /// one of them on the port side reds with that field's own message. The two
    /// booleans are corrupted in **both** directions, since a one-sided flip
    /// would be caught by an `any`-shaped bug only half the time.
    #[test]
    fn the_control_extras_are_compared_exactly() {
        type Corrupt = fn(&mut ElementSnapshot);
        let cases: [(&str, Corrupt); 7] = [
            ("NumControls", |s| s.num_controls += 1),
            ("OCPDevIndex", |s| s.ocp_dev_index += 1),
            ("OCPDevType", |s| s.ocp_dev_type = 2),
            ("HasVoltControl", |s| s.has_volt_control = true),
            ("HasSwitchControl", |s| s.has_switch_control = false),
            ("HasVoltControl", |s| {
                s.has_volt_control = true;
                s.has_switch_control = true;
            }),
            ("HasSwitchControl", |s| {
                s.has_volt_control = false;
                s.has_switch_control = false;
            }),
        ];
        for (what, corrupt) in cases {
            let (mut snaps, cap) = controlled_line();
            corrupt(&mut snaps[0]);
            let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "extras");
            }))
            .expect_err(&format!("a corrupted {what} must fail"));
            let msg = err
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| err.downcast_ref::<&str>().copied())
                .unwrap_or("");
            assert!(
                msg.contains(&format!("{what} differs")),
                "the panic for a corrupted {what} reads {msg:?}"
            );
        }
    }

    /// The first OCP tie, on the **oracle** side: one `repeat … until (i >
    /// listSize) or (Result > 0)` walk produces both scalars (r4133
    /// `DDLL/DCktElement.pas:242-258`, `Common/Utilities.pas:3165-3184`), so a
    /// capture in which exactly one of them is zero is a transport bug. Both
    /// directions of the disagreement are exercised.
    #[test]
    fn the_oracle_ocp_index_and_type_are_zero_together() {
        for (idx, ty) in [(Some(0), Some(3)), (Some(1), Some(0))] {
            let (snaps, mut cap) = controlled_line();
            cap.ocp_dev_index = idx;
            cap.ocp_dev_type = ty;
            // Keep the port in step so the plain equality asserts pass first and
            // the tie is what actually reds.
            let mut snaps = snaps;
            snaps[0].ocp_dev_index = usize::try_from(idx.unwrap()).unwrap();
            snaps[0].ocp_dev_type = ty.unwrap();
            let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "ocp tie");
            }))
            .expect_err("an oracle OCP pair that is half zero must fail");
            let msg = err
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| err.downcast_ref::<&str>().copied())
                .unwrap_or("");
            assert!(
                msg.contains("must be zero together"),
                "the panic for {idx:?}/{ty:?} reads {msg:?}"
            );
        }
        // …and the two legitimate pairs still pass.
        let (snaps, cap) = controlled_line();
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "ocp present");
        let (snaps, cap) = metered_line();
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "ocp absent");
    }

    /// The second OCP tie: `OCPDevIndex` is a 1-based position in the list whose
    /// size is `NumControls`, so it can never exceed it.
    #[test]
    #[should_panic(expected = "exceeds its own")]
    fn the_oracle_ocp_index_cannot_exceed_the_control_count() {
        let (mut snaps, mut cap) = controlled_line();
        snaps[0].ocp_dev_index = 4;
        cap.ocp_dev_index = Some(4);
        compare_element_extras(&snaps, &cap, PropsChannel::CapiV0145, "ocp bound");
    }

    /// An element the oracle sent and the port does not have is an existence
    /// miss, never a silent skip.
    #[test]
    #[should_panic(expected = "no element Line.l1")]
    fn an_element_missing_from_the_snapshot_fails() {
        let (_, cap) = metered_line();
        compare_element_extras(&[], &cap, PropsChannel::CapiV0145, "missing");
    }

    /// The population guard behind D-ii-1's zero ledger rows, accepting
    /// direction: the measured population (18 multi-control rows, all of them
    /// Relay-only lists) is what makes the r4133 per-edit re-attach unobservable.
    #[test]
    fn the_control_census_passes_on_the_measured_population() {
        super::check_control_census(298_565, 3_184, 18, 18);
    }

    /// …and its refusing direction, in **both** ways it can break: a
    /// multi-control population that grew (a nineteenth row — D-ii-1 may become
    /// observable) or shrank, and a census that counted nothing (the gating
    /// extras compare stopped running).
    #[test]
    #[should_panic(expected = "the multi-control population moved")]
    fn the_control_census_fires_on_a_new_multi_control_element() {
        super::check_control_census(298_565, 3_184, 19, 19);
    }

    /// The non-vacuity half of the same rule.
    #[test]
    #[should_panic(expected = "the control census looks broken")]
    fn the_control_census_fires_when_nothing_was_counted() {
        super::check_control_census(0, 0, 18, 18);
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
    //     `tests/TOLERANCE_NOTES.md:1896-1901` pins the r4133-side values and
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
    //     `tests/TOLERANCE_NOTES.md:1896-1901` says it outright ("The r4133 values
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
    //     witness it live — the same argument `tests/TOLERANCE_NOTES.md:1896-1901`
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
///    r4133 by construction, and `tests/TOLERANCE_NOTES.md:1896-1901` forbids
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
    /// r4133 values (`RevThreshold`, Fuse) `tests/TOLERANCE_NOTES.md:1896-1901`
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
/// [`PropsPolicy`]) and the census [`collect_prop_divergences`]. Since
/// GOLDEN_REBASE G1.3d(i) [`compare_element_extras`] reads it too (its no-meter
/// sentinel is channel-specific), so the name is historical: this is the
/// harness's channel type, not a property-only one.
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
// GOLDEN_REBASE G1.6b: the `PDElements` interface walk.
//
// The dss-python `ActiveCircuit.PDElements` surface the fastdss harness
// compares wholesale (`DSS-Python@origin/fastdss:dss/IPDElements.py:26-40`
// `_columns`, archived by `tests/save_outputs.py:365`), captured live on both
// channels (`tools/oracle/oracle_server.py::capture_pd_elements`,
// `crates/dss-epri/src/capture.rs::capture_pd_elements`) and compared against
// `Dss::pd_elements`.
// ---------------------------------------------------------------------------

/// One enabled PD element's `PDElements` record, exactly as **both** channels
/// serialize it: the thirteen `IPDElements._columns` fields plus `parent_name`.
///
/// The two transports are field-for-field identical by construction — the capi
/// side builds the dict in `oracle_server.capture_pd_elements`, the r4133 side
/// serializes `dss-epri`'s `PdElementCap` — so one struct deserializes both. No
/// `#[serde(default)]` anywhere: a channel that drops a field must fail loudly
/// here rather than compare a zero.
///
/// Values are the **stored** `TPDElement` fields. `lambda` is `BranchFltRate`
/// and `accumulated_l` is `AccumulatedBrFltRate` — both 0 until an EnergyMeter
/// `RelCalc` sweep writes them (r4133 `Version8/Source/PDElements/
/// PDElement.pas:106-110`, whose only caller is `Meters/EnergyMeter.pas`'s
/// `CalcReliabilityIndices`) — while `fault_rate` / `pct_permanent` are that
/// sweep's inputs.
#[derive(Debug, Clone, Deserialize)]
pub struct PdElementCap {
    pub name: String,
    pub accumulated_l: f64,
    /// 1-based, as both oracles report it (`TPDElement.FromTerminal`); 0 =
    /// unset.
    pub from_terminal: i32,
    /// r4133's `PDElementsI(3)` answers 0/1 and the capi side a bool; the r4133
    /// capture normalizes to bool, so the wire shape is one type.
    pub is_shunt: bool,
    pub num_customers: i32,
    pub section_id: i32,
    pub fault_rate: f64,
    pub repair_time: f64,
    /// `AccumulatedMilesDownStream` — a different quantity from
    /// `Bus.TotalMiles` (`CAPI/CAPI_PDElements.pas:294-303`,
    /// `Version8/Source/DDLL/DPDELements.pas:201-209`).
    pub total_miles: f64,
    pub total_customers: i32,
    pub pct_permanent: f64,
    pub lambda: f64,
    /// The parent's `ClassIndex` (1-based, per-class creation order,
    /// `General/DSSObject.pas:43`), 0 = no upline parent.
    pub parent_class_index: i32,
    /// The parent's full `Class.Name`, `""` when the index is 0 — not an oracle
    /// column of its own but the element the `ParentPDElement` read left
    /// active, which carries strictly more information than the bare class
    /// index (88 distinct values against 85 on IEEE123).
    pub parent_name: String,
}

/// The record's fourteen JSON keys, in the frozen order that **is** the capture
/// read order (`GOLDEN_REBASE_PLAN.md` §G1.6b; the `ParentPDElement` pair last).
/// Used by [`PD_SKIP_FIELDS`]' register test to reject a row naming a field the
/// record does not have.
const PD_FIELD_NAMES: [&str; 14] = [
    "name",
    "accumulated_l",
    "from_terminal",
    "is_shunt",
    "num_customers",
    "section_id",
    "fault_rate",
    "repair_time",
    "total_miles",
    "total_customers",
    "pct_permanent",
    "lambda",
    "parent_class_index",
    "parent_name",
];

/// One field of a `PDElements` record, type-tagged so the comparator can walk
/// all fourteen uniformly (one skip lookup, one accounting site, one message
/// shape) instead of fourteen hand-written asserts.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PdVal<'a> {
    F(f64),
    I(i32),
    B(bool),
    S(&'a str),
}

impl PdVal<'_> {
    /// Equality as this surface defines it (named `matches`, not `eq`, so the
    /// derived `PartialEq` — which is exact on the name fields too — stays
    /// reachable for the unit tests): **exact** for every numeric field
    /// (see [`compare_pd_elements`] for why there is no tolerance), and
    /// case-insensitive for the two name fields — the port renders
    /// `Capacitor.c83` (capitalized class, lowercased object name) while both
    /// oracles echo the deck's own spelling (`Capacitor.C83`).
    fn matches(self, other: Self) -> bool {
        match (self, other) {
            (PdVal::F(a), PdVal::F(b)) => a == b,
            (PdVal::I(a), PdVal::I(b)) => a == b,
            (PdVal::B(a), PdVal::B(b)) => a == b,
            (PdVal::S(a), PdVal::S(b)) => a.eq_ignore_ascii_case(b),
            _ => false,
        }
    }

    /// Full-precision rendering for a failure message (`{:?}` on `f64` is the
    /// shortest round-tripping decimal, so a denormal reads as `1.03e-311`
    /// rather than `0`).
    fn render(self) -> String {
        match self {
            PdVal::F(x) => format!("{x:?}"),
            PdVal::I(x) => x.to_string(),
            PdVal::B(x) => x.to_string(),
            PdVal::S(s) => format!("{s:?}"),
        }
    }
}

/// The fourteen `(field name, value)` pairs of one record, in
/// [`PD_FIELD_NAMES`] order.
///
/// A macro rather than two functions on purpose: it expands over **both**
/// [`PdElementCap`] (the oracle side) and `dss_core`'s `PdElementView` (the
/// port side), so the two extractions cannot drift — a field renamed on either
/// type stops this file compiling.
macro_rules! pd_fields {
    ($x:expr) => {{
        let r = $x;
        [
            ("name", PdVal::S(&r.name)),
            ("accumulated_l", PdVal::F(r.accumulated_l)),
            ("from_terminal", PdVal::I(r.from_terminal)),
            ("is_shunt", PdVal::B(r.is_shunt)),
            ("num_customers", PdVal::I(r.num_customers)),
            ("section_id", PdVal::I(r.section_id)),
            ("fault_rate", PdVal::F(r.fault_rate)),
            ("repair_time", PdVal::F(r.repair_time)),
            ("total_miles", PdVal::F(r.total_miles)),
            ("total_customers", PdVal::I(r.total_customers)),
            ("pct_permanent", PdVal::F(r.pct_permanent)),
            ("lambda", PdVal::F(r.lambda)),
            ("parent_class_index", PdVal::I(r.parent_class_index)),
            ("parent_name", PdVal::S(&r.parent_name)),
        ]
    }};
}

/// One `PDElements` cell an oracle channel reads out of **uninitialized
/// memory**, excluded field-by-field with its pin. See [`PD_SKIP_FIELDS`] for
/// the mechanism and the measurements.
pub struct PdSkipRow {
    /// [`PropsChannel::tag`] of the channel whose value is garbage.
    pub channel: &'static str,
    /// The element class the row applies to (matched case-insensitively).
    pub class: &'static str,
    /// The [`PD_FIELD_NAMES`] key this row drops on that channel.
    pub field: &'static str,
    /// The expected-value test that pins the port's correct value against the
    /// measured garbage — every exclusion names its pin
    /// (`GOLDEN_REBASE_PLAN.md` §1.1(e)).
    pub pin: &'static str,
    /// The upstream source line the defect lives on.
    pub cite: &'static str,
}

/// **The PDElements cells an oracle channel cannot be compared on** — a proven
/// uninitialized read in the EnergyMeter zone build, present on BOTH oracles.
///
/// `TEnergyMeter.MakeMeterZoneLists` puts shunt Capacitors and Reactors on the
/// **PC** adjacency list (r4133 `Version8/Source/Meters/EnergyMeter.pas:1855`,
/// comment *"Capacitor and Reactor put on the PC list if IsShunt=TRUE"*) and
/// then assigns through a `pPCelem: TPCElement` cursor (`:1739` decl, `:1843`
/// assignment):
///
/// ```text
/// If Not pPCelem.HasSensorObj then pPCelem.SensorObj := TPDElement(ActiveBranch).SensorObj;
/// pPCelem.MeterObj := Self;                                  {EnergyMeter.pas:1868-1869}
/// ```
///
/// The object is a `TPDElement`, so the two writes land at `TPCElement`'s field
/// offsets on a `TPDElement` instance — i.e. on two adjacent `Double`s of the
/// PD reliability block. capi 0.14.5 carries the identical code
/// (`.inputs/dss_capi/src/Meters/EnergyMeter.pas:1783` decl, `:1927-1929`) with
/// a different class layout, so a **different** pair of doubles is hit.
///
/// **Measured** (G1.6b part R, corpus-wide over the live non-`large`
/// population): `capi_v0145` corrupts `FaultRate` + `pctPermanent` — 194 cells
/// (96 + 96 Capacitor, 1 + 1 Reactor) over 22 cases; `r4133` corrupts
/// `Lambda` + `AccumulatedL` — 214 cells (105 + 105 Capacitor, 2 + 2 Reactor)
/// over 29 cases. The trigger is the zone build, not the element: a shunt capacitor
/// reads clean with a CapControl, a Monitor, a sibling capacitor or a
/// transformer present and goes garbage the moment an EnergyMeter is added,
/// already after `Compile` and before any solve. A metered deck whose capacitor
/// sits outside the zone stays clean, which is why the exclusion cannot be
/// keyed on the deck.
///
/// **Nondeterministic**: the same cells come back every run with a different
/// value every run — a heap pointer under ASLR. capi `Capacitor.c83.FaultRate`
/// measured `1.043284808626e-311` / `8.210645577926e-312` /
/// `1.511547243828e-311` in three separate processes; r4133
/// `Capacitor.cap1.Lambda` `7.20016348108e-312` then `7.20016742902e-312`
/// **inside one worker process**. An envelope over such a value is not a fact
/// (the `reliability_bus_int_duration_oob_bug_report.md` precedent), and the 51
/// (case, channel) pairs it touches would blow the plan's ~10-entry ledger kill
/// criterion with identical rows — so the exclusion lives here, beside
/// [`SKIP_PROPS_BOTH_CHANNELS`], which excludes the very same defect on the
/// property surface, and `tests/corpus/ledger.json` gains **nothing**.
///
/// **Scope — (channel, class, field) AND the element.** A row is consulted only
/// where the corrupting write actually lands: on an **in-zone shunt**
/// Capacitor/Reactor ([`pd_skip_applies`]). A series member of either class, and
/// a shunt one no meter zone reached, stays compared on all fourteen fields on
/// both channels — the defect touches 22 (capi) / 29 (r4133) of the 372 / 431
/// walked cases, so a class-wide row would drop ~350 clean comparisons per
/// channel, and Capacitor/Reactor `FaultRate`/`PctPerm` have no live check left
/// anywhere else (`SKIP_PROPS_BOTH_CHANNELS`/`SKIP_PROPS_CAPI_ONLY` drop them on
/// the property surface for this same defect).
///
/// The remaining asymmetry is coverage, not loss: `fault_rate` /
/// `pct_permanent` stay compared on `r4133` and `lambda` / `accumulated_l` stay
/// compared on `capi_v0145`, and Line / Transformer / AutoTrans /
/// GICTransformer keep all four fields on both channels.
///
/// Two guards keep the table honest, both in the `PROPS_ECHO_R4133` spirit:
/// `the_pd_skip_register_is_exactly_the_measured_rows` (a row cannot be added
/// or dropped without an edit that says so) and
/// [`assert_pd_skip_rows_are_live`] (a row that excludes nothing fails the
/// gate).
pub const PD_SKIP_FIELDS: &[PdSkipRow] = &[
    PdSkipRow {
        channel: "capi_v0145",
        class: "Capacitor",
        field: "fault_rate",
        pin: "pd_elements_shunt_reliability_inputs_survive_the_meter_zone",
        cite: "dss_capi/src/Meters/EnergyMeter.pas:1927-1929",
    },
    PdSkipRow {
        channel: "capi_v0145",
        class: "Capacitor",
        field: "pct_permanent",
        pin: "pd_elements_shunt_reliability_inputs_survive_the_meter_zone",
        cite: "dss_capi/src/Meters/EnergyMeter.pas:1927-1929",
    },
    PdSkipRow {
        channel: "capi_v0145",
        class: "Reactor",
        field: "fault_rate",
        pin: "pd_elements_shunt_reliability_inputs_survive_the_meter_zone",
        cite: "dss_capi/src/Meters/EnergyMeter.pas:1927-1929",
    },
    PdSkipRow {
        channel: "capi_v0145",
        class: "Reactor",
        field: "pct_permanent",
        pin: "pd_elements_shunt_reliability_inputs_survive_the_meter_zone",
        cite: "dss_capi/src/Meters/EnergyMeter.pas:1927-1929",
    },
    PdSkipRow {
        channel: "r4133",
        class: "Capacitor",
        field: "lambda",
        pin: "pd_elements_shunt_branch_flt_rate_survives_the_meter_zone",
        cite: "Version8/Source/Meters/EnergyMeter.pas:1868-1869",
    },
    PdSkipRow {
        channel: "r4133",
        class: "Capacitor",
        field: "accumulated_l",
        pin: "pd_elements_shunt_branch_flt_rate_survives_the_meter_zone",
        cite: "Version8/Source/Meters/EnergyMeter.pas:1868-1869",
    },
    PdSkipRow {
        channel: "r4133",
        class: "Reactor",
        field: "lambda",
        pin: "pd_elements_shunt_branch_flt_rate_survives_the_meter_zone",
        cite: "Version8/Source/Meters/EnergyMeter.pas:1868-1869",
    },
    PdSkipRow {
        channel: "r4133",
        class: "Reactor",
        field: "accumulated_l",
        pin: "pd_elements_shunt_branch_flt_rate_survives_the_meter_zone",
        cite: "Version8/Source/Meters/EnergyMeter.pas:1868-1869",
    },
];

/// The file, relative to `crates/dss-core`, that must define every
/// [`PdSkipRow::pin`] — checked by
/// `pd_elements_tests::every_pd_skip_row_pin_is_a_test_that_exists`.
const PD_PINS_FILE: &str = "tests/pd_elements_pins.rs";

/// Per-row visit counter, indexed exactly like [`PD_SKIP_FIELDS`]: cells the
/// row was consulted about.
static PD_SKIP_VISITS: [AtomicUsize; PD_SKIP_FIELDS.len()] =
    [const { AtomicUsize::new(0) }; PD_SKIP_FIELDS.len()];
/// Per-row hit counter: visits whose two sides actually differed, i.e. value
/// compares this row really excluded.
static PD_SKIP_HITS: [AtomicUsize; PD_SKIP_FIELDS.len()] =
    [const { AtomicUsize::new(0) }; PD_SKIP_FIELDS.len()];

/// Gating `PDElements` walks this process compared, per channel, indexed by
/// [`pd_channel_slot`].
static PD_WALKS: [AtomicUsize; 2] = [const { AtomicUsize::new(0) }; 2];
/// …and the elements those walks compared.
static PD_ELEMENTS: [AtomicUsize; 2] = [const { AtomicUsize::new(0) }; 2];

/// Index into [`PD_WALKS`]/[`PD_ELEMENTS`] for a channel.
fn pd_channel_slot(channel: PropsChannel) -> usize {
    match channel {
        PropsChannel::CapiV0145 => 0,
        PropsChannel::R4133 => 1,
    }
}

/// The element class of a full `Class.name`, or the whole string when it
/// carries no dot (which no PD element name does — both oracles answer
/// `FullName`).
fn pd_class(full_name: &str) -> &str {
    full_name.split_once('.').map_or(full_name, |(c, _)| c)
}

/// Which [`PD_SKIP_FIELDS`] row covers `(channel, class, field)`, if any.
///
/// A pure function so both directions are provable offline
/// (`the_pd_skip_lookup_matches_only_its_own_channel_class_and_field`) — the
/// shipped counters cannot be rewound once a gate run has moved them.
fn pd_skip_row(channel: &str, class: &str, field: &str) -> Option<usize> {
    PD_SKIP_FIELDS.iter().position(|r| {
        r.channel == channel && r.class.eq_ignore_ascii_case(class) && r.field == field
    })
}

/// Whether [`PD_SKIP_FIELDS`] may speak about this element at all.
///
/// The defect the table names is not a property of the class: it is the write
/// `MakeMeterZoneLists` performs on what it files on the meter's **PC**
/// adjacency list — an enabled **shunt** Capacitor/Reactor inside some meter's
/// zone (`Version8/Source/Shared/CktTree.pas:664-666` files them there,
/// `Meters/EnergyMeter.pas:1868-1869` then writes `MeterObj`/`SensorObj`
/// through a `TPCElement` cursor aimed at that `TPDElement`). A series
/// Capacitor/Reactor goes on the PD list instead and is written by nothing; a
/// shunt one outside every zone is never reached. Both read clean on both
/// oracles — `Reactor.rser` is clean on the very deck whose `Reactor.rsh` is
/// garbage (`pd_elements_shunt_reliability_inputs_survive_the_meter_zone`).
///
/// Both halves are the PORT's own state, and neither can hide a divergence:
/// `is_shunt` is itself compared (field 3 of 14, ahead of every skipped field),
/// so a port that got it wrong reds on that field; `in_meter_zone` is not an
/// oracle column at all ([`PdElementView::in_meter_zone`]), and a port that got
/// *it* wrong would suppress two cells whose correct value is a parsed class
/// default or an untouched `0.0` — the numbers both pins hold literally.
fn pd_skip_applies(a: &PdElementView) -> bool {
    a.is_shunt && a.in_meter_zone
}

/// What [`compare_pd_elements`] has counted in this process, as
/// `(capi walks, capi elements, r4133 walks, r4133 elements)`.
pub fn pd_walk_counters() -> (usize, usize, usize, usize) {
    (
        PD_WALKS[0].load(AtomicOrd::Relaxed),
        PD_ELEMENTS[0].load(AtomicOrd::Relaxed),
        PD_WALKS[1].load(AtomicOrd::Relaxed),
        PD_ELEMENTS[1].load(AtomicOrd::Relaxed),
    )
}

/// One [`PD_SKIP_FIELDS`] row's live `(visits, hits)`, or `None` when the table
/// has no such row.
pub fn pd_skip_counters(channel: &str, class: &str, field: &str) -> Option<(usize, usize)> {
    let i = pd_skip_row(channel, class, field)?;
    Some((
        PD_SKIP_VISITS[i].load(AtomicOrd::Relaxed),
        PD_SKIP_HITS[i].load(AtomicOrd::Relaxed),
    ))
}

/// Compare one channel's `PDElements` walk against the engine's
/// (`GOLDEN_REBASE_PLAN.md` §G1.6b).
///
/// The walk itself is asserted first — length, then the name **sequence**,
/// case-insensitively — because it is the membership *and* the order contract
/// in one: both oracles iterate the circuit's `PDElements` pointer list
/// skipping `not Enabled` (capi `CAPI/CAPI_Utils.pas:718-759`, r4133
/// `Version8/Source/DDLL/DPDELements.pas:27-59`), `Fault` objects never appear
/// (`PDElements/Fault.pas:114`), and `Dss::pd_elements` walks
/// `Circuit.pd_elements` in the same `AddCktElement` creation order. The
/// oracle's `PDElements.Count` is the raw `ListSize` and counts disabled
/// elements too — measured 2 against a walk of 1 — so the walk, never `Count`,
/// is what is compared.
///
/// Then every field of every record, **exactly**: `rel = abs = 0`, no
/// [`Tolerances`] argument at all. Nothing on this surface is computed on
/// either side — each float is a class-default constant (`Capacitor.pas:555-557`
/// 0.0005 / 100 / 3, `Line.pas:839-841` 0.1 / 20 / 3), a deck literal that FPC
/// `Val` and Rust `str::parse::<f64>` both round correctly, or an untouched
/// `0.0` — so a difference is a bug, not a floor
/// (`tests/TOLERANCE_NOTES.md`). Four of the fourteen fields (`section_id`,
/// `total_miles`, `lambda`, `accumulated_l`) are fed only by `CalcReliabilityIndices`
/// (`PDElement.pas:89-178`). **G1.6(i) discharged their non-vacuity demo**: the
/// gate now drives the executive `RelCalc` once per `compare_reliability` case,
/// before this comparator runs, so on those cases all four are live accumulated
/// values that are compared against the oracle for real — 3 of 3 PD rows on
/// `modes:time/midi_duty_ctrl.dss` (`both`), 4 of 6 on
/// `modes:makeposseq/makeposseq_ctrl.dss` (`capi_v0145`) and 37 of 41 on
/// `controls:combo/midi_protection.dss` (`r4133`), pinned with their values by
/// `tests/reliability_pins.rs::pd_elements_relcalc_fields_are_live_after_relcalc`.
/// Elsewhere they stay 0 and comparing them asserts the port does not populate
/// them prematurely. They remain in the exact set — the re-derivation the
/// G1.6b section of `tests/TOLERANCE_NOTES.md` owed is discharged there.
///
/// The only cells not compared are [`PD_SKIP_FIELDS`]', which the channel reads
/// out of uninitialized memory; each one is still *visited* and accounted, so a
/// row that stops excluding a divergence fails the gate.
pub fn compare_pd_elements(dss: &Dss, exp: &[PdElementCap], channel: PropsChannel, ctx: &str) {
    let act = dss.pd_elements();
    let tag = channel.tag();
    assert_eq!(
        act.len(),
        exp.len(),
        "{ctx}: PDElements walk length differs against `{tag}`: Rust {} vs oracle {} (Rust head \
         {:?}, oracle head {:?}). The walk is the ENABLED PD-element list in `AddCktElement` \
         creation order — `Fault` is NON_PCPD_ELEM and never in it \
         (`PDElements/Fault.pas:114`), and `PDElements.Count` is the raw ListSize, not this \
         length.",
        act.len(),
        exp.len(),
        act.iter().take(5).map(|p| &p.name).collect::<Vec<_>>(),
        exp.iter().take(5).map(|p| &p.name).collect::<Vec<_>>(),
    );
    if let Some(k) = act
        .iter()
        .zip(exp)
        .position(|(a, e)| !a.name.eq_ignore_ascii_case(&e.name))
    {
        panic!(
            "{ctx}: PDElements walk differs against `{tag}` at index {k}: Rust `{}` vs oracle \
             `{}` (membership or order — both sides are the circuit's PDElements pointer list)",
            act[k].name, exp[k].name,
        );
    }
    for (a, e) in act.iter().zip(exp) {
        let class = pd_class(&e.name);
        // A skip row speaks only about the elements the oracle's zone build
        // wrote through the wrong cursor — see [`pd_skip_applies`].
        let skippable = pd_skip_applies(a);
        let av = pd_fields!(a);
        let ev = pd_fields!(e);
        for ((fa, va), (fe, ve)) in av.iter().zip(ev.iter()) {
            debug_assert_eq!(fa, fe, "both extractions are the one `pd_fields!` macro");
            let equal = va.matches(*ve);
            if let Some(i) = pd_skip_row(tag, class, fa).filter(|_| skippable) {
                PD_SKIP_VISITS[i].fetch_add(1, AtomicOrd::Relaxed);
                if !equal {
                    PD_SKIP_HITS[i].fetch_add(1, AtomicOrd::Relaxed);
                }
                continue;
            }
            assert!(
                equal,
                "{ctx}: PDElements `{}` field `{fa}` differs against `{tag}`: Rust {} vs oracle \
                 {}. This surface is compared EXACTLY (rel = abs = 0): no value on either side \
                 is computed, so a difference is a bug, never a floor. The only excluded cells \
                 are `PD_SKIP_FIELDS`', on an in-zone SHUNT Capacitor/Reactor, which this \
                 element is not (port: is_shunt = {}, in_meter_zone = {}).",
                e.name,
                va.render(),
                ve.render(),
                a.is_shunt,
                a.in_meter_zone,
            );
        }
    }
    let slot = pd_channel_slot(channel);
    PD_WALKS[slot].fetch_add(1, AtomicOrd::Relaxed);
    PD_ELEMENTS[slot].fetch_add(exp.len(), AtomicOrd::Relaxed);
}

/// **Fail-on-nothing-ran for the whole PDElements surface** — the G1.6b half of
/// plan §1.1(f), modeled on `props_norm::assert_r4133_props_compare_ran`.
///
/// 96 of the 372 walked live capi cases hold **no** PD element at all, so
/// [`capture_guard::require_capture`] (count > 0) cannot be the per-case rail
/// here: the runner uses the *presence* form
/// ([`capture_guard::require_capture_opt`]) and lets an empty oracle walk match
/// an empty port walk. That leaves one hole, which this closes — a global
/// collapse to zero (the scheduler stops forcing the flag, a transport stops
/// honoring the request, the port loses every PD element on every deck) would
/// compare `[] == []` everywhere and pass. It fails unless **each** gating
/// channel compared at least one non-empty walk, because a surface wired on one
/// channel only is exactly what the plan's D2 forbids.
///
/// Silent under `DSS_GATE_ONLY` for the reason its neighbours are: a filtered
/// run may legitimately hold no case that gates a given channel.
pub fn assert_pd_elements_compare_ran() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let (cw, ce, rw, re) = pd_walk_counters();
    check_pd_elements_compare_ran((cw, ce), (rw, re));
}

/// The rule itself, over **injected** counters — the split exists for the
/// reason `props_norm::check_r4133_props_compare_ran`'s does: the shipped
/// statics cannot be zeroed once a gate run has moved them, so both directions
/// are pinned offline.
fn check_pd_elements_compare_ran(capi: (usize, usize), r4133: (usize, usize)) {
    assert!(
        capi.1 > 0 && r4133.1 > 0,
        "the PDElements compare never reached one of the two channels: capi_v0145 {} walk(s) / \
         {} element(s), r4133 {} walk(s) / {} element(s). Since GOLDEN_REBASE G1.6b every live \
         non-`large` case compares its full PDElements walk on every channel it gates \
         (`corpus_gate/scheduler.rs::force_pdelements`, pinned by \
         `FORCED_PDELEMENTS_POPULATION`). A zero here means the request was masked off, a \
         transport stopped honoring it, or the port lost every PD element — none of which any \
         manifest flag or `population.lock.json` fingerprint would show, because the per-case \
         rail must tolerate the 96 live cases that legitimately hold no PD element.",
        capi.0,
        capi.1,
        r4133.0,
        r4133.1,
    );
}

/// **Fail-on-stale for [`PD_SKIP_FIELDS`]** — every row must still exclude a
/// real divergence.
///
/// Two arms, each of which is a mask over nothing:
///
/// * `visits == 0` — the row's `(channel, class, field)` never occurred on an
///   in-zone shunt element ([`pd_skip_applies`]) anywhere in the population, so
///   it excludes nothing that exists;
/// * `hits == 0` — it was consulted on real cells and the two sides agreed
///   every time, so the defect it names is gone.
///
/// Silent under `DSS_GATE_ONLY`, and silent when the surface never ran on both
/// channels — that case is [`assert_pd_elements_compare_ran`]'s, invoked first
/// in the gate epilogue, and it deserves one line of diagnosis rather than
/// eight rows of "never visited".
///
/// Honest limit: the excluded value is a heap pointer, so `hits` measures live
/// memory rather than a constant. The Reactor rows are the thin ones (1 cell
/// per capi row, 2 per r4133 row over the whole population), and a run in which
/// those cells happened to read a nil pointer would report them stale. That is
/// still the verdict to surface loudly — re-measure the row before dropping it
/// — and the Capacitor rows (96 / 105 cells) carry the channel-level signal
/// with a wide margin.
pub fn assert_pd_skip_rows_are_live() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let read = |c: &[AtomicUsize]| -> Vec<usize> {
        c.iter().map(|c| c.load(AtomicOrd::Relaxed)).collect()
    };
    let (_, ce, _, re) = pd_walk_counters();
    check_pd_skip_rows_are_live(
        PD_SKIP_FIELDS,
        &read(&PD_SKIP_VISITS),
        &read(&PD_SKIP_HITS),
        ce > 0 && re > 0,
    );
}

/// The staleness rule over **injected** counters (see
/// [`assert_pd_skip_rows_are_live`]); `surface_ran` is false when the walk did
/// not compare anything on some channel, which silences both arms.
fn check_pd_skip_rows_are_live(
    table: &[PdSkipRow],
    visits: &[usize],
    hits: &[usize],
    surface_ran: bool,
) {
    assert_eq!(
        (table.len(), table.len()),
        (visits.len(), hits.len()),
        "the counters are indexed exactly like the table"
    );
    if !surface_ran {
        return;
    }
    let stale: Vec<String> = table
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            if visits[i] == 0 {
                Some(format!(
                    "stale PDElements skip row: {}/{}.{} was never consulted — no `{}`-gating \
                     case in the population holds an enabled {}. Cited: {}",
                    r.channel, r.class, r.field, r.channel, r.class, r.cite
                ))
            } else if hits[i] == 0 {
                Some(format!(
                    "stale PDElements skip row: {}/{}.{} excluded nothing across {} compared \
                     cell(s) — the two sides agreed every time, so the uninitialized read it \
                     names is gone (or this run's heap handed back the port's value). \
                     Re-measure before dropping. Cited: {}, pinned by `{}`",
                    r.channel, r.class, r.field, visits[i], r.cite, r.pin
                ))
            } else {
                None
            }
        })
        .collect();
    assert!(
        stale.is_empty(),
        "{}\n— each row drops a PDElements cell from the oracle compare, so it must name a \
         divergence that is really there (GOLDEN_REBASE_PLAN.md §1.1(e)).",
        stale.join("\n")
    );
}

#[cfg(test)]
mod pd_elements_tests {
    use super::*;

    /// A record whose every field is distinguishable, so an extraction that
    /// mixed two fields up cannot pass by coincidence.
    fn cap() -> PdElementCap {
        PdElementCap {
            name: "Capacitor.C83".to_string(),
            accumulated_l: 1.0,
            from_terminal: 2,
            is_shunt: true,
            num_customers: 3,
            section_id: 4,
            fault_rate: 5.0,
            repair_time: 6.0,
            total_miles: 7.0,
            total_customers: 8,
            pct_permanent: 9.0,
            lambda: 10.0,
            parent_class_index: 11,
            parent_name: "Line.L115".to_string(),
        }
    }

    /// The extraction covers all fourteen keys, in the frozen read order, with
    /// the right type on each. The *port* side of the same macro is checked by
    /// the compiler: `pd_fields!` expands over `PdElementView` too, so a field
    /// renamed there stops `harness/mod.rs` compiling.
    #[test]
    fn the_field_extraction_is_the_fourteen_frozen_keys_in_read_order() {
        let c = cap();
        let got = pd_fields!(&c);
        let names: Vec<&str> = got.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, PD_FIELD_NAMES.to_vec());
        assert_eq!(got[0].1, PdVal::S("Capacitor.C83"));
        assert_eq!(got[1].1, PdVal::F(1.0));
        assert_eq!(got[2].1, PdVal::I(2));
        assert_eq!(got[3].1, PdVal::B(true));
        assert_eq!(got[13].1, PdVal::S("Line.L115"));
        // `parent_class_index` is the LAST oracle field read and `parent_name`
        // the read that follows it — the active-element hijack contract
        // (`CAPI/CAPI_PDElements.pas:245-257`).
        assert_eq!(names[12], "parent_class_index");
        assert_eq!(names[13], "parent_name");
    }

    /// Equality is exact on numbers and case-insensitive on the two name
    /// fields — the port capitalizes the class and lowercases the object name
    /// while the oracles echo the deck.
    #[test]
    fn field_equality_is_exact_on_numbers_and_case_insensitive_on_names() {
        assert!(PdVal::S("Capacitor.C83").matches(PdVal::S("capacitor.c83")));
        assert!(!PdVal::S("Capacitor.C83").matches(PdVal::S("Capacitor.C84")));
        assert!(PdVal::F(0.0005).matches(PdVal::F(0.0005)));
        // One ULP apart must FAIL: there is no tolerance on this surface.
        assert!(!PdVal::F(0.0005).matches(PdVal::F(f64::from_bits(0.0005f64.to_bits() + 1))));
        // The measured UB denormal against the port's correct 0.0.
        assert!(!PdVal::F(0.0).matches(PdVal::F(1.043284808626e-311)));
        assert!(!PdVal::I(1).matches(PdVal::I(0)));
        assert!(!PdVal::B(true).matches(PdVal::B(false)));
        assert!(!PdVal::I(1).matches(PdVal::F(1.0)));
        assert_eq!(
            PdVal::F(1.043284808626e-311).render(),
            "1.043284808626e-311"
        );
    }

    /// The skip lookup is scoped to its own channel, class and field in all
    /// three directions — the asymmetry between the channels is the point:
    /// `fault_rate` stays compared on r4133, `lambda` on capi_v0145.
    #[test]
    fn the_pd_skip_lookup_matches_only_its_own_channel_class_and_field() {
        assert!(pd_skip_row("capi_v0145", "Capacitor", "fault_rate").is_some());
        assert!(pd_skip_row("capi_v0145", "capacitor", "fault_rate").is_some());
        assert!(pd_skip_row("r4133", "Capacitor", "fault_rate").is_none());
        assert!(pd_skip_row("r4133", "Capacitor", "lambda").is_some());
        assert!(pd_skip_row("capi_v0145", "Capacitor", "lambda").is_none());
        assert!(pd_skip_row("capi_v0145", "Line", "fault_rate").is_none());
        assert!(pd_skip_row("capi_v0145", "Transformer", "pct_permanent").is_none());
        assert!(pd_skip_row("capi_v0145", "Capacitor", "repair_time").is_none());
        assert_eq!(pd_class("Capacitor.c83"), "Capacitor");
        assert_eq!(pd_class("nodot"), "nodot");
    }

    /// **The register**: the eight rows, spelled out a second time, so neither
    /// adding nor dropping one can happen without an edit that says so. Every
    /// row must also name a real field, one of the two channel tags, a
    /// non-empty citation and its pin.
    #[test]
    fn the_pd_skip_register_is_exactly_the_measured_rows() {
        let got: Vec<String> = PD_SKIP_FIELDS
            .iter()
            .map(|r| format!("{}/{}.{} -> {}", r.channel, r.class, r.field, r.pin))
            .collect();
        let capi_pin = "pd_elements_shunt_reliability_inputs_survive_the_meter_zone";
        let r4133_pin = "pd_elements_shunt_branch_flt_rate_survives_the_meter_zone";
        let expected = vec![
            format!("capi_v0145/Capacitor.fault_rate -> {capi_pin}"),
            format!("capi_v0145/Capacitor.pct_permanent -> {capi_pin}"),
            format!("capi_v0145/Reactor.fault_rate -> {capi_pin}"),
            format!("capi_v0145/Reactor.pct_permanent -> {capi_pin}"),
            format!("r4133/Capacitor.lambda -> {r4133_pin}"),
            format!("r4133/Capacitor.accumulated_l -> {r4133_pin}"),
            format!("r4133/Reactor.lambda -> {r4133_pin}"),
            format!("r4133/Reactor.accumulated_l -> {r4133_pin}"),
        ];
        assert_eq!(
            got, expected,
            "PD_SKIP_FIELDS moved. Every row drops an oracle cell from the compare, so a row may \
             only be added with its measurement + pin, and only be dropped once the \
             uninitialized read is gone upstream (GOLDEN_REBASE_PLAN.md §1.1(e))."
        );
        for r in PD_SKIP_FIELDS {
            assert!(
                PD_FIELD_NAMES.contains(&r.field),
                "{}/{}.{} names a field the record does not have",
                r.channel,
                r.class,
                r.field
            );
            assert!(
                [PropsChannel::CapiV0145.tag(), PropsChannel::R4133.tag()].contains(&r.channel),
                "{} is not a gating channel tag",
                r.channel
            );
            assert!(!r.cite.is_empty() && !r.pin.is_empty());
        }
        // No row may be listed twice — a duplicate would split the liveness
        // accounting and hide a stale twin behind its live neighbour.
        let mut keys: Vec<(&str, &str, &str)> = PD_SKIP_FIELDS
            .iter()
            .map(|r| (r.channel, r.class, r.field))
            .collect();
        keys.sort_unstable();
        let n = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), n, "PD_SKIP_FIELDS holds a duplicate row");
    }

    /// A port-side walk row, shaped by the two halves of the skip scope.
    fn view(is_shunt: bool, in_meter_zone: bool) -> PdElementView {
        PdElementView {
            name: "Capacitor.c83".to_string(),
            accumulated_l: 0.0,
            from_terminal: 1,
            is_shunt,
            num_customers: 0,
            section_id: 0,
            fault_rate: 0.0005,
            repair_time: 3.0,
            total_miles: 0.0,
            total_customers: 0,
            pct_permanent: 100.0,
            lambda: 0.0,
            parent_class_index: 0,
            parent_name: String::new(),
            in_meter_zone,
        }
    }

    /// **The scope**: a row applies only where the oracle's zone build wrote
    /// through the wrong cursor — an in-zone SHUNT Capacitor/Reactor. The three
    /// other combinations are compared on every field, which is what keeps the
    /// ~350 clean cases per channel under live comparison.
    #[test]
    fn the_pd_skip_scope_is_the_in_zone_shunt_element() {
        assert!(pd_skip_applies(&view(true, true)));
        assert!(!pd_skip_applies(&view(true, false)), "no meter reached it");
        assert!(
            !pd_skip_applies(&view(false, true)),
            "series: on the PD list"
        );
        assert!(!pd_skip_applies(&view(false, false)));
    }

    /// **Every row's `pin` is a test that exists** — the
    /// `props_r4133_replay::every_echo_row_pin_is_a_test_that_exists` idiom.
    ///
    /// [`the_pd_skip_register_is_exactly_the_measured_rows`] pins the pin
    /// *names*; without this guard renaming or deleting either pin would leave
    /// the whole gate green while eight oracle cells kept being dropped with no
    /// expected-value test behind them — exactly the "mask over nothing" the
    /// rows exist to prevent (GOLDEN_REBASE_PLAN.md §1.1(e), CLAUDE.md).
    #[test]
    fn every_pd_skip_row_pin_is_a_test_that_exists() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PD_PINS_FILE);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
            // Line endings are the checkout's, not this test's business.
            .replace("\r\n", "\n");
        let mut named: Vec<&str> = PD_SKIP_FIELDS.iter().map(|r| r.pin).collect();
        named.sort_unstable();
        named.dedup();
        assert_eq!(named.len(), 2, "the eight rows are pinned by two tests");
        for pin in named {
            assert!(
                text.contains(&format!("#[test]\nfn {pin}() {{")),
                "PD_SKIP_FIELDS names `{pin}` as its pin, but crates/dss-core/{PD_PINS_FILE} \
                 defines no such #[test]. An exclusion without its expected-value test is a \
                 mask over nothing."
            );
        }
    }

    /// The global guard fires unless BOTH channels walked something — a surface
    /// wired on one channel only is a failure, not a half-success.
    #[test]
    fn the_global_pd_guard_fires_unless_both_channels_walked() {
        check_pd_elements_compare_ran((12, 340), (9, 271));
        for (capi, r4133) in [
            ((0, 0), (9, 271)),
            ((12, 340), (0, 0)),
            ((0, 0), (0, 0)),
            ((12, 0), (9, 271)),
        ] {
            let payload =
                std::panic::catch_unwind(move || check_pd_elements_compare_ran(capi, r4133))
                    .expect_err("the guard must fire");
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "<non-string panic>".to_string());
            assert!(msg.contains("force_pdelements"), "{msg}");
        }
    }

    /// The row-liveness guard: both arms fire, and both silences hold.
    #[test]
    fn the_pd_skip_liveness_guard_fires_on_an_unvisited_or_never_hitting_row() {
        let row = |field| PdSkipRow {
            channel: "r4133",
            class: "Capacitor",
            field,
            pin: "pd_elements_shunt_branch_flt_rate_survives_the_meter_zone",
            cite: "Version8/Source/Meters/EnergyMeter.pas:1868-1869",
        };
        let table: &[PdSkipRow] = &[row("lambda"), row("accumulated_l")];
        // Live: both rows excluded something.
        check_pd_skip_rows_are_live(table, &[105, 105], &[105, 105], true);
        // Silent when the surface never ran (the global guard's business).
        check_pd_skip_rows_are_live(table, &[0, 0], &[0, 0], false);
        let fire = |visits: [usize; 2], hits: [usize; 2]| -> String {
            let payload = std::panic::catch_unwind(move || {
                check_pd_skip_rows_are_live(table, &visits, &hits, true)
            })
            .expect_err("the guard must fire");
            payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "<non-string panic>".to_string())
        };
        let never_visited = fire([0, 105], [0, 105]);
        assert!(never_visited.contains("never consulted"), "{never_visited}");
        assert!(
            !never_visited.contains("accumulated_l"),
            "only the stale row is reported: {never_visited}"
        );
        let never_hit = fire([105, 105], [105, 0]);
        assert!(never_hit.contains("excluded nothing"), "{never_hit}");
        assert!(never_hit.contains("accumulated_l"), "{never_hit}");
    }
}

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
/// The four divergent bus quantities (`SeqVoltages`/`CplxSeqVoltages`,
/// `VLL`/`puVLL`, G1.4c) are the LAST block, also `#[serde(default)]`. They
/// follow neither convention: the sequence pair is indexed by symmetrical
/// component and the line-to-line pair by the pairs UPSTREAM's own walk
/// produced, which is not always the port's (see [`compare_bus_seq_and_vll`]).
#[derive(Debug, Deserialize)]
pub struct BusCap {
    /// `Bus.Name`, in `Circuit.AllBusNames` (= `BusList`) order.
    pub name: String,
    /// `Bus.kVBase` in kV; `<= 0` = not set.
    pub kv_base: f64,
    /// `Bus.Distance` — `TDSSBus.DistFromMeter` in km, the EnergyMeter zone
    /// walk's own accumulator published verbatim (capi `CAPI/CAPI_Bus.pas:419-427`
    /// -> `CAPI/CAPI_Alt.pas:2071-2074` == r4133 `DDLL/DBus.pas:122-128`,
    /// `BUSF` 5). G1.4b — compared by [`compare_bus_distances`], not by
    /// [`compare_bus`], which owns the voltage half of the same walk.
    ///
    /// Deliberately **required** (no `#[serde(default)]`, unlike every optional
    /// field below): both transports ship it unconditionally behind the
    /// `compare_bus` flag, so a transport that stopped emitting it must fail
    /// deserialization instead of silently deserializing as `0.0` — the value a
    /// meterless circuit legitimately reports, which would leave the whole
    /// surface green over nothing.
    pub distance: f64,
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
    /// `Bus.SeqVoltages` — `|V012|`, THREE doubles on both channels, always:
    /// the n/A reply is three `-1.0`s, not a short array
    /// (`CAPI_Alt.pas:2165-2200` == `DBus.pas:286-317`). G1.4c.
    #[serde(default)]
    pub seq_voltages: Vec<f64>,
    /// `Bus.CplxSeqVoltages` — the same `V012`, complex: SIX doubles always,
    /// the n/A reply six `-1.0`s (`CAPI_Alt.pas:2367-2398` == `DBus.pas:520-547`,
    /// where `cmplx(-1,-1) x 3` is those six doubles).
    #[serde(default)]
    pub cplx_seq_voltages: Vec<f64>,
    /// `Bus.VLL` — `2 * pairs` doubles, where the pairs are the ones upstream's
    /// own walk produced; or the 1-phase sentinel `[-99999.0, 0.0]`, or capi's
    /// one-double `DefaultResult` (`CAPI_Alt.pas:2473-2537` == `DBus.pas:549-601`).
    /// EMPTY when the r4133 bridge refused the call — see [`Self::vll_declined`].
    #[serde(default)]
    pub vll: Vec<f64>,
    /// `Bus.puVLL` — [`Self::vll`] over `1000 * kVBase * sqrt3`, same shape rule
    /// (`CAPI_Alt.pas:2400-2470` == `DBus.pas:603-658`).
    #[serde(default)]
    pub pu_vll: Vec<f64>,
    /// The **r4133 bridge** refused to call `BUSV(11)`/`BUSV(12)` on this bus
    /// because `DBus.pas:580-584`'s unbounded partner `repeat` would not
    /// terminate (`dss-epri::modes::bus_vll_would_hang`, the state-dependent
    /// refusal register). Always `false` on the capi channel, whose partner scan
    /// is the bounded `for k := 1 to 3` (`CAPI_Alt.pas:2512-2524`) — asserted by
    /// [`compare_bus_seq_and_vll`].
    #[serde(default)]
    pub vll_declined: bool,
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
    let d = wrapped_deg(actual_deg - expected_deg);
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
// GOLDEN_REBASE_PLAN.md WP-G1 sub-step G1.4b: the bus DISTANCE surface.
//
// `Bus.Distance`, `Circuit.AllBusDistances` and `Circuit.AllNodeDistances` —
// three views of the ONE field `TDSSBus.DistFromMeter` (capi
// `CAPI/CAPI_Bus.pas:419-427` -> `CAPI/CAPI_Alt.pas:2071-2074`,
// `CAPI/CAPI_Circuit.pas:671-688` and `:697-722`; r4133 `DDLL/DBus.pas:122-128`
// `BUSF` 5, `DDLL/DCircuit.pas:566-580` `CircuitV` 12 and `:582-604`
// `CircuitV` 13; fastdss dumps all three with the rest of the `IBus`/`ICircuit`
// `_columns`, `dss/IBus.py:28` and `dss/ICircuit.py:106`/`:113` on
// `origin/fastdss`). They ride the `compare_bus` flag and the ONE per-bus
// `SetActiveBus` walk `compare_bus` already pays for — no second flag, no
// second capture.
//
// **This surface is not a solve output.** `DistFromMeter` is written only by
// the EnergyMeter zone build (r4133 `Meters/EnergyMeter.pas:1833-1838` == port
// `solution/meters/zones/build.rs:240-251`, which adds
// `len · ConvertLineUnits(units, UNITS_KM)` per *line* branch and carries the
// parent's value across every other branch) and reset to `0.0` at the zone
// origin (`build.rs:178`). It is therefore NOT an image of `Solution.NodeV`,
// so `compare_bus`'s `voltages_excluded` rule does not reach it: a `voltages`
// ledger cause cannot explain a distance divergence, and suppressing it there
// would hide a real zone-build bug behind an unrelated triage.
//
// **Tolerance: none — `rel = abs = 0`.** Derivation and the measurement that
// backs it: tests/TOLERANCE_NOTES.md §"Bus distance surface (GOLDEN_REBASE
// G1.4b)". A measured upstream divergence is therefore never absorbed by a
// band: it is excluded bus by bus through the `distance` ledger field
// (coordinator decision D29 step 3) and pinned by an expected-value test —
// `the_reduced_midi_deck_reports_the_merged_lines_kft_distances` in
// `corpus_gate.rs` for the one case that has one.
// ---------------------------------------------------------------------------

/// Gating distance compares that carried at least one NON-ZERO distance — one
/// per [`compare_bus_distances`] call made from the corpus gate's runner.
static DISTANCE_WALKS: AtomicUsize = AtomicUsize::new(0);
/// …and the buses those walks compared a non-zero distance on.
static DISTANCE_BUSES: AtomicUsize = AtomicUsize::new(0);

/// What a full-population gate run must reach: `(walks, buses)` over which a
/// non-zero `DistFromMeter` was compared, re-derived on every run and asserted
/// EXACTLY, so the number fails on a drop **and** on a growth.
///
/// It is this surface's fail-on-stale, and it exists because the comparator is
/// an equality over a field that is `0.0` on the ~370 forced cases with no
/// EnergyMeter (a static scan following `Redirect`/`Compile` finds one in
/// **70** of the 443 — `TESTING.md`): on those the whole surface is the
/// (real, but trivial) shape assertion "the port invents no distance", and a
/// regression that stopped the zone walk from writing distances at all would
/// leave every one of them green.
/// `population.lock.json` cannot see it either — it fingerprints the manifest
/// FLAG (this surface sets none of its own, it rides `compare_bus`) and
/// `MODES_REQUIRED` the deck PATH, never that a deck still builds a meter zone.
///
/// Read off a COMPLETED full default-lane gate on 2026-09-05 (526/526 cases
/// passed, 223.6 s — the run's own `corpus_gate distance:` line), once micro-part
/// F2's one measured divergence was settled by the `distance` ledger exclusion
/// (coordinator decision D29 step 3). Never guessed: 867 gating compares over
/// both channels and every gated step carried at least one non-zero
/// `DistFromMeter`, and 79 137 individual buses did — the 70 metered cases
/// times their channels, steps and bus counts.
const DISTANCE_POPULATION: (usize, usize) = (867, 79_137);

/// Record one gating distance compare that saw `nonzero_buses` non-zero
/// distances.
///
/// The single caller is `corpus_gate/runner.rs`'s `compare_bus` block. The
/// harness' own unit drives ([`bus_distance_comparator_tests`]) call
/// [`compare_bus_distances`] directly and are deliberately NOT counted, for the
/// reason [`record_sc_study_compare`] gives: they run in this same test binary
/// and would make the population unstable.
pub fn record_distance_compare(nonzero_buses: usize) {
    if nonzero_buses > 0 {
        DISTANCE_WALKS.fetch_add(1, AtomicOrd::Relaxed);
        DISTANCE_BUSES.fetch_add(nonzero_buses, AtomicOrd::Relaxed);
    }
}

/// What [`record_distance_compare`] has counted in this process, as
/// `(walks, buses)`.
pub fn distance_counters() -> (usize, usize) {
    (
        DISTANCE_WALKS.load(AtomicOrd::Relaxed),
        DISTANCE_BUSES.load(AtomicOrd::Relaxed),
    )
}

/// **Fail-on-stale for the distance surface**, called once from the corpus
/// gate's epilogue. Silent under `DSS_GATE_ONLY` — a filtered run legitimately
/// holds no metered deck — exactly like its neighbour
/// [`assert_sc_study_compare_ran`].
pub fn assert_distance_compare_ran() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let (walks, buses) = distance_counters();
    check_distance_compare_ran(walks, buses);
}

/// The rule itself, over **injected** counters — split off for the reason
/// [`check_sc_study_compare_ran`]'s twin is: the shipped statics cannot be
/// rewound once the gate has moved them, so both directions are pinned offline
/// in `corpus_gate.rs`.
pub fn check_distance_compare_ran(walks: usize, buses: usize) {
    assert_eq!(
        (walks, buses),
        DISTANCE_POPULATION,
        "the distance surface compared {walks} walk(s) / {buses} bus(es) carrying a \
         non-zero DistFromMeter, not {DISTANCE_POPULATION:?}. Fewer means a deck stopped \
         building a meter zone (or lost a step, or a channel) — which the comparator's \
         own equality survives, because both sides then report the meterless 0.0; more \
         means a new metered case arrived. Re-derive the pair from the run's own \
         `corpus_gate distance:` line and move it deliberately (GOLDEN_REBASE G1.4b)."
    );
}

/// Compare the three `DistFromMeter` views against the engine's, in `BusList`
/// order (GOLDEN_REBASE_PLAN.md G1.4b). Returns the number of buses whose
/// distance was non-zero, for [`record_distance_compare`].
///
/// Runs AFTER [`compare_bus`], which pins the bus count and the name sequence
/// first; the name is re-checked here per bus so the function is honest when
/// called alone, as the offline drives in [`bus_distance_comparator_tests`] do.
///
/// Structure, strongest first:
/// * every LENGTH — the oracle's two arrays against the port's, and both
///   against the port's own per-bus walk (`Σ nodes`). This is what makes the
///   two circuit-level walks and the per-bus walk unable to drift apart
///   silently;
/// * the port-internal identity `all_bus_distances[i] == buses[i].distance` and
///   `all_node_distances[k] == that same value` for every node of bus `i` — the
///   run-length structure is the only ordering information the node array
///   carries (its values are constant across a bus), so it is asserted
///   explicitly rather than inferred;
/// * the ORACLE-internal form of the same two identities. Both oracles publish
///   the identical field through three entry points, so this pins that the
///   oracle's `AllBusDistances` really is in `BusList` order — the assumption
///   the per-bus comparison rests on;
/// * the values themselves, **exactly** (`rel = abs = 0`): the two oracles are
///   bit-identical to each other and to the port on this quantity, because all
///   three run the same zone walk over the same `len · ConvertLineUnits` sum
///   and never touch the solver. See tests/TOLERANCE_NOTES.md;
/// * "the port invents no distance": a non-zero distance requires the circuit
///   to hold an EnergyMeter object. The converse is deliberately NOT asserted —
///   a meter whose zone is a single bus, or whose zone was never built,
///   legitimately leaves every distance at `0.0`.
///
/// `excluded(bus_name)` is the `distance` ledger field
/// (`GOLDEN_REBASE_PLAN.md` G1.4b, coordinator decision D29 step 3), and it is
/// consulted **only after the exact equality has already failed** on that bus.
/// That order is the whole point: `LedgerView::excluded` sets the scope's `hit`
/// flag when it answers, so asking it lazily makes a hit mean *masked a real
/// divergence* rather than *matched a name*, and a scope whose upstream cause
/// got fixed goes unhit and fails the gate as STALE
/// (`corpus_gate::ledger::PER_VALUE_EXCLUSION_FIELDS`). Nothing else is
/// suppressed: an excluded bus keeps its name check, both internal identities
/// and every length, and it still counts toward the run-wide population.
pub fn compare_bus_distances(
    dss: &Dss,
    exp: &[BusCap],
    exp_all_bus: &[f64],
    exp_all_node: &[f64],
    excluded: &dyn Fn(&str) -> bool,
    ctx: &str,
) -> usize {
    let views = dss.all_bus_voltages();
    let all_bus = dss.all_bus_distances();
    let all_node = dss.all_node_distances();
    assert_eq!(
        views.len(),
        exp.len(),
        "{ctx}: bus count differs ({} vs {})",
        views.len(),
        exp.len()
    );
    let nodes_total: usize = views.iter().map(|v| v.nodes.len()).sum();
    for (what, port, oracle, want) in [
        (
            "AllBusDistances",
            all_bus.len(),
            exp_all_bus.len(),
            views.len(),
        ),
        (
            "AllNodeDistances",
            all_node.len(),
            exp_all_node.len(),
            nodes_total,
        ),
    ] {
        assert_eq!(
            port, want,
            "{ctx}: the port's {what} has {port} entries but its own bus-list walk saw {want}"
        );
        assert_eq!(
            oracle, want,
            "{ctx}: the oracle's {what} has {oracle} entries but the bus-list walk saw {want}"
        );
    }

    let mut nonzero = 0usize;
    let mut k = 0usize;
    for (i, (v, e)) in views.iter().zip(exp).enumerate() {
        assert!(
            v.name.eq_ignore_ascii_case(&e.name),
            "{ctx}: bus {i} name differs: {} vs {}",
            v.name,
            e.name
        );
        if v.distance != e.distance {
            assert!(
                excluded(&e.name),
                "{ctx}: bus {} Distance differs: {} vs {} km (compared exactly — the zone \
                 walk is the same sum on both engines and never touches the solver). \
                 No `distance` ledger exclusion covers this bus on this \
                 channel",
                e.name,
                v.distance,
                e.distance
            );
        }
        assert_eq!(
            all_bus[i], v.distance,
            "{ctx}: bus {} — the port's AllBusDistances[{i}] ({}) is not its own \
             Bus.Distance ({}); the two accessors publish one field",
            e.name, all_bus[i], v.distance
        );
        assert_eq!(
            exp_all_bus[i], e.distance,
            "{ctx}: bus {} — the oracle's AllBusDistances[{i}] ({}) is not its own \
             Bus.Distance ({}), so its array is not in BusList order",
            e.name, exp_all_bus[i], e.distance
        );
        for node in &v.nodes {
            assert_eq!(
                all_node[k], v.distance,
                "{ctx}: bus {} node {node} — the port's AllNodeDistances[{k}] ({}) is not \
                 the bus's own distance ({})",
                e.name, all_node[k], v.distance
            );
            assert_eq!(
                exp_all_node[k], e.distance,
                "{ctx}: bus {} node {node} — the oracle's AllNodeDistances[{k}] ({}) is not \
                 the bus's own distance ({}); the node array is the bus array expanded by \
                 NumNodesThisBus",
                e.name, exp_all_node[k], e.distance
            );
            k += 1;
        }
        if v.distance != 0.0 {
            nonzero += 1;
        }
    }
    if nonzero > 0 {
        assert!(
            dss.circuit().is_some_and(|c| !c.energy_meters.is_empty()),
            "{ctx}: {nonzero} bus(es) report a non-zero distance from a circuit that holds \
             no EnergyMeter — `DistFromMeter` is written only by the zone build \
             (`solution/meters/zones/build.rs:240-251`), so the port invented one"
        );
    }
    nonzero
}

/// Committed offline drives for [`compare_bus_distances`] — the negative
/// controls §1.1(f) asks every G1 comparator for, in the shape its three
/// per-bus siblings use ([`bus_comparator_tests`], [`bus_short_circuit_tests`],
/// [`bus_seq_vll_comparator_tests`]).
///
/// G1.4b shipped the surface with the plan-sanctioned *scratch* corruption only
/// (a corrupted `exec/view.rs`, run once and reverted), so nothing in the tree
/// held the comparator's failing direction, and its own doc claimed drives that
/// did not exist (G1.4b audit AC-1 / AT-1, settled 2026-09-06). These are those
/// drives: they perturb the ORACLE side of a live port capture, so no product
/// code is touched, and each rail is pinned by its own panic message.
///
/// The one assertion no offline drive can reach is the "port invents no
/// distance" guard — it fires on the PORT's own state (a non-zero distance from
/// a circuit that holds no EnergyMeter), unreachable without corrupting the
/// engine; its evidence stays the scratch drive logged in the G1.4b handoff.
#[cfg(test)]
mod bus_distance_comparator_tests {
    use super::{BusCap, compare_bus_distances};
    use dss_core::exec::Dss;

    /// The metered radial of `exec/view.rs::bus_distance_tests` (G1.4b F1):
    /// lengths in km, so the zone walk's `Σ len · ConvertLineUnits` is exactly
    /// `sourcebus = 0`, `b1 = 1`, `b2 = 1 + 2 = 3`, and the spur `b3` sits
    /// outside the meter's zone at `0`. Two non-zero buses — enough for a nudge
    /// to be a real divergence and for the return value to be non-trivial.
    fn metered() -> Dss {
        let mut dss = Dss::new();
        for c in [
            "clear",
            "New circuit.dist basekv=12.47 pu=1.0 phases=3 bus1=sourcebus",
            "New Line.l1 bus1=sourcebus bus2=b1 phases=3 r1=0.1 x1=0.3 c1=0 length=1 units=km",
            "New Line.l2 bus1=b1 bus2=b2 phases=3 r1=0.1 x1=0.3 c1=0 length=2 units=km",
            "New Line.spur bus1=sourcebus bus2=b3 phases=3 r1=0.1 x1=0.3 c1=0 length=5 units=km",
            "New Load.ld bus1=b2 phases=3 kv=12.47 kw=500 pf=0.95",
            "New EnergyMeter.em element=Line.l1 terminal=1",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve",
        ] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    /// The capture both transports send for this deck, built from the port's
    /// own view — the positive control every drive below perturbs. Only the
    /// fields [`compare_bus_distances`] reads are filled (`name`, `distance`,
    /// and `kv_base`/`nodes` for the shape); the voltage and short-circuit
    /// arrays belong to the other comparators' drives.
    fn capture(dss: &Dss) -> (Vec<BusCap>, Vec<f64>, Vec<f64>) {
        let caps = dss
            .all_bus_voltages()
            .iter()
            .map(|v| {
                let mut nodes = v.nodes.clone();
                nodes.sort_unstable();
                BusCap {
                    name: v.name.clone(),
                    kv_base: v.kv_base,
                    distance: v.distance,
                    nodes,
                    pu_voltages: Vec::new(),
                    vmag_angle: Vec::new(),
                    pu_vmag_angle: Vec::new(),
                    zsc1: Vec::new(),
                    zsc0: Vec::new(),
                    zsc: Vec::new(),
                    ysc: Vec::new(),
                    isc: Vec::new(),
                    voc: Vec::new(),
                    seq_voltages: Vec::new(),
                    cplx_seq_voltages: Vec::new(),
                    vll: Vec::new(),
                    pu_vll: Vec::new(),
                    vll_declined: false,
                }
            })
            .collect();
        (caps, dss.all_bus_distances(), dss.all_node_distances())
    }

    /// The ledger closure of a case with no `distance` entry: nothing excluded.
    fn nothing(_: &str) -> bool {
        false
    }

    /// Positive control: the comparator accepts the engine's own surface, the
    /// fixture really carries the two non-zero distances the drives perturb,
    /// and the returned count is the one `record_distance_compare` receives.
    #[test]
    fn compare_bus_distances_accepts_the_engines_own_surface() {
        let dss = metered();
        let (exp, all_bus, all_node) = capture(&dss);
        assert_eq!(
            all_bus,
            vec![0.0, 1.0, 3.0, 0.0],
            "the drives below are vacuous unless the zone walk really wrote km"
        );
        let nonzero = compare_bus_distances(
            &dss,
            &exp,
            &all_bus,
            &all_node,
            &nothing,
            "positive control",
        );
        assert_eq!(nonzero, 2, "b1 and b2 carry a non-zero DistFromMeter");
        assert_eq!(all_node.len(), 12, "4 three-phase buses × 3 nodes");
    }

    /// A wrong oracle distance on ONE bus reds — the surface's whole point, and
    /// the compare is `rel = abs = 0`, so a 1-ULP nudge is already a divergence.
    #[test]
    #[should_panic(expected = "bus b1 Distance differs")]
    fn a_nudged_oracle_distance_reds() {
        let dss = metered();
        let (mut exp, all_bus, all_node) = capture(&dss);
        exp[1].distance = f64::from_bits(exp[1].distance.to_bits() + 1);
        compare_bus_distances(&dss, &exp, &all_bus, &all_node, &nothing, "nudge");
    }

    /// …and the `distance` ledger field covers exactly the bus it names: the
    /// divergence on `b1` is masked, and the closure is consulted ONLY for the
    /// bus that actually diverged (which is what makes a ledger hit mean
    /// "masked a real divergence" rather than "matched a name").
    #[test]
    fn a_distance_exclusion_covers_only_the_bus_it_names() {
        let dss = metered();
        let (mut exp, all_bus, mut all_node) = capture(&dss);
        exp[1].distance = 42.0;
        // The oracle's own two identities must still hold on the excluded bus.
        let mut oracle_all_bus = all_bus.clone();
        oracle_all_bus[1] = 42.0;
        all_node[3] = 42.0;
        all_node[4] = 42.0;
        all_node[5] = 42.0;
        let asked = std::cell::RefCell::new(Vec::<String>::new());
        let only_b1 = |name: &str| {
            asked.borrow_mut().push(name.to_string());
            name.eq_ignore_ascii_case("b1")
        };
        compare_bus_distances(
            &dss,
            &exp,
            &oracle_all_bus,
            &all_node,
            &only_b1,
            "exclusion",
        );
        assert_eq!(
            asked.into_inner(),
            vec!["b1".to_string()],
            "the ledger is consulted lazily, only for a bus that already failed the equality"
        );
    }

    /// An exclusion that names another bus does NOT cover this one — the scope
    /// is per bus name, never per case.
    #[test]
    #[should_panic(expected = "bus b1 Distance differs")]
    fn a_distance_exclusion_naming_another_bus_still_reds() {
        let dss = metered();
        let (mut exp, all_bus, all_node) = capture(&dss);
        exp[1].distance = 42.0;
        let only_b2 = |name: &str| name.eq_ignore_ascii_case("b2");
        compare_bus_distances(&dss, &exp, &all_bus, &all_node, &only_b2, "wrong scope");
    }

    /// The ORACLE-internal identity `AllBusDistances[i] == Bus.Distance` — what
    /// pins that the oracle's array really is in `BusList` order. An exclusion
    /// must not silence it: an entry masks a VALUE, never the oracle's shape.
    #[test]
    #[should_panic(expected = "is not in BusList order")]
    fn an_oracle_all_bus_distances_out_of_bus_list_order_reds() {
        let dss = metered();
        let (exp, all_bus, all_node) = capture(&dss);
        let mut oracle_all_bus = all_bus.clone();
        oracle_all_bus.swap(1, 2);
        let everything = |_: &str| true;
        compare_bus_distances(
            &dss,
            &exp,
            &oracle_all_bus,
            &all_node,
            &everything,
            "shuffled",
        );
    }

    /// …and the same identity on the node array, whose run lengths are the only
    /// ordering information it carries.
    #[test]
    #[should_panic(expected = "node array is the bus array expanded")]
    fn a_nudged_oracle_node_entry_reds() {
        let dss = metered();
        let (exp, all_bus, mut all_node) = capture(&dss);
        all_node[4] = 99.0;
        compare_bus_distances(&dss, &exp, &all_bus, &all_node, &nothing, "node nudge");
    }

    /// The length rails: a transport that shipped a short node array — or, via
    /// `#[serde(default)]`, none at all — fails instead of being compared over
    /// the prefix. Drive C of the G1.4b handoff, made permanent.
    #[test]
    #[should_panic(expected = "the oracle's AllNodeDistances has 11 entries")]
    fn a_short_oracle_node_array_reds() {
        let dss = metered();
        let (exp, all_bus, mut all_node) = capture(&dss);
        all_node.pop();
        compare_bus_distances(
            &dss,
            &exp,
            &all_bus,
            &all_node,
            &nothing,
            "short node array",
        );
    }

    /// …and the same for the bus-level array.
    #[test]
    #[should_panic(expected = "the oracle's AllBusDistances has 0 entries")]
    fn an_empty_oracle_bus_array_reds() {
        let dss = metered();
        let (exp, _all_bus, all_node) = capture(&dss);
        compare_bus_distances(&dss, &exp, &[], &all_node, &nothing, "empty bus array");
    }

    /// The per-bus name check — what makes the index-for-index pairing honest
    /// even when this comparator is called alone.
    #[test]
    #[should_panic(expected = "bus 1 name differs")]
    fn a_renamed_bus_reds() {
        let dss = metered();
        let (mut exp, all_bus, all_node) = capture(&dss);
        exp[1].name = "b9".to_string();
        compare_bus_distances(&dss, &exp, &all_bus, &all_node, &nothing, "renamed");
    }

    /// A capture with a bus missing fails on the count before anything is
    /// compared — the pairing is never silently truncated.
    #[test]
    #[should_panic(expected = "bus count differs")]
    fn a_short_capture_reds() {
        let dss = metered();
        let (mut exp, all_bus, all_node) = capture(&dss);
        exp.pop();
        compare_bus_distances(&dss, &exp, &all_bus, &all_node, &nothing, "short capture");
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
/// a deliberate act (2026-09-05, lane `lane-b`, 525/525 green in both lanes;
/// re-measured unchanged at the merge into `update`, 526/526).
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
        wrapped_deg,
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
                    distance: v.distance,
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
                    // …and no G1.4c sequence / line-to-line arrays: those are
                    // `compare_bus_seq_and_vll`'s rows, tested in their own
                    // module, and `#[serde(default)]` keeps a G1.4a payload
                    // readable.
                    seq_voltages: Vec::new(),
                    cplx_seq_voltages: Vec::new(),
                    vll: Vec::new(),
                    pu_vll: Vec::new(),
                    vll_declined: false,
                }
            })
            .collect()
    }

    /// [`wrapped_deg`] folds the ±180° seam (a phasor whose angle the two engines
    /// report as +179.999… and −179.999… is 2e-3° apart, not 360°), and
    /// [`bus_angle_band_deg`] is the exact arcsine image of the magnitude band,
    /// saturating at 180° once the band swallows the magnitude.
    #[test]
    fn the_bus_angle_band_wraps_the_seam_and_images_the_magnitude_band() {
        assert_eq!(wrapped_deg(0.0), 0.0);
        assert_eq!(wrapped_deg(180.0), 180.0);
        assert_eq!(wrapped_deg(-180.0), 180.0);
        // The seam: +179.999 vs -179.999 is 0.002 deg apart.
        let d = wrapped_deg(179.999 - (-179.999));
        assert!((d.abs() - 0.002).abs() < 1e-9, "{d}");
        // …and it stays exact through a full turn.
        assert_eq!(wrapped_deg(360.0), 0.0);

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
                    // Exactly compared and not part of this drive: copied.
                    distance: e.distance,
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
                    // …and no G1.4c sequence / line-to-line arrays: those are
                    // `compare_bus_seq_and_vll`'s rows, tested in their own
                    // module, and `#[serde(default)]` keeps a G1.4a payload
                    // readable.
                    seq_voltages: Vec::new(),
                    cplx_seq_voltages: Vec::new(),
                    vll: Vec::new(),
                    pu_vll: Vec::new(),
                    vll_declined: false,
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
    /// angle a full turn away is accepted (`wrapped_deg`), while the 1e-3° drives
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
                    distance: 0.0,
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
                    seq_voltages: Vec::new(),
                    cplx_seq_voltages: Vec::new(),
                    vll: Vec::new(),
                    pu_vll: Vec::new(),
                    vll_declined: false,
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
                        distance: 0.0,
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
                        seq_voltages: Vec::new(),
                        cplx_seq_voltages: Vec::new(),
                        vll: Vec::new(),
                        pu_vll: Vec::new(),
                        vll_declined: false,
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
// GOLDEN_REBASE_PLAN.md WP-G1 sub-step G1.4c: the per-bus SEQUENCE and
// LINE-TO-LINE voltage surface.
//
// `Bus.SeqVoltages`, `Bus.CplxSeqVoltages`, `Bus.VLL` and `Bus.puVLL` — the
// four `IBus._columns` entries G1.4a deliberately left out because the two
// oracles do NOT compute them alike, and neither computes them correctly.
// Captured by the same per-bus walk `compare_bus` and
// `compare_bus_short_circuit` ride (`tools/oracle/oracle_server.py`'s and
// `dss-epri`'s `capture_all_buses`, four reads inserted after the five voltage
// ones and before the six short-circuit ones) and read back from the engine
// through `Dss::all_bus_voltages` (`crates/dss-core/src/exec/view.rs`).
//
// **Three engines, three answers.** For a bus whose nodes are exactly 1, 2, 3
// all three agree and this comparator is a plain value compare (34 081 of the
// corpus' 209 211 buses). Everywhere else:
//
// * `SeqVoltages`/`CplxSeqVoltages` — capi clamps `Nvalues > 3` to 3 and then
//   answers on a 4-node bus (`CAPI/CAPI_Alt.pas:2174-2186`), r4133 does not
//   clamp and returns the `-1` sentinel there (`DDLL/DBus.pas:296-300`), and
//   **both** substitute ground for a phase the bus does not carry
//   (`Find(i) = 0 => NodeV[0]`, `DBus.pas:305` == `CAPI_Alt.pas:2190`),
//   fabricating a 0 V phase on e.g. a `[1, 2, 10]` bus. The port answers under
//   **S-SEQ** (all three phase nodes or nothing — r4133's own stated intent,
//   `DBus.pas:299` *"Signify seq voltages n/A for less then 3 phases"*).
// * `VLL`/`puVLL` — both engines poll `FindIdx(jj)` BEFORE the `jj > 3 =>
//   jj := 1` wrap (`DBus.pas:580-584` == `CAPI_Alt.pas:2514-2524`), so they
//   pair phase 3 with node 4, pair a node with itself, or walk off the phase
//   set entirely; where the walk finds nothing capi bails to `DefaultResult`
//   after three tries (`CAPI_Alt.pas:2512-2529`, comment *"(2020-03-01) Changed
//   in DSS C-API to avoid some corner cases that resulted in infinite loops"*)
//   while r4133's unbounded `repeat` HANGS. The port answers under **S-VLL**
//   (line-to-line over the phase nodes actually present), which is what
//   r4133's own report path computes (`Common/ShowResults.pas:193-194` wraps
//   first) and what its own commented-out original did (`DBus.pas:586-587`).
//
// **Shape of the compare (coordinator decisions D4 / D8 / D21, the D15/D16
// settlement shape).** Nothing is excluded and no ledger row is spent. Each bus
// is classified from its own NODE SET — never from a value — and then either
// value-compared against the port's accessor (209 091 of 209 211 buses) or
// closed by a POSITIVE assertion of the upstream mechanism over the port's own
// `node_v`: `oracle == upstream_walk(port state)`. The four exceptional classes
// are counted into run-wide populations that fail on stale in both directions,
// so a bus that quietly changes class is a finding.
//
// **Tolerances: one new constant, [`C_012`]**, and no existing band moves.
// Derivations: tests/TOLERANCE_NOTES.md §"Bus sequence and line-to-line
// voltages (GOLDEN_REBASE G1.4c)".
// ---------------------------------------------------------------------------

/// The extra **relative** offset the r4133 sequence transform carries against
/// the port's and capi's, on the positive- and negative-sequence rows only.
///
/// r4133 builds `Ap2s` from the truncated `sin 60 deg = 0.866025403` and inverts
/// it numerically (`Version8/Source/Shared/mathutil.pas:302-303` `SetAMatrix`,
/// `:562-564` `SetAMatrix(Ap2s); Ap2s.Invert`), while capi selects the
/// better-precision analytic pair (`Shared/mathutil.pas:548`
/// `SelectAs2pVersion(False)`) and the port's default is the same
/// ([`SymComp::precise`], `crates/dss-core/src/support/mathutil/mod.rs:42`).
/// Rows 1 and 2 carry two entries that differ by `dsin60/3` each, so with
/// `sum_j |Ap2s[i][j]| = 1` the triangle inequality gives the **analytic
/// ceiling**
///
/// ```text
/// 2 * (0.8660254037844387 - 0.866025403) / 3 = 5.2296e-10
/// ```
///
/// per unit of `max_j |Vph_j|`; row 0 is `[1/3, 1/3, 1/3]` on both variants to
/// within the inversion's own ulps and takes no term at all. `5.30e-10` is that
/// ceiling rounded up — corpus-independent, and consistent with the 4.50e-10
/// relative gap the in-engine pin
/// `support::mathutil::tests::sym_comp_official_vs_precise_gap_is_the_truncated_sin60_constant`
/// measures between the two variants. A measured worst ratio ABOVE the ceiling
/// is a bug, never a widening.
///
/// *Dedup at merge (D7/D21):* lane-e's G1.3b derives the identical constant for
/// the element sequence surface, under the same name `C_012`; the merge keeps
/// ONE definition, reconciled by the analytic ceiling.
///
/// [`SymComp::precise`]: dss_core::support::mathutil::SymComp::precise
const C_012: f64 = 5.30e-10;

/// How many of the phase nodes 1, 2 and 3 the bus carries. The criterion S-SEQ
/// and S-VLL are stated in — never the node COUNT the two oracles test.
fn phase_nodes_present(nodes: &[i32]) -> usize {
    nodes.iter().filter(|n| (1..=3).contains(*n)).count()
}

/// Does this oracle channel decline the sequence pair on a bus with `n` nodes?
///
/// * **capi**: `n < 3` — `Nvalues > 3` is clamped to 3 first
///   (`CAPI_Alt.pas:2174-2176` for `SeqVoltages`, `:2373-2375` for the complex
///   arm), so the `<> 3` test only ever fires below three nodes.
/// * **r4133**: `n != 3` — no clamp at all (`DBus.pas:296-297` == `:530-531`),
///   so a 4-node bus gets the sentinel where capi computes a value.
///
/// The reply itself is the same three/six `-1.0` doubles on both channels
/// (`CAPI_Alt.pas:2181-2186`/`:2379-2381` == `DBus.pas:299`/`:531`, where
/// `cmplx(-1,-1) x 3` is those six doubles), which is what makes the split
/// recognizable structurally rather than by value.
fn seq_channel_declines(channel: PropsChannel, n: usize) -> bool {
    match channel {
        PropsChannel::CapiV0145 => n < 3,
        PropsChannel::R4133 => n != 3,
    }
}

/// What upstream's `VLL`/`puVLL` walk does on a bus with these (ascending) node
/// numbers — the literal transcription of `CAPI_Alt.pas:2479-2537` (`bounded`)
/// and `DBus.pas:558-597` (unbounded), used ONLY to build the oracle's
/// expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
enum UpstreamVll {
    /// `Nvalues <= 1`: both engines return the two-double `[-99999.0, 0.0]`
    /// without looking at a voltage (`CAPI_Alt.pas:2485-2492` ==
    /// `DBus.pas:594-596`).
    OnePhase,
    /// The `(node_i, node_j)` pairs the walk produced, by node NUMBER.
    Pairs(Vec<(i32, i32)>),
    /// capi's `DefaultResult` — the bounded `for k := 1 to 3` found no partner,
    /// which abandons the WHOLE array and returns the single `0.0`
    /// (`CAPI_Alt.pas:2525-2530`).
    CapiDefault,
    /// The first `repeat` (`CAPI_Alt.pas:2507-2510` == `DBus.pas:575-578`) never
    /// terminates: it walks `jj` upward without a bound, so a bus whose node
    /// numbers are all below `i` hangs BOTH engines. Defensive — no corpus bus
    /// reaches it (`crates/dss-epri/src/modes.rs`'s independent predicate
    /// measures the same 0).
    FirstLoopHang,
    /// r4133's second `repeat` (`DBus.pas:580-584`) never terminates: its probe
    /// set is `{jj0} + {1, 2, 3, 4}` and the bus carries none of them.
    /// Live-proven on `NEVTestCase` `double-1..6` (`[10, 31, 32, 33, 41, 42,
    /// 43]`).
    SecondLoopHang,
}

/// Replay the upstream `VLL` walk over `nodes` (ascending node numbers).
/// `bounded` selects capi's `for k := 1 to 3` partner scan; `false` is r4133's
/// unbounded `repeat`.
///
/// This is the harness' own transcription; `dss-epri`'s
/// `modes::bus_vll_would_hang` is an independent one, and the comparator asserts
/// the two agree on every bus of every gated case (a refusal here must meet a
/// refusal there, and vice versa).
fn upstream_vll(nodes: &[i32], bounded: bool) -> UpstreamVll {
    // `Nvalues := NumNodesThisBus; if > 3 then 3; if <= 1 then bail;
    //  if = 2 then 1` — `CAPI_Alt.pas:2481-2495` == `DBus.pas:559-563`.
    let nvalues = match nodes.len().min(3) {
        0 | 1 => return UpstreamVll::OnePhase,
        2 => 1,
        k => k,
    };
    // Both `repeat`s are unbounded upward in the Pascal; a terminating first
    // loop can only stop on a node number the bus carries, so `max(nodes)`
    // bounds it and anything past that is the hang itself.
    let ceiling = nodes.iter().copied().max().unwrap_or(0);
    let mut pairs = Vec::with_capacity(nvalues);
    for i in 1..=(nvalues as i32) {
        let mut jj = i;
        // `repeat NodeIdxi := FindIdx(jj); inc(jj) until NodeIdxi > 0` — `jj`
        // is left one PAST the node that matched.
        let node_i = loop {
            if jj > ceiling {
                return UpstreamVll::FirstLoopHang;
            }
            let hit = nodes.contains(&jj);
            jj += 1;
            if hit {
                break jj - 1;
            }
        };
        // The partner scan. `NodeIdxj := FindIdx(jj)` is evaluated BEFORE the
        // `if jj > 3 then jj := 1 else inc(jj)` wrap, so the probe sequence is
        // `jj0, then 1, 2, 3, 4, 1, 2, 3, 4, ...` — the pre-wrap poll is the
        // whole defect (`DBus.pas:581-583` == `CAPI_Alt.pas:2516-2520`).
        let mut node_j = None;
        // capi runs the body at most three times; r4133 cycles forever, and
        // five probes exhaust the reachable set `{jj0} + {1, 2, 3, 4}`.
        let tries = if bounded { 3 } else { 5 };
        for _ in 0..tries {
            let probe = jj;
            jj = if jj > 3 { 1 } else { jj + 1 };
            if nodes.contains(&probe) {
                node_j = Some(probe);
                break;
            }
        }
        match node_j {
            Some(j) => pairs.push((node_i, j)),
            None if bounded => return UpstreamVll::CapiDefault,
            None => return UpstreamVll::SecondLoopHang,
        }
    }
    UpstreamVll::Pairs(pairs)
}

/// The port's own **S-VLL** pairing, by node number — a second implementation of
/// `exec::view::bus_line_to_line`, kept here so the comparator decides "did
/// upstream reproduce S-VLL?" without asking the code under test. `None` = the
/// port publishes no line-to-line voltage for this bus.
fn port_vll_pairs(nodes: &[i32]) -> Option<Vec<(i32, i32)>> {
    let present: Vec<i32> = (1..=3).filter(|k| nodes.contains(k)).collect();
    match present.len() {
        3 => Some(vec![(1, 2), (2, 3), (3, 1)]),
        2 => Some(vec![(present[0], present[1])]),
        _ => None,
    }
}

/// The bus's `Solution.NodeV` entry for node NUMBER `num`, or ground when the
/// bus does not carry it.
///
/// `nodes` is the bus's ascending node-number list and `node_v` the port's
/// `BusVoltageView::node_v`, which is ordered by the same ascending walk — so
/// position `k` of one indexes the other. The ground fallback is exactly
/// upstream's `Find(i) = 0 => NodeV[0]` conflation (`DBus.pas:305` ==
/// `CAPI_Alt.pas:2190`) and is reached only by the sequence replay; the VLL
/// replay only ever asks for node numbers its own walk just found.
fn bus_node_voltage(nodes: &[i32], node_v: &[Complex64], num: i32) -> Complex64 {
    match nodes.iter().position(|x| *x == num) {
        Some(k) => node_v[k],
        None => Complex64::ZERO,
    }
}

/// Apply an upstream pairing to the port's own bus state.
fn expected_vll(nodes: &[i32], node_v: &[Complex64], pairs: &[(i32, i32)]) -> Vec<Complex64> {
    pairs
        .iter()
        .map(|&(a, b)| bus_node_voltage(nodes, node_v, a) - bus_node_voltage(nodes, node_v, b))
        .collect()
}

/// The **line-to-line** `BaseFactor` both engines divide `puVLL` by:
/// `1000 * kVBase * sqrt3`, or `1.0` when the bus has no base
/// (`DBus.pas:622-623` == `CAPI_Alt.pas:2427-2430`). Distinct from
/// [`bus_base_factor`], which carries no `sqrt3`; `sqrt3` is `Sqrt(3.0)` on
/// every engine (r4133 `Common/DSSGlobals.pas:2033` == capi `:733` ==
/// `dss_core::util::sqrt3`), an IEEE-exact operation, so the divide contributes
/// no error of its own. Taken from the ORACLE's `kv_base`, which
/// [`compare_bus`] has already pinned exactly against the port's.
fn bus_ll_base_factor(kv_base: f64) -> f64 {
    if kv_base > 0.0 {
        1000.0 * kv_base * 3.0f64.sqrt()
    } else {
        1.0
    }
}

/// The four exceptional classes one [`compare_bus_seq_and_vll`] call met, plus
/// the buses it classified at all.
///
/// Every field is DISCRETE — derived from the bus's node set and the channel,
/// never from a value — so `voltages_excluded` cannot move it and the run-wide
/// populations below stay stable under a ledger suppression.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SeqVllCounts {
    /// Buses this call classified. Equals the bus count on every call: the
    /// classification is total, which is the blindness guard — no bus can be
    /// silently unwitnessed on either channel.
    pub buses: usize,
    /// r4133 declined the sequence pair (`n > 3`) while capi answers.
    pub r4133_seq_sentinel: usize,
    /// The channel answered the sequence pair over a GROUND-substituted phase
    /// (`n >= 3` with one of 1/2/3 missing) where the port declines.
    pub seq_ground_substitution: usize,
    /// The upstream `VLL` walk did not reproduce S-VLL's pairing — the port's
    /// accessor is not the direct witness and the mechanism is asserted
    /// instead. Includes capi's `DefaultResult` bail.
    pub vll_upstream_pairing_declines: usize,
    /// The r4133 bridge refused the `VLL` call because the walk would hang.
    pub r4133_vll_hang: usize,
}

// `AtomicUsize` itself comes from the file-header import (the element lane's
// G1.3d(ii) control census added it); only the `Relaxed` alias these counters
// spell is local.
use std::sync::atomic::Ordering::Relaxed as AtomicRelaxed;

/// Gating comparator calls that met each exception class at all, and the buses
/// they met it on — one `(walks, buses)` pair per class, in the same shape and
/// for the same reason as [`SC_STUDY_WALKS`]/[`SC_STUDY_BUSES`].
static SEQ_SENTINEL_WALKS: AtomicUsize = AtomicUsize::new(0);
static SEQ_SENTINEL_BUSES: AtomicUsize = AtomicUsize::new(0);
static SEQ_GROUND_WALKS: AtomicUsize = AtomicUsize::new(0);
static SEQ_GROUND_BUSES: AtomicUsize = AtomicUsize::new(0);
static VLL_PAIRING_WALKS: AtomicUsize = AtomicUsize::new(0);
static VLL_PAIRING_BUSES: AtomicUsize = AtomicUsize::new(0);
static VLL_HANG_WALKS: AtomicUsize = AtomicUsize::new(0);
static VLL_HANG_BUSES: AtomicUsize = AtomicUsize::new(0);

/// The four run-wide populations as `(walks, buses)`: how many gating
/// comparator calls met the class at all, and how many buses they met it on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeqVllPopulation {
    pub r4133_seq_sentinel: (usize, usize),
    pub seq_ground_substitution: (usize, usize),
    pub vll_upstream_pairing_declines: (usize, usize),
    pub r4133_vll_hang: (usize, usize),
}

/// r4133 answers `-1` where capi computes a real `V012`: the buses with `n > 3`
/// on the r4133 channel. Re-derived on every full-population run and asserted
/// EXACTLY, so the number fails on a drop AND on a growth (the D15/D16 shape:
/// the class is not excluded, it is counted).
///
/// The live run prints all four on its own `corpus_gate seq/vll:` line; move
/// them only deliberately. **Measured** (G1.4c F5, full gate 2026-09-05): ten
/// decks carry this one — the two `NEVTestCase` masters (55 buses each),
/// `Examples/GICExample/GIC_Example` (7), the two `asymmetric:gic` micro decks
/// (3 each), `Test/indmachtest/Master` (2), and one bus each in
/// `Test/MultiCircuitTest`, `Test/YgD-Test`, `epri_dpv/K1/Master_NoPV` and
/// `4Bus-YD-Bal`. `modes:makeposseq/makeposseq_gic` declares `busNH=b1.4.4.4`
/// and so LOOKS like an eleventh, but its deck ends `makeposseq; solve`: the
/// positive-sequence reduction leaves `src` and `b1` carrying a single node
/// each at the gated step, so the case reaches none of the four classes
/// (probed, `tmp/g14c/f5_c012.py gic`).
const R4133_SEQ_SENTINEL_POPULATION: (usize, usize) = (10, 129);

/// The channel transformed a ground-substituted phase where the port declines:
/// `n >= 3` with one of the phase nodes 1/2/3 missing, on a channel that
/// answers there. The two `NEVTestCase` decks carry all of it — 19 buses per
/// deck on capi (which also answers at `n > 3`) and 8 on r4133.
const SEQ_GROUND_SUBSTITUTION_POPULATION: (usize, usize) = (4, 54);

/// The upstream walk paired differently from S-VLL (capi's `DefaultResult`
/// included), per channel — the buses whose `VLL`/`puVLL` are closed by the
/// mechanism assertion instead of the port's own accessor.
const VLL_UPSTREAM_PAIRING_DECLINES: (usize, usize) = (16, 196);

/// The calls the r4133 bridge refused because `DBus.pas:580-584` would not
/// terminate (`NEVTestCase` `double-1..6` on both NEV decks).
const R4133_VLL_HANG_POPULATION: (usize, usize) = (2, 12);

/// Record one **gating** sequence/line-to-line compare's class counts.
///
/// The single caller is `corpus_gate/runner.rs`' `compare_bus` block — the
/// gate's one entry point into this comparator. The harness' own unit drives
/// call [`compare_bus_seq_and_vll`] directly and are deliberately NOT counted:
/// they run in the same process as the gate in this test binary, and counting
/// them would make the exact populations above unstable (and green under
/// exactly the corpus regression they exist to catch) — the reason
/// [`record_sc_study_compare`]'s doc gives.
pub fn record_seq_vll_populations(counts: SeqVllCounts) {
    for (n, walks, buses) in [
        (
            counts.r4133_seq_sentinel,
            &SEQ_SENTINEL_WALKS,
            &SEQ_SENTINEL_BUSES,
        ),
        (
            counts.seq_ground_substitution,
            &SEQ_GROUND_WALKS,
            &SEQ_GROUND_BUSES,
        ),
        (
            counts.vll_upstream_pairing_declines,
            &VLL_PAIRING_WALKS,
            &VLL_PAIRING_BUSES,
        ),
        (counts.r4133_vll_hang, &VLL_HANG_WALKS, &VLL_HANG_BUSES),
    ] {
        if n > 0 {
            walks.fetch_add(1, AtomicRelaxed);
            buses.fetch_add(n, AtomicRelaxed);
        }
    }
}

/// What [`record_seq_vll_populations`] has counted in this process.
pub fn seq_vll_counters() -> SeqVllPopulation {
    let pair = |w: &AtomicUsize, b: &AtomicUsize| (w.load(AtomicRelaxed), b.load(AtomicRelaxed));
    SeqVllPopulation {
        r4133_seq_sentinel: pair(&SEQ_SENTINEL_WALKS, &SEQ_SENTINEL_BUSES),
        seq_ground_substitution: pair(&SEQ_GROUND_WALKS, &SEQ_GROUND_BUSES),
        vll_upstream_pairing_declines: pair(&VLL_PAIRING_WALKS, &VLL_PAIRING_BUSES),
        r4133_vll_hang: pair(&VLL_HANG_WALKS, &VLL_HANG_BUSES),
    }
}

/// **Fail-on-stale for the four G1.4c exception classes**, called once from the
/// corpus gate's epilogue. Silent under `DSS_GATE_ONLY` — a filtered run
/// legitimately holds none of the eleven carrying decks — exactly like its
/// neighbours [`assert_sc_study_compare_ran`] and
/// [`props_norm::assert_r4133_props_compare_ran`].
///
/// [`props_norm::assert_r4133_props_compare_ran`]: props_norm::assert_r4133_props_compare_ran
pub fn assert_seq_vll_populations() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    check_seq_vll_populations(seq_vll_counters());
}

/// The rule itself, over **injected** counters — split off for the reason
/// [`check_sc_study_compare_ran`]'s twin is: the shipped statics cannot be
/// rewound from a test once the gate has moved them, so both directions are
/// pinned offline.
pub fn check_seq_vll_populations(got: SeqVllPopulation) {
    for (what, have, want, why) in [
        (
            "r4133 sequence sentinel (n > 3)",
            got.r4133_seq_sentinel,
            R4133_SEQ_SENTINEL_POPULATION,
            "r4133's missing `Nvalues > 3` clamp (`DBus.pas:296-297`)",
        ),
        (
            "sequence ground substitution",
            got.seq_ground_substitution,
            SEQ_GROUND_SUBSTITUTION_POPULATION,
            "both oracles' `Find(i) = 0 => NodeV[0]` conflation (`DBus.pas:305`)",
        ),
        (
            "VLL upstream pairing declines",
            got.vll_upstream_pairing_declines,
            VLL_UPSTREAM_PAIRING_DECLINES,
            "the pre-wrap `FindIdx(jj)` poll (`DBus.pas:581-583`)",
        ),
        (
            "r4133 VLL hang refusals",
            got.r4133_vll_hang,
            R4133_VLL_HANG_POPULATION,
            "the unbounded partner `repeat` (`DBus.pas:580-584`)",
        ),
    ] {
        assert_eq!(
            have, want,
            "the G1.4c population `{what}` came out {have:?}, not {want:?} — {why}. \
             Fewer means a deck stopped carrying the class (and its positive mechanism \
             assertion now runs over nothing); more means a new deck reached it. \
             Re-derive the pair from the run's own `corpus_gate seq/vll:` line and move \
             it deliberately (GOLDEN_REBASE G1.4c)."
        );
    }
}

/// Compare every bus's captured sequence and line-to-line voltages against the
/// engine's, in `BusList` order (GOLDEN_REBASE_PLAN.md G1.4c).
///
/// Structure per bus, **strongest first**:
/// 1. **availability**, discrete and two-sided. The sequence pair: the channel's
///    own rule ([`seq_channel_declines`]) against the payload — a decline must
///    be the exact `-1.0` sentinel, an answer must carry no negative magnitude
///    (`Cabs` is never negative, so the two are provably disjoint). `VLL`: the
///    replayed walk class ([`upstream_vll`]) against the payload — `OnePhase`
///    against the two-double `[-99999.0, 0.0]`, `CapiDefault` against the
///    one-double `[0.0]`, `SecondLoopHang` against the bridge's own
///    `vll_declined`. That last one is a cross-check of **two independent
///    transcriptions** of the same Pascal loop (this one and
///    `dss-epri::modes::bus_vll_would_hang`); `vll_declined` must be `false` on
///    the capi channel always, whose partner scan is bounded.
/// 2. **lengths** — `3` and `6` for the sequence arms, `2 * pairs` for `VLL`
///    and `puVLL`, and `len(vll) == len(pu_vll)` (one loop over one node set
///    writes both, so a mismatch is a transport defect).
/// 3. **values**, only where the channel answered. Where the port answers too,
///    against the port's own accessor. Where it declines (S-SEQ's ground
///    substitution, S-VLL's pairing), against the upstream walk replayed over
///    the port's own `node_v` — a positive assertion of the oracle's mechanism,
///    never a skip, and the bus is counted into the matching run-wide
///    population ([`check_seq_vll_populations`]).
///
/// `voltages_excluded` is the caller's D11(2) flag, **narrowed** exactly as in
/// [`compare_bus_short_circuit`]: it drops step 3 only. Every availability
/// classification, every sentinel and every length stays compared, so a case
/// whose node voltages the ledger already triaged still gates the whole
/// structure of this surface.
///
/// Returns the classes this call met, for [`record_seq_vll_populations`].
pub fn compare_bus_seq_and_vll(
    dss: &Dss,
    exp: &[BusCap],
    tol: &Tolerances,
    channel: PropsChannel,
    voltages_excluded: bool,
    ctx: &str,
) -> SeqVllCounts {
    let mut counts = SeqVllCounts::default();
    let views = dss.all_bus_voltages();
    assert_eq!(
        views.len(),
        exp.len(),
        "{ctx}: bus count differs ({} vs {})",
        views.len(),
        exp.len()
    );
    let sym = dss_core::support::mathutil::SymComp::precise();
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
        let n = nodes.len();
        assert_eq!(
            v.node_v.len(),
            n,
            "{ctx}: bus {} port node_v has {} entries, not {n}",
            e.name,
            v.node_v.len()
        );
        // `compare_bus` pins this exactly too; repeated here so `puVLL`'s
        // line-to-line base is well founded WITHOUT depending on the order the
        // runner calls the two comparators in.
        assert_eq!(v.kv_base, e.kv_base, "{ctx}: bus {} kVBase differs", e.name);

        // ---- the sequence pair --------------------------------------------
        assert_eq!(
            e.seq_voltages.len(),
            3,
            "{ctx}: bus {} SeqVoltages length {} is not 3",
            e.name,
            e.seq_voltages.len()
        );
        assert_eq!(
            e.cplx_seq_voltages.len(),
            6,
            "{ctx}: bus {} CplxSeqVoltages length {} is not 6",
            e.name,
            e.cplx_seq_voltages.len()
        );
        let declines = seq_channel_declines(channel, n);
        if declines {
            assert_eq!(
                e.seq_voltages,
                vec![-1.0; 3],
                "{ctx}: bus {} ({n} nodes): the {} oracle should have published the \
                 n/A sentinel for SeqVoltages",
                e.name,
                channel.tag()
            );
            assert_eq!(
                e.cplx_seq_voltages,
                vec![-1.0; 6],
                "{ctx}: bus {} ({n} nodes): the {} oracle should have published the \
                 n/A sentinel for CplxSeqVoltages",
                e.name,
                channel.tag()
            );
            if channel == PropsChannel::R4133 && n > 3 {
                // The class the missing clamp creates: capi computes a real
                // V012 on this very bus, and so does the port when it carries
                // all three phases.
                counts.r4133_seq_sentinel += 1;
            }
        } else {
            // `Cabs` is never negative, so a non-sentinel payload is provably
            // an answer — this is the other direction of the availability rule,
            // not a value compare.
            assert!(
                e.seq_voltages.iter().all(|x| *x >= 0.0),
                "{ctx}: bus {} ({n} nodes): the {} oracle answers here, but SeqVoltages \
                 carries a negative magnitude {:?}",
                e.name,
                channel.tag(),
                e.seq_voltages
            );
        }
        // The three phase voltages upstream feeds its transform, ground
        // substituted — identical to the port's own three when it answers.
        let vph = [1i32, 2, 3].map(|k| bus_node_voltage(&nodes, &v.node_v, k));
        let max_vph = vph.iter().fold(0.0f64, |m, c| m.max(c.norm()));
        if !declines && v.cplx_seq_voltages.is_none() {
            // The only reason S-SEQ declines where a channel answers: the bus
            // has >= 3 nodes but not all three PHASE nodes, so upstream's
            // `Find(i) = 0 => NodeV[0]` fabricated a 0 V phase.
            assert!(
                phase_nodes_present(&nodes) < 3 && n >= 3,
                "{ctx}: bus {} (nodes {nodes:?}): the port declines the sequence pair on a                  bus that carries all three phase nodes",
                e.name
            );
            counts.seq_ground_substitution += 1;
        }
        // Port-internal identities — not oracle comparisons, so they hold on a
        // ledger-suppressed case too (the G1.4a audit settlement AC-7 rule):
        // the port's two sequence arms come out of ONE transform, and that
        // transform is `Ap2s * Vph` over the port's own `node_v`.
        if let Some(x) = v.cplx_seq_voltages {
            let mags = v.seq_voltages.unwrap_or_else(|| {
                panic!(
                    "{ctx}: bus {}: the port has CplxSeqVoltages but no SeqVoltages",
                    e.name
                )
            });
            for (k, (m, c)) in mags.iter().zip(&x).enumerate() {
                assert_eq!(
                    *m,
                    c.norm(),
                    "{ctx}: bus {} SeqVoltages[{k}] is not |CplxSeqVoltages[{k}]|",
                    e.name
                );
            }
            let mut replay = [Complex64::ZERO; 3];
            sym.phase_to_sym(&vph, &mut replay);
            assert_eq!(
                x, replay,
                "{ctx}: bus {}: the port's CplxSeqVoltages is not `Ap2s * Vph` over its own                  node_v",
                e.name
            );
        }
        if !declines && !voltages_excluded {
            // Where S-SEQ declines and the channel does not, the expectation is
            // the transform of the GROUND-SUBSTITUTED phases — a positive
            // assertion of the oracle's mechanism (the D15/D16 shape), never a
            // skip.
            let port012 = v.cplx_seq_voltages.unwrap_or_else(|| {
                let mut replay = [Complex64::ZERO; 3];
                sym.phase_to_sym(&vph, &mut replay);
                replay
            });
            // Band: `sum_j |Ap2s[i][j]| = 1` maps the node-voltage band onto
            // every V012 component unchanged, driven by the PHASE magnitude
            // (V0/V2 are near-total cancellations of three ~equal phasors and
            // carry no relative band of their own). The r4133 channel adds
            // `C_012` on rows 1 and 2. tests/TOLERANCE_NOTES.md §"Bus sequence
            // and line-to-line voltages (GOLDEN_REBASE G1.4c)".
            let base = tol.v_abs + tol.v_rel * max_vph;
            let extra = if channel == PropsChannel::R4133 {
                C_012 * max_vph
            } else {
                0.0
            };
            let oracle_cplx = deinterleave(&e.cplx_seq_voltages);
            for k in 0..3 {
                let allowed = base + if k == 0 { 0.0 } else { extra };
                let d = (port012[k] - oracle_cplx[k]).norm();
                assert!(
                    d <= allowed,
                    "{ctx}: bus {} CplxSeqVoltages[{k}] differs: {} vs {} \
                     (|diff| = {d:.3e} > allowed {allowed:.3e}, max|Vph| = {max_vph:e} V)",
                    e.name,
                    port012[k],
                    oracle_cplx[k]
                );
                // `||a| - |b|| <= |a - b|`, so the magnitude arm takes the same
                // absolute band.
                let dm = (port012[k].norm() - e.seq_voltages[k]).abs();
                assert!(
                    dm <= allowed,
                    "{ctx}: bus {} SeqVoltages[{k}] differs: {} vs {} \
                     (|diff| = {dm:.3e} > allowed {allowed:.3e})",
                    e.name,
                    port012[k].norm(),
                    e.seq_voltages[k]
                );
            }
        }

        // ---- the line-to-line pair -----------------------------------------
        assert_eq!(
            e.vll.len(),
            e.pu_vll.len(),
            "{ctx}: bus {} VLL and puVLL lengths differ ({} vs {}) — one loop over one \
             node set writes both",
            e.name,
            e.vll.len(),
            e.pu_vll.len()
        );
        if channel == PropsChannel::CapiV0145 {
            assert!(
                !e.vll_declined,
                "{ctx}: bus {}: the capi transport declined a VLL read, but its partner \
                 scan is bounded (`CAPI_Alt.pas:2512-2524`) and cannot hang",
                e.name
            );
        }
        let ll_base = bus_ll_base_factor(e.kv_base);
        // Port-internal identities for the L-L arm — the twin of the sequence
        // arm's above, and like it UNCONDITIONAL: the port's own S-VLL shape,
        // its `V_a − V_b` values and the `puVLL = VLL / BaseFactor_LL` identity
        // are asserted on EVERY bus, on both channels, whatever the oracle's
        // walk did and whether or not values are excluded. They used to run only
        // inside the `direct` branch below, which left `exec::view`'s accessor
        // unwitnessed by the live gate on exactly the buses where the two walks
        // disagree — the buses this sub-step exists for (G1.4c audit-code AC-1).
        match port_vll_pairs(&nodes) {
            None => assert!(
                v.vll.is_none() && v.pu_vll.is_none(),
                "{ctx}: bus {} (nodes {nodes:?}): the port published a line-to-line \
                 voltage for a bus with fewer than two phase nodes",
                e.name
            ),
            Some(port_pairs) => {
                let port_vll = v.vll.as_ref().unwrap_or_else(|| {
                    panic!(
                        "{ctx}: bus {} (nodes {nodes:?}): S-VLL pairs {port_pairs:?} but \
                         the port published no VLL",
                        e.name
                    )
                });
                assert_eq!(
                    *port_vll,
                    expected_vll(&nodes, &v.node_v, &port_pairs),
                    "{ctx}: bus {}: the port's VLL is not `V_a − V_b` over its own node_v \
                     for the pairs {port_pairs:?}",
                    e.name
                );
                let port_pu = v.pu_vll.as_ref().unwrap_or_else(|| {
                    panic!("{ctx}: bus {}: the port has VLL but no puVLL", e.name)
                });
                let scaled: Vec<Complex64> = port_vll.iter().map(|c| *c / ll_base).collect();
                assert_eq!(
                    *port_pu, scaled,
                    "{ctx}: bus {}: the port's puVLL is not its VLL over the line-to-line \
                     base {ll_base}",
                    e.name
                );
            }
        }
        let walk = upstream_vll(&nodes, channel == PropsChannel::CapiV0145);
        match &walk {
            UpstreamVll::FirstLoopHang => {
                // Defensive — no corpus bus reaches it. `dss-epri`'s independent
                // predicate refuses this shape too (`modes::VLL_HANG_FIRST_LOOP`),
                // so a *refusal* here is the two transcriptions AGREEING and only
                // an ANSWER contradicts the Pascal (G1.4c audit-code AC-4). On the
                // capi channel `vll_declined` is already asserted false above —
                // that transport carries no guard at all.
                assert!(
                    e.vll_declined,
                    "{ctx}: bus {} (nodes {nodes:?}): the {} oracle ANSWERED a VLL read \
                     whose FIRST `repeat` (`DBus.pas:575-578` == \
                     `CAPI_Alt.pas:2507-2510`) cannot terminate — the capture and the \
                     Pascal disagree",
                    e.name,
                    channel.tag()
                );
                assert!(
                    e.vll.is_empty() && e.pu_vll.is_empty(),
                    "{ctx}: bus {}: the bridge refused the VLL read but published {} / {} \
                     doubles",
                    e.name,
                    e.vll.len(),
                    e.pu_vll.len()
                );
                counts.r4133_vll_hang += 1;
            }
            UpstreamVll::SecondLoopHang => {
                assert_eq!(
                    channel,
                    PropsChannel::R4133,
                    "{ctx}: bus {}: only r4133's partner scan is unbounded",
                    e.name
                );
                assert!(
                    e.vll_declined,
                    "{ctx}: bus {} (nodes {nodes:?}): this harness' replay says \
                     `DBus.pas:580-584` would not terminate, but the bridge did NOT refuse \
                     the call — the two transcriptions disagree",
                    e.name
                );
                assert!(
                    e.vll.is_empty() && e.pu_vll.is_empty(),
                    "{ctx}: bus {}: the bridge refused the VLL read but published {} / {} \
                     doubles",
                    e.name,
                    e.vll.len(),
                    e.pu_vll.len()
                );
                counts.r4133_vll_hang += 1;
            }
            UpstreamVll::OnePhase => {
                assert!(
                    !e.vll_declined,
                    "{ctx}: bus {}: the bridge refused a VLL read that never enters the \
                     loop (`DBus.pas:594-596`)",
                    e.name
                );
                for (arr, what) in [(&e.vll, "VLL"), (&e.pu_vll, "puVLL")] {
                    assert_eq!(
                        arr.as_slice(),
                        [-99999.0f64, 0.0].as_slice(),
                        "{ctx}: bus {} ({n} nodes) {what}: the {} oracle should have \
                         published the 1-phase sentinel",
                        e.name,
                        channel.tag()
                    );
                }
                // (the port's own `None` here is asserted unconditionally above)
            }
            UpstreamVll::CapiDefault => {
                assert_eq!(
                    channel,
                    PropsChannel::CapiV0145,
                    "{ctx}: bus {}: only capi bails to `DefaultResult`",
                    e.name
                );
                for (arr, what) in [(&e.vll, "VLL"), (&e.pu_vll, "puVLL")] {
                    assert_eq!(
                        arr.as_slice(),
                        [0.0f64].as_slice(),
                        "{ctx}: bus {} (nodes {nodes:?}) {what}: the bounded partner scan \
                         found nothing, so capi should have published `DefaultResult`",
                        e.name
                    );
                }
                counts.vll_upstream_pairing_declines += 1;
            }
            UpstreamVll::Pairs(pairs) => {
                assert!(
                    !e.vll_declined,
                    "{ctx}: bus {}: the bridge refused a VLL read this replay resolves to \
                     {pairs:?}",
                    e.name
                );
                for (arr, what) in [(&e.vll, "VLL"), (&e.pu_vll, "puVLL")] {
                    assert_eq!(
                        arr.len(),
                        2 * pairs.len(),
                        "{ctx}: bus {} {what} length {} is not 2*{} (the walk's pairs \
                         {pairs:?})",
                        e.name,
                        arr.len(),
                        pairs.len()
                    );
                }
                // Where upstream's pairing IS the port's, the unconditional
                // block above has already pinned the port's accessor against
                // this harness' independent replay, so "compared against the
                // port" cannot mean "compared against a rewritten pairing";
                // where it is not, the class is counted and the mechanism
                // asserted below.
                if port_vll_pairs(&nodes).as_deref() != Some(pairs.as_slice()) {
                    counts.vll_upstream_pairing_declines += 1;
                }
                if !voltages_excluded {
                    // Either way the numbers compared are the pairs UPSTREAM
                    // walked, applied to the port's own state: identical to the
                    // port's accessor in the `direct` case (asserted above), the
                    // mechanism assertion otherwise.
                    let expect = expected_vll(&nodes, &v.node_v, pairs);
                    let oracle_vll = deinterleave(&e.vll);
                    let oracle_pu = deinterleave(&e.pu_vll);
                    for (k, &(a, b)) in pairs.iter().enumerate() {
                        // `V_ij = Vph_i - Vph_j`: two node bands add.
                        let va = bus_node_voltage(&nodes, &v.node_v, a).norm();
                        let vb = bus_node_voltage(&nodes, &v.node_v, b).norm();
                        let allowed = 2.0 * tol.v_abs + tol.v_rel * (va + vb);
                        let d = (expect[k] - oracle_vll[k]).norm();
                        assert!(
                            d <= allowed,
                            "{ctx}: bus {} VLL[{k}] (nodes {a}-{b}) differs: {} vs {} \
                             (|diff| = {d:.3e} > allowed {allowed:.3e})",
                            e.name,
                            expect[k],
                            oracle_vll[k]
                        );
                        // `puVLL` is the same difference over the exact
                        // line-to-line base.
                        let dpu = (expect[k] / ll_base - oracle_pu[k]).norm();
                        let allowed_pu = allowed / ll_base;
                        assert!(
                            dpu <= allowed_pu,
                            "{ctx}: bus {} puVLL[{k}] (nodes {a}-{b}) differs: {} vs {} \
                             (|diff| = {dpu:.3e} > allowed {allowed_pu:.3e})",
                            e.name,
                            expect[k] / ll_base,
                            oracle_pu[k]
                        );
                    }
                }
            }
        }
        counts.buses += 1;
    }
    // The blindness guard. What actually makes the classification total is that
    // every branch of the `match walk` either asserts or panics, plus the two
    // unconditional port-internal blocks above, which run before it on every
    // bus: a bus the comparator could not name cannot reach this line. The
    // count below is the tripwire for the ONE way that could change — a future
    // `continue` in the loop — and, as its own audit finding records
    // (G1.4c audit-code AC-7 / audit-tests), it cannot fail at HEAD, so it is
    // never a second, independent guarantee.
    assert_eq!(
        counts.buses,
        exp.len(),
        "{ctx}: {} of {} buses were classified",
        counts.buses,
        exp.len()
    );
    counts
}

#[cfg(test)]
mod bus_seq_vll_comparator_tests {
    use super::{
        BusCap, C_012, PropsChannel, R4133_SEQ_SENTINEL_POPULATION, R4133_VLL_HANG_POPULATION,
        SEQ_GROUND_SUBSTITUTION_POPULATION, SeqVllCounts, SeqVllPopulation, UpstreamVll,
        VLL_UPSTREAM_PAIRING_DECLINES, bus_ll_base_factor, bus_node_voltage,
        check_seq_vll_populations, compare_bus_seq_and_vll, expected_vll, interleave,
        phase_nodes_present, port_vll_pairs, seq_channel_declines, tol_for, upstream_vll,
    };
    use dss_core::exec::Dss;
    use dss_core::support::mathutil::SymComp;
    use num_complex::Complex64;

    /// The ten bus classes the corpus actually holds, measured live on both
    /// channels for the G1.4c spec (§3.1, 209 211 buses / 511 live cases) and
    /// re-probed at HEAD on 2026-09-05. `capi` = the bounded partner scan,
    /// `r4133` = the unbounded one.
    ///
    /// This is the harness' half of the two-transcription cross-check: the same
    /// Pascal loop is transcribed independently in
    /// `dss-epri::modes::bus_vll_would_hang`, and the live comparator asserts
    /// the two never disagree about a refusal.
    #[test]
    fn the_upstream_vll_walk_reproduces_the_ten_measured_bus_classes() {
        let pairs = |v: &[(i32, i32)]| UpstreamVll::Pairs(v.to_vec());
        let cases: Vec<(&[i32], UpstreamVll, UpstreamVll)> = vec![
            // 126 818 buses: a single phase node — neither engine enters the loop.
            (&[1], UpstreamVll::OnePhase, UpstreamVll::OnePhase),
            (&[3], UpstreamVll::OnePhase, UpstreamVll::OnePhase),
            // 2 buses: no nodes at all (`Test/REACTORTest.DSS` `loadbus2`).
            (&[], UpstreamVll::OnePhase, UpstreamVll::OnePhase),
            // 48 104 buses: exactly two phase nodes — one L-L pair, agreed.
            (&[1, 2], pairs(&[(1, 2)]), pairs(&[(1, 2)])),
            (&[2, 3], pairs(&[(2, 3)]), pairs(&[(2, 3)])),
            (&[1, 3], pairs(&[(1, 3)]), pairs(&[(1, 3)])),
            // 34 081 buses: the healthy three-phase bus.
            (
                &[1, 2, 3],
                pairs(&[(1, 2), (2, 3), (3, 1)]),
                pairs(&[(1, 2), (2, 3), (3, 1)]),
            ),
            // 21 buses: node 4 present — the pre-wrap poll pairs phase 3 with it.
            (
                &[1, 2, 3, 4],
                pairs(&[(1, 2), (2, 3), (3, 4)]),
                pairs(&[(1, 2), (2, 3), (3, 4)]),
            ),
            // 86 buses: a neutral at 10 does not disturb the walk.
            (
                &[1, 2, 3, 10],
                pairs(&[(1, 2), (2, 3), (3, 1)]),
                pairs(&[(1, 2), (2, 3), (3, 1)]),
            ),
            // 9 buses: `[1..5]`, `[1,2,3,4,10]`, … — node 4 again (`13kvbus`).
            (
                &[1, 2, 3, 4, 10],
                pairs(&[(1, 2), (2, 3), (3, 4)]),
                pairs(&[(1, 2), (2, 3), (3, 4)]),
            ),
            // 52 buses: `[1, 10]` — capi gives up after three probes, r4133
            // wraps onto node 1 and pairs it with ITSELF (`ckt1-1-1`: `[0, 0]`).
            (&[1, 10], UpstreamVll::CapiDefault, pairs(&[(1, 1)])),
            // 16 buses: `[1, 2, 10]` — the duplicate negative and the neutral
            // pair both engines agree on (`load1a`).
            (
                &[1, 2, 10],
                pairs(&[(1, 2), (2, 1), (10, 1)]),
                pairs(&[(1, 2), (2, 1), (10, 1)]),
            ),
            // 10 buses: the 13-node NEV `quad-*` — the same pair three times.
            (
                &[10, 11, 12, 13, 21, 22, 23, 31, 32, 33, 41, 42, 43],
                pairs(&[(10, 11), (10, 11), (10, 11)]),
                pairs(&[(10, 11), (10, 11), (10, 11)]),
            ),
            // 12 buses: the NEV `double-*` — capi bails, r4133 HANGS.
            (
                &[10, 31, 32, 33, 41, 42, 43],
                UpstreamVll::CapiDefault,
                UpstreamVll::SecondLoopHang,
            ),
        ];
        for (nodes, capi, r4133) in cases {
            assert_eq!(upstream_vll(nodes, true), capi, "capi walk over {nodes:?}");
            assert_eq!(
                upstream_vll(nodes, false),
                r4133,
                "r4133 walk over {nodes:?}"
            );
        }

        // The defensive first-loop arm: a bus whose node numbers are all below
        // `i` hangs BOTH engines' unbounded `repeat`. No corpus bus reaches it
        // (`dss-epri`'s independent sweep measures the same 0), so it is proven
        // here rather than live.
        assert_eq!(upstream_vll(&[-1, 0], true), UpstreamVll::FirstLoopHang);
        assert_eq!(upstream_vll(&[-1, 0], false), UpstreamVll::FirstLoopHang);

        // …and the pre-wrap poll is what decides: give the hanging class a node
        // the pre-wrap probe can see (11 == jj0) and r4133 terminates on it…
        assert_eq!(
            upstream_vll(&[10, 11, 31, 32, 33, 41, 42, 43], false),
            pairs(&[(10, 11), (10, 11), (10, 11)])
        );
        // …while a node 4 — the one number the wrap cycle can reach — makes it
        // terminate by pairing that node with ITSELF (the `[1, 10]` defect at
        // another node number), because the first loop already consumed it.
        assert_eq!(
            upstream_vll(&[4, 10, 31, 32, 33, 41, 42, 43], false),
            pairs(&[(4, 4), (4, 4), (4, 4)])
        );
    }

    /// The port's own S-VLL / S-SEQ criteria are the phase-node SET, and each
    /// oracle's is its own node COUNT rule.
    #[test]
    fn the_port_and_the_two_channels_state_their_availability_differently() {
        assert_eq!(phase_nodes_present(&[1, 2, 3, 4, 10]), 3);
        assert_eq!(phase_nodes_present(&[1, 2, 10]), 2);
        assert_eq!(phase_nodes_present(&[10, 31, 32]), 0);

        assert_eq!(
            port_vll_pairs(&[1, 2, 3, 4]),
            Some(vec![(1, 2), (2, 3), (3, 1)])
        );
        assert_eq!(port_vll_pairs(&[1, 2, 10]), Some(vec![(1, 2)]));
        assert_eq!(port_vll_pairs(&[2, 3]), Some(vec![(2, 3)]));
        assert_eq!(port_vll_pairs(&[1, 10]), None);
        assert_eq!(port_vll_pairs(&[10, 11, 12]), None);

        // capi clamps `Nvalues > 3` and only declines below three nodes; r4133
        // does not clamp, so it declines on four.
        for n in [0, 1, 2] {
            assert!(seq_channel_declines(PropsChannel::CapiV0145, n));
            assert!(seq_channel_declines(PropsChannel::R4133, n));
        }
        assert!(!seq_channel_declines(PropsChannel::CapiV0145, 3));
        assert!(!seq_channel_declines(PropsChannel::R4133, 3));
        for n in [4, 5, 13] {
            assert!(!seq_channel_declines(PropsChannel::CapiV0145, n));
            assert!(seq_channel_declines(PropsChannel::R4133, n));
        }
    }

    /// The four run-wide populations fail on stale in BOTH directions, over
    /// injected counters (the shipped statics cannot be rewound once the gate
    /// has moved them — [`check_sc_study_compare_ran`]'s reason).
    ///
    /// [`check_sc_study_compare_ran`]: super::check_sc_study_compare_ran
    #[test]
    fn the_seq_vll_population_rules_fail_in_both_directions() {
        let good = SeqVllPopulation {
            r4133_seq_sentinel: R4133_SEQ_SENTINEL_POPULATION,
            seq_ground_substitution: SEQ_GROUND_SUBSTITUTION_POPULATION,
            vll_upstream_pairing_declines: VLL_UPSTREAM_PAIRING_DECLINES,
            r4133_vll_hang: R4133_VLL_HANG_POPULATION,
        };
        check_seq_vll_populations(good);

        let bump = |mut p: SeqVllPopulation, which: usize, by: i64| {
            let slot = match which {
                0 => &mut p.r4133_seq_sentinel,
                1 => &mut p.seq_ground_substitution,
                2 => &mut p.vll_upstream_pairing_declines,
                _ => &mut p.r4133_vll_hang,
            };
            slot.1 = (slot.1 as i64 + by) as usize;
            p
        };
        let names = [
            "r4133 sequence sentinel (n > 3)",
            "sequence ground substitution",
            "VLL upstream pairing declines",
            "r4133 VLL hang refusals",
        ];
        for (which, name) in names.iter().enumerate() {
            for by in [-1, 1] {
                let bad = bump(good, which, by);
                let msg = reds(|| check_seq_vll_populations(bad));
                assert!(
                    msg.contains(name),
                    "population `{name}` moved by {by} and the rule did not name it: {msg:?}"
                );
            }
        }
    }

    /// `C_012` is the analytic ceiling of the truncated-`sin 60°` gap, rounded
    /// up — not a measured band, and never wider than the derivation allows.
    /// The line-to-line base factor is the `√3` one, with the live `1.0` arm.
    #[test]
    fn the_seq_and_line_to_line_bands_are_the_documented_images() {
        let ceiling = 2.0 * (0.8660254037844387_f64 - 0.866025403) / 3.0;
        assert!(
            (ceiling - 5.2296e-10).abs() < 1e-14,
            "the ceiling moved: {ceiling:e}"
        );
        assert!(C_012 >= ceiling, "C_012 {C_012:e} is below its own ceiling");
        assert!(
            C_012 < 2.0 * ceiling,
            "C_012 {C_012:e} is not the ceiling rounded up"
        );
        // …and it really is the gap between the two transforms: the in-engine
        // pin measures 4.50e-10 relative on its own sample, inside the ceiling.
        let vph = [
            Complex64::new(7199.5578, 0.0),
            Complex64::new(-3599.7789, -6235.0),
            Complex64::new(-3599.7789, 6235.0),
        ];
        let (mut a, mut b) = ([Complex64::ZERO; 3], [Complex64::ZERO; 3]);
        SymComp::precise().phase_to_sym(&vph, &mut a);
        SymComp::official().phase_to_sym(&vph, &mut b);
        let max_vph = vph.iter().fold(0.0f64, |m, c| m.max(c.norm()));
        for k in 1..3 {
            let ratio = (a[k] - b[k]).norm() / max_vph;
            assert!(
                ratio <= C_012,
                "row {k} gap {ratio:e} exceeds C_012 {C_012:e}"
            );
        }
        // …and it is a CEILING, not one lucky sample: 2 000 pseudo-random phase
        // triples (a fixed LCG, so the sweep is deterministic) at feeder scale.
        // The worst ratio must stay under `C_012` and must actually approach it
        // — a sweep that never exercises the gap would make the constant
        // vacuous.
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut rnd = || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((seed >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        };
        let mut worst = 0.0f64;
        for _ in 0..2000 {
            let vph = [
                Complex64::new(7200.0 * rnd(), 7200.0 * rnd()),
                Complex64::new(7200.0 * rnd(), 7200.0 * rnd()),
                Complex64::new(7200.0 * rnd(), 7200.0 * rnd()),
            ];
            let (mut x, mut y) = ([Complex64::ZERO; 3], [Complex64::ZERO; 3]);
            SymComp::precise().phase_to_sym(&vph, &mut x);
            SymComp::official().phase_to_sym(&vph, &mut y);
            let scale = vph.iter().fold(0.0f64, |m, c| m.max(c.norm()));
            for k in 1..3 {
                worst = worst.max((x[k] - y[k]).norm() / scale);
            }
            // Row 0 takes no `C_012` term at all: the two variants' first row is
            // `[1/3, 1/3, 1/3]` to within the numerical `Invert`'s own ulps.
            assert!(
                (x[0] - y[0]).norm() / scale < 1e-15,
                "row 0 carries a truncation gap: {:e}",
                (x[0] - y[0]).norm() / scale
            );
        }
        assert!(worst <= C_012, "sweep worst {worst:e} exceeds C_012");
        assert!(
            worst > 4.0e-10,
            "the sweep never exercised the gap: {worst:e}"
        );

        assert_eq!(bus_ll_base_factor(0.0), 1.0);
        assert_eq!(bus_ll_base_factor(-1.0), 1.0);
        assert_eq!(bus_ll_base_factor(7.2), 1000.0 * 7.2 * 3.0f64.sqrt());
    }

    /// A deck that carries six of the ten bus classes at once: `b4` = `[1,2,3,4]`
    /// (the `(3,4)` pairing and, on r4133, the sequence sentinel), `bx` =
    /// `[1,2,10]` (the ground substitution and the duplicate negative), `by` =
    /// `[1,10]` (capi's `DefaultResult` vs r4133's self-pair), `bz` = `[2,3]`
    /// (one honest pair), `bh` = `[10,31,32]` (r4133's hang), `b1n` = `[1]` (the
    /// one-phase sentinel), and `sourcebus` = `[1,2,3]` (everything agrees).
    fn solved() -> Dss {
        let mut dss = Dss::new();
        for c in [
            "clear",
            "New circuit.seqvll basekv=12.47 pu=1.0 phases=3 bus1=sourcebus",
            "New Line.l1 bus1=sourcebus.1.2.3 bus2=b4.1.2.3 phases=3 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l1b bus1=sourcebus.1 bus2=b4.4 phases=1 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l2 bus1=sourcebus.1.2 bus2=bx.1.2 phases=2 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l2b bus1=sourcebus.3 bus2=bx.10 phases=1 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l3 bus1=sourcebus.1 bus2=by.1 phases=1 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l3b bus1=sourcebus.2 bus2=by.10 phases=1 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l4 bus1=sourcebus.2.3 bus2=bz.2.3 phases=2 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l5 bus1=sourcebus.1.2.3 bus2=bh.10.31.32 phases=3 r1=0.1 x1=0.3 c1=0 length=1",
            "New Line.l6 bus1=sourcebus.1 bus2=b1n.1 phases=1 r1=0.1 x1=0.3 c1=0 length=1",
            "New Load.ld bus1=b4.1.2.3 phases=3 kv=12.47 kw=500 pf=0.95",
            "Set voltagebases=[12.47]",
            "CalcVoltageBases",
            "Solve",
        ] {
            dss.command(c);
        }
        assert!(dss.errors().is_empty(), "{:?}", dss.errors());
        dss
    }

    /// Build the capture THAT channel's transport would send for this deck out
    /// of the port's own view, reproducing each transport's Pascal shape rule
    /// (the rules `oracle_server.py::capture_all_buses` and
    /// `dss-epri::capture::capture_all_buses` assert against live payloads).
    /// The G1.4a/G1.5 fields are left empty — they are the other comparators'
    /// rows and nothing here reads them.
    fn capture(dss: &Dss, channel: PropsChannel) -> Vec<BusCap> {
        dss.all_bus_voltages()
            .iter()
            .map(|v| {
                let mut nodes = v.nodes.clone();
                nodes.sort_unstable();
                let n = nodes.len();
                let (seq_voltages, cplx_seq_voltages) = if seq_channel_declines(channel, n) {
                    (vec![-1.0; 3], vec![-1.0; 6])
                } else {
                    let vph = [1i32, 2, 3].map(|k| bus_node_voltage(&nodes, &v.node_v, k));
                    let mut v012 = [Complex64::ZERO; 3];
                    // Each channel's OWN transform: r4133 builds `Ap2s` from the
                    // truncated `sin 60` and inverts it numerically, so the
                    // positive control below runs through the `C_012` term
                    // rather than assuming it (G1.4c audit-tests, `capture`).
                    let sc = match channel {
                        PropsChannel::CapiV0145 => SymComp::precise(),
                        PropsChannel::R4133 => SymComp::official(),
                    };
                    sc.phase_to_sym(&vph, &mut v012);
                    (v012.iter().map(|c| c.norm()).collect(), interleave(&v012))
                };
                let (vll, pu_vll, vll_declined) =
                    match upstream_vll(&nodes, channel == PropsChannel::CapiV0145) {
                        UpstreamVll::OnePhase => (vec![-99999.0, 0.0], vec![-99999.0, 0.0], false),
                        UpstreamVll::CapiDefault => (vec![0.0], vec![0.0], false),
                        UpstreamVll::SecondLoopHang => (Vec::new(), Vec::new(), true),
                        UpstreamVll::FirstLoopHang => unreachable!("no such bus in this deck"),
                        UpstreamVll::Pairs(p) => {
                            let ex = expected_vll(&nodes, &v.node_v, &p);
                            let base = bus_ll_base_factor(v.kv_base);
                            let pu: Vec<Complex64> = ex.iter().map(|c| *c / base).collect();
                            (interleave(&ex), interleave(&pu), false)
                        }
                    };
                BusCap {
                    name: v.name.clone(),
                    kv_base: v.kv_base,
                    distance: 0.0,
                    nodes,
                    pu_voltages: Vec::new(),
                    vmag_angle: Vec::new(),
                    pu_vmag_angle: Vec::new(),
                    zsc1: Vec::new(),
                    zsc0: Vec::new(),
                    zsc: Vec::new(),
                    ysc: Vec::new(),
                    isc: Vec::new(),
                    voc: Vec::new(),
                    seq_voltages,
                    cplx_seq_voltages,
                    vll,
                    pu_vll,
                    vll_declined,
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
    /// The silencing hook is scoped to this thread and delegates every other
    /// panic to the previous hook — `bus_short_circuit_tests::reds`' pattern,
    /// for its reason (this module compiles into the corpus-gate binary).
    fn reds<T: std::fmt::Debug>(f: impl FnOnce() -> T) -> String {
        thread_local! {
            static SILENCE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
        }
        static HOOK: std::sync::Once = std::sync::Once::new();
        HOOK.call_once(|| {
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                if !SILENCE.with(|s| s.get()) {
                    prev(info);
                }
            }));
        });
        SILENCE.with(|s| s.set(true));
        let out = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        SILENCE.with(|s| s.set(false));
        let err = out.expect_err("the drive was expected to red");
        err.downcast_ref::<String>()
            .cloned()
            .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    }

    /// Positive control on BOTH channels, with the classes the deck reaches
    /// pinned exactly — so every negative drive below perturbs a live rule and
    /// the class accounting itself is asserted, not narrated.
    #[test]
    fn the_comparator_accepts_the_engines_own_surface_and_names_every_class() {
        let dss = solved();
        let tol = tol_for("feeder");

        let capi = compare_bus_seq_and_vll(
            &dss,
            &capture(&dss, PropsChannel::CapiV0145),
            &tol,
            PropsChannel::CapiV0145,
            false,
            "positive control capi",
        );
        assert_eq!(
            capi,
            SeqVllCounts {
                buses: 7,
                // capi clamps, so it never declines the sequence pair here.
                r4133_seq_sentinel: 0,
                // `bx` = [1,2,10] and `bh` = [10,31,32]: n >= 3, a phase missing.
                seq_ground_substitution: 2,
                // `b4` (3,4), `bx` (2,1)+(10,1), `by` DefaultResult, `bh` DefaultResult.
                vll_upstream_pairing_declines: 4,
                r4133_vll_hang: 0,
            }
        );

        let r4133 = compare_bus_seq_and_vll(
            &dss,
            &capture(&dss, PropsChannel::R4133),
            &tol,
            PropsChannel::R4133,
            false,
            "positive control r4133",
        );
        assert_eq!(
            r4133,
            SeqVllCounts {
                buses: 7,
                // `b4` = [1,2,3,4]: r4133's missing clamp.
                r4133_seq_sentinel: 1,
                // `bh` still answers on capi but not on r4133 (n = 3 there, so
                // it IS the ground-substituted arm); `bx` likewise.
                seq_ground_substitution: 2,
                // `b4`, `bx`, `by` (the self-pair) — `bh` is the hang instead.
                vll_upstream_pairing_declines: 3,
                r4133_vll_hang: 1,
            }
        );
    }

    /// Non-vacuity, values: a sequence magnitude and a line-to-line entry moved
    /// by ten times their own band each red their own row.
    #[test]
    fn a_perturbed_sequence_or_line_to_line_value_reds_the_comparator() {
        let dss = solved();
        let tol = tol_for("feeder");

        let mut exp = capture(&dss, PropsChannel::CapiV0145);
        // 10x the band at |Vph| ~ 7.2 kV is ~7e-4 V — far below the value and
        // far above the floor, so this is the band talking, not a typo.
        let step = 10.0 * (tol.v_abs + tol.v_rel * 7200.0);
        bus_mut(&mut exp, "b4").seq_voltages[1] += step;
        let msg = reds(|| {
            compare_bus_seq_and_vll(
                &dss,
                &exp,
                &tol,
                PropsChannel::CapiV0145,
                false,
                "seq drive",
            )
        });
        assert!(msg.contains("SeqVoltages[1] differs"), "{msg:?}");

        let mut exp = capture(&dss, PropsChannel::CapiV0145);
        bus_mut(&mut exp, "b4").vll[4] += step;
        let msg = reds(|| {
            compare_bus_seq_and_vll(
                &dss,
                &exp,
                &tol,
                PropsChannel::CapiV0145,
                false,
                "vll drive",
            )
        });
        assert!(msg.contains("VLL[2] (nodes 3-4) differs"), "{msg:?}");

        // The same two arms on the OTHER channel, and the `puVLL` arm on both:
        // before the G1.4c audit settlement no drive anywhere reddened a `puVLL`
        // cell, and every `VLL` red on record was `[CapiV0145]`, so the §1.1(f)
        // acceptance was argued rather than measured there (audit-tests).
        // `puVLL` is the same difference over `BaseFactor_LL`, so the drive is
        // the same step divided by that base.
        for channel in [PropsChannel::CapiV0145, PropsChannel::R4133] {
            let mut exp = capture(&dss, channel);
            bus_mut(&mut exp, "b4").vll[4] += step;
            let msg =
                reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, channel, false, "vll drive"));
            assert!(msg.contains("VLL[2] (nodes 3-4) differs"), "{msg:?}");

            let mut exp = capture(&dss, channel);
            let b = bus_mut(&mut exp, "b4");
            let ll_base = bus_ll_base_factor(b.kv_base);
            assert!(ll_base > 1.0, "b4 must carry a line-to-line base");
            b.pu_vll[4] += step / ll_base;
            let msg =
                reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, channel, false, "pu drive"));
            assert!(msg.contains("puVLL[2] (nodes 3-4) differs"), "{msg:?}");
        }

        // The GROUND-SUBSTITUTION mechanism assertion: on `bx` = [1,2,10] the
        // port declines and the expectation is the replay over the fabricated
        // 0 V phase, so this is the only drive that reds THAT arm.
        let mut exp = capture(&dss, PropsChannel::CapiV0145);
        bus_mut(&mut exp, "bx").cplx_seq_voltages[2] += step;
        let msg = reds(|| {
            compare_bus_seq_and_vll(
                &dss,
                &exp,
                &tol,
                PropsChannel::CapiV0145,
                false,
                "ground substitution drive",
            )
        });
        assert!(msg.contains("CplxSeqVoltages[1] differs"), "{msg:?}");
    }

    /// Non-vacuity, the S-VLL pairing: swapping the walk's wrap order — the
    /// upstream defect itself — reds the `[1,2,3,4]` bus, because the port's own
    /// `(3,1)` answer is NOT what either oracle publishes there.
    #[test]
    fn the_wrap_before_probe_pairing_reds_the_four_node_bus() {
        let dss = solved();
        let view = dss
            .all_bus_voltages()
            .into_iter()
            .find(|v| v.name == "b4")
            .expect("b4");
        let mut exp = capture(&dss, PropsChannel::CapiV0145);
        // What upstream WOULD publish if it wrapped before probing (its own
        // `ShowResults.pas:193-194` and the commented-out `DBus.pas:586-587`):
        // the port's S-VLL answer.
        let wrap_first = interleave(view.vll.as_ref().expect("b4 carries three phases"));
        assert_ne!(wrap_first, bus_mut(&mut exp, "b4").vll);
        bus_mut(&mut exp, "b4").vll = wrap_first;
        let msg = reds(|| {
            compare_bus_seq_and_vll(
                &dss,
                &exp,
                &tol_for("feeder"),
                PropsChannel::CapiV0145,
                false,
                "wrap-order drive",
            )
        });
        assert!(msg.contains("VLL[2] (nodes 3-4) differs"), "{msg:?}");
    }

    /// Non-vacuity, availability: each of the four discrete classifications reds
    /// when the payload stops matching the class the node set names — including
    /// the two-transcription cross-check on `vll_declined`.
    #[test]
    fn a_payload_that_contradicts_its_own_bus_class_reds_the_comparator() {
        let dss = solved();
        let tol = tol_for("feeder");
        let capi = PropsChannel::CapiV0145;
        let r4133 = PropsChannel::R4133;

        // (1) a channel that stops answering where its own rule says it answers.
        let mut exp = capture(&dss, capi);
        bus_mut(&mut exp, "sourcebus").seq_voltages = vec![-1.0; 3];
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, capi, false, "seq answer"));
        assert!(msg.contains("carries a negative magnitude"), "{msg:?}");

        // (2) …and one that starts answering where it should decline.
        let mut exp = capture(&dss, r4133);
        bus_mut(&mut exp, "b4").seq_voltages = vec![1.0, 2.0, 3.0];
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, r4133, false, "seq decline"));
        assert!(msg.contains("n/A sentinel for SeqVoltages"), "{msg:?}");

        // (3) the bridge refuses a call the harness' replay resolves…
        let mut exp = capture(&dss, r4133);
        {
            let b = bus_mut(&mut exp, "b4");
            b.vll_declined = true;
            b.vll.clear();
            b.pu_vll.clear();
        }
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, r4133, false, "false refusal"));
        assert!(
            msg.contains("refused a VLL read this replay resolves"),
            "{msg:?}"
        );

        // (4) …and the converse: it answers where the replay says the loop hangs.
        let mut exp = capture(&dss, r4133);
        {
            let b = bus_mut(&mut exp, "bh");
            b.vll_declined = false;
            b.vll = vec![0.0; 6];
            b.pu_vll = vec![0.0; 6];
        }
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, r4133, false, "missed hang"));
        assert!(msg.contains("the two transcriptions disagree"), "{msg:?}");

        // (5) the capi transport can never decline — its scan is bounded.
        let mut exp = capture(&dss, capi);
        bus_mut(&mut exp, "bh").vll_declined = true;
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, capi, false, "capi refusal"));
        assert!(msg.contains("its partner scan is bounded"), "{msg:?}");

        // (6) the one-phase sentinel is asserted, not assumed.
        let mut exp = capture(&dss, capi);
        {
            let b = bus_mut(&mut exp, "b1n");
            b.vll = vec![0.0, 0.0];
            b.pu_vll = vec![0.0, 0.0];
        }
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, capi, false, "one phase"));
        assert!(msg.contains("published the 1-phase sentinel"), "{msg:?}");

        // (7) so is capi's `DefaultResult`.
        let mut exp = capture(&dss, capi);
        {
            let b = bus_mut(&mut exp, "by");
            b.vll = vec![0.0, 0.0];
            b.pu_vll = vec![0.0, 0.0];
        }
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, capi, false, "default result"));
        assert!(
            msg.contains("should have published `DefaultResult`"),
            "{msg:?}"
        );
    }

    /// `voltages_excluded` drops the VALUES only: every availability
    /// classification, every sentinel, every length and the `vll_declined`
    /// cross-check stay compared — the D11(2) narrowing, proven in both
    /// directions.
    #[test]
    fn the_voltage_exclusion_drops_values_and_keeps_the_structure() {
        let dss = solved();
        let tol = tol_for("feeder");
        let capi = PropsChannel::CapiV0145;
        let step = 10.0 * (tol.v_abs + tol.v_rel * 7200.0);

        // A value drive that reds unexcluded passes under the exclusion…
        let mut exp = capture(&dss, capi);
        {
            let b = bus_mut(&mut exp, "b4");
            b.seq_voltages[1] += step;
            b.vll[4] += step;
        }
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, capi, false, "not excluded"));
        assert!(msg.contains("differs"), "{msg:?}");
        let counts = compare_bus_seq_and_vll(&dss, &exp, &tol, capi, true, "excluded");
        assert_eq!(counts.buses, 7);
        // …and the class accounting is unchanged by the exclusion.
        assert_eq!(
            counts,
            compare_bus_seq_and_vll(&dss, &capture(&dss, capi), &tol, capi, false, "control")
        );

        // …while a length, a sentinel and the availability rule still red.
        let mut exp = capture(&dss, capi);
        bus_mut(&mut exp, "b4").vll.pop();
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, capi, true, "length"));
        assert!(msg.contains("VLL and puVLL lengths differ"), "{msg:?}");

        let mut exp = capture(&dss, capi);
        bus_mut(&mut exp, "sourcebus").seq_voltages = vec![-1.0; 3];
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, capi, true, "sentinel"));
        assert!(msg.contains("carries a negative magnitude"), "{msg:?}");

        let mut exp = capture(&dss, capi);
        bus_mut(&mut exp, "b1n").vll = vec![-99999.0, 0.0, 0.0, 0.0];
        bus_mut(&mut exp, "b1n").pu_vll = vec![-99999.0, 0.0, 0.0, 0.0];
        let msg = reds(|| compare_bus_seq_and_vll(&dss, &exp, &tol, capi, true, "one-phase shape"));
        assert!(msg.contains("published the 1-phase sentinel"), "{msg:?}");
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

// ---------------------------------------------------------------------------
// GOLDEN_REBASE G1.6(i): the `Meters` reliability surface.
//
// The dss-python `ActiveCircuit.Meters` fields the fastdss harness archives
// once per case (`DSS-Python@origin/fastdss:dss/IMeters.py:13-42` `_columns`,
// written by `tests/save_outputs.py:283-291` + `:330-332`), captured live on
// both channels (`tools/oracle/oracle_server.py::capture_reliability`,
// `crates/dss-epri/src/capture.rs::capture_reliability`) and compared against
// `Dss::meter_reliability` + `Dss::meter_totals`.
//
// It is the one surface whose capture DRIVES an executive command: nothing in
// the corpus runs `RelCalc` (`Executive/ExecCommands.pas:154` ->
// `TExecHelper.DoLambdaCalcs`, r4133 `Executive/ExecHelper.pas:4404-4440`), so
// every reliability field would compare `0 == 0`. All three engines run it once
// per case, on the LAST step, right after the solve reply is read — see
// `corpus_gate/runner.rs` for the drive and the two transports for the read
// order (`SetActiveSection` before every section field; `Meters.Totals` last,
// because `TotalizeMeters` destroys the meter cursor).
// ---------------------------------------------------------------------------

/// One feeder section of one meter, exactly as **both** channels serialize it
/// (`oracle_server.capture_reliability`'s section dict == `dss-epri`'s
/// `FeederSectionCap`), read behind its own `Meters.SetActiveSection`.
///
/// No `#[serde(default)]` anywhere: a channel that drops a field must fail
/// loudly here rather than compare a zero.
#[derive(Debug, Clone, Deserialize)]
pub struct FeederSectionCap {
    /// The 1-based `SetActiveSection` argument this row was read at.
    pub idx: i32,
    pub num_section_customers: i32,
    pub num_section_branches: i32,
    pub sect_seq_idx: i32,
    pub sect_total_cust: i32,
    /// 1 = Fuse, 2 = Recloser, 3 = Relay; 0 = none.
    pub ocp_device_type: i32,
    pub sum_branch_flt_rates: f64,
    /// `SumFltRatesXRepairHrs / SumBranchFltRates` — an **unguarded** division
    /// on all three engines, so a section whose branches all carry
    /// `faultrate=0` is `0/0 = NaN` identically everywhere (r4133
    /// `Version8/Source/Meters/EnergyMeter.pas` `AverageRepairTime`; port
    /// `solution/meters/reliability.rs:259`). [`rel_num_eq`] is what makes that
    /// agreement, and only that agreement, pass.
    pub avg_repair_time: f64,
    pub fault_rate_x_repair_hrs: f64,
}

/// One meter's reliability record as both channels serialize it.
///
/// `caidi` is deliberately **absent**: neither channel has a `CAIDI` API mode,
/// so no capture can carry it. It still reaches the live gate — it is
/// EnergyMeter property `CAIDI` (index 22 of `AllPropertyNames`), which
/// [`compare_all_properties`] compares on both channels since `R4133_PROPS`
/// RP4.1, and it moves `0 -> 3` on `modes:time/midi_duty_ctrl.dss` the moment
/// `RelCalc` runs. It is therefore compared, never "not comparable".
#[derive(Debug, Clone, Deserialize)]
pub struct MeterReliabilityCap {
    pub name: String,
    pub total_customers: i32,
    pub saifi: f64,
    /// The API spells it `SAIFIkW`; the wire key is `saifikw` and the Rust
    /// spelling matches `MeterReliabilityView::saifi_kw` so one macro can
    /// extract both sides.
    #[serde(rename = "saifikw")]
    pub saifi_kw: f64,
    pub saidi: f64,
    pub cust_interrupts: f64,
    /// `|CalculatedCurrent[k]|`, `k` in `0..NPhases` — see
    /// [`RELIABILITY_SKIP_FIELDS`] for why it is not compared until an
    /// `AllocateLoads` has run.
    pub calc_current: Vec<f64>,
    /// `PhsAllocationFactor[0..NPhases]`; same caveat.
    pub alloc_factors: Vec<f64>,
    /// The three zone lists in the oracles' own `BranchList` walk order.
    pub branches: Vec<String>,
    pub ends: Vec<String>,
    pub pce: Vec<String>,
    pub num_sections: i32,
    pub sections: Vec<FeederSectionCap>,
}

/// One channel's whole reliability payload for the LAST checkpoint of a case.
#[derive(Debug, Clone, Deserialize)]
pub struct ReliabilityCap {
    /// The channel's `RelCalc` hit the by-design "no OCP device in the zone"
    /// abort (errno 52902, `Meters/EnergyMeter.pas:2502`).
    pub aborted: bool,
    /// The abort message verbatim; `""` when it did not abort.
    pub message: String,
    /// Every **enabled** meter, in `Meters.First`/`Next` order.
    pub meters: Vec<MeterReliabilityCap>,
    /// `Meters.Totals` — the masked register sum over every meter of the
    /// circuit list (`TotalizeMeters`), read LAST on both transports.
    pub totals: Vec<f64>,
}

/// What the *port's* `RelCalc` did, as the runner observed it: the new
/// `Dss::errors()` entries the command pushed.
///
/// It travels as a parameter rather than being derived inside
/// [`compare_reliability`] because only the caller knows the error baseline —
/// `Dss::errors()` is cumulative for the whole case.
///
/// The two sides are compared as **boolean + message**, never as a count: the
/// port reports one error per failing meter
/// (`solution/meters/reliability.rs:44-50` collects per meter) while
/// dss-python raises once per command, so on a multi-meter deck the counts
/// legitimately differ. The text is byte-identical on all three engines
/// (measured on `controls:energymeter/midi_energymeter.dss`).
///
/// The lines beyond the first are **kept**, not discarded (G1.6(i) audit
/// settlement, finding B/3): the drive is the LAST thing the runner does on
/// the last step, so the per-step `Dss::errors()` baseline assert
/// (`corpus_gate/runner.rs`) never runs again after it, and a second,
/// *different* error raised inside `RelCalc` would otherwise be invisible.
/// [`compare_reliability`] asserts every line carries the one tolerated abort
/// text before it compares the message to the oracle's.
#[derive(Debug, Clone, Default)]
pub struct RelCalcOutcome {
    pub aborted: bool,
    pub message: String,
    /// Every error line the command appended, in order (`message` is the
    /// first). Compared for uniformity, never for its count.
    pub messages: Vec<String>,
}

impl RelCalcOutcome {
    /// Build the outcome from the error lines `RelCalc` appended.
    pub fn from_new_errors(new: &[String]) -> Self {
        Self {
            aborted: !new.is_empty(),
            message: new.first().cloned().unwrap_or_default(),
            messages: new.to_vec(),
        }
    }
}

/// Equality as this surface defines it: **exact** (`rel = abs = 0`), with the
/// non-finite agreement rule.
///
/// Exactness is not an aspiration: on `modes:time/midi_duty_ctrl.dss` the two
/// independent oracle engines return bit-identical doubles for every
/// reliability number (`SAIFI 0.05600000000000001`, `SAIDI
/// 0.16799999999999998`, `CustInterrupts 0.11200000000000002`,
/// `SumBranchFltRates 0.0031360000000000008`, `AvgRepairTime
/// 2.999999999999999`, `FaultRateXRepairHrs 0.009408`). Two engines agreeing
/// to the last bit means the arithmetic is order-identical — sums over integer
/// customer counts and deck literals in a fixed zone-walk order — so a Rust gap
/// is an **order bug**, not a floor (`tests/TOLERANCE_NOTES.md`).
///
/// G1.6(i) micro-part **F2s** re-measured that claim where it looked broken: the
/// 15 cells the live gate reported as 1-ULP Rust↔oracle gaps (9 distinct values,
/// `accumulated_l` / `total_miles` / `cust_interrupts` / `saifikw` /
/// `sum_branch_flt_rates`, on BOTH channels) are **not** arithmetic at all. The
/// raw JSON number token each oracle puts on the wire round-trips to the port's
/// f64 bit-for-bit — 15/15, measured at the wire on both transports — and it
/// is the gate's own decoder that
/// loses the last bit: `serde_json` without the `float_roundtrip` feature parses
/// a 17-significant-digit token as `significand as f64` then one multiply or
/// divide by a power of ten, i.e. two roundings. That is exactly the defect
/// coordinator decision **D11** fixes workspace-wide (`serde_json` +
/// `float_roundtrip`); **D18** landed the identical hunk on this branch, so the
/// gate already decodes every oracle float exactly and the artifact is gone.
/// It was a transport artifact throughout, never a port or oracle value.
/// So this surface stays exact and gains no floor — derivation in
/// `tests/TOLERANCE_NOTES.md` §"The `Meters` reliability surface (G1.6(i))",
/// both numbers pinned by
/// `dss_core::exec::tests::reliability::reliability_accumulators_are_correctly_rounded_f64_sums`.
///
/// `NaN == NaN` and `+inf == +inf` / `-inf == -inf` count as agreement because
/// they are the *same* unguarded arithmetic on both sides
/// ([`FeederSectionCap::avg_repair_time`]); `NaN` against a finite number, and
/// `+inf` against `-inf`, are failures.
fn rel_num_eq(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// One reliability field's value, type-tagged so the comparator can walk the
/// meter scalars and the section fields uniformly (one message shape, one
/// accounting site).
#[derive(Debug, Clone, Copy, PartialEq)]
enum RelVal {
    F(f64),
    I(i32),
}

impl RelVal {
    fn matches(self, other: Self) -> bool {
        match (self, other) {
            (RelVal::F(a), RelVal::F(b)) => rel_num_eq(a, b),
            (RelVal::I(a), RelVal::I(b)) => a == b,
            _ => false,
        }
    }

    /// Full-precision rendering for a failure message (`{:?}` on `f64` is the
    /// shortest round-tripping decimal, so a denormal reads as `2.8e-309`
    /// rather than `0`).
    fn render(self) -> String {
        match self {
            RelVal::F(x) => format!("{x:?}"),
            RelVal::I(x) => x.to_string(),
        }
    }
}

/// The per-meter scalar fields, in the frozen order the capture reads them.
/// A macro rather than two functions so it expands over **both**
/// [`MeterReliabilityCap`] and `dss_core`'s `MeterReliabilityView` — a field
/// renamed on either type stops this file compiling.
macro_rules! rel_meter_scalars {
    ($x:expr) => {{
        let m = $x;
        [
            ("total_customers", RelVal::I(m.total_customers)),
            ("saifi", RelVal::F(m.saifi)),
            ("saifi_kw", RelVal::F(m.saifi_kw)),
            ("saidi", RelVal::F(m.saidi)),
            ("cust_interrupts", RelVal::F(m.cust_interrupts)),
            ("num_sections", RelVal::I(m.num_sections)),
        ]
    }};
}

/// The nine fields of one feeder section, in read order; expands over both
/// [`FeederSectionCap`] and `dss_core`'s `FeederSectionView`.
macro_rules! rel_section_fields {
    ($x:expr) => {{
        let s = $x;
        [
            ("idx", RelVal::I(s.idx)),
            ("num_section_customers", RelVal::I(s.num_section_customers)),
            ("num_section_branches", RelVal::I(s.num_section_branches)),
            ("sect_seq_idx", RelVal::I(s.sect_seq_idx)),
            ("sect_total_cust", RelVal::I(s.sect_total_cust)),
            ("ocp_device_type", RelVal::I(s.ocp_device_type)),
            ("sum_branch_flt_rates", RelVal::F(s.sum_branch_flt_rates)),
            ("avg_repair_time", RelVal::F(s.avg_repair_time)),
            (
                "fault_rate_x_repair_hrs",
                RelVal::F(s.fault_rate_x_repair_hrs),
            ),
        ]
    }};
}

/// The two per-phase array fields [`RELIABILITY_SKIP_FIELDS`] may name — the
/// register test's vocabulary, so a row naming a field that does not exist is
/// refused.
const RELIABILITY_ARRAY_FIELDS: [&str; 2] = ["calc_current", "alloc_factors"];

/// One `Meters` array field an oracle channel reads out of **uninitialized
/// memory**, excluded field-by-field with its pin. See
/// [`RELIABILITY_SKIP_FIELDS`] for the mechanism and the measurements.
pub struct ReliabilitySkipRow {
    /// [`PropsChannel::tag`] of the channel whose value is garbage.
    pub channel: &'static str,
    /// The [`RELIABILITY_ARRAY_FIELDS`] key this row drops on that channel.
    pub field: &'static str,
    /// The expected-value test that pins the port's correct value against the
    /// measured garbage (`GOLDEN_REBASE_PLAN.md` §1.1(e)).
    pub pin: &'static str,
    /// The upstream source line the defect lives on.
    pub cite: &'static str,
}

/// **The `Meters` cells no oracle channel can be compared on until the deck has
/// run `AllocateLoads`** — a proven uninitialized read, present on BOTH
/// oracles.
///
/// `TMeterElement.AllocateSensorArrays` `ReallocMem`s `PhsAllocationFactor` and
/// `CalculatedCurrent` **without zeroing them** (r4133
/// `Version8/Source/Meters/MeterElement.pas:45-52`; capi 0.14.5 carries the
/// identical code), and the only thing that ever writes them is
/// `TMeterElement.CalcAllocationFactors` (`:54-72`), whose sole driver is
/// `TExecHelper.DoAllocateLoadsCmd` (r4133
/// `Version8/Source/Executive/ExecHelper.pas:2624-2683`). **No vendored corpus
/// deck runs `AllocateLoads`**, so on every live case but one `Meters.CalcCurrent`
/// and `Meters.AllocFactors` answer from whatever was on the heap. The port
/// zero-initializes both arrays (`elements/meter/meter_element.rs:106,111`), so
/// it is right and upstream is undefined.
///
/// The one exception is `controls:energymeter/midi_relcalc.dss`, added by
/// G1.6(i) part F3 for exactly this reason: it ends in `AllocateLoads`, so both
/// fields are defined there and compared on **both** channels with no exclusion
/// at all — see the Scope paragraph below and
/// `crates/dss-core/tests/reliability_pins.rs`.
///
/// **Measured** (G1.6(i) part R, three fresh `epri-worker` processes): on
/// `controls:combo/combo_protection.dss` r4133 `Meters.AllocFactors` came back
/// `[2.806806272625585e-309, 2.121995791e-314, 2.37e-322]` in run 1 and
/// `[…, …, 2.4e-322]` in runs 2-3 — denormal garbage that **changes across
/// processes**. An envelope over such a value is not a fact, so this is a
/// harness exclusion, not a `tests/corpus/ledger.json` row (the
/// [`PD_SKIP_FIELDS`] precedent and coordinator decision D4).
///
/// **Scope — (channel, field) AND the port's own regime.** A row is consulted
/// only where `CalcAllocationFactors` has demonstrably not run for that meter,
/// which [`reliability_skip_applies`] reads off the PORT: after that call every
/// `PhsAllocationFactor[i]` is `SensorCurrent[i]/|I_i|` or, when the current is
/// zero, exactly `1.0` (`meter_element.rs:117-136`, r4133
/// `MeterElement.pas:54-72`), and the EnergyMeter constructor seeds
/// `SensorCurrent := 400 A` (`elements/meter/energymeter/mod.rs:328`) — so an
/// all-zero `alloc_factors` on the port means the writer never ran. The moment
/// a deck runs `AllocateLoads`, both fields are compared on both channels with
/// no exclusion at all.
///
/// Honest limit, stated the way [`PD_SKIP_FIELDS`]' is: the predicate is the
/// port's own state, so a port that *failed* to run an allocation it should
/// have run would suppress these two arrays instead of reding. That is what the
/// `AllocateLoads` deck and its pin exist for — on
/// `controls:energymeter/midi_relcalc.dss` the port's factors are non-zero, the
/// predicate is false, and both fields are compared live on both channels
/// (measured: 0 skip visits on that case, 25 on the rest of the flagged
/// population). Measured non-vacuity, G1.6(i) F3: perturbing the port's
/// `calc_current` or `alloc_factors` by 1e-6 relative reds that case on
/// `capi_v0145` **and** on `r4133` — four scratch drives, restored
/// byte-identically.
pub const RELIABILITY_SKIP_FIELDS: &[ReliabilitySkipRow] = &[
    ReliabilitySkipRow {
        channel: "capi_v0145",
        field: "calc_current",
        pin: "meter_alloc_factors_are_zero_until_allocateloads_runs",
        cite: "dss_capi/src/Meters/MeterElement.pas AllocateSensorArrays (ReallocMem, \
                no zeroing) + to_opendss/62-metered-sensor-arrays-are-never-initialised.md",
    },
    ReliabilitySkipRow {
        channel: "capi_v0145",
        field: "alloc_factors",
        pin: "meter_alloc_factors_are_zero_until_allocateloads_runs",
        cite: "dss_capi/src/Meters/MeterElement.pas AllocateSensorArrays (ReallocMem, \
                no zeroing) + to_opendss/62-metered-sensor-arrays-are-never-initialised.md",
    },
    ReliabilitySkipRow {
        channel: "r4133",
        field: "calc_current",
        pin: "meter_alloc_factors_are_zero_until_allocateloads_runs",
        cite: "Version8/Source/Meters/MeterElement.pas:45-52 + \
                to_opendss/62-metered-sensor-arrays-are-never-initialised.md",
    },
    ReliabilitySkipRow {
        channel: "r4133",
        field: "alloc_factors",
        pin: "meter_alloc_factors_are_zero_until_allocateloads_runs",
        cite: "Version8/Source/Meters/MeterElement.pas:45-52 + \
                to_opendss/62-metered-sensor-arrays-are-never-initialised.md",
    },
];

/// Per-row visit counter, indexed exactly like [`RELIABILITY_SKIP_FIELDS`]:
/// cells the row was consulted about.
static RELIABILITY_SKIP_VISITS: [AtomicUsize; RELIABILITY_SKIP_FIELDS.len()] =
    [const { AtomicUsize::new(0) }; RELIABILITY_SKIP_FIELDS.len()];
/// Per-row hit counter: visits whose two sides differed by more than the
/// field's own band ([`reliability_array_band`]) — i.e. visits where the row
/// was load-bearing.
static RELIABILITY_SKIP_HITS: [AtomicUsize; RELIABILITY_SKIP_FIELDS.len()] =
    [const { AtomicUsize::new(0) }; RELIABILITY_SKIP_FIELDS.len()];

/// Gating reliability payloads this process compared, per channel, indexed by
/// [`pd_channel_slot`].
static RELIABILITY_WALKS: [AtomicUsize; 2] = [const { AtomicUsize::new(0) }; 2];
/// …and the meters those payloads compared.
static RELIABILITY_METERS: [AtomicUsize; 2] = [const { AtomicUsize::new(0) }; 2];

/// Which [`RELIABILITY_SKIP_FIELDS`] row covers `(channel, field)`, if any.
///
/// A pure function so both directions are provable offline — the shipped
/// counters cannot be rewound once a gate run has moved them.
fn reliability_skip_row(channel: &str, field: &str) -> Option<usize> {
    RELIABILITY_SKIP_FIELDS
        .iter()
        .position(|r| r.channel == channel && r.field == field)
}

/// Whether [`RELIABILITY_SKIP_FIELDS`] may speak about this meter at all: the
/// port's allocation factors are still all exactly zero, i.e.
/// `CalcAllocationFactors` never ran (see the table's doc for why that is an
/// iff on the port side).
fn reliability_skip_applies(a: &MeterReliabilityView) -> bool {
    a.alloc_factors.iter().all(|x| *x == 0.0)
}

/// What [`compare_reliability`] has counted in this process, as
/// `(capi payloads, capi meters, r4133 payloads, r4133 meters)`.
pub fn reliability_walk_counters() -> (usize, usize, usize, usize) {
    (
        RELIABILITY_WALKS[0].load(AtomicOrd::Relaxed),
        RELIABILITY_METERS[0].load(AtomicOrd::Relaxed),
        RELIABILITY_WALKS[1].load(AtomicOrd::Relaxed),
        RELIABILITY_METERS[1].load(AtomicOrd::Relaxed),
    )
}

/// One [`RELIABILITY_SKIP_FIELDS`] row's live `(visits, hits)`, or `None` when
/// the table has no such row.
pub fn reliability_skip_counters(channel: &str, field: &str) -> Option<(usize, usize)> {
    let i = reliability_skip_row(channel, field)?;
    Some((
        RELIABILITY_SKIP_VISITS[i].load(AtomicOrd::Relaxed),
        RELIABILITY_SKIP_HITS[i].load(AtomicOrd::Relaxed),
    ))
}

/// The comparison band for the two per-phase `Meters` arrays — the ONLY
/// non-exact cells of the reliability surface, and the existing **current**
/// tier rather than a new floor. Returns `0.0` (i.e. exact) for every other
/// field name and for the cases where both engines take an exact branch.
///
/// Every other number this surface reports is integer customer counts and deck
/// literals summed in a fixed zone-walk order, which is why [`rel_num_eq`] is
/// exact. These two are not:
///
/// * `Meters.CalcCurrent[k]` is `Cabs(CalculatedCurrent[k])` — the metered
///   element's own terminal current at the solve `CalcAllocationFactors` ran on
///   (r4133 `Version8/Source/Meters/MeterElement.pas:54-72`; capi reads the
///   stored array, `CAPI_Meters.pas:335-350`). It is the same `|GetCurrents|`
///   quantity [`compare_element`] gates at `tol.i_rel`/`i_abs`, so it inherits
///   that tier verbatim — nothing here widens anything.
/// * `Meters.AllocFactors[k]` is `SensorCurrent[k] / Cabs(CalculatedCurrent[k])`
///   with `SensorCurrent` a deck literal (`peakcurrent=`, else the constructor's
///   400 A), so its band is the **image** of the current band under that
///   division and not a band of its own: `|Δf|/|f| = |Δ|I||/|I| ≤ i_rel +
///   i_abs/|I|`. `i_ref` is the larger of the two engines' `|I|` for the same
///   phase, so the propagated band is the tighter, not the looser, reading.
///   When both are exactly `0.0` the Pascal takes its `ELSE
///   PhsAllocationFactor^[i] := 1.0` branch (`MeterElement.pas:68`) on both
///   sides and there is nothing to propagate: the band collapses to `0.0` and
///   the compare is exact again.
///
/// **The denominator is band-limited from below**, the way the `SeqCurrents
/// %I…` rule this derivation cites is (`tests/TOLERANCE_NOTES.md`,
/// `ColTol::gate`): the image band `|f|·(i_rel + i_abs/|I|)` is only a band
/// while `|I|` is distinguishable from zero. Once `|I| < i_abs` the term
/// `i_abs/|I|` exceeds 1 and the "band" admits the whole value — the compare
/// would go silently vacuous with no counter and no message, which is exactly
/// the failure the `SeqCurrents` note calls load-bearing. So this returns
/// `None` there and [`compare_reliability`] turns it into a **loud** triage
/// failure instead of a pass; `i_ref == 0.0` on both sides stays the exact
/// arm. G1.6(i) audit settlement (finding A/4).
///
/// Measured on the one deck where the two fields are defined at all
/// (`controls:energymeter/midi_relcalc.dss`, G1.6(i) part F3): the two
/// **independent oracle engines** differ from each other by up to `3.30e-14`
/// relative on `calc_current` (`115.69585353499478` capi vs
/// `115.69585353499097` r4133) and `3.30e-14` on `alloc_factors`
/// (`1.0372022534386103` vs `1.0372022534386445`), while every scalar, section
/// field and `Meters.Totals` slot of the same payload is bit-identical. Two
/// KLU-based engines disagreeing at that scale is the proof that these cells
/// are solve-derived; the micro tier (`i_rel = 1e-9`, `i_abs = 1e-6`) leaves
/// ~4-5 orders of headroom. Derivation in `tests/TOLERANCE_NOTES.md`.
fn reliability_array_band(
    field: &str,
    oracle: f64,
    port_current: f64,
    oracle_current: f64,
    tol: &Tolerances,
) -> Option<f64> {
    match field {
        "calc_current" => Some(tol.i_abs + tol.i_rel * oracle.abs()),
        // A non-finite current on either side: `f64::max` would quietly
        // ignore the NaN and band the cell off the other engine's current,
        // so rule it out before the denominator is formed.
        "alloc_factors" if !port_current.is_finite() || !oracle_current.is_finite() => None,
        "alloc_factors" => {
            let i_ref = port_current.abs().max(oracle_current.abs());
            if i_ref == 0.0 {
                // Both engines took the Pascal's `ELSE … := 1.0` branch: no
                // division happened on either side, so nothing propagates.
                Some(0.0)
            } else if i_ref >= tol.i_abs {
                Some(oracle.abs() * (tol.i_rel + tol.i_abs / i_ref))
            } else {
                // Denominator inside its own absolute band: `S/|I|` is a
                // noise/noise form with no derivable band.
                None
            }
        }
        _ => Some(0.0),
    }
}

/// Compare one channel's reliability payload against the engine's
/// (`GOLDEN_REBASE_PLAN.md` §G1.6, sub-step (i)).
///
/// Order of assertions: the `RelCalc` outcome first (a channel that aborted
/// where the port did not has nothing worth comparing below it), then the meter
/// walk — length and name **sequence**, case-insensitively, because both
/// oracles' `Meters.First`/`Next` skips disabled meters and visits the
/// `EnergyMeters` pointer list in creation order (r4133 `DDLL/DMeters.pas:32-71`
/// loops `If pMeter.Enabled`; capi `CAPI/CAPI_Meters.pas:153-167` routes through
/// `Generic_CktElement_Get_First`/`_Next`), which is exactly what
/// `Dss::meter_reliability` walks — then every field, then `Meters.Totals`.
///
/// Every *reliability* value is compared **exactly** — see [`rel_num_eq`] for
/// the measured justification and for the NaN/±inf rule. The exceptions are the
/// three cells that are not reliability arithmetic at all, and they are why
/// this function takes a [`Tolerances`]: `Meters.Totals`, the masked sum of the
/// very energy registers [`compare_meter`] compares at
/// `tol.energy_rel`/`energy_abs` (coordinator decision **D17a**), and the two
/// per-phase arrays `Meters.CalcCurrent`/`Meters.AllocFactors`, which are
/// functions of the metered element's terminal currents and therefore carry the
/// current tier — see [`reliability_array_band`]. Both inherit an existing tier
/// on the quantity they are made of; neither is a new band. Derivations in
/// `tests/TOLERANCE_NOTES.md` §"The `Meters` reliability surface (G1.6(i))".
///
/// The three zone lists are compared here **in order** — length, then
/// case-insensitive membership, then element-by-element sequence. Both
/// oracles emit them in their own zone-walk order (capi
/// `CAPI/CAPI_Meters.pas:589-595` and r4133
/// `Version8/Source/DDLL/DMeters.pas:706-733` walk
/// `BranchList.First`/`GoForward` for `AllBranchesInZone`, `:682-705` for
/// `AllEndElements`, `:735-758` emits `GetPCEatZone`'s own array), and the
/// port returns `sequence_list()`, pushed by the same tree walk
/// (`solution/meters/zones/build.rs:194`) — so the sequences are expected to
/// agree, and G1.6(i) measured that they do: bit-identical on every flagged
/// case, on both channels (part F2 §3, re-run and extended to the sixth case
/// in part F4). The ordered arm is therefore **unconditional**, not per-kind.
/// [`compare_meter`]'s deliberate order-independence is untouched: it compares
/// the same three lists as sets for the reason its own doc gives, and this
/// surface layers the stronger contract on top rather than changing it.
/// Comparing them here is not redundant with [`compare_meter`] either: this
/// surface is flagged independently of `check_meters_monitors`, and
/// `modes:makeposseq/makeposseq_ctrl.dss` gates it without the meter capture.
///
/// The only cells not compared are [`RELIABILITY_SKIP_FIELDS`]', which the
/// channel reads out of uninitialized memory; each one is still *visited* and
/// accounted, plus whatever `ledger_skip` names.
///
/// `ledger_skip` is the per-VALUE `tests/corpus/ledger.json` hook, threaded as a
/// closure for the reason `compare_variables`' is: the harness compiles into ~20
/// test binaries and must not know the ledger. Its key is the lowercased
/// `<meter>:<field>` pair for every per-meter value (`em:saidi`,
/// `em:sum_branch_flt_rates` — a section field is named once and dropped for
/// every section of that meter) and the bare `totals` for the circuit-level
/// array. It can only ever drop a VALUE compare: the walk length, the meter
/// names, the section count and the array lengths are asserted before it is
/// consulted, so an exclusion cannot hide a missing meter, section or phase.
/// **No entry exists today** (G1.6(i) measured zero divergences on either
/// channel); the hook is wired so a future measured one has somewhere to go
/// that `LEDGER_FIELDS` can police.
pub fn compare_reliability(
    dss: &Dss,
    exp: &ReliabilityCap,
    rust: &RelCalcOutcome,
    channel: PropsChannel,
    ctx: &str,
    tol: &Tolerances,
    ledger_skip: &dyn Fn(&str) -> bool,
) {
    let tag = channel.tag();
    assert_eq!(
        rust.aborted, exp.aborted,
        "{ctx}: `RelCalc` abort differs against `{tag}`: Rust aborted={} ({:?}) vs oracle \
         aborted={} ({:?}). The abort is the by-design errno 52902 \
         (`Meters/EnergyMeter.pas:2502`) raised when a meter's zone holds no OCP device; both \
         engines run the command at the same point, so an asymmetric abort is a port bug.",
        rust.aborted, rust.message, exp.aborted, exp.message,
    );
    if exp.aborted {
        // Every line the port's `RelCalc` appended, not just the first: this is
        // the last drive of the case, so no later `Dss::errors()` baseline
        // assert would see a second, different error (audit settlement B/3).
        if let Some(other) = rust
            .messages
            .iter()
            .find(|m| m.trim() != rust.message.trim())
        {
            panic!(
                "{ctx}: `RelCalc` appended {} error line(s) and they are not all the one \
                 tolerated abort: first {:?}, also {:?}. Only errno 52902 (no OCP device \
                 in the zone, one line per failing meter) is by design here; a second, \
                 different error out of the same command is a real failure and must not \
                 be swallowed.",
                rust.messages.len(),
                rust.message,
                other,
            );
        }
        assert_eq!(
            rust.message.trim(),
            exp.message.trim(),
            "{ctx}: `RelCalc` abort message differs against `{tag}` (the text is byte-identical \
             on all three engines — capi, r4133 and the port's own \
             `solution/meters/reliability.rs`)"
        );
    }

    let act = dss.meter_reliability();
    assert_eq!(
        act.len(),
        exp.meters.len(),
        "{ctx}: reliability meter walk length differs against `{tag}`: Rust {} vs oracle {} \
         (Rust {:?}, oracle {:?}). The walk is the ENABLED EnergyMeter list in creation order.",
        act.len(),
        exp.meters.len(),
        act.iter().map(|m| &m.name).collect::<Vec<_>>(),
        exp.meters.iter().map(|m| &m.name).collect::<Vec<_>>(),
    );
    if let Some(k) = act
        .iter()
        .zip(&exp.meters)
        .position(|(a, e)| !a.name.eq_ignore_ascii_case(&e.name))
    {
        panic!(
            "{ctx}: reliability meter walk differs against `{tag}` at index {k}: Rust `{}` vs \
             oracle `{}` (membership or order)",
            act[k].name, exp.meters[k].name,
        );
    }

    for (a, e) in act.iter().zip(&exp.meters) {
        let name_lc = e.name.to_lowercase();
        let dropped = |field: &str| ledger_skip(&format!("{name_lc}:{field}"));
        let av = rel_meter_scalars!(a);
        let ev = rel_meter_scalars!(e);
        for ((fa, va), (fe, ve)) in av.iter().zip(ev.iter()) {
            debug_assert_eq!(fa, fe, "both extractions are the one macro");
            if dropped(fa) {
                continue;
            }
            assert!(
                va.matches(*ve),
                "{ctx}: meter {} reliability field `{fa}` differs against `{tag}`: Rust {} vs \
                 oracle {}. This surface is compared EXACTLY (rel = abs = 0): the two oracle \
                 engines are bit-identical on it, so a difference is a bug, never a floor.",
                e.name,
                va.render(),
                ve.render(),
            );
        }

        // The section rows. `num_sections` was compared above, so a payload
        // whose `sections` array disagrees with its own count is a transport
        // fault, not a divergence — say so separately.
        assert_eq!(
            e.sections.len(),
            e.num_sections.max(0) as usize,
            "{ctx}: meter {} `{tag}` payload carries {} section(s) for NumSections {} — the \
             capture must read every `1..=NumSections`",
            e.name,
            e.sections.len(),
            e.num_sections,
        );
        assert_eq!(
            a.sections.len(),
            e.sections.len(),
            "{ctx}: meter {} section count differs against `{tag}`: Rust {} vs oracle {}",
            e.name,
            a.sections.len(),
            e.sections.len(),
        );
        for (sa, se) in a.sections.iter().zip(&e.sections) {
            let sav = rel_section_fields!(sa);
            let sev = rel_section_fields!(se);
            for ((fa, va), (fe, ve)) in sav.iter().zip(sev.iter()) {
                debug_assert_eq!(fa, fe, "both extractions are the one macro");
                if dropped(fa) {
                    continue;
                }
                assert!(
                    va.matches(*ve),
                    "{ctx}: meter {} section {} field `{fa}` differs against `{tag}`: Rust {} vs \
                     oracle {}",
                    e.name,
                    se.idx,
                    va.render(),
                    ve.render(),
                );
            }
        }

        // The two per-phase arrays. Lengths are compared unconditionally (both
        // sides report `NPhases` entries); the VALUES go through
        // `RELIABILITY_SKIP_FIELDS` while the port's regime says the oracle is
        // reading uninitialized memory.
        let skippable = reliability_skip_applies(a);
        for (field, va, ve) in [
            ("calc_current", &a.calc_current, &e.calc_current),
            ("alloc_factors", &a.alloc_factors, &e.alloc_factors),
        ] {
            assert_eq!(
                va.len(),
                ve.len(),
                "{ctx}: meter {} `{field}` length differs against `{tag}`: Rust {} vs oracle {} \
                 (both sides report NPhases entries)",
                e.name,
                va.len(),
                ve.len(),
            );
            if dropped(field) {
                continue;
            }
            let skip = reliability_skip_row(tag, field).filter(|_| skippable);
            for (k, (x, y)) in va.iter().zip(ve.iter()).enumerate() {
                // These two are the ONLY cells of this surface that are not
                // deck-literal arithmetic: both are functions of the metered
                // element's terminal currents at the solve
                // `CalcAllocationFactors` ran on, so they carry the
                // faer-vs-KLU floor every current carries (see
                // [`reliability_array_band`]).
                let i_port = a.calc_current.get(k).copied().unwrap_or(0.0);
                let i_oracle = e.calc_current.get(k).copied().unwrap_or(0.0);
                let band = reliability_array_band(field, *y, i_port, i_oracle, tol);
                let equal = rel_num_eq(*x, *y) || band.is_some_and(|b| (x - y).abs() <= b);
                if let Some(i) = skip {
                    RELIABILITY_SKIP_VISITS[i].fetch_add(1, AtomicOrd::Relaxed);
                    if !equal {
                        RELIABILITY_SKIP_HITS[i].fetch_add(1, AtomicOrd::Relaxed);
                    }
                    continue;
                }
                // The denominator gate. A ratio whose denominator is inside its
                // own absolute band has no derivable band, so it must not be
                // admitted silently — see [`reliability_array_band`].
                assert!(
                    band.is_some(),
                    "{ctx}: meter {} `{field}[{k}]` against `{tag}`: the metered current \
                     is |I| = {:.3e} A (port {i_port:?}, oracle {i_oracle:?}), inside the \
                     tier's own absolute band i_abs = {:.3e} A, so `SensorCurrent/|I|` is \
                     a noise/noise form with no derivable band (the `SeqCurrents %I` \
                     band-limited-denominator precedent, `tests/TOLERANCE_NOTES.md`). \
                     Triage this deck before the cell is compared, never widen the band.",
                    e.name,
                    i_port.abs().max(i_oracle.abs()),
                    tol.i_abs,
                );
                let band = band.unwrap_or(0.0);
                assert!(
                    equal,
                    "{ctx}: meter {} `{field}[{k}]` differs against `{tag}`: Rust {x:?} vs oracle \
                     {y:?} (|diff| = {:.3e} > band {band:.3e}). The exclusion \
                     `RELIABILITY_SKIP_FIELDS` did NOT apply here, so this deck ran \
                     `AllocateLoads` and both sides are defined (port alloc_factors = {:?}).",
                    e.name,
                    (x - y).abs(),
                    a.alloc_factors,
                );
            }
        }

        // The three zone lists: length exact, membership as a case-insensitive
        // set, and then the **order**, element by element (see the fn doc for
        // the walks the order contract comes from and the measurement that
        // turned this arm on). The two assertions are kept apart on purpose:
        // "the port walks a different zone" and "the port walks the same zone
        // in a different order" are different bugs and must not print the same
        // message. The set arm alone cannot see a multiplicity error either —
        // `controls:energymeter/midi_energymeter.dss` legitimately reports
        // `Transformer.t8` twice in `ends` — which the ordered arm does.
        for (what, la, le) in [
            ("branches", &a.branches, &e.branches),
            ("ends", &a.ends, &e.ends),
            ("pce", &a.pce, &e.pce),
        ] {
            assert_eq!(
                la.len(),
                le.len(),
                "{ctx}: meter {} {what} list length differs against `{tag}`: Rust {} vs oracle {}",
                e.name,
                la.len(),
                le.len(),
            );
            if dropped(what) {
                continue;
            }
            let lower =
                |v: &[String]| -> Vec<String> { v.iter().map(|s| s.to_lowercase()).collect() };
            let (va, ve) = (lower(la), lower(le));
            let set = |v: &[String]| -> BTreeSet<String> { v.iter().cloned().collect() };
            let (sa, se) = (set(&va), set(&ve));
            assert!(
                sa == se,
                "{ctx}: meter {} {what} membership differs against `{tag}` \
                 (Rust∖oracle={:?}, oracle∖Rust={:?})",
                e.name,
                sa.difference(&se).collect::<Vec<_>>(),
                se.difference(&sa).collect::<Vec<_>>(),
            );
            if let Some(k) = va.iter().zip(&ve).position(|(x, y)| x != y) {
                panic!(
                    "{ctx}: meter {} {what} ORDER differs against `{tag}` at index {k}: Rust \
                     {:?} vs oracle {:?} (same members, different sequence). Both oracles emit \
                     these lists in their own zone-walk order — `BranchList.First`/`GoForward` \
                     for branches and ends (capi `CAPI/CAPI_Meters.pas:589-595`, r4133 \
                     `Version8/Source/DDLL/DMeters.pas:706-733` and `:682-705`) and \
                     `GetPCEatZone` order for the PCE list (r4133 `:735-758`) — and the port \
                     returns `sequence_list()`, pushed by the same tree walk \
                     (`solution/meters/zones/build.rs:194`). Rust {va:?} vs oracle {ve:?}",
                    e.name, va[k], ve[k],
                );
            }
        }
    }

    // `Meters.Totals` — the masked register sum over EVERY meter of the circuit
    // list (no `Enabled` filter, unlike the walk above), in creation order.
    let totals = dss.meter_totals();
    assert_eq!(
        totals.len(),
        exp.totals.len(),
        "{ctx}: `Meters.Totals` length differs against `{tag}`: Rust {} vs oracle {} \
         (NumEMRegisters = 32 + 5*7 = 67)",
        totals.len(),
        exp.totals.len(),
    );
    if !ledger_skip("totals") {
        for (i, (x, y)) in totals.iter().zip(&exp.totals).enumerate() {
            // The **energy-accumulation** tier, not this surface's exactness
            // rule (coordinator decision D17a): every slot is a masked sum of
            // the very registers `compare_meter` compares at
            // `tol.energy_rel`/`energy_abs`, so it inherits their floor — a sum
            // of quantities that are not exact cannot itself be exact. It is
            // not a new band: `tests/TOLERANCE_NOTES.md` carries the derivation
            // and the measured worst slot.
            let allowed = tol.energy_abs + tol.energy_rel * y.abs();
            let ok = if x.is_nan() || y.is_nan() {
                x.is_nan() && y.is_nan()
            } else {
                (x - y).abs() <= allowed
            };
            assert!(
                ok,
                "{ctx}: `Meters.Totals[{i}]` differs against `{tag}`: Rust {x:?} vs oracle {y:?} \
                 (|diff|={:.3e} > allowed {allowed:.3e}; `TotalizeMeters` = Σ registers·Mask \
                 over the circuit's meter list — r4133 `Common/Circuit.pas:2520-2538`, capi \
                 `Common/Circuit.pas:2347-2360` — compared at the energy tier the summed \
                 registers themselves use)",
                (x - y).abs(),
            );
        }
    }

    let slot = pd_channel_slot(channel);
    RELIABILITY_WALKS[slot].fetch_add(1, AtomicOrd::Relaxed);
    RELIABILITY_METERS[slot].fetch_add(exp.meters.len(), AtomicOrd::Relaxed);
}

/// **Fail-on-nothing-ran for the whole reliability surface** — the G1.6(i) half
/// of plan §1.1(f), modeled on [`assert_pd_elements_compare_ran`].
///
/// The per-case rail ([`capture_guard::require_capture_opt`]) can only see an
/// absent payload on a case whose flag is on; it cannot see the flag going off
/// everywhere. A collapse to zero — the manifest rows lost, a transport
/// dropping the request, `RelCalc` no longer driven — would compare nothing at
/// all and pass. This fails unless **each** gating channel compared at least
/// one meter, because a surface wired on one channel only is what the plan's D2
/// forbids.
///
/// Silent under `DSS_GATE_ONLY` for the reason its neighbours are: a filtered
/// run may legitimately hold no case that gates a given channel.
pub fn assert_reliability_compare_ran() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let (cw, cm, rw, rm) = reliability_walk_counters();
    check_reliability_compare_ran((cw, cm), (rw, rm));
}

/// The rule itself, over **injected** counters — the split exists for the
/// reason [`check_pd_elements_compare_ran`]'s does: the shipped statics cannot
/// be zeroed once a gate run has moved them, so both directions are pinned
/// offline.
fn check_reliability_compare_ran(capi: (usize, usize), r4133: (usize, usize)) {
    assert!(
        capi.1 > 0 && r4133.1 > 0,
        "the reliability compare never reached one of the two channels: capi_v0145 {} \
         payload(s) / {} meter(s), r4133 {} payload(s) / {} meter(s). Since GOLDEN_REBASE \
         G1.6(i) the `compare_reliability` manifest flag drives the executive `RelCalc` on all \
         three engines and compares the whole `Meters` reliability surface; the flag is set by \
         the manifests (never scheduler-forced — the predicate \"has an EnergyMeter\" is not a \
         manifest field), so a zero here means the rows were lost, a transport stopped honoring \
         `reliability`, or the drive was removed.",
        capi.0,
        capi.1,
        r4133.0,
        r4133.1,
    );
}

/// **Fail-on-stale for [`RELIABILITY_SKIP_FIELDS`]**: every row must still be
/// consulted about cells that exist.
///
/// One arm only, and the asymmetry with [`assert_pd_skip_rows_are_live`] is
/// deliberate and measured:
///
/// * `visits == 0` — the row's `(channel, field)` never occurred in the
///   uninitialized regime anywhere in the population, so it excludes nothing
///   that exists. **Fails.**
/// * `hits == 0` is **not** a failure here. The excluded value is a fresh
///   `ReallocMem` region rather than a live heap pointer, and such a region very
///   often reads back as `0.0` — the port's own correct value. Measured: capi
///   `modes:time/midi_duty_ctrl.dss` returns `[0.0, 0.0, 0.0]` for both arrays
///   while r4133 `controls:combo/combo_protection.dss` returns
///   `[2.806806272625585e-309, 2.121995791e-314, 2.37e-322]`, varying across
///   processes. "The two sides agreed" is therefore luck, not evidence that the
///   defect is gone, and a rule that failed on it would red the gate at random.
///   The count is printed by the gate epilogue instead, so a channel that stops
///   producing garbage entirely is visible without being fatal.
///
/// Silent under `DSS_GATE_ONLY`, and silent when the surface never ran on both
/// channels — that case is [`assert_reliability_compare_ran`]'s, invoked first.
pub fn assert_reliability_skip_rows_are_live() {
    if std::env::var("DSS_GATE_ONLY").is_ok() {
        return;
    }
    let read = |c: &[AtomicUsize]| -> Vec<usize> {
        c.iter().map(|c| c.load(AtomicOrd::Relaxed)).collect()
    };
    let (_, cm, _, rm) = reliability_walk_counters();
    check_reliability_skip_rows_are_live(
        RELIABILITY_SKIP_FIELDS,
        &read(&RELIABILITY_SKIP_VISITS),
        cm > 0 && rm > 0,
    );
}

/// The staleness rule over **injected** counters (see
/// [`assert_reliability_skip_rows_are_live`]); `surface_ran` is false when the
/// compare did not reach some channel, which silences it.
fn check_reliability_skip_rows_are_live(
    table: &[ReliabilitySkipRow],
    visits: &[usize],
    surface_ran: bool,
) {
    assert_eq!(
        table.len(),
        visits.len(),
        "the counters are indexed exactly like the table"
    );
    if !surface_ran {
        return;
    }
    let stale: Vec<String> = table
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            if visits[i] > 0 {
                return None;
            }
            Some(format!(
                "stale reliability skip row: {}/{} was never consulted — no `{}`-gating case \
                     in the population reached it with the port's allocation factors still \
                     untouched. Cited: {}, pinned by `{}`",
                r.channel, r.field, r.channel, r.cite, r.pin
            ))
        })
        .collect();
    assert!(
        stale.is_empty(),
        "{}\n— each row drops a `Meters` array from the oracle compare, so it must name a \
         regime that is really there (GOLDEN_REBASE_PLAN.md §1.1(e)).",
        stale.join("\n")
    );
}

#[cfg(test)]
mod reliability_tests {
    use super::*;

    fn section() -> FeederSectionCap {
        FeederSectionCap {
            idx: 1,
            num_section_customers: 2,
            num_section_branches: 3,
            sect_seq_idx: 4,
            sect_total_cust: 5,
            ocp_device_type: 6,
            sum_branch_flt_rates: 7.0,
            avg_repair_time: 8.0,
            fault_rate_x_repair_hrs: 9.0,
        }
    }

    /// **The `alloc_factors` band is band-limited from below** (G1.6(i) audit
    /// settlement, finding A/4). Three regimes, one per arm of
    /// [`reliability_array_band`]:
    ///
    /// * both currents exactly `0.0` — the Pascal's `ELSE … := 1.0` branch on
    ///   both sides, nothing propagated, `Some(0.0)` = exact;
    /// * a real current — the propagated image band, which must stay far below
    ///   the value it guards (here `1e-8` against a value of `1.0`);
    /// * a current inside the tier's own `i_abs` — `None`, so
    ///   [`compare_reliability`] fails loudly instead of admitting the cell.
    ///
    /// Before the settlement the last regime returned `|f|·1e3` at
    /// `i_ref = 1e-9`, i.e. a 100 000 % relative error passed with no counter
    /// and no message.
    #[test]
    fn the_alloc_factors_band_is_band_limited_from_below() {
        let tol = tol_for("micro");
        assert_eq!(tol.i_abs, 1e-6, "the micro tier's current floor");

        // Both engines took the `1.0` branch: exact.
        assert_eq!(
            reliability_array_band("alloc_factors", 1.0, 0.0, 0.0, &tol),
            Some(0.0),
        );

        // A real metered current: the image band, ~1e-8 on a ~1.0 ratio at
        // 115.7 A — four orders below the value, as the derivation says.
        let real = reliability_array_band("alloc_factors", 1.0372, 115.69, 115.69, &tol)
            .expect("a loaded phase has a derivable band");
        assert!(
            real < 1.0372 * 1e-7,
            "the propagated band on a loaded phase must stay far below the value: {real:e}"
        );

        // Denominator inside its own absolute band: no band at all.
        for i_ref in [1e-9, 1e-7, 9.99e-7] {
            assert_eq!(
                reliability_array_band("alloc_factors", 1.0, i_ref, i_ref, &tol),
                None,
                "|I| = {i_ref:e} A is inside i_abs = {:e}",
                tol.i_abs,
            );
        }
        // Exactly at the floor it is a band again, and a bounded one.
        let at = reliability_array_band("alloc_factors", 1.0, 1e-6, 1e-6, &tol)
            .expect("|I| == i_abs is the first admitted denominator");
        assert!(
            at <= 1.0 * (1.0 + tol.i_rel),
            "bounded at the floor: {at:e}"
        );

        // Non-finite currents never produce a band either — and the NaN must
        // be ruled out BEFORE `f64::max`, which returns the other operand and
        // would otherwise band the cell off a current the port never reported.
        for (a, b) in [(f64::NAN, 0.5), (0.5, f64::NAN), (f64::INFINITY, 0.5)] {
            assert_eq!(
                reliability_array_band("alloc_factors", 1.0, a, b, &tol),
                None,
                "({a:?}, {b:?}) must not yield a band"
            );
        }

        // `calc_current` divides by nothing, so it always has one.
        assert_eq!(
            reliability_array_band("calc_current", 115.0, 0.0, 0.0, &tol),
            Some(tol.i_abs + tol.i_rel * 115.0),
        );
    }

    /// The two extractions cover their keys, in the frozen read order, with the
    /// right type on each. The *port* side of the same macros is checked by the
    /// compiler: they expand over `MeterReliabilityView`/`FeederSectionView`
    /// too, so a field renamed there stops `harness/mod.rs` compiling.
    #[test]
    fn the_field_extractions_are_the_frozen_keys_in_read_order() {
        let s = section();
        let got = rel_section_fields!(&s);
        let names: Vec<&str> = got.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec![
                "idx",
                "num_section_customers",
                "num_section_branches",
                "sect_seq_idx",
                "sect_total_cust",
                "ocp_device_type",
                "sum_branch_flt_rates",
                "avg_repair_time",
                "fault_rate_x_repair_hrs",
            ]
        );
        assert_eq!(got[0].1, RelVal::I(1));
        assert_eq!(got[6].1, RelVal::F(7.0));

        let m = MeterReliabilityCap {
            name: "em".to_string(),
            total_customers: 11,
            saifi: 1.0,
            saifi_kw: 2.0,
            saidi: 3.0,
            cust_interrupts: 4.0,
            calc_current: vec![0.0],
            alloc_factors: vec![0.0],
            branches: vec![],
            ends: vec![],
            pce: vec![],
            num_sections: 1,
            sections: vec![section()],
        };
        let got = rel_meter_scalars!(&m);
        let names: Vec<&str> = got.iter().map(|(n, _)| *n).collect();
        assert_eq!(
            names,
            vec![
                "total_customers",
                "saifi",
                "saifi_kw",
                "saidi",
                "cust_interrupts",
                "num_sections",
            ]
        );
        assert_eq!(got[0].1, RelVal::I(11));
        assert_eq!(got[2].1, RelVal::F(2.0));
    }

    /// The wire key for `SAIFIkW` is `saifikw` on both transports; the Rust
    /// field is `saifi_kw` so one macro serves the cap and the view.
    #[test]
    fn the_saifikw_wire_key_deserializes_into_saifi_kw() {
        let m: MeterReliabilityCap = serde_json::from_str(
            r#"{"name":"em","total_customers":1,"saifi":0.5,"saifikw":0.25,"saidi":1.5,
                "cust_interrupts":2.5,"calc_current":[],"alloc_factors":[],"branches":[],
                "ends":[],"pce":[],"num_sections":0,"sections":[]}"#,
        )
        .expect("wire shape");
        assert_eq!(m.saifi_kw, 0.25);
    }

    /// A channel that drops a field must fail loudly (no `#[serde(default)]`).
    #[test]
    fn a_dropped_field_fails_deserialization() {
        let err = serde_json::from_str::<MeterReliabilityCap>(
            r#"{"name":"em","total_customers":1,"saifi":0.5,"saifikw":0.25,"saidi":1.5,
                "cust_interrupts":2.5,"calc_current":[],"alloc_factors":[],"branches":[],
                "ends":[],"pce":[],"num_sections":0}"#,
        )
        .expect_err("a payload without `sections` must be refused");
        assert!(format!("{err}").contains("sections"), "{err}");
    }

    /// The NaN/±inf agreement arm (dossier Q5): the unguarded
    /// `SumFltRatesXRepairHrs / SumBranchFltRates` is `0/0` on every engine, so
    /// `NaN == NaN` is agreement — and `NaN` against a number is not.
    #[test]
    fn nan_agrees_with_nan_and_never_with_a_number() {
        assert!(rel_num_eq(f64::NAN, f64::NAN));
        assert!(!rel_num_eq(f64::NAN, 0.0));
        assert!(!rel_num_eq(0.0, f64::NAN));
        assert!(rel_num_eq(f64::INFINITY, f64::INFINITY));
        assert!(rel_num_eq(f64::NEG_INFINITY, f64::NEG_INFINITY));
        assert!(!rel_num_eq(f64::INFINITY, f64::NEG_INFINITY));
        assert!(!rel_num_eq(f64::INFINITY, f64::MAX));
        // Exactness: one ULP apart must FAIL — there is no tolerance here.
        assert!(rel_num_eq(0.0031360000000000008, 0.0031360000000000008));
        assert!(!rel_num_eq(
            0.0031360000000000008,
            f64::from_bits(0.0031360000000000008f64.to_bits() + 1)
        ));
        // The measured UB denormal against the port's correct 0.0.
        assert!(!rel_num_eq(0.0, 2.806806272625585e-309));
        assert!(RelVal::F(0.0).matches(RelVal::F(-0.0)));
        assert!(!RelVal::I(1).matches(RelVal::F(1.0)));
        assert_eq!(
            RelVal::F(2.806806272625585e-309).render(),
            "2.806806272625585e-309"
        );
    }

    /// The skip lookup is scoped to its own channel and field in both
    /// directions, and names only fields the record has.
    #[test]
    fn the_reliability_skip_lookup_matches_only_its_own_channel_and_field() {
        assert!(reliability_skip_row("capi_v0145", "alloc_factors").is_some());
        assert!(reliability_skip_row("r4133", "alloc_factors").is_some());
        assert!(reliability_skip_row("capi_v0145", "calc_current").is_some());
        assert!(reliability_skip_row("capi_v0145", "saifi").is_none());
        assert!(reliability_skip_row("r3723", "alloc_factors").is_none());
        for r in RELIABILITY_SKIP_FIELDS {
            assert!(
                RELIABILITY_ARRAY_FIELDS.contains(&r.field),
                "row {}/{} names a field the record does not have",
                r.channel,
                r.field
            );
            assert!(
                r.channel == PropsChannel::CapiV0145.tag()
                    || r.channel == PropsChannel::R4133.tag(),
                "row names an unknown channel {:?}",
                r.channel
            );
            assert!(!r.pin.is_empty() && !r.cite.is_empty());
        }
    }

    /// Both directions of the "did the surface run at all" rule.
    #[test]
    fn the_reliability_surface_must_run_on_both_channels() {
        check_reliability_compare_ran((1, 2), (1, 2));
        for bad in [((0, 0), (1, 2)), ((1, 2), (0, 0))] {
            let payload = std::panic::catch_unwind(|| check_reliability_compare_ran(bad.0, bad.1))
                .expect_err("a channel that compared no meter must fail");
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .unwrap_or_else(|| "<non-string panic>".to_string());
            assert!(
                msg.contains("never reached one of the two channels"),
                "{msg}"
            );
        }
    }

    /// The staleness rule: a never-consulted row fails; a consulted row whose
    /// two sides happened to agree does NOT (the `hits` count is luck — see
    /// [`assert_reliability_skip_rows_are_live`]).
    #[test]
    fn a_never_consulted_reliability_skip_row_is_stale_but_a_zero_hit_one_is_not() {
        let table = &RELIABILITY_SKIP_FIELDS[..2];
        check_reliability_skip_rows_are_live(table, &[0, 0], false);
        check_reliability_skip_rows_are_live(table, &[7, 9], true);
        let payload =
            std::panic::catch_unwind(|| check_reliability_skip_rows_are_live(table, &[7, 0], true))
                .expect_err("a row that was never consulted must fail");
        let msg = payload
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_else(|| "<non-string panic>".to_string());
        assert!(msg.contains("never consulted"), "{msg}");
        assert!(msg.contains("alloc_factors"), "{msg}");
        assert!(
            !msg.contains("calc_current"),
            "only the stale row is reported: {msg}"
        );
    }

    /// The port-side regime predicate: all-zero allocation factors mean
    /// `CalcAllocationFactors` never ran, and any written value takes the
    /// exclusion off — including the `1.0` the writer stores for a zero
    /// current.
    #[test]
    fn the_skip_regime_is_the_ports_untouched_allocation_arrays() {
        let mk = |alloc: Vec<f64>| MeterReliabilityView {
            name: "em".to_string(),
            total_customers: 0,
            saifi: 0.0,
            saifi_kw: 0.0,
            saidi: 0.0,
            cust_interrupts: 0.0,
            caidi: 0.0,
            calc_current: vec![0.0; alloc.len()],
            alloc_factors: alloc,
            branches: Vec::new(),
            ends: Vec::new(),
            pce: Vec::new(),
            num_sections: 0,
            sections: Vec::new(),
        };
        assert!(reliability_skip_applies(&mk(vec![0.0, 0.0, 0.0])));
        assert!(!reliability_skip_applies(&mk(vec![0.0, 0.0, 1.0])));
        assert!(!reliability_skip_applies(&mk(vec![0.5, 0.5, 0.5])));
    }
}
