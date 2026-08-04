# DE_PASCALIZE wave 3 (W3.x) + wave-2a settler + R3iv + P3

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### DE_PASCALIZE wave 3 — settler pass: both audits settled (2 fixes, 4 pins strengthened, 3 proven non-fixes) (branch `depas-final`, 2026-07-26)

Two independent audits of the wave-3 range (`7c4a89b6..d63c163f`, 7 commits, 91
files) both returned **PASS** — AUDIT-CODE found no correctness/bit-neutrality
defect, AUDIT-TESTS found no weakened verification (test names 1806 → 1821, 0
removed; `#[ignore]`/`#[should_panic]` counts identical; zero diff under
`tests/`). Their nine findings (2 low + 7 info) are settled here, each
empirically — two fixes, four pins strengthened, three proven non-fixes, one
more test (1822) and no `#[ignore]`.

**(1) `Estimate`'s nested commands cleared `SolutionAbort` — REAL, FIXED.**
Upstream `SolutionAbort := FALSE  // Reset for commands entered from outside`
occurs 32× and **only** under `src/CAPI/*` (incl. `CAPI_Text.pas:35`); the
executive's own `TExecutive.ParseCommand` (`Executive.pas:225-266`) →
`ProcessCommand` (`ExecCommands.pas:214-236`) resets only `CmdResult` /
`ErrorNumber` / `GlobalResult`, and the only four engine-side resets
(`Circuit.pas:1607`, `Diakoptics.pas:546/703`, `DSSCallBackRoutines.pas:152`)
are off the command path. This port folded both roles into `Dss::command`,
so W3.5's two nested legs (`Set showexport=yes`, `Export Estimation`) cleared an
abort raised *inside* the command — reachable: `do_allocate_loads_cmd` runs
three solves and a failed Y build sets the flag (`solution/ymatrix.rs:191`, the
queued-`CalcYPrim`-error arm). Split into `Dss::command` (the CAPI wrapper: the
abort clear, then delegate) + `Dss::process_command` (Pascal `ProcessCommand`),
and `do_estimate_cmd` now uses the latter — the port's statement-for-statement
claim is now actually true. Pinned by
`exec/tests/allocation.rs::estimate_tail_legs_use_the_nested_seam_and_keep_a_solution_abort`:
the two seams side by side (`process_command` keeps a set flag, `command` clears
it — the CAPI behavior stays pinned by the older
`max_control_iterations_exceeded_warns_and_aborts`), then `estimate` under a
pre-raised abort (its three solves report "Solution aborted.", the flag stands,
the export leg still writes its CSV into a scratch datapath). **Verified
feature-sensitive by doing it**: routing the two legs back through `command`
fails the last assertion.

