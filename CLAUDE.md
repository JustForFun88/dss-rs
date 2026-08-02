# dss-rs — pure-Rust OpenDSS engine (ported 1:1 from the Pascal DSS C-API)

> **Ritual step 0 — source-integrity gate (every session, every plan, before anything
> else — even before the model-tier check).** The Pascal at `.inputs/dss_capi` (186
> `.pas` files) is the *spec*; oracle/live work also needs `.inputs/electricdss-tst`. If
> the folder you port FROM is missing or empty at **any** point — startup or mid-task —
> **STOP immediately**: make no edits, run no gate, and do **not** reconstruct, guess, or
> "port" a source you cannot read. Tell the user the vendored source is gone and must be
> re-vendored, then wait. No spec → nothing to port; inventing one from memory is silent,
> unverifiable fabrication — far worse than stopping. (Canonical placement: `PLAN_SEQUENCE.md`
> §Model-tier protocol, ahead of the tier/refuse check — binding for every plan.)

The **1:1 behavioral port** of the Free Pascal "DSS C-API" engine (vendored at
`.inputs/dss_capi`) to pure safe Rust is **finished** — final acceptance
(PORTING_PLAN.md §6) executed and referee-certified 2026-07-11. `PORTING_PLAN.md`
stays as the historical record of the binding porting decisions; the project's
direction is now **post-acceptance**: pure idiomatic Rust (`DE_PASCALIZE_PLAN.md`),
wasm-sandboxed user models (`WASM_USERMODELS_PLAN.md`), new methods and models,
and r4133-and-beyond upstream semantics (`UPGRADE_PLAN` line, `PLAN_SEQUENCE.md`
orders it all). Never argue from "we are a 1:1 port" — that stage is done; the
binding invariants that survive it are:

- `#![forbid(unsafe_code)]` in every **product** crate; no C bindings in the
  shipped engine ever. The **sole exception** is the test-only oracle bridge
  `crates/dss-epri` (`publish = false`): it drives the official EPRI
  `OpenDSSDirect.dll` via `libloading` under `#![deny(unsafe_op_in_unsafe_fn)]`,
  `#[cfg(windows)]`, and module-level `// SAFETY` docs, and never ships.
- Sparse solver is pure-Rust **faer**, wrapped in `dss-sparse` behind a
  KLUSolve-shaped API.
- No C-API export layer; the product is a Rust-native library (`dss-core`) + CLI.
- The numeric oracle is **dss-python pinned in `tools/golden/PIN.txt`**
  (0.15.7, backend = dss_capi 0.14.5 — the exact vendored Pascal source).
  Goldens live in `tests/golden/`; regenerate only manually, with the pinned
  versions, via `tools/golden/*.py`.
- A second **gating** oracle channel drives the official EPRI OpenDSS r4133
  binary (git-tracked at `tools/opendss/bin/r4133/`) through the in-house
  `crates/dss-epri` bridge (`epri-worker`). The unified corpus gate is
  manifest-driven: each case declares `engines: "capi_v0145" | "r4133" |
  "both"`, and measured upstream divergences are pinned per-case/per-channel in
  the gating divergence ledger `tests/corpus/ledger.json` (fail-on-stale — see
  TESTING.md). See `tools/opendss/README.md` for the artifact + bridge.
- Any phase may freely refactor earlier code; passing tests are the only contract.

## Policy (2026-08-02, user decision — binding): r4133 is the behavioral authority; bugs are NEVER reproduced

- The official EPRI OpenDSS **r4133** trunk (`.inputs/electricdss-code-r4133-trunk`)
  is the **primary behavioral reference**. The pinned dss_capi 0.14.5 is outdated:
  it remains a *numeric oracle only* (goldens, the `capi_v0145` corpus channel) and
  is **never** a behavioral authority over r4133.
- **Upstream bugs are never reproduced — in any lane, parity included.** The engine
  computes the correct value; each resulting divergence from an oracle channel is
  excluded field-by-field and pinned by its own expected-value test (lane.rs
  exclusions, `ledger.json` entries, golden-comparison fix-ups). Where r4133 does
  not share the bug, prefer gating the affected case on the `r4133` channel.
- What *may* still be reproduced is precision-compat only: truncated constants,
  FPC rounding/formatting, algorithm-identity numerics. The `oracle-parity` lane
  therefore shrinks to a *precision-compat* lane and is **scheduled for full
  teardown** — do not add new bug kernels to it, ever.
