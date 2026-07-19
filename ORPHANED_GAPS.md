# ORPHANED_GAPS — deferred work no plan will pick up

**Purpose.** dss-rs was built as a *chain* of plans, each deferring some work to a
named successor. Most deferrals have a live owner (see §2). The items in **§1** were
handed to a successor that **does not exist** or **will never reach them** — they are
**orphans**: real, self-contained work that falls through the cracks unless someone
deliberately schedules it. A fresh agent can pick up any §1 item standalone.

Audited 2026-07-18 against the tree (branch `update`). Everything here is **outside**
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
across oracle processes) — not reproduced. Two out-of-scope blockers to an
AutoTrans Full golden and a Generator/Storage Full golden are recorded in STATUS
Standing follow-ups (AutoTrans JSON array-alt metadata; NOT_PORTED ShaftModel).

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
- **Deferred by:** UPGRADE_PLAN (parked to "a future rung" that has no plan).
- **What:** the port's NCIM matches its `capi015` oracle loop-for-loop **including non-convergence** (both stall at byte-identical voltages, 100 iters); EPRI r4088/r4133 converge in 2 iters via a newer NCIM PV→PQ switching cadence the port has not adopted.
- **Spec:** the r4133 `Common/NCIMSolutionHelper.pas` diff vs the ported `r4103` version (needs the newer vendored source).
- **Current state:** parked in `tests/corpus/manifests/skipped_needs_investigation.json` (tag `ncim_pv_pq_switching_divergence`); report-only in `docs/upgrade/DIVERGENCES.md`.
- **To do:** port the newer cadence, then promote `IEEE118Bus`. **This is genuinely a new UPGRADE rung** (adopting a behavior *past* r4133-as-shipped). **Priority: low**, and note it moves the parity target.

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
- **Deferred by:** UNIFIED_GATE Phase D; **worked by the post-audit fix round (STATUS §1l, 2026-07-19).** Of the 4 owned here, **1 retired, 3 confirmed NOT ledgerable** (R3 — re-measured live).
- **RETIRED — `asymmetric line_spacing_asym`** → `engines:"both"`: capi_v0145 exact-pair-NUMERIC property+probe entries (Line.lsp.normamps 730→230, emergamps 1095→345; the min-over-phase D3 upgrade, port CORRECT) + r4133 `#303` skip. This required the exact-pair-numeric probe/property machinery §1i said was lacking — it was ADDED this round (num_rel absent + oracle/rust pins ⇒ exact float pin). Line.lspc is D3-invariant (165/247.5, no divergence). Done.
- **STAYS (R3, suspected port bug) — `controls regcontrol_idle`:** the earlier "~7e-5 regulator-tap" premise was WRONG. Live r4133 (2026-07-19) settles the regulated bus **MV.1 ~8.7% (623 V)** off the port — a WHOLESALE divergence, not a sub-tap ulp class. Per R3 this may NOT be ledgered (an 8.7% gap smells like a port idle-RegControl bug, e.g. tap initialization). **Needs a WP** to investigate the `idle` regulator; kept `defer_ledger`.
- **STAYS (R3, matches neither oracle) — `Dynamic_KundurDynExp` + `GFL_IEEE123 …DynExp`:** re-measured — the port matches NEITHER surviving oracle (pinned 0.14.5 AND EPRI r4133 agree with each other; the port differs from BOTH by ~1.5e-5 rel). Force-ledgering would pin a port-side DynExp-evaluator bug. **Needs a WP** on the DynExp evaluator; kept `defer_ledger`.
- **NCIM×4** (not here) — owned by **WP-U1.7** (suspected op-point port issue; must not be ledgered before re-validation, R3).
- **defer_ledger remaining after this round: 7** (regcontrol_idle + DynExp×2 + NCIM×4). **Where:** STATUS §1l has the full evidence. **Priority: low** (the two open items are now WP-scoped investigations, not envelope measurements).

---

## 2. Owned deferrals — NOT orphans (a live plan tracks them; do not re-port here)

| Deferred item | Owner plan (status) |
|---|---|
| `TODO(compat)` ×123 wipe + `HIDE_015X` ×15 → the oracle-parity feature split | **DE_PASCALIZE Stage F** (paused after wave 1) |
| A-Diakoptics `AggregateProfiles` + D9(d) r3723 AD-replay + WP-AD.6 threaded children | **DIAKOPTICS_PSTCALC Part II** (partial) |
| User-model native DLLs (Gen/PVSystem/Storage/CapControl `UserModel`) | **WASM_USERMODELS** (not started) |
| Actor / parallel-machine (`DSS_CAPI_PM`) mode | **MULTITHREADING M2** (not started) |
| Near-singular / floating-delta tolerance retighten (SubXFMR, GFM common-mode) | **RESONANCE WP-R1/R2** (not started) |

> **What "Stage F" is** (referenced above): the final stage of `DE_PASCALIZE_PLAN.md`
> (Part IV.2, "oracle-parity feature split"). In one pass it (1) wipes all `TODO(compat)`
> shims, (2) splits the engine with a cargo feature `oracle-parity` into a **default**
> lane (clean numerics — true π, honest `Round`, …) and an **oracle-parity** lane (keeps
> 1:1 bug-for-bug behavior so the byte-exact golden gates stay green forever), and (3)
> stands up the two CI lanes + a default↔parity differential job. Parity target = r4133.
> It is NOT started (DE_PASCALIZE is paused) and is a prerequisite for RESONANCE WP-R1 and
> MULTITHREADING M3c.

## 3. Permanent by-design non-ports (do NOT port)

- **DI-plot family** (`DI_plot`/`CompareCases`/`YearlyCurves`) — upstream calls the plot
  callback with no NIL guard = UB; deliberately `NOT_PORTED`.
- **`FireOffEditor` auto-open, `DOScmd`, TOP/`DI_plot` external-tool launches, native
  user-model DLL loading** — outside `#![forbid(unsafe_code)]` safe Rust (DLL loading is
  re-homed to WASM_USERMODELS; the rest stay never-port).
