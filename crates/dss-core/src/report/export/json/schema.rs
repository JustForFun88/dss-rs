//! Port of the AltDSS JSON-schema emitter — Pascal `DSS_ExtractJSONSchema`
//! (`.inputs/dss_capi/src/CAPI/CAPI_Schema.pas:1252-1521`), the
//! `DSS_ExtractSchema(DSS, jsonSchema=True)` surface. The emitter walks every
//! registered DSS class + enum and emits a JSON-Schema (draft 2020-12) document
//! from the property metadata.
//!
//! ## Scope of this port (read before extending)
//! Only the **static, metadata-independent core** is ported here and it is
//! byte-verified against the pinned oracle (`tools/golden/gen_schema.py`):
//!
//! - the schema envelope — `$schema`, [`ALTDSS_SCHEMA_ID`] `$id`, `type`,
//!   `required` (`CAPI_Schema.pas:1504-1513`);
//! - the ten reusable global `$defs` (`Complex`, `PComplex`, `SymmetricMatrix`,
//!   `ArrayOrFilePath`, `StringArrayOrFilePath`, `JSONFilePath`,
//!   `JSONLinesFilePath`, `Bus`, `BusConnection`, `DynInitType`)
//!   (`:1270-1459`), added to `$defs` **before** the enum/class loop, so they
//!   are the first ten keys;
//! - the static head of `circuitProperties` — `Name`, `DefaultBaseFreq`,
//!   `PreCommands`, `PostCommands`, `Bus` (`:1463-1474`).
//!
//! ## Deliberately NOT ported here (blocked on unported metadata — see STATUS
//! §OG-1.5): the per-class walk (`prepareClassJsonSchema`, `:325-1134`) and the
//! per-enum walk (`prepareEnumJsonSchema`, `:111-162`), plus the
//! `<Class>`/`<Class>List`/`<Class>Container` `$defs` triples and the per-class
//! `circuitProperties` refs (`:1476-1502`). Those need per-property metadata the
//! Rust port never carried: property **help/description** text
//! (`GetPropertyHelp`; ~1109 strings), the class **`AltPropertyOrder`**
//! (`$dssPropertyOrder`), the **`SpecSets`** (`oneOf`), the enum
//! **`AltNames`/`JSONName`/`JSONUseNumbers`** JSON metadata, and ~28 of the ~30
//! `Units_*` property flags (only `UNITS_HOUR`/`UNITS_OHM_PER_LENGTH` exist in
//! [`PropFlags`](crate::obj::props::PropFlags)). Porting that database is a
//! large, self-contained follow-up recorded in `ORPHANED_GAPS.md` §1.5.

use super::{Json, write_pretty};

/// Pascal `ALTDSS_SCHEMA_ID` (`CAPI_Schema.pas:12`).
pub const ALTDSS_SCHEMA_ID: &str =
    "https://dss-extensions.org/altdss-schema/2023-12-13.schema.json";

/// The draft the schema declares (`CAPI_Schema.pas:1505`).
const JSON_SCHEMA_DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";

// --- tiny ordered-tree constructors (local sugar over `Json`) -----------------

fn obj(members: Vec<(&str, Json)>) -> Json {
    Json::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    )
}
fn s(v: &str) -> Json {
    Json::Str(v.to_string())
}
fn i(v: i64) -> Json {
    Json::Int(v)
}
fn b(v: bool) -> Json {
    Json::Bool(v)
}

/// A `{"type": <t>}` leaf.
fn typ(t: &str) -> Json {
    obj(vec![("type", s(t))])
}

/// A file-path string leaf: `{"type":"string","format":"file-path"}`.
fn file_path() -> Json {
    obj(vec![("type", s("string")), ("format", s("file-path"))])
}

/// A `{"title","type":"object","properties":{k:file_path},"required":[k]}`
/// wrapper — the shape shared by `JSONFilePath`/`JSONLinesFilePath` and the
/// single-key `ArrayOrFilePath` variants (`CAPI_Schema.pas:1311-1382`).
fn single_file_prop(title: Option<&str>, key: &str) -> Json {
    let mut members = Vec::new();
    if let Some(t) = title {
        members.push(("title", s(t)));
    }
    members.push(("type", s("object")));
    members.push(("properties", obj(vec![(key, file_path())])));
    members.push(("required", Json::Arr(vec![s(key)])));
    obj(members)
}

