# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-22 — **Phase 7 IN PROGRESS** (branch
`phase-7-extended-elements`): **WP7.1 COMPLETE; WP7.2 (Protection) IN PROGRESS —
step 1 done** (the `Fault` element)**, step 2a done** (the `SwtControl` switch
control)**, step 2b done** (the `Fuse` per-phase TCC protection)**, step 2c done**
(the `Recloser` overcurrent recloser); **next = WP7.2 step 2d (`Relay`)**. WP7.1 landed the Carson line-constants engine
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
tests. Full per-step detail in **§1e**.

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
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | 🚧 in progress — `PHASE7_PLAN.md` (WP7.1–WP7.10); branch `phase-7-extended-elements`; **WP7.1 done**, **WP7.2 (Protection) in progress — steps 1 + 2a + 2b + 2c done** (Fault, SwtControl, Fuse, Recloser); **next = WP7.2 step 2d (Relay)**. Per-step detail in §1e |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 463, golden_feeders 1,
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

### Phase 5 gate — green
- `golden_feeders_controls.rs`: the **unmodified** IEEE13/IEEE37/IEEE123
  masters (controls active; IEEE123 issues the `post: ["solve"]` from
  `cases.json`) match the committed Phase-0 goldens
  `tests/golden/{ieee13,ieee37,ieee123}.json`: converged + total iterations
  **exact** (ieee13: 11), `YNodeOrder` exact, RegControl `tap_number` and
  capacitor `states` **exact**, final transformer taps at 1e-12 rel (not
  bitwise: the engines' ~1e-9 sparse-solver voltage differences can shift a
  banker's-rounding boundary and repartition the *same net* tap movement into
  a different step sequence, leaving the float accumulation an ulp apart —
  the integer tap_number is the exact discrete check), node voltages /
  element powers / currents at 1e-6 rel, total power + losses at 1e-6, and
  **every element's full property dump** via the numeric-skeleton comparator.
  **`ieee34mod1` (stretch) passes too** — no `#[ignore]` needed.
- `golden_phase5.rs` vs `tests/golden/phase5/*.json` (one file per scenario;
  `tools/golden/gen_phase5.py`,
  command-replay like slice.json): `daily_ieee13` (24 hourly steps, every load
  on a 24-pt shape, regcontrol event logs on), `duty_2bus` (12×300 s steps,
  TIMEDRIVEN), `eventlog_ieee13` (`Set Log=yes`), `capcontrol_micro` (kvar
  control opens Cap1). Per-step `dblHour` exact; **the event logs match the
  oracle line-for-line** (normalized), pinning every tap change/cap switch of
  the trajectories; final taps/tap numbers/states exact. **Per-step iteration
  counts exact on every step and per-step voltages at 1e-6 rel** — the whole
  daily trajectory tracks the oracle since `build_y_matrix` restamps each
  load's shape-scaled `Yeq` per Y build (commit `a6903f1`).

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

### Phase 4 gate (`crates/dss-core/tests/golden_feeders.rs`) — green
The three committed **controls-off variants** (`tests/golden/phase4/
{ieee13,ieee37,ieee123}_controlsoff.dss`, generated from the unmodified IEEE
masters by `tools/golden/gen_phase4.py`) compile and solve in both engines;
against `tests/golden/phase4.json` (pinned oracle) the Rust engine matches:
- converged flag and fixed-point iteration counts **exactly** (3/3/3);
- `YNodeOrder` **exactly** (41 / 117 / 278 nodes — control elements attach to
  existing buses and add none);
- node voltages within **1e-6 rel** (1e-9 abs floor);
- **every element's** terminal powers and currents within 1e-6 rel
  (1e-4 abs floor — dead-end branch currents are differences of nearly equal
  voltages, so 1e-6-rel voltage agreement caps absolute current agreement at
  the µA scale), in the oracle's First/Next (= creation) order, names checked;
- total power and total losses within 1e-6 rel.

