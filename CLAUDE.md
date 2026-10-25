# dss-rs — pure-Rust OpenDSS engine

> **Ritual step 0 — source-integrity gate (every session, every plan, before anything
> else).** `.inputs/dss_capi` (186 `.pas` files) must be present and non-empty, and
> oracle or live work also needs `.inputs/electricdss-tst`. If a folder you need is
> missing or empty at **any** point, startup or mid-task, **STOP**: make no edits, run
> no gate, and never reconstruct or guess a source you cannot read. Tell the user it
> must be re-vendored, then wait.

The 1:1 port of the Pascal DSS C-API to safe Rust is finished (accepted
2026-07-11). The direction now is idiomatic Rust, wasm-sandboxed user models, new
methods and models, and behaviour that is right by physics. Never argue from "we
are a 1:1 port". The invariants that bind:

- `#![forbid(unsafe_code)]` in every **product** crate, no C bindings in the
  shipped engine. The one exception is the test-only oracle bridge
  `crates/dss-epri` (`publish = false`, `#[cfg(windows)]`,
  `#![deny(unsafe_op_in_unsafe_fn)]`, module-level `// SAFETY` docs): it drives the
  official EPRI `OpenDSSDirect.dll` and never ships.
- The sparse solver is pure-Rust **faer**, wrapped in `dss-sparse`.
- The product is a Rust library (`dss-core`) plus a CLI, with no C-API export layer.
- Two oracle channels gate the corpus. `capi_v0145` is the dss-python pinned in
  `tools/golden/PIN.txt` (dss_capi 0.14.5), which also produced the goldens in
  `tests/golden/` (regenerated only by hand, with the pinned versions, through
  `tools/golden/*.py`). `r4133` is the EPRI OpenDSS r4133 binary, git-tracked at
  `tools/opendss/bin/r4133/` and driven through `epri-worker`
  (`tools/opendss/README.md`). Each corpus case declares its channels, and every
  measured divergence is pinned in `tests/corpus/ledger.json` (fail-on-stale).
- Any code may be refactored freely: passing tests are the only contract.

## Policy: physics is the only authority, bugs are never reproduced (user decisions 2026-08-02 and 2026-10-03)

- **No engine is an authority.** A behaviour question is decided by physics and
  power engineering, a derivation plus a measured balance: conservation of complex
  power, the circuit laws, what the quantity means to an engineer. When r4133 and
  dss_capi agree on a physically wrong number, both are wrong and the engine
  computes the right one.
- **Only where physics is indifferent** (the DSS script language, property and
  command semantics, report layout) r4133 is the reference, and only when its
  behaviour is logical and not a bug. "r4133 does it" alone is never evidence.
- dss_capi 0.14.5 is outdated: a numeric oracle only, never above r4133 or physics.
  A change taken from the dss_capi 0.15.x branch needs the same proof as any other.
- A doubtful case goes to the user with the physical argument.
- **Upstream bugs are never reproduced, in any lane.** The engine computes the
  correct value. Each divergence from an oracle channel is excluded field by field
  (`lane.rs` exclusions, `ledger.json` entries, golden fix-ups) and pinned by an
  expected-value test. Where r4133 does not share the bug, gate the case on the
  `r4133` channel. Every fixed OpenDSS bug gets an upstream report in
  `investigations/to_opendss/`.
- Only precision quirks may be reproduced (truncated constants such as
  `pi = 3.14159265359`, FPC rounding and formatting, algorithm-identity numerics),
  each marked `TODO(compat):` with the intended clean fix. The tag is used for
  nothing else and stays greppable (`rg "TODO\(compat\)"`). A precision site leaves
  only with empirical proof. The `oracle-parity` lane that holds them is scheduled
  for teardown and takes no new kernel.

## Launching agents and workflows (user rule 2026-10-03)

- **Check the machine before every launch** of an agent or a workflow, also when
  the user has just asked for the work: the workflows and building lanes in flight,
  the gate queue (`tools/gate/README.md`: waiting tickets mean the machine is full),
  CPU load, free memory and the running `cargo`/`rustc` processes.
- **Resources free: launch without asking.**
- **Resources short: ask first.** Tell the user what already runs and that one more
  will slow everything down, and ask whether to start it now.
