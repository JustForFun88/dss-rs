//! Unit tests for the fpjson float formatter and the two writers. Expected
//! bytes are the pinned oracle's (dss-python 0.15.7, probed 2026-07-11 via
//! `lib.DSSElement_ToJSON` over an XYcurve whose `Yarray` carried the table).
//!
//! Since Stage F.4 the float spelling and the pretty line break are the lane's
//! (`compat::json_float`, `compat::JSON_LINE_BREAK`), so the oracle tables below
//! are asserted against the **parity impl** — which is compiled in both lanes —
//! and the layout tests build their expectation from the lane's line break. The
//! two lane rows themselves are pinned by [`json_float_is_the_lane_kernel`] and
//! [`json_line_break_is_the_lane_kernel`].

use super::*;
use crate::compat::{JSON_LINE_BREAK, ORACLE_PARITY, json_float};

/// The oracle's float table: `(value, the pinned oracle's bytes)`, captured from
/// its XYcurve dump.
const ORACLE_FLOATS: [(f64, &str); 10] = [
    (0.0, "0.0000000000000000E+000"),
    (1.0, "1.0000000000000000E+000"),
    (-1.0, "-1.0000000000000000E+000"),
    (12.47, "1.2470000000000001E+001"),
    (0.1, "1.0000000000000001E-001"),
    (1e-9, "1.0000000000000001E-009"),
    (1e30, "1.0000000000000000E+030"),
    (1e-30, "1.0000000000000001E-030"),
    (-0.0, "-0.0000000000000000E+000"),
    (123456789.12345679, "1.2345678912345679E+008"),
];

#[test]
fn fpjson_float_oracle_table() {
    for (v, expected) in ORACLE_FLOATS {
        assert_eq!(fpjson_float_fpc_impl(v), expected, "value {v}");
    }
}

/// The JSON float row at its observable: the seam selects the lane's spelling,
/// and both spellings read back to the identical `f64`.
///
/// The default column is the shortest round-tripping literal — the conventional
/// JSON spelling — including the scientific switch that keeps `1e30` from being
/// written as 31 digits, and the `.0` that keeps a whole-valued float from
/// re-reading as an integer (or `-0.0` from losing its sign).
#[test]
fn json_float_is_the_lane_kernel() {
    let shortest: [&str; 10] = [
        "0.0",
        "1.0",
        "-1.0",
        "12.47",
        "0.1",
        "1e-9",
        "1e30",
        "1e-30",
        "-0.0",
        "123456789.12345679",
    ];
    for (i, (v, oracle)) in ORACLE_FLOATS.into_iter().enumerate() {
        assert_eq!(json_float_shortest_impl(v), shortest[i], "value {v}");
        assert_eq!(
            json_float(v),
            if ORACLE_PARITY { oracle } else { shortest[i] },
            "the seam must select the lane's spelling for {v}"
        );
        // Same number, either way — the whole claim of the row.
        let back: f64 = shortest[i].parse().expect("shortest literal parses");
        assert_eq!(back.to_bits(), v.to_bits(), "round-trip of {v}");
    }
}

/// The pretty-writer line-break row at its observable: CRLF in the parity lane
/// (fpjson's Windows `sLineBreak`, which the byte goldens carry), a plain `\n`
/// in the default one — and *only* in pretty mode, since the compact writer
/// emits no whitespace at all.
#[test]
fn json_line_break_is_the_lane_kernel() {
    assert_eq!(JSON_LINE_BREAK, if ORACLE_PARITY { "\r\n" } else { "\n" });
    let tree = Json::Obj(vec![("a".into(), Json::Int(1))]);
    let mut pretty = String::new();
    write_pretty(&tree, 0, &mut pretty);
    assert_eq!(
        pretty,
        format!("{{{JSON_LINE_BREAK}  \"a\" : 1{JSON_LINE_BREAK}}}")
    );
    let mut compact = String::new();
    write_compact(&tree, &mut compact);
    assert_eq!(compact, r#"{"a":1}"#, "compact mode is lane-independent");
}

#[test]
fn escaping_matches_fpjson() {
    // fpjson StringToJSON: `"`->\", `\`->\\, `/` NOT escaped (probe-pinned).
    let mut out = String::new();
    escape_into("a/b\\c\"d", &mut out);
    assert_eq!(out, "\"a/b\\\\c\\\"d\"");

    // control chars: short escapes for BS/TAB/LF/FF/CR, `\uXXXX` otherwise.
    let mut out = String::new();
    escape_into("x\t\n\r\u{08}\u{0C}\u{01}", &mut out);
    assert_eq!(out, "\"x\\t\\n\\r\\b\\f\\u0001\"");
}

#[test]
fn compact_writer_layout() {
    let tree = Json::Obj(vec![
        ("Name".into(), Json::Str("l1".into())),
        ("kV".into(), Json::Float(12.47)),
        (
            "a".into(),
            Json::Arr(vec![
                Json::Int(1),
                Json::Arr(vec![Json::Int(2), Json::Int(3)]),
            ]),
        ),
        ("e".into(), Json::Arr(vec![])),
        ("b".into(), Json::Bool(true)),
        ("n".into(), Json::Null),
    ]);
    let mut out = String::new();
    write_compact(&tree, &mut out);
    assert_eq!(
        out,
        format!(
            r#"{{"Name":"l1","kV":{},"a":[1,[2,3]],"e":[],"b":true,"n":null}}"#,
            json_float(12.47)
        )
    );
}

#[test]
fn pretty_writer_layout() {
    // Matches fpjson FormatJSON([],2): "key" : value, one member/line, empty
    // containers inline, arrays fully expanded (probe-pinned: Line/Load pretty).
    let tree = Json::Obj(vec![
        ("Name".into(), Json::Str("t1".into())),
        (
            "Bus".into(),
            Json::Arr(vec![Json::Str("a".into()), Json::Str("b".into())]),
        ),
        ("e".into(), Json::Arr(vec![])),
        ("o".into(), Json::Obj(vec![])),
    ]);
    let mut out = String::new();
    write_pretty(&tree, 0, &mut out);
    // An empty container is open-bracket + break + parent-indent + close
    // (probe-pinned, not inline). The break itself is the lane's.
    let nl = JSON_LINE_BREAK;
    let expected = format!(
        "{{{nl}  \"Name\" : \"t1\",{nl}  \"Bus\" : [{nl}    \"a\",{nl}    \"b\"{nl}  ],\
         {nl}  \"e\" : [{nl}  ],{nl}  \"o\" : {{{nl}  }}{nl}}}"
    );
    assert_eq!(out, expected);
}

#[test]
fn nested_matrix_pretty_one_scalar_per_line() {
    // A 2x2 sym-matrix nests [[a,b],[c,d]] with each scalar on its own line
    // (probe-pinned: Line RMatrix pretty).
    let tree = Json::Obj(vec![(
        "RMatrix".into(),
        Json::Arr(vec![
            Json::Arr(vec![Json::Float(1.0), Json::Float(0.0)]),
            Json::Arr(vec![Json::Float(0.0), Json::Float(1.0)]),
        ]),
    )]);
    let mut out = String::new();
    write_pretty(&tree, 0, &mut out);
    let (nl, one, zero) = (JSON_LINE_BREAK, json_float(1.0), json_float(0.0));
    let expected = format!(
        "{{{nl}  \"RMatrix\" : [{nl}    [{nl}      {one},{nl}      {zero}{nl}    ],\
         {nl}    [{nl}      {zero},{nl}      {one}{nl}    ]{nl}  ]{nl}}}"
    );
    assert_eq!(out, expected);
}
