# DE_PASCALIZE R2b + R3 — the typed-store flip and the `Any` removal

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### DE_PASCALIZE R3 — SETTLER PASS (two audits: code = 1 low + 5 notes, tests = PASS; every finding settled empirically; `DssObject::as_conductor` removed as the last type probe; final gate green) (branch `depas-r3`, 2026-07-26)

Stratum **[A]** bit-neutral. Base of the wave = `update` @ `67d2965`; settler base
= the R3.4 tip `392c7c0`. Ritual 0 held at start **and** before each commit: 186
`.pas` under `.inputs/dss_capi` (PowerShell recursion — never a delete through the
junction); `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**Audit verdicts.** `audit-code` = FINDINGS (1 **low**, 5 notes — the low one
explicitly "impact: none in practice"); `audit-tests` = **PASS** (3 low-severity
strengthening suggestions, no defect). No finding claimed a behavior regression,
and none was dismissed by argument: each was reproduced or refuted with an
experiment, and the two that motivated a *test* were both proven to go red under
a deliberate mutation before the mutation was reverted.

**Disposition of every finding (settled empirically, not by argument):**

| # | finding | settler disposition (experiment) |
|---|---|---|
| C-1 (low) | conductor NIL fallback: on `ConductorObj::from_resolved == None` the new `set_wires` (`line/code.rs:274`, `line_geometry/edit.rs:143`) `continue`s, skipping the `norm_amps`/`emerg_amps` writes the old `clone_box()` path did | **CONFIRMED unreachable, now PINNED rather than argued.** Re-derived the reachability myself: both `ObjectRefArray` paths in `class_props/parse.rs` resolve only via `foreign.find(<declared class>, …)` (l.272 fixed-class, l.696 proxy after the `object_classes` membership test → #10103), and `ForeignClasses::lookup` scans for the class name and returns only that class's arena — so a foreign object cannot reach a conductor slot. The only other `set_object_ref_array` callers are the two whitebox tests. **FIX: a coupling guard test** (below) replaces the reading argument; code left as-is (restoring the old writes would add dead code on a proven-unreachable branch). |
| C-2 (note) | `schema_skeleton`/`extract_schema_skeleton_json` + `NewObjectFn`/`DssClass::new_object` are `pub` removals (semver-visible) | **ACKNOWLEDGED, no change.** Re-verified zero callers at the base (`git grep` over `67d2965`) and zero today; the plan's dead-code sweep sanctions it. Recorded here so it lands in release notes if `dss-core` is consumed as a library. |
| C-3 (note) | `ClassArena::pair_ckt_mut` panics with `"triple_mut: object index out of range"` | **FIXED** → `"pair_ckt_mut: …"` (`obj/arena.rs:506`). Checked no `#[should_panic(expected …)]` reads that string; the two `traits.rs` messages (`split2` = `pair_mut`, `split3` = `triple_mut`) were already correct. |
| C-4 (note) | doc block describing `ckt_self_ref`/`ckt_self_mut` is attached to `macro_rules! clone_ckt_view` | **FIXED** — paragraph moved onto `ckt_self_ref`, `ckt_self_mut` given its "Mutable […]" one-liner; `clone_ckt_view` keeps its own paragraph only. |
| C-5 (note) | `DssObject::as_conductor` survives with a single consumer (`cim/export.rs::conductor_geom_amps`) — "the natural next Category-C cleanup" | **FIXED (scope call: it is Category C, which R3.4 owned).** Its three callers each walk their **own** `WireData`/`TSData`/`CNData` arena, so the helper became `conductor_geom_amps::<T: ArenaClass + ConductorData>(&ClassArena, oi)` = `arena.get::<T>(oi).map(…)` — same `Option` shape, same unreachable `None` arm, same `continue`. `DssObject::as_conductor` + its 3 impls removed. **The last runtime type probe in the tree is gone.** |
| C-6 (note) | verification summary: all binding invariants green | **CONFIRMED independently** — re-derived table below. |
| T-1/2/3/7/8 (notes) | scope/bit-neutrality, gate reproduction, the two re-oracled renames, test-context restructuring, dead-code sweep + escape records | **CONFIRMED**, re-measured below; nothing to change. |
| T-4 (low) | `typed_accessors_match_the_any_downcast_for_every_class` pushes ONE object per class, so every index is 0 — a class-generic `get(0)`-instead-of-`get(idx)` slip would pass | **FIXED + proven.** The macro now pushes **two** objects per class and address-checks `get`/`get_mut`/`all()[i]` against `arena.obj(i)` at **both** indices, asserts the two slots are distinct, moves the out-of-range probe to index 2, and round-trips `id`/`idx_of`/`try_ckt_elem*`/`clone_ckt` at index 1. **Mutation probe:** patching `ClassArena::get` to `T::arena_vec(self)?.get(0)` turns the test **red** (it was green under that mutation before); reverted. |
| T-5 (low) | the `monte_carlo` `FaultStore` double's `arena(_cls)`/`arena_mut(_cls)` ignore the class ordinal, so it cannot catch wrong-ordinal routing in `TypedStore` | **FIXED.** `obj`/`obj_mut`/`arena`/`arena_mut` now `assert_fault_ord(...)` against `<Fault as ArenaClass>::CLASS_ORD`, so a foreign ordinal panics instead of being silently answered with the Fault arena. Tests still green (the production path really does address it with the Fault ordinal). |
| T-6 (low) | nothing pins the `ConductorObj` ⇄ `object_classes` coupling: add a 4th conductor class and the slot silently goes NIL | **FIXED + proven** (this is also C-1's pin). New `conductor_data::tests::conductor_property_classes_match_the_conductor_obj_variants`: asserts `CONDUCTOR_PROXY_CLASSES == [WireData, CNData, TSData]`; walks all **11** conductor-slot properties (Line `Wires`/`CNCables`/`TSCables`/`Conductors`, LineGeometry `Wire`/`Wires`/`CNCable`/`CNCables`/`TSCable`/`TSCables`/`Conductors`) and requires every declared `object_class`/`object_class2`/`object_classes` entry to be a `ConductorObj` variant, that all three are reached, that each really narrows to its own arm via a live `ResolvedObj`, and that a `LineSpacing` object does **not**. **Mutation probe:** retargeting Line `Wires` to `LineSpacing` turns it **red**; reverted. |

**Final grep metrics (settler-measured, `rg -c … crates/dss-core/src`, summed lines / files):**

| metric | base `67d2965` | R3 HEAD | delta | note |
|---|---|---|---|---|
| `ElemRef` | 937 / 130 f | **0** | −937 | type gone (R3.1) |
| `downcast_ref\|downcast_mut` | 369 / 73 f | **0** | −369 | zero **code and prose** |
| `as_any` | 463 / 107 f | **0** | −463 | zero code and prose |
| `as_ckt_element` | 253 / 83 f | **5 / 1 f** | −248 | all 5 are doc prose in `obj/arena.rs` |
| `as_conductor` | 14 / 10 f | **2 / 2 f** | −12 | both doc prose; `fn as_conductor` defs 4 → **0** (this pass) |
| `clone_box` | 69 / 58 f | **2 / 2 f** | −67 | both doc prose |
| `Box<dyn DssObject>` | — | **7 / 5 f** | — | all 7 doc prose; owned storage = **0** |
| `std::any` | — | **0** | — | no `Any` anywhere |
| `fn recalc_element_data` | 43 | **1** | −42 | the live 2-arg WindGen inherent |
| `TODO(compat)` | 117 / 68 f | **117 / 68 f** | **0** | invariant held |
| `#[cfg(feature = "oracle-parity")]` | 0 | **0** | 0 | Stage F's job, untouched |

Scope re-derived independently: the union of `git diff --name-only 67d2965` and the
settler working set touches **nothing** outside `crates/dss-core/src/**` except
`STATUS.md` — zero churn in `tests/golden`, `tests/corpus` (decks, manifests,
`ledger.json`, `population.lock`), `tools/golden`, `tests/TOLERANCE_NOTES.md`,
`crates/dss-core/tests/`, `crates/dss-epri`, `crates/dss-sparse`. All 5 `#[ignore]`s
are pre-existing, each with a justification string, in files the wave never touched.
Pin tests `arena_order_matches_registry` and
`find_ckt_element_tie_breaks_by_registration_order` present and green.

**Bit-neutrality of the settler diff.** Three of the six changes are test-only
(`arena.rs` test macro, `monte_carlo` fixture asserts, the new conductor guard
test); two are comment/panic-string only (C-3, C-4 — the string is observable
solely through a panic on a proven-unreachable index). The one production change
(C-5) swaps a vtable probe for a compile-time match over the same three classes
and keeps the helper's `Option` shape and its callers' `continue` verbatim; the
values read (`geom()`, `amps().0`) come from the same concrete `impl ConductorData`
the vtable dispatched to. No list, loop, `find_*` tie-break, control-queue
insertion, stamp or accumulation order is touched; no arithmetic is in the diff.

**Gate (settler, FULL, SOLO, toolchain guard first — `cargo` = `.cargo\bin`).**
`cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D
warnings` exit 0; `cargo test --workspace` **exit 0** — **66 `test result: ok`
groups, 1990 passed, 0 failed, 5 ignored**, `corpus_gate_all_cases_match_engines
… ok` (25 passed, 134.7 s, both channels capi_v0145 + r4133), run solo, no name
filters. The +1 over R3.4's 1989 is exactly the new conductor coupling guard.
Corpus left pristine: the 10 run-artifacts listed by `git status tests/corpus`
(`AutoTrans/auto3bus_load_power.txt` + 9 `StorageControllerTechNote/Support/
IEEE8500u_*.csv`) removed by exact name — no wide `git clean`. Tree clean.

**Deliberately NOT fixed (recorded, not dropped).**
1. **C-2's public-surface removals** stay removed — callerless at the base and
   sanctioned by the plan's dead-code sweep. Flagged for release notes only.
2. **R3.1 sub-step (iv) remainder** — `geometry_obj` / the `xfmr_code_ref` family
   as typed `Idx<T>`. Still open, still out of R3's scope (neither `Any`
   consumers nor `dyn`-owned storage); carried forward exactly as R3.4 recorded
   it. It is the one item the R3 wave hands to its successor.
3. **C-1's code shape** — see the disposition table: the divergence lives on a
   branch proven unreachable *and now pinned*, so restoring the old
   `norm_amps = 0.0` writes would add dead code, not neutrality.

### DE_PASCALIZE R3.4 — Category-C conductor snapshots retyped (`ConductorObj`); `clone_box` and the dead `CktElement::recalc_element_data` REMOVED; Part I success metrics all zero (branch `depas-r3`, 2026-07-26)

Stratum **[A]** bit-neutral, type-channel only. Base = the R3.3 tip `dd06c54`.
Ritual 0 held at start **and** before the commit: 186 `.pas` under
`.inputs/dss_capi`; `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**Delivered: R3 handoff item 4 (a) + the R3 dead-code sweep (b) + the metric
verification (c) + the perf sanity check (d).** The last owned `Box<dyn
DssObject>` storage in the tree — the conductor snapshots — is now a typed enum,
which zeroes `clone_box`'s call sites and lets the trait method go with them.

**(a) The typed conductor snapshot** (`elements/general/conductor_data/mod.rs`):

```rust
pub enum ConductorObj { Wire(WireDataObj), Cn(CnDataObj), Ts(TsDataObj) }
//   ::from_resolved(ResolvedObj) -> Option<Self>   (the resolve-time clone)
//   .name()  .amps_owned()  + impl ConductorData (geom/amps/conductor_kind)
```

`LineGeometryObj::fwiredata` and `Line::line_wire_data` become
`Vec<Option<ConductorObj>>`; `LineGeometryObj::conductor(i)` and
`load_spacing_and_wires(wires: &[Option<ConductorObj>])` follow. The three
variants are exactly the classes these properties resolve against
(`object_ref_class("WireData"|"CNData"|"TSData")` plus the 3-class `Conductors`
proxy — `props/class_props/parse.rs` resolves an `ObjectRef` only inside its
declared class), so the storage cannot hold anything else and every read that
used to probe `as_conductor()` at runtime is a static match now. Retyped
consumers: the 11 conductor-snapshot `clone_box` sites →
`ConductorObj::from_resolved` / `.clone()`; `get_string` /
`get_object_ref_names` / both `Save` conductor loops → `.name()`; the
choice/ratings helpers (`conductor_choice_of` ×2, `default_amps_from`,
`set_wires` ×2, `conductor_norm_emerg`) → total `match`es over `ConductorKind`
with no `Option` arm; `cim/export.rs::conductor_class_name` → `&ConductorObj ->
&'static str` (infallible). Three helpers died with their last caller and were
deleted: `conductor_data::conductor_geom`, `line/code.rs::conductor_amps`,
`line_geometry/edit.rs::conductor_amps`.

**`clone_box` is gone** — zero call sites after the retype (the dispatch
`mon_clone` ×3 had moved to `ClassArena::clone_ckt` in R3.3, `make_like_within`
to the typed clone in R2b (c)), so the trait method, its 50 per-class impls and
the `json_tests` test-double impl were removed. With `fwiredata` /
`line_wire_data` typed, `LineGeometryObj` and `Line` are plain
`#[derive(Clone)]` — both hand-written `Clone` impls were field-complete (a
struct literal cannot omit a field), so the derive is the same field-wise copy.
The hand-written `Debug` impls stay: they deliberately print a subset.

**(b) Dead-code sweep — two candidates deleted, three are live.**
- **`CktElement::recalc_element_data(&mut self, &SysCtx)` — DELETED** (trait
  decl + 35 class impls + 6 test-double impls). Provably dead: `rg
  recalc_element_data` finds **no call site** anywhere (the executive drives
  recalc from `end_edit` / the property side effects). Every impl body was a
  one-line delegate to an inherent `recalc(...)`, and each of those inherent
  methods keeps other live callers (checked per class: `accessors.rs::end_edit`,
  the `mod.rs` constructors, unit tests) — nothing was orphaned. The unrelated
  WindGen/WTG3 inherent `recalc_element_data(h, t)` (live from
  `windgen/nominal.rs`) is untouched.
- **`schema_skeleton` / `extract_schema_skeleton_json` — DELETED**
  (`report/export/json/schema/mod.rs`): zero callers in the workspace, superseded
  by `assemble_full_document`. Their two helpers (`global_defs`,
  `circuit_properties_head`) stay — used by the full document **and** pinned by
  `tests/golden_schema.rs`.
- **`ClassStore` — NOT dead, and no longer a boxing adapter.** R1 turned it into
  the typed-arena store view (`classes: &mut [DssClass]` + the typed
  pair/triple getters) with ~20 live construction sites. The plan's "boxing
  adapter" wording predates the arena flip; nothing to remove.
- **`ControlKind` (`controls/dispatch.rs:42`) — NOT dead.** It is the typed
  dispatch enum built from `ControlClass` with the captured per-class refs, and
  every variant is matched in that file. The R0 "collapse" it refers to already
  happened (the identification chain became the `ControlElem` trait).
