# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-07-02 — **Phase 8 IN PROGRESS** (`PHASE8_PLAN.md` —
reporting/exports/Save). **WP8.1 COMPLETE, gate-green** (sub-steps 1+2: dispatch
skeleton `82b50fe`, output-path machinery + `Export Counts` + the `compare_export`
golden harness `929145c`). **WP8.2 IN PROGRESS:** sub-step 1 (`71067f7`) landed the
bus/node solution exports (`Voltages`/`BusCoords`/`NodeNames`/`YNodeList`);
sub-step 2a (`668bd18`) the aggregate PD/PC **power exports**
`Powers`/`Losses`/`P_byphase` + the mutable element-walk infra
(`for_each_enabled_elem` + `export_with_mut`); **sub-step 2b** the
**symmetrical-component family** `SeqVoltages`/`SeqCurrents`/`SeqPowers`
(`Phase2SymComp` + `PctNemaUnbalance` + PD ratings + `ColTol::gate`); **sub-step 2c
landed, gate-green** — the **per-terminal/per-conductor element exports**
`Currents`/`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`Taps`
(`CalcAndWriteCurrents`/`WriteNodeList`/`WriteElem*`/`ExportTaps`), the `ColSel`
name-prefix|index-parity harness refactor for the truncated-header mag/angle
reports, and the `ElemPowers` Vsource `Vterminal`-vs-EMF **order fix** (a real
Rust↔oracle divergence caught + proven, `-612.936`→`-612.729`). Detail in
the §1f Phase 8 record. Phase 8 lives on its own branch **`phase-8-reporting`**
(branched from the gate-green Phase-7 tip). **Phase 7 is COMPLETE but NOT merged to
`main`** (the per-phase merge is the explicit-request-only HARD STOP — `phase-8-
reporting` builds on top of `phase-7-extended-elements`); its retro audit (WP7.7 step 4 → WP7.8) found no Critical/Major bug (§1e
"Retro audit" under WP7.8). Tracked-open Phase-7 deferrals (both zero-corpus-payoff,
Plot-blocked): the **GFM grid-forming inverter mode** (NOT_PORTED loud abort across
Generator/PVSystem/Storage `DoDynamicMode`) and the **Generic/TD21 relay `Sample`**
logic.

All ten work packages landed gate-green: **WP7.1** (line constants & geometry),
**WP7.2** (protection: Fault/Fuse/Recloser/Relay/SwtControl + reliability
activation), **WP7.3** (DER A: DynamicExp + InvBasedPceData + PVSystem), **WP7.4**
(DER B: Storage + StorageController), **WP7.5** (DER C: InvControl + ExpControl),
**WP7.6** (Harmonics solve mode), **WP7.7** (Dynamics core: `SolveDynamic` driver +
Generator/PVSystem/Storage/IndMach012 state vars + Monitor mode 3 + DynEqPCE), **WP7.8**
(Converter/FACTS: VSConverter/VCCS/UPFC+UPFCControl/ESPVLControl), **WP7.9** (FaultStudy
mode; AutoAdd/MonteCarlo/LoadDuration/Feeder kept deferred — zero corpus cases), and
**WP7.10** (phase exit: marker sweep + gate + COVERAGE refresh).

Per-WP and per-step detail (decisions, audits, gate descriptions, the
real-port-bug write-ups) lives in **§1e** (one-line-per-step summaries) and the
archives under `docs/phase-records/`:
[`phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md),
[`phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md),
[`phase-7-wp3.md`](docs/phase-records/phase-7-wp3.md),
[`phase-7-wp4.md`](docs/phase-records/phase-7-wp4.md),
[`phase-7-wp5.md`](docs/phase-records/phase-7-wp5.md),
[`phase-7-wp6.md`](docs/phase-records/phase-7-wp6.md),
[`phase-7-wp7.md`](docs/phase-records/phase-7-wp7.md) (the completed WP7.7 steps).
Current scores: dss-core **lib 713**, **`solvable_now` 88** (the live corpus gate;
WP7.9 step 1 migrated the 3 ShortCircuitCases FaultStudy decks); oracle pinned to
dss-python 0.15.7 (backend = dss_capi 0.14.5, `tools/golden/PIN.txt`).

Phase 7 = DER, protection, line constants, harmonics, dynamics (PORTING_PLAN.md
§Phase 7, the largest phase ~18%). Earlier phases merged to `main` (newest first):
**Phase 6** (WP6.1–WP6.10 — meters/monitors/topology/Generator + the 8500-node gate
+ the live corpus gate; `--no-ff` `b98223a`, `main` not pushed to origin)
→ [record](docs/phase-records/phase-6.md); **Phase 5** (`10d3550`), **Phase 4**
(`5f27a25`). Their full logs and the per-WP detail live under `docs/phase-records/`
(§1b–1d indexes them) and the §1 table below.

**Standing toolchain note:** the gate runs on **`stable`** (`cargo +stable …`),
matching CI (`dtolnay/rust-toolchain@stable`) — no nightly dependency. `dss-core`
carries `#![allow(clippy::collapsible_match)]` (`d85d026`): clippy 0.1.96 (now on
stable) mis-fires that lint on the byte-faithful `match prop { CONST => if cond
{..} }` port idiom, and its autofix even drops `else` branches.

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
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | ✅ **COMPLETE** (WP7.1–WP7.10) — `PHASE7_PLAN.md`; branch `phase-7-extended-elements`, gate-green, **NOT merged to `main`** (explicit-request-only HARD STOP). WP7.1–7.6 (line constants, protection, DER, harmonics), WP7.7 (Dynamics core), WP7.8 (Converter/FACTS), WP7.9 (FaultStudy + AutoAdd/Feeder-deferred), WP7.10 (phase exit). Tracked-open deferrals: GFM grid-forming mode + Generic/TD21 relay `Sample` (both Plot-blocked, 0 corpus payoff). Per-step detail in §1e + `docs/phase-records/phase-7-wp{1..6}.md` |
| **8** | **Reporting: Export/Show/Save/Dump + executive tail + full ReduceAlgs** | 🚧 **IN PROGRESS** — `PHASE8_PLAN.md`. **WP8.1 COMPLETE, gate-green** (dispatch skeleton + GUI no-ops `82b50fe`; output-path machinery + `Export Counts` + the `compare_export` golden harness `929145c`). **WP8.2 IN PROGRESS:** sub-step 1 (`71067f7`) = bus/node solution exports; sub-step 2a (`668bd18`) = the aggregate PD/PC power exports `Powers`/`Losses`/`P_byphase` + the mutable element-walk infra + the MVA/kVA `Parm2` pre-parse; **sub-step 2b** = the symmetrical-component family `SeqVoltages`/`SeqCurrents`/`SeqPowers` + the `ColTol::gate` denominator-gate harness machinery; **sub-step 2c** = the per-terminal/per-conductor element exports `Currents`/`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`Taps` + the `ColSel` name-prefix\|index-parity harness refactor + the `ElemPowers` Vsource order fix. Branch `phase-8-reporting`. **next = WP8.2 sub-step 3 + completion gate** (`Y`/`Yprims`/`SeqZ`/`Summary`/`Result` + IEEE8500 goldens + `Export`-report corpus migration). Detail in §1f |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 722, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_phase6 1, golden_phase7 1,
                            # golden_phase7_protection 1, golden_phase8 19,
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
- **Live comparison (runs unconditionally in `cargo test`; the pinned oracle must
  be installed).** For each of the **84** `solvable_now` cases the gate
  compiles+solves on the Rust engine and on the pinned dss-python oracle
  (`tools/oracle/oracle_server.py`, a
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
- **Phase 7 WP7.2** (Protection) — Fault, SwtControl, Fuse, Recloser, Relay (9
  sub-types), reliability activation (`HasOCPDevice` + live `RelCalc`), and the
  step-4 gate (the `phase7_protection` trip/reclose golden, the `Open`/`Close` exec
  verbs, the SwtControl corpus migration). **Complete + gate-green on the
  `phase-7-extended-elements` branch (not yet merged);** the live §1e keeps a
  per-step summary + the Phase-7 carry-forward rules + the `DG_Prot_Fdr` tracked-open.
  → [`docs/phase-records/phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md)
- **Phase 7 WP7.3** (DER A) — `DynamicExp` (the diff-eq catalog object + its RPN
  expression interpreter), `InvBasedPceData` (the shared inverter PC-element base),
  and `PVSystem` (the power-flow PV element + zone admission + the Monitor mode-3
  fix + the GFM loud-abort). **Complete + gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp3.md`](docs/phase-records/phase-7-wp3.md)
- **Phase 7 WP7.4** (DER B) — the `Storage` element (the charge/idle/discharge state
  machine + integrated SOC) and the real `StorageController` fleet/dispatch (replacing
  the WP6.8 skeleton), plus the Storage-specific YPrim-rebuild fix. **Complete +
  gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp4.md`](docs/phase-records/phase-7-wp4.md)
- **Phase 7 WP7.5** (DER C, steps 1–4) — `RollAvgWindow`, the full `InvControl` (8
  modes + LPF/RiseFall + MonBus, both PVSystem and Storage DERs), `ExpControl`
  (the adaptive-`Vreg` volt-var control), and the step-4 corpus burn-down review.
  **COMPLETE + gate-green on the branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp5.md`](docs/phase-records/phase-7-wp5.md)
- **Phase 7 WP7.6** (Harmonics, steps 1–3) — the harmonics solve mode: the
  current-source family (VSource + Load) + the `SolveHarmonic`/`SolveHarmonicT`
  driver, the Thevenin DER family (Generator/PVSystem/Storage behind their
  subtransient reactance), and the monitor harmonic header + the `Set mode=`
  monitor/meter reset (Pascal `Set_Mode` tail); harmonics corpus burn-down is 0
  migratable (Phase-8/`Isource`/FaultStudy-blocked). **COMPLETE + gate-green on the
  branch (not yet merged).**
  → [`docs/phase-records/phase-7-wp6.md`](docs/phase-records/phase-7-wp6.md)
- **Phase 7 WP7.7** (Dynamics core, steps 1–3b cont.) — the `SolveDynamic`
  predictor/corrector driver + per-element dynamics state machinery for Generator /
  PVSystem / Storage / IndMach012, Monitor mode 3, the `Open`-verb fix, the
  `set_ITerminalUpdated` stamp sweep, and the DynEqPCE user-`DynamicExp` integration
  for all three PCE families. **✅ COMPLETE** (steps 1–4; GFM deferred,
  tracked-open). The completed-step detail (incl. all audit follow-ups) is archived;
  the live §1e keeps the concise per-step summary.
  → [`docs/phase-records/phase-7-wp7.md`](docs/phase-records/phase-7-wp7.md)

---

## 1e. Phase 7 record (branch `phase-7-extended-elements`) — ✅ COMPLETE

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

**WP7.2 (Protection) — ✅ COMPLETE.** Steps **1 (`Fault`), 2a (`SwtControl`),
2b (`Fuse`), 2c (`Recloser`), 2d (`Relay`), 3 (reliability activation), 4
(protection gate + corpus migration) done + gate-green**. Full per-step records
(decisions, audits, gate detail) archived at
[`docs/phase-records/phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md). In brief:
- **step 1 — `Fault` (`pd/fault.rs`):** an uncoupled multi-phase **conductance**
  branch (`G=1/r` / `Gmatrix`) + the FaultStudy input (WP7.9); `ElemKind::Fault` +
  `Circuit.faults`, `check_fault_status`/`reset_faults` control-loop wiring.
- **step 2a — `SwtControl`:** a manual whole-terminal switch; lands the generic
  `set_terminal_closed` + `RefAction::SetSwitchClosed` and the **dirty-edge rule**.
- **step 2b — `Fuse` (`pd/fuse/`):** per-phase TCC; **`TccCurveObj::get_tcc_time`
  ported**, per-conductor `set_conductor_closed`, `MappedStringEnumArray`.
- **step 2c — `Recloser`:** whole-terminal trip + reclose to `Shots`, fast→delayed;
  `PropFlags::ARRAY_MAX_SIZE` + integer-dump `VALUE_OFFSET`.
- **step 2d — `Relay`:** the general control (`Relay.pas`, 9 `Type=` sub-types);
  `Current`/`Voltage`/`ReversePower`/`46`/`47`/`Distance`/`DOC` live, `Generic`/`TD21`
  parse+dump but defer `Sample` to WP7.7 (`NOT_PORTED`). `get_ov_time`/`get_uv_time`,
  `PropFlags::ALLOW_NONE`; event log gated on `ShowEventLog`.
