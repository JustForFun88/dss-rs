//! Base per-object data and the object trait, the Rust replacement for
//! Pascal `TDSSObject` (DSSObject.pas). Pascal reaches into object fields by
//! raw pointer offset; we can't, so the generic property engine in
//! [`crate::obj::props`] drives a small typed accessor trait
//! ([`DssObject`]) that each class implements with `match idx` arms — the
//! 1:1 stand-in for `SetObjDouble`/`GetObjInteger`/... pointer pokes.

#[cfg(test)]
mod tests;

/// Shared object state every DSS object carries (`TDSSObject` fields that
/// matter to the port so far): its name and the property set-order tracker.
///
/// Properties are addressed 1-based, exactly as in Pascal, so `prp_sequence`
/// has `num_props + 1` slots and slot 0 is the monotonic counter
/// (`PrpSequence[0]`).
#[derive(Debug, Clone)]
pub struct DssObjData {
    /// Lowercased local name (`TNamedObject.LocalName`).
    name: String,
    /// Pascal `PrpSequence`: `[0]` is the counter, `[i]` is the order in
    /// which property `i` was last set (0 = never set).
    prp_sequence: Vec<u32>,
    /// Deferred `DoSimpleMsg`/`DoErrorMsg` messages emitted by
    /// `side_effects`/`end_edit` (which run without direct access to the
    /// engine error sink). The executive drains these right after the edit
    /// loop, so the message ordering within a command is preserved.
    deferred_errors: Vec<String>,
    /// Set alongside a deferred message emitted via [`Self::push_error_abort`]
    /// (the Pascal `DoErrorMsg` path, which sets `DSS.SolutionAbort := True` —
    /// `DSSGlobals.pas:265`), as opposed to [`Self::push_error`] (the
    /// `DoSimpleMsg` path, record-only). The executive lifts it into
    /// `Solution.SolutionAbort` when it drains the deferred messages.
    deferred_abort: bool,
    /// Pascal `Flg.HasBeenSaved`: set by `WriteDSSObject` when the Save
    /// serializer writes this object out, so a later `WriteClassFile` in the
    /// same session skips it. Persists across `Save` commands exactly like the
    /// Pascal flag (probe-proven 2026-07-07: a second `save load` writes 0
    /// records and deletes the file); only `Circuit.Save` clears them all
    /// (WP8.5 step 5).
    has_been_saved: bool,
    /// `TNamedObject.pUuid` (`NamedObject.pas`): the lazily-created UUID slot —
    /// `Get_UUID` makes a **random v4** on first read; the `Uuids` command
    /// preloads it. `MakeLike` does not copy it (Pascal copies fields, not
    /// `pUuid`), and our class `make_like` impls never touch `DssObjData`.
    uuid: Option<crate::cim::Uuid>,
}

impl DssObjData {
    pub fn new(name: impl Into<String>, num_props: usize) -> Self {
        Self {
            name: name.into(),
            prp_sequence: vec![0; num_props + 1],
            deferred_errors: Vec::new(),
            deferred_abort: false,
            has_been_saved: false,
            uuid: None,
        }
    }

    /// Pascal `Flg.HasBeenSaved in obj.Flags` (the `WriteClassFile` skip).
    pub fn has_been_saved(&self) -> bool {
        self.has_been_saved
    }

    /// Pascal `Include(obj.Flags, Flg.HasBeenSaved)` / the `Circuit.Save`
    /// `Exclude` reset (WP8.5 step 5).
    pub fn set_has_been_saved(&mut self, saved: bool) {
        self.has_been_saved = saved;
    }

    /// Pascal `TNamedObject.Get_UUID` (`NamedObject.pas:47-52`): return the
    /// object's UUID, creating a random v4 on first read.
    pub fn uuid(&mut self) -> crate::cim::Uuid {
        crate::cim::get_or_create_uuid(&mut self.uuid)
    }

    /// Pascal `TNamedObject.Set_UUID` (the `Uuids` command's re-assignment).
    pub fn set_uuid(&mut self, uuid: crate::cim::Uuid) {
        self.uuid = Some(uuid);
    }

