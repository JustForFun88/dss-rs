# Archived plan documents — completed

Moved here 2026-07-18; `R4133_PROPS_PLAN.md` joined 2026-09-04 (archived by its
own RP5.2). Each plan below completed **100 % of its own declared work-package
scope** and is gate-green; they are frozen history, superseded only by the code
and tests. Their condensed STATUS records live in `docs/phase-records/era-summaries.md`
§1a ("Archived — completed plan records", moved out of `STATUS.md` by the 2026-08-05
archiving); per-phase execution logs are in `docs/phase-records/`.

| Plan | Scope | Status |
|---|---|---|
| `PHASE4_PLAN.md` | PD elements + catalog (WP4.*) | ✅ complete |
| `PHASE5_PLAN.md` | Controls + time series (WP5.*) | ✅ complete |
| `PHASE6_PLAN.md` | Meters / topology / Generator (WP6.*) | ✅ complete |
| `PHASE7_PLAN.md` | DER / protection / line-constants / harmonics / dynamics (WP7.*) | ✅ complete |
| `PHASE8_PLAN.md` | Reporting / export / save / dump + executive tail (WP8.*) | ✅ complete |
| `GAPS_PLAN.md` | Closing `NOT_PORTED` gaps (WPG.1–21) | ✅ complete |
| `JSON_EXPORT_PLAN.md` | AltDSS JSON export (Stages A + B) | ✅ complete (A+B) |
| `UPGRADE_PLAN.md` | r3723 → r4133 upgrade (Rung 1 + Rung 2) | ✅ complete |
| `CORPUS_TEST_PLAN.md` | Vendored corpus + manifests + live-oracle gate (build-out) | ✅ complete build; the live map is `TESTING.md` + the running gate |
| `CONTROL_COVERAGE_PLAN.md` | Control/protection/metering coverage gate (5 steps + waves) | ✅ complete build; floor keeps accreting via other plans |
| `R4133_PROPS_PLAN.md` | r4133-channel property parity — the GOLDEN_REBASE G1.1 successor (WP-RP0 … WP-RP5) | ✅ complete (2026-09-04); hands G1.1 back satisfied, unblocking G3.4/G3.5 |

**Still at the repo root (active / foundational):** `PORTING_PLAN.md` (the authoritative
roadmap CLAUDE.md mandates reading first — encodes the still-live §4.1 `TODO(compat)`
convention, the known-upstream-bug ledger, and the binding `#![forbid(unsafe_code)]`/faer
decisions), `DE_PASCALIZE_PLAN.md` (paused after wave 1), `DIAKOPTICS_PSTCALC_PLAN.md`
(Part II open), `GOLDEN_REBASE_PLAN.md` (WP-G1 open), `RESONANCE_PLAN.md`,
`MULTITHREADING_PLAN.md`, `WASM_USERMODELS_PLAN.md`, `UNIFIED_GATE_PLAN.md`
(executed 2026-07-18–19 — Phases 0 and A–F plus §6 final acceptance, record
`docs/phase-records/unified-gate.md`; its own banner still reads *PLANNED*, and
`TESTING.md` cites it as the live gate's design record),
the master index `PLAN_SEQUENCE.md`, and `SPLITTING_RULES.md`.

**Residual work these plans left behind** that no active/future plan owns is tracked as an
actionable handoff in **`/ORPHANED_GAPS.md`** (root) — GICMvars export, the AltDSS JSON
DynInit/Full/import/schema tails, the IEEE118 NCIM r4133 cadence, UPFC modes 2/3/5.
