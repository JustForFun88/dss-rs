# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-23 — **Phase 7 IN PROGRESS** (branch
`phase-7-extended-elements`): **WP7.1 COMPLETE; WP7.2 (Protection) IN PROGRESS —
step 1 done** (the `Fault` element)**, step 2a done** (the `SwtControl` switch
control)**, step 2b done** (the `Fuse` per-phase TCC protection)**, step 2c done**
(the `Recloser` overcurrent recloser)**, step 2d done** (the `Relay` — all nine
sub-types, Generic/TD21 logic deferred to WP7.7); **next = WP7.2 step 3
(reliability activation)**. WP7.1 landed the Carson line-constants engine
(`support/line_constants/`), the `WireData`/`CNData`/`TSData`/`LineSpacing`/
`LineGeometry` catalog, and Line's `geometry`/`spacing`/`wires`/`cncables`/
`tscables` fetch path (all oracle-pinned); migrated the geometry/cable corpus
feeders into `solvable_now` (**17→35**); and (step 5) added the §1 tier-1
**targeted golden** `phase7/line_geometry*.json` (`gen_phase7.py` +
`golden_phase7.rs`, **5 scenarios** pinning the Carson geometry/spacing/cable Line
**YPrim** offline — the focused regression guard the live gate doesn't replace).
**WP7.2 step 1** landed the `Fault` element (`pd/fault.rs`): an uncoupled
conductance branch (`G=1/r` / `Gmatrix`), registered + on a new `Circuit.faults`
list, with the `Check_Fault_Status`/`DoResetFaults` control-loop wiring now live
(temporary-fault apply/clear); `props.json` `fault.json` + 8 oracle-pinned tests.
**WP7.2 step 2a** landed `SwtControl` (`control/swt_control/`): a manual switch
control on the WP5.7 control sweep (`Sample`/`DoPendingAction` open/close a
controlled element's terminal + event log), with the generic
`CktElementData::set_terminal_closed` conductor-open machinery and a
`RefAction::SetSwitchClosed` for the `State=` parse-time force; `props.json`
`swtcontrol.json` (7 scenarios) + 14 oracle-pinned tests.
**WP7.2 step 2b** landed `Fuse` (`pd/fuse/`): the first **TCC/sensing**
protection device — a per-phase fuse that evaluates `TCC_Curve.GetTCCTime`
(newly ported) on the monitored current and blows individual controlled
conductors via per-phase control-queue actions. New shared machinery:
`TccCurveObj::get_tcc_time` (log-log interpolation), per-conductor
`CktElementData::set_conductor_closed`, `RefAction::SetConductorsClosed`, and a
`PropType::MappedStringEnumArray` for the per-phase `Normal`/`State` arrays;
`props.json` `fuse.json` (8 scenarios) + 20 oracle-pinned tests.
**WP7.2 step 2c** landed `Recloser` (`control/recloser/`): an overcurrent
recloser that trips the controlled element's whole terminal on a phase/ground TCC
pickup and recloses after an interval, up to `Shots` operations before lockout
(fast then delayed curves). New engine machinery: `PropFlags::ARRAY_MAX_SIZE`
(`RecloseIntervals`) and the integer-dump `VALUE_OFFSET` (`Shots` aliases
`NumReclose−1`); `props.json` `recloser.json` (7 scenarios) + 26 oracle-pinned
tests.
**WP7.2 step 2d** landed `Relay` (`control/relay/`): the general protection
control — nine `Type=` sub-types over a shared 50-property surface and the same
whole-terminal `Closed[0]` trip/reclose state machine. Seven sub-types ported
live (`Current`/`Voltage`/`ReversePower`/`46`/`47`/`Distance`/`DOC`); the
dynamics-coupled `Generic` (needs PC state `Variable[]`) and `TD21` (needs
`DynaVars.h`/ring buffer) parse + dump but **defer their `Sample` logic to
WP7.7** (a `NOT_PORTED` error if reached). New shared machinery:
`TccCurveObj::get_ov_time`/`get_uv_time` (definite-time over/under-voltage scans)
and `PropFlags::ALLOW_NONE` (a zero-count `DoubleVArray` dumps `[NONE]`).
Unlike the Recloser, every Relay event-log line is gated on `ShowEventLog`, and
`MakeLike` copies `DelayTime`/`BreakerTime`; `props.json` `relay.json` (9
scenarios) + 27 oracle-pinned tests. Full per-step detail in **§1e**.

**Standing toolchain note:** the gate runs on **`stable`** (`cargo +stable …`),
matching CI (`dtolnay/rust-toolchain@stable`) — no nightly dependency. `dss-core`
carries `#![allow(clippy::collapsible_match)]` (`d85d026`): clippy 0.1.96 (now on
stable) mis-fires that lint on the byte-faithful `match prop { CONST => if cond
{..} }` port idiom, and its autofix even drops `else` branches. (The earlier
WP7.1-session EPRI/ADiakoptics power-floor fix — `c7c6649`/`6d3b9ac`,
`solvable_now` 32→35 — is in the archived WP7.1 record,
[`docs/phase-records/phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md).)

Phase 7 = DER, protection, line constants, harmonics, dynamics (PORTING_PLAN.md
§Phase 7, the largest phase ~18%). Earlier phases merged to `main` (newest first):
**Phase 6** (WP6.1–WP6.10 — meters/monitors/topology/Generator + the 8500-node gate
+ the live corpus gate; `--no-ff` `b98223a`, `main` not pushed to origin)
→ [record](docs/phase-records/phase-6.md); **Phase 5** (`10d3550`), **Phase 4**
(`5f27a25`). Their full logs and the per-WP detail live under
`docs/phase-records/` (§1b–1d indexes them) and the §1 table below.

> **Working cadence:** finish one small step → run the full gate → update this
> file → **stop and wait for explicit user confirmation** before the next step.
> The full per-step ritual (gate, STATUS sync, the two audits) is
> **`PHASE7_PLAN.md §0`**, run per **§1e**. (Earlier phases sometimes executed
> several WPs in one pass on explicit user instruction.)

---

## 1. Where we are

| Phase | Scope | Status |
|------|-------|--------|
| 0 | Tooling, oracle, faer spike, CI, Phase-0 goldens | ✅ done (committed) |
| 1 | Shared math (`support/`) + full `TDSSParser` port | ✅ done (commit `729eb77`) |
| 2 | Object model, property engine, executive skeleton | ✅ done (commit `22f861d`) |
| 3 | ★ Vertical slice: parse → circuit → Y matrix → solve → voltages | ✅ done (commit `2ac8691`) |
| **4** | **Transformer/Capacitor/Reactor/LineCode + controls (parse-only) + macro + feeder gate** | ✅ done (merged to main, `5f27a25`); `PHASE4_PLAN.md` |
| **5** | **LoadShape/XYcurve/controls behavior, control queue, time modes + feeder gate (controls active)** | ✅ done (merged to main, `10d3550`); `PHASE5_PLAN.md` |
| **6** | **Meters/Monitors/topology/Generator + 8500-node gate + live corpus gate** | ✅ done (merged to main, `b98223a`); `PHASE6_PLAN.md` |
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | 🚧 in progress — `PHASE7_PLAN.md` (WP7.1–WP7.10); branch `phase-7-extended-elements`; **WP7.1 done**, **WP7.2 (Protection) in progress — steps 1 + 2a + 2b + 2c + 2d done** (Fault, SwtControl, Fuse, Recloser, Relay); **next = WP7.2 step 3 (reliability activation)**. Per-step detail in §1e |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 492, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_phase6 1, golden_phase7 1,
                            # golden_checkpoints 1, golden_ieee8500 1,
                            # golden_reliability 1, golden_allocation 1,
                            # golden_gendispatcher 1, golden_autoadd_reduce 1,
                            # golden_slice 2, golden_smoke 3, props_roundtrip 1,
                            # corpus_manifest 1, corpus_live 3
                            #   (corpus_live_solvable_cases_match_oracle +
                            #    solvable_now_has_multistep_depth run
                            #    UNCONDITIONALLY — the pinned oracle MUST be
                            #    installed (it fails, not skips, without it);
                            #    only corpus_live_classify is opt-in, via
                            #    DSS_LIVE_CLASSIFY=1 — the growth/classify probe),
                            # dss-parser 62+1, dss-sparse 5
```

### Phase 5 gate — green  *(detail → `docs/phase-records/phase-5.md`)*
- `golden_feeders_controls.rs`: the unmodified IEEE13/IEEE37/IEEE123 masters
  (controls active) + `ieee34mod1` match the Phase-0 goldens — converged + total
  iterations exact, `YNodeOrder` exact, RegControl `tap_number` / capacitor
  `states` exact, final taps 1e-12 rel (the integer `tap_number` is the exact
  discrete check), V/I/P 1e-6, and every element's full property dump.
- `golden_phase5.rs` vs `tests/golden/phase5/*.json` (`gen_phase5.py`):
  `daily_ieee13`, `duty_2bus`, `eventlog_ieee13`, `capcontrol_micro` — per-step
  `dblHour` + iteration counts exact, **event logs line-for-line** (normalized),
  per-step V 1e-6 (the shape-scaled `Yeq` restamp per Y build, `a6903f1`).

### Checkpointed-model gate (`crates/dss-core/tests/golden_checkpoints.rs`) — green
- `gen_checkpoints.py` → `tests/golden/checkpoints/<scenario>.json` (schema 2,
  one file per scenario; the gate runs every file in the directory, so adding a
  scenario is just adding a file). Unlike the
  other command-replay gates (which compare only converged outputs), this one
  captures the **assembled electrical model after every committed time step** —
  the unfactored system Y, selected element YPrim blocks, the injection vector,
  node voltages, and discrete control state — and compares each to the oracle.
  A stale Y/YPrim fails at the step and matrix entry it first goes wrong, not as
  downstream register drift. Scenarios: `micro_yeq_steps` (control-free daily,
  full-CSC per-step pin), `ieee13_daily` (24-step daily with regulator tap
  changes — full CSC + fingerprint; the direct regression guard for the
  "frozen load Yeq" bug: reverting commit `a6903f1` makes it fail at step 6,
  `Y[634.1]`), `ieee123_snap` (large-feeder fingerprint-only + selected YPrim
  path). Tolerances: `tests/TOLERANCE_NOTES.md`. The assembled Y is compared
  **unfactored** so the `dss-sparse` row equilibration is out of scope.

### Live corpus oracle gate (`crates/dss-core/tests/corpus_live.rs`) — opt-in
See `CORPUS_TEST_PLAN.md`. The whole `electricdss-tst` corpus is **vendored** into
`tests/corpus/electricdss-tst/` (1544 files, 122 MiB; `tools/corpus/vendor.py`,
`.git` excluded, with `SHA256SUMS` + `README.md` provenance) so tests no longer
depend on the temporary `.inputs/electricdss-tst`.
- **Manifest accounting (always-on).** Every `.dss` (915) is in exactly one
  manifest under `tests/corpus/manifests/` (`solvable_now`, `skipped_unsupported`,
  `skipped_oracle_issue`, `skipped_needs_investigation`, `missing_dependency`,
  `not_an_entry_point`). `corpus_manifest.rs` enforces the bijection — no silent
  omissions — and runs in the normal `cargo test`: adding/removing a `.dss` fails
  it until the file is classified.
- **Live comparison (`DSS_LIVE_ORACLE=1`; runs in the `live-oracle` CI job).**
  For each of the **17** `solvable_now` cases the gate compiles+solves on the Rust
  engine and on the pinned dss-python oracle (`tools/oracle/oracle_server.py`, a
  one-shot subprocess over JSON), and compares the full assembled model per step —
  node order, **full** system Y (entry-by-entry, no fingerprint substitution),
  node voltages, **every** element's currents/powers, selected YPrim blocks (a
  guard fails the case if the oracle returns no YPrim for a named selected
  element), the injection vector, and discrete state — reusing the `harness/mod.rs`
  comparators and the checkpoint gate's tolerance policy.
  - **Three control-diverse 24-step daily runs** — `IEEE13Nodeckt` (wye gang
    reg), `ieee37` (delta, open-delta LDC reg bank) and `IEEE123Master` (multiple
    cascaded reg banks) — each with a meter + three monitors (modes 0/1/2) +
    selected elements, so the **multi-step per-step**, **YPrim**,
    **monitor-channel** and **EnergyMeter-register/zone** paths are all exercised
    live (`compare_monitor`/`compare_meter`, the *same* comparators
    `golden_phase6.rs` now routes through, gated per case by
    `check_meters_monitors`). Incidental master-defined monitors are *not*
    compared — the pinned oracle returns a phantom `Channel(i)` for an unsampled
    monitor (see `tests/TOLERANCE_NOTES.md`).
  - The **IEEE 8500-Node master is promoted** (snapshot; `post: Set
    Maxiterations=20` to converge — the bare probe didn't, which is why the
    classifier had parked it), so the full 8531-node Y, every element's I/P, and a
    YPrim block are compared live at scale (complementing the always-on
    `golden_ieee8500.rs` golden, whose `compare_discrete` also pins the full
    1190-transformer tap set here).
  - **Depth is guarded always-on.** `solvable_now_has_multistep_depth` (no oracle)
    asserts `solvable_now` keeps ≥1 multi-step `check_meters_monitors` case and ≥1
    case with selected elements, so the deep coverage can't silently revert to
    snapshots. The solvable + classify tests **auto-skip (pass)** without the env
    var / oracle, so `cargo test --workspace` stays green everywhere; the
    **`live-oracle` GitHub Actions job** installs the pinned oracle (PIN.txt) and
    runs the **whole `corpus_live` binary** (not a name filter that could green on
    zero matched tests). The oracle server hard-asserts **both** dss-python 0.15.7
    **and** engine 0.14.5 (PIN.txt). No goldens are written; the oracle is
    consulted live.
- **Growth.** `DSS_LIVE_CLASSIFY=1 corpus_live_classify` probes the
  `skipped_needs_investigation` candidates with the full comparison and writes
  `tmp/classify_report.json`; `tools/corpus/apply_classify.py` promotes the
  passing cases into `solvable_now` (and routes oracle/engine failures to the
  right skip bucket). `tools/corpus/coverage_report.py` →
  `tests/corpus/COVERAGE.md` tracks the burn-down toward 100% of entry points.

### Phase 4 gate (`golden_feeders.rs`) — green  *(detail → `docs/phase-records/phase-4.md`)*
The controls-off IEEE13/37/123 variants (`gen_phase4.py`) match `phase4.json`
(pinned oracle): converged + iterations exact (3/3/3), `YNodeOrder` exact
(41/117/278), V 1e-6, every element's I/P 1e-6 (creation order), total
power/losses 1e-6. The Phase-3 `golden_slice.rs` (13 scenarios) stays green; the
CLI runs the real masters (`cargo run -p dss-cli -- script.dss`).

---

## 1b–1d. Completed-phase records (archived)

The full work-package logs for the completed, merged phases (and completed
Phase-7 work packages) live under `docs/phase-records/` to keep this handoff
lean. They are frozen history, superseded only by the code and tests:

- **Phase 3** — the vertical-slice file-by-file map (circuit model / element base /
  solution / executive / property engine) — still the architectural reference §2
  points to. → [`docs/phase-records/phase-3.md`](docs/phase-records/phase-3.md)
- **Phase 4** — PD elements (Transformer/Capacitor/Reactor), catalog objects
  (LineCode/XfmrCode/GrowthShape), the Line→LineCode fetch path, parse-only
  RegControl/CapControl, the `define_properties!` macro, and the controls-off
  feeder gate. → [`docs/phase-records/phase-4.md`](docs/phase-records/phase-4.md)
- **Phase 5** — controls + time series: XYcurve / LoadShape / TShape /
  PriceShape, ControlQueue + event log, RegControl/CapControl behavior, the
  control loop (`Sample_DoControlActions`), and the time-series solve modes.
  → [`docs/phase-records/phase-5.md`](docs/phase-records/phase-5.md)
- **Phase 6** — meters + topology: CktTree, Generator, Monitor, EnergyMeter +
  zone build, registers/TakeSample, reliability (`RelCalc`), Sensor + load
  allocation, the GenDispatcher/StorageController/AutoAdd/ReduceAlgs skeletons,
  and the 8500-node gate. Merged to `main` `b98223a`.
  → [`docs/phase-records/phase-6.md`](docs/phase-records/phase-6.md)
- **Phase 7 WP7.1** (Line constants & geometry) — the Carson engine, the
  WireData/CNData/TSData/LineSpacing/LineGeometry catalog, Line's geometry/spacing
  Carson path, the corpus migration, and the offline geometry golden. **Complete +
  gate-green on the `phase-7-extended-elements` branch (not yet merged);** the live
  §1e keeps a step summary + the tracked-open plural-cable note.
  → [`docs/phase-records/phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md)

---

## 1e. Phase 7 record (branch `phase-7-extended-elements`) — IN PROGRESS

Execution plan: **`PHASE7_PLAN.md`** (WP7.1–WP7.10). Per-WP cadence — the full
ritual in `PHASE7_PLAN.md §0`, run autonomously per step: gate green → update this
file + commit → `/audit-code <step scope>` → fix + commit → `/audit-tests <step
scope>` → fix + commit → **full `STATUS.md` review + sync + archive-cleanup** +
commit → **then** stop for confirmation.

**WP7.1 (Line constants & geometry) — ✅ COMPLETE (steps 1–5), gate-green.** The
detailed per-step records are archived at
[`docs/phase-records/phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md); the
header frontier paragraph summarizes the deliverable. In brief:
- **step 1** — the Carson line-constants engine `support/line_constants/`
  (`LineConstants` + OH/CN/TS/cable specializations, `Calc(f)` → Z/Yc, Kron),
  **frequency-parameterized — the WP7.6 harmonics hook** (`z_matrix(f)`/`yc_matrix(f)`).
- **step 2** — the catalog classes `WireData`/`CNData`/`TSData` (`conductor_data/`),
  `LineSpacing`, `LineGeometry` (object + per-conductor edit state machine +
  `UpdateLineGeometryData`/`CalcMatrices`); new prop kind `PropType::ObjectRefArray`.
- **step 3** — Line's `geometry=`/`spacing=`/`wires=`/`cncables=`/`tscables=` Carson
  path (the `total_z_path` branch in `CalcYPrim`; total Z/Yc with length+units folded in).
- **step 4** — geometry/cable corpus feeder migration (`solvable_now` 17→35), incl.
  the new `Set EarthModel=Carson|FullCarson|Deri` option and the EPRI/ADiakoptics
  power-floor fix (`c7c6649`/`6d3b9ac`, the 3 meshed cases, `solvable_now` 32→35).
- **step 5** — the targeted offline golden `phase7/line_geometry*.json`
  (`gen_phase7.py` + `golden_phase7.rs`, 5 scenarios pinning the Carson Line YPrim
  entry-by-entry — the focused regression guard the live gate doesn't replace).
- **Tracked-open (un-pinned, needs investigation):** the **plural-cable**
  `cncables=`/`tscables=` active-conductor reset diverges between the vendored
  `LineGeometry.pas` (`istop`) and the pinned 0.14.5 binary (`Cond=1`); the scalar
  CN/TS *data* paths are covered, so the plural forms stay un-pinned pending that
  source/binary reconciliation (full note in the archive).

**WP7.2 (Protection) — 🚧 IN PROGRESS.** Steps **1 (`Fault`), 2a (`SwtControl`),
2b (`Fuse`), 2c (`Recloser`), 2d (`Relay`) done + gate-green**; **next = step 3
(reliability activation)** + step 4 (gate). Full per-step records (decisions,
audits, gate detail) archived at
[`docs/phase-records/phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md). In brief:
- **step 1 — `Fault` (`pd/fault.rs`):** an uncoupled multi-phase **conductance**
  branch (`G=1/r` / `Gmatrix`) + the FaultStudy input (WP7.9). New `ElemKind::Fault`
  + `Circuit.faults` (stamps Y, excluded from `pd_elements`); control-loop wiring
  `solution/faults.rs` `check_fault_status`/`reset_faults` (the `Set mode=` / `Reset
  Faults` path; raises `system_y_changed` on toggle). MonteFault → WP7.9.
  `fault.json` (6) + 8 inline.
- **step 2a — `SwtControl` (`control/swt_control/`):** a manual switch — the
  **whole-terminal** force. New generic `CktElementData::set_terminal_closed` +
  `RefAction::SetSwitchClosed` (the `State=` parse-time force). `swtcontrol.json`
  (7) + 14 inline. **Origin of the dirty-edge rule (`d0addb4`)** — see below.
- **step 2b — `Fuse` (`pd/fuse/`):** per-phase TCC. **`TccCurveObj::get_tcc_time`
  ported** (log-log interp). New per-conductor `set_conductor_closed`/
  `conductor_closed` + `RefAction::SetConductorsClosed` + `PropType::MappedStringEnumArray`
  (per-phase `Normal`/`State`). Default `FuseCurve=tlink` resolved via the
  `command.rs` foreign view. `fuse.json` (8) + 20 inline.
- **step 2c — `Recloser` (`control/recloser/`):** whole-terminal trip + reclose up
  to `Shots`, fast→delayed curves. New engine machinery `PropFlags::ARRAY_MAX_SIZE`
  (`RecloseIntervals`) + integer-dump `VALUE_OFFSET` (`Shots = NumReclose−1`);
  recloser enums; default `a`/`d` curves. `recloser.json` (7) + 26 inline.
- **step 2d — `Relay` (`control/relay/`):** the general protection control
  (`Relay.pas`, 2354 lines — the largest WP7.2 unit). Nine `Type=` sub-types over
  one 50-property surface + the shared `Closed[0]` trip/reclose/reset machine.
  Module split `mod.rs` (props/struct/recalc/Sample dispatch/DoPendingAction/Reset)
  + `logic.rs` (the per-sub-type sensing) + `accessors.rs` + `tests.rs`. **Ported
  live:** `Current` (overcurrent 50/51), `Voltage` (27/59 + voltage reclose),
  `ReversePower` (32), `46`/`47` (neg-seq), `Distance` (21), `DOC` (directional
  overcurrent — the dominant corpus type, with the `DOC_P1Blocking` forward-power
  block + the circle/high-line/inner-zone decision tree ported verbatim).
  **Deferred to WP7.7 (`NOT_PORTED` if `Sample` reached):** `Generic` (needs PC
  state `Variable[]`) and `TD21` (needs `DynaVars.h`/`Frequency`/`IterationFlag`
  + the per-cycle ring buffer) — both fully parse + dump. New shared machinery:
  `TccCurveObj::get_ov_time`/`get_uv_time` (definite-time scans) and
  `PropFlags::ALLOW_NONE` (zero-count `DoubleVArray` ⇒ `[NONE]`, the `Type=DOC`
  ⇒ NumReclose 0 case). Relay-specific quirks vs the Recloser: event-log lines are
  **gated on `ShowEventLog`** (`if ShowEventLog then AppendToEventLog`), the queue
  `CTRL_RESET` runs the full `Reset()` (logs "Resetting" + re-forces the element),
  and `MakeLike` **copies** `DelayTime`/`BreakerTime`. `relay.json` (9) + 27 inline.
  *audit-code follow-up:* verdict faithful, no Critical/Major; tidied the
  Generic/TD21 `NOT_PORTED` log to fire **once** per object (a `not_ported_logged`
  latch, not once per control iteration), and pinned the dead `Type=Voltage`
  `RecloseIntervals[3]=5.0` upstream quirk with a "don't simplify" comment.

**Carry into step 3 (reliability activation) + step 4 (gate):**
- **Dirty-edge discipline (implemented across all four controls).** Every trip/
  close/reset forces conductors via `Closed[]` → `TDSSCktElement.Set_ConductorClosed`
  (`CktElement.pas:287`) sets `YPrimInvalid := TRUE` → `SystemYChanged := TRUE`
  (`:240`) **unconditionally**, no change-comparison. So each control raises
  `system_y_changed` **unconditionally** (or via an exact per-conductor check),
  **never** gated on a `terminal_all_phases_closed`/`is_closed` aggregate (a
  partial-open terminal otherwise slips a real change past the rebuild → stale Y),
  and **each ships a partial-open fail-on-regression test** (`d0addb4`/`d1f48231`).
  Relay forces via `Closed[0]` (`Relay.pas:962…`) — same rule.
- **Reliability deferred to step 3.** SwtControl/Fuse/Recloser/Relay leave
  `Flg.HasOCPDevice`/`HasAutoOCPDevice` + the recalc-time `Closed` resync for step 3,
  which implements `GetOCPDeviceType` and activates the dormant Phase-6 `RelCalc`
  SAIFI/SAIDI (the `meter_zone_micro` + OCP gate). Relay's `RecalcElementData`
  already does the `Include(Flg.HasOCPDevice)` in Pascal — wire it in step 3.
- **Generic/TD21 Sample logic deferred to WP7.7** (dynamics): the relay parses +
  dumps `Type=Generic`/`TD21` but the live sensing records a `NOT_PORTED` error.
- **Corpus migration** of the `{Fault,Fuse,Recloser,Relay,SwtControl}` cases lands
  at the **step-4 gate**, with the targeted `phase7/protection*.json` golden +
  normalized event-log-equality for a trip/reclose sequence. The corpus relays are
  `Type=Current` (definite-time `Delay`), `Type=DOC` (the 68 LV network-protector
  cases), and `Type=Voltage`.
- dss-core lib **392 → 492** across steps 1–2d.

---

## 2. What Phase 3 built (file-by-file map) — archived

The Phase-3 vertical-slice **file-by-file architectural map** moved to
[`docs/phase-records/phase-3.md`](docs/phase-records/phase-3.md) (2026-06-21) to
keep this handoff lean. It is still the architectural reference §3/§4/§5 below
build on — only its location changed.

---

## 3. Key design decisions & rationale

### 3.1 Element storage stays in the executive; the solver sees `ElemStore`
`Vec<Box<dyn DssObject>>` per class; the circuit holds `Vec<ElemRef>` lists;
solution machinery walks them through `ElemStore` + `as_ckt_element_mut()`.
Zero unsafe, no double ownership.

### 3.2 Signal flags instead of `ActiveCircuit` globals *(load-bearing)*
Elements set `cd.signal_bus_name_redefined`/`cd.yprim_invalid`; the executive
propagates after the edit loop. Equivalent because nothing reads the globals
mid-edit (first reader is `BuildYMatrix` at solve time).

### 3.3 Compensation-current loads
Loads are stamped into Y **and** inject `Yprim·V − model current`; iteration
equality in the gates is the regression test for this.

### 3.4 Parse-time reference snapshots + deferred cross-element writes (Phase 4)
Pascal resolves object references mid-parse against live pointers and lets
recalc *read* (and `Set_TapNum` *write*) the target at any time. The Rust edit
loop holds only a read view of foreign classes, so:
- reads needed later (control `RecalcElementData` at `EndEdit`) come from a
  `RefSnapshot` captured at resolution time — same staleness semantics as
  Pascal (a control refreshes only on its own recalc);
- writes (`TapNum`) become queued `RefAction`s the executive applies right
  after the edit, with the writer keeping its snapshot in sync via the
  identical clamp. Nothing observes the target in between.

### 3.5 `NOT_PORTED` property flag
Catalog/machinery references that belong to later phases hard-error on set —
a script that needs unported machinery cannot produce silently-wrong numbers.

### 3.6 Controls are invisible to Y
`ElemKind::Control` elements join the device list (so `ProcessBusDefs` walks
them in creation order — node order matches the oracle) but never the PD/PC
lists; `yprim` stays `None` and the Y build skips them.

---

## 4. Empirical oracle facts (cumulative highlights)

- `?`, `Edit`, `~`, `Solve`, `Set`, `Get` are **circuit-gated** (error 301).
- `Solution.Iterations` = the total over control iterations, assigned at the
  end of `SolveSnap`.
- `YNodeOrder` = `ProcessBusDefs` allocation order over enabled elements in
  creation order — **including control elements** (their bus is set in
  `RecalcElementData`; they add no nodes in the IEEE feeders).
- The `DoubleSymMatrixProperty` getter is broken upstream (reads a field
  address as the array) — Capacitor `CMatrix`, Reactor `RMatrix`/`XMatrix`
  always dump garbage; canonicalized to zeros on both sides.
- `Circuit.Losses` skips shunt PD elements; `Circuit.TotalPower` = Σ sources
  `Power[1]`·1e-3 (not negated); element `Powers` = `GetPhasePower`·1e-3 over
  all conductors/terminals.
- RegControl `TapNum` get/set maps tap↔integer through the *controlled
  winding's* (TapWinding) Min/Max/Increment: `tapnum=5` on a 32-tap winding
  moves the tap to 1.03125; reads back 5. `winding=` resets `TapWinding`.
- CapControl `type=time` (and `follow`) forces `Terminal=1` and monitors the
  capacitor itself; a missing `capacitor=`/`element=` raises (303); `vbus=`
  set during parse warns "Did you wait until buses were defined?" and reverts
  the flag (bus list doesn't exist yet) — faithfully reproduced.
- Controls-off feeders: IEEE13 41 nodes, IEEE37 117, IEEE123 278; all solve in
  exactly 3 fixed-point iterations.

---

## 5. `TODO(compat)` / deferrals

Grep `rg "TODO\(compat\)"` for the full marker list (26 sites). Notable:
truncated `CALPHA`/`pi`/`0.001732`/`57.29577951` constants, FPC banker's
`Round` shims, LineCode `Repair`=0 default, the `DoubleSymMatrix` zero-matrix
getter.

`NOT_PORTED` (hard parse error; every site points at its phase):
- Line `geometry` — **ported (WP7.1 step 3a)**: resolves a `LineGeometry`, runs
  `FetchGeometryCode` + `FMakeZFromGeometry` (the Carson `Zmatrix`/`YCmatrix`).
  Line `spacing`/`wires`/`cncables`/`tscables` stay `NOT_PORTED` until step 3b
  (the `FetchLineSpacing`/`SetWires`/`FMakeZFromSpacing` path, PORTING_PLAN
  §Phase 7 sub-block 1).
- Reactor `RCurve`/`LCurve` — Phase 5 (XYcurve) — XYcurve is now ported; the
  fetch is still `NOT_PORTED` (only the harmonic `CalcYPrim` consumes it, Phase 7).
- CapControl `ControlSignal` — Phase 5 (LoadShape); still `NOT_PORTED` (the
  `Follow` control type that consumes it has no corpus case — WP5.6's `Sample`
  records the Pascal abort error if reached); `UserModel`/`UserData` — never
  (no DLL loading in safe Rust).
- LoadShape `CSVFile` — **ported (WP5.2b)** via the deferred-`FileLoad` path.
  `SngFile`/`DblFile`/`PQCSVFile` (binary/2-col input) stay `NOT_PORTED` until a
  gate needs them. Single-precision arrays + `MemoryMapping` (MMF) and
  `Action=DblSave`/`SngSave` (binary output) — not ported (no corpus case).
- TempShape (`TShape`)/PriceShape `CSVFile` — **ported (WP5.2c)** via the same
  deferred-`FileLoad` path. `SngFile`/`DblFile` (binary input) and
  `Action=DblSave`/`SngSave` (binary output) stay `NOT_PORTED`.
- GrowthShape `CSVFile`/`SngFile`/`DblFile` — file-input machinery, when a
  gate needs it.

Other deferrals: Transformer GIC path (<0.51 Hz) + harmonics interplay
(Phase 7); RegControl/CapControl `Sample`/`DoPendingAction` **wired into the
control loop (WP5.7)**; RegControl/ControlQueue debug-trace files (flag
stored, no file — port with Monitors, Phase 6+); `MakePosSequence` everywhere
(Phase 6+); `BusCoords` **ported (WP5.8)**; Monitors/EnergyMeters
`sample_all`/`EndOfTimeStepCleanup` are no-op hook stubs at the SolveDaily/
Yearly/Duty call sites (Phase 6); Newton algorithm, harmonics/dynamics/
faultstudy/Monte-Carlo/load-duration/`SolveGeneralTime` solve modes (Phase 7);
`Show`/`Export`/`Dump`/`Select`/... executive verbs record "not ported".

---

## 6. How to run / regenerate

```bash
# Gate (must be green before any commit)
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Run a script
cargo run -p dss-cli -- path\to\script.dss

# Regenerate goldens (MANUAL ONLY, pinned versions in tools/golden/PIN.txt)
python tools/golden/gen_props.py       # -> tests/golden/props/<class>.json   (Phase 2+)
python tools/golden/gen_slice.py       # -> tests/golden/slice.json           (Phase 3)
python tools/golden/gen_phase4.py      # -> tests/golden/phase4/*.dss + phase4.json
python tools/golden/gen_phase5.py      # -> tests/golden/phase5/<scenario>.json
python tools/golden/gen_phase6.py      # -> tests/golden/phase6/<scenario>.json
python tools/golden/gen_checkpoints.py # -> tests/golden/checkpoints/<scenario>.json
```
Oracle pin: Python 3.12.4, dss-python 0.15.7, dss-python-backend 0.14.5
(the same dss_capi release vendored in `.inputs/dss_capi`). `python` works in
this environment; the `py` launcher is broken — use `python` directly.

---

## 7. Phase 7 — inherited deferrals & architecture in place

> **The current frontier** (active step, branch, what's next, commit state) lives
> in the header up top and in **§1e** — not restated here, to avoid the two drifting
> apart. This section is the stable Phase-7 reference: what the phase inherits and
> what is already wired for it. Execute per `PHASE7_PLAN.md §0` (six
> independently-gated sub-blocks, risk-ascending: line constants → protection →
> DER → harmonics → dynamics → faultstudy/AutoAdd-modes/`Feeder`).

**What Phase 7 inherits / must finish (deferrals Phase 6 left explicit):**
- **DER classes** `Storage`/`PVSystem` (+ `InvControl`/`ExpControl`) and the real
  `StorageController` behavior — the WP6.8 StorageController is a parse-only
  skeleton (empty fleet → 37201); `solution/meters/zones/build.rs::is_zone_pce`
  carries a `TODO(WP7)` to add PVSystem/Storage to the zone allow-list once they
  exist.
- **Protection** `Relay`/`Recloser`/`Fuse`/`SwtControl`/`Fault` — until one sets
  `Flg.HasOCPDevice`, `RelCalc` aborts with #52902 (oracle-faithful) and the
  ported SAIFI/SAIDI/section math below the abort stays dormant
  (`GetOCPDeviceType` inlined to 0, `TODO(WP7)`).
- **Line constants** `WireData/CNData/TSData/CableData/LineSpacing/LineGeometry`
  + Carson — Line's `geometry`/`spacing`/`wires`/`cncables`/`tscables` are
  `NOT_PORTED` (round-trip empty only).
- **Dynamics & harmonics** (Generator/Storage `DoDynamicMode`/`DoHarmonicMode`,
  state vars beyond names/count) + `MakePosSequence` everywhere; Monitor modes
  3/4/7/8/10/12 build their header but defer the sample body; Transformer GIC
  (<0.51 Hz).
- **AutoAdd solve mode** (`circuit/auto_add.rs` skeleton) — needs aux-current
  injection (`UseAuxCurrents`) + meter-register sampling in the solve loop; the
  options round-trip but the mode keeps its "Unknown solution mode" error.
- **ReduceAlgs** zone reduction — blocked on the unported `TLineObj.MergeWith`.

**Architecture already in place for Phase 7:** the control loop dispatches
through `ElemStore::{obj,pair_mut,triple_mut}` + `DssObject::as_any_mut`; the
meter/monitor `sample_all_monitors_and_meters`/`end_of_time_step_cleanup` hooks
have real bodies; the zone-build dispatcher (`solution/meters/mod.rs`) fires from
`build_y_matrix` after bus reprocessing; `TakeSample`/`Integrate` + the
reliability fault-rate sweep are ported. Still Phase 8: the `SystemMeter`
register core and all demand-interval/phase-voltage/`Show`/`Export` files.
