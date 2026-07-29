//! AltDSS JSON-import round-trip byte goldens (WP OG-1.4). Each fixture under
//! `tests/golden/json_import/<deck>.json` carries the oracle's whole-circuit
//! export `input_json` (J0) and the oracle's re-export after a `Circuit_FromJSON`
//! round trip `expected_json` (J1). This driver feeds the identical J0 bytes to
//! [`Dss::circuit_from_json`], re-exports the rebuilt circuit, and asserts
//! **byte-equality** with J1 -- the strongest, oracle-backed gate for the import
//! path (the export half is gated separately by `golden_json.rs`).
//!
//! The re-export order is `AltPropertyOrder`, so J0 != J1 in general; the oracle
//! round trip is idempotent after one cycle (J1 == J2), which the Rust engine
//! must also honor (checked here).
//!
//! Regenerate only manually: `python tools/golden/gen_json_import.py`.

use std::path::PathBuf;

use dss_core::compat::JSON_LINE_BREAK;
use dss_core::exec::Dss;
use dss_core::report::export::json::JsonOpts;
use serde::Deserialize;

mod harness;
use harness::lane;

/// The generator's `Circuit_ToJSON` bits: SkipTimestamp only (bit 9).
const SKIP_TIMESTAMP_BITS: u32 = 512;

fn json_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "json_import",
    ]
    .iter()
    .collect()
}

#[derive(Debug, Deserialize)]
struct ImportGolden {
    name: String,
    input_json: String,
    expected_json: String,
}

