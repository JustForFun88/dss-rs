//! Property *flag* set ([`PropFlags`]) — the Rust form of Pascal's
//! `TPropertyFlag`.

/// Pascal `TPropertyFlag` set, as a small bitset. Only the flags that affect
/// the script parse/get/text-dump path carry behavior there. The rest
/// (`SuppressJSON`, `Redundant`, `RequiredInSpecSet`, `IsFilename`,
/// `GlobalCount`, ...) describe the property to the JSON export, the schema,
/// the alternate property order and `Save`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PropFlags(u128);

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
    /// [`DssObject::prop_scale`](crate::obj::base::DssObject::prop_scale)
    /// instead of `PropDef::scale`.
    pub const SCALED_BY_FUNCTION: Self = Self(1 << 9);
    /// Not a Pascal flag: marks properties whose backing machinery is not
    /// ported yet (e.g. Line's `linecode`/`geometry`). Setting one is a hard
    /// error so a script silently producing wrong numbers is impossible.
    pub const NOT_PORTED: Self = Self(1 << 10);
    /// Pascal `ConditionalValue`: the getter shows `----` instead of the stored
    /// value when
    /// [`DssObject::prop_conditional`](crate::obj::base::DssObject::prop_conditional)
    /// returns false (display-only; parsing is unaffected).
    pub const CONDITIONAL_VALUE: Self = Self(1 << 11);
    /// Pascal `ArrayMaxSize`: a `DoubleVArray` whose parse reads **up to**
    /// `PropDef::size_prop` values (`PropertyOffset3` in Pascal) — not a fixed
    /// count. The parser sets the object's own element count to however many
    /// were supplied (`integerPtr^ := ParseAsVector(...)`), and the dump renders
    /// [`DssObject::array_size`](crate::obj::base::DssObject::array_size) of
    /// them. Used by Recloser/Relay `RecloseIntervals`.
    pub const ARRAY_MAX_SIZE: Self = Self(1 << 12);
    /// Pascal `AllowNone`: a `DoubleVArrayProperty` whose **dump renders
    /// `[NONE]`** (not `[]`) when its element count is 0
    /// (`DSSObjectHelper.pas:2274`), and whose parse accepts the literal `NONE`
    /// to clear it. Used by Relay `RecloseIntervals` (`Type=DOC` ⇒ NumReclose 0).
    pub const ALLOW_NONE: Self = Self(1 << 13);
    /// Pascal `IntervalUnits` (`DSSObjectHelper.pas` l.273/325): an Integer/Double
    /// property whose value may carry a single trailing time-unit char — `h`
    /// (×3600), `m` (×60), or `s` (×1); a bare number is seconds. A bad number or
    /// unit logs the upstream error and leaves the field unchanged. Used by
    /// InvControl `AvgWindowLen` / `DynReacAvgWindowLen`.
    pub const INTERVAL_UNITS: Self = Self(1 << 14);
    /// dss_capi 0.14.5 `[SilentReadOnly, ReadByFunction]` on a double whose
    /// value comes from a read function rather than a field -- IndMach012 `PF`
    /// (`PowerFactor(Power[1])`) and StorageController
    /// `kWhTotal`/`kWTotal`/`kWhActual`/`kWActual` (the fleet aggregates), the
    /// only two classes that carry it. A set is silently ignored, the JSON
    /// export omits the key, a JSON load skips it, and the schema marks it
    /// `readOnly`.
    ///
    /// The text render (`?`, `Dump`) of such a property is `''` unless it also
    /// carries [`Self::RENDERS_LIVE_RESULT`]. r4133 renders the live value of
    /// all five properties above, so all five carry it, in both lanes. A
    /// property whose value has no live render keeps the `''` by not carrying
    /// it.
    ///
    /// The four readers, and what each does with this flag:
    ///
    /// | reader | behavior |
    /// |---|---|
    /// | `ClassProps::get_value` -- `?` / `Dump` / `all_properties` | `''`, **unless** [`Self::RENDERS_LIVE_RESULT`] is also present |
    /// | `ClassProps::get_json_value` (`class_props/json.rs`) | omits the key |
    /// | `ClassProps::set_json` (`class_props/json_set.rs`) | ignores the key on load |
    /// | the JSON schema walk (`report/export/json/schema/classes.rs`) | `readOnly: true` |
    pub const SILENT_READ_ONLY: Self = Self(1 << 15);
    /// Not a Pascal flag: marks a read-only result property whose string render
    /// reads the live `cd.vterminal` cache (Transformer `WdgCurrents`; AutoTrans
    /// `WdgCurrents` when ported). Pascal's getter is self-sufficient — it reloads
    /// `Vterminal[i] := Solution.NodeV[NodeRef[i]]` internally
    /// (`TTransfObj.GetAllWindingCurrents`, `Transformer.pas` l.1538) — but the
    /// Rust `&self` getter cannot reach the solution, so the `?`/`Dump` read
    /// surfaces refresh the buffer at one choke point,
    /// `Dss::refresh_vterminal_if_marked`, exactly when this flag is present.
    /// Future note: a result property whose render needs `iterminal` must add a
    /// *separate* marker whose refresh runs `compute_iterminal` **then**
    /// `compute_vterminal` (VSource's `GetCurrents` overwrites `vterminal` with
    /// the source EMF — see `export_elem_powers` in `report/export/elem.rs`). No
    /// Pascal property-table read needs `iterminal` today.
    pub const READS_VTERMINAL: Self = Self(1 << 16);
    /// Pascal `ReplaceZero`, with r4133 `DblValueNZ` semantics: a parsed double
    /// in the open band `(-1e-8, 1e-8)` is replaced by `+1e-8`, always, with no
    /// compat flag and no error. dss_capi 0.15.x instead replaces only an exact
    /// zero, behind its `PermissiveProperties` flag, and refuses a zero by
    /// default. Carried by the essential-sizing doubles (Load `kW`/`kVA`,
    /// Generator `kW`/`kVA` — but NOT Generator `MVA`, which uses plain
    /// `DblValue*1000` in r4133 — Storage `kW`/`kVA`, PVSystem `kVA`, WindGen
    /// `kW`/`kVA`/`MVA`). Applied pre-scale, matching where
    /// `DblValueNZ`/`ReplaceZero` sit in the parse.
    pub const REPLACE_ZERO: Self = Self(1 << 17);
    /// Pascal `AllowNoneItem`: on a `DSSObjectReferenceArrayProperty`
    /// (Line/LineGeometry `Wires`/`CNCables`/`TSCables`), a list entry of
    /// literal `none` resolves to a **NIL slot** instead of the "object not
    /// found" error. Distinct from [`Self::ALLOW_NONE`] (single ref /
    /// DoubleVArray).
    pub const ALLOW_NONE_ITEM: Self = Self(1 << 18);
    /// **Not a Pascal flag.** Marks a [`Self::SILENT_READ_ONLY`] property whose
    /// **text render is a live computed result** in EPRI r4133, so the
    /// `?` / `Dump` / `all_properties` gate must reach the value instead of
    /// returning `''` ([`Self::SILENT_READ_ONLY`] keeps its other three
    /// readers).
    ///
    /// It carries a second duty, the one [`Self::READS_VTERMINAL`] carries for
    /// Transformer `WdgCurrents`: the r4133 getter is self-sufficient because it
    /// holds live pointers (`GetkWhTotal` re-sums `FleetPointerList` on every
    /// call), while the Rust `&self` getter reaches neither the solution nor
    /// another class's arena. So the render surfaces refresh the object's
    /// live-result cache before reading it, exactly when this flag is present,
    /// and the `&self` getter then returns the just-refreshed number: `?`,
    /// `Dump` and `element_properties` one object at a time at
    /// `Dss::refresh_vterminal_if_marked`; `Save` — which walks whole classes
    /// and has no single property to gate on — in one up-front pass,
    /// `Dss::refresh_render_caches_for_save`. The **fifth** reader of
    /// `ClassProps::get_value`, `batchedit`'s `where <prop> <op> <x>` filter, is
    /// `&self` and refreshes nothing, which no corpus cell observes.
    ///
    /// **A read stays a pure read of the model**, on both holders. The OpenDSS
    /// engines mutate the object on both reads. This engine does not: a
    /// property read must not change the state it reports.
    ///
    /// * r4133's `GetkWhTotal(Var Sum)` / `GetkWTotal(Var Sum)` write their sum
    ///   back into the object's own `TotalkWhCapacity`/`TotalkWCapacity`. Nothing
    ///   reads those fields, and the getters re-sum the fleet from scratch
    ///   anyway, so this engine renders the same number without the write.
    /// * `IndMach012.pf` reads `Power[1]`, whose `ComputeIterminal` recomputes
    ///   the machine model while the `Iterminal` cache is unstamped. That
    ///   recompute advances the slip-Newton by one step, moving the machine's
    ///   own rendered `Slip`. This engine runs the recompute on a throwaway
    ///   clone and keeps only the number (`IndMach012::refresh_live_pf`).
    ///
    /// Holders: StorageController `kWhTotal`/`kWTotal`/`kWhActual`/`kWActual`
    /// and IndMach012 `PF`, nothing else.
    pub const RENDERS_LIVE_RESULT: Self = Self(1 << 19);
    /// An Integer property whose value is read as a number first and handed
    /// to [`DssObject::refuse_integer`], so the object can
    /// refuse a fraction or a number outside its range, naming the written
    /// text, instead of the rounding every other integer gets. Carried
    /// by LineGeometry `Cond` and `NConds`.
    ///
    /// [`DssObject::refuse_integer`]: crate::obj::base::DssObject::refuse_integer
    pub const WHOLE_NUMBER: Self = Self(1 << 20);
    /// A function-sized `DoubleVArray` that stores the values given, in order,
    /// at most its size, instead of a buffer of its size padded with zeros. A
    /// position the list does not reach holds no value, and an empty value
    /// followed by another value, or a value that is not a finite number, is
    /// refused, naming its place. Carried by LineSpacing `X` and `H`.
    pub const GIVEN_PREFIX: Self = Self(1 << 21);
    /// Not a Pascal flag: a current rating the element derives from its kVA
    /// rating, read only by the documentation. Writing it is refused whatever
    /// the value, naming the property that sets the rating, and the derived
    /// rating stays. Carried by the `NormAmps` and `EmergAmps` of Transformer
    /// and AutoTrans. The schema walk and the JSON writer do not read it.
    pub const READ_ONLY_RATING: Self = Self(1 << 22);
    /// Not a Pascal flag: a kVA or device rating held as a plain number. A
    /// value that is not a finite number is refused at its token, naming the
    /// rating, and the property keeps what it held. Carried by the
    /// `NormHkVA`, `EmergHkVA` and `Ratings` of the transformer classes and
    /// the `RatedCurrent` and `InterruptingRating` of the protective devices
    /// (`a_typed_non_finite_kva_or_device_rating_is_refused`).
    pub const FINITE_RATING: Self = Self(1 << 23);
    /// Not a Pascal flag: a rated voltage or kVA base, which the
    /// documentation gives as a positive number. A value that is not a finite
    /// number (an array with any such entry) is refused at its token and the
    /// property keeps what it held
    /// (`a_typed_non_finite_rated_voltage_or_kva_base_is_refused_at_its_token`).
    pub const FINITE_BASE: Self = Self(1 << 24);
    // Metadata, read by the JSON export, the schema and `Save`:
    pub const SUPPRESS_JSON: Self = Self(1 << 32);
    pub const REDUNDANT: Self = Self(1 << 33);
    pub const REQUIRED_IN_SPEC_SET: Self = Self(1 << 34);
    pub const IS_FILENAME: Self = Self(1 << 35);
    pub const GLOBAL_COUNT: Self = Self(1 << 36);
    /// Pascal `DynamicDefault`: the default is recomputed from other properties
    /// (e.g. `kWBand` from `%kWBand`); only affects JSON-default elision.
    pub const DYNAMIC_DEFAULT: Self = Self(1 << 37);
    /// Pascal `Units_hour`: documents the unit of a time property (JSON schema
    /// metadata only).
    pub const UNITS_HOUR: Self = Self(1 << 38);
    /// Pascal `NoDefault`: the property has no default value, so it is never
    /// elided on JSON export (and is flagged in the schema). Like
    /// [`Self::DYNAMIC_DEFAULT`] this only affects the JSON export and schema.
    /// It is inert for the text dump and the property setters.
    pub const NO_DEFAULT: Self = Self(1 << 39);
    /// Pascal `Units_ohm_per_length`: documents the unit (`Ω/[length_unit]`) of a
    /// per-length resistance/reactance property. Like the other `Units_*` flags
    /// it is consumed only by the CAPI JSON-schema export (`getPropertyUnits`),
    /// so it is inert for the text dump and the property setters.
    pub const UNITS_OHM_PER_LENGTH: Self = Self(1 << 40);
    /// Pascal `AltIndex`: a substructure-index property (e.g. a winding/wire
    /// selector). The JSON `Obj_ToJSONData` sweep skips it in both modes
    /// (`CAPI_Obj.pas:727/744`).
    pub const ALT_INDEX: Self = Self(1 << 41);
    /// Pascal `IntegerStructIndex`: the integer that selects the active
    /// struct-array entry (Transformer `Wdg`). Skipped by the JSON sweep like
    /// [`Self::ALT_INDEX`].
    pub const INTEGER_STRUCT_INDEX: Self = Self(1 << 42);
    /// Pascal `OnArray`: a scalar-per-struct/array property whose JSON value,
    /// under `preferArray`, is the full array over the struct-array count
    /// (`DSSObjectHelper.pas:1110/1123`).
    pub const ON_ARRAY: Self = Self(1 << 43);
    /// Pascal `FullNameAsJSONArray`: a `DSSObjectReferenceArrayProperty` whose
    /// JSON array uses each referenced object's `FullName` regardless of the
    /// `FullNames` option (`DSSObjectHelper.pas:1494`; Line `Wires`).
    pub const FULL_NAME_AS_JSON_ARRAY: Self = Self(1 << 44);
    /// Pascal `FullNameAsArray`: a `DSSObjectReferenceProperty` (scalar/on-array)
    /// whose JSON uses `FullName` regardless of `FullNames`
    /// (`DSSObjectHelper.pas:1175`).
    pub const FULL_NAME_AS_ARRAY: Self = Self(1 << 45);
    /// Pascal `Deprecated`: the property still parses/stores normally but is
    /// flagged deprecated in the JSON schema (`CAPI_Schema.pas:79/1277`,
    /// `deprecationMessage`). Unlike `DeprecatedAndRemoved` (a *ptype* that rejects
    /// the write with a message) it emits **no** runtime warning — probe-confirmed
    /// on capi015 that `New LineCode.x faultrate=…` still stores the value
    /// silently. Inert for the text dump and the setters. Carried by
    /// LineCode `FaultRate`/`PctPerm`/`Repair`.
    pub const DEPRECATED: Self = Self(1 << 46);
    /// Pascal `Unused`: the property is accepted for backward compatibility but is
    /// not consumed by the engine (`CAPI_Schema.pas:74`). Schema-only, inert here.
    pub const UNUSED: Self = Self(1 << 47);
    /// **Not a Pascal flag.** A dss_capi 0.15.x property kept off the
    /// *full-enumeration* surfaces whose byte goldens are pinned to dss_capi
    /// 0.14.5, which has no such property: the `Dump` text report
    /// ([`report::save::dump`](crate::report::save::dump)), the `Dump commands`
    /// help catalog, the AltDSS JSON export and the JSON schema walk. The
    /// property keeps its ordinal, so the properties after it keep their index.
    /// The named-query (`?`) and props-table surfaces still expose it (the
    /// props-table comparison lists it in the `PROPS_015X` allowlist). No solved
    /// number depends on the flag. Carried by Line
    /// `EpsRMedium`/`HeightOffset`/`HeightUnit`/`Conductors` and LineGeometry
    /// `Conductors`, the set
    /// `exec::tests::compat_quirks::hide_015x_carrier_set_is_the_measured_escape`
    /// pins.
    ///
    /// Line's JSON `Conductors` key is written by its `Wires` property
    /// (`json_name = "Conductors"`). Exposing the real `Conductors` while that
    /// alias stays would write the key twice in one object, so the flag and the
    /// alias go together, when the goldens of these surfaces are regenerated on
    /// an engine that has the properties. The alias is pinned by
    /// `exec::tests::compat_quirks::line_json_conductors_key_is_owned_by_the_masquerade`.
    pub const HIDE_015X: Self = Self(1 << 48);
    /// **r4133 `GetTccCurve('none')` semantics.** On a single
    /// `DSSObjectReferenceProperty` (a TCC_Curve ref), a value of literal
    /// `none` resolves to a **NIL reference silently** — no #401 "not found" —
    /// and the stored name renders as `none`. Distinct from the dss_capi 0.15.x
    /// path, which clears the reference **and** reports #401 (the
    /// [`Self::ALLOW_NONE`] note). Carried by Fuse `FuseCurve` only.
    pub const ALLOW_NONE_REF: Self = Self(1 << 49);
    /// **Not a Pascal flag.** An **r4133-only** property kept off the
    /// *full-enumeration* surfaces whose byte goldens are pinned to dss_capi
    /// 0.14.5 (the `Dump` text report, the `Dump commands` help catalog, the
    /// AltDSS JSON export and the JSON schema walk). Sibling of
    /// [`Self::HIDE_015X`]: a 0.15.x property is absent from 0.14.5 only, an
    /// r4133 property from both the 0.14.5 and the 0.15.x property tables. The
    /// named-query (`?`) and props-table surfaces still expose it (the
    /// props-table comparison lists it in the `PROPS_015X` allowlist, by name,
    /// independent of this flag). No solved number depends on the flag.
    ///
    /// Carriers: Generator `Rneut`/`Xneut` and Sensor `Action` (all three also
    /// [`Self::UPSTREAM_STUB`]), and AutoTrans `XfmrCode`, a fully implemented
    /// property: the criterion is "absent from both pinned tables", never "not
    /// implemented". Every carrier still occupies an ordinal, which is why the
    /// schema's per-class byte gate carries a `port_hidden_property` row for
    /// each (`tests/golden/json/schema_divergences.json`). The carrier set is
    /// pinned by
    /// `exec::tests::compat_quirks::hide_015x_carrier_set_is_the_measured_escape`.
    ///
    /// **`Save` is deliberately not one of the hidden surfaces.** It writes only
    /// the properties a deck explicitly set, with no flag filter: r4133 writes
    /// `Rneut=` back for a deck that set it, and so does this engine. Pinned by
    /// `exec::tests::upstream_stubs::save_writes_the_stub_names_like_r4133`.
    ///
    /// The flag goes when the `dump*`, `dump3*` and `json/` goldens stop being
    /// pinned to 0.14.5. `json/schema_full_oracle.json` and
    /// `schema_divergences.json` stay as they are.
    pub const HIDE_R4133: Self = Self(1 << 50);

    /// Pascal `TPropertyFlag.Ordering_First` (`DSSClass.pas:216`): this property
    /// is moved to the very front of the alternate load/save order
    /// (`AltPropertyOrder`, right after `Like`), so the JSON reader
    /// (`FillObjFromJSON`) applies it before the rest — e.g. Transformer
    /// `XfmrCode`, Line `Switch`, LoadShape `MemoryMapping`. Inert on every path
    /// except [`ClassProps::alt_property_order`](crate::obj::props::ClassProps).
    pub const ORDERING_FIRST: Self = Self(1 << 51);
    /// Pascal `TPropertyFlag.Ordering_Last` (`DSSClass.pas:217`): this property
    /// is moved to the very end of `AltPropertyOrder` (with the action
    /// properties), so the JSON reader applies it after every other property —
    /// e.g. Load `PF`. Inert except in `alt_property_order`.
    pub const ORDERING_LAST: Self = Self(1 << 52);
    /// Pascal `TPropertyFlag.Required` (`DSSClass.pas:210`): the property MUST be
    /// present in the AltDSS JSON object; `FillObjFromJSON` raises
    /// `JSON/<cls>/<name>: required property not provided: "<prop>"` when it is
    /// absent (`DSSObjectHelper.pas:4955`). Consumed only by the JSON *import*
    /// path ([`ClassProps::fill_from_json`](crate::obj::props::ClassProps));
    /// inert on every other surface. The Redundant Required twins (Transformer/
    /// AutoTrans/XfmrCode `buses`/`kVs`) are dropped from `alt_property_order`, so
    /// only the non-redundant Required props are ever checked — flagging the
    /// per-winding `bus`/`kV` (which the oracle always exports) suffices.
    pub const REQUIRED: Self = Self(1 << 53);

    // --- The `Units_*` property-flag family (schema metadata only) ---------
    // Pascal `TPropertyFlag.Units_*`: each documents a property's physical
    // unit, consumed only by the JSON-schema export, which checks them in a
    // fixed order (the first set flag wins). Inert on the text dump and the
    // property setters, like [`Self::UNITS_HOUR`] / [`Self::UNITS_OHM_PER_LENGTH`],
    // which sit above with bits 38 / 40.
    pub const UNITS_HZ: Self = Self(1 << 54);
    pub const UNITS_PU_VOLTAGE: Self = Self(1 << 55);
    pub const UNITS_PU_CURRENT: Self = Self(1 << 56);
    pub const UNITS_PU_POWER: Self = Self(1 << 57);
    pub const UNITS_PU_IMPEDANCE: Self = Self(1 << 58);
    pub const UNITS_OHM_METER: Self = Self(1 << 59);
    pub const UNITS_OHM: Self = Self(1 << 60);
    pub const UNITS_NF_PER_LENGTH: Self = Self(1 << 61);
    pub const UNITS_UF: Self = Self(1 << 62);
    pub const UNITS_MH: Self = Self(1 << 63);
    pub const UNITS_US_PER_LENGTH: Self = Self(1 << 64);
    pub const UNITS_S: Self = Self(1 << 65);
    /// Pascal `Units_ToD_hour`: a time-of-day hour. The schema renders it with
    /// `minimum:0` / `exclusiveMaximum:24` and the unit string `hour`
    /// (`CAPI_Schema.pas:1000-1006`).
    pub const UNITS_TOD_HOUR: Self = Self(1 << 66);
    pub const UNITS_MINUTE: Self = Self(1 << 67);
    pub const UNITS_V: Self = Self(1 << 68);
    pub const UNITS_W: Self = Self(1 << 69);
    pub const UNITS_KW: Self = Self(1 << 70);
    pub const UNITS_KVAR: Self = Self(1 << 71);
    pub const UNITS_KVA: Self = Self(1 << 72);
    pub const UNITS_MVA: Self = Self(1 << 73);
    pub const UNITS_KWH: Self = Self(1 << 74);
    pub const UNITS_V_PER_KM: Self = Self(1 << 75);
    pub const UNITS_DEG: Self = Self(1 << 76);
    pub const UNITS_DEGC: Self = Self(1 << 77);
    pub const UNITS_A: Self = Self(1 << 78);
    pub const UNITS_KV: Self = Self(1 << 79);

    /// Pascal `TPropertyFlag.PDElement`: a `DSSObjectReference[Array]Property`
    /// whose unqualified reference resolves against any PD element; the schema
    /// labels it `PDElement` instead of `CktElement` (`CAPI_Schema.pas:824/895`).
    /// Schema metadata only.
    pub const PD_ELEMENT: Self = Self(1 << 80);
    /// Pascal `TPropertyFlag.PowerFactorLimits`: a double constrained to the
    /// `[-1, 1]` power-factor band; the schema emits `minimum:-1`/`maximum:1`
    /// (`CAPI_Schema.pas:739-742`). Schema metadata only.
    pub const POWER_FACTOR_LIMITS: Self = Self(1 << 81);
    /// **Not a Pascal flag** — the port's marker for a Pascal
    /// `BooleanActionProperty` (e.g. LineCode `Kron`): a boolean that, when set,
    /// triggers an action and reads back false. The port models it as
    /// [`PropType::Boolean`](crate::obj::props::PropType); this flag restores the
    /// two `BooleanActionProperty`-specific behaviors the merge dropped — the JSON
    /// schema's `writeOnly:true` and the alternate-order placement at the end
    /// (`zorderNextEnd`, with the other action props, `DSSClass.pas:1972-1976`).
    pub const BOOLEAN_ACTION: Self = Self(1 << 82);

    /// Pascal `SuppressJSON` set **after** `inherited DefineProperties` has already
    /// built `AltPropertyOrder` (the inherited PD-tail `NormAmps`/`EmergAmps` on
    /// Transformer/Fault, `Transformer.pas:605-606`/`Fault.pas:215-216`). Unlike a
    /// [`Self::SUPPRESS_JSON`] set in the class body (before the base
    /// `DefineProperties`), such a property is **present in `AltPropertyOrder`** —
    /// it occupies a `$dssPropertyOrder` slot and shifts the following props' order
    /// — but is still excluded from the JSON/schema *output* like a normal
    /// `SuppressJSON`. Distinguished from `SUPPRESS_JSON` so
    /// [`ClassProps::alt_property_order`](crate::obj::props::ClassProps) keeps it
    /// while the JSON dump ([`report::export::json`](crate::report::export::json))
    /// and the schema walk skip it. Inert on the text dump / setters.
    pub const SUPPRESS_JSON_LATE: Self = Self(1 << 83);

    /// **Not a Pascal flag** — the port's marker for a Pascal `SilentReadOnly`
    /// property that still has a real `PropertyOffset` (so it is NOT function-only
    /// like [`Self::SILENT_READ_ONLY`]). Pascal renders such a property `readOnly`
    /// in the JSON schema and elides its default, yet still returns its stored
    /// value on the `?`/props surfaces (`PropertyOffset[Index] <> -1`) and includes
    /// it in the JSON *export* — i.e. exactly the opposite of `SILENT_READ_ONLY`'s
    /// `''`/omit behaviour on those surfaces. Schema-only: consumed by the schema
    /// walk's `readOnly` bit; inert on the text dump / setters / JSON export.
    /// Carried by StorageController `kWNeed` (`StorageController.pas:426`,
    /// `[SilentReadOnly]` with `PropertyOffset = @kWNeeded`).
    pub const READ_ONLY: Self = Self(1 << 84);

    /// **Not a Pascal flag** — the port's marker for an **EPRI r4133 upstream
    /// stub**: a property r4133 still *registers* (so it occupies a display slot
    /// and `AllPropertyNames` reports it) whose write path only stores the raw
    /// parse string and, at most, logs a soft `DoSimpleMsg`. There is no engine
    /// field behind it, no `RecalcElementData`, no Y effect — reading it back
    /// echoes whatever was last written (or the `InitPropertyValues` default).
    ///
    /// The parse arm lives once, in
    /// [`ClassProps::parse_into`](super::ClassProps): store through
    /// [`DssObject::set_string`](crate::obj::base::DssObject::set_string), then
    /// emit [`PropDef::stub_message`](super::PropDef::stub_message) when the row
    /// carries one. It is **never** a parse error, which is exactly why
    /// [`Self::NOT_PORTED`] does not fit (that one hard-errors the write and
    /// hides the row from JSON) — the value must round-trip.
    ///
    /// **Every carrier must be a [`PropType::String`](super::PropType) row**, and
    /// `ClassProps::new` `debug_assert`s it: the write stores a string while
    /// `get_value` renders by `ptype`, so any other type would echo a live field
    /// the write never touched (or hit the class's `get_f64` `unreachable!`).
    /// The flag is reusable on any class, not on any type.
    ///
    /// Carriers (pinned by
    /// `exec::tests::compat_quirks::upstream_stub_rows_are_the_measured_set`):
    /// Generator `Rneut`/`Xneut`, registered with the help text "Removed due to
    /// causing confusion - Add neutral impedance externally", answered with
    /// messages 5611/5612, default `'0'`, and no neutral stamping behind them.
    /// Sensor `Action`, help "NOT IMPLEMENTED", an empty setter, default `''`.
    ///
    /// Orthogonal to [`Self::HIDE_R4133`], which all three rows also carry: this
    /// flag says *how the write behaves*, that one says *which 0.14.5-pinned
    /// enumeration surfaces skip the row*.
    pub const UPSTREAM_STUB: Self = Self(1 << 85);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any flag of `other` is set.
    pub fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    /// Whether the property is excluded from the JSON/schema **output** — a
    /// [`Self::SUPPRESS_JSON`] (excluded from `AltPropertyOrder` too) or a
    /// [`Self::SUPPRESS_JSON_LATE`] (kept in `AltPropertyOrder`, output-suppressed
    /// only). The JSON dump and the schema walk gate on this; the order build
    /// gates on `SUPPRESS_JSON` alone.
    pub fn suppresses_json_output(self) -> bool {
        self.contains(Self::SUPPRESS_JSON) || self.contains(Self::SUPPRESS_JSON_LATE)
    }

    /// Whether the property is deferred from the *full-enumeration* 0.14.5-pinned
    /// surfaces (Dump / `Dump commands` / JSON): either a 0.15.x-deferred
    /// ([`Self::HIDE_015X`]) or an r4133-only ([`Self::HIDE_R4133`]) property. The `?` query
    /// and props-table surfaces still expose these.
    pub fn hidden_from_full_enum(self) -> bool {
        self.contains(Self::HIDE_015X) || self.contains(Self::HIDE_R4133)
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
