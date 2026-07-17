# A-Diakoptics sweep — `off:unclassified-new-deck` classification round (WP-T2)

**Date:** 2026-07-17 · **Branch:** `wt-t2` · **Scope:** the 9 `tests/corpus/manifests/ad_sweep.json`
entries carrying the placeholder disposition `off:unclassified-new-deck`.

## Why

`corpus_ad_matches_normal_mode` (rust-vs-rust A-Diakoptics vs normal-mode sweep) requires
every `solvable_now` entry point to carry an evidence-backed AD disposition. Nine decks —
promoted into the sweep after WP-AD.4 at the part2→update integration merge — sat on the
`off:unclassified-new-deck` pending placeholder (a bucket recorded as "NOT a measured
verdict"). This round runs the documented `DSS_AD_CLASSIFY` + `DSS_AD_DECOMPOSE` procedure
over exactly those 9 and replaces each placeholder with a measured taxonomy reason.

## Procedure (as documented in `crates/dss-core/tests/corpus_live.rs`)

1. `DSS_AD_CLASSIFY=1 cargo test -p dss-core --test corpus_live corpus_ad_matches_normal_mode
   -- --nocapture` — probes every case: compile + controls-off snapshot on the plain engine
   (`ad_solve_normal`) and the AD preamble (`ad_solve_ad`: `Num_SubCircuits=2` +
   `set ADiakoptics=True` + AD snapshot). Emits `ADCLASSIFY<TAB>path<TAB>proposal<TAB>gap<TAB>detail`.
2. For any `off:ad-gap` deck, `DSS_AD_DECOMPOSE=<rel-path>` splits the AD-vs-normal gap into
   the D7 legs: **leg1** = save round-trip (original-normal vs saved-`Master_Interconnected`
   solved normally) and **leg2** = the AD leg proper (saved-interconnected-normal vs AD).
3. `normal-fail` decks were re-probed with `maxiterations` raised (throwaway test, deleted)
   to reach the tear/AD stage the default-15/30-iteration snapshot could not, exposing the
   *real* AD blocker rather than the harness convergence artifact.

The classify probe is rust-vs-rust (no oracle). Remaining `ad-*` class members are attributed
by the shared mechanism the manifest documents (per-deck official-AD replay is the tracked
WP-AD.5 task); the mechanism here is directly observed (mesh / singular zone-matrix / ZLL
build failure), not merely inferred.

## Per-deck evidence and verdict

| # | deck | classify proposal | verdict | evidence |
|---|------|-------------------|---------|----------|
| 1 | `Test/indmachtest/Master.DSS` | `off:ad-gap` 2.533e-3 @ B1.4 | **`off:ad-floor-above-tier`** | decompose: leg1 save = **6.337e-5** (clean, « tier) / leg2 AD = **2.469e-3** (carries the whole gap) @ neutral node B1.4; total 2.533e-3. Clean leg1 rules out save-roundtrip; the residual is the AD stitch leg proper, just above the 2e-3 `AD_SWEEP_TIER` (same class as IEEE34Mod1 leg2=2.1e-3). NOT tolerance-widened. |
| 2 | `…/GFM_IEEE8500/Master-unbal.dss` | `pf` 7.153e-6 @ SX3048214A.2 | **`pf`** | plain unbalanced 8500-node feeder (the GFM inverters live in the `Run_*GFMSnap` scripts, not this master). AD-vs-normal node-V gap 7.2e-6 « tier. |
| 3 | `…/GFM_IEEE8500/Master.dss` | `pf` 6.844e-6 @ SX2862616C.1 | **`pf`** | plain balanced 8500-node feeder. Gap 6.8e-6 « tier. |
| 4 | `…/Stevenson/StevensonPflow-3ph.dss` | `off:normal-fail` (default 15 iters) | **`off:ad-switched-divergence`** | normal converges in **121** iters (needs `maxiterations`>15). Re-probed: AD-init **succeeds** but the **AD snapshot does not converge**; partitioning stats = reduction **0%**, max imbalance **80%**. 5-bus meshed transmission (lines 1-2/1-4/1-5/2-3/2-4/3-5) — a single link cut cannot separate a mesh; identical mechanism to `civanlar`. |
| 5 | `…/Stevenson/StevensonPflow.dss` | `off:normal-fail` (default 30 iters) | **`off:non-3ph-cut-only`** | normal converges in **120** iters. Re-probed: AD-init **fails at "Building ZLL…Error"**. 1-phase positive-sequence model (`set cktmodel=pos`, all lines `phases=1`) → no 3-phase line to form the ZLL link block. |
| 6 | `…/4wire-Delta/Kersting4wire_Lagging.dss` | `off:probe-panic` `row < self.n && col < self.n` | **`off:ad-singular-zone`** | normal solve **converges** (iter=3, 12 nodes). AD arm **panics** on the CMatrix singular zone-matrix `debug_assert!(row < self.n && col < self.n)` (`support/cmatrix/mod.rs:77`) — an ungrounded-wye/delta 4-wire tear isolates a singular child zone. Byte-identical mechanism to the NEV decks (`off:ad-singular-zone`). |
| 7 | `…/4wire-Delta/Kersting4wire_Leading.dss` | `off:probe-panic` (same) | **`off:ad-singular-zone`** | structurally identical to #6 (only the active single-phase transformer differs — XfmrBC vs XfmrCA); same normal-OK / AD-singular-panic. |
| 8 | `…/IEEETestCases/8500-Node/Master-unbal.dss` | `pf` 7.153e-6 @ SX3048214A.2 | **`pf`** | the standard unbalanced 8500-node feeder (same circuit as #2). Gap 7.2e-6 « tier. |
| 9 | `…/IEEETestCases/IEEE 30 Bus/Master.dss` | `off:normal-fail` (default 15 iters) | **`off:too-small`** | normal converges in **19** iters. Re-probed: AD-init **fails at "Setting up the Actors…Error / One or sub-systems cannot be compiled"** — the meshed 30-bus transmission tear cannot form solvable ≥2-bus zones. Same circuit and verdict as `Run_IEEE30.DSS` (already `off:too-small`). |

Note (#6/#7): the `IndMach012a` UserModel DLL is unloadable in safe Rust and falls back to the
built-in induction-machine model on **both** arms — irrelevant to the classification, which is
driven by the singular tear geometry, not the machine model.

## Outcome

- **0** `off:unclassified-new-deck` entries remain in `ad_sweep.json` (was 9).
- 3 promoted to `pf` (now gate-verified live by `corpus_ad_matches_normal_mode`, which solves
  each both ways and asserts the gap < `AD_SWEEP_TIER` = 2.0e-3).
- 6 given a specific `off:` taxonomy reason, each drawn from the existing `AD_OFF_REASONS`
  allowlist — no new reason invented.
- **`unclassified-new-deck` is retained** in `AD_OFF_REASONS`: the `ad_sweep.json` bucket is
  now empty, but the `asymmetric`/`controls`/`modes` synthetic family manifests still carry the
  placeholder on many decks (out of scope for this round — those are classified separately).
  The allowlist entry therefore cannot be retired yet.
- No tolerance was widened (CLAUDE.md §5 no-fudging): `ad-floor-above-tier` and the mesh /
  singular-zone / ZLL classes are recorded as `off:` because there is no correct AD answer to
  gate against, not by relaxing the tier.
