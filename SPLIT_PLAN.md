# SPLIT_PLAN — module-splitting plan (read-organization only, zero behavior change)

> **Status:** ALL SPLITS EXECUTED & gate-green. Splits A/B (committed) plus the
> two §5 splits — `obj/dss_enum/registry.rs` → `registry/` and
> `solution/meters/zones.rs` → `zones/` — the latter two done **ahead of their
> triggers at explicit user request (2026-06-21)**. Pure code-organization
> refactor under **PORTING_PLAN §1.4** (*"later phases may freely refactor earlier
> code; the regression test suite is the only contract"*). No numeric behavior
> changes; no goldens regenerated; `TODO(compat)`/`NOT_PORTED` markers preserved
> **verbatim**.
>
> **Companion review:** the findings this plan acts on are in the split-review
> reconciled against `PHASE7_PLAN.md` / `PORTING_PLAN.md` / `STATUS.md`. Only the
> two **frontier-clear** targets are scheduled for execution now; the rest are
> deferred to a phase/WP boundary because Phase 7 (WP7.1) is actively editing
> their files.

---

## 0. Rules of engagement

1. **One split at a time.** Finish a split → verify byte-faithfulness (§2) →
   run the full gate (§3) → **stop**. Do not start the next split until the
   current one is green and verified.
2. **Never delete the original until verified.** Snapshot the pre-split content
   first (the original is the contract). Compare new-vs-old with the normalized
   diff in §2. Only after **zero logic lines removed** is confirmed do we remove
   the original. (This mirrors the prior `gen_dispatcher`/`class_props`/`sampling`
   splits.)
3. **The gate must be green before any commit** (`cargo fmt --all --check`;
   `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test
   --workspace`, incl. the unconditional live-oracle `corpus_live` gate).
4. **Commit only on explicit user request.** Each split is one self-contained
   commit (`refactor: split <file> into submodules`), gate-green, no goldens
   touched.
5. **Do not touch the WP7.1 frontier** (§5). Splitting a file an open WP is
   mid-edit invites merge friction and is out of scope here.
6. **No `TODO(compat)` "fixes".** They stay greppable (`rg "TODO\(compat\)"`) and
   byte-identical across the move. Same for `NOT_PORTED`.

---

## 2. Verification procedure — run AFTER every split, BEFORE deleting the original

The invariant: **the set of executable lines is identical before and after** —
the split only *relocates* code, never edits it. Two complementary checks.

### 2a. Normalized line-set diff (applies to every split)

Capture the original from git (it is the committed contract), normalize both
sides (strip indentation + visibility qualifiers; drop `use`/`mod`/comment/
attribute/blank lines), sort, and diff:

```bash
# --- in Git Bash, from repo root ---
norm() {
  sed -E 's/^[[:space:]]+//; s/^pub\(crate\) //; s/^pub\(super\) //; s/^pub\(in [^)]*\) //; s/^pub //' \
  | grep -vE '^(use |mod |pub use |pub mod |#\[|#!\[|//|/\*|\*|$)' \
  | sort
}

OLD=crates/dss-core/src/exec/tests.rs                 # the file being split (path at HEAD)
NEWGLOB='crates/dss-core/src/exec/tests/*.rs'         # the new submodules

git show "HEAD:$OLD" | norm > /tmp/split_old.txt
cat $NEWGLOB        | norm > /tmp/split_new.txt

echo "### REMOVED (in OLD, not in NEW) — MUST be empty:"
comm -23 /tmp/split_old.txt /tmp/split_new.txt
echo "### ADDED (in NEW, not in OLD) — only benign scaffolding allowed:"
comm -13 /tmp/split_old.txt /tmp/split_new.txt
```

- **REMOVED must be empty.** A single removed logic line = something was lost →
  stop and fix before deleting the original.
- **ADDED is allowed only** for benign split scaffolding the `norm` filter does
  not already strip: extra `impl X {` / closing `}` from splitting one `impl`
  across files, and the occasional multi-line `use {…}`/signature reformatted to
  a single line. Anything semantic in ADDED = a real change → stop.

### 2b. Test-identity check (for `exec/tests.rs` specifically)

Tests give a stronger, simpler invariant than the line diff — the **set of test
names** and the **count cargo runs** must not change:

```bash
# test-fn name set unchanged
git show "HEAD:crates/dss-core/src/exec/tests.rs" \
  | grep -oE '^\s*fn [a-z_0-9]+' | sed 's/^[[:space:]]*//' | sort > /tmp/tests_old.txt
cat crates/dss-core/src/exec/tests/*.rs \
  | grep -oE '^\s*fn [a-z_0-9]+' | sed 's/^[[:space:]]*//' | sort > /tmp/tests_new.txt
diff /tmp/tests_old.txt /tmp/tests_new.txt && echo "OK: identical fn set"

# count unchanged (record N before, assert N after)
cargo test -p dss-core --lib exec::tests 2>&1 | grep 'test result'
```

`diff` must show no differences (every `#[test]` **and** every helper `fn`
accounted for, in exactly one new file), and the `test result: ok. N passed`
count must equal the pre-split count.

---

## 3. The gate (after each split)

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

All three green. `cargo test` includes the live oracle gate
(`corpus_live_solvable_cases_match_oracle`) — a pure file-move cannot affect it,
which is the point: if it moves, something leaked.

---

## 4. Scheduled splits (frontier-clear)

### Split A — `exec/tests.rs` (2259 lines, 87 tests) → `exec/tests/` themed dir

**Why:** by far the largest file in the repo; 87 flat `#[test]`s in one module,
no internal grouping. Pure mechanical re-home; biggest readability win.

**Mechanics — file-module → directory-module conversion:**

- `exec/mod.rs` line 18–19 is `#[cfg(test)] mod tests;` — leave it **unchanged**.
  A directory module resolves the same way, and the `#[cfg(test)]` on that one
  declaration gates the **whole** `tests/` subtree, so no submodule needs its own
  `#[cfg(test)]`.
- The old file began with `use super::*;` where `super == exec`. In the new
  submodules `super == tests`, so that import must become **`use crate::exec::*;`**
  (the documented file→dir `super`-remap rule). Add per-file only the extra
  imports a given group actually uses (e.g. `num_complex::Complex64` in the
  sensor group, `crate::circuit::bus::Bus` in reliability).
- **Shared helpers** `query` (55 uses), `dss_with_circuit` (8), `query_f64` (1
  use, at old line 504) go to `tests/common.rs` as **`pub(super)` fns**; each
  themed file that needs them does `use super::common::*;`. **Group-local
  helpers stay private** in their own file (`reg_two_bus`, `gen_disp_two_bus`,
  `micro_zone_script`, `daily_meter_case`, `meter_case`, `reliability_feeder`,
  `allocation_feeder`, `sample_feeder`, the `bus_*`/`accum_*`/`branch_*`
  reliability accessors, `close_rel`, `cclose`, `approx_meter`, …).
- `tests/mod.rs` holds the module doc + `mod common;` + the themed `mod` decls
  (alphabetical). It contains **no tests itself**.

**Proposed theming** (every test lands in exactly one file; boundaries finalized
at execution and proven by §2b). Target ~12 files, each cohesive and <~350 lines:

| File | Theme | Representative tests (old line) |
|---|---|---|
| `common.rs` | shared helpers | `query` (3), `dss_with_circuit` (10), `query_f64` (282) |
| `lifecycle.rs` | New/Edit/`~`/MakeLike/Clear/Set/Get/parse/circuit-gate | `new_and_query_defaults` (18) … `new_circuit_creates_default_source` (161), `get_returns_set_values` (738) |
| `solve.rs` | snapshot solve + generators + node order + adjacency | `two_bus_snapshot_solves` (174), `generator_model1/3_*` (202/242), `reg_control_does_not_change_node_order` (654), `bus_adjacency_lists_bucket_elements` (698) |
| `monitors.rs` | Monitor modes/headers | `monitor_mode0_mode1_daily` (291), `monitor_mode5_solution_vars` (357), `monitor_header_modifiers` (383), `monitor_mode2_tap_and_class_check` (453) |
| `line_fetch.rs` | Line/LineCode/Geometry fetch + shape refs | `line_fetches_*` (489/539/555/577), `load_and_vsource_resolve_shape_refs` (511), `line_geometry_*` (594/621) |
| `autoadd.rs` | AutoAdd options | `autoadd_*` (750/761/835/852), `ueregs_nonnumeric_*` (793), `addtype_unknown_*` (819) |
| `reduce.rs` | Reduce options/command | `reduce_*` (871/898/931/948/973/1003) |
| `controls.rs` | control loop, RegControl, GenDispatcher, StorageController | `control_loop_*` (1034), `max_control_iterations_*` (1057), `gendispatcher_*` (1106–1184), `storagecontroller_skeleton_*` (1204) |
| `time_series.rs` | time options, daily mode, bus coords | `time_options_*` (1226), `daily_mode_*` (1260), `bus_coords_*` (1291) |
| `energymeter_zones.rs` | EnergyMeter zone build | `energymeter_zone_radial` (1332) … `energymeter_no_element_no_revalidation` (1542) |
| `energymeter_registers.rs` | registers, daily, reset | `energymeter_daily_registers_*` (1613/1640), `energymeter_*_registers`/loss/een_ue (1707–1824), `energymeter_reset_registers` (1661), `reset_command_resets_controls` (1825) |
| `reliability.rs` | RelCalc / SAIFI–SAIDI | `relcalc_*` (2009/2028/2052/2072/2092/2112) |
| `allocation.rs` | load allocation + sensor sampling | `allocateloads_*` (2159–2321), `sensor_requires_element` (2239), `sensor_take_sample_wye/delta` (2370/2393) |

**Verify:** §2b (test-fn set diff + count) **and** §2a (normalized line diff).
Expect REMOVED empty; ADDED only the per-file `use` lines + the helper
signatures relocated (already filtered) — effectively empty.

**Known gotchas for Split A:**
- *G1 (super remap):* `use super::*;` → `use crate::exec::*;` in every themed
  file. The single most common mistake.
- *G2 (helper visibility):* shared helpers `pub(super)` in `common.rs`; siblings
  can't see another sibling's *private* fn, so the 3 cross-group helpers must be
  `pub(super)` and the group-local ones must stay where they're used.
- *G3 (cfg gating):* do **not** add `#[cfg(test)]` to the submodules — the parent
  `mod tests;` already carries it; double-gating is harmless but noise, and
  forgetting that the parent gates it is what tempts the error.
- *Note:* `exec/tests.rs` is **not frozen** — WP7.1 added
  `line_geometry_specified_resolves_and_solves` (old line 621). After the split,
  new Phase-7 exec tests have an obvious home (`line_fetch.rs` or a future
  themed file); this is a feature of the split, not a hazard.

---

### Split B — `elements/general/xy_curve/mod.rs` (343 lines) → add `accessors.rs`

**Why:** convention parity. `xy_curve` is the largest unsplit `general` element,
yet keeps `impl DssObject for XyCurveObj` (lines 212–367, ~155 lines) inline,
while comparable split `general` elements (`load_shape`, `line_code`, `xfmr_code`)
moved their trait impls into `accessors.rs`. Phase-5 element, **untouched by
Phase 7** — safe.

**Mechanics:**

- **Keep in `mod.rs`:** the module doc, `#[cfg(test)] mod tests;` (line 14–15),
  the imports the struct/macro/inherent-impl need (incl.
  `use crate::obj::base::DssObjData;` — but **drop `DssObject` from mod.rs's
  import** since it moves with the trait impl; keep `DssObjData` for the struct
  field + the `data()` return types stay in accessors though — see below), the
  `define_properties! { … }` macro invocation (line 21–53, which generates the
  `prop` module + `class_props` fn), `pub struct XyCurveObj` (54), and the
  inherent `impl XyCurveObj` (77–211: `new`, `n`, `interpolate`, `get_y_value`,
  `get_x_value`, `set_x`, `set_y`, `get_x`, `get_y`, `realloc`).
- **Move to `accessors.rs`:** the entire `impl DssObject for XyCurveObj` block
  (212–367). Its imports: `use crate::obj::base::{DssObjData, DssObject};` and
  `use super::XyCurveObj;` (plus whatever the bodies reference —
  `EnumRegistry`/prop types if used). `DssObjData` is needed there for the
  `data()`/`data_mut()` return types; `mod.rs` also still needs `DssObjData` for
  the struct field, so the import appears in both files (not shared).
- Add `mod accessors;` to `mod.rs` (alphabetically, before `mod tests;` is fine;
  follow the existing element convention: plain `mod` decls then the
  `#[cfg(test)] mod tests;`).

**Known gotcha for Split B — the trait-in-scope break (the `gen_dispatcher`
bug, will recur here):**

`xy_curve/tests.rs` opens with `use super::*;` and calls **`obj.end_edit()`**
(line 24) and **`obj.make_like(&base)`** (line 147) — both `DssObject` trait
methods. Today they resolve because `mod.rs` imports `DssObject` and `use
super::*` re-exports it into the test module. Once `impl DssObject` **and its
`use …DssObject`** move to `accessors.rs`, `mod.rs` no longer has `DssObject` in
scope, so `use super::*` stops providing it and the tests fail to compile
(E0599: method not found). **Fix:** add `use crate::obj::base::DssObject;`
immediately after `use super::*;` in `xy_curve/tests.rs`. (The macro-generated
`class_props`/`prop` items stay in `mod.rs`, so `tests.rs`'s `use super::*` keeps
resolving `class_props(&enums)` unchanged.)

**Verify:** §2a normalized diff with `OLD=…/xy_curve/mod.rs` and
`NEWGLOB='…/xy_curve/{mod,accessors}.rs'`. Expect REMOVED empty; ADDED only the
relocated `impl DssObject for XyCurveObj {` line + the `accessors.rs` `use`
header (filtered) + the one added `use` in `tests.rs` (a `use` line, filtered).
Net ADDED ≈ empty.

---

## 5. Formerly-deferred splits — EXECUTED 2026-06-21 (ahead of trigger, at user request)

Both were originally deferred to a phase/WP boundary (Phase 7 is mid-flight). The
user asked to do them now; each was executed under the **same §2/§3 procedure** and
is gate-green (full three-command gate + live-oracle corpus, dss-core lib 376).

| Target (was) | Now | Result |
|---|---|---|
| `obj/dss_enum/registry.rs` (488 ln) | `obj/dss_enum/registry/{mod,pd,pc,control,general,solution}.rs` | `new()` threads one `push` closure through five by-domain `register()` helpers, each returning the `EnumId`s it allocated. Ids are **local** (push order irrelevant — `EnumId` is an opaque handle, grep-confirmed), so the regroup is behavior-neutral. §2a REMOVED empty; the 32 `DssEnum::new` calls, the 18 special-flag mutations, and all 190 enum/variant string literals are byte-identical; ADDED is only struct/fn/field scaffolding. |
| `solution/meters/zones.rs` (585 ln) | `solution/meters/zones/{mod,build,flags}.rs` | Pure relocation (mirrors `sampling/`): `mod` keeps `DoResetMeterZones`/`ResetMeterZonesAll`, `flags` the `SetHas{Meter,Sensor}Flag` passes, `build` the `MakeMeterZoneLists` walk + predicates + `TotalUpDownstreamCustomers`. `downcast_meter` reached via `super::super` (the `sampling/allocate.rs` precedent). §2a REMOVED **and** ADDED both empty. The `is_zone_pce` `TODO(WP7)` (now in `zones/build.rs`) is preserved **verbatim** for WP7.2/7.3. |

The earlier `registry.rs` "intra-file extraction" sketch became a directory split
(`registry/`) — the cleaner outcome; `zones.rs` was the planned directory conversion.

---

## 6. Do NOT touch (active WP7.1 frontier)

These are mid-edit in the in-progress Phase-7 line-constants work
(`STATUS.md §1e`); splitting them now would collide with live changes:

- `elements/general/conductor_data/mod.rs` (358) — step 2a/2c-ii landed it; the
  units-converting read getters are an **explicitly deferred-tracked** addition
  for "step 3". Off-limits until WP7.1 closes.
- `elements/pd/line/` — step 3a/3b actively edit the `geometry=`/`spacing=` path.
- `elements/general/line_geometry/` — built across steps 2c-i/2c-ii/3a; already
  split (`accessors`/`edit`/`matrix`/`tests`).

---

## 7. Explicitly out of scope (keep as-is — not drift)

- **Inline `#[cfg(test)]` test modules:** inline tests are the **documented
  convention** (CLAUDE.md *"Unit tests inline as `#[cfg(test)]` modules"*;
  PORTING_PLAN §4); the separate `tests.rs` files are a **size-driven**
  accommodation, **not** a target everything must converge to. The convention
  stands — but **applied 2026-06-21** (user request) to the five **large**
  single-file modules (≥~430 ln), the same threshold that drove the element-dir
  `tests.rs`: `dss-parser/src/rpn.rs` (596), `obj/base.rs` (477),
  `circuit/ckt_tree.rs` (475), `dss-sparse/src/lib.rs` (465) and
  `solution/control_queue.rs` (434) had their inline `mod tests {…}` block
  extracted to a sibling `tests.rs` (each `foo.rs` → `foo/{mod,tests}.rs`; the
  crate-root `lib.rs` → `lib.rs` + `src/tests.rs`). Same-depth move, so the test
  block's `use super::*;` is unchanged and byte-faithful (§2a REMOVED = only the
  wrapper `}`; impl + test bodies byte-identical). The **small** single-file
  modules (`winding.rs` 118, `event_log.rs` 112, `vars.rs` 183, `util.rs` 351, …)
  **keep inline tests** — extraction there is pure overhead.
