# Phase 5 — Detailed Execution Plan: Control Loop, RegControl/CapControl, Time-Series Modes

> Companion to `PORTING_PLAN.md` §Phase 5; same status and rules of engagement as
> `PHASE4_PLAN.md` §0 (Pascal is the spec; probe the oracle, never guess; mark
> `TODO(compat)` / `NOT_PORTED`; goldens manual-only; gate-green per WP; commit
> only on request). **Prerequisite: Phase 4 complete and gate-green** — this phase
> consumes the Transformer tap machinery, Capacitor steps, ControlElem base and
> the ObjectRef resolution mechanism (PHASE4_PLAN §3.1) without re-explaining them.

## 1. Phase target and gate

**Scope** (PORTING_PLAN §Phase 5): `ControlQueue.pas`, the control behavior of
`RegControl.pas`/`CapControl.pas` (objects exist since Phase 4), `LoadShape.pas`,
`TempShape.pas`, `PriceShape.pas`, `XYcurve.pas`, the rest of `Solution.pas` +
`SolutionAlgs.pas` for Snap/Daily/Yearly/DutyCycle, and the event log.

**Gate** (all must pass):

1. `golden_feeders_controls.rs`: compile the **unmodified** IEEE13, IEEE37 and
   IEEE123 masters from `.inputs/electricdss-tst` (controls active, exactly as the
   committed Phase-0 goldens `tests/golden/{ieee13,ieee37,ieee123}.json` were
   generated — IEEE123 needs the `post: ["solve"]` command from
   `tools/golden/cases.json`), and match those Phase-0 goldens:
   - **final transformer taps and RegControl tap numbers exactly equal**
     (golden keys `transformers.<name>.taps`, `regcontrols.<name>.tap_number`);
   - capacitor states exactly equal (`capacitors.<name>.states`);
   - total iteration count exactly equal (ieee13: 11);
   - node voltages, element powers/currents, total power, losses at 1e-6 rel.
   - `ieee34mod1.json` is a **stretch goal**: attempt it; if the master uses any
     still-unported class/command, document in STATUS.md and skip with an
     `#[ignore]` + comment.
2. `golden_phase5.rs` vs new `tests/golden/phase5.json` (generator
   `tools/golden/gen_phase5.py`, command-replay style like `slice.json`):
   - a **daily-mode** scenario: hourly voltage trajectories over 24 steps match
     (1e-6 rel per step), final taps exact;
   - an **event log** scenario: normalized event-log equality;
   - a **duty-cycle** scenario (same machinery, finer steps).
3. Standard repo gate green; Phase 2/3/4 golden tests still green (regressions in
   shared paths — e.g. `set_nominal_load` — show up there first).

## 2. Phase-wide design decisions

### 2.1 Control sampling/acting under the borrow checker

Pascal flow per control iteration (`Solution.pas::SolveSnap` l.1178 → repeat
`SolveCircuit; CheckControls` until `ControlActionsDone or ControlIteration >=
MaxControlIterations`):

- `CheckControls` (l.1132): if converged → `Sample_DoControlActions`; afterwards if
  `SystemYChanged` → rebuild Y (voltages kept).
- `Sample_DoControlActions` (l.1996): `ControlMode = CONTROLSOFF` → just set
  `ControlActionsDone := TRUE`; else `SampleControlDevices` (every enabled element
  of `ckt.DSSControls` gets `Sample()`) then `DoControlActions` (l.1940, dispatch
  on control mode), then reset `ckt.Control_BusNameRedefined := FALSE`.

Sampling reads the controlled transformer (winding voltages, currents, taps) and
solution state, and pushes onto the queue. Acting pops a queue record and mutates
the controlled element (tap change / capacitor step). Rust design:

- The **ControlQueue stores no object pointers** — records hold
  `ElemRef` of the control element + action/proxy codes (Pascal stores the object
  pointer; ElemRef is our stable equivalent).
