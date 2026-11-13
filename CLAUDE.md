# dss-rs — instructions for agents

dss-rs is a pure-Rust power-system engine. It reads DSS scripts, solves the network (power flow, time series, harmonics, dynamics, fault studies) and reports the result. The product is the library `dss-core` plus the `dss-cli` binary. It grows into a suite for power-system design studies (`ROADMAP.md`), in idiomatic Rust, with wasm-sandboxed user models and results that are right by physics and by the standards the studies follow.

## Start every session here
Read `ROADMAP.md`: where the product is going. `plan/` is the gitignored notebook of the session the user works with (plans, briefs, the work status `plan/STATUS.md`, the defect files `plan/defects/`): read `plan/STATUS.md` when it exists, never ask for it and never stop when it is absent.

An agent reports to the session that launched it, and its questions go to that session, not the user (P4, H5).

- **S1. Only the session the user works with edits `ROADMAP.md` and `plan/STATUS.md`.** `ROADMAP.md` changes in batches, when a milestone lands (one line under Done) or the direction changes. `plan/STATUS.md` is rewritten, never appended, and holds only the current state, the open step and the open work: no step record, no past date, no account of a commit or of a review (the git logs of the project and of `plan/` hold those). A defect is a file in `plan/defects/`, like an issue, that `python plan/defects/query.py` accepts: the format is in its doc, the labels in `labels.toml` beside it. Whoever confirms a defect, an auditor included, opens its file in the main checkout (a worktree has no `plan/`), and the work that fixes it deletes the file. An agent's report names the milestone it touched and the defect files it opened or deleted.

## Physics and standards decide
Engineers act on the numbers this engine prints, so a number is right by physics or by the standard its study follows, or it is a bug, whatever another program prints.
- **P1. No program is an authority.** A behaviour question is decided by a derivation plus a measured balance: conservation of complex power, the circuit laws, what the quantity means to an engineer.
- **P2. A standard binds where it defines a method or a criterion.** A study names the standards and editions it follows and what each gives (method, data, criterion). Their formulas, factors and conventions stay even where an exact solution or a book disagrees, since an engineer reports their result, and the doc names each departure. Their worked examples are tests. The rest (models, numerical methods, a calculation the standard only judges) is physics (P1).
- **P3. Sources.** A derivation is checked against two independent authoritative published sources (a book quoting another counts once). With fewer, its doc says so, and a balance, a limit case or an analytic solution checks it too. A method a standard defines needs only that standard (P2). An authoritative published source is a publisher's book, a peer-reviewed paper, a standard or normative document (IEC, IEEE, ГОСТ, ПУЭ), a working-group report (CIGRE, IEEE PES) or a manufacturer's documentation or data on its own product, never a student paper, a forum, a wiki or generated text. A doc may cite them by section and formula.
- **P4.** A doubtful case goes to the user (an agent: through the session that launched it) with the physical argument.
- **P5. Reading sources.** Books, papers and reports are in `D:\OneDrive\1_LIBRARY\` by topic, the OpenDSS documents in its `OpenDSS_docs\` (a `.md` beside each Word and PowerPoint file), standards in `D:\OneDrive\0_NORMS\` by publisher. Both library folders have a `_catalog.md`, and the library's lists each topic's standards. The OpenDSS documents only show where an algorithm or a formula comes from and are never an authoritative source (P3). Formulas are read off the page image (`pdf_render_pages`), a scan one page per call: multi-page OCR hangs pdf-mcp until `/mcp`.

## Clean room
- **R1.** Learn from authoritative published sources (P3), public specifications and observed behaviour. Do not copy another product's code, assets, strings or artwork. Check a dependency's licence before adding it.
- **R2.** Never read, disassemble or copy anything inside the sources and binaries of the programs this engine is compared with (T1): file names and listings only. Never commit files produced by them. Never copy their wording beyond feature names.
- **R3.** File formats come from public specifications. Never copy GPL/LGPL/AGPL code.

## Licence allowlist
Dependencies and third-party files use SPDX identifiers. An expression is allowed when the identifiers it requires are all in this list (a dual licence passes through its permissive option). Nothing is added to the list without the user (H5).
- `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `0BSD`, `BSL-1.0`, `Unicode-3.0`
- `CC0-1.0`, `CC-BY-4.0`, `LicenseRef-Public-Domain` (data and documents)

