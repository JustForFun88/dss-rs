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
`tests/corpus/ledger.json`, `props_r4133_replay.rs:5428-5470`
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

---

### RP5.2 — closing record (2026-09-04)

**RP5.2 closed `R4133_PROPS_PLAN.md` and moved it to `docs/plans-archive/`** (the
2026-07 convention: a completed plan leaves the repo root, its condensed record
joins `era-summaries.md` §1a, and `PLAN_SEQUENCE.md` carries the COMPLETE row).
Documentation only, by design: zero product-crate lines, zero test *logic*, zero
table rows, zero tolerances, zero golden bytes, zero ledger entries, zero
manifest bytes. The only non-`.md` edit is a pair of **doc comments** in
`crates/dss-core/tests/props_r4133_replay.rs` (+5/−4, all `///` prose) whose plan
citation the move forced — and which were *already wrong* before it (see "Two
stale citations" below).

#### Final counters

Every number was re-measured on this tree at `64474762` (clean, branch
`r4133-props`); the command that produced each one is in `tmp/rp52/counters.md`.
Nothing is transcribed from the plan text.

| Counter | Final value | Where it is locked |
|---|---|---|
| normalization table `PROPS_NORM_R4133` | **168** rows — 77 `BoolFold` / 65 `CaseFold` / 21 `ArrayForm` / 5 `EnumSynonym` | `harness/props_norm.rs:742` (`NORM_ROWS`) + the four per-kind locks `:745`/`:750`/`:758`/`:764` |
| echo table `PROPS_ECHO_R4133` | **82** rows — 50 `EchoDefault` / 8 `EchoParse` / 14 `EchoEmptyCollection` / 10 `EchoLiveSemantics` | `props_norm.rs:2024` (`ECHO_ROWS`) + `:2026`/`:2028`/`:2030`/`:2032` |
| — rows living only on r4133-only cases | **58** rows over **34 971** cells / **833** cases | `R4133_ONLY_ROWS :2145`, `R4133_ONLY_CELLS :2147`, `R4133_ONLY_CASES :2155` |
| — per-cell narrowed pairs | **20** pairs over **66** spellings; **1** carve-out | `ECHO_NARROWED_PAIRS :2459`, `ECHO_NARROWED_SPELLINGS :2463`, `ECHO_CARVE_OUT_CELLS :2251` |
| tolerances introduced by the whole plan | **one** — the derived r4133 display floor **2e-4** | `R4133_DISPLAY_FLOOR = Some(2e-4)`, `props_norm.rs:895`; no `Tolerances` field or tier moved |
| ledger `tests/corpus/ledger.json` | **36 → 57** entries (**+21**, **none removed**) over **23 → 30** causes (8 added, 1 removed); `property`-scoped entries **21** (13 `capi_v0145` / **8** `r4133`) | measured `git show f887f806:tests/corpus/ledger.json` vs HEAD; the eight r4133 ones are exactly `LEDGER_ENTRY_PINS` (`props_r4133_replay.rs:5428`) |
| new ledger `match` field | **`variables`** (2 uses, both RP3.10) | `TESTING.md:431` |
| in-scope UNCLAIMED cells, r4133 channel | **521 841 → 889** (RP2.4) **→ 0** (RP4.1); 504 out-of-scope cells remain, each accounted to a named pin or record | the **→ 0** half is the live lock: an in-scope UNCLAIMED cell fails `corpus_gate`, and `assert_r4133_props_compare_ran` stops the walk going vacuous. The three census figures are *measurements* (`DSS_PROPS_CENSUS=claims`, both oracles), not constants in the tree — recorded at `r4133-props-rp4.md:96-124` (RP5.2 audit settlement, 2026-09-04) |
| **the gating-case outcome** | **523** manifest cases; **464** r4133-gating = **367** `both` + **97** r4133-only (plus 59 capi-only); 80 `kind=large`; the population `force_properties` actually compares = **313** non-`large` `both` cases | derived from `tests/corpus/manifests/population.lock.json` rigor fingerprints; the 313 is pinned by `the_property_forcing_rule_is_every_live_non_large_case` |
| RP3.x pin tables | `RP39_PINS` **27**, `RP310_WINDGEN_PINS` **5**, `RP311_SERIALIZATION_PINS` **11**, `RP312_UPSTREAM_BUG` **1** (plus 4 staged skips), `RP313_NCIM_PINS` **9**, `LEDGER_ENTRY_PINS` **8**, `LANDED_PROPERTY_ENTRY_PINS` **8** | `props_r4133_replay.rs:5572` / `:5315` / `:5109` / `:5751` / `:5220` / `:5428` / `:4983` (re-measured after the RP5.2 audit settlement moved lines below `:1341`) |
| tests | **4 499 passed / 0 failed / 5 ignored — per lane** at the plan's close, over 74 binaries; `props_r4133_pins` 54, `props_r4133_replay` 152, `props_r4133_evidence_lock` 11, `oracle_parity_cfg_gate` **13** | the closing commit's own gate measured **4 498** with 12 `oracle_parity_cfg_gate` tests (§"Gate for this pass", reproducing RP5.1's `8802fb6a` figures to the unit); the audit settlement's thirteenth test is the +1 (§"Gate for the settlement"). The 5 ignored are the pre-existing set — **no `#[ignore]` was added anywhere in this plan** (settlement record, ritual step 6, 2026-09-04) |
| commits | **26** sub-steps — 25 of them landed before this closing commit, which is the 26th — and **67** RP-titled commits at the plan's close: **64** in `f887f806..64474762`, plus RP5.2's `5a110653`, its audit settlement `bc16430b` and its settlement record (ritual step 6, 2026-09-04) (70 in the range; the other six are the plan's own round-2 hardening, GOLDEN_REBASE G1.2 and the STATUS round-2 archiving) | `git log --oneline f887f806..64474762`, the parent this record was measured at; `..HEAD` moves with every later commit and is *not* the range that yields these numbers (RP5.2 audit settlement, 2026-09-04) |

**The "462-case outcome" the plan asked RP5.2 to publish is stale by two, and is
published here as the measured 464.** The archived plan at `:266`, `:2616` and in
the RP5.2 section itself says "462 gating / 96 r4133-only / 366 both" — a 2026-08-22
authoring estimate. The population lock reads **464 / 97 / 367**. RP4.1 recorded
the correction as-executed (the archived plan at `:2736`,
`r4133-props-rp4.md:198`) and deliberately did not rewrite its historical text;
the closing counters therefore carry 464, together with the companion figure
RP4.1's audit corrected — the forced population is **313** non-`large` `both`
cases, not 367.

#### Recorded `lane_diff` runs

The plan owed `pwsh -File tools/lanes/lane_diff.ps1` at **RP1.2** and **RP1.3**
(the two sub-steps that change engine behavior: Y-invalidation via AutoTrans
`XfmrCode`, WindGen model dispatch). In practice it ran at every sub-step that
moved a `src/` line, and **every recorded run came back `max |Δ| = 0`** — so the
default lane stayed bit-identical to the parity lane across the whole plan, and
the transitive chain `|default − oracle| ≤ |default − parity| + |parity −
oracle|` keeps its measured-zero left term (CLAUDE.md §Gate).

| Sub-step | Owed? | Result | Record |
|---|---|---|---|
| **RP1.2** | **yes** | PASS — 522 cases / 3 220 247 records, max abs Δ = 0 on all eight kinds | `r4133-props-rp0-rp1.md:818-820` |
| **RP1.3** | **yes** | PASS — 522 cases / 3 220 247 records, max abs Δ = 0, zero iteration drift; both lane dumps byte-identical to the pre-edit baselines (default 226 437 002 B `a155aaa4…8672bbfe`, parity 226 437 001 B `dd7c5d59…226e65af`) | `r4133-props-rp0-rp1.md:952-958`, `:1019-1023` |
| RP3.5 + its settlement | settlement yes | PASS ×2 — max abs Δ = 0, 3 220 247 records / 522 cases | `r4133-props-rp3.md:1316-1320`, `:1506-1510` |
| RP3.6 (a), (b) + settlement | settlement yes | PASS ×3 — settlement run max abs Δ = 0.000e0 **and** max rel = 0.000e0 on all eight kinds | `r4133-props-rp3.md:1612-1616`, `:1763-1767`, `:1914-1918` |
| RP3.7 | yes | PASS — 523 cases / 3 220 861 records / ~4.83 M values, max abs Δ = 0.000e0 | `r4133-props-rp3.md:2329-2333` |
| RP3.8 / RP3.9 / RP3.11 / RP3.12 / RP3.13 | run wherever `src/` moved | PASS each | `r4133-props-rp3.md:2662`, `:2810`, `:3124`, `:3569`, `:3737`, `:4038`, `:4204` |
| **RP3.10** | **yes** (mandatory) | PASS — Δ = 0 | `r4133-props-rp3.md:4576`, `:4757`; `STATUS.md` §1 |
| RP4.1 | no (test-policy flip, no `src/` line) | run to completion anyway — PASS | `r4133-props-rp4.md:184` |
| RP5.1, RP5.2 | no — `.md` only | does not apply; no `src/` line moved | `r4133-props-rp5.md:138`, `:277`, `:321`, and this record |

**13+ recorded runs, zero non-zero deltas.**

#### The whole plan, sub-step by sub-step

*(the GOLDEN_REBASE WP-G2 condensed-record shape: one bullet per sub-step,
`(implement sha, fix sha, docs sha, date)`, then what it settled. Full records:
`r4133-props-rp0-rp1.md`, `-rp2.md`, `-rp3.md`, `-rp4.md`, and this file. Every
sub-step ran the full five-command gate green in **both** lanes before its
commit, and every one was audited by two fresh independent auditors plus a
dedicated fix agent — no batching, no shared fix agent across sub-steps.)*

- **RP0.1** (`cbcfafeb`, fix `19fc68d7`, 2026-08-22) — the G1.1 census evidence
  vendored to `tests/corpus/props_r4133/`: five byte-identical copies of the
  gitignored `investigations/g1_1_r4133_props/` extracts (24 944 B) plus five
  derivations of the 270 MiB local census, with a `.gitattributes -text` rule so
  nothing is rewritten on checkout. The plan's load-bearing evidence stopped
  being local-only.
- **RP0.2** (`fd79db48`, fix `23070130`, docs `8dde5802`, 2026-08-22) — the
  census became a permanent knob: `DSS_PROPS_CENSUS=1` arms
  `corpus_gate_props_census`, a separate `#[test]` that walks every live
  non-`large` case on **both** channels with `all_properties` forced on, masks
  bypassed, collect-don't-panic, asserting nothing. It reproduced the vendored
  rows on 209 of 210 structural pairs and all 94 numeric ones over the full
  438-case re-census.
- **RP1.1** (`e2415376`, fix `9fb45c6c`, docs `fccd7915`, 2026-08-22) — the three
  r4133 **upstream-stub** rows: Generator `Rneut`/`Xneut` (display slots 16/17)
  and Sensor `Action` (slot 13). Two shape gaps closed (generator 48 → 50 names,
  sensor 15 → 16) via one new property flag and one new `PropDef` field.
- **RP1.2** (`8a221016`, fix `0a23443d`, 2026-08-22) — AutoTrans `XfmrCode`, a
  **real behavioral port** (r4133 `AutoTrans.pas:329`, help `:414`,
  `FetchXfmrCode` `:520` → `:2339-2396`; dss_capi 0.14.5 deleted the row
  outright). Third shape gap closed, autotrans 52 → 53 names. `lane_diff` PASS,
  max abs Δ = 0.
- **RP1.3** (`c345434c`, fix `bdbdc63f`, docs `9b641a18`, 2026-08-23) — WindGen
  `UserModel`/`UserData`, a **real behavioral port over the WASM user-model
  host** (r4133 `WindGen.pas:391-395`, Edit arms `:641-642`, `MakeLike` `:829` +
  `:834-835`, getter `:2903`), with the whole `Model=6` behavior they feed.
  Fourth shape gap closed. `lane_diff` PASS, max abs Δ = 0, both lane dumps
  byte-identical to the pre-edit baselines. Models **3** and **7** were
  explicitly out of scope and handed to `ORPHANED_GAPS.md` §1.11 (below).
- **RP1.4** (`d3077994`, fix `d404ff30`, 2026-08-23) — GenDispatcher `weights`:
  the allowlist row plus an upstream report. **Zero engine change** — the port is
  right and matches dss_capi; the fifth and last `shape.txt` row is the only one
  that runs *backwards*, a property the port has and r4133's own table loses.
  **WP-RP1 COMPLETE: r4133 shape classes 5 → 0.**
- **RP2.1** (`e96d9248`, fix `1b93b91e`, 2026-08-23) — channel threading, the
  normalization engine, the replay accounting and the census's **disposition**
  mode (`DSS_PROPS_CENSUS=claims`). Zero engine change: the whole sub-step is
  harness + tests + vendored evidence + docs, and no ledger entry (the §1.1(e)
  staging rule).
- **RP2.2** (`ab2bf041`, fix `94db1542`, 2026-08-23) — enum synonyms and the S6
  dossier. Zero engine change; the dossier is what opened RP3.5, RP3.6 and RP3.7.
- **RP2.3** (`e15435fb`, fix `d7881fd4`, 2026-08-23) — the echo-exclusion table
  and its pins. Zero engine change, `R4133_DISPLAY_FLOOR` still `None`; the kill
  ruling taken here re-routed five pairs to RP3.8.
- **RP2.4** (`256e33e6`, fix `cfca32a0`, 2026-08-23) — **the r4133 props display
  floor; WP-RP2 closes with it.** The 2e-4 value is a *measurement*, not the
  plan's paragraph: worst display rel confirmed at 6.431124e-05 (`load.pf`,
  33 cells, in scope) against a nearest genuine value jump of 1.374769e-03
  (`storagecontroller.kwneed`, 6.874× the floor). Zero engine change, zero
  product-crate bytes, no `Tolerances` field or tier touched. In-scope UNCLAIMED
  cells **521 841 → 889**, every survivor attributed to an open RP3.x sub-step.
- **RP3.1** (`9319d427`, fix `3e878d30`, 2026-08-24) — `swtcontrol.delay`: a
  wired property that **r4133 silently ignores**. Zero product-crate bytes (the
  port already behaves correctly) and zero `ledger.json` bytes in this commit —
  the §1.1(e) **staging rule**: pins land in the sub-step, the entries themselves
  in RP4.1's unmask commit, because an entry landed earlier would fail
  `assert_all_hit` as NEVER APPLIED while the r4133 props compare is still masked.
- **RP3.2** (`c46bca42`, fix `24a3298f`, 2026-08-24) — `windgen.kvar`: a wired
  property whose **r4133 getter reads the wrong live field**. Zero product-crate
  bytes, staged entry. Its audit settlement opened **RP3.10** and made RP3.10's
  closure a precondition of RP5.2 itself.
- **RP3.3** (`fb0e9e7f`, fix `52770c5f`, 2026-08-24) — `generator.model`: a
  getter with no arm, echoing the deck's own token past a live conversion. Zero
  product-crate bytes (both engines' live state is identical). Its settlement
  opened **RP3.11**.
- **RP3.4** (`cab2e667`, fix `6821639a`, 2026-08-24) — `gictransformer.r2`: the
  r4133 twin of an already-fixed, already-pinned divergence (GOLDEN_REBASE G2.5
  fixed the engine in both lanes on 2026-08-06). Zero product-crate bytes.
- **RP3.5** (`9daff660`, fix `de2c179d`, docs `feea6a6a`, 2026-08-28) —
  `line.units`: outcome **`FIX` in both lanes** for three independent defects in
  one routine (`TLineObj.MergeWith`), plus one upstream report for a fourth that
  is r4133's alone. The first RP3 sub-step whose fix moves the port's own render.
  `lane_diff` PASS ×2, max abs Δ = 0.
- **RP3.6** (`f0837aca` part (a) + `4b146ab9` part (b), fix `bb467974`,
  2026-08-29) — `switch=yes` must not clear the linecode flag: the second
  **`FIX`**, and the one RP4.1 actually waits on (r4133 `Line.pas:694-700`), plus
  the `FLineCodeSpecified`/`CondCode` split and the CIM-units follow-on. Fourteen
  audit findings settled. `lane_diff` PASS ×3 — the settlement run reports max
  abs Δ = 0.000e0 **and** max rel = 0.000e0.
- **RP3.7** (`82022dab`, fix `1273cf14`, docs `d0718fb0`, 2026-09-02) — per-phase
  switch and relay state: **`FIX` in both lanes for all three parts**, and the
  widest RP3 sub-step — 50 engine/test/golden/ledger files, the two control
  classes rebuilt on r4133's `pStateArray` model, ten overlaid golden cells, five
  live pins. Eleven audit findings, all minor, all settled. It also spun out four
  `ORPHANED_GAPS.md` rows (§1.13–§1.16). `lane_diff` PASS, 523 cases.
- **RP3.8** (`17a165c4`, fix `6b9b115d`, docs `16edbc2f`, 2026-09-02) — the five
  read-only text surfaces r4133 renders live (`indmach012.pf`,
  `storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`, …): **`FIX` in both lanes**,
  one engine flag, 25 files, **zero golden bytes moved**. The settlement found
  `Save` as a fifth `get_value` reader and fixed it.
- **RP3.9** (`648ce284`, fix `d8ccc954`, docs `b2ca9c11`, 2026-09-02) — the r4133
  round-trip residue: **`PRECISION_ROUNDTRIP` on all 27 pairs**, in one commit,
  with zero product-crate lines, zero golden bytes and zero census cells moved.
  Its P0 open item is what opened **RP3.12**.
- **RP3.10** (`9f55095b`, fix `9f067c19`, docs `45f2ece3`, 2026-09-04) — the
  reproduced `QMode=0` dispatch: verdict **`FIX`**, and the kill criterion did
  **not** fire. The last reproduced upstream bug this plan uncovered is gone from
  **both** lanes — `TWindGenObj.SetNominalGeneration` gains the constant-Q arm
  r4133 never wrote — at the price of four r4133 `exclusion` entries, the new
  `variables` ledger `match` field and five pins behind a citation guard. Zero
  golden bytes; `lane_diff` PASS, Δ = 0. Ten findings settled; it opened one
  standing follow-up (AT-1, the two WindGen decks' thinned solved-state coverage).
- **RP3.11** (`97107e54`, fix `0194b086`, docs `9fbb0abf`, 2026-09-03) — the
  `Save`/`Dump` re-serialization surface: **`KEEP_LIVE_PINNED` on both surfaces**
  — the kill criterion fires on the one cell that decides it, so the divergence
  from r4133's serializer is recorded and pinned rather than reproduced. Eight
  pins, one golden; the settlement shipped the `SaveWrite` union it had only
  written. Its P0 findings opened **RP3.13**.
- **RP3.12** (`09e70233`, fix `c2a8b68c`, docs `28f37e4c`, 2026-09-03) —
  `autotrans.wdgcurrents` on the regulator decks: **`UPSTREAM_BUG` in r4133,
  never reproduced**, with zero product-crate lines. One pair, cause
  `regcontrol-autotrans-typecast`, minimum gap ratio 153.0 over 2 witnessed rows,
  plus four staged skips.
- **RP3.13** (`2ce1a66e`, fix `217355da`, docs `185e8648`, 2026-09-03) — the two
  NCIM port bugs: verdict **`PORT_BUG` × 2, both fixed lane-unconditionally**,
  with zero `ledger.json` entries, zero golden bytes and zero tolerances moved
  (nine pins). Three further r4133 NCIM defects were proven and **not** reproduced
  (`docs/upgrade/DIVERGENCES.md:1995`). **WP-RP3 COMPLETE, 13/13.**
- **RP4.1** (`59e521e5`, fix `6d787a8f`, docs `762df50b`, 2026-09-03) —
  **`all_properties` unmasked on the r4133 channel: WP-RP4's single sub-step and
  G1.1's deliverable**, in one commit, with zero product-crate lines, zero golden
  bytes and zero tolerances moved. The per-channel `compare_all_properties =
  false` clears are gone from *both* the gate path and the seeding path
  (`corpus_gate/scheduler.rs`), and `force_properties` now forces the property
  request on every live non-`large` case. The eight staged r4133 `property`
  entries landed in this commit (pinned by
  `the_staged_r4133_property_entries_landed_at_rp41`). **The re-armed kill
  criterion did not fire**; acceptance was read off the lossless census — r4133
  in-scope UNCLAIMED **0**, 504 out-of-scope accounted per owner, and the capi
  channel A/B **bit-identical** (8/8 artifacts byte-equal to the pre-flip
  baseline).
- **RP5.1** (`dd0b9e5b`, fix `8802fb6a`, docs `64474762`, 2026-09-04) — the
  operational docs: the four-link claim chain (normalize → echo → floor →
  assert), the property-divergence triage procedure, the `SKIP_PROPS` r4133
  disposition table, 46 `file.rs:LINE` citations. Its settlement turned those
  citations from a hand-verification into
  `operational_docs_line_citations_point_at_the_line_they_name`, a twelfth
  `oracle_parity_cfg_gate` test that resolves and anchors all **58** citations in
  `TESTING.md` + `tests/TOLERANCE_NOTES.md` on every run.
- **RP5.2** (this record, 2026-09-04) — the closing record and the archive move.

#### What RP5.2 itself changed

| File | What |
|---|---|
| `R4133_PROPS_PLAN.md` → `docs/plans-archive/R4133_PROPS_PLAN.md` | `git mv`, plus a seven-line ARCHIVED banner under the H1 in the blockquote style the other archived plans use (naming this record and the archived §1a record), plus §RP5.2's own dated **Landed 2026-09-04** line at the foot — the closing marker every other section carries. +27/−0 |
| `PLAN_SEQUENCE.md` | row **5b** flipped **IN FLIGHT → COMPLETE 2026-09-04** with the final counters and the measured gating-case outcome; row **5a**'s G1.1 hand-off note now says the successor COMPLETED it, so **G1.1 is satisfied** and **G3.4** + **G3.5** are unblocked; the "Plan file locations" note moves the plan from the root list to the archived list |
| `GOLDEN_REBASE_PLAN.md` | §G1.1's superseded-by note gains a dated **SATISFIED** paragraph, and §G3.4 / §G3.5's "this sub-step waits for it" become "unblocked" — the one cross-doc disagreement the closing record's acceptance would otherwise have left standing |
| `docs/plans-archive/README.md` | the eleventh table row, the header's date note, and `GOLDEN_REBASE_PLAN.md` added to the "still at the repo root" list (it was missing there) |
| `docs/phase-records/r4133-props-rp5.md` | this record |
| `docs/phase-records/era-summaries.md` | the archived-plan record, in the shape of the other archived-plan records |
| `STATUS.md` | section 1: "In flight." becomes the plan-complete sentence; the WP-RP5 paragraph gains RP5.2; the WP-G1 paragraph records G1.1 as satisfied and G3.4/G3.5 as unblocked; "Next." becomes the hand-back to GOLDEN_REBASE WP-G1; section 7's `era-summaries.md` row names the R4133_PROPS archived-plan record. To hold the file near its accustomed working length (it was at **599** lines; the ~600-line habit is a discipline this round kept, not a rule any doc or test states — RP5.2 audit settlement, 2026-09-04), the three now-closed **WP-RP0 / WP-RP1 / WP-RP2** work-package paragraphs were condensed into one — nothing is lost (their full records are in `r4133-props-rp0-rp1.md` and `-rp2.md`, and the condensed table above), and their two stale counts were corrected on the way: the normalization table reads **168** (not 157) and the echo table **82** (not 81) rows at close. Section 2's two open items that named RP5.2 as their owner got their dated disposition (accepted / handed on — see below), and section 7's `r4133-props-rp5.md` row now names this closing record. Length at the closing commit **598** lines |
| `ORPHANED_GAPS.md` | §1.11's "Deferred by" line now names the archived path (a verification pass — no owed row was missing) |
| `crates/dss-core/tests/props_r4133_replay.rs` | **two doc comments only** (+5/−4 lines of `///` prose) — the plan citation repointed and its line range corrected |

**Two stale citations, corrected because the move forced the edit.**
`props_r4133_replay.rs:1341` and `:6519` (`:1338` / `:6510` before the settlement) both cited
`` `R4133_PROPS_PLAN.md:1084-1090` `` for the WP-RP3 sanctioned-outcome sentence.
That range is the **RP2.4 display-floor derivation**; the sentence they quote
("…exactly one outcome — a port bug **fixed in both lanes**, or an upstream/echo
divergence **excluded + pinned** …, or an upstream bug **reported** with its
exclusion + pin") is at `:1148-1151` of the archived file (the banner shifts
every line by 7). Nothing gated this: `LINE_CITED_DOCS`
(`oracle_parity_cfg_gate.rs:3349-3353`) covers only `TESTING.md` and
`tests/TOLERANCE_NOTES.md`, so RP5.1's new line-citation walk does not reach a
`.rs` doc comment. Both now read
`docs/plans-archive/R4133_PROPS_PLAN.md:1148-1151`.

**The move breaks no test — verified, not assumed.** `operational_docs()`
(`oracle_parity_cfg_gate.rs:2980-3059`) names five fixed files — `CLAUDE.md`,
`TESTING.md`, `tests/TOLERANCE_NOTES.md`, `tests/corpus/ledger.json`,
`tests/corpus/props_r4133/README.md` — plus the `tests/corpus` and `tools` walks;
`docs/` is **not** walked and the plan is not in the list, so neither the old nor
the new location is gated. No Rust source opens the plan (the two
`"R4133_PROPS_PLAN` hits in `props_r4133_replay.rs` are assertion-message
*content*, not paths), the three pin-citation guards read
`docs/phase-records/r4133-props-rp3.md`, and **no Markdown file links the plan**
— every one of the other references is a bare-name prose citation the move leaves
valid. `.gitattributes:8` names the plan in a **comment**; its EOL patterns key
on `tests/corpus/props_r4133/**` and are untouched.

**CLAUDE.md was not edited.** `rg -c R4133_PROPS_PLAN CLAUDE.md` = **0** — it
never named the plan path — so the instruction "if and only if CLAUDE.md names
the plan path, edit that one sentence" does not fire.

#### The §1.3 deferrals — verified; one row owed, one row present

The archived plan's §1.3 lists five out-of-scope items; exactly one was
instructed to land an `ORPHANED_GAPS.md` row, and it is there:

| §1.3 item | Owed a row? | State |
|---|---|---|
| **WindGen power-flow models 3 (`DoPVTypeGen`) and 7 (`DoCurrentLimitedPQ`)** | **yes** | **present** — `ORPHANED_GAPS.md` §1.11, with r4133 citations (`WindGen.pas:2109-2118`, `:1413`, `:1406-1408`, `:1335-1345`), the current-Rust admission gate (the `WindGen: Model` enum lists `&[1, 2, 4, 5, 6]`), a "do NOT transcribe" trap note for model 7's never-assigned limit fields, and the local report `investigations/to_opendss/39-windgen-model7-uninitialized-current-limit.md`. Cross-referenced from `windgen/solve.rs:313` and `registry/pc.rs:207` |
| the 59 capi-only cases' r4133 behavior | no — owned per case by the manifests' `engines` key | n/a |
| Report/Dump/Save rendering | no — GOLDEN_REBASE WP-G3/G4 | n/a (RP3.8 and RP3.11 did touch `Save`; that is recorded, not deferred) |
| the `props/` golden migration to self-anchors | no — GOLDEN_REBASE **G3.5** | now unblocked; stated in `PLAN_SEQUENCE.md` row 5a and in STATUS |
| no tolerance tier moves | no | held — the 2e-4 floor is a new named parameter; no `Tolerances` field or tier moved |

**Nothing was added to `ORPHANED_GAPS.md`.** In particular the RP5.2
precondition's fallback ("if the user has not sanctioned the RP3.10 fix, it moves
to `ORPHANED_GAPS.md` with its evidence instead of vanishing with the plan") is
**moot**: RP3.10 was executed as a `FIX` in both lanes on 2026-09-04, so there
was nothing to park. The one item RP3.10 *did* open — AT-1, the two
`modes:windgen/*` decks whose r4133-only gating now compares almost no solved
state — is a standing follow-up in `STATUS.md`, which is where an open item with
a named fix belongs, not an orphan.

#### Cross-doc state at close

- **RP5.1's two handed-forward wording items**, both **refuted** as errors by its
  own settlement and kept only as optional pointers, are recorded here as
  *deliberately not edited*: (1) `CLAUDE.md:196-198`'s "Stage F introduces **no**
  tolerance anywhere" is true **of Stage F** — RP2.4's 2e-4 display floor is a
  later, channel-scoped parameter, not a Stage F tolerance, so no qualifying
  clause is owed; (2) `tests/corpus/props_r4133/README.md`'s "81 rows" is a
  **historical RP2.3-time** figure under its own dated header, while the live
  count-lock is `ECHO_ROWS = 82` (`props_norm.rs:2024`). Editing the README would
  also touch a file `operational_docs()` gates, for no gain.
- **Landing-marker wording is not uniform across the archived plan's sections** —
  RP0.1/RP0.2/RP1.1–RP1.4/RP2.1/RP2.4/RP3.6 use "As executed"/"Outcome" prose
  rather than a dated "**Landed**" line, and RP0.1 carries no dated line inside
  the plan at all (its record is in `r4133-props-rp0-rp1.md`). Recorded, not
  rewritten: the plan is frozen history now, and the per-sub-step table above is
  the uniform index it lacked.
- No doc disagrees with another on a count or a state: the counters above are the
  measured ones, `PLAN_SEQUENCE.md` row 5b carries them, `STATUS.md` section 1
  points here, and the archived §1a record in `era-summaries.md` repeats the same
  numbers. The one disagreement the archive move would have left standing —
  `GOLDEN_REBASE_PLAN.md` §G1.1 still reading "superseded … G3.4/G3.5 wait for
  RP4.1", and §G3.4/§G3.5 each reading "this sub-step waits for it" — was closed
  in the same commit with a dated **SATISFIED** paragraph and two "unblocked"
  edits, so the successor plan's own text now agrees with `PLAN_SEQUENCE.md` row
  5a and with STATUS.

#### The two items STATUS owed to RP5.2 — disposed

`STATUS.md` §2 carried two open items whose named owner was this sub-step. Both
are disposed here, neither is deleted:

- **The 54 `kind=large*` `engines: both` cases with no property compare on either
  channel (RP4.1 audit settlement, 2026-09-03) — DECIDED: accepted permanently.**
  `scheduler::force_properties`'s `!kind.starts_with("large")` guard is the
  plan's own §1.3 cost rule, not a drift: of the 367 `both` cases **313** compare
  their property table, and the forced population is pinned
  (`FORCED_PROPS_POPULATION = (440, 313, 83, 44)`, asserted by
  `the_property_forcing_rule_is_every_live_non_large_case`), so the gap is
  measured, bounded and locked. Pricing a `large`-deck property sweep — the
  `?`-sweep on the biggest feeders is the whole reason for the guard — stays
  available to GOLDEN_REBASE, but nothing in this plan is owed it.
- **`DECLARED_RP35`'s four remaining declared pairs (`line.units`,
  `line.linecode`, `relay.normal`, `relay.state`, `DECLARED_RP35 = (5, 4, 2)`) —
  NOT closed here, handed on with its evidence.** Retiring a declared row owes a
  per-pair live disposition, a cited r4133 getter arm and a pin — test logic,
  which a documentation-only sub-step must not write; and the census shows no
  divergent cell for any of the four, so nothing is unguarded in the meantime.
  The item stays open in `STATUS.md` §2 with this record named as its evidence,
  handed to whoever closes the WP-RP3 accounting inside GOLDEN_REBASE WP-G1.

#### Gate for this pass

Documentation only — seven `.md` files (one of them the `git mv`-ed plan itself)
plus **two doc comments** in one test file, `props_r4133_replay.rs` at +5/-4, all
of it `///` prose; **not one line under any crate's `src/`**. The five-command
gate was nonetheless run in full on the tree as it stood at 08:11 (`64474762` +
the working copy — see the settlement note under the table for the four `.md`
files written after it), each command's exit code read individually:

| # | command | exit | wall |
|---|---|---|---|
| 1 | `cargo fmt --all --check` | **0** | 2.8 s |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | **0** | 2.6 s |
| 3 | `cargo clippy --workspace --all-targets --features dss-core/oracle-parity -- -D warnings` | **0** | 7.6 s |
| 4 | `cargo test --workspace` | **0** | 242 s |
| 5 | `cargo test --workspace --features dss-core/oracle-parity` | **0** | 273 s |

**Both lanes: 4 498 passed / 0 failed / 5 ignored / 0 filtered out over 74
binaries** — identical binary for binary, and a delta of **0** against the RP5.1
baseline at `8802fb6a`, which is the predicted outcome for a documentation-only
sub-step. `corpus_gate` ran its whole 140-test binary at full population on both
oracle channels (155.3 s default / 146.7 s parity) with all four ledger guards
`ok`: **no ledger entry stale, none NEVER-APPLIED**, zero reds on either channel.
`grep -c '^warning'` over both clippy logs = **0**, and both printed `Checking
dss-core` rather than a warm no-op, so the current file contents really were
re-checked. The five ignored are the pre-existing set (three WASM/WM5 golden
generators, the ckt24 `.graph` diagnostic, one doc-test) — `rg '#\[ignore'
crates/ --include=*.rs` returns exactly those, so **RP5.2 added no `#[ignore]`
and no name filter**. `lane_diff` is not owed and does not apply: `git diff
--stat -- 'crates/*/src'` is empty. The gate's 8 untracked `Export` byproducts
under `tests/corpus/electricdss-tst/Test/AutoTrans/` were deleted afterwards; no
tracked corpus or golden file was modified at any point. Logs: `tmp/rp52/gate.md`.

**What that run did and did not see** (RP5.2 audit settlement, 2026-09-04). The
sentence this paragraph first carried — "the tree is byte-for-byte the one that
passed" — is false, and the audit measured it: the run ended **08:19:50**, and
four of the nine committed paths were written after it (`STATUS.md` 08:25:27,
`docs/plans-archive/R4133_PROPS_PLAN.md` 08:26:17, `GOLDEN_REBASE_PLAN.md`
08:27:01, this record 08:27:23; the commit at 08:27:46). The exposure is nil and
was checked rather than assumed: the one `.rs` edit predates the run (07:49:26),
so `cargo fmt` and both clippy passes covered it, and at that point no test read
any of the four — `operational_docs()` names five fixed files, none of them a plan
or a phase record, and the only `.md` opened anywhere in the test tree was
`docs/phase-records/r4133-props-rp3.md`. The standing fix is not a reworded
sentence: the settlement commit re-ran the full five-command gate on the final
content, and its new guard
`oracle_parity_cfg_gate::rust_comments_citing_a_record_line_point_at_the_passage_they_name`
**does** read `docs/plans-archive/R4133_PROPS_PLAN.md` on every run, so the
archived plan is no longer a file the gate cannot see.

**One commit — plus its settlement.** RP5.2 lands as a single commit — implement, gate and record
together — with no fix commit and no follow-on `docs:` commit: the sub-step is
documentation only, the gate was green on the first run, and the record was
written against the gate's own measured totals rather than a prediction. Its
audit round then landed a second commit — the settlement below, which is where
the "no fix commit" half of that sentence stops being true.

### RP5.2 — audit settlement (2026-09-04)

Two fresh auditors (`/audit-code`, `/audit-tests`) read `64474762..5a110653` —
the closing commit. Neither found a lost deliverable, a weakened test, a moved
tolerance or a reproduced bug, and both re-derived **every** counter the closing
record publishes off the tree independently (normalization 168, echo 82, ledger
36 → 57 over 23 → 30 causes, 523 / 464 / 367 / 97 / 59, `FORCED_PROPS_POPULATION
= (440, 313, 83, 44)`, 4 498 / 0 / 5 per lane, all 64 sub-step SHAs, both
preconditions discharged in code). What they returned is **fourteen findings —
nothing above Minor**: two guard gaps that are real protection holes, four
accuracy defects in the record's own self-description, two live cross-doc count
contradictions, and six notes. Each is settled below against the tree, never
against plausibility; twelve are fixed, one is recorded as deliberately not
fixed, and none is dropped.

| # | finding | disposition |
|---|---|---|
| AT-1 (minor) | RP5.2's only code line — the two repointed `R4133_PROPS_PLAN.md:1148-1151` citations — is guarded by nothing: the citation walk drops every non-`.rs` citation (`oracle_parity_cfg_gate.rs:3541`) and never reads Rust comments | **FIXED** — a thirteenth test, `rust_comments_citing_a_record_line_point_at_the_passage_they_name`, resolves the **7** `record.md:LINE` citations the test tree carries and anchors each one; proved to fire twice |
| AT-2 (minor) | "resolves *and anchors* all 58" was true of 52: an unanchored citation silently downgraded to existence-only (`:3595`), and the six in that hole were exactly the anchors the counters table leans on | **FIXED** — the silent `continue` is now a failure, and the six citing lines were re-spelled so the symbol is backticked; the walk fails on all six before the re-spelling and passes after, so the claim is now enforced, not asserted |
| AT-6 (note) | three RP3 pin tables lock `>= n`, not `== n`, while the records state exact sizes | **FIXED** — `RP311_SERIALIZATION_PINS` 11, `RP313_NCIM_PINS` 9, `RP310_WINDGEN_PINS` 5 are `assert_eq!` with the reason in the message, matching how `LEDGER_ENTRY_PINS` is pinned |
| AC-1 (minor) | "the tree is byte-for-byte the one that passed" is false — four of the nine committed paths were written after the gate ended | **FIXED** — the gate section now states what that run did and did not see, with the measured timestamps; the standing fix is that this settlement's gate ran on the final content and the new guard reads the archived plan on every run |
| AC-2 / AT-4 (minor) | `GOLDEN_REBASE_PLAN.md:404` — an **active** root plan — still said "the 96 r4133-only cases", six lines above the paragraph this commit inserted, against RP5.2's own "no doc disagrees on counts" acceptance | **FIXED** — a dated as-executed correction to **97**, derived here from `population.lock.json` (523 = 367 `both` + 97 `r4133` + 59 `capi_v0145`, i.e. 464 gating) |
| AC-3 (minor) | "25 sub-steps" contradicts the record's own 26-bullet list, in four documents | **FIXED** — the plan defines **26** (`grep -c '^### RP'` = 26); every place now reads 26, with the record's counters cell spelling out that 25 landed before the closing commit |
| AC-4 (minor) | the plan-move diffstat is recorded as +26/−0; `git diff --numstat` says +27/−0 | **FIXED** — +27/−0 |
| AT-3 (minor) | the commit counter cites `f887f806..HEAD`, a range that now yields 71 / 65 / 26 rather than the stated 70 / 64 / 25 | **FIXED** — re-spelled `f887f806..64474762`, matching `era-summaries.md`, with a note that `..HEAD` moves |
| AC-5 (note) | the "600-line ceiling" cited to justify condensing three WP paragraphs exists in no binding document | **FIXED** — re-worded as the working-length habit it is; the condensation itself stands (the removed specifics survive in `-rp0-rp1.md` and `-rp2.md`, spot-checked) |
| AC-6 (note) | the archived-plan record's WP-RP3 sentence says "closed all thirteen genuine jumps" but enumerates nine | **FIXED** — the four excluded-and-pinned divergences (RP3.1–RP3.4) are named, and the sentence closes 6 + 4 + 1 + 1 + 1 = 13 |
| AC-7 (note) | the repaired "still at the repo root" list still omits `UNIFIED_GATE_PLAN.md` | **FIXED for the list** (added, with its record named); the plan's own stale `Status: PLANNED` banner is **RECORDED** in `STATUS.md` §2 — flipping another plan's lifecycle belongs to its owner |
| AT-5 (note) | one counters cell's "Where it is locked" column names a record, not a lock | **FIXED** — the cell now names the live lock (`corpus_gate` fails an in-scope UNCLAIMED cell; `assert_r4133_props_compare_ran` blocks a vacuous walk) and marks the three figures as measurements |
| AC-8 (note) | RP5.2's six-line insert widened an already-stale cross-file citation in the archived plan (§0 cites `GOLDEN_REBASE_PLAN.md:928`/`:941` for G3.4/G3.5; the headers are now `:962`/`:975`) | **RECORDED, deliberately not fixed** — the archived plan is frozen history under its own §"as-executed line-number corrections are deliberately not edited" rule, and the drift pre-dates RP5.2 by 25 lines; the sentence names the sub-steps, which is what a reader follows. Recorded here so the next reader of §0 knows the two numbers are stale |
| — | the record's "no fix commit and no follow-on `docs:` commit" | **superseded by this settlement**, and the sentence now says so |

**The two guard gaps, and what closes them.** They are one gap seen from two
sides: a citation is only worth its anchor.

- *A record citation in Rust was checked by nobody.* The RP5.1 walk resolves
  `.rs` targets and `continue`s past every other extension, so
  `props_r4133_replay.rs`'s two citations into the plan — the ones RP5.2 itself
  had to repoint because the range had drifted onto the RP2.4 display-floor
  derivation — were unguarded, as were the five `tests/TOLERANCE_NOTES.md:987-993`
  citations in `harness/mod.rs`. The new test resolves all **7**: the record
  exists, the range is inside it and not blank, and the passage is *anchored* —
  the comment quotes it (the quotation's opening words, cut at the first ellipsis,
  must appear in the cited lines), or names a `§`section whose span contains the
  range, or shares a backticked symbol with it. *Proved to fire*, two throwaway
  mutations, both reverted: moving one citation to `:1248-1251` (out of §WP-RP3)
  **reds**; moving the quoting one 12 lines *inside* the same section, to
  `:1160-1163`, also **reds** — "the comment quotes ["exactly one outcome — a
  port bug fixed"] and the cited lines say none of it".
- *An unanchored citation was checked for existence only.* `if idents.is_empty()
  { continue; }` meant any non-blank line of the right file passed. Six of the 58
  sat there — `TESTING.md:880`/`:907`/`:946`/`:990` and
  `TOLERANCE_NOTES.md:1359`/`:1362`, i.e. the normalization table, the echo table,
  the display floor and `FORCED_PROPS_POPULATION`. All four were unanchorable for
  the same silly reason: the symbol was inside a backtick span that also carried
  `= (440, 313, 83, 44)`, which the ident extractor rejects. Re-spelling the six
  (`` `FORCED_PROPS_POPULATION` = (440, …) ``) anchors them, and the skip is now a
  failure, so the hole cannot reopen. Measured before and after: **6 red → 0**,
  58 citations checked, floors 47 / 11 unchanged.

**Documented where the rails are documented.** `TESTING.md`'s "three rails keep
the operational documents honest" paragraph now reads **four** and describes both
citation directions — the doc-to-code walk (with the unanchored case now failing)
and the new code-to-record one. The rewrite is **line-count-neutral** (17 changed
/ 17), because nine other citations point into `TESTING.md` by line number and
none of them is guarded by anything.

**One correction the guards forced.** Anchoring the two `harness/mod.rs`
comments first cost two lines, and the citation walk immediately reddened —
`TESTING.md:863`'s `mod.rs:3489` had become a blank line. Both edits were
re-done line-count-neutral (3 changed / 3 unchanged), which is the discipline a
line-cited file owes its citers. The `props_r4133_replay.rs` edits could not be
made neutral (a quoted anchor and three `assert!` → `assert_eq!` rewrites), so
the nine record citations below them were **re-measured** rather than assumed:
`:4980 → :4983`, `:5106 → :5109`, `:5215 → :5220`, `:5308 → :5315`, `:5419 →
:5428` (`:5419-5461 → :5428-5470`), `:5563 → :5572`, `:5742 → :5751`, and the two
citation sites themselves `:1338 → :1341`, `:6510 → :6519` — each verified to
name its symbol again.

**Gate for the settlement.** The full five-command gate, both lanes, run after
every code and doc byte of this commit was written **except this paragraph**,
which reports the run and therefore cannot precede it — the one file still
written afterwards is this record, and no test reads it (the three pin guards
read `docs/phase-records/r4133-props-rp3.md`; `operational_docs()` reads
`CLAUDE.md`, `TESTING.md`, `tests/TOLERANCE_NOTES.md`, `tests/corpus/ledger.json`
and `tests/corpus/props_r4133/README.md`, all of them edited before the run or
not at all). That is the honest form of the claim AC-1 caught, and it is stated
rather than rounded up. All five exit **0**
(`tmp/rp52/settle_gate_1..5.log`), **4 499 passed / 0 failed / 5 ignored / 0
filtered out over 74 binaries in each lane**, identical lane for lane: exactly
**+1** against RP5.2's 4 498, and the +1 is the thirteenth
`oracle_parity_cfg_gate` test. `grep -c '^warning'` over both clippy logs = 0.
The gate ran twice: once when the last `.rs` byte landed (both clippy passes
printing `Checking dss-core`, i.e. really re-checking) and once more after the
last `.md` byte, where clippy is a warm no-op precisely because no code changed
between them — the second run is what makes both `cargo test` lanes a statement
about the committed tree. The five ignored are still the pre-existing set — no
`#[ignore]` and no name filter were added, and `0 filtered out` in all 74
binaries of both lanes.
`lane_diff` is not owed: `git diff --stat -- 'crates/*/src'` is empty (the
settlement touches three test files and eight `.md`, and no product crate).

---

### RP5.2 — settlement record (ritual step 6, 2026-09-04)

Read end to end: `STATUS.md` (**595** lines after this pass, from 616 — the
under-600 working length restored) and this file, plus the archived plan's
§RP5.2, `PLAN_SEQUENCE.md` row 5b, `era-summaries.md` §1a and `ORPHANED_GAPS.md`
§1.11. The sub-step's own two commits are `5a110653` (closing record + archive
move) and `bc16430b` (audit settlement); this is the third. What RP5.2 and its
settlement had left stale, and what moved:

- **The plan's final counters were one commit out of date in four documents.**
  The closing record published the tree at `5a110653` — **4 498** tests per lane
  over 74 binaries with 12 `oracle_parity_cfg_gate` tests, and 64 RP-titled
  commits through `64474762`. The settlement then added a thirteenth test and two
  more commits, so the plan's *close* is **4 499 / 0 / 5** per lane and **67**
  RP-titled commits (64 + `5a110653` + `bc16430b` + this record). Both figures now
  read the same in `STATUS.md` §1, `PLAN_SEQUENCE.md` row 5b, this file's counters
  table and `era-summaries.md` §1a, each stating the 4 498 measurement and what
  the +1 is — the closing gate's own numbers are not overwritten, they are dated.
- **The archived plan's §RP5.2 landed line carried no shas** while every other
  landed line in the plan (§RP5.1 included) names its commits. It now names
  `5a110653`, `bc16430b` and this settlement record, with the post-settlement
  lane totals. The edit sits at the file's foot, ~1 660 lines below the
  `:1148-1151` range the two `props_r4133_replay.rs` doc comments cite, so the
  thirteenth guard's targets did not move — verified by re-running it, not
  assumed.
- **`STATUS.md` section 1 was above its working length and restated the plan's
  completion four times.** 616 → 595 lines: the "In flight", WP-G1 and "Next"
  paragraphs each keep one statement of *their own* stake in it (the frontier,
  G1.1 satisfied → G3.4/G3.5 unblocked, what runs next — now including the
  close-out merge of `r4133-props` into `update`); the WP-RP5 paragraph is back
  inside the 3–6-line-per-sub-step record-placement budget with the final
  counters; the thirteen per-sub-step RP3.x bullets are condensed into three
  outcome-grouped bullets (four excluded-and-pinned, six `FIX`-in-both-lanes,
  three recorded-never-reproduced) that still name every sub-step — their shas
  and settlements are the condensed table above, and their full records are in
  `r4133-props-rp3.md`. Two closed section-2 items (the `large`-case decision,
  the `UNIFIED_GATE_PLAN.md` banner) were tightened, not dropped. Nothing was
  deleted that is not held verbatim elsewhere.
- **Verified, not edited:** `ORPHANED_GAPS.md` §1.11 already names the archived
  path and RP5.2's re-verification; `GOLDEN_REBASE_PLAN.md` carries the dated
  **SATISFIED** paragraph and the two "unblocked" edits from the closing commit;
  section 7's `r4133-props-rp5.md` row already forwards §RP5.1/§RP5.2. The ~40
  bare-name `R4133_PROPS_PLAN.md` §-citations in Rust comments stay unrewritten
  under section 7's forwarding rule — they name a section, not a path, and the
  two that *were* line-cited were repointed at `5a110653`.

**Gate for this pass.** Documentation only — five `.md` files, not one line of
Rust, no test logic, no golden, no ledger, no tolerance. `cargo fmt --all
--check` **0** and `cargo test -p dss-core --test oracle_parity_cfg_gate` **13
passed / 0 failed** in **both** lanes — the executable check that covers exactly
what moved, since its thirteenth test reads the archived plan and its twelfth
walks `TESTING.md`'s citations. The five-command gate was not re-run for a
`.md`-only diff (the settlement ran it green on the final content at
`bc16430b`), and `lane_diff` does not apply: `git diff --stat -- 'crates/*/src'`
is empty. **WP-RP5, and with it `R4133_PROPS_PLAN.md`, is closed**; the frontier
returns to `GOLDEN_REBASE_PLAN.md` WP-G1 with G1.1 satisfied and G3.4/G3.5
unblocked.

**2026-09-05 — locator correction (GOLDEN_REBASE G1.3d(i), lane `lane-e`); content unchanged.**
The `TESTING.md:<line>` numbers above were measured when this record was written; `TESTING.md`
has grown since (WP-G1's G1.0, G1.3a and G1.3d(i) insertions), so the two sections it names now
sit at `TESTING.md:1441` ("The r4133 property policy — the claim chain (R4133_PROPS)", was
`:841`) and `TESTING.md:1980` ("Triage a property divergence (the r4133 channel)", was `:1116`)
— re-measured on `update` at the G1.3d(i) merge (2026-09-05), the lane read `:1197`/`:1648`;
the `:863`/`:880`/`:907`/`:946`/`:990` locators of the six re-spelled citations shifted with them.
Every anchor this record names is unchanged and still guarded by
`oracle_parity_cfg_gate::operational_docs_line_citations_point_at_the_line_they_name`; what is
unguarded is exactly the class this record already flags — a line-number citation *into*
`TESTING.md` from a phase record.