The Phase 3 gate (`golden_slice.rs`, 13 scenarios) stays green, and the CLI
runs the real masters end to end: `cargo run -p dss-cli -- script.dss`.

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

### 1e WP7.2 step 1 — the `Fault` element (`pd/fault.rs`) — ✅ done, gate-green

First step of WP7.2 (Protection): the `Fault` object (`PDElements/Fault.pas`,
`TFaultObj`) — an uncoupled multi-phase **conductance** branch and the FaultStudy
input (WP7.9). Landed registered, snapshot-solvable, and pinned offline.
- `crates/dss-core/src/elements/pd/fault.rs` (+ `fault/tests.rs`): the element
  (`define_properties!`-style `class_props`, struct, side effects, `MakeLike`),
  `CalcYPrim` (SpecType 1 = single `G=1/r` diagonal; SpecType 2 = `Gmatrix`), and
  the time-mode `CheckStatus`/`Reset`/`FaultStillGoing` (enable past `ONtime`,
  temporary self-clear below `MinAmps`). `r` stores its inverse `G`
  (`InverseValue`); `GMatrix` shares the oracle's DoubleSymMatrix garbage-getter
  bug (rendered zeros, like Capacitor.CMatrix).
- Class type `FAULTOBJECT or NON_PCPD_ELEM`: a `TPDElement` with a YPrim in the
  system Y but **excluded** from `pd_elements` (Pascal `AddCktElement`) — added to
  `ckt_elements` (so it stamps) + a new `Circuit.faults` list only. New
  `ElemKind::Fault`; registered after Reactor (`construct.rs`, DSSClassDefs:222).
- **Control-loop wiring** (the dormant Phase-7 placeholders, now live): a new
  `solution/faults.rs` with `check_fault_status` (the control-iteration loop,
  `power_flow.rs` — sets `system_y_changed` when a fault toggles `Is_ON`, per
  Pascal `Set_YprimInvalid`→`SystemYChanged`) and `reset_faults` (`DoResetFaults`,
  wired into the `Set mode=` tail and `Reset Faults` 'F' selector). Duty/event/time
  modes set `control_mode = TIMEDRIVEN`, so `CheckStatus` fires; snapshot is
  `CTRLSTATIC` (no-op).
- **MonteFault** randomization (`Randomize` + `RandomMult` jitter) is deferred with
  its solve loop to WP7.9; `CalcYPrim` keeps the Pascal `RandomMult = 1.0` guard
  for every non-MonteFault mode, so the field is inert but faithful.
- Gates (all oracle-probed): `props.json` `fault.json` (6 scenarios) via
  `gen_props.py`; 8 inline tests pinning YPrim entry-by-entry (1φ `r`, 3φ `r`, 2φ
  `Gmatrix`, off=zero), a snapshot 3φ fault drawing **1342.808 A** (full-circuit),
  and a duty-mode temporary fault logging **`**APPLIED**`** at the probed step.
  `gen_props.py` `_NUM_RE` extended to zero the non-finite `Nan`/`Inf` the oracle's
  garbage matrix getter can emit (3φ `GMatrix`). dss-core lib **392→400** (→401
  with the audit-tests follow-up); full three-command gate green on **stable**.
- **Corpus migration deferred to the WP7.2 gate (step 4):** the
  `unsupported_class={Fault,Fuse,Recloser,Relay,SwtControl}` cases migrate once the
  protection set is complete (PHASE7_PLAN §3 WP7.2 step 4). A targeted
  `phase7/protection*.json` golden is part of that gate.
- **Audit-code follow-up:** `MakeLike` was missing the `TPDElement` rating fields
  (`NormAmps`/`EmergAmps`/`FaultRate`/`PctPerm`/`HrsToRepair`) that
  `TPDElement.MakeLike` copies — added them; pinned by extending the
  `fault_makelike` props scenario with `faultrate=0.5 pctperm=80 repair=4`.
  (Verified clear: the `phases` side effect signals `bus_name_redefined` through
  `set_nconds` — no divergence; `CalcYPrim`/`CheckStatus` checked against Pascal +
  the oracle.)