- **step 3 — reliability activation:** enabled Relay/Recloser/Fuse set
  `Flg.HasOCPDevice` (+ `HasAutoOCPDevice` for the auto-reclosers) via a deferred
  `RefAction::SetOcpDevice`; `GetOCPDeviceType` live; the Phase-6 `RelCalc`
  SAIFI/SAIDI/section math runs (no more #52902 abort on a protected zone).
- **step 4 — protection gate + corpus migration:** the targeted golden
  `phase7_protection/*.json` (`gen_phase7_protection.py` + `golden_phase7_protection.rs`,
  **5 trip/reclose scenarios** — event log line-for-line + per-step + final-state vs
  the pinned oracle, reproduced exactly first-run); the **`Open`/`Close` exec verbs**
  ported (`do_open_close_cmd`, reusing the protection switching machinery; 4
  oracle-pinned tests); **`civanlar` + `IEEE_519` (SwtControl) migrated into
  `solvable_now` (35→37)**, `COVERAGE.md` refreshed. *audit-code:* faithful, fixed
  the `set_/get_conductor_closed` guard (`Nphases` → `Nconds`, matching Pascal's
  `Fnconds` — the new `Open` neutral-conductor path) + a unit test. *audit-tests:*
  real oracle-pinned gates; strengthened the golden with element-name-set equality.
  **Tracked-open:** `DG_Prot_Fdr.dss` compiles now but the live system Y diverges
  ~3e-5 rel at a **line** node — a WP7.1 Carson line-constants precision item (not a
  protection/`Open` regression), parked in `needs_investigation`. lib **502 → 514**
  across steps 2d–4.

**WP7.3 (DER A: DynamicExp + InvBasedPCE + PVSystem) — ✅ COMPLETE.** Full per-step
records (decisions, audits, gate detail) archived at
[`docs/phase-records/phase-7-wp3.md`](docs/phase-records/phase-7-wp3.md). In brief:
- **step 0 — `DynamicExp` (`general/dynamic_exp.rs`):** the user-defined diff-eq
  catalog object + its RPN expression interpreter (`InterpretDiffEq` → a flat `cmds`
  array, `SolveEq` stack machine over `[value, derivative]` memory); registered
  before Generator/PVSystem/Storage. Numeric pinning is spec-pinned here (the oracle
  exposes no `cmds`/`SolveEq` outside a dynamics run, WP7.7). Gate: 9 oracle props
  scenarios + 13 interpreter unit tests. lib **514 → 527**.
- **step 1 — `InvBasedPceData` (`pc/inv_based_pce.rs`):** the shared inverter PC base
  (`TInvBasedPCE` + the scalar `TInvDynamicVars`), an abstract base PVSystem/Storage
  embed (flattened like `GenVars`); the three power-flow shared methods
  (`StickCurrInTerminalArray`, `Get_Presentkvar`, `UsingCIMDynamics`). The per-phase
  dynamics arrays + GFM are deferred to WP7.7. Gate: 6 spec-pinned unit tests. lib
  **527 → 534**.
- **step 2 — `PVSystem` (`pc/pvsystem/`) + steps 3–4 (zone + gate):** the power-flow
  PV element on the Generator template with the InvBasedPceData base embedded
  (`ComputePanelPower` → `ComputeInverterPower`'s clamp cascade → `kWOut_Calc`;
  `SetNominalDEROutput`; the two models + the `ForceBalanced` path;
  registers/TakeSample); `ElemKind::PVSystem` + zone admission. Fixed a **real**
  Monitor mode-3 metered-kind bug (PVSystem now classifies as `PcElement`) and a GFM
  silent-degradation (now a loud pre-solve abort, WP7.7). GFM/harmonics/dynamics/
  UserModel/`MakePosSequence` deferred. Gate: `props/pvsystem.json` (10 scenarios) +
  goldens `phase7/pvsystem_{snapshot,curves,clamps}`; **corpus 37 → 44** (7 PVSystem
  cases). lib **534 → 543** (incl. audit follow-ups).

**WP7.4 (DER B: Storage + StorageController) — ✅ COMPLETE.** Full per-step records
archived at [`docs/phase-records/phase-7-wp4.md`](docs/phase-records/phase-7-wp4.md).
In brief:
- **step 1 — the `Storage` element (`pc/storage/`):** `TStorageObj` (the largest PC
  element) on the Generator template with the InvBasedPceData base — the
  charge/idle/discharge state machine (`FState`) + the integrated SOC
  (`kWhStored`/`%stored`, advanced in `EndOfTimeStepCleanup` via `UpdateStorage`, with
  the efficiency-curve `ComputeDCkW`/`QuadSolver` DC solve). Registered
  (`ElemKind::Storage`, `is_zone_pce`, the Monitor mode-3 + GFM-guard mirrors of
  PVSystem). GFM/harmonics/dynamics/UserModel deferred. Gate: `props/storage.json` +
  goldens `phase7/storage_{snapshot,clamps,daily,daily_charge}` (the SOC trajectory
  exact). **Corpus stays 44.** lib **543 → 557**.
- **step 2 — the real `StorageController` (`control/storage_controller/`):** replaces
  the WP6.8 parse-only skeleton with `MakeFleetList`, the `SetFleet*` helpers + fleet
  aggregates, `GetControlPower`/`GetControlCurrent`, and all the `Sample` dispatch
  modes (`DoLoadFollowMode` Peakshave/Follow/Support/I-Peakshave, `DoTimeMode`,
  `DoScheduleMode`, `DoLoadShapeMode`, `DoPeakShaveModeLow`) + `DoPendingAction`/
  `Reset`. The fleet resolves lazily through a `StorageDispatchEnv` (the GenDispatcher
  pattern). One `TODO(compat)` (the `if not FleetState = …` precedence bug) +
  SeasonalRating NOT_PORTED. Gate: goldens `phase7/storagecontroller_{daily,peakshave}`
  + 19 mock `sample_*` tests + 2 exec tests. **Corpus stays 44.** lib **557 → 572**.
- **The YPrim-rebuild fix** (post-audit): a Storage state flip (idle→discharging)
  changes its Norton `Yeq` but the port never propagated `yprim_invalid` to
  `system_y_changed`, so the solve ran a stale idle YPrim against the discharging
  injection (~1.8e-6 drift + an extra iteration). Restored that side effect at the
  StorageController dispatch env and `Storage::inj_currents` (via a new
  `InjCtx.system_y_changed`); the snapshot golden now matches bit-for-bit. **Scope is
  Storage-specific** — PVSystem/InvControl dispatch kvar/kW setpoints, not discrete
  state, so they never invalidate YPrim.

**WP7.5 (DER C: InvControl + ExpControl) — ✅ COMPLETE (steps 1–4).** Full
per-step records (incl. the two real-port-bug write-ups and the Storage
smart-inverter follow-up) archived at
[`docs/phase-records/phase-7-wp5.md`](docs/phase-records/phase-7-wp5.md). In brief:
- **step 1 — `RollAvgWindow` (`control/roll_avg_window.rs`):** the fixed-capacity FIFO
  with O(1) running sums backing InvControl's volt-var/DRC rolling-average voltage;
  ported 1:1 (incl. the faithfully-reproduced asymmetric `accum_sec` drift, whose only
  reader is dead upstream — a plain comment, not `TODO(compat)`). 4 spec-pinned tests.
  lib **572 → 577**.
- **step 2 — `InvControl` (`control/inv_control/`):** the single largest unit in the
  phase (`InvControl.pas`, 3586 lines), ported across sub-steps — **2a** parse-only
  skeleton (34 props + 7 enums + `ValidateXYCurve` + MakeLike), **2b** VOLTVAR, **2c**
  VOLTWATT + VV_VW, **2d** DRC + VV_DRC (+ the `IntervalUnits` time-suffix parse),
  **2e-i** WATTPF + WATTVAR, **2e-ii** AVR, **2e-iii** LPF/RiseFall rate-of-change +
  the explicit-`MonBus` path — on the StorageController clone-out `InvDispatchEnv`
  dispatch pattern (the fleet resolves lazily; the terminal bus is resolved at
  parse-time edit-completion). Storage AVR/WATTPF/WATTVAR ported to working (Storage
  VOLTWATT/VV_VW stay loudly guarded). Gate: `props/invcontrol.json` (20 scenarios) +
  a large `phase7/invcontrol_*` golden family (per-mode, daily, 24h, LPF/RiseFall,
  MonBus, Storage — and the per-step monitor comparison now runs on every multi-step
  phase7 golden) + many mock-env tests; **corpus 44 → 83**. lib **577 → 623**. **Two
  real port bugs found + fixed here** (each a [[dont-rationalize-conditioning]]
  instance): the cross-step `FFlagVWOperates` latch (the missing `UpdateInvControl`
  per-step reset — daily VOLTWATT diverged ~kW) and the missing `LoadsNeedUpdating :=
  TRUE` after `DoPendingAction` (without it AVR's iter-2 read a stale kvar = 0 →
  `DQDV = 0` → never converged).
- **step 3 — `ExpControl` (`control/exp_control/`):** the adaptive-`Vreg` volt-var
  control over a PVSystem-only fleet (`ExpControl.pas`, "adapted and simplified from
  InvControl") — the 14 props + the PVSystemList↔DERList sync, `MakePVSystemList`,
  `Sample`, `DoPendingAction` (slope-at-`Vreg` + `Qbias` → headroom/`PreferQ` curtail
  → `FOpenTau` low-pass → `DeltaQ_Factor` step), and `UpdateExpControl` (the per-step
  `Vreg` slew by `VregTau`). Gate: `props/expcontrol.json` + goldens
  `phase7/expcontrol_{daily,daily_preferq,24h,duty}` (the duty golden is the only
  `FOpenTau` LPF gate — daily runs under `CTRLSTATIC` gate it off) + 16 mock tests.
  One `TODO(compat)` (`FOpenTau := Tresponse/2.3026`, the truncated ln(10)). **Corpus
  stays 83.** lib **623 → 639**.
- **step 4 — the gate / corpus burn-down review (DER C).** No Rust code change (lib
  stays **639**); the gate is green and the DER-C corpus migration is confirmed
  **maximal**. A fresh `DSS_LIVE_CLASSIFY=1` re-probe of **all 48**
  InvControl/ExpControl-tagged `skipped_unsupported` cases (the bulk migrated in steps
  2b–2e; many of the rest still carried *stale* `unsupported_class=InvControl`/`PVSystem`
  tags from before the class landed) found exactly **1 newly-solvable** case —
  `…/PVSystem/CurrentkvarLimite/PV_currentkvarLimit_VV.dss` (a near-ideal-Thevenin
  snapshot: PVSystem + VOLTVAR InvControl regulating kvar from the file-set `kvar=500`
  to the curve zero-crossing at v≈1.0 pu, 56 control iters; full-model live-compared to
  the oracle, **corpus 83 → 84**). **Commit delta: 1 migrated + 39 stale tags
  refreshed** to the genuine current blocker (the other 8 of the 47 still-blocked cases
  already carried a correct `Export,Plot` tag). The **47 still-blocked** cases are
  blocked by Phase-8 / later work, **not** by DER-C numerics — current-state breakdown:
  **42** by a Phase-8 command (Export / Export+Plot — the Daily/MonitoredVoltage
  families), **3** GFM-mode-7 cases (`unsupported_command=BatchEdit; deferred=gfm-WP7.7`,
  also `File=`-blocked), **1** `ExpControl/Master.dss`
  (`unsupported_feature=file-backed-arrays`), and **1** `11_2_kWRatedViolation`
  (`deferred=storage-voltwatt-WP7.5`, the loud Storage-VOLTWATT guard from step 2c).
  `tools/corpus/COVERAGE.md` regenerated (84 → **25.1%** of entry points); the
  `corpus_manifest` bijection holds. *(The stale tags were a documentation-honesty fix
  only — they never affected the gate, which keys on the bijection + the `solvable_now`
  live compare.)*
  - **audit-code follow-up:** verdict **correct** — the full live gate matched all 84
    cases, the migrated VV case is non-trivial (the regulator moved kvar 500 → ~0 over
    56 iters, not an empty pass), the 39 refreshed tags each match the genuine re-probed
    Rust-engine error, and no hidden-migratable DER-C case was left behind (all 40
    InvControl/PVSystem candidates still error; the only solvable one is migrated). Two
    doc-only fixes applied: (1) the **GFM tags** were committed as plain
    `unsupported_command=BatchEdit` (the classify report truncates the reason at 400
    chars, dropping the `mode=7 … (GFM)` clause my appender keyed on), out of sync with
    this record's claim — re-set to `unsupported_command=BatchEdit; deferred=gfm-WP7.7`
    with a hand-transcribed full-blocker note; (2) this record's accounting was
    sharpened to separate the *commit delta* (1 migrated + 39 retagged) from the
    *current-state* family breakdown. **Surfaced-not-fixed (out of DER-C scope, tracked
    for a future corpus-hygiene pass):** ~16 *non*-DER-C `skipped_unsupported` cases
    (line-constants / `Show` / `Open`/`Close`) now compile+solve clean on the Rust
    engine but still carry possibly-stale tags — a clean Rust compile ≠ migratable (the
    gate also needs the oracle full-model match), so these need their own
    `DSS_LIVE_CLASSIFY` re-probe, not a blind migration.
  - **audit-tests follow-up:** verdict **sound + strictly additive** — the new case
    adds real verification (the harness pins the exact 56 control iters Rust↔oracle and
    the full unrelaxed model compare), it is **not flaky** (the gate ran green twice,
    84/84), and nothing was weakened (no tolerance loosened, no case removed/downgraded,
    the depth guard + bijection hold). **Minor (recorded, no fix):** the near-ideal-source
    `currentkvarLimit` *family* is borderline as a class (a sibling under
    `…/NewFeatures/varCapability/` is parked `live_mismatch_near_ideal_source`), but the
    migrated `_VV` variant sits in its stable zone — VOLTVAR drives Q→0 at v=1.0 pu so
    the reactive source current is ~1e-4 A and the ill-conditioned-Y mismatch never
    amplifies past tolerance (the parked siblings force a fixed `kvar=` → ~20 A reactive
    → the mismatch that parks them). No offline golden was added (the live gate is the
    pin); **no fix needed**.
- **next:** WP7.6 (Harmonics) — the first cross-cutting solve mode.

**WP7.6 (Harmonics) — ✅ COMPLETE (steps 1–3).** Full per-step records (decisions,
the two real-port-bug write-ups, the `capture_element` oracle-quirk investigation,
and all six audit follow-ups) archived at
[`docs/phase-records/phase-7-wp6.md`](docs/phase-records/phase-7-wp6.md). In brief:
- **step 1 — current-source family (VSource + Load) + the solve-mode driver:**
  `SolveHarmonic`/`SolveHarmonicT`, `InitializeForHarmonics` + the in-memory
  fundamental save/restore, `Spectrum.SetMultArray`/`GetMult`, the `harmonic =
  frequency/fundamental` fix, the VSource short-at-harmonics branch, and the Load
  `InitHarmonics`/`DoHarmonicMode` + the harmonic YPrim `%SeriesRL` split (the ~40%
  load-admittance bug the golden caught, not the smoke test). lib 639 → 650.
- **step 2 — Thevenin DER family (Generator/PVSystem/Storage):** each a voltage
  source behind its subtransient reactance — `InitHarmonics` (Yeq + the
  `Vthevharm`/`ThetaHarm` capture) + `DoHarmonicMode` (spectrum-scaled,
  phase-rotated injection through YPrim) + the harmonic `CalcYPrimMatrix` Y=Yeq
  branch + the `SetNominalGeneration` harmonic guard; the `guard_unported_harmonic_der`
  removed. The `capture_element` `Powers`-before-`Currents` swap pins the oracle's
  consistent harmonic power past a confirmed upstream stale-`Iterminal` engine bug
  (write-up in `investigations/`, git-ignored). lib 650 → 653.
- **step 3 — monitor harmonic header + the `Set mode=` reset + the corpus
  burn-down:** `ClearMonitorStream` labels the two time columns `Freq`/`Harmonic`
  in harmonics mode (offline-gated — the C-API `Monitors_Get_Header` strips them),
  and the `Set Mode=` handler now runs the full Pascal `Set_Mode` reset tail
  (monitors + meters ahead of faults + controls). Harmonics corpus burn-down is
  **0 migratable** — all 4 decks are Phase-8 (`Export`) / `Isource` /
  FaultStudy-blocked, not harmonics-blocked (2 stale `Swtcontrol` tags refreshed).
  lib 653 → 656; golden_phase7 **60**; `solvable_now` **84**.

**WP7.7 (Dynamics core) — ✅ COMPLETE (steps 1–4).** Full per-step records (decisions,
the real-port-bug write-ups — the `Open`-verb no-op, the `set_ITerminalUpdated`
stamp sweep, the per-step InvControl `FFlagVWOperates` reset — the
dynamics-tolerance reviews, and every audit follow-up) archived at
[`docs/phase-records/phase-7-wp7.md`](docs/phase-records/phase-7-wp7.md). In brief:
- **step 1 — the `SolveDynamic` predictor/corrector driver**
  (`solution/solution/dynamics.rs`): `SolveMode::Dynamic` → `solve_dynamic`, the step
  loop over `DynaVars.h` (`IntegratePCStates` + the pre-`set_mode`
  `calcInitialMachineStates` entry hook), and the Load `GENERALTIME`/`DYNAMICMODE`
  `SetNominalLoad` arm. lib 656 → 659.
- **step 2a — Generator dynamics + Monitor mode 3 + the `Open`-verb fix:** the classic
  (`DynamicEqObj = NIL`) shaft-swing machinery (voltage source behind `Zthev`,
  trapezoidal `Speed`/`Theta`), Monitor mode 3's real sample body, and a real
  `Open class.name` (omitted `term=`) no-op bug fix. Oracle-pinned on Kundur Ex.13.1
  (steady + fault + the full undamped swing). lib 659 → 662.
- **step 2b — PVSystem/Storage GFL inverter dynamics** (`InvDynamics.TInvDynamicVars`):
  the shared per-phase PI current loop + the 22/34-var mode-3 interface + a latent
  Storage SOC-in-dynamics fix; oracle-pinned on 4 PV/Storage decks. lib 662 → 666.
- **step 3a — IndMach012** (`pc/ind_mach012/`): the symmetrical-component induction
  machine (slip-Newton power flow + voltage-behind-`Zsp` dynamics, 22 mode-3 vars).
  Found + fixed the missing `set_ITerminalUpdated` stamp (a stateful extra slip step
  first mis-filed as "conditioning" — a [[dont-rationalize-conditioning]] catch),
  then swept the stamp across all 5 model-contribution sites. lib 666 → 672.
- **step 3b — DynEqPCE integration for the Generator** (`pc/dyneq_pce.rs`
  `DynEqPceData` + the edit-loop `ParseDynVar` fallback): a Generator driven by a user
  `DynamicExp` (`DynamicEq=`/`DynOut=`/inline initializers) instead of its built-in
  shaft model; oracle-pinned on the Kundur DynExp deck, reproducing the classic gate's
  physics exactly. Plus the two-part dynamics-tolerance review (all monitor pins
  tightened to 1e-6; the `dSpeed`/`dTheta` cancellation-floor residuals pinned against
  the oracle's actual value, not ≈0). lib 672 → 676.
- **step 3b cont. — DynEqPCE integration for the inverters (PVSystem + Storage):**
  `InvBasedPceData` now embeds the shared `DynEqPceData` (dropping its bare
  `DynamicEq`/`DynOut` fields), so PVSystem/Storage get the same machinery as the
  Generator (the `DynEqPce` trait, `parse_dyn_var`, `DynOut` resolution, the
  `DynamicEq=` sizing side effect). The per-phase `InitStateVars`/`IntegrateStates`
  `DynamicEqObj <> NIL` branches (`it[i]`/`dit[i]` ↔ `DynOut[0]`; the inverter
  calc-value overrides `2`→`Vgrid[i].mag` / `4`→nothing / `10`→`RatedVDC` /
  `11`→`SolveModulation`+`m[i]`, else `Get_PCE_Value`) + the DynExp
  `NumVariables`/`VariableName`/`GetAllVariables`/`Get_Variable`/`Set_Variable`
  interface. Oracle-pinned on self-contained PV/Storage GFL-DynExp mode-3 decks (the
  corpus `myDiffEq`/`myDiffEq2` filter equations) — steady + the two fault/disturbance
  gates + the PV sample-0 step — matched first-run, no fudging. *audit-code:* faithful
  (no behavioral deviation); fixed 8 stale `NOT_PORTED: DynamicEqObj` docs (4 inverter
  + 4 Generator). *audit-tests:* sound + non-vacuous (mutation-verified: a wrong
  readback slot / skipped `SolveModulation` each fail on both PCEs); added the two fault
  gates + the PV sample-0 pin. lib 676 → 680; `solvable_now` **84** (the corpus
  GFL_IEEE123 DynExp deck is a daily/`Plot`-blocked Phase-8 case — no migration).
- **step 4 — dynamics gate finalize + corpus burn-down (WP7.7 COMPLETE).** The
  focused dynamics gate is **`exec/tests/dynamics.rs`** (the comprehensive
  oracle-pinned mode-3 + fault tests for Generator / PVSystem / Storage / IndMach012
  / the DynExp variants — already the gate for steps 2a–3b cont.); a separate
  `phase7/dynamics*.json` command-replay golden would be redundant (the harness pins
  power-flow/control monitors, not dynamics mode-3 — the exec tests are the stronger
  guard). **Corpus burn-down = 0 migratable** (recon-confirmed): all 20 dynamics-mode
  corpus decks are blocked by a Phase-8 verb or a deferral, **not** by the dynamics
  engine — e.g. `DistanceRelayTest` *converges* on Rust (2 iters) and is blocked only
  by trailing `Plot`; the others by `var`/`@Zbase` (Kundur), `BatchEdit` (GFL_IEEE123),
  `MakeBusList`/`Plot` + `LoadShape action=normalize` (InductionMachine), the GFM mode
  (deferred), the `WindGen` class (Phase 9), or an oracle-side missing data file.
  `solvable_now` stays **84**. **WP7.7 (Dynamics core) is COMPLETE** (steps 1–3b cont.
  + this finalize). The **GFM grid-forming inverter mode** stays deferred (NOT_PORTED
  loud abort across Generator/PVSystem/Storage `DoDynamicMode`).
  - **Tracked-open (Generic/TD21 relay Sample).** A WP7.2 carry-forward: the dynamics
    machinery these need landed in WP7.7, so both are now *portable*, but they stay
    deferred — every Generic/TD21 (and Distance) corpus deck is Phase-8 `Plot`-blocked
    so they can never enter `solvable_now`, and the corpus-backed WP7.8 classes take
    precedence. GenericLogic is ~26 Pascal lines (reads `MonitoredElement.Variable[idx]`
    via the now-live state-var interface); TD21Logic is ~253 lines (a ring-buffer
    time-domain distance relay). The `NOT_PORTED` message text was updated to the
    honest framing (Phase-8 `Plot`-blocked, not "needs WP7.7"). Revisit in a focused
    follow-up or once Phase-8 `Plot` lands a no-op.
- **next:** WP7.8 (VCCS, UPFC + UPFCControl, VSConverter, ESPVLControl) — the
  converter/FACTS dynamics family.

**WP7.8 (Converter/FACTS family) — ✅ COMPLETE.**
- **VSConverter (`pc/vs_converter/`) — done, gate-green.** A 2-terminal AC/DC bridge
  (power-flow only, no dynamics state): the first `phases-Ndc` conductors are AC (a
  voltage source `Vdc·0.353553·m0∠d0` behind `Rac+jXac`, a `YPrim_series` block), the
  last `Ndc` are DC (a power-balance current source `Idc = Pac/|Vdc|` clamped to
  `±IDCMax·kW/kVDC`). 19 props + the `VSCMode` enum (the 5 modes parse/dump but Pascal
  `GetInjCurrents` only ever uses the fixed `m0/d0` — no mode-dependent behavior).
  **Proven upstream oracle bug (NOT reproduced).** `VSConverter.GetCurrents` →
  `GetInjCurrents(ComplexBuffer)` self-aliases `YPrim.MVMult(Curr, ComplexBuffer)`
  (`Curr == ComplexBuffer`) then re-reads the post-mult buffer for the `Pac` estimate,
  so the oracle's *reported* converter currents violate KCL (oracle-probed:
  |I_ac| ≈ 1248 A self-report vs the physical ≈ 390 A; the oracle's *source* current is
  the correct 390 A and ≠ −(self-report)). The port computes the physically-correct,
  KCL-consistent current and deliberately does **not** reproduce the self-report bug
  (cf. WP7.6 "не порти баг эталона"). Gate: `exec/tests/vs_converter.rs` pins the
  oracle's correctly-reported **source** currents + the KCL tie + the DC power-balance +
  the term-2 series mirror (a non-vacuous oracle gate that sidesteps the buggy
  self-report); + `props/vsconverter.json` (3 scenarios). **No corpus migration** — the
  3 VSConverter corpus decks compare the oracle's buggy currents (`vsc0/vsc1test` →
  `skipped_oracle_issue`) or don't converge on either engine (`vsctest`, near-short).
  lib 680 → **681**; `solvable_now` 84. (Fork-drafted; oracle bug + numerics
  independently re-verified in the main loop before commit.)
- **VCCS (`pc/vccs/`) — done, gate-green.** The HW-inverter voltage-controlled
  current source — a full 1:1 port incl. the z-domain ring-buffer filter dynamics.
  Power flow: ideal current source (`YPrim = 0`) injecting `BaseCurr` at the
  terminal-voltage angle (the `BP1`→scale→`BP2` PWL map of the pos-seq voltage,
  XYcurve refs). Dynamics: **both** the time-domain **waveform** ring-buffer path
  (`InitStateVars`/`IntegrateStates`, predictor/corrector via `IterationFlag`) **and**
  the **RMS/PLL** path (`InitPhasorStates`/`IntegratePhasorStates`, `RmsMode=true`) +
  the 6-var mode-3 interface; the ring buffers (`z`/`whist`/`zlast`/`wlast`/`y2`) kept
  1-indexed so the error-prone `MapIdx`/`OffsetIdx` wraparound ports verbatim. 13 props.
  `MakePosSequence` NOT_PORTED (the shared deferral); no oracle bugs found (KCL-clean).
  New `SysCtx.dyna_t` (`DynaVars.t`). **Gate:** `HWtest.dss` (snapshot) migrated to
  `solvable_now` (full-model live oracle match) — `solvable_now` 84 → **85**; +
  `exec/tests/vccs.rs` 3 oracle-pinned mode-3 dynamics tests (HWDyn waveform +
  HWPLL/HWPLL3 RMS, each pinning the t=0.1 s fault transient — the discriminating
  ring-buffer check; **independently re-verified in the main loop** by re-probing
  dss-python 0.15.7 — the HWDyn sample-50 fault transient matched the pins bit-for-bit)
  + `props/vccs.json`. `HWDyn`/`HWPLL`/`HWPLL3` stay `skipped_unsupported` (built-in
  `set mode=dynamic; solve` + `plot`, not snapshot-gateable); `DG_Prot_Fdr` now
  converges (only `plot`-blocked). lib 681 → **687**. (Fresh-agent-drafted; numerics
  re-verified before commit.)
- **UPFC + UPFCControl (`pc/upfc/` + `control/upfc_control/`) — done, gate-green.**
  The unified power-flow controller (PC element, power-flow only — the `Sr0`/`Sr1`
  shift registers are persistent control state, not differential) + its control
  element. UPFC: 17 props, the series `Xs` YPrim block, the 5-mode `GetOutputCurr`
  (dead-band / `VpqMax`-clamp / loss-curve; mode 1 = voltage regulator is what the
  corpus exercises), `GetInputCurr`/`CalcUPFCPowers`/`CalcUPFCLosses`, 14 mode-3 vars.
  UPFCControl: the `UPFCList` + `CheckStatus`→`Sample`→`UploadCurrents` control-sweep
  coupling (a dynamic fleet via a `UpfcDispatchEnv`, the GenDispatcher pattern; lazy
  list resolution). New `exec/view.rs::element_variables` (live f64 `AllVariableValues`
  analogue). **4 proven upstream oracle quirks, each handled** (oracle-probed): (1)
  *not reproduced* — a **2nd UPFC crashes** the oracle (Access Violation —
  `TUPFCObj.Create` casts the first UPFC to `TUPFCControlObj`, UB) → no UPFC MakeLike
  scenario; (2) *not reproduced* — `MakeUPFCList`'s name-list branch is dead+broken
  (clears then reads `FUPFCNameList`); (3) ***reproduced faithfully*** — the loss-curve
  `MakeLike` self-assign no-op (`UPFCLossCurveObj := UPFCLossCurveObj`): the curve is
  **not** copied to a `like=` UPFC, matching upstream (untestable — a 2nd UPFC crashes
  the oracle and `props/upfc.json` carries no `like` scenario; prose-doc, not
  `TODO(compat)`); (4) *not reproduced* — mode-3 monitor records nothing in a snapshot
  (`SampleCount=0`) → the 14 vars are pinned against the live f64 `AllVariableValues`,
  not the empty f32 channel (CLAUDE.md). **Convergence floor proven, not fudged:**
  `Vbin`/`Vbout` are mid-iteration snapshots, so at the default 1e-4 tol the engines
  stop ~4e-5 rel apart in the convergence band — but tightening to **1e-12** collapses
  the gap (both reach the identical fixpoint `Vbin = 236.41620285` to 12 digits in the
  **same 17 iterations**, oracle-probed), the CLAUDE.md proof of a shared fixpoint; the
  gate pins the tight-tol fixpoint. **Gate:** `exec/tests/upfc.rs` (transcribed
  UPFC_test_3 snapshot — `show`/`plot`-blocked so no corpus migration) pins the 14
  mode-3 vars + UPFC currents/powers + the controlled transformers' powers + the
  mode-3 header, all against dss-python 0.15.7 (**independently re-verified in the main
  loop**: 17 iters + all 14 vars + currents + powers matched the pins bit-for-bit) +
  `props/{upfc,upfccontrol}.json`. lib 687 → **693**. (Fresh-agent-drafted; numerics +
  oracle-bug claims re-verified before commit.)
- **ESPVLControl (`control/espvl_control/`) — done, gate-green. WP7.8 COMPLETE.**
  The storage/PV "local controller" — a **faithful no-op on circuit state** (oracle
  proven). The premise that `Sample` redispatches generators is a Pascal misread:
  `MakeLocalControlList` populates from *other ESPVLControl* objects, then `Sample`
  type-confuses each as a `TGeneratorObj` and writes `Gen.kWBase` onto another control's
  non-electrical memory (modeled as an unobservable `phantom_kw_base` field). There is
  no `kWLimit` prop (hardcoded 8000); only a `SystemController` acts; `Sample` never
  pushes a control action. Net: with vs without the control the solution is
  **byte-identical** and `ControlIterations` stays 1 (independently re-verified vs
  dss-python 0.15.7). Ported all 11 props + the lazy `MakeLocalControlList` (Ftype-1
  gate, name-list/scan-all, uniform weights) + `Sample`/`RecalcElementData` (err
  371/372)/`MakeLike`; the dead PVSystem/Storage pointer lists round-trip but never
  dispatch; `MakePosSequence` NOT_PORTED (shared deferral). Not a `TODO(compat)` — the
  type-confusion is dead/harmless upstream code with no golden-pinned value (prose-doc,
  per the convention). **Gate:** `exec/tests/espvl_control.rs` (6 synthetic oracle tests
  incl. the with==without byte-identity) + `props/espvlcontrol.json`. lib 693 → **711**.
  (Fresh-agent-drafted; the no-op + oracle behavior re-verified before commit.)
- **WP7.8 (Converter/FACTS family) COMPLETE** — VSConverter, VCCS, UPFC + UPFCControl,
  ESPVLControl all done, gate-green.
- **Retro audit (WP7.7 step 4 + all of WP7.8, done 2026-06-29).** The mandatory
  `/audit-code` + `/audit-tests` ritual steps (PHASE7_PLAN §3–4) were skipped from
  WP7.7 step 4 (`fb0be61`) through WP7.8 (`c8e23d0`); run retroactively here
  (inline, no agents — full Pascal-vs-Rust read of every element + the targeted
  tests, 31/31 green). **Verdict: faithful ports, no Critical/Major correctness
  bug.** Findings, all settled:
  - **WP7.7 step 4** — message-text + comment only (`relay/mod.rs` `Generic`/`TD21`
    NOT_PORTED loud-abort unchanged); no test files touched. Nothing to fix.
  - **VSConverter** — `GetInjCurrents`/`CalcYPrim`/`GetCurrents` 1:1 (incl. the
    one-iteration `ITerminal` lag, EPSILON, `VscMode` enum). **FIXED:** the loose
    `1e-3 rel` on 5-sig-fig pins in `vs_converter.rs` was a transcription artifact,
    **not** a bug — re-probed dss-python 0.15.7 at full f64 and the Rust source
    currents match the oracle **bit-for-bit (~1e-10 rel)**; the DC-source current
    to ~2e-12. The test now pins the full-precision oracle values at the standard
    **1e-6** current floor (1000× tighter), with the converter's own DC terminal a
    documented tight regression guard (the oracle masks it via the self-alias bug).
    No corpus deck still (the 3 decks stay `skipped_oracle_issue`).
  - **VCCS** — the strongest port: ring-buffer `MapIdx`/`OffsetIdx`, all 3 inj
    regimes + both dynamics paths verified; the local-`z_iu` accumulator proven safe
    (`MapIdx(iu-k+1)`, k≥2, never returns `iu`). Gate strong (`HWtest` live +
    waveform/RMS mode-3 1e-6). *Surfaced:* the `>3-phase` `FrmsMode` branch is an
    untested unreachable edge.
  - **UPFC/UPFCControl** — all 5 modes + `CheckStatus`/`checkPF`/`CalcYPrim`/the
    FPC short-circuit `or` in `Sample` faithful. *Surfaced, deferred (the one
    Major-level gap):* **only mode 1 (voltage regulator) is oracle-gated**; modes
    0/2/3/4/5 and the PF-compensation/`MonElm` path (`get_input_curr` 2/3/5) are
    ported loop-for-loop but behaviorally unverified (no corpus deck; would need
    synthetic probe decks). *Minor:* `calc_upfc_losses` silently returns `1.0` when
    no loss curve (Pascal NIL-crashes) — benign fallback, no corpus path. The
    `MakeLike` self-assign wording above (quirk 3) corrected this pass.
  - **ESPVLControl** — type-confusion no-op modeled correctly; tests are exemplary
    (redispatch fires both PDiff signs, cross-object + named-subordinate phantom
    writes, and `control_present_equals_control_absent` byte-identity — the no-op is
    *proven*, not rationalized). *Surfaced:* no-op gated only for snapshot solves
    (no multi-step corpus deck).
  - **Fixes applied:** (1) the VSConverter test tightened to full-precision oracle
    pins at 1e-6 (above — the only loose oracle tolerance in the range, empirically
    proven a no-bug); (2) the UPFC quirk-3 STATUS wording reconciled. **Genuinely
    still open (not "settled" — untested coverage):** the UPFC modes 0/2/3/4/5 +
    PF-compensation path (only mode 1 oracle-gated) — closing it needs synthetic
    probe decks. The remaining items (VCCS `>3-phase` RMS edge; ESPVL multi-step)
    are unreachable/zero-corpus edges justified by PHASE7_PLAN §2.6.

**WP7.9 (FaultStudy + AutoAdd + Feeder) — ✅ COMPLETE.**
- **step 1 — FaultStudy mode (`solution/solution/fault_study.rs`) — done, gate-green.**
  Ported `TSolutionAlgs.SolveFaultStudy` and its `TSolutionObj` helpers
  (`DisableAllFaults` → `SolveDirect` for the open-circuit Voc → `AllocateAllSCParms`
  → `UpdateVBus` → `ComputeAllYsc` → `ComputeIsc`). Each bus's `Zsc` is built column
  by column by injecting 1 A at each node and re-solving the **already-factored**
  system Y (`SparseSet::solve` reuses the cached LU), i.e. each `Zsc` column is a
  column of `Y⁻¹` restricted to the bus's nodes; `Ysc = Zsc⁻¹` (reusing
  `support/cmatrix` `Invert`, whose singular path matches Pascal — degenerate buses
  e.g. a delta-isolated zero sequence leave `Ysc` partially transformed, exactly like
  upstream); `Isc = Ysc·VBus`. New `Bus` fields `zsc`/`ysc` (`Option<CMatrix>`) +
  `allocate_bus_quantities`/`get_zsc1`/`get_zsc0`; new `exec/view.rs::bus_short_circuit`
  (dss-python `Bus.Zsc1`/`Zsc0`/`Isc`). `SolveFaultStudy` sets `LoadModel=ADMITTANCE`
  (faithful; no corpus FaultStudy deck has active loads). `MonteFault` still errors
  (no corpus case). **Gate:** `exec/tests/fault_study.rs` — a self-contained radial
  feeder whose bus `Zsc1`/`Zsc0` are the analytic series sums (e.g. b2 = source
  0.5+2.0j + line 0.2+0.6j = 0.7+2.6j), pinned to dss-python 0.15.7 along with the
  full-complex `Isc`. **Corpus 85 → 88:** the 3 `ShortCircuitCases` decks
  (`ieee37_SC_Currents`, `ieee34Mod2_SC_Case_II`, `IEEE123Master-SC`) classify
  **solvable** (full-model live oracle match — the post-study `NodeV` is the last
  `ComputeYsc` column on both engines and agrees). lib 711 → **712**.
  - **audit-code:** verdict **clean** (no blocker/major) — the port is loop-for-loop
    faithful (order, `ComputeYsc` indices + ground convention, single-LU factor reuse,
    bit-faithful singular invert, dynamics-entry timing all re-verified vs the oracle).
    Confirmed `LoadModel=ADMITTANCE` is **faithful but inert for `Zsc`**: Pascal
    `TLoadObj.CalcYPrim` runs identical code in the POWERFLOW/ADMITTANCE branches, so
    the load YPrim (already in Y from the snapshot) is LoadModel-independent. Fixed a
    `BusScView` doc nit (`vbus` is the stored `VBus`/Voc, **not** dss-python
    `Bus.Voltages`, which returns the live residual `NodeV`).
  - **audit-tests:** verdict **sound + non-vacuous** (every pin independently
    re-derived from dss-python 0.15.7). Both audits flagged that the **corpus
    migration validates the full power-flow model + FaultStudy mode behavior (node
    order, residual `NodeV`, Y, element I/P) but NOT the per-bus `Zsc`/`Ysc`/`Isc`
    deliverable** — that is the targeted test's job, and the Phase-8 `Export/Show
    FaultStudy` reports (WP8.3/8.4) will systematically gate the formatted Zsc/Isc on
    the corpus (a dedicated corpus Zsc capture now would duplicate that and need a
    bespoke tolerance for the near-singular delta-isolated `Zsc0`). **Fix applied:**
    extended `exec/tests/fault_study.rs` with a second oracle-pinned deck covering the
    paths the balanced anchor can't — a **single-phase** bus (the `n=1`
    `avg_off_diagonal`=0 branch + 1×1 invert), an **asymmetric** bus behind a
    full-matrix line (non-circulant `Ysc` → distinct per-node `Isc`, exercising
    `Ysc·VBus` + the `Zsc[j,i]` indexing), and a **delta-isolated** bus (near-singular
    `Zsc0`≈5.2e7j — the deliberately-ignored `invert()` failure path; pinned as the
    robust facts: well-conditioned `Zsc1` tight + `Zsc0` blows up + fault `Isc`
    matches). lib 712 → **713**.
- **step 2 — AutoAdd / MonteCarlo / LoadDuration / MonteFault — kept deferred (no
  port).** Corpus probe found **zero** decks using `mode=autoadd`/`A`, `mode=M1/M2/M3`
  (MonteCarlo), `mode=MF` (MonteFault), or `mode=LD1/LD2` (LoadDuration). Per the
  PHASE7_PLAN §2.6 empirical rule ("port only if a corpus case needs it"), each keeps
  the Pascal `Unknown solution mode.` error (the `dispatch.rs` catch-all, stale
  "Phase 5" suffix replaced with an honest "no corpus case" note + a comment naming
  the deferred modes). The `circuit/auto_add.rs` option skeleton (options round-trip)
  is unchanged. No code beyond the message/comment.
- **step 3 — Feeder — documented dead (no port).** Corpus probe found **zero**
  `New Feeder.` instantiations; `Feeder.pas` is largely dead upstream (Phase 6 found
  `DoFeederStuff` remnants dead). Nothing to port. **WP7.9 COMPLETE** (FaultStudy is
  the only real deliverable; AutoAdd/Feeder are empirical no-ops per the plan).

**WP7.10 (Phase 7 exit) — ✅ COMPLETE (docs/verification only, no code change).**
- **Marker sweep clean:** no `TODO`/`NOT_PORTED` orphan points at WP7.9/7.10. The
  remaining deferrals all have a documented home — **GFM** grid-forming mode (the one
  Phase-7-planned item descoped, tracked-open, NOT_PORTED loud abort) and the
  **Generic/TD21** relay `Sample` (tracked-open) — both **Plot-blocked with zero
  corpus payoff**; DLLs/UserModel = "never"; `MakePosSequence` = on-demand;
  AutoAdd/Monte/LD/Feeder = no corpus case (WP7.9); 39 `TODO(compat)` = the deliberate
  upstream-inexactness set wiped in the dedicated post-acceptance §6 pass (PORTING_PLAN
  §6), not now. 3 residual `TODO(WP7.7)` are unreachable-edge-case hardening notes
  (>3-phase dynamics, Model=6 UserModel generator).
- **Gate:** full three-command gate + the always-on live corpus gate (88 cases) green;
  `tests/corpus/COVERAGE.md` refreshed (solvable_now 85 → **88**, 26.3% of entry
  points; the WP7.9 burn-down = the 3 ShortCircuitCases decks).
- **No code/test audit:** WP7.10 changed only `STATUS.md` (docs/verification), so there
  is nothing for `/audit-code`/`/audit-tests` to review.
- **Phase 7 COMPLETE.** **next = Phase 8** (`PHASE8_PLAN.md` drafted). **Merge to
  `main` (`--no-ff`, per-phase convention) is the HARD STOP — explicit user request
  only; not done.**

**Phase-7 carry-forward (cross-cutting, beyond WP7.2):**
- **Dirty-edge discipline (all four controls + the `Open`/`Close` verbs).** Every
  trip/close/reset/Open forces conductors via `Closed[]` →
  `TDSSCktElement.Set_ConductorClosed` (`CktElement.pas:287`) sets `YPrimInvalid :=
  TRUE` → `SystemYChanged := TRUE` (`:240`) **unconditionally**, no
  change-comparison. So each raises `system_y_changed` **unconditionally** (or via
  an exact per-conductor check), **never** gated on a
  `terminal_all_phases_closed`/`is_closed` aggregate (a partial-open terminal
  otherwise slips a real change past the rebuild → stale Y); each control ships a
  partial-open fail-on-regression test (`d0addb4`/`d1f48231`), and the `Open` verb
  carries the Line/transformer round-trip guards.
- **Reliability (step 3).** OCP flags + `GetOCPDeviceType` + the live `RelCalc`
  SAIFI/SAIDI are in. The single-int `ocp_device_type` + single-flag model is exact
  for the realistic one-OCP-per-element case; the move/re-enable reassignment edge
  (a control redefined onto a different element, leaving the old element's flag
  stale) is **not** un-set — consistent with the existing controlled-element force
  model (the `SetSwitchClosed`/`SetConductorsClosed`/`Open` forces likewise never
  un-force a previous target). Not exercised by any gate.
- **Generic/TD21 Relay Sample logic deferred to WP7.7** (dynamics): the relay
  parses + dumps `Type=Generic`/`TD21` but the live sensing records a `NOT_PORTED`
  error. The corpus Distance/TD21 relay demos also need the dynamics solve mode.

---

## 1f. Phase 8 record (`PHASE8_PLAN.md`) — 🚧 IN PROGRESS

Execution plan: **`PHASE8_PLAN.md`** (WP8.1–WP8.8, the reporting/output + full
executive layer). Per-step cadence = the full ritual in `PHASE8_PLAN.md §0` (gate
→ STATUS + commit → `/audit-code` + `/audit-tests` as independent agents → fix +
commit → STATUS sync → stop). Phase 8 is almost entirely *read-and-format*: no
new electrical math, no new solve mode — the risk is faithful report layout and
**not silently faking output**.

**WP8.1 (Report infrastructure) — ✅ COMPLETE, gate-green (sub-steps 1+2).**
- **sub-step 1 — dispatch skeleton + GUI no-ops — done, gate-green (`82b50fe`).**
  The new top-level **`crate::report`** module (`report/mod.rs`): the
  `EXPORT_OPTIONS` (57, ADIAKOPTICS-off → `High=Laplacian`; confirmed: the pinned
  build defines `DSS_CAPI_ADIAKOPTICS_DISABLED`, `common-release.cfg:6`) and
  `SHOW_OPTIONS` (34) name tables in exact `TExportOption`/`TShowOption` ordinal
  order, each built into a `CommandList` (abbreviation-matched like the oracle's
  `ExportCommands`/`ShowCommands`). The routers live in **`exec/report.rs`**
  (`impl Dss`, like every other command router — they drive the private
  parser/circuit/error state and delegate formatting to `crate::report`):
  `do_export_cmd` (keyword match → unknown-keyword #24713 → scoped `NOT_PORTED`
  per keyword), `do_show_cmd` (keyword match → faithful **silent** no-op),
  `do_save_cmd`/`do_dump_cmd` (scoped `NOT_PORTED`). `command.rs` routes
  `Export`/`Save`/`Dump`/`Show` and the `Plot`/`Visualize` **headless no-op** in
  the **post-circuit** dispatch. No real report formatting yet (WP8.2–8.5).
  - **The Show-silent vs Export/Save/Dump-loud asymmetry is forced + correct.**
    The always-on live gate (`corpus_live.rs`) asserts `errors().is_empty()`, and
    **44 `solvable_now` decks** run `Show Power`/`Show Voltage`/`Show f` (all
    valid `TShowOption` keywords). So the unported-`Show` path is a **silent**
    no-op — faithful per §2.5 (a `Show` changes no electrical state; WP8.4 lands
    the real formatters + the 24700/24701/24702/999 errors + a targeted text
    golden, which is what proves it non-fake). `Export`/`Save`/`Dump` decks are
    all in `skipped_unsupported` (never in the live gate), so they **loudly**
    record a scoped `NOT_PORTED` — a half-ported `Export` never silently emits
    nothing (the plan's §WP8.1-step-1 intent). Audit-code swept the **whole**
    corpus (incl. redirected sub-files): the 21 distinct `Show` keywords are all
    valid — no `panel`, no unknown — so deferring those errors to WP8.4 is proven
    safe.
  - **audit-code (independent agent): one MAJOR, fixed.** `Plot`/`Visualize` were
    first placed in the *pre-circuit* no-op arm, which swallowed the oracle's #301
    "create a circuit first" guard — the oracle's dispatch gate errors #301 for
    `plot`/`visualize`/`show` *before* a circuit exists (oracle-probed:
    dss-python 0.15.7). **Fixed** by moving `Plot`/`Visualize`/`Show` to the
    **post-circuit** dispatch, so before a circuit they fall through to the
    generic #301 guard (matching the oracle) and after a circuit they are clean
    no-ops; the `report_verbs_before_circuit_error_301` test now pins the #301.
    Verified-correct: all option names/ordinals, the `cmd` ordinals
    (SAVE=7/PLOT=12/DUMP=16/EXPORT=34/VISUALIZE=73), `do_export_cmd`'s #24713
    message + the 1-based `ParamPointer` convention, and the module split.
  - **audit-tests (independent agent): sound + non-vacuous** (mutation-verified —
    forcing `do_show_cmd` to error fails the silent-no-op test; forcing
    `do_export_cmd` to no-op fails the loud-stub test). Strengthened: pin the
    *quoted* `"Voltages"` (not a substring that 4 siblings share), assert the
    plot snapshot is non-empty, pin the full #24713 message.
  - **Tracked-deferred (not bugs):** the `Export` circuit/solution gates
    (#24711/#24712) land with the real exporters in WP8.2; the `Show`
    panel/unknown/solution errors (999/24700/24701/24702) + real formatters in
    WP8.4; `Visualize` on an *unsolved* circuit errors #24722 on the oracle —
    not reproduced (no corpus deck reaches it; Visualize is a §2.5 no-op).
    Out-of-scope (pre-existing): `EXEC_COMMANDS` is missing `Abort`(124)/`Clone`
    (125) from the pinned PM build (no effect on any current ordinal/abbreviation;
    future corpus-hygiene pass). lib stays **713**; `solvable_now` **88**.
- **sub-step 2 — output-path machinery + `Export Counts` end-to-end + the CSV/text
  golden harness — done, gate-green (`929145c`).** Lands the first **real report**
  through the whole output path:
  - `report/output.rs` — `export_path` (Pascal `DoExportCmd`: explicit filename wins
    verbatim, else `<OutputDirectory><CircuitName_><default>` where `CircuitName_ =
    <CaseName>_`). `report/export/counts.rs` — `ExportCounts` (Pascal
    `ExportResults.pas:2965`): the `Format: DSS Class Name = Instance Count` text
    dump of every class + instance count, in registration order.
  - **Output-dir state on `Dss`:** `output_directory` (Pascal `OutputDirectory`,
    only `Set DataPath=` moves it — not `Compile`/`Redirect`, which move only
    `current_dir`) + `last_result_file` (`SetLastResultFile`, exposed via
    `Dss::last_result_file()` so the golden harness reads the produced file).
    **`Set DataPath=` wired** (`apply_data_path`, Pascal `SetDataPath`: create-dir +
    #907, both with and without a circuit — a common top-of-script pattern; the
    non-writable→scratch fallback is NOT_PORTED). `do_export_cmd` Counts(26) →
    write the file + `@lastexportfile`.
  - **The new golden harness `compare_export`** (PHASE8_PLAN §2.3, in `tests/harness`):
    tokenizes both files by the report separator, matches a fixed header block
    verbatim, then compares each data row field-by-field — numbers within tolerance
    (reusing `assert_value_matches_tol`), identifiers case-insensitively. Two row
    policies: `ExactOrdered` (the contract for most exports) and `RustSubsetByKey`
    (Counts: the Rust class registry is a **proven proper subset** of the oracle's,
    so every ported class's count is pinned exactly while the not-yet-registered
    oracle classes are ignored — documented in `tests/TOLERANCE_NOTES.md`, **not** a
    blanket relaxation; tightens to `ExactOrdered` once the registry is complete).
  - **Gate:** `gen_phase8.py` captures the oracle's `Export Counts` →
    `tests/golden/phase8/export_counts.{txt,meta.json}` (the meta carries the deck so
    the Rust + oracle fixtures can't drift); `golden_phase8.rs` replays the deck,
    exports to a process-unique temp dir, and subset-compares vs the oracle (the
    default-item counts `TCC_Curve=10`/`Spectrum=7` + the fixture `Line=2`/`Load=1`
    pinned). Self-test green. lib stays **713** (713→**714** with the new
    `counts.rs` unit test); golden_phase8 **1**; `solvable_now` 88.
  - **Micro-deviations from the plan (documented):** (1) the `csv` crate is **not**
    added yet — `Counts` is `=`-separated text, not CSV; `csv` lands in WP8.2 with the
    first real CSV export (avoids an unused dep). (2) `report/format.rs` is **not**
    created yet — `Counts` needs no float formatting; `format.rs` lands in WP8.2 when
    the first numeric export needs it. (3) the test uses a process-unique
    `std::env::temp_dir()` subdir, **not** the `tempfile` dev-dep (dependency-free;
    `tempfile` can be adopted later if isolation needs grow).
  - **audit-tests follow-up (one MAJOR, fixed).** `RowPolicy::RustSubsetByKey`
    iterated only the Rust rows, so a *dropped* class / empty report body passed
    silently (mutation-proven: header-only + missing-`Line`/`Load` bodies both
    passed) — a "report bug that hides" (PHASE8_PLAN §1). **Fixed:** the policy now
    takes a `require` key set (the deck-created + default-item classes —
    line/load/vsource/tcc_curve/spectrum/loadshape/growthshape) asserted present in
    the Rust output. The auditor confirmed the baseline is a genuine pinned-oracle
    capture (not self-generated), the deck is single-sourced via `meta.json`, and a
    wrong count / extra Rust class / bad header all fail correctly. Minor: the
    "tightens to `ExactOrdered`" claim also needs the Rust registration order
    reconciled to the oracle `DSSClassList` (they differ) — TOLERANCE_NOTES updated.
  - **audit-code follow-up (no Critical/Major; 2 Minors fixed).** Faithful port of
    `ExportCounts`/the path resolution/`SetDataPath`/`SetLastResultFile` confirmed
    (incl. `objects.len() == ElementList.Count`, the `casename_` filename prefix,
    no-circuit `DataPath` allowance, write-failure surfaced not swallowed). **Fixed:**
    (1) `apply_data_path` used `create_dir_all` (creates missing parents) vs Pascal's
    single-level `CreateDir` (#907 if a parent is missing) → switched to `create_dir`;
    (2) `write_report` baked in the **Export-specific** `@lastexportfile` — moved to
    the export router so reusing the generic writer for Show/Save (WP8.4/8.5) won't
    wrongly set it (Show sets neither; Save sets `@lastfile` + `GlobalResult`).
    Surfaced-not-fixed (latent, documented): the trailing-filename read is hoisted
    before the per-report pre-parse — a `TODO(WP8.2)` now warns that reports
    8/9/15/17/… must move their `Parm2` parse ahead of it. Nits (no fix, no corpus
    path): relative explicit filename resolves vs process cwd; no-circuit Counts is
    graceful where the oracle AVs; the scratch/empty-`DataPath` omissions are
    NOT_PORTED.
**WP8.2 (Export: solution outputs) — 🚧 IN PROGRESS.**
- **sub-step 1 — bus/node solution exports — done, gate-green (`71067f7`).**
  The first **real solution reports**, read-only over the solved circuit
  (PHASE8_PLAN §2.1, plan-step 1):
  - **`report/format.rs`** — the shared Pascal number formatters: `g(v, sig)`
    (`Format('%…g')`, delegating to the existing `util::fmt_g`) and `fixed(v,
    decimals)` (`Format('%…f')`). The width/justification prefixes (`%10.6g`,
    `%-13.11g`) only space-pad, which the comparator trims, so they're dropped.
  - **Four exports** in `report/export/` (one file each, loop-faithful to
    `ExportResults.pas`): `export_voltages` (`ExportVoltages:277` — per-bus
    node mag/angle/pu, ascending-node-number scan via `FindIdx`, zero-filled to
    MaxNumNodes), `export_bus_coords` (`ExportBusCoords:3064` — `CheckForBlanks`
    + `%-13.11g`, no header), `export_node_names` (`ExportNodeNames:3699` —
    `BusName.NodeNum`, original-case → lowercase since the HashList is lowercase;
    gate compares identifiers case-insensitively), `export_ynode_list`
    (`ExportYNodeList:3784` — node names in Y-order via `MapNodeToBus`, quoted
    uppercase).
  - **`Export` solution guard (#24712)** wired into `do_export_cmd`
    (`ExportOptions.pas:163-177`): the solution-reading exports (`ptr` ∈
    `1..24, 28..32, 35, 46..51`) need `Solution.NodeV` allocated (`node_v.len() >
    1`), else "The circuit must be solved before you can do this." The **no-circuit**
    half (#24711) is **unreachable** — `ProcessCommand`'s generic pre-circuit guard
    (#301, `command.rs`) fires first (Export is not in the OK-before-circuit list,
    `ExecCommands.pas:364`; the message matches the oracle's #301 verbatim). The
    router's `write_export` helper resolves the path + writes + sets the
    export-specific `@lastexportfile` (shared by Counts).
  - **Harness `ColTol`** (per-column tolerance, `tests/harness`): `ExportPolicy`
    gains `col_tol`, matched against the header column names. Needed because the
    `Angle%d` columns are `%6.1f` (one decimal) — two independent solves round
    that last 0.1 digit independently (a ±0.1 *formatting* floor), while every
    `%g` magnitude/pu column keeps the tight default. **Not** a blanket
    relaxation; documented in `tests/TOLERANCE_NOTES.md`. The voltage golden is a
    **report-layout** gate (header/columns/order/scaling), not the physics gate —
    voltages stay pinned to 1e-8 by the live `corpus_live` model compare.
  - **Gate:** `gen_phase8.py` compiles + solves the unmodified IEEE13 master
    (`tests/corpus`, single-sourced via each report's `meta.json`) and captures
    the oracle's four reports → `tests/golden/phase8/export_{voltages,buscoords,
    nodenames,ynodelist}.{txt,meta.json}`; `golden_phase8.rs` replays the same
    master + `solve`, exports into a scratch `datapath`, and diffs via
    `compare_export` (`ExactOrdered`). golden_phase8 **1→5**; lib unit-test count
    unchanged (the formatters are gated end-to-end, no new inline tests). The WP8.1
    `export_records_scoped_not_ported` unit test updated: `Voltages` is now ported,
    so it pins the #24712 guard + a still-unported `elem`→ElemCurrents NOT_PORTED.
  - **Micro-deviation (documented):** the `csv` crate is still **not** added — all
    four reports are bespoke layouts (`ExportVoltages`'s zero-fill / `%g` columns,
    the headerless coord/node lists) that `Writeln`-style `String` building
    expresses directly; `csv` lands if/when a plain RFC-4180 table export makes it
    pay (re-evaluate at the element exports). `report/format.rs` now exists.
  - **audit-code (independent agent): faithful — no Critical/Major/Minor, 2 doc
    nits fixed.** Empirically reconfirmed against the live oracle: unsolved
    `export voltages` → #24712 / no-circuit → #301 (so #24711 IS unreachable); the
    ptr-set `1..=24|28..=32|35|46..=51` reproduces the per-keyword solve
    requirement (buscoords/ynodelist need a solve, nodenames doesn't); the default
    filenames + `<case>_` prefix are byte-exact; a {2,3}-node bus emits the trailing
    zero-fill group; NodeNames is lowercase+trailing-space byte-identical. **Fixed:**
    the `node_names.rs` doc (the HashList lowercases on store, so the oracle *also*
    emits lowercase — byte-identical, not merely case-insensitively equal) and the
    `export_with` doc (circuit presence comes from the #301 dispatch gate, which
    also covers ptr 39/NodeNames that skips the solution guard).
  - **audit-tests (independent agent): genuine + non-vacuous — all 6 mutations +
    the guard caught.** Mutation-verified FAILs: dropped zero-fill, Angle/pu swap
    (the loose angle tol does **not** rescue it), kV-vs-V scaling, reordered buses,
    empty/header-only body, dropped node loop; and disabling the #24712 guard fails
    the unit test. Confirmed the goldens are real pinned-oracle captures (`check_pin`
    0.15.7/0.14.5), single-sourced via `meta.json`, and that `corpus_live` gates
    IEEE13 voltages at 1e-8 so the export golden is legitimately a layout gate.
    **Fixed (Minor 1):** the `Angle` override was `rel 1e-3`/`abs 0.11` — but the
    `%6.1f` floor is purely *additive*, and the angle is `arg(V)` (independent of
    |V|/pu, so the "pinned by magnitude+pu" rationale was wrong). Tightened to **rel
    0 / abs 0.11** (the exact proven printing floor) with the rationale corrected in
    `golden_phase8.rs` + the `ColTol` doc + `TOLERANCE_NOTES.md`. **Nit 2:** the
    "primary gate is corpus_live" wording now states precisely that corpus_live
    gates the *engine* voltage physics (daily mode) while the export golden gates
    the *snapshot report-layer transform*. **Nit 3:** the unit test now sets a
    scratch `datapath` (defensive — every branch errors before a write, but a future
    write-reaching branch must not pollute the source tree).
- **sub-step 2a — the aggregate PD/PC power exports — done, gate-green** (`668bd18` +
  audit follow-ups `1fda0a6`/`91a091c`). The first
  **element** exports (read/compute over the solved circuit, PHASE8_PLAN §2.1 plan-step
  2): `Powers` (`ExportPowers:1075` — per-terminal kW/kvar of every PD then PC element
  + each PD's terminal-1 normal/emergency excess kVA), `Losses` (`ExportLosses:1185` —
  per-PD total/load/no-load losses in W/var via `GetLosses`), `P_byphase`
  (`ExportPbyphase:1224` — per-conductor kW/kvar over the full `Yorder`, **no**
  positive-seq `×3`, formed directly from `ComputeVterminal`/`ComputeIterminal`).
  - **The mutable element-walk infrastructure** (the defining new piece): the element
    exports call the mutating terminal getters (`Power`/`GetLosses`/`ComputeIterminal`/
    `ComputeVterminal`), so they can't use the read-only `fn(&Circuit)` formatter shape.
    New `report::export::for_each_enabled_elem(&mut [DssClass], &[ElemRef], f)` walks a
    circuit element list under the `if Enabled` guard, and `Dss::export_with_mut` hands
    the formatter the disjoint `(&mut classes, &circuit, &sys, &node_v)` borrow (the
    `snapshot_elements` pattern). `exec::registry` is now `pub(crate) mod` so the report
    formatters can name `DssClass`. The element formatters are `pub(crate)` (they expose
    the `pub(crate)` `DssClass`).
  - **The `Powers`/`P_byphase` MVA/kVA `Parm2` pre-parse** (`ExportOptions.pas:190-199`):
    `do_export_cmd` now consumes the `m…`→MVA flag for ptr 9/19 **before** the trailing
    filename, fixing the WP8.1 `TODO(WP8.2)` hoist (the other Parm2-consuming reports
    8/15/17/20-21/32/51 land in later WPs). The dispatch arms 9/19/24 route through
    `export_with_mut`.
  - **`ElemPowers` deferred to sub-step 2c** (the `WriteElem*` family). An oracle probe
    found its Vsource power is an **intrinsic `WriteElemPowers` artifact**: `Export
    ElemPowers` writes the Vsource `−612.936` even in complete isolation, ≠ the oracle's
    own `CktElement.Powers` (`−612.729`, which `corpus_live` already pins and the Rust
    matches) — `WriteElemPowers`'s `ComputeVterminal; ComputeIterminal` sequence
    re-derives the source current differently from the canonical terminal power.
    `ElemCurrents` (probe-confirmed) does **not** have the quirk (it matches canonical),
    so the three `Elem*` reports go together in 2c where the source-element semantics can
    be settled (port-the-bug vs canonical, the WP7.6 / VSConverter precedent).
  - **Gate:** `gen_phase8.py` captures the oracle's `Powers`/`Losses`/`P_byphase` on
    solved IEEE13 → `tests/golden/phase8/export_{powers,losses,p_byphase}.{txt,meta.json}`;
    `golden_phase8.rs` replays + diffs via `compare_export` (`ExactOrdered`). All real
    columns (kW/kvar/W) — no angle/sequence — so the floors are the plain printing floors
    (`Powers` `%11.1f` → `rel 0`/`abs 0.11`; `P_byphase` `%10.3f` → `abs 0.0011`; `Losses`
    `%.7g` → the default `%g` floor), documented in `tests/TOLERANCE_NOTES.md`, **not** a
    physics relaxation (the engine V/I/P is pinned to 1e-8 by `corpus_live`). golden_phase8
    **5→8**; lib stays **719** (formatters gated end-to-end, no new inline tests);
    `solvable_now` **88** (no migration — the `Export` decks need the full export set; the
    migration lands at the WP8.2 completion gate).
  - **audit-code follow-up (independent agent): faithful — no Critical/Major, 1 nit fixed,
    1 coverage gap → audit-tests.** Verified field-by-field vs Pascal: the headers (incl. the
    double spaces), the PD-then-PC iteration order, the excess-kVA terminal-1 gating, the
    MVA/kVA `Parm2` pre-parse (incl. the Pascal non-`m`-token-swallowed-as-Parm2 quirk), and
    the highest-risk **`Power[j]`-`×3` vs `P_byphase`-no-`×3`** distinction — all correct. The
    Vsource omission from the Powers PC section is probe-confirmed correct (`ElemKind::Source`
    files into `sources`, never `pc_elements`), and the `ElemPowers` deferral is probe-confirmed
    real. **Fixed (nit):** the formatters uppercased the whole `Class.Name`; Pascal uppercases
    only the element name (`DSSClassName.UPPER(Name)`) — added `format::upper_elem_name` (split
    on the first `.`, uppercase only the suffix) so the raw output is byte-faithful, not merely
    case-insensitively equal. The MVA-path coverage gap is handled in the audit-tests follow-up.
  - **Coverage note (audit-corrected):** `excess_kva_norm` **is** value-exercised — IEEE13's
    XFM1 + lines 650632/632670/670671 carry above-`NormAmps` current, so the `Powers`
    `P_Normal`/`Q_Normal` columns are non-zero (the overload `factor > 0` branch is gated). Only
    the *emergency* twin `excess_kva_emerg` is all-`0.0` (no IEEE13 line exceeds `EmergAmps`);
    it is byte-identical logic to the gated norm branch (different rating field), so it is
    tracked-minor — `Export Overloads`/`Capacity` (WP8.3) value-exercise the emergency rating.
  - **audit-tests follow-up (independent agent): sound + non-vacuous, 2 Majors + 1 Minor
    fixed.** Confirmed the goldens are genuine pinned-oracle captures (`check_pin` 0.15.7/0.14.5,
    `meta.json` single-sources the deck, Rust diffs *its own* produced file), `ExactOrdered`
    enforces row count + per-row field count + PD-then-PC order, and Powers `rel=0/abs=0.11`
    is the correct tight floor (mutation-verified the gate is non-vacuous). **Fixed:**
    (1) **[Major]** `P_byphase` had a superfluous `rel=1e-4` band that mutation-provably masked
    a ~0.005% per-conductor scale drift — the `%10.3f` floor is purely additive (empirical max
    divergence exactly 1e-3, one ULP), so tightened to `rel=0`/`abs=0.0011` (the Powers twin's
    discipline). (2) **[Major]** the MVA `opt=1` path (the `m…` `Parm2` flag → MW/Mvar headers +
    the extra `×0.001`) was wired this WP but untested — **added** `export_powers_mva` +
    `export_p_byphase_mva` goldens (`export powers mva` / `p_byphase mva` on solved IEEE13),
    backstopping the scale + header (a missing `×0.001` prints kW ~1000× larger and fails loudly).
    (3) **[Minor]** `Losses` reused the 6-sig Voltages `EXPORT_REL=1e-4`, ~1000× looser than its
    `%.7g` floor; I **measured** the actual divergence (max 1.53e-7 rel on a substantial loss =
    the 7-sig 1-ULP floor; the only large-rel cells are near-zero noise ≤5.6e-9 W absorbed by
    `abs`) and tightened to a dedicated `LOSSES_REL=1e-6` (≈6× over the proven floor), correcting
    the `TOLERANCE_NOTES.md` rationale (no cancellation floor materializes on IEEE13; if a metered
    feeder later shows one it must be proven by decomposition, not by widening `rel`). golden_phase8
    **8→10**. *audit-tests* also confirmed a second feeder adds only engine-physics variety (already
    gated by `corpus_live`), not report-layout coverage, so IEEE13-only is adequate for §2.3 here.
- **sub-step 2b — the symmetrical-component family — done, gate-green** (`5eb351a`). The
  three sequence exports on solved IEEE13: `SeqVoltages` (`ExportSeqVoltages:177`,
  bus read-only — `Phase2SymComp` over named nodes 1/2/3 + `PctNemaUnbalance` over the
  line-to-line voltages), `SeqCurrents` (`ExportSeqCurrents:431` + `CalcAndWriteSeqCurrents:352`
  — element walk Sources→PD→PC→Faults; the PD pass alone writes the `%Normal`/`%Emergency`
  rating columns on terminal 1), `SeqPowers` (`ExportSeqPowers:1311` — PD then PC, sequence
  powers `S = V012·conj(I012)` printed `S.re·0.003` = per-seq VA→3φ kW, PD rows carry the
  4 excess-kVA columns on terminal 1). Files `report/export/seq_{voltages,currents,powers}.rs`;
  dispatch arms ptr 2/4/10 (`SeqPowers` ptr 10 never pre-parses the MVA flag — `ExportOptions.pas:191`
  traps only 9/19 — so `opt = 0` always; the MVA path is ported-but-unreached). The existing
  `mathutil::pct_nema_unbalance` + `SymComp::default()` (`phase_to_sym`) are reused.
  - **`TODO(compat)` (`seq_currents.rs`):** `Iresidual` sums the *terminal-1* conductors for
    **every** terminal row (Pascal `CalcAndWriteSeqCurrents` indexes `cBuffer^[i]`, not
    `cBuffer^[(j-1)*Ncond+i]`) — an upstream quirk reproduced verbatim; clean fix = per-terminal
    slice.
  - **New harness machinery — `ColTol::gate` (denominator gate).** `SeqCurrents`' ratio columns
    (`%I2/I1`/`%I0/I1`/`%NEMA`) divide by `I1`, which at an open/unloaded terminal is a near-zero
    cancellation residual (~1e-12 A): there I1/I2/I0 are noise (pinned to 0 by the magnitudes'
    `abs`) so the ratio is a faer-vs-KLU **noise/noise** form (`Line.671680.2`: oracle `%I2/I1=147.7`
    vs Rust `61.8`; the live f64 confirms both engines compute I1≈I2≈2e-12 there, terminal 1 matches
    to 6 sig, Iresidual matches exactly — proven cancellation floor, not a port bug). `ColTol::gate`
    skips a ratio cell only where the oracle's denominator is `0 < |I1| < thresh` (the **band-limit**
    lower bound keeps the 32 *exactly*-zero rows' `0==0` checks — Pascal prints those ratios as 0).
    Net on IEEE13: **1** genuine-noise row skipped, **54** rows' ratios still checked; magnitude
    columns checked on every row. A proven cancellation floor (decomposition), **not** a relaxation —
    `tests/TOLERANCE_NOTES.md`.
  - **Gate:** `gen_phase8.py` captures the oracle's `SeqVoltages`/`SeqCurrents`/`SeqPowers` →
    `tests/golden/phase8/export_seq{voltages,currents,powers}.{txt,meta.json}`; `golden_phase8.rs`
    replays + diffs (`ExactOrdered`). SeqVoltages/SeqCurrents = 6-sig magnitudes (`EXPORT_REL=1e-4`,
    `abs=1e-9`/`1e-8` = the **measured** volt/amp residual floor) + 4-sig ratio columns (`%`-prefix
    `ColTol`, `rel=1e-3`); SeqPowers = the `Powers` additive floor (`rel=0`, `abs=0.11`). golden_phase8
    **10→13**; lib stays **719** (formatters gated end-to-end). `solvable_now` **88** (no
    migration — the `Export` decks need the full export set; migration at the WP8.2 completion
    gate).
  - **audit-code follow-up (independent agent): faithful — no Critical/Major/Minor, no fixes.**
    Verified field-by-field vs Pascal: header byte-exactness (incl. double spaces / `p.u.,Base kV`),
    the Sources→PD→PC→Faults vs PD→PC iteration orders, `k=(j-1)*ncond+i` indexing, the zero/pos/neg
    `v012[0]/[1]/[2]`↔`V012[1]/[2]/[3]` mapping, `S.re·0.003` (+ excess no-`0.003`, PD-term-1-only,
    12-vs-8 fields), the ratings `>0` guards, the NEMA LL-vs-phase input split, and the `Iresidual`
    `TODO(compat)`. Settled **[Question]** SymComp `precise()` vs official → confirmed correct
    (oracle default is `precise()`; "official" only under the off-by-default `BadPrecision` compat
    flag). **[Nit]** `norm_amps`/`emerg_amps` fetched before the `do_ratings` guard — harmless (no
    side effects, consumed only under the guard). Tracked-untested-but-faithful: the Faults walk
    (no Fault objects in IEEE13) + the `<3`-node/positive-sequence else-branches.
  - **audit-tests follow-up (independent agent): sound + non-vacuous, 2 Minors fixed + 1 tracked.**
    Mutation-confirmed the gate catches a real per-row ratio scale error, a magnitude error on a
    gated row, and a `%Normal` error, while ignoring genuine noise. **Fixed:** (1) **[Minor]** the
    gate `|I1|<1e-6` over-skipped the 32 *exactly*-zero rows (a planted nonzero `%NEMA` there passed
    silently — mutation-proven); **band-limited** to `0 < |I1| < 1e-6` so those rows' `0==0` ratios
    stay checked (only the 1 genuine-noise row is now skipped). (2) **[Minor]** the `SeqCurrents`
    `abs=1e-3` magnitude floor was ~6 orders too loose (looser than the smallest real printed
    `I1=5.8e-4 A`); **measured** the actual per-column residual (max abs 1e-11 V / 1e-9 A) and
    tightened to `abs=1e-9` (SeqVoltages) / `1e-8` (SeqCurrents) — proven floor, not a guess.
    **(3) tracked [Minor]:** the `SeqPowers` MVA path is unreachable by dispatch (no golden possible);
    the Faults walk + 2-phase nonzero-`%NEMA` path need a synthesized fixture (deferred — no corpus
    deck). golden_phase8 stays **13** (tighter floors, no new tests).
- **sub-step 2c — the per-terminal/per-conductor element exports — done, gate-green.** The six
  remaining WP8.2 element/tap reports on solved IEEE13: `Currents` (`ExportCurrents:612` +
  `CalcAndWriteCurrents:518` — per-terminal/-conductor `|I|`/angle over the widest element `MaxCond×
  MaxTerm`, zero-filled, + a per-terminal `Iresid`; walk Sources→PD→Faults→PC via `GetCurrents`),
  `NodeOrder` (`ExportNodeOrder:750` + `WriteNodeList:716` — `"Elem", Nterms, Nconds, node#…` via
  `GetNodeNum` = `MapNodeToBus[NodeRef].NodeNum`), `ElemCurrents`/`ElemVoltages`/`ElemPowers`
  (`WriteElem*:799/881/962` — per-conductor `|I|`/`|V|`/kW·kvar over `ComputeIterminal`/
  `ComputeVterminal`), and `Taps` (`ExportTaps:3729` — per-RegControl controlled-transformer
  present/min/max tap + increment + `TapPosition` + winding + reverse/cogen mode). Files
  `report/export/{currents,node_order,elem,taps}.rs`; dispatch arms ptr 3/40/41/42/43/44. New router
  helpers `export_elem_ordered` (the `WriteElem*` per-element `IsSolved` #222001 guard: header-only +
  the error once when unsolved) and `export_with_classes` (read-only `(&[DssClass], &Circuit)` for
  `Taps`, which downcasts RegControl + Transformer). New RegControl accessors `tr_winding`/
  `in_reverse_mode`/`in_cogen_mode` (the runtime mode flags, distinct from the `cogen=` property).
  - **The `ElemPowers` Vsource order fix (a real divergence caught, not rationalized).** Our first
    cut reproduced Pascal's textual `ComputeVterminal; ComputeIterminal` order and the golden failed:
    Rust `Q_1 = -612.936` vs oracle `-612.729`. Root cause proven: Pascal's post-solve
    `ComputeIterminal` is a no-op (`ITerminalUpdated = TRUE`) so `Vterminal` stays `NodeV`, but our
    `compute_iterminal` re-runs `GetCurrents`, and `TVsourceObj.GetCurrents` **overwrites** `Vterminal`
    with the source EMF `[Vsource; 0]` (≈ but ≠ `NodeV` — the isolated-source `-612.936` 2a surfaced).
    Fix: call `compute_iterminal` **before** `compute_vterminal` so `Vterminal = NodeV` at the power
    product — reproducing the oracle's observable `NodeV·conj(I)` (`-612.729`). Inert for every other
    element (their `GetCurrents` also sets `Vterminal = NodeV`). Not a `TODO(compat)`; documented at
    `elem.rs` + `tests/TOLERANCE_NOTES.md`.
  - **New harness machinery — `ColSel` (name-prefix | index-parity) for `ColTol`.** The `ElemCurrents`/
    `ElemVoltages` headers are **truncated** (`…, I_1, Ang_1, ...`) — they name only the first mag/angle
    pair — so the existing header-name-prefix `ColTol` can't reach the later angle columns. Refactored
    `ColTol.prefix: String` → `sel: ColSel` (`Prefix(..)` | `Parity{start, parity}`) and `gate:
    Option<(usize,f64)>` → `Option<GateSpec>` (`Col(col,thresh)` | `PrevCol(thresh)`). The angle columns
    use `Parity{start, 1}` (every other column from `start`) with `rel=0`/`abs=0.011` (the `%8.2f`
    additive floor), gated on their **paired magnitude** (`PrevCol` = the preceding column) so the angle
    of a near-zero residual/open-terminal/grounded-neutral conductor (faer-vs-KLU noise) is skipped where
    `0 < |mag| < 1e-6` — the band-limit keeps exactly-zero rows' `0.00==0.00` checks. `Currents` (full
    header, all pairs) uses `Parity{1,1}`; the `SeqCurrents`/`SeqVoltages`/`Voltages` `Prefix` overrides
    are unchanged (mechanically ported to the new enum). A proven cancellation floor, not a relaxation.
  - **Gate:** `gen_phase8.py` captures the oracle's six reports on solved IEEE13 →
    `export_{currents,nodeorder,elemcurrents,elemvoltages,elempowers,taps}.{txt,meta.json}`;
    `golden_phase8.rs` replays + diffs (`ExactOrdered`). golden_phase8 **13→19**; lib **722** (formatters
    gated end-to-end, no new inline tests — the WP8.1 `export_records_scoped_not_ported` case 2 switched
    from the now-ported `elem` to the still-unported `summary`). `solvable_now` **88** (no migration — the
    full WP8.2 export set + the completion gate lands next).
  - **Tracked-untested-but-faithful:** the `WriteElem*`/`WriteNodeList` unsolved-circuit #222001 path
    (header-only + one error; Pascal emits it once per element — the observable file + error presence
    match, no gate checks the count) and the `Currents` Faults walk (no Fault objects in IEEE13) are
    code-faithful but unexercised (no unsolved/Fault corpus/golden deck).
  - **audit-code (independent agent): faithful — no findings.** Loop-for-loop reconfirmed vs Pascal:
    all six formatters (headers, `%10.6g`/`%8.2f`/`%8.5f` formats, name casing/quoting), the `Currents`
    `MaxCond=1`/`MaxTerm=2` width grown over *all* `ckt_elements` (verified `add_ckt_element` pushes
    regardless of `enabled`), the `GetNodeNum` ground branch, the `winding_tap_data` destructure order +
    `TapPosition` FPC-Round, the runtime `In{Reverse,Cogen}Mode` flags, and the ptr/filename map — all
    correct. The `ElemPowers` order inversion verified **load-bearing** (the export hits a
    `compute_iterminal` cache miss, so `get_currents` re-runs and clobbers `vterminal` to the EMF; the
    trailing `compute_vterminal` restores `NodeV`). The 222001-once-vs-per-element deviation is
    doc-acknowledged (observable file + error presence match), not a finding. Nothing to fix.
  - **audit-tests follow-up (independent agent): genuine + strong, 1 LOW fixed.** Mutation-verified all
    6 goldens non-vacuous — the `-612.729→-612.936` pre-fix regression fails loudly, magnitude/angle/
    node/tap mutations each fail, and the `PrevCol` angle gate skips **only** genuine faer-vs-KLU noise
    (a real 5.7e-4 A angle stays checked, a 6.2e-11 residual angle is skipped; `Parity` targets the
    angle columns, never masking a magnitude — a 0.005 `ElemCurrents` magnitude drift fails). Floors
    confirmed proven, meta single-sources the deck, goldens are genuine `check_pin` oracle captures.
    **Fixed [LOW]:** switching the unit test's case 2 to `summary` dropped the `elem`→`ElemCurrents`
    *earliest-wins* abbreviation coverage; **re-added** as case 4 (`export elem` on a solved circuit
    emits `…_EXP_ElemCurrents.csv`, no error) — an assertion inside the existing test, so lib stays **722**.
