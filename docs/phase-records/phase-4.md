# Phase 4 record

> Archived from `STATUS.md` (moved 2026-06-16 to keep the live handoff lean).
> Frozen historical work-package log; superseded only by the code + tests.
> Section references (`§3.x`, `§5` …) point back to `STATUS.md`.

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
