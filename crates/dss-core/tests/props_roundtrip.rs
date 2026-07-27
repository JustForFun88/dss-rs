//! Phase 2 gate: property round-trip against the dss-python oracle.
//!
//! For each scenario under `tests/golden/props/` (one `<class>.json` per DSS
//! class, each holding that class's scenarios; the gate runs every file in the
//! directory), replay the identical command script through the Rust [`Dss`]
//! executive and check that every property reads back the same value the oracle
//! produced. Per PORTING_PLAN.md §4, numbers are compared with tolerance and the
//! surrounding structure exactly, never by raw float-string diffing.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dss_core::exec::Dss;
use serde::Deserialize;

/// One per-class file: `{schema, oracle, class, scenarios}` (`oracle`/`class`
/// ignored here).
#[derive(Debug, Deserialize)]
struct PropsFile {
    schema: u32,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    commands: Vec<String>,
    target: String,
    /// Property name → oracle value string.
    properties: BTreeMap<String, String>,
    /// Some objects log a non-fatal error during their own RecalcElementData
    /// even when fully specified (e.g. a StorageController on a circuit with no
    /// Storage element always logs 37201 — faithfully reproduced). Those
    /// scenarios still pin the property dump; the error itself is covered by a
    /// dedicated unit test, so skip the "no engine errors" assertion here.
    #[serde(default)]
    allow_errors: bool,
}

/// Load every `*.json` class file from `tests/golden/props/` (sorted by file
/// name for deterministic order) and flatten their scenarios.
fn load_scenarios() -> Vec<Scenario> {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "props",
    ]
    .iter()
    .collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let mut scenarios = Vec::new();
    for p in &files {
        let text = std::fs::read_to_string(p)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
        let f: PropsFile = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("cannot parse {}: {e}", p.display()));
        assert_eq!(f.schema, 1, "{}: props golden schema mismatch", p.display());
        scenarios.extend(f.scenarios);
    }
    scenarios
}