- **side fix — VSConverter reporting made side-effect-free (WP7.8 follow-up).** Re-verifying the
  WP7.8 oracle-bug claim (user request) both **confirmed it beyond doubt** and refined it: the Pascal
  `GetCurrents` → `GetInjCurrents(ComplexBuffer)` self-aliased `MVMult` was reproduced **bit-exact**
  (~1 ulp) by simulating the aliased row-wise product from the converged voltages; the symptom is
  **backend-`mvmult`-size-dependent** — `Yorder=8` (default 4-phase): product returns all zeros →
  reported AC currents = plain `Yprim·V` (the famous 1248 A vs physical 390 A) + DC falls into the
  `Pac==0 → 1000·kW` default, reads stable-but-wrong; `Yorder=4` (the `vsc0/vsc1test` decks): reads
  are **non-idempotent** (×|Y|² growth per read: 1.1e5→5.1e7→1.8e10 A) and one `Currents` read
  between two solves **poisons the next solve** via the corrupted `ComplexBuffer` tail (vsc0test:
  re-solve diverges, 8.15 kV → 950 kV). Full upstream bug report: `tmp/vsconverter_bug_report.md`
  (for dss-extensions/dss_capi). Also re-confirmed `skipped_oracle_issue` is the right class: both
  decks match the oracle on voltages/iterations/all other elements; only the converter's own
  reported rows differ. **Fixed in the port:** `get_currents` had an accidental extra deviation —
  it overwrote `cd.inj_current` (Pascal's reporting writes only the `ComplexBuffer` scratch,
  leaving the solver's `InjCurrent` lag state intact), so a mid-run currents read would have
  shifted the next step's `Pac` lag vs the oracle (a future cross-step state-leak, the InvControl
  lesson). `compute_inj_currents` now **returns** the vector; the solve path stores it, the
  reporting path subtracts a local — observation no longer perturbs solver state. Values
  unchanged (probe-verified on both decks + gate; lib stays 722).
  **Follow-up sweep (same class, all elements):** audited every `get_currents`/reporting path for
  observation-perturbs-state deviations vs Pascal. Machine family (Generator/Load/PVSystem/
  Storage/IndMach012) — **faithful, no change**: Pascal's own `GetTerminalCurrents` recomputes
  `InjCurrent` on read behind the `IterminalSolutionCount` guard, and the port replicates guard
  and all. UPFC `Vbin`/`Vbout` refresh + VCCS `Vterminal`/`sV1` refresh on read are Pascal's own
  side effects — kept. PD elements/default trait impl — pure. `get_all_variables` — read-only
  everywhere. **Fixed (latent, same shape as VSConverter): VSource, VCCS, UPFC** — their
  `get_currents` overwrote `cd.inj_current` where Pascal writes the `ComplexBuffer` scratch; all
  three injections are pure functions of state/voltages (no lag), so the overwrite was
  value-identical today and only latent — normalized anyway to the `compute_inj_currents()`
  return-a-vector pattern (solve path stores, reporting subtracts a local). UPFC's SR0/SR1
  registers advance only in `upload_currents` (UPFCControl-clocked), never on read — verified
  both engines. ISource/GIC not yet ported — port them with this pattern from the start.
