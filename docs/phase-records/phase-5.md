# Phase 5 record

> Archived from `STATUS.md` (moved 2026-06-16 to keep the live handoff lean).
> Frozen historical work-package log; superseded only by the code + tests.
> Section references (`§3.x`, `§5` …) point back to `STATUS.md`.

## 1c. Phase 5 record (branch `phase-5-controls-timeseries`)

Execution plan: **`PHASE5_PLAN.md`** (WP5.1–WP5.10).

**WP5.1 — XYcurve — ✅ done, gate-green.** Files:
- `src/elements/general/xy_curve.rs` (`TXYcurveObj`): props 1–13 via
  `define_properties!`; parallel `XValues`/`YValues` arrays; `GetYValue`
  (hunt-cache linear interp + end-extrapolation, ported verbatim 0-based),
  `GetXValue` (axes-swapped, no cache), the `X`/`Y` scalar accessors with the
  `FX/FY` + shift/scale synch (`Set_X`→`GetYValue`, `Set_Y`→`GetXValue`),
  `SetPoints`/`GetPoints`, `PropertySideEffects` (npts realloc, `Xarray`/`Yarray`
  → first-point `X`/`Y` sync, `LastValueAccessed` reset over props 2–7),
  `MakeLike`. 6 inline tests. `CSVFile`/`SngFile`/`DblFile` `NOT_PORTED`.
- **Shared engine additions:** `PropType::DoublePoints` (interleaved `(x,y)`
  list) + `PropDef::double_points`, routed through new
  `DssObject::get_points`/`set_points`; `util::interpret_dbl_array_dynamic`
  (read all doubles, count unbounded — the `DoubleDArrayProperty` parse path).
- **Oracle bug (traced):** `points=` (write) raises an **access violation** in
  the pinned oracle (a dss_capi `DoubleDArrayProperty` bug, both `(...)` and
  `[...]` forms). Goldens therefore drive the arrays via `XArray`/`YArray` and
  validate the `Points` *getter* by readback; the `SetPoints` path is ported
  faithfully and covered by a Rust-only unit test. The `Points` getter's NIL
  fallback is `[ 0 0]` (a single `(0,0)` point), reproduced exactly.
- **Goldens:** 7 XYcurve scenarios in `gen_props.py` (default, arrays, abbrev,
  shift+scale, x-accessor edit, npts-shrink, makelike); `props.json` regenerated
  (pure insertions); `props_roundtrip` green. dss-core lib tests 132 → 138.
- Reactor `RCurve`/`LCurve` stay `NOT_PORTED` (only consumed by the harmonic
  `CalcYPrim`, Phase 7); the module note was updated to say so.

**WP5.2a — LoadShape (in-memory core) — ✅ done, gate-green.** Files:
- `src/elements/general/load_shape.rs` (`TLoadShapeObj`): props 1–22 via
  `define_properties!`; fixed/variable-interval data (`p_mult`/`q_mult`/`hour`
  as `Option<Vec<f64>>` = Pascal `Assigned` semantics, empty parse = NIL),
  `GetMultAtHour` (even-interval wraparound with FPC-`Round` `TODO(compat)`;
  hour-array hunt-cache interpolation incl. `Avg`/`Edge`; `Set_Result_im`),
  `Normalize` (`BaseP`/`BaseQ` vs peak), `SetMaxPandQ` (peak P + coincident Q),
  lazy `Mean`/`StdDev` (recomputed unless set; `RCDMeanAndStdDev` even-interval
  / `CurveMeanAndStdDev` over `hour`), `SInterval`/`MInterval` scale-aliases of
  `Interval`, `MakeLike`. 13 inline tests. Single arrays (`sP`/`sH`/`sQ`) + MMF
  not ported; **`CSVFile`/`SngFile`/`DblFile`/`PQCSVFile` `NOT_PORTED`** here
  (CSVFile → WP5.2b once the executive can resolve the path).
- **Shared engine additions:**
  - `PropType::Action` (Pascal `StringEnumActionProperty`): the parsed value
    maps to an enum ordinal and immediately runs `DssObject::do_action`
    (default no-op); the getter is always `""`. `PropDef::action(name, enum)`.
  - `define_properties!` gained an **`enums <ident>` clause** so builder
    expressions can reference the registry (`enums.load_shape_action`) under
    the caller's hygiene — the no-enums form is unchanged (xy/tcc/spectrum).
  - `EnumRegistry`: `load_shape_action` (Normalize/DblSave/SngSave→0/1/2) and
    `load_shape_interp` (Avg/Edge→0/1).