    /// Queue a `DoSimpleMsg`-style message from inside a property hook; the
    /// executive collects it after the active edit finishes.
    pub fn push_error(&mut self, msg: impl Into<String>) {
        self.deferred_errors.push(msg.into());
    }

    /// Queue a `DoErrorMsg`-style message: record it like [`Self::push_error`]
    /// **and** request a solution abort (Pascal `DoErrorMsg` sets
    /// `DSS.SolutionAbort := True`, `DSSGlobals.pas:265`; `DoSimpleMsg` does
    /// not). The executive lifts the flag via [`Self::take_abort`] when it
    /// drains the deferred messages after the edit.
    pub fn push_error_abort(&mut self, msg: impl Into<String>) {
        self.deferred_errors.push(msg.into());
        self.deferred_abort = true;
    }

    /// Drain the queued messages (Pascal would have already logged them).
    pub fn take_errors(&mut self) -> Vec<String> {
        std::mem::take(&mut self.deferred_errors)
    }

    /// Take (and clear) the `DoErrorMsg` solution-abort request queued by
    /// [`Self::push_error_abort`] — the executive lifts it into
    /// `Solution.SolutionAbort` right after draining [`Self::take_errors`].
    pub fn take_abort(&mut self) -> bool {
        std::mem::take(&mut self.deferred_abort)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Pascal `Set_Name`. Names are stored lowercased by the object
    /// constructors (`Name := AnsiLowerCase(...)`); callers pass the
    /// already-normalized form.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Pascal `SetAsNextSeq`: record that property `index` was just set, so
    /// `Save` can later replay edits in the order they happened.
    pub fn set_as_next_seq(&mut self, index: usize) {
        self.prp_sequence[0] += 1;
        self.prp_sequence[index] = self.prp_sequence[0];
    }

    /// Whether property `index` was explicitly set (Pascal `PrpSpecified`).
    pub fn prp_specified(&self, index: usize) -> bool {
        self.prp_sequence.get(index).copied().unwrap_or(0) != 0
    }

    /// Pascal `PrpSequence[index] := 0`: spec-set side effects clear the
    /// "explicitly set" marks of competing properties.
    pub fn clear_seq(&mut self, index: usize) {
        if index < self.prp_sequence.len() {
            self.prp_sequence[index] = 0;
        }
    }

    /// Pascal `TDSSObject.MakeLike`: the base-class part of `like=` copies the
    /// source's whole `PrpSequence` (counter slot included) onto the target,
    /// so `Save` later writes the copied properties as explicitly set. Class
    /// `make_like` impls call this first, mirroring `inherited MakeLike`.
    pub fn copy_prp_sequence_from(&mut self, other: &DssObjData) {
        self.prp_sequence.clone_from(&other.prp_sequence);
    }

    /// Pascal `GetNextPropertySet`: the property index whose set-order is the
    /// smallest one still greater than that of `after` (pass `None` to start).
    /// Returns `None` when there are no more — drives `SaveWrite` ordering.
    pub fn next_property_set(&self, after: Option<usize>) -> Option<usize> {
        let threshold = match after {
            Some(i) => self.prp_sequence.get(i).copied().unwrap_or(0),
            None => 0,
        };
        let mut smallest = u32::MAX;
        let mut result = None;
        for (i, &seq) in self.prp_sequence.iter().enumerate().skip(1) {
            if seq != 0 && seq > threshold && seq < smallest {
                smallest = seq;
                result = Some(i);
            }
        }
        result
    }
}

/// A deferred cross-element write queued by a property setter that needs
/// *mutable* access to a different object. Pascal pokes the target through a
/// live pointer mid-parse (e.g. RegControl `TapNum` → `tr.PresentTap[w] :=`);
/// here the property engine only holds a read view of foreign classes, so the
/// setter queues the write and the executive applies it right after the edit.
/// Nothing reads the target between the two points, so the timing shift is
/// unobservable.
#[derive(Debug, Clone)]
pub enum RefAction {
    /// RegControl `TapNum`: set 1-based winding `winding`'s `PresentTap` (pu)
    /// on the target transformer (Pascal `Set_TapNum`). The target clamps to
    /// the winding's Min/MaxTap exactly like `Set_PresentTap`.
    SetTransformerTap {
        target: crate::elements::traits::ElemRef,
        winding: usize,
        tap: f64,
    },
    /// SwtControl `State=`: force every phase conductor of the controlled
    /// element's 1-based `terminal` open/closed (Pascal `State`'s side effect
    /// `ControlledElement.Closed[0] := …`). Applied generically through the
    /// target's [`CktElement`](crate::elements::traits::CktElement) base, since
    /// the switched element can be any circuit element.
    SetSwitchClosed {
        target: crate::elements::traits::ElemRef,
        terminal: usize,
        closed: bool,
    },
    /// Fuse `State=`/`Action=`: force the controlled element's 1-based
    /// `terminal` conductors **per phase** (Pascal `State`'s side effect
    /// `for i := 1 to NPhases do Closed[i] := …`). `closed[i]` is the desired
    /// state of phase conductor `i+1`; unlike [`Self::SetSwitchClosed`] (which
    /// opens/closes the whole terminal) a Fuse can leave individual phases
    /// blown. Applied generically through the target's
    /// [`CktElement`](crate::elements::traits::CktElement) base.
    SetConductorsClosed {
        target: crate::elements::traits::ElemRef,
        terminal: usize,
        closed: Vec<bool>,
    },
    /// Relay/Recloser/Fuse `RecalcElementData`: mark the controlled element as
    /// carrying an over-current-protection device for the EnergyMeter
    /// reliability sweep (Pascal `Include(ControlledElement.Flags,
    /// Flg.HasOCPDevice)`; Relay/Recloser also `HasAutoOCPDevice`). `device_type`
    /// is the `GetOCPDeviceType` ordinal (1=Fuse, 2=Recloser, 3=Relay), recorded
    /// so the section sweep can report it; `auto` distinguishes the
    /// auto-reclosing Relay/Recloser (which set `HasAutoOCPDevice`) from the
    /// Fuse (which does not). Only an *enabled* control queues this, matching the
    /// Pascal `if Enabled then Include(...)` guard. Applied through the target's
    /// [`CktElement`](crate::elements::traits::CktElement) base.
    SetOcpDevice {
        target: crate::elements::traits::ElemRef,
        device_type: i32,
        auto: bool,
    },
    /// GICsource `RecalcElementData` (GICsource.pas:350): rewrite the spliced
    /// Line's `Bus2` to the inserted `GIC_<name>` bus. Pascal pokes the target
    /// Line through `ParsePropertyValue(TLineProp.Bus2, GICBus)`; the Line's
    /// `Bus2` side effect is inert (a plain bus rename), so this applies the bus
    /// name generically through the target's
    /// [`CktElement`](crate::elements::traits::CktElement) base.
    SetElementBus {
        target: crate::elements::traits::ElemRef,
        terminal: usize,
        bus: String,
    },
}

impl RefAction {
    /// The object the action must be applied to.
    pub fn target(&self) -> crate::elements::traits::ElemRef {
        match self {
            RefAction::SetTransformerTap { target, .. } => *target,
            RefAction::SetSwitchClosed { target, .. } => *target,
            RefAction::SetConductorsClosed { target, .. } => *target,
            RefAction::SetOcpDevice { target, .. } => *target,
            RefAction::SetElementBus { target, .. } => *target,
        }
    }
}

/// A deferred file load queued by a property setter that names a data file
/// (e.g. a LoadShape `CSVFile`). The setter cannot reach the filesystem or the
/// script's current directory, so it records the request; the executive
/// resolves the path (relative to `current_dir`, like `Redirect`), reads the
/// file, and hands the contents back via [`DssObject::apply_file_load`] (text)
/// or [`DssObject::apply_binary_file_load`] (raw bytes, `binary: true` — the
/// `SngFile`/`DblFile` little-endian f32/f64 streams, WPG.1). The object then
/// parses the content with its own format rules. Nothing reads the object's
/// data between the property set and the load, so the deferral is
/// unobservable (the load still completes before `EndEdit`).
#[derive(Debug, Clone)]
pub struct FileLoad {
    /// The 1-based property index that requested the load, so the object knows
    /// which data to populate (e.g. LoadShape distinguishes `CSVFile` from
    /// `PQCSVFile`).
    pub prop: usize,
    /// The filename exactly as written in the script (unresolved).
    pub filename: String,
    /// `true` for a raw byte read (`SngFile`/`DblFile`, dispatched to
    /// [`DssObject::apply_binary_file_load`]); `false` for a line-oriented text
    /// read (`CSVFile`/`PQCSVFile`, dispatched to [`DssObject::apply_file_load`]).
    pub binary: bool,
    /// When `Some`, this is a raw memory-mapped array directive from a
    /// `mult=(sngfile=…)` / `qmult=(file=…)` command (LoadShape `CustomSetRaw`
    /// under `MemoryMapping=Yes`, `LoadShape.pas:756-800`). The file kind /
    /// column / P-vs-Q side are not encoded by [`Self::prop`] there, so they
    /// travel here. Always read as bytes (`binary = true`); text kinds decode
    /// per line. `None` for the ordinary file-property loads
    /// (`SngFile`/`DblFile`/`CSVFile`/`PQCSVFile`), whose MMF handling the
    /// readers key off `prop` + the object's `use_mmf` flag.
    pub mmf: Option<MmfLoad>,
}

/// The three memory-mapped LoadShape file kinds (Pascal `TLSFileType`,
/// `LoadShape.pas:126`): plain-text CSV/txt, little-endian `f64`, `f32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MmfKind {
    /// `file=` — ANSI text, one record per fixed-width line.
    Text,
    /// `dblfile=` — little-endian `f64` stream.
    Float64,
    /// `sngfile=` — little-endian `f32` stream (widened to `f64`).
    Float32,
}

