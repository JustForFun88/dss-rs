# dss-rs — instructions for agents

dss-rs is a pure-Rust power-system engine. It reads DSS scripts, solves the network (power flow, time series, harmonics, dynamics, fault studies) and reports the result. The product is the library `dss-core` plus the `dss-cli` binary. It grows into a suite for power-system design studies (`ROADMAP.md`), in idiomatic Rust, with wasm-sandboxed user models and results that are right by physics and by the standards the studies follow.

## Start every session here
Read `ROADMAP.md`: where the product is going and its known defects. `plan/` is the gitignored notebook of the session the user works with (plans, briefs, the work status `plan/STATUS.md`): read `plan/STATUS.md` when it exists, never ask for it and never stop when it is absent.

An agent reports to the session that launched it, and its questions go to that session, not the user (P4, H5).

- **S1. Only the session the user works with edits `ROADMAP.md` and `plan/STATUS.md`.** `ROADMAP.md` changes in batches, when a milestone lands or the direction changes: a landed milestone becomes one line under Done. `plan/STATUS.md` is rewritten, never appended: what landed leaves it. An agent edits neither and names in its report the milestone or defect it touched.

## Physics and standards decide
Engineers act on the numbers this engine prints, so a number is right by physics or by the standard its study follows, or it is a bug, whatever another program prints.
- **P1. No program is an authority.** A behaviour question is decided by a derivation plus a measured balance: conservation of complex power, the circuit laws, what the quantity means to an engineer.
- **P2. A standard binds where it defines a method or a criterion.** A study names the standards and editions it follows and what each gives (method, data, criterion). Their formulas, factors and conventions stay even where an exact solution or a book disagrees, since an engineer reports their result, and the doc names each departure. Their worked examples are tests. The rest (models, numerical methods, a calculation the standard only judges) is physics (P1).
- **P3. Sources.** A derivation is checked against two independent authoritative published sources (a book quoting another counts once). With fewer, its doc says so, and a balance, a limit case or an analytic solution checks it too. A method a standard defines needs only that standard (P2). An authoritative published source is a publisher's book, a peer-reviewed paper, a standard or normative document (IEC, IEEE, ГОСТ, ПУЭ), a working-group report (CIGRE, IEEE PES) or a manufacturer's documentation or data on its own product, never a student paper, a forum, a wiki or generated text. A doc may cite them by section and formula.
- **P4.** A doubtful case goes to the user with the physical argument.
- **P5. Reading sources.** Books: `E:\1_WORK\Изучить\` (catalog `_каталог.md`), standards: `D:\OneDrive\0_NORMS\`. Formulas are read off the page image (`pdf_render_pages`). A scan (no text layer in `pdf_info`) is read one page per call: multi-page OCR hangs pdf-mcp until `/mcp`.

## Clean room
- **R1.** Learn from authoritative published sources (P3), public specifications and observed behaviour. Do not copy another product's code, assets, strings or artwork. Check a dependency's licence before adding it.
- **R2.** Never read, disassemble or copy anything inside the sources and binaries of the programs this engine is compared with (T1): file names and listings only. Never commit files produced by them. Never copy their wording beyond feature names.
- **R3.** File formats come from public specifications. Never copy GPL/LGPL/AGPL code.

## House rules
- **H1. Verify, don't guess.** Open a definition before using it. Confirm the cause in the code, even when someone names a location.
- **H2. Fix the cause, not the symptom.** Find where the wrong value (wrong by physics, P1) or state comes from and fix it there, so the whole family of failures goes, not only the case at hand. No special case, clamp or fallback that hides the symptom. When the cause lies beyond the task, report it (H6) instead of patching around it. A fix comes with a test that fails without it, a crash fix with a test that panicked before it.
- **H3. Review the diff before you hand it over.** Identifiers exist as used, the test fails without the fix, nothing unrelated changed, and the report is true.
- **H4. Report only what ran.** Say what you tested and what you could not run. Never claim a run you did not do.
- **H5. Stop and ask only about** a physics case you cannot settle (P4), work beyond the request, publishing, a new remote, secrets, spending money, destructive git, and anything else hard to undo, such as deleting files outside your task. Deleting a work branch after its squash landing is not destructive git. Otherwise keep working and report the decision.
- **H6. Found more work? Report it, don't widen the change.** A gap or a dependency you find goes into your report, and the session the user works with queues it in `plan/STATUS.md`.
- **H7. Guards say why.** A check that looks removable says what it prevents. Read that before changing it.
- **H8. Keep secrets and personal details out** of the repository, logs and requests.
- **H9. No agent attribution.** A commit does not say an agent wrote it: no trailers.
- **H10. Check a dependency's licence before adding it.**
- **H11. Structured results.** Decide on typed fields, never on English text. Catalogues are typed tables, not string matches.
- **H12. Put code at the lowest layer that can own it.**
- **H13. Never lock up around other programs.** Every external program you, a test or a tool run (Python, `epri-worker`, git) gets a closed or piped stdin (`< /dev/null` in Bash, the PowerShell tool already attaches the null device), never an inherited one, and a deadline. Its output is drained so pipes never fill, the child is killed on timeout, and a missing program or a timeout is an error with a remedy, never a hang.
- **H14. Parallel agents.** Each agent edits only the files it owns. A shared file gets small surgical edits, re-read right before editing. If someone else's edit breaks the build, wait and retry. Each worktree has its own `CARGO_TARGET_DIR`, deleted with the worktree (disk).

## Architecture
| Layer | Crate | Job |
|---|---|---|
| L0 | `dss-parser` | reads DSS text: tokens, quoting, inline RPN |
| L0 | `dss-sparse` | solves the sparse complex nodal system (pure-Rust faer) |
| L0 | `dss-metis` | splits the network graph for A-Diakoptics |
| L0 | `dss-compat` | the lane switch: `opendss-parity` builds the precision-compat engine |
| L1 | `dss-usermodel` | runs user-written models in a wasm sandbox |
| L2 | `dss-core` | the engine: turns a script into a circuit, solves it, reports it |
| L3 | `dss-cli` | the command-line binary |
| test | `dss-epri`, `dss-test-harness` | run the OpenDSS engines and compare their numbers with ours |
| tool | `gate-kind` | grades a diff for the gate (G4) |

A crate depends only on lower layers. Inside `dss-core` a script flows from the command executive (`exec`) into objects and their properties (`obj`), which make up the circuit (`circuit`, `elements`). `solution` builds the nodal matrix and iterates, `report` writes what was solved. A new element is one more module under `elements` and changes none of this.

Where things are: `tests/golden/` (byte goldens), `tests/corpus/` (our deck families, manifests, `ledger.json`, and the OpenDSS test suite that `tools/corpus/vendor.py` fetches), `tools/golden/` (golden generators and probes), `tools/oracle/` (the server the corpus gate calls for the `capi_v0145` channel), `tools/opendss/` (the r4133 binary and the OpenDSS documentation, `epri_worker.py`), `tools/gate/` (the gate queue), `tools/lanes/` (`lane_diff.ps1`). A tool documents itself in its own file (W6). A test's knobs (environment variables) are in the module doc of the test that reads them.

## Code
- **C1.** `#![forbid(unsafe_code)]` in every product crate. `dss-epri` is the one exception (`publish = false`, `#[cfg(windows)]`, `#![deny(unsafe_op_in_unsafe_fn)]`, `// SAFETY` docs) and never ships. Prefer pure-Rust crates: a dependency is safe to use from safe code.
- **C2.** Write idiomatic Rust.
- **C3.** No code is frozen: passing tests are the only contract. A module that grows large or mixes concerns becomes a directory module (`foo/mod.rs` plus submodules) with no behaviour change.
- **C4.** Unit tests stay inline as `#[cfg(test)]` modules, in a sibling `tests.rs` only when the file is large, the `#[cfg(test)] mod tests;` line right after the module doc. Integration tests are thin drivers over `dss-test-harness`. No test reads a document (`.md`).

