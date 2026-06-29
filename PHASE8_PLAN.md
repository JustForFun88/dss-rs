# Phase 8 — Detailed Execution Plan: Reporting, Exports, Save, Full Executive

> Companion to `PORTING_PLAN.md` §Phase 8; same status and rules of engagement as
> `PHASE4_PLAN.md §0` / `PHASE5_PLAN.md` / `PHASE6_PLAN.md` / `PHASE7_PLAN.md`
> (Pascal is the spec; probe the oracle, never guess FPC semantics; mark
> `TODO(compat)` for deliberate upstream-inexactness reproductions and `NOT_PORTED`
> for deferrals, each pointing at its phase; goldens are regenerated **manually**
> with the pinned oracle — `tools/golden/PIN.txt`: dss-python 0.15.7 / backend
> 0.14.5; the standard three-command gate must be green per step; commit only on
> explicit user request).
> **Prerequisite: Phase 7 complete** (branch `phase-7-extended-elements`,
> gate-green; the per-phase `--no-ff` merge to `main` is explicit-request-only).
> Phase 8 consumes everything Phases 3–7 built — the solved circuit/solution state,
> every element's current/power/voltage getters, the monitor in-memory f32 stream,
> the EnergyMeter registers + zone lists, the `RelCalc` reliability outputs, the
> DER/protection element surfaces, the control loop, and the live corpus gate
> (`corpus_live.rs` + manifests) — and turns them into **text/CSV report output**.
> It is almost entirely a *read-and-format* phase: **no new electrical math, no new
> solve mode** (those all landed in Phase 7). The risk is not numerics; it is
> **faithful field-by-field report layout** and **not silently faking output**.
>
> **Stop-and-confirm cadence (same as `PHASE4_PLAN §0.8` / `PHASE6_PLAN §0` /
> `PHASE7_PLAN §0`):** after each small step (a WP or a self-contained sub-step),
> run the full per-step ritual below, then stop and wait for the user's explicit
> confirmation before the next step — unless the user has explicitly authorized
> executing multiple WPs in one pass. **Run the whole ritual autonomously — do not
> pause between its sub-steps to ask permission; the single stop point is at the
> very end.**
>
> **Per-step ritual (do every step, in order, without being told):**
> 1. **Gate green.** The standard three-command gate (`cargo fmt --all --check`;
>    `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test
>    --workspace`) all pass. The third command runs **all golden tests** as part of
>    the workspace suite (`crates/dss-core/tests/golden_*.rs` — slice, feeders,
>    feeders_controls, phase5, phase6, phase7, phase7_protection, checkpoints,
>    ieee8500, reliability, allocation, gendispatcher, autoadd_reduce, smoke — plus
>    `props_roundtrip`, the always-on `corpus_*`, and Phase 8's new `golden_phase8`
>    + `save_roundtrip`); **every** golden must pass, no `#[ignore]`, no name-filter
>    that could green on zero matched tests. This is a hard requirement, not
>    advisory — a red golden blocks the commit (CLAUDE.md green-gate rule, §1 below).
> 2. **Update `STATUS.md`** for the step (the §1e record + the §1 frontier/table),
>    then **commit** the step (gate-green code + STATUS together).
> 3. **`/audit-code <scope>`** — run by an **independent agent** with a scoped brief
>    (see "Who runs the steps" below), not a fork of your full context. Scope it to
>    *this step's just-landed commit(s)*, not the whole branch: pass the step's
>    commit range (`<first-sha>^..HEAD`) or the step label (e.g. `WP8.2 step 1`).
>    Take back **only its findings report**; then *you* settle every finding against
>    the pinned oracle, **fix** what is real, update `STATUS.md` with an *audit-code
>    follow-up* note, and **commit** the fixes. (Re-run the gate before committing.)
> 4. **`/audit-tests <scope>`** — same: an **independent agent**, same scope (the
>    step's commits / label), launched in parallel with step 3. Take back its
>    findings report; *you* fix every real finding, add an *audit-tests follow-up*
>    note to `STATUS.md`, and **commit**. (Re-run the gate before committing.)
> 5. **`STATUS.md` full review + sync + cleanup.** Read **the whole of `STATUS.md`
>    end to end** (not just the section the step touched) and bring it back into a
>    lean, consistent state:
>    - **Sync:** fix anything the step made stale — the `Last updated:` line (today's
>      date + current step), the §1 phase table, the header frontier paragraph, and
>      §1e so they all agree on the active step / commit state / what's next (no two
>      places disagreeing, no "uncommitted" left after a commit).
>    - **Archive the dead weight:** move anything **no longer load-bearing for
>      executing the remaining plan** — long-completed/merged-phase logs, file-by-file
>      maps of finished phases, content fully superseded by `docs/phase-records/` —
>      out to `docs/phase-records/phase-N.md` (the established pattern: an `Archived
>      from STATUS.md (moved <date>)` header; leave a one-line pointer in `STATUS.md`,
>      add it to the §1b–1d archive index). **Do not** archive what the remaining
>      plan still reads: live gate descriptions, §3/§4/§5 (the records back-reference
>      them by number — preserve the numbering), the empirical oracle facts, run/
>      regenerate instructions, or the active §1e record.
>    - **Dedup:** collapse any paragraph that merely restates the frontier/another
>      section into a one-line pointer.
>    - **Commit** the sync+cleanup (a `docs:` commit; re-run the gate first since
>      docs-only — `cargo fmt`/`clippy`/`test` must still be green). If nothing is
>      stale or archivable this step, say so and skip the commit (no empty commits).
> 6. **Only now stop** and wait for confirmation. **The final report to the user at
>    this stop point is written in Russian** (the working language of this project) —
>    a short summary of what the step landed, what the two audits found and how it
>    was resolved, the gate/golden status, the `STATUS.md` sync/cleanup done, and
>    what the next step is. (Code, identifiers, commit messages, and `STATUS.md` stay
>    in English as before; only the conversational summary is Russian.)
>
> Notes: a finding that is *surfaced but deliberately not fixed* (needs
> investigation / upstream divergence) is **recorded in `STATUS.md`**, not silently
> dropped (the Phase-7 tracked-open notes are the template). If an audit finds
> nothing, say so and skip its fix-commit (no empty commits). The audits are
> **read-only** (they may write throwaway probes in a temp location and remove
> them); all edits are yours to make in the fix steps. Commit messages follow the
> existing `Phase 8 WPx.y step …: <audit-code|audit-tests> follow-up — <what>`
> shape already in the history.
>
> **Who runs the steps — default: you, in the main loop; the two audits go to
> independent agents.** The ritual is about staying in the loop and owning the
> result, so the gate, every `STATUS.md` edit, every fix, every commit, and the
> final report are done by **you directly** — they need the conversation context,
> the probe-the-oracle / Pascal-as-spec discipline, and ownership of the result.
> **The discovery half of the audits (steps 3–4), however, is delegated to
> independent agents** — `/audit-code` and `/audit-tests` each as its own agent, the
> two in parallel since code vs tests are independent scopes. Use **fresh,
> independent agents, not forks**: do **not** hand an auditor your whole context
> window. Forking copies the entire conversation, which is wasteful and buries the
> auditor in irrelevant state; a scoped agent reviews more sharply against a clean
> brief. Pass each auditor a **self-contained brief with only what it needs**, and
> nothing more:
> - the exact scope — the step's commit range (`<first-sha>^..HEAD`) or label;
> - the changed files / diff under audit;
> - the authoritative baselines to check against — the specific Pascal unit(s) +
>   procedure/identifier names, the relevant `PORTING_PLAN.md`/`PHASE8_PLAN.md`/
>   `STATUS.md` section(s);
> - the binding rules it must apply — the PIN (`tools/golden/PIN.txt`), the
>   `TODO(compat)` / `NOT_PORTED` discipline, and that the oracle is the spec.
>
> Each audit agent returns **only its findings report**. **You** (the main loop)
> then settle each finding against the oracle, fix what is real, and commit — the
> auditors are read-only and never edit. For a trivial sub-step where authoring the
> brief costs more than the audit, you may invoke the audit skill inline instead;
> the independent-agent path is the default whenever the diff/prior-code/Pascal
> reads would bloat the main context.
>
> **On Pascal line references:** this plan is written just-in-time, before the
> per-WP deep read. It cites Pascal **units + procedure/identifier names** (stable)
> plus the verified file sizes below; **exact line numbers are confirmed when each
> WP opens its unit** and recorded in `STATUS.md` as the WP lands (the established
> convention).