- `Sample` and `DoPendingAction` are methods on the control object taking a
  context struct:

  ```rust
  pub struct CtrlCtx<'a> {
      pub foreign: ForeignClassesMut<'a>, // ← mutable sibling of Phase 4's view:
                                          //   get_mut(ElemRef) -> &mut dyn DssObject
      pub node_v: &'a [Complex64],
      pub sys: &'a SysCtx,                // hour, t, control_mode, event-log flag...
      pub queue: &'a mut ControlQueue,
      pub events: &'a mut EventLog,
      pub errors: &'a mut Vec<String>,
  }
  ```

  Obtain the split exactly like Phase 4's `ForeignClasses` (split_at_mut around the
  control's class index): the control object is borrowed mutably from its class
  slot while `foreign` covers all *other* classes — RegControl and Transformer are
  different classes, so `foreign.get_mut(controlled)` works. Downcast the
  controlled element with `as_any_mut()` (add it next to `as_any()` if missing).
- The transformer mutation (`set_present_tap`) sets `yprim_invalid` on the
  transformer; after the action sweep, propagate to `system_y_changed` the same
  way `edit_active` does (walk the signal flags, or have `CtrlCtx` carry a
  `&mut bool system_y_changed` that `set_present_tap` callers raise — pick one,
  document it in the code).

### 2.2 Control modes and action codes (exact ordinals)

`Common/DSSGlobals.pas`: `CONTROLSOFF = -1, CTRLSTATIC = 0, EVENTDRIVEN = 1,
TIMEDRIVEN = 2, MULTIRATE = 3`. The Phase 3 `control_mode` enum in `EnumRegistry`
already parses the names; make the Rust enum carry these ordinals.
`RegControl.pas` l.246: `ACTION_TAPCHANGE = 0, ACTION_REVERSE = 1`.
The gate feeders all run the default `CTRLSTATIC`.

### 2.3 Event log

`Dss` grows `event_log: Vec<String>` (Pascal `DSS.EventStrings`). Two producers:
- `LogThisEvent(name)` — gated by `ckt.LogEvents` (`Set LogEvents=yes`); used by
  the solution loop ("Control Iteration N") — see `Solution.pas` l.1147;
- `AppendToEventLog(name, action)` — gated by the *per-control* `EventLog`
  property (default comes from `DSS.EventLogDefault`; check
  `Controls/ControlElem.pas`/`DSSGlobals` for the default value and the `Set
  EventLogDefault` option spelling in `ExecOptions`).
Entry format: check the Pascal `AppendToEventLog` implementation
(`Common/Utilities.pas`) and port the `Format` string exactly — the gate compares
normalized strings (numbers parsed out, like `props_roundtrip`'s
`numeric_skeleton`). Expose via `Dss::event_log()`; the oracle reads
`dss.ActiveCircuit.Solution.EventLog`.

## 3. Work packages

---

### WP5.1 — XYcurve [5%]

**Pascal:** `General/XYcurve.pas` (584 lines). Props 1–13: `NPts, Points, YArray,
XArray, CSVFile, SngFile, DblFile, X, Y, XShift, YShift, XScale, YScale`.

Steps: catalog object like Spectrum. `Points` parses interleaved x,y pairs;
`X`/`Y` are scalar "enter one point at a time" accessors with the LastValueAccessed
pattern; shift/scale props re-bake the arrays (read `PropertySideEffects` — the
order x-then-shift vs shift-then-x matters; probe the oracle on one case).
`GetYValue(x)` linear interpolation with hunt/locate caching — port verbatim
(Reactor RCurve/LCurve and many Phase 6–7 classes consume it; un-flag Reactor's
`NOT_PORTED` on RCurve/LCurve and wire its fetch now if cheap, else leave a note).
File props `NOT_PORTED`. `gen_props.py` scenarios + an interpolation unit test
probed against the oracle (`XYCurves` API in dss-python: `dss.ActiveCircuit.XYCurves`).

---

### WP5.2 — LoadShape (+ TempShape, PriceShape minimal) [15%]

**Pascal:** `General/LoadShape.pas` (2182 lines — much of it is file I/O and
memory-mapping; the live core is smaller). Props 1–22: `NPts, Interval, Mult,
Hour, Mean, StdDev, CSVFile, SngFile, DblFile, Action, QMult, UseActual, PMax,
QMax, SInterval, MInterval, PBase, QBase, PMult, PQCSVFile, MemoryMapping,
Interpolation`.

