# WPG.21 — `MakePosSeq` oracle hazard probes

Oracle: pinned dss-python **0.15.7** / backend dss_capi **0.14.5** (the exact
Pascal vendored at `.inputs/dss_capi`). Every config ran in a **separate
subprocess** (a NIL-deref takes the whole process down). Probe scripts:
`probe_makeposseq.py` / `probe_makeposseq2.py` (scratchpad; not committed — the
findings below are the artifact).

Purpose (GAPS ritual, WT-A1 brief §D): classify each `MakePosSeq` config as
**CRASH** (→ becomes a Rust safe-skip unit test in a later WT, NEVER an oracle
deck), **CREATE/SOLVE ERROR** (the deck aborts before `makeposseq`), or **OK**
(safe to put in an oracle deck).

## Root cause of the fleet-control crash (source-confirmed)

`GenDispatcher` / `ESPVLControl` / `UPFCControl` share a byte-identical
`MakePosSequence` (`Controls/GenDispatcher.pas:263`, `ESPVLControl.pas:357`,
`UPFCControl.pas:179`):

```pascal
if MonitoredElement <> NIL then
begin
    FNphases := ControlledElement.NPhases;   // <-- ControlledElement, NOT MonitoredElement
    Nconds := FNphases;
    Setbus(1, MonitoredElement.GetBus(ElementTerminal));
end;
inherited;
```

The NIL guard tests `MonitoredElement` but the body immediately dereferences
`ControlledElement.NPhases`. These are **fleet** controls: they act on a *list*
of generators/DERs/UPFCs, so `ControlledElement` is always NIL. Therefore:

- `element=` **set** (⇒ `MonitoredElement <> NIL`) → `ControlledElement.NPhases`
  derefs NIL → **Access violation (#303)**. Confirmed even with a fully resolved
  `element=line.l1` + a real generator (probe `3a`, `S4`).
- `element=` **not set** (⇒ `MonitoredElement = NIL`) → the block is skipped, no
  crash — but the element then fails at **solve** with `#372 "Monitored Element
  … is not set"` (probes `S3`, `S5`), so it still cannot sit in a solving deck.

`InvControl` / `ExpControl` (`InvControl.pas:943`, `ExpControl.pas:422`) crash by
a different path — the empty-DER guard calls `RecalcElementData` then
unconditionally `Setbus(1, MonitoredElement.GetBus(ElementTerminal))`; with an
empty list `MonitoredElement` stays NIL → **Access violation**. In dss-python
0.15.7 the *populated* case also faults (at object **creation** for InvControl,
at `makeposseq` for ExpControl — probes `S1`, `S2`, focused InvControl probe), so
neither is deck-usable in this oracle.

Per CLAUDE.md ("UB or state-mutating-read bugs are NOT reproduced"), these
Access-violations are UB → the Rust port must **safe-skip** (NIL-check + no
crash), gated by unit tests, not reproduced. None become oracle decks.

## Probe results

| # | Config | Oracle behavior | Class | Deck? |
|---|--------|-----------------|-------|-------|
| 1 | InvControl, empty DERList | `makeposseq` → Access violation #303 | CRASH | no (safe-skip) |
| 2 | ExpControl, empty PVSystemList | `makeposseq` → Access violation #303 | CRASH | no (safe-skip) |
| 3a | GenDispatcher, `element=line.l1` resolved + gen | `makeposseq` → Access violation #303 (ControlledElement NIL) | CRASH | no (safe-skip) |
| 3b/S4 | ESPVLControl, `element=` set | `makeposseq` → Access violation #303 | CRASH | no (safe-skip) |
| 3c | UPFCControl, no `element=`, no UPFC | `makeposseq` OK, converged | OK (NIL-guard skips) | n/a |
| 4a | CapControl, `element=line.doesnotexist` | **creation** aborts `#303 "Element is not set, aborting"` | CREATE ERROR | no |
| 4b | RegControl, `transformer=nonexistent` | **creation** aborts `#124 "Transformer Element is not set"` | CREATE ERROR | no |
| 5 | matrix (`rmatrix`/`xmatrix`/`cmatrix`) Line, `makeposseq` BEFORE solve | `makeposseq` OK, converged (Z/Yc built at edit-time RecalcElementData, not solve) | **OK** | **yes** — safe to `makeposseq` a matrix Line pre-solve |
| 6a | CapControl, resolved, terminal defaulted | `makeposseq` OK, converged | OK | yes |
| 6b | GenDispatcher, `genlist=[gmissing]` | **solve** aborts `#482 "Solution aborted"` | SOLVE ERROR | no |
| S1 | InvControl WITH pvsystem in DERList (+vvc_curve) | **creation** → Access violation #303 | CRASH | no (safe-skip) |
| S2 | ExpControl WITH pvsystem | `makeposseq` → Access violation #303 | CRASH | no (safe-skip) |
| S3 | GenDispatcher, no `element=`, `genlist=[g1]` | solve → `#372 Monitored Element … not set` | SOLVE ERROR | no |
| S5 | ESPVLControl, no `element=` | solve → `#372 Monitored Element … not set` | SOLVE ERROR | no |
| S6 | StorageController, `element=line.l1` + `elementlist=[st1]` | `makeposseq` OK, converged | **OK** | **yes** |

## Consequences for the 6 decks (brief §E)

- **Finding #5 (matrix Line pre-solve is safe)** confirms the plan's Line matrix
  branch can be exercised; but the plan's decks all `solve` first anyway, so this
  only proves the branch is not itself a NIL hazard.
- **`makeposseq_ctrl.dss` deviates from the plan's control list.** The plan named
  `InvControl+PVSystem`, `ExpControl+PVSystem`, `GenDispatcher` — all three are
  **CRASH hazards** above and are therefore **excluded** from the deck (they
  become WT-D safe-skip unit tests, per this log). The deck keeps the safe
  controls with resolved targets: EnergyMeter, Monitor (mode 0 and 1), Sensor,
  CapControl+cap, RegControl+xfmr, Relay/Recloser/SwtControl on lines, and
  **StorageController** (probe `S6`, OK). This is the only content deviation
  forced by oracle probing.
- CapControl/RegControl with a **bad** target abort at *creation* (4a/4b), so a
  "bad-name" config is not a `makeposseq` probe at all — it never reaches the
  command. Not decked; the resolved forms are.
