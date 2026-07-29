//! AltDSS JSON-schema byte golden (`CAPI_Schema.pas`,
//! `DSS_ExtractSchema(jsonSchema=True)`). The oracle emits a ~590 KB JSON-Schema
//! document; the Rust port reproduces the **whole** document (OG-1.5c). This
//! driver byte-gates it at three granularities, all through the same fpjson
//! pretty writer the oracle uses:
//!
//! - **fragments** (static `$defs`, the 21 global enum `$defs`, the
//!   `circuitProperties` head, and each ported per-class `$defs`) vs the
//!   oracle-captured golden `schema_static_core.json`; the per-class gate applies
//!   the documented r4133/0.15.x divergences (`schema_divergences.json`,
//!   fail-on-stale) first.
//! - **the full document** vs its own pinned PORT golden
//!   (`full_document_matches_port_golden`, a regression guard) and vs the
//!   verbatim ORACLE document (`full_document_reconciles_with_oracle`): every
//!   byte is accounted — the scaffolding is byte-identical to the oracle, each
//!   class region equals its per-class-gated `schema_class_def`, and the
//!   structural port-authored classes + WindGen are inventoried
//!   (`port_authored_classes`) and pinned by the PORT golden.
//! - **the class-set split** (`every_class_is_gated_or_inventoried`): every
//!   `DSS_CLASS_LIST_ORDER` class is byte-gated OR port-authored, never both.
//!
//! Regenerate the ORACLE side manually: `python tools/golden/gen_schema.py`.
//! Regenerate the PORT full-document golden: run the test with
//! `REGEN_SCHEMA_PORT=1` after a reviewed change.

use std::path::PathBuf;

use dss_core::exec::Dss;
use dss_core::report::export::json::schema;
use dss_core::report::export::json::{FPJSON_SPELLING, Json, write_pretty_with};
use serde_json::Value;

mod harness;
use harness::lane;

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
///
/// **Stage F.4:** deliberately rendered with [`FPJSON_SPELLING`] rather than the
/// lane's, in *both* lanes. This whole file is a **byte** gate whose subject is
/// the schema's own content — every property block, ordinal, enum and `$ref`, and
/// the exact line-oriented shape its divergence surgery below edits by literal
/// `\r\n`-terminated lines. The F-FMT rows (`compat::json_float`,
/// `compat::JSON_LINE_BREAK`) change only the *spelling* of that document, so
/// letting them vary here would trade a strong structural gate for a weaker one
/// and force ~23 line patterns to be written twice. Instead the spelling is
/// pinned to the oracle's and the default writer is held to the same document by
/// [`lane_spelling_renders_the_same_schema_document`] — the same "assert the
/// parity kernel in both lanes, then assert the default kernel against it" shape
/// `fmt_battery.rs` uses.
fn render(v: &Json) -> String {
    let mut out = String::new();
    write_pretty_with(FPJSON_SPELLING, v, 0, &mut out);
    out
}

/// The full `DSS_ExtractSchema` document in the oracle's spelling — the
/// document-level twin of [`render`], and for the same reason.
fn render_document(dss: &Dss) -> String {
    render(&dss.schema_document())
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
/// - `port_hidden_property`: a port-only property that is *also* `SUPPRESS_JSON`
///   in the port (renders no block) but occupies a `$dssPropertyIndex` ordinal
///   (`index`); only the following props' `$dssPropertyIndex` are renumbered down
///   (it is absent from `AltPropertyOrder`, so `$dssPropertyOrder` is untouched).
/// - `port_changed_line`: the port emits a different value for one line (`from`)
///   than the oracle (`to`) — an r4133-adopted default/semantics change; each of
///   `count` occurrences is rewritten to the oracle's line before comparing.
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
        let rendered = apply_divergences(render(&def), name, divergences);

        let expected = want[name]
            .as_str()
            .unwrap_or_else(|| panic!("golden missing class_defs[{name}]"));
        assert_eq!(
            &rendered, expected,
            "class `{name}` bytes differ from the oracle (after divergences)"
        );
    }
}

