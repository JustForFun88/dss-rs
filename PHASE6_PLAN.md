# Phase 6 — Detailed Execution Plan: Meters, Monitors, Topology, Generator; 8500-Node Gate

> Companion to `PORTING_PLAN.md` §Phase 6; same status and rules of engagement as
> `PHASE4_PLAN.md` §0 / `PHASE5_PLAN.md` (Pascal is the spec; probe the oracle,
> never guess; mark `TODO(compat)` / `NOT_PORTED`; goldens manual-only with the
> pinned oracle; gate-green per WP; commit only on request). **Prerequisite:
> Phase 5 complete and merged** — this phase consumes the control loop, the
> time-series modes (`solve_daily`/`solve_yearly`/`solve_duty` with their
> `sample_all_monitors_and_meters` / `end_of_time_step_cleanup` no-op hook stubs
> at `solution.rs:737-762`), the `ElemStore::{obj,pair_mut,triple_mut}` +
> `as_any_mut` dispatch pattern, and the shape machinery without re-explaining
> them.
>
> **Stop-and-confirm cadence (same as PHASE4_PLAN §0.8):** after each small step
> (a WP or self-contained sub-step), run the full gate, **update `STATUS.md`**,
> then stop and wait for the user's explicit confirmation before the next step —
> unless the user has explicitly authorized executing multiple WPs in one pass.

## 1. Phase target and gate

**Scope** (PORTING_PLAN §Phase 6): `Meters/MeterElement.pas`,
`Meters/EnergyMeter.pas` (zones, registers, SAIFI/SAIDI), `Meters/Monitor.pas`,
`Meters/Sensor.pas`, `Shared/CktTree.pas` + circuit topology
(`Circuit.GetTopology`, bus adjacency lists), `Meters/ReduceAlgs.pas` (basic),
`PCElements/generator.pas` (power-flow models; dynamics machinery → Phase 7;
user-model DLLs → never), `Common/AutoAdd.pas`, `Controls/GenDispatcher.pas`,
`Controls/StorageController.pas` **skeleton** (Storage element is Phase 7).

**Gate** (all must pass):

1. **`golden_ieee8500.rs`** vs new `tests/golden/ieee8500.json`: compile the
   unmodified `Version8/Distrib/IEEETestCases/8500-Node/Master.dss`, then
   (per `Run_8500Node.dss`) `New Energymeter.m1 Line.ln5815900-1 1`,
   `Set Maxiterations=20`, `Solve`:
   - converged + total iteration count **exact**; `YNodeOrder` **exact**;
   - node voltages 1e-6 rel; total power + losses 1e-6 rel;
   - final transformer taps / RegControl tap numbers / capacitor states
     **exact** (the 8500 has 4 regulator banks + 10 CapControls — this is the
     controls-at-scale regression);
   - EnergyMeter `m1` register values 1e-4 rel after a meter-sampled solve
     (registers are integration results — the looser tolerance per
     PORTING_PLAN §4 policy); register **names** exact;
   - per-element powers/currents are **not** dumped for 8500 (golden would be
     enormous; the three IEEE feeders already pin that machinery) — instead
     the meter registers pin the zone aggregation over all ~6k branches;
   - wall-clock solve time recorded in STATUS.md (initial budget: within 5×
     of dss_capi on the same machine).
2. **`golden_phase6.rs`** vs new `tests/golden/phase6.json`
   (`tools/golden/gen_phase6.py`, command-replay style like phase5.json):
   - `monitor_daily_ieee13`: IEEE13 (inline, controls on) + mode-0 and mode-1
     monitors on `line.671680` + a mode-2 monitor on the regulator transformer
     + a mode-5 monitor; 24 daily steps; compare **per-channel sample arrays
     elementwise** (f32; 1e-6 rel on the f32 values) + `SampleCount` + header
     strings exact;
   - `meter_daily_ieee13`: IEEE13 + `energymeter.m1` on `line.650632` +
     24 daily steps with a load shape; registers 1e-4 rel, register names
     exact, zone branch/load counts exact (`DI_Verbose`-style scalars read via
     the API);
   - `generator_snap`: IEEE13 + two generators (model 1 PQ + model 3 PV,
     wye + delta); snapshot solve; voltages 1e-6, generator powers 1e-6,
     iteration count exact;
   - `meter_zone_micro`: a hand-built ~6-branch radial with a sub-meter
     (second EnergyMeter mid-feeder) — zone membership lists exact
     (`ZonePCE`/branch sequence via the API), `IsDangling`/loop flags pinned;
   - `sensor_props` + `gendispatcher_props` + `storagecontroller_props`
     scenarios live in `props.json` (gen_props.py), not here.
