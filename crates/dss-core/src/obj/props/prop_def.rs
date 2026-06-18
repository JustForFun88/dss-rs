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
}