## 0. Scope and ordering

Phase 8 is **~12%** of the port (PORTING_PLAN cumulative). It is the reporting/
output and full-executive layer: `Export`, `Show`, `Save`, `Dump`, the long tail of
executive verbs (`BatchEdit`, `Interpolate`, `Distribute`, …), and the full
`ReduceAlgs`. Pascal scope (`wc -l` of the vendored `.inputs/dss_capi/src`):

| Sub-block | Pascal units (≈ lines) |
|---|---|
| Export | `Common/ExportResults.pas` 3927 + `Executive/ExportOptions.pas` 642 |
| Show | `Common/ShowResults.pas` 3981 + `Executive/ShowOptions.pas` 438 |
| Save / Dump | `Circuit.pas` (3121) save machinery + `Utilities.pas` (2406) `WriteClassFile` + `ExecHelper.pas` `DoSaveCmd`/`DumpProperties` |
| Executive tail | the unported remainder of `ExecHelper.pas` 5081 / `ExecOptions.pas` 1204 / `ExecCommands.pas` 708 (BatchEdit, Interpolate, Distribute, Uuids, GISCoords, …) |
| Reduce | `Meters/ReduceAlgs.pas` 540 + `EnergyMeter.ReduceZone`/`InterpolateCoordinates` + `Line.MergeWith` |
| Support | `Utilities.pas` number/string formatters + output-path machinery (cross-cutting) |

**Execution order (dependency-respecting, risk-ascending):**