A program run only as a separate process (the OpenDSS engines, T1) may have any licence: nothing of it is linked or copied (R2).

## House rules
- **H1. Verify, don't guess.** Open a definition before using it. Confirm the cause in the code, even when someone names a location.
- **H2. Fix the cause, not the symptom.** Find where the wrong value (wrong by physics, P1) or state comes from and fix it there, so the whole family of failures goes, not only the case at hand. No special case, clamp or fallback that hides the symptom. When the cause lies beyond the task, report it (H6) instead of patching around it. A fix comes with a test that fails without it, a crash fix with a test that panicked before it.
- **H3. Review the diff before you hand it over.** Identifiers exist as used, the test fails without the fix, nothing unrelated changed, and the report is true.
- **H4. Report only what ran.** Say what you tested and what you could not run. Never claim a run you did not do.
- **H5. Stop and ask only about** a physics case you cannot settle (P4), a licence outside the allowlist, work beyond the request, publishing, a new remote, secrets, spending money, destructive git, and anything else hard to undo, such as deleting files outside your task. Deleting a work branch after it landed is not destructive git. Otherwise keep working and report the decision.
- **H6. Found more work? Report it, don't widen the change. Can't finish? Stop, don't narrow it.** A gap or a dependency you find goes into your report, and the session the user works with queues it in `plan/STATUS.md`. Work that cannot be done as its task and plan say (it outgrows one agent, or the design meets an obstacle) stops with the reason, and the session that launched it cuts it finer: a smaller scope, another design or a remainder left for later never lands, even as a disclosed deviation. A plan has no fallback stop point.
- **H7. Guards say why.** A check that looks removable says what it prevents. Read that before changing it.
- **H8. Keep secrets and personal details out** of the repository, logs and requests.
- **H9. No agent attribution.** A commit does not say an agent wrote it: no trailers.
- **H10. Structured results.** Decide on typed fields, never on English text. Catalogues are typed tables, not string matches.
- **H11. Put code at the lowest layer that can own it.**
- **H12. Never lock up around other programs.** Every external program you, a test or a tool run (Python, `epri-worker`, git) gets a closed or piped stdin (`< /dev/null` in Bash, the PowerShell tool already attaches the null device), never an inherited one, and a deadline. Its output is drained so pipes never fill, the child is killed on timeout, and a missing program or a timeout is an error with a remedy, never a hang.
- **H13. Parallel agents.** Each agent edits only the files it owns. A shared file gets small surgical edits, re-read right before editing. If someone else's edit breaks the build, wait and retry. Each worktree has its own cargo target directory, deleted with the worktree (Git worktrees below).

## Architecture
| Layer | Crate | Job |
|---|---|---|
| base | `dss-compat` | the lane switch: `opendss-parity` builds the precision-compat engine |
| L0 | `dss-parser` | reads DSS text: tokens, quoting, inline RPN |
| L0 | `dss-sparse` | solves the sparse complex nodal system (pure-Rust faer) |
| L0 | `dss-metis` | splits the network graph for A-Diakoptics |
| L1 | `dss-usermodel` | runs user-written models in a wasm sandbox |
| L2 | `dss-core` | the engine: turns a script into a circuit, solves it, reports it |
| L3 | `dss-cli` | the command-line binary |
| test | `dss-epri`, `dss-test-harness` | run the OpenDSS engines and compare their numbers with ours |
| tool | `gate-kind` | grades a diff for the gate (G4) |