- **`energymeter`/`monitor` having no `tests.rs`:** intentional — tested via the
  golden/corpus gates + `exec/tests` (confirmed by `STATUS.md`/`PHASE7_PLAN.md`).
- **Tier-2 cohesive files** (`circuit/circuit.rs`, `obj/base.rs`,
  `elements/ckt.rs`, `exec/command.rs`, `exec/solve.rs`,
  `circuit/ckt_tree.rs`): single-responsibility, read fine; splitting buys
  little. Keep.

---

## 8. Execution order & checkpoints

1. **Split A** (`exec/tests.rs` → `exec/tests/`): snapshot original → build
   themed dir → remove original → **§2b + §2a verify** → **§3 gate** → stop.
   *(Optional commit on request.)*
2. **Split B** (`xy_curve` + `accessors.rs`): snapshot → move `impl DssObject` →
   fix `tests.rs` `DssObject` import → **§2a verify** → **§3 gate** → stop.
   *(Optional commit on request.)*
3. **Formerly-deferred** (§5): **executed 2026-06-21** at user request, ahead of
   their triggers — `registry.rs` → `registry/` and `zones.rs` → `zones/`, both
   §2/§3-verified and gate-green. (Originally: only when their triggers fire.)

Each numbered step is a stop-and-confirm checkpoint per the project's WP cadence.
