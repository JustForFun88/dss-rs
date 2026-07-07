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
> **Stop-and-confirm cadence:** after each small step (a WP or a self-contained
> sub-step) run the per-step ritual below **autonomously, without pausing between
> its sub-steps**; the single stop point is at the very end — then wait for the
> user's explicit confirmation (unless the user authorized several WPs in one
> pass).
>
> **Per-step ritual (do every step, in order, without being told):**
> 0. **Tier check** (added 2026-07-06; protocol: `PLAN_SEQUENCE.md` §Model-tier
>    protocol). Look up the step's **exec tier** in the per-WP tier table (§0
>    below); most Phase-8 steps are deliberately **Sonnet-executable**
>    (`sonnet-high+`), two are `opus-medium+`. Audit tier is **`opus-high+`**
>    everywhere — spawn the auditors with an explicit model/effort override.
>    If the session is below the step's exec tier, do NOT execute; reply
>    exactly: «Этот шаг требует <exec tier>. Переключи сессию (/model +
>    reasoning effort) и повтори команду.» and stop.
> 1. **Gate green** — `cargo fmt --all --check`; `cargo clippy --workspace
>    --all-targets -- -D warnings`; `cargo test --workspace` (runs **all**
>    goldens + the always-on live corpus gates). No `#[ignore]`, no name-filter
>    that could green on zero matches; a red test blocks the commit.
> 2. **Update `STATUS.md`** (the §1 frontier + the phase record), **commit**
>    (code + STATUS together).
> 3. **`/audit-code` + `/audit-tests` in parallel** — two **fresh independent
>    agents, never forks** (a scoped agent reviews more sharply than one buried
>    in your context), **spawned with an explicit model/effort override matching
>    the audit tier above** (never "whatever the session runs"). Each gets a
>    self-contained brief: the step's commit range
>    (`<sha>^..HEAD`) or label, the diff, the authoritative Pascal units +
>    plan/STATUS sections, and the binding rules (the PIN, `TODO(compat)`/
>    `NOT_PORTED`, the oracle is the spec). Auditors are **read-only** and
>    return findings only; **you** settle each finding against the pinned
>    oracle, fix what is real, note the follow-up in `STATUS.md`, re-run the
>    gate, commit. A finding deliberately not fixed is **recorded in STATUS**,
>    never dropped; if an audit finds nothing, skip its commit (no empty
>    commits). For a trivial sub-step the inline audit skill is allowed.
> 4. **`STATUS.md` full review** — read it end to end; sync whatever the step
>    made stale (no two places disagreeing), archive dead weight to
>    `docs/phase-records/`, dedup restated paragraphs; `docs:` commit if
>    anything changed (gate re-run first).
> 5. **Only now stop** and report **in Russian** (code, identifiers, commit
>    messages and STATUS stay English): what landed, what the audits found and
>    how it was settled, gate status, next step.
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

1. **Report infrastructure + the comparison harness (WP8.1)** — ✅ COMPLETE.
2. **Export: solution outputs (WP8.2)** — ✅ COMPLETE.
3. **Export: device/meter/reliability outputs (WP8.3)** — ✅ COMPLETE.
4. **Show reports (WP8.4)** — ✅ COMPLETE.
5. **Save + Dump (WP8.5)** — 🚧 IN PROGRESS: Dump single-object forms + 6 leaf
   overrides landed; remaining = the 8 leaf overrides, the whole-circuit/aux
   Dump forms, all `Save` forms, the round-trip gate.
   5b. **Corpus property parity (WP8.5b, addendum)** — exhaustive
   per-element property-value comparison vs the pinned oracle (pilot
   report → gate flags); executes after WP8.5 completes. See §WP8.5b.
6. **Executive tail (WP8.6)** — BatchEdit, MakeBusList/GISCoords, SetBusXY +
   Interpolate, Distribute, Uuids + `Export Uuids`, `cmd_coverage.py`.
   Independent of the report infra — can be pulled earlier if convenient.
7. **ReduceAlgs full (WP8.7)** — `TLineObj.MergeWith`, the 8 reduction
   strategies, `Set KeepList=`, the `Remove` command.
8. **Phase exit (WP8.8)** — marker sweep, `cmd_coverage.py` tail-coverage proof,
   full re-run + live re-classify, merge.

WP boundaries are flex points (the Phase-7 convention): WP8.6 is independent of
the report infra and may be pulled earlier.

**Per-WP model tiers** (vocabulary + step-0 refuse protocol:
`PLAN_SEQUENCE.md` §Model-tier protocol; audit tier is `opus-high+` for every
row — auditors are spawned with an explicit model/effort override):

| WP / step | Exec tier | Why |
|---|---|---|
| WP8.1–8.4 | — (✅ complete) | landed |
| WP8.5 steps 3a, 3b, 4, 6 | `sonnet-high+` | mechanical ports: the pattern is established by the landed Dump steps 1–2, each item cites its Pascal unit:lines, gates are pre-wired (`dump3.dss`/`dump_capacitor.dss`/`save_forms.dss`) |
| WP8.5 **step 5** (`Save circuit` + round-trip gate) | **`opus-medium+`** | the one non-mechanical WP8.5 piece: whole-circuit script emission + round-trip re-compile/re-solve debugging (failures surface as downstream voltage diffs, not local errors) |
| WP8.6 (all steps) | `sonnet-high+` | each verb has a pre-validated deck (`tools/golden/phase8_decks/`, `tests/corpus/modes/`) with oracle-pinned expectations; Uuids is a byte-exact golden recipe |
| WP8.7 (ReduceAlgs + `MergeWith` + `Remove`) | **`opus-medium+`** | graph surgery on the meter-zone tree + numeric impedance merge (`TLineObj.MergeWith`); the staging manifests' oracle-verified post-reduce element lists are the binding spec; the 8 strategy decks catch wrong-shape results, but *why* a shape is wrong takes real debugging |
| WP8.8 (phase exit) | `sonnet-high+` | marker sweep + coverage proof + re-classify, mechanical |