/// Apply exactly the documented per-property divergences for `name` to a rendered
/// class def (at indent 0), returning the transformed string. Panics
/// (fail-on-stale) if a divergence's occurrence count no longer matches. Shared by
/// the per-class byte gate and the full-document reconciliation.
fn apply_divergences(mut rendered: String, name: &str, divergences: &[Value]) -> String {
    // Extra/hidden properties are collected first (all blocks removed / counted) so
    // the ordinal renumbering sees the full set of removed ranks in one pass.
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
            Some("port_hidden_property") => {
                // A port-only property hidden from the schema output — either
                // `SUPPRESS_JSON` (Transformer/AutoTrans BH-curve; absent from
                // `AltPropertyOrder`, so only `$dssPropertyIndex` shifts) or
                // `HIDE_015X`/`HIDE_R4133` (Line's 0.15.x EpsRMedium/HeightOffset/
                // HeightUnit/Conductors; **kept** in `AltPropertyOrder`, so
                // `$dssPropertyOrder` shifts too — carried via the `order` field).
                // It renders NO top-level block but occupies an ordinal, shifting
                // every following prop down. Fail-on-stale: assert its block is
                // absent; if un-hidden later, the count breaks. The anchor is
                // indent-precise (leading CRLF + 4 spaces) so a same-named prop
                // nested in a `oneOf` (indent 8) — e.g. Line's `Wires`-as-
                // `Conductors` spec-set member vs the hidden top-level `Conductors`
                // — is not miscounted.
                let key = d["prop_key"]
                    .as_str()
                    .expect("port_hidden_property prop_key");
                let anchor = format!("\r\n    \"{key}\" : {{");
                let found = rendered.matches(&anchor).count();
                assert_eq!(
                    found, count,
                    "stale port_hidden_property for {name}.{key}: expected {count} block(s), \
                     found {found} — update schema_divergences.json"
                );
                removed_indices.push(d["index"].as_i64().expect("port_hidden_property index"));
                if let Some(o) = d["order"].as_i64() {
                    removed_orders.push(o);
                }
            }
            Some("port_changed_line") => {
                // The port emits a different value for a line than the 0.14.5
                // oracle (an r4133-adopted default/semantics change): replace
                // exactly `count` of the port's `from` lines with the oracle's
                // `to` line before comparing. Fail-on-stale via the count.
                let from = d["from"].as_str().expect("port_changed_line from");
                let to = d["to"].as_str().expect("port_changed_line to");
                let pat = format!("{from}\r\n");
                let found = rendered.matches(&pat).count();
                assert_eq!(
                    found, count,
                    "stale port_changed_line for {name}: expected {count} occurrence(s) of \
                     `{from}`, found {found} — update schema_divergences.json"
                );
                rendered = rendered.replace(&pat, &format!("{to}\r\n"));
            }
            other => panic!("unknown divergence kind {other:?} for {name}"),
        }
    }
    if !removed_indices.is_empty() {
        rendered = renumber_field(&rendered, "$dssPropertyIndex", &removed_indices);
    }
    if !removed_orders.is_empty() {
        rendered = renumber_field(&rendered, "$dssPropertyOrder", &removed_orders);
    }
    rendered
}

#[test]
fn skeleton_envelope_is_well_formed() {
    // The public Dss surface returns the same static core, independent of any
    // circuit state (no `New circuit` needed).
    let dss = Dss::new();
    let out = render_document(&dss);

    // Valid JSON with the expected envelope keys in order.
    let v: Value = serde_json::from_str(&out).expect("schema is valid JSON");
    assert_eq!(v["$schema"], "https://json-schema.org/draft/2020-12/schema");
    assert_eq!(v["$id"], schema::ALTDSS_SCHEMA_ID);
    assert_eq!(v["type"], "object");
    assert_eq!(v["required"], serde_json::json!(["Vsource"]));

    // The full document carries the ten static `$defs` + 21 global enum `$defs` +
    // per class a `<Class>`/`<Class>List`/`<Class>Container` triple. With 50
    // classes (49 oracle + WindGen) that is 10 + 21 + 50*3 = 181 `$defs`.
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
        assert!(defs.contains_key(name), "$defs missing static def {name}");
    }
    let n_classes = schema::DSS_CLASS_LIST_ORDER.len();
    assert_eq!(
        defs.len(),
        10 + 21 + n_classes * 3,
        "full-document $defs count wrong for {n_classes} classes"
    );
    // Every class contributes its def + List + Container + a circuitProperties ref.
    for &cls in schema::DSS_CLASS_LIST_ORDER {
        assert!(defs.contains_key(cls), "$defs missing class {cls}");
        assert!(
            defs.contains_key(&format!("{cls}List")),
            "$defs missing {cls}List"
        );
        assert!(
            defs.contains_key(&format!("{cls}Container")),
            "$defs missing {cls}Container"
        );
        assert!(
            v["properties"].get(cls).is_some(),
            "circuitProperties missing {cls} ref"
        );
    }
    // `required: ["Vsource"]` resolves — the Vsource `$defs` is present, and its
    // `VsourceList` carries the extra `minLength: 1` (`CAPI_Schema.pas:1499`).
    assert_eq!(v["$defs"]["VsourceList"]["minLength"], 1);

    // CRLF line breaks (fpjson Windows RTL), matching the oracle goldens.
    assert!(out.contains("\r\n"), "fpjson pretty uses CRLF");

    // Byte-level top-level member ORDER: $schema, $id, $defs, type, properties,
    // required (`CAPI_Schema.pas:1504-1513`). Anchor each to the top-level 2-space
    // indent (`\r\n  "key":`) — nested members sit at >=4 spaces.
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

