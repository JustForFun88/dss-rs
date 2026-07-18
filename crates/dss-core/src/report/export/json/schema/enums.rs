//! Per-enum JSON-schema walk — a loop-for-loop port of Pascal
//! `prepareEnumJsonSchema` (`.inputs/dss_capi/src/CAPI/CAPI_Schema.pas:111-162`)
//! plus the 21 **global** enums the schema emits (Pascal `DSS.Enums`, built in
//! `TDSSContext.Create`, `DSSClass.pas:1058-1196`).
//!
//! `DSS_ExtractJSONSchema` iterates `DSS.Enums` — a fixed, ordered list of 21
//! shared enums — and adds each to the top-level `$defs` under its `JSONName`
//! (`CAPI_Schema.pas:1476-1477`). Class-*local* enums (Load:Model, Generator:
//! Model, …) are NOT in that list; they are emitted inside their class def by
//! the class walk. The 21 here are ported directly from the Pascal `TDSSEnum.
//! Create` calls (names/ordinals/alt-names), with the symbolic ordinals resolved
//! from their Pascal enum declarations — never seeded from the oracle document.

use super::{Json, i, obj, s};

/// One global enum's schema-relevant data — the fields `prepareEnumJsonSchema`
/// reads off a `TDSSEnum` (`Name`, `AltNames`, `Names`, `Ordinals`,
/// `AltNamesValid`, `Hybrid`, `JSONUseNumbers`).
struct SchemaEnum {
    /// Pascal `TDSSEnum.Name` (also the `title`).
    name: &'static str,
    /// Pascal `Names` (the canonical string forms).
    names: &'static [&'static str],
    /// Pascal `Ordinals`.
    ordinals: &'static [i64],
    /// Pascal `AltNames` (JSON-friendly aliases). Same length as `names`; an
    /// empty string removes that option from the schema (`:122-123`). When the
    /// enum was built with the 2-arg constructor, `AltNames = Names` (`:2236`).
    alt_names: &'static [&'static str],
    /// Pascal `AltNamesValid` (default true). When false (CoreType only), the
    /// `enum` list uses `Names[i]`, but the `$dssFullEnum` mapping still carries
    /// `AltNames[i]` first (`:125-131`).
    alt_names_valid: bool,
    /// Pascal `Hybrid`: emit a `oneOf [string-enum | integer]` instead of a plain
    /// string enum (`:146-154`).
    hybrid: bool,
    /// Pascal `JSONUseNumbers`: emit an integer enum (`:134-144`). None of the 21
    /// global enums set it, but the flag is carried for fidelity.
    json_use_numbers: bool,
}

impl SchemaEnum {
    /// Pascal `JSONName` (`DSSClass.pas:2218-2221`): the `Name` with every space,
    /// `-`, and `:` removed. This is the `$defs` key.
    fn json_name(&self) -> String {
        enum_json_name(self.name)
    }

    /// Port of `prepareEnumJsonSchema(e, enumIds, prefixPath)`
    /// (`CAPI_Schema.pas:111-162`) — builds the enum's schema object. The
    /// `enumIds` side effect (registering the id) is tracked by the caller.
    fn to_json(&self) -> Json {
        render_enum(
            self.name,
            self.names,
            self.ordinals,
            self.alt_names,
            self.alt_names_valid,
            self.hybrid,
            self.json_use_numbers,
        )
    }
}

/// Pascal `TDSSEnum.JSONName` (`DSSClass.pas:2218-2221`): the enum `Name` with
/// every space, `-`, and `:` removed — the `$defs` key. Shared by the global
/// enum walk and the class-local walk (`schema/classes.rs`).
pub(super) fn enum_json_name(name: &str) -> String {
    name.replace([' ', '-', ':'], "")
}

