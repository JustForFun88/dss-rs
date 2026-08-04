# DE_PASCALIZE R1 + R2 — typed arenas and the injection seam

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### DE_PASCALIZE R2 — PARTIAL: M3b injection seam landed; flip + downcast categories ESCAPED (branch `depas-r2`)

Stratum **[A]** bit-neutral. Base `update` @ `1f0b768`. R2's brief was the full
Part I finish: (1) flip `ElemRef → ElemId` across the spine; (2) Category A pair
getters; (3) Category B meter reads; (4) Category D typed object-ref handle; (5)
Category E `make_like`; (6) remove `as_any`/`as_ckt_element` from `DssObject`; (7)
the thread-readiness (M3b) injection-seam rider; (8) flag R3 candidates. **Only
item 7 landed this session** — a complete, gate-green, committed cluster. Items
1–6 are escaped-and-recorded (below) because they are **interlocked and
one-session-infeasible gate-green**, and the disciplined path (plan §"escape
protocol": leave old code, gate green, record, keep going — never improvise a new
design) forbids a rushed stopgap. Disclosed in full so R2b can execute faster.

**Landed — item 7, the M3b PC-injection seam (commit `2ed12d3`).** The
`MULTITHREADING_PLAN.md` M3b seam the plan wants cut "in R2 so signatures churn
once." Done exactly as specified:
- `CktElement::inj_currents(&mut self, sys, ctx: &mut InjCtx)` →
  `compute_inj_currents(&mut self, sys, node_v, ctx: &mut InjComputeCtx) -> bool`.
  The element fills its own element-owned `cd.inj_current` buffer and **returns**
  the `SystemYChanged` flag; it performs no shared `Currents` write.
- `InjCtx` (node_v + currents + system_y_changed + errors + solution_abort) →
  `InjComputeCtx` (errors + solution_abort only). `node_v` is now a read-only
  parameter; `currents` and `system_y_changed` are gone from the per-element ctx.
- The **caller** (`solution/solution/power_flow.rs` `get_source_inj_currents`
  and `get_pc_inj_curr_filtered`) scatters `cd.inj_current` into `sol.currents`
  through `NodeRef` and ORs the returned flag into `sol.system_y_changed`,
  **sequentially in `sources`/`pc_elements` order** — identical element order,
  identical per-conductor `0..yorder` order, so the FP sums are bit-identical to
  the old fused `InjCurrents` (forbidden-move #3 respected: no accumulation
  reorder). All 14 PC/source impls converted (load, storage, generator, pvsystem,
  windgen, ind_mach012, vsource, isource, vs_converter, vccs, upfc, gic_line,
  gic_source; isource's private `compute_inj_currents(sys)->Vec` renamed
  `base_inj_currents` to free the trait-method name).
- **Disclosed deviation from the literal plan signature.** The plan sketched
  `compute_inj_currents(&mut self, sys, node_v)` with no ctx. The solve-time fault
  channels (`errors`/`solution_abort`, used by generator/storage/pvsystem for a
  user-model trap / missing-dynamics-model abort — WASM_USERMODELS §2.9-5) have no
  other home, so they stay in the reduced `InjComputeCtx`. Only `currents` +
  `system_y_changed` were removed (exactly what the plan asked). `errors`/`abort`
  remain a shared `&mut` for now; their per-element collection is M3b's own later
  step (noted in the `InjComputeCtx` doc). Behavior identical.
- Gate: fmt clean, clippy `-D warnings` clean, `cargo test --workspace` exit 0 —
  **corpus gate green (25 tests, `corpus_gate_all_cases_match_engines` ok,
  142 s)**, both channels (capi_v0145 + r4133), zero golden/tolerance/ledger churn.

**Escaped — items 1, 2, 4, 5, 6 (recorded per escape protocol; NOT started, old
code untouched, gate stays green).** Root cause of the escape: R2's items form one
tightly-coupled block, not independent clusters.
- **Item 1 (flip `ElemRef → ElemId`)** is the spine: `rg ElemRef crates/dss-core/src`
  = **927 hits across 130 files** (measured this session). `ElemRef {cls,idx}` and
  `ElemId` are *different types* — a half-flipped tree does not compile (a
  `Vec<ElemId>` cannot pass where `Vec<ElemRef>` is expected), so it cannot be
  landed one-cluster-at-a-time gate-green in a single session; it needs its own
  multi-session WP with `From`/`Into` bridging or a big-bang flip. R1 already
  provides the machinery (`ElemId::to_ref`, the bridge round-trip test); what is
  missing and must be added first is an `ElemId::from_ref(ElemRef) -> ElemId` (a
  match over all 50 class ordinals) for every runtime-class construction site.
