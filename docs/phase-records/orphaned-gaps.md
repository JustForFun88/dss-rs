# ORPHANED_GAPS rounds (OG-1.1 … OG-1.10)

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### OG-1.4 AltDSS JSON import `Circuit_FromJSON` (orphaned-gaps round, 2026-07-18)

Ported the whole-circuit AltDSS JSON **reader** — the inverse of the JSON export —
on `og14-json-import` (branch based `883a656`). Closes `ORPHANED_GAPS.md` §1.4.

- **What:** `Dss::circuit_from_json(&mut self, json) -> Result<(),String>`
  (`exec/json_import.rs`) — a loop-for-loop port of `Obj_Circuit_FromJSON_` +
  `loadClassFromJSON` + `busFromJSON` (`CAPI_Obj.pas:2674-2983`), wrapped like
  `Circuit_FromJSON` (`CAPI_Circuit.pas`). Clear → DefaultBaseFreq → MakeNewCircuit
  → PreCommands → per-class load (`PASCAL_CLASS_ORDER`) → ReprocessBusDefs → Bus
  coords → PostCommands. Property application is `ClassProps::fill_from_json` /
  `set_json_value` (`obj/props/class_props/json_set.rs`), the port of
  `FillObjFromJSON` / `SetObjPropertyJSONValue`: walks the new **`AltPropertyOrder`**
  (`class_props/mod.rs`, driven by two new flags `ORDERING_FIRST`/`ORDERING_LAST`
  on LoadShape.MemoryMapping / Line.Switch / Transformer.XfmrCode / Load.PF),
  redirects the `array_alternative` (singular per-winding key → plural array),
  drives the typed struct setters for `ON_ARRAY` scalars, and renders every other
  type to the string its existing `edit_property` parse path reads (a scalar
  double round-trips bit-exactly via `f64::from_str`). A hand-rolled `parse_json`
  (`report/export/json/read.rs`) is the text→`Json` front (no serde_json, same
  reason as the writers).
- **Why the re-export differs from J0:** the imported set-order becomes
  `AltPropertyOrder` (not the original deck order), so `export(J0) != export(import(J0))`
  in general — but the oracle round trip is **idempotent after one cycle**
  (J1 == J2). The test therefore imports the oracle's J0 and requires the re-export
  to equal the oracle's own re-export **J1 byte-for-byte** (oracle `Circuit_FromJSON`
  is reachable on the pin), plus idempotency. Goldens `tests/golden/json_import/`
  (`rt_micro` / `rt_transformer` / `rt_ieee13`), generator `tools/golden/gen_json_import.py`,
  driver `tests/golden_json_import.rs` (+ negative tests: malformed/non-object/
  unknown-class/unknown-prop/missing-Name).
- **Bugs found + fixed in-scope (all gate-green):**
  1. **Transformer constructor `SetAsNextSeq(XHL)`** (`Transformer.pas:848`) was
     not reproduced — so a JSON-imported `X12` (or any never-edited transformer)
     rendered its impedance at the wrong set-order position. Added; the low-seq
     redundant `XHL` defers to canonical `X12`, so `X12` renders first. Safe for
     existing goldens (an explicit `xhl=` overwrites the seq).
  2. **Transformer `set_struct_f64_array`/`set_struct_i32_array`** only handled the
     plural array props (kVs/kVAs/…); extended to the `ON_ARRAY` per-winding
     scalars (RNeut/XNeut/MaxTap/MinTap/RDCOhms/NumTaps) for JSON import.
     `RDCOhms` deliberately leaves `RdcSpecified` to the side effect (marks only
     the *active/last* winding), so winding-1 Rdc is derived at recalc — matching
     the oracle round trip 1:1.
  3. **`add_object` split** into `create_object_no_edit` + `edit_active` so JSON
     import creates an element without the empty pre-fill recalc (a RegControl's
     `RecalcElementData` errors "transformer not set" if run before `FillObjFromJSON`
     applies the ref). Pascal `obj_NewFromClass` does the same.
  4. **`Set` command gaps** the export's PostCommands emit but the executive
     lacked: `%mean`/`%stddev` (default daily shape Set_Mean/Set_StdDev) and
     `genmult` (GenMultiplier). Ported (`set_cmd.rs`, `tables.rs`, LoadShape
     `set_mean`/`set_std_dev`).
- **Deferred (recorded):** the `DynInit` tail of `FillObjFromJSON` (ORPHANED_GAPS
  §1.2, Generator/PVSystem/Storage `DynamicExp` init) — mirrors the export-side
  `DynInit` deferral; not exercised by any covered deck. The public wrapper takes
  no `joptions` (only the import-internal `DSSJSONOptions.Edit` bit matters and it
  is applied internally).
- **Gate:** fmt + clippy clean; `cargo test --workspace` green (all 27 live-corpus
  cases match the oracle — the transformer/add_object changes cause no divergence).
  NOTE: the worktree lacks the Oddie `tools/opendss/.venv` junction (4 corpus cases
  need it); run with `DSS_OPENDSS_PYTHON` pointing at main's venv, else those cases
  abort on setup (environment, not a code failure).

### OG-1.4 AltDSS JSON import — settle/fix round (2026-07-18)

Settled two independent read-only audits of `og14-json-import`. Fixes (all
oracle-probed, gate-green):

- **`Required`-property validation (major, AUDIT-CODE):** ported the missing
  `FillObjFromJSON` branch (`DSSObjectHelper.pas:4955`) — a missing `[Required]`
  key now raises `JSON/<cls>/<name>: required property not provided: "<prop>"` and
  aborts the load instead of silently importing an incomplete element. Added the
  `PropFlags::REQUIRED` bit (absent before — only `RequiredInSpecSet` existed) and
  the check in `class_props/json_set.rs`, then flagged the ~38 **non-redundant**
  Pascal-`Required` props across 28 element tables (bus1/bus2, kV, per-winding
  Bus, MonitoredObj/Element/transformer/capacitor refs, Sensor element+kvbase,
  DynamicExp Expression, XfmrCode kV, VSource bus1+basekV). Redundant twins
  (`buses`/`kVs`) are dropped from `AltPropertyOrder`, so only the exported keys
  are checked — round-trip goldens stay green. Oracle-confirmed the exact message
  on dss-python 0.15.7.
- **Edited default DSS_OBJECT re-exported (major, AUDIT-TESTS):** `fill_active_from_json`
  called `set_default_and_unedited(false)`, but Pascal `FillObjFromJSON` never
  `BeginEdit`s (only `EndEdit`), so it never clears `DefaultAndUnedited`. Removed
  the clear — a JSON-imported default (e.g. `spectrum.defaultload`) now stays
  flagged and is dropped from the re-export, matching the oracle (whose own round
  trip is lossy for edited defaults: J0 2328 B → J1 1458 B). New golden
  `rt_edited_default` pins it.
- **`busFromJSON` kVLN+kVLL conflict now aborts (minor, AUDIT-CODE/TESTS):**
  `bus_from_json` returns `Result`; the conflict propagates as `Err` (oracle
  aborts the whole load, error 20230919) instead of logging-and-continuing.
- **Test coverage (AUDIT-TESTS):** restored the two dropped whole-circuit decks as
  import goldens (`rt_positive_seq` = allowduplicates + cktmodel=positive;
  `rt_edited_default`) and added `rt_generator` (Thevenin-DER). Tightened the
  `unknown_class`/`missing_name` negatives to assert the positive/abort outcome,
  and added `missing_required_property_errors` + `bus_kvln_kvll_conflict_aborts`.
- **Rejected — DuplicatesAllowed (AUDIT-CODE, disproven):** `create_object_no_edit`
  already honors it (`command.rs:987`, gated on `!duplicates_allowed`, not
  unconditional as the audit read). Probe: oracle round trip of two duplicate
  `load.l1` under `AllowDuplicates` → Rust import re-export **byte-identical** to
  the oracle J1 (2 loads each).
- **Deferred (recorded follow-up) — numeric-array length validation (minor,
  AUDIT-CODE):** Pascal `SetObjPropertyJSONValue` rejects wrong-length int/double/
  complex/sym-matrix arrays (`Expected an array of %d …`); the Rust renders to a
  string and reparses without the `Norder` count check. Malformed-hand-authored-
  input only (round-trip exports are always correct length). Left for a follow-up;
  needs the per-type expected-count machinery in `set_json_value`.

Last updated: 2026-07-17 (late evening) — **STATUS RESTRUCTURED + PLAN-COMPLETION AUDIT.** All 16 plan docs were re-verified against the codebase; the records of the *completed* plans (FINAL ACCEPTANCE, JSON export, DIAKOPTICS Part I, UPGRADE Rung 1+2) moved to the new **§1a archive**, and every item those plans handed to a still-unfinished successor is now explicit in **§Standing open follow-ups**. Prior same-day — **TEST-TRIAGE ROUND MERGED** (user-ordered
backlog burn-down; six parallel worktree WPs, each gate-green + audited/verified,
merged wt-t1→t2→t3→t4→t6; DE_PASCALIZE is PAUSED by user order after wave 1 —
wave-2 WIP salvaged to origin as `wt-p5a`/`wt-p1b`/`wt-p1213`, R1 not started).
Records: `docs/phase-records/test-triage-{promotions,ad-classify,monitor-windings,indmach,infra-audit}.md`.
- **Stale-skip promotions (wt-t1).** Kundur2Area NCIM → `solvable_now`
  (`oracle:"capi015"`, kind large; §1.7 two-process bit-identical; Rust matches
  capi015 to 0 on all 33 nodes; warm 1 iter == oracle). **IEEE118Bus NOT
  promoted** — the earlier "port converges" read was a stalled iterate
  (`is_solved=false`); Rust reproduces capi015's NCIM non-convergence
  **byte-identically** (both stall at identical voltages, 100 iters) while EPRI
  r4088/r4133 converge (2 iters) via their newer NCIM PV→PQ switching cadence —
  the documented report-only DIVERGENCES item; re-tagged
  `ncim_pv_pq_switching_divergence` (future rung adopts the cadence). WindGen
  GFLDaily ×2: #263 block was stale (WindGen ported) but both decks are
  multi-step → blocked by the capi015 per-step re-nominalization limit;
  re-noted honestly. `solvable_now` 292→293; lock regenerated.
- **AD classify round (wt-t2).** The 9 `off:unclassified-new-deck` entries in
  `ad_sweep.json` got measured verdicts via DSS_AD_CLASSIFY/DECOMPOSE: 3 → `pf`
  (both 8500-Node masters + GFM twin; gaps ~7e-6 ≪ tier), 6 → measured `off:`
  reasons (indmachtest `ad-floor-above-tier` leg2=2.469e-3; StevensonPflow-3ph
  `ad-switched-divergence` mesh; StevensonPflow `non-3ph-cut-only` pos-seq;
  Kersting4wire ×2 `ad-singular-zone` panic-probed; IEEE30 `too-small`). Bucket
  emptied for ad_sweep (family manifests keep their separate follow-up round).
- **Monitor modes 8/10/12 — PORTED (wt-t3).** The deferred TakeSample bodies
  (winding currents / winding voltages / line-line V, Pascal `Monitor.pas`
  1:1) — the panic path is gone. New pinned golden `monitor_windings` (0.14.5
  oracle via `gen_metering_monitors.py`), unit pins in `exec/tests/monitors.rs`;
  audit settlement TIGHTENED the unit close() band 1e-3→1e-6+1e-7·|x| (measured
  floor ≤4.7e-7). Mode-12 upstream terminal-currents UB documented in
  `investigations/monitor_mode12_terminal_currents_ub.md` (local, gitignored).
