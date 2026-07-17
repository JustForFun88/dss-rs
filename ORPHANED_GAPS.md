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
- **Deferred by:** GAPS_PLAN WPG.16 (GIC elements were ported; only this export was punted to "Phase 9", which never materialized as a plan).
- **Spec:** `GICTransformer.pas` `WriteVarOutputRecord`; `ExportOptions.pas` `GICMvars` verb.
- **Current state:** `crates/dss-core/src/elements/pd/gic_transformer/mod.rs:118` (`// WriteVarOutputRecord (Export GICMvar, not ported here)`); the verb records a scoped `NOT_PORTED`, pinned by `crates/dss-core/src/exec/tests/report.rs` (asserts `export gicmvars` errors "not ported").
- **To do:** port `WriteVarOutputRecord` (per-GICTransformer Mvar/loss output), wire the `GICMvars` export verb, add a golden over a GIC deck, retire the `report.rs` negative-assert. **Priority: low** (niche; only GIC studies).

### 1.2 AltDSS JSON `DynInit` tail
- **Deferred by:** JSON_EXPORT_PLAN §6 ("DynInit JSON tail", explicitly scoped out).
- **Spec:** `CAPI_Obj.pas:752-759` — for a `TDynEqPCE` with `UserDynInit <> NIL`, append a `"DynInit"` object.
- **Current state:** `crates/dss-core/src/report/export/json/build.rs:123` (NOT_PORTED comment). Only fires for a Generator/PVSystem/Storage carrying a `UserDynInit` DynamicExp.
- **To do:** emit the DynInit tail in `obj_to_json_data`; add a golden with such a deck (a covered deck currently loud-mismatches, so it fails safe). **Priority: low.**

### 1.3 AltDSS JSON `Full`-mode solve-state matrices
- **Deferred by:** JSON_EXPORT_PLAN (discovered mid-implementation; `docs/phase-records/gaps.md:1030-1034`).
- **What:** under `Full`, Transformer/AutoTrans `WdgCurrents` + Capacitor `CMatrix` are excluded because the `&self` JSON path cannot run `refresh_vterminal_if_marked`.
- **Current state:** `crates/dss-core/src/exec/view.rs:381-382`; goldens mark those classes "Full excluded".
- **To do:** give the JSON-Full path a `&mut` (or pre-refreshed) route so those matrices render; extend the goldens. **Priority: low.**

### 1.4 AltDSS JSON **import** (`Obj_Circuit_FromJSON_`)
- **Deferred by:** JSON_EXPORT_PLAN §6 (out-of-scope sibling — export only was in scope).
- **Spec:** `CAPI_Obj.pas:2674-end` (whole-circuit JSON → object graph).
- **Current state:** not implemented; no import surface exists.
- **To do:** port the JSON reader (mirrors the §1.2-1.4 export renderer inversely). **Priority: medium** (the GUI the JSON export was built for may need round-trip). Sizeable WP.

### 1.5 `CAPI_Schema` JSON schema export
- **STATIC CORE PORTED 2026-07-18** on `og15-capi-schema` — see STATUS §OG-1.5.
  The schema envelope + the ten reusable global `$defs` + the static
  `circuitProperties` head are ported byte-exact vs the oracle
  (`report/export/json/schema.rs`, `Dss::extract_schema_json`, golden
  `tests/golden/json/schema_static_core.json` via `tools/golden/gen_schema.py`).
- **Still orphaned (the bulk):** the per-class walk (`prepareClassJsonSchema`) +
  per-enum walk (`prepareEnumJsonSchema`) — the **49 class + 21 global enum
  `$defs`** (plus each class's `List`/`Container` defs and `circuitProperties`
  ref). These are blocked on per-property metadata the Rust port never carried, a
  large self-contained data-entry effort: property **help/description** text
  (`GetPropertyHelp`, ~1109 strings), per-class **`AltPropertyOrder`**
  (`$dssPropertyOrder`), **`SpecSets`** (`oneOf`), enum
  **`AltNames`/`JSONName`/`JSONUseNumbers`**, and ~28 of ~30 `Units_*` `PropFlags`
  (only `UNITS_HOUR`/`UNITS_OHM_PER_LENGTH` exist). Oracle is reachable
  (`lib.DSS_ExtractSchema`) — the blocker is Rust-side metadata, not access.
  **Priority: low.**

### 1.6 IEEE118Bus NCIM `PV→PQ` r4133 switching cadence
- **Deferred by:** UPGRADE_PLAN (parked to "a future rung" that has no plan).
- **What:** the port's NCIM matches its `capi015` oracle loop-for-loop **including non-convergence** (both stall at byte-identical voltages, 100 iters); EPRI r4088/r4133 converge in 2 iters via a newer NCIM PV→PQ switching cadence the port has not adopted.
- **Spec:** the r4133 `Common/NCIMSolutionHelper.pas` diff vs the ported `r4103` version (needs the newer vendored source).
- **Current state:** parked in `tests/corpus/manifests/skipped_needs_investigation.json` (tag `ncim_pv_pq_switching_divergence`); report-only in `docs/upgrade/DIVERGENCES.md`.
- **To do:** port the newer cadence, then promote `IEEE118Bus`. **This is genuinely a new UPGRADE rung** (adopting a behavior *past* r4133-as-shipped). **Priority: low**, and note it moves the parity target.

### 1.7 UPFC control modes 2/3/5
- **Deferred by:** Phase 7 / UPGRADE (budget-parked; STATUS standing-follow-up).
- **Spec:** `UPFC.pas` — the mode dispatch (`Mode` 1..5). Only the ported subset works today.
- **Current state:** `crates/dss-core/src/elements/pc/upfc/` (grep the `mode` dispatch; modes 2/3/5 unhandled). No corpus deck exercises them.
- **To do:** port the missing UPFC modes + a `controls/upfc` deck per mode; gate vs the oracle. **Priority: low** (no corpus case demands it).

### 1.8 UTF-8 BOM residual edge cases *(minor)*
- **Deferred by:** final acceptance / GAPS ("GAPS follow-up"; GAPS is now closed).
- **What:** the main BOM issues were fixed (CF-A redirect BOM strip; `capture_eventlog` → `utf-8-sig`). Residual: any not-yet-covered BOM/encoding edge on odd input paths.
- **To do:** only if a real deck surfaces it — add the strip at the parse boundary + a fixture. **Priority: very low** (opportunistic).

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