- **Items 2/4/5/6 depend on item 1's typed store.** The concrete-borrow sites
  (`controls/dispatch.rs` reg→transformer / cap→capacitor, the `generator_mut`/
  `storage_mut`/`pvsystem_mut`/`espvl_mut`/`upfc_mut` helpers, meter reads, GET
  paths) all reach elements through `store: &mut dyn ElemStore`, whose only typed
  path today is `obj_mut(r).as_any_mut().downcast_mut::<T>()`. Getting a typed
  `&mut RegControl`/`&mut Capacitor` **without** a downcast requires the caller to
  hold a *concrete-typed* store (the `Elements` aggregate or `&mut ClassStore`)
  and match on `ElemId`/`ClassArena` variants — which is item 1's store-retyping.
  The only way to do items 2/4/5/6 *before* item 1 is to bolt ~10 concrete-typed
  methods (`reg_transformer_pair`, `cap_capacitor_pair`, `generator_mut`, …) onto
  the generic `ElemStore` trait — an improvised stopgap the escape protocol
  forbids (it couples the storage-agnostic trait to specific element classes and
  would be torn out again by item 1). So they wait for item 1.
- **Item 6 (remove `as_any`/`as_any_mut`/`as_ckt_element`/`as_ckt_element_mut`
  from `DssObject`)** is all-or-nothing: the trait method cannot be removed while
  any caller remains. Current populations (measured this session, unchanged by the
  item-7 landing): `rg "as_any|as_ckt_element" crates/dss-core/src` = **759 hits /
  134 files** (≈170 are the per-class boilerplate impls, the rest call sites);
  `rg "downcast_ref|downcast_mut" crates/dss-core/src` = **416 hits / 100 files**.
  Removing `as_ckt_element` alone is tractable-but-large (make the arena macro tag
  ckt vs data classes so `ClassArena::ckt_elem` upcasts `&v[idx] as &dyn
  CktElement` directly, then convert the ~50 external `.as_ckt_element()` call
  sites) and is a good standalone R2b sub-WP; removing `as_any` is the full
  downcast-elimination job (all 416 sites) and belongs to the item-1 flip.
- **Category E `make_like`** was scoped and is *ready to execute* but not started:
  it is 50 per-file structural extractions (`fn make_like(&mut self, other: &dyn
  DssObject)` inside `impl DssObject for X`, with a leading `other.as_any().
  downcast_ref::<X>()` guard, → an inherent `pub(crate) fn make_like(&mut self,
  other: &Self)` in an `impl X` block; production dispatch is the single site
  `ClassArena::make_like_within`, which would clone the typed source and call the
  inherent method — every element type is already `Clone`, its `clone_box` is
  `Box::new(self.clone())`). Low numerical risk, zero corpus impact, but ~50 files
  of body-moving edits; deferred to R2b as its own commit. NOTE: `clone_box` is
  **not** dead (item 8 candidate withdrawn) — beyond `make_like_within` it backs
  the `line_geometry` conductor-snapshot storage (`fwiredata`/`line_spacing_obj`
  clone `Box<dyn DssObject>`) and the `dispatch.rs` self-monitoring `mon_clone`
  (3 sites); keep it.

**R3 candidates flagged (item 8; NOT deleted).** `clone_box` — withdrawn, still
live (see above). The others named in the brief (`schema_skeleton`/
`extract_schema_skeleton_json`, `CktElement::recalc_element_data` in line/solve.rs,
`ClassStore` adapter, `ControlKind` remnants) were not re-examined this session and
remain for R3.

**R2b sub-plan (recommended split for the follow-up).** (a) `ElemId::from_ref` +
flip `ElemStore`/`circuit.rs` lists/`RefAction`/cross-refs `ElemRef → ElemId`
(item 1, its own session; typed `Idx<T>` where the class is statically known —
Load `daily: Option<Idx<LoadShapeObj>>` etc.); (b) on the now-typed store,
Categories A/B/D/E + the `generator_mut`-family helpers → typed arena matches;
(c) Category E `make_like` (50-file inherent-method extraction, standalone commit);
(d) `as_ckt_element` removal (arena macro ckt/data tag) as a separable sub-WP;
(e) finally remove `as_any` once (b)+(c)+(d) zero the 416 downcast sites.