/// Metadata for a raw MMF array directive (see [`FileLoad::mmf`]).
#[derive(Debug, Clone)]
pub struct MmfLoad {
    /// The record kind parsed from the directive's first token.
    pub kind: MmfKind,
    /// 1-based comma-delimited column for [`MmfKind::Text`] (`file=… column=N`).
    pub column: i32,
    /// `true` when the directive came from `qmult=` (store into `dQ`), else `dP`.
    pub qside: bool,
}

impl FileLoad {
    /// A text (line-oriented, e.g. `CSVFile`) deferred load.
    pub fn text(prop: usize, filename: impl Into<String>) -> Self {
        Self {
            prop,
            filename: filename.into(),
            binary: false,
            mmf: None,
        }
    }
    /// A binary (raw byte stream, `SngFile`/`DblFile`) deferred load.
    pub fn binary(prop: usize, filename: impl Into<String>) -> Self {
        Self {
            prop,
            filename: filename.into(),
            binary: true,
            mmf: None,
        }
    }
    /// A raw memory-mapped array directive (`mult=(sngfile=…)`) load; always
    /// read as bytes and dispatched to [`DssObject::apply_binary_file_load`].
    pub fn mmf_raw(prop: usize, filename: impl Into<String>, mmf: MmfLoad) -> Self {
        Self {
            prop,
            filename: filename.into(),
            binary: true,
            mmf: Some(mmf),
        }
    }
}