Steps:
1. State: `npts, interval (hours), p_mult[], q_mult[], hour[] (None = even
   spacing), use_actual, max_p/max_q (+ the set flags), base_p/base_q, mean/stddev
   (lazy via the existing `RCDMeanAndStdDev` port in `support/mathutil.rs` — it
   carries the single-point-stddev `TODO(compat)`), `stride`, interpolation enum`.
2. Property web: `Interval`/`SInterval`(s)/`MInterval`(min) write the same field
   with unit conversion; `Mult`/`PMult` are the same array; `Mean`/`StdDev` setters
   mark them externally-set; `Action=normalize` ports `Normalize` (divide by
   `MaxP` or `PBase`; read the Pascal for the `UseActual` interplay);
   `Action={DblSave,SngSave}` → `NOT_PORTED`. `MemoryMapping=yes` → `NOT_PORTED`
   (hard error; revisit only if a corpus case needs it). `CSVFile` **is** needed
   (common in tst circuits): port `DoCSVFile` — one `hour, mult[, qmult]` row per
   line, comma/space delimiters via the DSS parser, quoted filenames; reuse the
   file-resolution logic Redirect uses (relative to `current_dir`).
   `SngFile`/`DblFile`/`PQCSVFile` → `NOT_PORTED` until a gate case needs them.
3. **`GetMultAtHour`** (l.1421) — the consumer-facing core; port verbatim:
   even-interval branch (`round vs trunc`, the wrap at `npts·interval` — read
   carefully, it normalizes hour into the first cycle and has off-by-one traps),
   hour-array branch with `LastValueAccessed` linear hunt, and the
   `Interpolation=avg|edge` switch. Returns Complex (P-mult, Q-mult; Q defaults to
   P when no QMult — check `set_qmult`/ctor defaults).
4. TempShape/PriceShape (`General/TempShape.pas`, `PriceShape.pas`, ~520 lines
   each): same skeleton with `Temp`/`Price` instead of `Mult` and a scalar getter
   (`GetTemperature`/`GetPrice`). Port object + getters; file props `NOT_PORTED`.
   (`PriceShape` feeds `ckt.PriceSignal` in SolveDaily — wire in WP5.7.)
5. Tests: `gen_props.py` scenarios (interval forms, csvfile, normalize,
   hour-array); unit tests for `GetMultAtHour` probed against the oracle
   (`dss.ActiveCircuit.LoadShapes` exposes `HrInterval`/`Pmult`; for the getter
   itself probe via a load + daily solve, or transcribe values from a tiny
   2-point shape solved hourly).

---

### WP5.3 — Wire shapes into Load, VSource and Circuit defaults [10%]

**Pascal:** `PCElements/Load.pas` `CalcDailyMult` (l.919), `CalcDutyMult` (l.930,
falls back to daily), `CalcYearlyMult` (l.941), and the mode dispatch inside
`SetNominalLoad` (l.1020–1070); `Common/Circuit.pas` `DefaultDailyShapeObj` /
`DefaultYearlyShapeObj` / `DefaultHourMult`; VSource's shape usage (check
`PCElements/VSource.pas` — the Phase 3 port left `yearly_shape`/`daily_shape` as
strings with the DAILY fallback branch already sketched).

Steps:
1. Convert Load's `daily`/`duty`/`yearly`/`growth`/`CVRcurve` and VSource's shape
   refs from stored strings to ObjectRefs (PHASE4_PLAN §3.1). CVRcurve resolves to
   XYcurve (used by model 4 with CVRshape — check `Load.pas` for where CVRShapeObj
   is consumed; port what `SetNominalLoad` needs).
2. Port the `ShapeFactor` machinery: `CalcDailyMult` returns the shape's
   `GetMultAtHour(dbl_hour)` or `(1,1)` when no shape; duty falls back to daily;
   yearly defaults to the **circuit's** DefaultYearlyShapeObj? — **no: read the
   Pascal**; `TLoadObj` keeps its own refs and the *circuit* default applies via
   `DefaultHourMult` in the global load multiplier path. Get this from the source,
   not from memory.
3. Circuit: create the built-in `default` LoadShape (Pascal `TLoadShape.Create`
   registers a "default" object — find where; likely `DSSClassDefs`/class ctor)
   and the `DefaultDailyShapeObj`/`DefaultYearlyShapeObj` resolution; `set
   year=` hooks `DefaultGrowthFactor` (GrowthShape from Phase 4).