**Settler pass (two fresh audits: code + tests, both PASS/ACCEPT of the delivered
item 7; every finding settled empirically).** Ritual held (186 `.pas`, cargo =
`.cargo\bin`). Dispositions:
- *Escaped-site integrity (items 1–6) — CONFIRMED intact.* HEAD grep metrics are
  byte-identical to base `1f0b768`: `ElemRef` 927/130, `as_any|as_ckt_element`
  759/134, `downcast_ref|downcast_mut` 416/100. The whole diff `base..HEAD` touches
  only the 17 seam files + STATUS — no escaped path edited. Blocker records verified:
  the 5 `*_mut(store: &mut dyn ElemStore) -> &mut T` helpers, dispatch.rs
  reg→transformer (~955, via `ControlledTransformer`) and cap→capacitor (~1012–1077)
  downcast sites (dispatch.rs downcast count 113 == base), 50 `make_like(&dyn
  DssObject)` impls — all present. Records accurate.
- *Item-7 seam bit-neutrality — CONFIRMED by inspection + oracle.* The scatter loop
  moved verbatim to the caller (`for i in 0..cd.yorder { sol.currents[cd.node_ref[i]]
  += cd.inj_current[i] }`) in identical `sources`/`pc_elements` element order and
  identical `0..yorder` bound → FP sums bit-identical. `system_y_changed`: the ONLY
  injection-side raiser at base was `storage/accessors.rs` (guarded by
  `yprim_invalid`); it now returns the flag, caller ORs it — all other PC elements
  return `false` (matches base; the other `*ctx.system_y_changed=true` hits are
  control/PD ctx, unrelated, unchanged). Both gating oracles agree (below).
- *Audit-code Question (caller-scatter has no dedicated unit test) — DISMISSED,
  non-defect.* Empirical probe: a +1% multiplicative perturbation of the caller
  scatter breaks **54 dss-core lib oracle tests** (energymeter, dynamics, monitors,
  solve, storage, upfc, vs_converter, vccs, force_hooks, espvl, …). Probe reverted,
  no residue. The path is heavily guarded; a dedicated unit test is unnecessary.
- *Audit-code/tests Note (`clone_box` withdrawn from the R3-dead list) — CONFIRMED
  correct.* Live at 17 call sites / 9 files (line_geometry & line conductor
  snapshots, `arena.rs::make_like_within`, dispatch `mon_clone` ×3). Not dead.
- *Audit-code Minor / audit disclosure (scope 7/8, items 1–6 escaped) — NOTED,
  deferred to R2b (a→e above).* Not a code defect; a planning matter, fully recorded.
- *Audit-tests Note (corpus_gate is not concurrency-safe inside a shared worktree —
  two parallel gate runs race shared EnergyMeter DI / debugtrace output dirs → os
  error 3 / "Error 303 … being used by another process") — RECORDED as
  test-infra fragility, out of R2 scope.* Empirically confirmed a contention
  artifact, not a seam defect: solo `corpus_gate` = **25 passed / 0 failed, 135 s,
  both channels (capi_v0145 + r4133)**; the injection split cannot affect external
  file opens. Flagged for a future test-infra WP (serialize corpus runtime output
  per worktree, or a per-run scratch dir).
- *Final gate (settler, solo — toolchain guard first):* `cargo fmt --all --check`
  ok; `cargo clippy --workspace --all-targets -- -D warnings` ok; `cargo test
  --workspace` = **0 failed** across 66 result groups (no panics/compile errors).
  Zero golden/tolerance/ledger/`TODO(compat)` churn. Corpus tree pristine
  (untracked run-artifacts removed by exact name). Tree clean.

### DE_PASCALIZE R1 — typed arenas: `Idx<T>`/`ElemId`/`Elements` (wave 2, branch `depas-r1`)

Stratum **[A]** bit-neutral. Part I R1 — introduce the `PORTING_PLAN §2.1`
typed-arena storage behind the current API, plus the P7 Send rider. Lands as the
plan-mandated **two commits**.