- **Audit-tests follow-up:** the temporary-fault coverage exercised only the
  `**APPLIED**` path; added `temporary_fault_clears_below_minamps` (MinAmps above
  the fault current → self-clear), pinning the `**CLEARED**` event from the oracle
  probe. dss-core lib **400→401**. Gate green.

### 1e WP7.2 step 2a — the `SwtControl` switch control (`control/swt_control/`) — ✅ done, gate-green

The first protection **control** (`Controls/SwtControl.pas`, `TSwtControlObj`): a
manual/automatic **switch** that opens or closes every phase conductor of a
controlled element's terminal after a time delay, and can be *locked*. The
simplest control of the WP7.2 set — no TCC/sensing — so it lands the generic
"control opens/closes a controlled PD terminal" machinery the protection devices
(Fuse/Recloser/Relay, step 2b) reuse.
- `crates/dss-core/src/elements/control/swt_control/{mod,accessors,tests}.rs`: the
  element on the WP5.7 control sweep — `Sample` (queue the pending lock/switch
  action; reads **no** monitored quantity), `DoPendingAction` (lock/unlock or
  open/close all phases of the switched terminal + `Opened`/`Closed` event log),
  `Reset` (restore to `NormalState`). Registered after StorageController (Pascal
  `DSSClassDefs.pas:249`); `ElemKind::Control`, joins `ckt.controls`.
- **Generic conductor-open machinery:** new `CktElementData::set_terminal_closed`
  / `terminal_all_phases_closed` (Pascal `Set_/Get_ConductorClosed(0)` with the
  active terminal = the switch terminal) — opens/closes a *generic* controlled
  element (Line, etc.), marking `yprim_invalid` (the open-conductor Kron reduce in
  the existing `do_yprim_calcs` does the rest). The control-loop dispatch
  (`solution/controls/dispatch.rs`) gained `ControlKind::Swt`: `Sample` borrows
  only the control; `Action`/`Reset` borrow the control + the controlled element
  via `pair_mut` + `as_ckt_element_mut` (generic — any switched class).
- **Property quirks settled against the oracle** (probed, `swtcontrol.json`):
  `Action`/`Normal`/`State` all map onto the one `CurrentAction` field — the text
  `?` dump renders it (Action `close`/`open`, Normal/State `closed`/`open`); the
  `State` read-function `GetState` is **not** used by the dump (proved: `action=open`
  leaves the line *closed* yet `State` dumps `open`). They are `ConditionalReadOnly`
  on `Locked` — a write while locked is **ignored** (`lock=yes action=open` ⇒
  `Action=close`; parse-order sensitive). `State=` additionally forces the
  controlled element to that state at parse time, deferred as a new
  `RefAction::SetSwitchClosed` (applied generically by the executive through the
  CktElement base, since the switched element can be any class) — the established
  RegControl-`TapNum` deferred-write pattern.
- Two `SwtControl` enums registered (`swt_control_action` close/open,
  `swt_control_state` closed/open; EControlAction ordinals CTRL_CLOSE=2/CTRL_OPEN=1),
  plus CTRL_RESET/CTRL_LOCK/CTRL_UNLOCK added to the shared `EControlAction` set.
- Gates (all oracle-probed): `props.json` `swtcontrol.json` (7 scenarios —
  default, action/normal/state=open, lock+delay, the locked-read-only path, and
  makelike) via `gen_props.py`; **14 inline tests** — `Sample` arm/no-arm,
  `DoPendingAction` open/close (terminal flipped + `OPENED`/`CLOSED` event), the
  lock-blocks-open and locked-read-only paths, the `State=` deferred-force queue,
  `Reset`, `MakeLike`, plus two executive tests (a parallel-fed switch opening on
  `action=open` in a duty solve; `state=open` forcing the line open at parse). The
  event-log line format is `Element=SwtControl.sw1, Action=OPENED` (probe-confirmed
  it logs without `Set Log=yes`). dss-core lib **401→415** (→418 with the
  audit-tests follow-up); full three-command gate green on **stable**.