- **next — WP8.2 sub-step 3 + completion gate:** the matrix/summary exports (`Y`/`Yprims`/`SeqZ`/
  `Summary`/`Result`; `Counts` already done), then the WP8.2 completion gate — the IEEE8500
  bus/summary goldens + the `Export`-tagged solution-report corpus migration + `COVERAGE.md` refresh.

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

Grep `rg "TODO\(compat\)"` for the full marker list (39 sites). Notable:
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
(the frequency-scaled Y + the <0.51 Hz branch are **exercised by WP7.6
harmonics**; the GIC *elements* stay Phase 9); RegControl/CapControl
`Sample`/`DoPendingAction` **wired into the
control loop (WP5.7)**; RegControl/ControlQueue debug-trace files (flag
stored, no file — port with Monitors, Phase 6+); `MakePosSequence` everywhere
(Phase 6+); `BusCoords` **ported (WP5.8)**; Monitors/EnergyMeters
`sample_all`/`EndOfTimeStepCleanup` are no-op hook stubs at the SolveDaily/
Yearly/Duty call sites (Phase 6); the **dynamics** (WP7.7), **harmonics/harmonicT**
(WP7.6) and **faultstudy** (WP7.9) solve modes are **ported**; the Newton algorithm
and the Monte-Carlo/load-duration/AutoAdd/`SolveGeneralTime` solve modes keep the
"Unknown solution mode" error (no corpus case — WP7.9 empirical decision);
the report verbs are **routed (Phase 8)**: `Export Counts` (WP8.1) and the
bus/node solution exports `Voltages`/`BusCoords`/`NodeNames`/`YNodeList` (WP8.2
sub-step 1) are **real**, the remaining `Export` keywords + `Save`/`Dump` record a
scoped `NOT_PORTED` (real formatters in WP8.2–8.5), `Show` is a faithful silent
no-op (real `ShowResults` in WP8.4), `Plot`/`Visualize` are headless no-ops
(§2.5); `Select`/... remaining executive verbs still record "not ported"
(Phase 8 WP8.6).

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
  `StorageController` behavior — ✅ **all done**: `PVSystem` (WP7.3), `Storage` +
  `StorageController` (WP7.4), and `InvControl` + `ExpControl` (WP7.5) (the WP6.8
  StorageController parse-only skeleton is replaced by the real fleet dispatch;
  `is_zone_pce` now admits Storage/PVSystem).
