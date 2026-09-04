# R4133_PROPS — WP-RP5 record

> Created 2026-09-04 for `R4133_PROPS_PLAN.md` WP-RP5 (RP5.1, RP5.2, the
> closeout). Per `STATUS.md` section 1 "Record placement (from 2026-09-03)",
> every sub-step's **full** record lives in its per-WP file under
> `docs/phase-records/` while STATUS section 1 keeps only a 3–6 line landed
> paragraph — verdict, date, commit, pointer. This file is WP-RP5's; STATUS
> section 7 forwards `§WP-RP5` and `§RP5.x` here.

### R4133_PROPS WP-RP5 — records

**RP5.1 (operational docs for the r4133 property policy) landed 2026-09-04 in
one commit — documentation only: zero product-crate lines, zero test logic, zero
table rows, zero tolerances, zero golden bytes, zero ledger entries.** The
sub-step's `git diff --numstat` is `TESTING.md` +190/−4,
`tests/TOLERANCE_NOTES.md` +33/−2 and
`crates/dss-core/tests/harness/props_norm.rs` +2/−2 — the last of those two
**doc comments**, no code. Not one line under any crate's `src/`.

**What was written to `TESTING.md`.** A new section, "The r4133 property policy —
the claim chain (R4133_PROPS)" (`TESTING.md:841`), placed immediately after
§"0.15.x property-table allowlist (`PROPS_015X`)" and before §"Environment
variables", i.e. beside `PROPS_015X` as §RP5.1 asks. It documents each link of
the chain **with its liveness / count-lock guarantee**: the six-row order table
(shape allowlist → skip rows → normalization → echo → display floor → assert),
naming the one function that holds the order (`harness/mod.rs:3243`,
`compare_prop_lists`) and each link's seam line; the normalization table
`PROPS_NORM_R4133` (168 rows, the four rule kinds with their row counts, the
value-preserving contract, the count lock, both liveness halves); the echo table
`PROPS_ECHO_R4133` (82 rows, the four categories with counts, the witness
obligation, pair-scoped vs per-cell `ECHO_NARROWED` width, the pin arithmetic,
the two-arm liveness guard); the display floor (its two clauses, its scope, the
pointer to the RP2.4 derivation); the `SKIP_PROPS` r4133 dispositions of plan
§1.2 as a three-row table (`SKIP_PROPS_CAPI_ONLY` compares on r4133 /
`SKIP_PROPS_BOTH_CHANNELS` stays skipped / `LANE_SKIP_PROPS` channel-blind) with
the partition lock and the channel-scoped whole-element skip; and the "did the
chain run at all" guards (the global `assert_r4133_props_compare_ran` plus the
forcing-rule population lock `FORCED_PROPS_POPULATION = (440, 313, 83, 44)`).
The census knob, the replay accounting, the pins file and the frozen evidence
already have their own sections and are linked, not restated.

**Every claim cites a landed code line — 42 citation rows over 7 files**
(`harness/mod.rs`, `harness/props_norm.rs`, `props_r4133_replay.rs`,
`corpus_gate.rs`, `corpus_gate/scheduler.rs`, `corpus_gate/ledger.rs`,
`exec/tests/base_frequency.rs`), each verified by reading the current tree rather
than transcribed from the plan. The counts in the new section were **re-derived**
from the tables themselves: `SKIP_PROPS` 17 = 10 + 7;
`ECHO_ROWS_ON_R4133_ONLY_CASES` 58 rows / 34 971 cells (column 3 summed);
`PROPS_ECHO_R4133` 82 rows of which 64 name a pin, over 30 distinct pin names.

**Two stale counts in §"The r4133 echo-exclusion pins" were corrected**, each
with a dated note saying what it predates: "`ECHO_ROWS_ON_R4133_ONLY_CASES` (57
of the 81 rows, 34 969 cells)" → **58 of the 82 rows, 34 971 cells** (the old
pair predates RP3.3's `generator.model` row), and "**29** pins covering **63** of
the 81 rows" → **30** pins covering **64** of the 82 rows. That section's "62 of
the 82 rows are pair-scoped" was already correct and was left untouched.

**The triage delta** landed as its own procedure, "Triage a property divergence
(the r4133 channel)" (`TESTING.md:1108`), inserted immediately before "Triage a
divergence into the ledger" — it is that procedure's specialization and ends by
handing over to it. Step 0 is CLAUDE.md's rule (a gap is a port bug until proven
otherwise) plus locating the whole pair with `DSS_PROPS_CENSUS=claims`; then
**normalize** → **echo** (+ a pin in `props_r4133_pins.rs`) → **floor** (nothing
to edit, and never widened) → **ledger** (a `property`-scoped entry pinning both
numbers), in that order. It closes with the two things that are *not* steps:
parking an r4133 divergence in `SKIP_PROPS`, and "skip it for now".

