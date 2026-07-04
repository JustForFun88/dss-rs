# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-07-04 — **Phase 8 IN PROGRESS** (`PHASE8_PLAN.md` —
reporting/exports/Save; branch **`phase-8-reporting`**, branched from the
gate-green Phase-7 tip). Full Phase-8 detail is in **§1f**; the current frontier:

- **WP8.1 COMPLETE** (dispatch skeleton + output-path machinery + `Export Counts`
  + the `compare_export` golden harness).
- **WP8.2 COMPLETE** — the solution/power/symmetrical-component/per-terminal/matrix
  export families (`Voltages`…`Currents`…`Yprims`/`Y`/`SeqZ`/`Summary`/`Result`) +
  the mutable element-walk infra; the completion gate migrated the `Export`-unblocked
  corpus (`solvable_now` **88→119**, COVERAGE **26.3%→35.5%**) + landed the Rust
  `CorpusGuard`. The "9 decks hang" and "live-gate flake" tracked-opens are both
  **RESOLVED** (§1f Issue-1/Issue-2).
- **WP8.3 COMPLETE (steps 1–5), gate-green; audits pending** — landed: `Export
  Monitors` (step 1); the `Meters`/`Generators`/`Loads`/`PVSystem_Meters`/
  `Storage_Meters` register/load dumps + the DER `SampleAll`/`ResetAll` wiring the
  export surfaced as a gap (step 2); `EventLog`/`ErrorLog` + the missing circuit-build
  `LogThisEvent` markers (step 3a); `Faultstudy` read-only per-bus 3φ/1φ/L-L currents
  over the WP7.9 `Ysc`/`BusCurrent` (step 3b); `BusReliability`/`BranchReliability`/
  `Capacity` over the `RelCalc`-populated fields + a synthesized recloser+meter deck
  (step 3c part 1); `Overloads`/`Unserved`/`AllocationFactors` — PD-overload rows with
  the byte-faithful `NormAmps<=0` column shift, the mutable `EEN`/`UE` Load criteria,
  `DumpAllocationFactors` (step 3c part 2; the audit follow-up caught + fixed the
  column-shift empty-field defect); `Sections`/`Profile` — the meter
  `SectionCount`/`FeederSections` persistence, the `meter=`/phases pre-parses,
  `Set/Get Markercode|Nodewidth` + `Circuit.NodeMarker*`, and the multi-meter
  `Bus_Int_Duration` OOB guard the two-meter fixture exposed (step 3c part 3);
  the `TSystemMeter` register core + the full demand-interval machinery — the
  per-meter/system/totals `DI_*` writers, the phase-voltage report (+ the
  `TakeSample` pu-voltage accumulators), the overload/voltage-exception reports,
  the `Set DemandInterval/DIVerbose/Overloadreport/Voltexceptionreport/
  SampleEnergyMeters=` handlers, the `Set year=` `Set_Year` side effects,
  `CloseDI`, and the solve-loop open/close wiring (step 4); the completion gate —
  a Rust probe over `skipped_unsupported` found 49 now-clean decks, the live
  classify migrated **47** + (after fixing the **silent Spectrum `CSVFile`
  no-op** the two IEEE_519 harmonicT decks exposed — a real Phase-2 leftover,
  now ported via the `FileLoad` path) **2 more** (step 5).
  golden_phase8 **54**; lib **730**; `solvable_now` **119→168** (COVERAGE
  **35.5%→50.1%**).
- **next = the deferred WP8.3 audits** (steps 3c p3 + 4 + 5 in one pass,
  user-authorized), then WP8.4 (Show reports).