- The old doctrine "the parity lane must mirror dss_capi bugs" is **rescinded**;
  any statement to that effect elsewhere in the docs is historical.
- Status: the nine capi-only bugs (absent in r4133 — see
  `investigations/to_opendss/NOT-APPLICABLE-TO-R4133.md`) are removed from both
  lanes by the R4133-alignment pass (2026-08-02). The bug kernels shared with
  r4133 (Iresidual, Bus_Int_Duration, Newton stale Iterminal, Monitor
  BaseFrequency, single-point stddev, …) are queued for the same removal in a
  dedicated follow-up WP.

## `TODO(compat)` convention (see PORTING_PLAN.md §4.1)

Every place where we deliberately reproduce an upstream *precision* quirk
(truncated `pi = 3.14159265359`, `rad→deg = 57.29577951`, FPC `Round`'s
integer-indefinite path, ...) **must** be marked `TODO(compat):` with an
explanation and the intended clean fix.

- Since the 2026-08-02 policy the tag covers **precision/convention sites only**:
  logic bugs are never tagged — they are fixed outright in all lanes (see the
  policy section above).
- Do not use the `TODO(compat)` tag for anything else; it must stay greppable
  (`rg "TODO\(compat\)"`).
- The historical sweep was `DE_PASCALIZE_PLAN.md` Stage F (the `oracle-parity`
  feature split; the Part IV dual-kernel table was its closed inventory).
  Bug-reproducing kernels and sites are deleted by the R4133-alignment WPs;
  surviving precision sites are removed only with empirical proof (see the
  tolerance rules below).

## Known upstream bugs (`investigations/`)

Six proven dss_capi/OpenDSS engine bugs. The first five each have a full deep-dive
report in the (gitignored, local-only) `investigations/` folder — check there
before chasing a divergence in those areas. The sixth (Monitor BaseFrequency) is a
plain hardcoded-constant bug with a single fully-traced consumer, so it is
documented inline (a `compat` row + pin) rather than in a separate report.
**Rule (since 2026-08-02): no upstream bug is reproduced in ANY lane** — the
engine computes the correct value and every observable divergence from an oracle
channel is excluded field-by-field and pinned by an expected-value test. The
per-bug notes below describe the historical Stage-F lane-splits; their
parity-side reproductions are being dismantled (the nine capi-only bugs done
2026-08-02; the ones below — all shared with r4133 — queued for a dedicated WP).
English upstream-ready reports for all confirmed r4133 bugs live in
`investigations/to_opendss/`.

- **Export SeqCurrents `Iresidual`** — every terminal row prints *terminal 1*'s
  residual (missing `(j-1)*Ncond` offset). **Lane-split** since Stage F.3c
  (`compat::IRESIDUAL_FROM_TERMINAL_1` in `report/export/seq_currents.rs`):
  parity reproduces it (the goldens pin it), the default lane sums the row's own
  terminal.
- **Multi-meter `Bus_Int_Duration`** — the `CalcReliabilityIndices` duration loop
  walks ALL circuit buses, indexing foreign section ids into this meter's
  `FeederSections`. In-range id → deterministic cross-zone overwrite,
  **lane-split** since Stage F.3c
  (`compat::BUS_INT_DURATION_WALKS_ALL_BUSES` in
  `solution/meters/reliability.rs`; parity keeps the overwrite, golden
  `export_busreliability_multimeter`); out-of-range id → OOB heap read, proven
  nondeterministic, not reproduced in either lane (safe `.get()` skip; nothing
  to pin).
- **VSConverter `GetCurrents`** — self-aliased `MVMult` over `ComplexBuffer`:
  reported currents violate KCL and every read mutates state (can poison the next
  solve). Not reproduced — the port computes physically-correct currents, gated
  via oracle source currents + KCL (`exec/tests/vs_converter.rs`).
- **Harmonics `Powers`-after-`Currents`** — stale `Iterminal` cache makes
  Thevenin-DER (Generator/PVSystem/Storage) `Powers` order-dependent in harmonics
  mode. Not reproduced (Rust computes single-pass); golden capture reads `Powers`
  first (`tools/golden/gen_checkpoints.py::capture_element`).
