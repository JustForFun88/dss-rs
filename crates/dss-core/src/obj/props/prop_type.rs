//! Property *type* tags ([`PropType`]) — the Rust form of Pascal's
//! `TPropertyType`.

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
    /// `IntegerArrayProperty`: dynamic integer array whose length is the integer
    /// property `size_prop` (e.g. a capacitor `States` over `NumSteps`).
    /// Rendered `[ i1 i2 ...]`.
    IntegerArray,
    /// `DoubleSymMatrixProperty`: a real-valued lower-triangle symmetric matrix
    /// of order `obj.get_i32(size_prop)`, stored as a flat `order²` double array
    /// (e.g. a capacitor `CMatrix`). Distinct from `SymMatrixReal` (the real
    /// *part* of a complex matrix): renders with `(...)` brackets, not `[...]`.
    DoubleSymMatrix,
    MappedStringEnum,
    MappedIntEnum,
    /// `MappedStringEnumArrayProperty` with `SizeIsFunction`: a dynamic array of
    /// mapped enums whose element count is computed by the object
    /// ([`DssObject::array_size`](crate::obj::base::DssObject::array_size)) — e.g.
    /// a Fuse `State`/`Normal` over `min(FUSEMAXDIM, NPhases)`. A short input
    /// sets only the leading elements (the rest keep their prior value). Rendered
    /// `[s1, s2, ]` (trailing comma-space, like the on-struct variant).
    MappedStringEnumArray,
    /// `StringEnumActionProperty`: the parsed value maps to an enum ordinal and
    /// immediately triggers an action
    /// ([`DssObject::do_action`](crate::obj::base::DssObject::do_action)) — e.g.
    /// a LoadShape `Action=normalize`. Stores nothing; the getter is always
    /// empty.
    Action,
    /// `BusProperty`: the value is a bus spec for terminal `size_prop`
    /// (1-based), written via
    /// [`DssObject::set_bus_name`](crate::obj::base::DssObject::set_bus_name).
    Bus,
    /// `ComplexProperty` (one `Complex` field) and `ComplexPartsProperty`
    /// (two double fields): both parse a `(re, im)` 2-vector and go through
    /// [`DssObject::set_complex`](crate::obj::base::DssObject::set_complex).
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
    /// `DSSObjectReferenceArrayProperty`: a list of references resolved against
    /// a fixed class (Pascal `PropertyOffset2 = @TheClass`), e.g. a LineGeometry
    /// `wires`/`cncables`/`tscables` writing the per-conductor `FWireData` array.
    /// The object stores the resolved objects (and validates the count); the
    /// dump renders the referenced names as `[a, b, c]` (empty → `[]`).
    ObjectRefArray,
    /// `DoubleVArrayProperty` with `SizeIsFunction`: a dynamic double array
    /// whose element count is computed by the object
    /// ([`DssObject::array_size`](crate::obj::base::DssObject::array_size)),
    /// e.g. a transformer `XSCArray` (length `(NumWindings-1)·NumWindings/2`).
    DoubleVArray,
    /// `DoubleDArrayProperty` with `WriteByFunction`/`SizeIsFunction`: an
    /// interleaved `(x, y)` point list (e.g. an XYcurve `Points`). The write
    /// reads however many doubles are present and routes them through
    /// [`DssObject::set_points`](crate::obj::base::DssObject::set_points); the
    /// read interleaves the X/Y arrays via
    /// [`DssObject::get_points`](crate::obj::base::DssObject::get_points).
    DoublePoints,
    /// `DoubleArrayOnStructArrayProperty`: writes one double per struct-array
    /// entry (e.g. a transformer `kVs` → each winding's `kVLL`). The count is
    /// the integer property `size_prop` (`NumWindings`); omitted tokens keep the
    /// previous value. Rendered `[v1, v2, ]`.
    DoubleArrayOnStruct,
    /// `MappedStringEnumArrayOnStructArrayProperty`: an enum per struct-array
    /// entry (e.g. a transformer `Conns`). Rendered `[s1, s2, ]`.
    EnumArrayOnStruct,
    /// `BusOnStructArrayProperty` (transformer `bus`): the active struct-array
    /// entry's bus (the active winding's terminal).
    BusOnStruct,
    /// `BusesOnStructArrayProperty` (transformer `buses`): one bus per struct
    /// entry, count = the integer property `size_prop` (`NumWindings`);
    /// rendered `[b1, b2, ]`.
    BusesOnStruct,
    /// `StringListProperty` (e.g. an EnergyMeter `Option`/`ZoneList`): a list of
    /// string tokens parsed via Pascal `InterpretTStringListArray`. Lists may be
    /// read/written by function (`SetOptions`/`GetOptions`), which the object
    /// resolves per ordinal. Rendered `[a, b, c]` (empty → `""`).
    StringList,
}
