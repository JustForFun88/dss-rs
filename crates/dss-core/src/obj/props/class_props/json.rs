//! `ClassProps::get_json_value` — render one property as its fpjson value, a
//! loop-for-loop port of `TDSSClassHelper.GetObjPropertyJSONValue`
//! (`.inputs/dss_capi/src/General/DSSObjectHelper.pas:968-1518`). This reproduces
//! *that* function's control flow (the `case ptype of` matrix + the
//! `preferArray`/`PropertyArrayAlternative` recursion + the `PropertyOffset =
//! -1` guard), NOT `get_value`'s text-dump flow — the two diverge (the JSON path
//! renders on-struct scalars as full arrays, has no `ConditionalValue`/`----`
//! placeholder, and emits `null` for NaN/Inf and unset references).
//!
//! Every accessor reused here is the same one `get_value` calls; only the
//! assembly into the ordered [`Json`] tree differs.

use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::setters::get_obj_double;
use crate::obj::props::{PropFlags, PropType};
use crate::report::export::json::{Json, JsonOpts};

use super::ClassProps;

impl ClassProps {
    /// Pascal `GetObjPropertyJSONValue(obj, idx, joptions, val, preferArray)`:
    /// build property `idx`'s JSON value, or `None` when the property is omitted
    /// (Pascal `Result := False` — the fallthrough, the `PropertyOffset = -1`
    /// guard, and the AllowNone→null handled inside the arms).
    pub fn get_json_value(
        &self,
        obj: &dyn DssObject,
        idx: usize,
        enums: &EnumRegistry,
        opts: JsonOpts,
        prefer_array: bool,
    ) -> Option<Json> {
        let pd = &self.props[idx];

        // `preferArray and (PropertyArrayAlternative[idx] <> 0)` → recurse into
        // the array-form property (DSSObjectHelper.pas:988-998).
        if prefer_array && pd.array_alternative != 0 {
            return self.get_json_value(obj, pd.array_alternative, enums, opts, true);
        }

        // `not ((Index > 0) and (Index <= NumProperties) and
        // (PropertyOffset[Index] <> -1))` → omit. Offset -1 is the Rust
        // NOT_PORTED / SILENT_READ_ONLY (function-only) marker.
        if idx == 0
            || idx > self.num_properties()
            || pd.flags.contains(PropFlags::NOT_PORTED)
            || pd.flags.contains(PropFlags::SILENT_READ_ONLY)
        {
            return None;
        }

        let enum_as_int = opts.contains(JsonOpts::ENUM_AS_INT);
        let full_names = opts.contains(JsonOpts::FULL_NAMES);
        let on_array = pd.flags.contains(PropFlags::ON_ARRAY);

        let value = match pd.ptype {
            PropType::Double => {
                if prefer_array && on_array {
                    // DoubleOnStructArrayProperty under preferArray → the full
                    // per-struct array (DSSObjectHelper.pas:1014-1039).
                    array_f64(obj.get_struct_f64_array(idx), pd.scale)
                } else {
                    let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
                        obj.prop_scale(idx, true)
                    } else {
                        pd.scale
                    };
                    let d = get_obj_double(pd, obj, idx, scale);
                    // NaN/Inf → null (never reaches the float formatter).
                    if d.is_finite() {
                        Json::Float(d)
                    } else {
                        Json::Null
                    }
                }
            }
            PropType::Integer => {
                if prefer_array && on_array {
                    // IntegerOnStructArrayProperty → per-struct int array.
                    Json::Arr(
                        obj.get_struct_i32_array(idx)
                            .into_iter()
                            .map(|v| Json::Int(v as i64))
                            .collect(),
                    )
                } else {
                    Json::Int(int_value(pd, obj, idx) as i64)
                }
            }
            // MappedIntEnumProperty renders the bare ordinal (grouped with
            // Integer in Pascal; DSSObjectHelper.pas:1050).
            PropType::MappedIntEnum => Json::Int(int_value(pd, obj, idx) as i64),
            PropType::Boolean | PropType::Enabled => Json::Bool(obj.get_bool(idx)),
            PropType::Complex => {
                let (re, im) = obj.get_complex(idx);
                Json::Arr(vec![Json::Float(re), Json::Float(im)])
            }
            // BusProperty (scalar) is in the String group — not OnArray — so it
            // renders a plain string (DSSObjectHelper.pas:1101/1157).
            PropType::Bus => Json::Str(obj.get_bus_name(pd.size_prop)),
            // BusOnStructArrayProperty / BusesOnStructArrayProperty → array of
            // GetBus(i) (DSSObjectHelper.pas:1116-1121 / 1355-1368).
            PropType::BusOnStruct | PropType::BusesOnStruct => {
                Json::Arr(obj.get_struct_buses().into_iter().map(Json::Str).collect())
            }
            PropType::String => Json::Str(obj.get_string(idx)),
            // MakeLikeProperty / StringEnumActionProperty read as `''` (Pascal
            // GetObjString for these; the probe pins `"Like":""`).
            PropType::MakeLike | PropType::Action => Json::Str(String::new()),
            PropType::MappedStringEnum => {
                if enum_as_int {
                    Json::Int(obj.get_i32(idx) as i64)
                } else {
                    let enum_id = pd.enum_id.expect("mapped enum property needs an enum");
                    Json::Str(enums.get(enum_id).ordinal_to_string(obj.get_i32(idx)))
                }
            }
            PropType::DoubleArray => {
                let n = obj.get_i32(pd.size_prop).max(0) as usize;
                opt_array_f64(obj.get_f64_array(idx), n, pd.scale)
            }
            PropType::DoubleVArray => {
                let n = obj.array_size(idx);
                if n == 0 && pd.flags.contains(PropFlags::ALLOW_NONE) {
                    // AllowNone + count 0 → null (DSSObjectHelper.pas:1218).
                    Json::Null
                } else {
                    opt_array_f64(obj.get_f64_array(idx), n, pd.scale)
                }
            }
            PropType::DoubleFArray => opt_array_f64(obj.get_f64_array(idx), pd.size_prop, pd.scale),
            PropType::DoublePoints => {
                // DoubleDArrayProperty (ReadByFunction) → the interleaved points.
                array_f64(obj.get_points(), 1.0)
            }
            PropType::DoubleArrayOnStruct => {
                // DoubleArrayOnStructArrayProperty: one field per struct entry.
                array_f64(obj.get_struct_f64_array(idx), pd.scale)
            }
            PropType::EnumArrayOnStruct => {
                let enum_id = pd.enum_id.expect("enum-array property needs an enum");
                let en = enums.get(enum_id);
                Json::Arr(
                    obj.get_struct_i32_array(idx)
                        .into_iter()
                        .map(|o| enum_ordinal_json(en, o, enum_as_int))
                        .collect(),
                )
            }
            PropType::MappedStringEnumArray => {
                let enum_id = pd.enum_id.expect("enum-array property needs an enum");
                let en = enums.get(enum_id);
                let n = obj.array_size(idx);
                Json::Arr(
                    obj.get_enum_array(idx)
                        .into_iter()
                        .take(n)
                        .map(|o| enum_ordinal_json(en, o, enum_as_int))
                        .collect(),
                )
            }
            PropType::IntegerArray => {
                let n = obj.get_i32(pd.size_prop).max(0) as usize;
                match obj.get_i32_array(idx) {
                    None => Json::Null,
                    Some(vals) => {
                        Json::Arr(vals.iter().take(n).map(|&v| Json::Int(v as i64)).collect())
                    }
                }
            }
            PropType::StringList => Json::Arr(
                obj.get_string_list(idx)
                    .into_iter()
                    .map(Json::Str)
                    .collect(),
            ),
            PropType::DoubleSymMatrix => {
                let order = obj.get_i32(pd.size_prop).max(0) as usize;
                match obj.get_f64_array(idx) {
                    None => Json::Null,
                    // Pascal reads the full order×order stored matrix,
                    // row-major `darray[(i-1)*Norder + j] / scale`
                    // (DSSObjectHelper.pas:1270-1283).
                    Some(vals) => Json::Arr(
                        (0..order)
                            .map(|i| {
                                Json::Arr(
                                    (0..order)
                                        .map(|j| Json::Float(scaled(vals[i * order + j], pd.scale)))
                                        .collect(),
                                )
                            })
                            .collect(),
                    ),
                }
            }
            PropType::SymMatrixReal | PropType::SymMatrixImag => {
                let real = pd.ptype == PropType::SymMatrixReal;
                let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
                    obj.prop_scale(idx, true)
                } else {
                    pd.scale
                };
                match obj.get_matrix_part(idx, real) {
                    None => Json::Null,
                    // ComplexPartSymMatrixProperty: full order×order, row-major.
                    // `get_matrix_part` returns column-major flat, so element
                    // (row i, col j) = vals[j*order + i] (matches `get_value`).
                    Some((vals, order)) => Json::Arr(
                        (0..order)
                            .map(|i| {
                                Json::Arr(
                                    (0..order)
                                        .map(|j| Json::Float(scaled(vals[j * order + i], scale)))
                                        .collect(),
                                )
                            })
                            .collect(),
                    ),
                }
            }
            PropType::ObjectRef => {
                let name = obj.get_string(idx);
                if name.is_empty() {
                    // otherObj = NIL → null (DSSObjectHelper.pas:1197).
                    Json::Null
                } else if full_names || pd.object_class == Some("") {
                    Json::Str(self.object_full_name(pd, &name))
                } else {
                    Json::Str(name)
                }
            }
            PropType::ObjectRefArray => {
                let use_full = full_names
                    || pd.object_class == Some("")
                    || pd.flags.contains(PropFlags::FULL_NAME_AS_JSON_ARRAY);
                Json::Arr(
                    obj.get_object_ref_names(idx)
                        .into_iter()
                        .map(|n| {
                            if use_full {
                                Json::Str(self.object_full_name(pd, &n))
                            } else {
                                Json::Str(n)
                            }
                        })
                        .collect(),
                )
            }
        };
        Some(value)
    }

    /// Build a reference's `Class.Name` FullName. For an `object_ref_any`
    /// (`object_class == Some("")`) the stored name already carries the class,
    /// so it is returned as-is; otherwise the resolving class name is prefixed.
    fn object_full_name(&self, pd: &crate::obj::props::PropDef, name: &str) -> String {
        match pd.object_class {
            Some("") | None => name.to_string(),
            Some(cls) => format!("{cls}.{name}"),
        }
    }
}

