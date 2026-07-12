//! One property's metadata ([`PropDef`]) and the builder API that assembles a
//! class's property rows — the Rust form of a `DSSClass.pas`
//! `DefineProperties` entry.

use crate::obj::dss_enum::EnumId;

use super::{PropFlags, PropType};

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
    /// until Phase 5 wires them). `Some("")` is the Pascal `PropertyOffset2 = 0`
    /// case (e.g. CapControl `element`): the value carries a full
    /// `Class.Name` and resolves against *any* circuit class
    /// (`GetCktElementIndex`).
    pub object_class: Option<&'static str>,
    /// `PropertyOffset2` pointing at a Pascal `TProxyClass` over two classes
    /// (RegControl's `Transf_Or_AutoTrans_ProxyClass`, `RegControl.pas:264`): the
    /// *second* class to try when `object_class` misses, so `transformer=`
    /// resolves against both `Transformer` and `AutoTrans`. `None` for the
    /// single-class case.
    pub object_class2: Option<&'static str>,
    /// Pascal `PropertyRedundantWith` (JSON default-mode sweep): the 1-based
    /// index of the property this one is a redundant alias of; 0 = none. Only
    /// meaningful together with [`PropFlags::REDUNDANT`]. Drives the
    /// `Obj_ToJSONData` `iPropNext2` deferral (`CAPI_Obj.pas:699-722`).
    pub redundant_with: usize,
    /// Pascal `PropertyArrayAlternative` (JSON): the 1-based index of the
    /// array-form alternative of this (singular) property; 0 = none. When set
    /// and `preferArray`, `GetObjPropertyJSONValue` recurses into it
    /// (`DSSObjectHelper.pas:988-998`).
    pub array_alternative: usize,
    /// Pascal `PropertyNameJSON` explicit override — the JSON key, when it is
    /// NOT the plain `%→pct` / `-→__` derivation of [`Self::name`] (e.g. Line's
    /// `Wires` → `Conductors`). `None` uses the derivation. `LowercaseKeys` never
    /// consults this (it lowercases the modern name).
    pub json_name: Option<&'static str>,
}

/// The 1-based property index of `name` within a class's `defs` vec — the index
/// [`ClassProps`](super::ClassProps) will address it by (it prepends slot 0, so
/// the ordinal is `position + 1`). Used to wire the JSON
/// `redundant_with`/`array_alternative` cross-references by name at class-build
/// time, before `ClassProps::new` appends `Like`.
pub fn prop_index(defs: &[PropDef], name: &str) -> usize {
    defs.iter()
        .position(|d| d.name.eq_ignore_ascii_case(name))
        .map(|p| p + 1)
        .unwrap_or_else(|| panic!("no property named {name}"))
}