## Never crash
An engineer's study must not die on a bad script.
- **N1. No panic in non-test code**: no `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!` or `assert!`, no indexing `a[i]` or `a[i..j]`, no panicking std method (`split_at`, `copy_from_slice`, `chunks(n)`, `remove`, `insert`, `swap`…). Access goes through iterators, `get`, `get_mut` and `zip`. An error is a `Result` through the crate's error type and `?`. An unfinished feature returns an error, never panics. A default replaces a value only when it is the right one (the documented default of a property). Otherwise the error goes up with `?`: `unwrap_or`, `unwrap_or_default`, `.ok()` or `let _ =` over an error that matters hides the symptom (H2). A step that may fail without stopping the run still reports why.
- **N2. Numbers from input are checked at the boundary.** Input is the DSS script and the files it names (LoadShape files, curves, JSON). The check runs when a token is parsed and when a property or command is applied: ranges, sizes and counts (`phases=`, `Redirect` nesting), quantities derived from input once at setup. A bad value is refused with an error at the edit. Inside the solve values are right by construction and are not checked again.
- **N3. A result that is not finite is not a solution.** A solve that yields NaN or infinity (a singular matrix, a divergence) has not converged and reports an error.

## The DSS language and OpenDSS
The engine reads the DSS language, and two OpenDSS engines check the numbers of this part. They share code and bugs, so their agreement proves the code path, not the physics.
- **D1. The OpenDSS documentation specifies the DSS language** (`tools/opendss/bin/r4133/OpenDSS_Documentation.chm`, readable after `hh.exe -decompile <dir> <chm>`): what a script may say and what it means. Input it rules out is refused, even where OpenDSS accepts it silently. The language grows only by the user's ruling: an agent adds no syntax, no guessing at intent and no warning in place of a refusal. A meaning the documentation leaves open follows r4133 where physics (P1) and the input checks (N2) are indifferent. How a refusal is reported, the reports and the exports are ours.
- **D2. OpenDSS bugs are never reproduced, in any lane.** The divergence is excluded field by field, on the channel of each engine that has the bug (a ledger entry in the corpus gate), and pinned by a test of our value.
- **D3. No new precision quirk.** The existing ones (truncated constants, FPC rounding and formatting) live in the parity lane, each marked `TODO(compat):` with its clean fix. One leaves when the goldens stay byte-exact without it or become this engine's own.
- **T1. Channels.** `capi_v0145` is the dss-python pinned in `tools/golden/PIN.txt`, which also produced `tests/golden/` (regenerated only by hand through `tools/golden/*.py`). They are the one exception to R2, until they become this engine's own (`ROADMAP.md`). `r4133` is the EPRI binary at `tools/opendss/bin/r4133/`, driven through `epri-worker` (`crates/dss-epri`).
- **T2. Corpus gate.** `corpus_gate_all_cases_match_engines` (`crates/dss-core/tests/corpus_gate.rs`) runs every manifest case against its channels. Every measured divergence is an entry in `tests/corpus/ledger.json`, and an entry nothing hits fails the gate. A manifest leaves a channel out of `engines` only when that engine cannot run the case, never because of a bug (D2).
- **T3. A gap with OpenDSS is our bug until proven otherwise.** Tighten the loop tolerance (a gap that collapses means a shared fixpoint), isolate the suspect in a controlled experiment, dump both trajectories and find the first divergence. A gap that vanishes under a tighter tolerance but leaves different iteration counts is a state leak across steps.
- **T4. A cancellation floor is proven by decomposition, never by a tolerance sweep.** Read the live f64 state on both engines and show that each summand matches to the last ulps while only their difference is loose. Such a floor is neither a precision quirk to mark nor "fixed".
- **T5. Never loosen a tolerance** to make a comparison pass. A floor of `harness::tol_for` changes only with a proof by decomposition (T4), and its doc says why it exists (H7).
- **T6.** What an OpenDSS engine does is measured, never guessed: `tools/opendss/epri_worker.py` for r4133 (builds `epri-worker` on first use), the pinned dss-python for capi (pattern `tools/golden/probe_val.py`).

