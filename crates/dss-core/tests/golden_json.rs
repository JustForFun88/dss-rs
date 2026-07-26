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
    combo_names: Vec<String>,
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

    // Coverage guard: every (kind,target) must carry exactly the deck's declared
    // combo set. Catches a silently dropped/renamed combo in the generator that
    // would otherwise just shrink coverage while the byte checks stay green.
    assert!(
        !golden.combo_names.is_empty(),
        "{}: no combos declared",
        golden.name
    );
    let mut by_target: std::collections::BTreeMap<(&str, &str), Vec<&str>> =
        std::collections::BTreeMap::new();
    for cap in &golden.captures {
        by_target
            .entry((cap.kind.as_str(), cap.target.as_str()))
            .or_default()
            .push(cap.opts.as_str());
    }
    for ((kind, target), mut got) in by_target {
        got.sort_unstable();
        let mut want: Vec<&str> = golden.combo_names.iter().map(String::as_str).collect();
        want.sort_unstable();
        assert_eq!(
            got, want,
            "{}: {kind} {target} combos {got:?} != declared {want:?}",
            golden.name
        );
    }

    for cap in &golden.captures {
        let opts = JsonOpts::from_bits(cap.bits);
        let got = match cap.kind.as_str() {
            // The `_mut` routes refresh live-state caches (Transformer/AutoTrans
            // `WdgCurrents`) before rendering — a no-op for classes without a
            // `READS_VTERMINAL` property, so it is safe for every deck.
            "obj" => dss
                .obj_to_json_mut(&cap.target, opts)
                .unwrap_or_else(|| panic!("{}: obj {} not found", golden.name, cap.target)),
            "batch" => dss
                .class_batch_to_json_mut(&cap.target, opts)
                .unwrap_or_else(|| panic!("{}: class {} not found", golden.name, cap.target)),
            "circuit" => dss
                .circuit_to_json(opts)
                .unwrap_or_else(|| panic!("{}: no active circuit", golden.name)),
            other => panic!("{}: unknown capture kind {other}", golden.name),
        };
        assert_eq!(
            got, cap.expected,
            "\n[{}] {} {} opts={} (bits {})\n  Rust:   {}\n  oracle: {}\n",
            golden.name, cap.kind, cap.target, cap.opts, cap.bits, got, cap.expected
        );
    }
}

