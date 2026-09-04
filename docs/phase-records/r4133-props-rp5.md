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

**Every claim cites a landed code line — 46 `file.rs:LINE` tokens resolving to
43 distinct (file, line) targets over 6 code files** (`harness/mod.rs`,
`harness/props_norm.rs`, `props_r4133_replay.rs`, `corpus_gate.rs`,
`corpus_gate/scheduler.rs`, `corpus_gate/ledger.rs`), **plus 9 bare `:LINE`
continuations** of the token before them. A seventh file,
`exec/tests/base_frequency.rs`, is reached by pin *name* only
(`monitor_basefreq_inherits_the_fundamental`) and carries no citation row. Each
was verified by reading the current tree rather than transcribed from the plan;
since the settlement below they are re-verified by a test on every run. The counts in the new section were **re-derived**
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
(the r4133 channel)" (`TESTING.md:1116`), inserted immediately before "Triage a
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
rows" — the **RP2.1**-era count. Both sentences entered at `1b93b91e` (RP2.1
audit settlement) and one of them was *rewritten while still saying 157* by
`59e521e5`, the very commit that set `NORM_ROWS = 168`; `ab2bf041` (RP2.2) is
the commit that moved the table's OTHER "157 rows" mentions to 161 and missed
these two. Both are the liveness guard's own prose, i.e. exactly the claim the new `TESTING.md` section cites, so
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
added no `#[ignore]`. All **11** `oracle_parity_cfg_gate` tests are `ok` in both lanes,
`operational_docs_cite_the_compat_machinery_accurately` among them. That test
checks the *compat* half only — a doc line spelling the compat tag next to a
`.rs` name, and every `compat::` alias a doc names — and its path extractor
stops at `.rs`, so at RP5.1 it read **none** of the `:LINE` suffixes above:
§RP5.1's acceptance ("every doc claim cites the landed code line") held **by
hand-verification**, not by test. The settlement below closes that with a
twelfth test.
`lane_diff` was **not** run and does not apply: no product code moved (`git diff
--name-only` matches nothing under `/src/`), so the 2026-07-31 `max |Δ| = 0`
default↔parity baseline, last reproduced by RP3.10, cannot have moved.

**CLAUDE.md was verified and deliberately not edited** — §RP5.1 asks for no new
section there and for the gate section to be checked; its factual claims all hold
against the tree. One sentence is handed forward rather than silently touched:
CLAUDE.md:196-198's "Stage F introduces **no** tolerance anywhere", which RP2.4's
display floor now qualifies. It is a documentation-consistency question with no
gate, lane or test depending on it; **owner: RP5.2**.