**(2) Broken intra-doc link `CTRL_STATE_KEEP` (W3.1 leftover) — FIXED.**
`relay/accessors.rs:419` still linked the constant W3.1 deleted. Measured before
(`cargo doc --no-deps -p dss-core`: *"unresolved link to `CTRL_STATE_KEEP`
--> …accessors.rs:419:57"*) and after (gone); now `[`ControlAction::Keep`]`,
which resolves through the file's existing `use`.

**(3) `tests/corpus` run-artifacts — not a wave defect.** The 21 untracked files
AUDIT-CODE saw were its own gate run's exports; `git status --short` in the
worktree is empty at `d63c163f` and after every run of this pass (cleaned by
exact name). The leak itself is the known scheduler-level open item, restated
below.

**(4) Public-surface narrowing — RECORDED (documentation-only).**
`DynamicExpObj::get_var_idx` `pub` → `pub(crate)` (forced: it now returns the
`pub(crate)` `VarRef`), `pub const ckt_tree::NO_BUS` deleted, and this pass adds
`pub(crate) Dss::process_command`. Verified zero consumers outside
`crates/dss-core/src` (grep over the other five crates and `dss-core/tests`).

**(5) Pascal's `SetLength`-before-validation stray `Cmds` cells — PROVEN, not
reproduced, now pinned.** Both `InterpretDiffEq` error branches grow `Cmds`
*before* they validate the token (`DynamicExp.pas:502` +2 in the `dt` arm,
`:526` +1 in the general arm) and FPC zero-fills, so upstream keeps `[0, 0]` on
the 50006 path (which `Exit`s) and a trailing `0` on the 50005 path. Traced both
consumers: the 50006 stream feeds nothing but `SolveEq`'s final *unguarded*
`MemSpace[OutIdx][1]` with `OutIdx = -1` — the wild write this port already
guards (UB → not reproduced, CLAUDE.md rule); the 50005 stream `[0, -50, 0]`
makes upstream's final upload write `MemSpace[0][0]` where the port writes the
RPN seed `0` — deterministic, but reachable only by driving a `DynamicExp` whose
compilation already failed *and* whose `Expression` was cleared. Left as is
(reproducing it would need a new `TODO(compat)` site, which this wave's
invariant fixes at 117) and listed as an open item. The two error tests now
assert the compiled stream itself (`[]` and `[0, -50]`), so any drift is visible.

**(6) `ControlAction::Other(n)` aliasing — PROVEN unreachable, hardened.**
Workspace-wide grep: `ControlAction::Other(` occurs only inside `from_ordinal`
and its two unit tests, so no hand-built value exists. The property every
`== ControlAction::X` guard actually rests on is *injectivity of
`from_ordinal`*, which was unpinned — now asserted over a 17-code spread
(`i32::MIN`, `i32::MIN + 1`, negatives, 0..=8, 100/101, `i32::MAX`), plus a
variant doc that forbids direct construction. The `StorageState::Other`
precedent is left untouched (same shape, same invariant).

**(7) `solve_eq`'s `EqMark` boundary on a malformed stream — invariant made
runtime-checked.** The typed guard keeps the *previous* `out_idx` where Pascal
would latch a negative one; the stream shape that would show it is impossible
(`InterpretDiffEq`'s `dt` arm is the only `EqMark` writer and emits
`[Var(out), EqMark]` in one step). That claim is now a `debug_assert_eq!` in the
else arm of the boundary branch — debug-only, so the release path is byte-identical,
while every dynamics unit test and all 514 corpus cases execute it in the test build.
The `eq_mark_is_always_preceded_by_its_output_var` pin stays.

**(8) VSConverter terminal-2 mirror was real-part-only — STRENGTHENED with a
measurement.** The pre-existing assertion checked only `re` (a mirror broken
purely in reactive current would have passed); the retype carried that shape
through unchanged. Added the imaginary arm at the *same* `1e-6` bound and the
same per-component denominator (the real arm is byte-unchanged — nothing was
merged into a magnitude, nothing loosened). Measured 2026-07-26: both residuals
are **exactly 0.0** (terminal 2 is the negation of terminal 1), so the new arm
has full headroom.

**(9) Corpus-gate export-artifact leak — open item, unchanged.** Each corpus-gate
run leaves one or more untracked exports (`tests/corpus/electricdss-tst/Test/
AutoTrans/*`, and per-deck `*_EXP_*.CSV` under the family dirs). Nothing under
`tests/` is tracked-modified by the wave; the merge protocol stays "delete by
exact name, never a wide `git clean`".

**Open items handed forward.** (a) The other internal nested-command sites
(`auto_add.rs:393`, `tearing.rs:404/657`, `diakoptics/engine.rs:36/203-214`,
`json_import.rs:56/67/141`, `MakeNewCircuit`'s vsource, `construct.rs`'s default
items) still go through the CAPI-shaped `Dss::command`; upstream reaches them via
`ParseCommand` too, so they carry the same abort-clear inexactness. Converting
them is *not* bit-neutral (AutoAdd raises `solution_abort` itself inside the very
loop that then issues a nested command), so it needs its own gated pass — not a
settler drive-by. (b) The stray-`Cmds`-cell divergence of (5). (c) The
corpus-gate artifact leak of (9).

**Proof.** Full gate green: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -D warnings`, `cargo test --workspace` including the unconditional
514-case corpus gate (`corpus_gate_all_cases_match_engines ... ok`, both
channels). `git diff --stat 7c4a89b6..HEAD -- tests/` **empty** — no golden
regenerated or added, no tolerance, no `ledger.json`, no corpus deck touched.
`TODO(compat)` inventory unchanged (**117** in `crates/dss-core/src` across 68
files, **123** across `crates/**`); no new `downcast`/`as_any`/`Rc`/`RefCell`/
`Mutex`/statics; registration order untouched. `tests/corpus` pristine.

### DE_PASCALIZE W3.5 — the `Estimate` command is routed; ORPHANED_GAPS §1.10 is closed (branch `depas-final`, 2026-07-26)

Not a de-Pascalization — the wave's closing **rider**: the last unrouted piece of
§1.10. `Estimate` (`TExecCommand` ordinal **76** — `ExecCommands.pas:97`,
dispatched at `:572`; the "ordinal 90" in the §1.10 note was a mis-transcription,
corrected there) fell to `not_ported_command` even though **both** its
constituents have been ported for a while.

**What it is.** `TExecHelper.DoEstimateCmd` (`ExecHelper.pas:4213-4226`) is four
statements: "Load current Estimation is driven by Energy Meters at head of
feeders" → `DoAllocateLoadsCmd`; then "let's look to see how well we did" → `if
not AutoShowExport then ParseCommand('Set showexport=yes')` and
`ParseCommand('Export Estimation')`. EPRI r4133 is the same ordinal and the same
body (`Version8/Source/Executive/ExecCommands.pas:838` →
`ExecHelper.pas:3768-3782`), so both gating channels agree.

**The port** (`exec/solve.rs::do_estimate_cmd`, dispatched from `exec/command.rs`
next to `ALLOCATE_LOADS`, ordinal in `exec/tables.rs::cmd::ESTIMATE`): the same
three statements, with the two nested `ParseCommand`s issued through
[`Dss::command`] — this port's `ProcessCommand` and the established
nested-command seam (`auto_add.rs::do_auto_add_cmd`, `tearing.rs`). Going through
the real command path is what makes the export leg pick up `DoExportCmd`'s
filename resolution + last-file tail (`ExportOptions.pas:632-637`:
`SetLastResultFile` + `@lastexportfile`) for free, rather than re-implementing it.
The `showexport` leg is **engine-observable state**, not decoration: it latches
`AutoShowExport` permanently. Its only other consumer — `FireOffEditor`, the GUI
auto-open of the written file — is a headless no-op here (the established
`AllowEditor` convention) and in the oracle runs (`DSS_CAPI_ALLOW_EDITOR=0`); the
`auto_show_export` field doc ("nothing reads it") is updated accordingly, since
`do_estimate_cmd` is now its one engine-side reader.

**Live-probed on the pinned oracle** (dss-python 0.15.7 / backend 0.14.5, the
`est8` deck with its trailing `allocateloads` removed), four facts, all four now
asserted in the test: `estimate` raises no error and writes
`est8_EXP_ESTIMATION.csv`; the file is **byte-identical** to the one
`allocateloads` + `export estimation` writes; `Get showexport` flips `No` → `Yes`;
`@lastfile` == `@lastexportfile` == the written path.

**The pin is oracle-backed with no new capture** (`golden_reports.rs::
estimate_command_runs_allocation_then_exports_estimation`): replay the
`export_estimation.meta.json` deck **minus** its `allocateloads` line, let the
single word `estimate` supply both halves, and diff the produced file against the
golden the oracle wrote for `allocateloads` + `export estimation`
(`export_estimation.txt`, `estimation_policy` = `rel/abs = 0`). No golden was
regenerated or added. Feature-sensitive both ways and *verified* so: commenting
out the allocation leg fails on the first data row (`0` vs `82.184` — it collapses
to the `export_estimation_noalloc` all-zero-`Calc` shape), and dropping the export
leg leaves no file to read. The `showexport`/`@lastfile` asserts catch a routing
that called `do_allocate_loads_cmd` + the export formatter directly instead of the
two nested commands.

**Proof.** Full gate green: `cargo fmt --all --check`, `cargo clippy --workspace
--all-targets -D warnings`, `cargo test --workspace` including the unconditional
514-case corpus gate (`corpus_gate_all_cases_match_engines ... ok`, both
channels). Goldens, tolerances and the ledger untouched; `TODO(compat)` inventory
unchanged (117 in `crates/dss-core/src` across 68 files, 123 across `crates/**`);
no new `downcast`/`as_any`/`Rc`/`RefCell`/`Mutex`/statics; `tests/corpus` pristine
(the known intermittent `Test/AutoTrans` export leak removed by exact name).

### DE_PASCALIZE W3.4 (b) — the `ckt_tree::NO_BUS` sentinel web: the constant is DELETED; Part III has no open escapes (branch `depas-final`, 2026-07-26)

Stratum **[A]** bit-neutral. Closes the **P14 escape** recorded at `depas-p8p14`
(the 5th and last of that WP's sentinels; deferred there as "a zone-walk-wide
sentinel web… converting only `TreeNode.from_bus` forces `Option↔NO_BUS` bridging
at every consumer = net sentinel *increase*"). The escape's premise was true when
it was written and is now **inverted**: P14 itself converted `Terminal::bus_ref`
to `Option<usize>`, so the surviving code was `unwrap_or(usize::MAX)`-ing an
`Option` *back into* the sentinel at every producer. Converting the whole web at
once — as the escape asked — deletes both directions.

**`pub const NO_BUS: usize = usize::MAX` is gone** (`circuit/ckt_tree/mod.rs`).
The web, converted in one commit:

| site | was | is |
|---|---|---|
| `TreeNode.from_bus` | `usize` (`NO_BUS` = unset) | `Option<usize>` |
| `CktTree::add_new_child(elem, bus_ref, term)` | `bus_ref: usize` | `bus_ref: Option<usize>` |
| `TreeNode.to_bus_list` / `add_to_bus_reference` | `Vec<usize>` | `Vec<Option<usize>>` |
| `next_to_bus_reference` | `Option<usize>` (None = cursor past end, `Some(NO_BUS)` = no bus) | `Option<Option<usize>>` — the two "nothing here" cases stay **distinguishable**; the callers that treat them alike say so with `.flatten()` |
| `zones/build.rs` | `t.bus_ref.unwrap_or(usize::MAX)` ×3 + `!= usize::MAX &&` guards ×2 | the `Option` flows through; guards are `let Some(b) = … .filter(\|&b\| b < ckt.buses.len())` |
| `topology.rs` | `unwrap_or(NO_BUS)` + `if bus == NO_BUS \|\| bus >= len` | `bus_ref` passed as-is + one `let … else` |
| `take_sample.rs` | `from_bus != NO_BUS && ckt.buses[from_bus]…` | `let Some(fb) = from_bus && ckt.buses[fb]…` (let-chain) |
| `interpolate.rs` | `coord_defined(ckt, usize)`, `first/second_coord_ref: usize`, `calc_bus_coordinates(usize, usize)` | all `Option<usize>`; the **UB guard stays a guard** — `bus.and_then(\|b\| ckt.buses.get(b))`, i.e. an unset from-bus is still "no coordinate", never the Pascal `buses[0]` OOB read |
| `reduce.rs` | `red_bus_keep/red_bus_name/red_head_base_kv/red_load_base_kv(bus: usize)` fed `unwrap_or(usize::MAX)` | all take `Option<usize>`; `None` is "no such bus", exactly what an out-of-range index already meant |

**`ZoneEndsList.ends` deliberately stays `(usize, usize)`** — and that is a
*result*, not an omission: its single producer (`zones/build.rs`) now hoists the
wired-and-in-range test into a `let Some(test_bus) = term_bus[iterm-1].filter(…)
else { continue }` **before** the bus is used, so `zone_ends`, `add_to_bus_reference`
and `add_new_child` on that path can only ever record a REAL bus. The invariant is
documented on the type. The same holds for the meter-level `zone_ends:
Vec<(ElemId, usize)>`.

**Behavioral equivalence, case by case (this is the whole proof — no arithmetic is
involved anywhere in the diff).** Every old sentinel path ended in a
`buses.get(usize::MAX)`/`>= buses.len()` miss, and every new one ends in a `None`
miss on the same branch: `test_bus >= len` ≡ `filter(|b| b < len)` returning `None`
(an unset `bus_ref` was `MAX`, always `>= len`); `test_bus != test_bus_refs[j-1]` ≡
`test_bus_refs[j-1] != Some(test_bus)` (a `None` ref could never equal a valid
index); `red_bus_keep(MAX)` ≡ `red_bus_keep(None)` = false; the dangling-line
`Some(NO_BUS)` rejection ≡ `.flatten()` yielding `None`; `coord_defined(MAX)` ≡
`coord_defined(None)` = false. The `is_dangling`/`is_looped`/`DistFromMeter`/
`volt_base_index` writes and the `SequenceList` visit order are untouched, so the
EnergyMeter zone topology — which is what the reliability sweeps, zone dumps and
reductions observe — is identical.

**Proof.** Full gate green, goldens and tolerances untouched: `cargo fmt --all
--check`, `cargo clippy --workspace --all-targets -D warnings`, `cargo test
--workspace` including the unconditional 514-case corpus gate (both channels).
The channels that would catch a walk change are all in it: the `ckt_tree` unit
tests (extended — the cursor test now also pins `Some(None)` vs `None`, the
distinction the old `usize` list could not express), the reliability/branch-
reliability exports, `show isolated`/`show topology` byte-goldens (incl. the
compiled-but-unsolved deck, where every `bus_ref` is unset), the meter zone/
sequence dumps, the `interpolate`-driven bus-coordinate reports and the reduction
decks. `TODO(compat)` inventory unchanged (117 in `crates/dss-core/src` across 68
files; 123 across `crates/**`). `tests/corpus` pristine.

**Part III (P8–P15) now has zero open escapes** — plan header updated.

### DE_PASCALIZE W3.4 (a) — the `exec/view.rs` interleaved re/im snapshot: `ElementSnapshot` goes `Vec<Complex64>` (branch `depas-final`, 2026-07-26)

Stratum **[A]** bit-neutral. Closes the **P8 escape** recorded at `depas-p8p14`
("`exec/view.rs` COM-style interleaved re/im `Vec<f64>` → `Vec<Complex64>`;
deferred — blast radius is the gate-critical comparator plus ~40 in-crate test
sites"). Exactly what `DE_PASCALIZE_PLAN.md` §P8 prescribes: *"`exec/view.rs`
switches to typed `Vec<Complex64>`/slices — the product is a Rust-native library
(`PORTING_PLAN` binding decision 1), so the COM-style interleaved re/im `Vec<f64>`
(`[2k]`/`[2k+1]`) is a Pascal-ism, not a contract; one boundary adapter interleaves
where a text/CSV writer still needs the flat form."*

**The engine type.** `ElementSnapshot.powers`/`.currents` are now
`Vec<num_complex::Complex64>` of length `yorder` (was `Vec<f64>` of `2*yorder`):
`powers[k] = kW + j·kvar`, `currents[k] = A`. `snapshot_elements`'s two write loops
collapse — the powers loop is now a `zip` over `node_ref[..yorder]`/
`iterminal[..yorder]` (`*p = s * 0.001`; `Complex64 * f64` is
`Complex::new(re*s, im*s)`, i.e. the *same two multiplications* the interleaved form
did, in the same order), and the currents loop is a single
`copy_from_slice(&cd.iterminal[..yorder])`. The NCIM override block writes
`snap.currents[k] = i` / `snap.powers[k] = s * 0.001` directly. No arithmetic, no
accumulation order, no rounding changed — this is a re-*packing* of the same f64s.
`loss_w` stays the `(f64, f64)` tuple (not part of the recorded escape).

**The boundary adapter (the interleave stops at the comparator).** The oracle
surfaces — dss-python `CktElement.Powers`/`Currents`, the EPRI DLL, and the golden
JSON files — speak interleaved re/im, and none of them were touched. New in
`tests/harness/mod.rs`:
- `deinterleave(&[f64]) -> Vec<Complex64>` — decodes the oracle/golden encoding.
- `assert_complex_close_c(&[Complex64], &[Complex64], …)` — the pairwise comparator,
  now the *core*; the pre-existing `assert_complex_close(&[f64], &[f64], …)` keeps
  its signature (node voltages, YPrim, injections, monitor channels all still arrive
  interleaved) and is a thin length-check + `deinterleave` wrapper over it. Same
  formula (`hypot`-free `((ar-er)² + (ai-ei)²).sqrt()` vs `abs_floor + rel·|e|`),
  same per-entry index in the panic text.
- `assert_power_close` retypes its `actual` to `&[Complex64]` (its length assert
  drops the `2*`); the voltage-scaled floor arithmetic is byte-identical.
`compare_element` now zips the oracle's split `i_re`/`i_im` into `Complex64` instead
of interleaving them; `compare_injection` compares `node_injection_currents()[1..=n]`
directly instead of interleaving both sides first.

**Call-site sweep (27 files).** `snap.powers[2*k]`/`[2*k+1]` → `.powers[k].re`/`.im`
across `corpus_gate/ledger.rs` (envelope + selected-channel rewrite), the golden
drivers (`golden_feeders`, `golden_feeders_controls`, `golden_metering_monitors` —
each `deinterleave`s the golden array at the call), and ~20 in-crate test modules.
Every `.iter().step_by(2)` sum became `.iter().map(|s| s.re)` over the same
conductors in the same order; interleaved `[f64; 4]`/`[f64; 6]` oracle-pin literals
became `[Complex64; 2]`/`[Complex64; 3]` with the identical per-component
tolerance test (`re` and `im` each still checked at the old bound, never a merged
magnitude). `harness_power_floor.rs`'s three synthetic caps became one-conductor
`Complex64` vectors.

**Proof.** Full gate green with **zero golden regeneration and zero tolerance
change**: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -D
warnings`, `cargo test --workspace` (incl. the unconditional 514-case corpus gate,
both channels, `corpus_gate_all_cases_match_engines ... ok`). The byte/ULP-level
goldens (checkpoint Y/YPrim, feeder element powers+currents, protection, metering)
are exactly the channels this type feeds, so their unchanged pass *is* the
bit-neutrality proof. `TODO(compat)` inventory unchanged (117 in `crates/dss-core/src`
across 68 files; 123 across `crates/**`). `tests/corpus` pristine (the known
`Test/AutoTrans` export leak removed by exact name).

### DE_PASCALIZE W3.3 — the `DynamicExp` RPN token stream becomes a payload enum: **the P1 tail is CLOSED** (branch `depas-final`, 2026-07-26)

Stratum **[A]** bit-neutral. Closes P1-tail escape item **3** (= P1 §Deferred item
**10**, Tier-2) — the last open item of the whole P1 tail. Not a field retype: the
compiled expression `cmds: Vec<i32>` is Pascal's `Cmds` *automation array*, a tagged
union squeezed into one `Integer`, so this is the evaluator refactor the escape
record asked for.

**The encoding, and what replaces it.** Pascal states it in the `InterpretDiffEq`
header (`DynamicExp.pas:458-467`, identical in EPRI r4133 `DynamicExp.pas:564-573`):
a cell is a variable slot (`>= 0`), a constant index (`50000 + i` into `VarConsts`),
an operator (`-opCodes_index`), or the `-50` new-equation marker. New module
`elements/general/dynamic_exp/tokens.rs`:

| type | what it names | proof |
|---|---|---|
| **`DynToken`** | `EqMark` / `Op(DynOp)` / `Var(usize)` / `Const(usize)` — one variant per legal cell | the notation comment above; `Cmds` writers at `DynamicExp.pas:504-519` (`dt` pair), `:537` (const), `:544` (var), `:550` (operator) |
| **`DynOp`** (22) | `Add`…`Pow`, discriminant = the `opCodes` index stored negated | `opCodes` table `DynamicExp.pas:101-106` **==** r4133 `myOps` `:60-64`; `SolveEq`'s `case` `:397-449` == r4133 `:520-548` |
| **`Lexeme`** + `OP_LEXEMES[29]` | what `InterpretDiffEq`'s `case OpCode of` does with each `opCodes` entry: `Dt` (0) / `Notation` (1,6,7,8,9) / `End` (`]`=10) / `Op` | `DynamicExp.pas:501-554` |
| **`VarRef`** | `Get_Var_Idx`'s three-way `Integer`: `State(i)` / `Const` (the `50001` `CONST_CODE`) / `NotFound` (-1) | `DynamicExp.pas:283-312`; consumer test `(varIdx < 0) or (varIdx >= 50000)` at `DynEqPCE.pas:152-154` |

**Both named sentinels are gone from the engine.** `CONST_CODE = 50001` died with
`get_var_idx` → `VarRef` (its single cross-module consumer, `dyneq_pce.rs::
parse_dyn_var`, is now `let VarRef::State(var_idx) = … else { return false }` —
the same test, spelled). `CONST_BASE = 50000` and `EQ_MARK = -50` survive **only**
inside a `#[cfg(test)]` `encoding` module implementing `DynToken::ordinal`/
`from_ordinal`, which exists so the unit tests still pin the compiled stream
cell-for-cell against the Pascal notation — that is the bit-neutrality proof, not
a runtime path. `Get_Closer_Op`'s two sentinels went the same way: it now returns
`Option<(pos, op, Lexeme)>` (`None` == Pascal's `OpIdx = 10000` seed with `OpCode`
left at -1), while `10000` stays the *internal* leftmost-so-far threshold, so an
operator at position ≥ 10000 is still "not found" exactly as upstream.

**Two `_`-style fall-throughs became exhaustive matches.** `SolveEq`'s `else`
branch (`if Cmds[idx] >= 50000 … else …`) is now the `Const`/`Var` arms of the same
`match` that dispatches the 22 operators, and `InterpretDiffEq`'s `case … else`
is now `Lexeme::Op(_) | Lexeme::End` with the `if OpCode <> 10` guard spelled as
`if let Lexeme::Op(op) = lexeme`. The operator set is provably closed — the `else`
branch emits an operator only for `OpCode <> 10`, so the reachable codes are exactly
`{2,3,4,5} ∪ {11..28}`, asserted directly by the new
`op_codes_pin_the_pascal_opcodes_table`.

**One structural invariant had to be stated to type `OutIdx`.** Pascal latches the
output with `if Cmds[idx] <> -50 then OutIdx := Cmds[idx]`, and `OutIdx: Integer`
seeded -1 (its final unguarded `MemSpace[OutIdx][1]` write is a wild write on an
empty `Cmds` — the port already guarded that). Typed, `out_idx: Option<usize>` and
the latch is `if let DynToken::Var(slot)`: legitimate because the `dt` arm is the
only writer of an `EqMark` and it pushes `[Var(out), EqMark]` **in one step**
(`DynamicExp.pas:504-519`), so the cell before a marker is always the output
variable. Pinned by the new `eq_mark_is_always_preceded_by_its_output_var` over
four expressions (incl. a 3-equation one).

**Pins (+5 tests, 13 → 18 in the module).** `op_codes_pin_the_pascal_opcodes_table`
(the reachable-code set, `op_code()` round-trip through the negated cell, and
`from_ordinal = None` for `-1,-6..-10,-29,-49,-51` — the *closedness* the exhaustive
match relies on); `payload_cells_round_trip_the_pascal_encoding` (`-50`, var slots,
the `50000 + i` offset); `eq_mark_is_always_preceded_by_its_output_var`;
`get_var_idx_classifies_state_vars_constants_and_misses`;
`constant_before_dt_is_rejected_with_the_pascal_message` (error 50006 —
`DynamicExp.pas:506-511`'s `Exit` leaves the expression *uncleared* and `gotError`
false, and it pins the exact message text, which no golden covers). Every existing
`assert_eq!(o.cmds, vec![0, -50, 50000, …])` vector is **unchanged**, now read
through `cmds_ordinals()`; the Kundur case additionally asserts the same stream as
typed tokens. No registry coupling: like the W3.2 derived codes these are internal
compiler/evaluator types with no `DssEnum` entry (noted at the module doc).

**Bit-neutrality evidence.** `git diff --stat HEAD -- tests/` **empty**;
`TODO(compat)` **117 / 68 files** (`crates/**` = 123) before and after; `git diff
-U0` grep for added `downcast`/`as_any`/`Rc<`/`RefCell`/`Mutex`/statics/
`oracle-parity` = **0** (the two `static` hits are `&'static str` in the retyped
`get_closer_op` signature). String-literal diff of the two product files: the
`opCodes` table moved verbatim into `tokens.rs`, the 50006 message was re-wrapped
across the `\`-continuation to the **same bytes** (now pinned by its own test), and
`format!("\"{}\"", vars[0])` is unchanged — **zero** runtime strings changed.
Scope note: `Get_Out_Idx`'s own `-1` "not found" return is deliberately left as an
`i32` (a generic not-found sentinel, not part of the token channel), and the
`Check_If_CalcValue` operand codes (0..11/-1, consumed by four dynamics hosts) stay
raw — a separate, wider channel, untouched here.

**Gate:** `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -D
warnings` · `cargo test --workspace` — green, exit 0, **66 `test result: ok`
groups, 2041 passed, 0 failed, 5 ignored** (2036 → 2041 = the five new pins),
`corpus_gate_all_cases_match_engines … ok` on both channels — the dynamics decks
(`Dynamic_KundurDynExp` and family) run the new evaluator end to end. `tests/corpus`
left pristine (4 `Test/AutoTrans` run-artifacts of the known intermittent scheduler
leak removed by exact name — no wide `git clean`). Ritual 0 held at start and before
the commit: 186 `.pas` under `.inputs/dss_capi`; `cargo` =
`C:\Users\Admin\.cargo\bin\cargo.exe`.

**P1-tail escape record: fully closed.** With W3.1 (item 1), W3.2 a+b (item 2 /
item 8) and this record (item 3), no item of the P1 tail remains open.

### DE_PASCALIZE W3.2 (b) — item 8 CLOSED: shared `ScanType`/`SequenceType`, `VsourceZSpec`, `VscMode`, `OcpDeviceType` (branch `depas-final`, 2026-07-26)

Stratum **[A]** bit-neutral, **type-channel only**. Second half of P1-tail escape
item **2**; with (a) it is **CLOSED in full** — only item 3 (the `DynamicExp`
RPN token stream) is left of the whole P1 tail.

| enum | ordinals (proven twice) | classes converted |
|---|---|---|
| **`ScanType`** (new `pc/source_seq.rs`) | None=-1, Zero=0, Positive=1 — `DSSClass.pas:1087` `ScanTypeEnum` `['None','Zero','Positive']` / `[-1,0,1]` | VSource, Isource, **GICLine** |
| **`SequenceType`** (same module) | Negative=-1, Zero=0, Positive=1 — `DSSClass.pas:1090` `SequenceEnum` | VSource, Isource, **GICLine** |
| **`VsourceZSpec`** (`pc/vsource/mod.rs`) | MvaSc=1, Isc=2, Ohms=3 — `Vsource.pas:450`/`:620`, `:469`, `:488`/`:502` (derived, no registry) | VSource |
| **`VscMode`** (`pc/vs_converter/mod.rs`) | Fixed=0…VdcQac=4 — `VSC_*` at `VSConverter.pas:136-140` = `'VSConverter: Control Mode'` `[0..4]`, `DefaultValue = VSC_FIXED` | VSConverter |
| **`OcpDeviceType`** (`elements/ckt.rs`) | Unset=0, Fuse=1, Recloser=2, Relay=3 — `GetOCPDeviceType`, `Utilities.pas:1996-2018` (derived, no registry) | CktElement base + FeederSection + the `RefAction` channel |

**Three classes, not one — the P1 no-half-conversion rule.** The escape record
named only `vsource.{scan_type, sequence_type}`, but those two fields are
`ScanTypeEnum`/`SequenceEnum` **shared**: Isource declares the identical pair of
properties against the same two registry entries (`Isource.pas:173-178`), and
GICLine holds the same two fields property-less, pinned to zero sequence by
`Create` ("Always 0 for GIC", `GICLine.pas:390-391`) yet still running the same
`case`. Converting only VSource would leave the family half-typed, so all three
convert here and the enums live in a shared `pc/source_seq.rs` (the `MonPhase`
precedent). Every `_ =>` arm in the six rotation `match`es becomes the *named*
Pascal `else` arm it always was — `ScanType::None` and `SequenceType::Positive`
— so the arm that used to swallow any stray integer now names one value.

**`OcpDeviceType` keeps the `==0` sentinel exactly.** The escape record flagged
the `== 0` "unset" test plus the reliability ripple. `Unset` is a real variant
(Pascal's `Result := 0` pre-scan seed), it is `#[derive(Default)]` so
`FeederSection`'s all-zero allocation (`EnergyMeter.pas:2465`) is unchanged, and
the first-wins guard in `exec/command.rs` reads `== OcpDeviceType::Unset` — the
same test on the same value. The channel is typed end to end: `RefAction::
SetOcpDevice { device_type }` now carries the enum (Fuse/Recloser/Relay push
their own variant instead of a bare `1`/`2`/`3`), `CktElementData` and
`FeederSection` store it, and `Export Sections`'
`getOCPDeviceTypeString` (`ExportResults.pas:3859`) becomes exhaustive with
Pascal's `else` named `Unset => "Unknown"` — the four output strings are an
unchanged multiset.

**`VscMode` is store-only in both engines** (upstream never reads `Fmode`; only
the declaration, the property offset, `MakeLike` and the `Create` seed mention
it), so this is a pure property round-trip retype — recorded so nobody later
"finds" a missing control-mode dispatch here.

**Pins (4 new tests).** `scan_and_sequence_type_pin_pascal_ordinals`,
`vsource_z_spec_pins_pascal_ordinals` (also asserts the three `Create` seeds),
`vsc_mode_pins_pascal_ordinals` (incl. the live registry's `default_value` =
`Fixed`) and `ocp_device_type_pins_pascal_ordinals` (incl. `default()` ==
`Unset` == `CktElementData::new`'s seed). Each asserts `from_ordinal` = `None`
outside its set — the *closedness* the new exhaustive matches rely on.
`registry_enum_coupling` grows by the three registry-backed entries (`Scan
Type`, `Sequence Type`, `VSConverter: Control Mode`): **26 → 29**. Its module
doc now also lists the four W3.2 *derived* codes as deliberately-uncovered (no
registry entry exists to couple them to).

**Bit-neutrality evidence.** `git diff --stat HEAD -- tests/` **empty**;
`TODO(compat)` **117 / 68 files** (broader `crates/**` = 123) before and after;
`git diff -U0` grep for added `downcast`/`as_any`/`Rc<`/`RefCell`/`Mutex`/
`oracle-parity` = **0**. String-literal multiset diff over the 17 changed files:
the only literal lines that move are the four `Export Sections` device strings
(removed and re-added identically as named arms) and reflowed test-assertion
messages; the new `source_seq.rs` adds two test messages. **Zero** runtime
strings changed.

**Gate:** `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -D
warnings` · `cargo test --workspace` — green, exit 0, **66 `test result: ok`
groups, 2036 passed, 0 failed, 5 ignored** (2032 → 2036 = the four new pins;
the registry-coupling extension adds assertions, not entries),
`corpus_gate_all_cases_match_engines … ok` on both channels. `tests/corpus` left
pristine (10 `Test/AutoTrans` run-artifacts of the known intermittent scheduler
leak removed by exact name off the status list — no wide `git clean`). Ritual 0
held at start and before the commit: 186 `.pas` under `.inputs/dss_capi`;
`cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**P1-tail escape record, updated.** Item **2 (item 8)** is **CLOSED** by W3.2 (a)
+ (b). Item 3 (the `DynamicExp` RPN token sentinels, `CONST_CODE = 50001` /
`EQ_MARK = -50`) is untouched and stands exactly as recorded — it is W3.3.

### DE_PASCALIZE W3.2 (a) — the derived `SpecType` codes: `ReactorSpecType` / `CapacitorSpecType`, five `_` fall-throughs eliminated (branch `depas-final`, 2026-07-26)

Stratum **[A]** bit-neutral, **type-channel only**. First half of P1-tail escape
item **2** ("item 8 — the remaining bare-`i32` `DssEnum` fields"). Both fields
are *derived* codes: no property writes them, so — unlike every other family in
this wave — they have **no `DssEnum` registry entry** and the proof is the
Pascal declaration + the assignment sites alone.

| enum | ordinals (proven) | written by |
|---|---|---|
| **`ReactorSpecType`** (`pd/reactor/mod.rs`) | Kvar=1, RplusJx=2, Matrices=3, SymComponents=4 (`Reactor.pas:132` legend) | six property side effects: `kvar`→1 (`:382`), `RMatrix`/`XMatrix`→3 (`:419`), `X`→2 (`:434`), `Z1`→4 (`:451`), `Z`→2 (`:474`), `LmH`→2 (`:478`); `Create`→1 (`:601`) |
| **`CapacitorSpecType`** (`pd/capacitor/mod.rs`) | Kvar=1, Cuf=2, CMatrix=3 (`Capacitor.pas:584` legend) | `kvar`→1 (`:377`), `cmatrix`→3 (`:381`), `cuf`→2 (`:385`); `Create`→1 (`:584`) |

**The value sets are provably closed**, which is the whole point of the step:
the escape record called out `_` fall-throughs in `reactor/solve.rs`, and with
a closed enum every one of them becomes an exhaustive `match`. Five vanished —
three in `reactor/solve.rs` (`recalc`, `calc_yprim`, `make_pos_sequence`) and
two in `capacitor/solve.rs` (`recalc`, `make_pos_sequence`) — plus the two
`_ => {}` arms in `capacitor/steps.rs`'s `NumSteps` re-allocation pair, seven in
total. Each replacement arm is the *named* Pascal arm it always was, with the
Pascal line cited where the arm is empty on purpose (`Reactor.pas:661-665` has
an empty `3:` and **no** `4:` at all; `Capacitor.pas:623-660` has no `3:`, so
`PhasekV` keeps its pre-`case` `1.0` seeding that the Norm/Emerg amps then
divide by; `Capacitor.pas:427-430`'s `3:` is the "nothing to do" arm). A new
variant is now a compile error instead of a silent fall-through.

**Reactor's is invisible, Capacitor's is not.** The reactor code never leaves
the engine. The capacitor's *does*: `DumpProperties` with `Complete` writes the
bare `SpecType=<int>` line (`Capacitor.pas:764`), so `dump.rs` emits
`.ordinal()` and the `dump_capacitor` golden is the end-to-end pin. Kept
verbatim: the capacitor `CMatrix` + `has_zl` branch still carries its
capi015 ×1.000001 perturbation (`DIVERGENCES.md` §B1) — it is now the named
`CapacitorSpecType::CMatrix` arm rather than a `_`, nothing else changed.

**Pins.** `reactor_spec_type_pins_pascal_ordinals` and
`capacitor_spec_type_pins_pascal_ordinals_and_the_dump_line`: the literal
ordinals, the round-trip, `from_ordinal` = `None` for everything outside the
range (the *closedness* claim these matches now rely on), the `Create` seed, and
for the capacitor the rendered `SpecType=1` dump text. Not added to
`registry_enum_coupling`, deliberately and for the first time in this wave:
neither code has a registry entry to couple to (noted at the tests).

**Bit-neutrality evidence.** `git diff --stat HEAD -- tests/` **empty**;
`TODO(compat)` **117 / 68 files** before and after; no new
`downcast`/`as_any`/`Rc`/`RefCell`/`Mutex`/statics/`oracle-parity` (`git diff
-U0` grep = 0 additions). String-literal multiset diff of the 10 changed files:
three added assertion messages plus `"spec_type {st}"` → `"{st:?}"` (a test
message; the field is now `Debug`, not `Display`); **zero** runtime strings —
the `SpecType={}` format text is byte-identical, only its argument is now
`.ordinal()`.

**Gate:** `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -D
warnings` · `cargo test --workspace` — green, exit 0, **66 `test result: ok`
groups, 2032 passed, 0 failed, 5 ignored** (2030 → 2032 = the two new pins),
`corpus_gate_all_cases_match_engines … ok` on both channels. `tests/corpus`
pristine (`git status --short tests/corpus` empty — no run-artifacts this
round). Ritual 0 held at start and before the commit: 186 `.pas` under
`.inputs/dss_capi`; `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

### DE_PASCALIZE W3.1 — the shared `EControlAction` channel: one `ControlAction` enum across all five control classes (branch `depas-final`, 2026-07-26)

Stratum **[A]** bit-neutral, **type-channel only**: no arithmetic, no queue
order, no registration order, no event-log or error text touched. Closes P1-tail
escape item **1** — the last of the three open P1-tail items that is *not*
item 8 / the DynamicExp RPN stream.

**What it replaces.** `elements/control/control_elem.rs`'s seven bare constants
(`CTRL_NONE = 0` … `CTRL_UNLOCK = 5` plus `CTRL_STATE_KEEP = i32::MIN`) become
one **`ControlAction`** enum, and the five classes that spoke them —
SwtControl, Fuse, Recloser, Relay, CapControl — convert in this single commit
(the P1 protocol forbids a half-converted family; the escape record called the
five-class blast radius the exact reason it was deferred).

| Pascal | ordinal | proof |
|---|---|---|
| `CTRL_NONE`…`CTRL_UNLOCK` | 0…5 | `ControlElem.pas:20-29`, `EControlAction` under `{$Z4}` (int32, no explicit values → declaration order) |
| `CTRL_TAPUP` / `CTRL_TAPDOWN` | 6 / 7 | same declaration; referenced **nowhere** in the Pascal tree (RegControl runs its own `ACTION_TAPCHANGE=0`/`ACTION_REVERSE=1`, `RegControl.pas:246-247`) — modeled because they are part of the type |
| `Keep` | `i32::MIN` | not an `EControlAction` value: the `default_value` of the two Relay state `DssEnum`s (`registry/control.rs`), i.e. Pascal `InterpretRelayState`'s first-char-only `case` with no else |
| `Other(n)` | `n` | the open queue-code channel (below) |

**Eight registry entries, one enum.** `SwtControl: Action`/`State`, `Fuse:
Action`/`State`, `Recloser: Action`/`State`, `Relay: Action`/`State` all declare
`[2, 1]` = Close, Open (Recloser's pair carries a third `trip` spelling that also
maps to 1). SwtControl's `Normal` reuses its `State` entry; Relay's two carry the
`Keep` default. `registry_enum_coupling` now walks all eight against
`ControlAction::from_ordinal`, so the previous 18 registry entries become **26**.

**Why `Other(i32)` (the `StorageState` precedent, record 4/n).** The channel is
genuinely open on one path: a CapControl `USERCONTROL` guest schedules its own
queue code through `control_queue_push` (upstream `USER_BASE_ACTION_CODE = 100`,
`ControlElem.pas:67`), and `DoPendingAction` assigns that code straight into
`FPendingChange`, which `Set_PendingChange` also mirrors into
`DblTraceParameter` as a Double. A closed enum would silently drop those, so
`ordinal()`/`from_ordinal` are total and mutually inverse instead.

**The `ControlQueue` seam (the P1b `RegControlAction` precedent).**
`ControlQueue`'s `code: i32` stays `i32` — it is class-polymorphic — and every
class converts at its own push/pop boundary: `push_delay(…, action.ordinal(), …)`
on the way in (SwtControl's lock + action pushes, Relay/Recloser's
open/close/reset pushes, CapControl's armed push) and
`ControlAction::from_ordinal(code)` as the first act of each
`do_pending_action`. Fuse is untouched here on purpose — its queue code is the
**1-based phase number**, not an action. The Relay `DebugTrace` line still
formats the raw popped `code`, so `Debug DoPendingAction Code=…` is
byte-identical.

**The property seam.** `get_i32`/`get_enum_array` emit `.ordinal()`;
`set_i32`/`set_enum_array`/`do_action` take `ControlAction::from_ordinal(v)`.
Because `from_ordinal` is total there is no `unwrap_or(self.x)` fallback to
reason about (unlike the closed families of records 1/n–5/n): every ordinal the
parser can produce maps to exactly the value the bare `i32` stored. The Relay
`Keep` guards move one step earlier (`from_ordinal` first, then `!= Keep`), which
is the same test on the same number. Fuse's `set_enum_array` `copy_from_slice`
becomes a zip over equal-length slices (`write_states`).

**Deliberately NOT changed.** `Fuse::do_fuse_action` keeps its
`if action == Open { Open } else { Close }` shape rather than becoming Pascal's
two-arm `case` (`fuse.pas:145-158`, where an unmatched action leaves
`FPresentState` alone): the two differ only on ordinals the `Fuse: Action` enum
cannot produce, and this step changes types, not control flow. Recorded here so
the divergence is visible rather than silently "cleaned up".

**Pins.** Three new tests in `control_elem.rs`:
`control_action_pins_pascal_ordinals` (all ten named values + round-trip),
`control_action_round_trips_every_out_of_set_ordinal` (totality over
`i32::MIN+1`…`i32::MAX`, incl. `USER_BASE_ACTION_CODE` 100 — and that 0..7 plus
`i32::MIN` are the *only* non-`Other` values), and
`relay_state_enums_default_to_the_keep_sentinel` (the live registry's
`default_value` for `Relay: Action`/`State` **is** `Keep`, and none of the other
six CTRL-backed entries carries it). The end-to-end `Keep` behavior was already
pinned by `relay/tests.rs::state_parse_is_first_char_only_r4133` (`normal=trip`
→ `[closed, closed, closed, ]`) and the `props/relay.json` golden; both still
pass unchanged.

**Bit-neutrality evidence.** `git diff --stat HEAD -- tests/` is **empty** (zero
golden / ledger / tolerance / corpus-deck churn). `TODO(compat)` = **117 / 68
files** at HEAD *and* after (`git grep -c` at both revs). No new
`downcast`/`as_any` (the only two in the tree stay the pre-existing
`catch_unwind` `Box<dyn Any>` payload reads in `tests/corpus_gate/runner.rs`);
no `#[cfg(feature = "oracle-parity")]`; no new `Rc`/`RefCell`/`Mutex`/statics.
Per record 4/n's rule, a mechanical **string-literal multiset diff** of all 19
changed files against the base found exactly three added literals — the three
new assertion messages — and **zero** removed or altered ones: no `push_error`,
event-log, dump or report text moved anywhere in this step.

**Gate:** `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -D
warnings` · `cargo test --workspace` — green, exit 0, **66 `test result: ok` groups, 2030 passed, 0 failed, 5 ignored** (the three new pins are the only added test entries; the registry-coupling extension adds assertions, not entries),
`corpus_gate_all_cases_match_engines … ok` on both channels. `tests/corpus` left
pristine (run-artifacts removed by exact name off the status list — no wide `git
clean`). Ritual 0 held at start and before the commit: 186 `.pas` under
`.inputs/dss_capi`; `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

**P1-tail escape record, updated.** Item 1 (this) is **closed**. Items 2 (the
bare-`i32` `DssEnum` fields: reactor/capacitor `spec_type`, vsource
`z_spec_type`/`scan_type`/`sequence_type`, `vs_converter.f_mode`,
`energymeter.ocp_device_type`) and 3 (the `DynamicExp` RPN token sentinels) are
untouched and stand exactly as recorded.

### DE_PASCALIZE wave 2a settler — every audit finding settled, two more InvControl enums, gate green (branch `depas-p1p3`, 2026-07-26)

Settler pass over the whole wave (P1-tail 1/n–5/n + P3 + R3iv, base `634aac9`).
Both audits returned **PASS**; the nine findings are settled below — three fixed
in code, three answered with new tests, three recorded as proven non-fixes.
Nothing is left for the merge coordinator except the four already-disclosed
fenced lines.

**Metrics re-derived independently (not taken from the port log).** `TODO(compat)`
= **117 / 68 files** at base *and* at HEAD (`git grep` at both revs);
`downcast`/`as_any` **2** at base and at HEAD (the two pre-existing `catch_unwind`
`Box<dyn Any>` payload reads in `tests/corpus_gate/runner.rs`); zero
`#[cfg(feature = "oracle-parity")]`; no new `Rc`/`RefCell`/`Mutex`/statics (the 9
grep hits are doc prose plus the pre-existing `Arc<Mutex>` test sink in
`exec/plot/tests.rs`). `git diff --stat 634aac9 -- tests/` is **empty** — zero
golden / ledger / tolerance / corpus-deck churn across the entire wave.

**Fixed — audit-code 1: the InvControl family was NOT closed.** `FVoltage_CurveX_ref`
and `FVoltwattYAxis` are both `DssEnum`-backed (`InvControl.pas:438-441`) yet
appeared in *no* inventory — not the P1 record's Tier-1 row, not the P1-tail escape
record. Declaring the family closed while two live conversions stayed invisible is
exactly the half-converted state the P1 protocol forbids, so they are converted here
rather than merely listed: **`VoltageCurveXRef`** (Rated=0, Avg=1, RAvg=2 — field at
`InvControl.pas:297`, enum at `:438-439`, `Create` = 0 at `:843`; registry `[0,1,2]`)
and **`VoltWattYAxis`** (PAvailable=0, Pmpp=1, PctPmpp=2, KvaRating=3 — `:299`,
`:440-441`, `Create` = 1 at `:845`; registry `[0,1,2,3]`). Neither is a Pascal enum
*type* (both are plain `Integer` fields), so the closed value set is the `DssEnum`
declaration, cited in each doc comment. `Calc_PBase`'s `match` loses its
`_ => cv.p_base` fall-through (exhaustive over the four bases now,
`InvControl.pas:2850-2890`) and `FPresentVpu`'s two `== 1` / `== 2` tests become
named comparisons (`:1801-1806`). Pins extended in
`invcontrol_enums_pin_pascal_and_registry_ordinals`; the P1 phase record's item-4
row carries a dated correction footnote.

**Fixed — audit-code 5: the structurally-dead combi gate.** Under the closed
3-variant `InvCombiMode` the `combi != None && != VvVw && != VvDrc` test can never
fire, and it guarded a *stale* message ("deferred to WP7.7 (GFM)") — GFM has been
dispatched since WPG.11. It is an exhaustive `match` with no reject arm now, so a
**new** combi variant is a compile error instead of a silent fall-through into the
VV_VW path; the mode `case`'s `_ => {}` is likewise named `InvControlMode::NoneMode`.
The then-unreachable `not_ported_mode()` helper is deleted; `sample()` still returns
`Result` (four other `Err` arms remain, incl. the missing-curve error 382).

**Fixed — audit-tests 4: `RandomType::default()` disagreed with `Create`.** The
derive sat on `None` (0) while `TSolutionObj.Create` seeds `GAUSSIAN`
(`Solution.pas:487`) and `Solution::new` seeds `RandomType::Gaussian`. Nothing
consumes `default()` today, so moving `#[default]` to `Gaussian` is bit-neutral —
but `corpus_gate.rs` already uses `unwrap_or_default()` for `ControlMode`, so the
next `unwrap_or_default()` / `..Default::default()` on this family would have
silently turned `Set random` off with no test firing. The pin test now asserts it,
matching every other family in the wave.

**New coverage — audit-tests 1: the registry↔enum coupling was prose only.** Every
P1 enum claims its discriminants *are* a specific `DssEnum`'s value list, but the
pins encoded only the Pascal half, as literals. New
`obj/dss_enum/tests.rs::registry_enum_coupling` walks the **live** `EnumRegistry`
and asserts every declared ordinal of 18 registry entries resolves through — and
round-trips out of — its Rust enum: ControlMode, RandomType, LoadSolutionModel,
MonPhase (both the `Monitored Phase` and `RegControl: Phase Selection` hybrids), the
seven InvControl families, EspvlControlType, StorageCtrlMode (both the discharge and
the charge list), LoadShapeInterp, GenDispatchMode, StorageState. Proven live, not
assumed: flipping `Volt-Watt Y-Axis` to `[0,1,2,4]` in the registry fails it with
`registry ordinal 4 does not resolve` (reverted). Deliberately out of scope, with the
reason in the module doc: the `ControlQueue` action codes and `VarMode` have no
registry entry at all.

**New coverage — audit-tests 2: the `from_ordinal(v).unwrap_or(self.x)` write path.**
The retyped setters keep the previous value where the pre-enum ones stored the raw
`i32`; the fallback is unreachable (`class_props/parse.rs:307-324` rejects an unknown
`MappedStringEnum` token before any write and early-returns on an out-of-set
`MappedIntEnum` ordinal), which is precisely why nothing pinned that the error is
*raised* rather than swallowed into a silently-changed field. New
`inv_control/tests.rs::bad_enum_values_raise_and_leave_the_field_unchanged` drives
both arms end-to-end (`ControlModel=7` → "not a valid value" + the field keeps 1;
`VoltWattYAxis=nonsense` → error + `?` still reports `PctPMPPPU`).

**New coverage — audit-tests 3: the R3iv pin covered 6 of 34 handles.** The deck now
adds a second LoadShape, three XYcurves and a DynamicExp plus a Load / Generator /
WindGen / Isource and a PVSystem edit, and asserts **22** typed handles dereference
to the named object through their own arena: every `yearly`/`duty` sibling,
`Load.cvr_shape_ref`, `PVSystem.power_temp_curve_ref`, WindGen `VV_Curve`/`PLoss`,
and the shared `DynEqPCE` `dynamic_eq_ref` on two classes. `GicSource::line_ref` is
private with no getter (none added just for a test): its narrowing is pinned by the
`dump_gicsource` golden — a `None` handle raises Pascal error 333 and skips the
`GIC_<name>` splice the golden captures. Noted at the test.

**Non-fix, proven — audit-code 3: the monitor scratch `mem::take` is not
unwind-safe.** True as stated, and harmless: the body's first act on either buffer is
`clear()` + `resize(scratch_len, ZERO)`, the sole early `return` precedes any buffer
use, and a grep over the crate finds no other reader of
`self.voltage_buffer`/`self.current_buffer` (only the `mod.rs` declaration + init).
A panic mid-body therefore costs the reused **allocation** and nothing else — a
re-entered `take_sample` re-derives identical values. A guard type would buy no
observable behavior; the code comment now states the panic behavior explicitly
instead of claiming "handed back on every path".

**Non-fix, recorded — audit-code 2 / audit-tests 5: the four fenced lines stand.**
`exec/report.rs:1367` (1 line) and `report/export/json/circuit.rs` (3) are mechanical
`.ordinal()` tokens at an `ordinal_to_string(...)` boundary, disclosed by record 1/n.
Re-verified this pass: `solution/ncim.rs` is untouched across `634aac9..HEAD`
(`NCIM_PQ_NODE`/`NCIM_PV_NODE` are still raw `i32`) and nothing else under
`report/export/**` moved. The merge coordinator resolves those 4 lines; no other
conflict surface exists.

**Non-fix, already recorded — audit-code 5 (second half): P3 §(d)** keeps the
self-monitored `clone_ckt` / `cap.clone()` copies. That is the one enumerated P3
bullet that did not land, and it is a legitimate escape under "remove ONLY if
provably behavior-identical": the copy dissolves only by restructuring four
`sample()` bodies into read-all-then-mutate, a per-class proof rather than a
mechanical rewrite. The P3 record already states it; re-affirmed, not dropped.

**Escape record unchanged.** The three open P1-tail items stand exactly as recorded
(the shared `CTRL_*` + `CTRL_STATE_KEEP` control-action channel; item 8's
reactor/capacitor `spec_type`, vsource `z_spec_type`/`scan_type`/`sequence_type`,
`vs_converter.f_mode`, `energymeter.ocp_device_type`; the `DynamicExp` RPN token
sentinels). None was partially touched by this pass. With `VoltageCurveXRef` and
`VoltWattYAxis` landed, deferred item **4** is closed *in full* — record 3/n's "the
InvControl family (6 enums)" reads **8**.

**Gate (solo, this tree):** `cargo fmt --all --check` exit 0 · `cargo clippy
--workspace --all-targets -D warnings` exit 0 · `cargo test --workspace` exit 0 —
**66 `test result: ok` groups, 2021 passed, 0 failed, 5 ignored** (2019 → 2021 = the
two new tests; the extended pins add assertions, not test entries),
`corpus_gate_all_cases_match_engines … ok` on both channels. `tests/corpus` left
pristine (27 run-artifacts removed by exact name off the status list — no wide
`git clean`). Ritual 0 held at start and before the commit: 186 `.pas` under
`.inputs/dss_capi`; `cargo` = `C:\Users\Admin\.cargo\bin\cargo.exe`.

### DE_PASCALIZE R3iv — the statically-classed object-ref fields are typed `Idx<T>`; R3.1 sub-step (iv) CLOSED (branch `depas-p1p3`, 2026-07-26)

Stratum **[A]** bit-neutral, **type-channel only** — no arithmetic, no visit /
registration / control-queue order, no list, no `find_*` tie-break touched. Zero
golden / ledger / tolerance / corpus-deck churn; `TODO(compat)` still **117 / 68
files**; no `#[cfg(feature = "oracle-parity")]`; no new `downcast`/`as_any` (the
only two in the tree stay the pre-existing `catch_unwind` panic-payload reads in
`tests/corpus_gate/runner.rs`, which are `Box<dyn Any>`, not elements).

**What this closes.** R3.1's escaped sub-step (iv) — carried forward untouched
through R3.2/R3.3/R3.4 and handed on by the R3 settler as "the one item the R3
wave hands to its successor". Its blocker was explicitly the *read* side: an
`Idx<T>` drops the class ordinal, so the fields could not be dereferenced until
`ClassArena::get::<T>` existed. R3.3 landed that accessor, so (iv) is now a plain
narrowing pass — exactly the "fold (iv) into R3 item 2" recommendation, executed
one wave later.

**The 34 fields, all `Option<ElemId>` → `Option<Idx<T>>`** (16 distinct names,
14 classes): `*_shape_ref` → `LoadShapeObj` (Load ×4 incl. `cvr_`, Generator /
WindGen / Vsource / Isource / IndMach012 / `InvBasedPceData` ×3 each — the last
shared by PVSystem+Storage); `*_t_shape_ref` → `TShapeObj` (PVSystem ×3);
`growth_shape_ref` → `GrowthShapeObj`; `inverter_curve_ref` /
`power_temp_curve_ref` / `vv_curve_ref` / `loss_curve_ref` → `XyCurveObj`;
`dynamic_eq_ref` (`DynEqPceData`, shared by Generator/WindGen/PVSystem/Storage)
→ `DynamicExpObj`; `Line::line_code_ref` → `LineCodeObj`;
`Transformer::xfmr_code_ref` (+ its `windings.rs` getter) → `XfmrCodeObj`;
`GicSource::line_ref` → `Idx<Line>`. The task brief also listed the
`line_geometry` geometry/spacing refs — those are **already** owned snapshots
(`Option<LineGeometryObj>` / `Option<LineSpacingObj>`, converted by R3.4), so
there was no `ElemId` left to retype; nothing to do and nothing skipped.

**One new accessor, no new bridge.** `ResolvedObj::idx::<T>() -> Option<Idx<T>>`
(`obj/arena.rs`) — a single `ArenaClass::idx_of` match arm, the storage-side
companion of the existing `get`/`cloned`. `ResolvedObj::get` now routes through
it, so the two cannot disagree. `Idx` is re-exported from `elements::traits`
beside `ElemId`. No per-class `set_object_ref_typed` was added (the stopgap the
escape protocol forbids): the shared `set_object_ref` signature is unchanged and
each impl narrows inline.

**Why the narrowing is total (the bit-neutrality argument).** The only writer is
`obj/props/class_props/parse.rs`'s `PropType::ObjectRef` `Some(class)` arm, which
resolves with `foreign.find(class, value)` for the property's **declared** class.
Every one of these 34 fields is declared `PropDef::object_ref_class(...)` with a
single class — verified by grep over all `object_ref_class` sites; the *only*
two-class `object_ref_two_classes` proxy in the tree is RegControl's
`transformer=` (`Transformer|AutoTrans`), which is not in this set. So a resolved
reference is always of the target class and `idx_of` is `Some` exactly where
`map(|o| o.id())` was `Some`; the `None` (miss / `ALLOW_NONE_REF`) path is
byte-identical. The companion `_obj` snapshot beside each `_ref` was *already*
narrowed by class (`o.cloned::<LoadShapeObj>()` etc.), so the pair is now
consistent instead of theoretically divergent.

**The three real readers, converted.** (1) `cim/power_xfmr.rs` case-2 XfmrCode
UUID: the class ordinal now comes from `XfmrCodeObj::CLASS_ORD` instead of
`cr.class_ord()` (the same number — the handle's class was already XfmrCode), and
the surviving `get::<XfmrCodeObj>` is the arena **bounds** check it always
effectively was. (2) `GicSource::recalc`'s deferred Line `Bus2` rewrite widens
back with `Line::id(idx.get())` because `RefAction::SetElementBus` speaks the
class-erased handle — same class, same index. (3) `exec/command.rs`'s
`set_resolved_line` caller narrows at the `foreign.find("Line", …)` site, keeping
the original `ckt()`-first order so the `line_missing` flag is set on exactly the
same inputs. Everything else reading these fields is `.is_some()` (Line dump /
CIM export / Line `spec_set` guards) or a `like=` field copy, all type-preserving.

**New pin.** `exec/tests/line_fetch.rs::typed_object_ref_handles_dereference_to_
the_named_object` — a deck with a LoadShape / GrowthShape / TShape / XYcurve /
LineCode / XfmrCode and a Load, Line, Transformer and PVSystem referencing them,
asserting (a) every typed handle is `Some` (the totality claim would fail loudly
here if a narrowing ever missed) and (b) each dereferences **through its own
class arena** (`ClassArena::get::<T>`) to the object the deck named, plus a
control that an unresolved `daily=` still leaves `None`.

**Gate:** `cargo fmt --all --check` · `cargo clippy --workspace --all-targets -D
warnings` · `cargo test --workspace` — green, exit 0, **66 `test result: ok`
groups, 2019 passed, 0 failed** (+1 over P3's 2018 = the new pin, which lands in
the existing `dss-core` lib binary, so the group count is unchanged),
`corpus_gate_all_cases_match_engines … ok` (both channels). `tests/corpus` left
pristine (the 22 run-artifacts removed by exact name, no wide `git clean`).
Ritual 0 held at start and before the commit: 186 `.pas`; `cargo` =
`C:\Users\Admin\.cargo\bin\cargo.exe`.

**Fence respected.** `exec/report.rs`, `report/export/**` and the
`solution/ncim.rs` cadence were not touched (the sibling `depas-og2` owns them);
no enum retype rippled into them. Files this step touched that another worktree
might also: none outside `crates/dss-core/src/{obj/arena.rs, elements/**,
cim/power_xfmr.rs, exec/command.rs, exec/tests/line_fetch.rs}`.

### DE_PASCALIZE P3 — borrow hygiene: the clone-to-release-borrow swarm (branch `depas-p1p3`, 2026-07-26)

Stratum **[A]** bit-neutral: no arithmetic, no visit order, no registration order
changed — every removed allocation carried values that are byte-for-byte the ones
now read in place. Goldens untouched, `TODO(compat)` still 117.

**(a) `solution/controls/dispatch.rs` — the fleet handle-list clones are gone.**
The store (`env.store`) lives *outside* the circuit, so a fleet list never had to be
copied to keep it mutable: the dispatch envs now borrow it. `GenDispEnv.generators`,
`UpfcDispEnv.upfcs`, `StorageDispEnv.storages`, `InvDispEnv.pv_systems`/`.storages`,
`ExpDispEnv.pv_systems` are `&'a [ElemId]`; `StorageDispEnv.season_signal` is
`&'a str`. The two per-sample `Vec<f64>` bus-kV-base scatters (`ckt.buses.iter().
map(kv_base).collect()`, one per Inv/Exp dispatch) are replaced by `buses: &'a
[Bus]` + `.kv_base` at the point of use, and the `MonBus` resolution stores
`Vec<&'a [usize]>` instead of cloning each bus's whole `ref_no` array. The two
`UpdateAll` sweeps (`update_all_inv_controls` / `update_all_exp_controls`) had
cloned `controls`/`pv_systems`/`storages`/`bus_kvbase` **per control** inside the
loop — now a single `Circuit` field split (`controls`, `pv_systems`, `storages`,
`buses`, `bus_list`, `solution`) borrows all of them at once. 13 `Vec` clones per
control sample removed; the lists are the same objects in the same creation order,
so every fleet scan sees exactly what it saw before.

**(b) full-vector copies per step/frequency/bus.** `solution/solution/dynamics.rs`
(`calc_initial_machine_states`, `integrate_pc_states`) and `harmonics.rs`
(`initialize_for_harmonics`) copied the whole `node_v` once per half-step /
per sweep only to satisfy a borrow that was never in conflict (store ⟂ circuit) —
now `&ckt.solution.node_v` directly. `savePresentVoltages` uses `clone_from`
(reuses the saved allocation; identical contents). `fault_study.rs`:
`compute_ysc` splits `Circuit { buses, solution, .. }` and reads the bus `ref_no`
in place instead of cloning it per bus (N clones per fault study), and
`compute_isc` multiplies straight out of `b.vbus` (disjoint field from
`b.bus_current`). `solution/monitors.rs`: the `SampleAll`/`SaveAll`/`ResetAll`
sweeps walk `&ckt.monitors` instead of cloning the list per sample.

**(c) `elements/meter/monitor/sample.rs` — per-sample scratch buffers.** Pascal
keeps `VoltageBuffer`/`CurrentBuffer` as object fields (`Monitor.pas:146-147`,
sized in `RecalcElementData`); the port allocated two `Vec<Complex64>` on *every*
sample of *every* monitor. They are now `Monitor` fields, lent to the sample body
via `mem::take` + hand-back (`take_sample` → `take_sample_into`), because
`add_dbl` needs `&mut self` while the record is written. The body still
`clear()` + `resize(n, ZERO)`s them at exactly the point the `vec![ZERO; n]` used
to run, so their contents entering the match are identical (mode 12 relies on the
zeroed tail past `NPhases`). Mode 12's `vterminal.clone()` is also gone (read in
place; the metered element is untouched until the post-loop `compute_iterminal`).

**(d) self-monitored control clones — LEFT IN PLACE, deliberately.** The
`mon == target` arms of Fuse/Recloser/Relay (`clone_ckt`) and CapControl
(`cap.clone()`) hand the *monitored* role an owned copy of the controlled element.
This is not a borrow-hygiene wart that a typed pair getter can dissolve: upstream
the two roles are the **same object**, so removing the copy needs one `&mut` used
for both roles — impossible without restructuring each `sample()` into
"read every monitored quantity, then mutate the controlled side". That reordering
is only equivalent if no controlled-side mutation precedes a monitored read in any
of the four classes, which is a per-class proof, not a mechanical rewrite. P3's
rule is "remove ONLY if provably behavior-identical, else leave and record" →
recorded. (The copies are already value-neutral: the monitored role's only
mutation is the `Iterminal` cache, recomputed identically from the same `NodeV`.)

**Surveyed, out of P3's scope (follow-up candidates):** `ckt.<list>.clone()`
survives in `controls/sampling.rs` (2), `faults.rs` (2), `meters/**` (10),
`solution/monte_carlo.rs` (2), `time_series.rs`, `power_flow.rs:331` and
`ymatrix.rs:216` (`ckt_elements` per Y rebuild). Each of those loops passes
`&mut Circuit` into the body (`dispatch_control`, meter samplers), so removing the
clone means an index loop over a snapshot length — a different failure mode if a
body ever mutates the list, i.e. a real per-site proof rather than a borrow fix.
`solution/solution/ncim.rs` (4 more) is fenced off to the concurrent `depas-og2`
worktree and was not touched. No file the fence names was modified.

**Gate:** fmt · clippy `-D warnings` · `cargo test --workspace` (corpus gate, both
channels) — green, 0 failures; `tests/corpus` pristine; goldens untouched.
