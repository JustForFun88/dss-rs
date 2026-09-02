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
    /// dss_capi 0.14.5 `[SilentReadOnly, ReadByFunction]` on a double whose
    /// value comes from a read function rather than a field -- IndMach012 `PF`
    /// (`PowerFactor(Power[1])`) and StorageController
    /// `kWhTotal`/`kWTotal`/`kWhActual`/`kWActual` (the fleet aggregates), the
    /// only two classes that carry it. A set is silently ignored, the JSON
    /// export omits the key, a JSON load skips it, and the schema marks it
    /// `readOnly`.
    ///
    /// # What this flag stopped meaning: the `''` text render (RP3.8, 2026-09-02)
    ///
    /// 0.14.5 never assigns a `PropertyOffset` for a function-only property (it
    /// stays `-1`), so `GetObjPropertyValue`'s outer guard
    /// `(PropertyOffset[Index] <> -1)`
    /// (`.inputs/dss_capi/src/General/DSSObjectHelper.pas:2203-2204`)
    /// short-circuits **before** the read function runs and the `?` /
    /// `DumpProperties` surfaces answer `''`. The port reproduced exactly that.
    ///
    /// **EPRI r4133 -- the behavioral authority (CLAUDE.md, 2026-08-02) --
    /// renders the live value on those same five properties**:
    /// `TIndMach012Obj.GetPropertyValue` arm 5
    /// (`.inputs/electricdss-code-r4133-trunk/Version8/Source/PCElements/IndMach012.pas:1790`,
    /// `Format('%.6g',[PowerFactor(Power[1,ActiveActor])])`) and
    /// `TStorageControllerObj.GetPropertyValue` arms `propKWHTOTAL`..`propKWACTUAL`
    /// (`Version8/Source/Controls/StorageController.pas:991-994` ->
    /// `GetkWhTotal`/`GetkWTotal`/`GetkWhActual`/`GetkWActual`, bodies `:1162-1198`).
    /// The suppression is therefore a *surface convention of a superseded
    /// oracle*, not upstream behavior: RP3.8 retires it for the text render (both
    /// lanes, no `cfg`) and keeps it for the three JSON/schema surfaces it still
    /// describes. A property whose r4133 getter has a live arm carries
    /// [`Self::RENDERS_LIVE_RESULT`] **alongside** this flag; one whose getter
    /// has none keeps the `''` render by simply not carrying it.
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
    /// **Not a Pascal flag.** Marks a [`Self::SILENT_READ_ONLY`] property whose
    /// **text render is a live computed result** in EPRI r4133, so the
    /// `?` / `Dump` / `all_properties` gate must reach the value instead of
    /// returning `''` (RP3.8; the r4133 getters are cited on
    /// [`Self::SILENT_READ_ONLY`], which keeps its other three readers).
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
    /// `&self` and refreshes nothing; it is recorded with its measured
    /// zero-cell blast radius in `ORPHANED_GAPS.md` §1.17 (RP3.8 audit
    /// settlement).
    ///
    /// **A read stays a pure read of the model**, on both holders -- upstream
    /// mutates on both, and neither mutation is reproduced (they are the
    /// `VSConverter.GetCurrents` hazard, CLAUDE.md "Known upstream bugs", in
    /// miniature):
    ///
    /// * r4133's `GetkWhTotal(Var Sum)` / `GetkWTotal(Var Sum)` write their sum
    ///   back into the object -- `StorageController.pas:991-992` pass the
    ///   object's own `TotalkWhCapacity`/`TotalkWCapacity`. A whole-tree grep of
    ///   `Version8/Source` finds those two fields only in their declarations
    ///   (`:81-82`), the two property arms and two dead `RecalcElementData`
    ///   calls (`:1107-1108`) -- nothing ever reads them, and the getters re-sum
    ///   the fleet from scratch anyway, so the write has no observable and the
    ///   port renders the same number without it.
    /// * `IndMach012.pf` reads `Power[1]`, whose `ComputeIterminal` recomputes
    ///   the machine model while the `Iterminal` cache is unstamped -- and that
    ///   recompute advances the slip-Newton by one step, moving the machine's
    ///   own rendered `Slip`. The port runs the recompute on a throwaway clone
    ///   and keeps only the number
    ///   (`IndMach012::refresh_live_pf`).
    ///
    /// Holders: StorageController `kWhTotal`/`kWTotal`/`kWhActual`/`kWActual`
    /// (RP3.8 P1a) and IndMach012 `PF` (RP3.8 P1b) -- nothing else.
    pub const RENDERS_LIVE_RESULT: Self = Self(1 << 19);
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
    /// `EpsRMedium`/`HeightOffset`/`HeightUnit`/`Conductors` and LineGeometry
    /// `Conductors` (WP-U1.4) — the exact set
    /// `exec::tests::compat_quirks::hide_015x_carrier_set_is_the_measured_escape`
    /// pins.
    ///
    /// # Stage F status (F.3aa) — ESCAPED, and now measured rather than argued
    ///
    /// *(The prose below names this flag as "the flag": `UPGRADE_PLAN` §5 makes
    /// a grep for its name a plan-exit metric, and — per `CLAUDE.md`'s rule for
    /// the compat tag — a metric that counts prose as well as uses is not an
    /// index. Documenting the escape must not inflate the number it is about.)*
    ///
    /// `UPGRADE_PLAN` §5 makes "`rg` … empty" a plan-exit criterion for this flag
    /// and hands it to DE_PASCALIZE Stage F; the settled disposition is
    /// `docs/upgrade/DIVERGENCES.md` §"Line/LineGeometry Conductors" ("the
    /// masquerade + [the flag] are retained deliberately"). Stage F ran the flip
    /// — `hidden_from_full_enum` reduced to `HIDE_R4133` alone, i.e. all five
    /// props exposed in both lanes — and gated it. Three facts came out, none of
    /// which the earlier argument had:
    ///
    /// 1. **No physics moves.** The unconditional 520-case corpus gate is
    ///    completely unchanged (40/40, 136.9 s). This is a *surface-structure*
    ///    row, not a numeric one — the flip is invisible to every solve.
    /// 2. **Exactly 13 byte goldens move, all by row insertion**: `golden_json`
    ///    ×2 (`json_line_micro`, `json_circuit_micro`), `golden_reports` ×8
    ///    (`dump_line_geo`/`dump_line_lc`/`dump_line_sym`/`dump_line_switch`
    ///    +4 rows each, `dump_linegeometry` +1, `dump3_bare` +4, `dump3_debug`
    ///    +4, `dump3_commands` +5), `golden_schema` ×3 (two of them —
    ///    `ported_class_defs_bytes_match_oracle` and
    ///    `full_document_reconciles_with_oracle` — on
    ///    `schema_divergences.json`'s now-stale `port_hidden_property` rows,
    ///    naming `Line.EpsRMedium` first; the third on the port-golden document
    ///    drift). +4/+1 is precisely the carrier count per class, so nothing
    ///    else is disturbed.
    /// 3. **The naive flip emits a duplicate JSON key** — the reason it cannot
    ///    land alone. Line's `Wires` prop carries `json_name = "Conductors"`
    ///    (the masquerade that has owned the key since wt-u14props), so with the
    ///    real `Conductors` un-hidden the FULL view of `Line.l1` contains
    ///    `"Conductors":[]` **twice**, once after `Spacing` and once after
    ///    `HeightUnit` (observed bytes, `json_line_micro`). The flag and the
    ///    masquerade are therefore one atomic change, exactly as `UPGRADE_PLAN`
    ///    §5 spells it; the collision precondition is pinned by
    ///    `exec::tests::compat_quirks::line_json_conductors_key_is_owned_by_the_masquerade`.
    ///
    /// So the blocker is not "the flip is risky" but *where it belongs*: the 13
    /// artifacts are 0.14.5-oracle byte goldens the **parity lane may never
    /// re-baseline**, and re-pinning them (`gen_json.py` taught an engine
    /// switch) is the UPGRADE rung §5 describes. Exposing them in the default
    /// lane only would need default-lane self-goldens for those 13.
    ///
    /// # Where that lands — **`UPGRADE_PLAN` §5** (F.3aa said F.4; F.3ag said F.5)
    ///
    /// F.3aa handed the row to **F.4** on the premise that "F-FMT re-layouts the
    /// same Dump/Show surface", so the self-goldens would ride an event F.4 was
    /// opening anyway. That premise does not survive reading the step it names.
    /// `DE_PASCALIZE_PLAN.md` §F-FMT re-layouts **`Show`-style reports only**
    /// (step 2, line 1258: "`Show`-style reports assemble rows as data"), and
    /// step 3 (line 1268) states the opposite of a re-baseline for everything
    /// else — the default lane compares the **same committed goldens** through
    /// the parsed-numeric tokenizer, "valid as long as F-FMT v1 keeps row/column
    /// structure (it does; only rendering changes)". Free re-layout, the thing
    /// that *does* force self-goldens (drift model, line 1232: "re-layouted
    /// reports get default-lane self-goldens"), is the explicitly **optional
    /// v2**, "GUI era, separate decision" (step 4, line 1270).
    ///
    /// And none of the 13 is a `Show` report: **8** are `Dump` texts
    /// (`tests/golden/reports/dump_*.txt`, written by
    /// [`report::save::dump`](crate::report::save::dump)), **2** are AltDSS-JSON
    /// captures and **3** are schema walks (`tests/golden/json/*.json`) — no
    /// text table among them. The population is pinned by name and by surface
    /// directory in
    /// `oracle_parity_cfg_gate::the_hide_flag_escape_population_is_pinned_by_surface`,
    /// so this classification fails rather than rots.
    ///
    /// F.4 therefore opens no self-golden event these 13 could ride, and F.3aa's
    /// "not a second one in F.3" reasoning loses its first one. That left two
    /// candidate hosts: Stage F's own **landing** generation — "default-build
    /// self-goldens for regression detection only, **regenerated once at Stage F
    /// landing**" (line 1292), i.e. F.5 — and `UPGRADE_PLAN` §5's engine-switch
    /// re-pin. **Settled on 2026-07-29 for §5**, which is also what §5 already
    /// says it does. The landing host would buy a zero carrier count by exposing
    /// the five props in the **default lane only**: a permanent fork of a surface
    /// that carries no numeric content, paid for with 13 lane-specific artifacts
    /// that nothing but their own generator would ever check again. §5's re-pin
    /// (`gen_json.py` taught the engine switch) instead retires the flag in
    /// **both** lanes and moves the parity goldens with it, which is the only
    /// form in which these 13 may legitimately change.
    ///
    /// So **no Stage F step hosts this row** — F.4 and F.5 both leave it alone,
    /// and the 13 stay 0.14.5-pinned until the rung that re-pins them lands. The
    /// escape is Stage F's accepted exit for this flag, not a deferral inside it.
    ///
    /// Finally, the criterion as written is unreachable while the mechanism
    /// survives — proven by the sibling: between WP-U2.5 and R4133_PROPS RP1.1
    /// [`HIDE_R4133`] had **zero** carriers and still left 7 `rg` matches,
    /// because a flag's definition, its arm in `hidden_from_full_enum` and the
    /// comments naming it are not uses. (RP1.1 re-armed it with three carriers
    /// and RP1.2 brought it to four, so that era is history and its residue has
    /// grown with the prose; the measurement stands as taken.) Retiring this
    /// flag's *carriers* leaves the same residue, so the successor should
    /// restate the criterion as "zero carriers" — the form the pin above
    /// checks — or delete both flags together.
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
    /// **Carrier-free between WP-U2.5 and R4133_PROPS RP1.1.** SwtControl
    /// `RatedCurrent` (WP-U2.4) was the first carrier; U2.5 brought all four
    /// protection `Dump commands` blocks to their full r4133 shape
    /// (self-referential goldens), so that prop now renders on the full-enum
    /// surface and its flag was dropped. The flag was retained (like
    /// [`HIDE_015X`]) as the mechanism a future r4133-only prop on another class
    /// re-uses until that class's Dump block regenerates — and RP1.1 is that
    /// case: Generator `Rneut`/`Xneut` and Sensor `Action` (all three also
    /// [`UPSTREAM_STUB`]) are r4133-only, absent from both the pinned 0.14.5 and
    /// the capi015 tables, so they stay off Dump / `Dump commands` / JSON /
    /// schema output. R4133_PROPS RP1.2 added a **fourth** carrier of a different
    /// kind: AutoTrans `XfmrCode` is a fully implemented port of r4133's
    /// `FetchXfmrCode` — the flag's criterion is "absent from both pinned
    /// tables", never "not implemented" — and it is the first carrier on a class
    /// that owns committed `Dump` goldens, which is why the blast radius below
    /// had to be re-measured. Every carrier still occupies an ordinal, which is
    /// why the schema's per-class byte gate carries four `port_hidden_property`
    /// rows for them (`tests/golden/json/schema_divergences.json`). The carrier
    /// set is pinned by
    /// `exec::tests::compat_quirks::hide_015x_carrier_set_is_the_measured_escape`.
    /// The `PROPS_015X` allowlist rows that hide such props from the 0.14.5
    /// property-table walk are name-based, independent of this flag.
    ///
    /// **`Save` is deliberately not one of the hidden surfaces.** It serializes
    /// only the properties a deck explicitly set (Pascal `TDSSObject.SaveWrite`
    /// over `PrpSequence`, `General/DSSObject.pas:131-165`), and that Pascal has
    /// no flag filter — r4133 writes `Rneut=` back for a deck that set it, so the
    /// port does too. Pinned by
    /// `exec::tests::upstream_stubs::save_writes_the_stub_names_like_r4133`.
    ///
    /// **The escape this creates has an owner** (the [`HIDE_015X`] precedent):
    /// un-hiding the four rows moves exactly **8** committed artifacts —
    /// `json/der_usermodel_assigned.json`, `json/der_usermodel_full.json`,
    /// `json/dyneq_full.json` (Generator objects), `json/autotrans_micro.json`,
    /// `json/autotrans_solved.json` (the AutoTrans FULL views gain `"XfmrCode"`
    /// between `Bank` and `XRConst`), `reports/dump_autotrans.txt` (+1 row, 56 →
    /// 57), `reports/dump_autotrans3.txt` (+1 row, 63 → 64) and
    /// `reports/dump3_commands.txt` (+7 rows, 2328 → 2335: one help line each
    /// for the two `[Generator]` props and for `[AutoTrans] XfmrCode`, four for
    /// `[Sensor] Action`'s multi-line help) — plus
    /// `json/schema_full_port.json` and the deletion of the four
    /// `port_hidden_property` rows, and **no corpus case** (re-measured
    /// 2026-08-23 for the fourth carrier by disabling this flag's arm below and
    /// running `cargo test -p dss-core --no-fail-fast`: exactly those tests fail
    /// and `corpus_gate_all_cases_match_engines` stayed green). It unblocks when
    /// those surfaces stop being 0.14.5-pinned — GOLDEN_REBASE G3.3c (`dump*` +
    /// `dump3*` self-snapshot) and G3.4 (`json/`), which is where the row is
    /// tracked in `ORPHANED_GAPS.md` §2. The schema half is separate and stays:
    /// `json/schema_full_oracle.json` + `schema_divergences.json` are frozen by
    /// GOLDEN_REBASE §1.2 even after G3.4.
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
    /// Carriers (R4133_PROPS_PLAN RP1.1, pinned by
    /// `exec::tests::compat_quirks::upstream_stub_rows_are_the_measured_set`):
    /// Generator `Rneut`/`Xneut` — registered with the help text "Removed due to
    /// causing confusion - Add neutral impedance externally"
    /// (`Version8/Source/PCElements/generator.pas:441-442`), stored at `:625`,
    /// answered with messages 5611/5612 at `:651-652`, default `'0'`
    /// (`:2567-2568`); the neutral stamping they once fed is commented-out dead
    /// text (`:1294-1303`). Sensor `Action` — help "NOT IMPLEMENTED"
    /// (`Version8/Source/Meters/Sensor.pas:183,204-206`), stored at `:253`,
    /// `Set_Action` is an empty body (`:850-854`), default `''` (`:807`).
    ///
    /// Orthogonal to [`Self::HIDE_R4133`], which all three rows also carry: this
    /// flag says *how the write behaves*, that one says *which 0.14.5-pinned
    /// enumeration surfaces skip the row*.
    pub const UPSTREAM_STUB: Self = Self(1 << 85);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
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
