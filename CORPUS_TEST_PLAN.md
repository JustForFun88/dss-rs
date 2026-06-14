# Plan: Live oracle-comparison test infrastructure over a vendored `electricdss-tst` corpus

> **Status: implemented** (this is the design record; see `STATUS.md` →
> "Live corpus oracle gate" for current state). All development steps in §6 are
> done, including the steps written here as "later, separate": the corpus is
> vendored, the three runtime `.inputs/electricdss-tst` gate references are
> repointed at `tests/corpus/electricdss-tst` (no test reads `.inputs`), and the
> oracle server hard-asserts **both** the dss-python 0.15.7 and engine 0.14.5
> pins. Manifests are **JSON** (the §2 zero-new-dep fallback), not TOML. The live
> gate runs in the `live-oracle` CI job and covers 17 solvable cases, including
> three control-diverse 24-step daily runs and the 8500-node master.

## Context

The golden gates today are "Python generates JSON from the pinned oracle → commit
under `tests/golden/` → Rust replays the same commands and compares". That works,
but it covers only a handful of hand-built scenarios and four feeder masters
(`tools/golden/cases.json`), and three gates still read circuits **at runtime**
from `.inputs/electricdss-tst` (`golden_feeders_controls.rs:33`,
`golden_ieee8500.rs:81`, `golden_checkpoints.rs:496`).

Two problems motivate this work:

1. **`.inputs/electricdss-tst` is temporary** — it may be deleted. Any test that
   reads it at runtime will break. We need an in-repo, self-contained copy.
2. **Coverage is tiny and static.** The OpenDSS test corpus has **915 `.dss`
   files** (≈238 runnable masters; the rest are `redirect`/`compile` include
   fragments). We exercise ~4. We want a harness that runs every *currently
   solvable* case against the pinned oracle **live** and compares the full
   electrical model, with an explicit manifest accounting for **every** file, so
   coverage can grow monotonically to 100% as the port matures.

**Intended outcome:** a vendored corpus + manifests (every `.dss` accounted for)
+ an opt-in live Rust-vs-oracle gate that, for each solvable case, runs both
engines and compares the whole observable model — node order, full V, full Y,
full element currents/powers, YPrim, injection vector, taps/regs/caps, monitors,
meters, generators — per step for multi-step cases, reusing the existing
comparators. As classes/commands/modes land, cases migrate from the skip
manifests into `solvable_now` until the corpus is fully covered.

### Key findings from exploration (these de-risk the plan)

- **Whole-tree copy is self-contained (verified).** `Dss::do_redirect`
  (`crates/dss-core/src/exec/mod.rs:2372-2453`) saves `current_dir`, sets it to
  the master file's parent for the duration of a `redirect`/`compile`, and
  restores it afterward. So **every relative path resolves relative to the master
  file**. A verbatim copy preserving the relative tree resolves all data
  dependencies internally — no absolute paths leak, and the copy can replace
  `.inputs` at runtime with zero rewrites.
- **All required observables already exist on `Dss`** (`exec/mod.rs`):
  `system_y_csc` (unfactored CSC), `element_yprim` (column-major flat),
  `node_injection_currents`, `snapshot_elements` (per-element currents/powers,
  all elements), `transformer_taps`, `regcontrol_tap_numbers`,
  `capacitor_states`, `meter_registers`, `meter_zone`, `monitor_view`,
  `generator_kw_kvar`, `load_alloc`, `total_power`, `losses`, `event_log`, plus
  `circuit()` for node order / voltages.
- **The per-step comparison already exists.** `golden_checkpoints.rs` +
  `harness/mod.rs` implement exactly the multi-step model comparison the request
  wants ("for multi-step scenarios compare all for each step, like
  golden_checkpoints.rs"): `compare_system_y` (union of patterns, top-N
  offenders, sparsity-change flag), `compare_yprim`, `compare_injection`,
  `compare_element`, `compare_discrete`, `assert_complex_close`,
  `assert_value_matches_tol`, `Tolerances`, `tol_for(kind)`.
- **The oracle capture already exists** in `tools/golden/gen_checkpoints.py`
  (`capture_system_y/yprim/fingerprint/injection/element/discrete`,
  `capture_*`), emitting JSON shapes the Rust structs already deserialize. These
  become the body of a live oracle runner — almost no new capture code.
