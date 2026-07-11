# Porting Plan: DSS C-API (Free Pascal) → Pure Idiomatic Rust

## Source-integrity gate — ritual step 0 (before the model-tier check)

The Pascal we port FROM — `.inputs/dss_capi` (186 `.pas` files), plus
`.inputs/electricdss-tst` for oracle/live work — is the **specification**. Before doing
anything, and re-checked continuously (not only at kickoff), confirm that folder exists
and is non-empty. If it has vanished — missing or empty — at **any** point in the work,
**STOP immediately**: make no edits, run no gate, and do **not** reconstruct, guess, or
"port" a source you cannot read. Tell the user the vendored source is gone and must be
re-vendored, then wait. Reply exactly:
**«Исходник порта (`.inputs/dss_capi`) отсутствует или пуст — работа остановлена. Восстанови
vendored-исходник (re-vendor) и повтори команду.»**
No spec → nothing to port; fabricating one from memory is a silent, unverifiable
divergence — far worse than stopping. This gate runs **ahead of the tier/refuse check**
(`PLAN_SEQUENCE.md` §Model-tier protocol).

## Context

The goal is a complete 1:1 behavioral port of **AltDSS/DSS C-API** — the Free Pascal
alternative implementation of EPRI's OpenDSS power distribution simulator — to pure,
idiomatic, `#![forbid(unsafe_code)]` Rust. Pascal source: `.inputs/dss_capi` (186 .pas
files, ~136k lines). Rust state at planning time: a 4-crate cargo workspace (edition 2024)
where only the RPN calculator is done (`crates/dss-parser/src/rpn.rs`, 18 passing tests)
and a draft of the `ParserDel` tokenizer sits commented-out in `dss-parser/src/lib.rs`.

**Automatic transpilation was researched and rejected**: no viable Pascal→Rust transpiler
exists (only toy AI snippet converters). Object Pascal's OOP/virtual methods/manual memory
model can't be mechanically mapped to ownership semantics. The practical route — confirmed
by community consensus — is manual/LLM-assisted reimplementation, reading the Pascal as the
specification, validated numerically against the original engine.

**Binding decisions:**
1. **API surface**: idiomatic Rust-native library + CLI that runs `.dss` scripts. The
   60-file `src/CAPI` wrapper layer (~30k lines) is **out of scope** — it exists only to
   export C functions; its functionality is covered by the native Rust API. No unsafe.
2. **Sparse solver**: pure-Rust **faer** (sparse LU, complex f64), wrapped behind our own
   module mirroring the KLUSolve call surface (verified: `src/Common/KLUSolve.pas` is a
   96-line, ~17-function handle-based API — maps cleanly to one owned Rust struct).
3. **Scope**: plan covers 100% of the engine, but exotic subsystems (CIM XML export,
   A-Diakoptics, parallel-machine actor mode, GIC elements, plot callbacks) are the final
   phase — stopping before it still yields a complete usable simulator.
4. **No backward-compat between phases**: later phases may freely refactor earlier code;
   the regression test suite is the only contract.
5. Prefer existing crates (`num-complex`, `faer`, `csv`, `serde`, `thiserror`, `proptest`)
   over reinventing.

**"Port the math model first?"** — No: use a **vertical-slice strategy**. Phase 3 builds the
thinnest complete path (parse → build circuit → Y matrix → solve → voltages, validated
against the oracle), then later phases widen it element-by-element. This surfaces
architecture mistakes early, when they're cheap, and gives a working simulator at every
gate from Phase 3 onward.

**Oracle for validation**: `dss-python` (pip) wraps the *very same* dss_capi engine —
a perfect reference. Test circuits come from `github.com/dss-extensions/electricdss-tst`
(IEEE 13/34/37/123-bus, 8500-node, EPRI feeders).

---

## 1. Workspace & Crate Architecture

Keep 4 crates; rename `dss-solver` → `dss-sparse` (its real role):

```
crates/
  dss-parser/   # TDSSParser tokenizer + RPN (RPN already done)
  dss-sparse/   # faer-backed complex sparse LU behind the KLUSolve-shaped API
  dss-core/     # the entire engine — one crate, many modules
  dss-cli/      # binary: runs .dss scripts / command loop
```

One big `dss-core` (not crate-per-Pascal-dir): the Pascal units are mutually recursive
(Circuit ↔ Solution ↔ CktElement ↔ EnergyMeter); modules tolerate cycles, crates don't.
`dss-parser`/`dss-sparse` stay separate as genuine dependency-free leaves.

`dss-core` module tree (mirrors Pascal dirs):

