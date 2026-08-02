//! AltDSS JSON-export byte goldens (JSON_EXPORT_PLAN §3.3). Each fixture under
//! `tests/golden/json/<deck>.json` carries the deck plus the oracle's exact
//! `DSSElement_ToJSON` / `IActiveClass.ToJSON` bytes for a matrix of option
//! combos. This driver replays the identical deck through `Dss`, calls
//! `obj_to_json` / `class_batch_to_json` with the same option bits, and compares
//! against the recorded bytes — a very strong gate, valid because the export is a
//! pure dump of parsed input properties (no solve, no faer-vs-KLU last-ULP
//! exposure).
//!
//! **Stage F.4:** the JSON writer's float spelling and pretty-mode line break
//! became lane rows (`compat::json_float`, `compat::JSON_LINE_BREAK`), so the
//! comparison goes through `harness::lane::compare_json`: byte-exact in the
//! parity lane, token-for-token with bit-exact numeric equality in the default
//! one. Structure, key order and every *string* — including the DSS script text
//! in `PostCommands` — stay pinned in both.
//!
//! Regenerate only manually: `python tools/golden/gen_json.py`.

use std::path::PathBuf;

use dss_core::exec::Dss;
use dss_core::report::export::json::JsonOpts;
use serde::Deserialize;

mod harness;
use harness::lane;

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

/// The `Set CktModel=` line exactly as the pinned oracle emits it for a
/// positive-sequence circuit — see [`lane_expected_json`].
const CKT_MODEL_PARITY: &str = "\"Set CktModel=\"";
/// What the engine emits in its place, in both lanes.
const CKT_MODEL_DEFAULT: &str = "\"Set CktModel=Positive\"";

/// The AltDSS JSON export's deliberate divergences from the oracle capture, as
/// an expected-value transform.
///
/// The JSON goldens are byte-compared in *both* lanes (`tests/harness/lane.rs`:
/// their writer renders numbers through its own `{:.16E}` helper, outside the
/// F-FMT inventory).
///
/// **The `Set CktModel=` line — an oracle bug, rewritten in both lanes.**
/// dss_capi renders `OrdinalToString(Integer(PositiveSequence))` from a
/// `LongBool` -1, an out-of-range ordinal, so the `PreCommands` entry is emitted
/// value-less and the flag does not survive its own serialization (the oracle's
/// own `tests/golden/json_import/rt_positive_seq.json` records the re-export of
/// its own import carrying no `CktModel` line at all). The authority writes the
/// state being saved as a literal (r4133 `Version8/Source/Common/
/// Circuit.pas:2757`), which is what the engine emits. The golden is **not**
/// re-baselined: it stays the oracle capture and this single enumerated rewrite
/// is applied to the expectation in both lanes, so every other byte remains
/// pinned to the oracle.
///
/// **The two `%8.2f` weights — a precision row, still lane-split.** The
/// `Set …weight=%8.2f` PostCommands render through `compat::fixed_w_script`,
/// whose parity kernel re-rounds FPC's 15-significant intermediate
/// ties-away-from-zero and whose default kernel rounds once, correctly.
/// `circuit_positive_seq`'s fixture sets those weights to `0.125` and `2.675`
/// *on purpose* — the two values `report::format`'s own oracle table names as
/// the reachable boundary cases — so this is the one golden that observes the
/// row. They are `String`s inside the JSON, so [`lane::compare_json`] compares
/// them verbatim; the divergence is therefore enumerated here rather than
/// absorbed by the comparator.
///
/// Returns the lane-expected text plus the two rewrite counts, which
/// [`json_ckt_model_is_rewritten_to_the_correct_value`] uses to keep both
/// non-vacuous.
fn lane_expected_json(oracle: &str) -> (String, usize, usize) {
    let mut out = oracle.replace(CKT_MODEL_PARITY, CKT_MODEL_DEFAULT);
    let ckt_model = oracle.matches(CKT_MODEL_PARITY).count();
    let mut weights = 0usize;
    if !dss_core::compat::ORACLE_PARITY {
        for (from, to) in WEIGHT_TIES_AWAY {
            weights += out.matches(from).count();
            out = out.replace(from, to);
        }
    }
    (out, ckt_model, weights)
}