- **Pin** (`tools/golden/PIN.txt`): python 3.12.4 / dss-python 0.15.7 /
  dss-python-backend 0.14.5. The live runner must hard-assert this.
- **Corpus size:** 126 MB excluding the corpus's own 52 MB `.git`. Heavy
  non-input payloads: xlsx 36 MB, csv 34 MB, dbl 4.9 MB, png/tif/jpg ≈3 MB. The
  corpus has its own `.gitignore` (`*.pickle.*`).

**So the genuinely new work is:** (a) a one-time corpus copy; (b) manifests + a
coverage test; (c) a **cross-language live transport** (Rust ↔ pinned
dss-python at test time); (d) a whole-circuit sweep on top of the existing
per-element comparators. The comparison math itself is reused.

---

## 1. Corpus migration

- **Destination:** `tests/corpus/electricdss-tst/` — a verbatim copy of the
  tree, **all files, relative paths preserved**. Siblings:
  `tests/corpus/manifests/` (the manifests), `tests/corpus/README.md`
  (provenance), `tests/corpus/SHA256SUMS` (integrity).
- **Only exclusions:** the corpus's own VCS metadata `.git/` (52 MB — not corpus
  content) and whatever its own `.gitignore` excludes (`*.pickle.*`). Everything
  else — `.dss`, `.csv`, `.txt`, `.dbl`, `.dat`, `.dsv`, `.sng`, … — is copied
  verbatim, because masters read these as data.
- **Reproducible vendoring script** `tools/corpus/vendor.py`: copy with the
  `.git` exclusion, write `README.md` recording the source
  `git -C .inputs/electricdss-tst rev-parse HEAD` + copy date + exclusions, and
  write `SHA256SUMS` over every copied file. Run once (and only to re-vendor); it
  is **not** on the test path.
- **Decouple existing gates (separate, later step).** After the copy is proven,
  repoint the three runtime `.inputs/electricdss-tst` references at
  `tests/corpus/electricdss-tst`, so the whole suite stops depending on
  `.inputs`. Kept separate to stay reviewable.

**Size trade-off (your call; default honors "all files").** ~126 MB into the
repo is the honest cost of a self-contained corpus. Options: (a) commit as-is
(simplest, fully faithful — **recommended default**); (b) git-LFS the >1 MB
binaries; (c) drop doc-only binaries (xlsx/png/tif/jpg/xls(m) ≈39 MB — no DSS
solve reads them). I'll proceed with (a) unless you say otherwise.

## 2. Manifest structure

**Guarantee:** every `.dss` under the copy is accounted for in exactly one
manifest, enforced by a coverage test (below). Only "runnable entry points"
(a `.dss` that creates/compiles a circuit and is meant to be invoked directly)
are executed; include fragments are exercised transitively via their master and
classified as such.

Files under `tests/corpus/manifests/` (one per status — greppable, PR-friendly):

