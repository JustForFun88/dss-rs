//! `ClassProps::fill_from_json` / `set_json_value` — the JSON *import* property
//! applier, a port of `TDSSClassHelper.FillObjFromJSON` +
//! `SetObjPropertyJSONValue` (`.inputs/dss_capi/src/General/DSSObjectHelper.pas:
//! 4922-4999` / `1759-2205`). The inverse of [`ClassProps::get_json_value`]
//! (`json.rs`).
//!
//! `fill_from_json` walks the class's [`alt_property_order`](ClassProps::alt_property_order)
//! (Pascal `AltPropertyOrder`), and for every property whose JSON key
//! ([`PropDef::json_key`]) is present in the object, applies it — advancing the
//! set-order (`SetAsNextSeq`) and running the side effects, exactly like the
//! string edit loop. That fixed order is what makes a round-trip
//! (export → import → re-export) reproduce the oracle's byte-for-byte re-export
//! (the re-exported set-order equals `AltPropertyOrder`, not the original deck
//! order).
//!
//! ## How the value is applied
//! `set_json_value` mirrors the Pascal dispatch:
//!  * the `PropertyArrayAlternative` redirect (a singular per-winding key like
//!    `"Bus"`/`"kV"` carries the full plural array → apply the plural property);
//!  * `DoubleOnStructArray` / `IntegerOnStructArray` (`ON_ARRAY` scalar) values
//!    are per-struct JSON arrays → the typed struct setters
//!    ([`ClassProps::set_prop_struct_f64s`] / [`set_prop_struct_i32s`]);
//!  * every other type is rendered to the exact string its
//!    [`ClassProps::edit_property`] (string) path expects and applied through
//!    that same property-applier VM — the string parse reproduces the oracle's
//!    `SetObj*` arithmetic bit-for-bit (a scalar double round-trips through
//!    `f64::from_str`; an array/matrix re-parses to the identical stored values),
//!    so no separate typed leaf is needed for those.

use crate::obj::base::DssObject;
use crate::obj::props::{PropDef, PropEngine, PropFlags, PropType};
use crate::report::export::json::Json;

use super::ClassProps;

impl ClassProps {
    /// Pascal `FillObjFromJSON`: apply every present property of `members` (the
    /// JSON object's key/value pairs) to `obj`, in `AltPropertyOrder`. `Name` is
    /// extracted by the caller (`loadClassFromJSON`) and is not a property, so it
    /// is simply never matched here.
    pub fn fill_from_json(
        &self,
        obj: &mut dyn DssObject,
        members: &[(String, Json)],
        eng: &mut PropEngine,
    ) {
        // fpjson `TJSONObject.Find` is a case-sensitive exact match; the export
        // writes each key with `json_key(false)`, so build a direct lookup.
        let lookup: std::collections::HashMap<&str, &Json> =
            members.iter().map(|(k, v)| (k.as_str(), v)).collect();

        for &idx in &self.alt_property_order {
            let pd = &self.props[idx];
            let key = pd.json_key(false);
            let Some(&jval) = lookup.get(key.as_str()) else {
                // Pascal `FillObjFromJSON` (`DSSObjectHelper.pas:4955`): a missing
                // `Required` property is a loud error that aborts the load
                // (`raise` → `loadClassFromJSON`'s `except` → DoSimpleMsg 5021).
                // We push it and stop applying further properties; the caller
                // aborts the whole import on a non-empty error list.
                if pd.flags.contains(PropFlags::REQUIRED) {
                    eng.errors.push(crate::diag::DssDiagnostic::msg(
                        format!(
                            "JSON/{}/{}: required property not provided: \"{}\".",
                            self.class_name(),
                            obj.data().name(),
                            key
                        ),
                        Some(5021),
                    ));
                    return;
                }
                continue;
            };
            // Pascal: a silent read-only property is ignored on load.
            if pd.flags.contains(PropFlags::SILENT_READ_ONLY) {
                continue;
            }
            // Pascal: a `null` value is skipped unless the property allows NONE.
            // A rating reads `null` as a rating that is not set.
            if matches!(jval, Json::Null)
                && !pd.flags.contains(PropFlags::ALLOW_NONE)
                && pd.ptype != PropType::Rating
            {
                continue;
            }
            self.set_json_value(obj, idx, jval, eng);
        }
        // The `TDynEqPCE` `"DynInit"` tail (`DSSObjectHelper.pas:4979-4996`) is
        // NOT ported here (mirrors the export-side `DynInit` deferral,
        // JSON_EXPORT_PLAN §6): it only fires for a Generator/PVSystem/Storage
        // carrying a `DynamicExp`. A model that used it would round-trip every
        // other property and simply omit the dynamic-init overrides.
    }