/// The ten reusable global `$defs`, in the Pascal insertion order — a
/// byte-faithful port of `CAPI_Schema.pas:1270-1459`. Returned as ordered
/// `(key, value)` pairs so the caller can splice them at the head of `$defs`.
pub fn global_defs() -> Vec<(String, Json)> {
    // Complex / PComplex: a 2-element number array (rectangular / polar).
    let complex_like = |comment: &str| {
        obj(vec![
            ("type", s("array")),
            ("items", typ("number")),
            ("minItems", i(2)),
            ("maxItems", i(2)),
            ("$comment", s(comment)),
        ])
    };

    // ArrayOrFilePath: oneOf [inline float array | CSV | Dbl file | Sng file].
    let array_or_file_path = obj(vec![(
        "oneOf",
        Json::Arr(vec![
            obj(vec![
                ("title", s("FloatArray")),
                ("type", s("array")),
                ("items", typ("number")),
            ]),
            obj(vec![
                ("title", s("FloatArrayFromCSV")),
                ("type", s("object")),
                (
                    "properties",
                    obj(vec![
                        ("CSVFile", file_path()),
                        (
                            "Column",
                            obj(vec![
                                ("type", s("integer")),
                                ("default", i(1)),
                                ("exclusiveMinimum", i(0)),
                            ]),
                        ),
                        (
                            "Header",
                            obj(vec![("type", s("boolean")), ("default", b(false))]),
                        ),
                    ]),
                ),
                ("required", Json::Arr(vec![s("CSVFile")])),
            ]),
            single_file_prop(Some("FloatArrayFromDbl"), "DblFile"),
            single_file_prop(Some("FloatArrayFromSng"), "SngFile"),
        ]),
    )]);

    // StringArrayOrFilePath: oneOf [inline string array | file].
    let string_array_or_file_path = obj(vec![(
        "oneOf",
        Json::Arr(vec![
            obj(vec![
                ("title", s("StringArray")),
                ("type", s("array")),
                (
                    "items",
                    obj(vec![("type", s("string")), ("minLength", i(1))]),
                ),
            ]),
            single_file_prop(Some("StringArrayFromFile"), "File"),
        ]),
    )]);

    // Bus: named point with X/Y and mutually-exclusive kVLN/kVLL.
    let named_number = |title: &str, extra: Vec<(&str, Json)>| {
        let mut m = vec![("title", s(title)), ("type", s("number"))];
        m.extend(extra);
        obj(m)
    };
    let bus = obj(vec![
        ("type", s("object")),
        (
            "properties",
            obj(vec![
                (
                    "Name",
                    obj(vec![
                        ("title", s("Name")),
                        ("type", s("string")),
                        ("minLength", i(1)),
                        ("maxLength", i(255)),
                    ]),
                ),
                ("X", named_number("X", vec![])),
                ("Y", named_number("Y", vec![])),
                (
                    "kVLN",
                    named_number("kVLN", vec![("exclusiveMinimum", i(0))]),
                ),
                (
                    "kVLL",
                    named_number("kVLL", vec![("exclusiveMinimum", i(0))]),
                ),
                (
                    "Keep",
                    obj(vec![
                        ("title", s("Keep")),
                        ("type", s("boolean")),
                        ("default", b(false)),
                    ]),
                ),
            ]),
        ),
        (
            "anyOf",
            Json::Arr(vec![
                obj(vec![
                    ("required", Json::Arr(vec![s("kVLN")])),
                    ("not", obj(vec![("required", Json::Arr(vec![s("kVLL")]))])),
                ]),
                obj(vec![
                    ("required", Json::Arr(vec![s("kVLL")])),
                    ("not", obj(vec![("required", Json::Arr(vec![s("kVLN")]))])),
                ]),
                obj(vec![(
                    "not",
                    obj(vec![("required", Json::Arr(vec![s("kVLN"), s("kVLL")]))]),
                )]),
            ]),
        ),
        ("required", Json::Arr(vec![s("Name")])),
    ]);

    // DynInitType: free-form {string|number} value map.
    let dyn_init_type = obj(vec![
        ("title", s("DynInitType")),
        ("type", s("object")),
        (
            "additionalProperties",
            obj(vec![(
                "anyOf",
                Json::Arr(vec![typ("string"), typ("number")]),
            )]),
        ),
    ]);

    vec![
        (
            "Complex".to_string(),
            complex_like(
                "A **rectangular** complex number represented as an array of two \
                 floating-point numbers, real and imaginary parts.",
            ),
        ),
        (
            "PComplex".to_string(),
            complex_like(
                "A **polar** complex number represented as an array of two \
                 floating-point numbers, magnitude and angle parts. Angle in \
                 degrees.",
            ),
        ),
        (
            "SymmetricMatrix".to_string(),
            obj(vec![
                ("title", s("SymmetricMatrix")),
                ("type", s("array")),
                (
                    "items",
                    obj(vec![("type", s("array")), ("items", typ("number"))]),
                ),
            ]),
        ),
        ("ArrayOrFilePath".to_string(), array_or_file_path),
        (
            "StringArrayOrFilePath".to_string(),
            string_array_or_file_path,
        ),
        (
            "JSONFilePath".to_string(),
            single_file_prop(Some("JSONFilePath"), "JSONFile"),
        ),
        (
            "JSONLinesFilePath".to_string(),
            single_file_prop(Some("JSONLinesFilePath"), "JSONLinesFile"),
        ),
        ("Bus".to_string(), bus),
        (
            "BusConnection".to_string(),
            obj(vec![
                ("type", s("string")),
                ("pattern", s(r"[^.]+(\.[0-9]+)*")),
            ]),
        ),
        ("DynInitType".to_string(), dyn_init_type),
    ]
}

