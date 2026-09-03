# UNIFIED_GATE — Phases 0 and A through F, plus the cross-phase audit and fix rounds

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### UNIFIED_GATE Phase 0 — baseline recorded (2026-07-18)

`UNIFIED_GATE_PLAN.md` execution started (parallel worktree agents; Phases
A/B in flight on `ug-phase-a`/`ug-phase-b`, based `449c745`). Phase 0
baseline, tag **`pre-unified-gate`** = `449c745`:

- Full three-command gate wall-clock (measured 2026-07-18 01:52–02:00,
  **under concurrent load** — the ORPHANED_GAPS session was merging og*
  branches into `update` mid-run, so treat as an upper-bound baseline):
  fmt 1.4 s; clippy 50.5 s; `cargo +stable test --workspace` 426.6 s, of
  which the serial one-shot corpus_live suite = 292.6 s (27 tests; ~510
  cases across the four manifests). dss-core lib 1223+ unit tests.
- Populations at baseline: `solvable_now` 293 (oracle: 246 pinned /
  30 capi015 / 10 r4133 / 6 r3723 / 1 r4088), families asymmetric 47 /
  controls 101 / modes 69, `.dss` bijection 915, `known_diffs.json` 25
  entries (the Phase D ledger seed).
- The one red in the baseline run (`json_transformer_micro`) was an
  artifact of compiling mid-merge of og1213 (BHCurrent/BHFlux emission
  before its `SUPPRESS_JSON` fix landed) — not a unified-gate item;
  re-verified at the next merge-window gate.

Wall-clock table (rows appended per plan §6 at Phases B/D/F):

| point | fmt | clippy | test (full) | corpus gate share |
|---|---|---|---|---|
| `pre-unified-gate` (449c745, loaded box) | 1.4 s | 50.5 s | 426.6 s | 292.6 s |

## 1g. UNIFIED_GATE Phase B — persistent oracle pools + hand-rolled scheduler (branch `ug-phase-b`)

`UNIFIED_GATE_PLAN.md` §4 Phase B. Behavior-identical parallelization of the live
corpus gate; no comparison, tolerance, golden, or `harness` change.

- **Rename + module tree.** `crates/dss-core/tests/corpus_live.rs` → `corpus_gate.rs`
  (git rename, `--follow` preserved) with `#[path="corpus_gate/…"]` submodules:
  `manifest.rs` (schema + loaders + family completeness + AD disposition — schema
  UNCHANGED, `oracle` field still honored, `ORACLE_SPECS` stays), `engines.rs`
  (one-shot `Oracle` + persistent `WorkerPool`/`Worker` + `Channel`), `runner.rs`
  (`run_rust_capture` + `compare_capture` split of `run_and_compare`, CorpusGuard,
  abort/pending), `scheduler.rs` (task grouping + thread pool + proof modes).
- **Persistent pinned pool (D8).** N long-lived `python -u oracle_server.py`
  (`DSS_ORACLE_ENGINE=capi`) workers, ping-verified per process, dedicated
  stdout-line + stderr-drain threads, one in-flight request each, per-request
  deadline `DSS_ORACLE_TIMEOUT_SECS` (default 120) → kill/respawn/retry-once-then-
  fail-case, recycle after 64 cases. Target-rev cases (`capi015`/`r3723`/`r4088`/
  `r4133`) keep the ONE-SHOT `Oracle` path via `Channel::OneShot`.
- **Scheduler (D7a + §3.3).** ONE `#[test]` `corpus_gate_all_cases_match_engines`
  over the union of all four manifests (510 cases); task = case-dir group (cases
  sequential inside → NO per-dir mutexes), longest-first static weight,
  `AtomicUsize` cursor + `std::thread::scope`, T = `DSS_GATE_JOBS` |
  `available_parallelism()` (16), per-channel pool `max(2,T/2)`. Per-case
  `catch_unwind`; the one test fails iff any case failed and prints the COMPLETE
  list (manifest order) — replaces the 4 abort-at-first tests
  (`corpus_live_solvable_cases_match_oracle` + `{asymmetric,controls,modes}_cases_
  match_oracle`). Structural manifest guards, `corpus_live_{classify,properties,
  opendss}`, and the AD sweep are preserved (relocated, not changed).
- **`isolate` (the one permitted early schema addition, §1.2).** Additive
  `isolate: bool` (+ `note`, `isolate ⇒ note` enforced structurally); an isolate
  case runs every execution on a throwaway one-shot worker. Default false; absent
  everywhere until the contamination proof demanded it.

**Contamination proof (§4 Phase B DONE bar).** Ran the full 510-case set three
ways — (a) serial one-shot (`DSS_GATE_SERIAL=1`, T=1, fresh process per case), (b)
persistent parallel, (c) persistent parallel shuffled (`DSS_GATE_SHUFFLE=1234567`) —
each dumping a label-sorted `{verdict, result}` artifact (`DSS_GATE_DUMP`). All
three **bit-identical** — sha256 `e041e018…08116` on all three 1.276 GB dumps,
`diff -q` empty pairwise (settle re-run 2026-07-18).

Three real persistent-worker contamination classes were found and root-caused
(they pass one-shot, fail/pollute persistent) → `isolate: true` (22 cases):
- **AutoAdd process-exit corruption** (`modes/autoadd/autoadd{,_cap}.dss`): the
  documented AutoAdd solve corrupts the dss-python process, so the NEXT case on a
  reused worker access-violates on `clear`. Isolated → the corruption dies with
  the throwaway process.
- **debugtrace held-open trace CSV** (17 solvable_now cases: 7 ckt24-mm masters +
  EPRI/ADiakoptics ckt24 + DOCTechNote + NCIM + StorageTechNote): a `debugtrace`
  RegControl/Storage keeps a fixed-name trace CSV open in the persistent worker,
  so a different worker locks the file (`#303 being used by another process`) and
  the CorpusGuard cannot delete it (pollution). Isolated → the handle releases at
  process exit.
- **File-backed loadshape read drift** (3 `modes/inputformat` decks —
  `shape_mmf`, `shape_filearr`, `shape_binfiles`; found during the settle gate
  re-run, 2026-07-18). A pooled worker reused across many decks intermittently
  (~1-in-3 full-workspace runs) misreads a deck's file-backed loadshape data
  (binary `sng`/`dbl`, `csv`, `MemoryMapping=Yes`) after some prior deck, giving a
  ~2e-3 step-0 node-voltage drift on the ORACLE side. Root-caused empirically: the
  Rust value is bit-stable (`#![forbid(unsafe_code)]` ⇒ race-free; `load_shape`
  has no shared/global mutable state, no mmap); the oracle value drifts only under
  the pool. Never reproduced in 150+ one-shot / in-process-reuse / 12-way
  concurrent-process / sibling-predecessor oracle solves (all correct), and the
  pre-Phase-B one-shot gate + the serial proof run were green — so it is pooled
  cross-deck worker-state contamination, not intrinsic per-process nondeterminism.
  Isolated → a fresh dedicated oracle process per run (exactly the pre-Phase-B
  execution these decks were validated under) removes all predecessor state;
  7/7 consecutive full-workspace gates green post-isolate vs ~2/6 pre-isolate.

