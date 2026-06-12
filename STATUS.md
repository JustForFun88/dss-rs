# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-12, **Phase 5 IN PROGRESS** — Phase 4 merged to `main`
(`5f27a25`); on branch `phase-5-controls-timeseries`. **WP5.1 (XYcurve),
WP5.2a/b/c (LoadShape + CSVFile + TempShape/PriceShape) and WP5.3 (shapes wired
into Load/VSource) done, gate-green** (see §1c). Next: WP5.4 (ControlQueue +
event log).

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
| **5** | LoadShape/XYcurve/controls behavior, control queue, time modes | 🔄 **in progress** — WP5.1 (XYcurve), WP5.2 (LoadShape + CSVFile + TempShape/PriceShape), WP5.3 (shapes → Load/VSource) done; `PHASE5_PLAN.md` |

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 181, golden_feeders 1, golden_slice 2,
                            # golden_smoke 3, props_roundtrip 1, dss-parser 62+1,
                            # dss-sparse 5
```

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
- CapControl `ControlSignal` — Phase 5 (LoadShape); `UserModel`/`UserData` —
  never (no DLL loading in safe Rust).
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
(Phase 7); RegControl/CapControl `Sample`/`DoPendingAction` + control queue
(Phase 5 — unreachable under `controlmode=off`); RegControl debug-trace file
(Phase 5); `MakePosSequence` everywhere (Phase 5+); `BusCoords` command
(Phase 5 ports it for the unmodified masters; stripped from the Phase 4
variants); Newton algorithm, harmonics/dynamics/yearly/duty solve modes
(later phases); `Show`/`Export`/`Dump`/`Select`/... executive verbs record
"not ported".

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

## 7. Next session — Phase 5

Start from **`PHASE5_PLAN.md`**. Phase 5 scope (PORTING_PLAN): LoadShape /
XYcurve / TShape / PriceShape catalog objects, the control *behavior*
(`Sample`/`DoPendingAction`, the control queue, RegControl/CapControl
actions), time-series solution modes (daily/yearly wiring of DynaVars), and
the gate compiles the **unmodified** IEEE masters (controls active — needs
`BusCoords` and the control loop).

Enabling facts from Phase 4: controls already parse, resolve and sit on the
right buses; `Get/Set_PresentTap` + `winding_tap_data` are public on
Transformer; the control loop skeleton in `solve_snap` already counts control
iterations; `RefAction` gives controls a sanctioned mutation path outside the
edit loop (the Phase 5 control queue will instead act through `ElemStore`
during the solve, where mutable access is direct).

---

## 8. Outstanding action

Phase 4 (WP4.7–4.10) is **complete and gate-green but uncommitted** on
`phase-4-pd-elements` (WP4.1–4.6 are committed). Actions:
1. Commit WP4.7–4.10 on this branch — only on explicit request.
2. Review + merge `phase-4-pd-elements` to `main`.
3. Start Phase 5 (§7).
