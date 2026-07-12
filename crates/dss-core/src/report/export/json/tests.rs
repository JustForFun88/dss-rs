//! Unit tests for the fpjson float formatter and the two writers. Expected
//! bytes are the pinned oracle's (dss-python 0.15.7, probed 2026-07-11 via
//! `lib.DSSElement_ToJSON` over an XYcurve whose `Yarray` carried the table).

use super::*;

#[test]
fn fpjson_float_oracle_table() {
    // (value, oracle bytes) — captured from the pinned oracle's XYcurve dump.
    let cases: [(f64, &str); 10] = [
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
    for (v, expected) in cases {
        assert_eq!(fpjson_float(v), expected, "value {v}");
    }
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
        r#"{"Name":"l1","kV":1.2470000000000001E+001,"a":[1,[2,3]],"e":[],"b":true,"n":null}"#
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
    // fpjson pretty uses the Windows RTL line break (CRLF); an empty container
    // is open-bracket + break + parent-indent + close (probe-pinned, not inline).
    let expected = "{\r\n  \"Name\" : \"t1\",\r\n  \"Bus\" : [\r\n    \"a\",\r\n    \"b\"\r\n  ],\r\n  \"e\" : [\r\n  ],\r\n  \"o\" : {\r\n  }\r\n}";
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
    let expected = concat!(
        "{\r\n",
        "  \"RMatrix\" : [\r\n",
        "    [\r\n",
        "      1.0000000000000000E+000,\r\n",
        "      0.0000000000000000E+000\r\n",
        "    ],\r\n",
        "    [\r\n",
        "      0.0000000000000000E+000,\r\n",
        "      1.0000000000000000E+000\r\n",
        "    ]\r\n",
        "  ]\r\n",
        "}"
    );
    assert_eq!(out, expected);
}
