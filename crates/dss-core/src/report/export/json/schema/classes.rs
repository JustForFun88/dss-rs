//! Per-class JSON-schema walk — a loop-for-loop port of Pascal
//! `prepareClassJsonSchema` (`.inputs/dss_capi/src/CAPI/CAPI_Schema.pas:325-1134`).
//!
//! For one registered class this builds the `$defs/<Class>` object: the `Name`
//! head, every non-suppressed property rendered from its [`PropDef`] metadata +
//! a live all-default *sample object*, the `SpecSets` → `oneOf` block, the
//! `required` list, and the class-**local** enum `$defs`. The reusable global
//! `$defs`, the 21 global enum `$defs`, and the `<Class>List`/`<Class>Container`
//! triples are assembled by the caller (`CAPI_Schema.pas:1479-1502`).
//!
//! The property *type* → JSON-type mapping ([`pascal_jtype`]) reconstructs the
//! Pascal `PropertyTypeJson[TPropertyType]` table from the port's merged
//! [`PropType`] + flags (e.g. a `Double` with [`PropFlags::ON_ARRAY`] is Pascal's
//! `DoubleOnStructArrayProperty`). Defaults are read through the same accessors
//! the text/JSON dumps use; the elision rules (`NaN`/`Inf`/empty/`NoDefault`/
//! read-only) are the Pascal ones.

use crate::obj::base::DssObject;
use crate::obj::dss_enum::{DssEnum, EnumRegistry};
use crate::obj::props::{ClassProps, PropDef, PropFlags, PropType};
use crate::report::help_catalog::dss_help;

use super::enums::{enum_json_name, is_global_enum_json_name, render_enum};
use super::spec_sets::spec_sets;
use super::{Json, b, i, obj, s};

/// Pascal `PropertyTypeJson[ptype]` + `AnsiEndsStr('Array', stype)` — the JSON
/// type string for a property and whether its Pascal type *name* ends in
/// `Array` (the `onArray`/scalar-default guards, `CAPI_Schema.pas:327-385/509`).
/// The port merges several Pascal `TPropertyType`s into one [`PropType`] +
/// flags; `ON_ARRAY` is handled by the caller's `onArray` logic, never here, so
/// the scalar `jtype` (`'number'`/`'integer'`) is returned unchanged.
fn pascal_jtype(pd: &PropDef) -> (&'static str, bool) {
    match pd.ptype {
        PropType::Double => ("number", false),
        PropType::Integer => ("integer", false),
        PropType::Boolean | PropType::Enabled => ("boolean", false),
        PropType::String => ("string", false),
        PropType::MakeLike => ("string", false),
        PropType::Action => ("-", false), // StringEnumActionProperty
        PropType::DoubleArray => ("#/$defs/ArrayOrFilePath", true),
        PropType::DoublePoints => ("#/$defs/ArrayOrFilePath", true), // DoubleDArrayProperty
        PropType::DoubleVArray => ("numberArray", true),
        PropType::DoubleFArray => ("numberArray", true),
        PropType::SymMatrixReal | PropType::SymMatrixImag => ("#/$defs/SymmetricMatrix", false),
        PropType::DoubleSymMatrix => ("#/$defs/SymmetricMatrix", false),
        PropType::IntegerArray => ("integerArray", true),
        PropType::StringList => ("#/$defs/StringArrayOrFilePath", false),
        PropType::ObjectRef => ("-", false), // DSSObjectReferenceProperty
        PropType::ObjectRefArray => ("stringArray", true),
        PropType::DoubleArrayOnStruct => ("numberArray", true),
        PropType::Complex => ("#/$defs/Complex", false),
        PropType::Bus => ("#/$defs/BusConnection", false),
        PropType::MappedStringEnum | PropType::MappedIntEnum => ("-", false),
        PropType::MappedStringEnumArray => ("-", true),
        PropType::EnumArrayOnStruct => ("-", true),
        PropType::BusOnStruct => ("#/$defs/BusConnection", true),
        PropType::BusesOnStruct => ("#/$defs/BusConnectionArray", true),
    }
}

