# dss-rs — Project Status & Session Handoff

> **Purpose of this file:** a living snapshot so a fresh session can resume
> without re-deriving context. It records *what is done*, *what was decided*,
> and **why**. It is **not** authoritative for the plan itself — that is
> `PORTING_PLAN.md` (roadmap + binding decisions) and `CLAUDE.md` (conventions
> + the green-gate rule). Read those two first; then read this for the current
> frontier.

Last updated: 2026-06-11, end of **Phase 3** (★ vertical slice).

---

## 1. Where we are

| Phase | Scope | Status |
|------|-------|--------|
| 0 | Tooling, oracle, faer spike, CI, Phase-0 goldens | ✅ done (committed) |
| 1 | Shared math (`support/`) + full `TDSSParser` port | ✅ done (commit `729eb77`) |
| 2 | Object model, property engine, executive skeleton | ✅ done (commit `22f861d`) |
| **3** | **★ Vertical slice: parse → circuit → Y matrix → solve → voltages** | ✅ **done — uncommitted on branch `phase-2-object-model`** |
| 4 | Transformer/Capacitor/Reactor/LineCode + `define_properties!` | ⬜ next |

**Important:** Phases 2–3 live on the `phase-2-object-model` branch (off
`main`), per the repo rule that commits happen only on explicit request and
never directly on the default branch. Phase 3 is **not yet committed**.

> History note: an earlier Phase 3 attempt (assessed this session) was partial
> and numerically wrong (Zs=Z1 line diagonal, 1-terminal VSource, load power
> n× too big, no compensation currents, no oracle gate). It was fully redone
> from the Pascal sources; the old `phase3_sanity.rs` was deleted.

### Gate state (all green)
```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace      # dss-core lib 87, golden_slice 2, golden_smoke 3,
                            # props_roundtrip 1, dss-parser 62+1, dss-sparse 5
```

### Phase 3 gate (`crates/dss-core/tests/golden_slice.rs`) — green
13 scenarios from `tests/golden/slice.json` (pinned oracle), all matching:
- (a) 2-bus vsource+line+load: node voltages within **1e-9 rel**;
- (b) **IEEE13-flat** (transformers/regulators/caps stripped, linecodes inlined
  as per-line `rmatrix/xmatrix/cmatrix`, 32 nodes, min 0.88 pu): **1e-6 rel**;
- (c) fixed-point iteration counts **exactly equal** (2 for 2-bus family,
  3 for ieee13_flat);
- (d) all **8 load models** exercised (incl. CVR powf variant, delta
  connection, and a 5 km heavy case that drives |V| to 0.916 pu so the
  below-95% interpolation zones execute).

The CLI works end to end: `cargo run -p dss-cli -- script.dss` compiles the
script and prints per-node |V| / angle.

---

## 2. What Phase 3 built (file by file)

### Circuit model (`src/circuit/`)
- `bus.rs` — slim `Bus` (`TDSSBus`): `nodes`/`ref_no` allocation lists,
  `kv_base`, coords, `allocate_bus_state`. Node allocation itself lives in
  `Circuit::add_bus` (it needs the global counter).
- `circuit.rs` — `Circuit` (`TDSSCircuit` subset): `add_ckt_element` (device
  list + per-kind `Vec<ElemRef>` lists + 1-based handle), `add_bus`
  (find-or-create + the "Caution: Magic" node_buffer→global-ref rewrite),
  `process_bus_defs` (defaults 1..np then ground; `parse_as_bus_name`
  override; negative-node check), `reprocess_bus_defs` (save → rebuild →
  `allocate_bus_state` → restore kv_base/coords/voltages),
  `set_bus_name_redefined` (raises `system_y_changed`, like the Pascal
  property setter; the ctor starts with the flag **true**), `node_name(i)` =
  the oracle's `YNodeOrder` format (`BUSNAME.N` uppercased), `losses`.
- `terminal.rs` — `Terminal` (`TPowerTerminal`): `term_node_ref`,
  `conductors_closed`, `bus_ref`.

### Element base (`src/elements/`)
- `ckt.rs` — `CktElementData` (`TDSSCktElement` fields): nphases/nconds/
  nterms/yorder, `node_ref`, `vterminal`/`iterminal`/`inj_current`,
  `yprim`/`yprim_series`/`yprim_shunt`, `set_nterms`/`set_nconds` (realloc
  semantics incl. the nterms=0 trick), `set_bus`/`get_bus` (1-based),
  `set_node_ref`, `compute_vterminal`, `do_yprim_calcs` (open-conductor Kron
  reduction with EPSILON pinning), `set_enabled`, and the **signal flags**
  `signal_bus_name_redefined` / `yprim_invalid` (see §3.2).