fn load(stem: &str) -> ImportGolden {
    let path = json_dir().join(format!("{stem}.json"));
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn opts() -> JsonOpts {
    JsonOpts::from_bits(SKIP_TIMESTAMP_BITS)
}

fn run_deck(stem: &str) {
    let g = load(stem);

    // Import the oracle's J0, re-export, and require J1 byte-for-byte.
    let mut dss = Dss::new();
    dss.circuit_from_json(&g.input_json)
        .unwrap_or_else(|e| panic!("{}: circuit_from_json(J0) failed: {e}", g.name));
    assert!(
        dss.errors().is_empty(),
        "{}: import logged errors: {:?}",
        g.name,
        dss.errors()
    );
    let j1 = dss
        .circuit_to_json(opts())
        .unwrap_or_else(|| panic!("{}: no circuit after import", g.name));
    // Stage F.4: the JSON writer's float spelling and line break are lane rows
    // (`compat::json_float`, `compat::JSON_LINE_BREAK`), so the *oracle* side is
    // compared token-for-token in the default lane and byte-for-byte in the
    // parity one — structure, key order and every string exactly, numbers by
    // bit pattern. See `harness::lane::compare_json`.
    lane::compare_json(
        &g.expected_json,
        &j1,
        &format!(
            "{}: re-export after import != oracle J1 (first diff at {:?})",
            g.name,
            first_diff(&j1, &g.expected_json)
        ),
    );

    // Idempotency: a second import/export cycle must reproduce J1 exactly, the
    // fixed-point the oracle reaches (J1 == J2).
    let mut dss2 = Dss::new();
    dss2.circuit_from_json(&j1)
        .unwrap_or_else(|e| panic!("{}: circuit_from_json(J1) failed: {e}", g.name));
    let j2 = dss2
        .circuit_to_json(opts())
        .unwrap_or_else(|| panic!("{}: no circuit after second import", g.name));
    assert_eq!(j2, j1, "{}: import round trip is not idempotent", g.name);
}

/// The byte offset of the first difference plus a short window, for a readable
/// failure message on a 50 KB circuit dump.
fn first_diff(a: &str, b: &str) -> String {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let n = ab.len().min(bb.len());
    for i in 0..n {
        if ab[i] != bb[i] {
            let lo = i.saturating_sub(30);
            let hi_a = (i + 40).min(a.len());
            let hi_b = (i + 40).min(b.len());
            return format!(
                "byte {i}\n  got:    {:?}\n  expect: {:?}",
                &a[lo..hi_a],
                &b[lo..hi_b]
            );
        }
    }
    if a.len() != b.len() {
        format!("length {} vs {}", a.len(), b.len())
    } else {
        "no diff".to_string()
    }
}

#[test]
fn json_import_rt_micro() {
    run_deck("rt_micro");
}

#[test]
fn json_import_rt_transformer() {
    run_deck("rt_transformer");
}

#[test]
fn json_import_rt_ieee13() {
    run_deck("rt_ieee13");
}

#[test]
fn json_import_rt_edited_default() {
    // Default DSS_OBJECT edit path: the re-export must DROP the JSON-edited
    // `spectrum.defaultload` (the oracle does — `FillObjFromJSON` never clears
    // `DefaultAndUnedited`). Guards the `set_default_and_unedited` regression.
    run_deck("rt_edited_default");
}

#[test]
fn json_import_rt_positive_seq() {
    // AllowDuplicates + `Set CktModel=positive` PreCommand round trip.
    run_deck("rt_positive_seq");
}

#[test]
fn json_import_rt_generator() {
    // Thevenin-DER (Generator) import — widens the gate beyond PD/Vsource.
    run_deck("rt_generator");
}

// --- Negative / edge-case tests (error paths, mirroring the Pascal) ---

/// A tiny valid whole-circuit JSON to mutate for the negative cases.
fn tiny_circuit_json() -> String {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.neg basekv=12.47");
    dss.command("new load.l1 bus1=sourcebus kv=12.47 kw=5 pf=0.95");
    dss.command("makebuslist");
    dss.circuit_to_json(opts()).expect("tiny circuit")
}

#[test]
fn malformed_json_errors_not_panics() {
    // Pascal `Circuit_FromJSON`: `GetJSON` raises → caught → DoSimpleMsg 20230919.
    let mut dss = Dss::new();
    let r = dss.circuit_from_json("{ not valid json ");
    assert!(r.is_err(), "malformed JSON must be an error");
    assert!(
        dss.errors().iter().any(|e| e.contains("JSON")),
        "a loud DSS error is logged: {:?}",
        dss.errors()
    );
}

#[test]
fn non_object_top_level_errors() {
    // Pascal: `not (genericData is TJSONObject)` → "expected an object".
    let mut dss = Dss::new();
    let r = dss.circuit_from_json("[1, 2, 3]");
    assert!(r.is_err(), "a JSON array is not a circuit object");
}

#[test]
fn unknown_property_is_ignored() {
    // Pascal `FillObjFromJSON` walks the class's known properties and never
    // looks up a stray key — so an unknown property is silently ignored, not an
    // error, and the known props still round-trip.
    let j0 = tiny_circuit_json();
    let injected = j0.replace(
        "\"Name\" : \"l1\",",
        "\"Name\" : \"l1\",\r\n      \"NonexistentProp\" : 42,",
    );
    assert_ne!(injected, j0, "injection point exists");
    let mut dss = Dss::new();
    dss.circuit_from_json(&injected)
        .expect("an unknown property is ignored, not fatal");
    assert!(
        dss.errors().is_empty(),
        "unknown property must not log an error: {:?}",
        dss.errors()
    );
    let j1 = dss.circuit_to_json(opts()).expect("circuit after import");
    assert!(j1.contains("\"l1\""), "the load still imported");
}

#[test]
fn unknown_class_key_is_ignored() {
    // Pascal `Obj_Circuit_FromJSON_` only looks up known `DSSClassList` names;
    // an unrecognized top-level key is never consulted → silently ignored.
    let j0 = tiny_circuit_json();
    let injected = j0.replace(
        "\"Load\" :",
        "\"BogusClass\" : [\r\n    {\r\n      \"Name\" : \"x\"\r\n    }\r\n  ],\r\n  \"Load\" :",
    );
    assert_ne!(injected, j0, "injection point exists");
    let mut dss = Dss::new();
    dss.circuit_from_json(&injected)
        .expect("an unknown class key is ignored");
    assert!(
        dss.errors().is_empty(),
        "unknown class key must not log an error: {:?}",
        dss.errors()
    );
    // The real Load must still have imported (not aborted by the bogus key).
    let j1 = dss.circuit_to_json(opts()).expect("circuit after import");
    assert!(
        j1.contains("\"l1\""),
        "the real load still imported alongside the ignored class"
    );
}

#[test]
fn missing_name_errors() {
    // Pascal `loadSingleObj`: no "Name"/"name" → raise "missing \"Name\"", caught
    // → ErrorNumber set → `Obj_Circuit_FromJSON_` Exits (aborts the Load load).
    let j0 = tiny_circuit_json();
    let injected = j0.replace("\"Name\" : \"l1\",", "");
    assert_ne!(injected, j0, "injection point exists");
    let mut dss = Dss::new();
    let _ = dss.circuit_from_json(&injected);
    assert!(
        dss.errors().iter().any(|e| e.contains("Name")),
        "a missing element Name is a loud error: {:?}",
        dss.errors()
    );
    // The build aborted at the offending item: the Load class never imported.
    let j1 = dss.circuit_to_json(opts()).expect("circuit exists");
    assert!(
        !j1.contains("\"Load\""),
        "a name-less Load item aborts the class load, not a silent import: {j1}"
    );
}

#[test]
fn missing_required_property_errors() {
    // Pascal `FillObjFromJSON` (DSSObjectHelper.pas:4955): a missing `Required`
    // property raises `JSON/<cls>/<name>: required property not provided:
    // "<prop>"`. Oracle-confirmed (dss-python 0.15.7) by dropping a Load's Bus1.
    let j0 = tiny_circuit_json();
    // Drop only the Load's Bus1 (the Vsource keeps its own).
    // The injection anchors are cut from the engine's *own* export, so they use
    // the lane's line break (`compat::JSON_LINE_BREAK`), not a literal CRLF.
    let nl = JSON_LINE_BREAK;
    let injected = j0.replace(
        &format!("\"Name\" : \"l1\",{nl}      \"Bus1\" : \"sourcebus\",{nl}"),
        &format!("\"Name\" : \"l1\",{nl}"),
    );
    assert_ne!(injected, j0, "injection point exists");
    let mut dss = Dss::new();
    let _ = dss.circuit_from_json(&injected);
    assert!(
        dss.errors()
            .iter()
            .any(|e| e.contains("required property not provided") && e.contains("Bus1")),
        "a missing Required property is a loud error, not a silent partial import: {:?}",
        dss.errors()
    );
}

#[test]
fn bus_kvln_kvll_conflict_aborts() {
    // Pascal `busFromJSON` raises `Both "kVLN" and "kVLL" were specified.`; with
    // no try/except in `Obj_Circuit_FromJSON_` it aborts the whole load via the C
    // wrapper. Oracle-confirmed (error 20230919). Must abort, not skip-and-continue.
    let j0 = tiny_circuit_json();
    let nl = JSON_LINE_BREAK;
    let injected = j0.replace(
        &format!("\"Name\" : \"sourcebus\"{nl}    }}"),
        &format!(
            "\"Name\" : \"sourcebus\",{nl}      \"kVLN\" : 7.2,{nl}      \"kVLL\" : 12.47{nl}    }}"
        ),
    );
    assert_ne!(injected, j0, "injection point exists");
    let mut dss = Dss::new();
    let r = dss.circuit_from_json(&injected);
    assert!(
        r.as_ref()
            .is_err_and(|e| e.contains("Both \"kVLN\" and \"kVLL\" were specified.")),
        "the kVLN+kVLL conflict aborts the load: {r:?}"
    );
}
