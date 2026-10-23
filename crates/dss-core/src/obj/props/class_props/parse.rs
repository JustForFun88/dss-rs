//! `ClassProps::parse_into` — the value parser that drives the typed
//! [`DssObject`] setters (Pascal `DSSObjectHelper.ParseObjPropertyValue` +
//! `SetObj*`). Split out of `class_props/mod.rs` (no behavioral change).

use crate::obj::Rating;
use crate::obj::base::DssObject;
use crate::obj::props::setters::{
    get_double, get_integer, interval_units_error, parse_interval_units_f64,
    parse_interval_units_i32, set_obj_double, set_obj_integer,
};
use crate::obj::props::{PropEngine, PropFlags, PropType};
use crate::support::line_units::LineUnits;
use crate::util::{interpret_dbl_array, interpret_given_values};
use dss_parser::{ParserError, val_i32};

use super::ClassProps;

impl ClassProps {
    /// Parse `value` into property `idx` and write it through the typed
    /// accessors (Pascal `ParseObjPropertyValue` + `SetObj*`). Returns the
    /// property's previous integer value, which side effects may consult.
    /// Range/sign check failures are recorded in `eng.errors` and leave the
    /// property unchanged. A value refused at its token is returned as `Err`
    /// and leaves the property unchanged too: a property that is not ported,
    /// a value that does not read as a number or as a name of its
    /// enumeration, a rating that is not finite, a string list with an empty
    /// element before a name, a value its element refuses
    /// ([`DssObject::refuse_string_list`], [`DssObject::refuse_object_ref`],
    /// [`DssObject::refuse_integer`]) and `like=`, which only the executive
    /// applies.
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
        // EPRI r4133 upstream stub ([`PropFlags::UPSTREAM_STUB`]): the Edit loop
        // stores the raw parse string for every property before the per-property
        // arm runs (Pascal `TGenerator.Edit`,
        // `Version8/Source/PCElements/generator.pas:625`; `TSensor.Edit`,
        // `Version8/Source/Meters/Sensor.pas:253`), and a stub's own arm either
        // does nothing (Sensor `Action` — `Sensor.pas:277` assigns the `Action`
        // property whose `Set_Action` body is empty, `:850-854`) or only logs a
        // soft `DoSimpleMsg` (Generator `Rneut`/`Xneut`, `generator.pas:651-652`).
        // No field is written, nothing is recalculated, and the write is never a
        // parse error — the string round-trips through the class's own
        // `set_string`/`get_string`. That round-trip is what constrains the row's
        // declared type: `get_value` dispatches on `ptype`, so only a
        // [`PropType::String`] stub reads the stored string back (any other
        // `ptype` would render a live field that nothing ever wrote, and the
        // class's `set_string` arm would be missing entirely). `ClassProps::new`
        // `debug_assert`s that invariant for every carrier.
        if pd.flags.contains(PropFlags::UPSTREAM_STUB) {
            obj.set_string(idx, value.to_string());
            if let Some(m) = pd.stub_message {
                eng.errors
                    .push(crate::diag::DssDiagnostic::msg(m.text, Some(m.code)));
            }
            return Ok(0);
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
            PropType::Rating => {
                let rating = if Rating::is_none_token(value) {
                    Rating::NotSet
                } else {
                    let v = get_double(eng, value)?;
                    if !v.is_finite() {
                        return Err(not_finite_rating(&full, pd.name, value));
                    }
                    Rating::from_number(v)
                };
                obj.set_rating(idx, rating);
                Ok(0)
            }
            PropType::RatingArray => {
                // Up to `size_prop` entries: a number, `none`, or nothing. `-1`
                // and `none` are not set, and so is every entry not supplied.
                if let Some(spec) = crate::util::parse_dbl_array_file_spec(value) {
                    obj.data_mut()
                        .queue_dbl_array_file(crate::obj::base::GenericDblArrayFile {
                            prop: idx,
                            size_prop: pd.size_prop,
                            kind: spec.kind,
                            filename: spec.filename,
                            column: spec.column,
                            header: spec.header,
                            apply_round: false,
                            scale: 1.0,
                            non_zero: false,
                            rating: true,
                        });
                    return Ok(0);
                }
                let max = obj.get_i32(pd.size_prop).max(0) as usize;
                let buf = interpret_rating_array(eng, value, max)
                    .map_err(|e| e.unwrap_or_else(|| not_finite_rating(&full, pd.name, value)))?;
                obj.set_rating_array(idx, buf);
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
                    let at = eng.parser.position();
                    eng.parser.next_param(eng.vars);
                    let token = eng.parser.make_string(eng.vars);
                    if token.is_empty() && eng.parser.position() == at {
                        break;
                    }
                    list.push(token);
                }
                // An element that is empty or only white space names
                // nothing. Such elements at the end are dropped. One left in
                // the list stands before a name, and the list is refused at
                // its token with the element as it was
                // (`every_string_list_refuses_an_empty_element_before_a_name`).
                let blank = |name: &String| name.trim().is_empty();
                while list.last().is_some_and(blank) {
                    list.pop();
                }
                if list.iter().any(blank) {
                    return Err(ParserError::new(format!(
                        "{full}.{}: [{}] has an empty element before a name. Remove it so \
                         that each element names one entry.",
                        pd.name,
                        list.join(", ")
                    )));
                }
                if lower {
                    list.iter_mut().for_each(|name| name.make_ascii_lowercase());
                }
                // A list the element refuses is an error at this token, and
                // the element stays as it was.
                if let Some(msg) = obj.refuse_string_list(idx, &list) {
                    return Err(ParserError::new(msg));
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
                let order_found = eng
                    .parser
                    .parse_as_sym_matrix(eng.vars, &mut buf, order, 1, scale)?;
                // EPRI r4133 (WP-U1.1 item 2): an incomplete matrix is rejected —
                // the property keeps its prior value — with a DoSimpleMsg-and-
                // continue error, unlike the FPC line which silently zero-fills.
                if order_found < order {
                    eng.errors.push(sym_matrix_order_error(&full, pd.name));
                    return Ok(0);
                }
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
                        obj.set_string(idx, value.to_ascii_lowercase());
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
                            .map(|(_, full_name)| full_name.clone())
                            .unwrap_or_default();
                        obj.set_object_ref(idx, name, resolved.map(|(o, _)| o));
                    }
                    Some(class) => {
                        // NOTE(upstream-quirk): `TPropertyFlag.AllowNone` on a
                        // single object ref (SVN r4119 / fd034bb0, meant to let a
                        // `none` value clear a Recloser/Fuse TCC_Curve ref) is
                        // **observably a no-op in the Rung-1 oracle (capi015)**: its
                        // AllowNone branch (`DSSObjectHelper.pas:862`) sets `otherObj
                        // := NIL`, but the *unconditional* `if otherObj = NIL then
                        // DoSimpleMsg(... 401)` two lines down fires the "not found"
                        // error anyway (l.867-874). So `fusecurve=none` on capi015
                        // clears the ref AND logs #401 — bit-identical to the plain
                        // not-found path below, which the port already reproduces.
                        // We therefore add NO AllowNone shortcut (a silent clear
                        // would *diverge* from capi015). r4133 diverges differently
                        // (stores the literal `none` name, no #401) — a Rung-2 note,
                        // see DIVERGENCES.md §AllowNone-single-ref. WP-U1.1 item 4.
                        //
                        // EPRI r4133 `GetTccCurve('none')` (WP-U2.1, Fuse
                        // `FuseCurve` via `ALLOW_NONE_REF`): literal `none` resolves
                        // to NIL **silently** — no #401 — and renders `none`. Store
                        // the literal name so the executive re-resolve also misses
                        // (obj stays NIL). Supersedes the capi015 clear+#401 path.
                        if pd.flags.contains(PropFlags::ALLOW_NONE_REF)
                            && value.eq_ignore_ascii_case("none")
                        {
                            obj.set_object_ref(idx, "none".to_string(), None);
                            return Ok(0);
                        }
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
                        // EPRI r4133 legacy `Fetch<X>` miss (see
                        // [`PropDef::ref_miss_message`]): its own message number
                        // and text, and — unlike the #401 path below — the
                        // reference and the stored name are left untouched
                        // (`TAutoTransObj.FetchXfmrCode`'s else-arm is that one
                        // `DoSimpleMsg`, `AutoTrans.pas:2394-2395`).
                        if resolved.is_none()
                            && !value.is_empty()
                            && let Some(m) = pd.ref_miss_message
                        {
                            eng.errors.push(crate::diag::DssDiagnostic::msg(
                                format!("{}{value} not found.", m.prefix),
                                Some(m.code),
                            ));
                            return Ok(0);
                        }
                        if resolved.is_none() && !value.is_empty() {
                            // Pascal renders `cls.Name` — a `TProxyClass` is named
                            // `(Class1|Class2)` (`TProxyClass.Create`).
                            let cls_label = match pd.object_class2 {
                                Some(class2) => format!("({class}|{class2})"),
                                None => class.to_string(),
                            };
                            eng.errors.push(format!(
                                "{full}.{}: {cls_label} object \"{value}\" not found.",
                                pd.name
                            ));
                        }
                        // An object the element refuses is an error at this
                        // token, and the element stays as it was.
                        if let Some(o) = resolved.as_ref()
                            && let Some(msg) = obj.refuse_object_ref(idx, o)
                        {
                            return Err(ParserError::new(msg));
                        }
                        // The dump renders the resolved object's name (NIL → "").
                        let name = resolved.map(|o| o.name().to_string()).unwrap_or_default();
                        obj.set_object_ref(idx, name, resolved);
                    }
                }
                Ok(0)
            }
            PropType::ObjectRefArray if !pd.object_classes.is_empty() => {
                // The `conductors=` list of a Line or a LineGeometry: full
                // `Class.Name` items over WireData, CNData and TSData.
                parse_conductor_proxy(obj, idx, pd, value, eng, &full)
            }
            PropType::ObjectRefArray => {
                // Resolve every token against the class, then let the object
                // refuse the list before it stores it. A token that resolves to
                // nothing refuses the list at its own "not found". A refused
                // list writes nothing, enters no set order and runs no side
                // effect.
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
                let allow_none = pd.flags.contains(PropFlags::ALLOW_NONE_ITEM);
                let mut refs: Vec<crate::obj::base::ObjectRefArrayItem> =
                    Vec::with_capacity(names.len());
                for token in &names {
                    // With `ALLOW_NONE_ITEM` (the Line's `wires`, `cncables`
                    // and `tscables`) a `none` item is an empty slot.
                    if allow_none && token.eq_ignore_ascii_case("none") {
                        refs.push(None);
                        continue;
                    }
                    match eng.foreign.and_then(|f| f.find(class, token)) {
                        Some(o) => refs.push(Some((o.name().to_string(), o))),
                        None => {
                            return Err(ParserError::new(format!(
                                "{full}.{}: {class} object \"{token}\" not found.",
                                pd.name
                            )));
                        }
                    }
                }
                if let Some(e) = obj.refuse_object_ref_array(idx, &refs) {
                    return Err(e);
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
                } else if pd.flags.contains(PropFlags::WHOLE_NUMBER) {
                    // The number the text reads as, which the object may refuse.
                    let number = match val_i32(value) {
                        Some(v) => f64::from(v),
                        None => get_double(eng, value)?,
                    };
                    if let Some(e) = obj.refuse_integer(idx, value, number) {
                        return Err(e);
                    }
                    get_integer(eng, value)?
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
                    if pd.unit_words.is_empty() {
                        eng.enums
                            .get(enum_id)
                            .string_to_ordinal(&value.to_ascii_lowercase())?
                    } else {
                        // A length unit is one of its listed words, written
                        // whole, and the unit read is the unit stored. Any
                        // other text writes nothing and runs no side effect.
                        match LineUnits::from_word(value, pd.unit_words) {
                            Some(unit) => unit.code(),
                            None => {
                                return Err(ParserError::new(format!(
                                    "{full}.{}: \"{value}\" is not one of the length units {}.",
                                    pd.name,
                                    pd.unit_words.join(", ")
                                )));
                            }
                        }
                    }
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
                    .string_to_ordinal(&value.to_ascii_lowercase())?;
                obj.do_action(ord, eng.errors);
                Ok(0)
            }
            PropType::String => {
                let v = if pd.flags.contains(PropFlags::TRANSFORM_LOWERCASE) {
                    value.to_ascii_lowercase()
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
                // Pascal `CustomSetRaw`: a class may consume the raw value
                // before numeric parsing (LoadShape's `mult=(sngfile=…)` MMF
                // directives). If it does, skip `InterpretDblArray` entirely.
                if obj.set_f64_array_raw(idx, value) {
                    return Ok(0);
                }
                // WPG.19: a file-backed directive (`%mag=(file=…)`,
                // `Yarray=(sngfile=…)`) on the generic double-array path (Pascal
                // `DSSObjectHelper.pas:616-636` routes every double-array property
                // through `InterpretDblArray`). The read needs the filesystem +
                // `LastResultFile`, unreachable here, so queue it for the
                // executive; it reads the file, applies the shrink + round/scale/
                // non-zero, and writes the array via the typed accessors.
                if let Some(spec) = crate::util::parse_dbl_array_file_spec(value) {
                    obj.data_mut()
                        .queue_dbl_array_file(crate::obj::base::GenericDblArrayFile {
                            prop: idx,
                            size_prop: pd.size_prop,
                            kind: spec.kind,
                            filename: spec.filename,
                            column: spec.column,
                            header: spec.header,
                            apply_round: pd.flags.contains(PropFlags::APPLY_ROUND),
                            scale: pd.scale,
                            non_zero: pd.flags.contains(PropFlags::NON_ZERO),
                            rating: false,
                        });
                    return Ok(0);
                }
                let max = obj.get_i32(pd.size_prop).max(0) as usize;
                let mut buf = vec![0.0; max];
                interpret_dbl_array(eng.parser, eng.vars, value, max, &mut buf)?;
                if pd.flags.contains(PropFlags::APPLY_ROUND) {
                    // Pascal `TPropertyFlag.ApplyRound` is
                    // `doubles[i] := Round(doubles[i])` — an Int64 `Round`
                    // widened back into the Double, over raw deck array
                    // elements. Out of Int64 range the parity lane therefore
                    // stores FPC's integer-indefinite sentinel (probed:
                    // `year=[1e20]` reads back `-9.22337203685478E18`) while
                    // the default lane rounds in place; that is the round
                    // row's array kernel.
                    for v in &mut buf {
                        *v = dss_parser::compat::round_f64(*v);
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
                let order_found = eng
                    .parser
                    .parse_as_sym_matrix(eng.vars, &mut buf, order, 1, pd.scale)?;
                if order_found < order {
                    eng.errors.push(sym_matrix_order_error(&full, pd.name));
                    return Ok(0);
                }
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
            PropType::DoubleVArray if pd.flags.contains(PropFlags::GIVEN_PREFIX) => {
                // The values written, in order, at most the object's count. An
                // empty value inside the list, or one that is not a finite
                // number, writes nothing.
                let n = obj.array_size(idx);
                let (mut buf, refused) = interpret_given_values(eng.parser, eng.vars, value, n)?;
                if let Some((at, why)) = refused {
                    return Err(ParserError::new(format!(
                        "{full}.{}: value {at} of the list {why}, so the list is not applied.",
                        pd.name
                    )));
                }
                if pd.scale != 1.0 {
                    for v in &mut buf {
                        *v *= pd.scale;
                    }
                }
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
                        .string_to_ordinal(&token.to_ascii_lowercase())?;
                    ords.push(ord);
                }
                obj.set_struct_i32_array(idx, &ords);
                Ok(0)
            }
            PropType::MappedStringEnumArray => {
                // Class-owned raw-value writer (r4133 `InterpretSwitchState`
                // shape: ganged-vs-per-phase keyed on `WasQuoted`, first-char
                // token match — [`DssObject::set_enum_array_raw`]). SwtControl
                // `Normal`/`State` take it; every other enum array falls
                // through to the generic tokenizer below.
                if obj.set_enum_array_raw(idx, value, eng.was_quoted) {
                    return Ok(0);
                }
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
                        .string_to_ordinal(&token.to_ascii_lowercase())?;
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

/// The `conductors=` list of a Line or a LineGeometry, filled into the
/// conductor array from its first slot: each item is the `Class.Name` of a
/// WireData, CNData or TSData object, the class matched in any case, or
/// `none` for an empty slot. The list is refused at its token, writing
/// nothing, entering no set order and running no side effect, when the
/// array is not sized yet (402 "No objects are expected!", before any
/// item), when an item names no class (10103 "You must define the
/// `<Proxy>` class for all the valid items in the array."), when it names
/// another class (10103 "Invalid class (`<lowercased>`) for item. Valid
/// classes: (`WireData|CNData|TSData`)") and when its object is not found.
/// Pinned by `exec::tests::line_geometry_rules::a_refused_conductor_list_changes_nothing`.
fn parse_conductor_proxy(
    obj: &mut dyn DssObject,
    idx: usize,
    pd: &crate::obj::props::PropDef,
    value: &str,
    eng: &mut PropEngine,
    full: &str,
) -> Result<i32, ParserError> {
    let allow_none = pd.flags.contains(PropFlags::ALLOW_NONE_ITEM);
    let proxy_name = pd.proxy_name.unwrap_or("Conductor");
    let valid_classes = format!("({})", pd.object_classes.join("|"));

    // The conductor array is sized by a spacing or `nconds=`.
    let count = obj.array_size(idx);
    if count < 1 {
        return Err(ParserError::new(format!(
            "{full}.{}: No objects are expected! Check if the order of property \
             assignments is correct.",
            pd.name
        ))
        .with_code(402));
    }

    // At most `count` items, up to the first empty token.
    eng.parser.set_auto_increment(false);
    eng.parser.set_cmd_string(value);
    let mut refs: Vec<crate::obj::base::ObjectRefArrayItem> = Vec::new();
    for _ in 0..count {
        eng.parser.next_param(eng.vars);
        let token = eng.parser.make_string(eng.vars);
        if token.is_empty() {
            break;
        }
        if allow_none && token.eq_ignore_ascii_case("none") {
            refs.push(None);
            continue;
        }
        let lower = token.to_ascii_lowercase();
        let (class_tok, name_tok) = match lower.split_once('.') {
            Some((c, n)) => (c, n),
            None => {
                let items = if allow_none { "valid items" } else { "items" };
                return Err(ParserError::new(format!(
                    "{full}.{}: You must define the {proxy_name} class for all the \
                     {items} in the array.",
                    pd.name
                ))
                .with_code(10103));
            }
        };
        let subcls = pd
            .object_classes
            .iter()
            .find(|c| c.eq_ignore_ascii_case(class_tok))
            .copied();
        let Some(subcls) = subcls else {
            return Err(ParserError::new(format!(
                "{full}.{}: Invalid class ({class_tok}) for item. Valid classes: \
                 {valid_classes}",
                pd.name
            ))
            .with_code(10103));
        };
        match eng.foreign.and_then(|f| f.find(subcls, name_tok)) {
            Some(o) => refs.push(Some((o.name().to_string(), o))),
            None => {
                return Err(ParserError::new(format!(
                    "{full}.{}: {subcls} object \"{token}\" not found.",
                    pd.name
                )));
            }
        }
    }
    obj.set_object_ref_array(idx, &refs);
    Ok(0)
}

/// The refusal of a rating that is not a finite number: a rating is a number,
/// `none` or `-1`, and the edit leaves the property as it was
/// (`a_rating_that_is_not_a_finite_number_is_refused`).
pub(crate) fn not_finite_rating(full: &str, prop: &str, value: &str) -> ParserError {
    ParserError::new(format!(
        "{full}.{prop}: the rating {value} is not a finite number."
    ))
}

/// The inline value of a [`PropType::RatingArray`], whose caller has queued a
/// `file=`, `dblfile=` or `sngfile=` value already: `max` entries, each a
/// number, `none`, or nothing. `none`, `-1` and an entry not supplied read as a
/// rating that is not set. An entry that is not a finite number fails with
/// `None`, a parse error with its own error.
fn interpret_rating_array(
    eng: &mut PropEngine,
    value: &str,
    max: usize,
) -> Result<Vec<Rating>, Option<ParserError>> {
    eng.parser.set_auto_increment(false);
    eng.parser.set_cmd_string(value);
    eng.parser.next_param(eng.vars);
    let mut out = Vec::with_capacity(max);
    for _ in 0..max {
        let token = eng.parser.make_string(eng.vars);
        let rating = if token.is_empty() || Rating::is_none_token(&token) {
            Rating::NotSet
        } else {
            let v = eng.parser.make_double(eng.vars)?;
            if !v.is_finite() {
                return Err(None);
            }
            Rating::from_number(v)
        };
        out.push(rating);
        eng.parser.next_param(eng.vars);
    }
    Ok(out)
}

/// EPRI r4133 `ParseAsSymMatrix` incomplete-matrix message (`ParserDel.pas`:
/// "The matrix entered does not match with the expected order…"). Prefixed with
/// the element/property like the other `DoSimpleMsg`-and-continue diagnostics so
/// a bad deck is greppable in the error log. WP-U1.1 item 2 / DIVERGENCES.md.
fn sym_matrix_order_error(full: &str, name: &str) -> String {
    format!(
        "{full}.{name}: The matrix entered does not match with the expected order, \
         review the entered parameters and try again."
    )
}
