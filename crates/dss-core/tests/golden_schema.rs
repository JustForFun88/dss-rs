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
fn global_enum_defs_bytes_match_oracle() {
    let g = load_golden();
    let want = g["enum_defs"].as_object().expect("enum_defs object");
    let got = schema::global_enum_defs();

    // The 21 global enum `$defs` (`DSS.Enums`), in Pascal insertion order,
    // byte-for-byte — the full `prepareEnumJsonSchema` walk.
    let want_keys: Vec<&str> = g["enum_defs_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let got_keys: Vec<&str> = got.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        got_keys, want_keys,
        "global enum `$defs` key set / order drifted from the oracle"
    );

    for (name, value) in &got {
        let expected = want[name].as_str().unwrap();
        assert_eq!(
            &render(value),
            expected,
            "enum def `{name}` bytes differ from the oracle"
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

/// Load the expected-divergence inventory (`schema_divergences.json`).
fn load_divergences() -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "json",
        "schema_divergences.json",
    ]
    .iter()
    .collect();
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

/// Remove every top-level (4-space-indented) `"<key>" : { … }` property block
/// from a rendered class def, returning how many were removed. Used to strip a
/// port-only (r4133-adopted) property before byte-comparing against the 0.14.5
/// oracle. Brace-matched (string-aware) so nested objects don't confuse it; the
/// block's trailing `,\r\n` (or bare `\r\n` if it were the last member) goes too.
fn remove_property_blocks(s: &mut String, key: &str) -> usize {
    let anchor = format!("    \"{key}\" : {{");
    let mut count = 0;
    while let Some(pos) = s.find(&anchor) {
        let bytes = s.as_bytes();
        let brace = pos + anchor.len() - 1; // the opening '{'
        let (mut depth, mut in_str, mut esc) = (0i32, false, false);
        let mut end = None;
        let mut i = brace;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if in_str {
                if esc {
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    in_str = false;
                }
            } else if c == '"' {
                in_str = true;
            } else if c == '{' {
                depth += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            i += 1;
        }
        let mut tail = end.expect("unbalanced property block") + 1;
        if s[tail..].starts_with(",\r\n") {
            tail += 3;
        } else if s[tail..].starts_with("\r\n") {
            tail += 2;
        }
        s.replace_range(pos..tail, "");
        count += 1;
    }
    count
}

/// Decrement a `"<marker>" : N` field's value by the number of `removed` values
/// strictly less than `N`, on every line that carries it — the positional
/// renumbering the oracle would show after the port's extra properties are
/// dropped (`$dssPropertyIndex`/`$dssPropertyOrder` are dense ordinal ranks, so
/// removing a prop at ordinal `r` shifts every higher ordinal down by one).
fn renumber_field(s: &str, marker: &str, removed: &[i64]) -> String {
    let key = format!("\"{marker}\" : ");
    let mut out = String::with_capacity(s.len());
    for line in s.split_inclusive("\r\n") {
        if let Some(kpos) = line.find(&key) {
            let after = &line[kpos + key.len()..];
            let numend = after
                .find(|c: char| !c.is_ascii_digit() && c != '-')
                .unwrap_or(after.len());
            if let Ok(v) = after[..numend].parse::<i64>() {
                let shift = removed.iter().filter(|&&r| r < v).count() as i64;
                out.push_str(&line[..kpos + key.len()]);
                out.push_str(&(v - shift).to_string());
                out.push_str(&after[numend..]);
                continue;
            }
        }
        out.push_str(line);
    }
    out
}

/// The per-class walk (`prepareClassJsonSchema`): every ported class def is
/// byte-exact vs the pinned 0.14.5 oracle **after** removing exactly the
/// documented r4133 divergences (`schema_divergences.json`). Fails on any
/// unexplained byte diff AND on a stale inventory entry (a divergence whose
/// occurrence count no longer matches — i.e. the divergence is gone, or spread
/// further than recorded). Two divergence `kind`s are understood:
/// - `port_extra_line`: a single line (or embedded-CRLF block) the port emits
///   that the 0.14.5 oracle lacks; removed `count` times.
/// - `port_extra_property`: a whole port-only property block (keyed by
///   `prop_key`, at ordinal `index`/`order`) the port adopted from newer
///   dss_capi; removed `count` times, after which the trailing
///   `$dssPropertyIndex`/`$dssPropertyOrder` ordinals are renumbered down (the
///   positional cascade the extra property causes).
#[test]
fn ported_class_defs_bytes_match_oracle() {
    let g = load_golden();
    let want = g["class_defs"].as_object().expect("class_defs object");
    let order: Vec<&str> = g["class_defs_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let inv = load_divergences();
    let divergences = inv["divergences"].as_array().expect("divergences array");

    let dss = Dss::new();
    assert!(!order.is_empty(), "golden carries no ported class defs");

    for &name in &order {
        let def = dss
            .schema_class_def(name)
            .unwrap_or_else(|| panic!("class {name} not registered"));
        let mut rendered = render(&def);

        // Apply exactly the documented divergences for this class. Extra
        // properties are collected first (all blocks removed) so the ordinal
        // renumbering sees the full set of removed ranks in one pass.
        let mut removed_indices: Vec<i64> = Vec::new();
        let mut removed_orders: Vec<i64> = Vec::new();
        for d in divergences {
            if d["class"].as_str() != Some(name) {
                continue;
            }
            let count = d["count"].as_u64().expect("divergence count") as usize;
            match d["kind"].as_str() {
                Some("port_extra_line") => {
                    let line = d["line"].as_str().expect("divergence line");
                    let pat = format!("{line}\r\n");
                    let found = rendered.matches(&pat).count();
                    assert_eq!(
                        found, count,
                        "stale/incomplete divergence for {name}: expected {count} occurrence(s) \
                         of `{line}`, found {found} — update schema_divergences.json"
                    );
                    rendered = rendered.replace(&pat, "");
                }
                Some("port_extra_property") => {
                    let key = d["prop_key"]
                        .as_str()
                        .expect("port_extra_property prop_key");
                    let found = remove_property_blocks(&mut rendered, key);
                    assert_eq!(
                        found, count,
                        "stale port_extra_property for {name}.{key}: expected {count} block(s), \
                         found {found} — update schema_divergences.json"
                    );
                    removed_indices.push(d["index"].as_i64().expect("port_extra_property index"));
                    removed_orders.push(d["order"].as_i64().expect("port_extra_property order"));
                }
                other => panic!("unknown divergence kind {other:?} for {name}"),
            }
        }
        if !removed_indices.is_empty() {
            rendered = renumber_field(&rendered, "$dssPropertyIndex", &removed_indices);
            rendered = renumber_field(&rendered, "$dssPropertyOrder", &removed_orders);
        }

        let expected = want[name]
            .as_str()
            .unwrap_or_else(|| panic!("golden missing class_defs[{name}]"));
        assert_eq!(
            &rendered, expected,
            "class `{name}` bytes differ from the oracle (after divergences)"
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

    // Byte-level top-level member ORDER — serde's object map above ignores it,
    // so assert the emitted key sequence directly against the Pascal envelope
    // order (`CAPI_Schema.pas:1504-1513`): $schema, $id, $defs, type,
    // properties, required. `type`/`properties`/`required` also occur nested
    // inside `$defs`, so anchor each match to the top-level 2-space indent
    // (`\r\n  "key":`) — nested members sit at >=4 spaces. Catches an envelope
    // reordering that all the serde-parse checks would silently accept.
    let keys = ["$schema", "$id", "$defs", "type", "properties", "required"];
    let positions: Vec<usize> = keys
        .iter()
        .map(|k| {
            out.find(&format!("\r\n  \"{k}\" :"))
                .unwrap_or_else(|| panic!("top-level envelope key `{k}` not found at indent 0"))
        })
        .collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "envelope top-level member order drifted from the Pascal spec: {positions:?}"
    );
}