- `traits.rs` — `CktElement` trait (`RecalcElementData`/`CalcYPrim`/
  `InjCurrents`/`GetCurrents`/`ComputeIterminal`/`Get_Losses` with the PD
  default = Yprim·Vterminal and the PC compensation override per element);
  `ElemRef{cls,idx}` + `ElemStore` (the executive's class registry seen as
  Pascal's pointer lists); `SysCtx` (snapshot of the `ActiveCircuit.Solution`
  scalars elements read); `InjCtx{node_v, currents}`.
- `pc/vsource.rs` — full `TVsource` port: 2-terminal, ZSpecType 1/2/3 with
  `quad_solver` R0 path, sym/asym Z matrix (`CALPHA.conj()`), `get_vmag`
  (n-phase `2·sin(π/n)` formula), quasi-ideal Yprim branch, `inj_currents`
  via `GetVterminalForSource`, `get_currents = Yprim·V − InjCurrent`,
  spec-type switching side effects with `clear_seq` lists.
- `pc/load.rs` — full `TLoad` port: `set_nominal_load` (status/mode factor,
  per-phase W/var, Yeq/Yeq95/Yeq105/Yeq105I/ILow/I95/M95/IBase), `recalc`
  (LoadSpecType interplay, y_neut incl. 1e6 solid ground, yq_fixed), Yprim
  (wye neutral row/col + `·1.000001`; delta `add_sym`; series diag `1e-10`),
  **all 8 `Do*` models verbatim** with the VBaseLow/95/105 zones, the
  compensation-current `inj_currents`/`get_currents` pair, and the full
  kW/PF/kvar/kVA/xfkVA/kWh/Cfactor spec-set side-effect web.
- `pd/line.rs` — `TLine` sym + matrix paths: Zs=(2Z1+Z0)/3 / Zm=(Z0−Z1)/3,
  `CAP_EPSILON` on the series diagonal, Rg/Xg/rho earth correction with kxg,
  per-property scale functions via `DssObject::prop_scale`
  (GetZSeqScale/GetCSeqScale/GetZmatScale/GetYCScale/GetB1B0Scale), units
  conversion side effects, switch defaults, matrix props killing the sym
  model. `linecode`/`geometry`/`spacing`/`wires`/`cncables`/`tscables` are
  flagged `PropFlags::NOT_PORTED` → setting them is a hard parse error.

### Solution (`src/solution/`)
- `solution.rs` — `Solution` (`TSolutionObj` state incl. both sparse sets +
  `ActiveY` selector) and the free functions taking `(ckt, env)`:
  `solve` (mode dispatcher + growth factor) → `solve_snap` (control loop,
  `Iteration = total` at the end) → `solve_circuit` → `do_pflow_solution`
  (`solve_y_direct` init once) → `do_normal_solution` (the fixed-point loop);
  `solve_direct`, `solve_zero_load_snapshot` (SERIESONLY), `converged`
  (NodeVbase-relative |V| error), `solve_system` (factor+solve via
  dss-sparse), `set_voltage_bases` (zero-load solve + nearest legal base,
  with the 0.001732 `TODO(compat)`), `SolveMode` (ordinals = COM/TSolveMode).
- `ymatrix.rs` — `build_y_matrix`: reprocess buses when redefined → fresh
  `SparseSet` → recalc Yprims (all if `frequency_changed`, else invalid
  only) → stamp enabled elements (`CMatrix::to_row_major()` because
  dss-sparse `add_primitive_matrix` is row-major) → `allocate_vi` →
  `initialize_node_vbase`.

### Executive (`src/exec/mod.rs`) — Phase 2 skeleton replaced
- **Full Pascal name lists** registered: all 123 `TExecCommand` names (incl.
  the `DSS_CAPI_PM` extras) and all 119 `TExecOption` names with the Pascal
  spelling replacements (`vr`→`var`, `tilde`→`~`, `SetOpt`→`Set`, `pct`→`%`,
  `cls`→`class`, `typ`→`type`, `obj`→`object`) — abbreviation ownership now
  matches the oracle for *everything*, not a subset.
- `command()` = `ProcessCommand`: Compile/Redirect early exit; the
  before-circuit command set; the **error-301 circuit gate** for everything
  else; the property-reference fallback (`Line.l1.r1=0.2` style) via
  `SetObject` + rebuilt command line.
- `New circuit.x` → `MakeNewCircuit`: creates the `Circuit` then feeds
  `New object=vsource.source Bus1=SourceBus <remainder>` back through the
  executive. Second circuit → the MaxCircuits=1 error.
- `AddObject`: circuit-element path (`requires_circuit`, duplicate warning
  that **skips the edit**, `add_ckt_element` with `ElemKind`), DSS_OBJECT path
  unchanged.