- **InductionMachine r4133 — VERDICT: INTENTIONAL breaking change (wt-t4,
  fable-xhigh investigation, adversarially CONFIRMED).** The r4133 fuse
  overhaul (CurveMultiplier is the TCC divisor; RatedCurrent demoted to
  nameplate, NO legacy fallback; default curve none) is announced in the 11.0
  release notes, executed coherently across all four protection classes + COM
  API. Physics: Fuse.f2 carries ~53.5 A ≈ 0.82× of the 65 A link (holds, sound
  transformer-primary fusing) but 53.5 multiples under divisor 1.0 → blows at
  Sec=0 → island → non-convergence; EPRI shipped their own example un-migrated.
  Port keeps r4133 parity (no DIVERGENCES entry — nothing diverges). **Equivalent
  coverage added:** corpus twins `controls/fuse/indmach_r4133/{indmach_snap,
  indmach_dyn}.dss` (single edit `CurveMultiplier=65`, restores the exact
  r4088-era coordination incl. the full relay/recloser fault sequence),
  `oracle:"r4133"`, §1.7-validated, live-gated in the mandatory gate; originals
  stay parked as SETTLED. Harness: `compare_monitor` gained the **proven
  f32-ULP floor** (+ its angular image) — decomposition-proven (99.25% of
  490k-sample diffs are exactly 1 ulp at 2.8e-11 rel f64; TOLERANCE_NOTES
  §monitor-f32-floor); this is a floor-proof band, not a widening.
- **EPRI DLL crash repros.** All four #303 crash classes bisected to minimal
  repro decks + Delphi-source suspects:
  `investigations/epri_dll_crashes_r4088_r4133.md` (local). Upstream-binary
  bugs; port-side correctness re-affirmed; nothing to fix in the port.
- **Test-infra audit (wt-t6).** TESTING.md + TOLERANCE_NOTES synced to the
  tree; findings + dispositions in
  `docs/phase-records/test-triage-infra-audit.md`.
- Gate after merges: fmt/clippy clean, `cargo +stable test --workspace` exit 0;
  population.lock consistency re-proven by deliberate regen (no diff).

### OG-1.5b `CAPI_Schema` per-enum walk + class-walk groundwork (orphaned-gaps round, 2026-07-18)

Branch `og15b-schema-full`. Continues OG-1.5 (static core) toward the full
`DSS_ExtractSchema(jsonSchema=True)` document. **Delivered, byte-exact:** the
per-enum walk — `prepareEnumJsonSchema` (`CAPI_Schema.pas:111-162`) + the 21-entry
`DSS.Enums` global list ported directly from `DSSClass.pas:1058-1196` (names /
ordinals / alt-names, with the symbolic ordinals resolved from their Pascal enum
declarations, never seeded from the oracle JSON).
- New `report/export/json/schema/enums.rs` (`schema.rs` → `schema/mod.rs`
  directory module). `global_enum_defs()` reproduces all 21 enum `$defs`
  byte-for-byte incl. the AltNames-drop (LengthUnit/SolveMode), AltNamesValid=false
  (CoreType), the `Wye/Delta` connection remap, and the `oneOf` hybrids
  (MonitoredPhase, PlotProfilePhases).
- `gen_schema.py` now captures the 21 enum `$defs` (self-validated by verbatim
  containment in the 592 KB oracle output); `golden_schema.rs`
  `global_enum_defs_bytes_match_oracle` byte-tests them. Gate green.

**Groundwork for the class walk (not yet ported):** `tools/golden/extract_schema_help.py`
extracts the property-help catalog from the dss_capi gettext resource
`dss/messages/properties-en-US.mo` (the source `TDSSClass.GetPropertyHelp` reads,
`DSSClass.pas:2166` — NOT the schema JSON, so faithful, not circular). Validated:
that catalog reproduces **every** schema-emitted description (948 main + the
specset clones) via `<Class>.<proplower>` leaf lookup + the array-alternative name
redirect (Transformer/AutoTrans/XfmrCode Conn→Conns etc.); no class-parent
fallback is ever needed (`DynamicExp.Like` correctly falls to the literal key). The
`AltPropertyOrder`/`$dssPropertyOrder` computation is fully located
(`DSSClass.pas:1830` `nextByZOrder` + `:1934-2010`; `zorderNextStart=-999`,
`zorderNextEnd=999`, `MakeLike=-1000`; `Ordering_First/Last` used by only 4
classes). Defaults are read from the port's live sample object (constructors
compute them, e.g. Capacitor `norm_amps`), and the fpjson float formatter
(`fpjson_float`) + `get_json_value` accessors are reused.

**Still open (the remaining ~bulk of §1.5):** the per-class walk
(`prepareClassJsonSchema`) — 49 class `$defs` + `List`/`Container` triples +
`circuitProperties` refs + ~40 class-local enum defs. The dominant remaining cost
is per-property metadata the port never carried: the `Units_*` `PropFlags` family
(only 2 of ~30 exist; nearly every electrical class needs them), the
`Required`/`Ordering_First`/`Ordering_Last`/`PowerFactorLimits`/`PDElement` flags,
the 24 classes' `SpecSets`, and the local enums' `AltNames`/`JSONName`. This is a
large, self-contained data-entry effort (recorded in ORPHANED_GAPS §1.5). The
`# Incomplete` caveat on `Dss::extract_schema_json` therefore stays until the class
walk lands. No engine behavior changed.

**Settlement (two audits, 2026-07-18).** Both audits confirmed the delivered
enum surface is faithful, byte-exact vs the oracle (zero unexplained diffs, no
loosened tolerance, no golden regenerated from Rust output), and that the scope
shortfall is honestly disclosed, not hidden. Dispositions:
- *DONE-bar completeness not met* (audit-code OG15b-1 / audit-tests F1, F2) —
  **acknowledged, deferred.** The remaining bulk (49-class walk + 4 metadata
  families + the expected-divergence inventory / full-document byte-compare) is a
  large self-contained implementation, not a settlement fix; it stays open above
  and in ORPHANED_GAPS §1.5. The `# Incomplete` caveat is retained deliberately.
  For the *delivered* enum surface there is no divergence inventory because zero
  divergences were observed — all 21 global enum `$defs` byte-match oracle 0.14.5
  (stricter than an inventory: any diff fails `global_enum_defs_bytes_match_oracle`
  at zero tolerance).
- *Enum walk not wired into the runtime `extract_schema_json`* (OG15b-2 / F1) —
  **won't-fix (deliberate), documented.** `global_enum_defs()` is verified
  groundwork; the oracle `$defs` order is 10 static → 21 enums → class triples
  (`CAPI_Schema.pas:1476-1502`), so the enums are spliced by the class walk that
  assembles + byte-gates the full document. Wiring a standalone 10+21-def
  intermediate now would surface an unverified integrated state the class walk
  supersedes immediately; the skeleton stays the skeleton.
- *Stale module doc* (OG15b-3) — **fixed.** `schema/mod.rs` no longer lists the
  per-enum walk as "Deliberately NOT ported"; it now points at the `enums`
  submodule and scopes the deferral to the per-class walk + class-local enums.
- *Help-catalog provenance* (OG15b-4) — **acknowledged, deferred with the class
  walk.** Confirmed empirically: the `.mo` is genuinely absent from the vendored
  source, and `TDSSClass.GetPropertyHelp` → `DSSHelp` → `TMOFile('locale/en_US.mo')`
  (`DSSClass.pas:1052`, `DSSGlobals.pas:724`) reads exactly that gettext catalog,
  so sourcing from the pinned package's `properties-en-US.mo` is the engine's true
  source, not oracle-JSON seeding. `extract_schema_help.py` is committed as an
  unwired one-off; `help.rs` is neither generated-in-tree nor consumed. When the
  class walk consumes it, re-derive from the `.po` at rev 0.14.5 in
  `.inputs/dss_capi_with_git` for vendored provenance and re-scrutinize.
- *Stale golden bookkeeping* (F3) — **fixed.** `gen_schema.py` no longer lists the
  now-ported 21 global enums under `deferred_enum_def_*`; the golden was
  regenerated with the pinned oracle (592679 bytes, deterministic across 2
  processes) — a 24-line deletion of the two contradictory keys only, every
  byte-gated fragment (`global_defs`/`enum_defs`/`circuit_head`) unchanged.

### OG-1.5c `CAPI_Schema` per-class walk — infra + pilot (2026-07-18)

Branch `og15b-schema-full` (continues OG-1.5b). Ported the per-class walk
`prepareClassJsonSchema` (`CAPI_Schema.pas:325-1134`) loop-for-loop as
`report/export/json/schema/classes.rs::class_schema`, exposed via
`Dss::schema_class_def(class_name)`. This is the **infra stage** of a
multi-agent rollout: all shared machinery is built and proven byte-exact on a
6-class **pilot** (the first six `DSSClassList` classes: LineCode, LoadShape,
TShape, PriceShape, XYcurve, GrowthShape); the remaining 43 classes are ported
by sequential class-batch agents, then the full-document splice by an
integration agent.

Machinery built (all shared):
- **`classes.rs`** — the walker: the `PropertyTypeJson` mapping reconstructed from
  the port's merged `PropType`+flags (`pascal_jtype`), `extractUnits`
  (`extract_units`), the scalar/array/matrix/enum/object-ref default extraction
  with the exact Pascal elision rules, `$dssPropertyOrder` from the ported
  `AltPropertyOrder` (`index_of_in`), `$dssLength`/`$dssShape`/`$dssPropertyIndex`,
  the `SpecSets`→`oneOf`+`toRemove`+`RequiredInSpecSet` block (incl. the
  redundant-member abort), and class-local enum `$defs`.
- **`PropFlags`** widened to `u128`; the **`Units_*` family** completed (26 new,
  all 28 now present), plus `PD_ELEMENT`/`POWER_FACTOR_LIMITS` and a port-only
  **`BOOLEAN_ACTION`** marker (restores `BooleanActionProperty`'s `writeOnly` +
  end-ordering, which the `PropType::Boolean` merge dropped — LineCode `Kron`).
- **`spec_sets.rs`** — schema-side port of `TDSSClass.SpecSets`/`SpecSetNames`
  (`RequiredInSpecSet` stays the per-`PropDef` flag, matching Pascal).
- **`enums.rs`** — the enum rendering factored into a shared `render_enum` +
  `enum_json_name` + `is_global_enum_json_name`, reused for class-local enums;
  `EnumMeta` builds them from the port's `DssEnum` (+ an `enum_overrides` hook for
  the AltName-dropping / renumbering / integer locals the batches need).
- **help** reused from `report::help_catalog::dss_help` (`<Class>.<proplower>`) —
  no duplicate table.
- **`gen_schema.py`** captures each ported class's oracle fragment verbatim
  (`SCHEMA_CLASSES`, `slice_class_def`); **`golden_schema.rs`
  ::ported_class_defs_bytes_match_oracle** byte-compares Rust vs oracle **after**
  applying the committed **expected-divergence inventory**
  (`tests/golden/json/schema_divergences.json`) with **fail-on-stale** (a
  divergence whose occurrence count no longer matches fails; canary-verified).

Pilot result: 5/6 classes byte-exact; LineCode byte-exact **after** its 3
documented r4133 divergences (FaultRate/PctPerm/Repair `deprecated:true` — the
port follows dss_capi 0.15.x/r4133 per WP-U1.4; the pinned 0.14.5 oracle omits
the flag). Two genuine port-metadata fixes made along the way: LineCode `NPhases`
lost a spurious `NonNegative|NonZero` (Pascal has none — it validates in the
`Set_NumPhases` side effect; the flag both over-rejected `nphases<=0` at parse
and leaked `exclusiveMinimum:0` into the schema), and the shape classes' file
props gained the `NPts` `size_prop` (`getSizePropertyIndex`'s `GlobalCount`
branch → `$dssLength`). `extract_schema_json` still returns the skeleton (its
`# Incomplete` caveat stands until the integration splice). Gate green
(fmt/clippy/test); `tests/corpus` pristine. Pattern doc for the batch agents in
the coordinator's scratchpad (`schema_pattern.md`).