- **`solvable_now`** — entry points both engines solve; **live-compared**. Each
  entry: `path`, optional `post` commands, `kind` (`micro`/`feeder`/`large` →
  tolerance class), optional `selected_elements` (for YPrim focus; the gate
  compares *all* elements' I/P regardless), optional step/mode metadata.
- **`skipped_unsupported`** — uses something the port lacks. Required tag:
  `unsupported_class=<X>` / `unsupported_command=<X>` / `unsupported_mode=<X>` /
  `missing_feature=<X>` + free-text note.
- **`skipped_oracle_issue`** — the pinned oracle itself errors / is unreliable /
  known dss_capi quirk. Tag + note (+ upstream ref where known).
- **`skipped_needs_investigation`** — fails/diverges, not yet diagnosed. Reason =
  current symptom. This is the working queue that drains over time.
- **`not_an_entry_point`** (required by the "account for every file" mandate,
  since most `.dss` are not masters) — subtag `include_fragment` /
  `helper` / `plot_or_export_only` (no solve).
- **`missing_dependency`** — references a data file absent from the copy (corpus
  integrity gap, kept visible rather than folded into `unsupported`).

**Format:** **TOML** array-of-tables (`solvable_now.toml`, …) with
`path`/`tag`/`note` + case metadata — comments allowed, clean diffs, pleasant to
curate. Cost: a `toml` dev-dependency (tiny). Zero-new-dep fallback: JSON with
the same fields. **Recommend TOML**; will switch to JSON if you prefer minimal
deps.

**Coverage test (`corpus_manifest.rs`, oracle-free, always-on).** Globs every
`.dss` under `tests/corpus/electricdss-tst/`, loads all manifests, asserts a
**bijection**: each file in exactly one manifest; each manifested path exists.
This is the "no silent omissions" guarantee and runs in the normal
`cargo test --workspace` gate (needs no oracle), so corpus and manifests can
never silently drift.

**Growth mechanism.** `tools/corpus/classify.py` trial-runs each unclassified
entry point on both engines and *proposes* a bucket (solvable / unsupported-by-
error-text / oracle-issue); a human curates the proposal. As the port grows,
files migrate `skipped_* → solvable_now`; the coverage test keeps the accounting
exact until `solvable_now ∪ not_an_entry_point` = the whole corpus.

## 3. Live Rust-vs-oracle comparison (no pre-generated goldens)

The Rust crates are `#![forbid(unsafe_code)]` and never link C, so Rust cannot
call dss_capi in-process. The oracle runs as a **separate pinned-Python
process** the harness drives at test time.

**Recommendation: a persistent oracle-server subprocess** — one
`python tools/oracle/oracle_server.py` per test-binary run, speaking
line-delimited JSON over stdin/stdout:

- On start: `import dss`, **assert the pin** (0.15.7 / backend 0.14.5; wrong
  version = hard failure, not skip).
- Loop: read one request `{case_path, post, n_steps, selected_elements}` →
  `DSS.ClearAll()` → `Compile "<copied case>"` → run steps → reply with the
  captured model (reusing `gen_checkpoints.py`'s `capture_*` functions, extended
  to capture *all* elements). Both engines `Compile` the **same copied file**, so
  inputs are byte-identical.
- **Robustness:** per-case wall-clock timeout; on a Python crash/timeout, Rust
  restarts the server and records that case as a failure — **never a silent
  skip**.

Alternatives noted: **subprocess-per-case** (simpler, fully isolated, pays
Python startup each time — fine at current scale) and **batch-to-tempdir** (still
live: computed at test time, nothing committed). I'll structure a single
`run_case(args)->dict` so the transport can switch without touching comparison
code; **persistent server is the default**.

**Gating (critical).** The live gate needs Python + the pinned oracle, which most
`cargo test --workspace` runs (and contributor CI) won't have. So it lives in its
own binary `crates/dss-core/tests/corpus_live.rs` and is **opt-in**: it runs only
when `DSS_LIVE_ORACLE=1` (and corpus + python present); otherwise it prints
`SKIPPED: live oracle not enabled` and returns success. This keeps the mandated
`cargo test --workspace` gate green everywhere; the maintainer (and a dedicated
pinned-oracle CI job) run the full comparison explicitly.

**No goldens written.** Nothing is added to `tests/golden/`. The committed
schema-1/2 goldens and their gates remain unchanged; this is parallel, additive
infrastructure.

## 4. Comparison scope (full model; Y / I / V full, no exceptions)

Per case (and **per step** for any multi-step/multi-point case — daily, duty,
yearly, fault sweeps — exactly as `golden_checkpoints.rs` does; a snapshot is the
1-step degenerate), reusing the existing comparators:

- **node order** — exact (asserted first so Y/YPrim indices align).
- **node voltages** — full complex vector, every node. **No exceptions.**
- **assembled system Y** — **full** entry-by-entry CSC over the pattern union,
  **every case, no exceptions** (`compare_system_y`). The fingerprint is kept as
  an additional cheap pre-check, but the full compare is always on for this gate.
- **element currents** — full complex, all terminals/conductors, **every
  element** (`snapshot_elements`), not just selected. **No exceptions.**
- **element powers & losses** — per element + circuit totals (`total_power`,
  `losses`).
- **selected YPrim blocks** — controls + a load/line sample by default,
  extendable to all elements (`element_yprim` / `compare_yprim`).
- **node injection / RHS vector** — full complex (`node_injection_currents` vs
  `YMatrix.getI()`).
- **transformer taps; RegControl tap numbers/state; capacitor states** —
  discrete, exact (`compare_discrete`).
- **monitor data** — every monitor's channels (f32 tol per existing note;
  wall-clock channels skipped).