- `Set`/`Get` (`DoSetCmd`/`DoGetCmd` + the `_NoCircuit` variants): year,
  frequency, mode, random, number, tolerance, maxiterations, loadmodel,
  loadmult, norm/emerg volts, %growth, allowduplicates, zonelock,
  voltagebases (1000-slot `parse_as_vector`), algorithm, controlmode,
  cktmodel, basefrequency, maxcontroliter, casename, log,
  DefaultBaseFrequency, NeglectLoadY, miniterations. Unimplemented options
  record "not ported in Phase 3".
- `Solve` = `DoSetCmd(1)` → `do_solve_cmd` → `solution::solve`;
  `CalcVoltageBases` → `set_voltage_bases`; `BuildY` = invalidate PC
  elements; `Init` = `solution_initialized=false`.
- `Redirect`/`Compile` = `DoRedirect`: path resolution against
  `current_dir` (+`.dss` fallback), Pascal block-comment semantics (`/*` only
  at line start, `*/` anywhere), `solution_abort` → `redirect_abort`,
  compile-keeps-dir / redirect-restores-dir, `@lastfile` parser vars.
- `ClassStore` implements `ElemStore` over the class registry; `SolveEnv`
  gets the aux parser (bus-name parsing is config-identical to the main one).
- **Flag propagation** after each edit: `signal_bus_name_redefined` →
  `Circuit::set_bus_name_redefined(true)`; `yprim_invalid && enabled` →
  `system_y_changed = true` (see §3.2).

### Property engine extensions (`src/obj/`)
- New `PropType`s: `Bus`, `Complex`, `DoubleFArray`, `SymMatrixReal/Imag`,
  `Enabled`, `ObjectRef`. New flags: `SCALED_BY_FUNCTION` (scale from
  `DssObject::prop_scale`), `NOT_PORTED` (hard parse error).
- `DssObject` grew: `as_any`, `as_ckt_element(_mut)`, `set/get_bus_name`,
  `set/get_complex`, `set/get_matrix_part`, `prop_scale`.