`all_properties` heap-UB handling (settled 2026-07-18). The upstream dss_capi
`DoubleSymMatrixProperty` getter (`RMatrix`/`XMatrix`/`CMatrix`/`GMatrix`) and the
shunt-PD reliability inputs render UNINITIALIZED heap memory — order-dependent by
construction (a reused worker's heap carries prior-case residue). Those pairs are
exactly what `harness::skip_prop` (`SKIP_PROPS`) already excludes from the value
compare, so verdicts are unaffected. `write_gate_dump` nulls ONLY those UB values
(keeping the property name) and RETAINS every other property — so the three-way
bit-diff independently re-proves the byte-stability of all 72 k+ gate-asserted
property strings across reused/shuffled workers (2128 `all_properties` blocks
present in the dump, still sha256-identical). This is the ONLY source of
cross-process nondeterminism; every other property is a deterministic function of
the deck.

**Audit dispositions (settle, 2026-07-18).**
- **B1 — dropped `pending`⊕`expect_solve_abort` mutual-exclusivity assert** (both
  audits, low). REAL, latent (0 manifest cases set both, empirically checked). The
  old family-only `assert!` was not carried into the unified classifier, which
  silently resolves the conflict pending-first. FIXED: restored the `assert!` in
  `scheduler::make_case` — now applied to EVERY source (solvable_now + families),
  strictly stronger than the family-only original.
- **B2/B3 — dump stripped the whole `all_properties` block** (audit-code B2 /
  audit-tests B3, low). REAL completeness gap: a within-tolerance oracle-property
  drift on a reused worker could escape both the verdict channel
  (`assert_value_matches_tol`) and a whole-block strip. FIXED: `strip_ub_properties`
  now nulls only the `skip_prop` UB values, retaining all value-asserted properties;
  the three-way bit-diff is still byte-identical (proof above), so the fix strictly
  strengthens the artifact with no regression.
- audit-tests B2 (stray `.claude_tmp_corpus_live_old.rs` leftover) — not present;
  worktree clean, corpus pristine.

**Wall-clock** (settle-run 2026-07-18, 16-core, incl. dump write).

| mode | jobs | pool | wall-clock |
|---|---|---|---|
| serial one-shot (old cost model) | 1 | 2 | 341.9 s |
| persistent parallel | 16 | 8 | 104.3 s (3.3× faster) |
| persistent parallel shuffled | 16 | 8 | 78.1 s |

`tests/corpus` pristine after all runs (the vendored decks are untouched; the
corpus_gate CorpusGuard restores its own case dirs). `cargo fmt/clippy/test` green
in the worktree.

**Open follow-up (pre-existing, non-blocking).** A full `cargo test --workspace`
occasionally leaves a handful of untracked solver EXPORT outputs under
`tests/corpus/electricdss-tst` (e.g. `Test/AutoTrans/Auto3bus_noload_power.txt`,
IEEE8500/StorageControllerTechNote monitor/EXP CSVs). These are `Export`/monitor
files a deck writes to a path the per-case `CorpusGuard` (which guards only the
case's parent dir) does not sweep — a pre-existing CorpusGuard corner case
independent of Phase B (the guard logic is byte-identical to the old gate) and of
this settle's diff. They are untracked (never committed) and path-limited-cleaned
before commit. A future CorpusGuard hardening (guard the export CWD too) would
close it.

---

## 1h. UNIFIED_GATE Phase C — manifest schema v2 (`engines`) + population lock v2 + r4133 channel wiring (branch `ug-phase-c`)

`UNIFIED_GATE_PLAN.md` §4 Phase C (after the A+B integration merge, base `3ad37c8`).
Retires the target-rev one-shot Oracle/Oddie shim for the mandatory gate: the
`r4133` channel now gates through the in-house `epri-worker` pool (Phase A bridge).

- **Schema v2 (§1.2).** `SolvableCase.oracle: Option<String>` → `engines: "capi_v0145"
  | "r4133" | "both"` (default `"both"`; no case is `"both"` this phase). Migration
  preserved semantics exactly: pinned (no `oracle`) → `capi_v0145` (246 solvable_now
  + 162 family); every target-rev case (`capi015`/`r3723`/`r4088`/`r4133`) →
  `r4133` (106 total = 47 solvable_now + 4/34/21 asym/controls/modes). `ORACLE_SPECS`
  deleted; `EngineChannel {CapiV0145,R4133}` + `SolvableCase::engine_channels()`
  replace it. `corpus_manifest.rs` untouched; `.dss` bijection 915 intact.
- **r4133 worker pool (§3.1).** `engines.rs` gains `EpriPool` (persistent
  `epri-worker` processes, mirroring the capi `WorkerPool` lifecycle: per-request
  deadline → kill/respawn/retry-once/recycle-64, `{"epri":true,"rev":"r4133"}` ping
  assert) + `EpriOneShot` (throwaway worker for serial/`isolate` r4133 cases).
  Worker-binary resolution: `DSS_EPRI_WORKER` env → `target/<profile>/epri-worker` →
  OnceLock `cargo build -p dss-epri` fallback. `Channel` now has four arms
  (Capi/Epri × Pool/OneShot). Iteration policy is per-channel: capi_v0145 = exact
  1:1; r4133 = `rust_le_oracle`. Eventlog masks keyed on the channel. The r4133
  request masks `all_properties` off (capi_v0145-only per §1.2 — the bridge has no
  all-props capture). *(Superseded 2026-09-03: Phase D gave the bridge
  `capture_all_properties`, and R4133_PROPS RP4.1 removed the mask — the r4133
  request carries `all_properties` and its property table is gated.)*
- **Re-validation (§4/§5 R9) — all 106 r4133 cases gated live against the epri
  bridge.** 95 pass green; **11 deferred** to Phase D (all were `oracle:capi015`,
  the retired 0.15.0b4 line, and reproduce on NEITHER surviving channel — proven by
  flipping all 11 to `capi_v0145` and re-running: 0.14.5 also diverges). The 99
  live-gated cases split by their prior pin (base-commit `oracle` counts across
  the four manifests: capi015 59, r4133 40, r3723 6, r4088 1):
  - **40 were already `oracle:r4133`** → same revision, same value contract; the
    only change is the *transport* (in-house `epri-worker` replaces the Oddie
    one-shot). Phase A cross-validated epri-worker == Oddie **bit-for-bit on the
    same r4133 DLL** over 396 cases, so these are unchanged by construction.
  - **6 `oracle:r3723` + 1 `oracle:r4088` + 48 `oracle:capi015`** (55) → a genuine
    **revision re-pin**: their value contract shifts from "matches r3723/r4088/
    capi015-0.15.0b4" to "matches r4133". These are NOT covered by the Phase A
    bit-for-bit proof (that proof is bridge-equivalence for one DLL, not
    r3723≡r4133 or capi015≡r4133 behavior); the guarantee is the fresh green
    re-validation that Rust == r4133 for each (D4/D5: r3723/r4088/capi015 retired
    to r4133 as the single surviving EPRI line). All 55 gated green. Honest re-pins,
    not no-ops — noted so the revision shift is not mistaken for pure transport.
  - The remaining **11 capi015** cases could NOT re-pin to r4133 (both channels
    diverge) → deferred (see below).
- **`defer_ledger` — the Phase D ledger seam (§4 step c / §5 R9 last resort).** New
  optional field carrying the Phase-D-ledger-seed cause (+ mandatory `wp`, mutually
  exclusive with `pending`/`expect_solve_abort`). A deferred case is parked from live
  oracle comparison but still **Rust-smoke-run** (compile + solve every step must
  converge, no new errors) so a Rust regression can never hide. Membership is
  preserved (counts unchanged; the population lock records the `defer` flag) — the
  plan's "membership never shrinks" holds. The 11 deferrals, by class:
  - **NCIM ×4** (`NCIM/Xmission…Kundur2Area`, `modes/ncim/{ncim_pq,ncim_pv_pq,ncim_midi}`):
    0.14.5 lacks NCIM (`Set algorithm=NCIM` ignored, `Export deltaf` #24713); r4133-11.0
    NCIM converges to a different PV/Q op-point → `Vsource.source` current diverges
    wholesale. wp `WP-U1.7`.
  - **DynExp ×2** (`Dynamic_Expressions/Dynamic_KundurDynExp`, `IBRDynamics_Cases/GFL_IEEE123/
    Run_IEEE123Bus_GFLDaily_DynExp`): capi015 D14 `Exit`-no-op (frozen state); 0.14.5
    swings, r4133 differs → node V ~1.5e-5 rel.
  - **GFM ×3** (`controls/gfm/{gfm_micro,gfm_invcontrol,gfm_dynamics}`): capi015 B5 Isc1
    (drop ×1000) — 0.14.5 system-Y differs (9 entries); r4133 physics MATCHES but
    `Storage.batt.%stored` property STRING is r4133-rounded (92.4320641163609 vs
    92.4321). wp `WPG.10`/`WPG.13`.
  - **RegControl idle ×1** (`controls/regcontrol/regcontrol_idle`): capi015 `idle`
    property (0.15 feature) — 0.14.5 rejects `idle` (#110); r4133 node V ~7e-5 rel.
  - **line_spacing_asym ×1**: r4133 raises **#303 access violation at calcv** (known
    linespacing crash) AND 0.14.5 gives `Line.normamps=230` vs Rust/capi015 730 —
    diverges on BOTH channels (Phase D: r4133 `skip` + capi_v0145 property ledger).
- **Population lock v2 (§1.4).** `Case::rigor` drops `oracle=`, gains
  `engines={…} isolate={…} defer={…}`; the documented asymmetry is fixed — the three
  family manifests are now per-case rigor-covered (`family_rigor` replaces
  `family_paths`). Regenerated via `DSS_UPDATE_POPULATION_LOCK`; reviewed diff =
  field additions + **zero membership loss** (counts 293/47/105/69 bit-stable;
  solvable_now + family key sets identical to base).
- **Report channels kept compiling.** `corpus_live_opendss` + `known_diffs.json` +
  `DSS_LIVE_OPENDSS*` retained (Phase D deletes them); their target-rev exclusion is
  re-keyed on `engines` (contains r4133). `corpus_live_classify`/`_properties`
  re-keyed on `gates_capi()`. `AdSweepCase.oracle` (unused; ad_sweep.json had 0
  oracle values) dropped with its `ORACLE_SPECS` validation.

**Counts (bit-stable at base):** solvable_now 293, asymmetric 47, controls 105,
modes 69, `.dss` bijection 915.

**Gate (three-command, green).** `cargo fmt --all --check` clean; `cargo clippy
--workspace --all-targets -- -D warnings` clean; `cargo test --workspace` all pass
(dss-core lib 1231 + corpus_gate 25 incl. the 514-case unified gate; 1 pre-existing
`ckt24_graph_diagnostic` ignored). Corpus gate wall-clock **with the r4133 channel
active = 64.5 s** at the Phase-C build; the settle re-run (audit-fix build, same
machine) measured **67.4 s** for the `corpus_gate` target (main gate test 60+ s).
`tests/corpus` pristine after runs (path-limited-cleaned 12 pre-existing CorpusGuard
export-CWD leftovers — StorageControllerTechNote monitor CSVs + AutoTrans txt;
STATUS §1g open follow-up, not introduced by Phase C).

Wall-clock table row (plan §3.4 / §6):

| point | mode | jobs | pool | corpus gate |
|---|---|---|---|---|
| Phase C (r4133 channel active) | persistent-parallel | 16 | 8 (per channel) | 64.5 s |
| Phase C settle (audit-fix build) | persistent-parallel | default | default | 67.4 s |

**Deviation from plan (justified).** The brief's ladder step (c) said `pending: true
+ wp`, but `pending` structurally asserts the Rust engine ERRORS (unported feature);
the 11 deferrals are PORTED features that solve cleanly — `pending` would fail the
gate. Introduced `defer_ledger` instead (the plan's "leave a clear seam" for the
Phase D ledger, §1.4): same intent (park from live compare, documented cause + wp,
membership preserved, reviewed lock diff) without the false "must error" contract,
and it adds a Rust-side smoke net `pending` also lacks. Brief header said "76
re-targeted"; the actual re-targeted (oracle-carrying) population is **106** (the
brief's own parenthetical sums to 106); all 106 re-validated.

### Phase C settle — audit dispositions + empirical deferral proof (2026-07-18)

Two independent audits (audit-code, audit-tests) of `3ad37c8..b89b2d3` returned
**four low-severity findings**, all about the 11 `defer_ledger` cases. Each settled
**empirically** against the live r4133 `epri-worker` and the pinned dss-python 0.14.5
oracle in the worktree (not by argument).

**Re-validation table (106 target-rev cases, by prior pin → r4133 channel):**

| prior `oracle` | count | outcome on r4133 channel |
|---|---|---|
| r4133 | 40 | green — transport-only (epri-worker == Oddie bit-for-bit, Phase A) |
| r3723 | 6 | green — revision re-pin, Rust == r4133 |
| r4088 | 1 | green — revision re-pin, Rust == r4133 |
| capi015 | 48 | green — revision re-pin, Rust == r4133 |
| capi015 | 11 | **deferred** — diverges on BOTH surviving channels (proof below) |

**Empirical both-channels-diverge proof for the 11 deferrals** (settle probes, live):
- **GFM ×3** (`gfm_micro`/`gfm_invcontrol`/`gfm_dynamics`): on r4133 the worker
  returns `Storage.batt.%stored` = **"92.4321"** (property getter rounds to 4
  decimals); capi 0.14.5 and Rust return the full-precision **"92.43206411636…"**.
  The `micro` tier allows `1e-6 + 1e-9·|v| ≈ 1.09e-6`, but the string gap is
  **3.59e-5** → the probe genuinely fails on r4133. On capi_v0145 the assembled
  system-Y differs (B5 Isc1 ×1000, DIVERGENCES.md #b5 — Rust adopted the r4133 Isc1).
  Both channels diverge. **Why the sibling `pv_gfm_dynamics` gates r4133 GREEN and
  is NOT deferred** (the audit's specific question): it probes `PVSystem.pv`
  `irradiance`/`pmpp`/`kva`, which the r4133 worker returns as the stable input
  strings **"1"/"800"/"800"** (no state-integrated float, no getter rounding) — the
  distinguishing probe is `%stored`, not the physics, which matches r4133 for all
  four (Rust uses the r4133 Isc1). Verified with the worker on both decks.
- **line_spacing_asym ×1**: the r4133 worker **aborts at compile with #303 access
  violation** (`Error 303 Reported From OpenDSS Intrinsic Function`); capi 0.14.5
  gives `Line.normamps=230` vs Rust/capi015 730. Both channels diverge (confirmed).
- **RegControl idle ×1**: capi 0.14.5 **rejects `idle` with #110** ("Unknown
  parameter idle") — a 0.15 feature; r4133 idle-regulator node V ~7e-5 rel. Both
  diverge (capi side confirmed live).
- **NCIM ×4**: capi 0.14.5 has **no NCIM** (`Solution.algorithm` unknown to the 0.14.5
  API; `Set algorithm=NCIM` ignored) — confirmed; r4133 NCIM converges to a different
  PV/Q op-point than the Rust NCIM port (WP-U1.7). Both diverge.
- **DynExp ×2**: capi015 D14 `Exit`-no-op freezes the state-var seed; 0.14.5 swings,
  r4133 evaluates differently (documented D14, DIVERGENCES.md; STATUS §UPGRADE).
  Both diverge.

**Audit dispositions:**
- **C-1 (both audits) — FIXED.** The deferred-smoke doc comment (`runner.rs`) and
  the `defer_ledger` field doc (`manifest.rs`) overclaimed "a Rust regression can
  never hide behind the deferral." The smoke asserts only per-step convergence +
  unchanged error count — it catches a *convergence/error-surfacing* regression but
  **not** a *numeric-correctness* regression that still converges (no physical value
  is compared). Both comments reworded to scope the guarantee accurately and note
  that full numeric coverage returns with the Phase D ledger. No assertion changed.
- **C-2 (audit-tests) — FIXED.** The re-validation prose conflated bridge-equivalence
  (epri-worker == Oddie on the *same* r4133 DLL, Phase A) with revision-equivalence.
  Reworded above to split the 40 transport-only r4133 cases from the 55 genuine
  revision re-pins (6 r3723 + 1 r4088 + 48 capi015), whose contract shifted to
  "matches r4133" and rests on the fresh green re-validation, not the bit-for-bit
  proof.
- **C-2 (audit-code) — RECORDED, deliberately NOT code-fixed.** `defer_ledger` has no
  *mechanical* guard that the R9 ladder was exhausted (it enforces cause + `wp` +
  mutual-exclusion only). A mechanical guard is infeasible in an oracle-free
  structural test: proving both channels diverge requires running both live oracles,
  which the structural `manifest.rs` tests deliberately do not. The interim controls
  are (a) the reviewable population-lock diff (`defer=1` per case), (b) the mandatory
  documented per-case cause, and (c) — added here — the live both-channels-diverge
  proof above for all 11. Each deferral is a Phase-D ledger seed; the ledger machinery
  (envelope/probe carve-out) lands the mechanical re-gate. Recorded, not masked.

## 1i. UNIFIED_GATE Phase D — divergence ledger + seeding + engine flips (branch `ug-phase-d`)

Lands the gating divergence ledger (`tests/corpus/ledger.json`) that replaces the
report-only `known_diffs.json`, seeds it from a full both-channel measurement,
flips the cleanly-dual-gateable cases to `engines:"both"`, deletes the retired
Oddie/OpenDSS report path, and makes the both-channel gate **deterministic**. Base
`ae4b4ef`.

**Ledger machinery (`corpus_gate/ledger.rs`, 9d5852b + settle 35ad4e2).**
`LedgerRuntime` loads/validates `ledger.json` (kinds `divergence`/`skip`/
`exclusion`) and exposes per-(case,channel) `LedgerView`s that `compare_capture`
consults to partition each comparison field: the untouched `harness` comparator
runs the unscoped remainder; the ledger's envelope/exact-pair assert covers the
scoped part and records the hit. §1.3 envelope: (a) selected values differ ≤
envelope; (b) unselected meet the tier floor; (c) ≥1 selected value **exceeds the
tier floor** else the entry is **stale → gate fails**. `exceeded_floor` is measured
against the untouchable `harness` tier floor, NOT the entry's `max_rel` — so
widening an envelope can never hide a shrinking divergence (a stronger
fail-on-stale than the plan's phrasing). Structural test (oracle-free): unique ids,
case∈manifest, channel∈engines, divergence⇒non-empty match, cause/cause_ref
resolve, regexes compile, probe/property exact-pair-only.

**Seeding (`DSS_GATE_SEED_LEDGER=1`).** Ran every Live/Deferred case against BOTH
channels with no ledger → `tmp/ledger_candidates.json`: 510 cases ×2 = 1020
measurements, 836 match / 157 diverge / 27 error. **Zero** currently-single-channel
case matches on both channels — the inherited flip already captured every free
flip; further `both` requires a hand-reviewed ledger entry per case.

**Gate determinism — the R2 worker-state contamination, surfaced + fixed.** The
both-flip widened the r4133 pool's exposure enough to surface plan R2 live:
persistent pooled workers accumulate state that `clear` does NOT reset (`Set`
options, memory-mapped loadshape handles), so a worker that had served, e.g., a
relay / harmonics / IEEE13-geometry deck would *intermittently* hand the next deck
a stale option/mmap → ~1e-3 divergences on **either** channel that vanish
serial/one-shot (~1 flake per 6 full runs — the same deck matches cleanly in the
seeding). Fixed systemically in `engines.rs`: **`recycle_after()` now defaults to 1
(a fresh worker per case)** — every case sees a never-used worker, so no state is
inherited; the persistent pool keeps only its amortized startup. Wall-clock is
unchanged (respawns overlap across the pool). `DSS_GATE_RECYCLE_AFTER=<n>` raises it
for a faster, non-deterministic dev loop. Complementary `isolate:true` (one-shot,
process exits → releases handles) added for the file-handle-contention decks the
recycle cannot cover within a case: `StoCtrl_SeasonTarget` (EnergyMeter DI CSV held
open), plus defensive isolate on the IEEE13-geometry family + `mmf_singlecol` +
`YgD-Test`. Determinism verified: **4/4** consecutive green full runs post-fix
(after ~10 pre-fix runs that whack-a-mole isolation could not stabilise). `M1/
Master_NoPV` kept **r4133-only** (its both-flip surfaced a capi_v0145
all-properties divergence — a Load renders `PF=1` on the port vs `0.9` on the 0.14.5
oracle — **settled at Phase-D settlement, NOT a bug**: see the settlement addendum
below).

**Deletions (same landing).** `tests/corpus/known_diffs.json`, the
`corpus_live_opendss` test + `KnownDiff`/`load_known_diffs`, `Oracle::opendss` +
`oddie_venv_python` + the ping `want_oddie` arm, and the `DSS_LIVE_OPENDSS*` knobs.
`rg "known_diffs|DSS_LIVE_OPENDSS|Oracle::capi015|corpus_live_opendss" crates tests`
is clean of live code (doc-comment prose only). The `tools/opendss/*.py` report
scripts still reference the removed file — they die in **Phase E** per the brief.

**Lock v2 ledger component (§1.4).** `population_lock.rs::rigor()` gains
`ledger={sorted per-channel entry ids}` (`ledger_tags()`); every ledger add/widen/
flip changes the case's tag → a reviewed lock diff (e.g. `ledger=r4133:r4133-
binaryshape-303`), so the ledger cannot become a silent soft-tolerance backdoor.

**Ledger contents.** 6 entries / 20 causes: 3 `skip` (binary/MMF GrowthShape #303,
IEEE13 line-spacing + line-and-cable-spacing #303 on r4133) + 3 GFM r4133 probe
`divergence` (`Storage.%stored` %.6g Delphi rounding, `num_rel` 1e-6) — the
Phase-C seed, each hit + non-stale every run.

**Canary (fail-on-stale, live, §6).** A temporary all-node voltage divergence entry
on `vsource_asym` (which matches the oracle within the tier floor) was added: the
full gate passed all 514 cases, recorded the entry as applied (9 hits) but
never-exceeded-floor, and **FAILED** with `STALE — every selected value is now
within the tier floor. Prune it.` Reverted immediately. Fail-on-stale proven live.

**defer_ledger retirement — partial (honest).** GFM×3 retired in the Phase-C seed
(r4133 probe ledger). Of the remaining **8**, disposition settled empirically
(seed + focused probes) — the field REMAINS because 4 cases cannot be responsibly
retired:
- **NCIM×4** (`ncim_pq/pv_pq/midi`, `NCIM/Xmission`): capi 0.14.5 lacks NCIM
  (errors); the Rust NCIM port (validated vs the now-retired capi015) converges to a
  **wholesale-different op-point** than r4133 NCIM (Vsource source-current sign-flip
  ~156 A, and the Rust source-bus voltage reads suspiciously exactly-nominal). Not
  tightly fingerprintable (plan: stays single-channel) and possibly a port issue →
  per R3 NOT ledgered; needs a rigorous WP-U1.7 NCIM re-validation. Kept
  `defer_ledger` (Rust-smoke), note corrected.
- **DynExp×2** — ledgerable but deferred for measurement: the port deliberately
  adopts capi015's D14 evaluator (DIVERGENCES §D14, *settled*); the data confirms
  **both** 0.14.5 AND r4133 agree with each other (179425.907) and the port differs
  by ~1.5e-5 — a documented cross-line divergence, not a bug. Retirement needs a
  measured both-channel voltage envelope (follow-up).
- **RegControl idle×1** — capi rejects `idle` (#110); r4133 ~7e-5 regulator-tap
  class. Ledgerable on r4133 with a measured envelope (follow-up).
- **line_spacing_asym×1** — capi node-V ~7e-8 (line-impedance libm floor) +
  `Line.lsp.normamps`/`emergamps` **oracle 0.14.5 = 730/1095, port = 230/345**
  (settled empirically 2026-07-18; the port's 230 min-over-phase is CORRECT per
  r4133 LineGeometry.pas — earlier notes had the direction reversed, now fixed) +
  r4133 #303 skip. Ledgerable capi voltage + property exact-pair, but the discrete
  730→230 jump needs an exact-pair-numeric probe/property scope the current
  machinery lacks (probe path only exact-pairs non-numeric values) → follow-up.

**both% = 342/514 = 66.5%** (single-channel 172: capi_v0145 75, r4133 97). The
plan's ≥90% target is **arithmetically unreachable**, proven by the seeding: 97
cases are r4133-only 0.15/r4133 features 0.14.5 cannot run at all (WindGen, NCIM,
LineConstants upgrades, MonitoredVoltage InvControl, relay-0.15, batchedit-where,
MMF single-col …) → they can never be `both`; 75 are capi-only wholesale-divergent
on r4133 (reduce/makeposseq reductions, Carson geometry/cable upgrades, IEEE_519
harmonics, ckt24 conditioning …) which the plan explicitly keeps single-channel.
Even flipping every fingerprintable candidate caps ~72%. The documented follow-up
flip set (fingerprintable, cause-mapped, envelope-measurable): probe %.6g
display-precision ×~16 (storage/pvsystem), injection-fpc-delphi-ulp ×4
(indmach/combo asym), monitor-seq-magnitude-drift ×1 (`monitor_seqmag`).

**Counts (bit-stable):** solvable_now 293, asymmetric 47, controls 105, modes 69,
`.dss` bijection 915. isolate cases = 33.

**Gate (three-command, green + deterministic).** fmt clean; clippy clean; `cargo
test --workspace` green. Full BOTH gate ≈ 137–160 s at jobs=16 pool=8 recycle=1
(≪ §3.4 ≤10 min). `tests/corpus` pristine.

Wall-clock table row (plan §3.4 / §6):

| point | mode | jobs | pool | recycle | corpus gate |
|---|---|---|---|---|---|
| Phase D (full BOTH gate, ledger active) | persistent-parallel | 16 | 8/ch | 1/case | ~150 s |

**Open follow-ups (Phase D → later):** (1) retire the remaining `defer_ledger` —
DynExp×2 / idle×1 / line_spacing×1 via measured envelopes (line_spacing also needs
the exact-pair-numeric probe scope, below); NCIM×4 needs a WP-U1.7 NCIM op-point
re-validation first. (2) The fingerprintable `both` flip set (~21 cases) to push
toward the ~72% ceiling. (3) `tools/opendss/*.py` report scripts still reference
`known_diffs`/`corpus_live_opendss` → Phase E. (`M1/Master_NoPV` follow-up (3) is
now settled — see addendum.)

### Phase-D settlement (audit dispositions, 2026-07-18)

Two independent xhigh audits (audit-code + audit-tests) of `ae4b4ef..e9a2502`.
Dispositions, settled empirically (drive the live engines / read the Pascal), never
by loosening a tolerance:

- **F1 (both audits, HIGH) — element/monitor envelope only checked the SELECTED
  sub-channels, dropping the unscoped remainder from all comparison** (a matching
  `element`/`monitor` entry made the caller skip the *whole* element/monitor's
  `compare_element`/`compare_monitor`; the in-code doc falsely claimed the rest was
  "still tier-checked"). Latent (no such entries ship yet) but a real clause-(b)
  hole exactly on the brief's R3 focus. **FIXED** by generalizing the `property`
  rewrite pattern: `element_rewrites`/`monitor_rewrite` re-assert the pinned
  sub-channels inside their envelope (clause a) then rewrite ONLY those to the Rust
  values so the untouched `compare_element`/`compare_monitor` tier-checks every
  unscoped channel (clause b). `runner.rs` now always runs the harness comparator.
  False doc comments corrected.
- **F4-code (MEDIUM) — a non-numeric probe scope with no `oracle` pin
  self-certified (marked applied+exceeded, skipped all comparison).** **FIXED**:
  the non-numeric branch now requires an exact `oracle` pin (else panics — discrete
  state is exact-pair only, §1.3), asserts the live Rust value against an optional
  `rust` pin, and marks `exceeded` only when Rust ≠ oracle (so it CAN go stale).
- **F5-tests (MEDIUM) — exact-pair `property`/non-numeric-`probe` entries marked
  `exceeded` unconditionally → fail-on-stale could never fire for them.** **FIXED**:
  both now query the live Rust value and mark `exceeded` only when it still differs
  from the oracle, so a vanished discrete divergence trips STALE. (`mask_line`
  eventlog/ctrlqueue entries already self-detect via the NEVER-APPLIED path when the
  artifact line disappears; `skip` entries are `Kind::Skip`, exempt from the
  divergence-stale check by design.)
- **F2-code (HIGH-ish factual) — the `linespacing-normamps` cause, the
  `line_spacing_asym` defer_ledger note, and STATUS recorded the 730-vs-230
  direction BACKWARDS.** **SETTLED empirically**: pinned dss-python 0.14.5 oracle =
  `Line.lsp.normamps 730 / emergamps 1095` (first-wire ACSR_556 rating); the port =
  `230 / 345` (MIN over phase conductors {556→730, 4-0→340, 1-0→230}=230), matching
  r4133 V8 `LineGeometry.pas:1237-1239`. **The port's 230 is CORRECT** (WP-U1.2 D3
  min-over-phase upgrade) — a wrong-fact-in-the-ledger, not a papered-over bug. Cause
  text, manifest note, and this record corrected. The cause is currently unused (no
  entry references it), so nothing was mis-gated live.
- **F3-tests (MEDIUM) — `M1/Master_NoPV` Load `PF=1`-vs-`0.9`, flagged "possible
  PF-parse bug".** **SETTLED empirically: NOT a bug.** The ~10 diverging loads
  (`Loads_Only.dss`: `kW=0 kvar=0 pf=0.9`) hit the WP-U1.1 **L2 REPLACE_ZERO** clamp
  (`kW`/`kVA` parsed in `(-1e-8,1e-8)` → `+1e-8`, the EPRI r4133 `DblValueNZ`
  default the 0.14.5 oracle lacks; `prop_flags.rs REPLACE_ZERO`). On 0.14.5 `kW=0`
  stays 0 so `LoadSpec kW_kvar` leaves `PFNominal=0.9`; on the port `kW=1e-8` makes
  `kVA>0` so `PFNominal=kW/kVA=1`. The port correctly follows r4133 (which is why M1
  gates cleanly on r4133). The delta touches every zero-load's kw/kva/pf strings —
  **wholesale, not tightly fingerprintable** → per §4-D it stays r4133-only with a
  corrected note cause, not a ledger entry. No code change; the port is right.
- **F1-defer / F3-code (HIGH) — DONE-bar "11 ex-defer_ledger live-gated;
  defer_ledger fully retired" is NOT met; 8 remain Rust-smoke-only.** Deliberately
  **not force-retired** (rationale, per R3 "a ledger entry that papers over a fixable
  bug is the worst outcome"): NCIM×4 is a suspected op-point port bug (needs WP-U1.7)
  and DynExp×2 matches NEITHER surviving oracle — force-ledgering either would pin a
  bug; idle×1 and line_spacing_asym×1 are settled upgrades but need machinery the
  phase doesn't ship (r4133 voltage envelope resp. exact-pair-numeric probe). All 8
  keep `defer_ledger` (Rust-smoke: solve + no-new-errors) with corrected notes and
  the follow-ups above. The `defer_ledger` field is therefore RETAINED, honestly.
- **F5-code/F6-tests (LOW) — both% 66.5% < 90% target.** Disclosed above and
  data-backed (~72% seeding ceiling); the ≥90% target is arithmetically unreachable.
  No action.
- **F6-code (LOW) — `injection` envelopes the whole RHS vector (no node
  sub-selector).** Acknowledged design (injection has no natural per-node selector);
  the planned `injection-fpc-delphi-ulp` entries are whole-vector ulp floors, so
  clause (b) being vacuous is acceptable. No change; noted for the follow-up author.
- **F7-code/F4-tests (LOW) — `tools/opendss/*.py` read the deleted
  `known_diffs.json`; ~18/20 ledger causes are referenced only by prose notes.**
  The python scripts die in **Phase E** per the brief (out of scope here); the
  orphaned causes are the imported known_diffs class-prose kept as reference for the
  single-channel note cases — retained as documentation, no gating impact.

## 1k. UNIFIED_GATE pre-E/F cross-phase audit (branch `ug-audit`, 2026-07-19)

Two independent xhigh read-only audits (code-fidelity + verification-strength) of
the WHOLE delivered range `pre-unified-gate` (`449c745`) .. post-Phase-D `update`
(`a0ac274`), before Phases E/F run. **Per-phase verdicts: 0 PASS, A PASS, B PASS,
C PASS, D PASS-at-settled-bar** — the delivered gate verifies MORE than the
pre-range gate (whole-range `tests/harness` diff = one `fn`→`pub fn` visibility
change; no tolerance/floor/assert weakened; lock membership 514/514 zero loss;
corpus pristine). Both audits re-derived the shipped ledger from the live engines
(r4133 renders `Storage.%stored` to 6 sig figs vs full-f64 pinned oracle, inside
the `num_rel 1e-6` envelope; the #303 line-spacing crash reproduces live) — no
entry papers over a fixable port bug. 16 findings (2 medium, rest low), settled
here empirically; every fix is rigor-ADDING, no tolerance/envelope/assert loosened,
no golden touched.

**Fixed (this branch, canary-proven live):**
- **UGA-1/T1 (medium) — envelope widening escaped the population lock**: the
  lock's ledger tag was entry-id-only, so raising `max_rel`/`num_rel`, adding a
  scope, or cutting `steps` on an EXISTING entry produced no lock diff —
  contradicting plan §1.4/§5-R3 and the STATUS §1i claim. `ledger_tags()` now
  fingerprints each entry as `id@FNV-1a64(full entry JSON)`; lock regenerated
  (diff = exactly the 6 ledgered cases gaining `@digest`). Canary: widening
  `r4133-gfm-micro-pctstored` `num_rel` 1e-6→1e-2 now FAILS `population_lock`
  (proven, reverted).
- **UGA-T2 (medium) — `property` ledger scope half-enforced §1.3**: no mandatory
  `oracle` pin (a bare `name_re`-only scope masked with zero assertion), `rust`
  pin ignored, `num_rel` ignored. `property_handled_keys` now mirrors the probe
  path exactly (Phase-D F4/F5 parity): numeric-skeleton `num_rel` envelope
  against the live Rust `?`-value + tier-floor staleness, non-numeric requires
  the `oracle` pin + honors the `rust` pin; structurally a probe/property scope
  now REQUIRES `oracle` or `num_rel`. Canary-proven on `vsource_asym`
  (`Vsource.source.basekv`): wrong pin fails loudly ("oracle 12.47 != pinned
  9999"), `num_rel` entry applies with 1 hit. (Unblocks the ORPHANED_GAPS §1.9
  `line_spacing_asym` exact-pair-numeric retirement.)
- **UGA-2 — latent skip full-bypass**: no rule prevented a case's ONLY gating
  channel(s) from all being `skip`-ledgered (→ zero verification, not even the
  Rust smoke). `assert_structural` now requires ≥1 non-skipped channel per
  skip-bearing case (canary: a capi skip added to the r4133-skipped
  `IEEE13_LineSpacing` fails structurally). All 3 shipped skips sit on
  `engines:"both"` cases — latent only.
- **UGA-T4 — no scope-field allowlist**: a typo'd or §1.3-but-unimplemented
  field (`yprim`/`y_fingerprint`/`meter`/`global_result`) compiled fine and
  silently never applied (masked inside a multi-scope entry by per-entry hit
  accounting). `compile_scope` now rejects anything outside the 9 implemented
  fields, loudly (canary-proven).
- **UGA-T3 — `line_re` masks could never go stale on content-matched lines**:
  `mask_line` marked exceeded unconditionally on regex match. It now records a
  live divergence only when the trailing-whitespace artifact is actually present
  (`line != line.trim_end()`), so a vanished artifact trips STALE. (Mask power
  was always bounded to `trim_end`; 0 such entries ship.)
- **UGA-T5 — `DSS_GATE_ONLY` matching nothing greened a 0/0 run** (and skipped
  fail-on-stale): a leftover exported env var could silently neuter the gate.
  The scheduler now panics on zero retained cases (canary-proven).
  (`DSS_GATE_SEED_LEDGER` remains a deliberate, loudly-bannered report mode.)
- **UGA-3/T8 — stale in-code docs** contradicting delivered mechanics:
  `corpus_gate.rs`/`scheduler.rs` "target-rev cases keep the one-shot Oracle
  path" (retired in Phase C), `engines.rs` EpriPool "recycle after 64 cases"
  (default 1 since Phase D), `manifest.rs` "valid `oracle` field" (deleted in
  Phase C). Corrected.
- **UGA-4 — dangling follow-up ownership**: 5 manifest `wp` pointers still named
  `WP-UG-D` (a COMPLETE phase that deliberately did not retire them). The 4
  ledgerable-but-unretired defer_ledger cases (DynExp×2, `regcontrol_idle`,
  `line_spacing_asym`) now point at **ORPHANED_GAPS §1.9** (new entry, full
  retirement recipe); the solvable_now NCIM Xmission case now points at
  **WP-U1.7** like its 3 NCIM siblings. (`wp` is not in the rigor fingerprint —
  no lock impact.)
- **UGA-T6 — ubuntu CI leg structurally red post-Phase-C/D**: the mandatory gate
  spawns `epri-worker` (`#[cfg(windows)]`, exits 1 on Linux) for 439
  r4133/both-gated cases. `ci.yml` matrix reduced to `windows-latest` with the
  reason documented in place.

**Deliberately NOT fixed (recorded with rationale):**
- **UGA-5 — plan §2.2 all-properties enumeration sub-item not implemented** in
  the bridge ("implement anyway for report tooling parity"): `capture.rs`
  fail-louds on an `all_properties` request (verified — no fake-empty dump) and
  the scheduler masks it off per-channel; gating is unaffected (property parity
  is capi_v0145-only by plan). Recorded as an accepted §2.2 deviation: implement
  only if report tooling ever needs it (`DSSPut_Command("? name.Like")` +
  `DSSElementV`). *(Superseded 2026-09-03: implemented in Phase D and made
  gating by R4133_PROPS RP4.1.)*
- **UGA-6 — "Rust runs once per case" (§3.3/D7) violated for `engines:"both"`**:
  `compare_with_result` re-runs the deterministic Rust engine per channel (2×).
  Cost-only (~150 s full gate ≪ 10 min target), disclosed in the scheduler
  comment; caching the capture across channels is not worth the seam. Accepted.
- **UGA-7 — Phase D "≥90% both" target missed (342/514 = 66.5%)**: already
  disclosed in §1i with the seeding-data proof that ~72% is the arithmetic
  ceiling (97 r4133-only-feature cases can never be both; 75 capi-only
  wholesale-divergent stay single-channel per plan). The fingerprintable ~21-case
  flip set remains the follow-up. No action here.
- **UGA-T7 — `skip` entries can never go stale by construction**: a crash cannot
  be observed without sending the deck, and no channel would notice the crash
  disappearing (unlike the old report mode). Inherent to the design; the audit
  re-reproduced #303 live on 2026-07-19. Re-validation is manual:
  `DSS_GATE_SEED_LEDGER=1 DSS_GATE_SEED_ONLY=<case>` sends the deck to r4133 and
  reports. Accepted with this documented procedure.
- **UGA-T9 — P5a `show_busflow_unknown_bus_errors` assert reshape** (exact
  message equality → code 219 + `contains("not found")`): P5a is trusted context
  with its own audit trail; the assert gained the numeric-code dimension and the
  reshape is disclosed in the P5a record. The only text-weaker assert in the
  whole range — noted for completeness, no action.

**Phase E handoff (confirmed, NOT fixed here per brief):** `tools/opendss/*.py`
report scripts still read the deleted `known_diffs.json` / reference
`corpus_live_opendss` — they die in Phase E with the Oddie venv + `bin/r3723` +
`bin/r4088` (junction-safe protocol, CLAUDE.md). Phase F additionally rewrites
TESTING.md/CLAUDE.md (the in-code module docs were already fixed here).

**Gate:** `cargo fmt --all --check` + `cargo clippy --workspace --all-targets
-- -D warnings` + `cargo test --workspace` green at defaults (full BOTH corpus
gate 514/514, all 6 ledger entries hit, fail-on-stale active); `tests/corpus`
pristine; junctions intact.

## 1l. UNIFIED_GATE post-audit fix round (branch `ug-fixround`, 2026-07-19)

Closes the three actionable "deliberately not fixed" items from the §1k pre-E/F
audit triage. Every envelope MEASURED live on both engines (full 510-case ×2 seed +
`DSS_LEDGER_MEASURE` gate runs); R3 discipline throughout — no ledger entry papers
over a fixable port bug.

**Item 1 — fingerprintable follow-up flip set → `engines:"both"`.** Seeded all live
cases ×2 channels, triaged each single-channel case by first-divergence class.
**18 flipped** (both% 342→360 = 66.5%→**70.0%**, near the seeding-proven ~72%
ceiling); 4 measured NOT tightly fingerprintable stay `capi_v0145` (`note` cause):
- **storage/PVSystem 6-sig-fig display** ×12 — r4133 Delphi renders
  kw/kwhstored/%stored/kvar/kwtarget + PVSystem kvar to 6 sig figs, port+pinned-0.14.5
  full f64. `probe` `num_rel` **1e-5** (= 2× the 6-sig-fig class ceiling 5e-6; corpus
  max 4.9e-6 @ peakshave). storagectrl_{peakshave,support,ipeakshave,loadshape,
  chargelow}, midi_storagectrl, invcontrol_storage_vw, expcontrol_basic +
  modes time/{duty,generaltime,generaltime_duty,generaltime_yearly}.
- **injection+element fpc-delphi-ulp** ×4 (IndMach asymmetric) — `injection`
  whole-vector (max_abs 1e-5; seen 4e-6) + `element` currents/powers/losses
  (max_abs 2e-5 incl. a Line loss near-cancellation 9e-6; max_rel 1e-8);
  voltages/yprim/discrete tier-clean. combo/{combo_mesh,midi}_asym,
  indmach/{indmach,midi_indmach}_asym.
- **monitor seq-magnitude drift** ×1 (monitor_seqmag) — 3 `monitor` channel scopes:
  mseq ch4 (V2 mag, max_abs 5e-6, seen 2.4e-6), mseq ch5 (V2 angle, max_abs 1e-3 =
  angular image of the mag floor, seen 5.95e-4), mseqmag ch2 (|V|3, 5e-6). NB the
  ledger `channel_idx` is 0-based (the harness "channel N" display is 1-based).
- **stayed capi_v0145** (measured, not fingerprintable): storagectrl_{time,follow},
  invcontrol_storage_vv_vw, invcontrol_expmodel — r4133 event-log CONTENT differences
  (parallel-build `StorageController1` actor-suffix; `DER`-vs-`PVSYSTEM/STORAGE OUTPUT`
  wording) the trailing-whitespace-only eventlog mask (UGA-T3) cannot normalize.

**Item 2 — defer_ledger retirement (ORPHANED_GAPS §1.9).** 1 of 4 retired; 3
confirmed NOT ledgerable per R3, reasons sharpened. defer_ledger remaining: **7**
(regcontrol_idle + DynExp×2 + NCIM×4 [WP-U1.7]).
- **line_spacing_asym RETIRED** → `engines:"both"`: capi_v0145 exact-pair-NUMERIC
  property+probe scopes (Line.lsp.normamps 730→230, emergamps 1095→345 — the
  min-over-phase D3 upgrade, port CORRECT per r4133 LineGeometry.pas) + r4133 `#303`
  skip. Needed the exact-pair-numeric machinery §1i said was lacking — ADDED to
  `probe_handled`/`property_handled` (num_rel absent + oracle/rust pins ⇒ exact float
  pin, not an envelope). Line.lspc D3-invariant (165/247.5, no divergence).
- **regcontrol_idle STAYS** defer_ledger: the earlier "~7e-5 sub-tap" note was WRONG —
  the live r4133 idle regulator settles MV.1 **~8.7% (623 V)** off the port, WHOLESALE.
  Per R3 an 8.7% gap at the regulated bus smells like a port idle-RegControl bug
  (tap init), not an upstream ulp difference → NOT ledgered; needs a WP.
- **DynExp×2 STAY** defer_ledger (Dynamic_KundurDynExp, GFL_IEEE123 DynExp):
  re-measured — port matches NEITHER oracle (0.14.5 AND r4133 agree with each other,
  port ~1.5e-5 off BOTH). Force-ledgering would pin a port-side DynExp-evaluator bug.

**Item 3 — plan §2.2 all-properties enumeration in dss-epri** (audit UGA-5, dropped
without deviation). Implemented: `DSSElementV` FFI binding (mode 0 = AllPropertyNames)
+ `capture_all_properties` (byte-faithful port of
`oracle_server.capture_all_properties`: `? name.Like` activate → property list →
`? name.prop` values; read LAST per step). The run_case fail-loud is gone. Smoke
`#[test]` proves the round-trip (IEEE13: 38 elements, 1628 property values).
**Gating semantics UNCHANGED**: the scheduler still masks `all_properties` off on the
r4133 request (property parity stays capi_v0145-only per plan) — the r4133 all-props
path is capability-only (report tooling). Unsafe stays inside dss-epri (SAFETY comment;
V-protocol copied immediately). *(Superseded 2026-09-03: R4133_PROPS RP4.1 removed
both masks; this capture is what the r4133 property compare reads.)*

**Ledger measurement aid.** `DSS_LEDGER_MEASURE=1` makes the numeric handlers print
the live divergence per scope (env-gated stderr, NO gating-semantics change) so
envelopes are sized to the measured max — the "every envelope measured live" (R3)
operation, repeatable.

**Ledger:** 25 entries (was 6): +12 storage/pv display, +4 injection-ulp, +1 monitor,
+1 line_spacing exact-pair (capi) + 1 skip (r4133). Structural test green; every entry
HIT (1041 total hits) and non-stale in the full gate.

**Gate (three-command, defaults, green):** fmt clean; clippy clean; `cargo test
--workspace` green — corpus gate 514/514, 25 ledger entries all hit, fail-on-stale
active, population lock regenerated (diff = 18 engine flips + 18 ledger tags), dss-epri
smoke green (all_properties round-trip). `tests/corpus` pristine; junctions intact.

**Post-audit settlement (2026-07-19).** Two independent audits (audit-code, audit-tests)
of the fix round returned FOUR low-severity findings; each settled empirically (live
engines, code path, gating topology), no tolerance/envelope loosened:
- **exact-pair-numeric drift-safety (audit-code F1 / audit-tests F2) — FIXED.** The
  exact-pair-NUMERIC branch only marked stale on `rust==oracle`, so a port regression to
  a THIRD value (still `!= oracle`) with no `rust` pin would pass silently. Added a
  MANDATORY `assert!(sc.rust.is_some(), …)` to both `probe_handled` and
  `property_handled` (mirrors the non-numeric path's mandatory `oracle` guard).
  Strengthening only; the sole live exact-pair-numeric entry (`capi-linespacing-normamps`)
  pins `rust`=230/345, so the gate stays green. Closes the last soft spot in the new
  machinery.
- **all_properties smoke wording (audit-tests F1) — FIXED (wording).** The smoke check
  re-reads the same `? name.prop` getter that built the dump (proves enumeration
  non-empty + getter determinism, NOT value correctness vs an independent baseline).
  Reworded "round-trip verified" → "dump non-empty + getter re-read consistent" and
  expanded the comment: capability-only report tooling per §2.2; the pinned capi oracle,
  not this smoke, gates property correctness.
- **storage-display 1e-5 vs gfm 1e-6 for %stored (audit-tests F3) — NO CHANGE (no
  defect), rationale recorded.** Not an arbitrary looseness: storagecontroller cases are
  `engines=both`, so the **capi channel strict-gates `Storage.%stored` at full f64** via
  `compare_all_properties` (the r4133 storage-display scopes are `probe`, which never
  remove %stored from the capi all-properties compare) — a real port %stored drift is
  caught there. The r4133 `num_rel`=1e-5 only absorbs Delphi's 6-sig-fig display across
  the whole Storage class (incl. large-mantissa kw/kwhstored, ceiling 5e-6). gfm cases
  are `engines=r4133`-only (0.14.5's Isc1 default shifts system-Y ×1000 → capi cannot
  gate), so their 1e-6 is the SOLE %stored check and must be tight. Different gating
  topology, no masking.
- **line_spacing property scopes "inert" (audit-tests F4) — NO CHANGE (not a defect),
  scopes retained.** The finding reads the static lock `props=0`, but `force_properties`
  sets `compare_all_properties=true` at RUNTIME for asymmetric-family Live capi cases
  (`ASYMMETRIC.compare_all_properties=true`; `line_spacing_asym` `engines=both` →
  `gates_capi`). So on the capi channel the deck DOES capture all_properties and both
  `property` scopes (Line.lsp.normamps/emergamps) run and are pinned — confirmed by the
  green gate now that the mandatory-`rust` guard above is active on them.
- Gate re-run recovered from a transient Windows incremental-compilation linker flake
  (`LNK2019` anon.llvm/serde_json symbols, unrelated to the edits) by clearing
  `target/debug/incremental` and rebuilding with `CARGO_INCREMENTAL=0` — green.

## 1m. UNIFIED_GATE Phase E — retire the Python EPRI stack + r3723/r4088 (branch `ug-phase-e`)

`UNIFIED_GATE_PLAN.md` §4-E / §6-E executed: the retired opt-in EPRI-python
channel (Oddie bridge, dss-python 0.16.0b2/backend wheels, r3723/r4088 binaries)
is gone. The unified gate's two live oracles are unchanged — `capi_v0145`
(pinned dss-python via `tools/oracle/oracle_server.py`) and `r4133` (in-house
`crates/dss-epri` bridge over the git-tracked `bin/r4133` DLL).

**Deleted (git rm, 27 files):** `tools/opendss/` scripts
`ab_compare.py`, `smoke.py`, `dsspy_crosscheck.py`, `gen_ad_reference.py`,
`probe_59n.py`, `sweep_modes_isolated.py`, `sweep_merge.py`, `xcheck_bridge.py`;
`dsspy_validation/` (5 files); `wheels/` (2 beta wheels + SHA256SUMS);
`PIN_OPENDSS.txt`; `bin/r3723/**` (5) + `bin/r4088/**` (5).

**Pruned to r4133-only:** `revisions.json`, `bin/SHA256SUMS` (verified
`sha256sum -c` OK), `bin/README.md`, `vendor_binaries.py` (REVISIONS dict +
README template), `README.md` (rewritten: r4133 artifact + Rust `epri-worker`
bridge + re-vendor procedure). `oracle_server.py` pruned to the `capi` engine
only — removed the `capi015`/`oddie` `make_engine` arms, `_oddie_get_y_sparse`,
`_read_pin_opendss`, `OPENDSS_DIR`/`REPO_ROOT`, the Oddie `capture_eventlog`
export-CSV branch, and the `clear` cmd (only the deleted `xcheck_bridge.py` used
it); `DSS_ORACLE_ENGINE` now accepts only `capi` (anything else exits non-zero,
no silent pass). Stale comments referencing the deleted harness fixed in
`corpus_guard.py` and `epri-worker.rs`.

**rg sweep verdict** (`rg -i "oddie|capi015|r3723|r4088|dss_python_backend|0.16.0b2"`):
no LIVE code/config wiring to the retired channel remains — no manifest carries an
`oracle:"r3723|r4088|capi015"` field (all gating is via `engines`, values
`both`/`capi_v0145`/`r4133`); no executable reference (import/spawn/config) to any
deleted script survives; CI/`Cargo.toml`/`.gitignore` clean. All remaining matches
are legitimate-survivor classes: (a) historical docs (STATUS, `docs/plans-archive`,
`docs/upgrade/*`, `docs/phase-records/*`, `docs/wasm/*`) and root plan files
(`*_PLAN.md`, `PLAN_SEQUENCE.md`, `README.md`); (b) `CLAUDE.md`/`TESTING.md` (Phase
F rewrites these); (c) behavioral-spec / oracle-provenance citations in
`crates/dss-core/src/**` and test docs (which oracle calibrated a value —
`capi015`/`oddie:r4133`/`r4088`, analogous to STATUS records, not live wiring);
(d) golden generators `tools/golden/gen_*.py` (UNTOUCHED per the gate rules; frozen
manual tooling. NB — corrected at Phase F settle per re-review finding F2: the
original "gen_protection.py … still valid" wording here was overstated. The r4133
`ODDIE_SCENARIOS` arm of `gen_protection.py` and ALL of `gen_flicker.py` need
`from dss import IOddieDSS`, absent in pinned 0.15.7, and their environment —
Oddie venv, 0.16.0b2 wheels, `PIN_OPENDSS.txt`, r3723 binaries — was deleted in
this phase: they are frozen dead paths; regen would need a git-history restore.
Goldens are frozen so the gate is unaffected. See TESTING.md §Frozen historical
generator arms);
(e) frozen corpus fixtures — `.dss` deck comments + manifest `note`/`ledger` cause
provenance; (f) committed AD trusted-baseline data `tests/data/adiakoptics/r3723_ref/`
+ its PROVENANCE.txt; (g) WASM/FPC ABI lineage (`tools/fpc/usermodel_abi`,
`docs/wasm`, `WASM_USERMODELS_PLAN.md`) referencing the vendored SOURCE tree
`electricdss-code-r3723-trunk`, not the retired binary channel; (h) retirement-guard
assertions in `corpus_gate/engines.rs` that assert the pinned ping never reports an
`oddie`/`capi015` marker (they enforce the retirement and stay green).

**venv:** `tools/opendss/.venv` in main is already empty (2026-07-19 incident);
the coordinator removes that empty dir in MAIN separately (junction-safe, CLAUDE.md).
This worktree's `.venv` is a junction — not touched.

**Settlement (two audits, `ae3e6bd..c060c0b`):** five low-severity findings, all
non-gating; three fixed, two recorded non-fixes:
- *Fixed* — `tools/opendss/README.md`: added the plan-required (§1.1) note that
  the frozen A-Diakoptics baseline (`tests/data/adiakoptics/r3723_ref/`, consumed
  by `ad_reference.rs`) has no regen tool anymore and must be reimplemented over
  `epri-worker` if ever re-run (harvester `gen_ad_reference.py` was deleted).
- *Fixed* — `tools/oracle/README.md`: this KEPT live doc still described the
  retired `DSS_ORACLE_ENGINE=oddie` rebind, the "EPRI/Oddie channel", and the
  renamed `corpus_live.rs`/`corpus_live_opendss`; rewritten to match the pruned
  `oracle_server.py` (capi-only, exits non-zero on any other engine).
- *Fixed* — Gate paragraph below now carries exact counts/exit-codes/wall-clock.
- *Non-fix (recorded)* — `tools/golden/gen_checkpoints.py`'s `capi015` regen arm
  (`_read_pin_opendss`) still references the deleted `tools/opendss/PIN_OPENDSS.txt`.
  Left UNTOUCHED per the binding gate rule (golden generators `tools/golden/gen_*.py`
  are frozen): it is a **dead path** — reachable only via `DSS_ORACLE_ENGINE=capi015`,
  whose engine (dss-python 0.16.0b2) was retired here, and the four `"oracle":"capi015"`
  goldens are frozen. The live `capi` gate path never touches it. No gate impact.
- *Non-fix (deferred)* — `CLAUDE.md` + `TESTING.md` still describe the retired
  opt-in channel as live. Plan §4-F explicitly defers rewriting both to **Phase F**
  (survivor class (b) below); a forward-deferral to verify Phase F completes, not a
  Phase E defect.

**Gate** (defaults, worktree `wtE` @ settlement, both oracle channels live):
- `cargo fmt --all --check` → exit 0.
- `cargo clippy --workspace --all-targets -- -D warnings` → exit 0.
- `cargo test --workspace` → exit 0: 1893 passed, 0 failed, 2 ignored across
  58 test binaries; wall-clock ~172 s.
`tests/corpus` pristine (`git status tests/corpus` clean); junctions intact.

## 1n. UNIFIED_GATE Phase F — docs + final acceptance (branch `ug-phase-f`)

`UNIFIED_GATE_PLAN.md` §4-F Scope A (docs) + §6 final acceptance (clean-clone
gate) executed. The fix-round re-review (Phase F brief Scope B) ran as an
independent parallel audit — its findings settle in its own record, not here.
Base `20d03f7`.

**Docs rewritten — every claim re-verified against the live code, none copied
from stale prose:**

- **TESTING.md** (full rewrite): layer map = unit / golden / **unified corpus
  gate** / corpus hygiene (the opt-in EPRI row is gone); new sections for the
  gate architecture (one `#[test]`, 514 cases = 293+47+105+69, `engines`
  both=360 / r4133=96 / capi_v0145=58, `defer_ledger`×7, `isolate`×33, module
  tree, per-case worker recycle default 1) and the divergence ledger (3 kinds,
  the a/b/c envelope clauses, exact-pair pins incl. the mandatory `rust` pin
  on exact-pair-numeric, the 9 implemented scope fields, fail-on-stale +
  never-applied + hit accounting, lock `id@digest` tags, manual `skip`
  re-validation); procedures rewritten/added: add-a-corpus-case (`engines`
  discipline), **triage-a-divergence-into-the-ledger** (R3 rules: prove
  not-a-port-bug first, measure with the seed report / `DSS_LEDGER_MEASURE`,
  commit ledger + lock together), **re-vendor the r4133 binary**, **run the
  seeding report**; env-var table re-verified knob-by-knob against
  `corpus_gate/{scheduler,engines,ledger}.rs` + `oracle_server.py` (NB: no
  `DSS_GATE_POOL` exists — pool size is derived `max(2, jobs/2)`, documented
  as such); frozen historical generator arms documented (`gen_checkpoints.py`
  capi015 arm referencing the deleted `PIN_OPENDSS.txt` — the Phase E recorded
  non-fix, now documented instead of edited — plus `gen_bh_capi015`/
  `gen_regcontrol_capi015`/`gen_fuse_r4133`); golden-family table refreshed
  (`json_import/`, `gen_schema.py` rows added).
- **CLAUDE.md**: project identity reframed — the 1:1 port is the FINISHED
  stage (final acceptance 2026-07-11); direction = pure idiomatic Rust / wasm
  user models / new methods & models / r4133-and-beyond, with PORTING_PLAN.md
  as the historical record; the invariant list updated: the `dss-epri` unsafe
  carve-out is the sole `forbid(unsafe_code)` exception (matches
  PORTING_PLAN.md §scope note + `dss-epri/src/lib.rs`); the stale
  opt-in-Oddie bullet replaced by the two-channel unified gate + ledger; the
  gate section now describes `corpus_gate.rs`
  (`corpus_gate_all_cases_match_engines`, both oracles mandatory,
  Windows-only r4133 bridge, ledger fail-on-stale); the TODO(compat) wipe
  pointer retargeted to DE_PASCALIZE Stage F. Ritual step-0, the (freshly
  hardened) worktree junction protocol, conventions, and the MCP section
  untouched.
- **tools/opendss/README.md** (Phase E rewrite reviewed; gaps fixed): the
  "editor/registry suppression inside the bridge" claim was FALSE against the
  code (`rg RegistryUpdate|rundll32` over `dss-epri` = no match; that pair
  belonged to the retired Oddie `make_engine` and survives only in the frozen
  `gen_protection.py`) — replaced with the real mechanism (`DSSI(8,0)` ⇒
  `NoFormsAllowed`, `dss.rs`); added the never-`FreeLibrary` rule (the Phase A
  deadlock root-cause) and a "Gate wiring" section (worker resolution order,
  ping markers, ledger pointer, all-properties = capability-only).
- **Surgical de-staling of other live docs still naming retired machinery**
  (small justified scope addition): root `README.md` (deleted
  `known_diffs.json` → gating ledger; `corpus_live.rs` → `corpus_gate.rs`;
  opt-in-channel paragraph → two-oracle gate; forbid-scope wording),
  `tests/TOLERANCE_NOTES.md` (the plan-§7 "**ledger is not a tolerance**"
  paragraph added; `corpus_live`→`corpus_gate` renames; the Oddie-eventlog
  paragraph rewritten to the `dss-epri` CSV/BOM capture; EVENTLOG_MASKS keying
  updated to channels), `tests/corpus/README.md` + `COVERAGE.md`,
  `tools/oracle/{oracle_server,corpus_guard}.py` comment renames. Golden
  generators, goldens, tolerances, ledger entries, manifests: UNTOUCHED.

**Wall-clock before/after (plan §3.4 / §6 — consolidated from the phase
records):**

| point | mode | corpus-gate share | full `cargo test` |
|---|---|---|---|
| Phase 0 baseline (`pre-unified-gate` 449c745, loaded box) | serial one-shot, 1 channel | 292.6 s | 426.6 s |
| Phase B settle (capi_v0145 only; incl. dump write; serial ref 341.9 s) | persistent parallel, jobs 16 / pool 8 | 104.3 s | — |
| Phase C (r4133 channel live, 514 single-channel cases) | persistent parallel, jobs 16 / pool 8 | 64.5 s (settle build 67.4 s) | — |
| Phase D (full BOTH gate + ledger) | persistent parallel, recycle=1 | ~150 s (137–160) | — |
| Phase E settlement (warm build) | defaults | — | ~172 s (1893 pass / 2 ignored / 58 binaries) |
| Phase F worktree `wtF` (cold build; warm re-run 162 s) | defaults | — | 365 s (fmt 2 s, clippy 51 s) |
| Phase F clean clone (cold build, the §6 acceptance run) | defaults | — | 303 s (fmt 5 s, clippy 50 s) |

Net: from a 292.6 s serial single-channel corpus pass to a ~150 s
**two-channel** (BOTH-gated, ledger-checked) pass — roughly double the oracle
coverage at half the wall-clock, ≪ the plan's ≤10 min target.

**Clean-clone gate (plan §6 final acceptance) — GREEN.** `git clone --branch
ug-phase-f --single-branch e:/RustProject/dss-rs <Temp>\claude\dss-clean` at
head `cacb388`; clean checkout, **no `.inputs`, no venv** — the git-tracked
r4133 DLL + the system-python pinned oracle (verified `dss-python 0.15.7`) are
the only external deps, which IS the plan's proof. Three-command gate at
defaults: fmt exit 0 (5 s) / clippy `-D warnings` exit 0 (50 s) / `cargo
+stable test --workspace` exit 0 (303 s, both oracle channels live). The only
residue was the known §1g CorpusGuard export-CWD corner (untracked
StorageControllerTechNote CSVs, throwaway clone). NB a first attempt cloned
under the deep per-session scratchpad and **failed at checkout on Windows
MAX_PATH** (121-char root + the 166-char longest corpus path = 287 > 260;
`core.longpaths` is unset by default, and the Delphi DLL's file I/O is not
long-path-aware anyway) — a clean clone must sit at a short root, as a normal
user clone does.

**Tooling note:** the TortoiseSVN CLI tools were installed machine-globally on
2026-07-19 for the `.inputs` re-vendor (svn peg-revision checkouts of the EPRI
SVN source trees after the second junction-wipe incident).

**Gate (worktree `wtF`, defaults, both channels live):** fmt exit 0 (2 s);
clippy `-D warnings` exit 0 (51 s); `cargo +stable test --workspace` exit 0
(365 s, cold build). `tests/corpus` pristine after runs (path-limited clean of
the known §1g export-CWD leftovers); junctions intact. The `unified-gate-v1`
tag is created at settle on the final integrated head (deliberately not in
this record).

### Phase F settle — three audits + the Scope B re-review (2026-07-19)

Three independent reviews returned: **audit-code** (docs, 4 findings),
**audit-tests** (acceptance, 4 findings), and the **user-mandated fable
re-review of the opus fix round** (Scope B — the "independent parallel audit"
the record above pointed at; it materialized, verdict below). Both doc audits
independently re-verified essentially every TESTING.md/CLAUDE.md claim against
the live code and found the rewrites honest; audit-tests additionally
corroborated the clean-clone acceptance in a second fresh clone (fmt/clippy
exit 0 reproduced; its `cargo test` was still running clean at report cutoff —
the phase's own clean-clone run above is the §6 acceptance evidence).

**Scope B re-review verdict: PASS, nothing gating.** 11 of ~19 new ledger
envelopes plus the gfm class re-derived from the LIVE engines with an
independent protocol driver: every envelope is a measured upstream divergence
(Delphi 6-sig-fig display rendering, whole-model fpc-vs-delphi transcendental
ulp, seq-transform drift, the discrete normamps min-over-phase jump, hard #303
crashes); measured provenance values reproduce digit-for-digit (4.900e-06,
1.313e-06, 4.05e-06, 5.95e-04, 1.62e-06 …); the storage-display 1e-5 vs gfm
1e-6 split is empirically justified (mantissa-class ceilings 4.95e-6 vs
8.8e-7; the capi channel strict-gates the same probes at full f64); the two
R3-critical non-retirements are correct — regcontrol_idle is a genuine ~8.8 %
/ 620 V wholesale divergence (r4133 taps to 15 despite `idle=yes`; the port
holds tap 1.0 = the retired capi015 semantics — the follow-up WP should check
the dss_capi-0.15-vs-EPRI idle delta before assuming a port tap-init bug), and
Kundur DynExp is matches-neither (the two oracles agree to 4.8e-10 while the
port is 1.52e-5 off BOTH — ledgering it would have hidden a potential port-side
DynExp-evaluator bug). Eventlog stay-capi divergences verified real on live
r4133 (actor-suffix `StorageController1.`, STORAGE/DER wording). All-props
confirmed capability-only (scheduler masks it off every r4133 request; FFI
copies immediately, SAFETY-documented). Informational: 13 of 20 ledger causes
are channel-narrowing documentation with no current entry (Phase D heritage).

**Finding dispositions (all settled empirically at settle, in this commit):**

- *PF-1 / F-PHF-1 (Scope D silently dropped) — CONFIRMED, fixed.* The phase
  agent skipped brief Scope D; all four items are now done: **F1**
  `tools/corpus/README.md` no longer claims `dsspy_crosscheck.py` "moved"
  (deleted, Phase E); **F2** TESTING.md's frozen-arms list now includes
  `gen_flicker.py` (entirely Oddie/r3723) and `gen_protection.py`'s
  `ODDIE_SCENARIOS` arm, the regen procedure scopes itself to capi arms, and
  the §1m survivor-class (d) overstatement is corrected in place; **F3** the
  `probe_59n.py` reproducibility regression is recorded (TESTING.md §Retired
  probe scripts — the relay/tests.rs + skipped-manifest citations are
  historical; manifest/test text untouched per the brief); **F4** the five
  remaining stale `xcheck_bridge.py` present-tense comments in `dss-epri`
  (epri-worker.rs, lib.rs, capture.rs, dss.rs ×2) now say the cross-check was
  retired with its stack.
- *PF-2 — CONFIRMED, fixed* (the F2 items above).
- *PF-3 / F-PHF-2 (Scope B existence) — resolved:* the re-review ran and its
  record is this section.
- *PF-4 (env table incomplete) — CONFIRMED, fixed:* `DSS_EPRI_ACTOR_TIMEOUT_SECS`
  (default 300, `dss.rs::wait_for_actor`), `DSS_EPRI_DLL`, `DSS_EPRI_EXPECT`
  (smoke overrides) added to the TESTING.md table.
- *F-PHF-3 (tag) — done at settle:* annotated `unified-gate-v1` created on the
  final head as the last act (plan §6).
- *F-PHF-4 — noted:* the audit's corroboration clone was cleaned up; its gate
  was green through fmt/clippy and mid-`cargo test` (zero failures observed)
  at cutoff.
- *RR-1 (non-numeric exact-pair oracle-only pin hole) — CONFIRMED, fixed
  (tightening, latent — no live entry exercises the path):* the `rust` pin is
  now MANDATORY in the non-numeric probe/property arms of
  `corpus_gate/ledger.rs` (mirroring the fix-round F1/F2 numeric fix), and
  `assert_structural` now statically rejects any exact-pair scope (no
  `num_rel`) without a `rust` pin — the drift hole is closed in both arms and
  at load time.
- *RR-2 (skip provenance text) — CONFIRMED, fixed:* the
  `r4133-linespacing-asym-303` ledger `source` now records the live-re-derived
  crash site (compile of the `tscables=[…]` line, offset 41CE5E — not calcv,
  which is the IEEE13_LineSpacing sibling). Free-text-only change; the skip
  itself was re-derived correct. `population.lock.json` regenerated via the
  sanctioned `DSS_UPDATE_POPULATION_LOCK=1` path (the entry digest covers the
  full serialized entry by design); diff verified to touch only that case's
  ledger tag.

Settle gate + the `unified-gate-v1` tag: recorded in the commit that carries
this section (gate results in the final report).

## UNIFIED_GATE Phase A — `dss-epri` bridge + `epri-worker` + smoke + xcheck (session record)

New test-only crate `crates/dss-epri` (`publish = false`) — the **only** crate
without `#![forbid(unsafe_code)]` (carries `#![deny(unsafe_op_in_unsafe_fn)]` +
`#[cfg(windows)]` + module `// SAFETY` docs; carve-out documented in
`PORTING_PLAN.md` §1). It drives the official EPRI `OpenDSSDirect.dll` (r4133) via
`libloading` as a second live oracle (UNIFIED_GATE_PLAN.md R1/§2). Layout: `ffi.rs`
(raw `cdecl` externs + load with `LOAD_WITH_ALTERED_SEARCH_PATH` so sibling
`KLUSolve.dll` resolves), `dss.rs` (command + V-protocol decode + error polling),
`capture.rs` (`CaseResult` assembly, byte-mirroring `oracle_server.py` +
`gen_checkpoints.py`), `guard.rs` (corpus-guard port), `src/bin/epri-worker.rs`
(persistent `ping`/`run`/`quit` line-JSON worker + `--smoke`). Root `Cargo.toml`:
added the workspace member, `libloading` workspace dep, and the dev opt-level-3
override.

**Root-cause war story (load-bearing, do not re-litigate).** The DLL deadlocked
when driven from Rust — every call appeared to hang. Diagnosed by minidump: the
process parks in `NtUserMsgWaitForMultipleObjectsEx` inside `OpenDSSDirect.dll`.
The trigger is **`FreeLibrary` on `libloading::Library` drop**: the r4133 DLL's
unit finalization tears down its Delphi solver **actor thread** through a
message-pumping `TThread.WaitFor` that never completes headless (no VCL
`Application`/`WakeMainThread`). The hang *looked* mid-execution only because the
`Engine` (local in `run_smoke`) dropped — and FreeLibrary'd — before the report
printed. The Python/Oddie host never hit it because interpreter shutdown doesn't
`FreeLibrary` the DLL. **Fix:** never unload — `Dll::leak()` (`mem::forget` the
`Library`); the OS reclaims it at process exit (actor thread already terminated,
so DllMain-detach is clean). Confirmed by a minimal raw-FFI reproducer:
libloading-load hangs at drop, `mem::forget(lib)` cures it (single-thread, no
message pump needed). All the earlier COM/FP/message-pump/two-thread experiments
were chasing the wrong symptom and were reverted.

**Capture-decode notes** (verified against raw-DLL ctypes + Oddie probes):
V-protocol `mySize` is always **bytes**; type tags 1=int / 2=double / 3=complex
(flat re/im) / 4=string / 5=byte-stream. String arrays: strip **one** trailing
`\0`, split on `\0`, then lstrip a single leading space from element 0 (the Oddie
monitor-header first-column artifact — `[' V1', ...] → ['V1', ...]`; no other
array's element 0 has a leading space, so applying it universally is a no-op).
Monitor channels decoded from the raw `ByteStream` exactly like `IMonitors.Channel`
(272-byte header, `record_size = int32@offset8 + 2`, f32 records). The DDLL solve
is **async** (dispatches `SIMULATE` to the actor and returns), so `solve()` polls
`ParallelV(1)` = `ActorStatus` until done.

**DONE-bar evidence:**
- `epri-worker --smoke` (10/10 runs, exit 0, stderr→/dev/null):
  `version OK: Version 11.0.0.1 (64-bit build) - Charlottesville`
  `IEEE13 solved: 41 nodes, 2 iterations`
  `CSC export OK: n=41, nnz=267, voltages bit-identical (solution-neutral)`
  `getIpointer OK: len=84 (= 2*(NumNodes+1))`
- Smoke `#[test]` (`crates/dss-epri/tests/smoke.rs`) runs under `cargo test
  --workspace`, oracle-free.
- **xcheck** (`tools/opendss/xcheck_bridge.py`, temporary — Phase E deletes it):
  drives all 396 cases of the r4133 `corpus_live_opendss` universe (240
  solvable_now + 43 asymmetric + 66 controls + 47 modes; #303 `skip` decks
  excluded identically both sides) through the Python/Oddie r4133 engine AND the
  Rust `epri-worker`, bit-diffing the raw `CaseResult` — twice, second pass
  order-shuffled (seed 1337). Result (settle re-run, ~4m50s wall): **396 matched
  of 396** on pass 1 (manifest order) AND pass 2 (shuffled) → `XCHECK PASS: both
  passes bit-identical over 396 cases`, exit 0. `diverged = ok_mismatch =
  both_err = 0` (matched == universe). `tests/corpus` verified pristine after
  (CorpusGuard on both sides).
- fmt + clippy (`-D warnings`) clean on `dss-epri`.

**Audit dispositions (two independent audits of the phase diff).**
- **F1/A1 — `both_err` not gated in `xcheck_bridge.py` (real, FIXED).** The PASS
  boolean was `clean = not diverged and not ok_mismatch`, so a case that errored
  symmetrically on BOTH engines was silently absorbed — a latent fake-success
  channel (arithmetically inert for the reported clean run, but a weaker guarantee
  than the DONE bar). Strengthened to `... and not both_err`: the universe excludes
  solve-abort/pending cases, so every case must yield a comparable `CaseResult` on
  both engines; a symmetric error is a hole, not a pass. Empirically settled — the
  full re-run above with the stricter gate still PASSES 396/396 (both_err = 0), so
  the fix closes the channel without any false failure. Strengthening only; no
  tolerance/assertion weakened.
- **F2 — additive `clear` command in `oracle_server.py` (real deviation,
  ACCEPTED, no change).** UNIFIED_GATE_PLAN D8/§3.2 noted "no protocol change
  needed server-side." Phase A added a `clear` handler (release the circuit + any
  held loadshape MMF handle so the two processes can compile the same case). It is
  purely additive and non-breaking: `corpus_live.rs` never sends `clear`; `run`/
  `ping`/`quit` are untouched; `oracle_server.py` is not in the brief §4 "untouched
  consumers" list. Used only by the temporary `xcheck_bridge.py` (Phase E deletes
  both the tool and its need for the command). Kept as a justified, harmless
  deviation.
- **F3 — universal leading-space strip on string-array element[0] (not a bug,
  documented).** `decode_string_array` lstrips one leading space from element[0]
  of every V-protocol string array. Settled empirically + by grammar, not by
  universe coincidence: the other arrays (node order, element/register/variable
  names, zone lists) are whitespace-delimited DSS identifiers that can never begin
  with a space → the strip is a guaranteed no-op on them; the monitor CSV header
  (the one array whose first token carries a leading space) is exactly the intended
  target. Confirmed bit-for-bit against Oddie over all 396 cases. Doc comment
  strengthened to record the grammar guarantee. No behavior change.

Follow-ups (STATUS, non-blocking): none block Phase A. Phase E removes
`xcheck_bridge.py` and, with it, the `oracle_server.py` `clear` handler's only
consumer (drop the handler then).