/// The `compat::fixed_w_script` cells of the JSON goldens: `(parity spelling,
/// default spelling)` for the two `%8.2f` circuit weights — see
/// [`lane_expected_json`]. Same width, last digit re-rounded, nothing else.
const WEIGHT_TIES_AWAY: [(&str, &str); 2] = [
    ("Set ueweight=    0.13", "Set ueweight=    0.12"),
    ("Set lossweight=    2.68", "Set lossweight=    2.67"),
];

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
        let expected = lane_expected_json(&cap.expected).0;
        lane::compare_json(
            &expected,
            &got,
            &format!(
                "[{}] {} {} opts={} (bits {})",
                golden.name, cap.kind, cap.target, cap.opts, cap.bits
            ),
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

/// The DERIVED-RDCOhms branch (`Transformer.pas:1007-1008`). Every other
/// Transformer deck sets `rdcohms` explicitly, so only the `RdcSpecified` branch
/// was rendered and the `Rdcpu * SQR(VBase) / VABase` derivation was ungated.
/// `SQR` binds before the surrounding product, so re-associating it to
/// `(Rdcpu * VBase) * VBase` shifts the result by one ULP; winding 1 of `t2` is
/// such a case and the needle below is the SQR-first byte.
#[test]
fn json_transformer_derived_rdc() {
    assert_fixture_pins(
        "transformer_derived_rdc",
        &[r#""RDCOhms":[1.8735416666666669E+001"#],
        // The left-to-right association. Never the oracle's answer.
        &[r#""RDCOhms":[1.8735416666666666E+001"#],
    );
    run_deck("transformer_derived_rdc");
}

// The Capacitor's twin of the derived-RDCOhms gate above cannot live here: its
// derived values render only under Full, and every Full capture also emits
// `CMatrix`, whose oracle getter reads uninitialized memory (proven
// nondeterministic across processes — see the note in `gen_json.py`). Those
// values are pinned in `elements/pd/capacitor/tests.rs` instead.

/// AutoTrans's own copy of the array-alternative JSON metadata
/// (`AutoTrans.pas:439-557`): the DEFAULT sweep must render the per-winding
/// arrays under the SINGULAR keys. Losing `array_alternative`/`REDUNDANT` would
/// flip them to `Buses/Conns/kVs/kVAs`, which the positive needles below already
/// catch. No negative pin on the plural keys is possible here: they legitimately
/// appear in the deck's Full captures.
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

/// The ASSIGNED (non-empty) DER user-model data strings. `der_usermodel_full`
/// pins the six surfaces at their empty default, which cannot tell a correct
/// render from one that always emits `""`. The filename surfaces still cannot be
/// pinned (naming an unresolvable model makes the oracle raise `#570 … Not
/// Loaded`), but the data surfaces take any string with no loader involved.
#[test]
fn json_der_usermodel_assigned() {
    assert_fixture_pins(
        "der_usermodel_assigned",
        &[
            // Generator: both data surfaces, parentheses stripped on store.
            r#""UserData":"Kp=1.5,Ki=0.25""#,
            r#""ShaftData":"J=3.5,D=0.1""#,
            // Storage keeps UserData and DynaData apart.
            r#""UserData":"a=1""#,
            r#""DynaData":"b=2""#,
            r#""UserData":"c=3""#,
        ],
        // The parentheses are part of the array/quoting syntax, never stored.
        &[r#""UserData":"(Kp=1.5,Ki=0.25)""#],
    );
    run_deck("der_usermodel_assigned");
}

/// The `Spectrum=` FullNames render across the PC classes converted to
/// `PropDef::object_ref_deferred`. `der_usermodel_full` pins Generator only;
/// this deck gates the other ten oracle-visible classes, and pins GICLine's
/// `SUPPRESS_JSON_LATE` negatively (it must emit no `Spectrum` key at all).
#[test]
fn json_spectrum_refs() {
    assert_fixture_pins(
        "spectrum_refs",
        &[
            // The resolving-class prefix (Pascal `PropertyOffset2 =
            // SpectrumClass`) plus the lowercased stored name.
            r#""Spectrum":"Spectrum.mycustom""#,
            // Without FullNames the bare stored name is rendered.
            r#""Spectrum":"mycustom""#,
        ],
        &[
            // A regression that dropped `json_ref_class` would render the bare
            // name under FullNames too; the deck contains no object legitimately
            // named `Spectrum.MyCustom` in mixed case either.
            r#""Spectrum":"Spectrum.MyCustom""#,
        ],
    );
    run_deck("spectrum_refs");
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

/// The expected-value pin of the `Set CktModel=` golden rewrite, meaningful in
/// **both** lanes (the rewrite is unconditional; only the `%8.2f` weight row
/// below it is still lane-split).
///
/// Three things at once, over the whole committed JSON golden set:
///
/// 1. **Non-vacuity of the oracle side** — exactly one capture, in exactly one
///    deck, still carries the value-less `"Set CktModel="`. If that golden is
///    ever regenerated without it the rewrite becomes dead code and this fails
///    instead of passing silently.
/// 2. **Non-vacuity and scope of the transform** — exactly that one occurrence
///    is rewritten, in both lanes, and no other capture is touched.
/// 3. **Direction** — after the transform the expectation carries
///    `Set CktModel=Positive` and no value-less form; `json_circuit_positive_seq`
///    then holds the engine to it byte-for-byte.
#[test]
fn json_ckt_model_is_rewritten_to_the_correct_value() {
    let parity = dss_core::compat::ORACLE_PARITY;
    let mut oracle_hits = 0usize;
    let mut rewrites = 0usize;
    let mut weight_hits = 0usize;
    let mut weight_rewrites = 0usize;
    let mut decks = 0usize;

    let mut paths: Vec<PathBuf> = std::fs::read_dir(json_dir())
        .expect("read the JSON golden dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    paths.sort();

    for path in &paths {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        // `gen_schema.py`'s fixtures share this directory and carry no combos —
        // the same filter `json_every_deck_golden_has_a_driver` uses.
        if value.get("combo_names").is_none() {
            continue;
        }
        decks += 1;
        let golden: DeckGolden = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("{}: not a deck golden: {e}", path.display()));
        for cap in &golden.captures {
            oracle_hits += cap.expected.matches(CKT_MODEL_PARITY).count();
            for (from, _) in WEIGHT_TIES_AWAY {
                weight_hits += cap.expected.matches(from).count();
            }
            let (expected, n, w) = lane_expected_json(&cap.expected);
            rewrites += n;
            weight_rewrites += w;
            assert!(
                !expected.contains(CKT_MODEL_PARITY),
                "{}: no lane may expect a value-less `Set CktModel=`",
                path.display()
            );
            if parity {
                // The parity lane still expects the oracle capture verbatim
                // apart from this one rewrite — the `%8.2f` weight row is its
                // kernel, so it must not fire here.
                assert_eq!(
                    expected,
                    cap.expected.replace(CKT_MODEL_PARITY, CKT_MODEL_DEFAULT),
                    "{}: the parity lane expects the oracle capture with only \
                     the `Set CktModel=` rewrite applied",
                    path.display()
                );
                assert_eq!(
                    w,
                    0,
                    "{}: the parity lane must not re-round the `%8.2f` weights",
                    path.display()
                );
            }
        }
    }

    assert!(
        decks >= 20,
        "expected the whole JSON golden set, saw {decks}"
    );
    assert_eq!(
        oracle_hits, 1,
        "the committed JSON goldens no longer carry the value-less \
         `Set CktModel=` — the rewrite would be dead code"
    );
    assert_eq!(
        rewrites, oracle_hits,
        "exactly that one line is rewritten, in both lanes"
    );

    // The `compat::fixed_w_script` row, pinned the same way: exactly one capture
    // in the whole golden set carries the two `%8.2f` boundary weights
    // (`circuit_positive_seq`'s single `circuit` capture — only that kind emits
    // `PostCommands`), and the default lane re-rounds both.
    assert_eq!(
        weight_hits, 2,
        "the committed JSON goldens no longer carry exactly the two `%8.2f` \
         boundary weights — the `compat::fixed_w_script` row would stop being \
         observed here"
    );
    assert_eq!(
        weight_rewrites,
        if parity { 0 } else { weight_hits },
        "the default lane must re-round every weight cell and the parity lane none"
    );
}

/// Directory-completeness guard. Every deck is wired by hand with its own
/// `#[test]`, so a deck added to `gen_json.py` (whose golden is then committed)
/// without a matching driver would ship silently uncovered. This walks the
/// golden directory and asserts each deck golden is actually replayed, with an
/// anti-shrink floor so deleting one is caught too.
#[test]
fn json_every_deck_golden_has_a_driver() {
    // The floor is the count at the time of writing; raise it when decks are
    // added, never lower it to make a deletion pass.
    const MIN_DECKS: usize = 21;

    let src = include_str!("golden_json.rs");
    let mut stems: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(json_dir()).expect("read golden json dir") {
        let path = entry.expect("golden json dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let value: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        // `gen_schema.py`'s fixtures share this directory and carry no combos.
        if value.get("combo_names").is_none() {
            continue;
        }
        serde_json::from_str::<DeckGolden>(&text)
            .unwrap_or_else(|e| panic!("{}: not a deck golden: {e}", path.display()));
        stems.push(path.file_stem().unwrap().to_string_lossy().into_owned());
    }
    stems.sort();
    assert!(
        stems.len() >= MIN_DECKS,
        "only {} deck goldens found (floor {MIN_DECKS}): {stems:?}",
        stems.len()
    );
    for stem in &stems {
        assert!(
            src.contains(&format!("run_deck(\"{stem}\")")),
            "tests/golden/json/{stem}.json has no `run_deck(\"{stem}\")` driver — \
             the deck is committed but never replayed"
        );
    }
}