/// The typed field accessors the property engine calls, keyed by the 1-based
/// property index. Each concrete class implements only the kinds it actually
/// uses; the defaults panic so a wrong dispatch surfaces as an obvious bug
/// rather than silent data corruption (this mirrors the Pascal base
/// `CustomSetRaw` "base ... reached" guard).
#[allow(unused_variables)]
pub trait DssObject {
    fn data(&self) -> &DssObjData;
    fn data_mut(&mut self) -> &mut DssObjData;

    /// `&dyn Any` view for the rare flows that need a concrete downcast
    /// (`MakeLike` between circuit elements copies matrices that the typed
    /// accessors cannot express).
    fn as_any(&self) -> &dyn std::any::Any;

    /// Mutable downcast view — the control loop's bridge from an [`ElemRef`]
    /// to the concrete control/controlled types (PHASE5_PLAN §2.1: RegControl
    /// → Transformer, CapControl → Capacitor + monitored element).
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;

    /// Circuit-element view (Pascal `obj is TDSSCktElement`). `None` for
    /// `DSS_OBJECT` classes like TCC_Curve and Spectrum.
    fn as_ckt_element(&self) -> Option<&dyn crate::elements::traits::CktElement> {
        None
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn crate::elements::traits::CktElement> {
        None
    }

    fn get_f64(&self, idx: usize) -> f64 {
        unreachable!("get_f64 not implemented for property {idx}")
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        unreachable!("set_f64 not implemented for property {idx}")
    }
    fn get_i32(&self, idx: usize) -> i32 {
        unreachable!("get_i32 not implemented for property {idx}")
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        unreachable!("set_i32 not implemented for property {idx}")
    }
    fn get_bool(&self, idx: usize) -> bool {
        unreachable!("get_bool not implemented for property {idx}")
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        unreachable!("set_bool not implemented for property {idx}")
    }
    fn get_string(&self, idx: usize) -> String {
        unreachable!("get_string not implemented for property {idx}")
    }
    fn set_string(&mut self, idx: usize, value: String) {
        unreachable!("set_string not implemented for property {idx}")
    }
    /// `None` mirrors a NIL Pascal array pointer (dumps as an empty string).
    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        unreachable!("get_f64_array not implemented for property {idx}")
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        unreachable!("set_f64_array not implemented for property {idx}")
    }

