# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-28 — **Phase 7 IN PROGRESS** (branch
`phase-7-extended-elements`). Completed work packages this phase: **WP7.1 (line
constants & geometry), WP7.2 (protection), WP7.3 (DER A: DynamicExp +
InvBasedPceData + PVSystem), WP7.4 (DER B: Storage + StorageController), WP7.5
(DER C: InvControl + ExpControl), and WP7.6 (Harmonics) COMPLETE**; **WP7.7
(Dynamics core) IN PROGRESS — step 1 (the `SolveDynamic` predictor/corrector
driver) done.** WP7.6 ran in three steps. Step 1 landed the harmonics solve mode
for the
current-source family (VSource + Load): `Spectrum.SetMultArray`/`GetMult`, the
`harmonic = frequency/fundamental` fix, `Set/Get Harmonics=` + `DoAllHarmonics`, the
`SolveHarmonic`/`SolveHarmonicT` drivers (`CollectAllFrequencies`/`AddFrequency` +
the in-memory `savePresentVoltages`/`RetrieveSavedVoltages`), `InitializeForHarmonics`
on the `Set mode=harmonics` entry, the VSource harmonic `GetVterminalForSource`
branch and the Load `InitHarmonics`/`DoHarmonicMode`. **Step 2 landed the Thevenin
DER family (Generator/PVSystem/Storage `InitHarmonics`/`DoHarmonicMode` + the
harmonic `CalcYPrimMatrix` Y=Yeq branch + the `SetNominalGeneration` harmonic guard):
each is a voltage source behind its subtransient reactance (Generator Xd"; PV/Storage
%R/%X), injecting the spectrum-scaled, phase-rotated Thevenin voltage through YPrim.
The `guard_unported_harmonic_der` loud abort is removed.** **Step 3 landed the
monitor harmonic header (`ClearMonitorStream` labels the two time columns
`Freq`/`Harmonic` in harmonics mode) and the `Set mode=` monitor/meter reset (the
Pascal `Set_Mode` tail), and confirmed the harmonics corpus burn-down is maximal:
0 migratable — all 4 corpus harmonics decks are Phase-8 (`Export`/`Show`) /
`Isource` / FaultStudy-blocked, not harmonics-blocked (2 stale `Swtcontrol` tags
refreshed).** **WP7.7 step 1** wired `SolveMode::Dynamic` → `solve_dynamic`
(`solution/solution/dynamics.rs`): the predictor/corrector step loop over
`DynaVars.h` (`IterationFlag` 0/1), `IntegratePCStates`, and the
`calcInitialMachineStates` dynamics-entry hook on the `Set mode=dynamic` handler
(mirroring the harmonics entry). The per-element state machinery
(`InitStateVars`/`IntegrateStates`/dynamics injection) is the no-op `CktElement`
trait default for now — Generator/Storage/PVSystem fill it in step 2,
IndMach012/DynEqPCE in step 3. **next = WP7.7 step 2 (Generator/Storage/PVSystem
dynamics state vars + Monitor mode 3).**

Per-WP and per-step detail (decisions, audits, gate descriptions, the
real-port-bug write-ups) lives in **§1e** (one-line-per-step summaries) and the
archives under `docs/phase-records/`:
[`phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md),
[`phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md),
[`phase-7-wp3.md`](docs/phase-records/phase-7-wp3.md),
[`phase-7-wp4.md`](docs/phase-records/phase-7-wp4.md),
[`phase-7-wp5.md`](docs/phase-records/phase-7-wp5.md),
[`phase-7-wp6.md`](docs/phase-records/phase-7-wp6.md).
Current scores: dss-core **lib 659**, **`solvable_now` 84** (the live corpus gate;
the harmonics corpus family is Phase-8/`Isource`/FaultStudy-blocked — 0 migratable,
WP7.6 step 3); oracle pinned to dss-python 0.15.7 (backend = dss_capi 0.14.5,
`tools/golden/PIN.txt`).

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
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | 🚧 in progress — `PHASE7_PLAN.md` (WP7.1–WP7.10); branch `phase-7-extended-elements`; **WP7.1–WP7.5 done (all DER + protection + line constants); WP7.6 (Harmonics) COMPLETE; WP7.7 (Dynamics core) IN PROGRESS — step 1 (the `SolveDynamic` predictor/corrector driver) done**; **next = WP7.7 step 2 (Generator/Storage/PVSystem dynamics state vars)**. Per-step detail in §1e + `docs/phase-records/phase-7-wp{1..6}.md` |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 659, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_phase6 1, golden_phase7 1,
                            # golden_phase7_protection 1,
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