3. `props.json` additions (gen_props.py): Generator, Monitor, EnergyMeter,
   Sensor, GenDispatcher, StorageController default + edited dumps;
   `props_roundtrip` green.
4. Standard repo gate green; **all** earlier golden tests stay green
   (slice, feeders, feeders_controls, phase4, phase5, props).

## 2. Phase-wide design decisions

### 2.1 Topology data: CktTree as an index arena; flags on the elements

`Shared/CktTree.pas` is a pointer-linked n-ary tree with per-node shunt-object
lists plus a stack-based `GoForward` traversal (l.247-299) and a `ZoneEndsList`.
Rust port (`src/circuit/ckt_tree.rs`):

- `CktTree { nodes: Vec<TreeNode>, stack: Vec<usize>, zone_ends: Vec<(usize, i32)> }`
  with `TreeNode { elem: ElemRef, parent: Option<usize>, children: Vec<usize>,
  shunts: Vec<ElemRef>, from_bus: i32, to_bus: i32, from_terminal: i32,
  is_looped: bool, is_parallel: bool, is_dangling: bool, loop_elem:
  Option<ElemRef>, volt_base_index: i32 }` — every Pascal pointer becomes
  `ElemRef`/index. Port `AddNewChild` (l.223), `AddNewObject` (l.239),
  `PushAllChildren`+`GoForward` (l.247/264) **verbatim** — the traversal order
  defines `SequenceList`, which is observable (reliability sweep order, zone
  dumps, reductions).
- **Bus adjacency lists** (`BuildActiveBusAdjacencyLists`, CktTree.pas l.678):
  `Vec<Vec<ElemRef>>` indexed by bus ref — `adj_pd` gets enabled non-shunt PD
  elements per terminal bus; `adj_pc` gets enabled PC elements **plus shunt PD
  elements** (the Pascal puts `IsShunt` capacitors/reactors on the PC list).
  Built at the executive level where the registry is readable; owned by the
  EnergyMeter class-level state (Pascal keeps them on `TEnergyMeter`,
  l.239-240) and freed after zone building (`FreeTopology` semantics).
- **Element flags**: Pascal `Flg.Checked / IsIsolated / HasEnergyMeter /
  HasSensorObj / HasOCPDevice / ...` + per-terminal `TerminalsChecked[]`.
  Add a `flags: ElemFlags` bitflags field + `terminals_checked: Vec<bool>` to
  `CktElementData` (PC and PD both need them — Pascal keeps them on
  `TDSSCktElement`).
- **Zone/reliability fields** (Pascal `TPDElement`): `from_terminal`,
  `parent_pd: Option<ElemRef>`, `meter_obj: Option<ElemRef>`,
  `sensor_obj: Option<ElemRef>`, `branch_num_customers`,
  `branch_total_customers`, plus the reliability inputs `fault_rate`,
  `pct_perm`, `hrs_to_repair` (already parsed on Line? — verify; add the
  props where missing) and accumulators (`AccumulatedBrFltRate`,
  `MilesThisLine`, …). These go on `CktElementData` too (PC elements carry
  `meter_obj`/`sensor_obj` as well — `MakeMeterZoneLists` sets them on loads).