/// Pascal `GetObjInteger`: subtract `PropertyValueOffset` under `VALUE_OFFSET`.
fn int_value(pd: &crate::obj::props::PropDef, obj: &dyn DssObject, idx: usize) -> i32 {
    let mut v = obj.get_i32(idx);
    if pd.flags.contains(PropFlags::VALUE_OFFSET) {
        v -= pd.value_offset.round_ties_even() as i32;
    }
    v
}

fn scaled(v: f64, scale: f64) -> f64 {
    if scale == 1.0 { v } else { v / scale }
}

/// `GetDSSArray_JSON` over an owned `Vec<f64>` (scale-divided).
fn array_f64(vals: Vec<f64>, scale: f64) -> Json {
    Json::Arr(
        vals.into_iter()
            .map(|v| Json::Float(scaled(v, scale)))
            .collect(),
    )
}

/// `GetDSSArray_JSON` over a borrowed slice + count: `None` pointer → `null`.
fn opt_array_f64(vals: Option<&[f64]>, n: usize, scale: f64) -> Json {
    match vals {
        None => Json::Null,
        Some(v) => Json::Arr(
            v.iter()
                .take(n)
                .map(|&x| Json::Float(scaled(x, scale)))
                .collect(),
        ),
    }
}

fn enum_ordinal_json(en: &crate::obj::dss_enum::DssEnum, ord: i32, enum_as_int: bool) -> Json {
    if enum_as_int {
        Json::Int(ord as i64)
    } else {
        Json::Str(en.ordinal_to_string(ord))
    }
}