/// Fixture-intent guard. The byte comparison alone cannot tell a fixture that
/// pins the property under test from one that silently stopped rendering it
/// (both engines would just agree on the shorter output). So assert that the
/// recorded ORACLE bytes really contain the keys/values the deck was added for,
/// and never contain the ones its metadata is supposed to suppress.
fn assert_fixture_pins(stem: &str, required: &[&str], forbidden: &[&str]) {
    let path = json_dir().join(format!("{stem}.json"));
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let golden: DeckGolden =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    for needle in required {
        assert!(
            golden.captures.iter().any(|c| c.expected.contains(needle)),
            "{stem}: no oracle capture contains {needle:?} — the fixture no longer \
             pins what it was added for"
        );
    }
    for needle in forbidden {
        assert!(
            !golden.captures.iter().any(|c| c.expected.contains(needle)),
            "{stem}: an oracle capture contains {needle:?}, which this fixture \
             asserts is never rendered"
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
fn json_transformer_solved() {
    run_deck("transformer_solved");
}

/// AutoTrans's own copy of the array-alternative JSON metadata
/// (`AutoTrans.pas:439-557`): the DEFAULT sweep must render the per-winding
/// arrays under the SINGULAR keys. Losing `array_alternative`/`REDUNDANT` would
/// flip them to `Buses/Conns/kVs/kVAs` — pinned negatively below, because the
/// plural keys are legitimately present in the deck's Full captures.
#[test]
fn json_autotrans_micro() {
    assert_fixture_pins(
        "autotrans_micro",
        &[
            r#""Bus":["sourcebus","low","tert"]"#,
            r#""Conn":["series","wye","delta"]"#,
            r#""kV":[3.4500000000000000E+002"#,
            r#""kVA":[3.3000000000000000E+005"#,
            r#""pctR":[4.9299999999999997E-002"#,
            // The four `ON_ARRAY` scalars with no plural alternative.
            r#""RDCOhms":[1.1000000000000000E-001"#,
            r#""MaxTap":[1.1000000000000001E+000"#,
            r#""MinTap":[9.0000000000000002E-001"#,
            r#""NumTaps":[32,16,8]"#,
        ],
        &[],
    );
    run_deck("autotrans_micro");
}

/// AutoTrans `WdgCurrents` post-solve. `transformer_solved` proves the shared
/// JSON Vterminal-refresh route, but the auto has its OWN series/common/delta
/// getter (`TAutoTransObj.GetAllWindingCurrents`); every earlier AutoTrans
/// capture was pre-solve all-zeros and so could not catch a broken getter.
#[test]
fn json_autotrans_solved() {
    assert_fixture_pins(
        "autotrans_solved",
        &[r#""WdgCurrents":"549.4296, (-31.051), 513.7564, (146.64), 781.8308, (159.34),"#],
        // A dropped refresh (or a getter reading a zeroed Vterminal) renders the
        // all-zero phasor list; it must not appear anywhere in this fixture.
        &["\"WdgCurrents\":\"0, (0), 0, (0)"],
    );
    run_deck("autotrans_solved");
}

/// Generator/PVSystem/Storage user-model string properties under Full mode.
/// They were skipped from the Full sweep while they carried `NOT_PORTED`;
/// WASM_USERMODELS WM.3/WM.4 made them real properties, so the Full render must
/// now emit them as the oracle's empty strings.
#[test]
fn json_der_usermodel_full() {
    assert_fixture_pins(
        "der_usermodel_full",
        &[
            r#""UserModel":"","UserData":"","ShaftModel":"","ShaftData":"""#,
            r#""DynaDLL":"","DynaData":"""#,
            // The LowercaseKeys rendering of the same properties (FullNames
            // rewrites object *references*, not property keys, so it leaves
            // these untouched).
            r#""usermodel":"","userdata":"","shaftmodel":"","shaftdata":"""#,
            // FullNames on a deferred-resolution object ref: Pascal prefixes the
            // resolving class (`PropertyOffset2 = SpectrumClass`), and the
            // stored name is lowercased even though the object is `MyCustom`.
            r#""Spectrum":"Spectrum.mycustom""#,
            r#""Spectrum":"mycustom""#,
        ],
        &[],
    );
    run_deck("der_usermodel_full");
}

#[test]
fn json_dyneq_micro() {
    run_deck("dyneq_micro");
}

/// The `TDynEqPCE` "DynInit" tail under FULL mode. `dyneq_micro` is `skip_full`
/// (it predates the ShaftModel/ShaftData Full-render fix), so the tail was only
/// ever gated in the default sweep.
#[test]
fn json_dyneq_full() {
    assert_fixture_pins(
        "dyneq_full",
        &[
            // The literal `"DynInit"` key survives LowercaseKeys, and `damp` is
            // last (the second write moved it past the once-assigned variables)
            // with the RPN string value from the rewrite.
            r#""DynInit":{"#,
            r#""damp":"1 2 +"}"#,
            r#""speed":0.0000000000000000E+000"#,
            r#""pshaft":"P0""#,
        ],
        &[],
    );
    run_deck("dyneq_full");
}

#[test]
fn json_batch_micro() {
    run_deck("batch_micro");
}

#[test]
fn json_escape_micro() {
    run_deck("escape_micro");
}

#[test]
fn json_ieee13_samples() {
    run_deck("ieee13_samples");
}

#[test]
fn json_circuit_micro() {
    run_deck("circuit_micro");
}

#[test]
fn json_circuit_edited_default() {
    run_deck("circuit_edited_default");
}

#[test]
fn json_circuit_ieee13() {
    run_deck("circuit_ieee13");
}

#[test]
fn json_circuit_positive_seq() {
    run_deck("circuit_positive_seq");
}
