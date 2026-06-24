# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-24 — **Phase 7 IN PROGRESS** (branch
`phase-7-extended-elements`): **WP7.1 COMPLETE; WP7.2 (Protection) COMPLETE;
WP7.3 (DER A) COMPLETE; WP7.4 (DER B) — step 1 (Storage element) COMPLETE.**
WP7.4 step 1 = the port of `PCElements/Storage.pas` (`TStorageObj`, the battery PC
element) on the inverter base: the charge/idle/discharge **state machine** +
**integrated `%stored`** advanced by `UpdateStorage` in the time-step cleanup,
`ComputePresentkW`/`ComputeInverterPower`/`CheckStateTriggerLevel`, `CalcYPrim`
(state-dependent `YeqDischarge`), `DoConstantPQ`/`DoConstantZ`, `ComputeDCkW` + the
loss split, registers/`TakeSample`; registered as a zone PCE with the GFM guard +
the Monitor mode-3 metered-kind fix + two new enums. Gate: `props/storage.json`
(7 scenarios), goldens `phase7/storage_{snapshot,clamps,daily}` (the daily one pins
the **SOC trajectory** — 200 kWh → 20 kWh reserve → Idling at 1e-6); **corpus stays
44** (no Storage feeder unblocked by the element alone — all need Plot/Export/
InvControl/StorageController; 10 probed candidates re-tagged to their real blocker).
lib 543 → **557**. **next = WP7.4 step 2: the real `StorageController`
(fleet/dispatch, replace the WP6.8 skeleton).**
*(WP7.3 = the `DynamicExp` object + the `InvBasedPceData` inverter base + `PVSystem`;
its detail is in §1e.)*
**WP7.1 (line constants & geometry) and WP7.2 (protection) are COMPLETE** — the
per-step detail (the Carson line-constants engine + the `WireData`/`CNData`/
`TSData`/`LineSpacing`/`LineGeometry` catalog + Line's geometry/spacing path and
its golden; `Fault`/`SwtControl`/`Fuse`/`Recloser`/`Relay` + reliability
activation + the protection gate) lives in **§1e** (per-step summaries) and the
archives [`phase-7-wp1.md`](docs/phase-records/phase-7-wp1.md) /
[`phase-7-wp2.md`](docs/phase-records/phase-7-wp2.md). `solvable_now` is at **44**.

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
| 7 | Extended elements: DER, protection, line constants, harmonics, dynamics | 🚧 in progress — `PHASE7_PLAN.md` (WP7.1–WP7.10); branch `phase-7-extended-elements`; **WP7.1 done**, **WP7.2 (Protection) COMPLETE**, **WP7.3 (DER A) COMPLETE**, **WP7.4 (DER B) step 1 (Storage element + zone + SOC gate) COMPLETE**; **next = WP7.4 step 2 (the real `StorageController`)**. Per-step detail in §1e |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 557, golden_feeders 1,
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
  be installed).** For each of the **44** `solvable_now` cases the gate
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