A crate depends only on lower layers. Inside `dss-core` a script flows from the command executive (`exec`) into objects and their properties (`obj`), which make up the circuit (`circuit`, `elements`). `solution` builds the nodal matrix and iterates, `report` writes what was solved. A new element is one more module under `elements` and changes none of this.

Where things are: `tests/golden/` (byte goldens), `tests/corpus/` (our deck families, manifests, `ledger.json`, and the OpenDSS test suite that `tools/corpus/vendor.py` fetches), `tools/golden/` (golden generators and probes), `tools/oracle/` (the server the corpus gate calls for the `capi_v0145` channel), `tools/opendss/` (the r4133 binary and the OpenDSS documentation, `epri_worker.py`), `tools/gate/` (the gate queue), `tools/lanes/` (`lane_diff.ps1`).

## Code
- **C1.** `#![forbid(unsafe_code)]` in every workspace crate. `dss-epri` is the one exception (`publish = false`, `#[cfg(windows)]`, `#![deny(unsafe_op_in_unsafe_fn)]`, `// SAFETY` docs) and never ships. Prefer pure-Rust crates: a dependency is safe to use from safe code.
- **C2. Idiomatic Rust in the functional style.** Each independently managed part of the code is a type named for its role, with a private representation. An operation is a method of the smallest type that holds the data it changes, and no type gathers the operations of its parts (C5). Data flow is explicit, through arguments and return values, and operations compose through method calls, iterators and combinators. The only free-standing functions are `main`, tests and ABI exports (never a plain `pub fn`).
- **C3.** No code is frozen: passing tests are the only contract. A module that grows large or mixes concerns becomes a directory module (`foo/mod.rs` plus submodules) with no behaviour change.
- **C4.** Unit tests stay inline as `#[cfg(test)]` modules, in a sibling `tests.rs` only when the file is large, the `#[cfg(test)] mod tests;` line right after the module doc. A comparison with an OpenDSS engine goes through `dss-test-harness`. No test reads a document (`.md`).
- **C5. State ownership.** Each piece of mutable state has one owner, and none is global. An operation gets access only to the data it reads, changes or consumes, `self` and context objects included. One that changes several parts belongs to their smallest common owner, which only calls their methods and passes values between them. Mutation is explicit: `&mut self` or a `&mut` argument.

  State stays with its owner and is updated in place, and no operation holds a copy of it: what it gathers to work on borrows from the owner. A type that owns mutable state across operations is not `Clone`. A copy that must exist is state of its own, with an owner, a type and a role. So is the working storage of a repeated pass (a solver iteration, a time step), which its owner keeps and each pass reuses.

  A borrow conflict is resolved by narrowing access or restructuring ownership, never by a workaround, in any form: a copy written back, a value moved out and put back, a copied worklist, shared ownership, interior mutability. A workaround is code that exists only so that the borrow checker accepts the code: with direct access in its place the result would be the same. A fix beyond the task stops with the conflict described (H6).

  Code in the tree that breaks C2, C5 or C6 is a defect (S1), never a pattern.