- **EnergyMeter registers + zone data** — registers (energy tol) +
  branch/end/PCE counts (exact).
- **generator/load state** — per-element powers + injection cover the electrical
  state; plus `generator_kw_kvar`/`load_alloc` where relevant.
- **misc exposed** — iteration count, converged, dblHour, voltage bases.

**Mandate honored:** Y, currents, and voltages are compared in **full** for every
solvable case with **no field-specific exception**. Anything that genuinely
cannot be compared (one engine doesn't expose it, or it's inherently
non-deterministic such as wall-clock timing) must be **documented in
`tests/TOLERANCE_NOTES.md`** with its reason — never silently dropped.

## 5. Tolerance policy (reuse; no broad relaxation)

Reuse the established policy verbatim: `harness::Tolerances`, `tol_for(kind)`
(`golden_checkpoints.rs`), and `tests/TOLERANCE_NOTES.md`.

- micro/small deterministic: ~1e-9 rel (abs floors below physical significance);
- large feeder/full network: ~1e-6 rel (Y abs floor 1e-3 S; currents/powers abs
  floor 1e-4 for dead-end cancellation);
- EnergyMeter/registers: 1e-4 rel;
- **discrete exact** (node order, taps, tap numbers, cap states, nnz, sparsity
  pattern, converged).
- `kind` (from the manifest) selects the class. **No blanket relaxation;** any new
  field-specific exception must be added to `TOLERANCE_NOTES.md` with rationale.
  The Y/I/V full-compare mandate overrides any temptation to fingerprint-away a
  large feeder's Y here.

## 6. Development steps (small; smallest-first)

1. **Smallest useful first step — live round-trip pilot (no copy yet).** Build
   the oracle server + a Rust live-runner harness module; wire it to **one**
   already-available master (IEEE13, still from `.inputs`) and compare live
   (node order, full Y, full V, full I, injection, discrete). Proves the entire
   cross-language path with zero corpus-copy risk and ~no new comparison code.
2. **Vendor the corpus.** `tools/corpus/vendor.py` → `tests/corpus/electricdss-tst/`
   (+ `README.md`, `SHA256SUMS`); verify file count/paths vs source minus
   exclusions.
3. **Coverage test + seed manifests.** Implement the bijection coverage test.
   Auto-seed: `not_an_entry_point` for non-masters; all candidate entry points
   into `skipped_needs_investigation` so accounting is exact on day one. Gate
   green (oracle-free).
4. **Classification tool + first wins.** `tools/corpus/classify.py`; curate the
   obvious solvable masters (IEEE test cases, Example masters) into
   `solvable_now` with `kind` + `selected_elements`.
5. **Live execution layer.** `corpus_live.rs`: read `solvable_now`, run each on
   Rust + oracle server, gated by `DSS_LIVE_ORACLE=1`; per-case isolation,
   timeout, restart-on-crash; point Rust at the **copied** corpus.
6. **Comparison layers.** Wire the full §4 stack per case/step (mostly reuse) and
   add the all-elements I/P sweep (today's gate compares only *selected*
   elements).
7. **Failure diagnostics.** Reuse/extend the structured reporter
   (`[case … step … field … loc=(row,col=node) actual oracle |abs| |rel| tol]`,
   Y/YPrim top-N + sparsity-changed flag) with case-level context (master path,
   post commands, which engine errored).
8. **Coverage reporting.** Print a per-run summary (+ optional
   `tests/corpus/COVERAGE.md`): counts per manifest, % of entry points in
   `solvable_now`, the `needs_investigation` burn-down.
9. **Docs / STATUS.md.** Document the gate, the opt-in env var, the manifest
   workflow, vendoring/re-vendoring, and cross-link from `PORTING_PLAN.md` §4 +
   `STATUS.md`; record any "cannot compare" fields in `TOLERANCE_NOTES.md`.
