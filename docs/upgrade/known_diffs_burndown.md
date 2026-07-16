# `known_diffs.json` burn-down — Rung-1 exit (WP-U1.10)

Per `UPGRADE_PLAN.md` §1.6, `tests/corpus/known_diffs.json` is the **progress
inventory in reverse**: it catalogs every legitimate Rust↔EPRI divergence the
opt-in EPRI channel (`DSS_LIVE_OPENDSS=<rev>`) surfaces, and each Rung-1/2 WP
that ports a delta retires the entries that delta explains. The rung exit
(`DSS_LIVE_OPENDSS_ASSERT=1`) must go **green** — every remaining divergence is a
justified catalog entry (dss-ext deliberate, cross-solver floor, or an
oracle-can't-run skip) or a Rung-2 item; **zero unexplained**. This file records
that state at the Rung-1 exit.

The catalog is **NEVER** consulted by the mandatory gate or its tolerances — it
partitions only the report-only EPRI channel.

## The Rung-1-exit sweep (WP-U1.10, 2026-07-16, port @ `859cebc`)

Both official-EPRI revs were swept (the same corpus universe as the mandatory
gate; 71 target-rev `oracle:capi015` cases excluded — they are gated against
their own target in the mandatory gate):

| sweep | matched | known-diverged (before → after) | known-skipped | NEW: before → after cataloging |
|---|---|---|---|---|
| `r4088` (10.2.0.1, the rung-1 EPRI line) | 313 | 62 → 113 | 0 → 4 | **55 → 0** (ASSERT green) |
| `r3723` (9.8.0.1, the port's original calibration) | 273 | 124 → 156 | 0 → 1 | 33 → 0 (ASSERT green) |

The `r3723` sweep is informational (run for the prune criterion + to confirm the
new-deck classes are rev-independent); the **rung-exit gate is `r4088` ASSERT**.

### Why none of the 55 is a Rung-1 regression (the proof spine)

Every swept case is **also** in the mandatory gate (`solvable_now` + the
`asymmetric`/`controls`/`modes` families, compared against the pinned dss_capi
**0.14.5** oracle), and the mandatory gate is **green**. So the port equals the
pinned FPC oracle on all 55. Therefore the r4088 gap is purely the additional
**FPC(0.14.5)↔Delphi(r4088)** layer — independently corroborated by the committed
`docs/upgrade/sweeps/capi015_vs_r4088.md` engine-to-engine sweep, which shows the
**same** decks diverging (autotrans reg-tap, makeposseq, reduce, IEEE_519,
gendispatcher). A Rung-1 regression would have turned the mandatory gate **red**;
it is green. The classic power flow, injection assembly, meter zone bookkeeping,
harmonics and reduction paths are byte-identical r3723=r4088 (see
`delta_r3723_r4088.md`), which is why the numeric floors carry the same magnitude
against both EPRI revs.

## Entry ledger at Rung-1 exit (22 entries)

### Retained from the original r3723 triage, unchanged

| entry | revs | class |
|---|---|---|
| `epri-invcontrol-maxiter` | r3723 | EPRI 9.8 InvControl non-convergence (r4088 fixed it; port has the fix) |
| `invcontrol-fixpoint-drift` | r3723 | EPRI 9.8 InvControl fixpoint drift (vendored `InvControl/` decks) |
| `monitor-header-whitespace` | r3723/r4088/r4133 | EPRI pads monitor header with a leading space |
| `property-format-brackets` | r3723/r4088/r4133 | dss_capi renders array PropertyValue with brackets |
| `eventlog-trailing-space` | r3723/r4088/r4133 | EPRI trailing space after relay action text |

### Extended to r4088 by this WP (the divergence persists on the unchanged path)

| entry | revs | r4088 hits | note |
|---|---|---|---|
| `iteration-count-delta` | +r4088 | 4 | now also AutoTrans+RegControl (6-vs-3) & GenDispatcher (20-vs-4/31-vs-5) control-iteration deltas |
| `storage-kwhstored-drift` | +r4088 | 6 | kWhStored idling-loss integral drift (rel ~2e-7); the `.kw` dispatch-precision probes are a distinct class, split into `storage-kw-display-precision` below |
| `injection-fpc-delphi-ulp` | +r4088 | 4 | IndMach asymmetric injection ~3.9e-6 (identical magnitude vs r3723) |
| `meter-zonepce-count` | +r4088 | 6 | EPRI ZonePCE off-by-one (+energymeter +autoadd) |
| `harmonics-yfingerprint-drift` | +r4088 | 1 | IEEE_519 trace.im 2.891e-5 (identical vs r3723; the r4133 IEEE_519 move is Rung-2) |

### New this WP — cross-solver FPC-vs-Delphi floors (r3723+r4088)

| entry | r4088 hits | class |
|---|---|---|
| `autotrans-regcontrol-tap` | 2 | AutoTrans+RegControl discrete reg-tap lands one step apart (V ~2.4e-2, rel ~3.6e-7); capi015↔r4088 shows `[rat]` differs |
| `makeposseq-fpc-delphi` | 6 | MakePosSequence reduction last-digit drift (rel ~6e-6..4e-5) |
| `reduce-fpc-delphi` | 6 | circuit-reduction collapses branches differently (structural reduced-YPrim diff); port matches 0.14.5 reduced net exactly |
| `ckt24-regcontrol-conditioning` | 11 | ckt24 SubXFMR ultra-switch conditioning floor (rel ~1.2e-7, ~1.2× the pinned tol); STATUS CF-D |
| `pvsystem-kvar-display-precision` | 2 | Delphi renders PVSystem `kvar` to ~6 sf (empirically; the Delphi/FPC `Format`/`Str` last-digit rendering §1.3-2 relaxes to numeric-token); expected == actual@6sf |
| `storage-kw-display-precision` | 2 | Delphi renders StorageController-dispatched Storage `kw` to ~6 sf (split from `storage-kwhstored-drift`; the `.kw:` term scopes it, not a broad Storage-probe mask); expected == actual@6sf |
| `vsource-nearzero-power` | 1 | Vsource unenergized-conductor power (0,0) vs ~1e-4 VA near-ideal floor |

### New this WP — r3723-only

| entry | class |
|---|---|
| `invcontrol-fixpoint-drift-synthetic` | the `invcontrol-fixpoint-drift` class on the lowercase synthetic `controls:invcontrol/` decks (sibling's `case_contains "InvControl/"` misses them); r4088 catalogs them under `eventlog-trailing-space` |

### New this WP — `skip` (the EPRI oracle cannot run the deck)

| entry | revs | reason |
|---|---|---|
| `epri-binaryshape-crash` | r3723+r4088 | binary/MMF GrowthShape crashes the EPRI DLL (#303/#58614) on both revs; port reads it correctly |
| `epri-linespacing-r4088-crash` | r4088 | EPRI r4088 raises #303 on `IEEE13_LineSpacing` (r3723 solves it) |
| `epri-linecablespacing-r4088-crash` | r4088 | EPRI r4088 raises #303 on `IEEE13_LineAndCableSpacing` (r3723 solves it) |
| `epri-capcontrolfollow-r4088` | r4088 | EPRI r4088 raises #303 on `CapControlFollow` (dss-ext `ControlSignal`); r3723 runs it |

### Pruned this WP

| entry | why |
|---|---|
| `epri-gendispatcher-propname` | **dead** — 0 hits on both r3723 and r4088 (re-verified: the three gendispatcher decks diverge on control-iteration count, 20-vs-4 / 31-vs-5, never on a property name). The entry was originally cataloged when an EPRI engine rejected a deck property with `DSSException #34` "Invalid property name"; the current swept decks — which **do** set `kvarlimit`/`genlist`/`weights` (all seven props exist in dss_capi 0.14.5 `Controls/GenDispatcher.pas`, so the port accepts them) — surface no such rejection on either rev. The residual GenDispatcher control-iteration delta (dss_capi iterates more than EPRI) is folded into `iteration-count-delta`. |

## Rung-2 handoff (`WP-U2.6`)

- The three r4133-tagged entries (`monitor-header-whitespace`,
  `property-format-brackets`, `eventlog-trailing-space`) will be re-swept at
  r4133; the r4133 event-log wording overhaul may retire/reshape
  `eventlog-trailing-space`.
- The r4088→r4133 **protection overhaul** (Relay/Recloser/Fuse/SwtControl) and the
  two non-protection r4133-side moves (**IEEE_519** harmonics V 1.3e-2,
  **InductionMachine** converged-flip) are Rung-2 scope, inventoried in
  `delta_r4088_r4133.md` / `sweeps/r4088_vs_r4133.md` — **not** cataloged here
  (they do not appear against r4088).
