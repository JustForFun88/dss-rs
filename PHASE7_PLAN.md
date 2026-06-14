# Phase 7 — Detailed Execution Plan: Extended Elements (DER, Protection, Line Constants, Harmonics, Dynamics)

> Companion to `PORTING_PLAN.md` §Phase 7; same status and rules of engagement as
> `PHASE4_PLAN.md` §0 / `PHASE5_PLAN.md` / `PHASE6_PLAN.md` (Pascal is the spec;
> probe the oracle, never guess FPC semantics; mark `TODO(compat)` for deliberate
> upstream-inexactness reproductions and `NOT_PORTED` for deferrals, each pointing
> at its phase; goldens are regenerated **manually** with the pinned oracle
> (`tools/golden/PIN.txt`: dss-python 0.15.7 / backend 0.14.5); the standard
> three-command gate must be green per WP; commit only on explicit user request).
> **Prerequisite: Phase 6 complete and merged** (`b98223a`) — this phase consumes
> the control-loop dispatch (`ElemStore::{obj,pair_mut,triple_mut}` +
> `DssObject::as_any_mut`), the `CtrlCtx` disjoint-borrow, the meter/monitor
> sample hooks, the snapshot-clone ObjectRef pattern, the checkpoint gate, and the
> **live corpus gate** (`corpus_live.rs` + manifests) without re-explaining them.
>
> **Stop-and-confirm cadence (same as PHASE4_PLAN §0.8 / PHASE6_PLAN §0):** after
> each small step (a WP or a self-contained sub-step), run the full gate, **update
> `STATUS.md`**, then stop and wait for the user's explicit confirmation before the
> next step — unless the user has explicitly authorized executing multiple WPs in
> one pass.
>
> **On Pascal line references:** this plan is written just-in-time, before the
> per-WP deep read. It cites Pascal **units and procedure/identifier names**
> (stable across the codebase) plus verified file sizes; **exact line numbers are
> confirmed when each WP opens its unit** and recorded in `STATUS.md` as the WP
> lands (the PHASE6_PLAN convention). Sizes below are `wc -l` of the vendored
> `.inputs/dss_capi/src` at planning time.

## 0. Why this is the largest phase, and the ordering

Phase 7 is ~18% of the whole port — the biggest — and spans **~29k lines** of
Pascal across six independently-gated sub-blocks (PORTING_PLAN §Phase 7). Sizes:

| Sub-block | Pascal units (≈ lines) | Total |
|---|---|---|
| 1. Line constants | ConductorData 308, WireData 98, CNData 192, TSData 165, CableData 179, LineSpacing 265, LineGeometry 1019, LineConstants 652, OHLineConstants 37, CNLineConstants 258, TSLineConstants 240, CableConstants 186 | ~3.6k |
| 2. DER | Storage 3556, PVsystem 2735, InvBasedPCE 268, InvControl 3586, ExpControl 749, StorageController 2036 (skeleton exists) | ~13k |
| 3. Protection | Relay 2353, Recloser 738, fuse 537, SwtControl 395, Fault 611, TCC_Curve (done) | ~4.6k |
| 4. Harmonics | harmonic solve mode + per-element `InitHarmonics`/injection; Spectrum (object done) | small, cross-cutting |
| 5. Dynamics | IndMach012 1482, UPFC 1181, vccs 861, DynamicExp 591, VSConverter 498, ESPVLControl 461, UPFCControl 290, DynEqPCE 273 + per-element state-var machinery | ~5.6k |
| 6. Faultstudy/AutoAdd/Feeder | FaultStudy + AutoAdd solve modes (AutoAdd skeleton exists), Feeder (largely dead) | small |

**Execution order (risk-ascending, dependency-respecting — deviates from the
PORTING_PLAN numeric order deliberately):**

1. **Line constants first** — fully self-contained (catalog classes + Carson math
   → Line YPrim); **no solve-loop change**; unlocks geometry-based feeders and a
   batch of `skipped_unsupported` corpus cases. Lowest risk, clean warm-up.