- `MakeMeterZoneLists` (EnergyMeter.pas l.1773) runs **at the executive level**
  (it reads every element + mutates flags + the meter): implement as a free
  function over `&mut dyn ElemStore`-style registry access exactly like the
  WP5.7 control dispatcher, not as a method trying to hold `&mut self` and the
  world. Trigger points: Pascal fires it from `DoResetMeterZones`
  (Circuit.pas l.2145) which runs inside `ProcessBusDefs`-adjacent re-builds —
  port the call chain: `BuildYMatrix` → (buses rebuilt) → `DoResetMeterZones`
  → `ResetMeterZonesAll` (EnergyMeter.pas l.797: clears flags/refs on all
  elements, rebuilds adjacency, walks meters **in creation order**, then
  `SetHasMeterFlag` l.1752).

### 2.2 Sample hooks: fill the Phase 5 stubs, exact call sites

`SolutionAlgs.pas` `FinishTimeStep` (l.74) and the per-mode loops call, in
order: `MonitorClass.SampleAll` → (if `SampleTheMeters`)
`EnergyMeterClass.SampleAll` → `EndOfTimeStepCleanup`. `solve_snap` itself
**also** samples mode-5 monitors per solution (find the `SampleAllMode5` call
sites in `Solution.pas` — port every one). The Phase 5 stubs
(`sample_all_monitors_and_meters`, `end_of_time_step_cleanup` in
`solution.rs:735-762`) get real bodies; **verify against the Pascal which solve
modes sample meters** (`sample_the_meters` is already set per mode from Phase 5
— daily/yearly/duty true, snap false; the meters can also be sampled manually
via the `Sample` action/command).

Class-level sweeps (`ResetAll`, `SampleAll`, `SaveAll` on the meter classes)
walk **enabled** objects in creation order — implement as free functions over
the registry (the WP5.7 pattern), not methods on a class singleton.

### 2.3 Monitor stream: in-memory f32 buffer, byte-faithful

Pascal `MonBuffer: pSingleArray` — **samples are stored as float32** and
written to a `TMemoryStream` whose layout dss-python decodes (`Channel(i)`).
Port as `Vec<f32>` records appended per sample: `[hour: f32, sec: f32,
ch1..chN: f32]` (harmonic mode stores Freq/Harmonic instead — Phase 7). The
f32 quantization is compat-critical: compute in f64, push as `as f32` exactly
where Pascal narrows. The oracle gate reads
`dss.ActiveCircuit.Monitors.Channel(i)` / `dblHour` — match elementwise.
File save (`Save`/`OpenMonitorStream`/`TranslateToCSV`) is Phase 8 (`Show
monitor`/`Export monitor`); the in-memory buffer + `SampleCount` + `Header`
are the product here.

**Modes ported now**: 0 (V&I mag/angle or re/im, ±16 sequence, ±32 magnitude,
±64 pos-seq/avg modifiers, residual, VIpolar/Ppolar), 1 (powers), 2 (tap), 5
(solution vars), 6 (capacitor steps), 9 (losses), 8/10 (transformer winding
currents/voltages), 11 (all terminal V&I), 12 (LL voltages). Mode 3 (state
variables) ports against Generator's `NumVariables/Get_Variable` surface but
returns only what Phase-6 Generator exposes (the dynamics state set is
Phase 7); mode 4 (flicker, needs `Pstcalc`) and mode 7 (Storage) →
`NOT_PORTED`-style sample error or empty per Pascal behavior — check what
Pascal does when the metered element lacks the capability and reproduce
(`ValidMonitor` handling in `RecalcElementData` l.532).

### 2.4 EnergyMeter scope boundaries

In: props 1–24 (incl. the option strings of prop 4/`Mask`/`ZoneList`/
`LocalOnly`), `ResetRegisters`/`Integrate` (l.1205/1271 — trapezoid flag),
`TakeSample` (l.1289 — the full register accumulation: zone kWh, losses split
line/xfmr + sequence + voltage-base buckets, overload EEN/UE via
`Accumulate_Load` l.2208 / `Accumulate_Gen` l.2198, max-demand drag hands),
zone building (§2.1), `TotalUpDownstreamCustomers` (l.1693),
`CalcReliabilityIndices` (l.2411) + the `Relcalc` command
(`ExecHelper`/`ExecOptions` — find the exact command/option spelling),
`GetPCEatZone` (l.2809, feeds the `ZonePCE` API the micro golden uses),
register names incl. `AssignVoltBaseRegisterNames` (l.3082);
`NumEMRegisters = 32 + 5·NumEMVbase` (l.61).