    /// Pascal `SetObjPropertyJSONValue`: apply one JSON `jval` to property `idx`.
    /// `SetAsNextSeq` + `PropertySideEffects` are folded into the leaf setters
    /// ([`edit_property`](ClassProps::edit_property) and the `set_prop_struct_*`
    /// helpers), matching FillObjFromJSON's post-call bookkeeping.
    pub fn set_json_value(
        &self,
        obj: &mut dyn DssObject,
        idx: usize,
        jval: &Json,
        eng: &mut PropEngine,
    ) {
        let pd = &self.props[idx];

        // `PropertyArrayAlternative[Index] <> 0` → apply the array-form twin (the
        // singular per-winding `"Bus"`/`"kV"`/… key holds the whole array).
        if pd.array_alternative != 0 {
            self.set_json_value(obj, pd.array_alternative, jval, eng);
            return;
        }

        // Guard: `(Index in range) and (PropertyOffset <> -1)`. Offset -1 is the
        // Rust NOT_PORTED marker.
        if idx == 0 || idx > self.num_properties() || pd.flags.contains(PropFlags::NOT_PORTED) {
            return;
        }

        // `ON_ARRAY` scalar: a `DoubleOnStructArrayProperty` /
        // `IntegerOnStructArrayProperty` value is a per-struct JSON array (e.g.
        // transformer `RDCOhms`/`NumTaps`). The string edit path has no array
        // form for these, so drive the typed struct setters directly.
        if pd.flags.contains(PropFlags::ON_ARRAY) {
            match pd.ptype {
                PropType::Double => {
                    let vals: Vec<Option<f64>> = json_f64_iter(jval).map(Some).collect();
                    self.set_prop_struct_f64s(obj, idx, &vals, eng);
                    return;
                }
                PropType::Integer | PropType::MappedIntEnum => {
                    let vals: Vec<i32> = json_f64_iter(jval).map(|v| v as i32).collect();
                    self.set_prop_struct_i32s(obj, idx, &vals, eng);
                    return;
                }
                _ => {}
            }
        }

        // A list that keeps only the values given takes one position per item,
        // so an item that is not one number is refused, naming its place, and
        // the list is not applied.
        if pd.flags.contains(PropFlags::GIVEN_PREFIX)
            && obj.refuse_property(idx, pd.name).is_none()
            && let Some(why) = given_list_refusal(jval)
        {
            obj.value_unreadable(idx);
            eng.errors.push(dss_parser::ParserError::new(format!(
                "{}.{}.{}: {why}, so the list is not applied.",
                self.class_name,
                obj.data().name(),
                pd.name
            )));
            return;
        }

        // A seasonal rating array carries its season count: an object whose
        // `Seasons` was never set takes the array's length first, so every
        // season comes back (`a_json_round_trip_keeps_a_typed_rating_that_is_not_set`).
        let seasons = match jval {
            Json::Arr(a)
                if pd.ptype == PropType::RatingArray
                    && !obj.data().prp_specified(pd.size_prop)
                    && usize::try_from(obj.get_i32(pd.size_prop)).ok() != Some(a.len()) =>
            {
                Some(a.len())
            }
            _ => None,
        };
        if let Some(n) = seasons
            && let Err(e) = self.edit_property(obj, pd.size_prop, &n.to_string(), eng)
        {
            eng.errors.push(e);
            return;
        }

        // Everything else: render to the string its parse path expects, then run
        // the ordinary property-applier VM (seq mark + side effects included).
        if let Some(s) = json_to_value_string(pd, jval)
            && let Err(e) = self.edit_property(obj, idx, &s, eng)
        {
            eng.errors.push(e);
        }
    }
}

/// Render one scalar JSON value as the DSS value token `parse_into` reads. A
/// double uses Rust's shortest round-tripping form (parses back to the identical
/// `f64` via `f64::from_str`, so the re-export byte-matches).
fn json_scalar_str(v: &Json) -> String {
    match v {
        Json::Str(s) => s.clone(),
        Json::Int(i) => i.to_string(),
        Json::Float(f) => format!("{f}"),
        Json::Bool(true) => "true".to_string(),
        Json::Bool(false) => "false".to_string(),
        Json::Null => String::new(),
        Json::Arr(_) | Json::Obj(_) => String::new(),
    }
}

/// Iterate an array's numeric values (or a lone scalar) as `f64` — for the
/// typed `ON_ARRAY` struct setters.
fn json_f64_iter(v: &Json) -> impl Iterator<Item = f64> + '_ {
    let items: Vec<f64> = match v {
        Json::Arr(a) => a.iter().map(json_num).collect(),
        other => vec![json_num(other)],
    };
    items.into_iter()
}

fn json_num(v: &Json) -> f64 {
    match v {
        Json::Float(f) => *f,
        Json::Int(i) => *i as f64,
        _ => 0.0,
    }
}

/// `element` inside the first quote pair of the tokenizer whose closing
/// character it does not hold, so the tokenizer reads it as one element.
fn quoted_element(element: &str) -> String {
    [('"', '"'), ('\'', '\''), ('(', ')'), ('[', ']'), ('{', '}')]
        .into_iter()
        .find(|&(_, close)| !element.contains(close))
        .map_or_else(
            || element.to_string(),
            |(open, close)| format!("{open}{element}{close}"),
        )
}