- **Newton `Powers`/`Losses` stale `Iterminal`** — after `Set algorithm=Newton`,
  `DoNewtonSolution` stamps `Iterminal` at `NodeV_{n-1}` then does `NodeV -= dV`,
  so `Get_Powers`/`Get_Losses` (cache-aware) return a one-Newton-step-stale
  current while `Currents` recompute fresh (`S ≠ V·conj(I)`). Deterministic,
  defined, not state-poisoning → **lane-split** since DE_PASCALIZE Stage F.3j
  (`compat::POWERS_REUSE_STALE_NEWTON_ITERMINAL`, applied in
  `exec/view.rs::snapshot_elements`): the parity lane reproduces it and stays
  oracle-compared, the default lane recomputes all three reads at the converged
  `NodeV`. It was the only channel distinguishing Newton from the normal
  fixed-point on the `newton*` gates, so the default lane excludes those two
  decks' powers/losses (`tests/harness/lane.rs::LANE_SKIP_ELEM_POWERS`) and
  replaces the signal with the in-engine dispatch tripwire
  `exec::tests::newton::newton_dispatch_leaves_a_valid_but_stale_iterminal_cache`
  (plus the expected-value pin `newton_powers_are_the_lane_kernel`).
- **Monitor `BaseFrequency` 60.0** — `TMonitorObj.Create` hard-pins
  `Basefrequency := 60.0` (Monitor.pas:472 == r4133:552), overriding the base-class
  `BaseFrequency := ActiveCircuit.Fundamental` (CktElement.pas:203 — the last
  statement of `TDSSCktElement.Create`) that every other element inherits. Its one physical consumer is mode-4 flicker: it is passed as
  `fBase` into `FlickerMeter` (Monitor.pas:1657 → Pstcalc.pas:594), where `fBase =
  50.0` selects the IEC 61000-4-15 230V/50Hz lamp weighting coefficients vs the
  120V/60Hz set (Pstcalc.pas:609-626) — so a mode-4 monitor in a 50 Hz circuit
  computes Pst with the wrong (60 Hz) lamp curve unless the user sets `basefreq=50`.
  Deterministic, defined, not state-poisoning → **lane-split** since DE_PASCALIZE
  Stage F.3c (`compat::monitor_base_frequency`, applied in
  `exec/command.rs::create_object_no_edit`, pin `monitor_basefreq_is_the_lane_kernel`):
  the parity lane reproduces the 60.0 both gating oracles pin, the default lane
  inherits `Fundamental` (identical in a 60 Hz circuit, so no golden or corpus case
  moves).

## Gate (must be green before any commit)

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features dss-core/oracle-parity -- -D warnings
cargo test --workspace
cargo test --workspace --features dss-core/oracle-parity
```

Those five commands are the mandatory gate. Since DE_PASCALIZE **Stage F**
(the `oracle-parity` feature split) the engine ships in **two lanes**, and both
must be green:

- **parity** (`--features dss-core/oracle-parity`) — the *precision-compat*
  engine: byte goldens, checkpoint Y, corpus floors, exact iteration counts and
  discrete state. Its no-re-baseline discipline applies to **precision numerics
  only**; since the 2026-08-02 policy it does **not** mirror upstream bugs — bug
  fixes land in both lanes, with each observable divergence from the pinned
  oracles excluded field-by-field and pinned by an expected-value test. The lane
  is scheduled for full teardown.
- **default** (no features) — the idiomatic product: upstream bugs fixed,
  reports rendered natively. Same oracle floors on continuous quantities,
  discrete state still exact, iteration counts ±1, each deliberate divergence
  excluded field-by-field and pinned by its own expected-value test.

The lane policy is implemented once in `crates/dss-core/tests/harness/lane.rs`;
`#[cfg(feature = "oracle-parity")]` may appear only inside the three `compat`
modules and test code (gated by `oracle_parity_cfg_gate.rs`). Stage F
introduces **no** tolerance anywhere — the default-lane report policy is
`rel = abs = 0`.

A third, **on-demand** job compares the two lanes against each other:
`pwsh -File tools/lanes/lane_diff.ps1` builds both, dumps each one's solved-state
checkpoint stream (errors, convergence, iterations, node voltages, element
currents/powers/losses, the assembled Y — not meters, monitors, the event log or
report text, which the corpus gate compares live in both lanes) and diffs them
within the documented bounds (`crates/dss-core/examples/lane_dump.rs`;
`TESTING.md` §"The parity↔default differential gate").

The parity lane is byte-exact on the committed goldens and oracle-gated at the
calibrated `tests/TOLERANCE_NOTES.md` floors — it is **not** bitwise equal to
the oracle — so the honest chain is
`|default − oracle| ≤ |default − parity| + |parity − oracle|`. What makes this
job the transitive proof `default ≈ oracle` is the *measured* left term: the
2026-07-31 run came back `max |Δ| = 0` exactly on every gated kind, which makes
the default lane bit-identical to the parity lane and gives it precisely the
parity lane's oracle standing. Read the second term back in the moment Δ stops
being zero (expected at MULTITHREADING M3c and RESONANCE WP-R1).

