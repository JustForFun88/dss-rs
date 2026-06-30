# Phase 6 record

> Archived from `STATUS.md` (moved 2026-06-16 to keep the live handoff lean).
> Frozen historical work-package log; superseded only by the code + tests.
> Section references (`§3.x`, `§5` …) point back to `STATUS.md`.

## 1d. Phase 6 record (branch `phase-6-meters-topology` → merged to `main` `b98223a`)

Execution plan: **`PHASE6_PLAN.md`** (WP6.1–WP6.10).

**WP6.1 — Topology foundations — ✅ done, gate-green.** Files:
- `src/circuit/ckt_tree.rs` (new): `CktTree`/`TreeNode`/`ZoneEndsList` as an
  index arena (Pascal pointers → `ElemRef`/node indices), traversal ported
  verbatim — `Add` (root: parent link but *not* in the parent's child list),
  `AddNewChild`, `AddNewObject`, `PushAllChildren`+`GoForward` (LIFO stack:
  **the last-added child is visited first**, and children added mid-sweep are
  picked up via `ChildAdded` — observable as the meter `SequenceList` order),
  `GoBackward`/`First`/`StartHere`/`Level`, the stateful
  `Get_ToBusReference` cursor semantics (single entry always returned;
  multi-entry iterates → `None` → resets). 6 inline tests hand-traced from
  the Pascal. Plus `BuildActiveBusAdjacencyLists` (`build_active_bus_adjacency_lists`):
  enabled non-shunt PD branches bucketed at **every** terminal bus (only if
  `AllTerminalsClosed` = ≥1 closed conductor among the first nphases per
  terminal); PC elements **and shunt capacitors/reactors** on the terminal-1
  PC list. Verified: VSource is `NON_PCPD_ELEM` — in *neither* list (that is
  why Pascal's zone build has the special `GetSourcesConnectedToBus` sweep);
  exec test `bus_adjacency_lists_bucket_elements` pins all of this.
- `elements/ckt.rs`: `ElemFlags` bitset (element-level subset of Pascal
  `TDSSObjectFlag` — Checked/Flag/HasEnergyMeter/HasSensorObj/IsIsolated/
  HasControl/IsMonitored/HasOCPDevice/HasAutoOCPDevice; the property-engine
  flags are handled by other mechanisms) + the meter-zone fields on
  `CktElementData` (`from_terminal` (init 1, Pascal TPDElement ctor),
  `to_terminal`, `parent_pd`, `meter_obj`, `sensor_obj`,
  `branch_num_customers`, `branch_total_customers`) — on the shared base
  because Pascal puts `MeterObj`/`SensorObj` on TPCElement too.
- `circuit/bus.rs`: the 7 reliability accumulators (`BusFltRate`,
  `Bus_Num_Interrupt`, `BusCustInterrupts`, `BusCustDurations`,
  `BusTotalNumCustomers`, `BusTotalMiles`, `BusSectionID`) +
  `zero_reliability_accums` (`BusSectionID := -1`).
- `GetIsolatedSubArea`/`GetSourcesConnectedToBus`/`FindAllChildBranches`
  (CktTree.pas l.471-676) deliberately not ported yet: the WP6.4 meter zone
  build has its own loop; port them with `Circuit.GetTopology` when a
  consumer lands. dss-core lib tests 212 → 219.

**WP6.2 — Generator — ✅ done, gate-green.** Files:
- `elements/pc/generator.rs` (new): `TGeneratorObj` power-flow port (scope
  PHASE6_PLAN §2.5). Full 44-prop table + spectrum/basefreq/enabled tails;
  ctor defaults (kW=1000, kvar=60, kV=12.47, Vbase=7200 L-N, kVArating=
  kW·1.2, puXd/Xdp/Xdpp=1/0.28/0.20, Vminpu/max=0.90/1.10, PVFactor=0.1,
  pctReserve=20); `SetNominalGeneration` (dispatch ON/OFF via
  GeneratorDispatchReference/PriceSignal, OFF → tiny −0.1·kW/nphases
  resistive load; mode-dispatch shape mults; Yeq/Yeq95/Yeq105; model-3 var
  clamp; model-7 PhaseCurrentLimit); `RecalcElementData`; `CalcYPrim`/
  `CalcYPrimMatrix` (negate Yeq for generation, model-3 only 1% in Yprim,
  wye/delta); **all six power-flow models** `DoConstantPQGen`/`DoConstantZGen`/
  `DoPVTypeGen`/`DoFixedQGen`/`DoFixedQZGen`/`DoCurrentLimitedPQ` (model 7,
  incl. ForceBalanced pos-seq via `SymComp`); `StickCurrInTerminalArray`
  (signs **reversed** from Load — generator injects); `InjCurrents`/
  `GetTerminalCurrents`; the kW/PF/kvar/kVA/MVA web (`SyncUpPowerQuantities`/
  `SetkWkvar`/`side_effect_kvar`, `kVANotSet`); 6 energy registers +
  `TakeSample`/`Integrate`/`SetDragHandRegister`/`CheckOnFuel`; the model-3
  DQDV trio (`InitDQDVCalc`/`CalcDQDV`/`ResetStartPoint`). UserModel/UserData/
  ShaftModel/ShaftData/DynamicEq/DynOut are `NOT_PORTED` (no DLLs / dynamics →
  Phase 7) — stored + dumped, setting them is a hard parse error. Model 6
  (user DLL) records error 567 at solve.
- `obj/dss_enum.rs`: `gen_disp_mode`/`gen_status`/`gen_model` enums.
- `circuit/circuit.rs`: `ElemKind::Generator` (joins PC list +
  `generators` list), `generator_dispatch_reference` field.
- `elements/traits.rs` `SysCtx`: `gen_multiplier`/`generator_dispatch_reference`/
  `price_signal`; `ElemStore::obj_mut`. `elements/ckt.rs`:
  `signal_reset_solution_initialized` (model-3 edit → `SolutionInitialized:=
  FALSE`, propagated in `exec::edit_active`).
- `solution/solution.rs`: `SetGeneratorDispRef` (per-mode dispatch ref, run at
  `solve_snap` head) + `SetGeneratordQdV` (model-3 DQDV from the system-Y
  diagonal via new `Solution::system_matrix_element`, then a re-init zero-load
  snapshot), wired into `DoPFLOWsolution` where the Phase-3 stub had been.
- Registered in `exec::Dss::new`. Tests: 7 inline (`set_nominal_generation`
  scalars, kW/PF/kVA web, OFF state, delta nconds, fixed status, TakeSample)
  + 2 exec integration (`generator_model1_pq_snapshot`,
  `generator_model3_pv_snapshot` — the latter exercises the DQDV path),
  numbers transcribed from the oracle. `gen_props.py`: 8 Generator scenarios
  (default, kW/PF, kW/kvar delta, model-3 PV, kVA, fuel, status/dispatch,
  makelike) → `props.json` regenerated; `props_roundtrip` green. dss-core lib
  tests 219 → 228.

  Class-level `SampleAll`/`ResetRegistersAll` sweeps + the solution-loop
  sample call sites are deferred to WP6.5 (they belong with the EnergyMeter
  hook wiring); `TakeSample` itself is ported and unit-tested now.

**WP6.3 — MeterElement base + Monitor — ✅ done, gate-green.** Files:
- `elements/meter/meter_element.rs` (new): `MeterElementData` (Pascal
  `TMeterElement`, embeds `CktElementData`): `metered_element: Option<ElemRef>`
  + `metered_terminal`/`metered_element_changed` + a `MeteredSnapshot`
  (full_name/kind/nphases/nconds/nterms/yorder/buses + num_windings/num_steps/
  num_variables — captured at `element=` resolution like the WP4.7 control
  `RefSnapshot`, since `RecalcElementData` runs at `EndEdit` after the foreign
  view is gone) + the sensor-allocation arrays and
  `AllocateSensorArrays`/`CalcAllocationFactors` (ported, `#[allow(dead_code)]`
  until Sensor in WP6.7).
- `elements/meter/monitor.rs` (new): `TMonitorObj` port. Props 1–7 (`element`
  any-class `object_ref_any`, `terminal`, `mode`, `action`, `residual`,
  `VIPolar`, `PPolar`) + CktElement tail (`basefreq`/`enabled`/Like = 10);
  `RecalcElementData` (mode→class validation 663/664/2016001/2016002, terminal
  check 665, adopt metered nphases/nconds, `SetBus(1,…)`); `ClearMonitorStream`
  (the **exact** per-mode header strings + `RecordSize`, incl. all the
  ±16 sequence / ±32 magnitude / ±64 pos-seq / residual / VIpolar/Ppolar
  combos — probed against the oracle); `TakeSample` (modes **0,1,2,5,6,9,11**
  faithfully, the symmetrical-component + power + polar conversions, residual,
  the magnitude/posseq write paths) into a growing in-memory **`Vec<f32>`**
  (`AddDblToBuffer` f32 narrowing; one buffer holds all samples — we never
  spill to disk, so `Save`/`MonitorStream` flush is folded in). `CalcYPrim` is
  empty and `GetCurrents` returns zeros (a monitor never stamps Y). Modes 3
  (PCElement state vars — dynamics surface), 4 (flicker/Pstcalc), 7 (Storage),
  8/10 (transformer winding currents/voltages) and 12 (LL) build their header
  but **defer the sample body** (no gate exercises them; Phase 6+/7) — noted in
  the module header. File `Save`/`TranslateToCSV` is Phase 8.
- **Shared engine:** `obj/dss_enum.rs` `monitor_action` enum (Clear/Save/
  TakeSample/Process/Reset → 0/1/2/3/0); `capacitor.rs` `states()` accessor
  (mode-6 read). `circuit.rs`: `ElemKind::Meter` (device list + new `monitors`
  list, **not** PD/PC, no Yprim — like controls). `exec/mod.rs`: Monitor
  registered **after** Generator (Pascal DSSClassDefs.pas:288); the `Sample`
  command (`DoSampleCmd` → `MonitorClass.SampleAll`) and a minimal `Reset`
  command (`reset_all_monitors`); a public `Dss::monitor_view(name)` →
  `MonitorView` (header/sample_count/dbl_hour/channels) mirroring dss-python
  `Monitors.Header`/`SampleCount`/`Channel(i)`/`dblHour`.
- **Solution wiring:** `solution/monitors.rs` (new) — `sample_all_monitors`
  (Pascal `SampleAll` mode≠5 / `SampleAllMode5` mode=5; pair_mut the monitor +
  its metered element, the WP5.7 disjoint-borrow pattern) + `reset_all_monitors`.
  The Phase-5 no-op hook stubs got real bodies: `sample_all_monitors_and_meters`
  → monitor `SampleAll` (EnergyMeter `SampleAll` still WP6.5);
  `end_of_time_step_cleanup` → `SampleAllMode5` (`SolutionAlgs.pas` l.96).
- **Oracle facts (probed, then pinned):** snapshot `Solve` does **not** sample
  monitors — sampling happens in the time-series loop (or the `Sample`
  command + a buffer flush); a daily `number=1 stepsize=1h` solve samples at
  hour 1 where the flat default shape gives mult=1, so the sample equals the
  snapshot solution. Mode-5 channels 11/12 (`SolveSnap_uSecs`/`TimeStep_uSecs`)
  are wall-clock timings → the port records 0 and the tests skip them.
- Tests: 4 exec integration (`monitor_mode0_mode1_daily` — V/I + powers
  channels transcribed from the oracle; `monitor_mode5_solution_vars` — the 10
  deterministic solution vars; `monitor_header_modifiers` — 7 modifier-combo
  headers; `monitor_mode2_tap_and_class_check` — transformer tap + the 663
  class-mismatch error). `gen_props.py`: 5 Monitor scenarios (default, mode-1
  residual+ppolar, mag+seq VIpolar-off, transformer-tap, makelike) →
  `props.json` regenerated (pure insertions); `props_roundtrip` green. dss-core
  lib tests 228 → 232.

**WP6.4 — EnergyMeter object + zone build — ✅ done, gate-green.** Files:
- `elements/meter/energymeter.rs` (new): `TEnergyMeterObj` port. Props 1–24 +
  the CktElement tail (`basefreq`/`enabled`/Like = 27): `element`
  (`object_ref_any`), `terminal`, `action`, `option`/`ZoneList` (the new
  `string_list` prop type), `kVANormal`/`kVAEmerg`, `PeakCurrent`
  (`double_v_array` over `SensorCurrent`, length = nphases), `Mask`
  (`double_f_array` over the 67 registers), the 7 loss-report booleans,
  `Int_Rate`/`Int_Duration`, and the 5 read-only reliability doubles. Ctor
  seeds the fixed register names + `SensorCurrent := 400 A`, `ResetRegisters`
  (drag-hand maxima = −1e50). `RecalcElementData` (PD-element validation 525,
  terminal check 524, `SetBus(1,…)` + adopt nphases/nconds on element change),
  `MakeLike`, `AssignVoltBaseRegisterNames` (`%.3g kV …` via `util::fmt_g`).
  `CalcYPrim` empty, `GetCurrents` zeros. The register **accumulation**
  (`TakeSample`, WP6.5) and reliability indices (WP6.6) are deferred; the
  register/derivative/totals-mask arrays + drag-hand reset are in place.
- `solution/meters.rs` (new): the zone builder as free functions over the
  registry (the WP5.7 dispatcher pattern — the meter's `BranchList`/
  `SequenceList`/`LoadList`/`ZonePCE`/`VBaseList` are built in **locals** while
  the *other* elements' flags/refs and the buses' `DistFromMeter` are mutated
  through the store, then installed into the meter). `do_reset_meter_zones`
  (Circuit.pas `DoResetMeterZones`, gated on `meter_zones_computed`/
  `zones_locked`) → `reset_meter_zones_all` (clear Checked/IsIsolated/
  TerminalsChecked + meter/sensor/parent refs on every element, build bus
  adjacency, `SetHasMeterFlag`, walk meters in creation order) →
  `make_meter_zone_lists` (verbatim main loop: `AddNewObject` shunts,
  `AddNewChild` PD branches, `AddToVoltBaseList`, loop/parallel detection via
  `CheckParallel`, `ZoneEndsList`, customer counting) + `TotalUpDownstream​Customers`
  (backward sweep) + `GetPCEatZone`.
- **Trigger wiring:** `ymatrix.rs` `build_y_matrix` calls `do_reset_meter_zones`
  right after `reprocess_bus_defs` (Pascal `ReprocessBusDefs` tail, Circuit.pas
  l.2246) — so zones rebuild on every Y-build that reprocessed the buses (with
  `zones_locked = false`).
- **Shared engine:** new `PropType::StringList` + `PropDef::string_list`
  (Pascal `InterpretTStringListArray` parse / `StringListToString` render
  `[a, b, c]`) + `DssObject::{get,set}_string_list`; the `DoubleFArray` parse
  now returns the **parsed count** via `parse_as_vector` (Pascal `ParseAsVector`
  → `prevInt`), which the `Mask` side effect needs to default the unspecified
  slots to 1.0. `obj/dss_enum.rs` `energy_meter_action` enum (Allocate/Clear/
  Reduce/Save/TakeSample/ZoneDump → 0..5). `circuit.rs`: `ElemKind::EnergyMeter`
  + new `energy_meters` list (device list, no Yprim, not PD/PC). `meter_element.rs`
  `AllocateSensorArrays` fixed to **preserve** `SensorCurrent`/`SensorVoltage`
  across resizes (Pascal `ReAllocMem`; the ctor's 400 A survives the recalc).
  `exec/mod.rs`: EnergyMeter registered after Monitor; public
  `Dss::meter_zone(name)` → `MeterZoneView` (`AllBranchesInZone`/`AllEndElements`/
  `ZonePCE`/`RegisterNames`) mirroring dss-python `Meters.*`.
- **Oracle facts (probed, then pinned):** `Meters.AllBranchesInZone` =
  `SequenceList` = the BranchList `GoForward` (LIFO-over-children) order;
  `AllEndElements` = the `ZoneEndsList` order; `ZonePCE` = the shunt objects in
  branch order. A sub-meter mid-feeder **stops** the parent meter's zone (the
  metered element gets `HasEnergyMeter`, excluded from the PD search). The
  StringList dump is `[E, R, C]`-style; `Mask` defaults trailing slots to 1.0.
- Tests: 3 exec integration (`energymeter_zone_radial` — branches/ends/PCE +
  `TotalUpDownstreamCustomers` totals; `energymeter_submeter_boundary` — the
  sub-meter zone split; `energymeter_requires_pd_element` — the 525 error).
  `gen_props.py`: 3 EnergyMeter scenarios (default, option/mask/zonelist/
  peakcurrent edited, makelike) → `props.json` regenerated (pure insertions);
  `props_roundtrip` green. dss-core lib tests 232 → 235.

**WP6.4 hardening (audit-driven) — ✅ done, gate-green.** Closed the gaps an
audit flagged against the Pascal spec:
- **Manual `ZoneList` zone build implemented** (Pascal l.1987 else-branch): new
  `ElemStore::find_ckt_element` (Pascal `SetElementActive`) resolves the listed
  full names; each branch terminal consumes the next valid PD entry via the
  monotonic `zone_list_counter`. NOTE: the oracle (dss_capi 0.14.5) **access-
  violates** on a manual zone, so there is no golden — the port produces a
  deterministic, memory-safe zone instead (`energymeter_manual_zonelist` locks
  it and guards against silent-no-op regression).
- **PC-type filter** (`is_zone_pce`) added to the zone walk — the
  `PCElementType ∈ {LOAD,GEN,PVSYSTEM,STORAGE,CAP,REACTOR}` allow-list Pascal
  gates `AddNewObject` on (the adjacency list may hold any PC element).
- **`EndEdit` recalc now gated on `needs_recalc`** (Pascal `Flg.NeedsRecalc`,
  set only by `element`/`terminal`): editing an unrelated property — or creating
  a bare meter with no element — no longer raises a spurious "Circuit Element
  not set" (oracle: a bare meter is created cleanly).
- **`set_voltage_bases` voltage-base timing fix** (Pascal `SetVoltageBases`
  l.1083): suppress the meter-zone auto-build during the zero-load snapshot
  (force both gate flags TRUE), assign `kVBase`, then call `DoResetMeterZones`
  explicitly — so `AddToVoltBaseList` sees valid bases. Previously the zone was
  built during `CalcVoltageBases` with `kVBase = 0`, leaving every per-base loss
  register named `Aux<n>`.
- Disabled / no-element meters now install a non-nil empty `BranchList`
  (Pascal `TCktTree.Create` then `Exit`).
- +7 exec tests (parallel lines, meshed/loop zone, multi-voltage-base register
  names, manual zonelist, bad terminal 524, disabled empty zone, no-element/
  unrelated-edit no-revalidation) — all transcribed from the oracle where it
  doesn't crash. dss-core lib tests 235 → 242.

---

**WP6.5 — EnergyMeter registers + TakeSample + hook wiring — ✅ done, gate-green.**
Files: `elements/ckt.rs`, `elements/traits.rs`, `elements/pd/{line,transformer,
reactor,capacitor}.rs`, `elements/pc/load.rs`, `elements/meter/energymeter.rs`,
`solution/meters.rs`, `solution/solution.rs`, `exec/mod.rs`.
- **CktElement numeric surface** (`traits.rs`): `norm_amps`/`emerg_amps`
  accessors (default 0; PD elements override), `max_terminal_one_imag`
  (CktElement.pas l.552), `excess_kva_norm`/`excess_kva_emerg` (PDElement.pas
  l.230/257 — side-effect-set the new `overload_een`/`overload_ue` on
  `CktElementData`), `get_losses_split` (default `(total,total,0)`),
  `get_seq_losses` (default 0).
- **PD overrides:** `Line.get_seq_losses` (3-phase `Phase2SymComp`, Line.pas
  l.1495), `Transformer.get_losses_split` (no-load = power into `Yprim_Shunt`,
  Transformer.pas l.1635), `Reactor.get_losses_split` (`V²/Rp` shunt branch,
  Reactor.pas l.1017).
- **Load EEN/UE** (`load.rs`): `een_factor`/`ue_factor` fields +
  `exceeds_normal`/`unserved` (Load.pas l.2057/2004 — lowest-phase-Vpu vs
  the circuit `normal_min_volts`/`emerg_min_volts` criteria).
- **`TakeSample`** ported verbatim as a free function in `solution/meters.rs`
  (EnergyMeter.pas l.1289): metered-terminal power, the radial/meshed overload
  EEN/UE pass (sets PD `Overload_*` + load factors), the zone losses sweep
  (line/transformer split, seq + 3-/1-phase modes, voltage-base buckets),
  `Accumulate_Load`/`Accumulate_Gen`, drag-hand maxima, and the
  `MaxZonekVA`/excess overload-energy registers. `Integrate` honours the
  circuit trapezoidal flag (skipping the first sample after reset);
  `SetDragHandRegister` keeps running maxima. The meter's branch tree and
  register arrays are moved out for the walk (the store keeps the meter
  borrowed) and written back via `begin/end_take_sample`.
- **Hook wiring:** `sample_all_monitors_and_meters` now runs
  `take_sample_all` when the mode requests meter sampling; `DoSampleCmd`
  (`Sample`) and `DoResetCmd` (`Reset`/`Reset Meters`) wired; new
  `Set Trapezoidal=` option (ordinal 41); `Dss::meter_registers` test API.
- **`take_sample_all`/`reset_all_meters`** added; `SystemMeter` core and the
  Generator/Storage/PVSystem `ResetRegistersAll`/`SampleAll` call sites stay
  deferred (WP6.8 / later), as does the phase-voltage-report demand-interval
  path (Phase 8).

**WP6.5 audit follow-up — ✅ fixed, gate-green.**
- **`Reset` (no-arg) now resets controls + clears the event/error log**
  (`do_reset_cmd`), matching Pascal `DoResetCmd` (ExecHelper.pas l.1537):
  the no-arg path was previously only resetting monitors + meters, silently
  skipping `DoResetControls` even though Phase-5 controls exist. Re-uses the
  already-tested `reset_all_controls`; the `C`/`E` selectors and the
  unknown-argument error are now wired (`F`/`K` accepted as no-ops — no Fault /
  KeepList class yet).
- **Register-coverage tests** added against the oracle to exercise the
  TakeSample paths the original WP6.5 tests left unvalidated: generator
  registers (`Accumulate_Gen` sign), sequence-mode loss split, transformer
  load/no-load split + a 2nd voltage-base bucket, line-overload + radial
  EEN/UE, voltage-criterion EEN/UE, and the `Reset` controls path
  (`capacitor_closed` test API). dss-core lib tests 242 → **251**
  (3 WP6.5 daily-ramp tests + 6 follow-up).

---

**WP6.6 — Reliability: fault-rate sweep + `RelCalc` — ✅ done, gate-green.**
Files: `solution/meters.rs` (`calc_all_reliability_indices` /
`calc_reliability_indices`), `elements/traits.rs` (`ReliabilityData` +
`CktElement::reliability_data`), `elements/pd/{line,transformer,capacitor,
reactor}.rs` (`CalcFltRate` overrides), `elements/ckt.rs` (PD reliability
accumulators), `circuit/bus.rs` (`bus_int_duration` — the one missing
`TDSSBus` field), `elements/meter/energymeter.rs` (source getters +
`set_reliability_results`), `exec/mod.rs` (`RelCalc` cmd ord. 100 →
`do_relcalc_cmd`).
- Ports `TPDElement.CalcFltRate`/`AccumFltRate`/`CalcNum_Int`/
  `CalcCustInterrupts`/`ZeroReliabilityAccums`, `TLineObj.CalcFltRate`
  (× `Len`), and `TEnergyMeterObj.CalcReliabilityIndices` (EnergyMeter.pas
  l.2411) 1:1, plus `DoLambdaCalcs` (the per-circuit driver: zero all buses,
  loop meters; `AssumeRestoration` is the single positional yes/no param).
- **Decision (user-confirmed): "faithful port, dormant math".** OCP devices
  (Relay/Recloser/Fuse) are Phase 7, so `Flg.HasOCPDevice` is never set →
  `SectionCount` stays 0 → `RelCalc` aborts with **error 52902 exactly like
  the oracle** (dss-python raises `DSSException (#52902)` on the same feeder).
  The section-array / SAIFI / SAIDI / CAIDI math below the abort is ported
  verbatim but is unreachable until Phase 7 (`GetOCPDeviceType` inlined to 0
  for now). The backward fault-rate sweep + customer rollup *do* run before
  the abort, so the bus/branch accumulators are populated and testable.
- Oracle pin: probed `relcalc` on a 2-section radial feeder → `#52902` (no
  per-branch reliability getters exist in the COM API, so the dormant indices
  can't be golden-pinned until OCP devices land). 6 tests: the 52902 abort +
  hand-computed backward-sweep accumulators (`BranchFltRate =
  FaultRate·pctperm·0.01·Len`; `AccumulatedBrFltRate`/miles roll-up;
  `BusTotalNumCustomers`), a junction-rollup branching feeder, the no-section
  invariant, and `AssumeRestoration` parse. dss-core lib 251 → **257**.
- Audit hardening: `TotalUpDownstreamCustomers` now applies the full Pascal
  `HasOCPDevice ∧ AssumeRestoration ∧ HasAutoOCPDevice` roll-up guard (via a
  new meter `AssumeRestoration` field set by `DoLambdaCalcs`) instead of an
  unconditional roll-up — correct-by-vacuity in Phase 6, future-proof for
  Phase 7. `GetOCPDeviceType`'s inlined `0` is now marked `TODO(WP7)`.
- Oracle-pinned after all: although per-*branch* getters are absent, the
  per-*bus* reliability quantities the sweep fills before the abort **are**
  exposed (`Bus.Lambda`/`N_Customers`/`TotalMiles`/`SectionID`). New
  `tools/golden/gen_reliability.py` → `tests/golden/reliability.json` →
  `tests/golden_reliability.rs` pins both feeders (radial + branching) to the
  oracle; the branch accumulators follow from the bus↔branch identity (no OCP →
  branch value = FROM-bus value).

---

**WP6.7 — Sensor + load allocation — ✅ done, gate-green.** Files:
`elements/meter/sensor.rs` (new), `elements/meter/{mod,meter_element}.rs`,
`elements/pc/load.rs`, `solution/meters.rs`, `circuit/circuit.rs`, `exec/mod.rs`.
- `elements/meter/sensor.rs` (new): `TSensorObj` port. Props 1–12 + the
  CktElement tail (`element` `object_ref_any`, `terminal`, `kVBase`, `clear`
  boolean-action, `kVs`/`currents`/`kWs`/`kvars` `double_v_array` over Fnphases,
  `conn` mapped enum, `DeltaDirection`, `%Error`, `Weight`); ctor defaults
  (3-phase, kVBase 12.47, weight/%error 1, dir +1, wye); `RecalcElementData`
  (terminal check 665 / no-element 666, adopt nphases/nconds + bus, then
  `AllocateSensorObjArrays`+`ZeroSensorArrays`+`RecalcVbase`), `RecalcVbase`
  (wye L-N ÷√3 / delta L-L), `RotatePhases`, `UpdateCurrentVector` (P/Q →
  per-phase current on Vbase), `TakeSample` (V/I capture; no gate yet), the WLS
  current/voltage error getters, `MakeLike`. `CalcYPrim` empty, `GetCurrents`
  zeros. Registered in `exec` after EnergyMeter (Pascal DSSClassDefs.pas:294);
  new `ElemKind::Sensor` + `Circuit::sensors` list.
- **Zone wiring:** `solution/meters.rs` `set_has_sensor_flag` (Pascal
  `TSensor.SetHasSensorFlag`, called from `ResetMeterZonesAll` after
  `SetHasMeterFlag`): clears `HasSensorObj` on all PD/PC, then marks each
  sensor's metered element (`HAS_SENSOR_OBJ` + `sensor_obj` back-pointer) so the
  existing zone walk passes the sensor down its zone (the WP6.4 propagation is
  gated on `!HAS_SENSOR_OBJ`, so a directly-sensored branch keeps its own).
- **Load allocation:** `load.rs` `set_allocation_factor` (Pascal
  `Set_AllocationFactor`, used by `AllocateLoad`) + `set_kva_allocation_factor`
  (Pascal `Set_kVAAllocationFactor`, the `Set AllocationFactors=` path — forces
  ConnectedkVA spec + re-tracks the dump order). `solution/meters.rs`
  `allocate_loads` (the `DoAllocateLoadsCmd` loop: guess solve, then
  `MaxAllocationIterations` passes of `CalcAllocationFactors` on every
  meter+sensor → `AllocateLoad` over each meter's zone → re-solve);
  `allocate_load_for_meter` scales each zone load by its upstream
  Sensor-or-EnergyMeter factor (single-phase = connected-phase factor,
  poly-phase = AvgAllocFactor). `exec`: the `allocateloads` command (ord. 45),
  `Set AllocationFactors=` (opt 48, error 271 on ≤0) and `Set NumAllocIterations=`
  (opt 72; `Dss.max_allocation_iterations` default 2).
- **Oracle facts (probed, then pinned):** `element=`/`conn=`/`deltadirection=`
  set `NeedsRecalc`, so a single `New Sensor … currents=…` **zeros** the measured
  arrays at `EndEdit` (`ZeroSensorArrays`); values survive only when set in a
  later `edit`. `MakeLike` copies *only* the shape/metered fields — kVBase /
  conn / %Error / Weight / DeltaDirection stay at the new object's ctor defaults
  and the arrays stay NIL (dump `''`, so the `double_v_array` getter returns
  `None` for an empty array). `Set AllocationFactors=0.8` → `kWbase =
  xfkVA·0.8·|pf|`; `allocateloads` drives the metered current toward the meter's
  default 400 A `SensorCurrent`.
- Tests: 5 exec integration (`allocateloads_meter_drives_zone`,
  `allocateloads_honors_numallociterations`, `set_allocation_factors_scales_all_loads`,
  `allocateloads_with_sensor`, `sensor_requires_element`) with kW/factor values
  transcribed from the oracle (`Loads.kW`/`AllocationFactor`); `Dss::load_alloc`
  test API. `gen_props.py`: 6 Sensor scenarios (default, single-command zeroing,
  two-step survival, P/Q→current, kVs+delta, makelike) → `props.json` regenerated
  (pure insertions); `props_roundtrip` green. dss-core lib 257 → **262**.
- **WP6.7 audit follow-up (gate-green):** closed coverage/faithfulness gaps the
  self-audit found. `do_allocate_loads_cmd` now forces `Mode := SNAPSHOT` before
  the guess solve (Pascal ExecHelper.pas l.2617; guarded `if Mode <> SNAPSHOT`,
  via `set_mode`) — previously omitted, latent once non-snapshot modes run.
  Comments added in `allocate_load_for_meter` documenting the `load_list`-vs-
  `BranchList` equivalence and the two intentional defensive guards (nil
  `SensorObj` / connected-phase past the sensor's phase count, where Pascal would
  deref-nil / read OOB).
- **New `allocation` golden gate** (`tools/golden/gen_allocation.py` →
  `tests/golden/allocation.json` → `tests/golden_allocation.rs`, the
  `gen_reliability.py` pattern): 6 scenarios replayed and matched per load on the
  oracle `Loads.kW`/`AllocationFactor` after `allocateloads` — 3-phase
  ConnectedkVA, `NumAllocIterations=4`, kWh/Cfactor spec (the `KwhPf`→`c_factor`
  branch), unbalanced single-phase (distinct `PhsAllocationFactor[ConnectedPhase]`,
  pinning the connected-phase index), a current-spec Sensor and a P/Q Sensor.
- **New `exec` unit tests** (kept alongside the golden for clearer per-case
  failure messages; the `TakeSample`/WLS ones have no COM getter and so can only
  live here — same split as reliability's per-branch accumulators):
  `allocateloads_kwh_spec_loads`, `allocateloads_single_phase_per_phase_factor`,
  `allocateloads_pq_sensor`, `sensor_take_sample_{wye,delta}` (via new
  `Dss::sensor_sample` API — the only gate exercising `TakeSample`'s
  offset/`RotatePhases` math, oracle-cross-checked against the metered element's
  currents + node voltages), and 3 `sensor.rs` unit tests (`rotate_phases_wraps`,
  `wls_voltage_error_matches_formula`, `wls_current_error_from_pq`). Net dss-core
  lib 262 → **270**; new `golden_allocation` integration target (1 test).

**WP6.8 (part 1/4) — GenDispatcher — ✅ done, gate-green.** Files:
- `elements/control/gen_dispatcher.rs` (new, `TGenDispatcherObj`): props 1–7 +
  the `TCktElementClass` tail (`Element` any-class ObjectRef, `Terminal`,
  `kWLimit`/`kWBand`/`kvarLimit`, `GenList` string-list, `Weights` IndirectCount
  double array sized by the GenList), ctor defaults (kWLimit 8000, kWBand 100,
  halfband 50, kvarLimit = kWLimit/2 = 4000), `PropertySideEffects` (kWBand →
  halfband; GenList → levelize: clear pointer list, FListSize = name count,
  realloc weights to 1.0), `RecalcElementData` (372 if no monitored element /
  371 on bad terminal / SetBus(1, monitored bus)), `MakeGenList` (named list →
  resolve each enabled generator keeping its weight; empty list → scan all
  enabled gens with uniform weights; sum TotalWeight), `Sample` (PDiff/QDiff vs
  band → weighted redispatch of each gen's `kWBase`/`kvarBase`, floored at
  1.0/0.0), and `MakeLike` — **ported verbatim incl. the Pascal quirk that it
  copies only nphases/nconds/monitored-element/terminal, so a `like=` dispatcher
  reverts the dispatch settings to ctor defaults** (oracle-confirmed). 10 inline
  tests (mock env). `DoPendingAction`/`Reset` are Pascal no-ops.
- **Control-loop wiring** (`solution/controls.rs`): a GenDispatcher reaches a
  *dynamic* generator set (not a fixed pair/triple), so it can't use
  `pair_mut`/`triple_mut`. New `GenDispatchEnv` trait abstracts the executive
  surface `Sample` needs (monitored terminal power + per-generator
  `kWBase`/`kvarBase` by ref); `dispatch_control` handles `GenDispatch` *before*
  building the shared `CtrlCtx` (it uses none of the event/Y context), cloning
  the dispatcher out so the env can hold the whole store, then copying the cached
  gen list back and — when any base changed — setting `LoadsNeedUpdating` and
  pushing a present-time control action (Pascal `ControlQueue.Push(0,0,0,Self)`).
- **Registration:** new class right after Generator (Pascal DSSClassDefs.pas:231),
  `ElemKind::Control` (joins `ckt.controls`, no Yprim, zero currents).
- **Tests:** 5 oracle-pinned `exec` integration tests (`gendispatcher_*`): equal
  weights → both gens 1511.569498763734 kW; weights [3,1] → 1767.354.../1255.784...;
  no GenList → dispatch all gens (same as equal); **kvar redispatch** (pf=0.95
  gens, kvarlimit binds → 1509.812.../591.260...); **monitored terminal=2**
  honored (gens floor at 1.0, distinct from terminal 1). 3 `props.json` scenarios
  (default, full, makelike-quirk; pure insertions, `props_roundtrip` green).
- **Audit follow-up (this commit):** closed the kvar-path coverage hole flagged by
  the WP6.8 audit — the previous unit/integration tests all suppressed the QDiff
  branch. Added 4 unit tests (kvar redispatch / kvar weights / `Max(0.0,…)` floor /
  unresolved-genlist subset) + the 2 integration tests above, and documented the
  deliberate deferrals (`MakePosSequence` unported = upstream NIL-deref crash;
  `Element` Required flag inert) and the one Pascal divergence (resolved-subset
  iteration vs Pascal's NIL-deref on a partially-resolved list). Net dss-core lib
  270 → **289**.
- **New golden gate `golden_gendispatcher`** (`tests/golden/gendispatcher.json`
  from `tools/golden/gen_gendispatcher.py`): 5 oracle-pinned scenarios replaying
  the full redispatch feedback loop and matching every generator's converged
  `kWBase`/`kvarBase` (equal weights, weighted [3,1], no-genlist, kvar redispatch,
  monitored terminal=2) at 1e-6 — pins the end-to-end path the inline `exec` tests
  spot-check.

**WP6.8 (part 2/4) — StorageController skeleton — ✅ done, gate-green.** Files:
- `elements/control/storage_controller.rs` (new, `TStorageControllerObj`): the
  **full** 37-prop table + the `TCktElementClass` tail, ctor defaults,
  `PropertySideEffects` (kW/%-band/kWBand sync incl. the upstream
  `FpctkWBand`-typo `TODO(compat)` at l.544, MODEFOLLOW→noon trigger, Seasons
  array resize, DispFactor clamp, InhibitTime floor), and the value-copying
  `MakeLike`. `RecalcElementData` ports the 371/372 monitored-element checks +
  `MakeFleetList`, which — with **no Storage class (Phase 7)** — always yields an
  empty fleet → error **37201** (or 14403 for a named-but-missing element),
  reproducing the oracle on a Storage-less circuit exactly.
- Two new enums in `obj/dss_enum.rs` (`storage_ctrl_discharge_mode` /
  `storage_ctrl_charge_mode`, non-sequential ordinals).
- **NOT_PORTED → Phase 7** (need the Storage element's live state): `Sample` +
  all `Do*Mode` dispatch, `SetFleet*`, `GetControlPower`, the kWh/kW fleet
  aggregates, `MakePosSequence`. Because the fleet is always empty here,
  `Sample`/`DoPendingAction`/`Reset` are inert no-ops — also the observable
  behavior. Wired into the control sweep (`solution/controls.rs`) as
  `ControlKind::StorageSkeleton` (returns `Ok` for every op).
- The 4 fleet-aggregate readbacks (`kWhTotal`/`kWTotal`/`kWhActual`/`kWActual`)
  are `SilentReadOnly + ReadByFunction` doubles whose `?` getter renders `''`
  regardless of fleet (verified vs the oracle **with and without** Storage);
  modeled as read-only strings → `''`.
- **Tests:** 13 inline unit tests (defaults, prop-table shape, each side-effect,
  recalc 372, MakeFleetList 37201/14403, MakeLike copy) + 1 `exec` integration
  test (`storagecontroller_skeleton_solves_as_noop`: a circuit with a
  StorageController solves, only the parse-time 37201 is logged) + **4 new
  `props.json` scenarios** (default, full, elementlist+weights, makelike). The
  props harness gained a per-scenario `allow_errors` flag (gen_props.py +
  `props_roundtrip.rs`) so the faithful 37201/14403 don't trip the "no engine
  errors" assertion. Net dss-core lib 289 → **303**.

**WP6.8 (part 2/4) audit follow-up — ✅ done, gate-green.** Three Pascal-fidelity
fixes from an audit against `StorageController.pas`:
- **`MakeFleetList` flag clear (l.1927):** the default branch (and a
  fully-resolved named branch) now clears `FleetListChanged`, while the
  missing-name path still `Exit`s with it set (l.1889). Without this a second
  `Edit` re-ran the fleet build and re-emitted 37201; now it doesn't.
- **`RecalcElementData` phase sync (l.803-804):** the control now adopts the
  monitored element's `Nphases`/`NConds` on a valid recalc, so a later `MonPhase`
  edit validates against the right phase count (was always vs the default 3).
- **`Sample` named-missing divergence documented:** for a specified-but-missing
  `ElementList`, Pascal `Sample` re-runs `MakeFleetList` and emits 14403 *per
  sample step*; the skeleton's blanket no-op defers that with the rest of
  `Sample` (Phase 7) — now spelled out in the module doc + `controls.rs`.
- Plus metadata-only `DynamicDefault`/`Units_hour` PropFlags added and applied
  (kWThreshold/kWBand/kWBandLow, Tup/TFlat/Tdn/InhibitTime) for table fidelity.
- **+4 unit tests** (default-recalc clears flag / no repeat 37201; named-missing
  keeps flag pending; MonPhase>nphases errors+resets; recalc syncs nphases).
  No golden regeneration needed (none of the fixes change a `?` dump). Net
  dss-core lib 303 → **307**.

**WP6.8 (part 3/4) — AutoAdd skeleton — ✅ done, gate-green.** Per PHASE6_PLAN
§2.6 only the option-bearing object is ported; the capacity-search `Solve` is
`NOT_PORTED`. Files:
- `circuit/auto_add.rs` (new, `TAutoAdd`): the public option struct
  (`gen_kw`/`gen_pf`/`gen_kvar`/`cap_kvar`/`add_type`/`mode_changed`) with
  `Init` defaults (GenkW=1000, GenPF=1, Capkvar=600, AddType=GENADD,
  ModeChanged=true) + `GENADD`/`CAPADD` consts. The private `Solve`-only state
  (`BusIdxList`, `LastAdded*`, loss/EEN accumulators) is intentionally omitted.
- `circuit/circuit.rs`: `auto_add_obj` + the auto-add circuit fields
  `ue_weight`/`loss_weight` (1.0), `ue_regs` (`[10]`), `loss_regs` (`[13]`),
  `auto_add_bus_list` (a `Vec<String>` stand-in for the Pascal
  `TBusHashListType` — enough for the `Get` echo; the hash dedup/`Find` is only
  needed by the unported `MakeBusList`).
- `obj/dss_enum.rs`: `AddTypeEnum` (`Generator`/`Capacitor` → GENADD/CAPADD,
  default CAPADD).
- `exec/mod.rs`: wired `Set`/`Get` for GenkW(29), GenPF(30), Capkvar(31),
  AddType(32), UEweight(35), Lossweight(36), UEregs(37), LossRegs(38),
  AutoBusList(42). New free helpers `parse_int_array` (Pascal `parseIntArray`),
  `do_auto_add_bus_list` (inline list **and** `File=` form, Pascal
  `DoAutoAddBusList`), and `int_array_to_string` (Pascal `IntArrayToString` →
  `[NULL]`/`[a, b]`). AddType `Get` echoes the lowercase device word
  (`generator`/`capacitor`), not the enum name.
- **Decision (per §2.6):** the AutoAdd *solve mode* stays its Phase-3 "Unknown
  solution mode" error — the search loop needs aux-current injection
  (`UseAuxCurrents`) + meter register sampling that land in a later phase. Not
  cheap, so deferred and documented here.
- **+4 unit tests** (defaults via `Get`; `Set`→state+`Get` round-trip incl.
  int-arrays + AddType word; inline AutoBusList round-trip; AutoAdd solve mode
  still deferred). No golden regeneration (the wired options were previously the
  "not ported yet" error; no existing golden exercised them). Net dss-core lib
  307 → **311**.

**WP6.8 (part 4/4) — ReduceAlgs basic — ✅ done, gate-green.** Per PHASE6_PLAN
§3 step 5 the WP is scoped to the option/command **surface**; the zone
reduction is `NOT_PORTED` because every strategy (`DoReduceDefault` &
siblings in `ReduceAlgs.pas`) hinges on the unported 210-line
`TLineObj.MergeWith` series/parallel line merge. Files:
- `circuit/circuit.rs`: `ReductionStrategy` enum (`Default`/`ShortLines`/
  `MergeParallel`/`BreakLoop`/`Dangling`/`Switches`/`Laterals`; Pascal
  `TReductionStrategy`, `rsTapEnds` removed upstream) + circuit fields
  `reduction_strategy`/`reduction_strategy_string` (""), `reduction_zmag`
  (0.02), `reduce_laterals_keep_load` (true).
- `exec/mod.rs`: wired `Set`/`Get` for ReduceOption(59), KeepLoad(112),
  Zmag(113). `set_reduce_strategy` ports `DoSetReduceStrategy` (first-char
  dispatch; `S` → Switch via `CompareTextShortest(S,'SWITCH')`, else
  ShortLines; unknown → error "Unknown Reduction Strategy" + falls back to
  rsDefault). `do_reduce_cmd` ports the `Reduce` command's energy-meter
  precondition (error 1890, exact message) and then logs a NOT_PORTED
  deferral for the reduction itself (no silent no-op). `ReduceZone` /
  `Interpolate` (l.2298) deferred with the merge.
- **+4 unit tests** (option defaults+round-trip incl. the empty-ReduceOption
  `Get` elision; first-char strategy dispatch incl. the `S` ambiguity + unknown
  fallback; Reduce no-meter 1890; Reduce-with-meter deferral). No golden
  regeneration (the wired options/command were previously the "not ported"
  error; no existing golden exercised them). Net dss-core lib 311 → **315**.

**WP6.8 (parts 3/4 + 4/4) — audit follow-up — ✅ done, gate-green.** Fixed
fidelity gaps found auditing the AutoAdd/ReduceAlgs surface against Pascal:
- `parse_int_array` (`parseIntArray`) no longer swallows the parser exception.
  It now does the Pascal two-pass (count → `SetLength` zero-fill → fill),
  **records** the `MakeInteger` conversion error, and stops at the bad token —
  so `Set UEregs=(10 abc 13)` → `[10,0,0]` + logged error (was silently
  `[10,0,13]`). The roundable-decimal path (`13.7 → 14`) is unchanged.
- `do_reduce_cmd` now ports `MarkCapandReactorBuses` (marks enabled shunt
  cap/reactor buses `Keep`, *before* the meter check, exactly as Pascal) and
  the named-meter path: a missing meter is error 262 `EnergyMeter "X" not
  found.` (uppercased name) instead of the generic deferral; `'A'`/all-meters
  and a resolved meter still log the NOT_PORTED deferral.
- `do_auto_add_bus_list` File= read error now matches Pascal code 268
  (`Error trying to read bus list file: %s`).
- **+4 tests** (non-numeric reg token logs error + truncates; unknown
  `addtype` → CAPADD default, no error; Reduce named-meter 262; Reduce marks
  cap/reactor buses) and the 1890 test tightened to pin the full URL. Net
  dss-core lib 315 → **319**.
- **New golden gate** `golden_autoadd_reduce` (10 oracle-pinned scenarios) vs
  `tests/golden/autoadd_reduce.json` from `tools/golden/gen_autoadd_reduce.py`:
  replays the AutoAdd/Reduce option surface and matches every `Get` echo
  byte-for-byte, plus `addtype=foo`→`capacitor` (no error), `ueregs=(10 abc 13)`
  → `[10, 0, 0]` + conversion error (oracle #303), `13.7`→`14`, Reduce #1890 /
  #262. `Bus.Keep` and the reduction itself aren't exposed by the COM API, so
  they remain pinned by the unit tests. (Generated with the pinned oracle —
  python 3.12.4 / dss-python 0.15.7 / backend 0.14.5.)

WP6.8 complete (4/4). Next: WP6.9 (goldens + 8500-node gate). Note for WP6.9:
the `Interpolate` command (Run_8500Node calls it after solve) is unported, so
drop it from any 8500 replay or port it then.

---

**WP6.9 — Goldens + the 8500-node gate — ✅ done, gate-green.** Two new
oracle-pinned golden gates (no new lib tests — all golden-harness driven), both
generated with the pinned oracle (python 3.12.4 / dss-python 0.15.7 / backend
0.14.5):

- **`golden_ieee8500.rs` vs `tests/golden/ieee8500.json`** (the headline
  Phase-6 gate; `tools/golden/gen_ieee8500.py`). Compiles the **unmodified**
  `8500-Node/Master.dss`, then per `Run_8500Node.dss`: `New Energymeter.m1
  Line.ln5815900-1 1`, `Set Maxiterations=20`, `Solve` (snap), then a 24-step
  daily segment for register integration. The Rust engine matches:
  converged + total iterations **exactly (67)**; `YNodeOrder` exactly (**8531
  nodes**); node voltages + total power + losses at 1e-6 rel; the **12
  RegControl tap numbers** (4 banks) + **10 capacitor states** exactly + the 12
  regulated transformer taps (1e-12 rel) — the controls-at-scale regression;
  all **67 EnergyMeter registers at 1e-4 rel** (names exact) after the daily
  integration (zone kWh, line/xfmr loss split, seq + voltage-base buckets,
  EEN/UE). The golden stores only the 12 *moved* transformer taps; the gate
  **also asserts every other transformer reads 1.0** (the ~1178 fixed load xfmrs
  + substation), so a spurious tap on an *uncontrolled* transformer is caught,
  not just the regulated set. `Interpolate` + all Show/Export/Plot dropped
  (file/UI, Phase 8). **Solve time: 0.19 s release** (full compile + snap +
  24-step daily + assertions; oracle daily-solve ≈ 0.12 s) — well within the 5×
  budget; 4.1 s debug, so kept un-`#[ignore]`d.
- **`golden_phase6.rs` vs `tests/golden/phase6/*.json`** (one file per scenario;
  `tools/golden/gen_phase6.py`,
  command-replay like phase5; reuses the inline IEEE13 from `gen_phase5`). Four
  scenarios:
  - `monitor_daily_ieee13`: IEEE13 (controls active) + daily shape + monitors on
    `line.650632` (modes 0/1/5) and `transformer.reg1` (mode 2); 24 daily steps.
    Per-monitor header (data channels, vs our full-Pascal `header[2..]`) +
    `SampleCount` exact; channel sample arrays elementwise at **1e-6 rel / 1e-4
    abs** (PHASE6_PLAN §1.2 — the daily fixed-point path tracks the oracle to
    ~1e-9 since the `build_y_matrix` load-Yeq restamp, commit `a6903f1`). Only the
    **mode-5 wall-clock channels 10/11** (`SolveSnap_uSecs`/`TimeStep_uSecs`) are
    skipped; the iteration-count channels 0/1 now match exactly. **Plan deviation
    (empirical):** the plan named `line.671680`, but bus 680 is a dead-end stub
    (charging current only → noise-dominated angle); switched to the feeder head
    `line.650632`.
  - `meter_daily_ieee13`: IEEE13 + daily shape + `energymeter.m1` on
    `line.650632`; 24 steps. Registers **1e-4 rel** (PORTING_PLAN §4 energy
    policy); the overload/EEN/UE threshold-crossing energies are **nonzero and
    pinned** (Overload kWh Normal ≈12642, Load EEN ≈18207, Load UE ≈936) — they
    match at 1e-4 since the Yeq restamp; names exact; zone branch/end/PCE counts
    exact (13/6/17).
  - `generator_snap`: IEEE13 + two generators (model 1 PQ wye @675 + model 3 PV
    delta @634); snapshot. Iterations exact (**15**), node order exact, voltages
    1e-6 rel, each generator's terminal powers 1e-6 rel (via `snapshot_elements`).
  - `meter_zone_micro`: a hand-built radial with a branch + mid-feeder sub-meter
    — `AllBranchesInZone`/`AllEndElements`/`ZonePCE` exact for both meters (the
    parent zone stops at the sub-meter: m1 = [l1,l4]/[l4]/[ld4], m2 =
    [l2,l3]/[l3]/[ld3]).

**WP6.10 — phase exit — ✅ this update, gate-green.** `TODO(compat)` /
`NOT_PORTED` marker sweep clean: every site points at its phase. One stale
marker fixed — `solution/meters.rs` `is_zone_pce`'s `TODO(WP6.8)` ("add
PVSystem/Storage to the zone allow-list when those PC classes land") was
repointed to **`TODO(WP7)`**: PVSystem/Storage are Phase 7 (DER sub-block), not
WP6.8 (the old §7 note that scheduled them for WP6.8 was a misattribution — they
have no objects until Phase 7, so the allow-list is correct-by-vacuity now). The
only other forward markers are the dormant reliability `GetOCPDeviceType`
(`TODO(WP7)` — OCP devices are Phase 7) and the AutoAdd/ReduceAlgs `NOT_PORTED`
skeletons (each documents its blocking dependency — aux-current injection /
`TLineObj.MergeWith`). Full gate re-run green from a clean tree:
`cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`
(0 warnings); `cargo test --workspace` — every target passes (dss-core lib 319;
the golden gates incl. `golden_ieee8500` 4.3 s debug / 0.19 s release;
`corpus_manifest` 1; `corpus_live` 2 auto-skipped; dss-parser 62+1; dss-sparse
5). **Phase 6 is complete; next is Phase 7** — write `PHASE7_PLAN.md` first
(DER, protection, line constants, harmonics, dynamics; PORTING_PLAN.md
§Phase 7).

**WP6 testing audit follow-up — ✅ done, gate-green.** A self-audit of the WP6
testing changes (the per-file golden split + the live corpus gate) found real
holes; all fixed and verified:
- **Silent-pass holes closed.** The per-file split (`fcda714`) made the
  directory-reading gates pass vacuously on an empty dir. `golden_phase5.rs` now
  pins the scenario count (`assert_eq! == 4`) — previously *no* count/non-empty
  guard, so an emptied `tests/golden/phase5/` would have passed with zero
  assertions; `golden_checkpoints.rs` now pins `== 3` (was only non-empty).
  `golden_phase6.rs` already pinned `== 4`.
- **8500 fixed-tap coverage.** `golden_ieee8500.rs` now asserts every transformer
  *not* in the golden's moved set reads 1.0 (the ~1178 fixed load xfmrs +
  substation), catching a spurious tap on an *uncontrolled* transformer — the
  moved-only golden previously checked only the 12 regulated ones.
- **Live gate now exercises meters/monitors/multi-step/YPrim.** Previously every
  `solvable_now` case was a 1-step snapshot with no `selected_elements` and no
  meter/monitor, so `corpus_live.rs` never ran the multi-step, YPrim,
  `compare_monitor`/`compare_meter` paths. The `IEEE13Nodeckt.dss` case is now a
  24-step daily run with `energymeter.m1` + 3 deterministic-mode monitors +
  `selected_elements`; new `harness::{compare_monitor,compare_meter}` (reusing
  `Dss::monitor_view`/`meter_registers`/`meter_zone`) are gated per case by a new
  `check_meters_monitors` manifest flag. Verified live: **16/16 solvable cases
  match the oracle** (`DSS_LIVE_ORACLE=1`).
- **Live gate is no longer CI-decorative.** Added the **`live-oracle`** GitHub
  Actions job (`.github/workflows/ci.yml`) that installs the pinned oracle and
  runs `DSS_LIVE_ORACLE=1 corpus_live` — the plan's "dedicated pinned-oracle CI
  job", previously unimplemented (the live gate ran in *no* automated gate).
- **Classify panic-hook hazard removed.** `corpus_live_classify` no longer
  overrides the global panic hook (which would swallow a sibling test's panic
  message); it relies on the `catch_unwind` payload it already captures.
- **Oracle quirk documented (not hidden).** The new comparator surfaced that the
  pinned dss-python returns a phantom `Monitors.Channel(i)` element for an
  *unsampled* monitor (`SampleCount==0`, `len(Channel)==1`; EPRI J1 `subVI`);
  Rust is self-consistent. Hence the opt-in scoping above + a `TOLERANCE_NOTES.md`
  entry. (STATUS §1d tolerances re-synced: the monitor channels are **1e-6/1e-4**
  and registers **1e-4** — the earlier "2e-4 / 1e-3" text predated the `a6903f1`
  Yeq restamp and was stale.)
