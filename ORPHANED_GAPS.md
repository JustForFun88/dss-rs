# ORPHANED_GAPS — deferred work no plan will pick up

**Purpose.** dss-rs was built as a *chain* of plans, each deferring some work to a
named successor. Most deferrals have a live owner (see §2). The items in **§1** were
handed to a successor that **does not exist** or **will never reach them** — they are
**orphans**: real, self-contained work that falls through the cracks unless someone
deliberately schedules it. A fresh agent can pick up any §1 item standalone.

Audited 2026-07-18 against the tree (branch `update`); statuses refreshed 2026-07-25
(§1.9 closed by BUG WP DynExp + NCIM re-gate; §2 owner statuses). Everything here is **outside**
the parity gate — the mandatory `cargo test` stays green without them; they are missing
*features*, not bugs. Read `PORTING_PLAN.md` §0 first (source-integrity gate + Pascal =
spec), then `CLAUDE.md` (conventions). Each entry cites the Pascal spec + the current
Rust `NOT_PORTED` site so you can start immediately.

Distinct from **§2 owned deferrals** (a live plan tracks them — do NOT re-port here) and
**§3 permanent non-ports** (decided "never" by design — do NOT port).

---

## 1. Orphans — actionable, unowned

### 1.1 `Export GICMvars` (report verb 36) + `GICTransformer.WriteVarOutputRecord`
**PORTED 2026-07-18** on `og11-gicmvars` — see STATUS §OG-1.1.

### 1.2 AltDSS JSON `DynInit` tail
**PORTED 2026-07-18** on `og1213-json-tails` — see STATUS §OG-1.2+1.3.
`obj_to_json_data` emits the `TDynEqPCE` `"DynInit"` tail; golden `dyneq_micro`.