/// The body of `prepareEnumJsonSchema` (`CAPI_Schema.pas:111-162`): build an
/// enum's schema object from its `Names`/`Ordinals`/`AltNames` + the
/// `AltNamesValid`/`Hybrid`/`JSONUseNumbers` metadata. `alt_names` must be the
/// same length as `names`; an empty AltName drops that option (`:122-123`).
/// Shared by [`SchemaEnum::to_json`] (the 21 globals) and the class-local walk.
pub(super) fn render_enum(
    name: &str,
    names: &[&str],
    ordinals: &[i64],
    alt_names: &[&str],
    alt_names_valid: bool,
    hybrid: bool,
    json_use_numbers: bool,
) -> Json {
    // `:117-132`: build the `names`/`values` arrays and the `$dssFullEnum`
    // mapping, skipping options whose AltName is empty.
    let mut enum_names = Vec::new();
    let mut values = Vec::new();
    let mut mapping = Vec::new();
    for idx in 0..alt_names.len() {
        let alt = alt_names[idx];
        if alt.is_empty() {
            continue;
        }
        let display = if alt_names_valid { alt } else { names[idx] };
        enum_names.push(s(display));
        values.push(i(ordinals[idx]));
        mapping.push(Json::Arr(vec![s(alt), s(names[idx]), i(ordinals[idx])]));
    }

    if json_use_numbers {
        // `:134-144`
        return obj(vec![
            ("title", s(name)),
            ("type", s("integer")),
            ("enum", Json::Arr(values)),
            ("$dssFullEnum", Json::Arr(mapping)),
        ]);
    }

    if hybrid {
        // `:146-154`
        return obj(vec![
            ("title", s(name)),
            (
                "oneOf",
                Json::Arr(vec![
                    obj(vec![("type", s("string")), ("enum", Json::Arr(enum_names))]),
                    obj(vec![("type", s("integer")), ("minimum", i(1))]),
                ]),
            ),
            ("$dssFullEnum", Json::Arr(mapping)),
        ]);
    }

    // `:156-161`
    obj(vec![
        ("title", s(name)),
        ("type", s("string")),
        ("enum", Json::Arr(enum_names)),
        ("$dssFullEnum", Json::Arr(mapping)),
    ])
}

/// Sugar: an enum whose `AltNames` default to `Names` (2-arg `TDSSEnum.Create`).
const fn plain(
    name: &'static str,
    names: &'static [&'static str],
    ordinals: &'static [i64],
) -> SchemaEnum {
    SchemaEnum {
        name,
        names,
        ordinals,
        alt_names: names,
        alt_names_valid: true,
        hybrid: false,
        json_use_numbers: false,
    }
}