- **Downcast-related `.expect()` assertions — none left.** After R3.3 the only
  `expect`s on this spine unwrap the *typed* accessors (`try_ckt_elem`,
  `typed_*_pair_mut`), each with a live invariant.

**(c) Part I success metrics — verified on HEAD.**

| metric (`crates/dss-core/src`) | base `dd06c54` | HEAD | note |
|---|---|---|---|
| `downcast_ref\|downcast_mut` | 11 / 3 f | **0** | the prose naming the removed API reworded to "the removed `Any` downcast" |
| `as_any` | 18 / 6 f | **0** | same |
| owned `Box<dyn DssObject>` storage | 2 fields | **0** | the 7 remaining `Box<dyn DssObject>` hits are historical prose |
| `clone_box` | 65 / 58 f | **0 code** (2 prose) | trait decl + 51 impls + 12 call sites gone |
| `fn recalc_element_data` | 43 / 42 f | **1** | the live WindGen inherent 2-arg method |
| `TODO(compat)` | 117 / 68 f | **117 / 68 f** | invariant held |

`rg "downcast_ref|downcast_mut|as_any" crates/dss-core/src` → **0**, the plan's
R3 CI grep gate. The only workspace hits left are two `Box<dyn Any>` **panic
payload** downcasts in `tests/corpus_gate/runner.rs` (`catch_unwind` plumbing,
not the element type channel). Pin tests green and untouched:
`obj::arena::tests::arena_order_matches_registry`,
`exec::registry::tests::find_ckt_element_tie_breaks_by_registration_order`,
`typed_accessors_match_the_any_downcast_for_every_class`,
`typed_accessors_reject_a_foreign_class`.

**Bit-neutrality argument.** The retype changes *where the class is known*, not
what is read: `ConductorObj::from_resolved` clones the same object at the same
resolve-time point `clone_box()` did (`ResolvedObj::cloned::<T>` is the proven
typed twin), and `ConductorData` through the enum returns the same
`geom()`/`amps()`/`conductor_kind()` the `as_conductor()` vtable returned for
that concrete type. Every multi-arm chain that changed shape
(`conductor_choice_of` ×2, both `Save` kind selectors, `conductor_class_name`)
covers **mutually exclusive** classes and keeps its arm order and fall-through
target (`Wire → Overhead` / `"wire"` / `"Wires"`). Slot allocation, conductor
order, the skip-NIL compaction in `LoadSpacingAndWires`, the running-minimum
ampacity loop and every accumulation are verbatim; no arithmetic is in the diff.
`recalc_element_data` / `clone_box` / `schema_skeleton` had no callers, so their
removal cannot change behavior. Zero golden / ledger / tolerance / corpus-deck
churn.

