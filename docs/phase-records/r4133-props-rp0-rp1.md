# R4133_PROPS - WP-RP0 and WP-RP1 records

> Moved verbatim from `STATUS.md` on 2026-09-03 (STATUS.md archiving round 2);
> order preserved, nothing rewritten. It holds STATUS §1's `### R4133_PROPS WP-RP0`
> (RP0.1, RP0.2) and `### R4133_PROPS WP-RP1` (RP1.1-RP1.4) condensed record
> blocks. STATUS.md §7 forwards `§WP-RP0`, `§RP0.x`, `§WP-RP1` and `§RP1.x` here.

### R4133_PROPS WP-RP0 — condensed records

> Plan: `R4133_PROPS_PLAN.md`. Branch `r4133-props` off `update` @ `2ee6bb00`.
> Every sub-step runs the plan's per-sub-step ritual: full five-command gate
> before the commit, then two fresh auditors and a fix agent.

- **RP0.1** (2026-08-22) — the G1.1 census evidence vendored to
  `tests/corpus/props_r4133/`. Five byte-identical copies of the local-only
  `investigations/g1_1_r4133_props/` extracts (24 944 bytes: `triage.md` 146
  lines, `summary.json`, `structural_pairs.txt` 209 + 1, `numeric_pairs.txt`
  94 + 1, `shape.txt` 5) plus five derivations of the 270 MiB local census:
  `examples_full.txt` (3 378 rows — every distinct `(rust, r4133)` spelling per
  pair, untruncated; counts sum to all 1 055 446 value cells),
  `bins.tsv` (303 rows, one §1.1 bin each),
  `structural_pairs_in_scope.txt` (198), `numeric_pairs_in_scope.txt` (53) and
  `shape_in_scope.txt` (5 classes, `rows` / `rows_in_scope`),
  and a `README.md` carrying the provenance, the parse rule, the bin rule, the
  in-scope filter and the data traps. **Every derived count matches plan §1.1
  and §1 above**: bins 75 / 61 (59 case + 2 trailing space) / 8 / 21 / 44 / 61 /
  33 = 303 pairs and 297 593 / 93 213 / 4 400 / 122 554 / 442 369 cells; the
  numeric bins re-derived in-scope give 37 / 16; in-scope 198 / 53 pairs over
  920 954 / 90 627 cells across 390 of the 433 divergent cases; shape 429 rows
  full, 149 in scope (generator 137, autotrans 7, windgen 5).
  Rules recorded in `README.md` and reproducible from the vendored files:
  the structural bin is decided by the pair's representative cell — its first
  census row, which is the cell the pair extracts print — through the chain
  bool → empty → case/trim → bracketed-array → enum/singleton; the numeric bin
  is `max_rel >= 1e-4`, a cut inside the census's empty band (largest display
  pair 6.43e-5 `load.pf`, smallest genuine jump 1.00e-3 `invcontrol.lpftau`).
  The in-scope filter is `engines ∈ {both, r4133}` joined by the census case
  label `"<manifest>:<path>"` → `manifests/solvable_now.json` or
  `tests/corpus/<family>/manifest.json`; all 438 census labels resolve and the
  union is exactly the 521-case `population.lock.json` population.
  One wiring edit outside the data: `.gitattributes` gains
  `tests/corpus/props_r4133/** -text` (the repo runs `core.autocrlf=true`, and
  an evidence directory whose whole claim is byte-faithfulness must not be
  EOL-rewritten on checkout). No manifest, ledger, golden or engine source
  moved. Gate: all five commands green.

  **Audit settlement (same day, two fresh auditors → fix agent; 9 findings, all
  settled, none dropped).** Every claim was re-measured against the local census
  (a streaming pass over all 1 055 880 rows joined to the four manifests), and
  every count below is now re-derivable from the vendored files alone and locked
  by a test.

  - **Two majors, both real, both fixed.** (1) *`bins.tsv`'s one bin per pair
    hides intra-pair mixing.* Classifying every cell instead of the pair's
    representative one puts 6 719 cells (0.70 % of the structural population,
    6 591 in scope) in a bin other than their pair's, over **17** structural
    pairs — **15** in scope, not just the three the plan discusses: e.g.
    `load.yearly` (bin 5, 5 995 case-only cells), `relay.switchedobj` (bin 2,
    102 in-scope `''` echoes), `line.wires`/`load.zipv`/`storage.dynadata`/
    `storagecontroller.seasontargets(low)` (bin 5, array-form cells),
    `capcontrol.type`/`invcontrol.voltage_curvex_ref`/`reactor.bus2` (enum cells
    in a case/empty pair). No data is lost — `examples_full.txt`'s per-pair
    multiset equals the census exactly — so this was a summarisation gap; the
    README now carries the full table plus the two consequences: RP2.1's
    admissible-claim assert must be **per example row**, never "the set the
    pair's bin allows", and the label is **order-dependent**, so RP0.2's
    acceptance compares order-invariant cell populations (`cells`,
    `cells_in_scope`, the `examples_full.txt` multiset), not the bin letter, on
    these 17 pairs. (2) *the "three mixed pairs" count is refuted by the
    evidence.* Of bin 1's 75 pairs, **nine** answer with an echo instead of a
    boolean — 3 834 echo cells, **3 293 in scope**: four genuinely mixed
    (`relay.reset` 226 foldable / 44 `0.20`, `relay.distreverse` 32 / 238 `''`,
    `recloser.eventlog` 30 / 200 `''`, `regcontrol.idle` 1 / 887 `''`) and five
    with **no** foldable cell at all (`regcontrol.idleforward` 888,
    `regcontrol.idlereverse` 888, `capcontrol.reset` 446, `recloser.debugtrace`
    230, `upfccontrol.enabled` 13). `relay.reset`'s echo is a stale parse string,
    so it is an `EchoParse` row, not the `EchoDefault` shape the other three
    take; and bin 1 overstates the BoolFold population by the five pure-echo
    pairs. RP2.3 must therefore provision **nine** echo rows, not three — the
    six unnamed pairs carry 2 125 in-scope echo cells (the plan's three carry
    1 168) that would otherwise reach the RP2.1 liveness assert unclaimed.
  - **Minors fixed in the evidence.** The README's EOL sentence was false
    for `triage.md` (LF in the source too — byte-identity itself held, verified
    by SHA-256 on all five copies); the in-scope filter now records that 4 of
    its 462 cases (`shape_binfiles`, `IEEE13_LineAndCableSpacing`,
    `IEEE13_LineSpacing`, `line_spacing_asym`) carry a channel-scoped
    `kind: "skip"` on r4133 in `ledger.json`, so the r4133 compare never reaches
    them — zero cells today, recorded as a definition; the in-scope shape split
    (149 = generator 137 + autotrans 7 + windgen 5), which WP-RP1's acceptance
    is measured against, is vendored as `shape_in_scope.txt` instead of living
    only in the gitignored census; and `tests/corpus/README.md` gained the
    directory in its inventory.
  - **Minors fixed by wiring.** The evidence had no lock at all:
    `crates/dss-core/tests/props_r4133_evidence_lock.rs` (8 tests, both lanes,
    no oracle) now pins SHA-256 + length over the five verbatim copies (24 944
    bytes total) and the `-text` stanza that keeps them raw, the row counts of
    every derived file, the cross-file equalities (`bins.tsv` ↔ pair files ↔
    `examples_full.txt` ↔ in-scope files ↔ `shape_in_scope.txt`, including the
    per-pair cell sums and the 1 055 446 total), the §1.1 per-bin totals with
    the in-scope 198 and the re-derived numeric 37/16, and — re-derived from the
    vendored rows — the two corrected traps above (the 17-pair heterogeneity
    table and the nine bin-1 echo pairs), so the corrections are machine-checked
    rather than prose. Non-vacuity probed: deleting one `examples_full.txt` row
    fails 3 of the 8, flipping one byte of `triage.md` fails the digest.
    `golden_lock.rs::EXCLUDED_TREES` gains the tree with its reason (it is
    outside the §1.2 enumeration deliberately: frozen evidence, no anchor
    describes it, and it is locked by the file above), and
    `oracle_parity_cfg_gate.rs::operational_docs` now names
    `tests/corpus/props_r4133/README.md` individually — the `tests/corpus` walk
    filter drops non-manifest files because upstream's vendored docs are not
    ours, which is exactly false for this one. `TESTING.md` gains the lock's
    section next to the golden/population locks, and `golden.lock.json`'s
    comment was regenerated (`DSS_UPDATE_GOLDEN_LOCK=1`) for the new exclusion —
    comment line only, **no digest moved**.
  - **Roll call (each finding → where it landed).** audit-code: intra-pair bin
    mixing → major (1); `triage.md` EOL → evidence; `relay.reset` as a fourth
    mixed pair → major (2); ledger `kind: skip` cases inside the filter →
    evidence. audit-tests: the refuted mixed-pair count → major (2); no lock on
    the evidence → wiring; representative-cell / order-dependent bin → major
    (1); in-scope shape not vendored → evidence; `tests/corpus/README.md`
    inventory + the doc-gate filter → wiring.
  - **Nothing refuted, nothing deferred silently.** The one item that cannot be
    fixed inside RP0.1's scope is the **plan text**: `R4133_PROPS_PLAN.md`
    §1.1's bin-1 table row, the §1.2 replay bullet, the RP0.1 spec sentence,
    RP2.1's `BoolFold` sizing + claim-semantics sentence and RP2.3's echo-row
    list said "three mixed pairs", and RP2.1's admissibility sentence read as
    per-pair-bin. WP-RP0's rule is stop-and-report on a mismatch between the
    vendored evidence and the plan: reported, then **resolved in the sub-step's
    closing docs commit** — all six sites amended: the §1.1 bin-1 row carries
    the nine-pair correction (four mixed incl. `relay.reset`'s `'0.20'`
    `EchoParse`, five pure-echo, 70 `BoolFold` pairs), §1.2's replay bullet
    states per-example-row admissibility over the 17 heterogeneous pairs (the
    `bins.tsv` label is a representative-cell summary, never the admissibility
    authority), the RP0.1 spec is annotated as-executed, RP2.1 sizes `BoolFold`
    at 70 pairs and declares the five pure-echo pairs for RP2.3 wholesale,
    RP2.3 provisions nine echo rows, and RP0.2's acceptance compares
    order-invariant cell multisets.

