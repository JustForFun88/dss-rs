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
- **Deferred by:** UNIFIED_GATE Phase D (COMPLETE — it deliberately did not retire these; the manifests' `wp` pointers now name this entry, fixed at the pre-E/F audit).
- **What:** 4 cases stay `defer_ledger` (Rust-smoke only, membership lock-pinned): `Dynamic_KundurDynExp` + `GFL_IEEE123 …DynExp` (port adopts the settled capi015 D14 evaluator; 0.14.5 AND r4133 agree with each other, port ~1.5e-5 away — needs a **measured both-channel voltage envelope**), `controls regcontrol_idle` (capi rejects `idle` #110; needs a measured **r4133 voltage envelope**, ~7e-5 regulator-tap class), `asymmetric line_spacing_asym` (needs a capi_v0145 voltage envelope + **exact-pair-numeric property entries** for normamps/emergamps 730→230 + the existing r4133 #303 skip; the `num_rel` property scope machinery landed at the pre-E/F audit, UGA-T2).
- **To do:** re-measure each envelope live (seed mode `DSS_GATE_SEED_ONLY=<case>`), add the ledger entries with cause+source, drop `defer_ledger`, regen the population lock. The NCIM×4 defer cases are NOT here — they are owned by **WP-U1.7** (suspected op-point port issue; must not be ledgered before re-validation, R3).
- **Where:** STATUS §1i "defer_ledger retirement — partial (honest)" has the full evidence. **Priority: medium** (restores numeric coverage on 4 real decks).

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