// ---- Full-document splice: the port emits the whole DSS_ExtractSchema document,
// byte-gated two ways: (A) a pinned PORT golden (regression), (B) reconciliation
// against the verbatim oracle document (scaffolding byte-identical; each class
// region == the per-class-gated `schema_class_def`; port-only WindGen +
// structural port-authored classes inventoried). --------------------------------

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

fn read_golden(name: &str) -> String {
    let path = json_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Slice the top-level `$defs` members of a full schema document into ordered
/// `(key, raw-value-bytes)` pairs — the fpjson value at its natural host indent
/// (a top-level `$defs` member sits at indent 4). String-aware brace/bracket
/// matching (mirrors `gen_schema.py::slice_class_def`).
fn slice_defs(doc: &str) -> Vec<(String, String)> {
    let defs_anchor = "\r\n  \"$defs\" : {";
    let open = doc.find(defs_anchor).expect("$defs block") + defs_anchor.len() - 1;
    let bytes = doc.as_bytes();
    let end = match_bracket(bytes, open);
    let mut regions: Vec<(String, String)> = Vec::new();
    let mut i = open + 1;
    // A member key opens at exactly indent 4 (`\r\n    "`); the `\r\n  }` closer
    // sits at indent 2, so restricting the search to `[i, end)` and to the 4-space
    // opener naturally excludes it.
    while let Some(k) = doc[i..end].find("\r\n    \"") {
        let kstart = i + k + "\r\n    \"".len();
        let kend = kstart + doc[kstart..].find('"').expect("key end");
        let key = doc[kstart..kend].to_string();
        let vstart = kend + doc[kend..].find(" : ").expect("colon") + 3;
        let vend = match bytes[vstart] {
            b'{' | b'[' => match_bracket(bytes, vstart),
            _ => vstart + doc[vstart..].find("\r\n").unwrap_or(end - vstart),
        };
        regions.push((key, doc[vstart..vend].to_string()));
        i = vend;
    }
    regions
}

/// Index just past the `}`/`]` that closes the bracket opening at `open`.
fn match_bracket(bytes: &[u8], open: usize) -> usize {
    let close = if bytes[open] == b'{' { b'}' } else { b']' };
    let (mut depth, mut in_str, mut esc) = (0i32, false, false);
    let mut i = open;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            match c {
                _ if esc => esc = false,
                b'\\' => esc = true,
                b'"' => in_str = false,
                _ => {}
            }
        } else if c == b'"' {
            in_str = true;
        } else if c == bytes[open] {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
        i += 1;
    }
    panic!("unbalanced bracket");
}

