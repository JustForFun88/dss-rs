# R4133_PROPS - WP-RP4 record

> Moved verbatim from `STATUS.md` on 2026-09-03 (STATUS.md archiving round 2);
> order preserved, nothing rewritten. It holds STATUS §1's full record of RP4.1
> (`all_properties` unmasked on the r4133 channel) and its audit settlement.
> STATUS.md §7 forwards `§WP-RP4` and `§RP4.1` here.

**RP4.1 (`all_properties` unmasked on the r4133 channel) landed 2026-09-03 —
WP-RP4's single sub-step and G1.1's deliverable, in one commit, with zero
product-crate lines, zero golden bytes and zero tolerances moved.** The
per-channel `compare_all_properties = false` clears are gone from both the
**gate** path and the **seeding** path (`corpus_gate/scheduler.rs`), and
`force_properties` now forces the property request on **every live non-`large`
case** — `gates_capi ∪ gates_r4133` is every live case, so the honest spelling
replaced the two-arm test, and the `large` cost guard stays. The **83 r4133-only
non-large** cases get a property check for the first time and the **313
non-`large` `both`** cases get their r4133 property table compared: a full run
now does **1 670** gating r4133 property walks over **151 782** elements,
identically in both lanes. (The audit settlement corrected this figure from
"367 `both`": the cost guard leaves the remaining **54** `both` cases —
every `kind=large*` one — property-unchecked on *both* channels, which is the
plan's deliberate cost guard and an open item for the RP5.2 closing record, not
a regression.) ~24 stale doc claims asserting that the r4133 channel never
property-compares were corrected, or left with a dated "superseded by
R4133_PROPS RP4.1" note where the file is a historical record
(`docs/phase-records/`, `UNIFIED_GATE_PLAN.md`). The commit is **23 files
(+2 576 / −536)** (`59e521e5`; the first draft of this paragraph said 22 files
/ +2 305 / −515 and missed `tools/oracle/README.md` — corrected by the audit
settlement); the only `src/` diffs are comment-only, in a `#[cfg(test)]` module
(`exec/tests/controls.rs`) and in the `publish = false` bridge
(`crates/dss-epri`).

