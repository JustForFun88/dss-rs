//! Property *flag* set ([`PropFlags`]) — the Rust form of Pascal's
//! `TPropertyFlag`.

/// Pascal `TPropertyFlag` set, as a small bitset. Only the flags that affect
/// the script parse/get/text-dump path carry behavior; the rest
/// (`SuppressJSON`, `Redundant`, `RequiredInSpecSet`, `IsFilename`,
/// `GlobalCount`, ...) are recorded for fidelity but are inert in Phase 2 — they
/// only matter to the JSON/alt-order machinery, which is not ported yet.
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
    /// Pascal `SilentReadOnly` on a `ReadByFunction` double (IndMach012 `pf` →
    /// `PowerFactor(Power[1])`): a set is silently ignored, and the **text render
    /// is `""` always** — Pascal never assigns `PropertyOffset` for such a
    /// function-only property (it stays `-1`), so `GetObjPropertyValue`'s outer
    /// guard `PropertyOffset[Index] <> -1` (`DSSObjectHelper.pas` l.2221) short-
    /// circuits before the read function is ever called. Empirically probed
    /// against the pinned oracle: `? indmach012.m1.pf` returns `''` on a **solved**
    /// circuit too, not just pre-solve. (The live pf is still available as a
    /// dynamics state variable, computed where the solution exists.)
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
    /// Pascal `ReplaceZero` (`DSSObjectHelper.pas:2984`) — but adopted here with
    /// **EPRI r4133 `DblValueNZ` semantics** (UPGRADE_PLAN ledger L2). A parsed
    /// double in the open band `(-1e-8, 1e-8)` is replaced by `+1e-8`. EPRI's
    /// `TParser.MakeDoubleNZ` (`ParserDel.pas:912`) does this **unconditionally by
    /// default**; dss_capi 0.15.x instead gates its exact-zero `ReplaceZero` behind
    /// the `PermissiveProperties` compat flag and adds a strict `NonZero` error by
    /// default. We adopt the EPRI default (clamp, no compat flag, no error) — see
    /// `docs/upgrade/DIVERGENCES.md` L2. Carried by the essential-sizing doubles
    /// (Load `kW`/`kVA`, Generator `kW`/`kVA` — but NOT Generator `MVA`, which uses
    /// plain `DblValue*1000` in r4133 — Storage `kW`/`kVA`,
    /// PVSystem `kVA`; WindGen `kW`/`kVA`/`MVA` at U1.8). Applied pre-scale, matching where
    /// `DblValueNZ`/`ReplaceZero` sit in the parse.
    pub const REPLACE_ZERO: Self = Self(1 << 17);
    /// Pascal `AllowNoneItem` (`DSSObjectHelper.ValidateObjectItem`, l.6456): on a
    /// `DSSObjectReferenceArrayProperty` (Line/LineGeometry `Wires`/`CNCables`/
    /// `TSCables`), a list entry of literal `none` resolves to a **NIL slot**
    /// instead of the "object not found" error (SVN r3902/r3913). WP-U1.1 item 3.
    /// Distinct from [`Self::ALLOW_NONE`] (single ref / DoubleVArray). The mixed
    /// conductor-list *numerics* that consume a NIL conductor are WP-U1.4.
    pub const ALLOW_NONE_ITEM: Self = Self(1 << 18);
    // Metadata-only in Phase 2 (inert, kept for fidelity / future phases):
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
    /// [`Self::DYNAMIC_DEFAULT`] this only affects the not-yet-ported JSON
    /// subsystem; it is inert for the text dump and the property setters.
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
    /// silently. Inert for the text dump/setters; kept for fidelity. Carried by
    /// LineCode `FaultRate`/`PctPerm`/`Repair` (WP-U1.4).
    pub const DEPRECATED: Self = Self(1 << 46);
    /// Pascal `Unused`: the property is accepted for backward compatibility but is
    /// not consumed by the engine (`CAPI_Schema.pas:74`). Schema-only, inert here.
    pub const UNUSED: Self = Self(1 << 47);
    /// **Not a Pascal flag.** A 0.15.x-only property deferred from the
    /// *full-enumeration* 0.14.5-gated surfaces — the `Dump` text report
    /// ([`report::save::dump`](crate::report::save::dump)) and the AltDSS JSON
    /// export — because those byte-exact goldens are pinned to 0.14.5 and the class
    /// cannot flip them to capi015 until a sibling WP lands the remaining 0.15.x
    /// props (e.g. Line `Conductors`). The named-query (`?`) and props-table
    /// (`PROPS_015X` allowlist) surfaces still expose the property. Drop the flag
    /// when the class's Dump/JSON goldens regenerate on capi015. Carried by Line
    /// `EpsRMedium`/`HeightOffset`/`HeightUnit` (WP-U1.4).
    pub const HIDE_015X: Self = Self(1 << 48);
    /// **EPRI r4133 `GetTccCurve('none')` semantics** (WP-U2.1, delta D1/E3). On a
    /// single `DSSObjectReferenceProperty` (a TCC_Curve ref), a value of literal
    /// `none` resolves to a **NIL reference silently** — no #401 "not found" — and
    /// the stored name renders as `none`. r4133's `fuse.pas` general
    /// `GetTccCurve(CurveName)` short-circuits `if lowercase(CurveName)='none' then
    /// Exit` before the registry lookup + the unconditional NIL-check #401. Distinct
    /// from the capi015 path (clear **and** #401 — the Rung-1 [`Self::ALLOW_NONE`]
    /// note), which the fuse used pre-U2.1. Carried by Fuse `FuseCurve` only;
    /// Recloser/Relay adopt it at U2.2/U2.3.
    pub const ALLOW_NONE_REF: Self = Self(1 << 49);
    /// **Not a Pascal flag.** An **EPRI r4133-only** property deferred from the
    /// *full-enumeration* 0.14.5-gated surfaces (Dump text report, `Dump
    /// commands` help catalog, AltDSS JSON export). Sibling of [`HIDE_015X`] for
    /// the Rung-2 (r4133) delta: unlike a 0.15.x prop — which a capi015-regenerated
    /// surface *does* expose — an r4133 prop is absent from **both** the pinned
    /// 0.14.5 and the capi015 property tables, so it must stay hidden on every
    /// non-r4133 full-enumeration surface (r4133 render is never byte-gated,
    /// RUNG2-COMMON §"Property renames / additions"). The named-query (`?`) and
    /// props-table surfaces still expose it; the props-table comparison excludes
    /// it via the `PROPS_015X` allowlist row (tests/harness).
    ///
    /// **No live application site as of WP-U2.5.** SwtControl `RatedCurrent`
    /// (WP-U2.4) was the sole carrier; U2.5 brought all four protection `Dump
    /// commands` blocks to their full r4133 shape (self-referential goldens), so
    /// every r4133 prop now renders on the full-enum surface and the flag was
    /// dropped. It is retained (like [`HIDE_015X`]) as the mechanism a future
    /// r4133-only prop on another class re-uses until that class's Dump block
    /// regenerates. The `PROPS_015X` allowlist rows that hide such props from the
    /// 0.14.5 property-table walk are name-based, independent of this flag.
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
    // Pascal `TPropertyFlag.Units_*` (`DSSClass.pas`): each documents a
    // property's physical unit, consumed only by the CAPI JSON-schema export
    // (`CAPI_Schema.pas:extractUnits`, checked in this exact order — the first
    // set flag wins). Inert on the text dump and the property setters, like the
    // pre-existing [`Self::UNITS_HOUR`] / [`Self::UNITS_OHM_PER_LENGTH`]. The two
    // exceptions carried above keep their original bits (38 / 40); the remaining
    // 26 are added here (`OG-1.5c`).
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

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether the property is deferred from the *full-enumeration* 0.14.5-pinned
    /// surfaces (Dump / `Dump commands` / JSON): either a 0.15.x-deferred
    /// ([`HIDE_015X`]) or an r4133-only ([`HIDE_R4133`]) property. The `?` query
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
