# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-13, **Phase 6 IN PROGRESS** — Phase 5 merged to `main`
(`10d3550`); on branch `phase-6-meters-topology`. Execution plan:
**`PHASE6_PLAN.md`** (WP6.1–WP6.10: meters/monitors/topology/Generator,
8500-node gate). Done so far: **WP6.1 (topology foundations), WP6.2
(Generator), WP6.3 (MeterElement + Monitor), WP6.4 (EnergyMeter + zone
build), WP6.5 (EnergyMeter registers + TakeSample), WP6.6 (reliability:
fault-rate sweep + `RelCalc`), WP6.7 (Sensor + load allocation)** — see §1d.

Earlier — **Phase 5 COMPLETE (WP5.1–WP5.10), gate-green, merged** —
the **phase gate passes: the unmodified IEEE13/IEEE37/IEEE123 masters
(controls ACTIVE) compile, solve and match the Phase-0 goldens** — iteration
counts exact (ieee13: 11), final taps / RegControl tap numbers / capacitor
states, node voltages and per-element powers/currents at 1e-6 rel, and every
element's full property dump (numeric skeleton). **The ieee34mod1 stretch goal
also passes.** The `phase5.json` command-replay gate (daily/duty/event-log/
capcontrol scenarios) matches the oracle — the 24-hour tap-change trajectory is
event-log-identical. See §1c.

Earlier — **Phase 4 COMPLETE (WP4.1–WP4.10)** — all core PD
elements (Transformer/Capacitor/Reactor), catalog objects
(LineCode/XfmrCode/GrowthShape), the Line→LineCode fetch path, parse-only
RegControl/CapControl, the `define_properties!` macro (bounded scope), and the
**phase gate: the IEEE13/IEEE37/IEEE123 controls-off feeders solve and match
the oracle** (exact iteration counts + node order; voltages, per-element
powers/currents, total power and losses at 1e-6 rel). On branch
`phase-4-pd-elements`, Phase 4 work uncommitted past WP4.6. Next: Phase 5
(`PHASE5_PLAN.md`).

> **Working cadence (per PHASE4_PLAN §0.8):** finish one small step → run the
> full gate → update this file → **stop and wait for explicit user
> confirmation** before the next step. (WP4.7–4.10 were executed in one pass on
> explicit user instruction.)

---

## 1. Where we are

| Phase | Scope | Status |
|------|-------|--------|
| 0 | Tooling, oracle, faer spike, CI, Phase-0 goldens | ✅ done (committed) |
| 1 | Shared math (`support/`) + full `TDSSParser` port | ✅ done (commit `729eb77`) |
| 2 | Object model, property engine, executive skeleton | ✅ done (commit `22f861d`) |
| 3 | ★ Vertical slice: parse → circuit → Y matrix → solve → voltages | ✅ done (commit `2ac8691`) |
| **4** | **Transformer/Capacitor/Reactor/LineCode + controls (parse-only) + macro + feeder gate** | ✅ **done** — WP4.1–4.6 committed (`f5156eb`…`c45719a`); WP4.7–4.10 complete, gate-green, **uncommitted** |
| **5** | **LoadShape/XYcurve/controls behavior, control queue, time modes + feeder gate (controls active)** | ✅ done (merged to main, `10d3550`); `PHASE5_PLAN.md` |
| **6** | **Meters/Monitors/topology/Generator + 8500-node gate** | 🔨 **in progress** — WP6.1–WP6.7 done; `PHASE6_PLAN.md` |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 262, golden_feeders 1,
                            # golden_feeders_controls 4, golden_phase5 1,
                            # golden_reliability 1, golden_slice 2,
                            # golden_smoke 3, props_roundtrip 1,
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
- `golden_phase5.rs` vs `tests/golden/phase5.json` (`tools/golden/gen_phase5.py`,
  command-replay like slice.json): `daily_ieee13` (24 hourly steps, every load
  on a 24-pt shape, regcontrol event logs on), `duty_2bus` (12×300 s steps,
  TIMEDRIVEN), `eventlog_ieee13` (`Set Log=yes`), `capcontrol_micro` (kvar
  control opens Cap1). Per-step `dblHour` exact; **the event logs match the
  oracle line-for-line** (normalized), pinning every tap change/cap switch of
  the trajectories; final taps/tap numbers/states exact. Per-step iteration
  counts: exact on step 1, ±1 afterwards; voltages 1e-5 rel until the first
  iteration-count divergence, then 2e-4 (the 1e-4 convergence tolerance makes
  tolerance-terminated iterates path-dependent at that level — documented in
  the test).

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

