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
| `regcontrol-autotrans-typecast` (was `autotrans-regcontrol-tap`) | 2 | ~~AutoTrans+RegControl discrete reg-tap lands one step apart (V ~2.4e-2, rel ~3.6e-7); capi015↔r4088 shows `[rat]` differs~~ — **corrected 2026-09-03 (RP3.12): not a floor.** EPRI's `RegControl` never taps an `AutoTrans` (unchecked `TTransfObj` cast, `RegControl.pas:926/1026/1296/1370/1479` over `AutoTrans.pas:88`; zeroed increment at `:1249-1250`) — 0 event-log lines vs 10–13, the bus left outside its band. `UPSTREAM_BUG`, never reproduced; see the correction block in `DIVERGENCES.md` |
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
  - **Recloser PORTED (WP-U2.2):** the `recloser_temp/perm` (+ midi twins)
    ctrlqueue `2-vs-0` and inst-delay rows in `sweeps/r4088_vs_r4133.md` are dead
    against r4133 — those family decks now gate on `oracle: "r4133"` (the
    breaking removed-default curves make a curveless recloser inert, so the
    ctrlqueue is empty on both engines) and match the r4133 oracle. The full
    per-rev sweep partition re-run that formally prunes these stays with the
    rung-exit WP-U2.6 (per the pruning discipline above).

## The Rung-2-exit sweep (WP-U2.6, 2026-07-17)

Swept the port against official-EPRI **r4133** with `DSS_LIVE_OPENDSS_ASSERT=1`
(the rung-exit gate) and re-ran **r4088** (direction sanity-check). Both green;
the mandatory gate stays the regression spine (every swept case is also compared
vs the pinned 0.14.5 oracle, green — so the EPRI gap is purely FPC↔Delphi, never
a Rung-2 regression). `103` target-rev `oracle`-flipped cases are excluded (gated
in the mandatory gate against their own target).

| sweep | matched | known-diverged | known-skipped | NEW: before → after cataloging |
|---|---|---|---|---|
| `r4133` (11.0.0.1, the rung-2 EPRI target) | 326 | 70 | 4 | **58 → 0** (ASSERT green) |
| `r4088` (10.2.0.1, direction check) | 329 | 67 | 4 | **7 → 0** (%stored + monitor_seqmag) |

Of the 70 r4133 known-diverged, 16 are the format classes
(`property-format-brackets` ×10 + `eventlog-trailing-space` ×6) and 54 are the
FPC↔Delphi last-ulp/display floors (44 via entries extended to r4133 + 10 via the
3 new entries); known-skipped = the four EPRI-DLL #303 crash decks.

### Entry ledger change at Rung-2 exit (22 → 25 entries)

**Extended to r4133** (the divergence persists on a path behaviorally identical
r4088=r4133 — `PCElements/` / `Meters/` / injection / reduction / ckt24 feeder all
byte-identical; the solver `Common/Solution.pas` differs only in inert progress-form
plumbing + a commented-out debug `WriteLn`, `PDElements/AutoTrans.pas` only in two
read-only PropertyHelp strings — numerically inert across the EPRI delta; each also
stays green in the mandatory gate vs 0.14.5):

| entry | new revs | r4133 hits |
|---|---|---|
| `iteration-count-delta` | +r4133 | 4 (autotrans_both/reg, gendispatcher{,_kvarlimit}) |
| `injection-fpc-delphi-ulp` | +r4133 | 4 (combo/indmach asymmetric) |
| `regcontrol-autotrans-typecast` (was `autotrans-regcontrol-tap`) | +r4133 | 2 (midi_autotrans{,_both}) — **class corrected 2026-09-03 (RP3.12) from "FPC-vs-Delphi floor" to `UPSTREAM_BUG`, never reproduced; see the correction block in `DIVERGENCES.md`** |
| `pvsystem-kvar-display-precision` | +r4133 | 2 (expcontrol_basic, invcontrol_expmodel) |
| `storage-kwhstored-drift` | +r4133 | 6 (storagectrl kwhstored probes) |
| `storage-kw-display-precision` | +r4133 | 2 (storagectrl_chargelow/support) |
| `makeposseq-fpc-delphi` | +r4133 | 6 (modes:makeposseq) |
| `reduce-fpc-delphi` | +r4133 | 6 (modes:reduce YPrim + mergeparallel) |
| `ckt24-regcontrol-conditioning` | +r4133 | 11 (ckt24 + Torn_Circuit + MemoryMapping) |
| `vsource-nearzero-power` | +r4133 | 1 (Paulo_Example/subestacao) |
| `epri-binaryshape-crash` (skip) | +r4133 | 1 (#303, same as r4088) |
| `epri-linespacing-r4088-crash` (skip) | +r4133 | 1 (#303 at calcv) |
| `epri-linecablespacing-r4088-crash` (skip) | +r4133 | 1 (#303) |
| `epri-capcontrolfollow-r4088` (skip) | +r4133 | 1 (#303) |

**New this WP** (classes first witnessed by the Rung-2 sweeps; r4088+r4133 unless
noted, all display/last-ulp floors byte-identical across the delta):

| entry | revs | class |
|---|---|---|
| `storage-pctstored-display-precision` | r4088+r4133 | Storage `%stored` rendered to 6 sf by Delphi (`75.089575→75.0896`); §1.3-2 display-precision, sibling to `.kw`/`kvar` (6 decks: invcontrol_storage_vv_vw/vw + modes:time/*) |
| `monitor-seq-magnitude-drift` | r4088+r4133 | seq-magnitude monitor channel (V2) Fortescue-transform FPC↔Delphi last-ulp (rel ~1.1e-5); `Meters/Monitor.pas` byte-identical r4088=r4133 |
| `harmonics-ieee519-r4133` | **r4133** | the r4088→r4133 harmonics voltage move on IEEE_519 (V ~4.3e-4, Y bit-identical). Source-confirmed nothing to port: SolutionAlgs/Load/Spectrum/YMatrix byte-identical r4088=r4133; determinism-proven per engine ⇒ build-drift amplified by the 519-filter near-resonance (3 IEEE_519 decks) |

**Narrowed this WP** (empirically 0 hits on r4088 **and** r4133 — dropped to
`r3723`, their original triage provenance; a future r3723 sweep confirms or
prunes the tail):

| entry | new revs | why dead on r4088/r4133 |
|---|---|---|
| `monitor-header-whitespace` | r3723 | the harness `compare_monitor` now normalizes the Delphi leading-space CSV header directly (WP-U2.1 combo-restore audit fix), so it never surfaces on the EPRI channel |
| `meter-zonepce-count` | r3723 | the six witness decks (energymeter {sym,asym,options,midi} + autoadd{,_cap}) now MATCH both EPRI revs (check_meters_monitors runs, oracle=None, ZonePCE agrees) |

### The two WP-U0.2 non-protection surprises — closed

- **IEEE_519 harmonics** (V 1.3e-2): `harmonics-ieee519-r4133`, source-confirmed
  nothing to port (above).
- **InductionMachine converged-flip**: already resolved by **WP-U2.1** —
  `InductionMachine/{Master.DSS,Run.dss}` moved `solvable_now →
  skipped_needs_investigation` (tag `r4133_breaking_nonconvergence`); the port
  reproduces the r4133 non-convergence, so the deck is out of the swept universe.

**Rung-2 complete: engine behavior = OpenDSS 11.0.0.1 (r4133) except this
documented ledger.**
