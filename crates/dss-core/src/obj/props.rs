//! The generic property engine: a once-ported replacement for Pascal's
//! `DSSObjectHelper.ParseObjPropertyValue` / `GetObjPropertyValue` plus the
//! `SetObjDouble`/`SetObjInteger`/`SetObjString` flag-handling setters.
//!
//! Pascal addresses object fields by pointer offset and a parallel set of
//! `PropertyType[]`/`PropertyFlags[]`/`PropertyOffset*[]` arrays. Here a class
//! is described by a [`ClassProps`] table of [`PropDef`] rows, and the engine
//! drives the typed accessors of the [`DssObject`] trait. Property indices are
//! 1-based throughout, matching the Pascal `TProp` ordinals.
//!
//! Scope note (Phase 2): the property *types* and *flags* implemented here are
//! the ones the ported classes actually use. The remaining `TPropertyType`
//! variants (struct-array, matrix, complex, object-reference, string-list, ...)
//! and the JSON paths are added as the classes that need them are ported.

use crate::elements::traits::ElemRef;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::{EnumId, EnumRegistry};
use crate::support::command_list::CommandList;
use crate::util::{float_to_str_ex, get_dss_array_f64, interpret_dbl_array, str_y_or_n};
use dss_parser::{Parser, ParserError, ParserVars, val_f64, val_i32};

/// Pascal `TPropertyType` (subset). Discriminants are not significant here —
/// only the variant identity matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropType {
    Double,
    Integer,
    Boolean,
    String,
    MakeLike,
    DoubleArray,
    MappedStringEnum,
    MappedIntEnum,
    /// `BusProperty`: the value is a bus spec for terminal `size_prop`
    /// (1-based), written via [`DssObject::set_bus_name`].
    Bus,
    /// `ComplexProperty` (one `Complex` field) and `ComplexPartsProperty`
    /// (two double fields): both parse a `(re, im)` 2-vector and go through
    /// [`DssObject::set_complex`].
    Complex,
    /// `DoubleFArrayProperty`: fixed-size double array; the element count is
    /// `size_prop` itself (Pascal stored it in `PropertyOffset2`).
    DoubleFArray,
    /// `ComplexPartSymMatrixProperty`, real part (e.g. `rmatrix`): a
    /// lower-triangle symmetric matrix of order `obj.get_i32(size_prop)`.
    SymMatrixReal,
    /// `ComplexPartSymMatrixProperty`, imaginary part (e.g. `xmatrix`).
    SymMatrixImag,
    /// `EnabledProperty`: boolean through `set_bool`; the element's accessor
    /// performs the Pascal `Set_Enabled` side effects.
    Enabled,
    /// `DSSObjectReferenceProperty`: stored as the referenced object's name
    /// (resolution to live objects arrives with the classes that consume
    /// them — LoadShape, GrowthShape, Spectrum-as-reference, ...).
    ObjectRef,
    /// `DoubleVArrayProperty` with `SizeIsFunction`: a dynamic double array
    /// whose element count is computed by the object ([`DssObject::array_size`]),
    /// e.g. a transformer `XSCArray` (length `(NumWindings-1)·NumWindings/2`).
    DoubleVArray,
    /// `DoubleArrayOnStructArrayProperty`: writes one double per struct-array
    /// entry (e.g. a transformer `kVs` → each winding's `kVLL`). The count is
    /// the integer property `size_prop` (`NumWindings`); omitted tokens keep the
    /// previous value. Rendered `[v1, v2, ]`.
    DoubleArrayOnStruct,
    /// `MappedStringEnumArrayOnStructArrayProperty`: an enum per struct-array
    /// entry (e.g. a transformer `Conns`). Rendered `[s1, s2, ]`.
    EnumArrayOnStruct,
}

/// Pascal `TPropertyFlag` set, as a small bitset. Only the flags that affect
/// the script parse/get/text-dump path carry behavior; the rest
/// (`SuppressJSON`, `Redundant`, `RequiredInSpecSet`, `IsFilename`,
/// `GlobalCount`, ...) are recorded for fidelity but are inert in Phase 2 — they
/// only matter to the JSON/alt-order machinery, which is not ported yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PropFlags(u64);