10. **Repoint existing gates (separate).** Switch the three
    `.inputs/electricdss-tst` runtime refs to `tests/corpus/electricdss-tst`,
    decoupling the suite from `.inputs`.
11. **Full gate.** `cargo fmt --all --check` · `cargo clippy --workspace
    --all-targets -- -D warnings` · `cargo test --workspace` (live gate skipped
    without the env var) · then a manual `DSS_LIVE_ORACLE=1` run of `corpus_live`
    with the pinned oracle.

## Critical files

- **New:** `tools/corpus/vendor.py`, `tools/corpus/classify.py`,
  `tools/oracle/oracle_server.py`, `tests/corpus/electricdss-tst/**` (the copy),
  `tests/corpus/manifests/*.toml`, `tests/corpus/README.md`,
  `tests/corpus/SHA256SUMS`, `crates/dss-core/tests/corpus_live.rs`,
  `crates/dss-core/tests/corpus_manifest.rs`, and an oracle-transport module
  under `crates/dss-core/tests/harness/`.
- **Reused (no change):** `crates/dss-core/src/exec/mod.rs` accessors;
  `crates/dss-core/tests/harness/mod.rs`; the comparators in
  `golden_checkpoints.rs`; `tools/golden/gen_checkpoints.py` `capture_*`
  functions; `tools/golden/PIN.txt`.
- **Modified (later, separate step):** `golden_feeders_controls.rs:33`,
  `golden_ieee8500.rs:81`, `golden_checkpoints.rs:496` (repoint off `.inputs`);
  `STATUS.md`, `PORTING_PLAN.md`, `tests/TOLERANCE_NOTES.md`.

## Risks & trade-offs

- **Repo size (~126 MB).** Biggest trade-off; mitigations in §1 (commit-as-is /
  LFS / drop doc binaries).
- **Full-Y compare on large feeders, live, every step.** CPU/memory heavy
  (8500-node Y). Mitigation: compared in-memory and dropped (not stored); the
  fingerprint stays as a fast pre-check; the gate is opt-in/CI-only, off the
  inner-loop `cargo test`. Cap parallelism.
- **Cross-language fragility.** A case that hangs/crashes Python → per-case
  timeout + server restart + recorded failure (never silent skip).
- **Oracle pin drift.** Hard version assertion in the server; documented in
  PIN.txt.
- **Relative-redirect correctness.** Verified redirects resolve relative to the
  master dir, so the copy is self-contained; whole-tree copy preserves any `../`
  parents a master reaches into.
- **Manifest staleness.** Prevented by the always-on bijection coverage test.
- **Determinism.** Wall-clock channels / any RNG-seeded cases pinned or skipped
  explicitly and documented.

## Smallest useful first step

**Step 1:** stand up the oracle server + live-runner against a single
already-available master (IEEE13), comparing the full model live. It proves the
whole live cross-language path with zero corpus-copy risk and no new comparison
logic, and is the foundation everything else builds on.

## How it grows to 100%

The bijection coverage test makes the denominator explicit and the
omission-count always visible. Cases march `skipped_needs_investigation → (fix
port) → solvable_now`; `skipped_unsupported` drains as classes/commands/modes
land in later phases; `skipped_oracle_issue` is the small documented residue.
`COVERAGE.md` reports the burn-down each run until
`solvable_now ∪ not_an_entry_point` covers the entire copied corpus.

## Verification

- **Pilot (step 1):** `DSS_LIVE_ORACLE=1 cargo test -p dss-core --test corpus_live`
  with the pinned oracle — IEEE13 full-model live match passes; flip a value to
  confirm the diagnostics pinpoint the entry.
- **Coverage test (always-on):** `cargo test -p dss-core --test corpus_manifest`
  — bijection holds (add/remove a `.dss` → fails until manifested).
- **Full live run:** `DSS_LIVE_ORACLE=1 cargo test -p dss-core --test corpus_live`
  — every `solvable_now` case matches; `COVERAGE.md` summary printed.
- **Mandated gate (no oracle):** `cargo fmt --all --check` · `cargo clippy
  --workspace --all-targets -- -D warnings` · `cargo test --workspace` — green
  with the live gate auto-skipped.