/// Pascal `extractUnits(flags)` (`CAPI_Schema.pas:165-308`): the unit string for
/// the first set `Units_*` flag, in exactly the Pascal check order (the first
/// match wins). `None` when no unit flag is set.
fn extract_units(f: PropFlags) -> Option<&'static str> {
    // Order mirrors the Pascal `if ... Exit` chain 1:1 (first match wins).
    const TABLE: &[(PropFlags, &str)] = &[
        (PropFlags::UNITS_HZ, "Hz"),
        (PropFlags::UNITS_PU_VOLTAGE, "pu (voltage)"),
        (PropFlags::UNITS_PU_CURRENT, "pu (current)"),
        (PropFlags::UNITS_PU_POWER, "pu (power)"),
        (PropFlags::UNITS_PU_IMPEDANCE, "pu (impedance)"),
        (PropFlags::UNITS_OHM_METER, "\u{03a9}m"),
        (PropFlags::UNITS_OHM, "\u{03a9}"),
        (PropFlags::UNITS_OHM_PER_LENGTH, "\u{03a9}/[length_unit]"),
        (PropFlags::UNITS_NF_PER_LENGTH, "nF/[length_unit]"),
        (PropFlags::UNITS_UF, "\u{03bc}F"),
        (PropFlags::UNITS_MH, "mH"),
        (PropFlags::UNITS_US_PER_LENGTH, "\u{03bc}S/[length_unit]"),
        (PropFlags::UNITS_S, "s"),
        (PropFlags::UNITS_HOUR, "hour"),
        (PropFlags::UNITS_TOD_HOUR, "ToD-hour"),
        (PropFlags::UNITS_MINUTE, "minute"),
        (PropFlags::UNITS_V, "V"),
        (PropFlags::UNITS_W, "W"),
        (PropFlags::UNITS_KW, "kW"),
        (PropFlags::UNITS_KVAR, "kvar"),
        (PropFlags::UNITS_KVA, "kVA"),
        (PropFlags::UNITS_MVA, "MVA"),
        (PropFlags::UNITS_KWH, "kWh"),
        (PropFlags::UNITS_V_PER_KM, "V/km"),
        (PropFlags::UNITS_DEG, "\u{00b0}"),
        (PropFlags::UNITS_DEGC, "\u{00b0}C"),
        (PropFlags::UNITS_A, "A"),
        (PropFlags::UNITS_KV, "kV"),
    ];
    TABLE
        .iter()
        .find(|&&(flag, _)| f.contains(flag))
        .map(|&(_, u)| u)
}

/// The schema-relevant slice of a `TDSSEnum` for a mapped-enum property — the
/// port's [`DssEnum`] fields (`Names`/`Ordinals`/`Sequential`/`Hybrid`/min/max)
/// plus the JSON-only `AltNames`/`AltNamesValid`/`JSONUseNumbers` metadata the
/// [`DssEnum`] does not carry. For the enums whose AltNames equal their Names
/// (the 2-arg Pascal constructor) the defaults below reproduce the schema
/// exactly; the ones that drop aliases or renumber override via [`enum_meta`].
struct EnumMeta {
    name: &'static str,
    names: Vec<&'static str>,
    ordinals: Vec<i64>,
    alt_names: Vec<&'static str>,
    alt_names_valid: bool,
    sequential: bool,
    min_ordinal: i64,
    max_ordinal: i64,
    hybrid: bool,
    json_use_numbers: bool,
}

impl EnumMeta {
    fn json_name(&self) -> String {
        enum_json_name(self.name)
    }

    fn render(&self) -> Json {
        render_enum(
            self.name,
            &self.names,
            &self.ordinals,
            &self.alt_names,
            self.alt_names_valid,
            self.hybrid,
            self.json_use_numbers,
        )
    }

    /// Pascal `TDSSEnum.OrdinalToJSONValue` (`DSSClass.pas:2374-2413`): the JSON
    /// value for an ordinal — sequential enums use `AltNames`, non-sequential use
    /// `Names`; out-of-range → integer (hybrid) or null.
    fn ordinal_to_json_value(&self, value: i64) -> Json {
        if value < self.min_ordinal || value > self.max_ordinal {
            return if self.hybrid {
                Json::Int(value)
            } else {
                Json::Null
            };
        }
        if self.sequential {
            let k = (value - self.min_ordinal) as usize;
            return self.alt_names.get(k).map_or(Json::Null, |a| s(a));
        }
        for (k, &ord) in self.ordinals.iter().enumerate() {
            if ord == value {
                return s(self.names[k]);
            }
        }
        if self.hybrid {
            Json::Int(value)
        } else {
            Json::Null
        }
    }
}