## Gate
- **G1. `update` and `main` are always green on all eight commands below**, the tests of both lanes included. Every commit that enters them runs all eight on the tree that enters. The one exception: a work branch whose base is the current head of the target and whose head passed all eight lands as one squash commit without running them again, since the squash holds the same tree.
- **G2. Every cargo command that builds or tests, `gate-kind` included, goes through the queue**: `python tools/gate/gatelock.py --label "<who>" -- cargo <args>`. `cargo fmt` does not. The machine is shared, and unqueued runs slow each other until the OpenDSS runs time out.

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo clippy --workspace --all-targets --features dss-compat/opendss-parity -- -D warnings`
4. `cargo nextest run --workspace`
5. `cargo nextest run --workspace --features dss-compat/opendss-parity`
6. `cargo test --workspace --doc`
7. `cargo test --workspace --doc --features dss-compat/opendss-parity`
8. `RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links" cargo doc --workspace --no-deps --document-private-items`

- **G3.** Two lanes. **parity** (the commands with the parity feature) reproduces the OpenDSS precision and is byte-exact on the goldens. **default** is the product. The comparison policy of each lives in `crates/dss-test-harness/src/harness/lane.rs`. After a change to a `compat.rs` kernel, `dss-sparse` or the `solution` module, run `pwsh -File tools/lanes/lane_diff.ps1`: it must exit 0, the two lanes agreeing within the bounds of `crates/dss-core/examples/lane_dump.rs`.
- **G4. A commit on a work branch passes the gate its diff needs**, graded by `cargo run -p gate-kind -- --base <rev>` against the commit it builds on: `None` and `Docs` run nothing, `Comments` commands 1–3 and 6–8, `Code` all eight. In doubt, all eight.

