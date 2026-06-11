//! Phase 2 gate: property round-trip against the dss-python oracle.
//!
//! For each scenario in `tests/golden/props.json`, replay the identical command
//! script through the Rust [`Dss`] executive and check that every property
//! reads back the same value the oracle produced. Per PORTING_PLAN.md §4,
//! numbers are compared with tolerance and the surrounding structure exactly,
//! never by raw float-string diffing.

use std::collections::BTreeMap;
use std::path::PathBuf;

use dss_core::exec::Dss;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PropsGolden {
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
}

fn load_golden() -> PropsGolden {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "props.json",
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let g: PropsGolden = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
    assert_eq!(g.schema, 1, "props golden schema mismatch");
    g
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

fn assert_value_matches(actual: &str, expected: &str, ctx: &str) {
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
    let golden = load_golden();
    assert!(!golden.scenarios.is_empty(), "no scenarios in golden");

    for sc in &golden.scenarios {
        let mut dss = Dss::new();
        for cmd in &sc.commands {
            dss.command(cmd);
        }
        assert!(
            dss.errors().is_empty(),
            "scenario {}: unexpected engine errors: {:?}",
            sc.name,
            dss.errors()
        );

        for (prop, expected) in &sc.properties {
            dss.command(&format!("? {}.{}", sc.target, prop));
            let actual = dss.result().to_string();
            assert_value_matches(
                &actual,
                expected,
                &format!("scenario {} property {prop}", sc.name),
            );
        }
    }
}