- **Ceiling:** about three workflows and three to four building lanes at once. A
  freed lane is not a reason to launch.
- After stopping a workflow, check that its scripts no longer queue builds.

## Gate (green before any commit)

```
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features dss-core/oracle-parity -- -D warnings
cargo nextest run --workspace
cargo nextest run --workspace --features dss-core/oracle-parity
cargo test --workspace --doc
cargo test --workspace --doc --features dss-core/oracle-parity
```

Commands 4-5 run the pinned `cargo-nextest` of `.config/nextest.toml` (each test in
its own process, no retries) with `DSS_ORACLE_TIMEOUT_SECS=600`, and commands 6-7
run the doctests. On a shared machine every command goes through the queue,
`python tools/gate/gatelock.py --label "<who>" -- cargo <args>`.

A landing reuses the gate its tree already passed (user decision 2026-10-03). When
the lane head passed the seven commands, `update` has not moved past the lane's
base and `git diff --stat <lane head> <landing commit>` shows only `STATUS.md` and
the record, run `cargo fmt --all --check` and the doc rails
(`cargo nextest run -p dss-core --test oracle_parity_cfg_gate`), then push.
Otherwise the landing runs the full gate.

The engine ships in two lanes and both must be green:

- **parity** (`--features dss-core/oracle-parity`): the precision-compat engine,
  byte-exact on the goldens, exact iteration counts and discrete state.
- **default** (no features): the product. The same oracle floors on continuous
  quantities, discrete state exact, iteration counts within ±1.

The lane policy lives in `crates/dss-test-harness/src/harness/lane.rs`.
`#[cfg(feature = "oracle-parity")]` appears only in the three `compat` modules and
in test code (`oracle_parity_cfg_gate.rs`). The default-lane report policy has no
tolerance (`rel = abs = 0`). `pwsh -File tools/lanes/lane_diff.ps1` compares the
two lanes' solved states on demand (`TESTING.md` §"The parity↔default differential
gate"): run it whenever a `compat` kernel, a lane alias or the solver changes.

The live corpus gate (`crates/dss-core/tests/corpus_gate.rs`,
`corpus_gate_all_cases_match_engines`) runs inside commands 4-5 over every
manifest case against its channels, partitioned by `tests/corpus/ledger.json`,
where an entry nothing hits fails the gate. It needs the pinned dss-python
installed and fails without it. New tests read feeders from the vendored corpus,
never from `.inputs/` at runtime. `TESTING.md` maps the test infrastructure, its
knobs and its procedures.

## Git worktrees — safe deletion (`.inputs`/`.venv` junction hazard)

Worktrees under `.claude/worktrees/` hold Windows **directory junctions** to main's
`.inputs/` and `.venv/`. `git worktree remove` and any recursive delete (`rm -rf`,
`Remove-Item -Recurse`, `rmdir /s`) follow them and delete main's copies. This has
wiped main's `.inputs` twice. **Never batch-remove worktrees.** For each one, in
**PowerShell only** (under Git Bash `cmd /c rmdir` silently does nothing and exits 0):

1. List the reparse points without following them:
   `Get-ChildItem -LiteralPath <wt> -Recurse -Force | ? { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }`
2. Drop the **link only**: `$_.Delete()` on the `DirectoryInfo`, or
   `cmd /c rmdir "<path>"` without `/s`.
3. Prove the links are gone: `Test-Path '<wt>\.inputs'` and
   `Test-Path '<wt>\tools\opendss\.venv'` must both be **False**.
4. Only then `git worktree remove --force <wt>`, then `git worktree prune`.

After each removal `(gci .inputs -Force | measure).Count` must be unchanged. Delete
merged branches separately with `git branch -D`.

## Conventions

- **Module layout:** keep files focused. A module that grows large or mixes
  concerns becomes a directory module (`foo/mod.rs` plus concern submodules) by
  `SPLITTING_RULES.md`, with no behaviour change. Unit tests stay inline as
  `#[cfg(test)]` modules, in a sibling `tests.rs` only when the file is large, the
  `#[cfg(test)] mod tests;` line right after the module doc. Integration tests are
  thin drivers over the test-only crate `dss-test-harness`
  (`use dss_test_harness::harness;`).