**Three commits.** RP5.1 landed as a single commit, `dd0b9e5b`, carrying the
three documentation files together with this record and STATUS section 1's
landed paragraph; the audit round added the settlement commit `8802fb6a` below;
ritual step 6 added this settlement-record commit (STATUS + this file + the
plan's dated lines + one `TESTING.md` sentence). Nothing under `tmp/` or
`investigations/` was staged by any of them (the whole `tmp/rp51/` working set
is gitignored).

### RP5.1 — audit settlement (2026-09-04, `8802fb6a`)

Two fresh auditors (`/audit-code`, `/audit-tests`) read `45f2ece3..dd0b9e5b`.
Neither found a lost deliverable, a weakened test, a moved tolerance or a
reproduced bug: the range is documentation plus two doc comments, and both
auditors independently re-resolved every `file.rs:LINE` citation it landed and
re-derived every count it states from the tree. What they found is one
**overclaim about protection** and six accuracy defects in prose. Each is
settled below against the tree, never against plausibility.

| # | finding | disposition |
|---|---|---|
| AC-2 / AT-1 (major) | the record claimed the executable doc-walk verifies RP5.1's citations | **FIXED, twice** — the sentence now says what that walk checks, and a twelfth test makes the claim true |
| AC-1 / AT-2 | "Both halves skip a `DSS_GATE_ONLY` run by an explicit check" is false for the offline half | **FIXED** — only the live half reads the variable |
| AC-3 | wrong provenance for the corrected "157 rows" doc comments | **FIXED** — 157 is the RP2.1-era count; `ab2bf041` is where the *other* mentions moved to 161 |
| AC-4 / AT-3 | "42 citation rows over 7 files" is not a number the tree produces | **FIXED** — 46 tokens / 43 distinct targets / 6 code files, plus 9 bare continuations (record and `STATUS.md`) |
| AC-5 | "every value link is a `PropsPolicy` method gated on `is_r4133()`" over-generalises link 1 | **FIXED** — the claim is now scoped to links 2–4, with link 1's exception stated |
| AT-4 | the `SKIP_PROPS` disposition table invites summing to 18 | **FIXED** — the prose now spells 17 = 10 + 7 and separates the third list |
| AC-6 | `tests/corpus/props_r4133/README.md:514` says "RP2.3 filled `PROPS_ECHO_R4133` (81 rows)" against today's 82 | **REFUTED as an error**, recorded as an optional pointer (owner RP5.2) |
| AC-7 | `CLAUDE.md:196-198` "Stage F introduces no tolerance anywhere" vs the landed 2e-4 floor | **REFUTED as a contradiction**, recorded as a wording item (owner RP5.2) |

**The major one, and what closes it.** The RP5.1 record said "all 11
`oracle_parity_cfg_gate` tests are `ok` …, so every doc claim added here is
cited against a line the executable doc-walk can still find". It is not:
`operational_docs_cite_the_compat_machinery_accurately` checks a doc line that
spells the **compat tag** beside a `.rs` name, and every `compat::` alias a doc
names; its path extractor `rust_paths_in` stops at `.rs` and never reads a
`:LINE` suffix, and none of RP5.1's citations carries the tag or an alias. Zero
of them were reachable. Rather than downgrade the sentence to a confession, the
settlement makes it true: a **twelfth** test,
`oracle_parity_cfg_gate::operational_docs_line_citations_point_at_the_line_they_name`,
walks `TESTING.md` and `tests/TOLERANCE_NOTES.md`, resolves every
`file.rs:LINE` (and every bare `` `:LINE` `` continuation of one) the way a
reader does — by path suffix, disambiguating a shortened repeat against the
nearest fully-qualified spelling **earlier in the same document** — and asserts
three things: the path names exactly one file in the tree, the line is inside it
and not blank, and the line sits within ±3 of something the sentence itself
spells in backticks. A citation that resolves to no file, or to several, **fails**
rather than being skipped; per-doc floors (40 / 10) keep the walk from going
vacuous. It checks **58** citations today, and the record's sentence now states
the RP5.1-time truth (hand-verified) and hands the standing guarantee to this
test.

*Proved to fire*, three throwaway mutations of `TESTING.md`, each reverted
(`git diff --stat` back to the settlement's own 15/7):
`mod.rs:3243` → `:3143` reds with "lines 3140-3146 name none of
["compare_prop_lists", "capi_v0145"]"; `props_norm.rs:1772` → `:17720` reds with
"is past the end of … (5 186 lines)"; stripping the section's one fully-qualified
`harness/mod.rs` spelling reds with "names no file in the tree" and three
mis-resolutions. The instrument also **found a defect on its first run**:
`tests/TOLERANCE_NOTES.md`'s `midi` row cited `mod.rs:1090-1099` while naming
that arm only `` `_` ``, which anchors nothing — the row now names the `_`
fallback's `Tolerances`, the identifier the cited line actually spells.

**AC-1, settled by grep, not by reading.**
`crates/dss-core/tests/props_r4133_replay.rs` contains **zero** `env::var`
calls, so the offline half has no `DSS_GATE_ONLY` check and always runs; only
`assert_norm_rows_are_live` returns on it (`props_norm.rs:1413`), and that
function's own doc says the offline half "does not need the live gate at all".
`TESTING.md:680` already described the replay as unconditional, so the new
section had contradicted its own file. It now attributes the check to the live
half alone and says the replay reads frozen census files, not the gate
population.

**AC-3, settled by `git show`.** `NORM_ROWS` was **157** at `e96d9248` (RP2.1)
and at `1b93b91e` (RP2.1 audit settlement, which is where both prose sentences
entered), **161** at `ab2bf041` (RP2.2, which moved the table's other "157 rows"
mentions and missed these two) and **168** since `59e521e5` (RP4.1) — the commit
that *rewrote one of the two sentences while leaving it at 157*. So the record's
one git citation was wrong in both halves, and the more useful fact was the one
it hid.

**AC-4, settled by re-extraction.** Over RP5.1's added lines: **46**
`file.rs:LINE` tokens → **43** distinct (file, line) targets over **6** code
files, plus **9** bare `` `:LINE` `` continuations. The seventh file the record
named, `exec/tests/base_frequency.rs`, carries no citation row at all — it is
reached by pin *name* (`monitor_basefreq_inherits_the_fundamental`). No counting
rule yields 42/7. Corrected in the record and in `STATUS.md`.

**AC-5, settled at the seam.** `skip_prop` is a free function taking
`PropsChannel` (`mod.rs:2027`), called as `skip_prop(class, ename,
policy.channel())` (`mod.rs:3281`); links 2–4 are `PropsPolicy` methods behind
`is_r4133()`. Row 1's `LANE_SKIP_PROPS` half is channel-blind **on purpose** —
`Monitor.BaseFreq` is a bug both gating oracles share — so a blanket
"`is_r4133()`-gated" would have described the one link that must not be.

**AT-4, settled by the constants.** `SKIP_PROPS` (`mod.rs:1580`) has 17 rows and
is partitioned by `SKIP_PROPS_CAPI_ONLY` (10) + `SKIP_PROPS_BOTH_CHANNELS` (7);
`LANE_SKIP_PROPS` is a **separate** one-row list (`mod.rs:2017`) that
`every_skip_props_row_has_an_r4133_disposition` does not union. The table was
correct and misreadable; the prose now says 17 = 10 + 7 and why the third row is
shown anyway.

**AC-6 — refuted as an error.** `ECHO_ROWS` was **81** from `e15435fb` ("RP2.3:
the r4133 echo-exclusion table (81 rows)") through `cfca32a0`, and became **82**
at `fb0e9e7f` (RP3.3, `generator.model`). The README sentence sits under the
dated header "What RP2.3 moved (disposition census, 2026-08-23)", whose own
opening says it "**supplements** the RP2.1 and RP2.2 numbers above; it does not
rewrite them" — so 81 is true history, the same shape the plan's coordinator
decision 7 sanctions leaving historical. Not edited; an optional forward pointer
stays with RP5.2's cross-doc sweep.

**AC-7 — refuted as a contradiction.** `CLAUDE.md:196-198` makes two claims:
Stage F introduced no tolerance, and the default-lane **report** policy is
`rel = abs = 0`. `R4133_DISPLAY_FLOOR` is neither — RP2.4 introduced it, it
lives in the test harness's r4133 property claim chain, `claim_value` refuses
every channel but `R4133`, and it touches no `Tolerances` field, no `tol_for`
tier, no golden and no model quantity. The sentence is therefore not false, only
readable as broader than it is; the wording item stays owner RP5.2, unchanged
from the RP5.1 record.

**Gate — green in both lanes.** The five mandatory commands re-ran at
whole-workspace scope, no name filter, no `DSS_GATE_*` / `DSS_ORACLE_*` knob:
`cargo fmt --all --check` **0** (after one `cargo fmt --all` on the new test),
both `clippy` runs **0**, both test lanes **0** — **4 498 passed / 0 failed /
5 ignored / 0 filtered out** over 74 result-reporting binaries in each lane,
identical binary for binary and exactly **+1** on the RP5.1 baseline of 4 497:
the twelfth `oracle_parity_cfg_gate` test. `corpus_gate` ran whole (140 tests,
143.58 s) with `corpus_gate_all_cases_match_engines` **ok**; the five ignored
tests are the pre-existing set and the settlement added no `#[ignore]`.
`lane_diff` was **not** run and does not apply — the settlement moved no product
code (`git diff --name-only` matches nothing under any crate's `src/`); the one
non-documentation file is a test binary.

### RP5.1 — settlement record (ritual step 6, 2026-09-04)

Read end to end: `STATUS.md` (599 lines after this pass, from 602 — the
under-600 discipline holds) and this file. What RP5.1 had left stale, and what
moved:

- **STATUS section 1, the WP-RP5 paragraph** named only `dd0b9e5b`. It now names
  the settlement `8802fb6a` too and the post-settlement lane total (4 498 per
  lane), and is back inside the 3–6-line record-placement budget; the detail it
  shed is above, unchanged.
- **STATUS's "Next." paragraph** said RP5.2 owns "one handed-forward item". The
  settlement refuted **two** — `CLAUDE.md:196-198` (AC-7) and the vendored census
  README's historical "81 rows" (AC-6) — and recorded both as optional cross-doc
  pointers with owner RP5.2. Both are now named there.
- **The `CorpusGuard` leak follow-up** was at "eleven sightings"; the settlement
  run left five more deck-written `Auto3bus_*` files under
  `tests/corpus/electricdss-tst/Test/AutoTrans/`, which the settling agent could
  not delete (its sandbox refused the removal) and did not commit. Twelfth
  sighting recorded, files removed here, tracked corpus byte-unchanged.
- **STATUS's round-2 archive note** restated section 7's forwarding rule and
  spelled all three pin-citation guards by name where the section 7 table row
  already names them; condensed by four lines, every claim kept.
- **`TESTING.md`'s "Two rails" paragraph** described the compat-alias rail and
  `TORN_DOWN_ROWS` as the only two things keeping the operational docs honest.
  The settlement added a third — the `file.rs:LINE` citation walk — and the
  paragraph now says so, so a reader of `TESTING.md` learns their citations are
  checked without reading this record.
- **`R4133_PROPS_PLAN.md`** gained §RP5.1's dated landed line (shas, gate
  totals, pointer here) and the matching §0 note beside "RP5 is last".
- **Not touched, deliberately:** `tests/corpus/props_r4133/README.md` (AC-6 is
  true history under its own dated header — RP5.2's cross-doc sweep owns the
  optional pointer), `CLAUDE.md` (AC-7, same owner), and the pre-archive
  `STATUS §RPx.y` citations across the plan and the corpus README, which
  STATUS section 7's forwarding rule deliberately leaves unrewritten.

**Gate for this pass.** Documentation only — five `.md` files, not one line of
Rust. `cargo fmt --all --check` **0** and `cargo test -p dss-core --test
oracle_parity_cfg_gate` **12 passed / 0 failed** (both lanes), which is the
executable check that covers exactly what moved: the doc-walk and the new
line-citation walk both read `TESTING.md`. The five-command gate was not re-run
for a `.md`-only diff and `lane_diff` does not apply — no product code, no test
logic, no golden, no ledger, no tolerance moved by this commit.