Out (Phase 8 unless a gate needs them): all Demand-Interval/overload/voltage
report **files** (`OpenDemandIntervalFile`/`WriteDemandIntervalData`/
`CloseAllDIFiles`/`AppendAllDIFiles`/`SystemMeter` DI files — the `TSystemMeter`
register accumulation itself **is** in scope since `TotalizeMeters` and the
summary depend on it; check what the oracle exposes), `SaveZone` (Save command),
`ZoneDump` text report, `MakeVPhaseReportFileName`, `WriteOverloadReport`,
`WriteVoltageReport`, `InterpolateCoordinates`/`CalcBusCoordinates` (the
`Interpolate` command — WP6.8 only if cheap), feeder objects (`DoFeederStuff`
remnants).

### 2.5 Generator scope boundaries

In: the full property table (TGeneratorProp 1–43+, generator.pas l.~150;
`UserModel`/`UserData`/`ShaftModel`/`ShaftData` `NOT_PORTED` — no DLLs),
`SetNominalGeneration` (l.1069 — dispatch/status/shape logic mirroring Load's
`SetNominalLoad`), `RecalcElementData` (l.1229), `CalcYPrimMatrix`/`CalcYPrim`
(l.1271/1367), the power-flow injection models `DoConstantPQGen` (l.1505),
`DoConstantZGen` (l.1567), `DoPVTypeGen` (l.1596 — needs the DQDV machinery
l.2229-2260; **read how the standard solution uses it** — the V-control loop
runs inside the generator between iterations), `DoFixedQGen` (l.1655),
`DoFixedQZGen` (l.1713), `DoCurrentLimitedPQ` (l.1799, model 7),
`CalcGenModelContribution`/`CalcInjCurrentArray`/`InjCurrents`
(l.2062/2101/2128), kW/kvar/PF/kVA/MVA property interplay
(`SyncUpPowerQuantities` l.2820, `SetkWkvar` l.2846, `Set_PowerFactor` etc.),
generator registers (6: kWh, kvarh, MaxkW, MaxkVA, Hours, $) with
`TakeSample`/`Integrate`/`ResetRegistersAll`/`SampleAll` (l.2141-2213 — find
where the class `SampleAll` is invoked in the solution loops and port that call
site too), `CheckOnFuel`/fuel props (l.1492 — small, port now), `ForceOn`,
`DebugTrace` (file output → store flag, no file, like RegControl).

Out (Phase 7): `DoDynamicMode`/`DoHarmonicMode`/`InitStateVars`/
`IntegrateStates`/`CalcVthev_Dyn*`/`InitHarmonics` + the state-variable
surface beyond names/count stubs (Monitor mode 3 consumes what exists);
`MakePosSequence` (Phase 6+ convention, still unported everywhere).

### 2.6 What "skeleton" means for AutoAdd / GenDispatcher / StorageController

- **GenDispatcher** (346 lines): small real control — port whole (props,
  `Sample` reads the monitored element's power vs `kWLimit`, dispatches its
  generator list via `GenMultiplier`). It joins the control sweep from WP5.7.
- **StorageController** (1787 lines): the Storage element is Phase 7, so port
  the **property table + object skeleton** only; `Sample`/`DoPendingAction`
  record a Pascal-faithful error (or the documented no-op if Pascal tolerates
  an empty element list — **probe the oracle**: create one without storage
  elements and solve). Mark internals `NOT_PORTED` pointing at Phase 7.
- **AutoAdd** (567 lines): the autoadd solve **mode** machinery (capacity
  search loop using EnergyMeter registers + `Set AddType/GenkW/...` options).
  Port the option parsing + the object; the `Solve` integration only if the
  effort is small after meters land — otherwise the mode keeps its Phase 3
  "unsupported solve mode" error and STATUS.md documents the deferral
  (PORTING_PLAN lists AutoAdd in Phase 6 scope but no gate exercises it; the
  empirical corpus check decides).

## 3. Work packages

---

