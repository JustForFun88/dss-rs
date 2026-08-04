# DE_PASCALIZE Stage F — the `oracle-parity` lane split, F.1 through the wave-4 settlement

> Moved verbatim from `STATUS.md` on 2026-08-05 (STATUS.md history archiving);
> order preserved, nothing rewritten.

### DE_PASCALIZE Stage F settlement (wave 4), part 3 — the last gate hole: a lane split that is not an alias (branch `depas-stagef`, 2026-08-01)

Three residuals from the audit round, closed.

**A lane split written as two cfg-selected *definitions* was invisible to the
whole pin machinery.** `lane_aliases` recognises one shape — a `#[cfg(...)]`
line immediately followed by `pub use X as Y;` — so a row spelled instead as

    #[cfg(feature = "oracle-parity")]     pub const ROW: f64 = 1.0;
    #[cfg(not(feature = "oracle-parity"))] pub const ROW: f64 = 2.0;

is a genuine split that never counts toward `SPLIT_ALIAS_POPULATION` and is
never asked for a pin. Reproduced: appending such a pair to a *sanctioned*
compat module passed every test in the file. `every_lane_cfg_in_a_compat_module_
selects_an_alias` now requires every needle-bearing cfg in a compat module to be
an alias arm (or one of the six `ORACLE_PARITY` arms the lane model itself
needs), and fails on the probe. A comment between the attribute and its item
breaks the parser the same way and is rejected too. That was the last of the
five gate holes the round found.

**`GENERATOR_POSSEQ_RATING_GUARDS_READ_XDP_SLOTS` gains its r4133 evidence.**
The row rested on internal dss_capi evidence alone, which §D14 says is not
enough for adopting a fix. r4133 settles it: it writes the same two literals
`PrpSequence^[26]`/`^[27]` (`generator.pas:3061-3062`), but there they are
*correct* — its registration puts `kVA` at 26 and `MVA` at 27 (`:459`/`:460`)
with `Xdp`/`Xdpp` at 29/30. The guards were right upstream and were broken by
the 0.14.5 enum reorder, so the default lane is what upstream does.

**`fpc_general_digits` is `pub`**, which removes the one rustdoc warning F.4
introduced (`fmt_g_native_impl` linking a private item) now that both `%g`
kernels call it.

**Proof.** `cargo fmt --all --check` clean; both clippy invocations exit 0;
`cargo test --workspace` and its parity twin both **69 test binaries, 0
failed** (the cfg gate is now 8 tests), corpus gate green inside each;
`git status --short tests/corpus` empty after each, artifacts deleted by exact
name; `git diff --stat 6ef40238 -- tests/` **empty** across the whole
settlement — no golden, ledger entry, deck or tolerance moved.

### DE_PASCALIZE Stage F settlement (wave 4), part 2 — the operational docs stop over-claiming, CI grows its second lane, and the no-split evidence becomes re-runnable (branch `depas-stagef`, 2026-08-01)

Part 1 settled the code. This one settles the record: the audits found several
places where a document asserts more than the instrument delivers, and one where
the instrument was genuinely missing.

**CI ran one lane while `CLAUDE.md` declared two.** `.github/workflows/ci.yml`
ran `cargo fmt` + one clippy + one `cargo test --workspace` — the *default*
lane only. But the parity lane is not a variant worth spot-checking: it is the
lane that carries the byte goldens and the **exact** iteration comparison
against the oracle, i.e. it is the compensating control for every band the
default lane is allowed. CI now runs all five gate commands.

That also disposes of an audit finding rather than confirming it. The ±1
iteration band was reported as having its control outside `cargo test`, on the
grounds that the control is `tools/lanes/lane_diff.ps1`. It is not: the 13
routed sites are `assert_eq!(rust, oracle)` in the parity build, which is a
mandatory gate command; `lane_diff.ps1` compares the two *lanes*, not either
lane against the oracle. Recorded at `ITER_SLACK` so the next reader does not
re-derive it the wrong way round.

**"parity == oracle bitwise" was a false premise for a true conclusion.** Four
places said it (`CLAUDE.md`, `TESTING.md`, `lane_dump.rs`, the plan). The parity
lane is byte-exact on the committed goldens and oracle-gated at the calibrated
`TOLERANCE_NOTES` floors — it has its own pinned ledger entry on the
`capi_v0145` channel — so the honest chain is the triangle inequality. The
conclusion survives only because the measured `|default − parity|` came back
**exactly 0** on every gated kind, which makes the default lane bit-identical to
the parity lane and hands it that lane's oracle standing outright. All four now
say that, and say what to do when Δ stops being zero (bound = job bound **plus**
the case's tier) — which `TESTING.md` itself predicts for M3c and WP-R1.

Two neighbouring over-claims went with it: the differential job does not dump
"the full corpus checkpoint stream" (no meter registers, monitor channels, event
log, control queue, probes or report text — the corpus gate compares those live,
in both lanes), and it walks 520 cases while *solving* the 516 that are not
abort-by-design. `CLAUDE.md`'s corpus-gate paragraph still said 514.

**The plan's "closed inventory" contradicted the tree, 11 rows against 38.**
IV.2 says its dual-kernel table is "the complete, closed inventory — do not
invent new compat items", and the executor guidance repeats it. The tree carries
38 lane-split aliases. The extra ~23 are not inventions — they are per-site
upstream quirks, the class `PORTING_PLAN.md` §4.1 rule 4 (Update 2026-07-06)
rules must "become a dual kernel behind `#[cfg(feature = "oracle-parity")]`" —
but the reconciliation existed only in `compat.rs`'s module header, so the plan
alone read as either a stale inventory or a violated rule. IV.2 now scopes its
table to the **shared arithmetic kernels** (which is what it is), names the
second class with its governing rule, and points at `SPLIT_ALIAS_POPULATION` as
the mechanical counter. The Mechanism paragraph also listed two compat modules;
there are three (`dss-parser`'s was missing).

`§Verification`'s spot-check list still claimed `seq_currents.rs` has "no
`(j-1)*ncond` in sight" — falsified by Stage F's own F.3c fix, which
reintroduced exactly that form as the default lane's `Iresidual` base. Corrected
in place, since a stale self-check is precisely what makes an acceptance
checklist useless. The flat-offset metric was re-scoped the same way (see part
1): 17 → **15**, "all accessor-internal" → the four survivors that are not, and
the `(… - 1) *` form recorded as deliberately retired rather than silently
dropped.

**Two no-split verdicts were prose-only; one now has its script.**
`tools/lanes/cdiv_sweep.py` re-derives the complex-division row: `references`
prints the exact `from_bits` literals the test carries, `sweep` prints the
aggregate. Running it also corrected the recorded figures — the old
9.42e-17 / 3.82e-16 against 1.05e-16 / 4.26e-16 came from decimal-literal
references rather than the operands' binary values; the honest pair is
5.68e-17 / 3.68e-16 against 7.73e-17 / 4.50e-16, same ordering. The dense-inverse
row's "495 872 `Zb` inversions" is left as prose but relabelled a *survey*: it
came from a one-off instrumented build and is not what the verdict rests on —
that is `dense_inverse_kernels_differ_by_one_ulp_on_an_ideal_switch` and
`tests/compat_dense_inverse.rs`, both re-runnable in-tree.

**Audit findings disposed of as refuted, with the evidence.**

- *"Load-bearing r4133 citations are not re-checkable from the repo."* They are:
  `.inputs/electricdss-code-r4133-trunk` holds **949** `.pas`, and every cited
  line resolves exactly. Two rows were being read against the wrong file — the
  height-unit row cites r4133 throughout, and that surface does not exist in the
  0.14.5 backend at all; the row now says so.
- *"`LANE_SKIP_PROBE_PROPS` has three inert cells."* All four are load-bearing:
  `MODES` sets `compare_all_properties`, so three of them feed the
  whole-element dump rather than a manifest probe. Dropping any of them fails
  the case.
- *"The ±1 iteration band has no in-gate control."* See above.

**Also recorded, deliberately not changed.** `lane::compare_report`'s scoping
guard accepts `inf`/`nan`/integer text, so it cannot mechanically detect a
golden routed into the split whose numbers are all integer text — the
"iff its writer renders a number through the F-FMT seam" rule stays a review
obligation. Tightening the predicate to require a rendered *float* would reject
goldens that legitimately carry only integers, which is a worse failure mode
than the one it prevents. `PARALLEL_FACTORIZATION_PARITY_IMPL` keeps its name
but gains a note on the const itself: no call site reads it, both arms alias to
it, and faer 0.24's `Lu::try_new_with_symbolic` calls `get_global_parallelism()`
regardless — so it is a declaration awaiting M3c, not a contract the build
honours.

**For the coordinator, on merge.** `PLAN_SEQUENCE.md`'s HIDE_015X row points at
"item 4's open tail", which exists on `update` (commit `e352dc1c`, after this
branch point) and not here — verify it is present after merging. That commit's
text also says both HIDE_015X tripwires live in
`crates/dss-core/tests/oracle_parity_cfg_gate.rs`; one of them,
`hide_015x_carrier_set_is_the_measured_escape`, is at
`crates/dss-core/src/exec/tests/compat_quirks.rs:515`.

**Proof.** `cargo fmt --all --check` clean; both clippy invocations exit 0;
`cargo test --workspace` and its parity twin both **69 test binaries, 0
failed**, corpus gate green inside each, `git status --short tests/corpus` empty
after each (known intermittent `Test/AutoTrans/*` artifacts deleted by exact
name).

### DE_PASCALIZE Stage F settlement (wave 4), part 1 — one real parity-lane divergence closed, four gate holes shut, the evidence re-measured (branch `depas-stagef`, 2026-08-01)

Seven scoped audits ran over the whole Stage F range, each finding then
re-verified empirically. This commit settles the code half. Nothing here
re-baselines a golden, moves a ledger entry or touches a tolerance:
`git diff --stat 6ef40238..HEAD -- tests/golden tests/corpus` is **empty**.

**The one real defect: `Set time=` never reached the round row.** F.3a retired
19 `TODO(compat): FPC Round` markers on the premise that "every engine-internal
`Round` this port makes operates on bounded magnitudes". That is false wherever
the input is a *raw deck double*, and the parity lane was measurably wrong
because of it. Probed against the pinned oracle: `set time=(3e9,0)` then
`Solution.Hour` gives `-1294967296` upstream and gave `2147483647` here, while
the sibling `set hour=3e9` — routed through `dss_parser::compat::round_i32`
since F.1 — matched exactly. Five sites are now routed through the row's kernel
(bit-neutral in the default lane, which *is* the saturating cast): `Set time=`,
PstCalc's `CyclesPerSample`, both `Round(24/stepsize)` day counts, and
GrowthShape's base year. The premise at `reg_control/mod.rs` is rewritten to
what is true, and told not to be restored.

The same sweep left the `ApplyRound` **array** path unrouted, and that one is
observable too: Pascal writes `Round`'s Int64 *back into the Double*, so
`year=[1e20]` reads back `-9.22337203685478E18` from the oracle where we kept
`1e20`. The round row gains its array kernel (`compat::round_f64` — the same
`Round`, differing only in the Pascal assignment target), so
`SPLIT_ALIAS_POPULATION` moves **37 → 38**. Writing its pin also surfaced a
pre-existing default-lane panic: GrowthShape's `Inc(Yr)` is a Pascal `Integer`
increment with range checks off, and a saturated base year made `cur_year += 1`
overflow in debug — now `wrapping_add`, as Pascal has it.

**Four gate holes, each reproduced before and after the fix.**

- *A silently reverted flip used to pass everything.* Five of the 38 rows
  derived their expected value from the row's **own** alias, so engine and test
  read the same constant and the pin asserted only "the engine agrees with the
  declaration" — true whichever way the alias points. Flipping those five
  `*_DEFAULT_IMPL`s back to the parity value passed the entire workspace suite
  in both lanes; no oracle gate can catch that class, because reverting a fix
  restores exactly what the oracles return. All five now derive from
  `compat::ORACLE_PARITY`, and `branches_on_lane` **rejects** the
  self-referential form outright, so it cannot come back.
- *A fourth `compat` module was sanctioned by construction.* `is_sanctioned`
  whitelisted any path with a component named `compat`; it is now the three
  registered modules. Probe: an unregistered `src/audit_probe_zone/compat.rs`
  carrying two cfg arms passed before, fails now.
- *A gutted test module still satisfied the pin gate.* `is_pin_candidate`
  accepted any file containing the substring `#[cfg(test)]`, and
  `branches_on_lane` matched anywhere in it — so a **production** call site
  satisfied its own row. Probe: emptying `dispatch.rs`'s test module (leaving
  `#[cfg(test)] mod tests { #[test] fn placeholder() {} }`) kept the gate green
  with `kv_base_search_scale` unpinned; it now fails. Only the region after the
  `#[cfg(test)]` boundary counts, and it must contain a `#[test]`.
- *`compat::ORACLE_PARITY` was an unpoliced second lane-branch channel.*
  Product code could write `if compat::ORACLE_PARITY { .. } else { .. }` and
  carry none of the cfg text the grep gate searches for. A new test rejects
  reads outside the compat modules and test code; it found exactly one
  legitimate reader — `lane_dump`'s dump header — which is why
  `crates/*/examples/**` is allowed, and why that header exists at all.

**The differential job could not tell a lane from itself.** `diff` skipped the
`lane` header before comparing it, so two dumps both headed `default` returned
`VERDICT: PASS`, exit 0 — the one instrument that certifies default ≈ oracle,
degrading into comparing a lane with itself exactly when the feature failed to
propagate. It now requires `{default, parity}` (probe-confirmed both ways). A
NaN on either side was also invisible — every comparison against NaN is false,
so it did not even reach the statistics — and is now a hard failure;
`DOCUMENTED_DIVERGENCES` became fail-on-stale; the tab guard became a real
`assert!`, since the job runs `--release` where `debug_assert!` is off.

**The event-log re-round list was the one Stage F list that was not
fail-on-stale** — while the module doc claimed all three were. Two invented
cells, one with a key occurring nowhere in the corpus, passed the whole gate
unremarked. Cells are now keyed by **case** (they were applied to every line of
every gated case), guarded against matching twice in one log, and their
liveness is asserted once at the end of the corpus gate. Wiring it up found the
right shape empirically: the carrier has 24 compared checkpoints, cell 1 fires
in all of them and cell 2 in 23 — the log grows as the case steps — so liveness
is a per-case property, not a per-step one. Probe: a third, bogus cell on the
same case now fails the gate. `reround_cell` also indexed the original line
with offsets computed on a Unicode-lowercased copy, which silently missed one
probe input and panicked on another; ASCII folding makes the offsets correct by
construction.

**`props_roundtrip` excluded 69 value comparisons where 33 were sanctioned.**
`LANE_SKIP_PROP_VALUES` matched on property *name* alone, and
`RMatrix`/`XMatrix`/`CMatrix` are also property names on `Line` and `LineCode`,
where the oracle values are real numbers and nothing about
`SYM_MATRIX_GETTER_RENDERS_ZEROS` applies — only `Capacitor`, `Fault` and
`Reactor` declare `DoubleSymMatrixProperty` in the pinned backend. Proven live:
corrupting a `LineCode.RMatrix` golden number passed the default lane and
failed the parity lane. Now keyed `(class, property)`, with the cell count
asserted as an **equality** (33) instead of a `>=` that any count satisfied.

**Evidence re-measured rather than re-asserted.** The `DIV_REFERENCE` table was
computed from the decimal *literals* rather than the operands' exact binary
values, and that conversion's own half-ULP slip is the same order as the
quantity being measured: three of six rows were 1 ULP off, all in the imaginary
part, and every error flattered the kernel that was kept — the table scored the
kernels 1 vs 14 ULP where the truth is **5 vs 12**. Re-derived in exact
rationals; the verdict is unchanged and the score is now pinned. The docstring
also claimed Smith must be at least as close "on every row"; an unfiltered
20 000-pair sweep has Smith strictly *worse* on 24.7%, so the rows are labelled
what they are — a Smith-wins filtered sample — and the headline is the
aggregate the test actually asserts. `INVERT_REL_BOUND`'s recorded triple still
described the pre-F.3i kernel; re-measured to 1.93e-16 / 2.30e-16 / 1.31e-16.

**The `%g` battery asserted two invariants that are false.** "Notation never
moves" and "at most one character wider" were justified structurally, but both
kernels apply the shared window to the *rounded* exponent and they round
differently. The committed battery itself disproves both at `sig = 6` — the
engine's second-most-used precision, which the battery's `[2, 5, 8, 15]` sweep
never reached: `9.999994999999999e5` renders `1E6` against `999999`, a notation
flip three characters wide. The battery now sweeps the **11** precisions the
engine actually renders at (229 divergences, up from 225) and pins notation
flips and width excess as *measured populations*. The genuine kernel property —
that the two renders re-parse within one unit in the last printed place — is
still asserted on every render.

**Not fixed, and why.** The audits asked for a parity-lane byte gate on the
numeric `Show` goldens. Tried, and it is impossible for that family: those
goldens are not byte-reproducible in *any* lane, for two independent
long-standing reasons — physical near-zero cancellation (`show_losses` line 26
is `-4.36557E-14` kvar in the oracle against `-1.45519E-14` here) and the
bus-name column width (`max_bus_name_length` differs from the oracle's, so
`Show Voltages`' header is `"Bus" + 8` spaces against `+ 3`). Both predate F.4:
the pre-F.4 writer builds that header with the identical
`format::pad("Bus", mbnl)`. Recorded at the driver, with what carries the
layout contract instead. Routing those drivers through `lane::compare_report`
fails 27 goldens; do not retry it.

Also settled: the `Pad`→uppercase reorder F.4e introduced is made provably
byte-neutral by ASCII folding (Pascal's `AnsiUpperCase` is byte-length
preserving; `str::to_uppercase` is not, and the engine accepts a bus named
`busı1`); the flat-offset ceiling drops 17 → **15**, the number the gate
actually counts, closing two free slots; `kv_value_eq` is scoped to the
exact-value policies rather than to the lane, so a future report combining a
physical floor with `key=value` cells cannot silently become a tolerant
compare; the contamination artifact keeps `Monitor.BaseFreq`, which is
deterministic and never belonged in a strip set justified by upstream UB;
`fpc_general_digits` is now called by both `%g` kernels; and
`monitor_base_frequency` gains the direct both-impls test IV.2's Mechanism
clause requires — it was the only row without one.

**Citations.** Six were wrong or unresolvable and are corrected against the
vendored source: `fff.pas:69` (a file that exists nowhere) → `RPN.pas:69-70`;
`CktElement.pas:233` → `:203`; `Line.pas:1791`/`:1793` → `:1776`/`:1778`;
`DSSPointerList.pas:66` → `:88`; `CapControl.pas:446-490` → `:445-489`; and
LineCode's "`Create` sets `HrsToRepair := 3`", which is simply not in the
source — `TLineCodeObj.Create` never assigns the field; the `:= 3.0` belongs to
`TLineObj.Create` (`Line.pas:979`). The pi row's measured gap was recorded in
the wrong direction: `3.14159265359` is *above* pi, which is why the parity
lane's `sin 30°` is the larger `0.5000000000000299`.

**Two rows re-grounded on r4133, which is vendored.** The audits called their
citations unverifiable; `.inputs/electricdss-code-r4133-trunk` holds 949 `.pas`
and every one resolves exactly (`ExpControl.pas:184`, `Relay.pas:1196`/`:1212`/
`:1325`, `LineConstants.pas:71`/`:97`/`:689-696`/`:396-399`). The height-unit
row's citations were being resolved against the *0.14.5* backend, which has no
such surface at all — noted in the row so the next reader does not repeat it.
`LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING` genuinely rested on 0.15.x alone,
and §D14 says that is not an authority: r4133's `TLineSpacing.MakeLike`
(`Version8/Source/General/LineSpacing.pas:236-262`) copies all five
equivalent-spacing fields explicitly, so the **default** lane is what upstream
does and the 0.15.x omission is the regression. Recorded in the row.

**Proof.** `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity` both exit 0; `cargo test --workspace` and its parity
twin both **69 test binaries, 0 failed**, with the unconditional 520-case
corpus gate green inside each. Both runs leaked the known intermittent
`Test/AutoTrans/*` artifacts, deleted by exact name; `git status --short
tests/corpus` empty after each.

### DE_PASCALIZE Stage F.5 — CLOSED, and with it the whole plan: the metrics become tests, the exit criterion becomes a sentence that can fail (branch `depas-stagef`, 2026-07-31)

F.5a landed the differential job. This commit closes the step, the stage and
`DE_PASCALIZE_PLAN.md`: the plan's §Verification success metrics stop being a
paragraph someone re-greps by hand, the exit criterion is written in the form
that was actually *ruled*, and the three things the stage hands forward are
recorded in all three places that could otherwise lose them.

**The success metrics are now a test**
(`crates/dss-core/tests/depascalize_metrics_gate.rs`, 5 tests). Each metric in
§Verification was an `rg` with a target, re-measured by hand per stage and
recorded in prose — a *report*, not a contract. Nothing re-ran them, so a later
WP could reintroduce a downcast, an `Rc`, or a flat offset and no gate would
notice; making those shapes impossible to have is the entire point of Parts
I–III. Re-measured on this tree, and each one gated:

| metric (plan §Verification) | 2026-07-26 | now | form |
|---|---|---|---|
| `downcast_ref\|downcast_mut\|as_any` in `dss-core/src` | 369 / 716 | **0** | zero |
| `RefCell\|Rc<\|static mut\|thread_local` in `crates/*/src` | 0 | **0** | zero |
| `term_ref[` | 0 | **0** | zero |
| `pub const …: i32` in `elements/` | 7 | **0** | zero (the P1 tail closed the keep list) |
| `for … in 1..=` in `elements/` | 106 | **106**, all STAYS-by-design | ceiling |
| flat-offset `* nconds` / `* ncond` | ~23 | **17**, accessor-internal | ceiling |

Two shapes, deliberately: *zero* for the architectural invariants, *ceiling* for
the audited populations Part III left standing (they may shrink — that is a
welcome diff — but not grow). Counting is **code-only**: a hit counts only
before any `//` on its line, because `lib.rs` documents the `Rc`/`RefCell` ban
using the words it bans and `plot/tests.rs` explains what the pre-R1 design
used; a gate that counted prose would force the documentation to stop naming
what it forbids. Every walk carries a file-count floor **and** a named anchor
file (a broken walk fails loudly instead of reporting a clean zero), and the two
ceilings assert their populations are non-empty for the same reason.

**The exit metric, restated as it was ruled.** The plan's literal "at Stage F
exit: **0** markers" is replaced, in the plan and in `ORPHANED_GAPS.md`, by
*zero **unclassified** markers; zero carriers beyond the pinned escape; every
escape gated by a test*. This is not a softening after the fact: F.3 established
by **measurement** that two whole classes of clean fix are outside any Stage F
step's authority (a whole-case default-lane oracle exclusion; an UPGRADE-rung
oracle re-baseline), and the register that replaces the count is strictly
stronger than a number, because a number cannot rot while a fail-on-stale
register can only be moved deliberately: an unregistered marker fails, a
registered marker that vanished fails, and each survivor names its successor.
Population at exit: **18** = 11 `UpgradeRung` + 4 `WholeCase` + 3 `WasmGuest`,
plus the one non-marker escape.

**The `HIDE_015X` hand-off, written into all three places.** F.5 opens **no**
golden event, so the hide-flag bundle is `UPGRADE_PLAN` §5's — recorded now in
`DE_PASCALIZE_PLAN.md` §"Stage F as executed" (with the reasoning and both
tripwires), `ORPHANED_GAPS.md` §2 (its own row, no longer buried in the Stage F
row), and `PLAN_SEQUENCE.md` item 5 (which item 4 already carries as the UPGRADE
line's open tail). It is *one atomic change in both lanes* — un-hide the five
props, drop the `Line.Wires → "Conductors"` `json_name` masquerade, regenerate
the 13 gated artifacts (8 `Dump` texts + 5 JSON documents; no `Show` report, no
corpus case), delete the flag — and it moves **parity-lane byte goldens**, which
only an oracle-surface switch may do (`gen_json.py` is hard-pinned to 0.14.5;
§5 re-pins it). Its tripwires are named at every mention so a partial touch
fails a gate rather than passing quietly:
`oracle_parity_cfg_gate.rs::the_hide_flag_escape_population_is_pinned_by_surface`
(the 13 artifacts + their surface classification) and
`exec::tests::compat_quirks::hide_015x_carrier_set_is_the_measured_escape` (the
5 carriers + the sibling flag's carrier-free state that makes the measurement
isolate this row). Note for whoever reads `PLAN_SEQUENCE.md`'s tail entry
(added on `update` after this worktree branched, so it is not edited here): it
says "two pins in `oracle_parity_cfg_gate.rs`" — one of the two actually lives
in `exec/tests/compat_quirks.rs`.

**A diagnosability bug found by the flake in F.5a's record, and fixed here.**
That record advised "if it recurs, capture the panic message with
`--nocapture`". That advice was **wrong**, and finding out why is the more
useful result: `harness::lane::tests::passes` — the helper F.2 added so a
deliberately-failing comparison does not print a backtrace — installed a
**no-op panic hook for the whole process** while its closure ran. The panic
hook is global and those tests share their binary with
`corpus_gate_all_cases_match_engines`, so any panic on any thread inside that
window printed nothing; the corpus gate's failing-case list is produced by
exactly that mechanism, which is why the log said "519/520, 1 failed" and named
no case, twice, with `--nocapture` making no difference. Two overlapping calls
could also restore each other's saved hook and leave the no-op installed for
good. `passes` now installs its hook **once** and delegates to the previous one
unless a **thread-local** flag says the panic is its own — silence stays scoped
to that thread inside that helper, and a concurrent failure keeps its message.
(The flake itself remains unidentified and did not reproduce in three targeted
re-runs — including the whole 41-test `corpus_gate` binary, which reproduces the
same intra-binary concurrency. Both lanes are green on the committed tree. The
next occurrence will now name its case.)

**Plan status.** `DE_PASCALIZE_PLAN.md` header, §Ordering summary, §Verification
and Part IV.2 now read COMPLETE with the executed outcome; `ORPHANED_GAPS.md`
§2's Stage F row is closed and the three hand-offs each have their own row;
`PLAN_SEQUENCE.md` item 5 is COMPLETE, which unblocks RESONANCE WP-R1 and
MULTITHREADING M3c — the two rungs that will make the lanes genuinely diverge,
and therefore the first real consumers of F.5a's differential job.

**Proof.** `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` and the `--features dss-core/oracle-parity` twin
both exit 0; `cargo test --workspace --no-fail-fast` **2480 passed / 0 failed /
5 ignored** and the parity-lane twin identically **2480 / 0 / 5** (+5 = the new
metric gates), the 520-case corpus gate green inside each, `git status --short
tests/corpus` empty afterwards. No golden, tolerance, ledger entry or deck was
regenerated — the whole stage never spent its one sanctioned re-baseline.
`SPLIT_ALIAS_POPULATION` stays **37**, `TODO(compat)` **15** in `crates` / 18
tree-wide.

### DE_PASCALIZE Stage F.5a — the differential gate lands, and the two lanes come back **bit-identical** (branch `depas-stagef`, 2026-07-31)

Plan IV.2 specifies the parity↔default differential gate as "a CI job, not a
unit test — the two kernels live in different builds", and calls it "the single
strongest default-lane test". This commit lands it as a locally runnable
scripted job, runs it once, and records what it measured.

**Shape.** The moving part is `crates/dss-core/examples/lane_dump.rs` — an
*example*, so `cargo clippy --workspace --all-targets` lints it and `cargo test`
compiles it in **both** lanes (proven, not assumed: a deliberate `len_zero`
planted in it fails the default-lane clippy gate). It has two subcommands.
`dump` walks all **520** manifest cases exactly as `run_rust_capture` does
(`clear`; `compile`; the manifest's `post`; `n_steps` × `solve`) and writes one
record per compared quantity; `diff` streams two dumps and checks them. Neither
depends on the lane, so the same binary serves both. `tools/lanes/lane_diff.ps1`
drives all three steps into `target/lanes/`, each lane into its own target dir
so a re-run does not thrash the other lane's cache.

**What it compares, and against what.** Engine error count, per-step
convergence flag and iteration count, every node voltage, every element's
terminal currents / powers / losses, and the assembled system Y (once per case,
in the state the last step left it). Record *keys* are compared exactly and in
order — a renamed, reordered, dropped or added record fails structurally before
any number is read. Values go against the **tightest calibrated oracle tier**,
`tol_for("micro")`: `|Δ| ≤ 1e-6 + 1e-9·|parity|`, plus the same ±1 iteration
band `harness::lane` grants the default lane against the oracle. Reusing the
micro tier is deliberate and is a *tightening* — the corpus's feeder/large
tiers are 1e-8 … 5e-6, so every case is held here to a bound at or below its
own oracle comparison's.

**The measurement (2026-07-31).** 520 cases, **3 219 862 records**, ~4.8 M
compared values: `v` 375 692, `cur` 1 169 132, `pow` 1 169 058, `loss` 366 219,
`y` 1 738 004, `iter` 2 141, `conv` 2 141, `errs` 516 — **`max |Δ| = 0` on every
one**. Not "inside the bound": bit-identical. Iteration counts that drifted: 0.
The only measurable divergence is the deliberate Newton row — `newton.dss` `pow`
max |Δ| 4.856e-4 (rel 8.195e-7), `loss` 4.516e-1 (rel 4.449e-7);
`newton_feeder.dss` `pow` 3.084e-3 (rel 9.630e-7), `loss` 5.213e0 (rel
7.024e-7). That 4.86e-4 independently reproduces, from the other side, the
number `harness::lane::LANE_SKIP_ELEM_POWERS` documents for the same exclusion.

**Why bit-identical is the right answer and not a dead instrument.** F.3
measured each kernel row on the corpus *before* flipping it, and flipped only
the ones that moved no corpus case; the rows that would have moved one are
exactly the ones it blocked (the four `WholeCase` escapes) or handed to
`UPGRADE_PLAN` (the eleven truncated-constant rows). The flipped rows'
observables are reports, event logs and property surfaces — which this job does
not dump, and which the two golden lanes gate instead. So the corpus-level
electrical model agreeing bit-for-bit is what F.3's records predict. The
non-vacuity is asserted (the diff refuses to print PASS below 100 000 records
or without the `v`/`y` kinds) and demonstrated (the same comparator reports the
Newton rows).

**Two findings from the first run**, both of which would have silently
corrupted the record:

1. `|` is not a safe field separator for DSS names — the corpus contains a
   `Line.b1||b2`, which reshapes a `|`-separated record into extra fields. The
   record separator is a **tab**, with a `debug_assert` on the key.
2. A corpus run pollutes in **three** shapes, not the one the previous records
   describe: untracked report files (the known `Test/AutoTrans` set), whole
   created directories (`DI_yr_*` EnergyMeter output, six of them), and three
   *tracked vendored* files that decks overwrite under their own name
   (`Test/LineConstantsCode.DSS` from `Show LineConstants`, the two
   `IEEE_519_Mon_mpcc_1.csv` monitor exports). The script deletes the first two
   by exact path (refusing any reparse point) and `git restore`s the third —
   never a wide `git clean`.

**Docs.** `CLAUDE.md`'s gate definition now names **both lanes** (five commands)
plus the on-demand differential job; `TESTING.md` gains §"The two lanes (Stage
F)" — what each lane asserts, and that Stage F introduces no tolerance — and
§"The parity↔default differential gate" with how to run it, what it costs
(~215 MB per lane, two release builds, ~3 min of solving) and when to run it
(any `compat` kernel / lane alias / solver change; M3c and WP-R1 are the two
planned changes that will make the lanes genuinely diverge).

**Proof.** `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` and the `--features dss-core/oracle-parity` twin
both exit 0; `cargo test --workspace --no-fail-fast` **2475 passed / 0 failed /
5 ignored** and the parity-lane twin identically **2475 / 0 / 5** (F.4f's
counts — this commit adds no test), the 520-case corpus gate green inside each,
`git status --short tests/corpus` empty afterwards (the known intermittent
`Test/AutoTrans/*` artifacts deleted by exact name). No golden, tolerance,
ledger entry or deck was regenerated; `SPLIT_ALIAS_POPULATION` stays **37** and
`TODO(compat)` **15** in `crates` / 18 tree-wide.

**One flake seen and re-verified, recorded rather than swallowed.** An
intermediate default-lane run reported `corpus_gate 519/520 … 1 failed` on a
tree whose only delta from the green run before it was three Markdown files
(no test reads them; `operational_docs_cite_the_compat_machinery_accurately`
lives in a different binary and passed). Re-running `corpus_gate` alone came
back green in 134.6 s, and both full lanes are green above — so this is the
worker-transport flake class the gate already retries around
(`DSS_ORACLE_TIMEOUT_SECS` → kill/respawn/retry-once), not a result. Worth
noting for the next session: the failing case's name did not survive into the
captured log, so if it recurs, capture the panic message (run the gate alone
with `--nocapture`) before assuming the same cause.

### DE_PASCALIZE Stage F.4f — the two matrix dumps close F.4e's escape; §F-FMT step 2 has no report left (branch `depas-stagef`, 2026-07-31)

F.4e's own escape row was `Show Y` and `Show Yprim`: they use no `Pad`, so they
were outside F.4d's list of fourteen, but they *do* print fixed-width numeric
matrices. Both now build rows through the same seam, and with them **every**
`Show` report the port emits is row data — `rg "format::(pad|pad_dots|g_w|
fixed_w|fixed_w_int|g_left_w)" crates/dss-core/src/report/show` is down to
**15** sites, none of them a column: nine section-header literals, three
single-value footer lines (`Total Circuit Losses = …` ×2, `Max Error = …`), and
the two padded one-off labels Pascal writes outside any table (`Show busflow`'s
`ELEMENT = <padded name>` line, `Show Losses`' newline-less
`Percent Losses for Circuit = ` arm).

`Show Yprim` was the trivial half (each triangle is one run of
`Cell::right(g(v, 10), 13)` with a space gutter). `Show Y` is the interesting
one, and it is F.4e's **rule 1** in its purest form: Pascal writes
`Format('[%4d,%4d] = %13.10g + j%13.10g')`, so the brackets *and* the `j` sit
flush against right-justified numbers. Given a `[` or a `j` column of its own,
the table kernel would emit `[ 1 , 1 ] = … j -4.63` where the oracle has
`[   1,   1]` and `j -4.636999108` — three extra tokens per row. So the bracket
pair is one cell (`format!("[{:>4},{:>4}]", …)`) and the susceptance cell bakes
its `j` and the Pascal width into its own text; both render identically in the
two kernels, which is exactly what the golden comparator requires.

**Proof.** The same throwaway probe over all 72 `show_*` fixtures: parity lane
**byte-identical, 86/86** against `HEAD~1`'s dump; default lane **token-identical,
86/86** against F.4e's (so the one enumerated `TERMINAL TOTAL` rule is still the
only default-lane token move in the whole family). No golden, tolerance, ledger
entry or deck regenerated; `SPLIT_ALIAS_POPULATION` stays 37 and `TODO(compat)`
15 in `crates` / 18 tree-wide.

`cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D
warnings` and the `--features dss-core/oracle-parity` twin both exit 0;
`cargo test --workspace --no-fail-fast` **2475 passed / 0 failed / 5 ignored**
and the parity-lane twin identically **2475 / 0 / 5** (F.4e's count — this
commit adds no test), the 520-case corpus gate green inside each,
`git status --short tests/corpus` empty afterwards (the known intermittent
`Test/AutoTrans/*` artifacts deleted by exact name).

### DE_PASCALIZE Stage F.4e — the nine escaped `Show` modules join the table seam; one glued token was Pascal's, not ours (branch `depas-stagef`, 2026-07-31)

F.4d converted 5 of the 14 `Pad`-using `Show` modules and handed on the other
nine ("the recipe is mechanical and proven"). This commit executes it: `buses`,
`meters` (+`generators`), `elements`, `fault_study`, `diagnostics`
(`Mismatch` + `Convergence`), `voltages` (all three forms and the two per-bus
blocks `busflow` shares), `currents` (seq + terminal), `powers` (seq + element)
and `bus_powers` (both forms) — the **47** `Pad`/`PadDots` sites — now build
[`report::table::Row`]s and let `compat::render_rows` lay them out. §F-FMT step 2
is complete: no `Show` report measures its own data columns any more.

**Proof, in both directions.** A throwaway probe replayed all **72** `show_*`
fixtures and copied every file each one produced (**86**, `Elements` writes two)
on `HEAD` and on this tree. Parity lane: **byte-identical, 86/86** — the
refactor moved no glyph in the lane that must never re-baseline. Default lane:
the token streams the golden comparator sees are identical for **85** files and
differ on exactly **one row**, below. No golden was generated; the ruling that
F.4 opens no re-baseline event holds.

**The one row, and why it is a lane row rather than a bug.**
`show_powers_elem_autotrans` line 9 reads `TERMINAL TOTAL-25808.0`. Pascal writes
that label as `PadDots('   TERMINAL TOTAL', MaxBusNameLength + 10)`
(`ShowResults.pas:1230`); `PadDots` pads with a *leading space* then dots, so a
padded label always separates — but this deck's `MaxBusNameLength` is 7, the
field is exactly 17, the pad inserts **nothing**, and the following `%8.1f`
(`-25808.0`, exactly 8 characters) lands flush. The table kernel gives the number
its own column. Handled exactly like F.4b's `Show BusFlow` glue: an enumerated
default-lane expectation on the *oracle* text (`terminal_total_expected`, narrow
to the literal `TERMINAL TOTAL` immediately followed by a digit or `-`), with
`terminal_total_glue_transform_is_the_power_column` proving it is non-vacuous
(the committed golden really carries three such rows), row-count-preserving,
whitespace-only, and the identity in the parity lane.

**Two structural rules the conversion had to obey** (both are token-stream
correctness, not taste, and both are now written at their call sites):

1. **A literal glued to a right-justified number stays inside one cell.**
   `Show Voltages`' element form writes `'  (%3d) %4d … (%8.4g) /_'`: the
   parity lane's `( 12)` is *two* fields to the comparator, and a `(` cell of its
   own would make it three. So those cells carry `format!("({:>3})", …)` — the
   Pascal width baked into the content, identical bytes, identical tokens in both
   kernels. The same rule governs `Show Y`'s `[row,col]` (untouched here).
2. **A label whose field spans two data columns carries an explicit empty cell**
   for the column it swallows (`   TERMINAL TOTAL` over bus+node in `Show
   Powers`/`busflow`). A width-0 empty cell contributes nothing in the parity
   kernel and keeps the four power columns under their headings in the table one.

**Headers: decomposed where they *are* the columns, free text where they are
not.** `Show Currents`' nine seq columns, `Show Faults`' three node groups,
`Show Mismatch`, `Show Meters`/`Generators` and `Show Elements` map one label per
column and became header rows. `Show Voltages`' `Mag:` label, the `(Real)`/
`(Imag)` pair over an `= re +j im` group, and `Show Buses`' two-line `Coord`
banner span several data columns and have no such decomposition — they stay
free text, which keeps their bytes and their tokens exactly as Pascal wrote them.
The determination is recorded in `report/show/mod.rs`; re-laying them out is
plan §F-FMT **step 4** (the v2 decision), not this step.

One helper moved: `format::fpc_sci_body` is `fpc_sci_w` without the
right-justification, so the convergence report's `Str(v:14)` cells carry the
number and the *lane* fills the field (`fpc_sci_w` is now that body in a
width-wide field, byte for byte). New pin
`exec::tests::compat_quirks::show_voltage_table_layout_is_the_lane_kernel`
asserts at a real `Show Voltages` report that the bus column is `PadDots`ed in
one lane and content-sized in the other while every row keeps its twelve fields.
`SPLIT_ALIAS_POPULATION` stays **37** (no new alias — this is the same
`compat::render_rows` seam), `TODO(compat)` stays 15 in `crates` / 18 tree-wide,
and no golden, tolerance, ledger entry or deck was regenerated.

**ESCAPE — `Show Y` and `Show Yprim`.** They use no `Pad`, so they were outside
F.4d's list, but they do print fixed-width numeric matrices (`%13.10g`,
`[%4d,%4d]`). Unaffected (the seam is additive); the recipe is rule 1 above for
the `[row,col]`/`j<value>` glue plus a plain right cell per value.