/// Build the [`EnumMeta`] for a mapped-enum property's [`DssEnum`], applying the
/// `AltNames`/`AltNamesValid`/`JSONUseNumbers` overrides for the enums that need
/// them (default: `AltNames == Names`, valid, string). Sourced from the Pascal
/// enum declarations, never from the oracle JSON.
fn enum_meta(en: &DssEnum) -> EnumMeta {
    let names: Vec<&'static str> = en.names.clone();
    let ordinals: Vec<i64> = en.ordinals.iter().map(|&o| o as i64).collect();
    // Default: AltNames = Names, valid, string enum (2-arg `TDSSEnum.Create`).
    let (alt_names, alt_names_valid, json_use_numbers) =
        enum_overrides(en.name).unwrap_or_else(|| (names.clone(), true, false));
    EnumMeta {
        name: en.name,
        names,
        ordinals,
        alt_names,
        alt_names_valid,
        sequential: en.sequential,
        min_ordinal: en.min_ordinal as i64,
        max_ordinal: en.max_ordinal as i64,
        hybrid: en.hybrid,
        json_use_numbers,
    }
}

/// `AltNames`/`AltNamesValid`/`JSONUseNumbers` overrides for the class-local
/// enums whose schema metadata differs from the `AltNames == Names` default
/// (alias drops, renumbering, integer enums). Keyed by the Pascal `TDSSEnum.Name`.
/// Empty for the pilot classes (their locals use the default); class batches
/// extend it from the Pascal enum declarations as their classes need it.
#[allow(clippy::match_single_binding)] // extension point: class batches add arms
fn enum_overrides(name: &str) -> Option<(Vec<&'static str>, bool, bool)> {
    match name {
        _ => None,
    }
}

/// Pascal `indexOfIn(propIndex, AltPropertyOrder)` (`CAPI_Schema.pas:310-323`) —
/// the `$dssPropertyOrder`: the 1-based position of `prop_index` in the class's
/// `AltPropertyOrder`, or -1 if absent. The port's `alt_property_order()` omits
/// the leading `[0]=Name` slot, so the Pascal 1-based index is `position + 1`.
fn index_of_in(order: &[usize], prop_index: usize) -> i64 {
    order
        .iter()
        .position(|&p| p == prop_index)
        .map_or(-1, |p| (p + 1) as i64)
}