*Precondition A (RP2.3's audit settlement) — the 20 mixed echo rows narrowed
**per cell**, before the flip.* Mechanism (b) of the plan's two, per the
coordinator's ruling: `ECHO_NARROWED` gives each of the 20 rows the measured
`(rust, oracle)` spellings its exclusion still covers — **66** spellings, every
one present verbatim in the frozen census — and `echo_excluded` /
`echo_excluded_r4133` match on them via `row_covers`; the 62 pure echo rows keep
the conservative pair scope, and `ECHO_CARVE_OUTS`' doc was rewritten to own the
inversion for the 20 explicitly (on those rows an *unseen* spelling now falls
**out** of the exclusion and is compared — which is the point of the
precondition). `MULTI_LINK_ROWS` 135 → **0** (no frozen row matches two links
any more), the old population restated as `MIXED_PAIR_NORM_ROWS = 135` = the 20
pairs' 201 census rows − 66. `a_mixed_pairs_echo_row_masks_the_cells_its_rule_refuses`
was **replaced, not deleted**: its first assertion now asserts the cell is
COMPARED on r4133, with the capi and neighbour assertions kept.

*Precondition B (RP3.1's audit settlement) — the staged entries landed and the
accounting moved with them.* The **eight** entries landed verbatim from their
STATUS drafts into `tests/corpus/ledger.json` — `r4133-swtcontrol-delay-ignored-time`
and `-midi` (RP3.1), `r4133-windgen-kvar-dispatched-{daily,delta,dyn,dynfault}`
(RP3.2), `gic-pct-r2-honoured-gictransformer-r4133-props` and
`gic-pct-r2-honoured-midi-r4133-props` (RP3.4) — with the two new causes
`swtcontrol-delay-not-wired` and `windgen-kvar-renders-dispatched-q`; RP3.4's
two reuse the existing `gic-pct-r2-ignored`. Ledger **45 → 53 entries / 27 → 29
causes**. The accounting move needed a mechanism, not a hand edit: `DECLARED_RP3`
is asserted against the measured walk, so RP4.1 landed `RP3_LEDGERED` (the
interception shape `RP38_SUPERSEDED` already uses) with its own bucket lock
`LEDGERED_RP3 = (6, 3, 6)` and a both-ways guard that reads the eight ids back
out of the live ledger; `DECLARED_RP3` is `(0, 0, 0)` as a *measurement*, and the
three `RP3_ROUTING` rows (`swtcontrol.delay`, `windgen.kvar`,
`gictransformer.r2`) are retired to `0, 0` with their verdicts amended to "landed
at RP4.1". The tripwire was renamed
`the_staged_r4133_property_entries_have_not_landed_yet` →
`the_staged_r4133_property_entries_landed_at_rp41` and re-stated positively (the
landed `property`-scoped `r4133` set is exactly `LEDGER_ENTRY_PINS`' eight, each
on the case its pin cites); the "no NEW un-reviewed entry" half moved to the
`RP3_LEDGERED` guard. RP3.12's four `controls:autotrans/*` decks and
`makeposseq_ctrl.dss` keep `engines: capi_v0145` and their drafted skips were NOT
landed (coordinator decision 5), and `DECLARED_RP39` / RP3.12's tables were not
retired (decision 6).

*Residual triage — the kill criterion does not fire.* **Zero** ledger entries and
**zero** pins were born from RP4.1's own residual triage (criterion: more than
~15, or any residual that cannot be pinned → stop; the eight of precondition B do
not count), and no residual failed to be pinned. What the first unmasked run did
find was a **stale exclusion**, not a divergence: the two `ArrayForm` rows
`swtcontrol.normal` and `swtcontrol.state` folded nothing across 40 compared
cells, because RP3.7(a) made both sides spell the array identically
(`Version8/Source/Controls/SwtControl.pas:589-599` / `:600-610`, per-phase
`pStateArray`). The rows are dropped and re-owned by the RP3.8-shaped
`RP37_SUPERSEDED` (`SUPERSEDED_RP37 = (5, 2, 5)`, `Ledger::superseded` now per
table), with `DECLARED_RP35 (8, 6, 5) → (5, 4, 2)` and the six claim locks
re-derived: `NORM_ROWS` 170 → 168, `NORM_ARRAY_FORM_ROWS` 23 → 21,
`CLAIMED_ARRAY_FORM` 203 → 201, `CLAIMED_NORMALIZATION` 854 → 852,
`CLAIMED_TOTAL_LIVE` 855 → 853, `CLAIMED_SPELLINGS_LIVE` 2 982 → 2 980. The live
census independently reports 21 `ArrayForm` pairs / 201 spellings, so the offline
and live accountings now agree on this rule.

*Acceptance — the census at HEAD against the P0 baseline.* One full-population
run, not an aggregation of `DSS_GATE_ONLY` slices: **440 cases × 2 channels,
1 059 178 rows, 56 s**, `gate_only: null`.

| r4133 disposition | P0 cells | HEAD cells | P0 in scope | HEAD in scope | pairs P0 → HEAD |
|---|---|---|---|---|---|
| `UNCLAIMED` | 534 | **528** | 30 | **0** (reading rule below) | 49 → **47** |
| `ledger-hit` | 0 | **6**\* | 0 | **6**\* | 0 → **2** |
| `echo-row` | 488 019 | 488 019 | 468 046 | 468 046 | 82 → 82 |
| `normalized-by-BoolFold` | 294 519 | 294 519 | 280 915 | 280 915 | 77 → 77 |
| `normalized-by-CaseFold` | 99 023 | 99 023 | 95 270 | 95 270 | 65 → 65 |
| `normalized-by-ArrayForm` | 122 754 | 122 754 | 118 744 | 118 744 | 21 → 21 |
| `normalized-by-EnumSynonym` | 4 619 | 4 619 | 3 817 | 3 817 | 5 → 5 |
| `under-floor` | 49 484 | 49 484 | 46 627 | 46 627 | 71 → 71 |
| value cells | 1 058 952 | 1 058 952 | 1 013 449 | 1 013 449 | — |

\* **Second reading rule, to be carried forward:** acceptance is read off the
lossless `props_census.json`, **never** off `claims_summary.json`, which
collapses a mixed-disposition spelling to its weakest cell
(`mixed_disposition_spellings: 1`, `swtcontrol.delay` — 24 in-scope `ledger-hit`
cells on the two entry decks plus 12 out-of-scope `UNCLAIMED` on the capi-only
`swtcontrol_lock.dss`). Streamed per row over all 1 059 178 rows the lossless
record reads: r4133 `UNCLAIMED` in scope **0**, `UNCLAIMED` out of scope **504**,
`ledger-hit` in scope **30** = 24 (`swtcontrol.delay`) + 4 (`windgen.kvar`) + 2
(`gictransformer.r2`) — cell for cell the eight entries' 30 live gate hits, and
exactly the P0 prediction. No new pair appeared (two deletions, zero additions).
The **capi channel A/B is bit-identical**: all 8/8 capi_v0145 census artifacts
byte-equal to the pre-flip baseline.

**Plan-text deviation, recorded verbatim (coordinator decision 1, 2026-09-02):**
"**Acceptance is read as "zero UNCLAIMED cells IN SCOPE"** (the gate never
compares an out-of-scope cell, so the plan's "per-cell accounting closes" applies
to compared cells). No new census disposition is added in RP4.1. P4 MUST
additionally close the out-of-scope accounting in its handoff and STATUS with a
per-owner table of every out-of-scope UNCLAIMED cell (RP3.9 pins / RP3.12 pin /
`RP24_OUT_OF_SCOPE` gendispatcher / G2.5 Cuf), each owner citing its pin or
record, so the total (534 → whatever HEAD reports) is accounted line by line."
The 504 out-of-scope cells — **22 cases / 47 pairs, all on `engines:
capi_v0145` decks** — are accounted per owner:

| cells | dominant pairs | owner and citation |
|---|---|---|
| 195 | `generator.kvar` 104, `generator.kw` 91 (`controls:gendispatcher/*`) | **RP1.4** (decks stay capi-only, artifact = a `PROPS_015X` allowlist row) + **RP2.4** offline: `RP24_OUT_OF_SCOPE` row `("generator.kvar", 1.55e-06, 102)`; r4133 does not register `GenDispatcher.weights` at all |
| 136 | `autotrans.wdgcurrents`/`.tap`/`.taps` and `regcontrol.tapnum`, 34 each | **RP3.12** — `RP312_UPSTREAM_BUG`, cause `regcontrol-autotrans-typecast`, pin `autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans` (`RegControl.pas:1026/:1296/:1479`, `AutoTrans.pas:88`) |
| 72 | `load.kva`, `vsource.puz*`/`isc3`, `capacitor.*amps`, `line.b*` (`modes:makeposseq/*`) | **RP3.9** — `DECLARED_RP39 = (55, 27, 19)`, verdict `PRECISION_ROUNDTRIP`, pins in `RP39_PINS` |
| 68 | `sensor.kvs` 61, `sensor.currents` 5, `.kws`/`.kvars` 1+1 | **`Owner::OutOfScope`** (RP2.1/RP2.3) — `CELL_DISPOSITION` rows, "plan §1.3 / §RP2.1" |
| 18 | `swtcontrol.delay` on `swtcontrol_lock` plus three `IEEE_519` copies | **RP3.1**'s own cause `swtcontrol-delay-not-wired`; the decks are capi-only, so an entry there would be NEVER APPLIED |
| 8 | `storage.kva/kw/kwrated`, `storagecontroller.kwtotal/kwactual` | **RP2.3** pin `storagecontroller_fleet_aggregates_render_the_live_fleet` (plus RP2.4 for `storage.kw`), `RP38_SUPERSEDED` |
| 1 | `storagecontroller.kwneed` | **RP2.4** — `RP24_OUT_OF_SCOPE` row `(…, 4.96e-06, 1)` |
| 1 | `capacitor.cuf` (`[ 4]` vs `[ 4E-006]`) | **G2.5** — `RP24_OUT_OF_SCOPE` row `("capacitor.cuf", 4.00e-06, 1)` |
| 5 | `line.r1/x1/rmatrix/xmatrix`, `isource.bus1` | **`DECLARED_OUT_OF_SCOPE`** scope arm only, `(221, 21, 0)` — the only five whose owner is a scope arm rather than a cause; flagged, not owed by RP4.1 |
| **504** | **22 cases / 47 pairs** | **total, fully accounted** |