- **RP0.2** (2026-08-22) — the census is a permanent knob. `DSS_PROPS_CENSUS=1`
  arms the corpus-gate binary's own `corpus_gate_props_census` test, which walks
  every live non-`large` case (438 — exactly the
  population the 2026-08-08 scratch census walked) on **both** channels with
  `all_properties` forced on, bypassing the §1.1 r4133 masks, compares with the
  plain (un-normalized) comparator in collect-don't-panic mode and writes
  `tmp/props_census.json` + `tmp/props_census/run.json` +
  `tmp/props_census/<channel>/{structural_pairs,numeric_pairs,examples_full,
  shape,summary}` in the RP0.1 extract format. It **asserts nothing** — a
  divergence is the measurement, never a failure — and honors `DSS_GATE_ONLY`
  (an empty match still refuses, as on the gate; the filter is stamped into the
  artifacts). Files:
  `crates/dss-core/tests/corpus_gate/props_census.rs` (row model + extract
  writers, 12 unit tests pinning the vendored row formats, the 40/34-char example
  cuts, Python's `%.2e`, the `Line.b1||b2` pair key, `examples_full`'s ordering,
  the escape/parse round-trip, the heterogeneous-shape counter, the streamed JSON
  document, the loud mode parser, the `?`-surface stability argument and — the
  non-vacuity guard — that the collecting walk is silent on a faithful capture,
  emits exactly one row for one corrupted cell, a shape row plus an unaligned
  count for a dropped prop, and a counted skip for a capi-channel Relay), the
  walk in `scheduler.rs::run_props_census`/`census_one`, the trigger in
  `corpus_gate.rs`, `TESTING.md` (env-var row + a re-measurement procedure next
  to the evidence lock). `DSS_LIVE_PROPS`, the gate's r4133 property mask
  (`scheduler.rs::run_one_case`, `:364`) and the seeding one
  (`::seed_one`, `:720`) are untouched — both still read
  `cc.compare_all_properties = false` on the r4133 channel; knob OFF is today's
  gate exactly, proven by the five-command gate being green.
  - **The census is its own `#[test]`.** `corpus_gate_props_census` is armed by
    the env var; `corpus_gate_all_cases_match_engines` always runs the gate. The
    first cut dispatched the *mandatory* test on `DSS_PROPS_CENSUS`, so a stray
    `=1` in a shell or CI environment turned the one live corpus comparison into
    a green no-op (audit-tests, reproduced). Unset, the census test is a no-op.
  - **One comparator seam — the per-cell decision and the element walk.**
    `harness::value_verdict` is the single per-cell decision
    (`Match`/`Skeleton`/`NumberCount`/`Numeric{max_rel}`);
    `assert_value_matches_tol` is now its asserting wrapper (failure text
    byte-preserved) and `harness::collect_element_divergences` its collecting
    one. The pre-walk policy is shared verbatim too — `oracle_name_set`,
    `filter_015x` (the `PROPS_015X` relief) and `transformer_cursors_disagree`
    are single functions both walks call, after the audit found each of them
    written twice. What remains structurally separate is the loop body itself
    (aborting vs collecting), and it is pinned rather than trusted:
    `census_walk_tests::the_two_walks_agree_on_every_input` runs both over one
    input table and asserts the biconditional *`compare_prop_lists` panics **iff**
    the census reports a row or a blind spot* — the re-ordering case is why the
    blind-spot term is needed (a permutation leaves counts and name sets equal,
    so the census reports two uncomparable cells and no row). Both walks take the
    allowlist as a parameter, so both are testable with an injected table.
    Still owed by RP2.1, and NOT pre-built here: threading `Mode` and a ledger
    view from `run_props_census` → `census_one` → the walk, and the
    normalize → echo → floor → ledger stage the disposition mode adds. Two
    deliberate differences from the gate, both reproducing the scratch census:
    nothing panics, and the Recloser/Relay whole-element skips are
    channel-scoped (capi only —
    plan §1.2), which is where the census's 22 relay/recloser pairs come from.
    `max_rel` is `|a−e|/|e|` (or `|a−e|` when `e == 0`) maximized over the
    numbers that FAIL the floor, pinned by a `harness::comparator_tests` case.
    Its **verification breadth** was overstated in the first cut and is corrected
    here (audit-code, major): the formula reproduces all **95 317**
    `value_numeric` rows of the vendored census at each case's tier floor, but
    95 180 of those carry a SINGLE number, where the "over the offenders" and
    "over all numbers" readings cannot differ, and only **137** carry more (2:92,
    3:26, 4:4, 6:14, 10:1). Of the 137 exactly ONE discriminates them —
    `Line.l1` `cmatrix` on `modes:upgrade/upgrade_spacing_ratings.dss`, recorded
    as `5.833551103975013e-8` (offender-only, at the deck's `micro` floor) and
    not `1.1866505782192496e-7` (over all six numbers). The earlier claim "94 203
    multi-number rows", repeated in three permanent artifacts, matched no
    population in the census at all and is retracted at each site. The
    refactor's non-vacuity is the existing `props_015x_tests` (a value mismatch,
    a missing prop and a misordered prop each still panic) plus the census-side
    guard above. RP2.1 extends this seam for `DSS_PROPS_CENSUS=claims`; the mode
    parser rejects `claims` loudly today rather than silently handing back a
    plain census.
  - **Acceptance (the plan's family-bounded form).** `DSS_GATE_ONLY=modes:windgen`,
    both channels, 5 cases: the knob's 74 r4133 rows equal the local full
    census's 74 rows for that family as cell multisets — and in fact **row-for-row
    in order**. The three pairs whose entire census population lies inside the
    family reproduce their vendored extract lines byte-for-byte, `%.2e` and all:
    `windgen.kva | '3157.89473684211' | '3157.89' | 2.00e-06 | 2`,
    `windgen.kvar | '986.05231553659' | '0' | 9.86e+02 | 4`,
    `windgen.mva | '3.15789473684211' | '3.15789' | 2.00e-06 | 2`. The capi
    channel on those five decks records 5 `oracle_error` rows (0.14.5 has no
    `WindGen` class) instead of failing — the collect contract working.
    **WP-RP0 is closed**: the knob reproduces the vendored rows for the
    spot-checked family on both channels.
  - **Full re-census (not owed, run anyway: 438 cases × 2 channels in 58 s;
    re-run after the audit fixes, r4133 row count unmoved).** 1 056 790 r4133
    rows vs the vendored 1 055 880; 433 divergent cases = 433, 94 numeric
    pairs = 94, 5 shape classes = 5 with all five `shape.txt` rows
    byte-identical. The pair extracts, measured row-by-row (the first cut's
    "209 of 210 structural pairs identical including their example cells and
    counts" was too strong and contradicted this bullet's own finding 2 — audit-
    tests, corrected): **202** of the 209 vendored structural rows are
    byte-identical, **7** carry the +2 count shift of finding 2
    (`transformer.bhcurrent/bhflux/enabled/ratings/sub/xrconst/xscarray`) and
    **1** row is new (`regcontrol.fwdthreshold`, finding 1); of the 94 numeric
    rows **90** are byte-identical and **4** carry the same +2
    (`emergamps/normamps/pctperm/repair`). `examples_full.txt` (newly re-derived
    by the knob) lands **3 349 of 3 378 rows byte-identical and in position**;
    the 29 that move are all `transformer.*` — the finding-2 count bumps
    (Σ = exactly the +22 cells that finding predicts) plus the tie re-orderings
    they cause inside three pairs — with 2 new `regcontrol.fwdthreshold` rows and
    the one `storage.dynadll` row that differs only in backslash escaping (see
    finding 5). The residual is five findings, none of them a knob defect (per
    the vendored `README.md`, a disagreement is a finding, not a rewrite —
    RP0.1's frozen files are NOT touched):
    1. **`regcontrol.fwdthreshold` (888 cells, a whole new bin-5 pair) is missing
       from the vendored census.** r4133 `Controls/RegControl.pas` initializes
       `PropertyValue[1..32]` and stops (`:1423-1459`) while `GetPropertyValue`
       overrides only index 28 (`:820-828`), so props 33–36 — `idle`,
       `idleReverse`, `idleForward` **and** `fwdThreshold` — all answer with the
       `''` echo. The census records the first three at 888 cells each and omits
       the fourth; the port renders `100`. The omission is internally
       inconsistent with its own three siblings, so the pair is real: RP2.3 must
       provision an `EchoDefault` row for it (bin 5 goes 44 → 45 pairs,
       structural 209 → 210 / 960 129 → 961 031 cells). **Consequence for
       RP2.1**: `examples_full.txt` has no `regcontrol.fwdthreshold` row either,
       so that echo row would claim nothing in the offline replay and trip the
       both-ways liveness assert. The knob now measures the missing spellings
       exactly — its `examples_full.txt` carries `regcontrol.fwdthreshold |
       '100' | '' | 864` and `| '800' | '' | 24` (Σ 888) — so RP2.1/RP2.3 must
       either add those two rows to the replay's input or exempt the
       row with this record as its citation. **Decided in the closing docs
       pass (2026-08-22): RP2.1 vendors `examples_supplement.txt` with those
       two rows (provenance = this record) and feeds the replay from both
       files; the plan's §1.1 bin-5 row, §1.2 replay bullet, RP2.1 and RP2.3
       are amended accordingly.** The frozen RP0.1 files stay untouched.
    2. **The two cursor-disagreement transformers are absent whole-element**
       (`asymmetric:transformer/transformer_asym.dss` `Transformer.t3w`,
       `solvable_now:Test/YgD-Test.dss` `Transformer.tr1` — exactly the two the
       harness names at `TRANSFORMER_CURSOR_PROPS`). The scratch census dropped
       the entire element on a cursor disagreement; the gate — and therefore the
       knob — drops only the 13 cursor-contaminated props, so 11 further props
       compare on each: +2 cells on 7 structural pairs (`transformer.bhcurrent/
       bhflux/enabled/ratings/sub/xrconst/xscarray`) and 4 numeric ones
       (`emergamps/normamps/pctperm/repair`, max_rel unmoved). No new pair.
    3. **The 5 `oracle_error` rows differ only in the DLL load address** inside
       the r4133 access-violation text (`…50A3CE5E` vs `…54ADCE5E`, same
       `offset 41CE5E`): ASLR, not a divergence. Same five cases, same #303.
    4. **The vendored `README.md`'s "the walk covered ~512 live cases; 438 of
       them left a row here" is wrong** (audit-code; the fourth finding against
       the frozen evidence, and the plan §1.1 already says 438). Measured from
       the four manifests: 521 cases, 517 live, **438** live non-`large` — and
       that set is EXACTLY the census's 438 case labels (symmetric difference 0).
       So every case the 2026-08-08 walk touched left a row, and the 79 `large`
       decks left none, i.e. were not walked (they are excluded from property
       forcing on the gate too). The knob measures and prints the same 438.
       Recorded, not fixed by RP0.2: the README is RP0.1's frozen evidence and
       RP0.2 does not edit it — it is the outlier against both the plan and the
       data. **Fixed in the closing docs pass (2026-08-22): the README sentence
       now states the measured 438-of-438 population, with the original wording
       quoted in place.**
    5. **The vendored extracts spell the same cell two ways, and the README
       promises more re-derivation than the plain census can give** (audit-code).
       (a) The generator escaped backslashes in `structural_pairs.txt` (Python
       `repr`) but not in `examples_full.txt` / the `*_in_scope` files, so
       `storage.dynadll` reads `'C:\\Users\\…'` in one and `'C:\Users\…'` in the
       other — a live trap for RP2.1's replay, which reads `examples_full.txt`.
       The knob uses ONE quoting for every extract (escaped), which is the single
       row where its `examples_full.txt` deviates from the vendored copy.
       (b) The README's provenance sentence says the knob writes "these same
       extracts", i.e. all of them. RP0.2 now also emits `examples_full.txt` (the
       file RP2.1 actually replays) — but `bins.tsv` needs the §1.1 bin policy
       and the three `*_in_scope` files need the in-scope case filter, neither of
       which the plain census carries; both belong to RP2.1's disposition mode.
       **Fixed in the closing docs pass (2026-08-22): the README's provenance
       paragraph now states what the knob writes as landed (incl.
       `examples_full.txt` and `run.json`), that `bins.tsv`/`*_in_scope` wait
       on RP2.1's `claims` mode, and the uniform-escaping deviation; the
       `storage.dynadll` two-spellings trap is called out in the data-traps
       section with RP2.1 named as the consumer at risk.**
  - **Three things the vendored census could not report, now measured.** The
    `channels` block carries `unaligned_cells` — cells no index-ordered compare
    can reach because the two name lists desynchronized earlier in the element
    (r4133: **10 776**; capi: 0). That is why the vendored
    value population of the five shape-gap classes is a **lower bound**: e.g.
    `windgen.dynout` and `windgen.enabled` sit past r4133's `usermodel`/`userdata`
    insertion at 18/19 and are invisible until WP-RP1 closes the gap (its
    acceptance, "zero `shape_count` rows", is therefore also what unlocks the
    tail). And the **capi channel**, which the r4133-only scratch census never
    walked, comes back with 3 structural + 13 numeric pairs over 34 cases and 21
    `oracle_error`s — all of it outside the gate's own capi compare by
    construction: the census runs ledger-free (the gate's `property` ledger
    scopes are not applied) and forces properties on cases the gate never sends
    to capi at all (e.g. the `engines: r4133` `controls:swtcontrol/*` decks, whose
    `SwtControl.Normal` reads `closed` vs 0.14.5's `open`). And the capi
    channel's `unaligned_cells: 0` used to read as "complete" while whole
    Recloser/Relay elements were dropped there uncounted (audit-code): the block
    now also carries `skipped_elements`/`skipped_element_cells` — capi **312**
    elements / **13 520** cells, r4133 0/0 — plus
    `heterogeneous_shape_classes` (0 on both; a nonzero value would mean
    `shape.txt`'s one-row-per-class format is hiding a second shape, and prints a
    banner). No gate signal.
  - **Audit settlement (2026-08-22, 14 findings from audit-code + audit-tests;
    nothing dropped, nothing refuted).** Both majors and ten of the twelve minors
    are FIXED; one minor is split fixed + recorded (9), one is recorded-only (5),
    and major 2 keeps a recorded remainder the plan assigns to RP2.1:
    (1) major, the `max_rel` "94 203 multi-number rows" overclaim — **fixed** at
    all three sites, re-measured over the local census (95 317 numeric rows, 137
    multi-number, one discriminating cell) with the emulation cross-validated
    against the census's own kind partition (0/960 128 structure rows have equal
    skeletons, 0/95 317 numeric rows have unequal ones).
    (2) major, "the RP2.1 seam is a second copy of `compare_prop_lists`" —
    **fixed** as far as RP0.2 owns it: the three duplicated pre-walk policy
    expressions are now shared functions, the collecting walk takes an injectable
    allowlist, and the two loop bodies are pinned to each other by a
    biconditional test; the `Mode` + ledger threading is **recorded** as RP2.1's
    (the plan assigns the disposition mode there, and a one-variant enum threaded
    through three frames would be untestable plumbing).
    (3) `quote()`'s `\'` breaking the documented parse regex — **fixed**
    (`\x27`, with a round-trip test through the README's own regex rule).
    (4) the `NO_STEP` doc's "`-1` is what the vendored census uses" — **fixed**
    (the five `oracle_error` rows carry `['case','detail','kind']` and no `step`;
    the knob never serializes one either).
    (5) the README's "~512 live cases" — **recorded** as finding 4 above (frozen
    evidence; the measured population is 438 and the plan already says so).
    (6) a malformed oracle payload filed as `rust_error` — **fixed**
    (deserialization moved out of the Rust-side `catch_unwind`).
    (7) capi `unaligned_cells: 0` hiding whole-element skips — **fixed** (the two
    new counters above).
    (8) "the census walks solve→properties, the gate walks it last" — **fixed by
    demonstration**: every `dss.command` `compare_capture` issues between `solve`
    and the property block is a `? Element.Prop` QUERY (`compare_probe`, the
    ledger's probe handlers; the ledger's property rewrite is inside the block),
    `compare_export` compares two strings without touching the engine, a `?`
    query is a read, and the one property-visible cursor (`ActiveWinding`) is
    written only by property SETTERS while `element_properties` re-selects the
    object and refreshes `Vterminal` itself; the permanent test
    `the_props_surface_is_stable_across_the_gates_intervening_comparators`
    replays that traffic on a 3-winding transformer and asserts the `?`-surface
    is byte-identical.
    (9) capi extracts headed `r4133-example` + the un-regenerable derived files —
    **fixed** (headers name their channel; `examples_full.txt` is now emitted and
    reproduces 3 349/3 378 vendored rows in position) and **recorded** for
    `bins.tsv`/`*_in_scope`, which the plain census cannot derive (finding 5).
    (10) audit-tests' restatement of (2), incl. the gate's count-only assert vs
    the census's name-set compare — **fixed** by the same biconditional pin,
    which is what proves the two statements coincide.
    (11) the overstated full-re-census agreement — **fixed** (202/7/1 and 90/4,
    re-measured).
    (12) a stray `DSS_PROPS_CENSUS=1` greening the mandatory gate — **fixed**
    (own `#[test]`).
    (13) artifacts overwriting a fixed `tmp/` path with no record of the filter —
    **fixed** (`gate_only` in the census header and in a new
    `tmp/props_census/run.json`).
    (14) `shape.txt` keeping only the first shape per class — **fixed** as
    reporting: the format stays vendored-faithful (the JSON is the lossless
    record) and `heterogeneous_shape_classes` + a banner make the loss loud.
  - Gate: all five commands green. `lane_diff.ps1` not owed — no solved state,
    no compat kernel; the knob is test-harness code and inert unless the env var
    is set.