/// The static head of `circuitProperties` — `CAPI_Schema.pas:1463-1474`. The
/// per-class refs (`:1501`) are appended by the (unported) class walk.
pub fn circuit_properties_head() -> Vec<(String, Json)> {
    let str_array = || obj(vec![("type", s("array")), ("items", typ("string"))]);
    vec![
        (
            "Name".to_string(),
            obj(vec![
                ("title", s("Name")),
                ("type", s("string")),
                ("minLength", i(1)),
                ("maxLength", i(255)),
            ]),
        ),
        (
            "DefaultBaseFreq".to_string(),
            obj(vec![
                ("title", s("DefaultBaseFreq")),
                ("type", s("number")),
                ("exclusiveMinimum", i(0)),
                ("$comment", s("Dynamic default.")),
            ]),
        ),
        ("PreCommands".to_string(), str_array()),
        ("PostCommands".to_string(), str_array()),
        (
            "Bus".to_string(),
            obj(vec![
                ("type", s("array")),
                ("items", obj(vec![("$ref", s("#/$defs/Bus"))])),
                ("default", Json::Arr(vec![])),
            ]),
        ),
    ]
}

/// Assemble the schema **envelope** with the static `$defs` and the static
/// `circuitProperties` head — the ported portion of `DSS_ExtractJSONSchema`
/// (`CAPI_Schema.pas:1504-1513`). The per-class/enum `$defs` entries and the
/// per-class `circuitProperties` refs are NOT emitted (see the module doc):
/// this is the schema skeleton, not the full model dump.
pub fn schema_skeleton() -> Json {
    obj(vec![
        ("$schema", s(JSON_SCHEMA_DRAFT)),
        ("$id", s(ALTDSS_SCHEMA_ID)),
        ("$defs", Json::Obj(global_defs())),
        ("type", s("object")),
        ("properties", Json::Obj(circuit_properties_head())),
        // Pascal `'required', TJSONArray.Create(['Vsource'])` (`:1512`). The
        // `Vsource` class def + ref are part of the deferred class walk.
        ("required", Json::Arr(vec![s("Vsource")])),
    ])
}

/// Serialize [`schema_skeleton`] with the fpjson pretty layout (`FormatJSON()`,
/// 2-space indent, CRLF) — the exact serialization step of Pascal
/// `DSS_GetAsPAnsiChar(DSS, schema.FormatJSON())` (`CAPI_Schema.pas:1516`).
pub fn extract_schema_skeleton_json() -> String {
    let mut out = String::new();
    write_pretty(&schema_skeleton(), 0, &mut out);
    out
}