4. `SetNominalLoad`: replace the Phase 3 mode stub with the full dispatch
   (SNAPSHOT/HARMONICMODE..., DAILYMODE→CalcDailyMult, YEARLYMODE, DUTYCYCLE,
   etc. — `Load.pas` l.1020-1070; keep unported modes erroring).
5. Tests: unit test — load with a 2-point daily shape, `set mode=daily`, assert
   `set_nominal_load` factors at hours 0.5/1.5 against hand-computed shape values;
   the real check is WP5.9's daily golden. Re-run `golden_slice`/`golden_feeders`
   (snapshot paths must be byte-identical in behavior — shape factor (1,1)).

---

### WP5.4 — ControlQueue + event log [10%]

**Pascal:** `Common/ControlQueue.pas` (479 lines).

Steps:
1. Types: `TimeRec { hour: i32, sec: f64 }`; `ActionRecord { action_time: TimeRec,
   action_code: i32, action_handle: i32, proxy_handle: i32, control: ElemRef }`.
   Queue is an ordered list; **port the Pascal container semantics exactly**
   (it's a TList kept unsorted with linear scans for "nearest time" — `Pop`
   l.249, `Pop_Time` l.276; tie-breaking falls out of scan order and handle
   comparison; do NOT substitute a BinaryHeap — iteration order is observable via
   the event log and action application order).
2. API: `push(hour, sec, code, proxy, elem) -> handle` (4 Pascal overloads
   collapse to one; `ctrl_handle` counter increments per push — l.93-108),
   `delete(handle)` (l.528), `do_nearest_actions(&mut hour, &mut sec, ctx)`
   (l.212), `do_actions(hour, sec, ctx)` (l.330), `do_multi_rate` (l.359),
   `do_all_actions` (l.201), `clear`, `is_empty`. The `do_*` functions pop
   records and invoke `control.do_pending_action(code, proxy, ctx)` — route
   through the §2.1 split-borrow (queue is *inside* `CtrlCtx`; to avoid
   queue-borrow-vs-queue-mutation conflicts, pop the due records into a local
   `Vec` first, exactly like Pascal pops before acting — check whether actions
   may push *new* records mid-sweep (RegControl EVENTDRIVEN does!) and match
   the Pascal visit semantics).
3. Event log per §2.3 (`EventLog` type, both producers, `Set LogEvents` /
   per-control `EventLog=` property from Phase 4's table, `Show eventlog` stays
   unported — the API getter is enough for tests).
4. Unit tests: push/pop ordering with ties; delete-by-handle; `do_actions`
   time-window semantics — transcribe expectations from the Pascal by hand-tracing
   (no oracle API exposes the queue; the IEEE gates validate end-to-end).

---

### WP5.5 — RegControl behavior [20%, the heart of the phase]

**Pascal:** `Controls/RegControl.pas` — `RecalcElementData` (l.567, done in
Phase 4), `GetControlVoltage` (l.639), `Sample` (l.862), `DoPendingAction`
(l.748), `AtLeastOneTap` (l.700) + `OneInDirectionOf`, `ComputeTimeDelay`
(l.1292), `Reset` (l.1233), `Set_TapNum` (l.1245), `VLimitActive` (l.1307).

Steps (port `Sample` top-to-bottom; it is one long procedure — keep it one long
function with the same structure):
1. Early exit `TapLimitPerChange == 0` → `pending_tap_change = 0`.
2. Reverse/cogen power-direction block: forward power `-Power[ElementTerminal].re`
   via the transformer's terminal power (CktElement default `power(term)` from
   Phase 3); push/delete `ACTION_REVERSE` with `RevDelay`; the `ReverseNeutral`
   branch. The IEEE gates never enter reverse mode (no `reversible=` in the
   masters) but port it whole — partial ports rot.
3. Control voltage: `UsingRegulatedBus` branch (`bus=` property; wye/delta winding
   voltage selection via `RotatePhases`) vs `GetWindingVoltages(ElementTerminal,
   VBuffer)` + `GetControlVoltage(VBuffer, Fnphases, PTRatio)` (l.639: PTPhase
   selection — `AVGPHASES`/`MAXPHASE`/`MINPHASE` enum vs specific phase; port the
   `case` exactly).