1. **Report infrastructure + the comparison harness (WP8.1)** — the `report/`
   module, the output-directory machinery, the Pascal number-formatting helpers,
   the **new text/CSV golden-diff harness** (Phase 8's defining new test tool), and
   the faithful **Plot/Visualize no-op**. Nothing else can be gated without the
   harness, so it goes first.
2. **Export: solution outputs (WP8.2)** — the exports that read solved state
   (Voltages/Currents/Powers/Seq*/Losses/Taps/Summary/…). The simplest read-and-
   format path; shakes out the harness.
3. **Export: device/meter/reliability outputs (WP8.3)** — Monitors/Meters/DER/
   EventLog/Faultstudy/reliability + the demand-interval/SystemMeter files carried
   from Phase 6.
4. **Show reports (WP8.4)** — upgrade the current no-op to real text reports; shares
   field logic with WP8.2.
5. **Save + Dump (WP8.5)** — `Circuit.Save`, `Save <class>`, `Dump`; the
   re-compile/re-solve round-trip gate.
6. **Executive tail (WP8.6)** — BatchEdit, Interpolate, Distribute, Uuids, and the
   remaining executive verbs. Independent of the report infra — can be pulled
   earlier if convenient.
7. **ReduceAlgs full (WP8.7)** — the reduction strategies; needs the Phase-6-
   deferred `TLineObj.MergeWith`.
8. **Phase exit (WP8.8)** — marker sweep, `cmd_coverage.py` tail-coverage proof,
   full re-run + live re-classify, merge.

WP boundaries are flex points (the Phase-7 convention): WP8.6 (BatchEdit etc.) is
independent of the report infra and may be pulled earlier; the exports may be
re-grouped by element family.

## 1. Phase target and gate

The product of Phase 8 is **faithful report output**, not new electrical results.
Two distinct correctness axes hold, and the plan keeps them separate:

- **Export/Show/Dump/Save text correctness** — pinned by the **new targeted
  text/CSV golden gate** (the oracle writes the report, we diff it numerically,
  §2.3).
- **Corpus model correctness on the now-runnable cases** — pinned by the **existing
  always-on live full-model gate**: once a verb executes without error, its decks
  migrate into `solvable_now` and the assembled model (Y, V, every I/P, YPrim,
  injection, discrete state) is compared against the oracle. The live gate does
  **not** read report text, so a report bug cannot hide behind it — an export must
  therefore carry its own targeted golden.

**Overall gate** (every WP adds to it; all stay green):

1. The **standard three-command gate** (`cargo fmt --all --check`; `cargo clippy
   --workspace --all-targets -- -D warnings`; `cargo test --workspace`) green after
   every step, with **all** prior goldens still green.
2. **New targeted text/CSV golden** — `tools/golden/gen_phase8.py` →
   `tests/golden/phase8/<report>.{csv,txt}` (the oracle's exact export/show output)
   + `crates/dss-core/tests/golden_phase8.rs` driving the new harness comparator
   (§2.3). Coverage grows per WP. Canonical feeders: IEEE13/34/37/123 and 8500.
3. **Save round-trip** — `crates/dss-core/tests/save_roundtrip.rs`: `Save circuit`
   on IEEE13/37/123 → the emitted master + sub-files **re-compile on our own
   engine** → re-solve → node voltages identical to the pre-save solve (1e-6 rel,
   iterations exact). Faithfulness is *round-trip*, **not** byte-equality with the
   oracle's `Save`.
4. **Live corpus growth** — as each verb lands, `DSS_LIVE_CLASSIFY=1
   corpus_live_classify` + `tools/corpus/apply_classify.py` migrate the now-
   unblocked decks from `skipped_unsupported` into `solvable_now`; the **always-on**
   live full-model compare (`corpus_live.rs`, no env gate — runs in every `cargo
   test`, CLAUDE.md) then verifies each against the oracle. `tests/corpus/
   COVERAGE.md` records the burn-down.

**Per-sub-block focused gates** (one line each; the detail is in each WP):

1. **Export solution** — CSV diffed numerically vs the oracle's CSV on a solved IEEE
   feeder (header tokens exact, row set exact, per-number tolerance §2.3).
2. **Export device/meter** — monitor CSV channels match the oracle elementwise (the
   in-memory f32 stream → CSV), meter registers match.
3. **Show** — report text diffed numerically (numbers parsed out, never raw float
   strings).
4. **Save** — the round-trip gate (#3 above) on IEEE13/37/123.
5. **Executive tail** — BatchEdit/Interpolate/Distribute behavior vs the oracle via
   the live model compare on a corpus case.
6. **Reduce** — post-reduce branch/node counts + a re-solve vs the oracle on a
   metered zone.

**No in-scope port is skipped for lack of a test.** Where no vendored corpus deck
exercises a Phase-8 verb/option/mode, the test is **synthesized** — copy the nearest
corpus deck (or hand-write a minimal one), add the missing command, save it as a
fixture, and gate it through **both** engines (`gen_phase8.py` + `golden_phase8.rs`,
exactly as the Phase-7 protection / fault-study / UPFC goldens already synthesize their
scenarios). Test-absence is **never** a reason to defer a port (the WP7.9
AutoAdd/Monte/LD/Feeder "zero corpus cases → skip" was the anti-pattern, corrected
separately). The only legitimate deferrals are a real phase boundary (Phase 9 exotics),
a safe-Rust limit (DLLs/GUI), or code **proven dead upstream by an oracle probe** —
never "we have no test for it."

## 2. Phase-wide design decisions

### 2.1 A `report/` module: read-only formatters over disjoint borrows

PORTING_PLAN's module tree already reserves `dss-core/src/report/`. It does not
exist yet — create it as the home for all Phase-8 output:

```
dss-core/src/report/
  mod.rs        # do_{export,show,save,dump}_cmd dispatch (the ExportOptions/
                #   ShowOptions routers); the report option name tables
                #   (TExportOption/TShowOption) + abbreviation matching
  format.rs     # the Pascal number/string formatters used across reports
                #   (Format('%...g'), FloatToStrf, CmplxArrayToString,
                #   PowerFlowToStr, separators) — TODO(compat) on any quirk
  output.rs     # output-directory + file-handle machinery (§2.2)
  export/       # one submodule (or grouped file) per ExportResults procedure
  show/         # one per ShowResults procedure
  save.rs       # Circuit.Save + WriteClassFile + DumpProperties (WP8.5)
  reduce.rs     # ReduceAlgs (WP8.7) — or under support/ if it grows
```

Reports are **read-only over the solved circuit**: each formatter takes the same
disjoint-borrow context the solve/control code uses (a read view of the registry +
solution arrays), walks elements in the Pascal's iteration order, and writes
formatted lines. No formatter mutates shared electrical state. The one nuance: the
**fault-study reports** (`Export/Show Faultstudy`) read the **precomputed** bus
`Zsc`/`Ysc`/`BusCurrent` that a prior `Solve mode=faultstudy` (WP7.9) populated —
`ExportFaultStudy` (`ExportResults.pas:1526`, "Isc has been previously computed") does
only small **local per-bus `YFault` scratch inversions**, never a global re-solve and
never hidden mutation. `DoExportCmd` calls it directly with no solve, so the port reads
precomputed state (faithfully erroring/garbage if no faultstudy ran, like Pascal) — it
must **not** silently re-run the study.

`report/mod.rs` is the executive's new dispatch target. Today `Export`/`Save`/`Dump`
(and `Plot`/`Visualize`) fall through to the `not_ported_command` catch-all in
`exec/command.rs`, while `Show` and `Panel` are already no-op stubs
(`command.rs:60-71`). Phase 8 replaces these with explicit arms calling
`report::do_{export,show,save,dump}_cmd` (and turns `Plot`/`Visualize` into faithful
no-ops, §2.5). The Export/Show **option tables** mirror `TExportOption`/`TShowOption`
ordinal order with `TCommandList` abbreviation matching, exactly like the existing
`EXEC_COMMANDS`/`EXEC_OPTIONS` tables in `exec/tables.rs` — scripts rely on
abbreviations (`export v`, `show volt`).

### 2.2 Output directory & file handling (the one piece of new machinery)

Pascal writes reports to `DSS_OutputDirectory` / the per-circuit working dir, with
filenames it returns to the caller and (in GUI builds) auto-opens. Port the
**directory + filename** half; drop the auto-open (`FireOffEditor` → no-op, like the
GUI no-ops). Decisions:

- A `report::output` helper resolves the output path the way Pascal's
  `GetOutputDirectory`/`DSSDataDirectory`/`CurrentDSSDir` do: respects `Set DataPath=`
  and `Set CaseName=` (both already in the `EXEC_OPTIONS` table — wire the handlers),
  defaulting to the circuit's compile directory. **Probe the oracle** for the exact
  default-filename spelling per report (e.g. `<circuit>_EXP_VOLTAGES.csv`,
  `<circuit>_Mon_<name>_1.csv`) — these are observable and corpus `Export` decks
  sometimes re-`Redirect` the emitted file, so the name matters.
- `Export <x> <optional filename>` honours an explicit filename argument
  (`DoExportCmd` parses a trailing path); default otherwise.
- **Tests never write into the source tree.** The harness points `DataPath` at the
  session scratchpad / a `tempfile` dir (the proven `tempfile` crate, a dev-dep); the
  golden gate reads the produced file back from there. Port Pascal's `DoExportCmd`
  setting `GlobalResult` to the produced filename (the CLI/echo already surfaces
  `GlobalResult`); expose a `Dss` accessor for the last-written report path.
- CSV writing uses the **`csv` crate** — PORTING_PLAN §1.5 already blesses it ("prefer
  existing crates over reinventing"); it isn't in the workspace deps yet, so WP8.1
  adds it to `[workspace.dependencies]` + `dss-core`. The targeted gate parses numbers
  out of both files (§2.3), so the crate's
  RFC-4180 quoting/spacing never has to byte-match Pascal's `Writeln`/`Format` — the
  proven crate is both cleaner and gate-safe. Hand-roll only a spot where a report's
  exact layout is genuinely load-bearing and `csv` can't express it (record any such
  spot in STATUS). `Save`/`Dump`/`Show` are **not** CSV and have no fitting crate: `Save`/`Dump` emit
  OpenDSS *script* / property-dump text — a DSL with no standard-format serializer,
  assembled straight from the Phase-2 property getters (so "by hand" is thin string-
  joining, not reinvented formatting); `Show` emits Pascal-pinned fixed-width tables
  **plus** matrix dumps (`ShowLineConstants`) and tree/log layouts (`ShowMeterZone`/
  `ShowEventLog`) that a generic table crate (`tabled`/`comfy-table`) only partly fits
  and can't pad to Pascal's exact widths — both go through `format.rs`. Re-evaluate a
  table crate at WP8.4 if a large uniform-table `Show` subset makes it pay (the
  comparator parses tokens, so either choice is gate-valid).

### 2.3 The text/CSV comparison harness — Phase 8's defining new gate

`harness/mod.rs` today compares **numeric JSON skeletons** (`numeric_skeleton` /
`assert_value_matches_tol`, the voltage/power/energy tolerances). Phase 8 adds a
**file-output comparator** built on the same number-parsing core, honoring
PORTING_PLAN §4 ("text outputs compared after parsing numbers out, never raw
float-string diffs"):

- `compare_export(oracle_path, rust_path, policy)`:
  1. tokenize both files into rows/fields with the `csv` reader (or the report's
     separator for the non-CSV `Show` text);
  2. assert the **header row tokens are equal** (the column set/order is the report's
     contract) and the **row count is equal**;
  3. for each field: if it parses as a number on both sides, compare with the
     tolerance policy (reuse `assert_value_matches_tol`); else compare as text —
     **case-insensitively for identifiers** (element/bus names), exact for fixed
     labels. Element/row **ordering** is exact (Pascal iteration order is observable)
     unless a report is explicitly order-independent (probe).
- Field-specific exceptions (a column with a known upstream quirk, a non-deterministic
  timestamp/path column to mask) go in `tests/TOLERANCE_NOTES.md` — **no blanket
  relaxation**, same discipline as the numeric gate.
- This means Phase 8 does **not** need byte-exact float formatting in `format.rs` to
  pass the gate — but `format.rs` is still ported faithfully (digit counts, field
  widths) because `Save` output must re-parse and because diffs read far cleaner when
  the skeletons line up. Any deliberately-reproduced format quirk (truncated width,
  FPC `Format` rounding) is `TODO(compat)`.

### 2.4 Save = re-emit DSS script; gate by round-trip, not oracle-byte-match

`Circuit.Save` (`SaveDSSObjects`/`SaveVoltageBases`/`SaveMasterFile`/`SaveBusCoords`/
`SaveOpenTerminals`) and `Save <class>` (`WriteClassFile` in `Utilities.pas`) write
the circuit back out as `.dss` scripts by dumping each object's properties in
`PrpSequence` order. The port reuses the **existing property-dump machinery** (the
same `?`/props-roundtrip getters Phase 2 built) — `Save` is "for every object, write
`New <class>.<name> ` + each set property." The gate is **round-trip through our own
parser** (re-compile + re-solve = identical), which is stronger and more robust than
matching the oracle's exact bytes (the oracle's separator/ordering choices are not
behavior). `DumpProperties` (the `Dump` command) is the same getter set to a single
file/stream.

### 2.5 Plot / Visualize / Panel — faithful headless no-op

`Plot`, `Visualize`, `Panel` are GUI commands. In the pinned headless oracle they
produce **no engine-observable state** (the live gate compares the model, and the
oracle plots nothing in headless mode), so porting them as a **documented no-op is
faithful, not a fake** — identical justification to the already-committed `Show`
no-op, and it is what lets the `Export,Plot` decks migrate once `Export` is real.
`Panel` is already a no-op (`command.rs:60`); WP8.1 adds `Plot`/`Visualize`. Each
no-op carries a one-line comment citing the Pascal `DoPlotCmd`/`DoVisualizeCmd` and
"headless-faithful per the live gate." Distinguish sharply from `Export`/`Show`/
`Save`, which are **not** no-ops — they produce files the targeted golden checks.

### 2.6 Demand-interval / SystemMeter files (carried from Phase 6)

Phase 6 deferred to Phase 8 the EnergyMeter demand-interval (`DI_`) files, the
phase-voltage report files, the overload/voltage-exception report files, **and** the
circuit-wide `TSystemMeter` system aggregate — both its **register-accumulation core**
(`TSystemMeter.TakeSample`/`Integrate`, hooked into the meter `SampleAll` sweep — *not*
landed in Phase 6: STATUS §7 "Still Phase 8: the `SystemMeter` register core"; the only
Rust trace is the deferral note at `solution/meters/sampling/take_sample.rs:21`) **and**
its file output. The per-zone EnergyMeter registers accumulate in Phase 6; the
`TSystemMeter` does not — so WP8.3 ports the **core first**, then the DI/SystemMeter
file writers over it, gated by the same CSV comparator. The controlling `Set` options
(`DemandInterval`, `DIVerbose`, `Overloadreport`, `Voltexceptionreport`,
`SampleEnergyMeters`) already sit in `EXEC_OPTIONS`; wire the handlers.

### 2.7 Reused architecture (no re-explanation in the WPs)

- **Element iteration order** — every report walks the registry in creation /
  `ProcessBusDefs` order (the order the solve loop and the existing element-dumps
  already use); node order is `YNodeOrder`.
- **Property dump** — `Save`/`Dump` reuse the Phase-2 `?`/props getters.
- **Monitor stream** — `Export Monitors` / `Show monitor` read the Phase-6 in-memory
  f32 record buffer + headers (`TranslateToCSV`).
- **Abbreviation matching** — `TCommandList` semantics, as in `exec/tables.rs`.
- **`TODO(compat)`/`NOT_PORTED`** discipline; **probe the oracle** for every
  filename/column/format question.

## 3. Work packages

> Effort % = share of Phase 8. Each WP ends gate-green with its targeted text/CSV
> golden + (where it lands a verb) a live-corpus migration. Pascal line numbers
> confirmed at WP open.

---

### WP8.1 — Report infrastructure: `report/` module, output paths, the CSV/text harness, GUI no-ops [12%]

**Pascal:** `Executive/ExportOptions.pas` (`TExportOption` enum + `DoExportCmd`
router), `Executive/ShowOptions.pas` (`TShowOption` + `DoShowCmd`),
`Common/Utilities.pas` (`GetOutputDirectory` + number formatters), `Common/DSSGlobals`
(`DSSDataDirectory`/`OutputDirectory`/`GlobalResult`), `DoPlotCmd`/`DoVisualizeCmd`.

Steps:
1. `report/mod.rs` + the `TExportOption`/`TShowOption` name tables with abbreviation
   matching; route `exec/command.rs` `Export`/`Dump`/`Save` and the `Show` arm into
   `report::do_*_cmd` (each dispatching to a per-keyword stub that records a *scoped*
   `NOT_PORTED("<keyword> — WP8.x")` until its WP lands — so a half-ported Export
   never silently emits nothing).
2. `report/output.rs` — output-directory resolution (`DataPath`/`CaseName` handlers
   wired into `set_cmd.rs`), explicit-filename parsing, `GlobalResult` = produced
   path, the scratchpad redirect for tests. No auto-open.
3. `report/format.rs` — the shared number/string formatters (probe the oracle for the
   exact `Format` spec per use; `TODO(compat)` on truncations).
4. `harness/mod.rs` — `compare_export` (§2.3) + `tests/golden_phase8.rs` skeleton;
   `tools/golden/gen_phase8.py` skeleton (oracle writes a report to a temp dir,
   captures bytes into `tests/golden/phase8/`).
5. **Plot/Visualize faithful no-op** (§2.5) with citations; a regression test that a
   `Plot`-containing micro deck solves clean and the model is unchanged.
6. Gate: no numeric regressions; the harness self-tests on one trivial export (e.g.
   `Export Counts` — tiny, no solve dependency) round-tripped through the new golden
   path; migrate any **pure-`Plot`** corpus decks now.

---

### WP8.2 — Export: solution outputs [18%]

**Pascal:** `Common/ExportResults.pas` — `ExportVoltages`, `ExportCurrents`,
`ExportPowers`, `ExportSeqVoltages`/`ExportSeqCurrents`/`ExportSeqPowers`,
`ExportLosses`, `ExportPbyphase`, `ExportTaps`, `ExportSummary`, `ExportCounts`,
`ExportNodeOrder`/`ExportNodeNames`, `ExportYNodeList`, `ExportVoltagesElements`,
`ExportElemVoltages`/`ExportElemCurrents`/`ExportElemPowers`, `ExportResult`,
`ExportBusCoords`, `ExportY`/`ExportYprim`, `ExportSeqZ`; `DoExportCmd` dispatch.

Steps:
1. The bus/node solution exports (`Voltages`, `puVoltages`, `SeqVoltages`,
   `VoltagesElements`, `NodeOrder`, `NodeNames`, `YNodeList`, `BusCoords`) — pure
   reads of `node_v` + the symmetrical-components helper (already ported).
2. The element exports (`Currents`, `Powers`, `SeqCurrents`, `SeqPowers`,
   `P_byphase`, `ElemVoltages`/`ElemCurrents`/`ElemPowers`, `Losses`, `Taps`) — walk
   elements in creation order, reuse the existing I/P/loss getters.
3. The matrix/summary exports (`Y`, `Yprims`, `SeqZ`, `Summary`, `Counts`, `Result`)
   — `Y`/`Yprims` serialize the assembled/per-element matrices the checkpoint gate
   already exposes.
4. Gate: targeted `phase8/export_{voltages,currents,powers,seq*,losses,taps,
   summary,…}.csv` vs the oracle on solved **IEEE13/34/37/123**, plus the bus/summary
   exports (`Voltages`/`Summary`/`Counts`) on **IEEE8500** — completing the
   PORTING_PLAN §Phase 8 export-diff over 13/34/37/123/8500 (the 8500 per-element/matrix
   dumps are omitted as enormous, the established 8500-golden discipline). Migrate the
   `Export`-tagged **solution-report** decks; `COVERAGE.md` refresh.

---

### WP8.3 — Export: monitors, meters, DER, reliability, fault study, demand-interval files [16%]

**Pascal:** `Common/ExportResults.pas` — `ExportMeters`, `ExportGenMeters`,
`ExportLoads`, `ExportPVSystemMeters`, `ExportStorageMeters`, `ExportEventLog`,
`ExportErrorLog`, `ExportFaultStudy`, `ExportCapacity`, `ExportOverloads`,
`ExportUnserved`, `ExportBusReliability`/`ExportBranchReliability`, `ExportSections`,
`ExportProfile`; `Executive/ExportOptions.pas` `DoExportCmd` → `Monitor.TranslateToCSV`
(the `Export Monitors` path, case 15); `Common/Utilities.pas` `DumpAllocationFactors`;
`Meters/EnergyMeter.pas` demand-interval + `TSystemMeter` core/file writers (Phase-6
carry-forward, §2.6).

Steps:
1. `Monitors` — `TranslateToCSV` over the Phase-6 f32 stream + headers (the harmonic
   header already labels Freq/Harmonic from WP7.6); per-channel CSV.
2. `Meters`/`Generators`/`Loads`/`PVSystem_Meters`/`Storage_Meters` — register dumps
   over the Phase-6/7 register arrays + names.
3. `EventLog`/`ErrorLog` (the event-log line format already pinned by the Phase-5
   control gates — reuse), `Faultstudy` (read-only over the WP7.9-precomputed bus
   `Zsc`/`Ysc`/`BusCurrent` — local `YFault` scratch inversions only, no re-solve;
   §2.1), `Capacity`/`Overloads`/`Unserved`,
   `BusReliability`/`BranchReliability`/`Sections` (the WP7.2 `RelCalc` outputs),
   `AllocationFactors`, `Profile`.
4. The `TSystemMeter` register-accumulation core (`TSystemMeter.TakeSample`/`Integrate`
   + the `SampleAll` hook — not landed in Phase 6, §2.6), **then** the demand-interval /
   `SystemMeter` file writers over it + their `Set` option handlers.
5. Gate: monitor CSV channels elementwise vs the oracle, registers vs the oracle;
   migrate the `Export`-tagged monitor/meter/DER decks (incl. the harmonics decks
   blocked only by a trailing `export monitor`); `COVERAGE.md` refresh.

---

### WP8.4 — Show reports [14%]

**Pascal:** `Common/ShowResults.pas` (`ShowVoltages`, `ShowCurrents`, `ShowPowers`,
`ShowLosses`, `ShowBuses`, `ShowElements`, `ShowRegulatorTaps`, `ShowMeters`,
`ShowMeterZone`, `ShowFaultStudy`, `ShowIsolated`, `ShowLoops`, `ShowLineConstants`,
`ShowYprim`/`ShowY`, `ShowTopology`, `ShowNodeCurrentSum` (mismatch),
`ShowkVBaseMismatch`, `ShowRatings`, `ShowVariables`, `ShowControlledElements`,
`ShowResult`, …); `Solution.WriteConvergenceReport`; `ControlQueue.WriteQueue`;
`Executive/ShowOptions.pas` `DoShowCmd`.

Steps:
1. Replace the blanket `Show` no-op with the `DoShowCmd` dispatcher. `Show panel` is
   **not** a no-op — the oracle faithfully errors it (`ShowOptions.pas:248`, error 999:
   `Command "show panel" is not supported in DSS-Extensions`); reproduce that error
   (§4). Port the solution-report Shows (Voltages/Currents/Powers/Losses/Buses/Elements/
   Taps) first — they share field logic with the WP8.2 exports.
2. Topology/diagnostic Shows (`Zone`, `Isolated`, `Loops`, `Topology`, `Mismatch`,
   `kvbasemismatch`, `Convergence`, `controlqueue`) over the Phase-6 CktTree + the
   solution residual.
3. `LineConstants`, `Yprim`, `Y`, `Ratings`, `Variables`, `Controlled`, `Result`,
   `EventLog`, `Faults`, `Meters`, `Generators`.
4. Gate: targeted `phase8/show_*.txt` (numeric-skeleton diff) on a solved feeder;
   migrate the pure-`Show` corpus decks (the geometry/cable `Show LineConstants`
   family + `Dump,Show`); `COVERAGE.md` refresh.

---

### WP8.5 — Save circuit + `Save <class>` + Dump [14%]

**Pascal:** `Common/Circuit.pas` `Save`/`SaveDSSObjects`/`SaveVoltageBases`/
`SaveMasterFile`/`SaveBusCoords`/`SaveOpenTerminals`/`SaveFeeders`;
`Common/Utilities.pas` `WriteClassFile`; `Executive/ExecHelper.pas` `DoSaveCmd`,
`DumpProperties` (the `Dump` command).

Steps:
1. `report/save.rs` — `Circuit.Save` over the property-dump getters (§2.4):
   `SaveDSSObjects` (every object as `New …` + props in `PrpSequence` order),
   `SaveVoltageBases`, `SaveMasterFile` (header `Clear`/`Set` preamble + `Redirect`
   list + footer `MakeBusList`/`Set Voltagebases`/`CalcVoltageBases`/`Solve`),
   `SaveBusCoords`, `SaveOpenTerminals`, `SaveFeeders`. If an oracle probe proves
   Feeder objects are never instantiated (dead upstream, as Phase 6/7 found),
   `SaveFeeders` is a faithful empty path — documented via that probe, **not** skipped
   for lack of a corpus deck (§1).
2. `Save <class>` via `WriteClassFile`; `Save meters`/`Save voltages` special forms
   (`DoSaveCmd` branches).
3. `Dump` (`DumpProperties`) — the `?`-getter set to one file (`Dump <class>.<name>`
   and `Dump all`) and the `Dump debug` variant; synthesize a fixture for any variant
   the corpus doesn't exercise (§1).
4. Gate: `save_roundtrip.rs` — `Save circuit` on IEEE13/37/123 re-compiles on our
   engine and re-solves to identical voltages (§1 gate #3); migrate `Save`/`Dump`
   corpus decks; targeted golden for one `Dump` output.

---

### WP8.6 — Executive tail: BatchEdit, Interpolate, Distribute, Uuids, GISCoords, … [12%]

**Pascal:** `Executive/ExecHelper.pas` `DoBatchEditCmd`, `DoInterpolateCmd`
(`MetObj.InterpolateCoordinates`), `DoDistributeCmd`, `DoUuidsCmd`; `Executive/
ExecCommands.pas` inline dispatch for `MakeBusList` (`:541`) and `GISCoords` (`:639`);
plus the remaining `ExecCommands`/`ExecOptions` tail — drive from `tools/cmd_coverage.py`
(built this WP per PORTING_PLAN §Phase 8) to *prove* coverage; where a needed verb has
no corpus deck, synthesize a fixture (§1), never skip it.

Steps:
1. **Build `tools/cmd_coverage.py`** (PORTING_PLAN §Phase 8): enumerate every DSS
   command/option used across the vendored corpus, cross-referenced with what the
   engine already dispatches, to produce the *exact* tail list and prove coverage.
   Drive the rest of this WP (and the WP8.8 sweep) from its output.
2. **BatchEdit** (`DoBatchEditCmd` — `batchedit type=<class> name=<regex> <editstring>`)
   — select by a **case-insensitive, unanchored regex** over each class object's name
   (Pascal `TRegExpr` with `ModifierI := TRUE` + `Exec` = partial match,
   `ExecHelper.pas:292`), then replay the edit string against every match in creation
   order. Use the proven **`regex` crate** (`(?i)` + `is_match` mirrors `TRegExpr.Exec`'s
   unanchored semantics); `TODO(compat)` only if a corpus pattern hits a `TRegExpr`-vs-
   `regex` flavor gap (probe). No report infra needed; can land first in this WP.
3. `Interpolate` (bus-coordinate interpolation along zones — reuses the Phase-6
   `InterpolateCoordinates` if it landed; else port it here), `Distribute`
   (`Utilities` `WriteUniform`/`Random`/`EveryOther`/`ProportionalGenerators`),
   `Uuids`, `GISCoords`/`MakeBusList`, and the probed remainder.
4. Gate: BatchEdit/Interpolate/Distribute behavior vs the oracle on a corpus case
   (model compare via the live gate); migrate the `BatchEdit`-tagged decks;
   `COVERAGE.md` refresh.

---

### WP8.7 — ReduceAlgs (full) + `TLineObj.MergeWith` [8%]

**Pascal:** `Meters/ReduceAlgs.pas` (`DoReduceDefault`, `DoReduceShortLines`,
`DoReduceDangling`, `DoBreakLoops`, `DoMergeParallelLines`, `DoReduceSwitches`,
`DoRemoveAll_1ph_Laterals`, `DoRemoveBranches`, `IsShortLine`);
`Meters/EnergyMeter.pas` `ReduceZone`; `PDElements/Line.pas` `MergeWith` (the Phase-6
blocker).

Steps:
1. Port `TLineObj.MergeWith` (the series-line merge `ReduceZone` needs). The Phase-6
   `Reduce` work is **command-framing only** — `do_reduce_cmd` reproduces the meter
   lookup / error-1890 / `'A'`-dispatch, but **every** `ReduceZone` strategy (incl.
   `DoReduceDefault`) is `NOT_PORTED`, blocked on `MergeWith` (`exec/solve.rs:227-235`;
   STATUS §7). So WP8.7 ports the strategies **including** `DoReduceDefault`.
2. The reduction strategies + the `Set ReduceOption=` selector (already in
   `EXEC_OPTIONS`) + `KeepList` (`DoKeeperBusList`).
3. Gate: post-`Reduce` branch/node counts + a re-solve vs the oracle on a metered zone
   (extend `golden_autoadd_reduce.rs`); migrate any `Reduce`-tagged corpus decks.

---

### WP8.8 — Phase exit [6%]

1. `rg "TODO\(compat\)"` / `rg "NOT_PORTED"` / `rg "TODO\(WP8\)"` sweep — every
   remaining site points at its phase (Phase 9 CIM/GIC/A-Diakoptics/exotics, or
   "never" for DLLs/GUI).
2. Run `tools/cmd_coverage.py` to **prove tail coverage** of the corpus command/option
   set (PORTING_PLAN §Phase 8 deliverable); document the residual.
3. Re-run the full suite + the **always-on live corpus compare**; run a final
   `DSS_LIVE_CLASSIFY=1` pass and migrate every newly-unblocked deck. Refresh
   `tests/corpus/COVERAGE.md`.
4. Update `PORTING_PLAN.md` §Phase 8 cross-links; rewrite `STATUS.md` (Phase 8 record;
   "next = Phase 9 exotics — optional; stopping here is a complete usable simulator",
   per PORTING_PLAN §Phase 9 / cumulative note).
5. Merge to `main` (`--no-ff`, the per-phase convention) — **only on explicit user
   request**.

## 4. Deferred in this phase (pointing forward)

- **Phase 9 (exotics)** — the CIM XML exports `CIM100`/`CIM100Fragments`
  (`ExportCIMXML.pas`, explicitly Phase 9 per PORTING_PLAN); the
  A-Diakoptics exports `IncMatrix`/`IncMatrixRows`/`IncMatrixCols`/`BusLevels`/
  `Laplacian` and the `DSS_CAPI_ADIAKOPTICS`-only `ZLL`/`ZCC`/`Contours`/`Y4`; the GIC
  export `GICMvars` (GIC elements are Phase 9); `Pstcalc` flicker outputs (Monitor
  mode 4). Each remains a scoped `NOT_PORTED("… — Phase 9")` Export keyword, never a
  silent no-op.
- **Faithfully-errored upstream** — the `CDPSMAsset`/`CDPSMElec`/`CDPSMGeo`/
  `CDPSMTopo`/`CDPSMStateVar` exports ("no longer supported") and `Show panel` /
  `Plot`-GUI: reproduce the oracle's exact error/no-op, with a probe to confirm.
- **Parallel-machine / actor (`DSS_CAPI_PM`)** options & commands (`NumCPUs`,
  `NewActor`, `SolveAll`, `ConcatenateReports`, …) — Phase 9 (actor mode), kept as
  documented unsupported.
- **Never (safe-Rust / GUI)** — `FireOffEditor` auto-open, `DOScmd`, `TOP`/`DI_plot`
  external-tool launches, all user-model DLL hooks.
- **On demand / proven-dead** — `MakePosSequence` (ported where a consuming path
  reaches it; unreached in Phase 8 unless an export forces it); `SaveFeeders` / Feeder
  objects (ported; *proven* dead upstream by an oracle probe — Feeder never
  instantiated — **not** skipped for lack of a deck); the binary shape-file outputs
  (`SngSave`/`DblSave`, carried from Phase 5) — port with a **synthesized** fixture
  (§1), since no corpus deck exercises them.