**Pre-validated test fixtures for the remaining WPs** (authored up front so
each WP starts from a proven deck; every deck ran bit-identical across two
separate oracle processes and is feature-sensitive):

- **`tools/golden/phase8_decks/`** — fixture decks for the file-output gates
  (`dump3.dss`, `dump_capacitor.dss`, `save_forms.dss`, `interp.dss`,
  `distrib.dss`, `uuids.dss` + `uuids_pre.csv`; see its README). The WP that
  ports a verb wires its deck into `tools/golden/gen_phase8.py` (deck text →
  the golden `.meta.json`, the single source both engines replay) and adds the
  `golden_phase8.rs` test.
- **`tests/corpus/modes/`** — 12 live decks with `pending: true` and
  `wp: "WP8.6"/"WP8.7"` (`batchedit`, `midi_batchedit`, the 8 `reduce_*`
  strategy decks, `reduce_remove`, `midi_reduce`); manifest notes record the
  oracle-verified post-reduce element lists (merged names `l1~l2`/`b1||b2`,
  disabled partners, node counts). The family gate
  (`modes_cases_match_oracle` in `corpus_live.rs`) asserts each pending deck
  errors loudly today; the WP that ports the verb flips `pending: false` and
  proves the live compare green (GAPS_PLAN §3.1).

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

PORTING_PLAN's module tree reserves `dss-core/src/report/` as the home for all
Phase-8 output (built by WP8.1–8.5; the target layout):

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
  save/         # DumpProperties (dump.rs, landed) + Circuit.Save/
                #   WriteClassFile (save.rs, WP8.5 steps 4-5)
  reduce.rs     # ReduceAlgs (WP8.7) — or under solution/ if it grows
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

`report/mod.rs` is the executive's dispatch target (`Export`/`Show`/`Dump`/`Save`
arms landed in WP8.1; `Save` still routes to the `do_save_cmd` stub until WP8.5
step 4). The Export/Show **option tables** mirror `TExportOption`/`TShowOption`
ordinal order with `TCommandList` abbreviation matching, exactly like the
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

### WP8.1 — Report infrastructure [12%] — ✅ COMPLETE

Record: `STATUS.md` §1f (dispatch skeleton, output paths, `compare_export`
harness, `gen_phase8.py`, GUI no-ops).

---

### WP8.2 — Export: solution outputs [18%] — ✅ COMPLETE

Record: `STATUS.md` §1f (all solution/element/matrix exports; `solvable_now`
88→119).

---

### WP8.3 — Export: monitors, meters, DER, reliability, DI files [16%] — ✅ COMPLETE

Record: `STATUS.md` §1f (device/meter/reliability/log exports + the
`TSystemMeter` core + DI files; `solvable_now` 119→168).

---

### WP8.4 — Show reports [14%] — ✅ COMPLETE

Record: `STATUS.md` §1f (~32 `Show` reports, steps 1–16 + finalize;
golden_phase8 125).

---

### WP8.5 — Save circuit + `Save <class>` + Dump [14%] — 🚧 IN PROGRESS

**Done (record: STATUS.md §1 frontier + §1f):** Dump steps 1–2 — the
single-object forms `Dump <class>.[name|*] [debug]` (`report/save/dump.rs`
generic 3-kind base + `#903`/`#256`), the Reactor / Transformer / Line /
LineCode / LineGeometry / XfmrCode leaf overrides, the `READS_VTERMINAL`
refresh, `fmt_g`/`float_to_str` byte fixes. 16 byte-exact dump goldens.

**Remaining steps (3–6). Fixture decks are pre-validated in
`tools/golden/phase8_decks/` (README there records the probe-proven facts).**

**Step 3a — the 8 remaining leaf `DumpProperties` overrides.** Each is a
co-located `dump_body` dispatched from `report/save/dump/overrides.rs:35`
(add the downcast arm), exactly like the six already ported. All eight call
`inherited DumpProperties(F, Complete)` **dropping Leaf** (so the base does
NOT print props; the override prints all props itself via
`'~ '+PropertyName[i]+'='+PropertyValue[i]` — in Rust `ClassProps::get_value`),
then add their Complete extras:

- **Capacitor** (`Capacitor.pas:751-766`): all props; Complete →
  `SpecType=<int>`. **Upstream garbage** (probe-proven 2026-07-05, two-process
  diff): the oracle prints ASLR denormals in `~ CMatrix=(`/`~ FaultRate=`/
  `~ pctPerm=` for EVERY capacitor. Not reproduced (nondeterministic UB —
  CLAUDE.md rule): Rust renders the correct values; the golden
  (`dump_capacitor.dss`) is captured AND compared with those three line
  prefixes dropped on both sides; write the full report to
  `investigations/dump-propertyvalue-garbage.md` (cover the Reactor+meter
  case below in the same file).
- **Fault** (`Fault.pas:502-542`): custom lines `~ bus1=`/`~ bus2=` (first/
  next bus), `~ Phases=%d`, `~ R=%.2f` (= `1.0/G`), `~ pctStdDev=%.1f`
  (`StdDev*100`), `~ Gmatrix= (` lower triangle `%.3f ` with `|` row
  separators `)` only if set, `~ OnTime=%.3f`, `~ temporary= Yes/No`,
  `~ MinAmps=%.1f`, then the inherited tail (`NumPropsthisClass..Num`);
  Complete → `// SpecType=%d`.
- **Vsource** (`VSource.pas:1137-1165`): all props; Complete → blank,
  `BaseFrequency=%.1f`, `VMag=%.2f`, `Z Matrix=` lower triangle
  `%.8g +j %.8g `.
- **UPFC** (`UPFC.pas:1028-1056`): same shape as Vsource but NO `VMag` line.
- **RegControl** (`RegControl.pas:682-698`): all props; Complete →
  `! Bus =` + `GetBus(1)` + blank line.
- **Monitor** (`Monitor.pas:1811-1849`): all props; Complete → blank,
  `// BufferSize=`, `// Hour=`, `// Sec=`, `// BaseFrequency=%.1g`,
  `// Bufptr=`, `// Buffer=` + the raw sample floats (`WriteStr %0:1`, i.e.
  1 decimal), wrapped every `2 + Fnconds*4` values.