4. LDC: `ILDC = CBuffer[Nconds·(ElementTerminal−1) + ControlledPhase]/CTRating`;
   standard `Vcontrol += (R + jX)·ILDC` vs the `LDC_Z` Beckwith branch (magnitudes
   only); reverse variants. Requires `GetCurrents` on the transformer (Phase 4).
5. Band test: `|VregTest − Vactual| > BandTest/2` (+ the InReverseMode tap-divide
   and the Vlimit override); boost computation
   `BoostNeeded = Vboost·PTRatio / BaseVoltage[ElementTerminal]`,
   `PendingTapChange = Round(BoostNeeded/Increment)·Increment` — **FPC `Round` is
   banker's rounding**; use the Phase 1 `fpc_round` port, NOT `f64::round`
   (this single line decides tap-position equality; it is the most
   numerically-sensitive spot in the phase);
   direction flip when `TapWinding <> ElementTerminal`; arm via
   `queue.push(ComputeTimeDelay(Vactual), ACTION_TAPCHANGE, 0, self)` guarded by
   min/max tap; disarm path (delete handle, reset) when back in band.
6. `DoPendingAction` (l.748): `ACTION_TAPCHANGE` per control mode — CTRLSTATIC
   does `AtLeastOneTap` (full pending change, clamped to one-increment minimum,
   `LastChange` bookkeeping) and applies via the transformer's
   `set_present_tap(TapWinding, …)`; EVENT/TIME/MULTIRATE do `OneInDirectionOf`
   (single increment + re-push with `TapDelay`); `ACTION_REVERSE` toggles
   mode flags. Event-log lines via `AppendToEventLog('Regulator.<name>', …)` with
   the exact Pascal format strings.
7. `Reset` property/command behavior (l.1233: pending=0, disarm, tap → center?
   read it), `Set_TapNum` (l.1245: integer tap → pu via increment, clamped).
8. `ComputeTimeDelay` (l.1292: inverse-time option + fixed `Delay` — the gate
   cases use the default fixed delay; port both).
9. Unit test: hand-built 2-bus + regulator transformer + regcontrol; one
   `solve`; assert the first control iteration's pending tap against a
   dss-python probe of the same micro-circuit (probe `tap_number` after solve
   with `maxcontroliter=1`? — `Set maxcontroliter` caps it; transcribe whatever
   the oracle does). The full validation is the IEEE13 gate (taps 1.0625/1.05/
   1.06875 → regcontrol tap numbers 9/6/9 vs `regcontrols.*.tap_number` golden).

---

### WP5.6 — CapControl behavior [8%]

**Pascal:** `Controls/CapControl.pas` — `Sample` (l.846), `DoPendingAction`
(l.721), `GetControlVoltage` (l.798), `GetControlCurrent` (l.687),
`GetBusVoltages` (l.678), `Reset` (l.1212).

Steps: port `Sample`'s `ControlType` dispatch (CURRENTCONTROL / VOLTAGECONTROL /
KVARCONTROL / TIMECONTROL / PFCONTROL / FOLLOWCONTROL(ControlSignal)) — the
ON/OFF setting comparisons, `Delay`/`DelayOff`/`DeadTime` queue pushes, the
VoltOverride branch; `DoPendingAction` opens/closes capacitor steps via Phase 4's
`AddStep`/`SubtractStep`/`Set_ConductorClosed`; `PendingChange` enum
(Open/Close/None). Event log entries with exact format strings. None of the three
gate feeders has a CapControl, so add a **micro golden scenario** to
`gen_phase5.py`: IEEE13 + `New capcontrol.cc element=line.671680 terminal=1
capacitor=cap1 type=kvar onsetting=150 offsetting=-225`, solve, capture states +
event log + voltages; replay in `golden_phase5.rs`. (Pick the exact scenario by
probing the oracle so that it actually toggles at least one step.)

---

### WP5.7 — Control loop + `Sample_DoControlActions` in the solution [8%]

**Pascal:** `Solution.pas` `CheckControls` (l.1132), `Sample_DoControlActions`
(l.1996), `DoControlActions` (l.1940), `SampleControlDevices` (l.1973).

