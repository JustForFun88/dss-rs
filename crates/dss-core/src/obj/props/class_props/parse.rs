//! `ClassProps::parse_into` — the value parser that drives the typed
//! [`DssObject`] setters (Pascal `DSSObjectHelper.ParseObjPropertyValue` +
//! `SetObj*`). Split out of `class_props/mod.rs` (no behavioral change).

use crate::obj::base::DssObject;
use crate::obj::props::setters::{
    get_double, get_integer, interval_units_error, parse_interval_units_f64,
    parse_interval_units_i32, set_obj_double, set_obj_integer,
};
use crate::obj::props::{PropEngine, PropFlags, PropType};
use crate::util::interpret_dbl_array;
use dss_parser::ParserError;

use super::ClassProps;

impl ClassProps {
    /// Parse `value` into property `idx` and write it through the typed
    /// accessors (Pascal `ParseObjPropertyValue` + `SetObj*`). Returns the
    /// property's previous integer value, which side effects may consult.
    /// Range/sign check failures are recorded in `eng.errors` and leave the
    /// property unchanged, matching `DoSimpleMsg`-and-continue; only a genuine
    /// number-conversion failure is returned as `Err`.
    pub fn parse_into(
        &self,
        obj: &mut dyn DssObject,
        idx: usize,
        value: &str,
        eng: &mut PropEngine,
    ) -> Result<i32, ParserError> {
        let pd = &self.props[idx];
        let full = format!("{}.{}", self.class_name, obj.data().name());
        if pd.flags.contains(PropFlags::NOT_PORTED) {
            return Err(ParserError::new(format!(
                "{full}.{}: this property is not ported yet (Phase 3 slice)",
                pd.name
            )));
        }
        match pd.ptype {
            PropType::Double => {
                let v = if pd.flags.contains(PropFlags::INTERVAL_UNITS) {
                    match parse_interval_units_f64(value) {
                        Some(v) => v,
                        None => {
                            // Pascal logs the message and `Exit`s — field unchanged.
                            eng.errors.push(interval_units_error(&full, pd.name, value));
                            return Ok(0);
                        }
                    }
                } else {
                    get_double(eng, value)?
                };
                let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
                    obj.prop_scale(idx, false)
                } else {
                    pd.scale
                };
                set_obj_double(pd, obj, idx, v, scale, eng, &full);
                Ok(0)
            }
            PropType::Bus => {
                obj.set_bus_name(pd.size_prop, value);
                Ok(0)
            }
            PropType::Complex => {
                // Pascal ComplexProperty/ComplexPartsProperty: parse the value
                // as a 2-vector (re, im) through the scratch parser.
                let mut buf = [0.0; 2];
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(&format!("[{value}]"));
                eng.parser.next_param(eng.vars);
                eng.parser.parse_as_vector(eng.vars, &mut buf, false)?;
                obj.set_complex(idx, buf[0], buf[1]);
                Ok(0)
            }
            PropType::DoubleFArray => {
                // Pascal `DoubleFArrayProperty` (`DSSObjectHelper.pas` l.651):
                // `PropParser.ParseAsVector(count, ...)` returns the number of
                // values actually supplied — the `prevInt` some side effects need
                // (e.g. EnergyMeter `Mask` fills the remaining slots with 1.0).
                let n = pd.size_prop;
                let mut buf = vec![0.0; n];
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(&format!("[{value}]"));
                eng.parser.next_param(eng.vars);
                let count = eng.parser.parse_as_vector(eng.vars, &mut buf, false)?;
                obj.set_f64_array(idx, buf);
                Ok(count.min(n) as i32)
            }
            PropType::StringList => {
                // Pascal `InterpretTStringListArray`: tokenize the value through
                // the scratch parser (commas/spaces, brackets already stripped by
                // the outer command parser). `file=` specs are deferred.
                let lower = pd.flags.contains(PropFlags::TRANSFORM_LOWERCASE);
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(value);
                let mut list = Vec::new();
                loop {
                    eng.parser.next_param(eng.vars);
                    let token = eng.parser.make_string(eng.vars);
                    if token.is_empty() {
                        break;
                    }
                    list.push(if lower { token.to_lowercase() } else { token });
                }
                obj.set_string_list(idx, list);
                Ok(0)
            }
            PropType::SymMatrixReal | PropType::SymMatrixImag => {
                let order = obj.get_i32(pd.size_prop).max(0) as usize;
                let scale = if pd.flags.contains(PropFlags::SCALED_BY_FUNCTION) {
                    obj.prop_scale(idx, false)
                } else {
                    pd.scale
                };
                let mut buf = vec![0.0; order * order];
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(&format!("[{value}]"));
                eng.parser.next_param(eng.vars);
                eng.parser
                    .parse_as_sym_matrix(eng.vars, &mut buf, order, 1, scale)?;
                obj.set_matrix_part(idx, &buf, order, pd.ptype == PropType::SymMatrixReal);
                Ok(0)
            }
            PropType::Enabled => {
                let v = crate::util::interpret_yes_no(value);
                let prev = obj.get_bool(idx) as i32;
                obj.set_bool(idx, v);
                Ok(prev)
            }
            PropType::ObjectRef => {
                match pd.object_class {
                    None => {
                        // Phase 3 behavior: store the lowercased name only.
                        obj.set_string(idx, value.to_lowercase());
                    }
                    Some("") => {
                        // Pascal `DSSObjectReferenceProperty` with
                        // `PropertyOffset2 = 0`: the value is a full
                        // `Class.Name` resolved against any circuit class
                        // (`GetCktElementIndex`). On failure: DoSimpleMsg 402,
                        // NIL reference, edit continues.
                        let resolved = eng.foreign.and_then(|f| f.find_full(value));
                        if resolved.is_none() && !value.is_empty() {
                            eng.errors.push(format!(
                                "{full}.{}: CktElement \"{value}\" not found.",
                                pd.name
                            ));
                        }
                        // Dumps render the resolved object's FullName (NIL → "").
                        let name = resolved
                            .as_ref()
                            .map(|(_, _, full_name)| full_name.clone())
                            .unwrap_or_default();
                        obj.set_object_ref(idx, name, resolved.map(|(r, o, _)| (r, o)));
                    }
                    Some(class) => {
                        // Pascal `ParseObjPropertyValue` for
                        // `DSSObjectReferenceProperty`: resolve `cls.Find(name)`
                        // (case-insensitive). On failure DoSimpleMsg 401 and the
                        // reference is left NIL, but the edit continues.
                        let mut resolved = eng.foreign.and_then(|f| f.find(class, value));
                        // Pascal `TProxyClass` (RegControl `transformer=`): try the
                        // second class when the first misses.
                        if resolved.is_none()
                            && let Some(class2) = pd.object_class2
                        {
                            resolved = eng.foreign.and_then(|f| f.find(class2, value));
                        }
                        if resolved.is_none() && !value.is_empty() {
                            eng.errors.push(format!(
                                "{full}.{}: {class} object \"{value}\" not found.",
                                pd.name
                            ));
                        }
                        // The dump renders the resolved object's name (NIL → "").
                        let name = resolved
                            .map(|(_, o)| o.data().name().to_string())
                            .unwrap_or_default();
                        obj.set_object_ref(idx, name, resolved);
                    }
                }
                Ok(0)
            }
            PropType::ObjectRefArray => {
                // Pascal `DSSObjectReferenceArrayProperty` (`WriteByFunction`):
                // parse every token, resolve each against the fixed class
                // (`cls.Find`), and hand the resolved list to the object, which
                // validates the count and stores the references (Pascal
                // `SetWires`). On the *first* unresolved token the upstream logs
                // its "object not found" message and `Exit`s immediately
                // (DSSObjectHelper.pas) — the write function never runs, so
                // nothing is stored and no count error is raised.
                let class = pd
                    .object_class
                    .expect("object-ref-array property needs a class");
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(value);
                let mut names = Vec::new();
                loop {
                    eng.parser.next_param(eng.vars);
                    let token = eng.parser.make_string(eng.vars);
                    if token.is_empty() {
                        break;
                    }
                    names.push(token);
                }
                let mut refs = Vec::with_capacity(names.len());
                for token in &names {
                    match eng.foreign.and_then(|f| f.find(class, token)) {
                        Some((r, o)) => refs.push((o.data().name().to_string(), r, o)),
                        None => {
                            eng.errors.push(format!(
                                "{full}.{}: {class} object \"{token}\" not found.",
                                pd.name
                            ));
                            return Ok(0);
                        }
                    }
                }
                obj.set_object_ref_array(idx, &refs);
                Ok(0)
            }
            PropType::Integer => {
                let v = if pd.flags.contains(PropFlags::INTERVAL_UNITS) {
                    match parse_interval_units_i32(value) {
                        Some(v) => v,
                        None => {
                            // Pascal logs the message and `Exit`s — field unchanged.
                            eng.errors.push(interval_units_error(&full, pd.name, value));
                            return Ok(obj.get_i32(idx));
                        }
                    }
                } else {
                    get_integer(eng, value)?
                };
                Ok(set_obj_integer(pd, obj, idx, v, eng, &full))
            }
            PropType::Boolean => {
                let v = crate::util::interpret_yes_no(value);
                let prev = obj.get_bool(idx) as i32;
                obj.set_bool(idx, v);
                Ok(prev)
            }
            PropType::MappedStringEnum | PropType::MappedIntEnum => {
                let enum_id = pd.enum_id.expect("mapped enum property needs an enum");
                let ord = if pd.ptype == PropType::MappedStringEnum {
                    eng.enums
                        .get(enum_id)
                        .string_to_ordinal(&value.to_lowercase())?
                } else {
                    let v = get_integer(eng, value)?;
                    if !eng.enums.get(enum_id).is_ordinal_valid(v) {
                        eng.errors.push(format!(
                            "{full}.{}: \"{value}\" is not a valid value.",
                            pd.name
                        ));
                        return Ok(obj.get_i32(idx));
                    }
                    v
                };
                Ok(set_obj_integer(pd, obj, idx, ord, eng, &full))
            }
            PropType::Action => {
                // Pascal `StringEnumActionProperty`: map the value to an action
                // ordinal, then run it immediately (no field is stored).
                let enum_id = pd.enum_id.expect("action property needs an enum");
                let ord = eng
                    .enums
                    .get(enum_id)
                    .string_to_ordinal(&value.to_lowercase())?;
                obj.do_action(ord, eng.errors);
                Ok(0)
            }
            PropType::String => {
                let v = if pd.flags.contains(PropFlags::TRANSFORM_LOWERCASE) {
                    value.to_lowercase()
                } else {
                    value.to_string()
                };
                obj.set_string(idx, v);
                Ok(0)
            }
            PropType::MakeLike => {
                // `like=` copies another object's state; that needs the class
                // collection, so the executive intercepts it before reaching
                // here (Phase 2).
                Err(ParserError::new(
                    "MakeLike must be handled by the executive, not the property engine",
                ))
            }
            PropType::DoubleArray => {
                let max = obj.get_i32(pd.size_prop).max(0) as usize;
                let mut buf = vec![0.0; max];
                interpret_dbl_array(eng.parser, eng.vars, value, max, &mut buf)?;
                if pd.flags.contains(PropFlags::APPLY_ROUND) {
                    // TODO(compat): FPC `Round` is ties-to-even with an
                    // integer-indefinite path for out-of-Int64 magnitudes; for
                    // array rounding (years, point counts) the magnitudes are
                    // always in range, so plain ties-to-even suffices. The clean
                    // fix wipes this with the other compat shims.
                    for v in &mut buf {
                        *v = v.round_ties_even();
                    }
                }
                if pd.flags.contains(PropFlags::NON_ZERO) && buf.contains(&0.0) {
                    eng.errors
                        .push(format!("{full}.{}: Elements cannot be zero.", pd.name));
                    return Ok(0);
                }
                if pd.scale != 1.0 {
                    for v in &mut buf {
                        *v *= pd.scale;
                    }
                }
                obj.set_f64_array(idx, buf);
                Ok(0)
            }
            PropType::IntegerArray => {
                // Pascal `IntegerArrayProperty` + `InterpretIntArray`: read up
                // to `size_prop` integers; omitted/short tokens yield 0.
                let max = obj.get_i32(pd.size_prop).max(0) as usize;
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(value);
                let mut buf = vec![0; max];
                for slot in buf.iter_mut() {
                    eng.parser.next_param(eng.vars);
                    *slot = eng.parser.make_integer(eng.vars)?;
                }
                obj.set_i32_array(idx, buf);
                Ok(0)
            }
            PropType::DoubleSymMatrix => {
                // Pascal `DoubleSymMatrixProperty`: a real lower-triangle sym
                // matrix of order `get_i32(size_prop)`, stored flat (`order²`).
                let order = obj.get_i32(pd.size_prop).max(0) as usize;
                let mut buf = vec![0.0; order * order];
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(&format!("[{value}]"));
                eng.parser.next_param(eng.vars);
                eng.parser
                    .parse_as_sym_matrix(eng.vars, &mut buf, order, 1, pd.scale)?;
                obj.set_f64_array(idx, buf);
                Ok(0)
            }
            PropType::DoubleVArray if pd.flags.contains(PropFlags::ARRAY_MAX_SIZE) => {
                // Pascal `DoubleVArrayProperty` + `ArrayMaxSize` (DSSObjectHelper
                // l.637): `ParseAsVector(maxSize, array)` reads up to `max`
                // values and `integerPtr^ := <count supplied>`. The object owns
                // the count (`set_f64_array` sets it from the supplied length);
                // the dump renders `array_size` of the fixed-`max` buffer. We
                // pass the supplied values (clamped to `max` for memory safety —
                // the Pascal >max path reads uninitialized memory, an unpinnable
                // garbage edge), so the object's count never exceeds the buffer.
                // Pascal `AllowNone` (DSSObjectHelper.pas:592): the literal
                // `NONE` clears the array (count 0). Length-4 + case-insensitive.
                if pd.flags.contains(PropFlags::ALLOW_NONE) && value.eq_ignore_ascii_case("none") {
                    obj.set_f64_array(idx, Vec::new());
                    return Ok(0);
                }
                let max = pd.size_prop;
                let mut buf = vec![0.0; max];
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(&format!("[{value}]"));
                eng.parser.next_param(eng.vars);
                let count = eng.parser.parse_as_vector(eng.vars, &mut buf, false)?;
                if pd.scale != 1.0 {
                    for v in &mut buf {
                        *v *= pd.scale;
                    }
                }
                buf.truncate(count.min(max));
                obj.set_f64_array(idx, buf);
                Ok(0)
            }
            PropType::DoubleVArray => {
                // Pascal `DoubleVArrayProperty` + `SizeIsFunction`: the object
                // computes the element count (e.g. XSCArray = XscSize).
                let n = obj.array_size(idx);
                let mut buf = vec![0.0; n];
                interpret_dbl_array(eng.parser, eng.vars, value, n, &mut buf)?;
                if pd.flags.contains(PropFlags::NON_ZERO) && buf.contains(&0.0) {
                    eng.errors
                        .push(format!("{full}.{}: Elements cannot be zero.", pd.name));
                    return Ok(0);
                }
                if pd.scale != 1.0 {
                    for v in &mut buf {
                        *v *= pd.scale;
                    }
                }
                obj.set_f64_array(idx, buf);
                Ok(0)
            }
            PropType::DoublePoints => {
                // Pascal `SetPoints`: read every double present (the count is
                // not bounded by a size property), then split into (x, y) pairs.
                let buf = crate::util::interpret_dbl_array_dynamic(eng.parser, eng.vars, value)?;
                obj.set_points(buf);
                Ok(0)
            }
            PropType::DoubleArrayOnStruct => {
                // Pascal `DoubleArrayOnStructArrayProperty`: iterate exactly
                // `count` struct entries, skipping omitted tokens (which keep
                // the previous value), applying the scale to the rest.
                let count = obj.get_i32(pd.size_prop).max(0) as usize;
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(value);
                let mut vals = vec![None; count];
                for slot in vals.iter_mut() {
                    eng.parser.next_param(eng.vars);
                    let token = eng.parser.make_string(eng.vars);
                    if !token.is_empty() {
                        let v = eng.parser.make_double(eng.vars)? * pd.scale;
                        *slot = Some(v);
                    }
                }
                obj.set_struct_f64_array(idx, &vals);
                Ok(0)
            }
            PropType::EnumArrayOnStruct => {
                // Pascal `MappedStringEnumArrayOnStructArrayProperty`: a list of
                // enum strings, one per struct entry.
                let enum_id = pd.enum_id.expect("enum-array property needs an enum");
                let count = obj.get_i32(pd.size_prop).max(0) as usize;
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(value);
                let mut ords = Vec::with_capacity(count);
                for _ in 0..count {
                    eng.parser.next_param(eng.vars);
                    let token = eng.parser.make_string(eng.vars);
                    if token.is_empty() {
                        break;
                    }
                    let ord = eng
                        .enums
                        .get(enum_id)
                        .string_to_ordinal(&token.to_lowercase())?;
                    ords.push(ord);
                }
                obj.set_struct_i32_array(idx, &ords);
                Ok(0)
            }
            PropType::MappedStringEnumArray => {
                // Pascal `MappedStringEnumArrayProperty` + `SizeIsFunction`: a
                // list of enum strings up to the object-computed count
                // (`GetFuseStateSize`). A short input sets only the leading
                // elements; the rest keep their prior value.
                let enum_id = pd.enum_id.expect("enum-array property needs an enum");
                let count = obj.array_size(idx);
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(value);
                let mut ords = Vec::with_capacity(count);
                for _ in 0..count {
                    eng.parser.next_param(eng.vars);
                    let token = eng.parser.make_string(eng.vars);
                    if token.is_empty() {
                        break;
                    }
                    let ord = eng
                        .enums
                        .get(enum_id)
                        .string_to_ordinal(&token.to_lowercase())?;
                    ords.push(ord);
                }
                obj.set_enum_array(idx, &ords);
                Ok(0)
            }
            PropType::BusOnStruct => {
                obj.set_active_struct_bus(value);
                Ok(0)
            }
            PropType::BusesOnStruct => {
                // Pascal `BusesOnStructArrayProperty`: one bus token per struct
                // entry (`NumWindings`); omitted tokens keep the prior value.
                let count = obj.get_i32(pd.size_prop).max(0) as usize;
                eng.parser.set_auto_increment(false);
                eng.parser.set_cmd_string(value);
                let mut vals = vec![None; count];
                for slot in vals.iter_mut() {
                    eng.parser.next_param(eng.vars);
                    let token = eng.parser.make_string(eng.vars);
                    if !token.is_empty() {
                        *slot = Some(token);
                    }
                }
                obj.set_struct_buses(&vals);
                Ok(0)
            }
        }
    }
}