### R4133_PROPS WP-RP1 — condensed records

> Plan: `R4133_PROPS_PLAN.md` §WP-RP1. Same branch, same per-sub-step ritual.

- **RP1.1** (2026-08-22) — the three r4133 **upstream-stub** property rows.
  Generator `Rneut`/`Xneut` at display slots 16/17 and Sensor `Action` at slot
  13 now exist in the port, closing the two shape gaps `shape.txt` records
  (generator 48 → 50 names, sensor 15 → 16).
  - **Mechanism — one new flag, one new `PropDef` field.**
    `PropFlags::UPSTREAM_STUB` (bit 85) marks a property r4133 still *registers*
    but no longer implements. `ClassProps::parse_into` handles it ahead of the
    `ptype` match: store the raw parse string through the class's own
    `set_string`, then emit `PropDef::stub_message` if the row carries one, and
    return — never a parse error, no side effect, no recalc. That reproduces
    r4133 exactly: its Edit loop assigns `PropertyValue[]` for every property
    *before* the arm runs (`generator.pas:625`, `Sensor.pas:253`), and the arm
    then either logs a soft `DoSimpleMsg` (5611/5612, `generator.pas:651-652`)
    or does nothing (`TSensorObj.Set_Action` is an empty body,
    `Sensor.pas:850-854`). `PropFlags::NOT_PORTED` could not be reused — it
    hard-errors the write and hides the row from JSON. The per-class
    side-effect hook is the `stub_message` field itself (code + text), so
    Generator logs and Sensor stays silent with no per-class code at all;
    `ClassProps::new` `debug_assert`s that a `stub_message` implies the flag.
  - **Storage.** Plain `String` fields (`Generator::rneut_text`/`xneut_text`,
    default `"0"` per `generator.pas:2567-2568`; `Sensor::action_text`, default
    `""` per `Sensor.pas:807`), read/written by the classes' `get_string`/
    `set_string` arms and copied by `make_like` — r4133's `MakeLike` copies the
    donor's whole property-string array (`generator.pas:830-831`, right after the
    `ClassMakeLike` at `:828`; `Sensor.pas:428`),
    and these are the only properties where that copy is observable in a port
    that otherwise renders live fields.
  - **Ordinal shifts.** Generator `NUM_PROPS` 48 → 50 (`STATUS`..`ENABLED`
    +2), Sensor 15 → 16 (`BASE_FREQ`/`ENABLED` +1). Every `prop::` consumer is
    symbolic (checked by grep across the workspace: no `generator::prop`/
    `sensor::prop` reference outside the two modules). The raw-slot **prose**
    that remains — `make_pos_sequence`'s doc and the four `makeposseq_*` pins in
    `generator/tests.rs` (`slot 19`/`slot 20`, "r4133 slot 27", `PrpSequence^[26]`,
    "dss_capi's stale had_kVA index") — is all in r4133's *CmdMapIndex* space,
    the space old-OpenDSS keys `PropertyValue`/`PrpSequence` by, so the shift
    leaves it correct; both sites now say so explicitly (the audit round added
    the label to `tests.rs`, where the earlier claim that the doc was the only
    such prose was wrong).
  - **Surfaces.** All three rows also carry `PropFlags::HIDE_R4133` — they are
    absent from BOTH pinned captures (dss_capi 0.14.5 deleted Generator's pair
    outright and commented Sensor's out, `.inputs/dss_capi/src/Meters/
    Sensor.pas:39,55`) — so Dump / `Dump commands` / AltDSS JSON / the schema
    output skip them while `?` and the props table expose them. That re-arms a
    flag which had been carrier-free since WP-U2.5; the F.3aa blast-radius
    measurement still isolates its Line/LineGeometry population because the two
    hide flags' carrier sets are now asserted **class-disjoint**. The three rows
    join `PROPS_015X` (name-based, so on the r4133 channel — whose oracle name
    list *does* carry them — `filter_015x` keeps them and they compare in full).
  - **Goldens: measured, not moved.** `tests/golden/props/{generator,sensor}.json`
    do **not** change — `gen_props.py` enumerates the *oracle's*
    `AllPropertyNames` (capi 0.14.5), which can never learn an r4133-only name.
    Measured, not assumed: those two classes' 8 + 6 scenarios were re-run
    through the pinned dss-python (0.15.7 / backend 0.14.5) and compared to the
    committed records — **identical**. (A whole-file `gen_props.py` run is
    impossible in the pinned environment and was so before this sub-step:
    `props/regcontrol.json` is `capi015`-anchored and its `idle=` scenario
    raises #110 on the 0.14.5 backend.) `PROPS_CLASS_FILES`/`PROPS_SCENARIOS`/
    `PROPS_PROPERTY_CELLS` therefore stay 51/322/8343.
    Two `json/` schema artifacts *do* move, both predicted by §1.2: three new
    `port_hidden_property` rows in `schema_divergences.json` — Generator
    `Rneut` index 16 / order **17** and `Xneut` 17 / **18**, Sensor `Action`
    13 / **13** (both fields are the prop's OWN ranks, the convention the four
    Line rows use; on Generator `Like`'s hoist to order 1 puts every order one
    above its index — `Conn` 15/16, `Status` 18/19 — while on Sensor the
    `BooleanAction` `Clear` hoisted to the end cancels that, so order == index:
    `Weight` 12/12, `BaseFreq` 14/14) —
    after which `ported_class_defs_bytes_match_oracle` and
    `full_document_reconciles_with_oracle` pass untouched; and
    `schema_full_port.json` regenerated with `REGEN_SCHEMA_PORT=1`, whose diff
    is **exactly 35 `$dssPropertyIndex` + 35 `$dssPropertyOrder` renumberings**
    inside the Generator and Sensor regions and nothing else (no property block
    added or removed). `golden.lock.json` moved exactly those two digests, in
    this commit. No other golden byte moved anywhere.
  - **Measurement (RP0.2 census knob, `DSS_GATE_ONLY="IEEE 30 Bus"` — a
    5-generator `both`-channel case), before vs after:**
    r4133 shape classes **1 → 0** and cells uncomparable behind a desynchronized
    name list **175 → 0** (before: `generator: rust_count=48 oracle_count=50
    oracle_only=['rneut','xneut']`); the 175 newly comparable cells surface as
    structural pairs 23 → 29 and numeric 15 → 16, which is the WP-RP2/RP3
    workload the shape closure exposes, not a regression. The **capi_v0145
    channel is bit-unchanged**: 0 structural / 0 numeric / 0 shape / 0
    uncomparable, before and after.
  - **Tests.** `exec::tests::upstream_stubs` (new module, 8 tests) pins
    store+echo+message ordering and the `DoSimpleMsg`-not-abort severity, the
    silent Sensor write, the non-numeric round-trip, the display slots
    (`…DispValue, Conn, Rneut, Xneut, Status…` / `…Weight, Action, BaseFreq,
    Enabled` with the two table lengths), the `MakeLike` copy, the Dump/JSON
    absence and the `Save` *presence* (r4133's `SaveWrite` is flag-blind, so a
    deck that wrote `rneut=` gets it back). The "write does not change behavior"
    acceptance is per class:
    `generator_neutral_stubs_move_neither_y_nor_the_solution` — two *independent*
    solves of the same micro deck differing only in whether `rneut=`/`xneut=`
    were written give **bit-identical** Y triplets, iteration count and node
    voltages (the A/B pair rather than a re-`Solve` because re-solving from a
    converged state drifts ~1e-8 for reasons unrelated to these properties),
    plus an in-place `Edit` **followed by `Solve`** — the rebuild is what makes
    that arm discriminate, since `system_y_csc` reads the assembled matrix and
    `CalcYPrim` runs from `BuildYMatrix`, not from `RecalcElementData` — with a
    control write of three live Generator props that must move the rebuilt Y;
    and `sensor_action_moves_neither_sensor_state_nor_the_solution` — the same
    A/B shape plus the sensor's whole property table (its estimator input set)
    bit-identical outside the `Action` cell.
    `compat_quirks::upstream_stub_rows_are_the_measured_set` locks the flag's
    carrier set, that only the Generator pair carries a message (5611/5612), and
    that every carrier is also `HIDE_R4133`;
    `hide_015x_carrier_set_is_the_measured_escape` gained the `HIDE_R4133`
    carrier list and the class-disjointness assert;
    `props_r4133_evidence_lock::rp1_1_closes_the_generator_and_sensor_shape_gaps`
    reads the closure back off the vendored `shape.txt` itself (live table
    length == the census's `oracle_count`, the `oracle_only` names present, the
    frozen `rust_count` still the pre-fix number).
  - **Audit settlement (2026-08-22, `/audit-code` + `/audit-tests`, 10 findings:
    3 major, 7 minor — 10 fixed, 0 refuted, 0 deliberately unfixed).**
    (1) *major, code* — the three new `schema_divergences.json` rows carried an
    off-by-one `$dssPropertyOrder` (16/17/12 where the port's own ranks are
    17/18/13), invisible to the gate because `renumber_field` shifts by
    `count(removed < v)` and the slot below each was occupied. **Fixed** in the
    artifact, its `cause` prose (which now also records the own-rank convention
    and why an error there is absorbed) and in this record; verified against
    `schema_full_port.json` (`Conn` 15/16, `Status` 18/19; `Weight` 12/12,
    `BaseFreq` 14/14) and the Line precedent (`EpsRMedium` 31/27).
    (2) *major, code* — the shape closure makes value pairs live that the frozen
    2026-08-08 extracts structurally cannot contain (while a class carried a
    `shape_count` row its walk stopped at the name disagreement), and nothing
    opened a supplement item. **Fixed by measurement**: a full `DSS_PROPS_CENSUS=1`
    re-run on the post-RP1.1 tree (438 cases, both channels, 56 s) gives r4133
    shape classes **5 → 3**, structural **218**, numeric **98** (frozen extracts:
    209 / 94; the knob's own pre-RP1.1 re-census: 210 / 94), capi channel
    unchanged — so RP1.1 adds exactly **12** pairs and removes none:
    `generator.{debugtrace, enabled, status, dynamiceq, dynout, shaftdata,
    userdata}` + `sensor.enabled` (bins 1/2/5),
    `generator.{kva, maxkvar, minkvar}` (bin 6, max_rel ≤ 5.8e-6 — an order of
    magnitude under the worst display pair) and **`generator.d`** (`'1'` vs
    `'0'`, rel 1.00e+00) in **bin 7**, which grows the in-scope bin-7 population
    16 → 17 and un-closes §1.1's enumerated four-root-cause list. Its cause is already Pascal-read and is an
    **echo**, not a jump: `TGeneratorObj.Create` sets `GenVars.D := 1.0` and
    never `Dpu` (`generator.pas:955-971`), while `InitPropertyValues` echoes
    `Format('%-g', [GenVars.Dpu])` (`:2585`) — RP2.2 triage → an RP2.3 echo row.
    Recorded in the vendored `README.md` §"Pairs the WP-RP1 shape closures make
    live" (with the per-pair table), and the plan now (a) carries the correction
    in §1.1, (b) points RP2.1's `examples_supplement.txt` at it, (c) makes the
    re-measurement a per-sub-step obligation of WP-RP1 (autotrans/windgen next).
    (3) *major, tests* — arm 2 of the Generator acceptance was **vacuous**:
    `system_y_csc` reads the already-assembled `solution.y_system` and
    `CalcYPrim` runs only from `BuildYMatrix` on `Solve`, so "Edit then compare
    Y" held for every property. **Fixed**: the arm now re-`Solve`s, and a control
    write of three live Generator props asserts the rebuilt Y *does* move on that
    deck.
    (4) *minor, code* — `UPSTREAM_STUB` is only sound for `PropType::String`
    (`get_value` dispatches on `ptype`), which nothing enforced and the parse
    comment contradicted. **Fixed**: a second `debug_assert` in `ClassProps::new`,
    corrected comment, and the invariant stated on the flag.
    (5) *minor, code* — the re-armed `HIDE_R4133` escape had no owning row and no
    measured blast radius, unlike its `HIDE_015X` precedent. **Fixed by
    measurement**: disabling the flag's arm and running `cargo test -p dss-core
    --no-fail-fast` moves exactly 4 committed artifacts
    (`json/der_usermodel_{assigned,full}.json`, `json/dyneq_full.json`,
    `reports/dump3_commands.txt` +6 rows) plus `schema_full_port.json` and the
    3 `port_hidden_property` rows, and **no corpus case** (live gate green). Row
    added to `ORPHANED_GAPS.md` §2 naming GOLDEN_REBASE **G3.3c + G3.4** as the
    unblocking sub-steps (the schema half never unblocks: `schema_full_oracle` +
    `schema_divergences` stay frozen); the numbers also live on the flag's doc.
    The un-hide would render real text, not a key-on-miss fallback: all three
    help strings have been in `report/help_catalog.rs` since U2.5.
    *(These figures are the three-carrier measurement and are superseded by
    RP1.2's audit item (1): with the fourth carrier the un-hide moves 8
    artifacts, `dump3_commands` +7 rows, 4 `port_hidden_property` rows — corpus
    still untouched.)*
    (6) *minor, tests* — `prop_flags.rs`'s `HIDE_015X` doc still claimed the
    sibling has "zero carriers today", the empirical basis of `UPGRADE_PLAN` §5's
    unreachability argument. **Fixed** (restated as the U2.5→RP1.1 era, which is
    when the measurement was taken).
    (7) *minor, tests* — `Save` prints the three names and nothing decided or
    tested that. **Settled as correct and pinned**: r4133's `TDSSObject.SaveWrite`
    walks `PrpSequence` with no flag filter (`General/DSSObject.pas:131-165`), so
    a deck that set `rneut=` gets `Rneut=` back from r4133 too — the port matches
    the authority. New pin `save_writes_the_stub_names_like_r4133`; the cost (such
    a deck is not re-compilable by the 0.14.5 backend) is recorded on the flag and
    in `TOLERANCE_NOTES`. Inert: no corpus deck and no `save*` golden writes any
    of the three.
    (8) *minor, tests* — no "write changes nothing" pin for Sensor `Action`.
    **Fixed** (`sensor_action_moves_neither_sensor_state_nor_the_solution`).
    (9) *minor, code* — the `MakeLike` citation `generator.pas:828` is the
    `ClassMakeLike` call; the property-array copy is `:830-831` (**fixed** in all
    four places), and this record's claim that `make_pos_sequence`'s doc was the
    only raw-slot prose was false — `generator/tests.rs` carries four more
    (all upstream `CmdMapIndex` space, all still correct; **fixed** by labelling
    them and correcting the sentence above).
    (10) *minor, code* — two active docs still described the pre-RP1.1
    carrier-free `HIDE_R4133` assert: `DE_PASCALIZE_PLAN.md`'s tripwire
    description and `era-summaries.md`'s "no live application site after U2.5".
    **Fixed** — the plan now describes what the live test actually checks (the
    sibling's carrier set + the two flags' class-disjointness); the two phase
    records got as-of-U2.5 notes rather than rewrites, so no two places
    disagree. (This item was dropped from the first cut of this enumeration,
    which read "9 findings" — restored by the coordinator's closing review.)
  - Gate: all five commands green, re-run after the settlement.
    `lane_diff.ps1` not owed — no solved state and no compat kernel moved
    (proved by the bit-identical A/B above).

- **RP1.2** (2026-08-22) — AutoTrans `XfmrCode`, a **real behavioral port**.
  r4133 property 39 (`Version8/Source/PDElements/AutoTrans.pas:329`, help `:414`)
  and `TAutoTransObj.FetchXfmrCode` (`:520` → `:2339-2396`) now exist in the
  port, closing the third `shape.txt` gap (autotrans 52 → 53 names). dss_capi
  0.14.5 deleted the row outright (`.inputs/dss_capi/src/PDElements/`
  `AutoTrans.pas:76,125`), so there is no capi witness for any of it.
  - **The copy is written field by field, not reused.** The Transformer's
    `fetch_xfmr_code` (`pd/transformer/windings.rs:357-402`) assigns the winding
    vector wholesale and copies the 0.15.x kVA-ratings; neither is r4133's auto
    form. The AutoTrans variant (`pd/auto_trans/windings.rs`) follows the Pascal
    statement by statement, with its three deviations from the Transformer
    routine: the connection override (winding 1 → `SERIES`, winding 2 → `WYE`,
    tertiary and beyond keep the code's own connection, `:2356-2361`), the
    reactance rename (`XHL/XHT/XLT` → `puXHX/puXHT/puXXT`, `:2374-2376`) and
    `RdcSpecified := TRUE` on every winding (`:2367`), which selects
    `RecalcElementData`'s `Rdcpu := RdcOhms/(VBase²/VABase)` branch instead of
    the 85 %-of-ac default that would overwrite the copied `RdcOhms`.
    `MakeLike` carries the code name (`:848`), `Create` starts it `''` (`:905`).
  - **Two upstream bugs in that one arm, neither reproduced** (CLAUDE.md — never,
    in any lane); both got an English report in `investigations/to_opendss/`:
    - `36-autotrans-xfmrcode-not-used-message.md` — `DoSimpleMsg('XFmrCode
      Property not used with AutoTrans object.', 100131)` (`:567`) fires
      unconditionally right after `:520` applied the code. Measured on the
      epri-worker: it is **non-fatal inside a `Compile`** (the errno surface is
      clean afterwards, which is what lets the new deck gate r4133 with no
      `expect_warnings`), but on a per-command `Edit` it lands on the errno
      surface **and overwrites the real #100180** — a mistyped code name is
      reported as "property not used". The port stays silent on the hit path and
      answers a miss with r4133's own `Xfmr Code:<name> not found.` / #100180.
    - `37-autotrans-fetchxfmrcode-nconds.md` — `NConds := Fnphases + 1`
      (`:2353`), the *Transformer's* conductor rule copy-pasted into a class
      whose every other path sets `2 * Fnphases` (`:538`, `:798`, `:891`,
      `SetNumWindings` `:1021`) because the auto carries two conductors per phase
      and `SetNodeRef` aliases the series winding's second end inside that block
      (`:986-1002`). Measured on the released 11.0.0.1 DLL, one 3-phase 115/69
      auto: `NumConductors = 4` and `MID.1 = 92 236 V` on a 69 kV winding, versus
      6 and 38 472 V for the same deck with a trailing `phases=3` (which re-runs
      the correct side effect). Both runs converge and report nothing. The port
      keeps `2 * Fnphases` via `set_nconds`, which is what the Pascal statement
      does structurally (reallocate terminals, flag `BusNameRedefined`) with the
      auto's count.
  - **Measure-first, and the deck is single-phase on purpose.** No corpus deck
    set `xfmrcode=` on an AutoTrans; the new one
    (`asymmetric:autotrans/autotrans_xfmrcode.dss`, `engines: "r4133"` — 0.14.5
    would reject the property outright) is **1-phase** because that is the one
    width where the `NConds` bug is inert (`1 + 1 == 2 · 1`), so the r4133
    channel is a valid oracle for the whole copy instead of comparing against a
    structurally wrong element. Its `XfmrCode` declares BOTH windings `delta`,
    so the winding-1 override is strongly observable (at one phase
    `RecalcElementData` derives the series `VBase` from
    `kVseries = kVLL − Winding[2].kVLL`). Two autos share the code, one left on
    winding 2 and one on winding 1, so the same 16-property probe set reads both
    windings' copied values; the probes are limited to props r4133's
    `GetPropertyValue` re-renders live (`:1798-1896`) and whose exact decimal
    fits `%.7g` — `thermal`/`n`/`m`/`flrise`/`hsrise` are `PropertyValue` echoes
    and `normamps`/`emergamps` are 5-significant-digit renders, so neither is
    probed. Live-validated on r4133 before the port existed (epri-worker
    2026-08-22: compile clean, converged, 5 iters, `SOURCEBUS.1`
    114 908.668456851403, `MID1.1` 66 514.442565085992, `SRC2.1`
    117 228.327082133997, `MID2.1` 68 431.041427177013, both autos
    `NumConductors = 2`), and the gate then matched it at the micro floor with
    **no ledger entry**. Non-vacuity proven in a scratch tree: dropping the
    winding-1 override alone reds the `[R4133]` channel on node voltages
    (|Δ| = 8.7 V against an allowed 1.16e-4).
  - **The multi-phase form is pinned in-engine**, since r4133 cannot witness it:
    `exec::tests::autotrans_xfmrcode` (12 tests) covers the forced
    `Series`/`Wye`/`Delta` on a 3-winding code, every copied scalar, the
    `RdcSpecified` selection (with a control auto that never saw a code), the
    conductor count (`Yorder = 18`, where the reproduced bug gives 12), the miss
    arm (#100180, model and stored name untouched, rebuilt Y identical), the
    empty-name no-op (measured: r4133 logs nothing there either), the absence of
    #100131, the display slot, `MakeLike`, the surface split (`Save` writes the
    code name like r4133's flag-blind `SaveWrite`, `Dump` does not — the RP1.1
    disposition), and — the correctness statement no oracle can make — that a
    coded auto and the same auto written out longhand produce a **bit-identical**
    assembled Y and node-voltage vector — plus (audit round) the guard that keeps
    the copied-cell table honest: no asserted cell may equal what a bare auto of
    the same shape holds anyway.
  - **Mechanism added:** one `PropDef` field, `ref_miss_message`
    (`RefMissMessage { code, prefix }`). dss_capi routed every object reference
    through the property system's single #401 miss path, which the port follows
    for every row that has a capi counterpart; this row has none, and r4133 still
    resolves it through the legacy `FetchXfmrCode`, whose `else` is one
    `DoSimpleMsg` that leaves the stored name and the model exactly as they were
    (`:2394-2395`). The field buys both halves: the r4133 text/number, and a miss
    that returns before `set_object_ref` so nothing is written. Its two
    guarantees are the ones the sibling `stub_message` already has (audit round):
    a `ClassProps::new` invariant (only a `PropType::ObjectRef` row with a named
    `object_class` — the one arm that reads it) and a carrier-set pin
    (`ref_miss_message_rows_are_the_measured_set`).
  - **Surfaces.** The row is `HIDE_R4133` (r4133-only ⇒ absent from BOTH pinned
    tables) and joins `PROPS_015X`; the flag's carrier set assert grows to four
    and (audit round) its doc now says what it always meant — "absent from both
    pinned tables", never "not implemented" — with a re-measured blast radius.
    Bytes moved: `json/schema_full_port.json`
    (21 ordinal renumberings, no new block) and `json/schema_divergences.json`
    (a fourth `port_hidden_property` row at `$dssPropertyIndex` 39 /
    `$dssPropertyOrder` **32**, plus the AutoTrans BH trio's indices 42/43/44 →
    43/44/45), with `golden.lock.json`. The order rank was **measured**, not
    argued (the RP1.1 audit's lesson that the shift rule absorbs an off-by-one):
    regenerating moved the prop that stood at 39/32 — `XRConst` — to 40/33.
    `props/autotrans.json` provably does not move (the capture enumerates the
    0.14.5 oracle's own names, `props_roundtrip` 251 passed unchanged), and no
    `Dump`/`Save`/JSON byte moves (`prop_line` skips a hidden row; the dump's
    generic tail starts at prop 28 and walks over it).
  - **Census re-run (the WP-RP1 per-sub-step obligation).** Two full
    `DSS_PROPS_CENSUS=1` walks — pre-RP1.2 (438 cases, 56.5 s) and post-RP1.2
    (439, 56.8 s), both channels — so the delta is a diff, not a derivation:
    r4133 shape classes **3 → 2**, structural pairs **218 → 222**, numeric
    **98 → 103**, cells hidden behind a desynchronized name list **977 → 347**;
    the `capi_v0145` channel is bit-unchanged (3 / 13 / 0 both times). The **9**
    new pairs are recorded in `tests/corpus/props_r4133/README.md`
    §"Pairs the WP-RP1 shape closures make live" with their bins and cell counts.
    Six are covered machinery (bin 1 `enabled`/`xrconst`, bin 5
    `bhcurrent`/`bhflux`, bin 6 `normamps`/`emergamps` at 2.1e-5/4.0e-5 — an
    order of magnitude under the worst display pair). Of the three bin-7 rows,
    `pctperm`/`repair` are frozen-default echoes of exactly the family bin 7
    already enumerates (`InitPropertyValues` `:1958-1959`; the class's
    `GetPropertyValue` re-renders only PD-tail slots 1–2, `:1885-1888`, so 4–5
    fall through to the string store) — in-scope bin 7 grows 17 → 19, RP2.2
    disposition, RP2.3 landing site. The third, **`autotrans.wdgcurrents`**
    (max_rel 7.54e-2), is a **genuine jump and entirely out of scope**: all 35
    cells sit on `engines: "capi_v0145"` cases (the four `controls:autotrans/*`
    RegControl decks + one `modes:makeposseq/makeposseq_xfmr` cell), 0 in scope.
    It is not a port defect — the same 35 cells match exactly on the capi
    channel, and the two engines' `GetAllWindingCurrents` bodies are
    statement-for-statement identical, so the inputs differ, not the algorithm;
    ~4e-4 on the series winding against ~3–7 % on the common winding is the
    signature of a different landed tap, which is why those decks are capi-only.
    Recorded for RP2.2's closed pair list; no RP3 sub-step opens while it stays
    out of scope. — **Correction (2026-09-03, RP3.12):** the scope reading
    stands (still 0 in-scope cells) and the last clause does not: RP3.9's P0 open
    item opened §RP3.12, which decomposed the "different landed tap" — r4133's
    `RegControl` reaches its controlled element through an unchecked
    `TTransfObj(ControlledElement)` cast (`RegControl.pas:926`, `:1026`, `:1296`,
    `:1370`, `:1479`) over `TAutoTransObj = class(TPDElement)`
    (`AutoTrans.pas:88`), so `TapIncrement` reads the winding's `MaxTap` and
    r4133 never taps an AutoTrans at all (0 event-log lines on all four decks).
    `UPSTREAM_BUG`, never reproduced; the 8 `controls:autotrans/*` spellings are
    `Owner::Rp312`'s and the ninth (`modes:makeposseq`) stays RP3.9's. The same
    correction is dated into `tests/corpus/props_r4133/README.md` §RP1.2.
  - **Audit settlement (2026-08-23, `/audit-code` + `/audit-tests`, 8 findings:
    1 major, 7 minor — 7 fixed, 1 recorded-not-changed, 0 refuted).** Written up
    as **7** items below: the missing `ref_miss_message` invariants were raised
    by both auditors (audit-code's third minor and audit-tests' first) and are
    settled once, in item (3).
    (1) *major, code* — the `HIDE_R4133` un-hide measurement (RP1.1: "4 committed
    artifacts, `dump3_commands` +6 rows, no corpus case") was left describing a
    three-carrier flag while RP1.2 added a fourth **on a new class** — and the
    prose claiming otherwise sat in the sub-step's own record. **Fixed by
    re-measurement** (2026-08-23, the flag's own method: drop its arm from
    `hidden_from_full_enum`, `cargo test -p dss-core --no-fail-fast`): **8**
    committed artifacts move — `json/der_usermodel_{assigned,full}.json`,
    `json/dyneq_full.json`, `json/autotrans_micro.json`,
    `json/autotrans_solved.json` (the AutoTrans FULL views gain `"XfmrCode"`
    between `Bank` and `XRConst`), `reports/dump_autotrans.txt` (56 → 57 rows),
    `reports/dump_autotrans3.txt` (63 → 64) and `reports/dump3_commands.txt`
    (2328 → 2335, i.e. **+7**: one help line each for the two Generator props and
    for AutoTrans `XfmrCode`, four for Sensor `Action`'s multi-line help) — plus
    `schema_full_port.json` and the deletion of the **4** `port_hidden_property`
    rows, and **still no corpus case** (`corpus_gate_all_cases_match_engines`
    green with the arm off). AutoTrans is the first carrier class that owns
    committed `Dump` goldens, which is exactly why the number moved. Both copies
    of the measurement (the flag's doc and `ORPHANED_GAPS.md` §2, the row
    GOLDEN_REBASE G3.3c/G3.4 will read as its scope estimate) now carry it, and
    the carrier-set assert's message points at them.
    (2) *minor, code* — `set_object_ref`'s doc justified the silent empty-name
    arm with "r4133's `SetActive('')` misses too", which would in fact log
    #100180 and make the port's silence a divergence. **Fixed**: the real reason
    is the one its own pin records — r4133's `Edit` loop is
    `WHILE Length(Param)>0 Do` (`AutoTrans.pas:472`), so property 39 is never
    reached; the comment now says that and names the pin.
    (3) *minor, code+tests* — `ref_miss_message` landed without either guarantee
    its model `stub_message` has. **Fixed both ways**: a `ClassProps::new`
    `debug_assert` (the field is read only by the `PropType::ObjectRef` +
    named-`object_class` arm of `parse_into`; anywhere else it is a silently dead
    message) and a carrier-set equality pin,
    `compat_quirks::ref_miss_message_rows_are_the_measured_set`, which also
    re-asserts the shape invariant over the live tables. Reported independently
    by both auditors (audit-code minor 3, audit-tests minor 1) and settled once.
    (4) *minor, tests* — several cells of `every_copied_field_arrives` equalled
    the AutoTrans defaults and could not fail: winding 1's `kV` (115), winding
    3's `Tap`/`MaxTap`/`MinTap`/`NumTaps` and — the only cell for its own copy
    statement — `%LoadLoss`, whose code value was `%r` 0.21 + 0.19 = the default
    0.4. Not a coverage hole (the corpus deck's r4133 probes gate `%loadloss`
    live at one phase, and each field had another witness among the other
    windings), but cells that cannot be trusted as pins. **Fixed at the
    fixture** (winding 1 kV 115 → 138 with the circuit
    base, `%r` 0.21 → 0.33 ⇒ `%loadloss` 0.52, winding 3 off the default
    tap/min/max/numtaps), the table extracted into `PER_WINDING`/`WHOLE_ELEMENT`,
    and a new guard `no_asserted_cell_can_be_read_off_the_defaults` makes the
    rule executable. Both halves proven non-vacuous by probe: deleting
    `self.pct_load_loss = code.pct_load_loss()` now reds
    `every_copied_field_arrives` (it did not before), and pointing one cell back
    at a default reds the guard.
    (5) *minor, code* — an **inherited** divergence the sub-step read past:
    AutoTrans `bank=`. r4133 keeps the assignment commented out
    (`AutoTrans.pas:519`) and answers the write with #100130 (`:566`), so its
    `XfmrBank` is always `''` and every auto is its own CIM bank
    (`ExportCIMXML.pas:3280`); dss_capi 0.14.5 restored the property
    (`AutoTrans.pas:482`) and the port follows it, using the name as the CIM
    bank-grouping key and never logging #100130. **Recorded, not changed** — and
    deliberately so: unlike the `XfmrCode` arm, r4133's pair here is
    self-consistent (a documented limitation, not a bug), so nothing is reported
    upstream, and dropping the property would remove working behavior and move
    CIM output — a decision outside RP1.2's scope with no owning plan. Latent:
    every `bank=` in the corpus is a Transformer, the property echo agrees on
    both channels and the CIM goldens are capi-captured; it becomes observable
    only on an r4133-gated deck that writes `bank=` on an auto (r4133 would log
    #100130 there). The evidence now sits at the property row in
    `auto_trans/mod.rs`.
    (6) *minor, docs* — two stale citations. **Fixed**: the corpus deck's header
    cited `AutoTrans.pas:2358-2362` for the connection override (the `Case i of`
    block is `:2356-2361`, and `:2357` — winding 1 → SERIES — sat outside the
    quoted range), and the upstream report said the second `CASE` block is
    "twelve lines later" (it is 46; now phrased without the count) with the
    `bank=` assignment at `:520` (it is `:519`) — plus, found while fixing those,
    "a real, 150-line routine" for `FetchXfmrCode` (`:2339-2396` is 58). Both
    auditors re-checked every other Pascal line number in the sub-step and found
    them correct. The report edit is **not in this commit**: `investigations/`
    is gitignored (`.gitignore:5`), so it lands in the working tree only.
    (7) *minor, docs* — the frontier said "10 tests" where the record said 11.
    **Fixed**: both now say **12** (the guard of item 4 is the twelfth).
  - Gate: all five commands green, re-run after the settlement. `lane_diff.ps1`
    run for the sub-step: **PASS**, 522 cases / 3 220 247 records, `max |Δ| = 0`
    exactly on every gated kind (conv, cur, errs, iter, loss, pow, v, y), zero
    iteration drift — the default lane stays bit-identical to the parity lane,
    so it keeps precisely the parity lane's oracle standing. Not re-run for the
    settlement: it touched no solved state and no compat kernel (the fixture
    change lives inside an in-engine test's own deck).

- **RP1.3** (2026-08-23) — WindGen `UserModel`/`UserData`, a **real behavioral
  port over the WASM user-model host**. r4133 properties 18/19
  (`Version8/Source/PCElements/WindGen.pas:391-395`, Edit arms `:641-642`,
  `MakeLike :829` + `:834-835`, `GetPropertyValue` 18 `:2903`, defaults `''`
  `:2453-2454`) and the whole `Model=6` behavior they feed now exist, closing the
  fourth `shape.txt` gap (windgen 46 → 48 names). dss_capi 0.14.5 does not carry
  the class **at all**, so there is no capi witness for any of it, the rows join
  no allowlist (`PROPS_015X` untouched), and **no `props/` golden exists or was
  created** (plan §1.2).
  - **The shuttle is WindGen's own, and its layout was measured, not read off.**
    `TWindGenVars` differs from `TGeneratorVars` in three ways
    (`WindGenVars.pas:20-73`): no NCIM `deltaQNom` (so the integers stay at
    176/180/184 and the Thevenin tail keeps the Appendix-A offsets),
    `kVGeneratorBase` → `kVWindGenBase`, and a 13-double aerodynamic tail
    `ag`…`s` behind a managed `PLoss: string` reference. A new FPC probe
    (`tools/fpc/usermodel_abi/abi_probe_windgenvars.pas` →
    `docs/wasm/probes/p10_offsets_windgenvars_r4133.txt`) gives native
    `SizeOf = 356`. The wasm image is **348 B with `PLoss` dropped and the hole
    CLOSED** (`ag`@244, `s`@340) — the rule ABI §2.2b already applies to
    `deltaQNom` — which makes the WindGen image a byte-for-byte **extension** of
    the 244-byte Generator image, pinned by
    `records::tests::windgen_vars_head_matches_generator_vars` and recorded as a
    successor trap in ABI §2.6b + `PIN.txt` (never re-insert the hole).
    `crates/dss-usermodel` gained `WindGenVars`,
    `InterfaceKind::WindGenUserModel` and a generic `IntoShuttle`/`ShuttleVars`
    seam: the call surface became generic instead of growing a field, so
    dss-core compiled unchanged while the host half landed, and a kind/record
    mismatch is a loud `Usage` error (a Generator-sized 244 B buffer *traps* the
    348 B guest — pinned).
  - **Guest fixture.** `tests/fixtures/wasm/wgturbine.wasm` (61 425 B, sha256
    `6d5a9a66…f5471cc2`), source crate `tools/wasm_usermodel/models/wgturbine/`,
    `build_wgturbine_wasm.ps1`, PIN row + `fixture_pin.rs` row —
    **reproduced bit-identically from a wiped target dir**. It is a
    constant-admittance source in power flow and a first-order current+speed lag
    in dynamics, then fills `Pg/Ps/Pr/Pm/s/Cp/Lamda` and the record head; five
    `UserData=` keys; **fifteen** state variables (nine as first built, plus the
    six the audit settlement below added: four more record echoes and the two
    call counters `WgUpdCount`/`WgBadSet`). It **discriminates**: its currents
    are `Y·V` or the integrated state, never a native model-1/2/4/5 answer, and
    it is the only thing in the tree that writes the turbine tail (its `ag`@244
    read is the closed-hole witness).
  - **Ordinal shift.** `USERMODEL=18`/`USERDATA=19`, `DUTYSTART`…`VCUTOUT` +2,
    tails `SPECTRUM=45 BASE_FREQ=46 ENABLED=47`, `NUM_PROPS` 46 → **48** —
    `NumPropsThisClass = 44` (`:255`) + 4 inherited, which is exactly the
    census's `oracle_count`. Every `prop::` consumer is inside `windgen/`
    (grep-verified); the JSON-schema ranks are generated, never hand-written.
  - **Two substrate gaps closed on the way to model 6**, both read off r4133
    rather than inferred: the `Xd/Xdp/Xdpp` + `puXd/puXdp/puXdpp` family with
    `VTarget`, `Zthev`, `VThevHarm`/`ThetaHarm` (`Create :960-965,:940`,
    `RecalcElementData :1368-1370,:1406-1408`, `MakeLike :809,:817-819` — the
    two computation sites keep their *different* operand orders, last-ulp
    faithful), and `SetNominalGeneration`'s model-6 arm
    `Yeq := Cinv(cmplx(0, −Xd))` (`:1332-1345`, which deliberately leaves
    `Yeq95`/`Yeq105` untouched). A third, invisible divergence was corrected
    while doing it: `InitStateVars` formed `Edp`/`Yeq` from the WTG3 model's
    `Zthev` because the port had no `Xdp`, where r4133's body is one
    `With WindGenvars` and both come from the **record**
    (`Zthev = Cmplx(Xdp/XRdp, Xdp)`, `:2500-2505`). Now ported for every model
    and proven inert by the whole-corpus dump below.
  - **Two REAL engine defects, found by probe and fixed in the same sub-step.**
    (1) **`like=` produced a *dead* user model, silently.**
    `ClassArena::make_like_within` hands `make_like` an owned `clone()` and the
    slot's `Clone` drops the live wasmi instance, while every call site guards
    on `exists()` — so `New WindGen.w2 like=w1` echoed `UserModel=<path>`,
    reported **22** variables instead of 31, injected nothing and logged
    nothing (measured before the fix). r4133's `:829` is a `Set_Name`
    (`WindGenUserModel.pas:158-220`) = free + `LoadLibrary` + `FNew`, i.e. an
    **eager fresh instance at the guest's own defaults** (the donor's `UserData`
    is never replayed — `:834-835` copies only the property array, which is what
    `?` echoes). `make_like` now queues a real load, drained before `end_edit`
    so `RecalcElementData`'s `FUpdateModel` (`:1418`) sees it.
    (2) **The two-phase dynamics abort carried no error number** —
    `WindGen.pas:2538-2539` is `DoSimpleMsg(…, 5672)` + `SolutionAbort := TRUE`;
    the port pushed the text with `code: None`. Both are behavior changes inside
    RP1.3's scope, not test-only work. A third, pre-existing swallow was drained
    at the same time: `compute_inj_currents`/`get_currents` **built an
    `ErrorLog` and dropped it**, so every WindGen solve-time diagnostic
    (including the old non-3-phase guard) was invisible; they now feed
    `ctx.errors`/`ctx.solution_abort` and the element's deferred log, as the
    Generator twin always did.
  - **Two r4133 bugs found; neither reproduced** (reports in the local-only
    `investigations/to_opendss/`).
    `38-windgen-get-set-variable-usermodel-tail.md` — `Get_Variable`
    (`:2735-2743`) and `Set_Variable` (`:2777-2784`) put the user-model tail
    OUTSIDE the `if i < 19 … else case i of …` chain instead of in its `else`
    (contrast `generator.pas:2868-2884` and WindGen's own correct `VariableName`
    `:2856-2870`), so with a model loaded every native index `1..=22` satisfies
    `k = i−22 ≤ N` and is overwritten by `FGetVariable(k ≤ 0)`;
    `GetAllVariables` reads through it, so `Show Variables`,
    `AllVariableValues`, `Get StateVar` and monitor mode 3 all see it. The
    evidence is **source-structural, not probe-measured**, and honestly so: the
    bug needs a loaded native WindGen user-model DLL, none exists anywhere
    upstream (r4133 ships only the loader), so the epri-worker cannot exhibit
    it. The port routes `1..=22` native and `>22` to the model, pinned by
    `the_native_variables_stay_native_while_a_model_is_bound`.
    `39-windgen-model7-uninitialized-current-limit.md` — found while writing the
    models-3/7 deferral row: `DoCurrentLimitedPQ` reads
    `PhaseCurrentLimit`/`Model7MaxPhaseCurr` (`:70-71`), which are **never
    assigned** in the unit (the `If GenModel=7` initialiser at
    `generator.pas:1183-1187` was dropped when `SetNominalGeneration` was
    cloned), so upstream's model 7 limits every phase current to zero. Nothing
    to fix here — model 7 is not ported — but the trap is recorded at the
    deferral row so a future port does not transcribe it.
  - **Deferral recorded.** Models **3** (`DoPVTypeGen`) and **7**
    (`DoCurrentLimitedPQ`) stay out (plan §1.3): new `ORPHANED_GAPS.md` **§1.11**
    with the spec, the machinery each needs, the enum as the admission gate, and
    the model-7 trap above. The enum comment and the
    `models_3_and_7_are_still_refused` pin forward-reference that row.
  - **Tests — the fixture's law, never a captured number.**
    `exec::tests::windgen_usermodel` (new module, **18** tests): the guest's
    admittance law and untouched neutral; the turbine tail's write-back
    (`Pg` = Σ Re(V·conj(I)), `Cp`, `Lamda` = `ag`·(1+slip)); the 22 ++ 9
    variable surface with six record-head echoes; the `Get_Variable` pin; the
    tail-only `Set StateVar` routing; ordinals 18/19 and the 48-row table; empty
    defaults; re-instantiation on a second assignment; `like=` (defect 1's pin);
    #567 non-abort and #5671 abort; the first-order lag as a **two-run
    identity** (|e₂ₙ|/|eₙ| = fⁿ: 0.606610 measured vs 0.606515 wanted, 1.6e-4);
    WTG3 never stepping under model 6; the 1-phase `:2516-2522` arm; 5672;
    models 3/7 still refused; dormancy on model 1; and #570 warn-and-fallback
    (with the two WM.3-wide port conventions stated, not hidden: a FAILED load
    still echoes its name where r4133 answers `''`, and the port does not repeat
    #567 per iteration). Non-vacuity proven by **six** mutations applied in-tree
    and reverted (5/8/8/1/4/1 tests red), tabulated in the module doc. Plus
    `crates/dss-usermodel/tests/windgen_shuttle.rs` (7) and three `records.rs`
    unit tests for the codec, whose own non-vacuity probe (shifting `ag` back to
    252 — the "hole never closed" mistake) reds two of them.
  - **Dormancy: proven, whole corpus, both lanes.** With no user model bound the
    new arms must move nothing, and they do not: `lane_diff.ps1` PASS and the
    two lane dumps are **byte-identical to the pre-edit baselines** — default
    226 437 002 B `a155aaa4…8672bbfe`, parity 226 437 001 B `dd7c5d59…226e65af`
    — with the five `modes:windgen/*` slices reproducing their pre-edit sha256s
    in both lanes. (The five decks stay `engines: "r4133"`; no ledger entry, no
    manifest or population-lock byte, no tolerance moved.)
  - **Census re-run (the WP-RP1 per-sub-step obligation).** Two full
    `DSS_PROPS_CENSUS=1` walks over the same 439-case population — pre-RP1.3
    (1 059 272 rows, 55.8 s) and post-RP1.3 (1 059 277 rows, 55.5 s), both
    channels: r4133 shape classes **2 → 1** (only RP1.4's reverse
    `gendispatcher` row left) — i.e. the **5** windgen element rows the plan's
    Outcome predicted, all of them in scope — structural pairs **222 → 224**, numeric
    **103 → 103**, cells hidden behind a desynchronized name list **347 → 192**
    — the 155 that became comparable are exactly WindGen's 31 per element
    (2 count + 29 pushed out of alignment by the ordinal-18 insertion) over the
    five decks. The `capi_v0145` channel's five extracts are **byte-identical**
    before and after. The **2** new pairs are recorded in
    `tests/corpus/props_r4133/README.md` §"Pairs the WP-RP1 shape closures make
    live": `windgen.enabled` (`Yes`/`true`, bin 1) and `windgen.dynout`
    (`''`/`[]`, bin 5), 5 cells each and all in scope — both the exact shapes
    RP1.1 already measured on the Generator, **no bin-6/7 row and no genuine
    jump**, so nothing opens for RP2.2 and no RP3 sub-step. Two facts recorded
    with them: the two newly ported props produce **no pair at all** (`''` on
    both sides in all five decks — no corpus deck binds a WindGen user model),
    and the pre-existing `windgen.kva/kvar/mva` numeric pairs did not move
    (they sit at ordinals ≤ 17, inside the aligned prefix, so they compared even
    while the class carried a shape row).
  - **Goldens: the one artifact §1.2 predicts, plus a prose correction.**
    `json/schema_full_port.json` carries the port's own bytes — verified
    programmatically (both files parsed and compared key by key) that
    `$defs/WindGen` is the **sole** changed key: two property objects added at
    `$dssPropertyIndex`/`$dssPropertyOrder` **18/19**, the 28 props from
    `DutyStart` on shifted **+2/+2**, `Like`'s index 46 → 48 with its hoisted
    order 1 unchanged, the indexless `DynInit` tail's order 47 → 49, and
    `WindGenModel` gaining `"User model"`/6 — no other property field and no
    other class moved. Regenerated with
    `REGEN_SCHEMA_PORT=1` in **both** lanes to bit-identical output, satisfying
    its `produced_by: parity` row, with the one `golden.lock.json` line in the
    same commit. **`schema_divergences.json` does not gain a row**, and that is
    a measurement of what the schema gate does for this class, not an omission:
    WindGen is port-only versus 0.14.5 as a WHOLE CLASS, so it carries a single
    `port_authored_classes` entry (no `$dssPropertyIndex`/`Order` fields to get
    wrong) instead of the per-property `port_hidden_property` rows RP1.1/RP1.2
    had to add — the new props are not `HIDE_R4133` carriers and that flag's
    four-carrier set is unchanged. Its `cause` prose, which still said the
    class's metadata comes from "the dss_capi 0.15.x line", is corrected to name
    r4133 + RP1.3 (a one-sentence edit, its lock digest in the same commit); no
    other golden byte moved anywhere. `tests/TOLERANCE_NOTES.md` is owed
    **nothing** by this sub-step: no tier, floor or allowlist row is involved —
    WindGen has no capi channel, the r4133 decks already gate at the standing
    floors, and every new pin is an expected-value or fixture-law assertion.
    `TESTING.md` likewise: it carries no per-test-file inventory to extend (the
    wasm fixtures are documented by `tools/wasm_usermodel/PIN.txt`,
    `fixture_pin.rs` and the WASM phase record) and RP1.3 adds no env knob — so
    the disposition is recorded here rather than by inventing a section there.
  - Gate: all five commands green on the final tree — the parity
    `cargo test` needed **one re-run**, and the reason is recorded rather than
    swallowed: the first full parity run hit a single occurrence of the
    documented file-backed-loadshape **oracle** flake on
    `modes:upgrade/mmf_singlecol` (step 0, `SOURCEBUS.1`: the r4133 side
    2.1e-3 V below the port against an 8.2e-6 allowance — the ~2e-3 class that
    case's own `isolate: true` note describes). The port side is provably
    bit-stable: the lane dump taken minutes earlier carries exactly the failing
    "actual" (`7196.5232407823805, −4.811934119020634`) and is byte-identical to
    the pre-RP1.3 baseline, so nothing in this sub-step moved it. The case
    passes standalone and the re-run of the whole command is green; the standing
    follow-up below records the recurrence. `lane_diff.ps1` run for the
    sub-step (owed: a dispatch arm was added): **PASS**, 522 cases /
    3 220 247 records, `max |Δ| = 0` exactly on every gated kind (conv, cur,
    errs, iter, loss, pow, v, y), zero iteration drift — the default lane stays
    bit-identical to the parity lane and keeps precisely its oracle standing.
  - **Audit settlement (2026-08-23) — 11 findings (2 major, 9 minor) from two
    independent auditors (audit-code 7, audit-tests 4; the two sets are
    disjoint, so nothing was deduped). Ten fixed, one recorded; none dropped.**
    Every fix carries its own measured mutation, and the module's non-vacuity
    table grew from 6 rows to **13**.
    1. *(major, code)* **The `like=` dead-user-model defect was still live on
       Generator (both slots), PVSystem, Storage and CapControl** — RP1.3 had
       fixed only WindGen and parked the rest as an unowned item, against the
       "port gaps immediately" rule, leaving a silent wrong answer in four
       shipped classes. **FIXED** in all five slots (see the standing-follow-ups
       entry below for the Pascal citations), each with its own pin, each pin
       measured non-vacuous by reverting the fix.
    2. *(minor, code)* The `InitStateVars` record-`Zthev` correction was
       documented as invisible to every oracle channel — **wrong**, and the
       changed path had no test. **FIXED**: the claim is corrected in place, and
       the new `a_dynamic_eq_windgen_still_reports_the_models_variables` binds a
       `DynamicEq=` with a `theta = Edp` init pairing (domain code 9,
       `WindGen.pas:2588`) and re-derives `Cang(Edp)` from the engine's own
       solved terminal V/I at `Zthev = Xdp/XRdp + jXdp`; forming `Edp` from the
       WTG3 impedance instead reds it.
    3. *(minor, code)* Out-of-range `VariableName` answered `"ERROR"`, a
       dss_capi-only seed (`Generator.pas:2736`) that r4133's WindGen does not
       have — and the commit's new comment presented it as Pascal-faithful.
       **FIXED**: WindGen answers `''` (r4133 `:2830-2832`, `:2856-2879` — no
       `'ERROR'` literal in the unit) and the comment says why. The three
       sibling classes that DO gate on capi goldens (Generator, IndMach012,
       Storage) keep the capi seed — recorded here, deliberately out of RP1.3's
       scope, since r4133 drops it there too and that is an UPGRADE-lane call
       with golden bytes attached.
    4. *(minor, code)* `GetAllVariables` dropped the user-model tail on the
       `DynamicExp` path — an unported r4133 arm (`:2804-2807` sits outside the
       if/else) that the new doc comment did not declare. **FIXED**: the tail is
       appended after **either** arm, from a single `variable_base()` that
       `num_variables`/`variable_name`/`get_all_variables`/`set_user_model_variable`
       all share. Upstream's own inconsistency here (a `NumVariables` with no
       `DynamicExp` branch, so the reported count and the filled block disagree)
       is corrected, not reproduced, and both are documented.
    5. *(minor, code)* Two wrong Pascal citations. **FIXED, and re-measured
       against the vendored unit rather than patched by hand**: `FNew`
       `WindGenUserModel.pas:34` → **:33** (6 sites), the 15-function binding
       block `:180-194` → **:194-208** (4 sites), `Set_Edit` `:150-154` →
       **:152-156** (3 sites). The same sweep caught two more the auditors had
       not: `Get_Exists` `:131-139` → **:130-138** and `Integrate` `:141-145` →
       **:140-144**.
    6. *(minor, code)* `ensure_live`'s revive branch had no reachable caller —
       dead code that read as a working fallback. **FIXED**: every engine-side
       call site now goes through a new `take_live_user_model`, which revives a
       cloned slot from its spec (replaying `UserData=`) and reports #569 if the
       re-creation itself fails; `user_model_num_vars` stopped gating on
       `exists()` (an element snapshot's surface used to collapse to the native
       22); the helper `user_model_exists`, unused afterwards, is deleted. Pinned
       by `an_element_snapshot_revives_its_user_model`, which drives the very
       `ClassArena::clone_ckt` API the control dispatch uses.
    7. *(minor, code)* The #567/#5671 suppression for a designated-but-unloadable
       `UserModel=` (and the matching `? …UserModel` echo of an attempted name)
       lived only in a test doc comment. **RECORDED, behavior kept**: new
       `docs/upgrade/DIVERGENCES.md` **§L6** with both behaviors, the r4133
       lines, the evidence, the decision and the gate consequence — the
       native-DLL name can never load in a `forbid(unsafe_code)` engine, so #570
       is the one actionable diagnostic, and following r4133's dynamics abort
       would kill vendored decks wholesale for a model we cannot run either way.
       The pin now cites §L6.
    8. *(major, tests)* The write half of the deliberately-not-reproduced
       `Set_Variable` mis-nesting had **no pin** — reproducing it upstream-style
       left all 18 tests green, because the committed guest silently ignored the
       stray index. **FIXED at the fixture**: `wgturbine` now counts
       out-of-range `SetVariable` calls in a new `WgBadSet` variable, and the pin
       asserts it stays 0 after a native `vwind` write; the same mutation now
       reds exactly that test.
    9. *(minor, tests)* `WgConnEcho` was a default-equal cell (`Conn` of a wye
       WindGen is 0, the same value an unmarshaled field carries), so dropping
       `conn` from the engine→record shuttle passed everything. **FIXED**: new
       `a_delta_windgen_echoes_its_own_connection` (a `conn=delta` deck: `Conn`
       1, `NumConductors` 3) plus a non-default `conn` in the crate-side codec
       fixture; `conn: 0` now reds.
    10. *(minor, tests)* `RecalcElementData`'s `FUpdateModel` — a listed RP1.3
        deliverable — had no test, because the guest's `update_model` was an
        empty body. **FIXED**: the guest now re-reads the boundary record and
        counts the call (`WgUpdCount`), following the `indmach012a` precedent;
        `recalc_element_data_updates_the_bound_model` pins one call per
        `RecalcElementData` **and** the refreshed `kVArating` echo after an
        `Edit … kva=`; deleting the call site reds it.
    11. *(minor, tests)* The 43-field engine→record mapping was exercised for
        ~7 fields, so a wrong source field elsewhere passed. **FIXED** by
        echoing four more, one per region of the image — `Xdp`@88 (head
        doubles, derived `puXdp·1000·kV²/kVA`), `VTarget`@212 (the unaligned
        stretch), `Poles`@268 and `VCutin`@292 (turbine tail, both set off
        their `Create` defaults by the deck) — pinned by
        `the_record_fields_come_from_the_elements_own_sources`; `xdp: g.xd`
        now reds.
    The fixture was therefore **deliberately regenerated** (findings 8/10/11 —
    to make engine behavior observable, never to make a failing test pass, plan
    §2.9-4): `wgturbine.wasm` 60 281 B → **61 425 B**, sha256 `c441df68…` →
    **`6d5a9a66…f5471cc2`**, reproduced bit-identically from a wiped target dir
    with the same pinned toolchain, `PIN.txt` updated with both the new digest
    and the superseded one. No golden, no ledger row, no manifest byte, no
    tolerance and no `tests/corpus` byte moved for any of the eleven.
    **Dormancy re-proven on the settled tree**: the default-lane whole-corpus
    state dump (`examples/lane_dump`, 522 cases) is byte-identical to part A's
    PRE-RP1.3 baseline — 226 437 002 B, sha256
    `a155aaa403dc933b36ba535a8f7bd5c43828eb7c82035be1f04419cd8672bbfe` — so none
    of the eleven settlements moves a solved state anywhere in the corpus.
    `lane_diff.ps1` is not owed again (no compat kernel, lane alias or solver
    touched; the default/parity split is untouched by every change above).

- **RP1.4** (2026-08-23) — GenDispatcher `weights`: the allowlist row + the
  upstream report. **Zero engine change** — the port is right and matches
  dss_capi (`gen_dispatcher/mod.rs:63-67,90`, dispatch `compute.rs:28-79`); the
  fifth and last `shape.txt` row is the only one that runs **backwards**, a
  property the port has and r4133's own table loses.
  - **The r4133 bug, verified line by line.**
    `TGenDispatcher.DefineProperties` names seven properties
    (`Version8/Source/Controls/GenDispatcher.pas:127-133`, `PropertyName^[7] :=
    'Weights'`) but declares six (`NumPropsThisClass = 6`, `:92`) and hands the
    cursor to the base class at the declared count (`ActiveProperty :=
    NumPropsThisClass`, `:148-149`). `TCktElementClass.DefineProperties` writes
    at `ActiveProperty + 1` (`Common/CktElementClass.pas:98-99`), so slot 7
    becomes `basefreq`, slot 8 `enabled`, and `TDSSClass.DefineProperties`
    appends `like` at 9 (`Common/DSSClass.pas:307-308`). `CountProperties` sized
    the array for 6+2+1 = 9, so nothing overruns and nothing complains. The
    class constructor then builds `CommandList` from the **overwritten** names
    (`:103-105`) while the `Edit` dispatch `CASE ParamPointer OF … 7:` is still
    the weights arm (`:200-206`) with no `ELSE` route left to
    `ClassEdit`'s `BaseFrequency :=` (`CktElementClass.pas:53`).
    `InitPropertyValues` collides the same way (`:495` then `:499` →
    `Common/CktElement.pas:1307`).
  - **Measured on the git-tracked r4133 DLL** (epri-worker, three probes):
    `AllPropertyNames` returns **9** names with no `weights`;
    `? GenDispatcher.gd1.weights` → `Property Unknown`; `weights=[3, 1]` →
    **DSS error #364** `Unknown parameter "weights"`; `basefreq=[3, 1]` is
    accepted and `? …basefreq` echoes `3, 1`. The 7th **positional** parameter
    still reaches the weights arm (positional parsing skips the command list,
    `:186`). And a plain, legitimate `basefreq=60` silently sets
    `FWeights := [60, 0]` (`InterpretDblArray` pre-sets `Result := MaxValues`
    and zero-fills, `Common/Utilities.pas:683,789-791`): measured on a
    two-generator dispatcher, g1 2019.63 kW / g2 1000 kW (never dispatched)
    against the correct 1509.81 / 1509.81, with `? …basefreq` still answering
    `60`. Report:
    `investigations/to_opendss/40-gendispatcher-weights-registration-off-by-one.md`
    (fix = `NumPropsThisClass = 7`; no other ordinal moves).
  - **Liveness decided BY MEASUREMENT — the decks stay `capi_v0145`.** The
    census's `generator.kw/kvar` deltas on the three `controls:gendispatcher/*`
    cases root-cause **entirely** to the registration bug: on r4133 the decks'
    `weights=[3, 1]` is rejected, so `FWeights` keeps the `[1, 1]` the `GenList`
    arm installs (`:215-220`) and both machines split equally (step 1: g1 =
    g2 = 382.892 kW) while the port splits 3:1 (532.407 / 232.659). Measured over
    all 12 steps of `gendispatcher.dss`: the kW gap is **48 %** at the worst of
    steps 1-11 (step 6, 1880.18 against 2778.88 / 981.48) and **54.5x at step
    0**, where r4133's `Max(1.0, …)` floor (`:450`) holds BOTH its machines at
    1 kW while the port dispatches g2 to 55.52 — that step-0 cell is the census's
    `generator.kw` pair (`max_rel` 5.45e+01); kvar reaches 1.59e+00. (The record
    first wrote a bare "up to 48 % at every step", which omitted step 0 — the
    larger, and the census's own maximum; corrected in the audit settlement
    below.) Feeding r4133 the same vector through the misregistered
    name (`basefreq=[3, 1]`) reproduces the **port's** split to the last
    displayed digit (532.407 / 232.659; kvar 172.809 / 261.237), which proves the
    dispatch algorithms agree and only the parse does not. That is a
    whole-solution divergence, not a `property`-scoped one, so **no pin could
    cover a flip to `engines: "both"`** (§1.1(e) `property` scopes are per-case
    and divergence-only) — the decks stay capi-only, no manifest or
    `population.lock.json` byte moves, and the row's comment says *dormant until
    a gendispatcher deck gates r4133*. Recorded here and in the row itself; no
    silent third state.
  - **The row.** `("GenDispatcher", &["weights"])` joins `PROPS_015X`
    (`tests/harness/mod.rs`) with the r4133-bug provenance in its comment, and
    the table's own doc comment gains the one sentence that admits the second
    direction (the mechanism is channel-agnostic: `prop_015x` drops a Rust prop
    only when **this** capture's name list lacks it, `filter_015x`
    `mod.rs:1680-1692`). So the row is **inert on the capi channel** — 0.14.5
    counts the enum (`.inputs/dss_capi/src/Controls/GenDispatcher.pas:106`) and
    reports `weights`, so it is kept and fully value-compared, exactly as before
    this sub-step — and relieves the shape walk on r4133 only.
    `tests/TOLERANCE_NOTES.md` §"0.15.x property-table allowlist" gains the
    matching r4133-extension bullet. Non-vacuity is pinned against the
    **shipped** table, not a synthetic one, by
    `props_015x_tests::shipped_gendispatcher_weights_row_is_inert_when_the_oracle_knows_it`:
    the row must be in the shipped table, the r4133-shaped name list lines up,
    and a wrong `Weights` value against a capi-shaped list still panics — with
    the panic MESSAGE asserted, so an unconditional `filter_015x` (which would
    panic on the count instead) cannot satisfy it. That message assert is the
    audit settlement below; as first written the test caught only a value-mask
    regression, and the comment claimed a guarantee it could not discriminate.
  - **Census re-run (the WP-RP1 per-sub-step obligation).** Two full
    `DSS_PROPS_CENSUS=1` walks over the same 439-case population — pre-RP1.4
    (1 059 277 rows, 56.3 s) and post-RP1.4 (1 059 277 rows, 56.5 s), both
    channels: r4133 shape classes **1 → 0**, so with this the full census closes
    **429 → 0** and **WP-RP1's whole-WP acceptance criterion is met**.
    Structural pairs **224 → 225**, numeric **103 → 103**, cells hidden behind a
    desynchronized name list **192 → 0** — those 192 are exactly the 48
    GenDispatcher (element, step) rows — one dispatcher in each of the three
    decks, over 12 + 12 + 24 steps — × **4 each**, where the 4 is what
    `collect_element_divergences` actually counts: 1 for the count delta
    (`|filtered| - |oracle|`, 10 vs 9) plus 3 name disagreements along the zip
    (`weights` vs `basefreq`, `basefreq` vs `enabled`, `enabled` vs `like`); the
    Rust list's tenth entry `like` is never reached, the zip stopping at the
    oracle's nine. (The record first said "the four tail positions the misplaced
    `weights` desynchronized (`weights`/`basefreq`/`enabled`/`like`)" — the
    total was right, the mechanism was not; corrected in the audit settlement
    below.) The row count is unchanged overall because the 48
    `shape_count` rows were replaced one-for-one by 48 new value rows. The
    `capi_v0145` channel's five extracts are **byte-identical** before and after
    (3 structural / 13 numeric / 0 shape / 34 diverging cases / 312 elements
    skipped whole). Exactly **one** new pair, recorded in
    `tests/corpus/props_r4133/README.md` §"Pairs the WP-RP1 shape closures make
    live": `gendispatcher.enabled` (`Yes`/`true`, bin 1, 48 cells, **0 in
    scope** — the decks are capi-only). Of the other three positions the closure
    aligns, `weights` is the one the row drops, and `basefreq` and `like`
    agree cell for cell (`'60'` / `''`, probe-confirmed on both sides). **No
    bin-5/6/7 row and no genuine jump**, so nothing opens for RP2.2 and no RP3
    sub-step.
  - **Goldens: measured, none moved.** The whole diff is tests + docs + the
    gitignored report; `PROPS_015X` lives in the test harness and no capture,
    schema or `props/` artifact reads it. Measured by the full five-command gate
    on the final tree (every golden comparison green) and by `git status` (no
    `tests/golden/` byte, no `golden.lock.json`, no `population.lock.json`, no
    ledger row). **No `lane_diff` is owed** — zero engine change: no compat
    kernel, lane alias or solver was touched, and no product crate byte moved at
    all.
  - **Audit settlement (2026-08-23).** Two auditors raised **9** findings, all
    severity **minor**, **4** from `audit-code` and **5** from `audit-tests`.
    One issue was raised by BOTH (the new harness test's doc comment naming a
    failure mode the test could not discriminate), so the findings cover **8**
    distinct issues: **7 fixed**, **1 recorded here and deliberately not
    fixed**, **0 refuted**. Still zero engine change — every edit is a test, a
    test-only diagnostic print, or a doc.
    1. *(raised by both auditors)* **The test's stated guarantee was not the one
       it could catch.** Both mutation-proved it: an unconditional `filter_015x`
       trips the count assert *inside* the `catch_unwind`, which a bare
       `is_err()` accepts. Fixed two ways — the test now asserts the panic
       MESSAGE names the `Weights` value compare (a count panic no longer
       satisfies it, so it does discriminate the unconditional-filter
       regression), and the doc comment now enumerates what it pins: the value-
       mask regression (nothing else catches it — proven by `audit-tests`'
       mutation B2), the unconditional filter (also caught loudly and first by
       `allowlisted_present_in_oracle_value_mismatch_panics`, so "silently" was
       wrong), and deletion of the row itself.
    2. **"up to 48 %" was written as an unqualified maximum.** Re-measured here
       over all 12 steps of `gendispatcher.dss` on the r4133 DLL: kW 48 % at the
       worst of steps 1-11 (step 6), **54.5x at step 0** — which is the census's
       own `generator.kw` `max_rel` 5.45e+01 — and kvar 1.59e+00. Corrected in
       all six places (this record, the frontier paragraph, the `PROPS_015X` row
       comment, `tests/TOLERANCE_NOTES.md`, the vendored `README.md`, and the
       upstream report, whose table gains steps 0 and 6). The direction of the
       error was harmless — the true gap is larger — but it contradicted the
       project's own measurement.
    3. **The 192-cell decomposition named the wrong mechanism** (the total was
       right). `collect_element_divergences` counts the count delta plus one
       cell per disagreeing zip index; `like` is never walked. Corrected here
       and in the vendored `README.md`, which is the file RP2.1/RP2.2 read.
    4. **The row and `TOLERANCE_NOTES.md` claimed present-tense offline liveness
       through `crates/dss-core/tests/props_r4133_replay.rs`, which does not
       exist yet** (RP2.1's deliverable). Both now name today's only exerciser —
       the harness test above — and put the replay in the future tense. Plan
       §1.1(d) is untouched: it states a requirement, not a fact about the tree.
    5. **Plan §1.2's enumeration of the RP2.1 supplement's sources was stale**
       — it named RP1.1 only ("eleven pairs ... with autotrans/windgen to
       follow"), which also miscounted RP1.1 (12 pairs; eleven was the number in
       already-covered bins). Now closed out at 12 + 9 + 2 + 1 = **24 pairs**,
       matching structural 210 → 225 and numeric 94 → 103, with RP1.4's
       `gendispatcher.enabled` called out as the one reached through a
       `PROPS_015X` row rather than a port.
    6. **`PLAN_SEQUENCE.md` row 5b still read QUEUED.** Flipped to IN FLIGHT
       with WP-RP0/WP-RP1 COMPLETE, in row 5a's own house style; RP5.2 still
       owns the flip to COMPLETE and says so in the row.
    7. **A census run can come back short and read as complete.** `audit-tests`
       saw one run at HEAD report 1 059 237 rows / 224 structural against the
       recorded 1 059 277 / 225, with `cases_walked` still 439 and nothing red;
       two further runs reproduced the recorded numbers exactly, and a fourth,
       run here after the fixes, reproduced them again (439 cases, 1 059 277
       rows; r4133 225 / 103 / 0 shape / 434 diverging / 0 unaligned; capi
       3 / 13 / 0 / 34 / 312 skipped whole). The mechanism is `census_one`: a
       channel hiccup on one case turns
       that case's whole divergence population into ONE `oracle_error` row, and
       those counts lived only in `props_census.json`. Fixed by printing
       `oracle error(s)` / `rust error(s)` on the per-channel summary line and
       recording the measured baseline in `TESTING.md` — **5** on r4133 (the
       #303 crash decks) and **22** on `capi_v0145` (decks the 0.14.5 oracle
       cannot compile or solve: post-0.14.5 spellings, WindGen,
       `modes:upgrade/*`) — so a short run is visible without opening the JSON. The knob still asserts
       nothing about the data by design (RP0.2) — RP4.1's single-census
       acceptance must read that count.
    8. **RECORDED, deliberately not fixed — the dormancy closes the row's live
       coverage, and a weights-free r4133-gating deck was never measured.**
       `audit-tests` is right that no other corpus deck holds a GenDispatcher,
       so after RP4.1 the class gets no live r4133 property compare at all.
       Measured here: with the `weights=` line deleted, r4133 dispatches exactly
       as it does with it present (g1 = g2 = 382.892 kW at step 1, 757.77 at
       step 2 — both engines levelize the weights to 1.0 in the `GenList` arm,
       `GenDispatcher.pas:215-220` and `gen_dispatcher/compute.rs:28-31`) and
       the port answers the same for both machines (`Export Generators`, Max kW
       758, equal kWh) — so such a deck could gate `both` and would make the row
       live. **Not taken inside RP1.4**: the plan's RP1.4 offers exactly two
       outcomes, flip one of the three existing decks or mark the row dormant
       (§RP1.4, "if they are unpinnable, keep the decks capi-only and mark ...
       dormant"). RP1.2 did add a deck, but for a property it had just ported
       and with no `engines` question attached; here the deck exists only to
       give an r4133 channel to a class §1.3 deliberately leaves capi-only
       ("a broader `engines` re-audit is not [in scope]"), and it moves the
       census population, `population.lock.json` and the §1.1 case counts every
       later sub-step quotes. Recorded so the option is
       on the record for RP4.1/RP5, together with the latent trap it must avoid:
       r4133's slot 7 IS the weights arm, so a future r4133-gating GenDispatcher
       deck must not set `basefreq=` either, or the compare puts the port's base
       frequency against r4133's stored weights string (measured: `? gd1.basefreq`
       → `3, 1`). Both notes are in the row comment as well.