Steps:
1. Replace the Phase 3 `check_controls` skeleton: if
   `control_iteration < max_control_iterations` and converged → log "Control
   Iteration N" (gated `LogEvents`) → `sample_do_control_actions` →
   `check_fault_status` (no-op stub until Phase 7 faults; leave a comment);
   if not converged → `control_actions_done = true`. Then `system_y_changed` →
   rebuild Y **keeping voltages** (the Phase 3 `build_y_matrix` already preserves
   `node_v`; verify).
2. `sample_do_control_actions`: CONTROLSOFF short-circuit; else sample every
   enabled element of `ckt.dss_controls` (creation order!), then `do_control_actions`
   per §2.2 mode dispatch (CTRLSTATIC: queue empty → done, else
   `do_nearest_actions` ignoring time advance; EVENTDRIVEN/TIMEDRIVEN/MULTIRATE per
   the Pascal — port all four), then `control_bus_name_redefined = false`.
3. "Max Control Iterations Exceeded" warning (error 485) + `solution_abort` —
   exact Pascal message text.
4. `Set maxcontroliter` already parses (Phase 3); verify it reaches
   `max_control_iterations`.
5. Unit test: a control that never settles (regcontrol with band=0.0001 on a
   2-bus) must stop at `maxcontroliter` with the warning recorded — compare
   iteration count against the oracle on the same script.

---

### WP5.8 — Time-series modes: Daily/Yearly/Duty + time options [12%]

**Pascal:** `SolutionAlgs.pas` `SolveDaily` (l.160), `SolveYearly` (l.112),
`SolveDuty` (l.249), `FinishTimeStep` (l.74), `EndOfTimeStepCleanup` (l.86);
`Solution.pas` `Set_Mode` (l.~2010), `IncrementTime`; `Shared/Dynamics.pas`
DynaVars (Phase 1 port exists in `support/dynamics.rs`).

Steps:
1. **DynaVars wiring**: `interval h`, `intHour`, `t`, `dblHour`
   (`Update_dblHour: dblHour = intHour + t/3600`), `IncrementTime` (t += h, roll
   into intHour — port the modulo handling exactly). `Set
   hour=/sec=/time=/stepsize=/number=` options (the Phase 3 stubs): `time=` parses
   `[hour, sec]` 2-vector; `stepsize=` accepts plain seconds or `h/m/s`-suffixed
   values (find the Pascal interpreter — `ExecOptions`/`Utilities`
   `InterpretTimeStepSize` — and port its exact suffix handling and error).