- `EnumRegistry` grew: scan/sequence/connection/vsource_model/load_model/
  load_status/line_type/**solve_mode** (26 spellings, COM ordinals)/
  solve_alg/control_mode/random_mode/default_load_model/ckt_model.

### dss-cli
- `main.rs`: `dss-cli <script.dss>` → `Compile`, print errors (exit 1 if
  any), print convergence summary + per-node |V|/angle table.

### Gate artifacts
- `tools/golden/gen_slice.py` — 13 scenarios (2-bus parameterized on the
  load; IEEE13-flat inline-matrix script with lengths converted ft→mi since
  the per-line matrices carry the linecodes' per-mile units).
- `tests/golden/slice.json` — committed golden (YNodeOrder, complex node
  voltages, iterations, converged per scenario).
- `crates/dss-core/tests/golden_slice.rs` — the gate test (a–d above).
- `props_roundtrip.rs` now runs the oracle's preamble (`clear` +
  `new circuit.propsprobe`) before each scenario — `?` is circuit-gated.

---

## 3. Key design decisions & rationale (Phase 3)

### 3.1 Element storage stays in the executive; the solver sees `ElemStore`
Kept Phase 2's `Vec<Box<dyn DssObject>>` per class. The circuit holds
`Vec<ElemRef>` lists (`ckt_elements`, `sources`, `lines`, `loads`, `pc/pd`),
and the solution machinery walks them through the `ElemStore` trait +
`as_ckt_element_mut()` bridge. This replaces Pascal's raw pointer lists with
zero unsafe and no double ownership. (Plan §2.1's typed arenas remain an
option for a later refactor — passing tests are the only contract.)

### 3.2 Signal flags instead of `ActiveCircuit` globals *(load-bearing)*
Pascal property setters write circuit globals mid-edit
(`ActiveCircuit.BusNameRedefined := True`, `Solution.SystemYChanged := TRUE`
via `Set_YprimInvalid`). Elements here set `cd.signal_bus_name_redefined` /
`cd.yprim_invalid`, and the executive propagates **after** the edit loop.
Equivalent because nothing reads those globals mid-edit; verified against the
solve flow (the first reader is `BuildYMatrix` at solve time).

### 3.3 Compensation-current loads
Loads are stamped into Y **and** inject `Yprim·V − model current` (Pascal
`StickCurrInTerminalArray` sign conventions), which is what makes the
fixed-point converge in the same iteration count as the oracle — iteration
equality in the gate is the regression test for this.

### 3.4 Context structs of disjoint borrows
`SysCtx` snapshots the solution scalars for `RecalcElementData`/`CalcYPrim`;
`InjCtx` carries `&[node_v]` + `&mut currents`; `SolveEnv` carries
store/parser/vars/errors. No `RefCell`, no globals.

### 3.5 `NOT_PORTED` property flag
Line's catalog references (linecode/geometry/…) hard-error on set instead of
silently parsing into nothing — a script that needs Phase 4 machinery cannot
produce silently-wrong numbers.

---

## 4. Empirical oracle facts added this phase

- `?`, `Edit`, `~`, `Solve`, `Set`, `Get` are **circuit-gated** in
  `ProcessCommand` (error 301); `New`, `Clear` and a fixed list are not.
  (`gen_props.py` always ran `new circuit.propsprobe` — the replay must too.)
- `Solution.Iterations` (COM) = `Solution.Iteration` = the **total** over
  control iterations, assigned at the end of `SolveSnap`.
- `YNodeOrder` = `MapNodeToBus` order 1..NumNodes = exactly the order
  `ProcessBusDefs` allocates global node refs in element-creation order;
  `YNodeVarray` matches `NodeV[1..]` re/im interleaved.
- A per-line matrix spec has **no separate matrix units**: `units=` applies
  to both the matrices and the length (the original IEEE13 worked because the
  *linecode* carried `units=mi` while the line length was in ft). The flat
  variant therefore converts lengths to miles.
- IEEE13-flat (no regulators/transformers/caps): min |V| ≈ 0.88 pu,
  3 iterations; 2-bus family: 2 iterations.

---

## 5. `TODO(compat)` / deferrals (Phase 3 additions)

- `util.rs`: `CALPHA = (-0.5, -0.866025)` (truncated), `CDOUBLEONE`,
  truncated `0.001732` in `set_voltage_bases`, `pdeg_to_complex` with
  `57.29577951` — all marked `TODO(compat)`.
- Newton algorithm (`Set algorithm=newton`) errors "not ported in Phase 3".
- Harmonics/dynamics/yearly/duty solve modes error out; `is_harmonic_model`
  paths in elements exist but are unreachable until Phase 7.
- Line: `linecode`/`geometry`/`spacing`/`wires`/`cncables`/`tscables`
  properties are `NOT_PORTED` (Phase 4); `Seasons`/`Ratings` stored only.
- Load: loadshape/growthshape/CVRcurve object refs stored as strings only
  (no shape lookup until Phase 5); `interpolate_y95i_ylow` family ported.
- Executive: `Show`/`Export`/`Dump`/`Select`/`Enable`/`Disable`/`Open`/
  `Close`/`BatchEdit`/… record "not ported in Phase 3". `Set hour/sec/time/
  stepsize` need DynaVars wiring (Phase 5).
- `Set DataPath` not ported; `current_dir` only follows Redirect/Compile.

Grep `rg "TODO\(compat\)"` for the full marker list.

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
python tools/golden/gen_props.py     # -> tests/golden/props.json   (Phase 2)
python tools/golden/gen_slice.py     # -> tests/golden/slice.json   (Phase 3)
```
Oracle pin: Python 3.12.4, dss-python 0.15.7, dss-python-backend 0.14.5
(the same dss_capi release vendored in `.inputs/dss_capi`). `python` works in
this environment; the `py` launcher is broken — use `python` directly.

---

## 7. Next session — Phase 4 (core PD elements + catalog objects)

> **A full step-by-step execution plan now exists: `PHASE4_PLAN.md`** (work
> packages WP4.1–WP4.10, Pascal line references, design decisions, gate
> procedure). `PHASE5_PLAN.md` is written too. Start from PHASE4_PLAN.md;
> the summary below predates it and is kept for context.

From PORTING_PLAN §Phase 4:
- `Transformer.pas`, `Capacitor.pas`, `Reactor.pas`, `LineCode.pas`,
  `XfmrCode.pas`, the full Line LineCode path; `GrowthShape`, `Spectrum`
  (objects only).
- Introduce the `define_properties!` macro and retrofit the Phase 3 classes
  (the hand-written `match idx` accessor arms in vsource/load/line are the
  natural target).
- **Gate**: `golden_ieee13.rs` / `golden_ieee37.rs` with `controlmode=off`
  (Phase-0 goldens for those feeders are already committed) — voltages/
  powers/losses to 1e-6; property-dump tests for all new classes.

Enabling facts: the `ObjectRef` property type currently stores lowercased
names; LineCode needs real object resolution (executive-level lookup at
parse time, like Pascal's `FetchLineCode`). `ElemKind` needs Transformer/
Capacitor/Reactor variants and `AddCktElement`'s corresponding lists.

---

## 8. Outstanding action

Phase 3 is **complete and gate-green but uncommitted** (working tree on
`phase-2-object-model`, which contains the committed Phase 2). Actions:
1. Commit Phase 3 on this branch (or a new `phase-3-vertical-slice` branch)
   — only on explicit request.
2. Review + merge to `main`.
3. Start Phase 4 (§7).