It is not part of `cargo test` (two release builds, ~215 MB of dumps per lane):
run it whenever a `compat` kernel, a lane alias or the solver changes.

The unified live corpus gate
(`crates/dss-core/tests/corpus_gate.rs`, successor of `corpus_live.rs`) is part
of `cargo test` and runs **unconditionally**: one scheduler-driven test
(`corpus_gate_all_cases_match_engines`) walks all 520 manifest cases — solving
the 516 that are not abort-by-design
(vendored `tests/corpus/electricdss-tst` decks + the three synthetic families)
on the Rust engine and live-compares the full model against each case's gating
channel(s) — the pinned dss-python oracle (`capi_v0145`) and/or the EPRI r4133
DLL (`r4133`), partitioned by the divergence ledger `tests/corpus/ledger.json`
(every entry must be hit; stale entries fail the gate). Prerequisites: the
pinned dss-python (`tools/golden/PIN.txt`) must be installed — without it the
gate fails rather than skipping; the r4133 DLL is git-tracked and its
`epri-worker` bridge is built by `cargo test` itself (Windows-only —
`crates/dss-epri` is `#[cfg(windows)]`). New tests read feeders from the
vendored corpus, never from `.inputs/` at runtime.

**`TESTING.md`** is the map of the whole test infrastructure — the layers (unit
/ golden / unified corpus gate / corpus hygiene), the env-var knobs
(`DSS_GATE_*`, `DSS_ORACLE_*`, …), and the procedures (regenerate goldens, add
a corpus case, triage a divergence into the ledger, re-vendor the r4133
binary, run the seeding report). Read it to find where a given kind of test
lives.

## Git worktrees — safe deletion (`.inputs`/`.venv` junction hazard)

Parallel-agent worktrees live under `.claude/worktrees/`. Each one does **not**
copy the gitignored `.inputs/` (vendored `dss_capi` + `electricdss-tst`) or
`.venv/` — it holds Windows **directory junctions** pointing at main's real
copies. `git worktree remove` and *any* recursive delete (`rm -rf`,
`Remove-Item -Recurse`, `rmdir /s`) follow those junctions and delete the
**shared target's contents in main**. This has already wiped main's `.inputs`
once (recovery = a full re-vendor). Batching removals makes it worse: the loop
empties the shared target on the first worktree, then keeps going.

**Rule — never batch-remove worktrees; neutralize junctions first.** For each
worktree, drop the junction *reparse points only* (never descend into them),
then remove the worktree:

1. List reparse points without following them (recurse — a junction may sit
   below the top level, e.g. `tools/opendss/.venv`):
   `Get-ChildItem -LiteralPath <wt> -Recurse -Force | ? { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }`
2. Remove the **link only** — `$_.Delete()` on the `DirectoryInfo`
   (or `cmd /c rmdir "<path>"` **without** `/s`). Both drop the junction and
   leave the target untouched. Never use `Remove-Item -Recurse` / `rmdir /s` on
   a junction — they recurse through it into main.
   **PowerShell ONLY — never the Bash tool.** Under Git Bash, MSYS
   path-converts the `/c` in `cmd /c …`, cmd starts an interactive session,
   executes NOTHING, and exits 0 — the junction silently survives. This exact
   false-success wiped `.inputs` the second time (2026-07-19).
3. **Prove every link is gone before touching git:** `Test-Path '<wt>\.inputs'`
   and `Test-Path '<wt>\tools\opendss\.venv'` must both be **False**.
   Checking that main's target still has its items is NOT sufficient — it
   stays intact exactly while an un-dropped junction still exists, and dies
   on the next step.
4. Only now `git worktree remove --force <wt>`, then `git worktree prune`.

After each removal verify the shared target survived — `(gci .inputs -Force |
measure).Count` must be unchanged. Delete the merged per-agent branches
(`worktree-agent-*`, `wf_*`, `wp*`, `wpg*`) separately with `git branch -D`;
branch deletion never touches `.inputs`.

## Conventions

- **Module layout:** keep files focused; when a module grows large or mixes
  concerns, split it into a directory module (`foo/mod.rs` + concern submodules
  like `accessors`/`edit`/`solve`/`compute`) via `SPLITTING_RULES.md`'s byte-faithful
  protocol (no behavior change; the test suite is the contract). Unit tests stay
  inline as `#[cfg(test)]` modules, extracted to a sibling `tests.rs` only when the
  file is large; the `#[cfg(test)] mod tests;` declaration goes right after the
  module doc. Integration tests are thin drivers over the golden harness
  (`crates/dss-core/tests/harness/`).
