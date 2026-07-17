//! AltDSS JSON-schema STATIC-CORE byte golden (`CAPI_Schema.pas`,
//! `DSS_ExtractSchema(jsonSchema=True)`). The oracle emits a ~590 KB
//! JSON-Schema document; the Rust port reproduces its **static core** (the
//! schema envelope + the ten reusable global `$defs` + the static
//! `circuitProperties` head). This driver renders each static fragment through
//! the same fpjson pretty writer the oracle uses and asserts **byte-equality**
//! against the oracle-captured golden.
//!
//! The per-class/enum `$defs` walk is deferred (blocked on unported per-property
//! metadata — help text, `AltPropertyOrder`, `SpecSets`, enum JSON names, most
//! `Units_*` flags; see STATUS §OG-1.5). The golden records that deferred
//! inventory (class/enum def names) for the follow-up but does not gate it.
//!
//! Regenerate only manually: `python tools/golden/gen_schema.py`.

use std::path::PathBuf;

use dss_core::exec::Dss;
use dss_core::report::export::json::schema;
use dss_core::report::export::json::{Json, write_pretty};
use serde_json::Value;

fn golden_path() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "json",
        "schema_static_core.json",
    ]
    .iter()
    .collect()
}

fn load_golden() -> Value {
    let path = golden_path();
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Render a `$defs`/`properties` value alone at indent 0 — exactly what the
/// golden stores (see `gen_schema.py`).
fn render(v: &Json) -> String {
    let mut out = String::new();
    write_pretty(v, 0, &mut out);
    out
}

#[test]
fn schema_identity_matches_oracle() {
    let g = load_golden();
    assert_eq!(
        schema::ALTDSS_SCHEMA_ID,
        g["schema_id"].as_str().unwrap(),
        "$id drifted from the oracle"
    );
    assert_eq!(
        g["schema_draft"].as_str().unwrap(),
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(g["required"], serde_json::json!(["Vsource"]));
}

#[test]
fn global_defs_bytes_match_oracle() {
    let g = load_golden();
    let want = g["global_defs"].as_object().expect("global_defs object");
    let got = schema::global_defs();

    // Every oracle static def is reproduced, in the same order, byte-for-byte.
    // (`global_defs_order` carries the oracle insertion order — a serde_json
    // `Value` object map does not preserve it.)
    let want_keys: Vec<&str> = g["global_defs_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let got_keys: Vec<&str> = got.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        got_keys, want_keys,
        "static $defs key set / order drifted from the oracle"
    );

    for (name, value) in &got {
        let expected = want[name].as_str().unwrap();
        assert_eq!(
            &render(value),
            expected,
            "global def `{name}` bytes differ from the oracle"
        );
    }
}

#[test]
fn circuit_properties_head_bytes_match_oracle() {
    let g = load_golden();
    let want = g["circuit_head"].as_object().expect("circuit_head object");
    let got = schema::circuit_properties_head();

    let want_keys: Vec<&str> = g["circuit_head_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let got_keys: Vec<&str> = got.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(got_keys, want_keys, "circuitProperties head order drifted");

    for (name, value) in &got {
        let expected = want[name].as_str().unwrap();
        assert_eq!(
            &render(value),
            expected,
            "circuit head prop `{name}` bytes differ from the oracle"
        );
    }
}

#[test]
fn skeleton_envelope_is_well_formed() {
    // The public Dss surface returns the same static core, independent of any
    // circuit state (no `New circuit` needed).
    let dss = Dss::new();
    let out = dss.extract_schema_json();

    // Valid JSON with the expected envelope keys in order.
    let v: Value = serde_json::from_str(&out).expect("skeleton is valid JSON");
    assert_eq!(v["$schema"], "https://json-schema.org/draft/2020-12/schema");
    assert_eq!(v["$id"], schema::ALTDSS_SCHEMA_ID);
    assert_eq!(v["type"], "object");
    assert_eq!(v["required"], serde_json::json!(["Vsource"]));

    // The static `$defs` are exactly the ten reusable globals (the deferred
    // class/enum walk would add more — this is the skeleton).
    let defs = v["$defs"].as_object().unwrap();
    for name in [
        "Complex",
        "PComplex",
        "SymmetricMatrix",
        "ArrayOrFilePath",
        "StringArrayOrFilePath",
        "JSONFilePath",
        "JSONLinesFilePath",
        "Bus",
        "BusConnection",
        "DynInitType",
    ] {
        assert!(defs.contains_key(name), "skeleton $defs missing {name}");
    }
    assert_eq!(
        defs.len(),
        10,
        "skeleton must carry only the 10 static defs"
    );

    // CRLF line breaks (fpjson Windows RTL), matching the oracle goldens.
    assert!(out.contains("\r\n"), "fpjson pretty uses CRLF");
}