**Commit 1 — arena types (this commit).** New `obj/arena.rs` drives *everything*
from ONE class list (`with_all_classes!`), emitted in **`exec/construct.rs`
registration order** (recounted at execution — **50 classes**, 15 DSS_OBJECT +
35 circuit; the plan's 2026-07-12 count of 50 holds). One consumer macro
(`define_arena!`) expands the list into `enum ElemId` (one `Idx<T>` variant per
class — the R2 typed handle), `enum ClassArena` (one `Vec<T>` per class), and
every match-arm impl (`obj`/`obj_mut`/`ckt_elem`/`ckt_elem_mut`/`len`/
`class_name`/`push_new`/`pair_mut_same`/`triple_mut_same`/`for_each_ckt_elem_mut`)
plus `ElemId::CLASS_NAMES`. `Elements { arenas: Vec<ClassArena> }` owns them.
- **Design choice — `Vec<ClassArena>` (enum-of-`Vec<T>`), not the sketch's flat
  `struct { lines: Vec<Line>, … }` of named fields.** The property-edit path
  borrows the *active* class mutably while every other class is a read view
  (`edit_active_inner`, `classes.split_at_mut(ci)`), and `ci` is only known at
  run time — a named-field struct cannot be split by a run-time field, but a
  `Vec<ClassArena>` splits exactly like the existing `Vec<DssClass>`. This keeps
  the whole borrow-split machinery intact for the R1 ownership flip **with no
  `RefCell`/`unsafe`** and minimal churn. The typed `Vec<T>` is still exposed
  per class (`ClassArena::Line(v) => v.par_iter_mut()`) via `arenas_mut()` /
  `for_each_ckt_elem_mut` — the M3 parallelism substrate, never a single
  `&mut dyn ElemStore` funnel (Part V rider).
- **Mandatory ordering test** (`obj::arena::tests::arena_order_matches_registry`):
  builds a live `Dss`, asserts `ElemId::CLASS_NAMES[i]`, the **live**
  `DssClass::arena` variant `[i]` (`Dss::live_arena_class_names`), **and** the
  standalone `Elements` `ClassArena` layout `[i]` all match the registry class
  name at `i` for all 50 — the load-bearing registration-order == arena-order
  invariant, checked on the path that actually runs (settle: audit-tests C).
  Plus an `ElemId`↔`ElemRef` bridge round-trip test. (The R2 same-named-cross-class
  `find_ckt_element` tie-break test lands with the ownership flip, where the
  arenas are actually wired.)
- **P7 Send rider (rides with R1, per Part V item 1 / plan R1 step 3):** `: Send`
  supertraits on `DssObject`/`CktElement`/`ElemStore`; `const _:() =
  assert_send::<Dss>()` **and** `assert_send::<Elements>()` in `lib.rs`. The only
  non-`Send` blocker was `plot_callback: Box<dyn FnMut(&str)->i32>` (post-dates
  the 2026-07-06 P7 audit) → `+ Send` on `PlotCallback` and
  `register_plot_callback`. Its two test capture sinks (`exec/plot/tests.rs`,
  `tests/golden_plot_callback.rs`) moved `Rc<RefCell>` → `Arc<Mutex>` (test-only;
  removes the `Rc`/`RefCell` the P7 grep gate targets, `Mutex` is outside that
  pattern — flagged for audit).
- Arena types carry `#![allow(dead_code)]` until commit 2 wires them into `Dss`.
- Gate (commit 1): fmt clean, clippy `-D warnings` clean, `cargo test
  --workspace` exit 0 (corpus gate included; zero golden/tolerance churn — the
  arithmetic is untouched, so the byte goldens are the equivalence proof).

**Commit 2 — ownership flip (this commit).** The pre-R1
`DssClass.objects: Vec<Box<dyn DssObject>>` is **gone**; each `DssClass` now owns
its objects in a typed `ClassArena` (`DssClass::arena`, one `Vec<T>` per class),
built at registration from `props.class_name()` (`ClassArena::empty_for`). All
~330 object-access sites across 54 files moved `class.objects[i]` →
`class.arena[i]` (a `ClassArena: Index/IndexMut<usize, Output = dyn DssObject>`
makes it a near-rename; iteration → `arena.objs()`/`objs_mut()`,
`arena.get()`/`len()`; `make_like` → `arena.make_like_within`). `ClassStore`,
`ForeignClasses`, and the `edit_active_inner` `split_at_mut(ci)` borrow split are
**unchanged in shape** (they reach objects through `class.arena`). Downcasts and
`as_any` stay (R2 removes them). New pinning test:
`exec::registry::tests::find_ckt_element_tie_breaks_by_registration_order` (the
Risks-section tie-break — a bare name shared by Line+Load resolves to Line, the
first-registered class).
- **Deviation from the literal brief (disclosed, feasibility-driven).** The brief
  said "move ownership into a separate `Elements`; `DssClass` keeps metadata
  only." Executed instead as **`ClassArena` per `DssClass`** (still R1's core: the
  `Vec<Box<dyn DssObject>>` boxing is eliminated, replaced by typed per-class
  `Vec<T>` arenas, par-iteration-ready, `as_any` retained). Rationale: the
  separate-`Elements` design forced threading a new `arenas: &[ClassArena]`
  parameter through ~40 functions taking `classes: &[DssClass]` (cim/report/exec)
  **and every caller** — ~150 extra signature/caller edits across 54 files,
  infeasible to land gate-green in one session and against the plan's
  "minimize churn / behind the same API" directive. Arena-in-`DssClass` keeps all
  those signatures and the `ClassStore`/`ForeignClasses` split unchanged. The
  hoisted-aggregate form is preserved as the `Elements` type (`Vec<ClassArena>` +
  whole-registry accessors, `assert_send::<Elements>()`, the ordering test); R2 —
  which retypes `ElemRef`→`ElemId` across the spine anyway — can adopt it then.
