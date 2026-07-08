# dss-rs — Pascal → Rust port of DSS C-API (OpenDSS engine)

1:1 behavioral port of the Free Pascal "DSS C-API" engine (vendored at
`.inputs/dss_capi`) to pure safe Rust. **Read `PORTING_PLAN.md` first** — it is the
authoritative roadmap and encodes binding decisions:

- `#![forbid(unsafe_code)]` in every crate; no C bindings ever.
- Sparse solver is pure-Rust **faer**, wrapped in `dss-sparse` behind a
  KLUSolve-shaped API.
- No C-API export layer; the product is a Rust-native library (`dss-core`) + CLI.
- The numeric oracle is **dss-python pinned in `tools/golden/PIN.txt`**
  (0.15.7, backend = dss_capi 0.14.5 — the exact vendored Pascal source).
  Goldens live in `tests/golden/`; regenerate only manually, with the pinned
  versions, via `tools/golden/*.py`.
- A second, **opt-in** oracle channel drives official EPRI OpenDSS binaries
  (r3723 / r4088 / r4133) via the AltDSS Oddie bridge — `tools/opendss/`
  (separate venv, `PIN_OPENDSS.txt`). It never gates commits: divergence
  **reports** only (`DSS_LIVE_OPENDSS=<rev>` test, `ab_compare.py` A/B diff),
  for inventorying upstream changes ahead of porting newer engine behavior.
  See `tools/opendss/README.md`.
- Later phases may freely refactor earlier code; passing tests are the only contract.

## `TODO(compat)` convention (see PORTING_PLAN.md §4.1)

Every place where we deliberately reproduce an upstream inexactness or bug
(truncated `pi = 3.14159265359`, `rad→deg = 57.29577951`, FPC `Round`'s
integer-indefinite path, single-point stddev = the value itself, ...) **must** be
marked `TODO(compat):` with an explanation and the intended clean fix.

- Do **not** "fix" these during the port — goldens pin them; improved precision is
  indistinguishable from a porting bug in the gates.
- Do not use the `TODO(compat)` tag for anything else; it must stay greppable
  (`rg "TODO\(compat\)"`).
- They are all wiped out in one dedicated pass after the 1:1 port reaches final
  acceptance (PORTING_PLAN.md §6), regenerating goldens deliberately.

## Known upstream bugs (`investigations/`)

Five proven dss_capi/OpenDSS engine bugs, each with a full report in
`investigations/`. Check there before chasing a divergence in these areas. Rule:
a *deterministic, defined* upstream bug is reproduced 1:1 (`TODO(compat)` +
golden); UB or state-mutating-read bugs are NOT reproduced — document and gate
around them.

- **Export SeqCurrents `Iresidual`** — every terminal row prints *terminal 1*'s
  residual (missing `(j-1)*Ncond` offset). Reproduced (`TODO(compat)` in
  `report/export/seq_currents.rs`).
- **Multi-meter `Bus_Int_Duration`** — the `CalcReliabilityIndices` duration loop
  walks ALL circuit buses, indexing foreign section ids into this meter's
  `FeederSections`. In-range id → deterministic cross-zone overwrite, reproduced
  (`TODO(compat)` in `solution/meters/reliability.rs`, golden
  `export_busreliability_multimeter`); out-of-range id → OOB heap read, proven
  nondeterministic, not reproduced (safe `.get()` skip; nothing to pin).
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
  defined, not state-poisoning → reproduced (`TODO(compat)` in
  `exec/view.rs::snapshot_elements`); it is the only channel distinguishing
  Newton from the normal fixed-point on the `newton*` gates.

## Gate (must be green before any commit)

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Those three commands are the mandatory gate. The live oracle-comparison gate
(`crates/dss-core/tests/corpus_live.rs`) is now part of `cargo test` and runs
**unconditionally** (no `DSS_LIVE_ORACLE` env gate): it compiles + solves the
vendored corpus `tests/corpus/electricdss-tst` on both the Rust engine and the
pinned oracle and compares the full model live. The pinned dss-python oracle
(`tools/golden/PIN.txt`) must therefore be installed to run `cargo test` — without
it `corpus_live_solvable_cases_match_oracle` fails rather than skipping. New tests
read feeders from that vendored corpus, never from `.inputs/` at runtime.

**`TESTING.md`** is the map of the whole test infrastructure — the layers (unit
/ golden / live oracle / corpus hygiene / opt-in EPRI channel), the env-var
knobs, and the procedures (regenerate goldens, add a corpus deck, triage an
EPRI divergence). Read it to find where a given kind of test lives.

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