**Proof.** `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity` both exit 0; `cargo test --workspace --no-fail-fast`
**2475 passed / 0 failed / 5 ignored** and the parity-lane twin identically
**2475 / 0 / 5** (+2 over F.4d: the glue-transform proof and the voltage lane
pin), with the unconditional 520-case corpus gate green inside each. `git status --short tests/corpus` empty
after each run (the known intermittent artifacts deleted by exact name);
`git diff -- tests/golden tests/corpus` empty.

### DE_PASCALIZE Stage F.4d — the table crate lands: `Show` rows become data, and the escape's blocker dissolves (branch `depas-stagef`, 2026-07-29)

F.4b/c handed on §F-FMT step 2's **table-crate rendering** as an escape, with one
entry condition: a byte gate for the `Show` family, which no lane has (the
oracle's own name-column padding is the nondeterministic `max_bus_name_length`
quirk the port does not reproduce, so `run_feeder_show` token-compares in *both*
lanes). That reading is what this commit falsifies. The refactor needs the
before/after bytes of **our** renderer, not a committed artifact: a throwaway
probe dumped all **72** `show_*` fixtures on `HEAD` and again on this tree in the
parity lane, and every one is **byte-identical**. No golden was generated, so the
coordinator's "F.4 opens no re-baseline event" ruling holds exactly as written —
the proof was a diff, not a file.

**The model.** `report/table.rs` holds the column arithmetic Pascal spread across
`ShowResults.pas`: a `Cell` is one field (text, Pascal width, which padding
primitive fills it, and the literal that follows it), a `Row` is a line, a
`Report` interleaves free text with runs of rows. Two kernels, both always
compiled, `compat::render_rows` selecting: parity replays `Pad`/`PadDots`/
`Format('%W…')` in order; default hands the run to **comfy-table 7.2.2**
(`default-features = false`, `NOTHING` preset, `ContentArrangement::Disabled` so
a wide cell widens its column instead of wrapping a row onto two lines) and lets
each column size itself. The crate choice is the plan's own criterion, re-measured
here: comfy-table's whole closure — itself, `unicode-width`,
`unicode-segmentation` — is `forbid`/`deny(unsafe_code)`, `tabled` 0.21's
mandatory `papergrid` is not.

**Why the table kernel cannot silently drop a field.** The default lane compares
these reports parsed-numeric against the same oracle captures, so a merged or
lost column fails — but only on a report some fixture covers. `Cell::sep`
closes it structurally instead: a separator may contain **only whitespace and
commas**, i.e. exactly the characters the comparator splits on, so every token
lives in a cell and the gutters the table kernel substitutes carry nothing.
A separator that tried to smuggle content (`' kW'`, which is why `Show Losses`'
unit is a cell) panics at the call site. The single shape in which the two
kernels' token streams *can* differ is a text overflowing its Pascal width with
an empty separator — the `Show BusFlow` glue F.4b already enumerated — and
`table::tests::overflow_glue_is_the_only_token_difference` pins that it is the
only one.

**Converted this commit (5 of the 14 `Pad`-using `Show` modules):** `Losses`,
`Unserved`, `Taps`, `Overloads`, `DeltaV` — including their **header** literals,
decomposed into the columns they draw (the offsets are in each call site's
comment), so the default lane's headers sit over their own data instead of at
Pascal's hand-counted positions. Two details the conversion had to get right and
that the byte diff caught the shape of: a comma written *after* a padded field
(`'%s,  %4d'`) belongs to the separator, while `Pad('Element,', …)` pads the
comma itself; and `Show Losses`' final `Percent Losses` label is emitted by
Pascal **without** a trailing newline when the load power is zero, so that arm
leaves the row model.