## 1b. Phase 4 record (branch `phase-4-pd-elements`)

Execution plan: **`PHASE4_PLAN.md`** (WP4.1–WP4.10).

**WP4.1 — LineCode — ✅ committed.** Files:
- `src/elements/general/line_code.rs` (`TLineCodeObj`): props 1–27 + Like;
  `CalcMatricesFromZ1Z0` (no 1-phase special case), `Set_NumPhases`,
  `DoKronReduction`, `PropertySideEffects`, `EndEdit`, `MakeLike`; 6 inline tests.
- **Shared engine additions:** `PropFlags::CONDITIONAL_VALUE` +
  `DssObject::prop_conditional` (sym scalars render `----` under a matrix
  model); sym-matrix getter format `[v |v v |...]`; deferred-error buffer on
  `DssObjData` (`push_error`/`take_errors`, drained in `exec::edit_active`).
- **`TODO(compat)`:** LineCode `Repair` defaults to `0` (oracle getter)
  although the Pascal ctor sets 3; deprecated/unused field.

**WP4.2 — ObjectRef resolution + Line→LineCode fetch — ✅ committed.**
- `PropDef::object_ref_class(class, name)` resolves at parse time;
  `ForeignClassesView` trait + `PropEngine::foreign` (read view of every class
  except the one being edited, built in `edit_active` via `split_at_mut`);
  `DssObject::set_object_ref(idx, name, resolved)` lets the element copy data
  immediately (the `FetchLineCode` pattern). Pascal 401 message on miss.
- `elements/pd/line.rs`: `fetch_line_code` verbatim (units reconversion,
  norm/emerg, matrix copy/resize, set-order clearing); earth-model default
  fixed to DERI.