- **Protection** `Relay`/`Recloser`/`Fuse`/`SwtControl`/`Fault` — ✅ **done
  (WP7.2)**: all five classes ported on the control sweep, the `Open`/`Close` exec
  verbs landed, and an enabled Relay/Recloser/Fuse sets `Flg.HasOCPDevice` so
  `RelCalc` no longer aborts (#52902) and `GetOCPDeviceType` is live — the
  SAIFI/SAIDI/section math runs on a protected zone.
- **Line constants** `WireData/CNData/TSData/CableData/LineSpacing/LineGeometry`
  + Carson — ✅ **done (WP7.1)**: Line's
  `geometry`/`spacing`/`wires`/`cncables`/`tscables` resolve and drive the Carson
  Z/Yc (one plural-cable reset + the `DG_Prot_Fdr` ~3e-5 line-Y precision case
  tracked-open, §1e).
- **Harmonics** (`DoHarmonicMode` for VSource/Load + Generator/PVSystem/Storage,
  the frequency sweep + the harmonic monitor header) — ✅ **done (WP7.6)**.
- **Dynamics** (Generator/Storage `DoDynamicMode`, state vars beyond names/count)
  + `MakePosSequence` everywhere; Monitor modes 3/4/7/8/10/12 build their header
  but defer the sample body; Transformer GIC (<0.51 Hz) elements — WP7.7+ / Phase 9.
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