impl PropDef {
    pub(super) fn base(name: &'static str, ptype: PropType) -> Self {
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
            object_class2: None,
            redundant_with: 0,
            array_alternative: 0,
            json_name: None,
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
    /// `IntegerArrayProperty` whose length is the integer property `size_prop`
    /// (e.g. a capacitor `States` over `NumSteps`).
    pub fn int_array(name: &'static str, size_prop: usize) -> Self {
        Self {
            size_prop,
            ..Self::base(name, PropType::IntegerArray)
        }
    }
    /// `DoubleSymMatrixProperty` of order `obj.get_i32(order_prop)` (e.g. a
    /// capacitor `CMatrix` over `phases`), stored as a flat `order²` array.
    pub fn double_sym_matrix(name: &'static str, order_prop: usize) -> Self {
        Self {
            size_prop: order_prop,
            ..Self::base(name, PropType::DoubleSymMatrix)
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
    /// `StringEnumActionProperty`: `enum_id` maps the value to an action ordinal
    /// dispatched through
    /// [`DssObject::do_action`](crate::obj::base::DssObject::do_action) (e.g. a
    /// LoadShape `Action`).
    pub fn action(name: &'static str, enum_id: EnumId) -> Self {
        Self {
            enum_id: Some(enum_id),
            ..Self::base(name, PropType::Action)
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
    /// `DSSObjectReferenceProperty` resolved against a Pascal `TProxyClass` over
    /// two classes (RegControl `transformer=` → `Transformer` | `AutoTrans`):
    /// try `class` first, then `class2`.
    pub fn object_ref_two_classes(
        class: &'static str,
        class2: &'static str,
        name: &'static str,
    ) -> Self {
        Self {
            object_class: Some(class),
            object_class2: Some(class2),
            ..Self::base(name, PropType::ObjectRef)
        }
    }
    /// `DSSObjectReferenceProperty` with `PropertyOffset2 = 0` (no fixed
    /// class): the value is a full `Class.Name` resolved against any circuit
    /// class, e.g. CapControl's `element`. Dumps render the `FullName`.
    pub fn object_ref_any(name: &'static str) -> Self {
        Self {
            object_class: Some(""),
            ..Self::base(name, PropType::ObjectRef)
        }
    }
    /// `DSSObjectReferenceArrayProperty` resolved at parse time against class
    /// `class` (Pascal `PropertyOffset2 = @TheClass`), e.g. a LineGeometry
    /// `wires`. The object stores the resolved objects.
    pub fn object_ref_array(class: &'static str, name: &'static str) -> Self {
        Self {
            object_class: Some(class),
            ..Self::base(name, PropType::ObjectRefArray)
        }
    }
    /// `DoubleVArrayProperty` whose length is computed by the object
    /// ([`DssObject::array_size`](crate::obj::base::DssObject::array_size));
    /// e.g. a transformer `XSCArray`.
    pub fn double_v_array(name: &'static str) -> Self {
        Self::base(name, PropType::DoubleVArray)
    }
    /// `DoubleVArrayProperty` with Pascal `ArrayMaxSize`: the parse reads **up
    /// to** `max` values (`PropertyOffset3`) and the object updates its own
    /// element count; the dump renders
    /// [`DssObject::array_size`](crate::obj::base::DssObject::array_size). e.g. a
    /// Recloser `RecloseIntervals`. The `NonNegative` Pascal flag on these is
    /// JSON-schema-only (the text parse accepts negatives — probed), so it is
    /// not reproduced here.
    pub fn double_v_array_max(name: &'static str, max: usize) -> Self {
        Self {
            size_prop: max,
            flags: PropFlags::ARRAY_MAX_SIZE,
            ..Self::base(name, PropType::DoubleVArray)
        }
    }
    /// `DoubleDArrayProperty` (interleaved `(x, y)` point list), e.g. an
    /// XYcurve `Points`.
    pub fn double_points(name: &'static str) -> Self {
        Self::base(name, PropType::DoublePoints)
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
    /// `MappedStringEnumArrayProperty` with `SizeIsFunction`: a mapped-enum array
    /// whose element count is computed by the object
    /// ([`DssObject::array_size`](crate::obj::base::DssObject::array_size)) — e.g.
    /// a Fuse `State`/`Normal`.
    pub fn mapped_string_enum_array(name: &'static str, enum_id: EnumId) -> Self {
        Self {
            enum_id: Some(enum_id),
            ..Self::base(name, PropType::MappedStringEnumArray)
        }
    }
    /// `BusOnStructArrayProperty` (transformer `bus`): the active winding's bus.
    pub fn bus_on_struct(name: &'static str) -> Self {
        Self::base(name, PropType::BusOnStruct)
    }
    /// `BusesOnStructArrayProperty` (transformer `buses`) over `count_prop`
    /// struct entries (the 1-based ordinal of `NumWindings`).
    pub fn buses_on_struct(name: &'static str, count_prop: usize) -> Self {
        Self {
            size_prop: count_prop,
            ..Self::base(name, PropType::BusesOnStruct)
        }
    }

    /// `StringListProperty` (Pascal `InterpretTStringListArray` +
    /// `StringListToString`); e.g. an EnergyMeter `Option`/`ZoneList`.
    pub fn string_list(name: &'static str) -> Self {
        Self::base(name, PropType::StringList)
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
    /// Pascal `PropertyRedundantWith[this] := other` (1-based). Set together
    /// with [`PropFlags::REDUNDANT`].
    pub fn redundant_with(mut self, other: usize) -> Self {
        self.redundant_with = other;
        self
    }
    /// Pascal `PropertyArrayAlternative[this] := other` (1-based).
    pub fn array_alternative(mut self, other: usize) -> Self {
        self.array_alternative = other;
        self
    }
    /// Pascal `PropertyNameJSON[this] := name` override.
    pub fn json_name(mut self, name: &'static str) -> Self {
        self.json_name = Some(name);
        self
    }

    /// The JSON key for this property under option `lowercase`. Pascal
    /// `PropertyNameJSON` (the `%→pct` / `-→__` inverse of the modern name, or an
    /// explicit override) when not lowercase; `PropertyNameLowercase`
    /// (`AnsiLowerCase` of the modern name) when [`crate::report::export::json::
    /// JsonOpts::LOWERCASE_KEYS`] is set.
    pub fn json_key(&self, lowercase: bool) -> String {
        if lowercase {
            // AnsiLowerCase of the MODERN name — the `%`/`-` are preserved,
            // only the case folds (probe-pinned: `%Mean`→`%mean`, `Wires`→
            // `wires`, ignoring the `Conductors` JSON override).
            self.name.to_ascii_lowercase()
        } else if let Some(j) = self.json_name {
            j.to_string()
        } else {
            self.name.replace('%', "pct").replace('-', "__")
        }
    }
}