- Pascal is the spec: port algorithms loop-for-loop where numerics matter, and cite
  the Pascal unit/identifier in the doc comment (`Pascal \`TcMatrix.Invert\``).
- 0-based indexing everywhere except the ground-node convention (`NodeRef == 0` =
  ground), converted only at parse/report boundaries.
- Case-insensitive identifiers via lowercase-normalized keys (THashList semantics).
- New behavior questions are settled empirically against the oracle (see
  `tools/golden/probe_val.py` for the pattern), not by guessing FPC semantics.
- **The dss_capi 0.15.x branch is NOT an unconditional authority — every
  "improvement" taken from it must be verified against the EPRI r4133 source
  AND physics (live probe where observable) before adoption.** "capi015 does
  it" alone is never sufficient evidence. Two of its changes were proven wrong
  on 2026-07-19: the RegControl idle no-load-zone test written as an OR (a
  tautology under the default ±100 kW band — EPRI r4088/r4133 use the correct
  bounded AND; adopted r4133) and the DynExp SolveEq "index-bug fix" (an early
  `Exit` that makes the evaluator a no-op — both gating oracles evaluate the
  full RHS; reverted, `DIVERGENCES.md` §D14). New `DIVERGENCES.md` decisions
  must cite r4133 evidence (source lines and/or an epri-worker probe), not
  just the 0.15.x side.
- **Never wave off a Rust↔oracle divergence as "conditioning / not a bug" without
  empirical proof.** That label has hidden real port bugs (e.g. a missing per-step
  state reset in `InvControl.update_inv_control` that latched `FFlagVWOperates`
  across time steps — see STATUS §WP7.5). A contractive iteration cannot amplify
  ~1e-8 rounding into a kW-scale gap, so such a gap is a bug until proven otherwise.
  Prove cause before concluding: (1) tighten the loop tolerance — if the gap
  collapses, the engines share the fixpoint; (2) run a controlled experiment that
  isolates the suspect (e.g. fixed vs adaptive factor); (3) dump the per-iteration
  trajectory + iteration count on both engines and find the first divergence. A gap
  that vanishes under tighter tolerance but leaves *different iteration counts* is a
  cross-step state-leak bug, not conditioning.
- **The converse needs the same rigor: when a gap genuinely IS a cancellation
  floor, prove it by decomposition, never by a tolerance sweep.** A residual that is
  the near-cancellation of two large summands (e.g. generator dynamics `dSpeed =
  (Pshaft + TracePower.re)/Mmass`, two ≈±2e9 W terms whose ≈31 W difference is
  1.5e-8 rel) has an *inherent* Rust↔oracle floor of ~1 f32-ulp (~9e-8) / ~6e-8 in
  f64 — faer-vs-KLU last-ulp rounding amplified by the cancellation. Prove it's the
  floor and not a bug by reading the **live f64** state (dss-python
  `ActiveCktElement.AllVariableValues` vs Rust element fields / `get_all_variables`
  — the f32 monitor channel hides it) and checking each summand matches the oracle
  to f64-ulp (Pshaft 1 ulp, TracePower 7 ulp) while only their difference is loose.
  Such a floor is NOT a `TODO(compat)` and must not be "fixed" — forcing the
  residual to 0 *diverges* from the oracle = a real port bug. (Documented at the
  `dSpeed` pins in `exec/tests/dynamics.rs`.)
- **Never loosen a test tolerance to make a failing oracle comparison pass — no
  fudging.** The tier floors in `tests/harness` (`Tolerances`/`tol_for`, see
  `tests/TOLERANCE_NOTES.md`) are calibrated to *proven* f64/f32/faer-vs-KLU
  reality. A Rust↔oracle gap above its floor is a porting **bug**: find and fix the
  root cause (per the two rules above), never widen the band to hide it. Tolerances
  change only with empirical proof of the floor — they tighten far more often than
  they loosen, and are *never* relaxed to mask a divergence.
- **Commit messages: keep them short.** A concise subject line plus, only if
  needed, 1–3 short bullets — not half a page. State *what changed and why* in a
  sentence or two; the detailed rationale belongs in `STATUS.md`/code comments, not
  the commit body. Don't restate the diff.

<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
| ------ | ---------- |
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