**Phase 7 COMPLETE** (WP7.1–WP7.10, branch `phase-7-extended-elements`,
gate-green) but **NOT merged to `main`** (per-phase merge = explicit-request-only
HARD STOP; `phase-8-reporting` builds on top of it). Roll-up + archives in **§1e**
([`docs/phase-records/phase-7.md`](docs/phase-records/phase-7.md) +
`phase-7-wp{1..7}.md`). Tracked-open Phase-7 deferrals (both Plot-blocked, zero
corpus payoff): the **GFM grid-forming inverter mode** and the **Generic/TD21
relay `Sample`**. Current scores: dss-core **lib 730**, **`solvable_now` 168**;
oracle pinned to dss-python 0.15.7 (backend = dss_capi 0.14.5,
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
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | ✅ **COMPLETE** (WP7.1–WP7.10) — `PHASE7_PLAN.md`; branch `phase-7-extended-elements`, gate-green, **NOT merged to `main`** (explicit-request-only HARD STOP). WP7.1–7.6 (line constants, protection, DER, harmonics), WP7.7 (Dynamics core), WP7.8 (Converter/FACTS), WP7.9 (FaultStudy + AutoAdd/Feeder-deferred), WP7.10 (phase exit). Tracked-open deferrals: GFM grid-forming mode + Generic/TD21 relay `Sample` (both Plot-blocked, 0 corpus payoff). Per-step detail in §1e + `docs/phase-records/phase-7-wp{1..6}.md` |
| **8** | **Reporting: Export/Show/Save/Dump + executive tail + full ReduceAlgs** | 🚧 **IN PROGRESS** — `PHASE8_PLAN.md`. **WP8.1 COMPLETE, gate-green** (dispatch skeleton + GUI no-ops `82b50fe`; output-path machinery + `Export Counts` + the `compare_export` golden harness `929145c`). **WP8.2 COMPLETE, gate-green:** sub-step 1 (`71067f7`) = bus/node solution exports; sub-step 2a (`668bd18`) = the aggregate PD/PC power exports `Powers`/`Losses`/`P_byphase` + the mutable element-walk infra + the MVA/kVA `Parm2` pre-parse; **sub-step 2b** = the symmetrical-component family `SeqVoltages`/`SeqCurrents`/`SeqPowers` + the `ColTol::gate` denominator-gate harness machinery; **sub-step 2c** = the per-terminal/per-conductor element exports `Currents`/`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`Taps` + the `ColSel` name-prefix\|index-parity harness refactor + the `ElemPowers` Vsource order fix; **sub-step 3** = the matrix/summary exports `Yprims`/`Y`/`SeqZ`/`Summary`/`Result` + the `Y` triplet Parm2 flag + the `Summary` append/`DateTime`-mask (`ColSel::Index`+`GateSpec::Mask`) + the `SeqZ`-faultstudy fixture + the PM-build-faithful always-`null` `Result`; **completion gate** = the IEEE8500 `Voltages`/`Summary`/`Counts` goldens (`run_shared_exports`) + the `Export`-unblocked corpus migration (`solvable_now` **88→119**, COVERAGE **26.3%→35.5%**) + the Rust `CorpusGuard` (corpus stays pristine under report-writing decks). The "9 decks hang" tracked-open is **RESOLVED — no hang** (all complete + converge; watchdog artifact; stale tags refreshed, see §1f). Branch `phase-8-reporting`. **WP8.3 IN PROGRESS** — step 1 (`Export Monitors`) + **step 2** (the `Meters`/`Generators`/`Loads`/`PVSystem_Meters`/`Storage_Meters` register/load dumps + the DER `SampleAll`/`ResetAll` wiring the export surfaced as a gap) + **step 3a** (`EventLog`/`ErrorLog` dumps + the missing Circuit-build `LogThisEvent` markers the `Set Log=yes` EventLog golden surfaced) + **step 3b** (`Faultstudy` — read-only per-bus 3φ/1φ/L-L fault currents over the WP7.9-precomputed `Ysc`/`BusCurrent` via local `CMatrix` `YFault` scratch inversions) + **step 3c part 1** (`BusReliability`/`BranchReliability`/`Capacity` — read-only over the `RelCalc`-populated bus/branch fields + a synthesized recloser+meter deck fixture with a new deck-based golden runner) + **step 3c part 2** (`Overloads`/`Unserved`/`AllocationFactors` — PD-overload rows with the byte-faithful `NormAmps<=0` column shift, the mutable `EEN`/`UE` Load criteria + `u…` pre-parse, `DumpAllocationFactors`; audit follow-up caught + fixed the column-shift empty-field defect) + **step 3c part 3** (`Sections`/`Profile` — the meter `SectionCount`/`FeederSections` persistence, the `meter=`/phases-to-plot pre-parses, `Set/Get Markercode|Nodewidth` + `Circuit.NodeMarker*`, the multi-meter `Bus_Int_Duration` OOB guard) + **step 4** (`TSystemMeter` core + the full demand-interval machinery: the `DI_*`/phase-voltage/overload/volt-exception writers, the DI `Set` option handlers, `Set year=` `Set_Year` side effects, `CloseDI`, solve-loop open/close wiring, §2.6) + **step 5** (completion gate: 47 probe-clean decks migrated + the silent `Spectrum.CSVFile` no-op fixed → 2 more; `solvable_now` **119→168**, COVERAGE **50.1%**) landed gate-green; **next = the deferred WP8.3 audits**, then WP8.4 (Show). Detail in §1f |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 730, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_phase6 1, golden_phase7 1,
                            # golden_phase7_protection 1, golden_phase8 54,
                            # golden_checkpoints 1, golden_ieee8500 1,
                            # golden_reliability 1, golden_allocation 1,
                            # golden_gendispatcher 1, golden_autoadd_reduce 1,
                            # golden_slice 2, golden_smoke 3, props_roundtrip 1,
                            # corpus_manifest 1, corpus_live 3 (168 solvable_now
                            #   cases live-compared)
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

The full work-package logs for the completed, merged phases (and the completed
Phase-7/Phase-8 work packages) live under `docs/phase-records/` to keep this
handoff lean. They are frozen history, superseded only by the code and tests.
The two roll-ups that back the compact §1e/§1f frontier below:
[`phase-7.md`](docs/phase-records/phase-7.md) (the WP7.1–7.10 record) and
[`phase-8.md`](docs/phase-records/phase-8.md) (the completed WP8.1/8.2/8.3-step
detail). The per-phase entries:

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

The full per-WP roll-up (WP7.1–WP7.10 step summaries, decisions, audits, gate
detail, and the cross-cutting carry-forward rules) is **archived** at
[`docs/phase-records/phase-7.md`](docs/phase-records/phase-7.md); the deeper
per-step detail lives in the sibling `phase-7-wp{1..7}.md` archives. Phase 7 is
COMPLETE + gate-green on the branch, **NOT merged to `main`** (per-phase merge =
explicit-request-only HARD STOP). Headline: **WP7.1** line constants & geometry,
**WP7.2** protection (Fault/Fuse/Recloser/Relay/SwtControl + reliability
activation), **WP7.3–7.5** DER (DynamicExp/InvBasedPCE/PVSystem;
Storage/StorageController; InvControl/ExpControl), **WP7.6** Harmonics, **WP7.7**
Dynamics core (SolveDynamic + Generator/PVSystem/Storage/IndMach012 + DynEqPCE),
**WP7.8** Converter/FACTS (VSConverter/VCCS/UPFC+UPFCControl/ESPVLControl),
**WP7.9** FaultStudy (AutoAdd/Monte/LD/Feeder empirically deferred — zero corpus
cases), **WP7.10** exit. Retro audit (WP7.7 step 4 → WP7.8): no Critical/Major
correctness bug. Two real port bugs found+fixed in WP7.5 (the cross-step
`FFlagVWOperates` latch + the missing post-`DoPendingAction` `LoadsNeedUpdating`),
each a [[dont-rationalize-conditioning]] instance. **Tracked-open** (both
Plot-blocked, zero corpus payoff): GFM grid-forming mode + Generic/TD21 relay
`Sample`. lib **713**; `solvable_now` **88** at Phase-7 exit.

---

## 1f. Phase 8 record (`PHASE8_PLAN.md`) — 🚧 IN PROGRESS

Execution plan: **`PHASE8_PLAN.md`** (WP8.1–WP8.8, the reporting/output + full
executive layer; per-step cadence = `PHASE8_PLAN.md §0`). Phase 8 is almost
entirely *read-and-format* — no new electrical math, no new solve mode; the risk
is faithful report layout and **not silently faking output**. The full per-step
log for the **completed** work (decisions, audits, gate detail, the real-gap
write-ups) is **archived** at
[`docs/phase-records/phase-8.md`](docs/phase-records/phase-8.md). Live frontier:

- **WP8.1 (Report infrastructure) — ✅ COMPLETE, gate-green** (`82b50fe` dispatch
  skeleton + GUI/`Plot`/`Visualize` headless no-ops with the #301 pre-circuit
  guard; `929145c` output-path machinery + `Set DataPath=` + `Export Counts`
  end-to-end + the `compare_export` golden harness). The `Show`-silent vs
  `Export/Save/Dump`-loud-`NOT_PORTED` asymmetry is forced + proven-safe.

- **WP8.2 (Export: solution outputs) — ✅ COMPLETE, gate-green.** The full export
  families, read-only/mutating over the solved circuit: bus/node (`Voltages`/
  `BusCoords`/`NodeNames`/`YNodeList`), aggregate power (`Powers`/`Losses`/
  `P_byphase` + the `for_each_enabled_elem`/`export_with_mut` mutable element-walk
  infra + the MVA `Parm2` pre-parse), symmetrical-component (`SeqVoltages`/
  `SeqCurrents`/`SeqPowers` + the `ColTol::gate` denominator gate), per-terminal
  (`Currents`/`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`/`Taps` + the
  `ColSel` harness refactor), and matrix/summary (`Yprims`/`Y`/`SeqZ`/`Summary`/
  `Result`). **Completion gate:** the IEEE8500 `Voltages`/`Summary`/`Counts`
  goldens + the `Export`-unblocked corpus migration (`solvable_now` **88→119**,
  COVERAGE **26.3%→35.5%**) + the Rust `CorpusGuard` (corpus stays pristine under
  report-writing decks). Two tracked-opens **RESOLVED** here (detail in the
  archive): the "9 decks hang" was a debug-build watchdog artifact (all 9 converge,
  ≤3.4s release), and the rare live-gate flake was a per-process convergence misfire
  *inside the pinned oracle* (now retried in-process) — neither a Rust bug.

- **WP8.3 (Export: device/reliability + logs) — 🚧 IN PROGRESS**, landed gate-green
  through **step 3c part 2**:
  - **step 1** — `Export Monitors` (the Phase-6 f32 stream → `Monitor::to_csv` +
    `util::comma_text`).
  - **step 2** — the `Meters`/`Generators`/`Loads`/`PVSystem_Meters`/`Storage_Meters`
    register/load dumps + the `/m` multi-file switch. **Real gap fixed:** the DER
    `SampleAll`/`ResetAll` tail was never wired into the solve loop, so DER energy
    registers stayed zero — the export surfaced it.
  - **step 3a** — `EventLog`/`ErrorLog` dumps. **Real gap fixed:** the three
    circuit-build `LogThisEvent` markers (`ReprocessBusDefs`/`DoResetMeterZones`)
    were never wired (ported before the `EventLog` type existed).
  - **step 3b** — `Faultstudy`: read-only per-bus 3φ/1φ/L-L fault currents over the
    WP7.9-precomputed `Ysc`/`BusCurrent` via local `CMatrix` `YFault` scratch
    inversions (no re-solve, no mutation).
  - **step 3c part 1** — `BusReliability`/`BranchReliability`/`Capacity`, read-only
    over the `RelCalc`-populated fields + a synthesized recloser+meter deck fixture
    (new deck-based golden runner).
  - **step 3c part 2** — `Overloads`/`Unserved`/`AllocationFactors`. The audit
    follow-up **revealed + fixed** a real degenerate-path defect: the `NormAmps<=0`
    column-shift row was not byte-faithful (Pascal's trailing-`, ` doubling → an
    empty AmpsOver field).
  - **step 3c part 3** — `Sections`/`Profile`. `Sections`: the meter now
    **persists** `SectionCount` + `FeederSections` (the struct moved to
    `energymeter/mod.rs`; `calc_reliability_indices` writes both back on success
    and zeroes only the count on the no-OCP abort, exactly Pascal's field
    lifecycle), read back by `export_sections` with the `meter=<name>` pre-parse
    (`CompareTextShortest` incl. the empty-ParamName quirk; unknown name → all
    meters). `Profile`: the branch-list voltage profile over each meter's
    `SequenceList` + zone-build `DistFromMeter`, all seven `PhasesToPlot`
    selector branches (default/all/primary/ll3ph/llall/llprimary/explicit-digit
    `IntValue`) + the `WriteNewLine` layout and the header's appended `Title=…`
    tail. **Ported-immediately gaps** (per the no-deferral rule): `Set/Get
    Markercode|Nodewidth` handlers + `Circuit.node_marker_code/width`
    (Circuit.pas 16/1 defaults — Profile echoes them per row). **Upstream OOB
    found:** with ≥2 meters, Pascal's `Bus_Int_Duration` sweep
    (EnergyMeter.pas:2521) walks *all* circuit buses and indexes
    `FeederSections[BusSectionID]` from *another* meter's zone — in-range ids
    are a deterministic cross-zone overwrite (reproduced); out-of-range ids are
    an unchecked heap read (unpinnable garbage) — Rust skips that write
    (documented at the guard; NOT a `TODO(compat)` — no defined upstream value).
    Goldens: a synthesized two-meter recloser+fuse deck (`export_sections`,
    `export_sections_meter`) + 7 Profile variants on metered IEEE13
    (`run_shared_exports`) + the no-RelCalc/unknown-meter structural edges.
  - **step 4** — the `TSystemMeter` core + the demand-interval machinery
    (plan §2.6; new `solution/meters/demand_interval.rs`):
    - `MeterStream` reproduces the `MemoryMap_lib` observable emission (strings
      verbatim, doubles `", "`-separated `%-g` 15-sig); the `Append*` re-open
      paths are **proven dead upstream** (no `AppendAllDIFiles` caller in
      0.14.5) so files are always created fresh.
    - `SystemMeter` (`Clear`/`Integrate`/`TakeSample`/`Reset`/`Save`) sampled in
      `SampleAll` from `GetTotalPowerFromSources` + `Circuit.Losses`; state (+
      the whole `EmDiState` class-level DI state) lives on `Circuit` so exec and
      solve loop share it.
    - Per-meter DI files + the **phase-voltage report**: the `TakeSample`
      pu-voltage accumulators (`VphaseMax/Min/Accum/Count`, the `jiIndex`
      layout, the `|V|/kVBase` 1000·pu quirk) landed in the zone walk;
      `DI_Totals`/`EnergyMeterTotals`/`Totals`/`SystemMeter` writers; the
      overload (`DI_Overloads`, incl. the <3-phase per-phase current mapping
      via `MapNodeToBus` ≡ Pascal's FirstBus re-parse) and voltage-exception
      (`DI_VoltExceptions`, primary + LV scans) reports.
    - Wiring: `OpenAllDIFiles` at daily/yearly/peak-day solve head (duty opens
      nothing, faithfully), the daily/duty `finally` close, yearly staying open
      (pinned by a structural test), `Set DemandInterval/DIVerbose=` →
      `ResetAll`, `Overloadreport/Voltexceptionreport/SampleEnergyMeters=`,
      `CloseDI`, `Clear` flushing open files, and the **`Set year=`
      `Set_Year` side effects** (DI close + clock reset + `ResetAll`) that were
      missing from the YEAR handler — a real gap the step closed.
    - Goldens: 9 DI files from one daily IEEE13 fixture (meter +
      `PhaseVoltageReport` + all four switches), each diffed vs the oracle.
      **Two upstream-garbage findings documented:** opening the DI files with
      an unbuilt zone makes the oracle render *uninitialized heap memory* as
      PHV vbase labels (unpinnable — the fixture pre-solves so both engines'
      headers are deterministic; our unbuilt-zone rendering is the sane
      zero-vbase form), and the zone vbase list collects **from-buses only**
      (IEEE13 ⇒ a single 4.16 kV PHV group — bus 634 is nobody's from-bus).
  - **step 5 (completion gate)** — a release `dss-cli` probe over all 154
    `skipped_unsupported` decks found **49** that now compile+run clean (the
    InvControl daily family + PVSystemTest + the two IEEE_519 harmonics decks +
    RevRegTest + the InverterTechNote pair); moved to candidates and
    live-classified: **47** matched the oracle full-model and migrated. The two
    IEEE_519 (harmonicT) decks diverged — the Rust final NodeV was the
    *fundamental*, the oracle's the last harmonic — root-caused to the **silent
    `Spectrum.CSVFile` no-op** (a Phase-2 `TODO(phase2+)` that stored the
    filename and loaded nothing, so `CollectAllFrequencies` saw only 60 Hz and
    the harmonic sweep never ran): `TSpectrumObj.ReadCSVFile` is now ported via
    the WP5.2b deferred-`FileLoad` path (+ a unit test pinning the parse, the
    ×0.01 `%Mag` scale, and the `NumHarm` shrink), after which **both decks
    matched the oracle live and migrated**. `solvable_now` **119→168**,
    COVERAGE **35.5%→50.1%**; corpus stays pristine (probe outputs
    `git clean`ed; CorpusGuard confirmed 0 dirt).
  - Scores at the frontier: golden_phase8 **54**; lib **730**; `solvable_now`
    **168**.
  - **next:** the deferred WP8.3 audits (steps 3c p3 + 4 + 5 in one pass —
    user-authorized multi-step execution), then WP8.4 (Show reports).

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