```
dss-core/src/
  lib.rs        # pub struct Dss (= TDSSContext), public API
  support/      # src/Shared: cmatrix/mod.rs (TCMatrix), mathutil/mod.rs (sym components),
                #   hashlist/mod.rs, line_units/mod.rs, dynamics/mod.rs, ckt_tree/mod.rs, pstcalc.rs,
                #   line_constants/ (Carson engine: oh, cn, ts, cable)
  obj/          # DSSObject/DSSClass/DSSObjectHelper replacement:
                #   base/mod.rs (DssObjData, CktElementData, PdElementData, PcElementData)
                #   props/mod.rs (PropDef, PropType, PropFlags, generic value parsing)
                #   class_def.rs (per-class metadata registry, abbreviation matching)
                #   macros.rs (define_properties! DSL)
  circuit/      # Circuit.pas, Bus.pas, Terminal.pas
  solution/     # Solution.pas, Ymatrix.pas, SolutionAlgs.pas, ControlQueue.pas
  elements/
    pd/         # line/mod.rs, transformer/mod.rs, capacitor/mod.rs, reactor/mod.rs, autotrans.rs, fault.rs...
    pc/         # vsource/mod.rs, isource.rs, load/mod.rs, generator/mod.rs, pvsystem.rs, storage.rs...
    control/    # regcontrol.rs, capcontrol.rs, relay.rs, invcontrol.rs...
    meter/      # energymeter/mod.rs, monitor/mod.rs, sensor/mod.rs, reduce.rs
    general/    # loadshape.rs, linecode.rs, line_geometry/mod.rs, wiredata.rs,
                #   xfmrcode.rs, xycurve.rs, spectrum/mod.rs, tcc_curve/mod.rs,
                #   growth_shape/mod.rs...
  exec/         # Executive.pas, ExecCommands.pas, ExecHelper.pas, ExecOptions.pas
  report/       # ShowResults.pas, ExportResults.pas, Show/ExportOptions, save.rs
  cim/          # ExportCIMXML.pas (GAPS_PLAN WPG.18)
```

Dependencies: `num-complex`, `faer` (dss-sparse only), `thiserror`, `serde`/`serde_json`
(golden tests/exports), `csv`; dev: `proptest`, `criterion`. `#![forbid(unsafe_code)]`
in every crate. `src/lazutf8` is irrelevant (Rust strings are UTF-8 natively).

## 2. Core Design Patterns (load-bearing decisions)

### 2.1 Class hierarchy → composition + traits + typed `Vec` arenas

Pascal `TDSSObject → TDSSCktElement → TPDElement/TPCElement → concrete` becomes
**embedded base-data structs** (composition, not inheritance):

```rust
pub struct DssObjData { name: String, prp_sequence: Vec<u32>, ... }
pub struct CktElementData { obj: DssObjData, enabled: bool, nterms: usize, nconds: usize,
    nphases: usize, bus_names: Vec<String>, node_ref: Vec<NodeId>,
    yprim: Option<CMatrix>, yprim_invalid: bool, base_frequency: f64, ... }
pub struct Load { pc: PcElementData, kw: f64, kvar: f64, daily: Option<Idx<LoadShape>>, ... }
```

**Behavior traits** with data accessors; Pascal's shared base logic (ComputeIterminal,
power/losses getters) lives in default methods:

```rust
pub trait CktElement {
    fn cd(&self) -> &CktElementData;
    fn cd_mut(&mut self) -> &mut CktElementData;
    fn calc_yprim(&mut self, ctx: &YPrimCtx) -> DssResult<()>;       // virtual CalcYPrim
    fn get_currents(&self, ctx: &SolveView, out: &mut [Complex64]);  // virtual GetCurrents
    fn recalc_element_data(&mut self, ctx: &RecalcCtx) -> DssResult<()>;
    // default impls: compute_vterminal, power(term), losses, set_node_ref, ...
}
pub trait PcElement: CktElement { fn inj_currents(&mut self, ctx: &mut InjCtx) -> DssResult<()>; }
```

**Storage**: plain `Vec<T>` typed arenas in `Elements`, with `Idx<T>(u32, PhantomData)`
newtype indices. No `Rc<RefCell>`, no slotmap — OpenDSS never deletes individual elements
mid-script (only whole-circuit `Clear`), so indices are stable. Every Pascal raw pointer
cross-reference (`DailyShapeObj`, `ControlledElement`, …) becomes a typed index. A
catch-all `enum ElemId { Line(Idx<Line>), Load(Idx<Load>), ... }` plus match-based
`Elements::get(_mut)(&self, id) -> &(mut) dyn CktElement` gives uniform dispatch for
BuildYMatrix-style loops while keeping storage monomorphic.

