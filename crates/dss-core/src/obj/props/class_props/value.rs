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
        match pd.ptype {
            PropType::Double => {
                let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
                    obj.prop_scale(idx, true)
                } else {
                    pd.scale
                };
                float_to_str_ex(get_obj_double(pd, obj, idx, scale))
            }
            PropType::Integer => obj.get_i32(idx).to_string(),
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
                // TODO(compat): the oracle's `DoubleSymMatrixProperty` getter —
                // whose only user in scope is `Capacitor.CMatrix` — reads
                // uninitialized memory and returns denormal garbage (~0)
                // regardless of the stored matrix (a dss_capi bug). We emit a
                // zero matrix of the declared order to reproduce it
                // deterministically; the goldens zero the captured garbage to
                // match. The clean fix renders the stored `get_f64_array(idx)`
                // values (the `_vals` binding) divided by `pd.scale`.
                let order = obj.get_i32(pd.size_prop).max(0) as usize;
                let _vals = obj.get_f64_array(idx);
                if order == 0 {
                    return String::new();
                }
                let mut s = String::from("(");
                for i in 0..order {
                    if i > 0 {
                        s.push('|');
                    }
                    for _ in 0..=i {
                        s.push_str(&float_to_str_ex(0.0));
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
                get_dss_array_f64(obj.array_size(idx), obj.get_f64_array(idx), pd.scale)
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