- **Oracle facts:** `Action` getter is `""`; reading `Mean`/`StdDev` on an
  *empty* shape **raises 61107** (no data) — `gen_props.py` gained a
  `skip_props` hook to omit them for the `loadshape_default` scenario only.
  `QMax` (when not set) = Q at the index where |P| peaks (not max Q).
  `SetMaxPandQ` overrides `pmax=` when data is present.
- **Goldens:** 12 LoadShape scenarios in `gen_props.py` (default, fixed,
  abbrev, p+q, s/m-interval, hour-array, normalize ×2, interp=edge,
  explicit mean/stddev, makelike); `props.json` regenerated (pure insertions);
  `props_roundtrip` green. dss-core lib tests 138 → 151.

**WP5.2b — LoadShape `CSVFile` — ✅ done, gate-green.** Files:
- `load_shape.rs`: `CSVFile` un-`NOT_PORTED`; `read_csv_file` = Pascal
  `ReadCSVFile` (double, non-MMF): one row per point via the comma/whitespace
  aux parser, fixed-interval = one `mult`, variable (`Interval=0`) =
  `hour, mult`; reads ≤ `NumPoints` rows and shrinks `NumPoints` to the count
  read. `side_effects(CSVFile)` queues the read; `take_file_loads`/
  `apply_file_load` do it. 5 new tests incl. a full executive round-trip
  (temp CSV resolved via `current_dir`, oracle-transcribed values) and the 613
  missing-file path. (`SngFile`/`DblFile`/`PQCSVFile` stay `NOT_PORTED`.)
- **Shared engine addition — deferred file loads (`FileLoad`):** a property
  setter that names a data file can't reach the filesystem/current dir, so it
  queues a `FileLoad { prop, filename }` (parallel to `RefAction`); the
  executive drains it **before `end_edit`**, resolves the path relative to
  `current_dir` (like Redirect), reads the text, and calls
  `DssObject::apply_file_load` so the object parses it. `edit_active` binds
  `current_dir` and runs the loop; a missing file is Pascal error 613.
- **No props golden for CSVFile:** the command embeds a file path that resolves
  against the cwd, which differs between the oracle (repo root) and the Rust
  test (crate root) — a portable shared command string isn't possible, so the
  full path is validated by the executive integration test instead (values
  transcribed from the pinned oracle). dss-core lib tests 151 → 156.

**WP5.2c — TempShape (`TShape`) + PriceShape — ✅ done, gate-green.** Files:
- `scalar_shape.rs` (`ScalarShapeCore`): the data + the three byte-identical
  algorithms shared by both classes — `get_value_at_hour` (Pascal
  `GetTemperature`/`GetPrice`), the lazy `mean`/`std_dev` (`CalcMeanandStdDev`),
  and `read_csv_file` (`DoCSVFile`). These are the **legacy 1-based** Pascal
  lookups (init `LastValueAccessed := 1`, loop `for i := LastValueAccessed + 1`,
  fall-through returns the **last** point) — deliberately *not* derived from
  LoadShape's modernized 0-based `GetMultAtHour` (which falls through to the
  second-to-last point). Same FPC-`Round` `TODO(compat)` on the even-interval
  index.
- `temp_shape.rs` (`TShapeObj`, class **`TShape`**) and `price_shape.rs`
  (`PriceShapeObj`): the per-class property tables (props 1–12: `NPts, Interval,
  Temp|Price, Hour, Mean, StdDev, CSVFile, SngFile, DblFile, SInterval,
  MInterval, Action`) and the differing `PropertySideEffects`. Thin `DssObject`
  impls delegating to the core. 10 + 7 inline tests.