#### OG-1.5c batch B2 — sources + Load byte-exact; XfmrCode/Line deferred (2026-07-18)

Delivered **Vsource, Isource, VCCS, Load** byte-exact vs the pinned 0.14.5
oracle (added to `SCHEMA_CLASSES`; `ported_class_defs_bytes_match_oracle`
covers them). Zero divergence-inventory entries. Work was per-class metadata
(the `Units_*`/`NonNegative`/`NoDefault`/`DynamicDefault`/`RequiredInSpecSet`/
`PowerFactorLimits` flags from each `DefineProperties`) plus `spec_sets.rs`
entries for Vsource (4 `oneOf`; the 5th `R0,X0,R1,X1` set auto-aborts on the
Redundant members) and Load (5 `oneOf`).

Two **shared walker-infra gaps** the pilot never exercised were fixed in
`classes.rs` (both needed by every remaining batch):
- **scalar `Bus` default** — the walker read `get_string(propIndex)` for a
  `#/$defs/BusConnection` prop; a Bus value is the terminal's bus name
  (`class_props/json.rs:101`, `obj.GetBus(PropertyOffset)`), so it now reads
  `get_bus_name(pd.size_prop)` (matches Pascal; unset ⇒ elided).
- **scalar `DSSObjectReferenceProperty`** — the enum/object-ref chain handled
  mapped enums but not scalar object refs, so `LineCode`/`Yearly`/`Spectrum`/
  `BP1`… emitted no `type`. Added the `:818-895` arm: `type:string`
  (+`minLength`/`maxLength` for a fixed-class ref, `object_class != Some("")`),
  default = the referenced object's name when set (e.g. `Spectrum:"default"`).

`enum_overrides` gained two arms (the port's runtime `DssEnum` carries no
AltNames/`JSONUseNumbers` — schema-only): **`Connection`** (`Wye`/`Delta`
AltNames, so a sequential `Conn` default renders `"Wye"`) and **`Load: Model`**
(`JSONUseNumbers=true` integer enum + `ConstantPQ`… AltNames, matching
`Load.pas:339`). Note: Load `kW`/`kvar`/`kVA` carry **no** `Units_*` flag — the
pinned 0.14.5 oracle backend has none; the vendored source added them
post-0.14.5 (`dss_capi 90c572e4` "AltDSS-Schema: More units"), an engine-inert
schema-only change the port has not adopted (kept byte-exact vs the oracle,
zero inventory).

**Deferred: XfmrCode + Line.** Both are winding/conductor struct-array classes
whose winding props (`kV`/`kVA`/`Tap`/`%R`/`Conn` with array-alternative to the
plural `kVs`/…, and `RNeut`/`XNeut`/`MaxTap`/`MinTap`/`NumTaps`/`RDCOhms` as
direct `DoubleOnStructArray`/`IntegerOnStructArray`) render as **per-winding
arrays** with `$dssIterator:"Wdg"`. The port models them as plain scalars, so
the schema needs scalar-on-struct-array render-as-array support + the
`$dssIterator` metadata (`PropertyIteratorPropertyIndex`, `CAPI_Schema.pas:929`)
— shared infra with Transformer/AutoTrans (batches B3/B6). Line additionally
carries r4133 divergences (the `Conductors`/`EpsRMedium`/`HeightOffset` adds,
the `Wires`→`Conductors` JSON rename) needing inventory. Both are removed from
`SCHEMA_CLASSES` (like LineGeometry) pending that struct-array build-out; the
gate stays green.

#### OG-1.5c batch B4 — generation + storage byte-exact; Relay deferred (2026-07-18)

Delivered **Generator, GenDispatcher, Storage, StorageController** byte-exact vs
the pinned 0.14.5 oracle (added to `SCHEMA_CLASSES`). Per-class metadata: the
`Units_*` family (kV/kW/kVA/kvar/kWh/Hz/ToD-hour), `NoDefault`/`DynamicDefault`
on the derived doubles (Generator Maxkvar/Minkvar/kVA/kvar, Storage
kWhStored/AmpLimit), `PowerFactorLimits`+`RequiredInSpecSet` on the PF/kW/kvar
spec-set members, plus `spec_sets.rs` for Generator (`kW,pf`/`kW,kvar`) and
Storage (`kWRated,PF`/`kWRated,kvar`). Enum overrides: `Generator: Model`
(`JSONUseNumbers` integer enum + `ConstantPQ`… AltNames) and the two
`StorageController` mode enums (AltNames drop the `I-Peakshave`→`IPeakshave`
hyphen, `StorageController.pas:292-299`). BaseFreq gained the missing
`DynamicDefault`+`Units_Hz` (`CktElementClass.pas:96`) on all four classes.

**Shared walker/infra fixes** (pilot never exercised these; every remaining
batch benefits):
- **`MappedStringEnumArray` default getter** — the enum-array default read only
  `get_struct_i32_array` (on-struct-array enums, e.g. Transformer `Conns`); a
  plain per-phase `MappedStringEnumArray` (Relay/Fuse `Normal`/`State`) reads via
  `get_enum_array`. Split by `ptype` in `classes.rs`.
- **`DynInit` synthetic property** — `TDynEqPCEClass` classes (Generator, Storage,
  PVSystem) get a trailing `DynInit` (`$ref:DynInitType`, `$dssPropertyOrder =
  maxZorder+1`), `CAPI_Schema.pas:1106-1113`. Detected by the two defining props
  (`DynamicEq`+`DynOut`).
- **`READ_ONLY` PropFlags** (new, schema-only) — a Pascal `SilentReadOnly` prop
  with a *real* `PropertyOffset` (StorageController `kWNeed`, Storage `SafeMode`):
  schema `readOnly`+no-default, yet still returns its value on `?`/props and is
  kept in the JSON export (unlike function-only `SILENT_READ_ONLY` → `''`/omit).
- **`DoubleArray` + IndirectCount** — GenDispatcher `Weights` (Pascal
  `DoubleArrayProperty`) and StorageController `Weights` (`DoubleDArrayProperty`)
  were mis-modeled as `DoubleVArray` (rendered `numberArray`); now `DoubleArray`
  (renders `ArrayOrFilePath` + `$dssLength: GenList`/`ElementList`) with the count
  resolved via `get_i32(<listProp>)` = FListSize/FleetSize. The four
  StorageController fleet-aggregate readbacks became `Double`+`SILENT_READ_ONLY`
  (were `String`); Storage `%Idlingkvar` (Pascal `DeprecatedAndRemoved`) became
  `SUPPRESS_JSON` (skipped from schema + order, exactly as the walker skips a
  DeprecatedAndRemoved ptype).

**Divergence inventory (+2, both Generator):** `units:"kW"` (×2, in both spec
sets) and `units:"kvar"` (×1) — dss_capi `90c572e4` "AltDSS-Schema: More units"
added `Units_kW`/`Units_kvar` to Generator.kW/kvar *after* 0.14.5; the port
follows the vendored source, so the oracle backend omits the unit (same wave as
the existing Capacitor.kvar entry). Storage kW/kVA/kvar `Units_*` predate 0.14.5
(oracle has them) → byte-exact, no inventory.

**Relay deferred** to integration as a **port-authored (r4133) class** (like
LineGeometry/WindGen): the port ports OpenDSS r4133-trunk Relay (75 props —
`PhCurve`/`OC_GndCurve`/`PhPickup` renamed from the 0.14.5 `PhaseCurve`/
`GroundCurve`/`PhaseTrip`, plus `DOC_*`/`SinglePhTrip`/`Lock`/`RatedCurrent`/
`InterruptingRating`/per-phase `Normal`/`State` arrays), diverging **structurally**
from the pinned 0.14.5 oracle (53 props, classic names). It cannot be byte-compared
against 0.14.5 and needs a port-output reference; removed from `SCHEMA_CLASSES`
with a deferral note in `gen_schema.py`.

#### OG-1.5c batch B5 — protection + PV byte-exact; Recloser/SwtControl deferred (2026-07-18)

Delivered **PVSystem, UPFC, UPFCControl, ESPVLControl, IndMach012** byte-exact vs
the pinned 0.14.5 oracle, and **Fuse** byte-exact after 4 documented r4133
divergences (all added to `SCHEMA_CLASSES`).

**Per-class metadata fixes:**
- **BaseFreq** gained the missing `DynamicDefault`+`Units_Hz` (`CktElementClass.pas:97`)
  on all six B5 classes that lacked it (Recloser/Fuse/SwtControl/PVSystem/UPFC/
  UPFCControl/ESPVLControl/IndMach012 — the deferred two keep it too).
- **UPFC**: `PowerFactorLimits` (PF), `Units_Hz` (Frequency), `Units_kvar`
  (kvarLimit) — schema-only flags that were deliberately omitted pre-schema
  (`UPFC.pas:242-257`); `UPFC: Mode` enum override (`JSONUseNumbers` integer enum,
  AltNames `VoltageRegulator`… strip spaces/parens, `UPFC.pas:188-193`).
- **IndMach012**: `Units_kV` on kV, `Required` on kW/kVA (`IndMach012.pas:322-337`).
- **ESPVLControl**: the three `*Weights` were mis-modeled as `DoubleVArray`
  (`numberArray`); now `DoubleArray` + IndirectCount over the matching name-list
  (`ESPVLControl.pas:203-219`) → `ArrayOrFilePath` + `$dssLength: <List>`, count via
  new `get_i32(<List>)` = the list size.
- **PVSystem**: `Units_kV` (kV), `PowerFactorLimits`+`RequiredInSpecSet` (PF),
  `PVSystem: Model` enum override (integer, `ConstantP_PF`/`ConstantY`/`UserModel`),
  the `PF`/`kvar` spec sets (`spec_sets.rs`, `PVsystem.pas:432-439`), an explicit
  `json_name("PTCurve")` for `P-TCurve` (Pascal `PropertyNameJSON`, `PVsystem.pas:430`
  — the default `-`→`__` map gave `P__TCurve`), and `READ_ONLY` (not
  `SILENT_READ_ONLY`) on SafeMode so the schema is `readOnly`+no-default while the
  text/JSON dump still reads the live Yes/No (cf. Storage.SafeMode).
- **Fuse** (byte-exact after divergences): dropped-metadata bugs fixed — `Units_s`
  on Delay and `Deprecated` on Action (both present in 0.14.5 and dss_capi HEAD,
  `dss_capi_with_git` `src/PDElements/fuse.pas`).

**Shared walker fix:** the `MappedStringEnumArray` default was read from the
full fixed-`FUSEMAXDIM` buffer (Fuse/SwtControl rendered 6 `closed` states) —
now truncated to `array_size(propIndex)` (`GetFuseStateSize`), exactly as the
JSON dump already does (`class_props/json.rs:168` `.take(n)`), matching Pascal
`GetIntegers`' `aDim[0]` count (`CAPI_Schema.pas:800`). Pilot/B1-B4 never hit
this (Relay/Fuse/SwtControl were the first per-phase enum-array classes; Relay
deferred).

**Divergence inventory (+4, all Fuse):** the port ports OpenDSS **r4133** Fuse
(WP-U2.1): `FuseCurve` default (`Tlink`→`none`) + help, `RatedCurrent`
repurposed from the TCC divisor (default 1.0) to an informational rating
(default 0.0) + help — two `port_changed_line` (default+description 2-line
blocks, embedded-CRLF to disambiguate the shared `0.0` default) — plus the two
new props `CurveMultiplier` (the new divisor) and `InterruptingRating`, two
`port_extra_property` at ordinals 11/12. Cause = r4133 Version8 `Controls/fuse.pas`.