- **EnergyMeter** (`EnergyMeter.pas:2082-2119`): all props; Complete →
  `Registers` heading + per register `"%s" = %.0g`, then `Branch List:`
  walking `BranchList` — per PD branch `Circuit Element = <name>` + nested
  `   Shunt Element = <fullname>`.
- **Spectrum** (`Spectrum.pas:326-347`): all props; Complete →
  `Multiplier Array:`, header `Harmonic, Mult.re, Mult.im, Mag,  Angle`,
  per-harmonic `%-g` fields.

Also: correct each class's `PropDef` name literals to the oracle display case
(the `elements/pd/reactor/mod.rs:76` convention) as its golden lands — the
bare-dump golden (step 3b) additionally forces the pass for every class in
`dump3.dss` **and** the default library objects (LoadShape/GrowthShape/
Spectrum/TCC_Curve defaults). The two overrides NOT in this list belong to
unported classes and land with their GAPS_PLAN ports: AutoTrans
(`AutoTrans.pas:1246`, WPG.15) and GICLine (`GICLine.pas:627`, WPG.16) —
cross-reference them there, do not port here.

**Step 3b — the whole-circuit / aux Dump forms** (replace the stub at
`exec/report.rs:1754-1767`; Pascal `DoPropertyDump`, `ExecHelper.pas:1194-1396`).
Keyword dispatch on the first param (exact `CompareText` except `alloc*` =
first-5-chars match):

1. `commands` → `DumpAllDSSCommands` (`Utilities.pas:821-872`): file always
   `<OutputDir>DSSCommandsDump.txt`; sections `[execcommands]` /
   `[execoptions]` / one `[<ClassName>]` per class; lines `i, "name", "help"`
   with help run through `ReplaceCRLF`. **Help data**: the pinned oracle loads
   it from the gettext catalog shipped in the dss-python wheel
   (`dss/messages/properties-en-US.mo`, 1697 entries; keys `Command.<name>`,
   `Option.<name>`, `<Class>.<prop-lowercase>`; lookup falls back through
   `ClassParents` and returns the KEY itself on a miss —
   `DSSClass.pas:2166-2200`, `DSSGlobals.pas:719-724`). Add
   `tools/golden/gen_help_catalog.py`: parse the `.mo` from the pinned wheel
   → emit `crates/dss-core/src/report/help_catalog.rs` (a generated static;
   regenerate manually with the PIN only, same rule as goldens). The dump is
   then a pure formatter over the catalog + the existing command/option/prop
   tables.