2. **Protection second** — smaller than DER, reuses the Phase-5 control sweep
   verbatim, and **closes a Phase-6 loose end**: it sets `Flg.HasOCPDevice`, which
   activates the SAIFI/SAIDI/section logic ported-but-dormant in Phase 6
   (`GetOCPDeviceType` inlined to 0, `TODO(WP7)`; `RelCalc` currently aborts
   #52902). Delivers the `Fault` element that FaultStudy (sub-block 6) needs.
3. **DER third** (largest) — PVSystem/Storage/InvBasedPCE then their controls
   (InvControl/ExpControl/real StorageController). Highest corpus-coverage payoff.
4. **Harmonics**, **Dynamics**, **Faultstudy/AutoAdd** last — the cross-cutting
   **solve modes** that exercise the elements landed above.

The six sub-blocks are independently gated, so this order can flex (e.g. pull DER
earlier for corpus coverage) without breaking the plan; WP boundaries are the
flex points. **GIC elements (`GICLine`/`GICsource`/`GICTransformer`) are Phase 9,
not Phase 7** (PORTING_PLAN §Phase 9) — only the Transformer's existing
low-frequency GIC branch (<0.51 Hz), which harmonics touches, is in scope here.

## 1. Phase target and gate

**Overall gate** (every sub-block adds to it; all must stay green):

- The **standard three-command gate** (`cargo fmt --all --check`; `cargo clippy
  --workspace --all-targets -- -D warnings`; `cargo test --workspace`) green
  after every WP, with **all** prior golden tests (slice, feeders,
  feeders_controls, phase4, phase5, phase6, checkpoints, ieee8500, reliability,
  allocation, gendispatcher, autoadd_reduce, props, corpus_manifest) still green.
- **`props_roundtrip`** extended with default + edited dumps for **every** new
  class (≈18 new classes: WireData, CNData, TSData, CableData, LineSpacing,
  LineGeometry, Fault, Fuse, Recloser, Relay, SwtControl, PVSystem, Storage,
  InvControl, ExpControl, IndMach012, VCCS, UPFC, UPFCControl, VSConverter,
  ESPVLControl, DynamicExp).
- **Two-tier numeric gate per sub-block** (the established Phase-4/5/6 pattern):
  1. **Targeted golden** — `tools/golden/gen_phase7.py` → `tests/golden/phase7/
     <scenario>.json` (command-replay like phase5/phase6), plus checkpoint
     scenarios (`gen_checkpoints.py`) for the new solve modes so an assembled-
     model bug fails at the step/entry it appears, not as downstream drift.
  2. **Live corpus growth** — as each class/mode lands, migrate the corpus cases
     it unblocks from `skipped_unsupported`/`skipped_needs_investigation` into
     `solvable_now` (via `DSS_LIVE_CLASSIFY=1 corpus_live_classify` +
     `tools/corpus/apply_classify.py`), so the **live** Rust-vs-oracle gate
     (`DSS_LIVE_ORACLE=1`) proves them against the oracle full-model, per step.
     `tests/corpus/COVERAGE.md` records the burn-down. This is the *systematic*
     Phase-7 gate; the targeted goldens are the *focused* regression guards.

**Per-sub-block focused gates** (PORTING_PLAN §Phase 7):

1. **Line constants** — a geometry/wiredata-based corpus feeder (e.g. an IEEE
   34-bus or a `Test/`-tree LineGeometry case; exact case chosen by probing the
   corpus + oracle at WP open) compiles, solves, and matches the oracle: node
   order exact, voltages/powers/currents 1e-6 rel, iteration count exact, and the
   **Line YPrim** from the geometry path matches entry-by-entry (the Carson Z/Yc
   is the new math under test). Both the snapshot and a harmonic-frequency
   recompute of the geometry matrices are checked (the latter folds into WP7.6).
2. **DER** — InvControl + Storage + PVSystem test scripts from the corpus (the
   `Test/InvControl*`, `Test/Storage*`, `Test/PVSystem*` families): voltages and
   element powers 1e-6 rel, **inverter control discrete state** (curve region,
   on/off, kVA-limit clamp) exact, **Storage %stored / state** trajectory exact
   over a daily run, StorageController dispatch decisions event-log-equal.
3. **Protection** — a fault + protection sequence (recloser/relay/fuse trip +
   reclose), **event logs equal** (normalized, line-for-line, like the Phase-5
   control gate), final switch/recloser states exact, plus the now-live
   `RelCalc` SAIFI/SAIDI on a metered zone *with* OCP devices matching the oracle.
4. **Harmonics** — a harmonics corpus case (the `Test/Harmonics*` /
   `Version8/.../Harmonics` family): per-harmonic node voltages and element
   currents (the distortion outputs) match the oracle 1e-6 rel across the full
   harmonic list; harmonic-mode monitor channels match.
5. **Dynamics** — a dynamics-mode case (IndMach012 and/or generator dynamics):
   **dynamics-mode monitor trajectories** (state variables, mode-3 monitor) match
   the oracle **1e-5 rel** (the looser dynamics tolerance per PORTING_PLAN §Phase
   7), discrete integration step count exact.
6. **Faultstudy/AutoAdd** — `Solve mode=faultstudy` bus Zsc / fault-current
   results match the oracle; AutoAdd mode (if a corpus case exercises it) matches,
   else the mode keeps its documented unsupported-mode error (empirical decision,
   like Phase 6).

## 2. Phase-wide design decisions

### 2.1 Line constants: a Carson engine + frequency-parameterized Line YPrim

`General/LineConstants.pas` (the base `TLineConstants`) computes the series Z and
shunt Yc of a multi-conductor line from geometry via **Carson's equations**;
`OHLineConstants`/`CNLineConstants`/`TSLineConstants`/`CableConstants` specialize
it for overhead, concentric-neutral, tape-shield, and bare cable. Port into a new
**`src/support/line_constants/`** module (mirrors PORTING_PLAN's planned
`general/line_constants/`):

- `mod.rs` — `LineConstants` base: the conductor coordinate arrays (X/Y/radius/
  GMR/Rdc/R60/`NormAmps`), `Get_Zint` (internal impedance / skin effect — the
  truncated-constant `TODO(compat)` discipline applies to any literal µ0/√ etc.),
  the **Carson earth-return** term selected by `EarthModel` (Carson / FullCarson /
  Deri — the enum already exists on Line from Phase 4), `Calc(freq)` →
  `Zmatrix`/`YCmatrix`, and the `Kron`-reduce-to-phase-conductors path (reuse
  `support/cmatrix.rs`'s Kron, which already carries the unchecked-pivot
  `TODO(compat)`). **Frequency is a parameter** (`Calc(f)`): power flow uses the
  base frequency; harmonics (WP7.6) re-`Calc` per harmonic.
- `oh.rs`/`cn.rs`/`ts.rs`/`cable.rs` — the four specializations.

New catalog classes under `src/elements/general/`:

- `conductor_data.rs` — `ConductorData` base (`TConductorData`: Rdc, R60/Rac,
  GMR, radius, diameter, `NormAmps`/`EmergAmps`, unit reconciliation via the
  existing `LineUnits`) with **`WireData`**, **`CNData`** (concentric neutral:
  `DiaCable`/`DiaStrand`/`kStrand`/`RStrand`/`GmrStrand`), **`TSData`** (tape
  shield: `DiaShield`/`TapeLayer`/`TapeLap`) as the concrete classes; `CableData`
  is the shared cable base for CN/TS.
- `line_spacing.rs` — `LineSpacing` (`TLineSpacingObj`: per-conductor X/H arrays,
  `nconds`/`nphases`, units).
- `line_geometry.rs` — `LineGeometry` (`TLineGeometryObj`, 1019 lines: the
  `cond=`/`wire=`/`cncable=`/`tscable=`/`spacing=`/`x=`/`h=` editing state
  machine, `NConds`/`NPhases`/`reduce`, `AssignFrequency`, `CalcMatrices` →
  drives a `LineConstants` engine and caches `Zmatrix`/`YCmatrix`/`Rho`).

Then un-`NOT_PORTED` Line's geometry path (`elements/pd/line.rs`): the
`FetchGeometryCode`/`FetchLineSpacing`/`FetchWireList`/`FetchCNCableList`/
`FetchTSCableList` resolvers (the snapshot-clone ObjectRef pattern, WP4.2/WP5.3)
copy the geometry/spacing/conductor objects into the Line, and `RecalcElementData`
calls `CalcMatrices(BaseFrequency)` to populate the Line's Z/Yc → existing YPrim
path. **No solve-loop change** — the geometry just becomes another way to fill the
Line impedance that already flows through Phase-3 machinery.

### 2.2 Protection & OCP: join the control sweep, activate the dormant reliability path

`PDElements/Fault.pas` → `src/elements/pd/fault.rs`: a PD element (a shunt/series
fault admittance) with `Randomize`/temporary-fault behavior. It is a normal PD
element for power flow **and** the object FaultStudy mode (WP7.9) drives.

The four protection **controls** — `Controls/Relay.pas` (2353),
`Controls/Recloser.pas` (738), `PDElements/fuse.pas` (537),
`Controls/SwtControl.pas` (395) — are `ControlElem`s that **join the existing
WP5.7 control sweep** with zero new dispatch machinery: each monitors a PD
terminal, evaluates a `TCC_Curve` (already ported) or voltage/time logic, and
queues an OPEN/CLOSE action on the controlled element's terminal (the
conductor-open machinery exists from CapControl's `set_closed`). Relay has many
sub-types (overcurrent 50/51, voltage 27/59, reverse-power 32, neg-seq 46/47,
distance 21/TD21, generic) — port the dispatch verbatim; gate event-log equality
(protection sequences are discrete + logged, exactly like Phase-5 controls).

**Activate the Phase-6 reliability path:** Relay/Recloser/Fuse/SwtControl set
`Flg.HasOCPDevice` and implement `GetOCPDeviceType` (currently inlined to `0`
with `TODO(WP7)` in `solution/meters.rs`); the SAIFI/SAIDI/section logic in
`CalcReliabilityIndices` (ported verbatim in Phase 6 but unreachable — `RelCalc`
aborts #52902 when no OCP device exists) becomes live. The WP7.2 reliability gate
re-runs the Phase-6 `meter_zone_micro` extended with an OCP device.

### 2.3 DER: InvBasedPCE base + PVSystem/Storage as PC elements; controls over them

`PCElements/InvBasedPCE.pas` (268) is the **shared inverter base** for PVSystem
and Storage — port as a shared `InvBasedPceData` struct + trait the way
`PcElementData` factors Generator/Load (`InverterON`, `CutIn`/`CutOut`,
watt/var-priority, `kVA_exceeded` limiting, the smooth/`VarFollowInverter` logic).

`PVsystem.pas` (2735) and `Storage.pas` (3556) are PC elements built on the
**Generator template** (the WP6.2 injection-model architecture):
`SetNominalPVSystem`/`SetNominalStorage`, `CalcYPrimMatrix`/`CalcYPrim`, the
power-flow injection (`DoConstantPQPV`/`DoConstantPQStorage` + the inverter
clamp), registers + `TakeSample`, and the shape refs (irradiance/temperature/
PT-curves for PV via XYcurve already ported; daily/duty/yearly via the
snapshot-clone pattern). **Storage carries integrated state** (`kWhStored`/
`%stored`, charge/idle/discharge state machine) advanced by `UpdateStorage` in
the time-step cleanup hook (Phase-6 hook exists). Both feed:

- the EnergyMeter zone allow-list — flip `solution/meters.rs::is_zone_pce`'s
  `TODO(WP7)` to admit PVSystem/Storage once they exist;
- the StorageController fleet — the WP6.8 skeleton's `MakeFleetList` (empty →
  37201) gets a real fleet; port the dispatch modes (PeakShave/Follow/Support/
  Schedule/Time/…) and join the control sweep.

`Controls/InvControl.pas` (3586) and `Controls/ExpControl.pas` (749) are controls
over the inverter PCEs (volt-var / volt-watt / DRC / watt-pf / AVR via XYcurve
curves; InvControl uses **rolling-average windows** — port `Controls/
RollAvgWindow.pas`). They join the control sweep; their multi-element fleet
borrow is the WP5.7 `pair_mut`/`triple_mut` pattern. **InvControl is the single
largest unit in the phase** — budget it as its own WP (WP7.5) and gate on the
`Test/InvControl*` corpus family for discrete control-region equality.

### 2.4 Harmonics: a frequency-sweep solve mode + per-element Norton injection

`SolveMode::Harmonic`/`HarmonicT` currently fall through to the "Unknown solution
mode" error; `set_mode` already carries the `OK_for_Harmonics` guard
(`solution.rs:359-380`) and `harmonic = 1.0 // TODO(phase7)`. Port
`SolutionAlgs.SolveHarmonic`(`T`):

- `InitializeForHarmonics` per element: each source/PC element computes its
  harmonic Norton-equivalent injection from its `Spectrum` (object already
  ported) scaled to the **fundamental** solution captured first (VSource, Isource,
  Load, Generator, PVSystem, Storage each have a harmonic `GetInjCurrents`
  branch). Port the per-element `InitHarmonics` + harmonic injection.
- The system Y is **rebuilt at each harmonic frequency**: lines via the
  frequency-parameterized `CalcMatrices(f)` (WP7.1), transformer/reactor/
  capacitor frequency scaling, and the Transformer GIC <0.51 Hz branch (the one
  Phase-6 GIC deferral that lands here). `Solution.Frequency`/`HarmonicList`
  drive the sweep.
- Monitor harmonic-mode sampling (the per-sample record stores Freq/Harmonic
  instead of hour/sec — Phase 6 deferred the harmonic monitor header/body).
- Gate: per-harmonic voltages/currents (distortion outputs) vs the oracle.

### 2.5 Dynamics: DynaVars integration loop + per-element state machinery

`SolveMode::Dynamic` currently errors; `set_mode`'s `OK_for_Dynamics` guard and
`support/dynamics.rs` (DynaVars: `h`, `t`, `IntegrationMethod`, `iteration`)
exist. Port `SolutionAlgs.SolveDynamic`: the predictor/corrector step loop
(`SolveDynamicStep`/`IterativeSolution`) over `DynaVars.h` with the integration
method (Euler/Trapezoidal/Gear), driving:

- per-element `InitStateVars`/`IntegrateStates`/`CalcDynamic`/`StateVars` —
  ported for Generator (deferred from Phase 6: §2.5 of PHASE6_PLAN), Storage,
  PVSystem (the dynamics state set Monitor mode 3 consumes), and the
  **dynamics-only elements**: `IndMach012.pas` (1482 — the double-cage induction
  machine, the canonical dynamics test element), `vccs.pas` (861), `UPFC.pas`
  (1181) + `UPFCControl.pas` (290), `VSConverter.pas` (498),
  `ESPVLControl.pas` (461).
- `General/DynamicExp.pas` (591, user-defined dynamic expressions) + its
  evaluator, and `PCElements/DynEqPCE.pas` (273, the base PCE that integrates a
  `DynamicExp`) — a small expression interpreter over the existing RPN machinery
  (`dss-parser/src/rpn.rs`).
- Monitor mode 3 (state variables) gets its real sample body (Phase 6 stubbed it
  to names/count); gate compares dynamics-mode trajectories at **1e-5 rel**.
- `MakePosSequence` (deferred everywhere since Phase 6) is ported on demand where
  a dynamics-init path needs it.

Split across two WPs: **WP7.7** (the solve loop + Generator/Storage/PVSystem
state vars + IndMach012 + DynamicExp/DynEqPCE — the core machinery) and **WP7.8**
(VCCS/UPFC/UPFCControl/VSConverter/ESPVLControl — the converter/FACTS family).

### 2.6 Faultstudy / AutoAdd / Feeder

- **FaultStudy** (`SolveMode::FaultStudy`): port `SolutionAlgs.SolveFaultStudy` +
  the bus short-circuit machinery (`Circuit.ComputeYsc`/`ComputeAllYsc`, bus Zsc
  via per-bus current injection through the existing solver). Needs the Fault
  element (WP7.2). Gate: bus Zsc / fault currents vs oracle.
- **AutoAdd** (`circuit/auto_add.rs` skeleton from Phase 6): the capacity-search
  solve mode using EnergyMeter registers + aux-current injection
  (`UseAuxCurrents`). Port the solve integration **only if** a corpus case
  exercises it (empirical decision, like Phase 6) — else keep the documented
  unsupported-mode error.
- **MonteCarlo / LoadDuration** modes (`Monte1/2/3`, `LD1/2`, `MonteFault`): not
  called out in the PORTING_PLAN sub-blocks; port only if a corpus case needs
  them, else keep the unsupported-mode error + a STATUS note.
- **Feeder.pas**: largely dead upstream (Phase 6 found `DoFeederStuff` remnants
  dead). Port only what a corpus case needs; otherwise document as dead.

### 2.7 Reused architecture (no re-explanation in the WPs)

- **PC-element injection** — Generator (WP6.2) is the template for PVSystem/
  Storage/IndMach012/DynEqPCE.
- **Control sweep** — Relay/Recloser/Fuse/SwtControl/InvControl/ExpControl/real
  StorageController dispatch through `ElemStore::{obj,pair_mut,triple_mut}` +
  `as_any_mut` + `CtrlCtx` (WP5.7), exactly like RegControl/CapControl/
  GenDispatcher.
- **ObjectRef snapshot-clone** — geometry/spacing/conductor fetch and all DER
  shape refs (WP4.2/WP5.3 `FetchLineCode` pattern).
- **`define_properties!`** for every new class's prop table.
- **Gates** — targeted goldens (`gen_phase7.py`) + checkpoint scenarios +
  live-corpus manifest growth; tolerance policy in `tests/TOLERANCE_NOTES.md`
  (any new field-specific exception, e.g. the 1e-5 dynamics class, is recorded
  there — no blanket relaxation).
- **`TODO(compat)`/`NOT_PORTED`** discipline; **probe the oracle** for every new
  behavior question (`tools/golden/probe_val.py` pattern).

## 3. Work packages

> Effort % = share of Phase 7. Each WP ends gate-green with its targeted golden +
> props dumps + (where the sub-block lands a class/mode) a live-corpus manifest
> migration. Pascal line numbers confirmed at WP open.

---

### WP7.1 — Line constants & geometry [9%]

**Pascal:** `General/{ConductorData,WireData,CNData,TSData,CableData,LineSpacing,
LineGeometry,LineConstants,OHLineConstants,CNLineConstants,TSLineConstants,
CableConstants}.pas`; `PDElements/Line.pas` geometry fetch path.

Steps:
1. `support/line_constants/` — `LineConstants` base (Carson earth-return per
   `EarthModel`, `Get_Zint` skin effect, `Calc(freq)` → Z/Yc, Kron reduce) +
   `oh`/`cn`/`ts`/`cable` specializations. Unit tests vs oracle-probed Z/Yc on a
   known 3-wire overhead geometry (transcribe `LineGeometry.Zmatrix`/`YCmatrix`).
2. `conductor_data.rs` (ConductorData base + WireData/CNData/TSData; CableData
   base), `line_spacing.rs`, `line_geometry.rs` — props via `define_properties!`,
   edit state machines, `MakeLike`; `props.json` scenarios.
3. `line.rs`: un-`NOT_PORTED` `geometry`/`spacing`/`wires`/`cncables`/`tscables`;
   the fetch resolvers + `RecalcElementData` `CalcMatrices(BaseFrequency)`.
4. Gate: the geometry/wiredata corpus feeder (probe-selected) matches the oracle
   (§1 focused gate 1); migrate the `unsupported_class={WireData,LineGeometry,…}`
   corpus cases into `solvable_now`; targeted golden `phase7/line_geometry*.json`.

---

### WP7.2 — Protection: Fault + Fuse/Recloser/Relay/SwtControl + OCP/reliability activation [12%]

**Pascal:** `PDElements/Fault.pas`, `PDElements/fuse.pas`, `Controls/Recloser.pas`,
`Controls/Relay.pas`, `Controls/SwtControl.pas`; `Meters/EnergyMeter.pas`
`CalcReliabilityIndices` + `GetOCPDeviceType`/`HasOCPDevice` (Phase-6 dormant).

Steps:
1. `pd/fault.rs` — `TFaultObj` (admittance, `Randomize`, temporary-fault/
   `MinAmps` reset). Props + YPrim + `props.json`. (Also the FaultStudy input.)
2. `pd/fuse.rs` + `control/{recloser,relay,swt_control}.rs` — each a `ControlElem`
   on the WP5.7 sweep: `Sample` (TCC/voltage/time/reverse-power/… logic per type)
   + `DoPendingAction` (OPEN/CLOSE the controlled terminal) + event-log lines.
   Relay's sub-type dispatch ported verbatim. Inline tests vs a mock monitored
   element + the oracle.
3. **Activate reliability:** set `Flg.HasOCPDevice`; implement `GetOCPDeviceType`
   (replace the `TODO(WP7)` inlined 0); `RelCalc` no longer aborts when an OCP
   device is present — the Phase-6 SAIFI/SAIDI/section math goes live.
4. Gate: a fault+protection sequence — **event logs equal** (normalized), final
   recloser/switch states exact; `RelCalc` SAIFI/SAIDI on a metered zone with an
   OCP device vs oracle; targeted golden `phase7/protection*.json`; migrate the
   `unsupported_class={Fault,Fuse,Recloser,Relay,SwtControl}` corpus cases.

---

### WP7.3 — DER A: InvBasedPCE base + PVSystem [12%]

**Pascal:** `PCElements/InvBasedPCE.pas`, `PCElements/PVsystem.pas`.

Steps:
1. `pc/inv_based_pce.rs` — `InvBasedPceData` + trait (inverter on/off, CutIn/
   CutOut, watt/var priority, `kVA_exceeded` clamp, VarFollowInverter).
2. `pc/pvsystem.rs` — `TPVsystemObj` on the Generator template:
   `SetNominalPVSystem`, P-T-V curves (XYcurve) + irradiance/temperature shapes
   (snapshot-clone), `CalcYPrim`, `DoConstantPQPV` + inverter clamp, registers +
   `TakeSample`. Inline tests vs oracle (per-model, transcribed).
3. Flip `solution/meters.rs::is_zone_pce` `TODO(WP7)` to admit PVSystem.
4. Gate: `Test/PVSystem*` corpus migration + targeted golden
   `phase7/pvsystem*.json` (snapshot + daily); voltages/powers 1e-6, inverter
   state exact; `props.json`.

---

### WP7.4 — DER B: Storage + StorageController behavior [14%]

**Pascal:** `PCElements/Storage.pas`, `Controls/StorageController.pas` (the
WP6.8 skeleton → real behavior).

Steps:
1. `pc/storage.rs` — `TStorageObj` on the inverter base: the charge/idle/
   discharge **state machine** + integrated `kWhStored`/`%stored` advanced in the
   time-step cleanup hook (`UpdateStorage`), `DoConstantPQStorage`, dispatch by
   `%charge`/`%discharge`/shape, registers + `TakeSample`.
2. `control/storage_controller.rs` — replace the skeleton: real `MakeFleetList`
   (now non-empty), the dispatch modes (PeakShave/Follow/Support/Schedule/Time/…),
   `Sample`/`DoPendingAction` on the control sweep; remove the empty-fleet 37201
   `NOT_PORTED` path.
3. Gate: `Test/Storage*` corpus migration + targeted golden `phase7/storage*.json`
   — **%stored / state trajectory exact** over a daily run, StorageController
   dispatch event-log-equal; `props.json`.

---

### WP7.5 — DER C: InvControl + ExpControl [11%]

**Pascal:** `Controls/InvControl.pas` (3586 — the largest unit in the phase),
`Controls/ExpControl.pas`, `Controls/RollAvgWindow.pas`.

Steps:
1. `control/roll_avg_window.rs` — the rolling-average window helper.
2. `control/inv_control.rs` — `TInvControlObj`: the control modes (VOLTVAR /
   VOLTWATT / DRC / VV_DRC / WATTPF / WATTVAR / AVR) via XYcurve curves + the
   rolling-average windows; `Sample`/`DoPendingAction` over the PVSystem/Storage
   fleet (pair/triple borrow). 
3. `control/exp_control.rs` — `TExpControlObj` (dynamic reactive-power control).
4. Gate: `Test/InvControl*` corpus migration + targeted golden
   `phase7/invcontrol*.json` — **discrete control-region / clamp state exact**,
   voltages/vars 1e-6; `props.json`. **Do not let InvControl balloon** — port the
   mode dispatch verbatim, probe the oracle for region boundaries.

---

### WP7.6 — Harmonics mode [8%]

**Pascal:** `Common/SolutionAlgs.pas` `SolveHarmonic`/`SolveHarmonicT`;
per-element `InitHarmonics`/harmonic `GetInjCurrents` (VSource, Isource, Load,
Generator, PVSystem, Storage); `General/Spectrum.pas` (object done);
`Common/Solution.pas` `Frequency`/`HarmonicList`.

Steps:
1. `solution/solution.rs`: wire `SolveMode::Harmonic`/`HarmonicT` →
   `solve_harmonic` (frequency sweep over the harmonic list; capture the
   fundamental first); resolve the `harmonic = 1.0 // TODO(phase7)`.
2. Per-element `InitHarmonics` + harmonic Norton injection from `Spectrum`;
   frequency-dependent Y rebuild (line `CalcMatrices(f)` from WP7.1; transformer/
   reactor/capacitor scaling; Transformer GIC <0.51 Hz branch).
3. Monitor harmonic-mode sample body (Freq/Harmonic record) + header.
4. Gate: harmonics corpus migration + targeted golden `phase7/harmonics*.json` +
   a checkpoint scenario (per-harmonic assembled Y/V); distortion outputs 1e-6.

---

### WP7.7 — Dynamics core: solve loop + Generator/Storage/PV state vars + IndMach012 + DynamicExp/DynEqPCE [14%]

**Pascal:** `Common/SolutionAlgs.pas` `SolveDynamic`; `Common/Solution.pas`
`SolveDynamicStep`/`IterativeSolution`; `Common/Dynamics.pas` (DynaVars, done as
support); per-element `InitStateVars`/`IntegrateStates`/`CalcDynamic`/`StateVars`
(Generator, Storage, PVSystem); `PCElements/IndMach012.pas`,
`General/DynamicExp.pas`, `PCElements/DynEqPCE.pas`.

Steps:
1. `solution/solution.rs`: `SolveMode::Dynamic` → `solve_dynamic` (the
   predictor/corrector step loop over `DynaVars.h`; integration method dispatch).
2. Generator/Storage/PVSystem dynamics state machinery (the Phase-6-deferred
   §2.5 set) + Monitor mode 3 real sample body.
3. `pc/ind_mach012.rs` — the induction machine (the canonical dynamics test
   element). `general/dynamic_exp.rs` + `pc/dyneq_pce.rs` — the dynamic-expression
   evaluator over `dss-parser` RPN + the integrating base PCE.
4. Gate: a dynamics corpus case (IndMach012/generator dynamics) — **mode-3
   monitor trajectory 1e-5 rel**, step count exact; targeted golden
   `phase7/dynamics*.json`; `props.json` for IndMach012/DynamicExp.

---

### WP7.8 — Dynamics elements: VCCS, UPFC + UPFCControl, VSConverter, ESPVLControl [9%]

**Pascal:** `PCElements/vccs.pas`, `PCElements/UPFC.pas` + `Controls/
UPFCControl.pas`, `PCElements/VSConverter.pas`, `Controls/ESPVLControl.pas`.

Steps:
1. `pc/vccs.rs` (voltage-controlled current source), `pc/upfc.rs` +
   `control/upfc_control.rs`, `pc/vs_converter.rs`, `control/espvl_control.rs` —
   each on the established PC-element/control-sweep templates with its dynamics
   state vars.
2. Gate: corpus migration for each (the `Test/` families) + targeted goldens;
   `props.json` for all five classes. These are smaller/self-contained relative
   to WP7.7's machinery — most of the dynamics solve work is already done there.

---

### WP7.9 — Faultstudy + AutoAdd modes + Feeder [7%]

**Pascal:** `Common/SolutionAlgs.pas` `SolveFaultStudy`/`SolveAutoAdd`;
`Common/Circuit.pas` `ComputeYsc`/`ComputeAllYsc`; `Common/AutoAdd.pas` (skeleton
exists, `circuit/auto_add.rs`); `Common/Feeder.pas` (mostly dead).

Steps:
1. `SolveMode::FaultStudy` → `solve_fault_study` + bus Zsc machinery
   (`ComputeAllYsc`). Uses the Fault element (WP7.2). Gate: bus Zsc / fault
   currents vs oracle (a `Test/`/`Distrib` faultstudy case).
2. AutoAdd solve integration **only if** a corpus case needs it (probe) — else
   keep the documented unsupported-mode error. MonteCarlo/LoadDuration likewise.
3. Feeder: port only what a corpus case needs; else document dead.

---

### WP7.10 — Phase exit [4%]

1. `rg "TODO\(compat\)"` / `rg "NOT_PORTED"` / `rg "TODO\(WP7\)"` sweep — every
   remaining site points at its phase (Phase 8 reporting/exports/Save; Phase 9
   GIC/CIM/A-Diakoptics; or "never" for DLLs).
2. Re-run everything: props, slice, feeders, feeders_controls, phase4, phase5,
   phase6, phase7, checkpoints, ieee8500, reliability, allocation, gendispatcher,
   autoadd_reduce, corpus_manifest — all green. Run the **live corpus gate**
   (`DSS_LIVE_ORACLE=1`) with the pinned oracle; refresh
   `tests/corpus/COVERAGE.md` (the Phase-7 burn-down: many `skipped_unsupported`
   classes now `solvable_now`).
3. Update `PORTING_PLAN.md` §Phase 7 cross-links if needed; rewrite `STATUS.md`
   (Phase 7 record; "next = Phase 8, write `PHASE8_PLAN.md` first").
4. Merge to `main` (`--no-ff`, the per-phase convention) — **only on explicit
   user request**.

## 4. Deferred in this phase (pointing forward)

- **Phase 8** (reporting/exports/Save): all `Show`/`Export`/`Dump` text + CSV
  outputs for the new classes (fault report, fault-study export, harmonics/
  dynamics exports, monitor `Save`/`TranslateToCSV`), `Circuit.Save` of the new
  classes, the `SystemMeter` register core + demand-interval files (still Phase 8
  from Phase 6), the long-tail ExecHelper verbs.
- **Phase 9** (exotics): **GIC elements** (`GICLine`/`GICsource`/
  `GICTransformer` — explicitly Phase 9 per PORTING_PLAN; only the Transformer's
  <0.51 Hz GIC branch is touched here for harmonics), `ExportCIMXML`,
  A-Diakoptics + parallel-machine actor mode, `Pstcalc` flicker (Monitor mode 4),
  plot callbacks.
- **Never** (safe-Rust): all user-model DLL loading — `GenUserModel`,
  `PVSystemUserModel`, `StoreUserModel`, `CapUserControl`,
  Relay/Recloser user-model hooks (each a hard `NOT_PORTED`).
- **On demand:** `MakePosSequence` (ported only where a Phase-7 dynamics init
  needs it; otherwise still deferred everywhere); the binary shape file inputs
  (`SngFile`/`DblFile`/`PQCSVFile`) carried from Phase 5; AutoAdd/MonteCarlo/
  LoadDuration solve modes kept as unsupported-mode errors unless a corpus case
  forces them (the WP7.9 empirical decision).