- **C6. Contracts live in types.** As many types as possible, and as many contracts as possible held on them, so that the compiler checks what must hold and an access error cannot exist. Among them: a type per kind of index, quantity and state, a handle that only its owner makes and that cannot outlive what it points at, a value that exists only once the step that yields it has succeeded. A check stays at run time only where no type can state the contract. A type is given up for performance only when a measurement shows the bound of the task broken. How hard a design is to build is no argument against it.

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
- **T1. Channels.** `capi_v0145` is the dss-python pinned in `tools/golden/PIN.txt`, which also produced `tests/golden/` (regenerated only by hand through `tools/golden/*.py`, one golden, `props/fuse.json`, carrying r4133 values). The pin and the goldens are the one exception to R2, until they become this engine's own (`ROADMAP.md`). `r4133` is the EPRI binary at `tools/opendss/bin/r4133/`, driven through `epri-worker` (`crates/dss-epri`).
- **T2. Corpus gate.** `corpus_gate_all_cases_match_engines` (`crates/dss-core/tests/corpus_gate.rs`) runs every manifest case against its channels. Every measured divergence is an entry in `tests/corpus/ledger.json`, and an entry nothing hits fails the gate. A manifest leaves a channel out of `engines` only when that engine cannot run the case, never because of a bug (D2).
- **T3. A gap with OpenDSS is our bug until proven otherwise.** Tighten the loop tolerance (a gap that collapses means a shared fixpoint), isolate the suspect in a controlled experiment, dump both trajectories and find the first divergence. A gap that vanishes under a tighter tolerance but leaves different iteration counts is a state leak across steps.
- **T4. A cancellation floor is proven by decomposition, never by a tolerance sweep.** Read the live f64 state on both engines and show that each summand matches to the last ulps while only their difference is loose. Such a floor is neither a precision quirk to mark nor "fixed".
- **T5. Never loosen a tolerance** to make a comparison pass. A floor of `harness::tol_for` changes only with a proof by decomposition (T4), and its doc says why it exists (H7).
- **T6.** What an OpenDSS engine does is measured, never guessed: `tools/opendss/epri_worker.py` for r4133 (builds `epri-worker` on first use), the pinned dss-python for capi (pattern `tools/golden/probe_val.py`).

## Gate
- **G1. Every head of `update` and `main` is green on all eight commands below**, the tests of both lanes included. Work enters them as a series of commits (M2) from their current head, and only the head of the series is gated (G4): a commit inside it may fail to build or to pass. A series whose base is no longer the head of the target is rebased onto it and passes its gate again.
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
- **G4. The head of a series passes the gate of the whole series**, graded by `python tools/gate/gatelock.py --label "<who>" -- cargo run -p gate-kind -- --base <rev>` (`--head <commit>` once committed), `<rev>` being the base of the series: `None` and `Docs` run nothing, `Comments` commands 1–3 and 6–8, `Code` all eight. In doubt, all eight.
- **G5. A series never weakens the tests.** No test is removed, ignored, narrowed or given another expected value or a looser tolerance so that the gate passes. A test moved or rewritten for a new structure keeps its cases and its expected values. A test goes or changes what it asserts only in a commit whose message names it and says why: the behaviour changed by physics, a standard or the DSS documentation, or what it checked no longer exists, and then the message says what took its place.

## Writing
Every agent reads these texts, so a stale or long sentence costs every session.
- **W1. Comments and docs are short, self-contained and say what the code does now, not its history.** A doc opens with what its item is or does, and a long enumeration in it is a list. The general never names the particular: a trait names no implementors, a helper no callers, a lower layer no higher one.
- **W2. A test's name says what it checks.** Its doc comment is optional: at most two sentences on what is pinned and why it is right (by physics, P1, a standard, P2, or the DSS documentation, D1). No measured values, OpenDSS readings or dates. Expected values live only in the assertions.
- **W3. No measurement in prose in a committed file**, except the first failure an `exclusion` records in the ledger. Local paths go in your report.
- **W4. A citation of code names it, never a line number.** In a doc comment the name is an intra-doc link, which rustdoc checks. Elsewhere it is the shortest Rust path that stays unambiguous (`harness::tol_for`, `Solution::converged`). A whole file is cited by its repository path.
- **W5. Committed text cites no other project's source (no file, routine, line or path), and no comment or doc retells what an OpenDSS engine does.** The OpenDSS documentation (D1) and `TODO(compat):` marks (D3) stay.
- **W6. No README.** A tool or a test documents itself in the module doc or docstring of its own file: what it is, how it runs, its knobs. The repository has one `README.md`, at the root, and third-party files are listed in `ATTRIBUTION.md`.