/// Dedent a top-level `$defs` region (host indent 4) to indent 0 — every line
/// after the first loses its leading 4 spaces, matching what `write_pretty(def,
/// 0)` (and `gen_schema.py::slice_class_def`) produce.
fn dedent(region: &str) -> String {
    let mut out = String::with_capacity(region.len());
    for (i, line) in region.split("\r\n").enumerate() {
        if i > 0 {
            out.push_str("\r\n");
        }
        if i > 0 && line.starts_with("    ") {
            out.push_str(&line[4..]);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// The document the **engine** actually emits (`Dss::extract_schema_json`, the
/// lane's spelling) is the same document the byte gates above check in the
/// oracle's spelling.
///
/// This is what keeps [`render`]'s decision — pin the spelling to fpjson's in
/// both lanes — from quietly turning the whole file into a parity-only gate:
/// every number is compared bit-for-bit and every structural token and string
/// verbatim, so the only thing the default lane is allowed to do differently is
/// spell a float and break a line. In the parity lane the two renders are
/// byte-identical and `compare_json` says so directly.
#[test]
fn lane_spelling_renders_the_same_schema_document() {
    let dss = Dss::new();
    lane::compare_json(&render_document(&dss), &dss.extract_schema_json(), "schema");
}

/// (A) The full port document is byte-identical to its pinned golden — a
/// regression guard over the whole `DSS_ExtractSchema` output (envelope, all
/// `$defs`, `<Class>List`/`<Class>Container` triples, `circuitProperties`).
/// Regenerate deliberately with `REGEN_SCHEMA_PORT=1` after a reviewed change.
#[test]
fn full_document_matches_port_golden() {
    let out = render_document(&Dss::new());
    let path = json_dir().join("schema_full_port.json");
    if std::env::var("REGEN_SCHEMA_PORT").is_ok() {
        std::fs::write(&path, &out).unwrap();
        return;
    }
    let golden = read_golden("schema_full_port.json");
    assert_eq!(
        out, golden,
        "full port schema document drifted from schema_full_port.json — if intended, \
         review and regenerate with REGEN_SCHEMA_PORT=1"
    );
}

/// (B) The full port document reconciles with the verbatim oracle document
/// (`schema_full_oracle.json`), accounting for EVERY byte:
/// - the `$defs` key sequence equals the oracle's with WindGen's three keys
///   inserted right after `GeneratorContainer`;
/// - the scaffolding `$defs` (10 static + 21 enum + every `<Class>List`/
///   `<Class>Container`) and every shared `circuitProperties` ref are
///   BYTE-IDENTICAL to the oracle (proves the splice + the `VsourceList` minLength);
/// - every class def region, dedented, equals `schema_class_def` (proves the splice
///   placed the per-class-gated bytes) AND, for a byte-gated class, equals the
///   oracle region after its documented divergences (a direct in-document oracle
///   byte compare); the structural port-authored classes + WindGen are inventoried
///   (`port_authored_classes`) and pinned by test (A) instead.
#[test]
fn full_document_reconciles_with_oracle() {
    let dss = Dss::new();
    let port = render_document(&dss);
    let oracle = read_golden("schema_full_oracle.json");
    let inv = load_divergences();
    let divergences = inv["divergences"].as_array().expect("divergences array");

    let p_defs = slice_defs(&port);
    let o_defs = slice_defs(&oracle);
    let p_prop_regions = slice_props(&port);
    let o_prop_regions = slice_props(&oracle);

    let statics = [
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
    ];
    let g = load_golden();
    let enum_keys: Vec<&str> = g["enum_defs_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let port_authored: Vec<&str> = inv["port_authored_classes"]
        .as_array()
        .expect("port_authored_classes array")
        .iter()
        .map(|e| e["class"].as_str().expect("class"))
        .collect();
    let windgen_keys = ["WindGen", "WindGenList", "WindGenContainer"];

    // 1) `$defs` key sequence: port keys minus WindGen's three == oracle keys.
    let p_keys: Vec<&str> = p_defs.iter().map(|(k, _)| k.as_str()).collect();
    let o_keys: Vec<&str> = o_defs.iter().map(|(k, _)| k.as_str()).collect();
    let p_stripped: Vec<&str> = p_keys
        .iter()
        .copied()
        .filter(|k| !windgen_keys.contains(k))
        .collect();
    assert_eq!(
        p_stripped, o_keys,
        "$defs key sequence (minus WindGen) diverged from the oracle"
    );
    // WindGen's three keys sit immediately after `GeneratorContainer`.
    let gpos = p_keys
        .iter()
        .position(|&k| k == "GeneratorContainer")
        .unwrap();
    assert_eq!(
        &p_keys[gpos + 1..gpos + 4],
        &windgen_keys,
        "WindGen $defs not spliced right after GeneratorContainer"
    );

    // 2) circuitProperties keys: port minus WindGen == oracle.
    let p_props_stripped: Vec<&str> = p_prop_regions
        .iter()
        .map(|(k, _)| k.as_str())
        .filter(|k| *k != "WindGen")
        .collect();
    let o_props_ref: Vec<&str> = o_prop_regions.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        p_props_stripped, o_props_ref,
        "circuitProperties key sequence (minus WindGen) diverged from the oracle"
    );

    // 3) Per-region checks.
    let o_map: std::collections::HashMap<&str, &str> = o_defs
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    for (key, p_region) in &p_defs {
        let k = key.as_str();
        if windgen_keys.contains(&k) {
            // Port-only WindGen triple: `WindGen` def == its `schema_class_def`;
            // the List/Container follow the identical scaffolding formula, proven
            // by substituting the name into the oracle's `Generator` scaffolding.
            if k == "WindGen" {
                let cd = render(&dss.schema_class_def("WindGen").expect("WindGen def"));
                assert_eq!(dedent(p_region), cd, "spliced WindGen != schema_class_def");
            } else {
                let base = k.strip_prefix("WindGen").unwrap(); // "List" | "Container"
                let expected = o_map[&*format!("Generator{base}")].replace("Generator", "WindGen");
                assert_eq!(
                    p_region, &expected,
                    "WindGen{base} does not follow the oracle scaffolding formula"
                );
            }
            continue;
        }

        let o_region = o_map[k];
        let is_scaffold = statics.contains(&k)
            || enum_keys.contains(&k)
            || k.ends_with("List")
            || k.ends_with("Container");
        if is_scaffold {
            assert_eq!(
                p_region, o_region,
                "scaffolding $defs `{k}` is not byte-identical to the oracle"
            );
            continue;
        }

        // A class def. The splice must place exactly `schema_class_def`'s bytes.
        let cd = render(
            &dss.schema_class_def(k)
                .unwrap_or_else(|| panic!("{k} not registered")),
        );
        assert_eq!(
            dedent(p_region),
            cd,
            "spliced class `{k}` != schema_class_def (splice corrupted the class def)"
        );
        if port_authored.contains(&k) {
            // Structural port-authored (r4133/0.15.x): NOT byte-gated vs the
            // 0.14.5 oracle — pinned by `full_document_matches_port_golden`.
            continue;
        }
        // Byte-gated: direct in-document oracle byte compare (after divergences).
        assert_eq!(
            apply_divergences(dedent(p_region), k, divergences),
            dedent(o_region),
            "class `{k}` (in full document) differs from the oracle after divergences"
        );
    }

    // 4) Every shared circuitProperties ref is byte-identical to the oracle; the
    // WindGen ref follows the same `{$ref: #/$defs/<Class>Container}` formula.
    let op: std::collections::HashMap<&str, &str> = o_prop_regions
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    for (k, region) in &p_prop_regions {
        if k == "WindGen" {
            assert_eq!(
                region, "{\r\n      \"$ref\" : \"#/$defs/WindGenContainer\"\r\n    }",
                "WindGen circuitProperties ref malformed"
            );
        } else {
            assert_eq!(
                region,
                op[k.as_str()],
                "circuitProperties ref `{k}` is not byte-identical to the oracle"
            );
        }
    }
}

/// Slice the `properties` (circuitProperties) members into ordered `(key, value
/// bytes at indent 4)` pairs.
fn slice_props(doc: &str) -> Vec<(String, String)> {
    let props_anchor = "\r\n  \"properties\" : {";
    let open = doc.find(props_anchor).expect("properties block") + props_anchor.len() - 1;
    let bytes = doc.as_bytes();
    let end = match_bracket(bytes, open);
    let mut out = Vec::new();
    let mut i = open + 1;
    while let Some(k) = doc[i..end].find("\r\n    \"") {
        let ks = i + k + "\r\n    \"".len();
        let ke = ks + doc[ks..].find('"').unwrap();
        let key = doc[ks..ke].to_string();
        let vs = ke + doc[ke..].find(" : ").unwrap() + 3;
        let ve = match bytes[vs] {
            b'{' | b'[' => match_bracket(bytes, vs),
            _ => vs + doc[vs..].find("\r\n").unwrap(),
        };
        out.push((key, doc[vs..ve].to_string()));
        i = ve;
    }
    out
}

/// The class-walk order splits cleanly into the byte-gated set (`gen_schema.py`
/// `SCHEMA_CLASSES`, captured in the golden's `class_defs_order`) and the
/// structural port-authored set (`schema_divergences.json::port_authored_classes`),
/// with no overlap and no omission. Fail-on-stale: adding a class to
/// `DSS_CLASS_LIST_ORDER` without gating or inventorying it — or leaving a stale
/// port-authored entry — breaks this.
#[test]
fn every_class_is_gated_or_inventoried() {
    let g = load_golden();
    let gated: Vec<&str> = g["class_defs_order"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let inv = load_divergences();
    let authored: Vec<&str> = inv["port_authored_classes"]
        .as_array()
        .expect("port_authored_classes array")
        .iter()
        .map(|e| e["class"].as_str().expect("class"))
        .collect();

    for &cls in schema::DSS_CLASS_LIST_ORDER {
        let in_gated = gated.contains(&cls);
        let in_authored = authored.contains(&cls);
        assert!(
            in_gated ^ in_authored,
            "class `{cls}` must be in exactly one of SCHEMA_CLASSES (byte-gated) or \
             port_authored_classes — gated={in_gated}, authored={in_authored}"
        );
    }
    // No stale entries: every listed class exists in the walk order.
    for cls in gated.iter().chain(authored.iter()) {
        assert!(
            schema::DSS_CLASS_LIST_ORDER.contains(cls),
            "stale inventory: `{cls}` is not in DSS_CLASS_LIST_ORDER"
        );
    }
    assert_eq!(
        gated.len() + authored.len(),
        schema::DSS_CLASS_LIST_ORDER.len(),
        "gated + port-authored must cover every class exactly once"
    );
}