impl PropFlags {
    pub const NONE: Self = Self(0);
    // Behavioral (checked by the setters):
    pub const NON_NEGATIVE: Self = Self(1 << 0);
    pub const NON_ZERO: Self = Self(1 << 1);
    pub const NON_POSITIVE: Self = Self(1 << 2);
    pub const GREATER_THAN_ONE: Self = Self(1 << 3);
    pub const IGNORE_INVALID: Self = Self(1 << 4);
    pub const INVERSE_VALUE: Self = Self(1 << 5);
    pub const APPLY_ROUND: Self = Self(1 << 6);
    pub const VALUE_OFFSET: Self = Self(1 << 7);
    pub const TRANSFORM_LOWERCASE: Self = Self(1 << 8);
    /// Pascal `ScaledByFunction`: the scale comes from
    /// [`DssObject::prop_scale`] instead of `PropDef::scale`.
    pub const SCALED_BY_FUNCTION: Self = Self(1 << 9);
    /// Not a Pascal flag: marks properties whose backing machinery is not
    /// ported yet (e.g. Line's `linecode`/`geometry`). Setting one is a hard
    /// error so a script silently producing wrong numbers is impossible.
    pub const NOT_PORTED: Self = Self(1 << 10);
    /// Pascal `ConditionalValue`: the getter shows `----` instead of the stored
    /// value when [`DssObject::prop_conditional`] returns false (display-only;
    /// parsing is unaffected).
    pub const CONDITIONAL_VALUE: Self = Self(1 << 11);
    // Metadata-only in Phase 2 (inert, kept for fidelity / future phases):
    pub const SUPPRESS_JSON: Self = Self(1 << 32);
    pub const REDUNDANT: Self = Self(1 << 33);
    pub const REQUIRED_IN_SPEC_SET: Self = Self(1 << 34);
    pub const IS_FILENAME: Self = Self(1 << 35);
    pub const GLOBAL_COUNT: Self = Self(1 << 36);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

impl std::ops::BitOr for PropFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for PropFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// One property's metadata — the Rust form of a `DSSClass.pas` `DefineProperties`
/// row (`PropertyType`/`PropertyFlags`/`PropertyScale`/`PropertyOffset2`/...).
#[derive(Debug, Clone)]
pub struct PropDef {
    pub name: &'static str,
    pub ptype: PropType,
    pub flags: PropFlags,
    /// Pascal `PropertyScale` (default 1.0).
    pub scale: f64,
    /// Pascal `PropertyTrapZero`: substitute this for a parsed 0 (default 0.0,
    /// meaning "no trap").
    pub trap_zero: f64,
    /// Pascal `PropertyValueOffset` (added to integers under `VALUE_OFFSET`).
    pub value_offset: f64,
    /// `PropertyOffset2` for mapped enums: which enum drives the mapping.
    pub enum_id: Option<EnumId>,
    /// `PropertyOffset2` for `DoubleArray`: the 1-based index of the integer
    /// property that holds the element count (e.g. `npts`).
    pub size_prop: usize,
    /// `PropertyOffset2` for `ObjectRef`: the name of the class the reference
    /// resolves against (Pascal `cls.Name`). `None` keeps the Phase 3
    /// behavior — the value is stored verbatim as a lowercased name string with
    /// no live resolution (still the case for Load/VSource shape references
    /// until Phase 5 wires them).
    pub object_class: Option<&'static str>,
}

impl PropDef {
    fn base(name: &'static str, ptype: PropType) -> Self {
        Self {
            name,
            ptype,
            flags: PropFlags::NONE,
            scale: 1.0,
            trap_zero: 0.0,
            value_offset: 0.0,
            enum_id: None,
            size_prop: 0,
            object_class: None,
        }
    }

