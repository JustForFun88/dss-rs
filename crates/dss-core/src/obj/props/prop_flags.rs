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