**Borrow-checker strategy** (replaces Pascal's "reach through ActiveCircuit globals"):
element methods never touch the whole circuit; they take small **context structs of
disjoint borrows** (e.g. `InjCtx { node_v: &[Complex64], currents: &mut [Complex64],
shapes: &Shapes, dynavars: &DynaVars, ... }`) built by destructuring at the call site.
Controls editing elements mid-solve already go through Pascal's ControlQueue
(sample → queued action → apply → `system_y_changed` check) — port that architecture
as deferred action lists; fallback for stubborn flows: `std::mem::take` the element,
mutate, put back.

### 2.2 Property system (replaces DSSObjectHelper's pointer-offset reflection)

Pascal pokes fields via `ptruint(@obj.Field)` offsets — impossible in safe Rust. Per class:
1. A property enum mirroring the Pascal `TProp` ordinals (ordering matters for
   `PrpSequence`/Save output).
2. A static `ClassDef` of `PropDef { name, prop_type, flags, enum_map, scale, ... }`
   rows, ported from DSSClass.pas — these drive generic parsing (arrays, matrices,
   interval units, object refs, mapped enums).
3. The generic string/array parsing from `DSSObjectHelper.ParseObjPropertyValue` ported
   **once** into `obj/props/mod.rs`, producing typed `PropValue`.
4. Per-class `PropertyApi` impl: `set_value`/`get_value`/`side_effects` as match arms
   instead of pointer offsets — `PropertySideEffects` ported verbatim per class (this is
   where kW/PF/kvar/kVA interplay and other hidden behavior lives).
5. A `define_properties!` macro_rules DSL generates the enum + table + trivial field-map
   arms with custom escape hatches. Hand-write the first 3 classes (Vsource, Line, Load)
   to discover the right macro shape; introduce the macro in Phase 4.
6. Replicate `TCommandList` abbreviation matching exactly (scripts rely on `kv`, `bus1=`).

### 2.3 TDSSContext → `Dss` struct; errors; indexing

The Pascal codebase already encapsulates all state in `TDSSContext` (no raw globals) — it
ports directly to `pub struct Dss { circuit: Option<Circuit>, classes: ClassRegistry,
parser, options, error_sink, ... }` with `Dss::command(&mut self, &str)` and
`Dss::compile(&mut self, path)`. No statics.

- Errors: `DssError { number: i32, message, kind }` via `thiserror`. Pascal's
  `DoSimpleMsg` (record-and-continue) → `error_sink.push(...)`; genuinely aborting paths
  (solver failures, SolutionAbort) → `Err`. Keep Pascal error numbers for comparability.
- Identifiers: case-insensitive via lowercase-normalized `HashMap` keys (THashList semantics).
- Node indexing keeps Pascal's load-bearing **ground = node 0** convention
  (`node_v[0] == 0`, `NodeRef == 0` means grounded) via `NodeId(u32)` newtype with
  `NodeId::GROUND`; everything else 0-based with conversions at parse/report boundaries.

### 2.4 dss-sparse: faer behind the KLUSolve surface

Mirror `src/Common/KLUSolve.pas` as one owned struct (ownership replaces handles):

```rust
pub struct SparseSet { ... }
impl SparseSet {
    pub fn new(n: usize) -> Self;                                   // NewSparseSet
    pub fn zero(&mut self);                                         // ZeroSparseSet
    pub fn add_primitive_matrix(&mut self, nodes: &[NodeId], yprim: &CMatrix); // skips ground
    pub fn factor(&mut self) -> Result<(), SparseError>;            // Singular { col }
    pub fn solve(&mut self, b: &[Complex64], x: &mut [Complex64]) -> Result<(), SparseError>;
    pub fn nnz(&self) -> usize;          pub fn sparse_nnz(&self) -> usize;
    pub fn compressed(&self) -> CscView<'_>;                        // GetCompressedMatrix
    pub fn find_islands(&self) -> Islands;                          // we implement (union-find)
    pub fn rcond(&self) -> f64;          pub fn singular_col(&self) -> Option<usize>;
    pub fn get_element(&self, i, j) -> Complex64;                   // CheckYMatrixforZeroes
}
```

Internals: triplet accumulation → sorted dedup → `faer` `SparseColMat<usize, c64>` → sparse
LU. `find_islands`/`rcond`/`singular_col` are KLUSolveX extensions we implement ourselves.
Incremental-Y (`IncrementMatrixElement` etc., behind `DSS_CAPI_INCREMENTAL_Y`) is an
optimization — defer/skip; full rebuild is always correct.

---

## 3. Phased Roadmap

Effort % = share of total port. Every phase ends with a **hard testing gate**.
Phases 4+ each get a dedicated step-by-step execution plan (`PHASE<N>_PLAN.md` at the
repo root, written just-in-time before the phase starts) breaking the summary below
into small work packages with Pascal line references and per-WP definitions of done;
when such a file exists it supersedes the summary here for execution purposes.

### Phase 0 — Tooling, oracle, faer spike, CI (~3%)
- Save this plan as `PORTING_PLAN.md` in repo root.
- Clone `electricdss-tst` into `.inputs/electricdss-tst`.
- `tools/golden/generate.py` using **pinned** `dss-python` (record version in
  `tools/golden/PIN.txt`): for each test case dump JSON goldens — node names/order,
  complex node voltages, per-element terminal currents/powers, total losses, transformer
  taps, regulator/cap states, iteration counts, energymeter registers, monitor channels,
  event log, full property dump of every object. Commit under `tests/golden/`
  (regeneration manual, never in CI).
- Shared comparison harness: `crates/dss-core/tests/harness/mod.rs` (golden loader +
  tolerance policy).
- **faer spike**: prototype `dss-sparse` factoring/solving a c64 sparse system at ~25k
  nodes; record perf numbers. Nothing else proceeds until this is known.
- CI (GitHub Actions): `cargo fmt --check`, `clippy -D warnings`, `cargo test` on
  windows + ubuntu.
- **Gate**: harness runs end-to-end against a stub engine; spike numbers recorded;
  goldens for IEEE13/34/37/123 committed.

### Phase 1 — Foundations: shared math + parser completion (~7%)
- Scope: `Shared/UComplex`+`DSSUcomplex` (thin extensions over num-complex),
  `Shared/Ucmatrix.pas` (dense complex TCMatrix incl. invert + Kron reduction),
  `Shared/MathUtil.pas` (symmetrical components etc.), `LineUnits`, `Dynamics` (DynaVars),
  `HashList`; finish the `ParserDel.pas` port in dss-parser (delimiters, quote pairs
  `()[]{}"'`, vector/matrix syntax, `@variables`, inline RPN, `file=` array loading —
  the commented draft in `lib.rs` is the starting point).
- **Gate**: ~60 unit tests — tokenizer cases lifted from Pascal semantics; cmatrix invert
  vs hand-computed 3×3; sym-component round-trip to 1e-12.

### Phase 2 — Object model, property engine, executive skeleton (~10%)
- Scope: `NamedObject`, `DSSObject`, `DSSClass`/`DSSClassDefs` (constants → enums),
  `DSSObjectHelper` generic parse/get paths, `Shared/Command.pas` abbreviation matching,
  `Executive.pas` + the subset of `ExecCommands`/`ExecHelper` for: `New`, `Edit`, `~`,
  `Set`/`Get`, `?`, `Redirect`/`Compile`, `Clear`, `like=` (MakeLike).
- **Gate**: `tests/props_roundtrip.rs` — for each implemented class: create with defaults,
  dump every property, compare against dss-python all-defaults golden; scripted edits in
  odd orders and via abbreviations, compare dumps.

### Phase 3 — ★ VERTICAL SLICE: minimal end-to-end power flow (~15%)
- Scope: `Bus.pas`, `Terminal.pas`, `Circuit.pas` (subset: AddCktElement, bus/node
  mapping, ProcessBusDefs), `CktElement.pas` (full base), `PDElement`/`PCElement` bases,
  `VSource.pas`, `Line.pas` (R1/X1/R0/X0/C1/C0 + rmatrix paths; LineCode in Phase 4),
  `Load.pas` (all 8 load models), `Ymatrix.pas` (full-rebuild path), `Solution.pas`
  subset (SnapShotInit, SolveSnap, DoNormalSolution, DoPFLOWsolution, Converged,
  injection-current machinery, SolveSystem, InitializeVoltages), `CalcVoltageBases`
  (incl. its zero-load solve), `Set voltagebases/mode/maxiterations/tolerance`.
- Deliverables: dss-sparse production-ready; CLI runs a script and prints voltages.
- **Gate**: `tests/golden_slice.rs` —
  (a) 2-bus vsource+line+load matches dss-python voltages to 1e-9 rel;
  (b) "IEEE13-flat" variant (transformers/regulators/caps stripped) matches to 1e-6 rel;
  (c) fixed-point iteration counts equal;
  (d) all 8 load models exercised and matching.

### Phase 4 — Core PD elements + catalog objects (~10%)
> **Detailed execution plan: [`PHASE4_PLAN.md`](PHASE4_PLAN.md)** — work packages
> WP4.1–WP4.10 with Pascal line references, the object-reference-resolution and
> control-element design decisions, the `define_properties!` macro spec, and the
> controls-off variant-script gate procedure. Execute that file, not this summary.
- Scope: `Transformer.pas`, `Capacitor.pas`, `Reactor.pas`, `LineCode.pas`,
  `XfmrCode.pas`, full `Line.pas` LineCode path; `GrowthShape`, `Spectrum` (objects only);
  RegControl/CapControl as parse-only objects (the IEEE masters create them).
  Introduce `define_properties!` macro; retrofit Phase 3 classes.
- **Gate**: `golden_feeders.rs` — IEEE13/IEEE37/IEEE123 controls-off variant scripts vs
  new `tests/golden/feeders_controlsoff.json` (oracle run the same way) — voltages/powers/losses to
  1e-6 rel, iteration counts exact; property-dump tests for all new classes.

### Phase 5 — Control loop, RegControl/CapControl, time-series modes (~10%)
> **Detailed execution plan: [`PHASE5_PLAN.md`](PHASE5_PLAN.md)** — work packages
> WP5.1–WP5.10 with the control-sampling borrow design (`CtrlCtx`), ControlQueue
> semantics, the FPC-rounding tap-computation pitfall, and the gate test inventory.
> Execute that file, not this summary.
- Scope: `ControlQueue.pas`, `ControlElem.pas`, `RegControl.pas`, `CapControl.pas`,
  `LoadShape.pas`, `TempShape`, `PriceShape`, `XYcurve`, rest of `Solution.pas` +
  `SolutionAlgs.pas` for Snap/Daily/Yearly/DutyCycle, event log.
- **Gate**: IEEE13 full master (regulator+caps): final tap positions **exactly** equal,
  voltages 1e-6; IEEE123 with regulators: same — both vs the committed Phase-0 goldens;
  event log equality (numbers normalized); a daily-mode loadshape case matches hourly
  voltage trajectories (`tests/golden/timeseries_controls/`).

### Phase 6 — Meters, monitors, topology, Generator; large-feeder gate (~12%)
- Scope: `MeterElement`, `EnergyMeter.pas` (zones, registers, SAIFI/SAIDI),
  `Monitor.pas`, `Sensor.pas`, `CktTree.pas` + circuit topology, `ReduceAlgs` (basic),
  `Generator.pas` (user-model DLL loading stubbed — out of scope per safe-Rust),
  `AutoAdd.pas`, `GenDispatcher`, `StorageController` skeleton.
- **Gate**: `golden_ieee8500.rs` — 8500-node converges with matching iteration count,
  voltages 1e-6, energymeter registers 1e-4 rel after a daily run; monitor channels match
  elementwise; solve time recorded (initial budget: within 5× of dss_capi).

### Phase 7 — Extended elements: DER, protection, line constants, harmonics, dynamics (~18%, largest)
Sub-blocks, each independently gated with targeted electricdss-tst cases:
1. Line constants: `WireData/CNData/TSData/CableData/LineSpacing/LineGeometry` +
   `OH/CN/TS/Cable LineConstants` (Carson's equations) — gate: geometry-based feeders match.
2. DER: `Storage.pas`, `PVSystem.pas`, `InvBasedPCE.pas`, `InvControl.pas`,
   `ExpControl.pas`, full `StorageController` — gate: InvControl/Storage test scripts.
3. Protection: `Relay.pas`, `Recloser.pas`, `Fuse` (+`TCC_Curve.pas`), `SwtControl.pas`,
   `Fault.pas` — gate: fault + protection sequences, event logs equal.
4. Harmonics: harmonic solution mode, Spectrum application, harmonic source models —
   gate: harmonics cases (distortion outputs).
5. Dynamics: `Dynamics` integration, machine models, `IndMach012.pas`, `DynamicExp`,
   `DynEqPCE`, `VCCS`, `UPFC`+`UPFCControl`, `VSConverter`, `ESPVLControl` —
   gate: dynamics-mode monitor trajectories, 1e-5 rel.
6. Faultstudy + AutoAdd modes, `Feeder.pas`.

### Phase 8 — Reporting, exports, Save, full Executive (~12%) — **COMPLETE (2026-07-10)**

> **Executed** (`PHASE8_PLAN.md` WP8.1–WP8.8; records in `STATUS.md` +
> `docs/phase-records/phase-8.md`). The WP8.8 exit sweep additionally ported the
> whole corpus-used executive tail (Enable/Disable, SetkVBase, Losses, Summary,
> Reconductor, the `_InitSnap…_SolvePFlow` step-solution family, `var`,
> Fileedit/Classes/Userclasses/CD/DOScmd, `Set/Get ShowExport`), the AutoTrans
> arms of the element-form Show reports, and the dynamics/harmonics-leave
> `InvalidateAllPCElements` rebuild trigger. `tools/cmd_coverage.py` proves the
> only corpus-used residual is the `DSS_CAPI_PM` actor family
> (NewActor/SolveAll/Abort/Clone + ActiveActor/CPU/Parallel/ConcatenateReports)
> — owner: `MULTITHREADING_PLAN.md` stage M2 (actor mode).

- Scope: `ShowResults.pas` (3.4k), `ExportResults.pas` (3.4k), `Show/ExportOptions`,
  remaining `ExecHelper`/`ExecCommands` long tail (batchedit, interpolate, distribute,
  reduce, …), `Circuit.Save`, `DumpProperties`, `Utilities.pas` leftovers, full ReduceAlgs.
- Build `tools/cmd_coverage.py`: list every DSS command/option used across
  electricdss-tst to prioritize and prove tail coverage.
- **Gate**: export CSVs diffed numerically vs oracle exports for IEEE13/34/37/123/8500;
  `Save circuit` output re-compiles and re-solves to identical voltages
  (`tests/save_roundtrip.rs`).

### Phase 9 — Exotics (~8%, optional — stopping before this still = complete simulator)

> **GAPS_PLAN executed (2026-07-09):** WPG.1–WPG.18 + the WPG.17 exit sweep are
> COMPLETE (see `GAPS_PLAN.md` §WPG.17 closure addendum and `STATUS.md`). The
> closure round additionally ported the sweep-surfaced items (XYcurve file
> props, LoadShape MemoryMapping + TotalTime, SngSave/DblSave,
> PreserveNodeVoltages, the Monitor 1024-flush, the dynamics-mode GFM branch —
> retiring the WPG.13 deferral — and the Plot/Visualize callback surface with
> `Dss::register_plot_callback` for GUI hosts). Named follow-ups: WPG.19
> (non-MM `File=` arrays), WPG.20 (MMF-shape save), `JSON_EXPORT_PLAN.md`
> (user-deferred JSON output). The remaining Phase-9 scope below is the
> A-Diakoptics/actor/Pstcalc residue — the actor half lives in
> `MULTITHREADING_PLAN.md` (M2); the A-Diakoptics + Pstcalc halves now have their
> own execution plan, **`DIAKOPTICS_PSTCALC_PLAN.md`** (2026-07-11): Part I
> (Pstcalc command, Monitor mode-4 flicker, incidence matrix + Sparse_Math) is
> oracle-gated and runs pre-acceptance; Part II (the A-Diakoptics engine) runs
> last, gated rust-vs-rust because the pinned oracle build has
> `DSS_CAPI_ADIAKOPTICS` compiled out. Ordering in `PLAN_SEQUENCE.md`.

- ~~`ExportCIMXML.pas` (4.5k lines, pure output → XML diff vs oracle)~~ —
  **moved to `GAPS_PLAN.md` WPG.18** (byte-exact golden XML gate via the
  `uuids file=` determinism recipe) — **executed**.
- ~~A-Diakoptics + parallel-machine actor mode → re-architect on `std::thread` + channels;
  gate: numerically identical to single-actor results.~~ — **moved to
  `MULTITHREADING_PLAN.md` (2026-07-06):** actor mode = stage M2 there (same
  std::thread + channels design, gate unchanged); A-Diakoptics, deferred there,
  is now **`DIAKOPTICS_PSTCALC_PLAN.md` Part II (2026-07-11)** — runs last in
  `PLAN_SEQUENCE.md` (early-start after M2).
- ~~GIC elements (`GICLine`, `GICsource`, `GICTransformer`)~~ — **moved to
  `GAPS_PLAN.md` WPG.16**; ~~`Pstcalc` flicker (if not
  already pulled in by Monitor)~~ — **moved to `DIAKOPTICS_PSTCALC_PLAN.md` Part I
  (WP-PF.1 command + WP-PF.2 Monitor mode 4, oracle-gated, pre-acceptance)**;
  plotting callbacks as a data-only `PlotSink` trait.

**Cumulative**: P0–P3 ≈ 35% → working vertical slice; P0–P6 ≈ 67% → production-usable
simulator; P7–P8 → full behavior parity; P9 → 1:1 including exotics.

---

## 4. Testing Strategy (woven through every phase)

- **Oracle**: pinned dss-python generates committed JSON goldens (same engine as the
  Pascal source — ideal reference). Goldens are self-describing (embed input options).
- **Tolerance policy** (one `Tolerances` struct in the harness):
  pure math 1e-12..1e-9 abs · node voltages 1e-6 rel (floor 1e-9 pu) · angles 1e-6 rad ·
  powers/losses 1e-6 rel · energymeter accumulations 1e-4 rel · discrete states (taps,
  switch states, control action counts, island counts) **exact** · text outputs compared
  after parsing numbers out, never raw float-string diffs. Every field-specific
  exception to this is recorded in `tests/TOLERANCE_NOTES.md` (no blanket relaxations).
- **Checkpointed-model gate** (`golden_checkpoints.rs` / `gen_checkpoints.py`): the
  command-replay gates above compare only converged outputs, which let an
  assembled-model bug (a stale admittance that still converges to nearly-right
  voltages) hide until it accumulates into meters/registers. This gate captures
  the **assembled model after every committed time step** — the unfactored system
  Y (full CSC on micro/feeders, fingerprint + selected YPrim blocks on large
  feeders), per-element YPrim, injection vector, voltages, and discrete state —
  so such a bug fails at the step and matrix entry it first appears. The Y is
  compared unfactored, so the `dss-sparse` row equilibration is out of scope.
- **Live corpus gate** (`corpus_live.rs` / `tools/oracle/oracle_server.py`; see
  `CORPUS_TEST_PLAN.md`): the entire `electricdss-tst` corpus is vendored into
  `tests/corpus/electricdss-tst/` (so tests never depend on the temporary
  `.inputs/`), with every `.dss` accounted for in exactly one manifest under
  `tests/corpus/manifests/` (bijection enforced by `corpus_manifest.rs`, always
  on). For each `solvable_now` case an **opt-in** (`DSS_LIVE_ORACLE=1`) gate runs
  the Rust engine and the pinned oracle **live** (no goldens) and compares the
  full model — full Y, voltages, every element's currents/powers, YPrim,
  injection, discrete state — reusing the checkpoint gate's comparators and
  policy. The `solvable_now` manifest expands toward 100% as the port matures.
- **Official-EPRI-OpenDSS oracle channel** (`tools/opendss/`, added 2026-07-07;
  **opt-in only — the mandatory gate and the pinned dss-python oracle are
  unchanged**): the same `oracle_server.py` protocol and captures driven over
  vendored official EPRI `OpenDSSDirect.dll` binaries (r3723 = the 0.14.x base,
  r4088 = the 0.15.x base, r4133 = release 11.0.0.1) through the AltDSS Oddie
  bridge (`DSS_ORACLE_ENGINE=oddie`; separate venv, `tools/opendss/PIN_OPENDSS.txt`).
  Two workflows: `corpus_live_opendss` (`DSS_LIVE_OPENDSS=<rev>`, divergence
  *report* mode — the port is calibrated to dss_capi, which intentionally
  differs from EPRI upstream) and `tools/opendss/ab_compare.py` (EPRI-vs-EPRI
  diff = the upstream-change inventory for the future "port newer OpenDSS
  behavior" work). See `tools/opendss/README.md`.
- **Unit tests**: inline `#[cfg(test)]` (existing convention); golden values obtained by
  probing the Pascal behavior through dss-python (e.g. a single Line's YPrim entries).
- **Auto-generated property tests**: every registered class gets default-dump +
  scripted-edit dump comparison (catches side-effect bugs — the highest-risk area).
- **Property-based tests** (proptest): parser round-trips; dss-sparse vs dense cmatrix
  solve on random small well-conditioned complex systems; RPN vs reference.
- **Integration tests**: one file per feeder/scenario (`golden_ieee13.rs`, …,
  `golden_dynamics.rs`), thin drivers over the shared harness; slow ones
  `#[ignore]`-by-default, run nightly.
- **CI**: PR job = fmt + clippy + unit + fast integration; nightly = full feeder suite +
  8500-node criterion benchmark with regression alert.

### 4.1 `TODO(compat)` — bug-for-bug compatibility markers

Wherever the Rust code deliberately reproduces an upstream inexactness or bug from the
Pascal/C/Python stack **only** to stay numerically identical to the oracle, the site is
marked with a `TODO(compat):` comment stating what the quirk is and what the clean
replacement will be. Examples already in the tree: the truncated
`pi = 3.14159265359` / `rad→deg = 57.29577951` constants in the RPN calculator and
`DSSUcomplex` polar helpers, the hand-rolled `ATAN2`, FPC `Round`'s integer-indefinite
path (`inf` → 0), the single-point "standard deviation = the value itself" quirk in
`RCDMeanAndStdDev`, `TcMatrix.Invert` leaving the matrix half-transformed on a singular
pivot, and the unchecked zero pivot in Kron reduction.

Rules:
1. Every such site **must** carry `TODO(compat):` — they are intentionally greppable
   (`rg "TODO\(compat\)"`); nothing else may use that tag.
2. Each marker explains the quirk, where it came from, and the intended clean fix.
3. They are **not** to be "fixed" during the port — the goldens pin them, and silently
   improving precision is indistinguishable from a porting bug in the gates.
4. **After the 1:1 port is finished** (final acceptance in §6 green), the markers are
   swept in one dedicated cleanup pass: replace each quirk with the correct/precise
   implementation and regenerate the affected goldens deliberately, one quirk at a time.

   > **Update (2026-07-06, supersedes rule 4's "replace" — rules 1–3 unchanged):** the
   > sweep is now **Stage F of `DE_PASCALIZE_PLAN.md`** (Part IV.2). Compat quirks are
   > **not deleted** — each becomes a dual kernel behind `#[cfg(feature =
   > "oracle-parity")]`: the default build gets the correct/precise implementation, the
   > parity build keeps the quirk so every 1:1 oracle gate stays permanently re-runnable.
   > Only the default lane re-baselines. See `PLAN_SEQUENCE.md` for the post-acceptance
   > order (DE_PASCALIZE → RESONANCE → MULTITHREADING).

## 5. Risk Register

| Risk | L/I | Mitigation |
|---|---|---|
| faer sparse LU: c64 gaps, perf, singular-detection ergonomics | M/H | Phase 0 spike before any engine code; `dss-sparse` isolates the choice. Fallbacks: (1) faer tuning; (2) write our own KLU-style solver (BTF+AMD+left-looking LU, ~3k lines, pure Rust); (3) dense solve for small circuits as dev stopgap. Never C bindings. |
| Pascal float semantics (libm ULPs, Format rounding) | H/L-M | Same x86-64 f64 model; ULP differences absorbed by 1e-6 tolerances; parse numbers, never compare formatted strings; document exceptions in `tests/TOLERANCE_NOTES.md`. |
| Iteration-count divergence flips discrete control actions | M/M | Gate on exact final tap/switch equality — divergence indicates a real port bug, not tolerance. Port `Converged` (incl. NaN/Inf checks) exactly. |
| Property-system hidden behavior (side-effect order, kW/PF/kvar/kVA spec-sets, abbreviations) | H/H | Highest risk. Auto-generated dump tests per class from Phase 2 on; port `PropertySideEffects` verbatim; port `TCommandList` matching exactly with its own tests. |
| ExecHelper's 4.5k lines of command quirks | M/M | Port on demand, prioritized by `cmd_coverage.py` over the test corpus; Phase 8 sweeps the tail. |
| Borrow-checker fights in reentrant flows (controls editing elements, zone traversal) | M/M | Ctx-struct disjoint borrows + ControlQueue deferred actions (already Pascal's design); fallback `std::mem::take`. Phase 5 gate exercises the worst path. |
| 1-based/0-based off-by-one class of bugs | H/M | `NodeId` newtype with `GROUND=0`; ground-at-0 solution arrays preserved; conversions only at boundaries; review checklist item. |
| Oracle version skew vs the exact Pascal commit in `.inputs` | L/M | Pin dss-python version; on mismatch check upstream changelog before assuming a port bug. |
| Scale (~136k lines) | H/– | Vertical-slice ordering → usable simulator at every gate from Phase 3; phases 7–9 cleanly separable; no backward-compat burden between phases. |

## 6. Verification (end-to-end)

At any point: `cargo test` (unit + fast integration) and
`cargo test -- --ignored` (full feeder suite) must be green; per-phase gates as listed.
Final acceptance for the 1:1 port: all electricdss-tst cases covered by
`cmd_coverage.py` run through both engines with the harness reporting zero
out-of-tolerance values and exact discrete-state matches, plus `save_roundtrip` and
export-diff suites green on IEEE 13/34/37/123/8500.

> **EXECUTED 2026-07-11 (branch `final-acceptance`), on explicit user request.** A
> max-effort referee returned `criteria_met=true` / `blocking_items=[]`: `corpus_live`
> live-compares all 245 solvable_now decks (zero out-of-tolerance, exact discrete
> state) + the 3 family suites, `corpus_manifest` bijection green; `save_roundtrip`
> 6/6 and `golden_reports` 193/0 on IEEE 13/34/37/123/8500; full gate exit 0. The
> criteria→evidence table, named non-blocking residuals + owners, and the settled
> audit findings are the top entry of `STATUS.md` §1.

> **Update (2026-07-06) — verification after acceptance.** Final acceptance itself is
> unchanged (it runs on the 1:1 engine — the configuration that becomes the
> `oracle-parity` build). From `DE_PASCALIZE_PLAN.md` Stage F onward, verification is
> **two permanent CI lanes**: the **parity lane** (`--features oracle-parity`) keeps this
> section's full gate — byte goldens, calibrated floors, exact discrete states and
> iteration counts — bitwise-green forever; the **default lane** (idiomatic kernels,
> upstream bugs fixed, parallel LU / iterative refinement allowed) keeps the same floors
> on continuous quantities and exact discrete states but **unpins iteration counts**,
> and is anchored to the oracle transitively via the parity↔default differential gate.
> Drift model and lane rules: `DE_PASCALIZE_PLAN.md` Part IV.2; ordering:
> `PLAN_SEQUENCE.md`; per-step ritual: as in `PHASE8_PLAN.md`, adopted by every
> post-acceptance plan. Executor/auditor **model tiers** (per-stage exec+audit
> requirements, the step-0 refuse protocol, explicit model override for spawned
> auditors): `PLAN_SEQUENCE.md` §Model-tier protocol — adopted by `PHASE8_PLAN.md`
> and every subsequent plan.

## 7. Immediate first actions

1. Write `PORTING_PLAN.md` to repo root (this document). ✔
2. Phase 0 in full — especially the **faer spike** and the golden generator; nothing else
   proceeds until spike numbers are known.
3. Rename `crates/dss-solver` → `crates/dss-sparse`; add deps; add
   `#![forbid(unsafe_code)]` everywhere.
4. Finish the `ParserDel` port in dss-parser (Phase 1).

## Key Pascal reference files

- `.inputs/dss_capi/src/Common/Solution.pas` — solve loop, convergence, modes (behavioral heart)
- `.inputs/dss_capi/src/Common/KLUSolve.pas` — the exact call surface dss-sparse mirrors
- `.inputs/dss_capi/src/General/DSSObjectHelper.pas` — generic property engine to port once
- `.inputs/dss_capi/src/Common/CktElement.pas` — base behavior → CktElementData + trait defaults
- `.inputs/dss_capi/src/Common/Circuit.pas` — arenas, bus/node mapping, ProcessBusDefs
- `.inputs/dss_capi/src/Common/Ymatrix.pas` — Y assembly / rebuild flow