    /// Pascal per-class `CustomSetRaw` hook for a `DoubleArray` property: given
    /// the raw property value *before* numeric parsing, the object may consume
    /// it directly and return `true` to skip [`Self::set_f64_array`] +
    /// `ParseAsVector`. Only LoadShape overrides it — to intercept the
    /// `mult=(sngfile=…)` / `file=…` / `dblfile=…` file directives that the
    /// numeric parser cannot read (`LoadShape.pas:746-811`). Default: `false`
    /// (the value flows to the normal numeric path unchanged).
    fn set_f64_array_raw(&mut self, idx: usize, raw: &str) -> bool {
        let _ = (idx, raw);
        false
    }

    /// Pascal `GetPropertyValue` override for a `DoubleArray` property: when
    /// the array is backed by a memory-mapped file, the dump is the original
    /// directive `(<mmFileCmd>)`, not the numeric values (`LoadShape.pas:1846-
    /// 1867`). `None` renders the normal numeric array. Only LoadShape overrides.
    fn f64_array_dump_override(&self, idx: usize) -> Option<String> {
        let _ = idx;
        None
    }
    /// `IntegerArrayProperty` read (e.g. a capacitor `States`); `None` mirrors a
    /// NIL Pascal array pointer (dumps as an empty string).
    fn get_i32_array(&self, idx: usize) -> Option<&[i32]> {
        unreachable!("get_i32_array not implemented for property {idx}")
    }
    fn set_i32_array(&mut self, idx: usize, value: Vec<i32>) {
        let _ = value;
        unreachable!("set_i32_array not implemented for property {idx}")
    }