2. **`Set_Mode` side effects** (`Solution.pas`): mode assignment resets
   `intHour=0, t=0`, re-derives `dblHour`, resets
   `control_mode = default_control_mode`, `load_model = default_load_model`,
   `solution_initialized = false`; `OK_for_Dynamics/Harmonics` guards keep
   erroring (Phase 7). The Phase 3 `SolveMode::from_ordinal` mapping is already
   COM-compatible; daily=1? — **verify against TSolveMode** (don't trust memory).
3. **`solve()` dispatch**: add DAILYMODE → `solve_daily`, YEARLYMODE →
   `solve_yearly`, DUTYCYCLE → `solve_duty` to the Phase 3 mode dispatcher
   (which currently errors on them).
4. `solve_daily` (port l.160 verbatim): `interval_hrs = h/3600`; loop
   `number_of_times`: `IncrementTime` → `DefaultHourMult =
   DefaultDailyShapeObj.GetMultAtHour(dblHour)` → PriceShape signal if set →
   `solve_snap` → monitor/meter `sample_all` hooks (**introduce the hooks as
   no-op trait/function stubs with a Phase 6 note** — do NOT skip the call sites;
   Phase 6 fills them) → `EndOfTimeStepCleanup` (storage/invcontrol — empty for
   now, keep the function). `solve_yearly` (l.112) and `solve_duty` (l.249) same
   pattern — note what differs (meter opening, `DefaultGrowthFactor`, duty uses
   duty shape fallback).
5. **`BusCoords` command** (the unmodified masters call it): port
   `ExecHelper.DoBusCoordsCmd` — parse `bus, x, y` CSV via the aux parser, set
   coords on existing buses, silently skip unknown buses (check the Pascal!).
   Coordinates must survive `reprocess_bus_defs` (Phase 3 already restores them).
6. Verify `Set number/stepsize/hour/time` round-trip via `Get` (oracle parity:
   probe `get stepsize` after `set stepsize=15m`).

---

### WP5.9 — Goldens + gate tests [10%]

1. **`golden_feeders_controls.rs`** (new): compiles the three **unmodified**
   masters (absolute paths into `.inputs/electricdss-tst` built from
   `CARGO_MANIFEST_DIR`; IEEE123 issues `solve` after compile per `cases.json`),
   then asserts against the **existing committed Phase-0 goldens** —
   `tests/golden/{ieee13,ieee37,ieee123}.json`:
   `solution.converged/iterations` (exact), `node_order`, `node_voltages`
   (1e-6 rel), `total_power_kw_kvar`, `losses_w_var`, `transformers.*.taps`
   (**exact**), `regcontrols.*.tap_number` (**exact**), `capacitors.*.states`
   (**exact**), per-element `powers`/`currents` (1e-6 rel) from the `elements`
   map. (The goldens embed per-element `properties` dumps too — assert them with
   the `props_roundtrip` numeric-skeleton comparator; this is the no-extra-cost
   property regression for every class in the feeder.) Do **not** regenerate
   these goldens.
2. **`tools/golden/gen_phase5.py` → `tests/golden/phase5.json`** (command-replay
   schema like `slice.json`; each scenario stores its full command list):
   - `daily_ieee13`: IEEE13 commands (inline the variant from Phase 4 **with
     controls on**, i.e. strip only `solve`/`BusCoords`), plus
     `New loadshape.day npts=24 interval=1 mult=(...)` (pick a 24-value curve
     spanning ~0.3–1.1), `batchedit` is unported so assign per load:
     `Load.671.daily=day` (property-reference syntax works since Phase 3) for
     every load; `set mode=daily stepsize=1h number=1`; then 24× `solve`,
     capturing after each step: `dblHour`, YNodeVarray, iterations; final taps +
     regcontrol tap numbers + event log at the end. Replay side: identical
     commands, assert per-step voltages 1e-6 rel, iterations exact, final taps
     exact, event log normalized-equal.
   - `duty_2bus`: the Phase 3 2-bus circuit + a duty shape
     (`sinterval=300 npts=12`), `set mode=duty number=1`, 12 steps, same captures.
   - `eventlog_ieee13`: IEEE13 with `set LogEvents=yes` before solve; capture
     `dss.ActiveCircuit.Solution.EventLog` and compare normalized (skeleton +
     numbers at 1e-6; tap values inside event strings count as numbers).
   - `capcontrol_micro` from WP5.6.
3. Debugging order for tap mismatches: compare event logs first (they tell you
   *which* control iteration diverged), then the pending-tap computation inputs
   (`Vactual`, `BoostNeeded`) — add a temporary trace behind an env var if
   needed, but **delete it before the WP ends** (or keep it as the Pascal-style
   `DebugTrace` property port — RegControl has one; porting it properly is the
   better move).

---

### WP5.10 — Phase exit

1. `rg "TODO\(compat\)"` / `rg "NOT_PORTED"` sweep — all new sites tagged and
   pointing at their phase.
2. Re-run **everything**: Phase 2 props, Phase 3 slice, Phase 4 feeders +
   phase4.json, Phase 5 gates. All green.
3. Rewrite `STATUS.md` (Phase 5 section: built/decisions/oracle-facts/deferrals;
   "next = Phase 6, write PHASE6_PLAN.md first").
4. Commit only on explicit user request.

## 4. Deferred in this phase

- Monitors/EnergyMeters: `sample_all` hooks are no-ops (Phase 6). SolveDaily's
  meter open/close calls stay as commented hook sites.
- Harmonics/dynamics/faultstudy modes, `SolveMonte*`, `SolveLD*`, `SolvePeakDay`
  (unless trivial once Daily works — it differs only in the multiplier source;
  fine to include), `SolveGeneralTime` (Phase 7).
- LoadShape `MemoryMapping`, `SngFile`/`DblFile`/`PQCSVFile`, `Interpolation`
  beyond what the corpus needs.
- CapControl `UserModel`/`UserData` (never), `ControlSignal`/FOLLOWCONTROL if no
  oracle case exercises it — then `NOT_PORTED` instead of a blind port.
- `Show eventlog`, `Show taps` and all Show/Export (Phase 8); the event log is
  validated via the API, not text reports.