### WP6.1 — Topology foundations: element flags, bus adjacency, CktTree [8%]

**Pascal:** `Shared/CktTree.pas` (649 lines); `Common/Circuit.pas`
`GetTopology`/`FreeTopology`/`DoResetMeterZones` (l.3034/3056/2145);
the `Flg.*` element flag set (`Common/DSSClass.pas` or `CktElement.pas` —
locate the enum) and `TerminalsChecked`.

Steps:
1. `ElemFlags` bitflags + `terminals_checked` on `CktElementData`; the
   zone/meter fields of §2.1 (PD reliability inputs verified against the
   existing Line/Transformer prop tables — `faultrate`/`pctperm`/`repair`
   exist on PD prop tables since Phase 3/4; wire them to the new fields if
   they were parse-only).
2. `ckt_tree.rs`: arena tree + traversal + `ZoneEndsList` + unit tests
   hand-traced from the Pascal (add-children order, `GoForward` DFS order,
   zone-end registration).
3. Bus adjacency builder (free fn over the registry): enabled PD (non-shunt)
   per terminal bus → `adj_pd`; PC + shunt PD → `adj_pc`. Unit test on a
   micro circuit (line + capacitor + load: capacitor lands on the PC list).
4. `GetIsolatedSubArea`/`GetSourcesConnectedToBus`/`FindAllChildBranches`
   (CktTree.pas l.471-622) — port now only if WP6.4 zone build needs them
   (it uses its own loop); else defer to the topology API consumer
   (`Circuit.GetTopology` for the whole-circuit tree) and note it.
5. No behavior change to any existing gate (pure additions) — full gate green.

---

### WP6.2 — Generator [15%]

**Pascal:** `PCElements/generator.pas` (2550 lines), scope per §2.5.