**The two verify-only items of §RP5.1 needed no edit, and the verification is
recorded rather than assumed.** (a) *Env-var rows (RP0.2)*: every `DSS_*` name
appearing under `crates/dss-core/tests` was enumerated (24 distinct real vars)
and cross-read against §"Environment variables" — all present, including
`DSS_PROPS_CENSUS` with **both** modes (`1` and `claims`) and its `DSS_GATE_ONLY`
interaction; the new section links that row instead of restating it. (b)
*§PROPS_015X wording (RP4.1)*: the section already carries the post-RP4.1 wording
and its numbers check out against the tree — "440 = 313 `both` + 83 r4133-only +
44 capi-only" equals `corpus_gate/scheduler.rs:148`, the named forcing-rule test
exists at `:169`, and the "54 `kind=large*` `both` decks" cost guard matches
[`r4133-props-rp4.md`](r4133-props-rp4.md):217-227.

**`tests/TOLERANCE_NOTES.md` — the RP2.4 cross-check: one correction, one new
block.** The correction (`:1262`, dated inline): the "why no real coverage is
lost" clause (c) claimed the four root-cause pairs "stay UNCLAIMED and are still
compared raw". Half of that went false after RP2.4 — RP3.3 gave
`generator.model` an echo row with its own pin, and RP4.1 landed **eight**
`property`-scoped ledger entries (2 × `swtcontrol.delay`, 4 × `windgen.kvar`,
2 × `gictransformer.r2`) that pin both numbers per case. The clause now says the
floor **refuses** them (still true, and the paragraph's point) and that the link
*after* the floor — the ledger — handles them, with RP3.9's 55 spellings the only
ones that genuinely stay UNCLAIMED; evidence read back from
`tests/corpus/ledger.json`, `props_r4133_replay.rs:5419-5461`
(`LEDGER_ENTRY_PINS`) and `props_norm.rs:2082`. The new block, "RP5.1
cross-check against the landed tree (2026-09-04)" (`:1352`), is an 8-row table of
every number the section states against the line it was re-read from: the floor
`2e-4` (`props_norm.rs:895`), both clauses shipping (`:1082`, `:1175`,
`mod.rs:3311`), the four derivation magnitudes (`:786-792`),
`CLAIMED_DISPLAY_FLOOR = 1951` (`props_r4133_replay.rs:565`), and the `micro` /
`feeder` / `midi` / `micro_wtg3_dynamics` tier floors plus the uncovered
magnitudes in `harness/mod.rs` — **all eight unchanged**. Nothing was loosened;
the one edit tightened a description.

**The two doc-comment fixes.** `NORM_ROWS` has been **168** since RP4.1
(`props_norm.rs:742`), but two doc comments still called the table "the 157
rows" — the RP2.2-era count, which entered at `ab2bf041`. Both are the liveness
guard's own prose, i.e. exactly the claim the new `TESTING.md` section cites, so
they were corrected (`props_norm.rs:1429`, `:4359`) rather than cited stale. Doc
comments only; the count lock itself (`assert_eq!(PROPS_NORM_R4133.len(),
NORM_ROWS, "total row count moved")`, `:3054`) was not touched — the fix corrects
prose, it does not relax an assertion.

**Gate — green in both lanes, first try.** The five mandatory commands ran at
whole-workspace scope with no name filter and no `DSS_GATE_*` / `DSS_ORACLE_*`
knob set: `cargo fmt --all --check` **0**; both `clippy` runs **0**, each
reporting `Checking dss-core` before `Finished` (so neither replayed a warm
fingerprint) and each emitting zero diagnostics; both test lanes **0**. **4 497
passed / 0 failed / 5 ignored / 0 filtered out** in each lane over 74
result-reporting binaries — identical binary for binary, and a delta of exactly
**0** against the RP3.10-settled baseline at `45f2ece3`, which is the predicted
outcome for a docs-only diff. `corpus_gate` ran whole (140 tests, 142.75 s
default / 139.53 s parity): `corpus_gate_all_cases_match_engines` and
`corpus_gate_props_census` **ok** in both lanes, all four ledger guards green,
zero NEVER-APPLIED entries, zero stale entries, zero reds on either channel. The
five ignored tests are the pre-existing set of the RP3.9 / RP3.10 records; RP5.1
added no `#[ignore]`. §RP5.1's own acceptance target holds: all **11**
`oracle_parity_cfg_gate` tests are `ok` in both lanes,
`operational_docs_cite_the_compat_machinery_accurately` among them, so every doc
claim added here is cited against a line the executable doc-walk can still find.
`lane_diff` was **not** run and does not apply: no product code moved (`git diff
--name-only` matches nothing under `/src/`), so the 2026-07-31 `max |Δ| = 0`
default↔parity baseline, last reproduced by RP3.10, cannot have moved.

**CLAUDE.md was verified and deliberately not edited** — §RP5.1 asks for no new
section there and for the gate section to be checked; its factual claims all hold
against the tree. One sentence is handed forward rather than silently touched:
CLAUDE.md:196-198's "Stage F introduces **no** tolerance anywhere", which RP2.4's
display floor now qualifies. It is a documentation-consistency question with no
gate, lane or test depending on it; **owner: RP5.2**.

**One commit.** RP5.1 landed as a single commit carrying the three documentation
files together with this record and STATUS section 1's landed paragraph — no
follow-up fix commit, and nothing under `tmp/` or `investigations/` staged (the
whole `tmp/rp51/` working set is gitignored).