**Recloser deferred** to integration as a **port-authored (r4133) class** (like
Relay/LineGeometry): the port ports OpenDSS r4133-trunk Recloser (46 props —
`PhaseFast`/`PhaseDelayed`/`GroundFast`/… renamed to `PhFastCurve`/`PhSlowCurve`/
`GndFastCurve`/… with the classic names kept as deprecated aliases, plus
`MechanicalDelay`/`SinglePhTrip`/`Lock`/`Reset`/`RatedCurrent`/`InterruptingRating`),
diverging **structurally** from the pinned 0.14.5 oracle (24 props, classic
names). Needs a port-output reference; removed from `SCHEMA_CLASSES`.

**SwtControl deferred** to integration as a **port-authored (r4133/0.15.x)
class**: the port adopted the 0.15.x property model (D12/WP-U1.6 — Normal→
NormalState, State→PresentState instead of the shared 0.14.5 CurrentAction; the
State `ReadByFunction=GetState` "no controlled element → CTRL_NONE" semantics
are intentionally not modeled in the accessor), plus the OpenDSS r4133
`RatedCurrent` prop and the 0.15.x help catalog (deprecated Delay, per-phase
Normal/State help). Its Normal/State schema **defaults** therefore differ from
the 0.14.5 oracle in getter semantics (not a clean line diff; State renders
`closed` where both oracle versions render `null` via GetState), so it needs a
port-output reference. The inert `Units_s` (Delay) + BaseFreq schema flags were
still corrected in source; the behavioral `Reset` BooleanAction `writeOnly` and
the State getter are left for the integration pin. Removed from `SCHEMA_CLASSES`.

#### OG-1.5c batch B6 — GIC + inverter/converter controls + metering byte-exact (2026-07-18)

Delivered all ten B6 classes byte-exact vs the pinned 0.14.5 oracle:
**GICsource, InvControl, ExpControl, GICLine, GICTransformer, VSConverter,
Monitor, EnergyMeter, Sensor** with zero divergences, and **AutoTrans** byte-exact
after 3 documented r4064 BH-curve divergences (`port_hidden_property`, mirroring
Transformer). All added to `SCHEMA_CLASSES`.

**Per-class metadata fixes:**
- **BaseFreq** gained the missing `DynamicDefault`+`Units_Hz`
  (`CktElementClass.pas:97`) on all ten classes (they lacked it).
- **AutoTrans** (the Transformer struct-array sibling): wired the per-winding
  `array_alternative` redirects (kV/kVA/Tap/%R/Bus/Conn → their plurals) +
  `REDUNDANT` plurals, `ON_ARRAY` on the no-plural scalars (RDCOhms/MaxTap/MinTap/
  NumTaps) — with new struct-array getters in `accessors.rs` for those four (the
  schema sweep reads them) — `INTEGER_STRUCT_INDEX` on Wdg, the `XHX,XHT,XXT` /
  `XscArray` spec sets, units (kV/Thermal-hour/FLRise-HSRise-°C/NormHkVA-EmergHkVA-
  kVA), `AutoTrans: Connection` enum override (`Wye`/`Delta`/`Series` AltNames), and
  **removed the erroneous `SUPPRESS_JSON` from NormAmps/EmergAmps** (AutoTrans has
  no such Pascal override, unlike Transformer). (`AutoTrans.pas:320-557`)
- **InvControl**: `RoCEnum.JSONName` override (`InvControlRateOfChangeMode`),
  `InvControl: Control Model` enum → `JSONUseNumbers` integer enum, `Units_s` on
  LPFTau, and `VV_RefReactivePower` (Pascal `DeprecatedAndRemoved`) → `SUPPRESS_JSON`
  (skips schema/JSON export + AltPropertyOrder while the `?` surface keeps '', like
  Storage `%Idlingkvar`). (`InvControl.pas:444-449,525,571`)
