//! AltDSS JSON-export byte goldens (JSON_EXPORT_PLAN §3.3). Each fixture under
//! `tests/golden/json/<deck>.json` carries the deck plus the oracle's exact
//! `DSSElement_ToJSON` / `IActiveClass.ToJSON` bytes for a matrix of option
//! combos. This driver replays the identical deck through `Dss`, calls
//! `obj_to_json` / `class_batch_to_json` with the same option bits, and asserts
//! **byte-equality** — the strongest gate, valid because the export is a pure
//! dump of parsed input properties (no solve, no faer-vs-KLU last-ULP exposure).
//!
//! Regenerate only manually: `python tools/golden/gen_json.py`.

use std::path::PathBuf;

use dss_core::exec::Dss;
use dss_core::report::export::json::JsonOpts;
use serde::Deserialize;

fn json_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "json",
    ]
    .iter()
    .collect()
}

#[derive(Debug, Deserialize)]
struct Capture {
    kind: String,
    target: String,
    opts: String,
    bits: u32,
    expected: String,
}

#[derive(Debug, Deserialize)]
struct DeckGolden {
    name: String,
    commands: Vec<String>,
    #[serde(default)]
    master: Option<String>,
    captures: Vec<Capture>,
}

fn corpus_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect()
}

fn run_deck(stem: &str) {
    let path = json_dir().join(format!("{stem}.json"));
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let golden: DeckGolden =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));

    let mut dss = Dss::new();
    dss.command("clear");
    if let Some(master) = &golden.master {
        let abs = corpus_dir().join(master);
        dss.command(&format!("compile \"{}\"", abs.display()));
    }
    for c in &golden.commands {
        dss.command(c);
    }

    for cap in &golden.captures {
        let opts = JsonOpts::from_bits(cap.bits);
        let got = match cap.kind.as_str() {
            "obj" => dss
                .obj_to_json(&cap.target, opts)
                .unwrap_or_else(|| panic!("{}: obj {} not found", golden.name, cap.target)),
            "batch" => dss
                .class_batch_to_json(&cap.target, opts)
                .unwrap_or_else(|| panic!("{}: class {} not found", golden.name, cap.target)),
            other => panic!("{}: unknown capture kind {other}", golden.name),
        };
        assert_eq!(
            got, cap.expected,
            "\n[{}] {} {} opts={} (bits {})\n  Rust:   {}\n  oracle: {}\n",
            golden.name, cap.kind, cap.target, cap.opts, cap.bits, got, cap.expected
        );
    }
}

#[test]
fn json_load_micro() {
    run_deck("load_micro");
}

#[test]
fn json_line_micro() {
    run_deck("line_micro");
}

#[test]
fn json_line_matrix() {
    run_deck("line_matrix");
}

#[test]
fn json_vsource_micro() {
    run_deck("vsource_micro");
}

#[test]
fn json_transformer_micro() {
    run_deck("transformer_micro");
}

#[test]
fn json_escape_micro() {
    run_deck("escape_micro");
}

#[test]
fn json_ieee13_samples() {
    run_deck("ieee13_samples");
}