- **Corpus migration deferred to the WP7.2 gate (step 4):** the lone corpus
  SwtControl case (only bare `switchedobj=` appears — no `action`/`state`/`lock`
  usage across the corpus) migrates with the rest of the
  `unsupported_class={Fault,Fuse,Recloser,Relay,SwtControl}` set once the protection
  block is complete (PHASE7_PLAN §3 WP7.2 step 4), alongside the targeted
  `phase7/protection*.json` golden.
- **Audit-code follow-up:** of the three surfaced notes, one is a non-bug kept
  as-is (the typed `GetState` accessor is unreachable in this CAPI-less port — the
  `?` dump faithfully returns `CurrentAction` on **both** engines, so reading the
  live `Closed[0]` instead would *diverge* from the oracle; the live semantics
  belong to the unported typed getter / a future report path). The
  `ActiveTerminalIdx` note was **fixed** for literalness — `DoPendingAction` now
  sets the controlled element's `active_terminal := ElementTerminal` before the
  case (for every code, incl. LOCK/UNLOCK), as Pascal does (behaviorally inert —
  nothing reads it after a lock — but a 1:1 port; covered by
  `do_pending_sets_controlled_active_terminal_even_for_lock`). The third — *Reset
  gated `system_y_changed` on an all-or-nothing change check* — was promoted to a
  **real fix** on review: Pascal
  `Reset` does `Closed[0] := …` unconditionally, and the change-gated version
  could miss a real Y change on a **partially-open** terminal (one phase closed,
  the rest open ⇒ `terminal_all_phases_closed` reads false ⇒ `was == want` ⇒ the
  rebuild is skipped ⇒ stale system Y, since `build_y_matrix` is gated solely on
  `system_y_changed`). Refactored the SwtControl Reset into a `reset_with(ctrl)`
  method (mirroring `CapControl::reset_with`) that raises `system_y_changed`
  unconditionally when a force is applied; **fixed the same dirty edge in
  `CapControl::reset_with`** (`want_closed.is_some()`, was
  `is_some_and(want != was)`). `RegControl::Reset` is clean (touches no element —
  just `PendingTapChange=0; Armed=FALSE`). Both fixes carry a **fail-on-regression
  partial-open test** (proven: re-introducing the change-gate makes each test
  fail). dss-core lib **418→420** (SwtControl 18 + a CapControl partial-open test;
  net of the `reset_control_side` test reshape). Gate green.
- **Forward guard for WP7.2 step 2b (Recloser/Relay/Fuse) — `system_y_changed`
  discipline + required tests:** a protection *trip* opens the controlled
  element's conductors, so every `DoPendingAction`/reset that forces conductors
  (`set_terminal_closed`/`Closed[0] := …`) must raise `system_y_changed`
  **unconditionally** (or via an *exact* per-conductor check), **never** via an
  all-or-nothing `terminal_all_phases_closed`/`is_closed` aggregate — the
  `build_y_matrix` trigger is gated solely on `system_y_changed`, so a
  partially-open terminal otherwise slips a real change past the rebuild → stale Y
  (the step-2a Reset dirty edge, `d0addb4`). **Each protection device must ship a
  partial-open fail-on-regression test** modeled on
  `reset_with_partial_open_*` (SwtControl/CapControl). The verified sweep found no
  *other* current sites (every existing `system_y_changed` write is unconditional
  or gated on an exact check — scalar tap `v != putap`, `add_step`/`subtract_step`,
  fault `Is_ON` toggle, direct `yprim_invalid` propagation); per-phase open is a
  single shared base path (`apply_yprim_open_conductor_calcs`), and a winding tap
  is scalar (per-phase regulation = separate single-phase regulators). The
  unported `open`/`close` exec commands must likewise set the flag when ported.
  Recorded in PHASE7_PLAN §3 WP7.2 step 2.