    pub fn double(name: &'static str) -> Self {
        Self::base(name, PropType::Double)
    }
    pub fn integer(name: &'static str) -> Self {
        Self::base(name, PropType::Integer)
    }
    pub fn boolean(name: &'static str) -> Self {
        Self::base(name, PropType::Boolean)
    }
    pub fn string(name: &'static str) -> Self {
        Self::base(name, PropType::String)
    }
    pub fn make_like(name: &'static str) -> Self {
        Self::base(name, PropType::MakeLike)
    }
    pub fn double_array(name: &'static str, size_prop: usize) -> Self {
        Self {
            size_prop,
            ..Self::base(name, PropType::DoubleArray)
        }
    }
    pub fn mapped_string_enum(name: &'static str, enum_id: EnumId) -> Self {
        Self {
            enum_id: Some(enum_id),
            ..Self::base(name, PropType::MappedStringEnum)
        }
    }
    pub fn mapped_int_enum(name: &'static str, enum_id: EnumId) -> Self {
        Self {
            enum_id: Some(enum_id),
            ..Self::base(name, PropType::MappedIntEnum)
        }
    }
    /// `BusProperty` for `terminal` (1-based, Pascal `PropertyOffset`).
    pub fn bus(name: &'static str, terminal: usize) -> Self {
        Self {
            size_prop: terminal,
            ..Self::base(name, PropType::Bus)
        }
    }
    pub fn complex(name: &'static str) -> Self {
        Self::base(name, PropType::Complex)
    }
    /// `DoubleFArrayProperty` with a fixed element `count`.
    pub fn double_f_array(name: &'static str, count: usize) -> Self {
        Self {
            size_prop: count,
            ..Self::base(name, PropType::DoubleFArray)
        }
    }
    /// Symmetric-matrix part; `order_prop` is the 1-based index of the
    /// integer property holding the matrix order (`phases`).
    pub fn sym_matrix_real(name: &'static str, order_prop: usize) -> Self {
        Self {
            size_prop: order_prop,
            ..Self::base(name, PropType::SymMatrixReal)
        }
    }
    pub fn sym_matrix_imag(name: &'static str, order_prop: usize) -> Self {
        Self {
            size_prop: order_prop,
            ..Self::base(name, PropType::SymMatrixImag)
        }
    }
    pub fn enabled(name: &'static str) -> Self {
        Self::base(name, PropType::Enabled)
    }
    /// `DSSObjectReferenceProperty` stored as a name string only (Phase 3
    /// behavior — no live resolution). Used by Load/VSource shape refs.
    pub fn object_ref(name: &'static str) -> Self {
        Self::base(name, PropType::ObjectRef)
    }
    /// `DSSObjectReferenceProperty` resolved at parse time against class
    /// `class` (Pascal `PropertyOffset2 = @TheClass`), e.g. Line's `linecode`.
    pub fn object_ref_class(class: &'static str, name: &'static str) -> Self {
        Self {
            object_class: Some(class),
            ..Self::base(name, PropType::ObjectRef)
        }
    }
    /// `DoubleVArrayProperty` whose length is computed by the object
    /// ([`DssObject::array_size`]); e.g. a transformer `XSCArray`.
    pub fn double_v_array(name: &'static str) -> Self {
        Self::base(name, PropType::DoubleVArray)
    }
    /// `DoubleArrayOnStructArrayProperty` over `count_prop` struct entries
    /// (the 1-based ordinal of the count integer, e.g. `Windings`).
    pub fn double_array_on_struct(name: &'static str, count_prop: usize) -> Self {
        Self {
            size_prop: count_prop,
            ..Self::base(name, PropType::DoubleArrayOnStruct)
        }
    }
    /// `MappedStringEnumArrayOnStructArrayProperty` over `count_prop` struct
    /// entries (e.g. a transformer `Conns`).
    pub fn enum_array_on_struct(name: &'static str, enum_id: EnumId, count_prop: usize) -> Self {
        Self {
            enum_id: Some(enum_id),
            size_prop: count_prop,
            ..Self::base(name, PropType::EnumArrayOnStruct)
        }
    }

