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
            nums.push(value);
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

/// Assert two value strings match: identical skeletons, numbers within
/// `rel`/`abs` tolerance.
pub fn assert_value_matches_tol(actual: &str, expected: &str, rel: f64, abs: f64, ctx: &str) {
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
        return;
    }
    let (askel, anums) = numeric_skeleton(actual);
    let (eskel, enums) = numeric_skeleton(expected);
    assert_eq!(
        askel, eskel,
        "{ctx}: structure differs (actual {actual:?} vs expected {expected:?})"
    );
    assert_eq!(
        anums.len(),
        enums.len(),
        "{ctx}: number count differs (actual {actual:?} vs expected {expected:?})"
    );
    for (i, (a, e)) in anums.iter().zip(&enums).enumerate() {
        let allowed = abs + rel * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: number {i} differs: actual {a} vs expected {e} \
             (from {actual:?} vs {expected:?})"
        );
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

    fn run(actual: &[(&str, &str)], oracle: &[(&str, &str)]) {
        compare_prop_lists(
            "Foo.x",
            "Foo",
            &props(actual),
            &props(oracle),
            TEST_ALLOW,
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
                "Foo.x", "Foo", &a, &o, TEST_ALLOW, false, 1e-9, 1e-9, "self",
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
/// Exists for the Stage F lane policy: a *deliberate* divergence (the default
/// lane's post-Newton `Powers`/`Losses`, `compat::
/// POWERS_REUSE_STALE_NEWTON_ITERMINAL`) is excluded **field-by-field**, never
/// case-by-case — the element name set, terminal currents, node voltages,
/// discrete state and iteration count of such a case stay fully oracle-gated.
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
    ("Capacitor", "CMatrix"),
    ("Reactor", "RMatrix"),
    ("Reactor", "XMatrix"),
    ("Fault", "GMatrix"),
    // (b) Near-zero winding-current angle: WdgCurrents renders `mag, (angle)`
    //     pairs; a ~1e-12 A (numerically-zero) winding current's angle is
    //     faer-vs-KLU noise (a cancellation floor). The magnitudes and the
    //     non-degenerate angles match; only the zero-magnitude angle diverges.
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
    ("Fuse", "FuseCurve"),
    ("Fuse", "RatedCurrent"),
];

/// The one property whose **value** the *default* lane excludes: a Stage F
/// deliberate divergence, not a comparability problem.
///
/// `Monitor.BaseFreq` — upstream's `TMonitorObj.Create` hard-pins 60.0 over the
/// inherited `ActiveCircuit.Fundamental` (Monitor.pas:472 == r4133:552; the
/// value is what selects mode-4 flicker's lamp curve), and the default lane
/// inherits like every other element. The two lanes agree in every 60 Hz deck; the one
/// gated corpus case that disagrees is the 50 Hz `LVTestCase`, whose monitors
/// then read 50 instead of 60. Excluded **only in the default lane** (the
/// parity lane still compares it, and the property *name*/order is checked in
/// both), and pinned by its own expected-value test
/// `exec::tests::base_frequency::monitor_basefreq_is_the_lane_kernel`, which
/// asserts both lanes' values on a 50 Hz deck and their agreement on a 60 Hz
/// one.
///
/// Keyed by `(class, prop)` rather than by case, so it drops the value compare
/// on every case and not just the one that needs it. That is a real if small
/// coverage loss,
/// bounded by measurement: exactly one gated deck sets a 50 Hz fundamental
/// (`electricdss-tst/Version8/Distrib/IEEETestCases/LVTestCase/Master.dss`) and
/// every mode-4 flicker deck in the corpus is 60 Hz, so no Pst output moves;
/// and what the exclusion gives up on the 60 Hz decks — that both lanes still
/// report 60 — is exactly what the pin above asserts directly.
const LANE_SKIP_PROPS: &[(&str, &str)] = &[("Monitor", "BaseFreq")];

pub fn skip_prop(class: &str, prop: &str) -> bool {
    let lane_skipped = !lane::PARITY
        && LANE_SKIP_PROPS
            .iter()
            .any(|(c, p)| class.eq_ignore_ascii_case(c) && prop.eq_ignore_ascii_case(p));
    lane_skipped || skip_prop_ub(class, prop)
}

/// The [`SKIP_PROPS`] half of [`skip_prop`] **only** — the properties whose
/// upstream getter renders uninitialized heap memory.
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
///
/// Ships EMPTY: rows land with the WP that ports each 0.15.x property. Keep it
/// one class per line so parallel WP branches each add a line without conflict
/// (duplicate class rows are fine — the predicate ORs every matching row).
const PROPS_015X: &[(&str, &[&str])] = &[
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
    // (0.14.5 and capi015); r4133-oracle cases do not property-compare.
    ("SwtControl", &["RatedCurrent"]),
    // Further rows land here with their porting WP.
];

/// Whether property `prop` of `class` is a 0.15.x-only property in `allowlist`
/// (matched case-insensitively across every row, so duplicate class rows OR).
fn prop_015x(allowlist: &[(&str, &[&str])], class: &str, prop: &str) -> bool {
    allowlist.iter().any(|(c, props)| {
        class.eq_ignore_ascii_case(c) && props.iter().any(|p| prop.eq_ignore_ascii_case(p))
    })
}

/// The property-list comparison core of [`compare_all_properties`], factored out
/// so the [`PROPS_015X`] allowlist can be injected for the self-tests (the
/// shipped table is empty). `actual` is the Rust `?`-surface list, `oracle` the
/// capture; both are `(name, value)` in property-index order. A Rust-side prop
/// that is 0.15.x-only (`allowlist`) AND absent from the `oracle` capture is
/// dropped before the count/order/name walk; every surviving prop is compared
/// exactly as before (name in order, value via [`assert_value_matches_tol`],
/// with the [`SKIP_PROPS`]/[`skip_transformer_cursor`] value-skip gates).
#[allow(clippy::too_many_arguments)]
fn compare_prop_lists(
    element: &str,
    class: &str,
    actual: &[(String, String)],
    oracle: &[(String, String)],
    allowlist: &[(&str, &[&str])],
    cursors_disagree: bool,
    rel: f64,
    abs: f64,
    ctx: &str,
) {
    // Oracle capture's property names (case-insensitive) — the "does the pinned
    // oracle know this prop?" set that gates the 0.15.x exclusion.
    let oracle_names: BTreeSet<String> = oracle.iter().map(|(n, _)| n.to_lowercase()).collect();
    // Exclude Rust-side 0.15.x-only props the 0.14.5 capture cannot contain. If
    // the capture DOES contain the prop (capi015), keep it → full compare.
    let filtered: Vec<&(String, String)> = actual
        .iter()
        .filter(|(n, _)| {
            !prop_015x(allowlist, class, n) || oracle_names.contains(&n.to_lowercase())
        })
        .collect();
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
        if skip_prop(class, ename) || skip_transformer_cursor(class, ename, cursors_disagree) {
            continue;
        }
        // Case-EXACT compare (no lowercasing): every DSS enum getter renders the
        // Pascal-faithful case — `ordinal_to_string` returns the exact registry
        // strings (`wye`/`delta` lowercase, `Variable`/`Fixed` capitalized,
        // booleans `Yes`/`No`) that the oracle's `Val` emits, so a case
        // divergence is a real rendering regression this gate must catch, not a
        // formatting artifact to smooth over.
        assert_value_matches_tol(
            aval,
            eval,
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
pub fn compare_all_properties(dss: &mut Dss, exp: &[PropsCap], tol: &Tolerances, ctx: &str) {
    for pc in exp {
        let class = pc.element.split('.').next().unwrap_or("");
        // WP-U2.2: the Recloser property TABLE moved to the r4133 46-prop surface
        // (renames + new props); it cannot match the pinned **0.14.5** oracle's
        // 24-prop table shape (`compare_all_properties` runs only vs 0.14.5 —
        // corpus_live gates it on `oracle.is_none()`). The recloser's r4133 shape
        // is code-verified (the `class_props` `debug_assert` on the 46 defs) and
        // its property VALUES are gated by the `recloser.json` props golden (by
        // name) + the `oracle: "r4133"` family probes; its shape simply isn't
        // 0.14.5-oracle-gateable. Skip the whole element here rather than mask 46
        // individual rows. Sibling protection classes (Relay/Fuse) join this list
        // when WP-U2.1/U2.3 move their tables.
        if class.eq_ignore_ascii_case("Recloser") {
            continue;
        }
        // WP-U2.3: the Relay property TABLE likewise moved to the r4133 71-prop
        // surface (50->71 renames + new props + `Normal`/`State` per-phase arrays);
        // it cannot match the pinned 0.14.5 oracle's 50-prop table shape. Its shape
        // is code-verified (`class_props` `debug_assert` on 73 defs) and its values
        // are gated by the `relay.json` props golden (by name) + the `oracle:
        // "r4133"` family probes. Skip the whole element here (as Recloser).
        if class.eq_ignore_ascii_case("Relay") {
            continue;
        }
        let actual = dss
            .element_properties(&pc.element)
            .unwrap_or_else(|| panic!("{ctx}: no element {} (all_properties)", pc.element));
        // Transformer cursor-skip gate (see [`skip_transformer_cursor`]): the
        // singular per-winding forms compare only when both engines' ActiveWinding
        // (the `Wdg` value) point to the same winding. Read Rust's from `actual`
        // and the oracle's from the capture.
        let cursor_of = |props: &[(String, String)]| -> Option<String> {
            props
                .iter()
                .find(|(n, _)| n.eq_ignore_ascii_case("Wdg"))
                .map(|(_, v)| v.trim().to_string())
        };
        let cursors_disagree =
            class.eq_ignore_ascii_case("Transformer") && cursor_of(&actual) != cursor_of(&pc.props);
        compare_prop_lists(
            &pc.element,
            class,
            &actual,
            &pc.props,
            PROPS_015X,
            cursors_disagree,
            tol.i_rel,
            tol.i_abs,
            ctx,
        );
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
pub fn compare_variables(dss: &mut Dss, exp: &VariablesCap, tol: &Tolerances, ctx: &str) {
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
    for (i, (a, e)) in act.iter().zip(&exp.values).enumerate() {
        let allowed = tol.i_abs + tol.i_rel * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: {} variable {} ({}) differs: {a} vs {e} (|diff|={:.3e} > allowed {allowed:.3e})",
            exp.name,
            i + 1,
            exp.var_names.get(i).map(String::as_str).unwrap_or("?"),
            (a - e).abs()
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
        // The lane's reading of the capture: identical to it except for
        // dss-python's unflushed-stream `[0.0]` placeholder in the default lane
        // (`lane::expected_monitor_channel`).
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
    /// Its one use is a Stage F **deliberate divergence** confined to
    /// identifiable rows: `Export SeqCurrents`' `Iresidual` reproduces an
    /// upstream indexing bug that prints *terminal 1's* residual on every
    /// terminal row, and the default lane fixes it — so exactly the rows with
    /// `Terminal ≥ 2` are excluded there (gate `ColAbove(1, 1.5)`), while every
    /// terminal-1 cell stays compared against the oracle in both lanes. The
    /// excluded cells are pinned instead by their own expected-value test
    /// (`export_seqcurrents_iresidual_is_the_lane_kernel`), which is why this
    /// is an *exclusion with a replacement gate*, not a relaxation.
    ColAbove(usize, f64),
    /// **Always** skip the matched column — a non-deterministic column that
    /// carries no comparable value (a wall-clock timestamp or an absolute path).
    /// Not a tolerance relaxation of any *value*: the column is genuinely
    /// unpinnable (`Summary`'s `DateTimeToStr(Now)`), documented in
    /// `tests/TOLERANCE_NOTES.md`.
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