- `#![allow(dead_code)]` removed from `obj/arena.rs` (everything is wired or is
  pub R2/M3 scaffolding, exempt from the lint).
- Gate (commit 2): fmt clean, clippy `-D warnings` clean, `cargo test --workspace`
  exit 0 — **corpus gate green (25 tests, `corpus_gate_all_cases_match_engines`
  ok, 144 s)**: all manifest cases still match the pinned dss-python + r4133
  oracles, **zero golden/tolerance churn** — the arithmetic is untouched, so the
  byte goldens are the equivalence proof (bit-neutral confirmed).

**Settle (two independent audits, opus-xhigh).** Both verdicts: faithful,
bit-neutral mechanical refactor; the one deviation (arena-on-`DssClass` vs
standalone `Elements`) is disclosed and behavior-neutral. Three findings, all
Minor, dispositioned:
- **`Elements` production-dead + ordering test validated the dead aggregate**
  (audit-code C / audit-tests C). Empirically confirmed: `git grep` shows the
  `Elements` struct's only non-comment reference outside `arena.rs` is the
  `lib.rs` `assert_send::<Elements>()`, and `pair_mut_arenas`/`triple_mut_arenas`
  have zero callers outside `arena.rs`. The deviation itself is **kept** — it is
  the disclosed, feasibility-driven design (the standalone-storage hoist is R2's
  job, which retypes the spine anyway) and both audits accept it as behavior-
  neutral; deleting vs adopting `Elements` is R2's call. Both concrete gaps the
  audits flagged are **fixed** (test-only, stratum [A]): (1) the ordering test now
  also asserts the **live** `DssClass::arena` variant order (new `#[cfg(test)]
  Dss::live_arena_class_names`), so the load-bearing invariant is checked on the
  path that runs, not only the dead aggregate; (2) new
  `elements_disjoint_borrows_cover_all_branches` + `elements_pair_mut_rejects_aliasing`
  pin the `Elements::pair_mut`/`triple_mut` disjoint-borrow case-analysis across
  every class-aliasing branch (was zero coverage) — so the R2/M3 substrate is
  verified, not "correct by inspection".
- **`Arc<Mutex>` in two plot test sinks vs the "no Mutex" forbidden-move**
  (audit-code D / audit-tests B). **Not a real violation — no change.**
  Empirically: the P7 grep gate as actually enforced is `RefCell|Rc<|static
  mut|thread_local` (Part V rule #4 / §695 — `Mutex` is not in it) and it is
  **clean over all `crates/*/src` production source**. The two `Arc<Mutex>` sinks
  (`exec/plot/tests.rs`, `tests/golden_plot_callback.rs`) are test-only capture
  buffers, **forced** by the sanctioned P7 `+ Send` rider on `PlotCallback`
  (`Rc<RefCell>` is not `Send`), add no ambient engine state, and leave every
  assertion byte-identical. The forbidden-move list names `Mutex` to keep it out
  of the **shipped engine**; a `Send` test sink is the idiomatic capture and does
  not touch that guarantee.