`SPLIT_ALIAS_POPULATION` 36 → **37**, pinned by
`exec::tests::compat_quirks::show_table_layout_is_the_lane_kernel`, which asserts
at a report the executive really produced. Its first claim is content-independent
on purpose: it *reconstructs* the aggregate line from Pascal's own formula
(`Pad(label,30) + Format('%10.1f') + ' kW'`) around whatever number the solve
produced and requires equality in the parity lane and inequality in the default
one — a column index would have pinned this fixture's values instead of the rule.
The other two: the default lane pads two differently-long quoted names into one
column (the parity lane's `Pad(name, 0 + 2)` cannot), and both lanes carry the
same four fields per row. No
`TODO(compat)` marker moved (the F-FMT bucket was already empty), and no golden,
tolerance, ledger entry or deck was regenerated.

**ESCAPE — nine `Show` modules still write their own widths.** `bus_powers`,
`buses`, `currents`, `diagnostics`, `elements`, `fault_study`, `meters`,
`powers`, `voltages` (47 `Pad`/`PadDots` sites) keep the hand-built `String`.
They are unaffected — the seam is additive — and the recipe is now mechanical and
proven: express the line as cells, keep every literal that is not whitespace or a
comma in a cell of its own, and re-run the byte probe. What is *not* mechanical,
and is why they are handed on rather than rushed: `powers`/`currents`/`voltages`
carry per-element blocks whose column count varies with the terminal and phase
count, so their runs have to be cut where the Pascal sections are, and
`fault_study`/`diagnostics` mix matrix dumps into the same file.

**Proof.** `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity` both exit 0; `cargo test --workspace --no-fail-fast`
**2473 passed / 0 failed / 5 ignored** and the parity-lane twin identically
**2473 / 0 / 5** (+9 over F.4c: the eight `report::table` kernel tests and the
lane pin), with the unconditional 520-case corpus gate green inside each. The
runs leaked the known intermittent artifact set (`Test/AutoTrans/*` and
`Examples/StoCtrl_Current_PeakShave/ckt7*`), deleted by exact name;
`git status --short tests/corpus` empty after each. `git diff -- tests/golden
tests/corpus` empty: no artifact moved.

### DE_PASCALIZE Stage F.4b/c — the `Show` column that was measured to zero, the `Save`→Y guard, and the one table-crate step that is escaped (branch `depas-stagef`, 2026-07-29)

F.4a left one `Ffmt` escape row and two of F-FMT's four sub-steps. This commit
closes the row and the `Save` step, and records — with its measurement — why the
table-crate rendering of `Show` tables is **not** landed here. `TODO(compat)` in
`crates` drops **16 → 15** (19 → 18 tree-wide): the `Ffmt` bucket is now **empty**
and the `Escape::Ffmt` variant is deleted, leaving `UpgradeRung` 11 + `WholeCase`
4 + `WasmGuest` 3 = 18. `SPLIT_ALIAS_POPULATION` 35 → **36**. No golden,
tolerance, ledger entry or deck regenerated.

**F.4b — `compat::max_device_name_length` (§F-FMT step 2's `Show`-layout row).**
Pascal `SetMaxDeviceNameLength` computes the longest `Class.Name` in the circuit;
the pinned 0.14.5 backend returns **0** whatever the names are, so every
`Show Currents`/`Powers`/`Losses`/`Elements`/`BusFlow`/… device-name column
collapses. The honest computation now runs in **both** lanes
(`show::device_name_width`) and the lane decides whether the column *uses* it.
The observable is `Show BusFlow`, whose power rows are
`Pad(EncloseQuotes(FullName), width + 2) + IntToStr(term)`
(`ShowResults.pas:1375`): `IntToStr` carries no width, so at width 0 the terminal
number is glued to the closing quote and at the honest width it is its own
column. The three `show_busflow*` goldens are compared through one enumerated
default-lane rule — a closing `"` **immediately** followed by an ASCII digit gets
a space — proven non-vacuous, row-count-preserving and whitespace-only by
`busflow_glue_transform_is_the_terminal_column`. A detail worth recording because
it shaped the rule: `width` counts the *unquoted* name, so `width + 2` is exactly
the longest quoted name's length and the **longest** element still glues in both
lanes; the split is per-row, not per-column.

**F.4c — `Save` → `Compile` → same checkpoint Y (§F-FMT's sequencing guard).**
`save_roundtrip.rs`'s seven feeder round trips now also capture the assembled
system Y through `Dss::system_y_csc`, keyed by `(row node name, col node name)`
with duplicate stamps summed, and compare it entry for entry. This is strictly
stronger than the node-voltage compare beside it — a wrong impedance the power
flow happens to absorb still fails here — and it is precisely what a rendering
change could break, since every number in the emitted script is printed through
the F-FMT seam. The floor was **measured, then set**: worst relative entry
difference 2.875e-15 on IEEE-8500 (46 259 entries) and ≤ 1.9e-16 on the other six,
so `Y_TOL = 1e-14` (≈3.5× headroom); each run re-prints its own figure rather than
leaving the number in a comment.

**ESCAPE — §F-FMT step 2's *table crate* is not landed, and the reason is a
missing gate, not effort.** The crate choice was made by measurement and is
recorded for whoever picks it up: **comfy-table 7.2.1 with
`default-features = false`** — its whole closure (comfy-table, `unicode-width`,
`unicode-segmentation`) is `forbid`/`deny(unsafe_code)`, while `tabled 0.21`'s
mandatory `papergrid 0.18` carries three real `unsafe` blocks, so the plan's own
criterion ("`forbid(unsafe_code)`-clean dependency tree") selects comfy-table and
rejects tabled. What blocks the conversion is this: **no numeric `Show` report has
a byte gate in either lane.** `run_feeder_show`/`run_deck_show` call
`compare_export` (token-level) in *both* lanes, and byte comparison is impossible
there because the oracle's own padding is the nondeterministic
`max_bus_name_length` quirk the port deliberately does not reproduce
(`show::max_bus_name_length`). Rewriting 23 report modules' width arithmetic into
a row/cell model with only a whitespace-tokenizing comparator watching would let a
parity-lane byte regression land unnoticed — the one thing Stage F must never do.
Making it safe needs a parity-lane byte **self**-golden for the `Show` family
first, which is a golden-generation event the coordinator's F.3 exit ruling does
not grant F.4. Handed on with that as its entry condition; nothing in the tree
depends on it, and the marker it would have carried is already resolved.

**Proof.** `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity` both exit 0; `cargo test --workspace --no-fail-fast`
**2464 passed / 0 failed / 5 ignored** and the parity-lane twin identically
**2464 / 0 / 5** (+2 over F.4a: the device-name pin and the busflow transform's
non-vacuity test), with the unconditional 520-case corpus gate green inside each.
Both runs leaked the known intermittent `Test/AutoTrans/*` set, deleted by exact
name; `git status --short tests/corpus` empty after each. `git diff --
tests/golden tests/corpus` empty: no artifact moved.

### DE_PASCALIZE Stage F.4a — F-FMT step 1: the number-rendering seam, and what a re-rounded last digit actually costs (branch `depas-stagef`, 2026-07-29)

The first of F.4's three steps (`DE_PASCALIZE_PLAN.md` Part IV.2 §F-FMT): **every
number-to-text call in the engine now goes through a `compat` alias**, and the
five aliases that carry a genuine difference are flipped. `TODO(compat)` in
`crates` drops **22 → 16** (25 → 19 tree-wide) — six of the seven `Ffmt` escape
rows resolved; the survivor is the `Show` *table-layout* row, which is step 2's,
not a number format at all. `SPLIT_ALIAS_POPULATION` 30 → **35**. No golden,
tolerance, ledger entry or deck was regenerated: F.4 opens no re-baseline event,
per the coordinator's F.3 exit ruling and §F-FMT step 3.

**The five rows, each with its measurement.**

* `compat::fmt_g` — `util::fmt_g_fpc_impl` (the FPC 3.2.2 Grisu1 + `ffGeneral`
  pipeline) vs `fmt_g_native_impl` (~40 lines: one correctly-rounded
  `format!("{:.*e}")`, then FPC's own notation window and trailing-zero rules).
  The native kernel deliberately keeps the *digit count* and the
  fixed-vs-scientific threshold (`-6 < exp < digits`, one decade wider than C's
  `%g`) — those are the layout policy the fixed-width tables are built around,
  not an inexactness. What it drops is the one thing IV.2 calls a wart: FPC's
  **two-stage** decimal re-rounding. Measured over the committed 13 198-value FPC
  battery at precisions 2/5/8/15 (52 792 renders): **225 divergences**, worst
  9.86e-15 relative, every one a last-digit difference, notation never moved, and
  the default render at most **1 character longer** (a round-up into a trailing
  zero that FPC then strips — `0.4020128841512195`).
* `compat::fixed_w_script` — the AltDSS `%8.2f` PostCommands: FPC's 15-sig
  intermediate + ties-away vs one correct rounding (`0.125` → `0.13` vs `0.12`).
* `compat::CONTROL_QUEUE_SEC_DIGITS` — `Show ControlQueue`'s `Sec` column. Its
  `%-.g` precision is *unobservable* (the queue is always drained before any
  text-interface `show controlqueue`), so the parity lane keeps the 6-significant
  stand-in the byte contract was written against and the default lane stops
  guessing: 15, FPC's documented `ffGeneral` default.
* `compat::json_float` — fpjson's fixed 17-significant scientific vs the shortest
  round-tripping literal. **A `.0` suffix is part of the fix**: `format!("{}")`
  renders `1.0` as `1`, which this module's own reader then takes for an
  *integer* (and `-0.0` loses its sign) — caught by
  `read::tests::round_trips_writer_output`, which is exactly the regression a
  "shortest is shortest" flip would have shipped.
* `compat::JSON_LINE_BREAK` — fpjson writes the RTL platform `sLineBreak`, so the
  goldens carry Windows CRLF; the default lane writes `\n` on every platform.

**What the flips cost the gate, measured rather than assumed.** Exactly **five
cells** in the whole suite, each a last-digit re-spelling of the *same* `f64`,
each enumerated as a fail-on-stale expected-value transform of the oracle
capture — never as a widened tolerance, because `exact_value_policy` is `rel =
abs = 0` on purpose and a band would also absorb a real change:
`dump3_bare`/`dump3_debug`'s `Mean=0.825828333333334` (the `loadshape.default`
FMean the parity kernel's own doc names), `dump_autotrans`'s
`Rdcohms=0.008430938` (winding 2, printed at `g(v, 7)`), and — the only corpus
movement in 520 cases — two `QoutPU=` cells of
`controls:invcontrol/midi_invcontrol_drc.dss`'s DRC trace, printed at
`fmt_g(v, 3)` where FPC's `AGRESSIVE_ROUNDUP` turns `…4999` into a round-up. All
four are *display* values; the DRC trigger itself compares `f64`s.

**The JSON goldens changed comparator, not content.** They used to be byte-equal
in both lanes; `harness::lane::compare_json` now lexes both sides and compares
token-for-token — structure and key order exactly, every **string** verbatim
(so the `PostCommands` DSS script stays byte-gated), numbers by `f64::to_bits`
with no tolerance at all. `golden_schema.rs` took the other route: its 23
`\r\n`-anchored divergence patterns make it a *layout* gate, so it now renders
through the new `write_pretty_with(FPJSON_SPELLING, …)` in both lanes and a new
`lane_spelling_renders_the_same_schema_document` holds the default writer to the
same document through `compare_json`. `Dss::schema_document()` was split out of
`extract_schema_json()` to make that possible.

**Both kernels stay compiled and asserted, in either build.**
`fmt_battery_matches_fpc_rtl` now pins `fmt_g_fpc_impl` (not "whatever the seam
selects") against the real FPC RTL on all 13 198 × 7 renders *in both lanes* —
strictly stronger than before — and
`native_kernel_differs_from_fpc_only_by_the_rounding_rule` records the divergence
population as a number that has to be re-measured, not silenced.

**Proof.** `cargo fmt --all --check` clean; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity` both exit 0; `cargo test --workspace --no-fail-fast`
**2462 passed / 0 failed / 5 ignored** and the parity-lane twin identically
**2462 / 0 / 5**, with the unconditional 520-case corpus gate green inside each.
That is +101 over F.3's 2361, all of it the two test binaries that gained the
harness (`golden_json`, `golden_json_import`) plus the new lane pins; 0 removed,
0 new `#[ignore]`. The default-lane run leaked the known intermittent
`Test/AutoTrans/*` set and the parity-lane run a different one; both deleted by
exact name, after which `git status --short tests/corpus` is empty. `git diff --
tests/golden tests/corpus` is empty: no artifact moved.

### DE_PASCALIZE Stage F.3 — CLOSED: the coordinator's exit ruling, and the one hand-off it moves (branch `depas-stagef`, 2026-07-29)

F.3ag ended with the 0.15.x hide-flag escape parked on **F.5**, having disproved
F.4 as its host but with the choice between "Stage F's landing generation" and
"`UPGRADE_PLAN` §5's engine-switch re-pin" still open — F.3ag itself named §5 the
*preferred* end state and handed F.5 the row only because F.5 is the one
generation event Stage F sanctions. The coordinator settled it: **§5**, and with
it the whole step. This commit writes that ruling into the tree and re-verifies
the exit against it. `TODO(compat)` stays **22** in `crates` (25 tree-wide),
`HIDE_015X` **19**, `SPLIT_ALIAS_POPULATION` **30**; no engine code, test, golden,
tolerance, ledger entry or deck moved — the diff is one flag's doc.

**The exit criterion, as ruled.** The plan's literal "0 markers" is unreachable
inside F.3's sanctioned scope — F.3ag proved that with the two blocked classes
(whole-case default-lane oracle exclusion; UPGRADE-rung oracle re-baseline),
neither of which any Stage F step may authorize. The **escape register is the
accepted exit**: (a) every sanctioned flip done, each with an expected-value pin;
(b) the one default-lane self-golden re-baseline done and measurably empty;
(c) every surviving marker either plan-placed in F.4 or a recorded escape pinned
by a gate test — zero unclassified, and no hide-flag carrier outside the pinned
13-artifact row. All three hold at `fe66821b`, and each is enforced by a test
rather than asserted here: `surviving_compat_markers_are_exactly_the_recorded_
escape_register` (25 = `UpgradeRung` 11 + `Ffmt` 7 + `WholeCase` 4 + `WasmGuest`
3, no more and no fewer), `every_lane_split_alias_is_pinned_by_an_expected_value_
test` (30 split aliases, 2 declared-not-wired), `hide_015x_carrier_set_is_the_
measured_escape` + `the_hide_flag_escape_population_is_pinned_by_surface` (5
carriers, 13 artifacts), `oracle_parity_cfg_appears_only_in_compat_modules_and_
tests`, `compat_tag_is_only_ever_a_marker_never_prose`, and
`operational_docs_cite_the_compat_machinery_accurately`. The re-baseline is still
empty by measurement: `git diff 6f691ecb..HEAD -- tests/golden tests/corpus` shows
only F.3af's manifest `note`.

**Why the host moves off F.5.** Both candidate hosts were live because both can
produce the 13 artifacts; they differ in what the tree looks like afterwards. The
landing host generates *default-lane* self-goldens for them, i.e. it buys the zero
carrier count by exposing the five props in one lane only — a permanent fork of a
surface with **no numeric content**, paid for with 13 lane-specific artifacts that
nothing but their own generator would ever read again. §5's re-pin teaches
`gen_json.py` the engine switch and retires the flag in **both** lanes, moving the
parity goldens with it; that is the only form in which 0.14.5-pinned byte goldens
may legitimately change, since the parity lane may never re-baseline. The row is
therefore not deferred *inside* Stage F at all: **F.4 and F.5 both leave it
alone.** The flag's doc now says so, in place of F.3ag's "F.5, whose executor has
to be told the row exists".

**Nothing else in the tree pointed at F.5 for this.** Checked, not assumed: the
two remaining `F.5` mentions under `crates/` (`compat.rs`'s alias-label note,
`golden_reports.rs`'s layout note) are about the differential job, and
`docs/upgrade/DIVERGENCES.md` §"Line/LineGeometry Conductors" never named a Stage
F step — it records the deliberate retention plus `UPGRADE_PLAN` §1.4's
"disproportionate → keep the flag and document why", which is the same disposition
this ruling confirms. The surface pin and the carrier pin are untouched: their
subject is the measurement's population, not its host.

**Proof.** Both lanes green on the exact committed tree: `cargo fmt --all
--check`; `cargo clippy --workspace --all-targets -- -D warnings` and the same
with `--features dss-core/oracle-parity` both exit 0; `cargo test --workspace
--no-fail-fast` **2361 passed / 0 failed / 5 ignored** and the parity-lane twin
identically **2361 / 0 / 5**, with the unconditional 520-case corpus gate green
inside each. Tests unchanged (0 added, 0 removed, 0 new `#[ignore]`). The 5 ignored
were re-enumerated rather than copied from the previous record, which described
them as "four manual generation binaries plus one doctest": they are **three**
manual generation binaries (`dss-epri`'s `gen_wasm_usermodels{,_wm4,_wm5}`, each
needing an env-gated twin DLL), the `ckt24_graph_diagnostic` inventory report in
`adiakoptics.rs`, and the ```ignore` doctest on `obj/props/mod.rs`'s
`define_properties`. The default-lane run leaked the known intermittent
`Test/AutoTrans/auto3bus_{load_power,lt_current,lt_losses}.txt` trio, deleted by
exact name; the parity-lane run leaked nothing. `git status --short tests/corpus`
empty after both.

**Handoff.** F.4 (F-FMT) owns the 7 `Ffmt` markers and nothing else from this
list; F.5 owns lanes + differential gate + docs and **no golden event**;
`UPGRADE_PLAN` §5 owns the hide-flag row (flag + `Wires`→`Conductors` masquerade,
one atomic change, 13 artifacts, both lanes); the 4 `WholeCase` rows need a
default-lane oracle-exclusion policy no Stage F step may grant; the 3 `WasmGuest`
rows need a second `.wasm` fixture (`WASM_USERMODELS_PLAN`).

### DE_PASCALIZE Stage F.3ag — the last escape was handed to a step that never opens the event it needs (branch `depas-stagef`, 2026-07-29)

F.3af closed the loop on documents that cite the compat machinery. This commit
closes it on the one hand-off nobody had re-read: **where the 0.15.x hide-flag
escape actually lands.** F.3aa deferred it to **F.4** on the premise that
"F-FMT re-layouts the same Dump/Show surface", so the 13 self-goldens it needs
would ride an event F.4 was opening anyway. Read at the step it names, that
premise is false — and it is the *only* thing holding the deferral, because
F.3aa's own conclusion is worded as "not a **second** re-baseline in F.3",
which presupposes a first one. `TODO(compat)` stays **22** in `crates` (25
tree-wide), `HIDE_015X` stays **19** (the correction deliberately never spells
the flag — see F.3aa on why that count must not be inflated by prose about it);
no engine code, golden, tolerance, ledger entry or deck moved.

**Inherited state re-verified before building on it.** F.3af's tree
(`0b599d77`) was re-gated from scratch in both lanes before any edit: `cargo fmt
--all --check` clean, `cargo clippy --workspace --all-targets -- -D warnings`
and the same with `--features dss-core/oracle-parity` both exit 0, `cargo test
--workspace --no-fail-fast` **2360 passed / 0 failed / 5 ignored** and the
parity-lane twin identically **2360 / 0 / 5**, with the unconditional 520-case
corpus gate green inside each. The two counts were re-measured, not read: `rg`
gives 22 and 19. The default-lane run leaked nothing; the parity-lane run leaked
the known intermittent `Test/AutoTrans/*` trio, deleted by exact name, after
which `git status --short` is empty.

**The premise, checked at `DE_PASCALIZE_PLAN.md` §F-FMT rather than assumed.**
Step 2 (line 1258) re-layouts **`Show`-style reports only** — "`Show`-style
reports assemble rows as data". Step 3 (line 1268) states the *opposite* of a
re-baseline for everything else: the default lane compares the **same committed
goldens** through the parsed-numeric tokenizer, "valid as long as F-FMT v1 keeps
row/column structure (it does; only rendering changes)". The one clause that
does force self-goldens — "re-layouted reports get default-lane self-goldens"
(drift model, line 1232) — is reached only by the **optional v2**, "GUI era,
separate decision" (step 4, line 1270). So F-FMT v1 re-baselines nothing.

**And the 13 artifacts are on the wrong surface for that step anyway.** Eight
are `Dump` texts (`tests/golden/reports/dump*.txt`, written by
`report::save::dump`), two are AltDSS-JSON captures and three are schema walks
(`tests/golden/json/*.json`). Not one is a `Show` table — the only surface F.4
re-layouts. F.4 therefore opens no self-golden event these could ride, in v1 by
its own text and in fact by where they live.

**Re-homed, not re-litigated.** The escape itself stands exactly as F.3aa
measured it (no physics moves, 13 artifacts by row insertion, the flip is atomic
with the `Wires`→`Conductors` JSON-key masquerade). Only its *host* changes: the
sole generation event Stage F sanctions is the landing one — "default-build
self-goldens for regression detection only, **regenerated once at Stage F
landing**" (line 1292) — i.e. **F.5**, whose brief scope ("lanes + differential
gate + docs") does not imply a golden event, so its executor has to be told the
row exists. The preferred end state is unchanged and still better than either:
UPGRADE §5's engine-switch re-pin retires the flag in *both* lanes, where a
default-lane-only exposure buys a zero carrier count at the price of a permanent
lane difference in a surface with no numeric content.

**The correction is pinned, because the disproof is a claim about the tree.**
The argument turns on which surfaces those 13 sit on, so
`oracle_parity_cfg_gate::the_hide_flag_escape_population_is_pinned_by_surface`
asserts it: all 13 exist by name under their surface directory, and every text
artifact is a `Dump`. This is the half the carrier-set pin never covered —
`compat_quirks::hide_015x_carrier_set_is_the_measured_escape` pins the *five
carriers*, i.e. one of the measurement's two populations; the 13 gated artifacts
were prose until now. Both probes run and reverted: renaming `dump_line_geo.txt`
in the list → "gated artifact(s) … no longer exist"; substituting the real
`show_buses.txt` → "is not a `Dump` golden … reopens whether F.4 hosts this
row".

**Proof.** Both lanes green on the exact committed tree: `cargo fmt --all
--check`; `cargo clippy --workspace --all-targets -- -D warnings` and the same
with `--features dss-core/oracle-parity` both exit 0; `cargo test --workspace
--no-fail-fast` **2361 passed / 0 failed / 5 ignored** and the parity-lane twin
identically **2361 / 0 / 5**, with the unconditional 520-case corpus gate green
inside each. Tests **+1**, 0 removed, 0 new `#[ignore]` (the 5 are the four
manual generation binaries plus one `define_properties` doctest). `git status
--short tests/corpus` empty after every run, run-artifacts deleted by exact name.

### DE_PASCALIZE Stage F.3af — the stage's own flips falsified five documents, and no gate could see it (branch `depas-stagef`, 2026-07-29)

F.3ae wrote this stage's deviations back to the plan that ordered the work. The
same failure had a second and larger instance that nobody had gone looking for:
F.3 resolved ~100 markers and flipped 30 aliases, and **every document outside
`crates/` that cited one of those sites by name was silently falsified**. Five
were. Nothing objected, because every Stage F gate — the cfg gate, the tag
spelling gate, the escape register, the pin rule — walks `.rs` files only. That
is exactly F.3ac's finding (the instrument scoped narrower than the claim it
serves) in the last place it could still hide. `TODO(compat)` stays **22** in
`crates` (25 tree-wide), `HIDE_015X` **19**; no engine code, golden, tolerance,
ledger entry or deck moved.

**Inherited state re-verified before building on it.** F.3ae's tree
(`02793a79`) was re-gated from scratch in both lanes before any edit: `cargo fmt
--all --check` clean, `cargo clippy --workspace --all-targets -- -D warnings`
and the same with `--features dss-core/oracle-parity` both exit 0, `cargo test
--workspace --no-fail-fast` **2359 passed / 0 failed / 5 ignored** and the
parity-lane twin identically **2359 / 0 / 5**, with the unconditional 520-case
corpus gate green inside each. `git status --short tests/corpus` empty after
both. The two counts were re-measured, not read: `rg` gives 22 and 19.

**The five, each checked at the site rather than assumed.**

| document | what it still claimed | what F.3 had made true |
|---|---|---|
| `tests/TOLERANCE_NOTES.md` | a marker in `seq_currents.rs`, and "the clean fix **is** the per-terminal slice" | F.3c *took* that fix: `compat::IRESIDUAL_FROM_TERMINAL_1` |
| `tests/TOLERANCE_NOTES.md` | a marker at `obj/props/class_props/value.rs` "reproduces by rendering a deterministic zero matrix" | `compat::SYM_MATRIX_GETTER_RENDERS_ZEROS`; only the parity lane renders zeros |
| `tests/corpus/modes/manifest.json` | a marker in `exec/view.rs` is "what actually verifies Newton dispatch is wired" | F.3j lane-split that read; the default lane drops this deck's element powers entirely |
| `tools/golden/gen_props.py` | "see the port's marker" for the Isource `Bus1`/`Bus2` ordering | `compat::ISOURCE_BUS2_NEVER_LATCHES`; the ordering is the *parity* lane's requirement |
| `tools/golden/gen_json.py` | the `CktModel=` empty-value quirk is a marker | `compat::CKT_MODEL_RENDERED_ORDINAL`; the capture's value-less line is now the parity answer |

Plus `gen_plot_callback.py`'s unrecognized-`type=` note, stale the other way:
that arm was resolved earlier in this stage to IV.1 **permanent semantics in
both lanes** (`exec/plot.rs`, `PlotOptions.pas:305`'s empty `else`), so it is
not a compat row at all and the golden pins the same bytes either way.

**The manifest one is the load-bearing member of that list.** It sits inside a
**gated artifact** and told whoever triages `newton.dss` that the deck's element
powers are the signal proving Newton dispatch. In the default lane they are not
compared at all: `elem_channels_for` drops that case (and `newton_feeder.dss`)
to `CURRENTS_ONLY` via `LANE_SKIP_ELEM_POWERS`, and the verification moved to
the in-engine tripwire
`exec::tests::newton::newton_dispatch_leaves_a_valid_but_stale_iterminal_cache`
plus the pin `newton_powers_are_the_lane_kernel`. A triager following the old
note in the default lane would have been hunting a channel that is switched off.

**The correction is not "delete the tag".** Each sentence now names the lane row
and says which lane answers what, because that is the fact a reader needs: the
tolerance note explains that the default lane excludes only the `Terminal >= 2`
cells (`GateSpec::ColAbove(1, 1.5)`) and keeps the column's `abs = 1e-8` — an
exclusion, never a loosened band; `gen_json.py` now says to keep capturing the
oracle's value-less line because the golden stays byte-compared in *both* lanes
with one enumerated rewrite (`golden_json.rs::lane_expected_json`) rather than a
re-baseline; `gen_props.py` says to keep the `Bus2`-after-`Bus1` order because
the capture comes from the engine the parity lane reproduces. The
`TOLERANCE_NOTES.md` section that described the marker convention as "removed in
one pass after final acceptance" now states Stage F's actual outcome — a lane
split, with the parity lane's floors unmoved and the default lane excluding
fields rather than widening bands.

**The gate.** `oracle_parity_cfg_gate::operational_docs_cite_the_compat_
machinery_accurately` checks the surface the other four gates cannot see, in
two independent directions. (1) A doc line that spells the tag **and** names a
Rust file is a claim about where a marker lives: it must carry a row in
`TAG_PATH_CITATIONS` saying `Present` or `Absent`, and the marker index must
agree. (2) Every `compat::<ident>` a doc names must be declared by a compat
module — inside `crates/` rustdoc links are compiler-checked, but Markdown,
Python and JSON get no such help, so a rename leaves a lying sentence. Half (2)
is populated by the corrections above, so it is load-bearing on landing rather
than a promise.

**Scoped by what a document is *for*, not by where it sits.** The surface is
`CLAUDE.md`, `TESTING.md`, `tests/TOLERANCE_NOTES.md`, the corpus manifests and
`ledger.json`, and the `tools/` generators and READMEs — files that describe the
tree *as it is*. Plans and records are deliberately excluded: `DE_PASCALIZE_
PLAN.md`, `PORTING_PLAN.md`, `STATUS.md` and `docs/**` state intent or history
and are *allowed* to differ from HEAD; mechanising that would burden every
historical record and invite editing the log to please a test. The one plan row
whose content this stage disproved was corrected by hand in F.3ae, which is the
right shape for that class.

**Two under-scopings of the new instrument, both caught by its own probes
before the commit.** (a) The first cut of `rust_paths_in` required a `/`, so it
did **not** match the very citation it was written for — the real staleness
spelled the file bare (`seq_currents.rs`). Probe 1 failed to fire, which is how
it was found; the helper now accepts full, partial and bare spellings, resolved
against the marker index by suffix. (b) The first walk pulled 33 files of the
**vendored** `electricdss-tst` checkout into "our documentation" — upstream's
own `.py`/`.md`, which make no claim about this tree and would fail the suite on
a routine re-vendor. The walk is now per-subtree (`tests/corpus` contributes
only the manifests we author), and the exclusion is asserted, so widening it
back fails by name.

**Five probes, each run and reverted.** (1) Re-adding the stale bare-name
citation to `TOLERANCE_NOTES.md` → `names ["seq_currents.rs"]`, unregistered.
(2) Flipping the ledger row to `Present` → "claims Present of
`crates/dss-core/src/exec/view.rs`, tree says Absent". (3) Renaming a cited
alias to `compat::IRESIDUAL_FROM_TERMINAL_9` → named as undeclared. (4) Widening
the corpus filter to `.py` → "the doc walk descended into the vendored corpus
(30 files…)". (5) Pointing a `Present` row at a marker-free file → the same
inverted-claim failure as (2). Every arm of the gate is load-bearing.

**One documentation gap closed as a side effect.** The wasm reference model's
README listed its three surviving markers but never said which files they live
in — the only reader-facing pointer at `Escape::WasmGuest`, and unanchored. It
now names `models/indmach012a/src/{symcomp,model}.rs` and records why they are
escapes (workspace-excluded crate: a lane split there is a second `.wasm`
fixture, not a `cfg` alias), and those two claims are the gate's `Present` rows.

**What this does not change.** The F.3 verdict stands: 25 markers survive, each
with a measured blocker owned by a named successor (`UpgradeRung` 11, `Ffmt` 7,
`WholeCase` 4, `WasmGuest` 3); `HIDE_015X` keeps its F.3aa disposition; the
sanctioned re-baseline is still measurably empty (`git diff 6f691ecb..HEAD --
tests/golden tests/corpus` shows only this commit's one manifest `note`) and
still handed to Stage F landing. No engine behaviour moved in either lane.

**Proof.** Both lanes green on the exact committed tree: `cargo fmt --all
--check`; `cargo clippy --workspace --all-targets -- -D warnings` and the same
with `--features dss-core/oracle-parity` both exit 0; `cargo test --workspace
--no-fail-fast` **2360 passed / 0 failed / 5 ignored** and the parity-lane twin
identically **2360 / 0 / 5**, with the unconditional 520-case corpus gate green
inside each. Tests **+1**, 0 removed,
0 new `#[ignore]` (the 5 are the four manual generation binaries plus one
`define_properties` doctest). Both runs leaked the known intermittent
`Test/AutoTrans/*` artifacts (17 files); deleted by exact name, after which
`git status --short tests/corpus` shows only this commit's `modes/manifest.json`
edit.

### DE_PASCALIZE Stage F.3ae — the plan's own dual-kernel table still specified three kernels this stage disproved (branch `depas-stagef`, 2026-07-29)

F.3ab→F.3ad made the stage's findings executable *in the tree*: the escape
register, the tag-spelling gate, the pin rule. The one place they were never
written back is the document that ordered the work. `DE_PASCALIZE_PLAN.md` IV.2's
dual-kernel table is the closed inventory a Stage F executor is told to follow —
and for **three of its eleven rows** it has specified, since F.3e/F.3i measured
otherwise, a default kernel the code deliberately does not use. A reader of the
spec alone would conclude the product lane divides with `num_complex` `/` and
inverts with partial pivoting, and that the parity lane uses
`SymComp::official`. All three are false, all three on purpose. This commit
records the deviations at the spec. `TODO(compat)` stays **22** in `crates` (25
tree-wide), `HIDE_015X` **19**; no code, golden, tolerance, ledger or deck moved
— the diff is two Markdown files.

**Inherited state re-verified before building on it.** F.3ad's tree
(`1d2d4fd2`) was re-gated from scratch in both lanes before any edit: `cargo fmt
--all --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` and
the same with `--features dss-core/oracle-parity` both exit 0, `cargo test
--workspace --no-fail-fast` **2359 passed / 0 failed / 5 ignored** and the
parity-lane twin identically **2359 / 0 / 5**, with the unconditional 520-case
corpus gate green inside each. `git status --short tests/corpus` empty after
both. The counts F.3ad states were re-measured rather than read: `rg` gives 22
and 19 exactly.

**Why a spec that contradicts the code is not a cosmetic problem here.** Every
other Stage F verdict is anchored twice — a measurement at the site and a gate
that fails if it rots. These three were anchored only in `compat.rs`'s module
doc, i.e. in the answer, never in the question. The next two steps are F.4 and
F.5, whose executors are pointed at IV.2's table by the brief; the closed-
inventory rule ("do not invent new compat items") makes that table the authority
on what may exist, so a stale row there is exactly the input that produces a
re-litigation — or worse, a "fix" restoring a kernel this stage rejected on
measurement.

**The three, each with its disproof at the spec.** `complex division` — FPC
`ucomplex`'s `/` *is* Smith's algorithm, so the proposed `num_complex` default is
the worse and less robust kernel (4.26e-16 vs 3.82e-16 worst relative error;
`0`/`NaN` outside `|den| ∈ [1e-154, 1e154]`). `dense inverse` — the partial-pivot
candidate is 1 ULP from the parity kernel on an ideal switch and *closer* to the
exact reciprocal, yet that ULP is worth 1.45 kW on `Auto1bus-step1.dss` against a
floor calibrated from a bit-identical Y; it also disagrees on what a singular
matrix leaves behind, which three call sites consume. `sym components` — the
proposed **parity** kernel is unreachable in the pinned oracle at all:
`mathutil.pas:548` closes initialization with `SelectAs2pVersion(False)` and the
truncated pair sits behind `DSSCompatFlag.BadPrecision` (`CAPI_DSS.pas:315`),
which no gating oracle sets. Both Pascal citations were re-read in the vendored
source for this commit rather than copied from `compat.rs`.

**Scope, deliberately.** Only the rows whose *content* the stage disproved are
touched. The plan's status markers ("Stage F not started"), the `HIDE_015X` exit
criterion that F.3aa proved unreachable as worded, and the `ORPHANED_GAPS.md` row
are **F.5's** listed scope and stay untouched here — this is the deviation
record F.3 owes, not an early landing.

**Proof.** Both lanes green on the exact committed tree: `cargo fmt --all
--check`; `cargo clippy --workspace --all-targets -- -D warnings` and the same
with `--features dss-core/oracle-parity` both exit 0; `cargo test --workspace
--no-fail-fast` **2359 passed / 0 failed / 5 ignored** and the parity-lane twin
identically **2359 / 0 / 5**, with the unconditional 520-case corpus gate green
inside each. Tests ±0, 0 removed, 0 new `#[ignore]` (the 5 are the four manual
generation binaries plus one `define_properties` doctest). `git status --short
tests/corpus` empty after every run.

### DE_PASCALIZE Stage F.3ad — the *flipped* half of the register becomes a gate: every deliberate divergence has to name its pin (branch `depas-stagef`, 2026-07-29)

F.3ab made the stage's **escapes** executable and F.3ac widened their walk. The
rows F.3 actually *flipped* — the larger half — stayed prose. They are the part
the oracle gates structurally cannot cover: **30** `compat::` aliases now resolve
to different code in the two lanes, and every one of them makes the default lane
answer something both gating oracles do not, which is why `DE_PASCALIZE_PLAN.md`
IV.2's drift model ends its last row with "pinned by their own **expected-value
tests**". That those tests still exist was, until this commit, a claim a reader
had to take on trust — the same state F.3ab judged unacceptable for the escapes,
and for the same reason. `TODO(compat)` stays **22** in `crates` (25 tree-wide),
`HIDE_015X` **19**; no golden, tolerance, ledger or deck moved and no engine
behaviour changed. The diff is one gate and one doc line.

**Inherited state re-verified before building on it.** F.3ac's tree
(`ec154028`) was re-gated from scratch in both lanes before any edit: `cargo fmt
--all --check` clean, `cargo clippy --workspace --all-targets -- -D warnings`
and the same with `--features dss-core/oracle-parity` both exit 0, `cargo test
--workspace --no-fail-fast` **2358 passed / 0 failed / 5 ignored** and the
parity-lane twin identically 2358/0/5, with the unconditional 520-case corpus
gate green inside each (40/40 in 134.2 s and 135.0 s). `git status --short
tests/corpus` empty after both runs — nothing leaked.

**The measurement, and the single row that failed it.** Of the 30 split
aliases, **29** already had a test that names the row it pins — in an assertion
(`crate::compat::LINECODE_SYM_CLEAR_OMITS_C0`) or in the doc comment that
declares what it is pinning (`[`crate::compat::SYM_MATRIX_GETTER_RENDERS_ZEROS`]`).
The exception was `round_i32`, whose observable pin
(`dss-parser` `parser::tests::make_integer_out_of_range_is_the_lane_kernel`,
`inf → 0` vs `i32::MAX`) called itself "the Stage F round row" in words only, so
nothing connected it to the alias. One doc link closes that, and with the naming
uniform the obligation becomes checkable rather than narratable.

**The instrument was under-scoped a second time — now pinned so it cannot
narrow again.** The first scan written for this reported `kv_base_search_scale`
as unpinned. It is not: its pin is an **inline** `#[cfg(test)] mod tests` inside
`solution/solution/dispatch.rs`, and the scan looked only at files *called*
tests. That is F.3ac's finding in a new place — the instrument, not the tree, was
wrong — so `is_pin_candidate` accepts all three shapes the tree actually uses
(a `crates/*/tests/**` integration test, an extracted sibling `tests.rs`, an
inline `#[cfg(test)]` module), and the gate anchors one alias in each of the two
non-obvious shapes: narrowing the walk back makes it fail by name.

**What it checks, and what it deliberately does not.** For every split alias:
a test **names** it, and that test **branches on the lane** — reads
`ORACLE_PARITY`, or reads `compat::<alias>` in code. Kernel-vs-kernel tests
inside the compat modules explicitly do *not* count: they assert the two impls
against each other, which is a different obligation (IV.2 "Mechanism") from
pinning what a deck observes. It does not check what a pin *asserts* — that is
review, and mechanizing it would only produce a test that can be satisfied
without being true. The two things it does check are exactly the two that rot
silently: a deleted pin, and a new row flipped without one.

**The two declaration-only knobs are named rather than skipped.**
`dss-sparse`'s `PARALLEL_FACTORIZATION` and `ITERATIVE_REFINEMENT` select the
same impl in both lanes today (F.1 staging; M3c and WP-R1 own the flip), so
there is no observable to pin. Listing them by name instead of excusing them by
count turns that handoff into a gate: the moment their owner makes the arms
differ, the row joins the split population and the pin rule starts applying —
which is the commit where the expected-value test has to be written.

**Three probes, each run and reverted.** (1) Restoring `round_i32`'s prose-only
doc makes the gate fail with `no expected-value pin: ["round_i32"]`. (2)
Narrowing `is_pin_candidate` to files *named* tests makes it fail with
`["kv_base_search_scale"]` — the exact under-scoping above, now self-detecting.
(3) Pointing the default-lane arm of `PARALLEL_FACTORIZATION` at its
`_DEFAULT_IMPL` (i.e. wiring the knob) makes the declared-set assertion report
the moved set and demand a pin. The gate is load-bearing in all three
directions, not decoration.

**What this does not change.** The F.3 verdict is untouched: 25 markers survive,
each with a measured blocker owned by a named successor (`UpgradeRung` 11,
`Ffmt` 7, `WholeCase` 4, `WasmGuest` 3); `HIDE_015X` keeps its F.3aa
disposition; the sanctioned re-baseline is still measurably empty
(`git diff 6f691ecb..HEAD -- tests/golden tests/corpus` remains empty) and still
handed to Stage F landing.

**Proof.** Both lanes green on the exact committed tree: `cargo fmt --all
--check`; `cargo clippy --workspace --all-targets -- -D warnings` and the same
with `--features dss-core/oracle-parity` both exit 0; `cargo test --workspace
--no-fail-fast` **2359 passed / 0 failed / 5 ignored** and the parity-lane twin
identically **2359 / 0 / 5**, with the unconditional 520-case corpus gate green
inside each (40/40 in 134.6 s and 134.5 s). Tests **+1**, 0 removed, 0 new
`#[ignore]` (the 5 are the four manual generation binaries plus one
`define_properties` doctest). The parity run leaked the known intermittent
`Test/AutoTrans/{Auto3bus_noload_power,auto3bus_hl_current,auto3bus_hl_losses,
auto3bus_ht_current}.txt`; deleted by exact name, `git status --short
tests/corpus` empty.

### DE_PASCALIZE Stage F.3ac — the sweep's own instrument was under-scoped: three compat markers were hiding outside `crates/` (branch `depas-stagef`, 2026-07-29)

F.3ab closed with a register that checks the surviving markers **both ways** and
a population of 22. Both halves of that claim were true only of the subtree the
walk looked at. `rust_sources` — the walk every Stage F gate is built on —
enumerated `crates/*/{src,tests,benches,examples}` and nothing else, because
Stage F's exit criterion is written as `rg "TODO\(compat\)" crates`. `CLAUDE.md`
is not: it says the markers "are **all** absorbed in one dedicated pass". Those
two scopes differ by a real crate, and the difference was hiding three markers
that name Stage F as their owner in their own text ("deferred to the
DE_PASCALIZE Stage F sweep"). The population is **22 → 25**; no engine code, no
golden, tolerance, ledger or deck moved, and both lanes stay green.

**Where they were.** `tools/wasm_usermodel/models/indmach012a` — the WM.2
reference user model — is a hand-ported Rust crate, not tooling scaffolding. It
is **workspace-excluded on purpose**: its `Cargo.toml` carries an empty
`[workspace]` table so the fixture toolchain stays pinned separately from the
product gate (`WASM_USERMODELS_PLAN.md` §2.6). That is exactly what made it
invisible — it is Rust the port owns, in a directory the port's own tag index
never read. The same blind spot also swallowed a **prose** mention of the tag
(`model.rs:513`, "NOT a `TODO(compat)` — …"), the precise thing F.3g's spelling
gate exists to reject; widening the walk makes it fail, so it is reworded here.

**Inherited state re-verified before building on it.** F.3ab's tree
(`64bc94f2`) was re-gated from scratch in both lanes before any edit: `cargo fmt
--all --check` clean, `cargo clippy --workspace --all-targets -- -D warnings`
and the same with `--features dss-core/oracle-parity` both exit 0, `cargo test
--workspace --no-fail-fast` and the parity-lane twin both exit 0 with the
unconditional 520-case corpus gate inside each. The parity run leaked the known
intermittent `Test/AutoTrans/Auto3bus_*` artifacts; deleted by exact name,
`git status --short tests/corpus` empty.

**Why all three escape — measured per row, not argued as a category.** The
fixture crate carries its own host-target twin tests, so each constant's flip
was run *in isolation* against the native FPC twin's bit-exact generated pins
(`tests/twin_expected.rs`, "GENERATED … DO NOT EDIT"), 2026-07-29:

| row | flip | first pin it breaks | relative |
|---|---|---|---|
| `symcomp.rs` | `0.866025403` → exact `sin 120°` | `pf_i_1_0` −1436.051530295505 vs −1436.0515303730524 | 5.40e-11 |
| `model.rs` `Compute_dSdP` | `1.732` → `sqrt(3)` | `vars_initial_10` (`Ir1`) 1723.6616943288936 vs 1723.7122572967085 | **2.93e-5** |
| `model.rs` var 14 | f32-folded `3.0/746.0` → f64 | `vars_initial_14` (`HPshaft`) −1848.1305807400754 vs −1848.1305331200504 | 2.58e-8 |

They are the same truncated-constant family as the engine-side `UpgradeRung`
rows, but with a **structurally stronger** blocker, which is why they get their
own owner (`Escape::WasmGuest`) rather than joining that bucket: the Stage F
mechanism does not exist here. `dss-core/oracle-parity` cannot reach a crate
that is not in the workspace, and the gated artifact is ONE committed binary
(`tests/fixtures/wasm/indmach012a.wasm`, regenerated manually under
`tools/wasm_usermodel/PIN.txt`). A lane split is therefore not a `cfg` alias but
a *second `.wasm` fixture* — a WASM_USERMODELS_PLAN decision, not an executor's.
The third row is also the one place where the truncation is not a wart at all:
FPC folds `3.0/746.0` at SINGLE precision, so the f32 quotient *is* the twin's
arithmetic.

**The instrument, fixed.** `rust_sources` now walks the whole repository with a
named skip list. `.claude` is on it for a specific reason: it holds the
parallel-agent worktrees, which are complete copies of this repository, so a
naive widening would report every marker two or three times and fail the gate on
a clean checkout. The widening is pinned against being tidied back:
`rust_sources` asserts it reaches both `crates/dss-core/src/compat.rs` **and**
`tools/wasm_usermodel/models/indmach012a/src/model.rs`. The walk stays
`.rs`-only, which is the right boundary and was checked rather than assumed:
the tag's remaining tree-wide hits are cross-references *about* markers in
Markdown and in the golden-generator Python (`tools/golden/gen_*.py`,
`tools/fpc/fmt_battery/gen_values.py`, `docs/wasm/USERMODEL_ABI.md`) — capture-
side notes, not ported reproduction sites. `tools/wasm_usermodel/README.md:56`
already listed these three constants, which is corroboration that they were
known and simply had nowhere to be counted.

**Non-vacuity, probed rather than asserted.** Restoring the prose mention makes
`compat_tag_is_only_ever_a_marker_never_prose` fail *naming the `tools/` path*;
re-keying one `WasmGuest` row makes the register report the orphaned marker,
also by its `tools/` path. Both probes were run and reverted — the widening is
load-bearing in both directions, not decoration.

**What this does not change.** The F.3 verdict stands: every marker whose clean
fix fits the IV.2 drift model is flipped, and each of the 25 survivors carries a
measured blocker owned by a named successor (`UpgradeRung` 11, `Ffmt` 7,
`WholeCase` 4, `WasmGuest` 3). The re-baseline is still empty and still handed
to Stage F landing (`git diff 6f691ecb..HEAD -- tests/golden tests/corpus`
remains empty), and `HIDE_015X` keeps its F.3aa disposition.

**Proof.** Both lanes green on the exact committed tree: `cargo fmt --all
--check`; `cargo clippy --workspace --all-targets -- -D warnings` and the same
with `--features dss-core/oracle-parity`; `cargo test --workspace
--no-fail-fast` **2358 passed / 0 failed / 5 ignored** and the parity-lane twin
identically **2358 / 0 / 5**, with the unconditional 520-case corpus gate green
inside each. Tests ±0, 0 removed, 0 new `#[ignore]` (the 5 are the four manual
generation binaries plus one `define_properties` doctest). `git status --short
tests/corpus` empty after every run.

### DE_PASCALIZE Stage F.3ab — the escape register becomes a gate, and the missing re-baseline turns out to be empty (branch `depas-stagef`, 2026-07-28)

F.3 drove the compat-marker population **123 → 22** (`rg "TODO\(compat\)" crates`,
counted at `6f691ecb` and at `HEAD`) across 27 commits, and every one of the 22
survivors carries a measured blocker at its site. What it did *not*
have was a way to notice when that stops being true. The register lived in this
file as prose, so a successor could add a 23rd marker, or quietly close one and
decrement the count, and nothing would object — the tag gate
(`compat_tag_is_only_ever_a_marker_never_prose`, F.3g) polices the *spelling* of
markers and asserts only that the population is `> 0`. This commit turns the
register into a test. No engine change: `TODO(compat)` stays **22**, `HIDE_015X`
**19**, both lanes green, no golden, tolerance, ledger or deck touched.

**Inherited state re-verified before building on it.** F.3aa's tree
(`79fe0cc2`) was re-gated from scratch in both lanes: `cargo fmt --all --check`
clean, `cargo clippy --workspace --all-targets -- -D warnings` and the same with
`--features dss-core/oracle-parity` both exit 0, `cargo test --workspace
--no-fail-fast` exit 0 and the parity lane **2357 passed / 0 failed / 5
ignored**, the unconditional 520-case corpus gate green inside each. The 5
ignored are the four pre-existing manual-generation binaries plus one
`define_properties` doctest — no new `#[ignore]` anywhere on the branch.

**The finding that closes F.3's last open sub-item: the sanctioned re-baseline
has no content yet.** F.3's brief lists "regenerate the default-lane
self-goldens ONCE" among its own work, and this session set out to do it. It is
a no-op, and that is measurable rather than arguable: `git diff 6f691ecb..HEAD
-- tests/golden tests/corpus` is **empty** — the entire 28-commit flip series
landed without moving a single golden byte, because every deliberate divergence
so far was absorbed by a *field-scoped* default-lane exclusion (`GateSpec::Mask`)
plus an expected-value pin, exactly as the IV.2 drift model prescribes. There is
nothing to re-baseline: no artifact is failing in the default lane. The plan text
agrees on where it belongs — IV.2 says the self-goldens are "regenerated once at
Stage F **landing**", and the two things that will actually make default-lane
output *structurally* differ (F-FMT's re-layout, and the 13 artifacts the 0.15.x
hide flag would grow by row insertion, F.3aa) both land later. Doing it now would
create a duplicate golden tree and force a *second* re-baseline at F.4 — which is
precisely what "the single re-baseline event" forbids. F.3 therefore closes with
the re-baseline handed forward, and with the reason recorded as a diff rather
than as a judgement.

**The register, executable.**
`oracle_parity_cfg_gate::surviving_compat_markers_are_exactly_the_recorded_escape_register`
carries all 22 survivors as `(file, distinctive slice of the marker's own text,
owner)` and checks the population **both ways** — an unregistered marker fails,
and a registered row that no longer matches exactly one marker fails. That is the
`tests/corpus/ledger.json` fail-on-stale discipline applied to the same kind of
object, and for the same reason: a list of known divergences that nothing
re-reads decays into folklore. It makes two plan rules executable that were
previously habits — "the dual-kernel inventory is **closed**, do not invent new
compat items" (a new marker now cannot appear without naming its owner and its
measured cost), and "Stage F's exit criterion is a count of this tag" (closing a
row now means deleting it and decrementing `EXIT_POPULATION`, a deliberate act
instead of a silent decrement). Each row carries the number that stopped it, so
the successor argues with measurements.

| owner | rows | what has to happen first |
|---|---|---|
| `Escape::UpgradeRung` | **11** | a re-baseline against a different oracle — `mu0`/`Twopi`, `CALPHA` ×2, `0.3183`, the truncated pi/rad-to-deg pair ×2, Kxg ×3, `2.3026` ×2 |
| `Escape::Ffmt` | **7** | F.4 (`F-FMT`): the `compat::fmt` seam and the table-layout step |
| `Escape::WholeCase` | **4** | a whole-case default-lane oracle exclusion, which the drift model does not grant the executor — GICTransformer `%R2`, Capacitor `Cuf`, the LoadShape MMF accept-set, the Generator Model=6 seed |

**Non-vacuity, probed rather than asserted.** Re-keying one row to a constant
that does not appear in the tree (`LPFTau * 9.9999`) makes the test name the
orphaned marker; adding a row for a marker that does not exist makes it name the
stale row and tell the reader to decrement the bucket. Both probes were run and
reverted; neither branch is reachable by accident.

**And one row's classification is now sourced, not inherited.** F.3x escaped the
Kxg trio on the reading that "both gating oracles keep 658.5 in `Line` while
r4133's `LineConstants` moved on", i.e. that upstream holds two values of one
constant. The pinned 0.14.5 oracle is in fact **self-consistent** at 658.5 —
`General/LineConstants.pas:424` *and* `PDElements/Line.pas:520/704/959` — so the
split engine is ours: WP-U1.2 B2/D1 adopted r4133's corrected
`658.8530451057239` for `LineConstants` alone. Finishing that job in `Line` is
the next step of that rung, not a lane flip, which is what the golden says too
(`harmonics_doall`, `Line.l1 Yprim[0,0]`, 1.732e-6 against an allowed 1.002e-6).
Same verdict, now anchored in the vendored source.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate in each.
Tests **+1**, 0 removed, 0 new `#[ignore]`. `git diff -- tests/golden
tests/corpus` empty; `git status --short tests/corpus` empty after every run.

### DE_PASCALIZE Stage F.3aa — the `HIDE_015X` waiver stops being the one argued escape: measured, and the naive flip is *wrong*, not merely stale (branch `depas-stagef`, 2026-07-28)

Every other Stage F escape on this branch carries its own number. `HIDE_015X`
— the one exit item that is not a compat marker (`UPGRADE_PLAN` §5: "`rg
HIDE_015X` must be empty", handed to Stage F) — did not: F.3's escape register
argued it from the *shape* of the change ("a byte-golden re-baseline against a
different oracle, which the parity lane may never do") and stopped there. This
commit runs the flip and gates it. The escape stands, but for a sharper reason
and with a bill attached, and one of the three findings contradicts the naive
form of the fix outright. `TODO(compat)` stays **22**; `rg HIDE_015X crates`
goes **17 → 19** — the two new matches are the pin's own `PropFlags::HIDE_015X`
argument and its rustdoc link, i.e. *uses*, and the count is held there
deliberately (see the last section); both lanes green; no golden, tolerance,
ledger or deck touched, and no engine behaviour changed.

**The flip that was run.** `hidden_from_full_enum()` reduced to `HIDE_R4133`
alone — i.e. all five carriers (`Line.EpsRMedium`/`HeightOffset`/`HeightUnit`/
`Conductors`, `LineGeometry.Conductors`) exposed on the full-enumeration
surfaces in **both** lanes — then `cargo test --workspace`, whole suite, no
filters. Reverted before the commit; the committed diff is documentation and
two pins.

**1. No physics moves — this is a surface-structure row, not a numeric one.**
The unconditional 520-case corpus gate is *completely unchanged*: **40/40 in
136.9 s**, as are all 1355 lib unit tests and every checkpoint/feeder/control
binary. That is a materially different picture from the truncated-constant
bucket next door, where a flip costs 35 corpus cases; nothing about this row
touches a solve.

**2. Exactly 13 byte goldens move, every one of them by row insertion.**
`golden_reports` ×8 — `dump_line_geo`/`dump_line_lc`/`dump_line_sym`/
`dump_line_switch` **+4 rows each** (44 vs 40), `dump_linegeometry` **+1** (33
vs 32), `dump3_bare` **+4** (769 vs 765), `dump3_debug` **+4** (1300 vs 1296),
`dump3_commands` **+5** (2333 vs 2328); `golden_json` ×2 (`json_line_micro`,
`json_circuit_micro`); `golden_schema` ×3 — two of them
(`ported_class_defs_bytes_match_oracle`, `full_document_reconciles_with_oracle`)
on `schema_divergences.json`'s `port_hidden_property` rows going stale (they
name `Line.EpsRMedium` first), the third on the port-golden document drift. +4
per Line and +1 per LineGeometry is exactly the carrier count, so the blast
radius is closed: nothing outside those five props is disturbed.

**3. And the finding that changes the shape of the fix: the naive flip emits a
duplicate JSON key.** Line declares **two** properties that render the JSON key
`Conductors` — the legacy `Wires` array carrying `json_name = "Conductors"` (the
masquerade that has owned the key since wt-u14props) and the real 0.15.x
`Conductors` proxy. With only `HIDE_015X` dropped, the FULL view of `Line.l1`
contains `"Conductors":[]` **twice** in one object, once after `Spacing` and
once after `HeightUnit` — observed bytes, not a prediction. So the flag and the
masquerade are *one atomic change*, which is precisely how `UPGRADE_PLAN` §5
words the exit item ("the masquerade drops, and the real `Conductors` prop owns
the JSON key") — a sequencing detail the earlier escape record did not carry,
and the difference between "these goldens are stale" and "this output is
wrong".

**Why it still escapes — now a question of *where*, not *whether*.** The 13
artifacts are 0.14.5-oracle byte goldens; the parity lane may never re-baseline
them, and re-pinning them means teaching `gen_json.py` an engine switch — the
UPGRADE rung §5 describes and `DIVERGENCES.md` §"Line/LineGeometry Conductors"
already measured as disproportionate for this tail. Exposing them in the
**default lane only** is a real option, and cheap now that the cost is known
(13 self-goldens, zero corpus movement) — but it needs default-lane self-goldens
for those 13, i.e. the plan's **single** sanctioned re-baseline, which is
already sequenced at **F.4** (F-FMT re-layouts the same Dump/Show surface). F.3
must not open a second re-baseline event, so the row is handed forward with its
numbers rather than flipped here.

**And the criterion as worded is unreachable anyway — proven by its own
sibling.** `HIDE_R4133` has had **zero carriers** since WP-U2.5 and still leaves
**7** `rg` matches: a flag's definition, its arm in `hidden_from_full_enum`, and
the comments naming it are not uses. Retiring the five 0.15.x carriers leaves
the same residue — of the 17 matches at HEAD, only **5** are carriers and one is
the predicate; the other **11** are the definition and comments *about* the flag
(the earlier record's "six" undercounted). "`rg` empty" therefore means *delete
the mechanism*, which `HIDE_R4133`'s own doc explicitly declines ("retained … as
the mechanism a future r4133-only prop re-uses"). The successor should restate
the criterion as **zero carriers** — the form the new pin checks — or delete both
flags together.

**A note on the number, because this commit is a live demonstration of the
problem.** Writing the measurement up in full, naming the flag naturally
throughout, took `rg HIDE_015X crates` from 17 to **39**: documenting an escape
made the metric that tracks it three-quarters worse, while the substance (five
carriers) did not move at all. That is the same failure `CLAUDE.md` already
solved for the compat tag — "it must stay greppable", enforced by
`compat_tag_is_only_ever_a_marker_never_prose`. So the same discipline is
applied here by hand: the new prose says "the flag" / "the 0.15.x hide flag",
and the name is spelled only where it is a real code reference. Final count
**19**, +2 for the pin's flag argument and its doc link. Gaming would be
choosing words to move a number; this is the opposite — refusing to let prose
count as a use, so the number keeps meaning what the plan intended.

**Two pins, so the measurement cannot rot.** Both run in **both** lanes (the row
is reproduced in both):

* `hide_015x_carrier_set_is_the_measured_escape` — the flag's carrier set read
  off the live class table and asserted to be exactly those five, because a
  blast-radius measurement is only worth the population it was taken over; plus
  `HIDE_R4133` asserted carrier-free, which is both what makes
  `hidden_from_full_enum` a synonym for the 0.15.x flag today (the premise of
  the measurement) and the empirical proof of the paragraph above.
* `line_json_conductors_key_is_owned_by_the_masquerade` — the collision
  precondition stated executably: Line must declare exactly two props rendering
  the key `Conductors`, `("Wires", visible)` and `("Conductors", hidden)`, and
  the FULL view must carry the key exactly **once**. It also pins the flag's
  cost in the *other* gated branch — a `Line` edited with `epsrmedium=2.5` loses
  that value from the set-order JSON view (so a JSON round-trip drops it) while
  `? Line.l1.EpsRMedium` still answers `2.5`. That asymmetry is the
  product-visible price of keeping the waiver, and it is the argument for the
  rung actually being scheduled.

**Escape register — unchanged in content, one entry upgraded from argued to
measured.** The **11** truncated physical constants (F.3x/z, each with its own
number at its own site), the **7** F-FMT rendering markers (F.4's defined
scope), the **4** single-site quirks (F.3v, all whole-artifact exclusions), and
the 0.15.x hide flag (**5 carriers**) — now carrying the 13-artifact /
0-corpus-case bill and the duplicate-key finding above. 11 + 7 + 4 = the 22
remaining markers. `rg "TODO\(compat\)" crates` = **22**,
`rg HIDE_015X crates` = **19** (5 carriers + 1 predicate + 13 references).

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate in each.
Tests **+2** (`hide_015x_carrier_set_is_the_measured_escape`,
`line_json_conductors_key_is_owned_by_the_masquerade`), 0 removed, 0 new
`#[ignore]`. `git diff -- tests/` empty — no golden, tolerance, ledger or deck
touched; `git status --short tests/corpus` empty after every run (the known
intermittent `Test/AutoTrans/*` leak deleted by exact name). The probe edit to
`hidden_from_full_enum` was reverted in full before the gate.

### DE_PASCALIZE Stage F.3z — the truncated-constant bucket is fully measured: one row splits, and the other four now carry their own numbers (branch `depas-stagef`, 2026-07-28)

F.3v filed **14** truncated physical constants under one blanket escape reason;
F.3w split one of them, F.3x measured two more, and F.3y removed the bucket's
kernel row. This commit finishes the audit on **everything that was left** — the
four remaining rows are each flipped, gated and measured, and one of them turns
out to be the narrowest lane row in the whole stage. **No category reason
survives anywhere in the bucket.** `TODO(compat)` **23 → 22**; `HIDE_015X`
**17**; both lanes green; no golden, tolerance, ledger or deck touched.

**The row that splits — `Export Profile`'s line-to-line per-unit divisor.**
Upstream divides the L-L volt magnitude by the four-digit literal `1732.0`
(`Common/ExportResults.pas:3207/3231/3256`; r4133
`Version8/Source/CMD_Lazz/Common/ExportResults.pas:2979/2995/3012`, so **both**
gating oracles carry it). `Bus.kVBase` is the **line-to-neutral** base kV, so the
L-L divisor should be `1000·√3 = 1732.0508…` and the literal makes every reported
L-L per-unit 2.93e-5 relative high.

The single-site membership rule (ii) is satisfied *inside the same procedure*, by
the sibling branch — F.3w's shape again, one level up: `ExportProfile`'s
**line-to-neutral** arms divide by the exact `1000.0` (eight sites) while its
three **line-to-line** arms divide by `1732.0`. One routine, one quantity, one
branch exact and the other truncated to four digits. Nothing else differs, so the
constant the author meant is named by the code beside it.

**And the reach is one number in one report** — measured, not asserted:
`pu_ll` is local to `report::export::profile` and feeds only the `puV1`/`puV2`
column of the three L-L variants. With the flip selected, the *only* moving
assertion in the entire suite is `export_profile_ll3ph` row 0 field 2 (`1.04672`
vs the oracle's `1.04675`), and **all 520 gated corpus cases are unchanged**.
That is the drift model's "deliberate divergences … excluded from oracle
comparison **at those fields**" in its purest form, so the default lane masks
exactly columns 2 and 4 of those three goldens (`lane::profile_ll_policy`, using
the `GateSpec::Mask` mechanism the `Iresidual` row already established — an
exclusion, never a widened tolerance). Still gated in the default lane: the
header, the row set and order, the element name, both distances, `Color`,
`Thickness`, `Linetype`, the marker fields, and **in full** the four L-N variants
of the same report.

**The replacement pin states the physics, not a captured number.** On a balanced
three-phase bus `|V_LL| = √3·|V_LN|` exactly, so
`pu_LL = √3·|V_LN| / (kVBase·1000·√3) = pu_LN`: the two reports must print the
*same* per-unit for the same bus.
`exec::tests::compat_quirks::export_profile_ll_pu_is_the_lane_kernel` builds a
transposed, balanced feeder, runs both exports, and asserts that identity in the
default lane and the exact divisor ratio `1000·√3/1732 = 1.0000293346` in the
parity lane — then asserts the *other* lane's value is distinguishable at the
report's own 6-digit resolution, so the test cannot pass in both builds and pin
nothing. An engine that swapped one wrong constant for another fails it.

**The four rows that do not split, each with the number that stopped it.**

| row | measured cost of the flip |
|---|---|
| `mu0 = 12.56637e-7` (`line_constants/mod.rs`) — 4.889e-8 below `4πe-7`, scales the whole series `Z` | **35 of 520** corpus cases + 33 unit tests + `dump_line_geo` + `show_lineconstants` |
| `Twopi = 6.283185307` (same block) — 2.858e-11 below `TAU`, one consumer (`LFactor`) | **1 of 520**: `4Bus-YYD/YYD-Master-step1.DSS` on the r4133 channel, entry 12, \|diff\| 1.380e-4 vs an allowed 1.338e-4 = **1.031x** its floor — the same deck F.3f already found on its calibration boundary |
| `CALPHA = (-0.5, -0.866025)` (`util.rs` + the `Reactor` twin) — imag part 4.37e-7 short of `−sin 120°` | **33 of 520** + `dump_reactor_symcomp` |
| `0.3183` as `1/π` (`line_constants/cable.rs`) — 3.10e-5 low, multiplies the tape-shield resistance | **8 of 520** (every tape-shield deck) + 8 unit tests + `line_constants_scenarios` |
| `TRUNCATED_PI` + `TRUNCATED_RAD_TO_DEG` (`complexutil`) | see below — **16 of 520** + 5 `wasm_*` r4133 goldens + 7 byte goldens |

`mu0` and `Twopi` were split apart deliberately, because the pair had been
escaping as one: they are 1700x apart in relative distance and 35x apart in
blast radius. The physically meaningful group is `mu0/twopi`, which the exact
constants make exactly `2e-7` (both truncated: 1.99999990e-7) — so a *correct*
fix flips both and pays `mu0`'s 35-case bill. `Twopi` alone at 1.03x one floor is
recorded as such rather than as "above the floors".

**The `complexutil` row is the one whose escape reason was actually wrong, and
the correction matters for whoever picks it up.** Upstream names the fix itself
— `// TODO: better precision` sits on both `CDANG` and `PDEGtoCompLeX`, and
`// TODO: remove for 0.13` on the local `PI` — and the distances are tiny
(5.38e-11 and 6.6e-14), far below every calibrated floor. The row still escapes,
for a structural reason no distance argument would have found: **`cdang` and
`pdeg_to_complex` are inverses, and the engine round-trips through them.** A
Spectrum harmonic is typed in degrees, turned into a phasor by the divisor and
read back in degrees by the multiplier; the two truncations cancel *exactly*.
Measured both ways: flipping only the reporting half moves **0** corpus cases yet
breaks that identity (`dump_spectrum` prints `30.0000000016139` where both the
oracle and the truncated engine print `30`), and flipping both — the only
coherent form — makes the engine strictly *more* accurate (`export_seqvoltages`'
`V0` residual falls from 4.31951e-6 to 1.49998e-11) at a cost of 16 corpus cases,
5 `wasm_*` r4133 goldens and 7 byte goldens. Being more correct is precisely what
breaks it, which is why "the constant is close enough" was never the right test.

**Escape register — final for the truncated-constant bucket, and now entirely
numeric.** The **11** remaining truncated-constant markers are: Kxg ×3 and
`2.3026` ×2 (F.3x), `mu0`/`Twopi` ×1, CALPHA ×2, `0.3183` ×1, `complexutil` ×2.
Each carries its own measurement at its own site. Plus the **7** F-FMT rendering
markers (F.4's defined scope), the **4** single-site quirks (F.3v) and
`HIDE_015X` ×17. 11 + 7 + 4 = the 22 remaining markers.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` (2355 passed /
0 failed / 5 ignored) and the same with the feature (2355 / 0 / 5), including
the unconditional 520-case corpus gate in each. Tests **+1**, 0 removed, 0 new
`#[ignore]`. `git diff -- tests/golden tests/corpus` empty — no golden,
tolerance, ledger or deck touched (`tests/` changes are the harness lane policy
only); `git status --short tests/corpus` empty after both runs (the known
intermittent `Test/AutoTrans/*` leak deleted by exact name).

### DE_PASCALIZE Stage F.3y — the "less precise FPC primitives" row: two of the three were already `num_complex`, and the third is the better kernel (branch `depas-stagef`, 2026-07-28)

F.3x closed the audit of the two truncated-constant rows whose escape reason was
still a prediction. This commit takes the bucket's remaining *kernel* row — the
`cabs_fpc`/`csqrt_fpc`/`cln_fpc` trio in `support/line_constants/mod.rs` — and
measures it instead of arguing it. The marker's own claim was that all three
"reproduce FPC's *less precise* forms only to match the oracle bit-for-bit" and
that "the clean fix is to drop all three for `num_complex`'s
`.norm()`/`.sqrt()`/`.ln()`". **Measured against a 60-digit `mpmath` reference,
that claim is false for every one of the three, in two opposite directions**, and
the row resolves with **no lane split at all**. `TODO(compat)` **24 → 23**;
`HIDE_015X` **17**; both lanes green; no golden, tolerance, ledger or deck
touched.

**The measurement** (F.3e's protocol: the shipped kernels dumped as raw f64 bits
from a Rust probe, the error analysis done in `mpmath` at 60 digits, 20 000
operands, deterministic xorshift, |z| sweep 1e-12…1e12 for accuracy and
1e-150…1e150 for the equality claim; probe removed before commit):

| kernel | mean ULP err | worst ULP err | worst rel err |
|---|---|---|---|
| `csqrt` FPC algebraic (NR), real part | **0.339** | **1.82** | **2.45e-16** |
| `csqrt` `num_complex` polar, real part | 2.413 | **12818** | **1.88e-12** |
| `csqrt` FPC vs polar, imag part | 0.339 / 0.472 | 1.73 / 2.44 | 2.38e-16 / 3.51e-16 |
| `cabs` naive vs `f64::hypot` | 0.292 / 0.292 | 1.11 / 1.11 | identical |

**1. `Cabs` and `Cln` were never warts — they were `.norm()` and `.ln()` all
along.** Over the entire range where `re²+im²` is representable, the naive
`√(re²+im²)` is **bit-for-bit** `f64::hypot` on this toolchain: **0
disagreements in 20 000 samples** spanning |z| ∈ 1e-150…1e150. And because
`cln`'s imaginary part is literally `im.atan2(re)` — the same expression
`Complex::arg` evaluates — `cln_fpc` was bit-identical to `.ln()` as a
consequence, with nothing left to check. Where the two forms *do* part is
outside that band, and there the hand-rolled one is simply broken: `re²+im²`
over/underflows, so it returns `inf`/`0` (measured at `(1.5e154, 2.5e154)` and
`(1e-170, 1e-170)`) where `hypot` stays exact. Equal wherever any gate can look,
strictly more robust where none can — so both are now used **unconditionally, in
both lanes**, the two helpers are deleted, and the marker closes without a
`compat` row.

**2. `Csqrt` stays FPC's — and *not* for parity.** `num_complex`'s `.sqrt()` is
the polar form `from_polar(√r, θ/2)`, which routes through `atan2` and `cos`;
as `θ → ±π` (just off the negative real axis) `cos(θ/2)` cancels
catastrophically. That costs it **7× the mean** and **7000× the worst-case**
error of the branch-split Numerical-Recipes form the port already had. Flipping
this row would have made the *product* lane strictly less accurate for nothing —
the identical verdict F.3e reached for `compat::cdiv` (Smith's division), and the
same IV.1 principle: legitimate numerics stay shared by both lanes. So the
plan's expectation was inverted here too, and for the second time in this stage
the "idiomatic" candidate is the worse kernel.

**Why this is a resolution and not an escape.** The row needed lane branching
only under the marker's premise that the parity forms were deliberately
imprecise. With that premise measured false, one arm of the trio is a pure
robustness upgrade that no gate can observe and the other is a kernel the port
should keep on merit — neither needs a lane. That is the same shape as the
"complex division", "dense inverse" and "Y triplet dedup" rows, which is why it
adds no row to IV.2's closed table.

**Both ends are pinned, in both lanes.**
`naive_modulus_equals_hypot_until_the_square_overflows` re-runs a slice of the
equality sweep (4 000 operands over the same 1e-150…1e150 band) with the deleted
naive form kept locally as the reference, asserting bit-equality of the modulus
*and* of both `cln` components — then asserts the extreme-range collapse
(`inf`/`0` vs the exact `hypot` values) that motivates the switch. This matters
for the **parity** lane specifically: the whole justification for letting the
1:1 engine call `.norm()` is that it is bit-identical, so if a future toolchain's
`hypot` ever stopped agreeing, this fails loudly instead of drifting the DERI
goldens in silence. `csqrt_algebraic_beats_the_polar_form` pins the other
verdict with the measured worst-case operand as literals: `csqrt_fpc` returns
the **correctly rounded** result bit-for-bit, `z.sqrt()` is 12 818 ULP away, and
a "modernizing" swap fails on the number that forbids it.
`fpc_complex_primitives_match_ucomplex_not_num_complex` keeps every RTL bit pin
it had, with the `cmod`/`cln` halves now asserted through `.norm()`/`.ln()` —
i.e. the crate reproducing the x86_64 FPC `ucomplex` RTL exactly.

**Escape register.** Unchanged except that the truncated-constant bucket drops
its one *kernel* member: the **12** remaining truncated physical constants
(Kxg ×3 and `2.3026` ×2 measured in F.3x; CALPHA ×2, `1732.0`, `0.3183`,
`mu0`/`Twopi` and the `complexutil` pair still carrying the category reason),
the **7** F-FMT
rendering markers (F.4's defined scope), the **4** single-site quirks (F.3v),
and `HIDE_015X` ×17. 12 + 7 + 4 = the 23 remaining markers.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` (2354 passed /
0 failed / 5 ignored) and the same with the feature (2354 / 0 / 5), including
the unconditional 520-case corpus gate in each. Tests **+2**, 0 removed, 0 new
`#[ignore]`. `git diff -- tests/` empty — no golden, tolerance, ledger or deck
touched; `git status --short tests/corpus` empty after both runs (the known
intermittent `Test/AutoTrans/*` leak deleted by exact name).

### DE_PASCALIZE Stage F.3x — the truncated-constant bucket stops being one blanket claim: two rows measured, one of them from the source (branch `depas-stagef`, 2026-07-28)

F.3w re-opened one member of F.3v's "14 truncated physical constants" escape
bucket and split it, which proved the bucket's blanket reason was doing work it
had not earned. This commit finishes that audit on the two members whose reason
was still a prediction, and files both **with numbers instead of a category**.
No engine behaviour changes and no marker closes: `TODO(compat)` stays **24**
(13 truncated constants + 7 F-FMT + 4 single-site quirks), `HIDE_015X` **17**.

| row | what the audit found | verdict |
|---|---|---|
| Line `Kxg` De = **658.5** (`line/{accessors,code,mod}.rs`, 3 markers) | The *fix* is unambiguous and the row is not an isolated truncation: **the same engine holds two values of one constant**. Both gating oracles keep 658.5 in `Line` (`src/PDElements/Line.pas:520/704/959`; r4133 `Version8/…/Line.pas:410/702/832`) while r4133's own `General/LineConstants.pas:492` already computes Carson's earth-return depth with `658.8530451057239` — the value the port adopted for `LineConstants` in WP-U1.2 B2/D1. What blocks it is cost, and the cost is now **measured**: `kxg` has exactly one consumer (`xgmod = 0.5·kxg·ln(freq_multiplier)` under `xg ≠ 0`), so it is invisible at the base frequency and bites off-nominal. With all three sites flipped, `tests/golden/harmonics/harmonics_doall` fails on `Line.l1 Yprim[0,0]`: actual `(2.007606e-2, −2.487370e-1)` vs oracle `(2.007673e-2, −2.487385e-1)`, **\|diff\| = 1.732e-6 against an allowed 1.002e-6** | an **oracle** golden, compared in **both** lanes at a calibrated floor → re-baseline (UPGRADE rung), not a lane flip. Escaped, with the numbers at the site |
| `2.3026` as ln(10) — `ExpControl` `FOpenTau := Tresponse / 2.3026` and CIM `VV_olrt := LPFTau * 2.3026` (2 markers) | This one fails the single-site membership rule *before* any floor argument. Both oracles carry the literal in both places (dss_capi `ExpControl.pas:392` + `ExportCIMXML.pas:2523`; r4133 `:296`/`:380` + `:2164`), and r4133 states it in the **user-facing property help**: `Tresponse` "corresponds to a low-pass filter having tau = Tresponse / 2.3026" (`ExpControl.pas:184`). Nothing in either source says `LN_10` was meant — it is a *documented model constant*, not a slip | escaped: replacing it is a specified-behaviour change (6.47e-6 on `FOpenTau`, every gated ExpControl trajectory, plus the byte-exact `cim_der{,_DYN}.xml`) |

**Why this is worth a commit that closes no marker.** F.3v's escape register
filed all 14 constants under one reason — "their 5.4e-4-class distance from the
exact constant is above the calibrated oracle floors". That is right for the
constants that form an impedance and wrong for at least one that did not
(F.3w's `0.001732` feeds a discrete argmin, and split cleanly), so the category
could not be trusted as a verdict for the rest either. The two rows above are
the ones whose escape a successor would most plausibly re-litigate — the Kxg
trio because the port itself already uses the corrected constant fifty lines
away, the `2.3026` pair because "truncated ln(10)" reads like an obvious fix.
Both now carry an executable reason at the site.

**Housekeeping.** The Kxg marker's Pascal citation was stale
(`Line.pas:531/741/1077`); it now cites the vendored 0.14.5 lines and their
r4133 twins, which is what a successor greps.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate (2352 passed
/ 0 failed / 5 ignored in each lane; the parity corpus gate ran 520/520 clean,
i.e. the F.3m/F.3p scheduler race did not recur on this tree). Tests ±0, 0
removed, 0 new `#[ignore]`. `git diff -- tests/` empty — no golden, tolerance,
ledger or deck touched; `git status --short tests/corpus` empty after both runs
(the known intermittent `Test/AutoTrans/*` leak deleted by exact name).

### DE_PASCALIZE Stage F.3w — one Pascal statement spells `√3` twice, and only one of the two is truncated (branch `depas-stagef`, 2026-07-28)

F.3v closed the *single-site quirk* sweep at the four rows whose fix costs a
whole artifact. It did not re-open the row this session lands, because F.3v's
escape register filed it under "the 14 truncated physical constants" — a
category whose blanket reason ("their 5.4e-4-class distance from the exact
constant is above the calibrated oracle floors") is right for the constants that
feed an impedance and **wrong for this one**, which feeds a *discrete argmin*.
Re-measuring the category one member at a time is what found it. `TODO(compat)`
**25 → 24**; both lanes green; no golden, tolerance, ledger or deck touched.

**The row.** `SetVoltageBases` (the `CalcVoltageBases` command) writes

```pascal
kVBase := NearestBasekV(Cabs(NodeV^[GetRef(1)]) * 0.001732) / SQRT3;  // l-n base kV
```

— `Common/Solution.pas:1103`, and the identical statement in EPRI r4133
`Version8/Source/Common/Solution.pas:2541`, so **both** gating oracles carry it.
The same `√3` appears twice in one line: truncated to four digits on the way in,
and as the unit's full-precision startup `SQRT3` on the way out. That is the
single-site membership rule (ii) — "the Pascal itself … says what was meant" —
satisfied inside one statement, so the marker's own "clean fix is a single
constant" needed no probe. Parity keeps `0.001732`; the default lane scales by
`SQRT3/1000` (`compat::kv_base_search_scale`).

**Why it is a lane row and not a re-baseline: the scaled estimate is never
stored.** It is consumed by `nearestBasekV`, a **relative**-distance argmin
(`|1 − kv/base|`) over the `Set VoltageBases` list, and the bus then records
`matched / SQRT3` — a number taken verbatim from the user's list, not from the
estimate. A 2.93e-5-low estimate therefore writes a **bit-identical** `kVBase`
unless it sits within 2.93e-5 of a tie between two adjacent legal bases, the tie
of `a` and `b` in this metric being their harmonic mean `2ab/(a+b)`. Measured,
not assumed: with the flip selected the entire suite — every byte golden and all
520 gated corpus cases — is unchanged in the default lane.

**Pins, at both ends of the row.**
`compat::tests::kv_base_search_scale_kernels_differ_by_the_truncation` holds the
two `_impl`s against each other (one-sided, 2.9333e-5..2.9334e-5) in both lanes.
`dispatch::tests::kv_base_search_scale_moves_only_a_tied_selection` drives the
argmin itself: at a constructed tie (`12.47`/`13.2`, harmonic mean 12.8246…) the
two scales select **different** bases, and at four voltages away from it they
select the same one — so the test states the divergence *and* the reason the
suite does not move. `dispatch::tests::calc_voltage_bases_snaps_to_the_lane_
estimate` then runs the real command on a source built to sit inside that
2.93e-5 window (`basekv=12.8248`): the parity lane stores `12.47/√3` on every
bus, the default lane `13.2/√3`.

**Housekeeping.** `nearest_base_kv` now takes the legal-base slice instead of
the whole `&Circuit` (it read one field), which is what makes the argmin
directly testable; its doc states the metric, the scan order and the tie rule.

**The `CorpusGuard` same-directory race recurred — third occurrence, same
shape.** The first parity-lane run of this commit's gate reported
`519/520 … 1 failed`; re-running the *same, unchanged* test binary gave
`520/520`, and the full parity workspace re-run gave 2352/0/5. Lane-independent
(F.3m saw it in the default lane, F.3p in the parity lane), change-independent
(the parity alias here **is** the pre-existing literal, so the parity engine is
bit-identical to `HEAD~1`), and not reproducible on a fixed binary. It remains
the open harness item F.3m diagnosed — `electricdss-tst/Test/AutoTrans` writes
fixed-name export files and the scheduler runs two cases from that directory
concurrently; the fix is to serialize (or per-case-scratch) same-directory cases
in `corpus_gate::scheduler`, and to print the failing label from the scheduler
rather than the panic body, which the redirect again swallowed.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` (2352 passed /
0 failed / 5 ignored) and the same with the feature (2352 / 0 / 5), including
the unconditional 520-case corpus gate. Tests +3, 0 removed, 0 new `#[ignore]`.
`git diff -- tests/` empty; `git status --short tests/corpus` empty after both
runs (the known intermittent `Test/AutoTrans/*` leak deleted by exact name).

### DE_PASCALIZE Stage F.3v — the four remaining quirks stop being predictions: all four are whole-artifact exclusions (branch `depas-stagef`, 2026-07-28)

The single-site quirk sweep opened at 28 rows (F.3k) and came down to four. Each
of those four carried a *prescription* inherited from F.3k — "a field-scoped
default-lane exclusion plus a replacement in-engine assertion and a transitive
cover, the Newton row's shape" — and none of them carried a measurement. This
session implemented and gated all four flips. **The prescription is wrong for
every one of them**: each moves the *node voltages* (or, for the Generator, the
whole dynamics surface), so each is a **whole-case / whole-golden** default-lane
exclusion, not a field-scoped one. All four are therefore escape-recorded with
their numbers, per the executor escape protocol, and the sweep closes here at the
executor's authority. `TODO(compat)` stays **25**; both lanes green; no golden,
tolerance, ledger or deck touched, and no engine behaviour changed.

| row | the flip that was run | what it moves | measured |
|---|---|---|---|
| GICTransformer `G2` off `%R1` (`gic_transformer/solve.rs`) | `g2 = 100/(z_base2 * pct_r2)` | `type=Auto` puts the G1 and G2 blocks in **series** on the H→X→neutral path (the `BusX` side effect sets terminal 2 to `BusX`, `GICTransformer.pas:251`), so honouring `%R2` changes the element admittance, the system Y and every node voltage downstream. What F.3k called "the deck's GIC current" is in fact `corpus_gate::runner`'s **first** comparison — the node voltages — which is why both runs abort at its `entry 0` | `gictransformer_gic.dss` 4.502e-4 (allowed 1.001e-6); `gic_midi.dss` 1.021e-4 (allowed 1.074e-6) |
| Capacitor `Cuf` discarded write (`capacitor/solve.rs`) | `SetStructF64s(CUF, [Cs-Cm, 0…])` plus the `CUF` arm `set_struct_f64_array` still lacks | r4133 predates the typed-setter refactor and *does* apply the write — `S := S + Format(' Cuf=%-.5g', [Cs-Cm])` then one `Edit(ActorID)` (`Capacitor.pas:829`) — through `InterpretDblArray`, whose own comment says it "fills array with zeros if we run out of numbers" (`Common/Utilities.pas:788-791`). So the faithful fix is the array write the parser would have made, which collapses `cap_cmat` (`cmatrix=[10 / -2 10 / -2 -2 10]`) to `Cs - Cm = 4 µF` where parity keeps reading the 10 µF diagonal. Node voltages again | `makeposseq_shunt.dss` 1.438e-1 V (allowed 3.339e-6) |
| LoadShape MMF plain-text accept-set (`load_shape/compute.rs`) | column taken verbatim, trimmed, through the float parser (comma walk unchanged) | the witness is as strong as the membership rule asks — `TLoadShapeObj` owns a *second* reader for the same format, `ReadCSVFile`'s non-mapped branch (`LoadShape.pas:1044`), which honours sign and exponent. But `tests/corpus/modes/inputformat/shape_mmf/shape_mmf.dss` exists **to observe** the quirk: `mmpq8.csv`'s P column is deliberately exponent notation, so `ls_pq` reads `{1.51, 2.01, …}` mapped vs `{0.15, 0.20, …}` unmapped | `shape_mmf.dss` **1.641e1 V** (allowed 8.179e-6) |
| Generator Model=6 stale `Vterminal` seed (`generator/user_model.rs`) | one `compute_vterminal(node_v)` ahead of the two `FInit` calls | the site guessed this would shift the initial state "less than the power-flow convergence tolerance (~1e-4 pu)". It seeds `E1` for the *whole* dynamics run | the entire `wasm_gen_dyn` r4133 golden: `dSpeed` **3.449e-2 relative** (-118.32 vs -122.55 Deg/sec), `Slip` 9.824e-5, `Is1`/`Ir1` ≈5.3e-4, stator/rotor losses ≈1.1e-3, six of twelve node-voltage components past their floors, and all four step-1 trajectory channels |

**Why that ends the sweep here rather than four commits later.** The IV.2 drift
model sanctions excluding a deliberate divergence "from oracle comparison **at
those fields**". A whole-case exclusion is not that: landing GIC costs **two of
520 gated cases** their entire default-lane oracle comparison — and with them
their unrelated GICLine-geodesy / GICsource / ordinary-Line surface; Capacitor
costs `makeposseq_shunt.dss`; LoadShape costs `shape_mmf.dss` *including* its
sng/dbl/`mult=(sngfile=)` MMF-reader coverage, which has nothing to do with the
accept-set; the Generator costs every node voltage and all 34 variables of the
only deck exercising a Model=6 user model **and** a ShaftModel, on an r4133
golden the parity lane may never re-baseline. That is a coverage trade plus a
piece of harness machinery (a whole-case lane skip) the plan does not sanction,
and the brief's escape protocol covers exactly this case: "leave the site, green,
list it".

**What is left behind is executable, not narrated.** Each of the four markers now
carries its own measurement and the shape of the fix, so the owner decides from
numbers rather than from a prescription. Two new tests keep the LoadShape finding
from rotting, and both run in **both** lanes because the row is reproduced in
both:

* `mmf_text_reader_disagrees_with_its_non_mapped_twin` — the same bytes
  (`-0.500`, `1.5e-3`, `+2.000`, ` 0.250`) through the mapped and the non-mapped
  reader, each pinned at its exact value, with the disagreement asserted to be a
  *deletion* (rows 1-2 differ by >0.9 and >1.5) while the two rows carrying
  neither a sign nor an exponent are asserted **equal** — which is what makes it
  a filter artifact rather than two unrelated parsers.
* `mmf_accept_set_quirk_is_gated_by_exactly_one_deck` — the corpus-byte
  measurement that decided the row, kept live: the vendored
  `ckt24/LS_Phase_AOK.{txt,csv}` must stay *inside* the accept-set (so the three
  gated ckt24 MMF-text cases cannot see a fix) and `mmpq8.csv` must keep its
  exponent bytes (`45`, `101`, so `shape_mmf.dss` still can). Either half moving
  fails this test instead of silently invalidating the escape record.

GIC additionally leaves the transitive cover it would need, spelled out at the
site: a GICTransformer given `R1=`/`R2=` in **ohms** takes the `else` branch, has
no quirk and is identical in both lanes (`tg2` in the very same deck is one), so
a corrected `%R` path can be pinned against the oracle-gated `R` path instead of
against a re-baselined deck.

**Escape register, final for F.3 at executor authority.** The **14** truncated
physical constants (their 5.4e-4-class distance from the exact constant is above
the calibrated oracle floors — an UPGRADE-rung re-baseline, not a lane flip), the
**7** F-FMT rendering markers (F.4's defined scope), the now-measured **4**
single-site quirks above, and `HIDE_015X` ×17 (retiring it needs a byte-golden
re-baseline against a different oracle, which the parity lane may never do).
14 + 7 + 4 = the 25 remaining markers. `rg "TODO\(compat\)" crates` = **25**,
`rg HIDE_015X crates` = **17**.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +2
(`mmf_text_reader_disagrees_with_its_non_mapped_twin`,
`mmf_accept_set_quirk_is_gated_by_exactly_one_deck`), 0 removed, 0 new
`#[ignore]`. `git diff -- tests/` is empty — no golden, tolerance, ledger or
deck touched; `git status --short tests/corpus` empty after both runs (the known
intermittent `Test/AutoTrans/*` + `asymmetric/line/DA1FC.tmp` leak deleted by
exact name).

### DE_PASCALIZE Stage F.3u — a unit conversion that reads the wrong field, and the one row whose upstream is not Pascal (branch `depas-stagef`, 2026-07-28)

Two markers, two split rows, and between them the sweep's two extremes of
reach: one that no gated deck can currently distinguish and one that two gated
monitors hit on every run. `TODO(compat)` **27 → 25**; both lanes green; no
golden, tolerance, ledger or deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `HEIGHT_UNIT_CHANGE_REREADS_THE_METRES_FIELD` | `TLineConstants.Set_FuserHeightUnit` moves the unit field and then calls `Set_FheightOffset(FheightOffset)` (`LineConstants.pas:689-695`; byte-identical in r4133 `Version8/…:689-696`) | `FheightOffset` is declared *"The height is always saved in meters here"* (`:71`, `:97`) while `Set_FheightOffset`'s argument is a **user-unit** number it multiplies by `To_Meters` (`:676-687`) — a metres value fed into a user-unit parameter. The line's own comment states the intent the fix implements: *"This updates the existing value to fit the new user units"* | re-reads the number the user typed — `Get_FheightOffset()`, the expression the class already has (`:396-399`), captured before the unit field moves |
| `MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM` | for a monitor that has flushed nothing, `Channel(i)` reports a one-element `[0.0]` | the **engine** does not do this: `CAPI_Monitors.pas:295-331` returns `DefaultResult` — an empty array. The padding is dss-python's Python-side `IMonitors.Channel` (`dss/IMonitors.py:28-55`), which bypasses `Monitors_Get_Channel` entirely, reads the raw `ByteStream` and short-circuits `if cnt == 272: return np.zeros((1,))` | the empty channel, which also makes `Channel` agree with `dblHour` — the same stream through a surface dss-python does *not* special-case, already empty in both lanes |

**The height row is a lane split precisely because it changes nothing yet.**
Its only consumer is the Line → Carson push, whose call order is fixed —
`SetEpsRMedium`, `SetHeightOffset`, `SetUserHeightUnit`
(`line_geometry::matrix::set_line_constants_medium`, `Line.pas:2049-2051`).
The offset is stored while the engine still carries its constructed `UNITS_M`,
so `From_Meters(m) = 1` and both readings re-apply the same number: that is
what makes `HeightUnit=ft` mean anything at all on
`tests/corpus/modes/upgrade/upgrade_linecs_heightoffset.dss`
(`HeightOffset=5 HeightUnit=ft` → 1.524 m), and the deck is byte-identical in
both lanes. They part only on a *second* unit change, where the outgoing unit is
no longer metres — parity compounds the conversions (5 ft → 1.524 m → 1.524 in),
the default lane keeps the typed 5.
`height_unit_change_rereads_the_typed_number` pins both halves: the shared
metre-sourced change bit-for-bit (offset **and** every conductor `Y`), then the
ft→in divergence, plus a discriminator asserting the two readings are a factor
0.3048 apart — never confusable with the one-ULP ft→m→ft round-trip the same
test tolerates by name.

**The monitor row is the opposite, and it is the first row in the sweep whose
upstream is a *client*, not the Pascal.** Reproducing dss-python's convenience
wrapper inside the engine means a caller asking a freshly-sampled monitor for a
channel is handed a fabricated zero sample. And the unflushed state is not a
corner of the corpus: instrumenting the transform for one default-lane corpus
run (throwaway probe, reverted before commit) shows it firing **787** times —
`SolveGeneralTime` never calls `SaveAll`, and neither does a case captured
before its monitors are saved. So this row could not be flipped by leaving the
oracle comparison alone.

**So the oracle keeps gating those monitors; the capture is mapped, not
dropped.** `harness::lane::expected_monitor_channel` rewrites a capture to the
empty channel only when **both** hold: the Rust monitor's own flush cursor is 0
(now exposed as `MonitorView::flushed_records`, `Monitor::flushed_records()`),
and the capture really is the placeholder — exactly one sample, exactly `0.0`.
The parity arm is the identity. That keeps the transform from rotting in either
direction, which `monitor_transform_is_the_unflushed_placeholder` asserts:
a *flushed* monitor's identical `[0.0]` is compared strictly in both lanes (so
an engine that lost real samples still fails the length check), and an
unflushed capture of any other shape — `[0.0, 0.0]`, `[1.0]`, `[]`, `[1e-30]` —
is passed through untouched, so if dss-python ever stops padding, the compare
fails loudly instead of passing silently. Every other monitor assertion is
unchanged: header, `SampleCount`, channel count and every sample stay exact in
both lanes.

**Replacement pins.** `monitor::tests::channel_reflects_flush_state` now asserts
the lane's unflushed reading (and the flush cursor on both sides of `save()`),
and the new `channel_placeholder_is_confined_to_the_unflushed_stream` fences it
in from the other side — an out-of-range index is empty in both lanes, and a
flushed monitor never pads. `to_csv_flushes_like_pascal_save` reads the same
lane helper.

**Carry-over, corrected: the Capacitor `Cuf` row's proposed direction was
wrong, and the source says why.** F.3s recorded that
`SetDouble(ord(TProp.Cuf), Cs - Cm)` is silently discarded (no array arm in
`SetObjDouble`) while r4133 `Capacitor.pas:829` *does* write it, and concluded
"the default lane should therefore **write** it… what blocks the flip is the
array semantics, and that needs an `epri-worker` probe". No probe is needed —
`InterpretDblArray` (r4133 `Common/Utilities.pas:788-791`) answers it in a
comment: *"Fills array with zeros if we run out of numbers"*. So r4133's
`Cuf=<scalar>` on an `N`-step bank sets step 1 and **zeroes steps 2..N**, which
is not a behaviour to adopt as-is. Two consequences for whoever picks this up:
(i) for the common `NumSteps=1` capacitor the divergence is real and r4133 is
right; (ii) the faithful default-lane fix is not "write the scalar" but "write
it through the array path the *parser* would have taken" — first element, zeros
after — which reproduces r4133 exactly instead of inventing a third behaviour.
The marker stays, unchanged and green, with this now on the record.

**Scope discipline.** No IV.2 kernel row was added; both enter under the
*Single-site upstream quirks* membership rule (the monitor row with the stronger
form of criterion (ii) — the *engine's* own C-API is the sibling that
contradicts the padding). Escape register unchanged: the **14** truncated
physical constants, the **7** F-FMT rendering markers, `HIDE_015X` ×17; the
single-site quirk census drops **6 → 4**, and 14 + 7 + 4 = the 25 remaining
markers.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +3
(`height_unit_change_rereads_the_typed_number`,
`channel_placeholder_is_confined_to_the_unflushed_stream`,
`monitor_transform_is_the_unflushed_placeholder`), 0 removed, 0 new
`#[ignore]`. `git diff -- tests/` touches no golden, tolerance, ledger or deck;
`git status --short tests/corpus` empty after both runs.

### DE_PASCALIZE Stage F.3t — the Storage conversion's missing `BeginEdit`, settled by the engine it was refactored from (branch `depas-stagef`, 2026-07-28)

F.3s left this row argued but unimplemented, with a hypothesis about how it
could be pinned. The hypothesis was **wrong** and the measurement replaces it:
the row costs five recalcs and moves nothing else — which is what makes it the
first split in the sweep that the gate does not have to pay for.
`TODO(compat)` **28 → 27**; both lanes green; no golden, tolerance, ledger or
deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `STORAGE_POSSEQ_LEAVES_ITS_SETS_UNBRACKETED` | `TStorageObj.MakePosSequence` counts its writes into `changes` (`Storage.pas:3337` `= 3`, `:3342` `+ 2`) and hands the count to a single `EndEdit(changes)` (`:3352`) — the bookkeeping of *one* edit — but never opens it with `BeginEdit(True)`. Each `SetInteger`/`SetDouble` therefore auto-brackets itself and the dangling `EndEdit` adds one more | **three** independent readings: (i) the sibling with the identical body, `TPVsystemObj.MakePosSequence` (`PVsystem.pas:2642-2673`) — same `changes` counter, same five writes — opens at `:2649`; (ii) of the **eleven** `MakePosSequence` bodies in the tree that call `EndEdit`, **ten** open first (`generator.pas:2827`, `Load.pas:2237`, `Line.pas:1541`, `Transformer.pas:1739`, `AutoTrans.pas:1780`, `Reactor.pas:1058`, `Capacitor.pas:790`, `VSource.pas:1208`, `GICLine.pas:668`, `PVsystem.pas:2649`) — Storage is the only one that does not; (iii) **r4133, which predates the refactor**: `Version8/…/Storage.pas:3962-3987` and `PVsystem.pas:2846-2863` both build one command string and call `Edit(ActorID)` **once** — one edit, one recalc, for *both* classes | the writes are bracketed |

**The r4133 reading is the load-bearing one** (`DIVERGENCES.md` §D14 — a
0.15.x/0.14.x shape is never authority on its own): the pre-refactor engine
gives Storage and PVSystem the *same* single-edit semantics, so the typed-setter
rewrite kept it for PVSystem via `BeginEdit` and lost it for Storage. The
default lane therefore moves **towards** one gating oracle, not away from either.

**What it costs is the recalc count and nothing else — measured, not argued.**
F.3s guessed that an intermediate recalc "can flip `storage_state` in
`dispmode=Load`/`Price` and the final recalc need not flip it back". It cannot:
the port's `end_edit` is `recalc` + `yprim_invalid`, `recalc` derives every
field it writes from the element's *current* property values, and its one
state-carrying step — `CheckStateTriggerLevel` / `ComputePresentkW` — reads only
`kWhStored`, `kWhRating`, `kWhReserve`, the two triggers and the dispatch level,
**none of which `MakePosSequence` writes**. Repeating it at half-converted
property values is therefore idempotent. That is asserted rather than narrated:
`makeposseq_begin_edit_moves_only_the_recalc_count` builds two identical
`DispMode=Load` 3-phase elements whose trigger pair straddles the dispatch level
(so the state machine is live and the fixture asserts it settled to
DISCHARGING), drives the unbracketed list over one and the bracketed list over
the other through the real property engine with the applier's own VM semantics,
and then asserts `(6, 1)` recalcs and bit-for-bit equality of six discrete
fields and twenty-six numeric ones (`kWrating`, `VBase`, `Yeq`, `YeqDischarge`,
`kW_out`, `PnomPerPhase`, `pctkWout`, `Rthev`, `CutInkW`, …).

**So the gate pays nothing.** `tests/corpus/modes/makeposseq/makeposseq_pc.dss`
— the gated deck that runs `makeposseq` twice over a 3-phase `Storage.st1` — is
byte-identical on its channel in both lanes, and no `LANE_SKIP_*` entry, ledger
row or golden was needed. Every other split so far has cost either a field-scoped
exclusion or a golden transform; this one is free because the divergence is
confined to how many times an idempotent recalc runs.

**Replacement pins.** `makeposseq_plan_brackets_its_writes_only_in_the_default_lane`
is the expected-value test for the row itself (the bracket appears iff the lane
fixes the slip; the five writes, their values and their order are asserted
identical in both lanes). The two pre-existing plan tests are now lane-aware
through one `posseq_head()` helper — `makeposseq_storage_three_phase_no_begin_edit`
loses the now-lane-dependent half of its name and becomes
`makeposseq_storage_three_phase`, and the 1-phase twin keeps its list.

**Scope discipline.** No IV.2 kernel row was added; the split enters under the
*Single-site upstream quirks* membership rule, and passes criterion (ii) —
nothing in the source says the missing bracket was meant; the `changes := 3 + 2`
counter says the opposite. Escape register unchanged: the **14** truncated
physical constants, the **7** F-FMT rendering markers, `HIDE_015X` ×17; the
single-site quirk census drops **7 → 6**, and 14 + 7 + 6 = the 27 remaining
markers.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +2
(`makeposseq_plan_brackets_its_writes_only_in_the_default_lane`,
`makeposseq_begin_edit_moves_only_the_recalc_count`), 0 removed, 0 new
`#[ignore]`. `git diff -- tests/` touches no golden, tolerance, ledger or deck;
`git status --short tests/corpus` empty after both runs.

### DE_PASCALIZE Stage F.3s — the Fault dump's off-by-one tail, and two carry-over cleanups (branch `depas-stagef`, 2026-07-28)

One marker, one line of dump text, and the shortest sibling argument in the
sweep so far. `TODO(compat)` **29 → 28**; both lanes green; no golden,
tolerance, ledger or deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `FAULT_DUMP_TAIL_REPRINTS_MINAMPS` | `TFaultObj.DumpProperties` runs its generic tail as `for i := NumPropsthisClass to ParentClass.NumProperties` (`Fault.pas:533`), and `NumPropsThisClass = Ord(High(TProp)) = 9 = MinAmps`, so the first iteration re-emits the property the custom `%.1f` line just wrote — `~ MinAmps=5.0` then `~ MinAmps=5` | the **three** other classes with this exact loop all write `NumPropsthisClass + 1`: `Transformer.pas:1276`, `AutoTrans.pas:1307`, `XfmrCode.pas:663`. No class double-prints a property on purpose | the tail starts at `NormAmps` |

**The goldens stay the oracle's.** `golden_reports::fault_dump_expected` removes
the **second** member of each adjacent `~ MinAmps=` pair in the default lane and
nothing else, so all **four** goldens that carry a Fault dump —
`dump_fault{,_gmatrix}.txt` (one Fault each) and the whole-circuit
`dump3_{bare,debug}.txt` (two each; caught by the first gate run, which is what
the gate is for) — remain pinned to the captured oracle in both lanes. The engine
is then held to that expectation line-for-line, which is the expected-value pin
in both directions (twice per Fault in parity, once in default). The transform is
*positional* — "the second of two adjacent lines" — never "the line that looks
generic", so a differently-rendered `MinAmps` can never be eaten by it.
`fault_dump_goldens_carry_the_double_print` keeps it honest across all four: the
oracle side must still carry exactly two lines per Fault (so a recapture cannot
rot the transform into a no-op), the default-lane expectation exactly one, and
each survivor must be the *first* of its pair — the custom `%.1f` render, not the
generic one. `run_deck_dump_exact` grew an `_expected` twin for this; every other
dump golden passes the identity.

**Two carry-overs from F.3r, gated by this commit's run.** (i) `corpus_gate::
runner`'s new `class_member_names` helper had been inserted *between*
`run_rust_capture`'s doc comment and the function — moved above it. (ii)
`lane::expected_eventlog` now documents that its `Debug Sample: Relay.` drop is
unconditional, which is exact only while no gated deck sets `DebugTrace=yes`
(verified empty over `tests/corpus/controls`), and that the failure mode if one
ever does is a loud length mismatch, never a silent pass.

**Scope discipline.** No IV.2 kernel row was added. Escape register unchanged:
the **14** truncated physical constants, the **7** F-FMT rendering markers,
`HIDE_015X` ×17; the single-site quirk census drops **8 → 7**, and
14 + 7 + 7 = the 28 remaining markers.

**Two rows argued but NOT implemented, recorded so the next pass does not
re-derive them.**

* **Capacitor `SetDouble(ord(TProp.Cuf), Cs - Cm)` silently discarded**
  (`Capacitor.pas:814`, `pd/capacitor/solve.rs`). The site's own marker proposes
  "drop the discarded write"; that is the **wrong** fix, and r4133 says so:
  `Version8/Source/PDElements/Capacitor.pas:829` builds `' Cuf=%-.5g'` and parses
  it, so the pre-refactor EPRI engine *does* write the value — the dss_capi move
  to typed setters lost it, because `SetObjDouble` has no array arm and `Cuf` is
  a `DoubleArrayProperty`. The default lane should therefore **write** it (which
  also moves us *towards* one gating oracle, not away). What blocks the flip is
  the array semantics of that write — which step(s) of a multi-step bank the
  scalar lands on — and that needs an `epri-worker` probe, not an argument.
* **Storage `MakePosSequence` has no `BeginEdit`** (`Storage.pas:3339`; the
  PVSystem sibling at `:2649` opens one). Measured this session: the difference
  is not merely "one extra recalc". `recalc` → `set_nominal_der_output` →
  `check_state_trigger_level` **latches** `storage_state` in `dispmode=Load`/
  `Price`, so an intermediate recalc — run at `phases=1` with the *old* `kV`/
  `kWrated` — can flip the state and the final recalc need not flip it back.
  A pin is therefore constructible (a `dispmode=load` bank whose trigger
  straddles the intermediate configuration); building it is the next step.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +1
(`fault_dump_goldens_carry_the_double_print`), 0 removed, 0 new `#[ignore]`.
`git diff -- tests/` touches no golden, tolerance, ledger or deck; `git status
--short tests/corpus` empty after both runs.

### DE_PASCALIZE Stage F.3r — the Relay's two event-log slips, each contradicted by its own Recloser donor (branch `depas-stagef`, 2026-07-28)

Both markers are r4133 copy-paste damage in the same class, and for both the
Recloser — the file the text was copied *from* — spells the fix.
`TODO(compat)` **31 → 29**; both lanes green; no golden, tolerance, ledger or
deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `RELAY_SAMPLE_TRACE_IGNORES_DEBUGTRACE` | `TRelayObj.Sample` closes its state resync with a bare `AppendtoEventLog('Debug Sample: Relay.'+Name, 'FPresentState: …')` (r4133 `Relay.pas:1325`) — no `DebugTrace` guard, and not gated on `ShowEventLog` either | the Recloser's byte-identical line **is** guarded (`Recloser.pas:1044`), and so is every *other* `Debug Sample` line in `Relay.pas` itself (`:1822`, `:1837`, …) — exactly one line lost its guard | written only under `DebugTrace`, through the `Relay::dbg` helper the port already had |
| `RELAY_RESET_EVENT_IS_LABELLED_RECLOSER` | both `CTRL_RESET` arms of `TRelayObj.DoPendingAction` log `'Recloser.'+Self.Name` (`Relay.pas:1196`, `:1212`) | they are verbatim copies of `Recloser.pas:909`/`:924` — same format strings, same guard — while all ~10 other events in the same procedure (`:1087`-`:1176`) write `'Relay.'+Self.Name` | `Relay.<name>` |

**Why this is worth splitting and not just noise.** The first row puts a *debug*
line into the user-facing event log on every control sample of every relay,
which is the one thing `ShowEventLog`/`DebugTrace` exist to prevent. The second
attributes a relay's reset to a class that may not exist in the circuit — or, if
it does, to the wrong device.

**The oracle stays the source of truth on all 11 gated relay-bearing decks**
(the nine `controls/relay/*` plus `controls/combo/{combo,midi}_protection.dss`;
the other two eventlog-gated `controls/fuse/indmach_r4133/*` decks declare no
relay). Their event logs are the *point* of those `oracle: "r4133"` cases, so
re-capturing them for the default lane would trade an oracle proof for two label
changes.
Instead `harness::lane::expected_eventlog` applies an enumerated transform to the
oracle capture — drop the `Debug Sample: Relay.` lines, relabel the relay reset
lines — and the engine is still held to the result line-for-line, count
included. The parity arm is the identity.

**The relabel refuses to guess.** It fires only on the two copied wordings
(`PHASE … RESET (1PH RESET)` / `(3PH RESET)`) and only when the named device is
a Relay of the circuit **and not** a Recloser of it — the names come from the
Rust element set, which the same case has already pinned against the oracle. A
genuine recloser reset (identical wording) is never touched, and a circuit
carrying both classes under one name is left alone to fail the compare loudly
rather than be silently rewritten. `eventlog_transform_is_the_two_relay_rows`
asserts exactly that, including that a `Debug Sample: Recloser.` line — which
only appears when the user *asked* for it — survives in both lanes.

**Non-vacuity, measured rather than assumed.** A parity-lane `dss-cli` run of
each of those 11 decks emits 1–2 `Debug Sample: Relay.` lines per solve
(`relay_4647_asym`, `midi_relay_4647`, `relay_generic` two; the other eight
one), so the drop is load-bearing on every one of them. The **reset** relabel is *not* reached by a
single-solve run of any of them (0 `Element=Recloser.` lines); it is pinned by
`do_pending_reset_only_resets_opcount_d4`, which now asserts both directions —
the parity lane must log `Recloser.r1` and must *not* log `Relay.r1`, and the
default lane the reverse.

**Replacement pin for the trace row.** `sample_state_trace_is_the_lane_guard`
walks all four `DebugTrace × ShowEventLog` combinations and asserts the line
appears iff `debug_trace || ORACLE_PARITY` — so the row cannot degrade into "the
line is gone": with `DebugTrace=yes` both lanes must still write it, and
`ShowEventLog` must gate it in neither, which is what distinguishes this line
from every protection event around it.

**Scope discipline.** No IV.2 kernel row was added; both rows enter under the
*Single-site upstream quirks* membership rule. Escape register unchanged: the
**14** truncated physical constants, the **7** F-FMT rendering markers,
`HIDE_015X` ×17; the single-site quirk census drops **10 → 8**, and
14 + 7 + 8 = the 29 remaining markers.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +3
(`sample_state_trace_is_the_lane_guard`,
`eventlog_transform_is_the_two_relay_rows`,
`eventlog_reset_relabel_needs_an_unambiguous_relay`), 0 removed, 0 new
`#[ignore]`. `git diff -- tests/` touches no golden, tolerance, ledger or deck;
`git status --short tests/corpus` empty after both runs.

### DE_PASCALIZE Stage F.3q — the `MakePosSequence` family: one guard split, one divisor kept (branch `depas-stagef`, 2026-07-28)

Two markers on the same conversion procedure, and they resolve in **opposite**
directions — which is the point of arguing each site against the Pascal instead
of counting them. `TODO(compat)` **33 → 31**; both lanes green; no golden,
tolerance, ledger or deck touched.

| row | upstream | disposition | default lane |
|---|---|---|---|
| `GENERATOR_POSSEQ_RATING_GUARDS_READ_XDP_SLOTS` | `had_kVA := PrpSequence[26] > 0` / `had_MVA := PrpSequence[27] > 0` (`generator.pas:2804-2805`) — the only two raw ordinals in a procedure that names every other property `ord(TProp.…)`; 26/27 are `Xdp`/`Xdpp`, `kVA`/`MVA` are 23/24 | **lane split** | the guards read the two properties they are named after |
| Load's `kW/3.0` divisor (`Load.pas:2230`) | a hard-coded 3, where Generator/PVSystem/Storage all divide by `Fnphases` | **permanent semantics, both lanes** — marker replaced by documentation | unchanged |

**Why one is a defect and the other is a decision — stated from the source, not
from symmetry.** The Load divisor carries upstream's own rationale *and its
date*: "New Method: Assume load is distributed equally among the 3 phases --
works better // 1-5-2016 RCD", written where the replaced `/Fnphases` still
shows in the comment. That is criterion (ii) of the single-site membership rule
failing in the strongest possible way: the source says the 3 **is** what was
meant, so a lane that "fixed" it would be changing a modelling choice. The
generator guards have the opposite evidence: the local names (`had_kVA`,
`had_MVA`), the `SetDouble(ord(TProp.kVA), …)` each one gates (`:2841`), and the
third guard of the same block — `had_kvars`, raw `[19]`/`[20]`, slots that still
*are* `Maxkvar`/`Minkvar`. It is a rename that outran its literals. It also gets
the answer wrong in both directions, which is what the pins assert: `kVA=250`
survives a `makeposseq` unscaled, while setting a *reactance* (`Xdp=`) divides
the kVA rating.

**The gate cost is four cells, and they are four because upstream aliases two
properties onto one field.** `makeposseq_pc.dss` was built for this quirk, so
the default lane necessarily moves there. `PropertyOffset` aims both `kVA`
(`:620`) and `MVA` (`:640`) at `GenVars.kVArating`, so the rating shows twice per
generator: `(g_kva|g_mva) × (kva|mva)`. Those four cells — of a deck with 13
elements, 4 probes and a full-property dump — are what the new
`harness::lane::LANE_SKIP_PROBE_PROPS` drops in the **default lane only**, at
both surfaces that read them (the probe loop and `compare_all_properties`, the
latter through the same oracle-value rewrite a ledger `property` scope uses).
Nothing else about the deck is relaxed: the same generators' `phases`/`kv`/`kw`,
`g_kvar`'s four probes, every other property of the same two elements, the
element channels, voltages, system Y, discrete state and iteration count stay
oracle-compared in both lanes.

**That exclusion is safe because the rating is not a power-flow quantity, which
is measured rather than assumed**: `kVArating` is read in exactly two places
(`rg` over `generator.pas`) — the `Xdp`/`Xdpp` ohm conversion (`:1281-1282`) and
the dynamics inertia constants `Mmass`/`D` (`:2436-2437`). Neither is touched by
the deck's snapshot solve, and the live run confirms it: with the four cells
excluded the case is byte-clean on both channels, i.e. no voltage, current,
power or iteration count moved.

**Replacement pins.** The three `generator/tests.rs` cases that used to pin the
quirk now pin the *row*, asserted as an equality against `compat::ORACLE_PARITY`
so they are load-bearing in both lanes: `kVA=` and `MVA=` each emit their divide
in the default lane and nothing in the parity lane, and `Xdp=` trips the kVA
divide only in the parity lane. `harness::lane::probe_exclusion_is_one_cell_wide`
keeps the exclusion honest from the other side — the listed cells are dropped in
the default lane only, and their siblings (another property of the same element,
the same property on another element, the same cell on a label with a suffix)
must still be gated in both lanes.

**Scope discipline.** No IV.2 kernel row was added; the split enters under the
*Single-site upstream quirks* membership rule. Escape register unchanged: the
**14** truncated physical constants, the **7** F-FMT rendering markers,
`HIDE_015X` ×17; the single-site quirk census drops **12 → 10**, and
14 + 7 + 10 = the 31 remaining markers.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +1
(`probe_exclusion_is_one_cell_wide`), 0 removed, 0 new `#[ignore]`. `git diff --
tests/` touches no golden, tolerance, ledger or deck; `git status --short
tests/corpus` empty after both runs.

### DE_PASCALIZE Stage F.3p — the `Set CktModel=` LongBool row, and the two unmarked port divergences it uncovered (branch `depas-stagef`, 2026-07-28)

One marker, three engine sites — and the row's real value is that only **one**
of the three was reproducing upstream at all. `TODO(compat)` **34 → 33**; both
lanes green; no golden, tolerance, ledger or deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `CKT_MODEL_RENDERED_ORDINAL` | `CktModelEnum.OrdinalToString(Integer(PositiveSequence))` with `PositiveSequence: LongBool` (`Circuit.pas:180`) → `Integer(True)` = **-1**, out of the enum's `[0,1]` range → `''`, so a positive-sequence circuit reports `Set CktModel=` with no value | the oracle's **own** JSON round-trip golden records the loss: `rt_positive_seq.json`'s J0 carries `Set CktModel=` and J1 — the oracle re-exporting *its own import of J0* — carries no `CktModel` line at all | ordinal `1` → `Positive` |

**The finding.** The three surfaces that render this datum
(`CAPI_Obj.pas:2537` → `report/export/json/circuit.rs`, `Circuit.pas:2768` →
`exec/save_circuit.rs`, `ExecOptions.pas:919` → `exec/get_cmd.rs`) all use the
identical Pascal expression, but only the JSON one had been ported that way. The
`Save` writer had been ported as `ordinal_to_string(1)` and the `Get` reader as
`positive_sequence as i32` — i.e. **the fixed form, in both lanes, with no
marker**. Neither is covered by any golden or gated corpus case (no `Save`-golden
circuit is positive-sequence; nothing captures `Get cktmodel`), which is why the
port has been silently diverging there since the 1:1 stage. Routing all three
through the one Stage F row makes the parity lane faithful on two surfaces where
it was not, and gives the default lane a single answer everywhere.

**Why the fix is a fix and not a spelling preference — stated as a consequence,
not an opinion.** `Set CktModel=` does not survive its own re-import: the
value-less form parses as `Multiphase`, so a saved circuit silently loses the
flag the file was written to record. `ckt_model_rendered_ordinal_is_lane_split`
asserts exactly that end to end — it saves a positive-sequence circuit,
re-compiles the produced `Master.dss` in a fresh engine, and requires
`Get cktmodel` to answer `Multiphase` in the parity lane and `Positive` in the
default one. The parity lane thereby *pins the data loss* rather than merely
tolerating it.

**No golden is re-baselined.** The one gated surface, `tests/golden/json/
circuit_positive_seq.json`, is compared through the same enumerated
expected-value transform F.3n introduced for CIM — `golden_json.rs::
lane_expected_json` rewrites exactly the one `"Set CktModel="` capture line in
the default lane and nothing else in any of the 20+ deck goldens.
`json_ckt_model_divergence_is_pinned` walks the whole directory (skipping the
`gen_schema.py` fixtures with the same `combo_names` filter the existing
completeness guard uses) and asserts the oracle side still carries exactly one
occurrence, that the default lane rewrites exactly that one and the parity lane
none, and that the default-lane expectation contains no value-less form left.
The **import** side is untouched in both lanes, so the oracle's J0 — which
carries the value-less line — still round-trips identically
(`golden_json_import` unchanged and green in both lanes).

**Scope discipline.** One `compat` row for three reproduction sites, entered
under the *Single-site upstream quirks* membership rule. Escape register
unchanged: the **14** truncated physical constants, the **7** F-FMT rendering
markers, `HIDE_015X` ×17; the single-site quirk census drops **13 → 12**, and
14 + 7 + 12 = the 33 remaining markers.

**Two more quirk rows measured and blocked this session — recorded so the next
pass does not re-derive them.** Both have the GIC `%R1` shape (F.3k): the flip is
unambiguous, but it moves a *gated deck's primary physical channel*, so it needs
its own commit with a field-scoped exclusion + a replacement in-engine assertion
+ a transitive cover, and a blanket exclusion would gut the case rather than trim
a field.

* **`load_shape/compute.rs`'s MMF accept-set** (bytes `[46,58)`, dropping sign
  and exponent). `tests/corpus/modes/inputformat/shape_mmf/shape_mmf.dss` was
  *built* to be sensitive to it — its own header says the `pq` shape's P column
  "is written in exponent notation … so P reads as {1.51, 2.01, …} under MMF vs
  {0.15, 0.20, …} without it — a large, oracle-observable divergence in `ld_pq`'s
  power". Honouring sign/exponent therefore moves that deck's load multiplier,
  and through it the whole (small) circuit's voltages — not one field.
* **`support/line_constants/mod.rs`'s `set_user_height_unit` re-conversion.** The
  marker's note ("goldens will pin it when the Line-level HeightUnit/HeightOffset
  slice lands") is stale: that slice landed as WP-U1.4, and
  `tests/corpus/modes/upgrade/upgrade_linecs_heightoffset.dss` now drives exactly
  the `SetHeightOffset` → `SetUserHeightUnit` pair through the equivalent-spacing
  Carson path, so the re-scaled offset is live in that deck's Z/Yc. (Both of the
  marker's two suggested clean fixes reduce to the same thing — leave the stored
  meters value alone — so the fix itself is unambiguous; only its gate cost
  blocks it.)

**The F.3m corpus-scheduler intermittent recurred — and this occurrence pins its
shape.** F.3m saw `519/520 … 1 failed` in the *default* lane with the parity lane
green on the same tree; this commit saw it in the **parity** lane with the
default lane green, and re-running the *same test binary* unchanged gave
`520/520`. So it is lane-independent, change-independent and not reproducible on
a fixed binary — i.e. the `CorpusGuard` / same-directory race F.3m diagnosed
(`electricdss-tst/Test/AutoTrans` writes fixed-name export files, and the
scheduler runs two cases from that directory concurrently), not anything the
engine computes. It is still an **open harness item for the coordinator** — fix
= serialize (or per-case-scratch) same-directory cases in
`corpus_gate::scheduler`. The failing case label was again lost: the panic body
goes to the redirect while only the two `eprintln` summaries survive, which is
its own small fix (print the label from the scheduler, not the panic).

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +2
(`json_ckt_model_divergence_is_pinned`,
`ckt_model_rendered_ordinal_is_lane_split`), 0 removed, 0 new `#[ignore]`.
`git diff -- tests/` touches no golden, tolerance, ledger or deck; `git status
--short tests/corpus` empty after both runs.

### DE_PASCALIZE Stage F.3o — the two CIM `grounded := TRUE` TODOs answered by the writer in the same unit that already answers them (branch `depas-stagef`, 2026-07-28)

The remaining pair of `cim/export.rs` markers, and the one row in this sweep
where upstream did not merely slip: it *wrote the open question down* and shipped
the placeholder. `TODO(compat)` **36 → 34**, and `cim/export.rs` reaches **0**;
both lanes green; no golden, tolerance, ledger or deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `CIM_WYE_GROUNDED_IS_HARDCODED_TRUE` (2 sites) (**torn down by GOLDEN_REBASE G2.1g** — the split is gone, both lanes read the neutral-side node refs) | `BooleanNode('ShuntCompensator.grounded', TRUE)` (`ExportCIMXML.pas:3700`) and `BooleanNode('EnergyConsumer.grounded', TRUE)` (`:4478`), each carrying `// TODO - check bus 2` | `XfmrTankPhasesAndGround` in the **same unit** (`:1531-1570`) already decides `TransformerEnd.grounded` from `NodeRef[j2] = 0`, "last conductor is grounded solidly" | the neutral-side node refs |

**The fix is not an invented semantic — it is the sibling's, applied to where
each class keeps its neutral.** The transformer writer's test is "the winding
terminal's last conductor is node 0". The two shunt classes put that conductor in
different places, and the DSS data model says exactly where:

* a **Capacitor** is a two-terminal element (`Nterms = 2`, `Nconds = Nphases`)
  whose wye point *is* its second terminal — literally the "bus 2" the TODO
  names — defaulting to `.0.0.0`. Default lane: grounded ⟺ every terminal-2 node
  ref is 0.
* a **Load** has one terminal, and `SetNcondsForConnection` gives a wye
  connection `Nconds = Nphases + 1`, so its neutral is that terminal's
  `Nphases+1`-th conductor — the same index the transformer calls `j2`. Default
  lane: grounded ⟺ that node ref is 0.

Both reads reuse the in-tree idiom the transformer port already established
(`cim/power_xfmr.rs`, including its documented pre-solve case): before
`SetNodeRef` has run, `node_ref` is empty and both lanes answer `true`, so an
export issued before any solve is unchanged.

**No golden moves, and that is measured rather than assumed.** No CIM golden deck
and no gated corpus deck contains a wye capacitor with an explicit `bus2=` or a
wye load with a non-ground neutral node, so all 15 CIM goldens stay byte-identical
in both lanes — `F.3n`'s expected-value transform gained no third entry. The
divergence is therefore pinned by a deck built for it,
`cim_wye_grounded_is_lane_split`, which is a **neutral test, not a flipped
constant**: a *probe* circuit ties the capacitor's second terminal to a live bus
and the load's 4th conductor to a grounding reactor's node (both lanes must
disagree, asserted as equality against `compat::ORACLE_PARITY`), and a *control*
circuit is the same feeder with the default ground neutrals (both lanes must
still answer `true`). Its `only_grounded` reader asserts the deck yields exactly
one node of each kind, so a future edit that adds a second shunt fails loudly
instead of reading the wrong one. (**Superseded by GOLDEN_REBASE G2.1g**: the
split is gone and both lanes read the neutral; the pin lives on unconditional as
`golden_cim::cim_wye_grounded_reads_the_neutral`, asserting `false` for both
classes on the probe deck, `true` for both on the control deck and `false` on the
mixed deck G2.1g's audit settlement added to pin the aggregation as `all`. The
"no golden moves" measurement above was re-run from the parity side, which is the
lane that actually moved, and holds.)

**Scope discipline.** One `compat` row for two reproduction sites (the
StorageController precedent of F.3l), entered under the *Single-site upstream
quirks* membership rule. Escape register unchanged: the **14** truncated physical
constants, the **7** F-FMT rendering markers, `HIDE_015X` ×17; the single-site
quirk census drops **15 → 13**, and 14 + 7 + 13 = the 34 remaining markers.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +1
(`cim_wye_grounded_is_lane_split`), 0 removed, 0 new `#[ignore]`. `git diff --
tests/` touches no golden, tolerance, ledger or deck; `git status --short
tests/corpus` empty after both runs.

### DE_PASCALIZE Stage F.3n — the CIM writer's two element-name slips split, with the oracle goldens kept as the source of truth (branch `depas-stagef`, 2026-07-28)

Two `cim/export.rs` markers, both pure **element-name** mistakes whose correct
form a sibling in the very same Pascal unit already writes. `TODO(compat)`
**38 → 36**; both lanes green; no golden, tolerance, ledger or deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `CIM_DELTA_SHUNT_GROUNDED_USES_LINEAR_PREFIX` | the delta arm writes `LinearShuntCompensator.grounded` | the wye arm of the *same* `if` writes `ShuntCompensator.grounded` (`ExportCIMXML.pas:3700` vs `:3706`), and CIM100 declares `grounded` on `ShuntCompensator` — the delta form resolves against no class | `ShuntCompensator.grounded` |
| `CIM_ACLINESEGMENT_G0CH_WRITTEN_AS_B0CH` | the sym-components line writer closes its quartet `bch, gch, b0ch, b0ch` (`:4366-4367`) | the `PerLengthSequenceImpedance` sibling writes `bch, gch, b0ch, g0ch` with exactly these values — and both forms sit 700 lines apart in the *same* golden | `ACLineSegment.g0ch` |

**Neither row re-baselines a golden — the oracle stays the source of truth in
both lanes.** The CIM XML goldens are byte-compared in *both* lanes by the F.2
scoping rule (their writer renders no number through the F-FMT seam), and these
two rows are the first Stage F change that moves their bytes at all.
Regenerating them for the default lane would have surrendered the oracle proof
for 15 files in order to fix three lines. Instead
`golden_cim.rs::lane_expected_cim` applies an **enumerated expected-value
transform** to the oracle text before the byte compare: in the default lane
exactly the delta-`grounded` node and exactly the *second* of each pair of
consecutive `ACLineSegment.b0ch` nodes are renamed, and every other byte of all
15 goldens stays pinned to the oracle. The parity-lane transform is the identity,
so that lane's gate is untouched.

The `b0ch` rewrite is **positional, not value-based** — "the second of two
consecutive `b0ch` nodes" — so a segment whose genuine zero-sequence
susceptance happens to be 0 can never be caught by it. Neither row changes a
value, a node count or an emission order; only two element names move.

`cim_lane_divergences_are_pinned` keeps the list honest in both directions and in
both lanes. It walks all 15 committed goldens and asserts (a) the oracle side is
non-vacuous — exactly one delta-`grounded` node and exactly two duplicated
`b0ch` nodes are still there, so the split cannot rot into dead code if a golden
is ever regenerated; (b) the default lane rewrites exactly those three lines and
the parity lane none; (c) after the transform the default-lane expectation
carries neither quirk form. The engine is then held to that expectation
byte-for-byte by all nine `run_case`/`run_feeder`/fragments cases, so the quirk
cannot drift in either lane.

**Scope discipline.** No IV.2 kernel row was added; both rows enter under the
*Single-site upstream quirks* membership rule (deterministic + defined, clean fix
spelled by a sibling, pinned by a test). Escape register unchanged: the **14**
truncated physical constants, the **7** F-FMT rendering markers, and `HIDE_015X`
×17; the single-site quirk census drops **17 → 15**, and 14 + 7 + 15 = the 36
remaining markers.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +1
(`cim_lane_divergences_are_pinned`), 0 removed, 0 new `#[ignore]`. `git diff --
tests/` touches no golden, tolerance, ledger or deck; `git status --short
tests/corpus` empty after both runs (the `Test/AutoTrans/*` leak deleted by exact
name).

### DE_PASCALIZE Stage F.3m — two more quirks split, and one marker is *re-owned* by F.4 on a measurement that contradicts its own note (branch `depas-stagef`, 2026-07-28)

Three report-surface markers, argued one at a time. Two become lane splits; the
third was implemented, gated, and handed to F-FMT because the flip is a `Show`
**table layout** change, not a value change. `TODO(compat)` **40 → 38**; both
lanes green; no golden, tolerance, ledger or deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `SAVE_CLASS_JOINS_ITS_REPORTED_PATH_AS_STRINGS` | `Save <class>` reports `…\\load` — a doubled separator | the file it names was written through a normalized join all along | the normalized path |
| `SYM_MATRIX_GETTER_RENDERS_ZEROS` | the `?`/Dump text getter of a `DoubleSymMatrixProperty` prints zeros whatever is stored | the **same property's** JSON exporter reads the same array and prints the real numbers | the stored lower triangle |

**The `Save` row moves a string, never a file.** `DoSaveCmd` composes
`SaveFile := SaveDir + PathDelim + SaveFile` as raw strings
(`ExecHelper.pas:835-841`) and `SaveDir` defaults to `OutputDirectory`, which
already ends in a delimiter — so `GlobalResult`/`LastResultFile` hand the caller
`…\\load`, which a Windows consumer cannot open verbatim (`\\` starts a UNC
name). The I/O has always used the joined `PathBuf`, so both lanes write the
identical bytes to the identical place and only the *reported* string differs;
the explicit `dir=` form (the raw parameter) is byte-identical in both.
`save_class_global_result_delimiter_is_lane_split` asserts the reported string
against `compat::ORACLE_PARITY` **and** re-asserts the file at the normalized
location in both lanes, so the split cannot quietly become a file-placement
change.

**The sym-matrix row is the one case where upstream contradicts itself about the
same bytes.** `GetObjPropertyValue`'s `DoubleSymMatrixProperty` arm reads
uninitialized memory, so `? Capacitor.c1.CMatrix` on a bank built with
`cmatrix=(2.8 | -0.6 2.8 | -0.6 -0.6 2.8)` answers `(0 |0 0 |0 0 0 )` — while
`class_props/json.rs`'s arm for that *same property* reads
`darray[(i-1)*Norder + j] / scale` and emits `2.8`/`-0.6` in both engines. So
there is nothing to argue about what was meant. The garbage itself is **not**
reproduced in either lane (reading uninitialized memory is UB, which CLAUDE.md
forbids reproducing); what the parity lane keeps is the deterministic surrogate
the captured goldens hold — a zero matrix of the declared order.

Three `props` goldens carry 33 affected pairs (`cap.json` `CMatrix` ×7,
`fault.json` `GMatrix` ×6, `reactor.json` `RMatrix`/`XMatrix` ×20). They are
**not** dropped: `props_roundtrip::LANE_SKIP_PROP_VALUES` makes the default lane
compare the rendered *skeleton* — the `(v |v v |v v v )` punctuation, the matrix
order, the row split and the number **count** — and skip only the values, which
`exec::tests::compat_quirks::sym_matrix_text_getter_is_lane_split` pins against
`ORACLE_PARITY` instead (asserting the JSON view carries the stored matrix in
both lanes as the corroborating half). The exclusion carries a non-vacuity guard
in *both* directions: the parity lane asserts it skipped **zero** values, the
default lane asserts it saw at least one pair per listed property, so a renamed
property fails the gate rather than silently widening it.

**`max_device_name_length` is an F-FMT row, and its own compat note was wrong.**
The marker claimed the backend's `0` "matters only for the dot-padded (`Paddots`)
reports … the space-padded (`Pad`) reports are token-invariant to it". The flip
was implemented (the honest `Length(Name) + Length(ParentClass.Name) + 1` max)
and gated: **three goldens move** — `show_busflow`, `show_busflow_mva`,
`show_busflow_1ph`, each failing "row 13 field count differs, 8 vs 7". The cause
is `ShowResults.pas:1375`, `Pad(EncloseQuotes(FullName), MaxDeviceNameLength + 2)
+ IntToStr(j)`: `IntToStr` carries **no width**, so at width 0 the terminal
number is *glued* to the name and the golden reads `"Capacitor.cap1"1        0.0`
— one token where the honest width produces two. And because the extra token
shifts every later column, `busflow_seq_policy`'s `col_tol` indices (2 and 6, the
capacitor's near-zero kW and PF cells) would have to be re-calibrated per lane.
Re-laying out a `Show` table and re-baselining its default-lane comparison is
verbatim F-FMT steps 2–3, so the row is handed to F.4 with the measurement
written at the site rather than split here behind a bespoke token patch.

**Scope discipline.** No IV.2 kernel row was added. Escape register: the **14**
truncated physical constants, the **7** F-FMT rendering markers (same size,
swapped membership — in comes `max_device_name_length`, out goes
`export/json/circuit.rs:176`'s `Set CktModel=` `LongBool`, re-read as a
single-site quirk in F.3l), and `HIDE_015X` ×17.
**17** single-site quirk markers remain — the full census of the 38 is
14 + 7 + 17 (F.3l's record said 19 for its own state; the correct figure there
was 20, because the `Set CktModel=` marker it re-read as a quirk had been counted
in the F-FMT set).

**An intermittent in the corpus scheduler, recorded rather than swallowed.** The
first default-lane run of this commit's gate reported `519/520 … 1 failed` and
the *parity* lane was green on the same tree. Re-running the **same test binary**
unchanged gave `520/520`, and the final gate below is green in both lanes — so
this is not a divergence the code can produce, it is non-determinism in the
harness. The mechanism is almost certainly the one the ritual already works
around by hand: `CorpusGuard` snapshots and restores a case's *directory*, and
its registry lets the **last** guard on a directory restore it — but while two
cases from the same directory are in flight the scheduler runs them
concurrently, and the `electricdss-tst/Test/AutoTrans` family writes
fixed-name export files (`Auto3bus_HT_current.txt`, `AutoHLT_LT_losses.txt`, …)
straight into it. Whichever pair overlaps decides which files exist when a case
reads its own export back; the run-to-run leaked-artifact set (this session saw
`AutoAuto_*`/`AutoHLT_*` after one run and `Auto3bus_*` after another) is the
same race seen from the other side. The failing case label could not be
recovered — the panic body was lost in the redirect while the two `eprintln`
summaries survived — so this is logged as an open harness item for the
coordinator, not diagnosed further here: it is orthogonal to Stage F and fixing
it means serializing (or per-case-scratching) same-directory cases in
`corpus_gate::scheduler`.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace --no-fail-fast` and the same
with the feature, including the unconditional 520-case corpus gate. Tests +1
(`sym_matrix_text_getter_is_lane_split`), 1 renamed
(`save_class_global_result_pascal_delimiters` →
`…_delimiter_is_lane_split`), 0 removed, 0 new `#[ignore]`. `git diff -- tests/`
touches no golden, tolerance, ledger or deck; `git status --short tests/corpus`
empty after both runs (the `Test/AutoTrans/*` leak deleted by exact name).

### DE_PASCALIZE Stage F.3l — three more single-site quirks split; the *first* one is a cursor bug the corpus can never see (branch `depas-stagef`, 2026-07-28)

F.3k opened the single-site sweep with five rows and a measured blocker. This
continues it on the same membership rule, with three rows whose clean fix a
**sibling in the very same procedure** spells out. `TODO(compat)` **44 → 40** (the StorageController row carries two reproduction sites);
both lanes green; no golden, tolerance, ledger or deck touched.

| row | upstream | what says it is a slip | default lane |
|---|---|---|---|
| `REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT` (**torn down by GOLDEN_REBASE G2.1d** — the split is gone, both lanes scan them all) | `DoReduceShortLines`' merge-with-parent scan reads exactly ONE parent shunt | the merge-with-**child** branch 40 lines below (`ReduceAlgs.pas:246-258`) spells the same loop with a single cursor | scans them all |
| `STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL` (**torn down by GOLDEN_REBASE G2.1e** — the split is gone, both lanes ask `FleetState <> STORE_IDLING`) | `if not FleetState = STORE_IDLING` is `(not FleetState) = 0` — fires only for `STORE_CHARGING` | the branch it guards calls `SetFleetToIdle` + "force a new power flow" | `FleetState <> STORE_IDLING` |
| `STORAGE_MULTIFILE_USES_THE_PV_PREFIX` (**torn down by GOLDEN_REBASE G2.1f** — the split is gone, both lanes write `EXP_STORAGE_`) | `Export Storage_Meters /m` writes `EXP_PV_<NAME>.csv` | the same command's single-file sibling writes `EXP_STORAGEMeters.csv` | `EXP_STORAGE_` |

**The reduce row is a cross-node cursor mix, and the fix is measured, not
argued.** `ReduceAlgs.pas:200-210` opens the capacitor scan on
`ParentNode.FirstShuntObject()` and then advances it with
`PresentBranch.NextShuntObject()`. The present branch's `TDSSPointerList` cursor
still sits at its last item from tree construction (`Add` sets
`ActiveItem := Result`, `DSSPointerList.pas:88`), so the very first `Next`
overflows and returns `NIL` (`:113-131`): the loop ends after one element and a
capacitor at parent-shunt position ≥ 2 is merged onto another bus instead of
blocking the merge. What makes that *reachable* rather than theoretical is the
shunt list's build order — `build_active_bus_adjacency_lists` fills
`adj.pc[bus]` from `pc_elements` **first** and only then appends the shunt PD
elements (already pinned by `exec::tests::solve`'s `adj.pc[b2] == ["ld1",
"cap1"]`), so a capacitor sharing a bus with *any* load or generator is never
first. Upstream's one-element scan is therefore blind to precisely the elements
it was written to find.

`exec::tests::reduce::short_line_parent_shunt_scan_is_lane_split` builds
`src —lfeed(long)→ b1 —l1(short)→ b2 —l2(short)→ b3` with a load and a capacitor
at `b2` and asserts the outcome against `compat::ORACLE_PARITY`: the parity lane
merges `b2` out, the default lane keeps it. Its control puts the same capacitor
alone at `b2` — first in the list — where **both** lanes refuse the merge, which
is what makes the row a scan-length difference rather than a changed predicate.
(**Superseded by GOLDEN_REBASE G2.1d**: the lane split and this test's
`ORACLE_PARITY` assertion are gone; the pin lives on unconditional as
`exec::tests::reduce::short_line_merge_scans_every_parent_shunt`, on the same
feeder plus a long-`l1` variant that isolates the merge-with-parent arm.)

**The StorageController row is an operator-precedence slip on the raw ordinal.**
Object Pascal binds `not` tighter than `=` and `FleetState` is an `Integer`, so
`if not FleetState = STORE_IDLING` (`StorageController.pas:1350` "Ran out of
OOMPH", `:1619` "Fully charged") evaluates `(not FleetState) = 0`, true only for
`STORE_CHARGING = -1` — the guard fails in exactly the state that reaches those
branches by discharging. `fleet_idle_guard_is_lane_split` pins the predicate over
all three fleet states plus the three complement values (`not (-1) = 0`,
`not 0 = -1`, `not 1 = -2`) that make `Charging` upstream's only firing state.
(**Superseded by GOLDEN_REBASE G2.1e**: the lane split and this test's
`ORACLE_PARITY` assertion are gone; the pin lives on unconditional as
`storage_controller::tests::fleet_idle_guard_fires_unless_the_fleet_is_already_idling`,
on the same three states plus the "Ran out of OOMPH" observable.)

**Why none of the three moves a gated artifact — checked by the gate, not
asserted.** Their lanes diverge only on inputs no gated artifact contains: a
reduced feeder whose parent branch carries a capacitor behind a load; a fleet
that reaches "out of OOMPH"/"fully charged" while discharging; the `Export
Storage_Meters /m` file *name*. The 520-case corpus gate and every byte golden
are unchanged in both builds. The `/m` row's pin was already a Rust-side test
(`export_storage_multifile_prefix_is_lane_split`, renamed from
`…_uses_pv_prefix`); it now asserts the lane's name **and** the absence of the
other lane's name, so the quirk cannot drift in either direction, and it still
compares the produced rows against the oracle-anchored single-file golden in
both lanes. (**Superseded by GOLDEN_REBASE G2.1f**: the `/m` split is gone and
both lanes write `EXP_STORAGE_`; the pin lives on unconditional as
`golden_reports::export_storage_multifile_uses_the_storage_prefix`, asserting
the `EXP_STORAGE_` file present, the `EXP_PV_` one absent, and the same
golden-anchored row compare.)

**Scope discipline.** No IV.2 kernel row was added. The escape register is
unchanged except that F.3k's GIC row keeps its measurement: the **14** truncated
physical constants (register (a) — flipping them is a physical-input change with
its own ledger triage), the **7** F-FMT rendering markers (F.4's scope, one of
which — `export/json/circuit.rs:176`'s `Set CktModel=` `LongBool` — is on
re-reading a single-site quirk, not a rendering one, and moves to the sweep),
and `HIDE_015X` ×17. **19** single-site quirk markers remain.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature —
exit 0 in each, including the unconditional 520-case corpus gate. Tests +2 unit
rows (the reduce lane test and the StorageController lane test), 1 renamed, 0
removed, 0 new `#[ignore]`. `git diff -- tests/` touches no golden, tolerance,
ledger or deck; `git status --short tests/corpus` empty after both runs.

### DE_PASCALIZE Stage F.3k — the single-site upstream-quirk sweep opens: five sites resolved, a sixth measured and blocked (branch `depas-stagef`, 2026-07-28)

F.3's four previous sessions flipped every IV.2 *kernel* row and then stopped,
escaping the whole remainder — 28 single-site upstream quirks — on the reading
that each one "needs a 12th table row the executor may not open". Re-reading the
two documents that actually own the marker population settles that without
inventing anything:

* `PORTING_PLAN.md` §4.1 rule 4, **as updated 2026-07-06**, is plan text that
  defers *every* compat marker to this stage and already prescribes the
  disposition verbatim: "Compat quirks are **not deleted** — each becomes a dual
  kernel behind `#[cfg(feature = "oracle-parity")]`: the default build gets the
  correct/precise implementation, the parity build keeps the quirk so every 1:1
  oracle gate stays permanently re-runnable."
* IV.2's table is closed for **kernels** — the shared arithmetic primitives
  called from hundreds of sites (`cdiv`, the dense inverse, `round_i32`, `PI`).
  A single-site behavioral bug is not a kernel, and the machinery its clean fix
  needs already exists and has been exercised four times (`Iresidual`,
  `Bus_Int_Duration`, Monitor `BaseFrequency`, Newton stale `Iterminal`).

So the sweep resumes on the pattern those four established. `TODO(compat)`
**49 → 44**; both lanes green; no golden, tolerance, ledger or deck touched.

**Membership rule, recorded in `compat.rs` so the section cannot become a
dumping ground.** A site qualifies only if (i) upstream's behavior is
deterministic and defined — UB is never reproduced in *either* lane, per
CLAUDE.md; (ii) the clean fix is unambiguous, because the Pascal itself, a
sibling class, or the source's own comment says what was meant; and (iii) the
divergence is pinned by a test rather than narrated. Each row carries an
expected-value test asserted against `compat::ORACLE_PARITY`, so the test is
load-bearing in **both** builds rather than a default-lane-only claim.

| row | upstream | Pascal | default lane |
|---|---|---|---|
| `ISOURCE_BUS2_NEVER_LATCHES` | `Bus2Defined` never set, so a later `Bus1=` clobbers an explicit `Bus2` | `Isource.pas:221` has no `Bus2` case; `Vsource.pas:498` does | latches, like the sibling class |
| `CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL` | `Like=` on a Follow CapControl silently loses its signal | `CapControl.pas:446-490` copies every *other* reference | copies it too |
| `LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING` | `Like=` keeps `Create` defaults for the five equivalent-spacing fields while the copied `PrpSequence` marks them set | `TLineSpacingObj.MakeLike` copies only NConds/NPhases/FX/FY/Units | copies them |
| `LINECODE_SYM_CLEAR_OMITS_C0` | `c0=` alone neither selects the sym model nor clears matrix tracking — and `C0` only *displays* on the sym model, so the written value does not even read back | the source flags its own omission with `-- Missing?` | `C0` behaves like its seven siblings |
| `SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING` | a non-positive rating leaks raw into the `%Normal`/`%Emergency` columns (`normamps=-1` prints `-1`) | `ExportResults.pas:409-414` seeds `iNormal := NormAmps`, overwriting only when `> 0` | prints `0` |

**One committed golden moves, by exactly one value.** The props-roundtrip
scenario `isource_bus2_clobbered_by_bus1` (`New Isource.i1 bus2=b2 bus1=b1 …`)
exists to pin the Isource quirk, so the default lane now reads `b2` where the
oracle wrote `b1.0.0.0`. That single `(scenario, property)` pair is excluded in
the **default lane only** (`props_roundtrip::LANE_SKIP_SCENARIO_PROPS`); the
parity lane still compares it, both lanes still compare every *other* property
of that scenario and all five sibling Isource scenarios, and the exclusion
carries a stale-entry guard that fails the gate if the scenario or the property
is ever renamed away. No tolerance moved and no golden file was rewritten.

**The sixth candidate was flipped, gated, and reverted — the measurement is the
point.** GICTransformer's `G2 := 100.0/(FZBase2 * FPctR1)`
(`GICTransformer.pas:441`, copy-pasted from the `G1` line above) silently
ignores a user's `%R2`, and the site had named the clean fix since the port. It
was implemented and put through the 520-case gate, where it failed two cases:
`asymmetric/gic/gictransformer_gic.dss:18` builds `GICTransformer.tg3 … %R1=0.2
%R2=0.15`, so honouring `%R2` moves that deck's GIC current **4.50e-4** against
the `capi_v0145` oracle (allowed 1.00e-6), and `gic/gic_midi.dss` **1.02e-4**
(allowed 1.07e-6). Both gating oracles reproduce the quirk. Landing it therefore
costs those two decks' *primary physical channel* in the default lane — the same
shape as the Newton row and owed the same treatment (a field-scoped exclusion, a
replacement in-engine assertion, a transitive cover), which is a commit of its
own rather than a line in a batch. Reverted; the marker stays, now carrying the
measurement, and the reproduced value is pinned in **both** lanes by
`exec::tests::compat_quirks::gic_transformer_g2_reproduces_the_pct_r1_bug` —
which also fails loudly if someone "fixes" it without doing the exclusion work.

**Why the other four move nothing — checked by the gate, not asserted.** Their
lanes diverge only on inputs no gated artifact contains: `Like=` on a
Follow-type CapControl; a LineSpacing clone read before its next recalculation;
a LineCode edit whose **only** sequence property is `C0`; an element with a
non-positive `normamps`. The 520-case corpus gate and every byte golden are
unchanged in both builds. Two of them also pin the *unaffected* neighbour
explicitly — the `Export SeqCurrents` test rates a second line positively and
pins the ordinary percentage path in both lanes, and the LineCode test pins `C1`
(the sibling the Pascal *does* list) selecting the sym model in both lanes,
which is what makes `C0`'s omission a slip rather than a rule.

**Two rows are pinned at their behavioral consequence, not just at the field.**
The CapControl test samples the cloned control and asserts that upstream's clone
*aborts the solve* (a Follow CapControl with no signal raises 10362) while the
default lane's clone follows the signal and arms a CLOSE. The LineCode test
asserts the property surface, where upstream's `c0=` renders as `----` because
the object never left the matrix model.

**Scope discipline.** No IV.2 kernel row was added, and the unresolved
population keeps its recorded escapes unchanged: the **14** truncated physical
constants (register (a) — their 5.4e-4-class distance from the exact constant is
*above* the calibrated oracle floors, so flipping them is a physical-input
change belonging to an UPGRADE rung with its own ledger triage, not a Stage F
lane flip), the **7** F-FMT rendering markers (F.4's defined scope), and
`HIDE_015X` ×17 (retiring it needs a byte-golden re-baseline against a different
oracle, which the parity lane may never do — `DIVERGENCES.md`'s settled
disposition is "retained deliberately"). **23** single-site quirks remain, GIC
now the best-specified of them.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature —
**2262 passed / 0 failed / 5 ignored in each**, including the unconditional
520-case corpus gate. Tests +5 (2257 → 2262), 0 removed, 0 new `#[ignore]`.
`git diff -- tests/` touches no golden, tolerance, ledger or deck (only
`tests/props_roundtrip.rs`'s lane exclusion and one new test body in
`tests/golden_reports.rs`); `git status --short tests/corpus` empty after both
runs — the known intermittent `Test/AutoTrans/*` leak deleted by exact name.

### DE_PASCALIZE Stage F.3j — the **Newton stale-`Iterminal`** row is flipped: the CLAUDE.md named-bug set is now closed 4/4 (branch `depas-stagef`, 2026-07-28)

F.3c flipped three of the four *reproduced* CLAUDE.md upstream bugs and left the
fourth — bug 5, the post-Newton `Powers`/`Losses` staleness — in the escape
register. That was the only asymmetry left in the sanctioned inventory (the
brief's "upstream-bug clean fixes that plan text or CLAUDE.md explicitly defers
to Stage F"), and the site's own comment named the resolution shape years ago:
"Eliminating it is a de-compat DECISION… path (b): convert the `newton*`
Powers/Losses compare to a documented live-gate exclusion plus the replacement
Newton assertion." Done, measured, in both lanes. `TODO(compat)` **50 → 49**.

**The quirk, re-verified in-tree.** `DoNewtonSolution`'s per-iteration
`SumAllCurrents` calls `compute_iterminal` and so stamps every element's
`Iterminal` for the live `SolutionCount` — from the *pre-final* guess
`NodeV_{n-1}`, because `solve_system_newton_step`'s `NodeV -= dV` follows it. The
cache-aware `Get_Powers`/`Get_Losses` then read that stale current while
`Currents` recomputes fresh, i.e. upstream reports `S ≠ V·conj(I)` for the same
element in the same read.

**Both sides measured on `modes/newton/newton.dss`** (Newton vs the normal
algorithm on the same deck — same voltages to 7.6e-12 V, same 2 iterations, and
the normal solve leaves *no* valid cache, so its powers are fresh by
construction):

| quantity | parity lane | default lane |
|---|---|---|
| worst per-conductor \|ΔS\| | **5.283e-1 kVA** (`Vsource.source`) | 3.256e-11 kVA |
| worst \|Δlosses\| | **6.248e2 W** | 3.329e-8 W |
| worst \|ΔI\| (control) | 4.547e-12 A | 4.547e-12 A |

Ten orders apart, and the currents row is the control that makes it a *bug*
rather than a convention: `Currents` was always fresh, in both lanes.

**Against the oracle**, the default lane's fix reads 4.86e-4 kVA off
`capi_v0145` on `newton.dss` and 2.46e-3 kVA on `newton_feeder.dss` (both
`Vsource.source` conductor 0) — ~63× and ~35× their tiers' floors, i.e. loud, not
drift. Those two decks' **powers/losses only** are therefore excluded in the
**default lane only** (`tests/harness/lane.rs::LANE_SKIP_ELEM_POWERS` →
`ElemChannels`, consumed by `corpus_gate::runner`); the parity lane still
compares both channels against both gating oracles, and in *both* lanes the
element name set, terminal currents, node voltages, system Y, discrete state and
iteration count of those cases stay fully gated. No tolerance moved anywhere.
The exclusion is *value-only* by construction: `compare_element_channels`
asserts the element's presence **and** its conductor counts under every channel
policy, so dropping a value channel can never excuse a missing element or a
shape mismatch.

**The replacement dispatch signal is stronger than the one given up.** Newton and
the normal fixed point agree on voltages and iteration count on these decks, so
the staleness was the ONLY channel there that would notice `algorithm=Newton`
silently falling back to `DoNormalSolution` — an inference from a reported power.
`exec::tests::newton::newton_dispatch_leaves_a_valid_but_stale_iterminal_cache`
asserts the property at its source instead, in **both** lanes: after a Newton
solve `Line.l1`'s `Iterminal` is *marked solved for the live `SolutionCount`* and
sits 7.378e-2 A off a recompute at the converged `NodeV`; after a normal solve it
is not marked at all (measured: the normal run's cache is a full 82.8 A off,
i.e. never stamped — which is also why no other algorithm can exhibit the quirk).
That is a direct assertion that `DoNewtonSolution` ran.

**Transitive cover for the excluded channel.**
`newton_powers_are_the_lane_kernel` pins the default lane's Newton powers to the
*normal* algorithm's on the same deck at 1e-8 kVA, and the normal algorithm's
powers are oracle-gated on ~500 other corpus cases — so the excluded channel is
still bounded by the gate, one step removed.

**Scope discipline.** No IV.2 table row was invented: this is the second
CLAUDE.md-deferred named bug (after Monitor `BaseFrequency` in F.3c), recorded in
`compat.rs`'s inventory exactly the way F.3c recorded that one. With it the
named-bug set is closed: two of the six were never reproduced (VSConverter,
harmonics `Powers`-after-`Currents`) — as is the OOB half of `Bus_Int_Duration` —
and all four that are reproduced now carry the parity/default split. CLAUDE.md's
bug list was corrected accordingly (its `Iresidual` and `Bus_Int_Duration`
bullets still pointed at `TODO(compat)` markers F.3c had already removed).

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy --workspace
--all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature —
**2257 passed / 0 failed / 5 ignored in each**, including the unconditional
520-case corpus gate. Tests +20 (2237 → 2257: 2 new engine tests + the one new
`harness::lane` test, which compiles into each of the 18 integration binaries),
0 removed, 0 new `#[ignore]`. `git diff -- tests/` touches no golden, tolerance,
ledger or deck (only `tests/harness` + `tests/corpus_gate` code);
`git status --short tests/corpus` empty after both runs — the known intermittent
`Test/AutoTrans/{Auto1bus_HT_current,AutoAuto_HL_current}.txt` leak deleted by
exact name. Non-vacuity of the new exclusion is measured, not assumed: with the
flip in and the exclusion out, both `newton*` cases FAIL the default-lane gate
(4.86e-4 > 7.75e-6 allowed; 2.46e-3 > 7.04e-5).

**Still escaped, unchanged from F.3i**: the 7 F-FMT markers (F.4's scope), the 14
truncated physical constants, the 28 remaining single-site upstream quirks (each
needing a 12th table row the executor may not open), and `HIDE_015X` ×17. `rg
"TODO\(compat\)" crates` = **49**.

### DE_PASCALIZE Stage F.3i — IV.2 **row 2** (dense inverse) is settled: *no split*, with the amplification decomposed to its root (branch `depas-stagef`, 2026-07-27)

F.3f left the row blocked with a recommendation: "re-attempt with a faer-backed
LU inverse and re-measure the six cases". That recommendation is **withdrawn on
evidence** — the disqualifier is not specific to the candidate kernel, it
applies to *any* kernel that differs by a single ULP. The row therefore resolves
the way rows 1 and 3 did: **one shared kernel, no `cfg` at all**.
`TODO(compat)` **55 → 50**; both lanes stay byte-identical (the alias already
selected this impl in both).

**Measurement 1 — the surface is the ideal switch, not the transformer, and the
gap is exactly one ULP.** F.3f/F.3h had attributed the 1.45 reading to
transformer `YPrim`. Bisected: with the flip still selected everywhere, pinning
**only** `pd/line/solve.rs`'s series-impedance inversion back to the parity
kernel restores `Line.low`'s exact `0` on `Test/AutoTrans/Auto1bus-step1.dss`.
Dumping that matrix under both kernels: a `switch=yes r1=1e-6` line has
`Z = 1.00000000000000006e-9·I`, inverted to `1.00000000000000000e9` (parity) vs
`9.99999999999999881e8` (candidate) — **1 ULP apart, and the candidate is the
closer of the two** to the `Decimal`-60 exact reciprocal
`999999999.99999993771…` (5.69e-8 vs 6.23e-8). Being *more* accurate is exactly
what it is: irrelevant to the gate.

**Measurement 2 — one ULP there is worth 14.5× the calibrated floor, linearly.**
Perturbing the parity inverse of that one entry by −1 ULP reproduces the flip's
reading to nine digits (`1.45235132836994740` vs the flip's
`1.45235132964843072`). The chain, end to end: 1 ULP on a 1e9 S switch
admittance → 1.56e-11 V of split across the switch (≈1 ULP of the 92.95 kV node)
→ `y·ΔV` = 1.5625e-2 A → `V·ΔI` = 1.4524 kW, against the
`large_near_ideal_source` tier's `1e-1`. The gain **is** the family: an ideal
switch turns one ULP of node voltage into 15 mA, and TOLERANCE_NOTES
§near-ideal-source calibrated that floor by decomposition *from a bit-identical
Y*. (The `+1` ULP direction leaves the split at exactly `0` — a knife-edge, so
roughly half the perturbation directions trip it; that is the drift model's
"knife-edge discrete flip" in its purest form.)

**Disqualifier 2, independent and semantic.** The two kernels also disagree
about what a *singular* matrix leaves behind — parity leaves it partially
transformed (Pascal), the candidate restores it — and three call sites consume
precisely that: `solution::fault_study::compute_ysc` stores the transformed
`Ysc` and `compute_isc` multiplies `VBus` through it (how `Isc` mirrors the
upstream value), `report::{show,export}::fault_study` do the same with `Yfault`,
and `line_constants` inverts `FYc` "ignoring singularity like Pascal does".
Flipping would silently change those reports on a degenerate bus. So the
partial transformation is load-bearing semantics, not a wart — which also
retires the `TODO(compat)` that proposed "restore-or-zero on failure": that
"clean fix" is wrong.

**Pinned, not narrated.** `compat::tests::dense_inverse_kernels_differ_by_one_
ulp_on_an_ideal_switch` asserts the 1-ULP gap with literals and that the alias
resolves to the oracle's value (in either lane);
`tests/compat_dense_inverse.rs::dense_inverse_keeps_the_ideal_switch_split_at_
exactly_zero` pins the consequence at the physical boundary with **zero**
tolerance — the tripwire for any future kernel swap, failing next to the
documented reason instead of as an unexplained corpus divergence, with a
non-vacuity assertion that the deck is energized.
`invert_aliases_are_unflipped_in_both_lanes` is renamed
`invert_is_one_shared_kernel_in_both_lanes` to match the settled verdict.

**Every row of the closed table now has a verdict.** Flipped: FPC `Round`
(F.3a), single-point stddev (F.3b), `Iresidual` / `Bus_Int_Duration` / Monitor
`BaseFrequency` (F.3c), RPN pi (F.3d). *No split*, each measured: complex
division (F.3e), sym components (F.1), Y triplet dedup (IV.1), dense inverse
(F.3i). Owned by a later plan: solver execution (M3c / WP-R1), report rendering
(F.4 / F-FMT). **Nothing in IV.2 is "pending" any more.**

**Four more markers resolved, none by relabeling** (55 → 50): the two
dense-inverse ones above (`compat.rs`'s singular-pivot note — the proposed fix
disproven; `cmatrix::kron`'s unchecked zero pivot — nothing inexact is
reproduced, and the guard it asked for cannot be expressed through an `Option`
that already means "shape error", so it belongs to the P5/miette rung) and the
three `exec/plot.rs` option-parsing ones: the first-letter `type=` dispatch
(IV.1's abbreviation matching), the empty `else` (deck-language leniency →
IV.1b layer 2, opt-in, after Stage F) and `MinScaleIsSpecified` for `min=0`
(the flag answers "was `min=` given?", which is true; both flags are fields of
the plot-callback JSON — an IV.1 interface contract with the external plotter).

**The remaining 50, classified — the closing F.3 register.** Every site now has
a recorded disposition; nothing is unexamined.

* **7 → F.4 (F-FMT), by the plan's own scope**: `util::fmt_g`'s two-stage
  rounding, `report/format::fixed_w_fpc`, `show/diagnostics`'s `%-.g`,
  `show/mod`'s `MaxDeviceNameLength = 0` (unpadded device columns),
  `json/mod`'s fpjson float + `NL`, `json/circuit`'s `%g`/`%.4g`/`%8.2f` family
  header. F.3 **cannot** reach `rg TODO(compat) = 0`; F.4 owns these seven.
* **14 truncated physical constants — ESCAPED with the measurement** (register
  (a) below): `CALPHA` ×2, the `complexutil` pi / rad→deg pair + `pascal_atan2`,
  `658.5` ×3, `MU0`/`Twopi`/`E0`, the FPC `csqrt`/`cmod`/`cln` forms, the
  tape-shield `1/pi`, `1732.0`, `0.001732`, `ln 10` ×2. Their relative gaps
  (up to 5.4e-4) are *physical-input* changes, orders **above** the 1e-6-class
  oracle floors, so a default-lane flip breaks drift-model row 1 by
  construction. Their home is an UPGRADE-style rung with per-case ledger triage
  and a deliberate re-baseline, not a Stage F lane flip.
* **29 single-site upstream quirks — ESCAPED, each needing a lane branch the
  closed table does not sanction.** CIM `grounded := TRUE` ×2, the delta
  `LinearShuntCompensator.` prefix slip and the `b0ch` double write;
  CapControl/LineSpacing/Storage `MakeLike` gaps; Relay's unconditional "Debug
  Sample" line and its `Recloser.<name>` labels; StorageController's `not
  FleetState = STORE_IDLING` precedence ×2; LineCode's `C0` omission; the
  LoadShape MMF accept-set; Monitor's `[0.0]` channel placeholder (which
  reproduces *dss-python's wrapper*, not the engine); Generator's
  `PrpSequence[26]/[27]` ordinals and the Model=6 stale-`Vterminal` seed;
  Isource's `Bus2Defined`; Load `makeposseq`'s `/3.0`; Capacitor's dropped `Cuf`
  write; Fault's `MinAmps` double-print; GICTransformer's `G2` off `%R1`;
  `reduce.rs`'s parent-shunt cursor bug; the `EXP_PV_` Storage prefix; `Save`'s
  doubled delimiter; the Newton stale-`Iterminal` split; the
  `DoubleSymMatrixProperty` getter; SeqCurrents' raw non-positive rating; the
  `CktModel=` `LongBool` empty string; `line_constants`' `UserHeightUnit`
  re-conversion. Each one's clean fix changes a value or a string that a
  committed golden or a gated corpus case compares, so landing it needs a
  per-site default-lane branch — i.e. a **12th table row** ("upstream
  single-site behavioral bugs: parity reproduces, default fixes", with
  per-site expected-value tests and the field-scoped exclusions F.3c already
  built: `GateSpec::ColAbove`, `LANE_SKIP_PROPS`). The executor may not open
  that row; the owner decides it in one pass.

**The "one sanctioned default-lane re-baseline" has nothing to re-baseline yet
— by measurement, not by omission.** Every F.3 flip so far is byte-neutral on
every committed golden: `Round` moves only out-of-range deck literals, stddev
only `npts=1` shapes (no golden has one), the RPN pi only one oracle-golden
record (pinned by expected value, not re-baselined), and the three bug fixes are
field-scoped exclusions plus expected-value tests. Both lanes still produce
identical bytes on the whole golden corpus, so a default-lane self-golden set
would be a copy of the committed one. Its trigger is F.4: F-FMT is what makes
default-lane rendering differ, and the plan already sequences the re-baseline
there ("re-layouted reports get default-lane self-goldens").

**`HIDE_015X` ×17 — ESCAPED, and now with the proof rather than a preference.**
Driving `rg HIDE_015X` to zero needs one of: (i) exposing the four deferred
props (`Line.EpsRMedium/HeightOffset/HeightUnit`, `Line`/`LineGeometry`
`Conductors`) in **both** lanes — which changes the *structure* of the
0.14.5-pinned Dump / FULL-JSON / `Dump commands` byte goldens, i.e. a parity-lane
re-baseline, forbidden forever; (ii) deleting the props — losing ported r4133
behavior; or (iii) making the flag lane-conditional — a 12th table row plus a
default-lane structural golden change that F.4's parsed-numeric comparator does
not cover (it compares structure, only rendering is allowed to move). The
settled decision in `DIVERGENCES.md` §"Line/LineGeometry Conductors" is
"retained deliberately" under UPGRADE_PLAN §1.4's own fallback; the real
successor is an UPGRADE rung that teaches `gen_json.py` an engine switch and
re-pins the surface — exactly what that section measured as disproportionate.
Six of the 17 matches are already documentation *about* the flag, not uses.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature
— **2237 passed / 0 failed / 5 ignored in each**, including the unconditional
520-case corpus gate. `git diff -- tests/` empty (no golden, tolerance, ledger
or deck touched); `git status --short tests/corpus` empty after both runs — the
known intermittent `Test/AutoTrans/*` leak deleted by exact name. Tests +2
(2235 → 2237), 0 removed, 0 new `#[ignore]`. All probe instrumentation
(`pd/line/solve.rs`, `pd/transformer/yterminal.rs`, the throwaway probe test)
was reverted in full before the gate — the committed diff is documentation, two
pins and four marker resolutions.

*(Process note, repeat offence: a failed gate run was still alive when the next
one started, and the two overlapped in this worktree — the default-lane count
came out as 1371 from a truncated log. Same hazard as F.3b's note: **one gate at
a time per worktree**, and re-run rather than trust a count whose log was
contended.)*

### DE_PASCALIZE Stage F.3h — the dense-inverse blocker's open question is **answered**: (b) is disproven corpus-wide (branch `depas-stagef`, 2026-07-27)

F.3f blocked IV.2 **row 2** on a named, unanswered probe: *"does `zb.invert()`
return `Err` on this deck under either kernel? Run it before the row is
reconsidered."* Run — and widened from that one deck to the whole gate.

**Method.** Both `zb.invert()` call sites (`elements/pd/transformer/yterminal.rs`,
`elements/pd/auto_trans/yterminal.rs`) were temporarily instrumented to invert
every `Zb` with *both* compiled kernels and report `is_err()` plus the worst
relative element gap; the 520-case corpus gate was then run end to end. The
instrumentation is throwaway and was reverted in full (`git status` clean, the
committed diff is documentation only).

**Result — 495 872 `Zb` inversions, ZERO `Err` from either kernel.** The Pascal
error-117 branch (`zb.clear()` + `ε·I`) never fires on any gated deck under
either kernel, so the two lanes build the **same circuits**; hypothesis (b), "a
singular-pivot branch flip makes the lanes different models", is dead. The two
inverses agree to at most **1.01e-15** relative corpus-wide (**4.465e-16**, ~2
ULP, on `Test/AutoTrans/Auto1bus-step1.dss`'s own 2×2 `Zb` — the deck that broke
the floor).

**What that changes.** The 1.4523513296 W reading is hypothesis **(a)**: ~2 ULP
of `YPrim` difference amplified by the deck's κ≈1e12 path. The row stays
blocked — F.3f finding 2 (five cases 1.4–1.8× past calibrated floors) is
untouched, and `TOLERANCE_NOTES` §near-ideal-source still proves `i_abs = 0.1`
from a *bit-identical* Y that this kernel would destroy. But the blocker is now
a purely **numerical** one, so the next attempt is a faer-backed LU inverse plus
a re-measurement of the six cases, **not** a semantics investigation. Recorded
where it will be read: `compat.rs`'s "Dense inverse" section now carries the
measurement in place of the open question.

*(Method note worth keeping: the probe is the cheap general shape for any
"does this kernel flip a discrete branch?" question — compile both kernels at
the call site, run the corpus gate once, count `Err` disagreements. It converts
an argument into a number for ~2 minutes of wall clock.)*

### DE_PASCALIZE Stage F.3g — the marker population is re-audited: **89 → 55**, and the tag becomes a *gated* index (branch `depas-stagef`, 2026-07-27)

The F.3 register below counted the remaining `TODO(compat)` markers but had to
admit that the number was not trustworthy: a fifth of the population was prose
*about* the tag rather than sites carrying it. This commit fixes the index and
the sites whose marker premise turns out to be false. **34 markers are resolved
and not one line of engine behavior changes** — both lanes stay byte-identical
(doc-only diff outside the new test).

**(1) 24 occurrences were never reproduction sites.** Prose cross-references
("see the `TODO(compat)` at …"), continuation lines of a marker two lines above,
and — worst — **negative** mentions whose text says *"this is NOT one"*
(`exec/plot.rs`, `exec/diakoptics/matrices.rs`, `solution/time_series.rs`,
`control/roll_avg_window.rs`, `pc/generator/dynamics.rs`,
`pc/ind_mach012/{mod,dynamics}.rs`). Plus the JSON `circuit.rs` `%g` / `%-.4g` /
`%8.2f` block, which now carries **one** marker in the module doc (its F-FMT
family header) instead of four. All reworded to "compat-tagged at …"; the
information is kept, the tag is not.

**(2) 10 sites whose marker premise is false**, argued one at a time against the
Pascal — each becomes plain documentation, none becomes a flip:

* `mathutil::SymComp::official` — IV.2 **row 3** was already settled as *no
  split* under F.1 (`mathutil.pas:548` ends with `SelectAs2pVersion(False)`, so
  the pinned oracle uses `precise`; `official` is reachable upstream only via the
  `DSSCompatFlag.BadPrecision` env flag nothing sets). So it is not a site
  awaiting a fix — it is the compiled comparison partner that keeps the row's
  verdict *asserted*, exactly like `compat::cdiv_std_impl`. Deleting it, as the
  old marker proposed, would delete the evidence.
* `pc/generator/accessors.rs::variable_name` — upstream's
  `UserModel.FGetVarName` mis-dispatch is **UB** (nil function-pointer deref, or
  an uninitialized stack read out of range); per CLAUDE.md the port deliberately
  does **not** reproduce it. A compat marker on something we refuse to reproduce
  is backwards.
* `pc/storage/dynamics.rs` `OFFVal` — same class: an indeterminate FPC local,
  not reproduced (0.0), and unreachable in the gated corpus anyway.
* `exec/reduce.rs` `TotalLen := Len/2` — writes a **local that nothing reads**,
  on a branch that updates no property: the upstream approximation has *no
  observable effect in either engine*. Nothing to reproduce, nothing to fix.
* `general/line_code/mod.rs` `hrs_to_repair` — we store what the oracle's getter
  actually reports (`0.0`), i.e. this **matches the oracle exactly**. What is
  left is dead-field hygiene (the field has been unused upstream since 2014),
  not a compat divergence.
* `cim/export.rs` `bAllowSec` — `LoadClass <= 1` is the exporter's *rule* for
  "this load may be a PNNL-taxonomy secondary": a convention with no named clean
  fix, not an inexactness. Changing the threshold would change which loads the
  profile calls secondary, i.e. the exported model.
* …plus 4 duplicate/continuation markers folded into the site they belong to
  (`generator/accessors.rs`'s inline `PrpSequence` note, `relay/mod.rs`'s second
  `Recloser.<name>` log line, `line_constants`' `TWOPI` allow-attribute,
  `json/circuit.rs`'s `%g` helper doc).

**The tag is now enforced, not merely conventional.** New CI gate
`oracle_parity_cfg_gate::compat_tag_is_only_ever_a_marker_never_prose`: every
occurrence in `crates/*/{src,tests,benches,examples}` must sit inside a comment
and be followed immediately by `": "`. That is CLAUDE.md's "it must stay
greppable" rule made mechanical — the count that Part IV.2 drives to zero can no
longer be inflated by prose. The gate file spells the tag only at runtime, so it
does not trip itself, and it asserts the population is non-empty so a broken
source walk cannot make it pass vacuously.

**The 55 that remain, by bucket** (re-enumerated site by site, not estimated):
**14** truncated physical constants (`MU0`, `Twopi`, `CALPHA`×2, `658.5`×3,
`ln 10`×2, `1/pi`, `1732.0`, `0.001732`, the `complexutil` pi/rad→deg pair with
`pascal_atan2`, the FPC `csqrt`/`cmod`/`cln` forms) — escaped with the
measurement in register **(a)**; **7** F-FMT rendering markers — F.4's defined
scope; **2** blocked with the dense-inverse row (F.3f); **32** single-site
upstream quirks whose clean fix would change a value or a string that a
committed golden or a gated corpus case compares, i.e. needs a per-site lane
branch that the *closed* IV.2 table does not sanction — register **(d)**, owner
decision required. Nothing was silently dropped and no table row was invented.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature
— **2235 passed / 0 failed / 5 ignored in each**, including the unconditional
520-case corpus gate (133.8 s). `git diff -- tests/` empty (no golden,
tolerance, ledger or deck touched); `git status --short tests/corpus` empty
after both runs — the known intermittent `Test/AutoTrans/*` leak deleted by
exact name. Tests +1, 0 removed, 0 new `#[ignore]`.

### DE_PASCALIZE Stage F.3f — the dense-inverse row: flip attempted, **measured, and blocked** (branch `depas-stagef`, 2026-07-27)

IV.2 **row 2** was flipped in the working tree, the whole suite + the 520-case
corpus gate were run against it, and the result says the flip is not the
ULP-level change the drift model assumes. The alias is therefore back to the
parity kernel in both lanes (bit-neutral; nothing shipped moved) and the
measurement is recorded — in `compat.rs`'s module header and here — so the row
is now *blocked by named findings* instead of merely pending.

**Finding 1 — the flip blows through a floor that was built for exactly this
amplification.** `Test/AutoTrans/Auto1bus-step1.dss`: `Line.low` conductor-0
power reads **1.4523513296 W** where the oracle has exactly `0` — ~14× over the
1e-1 allowed at |V| = 1 kV. What makes this the loudest of the six is *which*
floor it breaks: that case runs on the `large_near_ideal_source` tier, whose
`i_abs = 0.1` was calibrated **by decomposition**
(`tests/TOLERANCE_NOTES.md` §near-ideal-source) against this very effect —
κ≈1e12 from `mvasc3=2e6` + `r1=1e-6 Ω` switches + a floating delta tertiary,
where a single 1-ulp RHS component (1.5e-11 V) propagates linearly into the
0.15 A no-load currents. And that tier's own note says the family's "unique
surface is YPrim assembly" — the exact surface this row changes. So the
candidate kernel is not just noisier; it is noisier *past a band already
widened for this phenomenon*.

Sharper still: that floor's **proof** rests on a property the flip destroys.
§near-ideal-source establishes the band by showing the assembled system Y is
"**BIT-IDENTICAL** across engines on every family deck at every probed stage",
so the whole residual gap is one-shot cross-solver junk on identical `(Y, I)`.
Flipping the inverse changes the transformer `YPrim`, i.e. Y is no longer
bit-identical — the decomposition that justified `i_abs = 0.1` no longer
applies to the default lane at all. Whoever lands this row has to redo that
proof, not just re-measure the number.

The **mechanism is still not proven**, and the two candidates want opposite
responses, so it is recorded as an open question rather than diagnosed
(CLAUDE.md: prove cause before concluding):

* **(a) more solver junk down the same κ≈1e12 path** — a kernel-quality /
  floor question, consistent with Finding 2's direction.
* **(b) a singular-pivot branch flip** — the transformer-family `Yprim`
  builders map `Err(SingularMatrix)` to Pascal error 117 and substitute `ε·I`
  for `Zb`. If this deck's `Zb` is one the diagonal-only kernel rejects and
  partial pivoting inverts, the two lanes build *different circuits*. Under (b)
  it is a semantic divergence needing its own decision — plus the prior
  question of which engine is right, since Pascal never exchanges rows and so
  can reject an invertible matrix.

One probe separates them: does `zb.invert()` return `Err` on this deck under
either kernel? Run it before the row is reconsidered. **Do not** answer this by
widening `large_near_ideal_source` — that band is decomposition-proven against
the parity kernel, and the tier note already says a future trip is "a re-triage
signal, NOT a widen-the-band signal".

**Finding 2 — five cases 1.4–1.8× past their calibrated floors.**
`Transformer.sub1` currents on `EPRITestCircuits/ckt7` (both drivers) and
`Examples/StoCtrl_Current_PeakShave` (1.47e-4 / 1.70e-4 vs 1.06e-4 / 1.15e-4),
the `IEEE_519` Y-fingerprint `trace.im` (2.89e-5 vs 1.64e-5), and `4Bus-YYD` on
the **r4133** channel (2.03e-4 vs 1.34e-4). Not fudgeable — CLAUDE.md forbids
widening a floor to pass — and the *direction* is itself evidence: the candidate
kernel is a textbook Gauss-Jordan on `[A | I]`, roughly twice the arithmetic of
Pascal's in-place variant, so it is plausibly **noisier** on well-conditioned
impedance matrices even while being more *stable* on ill-placed ones. A default
kernel worth flipping to is probably an **LU solve (faer)**, not this
hand-rolled GJ. Recommendation: re-attempt the row with a faer-backed inverse
and re-measure these five before touching anything else.

Also moved (both expected, both cheap once the row lands):
`transformer_yprim_bitexact` by 3 ULP — it becomes a lane-split pin — and
`golden_reports::export_currents` row 20 column `AngResid1`, where a residual
current the oracle reports as exactly `0` comes out a hair *negative*, so its
angle renders `180.00` instead of `0.00` (the report's residuals are ~1e-11 A;
the same class as the `("Transformer", "WdgCurrents")` zero-magnitude-angle
exclusion already in `harness::skip_prop`).

**What the attempt did leave behind — a real fix.**
`invert_partial_pivot_impl` normalized its pivot row with `num_complex`'s `/`,
which on a real-valued matrix computes `x·c/c²` rather than `x/c`; that drifted
the complex kernel one ULP away from its real twin and
`mathutil::tests::etk_invert_matches_cmatrix_invert_on_real_matrix` (a plain
`assert_eq!`) caught it. It now divides through `compat::cdiv` — Smith's, which
F.3e had just measured as the more accurate kernel and which reduces *exactly*
to the real division when `im == 0`. The two kernels are bit-consistent again
and that test keeps its exact equality **in both lanes**; no tolerance moved.
The kernel is unselected in both lanes but always compiled, so the fix is inert
today and correct for whoever lands the row.

`compat::tests::invert_aliases_are_unflipped_in_both_lanes` (renamed from
`unflipped_aliases_…`, which also covered `cdiv` before F.3e) keeps the
"still parity in both lanes" claim behavioral: it asserts on the anti-diagonal
matrix, where the two kernels genuinely disagree, so a future flip trips *there*
— next to the documented reason — before it reaches the corpus gate.

### DE_PASCALIZE Stage F.3e — the complex-division row resolves to *no split* (branch `depas-stagef`, 2026-07-27)

**A deliberate deviation from the plan's IV.2 table, settled by measurement
rather than by argument — flagged here for the plan owner.** The table proposes
`num_complex`'s `/` as the default kernel of row 1. Executing that flip would
make the *product* lane strictly worse, so the row is resolved the way the
table's own "Y triplet dedup" and (F.1's) sym-components rows are: **one shared
kernel, no `cfg` at all**. `compat::cdiv` is now an unconditional alias of
`cdiv_fpc_impl`. Engine-bit-neutral (the default lane already selected that
impl under F.1 staging); the change is the *decision* plus its evidence.

**Why.** FPC `ucomplex`'s `/` is not a Pascal wart — it is **Smith's
algorithm**, the standard robust complex division (C99 `_Cdivd`, LAPACK
`dladiv`). Measured against the correctly-rounded quotient (60-digit `Decimal`
reference, 20 000 random operand pairs spanning 1e-6…1e6, both Smith branches,
2026-07-27):

| kernel | mean rel. error | worst rel. error | `|den|` outside [1e-154, 1e154] |
|---|---|---|---|
| Smith (`cdiv_fpc_impl`) | **9.42e-17** | **3.82e-16** | still exact |
| naive (`cdiv_std_impl`) | 1.05e-16 | 4.26e-16 | `0` / `NaN` — total loss |

`num_complex::fdiv` (`self · conj/norm/norm`) is worse still — 22.2% of
components >1 ULP vs Smith's 10.5%. The parity lane already delivers bit-parity,
so the flip would buy the default lane nothing and cost it both accuracy and
robustness; IV.1 keeps "legitimate numerics with no crate equivalent" (complex
Bessel, `dss-sparse` row equilibration) in **both** modes for exactly this
reason. `support::line_constants` had independently recorded the same verdict
in-tree ("Smith's division … stays permanently") before this measurement.

**The verdict is asserted, not narrated.** `cdiv_std_impl` stays compiled and
gains two pins: `compat::tests::cdiv_shared_kernel_is_the_more_accurate_one`
(six operand pairs with their `Decimal`-60 correctly-rounded quotients embedded
as bit patterns — Smith must be no further from the truth on every row and
strictly closer on at least one) and
`naive_division_collapses_where_smith_stays_exact` (the overflow/underflow
regimes, `0`/`NaN` vs exact). `cdiv_is_one_shared_kernel_in_both_lanes` pins the
alias itself on an operand where the impls differ bitwise.

**If the plan owner still wants the flip**, everything needed is in place — flip
the alias, and the two pins above become the documented cost. Nothing else in
Stage F depends on this row.

### DE_PASCALIZE Stage F.3d — the RPN pi row flips (branch `depas-stagef`, 2026-07-27)

Fourth kernel family of F.3, and the plan IV.2 table's **row 4**:
`dss_parser::compat::PI`'s default arm now selects `PI_STD_IMPL`
(`f64::consts::PI`); the parity lane keeps the Pascal literal `3.14159265359`.
`TODO(compat)` **90 → 89**.

**What it moves.** `compat::PI` scales exactly two constants —
`RPNCalculator::{DEG_TO_RAD, RAD_TO_DEG}` — so the flip is confined to the
degree-trig entries of the deck language's inline RPN calculator (`sin`, `cos`,
`tan`, `asin`, `acos`, `atan`, `atan2`). `(30 sin)` evaluates to
`0.5000000000000299` in the parity lane and `0.49999999999999994` in the
default lane. The `pi` token (`EnterPi`) is untouched in **both** lanes:
upstream pushes FPC's full-precision `pi` builtin there, so nothing to flip.

**Where it is observable — measured, not assumed.** A tree-wide search for an
RPN trig expression (`rg -i "\b(sin|cos|tan|asin|acos|atan2?)\s*\)" tests/`)
finds exactly **one** occurrence in the whole gated corpus + golden corpus: case
`rpn_expressions` record 9 of the oracle golden `tests/golden/parser.json`,
whose value is the pinned `0.5000000000000299`. No `.dss` deck in
`tests/corpus` (514 cases) contains one, so no solved quantity, no report and no
corpus case moves in either lane.

**That one golden record is the deliberate divergence, and it is pinned, not
skipped.** `parser_golden.rs` grows a `DEFAULT_LANE_DIVERGENCES` table
(`case`, record index, expected, abs tol, why); in the **default** lane that one
record is compared against **0.5 ± ½ ULP** instead of the oracle value — the
*correctly rounded* `sin 30° = ½`, i.e. an independently-derived target rather
than our own constant re-evaluated (the parity value is 60 ULP away and fails
that assertion, so it cannot pass vacuously). Every other record of that case,
and every record of every other case, stays oracle-compared in **both** lanes.
The table is **fail-on-stale**: a row whose `(case, index)` never appears fails
the test, so a regenerated golden cannot silently disarm the exception.

**Pins.** `compat::tests::pi_alias_is_the_lane_kernel` (alias resolves to the
lane's impl, asserted against both impls, which differ) and
`parser::tests::rpn_degree_trig_is_the_lane_kernel` (deck-language boundary:
literal expected values per lane, no tolerance, plus `(pi)` as the control that
must stay `f64::consts::PI` in both lanes). The pre-existing
`pi_impls_agree_to_the_truncation_of_the_pascal_literal` keeps the measured
6.59e-14 relative gap documented.

### DE_PASCALIZE Stage F.3 — escape register, refined: the 89 remaining markers, classified (branch `depas-stagef`, 2026-07-27)

The F.3c register below classified the population into four buckets but left
its largest one — "remaining single-site semantic markers (59)" — as an
undifferentiated pile. Re-enumerated site by site (2026-07-27, `rg
"TODO\(compat\)" crates`, **89** occurrences after F.3d), it splits as:

**(1) 21 occurrences are not reproduction sites at all** — prose
cross-references (`see the TODO(compat) at …`), continuation lines of a marker
two lines above, and, worst, **negative** mentions whose text says *"this is
NOT a `TODO(compat)`"* (`exec/plot.rs` module doc, `exec/diakoptics/matrices.rs`,
`solution/time_series.rs`, `control/roll_avg_window.rs`,
`pc/generator/dynamics.rs`, `pc/ind_mach012/{mod,dynamics}.rs`). Those directly
violate CLAUDE.md's "the tag must stay greppable / do not use it for anything
else": they inflate every count and make the grep an unreliable index. Fixing
them is pure hygiene — reword, change nothing — and is the one part of the
sweep that needs no owner decision.

**(2) ~13 are the F-FMT rendering seam** (`util::fmt_g`, `report/format`,
`show/{mod,diagnostics}`, the JSON float/`NL`/`circuit.rs` `%g` block) — F.4's
defined scope, resolved by building `compat::fmt`, not here.

**(3) ~18 are the truncated-constant family** — unchanged, see (a) below.

**(4) 2 are resolved by the still-pending dense-inverse row** (`compat.rs`'s
"singular pivot leaves the matrix partially transformed" and `cmatrix::kron`'s
unchecked zero pivot).

**(5) the rest (~35) are genuine single-site upstream quirks**, and re-reading
them one by one shows the *same* blocker every time, which is why they are
listed here rather than swept: each one's clean fix changes a value or a string
that a **committed golden or a gated corpus case compares**, so landing it in
the default lane needs a per-site lane branch — i.e. a new row in a table the
plan declares closed. Representative: GICTransformer's `G2` scaled off `%R1`;
the Storage `/m` export's `EXP_PV_` prefix; `Save`'s doubled path delimiter;
Relay's `Recloser.<name>` event labels and its unguarded "Debug Sample" line;
StorageController's `not FleetState = STORE_IDLING` precedence bug; Load
`makeposseq`'s hard-coded `/3.0` (pinned by
`tests/corpus/modes/makeposseq/makeposseq_pc.dss`); Capacitor `makeposseq`'s
discarded `Cuf` write; CIM's `grounded := TRUE` pair, the `b0ch` typo and the
`LinearShuntCompensator.` prefix slip; Fault's `MinAmps` double-print;
Generator's `PrpSequence[26]/[27]` ordinals and the ShaftModel `FGetVarName`
mis-dispatch; the `DoubleSymMatrixProperty` zero-matrix; Monitor `Channel`'s
`[0.0]` placeholder (which reproduces *dss-python's wrapper*, not the engine).
A handful are instead honest **permanent semantics** whose marker only needs to
become plain documentation (Isource's `Bus2Defined`, LineCode's `C0` omission
and its dead `HrsToRepair`, the plot-option letter mapping and empty `else`,
`reduce.rs`'s `TotalLen := Len/2` — which writes a value nothing reads).

**What the owner has to decide** (unchanged in substance from (a)/(b) below,
now with the population attached): either the IV.2 table gains a general row —
"upstream single-site behavioral bugs: parity reproduces, default fixes" — with
per-site expected-value tests and field-scoped golden exclusions (the F.3c
machinery already exists: `GateSpec::ColAbove`, `LANE_SKIP_PROPS`), or the
sites are re-documented as permanent both-lane semantics. Until then they stay,
unchanged and green.

### DE_PASCALIZE Stage F.3 — escape register: what the closed table cannot absorb (branch `depas-stagef`, 2026-07-26)

Recorded with F.3c so the remaining `TODO(compat)` population is *classified*,
not merely counted. Nothing here is silently dropped; each item names its owner
and the evidence.

**(a) The truncated-constant family — ESCAPED, with a measurement that says
why.** 18 markers reproduce upstream's low-precision literals: `MU0 =
12.56637e-7`, `Twopi = 6.283185307`, `E0`, the tape-shield `1/pi = 0.3183`, the
Line `Kxg` earth constant `658.5` (×3 sites), `1000·√3 = 1732.0` and its
`0.001732` twin, `ln 10 = 2.3026` (ExpControl + the CIM IEEE1547 export),
`CALPHA = (-0.5, -0.866025)` (×2), the `complexutil` pair `3.14159265359` /
`57.29577951` with its hand-rolled `pascal_atan2`, and the `SymComp::official`
truncated `sin 60°`. **None of them is a table row**, and measurement shows why
they *cannot* be treated like one: their relative distance from the exact
constant is

| literal | rel. gap | | literal | rel. gap |
|---|---|---|---|---|
| RPN / complexutil pi | 6.6e-14 | | `MU0` | 4.9e-8 |
| `Twopi` | 2.9e-11 | | `CALPHA` | 4.7e-7 |
| rad→deg `57.29577951` | 5.4e-11 | | `ln 10` | 6.5e-6 |
| `sin 60°` (official) | 9.1e-10 | | `1732.0` / `0.001732` | 2.9e-5 |
| | | | `1/pi` `0.3183` | 3.1e-5 |
| | | | `658.5` (Kxg `De`) | **5.4e-4** |

The right-hand column is **at or above the calibrated 1e-6-class oracle
floors** — these are *physical-input* changes (like editing a deck's data), not
last-ulp kernel differences. Flipping them in the default lane would push it off
the oracle on continuous quantities and break IV.2's drift-model row 1
("continuous results — still oracle-compared, same floors, unchanged"), which is
exactly the promise the lane machinery exists to keep. Their clean fix therefore
belongs to an UPGRADE-style rung with its own per-case ledger triage and a
deliberate golden re-baseline, **not** to a Stage F lane flip. Decision needed
from the plan owner: either add a 12th table row ("upstream truncated physical
constants") with that ledger work, or re-document the family as permanent
both-lane semantics. Until then the markers stay, unchanged and green.

**(b) `HIDE_015X` ×17 in 7 files — ESCAPED.** UPGRADE §5's exit criterion is
"`rg HIDE_015X` empty", achieved by *flipping the Line/LineGeometry Dump/JSON
golden surface to capi015* (re-pinning `gen_json.py` off the 0.14.5 oracle and
dropping the `Line.Wires → "Conductors"` masquerade). That is a **byte-golden
re-baseline against a different oracle**, and the parity lane may never
re-baseline — so retiring the flag engine-wide breaks the parity contract, while
making it lane-conditional is a 12th table row nobody sanctioned. It also needs
`gen_json.py` to learn an engine switch (DIVERGENCES.md's own fallback: "if the
flip is disproportionate, keep HIDE_015X and document why"). Owner decision
required; the flag and its documentation are untouched.

**(c) The rendering markers (13: `util::fmt_g`, `report/format::fixed_w_fpc`,
`show/diagnostics`'s `%-.g`, `show/mod`'s device-name width, the JSON
`fpjson_float`/`NL`/`circuit.rs` `%g`/`%.4g`/`%8.2f` block) — F.4's, not
F.3's.** They are the F-FMT seam's parity kernel; their markers are resolved by
building the seam (`compat::fmt`), which is the next step's defined scope.

**(d) Remaining single-site semantic markers (59, of which ~8 are prose
mentions of the tag inside test/doc comments rather than sites).** Upstream
quirks the port reproduces that are neither arithmetic kernels nor sanctioned
bug fixes — CIM
`grounded := TRUE`, the `b0ch` typo, plot-option letter mapping, Isource's
`Bus2Defined`, StorageController's `not FleetState = STORE_IDLING`, LineCode's
`HrsToRepair`, Storage `MakeLike`'s missing `BeginEdit`, the Load `makeposseq`
divisor 3.0, the Relay `Recloser.<name>` event labels + unconditional debug
line, the Newton stale-`Iterminal` row, the `Save` doubled path delimiter, the
`EXP_PV_` Storage prefix, … Each needs an individual disposition (permanent
semantics re-documented / a default-lane fix + expected-value test), which is
mechanical but must be argued site-by-site against the Pascal. Not started —
this is the bulk of the remaining sweep and the honest reason F.3 is not closed.

### DE_PASCALIZE Stage F.3c — the three upstream-bug rows get their clean fix (branch `depas-stagef`, 2026-07-26)

The plan's two bug rows (`Export SeqCurrents` `Iresidual`, multi-meter
`Bus_Int_Duration`) plus the Monitor `BaseFrequency` fix CLAUDE.md defers to
Stage F **by name**. Each lands as a compat selector + the fixed branch +
an expected-value test + a *field-scoped* exclusion of the affected cells from
the default lane's oracle compare (plan IV.2's "deliberate divergences" row).
`TODO(compat)` **95 → 90**.

**1. `Iresidual` (`compat::IRESIDUAL_FROM_TERMINAL_1`).** Upstream's
`CalcAndWriteSeqCurrents` sums `cBuffer^[i]`, i = 1..Ncond *inside* the
per-terminal loop — the `(j-1)*Ncond` offset is missing — so every terminal row
prints terminal 1's residual. Default lane sums the row's own terminal (`base =
(j-1)*ncond`). Nothing solved changes; this is a reporting slice.
*Golden:* the default lane excludes exactly the `Terminal ≥ 2` cells of the
`Iresidual` column (new `GateSpec::ColAbove(1, 1.5)` — the mirror of the
existing `Col`), so every terminal-1 cell stays oracle-compared in both lanes,
at the same tolerance as before (the new `ColTol` carries the policy's own
`abs = 1e-8`; setting it to 0 tightened row 0 and the suite caught it).
*Pin:* `export_seqcurrents_iresidual_is_the_lane_kernel` derives its expectation
from an **independent** report — `Export Currents` writes a per-terminal
`Iresid<j>` column through a different code path, itself oracle-anchored by its
own byte golden — and asserts every row of `Export SeqCurrents` equals
`Iresid_j` (default) / `Iresid_1` (parity), plus that some row actually
separates the two kernels.

**2. `Bus_Int_Duration` (`compat::BUS_INT_DURATION_WALKS_ALL_BUSES`).**
`CalcReliabilityIndices`' duration loop walks **every circuit bus**, so with two
meters the later one re-reads foreign buses' `BusSectionID` against its *own*
`FeederSections`. The forward sweep now records the buses it assigns
(`zone_buses`), and the default lane iterates only those. The out-of-range
regime is unchanged: proven-nondeterministic OOB heap read, not reproduced in
either lane. *Golden:* no row key identifies "a bus of the earlier meter", so
the default lane masks the `Duration` column of
`export_busreliability_multimeter` and pins it **completely** instead —
`export_busreliability_multimeter_duration_is_the_lane_kernel` asserts all five
rows literally: parity `SRC 0, B1 6, B2 9, C1 6, C2 9` (the C-feeder's repair
times landing on the B-feeder's buses — what the oracle golden contains),
default `SRC 0, B1 4, B2 5, C1 6, C2 9` (each bus's own line's repair time).
Lambda / interruptions / customers / cust-interruptions / miles stay
oracle-compared in both lanes.

**3. Monitor `BaseFrequency` (`compat::monitor_base_frequency`).**
`TMonitorObj.Create` hard-pins 60.0 after the inherited constructor
(Monitor.pas:472 == r4133:552); the default lane inherits `Fundamental` like
every other element. **In a 60 Hz circuit the two are bit-identical**, which is
why no golden and no gated corpus case moves — asserted inside the test rather
than claimed. *Pin:* `monitor_basefreq_is_the_lane_kernel` (renamed from
`monitor_basefreq_pins_60hz_upstream_bug`) asserts 60.0 in the parity lane and
the inherited 50.0 in the default lane on a `DefaultBaseFrequency=50` deck, and
60.0 in both lanes on a 60 Hz deck. CLAUDE.md's bug-6 entry is updated from
"reproduced" to "lane-split", with the new pin name.
*Corpus:* the claim "no gated case moves" was **false and the gate said so** —
the 520-case run failed one case, `LVTestCase/Master.dss`, the vendored 50 Hz
European feeder, on `Monitor.line558_vi_vs_time` property `BaseFreq` (50 vs
60): the corpus gate compares element properties, and that deck's monitors are
live. Resolved the way the plan prescribes for a deliberate divergence — a
*field* exclusion, `harness::LANE_SKIP_PROPS = [("Monitor", "BaseFreq")]`,
applied **only in the default lane** (the parity lane still compares the value;
the property name/order walk still runs in both). Not a ledger entry: the
ledger records *upstream* divergences and is fail-on-stale across both lanes, so
a default-lane-only entry would break the parity run.

**Why these three and nothing else.** The dual-kernel table is closed. These are
its rows 9 and 10 plus the one upstream-bug fix CLAUDE.md explicitly defers to
Stage F; no new compat item was invented.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature
— **2230 passed / 0 failed / 5 ignored in each**, including the unconditional
corpus gate. `git diff -- tests/` empty (no golden, tolerance, ledger or deck
file touched — the lane branching lives in the drivers); `git status --short
tests/corpus` empty after the run (the known intermittent `Test/AutoTrans/*`
leak deleted by exact name). Tests +2 (2228 → 2230), 0 removed, 0 new
`#[ignore]`.

### DE_PASCALIZE Stage F.3b — the single-point stddev row: the first *deliberate divergence* goes live (branch `depas-stagef`, 2026-07-26)

Second kernel family of F.3, and the first row where the two lanes now print
**different numbers**: `compat::stddev_single_point`'s default arm selects
`stddev_single_point_zero_impl`. `TODO(compat)` **99 → 94**.

**What upstream does and why we stop doing it.** Pascal's four
`mathutil` mean/std-dev entry points (`RCDMeanAndStdDev`,
`…Single`, `CurveMeanAndStdDev`, `…Single`) special-case a one-element sample by
assigning `Mean` and *never clearing* `StdDev`, so the reported "standard
deviation" of a single point is the point itself. A one-element sample has no
spread; the default lane returns `0.0`. The parity lane keeps the quirk.

**Observable, and pinned by expected values in both lanes.** `npts=1` shapes
are real corpus input (`epri_dpv/{J1,K1,M1}`), and the value surfaces as the
`LoadShape`/`TShape`/`PriceShape` `stddev` property (and through it in
`Dump`/`Save`/AltDSS-JSON `Set %stddev=`). Three tests, no tolerances anywhere:
`mathutil::tests::single_point_std_dev_is_the_lane_kernel` (all four entry
points, literal `3.5` vs `0.0`, plus a non-vacuity assertion that the two impls
disagree), `load_shape::tests::single_point_shape_stddev_property_is_the_lane_kernel`
(deck-level: `New LoadShape.one npts=1 mult=(0.4)` → `? …stddev` prints `0.4` in
the parity lane, `0` in the default lane, while `mean` is `0.4` in both so a
broken accessor cannot fake either result), and the alias-level
`compat::tests::stddev_alias_is_the_lane_kernel`.

**No golden or corpus case moved** — verified by running, not assumed: every
shape any committed golden or gated corpus case computes a std-dev for has ≥2
points (the `default` daily shape has 24), so both lanes stay byte-identical on
the whole existing gate. The F.1 staging test `f1_staging_…` is split into
`unflipped_aliases_still_select_the_parity_kernel_in_both_lanes` (cdiv /
invert / etk_invert — still parity in both lanes) and the per-lane stddev test
above, so the "which aliases are flipped" claim stays asserted rather than
narrated.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature
— **2228 passed / 0 failed / 5 ignored in each**, including the unconditional
corpus gate. `git diff -- tests/` empty; `git status --short tests/corpus`
empty after the run. Tests +3 (2225 → 2228), 0 removed, 0 new `#[ignore]`.

**Process note (worth keeping).** An earlier attempt showed 9 corpus cases
failing with the *oracle* worker's "cannot access the file because it is being
used by another process" on ckt24's `REG_subxfmr_regulator.csv` — caused by
**two gate runs overlapping in this worktree** (a second run started while the
first was still in its parity lane), not by any code change. The gated corpus
decks are shared mutable state: one gate at a time per worktree. The stray
artifacts were deleted by exact name.

### DE_PASCALIZE Stage F.3a — first kernel flip: the FPC `Round` row (branch `depas-stagef`, 2026-07-26)

F.3 runs one kernel family per commit. This is the **round** row (plan IV.2
table): `dss_parser::compat::round_i32`'s default arm now selects
`round_i32_saturating_impl`, and the family's 19 upstream-inexactness markers
are resolved. `TODO(compat)` **123 → 99** workspace-wide.

**The flip.** Parity lane: FPC `Round` → Int64 with the x87 *integer
indefinite* sentinel on overflow/non-finite, then the Pascal
`Integer := Round(…)` truncation — so a deck's `n=inf` converts to **0** and
`n=1e10` to **1410065408**. Default lane: `round_ties_even` + Rust's saturating
cast — `i32::MAX`/`i32::MIN`. Identical on every in-range finite value (pinned
over a 15-value set), so no golden, corpus case or checkpoint moves: **2225
tests pass in both lanes, byte-identically**. The divergence is pinned by
expected values at the only boundary a deck can observe it —
`parser::tests::make_integer_out_of_range_is_the_lane_kernel` (4 inputs × both
kernels, literal expectations, no tolerance), plus `compat::tests::
round_alias_is_the_lane_kernel` at the alias.

**Marker sweep (19 sites, 15 files) — why they were never divergences.** Every
in-engine `TODO(compat): FPC Round` marked a plain `x.round_ties_even()`, which
*is* FPC `Round` (both round-half-to-even) for every value those sites can
produce — tap positions, years, hour/interval indices, point counts. Nothing
upstream-inexact was being reproduced there, so the marker was mis-applied: it
is replaced by plain documentation pointing at the one reference site
(`reg_control::get_tap_num`) and at the deck-language boundary that *does* model
the indefinite path. Two sites (`LoadShape`/`ScalarShape` index) say so
precisely: an `interval` small enough to push `hr/interval` out of Int64 range
makes FPC index the array out of bounds — upstream UB, which per CLAUDE.md is
not reproduced (the port saturates into a real bounds check). No behavior
changed anywhere in the sweep.

**`val_f64`'s `infinity` rejection is now permanent, in both lanes** (its marker
promised the opposite). Which literals the deck language accepts is
command-input semantics, not a numeric kernel; widening it belongs to the
opt-in strict-parsing layer the plan sequences *after* Stage F (IV.1b). Noted at
the site: `f64::from_str` would map `infinity` to `inf`, so "collapsing" it
would turn a conversion error into a non-finite property value.

**Feature propagation is now measured, not assumed.** `dss-parser` and
`dss-sparse` gain a `compat::ORACLE_PARITY` const (like `dss-core`'s), and
`dss-core`'s new `compat::tests::the_lane_reaches_every_compat_crate` asserts
all three agree. Without it a dropped `Cargo.toml` feature edge would compile
the *default* pi/round/solver kernels inside a parity build, and neither
crate's own per-lane test could tell (const and alias share a compilation
unit). It passes in the parity lane → the edge is proven live.

**Not flipped here** (each lands with its own family): `PI` (moves
RPN-computed deck values), the three `dss-core` kernels, and the `dss-sparse`
knobs (declaration-only until M3c / WP-R1 own them).

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature
— **2225 passed / 0 failed / 5 ignored in each**, including the unconditional
corpus gate. `git diff -- tests/` empty (no golden, tolerance, ledger or deck
touched); `git status --short tests/corpus` empty after the run (the known
intermittent `Test/AutoTrans/AutoHLT_{HT_losses,LT_current}.txt` leak deleted by
exact name). Tests +3 (2222 → 2225), 0 removed, 0 new `#[ignore]`.

### DE_PASCALIZE Stage F.2 — the default-lane test policy (drift model), still bit-neutral (branch `depas-stagef`, 2026-07-26)

Second step of Part IV.2: the plan's **drift-model table** becomes code. The
suite gains one lane-policy module and every affected golden driver routes
through it; the engine itself is untouched except for a lane marker. With the
F.1 staging still selecting the parity kernels everywhere and rendering
unchanged, **both lanes pass trivially and byte-identically** — 2222 tests, 0
failed, in each lane.

**`crates/dss-core/tests/harness/lane.rs` (new)** — the single place the suite
reads the lane. `pub const PARITY: bool = cfg!(feature = "oracle-parity")`, so
both arms of every policy always compile and are type-checked in both lanes
(the same discipline as the engine's `compat` kernels); no golden driver reads
the cfg itself. Policies:

| Quantity class | parity lane | default lane |
|---|---|---|
| Continuous (V, I, S, losses, registers) | `tol_for` floors | **identical, untouched** |
| Discrete state (taps, control state, action counts, event log) | exact | **exact** |
| Iteration count vs an oracle golden | `==` (or `<=` on r4133) | `compare_iterations`/`_le` — a ±`ITER_SLACK` = 1 band + a NOTE on any nonzero drift |
| Text report goldens rendered through the F-FMT seam | byte-exact (`assert_bytes_eq`) | `compare_report` → `compare_export` on the SAME committed golden |
| Deliberate divergences | (F.3 — none exist yet) | (F.3 — excluded field-by-field + expected-value tests) |

**No tolerance is introduced anywhere.** Every default-lane report policy is
`lane::exact_value_policy` = `rel = abs = 0`: the numbers must still parse to
*bit-identical* f64 — only their spelling and padding are freed. A unit test
pins that a 1-ulp value change fails in **both** lanes.

**Which goldens are lane-split — the empirical scoping rule.** A byte golden is
routed through `compare_report` **iff its writer renders a number through the
F-FMT seam** (`util::fmt_g`, `report::format`'s `g`/`fixed*`/`g_w`/`fpc_sci_w`/
`pad` family, `comma_text`) — i.e. iff F.4 can move its bytes. Everything else
keeps the byte compare in *both* lanes, since that is the strictly stronger
check and Stage F cannot move it. Determined by reading the writers and the
goldens, not by category:

- **split** (parity bytes / default parsed-numeric): the ~50 `Dump` script
  goldens (incl. the masked/block-masked twins), the 4 `Show LineConstants`
  pairs (report + `LineConstantsCode.dss`), `save_mtr`.
- **byte in both lanes**: `Show Loops`/`Zone`/`Controlled`/`Isolated`/
  `Topology*`, `Export UUIDs`, `Show PV2PQ_Conversions`, the incidence-matrix
  CSVs — identifier/tree/integer text with no float rendered at all; the CIM
  XML profiles and the AltDSS JSON captures (no `report::format` float helper —
  JSON renders through its own `{:.16E}` in `export/json/mod.rs`, outside the
  F-FMT inventory); the `SngSave/DblSave` **binary** goldens.
- The rule is **enforced mechanically, in both lanes**: `compare_report`
  asserts the golden actually carries a rendered number, so a number-free
  golden cannot be silently down-graded to a near-vacuous token compare (unit
  test `number_free_golden_is_rejected_by_the_scoping_guard`).
- One *supplementary* byte-layout block inside
  `show_powers_elem_autotrans_matches_oracle` (column widths / pad glyphs, on
  top of the tokenizing compare) is now `if lane::PARITY` — it pins exactly
  what F-FMT re-renders. Its numeric twin is lane-independent.

**`Key=Value` script tokens.** The `Dump`/`Save` text is `~ R=1.1`, which the
whitespace tokenizer cannot split further, so a re-rendered value would fail
even the token compare. `harness::mod.rs`'s `field_eq` therefore gets ONE
default-lane fallback (`kv_value_eq`, marked in place): when a non-numeric
token fails the verbatim compare, and **both** sides are `key=<number>` with
matching keys, the values are compared numerically under the policy tolerance
(= exactly, here). Keys, token structure and values stay pinned; only spelling
is freed. The parity lane never calls it. Unit-tested both ways (`R=1.100`
passes default-only; `R=1.2` and `Rp=1.1` fail in both).

**Iteration counts.** All 12 oracle-facing sites route through the helpers:
`golden_{checkpoints,feeders,feeders_controls,ieee8500,metering_monitors,
protection,slice,timeseries_controls}`, `harness/scenario.rs`,
`wasm_usermodels{,_wm4}`, and the corpus gate's two non-ledgered shapes. Kept
exact in both lanes on purpose: (a) **ledger-scoped** corpus iteration pins —
they are hit-tracked (fail-on-stale), so a default-lane flip that moves one
must be re-triaged in `ledger.json`, not absorbed; (b) the **Rust-vs-Rust**
pins (`save_roundtrip`'s warm-re-solve counts) and the `src/exec/tests/*` unit
pins — they are self-regression signals, exactly the "tracked" side of the
drift model, and the plan only unpins counts *vs the oracle*. The
`golden_timeseries_controls` comment recording the historical `Yeq`-restamp bug
(which manifested as ±1 iteration drift) is preserved with a note that the
parity lane keeps that exact pin forever, so that regression stays caught.

**Non-vacuity — the split is proven by doing, in both lanes.** Nine unit tests
in `lane.rs` (×18 harness-including binaries = 162 runs per lane) assert the
*split itself* rather than one lane's behavior: `assert_eq!(ok, !PARITY)` for a
rendering-only report difference, for a re-rendered `Key=Value` token, for a
one-step iteration drift, and for the `<=` variant — each must PASS in the
default lane and FAIL in the parity lane, whichever lane is running. Plus
`lane_const_tracks_the_engine_build`, which asserts the harness's lane const
equals the engine's new `dss_core::compat::ORACLE_PARITY`: if the feature ever
stopped propagating into the integration-test crate, every lane branch would
silently run the default policy against a parity engine and the parity gate
would evaporate. It passes in the parity lane → feature propagation into the
test crate is *measured*, not assumed.

**Engine change (one, inside the sanctioned module).**
`dss_core::compat::ORACLE_PARITY` — a `bool` the engine never reads, so the
build stays bit-neutral; consumed by the harness guard above and, later, by
F.5's differential job to label its two builds.

**Deferred / recorded, not silently dropped.** (a) `export/json/circuit.rs`'s
two `fixed_w_fpc` calls render DSS *script* text inside the JSON payload; if
F.4 moves them, that commit routes whatever golden covers them by the same
rule. (b) Event-log strings contain seam-rendered numbers but are drift-model
*discrete* state (exact in both lanes) — if F-FMT changes them, F.4 decides
(exclude the event log from the seam, or tokenize it) rather than relaxing the
discrete row. (c) `ITER_SLACK` is 1 today; M3c/WP-R1 may need a wider band and
must raise it here **with a measurement**, per case.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature
— 2222 passed / 0 failed / 5 ignored in each, including the unconditional
corpus gate (`corpus_gate_all_cases_match_engines ... ok`, 520/520 cases, 26
ledger entries all hit). `git diff -- tests/` empty (no golden, tolerance,
ledger or deck touched); `git status --short tests/corpus` empty after each run
(the known intermittent `Test/AutoTrans/*` leak deleted by exact name, never a
wide clean). Tests: +9 (all new, in `lane.rs`), 0 removed, 0 new `#[ignore]`
(`git diff -U0` shows no removed `#[test]`/`#[ignore]`). Inventory unchanged,
as expected for F.2: `TODO(compat)` **123** workspace / **117**
`dss-core/src`, `HIDE_015X` **17**.

One parity-lane corpus run failed and was re-run green: the *oracle* worker
could not create `IEEE8500u_VLN_Node.txt` ("used by another process") while
writing `Show Voltage LN Nodes` into the vendored deck folder — a Windows
file-lock race in the shared corpus dir, not a comparison failure. The same
binary passed before and after, and this step touches no engine path and no
corpus file.

### DE_PASCALIZE Stage F.1 — the `oracle-parity` seam, bit-neutral in both lanes (branch `depas-stagef`, 2026-07-26)

First step of the plan's **last** stage (Part IV.2). It adds the two-lane
machinery and *nothing else*: the feature, the three `compat` modules, both
implementations of every function-shaped dual kernel, the per-kernel tests, and
the CI grep gate. **Every alias deliberately selects the parity impl in BOTH
lanes**, so the commit is byte-neutral everywhere and the whole existing gate
(byte goldens, checkpoint Y, 514-case corpus, iteration counts) is unchanged in
the default build too. F.3 flips the `not(oracle-parity)` arms one kernel family
at a time.

**Feature wiring (additive).** `dss-core/oracle-parity =
["dss-parser/oracle-parity", "dss-sparse/oracle-parity"]`; `dss-cli` exposes it
as its own build option (`--features oracle-parity` → `dss-core/oracle-parity`).
Gate invocation for the parity lane: `--features dss-core/oracle-parity` (from
the workspace root; unification turns it on for `dss-cli`/`dss-epri` builds of
`dss-core` too).

**Kernels wired** (`compat.rs` per crate; parity impl ⇄ default impl, both
always compiled, measured cross-impl bound in the unit test):

| Row | parity `_impl` | default `_impl` | measured gap |
|---|---|---|---|
| complex division (`compat::cdiv`, dss-core) | `cdiv_fpc_impl` (FPC Smith) | `cdiv_std_impl` (`num_complex` `/`) | 2.10e-16 rel = 0.95 ULP |
| dense complex inverse (`compat::invert`) | `invert_gj_no_exchange_impl` | `invert_partial_pivot_impl` | 2.15e-16 rel; residuals 2.30e-16 vs 1.27e-16 |
| dense real inverse (`compat::etk_invert`) | `etk_invert_gj_no_exchange_impl` | `etk_invert_partial_pivot_impl` | 1.25e-16 rel |
| single-point stddev (`compat::stddev_single_point`) | value itself | `0.0` | deliberate divergence |
| RPN pi (`compat::PI`, dss-parser) | `3.14159265359` | `f64::consts::PI` | 2.069e-13 abs / 6.59e-14 rel |
| FPC round (`compat::round_i32`, dss-parser) | `round_i32_fpc_impl` | `round_i32_saturating_impl` | equal on all in-range finite; diverges out of i32 range |
| solver execution (dss-sparse `compat`) | `PARALLEL_FACTORIZATION`/`ITERATIVE_REFINEMENT` = false | true | declaration only — see below |

Call sites are unconditional: `CMatrix::invert` and `mathutil::etk_invert`
delegate to the aliases, `cdiv_fpc` is gone from `cmatrix` (7 call sites now
call `compat::cdiv`), the four `mathutil` single-point returns call
`compat::stddev_single_point`, `RPNCalculator::DEG_TO_RAD/RAD_TO_DEG` are built
from `compat::PI`, and `parser::convert::pascal_round_to_i32` delegates to
`compat::round_i32`. The parity `invert` kernel calls `cdiv_fpc_impl`
**directly**, never the alias — F.3 must not drag it along when it flips `cdiv`.

**Two rows measured out of the split (recorded, not invented).**
(a) *Sym components.* The plan table puts `SymComp::official` on the parity
side; the Pascal says otherwise — `mathutil.pas:518-548` selects
`SelectAs2pVersion(False)` = "ours" = `SymComp::precise` at initialization, and
the `official` truncated pair is reachable only under upstream's
`DSSCompatFlag.BadPrecision` env flag (`CAPI_DSS.pas:315`), which neither gating
oracle sets. So parity == default == `precise` and an alias would select the
same impl twice: **no split**, like the Y-triplet row. Both variants stay
compiled and a new test
(`mathutil::tests::sym_comp_official_vs_precise_gap_is_the_truncated_sin60_constant`)
pins the gap at 4.50e-10 relative (the truncated `sin 60° = 0.866025403`).
(b) *Solver execution.* The knobs are **declared, not yet wired**, and the doc
says so: `SparseSet::factor` goes through faer's `Lu::try_new_with_symbolic`,
which reads faer's *global* parallelism (`get_global_parallelism()` — with
faer's default `rayon` feature that is `Par::rayon(0)`, **not** `Par::Seq`, so
the plan-table wording is aspirational today) and takes no per-call `Par`.
Overriding it needs the lower-level `factorize_numeric_lu` path = M3c's job;
WP-R1 owns refinement. F.1 does not touch either — that would not be
bit-neutral.

**CI grep gate.** `crates/dss-core/tests/oracle_parity_cfg_gate.rs` walks every
`.rs` under `crates/*/{src,tests,benches,examples}` (asserting the walk found
>100 files) and fails if `feature = "oracle-parity"` appears outside a `compat`
module or test code, printing file:line. Non-vacuity: it also asserts the three
`compat.rs` files exist and each still carries the cfg. **Verified
feature-sensitive by doing it** — a probe `#[cfg(feature = "oracle-parity")]`
inserted at `dss-cli/src/main.rs:1` failed the gate with that exact line, and
was reverted.

**Inventory unchanged** (F.1 resolves nothing — that is F.3's sweep):
`TODO(compat)` **123** workspace / **117** `crates/dss-core/src`, `HIDE_015X`
**17**, exactly the pre-Stage-F values. Markers moved with the bodies they
document (the `CMatrix::invert` marker into `compat`, the RPN-pi and FPC-round
markers into `dss-parser/compat.rs`); no marker was added or deleted, and the
prose in the new files avoids the literal tag so the grep stays exact.

**Deferred to F.3 (recorded, not silently dropped).** The three bug-fix branches
(`Iresidual` offset, `Bus_Int_Duration` skip, Monitor `BaseFrequency` inherit)
get their seams *with* their fixed branch and expected-value tests — writing
half of them here would land untested code. `complexutil`'s truncated
`3.14159265359`/`57.29577951` + `pascal_atan2` are the same upstream-inexactness
family as the pi row but a different kernel (a hand-rolled `atan2`, not just a
constant): F.3 decides fold-in vs escape-record; F.1 deliberately did not extend
the table.

**Proof.** Both lanes green: `cargo fmt --all --check`; `cargo clippy
--workspace --all-targets -- -D warnings` and the same with `--features
dss-core/oracle-parity`; `cargo test --workspace` and the same with the feature
— including the unconditional 514-case corpus gate in each. `git diff --stat --
tests/` empty (no golden, tolerance, ledger or deck touched); `git status
--short tests/corpus` empty after every run. New tests: 16 (9 dss-core compat,
4 dss-parser compat, 2 dss-sparse compat, 1 mathutil sym-comp) + the grep gate;
0 removed, 0 `#[ignore]`.