- **Oracle facts / class differences (probed, then pinned):**
  - Empty-shape `Mean`/`StdDev` return **0** with no error (unlike LoadShape's
    61107 — `CalcMeanandStdDev` sets the calculated flag even at `npts=0`), so
    no `skip_props` is needed.
  - **TempShape has no `Hour→Interval:=0` coupling**: giving `Hour` without an
    explicit `interval=0` leaves it a fixed-interval curve (golden
    `tshape_hour_no_interval` pins `Interval=1`, `Mean=2.333…`). **PriceShape
    auto-zeroes** `Interval` on `Hour` (golden `priceshape_hour` →
    `Interval=0`, trapezoid `Mean=2.5`), and drops the hour array when a
    positive `Interval` is set.
  - PriceShape's `Interval`/`SInterval`/`MInterval` are **not** `NonNegative`
    (TempShape's are); both classes' `Action` is only `DblSave`/`SngSave`
    (binary output) → `do_action` records a `NOT_PORTED` message (no
    `Normalize`).
  - `CSVFile` reuses the WP5.2b deferred-`FileLoad` path verbatim;
    `SngFile`/`DblFile` (binary input) stay `NOT_PORTED`.
- **Shared engine:** two enums added to `EnumRegistry`
  (`t_shape_action`/`price_shape_action`, both `DblSave/SngSave`); both classes
  registered in the executive.
- **Goldens:** 8 TShape + 6 PriceShape scenarios in `gen_props.py`;
  `props.json` regenerated (pure insertions, +310 lines); `props_roundtrip`
  green. dss-core lib tests 156 → 173.

**WP5.3 — shapes wired into Load + VSource — ✅ done, gate-green.** Files:
- `pc/load.rs`: the five shape refs (`yearly`/`daily`/`duty`/`CVRcurve` →
  `LoadShape`, `growth` → `GrowthShape`) became resolved `object_ref_class`
  props. Each is **snapshot-cloned** into the Load at `set_object_ref` time
  (`*_shape_obj: Option<LoadShapeObj/GrowthShapeObj>` + the resolved `ElemRef`),
  exactly like `FetchLineCode` (§3.4): the solve path only carries scalar
  `SysCtx`, so the owned clone is what `SetNominalLoad` drives through
  `GetMultAtHour`. Ported `CalcDailyMult`/`CalcDutyMult` (daily fallback)/
  `CalcYearlyMult`/`CalcCVRMult` setting `shape_factor`/`shape_is_actual`;
  `GrowthFactor` now reads `GrowthShapeObj.GetMult(Year)`; `SetkWkvar` +
  the `UseActual` shape side effects (`yearly`/`daily`/`duty` set kW/kvar to the
  shape's peak demand; `daily` seeds an unset `yearly`); the full `SetNominalLoad`
  mode dispatch (SNAPSHOT/HARMONIC unchanged; DAILY/YEARLY/DUTYCYCLE now consult
  the shape; CVR loads add `CalcCVRMult` in YEARLY). `MakeLike` copies the
  resolved shapes. 5 inline tests probed against the oracle.
- `pc/vsource.rs`: same conversion for `yearly`/`daily`/`duty` (LoadShape only),
  `CalcDailyMult`/`Duty`/`Yearly`, and the loadshape-mode magnitude in
  `GetVterminalForSource` (`Vmag = kVBase·ShapeFactor.re·…`, or `1000·re` when
  `UseActual`). 2 inline tests.
- **Shared engine:** none — reuses Phase 4's `object_ref_class`/`set_object_ref`
  resolution and the `SolveMode` enum. `LoadShapeObj` gained `use_actual()`/
  `max_p()`/`max_q()` accessors.
- **Oracle facts (probed, then pinned in the unit tests):** in DAILY/YEARLY/DUTY
  the per-conductor power is `kW·mult·1000/Nphases` (Q scales the same when no
  QMult); a `UseActual` daily shape sets the load to `(MaxP, coincident MaxQ)`
  and the ObjectRef getter renders `YearlyShapeObj.Name` (so `daily=d1` with no
  `yearly` makes `Get yearly` return `d1`). No new goldens: the existing feeders
  reference no shapes (snapshot paths stay byte-identical), and a daily-mode
  `solve` can't run until the WP5.8 dispatcher lands — `SetNominalLoad` is
  validated directly with a `SolveMode::Daily` `SysCtx`.
- **Snapshot-clone limitation:** a later `edit loadshape.x` is not seen by loads
  that already resolved it (Pascal keeps a live pointer). Documented; no corpus
  case re-edits a referenced shape. The circuit's built-in `default` LoadShape +
  `DefaultDailyShapeObj`/`DefaultYearlyShapeObj` and the global `DefaultHourMult`
  path are **deferred to WP5.8** (only `SolveDaily` consumes them).
  dss-core lib tests 173 → 181 (+7 element + 1 executive resolution).

**WP5.4 — ControlQueue + event log — ✅ done, gate-green.** Files:
- `src/solution/control_queue.rs` (`TControlQueue`): `TimeRec`
  (`hour`/`sec` + `to_time`), private `ActionRecord`, and `ControlQueue`
  (ordered `Vec`, `ctrl_handle` serial). Ported verbatim: `push` (Sec>3600
  hour-normalization + insert-before-first-`>=`-time, so a later equal-time
  push lands *ahead* of an earlier one — tie-break is observable), `push_delay`
  (the `Delay` overload), private `pop` (linear scan for earliest time `<= t`),
  `delete` by handle, `clear`/`is_empty`/`queue_size`, and the three
  pure-queue dispatchers `do_all_actions`/`do_nearest_actions`/`do_actions`.
  Dispatch routes through the **`ControlActioner` trait** (`do_pending_action(
  control: ElemRef, code, proxy, &mut ControlQueue)`): because `pop` returns
  owned (`Copy`) record data, the queue hands *itself* to the actioner, so an
  action may push/delete further records mid-sweep (RegControl EVENTDRIVEN
  re-arm). 7 inline tests (handle/time ordering, equal-time tie-break,
  Sec>3600, delete-by-handle, nearest-bucket-only, mid-sweep re-arm,
  do_all+clear) hand-traced from the Pascal — no oracle exposes the queue.
- `src/solution/event_log.rs` (`EventLog` = `DSS.EventStrings`): the two
  producers ported with exact `Format` strings — `log_this_event` (Pascal
  `TDSSContext.LogThisEvent`, `Hour=…, Sec=%-.8g, Iteration=…, ControlIter=…,
  Event=…`) and `append` (`TDSSObject.AppendToEventLog`, `Hour=…, Sec=%-.5g,
  ControlIter=…, Element=…, Action=…` with `AnsiUpperCase(action)`). 2 inline
  tests. The gate normalizes numbers out, so the `%g` time fields are faithful
  but not load-bearing.
- **Shared engine:** `Solution` grew `control_queue: ControlQueue` and
  `event_log: EventLog` (constructed in `new`; driven by WP5.5/5.7);
  `Dss::event_log()` surfaces the lines (oracle reads `Solution.EventLog`).
  `util::fmt_g(v, sig)` = C `%.*g` (sci for exp `<-4`/`>=sig`, trailing-zero
  strip) for the event-log time fields.
- **Deferred to WP5.7/5.8:** `do_multi_rate` (Pascal `DoMultiRate` needs
  `Pop_Time`/keepIn plus mid-sweep `SolveCircuit`/`SampleControlDevices` and
  DynaVars `Recalc/Restore_Time_Step` — none exist until the control loop +
  DynaVars land); the queue's `DebugTrace`/`WriteTraceRecord` file (RegControl
  debug-trace, WP5.5+); `WriteQueue`/`QueueItem` (Show, Phase 8); the
  `EventLogDefault` global (default `False` already matches controls'
  `show_event_log` default — wire when a gate flips it). dss-core lib tests
  181 → 190 (+7 control_queue + 2 event_log).

**WP5.5 — RegControl behavior — ✅ done, gate-green.** Files:
- `reg_control.rs`: ported `Sample` (top-to-bottom: maxtapchange-0 early exit,
  the reverse/cogen power-direction block incl. `ReverseNeutral`, the
  regulated-bus vs `GetWindingVoltages` control-voltage paths, `GetControlVoltage`
  PTphase selection, the Vlimit local-bus check, R+jX / Beckwith `LDC_Z` line-drop
  compensation, the band test with the InReverse tap-divide, the
  `Round(BoostNeeded/Increment)·Increment` pending-tap + winding/reverse
  direction flip, and the arm/disarm queue push/delete) and `DoPendingAction`
  (CTRLSTATIC `AtLeastOneTap`; EVENT/TIME/MULTIRATE `OneInDirectionOf` + re-push;
  `ACTION_REVERSE` mode toggle), plus `AtLeastOneTap`/`OneInDirectionOf`/
  `ComputeTimeDelay`/`GetControlVoltage`/`set_PendingTapChange`/`VLimitActive`.
  New runtime fields (`last_change`, `control_action_handle`, `rev_handle`/
  `rev_back_handle`, `in_reverse_mode`/`reverse_pending`/`in_cogen_mode`,
  `controlled_phase`). The pending-tap `Round` uses `round_ties_even`
  (`TODO(compat)` FPC banker's rounding — **the single most tap-sensitive line**).
  `sample`/`do_pending_action` are `pub(crate)` + `#[allow(dead_code)]` (wired by
  the control loop in WP5.7). 7 inline tests against a mock transformer
  (out-of-band-high arms a downward tap, in-band disarm, CTRLSTATIC ≥1-tap apply
  + `system_y_changed`, EVENTDRIVEN one-tap + re-push, event-log line,
  inverse-time delay, maxtapchange=0 exit).
- **Shared engine:** the controls' shared `CtrlCtx` (PHASE5_PLAN §2.1 disjoint
  borrow: `node_v`/`sys`/`queue`/`events`/`errors`/`system_y_changed` + the
  `int_hour`/`t`/`dbl_hour`/`control_iter` scalars + `control_mode` + `self_ref`)
  and the `CTRL_NONE`/`OPEN`/`CLOSE` action codes live in `control_elem.rs`.
  Transformer gained the **`ControlledTransformer` trait** (`Sample`/`DoPending`'s
  read/mutate surface — `winding_voltages` = `GetWindingVoltages`, `power_into_re`
  = `Power[t].re`, `terminal_currents` = `GetCurrents`, tap getters/`set_present_tap`,
  `wdg_connection`/`base_voltage`/`rotate_phases`) so the regulator logic is
  unit-testable against a mock; `set_present_tap` now returns "Y must rebuild"
  (Pascal `Set_YprimInvalid`'s `SystemYChanged` trigger, gated by `Enabled`).
  Control-mode ordinals `EVENTDRIVEN`/`TIMEDRIVEN`/`MULTIRATE` added to
  `solution`. **Indexing note:** the control's voltage/current buffers and
  `controlled_phase` are 0-based in the port (Pascal `VBuffer`/`CBuffer`/
  `ControlledPhase` are 1-based); the LDC pickup index is
  `nconds·(term−1) + controlled_phase`.
- **Deferred:** `RegWriteTraceRecord`/`RegWriteDebugRecord` debug-trace file
  (DebugTrace flag stored, no file); `MakePosSequence`. dss-core lib tests
  190 → 197 (+7 RegControl behavior).

**WP5.6 — CapControl behavior — ✅ done, gate-green.** Files:
- `cap_control.rs`: ported `Sample` (PresentState from the bank's `Closed[0]`;
  the voltage-override block; the `ControlType` dispatch —
  Current/Voltage/kvar/Time/PF; the `Delay`/`DelayOff`/`DeadTime` arm-on-queue
  + the `Armed && PendingChange=None` disarm/delete) and `DoPendingAction`
  (open/close all phases + AddStep/SubtractStep, multi-step step-up/down, event
  log), plus `GetControlCurrent`/`GetControlVoltage` (the `mon_phase`
  avg/max/min → −1/−2/−3 selection, delta L-L on the controlled cap's
  connection), `Set_PendingChange`, the `pf_1to2` PF mapping, and
  `TimeOfDay(useEpsilon)` for time control. New runtime fields
  (`pending_change`, `present_state`/`initial_state`, `armed`,
  `voverride_event`, `control_action_handle`). `sample`/`do_pending_action` are
  `pub(crate)` + `#[allow(dead_code)]` (wired by the control loop in WP5.7). 10
  inline tests against mock cap + mock monitored element (kvar arm-close/
  arm-open/in-band, single-step open/close, multi-step step-down, time-window
  close, PF leading-room close, event-log line, `pf_1to2`).
- **Shared engine:**
  - **`ControlledCapacitor` trait** (`capacitor.rs`, mirroring
    `ControlledTransformer`): `num_steps`/`available_steps`/`total_kvar`/
    `connection`/`is_closed`/`set_closed`/`add_step`/`subtract_step`/
    `full_name`, with `Capacitor` the production implementor. New private
    Capacitor methods `add_step`/`subtract_step` (Pascal verbatim — `set_states`
    invalidates Y on change), `available_steps`, and terminal-1 conductor
    open/close (`Closed[0]` get/set → `cd.yprim_invalid`).
  - Two **default `CktElement` methods** for the *generic* monitored element:
    `get_term_voltages` (Pascal `TDSSCktElement.GetTermVoltages`) and
    `terminal_power` (Pascal `Get_Power(idxTerm)`).
- **Design note — Sample's two trait objects:** `sample(cap: &mut dyn
  ControlledCapacitor, mon: &mut dyn CktElement, ctx)`. For Current/Voltage/
  kvar/PF the monitored element ≠ the capacitor; for Time control `mon` is the
  capacitor and is **not** read. WP5.7 must obtain both from the foreign view
  (the same-`ElemRef`/double-`&mut` case only arises for Time control, where
  `mon` is unused — pass a scratch).
- **Deferred:** `VOverrideBus` voltage path (`GetBusVoltages` from a named bus
  needs solve-time bus resolution; `VoverrideBusSpecified` is always reverted at
  parse, so the branch is unreachable — sense the monitored terminal instead);
  `FOLLOWCONTROL` (ControlSignal/LoadShape is `NOT_PORTED` — Pascal aborts the
  solution when unset, which is always the case here, so `Sample` records that
  error); `USERCONTROL` (no DLLs); `Reset`'s `Closed[0]` restore (needs the
  cap — control-loop reset path, WP5.7); `MakePosSequence`. dss-core lib tests
  197 → 207 (+10 CapControl behavior).

**WP5.7 — control loop + `Sample_DoControlActions` — ✅ done, gate-green.**
- `src/solution/controls.rs` (new): `sample_do_control_actions` /
  `sample_control_devices` / `do_control_actions` (`Solution.pas` l.1941–2008),
  `reset_all_controls` (`Utilities.DoResetControls`) and the verbatim
  `do_multi_rate` (`ControlQueue.DoMultiRate` incl. `Recalc/Restore_Time_Step`
  and the `Temp_Int`/`Temp_dbl` scratch choreography; it solves the circuit and
  re-samples mid-sweep, so it lives at the solution level, not on the queue).
  All four control modes dispatch (CTRLSTATIC nearest-ignoring-time /
  EVENTDRIVEN advancing `intHour`/`t` / TIMEDRIVEN `do_actions` / MULTIRATE).
- **The dispatch core** (`dispatch_control`): resolves the control's `ElemRef`s
  against the registry and splits the mutable borrows per PHASE5_PLAN §2.1 —
  `ElemStore` grew `obj`/`pair_mut`/`triple_mut` (implemented in the
  executive's `ClassStore` via `get_disjoint_mut` at the class and object
  levels), `DssObject` grew **`as_any_mut`** (all 17 classes). RegControl
  pairs with its Transformer; CapControl triples with capacitor + monitored
  element; for Time/Follow control (monitored == controlled) the monitored
  role gets a *clone* of the capacitor (read-only role; covers the
  voverride-with-time branch the WP5.6 "scratch" note missed). The control
  queue is `std::mem::take`n out of the solution per sweep, so actions can
  push/delete further records mid-sweep exactly like Pascal.
- `check_controls` (real): converged → log "Control Iteration N" (gated
  `ckt.LogEvents`) → sample/act → Y rebuild keeping voltages;
  `solve_snap` gained the exact 485 warning ("Warning Max Control Iterations
  Exceeded.\nTip: …") + `solution_abort` and the "Solution Done" log.
  **All Pascal `LogThisEvent` call sites ported** (probed: the oracle's
  `Set Log=yes` log includes them): "Solution Iteration N" / "Solve Sparse Set
  DoNormalSolution ..." (DoNormalSolution), "Initializing Solution"
  (DoPFLOWsolution), "Solve Sparse Set ZeroLoadSnapshot ...", and Ymatrix's
  "Recalc All/Invalid Yprims" / "Building Whole/Series Y Matrix" /
  "Reallocating Solution Arrays".
- RegControl `do_pending_action` now **syncs `tap_snap`** after applying a tap
  (Pascal's `Get_TapNum` reads the live transformer; our snapshot must track
  the control-action mutation path or the `TapNum` getter/dump goes stale).
  CapControl gained `reset_with(cap)` (the full Pascal `Reset` incl. the
  `Closed[0] := InitialState` restore). `Set mode=` now runs
  `reset_all_controls` (the Pascal `Set_Mode` tail).
- **External-command abort reset:** CAPI `Text_Set_Command` clears
  `SolutionAbort` per command from outside; `Dss::command` now does the same
  when not inside a Redirect (nested redirects also keep `in_redirect` via
  save/restore now). Probed: after a 485 abort the oracle's next `solve` runs
  (and exceeds again) rather than reporting "Solution aborted.".
- **Oracle-pinned unit tests** (exec): 2-bus regulator drives to tap 1.01875
  in 6 total iterations; `maxcontroliter=2` + `maxtapchange=1` stops at 4
  iterations, tap 1.00625, 485 warning, abort + external reset.

**WP5.8 — time-series modes + time options + BusCoords — ✅ done, gate-green.**
- `Solution` grew the live DynaVars fields `int_hour`/`t`/`h` (+ existing
  `dbl_hour`), `update_dbl_hour`, `increment_time` (exact modulo roll).
  `set_mode` is now the **full Pascal `Set_Mode`** (free fn over the circuit):
  clock reset, `OK_for_Dynamics`/`OK_for_Harmonics` guards (486/487 on
  unsolved; the machine-state init behind a successful dynamics/harmonics
  entry is Phase 7), default-control/load-model reverts, and the per-mode
  defaults block (PEAKDAY/DAILY h=3600 n=24; YEARLY n=8760; DUTYCYCLE h=1 +
  TIMEDRIVEN; HARMONIC CONTROLSOFF+ADMITTANCE; LD1/LD2 trapezoidal; ...).
- `solve()` dispatches DAILY/YEARLY/DUTYCYCLE/PEAKDAY → `solve_daily`/
  `solve_yearly`/`solve_duty`/`solve_peak_day` (SolutionAlgs.pas verbatim:
  IncrementTime → `DefaultHourMult` from the circuit's default shape →
  PriceShape signal → SolveSnap → monitor/meter `sample_all` hooks (no-op
  stubs, Phase 6) → `EndOfTimeStepCleanup` (empty body, call sites kept)).
- **Default DSS items**: `Dss::new` (and `Clear`) now runs the verbatim
  `CreateDefaultDSSItems` command list (loadshape.default, growthshape.default,
  spectrum.default/…, TCC_Curve.A/D/TLink/…); `New circuit.` resolves
  `DefaultDailyShapeObj`/`DefaultYearlyShapeObj` to `loadshape.default`
  (snapshot-cloned, same staleness as the WP5.3 shape refs). Circuit grew
  `default_hour_mult` (FPC zero-init reproduced), `price_signal` (25.0),
  `price_curve_obj`, `trapezoidal_integration`, `control_bus_name_redefined`
  (raised by `set_bus_name_redefined`, cleared by the control loop).
- **Set/Get options**: `hour`/`sec`/`stepsize` (+ alias `h`, `interpretTimeStepSize`
  with the exact h/m/s suffix rules)/`time` (2-vector, FPC-Round hour,
  `[ %d, %-g ] !... %-g (hours)` Get format)/`number`/`defaultdaily`/
  `defaultyearly`/`pricesignal`/`pricecurve`. All round-trips oracle-pinned in
  a unit test.
- **`BusCoords` command** (`DoBusCoordsCmd`): aux-parser `bus, x, y` rows,
  file resolved against `current_dir`, unknown buses silently skipped,
  read errors abort the file (275-style message). Coordinates survive
  `reprocess_bus_defs` (Phase 3 restore path). The unmodified masters need it.
- **Property-dump fix found by the gate:** Pascal `GetPropertyValue` renders
  `MappedIntEnumProperty` as the **ordinal** (`IntToStr`) — only string enums
  dump the name (`DSSObjectHelper.pas` l.2241). Load `Model` now dumps "5",
  not "Constant I". Also Transformer `WdgCurrents` now uses the exact
  `%.7g, (%.5g), ` Pascal format. dss-core lib tests 207 → 212.

**WP5.9 — goldens + gate tests — ✅ done (the phase gate).** See "Phase 5
gate" in §1 above. Generator facts:
- There is **no `LogEvents` Set option** — the option is `Log` (TExecOption 66,
  `ckt.LogEvents`); PHASE5_PLAN's `Set LogEvents=yes` spelling raises 130 in
  the oracle. Scenarios use `set log=yes`.
- The phase5 scenarios inline the IEEE13 master (controls active) with the
  `IEEELineCodes.DSS` redirect dropped — the master only uses its inline
  mtx601..607 codes, and command-replay goldens must be self-contained.
- `capcontrol_micro` probe: `type=kvar onsetting=500 offsetting=300` on
  `line.692675` opens Cap1 (the plan's 150/−225 suggestion never toggles).

**WP5.10 — phase exit — ✅ this update.** Marker sweeps clean (every
`TODO(compat)`/`NOT_PORTED` site points at its phase); stale "not ported in
Phase 3" executive messages reworded; full gate green.