## Writing
Every agent reads these texts, so a stale or long sentence costs every session.
- **W1. Comments and docs are short, self-contained and say what the code does now, not its history.** A doc opens with what its item is or does, and a long enumeration in it is a list. The general never names the particular: a trait names no implementors, a helper no callers, a lower layer no higher one.
- **W2. A test's name says what it checks.** Its doc comment is optional: at most two sentences on what is pinned and why it is right (by physics, P1, a standard, P2, or the DSS documentation, D1). No measured values, OpenDSS readings, dates or report numbers. Expected values live only in the assertions.
- **W3. No measurement in prose in a committed file**, except the first failure an `exclusion` records in the ledger. Local paths go in your report.
- **W4. A citation of code names it, never a line number.** In a doc comment the name is an intra-doc link, which rustdoc checks. Elsewhere it is the shortest Rust path that stays unambiguous (`harness::tol_for`, `Solution::converged`). A whole file is cited by its repository path.
- **W5. Committed text cites no other project's source (no file, routine, line or path), and no comment or doc retells what an OpenDSS engine does.** The OpenDSS documentation (D1) and `TODO(compat):` marks (D3) stay.
- **W6. No README.** A tool or a test documents itself in the module doc or docstring of its own file: what it is, how it runs, its knobs. The repository has one `README.md`, at the root, and third-party files are listed in `ATTRIBUTION.md`.

## Commits
History is read without our plans, so a commit must make sense on its own.
- **M1. The subject names the change in the code** (`SeqCurrents: exclude and pin the negative-rating divergence`). The body is as long as the change needs: what changed and why, by area (behaviour, tests added or removed, divergences excluded, data regenerated). A small fix needs no body. The body tells what the commit does, not how the work went. No plan, step, wave, branch, ruling or work-package id in a subject or body, and no path that git ignores (the fetched OpenDSS test suite excepted) or that lies outside the repository.
- **M2.** A work branch lands as one squash commit.
- **M3.** Stage files by name, never `git add -A`. Deck droppings are never staged. Once no test run is in flight (the gate queue is empty), whoever left them, delete them by name in the tracked deck families and run `python tools/corpus/vendor.py` for the fetched OpenDSS test suite.

## Launching agents and workflows
Agents share one machine. Past its capacity every run slows down and the OpenDSS runs time out.
- **L1. Check the machine before every launch, also when the user has just asked for the work**: workflows and work branches in flight, the gate queue (`tools/gate/gatelock.py`, waiting tickets mean full), CPU, memory, `cargo`/`rustc` processes. Ceiling: about three workflows and three to four worktrees building at once.
- **L2.** Free: launch. Short: wait or queue, and report the decision in one line. Ask the user only when the launch widens the scope. A freed slot is not a reason to launch.
- **L3.** After stopping a workflow, check that its scripts no longer queue builds.

## Git worktrees
Worktrees under `.claude/worktrees/` hold a Windows directory junction to the worktree's cargo target. A new worktree runs `python tools/corpus/vendor.py` once (the tarball is cached). `git worktree remove` and any recursive delete (`rm -rf`, `Remove-Item -Recurse`, `rmdir /s`) follow a junction and delete what it points at: this has wiped a shared directory twice. **Never batch-remove.** For each worktree, in PowerShell only (under Git Bash `cmd /c rmdir` does nothing and exits 0):
1. List the reparse points without following them: `Get-ChildItem -LiteralPath <wt> -Recurse -Force | ? { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }`
2. Drop each link only: `$_.Delete()` on the `DirectoryInfo`, or `cmd /c rmdir "<path>"` without `/s`.
3. `Test-Path` of every listed link must be False.
4. Then `git worktree remove --force <wt>` and `git worktree prune`. Delete merged branches with `git branch -D`.