    /// `StringListProperty` read (e.g. an EnergyMeter `Option`/`ZoneList`):
    /// the list of strings the dump renders as `[a, b, c]` (empty → `""`).
    /// Some lists are computed on read (Pascal `ReadByFunction`, e.g.
    /// `GetOptions`), so this returns by value.
    fn get_string_list(&self, idx: usize) -> Vec<String> {
        unreachable!("get_string_list not implemented for property {idx}")
    }
    /// `StringListProperty` write: the parsed token list (Pascal
    /// `InterpretTStringListArray`; `WriteByFunction` lists like `SetOptions`
    /// interpret the tokens into flags instead of storing them).
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        let _ = value;
        unreachable!("set_string_list not implemented for property {idx}")
    }

    /// `DoubleDArrayProperty` read (e.g. an XYcurve `Points`): the interleaved
    /// `[x0, y0, x1, y1, ...]` pairs, freshly built (no stable backing slice, so
    /// this returns by value unlike [`DssObject::get_f64_array`]).
    fn get_points(&self) -> Vec<f64> {
        unreachable!("get_points not implemented")
    }
    /// `DoubleDArrayProperty` write: the interleaved `[x0, y0, ...]` pairs; the
    /// implementor splits them into its X/Y arrays and resets the point count.
    fn set_points(&mut self, value: Vec<f64>) {
        let _ = value;
        unreachable!("set_points not implemented")
    }

    /// Element count of a function-sized array property (Pascal
    /// `TPropertyFlag.SizeIsFunction`, `PropertyOffset3` holding a function
    /// pointer) — e.g. an XfmrCode/Transformer `XSCArray` whose length is
    /// `(NumWindings-1)·NumWindings/2`.
    fn array_size(&self, idx: usize) -> usize {
        unreachable!("array_size not implemented for property {idx}")
    }

    /// `DoubleArrayOnStructArrayProperty` read (e.g. a transformer `kVs`): the
    /// per-winding field values, one per active struct-array entry, in raw
    /// (unscaled) units. The engine divides by `PropDef::scale` on dump.
    fn get_struct_f64_array(&self, idx: usize) -> Vec<f64> {
        unreachable!("get_struct_f64_array not implemented for property {idx}")
    }
    /// `DoubleArrayOnStructArrayProperty` write: `values[i]` is `Some` for the
    /// i-th struct-array entry (already scaled) or `None` for an omitted token
    /// (Pascal keeps the previous value). The implementor also advances the
    /// struct-array index (`ActiveWinding := count`), matching the Pascal
    /// `positionPtr^ := intVal`.
    fn set_struct_f64_array(&mut self, idx: usize, values: &[Option<f64>]) {
        let _ = values;
        unreachable!("set_struct_f64_array not implemented for property {idx}")
    }
    /// `MappedStringEnumArrayOnStructArrayProperty` read (e.g. `Conns`): the
    /// per-winding enum ordinals.
    fn get_struct_i32_array(&self, idx: usize) -> Vec<i32> {
        unreachable!("get_struct_i32_array not implemented for property {idx}")
    }
    /// `MappedStringEnumArrayOnStructArrayProperty` write: ordinals for the
    /// leading struct-array entries; also advances the struct-array index.
    fn set_struct_i32_array(&mut self, idx: usize, values: &[i32]) {
        let _ = values;
        unreachable!("set_struct_i32_array not implemented for property {idx}")
    }
    /// `MappedStringEnumArrayProperty` read (e.g. a Fuse `State`/`Normal`): the
    /// per-phase enum ordinals (at least `array_size(idx)` long).
    fn get_enum_array(&self, idx: usize) -> Vec<i32> {
        unreachable!("get_enum_array not implemented for property {idx}")
    }
    /// `MappedStringEnumArrayProperty` write: ordinals for the leading elements
    /// (a short list leaves the trailing elements unchanged).
    fn set_enum_array(&mut self, idx: usize, values: &[i32]) {
        let _ = values;
        unreachable!("set_enum_array not implemented for property {idx}")
    }

    /// `BusProperty` write: `terminal` is 1-based (`PropertyOffset`); the
    /// element lowercases and flags `BusNameRedefined` (Pascal `SetBus`).
    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        unreachable!("set_bus_name not implemented (terminal {terminal})")
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        unreachable!("get_bus_name not implemented (terminal {terminal})")
    }

    /// `BusOnStructArrayProperty` write (Pascal transformer `bus`): set the
    /// active struct-array entry's bus (the active winding's terminal).
    fn set_active_struct_bus(&mut self, value: &str) {
        let _ = value;
        unreachable!("set_active_struct_bus not implemented")
    }
    /// Read the active struct-array entry's bus.
    fn get_active_struct_bus(&self) -> String {
        unreachable!("get_active_struct_bus not implemented")
    }
    /// `BusesOnStructArrayProperty` write (Pascal transformer `buses`): set
    /// each struct-array entry's bus (`None` keeps the previous value) and
    /// advance the active index to the count.
    fn set_struct_buses(&mut self, values: &[Option<String>]) {
        let _ = values;
        unreachable!("set_struct_buses not implemented")
    }
    /// All struct-array entries' buses, one per active entry.
    fn get_struct_buses(&self) -> Vec<String> {
        unreachable!("get_struct_buses not implemented")
    }

    /// `DSSObjectReferenceProperty` write for a *resolved* reference (a
    /// `PropDef::object_ref_class`): `name` is the referenced object's name for
    /// dumps (Pascal `otherObj.Name`, `""` when unresolved), and `resolved`
    /// carries its stable [`ElemRef`] plus a read view of the object so the
    /// element can copy data immediately (Pascal `FetchLineCode` etc.). The
    /// dump value is read back through [`DssObject::get_string`].
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(
            crate::elements::traits::ElemRef,
            &dyn crate::obj::base::DssObject,
        )>,
    ) {
        let _ = (idx, name, resolved);
        unreachable!("set_object_ref not implemented for property {idx}")
    }

    /// `DSSObjectReferenceArrayProperty` write (e.g. a LineGeometry `wires`):
    /// `refs` is the parsed, resolved list — each `(name, ElemRef, read view)`
    /// in script order. The object validates the count and stores the references
    /// (Pascal `SetWires`), cloning each read view as needed (the snapshot-clone
    /// pattern). The dump value is read back through
    /// [`DssObject::get_object_ref_names`].
    fn set_object_ref_array(
        &mut self,
        idx: usize,
        refs: &[(
            String,
            crate::elements::traits::ElemRef,
            &dyn crate::obj::base::DssObject,
        )],
    ) {
        let _ = (idx, refs);
        unreachable!("set_object_ref_array not implemented for property {idx}")
    }
    /// `DSSObjectReferenceArrayProperty` read: the referenced objects' names in
    /// order (NIL slots render as the empty name `""`).
    fn get_object_ref_names(&self, idx: usize) -> Vec<String> {
        unreachable!("get_object_ref_names not implemented for property {idx}")
    }

    /// `ComplexProperty` / `ComplexPartsProperty`: both parse a 2-vector
    /// `(re, im)`; the class stores it as one `Complex` field or two doubles.
    fn get_complex(&self, idx: usize) -> (f64, f64) {
        unreachable!("get_complex not implemented for property {idx}")
    }
    fn set_complex(&mut self, idx: usize, re: f64, im: f64) {
        unreachable!("set_complex not implemented for property {idx}")
    }

    /// `ComplexPartSymMatrixProperty` write: `values` is the full `order²`
    /// column-major matrix already scaled; `real` selects the re/im part
    /// (Pascal writes one part with stride 2, preserving the other).
    fn set_matrix_part(&mut self, idx: usize, values: &[f64], order: usize, real: bool) {
        unreachable!("set_matrix_part not implemented for property {idx}")
    }
    /// Read one part of the matrix back, column-major, unscaled; `None` when
    /// the matrix is not allocated.
    fn get_matrix_part(&self, idx: usize, real: bool) -> Option<(Vec<f64>, usize)> {
        unreachable!("get_matrix_part not implemented for property {idx}")
    }

    /// Pascal `ScaledByFunction` (`PropertyOffset2` holding a function
    /// pointer): the per-class scale for property `idx`. The engine multiplies
    /// parsed values by `prop_scale(idx, false)` and divides dumps by
    /// `prop_scale(idx, true)`.
    fn prop_scale(&self, idx: usize, getter: bool) -> f64 {
        1.0
    }

    /// Pascal `TPropertyFlag.ConditionalValue` (`PropertyOffset3` holding a
    /// `LongBool`): whether property `idx`'s value should be displayed. When a
    /// `CONDITIONAL_VALUE` property returns `false` here, the getter renders
    /// the Pascal placeholder `----` instead of the stored value (e.g. a
    /// LineCode's `R1` once a matrix model has replaced the sym-component one).
    fn prop_conditional(&self, idx: usize) -> bool {
        let _ = idx;
        true
    }

    /// Pascal `PropertySideEffects`: run after property `idx` is written.
    /// `prev_int` is the integer value the property held beforehand (only
    /// meaningful for integer/boolean/enum properties; 0 otherwise).
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        let _ = (idx, prev_int);
    }

    /// Pascal `StringEnumActionProperty`: run the action whose enum ordinal is
    /// `ordinal` (e.g. a LoadShape `Action=normalize` → `Normalize`). The action
    /// runs immediately during the property parse; `errors` collects any
    /// `DoSimpleMsg` (e.g. an unported save action). No-op by default.
    fn do_action(&mut self, ordinal: i32, errors: &mut Vec<String>) {
        let _ = (ordinal, errors);
    }

    /// Pascal `TDSSObject.ParseDynVar` (DSSObject.pas l.242, overridden by
    /// `TDynEqPCE`): handle a `name=value` pair whose `name` is **not** a class
    /// property but may be a state variable of a linked `DynamicExp` (the
    /// `DynamicEq=`/`DynOut=` hosts: Generator/PVSystem/Storage). The edit loop
    /// calls this on the "unknown parameter" fallback (Pascal `DSSClass.Edit`
    /// l.1656); `true` means it was consumed, `false` ⇒ report the unknown
    /// parameter. `vars` re-evaluates the value with the executive's `Var`
    /// substitutions. Default `false` (the base `TDSSObject`).
    fn parse_dyn_var(
        &mut self,
        variable: &str,
        value: &str,
        vars: &dss_parser::ParserVars,
    ) -> bool {
        let _ = (variable, value, vars);
        false
    }

    /// Pascal per-class `EndEdit`: recompute derived state once an edit block
    /// finishes (e.g. `ReCalcYearMult`, `SetMultArray`). No-op by default.
    fn end_edit(&mut self) {}

    /// Drain the [`RefAction`]s queued during the last edit (see `RefAction`).
    /// The executive applies them right after `end_edit`.
    fn take_ref_actions(&mut self) -> Vec<RefAction> {
        Vec::new()
    }

    /// Drain the [`FileLoad`]s queued during the last edit (see [`FileLoad`]).
    /// The executive resolves each path and calls [`DssObject::apply_file_load`]
    /// *before* `end_edit`, so derived state (e.g. `SetMaxPandQ`) sees the data.
    fn take_file_loads(&mut self) -> Vec<FileLoad> {
        Vec::new()
    }

    /// Apply a resolved file's full contents to this object (the data side of a
    /// queued [`FileLoad`]). The object parses `content` per its own format.
    /// Default: ignore.
    fn apply_file_load(&mut self, load: &FileLoad, content: &str, errors: &mut Vec<String>) {
        let _ = (load, content, errors);
    }

    /// The binary counterpart of [`Self::apply_file_load`] for a `FileLoad`
    /// with `binary: true` (`SngFile`/`DblFile`). Default: ignore.
    fn apply_binary_file_load(
        &mut self,
        load: &FileLoad,
        content: &[u8],
        errors: &mut Vec<String>,
    ) {
        let _ = (load, content, errors);
    }

    /// Apply a [`RefAction`] addressed to this object (the target side of the
    /// deferred write). Default: ignore.
    fn apply_ref_action(&mut self, action: &RefAction) {
        let _ = action;
    }

    /// Pascal `TDSSObject.MakeLike`: copy `other`'s field state onto `self`
    /// (the name is *not* copied). Default is a no-op; classes override it.
    /// `other` is the same concrete class as `self`, so the implementation can
    /// read it through the typed accessors.
    fn make_like(&mut self, other: &dyn DssObject) {
        let _ = other;
    }

    /// Clone this object behind the trait object, so the executive can copy a
    /// `MakeLike` source out of its arena without aliasing the target.
    fn clone_box(&self) -> Box<dyn DssObject>;
}