**Second plan-text deviation, recorded verbatim (coordinator decision 2):**
"**RP1.4 staged no ledger entry** (its artifact is a `PROPS_015X` allowlist row;
decks stay capi-only). Correct the phrase in BOTH copies (plan §RP4.1 and the
tripwire doc) in P2." Both copies are corrected: RP4.1 landed **eight** entries,
not "eight plus RP1.4's".

*The re-mask alarm.* `population.lock.json` fingerprints manifest flags and
per-case ledger tags but **no scheduler code**, so re-masking the request would
be invisible to it. RP4.1 therefore adds
`props_norm::assert_r4133_props_compare_ran()`, invoked once from the corpus
gate's epilogue beside the two per-row liveness guards and self-silencing under
`DSS_GATE_ONLY`; it fails when the r4133 property compare never ran, and the
epilogue also prints the live figure. **Deviation from the plan's letter,
deliberately:** the plan spelled the quantity as `visits > 0` summed over the two
tables, but those statics are moved by sibling unit tests in the same binary that
drive the comparator themselves — the sum was already > 0 through the whole
masked era and would stay green under a re-mask — so the counters are bumped at
the single gating call site (`compare_all_properties`'s `PropsChannel::R4133`
arm) and the census walk is deliberately not counted. Both directions are pinned
offline over injected counters. Together with the landed entries (which would go
NEVER APPLIED in `assert_all_hit`) a wholesale re-mask is loud in two independent
ways. The two per-row guards `assert_norm_rows_are_live` /
`assert_echo_rows_are_live`, dormant since RP2.1, are now live and silent.

*Lock.* `population.lock.json` regenerated in the same commit: **8 lines**, all
in `family_rigor`, all moving only the `ledger=` component of the eight entries'
cases. The `props=` component did not move on any line — it is the *manifest*
flag, and RP4.1 changed the scheduler, not a manifest.

*Gate.* All five commands green in both lanes, each exit code read individually:
**4 382 passed / 0 failed / 5 ignored / 0 filtered out** per lane over 74 test
binaries (+92 on the RP3.12 settlement's 4 290 — four new
`harness::props_norm::tests` compiled into each of the 22 harness-linking
binaries, plus `props_r4133_replay`'s own four; `props_r4133_pins` stays at 54,
0 entries and 0 pins being born here), the same five pre-existing `ignored`, no
`#[ignore]` and no name filter. `corpus_gate` **135** per lane over the full
523-case population, ledger **53 entries / 1 534 hits**, none stale, zero
NEVER-APPLIED, zero reds on either channel. Wall clock for `corpus_gate`:
**141.78 s** default / **142.94 s** parity against **142.90 s / 139.68 s** before
the unmask — the flip costs nothing measurable (±2 % run-to-run) against a
budgeted +1–3 min, even though the run now walks 1 670 r4133 property tables.
`lane_diff` was **run to completion** although no product line moved (so it was
not owed): `VERDICT: PASS`, 523 cases / 3 220 861 records / 4 825 419 compared
values, **`max |Δ| = 0.000e0` and `max rel = 0.000e0` on every one of the eight
kinds** (`conv`, `cur`, `errs`, `iter`, `loss`, `pow`, `v`, `y`), zero iteration
drifts — the 2026-07-31 bit-identical default↔parity baseline reproduced
unchanged. The known overlapping-guard snapshot race again dropped untracked
`Test/AutoTrans/*.txt` (fifth sighting; §"Standing open follow-ups"), removed by
exact name; no tracked corpus or golden file moved.

*As-executed corrections to the plan's cited lines* (reported, never edited into
the historical text): `scheduler.rs:357-363` → `:359-365`; `:710-717` →
`:717-721`; `:97-114` → `:97-116`; `harness/mod.rs:1452-1458` → `:3043-3049`
(the cited range is now unrelated power/loss code); `capture.rs:619-664` → fn
`:630-662`, doc `:618-628`; the population figures "96 r4133-only / 366 both /
462 gating" → the lock reads **97 / 367 / 464** (523 cases, 80 `large`, 396
non-large r4133-gating) and must be re-derived, not transcribed.