- **Audit-tests follow-up:** the audit found two genuinely-untested new paths and
  one under-pinned guard; added 3 tests (dss-core lib **415→418**): an executive
  `reset_restores_switch_to_normal_via_dispatch` (the dispatch `Reset` element-force
  — `reset` re-closes a `state=open` line; oracle-probed), a `reset_yes_unlocks_and_
  restores_with_force` (the `Reset=yes`/DoReset unlock + restore + deferred force),
  and `locked_ignores_normal_and_state_writes` (the ConditionalReadOnly guard on
  Normal/State, previously pinned only for Action). Gate green.

### 1e WP7.2 step 2b — the `Fuse` per-phase TCC protection (`pd/fuse/`) — ✅ done, gate-green

The first **TCC/sensing** protection device (`PDElements/fuse.pas`, `TFuseObj`):
a per-phase fuse on the WP5.7 control sweep that, despite living in the Pascal
`PDElements/` tree, is a `TControlElem` (zero Yprim/current). It monitors one
element's terminal currents and **blows individual phases** of the controlled
element when a phase current stays above the `FuseCurve` pickup. Each phase has
its own link state, arm flag, and queue action — so it lands the
sensing/TCC/per-phase machinery Recloser (2c) and Relay (2d) reuse.
- `crates/dss-core/src/elements/pd/fuse/{mod,accessors,tests}.rs`: `Sample`
  reads `MonitoredElement.GetCurrents` and, per closed phase, evaluates
  `FuseCurve.GetTCCTime(Cmag / RatedCurrent)`, arming/disarming a per-phase
  `ControlQueue` action at `TripTime + Delay`; `DoPendingAction(phase)` opens
  that one conductor and logs `Phase N Blown`; `Reset` restores each phase to
  `Normal`. Registered with the protection controls (Pascal `DSSClassDefs`
  Relay/Recloser/Fuse), `ElemKind::Control`; dispatch (`ControlKind::Fuse`)
  borrows the control + controlled + monitored (the default fuse monitors its
  own controlled element — the same-element clone path, like CapControl).
- **`TCC_Curve.GetTCCTime` ported** (`tcc_curve/mod.rs`): the log-log
  interpolation over the (already-precomputed) `log_c`/`log_t` arrays, including
  the `LastValueAccessed` hunt cache (now a per-owner field — identical results
  to Pascal's per-curve field for the monotonic curves that are the only kind in
  practice). Unit-tested against the built-in `tlink` curve (Python reference).
- **New shared machinery (Recloser/Relay will reuse):** per-conductor
  `CktElementData::set_conductor_closed`/`conductor_closed` (Pascal
  `Set_/Get_ConductorClosed(index>0)` — a single phase, vs step-2a's
  whole-terminal `set_terminal_closed`); `RefAction::SetConductorsClosed` (the
  per-phase `State=`/`Action=` parse-time force, applied generically by the
  executive through the CktElement base); and a new property type
  `PropType::MappedStringEnumArray` (a `SizeIsFunction` mapped-enum array sized
  by `DssObject::array_size`, dumped `[s1, s2, ]`) for the per-phase
  `Normal`/`State`, with `get_enum_array`/`set_enum_array` base hooks.
- **`FuseCurve` default `tlink`:** Pascal's constructor does
  `Find('tlink')`; our constructor cannot reach the registry, so the executive
  resolves the (default or explicit) curve name through the same `foreign` view
  the property edits use — cloning the `TccCurveObj` into the Fuse for
  solve-time `GetTCCTime`, after the edit loop in `command.rs`. The built-in
  `tlink`/`klink`/… curves already exist (`CreateDefaultDSSItems`).
- **Property quirks settled against the oracle** (probed, `fuse.json`):
  `MonitoredObj` defaults `SwitchedObj` to the same element (and `MonitoredTerm`
  → `SwitchedTerm`); `Normal`/`State` are per-phase enum arrays sized by
  `ControlledElement.NPhases`, dumped `[closed, closed, closed, ]` (a short
  input sets only the leading phases — `state=[open]` opens only phase 1);
  `State=` forces the controlled conductors **per phase** at parse time;
  `Action` (deprecated close/open) sets all phases then runs the State side
  effect and dumps empty; `MakeLike` copies the refs/rating/states but **not**
  `DelayTime` or `NormalStateSet` (Pascal omits them).