/// Split a value string into its non-numeric "skeleton" (each number replaced
/// by `#`) and the list of numbers it contains.
fn numeric_skeleton(s: &str) -> (String, Vec<f64>) {
    let mut skeleton = String::new();
    let mut nums = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if let Some((value, len)) = scan_number(&s[i..]) {
            nums.push(value);
            skeleton.push('#');
            i += len;
        } else {
            // values are ASCII; advance one byte
            skeleton.push(s.as_bytes()[i] as char);
            i += 1;
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

/// Stage F deliberate divergences: `(scenario, property)` pairs whose **value**
/// the *default* lane does not compare against the oracle, because its clean fix
/// intentionally reports a different one. The **parity** lane still compares
/// every pair, and both lanes still compare every *other* property of these
/// scenarios — the exclusion is value-only and per-property, never per-scenario.
///
/// * `isource_bus2_clobbered_by_bus1` / `Bus2` — the scenario exists to pin the
///   upstream quirk that `TIsourceObj.PropertySideEffects` (`Isource.pas:221`)
///   has no `Bus2` case, so `Bus2Defined` never latches and the `Bus1` side
///   effect re-derives `b1.0.0.0` over the explicit `b2` this deck wrote first.
///   `TVsourceObj.PropertySideEffects` (`Vsource.pas:498`) latches it on the
///   very same property; the default lane does too
///   (`compat::ISOURCE_BUS2_NEVER_LATCHES`), so it reports `b2`. Pinned in both
///   lanes by `elements::pc::isource::tests::bus2_latching_is_the_lane_kernel`.
const LANE_SKIP_SCENARIO_PROPS: &[(&str, &str)] = &[("isource_bus2_clobbered_by_bus1", "Bus2")];

/// Stage F deliberate divergence, by **property name across every scenario**:
/// the `DoubleSymMatrixProperty` text getter
/// (`compat::SYM_MATRIX_GETTER_RENDERS_ZEROS`). Upstream's arm reads
/// uninitialized memory and prints ~0 whatever was stored, so every one of these
/// oracle values is an all-zero matrix; the default lane renders the stored
/// values — the same numbers this property's own JSON exporter already emits in
/// both engines.
///
/// The exclusion is **values only**: [`assert_shape_matches`] still compares the
/// numeric *skeleton* in the default lane, so the parenthesised
/// `(v |v v |v v v )` shape, the matrix order and the row split stay gated
/// there; the parity lane compares the values too. The rendered numbers are
/// pinned in both lanes by `dss_core::exec::tests::compat_quirks::
/// sym_matrix_text_getter_is_lane_split`.
const LANE_SKIP_PROP_VALUES: &[&str] = &["CMatrix", "GMatrix", "RMatrix", "XMatrix"];

/// The value-free half of [`assert_value_matches`]: the rendered *shape* only —
/// the literal text around the numbers and how many numbers there are. Used in
/// the default lane for [`LANE_SKIP_PROP_VALUES`], where the values are a
/// deliberate divergence but the layout is not.
fn assert_shape_matches(actual: &str, expected: &str, ctx: &str) {
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
}

fn assert_value_matches(actual: &str, expected: &str, ctx: &str) {
    assert_shape_matches(actual, expected, ctx);
    let (_, anums) = numeric_skeleton(actual);
    let (_, enums) = numeric_skeleton(expected);
    for (i, (a, e)) in anums.iter().zip(&enums).enumerate() {
        let allowed = 1e-12 + 1e-9 * e.abs();
        assert!(
            (a - e).abs() <= allowed,
            "{ctx}: number {i} differs: actual {a} vs expected {e} \
             (from {actual:?} vs {expected:?})"
        );
    }
}

#[test]
fn props_roundtrip_matches_oracle() {
    let scenarios = load_scenarios();
    assert!(!scenarios.is_empty(), "no scenarios in golden");

    // Stale-entry guard: every exclusion below must still name a real
    // `(scenario, property)` pair, so a renamed or deleted scenario fails the
    // gate instead of silently widening it.
    for (scenario, prop) in LANE_SKIP_SCENARIO_PROPS {
        let sc = scenarios
            .iter()
            .find(|s| s.name == *scenario)
            .unwrap_or_else(|| panic!("stale lane exclusion: no scenario {scenario:?}"));
        assert!(
            sc.properties.keys().any(|k| k.eq_ignore_ascii_case(prop)),
            "stale lane exclusion: scenario {scenario:?} has no property {prop:?}"
        );
    }

    let mut value_skips = 0usize;
    for sc in &scenarios {
        let mut dss = Dss::new();
        // gen_props.py runs this preamble before every scenario (the `?`
        // query is circuit-gated in ProcessCommand, so the oracle needed a
        // circuit too).
        dss.command("clear");
        dss.command("new circuit.propsprobe");
        for cmd in &sc.commands {
            dss.command(cmd);
        }
        assert!(
            sc.allow_errors || dss.errors().is_empty(),
            "scenario {}: unexpected engine errors: {:?}",
            sc.name,
            dss.errors()
        );

        for (prop, expected) in &sc.properties {
            if !dss_core::compat::ORACLE_PARITY
                && LANE_SKIP_SCENARIO_PROPS
                    .iter()
                    .any(|(s, p)| *s == sc.name && p.eq_ignore_ascii_case(prop))
            {
                continue;
            }
            dss.command(&format!("? {}.{}", sc.target, prop));
            let actual = dss.result().to_string();
            let ctx = format!("scenario {} property {prop}", sc.name);
            if !dss_core::compat::ORACLE_PARITY
                && LANE_SKIP_PROP_VALUES
                    .iter()
                    .any(|p| p.eq_ignore_ascii_case(prop))
            {
                assert_shape_matches(&actual, expected, &ctx);
                value_skips += 1;
                continue;
            }
            assert_value_matches(&actual, expected, &ctx);
        }
    }

    // Non-vacuity: the by-property exclusion must actually name properties the
    // goldens carry, so a rename cannot silently turn it into a no-op. (In the
    // parity lane nothing is skipped — the count is asserted zero there.)
    if dss_core::compat::ORACLE_PARITY {
        assert_eq!(value_skips, 0, "the parity lane compares every value");
    } else {
        assert!(
            value_skips >= LANE_SKIP_PROP_VALUES.len(),
            "stale lane exclusion: only {value_skips} of the {} \
             LANE_SKIP_PROP_VALUES properties were seen in the goldens",
            LANE_SKIP_PROP_VALUES.len()
        );
    }
}
