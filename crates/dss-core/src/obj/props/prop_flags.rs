//! Property *flag* set ([`PropFlags`]) — the Rust form of Pascal's
//! `TPropertyFlag`.

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