    pub fn flags(mut self, flags: PropFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn scale(mut self, scale: f64) -> Self {
        self.scale = scale;
        self
    }
    pub fn trap_zero(mut self, trap_zero: f64) -> Self {
        self.trap_zero = trap_zero;
        self
    }
    pub fn value_offset(mut self, value_offset: f64) -> Self {
        self.value_offset = value_offset;
        self
    }
}

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
                let n = pd.size_prop;
                let mut buf = vec![0.0; n];
                interpret_dbl_array(eng.parser, eng.vars, value, n, &mut buf)?;
                obj.set_f64_array(idx, buf);
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
            PropType::MakeLike => String::new(), // Pascal: always ''
            PropType::MappedStringEnum | PropType::MappedIntEnum => {
                let enum_id = pd.enum_id.expect("mapped enum property needs an enum");
                enums.get(enum_id).ordinal_to_string(obj.get_i32(idx))
            }
            PropType::DoubleArray => {
                let n = obj.get_i32(pd.size_prop).max(0) as usize;
                get_dss_array_f64(n, obj.get_f64_array(idx), pd.scale)
            }
            PropType::DoubleFArray => {
                get_dss_array_f64(pd.size_prop, obj.get_f64_array(idx), pd.scale)
            }
            PropType::DoubleVArray => {
                get_dss_array_f64(obj.array_size(idx), obj.get_f64_array(idx), pd.scale)
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
            PropType::Bus => obj.get_bus_name(pd.size_prop),
            PropType::Complex => {
                // TODO(phase4): match the oracle's exact complex rendering when
                // property-dump goldens cover these classes.
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

/// Shared scratch state the engine threads through, the equivalent of Pascal's
/// context-owned `PropParser`/`AuxParser`, the enum registry, and the
/// `DoSimpleMsg` sink. `parser` must be a dedicated scratch parser, not the one
/// driving the outer `Edit` loop.
pub struct PropEngine<'a> {
    pub parser: &'a mut Parser,
    pub vars: &'a ParserVars,
    pub enums: &'a EnumRegistry,
    pub errors: &'a mut Vec<String>,
    /// Read view of every class except the one being edited, alive for the
    /// duration of an edit so `ObjectRef` properties can resolve immediately
    /// (Pascal resolves `cls.Find` mid-`Edit`; see [`ForeignClassesView`]).
    /// `None` outside the executive's edit loop (unit tests, etc.).
    pub foreign: Option<&'a dyn ForeignClassesView<'a>>,
}

/// A read view of the other registered classes, the abstraction `parse_into`
/// uses to resolve an `ObjectRef` to a live object (Pascal `cls.Find`). The
/// executive implements it over the class registry minus the active class; the
/// returned `ElemRef` is stable (nothing is deleted except whole-circuit
/// `Clear`, PORTING_PLAN §2.1).
pub trait ForeignClassesView<'a> {
    /// Case-insensitive lookup of `name` in class `class`. `None` when the
    /// class or the object is unknown.
    fn find(&self, class: &str, name: &str) -> Option<(ElemRef, &'a dyn DssObject)>;
}

/// Pascal `ParseObjPropertyValue.GetDouble`: try FPC `Val` first, falling back
/// to the parser (so RPN expressions like `"2 3 *"` work) by wrapping in `()`.
fn get_double(eng: &mut PropEngine, value: &str) -> Result<f64, ParserError> {
    if let Some(v) = val_f64(value) {
        return Ok(v);
    }
    eng.parser.set_auto_increment(false);
    eng.parser.set_cmd_string(&format!("({value})"));
    eng.parser.next_param(eng.vars);
    eng.parser.make_double(eng.vars)
}

/// Pascal `ParseObjPropertyValue.GetInteger`.
fn get_integer(eng: &mut PropEngine, value: &str) -> Result<i32, ParserError> {
    if let Some(v) = val_i32(value) {
        return Ok(v);
    }
    eng.parser.set_auto_increment(false);
    eng.parser.set_cmd_string(&format!("({value})"));
    eng.parser.next_param(eng.vars);
    eng.parser.make_integer(eng.vars)
}

/// Pascal `SetObjDouble`: apply scale and the range/sign checks, the zero trap,
/// and `InverseValue`, then write. A failed check records a message and leaves
/// the field untouched (the field keeps its previous value).
fn set_obj_double(
    pd: &PropDef,
    obj: &mut dyn DssObject,
    idx: usize,
    mut value: f64,
    scale: f64,
    eng: &mut PropEngine,
    full: &str,
) {
    let f = pd.flags;
    let ignore = f.contains(PropFlags::IGNORE_INVALID);
    if f.contains(PropFlags::GREATER_THAN_ONE) && value <= 1.0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) must be greater than one.",
                pd.name
            ));
        }
        return;
    }
    if f.contains(PropFlags::NON_ZERO) && value == 0.0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be zero.",
                pd.name
            ));
        }
        return;
    }
    if f.contains(PropFlags::NON_NEGATIVE) && value < 0.0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be negative.",
                pd.name
            ));
        }
        return;
    }
    if f.contains(PropFlags::NON_POSITIVE) && value > 0.0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be positive.",
                pd.name
            ));
        }
        return;
    }

    value *= scale;
    if value == 0.0 && pd.trap_zero != 0.0 {
        value = pd.trap_zero;
    }
    if value != 0.0 && f.contains(PropFlags::INVERSE_VALUE) {
        value = 1.0 / value;
    }
    obj.set_f64(idx, value);
}

/// Pascal `GetObjDouble`: divide by scale on the way out (and invert under
/// `InverseValue`), the mirror of [`set_obj_double`].
fn get_obj_double(pd: &PropDef, obj: &dyn DssObject, idx: usize, scale: f64) -> f64 {
    let raw = obj.get_f64(idx);
    if pd.flags.contains(PropFlags::INVERSE_VALUE) {
        1.0 / (raw / scale)
    } else {
        raw / scale
    }
}

/// Pascal `SetObjInteger`: the range/sign checks plus `ValueOffset`, returning
/// the previous value (captured before the write) for side effects.
fn set_obj_integer(
    pd: &PropDef,
    obj: &mut dyn DssObject,
    idx: usize,
    mut value: i32,
    eng: &mut PropEngine,
    full: &str,
) -> i32 {
    let f = pd.flags;
    let ignore = f.contains(PropFlags::IGNORE_INVALID);
    let prev = obj.get_i32(idx);
    if f.contains(PropFlags::GREATER_THAN_ONE) && value <= 1 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) must be greater than one.",
                pd.name
            ));
        }
        return prev;
    }
    if f.contains(PropFlags::NON_ZERO) && value == 0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be zero.",
                pd.name
            ));
        }
        return prev;
    }
    if f.contains(PropFlags::NON_NEGATIVE) && value < 0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be negative.",
                pd.name
            ));
        }
        return prev;
    }
    if f.contains(PropFlags::NON_POSITIVE) && value > 0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be positive.",
                pd.name
            ));
        }
        return prev;
    }
    if f.contains(PropFlags::VALUE_OFFSET) {
        value += pd.value_offset.round_ties_even() as i32;
    }
    obj.set_i32(idx, value);
    prev
}