Steps:
1. Props + ctor defaults + `MakeLike` + connection/ncond side-effect web
   (`SetNcondsForConnection` l.658, the kV/kW/PF/kvar/kVA interplay —
   mirror the Load port's structure).
2. `RecalcElementData` + shapes (reuse WP5.3's snapshot-clone ObjectRef
   pattern for `yearly`/`daily`/`duty`) + `SetNominalGeneration` (dispatch
   modes incl. `DispValue`, `Status=fixed/variable`, ForceOn, the
   loadshape-mode multipliers via `CalcDailyMult`/`CalcDutyMult`/
   `CalcYearlyMult` l.1035-1068).
3. `CalcYPrimMatrix`/`CalcYPrim` + all six power-flow models + the
   compensation-current injection (`InjCurrents`, `CalcInjCurrentArray`) —
   Load is the template; **model 3 (PV) needs the DQDV var-adjustment** —
   read `Solution.pas` for where generators' var loops hook in
   (`Check...`? probe a model-3 case against the oracle for iteration counts).
4. Registers + `TakeSample` + the class `SampleAll`/`ResetRegistersAll`
   call-site wiring.
5. Tests: inline unit tests per model probed against the oracle (the WP5.3
   pattern: micro-circuit, transcribed numbers); `gen_props.py` scenarios
   (defaults, kW/PF/kvar/kVA web, conn=delta, model variants, fuel props,
   makelike); the `generator_snap` scenario lands in WP6.9's phase6.json.

---

### WP6.3 — MeterElement base + Monitor [15%]

**Pascal:** `Meters/MeterElement.pas` (110 lines), `Meters/Monitor.pas`
(1696 lines), scope per §2.3.

Steps:
1. `MeterElementData` (embeds `CktElementData`: `metered_element:
   Option<ElemRef>` + `RefSnapshot` per the WP4.7 pattern, `metered_terminal`,
   sensor arrays, `AllocateSensorArrays`/`CalcAllocationFactors` l.47/72) +
   `ElemKind::Meter` + circuit `meters` list — check Pascal list membership:
   monitors/meters join the device list but **not** PD/PC (yprim `None`,
   like controls; `TMonitorObj.CalcYPrim` l.683 is empty).
2. Monitor props 1–7 (`element` any-class ObjectRef like CapControl's,
   `terminal`, `mode`, `action` Action-prop {clear|save|take|process},
   `residual`, `VIpolar`, `Ppolar`) + `RecalcElementData` (l.532:
   `ValidMonitor` per mode vs element class, buffer allocation) +
   `ClearMonitorStream` (l.691: header build per mode — port the exact
   strings).
3. `TakeSample` (l.1170-1580): the mode dispatch + the ±16/±32/±64 modifier
   paths + residual + VIpolar/Ppolar; `AddDblsToBuffer`/`AddDblToBuffer`
   (f32 narrowing per §2.3).
4. Class sweeps `ResetAll`/`SampleAll`/`SampleAllMode5` + the solution-loop
   call sites (§2.2 — `FinishTimeStep` ordering and the per-solve mode-5
   sites).
5. Tests: unit tests on a 2-bus circuit — mode 0/1 sample values transcribed
   from the oracle (`Monitors.Channel`); header-string equality; modifier
   bits; `props.json` scenarios. dss-python exposes `Monitors.ByteStream` —
   if cheap, assert our serialized record layout against it once (pins the
   f32 layout).

---

### WP6.4 — EnergyMeter object + zone build [15%]

**Pascal:** `Meters/EnergyMeter.pas` — class/ctor/props (l.509-758),
`ResetMeterZonesAll` (l.797), `SetHasMeterFlag` (l.1752),
`MakeMeterZoneLists` (l.1773-2019), `AddToVoltBaseList` (l.2121),
`CheckBranchList` (l.2286); `Common/Circuit.pas` `DoResetMeterZones` (l.2145).

Steps:
1. Props 1–24 (option-list prop semantics: the `Option` prop parses a string
   array into the boolean set — read `SetOptions`/`GetOptions` l.582/604;
   `Mask` is a `NumEMRegisters`-sized double array; `ZoneList`; the read-only
   reliability props error or return per Pascal), ctor register-name
   initialization, `MakeLike`.
2. Zone build per §2.1: `ResetMeterZonesAll` (flag/ref clearing sweep over
   **all** PD+PC elements, adjacency rebuild, meters in creation order,
   `SetHasMeterFlag` first — port the order exactly) + `MakeMeterZoneLists`
   verbatim (l.1773 loop: sensor/meter ref assignment, `BranchNumCustomers`,
   `DistFromMeter` accumulation in km, loop/parallel detection via
   `CheckParallel`, `ZoneEndsList`, the manual `ZoneList` branch,
   `TotalUpDownstreamCustomers` l.1693, `AssignVoltBaseRegisterNames` l.3082).
3. Trigger wiring: `DoResetMeterZones` from the Y-build path when bus lists
   were rebuilt (`MeterZonesComputed`/`ZonesLocked` flags on the circuit) —
   find every Pascal call site (`Circuit.pas` l.2246 ProcessBusDefs tail; the
   energymeter property edits that set `MeteredElementChanged`).
4. `GetPCEatZone` (l.2809) + a `Dss::meter_zone(name)` API for tests
   (sequence list element names, load list, zone-end buses).
5. Tests: the `meter_zone_micro` expectations (zone membership/order/flags)
   probed via dss-python `Meters.AllBranchesInZone`/`AllEndElements`/
   `ZonePCE`; sub-meter boundary (zone stops at the second meter);
   `props.json` scenarios. Run the IEEE13 master + meter — zone walks all
   branches, node order/voltages unchanged (meters add no Y entries).

---

### WP6.5 — EnergyMeter registers + TakeSample + hook wiring [12%]

**Pascal:** `EnergyMeter.pas` `ResetRegisters` (l.1205), `Integrate` (l.1271),
`TakeSample` (l.1289-1692), `Accumulate_Gen` (l.2198), `Accumulate_Load`
(l.2208), `SetDragHandRegister` (l.2858), `ResetAll`/`SampleAll`/`SaveAll`
(l.851/900/934); `TSystemMeter` register core (l.3388-3516, files excluded);
`SolutionAlgs.pas` `FinishTimeStep` (l.74).

Steps:
1. Register array (`NumEMRegisters = 32 + 5·NumEMVbase`), names, totals
   (`RegisterTotals`), `Integrate` with `trapezoidal_integration` (circuit
   flag exists since WP5.8) and `Delta_Hrs`.
2. `TakeSample` verbatim: metered-terminal power/current, zone sweep over the
   branch list (losses split by line/transformer, 1ph/3ph/seq, voltage-base
   buckets via `VBaseList`, EEN/UE overload accumulation — `Accumulate_Load`
   reads each zone load's `Get_ExceedsNormal`/`Unserved`; check what Load
   already exposes from Phase 3 and add the missing getters), gen
   accumulation, drag-hand maxima, `FirstSampleAfterReset` semantics.
3. Fill the Phase 5 hook stubs (§2.2) with the real class sweeps; `Reset`/
   `Sample`/`Save` meter actions + the `Set DemandInterval`-adjacent options
   only as far as register behavior needs (DI files stay Phase 8).
4. `TSystemMeter` register accumulation (no files) if the oracle exposes the
   totals cheaply (`Circuit.TotalPower` exists; system-meter registers feed
   `Totalize` — probe `dss.ActiveCircuit.Meters.Totals`); else document.
5. Tests: 2-bus + meter, 3 daily steps with a known shape — registers
   hand/oracle-checked (kWh = trapezoid vs plain per the flag); drag-hand;
   reset semantics; the full `meter_daily_ieee13` golden lands in WP6.9.

---

### WP6.6 — Reliability: SAIFI/SAIDI/CAIDI + customer counts [8%]

**Pascal:** `EnergyMeter.pas` `CalcReliabilityIndices` (l.2411-2584),
`TotalUpDownstreamCustomers` (l.1693, already in WP6.4 zone build),
`CheckBranchList` (l.2286); the `Relcalc` command + `AssumeRestoration`
option (`ExecHelper`/`ExecOptions` — locate exact spellings); PD reliability
accumulators (`AccumulatedBrFltRate`, `BranchFltRate`, `CalcFltRate` on
PDElement — `PDElement.pas`).

Steps:
1. `TPDElement.CalcFltRate`/`AccumFltRate` (read `PDElement.pas` — the
   per-branch λ accumulation walks parent links) + Line's `Miles`/length
   handling.
2. `CalcReliabilityIndices` verbatim (feeder-section array, OCP device
   detection via the `HasOCPDevice` flag — relays/fuses/reclosers are
   Phase 7, so sections reduce to the no-OCP case; **probe the oracle** on a
   meter zone without protection devices and pin SAIFI/SAIDI/CustInterrupts),
   the read-only props 20–24 getters.
3. `Relcalc` command + tests vs oracle on `meter_zone_micro` extended with
   `faultrate`/`numcust` data.

---

### WP6.7 — Sensor + load allocation [5%]

**Pascal:** `Meters/Sensor.pas` (581 lines); `EnergyMeter.pas` `AllocateLoad`
(l.2147); `ExecHelper` `DoAllocateLoadsCmd` + `Set AllocationFactors`;
`MeterElement.CalcAllocationFactors` (WP6.3).

Steps: Sensor props/ctor/`RecalcElementData`/`TakeSample`/`ClearSensor` +
`SetHasSensorFlag` (l.357; feeds the zone build's `HasSensorObj` inheritance);
`AllocateLoad` sweep + the `allocateloads` command + Load's
`set_kw_from_allocation` path (`AllocationFactor`/`kWh`-spec loads — verify
what Phase 3 Load already parses; `XFKVA`/`AllocationFactor` interplay).
WLS error getters (l.599/637) port with it (pure functions). Tests: oracle
probe of an allocation round-trip (sensor on a line + 2 loads with kwh specs).
If the oracle shows `allocateloads` needs unported machinery, document +
`NOT_PORTED` the command, keep the Sensor object.

---

### WP6.8 — GenDispatcher; StorageController + AutoAdd skeletons; ReduceAlgs (basic) [10%]

**Pascal:** `Controls/GenDispatcher.pas` (346), `Controls/StorageController.pas`
(1787), `Common/AutoAdd.pas` (567), `Meters/ReduceAlgs.pas` (480),
`EnergyMeter.pas` `ReduceZone` (l.2257), `InterpolateCoordinates` (l.2298).

Steps:
1. GenDispatcher: full port (props, gen list resolution, `Sample` power test +
   `GenMultiplier` dispatch, `DoPendingAction`) into the WP5.7 control sweep;
   unit test vs oracle on a 2-bus + generator + dispatcher case.
2. StorageController per §2.6 (props + skeleton; behavior `NOT_PORTED` →
   Phase 7).
3. AutoAdd per §2.6 (options + object; mode integration only if cheap —
   document the empirical decision).
4. ReduceAlgs **basic**: `DoReduceDefault` + the `Reduce` command plumbing
   (`ReduceZone` l.2257); the exotic strategies (`DoReduceStubs`,
   `DoBreakLoops`, …) `NOT_PORTED` with phase pointers. `Interpolate`
   (l.2298) if cheap (pure coordinate math; Run_8500Node calls it **after**
   solve so the gate replay can include it only if ported — else drop the
   command from the replay and note it).
5. Tests: props scenarios; a reduce smoke test vs oracle (branch count after
   `Reduce` on the micro zone) — if oracle behavior turns out to depend on
   unported pieces, scope down to parse + documented deferral. **Do not let
   this WP balloon — everything here is skeleton-grade except GenDispatcher.**

---

### WP6.9 — Goldens + the 8500-node gate [10%]

1. `tools/golden/gen_phase6.py` → `tests/golden/phase6.json` (scenarios per
   §1.2) and the `ieee8500` golden:
   - extend `tools/golden/generate.py` (or a dedicated gen) to capture meter
     registers (`Meters.RegisterNames/RegisterValues`), monitor channels
     (`Monitors.Channel(i)`, `dblHour`, headers, `SampleCount`) — whatever
     the Phase-0 generator already captures for these, verify and extend;
   - `ieee8500` case: Master.dss + meter + `Set Maxiterations=20` + Solve
     (+ a daily segment for register integration — `set mode=daily number=24`
     after the snap; pin the exact command list in the golden), `props=false`.
2. `crates/dss-core/tests/golden_ieee8500.rs` + `golden_phase6.rs` asserting
   §1.1/§1.2. The 8500 runs in seconds — keep it un-`#[ignore]`d if the full
   test stays under ~60 s in debug; else `#[ignore]` + a release-mode CI note
   (measure first).
3. Record solve-time comparison (release build) in STATUS.md.
4. Debugging order for register mismatches: zone membership first
   (`meter_zone_micro` catches structural bugs cheap), then per-register on
   the micro daily case, then 8500.

---

### WP6.10 — Phase exit

1. `rg "TODO\(compat\)"` / `rg "NOT_PORTED"` sweep — all new sites point at
   their phase.
2. Re-run everything: props, slice, feeders, feeders_controls, phase4,
   phase5, phase6, ieee8500. All green.
3. Rewrite `STATUS.md` (Phase 6 record; "next = Phase 7, write PHASE7_PLAN.md
   first").
4. Commit only on explicit user request.

## 4. Deferred in this phase

- All meter/monitor **file** outputs: DI/CSV/overload/voltage report files,
  monitor `Save`/`TranslateToCSV`, `ZoneDump`, `SaveZone` (Phase 8 with
  Show/Export).
- Monitor modes 4 (flicker/Pstcalc) and 7 (Storage); harmonic-mode sampling
  (Phase 7).
- Generator dynamics/harmonics machinery (§2.5), `MakePosSequence`
  (everywhere).
- Storage element + real StorageController behavior, Relay/Fuse/Recloser
  (`HasOCPDevice` stays effectively false) — Phase 7.
- A-Diakoptics/`EnergyMeter` actor-parallel paths, `FeederObj`/`DoFeederStuff`
  remnants (dead upstream), MemoryMap DI machinery (`MemoryMap_lib.pas` —
  replaced by plain in-memory buffers where behavior-relevant).
- `BatchEdit`, `vdiff`, and other ExecHelper tail commands unless a gate
  script needs one (Phase 8).