**WP7.3 (DER A: DynamicExp + InvBasedPCE + PVSystem) — ✅ COMPLETE.**
- **step 0 — `DynamicExp` (`general/dynamic_exp.rs`):** the user-defined
  differential-equation catalog object (`General/DynamicExp.pas`), a `DSS_OBJECT`
  registered before Generator/PVSystem/Storage (Pascal "before Generator,
  PVsystem, Storage"). Lands the object **and its expression interpreter**: setting
  `Expression` compiles the RPN diff-eq (`InterpretDiffEq`) into a flat `cmds`
  automation array (operator codes + variable/constant slots), evaluated each step
  by a stack machine (`SolveEq`) over a per-element `[value, derivative]` memory
  space — both ported loop-for-loop on the done `RPNCalculator`. Props: `NVariables`
  / `VarNames` (lowercased StringList) / `var` (active-var, drives `VarIdx`) /
  `VarIdx` (Pascal `SilentReadOnly`) / `Expression` (kept verbatim, cleared on a
  compile error) / `Domain` (`Time`/`dq`; parse default `dq`, field default `Time`).
  New enum `dynamic_exp_domain`; `MakeLike` is a no-op-with-error (Pascal 50099).
  Gate: `props/dynamicexp.json` (**9** oracle-pinned scenarios incl. the vendored
  Kundur expression, the bad-expr clear, the `var`/MakeLike/empty-`dt` error paths)
  + **13 interpreter unit tests** (cmds compilation + numeric `SolveEq` for the
  Kundur/π/trivial + operator-dispatch expressions + the
  `Get_*`/`IsInitVal`/`Check_If_CalcValue` accessors). The
  evaluator's *numeric* oracle pinning comes with the dynamics solve (WP7.7); here it
  is spec-pinned (Pascal is the spec) since the oracle exposes no `cmds`/`SolveEq`
  outside a dynamics run. *Self-contained: no solve-loop change.*
  *audit-code:* faithful interpreter/evaluator port; fixed a **panic** — an empty
  operand before `dt` (e.g. `expression=[ dt = b]`) hit `vars[0]` on an empty list
  and aborted the process, where Pascal raises a *catchable* `EStringListError`
  ("List index (0) out of bounds"), logs it, and leaves the expression as written.
  Now a recoverable error (+ a `props/dynamicexp.json` scenario + a unit test
  pinning it to the oracle). Surfaced-not-reproduced: on that error path Pascal's
  `SetLength(Cmds,+2)` leaves two zero `cmds` slots — unobservable (`cmds` on an
  errored expression is never evaluated; not a dumped property), so not reproduced.
  *audit-tests:* strong (exact `cmds` + numeric `SolveEq` + oracle props); closed
  the one gap — `SolveEq`'s dispatch was exercised for only 5/22 opcodes. Added an
  end-to-end operator-dispatch test (sqr/inv/ln/exp/`^`/swap) **and** a guard pinning
  the substring tie-break that makes `sqrt`/`atan2` dead opcodes (`sqr`/`atan` shadow
  them — verbatim Pascal quirk). lib **514 → 527**.
- **step 1 — `InvBasedPceData` (`pc/inv_based_pce.rs`):** the shared
  inverter-based PC-element base (`PCElements/InvBasedPCE.pas`, `TInvBasedPCE`) plus
  the `DynamicEq`/`DynOut` fields its parent `PCElements/DynEqPCE.pas` contributes.
  An **abstract base** — not New-able (`CreateDSSClasses` never registers it; not in
  `construct.rs`); PVSystem (step 2) and Storage (WP7.4) embed it the way Generator
  flattens `GenVars`, and dispatch the virtuals through the `InvBasedPce` trait.
  Lands: the `InvBasedPceData` data record; the `InvDynamicVars` **scalar**
  sub-record (`Shared/InvDynamics.pas` `TInvDynamicVars` — only the scalars, which
  back the PVSystem/Storage props `kVDC`/`kP`/`PITol`/`SafeVoltage`/`AmpLimit`/
  `AmpLimitGain`/`SafeMode`, so they must exist before those classes); the
  `InvBasedPce` virtual trait (`IsPVSystem`/`IsStorage`/`GetPFPriority`, base
  `False`); and the three power-flow shared methods —
  `StickCurrInTerminalArray` (wye/delta current routing, same sign convention as
  Generator), `Get_Presentkvar` (`Qnominalperphase·0.001·Fnphases`),
  `UsingCIMDynamics` (`VW|VV|WV|AVR|DRC`, WPMode deliberately excluded). **Deferred
  to WP7.7 (dynamics/GFM):** the `TInvDynamicVars` per-phase arrays + every method
  (`SolveDynamicStep`/`SolveModulation`/`CalcGFM*`/`InitDynArrays`/`Get_/Set_InvDyn*`),
  the `PICtrl` PI-controller array, `CheckAmpsLimit`, the GFM `GetCurrents`
  override, and the `DynEqPCE` dynamics memory (`DynamicEqVals`/`DynamicEqPair`/
  `UserDynInit`). Gate: **6 spec-pinned unit tests** (base `Create` defaults,
  `Get_Presentkvar` scaling, `UsingCIMDynamics` WPMode exclusion, wye/delta
  `StickCurr` routing, the trait default/override via a mock implementor) — spec-
  pinned (Pascal is the spec) since the oracle exposes none of these helpers outside
  a full PVSystem/Storage solve (numeric pinning arrives with PVSystem, step 2).
  *Self-contained: no solve-loop change, no class registration.* *audit-code:*
  faithful + complete (all 55 `TInvBasedPCE` fields incl. the full 18-scalar
  `TInvDynamicVars` set; `StickCurr` 1-based→0-based routing verified vs the
  oracle-validated `Generator::stick_curr`; deferrals all dynamics/GFM →
  plan-sanctioned WP7.7) — **no fix needed**. *audit-tests:* strong (exact values,
  the WPMode edge, the delta wrap); closed one gap — the `StickCurr` tests seeded a
  zero array, so they could not distinguish accumulate (`+=`/`-=`) from overwrite
  (`=`). Added `stick_curr_wye_accumulates_neutral` (non-zero seed + two phase
  currents stacked onto the wye neutral — the `+=`→`=` regression guard). lib
  **527 → 534**.
- **step 2 — `PVSystem` (`pc/pvsystem/`) + steps 3–4 (zone + gate):** the
  photovoltaic PC element (`PCElements/PVsystem.pas`, `TPVsystemObj`) on the
  Generator injection template (WP6.2) with the step-1 `InvBasedPceData` base
  embedded (flattened like `GenVars`); a directory module mirroring
  `generator/` (`mod`/`nominal`/`solve`/`registers`/`accessors`/`tests`). Lands the
  **power-flow** PVSystem: the PV-panel + inverter model `ComputePanelPower`
  (irradiance·shape·`Pmpp`·temp-derate) → `ComputeInverterPower` (cut-in/cut-out,
  efficiency curve, watt/var priority, the kvar + `kVA` clamps — ported
  loop-for-loop, with a Pascal-faithful `Sign` that returns 0 at zero) →
  `kWOut_Calc`; `SetNominalDEROutput` (= `SetNominalPVSystem`: shape/temperature
  by solve mode → per-phase P/Q → `YEQ`/`YEQ_Min`/`YEQ_Max`/`PhaseCurrentLimit`);
  `CalcYPrim`/`CalcYPrimMatrix`; `DoConstantPQPVsystemObj` (model 1, with the
  current-limited + impedance-outside-band branches) / `DoConstantZPVsystemObj`
  (model 2) + the `ForceBalanced` pos-seq path; energy-meter registers +
  `TakeSample`. Curves/shapes resolve via the snapshot-clone ObjectRef pattern:
  irradiance `daily`/`yearly`/`duty` (LoadShape), `Tdaily`/`Tyearly`/`Tduty`
  (TShape), `EffCurve`/`P-TCurve` (XYcurve), and **`DynamicEq` as a real
  `DynamicExp` ref** (step 0). Two new enums: `pvsystem_model`
  (ConstantP_PF/ConstantY/UserModel) and `inv_control_mode` (GFL/GFM). **Step 3:**
  `ElemKind::PVSystem` + a `pv_systems` circuit list; registered in `construct.rs`
  after the protection controls (Pascal `PVSYSTEM_ELEMENT`); `is_zone_pce` admits
  PVSystem (`EnergyMeter.pas:1911` — the zone walk still ignores it for
  accumulation, matching Pascal "ignore other PC elements"). Also fixed a **real
  bug** surfaced by the corpus probe: the Monitor mode-3 metered-kind detection
  classified only Load/Generator as `PcElement`, so a `mode=3` monitor on a
  PVSystem (Pascal: any `TPCElement`) errored "must be a power conversion element"
  and left a singular Y — `Test/PVSystemTest.dss`; PVSystem now classifies as
  `PcElement` (the mode-3 channel stays the Phase-6 empty stub, like Generator).
  **Deferred (WP7.6/7.7, matching Generator):** the GFM mode
  (`DoGFM_Mode`/`CalcGFMYprim`/`CheckOLInverter`), harmonics
  (`DoHarmonicMode`/`InitHarmonics`), dynamics
  (`DoDynamicMode`/`InitStateVars`/`IntegrateStates` + the state-variable
  interface `NumVariables`/`Get_/Set_Variable`/`VariableName`), the user-written
  DLL model (model 3 → error 567, never ported), and `MakePosSequence`.
  Gate: **`props/pvsystem.json`** (10 oracle-pinned scenarios — default, PF, kvar/
  delta, cut-in/out + model 2, eff/P-T curves, all six shapes, the kvar/Pmin
  limits, the inverter params, a `DynamicEq` ref, MakeLike — all round-trip
  exactly); two targeted goldens **`phase7/pvsystem_{snapshot,curves}`** (voltages
  + element powers 1e-6, the second exercising the eff curve + P-T derate + the
  `kVA` clamp at pf=0.95); **corpus migration 37 → 44** (7 PVSystem cases:
  EPRI `Master_withPV`, the 2 `CurrentkvarLimite` kvar/kvarNEG, the 4 ConstantPF
  `SnapShot_PFP_*`), with the InvControl/Export/Plot cases re-tagged accurately
  (→ `unsupported_class=InvControl` / `unsupported_command=Export`) and **2**
  `varCapability` cases held in `needs_investigation` (live ~4e-6 mismatch:
  near-ideal-source `Z=1e-8` amplifies a sub-1e-6 eff-curve interpolation
  difference — the kvar-clamp path is validated by the sibling
  `CurrentkvarLimite` cases that pass at full tolerance; conditioning, not a
  logic bug — diagnosed in the manifest note). lib **534 → 539**.
  *audit-code:* faithful port (the intricate `ComputeInverterPower` clamp cascade,
  `SetNominalDEROutput`, `CalcYPrimMatrix`, `DoConstantPQ`/`Z`, `MakeLike`, the
  side-effects all match Pascal + the oracle goldens). Fixed one real
  silent-degradation: **`ControlMode=GFM`** is a settable property (round-trips)
  but its solve behavior (`DoGFM_Mode`/`CalcGFMYprim`) is WP7.7 — the model dispatch
  ignored `gfm_mode` and silently ran the regular PQ model (plausible-but-wrong
  numbers). Now a **pre-solve guard** (`solution/dispatch.rs`) aborts the solve with
  an explicit "not ported (WP7.7)" error + `solution_abort` (the
  deferral-is-never-a-silent-fallback convention), plus a defensive
  contribution-path guard mirroring the user-model path; 2 exec tests pin it (the
  clean snapshot solve + the GFM error). Surfaced-not-fixed (recorded, both **exact
  Generator parity**, not new): the GENERALTIME arm ignores `ActiveLoadShapeClass`
  (`SysCtx` carries no class — shared with `generator/nominal.rs`); and
  `Set_ConductorClosed` is not wired (`pv_system_obj_switch_open` never set, like
  Generator's `gen_switch_open`). lib **539 → 541**.
  *audit-tests:* the property + solve goldens are genuinely oracle-pinned (10 props
  scenarios round-trip exactly; `pvsystem_{snapshot,curves}` pin V + element powers
  at 1e-6) — but the plan's "inverter control discrete state exact" was guarded by a
  unit assertion that **could not fail** (`kva_clamp_backs_off_kw` checked only an
  upper bound — kw=0/kvar=0 passed it) and the Monitor mode-3 fix had **no fast
  regression test**. Closed both: added golden **`phase7/pvsystem_clamps`** (3
  PVSystems oracle-pinning the three discrete states at 1e-6 — non-priority kVA
  back-off `kw=300,kvar=400`; cut-out `kw=0`; absorption-clamp + back-off
  `kvar=-300,kw=400`), strengthened the unit test to pin `kw=300,kvar=400` exactly +
  added `kvar_absorption_clamp_then_backoff`, and added an exec regression test
  (`pvsystem_accepts_mode3_monitor`) for the mode-3 metered-kind fix. Corrected an
  **overclaiming** deferral note: the passing `CurrentkvarLimite/kvarNEG` sibling
  uses `kvar=+500` (generation), so it does **not** cover the absorption direction —
  now independently pinned by `pvsystem_clamps` (pvc) + the unit test. Surfaced-not-
  fixed (pre-existing, WP7.5): 11 combined-class cases still carry a stale
  `unsupported_class=InvControl,PVSystem` tag (they need InvControl; a re-probe lands
  with WP7.5). lib **541 → 543**.
- **next:** **WP7.3 (DER A) COMPLETE** → **WP7.4 (DER B): `pc/storage.rs`
  (`TStorageObj` on the inverter base — the charge/idle/discharge state machine +
  integrated `%stored`) + the real `StorageController` fleet/dispatch.**

**WP7.4 (DER B: Storage + StorageController) — 🚧 step 1 (Storage element)
COMPLETE.** Port of `PCElements/Storage.pas` (`TStorageObj`, 3556 lines — the
largest PC element) as a directory module `pc/storage/` (mod/nominal/solve/
registers/accessors/tests) on the Generator template, embedding the WP7.3 step-1
`InvBasedPceData` the way PVSystem does. Adds the **charge/idle/discharge state
machine** (`FState` ∈ {−1,0,1}) and an **integrated state of charge**
(`kWhStored`/`%stored`) that the time-step cleanup hook advances. Scope = the
**power-flow** Storage:
- `ComputePresentkW` (state + dispatch → terminal kW: discharge `kWrating·%Discharge`,
  charge `−kWrating·%Charge`, idle `−kWOutIdling`), `CheckStateTriggerLevel` (the
  Follow / trigger-level / `ChargeTime`-of-day dispatch), `ComputeInverterPower`
  (the idling-state branch + the cut-in/out-reflected-to-AC clamp cascade, ported
  loop-for-loop), `kWOut_Calc` (the VW requesting/limiting regions).
- `CalcYPrimMatrix` (state-dependent: charge `+YeqDischarge`, idle `0`, discharge
  `−YeqDischarge`), `DoConstantPQStorageObj`/`DoConstantZStorageObj` + the inverter
  clamp, daily/yearly/duty shapes (snapshot-clone), a real `DynamicEq` ref.
- **The SOC integration:** `ComputeDCkW` (ideal-inverter signed terminal kW, or the
  efficiency-curve `GetCoefficients`+`QuadSolver` solve — `XyCurveObj::get_coefficients`
  ported alongside) + the loss split (idling/inverter/charge-discharge) +
  `UpdateStorage` (the `(DCkW+idle)/eff·Δh` charge/discharge integration, the reserve/
  full clamps and the state flip), wired into `EndOfTimeStepCleanup` (Pascal
  `StorageClass.UpdateAll`) via a new `ckt.storages` list. Registers + `TakeSample`
  (discharge-hours-only) mirror PVSystem.
- Registration: `ElemKind::Storage` + the circuit list + `construct.rs` (after the
  protection controls, before PVSystem; Pascal `Storage_ELEMENT`); two new enums
  (`storage_state` Charging/Idling/Discharging, `storage_dispatch_mode`); `is_zone_pce`
  admits Storage; the Monitor mode-3 metered-kind branch classifies Storage as
  `PcElement` (mirrors the PVSystem fix); the `dispatch.rs` GFM pre-solve guard
  rejects `ControlMode=GFM` (WP7.7) with an explicit error (no silent PQ fallback).
  `SysCtx` gained `time_of_day`/`dyna_h` (the `ChargeTime` trigger).
- **Deferred** (matching PVSystem): GFM solve (`DoGFM_Mode`/`CalcGFMYprim`), harmonic
  injection, the dynamics state machinery + the state-variable interface
  (`NumVariables`/`Get_Variable`/`VariableName`), the user-written `UserModel`/
  `DynaModel` DLLs (never ported), `MakePosSequence` → WP7.6/7.7.
- **Gate:** `props/storage.json` (7 scenarios, all round-trip exactly); goldens
  `phase7/storage_{snapshot,clamps,daily}` — the snapshot/clamps pin the three
  discrete states' terminal powers, and **`storage_daily` pins the integrated SOC
  trajectory** (a 200 kWh battery discharging 6 h depletes to the 20 kWh reserve and
  flips to Idling — `kWhStored`/`%Stored`/`State` read back via `? …` and matched at
  1e-6, the WP7.4 "%stored/state trajectory exact over a daily run" gate); exec tests
  (`storage_snapshot_solves_clean`, `storage_daily_run_depletes_soc` — the
  EndOfTimeStepCleanup wiring guard, `storage_gfm_mode_errors_not_silent`,
  `storage_accepts_mode3_monitor`). **Corpus: 0 net growth (stays 44).** No corpus
  Storage feeder is unblocked by the element alone — all are gated behind `Plot`/
  `Export` (Phase 8), `InvControl` (WP7.5), `StorageController` (step 2),
  file-backed arrays, or oracle errors; the 10 probed candidates were
  re-tagged to their **real** blocker (was stale `unsupported_class=Storage`). The
  targeted golden is the focused gate; the live-corpus burn-down for Storage waits
  on WP7.5 / step 2 / Phase 8. lib **543 → 557**.
- **audit-code follow-up:** verdict faithful 1:1 (no Critical/Major). Marked the
  `UpdateStorage` GFM absorbing/recharge branch `NOT_PORTED → WP7.7` (it was an
  implicit drop; GFM is rejected at solve time so the branch is unreachable in
  power flow, but the marker makes the WP7.10 grep sweep catch it). Surfaced-not-
  fixed (all upstream-faithful / pre-existing): `ComputeDCkW`'s `while (a≠ga AND
  b≠gb) OR (N>9)` loop is ported verbatim — a latent non-termination only if an
  `EffCurve` is non-monotonic enough to oscillate between segments (never for the
  gated cases; the ideal path early-returns, monotonic curves converge exact-float
  in 1–2 iters — matches the oracle, which would also hang); the `CalcYPrimMatrix`
  harmonic branch is dead until WP7.6; Monitor mode-7 (the Storage monitor) is
  header-only like PVSystem mode-3 (WP7.7, no gate uses it).
- **audit-tests follow-up:** closed two real gaps. (1) the daily golden only
  covered **discharge**→reserve→idle; added `phase7/storage_daily_charge` (a 20%
  battery charging at 80 kW fills to the 200 kWh rating in 4 h and flips to Idling)
  to oracle-pin the **charge** half of `UpdateStorage` (the fill branch + the
  full-clamp + state flip). (2) the `kva_clamp_backs_off_kw_on_pf` unit test pinned
  only the apparent power (a wrong-leg PF-priority regression `kw=20,kvar=15` also
  sits on the kVA circle → passed); strengthened to pin `kw_out=16.5359…`,
  `kvar_out=18.75` exactly (oracle-probed), and added a 4th battery `sd` (pf=0.8 at
  the kVA limit) to `storage_clamps` so the Q-priority back-off legs (kW=330.72,
  kvar=375) are oracle-pinned too. (3) re-probed the 2 *purely* stale
  `unsupported_class=Storage` cases (`GFM_IEEE8500/Run_8500Node_GFMDaily{,SmallerPV}`)
  → re-tagged to their real blocker. **Surfaced-not-fixed:** ~29 other Storage cases
  still carry a *superset* tag that includes the now-supported `Storage` alongside
  the real blocker (StorageController / BatchEdit / InvControl); they clear on the
  WP7.4-step-2 / WP7.5 re-probes (the bijection holds; `corpus_manifest` passes).
- **next:** WP7.4 **step 2** — the real `StorageController` (replace the WP6.8
  skeleton: a non-empty `MakeFleetList`, the dispatch modes
  PeakShave/Follow/Support/Schedule/Time, `Sample`/`DoPendingAction` on the control
  sweep), gated on `Test/Storage*`/`StorageControllerTechNote` event-log equality.

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