**(d) Perf sanity (criterion, `crates/dss-core/benches`, release bench profile,
median).** One run on this HEAD, next to the only prior recorded numbers (the
P15 record's "after" column). Different session and machine load — an
order-of-magnitude sanity check, not a controlled A/B:

| bench | P15 record ("after") | R3.4 HEAD (median [lo hi]) |
|---|---|---|
| `snapshot_8500/compile_solve` | 186 ms | **132.65 ms** [132.10, 133.20] |
| `ybuild_8500/rebuild_whole_y` | 4.33 ms | **1.657 ms** [1.6473, 1.6673] |
| `lu_factor_solve/zero_restamp_factor_solve` | 4.41 ms | **2.639 ms** [2.6204, 2.6659] |

No `snapshot_8500`-class regression: the R3 type-channel work is compile-time
only (static matches replacing vtable/`Any` probes), so a regression was not
expected and none is visible above the run-to-run noise.

**Deviations disclosed.**
1. `ConductorObj::from_resolved` returns `Option`; a reference naming a
   non-conductor class yields `None` where `clone_box()` would have stored a
   foreign object. Unreachable (the property engine resolves an `ObjectRef` only
   within its declared class(es)), and every site treats that `None` as the NIL
   slot it already had — `line/code.rs::set_wires` writes `None` explicitly so no
   stale slot can survive. *(The unreachability is no longer only an argument:
   the R3 SETTLER record above adds
   `conductor_property_classes_match_the_conductor_obj_variants`, which pins the
   declared class list of all 11 conductor properties to the three
   `ConductorObj` variants.)*
2. `cim/export.rs::conductor_class_name` lost its `Option` (a conductor snapshot
   is always one of the three classes); its single caller used to drop the whole
   `ConductorRef` on `None` — now unreachable, same output.
3. `LineGeometryObj`/`Line` moved from hand-written to derived `Clone` (identical
   field for field, see above).
4. `line_geometry/accessors.rs::set_object_ref` no longer clones the resolved
   object *before* the property match — the clone happens inside the conductor
   arm only. The `spacing=` arm never used that clone (it takes its own typed
   `LineSpacingObj` clone), so this drops a wasted allocation, nothing
   observable.
5. The three deleted helpers and the two deleted schema functions had no callers
   and no test referenced them.
6. Ritual step 3 (two fresh audits) is not run in this session; the coordinator
   spawns the R3 audits.

**Still open → NOT R3.4's scope.** R3.1's sub-step (iv) beyond
`line_spacing_obj`: the remaining "statically-known shape refs as typed `Idx<T>`"
fields (`geometry_obj` / the `xfmr_code_ref` family). They are neither `Any`
consumers nor `dyn`-owned storage (already concrete snapshots or `ElemId`), so
they are a separate [A] refactor, not part of the Category-C storage retype this
step closed.

**Gate (full, SOLO, toolchain guard first).** `cargo fmt --all --check` exit 0;
`cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test
--workspace` **exit 0** — 66 `test result: ok` groups, **1989 passed, 0 failed,
5 ignored** (the same 5 pre-existing ones; zero `#[ignore]` churn),
`corpus_gate_all_cases_match_engines … ok` (both channels capi_v0145 + r4133),
run solo with no name filters. Corpus left pristine: the 26 run-artifacts listed
by `git status tests/corpus` removed by exact name — no wide `git clean`. Diff:
81 files, **+205 / −666**.

### DE_PASCALIZE R3.3 — the mechanical collapse: `as_any`/`as_any_mut` **and** `as_ckt_element`/`as_ckt_element_mut` REMOVED from `DssObject`; zero `Any` in the tree (branch `depas-r3`, 2026-07-26)

Stratum **[A]** bit-neutral, type-channel only. Base = the R3.2 part-3 tip
`a05a4a2`. Ritual 0 held at start **and** before the commit: 186 `.pas` under
`.inputs/dss_capi`; `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**Delivered: R3 handoff item 3 in full.** The 261 remaining `downcast_*` sites
(the R2b (e) blocker map's 214 typed-arena reads / 28 Category-A disjoint
borrows / 119 Category-B/D bare-`&dyn` params, as they stood after R3.1/R3.2)
are converted onto the R3.2 typed accessors, the ~200 `as_ckt_element*` call
sites onto `ClassArena::try_ckt_elem{,_mut}`, and **all four trait methods plus
their 50-class boilerplate are gone**. `std::any` no longer appears anywhere in
the product path: the only way from a handle to a concrete type is now a static
[`ArenaClass`] match.

**The four conversion shapes (all mechanical, receiver-preserving).**
- `store.obj(r).as_any().downcast_ref::<T>()` → `store.typed::<T>(r)` and the
  `_mut` twin — 50 sites (`controls/dispatch.rs`, `exec/command.rs`,
  `solution/{ncim,monte_carlo,faults,fault_study,power_flow,time_series,topology}.rs`,
  `circuit/auto_add.rs`, `exec/set_cmd.rs`).
- `classes[r.class_ord()].arena[r.index()].as_any().downcast_ref::<T>()` →
  `…arena.get::<T>(r.index())` — 79 sites across `cim/`, `exec/`, `report/`,
  including the two-step `let obj = &…arena[i]; obj.as_any()…` form.
- the `InvDispEnv` DER cluster (`controls/dispatch.rs`, 56 sites): the shared
  `let obj = self.store.obj(_mut)(r)` binding + PVSystem/Storage probe chain →
  two direct `self.store.typed{,_mut}::<T>(r)` probes. The `if let … else if let`
  double mutable borrow borrow-checks under NLL (the `Option<&mut T>` scrutinee
  temporary needs no drop, so the region is dead in the `else` arm); verified by
  compiling, not assumed.
- `obj.as_ckt_element{,_mut}()` → `arena.try_ckt_elem{,_mut}(idx)` — 91 sites.
  `objs()`/`objs_mut()` scans that needed the circuit view became `0..len()`
  index loops over the same arena, same order.

**Six bare-`&dyn` readers were retyped onto `(arena, idx)`** — the family the
R2b map listed as Category B/D, each with its callers:
`recalc_pc_create`, `report::save::dump::{dump_object, dump_generic}`,
`report::save::dump::overrides::dump_override` (its 16-way override chain is now
16 `arena.get::<T>(idx)` arms, same order), `report::save::save::write_dss_object`
(+ `class_file_text`'s `objs_mut()` loop → an index loop),
`exec::reduce::control_data_mut` (the 11-class `try_ccd!` macro keeps its
"probe shared, then borrow mutably" two-step verbatim), and
`cim::export::conductor_geom_amps`/`conductor_class_name` — the last two went to
the **existing** `DssObject::as_conductor()` + `ConductorKind` behavior trait
(R0 Category C) rather than to the arena, because their conductor operands are
snapshot clones held inside `Line`/`LineGeometry`, not arena residents.
*(Superseded for `conductor_geom_amps` by the R3 SETTLER record above: its three
callers walk their own `WireData`/`TSData`/`CNData` arenas after all, so it is a
typed arena read now and `DssObject::as_conductor` is removed. `ConductorKind`
via `ConductorObj` stands — it is a behavior trait, not a type probe.)*

**Item (e)'s escape is closed — `Monitor::take_sample` (`elements/traits.rs`).**
The four concrete reads (mode 9 `Capacitor::states`, mode 11 `Storage` present
kW/kvar/kWh/state, mode 8 `Transformer::get_all_winding_currents`, mode 10
`Transformer::get_winding_voltages`) plus the 8 `as_ckt_element*` reads now go
through a typed view built by the caller:

```rust
pub enum MeteredElem<'a> {                 // .ckt() .ckt_mut()
    Capacitor(&'a mut Capacitor),          // .capacitor() .storage()
    Storage(&'a mut Storage),              // .transformer() .transformer_mut()
    Transformer(&'a mut Transformer),
    Other(&'a mut dyn CktElement),
}
fn TypedStore::typed_metered_pair_mut(c, t) -> (&mut Monitor, MeteredElem<'_>)
```

This is escape-route (2) from the R3.2 record **without** its blocker: the
same-class branch (a Monitor whose `element=` names another Monitor) is served by
the existing same-arena `split2` and simply yields `MeteredElem::Other` — which is
exactly what the removed `downcast_ref::<Capacitor/Storage/Transformer>()` chain
returned `None` for on that branch. No behavior changes on any branch, probed or
not. `typed_obj_pair_mut` (landed by R3.2 part 3 for this one caller) has no
remaining user and was **retired**; its live-store proof case moved to
`typed_metered_pair_mut`, still asserting pointer identity with the untyped
`pair_mut` on both branches (cross-arena monitor⇄line, same-arena monitor⇄monitor).

**R3.1's escaped sub-step (iv) — one field converted, the rest re-scoped.** The
only `Box<dyn DssObject>` field the `as_any` removal actually forced was
`LineGeometryObj::line_spacing_obj`, now `Option<LineSpacingObj>` (its `clone_box`
clone became a plain `.clone()`, its `set_object_ref` a `ResolvedObj::cloned`, and
`apply_spacing`'s downcast disappears). The remaining (iv) fields
(`fwiredata`/`line_wire_data`/`geometry_obj`/`xfmr_code_ref`-family) are **not**
`as_any` consumers — they are read through behavior traits (`ConductorData`) or
as `ElemId` — so they are not R3.3's business and stay open (see below).

**Two bridges retired because they became unused.**
- `DssClass::new_object` / `type NewObjectFn` and the 50 `|name| Box::new(<T>::new(name))`
  closures in `exec/construct.rs`: after `recalc_pc_create` moved onto
  `(arena, idx)`, the sole caller (`exec/view.rs::schema_class_def`'s
  defaults sample) builds a throwaway one-object `ClassArena::empty_for(cls)` +
  `push_new(name)` instead. `push_new` expands to the very same
  `<$ty>::new(name)` the closure did, so the sampled object is identical.
- `elements::pd::transformer::as_controlled_transformer(&dyn DssObject)` →
  `ClassArena::try_controlled_transformer(idx)` (the shared twin of the existing
  `_mut`), 3 call sites.

**Bit-neutrality argument.** Every converted site resolves the same object by the
same `(class ordinal, index)`; `typed`/`typed_mut`/`ClassArena::get` are the
proven twins of the downcast (`typed_accessors_match_the_any_downcast_for_every_class`,
`typed_accessors_reject_a_foreign_class`), and `try_ckt_elem` is the proven twin
of `as_ckt_element` (the arena `ckt`/`data` tag). Every multi-class probe chain
that changed shape (`dump_override`, `write_dss_object`, `control_data_mut`,
`recalc_pc_create`, `conductor_class_name`) keeps its original arm order over
**mutually exclusive** classes, so the arm chosen is unchanged. No list, loop,
`find_*` tie-break, control-queue insertion, stamp or accumulation order was
touched; the `objs()` → `0..len()` rewrites iterate the same `Vec` in the same
direction. No arithmetic is in the diff; zero golden / ledger / tolerance /
corpus-deck churn.

**Proof surface.** Two tests were retargeted (never weakened) because their
oracle was the removed API, and one test double was strengthened:
- `obj::arena::tests::typed_accessors_match_the_any_downcast_for_every_class`
  compared `arena.get::<T>(0)` against `arena.obj(0).as_any().downcast_ref::<T>()`;
  it now asserts **pointer identity with the stored object itself**
  (`arena.obj(0) as *const dyn DssObject as *const ()`) — the same address the
  downcast handed back, for all 50 classes.
- `arena_tag_matches_trait_ckt_view` → `arena_tag_matches_registry_ckt_classes`:
  the `ckt`/`data` tag is now pinned against the **independent** registration
  column in `exec/construct.rs` (`ckt_class` ⇒ `kind.is_some()` vs `dss_object`
  ⇒ `None`, read through the new test-only `Dss::registered_class_is_ckt`), still
  asserting exactly 35 circuit-element classes. A `ckt` tag on a non-`CktElement`
  type cannot compile, so this covers the only silent direction left.
- `solution::solution::monte_carlo::tests`' `FaultStore` double was a flat
  `Vec<Fault>` addressed by `ElemId::new(0, idx)` — class ordinal **0** is
  `TCC_Curve`, which the old class-blind `obj_mut` happily ignored. It now holds a
  real `ClassArena::Fault` and hands out real `Fault` handles (`ArenaClass::id`),
  so `pick_a_fault` exercises the same typed path production does. Caught by the
  gate (2 red tests), fixed in the fixture, not in the code under test.
- `exec::registry::tests::typed_store_accessors_match_the_untyped_pair_and_downcast`
  keeps both `typed_metered_pair_mut` branches and its `store.obj(r)` identity
  assert. No case removed, weakened, or ignored; zero `#[ignore]` churn.

**Metrics (`rg -c … crates/dss-core/src`, summed lines / files).**

| metric | base `a05a4a2` | HEAD | delta |
|---|---|---|---|
| `downcast_ref\|downcast_mut` | 261 / 47 f | **11 / 3 f** | **−250** — every remaining hit is prose in a doc comment; **0 code** |
| `as_any` | 357 | **18 / 6 f** | **−339** — doc prose only; **0 code** |
| `as_ckt_element` | 212 | **5** | **−207** — doc prose only; **0 code** |
| `fn as_any` / `fn as_any_mut` defs | 52 / 52 | **0 / 0** | trait decl + 50 impls + the base helper gone |
| `fn as_ckt_element` / `_mut` defs | 35 / 35 | **0 / 0** | trait decls + 35 impls gone |
| `TODO(compat)` | 117 / 68 f | **117 / 68 f** | **0** — invariant held |
| `clone_box` | 65 / 58 f | 65 / 58 f | 0 — still live (`fwiredata`, `line_wire_data`, JSON), stays per brief |

Diff: 133 files, all under `crates/dss-core/src/`, **+1058 / −1832**.

**R3 total vs its base `update`@`67d2965`:** `ElemRef` **937 → 0**,
`downcast_ref|downcast_mut` **369 → 0 code**, `as_any|as_ckt_element`
**716 → 0 code**, `TODO(compat)` **117 unchanged**.

**Deviations disclosed.**
1. `ClassArena::get::<T>(idx)` / `TypedStore::typed::<T>(r)` return `None` for an
   out-of-range index where the old `arena[idx]` / `obj(r)` panicked first. Every
   converted site takes its index from a registry-produced `ElemId` or a
   `0..arena.len()` loop, so the state is unreachable; this is the same deviation
   already disclosed for R3.2 part 1. The two sites that *did* rely on the
   bounds-checked shape (`report/export/json/build.rs`'s first-element probe,
   `exec/make_pos_seq.rs::resolve_pos_seq_info`) kept it with an explicit
   `!arena.is_empty()` / `r.index() >= arena.len()` guard rather than by widening
   `try_ckt_elem`'s contract.
2. Six helpers changed signature from `&(mut) dyn DssObject` to `(arena, idx)`
   (listed above); all callers updated in the same diff, none is public API.
3. `DssClass::new_object`/`NewObjectFn` removed (50 construct.rs closures
   deleted). `ClassArena::push_new` is the identical constructor.
4. `first_enabled`/`last_enabled` (`ForeignClasses`) now return
   `Option<&dyn CktElement>` instead of `Option<&dyn DssObject>` — both callers
   immediately did `.as_ckt_element()` on the result, and the enabled test they
   already applied implies the ckt view exists.
5. One new `ClassArena::try_controlled_transformer` (shared twin of the existing
   `_mut`) and one new `TypedStore::typed_metered_pair_mut` + `MeteredElem`; one
   getter (`typed_obj_pair_mut`) retired.
6. `obj::arena::tests::from_ref_covers_every_class_and_round_trips` was renamed
   `elem_id_new_covers_every_class_and_round_trips` — its subject has been
   `ElemId::new` since R3.1 removed the `from_ref` bridge; the body is unchanged.
7. Doc comments that named the removed methods were reworded ("the removed
   `as_any` downcast"); the historical R0/R2b rationale lines are kept as history.
8. Ritual step 3 (two fresh audits) is not run in this session; the coordinator
   spawns the R3 audits.

**Still open → NOT R3.3's scope.** R3.1's sub-step (iv) beyond
`line_spacing_obj`: the remaining "statically-known shape refs as typed `Idx<T>`"
fields. They were re-scoped, not skipped — none of them was an `as_any` consumer
(they read through `ConductorData`/`ElemId`), so converting them is a separate
[A] refactor with its own bit-neutrality argument, not a prerequisite of the
trait removal this step landed.

**Gate (full, SOLO, toolchain guard first).** `cargo fmt --all --check` exit 0;
`cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test
--workspace` **exit 0** — 66 `test result: ok` groups, **1989 passed, 0 failed, 5 ignored** (the same 5 pre-existing ones), `corpus_gate_all_cases_match_engines … ok`
(both channels capi_v0145 + r4133), run solo with no name filters. Corpus left
pristine: run-artifacts removed by exact name (`git status tests/corpus`) — no
wide `git clean`. Tree clean.

### DE_PASCALIZE R3.2 (part 3/3) — sub-item (e): Category-B meter typed reads + disjoint borrows (12 converted files + 1 getter + 1 test); the escape narrows to the 4 `monitor/sample.rs` concrete reads (branch `depas-r3`, 2026-07-26)

Stratum **[A]** bit-neutral, type-channel only. Base = the R3.2 part-2 tip
`377bbc8`. Ritual 0 held at start **and** before the commit: 186 `.pas` under
`.inputs/dss_capi`; `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**Delivered: R3 handoff item 2's last sub-item (e)** — the Category-B meter
cluster (`DE_PASCALIZE_PLAN.md` Part I, category **B**: "meter type-guards &
concrete reads … typed arena reads"). Every meter-side `Any` round-trip is gone
except the four escaped ones below; nothing else in the cluster remains.

**One new getter, same family as R3.2 part 1** (`elements/traits.rs`):
`TypedStore::typed_obj_pair_mut::<C>(c, t) -> (&mut C, &mut dyn DssObject)` —
the meter as its concrete class, the metered element as the **generic** object
view. It mirrors `typed_ckt_pair_mut` branch for branch (same-arena `split2`,
cross-arena `arena_pair_mut`) and keeps the `"pair_mut: aliasing refs"` assert
verbatim; only the target view differs, because `Monitor::take_sample` still
reaches past `CktElement` (see the escape).

**Converted (12 files).**
- `solution/monitors.rs` — the `SampleAll`/`SaveAll`/`ResetAll` monitor reads →
  `typed`/`typed_mut`; the sample-time `pair_mut` + `downcast_mut::<Monitor>()`
  → one `typed_obj_pair_mut::<Monitor>`.
- `solution/meters/sampling/take_sample.rs` — the Generator/Storage/PVSystem
  `ResetRegistersAll` + `SampleAll` tails, the meter `enabled()` peek and the
  zone-walk Load/Generator accumulation → `typed`/`typed_mut`.
- `solution/meters/sampling/allocate.rs` — both `CalcAllocationFactors` pairs
  (`pair_mut` + downcast + `as_ckt_element_mut`) → `typed_ckt_pair_mut::<EnergyMeter>` /
  `::<Sensor>`; the zone-load reads/writes and `sensor_alloc_data`'s
  Sensor-then-EnergyMeter probe → `typed`/`typed_mut`.
- `solution/meters/{mod,zones/build,zones/flags,reliability,demand_interval}.rs`
  — the meter/sensor/load/XYcurve reads → `typed`/`typed_mut`; the shared
  `downcast_meter` helper is now `meter_mut` (`typed_mut::<EnergyMeter>`,
  renamed in its 6 users); `pd_full_name`'s five-way class probe → an `ElemId`
  variant `match` (same five names, same `"PDElement"` fall-through).
- `elements/meter/{monitor,energymeter,sensor}/accessors.rs` — the three
  `capture_metered`/`capture` RefSnapshot helpers now take the
  `ResolvedObj<'_>` the caller already holds instead of `&dyn DssObject`:
  `as_ckt_element()` → `o.ckt()`, the monitor's 10-way `MeteredKind` chain →
  `o.get::<Transformer>()`/`get::<AutoTrans>()`/`get::<Capacitor>()` plus two
  `matches!` over `ElemId`, and the EnergyMeter `is_pd` five-way probe → one
  `matches!` over `ElemId`.

**Bit-neutrality argument.** Every converted site resolves the same object by
the same `(class ordinal, index)`; `typed`/`typed_mut` are the proven twins of
the downcast (`typed_accessors_match_the_any_downcast_for_every_class`), and the
`matches!(id, …)` chains replace probes over **mutually exclusive** classes, so
the arm chosen is the same regardless of order (`ElemId` variant ⇔ concrete
class by `arena_order_matches_registry`). `capture_metered` still runs at
resolve time, from the same object, filling the same snapshot fields — only the
type channel moved. No list, loop, `find_*` tie-break, control-queue insertion,
stamp or accumulation order was touched; no arithmetic is in the diff.

**New proof surface.** `exec::registry::tests::typed_store_accessors_match_the_untyped_pair_and_downcast`
gains a `typed_obj_pair_mut` case over the LIVE `ClassStore`, asserting **pointer
identity** with the untyped `pair_mut` on both branches — cross-arena
(monitor ⇄ line) and same-arena (monitor ⇄ monitor, the case reachable via
`element=monitor.…`). Two monitors were added to that test circuit; no case was
removed or weakened.

**ESCAPE, narrowed → R3.3 — the 4 concrete reads inside
`elements/meter/monitor/sample.rs`.** Old code untouched, gate green.
`Monitor::take_sample(metered: &mut dyn DssObject, …)` still recovers
`Capacitor::states` (mode 9, l.135), `Storage` present kW/kvar/kWh/state (mode
11, l.176), `Transformer::get_all_winding_currents` (mode 8, l.226) and
`Transformer::get_winding_voltages` (mode 10, l.250) by downcast, and reads the
element through 8 `as_ckt_element*` calls. The borrow side is now typed
(`typed_obj_pair_mut`), so the residue is purely the **reader**: retyping the
parameter to `&mut dyn CktElement` loses those four reads, and both ways out
are R3.3's call, not this step's —
1. four monitor-specific typed reads on the `CktElement` trait (the plan's R0
   "small typed reads" bullet) = a 50-class trait-surface decision; or
2. a typed `(arena, idx)` view, whose **same-class** branch (a monitor whose
   `element=` names another monitor) cannot hand out an arena view while the
   monitor itself is borrowed — it would silently return `None` for the
   concrete reads, i.e. a behavior change on an unprobed branch, which the
   escape protocol forbids.
R3.3 removes `as_ckt_element` from `DssObject` and must decide this for every
bare-`&dyn` reader at once; `take_sample` is the last member of that family in
the meter cluster.

**Still open → R3.3 — R3.1's escaped sub-step (iv)** ("statically-known shape
refs as typed `Idx<T>`", 34 field declarations / 16 names / ~14 classes;
inventory in the R3.1 record). Unchanged by this step, and still best done
**with** R3.3's arena-read sweep, since the fields' readers are exactly the
sites that sweep converts.

**Metrics (`rg -c … crates/dss-core/src`, summed lines / files).**

| metric | R3.2 part-2 `377bbc8` | HEAD | delta | note |
|---|---|---|---|---|
| `downcast_ref\|downcast_mut` | 310 / 57 f | **261 / 47 f** | **−49** | the whole meter cluster (**−10 files**) |
| `as_any\|as_ckt_element` | 619 / 134 f | **569 / 126 f** | **−50** | their `as_any()` halves + 4 `as_ckt_element` |
| `TODO(compat)` | 117 / 68 f | **117 / 68 f** | **0** | invariant held |

**R3.2 total vs its base `275de68`:** `downcast_ref|downcast_mut` **369 → 261
(−108, 73 → 47 files)**, `as_any|as_ckt_element` **716 → 569 (−147)**,
`TODO(compat)` **117 unchanged**, zero golden / ledger / tolerance /
corpus-deck churn.

**Deviations disclosed.** (1) `downcast_meter` → `meter_mut` (private helper,
6 users) — the old name describes a mechanism that no longer exists. (2) The
three RefSnapshot helpers changed signature (`&dyn DssObject` →
`ResolvedObj<'_>`); each has exactly one caller, which already held the handle.
(3) In `allocate.rs` the two `pair_mut` conversions move the meter/sensor
class-mismatch panic *ahead* of the `"metered element is a circuit element"`
expect (`typed_ckt_pair_mut` narrows `C` first). Unreachable in practice —
`ckt.energy_meters`/`ckt.sensors` only ever hold their own class — and it is the
same deviation already disclosed for the R3.2 part-1 dispatch conversions.
(4) Item (e)'s reader half escaped (above). (5) Ritual step 3 (two fresh audits)
is not run in this session; the coordinator spawns the R3 audits.

**Gate (full, SOLO, toolchain guard first).** `cargo fmt --all --check` exit 0;
`cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test
--workspace` **exit 0** — 66 `test result: ok` groups, **1989 passed, 0 failed,
5 ignored** (the same 5 pre-existing ones; zero `#[ignore]` churn),
`corpus_gate_all_cases_match_engines … ok` (25 passed, both channels
capi_v0145 + r4133), run solo with no name filters. Corpus left pristine: the
run-artifacts listed by `git status tests/corpus` removed by exact name — no
wide `git clean`. Tree clean.

### DE_PASCALIZE R3.2 (part 2/3) — Category-D typed resolved-object handle (`ResolvedObj`) across all 29 `set_object_ref` files; item (e) escape-recorded (branch `depas-r3`, 2026-07-26)

Stratum **[A]** bit-neutral, type-channel only. Base = the R3.2 part-1 tip
`7dc07ea`. Ritual 0 held at start **and** before the commit: 186 `.pas` under
`.inputs/dss_capi`; `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**Delivered: R3 handoff item 2's sub-item (c).** The shared resolved-object
tuple `Option<(ElemId, &dyn DssObject)>` is replaced tree-wide by a **typed
handle**:

```rust
pub struct ResolvedObj<'a> { id: ElemId, arena: &'a ClassArena }   // obj/arena.rs
//   .id()  .obj()  .name()  .ckt()  .get::<T>()  .cloned::<T>()
```

`ResolvedObj::new(arena, idx)` derives the `ElemId` from the arena itself
(new `ClassArena::id(idx)`), so the handle and the storage **cannot** disagree —
there is no way to build a mis-classed one. `get::<T>`/`cloned::<T>` are static
[`ArenaClass`] matches: the Category-D snapshot clones
(`o.as_any().downcast_ref::<LoadShapeObj>().cloned()` etc.) become
`o.cloned::<LoadShapeObj>()` with **no `Any` round-trip**.

**Retyped, in one sweep (39 files, all under `crates/dss-core/src/`):**
- `DssObject::set_object_ref(idx, name, resolved: Option<ResolvedObj<'_>>)` and
  `ObjectRefArrayItem<'a> = Option<(String, ResolvedObj<'a>)>` (`obj/base/mod.rs`).
- `ForeignClassesView::find` → `Option<ResolvedObj<'a>>`, `find_full` →
  `Option<(ResolvedObj<'a>, String)>` (`obj/props/engine.rs`), implemented by
  `ForeignClasses` (`exec/registry.rs`).
- The **29** per-class `set_object_ref` impls + the 3 `ObjectRefArrayItem`
  consumers (`line/code.rs` `set_wires`/`set_conductors`/`set_cables`,
  `line_geometry/{accessors,edit}.rs`).
- The producers: `obj/props/class_props/parse.rs` (both single-ref paths + both
  array paths) and `exec/command.rs` (TCC injection ×2, Spectrum, the
  InvControl/ExpControl DER-fleet scans, the GICsource Line resolve).
- The two test helpers (`line/tests.rs`, `line_geometry/tests.rs`) now build a
  real one-object `ClassArena` per target (`arena_of::<T>`) instead of faking an
  `ElemId::new(0, 0)`, so the tests exercise the same typed path production does
  — a strictly stronger fixture, and the stored handle now carries the object's
  **true** class ordinal.
- Two supporting arena primitives: `ClassArena::id(idx)` and
  `ClassArena::push::<T>(obj)` (typed append; `ArenaClass::arena_slice*` became
  `arena_vec*` returning the `Vec` so `push` can exist — `all`/`all_mut` still
  hand out slices).

**Bit-neutrality argument.** The clone still happens inside the same
`set_object_ref` call, at resolve time, from the same object — only the type
channel changed (`DE_PASCALIZE_PLAN.md` Part I, "Category D timing"). The
`ForeignClasses` scan order is unchanged (left half, then right half, first
class-name match wins, `cls.Find` semantics); the returned class ordinal moved
from a hand-computed `k`/`split + 1 + k` to `ClassArena::id`, which is the same
number by the `arena_order_matches_registry` invariant. The dump name path is
unchanged (`resolved.name()` ⇔ `obj.data().name()`; `find_full` still rebuilds
`Class.Name` from `props.class_name()`). Every `downcast_ref::<T>().cloned()`
became `cloned::<T>()`, which returns `Some` on exactly the same class match.
No golden, ledger, tolerance or corpus deck was touched.

**ESCAPED → R3.3 — sub-item (e), the Category-B monitor `take_sample`
bare-`&dyn` reader.** Old code untouched, gate green. `solution/monitors.rs:61`
still does `store.pair_mut(mon_ref, metered_ref)` + `as_any_mut()
.downcast_mut::<Monitor>()`, and `Monitor::take_sample(metered: &mut dyn
DssObject, …)` (`meter/monitor/sample.rs`) still reads its metered element
through 8 `as_ckt_element*` calls **and 4 concrete downcasts** (mode-9
`Capacitor::states`, mode-11 `Storage` present kW/kvar/kWh/state, mode-8
`Transformer::get_all_winding_currents`, mode-10
`Transformer::get_winding_voltages`). Blocker: retyping the parameter to
`&mut dyn CktElement` **loses** those four concrete reads, and the two ways out
are both bigger than this step:
1. add 4 monitor-specific typed reads to the `CktElement` trait (the plan's R0
   "small typed reads" bullet) — a 50-class trait-surface change; or
2. pass a typed `(arena, idx)` view instead — which needs a disjoint-borrow
   getter whose **same-class** branch (a Monitor whose `element=` names another
   Monitor — reachable, `find_ckt_element` resolves Monitor like any other
   circuit class) cannot hand out an arena view, so it would silently return
   `None` for the concrete reads. Changing behavior on that branch without a
   probe is exactly what the escape protocol forbids.
Recommendation: fold (e) into R3.3, where the `as_ckt_element` trait-method
removal forces the decision for every bare-`&dyn` reader at once (the same
`capture_metered`/`capture` family listed in the R2b (e) blocker map).

**Still open → R3.3 — R3.1's escaped sub-step (iv), "statically-known shape refs
as typed `Idx<T>`" (34 field declarations, 16 names, ~14 classes; inventory in
the R3.1 record).** Its stated blocker is now **gone** — `ClassArena::get::<T>`
exists, and the write side is a one-variant `ArenaClass::idx_of` narrowing on
the `ResolvedObj` this step landed. It was *not* done here on sequencing
grounds, disclosed: the fields' readers are exactly the 214 direct-arena
`downcast_*` sites (`cim/power_xfmr.rs:761-775` reading
`Transformer::xfmr_code_ref()` is the canonical one), which R3.3 converts
wholesale. Retyping the fields first would force those readers to be touched
twice. R3.3 should do `(iv)` **with** its arena-read sweep, in one pass.

**Metrics (`rg -c … crates/dss-core/src`, summed lines / files).**

| metric | R3.2 part-1 `7dc07ea` | HEAD | delta | note |
|---|---|---|---|---|
| `downcast_ref\|downcast_mut` | 341 / 75 f | **310 / 57 f** | **−31** | all Category-D resolve-time clones (**−18 files**) |
| `as_any\|as_ckt_element` | 667 / 134 f | **619 / 134 f** | **−48** | the `as_any()` half of those clones |
| `TODO(compat)` | 117 / 68 f | **117 / 68 f** | **0** | invariant held |

**R3.2 total vs its base `275de68`:** `downcast_ref|downcast_mut` **369 → 310
(−59, −16 files)**, `as_any|as_ckt_element` **716 → 619 (−97)**, `TODO(compat)`
**117 unchanged**, zero golden / ledger / tolerance / corpus-deck churn.

**Deviations disclosed.** (1) `ArenaClass::arena_slice`/`arena_slice_mut` were
renamed to `arena_vec`/`arena_vec_mut` and now return the `Vec` (so
`ClassArena::push` can exist); the public slice API is unchanged
(`all`/`all_mut`). (2) `ResolvedObj::new` takes `(arena, idx)` — deriving the
handle — rather than `(id, arena)`; this removes a whole class of possible
mis-pairing and is why `ForeignClasses::lookup` no longer computes the ordinal
by hand. (3) The two test `set_ref`/`set_ref_array` helpers changed signature
(`&dyn DssObject` → `&ClassArena` built by `arena_of`), which also fixes the
old fixture's fake `ElemId::new(0, 0)` handle; no test case was removed,
weakened or ignored. (4) Item (e) escaped (above). (5) Ritual step 3 (two fresh
audits) is not run in this session; the coordinator spawns the R3 audits.

**Gate (full, SOLO, toolchain guard first).** `cargo fmt --all --check` exit 0;
`cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test
--workspace` **exit 0** — 66 `test result: ok` groups, **1989 passed, 0 failed,
5 ignored** (the same 5 pre-existing ones; zero `#[ignore]` churn),
`corpus_gate_all_cases_match_engines … ok` (25 passed, 155.9 s, both channels
capi_v0145 + r4133), run solo with no name filters. Corpus left pristine: 18
run-artifacts (`StorageControllerTechNote/{Support,Time}/IEEE8500u_*.csv`)
removed by exact name — no wide `git clean`. Tree clean.

### DE_PASCALIZE R3.2 (part 1/3) — typed arena accessors + Category-A typed pair/triple getters + the `*_mut` helper family (branch `depas-r3`, 2026-07-26)

Stratum **[A]** bit-neutral, type-channel only. Base = the R3.1 tip `275de68`
(on `update` @ `67d2965`). Ritual 0 held at start **and** before the commit: 186
`.pas` under `.inputs/dss_capi` (PowerShell recursion — bash globbing false-zeros
through the junction); `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**Delivered: R3 handoff item 2's sub-items (a) accessors, (b) Category-A typed
pair/triple getters, (d) the `generator_mut`/`storage_mut`/`pvsystem_mut`/
`espvl_mut`/`upfc_mut` family — plus the two meter/exec pair sites that share
the same shape.** Sub-item (c) (Category-D typed resolved-object handle) and
(e) (the monitor `take_sample` bare-`&dyn` reader) are the remaining half; see
the follow-on record.

**The accessor design — `ArenaClass`, one static impl per registered class,
emitted by the SAME `with_all_classes!` list (no `Any`, no `TypeId`, no
vtable).** `T` alone determines the arena variant, the registration ordinal and
the `ElemId` variant, so every narrowing is a compile-time match arm whose
`None` arm is exactly the case the downcast returned `None` for:

- `obj/arena.rs`: `trait ArenaClass: DssObject + Sized + 'static` with
  `CLASS_NAME` / `CLASS_ORD` (the `ClassOrd` discriminant, so it is a constant,
  not a scan), `id(idx) -> ElemId`, `idx_of(ElemId) -> Option<Idx<Self>>`,
  `arena_slice`/`arena_slice_mut`, and `ckt_ref`/`ckt_mut` (the concrete-`&T`
  twin of `try_ckt_elem`, driven by the same `ckt`/`data` tag column).
- `ClassArena::get::<T>(idx) -> Option<&T>` / `get_mut::<T>` / `all::<T>` /
  `all_mut::<T>` (the plan's `ClassArena::get::<T>`); the pre-existing untyped
  `get` was renamed `get_obj` (2 call sites, `exec/make_pos_seq.rs`,
  `report/export/json/build.rs`). Plus `clone_ckt(idx) -> Option<Box<dyn
  CktElement>>` (typed clone for the self-monitoring paths) and
  `try_controlled_transformer_mut` (the Transformer|AutoTrans proxy) and
  `pair_ckt_mut` (two ckt views out of one arena).
- `elements/traits.rs`: `ElemStore` gains four dyn-safe primitives — `arena`,
  `arena_mut`, `arena_pair_mut`, `arena_triple_mut` — and a new blanket
  extension trait **`TypedStore: ElemStore`** (implemented for every store
  incl. `dyn ElemStore`) carrying the generic narrowing: `typed::<T>` /
  `typed_mut::<T>` (⇔ `obj(r).as_any().downcast_ref::<T>()`),
  `typed_ckt_pair_mut::<C>` → `(&mut C, Option<&mut dyn CktElement>)`,
  `typed_ckt_triple_mut::<C>`, `typed_transformer_pair_mut::<C>` →
  `(&mut C, Option<&mut dyn ControlledTransformer>)`, and
  `typed_pair_mut::<A, B>` / `typed_triple_mut::<A, B>` (both sides concrete —
  CapControl ⇄ Capacitor).

**Converted (all in `solution/controls/dispatch.rs` unless noted).** 12
`pair_mut` + 3 `triple_mut` control sites → the typed getters (Swt/Fuse/
Recloser/Relay); 3 CapControl `pair_mut` + 1 `triple_mut` → `typed_pair_mut`/
`typed_triple_mut::<CapControl, Capacitor>`; the RegControl cluster's
`is::<Transformer>()`/`is::<AutoTrans>()` downcast chain → one
`typed_transformer_pair_mut`; the 3 self-monitoring `clone_box()` +
`as_ckt_element_mut()` paths → `ClassArena::clone_ckt`; the 10 `generator`/
`generator_mut`/`storage`/`storage_mut`/`pvsystem`/`pvsystem_mut`/`espvl`/
`espvl_mut`/`upfc`/`upfc_mut` helpers → `typed`/`typed_mut`; plus
`exec/view.rs`'s sensor pair and `exec/command.rs`'s RegControl tap-sync pair.
17 `tobj`/`mobj`/`monobj` `as_ckt_element_mut()` calls disappear with them.

**Bit-neutrality argument.** Every converted site resolves the same object by
the same `(class ordinal, index)` and the same aliasing case analysis — the
typed getters mirror `pair_mut_arenas`/`triple_mut_arenas` branch for branch
(same-arena `get_disjoint_mut`, cross-arena `get_disjoint_mut` over classes),
and keep the `"pair_mut: aliasing refs"` / `"triple_mut: aliasing refs"`
asserts verbatim. No list, loop, `find_*` tie-break, control-queue insertion,
stamp or accumulation order was touched; no arithmetic is in the diff. The 3
self-monitoring paths still clone the controlled element *before* the disjoint
borrow and still abort (not panic) when it is not a circuit element — the
`clone_ckt` result is unwrapped only **after** the `Switched element is not a
circuit element` check, preserving the original ordering of the two failure
paths.

**Proof surface (2 new tests + 1 new store-level test + 1 panic test).**
`obj::arena::tests::typed_accessors_match_the_any_downcast_for_every_class` is
generated from the class list, so it covers **all 50** classes: for each, the
typed read is pointer-identical to `obj(0).as_any().downcast_ref::<T>()`,
out-of-range is `None`, the `id`/`idx_of`/`class_ord` round-trip holds, and
`ckt_ref`/`ckt_mut`/`clone_ckt` agree with the arena `ckt`/`data` tag.
`typed_accessors_reject_a_foreign_class` pins the other half of the downcast
contract (wrong arena / wrong handle ⇒ `None`, never a reinterpretation).
`exec::registry::tests::typed_store_accessors_match_the_untyped_pair_and_downcast`
does the same over a **live** `ClassStore` (RegControl⇄Transformer,
CapControl⇄Capacitor, monitored Line), and `typed_pair_mut_rejects_aliasing`
pins the aliasing guard.

**Metrics (`rg -c … crates/dss-core/src`, summed lines / files).**

| metric | base `275de68` | HEAD | delta | note |
|---|---|---|---|---|
| `downcast_ref\|downcast_mut` | 369 / 73 f | **341 / 75 f** | **−28** | all Category-A dispatch + the `*_mut` family |
| `as_any\|as_ckt_element` | 716 / 134 f | **667 / 134 f** | **−49** | the 17 `as_ckt_element_mut` pair/triple reads + the `as_any` chains |
| `TODO(compat)` | 117 / 68 f | **117 / 68 f** | **0** | invariant held |
| `ElemId` | 955 / 130 f | 984 / 130 f | +29 | the new accessor surface |
| `fn as_any` / `fn as_any_mut` defs | 52 / 52 | 52 / 52 | 0 | removal still gated on items (c)+(e)+R3.3 |

Zero golden / ledger / tolerance / corpus-deck churn (diff is 11 files, all
under `crates/dss-core/src/`). Zero `#[ignore]` churn.

**Deviations disclosed.** (1) `ClassArena::get` (untyped, `Option<&dyn
DssObject>`) was **renamed** `get_obj` so the typed `get::<T>` can carry the
name the plan specifies; both call sites updated, no behavior change. (2) The
`*_mut` helper family's panic *message* is now the helper's own `.expect(...)`
in the class-mismatch case where the old code would first panic inside
`obj_mut` on an out-of-range index — both are unreachable ("construction bug")
states, the messages for the reachable mismatch case are unchanged. (3)
`typed_pair_mut`/`typed_triple_mut` assert `A::CLASS_ORD != B::CLASS_ORD` (one
arena cannot hand out two different concrete types); the only user is
CapControl⇄Capacitor, structurally distinct. (4) The two test-only `ElemStore`
doubles (`ymatrix.rs::EmptyStore`, `monte_carlo.rs::FaultStore`) implement the
four new primitives as `unimplemented!()`, matching their existing style —
neither test path reaches them. (5) Ritual step 3 (two fresh audits) is not run
in this session; the coordinator spawns the R3 audits.

**Gate (full, SOLO, toolchain guard first).** `cargo fmt --all --check` exit 0;
`cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test
--workspace` **exit 0** — 0 failed anywhere, 5 ignored (the same 5 pre-existing
ones), `corpus_gate_all_cases_match_engines … ok` (25 passed, 185.7 s, both
channels capi_v0145 + r4133), run solo with no name filters. Corpus left
pristine: 4 run-artifacts (`Test/AutoTrans/auto*_current.txt`,
`auto1bus_ht_losses.txt`) removed by exact name — no wide `git clean`.

### DE_PASCALIZE R3.1 — the `ElemRef` → `ElemId` STORE FLIP: LANDED IN FULL (spine + access layer, zero `.to_ref()` bridges ever created) (branch `depas-r3`, 2026-07-26)

Stratum **[A]** bit-neutral, type-channel only. Base `update` @ `67d2965`. Ritual 0
held at start **and** before the commit: 186 `.pas` under `.inputs/dss_capi`
(PowerShell recursion — bash globbing false-zeros through the junction, never
deleted through it); `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**Result: the R3 handoff's item 1 is done except sub-step (iv).** The untyped
`ElemRef { cls: usize, idx: usize }` tag struct is **gone from the tree** (937 → 0)
and the typed `ElemId` (one `Idx<T>` variant per registered class) is the spine's
only element handle: `circuit.rs`'s 20 per-kind lists, `RefAction`'s 5 `target`s,
the `controlled_element`/`monitored_element` cross-refs, the `ElemStore` trait +
`find_ckt_element`/`find_general` returns, and the three sibling structures
(`control_queue` `ActionRecord`/`PoppedAction.control`, `ckt_tree`
`elem`/`shunts`/`loop_elem`, `gic_source` `set_resolved_line`) all speak `ElemId`.

**The 427-site "all-or-nothing" wall was an artefact of the sequencing, not of the
flip — measured and disproven this session.** R2b's (a) record measured 427 primary
compile errors by flipping **only** `circuit.rs`'s lists while leaving `ElemStore`
on `ElemRef`; every one of those errors is the *mismatch* between the two, not real
work. Retyping the store and the lists in the **same** step makes them type-check
untouched: `for &r in &ckt.lines { store.ckt_elem(r) }` never breaks when both sides
move together. So sub-steps (i)+(ii)+(iii)+(v) were executed as **one** atomic
commit and **no `.to_ref()` bridge was ever inserted** — step (v)'s "removes the
bridges" had nothing to remove. Measured real breakage of the atomic retype:
**511** `.cls`/`.idx` field accesses + **102** `{ cls, idx }` struct literals
(613 sites, 54 files), versus the sequenced path's 427 bridge insertions *plus*
their later removal. Disclosed as a deliberate minimal grouping (brief: "where two
sub-steps are inseparable, group minimally and disclose").

**Method (compiler-driven, no hand-editing of the 613 sites).** (1) `ElemRef` was
made a one-line `pub type ElemRef = ElemId;` alias, which retyped every signature,
`Vec<…>`, and `Option<…>` in the tree at once. (2) The 511 field accesses were
rewritten from **rustc's own E0609 diagnostic spans** (`--message-format json`,
line/column-exact, filtered on `"ElemId" in message`) → `.cls` ⇒ `.class_ord()`,
`.idx` ⇒ `.index()`; iterated to a fixpoint. (3) The 102 literals were rewritten by
one anchored regex (`ElemRef {` **immediately** followed by `cls`, so the `-> ElemRef {`
return-type sites are untouched) → `ElemId::new(cls, idx)`. (4) The alias was
replaced by a `pub use crate::obj::arena::ElemId;` re-export from
`elements::traits` and `ElemRef` renamed to `ElemId` tree-wide (811 identifiers,
126 files) — so **every existing `use crate::elements::traits::ElemRef` import path
survives verbatim as `::ElemId`**; the type's home stays `obj/arena.rs`.

**Two arena primitives changed (`obj/arena.rs`), both value-identical:**
- `ElemId::class_ord()` is now **O(1)**: a field-less `#[repr(usize)] enum ClassOrd`
  emitted from the *same* `with_all_classes!` list, so each arm is a compile-time
  discriminant. It was a 50-entry `CLASS_NAMES.iter().position()` string scan **per
  call** — acceptable while nothing called it, a real regression once the flip makes
  it the hot accessor behind every `store.ckt_elem(r)`. Same values, pinned by the
  unchanged `arena_order_matches_registry` (live registry ↔ `CLASS_NAMES` ↔ live
  `DssClass::arena`) and the retargeted round-trip tests.
- `ElemId::from_ref`/`to_ref` and the two `From` bridges are **deleted** — their
  consumer never materialised (R2b (e) swept and found zero external call sites) and
  `ElemRef` no longer exists. They are replaced by `ElemId::new(cls, idx)`, the
  dynamic-ordinal constructor the registry-side producers genuinely need
  (`find_ckt_element`, `find_general`, `ForeignClasses::lookup`, `add_ckt_element`,
  the class-loop reports). O(1) via a per-variant constructor table indexed by the
  ordinal; the two `from_ref` tests were retargeted onto `ElemId::new` with the same
  every-class-against-the-live-registry coverage (nothing weakened, nothing removed).

**Bit-neutrality argument (why this cannot move a number).** `class_ord()` returns
exactly the old `.cls`, `index()` exactly the old `.idx`, and `ElemId::new(c, i)` is
their inverse (pinned for **every** registered class against the live registry). No
list, loop, `find_*` tie-break, control-queue insertion, stamp or accumulation order
was touched — `git diff` contains no reordering, only type/accessor rewrites.
`ElemId`'s `PartialEq` compares (variant, `Idx`) ⇔ the old `(cls, idx)` tuple
compare. The only observable delta is the `Debug` rendering of a handle
(`ElemRef { cls: 18, idx: 3 }` → `Line(Idx(3))`); no golden, report or corpus row
reads it (whole gate incl. every text golden green, and no `{:?}` of a handle exists
in any report path).

**Metrics (`rg … crates/dss-core/src`).**

| metric | base `67d2965` | HEAD | delta | note |
|---|---|---|---|---|
| `ElemRef` | 937 / 130 f | **0 / 0 f** | **−937** | the type is gone from the tree |
| `ElemId` | 34 / 2 f | **955 / 130 f** | +921 | the spine now speaks it everywhere |
| `TODO(compat)` | 117 / 68 f | **117 / 68 f** | **0** | invariant held |
| `as_any\|as_ckt_element` | 716 / 134 f | **716 / 134 f** | 0 | untouched — R3 items 2/3 own these |
| `downcast_ref\|downcast_mut` | 369 / 73 f | **369 / 73 f** | 0 | untouched — the typed accessors are item 2 |
| `fn as_any` / `fn as_any_mut` defs | 52 / 52 | **52 / 52** | 0 | removal target, now unblocked |

Diff = 146 files, +1365/−1333, **all under `crates/dss-core/src/`**. Zero golden /
ledger / tolerance / corpus-deck churn (`git diff --name-only` matches nothing under
`tests/`, `*.json`, `*.csv`, `population.lock`, `*.toml`). Zero `#[ignore]` churn.

**ESCAPED → R3.2 — handoff sub-step (iv), "statically-known shape refs as typed
`Idx<T>`".** Not started; old code untouched, gate green. The candidate fields are
the resolve-time `_ref` companions of the Category-D snapshot clones —
`load/mod.rs:325-329` `yearly`/`daily`/`duty`/`cvr`/`growth_shape_ref`,
`inv_based_pce.rs:447-454` `yearly`/`daily`/`duty_shape_ref` +
`inverter_curve_ref` (shared by PVSystem/Storage), `pd/line/mod.rs:294` `line_code_ref`, `pd/transformer/mod.rs:321`
`xfmr_code_ref`, and their siblings: **34 field declarations, 16 distinct names,
~14 classes** (`*_shape_ref` → LoadShape, `*_t_shape_ref` → TShape,
`growth_shape_ref` → GrowthShape, `line_code_ref` → LineCode, `xfmr_code_ref` →
XfmrCode, `inverter_curve`/`loss_curve`/`power_temp_curve`/`vv_curve_ref` → XYcurve,
`dynamic_eq_ref` → DynamicExp, `gic_source.line_ref` → Line).

**Blocker — measured on the readers, not the writers.** The *write* side is easy
(the shared `DssObject::set_object_ref(idx, name, resolved: Option<(ElemId, &dyn
DssObject)>)` already carries an `ElemId`, so each class could narrow it with a
one-variant match). The *read* side is what blocks: `Idx<T>` deliberately drops the
class ordinal, and every non-trivial consumer of these fields still needs it —
e.g. `cim/power_xfmr.rs:761-775` does `classes[cr.class_ord()].arena[cr.index()]
.as_any().downcast_ref::<XfmrCodeObj>()` on `Transformer::xfmr_code_ref()`.
Dereferencing an `Idx<XfmrCodeObj>` requires the typed arena accessor
`ClassArena::get::<T>(idx) -> Option<&T>` (concrete ref, no `Any`) — which is
**R3 item 2**, and which simultaneously *removes* that downcast (item 3). Landing
(iv) alone would leave 34 fields that nothing can dereference until item 2 exists,
and would either need a per-class `set_object_ref_typed` bolted beside the existing
one (a stopgap the escape protocol forbids) or Category D pulled in early. This
matches the plan (`DE_PASCALIZE_PLAN.md` R2 l.342, "typed handle in resolved
object-ref tuple; per-class `set_object_ref` match") and the R2b (a) record's own
note that step 4 "couples to the typed-store reads of sub-step (b) (Category D), so
best done with (b)". **Recommendation: fold (iv) into R3 item 2**, where the typed
accessor, the per-class `set_object_ref` match and these 34 fields all land in one
coherent pass.

**Deviations disclosed.** (1) Sub-steps (i)/(ii)/(iii)/(v) grouped into one commit —
rationale + measurement above; they are inseparable in the zero-bridge form. (2)
`class_ord()` made O(1) (a performance fix the flip makes load-bearing), values
unchanged and pinned. (3) The `from_ref`/`to_ref`/`From` bridges were **deleted**,
not carried forward — `ElemRef` no longer exists, so they are uninhabitable; their
tests were retargeted, not dropped. (4) `ElemId::new` panics on an out-of-range class
ordinal where the old `ElemRef { cls, idx }` literal could not. Every producer was
audited: all read `cls` off a live registry enumeration (`classes.iter().enumerate()`,
`ForeignClasses`'s `k` / `split + 1 + k`, `command.rs`'s created-class index), so the
range is structurally guaranteed; the panic is the same class of "construction bug,
never a valid state" assertion the arena already carries. (5) `ElemId` is re-exported
from `elements::traits` (import paths unchanged) while its definition stays in
`obj/arena.rs`. (6) Ritual step 3 (two fresh `/audit-code` + `/audit-tests` agents) is
**not** run in this session — the coordinator spawns the audits for R3 steps; this
record is the audit brief's starting point.

**Gate (full, SOLO, toolchain guard first).** `cargo fmt --all --check` exit 0;
`cargo clippy --workspace --all-targets -- -D warnings` exit 0; `cargo test
--workspace` exit 0 — **66 `test result: ok` groups, 1985 passed, 0 failed, 5
ignored** (the same 5 pre-existing ones; zero `#[ignore]` churn), corpus gate
`corpus_gate_all_cases_match_engines` ok (25 passed, 149.6 s, both channels
capi_v0145 + r4133), run solo with no name filters. Corpus left pristine: 9
run-artifacts (`StorageControllerTechNote/Schedule/IEEE8500u_*.csv`) removed by exact
name — no wide `git clean`. Tree clean.

### DE_PASCALIZE R2b — SETTLER PASS (two audits both PASS / zero findings; all verified empirically; final gate green) (branch `depas-r2b`, HEAD after this record, 2026-07-26)

Stratum **[A]** bit-neutral. Base `update` @ `5a416ee`; code tip = the (e) STATUS
tip `868cd1e` (last production code = the (d) commit `107d7d5`; (a)+(c)+(d) are the
only production changes — see the sub-step records below). Ritual 0 held at start
**and** end: 186 `.pas` under `.inputs/dss_capi` (PowerShell recursion; bash glob
false-zeros on the junction, expected — never deleted through it); `cargo` =
`C:\Users\Admin\.cargo\bin\cargo.exe`.

**Both fresh audits returned PASS with zero production findings.** There was
therefore **no defect to fix** — the settler duty was to (1) re-derive every metric
and equivalence claim independently, (2) reconcile the two counting discrepancies
the audits surfaced, (3) run the full workspace gate solo (neither auditor did — the
code auditor ran none by instruction, the test auditor ran only `-p dss-core --lib`
+ `corpus_gate`). All done; results below.

**Disposition of every audit conclusion (settled empirically, not by argument):**

| # | audit conclusion | settler disposition (experiment) |
|---|---|---|
| C-a | order/iteration preserved; spine flip really escaped (`circuit.rs`/`find_ckt_element` byte-identical) | **CONFIRMED.** `git diff 5a416ee..HEAD` touches 55 files; `circuit.rs`, `exec/registry.rs` not among them. |
| C-b | no hidden downcast/unsafe/reorder; `try_ckt_elem` removes Any round-trip; downcast net 416→369 | **CONFIRMED.** Diff has **47 removed** `downcast_*` lines, **0 added** → exactly the make_like guards; measured HEAD downcast = **369**. |
| C-c | Category-D resolve-time snapshot untouched (escaped) | **CONFIRMED.** `dispatch.rs`, reg/cap resolve, `set_object_ref` producers not in diff. |
| C-d | 50/50 make_like moved byte-identically; dispatch = typed `clone()`+inherent | **CONFIRMED** (independent of the auditor): downcast delta −47 = 50 impls − 3 no-downcast; `make_like` gone from trait (`obj/base/mod.rs` −8), only `arena.rs::make_like_within` in production. |
| C-e | escaped sites left on OLD code (no half-flip); ElemRef +10 all in `arena.rs`; trait methods present | **CONFIRMED.** ElemRef +/- lines outside `arena.rs` = only one moved comment (net zero); `as_any`/`as_any_mut`/`as_ckt_element`/`as_ckt_element_mut`/`clone_box` all still in the trait; `make_like` removed. |
| C-f | M3b seam / live-ctx / FP-order untouched; `TODO(compat)` 117 | **CONFIRMED.** `TODO(compat)` = **117** (base 117); M3b/live-ctx files not in diff. |
| T-a | zero golden/ledger/tolerance/corpus-deck churn | **CONFIRMED.** `git diff --name-only 5a416ee..HEAD \| grep -Ei 'golden\|ledger\|population.lock\|tests/corpus\|\.csv\|tolerance\|\.toml\|\.json'` = empty. |
| T-b | test edits mechanical (2 files: relay/recloser); no weakened/removed/ignored cases | **CONFIRMED.** Only `relay/tests.rs` + `recloser/tests.rs` in diff; **no `#[ignore]` added/removed** anywhere in the diff. |
| T-c | R1 invariants live (`arena_order_matches_registry`, `find_ckt_element_tie_breaks_by_registration_order`, bridge tests); coverage grew | **CONFIRMED** by the full-suite run (all green, below). |
| T-d | P1/P2 perturbations both FAIL under mutation, clean revert | **CONFIRMED.** Tree was clean at session start (`git status --porcelain` empty at `868cd1e`) — no perturbation residue. |
| T-e | corpus gate green solo (25) | **SUPERSEDED** by the settler full-workspace solo run (below). |

**Two reconciliations (the audits' only numeric caveats), both settled:**
- *ElemRef "952 vs 937."* Counting method only: **937 matching lines** (`rg -c`
  summed — the STATUS convention) vs **952 occurrences** (`rg -o` — the test
  auditor's figure). Same code; non-material. STATUS keeps the line-count 937.
- *ElemId "31→34" — CORRECTED (verify-forward-handoffs).* Independent measure:
  base `5a416ee` = **20** occurrences (`git grep -o`), HEAD = **34**; delta **+14**,
  confined to `arena.rs` (the `from_ref` 50-ordinal match + `From` impls + (a)/(d)
  tests) plus the one pre-existing `exec/mod.rs` ref. The prior (a)/(e) records'
  base "31" was a mis-measure; the true base is 20. Not a success metric (ElemId is
  new R3 scaffolding); recorded accurately here.

**Final grep metrics (settler-measured, `rg … crates/dss-core/src`):**

| metric | base `5a416ee` | HEAD | delta | note |
|---|---|---|---|---|
| `ElemRef` (lines) | 927 / 130 f | **937 / 130 f** | +10 | R3-staged `from_ref`/`From` bridges + tests, all in `arena.rs` |
| `ElemRef` (occurrences) | — | 952 | — | `rg -o`; = the "952" the test auditor cited |
| `as_any\|as_ckt_element` | 759 / 134 f | **716 / 134 f** | −43 | make_like guard removal net of (d)'s +4 doc/test refs |
| `downcast_ref\|downcast_mut` | 416 / 100 f | **369 / 73 f** | −47 | all from make_like (50 impls − 3 no-downcast); 47 removed / 0 added in diff |
| `TODO(compat)` | 117 / 68 f | **117 / 68 f** | **0** | invariant held |
| `ElemId` (occurrences) | 20 / 2 f | **34 / 2 f** | +14 | R3 scaffolding in `arena.rs` (base corrected from the "31" in prior records) |
| `fn as_any` / `fn as_any_mut` defs | 52 / 52 | **52 / 52** | 0 | the R3 removal target (unblocked by the store flip) |

**Final gate (settler, SOLO, toolchain guard first — `cargo` = `.cargo\bin`):**
`cargo fmt --all --check` exit 0; `cargo clippy --workspace --all-targets -- -D
warnings` exit 0; `cargo test --workspace` exit 0 — **66 `test result: ok` groups,
1985 passed, 0 failed, 5 ignored** (the 5 are pre-existing, not R2b — zero
`#[ignore]` churn in the diff), `corpus_gate_all_cases_match_engines … ok` (both
channels capi_v0145 + r4133), run solo (no parallel corpus gate). Corpus left
pristine: 12 run-artifacts (6 `AutoTrans/*.txt` + 6 `GFM_IEEE8500/IEEE8500_Mon_*.csv`)
removed by exact name — no wide `git clean`. Tree CLEAN (`git status --porcelain`
empty).

**R3 handoff (single prerequisite: the item-1 store flip).** R2b is closed as a→e
executed. Landed production: (a) `ElemId::from_ref` + `From` bridges; (c) Category E
`make_like` in full (trait method removed, byte-identical, the only downcast
reduction 416→369); (d) the arena ckt/data tag (`try_ckt_elem`). R3, one
multi-session WP, in sequence:
1. **Store flip** `ElemRef → ElemId` (the escaped (a) 5-step remainder): `circuit.rs`
   per-kind lists → `Vec<ElemId>` (measured **427** all-or-nothing consumer sites,
   ~55 files, no cascade into the access layer) → `RefAction.target` → cross-refs
   (`controlled_element`/`monitored_element`) → statically-known shape refs as
   `Idx<T>` → the access layer (`ElemStore` + `find_*` + the control_queue/ckt_tree/
   gic_source siblings), which removes the `.to_ref()` bridges.
2. **Typed arena accessors** `ClassArena::get::<T>(id)`/`get_mut::<T>` (concrete ref,
   no `Any`) + **Category-A** typed pair/triple getters (`(&mut RegControl, &mut
   Transformer)` / `(&mut CapControl, &mut Capacitor)`; Transformer|AutoTrans via
   `ControlledTransformer`) + the **Category-D** typed resolved-object handle
   (retype the shared `set_object_ref` tuple across ~29 files, preserving resolve-
   time snapshot timing) + the `generator_mut`/`storage_mut`/`pvsystem_mut`/
   `espvl_mut`/`upfc_mut` family → typed matches.
3. With 1+2 in place the **364 production downcasts** (214 typed-arena reads / 28
   Category-A disjoint borrows / 119 Category-B/D bare-`&dyn` params — file-by-file
   map in the (e) record) and the **~174 `as_ckt_element*`** sites collapse
   mechanically; then remove `as_any`/`as_any_mut` (52+52 defs) **and**
   `as_ckt_element`/`as_ckt_element_mut` from `DssObject` together, and retire the
   `from_ref`/`to_ref`/`try_ckt_elem` bridges. `clone_box` stays (17 live sites).
4. **Category C leftover (coordinator check at R2b merge, 2026-07-26): conductor
   snapshot storage is still `dyn`-owned** — `line_geometry/mod.rs:113`
   `fwiredata: Vec<Option<Box<dyn DssObject>>>`, `:134` `line_spacing_obj:
   Option<Box<dyn DssObject>>`, and `line/accessors.rs` `line_wire_data` (same
   shape). The R0 `ConductorData` trait landed but the storage retype did not.
   R3 must retype these to typed snapshots (the ~11 conductor-snapshot
   `clone_box` call sites then become typed `Clone`, leaving only the dispatch
   `mon_clone` ×3 consumers of `clone_box`). Without this the Part I grep gate
   (`Box<dyn DssObject>` owned storage = zero) cannot close.

**Net R2b delta from base `5a416ee`:** `downcast_ref|downcast_mut` 416→369 (−47,
all make_like); `as_any|as_ckt_element` 759→716 (−43); `ElemRef` 927→937 (+10 R3
bridges); `ElemId` 20→34 (+14 R3 scaffolding); `TODO(compat)` **117 unchanged**;
**zero golden / ledger / tolerance / corpus-deck churn** across all five sub-steps.

### DE_PASCALIZE R2b — CLOSING SUMMARY (a→e complete; store flip = the single R3 prerequisite) (branch `depas-r2b`, 2026-07-26)

R2b executed the R2-escaped block as five sub-steps. **Landed (production
code):** (a) `ElemId::from_ref` + the `ElemRef`↔`ElemId` `From` bridges (the flip
enabling primitive); (c) **Category E `make_like` in full** — 50 per-class bodies
moved `impl DssObject`→inherent `impl X { fn make_like(&mut self, &Self) }`,
byte-identical, `make_like` **removed from the `DssObject` trait**, dispatch
narrowed to the single `ClassArena::make_like_within` typed-clone site (this is the
only sub-step that reduced downcasts: 416→369); (d) the arena **ckt/data tag
primitive** (`try_ckt_elem`/`try_ckt_elem_mut`, no `Any` round-trip) with arena
internals rerouted off `as_ckt_element*` onto the tag. **Escaped → R3 (all rooted
in one blocker):** (a) the `ElemRef`→`ElemId` **spine flip** (427-site first
field-group cluster + the 5-step sequenced remainder in the (a) record); (b)
Categories **A** (typed pair/triple getters) / **B** (meter disjoint-borrow reads)
/ **D** (typed resolved-object handle) + the `generator_mut`/`storage_mut`/
`pvsystem_mut`/`espvl_mut`/`upfc_mut` family; (d) removal of
`as_ckt_element`/`as_ckt_element_mut` from `DssObject` + its ~50 external sites;
(e) removal of `as_any`/`as_any_mut` from `DssObject` + the **364 production
downcasts** (see the (e) record below for the file-by-file blocker map). **Root
cause of every escape is a single item:** the item-1 store flip has **no
gate-green partial state** (a Rust field type is global, so the first field-group
flip alone breaks 427 all-or-nothing consumer sites, several × that for the whole
spine) and is the sole prerequisite that then unblocks A/B/D and *both* trait-
method removals. **R3 shape:** land the store flip as its own multi-session WP
(the 5-step sequence), add the typed arena accessors (`get::<T>`/`get_mut::<T>`
returning a concrete ref, no `Any`) + Category-A typed pair/triple getters + the
Category-D typed resolved-object handle; then the 364 `downcast_*` sites and the
~174 `as_ckt_element*` sites collapse mechanically and both trait-method families
are removed together. **Net R2b delta from base `update`@`5a416ee`:** `downcast_ref|
downcast_mut` 416→369 (−47, all from make_like); `as_any|as_ckt_element` 759→716;
`ElemRef` 927→937 (+10 = the R3-staged `from_ref`/`From` bridges + their tests);
`ElemId` 31→34; `TODO(compat)` **117 unchanged**; **zero golden / ledger /
tolerance churn** across all five sub-steps.

### DE_PASCALIZE R2b sub-step (e) — `as_any` removal: BLOCKED (364 production downcasts, no typed store to zero them); definitive R3 handoff produced (branch `depas-r2b`, 2026-07-26)

Stratum **[A]** bit-neutral. Base = the (d) tip `5c8d9a5` (on `update`
@ `5a416ee`). Brief step (e): remove `as_any`/`as_any_mut` from `DssObject` + delete
the boilerplate impls **only if** the production `downcast_*` count is actually
zero (test-only downcasts may be restructured or kept, least-churn, disclosed);
**all-or-nothing per method.** Brief fallback (binding): "If production downcasts
remain (escaped from b/c/d), do NOT remove the trait methods; instead produce the
definitive named list (file:line + blocker) as the R3 handoff."

**Verdict: BLOCKED — production downcasts = 364, not zero, and the typed store that
would zero them was escaped in (a)/(b).** So (e) removes **nothing** (zero
production code) and lands the definitive R3 handoff below. This is the same
store-flip wall that blocked (b) and (d): every remaining downcast resolves a
`&dyn DssObject`/`&mut dyn DssObject` to a **concrete** type, which fundamentally
needs a typed store/arena or a typed handle — the escaped item-1 flip. The (d)
`try_ckt_elem` tag cannot help: it yields `&dyn CktElement`, never a concrete `T`.

**Measured downcast population (verified independently this session,
`crates/dss-core/src`).** Total `downcast_ref|downcast_mut` = **369** =
**364 production** (69 files) + **3 inline `#[cfg(test)]`** (`elements/pd/transformer/
mod.rs:506,509`; `solution/inc_matrix.rs:187`) + **2 test-path**
(`exec/tests/live_ctx.rs`, `exec/tests/reliability.rs`). The 5 test-context sites
are left as-is (least-churn; they restructure trivially only alongside the R3
removal). Removal target if unblocked: `fn as_any`/`fn as_any_mut` defs = **52 each**
(50 per-class boilerplate impls + the trait decl + the base helper).

**Definitive R3 handoff — the 364 production downcasts by blocker** (classifier over
the source expression feeding each `downcast_*`; 361 auto-classified, ~3
window-misses fold into the arena buckets). Three blocker families, all gated on
the item-1 store flip:

- **(1) Typed-arena reads — 214 sites — mechanical once the flip lands a typed
  accessor `ClassArena::get::<T>(id) -> Option<&T>` / `get_mut::<T>(id)` (concrete
  ref, no `Any`):**
  - `store.obj_mut(r).as_any_mut().downcast_mut::<T>()` — **85**: `controls/
    dispatch.rs` 52, `meters/sampling/take_sample.rs` 9, `exec/command.rs` 5,
    `solution/ncim.rs` 5, `solution/monte_carlo.rs` 3, `solution/faults.rs` 2,
    `meters/mod.rs` 2, `solution/monitors.rs` 2, then `exec/set_cmd.rs`,
    `meters/sampling/allocate.rs`, `solution/{fault_study,power_flow,time_series}.rs` ×1.
  - `classes[c].arena[i].as_any().downcast_ref::<T>()` (direct arena index) — **77**:
    `cim/export.rs` 16, `exec/report.rs` 10, `cim/power_xfmr.rs` 8, `cim/ieee1547.rs`
    6, `exec/reduce.rs` 6, `exec/save_circuit.rs` 6, `exec/view.rs` 5,
    `exec/helpers.rs` 2, `report/export/profile.rs` 2, `report/show/{diagnostics,
    meters}.rs` 2 each, then a report/{export,show} + `report/save/dump.rs` tail ×1.
  - `store.obj(r).as_any().downcast_ref::<T>()` — **52**: `controls/dispatch.rs` 31,
    `meters/demand_interval.rs` 5, `meters/sampling/allocate.rs` 3, `circuit/
    auto_add.rs` 2, `meters/zones/flags.rs` 2, `solution/ncim.rs` 2, then
    `exec/command.rs`, `meters/{reliability,zones/build,sampling/take_sample}.rs`,
    `solution/{monitors,power_flow,topology}.rs` ×1.
- **(2) Category A disjoint borrows — 28 sites — need typed pair/triple getters
  `(&mut RegControl, &mut Transformer)` / `(&mut CapControl, &mut Capacitor)` by
  ElemId (mind Transformer|AutoTrans via `ControlledTransformer`):** `pair_mut` **23**
  + `triple_mut` **5**, almost all in `controls/dispatch.rs` — reg→xfmr cluster
  (`pair_mut` at :603/622/703/719/749/792/814/855/904/926/952), cap→cap cluster
  (:1009/1050/1065), and self-monitoring `triple_mut` (:680/766/875/1024 + one),
  plus 5 stragglers (`meters/sampling/allocate.rs` 2, `exec/command.rs` 1,
  `exec/view.rs` 1, `solution/monitors.rs` 1). The `mon_clone.as_ckt_element_mut()`
  self-monitor path also needs a typed clone from the arena (touches Category E's
  `clone_box` consumer, still live).
- **(3) Category B/D bare `&dyn`/`&mut dyn` params — 119 sites — need the Category-D
  typed resolved-object handle (change the shared `set_object_ref(resolved:
  Option<(ElemRef, &dyn DssObject)>)` tuple type across the ~29 `set_object_ref`
  files — producers `reg_control` accessors:319, `cap_control`:295/315 — and the
  bare-`obj` readers they feed: `capture_metered(obj: &dyn DssObject)` at
  `meter/monitor/accessors.rs:267` and `meter/energymeter/accessors.rs:17`) + the
  Category-B disjoint-borrow meter reads (`meter/monitor/sample.rs`):**
  `exec/command.rs` 24 (the `edit_*_class` object dispatch, `obj.as_any_mut().
  downcast_mut::<T>()` at :15-25), `report/save/dump/overrides.rs` 18,
  `meter/monitor/accessors.rs` 10, `exec/view.rs` 6, `meter/energymeter/
  accessors.rs` 5, `meter/monitor/sample.rs` 4, `pc/pvsystem/accessors.rs` 4,
  `report/save/save.rs` 4, `controls/dispatch.rs` 4, `cim/export.rs` 3 (the
  `conductor_geom_amps(o: &dyn DssObject)` helper :1577), `pc/{storage,windgen}/
  accessors.rs` 3 each, `pd/line/accessors.rs` 3, then a long tail of per-class
  `accessors.rs` (`reg_control`, `generator`, `load` ×2; `cap_control`,
  `inv_control`, `storage_controller`, `ind_mach012`, `isource`, `upfc`, `vccs`,
  `vsource`, `gic_transformer`, `reactor`, `transformer` ×1) + `control_elem.rs`,
  `general/line_geometry/edit.rs`, `elements/traits.rs`, `exec/{distribute,reduce,
  save_circuit}.rs`, `solution/{meters/demand_interval,ncim,meters/sampling/
  allocate}.rs` ×1-2.

**`from_ref`/`to_ref` bridge sweep (brief item).** Swept the whole tree:
`from_ref`, `to_ref`, `From<ElemRef>`, `From<ElemId>` have **zero external call
sites** — they live only in `obj/arena.rs` (the definitions + the (a) self-tests
`from_ref_covers_every_class_and_round_trips` / `elemid_ref_bridge_round_trips`).
The flip never started (escaped in a/b), so no call site "became typed"; there is
nothing to remove. The bridges are the R3 enabling primitive (exactly as (a)
landed them and (d) landed `try_ckt_elem`) — **KEPT and flagged for R3**. They are
`pub`, so no dead-code warning; the gate stays green.

**Metrics (HEAD `5c8d9a5`; (e) is zero-code, so identical to the (d) tip).**

| metric (`rg … crates/dss-core/src`) | count | note |
|---|---|---|
| `ElemRef` | **937** | +10 vs base = R3-staged `from_ref`/`From` bridges + tests |
| `as_any\|as_ckt_element` | **716** | −43 vs base 759 (make_like extraction, net of (d)'s +4 doc/test refs) |
| `downcast_ref\|downcast_mut` | **369** | 364 prod + 3 inline-test + 2 test-path (−47 vs base 416, all make_like) |
| `TODO(compat)` | **117** | **unchanged** (base = 117) |
| `ElemId` | **34** | +3 vs (a) = (d) `try_ckt_elem` test refs |
| `fn as_any` / `fn as_any_mut` defs | **52 / 52** | the removal target R3 unblocks |

Zero golden / ledger / tolerance churn (no production path edited this sub-step).

**Deviations disclosed.** (1) (e) landed **zero production code** — the trait
removal is all-or-nothing and the production downcast count is 364, not zero;
blocked on the same escaped store flip as (b)/(d). (2) No boilerplate impls
deleted, no trait method removed — correct per the brief's zero-count gate. (3)
Ritual "two fresh independent audits (code/tests)" not run — there is **no
production diff to audit** (STATUS-only commit); nothing changed to regress. (4)
The 3 inline-test + 2 test-path downcasts left in place (least-churn; they only
restructure alongside the R3 trait removal).

**Gate.** Toolchain guard first (`cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`;
186 `.pas` under `.inputs/dss_capi`). `cargo fmt --all --check` ok; `cargo clippy
--workspace --all-targets -- -D warnings` ok; `cargo test --workspace` exit 0 — 66
`test result: ok` lines, 0 failures, `corpus_gate_all_cases_match_engines … ok`
(both channels capi_v0145 + r4133), run **solo**. Tree clean; no corpus
run-artifacts left.

### DE_PASCALIZE R2b sub-step (d) — `as_ckt_element` removal: arena ckt/data tag primitive landed; trait-method removal escape-recorded (blocked on the escaped store flip) (branch `depas-r2b`, 2026-07-26)

Stratum **[A]** bit-neutral. Base = the (c) tip `6c40830` (on `update` @ `5a416ee`).
Brief step (d): tag the arena macro ckt-vs-data so `ClassArena` upcasts
`&dyn CktElement` directly (no `Any` round-trip); convert the ~50 external
`.as_ckt_element()/.as_ckt_element_mut()` call sites; then remove
`as_ckt_element`/`as_ckt_element_mut` from `DssObject` + delete the 35 boilerplate
impls. Brief fallback (binding): "If some call sites cannot be converted without
the typed store pieces that escaped earlier, escape-record them and leave the trait
methods in place (all-or-nothing per method: only remove a trait method when its
caller count is zero)."

**Verdict: both trait methods are BLOCKED — neither's caller count can reach zero
without the escaped store flip / Categories A/B/D/E.** So (d) lands the genuinely-
additive **enabling primitive** (the arena ckt/data tag, exactly parallel to how
(a) landed the `from_ref` primitive and escaped the flip) and escape-records the
trait-method removal + external-site conversion with a precise blocked inventory.
The R2 record's earlier estimate that "`as_ckt_element` removal is tractable-but-
large standalone" was optimistic — it counted only the ~50 `arena[r.idx]` sites and
did **not** enumerate the Category-B/D bare-object readers that also call the
immutable method (corrected here with evidence, per the "verify forward-handoffs"
rule).

**Landed — the arena ckt/data tag primitive (arena.rs only, +150/−66).**
- `with_all_classes!` gains a 4th column per class: `ckt` (the 35 circuit classes
  whose concrete `T: CktElement`) / `data` (the 15 `DSS_OBJECT` general classes
  that do not). Two inner dispatch macros `ckt_view_ref!`/`ckt_view_mut!` emit the
  **direct** `Some(&v[idx] as &dyn CktElement)` upcast for `ckt` variants and
  `None` for `data` variants (the cast is never generated for a non-`CktElement`
  type).
- New `ClassArena::try_ckt_elem(idx) -> Option<&dyn CktElement>` /
  `try_ckt_elem_mut(idx)` — the fallible twin of `ckt_elem`/`ckt_elem_mut`, tag-
  driven, **no `Any` round-trip**. Named `try_ckt_elem*` (not `as_ckt_element*`) so
  the arena accessor is distinct from the trait method and does not pollute the
  `as_ckt_element` grep metric.
- Arena internals **rerouted off the trait method onto the tag**: `ckt_elem`/
  `ckt_elem_mut` now delegate to `try_ckt_elem*().expect(...)` (same panic message,
  bit-identical), and `for_each_ckt_elem_mut` is an index loop over
  `try_ckt_elem_mut(idx)` (same 0..len order, same `ElemRef`, same data-skip).
  The arena no longer depends on `DssObject::as_ckt_element*` in production.
- New test `arena_tag_matches_trait_ckt_view`: for **every** registered class,
  `push_new` one object and assert `try_ckt_elem(0).is_some()` /
  `try_ckt_elem_mut(0).is_some()` equal the still-present `obj(0).as_ckt_element()`
  trait method object-for-object, and pin the circuit-class count at 35. Proves the
  tag is correct against the oracle it will eventually replace. All 6 `obj::arena`
  tests pass.

**Escaped — the trait-method removal + external-site conversion (recorded per
escape protocol; old code untouched, gate green). Both methods have nonzero
callers rooted in the escaped store-flip categories:**
- **`as_ckt_element_mut` (65 call sites) — blocked by Category A + E.** Control
  dispatch (`solution/controls/dispatch.rs`, 21 `_mut` sites) obtains the
  controlled/monitored element as a bare `&mut dyn DssObject` from
  `store.pair_mut(r, target)` / `triple_mut(...)` (`store: &mut dyn ElemStore`) and
  calls `tobj/mobj.as_ckt_element_mut()`; a typed `&mut dyn CktElement` from a
  disjoint borrow needs the typed-arena pair getters (Category A, escaped in (b),
  plan-scheduled with item-1 flip) — reachable only by bolting new
  `pair_mut_ckt`-style methods onto the generic `ElemStore` trait, an improvised
  stopgap the escape protocol forbids. Plus `mon_clone.as_ckt_element_mut()` (self-
  monitoring): `mon_clone` is an owned `Box<dyn DssObject>` from `clone_box`; its
  ckt view needs a **typed clone from the arena** (Category E), escaped.
- **`as_ckt_element` (109 call sites) — blocked by Category D + B.** Category-D
  `set_object_ref(resolved: Option<(ElemRef, &dyn DssObject)>)` (reg_control:319,
  cap_control:295/315) and the `capture_metered(obj: &dyn DssObject)` /
  sensor `capture(obj)` readers it feeds (energymeter/accessors.rs:19 ← :325,
  monitor/accessors.rs:269 ← :208, sensor/accessors.rs:18) take a **bare
  `&dyn DssObject` with no store/arena in scope** and read `cd()` via
  `as_ckt_element()`. Feeding a ckt view means changing the shared resolved-object
  tuple type across all 29 `set_object_ref` files — the escaped Category-D typed-
  handle redesign ("Preserve resolve-time snapshot timing"). Monitor `sample.rs`
  reads (`metered.as_ckt_element()` :60/:96) are on a `&mut dyn DssObject` disjoint-
  borrow param (Category B) — the same disjoint-borrow wall as dispatch.
- **The ~90 remaining `arena[r.idx].as_ckt_element()` sites** (report/exec/solution/
  diakoptics/cim) ARE mechanically convertible to `arena.try_ckt_elem(r.idx)`, but
  converting them does **not** remove either trait method (the blocked sites above
  keep both alive), and the plan sequences the full conversion **with** the store
  flip (item 1) — so per the escape protocol (leave old code, gate green) and the
  (a)/(b) precedent (land the primitive, don't churn a blocked category early),
  they are left for the flip WP. `try_ckt_elem*` are ready for that mechanical pass.

**Metrics (HEAD vs (c) base `6c40830`).** Only `crates/dss-core/src/obj/arena.rs`
changed. Trait-method caller counts **unchanged**: `.as_ckt_element()` 109,
`.as_ckt_element_mut()` 65 — zero external sites converted, neither trait method
removed, 35 impls intact. `as_any|as_ckt_element` 712 → **716** (+4 = the doc/test
references to the *unchanged* trait method in arena.rs, incl. the equivalence-test
oracle call `obj(0).as_ckt_element()`; no new trait-method call sites).
`downcast_ref|downcast_mut` **369** (unchanged). `TODO(compat)` **117** (unchanged).
`ElemRef` 937 (unchanged). Zero golden / ledger / tolerance churn.

**Deviations disclosed.** (1) (d) removed neither trait method — blocked on the
escaped store flip (same wall as (b)); landed the additive tag primitive instead.
(2) Named the arena accessors `try_ckt_elem*` (not `as_ckt_element*`) to keep the
grep metric honest and disambiguate from the trait method. (3) Did not convert the
~90 convertible external sites (churn for zero removal; plan-sequenced with the
flip) — `try_ckt_elem*` are staged for that pass.

**Gate.** Toolchain guard first (`cargo = .cargo\bin`, 186 `.pas`). `cargo fmt
--all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok;
`cargo test --workspace` (corpus gate inside, both channels capi_v0145 + r4133, run
solo) — exit 0, all suites green incl. the 6 `obj::arena` tests. Tree clean; corpus
run-artifacts removed by exact name.

### DE_PASCALIZE R2b sub-step (c) — Category E `make_like`: trait method → inherent typed fn (50 classes) landed, trait method removed (branch `depas-r2b`, commit `5a6a41c`, 2026-07-26)

Stratum **[A]** bit-neutral. Base for this sub-step = the (a)+(b) tip `c32a2cb`
(itself on `update` @ `5a416ee`). Executes plan §Category E (`DE_PASCALIZE_PLAN.md`
R2, brief step (c)) **standalone** — it does NOT depend on the escaped store flip
(item 1), so unlike (b) it was fully executable now.

**What landed (commit `5a6a41c`, 54 files, +1844/−1856).**
- All **50** per-class `make_like` bodies moved out of `impl DssObject for X` into
  an inherent `impl X { pub(crate) fn make_like(&mut self, other: &Self) { … } }`
  block placed immediately above the trait impl. The downcast guard
  (`let Some(o) = other.as_any().downcast_ref::<X>() else { return; }` /
  `if let Some(o) = … {`) is dropped; the body is **byte-identical** (mechanically
  copied, not retyped — see proof below). Guard-bound name preserved via a
  `let o = other;` alias where the body used `o`; `if let` bodies kept inside a
  bare `{ }` block (zero body-byte change, clippy-clean). The 3 no-downcast impls
  (Spectrum, TccCurve, DynamicExp — read via typed `DssObject` accessors) moved
  sig-only. DynamicExp keeps its error-only no-op (`_other: &Self`).
- Production dispatch = the single site `ClassArena::make_like_within` (arena.rs):
  now `let src = v[source].clone(); v[target].make_like(&src);` (typed `Clone` +
  inherent call) instead of `clone_box()` + trait dispatch. Source-snapshot-before-
  mutable-borrow (the `source == target` aliasing safety) preserved; `clone()`
  yields the same value as `clone_box()` (which is `Box::new(self.clone())`).
- `fn make_like` **removed from the `DssObject` trait** (base/mod.rs) — all callers
  converted (only `make_like_within` in production; unit tests call the concrete
  type). Two test-only touch-ups: `relay/tests.rs` `&src as &dyn DssObject` → `&src`;
  `recloser/tests.rs` dropped the now-unused `use …DssObject`.
- `clone_box` **kept** (still live: moved line_geometry conductor snapshots +
  line/dispatch sites) — not deleted, per brief.

**Byte-neutrality proof.** A scratch script line-diffed each of the 50 bodies
base(`5a416ee`)→HEAD after stripping only sig/guard/alias/bare-block: **0
differences across all 50**. Metrics: `TODO(compat)` = **117** (unchanged);
`downcast_ref|downcast_mut` **416 → 369** (−47 = 50 impls minus the 3 no-downcast
classes); `as_any|as_ckt_element` **759 → 712** (−47); zero golden / ledger /
tolerance churn (`git show --stat`: only src + 2 test files).

**Gate (full, solo).** `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` clean; `cargo test --workspace` exit 0 — corpus gate
`corpus_gate_all_cases_match_engines … ok` (25 passed, 148 s, both channels
capi_v0145 + r4133). Corpus tree left pristine (19 run-artifacts removed by exact
name, no wide clean). Two fresh audits (code + tests) both PASS/no-findings.

**Deviations disclosed.** (1) `if let` guards became `let o = other; { … }` bare
blocks rather than de-indented bodies — chosen to keep body bytes identical (zero
retype risk); clippy-clean. (2) Ritual audits found nothing to settle empirically
(pure mechanical move proven byte-identical + gate-green), so no probe experiments
were needed beyond the body-equivalence diff.

### DE_PASCALIZE R2b sub-step (a) — spine flip: `ElemId::from_ref` primitive landed; full `ElemRef → ElemId` flip escape-recorded (branch `depas-r2b`, 2026-07-25)

Stratum **[A]** bit-neutral. Base `update` @ `5a416ee` (R2 item-7 M3b seam
merged). R2b's brief split the escaped Part-I flip into sub-steps (a)…(e); this
session is **(a) the spine flip**. Ground truth: the R2 record below already
established the flip is **one-session-infeasible gate-green** (927 `ElemRef` hits
/ 130 files; `ElemRef` and `ElemId` are different types, so a half-flipped tree
does not compile). This session confirmed that empirically and landed the
**enabling primitive** both the brief and the R2 record flagged as "missing and
needed first," then escape-recorded the flip with **measured** metrics that make
the remainder estimable and sequenceable.

**Landed — the `from_ref` primitive (commit `3c976a3`).** `obj/arena.rs` only
(+56 lines; `git diff 5a416ee..HEAD --stat` = one file):
- `ElemId::from_ref(ElemRef) -> ElemId` — a `match` over all 50 class ordinals
  via `Self::CLASS_NAMES[r.cls]` (registration order; names are distinct), the
  inverse of the pre-existing `to_ref`. `unreachable!` on an out-of-range/unknown
  ordinal (an invalid ref is a construction bug, never a valid state).
- `impl From<ElemRef> for ElemId` / `impl From<ElemId> for ElemRef` — the R2
  spine-flip bridges (producers still speaking `ElemRef` feed `.into()`;
  consumers still calling the `ElemRef`-typed access layer feed `id.to_ref()` /
  `.into()`). Both removed once the access layer is retyped (later R2 / R3).
- Tests: extended `elemid_ref_bridge_round_trips` (both `From` directions) +
  new `from_ref_covers_every_class_and_round_trips` (asserts `from_ref` selects
  the correct variant for **every** registered class ordinal against the live
  registry, and `from_ref`/`to_ref` are mutual inverses). All 5 `obj::arena`
  tests pass.

**Escaped — the field/reference/access-layer flip (recorded per escape protocol;
NOT started, old code untouched, gate stays green).** The flip cannot be landed
as a partial gate-green prefix: a Rust field type is global, so flipping any one
storage field-group breaks **all** its consumers at once, and there is **no
gate-green landing state between "primitive only" and "the whole field-group +
every consumer bridged."** Measured this session by actually performing the
first field-group flip (`circuit.rs` per-kind lists `Vec<ElemRef>` → `Vec<ElemId>`
+ producer `add_ckt_element` + the `ReprocessBusDefs` clone) and running
`cargo check` — then reverting cleanly (`git checkout`, tree restored to
`3c976a3`):
- **`circuit.rs` per-kind lists alone → 427 distinct primary compile-error sites
  across ~55 files** (E0308 type-mismatch 227 / E0609 `.cls`|`.idx`-on-`ElemId`
  196 / E0277 `.collect::<Vec<ElemRef>>()` 4). **No cascade into the access
  layer** — `elements/traits.rs` (`ElemStore`/`CktElement`) appears only in
  cited "expected because of this" *notes*, never as a primary error; the flip is
  contained to consumers.
- The sites are **uniformly mechanical** (three bridge patterns: `id.to_ref()`
  at loop tops or arg sites; `.class_ord()`/`.index()` for scattered `.cls`/`.idx`
  field access; `.map(|id| id.to_ref()).collect()` for the 4 `collect`s) but
  **scattered, not loop-collapsible** — e.g. `cim/export.rs` = 47 sites spread
  across lines 1197…4400 in many distinct functions (30 distinct lines, most
  carrying two `.cls`+`.idx` errors). Top consumer files: `cim/export.rs` 47,
  `exec/report.rs` 33, `exec/view.rs` 26, `controls/dispatch.rs` 22,
  `cim/power_xfmr.rs` 14, `cim/ieee1547.rs` 13, `solution/inc_matrix.rs` 12,
  `report/show/diagnostics.rs` 11, `meters/sampling/take_sample.rs` 10,
  `meters/demand_interval.rs` 10, then a long tail of report/show + report/export
  + solution + exec files.
- **Cascade boundaries mapped (all containable via `.to_ref()` bridges, no forced
  co-flip):** three sibling structures also store `ElemRef` and would be bridged
  at their push/read boundary, not co-flipped in this cluster —
  `solution/control_queue/mod.rs` (`ActionRecord`/`PoppedAction.control: ElemRef`),
  `circuit/ckt_tree/mod.rs` (node `elem`/`shunts`/`loop_elem: ElemRef`),
  `elements/pc/gic_source/mod.rs` (`set_resolved_line(Option<(ElemRef,String)>)`).

**Sequenced remainder for the (a)-continuation / (b) (do these as their own
sessions/commits, each `cargo check`-green cluster-by-cluster):**
1. `circuit.rs` per-kind lists → `Vec<ElemId>` + bridge the 427 consumer sites
   (the measured cluster above). Largest single cluster; ~55 files.
2. `RefAction.target` (`obj/base/mod.rs`, 5 variants) → `ElemId`; bridge the ~5
   control-accessor producers + the `apply_ref_action` applier.
3. Cross-references: the stored `controlled_element`/`monitored_element` fields
   (in `ControlElemData`/meter data) → `ElemId`, and the trait getters
   `CktElement::controlled_element`/`monitored_element_ref` (flipping the getter
   signature touches every override + caller — all-or-nothing per getter).
4. Shape/object refs where the class is statically known → typed `Idx<T>` (e.g.
   Load `daily: Option<Idx<LoadShapeObj>>`) — couples to the typed-store reads of
   sub-step (b) (Category D), so best done with (b).
5. The access layer itself — `ElemStore` trait methods + `find_ckt_element`/
   `find_general` returns + the sibling structures (control_queue/ckt_tree/
   gic_source) → `ElemId`; this removes the `.to_ref()` bridges. All-or-nothing
   big-bang (one impl of `ElemStore`, but every caller flips together).

**Deviations disclosed.** (1) Sub-step (a) did **not** flip any storage/reference
field (the R2 record's item-1); only the primitive landed. Reason: the escape
protocol forbids a rushed stopgap, and the flip has no committable partial
gate-green state (427 all-or-nothing sites for the first field-group alone,
several × that for the full spine). The primitive is the genuinely-additive,
zero-risk, unblocking piece; the field flips are handed on with a measured map.
(2) `ElemRef` count rose 927 → 937 / files unchanged 130 (the 10 new hits are the
`from_ref`/`From` bridges + their test in `arena.rs`) — expected; the flip that
reduces the count is the escaped remainder. `ElemId` 31 hits. `TODO(compat)`
unchanged (117). Zero golden/tolerance/ledger churn (no non-test production path
edited).

**Gate.** Toolchain guard first (`cargo = .cargo\bin`, 186 `.pas`). `cargo fmt
--all --check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok;
`cargo test --workspace` (corpus gate inside, both channels capi_v0145 + r4133,
run solo) — exit 0, all workspace suites green (corpus gate
`corpus_gate_all_cases_match_engines` ok inside the 26-test dss-core integration
binary). Tree clean; corpus run-artifacts removed by exact name.

### DE_PASCALIZE R2b sub-step (b) — typed-store categories: all escape-recorded (blocked on the un-flipped store) (branch `depas-r2b`, 2026-07-25)

Stratum **[A]** bit-neutral. Base = sub-step (a)'s HEAD `3c976a3` (`from_ref`
primitive only). The brief specifies (b) is executed **"on the flipped store"**:
Category A pair getters (`(&mut RegControl,&mut Transformer)` /
`(&mut CapControl,&mut Capacitor)` by `ElemId` match), Category B meter reads,
Category D typed handle in resolved object-ref tuples, and the
`generator_mut`/`storage_mut`/`pvsystem_mut`/`espvl_mut`/`upfc_mut` helpers →
typed arena matches. **The flipped store does not exist yet:** sub-step (a)
landed only the `from_ref` primitive and escape-recorded the field/reference/
access-layer flip (its "sequenced remainder" steps 1–5, ~427 all-or-nothing
sites for the first field-group alone). Verified independently this session —
`ElemStore` is still `dyn` with `obj_mut(r) -> &mut dyn DssObject` as its sole
typed path; `circuit.rs` per-kind lists are still `Vec<ElemRef>`; measured
populations unchanged from (a): `ElemRef` 937 / 130, `as_any|as_ckt_element`
759 / 134, `downcast_ref|downcast_mut` 416 / 100, `TODO(compat)` 117.

**Plan-sequencing confirms the block.** `DE_PASCALIZE_PLAN.md` R2 (l.334–345)
orders "Replace `ElemRef` with `ElemId` in `circuit.rs` (all per-kind lists),
every cross-reference, `RefAction`, and `solution/` Y-build" **first**, and only
*then* "Add typed arena pair getters so `dispatch.rs` gets `(&mut RegControl,
&mut Transformer)` … by `ElemId` match — emptying Category A" and "Finish
Categories D (typed handle in resolved object-ref tuple; per-class
`set_object_ref` match; concrete clone pulled from the typed arena)." The
category table (l.229–232) puts the A pair borrows, the Cat-B "rest via arena",
and Cat D all in Stage **R2**, downstream of the flip. Every category assigned to
(b) is therefore plan-blocked until the flip lands. Per the escape protocol +
the coordinator's explicit "if the typed store is not available for a given
site, escape-record rather than improvising downcasts," all of (b) is escaped
(old code untouched, gate stays green). No forbidden stopgap (concrete-typed
methods bolted onto the storage-agnostic `ElemStore` trait) was introduced.

**Per-category disposition (precise site inventory):**
- **Category A pair getters — ESCAPED.** `controls/dispatch.rs` reg→transformer
  L954–970 (RegControl + `ControlledTransformer`, with the `Transformer` vs
  `AutoTrans` branch at L958–964) and cap→capacitor L1011–1070 (4 sub-sites,
  each `CapControl` + `Capacitor`). Today reached via `store.triple_mut/pair_mut`
  → `(&mut dyn DssObject,…)` then `as_any_mut().downcast_mut::<T>()`. Typed
  `(&mut RegControl,&mut Transformer)` needs an `ElemId`/`ClassArena` match over a
  concrete-typed store (plan l.339–340) — not reachable through `dyn ElemStore`.
- **`generator_mut`-family helpers — ESCAPED.** `controls/dispatch.rs`
  `generator_mut` L1115, `espvl_mut` L1181, `upfc_mut` L1242, `storage_mut`
  L1385, `pvsystem_mut` L2343 — all `store: &mut dyn ElemStore` +
  `obj_mut(r).as_any_mut().downcast_mut::<T>()`. "→ typed arena matches"
  requires the typed store; same blocker as Category A.
- **Category B meter reads — R0 part DONE, remainder ESCAPED.** Already
  converted by R0 (verified in-tree, no work needed): the type-guards in
  `solution/meters/zones/build.rs` (L16/26/43 `matches!(store.kind(r), …)`) and
  `solution/meters/sampling/take_sample.rs` (L138/139/296/297 `store.kind`),
  plus the small typed `CktElement` reads (`line_length_km`, `load_num_customers`,
  `present_tap`). **Remaining, escaped:** the type-classification cascades in
  `elements/meter/energymeter/accessors.rs::capture_metered` `is_pd` (L25–32,
  Line/Transformer/AutoTrans/Capacitor/Reactor) and
  `elements/meter/monitor/accessors.rs` snapshot `MeteredKind` (L274–324), plus
  the concrete rich reads in `elements/meter/monitor/sample.rs` (L135 Capacitor
  `states()`, L177 Storage monitor vars, L227 Transformer
  `get_all_winding_currents`, L251 winding voltages). All take a bare
  `&dyn DssObject`/`&mut dyn DssObject` with **no `ElemRef`/store in scope**, so
  `store.kind` is unusable here. These are the plan's "concrete reads → typed
  arena reads (R2)" bucket (l.230, l.342–343); clean end-state is an `ElemId`
  match on the flipped store. Virtualizing the rich monitor reads as new narrow
  `CktElement` trait methods would create monitor-specific throwaway surface the
  flip tears out again (escape-protocol-forbidden stopgap); reshaping the
  classification cascades in isolation carries a bit-neutrality risk (the
  hand-enumerated PD / `MeteredKind` sets must be preserved exactly). Deferred to
  the coherent flipped-store Category B pass.
- **Category D typed handle in resolved object-ref tuples — ESCAPED.**
  `fn set_object_ref` lives across 29 element accessor files (the plan names
  `{load,line,vsource,generator}/accessors.rs`, l.232). The fix carries a typed
  handle (`ElemId`/`Idx<T>`) in the resolved object-ref tuple so each
  `set_object_ref` matches the expected variant and clones the concrete object
  from the typed arena (l.232, l.342). Resolution today yields a bare `ElemRef`/
  `&dyn DssObject` and the whole reference plumbing is `ElemRef`-typed; a typed
  handle needs the reference/store retype (sub-step (a)'s remainder steps 3–5).
  The brief's hard requirement to **preserve resolve-time snapshot timing
  exactly** cannot be guaranteed by an isolated conversion without the typed
  store, so escaped.

**Deviations disclosed.** (1) (b) produced **zero production code** — it is
entirely blocked on the un-done store flip; this STATUS record is the only
change. Metrics unchanged from (a) (`ElemRef` 937, `downcast` 416, `as_any` 759,
`TODO(compat)` 117); zero golden/tolerance/ledger churn. (2) The brief's premise
("on the flipped store") was not met because sub-step (a) delivered only the
enabling primitive, not item-1 (the flip). This is disclosed, not worked around.

**Recommendation for the coordinator.** (b) is not independently executable. Its
four categories become mechanical `ElemId`-match conversions **only after** the
(a)-continuation lands step-a's sequenced remainder — step 1 (`circuit.rs`
per-kind lists → `Vec<ElemId>`, ~427 consumer sites) through step 5 (access-layer
retype: `ElemStore` trait + `find_*` returns + the control_queue/ckt_tree/
gic_source siblings, which removes the `.to_ref()` bridges). Suggest folding
(b) into the flip WP (do A/D/generator-family/rich-B as the flip retypes each
site) rather than scheduling it as a standalone sub-step.

**Gate.** Toolchain guard (`cargo = .cargo\bin`, 186 `.pas`). `cargo fmt --all
--check` ok; `cargo clippy --workspace --all-targets -- -D warnings` ok;
`cargo test --workspace` (corpus gate inside, both channels capi_v0145 + r4133,
run solo) — exit 0, all workspace suites green (corpus gate
`corpus_gate_all_cases_match_engines` ok, 26-test dss-core integration binary).
Tree clean; corpus run-artifacts removed by exact name.
