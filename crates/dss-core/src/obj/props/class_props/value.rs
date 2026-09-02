//! `ClassProps::get_value` — render a property as the string the `?` query and
//! `DumpProperties` emit (Pascal `DSSObjectHelper.GetObjPropertyValue`). Split
//! out of `class_props/mod.rs` (no behavioral change).

use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::setters::get_obj_double;
use crate::obj::props::{PropFlags, PropType};
use crate::util::{float_to_str_ex, get_dss_array_f64, get_dss_array_i32, str_y_or_n};

use super::ClassProps;

impl ClassProps {
    /// Pascal `GetObjPropertyValue`: render property `idx` as the string the
    /// `?` query and `DumpProperties` emit.
    pub fn get_value(&self, obj: &dyn DssObject, idx: usize, enums: &EnumRegistry) -> String {
        let pd = &self.props[idx];
        if pd.flags.contains(PropFlags::CONDITIONAL_VALUE) && !obj.prop_conditional(idx) {
            return "----".to_string();
        }
        // A function-only `ReadByFunction` value renders `""`: dss_capi 0.14.5
        // leaves its `PropertyOffset` at `-1`, so `GetObjPropertyValue`'s outer
        // guard short-circuits before the read function runs
        // (`DSSObjectHelper.pas:2203-2204`).
        //
        // EPRI r4133 -- the behavioral authority -- renders the live value
        // instead wherever its own `GetPropertyValue` has an arm for the
        // property (`IndMach012.pas:1790`, `StorageController.pas:991-994`);
        // those carry `RENDERS_LIVE_RESULT` and fall through to the normal
        // render below, reading the cache the read surfaces refresh at
        // `Dss::refresh_vterminal_if_marked`. The 0.14.5 suppression stays for
        // the JSON export/load and the schema (`PropFlags::SILENT_READ_ONLY`,
        // which documents all four readers). RP3.8.
        if pd.flags.contains(PropFlags::SILENT_READ_ONLY)
            && !pd.flags.contains(PropFlags::RENDERS_LIVE_RESULT)
        {
            return String::new();
        }
        match pd.ptype {
            PropType::Double => {
                let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
                    obj.prop_scale(idx, true)
                } else {
                    pd.scale
                };
                float_to_str_ex(get_obj_double(pd, obj, idx, scale))
            }
            PropType::Integer => {
                // Pascal `GetObjInteger` (DSSObjectHelper l.4350) subtracts
                // `PropertyValueOffset` on read — the inverse of the `+offset`
                // the setter applies (e.g. Recloser `Shots` stores `NumReclose =
                // Shots - 1` and dumps `NumReclose + 1`).
                let mut v = obj.get_i32(idx);
                if pd.flags.contains(PropFlags::VALUE_OFFSET) {
                    v -= pd.value_offset.round_ties_even() as i32;
                }
                v.to_string()
            }
            PropType::Boolean | PropType::Enabled => str_y_or_n(obj.get_bool(idx)).to_string(),
            PropType::String | PropType::ObjectRef => obj.get_string(idx),
            PropType::ObjectRefArray => {
                // Pascal renders the referenced names as `[a, b, c]`; an empty
                // list is `[]` (unlike `StringList`, which renders empty as "").
                format!("[{}]", obj.get_object_ref_names(idx).join(", "))
            }
            PropType::MakeLike | PropType::Action => String::new(), // Pascal: always ''
            PropType::MappedStringEnum => {
                let enum_id = pd.enum_id.expect("mapped enum property needs an enum");
                enums.get(enum_id).ordinal_to_string(obj.get_i32(idx))
            }
            // Pascal `GetPropertyValue` renders MappedIntEnumProperty with
            // `IntToStr` (the ordinal), unlike the string-enum name above
            // (`DSSObjectHelper.pas` l.2241).
            PropType::MappedIntEnum => obj.get_i32(idx).to_string(),
            PropType::DoubleArray => {
                // Pascal `GetPropertyValue`: a memory-mapped array (LoadShape
                // Mult/PMult/QMult under MMF) dumps its directive `(<mmFileCmd>)`
                // instead of the numeric values.
                if let Some(s) = obj.f64_array_dump_override(idx) {
                    return s;
                }
                let n = obj.get_i32(pd.size_prop).max(0) as usize;
                get_dss_array_f64(n, obj.get_f64_array(idx), pd.scale)
            }
            PropType::IntegerArray => {
                let n = obj.get_i32(pd.size_prop).max(0) as usize;
                get_dss_array_i32(n, obj.get_i32_array(idx))
            }
            PropType::DoubleSymMatrix => {
                // Pascal `GetObjPropertyValue` for `DoubleSymMatrixProperty`:
                // lower triangle, each element trailed by a space, rows split by
                // `|`, parenthesised — `(r |r r |r r r )`.
                //
                // The stored matrix is rendered, as the authority does: r4133
                // has no typed property table, so `Fault.GMatrix` is rendered by
                // hand from the dereferenced pointer under an `If Assigned`
                // (`Version8/Source/PDElements/Fault.pas:695-717`) and
                // Capacitor/Reactor fall through to `TDSSObject.GetPropertyValue`
                // = the stored property text (`General/DSSObject.pas:112-115`).
                // dss_capi's generic arm addresses the *pointer field* as if it
                // were the array (`src/General/DSSObjectHelper.pas:2296-2313`;
                // its JSON arm at `:1242-1266` repeats the same slip), so it
                // reads uninitialized memory and prints ~0 whatever was stored.
                // That is UB and is not reproduced in any lane.
                let order = obj.get_i32(pd.size_prop).max(0) as usize;
                let vals = obj.get_f64_array(idx);
                if order == 0 {
                    return String::new();
                }
                let mut s = String::from("(");
                for i in 0..order {
                    if i > 0 {
                        s.push('|');
                    }
                    for j in 0..=i {
                        // The stored order×order matrix, read row-major
                        // (`darray[(i-1)*Norder + j] / scale`); the text form
                        // prints its lower triangle.
                        let v = vals
                            .as_ref()
                            .and_then(|v| v.get(i * order + j).copied())
                            .map_or(0.0, |v| if pd.scale == 1.0 { v } else { v / pd.scale });
                        s.push_str(&float_to_str_ex(v));
                        s.push(' ');
                    }
                }
                s.push(')');
                s
            }
            PropType::DoubleFArray => {
                get_dss_array_f64(pd.size_prop, obj.get_f64_array(idx), pd.scale)
            }
            PropType::DoubleVArray => {
                let n = obj.array_size(idx);
                // Pascal `AllowNone` + count 0 dumps `[NONE]`, not `[]`
                // (DSSObjectHelper.pas:2274). Otherwise the normal array render.
                if n == 0 && pd.flags.contains(PropFlags::ALLOW_NONE) {
                    "[NONE]".to_string()
                } else {
                    get_dss_array_f64(n, obj.get_f64_array(idx), pd.scale)
                }
            }
            PropType::DoublePoints => {
                // Pascal `GetPoints`: interleaved `[x0 y0 x1 y1 ...]`, length
                // `2·NumPoints`.
                let pts = obj.get_points();
                get_dss_array_f64(pts.len(), Some(&pts), 1.0)
            }
            PropType::DoubleArrayOnStruct => {
                // Pascal: `[` + `%g, ` per entry (field / scale) + `]`.
                let vals = obj.get_struct_f64_array(idx);
                let mut s = String::from("[");
                for v in vals {
                    let value = if pd.scale == 1.0 { v } else { v / pd.scale };
                    s.push_str(&float_to_str_ex(value));
                    s.push_str(", ");
                }
                s.push(']');
                s
            }
            PropType::EnumArrayOnStruct => {
                // Pascal: `[` + `OrdinalToString, ` per entry + `]`.
                let enum_id = pd.enum_id.expect("enum-array property needs an enum");
                let en = enums.get(enum_id);
                let mut s = String::from("[");
                for ord in obj.get_struct_i32_array(idx) {
                    s.push_str(&en.ordinal_to_string(ord));
                    s.push_str(", ");
                }
                s.push(']');
                s
            }
            PropType::MappedStringEnumArray => {
                // Pascal: `[` + `OrdinalToString, ` per entry + `]`, over the
                // function-computed element count (`GetFuseStateSize`).
                let enum_id = pd.enum_id.expect("enum-array property needs an enum");
                let en = enums.get(enum_id);
                let n = obj.array_size(idx);
                let mut s = String::from("[");
                for &ord in obj.get_enum_array(idx).iter().take(n) {
                    s.push_str(&en.ordinal_to_string(ord));
                    s.push_str(", ");
                }
                s.push(']');
                s
            }
            PropType::StringList => {
                // Pascal `StringListToString`: `[a, b, c]`; empty list → "".
                let list = obj.get_string_list(idx);
                if list.is_empty() {
                    String::new()
                } else {
                    format!("[{}]", list.join(", "))
                }
            }
            PropType::Bus => obj.get_bus_name(pd.size_prop),
            PropType::BusOnStruct => obj.get_active_struct_bus(),
            PropType::BusesOnStruct => {
                // Pascal: `[` + `<bus>, ` per struct entry + `]`.
                let mut s = String::from("[");
                for b in obj.get_struct_buses() {
                    s.push_str(&b);
                    s.push_str(", ");
                }
                s.push(']');
                s
            }
            PropType::Complex => {
                // Pascal `GetObjPropertyValue` for `ComplexProperty`:
                // `Format('[%g, %g]', [c.re, c.im])` (verified against the oracle
                // by the Reactor `Z`/`Z1`/`Z2`/`Z0` props goldens).
                let (re, im) = obj.get_complex(idx);
                format!("[{}, {}]", float_to_str_ex(re), float_to_str_ex(im))
            }
            PropType::SymMatrixReal | PropType::SymMatrixImag => {
                // Pascal `GetObjPropertyValue` for `ComplexPartSymMatrixProperty`:
                // lower triangle, every element followed by a space, rows split
                // by `|`, no space after the opening bracket — e.g.
                // `[0.098 |0.040 0.098 |0.040 0.040 0.098 ]`.
                let real = pd.ptype == PropType::SymMatrixReal;
                let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
                    obj.prop_scale(idx, true)
                } else {
                    pd.scale
                };
                match obj.get_matrix_part(idx, real) {
                    None => String::new(),
                    Some((vals, order)) => {
                        let mut s = String::from("[");
                        for i in 0..order {
                            if i > 0 {
                                s.push('|');
                            }
                            for j in 0..=i {
                                s.push_str(&float_to_str_ex(vals[j * order + i] / scale));
                                s.push(' ');
                            }
                        }
                        s.push(']');
                        s
                    }
                }
            }
        }
    }
}