**WP4.3a — GrowthShape — ✅ committed.** `general/growth_shape.rs`: props 1–6,
`APPLY_ROUND` (FPC banker's rounding) on `Year`, `get_mult`/`recalc_year_mult`
verbatim. `CSVFile`/`SngFile`/`DblFile` are `NOT_PORTED` (file input).

**WP4.3b — XfmrCode — ✅ committed.** `pd/winding.rs` (shared `Winding` =
Pascal `TWinding`, incl. `compute_anti_float_adder`) +
`general/xfmr_code.rs`: props 1–39, winding-edit state machine, XSC handling.
**Shared engine:** `DoubleVArray` (function-sized, `DssObject::array_size`),
`DoubleArrayOnStruct`, `EnumArrayOnStruct` prop types + struct-array accessors.

**WP4.4 — Transformer — ✅ committed.** `pd/transformer.rs` (`TTransfObj`):
props 1–49, `SetNumWindings`, winding-edit side-effect web, `RecalcElementData`
(DeltaDirection, per-winding VBase, Rdc on the transformer VABase, anti-float
adders, ratings), `SetTermRef` (incl. delta `RotatePhases`), `CalcY_Terminal`
(ZB → `Y_1Volt = AT·ZB⁻¹·A` → magnetizing branch → `Y_Term`), `CalcYPrim`
(`BuildYPrimComponent` + `AddNeutralToY`), `FetchXfmrCode`,
`Get/Set_PresentTap`, `WdgCurrents`. **Shared engine:** `BusOnStruct`/
`BusesOnStruct` prop types, `core_type`/`lead_lag` enums,
`ElemKind::Transformer` + circuit list. GIC path (<0.51 Hz) deferred (Phase 7).

**WP4.5 — Capacitor — ✅ committed.** `pd/capacitor.rs` (`TCapacitorObj`):
2-terminal shunt/series, 3 spec types (kvar/Cuf/CMatrix), per-step YPrim with
series-filter ZL, NumSteps split, `States`/`FindLastStepInService`. **Shared
engine:** `IntegerArray` + `DoubleSymMatrix` prop types,
`ElemKind::Capacitor` + `shunt_capacitors` list. **Oracle bug (traced):** the
`DoubleSymMatrixProperty` *getter* (`DSSObjectHelper.pas:2318`) reads a field
address instead of the heap array → always garbage; canonicalized to zeros on
both sides (`zero_garbage` in `gen_props.py`, `TODO(compat)` zero matrix in the
Rust getter); the real matrix→YPrim path is unit-tested instead.

**WP4.6 — Reactor — ✅ committed (`c45719a`).** `pd/reactor.rs`
(`TReactorObj`): 4 spec types (kvar / R+jX (`Z`/`LmH`) / R,X matrices /
Z1Z2Z0), `Parallel` R∥X via `etk_invert`, `Rp`, GIC R-only path, shunt
diagonal mirror with the 1φ-grounding-reactor exception. Same
`DoubleSymMatrix` oracle-getter bug (RMatrix/XMatrix zeroed in goldens).
`TODO(compat)` for the truncated `CALPHA` literal. RCurve/LCurve `NOT_PORTED`
(XYcurve, Phase 5).

**WP4.7 — ControlElem + RegControl/CapControl (parse-only) — ✅ NEW.** Files:
- `src/elements/control/control_elem.rs`: `ControlElemData` (embeds
  `CktElementData`; `element_terminal`, `controlled_element`/
  `monitored_element: Option<ElemRef>`, `time_delay`, `show_event_log`) and
  `RefSnapshot` — a parse-time shape snapshot (full name, nphases, nterms,
  terminal buses) of the referenced element, captured in `set_object_ref`
  because `RecalcElementData` runs at `EndEdit`, after the foreign-class view
  is gone. Staleness semantics match Pascal: a control's bus string is only
  refreshed by its *own* next recalc.
- `src/elements/control/reg_control.rs` (`TRegControlObj`): props 1–32 + tails
  (`transformer=` resolves against the Transformer class; AutoTrans not ported
  so the proxy is single-class), `PropertySideEffects` (winding→tapwinding,
  ptratio→RemotePTRatio, maxtapchange clamp, revThreshold×1000),
  `RecalcElementData` (LDC/regulated-bus flags, nphases/nconds from the
  transformer, PTphase reset, winding validation 122, `SetBus(1, <winding
  bus>)`; errors 124/122 as deferred messages), **`TapNum` get/set ported now**
  (`Get_TapNum`/`Set_TapNum` arithmetic vs the per-winding tap snapshot, FPC
  `Round` → `round_ties_even` `TODO(compat)`), `Reset` action property,
  `Set_Enabled` override (no BusNameRedefined), `MakeLike` (incl. the
  `TapNum := Other.TapNum` property-setter copy). 3 inline tests.
- `src/elements/control/cap_control.rs` (`TCapControlObj`): props 1–23 + tails
  (`element=` is the any-class full-name reference; `capacitor=` resolves
  against Capacitor), PF-mode On/Off translation (range check), CT/PT phase
  validation, VBus lowercase+flag, `RecalcElementData` (nphases from the
  capacitor, Time/Follow force terminal 1 + monitor the capacitor, terminal
  validation 362, bus from the effective element, the parse-time
  Voverride-bus warning + flag revert — bus list does not exist during parse,
  same as Pascal), `MakeLike`. `UserModel`/`UserData` `NOT_PORTED` (no DLLs
  ever); `ControlSignal` `NOT_PORTED` (LoadShape — Phase 5). 3 inline tests.
- **Shared engine additions:**
  - `PropDef::object_ref_any(name)` (`object_class: Some("")` = Pascal
    `PropertyOffset2 = 0`): the value carries `Class.Name`, resolved against
    any circuit class via the new `ForeignClassesView::find_full` (canonical
    `FullName` returned for dumps; error 402 on miss).
  - **Deferred cross-element writes**: `RefAction` enum +
    `DssObject::{take_ref_actions, apply_ref_action}`. Pascal pokes a foreign
    object through a live pointer mid-parse (RegControl `TapNum` →
    `tr.PresentTap[w] :=`); the property engine only holds a read view, so the
    setter queues the write and `edit_active` applies it right after the edit
    (plus target-side flag propagation). Nothing reads the target in between,
    so the timing shift is unobservable. Transformer implements the target
    side (`set_present_tap`, identical clamp to the control's local snapshot
    update, so dumps agree).
  - `EnumRegistry`: `reg_control_phase` ('RegControl: Phase Selection',
    hybrid min/max→−3/−2), `mon_phase` ('Monitored Phase', hybrid
    min/max/avg→−3/−2/−1), `cap_control_type` (Current/Voltage/kvar/Time/
    PowerFactor/Follow → 0..5; upstream comments out UserControl).
  - `ElemKind::Control` + `Circuit::controls` list (device list + own list;
    **not** PD/PC). `ymatrix.rs` already skips `yprim: None` elements; controls
    keep `yprim = None` forever (`calc_yprim` no-op, `get_currents` zeros).
- **Node-order check (the "silent killer")**: exec test
  `reg_control_does_not_change_node_order` builds the same circuit with and
  without a RegControl — `YNodeOrder` and iteration counts equal, control bus
  = the transformer's winding bus, no Yprim. The WP4.9 feeders re-verify at
  scale.
- **Goldens:** 11 RegControl/CapControl scenarios in `gen_props.py` (basic,
  full, ptphase=max+regulated-bus, **tapnum=5** (probed: moves the
  transformer tap to 1.03125 and reads back 5), makelike-with-override ×2,
  current/kvar+voltoverride/voltage+phases/time-forces-terminal/pf);
  `props.json` regenerated (pure insertions). `props_roundtrip` green.
- DoD check: the unmodified-except-variant IEEE13 master parses end-to-end
  through dss-cli with zero errors (38 devices) up to `Solve`.

**WP4.8 — `define_properties!` macro — ✅ NEW (bounded outcome).**
- `obj/props.rs::define_properties!`: generates the 1-based ordinal consts
  (`pub mod prop` incl. `NUM_PROPS`) and `class_props(&EnumRegistry)` from one
  declarative listing (`ordinal CONST => <PropDef builder expr>;`), so
  ordinals and table can never drift. Builder expressions keep the full
  `PropDef` API (scale/flags/`enums.<id>`) without macro ceremony.
- **Consumers:** TCC_Curve and Spectrum retrofitted (purely mechanical;
  `props.json` untouched and `props_roundtrip` green against the same file —
  zero behavior change).
- **Fallback note (per §3.4):** the accessor-arm part (goal (c)) was
  deliberately dropped — in every ported class the non-trivial arms
  (spec-type side effects, clamping, redundant aliases, cross-field writes)
  dominate, so a field-mapping macro needs an escape hatch per arm and stops
  paying for itself. Retry in Phase 5 only if the new catalog classes
  (LoadShape/XYcurve) turn out accessor-trivial.

**WP4.9 — Controls-off feeder gate — ✅ NEW (the phase gate).**
- `tools/golden/gen_phase4.py`: writes the three variants to
  `tests/golden/phase4/` (master copied line-by-line; `solve`/`buscoords`
  dropped; top-level `redirect`/`compile` args rewritten relative to the
  variant dir — computed via `os.path.relpath`, nested redirects untouched
  because the current dir follows the chain; `Set controlmode=OFF` + `Solve`
  appended) and captures `tests/golden/phase4.json` (converged, iterations,
  YNodeOrder, voltages, TotalPower, Losses, **ordered** per-element
  powers/currents lists).
- `crates/dss-core/tests/golden_feeders.rs`: compiles the same committed
  variants and asserts the gate (§1 above). Engine support added:
  `Dss::snapshot_elements()` (CAPI `Alt_CE_Get_Powers` = `GetPhasePower`×1e-3
  + `Iterminal`, creation order, `Class.name`), `Dss::total_power()` (CAPI
  `Circuit_Get_TotalPower` = Σ sources `Power[1]`·1e-3), `Dss::losses()`.
- **One genuine bug found by the gate:** `Circuit::losses` summed *all* PD
  elements; Pascal `TDSSCircuit.Get_Losses` skips `IsShunt` elements. Added
  `CktElement::is_shunt()` (default false; Capacitor/Reactor return their
  shunt flag) — reactive losses then matched to 1e-9.

**WP4.10 — phase exit — ✅ this update.** `TODO(compat)`/`NOT_PORTED` sweeps
clean (every deferral points at its phase, see §5); full gate green.

---

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

---

## 1d. Phase 6 record (branch `phase-6-meters-topology`)

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

---

## 2. What Phase 3 built (file-by-file map — still the architectural reference)

### Circuit model (`src/circuit/`)
- `bus.rs` — slim `Bus` (`TDSSBus`): `nodes`/`ref_no` allocation lists,
  `kv_base`, coords, `allocate_bus_state`.
- `circuit.rs` — `Circuit` (`TDSSCircuit` subset): `add_ckt_element` (device
  list + per-kind `Vec<ElemRef>` lists + 1-based handle), `add_bus`
  (find-or-create + the "Caution: Magic" node_buffer→global-ref rewrite),
  `process_bus_defs`, `reprocess_bus_defs`, `set_bus_name_redefined`,
  `node_name(i)` (= oracle `YNodeOrder` format), `losses` (now skips shunt).
- `terminal.rs` — `Terminal` (`TPowerTerminal`).

### Element base (`src/elements/`)
- `ckt.rs` — `CktElementData` (`TDSSCktElement` fields + realloc semantics,
  `set_bus`/`get_bus`, `compute_vterminal`, `do_yprim_calcs` open-conductor
  Kron, signal flags `signal_bus_name_redefined`/`yprim_invalid`).
- `traits.rs` — `CktElement` trait (+ `is_shunt` since Phase 4), `ElemRef`,
  `ElemStore`, `SysCtx`, `InjCtx`.
- `pc/vsource.rs`, `pc/load.rs` — full Phase 3 ports (all 8 load models,
  compensation currents).
- `pd/line.rs` — sym + matrix paths + LineCode fetch (Phase 4).
- `pd/{transformer,capacitor,reactor,winding}.rs`,
  `general/{line_code,xfmr_code,growth_shape}.rs`,
  `control/{control_elem,reg_control,cap_control}.rs` — Phase 4 (§1b).

### Solution (`src/solution/`)
- `solution.rs` — `solve` → `solve_snap` (control loop; `controlmode=off` ⇒
  `control_actions_done` immediately) → `solve_circuit` → `do_pflow_solution`
  → `do_normal_solution`; `converged`, `solve_system`, `set_voltage_bases`.
- `ymatrix.rs` — `build_y_matrix`: reprocess buses when redefined → recalc
  invalid Yprims → stamp enabled elements (skips `yprim: None`, i.e. control
  elements) → `allocate_vi` → `initialize_node_vbase`.

### Executive (`src/exec/mod.rs`)
- Full Pascal command/option name lists; `command()` = `ProcessCommand` (incl.
  error-301 circuit gate and the property-reference fallback);
  `New circuit.x` → default Vsource; `AddObject`; `Set`/`Get`; `Solve`;
  `Redirect`/`Compile` (block-comment semantics, dir following);
  `edit_active` (split-borrow `ForeignClasses` view, deferred-error drain,
  signal-flag propagation, **deferred `RefAction` application** since Phase 4);
  `snapshot_elements`/`total_power`/`losses` public gate API (Phase 4).

### Property engine (`src/obj/`)
- `props.rs` — `PropType` (Double/Integer/Boolean/String/MakeLike/arrays/
  matrices/Bus/Complex/Enabled/ObjectRef/struct-array family), `PropFlags`
  (incl. `NOT_PORTED`, `CONDITIONAL_VALUE`, `SCALED_BY_FUNCTION`), parse/dump
  paths, `ForeignClassesView` (+ `find_full`), **`define_properties!`**.
- `base.rs` — `DssObjData` (PrpSequence, deferred errors), `DssObject` trait
  (typed accessors, `set_object_ref`, struct-array hooks, `side_effects`,
  `end_edit`, `make_like`, **`take_ref_actions`/`apply_ref_action`**).
- `dss_enum.rs` — `TDSSEnum` + registry (17 enums).

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

Grep `rg "TODO\(compat\)"` for the full marker list (16 sites). Notable:
truncated `CALPHA`/`pi`/`0.001732`/`57.29577951` constants, FPC banker's
`Round` shims, LineCode `Repair`=0 default, the `DoubleSymMatrix` zero-matrix
getter.

`NOT_PORTED` (hard parse error; every site points at its phase):
- Line `geometry`/`spacing`/`wires`/`cncables`/`tscables` — Phase 6
  (LineGeometry/WireData).
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
python tools/golden/gen_props.py     # -> tests/golden/props.json   (Phase 2+)
python tools/golden/gen_slice.py     # -> tests/golden/slice.json   (Phase 3)
python tools/golden/gen_phase4.py    # -> tests/golden/phase4/*.dss + phase4.json
```
Oracle pin: Python 3.12.4, dss-python 0.15.7, dss-python-backend 0.14.5
(the same dss_capi release vendored in `.inputs/dss_capi`). `python` works in
this environment; the `py` launcher is broken — use `python` directly.

---

## 7. Current frontier — Phase 6

Executing `PHASE6_PLAN.md` on branch `phase-6-meters-topology`.
Done: WP6.1 (topology foundations), WP6.2 (Generator), WP6.3 (MeterElement +
Monitor), WP6.4 (EnergyMeter object + zone build), WP6.5 (EnergyMeter registers
+ `TakeSample` + hook wiring). Next: WP6.6 (reliability), WP6.7 (Sensor + load
allocation), WP6.8 (GenDispatcher + skeletons — also add PVSystem/Storage to
`is_zone_pce`), WP6.9 (goldens + 8500-node gate), WP6.10 (exit).

Enabling facts from Phase 5/WP6.3: the control loop dispatches through
`ElemStore::{obj,pair_mut,triple_mut}` + `DssObject::as_any_mut` (the pattern
the monitor sweep now reuses — `solution/monitors.rs`); the meter/monitor
`sample_all_monitors_and_meters` / `end_of_time_step_cleanup` hooks now have
real bodies (monitor `SampleAll` mode≠5 + `SampleAllMode5`, plus the EnergyMeter
`SampleAll`/`take_sample_all` wired in WP6.5). Deferred monitor modes
3/4/7/8/10/12 build their header but defer the sample body (no gate uses them).
WP6.4 added the zone-build dispatcher (`solution/meters.rs`) fired from
`build_y_matrix` after bus reprocessing — the EnergyMeter `BranchList`/
`SequenceList`/`ZonePCE` are now populated and exposed via `Dss::meter_zone`.
WP6.5 ported `TakeSample`/`Integrate` (register accumulation over the zone walk)
plus the PD loss/excess-kVA surface and the Load EEN/UE getters; registers are
exposed via `Dss::meter_registers`. Still deferred: the `SystemMeter` register
core, the Generator/Storage/PVSystem `ResetRegistersAll`/`SampleAll` call sites
(WP6.8+), reliability indices (WP6.6), and the demand-interval/phase-voltage
files (Phase 8).