2. `buslist` → `<OutputDir>Bus_Hash_List.txt`, `BusList.DumpToFile`;
   `devicelist` → `<OutputDir>Device_Hash_List.txt`, `DeviceList.DumpToFile`
   (both err 255 on open failure). This byte-pins **THashList internals**
   (`Shared/HashList.pas` — port `MakeHash`, the bucket layout and
   `DumpToFile`'s three sections: `Number of Hash Lists = N, Number of
   Elements = M`, the per-bucket distribution + members `"name"  Idx= i`,
   then `LINEAR LISTING...`). Empirically the bus list printed only the
   LINEAR section while the device list printed all three — read
   `HashList.pas` to find the structural reason (likely sub-list allocation
   size) and reproduce it, don't special-case.
3. `alloc*` → `<OutputDir>AllocationFactors.txt`, `DumpAllocationFactors`
   (`Utilities.pas:784-819`): per load, `ConnectedkVA`-spec →
   `Load.<name>.AllocationFactor=%-.5g`, `kwh`-spec → `Load.<name>.CFactor=%-.5g`,
   any other LoadSpecType prints NOTHING (probe-proven: kW/PF loads absent).
4. `debug` → set the flag, fall through to the bare dump; `solution` →
   `Solution.DumpProperties(F, debug, Leaf=TRUE)`.
5. bare `Dump` / `Dump debug` → open `<OutputDir><CircuitName_>PropertyDump.txt`
   (err 255); if debug, `Circuit.DebugDump` header first (`Circuit.pas:
   2285-2319`: `NumBuses=`/`NumNodes=`/`NumDevices=`, `BusList:` per bus
   `Pad(name,12) (n Nodes) <nodes>`, `DeviceList:` per device `Pad(name,12)`
   + `  DISABLED`, `NodeToBus Array:`); then every `CktElements` in order,
   then every general `DSSObjs`, then `Solution.DumpProperties` — all with
   Leaf=TRUE. GlobalResult = the file path.
6. `Solution.DumpProperties` (`Solution.pas:1768-1891`): the `Set …` option
   list — port line-for-line; the Leaf-gated lines (`Set Mode=`, `hour`,
   `sec`, `year`, `circuit`, `editor`, `allowduplicates`, `voltagebases`)
   print only for `dump solution`/bare dump, NOT for the Save
   `IncludeOptions` path. `Set editor=NotePad.exe` is the pinned-oracle
   Windows default — reproduce the platform default string. Formats: mostly
   `%-g`; `%mean/%stddev` = `%-.4g` of value*100; `ueweight/lossweight`
   `%8.2f`; voltagebases `%10.2f` list; Complete additionally factors Y and
   dumps `System Y Matrix (Lower Triangle by Columns)` (`[%4d,%4d] = %12.5g +
   j%12.5g`) — bare `dump debug` passes Complete=debug, so this fires there.

**Step 3 gate:** wire `dump3.dss` + `dump_capacitor.dss` into `gen_phase8.py`
(one meta per dump form — the validated post-command list is in the
scratchpad validator and the phase8_decks README) + `run_deck_dump_exact`
tests, all byte-exact except the masked capacitor lines. Then the audits +
STATUS per the §0 ritual.

**Step 4 — `Save <class>` / `save` (meters) / `save voltages`** (replace the
stub `exec/report.rs:1733`; Pascal `DoSaveCmd`, `ExecHelper.pas:744-842`).
Parse via the `SaveCommands` table `['class','file','dir','keepdisabled']`
(positional-or-named; `keepdisabled` is parsed but IGNORED upstream —
`:780`); defaults `ObjClass=''`, `SaveDir=OutputDirectory`, `SaveFile=''`.
Branches, in order (`CompareTextShortest` prefix matching):

1. empty or `meters` → for every Monitor `Save` (flushes the in-memory
   sample buffer to the monitor's internal stream — NO file, probe-proven;
   port as the equivalent buffer flush or a documented no-op if the Rust
   monitor has no separate stream) + for every EnergyMeter `SaveRegisters`
   (`EnergyMeter.pas:1235-1269`): file `<OutputDir>MTR_<name>.csv`, header
   `Year, <year>,` then per register `"<RegName>",<value :0:0>`;
   GlobalResult = the RELATIVE `MTR_<name>.csv` (probe-proven), err 526.
2. `circuit` → `Circuit.Save(SaveDir)` (step 5).
3. `voltages` → `Solution.SaveVoltages` (`Solution.pas:2277-2315`): file
   `<OutputDir><CircuitName_>SavedVoltages.txt`, per node
   `<bus>, <nodenum>, <|V| %-.7g>, <ang %-.7g>`; GlobalResult = full path,
   err 488.
4. any class name → `WriteClassFile` (`Utilities.pas:1134-1210`): default
   filename = the bare class name (NO `.dss` extension — probe: `save load`
   → a file named `load`), `SaveDir + PathDelim + SaveFile` when dir given
   (mkdir, err 247); GlobalResult = the final SaveFile.

The serializer pair (shared with step 5; `report/save/save.rs`):
`WriteDSSObject` (`Utilities.pas:1221-1235`) = `New "Class.name"` (name
always double-quoted) + `SaveWrite` + ` ENABLED=NO` when a disabled
CktElement + mark `HasBeenSaved`; `SaveWrite` (`DSSObject.pas:145-165`) =
ONLY explicitly-set props, in set order (Rust: `DssObjData::
next_property_set`), skipping empty and the `----` sentinel, as
` <Name>=<CheckForBlanks(value)>`; `CheckForBlanks` (`Utilities.pas:1212`) =
quote with `"` only when the value contains a space and doesn't already
start with `(`/`[`/`{`/`"`/`'`. **This is a different serializer from
Dump** (all props, `~` lines) — do not conflate.

Gate: goldens over `save_forms.dss` — `MTR_em1.csv` (integer registers,
byte-exact viable), `svf_SavedVoltages.txt` + the `save load` file via
`compare_export` (7-sig magnitudes can straddle the faer/KLU last digit).

**Step 5 — `Save circuit` + the round-trip gate** (`Circuit.Save`,
`Circuit.pas:2409-2655`; on the command path `saveFlags = []`, so every
flag-gated branch — SingleFile/KeepOrder/IncludeOptions/SetVoltageBases/
IsOpen/IncludeDisabled/ExcludeDefault — is DORMANT; implement the flag enum
faithfully but only the empty-set path is reachable/gated):

1. Directory logic: `Save circuit` always has non-empty `Dir` (default
   `OutputDirectory`), so the classic `<Name>`/`<Name>000..999`
   (`Format('%.3d')`) fresh-subdir loop (`:2457-2478`) is UNREACHABLE from
   the executive — port it (C-API parity) but gate only the reachable
   branch: normalize/create `Dir`, `SetCurrentDSSDir` equivalent
   (`Dss::current_dir`), err 432 on create failure, restore the saved dir at
   the end (`:2652`).
2. Body order (`:2533-2614`): clear `SavedFileList` + all `HasBeenSaved`/
   class `Saved` flags; `WriteClassFile` the library classes in the verbatim
   order `wiredata, cndata, tsdata, linegeometry, linespacing, linecode,
   xfmrcode, loadshape, TShape, priceshape, growthshape, XYcurve, TCC_Curve,
   Spectrum, DynamicExp` (empty classes write nothing; a 0-record file is
   deleted and not listed — `:1191-1198`); `WriteVsourceClassFile`
   (`Utilities.pas:1088-1132`: FIRST vsource as `Edit "Vsource.source"`, the
   rest as `New`); `SaveFeeders` (`:2817-2856`: one subdir per ENABLED
   EnergyMeter named after it + `EnergyMeterObj.SaveZone` — read it in
   `EnergyMeter.pas` at WP open; the probe shows it writes `Branches.dss` /
   `Shunts…`/`Loads.dss`/`Capacitors.dss` and they join the Redirect list);
   `SaveDSSObjects` (`:2657-2706`: every remaining class via
   `WriteClassFile`); `SaveVoltageBases` (`:2708-2747`: file
   `BusVoltageBases.dss` = `Set VoltageBases=<get voltagebases result>` +
   `CalcVoltageBases` — reuse the ported `get` machinery); `SaveBusCoords`
   (`:2942-2988`: file `BusCoords.dss` ALWAYS created, CSV rows
   `<name>, %-g, %-g` only for coord-defined buses — probe: empty file when
   none); `SaveMasterFile` (`:2749-2815`: header `! Last saved by …` —
   write the port's own analogous stamp — `Clear`, `Set DefaultBaseFreq=`,
   `New Circuit.<name>`, conditional `Set Cktmodel/AllowDuplicates/
   LongLineCorrection`, `Set EarthModel=`; footer `Redirect <relative>` per
   `SavedFileList` entry, `MakeBusList`, `Redirect BusVoltageBases.dss  !
   set voltage bases`, `BusCoords BusCoords.dss` if the file exists).
   `SaveOpenTerminals` is `IsOpen`-gated → dormant, document only.
   GlobalResult `Circuit saved in directory: "<CurrentDSSDir>"`.
3. **Gate — NEW `crates/dss-core/tests/save_roundtrip.rs`** (§1 gate #3):
   on IEEE13/37/123 — solve, `save circuit dir=<tempdir>`, `clear`, compile
   the emitted `Master.dss`, re-solve: node voltages ≤1e-6 rel vs pre-save,
   iteration count exact. Round-trip through OUR parser, never byte-match vs
   the oracle's Save (§2.4). Plus a `save_forms.dss` structural test: the
   emitted file SET equals the oracle's probe-proven set (Master/
   BusVoltageBases/BusCoords/LineCode/LoadShape/…/em1/*).
4. `SaveFeeders`/Feeder note: the meter-zone subdir path above IS the
   feeder save; the Feeder CLASS stays dead upstream (Phase-6/7 probes) —
   nothing else to port.

**Step 6 — classify/migrate.** `DSS_LIVE_CLASSIFY=1` + `apply_classify.py`:
the two Dump-blocked decks (`Test/REACTORTest.DSS` — also unblocked by the
step-1 Reactor work, tag refresh; `IEEE-TIA-LV Model/Split-Phase_IEEE_TIA`)
+ anything Save unblocks; `COVERAGE.md` refresh.

---

### WP8.5b — Corpus property parity (addendum, 2026-07-07) [after WP8.5]

Closes the "properties are only spot-checked via opt-in `probes`" gap:
every element's **every** property value, Rust `?`-surface
(`refresh_vterminal_if_marked` + `ClassProps::get_value` — the byte-proven
WP8.5 Dump path) vs pinned-oracle `Properties(p).Val`, corpus-wide.
Executes **after WP8.5 completes** (steps 3b–5 finish property/Save-side
rendering — sweeping earlier just re-discovers known WP8.5 TODOs); cited by
the WP8.8 exit sweep. Design settled (plan `robust-stirring-engelbart`,
approved 2026-07-07):

1. Oracle (`tools/oracle/oracle_server.py`): opt-in request field
   `"all_properties": true` → per checkpoint `capture_all_properties`:
   per `AllElementNames` element, ordered `[[prop, str(Properties(p).Val)]]`
   over `AllPropertyNames` (property-index order is contract); reads happen
   after `capture_all_elements` (preserve established read order).
2. Rust accessor (`exec/view.rs`, additive):
   `element_properties(&mut self, full_name) -> Option<Vec<(String, String)>>`
   — resolve like `do_query_cmd`, per index `refresh_vterminal_if_marked` +
   `get_value` (the exact `?` path without executive round-trips).
3. Harness (additive): `PropsCap` + `compare_all_properties` — property name
   lists equal **in order** (case-insensitive; pins the property-table
   shape), each value via the existing `assert_value_matches_tol`
   (`compare_probe` semantics generalized). A documented `skip_props`
   table keyed `(class, prop)` for provably non-comparable props (path
   echoes etc.), each entry with proof cited in `tests/TOLERANCE_NOTES.md` —
   comparability exclusion, never tolerance loosening.
4. Phase A (pilot, report-first): env-gated `corpus_live_properties`
   (`DSS_LIVE_PROPS=1`, classifier shape) over solvable_now + asymmetric +
   controls with `all_properties` forced → `tmp/props_report.json`; triage
   (port bug → fix; unported class → NOT_PORTED cross-ref; non-comparable →
   skip_props+proof). Measure report size (8500-node class ≈ 1e4 els × ~50
   props × steps).
5. Phase B (gate): `#[serde(default)] compare_all_properties: bool` on
   `SolvableCase`, threaded through `run_and_compare` (additive block after
   probes); flip flags manifest-by-manifest (asymmetric + controls first,
   then solvable_now in tag groups); heavy cases stay off with a manifest
   note (coverage inventory, never silent). Suppress `all_properties` in
   `corpus_live_opendss`'s requests (one line + README note) — the EPRI
   channel would drown in the known `property-format-brackets` class.

---

### WP8.6 — Executive tail: BatchEdit, MakeBusList/GISCoords, SetBusXY+Interpolate, Distribute, Uuids [12%]

**Pascal:** `Executive/ExecHelper.pas` `DoBatchEditCmd:292`,
`DoInterpolateCmd:3106`, `DoDistributeCmd:3755` + `makeDistributedGenerators:3701`,
`DoUuidsCmd:4465`; `Common/Utilities.pas` `Write*Generators:1338-1531`;
`Meters/EnergyMeter.pas` `InterpolateCoordinates:2298`;
`Executive/ExecCommands.pas` `MakeBusList:541`, `GISCoords:639`;
`Common/ExportResults.pas` `ExportUuids:2871`.

**Dispatch prerequisite (first commit of the WP):** none of these commands has
a `cmd::` ordinal — they all fall through to `not_ported_command`
(`exec/command.rs:161`). Add the ordinals to `exec/tables.rs::cmd`
(BatchEdit, Interpolate, Distribute, Uuids, MakeBusList, GISCoords, SetBusXY;
indices = their 1-based positions in `EXEC_COMMANDS`) + the match arms.

Steps:

1. **`tools/cmd_coverage.py`** (PORTING_PLAN §Phase 8): enumerate every DSS
   command/option used across the vendored corpus, cross-referenced with the
   dispatched set, producing the exact tail list. Drive the rest of this WP
   (and the WP8.8 sweep) from its output.
2. **BatchEdit** (`ExecHelper.pas:292-341`). Parse via `GetObjClassAndName`
   (`:200-224`): one param, optionally named — the name must
   abbreviation-match `object` (else error 240 with `CRLF+CmdString`); split
   `Class.pattern` on the dot. `circuit` class → silent no-op. Unknown class
   → error 267 `BatchEdit Command: Object Type "%s" not found. %s`. Else:
   remember the parser position at the start of the trailing edit string,
   walk the class list in creation order, and for each object whose NAME
   matches the pattern **case-insensitively and UNANCHORED** (`TRegExpr`
   `ModifierI` + `Exec` = search anywhere; Rust: `regex` crate,
   `RegexBuilder::case_insensitive(true)` + `is_match`), replay the edit
   string against it (re-parse from the saved position ↔ in Rust: capture
   the remainder once, run the class `Edit` per match). **There is NO count
   message** — the command always returns 0 silently; do not invent one.
   `TODO(compat)` only if a corpus pattern hits a TRegExpr-vs-`regex` flavor
   gap (probe first). Gate: flip `tests/corpus/modes` `batchedit.dss` +
   `midi_batchedit.dss` to `pending:false` (expected: `LA` edits
   la1/la2/xla1 not lb1; `ld1` edits 11 loads; `^LD2$` exactly one), prove
   the live compare green (GAPS_PLAN §3.1); migrate the **39**
   `unsupported_command=BatchEdit` corpus decks.
3. **MakeBusList + GISCoords** (`ExecCommands.pas:541-544`, `:639-644`):
   `MakeBusList` = `if BusNameRedefined { reprocess_bus_defs() }` — nothing
   else; `GISCoords` = documented no-op ("Do nothing here on DSS C-API").
   Migrate the **13** MakeBusList- and **11** GISCoords-tagged decks (the
   ADiakoptics `Torn_Circuit` family — check the paired tags first; a deck
   also blocked by an ADiakoptics-only feature stays put with its tag
   corrected).
4. **SetBusXY + Interpolate.** `SetBusXY` (find `DoSetBusXYCmd` in
   `ExecHelper.pas` at WP open — trivial: bus/x/y params, `BusList.Find`,
   sets X/Y + `CoordDefined`, error if the bus doesn't exist yet).
   `Interpolate` (`ExecHelper.pas:3106-3163`): clear `Flg.Checked` on every
   ckt element; empty param → `'A'` = every ENABLED meter's
   `InterpolateCoordinates`, else the named meter (disabled → error 283,
   missing → error 277). `InterpolateCoordinates`
   (`EnergyMeter.pas:2298-2369`): guard `CheckBranchList` → error 529
   `'Meter Zone Lists need to be built. Do Solve or Makebuslist first!'`;
   per zone end, walk parent branches to find the two nearest
   coordinate-defined anchor buses, then `CalcBusCoordinates`
   (`:2371-2409`) spaces the in-between buses EVENLY
   (`Xinc=(X1-X2)/LineCount`) — port loop-for-loop. Gate: wire
   `phase8_decks/interp.dss` into `gen_phase8.py`; the golden is the
   `export buscoords` CSV after `interpolate` (anchors src/b1/b5/c2 →
   b2/b3/b4 + c1 filled; oracle-validated deterministic).
5. **Distribute** (`DoDistributeCmd:3755-3819` +
   `makeDistributedGenerators:3701-3753`). Param table
   `['kW','how','skip','pf','file','MW','what']`; defaults kW=1000,
   How=`Proportional`, Skip=1, PF=1, file=`DistGenerators.dss`; `MW` =
   kW×1000; `what` starting with `L` → loads AND unconditionally renames the
   output to `DistLoads.dss` (probe-proven — an explicit `file=` is
   overridden). Refuse to overwrite an existing file (error 721); header:
   `! Created with Distribute Command:` +
   `! Distribute kW=%-.6g PF=%-.6g How=%s Skip=%d  file=%s  what=%s` + blank.
   Dispatch on the first letter of How: `U`/`R`/`S`/else-P →
   `WriteUniform/Random/EveryOther/ProportionalGenerators`
   (`Utilities.pas:1338-1531`); every writer iterates the Load class in
   creation order, enabled loads only, one line
   `new generator.DG_%d  bus1=%s phases=%d kV=%-g kW=%-g PF=%-.3g model=1`
   (loads: `load.DL_%d`; Skip writer has a trailing space after kW —
   `:1473`). kW math: Uniform = kW/Count (÷3 if PositiveSequence); Skip =
   every (Skip+1)-th load, kW·kWBase/ΣkWBase over the selected; Proportional
   = kW·kWBase/ΣkWBase over all enabled. **`How=Random` is RNG-carried**
   (`randomize` + `random`, `:1400/:1415` — FPC time-seeded, probe-proven
   class): port it with a fresh entropy-seeded RNG, never golden-gate it
   (the GAPS_PLAN RNG rule). GlobalResult = the file name. Gate: wire
   `phase8_decks/distrib.dss` — four deterministic variants
   (Proportional/Uniform/Skip/what=Load; validated content in the probe
   record), `compare_export` numeric-token compare.
6. **Uuids + `Export Uuids`** (`DoUuidsCmd:4465-4535`;
   `ExportUuids`, `ExportResults.pas:2871-2960`; UUID plumbing
   `General/NamedObject.pas`). The command: read a comma-CSV of
   `<fullname>, <uuid>` lines (AuxParser, delimiter `,`; missing file →
   error 242); wrap a brace-less uuid in `{}`; a name containing `=` goes to
   the CIM exporter's hashed-key list (`AddHashedUuid`), else dispatch
   `circuit` / `Bus.<name>` (BusList find) / `<Class>.<name>` (SetActive)
   and set the object's UUID. Rust plumbing: a lazily-generated UUID slot on
   the object base + buses + circuit (`Get_UUID` creates a **random v4** on
   first read, `CreateUUID4`, `NamedObject.pas:47-52` — use getrandom/rand;
   never oracle-pinnable, which is why the fixture preloads everything).
   `Export Uuids` (keyword 25, default file `EXP_UUIDS.csv` →
   `<CircuitName_>` prefix applies): rows `<FullName> {UUID}` for circuit,
   every bus, every ckt element, then the linecode/wiredata/linegeometry/
   xfmrcode/linespacing/tsdata/cndata classes, then `WriteHashedUUIDs`
   (`ExportCIMXML.pas:1286`) — the exporter auto-creates the hashed keys
   `Station=Station=1`, `GeoRgn=GeoRgn=1`, `SubGeoRgn=SubGeoRgn=1`
   (probe-proven; the fixture preloads all three). **Mechanism + storage
   (port these exactly — GAPS_PLAN WPG.18 builds on them):** the three keys
   come from `DefaultCircuitUUIDs` (`ExportCIMXML.pas:1276`), which
   `DoExportCmd` calls on **every** export keyword (`ExportOptions.pas:188`),
   via `GetDevUuid` (`:1002`, the exact `'Station=' + name + '=' + seq` key
   strings) → `GetHashedUuid` (`:952`, find-or-CreateUUID4). The hashed list
   (`UuidHash`/`UuidList`/`UuidKeyList`) **persists across commands** on the
   DSS context (the `FreeUuidList` call after CIM export is commented out —
   "deferred for UUID export", `:4697`); `DoUuidsCmd` resets it first via
   `StartUuidList` (`:822`, called at `ExecHelper.pas:4472`). Rust home:
   seed `crates/dss-core/src/cim/` (the PORTING_PLAN module) with this state
   + the four helpers, keeping the Pascal surface — WPG.18 fills the rest of
   the module later; don't invent a different storage shape here. Probe-proven quirk to reproduce: `Text.Result` stays
   EMPTY after `export uuids` (unlike every other export). Gate: wire
   `phase8_decks/uuids.dss` + `uuids_pre.csv` (the `@FIXTURES@` token →
   absolute fixtures dir on both the Python and Rust sides); byte-exact —
   every object is preloaded. Migrate the 1 `Uuids`-tagged corpus deck (its
   paired `Export` tag permitting).
7. `COVERAGE.md` refresh + the §0 ritual (audits, STATUS).

---

### WP8.7 — ReduceAlgs (full) + `TLineObj.MergeWith` + `Remove` [8%]

**Pascal:** `PDElements/Line.pas` `MergeWith:1631-1840`;
`Meters/ReduceAlgs.pas` (whole file, procedures below);
`Meters/EnergyMeter.pas` `ReduceZone:2257-2284`;
`Executive/ExecHelper.pas` `DoReduceCmd:1614-1663`, `DoKeeperBusList:2035-2095`,
`DoSetReduceStrategy:3049-3104`, the `Remove` command handler (find
`DoRemoveCmd` at WP open — it drives `DoRemoveBranches`).

**Already in place (Phase 6 framing):** `do_reduce_cmd` (`exec/solve.rs:252`)
reproduces `MarkCapandReactorBuses` / error 1890 / the `'A'`-vs-named-meter
dispatch / error 262 — only the `ReduceZone` call is the
`reduce_deferred_msg()` stub; `set_reduce_strategy` (`exec/helpers.rs:210`)
fully parses `Set ReduceOption=` (first-letter B/D/E/L/M/S with the
SWITCH-vs-Shortlines `CompareTextShortest` split) into `ReductionStrategy`;
`Zmag`/`KeepLoad` are plain `Set` options (`ExecOptions` cases 113/112 →
`ReductionZmag`/`ReduceLateralsKeepLoad`). `Set KeepList=` has the option
name only — the handler is unported.

Steps:

1. **`TLineObj.MergeWith(other, series)`** (`Line.pas:1631-1840`) — port
   loop-for-loop:
   - guards: nil → error 184; `Fnphases` mismatch → return false;
     `YPrimInvalid` set up front; `TotalLen` = series: `Len +
     Other.Len·ConvertLineUnits(...)`, parallel: 1.0;
   - series bus rewiring (`:1663-1706`): find the common bus by NodeRef→
     BusRef scan, re-point this line's bus1/bus2 at the two OUTER buses via
     the bus property setters;
   - naming (`:1708-1719`): series → `Other.Name + '~' + Name` (the CHILD
     survives, renamed), parallel → `<bus1-stripped>||<bus2-stripped>`;
     `UpdateControlElements` (`:1842-1851`) re-points controls monitoring
     `Other` onto `self`; series clears `IsSwitch`;
   - impedance: (a) both sym-components & 3φ → length-weighted per-unit
     R1/X1/R0/X0 averages (series) or `ParallelZ` on total-ohm Z1/Z0 +
     summed C (parallel), applied through the property-edit path
     (`BeginEdit`/`SetDouble`/`EndEdit` — so PrpSequence updates like a user
     edit); (b) matrix parallel → `TotalLen = Len/2` (upstream "assume equal"
     TODO — reproduce); (c) matrix series → element-wise
     `(Z1·len1 + Z2·len2)/TotalLen` for Z and Yc, with `len=1.0` when
     geometry/spacing-specified (length already baked into the matrices);
   - finalize: `Other.Enabled := FALSE`, return true. Ratings are NOT
     recombined — don't invent it.
   Oracle-verified expectations to pin (from the pre-validated decks):
   `l1~l2` / `s1~s2` / `b1||b2` names, the partner disabled, node counts in
   the `tests/corpus/modes` manifest notes.
2. **The strategy procedures** (`ReduceAlgs.pas`; all early-exit when
   `BranchList = NIL`; `First()` then `GoForward()` so the head branch is
   always kept; `SERIESMERGE=TRUE`/`PARALLELMERGE=FALSE`):
   - `DoMergeParallelLines:47` — `IsParallel` branch → merge with
     `LoopLineObj`, parallel;
   - `DoBreakLoops:69` — `IsLoopedHere` → disable the `LoopLineObj` partner;
   - `DoReduceDangling:91` — `IsDangling` line whose to-bus ref > 0 and not
     `Keep` → disable;
   - `IsShortLine:119` — `|Z1|·len ≤ ReductionZmag` (sym-comp) /
     `|Z[1,1]-Z[1,2]|·len` (multi-φ matrix) / `|Z[1,1]|·len` (1φ);
   - `DoReduceShortLines:142` — pass 1 flags short lines; pass 2 (skip
     `HasControl`/`IsMonitored`): 0 children + 0 shunts + not Keep →
     disable; 0 children via in-line parent (parent must have exactly 1
     child, no cap/reactor shunts) → parent-merge + move the shunts' `bus1=`
     to the to-bus (`:226-227` re-edit through the parser); 1 child →
     child-merge + move shunts to the FROM bus (`:277-278`), then an extra
     `GoForward` (`:282`); tail: `ReprocessBusDefs` + `SystemYChanged`;
   - `DoReduceSwitches:297` — `IsSwitch` lines: dangling+no-shunts →
     disable; 1 child + no shunts + not Keep + child is a non-switch line →
     child-merge;
   - `DoReduceDefault:335` — non-switch, no control/monitor, 1 child +
     0 shunts + not Keep → child-merge;
   - `DoRemoveAll_1ph_Laterals:449` — a 1φ branch whose to-bus has EXACTLY
     one node (a shared-bus pair is SKIPPED — `:474`); with
     `ReduceLateralsKeepLoad` re-parent each shunt onto the head bus via
     `Bus1=<head> kV=<head kVLN %.6g>` (`:509-510`), disable the lateral's
     PD elements down to the start level; tail: `ReprocessBusDefs` +
     `SystemYChanged`;
   - `DoRemoveBranches:374` — position at the named PD element (miss →
     error 5432100 `%s not found (Remove Command.)`); if KeepLoad, create
     `Load.Eq_<elem>_<frombus-stripped>` at the FROM bus with
     ` phases=%d Bus1=%s kW=%g kvar=%g kV=%g %s` from the branch's metered
     power (kV from the bus kVBase, ×√3 when NPhases>1, else the
     `|VBus[1]|·0.001` fallback); disable every branch + shunt below the
     start level; tail: `ReprocessBusDefs` + `SystemYChanged`.
   Only shortlines/laterals/removebranches reprocess bus defs themselves;
   the other strategies rely on the bus1/bus2 edits raising
   `BusNameRedefined` → the next solve reprocesses. Don't add extra
   invalidation.
3. **`ReduceZone` + wiring** (`EnergyMeter.pas:2257`): build zone lists if
   `BranchList` unassigned (`MakeMeterZoneLists`), then dispatch the
   `ReductionStrategy` enum → the procedures above (default incl. TapEnds →
   `DoReduceDefault`; `rsTapEnds` is commented out upstream). Replace both
   `reduce_deferred_msg()` sites in `do_reduce_cmd`.
4. **`Set KeepList=`** (`DoKeeperBusList`, `ExecHelper.pas:2035-2095`):
   cumulative; `File=<path>` (first token per line) or an inline bus-name
   array; each hit sets `Bus.Keep` (the flag `mark_cap_and_reactor_buses`
   already writes). **`Remove`** command: add the `cmd::` ordinal + handler
   → `DoRemoveBranches`.
5. Gate: flip the ten `tests/corpus/modes` Reduce cases (the nine
   `reduce_*.dss` + `midi_reduce.dss`) to `pending:false` (each manifest note records the oracle-verified
   outcome: merged names, disabled partners, node counts, the KeepList
   block, the skipped x2 pair, the `Load.eq_l2_b2` equivalent), prove the
   live compare green at micro tolerance;
   extend `golden_autoadd_reduce.rs` with a post-`Reduce` re-solve on a
   metered feeder (branch/node counts + voltages vs the oracle); no corpus
   decks are Reduce-tagged (verified) — `COVERAGE.md` refresh only.

---

### WP8.8 — Phase exit [6%]

1. `rg "TODO\(compat\)"` / `rg "NOT_PORTED"` / `rg "TODO\(WP8\)"` sweep — every
   remaining site points at its owner (GAPS_PLAN WPG.* — incl. WPG.18 for CIM —
   Phase 9 GIC/A-Diakoptics/exotics, or "never" for DLLs/GUI). The WP8.4-era byte-pass notes (FPC `Format('%g')`
   stand-ins flagged in STATUS) are settled here.
2. Run `tools/cmd_coverage.py` to **prove tail coverage** of the corpus command/option
   set (PORTING_PLAN §Phase 8 deliverable); document the residual.
3. Verify every family-manifest case with `wp: "WP8.*"` is `pending:false`
   (the WPG.* cases stay pending until GAPS_PLAN executes; WPG.17 owns the
   final no-pending sweep). Verify `tools/golden/phase8_decks/` decks are all
   wired into `gen_phase8.py` (the dir then holds only the README + decks the
   metas reference).
4. Re-run the full suite + the **always-on live corpus compare**; run a final
   `DSS_LIVE_CLASSIFY=1` pass and migrate every newly-unblocked deck. Refresh
   `tests/corpus/COVERAGE.md`.
5. Update `PORTING_PLAN.md` §Phase 8 cross-links; rewrite `STATUS.md` (Phase 8 record;
   "next = Phase 9 exotics — optional; stopping here is a complete usable simulator",
   per PORTING_PLAN §Phase 9 / cumulative note).
6. Merge to `main` (`--no-ff`, the per-phase convention) — **only on explicit user
   request**.

## 4. Deferred in this phase (pointing forward)

- **Out-of-band infrastructure landed mid-phase (2026-07-07, no effect on this
  phase's gate):** the **official-EPRI-binary oracle** `tools/opendss/` — Oddie
  bridge over vendored `OpenDSSDirect.dll` r3723/r4088/r4133, opt-in only
  (`DSS_LIVE_OPENDSS=<rev>` report-mode test + `ab_compare.py` A/B inventory).
  Phase 8 WPs keep gating on the pinned dss-python oracle exactly as §1 states;
  the EPRI channel exists to scope the future post-acceptance upgrade work
  (see `PLAN_SEQUENCE.md`, `tools/opendss/README.md`).
- **GAPS_PLAN WPG.18** — the CIM XML exports `CIM100`/`CIM100Fragments`
  (`ExportCIMXML.pas`; pulled out of Phase 9 on 2026-07-06 — byte-exact golden
  gate on the WP8.6 `uuids file=` determinism recipe). Until it runs, the two
  keywords stay scoped `NOT_PORTED`.
- **Phase 9 (exotics)** — the
  A-Diakoptics exports `IncMatrix`/`IncMatrixRows`/`IncMatrixCols`/`BusLevels`/
  `Laplacian` and the `DSS_CAPI_ADIAKOPTICS`-only `ZLL`/`ZCC`/`Contours`/`Y4`; the GIC
  export `GICMvars` (GIC elements are GAPS_PLAN WPG.16, which decides whether to
  pull this export in); `Pstcalc` flicker outputs (Monitor
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
