# Phase 3 record

> Archived from `STATUS.md` (moved 2026-06-21 to keep the live handoff lean).
> Frozen historical work-package log; superseded only by the code + tests.
> This is the Phase-3 vertical-slice **file-by-file map** — still the architectural
> reference later phases build on. Section references (`§3.x`, `§4`, `§5` …) point
> back to `STATUS.md`.

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
- `pc/vsource/mod.rs`, `pc/load/mod.rs` — full Phase 3 ports (all 8 load models,
  compensation currents).
- `pd/line/mod.rs` — sym + matrix paths + LineCode fetch (Phase 4).
- `pd/{transformer,capacitor,reactor,winding}.rs`,
  `general/{line_code,xfmr_code,growth_shape}.rs`,
  `control/{control_elem,reg_control,cap_control}.rs` — Phase 4 ([record](phase-4.md)).

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
- `props/mod.rs` — `PropType` (Double/Integer/Boolean/String/MakeLike/arrays/
  matrices/Bus/Complex/Enabled/ObjectRef/struct-array family), `PropFlags`
  (incl. `NOT_PORTED`, `CONDITIONAL_VALUE`, `SCALED_BY_FUNCTION`), parse/dump
  paths, `ForeignClassesView` (+ `find_full`), **`define_properties!`**.
- `base/mod.rs` — `DssObjData` (PrpSequence, deferred errors), `DssObject` trait
  (typed accessors, `set_object_ref`, struct-array hooks, `side_effects`,
  `end_edit`, `make_like`, **`take_ref_actions`/`apply_ref_action`**).
- `dss_enum/mod.rs` — `TDSSEnum` + registry (17 enums).