### 1.3 AltDSS JSON `Full`-mode solve-state matrices
**WdgCurrents PORTED 2026-07-18** on `og1213-json-tails` — see STATUS §OG-1.2+1.3.
Transformer/AutoTrans `WdgCurrents` now render via a `&mut` JSON refresh route
(`obj_to_json_mut`/`class_batch_to_json_mut`); golden `transformer_micro` Full.
**Capacitor `CMatrix` = proven UB non-port** (uninitialized heap, nondeterministic
across oracle processes) — not reproduced.
**Both remaining §1.3 blockers CLOSED 2026-07-26** on `depas-og` — see STATUS
§OG-1.3a. The AutoTrans array-alternative metadata turned out already landed
(`430d033`, og15c-B6), so the gap was the missing golden: `autotrans_micro`
(default + Full sweep) and `autotrans_solved` (NONZERO Full `WdgCurrents`, the
auto's own `GetAllWindingCurrents`) now pin it byte-exact. The
`ShaftModel`/`ShaftData` Full-render gap re-triaged clean post-WM.3/WM.4 and is
pinned by `der_usermodel_full` (Generator + Storage + PVSystem).
**Settle pass 2026-07-26** (same branch, audits dispositioned): added
`transformer_derived_rdc` (the derived-RDCOhms branch, previously ungated on the
Transformer side), `spectrum_refs` (ten more of the thirteen `Spectrum`
FullNames conversions + GICLine's suppression pinned negatively) and
`der_usermodel_assigned` (non-empty user-model DATA strings); completed the
`SQR`-binds-first bug class in the Capacitor (3 sites, pinned bit-exactly in
`capacitor/tests.rs` because the class's Full JSON is poisoned by the `CMatrix`
UB above); added `json_every_deck_golden_has_a_driver`. Two residuals stay open
and are listed under STATUS "Standing open follow-ups": the user-model FILENAME
render (needs a generator that tolerates the oracle's `#570` load error) and
WindGen's `Spectrum` render (no capi channel — class absent from 0.14.5).

### 1.4 AltDSS JSON **import** (`Obj_Circuit_FromJSON_`)
**PORTED 2026-07-18** on `og14-json-import` — see STATUS §OG-1.4. `Dss::circuit_from_json`
(+ hand-rolled `parse_json`, `ClassProps::fill_from_json`/`set_json_value`,
`AltPropertyOrder`); oracle-backed byte round-trip goldens (rt_micro / rt_transformer /
rt_ieee13) via `Circuit_FromJSON` reachable on the pin. `DynInit` tail (§1.2) still
deferred.

### 1.5 `CAPI_Schema` JSON schema export
- **STATIC CORE PORTED 2026-07-18** on `og15-capi-schema` — see STATUS §OG-1.5.
  The schema envelope + the ten reusable global `$defs` + the static
  `circuitProperties` head are ported byte-exact vs the oracle
  (`report/export/json/schema.rs`, `Dss::extract_schema_json`, golden
  `tests/golden/json/schema_static_core.json` via `tools/golden/gen_schema.py`).
- **PER-ENUM WALK PORTED 2026-07-18** on `og15b-schema-full` — see STATUS
  §OG-1.5b. `prepareEnumJsonSchema` + the 21-entry `DSS.Enums` global list
  (`DSSClass.pas:1058-1196`) are ported byte-exact vs the oracle
  (`report/export/json/schema/enums.rs`, golden `enum_defs` in
  `schema_static_core.json`, driver `golden_schema.rs`). Also groundworked: the
  property-help catalog extraction (`tools/golden/extract_schema_help.py` reads
  the dss_capi gettext resource `dss/messages/properties-en-US.mo` — validated to
  reproduce every schema description by leaf key + array-alternative redirect,
  no parent fallback needed), and the `AltPropertyOrder` computation is fully
  located (`DSSClass.pas:1830/1934-2010`, `zorderNextStart=-999`/`End=999`,
  `Ordering_First/Last` used by only 4 classes).
- **PER-CLASS WALK — INFRA + PILOT PORTED 2026-07-18** on `og15b-schema-full` —
  see STATUS §OG-1.5c. `prepareClassJsonSchema` (`CAPI_Schema.pas:325-1134`) is
  ported as `report/export/json/schema/classes.rs::class_schema`
  (`Dss::schema_class_def`), with ALL shared machinery built: the complete
  `Units_*` `PropFlags` family (`PropFlags` widened to `u128`), the `BOOLEAN_ACTION`
  marker, schema-side `SpecSets` (`schema/spec_sets.rs`), class-local enum
  rendering (shared `render_enum` + `EnumMeta` from `DssEnum` + `enum_overrides`),
  `$dssPropertyOrder` from the ported `AltPropertyOrder`, and help via
  `report::help_catalog`. Proven **byte-exact** on the 6-class pilot (first six
  `DSSClassList` classes) via `gen_schema.py::SCHEMA_CLASSES` +
  `golden_schema.rs::ported_class_defs_bytes_match_oracle`, with the committed
  expected-divergence inventory (`tests/golden/json/schema_divergences.json`,
  fail-on-stale) covering LineCode's 3 r4133 `deprecated` diffs.
- **FULL DOCUMENT — PORTED 2026-07-18/19** on `og15b-schema-full` — see STATUS
  §OG-1.5c (batches B1–B6 + integration). All 49 oracle class `$defs` + the
  port's 50th class WindGen are emitted; `Dss::extract_schema_json` now returns
  the **whole** `DSS_ExtractSchema(jsonSchema=True)` document (the `# Incomplete`
  caveat is gone) via `schema::assemble_full_document` over
  `DSS_CLASS_LIST_ORDER` (the `<Class>List`/`<Class>Container` triples +
  `circuitProperties` refs, `CAPI_Schema.pas:1479-1513`). Byte-gated three ways
  in `golden_schema.rs`: 45 classes byte-exact vs the pinned 0.14.5 oracle after
  the documented r4133/0.15.x divergences (`schema_divergences.json`,
  fail-on-stale); the full document vs a pinned PORT golden
  (`schema_full_port.json`) AND reconciled byte-for-byte against the verbatim
  oracle document (`schema_full_oracle.json`); the 5 structural port-authored
  classes (LineGeometry/Relay/Recloser/SwtControl/WindGen) inventoried
  (`port_authored_classes`) with a fail-on-stale gated/authored split. **§1.5 is
  CLOSED.**
- **SETTLE 2026-07-19** (two audits, both green): added a registry-coverage assert
  in `extract_schema_json` (the walk drove off the static `DSS_CLASS_LIST_ORDER`
  only; the guard cross-checks it against the live registry so a future-registered
  class can't be silently omitted). Accepted residuals (both low, plan-sanctioned):
  the 4 oracle-present port-authored classes (LineGeometry/Relay/Recloser/SwtControl)
  are pinned to the port golden only — empirically confirmed genuine structural
  divergence (SwtControl's `$dssPropertyOrder` remap is not inventory-expressible);
  and the `REGEN_SCHEMA_PORT` env-guard matches the repo's `DSS_REGEN_*` convention.
  See STATUS §OG-1.5c settle round.

### 1.6 IEEE118Bus NCIM `PV→PQ` r4133 switching cadence
**ADOPTED 2026-07-26** on `depas-og2` — see STATUS §OG-1.6. The r4133 cadence is
one structural difference, not a solver restructure: `UpdateGenQ`'s
`if GenModel = 3 … else …` (r4133 `Version8/Source/Common/Solution.pas` l.2059 /
l.2166; byte-identical in r4088) makes PV→PQ and PQ→PV **mutually exclusive
within one Newton pass**, while the ported capi015 r4103 form ran the PQ→PV test
unconditionally — so a generator converted PV→PQ could be flipped straight back
in the same pass. That chatter (around `|V| = VTarget`) was the whole
non-convergence: IEEE118Bus now converges in **exactly r4133's 9 cold iterations**
at the same voltages, `Xmission_System_Kundur2Area` + the three `modes/ncim/*`
decks keep their exact iteration counts, and the frozen capi015 report goldens
(`tests/golden/ncim/`) still pass unchanged. `IEEE118Bus/master_file.dss` is
promoted to `solvable_now` with `engines:"r4133"`, no ledger entry; the two unit
pins that encoded capi015 cadence artifacts (`pv_qlimit` 8 iters; the `vpu=1.02`
"shared non-convergence") are re-pinned to live r4133 probe values (4 iters,
converged). Note: r4133's `PV2PQList` bookkeeping needs no port — the port's
per-generator `Generator.ncim_expv` flag already is it, mutated at the same three
sites (l.1938 / l.2158 / l.2260-2291). Of r4133's consumers, `ReversePQ2PV` and
`DistGenClusters` are dead code; the one **live** consumer is `Show PV2PQGen`
(`ShowResults.pas` l.3617, Show verb 35), which the port already has as `Show
PV2PQ_Conversions`. The settler pass fixed the one site that did not match — the
zero-Q-limit demotion, whose record r4133 gates on `if InitQ` (l.1936-1939) —
and dropped an unreachable capi015 leftover from the `Add2Limits` `else` arm.
Corpus disposition corrected in the same pass: `off:mode-outside-AD-scope` →
`off:ad-baseline-nonconvergent` (a new reason class; the deck is a `mode=snap`
deck, so the old label was simply wrong — the measured fact is that the AD
probe's *normal* arm does not converge, on r4133 as on the port).

### 1.7 UPFC control modes 2/3/5
**PORTED 2026-07-18** on `og17-upfc-modes` — see STATUS. The mode dispatch
(0..5) was already implemented in `crates/dss-core/src/elements/pc/upfc/compute.rs`
(GetOutputCurr/GetInputCurr/CheckStatus/CalcUPFCPowers cover all six modes); the
gap was the missing test surface. Added three live corpus decks under
`tests/corpus/controls/upfc/` (`upfc_statcom.dss` = mode 2, `upfc_dual.dss` =
mode 3, `upfc_doubleref_dual.dss` = mode 5), each GAPS §3-proven (pin
solves+converges; two-process bit-identical; feature-sensitive), registered in
the controls manifest + `CONTROLS_REQUIRED` floor, gated vs the pinned oracle
(full model + all 14 UPFC state variables).

### 1.8 UTF-8 BOM residual edge cases *(minor)*
- **Deferred by:** final acceptance / GAPS ("GAPS follow-up"; GAPS is now closed).
- **What:** the main BOM issues were fixed (CF-A redirect BOM strip; `capture_eventlog` → `utf-8-sig`). Residual: any not-yet-covered BOM/encoding edge on odd input paths.
- **To do:** only if a real deck surfaces it — add the strip at the parse boundary + a fixture. **Priority: very low** (opportunistic).

### 1.9 UNIFIED_GATE `defer_ledger` retirement — DynExp×2, RegControl idle, line_spacing_asym
**CLOSED 2026-07-20 — defer_ledger remaining: 0.** All four owned here retired
(line_spacing_asym + regcontrol_idle below), plus the two follow-up WPs landed:
**DynExp×2** retired by **BUG WP DynExp** (2026-07-19, STATUS §BUG WP DynExp) —
the D14 `SolveEq` no-op was the port-side bug; reverted to the full 0.14.5/r4133
evaluator, both decks live-gate with no ledger entry. **NCIM×4** retired by the
**NCIM re-gate** (2026-07-20, STATUS §NCIM re-gate) — oracle-of-record flipped to
r4133, all 4 cases live-gated, no ledger entries. Historical detail below.
- **Deferred by:** UNIFIED_GATE Phase D; **worked by the post-audit fix round (STATUS §1l, 2026-07-19)** + **BUG WP regcontrol_idle (2026-07-19).**
- **RETIRED — `asymmetric line_spacing_asym`** → `engines:"both"`: capi_v0145 exact-pair-NUMERIC property+probe entries (Line.lsp.normamps 730→230, emergamps 1095→345; the min-over-phase D3 upgrade, port CORRECT) + r4133 `#303` skip. This required the exact-pair-numeric probe/property machinery §1i said was lacking — it was ADDED this round (num_rel absent + oracle/rust pins ⇒ exact float pin). Line.lspc is D3-invariant (165/247.5, no divergence). Done.
- **RETIRED — `controls regcontrol_idle`** → `engines:"r4133"` (BUG WP regcontrol_idle, STATUS record, 2026-07-19): the 8.8% MV-bus gap was a **port bug relative to r4133**, not a ledgerable divergence. The port faithfully reproduced dss_capi 0.15.x's idle no-load-zone test written as an **OR** — `(FwdPower ≥ RevThr) or (FwdPower ≤ FwdThr)`, a tautology under the default ±100 kW band that makes an idling reg never tap (MV.1 held 7030.24 V, tapnum 0); EPRI r4088/r4133 use the correct **bounded AND** (RegControl.pas:1218), so idle=yes taps identically to idle=no when the through-power (~7438 kW) is outside the band. Dated: OR is in `8a898cba` "port SVN r4086", still OR at the 0.15.x tip; r3723/0.14.5 have no idle. Live r4133 probe D (widen fwdThreshold to bracket the load → r4133 idles to the port's old 7030.24 V) proved the mechanism. Adopted r4133's AND (`reg_control/control_loop.rs`, r4133-and-beyond direction; idle is 0.15-only, no pinned golden exercises the no-load physics — the one idle golden pins property readback only); tap/voltage/full-model now match r4133 exactly. Residual = the revThreshold/fwdThreshold getter convention (r4133 display strings '100'/'' vs port signed kW −100/+100) ledgered exact-pair (`r4133-regcontrol-idle-threshold-display`). Done.
- **RETIRED — `Dynamic_KundurDynExp` + `GFL_IEEE123 …DynExp`** (BUG WP DynExp, 2026-07-19): the "matches neither oracle" signal was a port-side bug — the D14-adopted no-op `SolveEq` (early `Exit` from retired capi015) froze the DynExp state at its seed. Reverted to the full 0.14.5/r4133 evaluator; the port now swings with both oracles (step-1 `dspeed` -1.6169543e-6 to the f32 floor). Both decks live-gate, no ledger entry. Done.
- **RETIRED — NCIM×4** (NCIM re-gate, 2026-07-20): r4133 declared oracle-of-record for NCIM (capi015 probe venv retired); the swing-report `+1` shift dropped, 4 cases (`ncim_pq`/`ncim_pv_pq`/`ncim_midi`/`Kundur2Area`) live-gated on r4133, warm-resolve iteration counts match exactly, no ledger entries. Done.
- **defer_ledger remaining: 0.**

### 1.10 `Export Estimation` (export verb 5) — functional exporter never ported
**PORTED 2026-07-26** on `depas-og2` — see STATUS §OG-1.10. New
`report/export/estimation.rs` ports `ExportEstimation` loop-for-loop (the
`TempX[1..3]` staging buffer with its deliberate *no* re-zero before the
percent-error pass, `Max(0.001, target)` denominators, `%.6g` everywhere,
`Get_WLSCurrentError`'s mutating P→I re-derivation in Pascal's order); verb 5
routes in `exec/report.rs` to the standard `EXP_ESTIMATION.csv` path and keeps
the existing #24712 solution guard (5 ∈ the `1..24` set). Three oracle-generated
goldens (`export_estimation{,_noalloc,_empty}`, `tools/golden/gen_reports.py
estimation`) gate it at `rel/abs = 0`, GAPS §3-proven (data-bearing, two-process
byte-identical, and four mutations each caught by `est8`: the *sensor* `Enabled`
filter, the percent-error re-zero, the `Nphases` slot count, the WLS column
order). Settler pass added the missing coverage: the **meter-side** `Enabled`
filter (a disabled `energymeter.mdis` in the `estns` deck — the 0.14.5 access
violation that blocked it is specific to `allocateloads`, which `estns` never
runs), the blank-line section separator (`compare_export` drops blank lines), and
the `Nphases > 3` clamp (a Rust-side unit test — upstream UB, not oracle-gateable).
**Rider done:** verbs 22/28-31 now have their own arms with Pascal's exact
message (`ExportOptions.pas:543`/`:555-561`; r4133 `:461`/`:467-470`) **plus**
`DoExportCmd`'s last-file tail (`:632-637` / r4133 `:514-516`), which the first
commit wrongly skipped: `AbortExport` is set only by the unknown-keyword `else`,
so these five resolved keywords DO overwrite `LastResultFile`/`@lastfile`/
`@lastexportfile` with `<OutputDirectory><CircuitName_>` — live-probed on the
pinned 0.14.5 oracle. The default arm's stale "(Phase 8)" wording is gone — it is
now unreachable, every one of the 64 `EXPORT_OPTIONS` keywords is routed.
**Tail rider done (DE_PASCALIZE W3.5, `depas-final`, 2026-07-26):** the `Estimate`
*command* — ordinal **76**, not 90 (`ExecCommands.pas:97`; the "90" above was a
mis-transcription), `ExecHelper.pas:4213` = `DoAllocateLoadsCmd` + `Set
showexport=yes` + `Export Estimation` — now routes in `exec/command.rs` to
`exec/solve.rs::do_estimate_cmd` instead of falling to `not_ported_command`.
Oracle-backed pin without a new capture: the `est8` deck minus its
`allocateloads`, driven by the single word `estimate`, reproduces the existing
`export_estimation` golden (STATUS §DE_PASCALIZE W3.5). **§1.10 is closed.**
- **Found:** 2026-07-25, assumption-gap sweep (strict re-verification flipped it from
  "benign_documented" to real gap — visible-error deferral, but real lost functionality).
- **Spec:** `ExportOptions.pas:452` → `ExportEstimation` (`ExportResults.pas:1652+`):
  writes `EXP_ESTIMATION.csv` — EnergyMeter `SensorCurrent`/`CalculatedCurrent` rows +
  Sensor target/calculated voltage & current + WLS `%err` columns. Reachable directly
  (`Export Estimation`) and via `ExecHelper.pas:4225` (`Estimate` command tail).
- **Current Rust:** `exec/report.rs:484-489` default arm errors
  `Export "Estimation" is not ported yet (Phase 8).` and writes no file. The "(Phase 8)"
  label is stale (Phase 8 complete). Not silent — the caller sees an error — but the
  export itself is genuinely missing and ungated (Export decks sit in `skipped_unsupported`).
- **Rider (error-text fidelity, same file):** CDPSM verbs 22/28-31 are *removed upstream*
  (Pascal errors `"<X> export no longer supported; use Export CIM100"`, code 252,
  `ExportOptions.pas:543-561`); Rust routes them to the same generic "not ported yet
  (Phase 8)" placeholder. Give them their own arms echoing Pascal's exact message + drop
  the stale Phase-8 wording from the default arm.
- **To do:** port `ExportEstimation` loop-for-loop + oracle-gated golden (needs a deck
  with EnergyMeter sensors/`Estimate`); do the CDPSM/label rider alongside. **Priority:
  low** (state estimation is a rarely-used subsystem; no corpus deck exercises it).

### 1.11 WindGen power-flow models **3** (`DoPVTypeGen`) and **7** (`DoCurrentLimitedPQ`)
- **Deferred by:** `R4133_PROPS_PLAN.md` §1.3 + **RP1.3** (2026-08-23), the sub-step that ported
  WindGen `UserModel`/`UserData` and admitted model **6**. §1.3 names this file as the row's home
  and no other plan claims the two models. That plan completed 2026-09-04 and is frozen at
  `docs/plans-archive/R4133_PROPS_PLAN.md`; its §RP5.2 re-verified this row as the one §1.3
  deferral that was owed one, and left it open — the row is now this file's alone.
- **What:** r4133 dispatches seven WindGen power-flow models
  (`Version8/Source/PCElements/WindGen.pas:2109-2118`); the port admits 1/2/4/5 and — since RP1.3 —
  6. Models 3 and 7 are simply absent. No corpus deck sets either (all five `modes:windgen/*` decks
  are `model=1`), and the enum refuses to parse them, so nothing degrades silently.
- **Spec.** Model 3 = `DoPVTypeGen` (`:1721-1768`), "constant P, constant |V|": a secant-style var
  search — `DQ := PVFactor * DQDV * (Vtarget − V_Avg)` clamped to `DeltaQMax` and then to
  `varMax`/`varMin`, with `DQDV`/`DQDVSaved` carried across solutions (`:931`, `:1412`, `:2348`),
  `DeltaQMax := (varMax − varMin) * 0.10` (`:1413`) and `WindGenvars.Vtarget` from `Vpu` (`:1406-1408`);
  the `Edit` arm additionally forces `Solution.SolutionInitialized := FALSE` when a model-3 WindGen
  appears (`:682`). Model 7 = `DoCurrentLimitedPQ` (`:1901-1969`), constant PQ with a per-phase
  current limit below `Vminpu` (wye at `:1936-1937`, the two delta forms at `:1951-1958`), plus the
  dynamics-side `Model7LastAngle` (`:2548`, `:2589`, read by `CalcVthev_Dyn_Mod7` `:3056-3073`).
- **Current Rust:** `crates/dss-core/src/obj/dss_enum/registry/pc.rs` — the `WindGen: Model` enum
  lists `&[1, 2, 4, 5, 6]`, so `model=3`/`model=7` is rejected at parse with the enum's own message
  and the property keeps its previous value; the dispatch
  (`elements/pc/windgen/solve.rs::calc_gen_model_contribution`) has no arm for them and its `_` arm
  — the port of the Pascal `ELSE` at `:2117-2118` — is unreachable for those two codes. There is no
  `NOT_PORTED` marker to grep: the admission gate is the enum.
- **Trap for whoever ports model 7: do NOT transcribe `WindGen.pas`.** Its limit fields
  `PhaseCurrentLimit`/`Model7MaxPhaseCurr` (`:70-71`) are **never assigned** in that unit — the
  `If GenModel=7 …` initialiser that `generator.pas:1183-1187` runs at the end of
  `SetNominalGeneration` was dropped when the routine was cloned (`WindGen.pas:1335-1345` is the
  same `CASE GenModel` block without it) — so upstream's model 7 zero-limits every phase current.
  Report: `investigations/to_opendss/39-windgen-model7-uninitialized-current-limit.md` (local-only).
  Upstream bugs are never reproduced: a port computes the limit the way the Generator does.
- **To do:** port both models loop-for-loop (they are self-contained `DoXxxGen` routines over
  `Vterminal`/`InjCurrent`, the same shape as the five that exist), widen the enum, add the model-3
  `SolutionInitialized` side effect, and gate each with its own `modes:windgen/*` deck on the
  `r4133` channel (model 7 only after the limit fields are given the Generator's initialiser, since
  the oracle's own answer is the zero-injection bug). **Priority: low** — no deck upstream or here
  exercises them.

### 1.12 `TLine.MakeLike` copies the impedance-*source* state upstream does not
- **Deferred by:** `R4133_PROPS_PLAN.md` §RP3.6 (2026-08-29) and its audit settlement. RP3.6 listed
  "`MakeLike`'s copy set" among the divergences "recorded with owners"; the audit observed that no
  plan or section was ever named, so the row lands here. It is `line.spacing`/`line.geometry` /
  class-wide `MakeLike` territory, not `line.linecode` — RP3.6 owns only the flag.
- **What (measured, r4133 DLL 11.0.0.1, 2026-08-29):** `New Line.cp like=live` on a line that names
  a LineCode answers `? Line.cp.linecode` = `''` on r4133 **and** on the pinned 0.14.5 oracle, while
  `? Line.cp.r1` = `'0.1'` — the impedances travel, the source does not. The port answers the code
  name. Same on a switched coded line (`like=sw` → `''` upstream, the name here).
- **Spec.** `TLine.MakeLike` (r4133 `Version8/Source/PDElements/Line.pas:735-787`) copies `Z`, `Yc`,
  `R1..C0`, `Len`, `SymComponentsModel`, `FCapSpecified`, then `ClassMakeLike` and the whole
  `FPropertyValue[]` array — and **nothing else**: not `CondCode`, not `FLineCodeSpecified`, not
  `FLineGeometryObj`/`GeometryCode`, not `FLineSpacingObj`/`SpacingCode`/`FLineWireData`/
  `FPhaseChoice`, not `LengthUnits`/`FUnitsConvert`/`FLineCodeUnits`. dss_capi 0.14.5
  (`src/PDElements/Line.pas:889-930`) copies the same short list.
- **Current Rust:** `crates/dss-core/src/elements/pd/line/accessors.rs::make_like` copies the full
  impedance-source state — `line_code_ref`/`line_code_name`/`line_code_specified`,
  `geometry_obj`/`geometry_name`, `line_spacing_obj`/`spacing_specified`/`line_wire_data`/
  `fphase_choice`, plus `length_units`/`user_length_units`/`line_code_units`/`units_convert`. Held
  (as a divergence lock, labelled as such) by the last assertion of
  `exec::tests::line_fetch::linecode_name_survives_the_flag_that_gates_its_render`.
- **Why it is not a one-liner:** narrowing it has **numeric** reach. `units_convert`/`length_units`
  feed `FUnitsConvert`, i.e. the `r1..c0` getters and `LengthMultiplier` in `CalcYPrim`; dropping
  `geometry_obj`/`line_spacing_obj` moves the copy off the Carson path onto the copied `Z`/`Yc`
  matrices, which is upstream's behaviour but a different code path here. It wants one sweep over
  every class's `make_like` against its Pascal counterpart, not a Line-only patch.
- **Blast radius today: zero cells.** Swept `tests/corpus`, `tools/golden`, `tests/golden` and
  `crates/dss-core/tests` for `New`/`Edit Line.` commands carrying a `like=` (continuations folded
  in): **0 hits** — no deck exercises `TLine.MakeLike` at all, which is why `line.linecode`'s whole
  census is 5 switch-shaped cells.
  **Priority: low**, but it is a real upstream divergence, not a stylistic one.

### 1.13 Ref-snapshot staleness: a control's render and drive bound follow a frozen phase count
- **Deferred by:** `R4133_PROPS_PLAN.md` §RP3.7 and its verify-A1 settlement (2026-09-02,
  finding F8). RP3.7 fixed the one reachable instance it owned (`MakePosSequence` now refreshes
  `ctrl_snap` on both SwtControl and Relay); the general gap is architectural and has no owner.
- **What (measured on BOTH engines, 2026-09-02, r4133 DLL 11.0.0.1 vs the port):** with a
  SwtControl wired to a 3-phase `line.swk`, `edit line.swk phases=1 bus1=src.1 bus2=ld.1` makes
  r4133 answer `? swtcontrol.sw1.state` = `[closed, ]` immediately, while the port keeps
  `[closed, closed, closed, ]` — through further unrelated edits of the control — until the ref is
  re-resolved by a `switchedobj=` write. The same shape exists on Relay, and on Relay's
  `mon_snap` it is *numeric*, not just a render: `recalc` re-reads the frozen monitored snapshot,
  so an `edit relay.x …` after `makeposseq` restores the pre-pos-seq `Nphases` and with it
  `vbase` / `PickupVolts47`.
- **Spec.** r4133 reads the live object at every use: `SwtControl.pas:591`/`:602`
  (`GetPropertyValue` 6/7 loop `ControlledElement.NPhases`), `:346` (`RecalcElementData`), `:633`
  (`Reset`); `Relay.pas:1407-1428` (getters 39/40), `:1318` (`Sample`), `:965`
  (`RecalcElementData`), `:1454` (`Reset`).
- **Current Rust:** the port's classes hold ref **snapshots** by design — an object cannot read
  another object — so `swt_control::SwtControl::state_size()` and `relay::Relay::state_size()`
  read `ctrl_snap.nphases`, refreshed only at `switchedobj=`/`monitoredobj=` resolution and (since
  RP3.7) in `make_pos_sequence`. The bound arithmetic itself is pinned
  (`recalc_redrives_after_a_phase_count_change`, `the_render_bound_follows_makeposseq`).
- **Why it is not a one-liner:** a live count needs an engine-level snapshot-refresh hook — every
  ref-holding class has the same gap, not only these two — which is a `dss-core` object-model
  change, not a control-class patch.
- **Blast radius today: zero cells.** No corpus deck edits a controlled or monitored element's
  phase count after wiring its control (swept for RP3.7). **Priority: low**, but it is a real
  divergence from the authority engine, not a modelling preference.

### 1.14 The per-phase state seam RP3.7 landed for SwtControl and Relay, and its untouched siblings
- **Deferred by:** `R4133_PROPS_PLAN.md` §RP3.7 (2026-09-02). RP3.7 owns `swtcontrol.normal`/
  `.state` and `relay.normal`/`.state` only; no plan sub-step owns the Recloser or Fuse pairs
  (checked against §RP3.8, §RP3.9, §RP3.10, §RP3.11 and §RP4.1), and the Relay items below
  were deliberately not landed on a hunch — except the NIL render, which the RP3.7 audit
  settlement measured on the DLL and landed the same day (see (c)).
- **What.** (a) **Recloser carries both defects RP3.7 removed from Relay**
  (`crates/dss-core/src/elements/control/recloser/accessors.rs:334-362`): the
  `values.len() == 1 → ganged` heuristic, which reads a *quoted* single token as a ganged write
  where r4133 writes phase 1 only, and a whole-object `if self.f_locked { return; }` that refuses
  `Normal` as well as `State`, where `Recloser.pas`'s guard is the same name-based `('a'|'s')`
  rule the SwtControl/Relay interpreters use. It has no raw write hook and no five-token cap.
  (b) **Fuse has no ganged path at all** (`elements/pd/fuse/accessors.rs:205-214`): it writes only
  the leading `values.len()` slots, so a bare `fuse … state=open` sets phase 1 alone where
  `Fuse.pas:569-597` — the one interpreter of the three that *has* the `Else Begin` — fills every
  slot. (c) **The NIL-`ControlledElement` guards, minus the render.** The **render** half is
  **CLOSED** by the RP3.7 audit settlement (2026-09-02): `Relay::render_size` carries r4133's
  own getter-local guard (`Relay.pas:1407`/`:1418`), so `Normal`/`State` answer `'[]'` on a relay
  whose `SwitchedObj` never resolved, exactly as the DLL does (measured on 11.0.0.1; pinned by
  `relay::tests::nil_controlled_element_renders_the_empty_array`). It is scoped to the two render
  accessors on purpose — the deferral's real subject was never the render but the other twelve
  `state_size` call sites. **What is left** is the behavioral half: r4133 guards `Reset`
  (`:1447` `If not Locked and (ControlledElement <> NIL)`) and `set_States` (`:1494`/`:1514`) on
  the same pointer, so with no controlled element it restores **nothing**, while the port's
  `reset_action`/`do_reset_action` run their state-array loop regardless (`Sample` is not a
  question: r4133's `:1071` `WITH ControlledElement Do` faults outright, so there is no defined
  observable to port). Unprobed, and the object it needs cannot exist in a corpus deck without a
  #387 error first. (d) **The same NIL guard is unported on Recloser (`Recloser.pas:1377`/`:1388`)
  and Fuse (`Fuse.pas:690`/`:701`)** — both render their own phase count where r4133 renders
  `'[]'`; they belong with those classes' interpreter ports in (a)/(b), not with Relay's. (e) **`set_States`' `ArmedForReset := FALSE`** (`Relay.pas:1509-1540`) is
  not reproduced: the port drives the element from `recalc` (r4133's `RecalcElementData:965-980`,
  which clears `ArmedForOpen`/`ArmedForClose` but not `ArmedForReset`); observable only for an
  `edit` issued mid-simulation with a reset armed.
- **How to do it:** the RP3.7 pattern, per class — probe the authority DLL first (a quoted single
  token, a bare token, a locked `normal=`, a 6-token list on a 6-phase element), then port
  `InterpretFuseState`/the Recloser interpreter behind a `set_enum_array_raw` hook with the
  `WasQuoted` reconstruction, and pin the measured bytes.
- **Blast radius today: zero cells** on either channel (the census pairs for these classes carry
  no `Normal`/`State` divergence, and Recloser/Relay are whole-element-skipped on the capi
  channel). **Priority: low-medium** — the defects are real write-semantics divergences that a
  deck could hit the day it writes a quoted list.

### 1.15 `TSwtControlObj.RecalcElementData`'s terminal validation
- **Deferred by:** `R4133_PROPS_PLAN.md` §RP3.7 (2026-09-02) — pre-existing omissions outside the
  sub-step's line ranges, recorded rather than fixed.
- **What.** r4133 raises `DoErrorMsg` **384** when the control's `ElementTerminal` exceeds the
  controlled element's `NTerms` (`Version8/Source/Controls/SwtControl.pas:335-341`) and warns when
  `FNphases > SWTCONTROLMAXDIM` (`:334`). The port emits the sibling **387** for a missing element
  but silently clamps the terminal index instead of erroring — a genuine validation gap, i.e. a
  deck typing `switchedterm=3` on a two-terminal line is diagnosed upstream and not here.
- **Not part of it:** `HasSwtControl := TRUE` (`:344`) is dead upstream state (declared
  `CktElement.pas:99`, initialized `:213`, set here, read nowhere), so it is deliberately **not**
  ported and needs no row of its own.
- **Blast radius today: zero cells** — no corpus deck mis-types the terminal. **Priority: low**;
  it is one guarded error, but validation gaps are exactly what silently degrades a port.

### 1.16 SwtControl still runs the retired 0.14.5 `Sample`/`DoPendingAction` control-queue glue
- **Deferred by:** `R4133_PROPS_PLAN.md` §RP3.7 and its verify-A1 settlement (2026-09-02,
  finding F2), registered here by the RP3.7 audit settlement the same day. Pre-existing (the
  scalar model armed identically), **not** an RP3.7 regression — but it is the only RP3.7 item
  that moves a **solved** result, so it gets a row of its own rather than living in a test doc.
- **What (measured on BOTH engines, 2026-09-02, r4133 DLL 11.0.0.1 vs the port):** r4133 comments
  out both `TSwtControlObj.Sample` and `DoPendingAction`
  (`Version8/Source/Controls/SwtControl.pas:396-408`, `:484-507`) — a SwtControl queues nothing
  and operates nothing; the switch moves only at parse time, from `set_States`/
  `RecalcElementData`. The port keeps the 0.14.5 bodies, and 0.14.5 maps all three state
  properties onto one `CurrentAction` field, so `side_effects(NORMAL)` leaves
  `current_action = Open` against an all-CLOSED `present_state` — `Sample`'s arming condition.
  Consequence: after `edit swtcontrol.sw normal=open` the port OPENS the switch three duty steps
  later (`Action=OPENED` in the event log) where the DLL leaves it closed for the whole run and
  logs nothing. The **locked** write arms it too since RP3.7 (a2) applies a locked `normal=` per
  `:416-417`; there the switch stays shut (`do_pending_action` is `!locked`-guarded) but the
  queue push and the `armed` latch still happen.
- **Current Rust:** `elements/control/swt_control/mod.rs::sample` / `::do_pending_action`, held
  by the delete-don't-re-baseline tripwire
  `swt_control::tests::sample_arms_on_a_normal_write_the_retained_capi_channel` (both the
  unlocked and the locked arm).
- **Why it is not a one-liner — the blocker, named.** `controls/swtcontrol/swtcontrol_lock.dss`
  is gated `engines: "capi_v0145"` with `compare_ctrlqueue`, so the capi lane **pins** the
  spurious `CTRL_LOCK` push this body makes. Retiring the body means re-gating that deck onto
  `r4133`, which gives up the only capi deck that both probes and property-compares a SwtControl
  and retires one of the five ledger entries RP3.7 landed. That is a channel decision, and RP3.7
  **chose not to take it** — not a mechanical limit (the sub-step edited the manifest lock and
  the ledger for other reasons). The capi015 props golden's `Action` readback is **not** a second
  blocker: it reads `current_action` via `get_i32(ACTION)`, maintained by the property side
  effects, which neither `Sample` nor `DoPendingAction` writes.
- **Blast radius today: zero cells.** Every corpus `normal=` is a ganged `normal=closed` over an
  all-closed state (swept for RP3.7 over all eight SwtControl decks: `midi_swtcontrol.dss:123`,
  `swtcontrol_lock.dss:16`, `swtcontrol_time.dss:16`), and `swtcontrol_lock.dss` types
  `normal=closed` **before** `lock=yes` on the same `New`. **Priority: medium** — zero exposure
  today, but it is a live divergence from the behavioral authority that no gate can fail on.

### 1.17 `batchedit … where <prop> > x` reads the render cache, not the live value
- **Deferred by:** `R4133_PROPS_PLAN.md` §RP3.8 (2026-09-02) and its audit settlement. RP3.8 owns
  the five live-render surfaces themselves, not the **fifth** reader of `ClassProps::get_value`;
  no plan sub-step owns that reader (checked against §RP3.9, §RP3.10, §RP3.11 and §RP4.1).
- **What.** RP3.8 made five read-only properties render a **live** result
  (`PropFlags::RENDERS_LIVE_RESULT`: `indmach012.pf` and the four StorageController fleet
  aggregates). There are **five** `get_value` readers in `src/`, and four of them refresh the
  cache first: `?` (`exec/command.rs`), `Dump` (`exec/report.rs` → `report/save/dump.rs`) and
  `Dss::element_properties` (`exec/view.rs`) one object at a time at the choke point
  `Dss::refresh_vterminal_if_marked`, and `Save` (`report/save/save.rs`, reached from
  `exec/save_circuit.rs` and `exec/report.rs::write_class_file`) in one up-front pass over the
  store, `Dss::refresh_render_caches_for_save`. The fifth, the `where <prop> <op> <x>` filter of
  `batchedit` (`crates/dss-core/src/exec/batchedit.rs:259-270`), is `&self` and cannot: it reads
  whatever the cache last held (the construction value until some other surface has rendered).
  r4133 evaluates the live getter there, so a `batchedit storagecontroller..* where kWhActual >
  1000 …` can select a different set on the two engines.
- **The `Save` half of this row was a real defect and is fixed, not deferred** (audit settlement,
  2026-09-02): `Save` emits every property a deck explicitly set, and a write to a read-only
  property is silently ignored *yet still marks the property set*, so the serializer really did
  reach these caches — writing `PF=1` / `kWhTotal=0` by default and the live number only when an
  earlier `?` had refreshed it (r4133, measured: `pf=0.886059`, `kWhTotal=6000`). The same latency
  pre-dated RP3.8 on `READS_VTERMINAL` (`Transformer.WdgCurrents` saved as an all-zero buffer
  where r4133 saves the solved currents) and is fixed by the same pass. Pinned by
  `exec::tests::report::save_renders_the_live_result_properties`; no corpus deck or golden writes
  any of the six properties, so no committed byte moved (swept 2026-09-02). RegControl `TapNum` is
  the one marked reader that was already correct on `Save` (its `tap_snap` is resynced by the
  control action itself — probed: saved `TapNum=6`, the live tap).
- **Blast radius today: zero cells.** No committed golden and none of the 523 corpus cases
  filters on any of the marked properties (swept for RP3.8 over the whole population; the
  `claims` census produces no cell through that path).
- **Why it is not a one-liner:** the filter would have to run the same choke point, which needs
  `&mut self` plumbing through `batchedit`'s selection loop — an executive-level signature change,
  not a property-class patch. **Priority: low.**

### 1.18 `Dss::element_variables` on an IndMach012 advances the slip-Newton (a read that mutates)
- **Deferred by:** `R4133_PROPS_PLAN.md` §RP3.8 (2026-09-02), which fixed the same hazard on the
  property render and recorded this sibling surface. Pre-existing, **not** an RP3.8 regression.
- **What (measured on both engines, 2026-09-02).** IndMach012 state variable #21 ("Power Factor")
  computes `power_factor(terminal_power(…))` **on `self`**, so reading the variables while the
  `Iterminal` cache is unstamped runs `CalcPFlow` and advances the machine's fixed-slope
  slip-Newton by one step — the following solve then starts from a different slip (measured on
  `asymmetric/indmach/indmach_asym.dss`: step-0 `pf` `0.908916` with no pre-solve read,
  `0.908755` after one variables read). It mirrors r4133's own
  `Get_Variable` → `Get_Power` → `ComputeIterminal` (`PCElements/IndMach012.pas:1988`,
  `Common/CktElement.pas:666-703`), i.e. the port reproduces an upstream read-that-mutates of the
  `VSConverter.GetCurrents` family here, which the 2026-08-02 policy says it should not.
- **How RP3.8 solved it one surface over:** `IndMach012::refresh_live_pf` runs the recompute on a
  throwaway `self.clone()` and keeps only the resulting `f64`
  (`elements/pc/ind_mach012/accessors.rs`), pinned by
  `ind_mach012::tests::pf_is_a_pure_read`. The same shape would fix the variables path.
- **Blast radius today: zero cells.** No golden and no corpus case reads an IndMach012's variables
  before its first solve (the gate's own schedule reads every observable after a solve, where the
  cache is stamped and neither engine recomputes). **Priority: low** — but it is a live
  read-that-mutates in the port, so it belongs to whoever owns the variables surface.

### 1.19 `Export SeqVoltages` still substitutes ground for a phase the bus does not carry
- **Deferred by:** `GOLDEN_REBASE_PLAN.md` §G1.4c audit settlement (2026-09-05, finding AC-2) —
  the API surface stopped reproducing the defect in that sub-step; the report path was outside it.
- **What.** `report/export/seq_voltages.rs:39-41` reads `node_v[bus.find(k)]` for `k = 1, 2, 3`,
  and `Bus::find` returns `0` (= ground) for a node the bus does not carry — upstream's own
  `Vph[j] := NodeV^[GetRef(FindIdx(j))]` conflation (r4133 `Common/ExportResults.pas:183`, the same
  defect as `DDLL/DBus.pas:305` == `CAPI/CAPI_Alt.pas:2190`, reported upstream as
  `investigations/to_opendss/66-bus-seqvoltages-node-count-and-ground-substitution.md`). On a bus
  with >= 3 nodes and a phase missing (e.g. `[1, 2, 10]`) the export therefore publishes a V012
  built on a fabricated 0 V phase. `report/show/voltages.rs:73-75` shares the read; its L-L half
  (`:191-193`) does **not** share the pairing defect — it wraps first, r4133's own correct order.
- **Why it is still here.** Not golden bytes: measured 2026-09-05, every voltage-report golden runs
  `IEEETestCases/13Bus/IEEE13Nodeckt.dss`, whose bus specs carry only node numbers 1-3, so no bus
  there reaches the substituting branch and fixing it would move **zero** golden bytes. What is
  missing is the *decision* — what an export column set should print where symmetrical components
  do not exist (the API surface answers "unavailable"; a CSV row has no such convention) — which is
  report semantics, i.e. WP-G4/G5 territory, not WP-G1's.
- **Blast radius today: zero golden cells and zero live cells** (no oracle channel compares
  `Export SeqVoltages` text in the corpus gate). **Priority: low**, but it is a knowingly retained
  upstream defect in shipped product code, which the 2026-08-02 policy does not allow to stay
  unowned.

### 1.20 `RelCalc` zone-boundary semantics — what `Bus.TotalMiles` and its siblings mean on a nested head bus
- **Deferred by:** `GOLDEN_REBASE_PLAN.md` G1.6(i) (audit settlement AT-1, 2026-09-05) and handed on
  by G1.6(ii) (2026-09-05), whose gated population holds no witness: its only two-meter deck
  (`tests/corpus/controls/energymeter/midi_energymeter.dss`) aborts at 52902 before a section exists,
  and the four `DOCTechNote` decks that would supply one are out of the population.
- **What (measured on both oracles).** `DoLambdaCalcs` zeroes only `BusFltRate` /
  `Bus_Num_Interrupt` circuit-wide (r4133 `Executive/ExecHelper.pas:4432-4437`), while
  `BusTotalMiles` and its siblings are zeroed **per meter**, on the FROM bus of that meter's
  `SequenceList` (`Meters/EnergyMeter.pas:2471-2472` → `PDElements/PDElement.pas:313-327`), and read
  on the TO bus (`:105-111`). A bus on a zone boundary is therefore zeroed by the inner meter and
  read by the outer one: for a nested pair `Bus.TotalMiles(src)` walks `2.0 → 3.0 → 3.0` when the
  outer meter is declared first, and is `3.0` from run 1 when the inner one is. Both oracles do the
  same, so the port mirrors them; reported upstream as
  `investigations/to_opendss/61-relcalc-cross-zone-accumulator-leak.md` and frozen by the G1.6(i)
  pin with its run-count and declaration-order arms.
- **What is open** is not the pin but the *correct* value: circuit-wide zeroing would fix
  idempotence, not order-independence, and nothing has decided whether a nested head bus should read
  the outer zone's miles (`3.0`) or its own (`2.0`). Needs a deliberate semantics decision **plus a
  witness deck** — a two-meter nested feeder in the corpus, gated on both channels — before any
  engine change. **Priority: low**: no gated case reaches it today, and R-14(d) kept G1.6(i)/(ii) out
  of `solution/meters/reliability.rs`.

### 1.21 The `AllPCEatBus` / `AllPDEatBus` **executive commands** have a name and help text but no dispatch
- **Deferred by:** `GOLDEN_REBASE_PLAN.md` §G1.4d (2026-09-06, coordinator decision **D34**) — that
  sub-step ported the *API* surface (`Bus.AllPCEatBus`/`AllPDEatBus`, live on both oracle channels)
  and did not touch the command registry, so D34's "port it if the engine part touches the same
  registry, else record it with an owner" resolves to this entry.
- **What.** `EXEC_COMMANDS` carries both names (`crates/dss-core/src/exec/tables.rs:129-130`) and the
  help catalog carries their upstream help strings
  (`crates/dss-core/src/report/help_catalog.rs:515-519`), but `exec/command.rs` has no dispatch arm,
  so both fall through to `not_ported_command` (`:353` → `:425`). Upstream they are
  `TDSSCircuit.ReportPCEatBus` / `ReportPDEatBus`
  (r4133 `Version8/Source/Common/Circuit.pas:1586-1600` / `:1601-1615`), which format the SAME two
  lists this sub-step now computes into `GlobalResult` as a comma-separated string.
- **What is already there.** The lists themselves exist and are gated:
  `Dss::all_bus_elements` / `Dss::bus_elements` → `BusElementsView`
  (`crates/dss-core/src/exec/view.rs`), compared live on both channels by
  `harness::compare_bus_at_bus`. Only the *text* surface is missing, so the work is a formatter plus
  a dispatch arm, not a walk.
- **A second consumer, not just the command.** r4133 calls `ReportPDEatBus` from its own A-Diakoptics
  zone setup to find the feeder-head branch (`Common/Circuit.pas:1683`, `'New EnergyMeter.myEMZoneFH
  element=' + first entry`), so the ordering of that string is observable there too — a port of the
  command should keep the class-walk order upstream emits rather than the port's creation order.
- **Why it is still here.** No corpus deck issues either command, and WP-G1 gates API surfaces, not
  report text; wiring it inside G1.4d would have shipped an unwitnessed feature. **Priority: low.**
  Whoever ports it owes a `modes:` micro deck that issues both commands, and must decide the empty
  answer's spelling (upstream prints the `None` seed).
- **Owner (D34, settled at the G1.4d audit settlement 2026-09-06).** D34 asked for this entry to be
  recorded "with an owner", but no live plan covers unported *executive commands* today, so §2 —
  "a live plan tracks them" — would be a false claim (checked: GOLDEN_REBASE WP-G3/G4/G5 are goldens
  and docs, UPGRADE_PLAN is properties/semantics, WASM_USERMODELS is user models). It is therefore
  **scheduled instead of owned**: `GOLDEN_REBASE_PLAN.md` §G5.2 (the closing record, which already
  owes the same "add a named WP row … and record it in `ORPHANED_GAPS.md` until that row exists"
  treatment for the model-6 `FInit` deferral, plan §1.3) either names a WP row for these two commands
  or ratifies this entry as a standing orphan. Until G5.2 rules, this bullet is the record.

---

## 2. Owned deferrals — NOT orphans (a live plan tracks them; do not re-port here)

| Deferred item | Owner plan (status) |
|---|---|
| `TODO(compat)` wipe (×123 workspace-wide / ×117 in `crates/dss-core/src`, re-measured 2026-07-26) → the oracle-parity feature split | **DE_PASCALIZE Stage F** (**COMPLETE 2026-07-31**, branch `depas-stagef`: 117 → **18**, every survivor a registered escape owned by a named successor and gated by a test — see the three rows below and `DE_PASCALIZE_PLAN.md` §"Stage F as executed") |
| **`HIDE_015X` retirement** — un-hide the 5 Line/LineGeometry props, drop the `Line.Wires → "Conductors"` `json_name` masquerade, regenerate the 13 gated artifacts (8 `Dump` + 5 JSON), delete the flag: **one atomic change, both lanes** | **UPGRADE_PLAN §5** (open tail of the UPGRADE line; §5's "`rg HIDE_015X` must be empty" was not met at the Rung 1/2 exit). NOT Stage F's: it moves *parity-lane* byte goldens, which only an oracle-surface switch may do (`gen_json.py` is hard-pinned to 0.14.5). Measured in Stage F F.3aa; re-homed in F.3ag + the F.3 close. Tripwires: `oracle_parity_cfg_gate.rs::the_hide_flag_escape_population_is_pinned_by_surface` (13 artifacts) and `exec::tests::compat_quirks::hide_015x_carrier_set_is_the_measured_escape` (5 carriers) — a partial touch fails the gate. Disposition: `docs/upgrade/DIVERGENCES.md` §"Line/LineGeometry Conductors" |
| **`HIDE_R4133` un-hide** — the 4 r4133-only props (Generator `Rneut`/`Xneut`, Sensor `Action` — the RP1.1 stubs; AutoTrans `XfmrCode` — RP1.2's real port) are deferred from the 0.14.5-pinned full-enumeration surfaces. Un-hiding moves **8** committed artifacts (`json/der_usermodel_assigned.json`, `json/der_usermodel_full.json`, `json/dyneq_full.json`, `json/autotrans_micro.json`, `json/autotrans_solved.json`, `reports/dump_autotrans.txt` +1 row, `reports/dump_autotrans3.txt` +1 row, `reports/dump3_commands.txt` +7 rows) plus `json/schema_full_port.json` and the deletion of the 4 `port_hidden_property` rows; **no corpus case** moves (re-measured 2026-08-23 with the fourth carrier by disabling the flag's arm and running `cargo test -p dss-core --no-fail-fast`) | **GOLDEN_REBASE G3.3c** (`dump*`/`dump3*` self-snapshot) + **G3.4** (`json/`) — the sub-steps that stop pinning those surfaces to 0.14.5; until then the rows must stay hidden (R4133_PROPS_PLAN §1.2, "no other golden byte moves in this plan"). The schema half never unblocks there: `json/schema_full_oracle.json` + `schema_divergences.json` stay frozen (GOLDEN_REBASE §1.2), so the four `port_hidden_property` rows outlive the un-hide. Tripwire: `exec::tests::compat_quirks::hide_015x_carrier_set_is_the_measured_escape` pins the carrier list and the two hide flags' class-disjointness |
| The **1** surviving `WholeCase` compat marker: the Generator user-model `E1` seeding (`elements/pc/generator/user_model.rs`, the Model=6 dynamics-entry seed on `wasm_gen_dyn`) | **WASM_USERMODELS** — GOLDEN_REBASE_PLAN.md §1.3 defers it there explicitly (it needs its own decision about diverging from an r4133-anchored dynamics trajectory); G5.2 must add the named WP row. Until that row exists this line is the record. The other three of the original four — GICTransformer `%R2`, Capacitor `Cs − Cm` posseq write, LoadShape MMF accept-set — were **closed by GOLDEN_REBASE G2.5** (2026-08-07): fixed in both lanes, each paid for by a `tests/corpus/ledger.json` entry (`gic-pct-r2-honoured-*`, `makeposseq-cuf-applied-capi*`, `mmf-accept-set-honoured-capi`) plus an expected-value pin, registered in `oracle_parity_cfg_gate.rs::TORN_DOWN_ROWS`. So the "unowned policy call" is settled: the owner granted the coverage trade, and `EXIT_POPULATION[WholeCase]` is 4 → 1 |
| The 3 `WasmGuest` compat markers (truncated `sqrt(3)/2`, `1.732`, FPC single-precision `3.0/746.0` in the reference user model) | **WASM_USERMODELS** — the model is a workspace-excluded crate `dss-core/oracle-parity` cannot reach; a lane split there is a second `.wasm` fixture, not a cfg alias |
| A-Diakoptics `AggregateProfiles` + D9(d) r3723 AD-replay + WP-AD.6 threaded children | **DIAKOPTICS_PSTCALC Part II** (partial) |
| User-model native DLLs (Gen/PVSystem/Storage/CapControl `UserModel`) | **WASM_USERMODELS** (**COMPLETE 2026-07-25** — WM.0–WM.7 merged; DLLs re-homed to wasmi-sandboxed models, r4133 oracle-of-record) |
| Actor / parallel-machine (`DSS_CAPI_PM`) mode | **MULTITHREADING M2** (not started) |
| Near-singular / floating-delta tolerance retighten (SubXFMR, GFM common-mode) | **RESONANCE WP-R1/R2** (not started) |

> **What "Stage F" was** (referenced above): the final stage of `DE_PASCALIZE_PLAN.md`
> (Part IV.2, "oracle-parity feature split"). It (1) swept the `TODO(compat)` shims,
> (2) split the engine with a cargo feature `oracle-parity` into a **default** lane
> (clean numerics — true π, honest `Round`, upstream bugs fixed, native report
> rendering) and an **oracle-parity** lane (keeps 1:1 bug-for-bug behavior so the
> byte-exact golden gates stay green forever), and (3) stood up the two gate lanes + the
> on-demand default↔parity differential job (`tools/lanes/lane_diff.ps1`). Parity target
> = r4133. **Completed 2026-07-31**; its exit metric is *zero unclassified markers, zero
> carriers beyond the pinned escape, every escape gated by a test* — the literal "0
> markers" was ruled unreachable inside the stage's sanctioned scope, by measurement, at
> the F.3 close. It was the prerequisite for RESONANCE WP-R1 and MULTITHREADING M3c,
> which are now unblocked.

## 3. Permanent by-design non-ports (do NOT port)

- **DI-plot family** (`DI_plot`/`CompareCases`/`YearlyCurves`) — upstream calls the plot
  callback with no NIL guard = UB; deliberately `NOT_PORTED`.
- **`FireOffEditor` auto-open, `DOScmd`, TOP/`DI_plot` external-tool launches, native
  user-model DLL loading** — outside `#![forbid(unsafe_code)]` safe Rust (DLL loading is
  re-homed to WASM_USERMODELS; the rest stay never-port).