- **GICsource/GICLine**: the `Volts,Angle` / `EN,EE,Lat1,Lon1,Lat2,Lon2` spec sets
  + units (deg/Hz/V·km⁻¹/V/Ω/µF); GICLine's Spectrum+BaseFreq are `SuppressJSON`
  after inherited → `SUPPRESS_JSON_LATE` (`GICLine.pas:249-250`). GICLine Angle
  carries no unit (unlike GICsource's).
- **GICTransformer**: `R1,R2` / `pctR1,pctR2` spec sets + units (Ω/kV/MVA).
- **VSConverter**: `Units_ohm` on RAC/XAC. **ExpControl**: `Units_s` on VRegTau.
- **Sensor**: `kWs,kvars` / `currents` spec sets, `Clear` → `BOOLEAN_ACTION`
  (writeOnly + ordering-last), `Units_kV` on kVBase. **Monitor**: Element default.
- **EnergyMeter**: `Option` StringList default (`[E,R,C]`), `3PhaseLosses` →
  `json_name("ThreePhaseLosses")`, SAIFI/SAIFIkW/SAIDI/CAIDI/CustInterrupts →
  `READ_ONLY` (schema `readOnly`+no-default, props keeps stored value).

**Monitor/EnergyMeter Element default = `Vsource.source`:** Pascal `Create`
defaults the metered element to the first circuit element (the auto-source,
`Monitor.pas:482`/`EnergyMeter.pas:959`); the oracle schema runs
`new circuit.defaults` (`CAPI_Schema.pas:1264`) so the sample sees it. The port's
constructors now default `element_full_name` to `"Vsource.source"` (every real
deck overrides it via the Element property before solve; only the all-default
sample's schema default observes it).

**Shared walker fix:** `string_array_default` now handles `PropType::StringList`
(EnergyMeter `Option`/`ZoneList`, InvControl `MonBus`) via `get_string_list` — the
anticipated batch-owned extension (previously returned `None`).

**Divergence inventory (+3, all AutoTrans):** BHPoints/BHCurrent/BHFlux — the
r4064 (dss_capi 90962ae8) GICharm BH-curve props the port carries `SUPPRESS_JSON`
(port-only, absent from 0.14.5), occupying `$dssPropertyIndex` 42-44 and shifting
the following indices +3 (`$dssPropertyOrder` untouched). Three `port_hidden_property`.

#### OG-1.5c integration — full-document splice + XfmrCode/Line + port-authored pins (2026-07-19)

Closed §1.5. `Dss::extract_schema_json` now emits the **whole**
`DSS_ExtractSchema(jsonSchema=True)` document (the `# Incomplete` caveat is gone):
`schema::assemble_full_document` splices the 10 static + 21 enum `$defs`, then per
class in `schema::DSS_CLASS_LIST_ORDER` (Pascal `DSS.DSSClassList`, 49 oracle
classes + WindGen after Generator) the `$defs/<Class>` + `<Class>List`/
`<Class>Container` triple + `circuitProperties` container ref, and the envelope
(`CAPI_Schema.pas:1479-1513`). Byte-verified 181 `$defs` (10+21+50·3).

**XfmrCode + Line finished byte-exact** (the two B2-deferred struct/spec-set
classes; added to `SCHEMA_CLASSES` → 45 byte-gated):
- **XfmrCode** — wired the Transformer winding pattern (array_alternative kV/kVA/
  Tap/%R/Conn→plurals + `REDUNDANT`, `ON_ARRAY` on RNeut/XNeut/Max/MinTap/RDCOhms/
  NumTaps + struct getters, `INTEGER_STRUCT_INDEX` on Wdg), units (kV/Ω/hour/°C/
  kVA), `DYNAMIC_DEFAULT` on NormHkVA/EmergHkVA/XSCArray, the `X12,X13,X23`/`XscArray`
  spec sets. 1 divergence: 3 `exclusiveMinimum` lines (X12/X13/X23 `NonZero` added
  post-0.14.5, git `69fca934`, first in 0.14.6a1).
- **Line** — 5 spec sets (LineCode/LineGeometry/"Spacing, Wires"/Z0Z1C0C1/ZMatrix,
  the B0/B1 set auto-aborts) + `REQUIRED_IN_SPEC_SET` members, missing units
  (rho Ωm, C0/C1/CMatrix nF), RMatrix/XMatrix `NO_DEFAULT`, BaseFreq
  `DynamicDefault`+`Units_Hz`. **Walker fix** (`schema/classes.rs`): the schema now
  skips `hidden_from_full_enum()` (HIDE_015X/HIDE_R4133) props, matching the JSON
  dump (`build.rs`) — the 0.14.5-pinned schema must not emit 0.15.x/r4133 props.
  4 divergences: EpsRMedium/HeightOffset/HeightUnit/Conductors are HIDE_015X (in
  `AltPropertyOrder`), a new `port_hidden_property` variant carrying `order` renumbers
  both `$dssPropertyIndex` and `$dssPropertyOrder` (+4 on the tail).

**Port-authored classes (5, not byte-gated vs 0.14.5)** — pinned to the port's own
bytes and inventoried (`schema_divergences.json::port_authored_classes`, cause-attributed):
LineGeometry (0.15.x Conductors model), Relay/Recloser (r4133 renamed protection
curves), SwtControl (0.15.x per-phase Normal/State getter semantics), WindGen (the
50th class, absent from the 49-class 0.14.5 oracle).

**Verification** (`golden_schema.rs`, 9 tests): `ported_class_defs_bytes_match_oracle`
(45 classes after divergences); `full_document_matches_port_golden`
(`schema_full_port.json`, regression); `full_document_reconciles_with_oracle`
(vs the verbatim `schema_full_oracle.json` — scaffolding byte-identical, each class
region == `schema_class_def`, byte-gated classes == oracle-after-divergences, WindGen
scaffolding == the substituted-name formula); `every_class_is_gated_or_inventoried`
(gated ⊕ port-authored split, fail-on-stale). Both fail-on-stale canaries proven
(falsified divergence count → per-class fails; removed port-authored entry → split
fails). **Divergence inventory total: 30 per-property (LineCode 1, CNData 1,
LineSpacing 5, XfmrCode 1, Line 4, Capacitor 1, Transformer 3, AutoTrans 3,
RegControl 5, Generator 2, Fuse 4) + 5 port-authored classes.**
Full gate green; `tests/corpus` pristine.

**Settle round (2026-07-19, two audits).** Both audits confirmed the class walk is
a faithful `prepareClassJsonSchema` port with no weakening: full-document equality
is real byte equality, goldens are oracle-sourced (`gen_schema.py` pin-check +
two-process determinism), and fail-on-stale is real (exact occurrence counts,
gated ⊕ authored XOR). Three low findings, all settled empirically:
- **Completeness guard (FIXED).** `extract_schema_json` drove the walk only from
  the static `DSS_CLASS_LIST_ORDER`; a future-registered class not added to the
  list would be silently omitted (Pascal walks the *live* `DSS.DSSClassList`).
  Added a registry-coverage assert (`class_defs.len() == self.classes.len()`) at
  the walk site — every listed class is already proven registered, so equal counts
  make it a bijection. Exercised by every `extract_schema_json` call (the skeleton
  + full-document tests). Confirmed 50 registered == 50 listed today.
- **4 port-authored classes get no oracle content byte-gate (accepted).**
  LineGeometry/Relay/Recloser/SwtControl exist in the 0.14.5 oracle but their
  content is pinned only to `schema_full_port.json` (self-referential); WindGen is
  the 50th class, absent from the oracle. Empirically confirmed genuine structural
  divergence, not a lazy escape hatch: property-set deltas LineGeometry ±10, Relay
  +22/−10 (renamed protection curves), Recloser +23. **SwtControl** — the audit's
  specific worry — was verified in detail: beyond the extra `RatedCurrent` prop its
  `$dssPropertyOrder` remaps non-monotonically (Reset 8→10 while others shift by the
  insert), the oracle carries `writeOnly` on Reset that the port lacks, and Lock/
  Delay/Normal/State help+defaults all changed (r4133 per-phase NormalState/
  PresentState model). That order remap is **not expressible** in the per-property
  divergence DSL (`renumber_field` only does a uniform decrement), so byte-gating
  SwtControl would require weakening the DSL — port-authored is the honest call.
  Left as an accepted residual per brief_schema item 3. Follow-up (non-blocking):
  an independent r4133 `DSS_ExtractSchema` oracle (via the opt-in EPRI channel)
  could byte-gate these four; not built (that channel never gates commits).
- **`REGEN_SCHEMA_PORT` env-guard no-op (accepted).** The port-golden test
  rewrites-and-returns when the var is set — identical to the established
  `DSS_REGEN_AD_GOLDEN` pattern in `adiakoptics.rs`, a repo-wide convention, not a
  new weakening; both oracle goldens remain pin-checked. No change.

### OG-1.7 UPFC modes 2/3/5 (orphaned-gaps round, 2026-07-18)

Branch `og17-upfc-modes`. `ORPHANED_GAPS.md` §1.7. **The engine code already
handled all six modes** (0..5) — `elements/pc/upfc/compute.rs`
`get_output_curr`/`get_input_curr`/`check_status`/`calc_upfc_powers` are a
loop-for-loop port of `UPFC.pas` and match the oracle to the printed precision on
modes 2/3/5. The real gap was the **missing test surface**: no corpus deck
exercised modes 2 (StatCOM shunt reactive), 3 (Dual = series V-reg + shunt PF),
or 5 (DoubleRef Dual). Added three live decks under `tests/corpus/controls/upfc/`:
- `upfc_statcom.dss` (mode 2): series path off (Sr0=0), shunt QIdeal ~3.1 kvar
  drives the monitored service-transformer PF to pf=0.95; `element=` mandatory
  (CheckStatus mode 2 = checkPF only); 16 iters.
- `upfc_dual.dss` (mode 3): mode-1 series V-reg to refkV **plus** the synced shunt
  reactive branch (QIdeal ~7570 var); `element=` required or it collapses onto
  mode 1; 25 iters.
- `upfc_doubleref_dual.dss` (mode 5): mode-4 two-band reference (lower band
  engaged, Vbout→refkV2) **plus** the shunt PF branch (QIdeal ~7.8 kvar); 25 iters.

Each GAPS §3-proven (pin solves+converges; two-process bit-identical fingerprint;
feature-sensitive — mode 3 vs mode 1 and mode 5 vs mode 4 share the series Sr0 but
QIdeal→0, isolating exactly the dual shunt branch; mode=0 zeros both). Registered
in the controls `manifest.json` + `CONTROLS_REQUIRED` floor with
`compare_variables:[UPFC.test]` (all 14 state vars) + mode/refkv/pf/element probes;
`controls_cases_match_oracle` green (Rust == pinned dss-python 0.15.7 on full model
+ all UPFC variables + properties). Convergence needs a **reachable** PF target
(pf=0.99 unclamped only with kvarLimit≥20; the kvarLimit clamp otherwise stalls
checkPF at max-control-iterations) and tol1≥0.005 to settle both deadbands — the
non-convergence the pre-existing `upfc_vreg` note warned about. No engine code
changed.

**Settle round (2026-07-18, two audits).** Both audits confirmed the engine port is
faithful (all 6 modes loop-for-loop) with no regressions/simplifications and the
decks feature-sensitive vs the pinned oracle. The single substantive finding (both
audits, minor): the three new decks are all snapshot (`n_steps:1`), so the brief's
"multi-step solve if the mode has temporal state" clause was not literally met even
though UPFC carries genuine cross-step `Sr0/Sr1` shift-register state. **Closed** by
adding a fourth deck `upfc_dual_daily.dss` (mode 3, `n_steps:4`, daily loadshape
0.7/1.0/1.25/0.9): the load ramps each hour so the UPFC re-regulates from the
`Sr0/Sr1` carried over from the prior step. The cross-step channel is proven — at
step1 the dual series deadband does not re-fire so `Sr0` stays at the step0 value
(79.40,-261.23) while mode=1 on the same feeder moves `Sr0` to (111.26,-328.50);
the persisted `Sr0` is exactly the cross-step divergence a snapshot cannot reach.
GAPS §3-proven (4 steps converge 22/16/26/24 iters; two-process bit-identical;
mode 1/0 feature-sensitivity per step); `compare_variables:[UPFC.test]` all 14 vars
+ full model compared PER STEP; `controls_cases_match_oracle` green. Second finding
(untracked `GFM_IEEE8500/IEEE8500u_VLN_Node.txt` solve byproduct) was a pre-declared
unrelated GFM output, not part of this WP's diff — removed from the worktree to keep
`tests/corpus` pristine; nothing to commit there.

### OG-1.1 GICMvars export (orphaned-gaps round, 2026-07-18)

Ported `Export GICMvars` (report verb 36) — the last unported GIC surface
(ORPHANED_GAPS §1.1; GAPS WPG.16's punt to a never-materialized "Phase 9").

- **`WriteVarOutputRecord`** → `GicTransformer::var_output_record`
  (`elements/pd/gic_transformer/solve.rs`): `ComputeIterminal`, sum the per-phase
  terminal currents, `GICperPhase = |ΣI|/nphases`; Mvar on the K-factor path
  (`FKfactor·FkV1·GICperPhase/1000`) or the VarCurve path
  (`GetYValue(pu)·FMVArating/√2`, pu = GICperPhase/(MVA·1000/kV1/√3)).
- **Driver** `ExportGICMvar` → `report/export/gic_mvars.rs::export_gic_mvars`:
  walks the GICTransformer class ElementList (creation order, no Enabled filter);
  header `Bus, Mvar, GIC Amps per phase`; `%.8g` cells. Verb 36 routed in
  `exec/report.rs` (default file `EXP_GIC_Mvar.csv`; not solution-guarded, 1:1 with
  Pascal `ExportOptions.pas`).
- **Test surface:** new golden `export_gicmvars` (generator `gen_gic_mvars` in
  `tools/golden/gen_reports.py`) over the GIC-study deck — all three types
  (GSU/YY K-path + Auto VarCurve path); `golden_reports.rs::export_gicmvars_matches_oracle`
  (rel 1e-7/abs 1e-8, the faer-vs-KLU GIC-current floor). Retired the
  `exec/tests/report.rs` GICMvars NOT_PORTED assert → now pins `Estimation`(5) on a
  solved circuit (all remaining unported verbs are solution-guarded).

### OG-1.2+1.3 AltDSS JSON tails — DynInit + Full WdgCurrents (orphaned-gaps round, 2026-07-18)

Branch `og1213-json-tails`. Closes `ORPHANED_GAPS.md` §1.2 and the WdgCurrents
half of §1.3.

- **§1.2 DynInit tail — PORTED.** `obj_to_json_data` now appends the Pascal
  `TDynEqPCE` `"DynInit"` object (`CAPI_Obj.pas:752-759`) for any object whose
  `UserDynInit` is non-empty (Generator/PVSystem/Storage, reached via a new
  `DssObject::as_dyneq` accessor). `DynEqPceData.user_dyn_init` changed from
  `Vec<(String,String)>` to `Vec<(String,DynInitValue)>` where `DynInitValue` is
  `Number(f64)` | `Text(String)`: `parse_dyn_var` uses `make_double_ex` to
  recover Pascal's `requiredRPN`, so a plain constant → JSON number, a
  calc-value operand or RPN constant → JSON string (raw case). The `"DynInit"`
  key is emitted literally (never lowercased, even under LowercaseKeys — matches
  oracle). Golden `dyneq_micro` (Generator + Storage; default-family combos).
- **§1.3 Full WdgCurrents — PORTED (refresh route).** Added
  `Dss::obj_to_json_mut` / `class_batch_to_json_mut`: they run
  `refresh_vterminal_if_marked` before the `&self` builder, so Full-mode
  `READS_VTERMINAL` result strings (Transformer/AutoTrans `WdgCurrents`) render
  from the current solution exactly as Pascal's self-refreshing getter does
  (pre-solve = all-zero phasor list). Golden driver uses the `_mut` routes.
  `transformer_micro` flipped off `skip_full` → Full WdgCurrents now pinned
  byte-exact. Fixed en route: Transformer/AutoTrans `BHCurrent`/`BHFlux` (newer
  r4064 props absent from the 0.14.5 oracle) leaked into Full JSON as `null` —
  now `SUPPRESS_JSON` like their sibling `BHPoints`.
- **§1.3 Capacitor CMatrix — proven UB, NOT reproduced.** The oracle renders
  Capacitor `CMatrix` under Full from an uninitialized `pDoubleArray` (denormal
  garbage: `2.1e-308`, `4.9e-318` …) that **differs across oracle processes**
  (probed twice), for both kvar- and explicit-`cmatrix`-defined caps. Per the
  UB/state-mutating-read rule it is not reproduced (like the multi-meter OOB
  case); decks with capacitors stay `skip_full`.
- Gate green (fmt + clippy + `cargo test --workspace`). Out-of-scope discoveries
  recorded in Standing follow-ups (AutoTrans JSON array-alt metadata; Generator
  ShaftModel/ShaftData hidden under Full).

**Audit-settle round (2026-07-18).** Two read-only audits returned 6 findings
(1 major, 5 minor); settled empirically against the pinned oracle:
- **[FIXED — major] Full WdgCurrents pinned only pre-solve (a no-op).** The
  pre-solve refresh recomputes zeros (NodeV=0), so `transformer_micro` alone
  could not catch a refresh regression. Added golden **`transformer_solved`**:
  the transformer primary is on the energized `sourcebus` feeding a 500 kW load,
  the deck `solve`s, and Full `WdgCurrents` is now a NONZERO phasor list pinned
  byte-exact (obj + batch × 4 Full combos). Verified Rust==oracle bit-for-bit;
  byte-exact is valid post-solve because the getter formats at `%.7g`/`%.5g`
  (Transformer.pas:362), far coarser than faer-vs-KLU last-ULP. Deleting the
  `refresh_vterminal_if_marked` call now fails the gate.
- **[FIXED — minor] DynInit dedup rewrite unpinned.** `dyneq_micro` now assigns
  `Damp` twice (`= 0` number, then `= (1 2 +)` RPN string): pins the Pascal
  `UserDynInit.Delete`+`Add` reorder — the rewrite changes the value type AND
  moves `damp` to the tail. Oracle-confirmed and reproduced byte-exact.
- **[disproven — minor] AutoTrans "proven byte-exact by analogy".** Overstated;
  softened the `gen_json.py` NOTE to "inference, not verified". The refresh route
  is class-agnostic (now proven post-solve via `transformer_solved`), but a
  standalone AutoTrans byte-golden stays blocked by the plural/singular metadata
  gap (already a follow-up below). Real, deferred — not silently dropped.
- **[not-a-defect — minor] WindGen `as_dyneq` override.** `WindGen.pas` is absent
  from the 0.14.5 pinned source (Rust-only forward-port from a newer engine where
  WindGen IS a `TDynEqPCE`); it embeds a real `dyneq` field, so emitting DynInit
  is internally consistent. Cannot appear in any oracle golden/live compare, so
  untestable and harmless — kept for sibling consistency (Generator/PVSystem/Storage).

**Audit-settle round 2 (2026-07-18, post-merge on `update`).** Two further
read-only audits returned findings; most were already remediated in-branch by
38a5e67 (the pre-solve Full WdgCurrents no-op and the DynInit dedup — both re-flagged
against the `afd8853` HEAD, RESOLVED above). One material gap survived and is fixed:
- **[FIXED — major] AutoTrans `WdgCurrents` getter never verified nonzero.** The
  §1.3 deliverable names *both* Transformer AND AutoTrans WdgCurrents. Transformer
  is pinned nonzero by `transformer_solved`, but AutoTrans has its OWN distinct
  series/common/delta getter (`TAutoTransObj.GetAllWindingCurrents`,
  `auto_trans/yterminal.rs`), and every AutoTrans WdgCurrents golden was pre-solve
  all-zeros — indistinguishable from a broken refresh. Added props scenario
  **`autotrans_solved`** (`gen_props.py`): a solved 3-winding YNad1 auto (Series/
  Common/Delta-tertiary, unit from corpus `autotrans_snap.dss` t1) fed on the series
  winding, loads on the 161 kV common + 13.8 kV tertiary. The `?`-query path hits
  `refresh_vterminal_if_marked` (`command.rs:1107`) → reloads Vterminal → runs the
  auto's own getter. `props_roundtrip` now pins `WdgCurrents` NONZERO
  (`549.4296, (-31.051), …`) vs the pinned 0.15.7 oracle; Rust matches numerically.
  A broken refresh would emit all-zeros and fail. Closes the last un-verified §1.3
  getter. (JSON-Full AutoTrans golden stays blocked by the plural/singular metadata
  follow-up below; the props route needs no JSON metadata and closes the gap.)
- **[deferred — minor] DynInit tail not gated under Full mode.** `dyneq_micro` is
  `skip_full` (blocked by the ShaftModel/ShaftData NOT_PORTED Full-render gap,
  already a follow-up). The DynInit append is sweep-independent code fully exercised
  by the default sweep, so residual risk is low; kept as-is. Recorded, not dropped.
  **→ CLOSED 2026-07-26 (OG-1.3a).** The blocking gap is gone; rather than
  regenerate `dyneq_micro`, the new `dyneq_full` deck pins the tail under Full
  directly (same assignment mix, incl. the `Damp` rewrite reorder).

### OG-1.3a AltDSS JSON §1.3 tails — AutoTrans + DER user-model Full (2026-07-26)

Branch `depas-og`. Closes both `ORPHANED_GAPS.md` §1.3 standing follow-ups (the
AutoTrans array-alternative blocker and the `ShaftModel`/`ShaftData` re-triage),
plus the `dyneq_micro` `skip_full` residual. Four new JSON byte goldens; no
existing golden regenerated (`gen_json.py` grew a deck-name filter so a new deck
cannot drag unrelated oracle drift into the committed bytes).

- **AutoTrans metadata — the follow-up's premise was STALE.** `auto_trans/mod.rs`
  already carries the full singular/plural `array_alternative` + `REDUNDANT` +
  `ON_ARRAY` block; it landed in `430d033` (og15c-B6 schema round), *after* the
  og1213 note was written. Re-verified 1:1 against `AutoTrans.pas:439-557`
  (bus/buses, conn/conns, kV/kVs, kVA/kVAs, tap/taps, pctR/pctRs +
  RDCOhms/MaxTap/MinTap/NumTaps `ON_ARRAY` + `Wdg` struct index) — complete and
  correct. So the real gap was only the missing golden.
- **Golden `autotrans_micro`** (obj + batch × the full 10-combo sweep incl.
  Full). Pins that the DEFAULT sweep renders the SINGULAR `Bus/Conn/kV/kVA/pctR`
  per-winding arrays — losing the metadata flips them to `Buses/Conns/kVs/kVAs`
  and fails. Three windings (series/wye/delta) so the AutoTrans-only `series`
  ordinal is covered; the four `ON_ARRAY` scalars carry DISTINCT per-winding
  values so their positional indexing is pinned.
- **Golden `autotrans_solved`** (Full-family combos). AutoTrans has its OWN
  `TAutoTransObj.GetAllWindingCurrents`; every previous AutoTrans JSON capture
  was pre-solve all-zeros, indistinguishable from a broken getter. Now pinned
  NONZERO byte-exact (`549.4296, (-31.051), …`).
- **[FIXED — real bug found by that golden] `RdcOhms` operator association.**
  Pascal `Rdcpu * SQR(VBase) / VABase` forms the square FIRST; the port had
  left-to-right `rdcpu * vbase * vbase / vabase`, reassociating the product and
  landing ONE ULP off the oracle on the rendered `RDCOhms`
  (`1.4330663434343431E-002` vs `…35E-002`). Fixed in **both**
  `auto_trans/yterminal.rs` (AutoTrans.pas:1021) and `transformer/yterminal.rs`
  (Transformer.pas:1008) — the same trap, same line shape. Reported-value only
  (`rdcpu` itself is unchanged), so no solve behavior moved; whole gate green.
- **ShaftModel/ShaftData re-triage — clean, no divergence.** With the WM.3/WM.4
  NOT_PORTED removal and `hidden_from_full_enum()` no longer consulting
  NOT_PORTED, Full-mode JSON *does* emit them and matches the pinned 0.14.5
  oracle exactly (`""`). **Golden `der_usermodel_full`** pins all six surfaces —
  Generator `UserModel`/`UserData`/`ShaftModel`/`ShaftData`, Storage
  `UserModel`/`UserData` + `DynaDLL`/`DynaData`, PVSystem `UserModel`/`UserData`
  — obj + batch × six Full/FullNames/LowercaseKeys combos. Values left at the
  empty-string default deliberately: the gap was the *render*, not the loader.
- **[FIXED — second real bug found by that golden] `Spectrum=` FullNames render.**
  The port resolves `Spectrum` outside the property machinery, so its `PropDef`
  carried no resolving class and JSON `FullNames` emitted a bare `mycustom`
  where Pascal (`PropertyOffset2 = SpectrumClass`, `PCClass.pas:96-98`) emits
  `Spectrum.mycustom`. Oracle-probed across ALL twelve Spectrum-bearing PC
  classes on the pin — uniform, and visible in the DEFAULT sweep too whenever a
  non-default spectrum is assigned. Fixed render-only: new
  `PropDef::json_ref_class` + `PropDef::object_ref_deferred(class, name)`,
  consulted by `object_full_name` solely on the `object_class == None` arm, with
  all 13 `Spectrum` sites switched over. **No parse/resolution behavior changes**
  (the deferred-resolve path and the lowercased stored name are untouched — the
  golden asserts `MyCustom` declared / `MYCUSTOM` assigned still renders
  `mycustom`). Unit-covered in `json_tests.rs`.
- **Golden `dyneq_full`** — the `TDynEqPCE` "DynInit" tail under Full, which
  `dyneq_micro`'s `skip_full` (a consequence of the ShaftModel gap) had left
  ungated. Added as a NEW deck instead of flipping `dyneq_micro`, so no existing
  golden bytes were regenerated.
- **Test-infrastructure guard:** `golden_json.rs` gained `assert_fixture_pins` —
  each new deck declares the oracle substrings it exists to pin (and, for
  `autotrans_solved`, the all-zero `WdgCurrents` it must never contain). A byte
  comparison alone cannot distinguish a fixture that pins the property under test
  from one that silently stopped rendering it; this does.
- Gate green (fmt + clippy + `cargo test --workspace`, corpus gate on both
  channels, exit 0). `TODO(compat)` still exactly 117; zero new
  `downcast_ref`/`as_any` sites.

#### OG-1.3a settle pass (2026-07-26) — audit findings dispositioned

Both audits returned PASS; every finding was re-settled empirically against the
pinned oracle (0.15.7 / backend 0.14.5) rather than argued. Three new goldens,
four new unit pins, one gate that did not exist before.

- **[FIXED — bug class completed] Capacitor `SQR`-binds-first, 3 sites.** The
  code audit flagged that the `Rdcpu * SQR(VBase)` fix had a surviving sibling.
  Swept every Pascal `SQR` site adjacent to a `*`/`/` (20 of the 92 in the
  vendored tree) against its port: Generator, PVSystem, Storage, Load, Reactor,
  IndMach012, InvDynamics, VSource, GICTransformer and the CIM exporter are all
  faithful (`powi(2)`, or Pascal itself writes `x*x` — verified for
  `pstcalc`/`flicker`/`WTG3 CalcWtRef`/`Load VRatio³`/`ExportCIMXML zbase`).
  **Capacitor was the lone outlier** and is now fixed at all three sites:
  `capacitor/solve.rs:29` (Capacitor.pas:642, derived `FC`),
  `capacitor/solve.rs:41` (Capacitor.pas:664, `Ftotalkvar`) and
  `capacitor/mod.rs:165` (Capacitor.pas:581, the `Create` seed). Oracle probe
  over 12 realistic (kV, kvar, f) triples: SQR-first matched 12/12,
  left-to-right 9/12.
  - The Create seed (line 581) is **dead**: `TCapacitorObj.Create` ends with a
    `RecalcElementData` that recomputes `FC` from SpecType = 1, so line 642
    always overwrites it. Proven by a 10-frequency sweep of a cmatrix-spec
    capacitor's `Cuf` — it matches the line-642 model 10/10 and the line-581
    model 4/10. Fixed anyway for faithfulness, and labelled as unobservable.
  - **Why no JSON golden:** the derived values render only under Full, and every
    Full Capacitor capture also emits `CMatrix`, whose oracle getter reads
    uninitialized memory **even when `cmatrix=` is set** — re-proven here, five
    fresh oracle processes gave five different renders. That is the same dss_capi
    UB `gen_props.py` already canonicalizes with `zero_garbage`; per the UB
    policy the port emits `null` instead of reproducing it, so the class is
    un-gateable through the byte channel, and the props round-trip channel
    compares at 1e-9 relative. The values are therefore pinned BIT-EXACTLY
    against recorded oracle probes in `capacitor/tests.rs` (4 tests; 3 of the 4
    verified to fail under a revert of the fix, the delta case coincides).
- **[FIXED — the test audit's headline gap] Transformer-side RDCOhms had no
  gate.** Every existing Transformer golden sets `rdcohms` explicitly, so only
  the `RdcSpecified` branch was ever rendered and the derived branch was
  asserted by analogy with AutoTrans. New golden
  **`transformer_derived_rdc`** leaves `rdcohms` unset on a 2-winding and a
  3-winding transformer; winding 1 of `t2` (115 kV wye, 1500 kVA, %r = 0.75) is
  an association-differing case, pinned positively (`1.8735416666666669E+001`)
  and the left-to-right value pinned negatively. Verified to FAIL under a revert
  of the `transformer/yterminal.rs` fix. Both transformers sit on unenergized
  buses so the Full sweep's `WdgCurrents` is the exact all-zero list: hanging
  winding 1 off `sourcebus` leaves a ~1e-13 cancellation residue that is
  solver-dependent noise and must not be byte-pinned.
- **[FIXED] `Spectrum` FullNames conversion was pinned for Generator only.** New
  golden **`spectrum_refs`** gates ten more of the thirteen converted classes
  against the oracle — Vsource, Load, Isource, PVSystem, Storage, IndMach012,
  VCCS, UPFC, VSConverter, GICsource all render `Spectrum.mycustom` — and pins
  GICLine's `SUPPRESS_JSON_LATE` NEGATIVELY (no `Spectrum` key at all). Verified
  to FAIL when Load's `object_ref_deferred` is reverted to `object_ref`.
  **WindGen remains ungated**: the class does not exist in the pinned 0.14.5
  oracle, so it has no capi channel (it is r4133/0.15.x-only).
- **[FIXED] DER user-model goldens pinned only the empty-string render.** Probed:
  the oracle STORES and renders an unresolvable model filename (it raises
  `#570 … Not Loaded` *after* storing), but pinning that needs the generator to
  tolerate a `DSSException`. The DATA surfaces need no loader at all, so new
  golden **`der_usermodel_assigned`** pins them non-empty — Generator
  `UserData`+`ShaftData`, Storage `UserData`+`DynaData`, PVSystem `UserData` —
  including the parenthesis stripping (`(Kp=1.5,Ki=0.25)` → `Kp=1.5,Ki=0.25`).
  **Deliberate non-fix:** the `UserModel`/`ShaftModel`/`DynaDLL` *filename*
  render stays unpinned; closing it means teaching `gen_json.py` to swallow
  oracle errors on selected commands, which trades a real gate property for a
  narrow one. Recorded as a standing follow-up.
- **[FIXED] No completeness guard tied goldens to drivers.** New
  `json_every_deck_golden_has_a_driver` walks `tests/golden/json/`, skips the
  `gen_schema.py` fixtures (no `combo_names`), and asserts every deck golden is
  replayed by a `run_deck("<stem>")` call, with an anti-shrink floor of 21. It
  immediately caught its own floor when the capacitor deck was withdrawn.
- **[SETTLED, no change] `autotrans_solved` rounding-boundary brittleness.**
  Measured, not assumed: re-solving the deck on the oracle at tolerance 1e-8 /
  1e-10 / 1e-12 (7 / 9 / 15 iterations — three different converged
  realizations) gives BYTE-IDENTICAL `WdgCurrents`; only a deliberately
  unconverged 1e-6 run moves the 8th digit. Recorded next to the deck.
- **[FIXED — docs]** `gen_json.py`'s `dyneq_micro` `skip_full` rationale no
  longer claims a live blocker (the gap is closed; the flag now only protects
  committed bytes). `golden_json.rs`'s `json_autotrans_micro` doc no longer
  claims a negative pin it does not make. `TESTING.md` "Regenerate a golden" now
  documents the deck-name filter and the driver requirement.
- **Fence deviations (recorded, not defects).** Both audits noted the round
  reaches past the briefed allow-list. Nothing on R3's FORBIDDEN list was
  touched (`exec/`, `solution/`, `meters/`, `controls/`, `cim/`,
  `obj/arena.rs`, `obj/base/mod.rs`, `elements/traits.rs`, `circuit/` — all
  clean). Beyond it: ten `pc/*/mod.rs` one-liners for the Spectrum switch,
  numeric edits in `pd/{auto_trans,transformer}/yterminal.rs` (marked
  read-only-preferred), and now `pd/capacitor/{mod,solve,tests}.rs`. Merge
  coordinator: check these 15 files against R3's typed-store rewrite. The
  Capacitor edit was taken under the "port gaps immediately" rule — the bug was
  found by this round's own sweep and is one ULP wide, i.e. invisible to every
  tolerance-based gate.
- Gate green (fmt + clippy + `cargo test --workspace`, corpus gate on both
  channels, exit 0). `TODO(compat)` still exactly 117; zero new
  `downcast_ref`/`as_any` sites; no existing golden regenerated.

---

### OG-1.6 NCIM `PV↔PQ` switching cadence → r4133; IEEE118Bus promoted (2026-07-26)

Branch `depas-og2`. Closes `ORPHANED_GAPS.md` §1.6. Ritual 0 held at start and
before the commit (186 `.pas` under `.inputs/dss_capi`, PowerShell recursion;
`cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`).

**The spec pointer in §1.6 was wrong.** There is no `Common/NCIMSolutionHelper.pas`
in any vendored EPRI tree — that file is the *retired capi015 r4103 refactor* the
port was originally written from. In r4133 NCIM lives inline in
`Version8/Source/Common/Solution.pas` (`DoNCIMSolution` l.1095 … `UpdateGenQ`
l.1993 … `BuildJacobian` l.2326). Every routine was re-read against r4133; all of
them already matched **except one structural difference**, so this is a cadence
tweak, not a solver restructure (the escape condition did not trigger).

**The cadence.** r4133 `UpdateGenQ` is `if (pGen.GenModel = 3) then … else begin
… end` (l.2059 / l.2166): the PV→PQ demotion (l.2117-2163) and the PQ→PV
promotion (l.2169-2295) are **mutually exclusive within one Newton pass**, and the
trailing "update currents for all the other gen models" loop (l.2301-2307) is
inside the `else` too — a model-3 generator's `Iterminal` was already stamped
per phase from `deltaQNom[j]` at l.2108. The port ran the PQ→PV block as an
unconditional second `if` and the current update unconditionally, so a generator
demoted PV→PQ could be promoted straight back in the same pass. Around
`|V| = VTarget` that is a limit cycle — the PV equation drives `|V|` onto the
target, the PQ test then reads `VNode` at the target and un-converts. r4088's
`Solution.pas` is byte-identical in this routine (diffed: only commented-out debug
file I/O differs), which is why r4088 and r4133 share the counts.

**Measured (live `epri-worker`, r4133 = `Version 11.0.0.1 (64-bit build) -
Charlottesville`, 2026-07-26).** `Solution.Iterations`/`Converged`,
`YNodeVarray`, `GeneratorsF(4)`:

| deck | r4133 | port before | port after |
|---|---|---|---|
| `pq_circuit(1)` / `(2)` / `pv_circuit(1.0)` | True/3 | True/3 | True/3 |
| `pv_circuit(1.01)` | True/**4** | True/8 | True/**4** |
| `pv_circuit(1.02)` | True/**4** | **False**/15 | True/**4** |
| `IEEE118Bus/master_file.dss` | True/**9** cold, 2 warm | **False**/100 | True/**9** cold, 2 warm |

IEEE118Bus's converged voltages are *exactly* the vector the port used to stall on
(`89_CLINCHRV.1 = 80072.708834`, `1_RIVERSDE.1 = 76088.991977`,
`4_NWCARLSL.1 = 79514.988474`): the fixpoint was always right, only the
convergence test never fired.

- **Code** (`solution/solution/ncim.rs`, the only solver file touched): the
  `else` arm + the relocated `Iterminal` loop, plus two loop-faithfulness fixes
  taken with it — the `deltaQNom[0] >= 0` sign test is re-read **per phase**
  (r4133 l.2151 / l.2253; phase 0's own overwrite feeds the later phases —
  identical whenever `qMax >= 0 > qMin`, faithful when not). Module doc now names
  r4133 `Solution.pas` as the spec of record and flags the stale capi015 line
  citations in the per-function docs.
- **`PV2PQList` deliberately NOT ported.** r4133's list (l.346, appended
  l.1938/2158, removed l.2260-2291, cleared l.645/l.1119) has no effect on the
  solve: `ReversePQ2PV` (l.1743) and `DistGenClusters` (l.1687) are dead code
  (declared, defined, never called), and the one **live** consumer is `Show
  PV2PQGen` (`ShowResults.pas` l.3617, Show verb 35 at `ShowOptions.pas:388`).
  The port's per-generator `Generator.ncim_expv` is the equivalent and already
  drives `Show PV2PQ_Conversions` — no new solution state, no `elements/**` field
  change.
  - *Settler correction:* it is **three** mutation sites, not two (the
    `GetNumGenerators` zero-Q-limit demotion l.1938, the `UpdateGenQ` PV→PQ
    conversion l.2158, the PQ→PV reversal l.2260-2291), and the first did not
    match — r4133 gates the append on `if InitQ then` (l.1936-1939) while the port
    set `ncim_expv` unconditionally. Effect: a *warm* re-solve of a generator
    edited back to `model=3` with `kvarmax=kvarmin=0` listed a generator in `Show
    PV2PQ_Conversions` that r4133 would not. Report-only (the model-4 demotion
    itself is unconditional on both), unreachable from the solver's own state (the
    PQ→PV promotion at l.2216 requires nonzero limits), and not probeable — `Show`
    is the sole consumer and is a file+editor path through the DLL — so settled on
    the r4133 source lines. Fixed (`if init_q { … }`). The same pass dropped a
    retired-capi015 leftover in the `Add2Limits` `else` arm (`gen_model == 3 &&
    ncim_expv`, unreachable inside that arm; r4133 l.1972-1973 is plain
    `Add2Limits := pGen.GenModel = 4`), so the module's "re-verified against
    r4133" claim now holds at a glance.
- **Unit pins re-measured, not loosened** (`exec/tests/ncim.rs`): `pv_qlimit`
  8 → **4** iters; `ncim_pv_aggressive_nonconvergence_is_faithful` → renamed
  `ncim_pv_aggressive_qlimit_converges_matches_r4133` (that test pinned a *shared
  capi015 non-convergence* that r4133 does not have — it now pins r4133's
  converged 4-iteration answer, the same clamped fixpoint as `vpu=1.01`). New
  shared helper `assert_gen_q_clamped` pins BOTH r4133 facts: terminal powers
  `(−266.667 kW, −500 kvar)` per conductor (the real +1500 kvar clamp) **and**
  `Generators.kvar = 0.0` — probed on r4133, because `GetNCIMPowers` writes
  `Qnominalperphase := deltaQNom[j]` only on its model-3 arm (l.1308), so after
  the conversion the last write is iteration 1's zero. The port's old `1500` was
  a capi015-cadence artifact (its generator was model-3 again on the final pass).
- **Frozen capi015 goldens untouched and still green.** `tests/golden/ncim/`
  (UNREGENERABLE — retired venv) passes unchanged on both decks: Jacobian nnz +
  values, deltaF/deltaZ shape, and the byte-exact PV2PQ list. The cadence changes
  *how* the fixpoint is reached, not the converged Jacobian nor which generator
  ends up converted.
- **Corpus.** `IEEE118Bus/master_file.dss` moved out of
  `skipped_needs_investigation.json` (tag `ncim_pv_pq_switching_divergence`) into
  `solvable_now.json` — `engines:"r4133"` (the pinned 0.14.5 oracle predates NCIM),
  `kind:"large"`, `n_steps:1`, **no ledger entry, no tolerance change**: the
  whole-model compare (354 nodes, 353 elements, Y, injection, warm-resolve
  iterations = 2) matches r4133 first try. The four existing NCIM cases
  (`modes/ncim/*` + `Kundur2Area`) keep their exact iteration counts.
  `ad_sweep.json` gets its mandatory disposition. **Settler correction:** the
  first label `off:mode-outside-AD-scope` was wrong — that class is defined as
  "dynamics/harmonics/faultstudy/monte/LD", and this is a `mode=snap` deck, i.e.
  squarely inside AD's mode scope (what is outside scope is the NCIM *algorithm*,
  not the mode), and `ad_disposition_is_valid` only checks class membership so the
  wrong-but-valid label passed the gate and would have misdirected the tracked
  WP-AD.5 per-deck replay. The measured fact is a different one, re-probed on
  r4133 2026-07-26 (`epri-worker`, `Version 11.0.0.1`): the AD probe's **normal
  arm** (`ad_solve_normal` = `compile; set controlmode=off; solve mode=snap`) does
  not converge, and does not converge upstream either — cold compile True/**9**,
  then `solve mode=snap` **False/100 on r4133**, exactly as on the port, while a
  plain warm `Solve` is True/**2** on both; the re-entry non-convergence tracks
  `mode=snap`, not the control mode (probed with and without `set
  controlmode=off`: identical). So there is no AD-vs-normal baseline at all. New
  precise reason class `ad-baseline-nonconvergent` added to `AD_OFF_REASONS`
  (`corpus_gate/manifest.rs`) + the `ad_sweep.json` comment, and the disposition
  is now `off:ad-baseline-nonconvergent`. Deliberately *not* folded into
  `ad-nonconvergent`, which means the AD arm failing on a singular torn zone.
  Population lock regenerated deliberately (`solvable_now` 293→294,
  `skipped_needs_investigation` 13→12, `ad_sweep` 293→294).
- Gate green (fmt + clippy + `cargo test --workspace`, corpus gate both channels);
  `tests/corpus` pristine; `TODO(compat)` unchanged (123 in `crates/**/*.rs`, same
  as base `634aac98`); no new `downcast_ref`/`as_any`; no existing golden
  regenerated.
  - *Settler finding — a real `CorpusGuard` leak, now fixed.* The tree was **not**
    pristine at the end of the two port steps (20 untracked artifacts under
    `tests/corpus`), and the cause is not only out-of-band probe runs: a full
    `cargo test --workspace` **does** leak, reproduced twice with a *different*
    file set each time (`Test/AutoTrans/Auto3bus_*`+`AutoAuto_*`,
    `StorageControllerTechNote/{Support,Time}/IEEE8500u_*`,
    `GFM_IEEE8500/IEEE8500_Mon_*`). Root cause: `CorpusGuard` (`corpus_gate/
    runner.rs`) snapshotted **per guard**, while the corpus puts many decks in one
    folder and the scheduler runs cases in parallel — guard A snapshots a clean
    dir, A's engine writes `X`, guard B then snapshots and adopts `X` as vendored,
    A drops and sweeps `X`, B's engine rewrites `X`, B's drop keeps it. (Same
    shared-directory race as the `AutoHLT.dss` `I/O error 103` flake noted under
    OG-1.10.) Mitigated by sharing **one pristine snapshot per directory** with a
    refcount, sweeping only when the last guard leaves; new unit test
    `corpus_guard_overlapping_guards_still_sweep` pins the interleaving, and the
    existing `corpus_guard_restores_case_dir_recursively` is unchanged.
    Independently of that, the newly promoted `IEEE118Bus` deck is **not** a
    leaker: its `show voltages`/`show powers` output was swept on every gate run
    (the pair only ever appeared after a manual `epri-worker` probe, which chdirs
    into the deck directory outside any guard).
  - *Open follow-up (NOT closed here — escape protocol).* The refcount fix removed
    the `StorageControllerTechNote/{Support,Time}` and `GFM_IEEE8500` leaks (two
    post-fix gate runs: gone), but `Test/AutoTrans` still leaks intermittently
    (run 1 clean, run 2 left 8 of `Auto3bus.dss`'s 9 explicitly-named exports).
    The surviving signature points past the snapshot to a **write that outlives
    its guard**: 8 files survive while the 9th and last, `Auto3bus_Load_voltage.txt`,
    was swept — i.e. a later guard on the same folder snapshotted *after* the first
    8 landed and adopted them as vendored. Note these are `export … file=<relative>`
    names, which resolve against the DSS **current dir** (the deck dir) and ignore
    `Set DataPath`, so no scratch-dir redirection can move them. Chasing which
    writer escapes (pooled `epri-worker` cwd lifetime vs the AD-sweep arms vs the
    oracle server) is a scheduler-level investigation, out of slice B's scope and
    pre-existing on `update` — recorded here rather than half-fixed. Workaround
    until then: the artifacts are gitignored-by-absence only, so remove by exact
    name before committing (`git status --porcelain -- tests/corpus`).

---

### OG-1.10 `Export Estimation` (export verb 5) + the CDPSM error-text rider (2026-07-26)

Branch `depas-og2`. Closes `ORPHANED_GAPS.md` §1.10 — the last `EXPORT_OPTIONS`
keyword without a dispatch arm. Three new report goldens; no existing golden
regenerated (`gen_reports.py` grew a generator-name filter, mirroring
`gen_json.py`/`gen_checkpoints.py`, so a new fixture cannot drag unrelated oracle
drift into the committed bytes: `python tools/golden/gen_reports.py estimation`).

- **`report/export/estimation.rs`** ports `ExportResults.pas:1652` loop-for-loop.
  The layout quirks are reproduced, not smoothed:
  - the `TempX: array[1..3]` staging buffer is zeroed before the *target* and
    *calculated* passes but **deliberately not** before the *percent-error* pass
    (which therefore consumes the calculated magnitudes in place, and leaves the
    slots past `Nphases` at the zero the calculated pass wrote — that is how a
    1-phase meter prints `50, 0, 0, 51.3317, 0, 0, -2.66342, 0, 0`);
  - `%Err = (1 - calc / Max(0.001, target)) * 100` — the `Max` clamp is what makes
    an unspecified target render `100`, not `inf`/`NaN`;
  - `Cabs(CalculatedCurrent^[i])` is read from the head of the buffer with **no**
    metered-terminal offset (unlike `CalcAllocationFactors`, which does offset);
  - `Get_WLSCurrentError` is a *mutating* getter (P-specified sensors re-derive
    `SensorCurrent` from `kWs`/`kvars` and latch `Ispecified`, `Sensor.pas:599-631`);
    it is called in Pascal's position — after the row's own columns — so the
    side effect is observable only to later reads, exactly as upstream;
  - every number `Format('%.6g')` via `report::format::g(v, 6)`.
  `Nphases > 3` is the one non-reproduction: Pascal writes `TempX[i]` for
  `i := 1..Nphases` into a `1..3` stack array (UB, not a defined bug), so
  `temp_x_slots` clamps (unit-pinned by `estimation.rs::temp_x_cadence_clamps_and
  _does_not_rezero`, added in the settler pass). r4133
  `Version8/Source/Common/ExportResults.pas:1599` is **semantically** identical to
  the pinned 0.14.5 source (columns, order, headers, `TempX` cadence, `Max(0.001,·)`
  clamp — both gating oracles agree on every field), *not* character-identical:
  r4133 uses `TextFile` + `Write`/`Writeln` (CRLF) and `Uppercase`, 0.14.5 uses
  `TBufferedFileStream` + `FSWrite`/`FSWriteln` (LF) and `AnsiUpperCase`.
- **Routing** (`exec/report.rs`): verb 5 → `export_with_mut` →
  `EXP_ESTIMATION.csv`. It needs `&mut classes` only for the WLS getter; the
  solved node voltages are unused (the report is a *read* of stored sensor
  arrays). The existing #24712 solution guard already covers it (5 ∈ the `1..24`
  set) — checked against `ExportOptions.pas:163-177`, no new guard code.
- **Goldens** (`tests/golden/reports/export_estimation{,_noalloc,_empty}.txt`,
  `golden_reports.rs::export_estimation*`, policy `sep=','`, `header_lines=2`,
  `rel=abs=0.0` — exact):
  - `est8` — the allocated path. Two feeder heads so both EnergyMeters are legal:
    3-phase `m1` (unequal `peakcurrent=`) and **1-phase** `m2` (the sub-3-phase
    column shape). Three Sensors cover every spec — current, P/Q (`weight=2`,
    driving the mutating WLS getter), and a 1-phase voltage+current sensor (the
    only nonzero `V… Target` / `WLSVoltageError`) — plus a **disabled** `s4` that
    must not appear. A fixed non-allocatable kW load inside `m1`'s zone keeps the
    allocation loop from landing on the target, so the `%Err` columns are 17-57 %,
    nowhere near a cancellation floor.
  - `estns` — solved but **not** allocated: nonzero targets vs an all-zero
    `CalculatedCurrent`/`CalculatedVoltage`, so every `%Err` is the `100` form and
    the WLS residuals are the pure `-Weight * sum(target²)` term. Also carries the
    **disabled EnergyMeter** `mdis` (settler pass — see below).
  - `estem` — no meters, no sensors: both section headers, both bodies empty.
  - GAPS §3 proof: data-bearing; two independent oracle processes byte-identical;
    four mutations each caught by `est8` — dropping the *sensor* `Enabled` filter
    (row count 7→8), re-zeroing before the percent-error pass (17.816 → 100),
    ignoring `Nphases` (M2's `I2 Calc` 0 → 51.3357), swapping the two WLS columns.
  - `export_estimation_blank_line_layout_matches_oracle` (settler pass) closes the
    one structural hole the numeric comparator cannot see: `compare_export` runs on
    `report_lines()`, which drops blank lines, so Pascal's `FSWriteln(F)` separator
    between the two sections (`ExportResults.pas:1711`) was unpinned. The test
    replays all three decks and requires the blank-line positions + line count to
    match the captured oracle file exactly.
- **Empirical: the meter-side `Enabled` filter (settler pass).** The original
  claim — "not gate-able, pinned by `estns`+`estem` instead" — was **wrong**:
  neither fixture had a disabled meter, so deleting the port's
  `if !em.med.cd.enabled { continue }` changed no golden. Re-probed on the pinned
  0.14.5 oracle: the access violation (#303, `TEnergyMeterObj.AllocateLoad` walking
  a `BranchList` the disabled meter never built — the `if not Enabled then Exit`
  r4115/D9 fix the port already carries) is specific to **`allocateloads`**, which
  `estns` never runs. `estns` now carries `energymeter.mdis … enabled=no` on its
  own feeder head; the oracle lists it in `Meters.AllNames` and omits it from the
  report, so the filter is genuinely pinned (dropping it adds an
  `"Energymeter.MDIS"` row). Only `export_estimation_noalloc.meta.json` changed —
  the golden `.txt` bytes are unchanged, and `est8`/`estem` regenerated identical.
- **Rider — the retired CDPSM profiles.** Verbs 22/28-31 get their own arms
  emitting Pascal's exact fixed text (`<Profile> export no longer supported; use
  Export CIM100`, `ExportOptions.pas:543`/`:555-561`; r4133
  `Version8/Source/Executive/ExportOptions.pas:461`/`:467-470` — identical) and
  write no file. **Corrected in the settler pass:** they do NOT leave the last-file
  state untouched. `AbortExport` is set only by the unknown-keyword `else`
  (`ExportOptions.pas:626`), so for these five *resolved* keywords `DoExportCmd`'s
  tail (`:632-637`; r4133 `:514-516`) still runs `SetLastResultFile` +
  `@lastexportfile` over the empty default filename (`:354`/`:366-373`) prefixed at
  `:439` → `<OutputDirectory><CircuitName_>`. Live probe on the pinned 0.14.5
  oracle: after `export voltages` both vars read `…\t_EXP_VOLTAGES.csv`; after
  `export cdpsmasset` (which raises #252) both read `…\t_`; after an *unknown*
  keyword they stay put. The original commit's arms skipped the tail and the test
  asserted the divergence. Now a shared `Dss::set_export_last_file` implements the
  tail once, used by both the CDPSM arms and `export_ad` (which already reproduced
  it — the two were handled opposite ways in the same file).
  `exec/tests/report.rs::export_router_outcomes_match_oracle` (renamed from
  `export_records_scoped_not_ported`, whose `Estimation` example this WP
  invalidated) now pins `LastResultFile`/`@lastfile`/`@lastexportfile` = `…\t_`
  plus "no file created" for all five keywords, and the *unchanged* state for the
  aborting unknown keyword. The default arm's stale "(Phase 8)" wording is gone; it
  is now **unreachable** (all 64 keywords routed), documented as the safety valve
  for a keyword added to the table without a route — and therefore **untested by
  construction**, which the test doc now says out loud.
- **Left open (out of this worktree's write fence):** the `Estimate` *command*
  (`EXEC_COMMANDS` ordinal 90, `ExecHelper.pas:4225` = `DoAllocateLoadsCmd` +
  `Set showexport=yes` + `Export Estimation`) is still unrouted in
  `exec/command.rs` and falls to `not_ported_command`. Both constituents now
  exist, so it is a small follow-up; recorded in `ORPHANED_GAPS.md` §1.10.
- Gate green (fmt + clippy + `cargo test --workspace`, corpus gate on both
  channels). `TODO(compat)` unchanged at **123** occurrences in `crates/**/*.rs`
  (base `634aac98` = 123; the "117" in the worktree brief is stale against this
  baseline — the only delta anywhere is +2 prose mentions in this file); zero new
  `downcast_ref`/`as_any` sites; no existing golden regenerated; `tests/corpus`
  pristine **at commit** (see the `CorpusGuard` item under OG-1.6 — the gate
  itself still leaks `Test/AutoTrans` artifacts intermittently, so "pristine"
  means swept by exact name before committing, not "the gate never writes").
  - *Flake note:* one full-workspace run had `corpus_gate` fail with the **r4133
    oracle** raising `I/O error 103` on `Test/AutoTrans/AutoHLT.dss`'s
    `export losses file=…`; it passed on re-run and on every subsequent run. That
    deck family writes report files into the shared `Test/AutoTrans` corpus dir
    from several parallel jobs — a pre-existing scheduler/IO race, untouched by
    this WP, and the **same** shared-directory mechanism the settler pass
    root-caused and partly fixed (OG-1.6 `CorpusGuard` item).

---