- 0-based indexing everywhere except the ground node (`NodeRef == 0`), converted
  only at parse and report boundaries. Identifiers are case-insensitive through
  lowercase keys.
- Behaviour questions are settled by measurement (`tools/golden/probe_val.py`,
  epri-worker), never by guessing.
- **A Rust↔oracle gap is a bug until proven otherwise.** "Conditioning" needs
  proof: tighten the loop tolerance (a gap that collapses means a shared fixpoint),
  isolate the suspect in a controlled experiment, dump both trajectories and find
  the first divergence. A gap that vanishes under a tighter tolerance but leaves
  different iteration counts is a state leak across steps.
- **A cancellation floor is proven by decomposition, never by a tolerance sweep.**
  Read the live f64 state on both engines and show that each summand matches to the
  last ulps while only their difference is loose (the `dSpeed` pins in
  `exec/tests/dynamics.rs`). Such a floor is not a precision quirk to mark and is
  not "fixed".
- **Never loosen a tolerance to make an oracle comparison pass.** The floors of
  `crates/dss-test-harness/src/harness/mod.rs::tol_for` (`tests/TOLERANCE_NOTES.md`)
  change only with empirical proof of the floor.

### Commits, records and citations

- **A commit subject names the change in the code.** No plan, step, wave, lane,
  ruling or work-package id in a subject or body, and no path into `docs/`, `tmp/`,
  `investigations/`, `.inputs/` or a plan file. Write
  `SeqCurrents: exclude and pin the negative-rating divergence`, not
  `RF-D03-01 settlement + record`. A subject plus at most three short bullets. A
  lane lands as one squash commit, and its `STATUS.md` or record edit rides in it.
- **Plans and queues are not committed.** `RULINGS_QUEUE.md`, `MAKEPOSSEQ_PLAN.md`,
  `RETRO_FIXES_PLAN.md` and the step plans are the coordinator's working files,
  kept in the checkout outside git.
- **No OpenDSS or Pascal source is cited in committed text**: not in code, tests,
  records, `TESTING.md`, `ledger.json` or this file. No `.pas` name, no Pascal
  line, no `.inputs/` path. A behaviour choice rests on physics (a derivation and a
  measured balance) or on a measurement named by its recipe and its numbers
  ("measured 2026-10-17 with the epri-worker on the r4133 DLL"). The upstream
  reports in `investigations/to_opendss/` (gitignored, written for the OpenDSS
  developers) are the one place that cites the source. Existing citations leave
  when their text is next edited.
- **Cite code by name, never by line number.** No `file.rs:N` (nor `:A-B`, `.py:N`,
  `.md:N`) for a file of this repository: from a doc comment an intra-doc link
  (`` [`tol_for`] ``) so `cargo doc` checks it, elsewhere the path plus the item
  (`crates/dss-test-harness/src/harness/mod.rs::tol_for`), and a document passage
  by its heading. A record may use the commit-pinned form `<sha>:<path>:<N>` for a
  statement about the past.
- **Comments are short and say what the code does now.** No history, no
  `<sha>:path:N`. A behaviour choice that needs a justification gets one sentence
  and the name of the test that pins it. The measurement behind it lives in the
  record, in a ledger entry or in the upstream report, never in a comment.
- **A test's doc comment is at most two sentences on what is pinned.** No numbers,
  no oracle readings, no dates, no report numbers. Expected values live only in the
  assertions, measurements only in `ledger.json`, the records and the upstream
  reports.
- **Records are short.** A step record is 5-10 lines: what changed and why, the
  measurement behind it, the ledger exclusions and their pins by name, the commits,
  the gate result. Point at evidence that exists (tests, `ledger.json`, doc
  comments, audit reports). No tables, no narrative.
- **No gitignored or scratch file as provenance in committed text** (`tmp/`, the
  session scratchpad, a worktree-local probe): they vanish. Committed provenance, in
  a record or a ledger entry, is a dated sentence naming the tool and the recipe
  that re-derives the numbers and, when small, the tracked evidence itself (a pin
  literal, a golden, a `tests/corpus` file). Local paths go in the report to the
  coordinator.