## Commits
History is read without our plans, so a commit must make sense on its own.
- **M1. The subject names the change in the code** (`SeqCurrents: exclude and pin the negative-rating divergence`). The body is as long as the change needs: what changed and why, by area (behaviour, tests added or removed, divergences excluded, data regenerated). A small fix needs no body. The body tells what the commit does, not how the work went. No plan, step, wave, branch, ruling or work-package id in a subject or body, and no path that git ignores (the fetched OpenDSS test suite excepted) or that lies outside the repository.
- **M2. A work branch lands as a series of small commits, as in a reviewed pull request.** A series of one commit lands by `git merge --ff-only`. A longer one lands by a merge commit (`git merge --no-ff`) on the gated head, with the same tree and a message that names the change of the whole series (M1), so the first-parent history of the target holds only gated heads (`git bisect --first-parent`). Each commit is one change that makes sense alone (M1), with its tests (H2), the data a generator wrote for it (goldens, locks) and the files it deletes. It stays within about 500 changed lines, never above 1500, not counting that data and those deletions. A move with no behaviour change (C3) is a commit of its own and does not count. A review fix folds into the commit it fixes (`git commit --fixup`, `git rebase --autosquash`).
- **M3.** Stage files by name, never `git add -A`. Deck droppings are never staged. Once no test run is in flight (the gate queue is empty), whoever left them, delete them by name in the tracked deck families and run `python tools/corpus/vendor.py` for the fetched OpenDSS test suite.

## Launching agents and workflows
Agents share one machine. Past its capacity every run slows down and the OpenDSS runs time out.
- **L1. Check the machine before every launch, also when the user has just asked for the work**: workflows and work branches in flight, the gate queue (`tools/gate/gatelock.py`, waiting tickets mean full), CPU, memory, `cargo`/`rustc` processes. Ceiling: about three workflows and three to four worktrees in flight.
- **L2.** Free: launch. Short: wait or queue, and report the decision in one line. Ask the user only when the launch widens the scope. A freed slot is not a reason to launch.
- **L3.** After stopping a workflow, check that its scripts no longer queue builds.
- **L4. Briefs and compaction.** An agent that writes code or audits gets its brief as a file its prompt names, and re-reads it after a compaction before the next action. An auditor appends each finding to its audit file the moment it confirms it, since a compaction keeps only a summary of what is not on disk. The harness refuses a subagent's write of a `.md` file whose name begins with `report` or `findings`, so the audit file is named otherwise (`fa_code_1.md`).
- **L5. Short waits.** An agent waiting on a background run returns from each wait within 4 minutes and checks again. Its prompt cache expires after 5 minutes of silence, and the next turn then rewrites the whole context at the cache-write price.

## Git worktrees
Worktrees under `.claude/worktrees/` hold a Windows directory junction to the worktree's cargo target. A new worktree runs `python tools/corpus/vendor.py` once (the tarball is cached). `git worktree remove` and any recursive delete (`rm -rf`, `Remove-Item -Recurse`, `rmdir /s`) follow a junction and delete what it points at: this has wiped a shared directory twice. **Never batch-remove.** For each worktree, in PowerShell only (under Git Bash `cmd /c rmdir` does nothing and exits 0):
1. List the reparse points without following them, with their targets: `Get-ChildItem -LiteralPath <wt> -Recurse -Force | ? { $_.Attributes -band [IO.FileAttributes]::ReparsePoint } | select FullName, Target`
2. Drop each link only: `$_.Delete()` on the `DirectoryInfo`, or `cmd /c rmdir "<path>"` without `/s`.
3. `Test-Path` of every listed link must be False.
4. Then `git worktree remove --force <wt>` and `git worktree prune`. Delete merged branches with `git branch -D`.
5. Delete the listed target directory by its name (`Remove-Item -LiteralPath <target> -Recurse -Force`): it is the worktree's cargo target and nothing else points at it.
