//! The per-class property table ([`ClassProps`]) and the generic
//! parse/edit/get engine that drives the typed [`DssObject`] accessors — the
//! Rust replacement for Pascal's `DSSObjectHelper.ParseObjPropertyValue` /
//! `GetObjPropertyValue`. Property indices are 1-based throughout, matching the
//! Pascal `TProp` ordinals.

use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::support::command_list::CommandList;
use crate::util::{
    float_to_str_ex, get_dss_array_f64, get_dss_array_i32, interpret_dbl_array, str_y_or_n,
};
use dss_parser::ParserError;

use super::setters::{get_double, get_integer, get_obj_double, set_obj_double, set_obj_integer};
use super::{PropDef, PropEngine, PropFlags, PropType};

/// A class's property table plus its abbreviation-matching command list — the
/// Rust stand-in for the per-`TDSSClass` `PropertyType[]`/`CommandList` state.
///
/// Properties are 1-based; slot 0 is an unused placeholder so `props[idx]`
/// lines up with the Pascal ordinals. The common `Like` (`MakeLikeProperty`) is
/// appended automatically, mirroring `inherited DefineProperties`.
#[derive(Debug)]
pub struct ClassProps {
    class_name: &'static str,
    props: Vec<PropDef>,
    command_list: CommandList,
}

impl ClassProps {
    /// Build from the class-specific property rows (1-based order, excluding
    /// `Like`). `abbrev` mirrors `CommandList.Abbrev` — `GrowthShape` is the one
    /// class that disables it.
    pub fn new(class_name: &'static str, mut defs: Vec<PropDef>, abbrev: bool) -> Self {
        defs.push(PropDef::make_like("Like"));
        let names: Vec<String> = defs.iter().map(|d| d.name.to_string()).collect();
        let mut command_list = CommandList::new(names);
        command_list.abbrev_allowed = abbrev;

        let mut props = Vec::with_capacity(defs.len() + 1);
        props.push(PropDef::base("", PropType::Integer)); // slot 0, never addressed
        props.extend(defs);

        Self {
            class_name,
            props,
            command_list,
        }
    }

    pub fn class_name(&self) -> &'static str {
        self.class_name
    }

    pub fn num_properties(&self) -> usize {
        self.props.len() - 1
    }

    pub fn prop(&self, idx: usize) -> &PropDef {
        &self.props[idx]
    }

    pub fn property_name(&self, idx: usize) -> &'static str {
        self.props[idx].name
    }

    /// Pascal `PropertyIndex`/`CommandList.GetCommand`: resolve a (possibly
    /// abbreviated) property name to its 1-based index.
    pub fn property_index(&self, name: &str) -> Option<usize> {
        self.command_list.get_command(name).map(|i| i + 1)
    }

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
                let v = get_double(eng, value)?;
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
                        let resolved = eng.foreign.and_then(|f| f.find(class, value));
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
                let v = get_integer(eng, value)?;
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

    /// One iteration of the Pascal `Edit` loop body: parse + write, record the
    /// set order (`SetAsNextSeq`), then run `PropertySideEffects`. A
    /// number-conversion failure aborts before any of the bookkeeping, exactly
    /// as the Pascal exception would unwind past `SetAsNextSeq`.
    pub fn edit_property(
        &self,
        obj: &mut dyn DssObject,
        idx: usize,
        value: &str,
        eng: &mut PropEngine,
    ) -> Result<(), ParserError> {
        let prev_int = self.parse_into(obj, idx, value, eng)?;
        obj.data_mut().set_as_next_seq(idx);
        obj.side_effects(idx, prev_int);
        Ok(())
    }

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