- Two `Fuse` enums registered (`fuse_action` close/open, `fuse_state`
  closed/open; EControlAction ordinals CTRL_CLOSE=2/CTRL_OPEN=1).
- Gates (all oracle-probed): `props.json` `fuse.json` (8 scenarios — default,
  1-phase, action=open, state all/partial, normal partial, explicit
  curve+rating+switched, makelike) via `gen_props.py`; **20 inline tests** — 8
  Fuse (Sample arm/disarm, per-phase blow + `PHASE N BLOWN` event, disarmed
  no-op, Reset, the `state=[open]` partial force, MakeLike) incl. a partial-open
  `reset_with` fail-on-regression guard + 3 executive tests (per-phase parse
  force, default `tlink` resolution, end-to-end overcurrent blow), and 4
  `TccCurveObj::get_tcc_time` tests. dss-core lib **421→435**; full
  three-command gate green on **stable** (incl. the always-on `corpus_live`).
- **`system_y_changed` discipline (the step-2a guard):** the per-phase blow
  (`DoPendingAction`) and `reset_with` raise `system_y_changed` **unconditionally**
  on a conductor flip (never gated on an aggregate); `reset_with` carries a
  **partial-open fail-on-regression test** as required by the step-2b guard.
- **`HasOCPDevice` / recalc-Closed-resync deferred to WP7.2 step 3** (the
  reliability activation, as PHASE7_PLAN §3 sections it): the Fuse's
  `RecalcElementData` would `Include(ControlledElement.Flags, HasOCPDevice)` and
  resync the controlled `Closed[i]`, both reaching the controlled element which
  `recalc` cannot see; they land with `GetOCPDeviceType` in step 3 (the
  parse-time `State=` force already drives the element). Noted in `fuse/mod.rs`.
- **Corpus migration blocked on co-occurring classes:** every Fuse-using corpus
  case is also tagged `unsupported_class=…Recloser,Relay,PVSystem,Storage…`
  (e.g. `Test/IEEE13_CDPSM.dss`), so none can migrate to `solvable_now` until
  Recloser/Relay (2c/2d) and the DER block land — migration stays at the WP7.2
  gate (step 4), with the targeted `phase7/protection*.json` golden.