*Open follow-up (recorded, not RP4.1 work).* `DECLARED_RP35` still declares four
pairs — `line.units` (RP3.5), `line.linecode` (RP3.6), `relay.normal` and
`relay.state` — whose HEAD census shows no divergent cell either, but a
superseded row owes a per-pair live disposition, a cited r4133 getter arm and a
pin, and nobody has produced that trio for them.

*Audit settlement (2026-09-03, RP4.1 — ten findings from the two auditors, four
minor + six notes; audit-code's partial-re-mask note and audit-tests' T3 are the
same issue, so nine dispositions: eight fixed, one recorded, every one settled
against evidence rather than plausibility).*

* **The coverage headline was overstated by 54 cases (audit-code 1) — FIXED.**
  "The **367 `both`** cases get their r4133 property table compared" counted the
  whole `engines: both` population, but `force_properties` keeps the plan's
  `!kind.starts_with("large")` cost guard, and all 79 live `kind=large*` decks are
  `solvable_now`. Re-derived from `population.lock.json`: 523 cases → 519 live →
  **440 forced** = **313** `both` + **83** r4133-only + 44 capi-only, which is
  also exactly the 440 the claims census walks and the only reading consistent
  with the measured 1 670 walks. The three copies of the sentence (this record,
  the frontier note, the plan's as-executed part 2) now say **313 non-`large`
  `both`**, and the **54** `kind=large*` `both` cases that have no property
  compare on *either* channel are recorded as an open item for the RP5.2 closing
  record. The figures are no longer prose: `corpus_gate::scheduler::
  the_property_forcing_rule_is_every_live_non_large_case` walks the four
  manifests without an oracle, asserts the forced set **is** the live non-`large`
  population case for case, and pins the split as
  `FORCED_PROPS_POPULATION = (440, 313, 83, 44)`.
* **That new test is also the missing PARTIAL-re-mask alarm (audit-code note /
  audit-tests 3) — FIXED.** `assert_r4133_props_compare_ran` is a boolean
  (`walks > 0`), so re-adding `gates_capi()` to `force_properties` — which would
  drop all 83 r4133-only cases while the 313 `both` ones keep ~1 300 walks —
  passes it; what caught that until now was `assert_all_hit` on the six landed
  entries whose cases are `engines: "r4133"`, an *implicit* coupling that would
  vanish with those entries. The scheduler test fails on any re-mask, partial
  included, without depending on the ledger; the coupling is now stated in the
  global guard's doc, and the guard is invoked **first** in the gate epilogue so
  "nothing ran at all" reports as one line instead of nineteen row messages.
* **The narrowed echo rows had lost their live staleness detector (audit-tests 1)
  — FIXED.** After precondition A a narrowed row counts only cells it *covers*,
  so `visits == hits` by construction and `check_echo_rows_are_live`'s
  `visits > 0 && hits == 0` arm can never fire on one; a narrowed row going dead
  the way `swtcontrol.normal`/`.state` did would have been invisible, because the
  compensating derivation test reads the **frozen** census. The guard grows the
  mirror-image arm: a narrowed row with `visits == 0` in a run whose seam ran at
  all is stale. Sound on this population — the HEAD claims census gives 19 of the
  20 pairs between 4 and 75 160 in-scope `echo-row` cells, and the twentieth
  (`fault.bus2`) is already the sanctioned `ECHO_ROWS_WITH_NO_IN_SCOPE_CELL` row
  — and both directions are pinned offline over injected counters
  (`the_echo_liveness_guard_is_silent_when_every_narrowed_row_was_visited`,
  `the_echo_liveness_guard_fires_on_an_unvisited_narrowed_row`); the old stale-row
  pin moved from `regcontrol.idle` (now narrowed) to the pair-scoped
  `monitor.mode`, so it still tests the arm it names.
* **`ECHO_NARROWED`'s doc claimed a per-citation review nobody performed
  (audit-code 2) — FIXED as a doc correction, the substance verified.** The 66
  spellings are derived mechanically ("the pair's census rows the typed rule does
  not claim"), i.e. exactly the cells that rule refused, so on the measured
  population the narrowed exclusion is cell-for-cell the old pair-scoped one and
  **the narrowing unmasks nothing the flip would not have unmasked anyway**;
  what it buys is prospective — a *new* spelling on one of these 20 pairs is
  compared instead of inheriting the citation. The doc now says that in the
  shipped table (and no longer says "the spellings its citation actually
  explains"), with the same correction in this record. The auditor's stronger
  option — one assertion per narrowed row tying each spelling to its
  `EchoCategory` shape — was weighed and **not** taken: the categories are prose
  citations of a Pascal getter arm, and turning them into a machine-checkable
  shape is an RP5-sized invention, not a settlement; what protects the 20 pairs
  meanwhile is that an unlisted spelling is now COMPARED.
* **STATUS's own commit-size line was wrong (audit-code 3) — FIXED.** `59e521e5`
  is **23 files, +2 576 / −536**, not 22 / +2 305 / −515; the missing file was
  `tools/oracle/README.md`.
* **The census artifacts a reader opens first still report a non-zero
  (audit-code note) — FIXED in the writer.** `claims_summary.json` and
  `claims_unclaimed_pairs.txt` are **per-spelling** and fold a mixed-disposition
  spelling to its weakest verdict, which is why `swtcontrol.delay` still shows
  "UNCLAIMED, 24 in scope" after its two entries landed. Both artifacts now carry
  the reading rule in themselves — a `reading_rule` field and a header note — and
  the module doc says acceptance is read off the lossless `props_census.json`; the
  stale comment "`mixed_disposition_spellings` — 0 on every measured run" is
  corrected to 1, naming the spelling.
* **`force_properties`' family arm has no `large` guard, and `Row::in_scope` is
  `engines`-only (audit-code notes) — FIXED as assertions/doc.** The family arm's
  premise ("no family deck is `kind=large*`") is now asserted by the new
  scheduler test instead of being inert-by-luck, and `Row::in_scope`'s doc states
  that the gate additionally excludes `kind=large*` and that the two predicates
  coincide only because the census walks the forced population.
* **`RP3_LEDGERED` credited a spelling no landed entry names (audit-tests 2) —
  FIXED.** The interception retires rows per PAIR while an entry pins one exact
  `(rust, oracle)`, so the frozen row `swtcontrol.delay '0' vs '120'` counted as
  ledgered although both entries pin `rust: "0.25"`. Measured at HEAD: its 6 cells
  sit on the three `Version8/Distrib/Examples/**/IEEE_519.DSS` decks, all
  `engines: capi_v0145`, **0 in scope** — no entry is owed, but the accounting now
  says so out loud. New `RP3_LEDGERED_UNNAMED` names the spelling with that
  measurement and the same `SwtControl.pas:195-218` / `:310` citation the entries
  carry, and the guard test checks both ways: an unnamed spelling that is not
  listed fails, and a listed row that stops being unnamed fails.
* **`CLAIMED_TOTAL_LIVE` / `CLAIMED_SPELLINGS_LIVE` enforce only their offline
  half (audit-tests 4) — RECORDED, not fixed.** The identity `cargo test` checks
  is `CLAIMED_NORMALIZATION + LIVE_ONLY_SPELLINGS.len()`; the "and the live census
  measures the same" half rests on an opt-in `DSS_PROPS_CENSUS=claims` run. That
  is deliberate — the mandatory gate must not require an oracle census — and the
  numbers were re-measured at HEAD (21 `ArrayForm` pairs / 201 spellings). The
  limitation is now stated at the constant instead of being implied by its name.

*Gate (settlement).* All five commands green in both lanes on the settled tree —
**4 427 passed / 0 failed / 5 ignored** per lane over 74 binaries (+45 against
RP4.1's 4 382: the two new offline echo pins compile into every
harness-including binary, plus the scheduler test), `corpus_gate` **523/523**
unfiltered in both lanes (147.5 s default / 142.2 s parity) with `assert_all_hit`
green over an untouched `ledger.json`. Nothing product-side moved in the
settlement either (test harness, census writer and docs only), so `lane_diff` was
not owed a second time. The known overlapping-guard snapshot race dropped five
untracked `Test/AutoTrans/*.txt` again (sixth sighting); removed by exact name,
no tracked corpus or golden file moved.