/// The 21 global enums, in `DSS.Enums` insertion order
/// (`DSSClass.pas:1058-1196`). This is the order the schema adds them to `$defs`.
fn global_enums() -> Vec<SchemaEnum> {
    vec![
        plain(
            "Visualize: Quantity",
            &["Currents", "Voltages", "Powers"],
            &[1, 2, 3],
        ),
        plain(
            "Reduction Strategy",
            &[
                "Default",
                "ShortLines",
                "MergeParallel",
                "BreakLoop",
                "Dangling",
                "Switches",
                "Laterals",
            ],
            &[0, 1, 2, 3, 4, 5, 6],
        ),
        plain("Earth Model", &["Carson", "FullCarson", "Deri"], &[1, 2, 3]),
        plain(
            "Line Type",
            &[
                "oh",
                "ug",
                "ug_ts",
                "ug_cn",
                "swt_ldbrk",
                "swt_fuse",
                "swt_sect",
                "swt_rec",
                "swt_disc",
                "swt_brk",
                "swt_elbow",
                "busbar",
            ],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
        ),
        SchemaEnum {
            // Length Unit: the last two names (`meter`, `miles`) are aliases with
            // empty AltNames, so the schema drops them (`:1084-1087`).
            name: "Length Unit",
            names: &[
                "none", "mi", "kft", "km", "m", "ft", "in", "cm", "mm", "meter", "miles",
            ],
            ordinals: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 4, 1],
            alt_names: &[
                "none", "mi", "kft", "km", "m", "ft", "in", "cm", "mm", "", "",
            ],
            alt_names_valid: true,
            hybrid: false,
            json_use_numbers: false,
        },
        plain("Scan Type", &["None", "Zero", "Positive"], &[-1, 0, 1]),
        plain(
            "Sequence Type",
            &["Negative", "Zero", "Positive"],
            &[-1, 0, 1],
        ),
        SchemaEnum {
            name: "Connection",
            names: &["wye", "delta", "y", "ln", "ll"],
            ordinals: &[0, 1, 0, 0, 1],
            alt_names: &["Wye", "Delta", "", "", ""],
            alt_names_valid: true,
            hybrid: false,
            json_use_numbers: false,
        },
        SchemaEnum {
            // Core Type: AltNamesValid=false — the `enum` list uses Names, but the
            // mapping's first column still carries the AltNames (`:1103-1108`).
            name: "Core Type",
            names: &[
                "shell",
                "1-phase",
                "3-leg",
                "4-leg",
                "5-leg",
                "core-1-phase",
            ],
            ordinals: &[0, 1, 3, 4, 5, 9],
            alt_names: &[
                "Shell",
                "OnePhase",
                "ThreeLeg",
                "FourLeg",
                "FiveLeg",
                "CoreOnePhase",
            ],
            alt_names_valid: false,
            hybrid: false,
            json_use_numbers: false,
        },
        SchemaEnum {
            name: "Phase Sequence",
            names: &["Lag", "Lead", "ANSI", "Euro"],
            ordinals: &[0, 1, 0, 1],
            alt_names: &["Lag", "Lead", "", ""],
            alt_names_valid: true,
            hybrid: false,
            json_use_numbers: false,
        },
        plain("Load Solution Model", &["PowerFlow", "Admittance"], &[1, 2]),
        plain(
            "Random Type",
            &["None", "Gaussian", "Uniform", "LogNormal"],
            &[0, 1, 2, 3],
        ),
        plain(
            "Control Mode",
            &["Off", "Static", "Event", "Time", "MultiRate"],
            &[-1, 0, 1, 2, 3],
        ),
        plain("Inverter Control Mode", &["GFL", "GFM"], &[0, 1]),
        SchemaEnum {
            // Solution Mode: 26 names, the last 8 are compatibility aliases with
            // empty AltNames and are dropped (`:1142-1154`).
            name: "Solution Mode",
            names: &[
                "Snap",
                "Daily",
                "Yearly",
                "M1",
                "LD1",
                "PeakDay",
                "DutyCycle",
                "Direct",
                "MF",
                "FaultStudy",
                "M2",
                "M3",
                "LD2",
                "AutoAdd",
                "Dynamic",
                "Harmonic",
                "Time",
                "HarmonicT",
                "Snapshot",
                "Dynamics",
                "Harmonics",
                "S",
                "Y",
                "H",
                "T",
                "F",
            ],
            ordinals: &[
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 0, 14, 15, 0, 2, 15,
                16, 9,
            ],
            alt_names: &[
                "Snapshot",
                "Daily",
                "Yearly",
                "M1",
                "LD1",
                "PeakDay",
                "DutyCycle",
                "Direct",
                "MF",
                "FaultStudy",
                "M2",
                "M3",
                "LD2",
                "AutoAdd",
                "Dynamic",
                "Harmonic",
                "Time",
                "HarmonicT",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
            ],
            alt_names_valid: true,
            hybrid: false,
            json_use_numbers: false,
        },
        plain("Solution Algorithm", &["Normal", "Newton"], &[0, 1]),
        plain("Circuit Model", &["Multiphase", "Positive"], &[0, 1]),
        plain("AutoAdd Device Type", &["Generator", "Capacitor"], &[1, 2]),
        plain(
            "Load Shape Class",
            &["None", "Daily", "Yearly", "Duty"],
            &[-1, 0, 1, 2],
        ),
        SchemaEnum {
            name: "Monitored Phase",
            names: &["min", "max", "avg"],
            ordinals: &[-3, -2, -1],
            alt_names: &["min", "max", "avg"],
            alt_names_valid: true,
            hybrid: true,
            json_use_numbers: false,
        },
        SchemaEnum {
            name: "Plot: Profile Phases",
            names: &["Default", "All", "Primary", "LL3Ph", "LLAll", "LLPrimary"],
            ordinals: &[-1, -2, -3, -4, -5, -6],
            alt_names: &["Default", "All", "Primary", "LL3Ph", "LLAll", "LLPrimary"],
            alt_names_valid: true,
            hybrid: true,
            json_use_numbers: false,
        },
    ]
}

/// The 21 global enum `$defs` in Pascal insertion order, keyed by `JSONName`.
/// Spliced into `$defs` right after the ten static defs and before the class
/// defs (`CAPI_Schema.pas:1476-1477`).
pub fn global_enum_defs() -> Vec<(String, Json)> {
    global_enums()
        .into_iter()
        .map(|e| (e.json_name(), e.to_json()))
        .collect()
}

/// Whether `json_name` names one of the 21 top-level (`DSS.Enums`) global enums
/// — the class walk's test for whether a mapped-enum property refs a global
/// `$defs/<JSONName>` (registered in `enumIds` before the class loop) or emits a
/// class-local `$defs`. Pascal `enumIds.Find(aenum.JSONName) <> 0`
/// (`CAPI_Schema.pas:761-773`).
pub(super) fn is_global_enum_json_name(json_name: &str) -> bool {
    global_enums().iter().any(|e| e.json_name() == json_name)
}