**WP7.7 (Dynamics core) — 🚧 IN PROGRESS.**
- **step 1 — the `SolveDynamic` driver (`solution/solution/dynamics.rs`):** wired
  `SolveMode::Dynamic` → `solve_dynamic`, the predictor/corrector step loop over
  `DynaVars.h` (per step: `IncrementTime` → `DefaultHourMult` → predictor
  [`IterationFlag = NewTimeStep`, `IntegratePCStates`, `SolveSnap`] → corrector
  [`SameTimeStep`, …] → `MonitorClass.SampleAll` → `EndOfTimeStepCleanup`), with
  `SolutionInitialized` forced true so the inner power flow does not re-init per
  step. Added `IntegratePCStates` (`SolutionAlgs.pas` l.321 — the full PCElements
  list, no `Enabled` test) and `calcInitialMachineStates` (`Solution.pas` l.2156 —
  the dynamics-entry machine-state init, enabled-gated), wired into the
  `Set mode=dynamic` handler and fired only on a *fresh* entry into a dynamics mode
  from a solved circuit (`was_dynamic` capture). Critically it runs **before**
  `set_mode` commits `is_dynamic_model`/`h`/`mode` — Pascal's `OK_for_Dynamics`
  timing — so each machine's `InitStateVars`/`ComputeIterminal` captures its
  operating point from the power-flow state, not the dynamic Norton branch (the
  harmonics entry stays post-commit; only dynamics needs the pre-commit order).
  New no-op-default `CktElement` trait hooks `init_state_vars`/`integrate_states`
  (base `TPCElement` does nothing); `Solution.iteration_flag` set by the driver
  (surfaced to `SysCtx` in step 2 when a machine consumes it). Ported the Load
  `GENERALTIME`/`DYNAMICMODE` `SetNominalLoad` case (growth × load-multiplier,
  `ShapeFactor` 1+j1 under the `USENONE` `ActiveLoadShapeClass` default — the
  established Generator/Storage/PVSystem assumption; the old `_ =>` arm wrongly
  claimed Dynamic was unreachable + dropped the load-multiplier). Gate: 3 driver
  integration tests (`exec/tests/dynamics.rs`) — a static-circuit dynamics solve
  holds the snapshot fixpoint to 1 ppm across 5 steps + one monitor sample/step, a
  `loadmult=2` solve drops the loadbus voltage (proves the re-solve + covers the
  new multiplier arm), and the `OK_for_Dynamics` unsolved-entry guard. The focused
  oracle gate over a real dynamics machine is step 4. lib **656 → 659**;
  `solvable_now` **84**.
  - **audit-code follow-up:** verdict — no step-1 bug; the driver is a faithful 1:1
    `SolveDynamic` port (predictor/corrector order, `IterationFlag`, the
    `IntegratePCStates`-all vs `calcInitialMachineStates`-enabled-gated distinction,
    the Load `DYNAMICMODE` arm all confirmed against Pascal). Acted on three
    forward-risk items it surfaced: **(1)** the entry hook was moved **ahead of**
    `set_mode`'s mode commit (above) — it was post-commit, which would have made a
    step-2 Generator `InitStateVars` capture the dynamic-branch current instead of
    the power-flow one; **(2)** leaving dynamics mode does not yet
    `InvalidateAllPCELEMENTS` — marked `NOT_PORTED(WP7.7 step 2)` in `set_mode`
    (inert until a machine presents a mode-dependent Norton YPrim); **(3)**
    `preserve_node_voltages` is set but `build_y_matrix` does not yet honour
    `UpdateVBus`/`RestoreNodeVfromVbus` — marked `NOT_PORTED(WP7.7)` at the build
    site (pre-existing since WP7.6-harmonics; inert with no mid-step Y rebuild).
  - **audit-tests follow-up:** verdict — genuine guards, not smoke (loop-bound /
    clock / sample-count / unsolved-guard all real). Closed the one gap it found:
    the 1-ppm voltage-hold check is tautological for a static fixture (a no-solve
    driver passes it; only `is_solved` caught that), and the new Load `DYNAMICMODE`
    multiplier arm was exercised only at loadmult 1 — added
    `dynamic_mode_load_multiplier_moves_operating_point` (loadmult=2 → loadbus
    voltage must drop), which makes the re-solve observable and gives the arm its
    first non-trivial coverage. lib 658 → **659**.
- **next:** step 2 — Generator/Storage/PVSystem `InitStateVars`/`IntegrateStates`/
  the dynamics current injection (`DoDynamicMode`/`CalcVthev_Dyn`) + Monitor mode 3.

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
(the frequency-scaled Y + the <0.51 Hz branch are **exercised by WP7.6
harmonics**; the GIC *elements* stay Phase 9); RegControl/CapControl
`Sample`/`DoPendingAction` **wired into the
control loop (WP5.7)**; RegControl/ControlQueue debug-trace files (flag
stored, no file — port with Monitors, Phase 6+); `MakePosSequence` everywhere
(Phase 6+); `BusCoords` **ported (WP5.8)**; Monitors/EnergyMeters
`sample_all`/`EndOfTimeStepCleanup` are no-op hook stubs at the SolveDaily/
Yearly/Duty call sites (Phase 6); Newton algorithm, dynamics/faultstudy/
Monte-Carlo/load-duration/`SolveGeneralTime` solve modes (Phase 7; the
**harmonics**/`harmonicT` modes are **ported, WP7.6**);
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