/// The number one item of a JSON list holds: a number, or a string holding
/// one number, blanks around it ignored. It may be infinite or NaN.
fn json_one_number(v: &Json) -> Option<f64> {
    match v {
        Json::Int(i) => Some(*i as f64),
        Json::Float(f) => Some(*f),
        Json::Str(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// An empty item of a JSON list: `null` or a blank string. The parse refuses
/// it inside the list and ends the list at it.
fn json_empty_item(v: &Json) -> bool {
    match v {
        Json::Null => true,
        Json::Str(s) => s.trim().is_empty(),
        _ => false,
    }
}

/// Why `jval` cannot be read as a list that keeps only the values given, or
/// `None` when it can. Only a list is a list: an object, a boolean, a number
/// or a string as the whole value is refused. Each item of a list is a finite
/// number, a string holding one or an empty item. An item that is neither is
/// named here, ahead of an empty item before it (`[-4,null,1e400]` names value
/// 3), and the text parse names an empty item when the list holds no such
/// item. Pinned by
/// `exec::tests::line_geometry_rules::a_spacing_list_with_an_empty_value_is_refused`
/// and `::a_spacing_coordinate_is_a_finite_length`, which holds the
/// `[-4,null,1e400]` leg.
fn given_list_refusal(jval: &Json) -> Option<String> {
    match jval {
        Json::Arr(items) => items.iter().enumerate().find_map(|(at, v)| {
            let why = match json_one_number(v) {
                Some(f) if f.is_finite() => return None,
                Some(_) => "is not a finite number",
                None if json_empty_item(v) => return None,
                None => "is not a number",
            };
            Some(format!("value {} of the list {why}", at + 1))
        }),
        Json::Null => None,
        _ => Some("the value is not a list of numbers".to_string()),
    }
}

/// Build the DSS value string for property `pd` from its JSON value, matching
/// the grammar `ClassProps::parse_into` reads for that [`PropType`]. Returns
/// `None` when there is nothing to apply.
fn json_to_value_string(pd: &PropDef, jval: &Json) -> Option<String> {
    let join_scalars =
        |a: &[Json]| -> String { a.iter().map(json_scalar_str).collect::<Vec<_>>().join(" ") };

    // A rating's `null` (and a `null` entry of a rating array) is the `none`
    // token its parse reads as not set.
    let rating_token = |v: &Json| match v {
        Json::Null => crate::obj::Rating::NONE_TOKEN.to_string(),
        other => json_scalar_str(other),
    };
    let s = match pd.ptype {
        PropType::Rating => rating_token(jval),
        // Each item of a string list is one element, an empty item included,
        // so `parse_into` decides the list by its items.
        PropType::StringList => match jval {
            Json::Arr(a) => a
                .iter()
                .map(|item| quoted_element(&json_scalar_str(item)))
                .collect::<Vec<_>>()
                .join(","),
            scalar => json_scalar_str(scalar),
        },
        PropType::RatingArray => match jval {
            Json::Arr(a) => a.iter().map(rating_token).collect::<Vec<_>>().join(" "),
            scalar => rating_token(scalar),
        },
        // Real matrices render as the lower triangle, `row0 | row1 | …`, which
        // `parse_as_sym_matrix` reads (the stored matrix is symmetric, so the
        // triangle is loss-free). Each cell is scalar-formatted.
        PropType::SymMatrixReal | PropType::SymMatrixImag | PropType::DoubleSymMatrix => {
            let Json::Arr(rows) = jval else {
                return None;
            };
            let mut parts: Vec<String> = Vec::with_capacity(rows.len());
            for (i, row) in rows.iter().enumerate() {
                let Json::Arr(cells) = row else {
                    return None;
                };
                let tri: Vec<String> = cells.iter().take(i + 1).map(json_scalar_str).collect();
                parts.push(tri.join(" "));
            }
            parts.join(" | ")
        }
        // A list that keeps only the values given is joined with commas, one
        // number per item, so an empty item inside it is an empty value its
        // parse refuses, naming it (`given_list_refusal` has refused every
        // other item). Pinned by
        // `exec::tests::line_geometry_rules::a_spacing_list_with_an_empty_value_is_refused`.
        PropType::DoubleVArray if pd.flags.contains(PropFlags::GIVEN_PREFIX) => match jval {
            Json::Arr(a) => a
                .iter()
                .map(|v| {
                    json_one_number(v)
                        .map(|f| format!("{f}"))
                        .unwrap_or_default()
                })
                .collect::<Vec<_>>()
                .join(","),
            scalar => json_scalar_str(scalar),
        },
        // Every other array-shaped property is a flat, space-separated token
        // list (numbers, enum strings, bus names, object-ref names, complex
        // [re, im], interleaved xy points). `parse_into` tokenizes it.
        _ => match jval {
            Json::Arr(a) => join_scalars(a),
            scalar => json_scalar_str(scalar),
        },
    };
    Some(s)
}
