# R4133_PROPS - WP-RP2 records

> Moved verbatim from `STATUS.md` on 2026-09-03 (STATUS.md archiving round 2);
> order preserved, nothing rewritten. It holds STATUS §1's `### R4133_PROPS WP-RP2`
> condensed record block (RP2.1-RP2.4 and the RP2.4 audit settlement).
> STATUS.md §7 forwards `§WP-RP2` and `§RP2.x` here.

### R4133_PROPS WP-RP2 — condensed records

> Plan: `R4133_PROPS_PLAN.md` §WP-RP2. Same branch, same per-sub-step ritual.

- **RP2.1** (2026-08-23) — channel threading, the normalization engine, the
  replay accounting and the census's disposition mode. **Zero engine change**:
  the whole sub-step is `tests/harness/*`, `crates/dss-core/tests/*`, the
  vendored evidence and docs. No golden byte, no `golden.lock.json`, no
  `population.lock.json`, no `ledger.json` entry (the §1.1(e) staging rule:
  r4133 `property` entries land at RP4.1), and the r4133 props live masks are
  untouched — every proof here is offline or annotation.
  - **The seam.** `harness::PropsPolicy { channel, value_policy }` travels
    through both property walks exactly as the shape allowlist does, and carries
    the one hook `normalize(class, prop, rust, oracle)` sitting immediately
    before `assert_value_matches_tol` (and before `value_verdict` in the census
    walk, which still records the RAW strings). Capi-invariance is
    **structural**: every RP2.1 behavior hangs off `is_r4133()`, so a capi policy
    walks the pre-RP2.1 code path. `PropsPolicy::plain` disarms the value half —
    that is the census's permanent RP0.2 baseline, so a rule can never shrink
    what a re-census reports.
  - **`SKIP_PROPS` dispositions — all 12 rows, all MEASURED** (a full census run
    with the rows unmasked on r4133, probe lists reverted): 5 rows compare on
    r4133 (`Fuse.FuseCurve`, `Fuse.RatedCurrent`, `Capacitor.pctperm`,
    `Reactor.pctperm` — 0 divergent cells each; `RegControl.RevThreshold` — 888
    cells, `'-100'` vs `'100'`, an `EchoDefault` at `RegControl.pas:1448`), 7
    stay skipped on both channels with their unmasking cost recorded
    (`Capacitor.CMatrix` 1 059, `Reactor.RMatrix`/`XMatrix` 670 each,
    `Capacitor`/`Reactor.FaultRate` 1 059/670, `Fault.GMatrix` 386,
    `Transformer.WdgCurrents` 571). `LANE_SKIP_PROPS`' Monitor `BaseFreq` and
    `skip_transformer_cursor` stay channel-blind, each with its reason.
    The two consts `SKIP_PROPS_CAPI_ONLY`/`SKIP_PROPS_BOTH_CHANNELS` **partition**
    the table, enforced by a test. Unmasking `RevThreshold` is a deliberate
    **policy** change to the r4133 census population (+1 numeric pair, 888
    cells), not a disagreement with the frozen extracts — those stay frozen; the
    pair is vendored in the supplement instead (864 `'-100'`/`'100'` + 24
    `'-800'`/`'800'`) and **RP2.3 owes it an `EchoDefault` row and its pin, or
    RP4.1 breaks on 888 cells**.
  - **The table.** `tests/harness/props_norm.rs`, **157 rows** = `BoolFold` 77 +
    `CaseFold` 63 + `ArrayForm` 17 + `EnumSynonym` 0 (RP2.2 fills the last), each
    row citing its census pair by `(pair, bin, cells)`. Count locks per kind, a
    sortedness/uniqueness pin, and per-row live counters that are armed but
    dormant. The plan's "21 array-form pairs" derives to **17**: the four
    dropped (`expcontrol.derlist`, `relay.normal`, `relay.state`, `sensor.kvs`)
    have zero claimable example rows and are each named by the plan itself for
    RP2.2 / §1.3, so the kill criterion did **not** fire. `PROPS_ECHO_R4133`
    ships empty and deliberately **unwired** — RP2.3 lands its rows and the
    consult together, so no cell can be masked before a cited row exists to mask
    it. `props_norm.rs` carries the tree's only `#[rustfmt::skip]` (the 157-row
    data table; rustfmt's call width would explode 50 rows into eight lines each).
  - **The evidence.** `tests/corpus/props_r4133/examples_supplement.txt` —
    **76 rows / 26 pairs / 4 541 cells**, measured with the RP0.2 knob on this
    tree: the 24 pairs the WP-RP1 shape closures made live (8 of them carry 2-13
    distinct spellings the README's one-representative records cannot),
    `regcontrol.fwdthreshold` and `regcontrol.revthreshold`. Locked by row count
    and cross-file disjointness in `props_r4133_evidence_lock.rs`.
  - **The replay** (`crates/dss-core/tests/props_r4133_replay.rs`, no oracle, no
    solve): **3 454 example rows = 3 378 frozen + 76 supplement, 0 unaccounted**.
    Claimed **748** (BoolFold 114 / CaseFold 442 / ArrayForm 192); shape
    allowlist, echo and floor claim 0 **by construction**, and each zero is
    asserted rather than assumed. Everything else is *declared* to a named
    owner — RP2.2 170 rows/26 pairs · RP2.3 295/72 · RP2.4 2 100/70 · RP3 7/4 ·
    out-of-scope 134/18 (asserted to carry zero in-scope cells) — so the
    accounting is total from day one and later sub-steps only move rows between
    mechanisms. Liveness **157/157**; on `examples_full.txt` alone exactly the 9
    WP-RP1 rows are dead, which is what the supplement exists to fix. The replay
    **parses the README's WP-RP1 tables**: reformatting that section reds a test
    on purpose.
  - **The disposition mode** (`DSS_PROPS_CENSUS=claims`, the RP0.2 seam RP2.1
    was to extend). Same walk, same rows, each **value** row annotated with the
    chain's verdict through the *shipped* predicates
    (`props_norm::claim_value`, and a new non-asserting
    `LedgerView::property_scope_keys` that shares the gate's own scope selection
    and `name_re` decision with `property_handled_keys` so the two cannot drift —
    pinned by `property_scope_keys_names_the_cells_the_gate_would_handle`, which
    also holds it to naming nothing for an unnamed/foreign-field/non-divergence
    scope and to moving no hit counter). Full population, 2026-08-23,
    439 cases × 2 channels, 1 060 165 rows, 56.8 s, error counts at the recorded
    baselines (5 r4133 / 22 capi — the run is complete, not short). r4133:
    **1 060 039 value cells (1 014 217 in scope), 0 shape rows**; claimed
    **510 106 / 488 703 in scope** = BoolFold 294 519, CaseFold 93 230,
    ArrayForm 122 357; echo 0, floor 0, ledger 0; **UNCLAIMED 549 933 / 525 514**
    over 189 pairs, every one of them landing on the owner §1.1 predicts (bin 5 →
    RP2.3 425 419 in scope, bin 6 → RP2.4 45 552, bin 7 → RP2.3/RP3 45 093, bin 3
    → RP2.2 3 691, the nine bin-1 echo pairs 3 293, bins 2/4 residue 318, the
    WP-RP1/supplement pairs 2 148). RP4.1's acceptance is that in-scope column
    reaching zero. The same run reads the capi channel too, and both halves of
    the design show up there: **0** cells normalized and **11 `ledger-hit`s**
    over 8 pairs (the `makeposseq-cuf-applied-capi-props` entry) — so the ledger
    link is exercised, not dead code, even while the r4133 side must stay at zero
    until RP4.1. That capi zero was *measured* as first landed and is
    **structural** since the audit round: `claim_value`/`for_value` take the
    channel and reach no r4133 link on capi (finding 1 below).
  - **The plan's ~491 000 headline, reconciled by measurement.** Frozen bins
    1/2/4 hold 491 854 in-scope cells; measured claimed is 488 703. Every term of
    the difference is measured and vendored in the README's new dated section:
    −2 092 for the 9 bins-1/2/4 pairs that take no rule row (the five pure-echo
    bin-1 pairs + the four the plan routes elsewhere), −1 519 for the
    heterogeneous halves *inside* pairs the table does hold, +24 population drift
    since 2026-08-08 (RP0.2's cursor correction, RP1.2's autotrans), +436 for the
    9 WP-RP1 pairs. Independent cross-check of the two accountings: the census
    measures **749** claimed spellings against the replay's 748 — the extra one is
    `autotrans.conn | 'series' | 'Series'`, a cell RP1.2's closure created. Since
    the audit round that gap is a **locked term**, not a note:
    `props_r4133_replay::LIVE_ONLY_SPELLINGS` names it, asserts `748 + 1 = 749`,
    asserts the shipped `CaseFold` row still claims it, and asserts it really is
    absent from both vendored files (finding 5 below).
  - **Non-vacuity (scratch probe, deleted after the run).** Driving the real
    `compare_all_properties` on the r4133 channel: the legitimate r4133 spellings
    pass (`Capacitor.c1.enabled` vs `'true'`, `Line.l1.ratings` vs `'(400)'`,
    `EnergyMeter.em1.peakcurrent` vs `'(400, 400, 400)'`) and all four
    corruptions still fail —
    `Capacitor.c1 property Enabled: structure differs (actual "Yes" vs expected "false")`,
    `EnergyMeter.em1 property PeakCurrent: structure differs (actual "[ 400 400 400]" vs expected "(400, 4X0, 400)")`,
    `Line.l1 property Ratings: number 0 differs: actual 400 vs expected 404 (from "[ 400]" vs "[ 404]")`
    (the 1e-2 case with the skeletons deliberately equal, so the red comes from
    the NUMBER), and the same 404 inside r4133's paren form. The probe also
    caught a **latent trap for RP4.1**: `NORM_VISITS`/`NORM_HITS` are per-process
    statics, so three `props_norm` unit tests that called
    `assert_norm_rows_are_live()` would start failing on test *ordering* the
    moment the gate visits rows in the same binary. They now assert a counter
    **delta** of zero instead; the live assertion stays where it belongs, once,
    at the end of the gate.
  - **Capi-invariance, measured twice — and what did NOT stay identical.** Part
    A's A/B (`DSS_GATE_ONLY='controls:'` 105 cases, and `controls:regcontrol/` 8)
    was re-run on the final tree: gate stdout identical line for line (only the
    `filtered out` unit-test count and a `Compiling` line differ), and the
    bounded **plain** census is byte-identical between part A's tree and the
    final one — all 10 per-channel extracts, `run.json` and the 33 MB
    `props_census.json`. Against the **pre-RP2.1** tree the statement is
    narrower and is the one that matters: the five `capi_v0145` extracts are
    byte-identical, while the five `r4133` extracts legitimately gain one pair —
    `regcontrol.revthreshold` (`'-100'`/`'100'`, `2.00e+00`, 269 cells on that
    bounded run; 888 on the full population) — because unmasking that
    `SKIP_PROPS` row on r4133 is this sub-step's own deliberate policy change,
    not a normalization leak. Re-measured in the audit round; the correction is
    vendored in the README (§"The RP2.1 policy change a re-census now reports")
    and in TESTING.md's comparison procedure. **The commit body's third bullet
    ("plain census byte-identical") carries the same imprecision and is
    superseded by this paragraph.** The claims mode adds columns and files only
    in `claims` mode, and a plain run now deletes a previous claims run's three
    files by name rather than leave them to be misread.
  - **Goldens: measured, none moved.** `git status` after the full five-command
    gate shows only the sub-step's own harness/test/doc paths; no artifact under
    `tests/golden/` and no lock file appears. No lane_diff owed (zero engine
    change).
  - **Findings handed forward** (each in no plan list before this sub-step):
    1. **`generator.dynout` is an r4133 RENDERING bug, not an echo** —
       `SetDynOutput` stores `Get_Out_Idx`'s `FVarNames` index
       (`General/DynamicExp.pas:411-437`) while `GetDynOutputStr` decodes it as a
       flat (var, slot) index over `DynSlot = array[0..1]`, so var 5 `theta`
       prints `dpshaft`. Dynamics are unaffected and the port is right; declared
       to **RP2.2** so an RP2.3 echo row cannot mask it, and a candidate
       `investigations/to_opendss/` report.
    2. **`energymeter.peakcurrent` needs BOTH kinds of row** — two of its
       spellings are `ArrayForm`-claimed, the third (`'[ 400]'` vs
       `'((400, 400, 400))'`, 6 cells) is an `EchoDefault`
       (`Meters/EnergyMeter.pas:2637-2664` re-wraps the never-refreshed
       `:2209` default). RP2.3.
    3. **`Fault.GMatrix` (386 cells, `'(0 )'` vs `'()'`)** — surfaced by the
       disposition sweep while the row stays masked on both channels: upstream's
       `If Assigned(Gmatrix)` guard against the port's materialised zero matrix.
       Not in any table and not in the census today; handed to RP2.2's triage.
    4. **RP2.3/RP2.4 must return here**: `MULTI_LINK_ROWS` and the two zero
       `CLAIMED_` locks in the replay force a visit when the echo table or the
       floor starts claiming, and `the_echo_table_and_the_display_floor_claim_nothing_yet`
       is the test that will red.
    5. **Hygiene, unowned — and it bit once.** Corpus/gate runs leave `Export`
       artifacts inside the **vendored** corpus tree (this sub-step's runs left
       seven across `tests/corpus/electricdss-tst/Test/AutoTrans/`, untracked and
       not gitignored; all deleted by name). No corpus-hygiene test catches a
       deck writing into the vendored tree. In the same directory the first full
       `cargo test --workspace` of this sub-step hit a **transient** gate failure
       — `solvable_now:Test/AutoTrans/Auto3bus.dss`, r4133 channel, `export
       losses file=Auto3bus_HL_losses.txt` → `Error 303 … I/O error 103` — which
       did not reproduce: the AutoTrans slice re-ran 9/9 and the full suite came
       back 3 456 passed / 0 failed in both lanes. It cannot come from this
       diff (nothing here executes outside `DSS_PROPS_CENSUS=claims`), and the
       suspicion is exactly the artifact-in-the-vendored-tree hygiene above.
       Recorded so a second sighting is a pattern, not a surprise.
    6. **RP2.2 must read the `swtcontrol` getter for the WHOLE pair.**
       `swtcontrol.normal`/`state` are on RP2.2's S6 list *and* carry an RP2.1
       `ArrayForm` row that claims one cell of 59 each (the single one-token
       spelling); the per-phase renders are still RP2.2's. Same shape, fully
       claimed, on `invcontrol.monbus`/`monbusesvbase`. Added by the audit
       settlement below (finding 8) — a row on `RP22_S6` is not proof RP2.1 left
       the pair alone.
  - **Audit settlement (2026-08-23, one commit on top of `e96d9248`).** Two
    audits (code, tests) raised **9** findings. **Raw count 9** = 5 (audit-code,
    all minor) + 4 (audit-tests: 1 major + 3 minor), i.e. 1 major + 8 minor.
    **Deduped count 9** — the two auditors overlap on exactly one substance, the
    `swtcontrol` `ArrayForm` rows on RP2.2's S6 list, which audit-code raised
    under "needs manual review" (not as a finding) and audit-tests as its finding
    8; settled **once**, as item 8 below. **Settled 9 = 9 fixed + 0
    recorded-without-fix + 0 refuted** — nothing was dropped and nothing needed
    refuting; every one of the nine was real.
    Every fix is test-only, harness-only or documentation; **zero engine byte,
    zero golden/lock/ledger/manifest byte, no tolerance or floor moved**
    (`R4133_DISPLAY_FLOOR` stays `None`). **No `NormRule` predicate changed at
    all** — the answer to the major finding is pins, not edits, so no example row
    moved owner and every replay count lock holds unmoved. The only behavior
    changes anywhere are three, and each *narrows* or *records*: the channel gate
    on `claim_value`/`for_value` (narrows what the measurement layer claims), the
    census's ingest aggregation (records a conflict instead of aborting) and the
    `#`-comment filter (narrows to one file).
    1. *(minor, code)* **The claims census applied the r4133 value policy to
       capi rows.** `Row::annotate` ignored the row's own channel and
       `claim_value`/`for_value` took none, so the "capi normalizes nothing"
       zero was a property of today's data, not the contract — and RP2.4's floor
       would have been consulted on capi scalars once the slot fills. **Fixed:**
       the channel is a parameter of both, `claim_value` refuses every channel
       but `R4133`, and the **ledger** link stays live on capi because it is a
       real capi mechanism. Pinned by `props_norm::the_capi_channel_claims_nothing`
       and `props_census::the_capi_channel_reaches_no_r4133_link`; re-measured
       live (`DSS_PROPS_CENSUS=claims`, `modes:makeposseq/`): capi = 0 normalized
       / **7 `ledger-hit`s**, so the gate is a channel gate and not a mute.
       `R4133_DISPLAY_FLOOR`'s doc, which claimed it was reachable only from
       `ArrayForm`, now names both callers of `numbers_match` and why both are
       r4133-only.
    2. *(minor, code)* **`ChannelExtracts::ingest` could abort a claims run on
       legitimate data.** Its `assert_eq!` declared the chain a pure function of
       `(class, prop, rust, oracle)`; the ledger link is per **(case, channel)**
       by design, so one spelling named by an entry in case A and by none in
       case B legitimately answers two ways — and the population is one deck away
       from it (`gictransformer.r2 | '0.09522' | '0.12696'` already occurs in two
       cases with an entry each). A mode whose contract is that it **asserts
       nothing** would have died after a 56 s / 1e6-row walk. **Fixed:** the two
       verdicts aggregate to the **weakest** (`Disposition`'s variant order runs
       claimed → `UNCLAIMED`, so `max` can only move a spelling toward the work
       list), the conflict is counted in `claims_summary.json`'s
       `mixed_disposition_spellings` and printed as a banner NOTE — the
       `heterogeneous_shape_classes` precedent. Pinned by
       `a_spelling_dispositioned_two_ways_aggregates_to_the_weakest` (both
       arrival orders).
    3. *(minor, code)* **The vendored README did not record RP2.1's own
       population change.** Unmasking `RegControl.RevThreshold` on r4133 is a
       policy change, so a plain re-census on this tree reports a numeric pair
       the frozen extracts cannot have. **Fixed:** the README gains
       §"The RP2.1 policy change a re-census now reports" (the pair, its 888
       cells, the `RegControl.pas:1448` citation, the RP2.3 obligation), the
       RP0.2 corrections section points at it so a reader does not stop at
       "two", and TESTING.md's "Comparing against the vendored files" procedure
       now names all **three** expected r4133-side differences. Re-measured
       (`DSS_PROPS_CENSUS=1`, `controls:regcontrol/`): r4133 gains
       `regcontrol.revthreshold | '-100' | '100' | 2.00e+00 | 269`, capi reports
       no `regcontrol` divergence at all.
    4. *(minor, code)* **STATUS overstated the plain-census A/B.** "Byte-identical
       — all 10 per-channel extracts" is true final-tree-vs-part-A-tree and false
       against the pre-RP2.1 tree, where the five r4133 extracts move by exactly
       finding 3's pair. **Fixed:** the bullet now states both baselines
       separately and says which five files move and why; it also records that
       the commit body of `e96d9248` carries the same imprecision and is
       superseded (history is not rewritten — the audited range stays intact).
    5. *(minor, code)* **The replay's evidence base is one spelling behind the
       live population, with no guard.** `autotrans.conn | 'series' | 'Series'`
       is claimed live and vendored nowhere: its pair has a frozen `bins.tsv`
       row, which is exactly what bars it from the supplement. **Fixed:** it is
       now a term — `LIVE_ONLY_SPELLINGS` + `CLAIMED_TOTAL_LIVE = 749`, with
       `the_live_only_spellings_are_claimed_and_reconcile_the_two_accountings`
       asserting the arithmetic, that the shipped `CaseFold` row still claims the
       spelling, that its pair really is in `bins.tsv`, and that no vendored row
       carries it. Recorded in the README's claims section too (443 = 442 + 1).
    6. **(MAJOR, tests) The typed rules' accepted sets were not pinned.** The
       per-kind tests were sample-based, so a widening no sample touches was
       invisible: the auditor's probes showed `fold_bool` widened to `{on, off}`
       and a sign-stripping `CaseFold` both passing the entire suite — and a
       sign-blind `CaseFold` would fold `regcontrol.revthreshold`'s `'-100'` vs
       `'100'`, **888 real divergent cells**, into silence. **Fixed by pinning
       the boundaries, never by narrowing a rule** (no example row moved owner):
       `boolfold_accepts_a_closed_set_of_spellings` walks an exhaustive universe
       — the six accepted tokens under case/trim, plus 33 near-misses
       (`on/off/1/0/t/f/enabled/disabled/…`) asserted `None` — and re-checks all
       eleven census spellings; `casefold_never_drops_a_character` pins the
       predicate itself (accepts case + outer blanks; refuses a dropped sign,
       digit, leading zero, inner blank, separator, node reference, class
       prefix); `arrayform_separators_are_a_closed_set` pins that only
       `[](),`+whitespace separate. **Verified by re-running the auditor's own
       three surviving mutations in-tree** (`{on}` added to `fold_bool`,
       `trim_start_matches('-')` on `CaseFold`, the liveness assert neutered):
       each reddened exactly its new pin and nothing else — 3 failed / 78 passed
       — and the file was reverted to a byte-identical SHA-256 afterwards.
    7. *(minor, tests)* **`assert_norm_rows_are_live` had no canary.** Its
       counters are private process-global statics, so nothing could make the
       guard fire — and at RP4.1 it becomes the sole live anti-rot guard for all
       157 rows. **Fixed:** the rule moved into `check_rows_are_live(table,
       visits, hits)` and the public helper is a two-line adapter over it, so
       both directions are pinned offline —
       `the_liveness_guard_is_silent_when_dormant_or_live` (dormant, fully live,
       mixed) and `the_liveness_guard_fires_on_a_stale_row` (`should_panic`,
       matching the message that names the row). *Recorded, not fixed:* the
       precedent it cites, `lane::assert_reround_cells_are_live`, has the same
       weakness; it is GOLDEN_REBASE territory and is left alone here.
    8. *(minor, tests; the one both auditors raised)* **Two `ArrayForm` rows sit
       on pairs RP2.2 must still triage.** `swtcontrol.normal`/`state` fold their
       single one-token spelling (`'closed'` vs `'[closed, ]'`, 1 cell of 59
       each) while their 58/31/27-cell per-phase renders stay unclaimed, and both
       pairs are on RP2.2's S6 list. **Kept — narrowing would be wrong**: the
       fold is token-for-token value-preserving and the plan itself assigns the
       21 bin-4 pairs to RP2.1, while bare-vs-delimited is a *named* bin-4
       mechanism (`invcontrol.monbus` `'[A.1, A.2, A.3]'` vs `'A.1 A.2 A.3'`).
       **Fixed as disclosure + pins:** the `ArrayForm` doc now states the
       scalar↔one-element case with its census spellings, the module doc gains
       §"Four rows this table DOES hold sit on RP2.2's S6 list" (naming
       `invcontrol.monbus`/`monbusesvbase` as fully claimed and the two
       `swtcontrol` pairs as claimed-in-part, with the `relay.normal`/`state`
       contrast), `RP22_S6`'s doc says a row on that list is not proof RP2.1 left
       the pair alone, and `arrayform_folds_delimiters_not_contents` pins both
       directions on both `swtcontrol` pairs plus the `relay` twin. *Handed to
       RP2.2:* it must read the `swtcontrol` getter for the **whole** pair, the
       two folded cells included.
    9. *(minor, tests)* **The evidence lock's `#`-line filter had been widened to
       the frozen extracts.** Admitting the supplement's provenance header made a
       `#` line inserted into any frozen file invisible to the row-count lock —
       the only assertion that would have seen it. **Fixed:** both readers
       (`props_r4133_evidence_lock.rs`, `props_r4133_replay.rs`) skip comments
       for `examples_supplement.txt` **only**, and
       `derived_extracts_keep_their_row_counts` additionally asserts no frozen
       row-shaped file contains one.

    **Found while settling, not by either audit — one wrong Pascal citation, in
    six places.** RP2.1 cited `RegControl.pas:1437` for
    `PropertyValue[23] := '100'` (RevThreshold's frozen default) and `:1441` for
    `remoteptratio`'s `PropertyValue[27] := '60'`. Both are off by 11: the real
    lines are **`:1448`** and **`:1452`** (re-read in
    `.inputs/electricdss-code-r4133-trunk`; `:1437` is `PropertyValue[12] :=
    '120'`). The plan's own `InitPropertyValues` span `:1423-1459` and its
    `remoteptratio` `:1452` were right — RP2.1's derived copies were not.
    Corrected in `harness/mod.rs`, `props_r4133_replay.rs`,
    `tests/TOLERANCE_NOTES.md`, the vendored `README.md` and
    `examples_supplement.txt`'s provenance header (a comment line — no data row,
    no count, and the header markers the evidence lock checks are untouched), and
    in this record above. `RegControl.pas:820-827` (the `GetPropertyValue`
    override of index 28) was re-verified and is correct.

- **RP2.2** (2026-08-23) — enum synonyms and the S6 dossier. **Zero engine
  change**: `tests/harness/props_norm.rs`, `tests/harness/mod.rs`,
  `crates/dss-core/tests/props_r4133_replay.rs`, the vendored README, the plan
  and this file. No golden byte, no `golden.lock.json`, no
  `population.lock.json`, no `ledger.json` entry, no manifest byte, and the
  r4133 props live masks untouched (§1.1(e) staging). `R4133_DISPLAY_FLOOR`
  stays `None`; no tolerance moved; no test was deleted, `#[ignore]`d or
  loosened. **Kill criterion: not fired** — every pair fits exactly one of the
  three outcomes.
  - **The dossier, condensed** — pair → verdict → r4133 site → cells (full /
    in scope). The routing table itself lives in code with the same citations
    (`props_r4133_replay.rs::RP22_ROUTING`), so a stale verdict fails a test.

    | pair | verdict | r4133 site | cells |
    |---|---|---|---|
    | `vsource.scantype` | **EnumSynonym** | `Vsource.pas:1323-1349` (no arm 17/18) + `:1300-1301` + `:615-616` + `:355`/`:378-384` | 2 042 / 1 697 |
    | `vsource.sequence` | **EnumSynonym** | same, `:385-391` | 2 042 / 1 697 |
    | `isource.scantype` | **EnumSynonym** | no getter override at all + `:626` + `:396` + `:239`/`:257-263` | 137 / 136 |
    | `isource.sequence` | **EnumSynonym** | same, `:627`/`:397`/`:264-270` | 137 / 136 |
    | `invcontrol.voltage_curvex_ref` | **EnumSynonym** (re-typed from `CaseFold`) | `InvControl.pas:3244-3249` live getter + `:837` | 257 / 147 (3 off-bin) |
    | `swtcontrol.action` | echo → RP2.3 `EchoParse` | `SwtControl.pas:573-620` (no arm 3) + `:192-193` + `:652` + `:417` | 34 / 16 |
    | `monitor.mode` | echo → RP2.3 `EchoParse` | `Monitor.pas` no override + `:359` + `:1843` | 4 / 4 |
    | `storagecontroller.modedischarge` | echo → RP2.3 `LiveSemanticsDiffer` + pin | `StorageController.pas:1200-1214` vs `:2322-2333` | 1 / 1 |
    | `line.spacing` | echo → RP2.3 `EchoParse` | `Line.pas:1347-1429` (no arm 21) + `:1511` | 1 / 0 |
    | `isource.bus2` | echo → RP2.3 `EchoDefault` | `Isource.pas:631` (+ report 14) | 136 / 135 |
    | `isource.yearly` | echo → RP2.3 `EchoDefault` | `Isource.pas:628` + `:286` (object aliased, string not) | 122 / 122 |
    | `load.yearly` | echo → RP2.3 `LiveSemanticsDiffer`, ONE row for the pair | `Load.pas:2346` (live raw string) + `:657` + `:807` | 32 548 / 30 546 |
    | `reactor.bus2` | echo → RP2.3 `EchoParse` (derived snapshot) | `Reactor.pas:1087-1105` (no arm 2) + `:386` + `:419-420` + `:341-358` | 47 / 43 |
    | `invcontrol.monvoltagecalc` | echo → RP2.3 | `InvControl.pas:505`, no arm 25, not in `:2806-2839` | 254 / 144 |
    | `invcontrol.pvsystemlist` | echo → RP2.3 `EchoDefault` | `:512`, no arm 32 | 257 / 147 |
    | `invcontrol.vsetpoint` | echo → RP2.3 `EchoDefault` | `:513`, no arm 33 | 249 / 147 |
    | `expcontrol.derlist` | echo → RP2.3 `LiveSemanticsDiffer` + pin | `ExpControl.pas:696` + `:702-715` vs `:227-234`/`:247-252` | 11 / 11 |
    | `capcontrol.type` | echo → RP2.3 `EchoParse` | no `GetPropertyValue` in the unit + `:178` + `:304-311` | 30 off-bin / 6 |
    | `fault.bus2` | echo → RP2.3 `EchoParse` | `Fault.pas:695-718` (no arm 2) + `:297` + `:672` | 1 off-bin / 0 |
    | `generator.dynout` | echo → RP2.3 `LiveSemanticsDiffer` + pin (**not** RP3.5) | `PCElement.pas:197-209`/`:228-236`, `DynamicExp.pas:411-437`/`:441-465`, `Arraydef.pas:39` | 273 / 2 misrendered |
    | `line.units` | **RP3.5** | `Line.pas:1404` live + `:1627`/`:1721-1726`/`:1791-1796`/`:2326-2331` | 3 / 0 |
    | `line.linecode` | **RP3.6** | `Line.pas:1357` live + `:694-700` + `:626-627` | 5 / **5** |
    | `swtcontrol.normal` / `.state` | **RP3.7 (a)** | `SwtControl.pas:589-599`/`:600-610` + `:37-38`/`:299-305`/`:433-480`/`:532-549` | 59 / 40 each |
    | `relay.normal` / `.state` | **RP3.7 (b)** | `Relay.pas:1407-1428` (loops `ControlledElement.NPhases`) | 1 / 0 each |
    | `invcontrol.monbus` / `.monbusesvbase` | already claimed, RP2.1 `ArrayForm` | `InvControl.pas:3226-3285` (no arm 26/27) + `:2806-2839` | 15 / 7 each |
    | `Fault.GMatrix` | triage only — stays value-skipped on BOTH channels | `Fault.pas:695-718` `If Assigned(Gmatrix)` + `:76-77` | 386 (masked) |
  - **The `EnumSynonym` rows: two maps, not one — and that is a correction.**
    The dossier's first draft equated `'Neg'` with our `'Negative'` on *both*
    properties. It is wrong on `scantype`: the two DSS registries are different
    lists — `'Scan Type'` is `['None', 'Zero', 'Positive']` and `'Sequence
    Type'` is `['Negative', 'Zero', 'Positive']`, both over `[-1, 0, 1]`
    (`obj/dss_enum/registry/solution.rs:22-38`) — so ordinal −1 renders `None`
    on one and `Negative` on the other. A shared map would have folded nothing
    on `scantype` and gone stale silently. Hence `SCAN_TYPE_SYNONYMS`
    (`Positive↔Pos`, `Zero↔Zero`) and `SEQUENCE_TYPE_SYNONYMS`
    (`Positive↔Pos`, `Negative↔Neg`), each closed at the spellings the census
    actually measured plus the frozen `InitPropertyValues` default, matched
    exact-token + case-insensitively, **fail-closed** on anything else.
  - **A second dossier correction, found by re-reading the source.** The draft
    recorded that `TVsource.MakeLike`/`TIsource.MakeLike` do not copy
    `ScanType`/`SequenceType`, which would have been the one desync path between
    r4133's echo store and its live enum. They **do** copy both —
    `Vsource.pas:527-528` and `Isource.pas:324-325`, right beside the
    `FPropertyValue[]` loop at `:566`/`:339`. So no desync path exists and the
    rows rest on a stronger argument than the draft claimed; the "r4133 omission"
    finding is **withdrawn**, and no upstream report was written for it.
  - **The three RP3.5+ sub-steps** (full specs in `R4133_PROPS_PLAN.md`
    §WP-RP3; tier `opus-high+`, the RP3.1–RP3.4 row). **All three blocked
    RP4.1** (plan §0: "RP4.1 starts only after every RP1–RP3 sub-step is landed,
    including any RP3.5+ sub-step RP2.2's triage opens"); **all three have since
    landed — RP3.5 on 2026-08-28, RP3.6 on 2026-08-29, RP3.7 on 2026-09-02** —
    and the fourth sub-step this WP opened, RP3.8, landed 2026-09-02 too, so
    what still blocked RP4.1 from this WP was RP3.9 alone — and **RP3.9
    landed 2026-09-02** as well (27 pairs, all `PRECISION_ROUNDTRIP`; see the
    §RP3.9 record in §1), so this WP no longer blocks the unmask.
    - **RP3.5 — line length units lost by the matrix-branch merge. SETTLED
      2026-08-28 (`FIX`, both lanes; audit settled 2026-08-29) — see the §RP3.5
      record below.**
      `exec/reduce.rs` wrote `l.length_units = len_units_saved` and then ran the
      `RMATRIX/XMATRIX/CMATRIX` side effects, which call `reset_length_units`
      (`elements/pd/line/accessors.rs:478-486` → `code.rs:24-28`); r4133 does the
      two in the opposite order (`Line.pas:1627` saves, `:1721-1726`/`:1791-1796`
      re-edit *after* the impedance edit) and `MakePosSequence` re-appends
      `Units=` for the same reason (`:1596`). Two further divergences in the same
      routine, both from **both** oracles: `reset_length_units` also cleared
      `user_length_units`, which r4133 (`:2330`) and dss_capi 0.14.5 (`:2084`)
      both keep under the identical "in case of CIM export" comment; and the
      sym-branch `Length=`/`Units=` re-apply was nested inside the impedance arm
      (upstream runs it unconditionally), whose partner-is-switch half also
      emitted the parser-rejected token `Switch=1`. Evidence: `line.units` 3
      cells, **0 in scope**, and — corrected by the sub-step's probe — all 3 on
      `modes:reduce/midi_reduce.dss`, **none** on `reduce_mergeparallel`, whose
      merged line renders `km` on all three engines. Cost the plan text did not
      anticipate: the fix reds the **live** `capi_v0145` `all_properties` compare
      on `midi_reduce.dss`, so a capi ledger entry + cause +
      `population.lock.json` rewrite landed with it.
    - **RP3.6 — `switch=yes` must not clear the linecode flag. SETTLED
      2026-08-29 (`FIX`, both lanes; audit settled the same day) in two parts —
      (a) the switch arm and
      (b) the `FLineCodeSpecified`/`CondCode` split plus the CIM units
      back-fill, plus the settlement's `SpacingSpecified` split and r4133's
      eighth flag-clear site; see the three §RP3.6 records above.** r4133's arm
      (`Line.pas:694-700`) writes r1/x1/r0/x0/c1/c0/len as fields, kills geometry
      and spacing and resets the units, but leaves `FLineCodeSpecified` TRUE; the
      port calls `kill_line_code_specified()`
      (`elements/pd/line/accessors.rs:488-511`, following 0.14.5's
      `KillLineCodeSpecified`). Consequence beyond the render: the flag picks a
      different `FUnitsConvert` formula on a later `units=` (`:626-627`), and the
      decks that carry the cells
      (`ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_2/Branches.dss:93,:95,:479` and
      `zone_3/Branches.dss:161,:165`) put `units=m` after `Switch=True`.
      `StoCtrl_Current_PeakShave/Line.DSS` has the same shape but is `kind:
      large`, so it is never property-compared and carries none of them.
      Evidence: `line.linecode` **5 cells, all 5 in scope** — the
      only RP3.5+ pair the unmask will actually compare, so **RP4.1 breaks on it
      if RP3.6 does not land first**. r4133 is the authority; "capi does it" is
      not evidence (CLAUDE.md).
    - **RP3.7 — per-phase switch and relay state. SETTLED 2026-09-02 (`FIX`,
      both lanes, all three parts — (a), (a2) and (b); audit settled the same
      day, which also landed Relay's NIL-element render and opened
      `ORPHANED_GAPS.md` §1.16) — see the §RP3.7 record and its audit-settlement
      paragraph above.** (a) r4133 keeps
      `FPresentState`/`FNormalState : pStateArray` per phase
      (`SwtControl.pas:37-38`, `:299-305`), settable phase-by-phase from a quoted
      list (`:453-480`), each phase driving its own conductor (`:532-549`), and
      renders one token per controlled-element phase (`:589-610`); the port holds
      one scalar applied to the whole terminal
      (`elements/control/swt_control/accessors.rs:126-163`, `:270-276`,
      `mod.rs:2`). Every r4133 render the census saw is homogeneous, so **no
      value differs today** — a deck writing `state=(open, closed, closed)` would
      diverge in Y. **(a2), added by the audit settlement and probe-confirmed:**
      a `normal=` write on a **locked** SwtControl is refused by the port
      (`accessors.rs:151-155`, following 0.14.5's `ConditionalReadOnly`) and
      **applied** by r4133 — `InterpretSwitchState` exits early only for property
      names starting `'a'`/`'s'` (`SwtControl.pas:416-417`, comment "Only allowed
      to change normal state if locked") and property 6 is `'Normal'` (`:128`),
      so arm 6 (`:201-204`) reaches `set_NormalStates` (`:556-561`), which has no
      lock guard. Live r4133 DLL probe: under `lock=yes`, `normal=open` →
      `[open, open, open, ]` while `state=`/`action=` move nothing. The port's own
      Relay implements the r4133 rule and documents it
      (`relay/accessors.rs:416-420`), so this is a port bug, not a decision; the
      §D12 record in `docs/upgrade/DIVERGENCES.md` claimed the opposite and is
      corrected. Zero corpus exposure (no deck writes `normal=` under lock), so
      it is a *fix*, not an exclusion — RP3.7 lands it in both lanes with a pin
      and it may not be deferred to `ORPHANED_GAPS.md`. (b) The mirror on Relay:
      r4133 renders over the live
      `ControlledElement.NPhases` (`Relay.pas:1407-1428`) while the port renders
      its own per-phase array, unresynced after `MakePosSequence`. Evidence:
      `swtcontrol.normal`/`state` 59/40 each, `relay.normal`/`state` 1/0 each.
      Interim treatment if the port change is deferred: exclusion + pin, and the
      gap recorded in `ORPHANED_GAPS.md`.
  - **`generator.dynout` — RP2.1's hand-off, closed here as RP2.3's, not as a
    sub-step.** The root cause was re-verified against the source rather than
    taken on trust: `TPCElement.SetDynOutput` stores `Get_Out_Idx`'s **variable**
    index (`PCElement.pas:231`; `DynamicExp.pas:411-437`) while
    `GetDynOutputStr` (`:205`) hands it to `Get_VarName` (`DynamicExp.pas:441-465`),
    which decodes it as a flat *(variable, DynSlot column)* index — `mylen =
    length(myProt)` with `DynSlot = array[0..1] of double`
    (`Shared/Arraydef.pas:39`). On the shipped deck (`varnames=[Speed Mass
    PShaft Pterm Damp theta]`, `DynOut=[Speed theta]` → `[0, 5]`),
    `Get_VarName(5)` → row 2, col 1 → `'d' + 'pshaft'`. The dynamics are
    unaffected (`generator.pas:2823-2849` indexes `DynamicEqVals` with the same
    variable index the port uses), so the port is right and the remaining work is
    **one cited `LiveSemanticsDiffer` row plus its expected-value pin**, not an
    investigation — which is exactly what plan §RP2.2's third outcome is *not*
    for. The `CELL_DISPOSITION` comment that declared it to RP2.2 now carries the
    verdict, and the row must be a **tagged** `LiveSemanticsDiffer`, never a
    silent `EchoDefault`, or the 2 misrendered cells are masked without a pin.
    Upstream report written:
    `investigations/to_opendss/41-dynout-readback-renders-wrong-variable.md`
    (gitignored, local-only; next free number — 01..40 with gaps).
  - **`Fault.GMatrix` — RP2.1's other hand-off, settled with no sub-step.**
    `TFaultObj.GetPropertyValue` index 6 emits `'('`, fills the lower triangle
    only `If Assigned(Gmatrix)` (`Fault.pas:703`) and closes with `')'`; an
    `r=`-specified fault never allocates `Gmatrix` — `Create` nils it (`:411`)
    and the only writers are `DoGmatrix` (`:196-209`, Edit arm 6 at `:286`) and
    `MakeLike` (`:364-367`). (The audit settlement replaced the first draft's
    citation of the field comment `:76-77`, which is documentation, not
    evidence.) So `'()'` and the port's `'(0 )'` denote the **same** state, "no G
    matrix specified" — the unset-array render family of
    `generator.dynout`/`autotrans.bhcurrent`, with no port behavior to change.
    The 386 cells therefore stay value-skipped on both channels under the
    existing `SKIP_PROPS`/`SKIP_PROPS_BOTH_CHANNELS` row, whose comment now
    carries the verdict; no unmask (§1.1(e)), no new table row, and RP4.1 owes no
    ledger entry for it. What it *did* owe — and now has — is the expected-value
    pin CLAUDE.md requires beside any deliberate exclusion, since
    `both_channel_rows_stay_skipped_everywhere` only asserts the skip is
    configured and its sibling `generator.dynout` gets a tagged exclusion **plus**
    a pin: `harness/mod.rs::skip_props_disposition_tests::
    fault_gmatrix_renders_a_materialised_zero_matrix_when_unset` pins `'(0 )'` at
    1 phase, `'(0 |0 0 |0 0 0 )'` at 3, and the non-zero specified render as the
    discriminator.
  - **Closing bin 3 meant closing its CELLS.** The replay's own declaration rule
    put four example rows in RP2.2's bucket that the plan's pair list does not
    name: three bin-2-labelled pairs carrying bin-3 cells, which the vendored
    `README.md` §"A pair's bin is a label" already enumerates. RP2.2 read all
    three getters — `capcontrol.type` (`'pf'`/`'volt'`, 30 cells; the class has
    **no** `GetPropertyValue` override, so `type` echoes the deck token) and
    `fault.bus2` (`'b2.0'` vs `'b2.0.0.0'`, 1 cell; the same derived-bus2
    snapshot as `reactor.bus2`, `Fault.pas:297`) went to RP2.3 beside their
    untouched `CaseFold` rows (the mixed-pair pattern the chain order exists
    for), while `invcontrol.voltage_curvex_ref` is a **live** getter
    (`InvControl.pas:3244-3249`, `0→'rated'`, `1→'avg'`, `2→'avgrated'`) against
    our registry's `['Rated','Avg','RAvg']`, so an exclusion would have been a
    lie and the whole pair was re-typed to an `EnumSynonym` row. The two
    predicates are **incomparable, not nested** — a claim this record and the
    map's own doc first got wrong and the audit settlement corrected: the map is
    *narrower* on case-only differences outside the three named ordinals, and
    *wider* by exactly the 3 `'RAvg'`/`'avgrated'` cells (all 3 in scope) that
    the `CaseFold` row refused and whose refusal this sub-step deleted from
    `casefold_folds_case_and_the_two_trailing_blanks`. Those 3 folds are the
    point of the re-typing and are value-preserving by the live-getter argument;
    what would not be acceptable is folding them silently, which is why the count
    is now named at the map, here, and in the vendored README (which had it right
    all along). The scope growth is recorded as data
    (`props_r4133_replay::RP22_BEYOND_THE_CLOSED_LIST`), not folded away.
  - **The table.** 157 → **161 rows** = `BoolFold` 77 + `CaseFold` **62** +
    `ArrayForm` 17 + `EnumSynonym` **5**. The `CaseFold` lock moved −1 for the
    re-typed pair. New pins, all in `props_norm.rs`:
    `enumsynonym_folds_the_shipped_scan_and_sequence_spellings` (every census
    spelling of all five rows accepted; the opposite ordinal, the reverse
    direction, the cross-registry `'Negative'`-on-`scantype` trap and seven near
    misses all refused) and `enumsynonym_maps_are_injective` (structural: one
    r4133 token may not name two of our values, no empty map, no `''` entry, and
    each row really carries the map the doc names). The bin↔rule pin gained one
    **counted** exception (`invcontrol.voltage_curvex_ref`, bin 2), and
    `the_pairs_routed_elsewhere_have_no_row` gained the four bin-3 pairs RP2.2
    routed away, so adding a synonym row for any of them reds a test.
    `harness/mod.rs`'s `FOLDABLE` fixture grew to one spelling per rule kind, so
    both channel tests now cover the whole table.
  - **The replay.** 3 454 example rows, **0 unaccounted**, claimed **755**
    (`BoolFold` 114 / `CaseFold` **440** / `ArrayForm` 192 / `EnumSynonym`
    **9**). Declared: RP2.2 **(0, 0, 0)** — the acceptance — RP2.3 450 rows/86
    pairs (was 295/72), RP2.4 2 100/70, RP3 7/4, **RP3.5+ 8 rows / 6 pairs / 5
    in scope**, out-of-scope 134/18. Two new structural guards: `declare` now
    **errors** (rather than declaring to RP2.2) for any pair on the closed list
    or any bin-3 cell that neither the chain claims nor `RP22_ROUTING` names, and
    `rp22_settled_every_pair_it_was_handed` proves the input list is partitioned
    into "claimed by the table" (checked per example row through the shipped
    predicate) and "routed with a citation", with every routing row live, citing
    a `.pas:` line, and owning one of the two admissible outcomes. **The split,
    stated over the whole 27-pair input** (the audit settlement's correction —
    this bullet and the commit subject `ab2bf041` quoted only the closed list's
    own 6/18 without saying so): `RP22_ROUTING` holds **20** rows (14 → RP2.3,
    6 → RP3.5+) and **7** pairs are claimed outright, i.e. 24 closed-list pairs
    = 18 routed + 6 claimed, plus the 3 `RP22_BEYOND_THE_CLOSED_LIST` pairs
    = 2 routed + 1 claimed.
  - **The claims census** (`DSS_PROPS_CENSUS=claims`, full population,
    2026-08-23): 439 cases × 2 channels, **1 060 165 rows, 57.3 s**, error counts
    at the recorded baselines (**5** r4133 / **22** capi — complete, not short).
    r4133 claimed **514 471 / 492 376 in scope** (was 510 106 / 488 703):
    `BoolFold` 294 519 / 280 915, `CaseFold` **92 976 / 89 231**, `ArrayForm`
    122 357 / 118 413, **`EnumSynonym` 4 619 / 3 817** (9 spellings, 5 pairs);
    echo 0, floor 0, ledger 0 — all three still load-bearing zeros. **UNCLAIMED
    545 568 / 521 841** over 184 pairs (was 549 933 / 525 514 / 189), with the
    frozen-bin-3 row down from 8 pairs / 4 404 / 3 691 to **4 / 42 / 21**. The
    +4 cells over the frozen `bins.tsv` totals on the four source pairs are the
    same population drift RP2.1 reconciled (+2 per `vsource` pair). Live
    spellings **756** = the replay's 755 + the one `LIVE_ONLY_SPELLINGS` term,
    so `CLAIMED_TOTAL_LIVE` is a measurement, not a guess. Capi: **0** on all
    six r4133 dispositions, 11 `ledger-hit`s — unchanged and by contract.
    Numbers vendored in `tests/corpus/props_r4133/README.md` §"What RP2.2 moved".
  - **Doc corrections this sub-step measured** (frozen extracts untouched):
    `monitor.mode`'s r4133 spelling is a stored **RPN source string**
    (`mode=(1 16 +)` in `Test/Dynamic_Kundur.dss:55-56` and
    `Examples/Dynamic_Expressions/Dynamic_KundurDynExp.dss:66-67`), not a
    "decomposition render" — corrected in the plan's §1.1 bin-3 row and in the
    vendored README's new section; `triage.md`'s §S6 wording is superseded by
    the same README section and stays as the historical triage.
  - **Two upstream-report candidates recorded, not written** (neither is needed
    by this sub-step's exclusions): `GetModeString` has no `MODESCHEDULE` arm
    (`StorageController.pas:1200-1214` vs `:2322-2333`), and
    `TExpControlObj.GetPropertyValue` answers the bare PVSystem list for
    `DERList` (`ExpControl.pas:696` + `:702-715` against the two deliberately
    distinct lists built at `:227-234`/`:247-252`).
  - **Gate: all five green on the frozen tree** — `fmt --check`, both clippy
    lanes, both `cargo test --workspace` lanes, **7 274 tests passed / 0
    failed**, corpus gate included.
  - **Goldens: measured, none moved.** `git status` after the five-command gate
    shows only this sub-step's harness/test/doc paths — no artifact under
    `tests/golden/`, no lock file, no manifest, no ledger. No lane_diff owed
    (zero engine change).
  - **Operational note, so it is not miscounted as a second sighting of RP2.1's
    finding 5.** An earlier gate attempt reported 9 corpus-gate failures, all
    `Error Attempting to open file` / `#303` on `ckt7`,
    `StoCtrl_Current_PeakShave` and `ckt24`. The cause was **local and known**:
    a first gate run's script survived its cancellation and its
    `cargo test --workspace` overlapped a second one on the same working tree,
    so two runs raced on the same deck output files. Killed both, `git clean -fd
    -- tests/corpus`, re-ran once — green. That is a *different* cause from
    RP2.1's unexplained transient, so RP2.1's "a second sighting is a pattern"
    counter does **not** advance. What it does confirm is the underlying
    hygiene gap RP2.1 recorded: corpus runs write `Export`/monitor artifacts
    into the **vendored** tree (this sub-step's runs left ~55 across nine
    directories, all untracked, all removed by name), and nothing fails when
    they appear.
  - **Audit settlement (2026-08-23, one follow-up commit — still zero engine
    change).** The two audits raised **8** raw findings (audit-code 1 major + 4
    minor, audit-tests 1 major + 2 minor). Two pairs are the same finding seen
    twice — the "the re-typed `voltage_curvex_ref` map is stricter" wording and
    the stale `EnumSynonym` row counts were each raised by both auditors — so
    the deduped set is **6** (2 major, 4 minor): **5 fixed, 1 recorded, 0
    refuted** (5 + 1 + 0 = 6 ✔).
    - **RECORDED, not fixed — `swtcontrol.normal` under `Lock`** (major,
      audit-code). The dossier routed `swtcontrol.normal`/`state` to RP3.7 on
      the per-phase question alone and missed a second, *live* divergence in the
      same Edit arm the brief required reading: the port refuses a `normal=`
      write while `Locked`, r4133 applies it. Verified against the source
      (`SwtControl.pas:416-417` guards only `'a'`/`'s'` property names; property
      6 is `'Normal'`, `:128`; `set_NormalStates`, `:556-561`, has no lock
      guard) **and** by a live probe on the vendored r4133 DLL (under
      `lock=yes`: `normal=open` → `[open, open, open, ]`; `state=`/`action=`
      move nothing). The fix is an engine change, which RP2.2's scope forbids,
      so it is recorded — `R4133_PROPS_PLAN.md` §RP3.7 **(a2)** with its
      acceptance, the `RP22_ROUTING` row comment, this record, and a correction
      to `docs/upgrade/DIVERGENCES.md` §D12, whose "matching 0.14.5/r4133"
      claim this refutes. Zero corpus exposure today.
    - **FIXED — the third `EnumSynonym` map shipped with no closed-set pin**
      (major, audit-tests; the RP2.1 major finding's shape, one level down).
      `SCAN_TYPE_SYNONYMS`/`SEQUENCE_TYPE_SYNONYMS` were pinned literally but
      `VOLTAGE_CURVEX_REF_SYNONYMS` only by identity, so an entry folding
      nothing in today's population (the auditor's mutation `("Rated","ravg")` —
      r4133 *parses* `'ravg'` to ordinal 2, `InvControl.pas:837`, against our
      ordinal-0 `'Rated'`) left the whole suite green. `enumsynonym_maps_are_
      injective` now pins all three maps literally.
    - **FIXED — injectivity was checked in one direction only** (minor,
      audit-code). The mirror collision (one of *our* spellings mapping to two
      r4133 tokens) is equally value-destroying and is now asserted in the same
      loop, with the kind's doc restated to promise both directions.
    - **FIXED — the "stricter" claim on the re-typed row** (minor, both
      auditors). The new map is *incomparable* with the `CaseFold` row, not
      narrower: it drops the open-ended case claim but adds the 3 `'RAvg'`/
      `'avgrated'` cells whose refusal this sub-step deleted from
      `casefold_folds_case_and_the_two_trailing_blanks`. Corrected at the map,
      in the test's own comment and in the bullet above; the vendored README had
      it right and is unchanged.
    - **FIXED — the `Fault.GMatrix` triage retired its obligation without a
      pin** (minor, audit-code), plus its `:76-77` citation (a field comment,
      not evidence). Both corrected above and at the `SKIP_PROPS` row; the new
      pin is `fault_gmatrix_renders_a_materialised_zero_matrix_when_unset`.
    - **FIXED — stale counts around the late-added fifth row** (minor, both
      auditors): `props_norm.rs`'s `EnumSynonym` doc ("landed four"), the new
      test's heading ("the four shipped rows"), the replay's
      `CLAIMED_ENUM_SYNONYM` failure message ("four bin-3 synonym rows"),
      `rp22_settled_every_pair_it_was_handed`'s doc ("four … and two", against
      its own seven-pair assertion) and this record's 6/18 routing split (true
      only of the closed list; the whole input is 7 claimed / 20 routed).
    - Gate re-run in full on the settled tree; no golden, lock, ledger, manifest
      or product-crate byte moved, and no test was deleted, `#[ignore]`d or
      loosened. Test count **7 274 → 7 318** (3 637 → **3 659** per lane, ×2
      lanes): the one new pin lands in each of the 22 test binaries that include
      the harness.

- **RP2.3** (2026-08-23) — the echo-exclusion table and its pins. **Zero engine
  change**: `tests/harness/*`, `crates/dss-core/tests/*`, the vendored
  `props_r4133/README.md` and docs. No golden byte, no `golden.lock.json`, no
  `population.lock.json`, no `ledger.json` entry (§1.1(e) staging), the r4133
  props live masks untouched, and `R4133_DISPLAY_FLOOR` still `None` (RP2.4's
  slot). Run as parts A (dossier) / B1 (tables) / B2 (pins, census, docs, the
  one commit).
  - **THE KILL CRITERION FIRED — six rows, and the user ruled both halves.**
    Part A read all 86 pairs of the declared `Owner::Rp23` bucket against the
    r4133 source and found six that cannot be cited to an echo site. Reported
    per the plan's stop-and-consult gate; the user chose (AskUserQuestion,
    2026-08-23):
    - **R1, the five `SilentReadOnly` surfaces** — `indmach012.pf` and
      `storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`/`kwactual`, **1 064
      cells / 772 in scope**. r4133 renders each one LIVE
      (`Version8/Source/PCElements/IndMach012.pas:1790`;
      `Controls/StorageController.pas:991-994` → `GetkWhTotal`/`GetkWTotal`/
      `GetkWhActual`/`GetkWActual`, declared `:136-139`); the port answers `''`
      only because dss_capi 0.14.5 flags them `[SilentReadOnly, ReadByFunction]`
      and the port reproduces that at one class-agnostic gate
      (`obj/props/class_props/value.rs:21-27`). Under the standing 2026-08-02
      policy the 0.14.5 convention yields, so the fix is an ENGINE change — and
      RP2.3 may not touch a product crate. They therefore take **no echo row**
      and are re-routed loudly: `props_r4133_replay::RP38_ROUTING` +
      `DECLARED_RP38 = (181, 5, 181)`, with a new plan **§RP3.8** that **blocks
      RP4.1** (§0's list read RP3.5/3.6/3.7 **+ 3.8**; RP3.5 landed 2026-08-28,
      leaving 3.6/3.7 + 3.8). **Landed 2026-09-02** — the engine renders all
      five live, `RP38_ROUTING` is now `RP38_SUPERSEDED` and `DECLARED_RP38` is
      `(0, 0, 0)`; see the §RP3.8 record in §1.
    - **R2, `generator.d`** — the plan, the vendored README §RP1.1 and
      `BIN7_ECHO_SUPPLEMENT`'s comment all called it an echo "from a frozen
      default whose field says 1.0". Wrong mechanism: the field that is 1.0 is
      `GenVars.D`; the property's field is `GenVars.Dpu`, which `Edit` writes
      (`generator.pas:669`) and `InitPropertyValues` snapshots (`:2585`), and
      which `Create` **never** touches (it sets `D := 1.0`, `:969`). So r4133's
      `'0'` is its own live value, and `InitStateVars` then computes
      `D := Dpu*kVArating*1000/w0 = 0` (`:2710`) — every generator that does not
      set `D=` runs dynamics **undamped**, against the property's documented
      default (`:467`). An upstream initialisation bug; 0.14.5 fixed it
      (`Dpu := 1.0`, `src/PCElements/Generator.pas:1006`) and the port follows.
      Landed as `LiveSemanticsDiffer` + pin + upstream report
      `investigations/to_opendss/42-generator-dpu-never-initialized.md`
      (gitignored, not in the commit); the three wrong texts are corrected.
  - **The table: 81 rows** (86 − the 5 re-routes), each with an r4133
    `unit:line` citation, a measured `cells` column read back against
    `bins.tsv`/the README, and a typed witness. Split **50** `EchoDefault` /
    **7** `EchoParse` / **14** `EmptyCollectionRender` / **10**
    `LiveSemanticsDiffer`.
    `EmptyCollectionRender` is the **one** new category the ruling sanctioned:
    14 pairs where r4133's getter arm is LIVE and merely renders the other
    empty-collection convention (`'[]'` from a loop over zero points, `'()'`
    from a paren wrapper, `''` from an early exit) — all three original tags
    would have been a lie there, and the kill criterion exists to stop exactly
    that. The count is above the plan's ~50–70 estimate because the bucket the
    RP2.1 replay had already declared by name holds 86 pairs (bin 5's 43 + bin
    7's 12 + bin 1's 9 + six bin-2-labelled pairs whose `''`-echo cells the chain
    routes here + bin 3's 3 + bin 4's 2 + the supplement's 11) — measured scope,
    not creep.
  - **The seam.** `PropsPolicy::echo_excluded` sits in `compare_prop_lists`
    **after** the normalization seam and immediately before
    `assert_value_matches_tol`; `continue` drops the VALUE assert only — name
    and index order are still walked, exactly `SKIP_PROPS`' shape. r4133 only:
    the capi arm is `false` for everything and is pinned
    (`the_capi_channel_never_excludes`, plus the real-comparator four-corner
    test `an_echo_row_drops_only_its_own_value_only_on_r4133`). The order is the
    per-cell **attribution**: **20 pairs hold both** a normalization row and an
    echo row, and 135 of their example rows are claimed by the typed rule first
    (`MULTI_LINK_ROWS`). It is **not** a per-cell mask — the exclusion is asked
    per pair, so it covers a mixed pair's refused cells too (the audit
    settlement's recorded finding, below).
  - **Nine typed rows landed FIRST** (part A finding F4, ruling R3):
    `load.yearly`,
    `reactor.bus2`, `invcontrol.monvoltagecalc` (`CaseFold`) and `line.wires`,
    `load.zipv`, `generator.userdata`, `storage.dynadata`,
    `storagecontroller.seasontargets`/`seasontargetslow` (`ArrayForm`) — **6 446
    cells / 6 370 in scope**, which the census then attributes to their rule
    (this record first said they "stay compared"; they do not — see the
    settlement). The tenth measured candidate,
    `swtcontrol.action`, deliberately took **no** row: one foldable spelling, 6
    cells, **0 in scope**, and the row would have loosened a live RP2.2 pin
    (`the_pairs_routed_elsewhere_have_no_row`) for zero live coverage.
  - **Witnesses — 20 expected-value pins** (**29 covering 63 rows** after the
    settlement below), new file
    `crates/dss-core/tests/props_r4133_pins.rs`, covering the **32** rows whose
    value capi does not (or must not only) hold. **21 are pin-only**: the seven
    Recloser/Relay pairs (capi skips those elements whole), ten
    RegControl/Line/Transformer/AutoTrans pairs dropped by
    `PROPS_015X`/`SKIP_PROPS` (five RegControl props, `line.conductors`, both
    `transformer.bh*`, both `autotrans.bh*` — probe-confirmed: the pinned oracle
    reports 35 RegControl property names, the port 39), `windgen.dynout` (no
    capi-gating case holds a WindGen), and the three whose echo cells all sit on
    capi-only cases (`fault.bus2`, `line.spacing`, `swtcontrol.action`). The
    other **11 have a capi witness *and* a pin**: every `LiveSemanticsDiffer` row
    (capi is a numeric oracle, so it cannot carry "ours is the right value") plus
    `energymeter.peakcurrent`, which the plan names. Each compiles the deck
    the census flagged, asserts the port's live `?` render literally and adds a
    discriminating second reading (a deck or an edit where the same getter
    answers otherwise). Own binary on purpose — in `harness/` each pin would
    re-solve its deck in all 22 harness binaries.
    `every_echo_row_pin_is_a_test_that_exists` reads the names back both ways;
    a `DeckDirGuard` sweeps the `Export`/`show eventlog` artifacts two of the
    decks write into the vendored tree (measured: `git status` clean after).
  - **Liveness, both ways.** Offline, in the replay:
    `every_echo_row_claims_at_least_one_example_row` (all 81 exclude ≥1 example
    row) and `every_echo_row_matches_its_cited_evidence` (each `cells` column
    read back against the file it cites). Live: `ECHO_VISITS`/`ECHO_HITS` per row +
    `assert_echo_rows_are_live()` in the gate epilogue, **silent-when-dormant**
    (part A finding F5) with a closed two-row exemption list,
    `ECHO_ROWS_WITH_NO_IN_SCOPE_CELL` = `fault.bus2`, `line.spacing` — both
    measured at 0 in-scope cells, and `fault.bus2` is the one that makes the
    exemption necessary rather than tidy (its pair *is* compared on r4133, so
    its row will be visited and can never hit).
  - **Locks moved, each delta measured** (offline replay + an independent
    Python replication that first reproduced the pre-change baseline exactly):
    `NORM_ROWS` 161→**170**, `CLAIMED_CASE_FOLD` 440→**528**,
    `CLAIMED_ARRAY_FORM` 192→**203**, new `CLAIMED_NORMALIZATION` **854**,
    `CLAIMED_ECHO` 0→**170**, `CLAIMED_TOTAL` **1 024**, `MULTI_LINK_ROWS`
    0→**135** (20 pairs, split per pair), `CLAIMED_TOTAL_LIVE` 756→**855**,
    `ECHO_ROWS` 0→**81** + four per-category locks, `DECLARED_RP23`
    (450, 86, 449)→**(0, 0, 0)** and new `DECLARED_RP38` **(181, 5, 181)**.
    450 = 99 norm + 170 echo + 181 RP3.8, exactly; the RP2.4/RP3/RP3.5/
    OutOfScope triples did not move. (The settlement below re-splits the same 450
    as 99 + **169** echo + 181 + **1** carved out to RP2.4, and moves
    `DECLARED_RP24` with it.)
    `the_echo_table_and_the_display_floor_claim_nothing_yet` was **replaced by a
    successor that asserts strictly more** —
    `the_echo_table_claims_only_its_cited_pairs_and_the_floor_claims_nothing`
    (the floor is still inert over the same walk; the echo link claims ONLY
    cited pairs; the five re-routed pairs stay compared).
  - **Full claims census at HEAD** (`DSS_PROPS_CENSUS=claims`, 439 cases × 2
    channels, 1 060 165 rows, 65.2 s; error counts at the recorded baselines —
    **5** r4133 / **22** capi, so the run is complete, not short):
    `echo-row` **488 018 cells / 468 044 in scope**, 170 spellings, all **81**
    pairs (**488 017 / 169 spellings** after the settlement's carve-out, same
    in-scope count); `CaseFold` 99 023/95 270 (+6 047/+6 039), `ArrayForm`
    122 756/118 744 (+399/+331), `BoolFold` and `EnumSynonym` unmoved;
    claimed **1 008 935 / 966 790** over **231 distinct** pairs (the `pairs`
    column stops being summable this sub-step — 20 pairs carry two
    dispositions); `UNCLAIMED` 545 568 → **51 104** (521 841 → **47 427**), 103
    pairs (**51 105 / 104 pairs** after the settlement, in-scope unchanged).
    capi claims **0** on every r4133 disposition, its 11 `ledger-hit`s
    intact. Term-by-term reconciliation: census claimed spellings 1 025 = the
    replay's `CLAIMED_TOTAL` 1 024 + the one `LIVE_ONLY_SPELLINGS` row (the same
    +1 drift RP2.1/RP2.2 recorded — 1 024 = 1 023 + 1 after the settlement);
    norm-link 855 = `CLAIMED_TOTAL_LIVE`;
    echo 170 = `CLAIMED_ECHO`; residual bin 5 = 1 069 cells = RP3.8's 1 064 +
    `line.linecode`'s 5 (RP3.6).
  - **Hand-offs consumed:** the `energymeter.peakcurrent` third spelling (the
    pair carries BOTH kinds of row and the chain discriminates per cell — 518
    `ArrayForm`, 6 echo), the two zero `CLAIMED_` locks and `MULTI_LINK_ROWS`
    (all three moved deliberately, above), and `generator.dynout` (RP2.2's fixed
    `LiveSemanticsDiffer` verdict, now with its pin).
  - **New hand-offs:** **§RP3.8** (engine sub-step, blocks RP4.1; scope, probe
    procedure, capi-side exclusion and golden-exposure measurement authored in
    the plan); **RP4.1 must narrow the 20 mixed echo rows per cell BEFORE the
    unmask** (the settlement's recorded finding, written into plan §RP4.1 as a
    precondition — afterwards the same gap is a green gate that proves less than
    it says); **RP4.1 must check the zero-damping ledger** — any r4133-gated
    deck running generator dynamics without `D=` may already carry the R2 bug in
    its triage; and **F7's deferral** — `Capacitor`/`Reactor` `FaultRate`
    (1 729 cells) and the three `CMatrix`/`RMatrix`/`XMatrix` rows stay in
    `SKIP_PROPS_BOTH_CHANNELS` with no echo row, because a row would be dead
    offline (no example row) and dead live (the skip keeps the compare away);
    their unmask + row + pin is an RP4.1-or-later decision that must land in one
    commit. Both `SKIP_PROPS` comments record it.
  - **Goldens measured, not assumed:** `git status` after the full gate shows
    only the sub-step's own files; no golden, lock, ledger, manifest or frozen
    census extract moved, and the stray `Export`/eventlog artifacts the pinned
    decks drop into `tests/corpus/electricdss-tst/` are swept by the pin file's
    own guard (verified by `git status` and `git clean -f -- tests/corpus`).
  - Full five-command gate green on the final tree, both lanes; no test was
    deleted, `#[ignore]`d or loosened — the one test that was *replaced*
    (`the_echo_table_and_the_display_floor_claim_nothing_yet`) is replaced by a
    strictly stronger successor. Test count **7 318 → 7 812** (3 659 →
    **3 906** per lane, ×2 lanes): 22 in the new pin binary (20 pins + the deck
    guard's two self-tests), 1 in the replay, and the rest are the harness-side
    guards, which land in each of the 22 test binaries that include the harness.
  - **AUDIT SETTLEMENT** (2026-08-23, follow-up commit, still **zero engine
    change**). The two audits (`audit-code`, `audit-tests`) raised **7 raw
    findings**, none of them a duplicate, so **7 deduped**: **6 fixed + 1
    recorded with an owner + 0 refuted = 7**. Every one was confirmed against
    the r4133 source, the census or a mutation before it was acted on.
    - **RECORDED (major, audit-tests) — the nine typed rows keep no cell inside
      the LIVE compare.** True as stated: `echo_excluded_r4133` answers on row
      presence, so on the 20 mixed pairs the exclusion also drops the cells the
      typed rule REFUSED, and the mitigation this record and three documents
      credited to the chain order ("keeping 6 446 cells inside the value
      compare") is offline attribution only. Reproduced here through the real
      comparator: `Load.Yearly` `'day'` vs `'night'` passes on r4133 and fails on
      capi. What the order really buys — the census disposition and each row's
      liveness hit — is now what the docs say (props_norm's module doc and the
      `PROPS_ECHO_R4133`/`PropsPolicy::echo_excluded` docs, the plan §RP2.3
      as-executed and §1.1, the vendored README, this record), the shipped
      behaviour is pinned by
      `props_policy_tests::a_mixed_pairs_echo_row_masks_the_cells_its_rule_refuses`
      (written to be *replaced* by the narrowing, not deleted), the offline test
      that carried the false name is renamed
      (`…masks_only_what_the_typed_rules_leave` →
      `the_typed_rules_and_the_echo_rows_split_their_shared_pairs_offline`), and
      the narrowing itself — per-cell rows for those 20 pairs — is an explicit
      **RP4.1 precondition** in the plan, with `ECHO_CARVE_OUTS` as the
      mechanism. Not fixed here because it re-opens RP2.3's row shape, which
      plan §1.2 fixes, and would move every count lock plus a full re-census.
    - **FIXED (minor, audit-code) — `reactor.kvar`'s row masked a cell that is
      not an echo.** 606 of the pair's 607 census cells are the frozen `'1200'`
      the row cites; the 607th (`'66.6666666666667'` vs `'66.667'`, 5.0e-06) is
      r4133's own LIVE value, because `TReactorObj.MakePosSequence` builds
      `Format(' kV=%-.5g kvar=%-.5g')` and runs it back through
      `Parser.CmdString`/`Edit` (`Version8/Source/PDElements/Reactor.pas:
      1145-1201`) — dss_capi 0.14.5 uses `SetDouble` there and never a string,
      which is why the capi channel compares that cell and the port matches it.
      Narrowed, not re-explained: new `props_norm::ECHO_CARVE_OUTS` (exact
      `(rust, r4133)` match, never a shape heuristic) takes the cell out of the
      row, and `props_r4133_replay::ECHO_CARVE_OUT_ROUTING` declares it to
      **RP2.4** — where the same round-trip's `reactor.kv` already sits
      (`bins.tsv` bin 6, 5.85e-06). Locks: `CLAIMED_ECHO` 170→**169**,
      `CLAIMED_TOTAL` 1 024→**1 023**, `DECLARED_RP24` (2 100, 70, 2 020)→
      **(2 101, 71, 2 021)**. Re-measured census: `echo-row` 488 018→**488 017**
      (169 spellings), `UNCLAIMED` 51 104→**51 105** (104 pairs) — the cell is on
      a capi-only case, so **no in-scope number moves**.
    - **FIXED (minor, audit-code) — `RP38_ROUTING` cited `IndMach012.pas:1789`
      and `Power[1,ActorID]`.** :1789 is arm 4 (`kWBase`); arm 5 is `:1790` and
      reads `Power[1,ActiveActor]`. Corrected there and in
      `the_silent_readonly_pairs_have_no_echo_row`'s doc; the plan, this file and
      the README already said :1790, so the two records now agree.
    - **FIXED (minor, audit-code) — the `EmptyCollectionRender` doc claimed too
      much.** "The two sides mean the SAME thing… an empty render carries no
      value to preserve" holds for 12 of the 14 rows, not for
      `storagecontroller.seasontargets`/`seasontargetslow`: with `Seasons = 1`
      r4133's `ReturnSeasonTarget` exits before emitting anything
      (`StorageController.pas:2445-2449`) while the port prints the live target,
      237 cells each. The doc now says what all 14 share and names the two
      exceptions with their Pascal, including why the tag still fits (the value
      itself is identical — `SeasonTargets[0] := FkWTarget`, `:883-884`, read
      back at `:1470`).
    - **FIXED (minor, audit-tests) — the seam ORDER was untested where it
      ships.** It was pinned in the two OFFLINE copies of the chain
      (`claim_value`, `Link::ORDER`) and nowhere at `compare_prop_lists`, so
      swapping the two lines there left the whole suite green.
      `props_policy_tests::the_normalization_seam_runs_before_the_exclusion_on_a_mixed_pair`
      drives the real comparator on `reactor.bus2` and asserts the row's
      visit/hit deltas; mutation-checked (the swap reds it, `Some((0,0))` vs
      `Some((1,1))`). Two new `props_norm` accessors (`norm_counters`,
      `echo_counters`) make the counters readable; the two tests use different
      mixed pairs and each leaves `hits > 0` on every row it touches, because
      `cargo test` runs them concurrently with the gate's own
      `assert_*_rows_are_live()` epilogue (measured: the first draft used
      `Load.Daily`, which HAS a `CaseFold` row, and reddened the gate).
    - **FIXED (minor, audit-tests) — two unpinned exemption lists.**
      `ECHO_ROWS_WITH_NO_IN_SCOPE_CELL` (disarms fail-on-stale per row) and the
      replay's `NOT_A_PIN` (disarms the "a witness names a real `#[test]`"
      guard) were iterated as-is, so a third entry would have silently switched
      the guard off for another row. Both are now pinned literally
      (`the_dormant_row_exemption_list_is_pinned`,
      `the_non_pin_exemption_list_is_pinned`), `NOT_A_PIN` hoisted out of the
      test body to a documented const; mutation-checked (a third entry in either
      reds its test).
    - **FIXED (minor, audit-tests) — `Capi(n)` cannot witness cells on
      `engines: "r4133"` cases.** Measured with the full claims census crossed
      against each case's `engines` flag (97 of the 439 walked cases are
      r4133-only): **57 of the 81 rows mask 34 969 such cells**, and 31 of them
      carried a capi-only witness — the exact rule plan §1.2 mechanic (c) states
      ("r4133-only classes/**cases**") and part B2 applied by hand to
      `swtcontrol.action` alone. The population is now a const
      (`ECHO_ROWS_ON_R4133_ONLY_CASES`, sorted, count- and cell-locked) with a
      test that every listed pair names a pin, and the 31 rows got one: **nine
      new pins** (20 → 29 names, 32 → 63 rows), each compiling a deck the census
      named for that pair — `der_amp_limits_render_the_live_sentinel_and_gain`,
      `der_user_model_arrays_render_empty_when_unset`,
      `energymeter_action_and_capcontrol_reset_render_no_pending_command`,
      `fuse_switchedobj_defaults_to_the_monitored_element`,
      `invcontrol_defaults_render_the_live_values`,
      `load_zipv_renders_the_live_seven_element_vector`,
      `pd_element_perm_and_repair_render_the_live_ratings`,
      `reactor_kvar_renders_the_live_rating`,
      `storagecontroller_seasontargets_render_the_live_targets` — plus
      `line_conductors_renders_the_live_conductor_list` extended to `wires`/
      `cncables`/`tscables` (6 231 exposed cells each). All but two carry a
      discriminating second reading; the two that cannot (`energymeter.action`,
      `capcontrol.reset`) are one-shot COMMANDS whose empty re-read is itself the
      pinned fact, asserted before and after the command. The other half of the
      finding — nothing checked that a `Capi` witness names a pair capi can even
      compare — is closed by
      `a_capi_witness_is_a_pair_the_capi_channel_can_compare` (no `Capi` row may
      collide with `SKIP_PROPS`, `PROPS_015X` or the whole-element skip).
    - **Scope kept:** zero product-crate bytes, no golden / lock / ledger /
      manifest / frozen-extract byte, `R4133_DISPLAY_FLOOR` still `None`, the
      §1.1(e) masks untouched, no tolerance anywhere. Census artifacts
      (`tmp/props_census.json`, `tmp/props_census/`, `tmp/`) deleted by name;
      `git status` clean before the commit. Full five-command gate green on the
      final tree, both lanes; test count **7 812 → 8 098** (3 906 → **4 049** per
      lane): nine pins, two replay guards and six harness-side guards that land
      in each of the 22 binaries including the harness. No test deleted,
      `#[ignore]`d or loosened; the one renamed test asserts the same thing under
      a name that describes it.

- **RP2.4** (2026-08-23) — **the r4133 props display floor; WP-RP2 closes with
  it.** **Zero engine change**: four test files + docs, zero product-crate
  bytes, no golden / lock / ledger / manifest / frozen-extract byte, no r4133
  mask moved, no `Tolerances` field or tier touched.
  - **The derivation is a measurement, not the plan's paragraph.** Scored every
    example row of `examples_full.txt` + `examples_supplement.txt` (**3 454
    rows** = every distinct `(rust, r4133)` spelling of every census pair, so
    the worst *spelling* IS the worst *cell*) with the shipped metric
    `props_norm::display_rel`. Worst cell the floor claims **6.431124e-05**
    (`load.pf` `'0.747651914485831'` vs `'0.7477'`, 33 cells, in scope);
    **floor = `2e-4`**, 3.110× above it; nearest row above the band
    **1.374769e-03** (`storagecontroller.kwneed`, 6.874× over the floor);
    nearest genuine value jump **4.404256e-03** (`generator.kvar`, 22.02×);
    smallest in-scope genuine jump **5.524501e-02** (`regcontrol.remoteptratio`,
    276.2×). The band `(6.431124e-05, 1.374769e-03)` is **empty, 21.38× wide** —
    the kill criterion needs >5×, so no kill, and the number is *placed* rather
    than tuned.
  - **Two recalibrations against the plan**, both recorded in
    `tests/TOLERANCE_NOTES.md`: (1) the plan's worst `6.43e-5` on `load.pf` is
    confirmed exactly; (2) the plan's "smallest genuine jump 1.00e-3,
    `invcontrol.lpftau`" is a number in the **census's** metric, which reports
    the ABSOLUTE difference when the expected side is 0
    (`harness::value_verdict`). Under the floor's symmetric metric that cell
    (`'0.001'` vs `'0.0'`) is rel **1.0** — a 0-vs-nonzero pair can never be
    claimed at any magnitude. Do not repeat 1.00e-3 as the floor's upper
    neighbour.
  - **Mechanism, cited site by site** (`Version8/Source/`) — the fixed-precision
    getters, N ∈ {4,5,6,7,8}: `%-.4g` `Load.pas:2345` (`pf`, the worst cell's,
    and a formatter the plan does not name) + `:2353`; `%-.5g`
    `Vsource.pas:1327-1335` and `:1343`, `Transformer.pas:1842-1843`,
    `AutoTrans.pas:1886-1887` + the `MakePosSequence` round-trips
    (`Transformer.pas:1982-1991`, `AutoTrans.pas:2021-2030`,
    `Reactor.pas:1145-1201`); `%.6g` `Storage.pas:1531-1562` and
    `Utilities.pas:2600-2607` `GetDSSArray_Real`; `%-.7g` `Line.pas:1358-1365`,
    `:1406-1407`; `%-.8g` `Vsource.pas:1337-1342` + `:1344`,
    `Reactor.pas:1091-1098`. The plan's "`%-.5g`/`%-.8g`" naming is
    **incomplete, not wrong**. *(As landed, the `Vsource` ranges overlapped the
    wrong formatter and the `%-.5g` row listed `basekv`, which has no getter arm
    at all — corrected by the audit settlement in all three copies; and the
    universal claim "every claimed cell is such a render" was replaced by the
    per-cell clause, see the settlement bullet below.)*
  - **The `%-.4g` residual is stated, not absorbed** (the anti-fudge decision of
    the sub-step). `load.pf`'s theoretical class ceiling is 5e-4, which does not
    fit the band (2.75× under its upper neighbour). The floor comes from the
    population instead: the 293 `load.pf` spellings have min mantissa **5.653**
    (`pf = 0.5653`) → population ceiling **8.845e-05**, and 2e-4 sits 2.26×
    above THAT. Residual risk left loud: a future in-scope load with
    `pf ∈ [0.1, 0.25)` could print up to 5e-4, the floor would **refuse** it and
    the gate would red — the safe, re-derivable direction. Widening to the class
    ceiling would be the fudge the discipline forbids.
  - **Landed as a cell predicate, r4133 only.** `R4133_DISPLAY_FLOOR`
    `None` → `Some(2e-4)`; new `number_rel` (the symmetric metric),
    `display_rel` (whole-cell, over `numeric_skeleton`, so scalars, bracketed
    vectors and `|`-separated matrices go through one path — a scalar-only floor
    would have left every array cell unclaimed), `under_display_floor` (offline)
    and `under_display_floor_r4133` (the seam, with `FLOOR_VISITS`/`FLOOR_HITS`
    for RP4.1); `numbers_match` is the ONE widening point and grew a non-finite
    guard. Seamed into `compare_prop_lists` as the fourth link **after** the
    echo seam via `PropsPolicy::under_display_floor` (gated on `is_r4133()`).
    The replay's floor link now calls the **shipped** predicate — it was a
    scalar-parse copy, which would have left every bracketed/matrix display cell
    unclaimed — and `props_census`'s `under-floor` disposition (built dormant by
    RP2.1) went live.
  - **Locks moved** (the values below are the as-landed ones; the settlement
    bullet carries the four it then moved): `CLAIMED_DISPLAY_FLOOR` 0 →
    **2 006**; `CLAIMED_TOTAL` 1 023 → **3 029** (now the sum of all four links);
    `DECLARED_RP24`
    (2 101, 71, 2 021) → **(0, 0, 0)** — the sub-step's acceptance;
    `DECLARED_OUT_OF_SCOPE` (134, 18, 0) → **(229, 22, 0)**. New locks
    `RP24_OUT_OF_SCOPE_ROWS` 105, `RP24_OUT_OF_SCOPE_MIN_RATIO` 277.0,
    `CEILING_ROUND_MARGIN` 1.01, `LIVE_ONLY_DISPLAY_SPELLINGS` (6) and
    `CLAIMED_SPELLINGS_LIVE` 3 036. **Unmoved and verified:** `MULTI_LINK_ROWS`
    135 (measured 0 overlaps — the floor's 2 006 is a clean addition, not a
    re-partition), `CLAIMED_NORMALIZATION` 854, `CLAIMED_ECHO` 169,
    `CLAIMED_SHAPE_ALLOWLIST` 0, `CLAIMED_TOTAL_LIVE` 855, `NORM_ROWS` 170,
    `PROPS_ECHO_R4133` 81, `ECHO_CARVE_OUT_CELLS` 1, every `SKIP_PROPS*` /
    `PROPS_015X` / scheduler mask.
  - **`DECLARED_RP24`'s 105-row residual is PROVED out, not waved off.** The
    floor claims 1 996 of the bucket's 2 101 rows; the other 105 sit on exactly
    four pairs (`generator.kvar` 102, `capacitor.cuf` 1, `storage.kw` 1,
    `storagecontroller.kwneed` 1) — the four whose FULL-census bin is 7 while
    their in-scope re-derivation is bin 6, so `effective_bin()` routes the pair
    to the floor while its example inventory still carries the spellings that
    made it bin 7. `RP24_OUT_OF_SCOPE` + `row_out_of_scope_by_ceiling` re-declare
    them `Owner::OutOfScope` by a citable argument: `max_rel_in_scope` is the
    maximum over exactly the in-scope cells, so a spelling that exceeds it cannot
    be one of them. Narrow by construction (four cited pairs, numeric rows only,
    ceilings read back from `bins.tsv`), and the tightest row clears its ceiling
    by **277×** against a `%.2e` rounding needing 1.005×. `row_in_scope` uses it,
    so `DECLARED_OUT_OF_SCOPE.2` stays **0** — now a per-ROW zero, stronger than
    the per-pair one it replaces. **The live census confirms it independently:**
    all 12 pairs that carry both an `under-floor` and an `UNCLAIMED` cell report
    `cells_in_scope = 0` on every unclaimed spelling.
  - **Full claims census at HEAD** (`DSS_PROPS_CENSUS=claims`, 439 cases × 2
    channels, 1 060 165 rows, 58.1 s; error baselines exact — **5** r4133 /
    **22** capi, so the run is complete, not short): `under-floor` **49 451
    cells / 46 538 in scope**, **2 012** spellings, **79** pairs; every other
    r4133 disposition **unmoved to the cell** (`BoolFold` 294 519/280 915,
    `CaseFold` 99 023/95 270, `ArrayForm` 122 756/118 744, `EnumSynonym`
    4 619/3 817, `echo-row` 488 017/468 044); claimed **1 058 385 / 1 013 328**
    over **309** distinct pairs; `UNCLAIMED` 51 105 → **1 654** (47 427 →
    **889**), 104 → **37** pairs. **capi: 0 on every r4133 disposition**, its 11
    `ledger-hit`s (4 in scope, 8 pairs) and 88/88 `UNCLAIMED` intact — 99 value
    cells / 92 in scope, unchanged since RP2.1.
    - **The live worst floor-claimed cell is the derivation's own**:
      6.431124e-05 on that same `load.pf` spelling, over the whole 439-case
      population — the frozen extract did not under-sample the tail. All 293
      live `load.pf` spellings are `under-floor`.
    - **Term-by-term reconciliation.** Live floor spellings **2 012** = the
      replay's `CLAIMED_DISPLAY_FLOOR` **2 006** + **6** live-only
      (`vsource.isc1` ×2, `isc3`, `r1`, `x0`, `x1` — the same "+2 cells per
      `vsource` pair" population drift RP2.1/RP2.2 recorded, at spelling
      granularity; all five pairs hold a frozen `bins.tsv` row, so no vendored
      file may carry them). Live floor cells **49 451** = the frozen 49 437 over
      the shared 2 006 rows + **8** (eight rows are +1 live) + **6**. Claimed
      spellings **3 036** = `CLAIMED_TOTAL` 3 029 + 1 `LIVE_ONLY_SPELLINGS` + 6.
      All of it is now an *asserted* term
      (`LIVE_ONLY_DISPLAY_SPELLINGS`, `CLAIMED_SPELLINGS_LIVE` and
      `the_display_floors_live_only_spellings_reconcile_the_claims_census`),
      not narration. `UNCLAIMED`'s shrink is exactly the floor's claim
      (−49 451 / −46 538).
    - **The 889 in-scope residual is fully attributed** (re-grouped by the pair's
      frozen bin): bin 5 → 6 pairs / 1 069 / 777 = **RP3.8** (5 pairs,
      1 064/772) + `line.linecode` (**RP3.6**, 5/5); bin 7 → 20 pairs / 361 /
      **32** = `swtcontrol.delay` (**RP3.1**, 42/24), `windgen.kvar` (**RP3.2**,
      4/4), `generator.model` (**RP3.3**, 2/2), `gictransformer.r2` (**RP3.4**,
      2/2), the other 16 pairs 311/**0**; bin 4 → 8 pairs / 186 / 80 = **RP3.7**
      (`swtcontrol`/`relay` `normal`/`state`) + the four `sensor.*` at 0 in
      scope; `autotrans.wdgcurrents` 34/0, `line.units` 3/0 (**RP3.5**),
      `isource.bus1` 1/0 (§1.3). **Bin 6 is gone from the unclaimed table
      entirely** — RP2.4's acceptance, measured.
  - **Hand-off consumed:** RP2.3's `ECHO_CARVE_OUT_ROUTING` cell
    (`reactor.kvar` `'66.6666666666667'` vs `'66.667'`, 5.0e-06, r4133's own
    `MakePosSequence` `%-.5g` round-trip) is now **claimed by
    `Link::DisplayFloor`**, and `the_carve_outs_are_routed_and_only_they_are`
    chases it to that link instead of asserting a non-empty RP2.4 bucket (which
    at `(0,0,0)` would be vacuous). Its sibling `reactor.kv` (bin 6, 5.85e-06)
    goes the same way. **No new hand-off.** Buckets still open: `Rp3` (7, 4, 7),
    `Rp35` (8, 6, 5), `Rp38` (181, 5, 181), `OutOfScope` (229, 22, 0);
    `Rp22`/`Rp23`/`Rp24` are all `(0, 0, 0)`.
  - **`tests/TOLERANCE_NOTES.md` §"r4133 props display floor"** carries all eight
    required components (scope; measured worst + ratio; the `%[-].Ng` mechanism
    with its Pascal table; the decomposition/empty-band argument; why no coverage
    is lost; scope justification; fix owner; and the relaxes/never-relaxes pair,
    including the `%-.4g` residual and the honest limit that a floor cannot
    separate a display artifact from a genuine sub-2e-4 difference — bounded by
    the capi compare at the case tier floors on every `both` case and by the
    1e-6-class model gate on the `r4133`-only ones). The stale RP2.1 paragraph
    that said the slot was `None` is corrected in the same file. This is the
    plan's ONE sanctioned edit to that file (§1.1). *(Components 3, 5, 7 and 8
    were rewritten by the audit settlement: the mechanism is now a clause, the
    capi bound is the tier floors and not "zero tolerance", and the fix owner is
    "none for what it claims, RP3.9 for what it refuses".)*
  - **Mutation-tested, three ways** (tree restored byte-identical each time):
    the floor made channel-blind → caught by 2 tests; widened 10× to `2e-3` →
    9 tests across 3 binaries, including the master accounting; made scalar-only
    → 5 tests.
  - **Tests: none deleted, three replaced by strictly stronger successors** —
    the dormant `R4133_DISPLAY_FLOOR.is_none()` guard → the literal value plus
    both sides of the boundary (1.8996e-4 in, 2.0996e-4 out) plus the
    0-vs-nonzero and non-finite refusals; the floor-slot placeholder row in
    `the_capi_channel_claims_nothing` → promoted into that test's r4133-claimed
    list (a live channel statement, not a slot guard);
    `..._and_the_floor_claims_nothing` → `..._and_the_floor_only_its_derivation`
    (every floor-claimed row is numeric, inside the floor, and the worst is
    6.431124e-05). Six new: the metric decomposition, the seam's visit/hit
    counters, the two channel tests, the plan's acceptance probe landed
    permanently
    (`the_display_floor_drops_only_the_cell_it_claims_only_on_r4133`), the
    residual scope proof, and the live-only reconciliation.
  - **Goldens measured, not assumed:** `git status` after the full gate showed
    only the sub-step's own files; the stray `Export` artifacts the AutoTrans
    decks drop into `tests/corpus/electricdss-tst/` were swept
    (`git clean -f -- tests/corpus`), census artifacts (`tmp/props_census.json`,
    `tmp/props_census/`, `tmp/`) deleted by name. Full five-command gate green
    on the final tree, both lanes; test count **8 098 → 8 322** (4 049 →
    **4 161** per lane, **+112**): 5 harness-side guards × 22 harness-bearing
    binaries, plus 2 in the replay binary (the residual scope proof and the
    live-only reconciliation).

- **RP2.4 audit settlement** (2026-08-23, one commit, still **zero engine
  change**) — eleven findings from the two auditors, all settled; the two majors
  were the same defect measured two ways.
  - **The majors: the mechanism was a survey, and 55 spellings contradicted it.**
    Part A attributed the claimed population to the Delphi `%[-].Ng` family by
    naming the formatter sites and asserting the conclusion universally in four
    places (`R4133_DISPLAY_FLOOR`, TOLERANCE_NOTES components 3 and 7, STATUS).
    `audit-code` re-scored all 3 454 spellings against "is r4133's number a
    rounding of ours to SOME digit count (both tie rules)" and found **55 rows /
    70 cells / 27 pairs** where it is not; `audit-tests` reached the same
    population from the other side (the pair's own formatter ceiling, 88 rows /
    109 cells / 34 pairs, a superset by a laxer estimator) and additionally ran
    the live census to prove all of them carry `count_in_scope = 0`. Both are
    reproduced here. **Fixed by narrowing, not by re-explaining**: the floor grew
    a second clause, `props_norm::display_is_render` — every number of the r4133
    side must be our number rounded to the digits r4133 *printed*, read off its
    own spelling (`0.5·10^(e−N+1)` + a tie margin + one half-unit of the 15-digit
    grid FPC's conversion may round through). The mechanism claim is now a
    property of the predicate. The 55 are declared to a new **RP3.9**
    (`RP39_ROUTING`: 27 pairs, each with its r4133 site and its round-trip chain
    — `load.kva` recomputed from an already round-tripped `pf`, `vsource.puz*`
    from a round-tripped Z, `line.b0`/`b1` from a round-tripped C, and the
    full-precision `%-g`/`%g` getters where the two engines simply differ), which
    puts them back in `claims_unclaimed_pairs.txt` — the WP-RP3 work list — and
    makes RP4.1 wait on them (plan §0). No cell of the 55 is in scope, so nothing
    the gate compares moved. (**RP3.9 landed 2026-09-02** — all 27 pairs
    `PRECISION_ROUNDTRIP`, its record in §1.)
  - **Locks moved by the settlement:** `CLAIMED_DISPLAY_FLOOR` 2 006 → **1 951**;
    `CLAIMED_TOTAL` 3 029 → **2 974**; `CLAIMED_SPELLINGS_LIVE` 3 036 →
    **2 981**; new `DECLARED_RP39` **(55, 27, 19)** and `RP39_ROUTING` (27 rows,
    per-pair row + in-scope split + citation). `DECLARED_OUT_OF_SCOPE`
    (229, 22, 0), `RP24_OUT_OF_SCOPE_ROWS` 105 and the 277× margin are
    **unmoved** — `declare` runs the mechanism arm first, so the two rows that
    would otherwise have drifted into the ceiling rule (and dropped its measured
    margin to 2.08×) stay RP3.9's.
  - **Live re-census** (439 × 2, 1 060 165 rows, 56.9 s, error baselines exact
    5/22): `under-floor` **49 381 / 46 538 in scope / 1 957 spellings / 69
    pairs**; `UNCLAIMED` **1 724 / 889 / 480 / 59**. **In-scope counts on both
    sides unchanged**, every other disposition unmoved to the cell, capi still 0
    on every r4133 disposition — the settlement moves exactly the 70 cells it
    claims to move.
  - **The other major — "the capi compare is exact / at zero tolerance" is
    false.** `compare_all_properties` hands `compare_prop_lists` the case tier's
    `i_rel`/`i_abs` and `value_verdict` passes at `abs + rel·|e|`. Corrected in
    all five places (TOLERANCE_NOTES components 5 and "scope justification",
    `PropsPolicy::under_display_floor`, `R4133_DISPLAY_FLOOR`, the census
    comment) and replaced by the honest tier statement, now pinned with the two
    kinds that are the hole: `midi` (no `tol_for` arm → 1e-6/1e-4) and
    `micro_wtg3_dynamics` (2e-5/1e-4) do not bound a value below 0.5, `feeder`
    none below 0.05 — `props_policy_tests::the_capi_property_compare_runs_at_the_case_tier_floors`.
  - **Six minors, each with the guard the auditor named missing.** (1) The
    `Vsource.pas` ranges + the `basekv` attribution, fixed in all three copies.
    (2) The "never relaxes a discrete value" bullet, which the metric alone did
    not provide: reworded to what the predicate does, and the general case is now
    covered by the mechanism clause (`'5001'` vs `'5000'`, 2.0e-4, refused —
    pinned). Its two one-sided constants are pinned as measurements now as well:
    `RP24_OUT_OF_SCOPE_MIN_RATIO` is bracketed above (`< 278.0` against the
    measured 277.17×) and `CEILING_ROUND_MARGIN` literally (`== 1.01`), so the
    auditor's 277 → 2 and 1.01 → 200 mutations red. (3) The `ArrayForm` wiring, provably inert (the auditor's mutation
    `numbers_match → ==` ran green): the RP2.1 discrimination row the widening
    had taken back (`'[600,700.007,]'`, 1e-5) is **restored** — the render clause
    refuses it — and a positive display fold (`'[ 600 700.00003]'` vs
    `'[600,700,]'`) now pins the wiring from the other side. (4) The seam
    counters, which no test tied to the seam: the comparator test and the counter
    test both drive `PropsPolicy::under_display_floor` and assert
    `FLOOR_VISITS`/`FLOOR_HITS` move, so the offline-twin swap reds. (5)
    `LIVE_ONLY_DISPLAY_SPELLINGS`, which carried only its length: each of the six
    is now anchored to one of five cited `vsource` pairs and to that pair's own
    frozen `max_rel` (the auditor's `'1.4257'` mutation is 9.09e-5 against
    3.72e-5 — and is no longer a render either). (6) The vacuous
    "differing skeleton" block in the metric test: two rows with the SAME number
    count and a different skeleton (`'[ 400]'` vs `'400'`, `'1 kV'` vs `'1 kW'`)
    now exercise the clause the block claimed to pin. (7) `row_out_of_scope_by_ceiling`
    compared a symmetric metric with the census's offender-only `max_rel`: it now
    **refuses multi-number rows** outright, with the implication written out — on
    a one-number row the census metric dominates `display_rel`, on a multi-number
    row it need not.
  - **Every mutation the auditors landed green now reds**, verified: the
    `tokens_match` widening removal (1 test), the `PropsPolicy → props_norm`
    seam swap (2), the fabricated live-only spelling (1), the skeleton-clause
    deletion (2), plus two of the settlement's own — dropping the mechanism
    clause from the floor (7 tests across the accounting, the two channel tests
    and the metric) and dropping the single-number guard from the ceiling rule
    (1). Test count
    **8 322 → 8 368** (4 161 → **4 184** per lane, **+23**): 1 new harness guard
    (the capi tier floors) × 22 harness-bearing binaries, plus 1 new replay test
    (RP3.9's both-ways proof) — and no test deleted, `#[ignore]`d or loosened.