/// Port of `prepareClassJsonSchema(cls, enumIds)` — build the `$defs/<Class>`
/// schema object from the class's property table and a live all-default sample
/// object. `class_name` is the Pascal `cls.Name` (== `cls.Class_Name`).
pub(crate) fn class_schema(
    class_name: &str,
    props: &ClassProps,
    sample: &dyn DssObject,
    enums: &EnumRegistry,
) -> Json {
    let n = props.num_properties();
    let order = props.alt_property_order();

    // `props`/`requiredProps`/`localEnums` accumulate as we walk; `prop_json`
    // keeps each built property by index for the SpecSets clone step.
    let mut members: Vec<(String, Json)> = Vec::new();
    let mut required_props: Vec<String> = vec!["Name".to_string()];
    let mut local_enums: Vec<(String, Json)> = Vec::new();
    let mut prop_json: Vec<Option<Json>> = vec![None; n + 1];

    members.push((
        "Name".to_string(),
        obj(vec![
            ("title", s("Name")),
            ("type", s("string")),
            ("minLength", i(1)),
            ("maxLength", i(255)),
            ("$dssPropertyOrder", i(0)),
            ("$dssPropertyIndex", i(0)),
        ]),
    ));

    for prop_index_ in 1..=n {
        let pd0 = props.prop(prop_index_);
        let flags0 = pd0.flags;
        // Skip redundant / suppressed / struct-index props (`:476-482`).
        if flags0.contains(PropFlags::SUPPRESS_JSON)
            || flags0.contains(PropFlags::ALT_INDEX)
            || flags0.contains(PropFlags::INTEGER_STRUCT_INDEX)
            || flags0.contains(PropFlags::REDUNDANT)
        {
            continue;
        }

        let prop_name = pd0.name; // title (original name, pre-redirect)
        let prop_name_json = pd0.json_key(false);

        let mut zorder = index_of_in(order, prop_index_);
        // Array-alternative redirect (`:485-495`): render the alternative's
        // metadata but keep the original name/index.
        let prop_index = if pd0.array_alternative != 0 {
            let pi = pd0.array_alternative;
            let z_alt = index_of_in(order, pi);
            if z_alt != -1 {
                zorder = z_alt;
            }
            pi
        } else {
            prop_index_
        };
        let pd = props.prop(prop_index);
        let flags = pd.flags;

        let units = extract_units(flags);
        let read_only = flags.contains(PropFlags::SILENT_READ_ONLY);
        let no_default =
            flags.contains(PropFlags::NO_DEFAULT) || flags.contains(PropFlags::DYNAMIC_DEFAULT);

        let (mut jtype, stype_ends_array) = pascal_jtype(pd);
        let on_array = ((jtype == "-") && stype_ends_array)
            || jtype.ends_with("Array")
            || matches!(
                jtype,
                "#/$defs/ArrayOrFilePath"
                    | "#/$defs/StringArrayOrFilePath"
                    | "#/$defs/SymmetricMatrix"
            )
            || flags.contains(PropFlags::ON_ARRAY);

        let mut prop: Vec<(&'static str, Json)> = Vec::new();

        if on_array && jtype != "-" {
            // ---- array-valued property (`:518-670`) ------------------------
            let mut jtype_single = jtype;
            if matches!(jtype, "#/$defs/ArrayOrFilePath" | "#/$defs/SymmetricMatrix") {
                jtype_single = "number";
            } else if jtype == "#/$defs/StringArrayOrFilePath" {
                jtype_single = "string";
            } else if jtype == "#/$defs/BusConnectionArray" {
                jtype_single = "#/$defs/BusConnection";
                jtype = "array";
            } else if flags.contains(PropFlags::ON_ARRAY) {
                jtype = "array";
            } else {
                jtype_single = &jtype[..jtype.len() - "Array".len()];
                jtype = "array";
            }

            if jtype.starts_with('#') {
                prop.push(("$ref", s(jtype)));
            } else {
                prop.push(("type", s(jtype)));
            }

            if jtype == "array" {
                let mut subprop: Vec<(&str, Json)> = if jtype_single.starts_with('#') {
                    vec![("$ref", s(jtype_single))]
                } else {
                    vec![("type", s(jtype_single))]
                };
                if pd.trap_zero != 0.0 {
                    subprop.push(("exclusiveMinimum", i(0)));
                }
                prop.push(("items", obj(subprop)));
            }

            if !(read_only || no_default)
                && let Some(def) = array_default(pd, sample, prop_index, jtype_orig_is_matrix(pd))
            {
                prop.push(("default", def));
            }
        } else if jtype != "-" {
            // ---- scalar-valued property (`:672-747`) -----------------------
            if jtype.starts_with('#') {
                prop.push(("$ref", s(jtype)));
            } else {
                prop.push(("type", s(jtype)));
            }

            let elide_scalar = read_only || stype_ends_array || no_default;
            match jtype {
                "number" => {
                    if !elide_scalar {
                        let d = scalar_double(pd, sample, prop_index);
                        if d.is_finite() {
                            prop.push(("default", Json::Float(d)));
                        }
                    }
                }
                "integer" => {
                    if !elide_scalar {
                        prop.push((
                            "default",
                            Json::Int(scalar_integer(pd, sample, prop_index) as i64),
                        ));
                    }
                }
                "boolean" => {
                    if !elide_scalar {
                        prop.push(("default", b(sample.get_bool(prop_index))));
                    }
                }
                "string" | "#/$defs/BusConnection" => {
                    if !elide_scalar {
                        // MakeLike's `GetString` is `''` (Pascal), so its default
                        // is always elided; the port has no `get_string` for it.
                        let ds = if pd.ptype == PropType::MakeLike {
                            String::new()
                        } else {
                            sample.get_string(prop_index)
                        };
                        if !ds.is_empty() && !ds.starts_with("sample_for_defaults") {
                            prop.push(("default", s(&ds)));
                        }
                    }
                }
                "#/$defs/Complex" => {
                    // Note: no `noDefault` guard for Complex (`:717`).
                    if !(read_only || stype_ends_array) {
                        let (re, im) = sample.get_complex(prop_index);
                        if re.is_finite() && im.is_finite() {
                            prop.push((
                                "default",
                                Json::Arr(vec![Json::Float(re), Json::Float(im)]),
                            ));
                        }
                    }
                }
                _ => {}
            }

            if prop_name == "DynOut" || prop_name == "WdgCurrents" {
                prop.push(("$comment", s("TODO: use array instead of string")));
            }

            // NonNegative / PowerFactorLimits / zero-trap bounds (`:732-746`).
            if flags.contains(PropFlags::NON_NEGATIVE) {
                if pd.trap_zero != 0.0 || flags.contains(PropFlags::NON_ZERO) {
                    prop.push(("exclusiveMinimum", i(0)));
                } else {
                    prop.push(("minimum", i(0)));
                }
            } else if flags.contains(PropFlags::POWER_FACTOR_LIMITS) {
                prop.push(("minimum", i(-1)));
                prop.push(("maximum", i(1)));
            } else if pd.trap_zero != 0.0 || flags.contains(PropFlags::NON_ZERO) {
                prop.push(("exclusiveMinimum", i(0)));
            }
        }

        // ---- enum / object-reference chain (`:750-905`) --------------------
        if matches!(
            pd.ptype,
            PropType::MappedStringEnum
                | PropType::MappedIntEnum
                | PropType::MappedStringEnumArray
                | PropType::EnumArrayOnStruct
                | PropType::Action
        ) {
            let en = enums.get(pd.enum_id.expect("mapped-enum property needs an enum"));
            let meta = enum_meta(en);
            let jname = meta.json_name();
            let enum_path = if is_global_enum_json_name(&jname) {
                format!("#/$defs/{jname}")
            } else {
                if !local_enums.iter().any(|(k, _)| *k == jname) {
                    local_enums.push((jname.clone(), meta.render()));
                }
                format!("#/$defs/{class_name}/$defs/{jname}")
            };

            if !on_array {
                prop.push(("$ref", s(&enum_path)));
                match pd.ptype {
                    PropType::MappedIntEnum => {
                        let v = scalar_integer(pd, sample, prop_index) as i64;
                        if v >= meta.min_ordinal && v <= meta.max_ordinal {
                            prop.push(("default", Json::Int(v)));
                        }
                    }
                    PropType::MappedStringEnum => {
                        let v = sample.get_i32(prop_index) as i64;
                        prop.push(("default", meta.ordinal_to_json_value(v)));
                    }
                    _ => {}
                }
            } else {
                prop.push(("type", s("array")));
                prop.push(("items", obj(vec![("$ref", s(&enum_path))])));
                // (array enum defaults are struct-array specific; batch classes)
            }
        }

        // ---- onArray positional metadata (`:907-947`) ----------------------
        if on_array {
            if pd.ptype == PropType::DoubleFArray {
                prop.push(("minItems", i(pd.size_prop as i64)));
                prop.push(("maxItems", i(pd.size_prop as i64)));
            } else {
                let length_prop = sizing_property_index(pd);
                if length_prop > 0 {
                    if jtype_orig_is_matrix(pd) {
                        let nm = props.prop(length_prop).json_key(false);
                        prop.push(("$dssShape", Json::Arr(vec![s(&nm), s(&nm)])));
                    } else {
                        prop.push(("$dssLength", s(&props.prop(length_prop).json_key(false))));
                    }
                }
                if prop_index != prop_index_ {
                    prop.push(("$dssScalarProperty", s(props.property_name(prop_index_))));
                    prop.push(("$dssArrayProperty", s(props.property_name(prop_index))));
                }
            }
        }

        // ---- description / flags / units / title / indices (`:967-1021`) ---
        let help_key = format!(
            "{class_name}.{}",
            props.property_name(prop_index).to_ascii_lowercase()
        );
        prop.push(("description", s(dss_help(&help_key))));

        if flags.contains(PropFlags::DEPRECATED) {
            prop.push(("deprecated", b(true)));
        }
        // Pascal `ptype in [BooleanActionProperty, StringEnumActionProperty,
        // MakeLikeProperty]` → writeOnly (`:978`). The port's `Action`/`MakeLike`
        // cover the last two; a `BooleanActionProperty` is a `Boolean` + the
        // `BOOLEAN_ACTION` marker.
        if matches!(pd.ptype, PropType::Action | PropType::MakeLike)
            || flags.contains(PropFlags::BOOLEAN_ACTION)
        {
            prop.push(("writeOnly", b(true)));
        }
        if read_only {
            prop.push(("readOnly", b(true)));
        }
        if let Some(u) = units {
            if u == "ToD-hour" {
                prop.push(("minimum", i(0)));
                prop.push(("exclusiveMaximum", i(24)));
                prop.push(("units", s("hour")));
            } else {
                prop.push(("units", s(u)));
            }
        }

        prop.push(("title", s(prop_name)));

        if flags.contains(PropFlags::IS_FILENAME) {
            prop.push(("format", s("file-path")));
            let length_prop = sizing_property_index(pd);
            if length_prop > 0 {
                prop.push(("$dssLength", s(&props.prop(length_prop).json_key(false))));
            }
        }

        prop.push(("$dssPropertyIndex", i(prop_index_ as i64)));
        prop.push(("$dssPropertyOrder", i(zorder)));
        if flags.contains(PropFlags::VALUE_OFFSET) {
            prop.push(("$dssValueOffset", Json::Float(pd.value_offset)));
        }

        let prop_obj = obj(prop);
        prop_json[prop_index] = Some(prop_obj.clone());
        members.push((prop_name_json.clone(), prop_obj));
        if flags.contains(PropFlags::REQUIRED) {
            required_props.push(prop_name_json);
        }
    }

    // ---- SpecSets → oneOf + toRemove (`:1030-1104`) ------------------------
    let sets = spec_sets(class_name);
    let mut one_of: Vec<Json> = Vec::new();
    let mut to_remove: Vec<String> = Vec::new();
    if !sets.is_empty() {
        for set in sets {
            let mut spec_props: Vec<(String, Json)> = Vec::new();
            let mut required_in_spec: Vec<String> = Vec::new();
            let mut aborted = false;
            for &member in set.props {
                let Some(mut pi) = props.property_index(member) else {
                    aborted = true;
                    break;
                };
                if prop_json[pi].is_none() {
                    let apd = props.prop(pi);
                    if apd.array_alternative != 0 {
                        pi = apd.array_alternative;
                    }
                }
                let Some(built) = prop_json.get(pi).and_then(|o| o.clone()) else {
                    aborted = true;
                    break;
                };
                let key = props.prop(pi).json_key(false);
                spec_props.push((key.clone(), built));
                if !to_remove.contains(&key) {
                    to_remove.push(key.clone());
                }
                if props
                    .prop(pi)
                    .flags
                    .contains(PropFlags::REQUIRED_IN_SPEC_SET)
                {
                    required_in_spec.push(key);
                }
            }
            if aborted {
                continue;
            }
            let mut spec = vec![
                ("title", s(set.name)),
                ("type", s("object")),
                ("properties", Json::Obj(spec_props)),
            ];
            if !required_in_spec.is_empty() {
                spec.push((
                    "required",
                    Json::Arr(required_in_spec.into_iter().map(|k| s(&k)).collect()),
                ));
            }
            one_of.push(obj(spec));
        }
        members.retain(|(k, _)| !to_remove.contains(k));
    }

    // ---- assemble (`:1115-1132`) -------------------------------------------
    let mut result = vec![
        ("title", s(class_name)),
        ("type", s("object")),
        ("properties", Json::Obj(members)),
    ];
    if !one_of.is_empty() {
        result.push(("oneOf", Json::Arr(one_of)));
    }
    if !required_props.is_empty() {
        result.push((
            "required",
            Json::Arr(required_props.into_iter().map(|k| s(&k)).collect()),
        ));
    }
    if !local_enums.is_empty() {
        result.push(("$defs", Json::Obj(local_enums)));
    }
    obj(result)
}

/// Whether the property's *original* Pascal type is a `SymmetricMatrix`
/// (`ComplexPartSymMatrix`/`DoubleSymMatrix`) — the `$dssShape` vs `$dssLength`
/// and matrix-vs-array default split.
fn jtype_orig_is_matrix(pd: &PropDef) -> bool {
    matches!(
        pd.ptype,
        PropType::SymMatrixReal | PropType::SymMatrixImag | PropType::DoubleSymMatrix
    )
}

/// Pascal `PropertySizingPropertyIndex[propIndex]` (`getSizePropertyIndex`) — the
/// 1-based index of the integer property holding this array's length, or 0 if
/// the length is computed by function (`SizeIsFunction`, e.g. `DoubleVArray`).
fn sizing_property_index(pd: &PropDef) -> usize {
    match pd.ptype {
        PropType::DoubleArray
        | PropType::IntegerArray
        | PropType::DoubleSymMatrix
        | PropType::SymMatrixReal
        | PropType::SymMatrixImag
        | PropType::DoubleArrayOnStruct
        | PropType::EnumArrayOnStruct
        | PropType::BusesOnStruct => pd.size_prop,
        // Pascal `getSizePropertyIndex`'s `GlobalCount`/`IndirectCount` branches:
        // a `String`/file property counted by the class's `NPts`. The port carries
        // that count index in `size_prop` (set on the shape classes' file props).
        _ if pd.flags.contains(PropFlags::GLOBAL_COUNT) => pd.size_prop,
        _ => 0,
    }
}

/// Pascal `obj.GetDouble(propIndex)` (`GetObjDouble`): the scalar value scaled
/// out (and inverted under `InverseValue`).
fn scalar_double(pd: &PropDef, obj: &dyn DssObject, idx: usize) -> f64 {
    let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
        obj.prop_scale(idx, true)
    } else {
        pd.scale
    };
    let raw = obj.get_f64(idx);
    if pd.flags.contains(PropFlags::INVERSE_VALUE) {
        1.0 / (raw / scale)
    } else {
        raw / scale
    }
}