- **Audit-code follow-up:** the `/audit-code` pass confirmed a faithful 1:1 port
  (no Critical/Major); the one fixed finding was the dropped FUSEMAXDIM-overflow
  **warning** (Pascal `fuse.pas` DoSimpleMsg 404 when the monitored element has
  >6 phases) — now emitted in `recalc`. The other notes are kept as-is: the
  per-phase arrays / `GetFuseStateSize` are deliberately `FUSEMAXDIM`-capped
  (Pascal's own getter carries a documented invalid-access risk for >6 phases, and
  such fuses don't exist in the corpus); the recalc `Closed[i]` resync is redundant
  at parse (the `State=`/`Action=` force already drives it) and rides with the
  step-3 `HasOCPDevice` work; `CondOffset` is confirmed vestigial in Pascal (Sample
  reads `cBuffer[i]`). Gate green.
- **Audit-tests follow-up:** the `/audit-tests` pass found the partial-open
  `reset_with` guard ineffective — its "all-closed target" seed made a reintroduced
  aggregate gate (`was != want`) compute the same result as the correct code, so it
  could not fail on the regression it names. **Reseeded** it to terminal
  `[open, closed, closed]` + `normal=[CLOSE,OPEN,CLOSE]` so the aggregate reads false
  **both before and after** the reset while phases 0/1 actually flip (proven: the
  reseeded test now fails when the aggregate gate is reintroduced; the old seed did
  not). Also strengthened the event-log assertion to the full normalized line
  (`Element=Fuse.f1, Action=PHASE N BLOWN`) and added two tests: a `RatedCurrent`
  divisor trip (`5 A / 10 A = 0.5 pu` below pickup → no arm — pins `Cmag/RatedCurrent`
  against a dropped/inverted divisor) and the missing-`SwitchedObj` error path
  (Pascal `#405`, oracle-confirmed). dss-core lib **435→437**. Gate green.

### 1e WP7.2 step 2c — the `Recloser` overcurrent recloser (`control/recloser/`) — ✅ done, gate-green

The second **TCC/sensing** protection device (`Controls/Recloser.pas`,
`TRecloserObj`): an overcurrent recloser on the WP5.7 control sweep that monitors
a PD terminal's currents, trips the controlled element's **whole terminal** open
on a phase/ground TCC pickup, then **recloses** it after a configured interval —
repeating up to `Shots` operations before locking out, the first `NumFast` on the
*fast* curves and the rest on the *delayed* curves. Reuses the step-2b sensing
machinery (`TccCurveObj::get_tcc_time`, the control-queue arm/disarm, the
`SetSwitchClosed` whole-terminal force from step 2a).
- `crates/dss-core/src/elements/control/recloser/{mod,accessors,tests}.rs`:
  `Sample` reads `MonitoredElement.GetCurrents` and evaluates the ground-sum and
  per-phase `GetTCCTime(Cmag / Trip)` (plus the inst-trip on operation 1),
  arming an `OPEN` then a reclose `CLOSE` on the queue; `DoPendingAction(OPEN/
  CLOSE/RESET)` flips the whole controlled terminal and advances/locks/resets the
  operation count, logging `Opened, Fast`/`Opened, Delayed`/`Opened, Locked Out`/
  `Closed` + `Phase`/`Ground Target`; `Reset` restores `NormalState`. Registered
  **before Fuse** (Pascal `DSSClassDefs.pas:243`, Relay/Recloser/Fuse order;
  Relay/2d still pending), `ElemKind::Control`; dispatch (`ControlKind::Recloser`)
  borrows control + controlled + monitored with the same-element clone path the
  Fuse uses.
- **New engine machinery (Relay/2d reuses both):**
  - `PropFlags::ARRAY_MAX_SIZE` + `PropDef::double_v_array_max(name, max)` — the
    Pascal `DoubleVArrayProperty` + `ArrayMaxSize` parse (`ParseAsVector(maxSize,
    array)`): reads **up to** `max` values, the object sets its own element count,
    the dump renders `array_size` of a fixed buffer. Drives `RecloseIntervals`.
  - the **integer-dump `VALUE_OFFSET`** (Pascal `GetObjInteger` subtracts the
    offset) — `Shots` stores `NumReclose = Shots − 1` and dumps `NumReclose + 1`.
- Two `Recloser` enums registered (`recloser_action` close/open/trip,
  `recloser_state` closed/open/trip → ordinals 2/1/1; `trip` aliases `open`, so an
  opened recloser dumps `open`). Default `PhaseFast`/`PhaseDelayed` resolve to the
  built-in `a`/`d` curves (`CreateDefaultDSSItems`) via the same `command.rs`
  `foreign`-view resolution the Fuse `tlink` uses (all four curves cloned in for
  solve-time `GetTCCTime`).
- **Property quirks settled against the oracle** (probed, `recloser.json`):
  `Action`/`State` map onto `FPresentState`, `Normal` onto `NormalState`; the
  first `State`/`Action` defaults `Normal` (`NormalStateSet`). `Shots` **and**
  `RecloseIntervals=(…)` both set `NumReclose` (last write wins): `shots=2
  recloseintervals=(1 3)` ⇒ Shots 3 / `[ 1 3]`, `shots=1` ⇒ NumReclose 0 ⇒ `[]`.
  `RecalcElementData` syncs the controlled terminal to `FPresentState` (the
  `SetSwitchClosed` RefAction — so `state=open` opens the line at parse). `MakeLike`
  copies the trips/curves/shots/intervals/normal state but **not** `DelayTime` or
  the TD* time dials (Pascal omits them).
- **`HasOCPDevice`/`HasAutoOCPDevice` deferred to WP7.2 step 3** (as the Fuse —
  the reliability flags reach the controlled element which `recalc` cannot see;
  they land with `GetOCPDeviceType`). The whole-terminal Closed[0] sync **is**
  done here (queued from recalc) — unlike the Fuse, the recloser has no parse-time
  element force, so the recalc sync is its only path for `state=`.
- Gates (all oracle-probed): `props.json` `recloser.json` (7 scenarios — default,
  full+ground curves+switched, shots-then-intervals aliasing, one-shot empty
  array, state=open, normal=trip, makelike) via `gen_props.py`; **22 inline
  tests** (Sample arm/reclose/disarm, fast↔delayed + inst selection, ground-sum
  trip, terminal-open skip; DoPendingAction trip/lockout/delayed/reclose/
  reset; Reset closed/open; MakeLike-omits-time-dials; 4 executive tests — default
  dump, shots/intervals aliasing, `state=open` forces the controlled terminal,
  end-to-end overcurrent trip). dss-core lib **437→459**; full three-command gate
  green on **stable** (incl. the always-on `corpus_live`).
- **Corpus migration blocked (same as 2b):** Recloser-using corpus cases also tag
  `Relay`/`PVSystem`/`Storage`, so migration stays at the WP7.2 gate (step 4)
  with the targeted `phase7/protection*.json` golden.
- **Audit-code follow-up (`/audit-code` step 2c):** the pass confirmed a faithful
  1:1 port — **no Critical/Major**. `Sample`/`DoPendingAction`/`RecalcElementData`/
  `Reset`/`MakeLike` and the two engine pieces (`ARRAY_MAX_SIZE`, integer-dump
  `VALUE_OFFSET`) all match Pascal line-for-line; `reset_with` raises
  `system_y_changed` unconditionally (the step-2a dirty-edge discipline). The only
  fix was a **doc-comment clarity tweak** (`reclose_intervals[4]` is dead only for
  the default `Shots ≤ 4`; a larger `Shots` reads it, but Pascal's slot is
  uninitialized there too — the oracle dumps `Nan`, so `0.0` is the safe defined
  choice, probe-confirmed). The MakeLike whole-array copy vs Pascal's
  `[1..NumReclose]` and the NIL-element generic-abort text are unpinnable/degenerate
  (documented, no change). Gate green.
- **Audit-tests follow-up (`/audit-tests` step 2c):** closed the two Major test
  gaps the audit flagged plus the mandated dirty-edge guard. (1) **No trip *time*
  was pinned** — every Sample assertion was `queue_size`-only, so a dropped
  `+DelayTime`, a missing time-dial, or an interval-index off-by-one passed: added
  `sample_queues_trip_and_reclose_at_correct_times`, which `pop_time`s the OPEN and
  reclose CLOSE and asserts `TripTime = TDPhFast·GetTCCTime = 0.2`, OPEN at `+Delay
  = 0.25`, reclose at `+RecloseIntervals[0] = 0.75`. (2) The **end-to-end test
  under-asserted** (log substring only) — strengthened to `OPENED, FAST` **and**
  `line_term1_max_current(Line.l1) < 1.0` (the line actually opens, the Fuse
  precedent). (3) Added the **partial-open `reset_with` fail-on-regression guard**
  the WP7.2 step-2a rule (`d0addb4`/`d1f48231`) mandates for every protection
  control — seed `[closed, open, open]` + `normal=OPEN` ⇒ aggregate false before
  **and** after while phase 0 flips, so a reintroduced `was != want` aggregate gate
  fails it. Plus three Minor gaps: the `GROUND TARGET` event line, the OPEN-when-open
  / CLOSE-when-closed no-op guards, and the MakeLike curve-clone copy (a `like=`
  recloser still trips). dss-core lib **459→463**. Gate green. *(The trip/reclose
  **sequence**-level event-log-equality + final-state golden remains the planned
  step-4 `phase7/protection*.json` gate.)*

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
