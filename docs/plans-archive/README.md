# Archived plan documents — completed

Moved here 2026-07-18. Each plan below completed **100 % of its own declared
work-package scope** and is gate-green; they are frozen history, superseded only by the
code and tests. Their condensed STATUS records live in `STATUS.md` §1a ("Archived —
completed plan records"); per-phase execution logs are in `docs/phase-records/`.

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

**Still at the repo root (active / foundational):** `PORTING_PLAN.md` (the authoritative
roadmap CLAUDE.md mandates reading first — encodes the still-live §4.1 `TODO(compat)`
convention, the known-upstream-bug ledger, and the binding `#![forbid(unsafe_code)]`/faer
decisions), `DE_PASCALIZE_PLAN.md` (paused after wave 1), `DIAKOPTICS_PSTCALC_PLAN.md`
(Part II open), `RESONANCE_PLAN.md`, `MULTITHREADING_PLAN.md`, `WASM_USERMODELS_PLAN.md`,
the master index `PLAN_SEQUENCE.md`, and `SPLITTING_RULES.md`.

**Residual work these plans left behind** that no active/future plan owns is tracked as an
actionable handoff in **`/ORPHANED_GAPS.md`** (root) — GICMvars export, the AltDSS JSON
DynInit/Full/import/schema tails, the IEEE118 NCIM r4133 cadence, UPFC modes 2/3/5.