/// Pascal `obj.GetInteger(propIndex)`: the integer value less `PropertyValueOffset`
/// under `ValueOffset`.
fn scalar_integer(pd: &PropDef, obj: &dyn DssObject, idx: usize) -> i32 {
    let mut v = obj.get_i32(idx);
    if pd.flags.contains(PropFlags::VALUE_OFFSET) {
        v -= pd.value_offset.round_ties_even() as i32;
    }
    v
}

fn scaled(v: f64, scale: f64) -> f64 {
    if scale == 1.0 { v } else { v / scale }
}

/// The array/matrix default (`:561-668`): read the sample object's array, elide
/// on empty / any non-finite value, else render (matrix as order×order rows,
/// plain array flat), scaled. `idx` is the resolved property index.
fn array_default(pd: &PropDef, obj: &dyn DssObject, idx: usize, is_matrix: bool) -> Option<Json> {
    let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
        obj.prop_scale(idx, true)
    } else {
        pd.scale
    };

    if is_matrix {
        // ComplexPartSymMatrix / DoubleSymMatrix: order×order, scaled. Rows are
        // built exactly as `GetObjPropertyJSONValue` does (matching the dump).
        let (vals, order) = match pd.ptype {
            PropType::SymMatrixReal | PropType::SymMatrixImag => {
                let real = pd.ptype == PropType::SymMatrixReal;
                let (v, order) = obj.get_matrix_part(idx, real)?;
                // column-major flat: element (row i, col j) = v[j*order + i].
                (
                    (0..order * order)
                        .map(|k| v[(k % order) * order + k / order])
                        .collect::<Vec<_>>(),
                    order,
                )
            }
            PropType::DoubleSymMatrix => {
                let order = obj.get_i32(pd.size_prop).max(0) as usize;
                let v = obj.get_f64_array(idx)?.to_vec(); // row-major order²
                (v, order)
            }
            _ => return None,
        };
        if order == 0 || vals.iter().any(|x| !x.is_finite()) {
            return None;
        }
        return Some(Json::Arr(
            (0..order)
                .map(|i| {
                    Json::Arr(
                        (0..order)
                            .map(|j| Json::Float(scaled(vals[i * order + j], scale)))
                            .collect(),
                    )
                })
                .collect(),
        ));
    }

    // Plain number array — the element count per the property's sizing rule.
    let count = match pd.ptype {
        PropType::DoubleArray | PropType::IntegerArray => obj.get_i32(pd.size_prop).max(0) as usize,
        PropType::DoubleVArray => obj.array_size(idx),
        PropType::DoubleFArray => pd.size_prop,
        PropType::DoubleArrayOnStruct | PropType::EnumArrayOnStruct => {
            return finite_array(obj.get_struct_f64_array(idx), scale);
        }
        PropType::DoublePoints => return finite_array(obj.get_points(), scale),
        _ => 0,
    };
    let vals = obj.get_f64_array(idx)?;
    let slice: Vec<f64> = vals.iter().take(count).copied().collect();
    finite_array(slice, scale)
}

/// Build a JSON number array from `vals`, or `None` if empty or any value is
/// non-finite (the schema elides the whole default in that case).
fn finite_array(vals: Vec<f64>, scale: f64) -> Option<Json> {
    if vals.is_empty() || vals.iter().any(|x| !x.is_finite()) {
        return None;
    }
    Some(Json::Arr(
        vals.into_iter()
            .map(|v| Json::Float(scaled(v, scale)))
            .collect(),
    ))
}
