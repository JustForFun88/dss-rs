//! **The r4133 property replay-accounting test** (`R4133_PROPS_PLAN.md` §1.2,
//! sub-step RP2.1) — the completeness proof that precedes the RP4.1 unmask, and
//! the anti-rot guard afterwards.
//!
//! # What it does
//!
//! It reads the vendored census evidence at `tests/corpus/props_r4133/` —
//! `examples_full.txt` (3 378 rows, the frozen 2026-08-08 census) **plus**
//! `examples_supplement.txt` (76 rows: the 24 pairs WP-RP1's shape closures made
//! live, the pair the 2026-08-08 walk missed, and the pair a `SKIP_PROPS` row
//! hid) — and pushes **every example row**, i.e. every distinct `(rust, r4133)`
//! spelling of every census pair, through the r4133 policy in the documented
//! chain order:
//!
//! ```text
//! shape allowlist  ->  normalization  ->  echo table  ->  display floor
//!  PROPS_015X          PROPS_NORM_R4133   PROPS_ECHO_R4133   RP2.4's floor
//!  (RP1)               (RP2.1 + RP2.2)    (RP2.3)            (absent, RP2.4)
//! ```
//!
//! Every row ends up in exactly one of two states, and **the accounting is
//! total from day one**:
//!
//! * **claimed** — the first matching link of the chain recognises the two
//!   spellings as one value (RP2.1's own deliverable, bins 1/2/4, plus RP2.2's
//!   `EnumSynonym` rows for bin 3);
//! * **declared pending** — no link claims it *yet*, and the row carries a
//!   marker naming the sub-step whose mechanism will claim it (RP2.3, RP2.4,
//!   RP3, or one of the RP3.5+ sub-steps RP2.2's dossier opened) or the reason
//!   nothing will (`OutOfScope`, plan §1.3). RP2.2's own bucket is empty since
//!   that sub-step closed — see [`RP22_ROUTING`] and [`DECLARED_RP22`].
//!
//! Later sub-steps therefore *move* rows between mechanisms; they can never
//! invent a row, and a pair that silently vanishes or appears breaks a count
//! lock here.
//!
//! # The three things it proves
//!
//! 1. **Per-example-row completeness.** Bins 1/2/4 are fully claimed, and every
//!    other row has a named owner. (The contract is per-example-row offline,
//!    per-cell live: cell-level closure is RP4.1's, through the census knob's
//!    disposition mode.)
//! 2. **Liveness both ways, offline** (plan mechanic (d)). Every one of
//!    `PROPS_NORM_R4133`'s [`NORM_ROWS`] rows claims at least one example row —
//!    a row that folds nothing exempts a spelling difference that is not there.
//!    The shape half is exercised against the full `shape.txt`, capi-only
//!    classes included; only the rows **this plan** added to the shared
//!    `PROPS_015X` are in that accounting (the pre-existing 0.15.x rows answer
//!    to `props_roundtrip`/goldens).
//! 3. **Evidence integrity of the tables** (plan mechanic (a)). Every
//!    `PROPS_NORM_R4133` row's citation triple *(pair, bin, cells)* is read back
//!    against the file it cites — `bins.tsv` for the frozen census, the vendored
//!    `README.md` §"Pairs the WP-RP1 shape closures make live" for the pairs
//!    RP1.1-RP1.4 created.
//!
//! # Why the pair's `bins.tsv` bin is NOT the admissibility rule
//!
//! A pair's bin is the classification of its **first census row**, not of its
//! cells: 17 structural pairs are bin-heterogeneous (15 in scope) and bin 1
//! holds nine pairs whose r4133 side is an echo in some or all cells — both
//! measured, both locked in `props_r4133_evidence_lock.rs`, both documented in
//! the vendored `README.md`. So admissibility is decided **per example row**:
//! `load.yearly` is a bin-5 pair with case-only cells, `relay.switchedobj` a
//! bin-2 pair with `''` echoes, and a mixed bin-1 pair legitimately holds a
//! `BoolFold` claim *and*, later, an echo row. Single-claim is per example row,
//! never per pair.
//!
//! # No oracle, no solve, no feature flag
//!
//! The whole test is a walk over vendored text plus the shipped tables, so it is
//! green in both lanes and needs neither dss-python nor the r4133 DLL.

mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use harness::props_norm::{
    self, Evidence as CiteSrc, NORM_ROWS, NormRow, NormRule, PROPS_ECHO_R4133, PROPS_NORM_R4133,
};
use harness::{PROPS_015X, prop_015x};

/// The vendored evidence directory, repo-root-relative.
const DIR: &str = "tests/corpus/props_r4133";

/// The one file in [`DIR`] that carries `#` comment lines — see [`data_rows`].
const SUPPLEMENT: &str = "examples_supplement.txt";

/// Where RP2.3's expected-value pins live, repo-root-relative — the file
/// [`every_echo_row_pin_is_a_test_that_exists`] reads back against
/// `PROPS_ECHO_R4133`'s witness column.
const PINS: &str = "crates/dss-core/tests/props_r4133_pins.rs";

// ---------------------------------------------------------------------------
// Count locks (`props_roundtrip.rs:62-68,238` pattern — equalities, both ways).
// Every number below is a measurement over the vendored files; moving one
// belongs in the commit that argues for the new population.
// ---------------------------------------------------------------------------

/// `examples_full.txt` data rows — the frozen census's spelling inventory.
const ROWS_FROZEN: usize = 3378;
/// `examples_supplement.txt` data rows (RP2.1 part C).
const ROWS_SUPPLEMENT: usize = 76;
/// Pairs the supplement carries, all absent from `bins.tsv` by construction.
const SUPPLEMENT_PAIRS: usize = 26;
/// Cells behind those rows (the supplement's own `count` column).
const SUPPLEMENT_CELLS: usize = 4541;

/// Example rows the normalization claims, per rule kind.
const CLAIMED_BOOL_FOLD: usize = 114;
/// …**528** since RP2.3: RP2.1's 442, **−2** when RP2.2 re-typed
/// `invcontrol.voltage_curvex_ref` as an `EnumSynonym`
/// (`harness/props_norm.rs::VOLTAGE_CURVEX_REF_SYNONYMS`), **+88** for RP2.3's
/// three off-bin `CaseFold` rows — `load.yearly` 72, `reactor.bus2` 14,
/// `invcontrol.monvoltagecalc` 2. Those 88 spellings were RP2.3's own bucket
/// before: an echo row would have masked them, and a typed rule compares them
/// instead (props_norm module doc §"RP2.3's nine off-bin rows").
const CLAIMED_CASE_FOLD: usize = 528;
/// …**203**: RP2.1's 192 **+11** for RP2.3's six off-bin `ArrayForm` rows —
/// `line.wires` 2, `load.zipv` 2, `generator.userdata` 2, `storage.dynadata` 1,
/// `storagecontroller.seasontargets`/`seasontargetslow` 2 each.
const CLAIMED_ARRAY_FORM: usize = 203;
/// RP2.2's five `EnumSynonym` rows: `vsource.scantype`/`sequence` one spelling
/// each, `isource.scantype`/`sequence` two each, and
/// `invcontrol.voltage_curvex_ref` three (`examples_full.txt`).
const CLAIMED_ENUM_SYNONYM: usize = 9;
/// The four rule kinds together — the NORMALIZATION link's own total, which is
/// what [`CLAIMED_TOTAL_LIVE`] reconciles against. RP2.1 measured 748, RP2.2
/// 755, RP2.3 854 (+99, the nine off-bin rows).
const CLAIMED_NORMALIZATION: usize = 854;
/// **RP2.3's echo table** — example rows the exclusion claims, i.e. the ones
/// `PROPS_ECHO_R4133` covers that no earlier link took. 450 (the RP2.3 bucket)
/// − 99 (claimed by the nine new normalization rows instead) − 181 (the five
/// `SilentReadOnly` pairs re-routed to [`Owner::Rp38`]) − 1 (the audit
/// settlement's carve-out, [`ECHO_CARVE_OUT_ROUTING`]) = **169**.
const CLAIMED_ECHO: usize = 169;
/// …and in total, over all four links.
const CLAIMED_TOTAL: usize = CLAIMED_NORMALIZATION + CLAIMED_ECHO;

/// **What the LIVE population claims, against what this evidence base can see.**
///
/// The two accountings are not the same number and must not be pretended to be:
/// the RP2.1 full-population claims census measured **749** claimed spellings on
/// the r4133 channel (vendored `README.md` §"What the r4133 policy claims
/// today") against the 748 of the day. The difference is
/// [`LIVE_ONLY_SPELLINGS`] —
/// spellings that exist live but that **no vendored file may carry**, because
/// they sit on a pair that already has a `bins.tsv` row (the supplement's own
/// lock forbids a pair in both files, `props_r4133_evidence_lock.rs`
/// `the_supplement_carries_only_pairs_no_frozen_row_can`).
///
/// Recorded as a term with its own assertions rather than left as drift: the
/// replay's count locks are locks on the vendored files, so without this the two
/// populations could diverge silently while the module doc kept claiming
/// "spelling-level completeness before the unmask" (RP2.1 audit round).
///
/// **855 since RP2.3** (756 + 99): the nine off-bin normalization rows claim
/// exactly the same 99 spellings live as offline — RP2.3 part A re-measured
/// each pair's live spelling inventory against the frozen one and they match
/// pair for pair (6 446 live cells, 6 370 in scope). The term itself is
/// unchanged, still the one `autotrans.conn` spelling.
const CLAIMED_TOTAL_LIVE: usize = 855;

/// The spellings the live claims census sees and the vendored evidence cannot,
/// each with the pair it belongs to, the two sides, and why no file carries it.
///
/// One entry today: RP1.2's `XfmrCode` port added a deck that reads the
/// AutoTrans **series** winding, whose `Conn` getter is the third arm of the
/// same `CASE` the `CaseFold` trailing-blank rows cite —
/// `2: Result := 'Series';`
/// (`Version8/Source/PDElements/AutoTrans.pas:1820`, next to `'wye '` at `:1818`
/// and `'Delta '` at `:1819`; winding 1 is always `SERIES`, `:620`). So the
/// pre-existing bin-2 pair `autotrans.conn` grew a third spelling. Its
/// `bins.tsv` row is frozen at the two the 2026-08-08 walk saw
/// (`'wye'`/`'wye '`, `'delta'`/`'Delta '`),
/// and §"Pairs the WP-RP1 shape closures make live" records only pairs a closure
/// **created** — a new spelling on an existing pair has no home in either.
const LIVE_ONLY_SPELLINGS: &[(&str, &str, &str, &str)] = &[(
    "autotrans.conn",
    "series",
    "Series",
    "RP1.2's XfmrCode deck; the pair's frozen bins.tsv row predates it",
)];

/// Example rows claimed by the chain's remaining two links, both still zero and
/// each for a *different* reason — see [`Link`]. (`CLAIMED_ECHO` moved up to
/// the normalization block, where its derivation lives.)
const CLAIMED_SHAPE_ALLOWLIST: usize = 0;
const CLAIMED_DISPLAY_FLOOR: usize = 0;

/// Rows for which **more than one** link matches — **135 since RP2.3**, over 20
/// pairs that hold a `PROPS_NORM_R4133` row AND a `PROPS_ECHO_R4133` row.
///
/// It was zero while the echo table was empty, and RP2.1 made it a lock rather
/// than a structural assert precisely so RP2.3 would have to re-read the
/// first-match argument instead of silently starting to rely on it. Re-read,
/// and it holds: normalization precedes the exclusion
/// ([`first_match_returns_the_earliest_link`]), so on each of those 20 pairs
/// the typed rule claims its foldable spellings and only the rest is credited
/// to the echo row. The split is measured per pair by
/// [`the_typed_rules_and_the_echo_rows_split_their_shared_pairs_offline`], whose
/// doc is also where the difference between this attribution and the live
/// (pair-scoped) mask is spelled out.
///
/// The 20: the four mixed bin-1 pairs (`recloser.eventlog` 1,
/// `regcontrol.idle` 1, `relay.distreverse` 2, `relay.reset` 1), the six
/// bin-2-labelled ones (`capcontrol.type` 3, `fault.bus2` 1,
/// `invcontrol.mode` 7, `fuse.switchedobj` 4, `recloser.switchedobj` 6,
/// `relay.switchedobj` 8), `energymeter.peakcurrent` 2, and the nine RP2.3
/// off-bin rows (`load.yearly` 72, `reactor.bus2` 14,
/// `invcontrol.monvoltagecalc` 2, `line.wires` 2, `load.zipv` 2,
/// `generator.userdata` 2, `storage.dynadata` 1,
/// `storagecontroller.seasontargets`/`seasontargetslow` 2 each).
const MULTI_LINK_ROWS: usize = 135;

/// Declared-pending rows per owner: `(rows, pairs, rows on in-scope pairs)`.
/// This is what RP2.3, RP2.4, RP3 and the RP3.5+ sub-steps each inherit.
///
/// **`DECLARED_RP22` is `(0, 0, 0)` since RP2.2 closed**, and that zero is the
/// sub-step's own acceptance ("the replay accounting claims bin 3 fully"): every
/// pair it was handed is either claimed by the shipped table or carries a
/// verdict in [`RP22_ROUTING`], so nothing is left pending on RP2.2. The variant
/// stays so a regression that re-creates the bucket fails here by name.
const DECLARED_RP22: (usize, usize, usize) = (0, 0, 0);
/// **`DECLARED_RP23` is `(0, 0, 0)` since RP2.3 closed**, and that zero is the
/// sub-step's own acceptance. It inherited `(450, 86, 449)` — RP2.1's 295 rows
/// plus the 155 RP2.2 routed to it — and every one of those 450 rows is now
/// resolved, in exactly three ways, each of which the locks above count:
///
/// * **99** claimed by the nine off-bin `PROPS_NORM_R4133` rows RP2.3 landed
///   first, so that the census attributes them to their rule and each row can
///   prove itself live ([`CLAIMED_CASE_FOLD`], [`CLAIMED_ARRAY_FORM`]);
/// * **169** claimed by `PROPS_ECHO_R4133`'s 81 cited rows ([`CLAIMED_ECHO`]);
/// * **181** re-routed to [`Owner::Rp38`] by the kill ruling ([`RP38_ROUTING`]);
/// * **1** carved back out of its row and declared to RP2.4 by the audit
///   settlement ([`ECHO_CARVE_OUT_ROUTING`]).
///
/// The variant stays so a regression that re-creates the bucket fails here by
/// name.
const DECLARED_RP23: (usize, usize, usize) = (0, 0, 0);
/// **RP3.8 — the five `SilentReadOnly` pairs the RP2.3 kill criterion fired
/// on**: 181 example rows over 5 pairs, all of them on pairs the RP4.1 unmask
/// will compare (1 064 live cells, 772 in scope). See [`RP38_ROUTING`].
const DECLARED_RP38: (usize, usize, usize) = (181, 5, 181);
/// **RP2.4 — the display floor's bucket.** `(2101, 71, 2021)` since the RP2.3
/// audit settlement: 2 100 rows over 70 pairs from RP2.1's own bin-6 walk, plus
/// the one cell [`ECHO_CARVE_OUT_ROUTING`] takes out of `reactor.kvar`'s echo
/// row — two live values 5.0e-06 apart, i.e. this bucket's own class (its
/// sibling `reactor.kv`, the other output of the same r4133 `MakePosSequence`
/// round-trip, is already here at 5.85e-06). The third number counts rows on
/// pairs the RP4.1 unmask compares at all, and `reactor.kvar` is such a pair
/// even though this particular cell sits on a capi-only case.
const DECLARED_RP24: (usize, usize, usize) = (2101, 71, 2021);
const DECLARED_RP3: (usize, usize, usize) = (7, 4, 7);
/// The three sub-steps RP2.2 opened: **8 rows over 6 pairs, 5 of them in
/// scope** — RP3.5 `line.units` (1 row, 0 in scope), RP3.6 `line.linecode`
/// (2 rows, both in scope — the only RP3.5+ pair the RP4.1 unmask will actually
/// compare), RP3.7 `swtcontrol.normal`/`state` (1 + 2 rows, all in scope; their
/// one-token spellings are claimed by `ArrayForm` and are not here) and
/// `relay.normal`/`state` (1 row each, both out of scope). RP4.1 does not start
/// until all three close (plan §0).
const DECLARED_RP35: (usize, usize, usize) = (8, 6, 5);
/// `OutOfScope` rows must have **zero** in-scope cells — that is the whole
/// claim the marker makes (plan §1.3).
const DECLARED_OUT_OF_SCOPE: (usize, usize, usize) = (134, 18, 0);

/// `shape.txt` gap names a `PROPS_015X` row this plan added covers:
/// `generator.rneut`, `generator.xneut` and `sensor.action` (RP1.1),
/// `autotrans.xfmrcode` (RP1.2) and `gendispatcher.weights` (RP1.4) — the last
/// being the only one that fires on the **r4133** channel, the other four on the
/// 0.14.5 capture.
const SHAPE_ALLOWLIST_NAMES: usize = 5;
/// …and the ones closed by a real port instead (RP1.3's WindGen surface).
const SHAPE_PORTED_NAMES: usize = 2;

/// The `PROPS_015X` rows that are **active on the r4133 channel**: the property
/// is on the Rust side only, so the r4133 name list cannot carry it and
/// `prop_015x` drops it before any value is compared. Exactly one exists
/// (`GenDispatcher.weights`, the r4133 registration bug of RP1.4) — the rest of
/// this plan's rows are `oracle_only` gaps, i.e. inert on r4133 and active on
/// the 0.14.5 capture.
const R4133_ACTIVE_ALLOWLIST_NAMES: usize = 1;

// ---------------------------------------------------------------------------
// The plan's own enumerated pair lists (plan §1.1 bin 7, §RP2.2). Each list is
// transcribed from the plan and cross-checked against the vendored files below,
// so a stale plan list fails a test instead of silently widening a marker.
// ---------------------------------------------------------------------------

/// Plan §1.1 bin 7: "12 of the 16 are echo (frozen defaults)". RP2.3's table is
/// their landing site.
const BIN7_ECHO: &[&str] = &[
    "fault.pctperm",
    "gictransformer.pctperm",
    "invcontrol.lpftau",
    "invcontrol.risefalllimit",
    "pvsystem.%pminkvarmax",
    "pvsystem.%pminnovars",
    "reactor.kvar",
    "regcontrol.remoteptratio",
    "storage.%pminkvarmax",
    "storage.%pminnovars",
    "transformer.pctperm",
    "transformer.repair",
];

/// Plan §1.1 bin 7: "4 need root-cause" — WP-RP3's four sub-steps.
const BIN7_ROOT_CAUSE: &[&str] = &[
    "generator.model",
    "gictransformer.r2",
    "swtcontrol.delay",
    "windgen.kvar",
];

/// Bin-7 pairs the **supplement** carries, each read off the Pascal as RP2.3's
/// row rather than an RP3 root-cause sub-step:
///
/// * `generator.d` — **not an echo**, which is what RP2.3's kill criterion
///   caught (part A finding F1; the ruling's R2, user-approved 2026-08-23).
///   Reading the whole class settles it: `Create` sets `GenVars.D := 1.0`
///   (`Version8/Source/PCElements/generator.pas:969`) but the property's field
///   is `GenVars.Dpu` — what `Edit` writes (`:669`) and what
///   `InitPropertyValues` snapshots (`Format('%-g', [GenVars.Dpu])`, `:2585`) —
///   and `Create` **never** touches `Dpu`. So the frozen `'0'` IS r4133's own
///   live value, not a stale store, and `InitStateVars` then runs dynamics at
///   `D := Dpu*kVA*1000/w0 = 0` (`:2710`) against the property's documented
///   default ("Default is 1.0", `:467`) — an upstream initialisation bug.
///   dss_capi 0.14.5 fixed it (`Dpu := 1.0`,
///   `src/PCElements/Generator.pas:1006`) and the port follows, so the pair
///   lands as `EchoCategory::LiveSemanticsDiffer` plus the expected-value pin
///   `generator_d_renders_the_documented_damping_default`
///   (`crates/dss-core/tests/props_r4133_pins.rs`), never as a silent
///   `EchoDefault`;
/// * `autotrans.pctperm` / `autotrans.repair` — `InitPropertyValues` freezes
///   `'100'` / `'36'` (`Version8/Source/PDElements/AutoTrans.pas:1958-1959`)
///   and the `GetPropertyValue` override re-renders only PD-tail slots 1 and 2
///   (`:1885-1888`), so slots 4 and 5 fall through to the `PropertyValue[]`
///   store (`General/DSSObject.pas:112-115`);
/// * `regcontrol.revthreshold` — `RegControl.pas:820-827` overrides only TapNum
///   and `:1448` freezes `PropertyValue[23] := '100'`, the sibling of
///   `remoteptratio` (`:1452`), measured at 888 cells by RP2.1 part A.
const BIN7_ECHO_SUPPLEMENT: &[&str] = &[
    "autotrans.pctperm",
    "autotrans.repair",
    "generator.d",
    "regcontrol.revthreshold",
];

/// **RP2.2's closed pair list, as it was handed to RP2.2** (plan §RP2.2): the
/// S6 singletons the triage flagged for individual source investigation, next to
/// the eight bin-3 pairs (derived from `bins.tsv` and cross-checked in
/// [`the_plan_pair_lists_still_describe_the_vendored_evidence`]). A pair on this
/// list was RP2.2's **whatever bin its cells fall in**: RP2.2 read its getter
/// and routed it (an `EnumSynonym` row, an RP2.3 echo row, or a new RP3.5+
/// sub-step), which is exactly the "later sub-steps move rows between
/// mechanisms" the accounting is built for.
///
/// **RP2.2 is done, and this list is now the record of its input**, not a
/// pending marker: every pair below is either claimed by the shipped table or
/// carries a verdict in [`RP22_ROUTING`], and
/// [`rp22_settled_every_pair_it_was_handed`] proves that with no third state —
/// so `Owner::Rp22`'s bucket is empty ([`DECLARED_RP22`]).
///
/// **Four of these pairs carry an RP2.1 `ArrayForm` row**, and RP2.2 read their
/// getters anyway (disclosure, RP2.1 audit round — `harness/props_norm.rs`
/// §"Four `ArrayForm` rows sit on RP2.2's S6 list"): `invcontrol.monbus` /
/// `monbusesvbase`, where every census spelling folds (bracketed vs bare, equal
/// token counts) and RP2.2 therefore found nothing left to route, and
/// `swtcontrol.normal` / `state`, where only the one-token spelling folds — 1
/// cell of 59 each, while the 58/31/27-cell per-phase renders are refused by the
/// token-count rule and are routed to RP3.7 below. A row on this list was
/// therefore never proof that RP2.1 left the pair alone; it was proof that the
/// pair's *unclaimed* cells were RP2.2's.
const RP22_S6: &[&str] = &[
    "expcontrol.derlist",
    "invcontrol.monbus",
    "invcontrol.monbusesvbase",
    "invcontrol.monvoltagecalc",
    "invcontrol.pvsystemlist",
    "invcontrol.vsetpoint",
    "isource.bus2",
    "isource.yearly",
    "line.linecode",
    "line.spacing",
    "line.units",
    "load.yearly",
    "reactor.bus2",
    "relay.normal",
    "relay.state",
    "swtcontrol.normal",
    "swtcontrol.state",
];

/// **RP2.2's verdicts — one row per pair it had to route, with the r4133 site
/// that decides it** (plan §RP2.2: "read the r4133 getter/echo site and classify
/// into exactly one of three"). This table is the dossier, condensed to the form
/// the accounting can execute: it is consulted *first* in [`declare`], so a pair
/// here lands in its verdict's bucket instead of the blanket "RP2.2 owes it".
///
/// The three outcomes of plan §RP2.2 map onto this table as follows.
///
/// * **`EnumSynonym` row** — not here at all: those five pairs
///   (`vsource`/`isource` × `scantype`/`sequence`, plus
///   `invcontrol.voltage_curvex_ref`) are *claimed* by `PROPS_NORM_R4133` and
///   never reach [`declare`]. Same for `invcontrol.monbus`/`monbusesvbase`,
///   fully claimed by RP2.1's `ArrayForm` rows.
///   [`rp22_settled_every_pair_it_was_handed`] is what makes those two silences
///   legible instead of a gap.
/// * **echo → [`Owner::Rp23`]** — r4133's string is a `PropertyValue[]` echo
///   (no `GetPropertyValue` arm for the index, `Version8/Source/General/
///   DSSObject.pas:112-115`), or both sides render live but the r4133 render is
///   an upstream quirk whose root cause is fully established and whose port
///   answer is proven right. RP2.3 lands the cited exclusion row (plus the
///   expected-value pin where no capi witness exists).
/// * **suspected divergence → [`Owner::Rp35`]** — the two engines' live state or
///   modelling differs, so a numbered sub-step must decide it before RP4.1.
///
/// Every row's `cite` names the r4133 `Version8/Source` site the verdict was
/// read from; the full dossier prose is STATUS §WP-RP2's RP2.2 record.
const RP22_ROUTING: &[(&str, Owner, &str)] = &[
    // --- bin 3, the four pairs that are NOT enum synonyms --------------------
    // `'close'` vs `'open'` (12 cells of 34) is a stale STORE, not a state
    // disagreement: `TSwtControlObj.GetPropertyValue` has arms for 1,2,4..9 and
    // none for 3 (`Controls/SwtControl.pas:573-620`), so `Action` echoes
    // `PropertyValue[3]` — written with the raw token UNCONDITIONALLY, before
    // the CASE (`:192-193`), and then not acted on because `Locked` makes
    // `InterpretSwitchState` exit (`:417` — the guard fires because the
    // property name starts with `'a'`). Both engines refuse the write
    // (`elements/control/swt_control/accessors.rs:146-150`), verified live on
    // the r4133 DLL (RP2.2 audit settlement, 2026-08-23: `action=open` under
    // `lock=yes` moves neither `state` nor `normal`).
    //
    // The live-state agreement was originally argued from the pair's twin
    // `swtcontrol.state` matching token for token on all 59 cells. That twin is
    // a **weaker** witness than it reads: r4133's `get_States` re-reads the
    // controlled element (`:514-530`) while the port's STATE accessor returns
    // its tracked field and says so (`accessors.rs:129-132`). What actually
    // settles it is the probe above plus the green corpus gate on the switch
    // decks; the census agreement is corroboration, not proof.
    // NOT the plan's suspected RP3.5 candidate.
    (
        "swtcontrol.action",
        Owner::Rp23,
        "SwtControl.pas:573-620 (no arm 3) / :192-193 / :652 / :417",
    ),
    // `'17'` vs `'1 16 +'`: not a "decomposition render" (the plan's and the
    // vendored triage's guess) — it is the deck's own RPN SOURCE TEXT,
    // `mode=(1 16 +)`, echoed back with the parser's parens stripped.
    // `Meters/Monitor.pas` has no `GetPropertyValue` override at all; `:359`
    // stores the raw `Param` and `:1843` defaults `PropertyValue[3] := '0'`.
    // Decks: `Test/Dynamic_Kundur.dss:55-56`,
    // `Version8/Distrib/Examples/Dynamic_Expressions/Dynamic_KundurDynExp.dss:66-67`.
    (
        "monitor.mode",
        Owner::Rp23,
        "Monitor.pas: no GetPropertyValue override / :359 / :1843",
    ),
    // Both engines hold `MODESCHEDULE`: `InterpretMode` maps `'s'` + `'c'` to it
    // (`Controls/StorageController.pas:2322-2333`) but `GetModeString`'s
    // `propMODEDISCHARGE` arm lists only FOLLOW/LOADSHAPE/SUPPORT/TIME/
    // PEAKSHAVE/I-PEAKSHAVE and falls to `ELSE Result := 'UNKNOWN'`
    // (`:1200-1214`). Explicitly NOT an `EnumSynonym`: `'UNKNOWN'` is the
    // catch-all for every unnamed mode, so the mapping would not be injective —
    // an exclusion plus a pin (`LiveSemanticsDiffer`), never a rule row.
    (
        "storagecontroller.modedischarge",
        Owner::Rp23,
        "StorageController.pas:1200-1214 vs :2322-2333",
    ),
    // **RP3.5.** r4133 renders index 20 LIVE — `LineUnitsStr(LengthUnits)`
    // (`PDElements/Line.pas:1404`) — so `'none'` vs `'kft'` means the two
    // engines hold different `LengthUnits`. r4133 re-applies the saved units
    // AFTER the impedance edit (`MergeWith` saves at `:1627`, re-edits at
    // `:1721-1726` and `:1791-1796`; `MakePosSequence` re-appends `Units=` at
    // `:1596` "to compensate for unexpected reset"); the port's matrix-series
    // branch does the two in the opposite order (`exec/reduce.rs:396-409` writes
    // `length_units` and then runs the RMATRIX/XMATRIX/CMATRIX side effects,
    // which call `reset_length_units`). Second divergence in the same routine:
    // the port's `reset_length_units` clears `user_length_units`
    // (`elements/pd/line/code.rs:24-28`), which r4133 deliberately preserves
    // (`Line.pas:2330`, "but do not erase FUserLengthUnits").
    (
        "line.units",
        Owner::Rp35,
        "RP3.5 — Line.pas:1404 / :1627 / :1721-1726 / :1791-1796 / :2326-2331",
    ),
    // --- the S6 singletons ---------------------------------------------------
    // **RP3.6.** r4133 renders index 3 live (`If FLineCodeSpecified Then Result
    // := CondCode`, `Line.pas:1357`) and its `switch=yes` arm (`:694-700`)
    // assigns r1/x1/r0/x0/c1/c0/len as fields, kills geometry and spacing and
    // resets the length units — but leaves `FLineCodeSpecified` TRUE. The port
    // calls `kill_line_code_specified()` there
    // (`elements/pd/line/accessors.rs:488-511`). Not cosmetic: the flag selects
    // the `FUnitsConvert` formula on a later `units=` (`Line.pas:626-627`), and
    // these decks put `units=m` AFTER `Switch=True`. 5 cells, all in scope.
    (
        "line.linecode",
        Owner::Rp35,
        "RP3.6 — Line.pas:1357 / :413 / :685 / :691 / :694-700 / :626-627",
    ),
    // Index 21 has no getter arm (`Line.pas:1347-1429` covers 1..20, 23, 26..33
    // and the PD tail) → `DSSObject.pas:112-115` echoes `PropertyValue[21]`,
    // default `''` (`:1511`), overwritten with the deck's `'sp'`.
    // `SpacingSpecified` is killed later but the echoed string never moves.
    (
        "line.spacing",
        Owner::Rp23,
        "Line.pas:1347-1429 (no arm 21) / :1511 / :2266-2276",
    ),
    // No `GetPropertyValue` in `Isource.pas` at all; `InitPropertyValues` sets
    // `PropertyValue[11] := ''` (`:631`) and only an explicit `bus2=` would
    // overwrite it. None of these decks sets one, so r4133 echoes `''` while the
    // port renders the derived grounded-wye terminal 2. Already an accepted
    // upstream report: `investigations/to_opendss/14-isource-bus2-not-stored.md`.
    ("isource.bus2", Owner::Rp23, "Isource.pas:631 (+ report 14)"),
    // Same echo (`PropertyValue[8] := ''`, `Isource.pas:628`), and the port's
    // live value IS r4133's live value: the `daily=` arm aliases the OBJECT
    // without touching the string — `IF YearlyShapeObj=Nil THEN YearlyShapeObj
    // := DailyShapeObj` (`:286`) — and `CalcYearlyMult` (`:672-681`) consumes
    // the object. The port reports the aliased object's name.
    ("isource.yearly", Owner::Rp23, "Isource.pas:628 / :286"),
    // NOT an echo — `TLoadObj.GetPropertyValue` index 7 answers the live string
    // field `Yearlyshape` (`PCElements/Load.pas:2346`). r4133 prints the RAW
    // user-typed name (case preserved, `''` when never typed, `:807`); the port
    // prints the resolved shape object's lowercased name, and the `''` half is
    // the same daily→yearly object aliasing as `isource.yearly` (`:657`).
    // A `CaseFold` row alone is not SUFFICIENT for the pair: it claims only the
    // case-only half and leaves the 18 832 + 7 145 empty-vs-value cells
    // unclaimed, so RP2.3 owes a value-only exclusion covering those.
    // **RP2.3 landed both** — the `CaseFold` row for the 5 995 cells a typed
    // rule can compare value-preservingly, and a `LiveSemanticsDiffer` echo row
    // (plus its pin) for the rest. The chain order keeps them apart, cell by
    // cell (`props_norm` module doc §"RP2.3's nine off-bin rows").
    ("load.yearly", Owner::Rp23, "Load.pas:2346 / :657 / :807"),
    // `TReactorObj.GetPropertyValue` has arms for 10, 11, 13..16, 19 and the PD
    // tail — not 2 (`PDElements/Reactor.pas:1087-1105`) → echo. Two writers: the
    // raw `bus2=` token (`:386`) and, for the shunt case, a SNAPSHOT of the
    // derived bus2 taken while `bus1=` is parsed (`:419-420`,
    // `PropertyValue[2] := GetBus(2)` after `ReactorSetbus1`, `:341-358`). A
    // later `phases=` resizes the terminal but never refreshes the snapshot —
    // exactly the `'b2.0'` vs `'b2.0.0.0'` shape. The live terminals agree.
    (
        "reactor.bus2",
        Owner::Rp23,
        "Reactor.pas:1087-1105 (no arm 2) / :386 / :419-420 / :341-358",
    ),
    // Indices 25/32/33 have no getter arm
    // (`Controls/InvControl.pas:3226-3285` covers 1, 4..18, 21, 23, 24, 28, 34)
    // and no `InitPropertyValues` entry (`:2806-2839` initialises 1..24 and 28),
    // so each holds `''` until a deck writes it. `monvoltagecalc` is the mixed
    // one — `''` on 239 cells and the raw token `'MAX'`/`'AVG'` on 15 — and one
    // row covers both halves.
    (
        "invcontrol.monvoltagecalc",
        Owner::Rp23,
        "InvControl.pas:505 (no arm 25, not in :2806-2839)",
    ),
    // …and the decks drive the DER list through property 1, whose getter IS live
    // (`:3233` → `ReturnElementsList`), which is why that pair does not diverge.
    (
        "invcontrol.pvsystemlist",
        Owner::Rp23,
        "InvControl.pas:512 (no arm 32, not in :2806-2839)",
    ),
    (
        "invcontrol.vsetpoint",
        Owner::Rp23,
        "InvControl.pas:513 (no arm 33, not in :2806-2839)",
    ),
    // Both sides render live and r4133 renders the WRONG list:
    // `TExpControlObj.GetPropertyValue` answers `ReturnElementsList` for BOTH
    // index 1 (`PVSystemList`) and index 14 (`DERList`)
    // (`Controls/ExpControl.pas:684` and `:696`), and `ReturnElementsList`
    // (`:702-715`) is hard-wired to `FPVSystemNameList` — the class-prefix-
    // stripped list. The Edit arms keep the two lists distinct on purpose
    // (`:227-234` fills one and derives the other, `:247-252` the mirror), and
    // `expcontrol.pvsystemlist` is NOT a divergent pair, which confirms the two
    // engines agree wherever the bare list is the right answer. Port is right;
    // `LiveSemanticsDiffer` + pin, and an upstream-report candidate.
    (
        "expcontrol.derlist",
        Owner::Rp23,
        "ExpControl.pas:696 + :702-715 vs :227-234 / :247-252",
    ),
    // **RP3.7 (a).** r4133 keeps per-phase switch state: `FPresentState` /
    // `FNormalState : pStateArray` (`Controls/SwtControl.pas:37-38`), allocated
    // per phase (`:299-305`), settable phase-by-phase from a quoted list
    // (`:453-480`) or ganged from a bare token (`:433-451`), each phase driving
    // its own conductor (`:532-549`), and the getter renders one token per
    // CONTROLLED-ELEMENT phase (`:589-599` Normal, `:600-610` State). The port
    // holds a single scalar per field, applied to the whole terminal
    // (`elements/control/swt_control/accessors.rs:126-163`, `:270-276`). Every
    // observed r4133 render is homogeneous, so no VALUE differs today — a deck
    // writing `state=(open, closed, closed)` would diverge in Y.
    //
    // **Second defect on the same pair, found by the RP2.2 audit (2026-08-23)
    // and confirmed by a live r4133 probe — RP3.7(a) owns it too.** The port
    // refuses a `normal=` write while `Locked` (`accessors.rs:151-155`, its
    // side effect `:244-249`, pinned by `swt_control/tests.rs::
    // locked_ignores_normal_and_state_writes`), following 0.14.5's
    // `ConditionalReadOnly` flag (`.inputs/dss_capi/src/Controls/
    // SwtControl.pas:159-160`). r4133 applies it: `InterpretSwitchState`'s
    // guard is property-name-conditional — `if Locked and ((LowerCase(
    // property_name[1]) = 'a') or (… = 's')) Then Exit` under the comment
    // "Only allowed to change normal state if locked" (`:416-417`) — and
    // property 6 is `'Normal'` (`:128`), so arm 6 (`:201-204`) reaches
    // `set_NormalStates` (`:556-561`), which has no lock guard. Probed on the
    // vendored r4133 DLL: with `lock=yes`, `normal=open` moves `Normal` to
    // `[open, open, open, ]` while `state=`/`action=` leave both fields
    // untouched. The port's own Relay gets this rule right and documents it
    // (`elements/control/relay/accessors.rs:416-420`), so it is a port bug, not
    // a decision — but it is an ENGINE fix, out of RP2.2's zero-product-bytes
    // scope, and no census cell exposes it (no corpus deck writes `normal=`
    // under lock). Recorded here, in plan §RP3.7 and in STATUS rather than left
    // to RP4.1's residual triage.
    (
        "swtcontrol.normal",
        Owner::Rp35,
        "RP3.7(a) — SwtControl.pas:589-599 / :37-38 / :299-305 / :433-480 / :532-549",
    ),
    (
        "swtcontrol.state",
        Owner::Rp35,
        "RP3.7(a) — SwtControl.pas:600-610 (same set)",
    ),
    // **RP3.7 (b), the mirror image**: here the PORT renders three tokens and
    // r4133 one. `TRelayObj.GetPropertyValue` 39/40 loops the LIVE
    // `ControlledElement.NPhases` (`Controls/Relay.pas:1407-1428`); the port has
    // a per-phase array (hence `[closed, open, open, ]`) but does not resync it
    // to the controlled element after `MakePosSequence`. The single cell is
    // `modes/makeposseq/makeposseq_ctrl.dss:44`, `engines: "capi_v0145"`.
    (
        "relay.normal",
        Owner::Rp35,
        "RP3.7(b) — Relay.pas:1407-1417 (loops ControlledElement.NPhases)",
    ),
    ("relay.state", Owner::Rp35, "RP3.7(b) — Relay.pas:1418-1428"),
    // --- the three pairs beyond the closed list ------------------------------
    // Closing bin 3 means closing its CELLS, not only its eight pairs: the
    // vendored `README.md` §"A pair's bin is a label, not a per-cell
    // classification" names three bin-2-labelled pairs that carry enum-spelling
    // cells, and the accounting routes those cells here. RP2.2 read all three
    // getters; the third, `invcontrol.voltage_curvex_ref`, is a LIVE enum
    // rendering (`Controls/InvControl.pas:3244-3249`) and is therefore CLAIMED
    // by `PROPS_NORM_R4133`'s fifth `EnumSynonym` row, not routed. See
    // [`RP22_BEYOND_THE_CLOSED_LIST`].
    //
    // `TCapControlObj` has NO `GetPropertyValue` override at all, so `type`
    // (`PropertyName^[4]`, `Controls/CapControl.pas:178`) echoes
    // `PropertyValue[4]` — the deck's raw token, written before the CASE that
    // dispatches on `lowercase(param)[1]` (`:304-311`). Hence `'pf'` and
    // `'volt'` against our registry name `'PowerFactor'`/`'Voltage'`
    // (`obj/dss_enum/registry/control.rs:69`). The pair's three case-only
    // spellings stay claimed by its RP2.1 `CaseFold` row — normalization
    // precedes the echo table, so only these 30 cells are credited to RP2.3's
    // row (credited: the shipped exclusion is pair-scoped).
    (
        "capcontrol.type",
        Owner::Rp23,
        "CapControl.pas: no GetPropertyValue override / :178 / :304-311",
    ),
    // The same derived-bus2 snapshot as `reactor.bus2`: `TFaultObj`'s getter has
    // only arm 6 (`PDElements/Fault.pas:695-718`), so index 2 echoes
    // `PropertyValue[2]` — seeded at `:672` and re-snapshotted from `GetBus(2)`
    // whenever `bus1=` is parsed (`:297`), never refreshed by a later
    // `phases=`. Its one cell is out of scope, and the pair's `'b3.0'`/`'B3.0'`
    // spelling stays with the `CaseFold` row.
    (
        "fault.bus2",
        Owner::Rp23,
        "Fault.pas:695-718 (no arm 2) / :297 / :672",
    ),
];

/// The three pairs RP2.2 had to route that its plan-given closed list does not
/// name: a **bin-2 label** with genuine bin-3 cells inside it (vendored
/// `README.md` §"A pair's bin is a label, not a per-cell classification", whose
/// consequence 1 says in as many words that "`capcontrol.type` /
/// `invcontrol.voltage_curvex_ref` / `reactor.bus2` are case-or-empty pairs
/// carrying enum-spelling cells"). `reactor.bus2` was already on the S6 list;
/// these are the rest, plus `fault.bus2` from the same README table.
///
/// Recorded as a named list rather than silently folded into [`RP22_ROUTING`]
/// because it is the one place RP2.2's scope grew beyond the plan's enumeration,
/// and the growth is a *measurement* (the accounting put four example rows in
/// RP2.2's bucket that the pair list did not predict), not a decision.
const RP22_BEYOND_THE_CLOSED_LIST: &[&str] = &[
    "capcontrol.type",
    "fault.bus2",
    "invcontrol.voltage_curvex_ref",
];

/// The residue the bin map cannot decide: example rows whose **own** cell
/// classification (bin 2 or 4) differs from the mechanism that will claim them,
/// on a pair the plan's lists do not already own. One entry per pair, each with
/// the evidence that decides it.
///
/// **Three of the seven are consumed since RP2.3** — `energymeter.peakcurrent`,
/// `generator.dynout` and `generator.userdata` are now claimed by the chain
/// (an echo row, and for the last one a `PROPS_NORM_R4133` `ArrayForm` row
/// beside it), so [`declare`] never reaches their entries. They are kept as the
/// record of the disposition that put them there, exactly as [`RP22_S6`] is
/// kept as the record of RP2.2's input; the four `sensor.*` entries are still
/// live. Every entry's pair is checked against the evidence by
/// [`the_plan_pair_lists_still_describe_the_vendored_evidence`].
const CELL_DISPOSITION: &[(&str, Owner, &str)] = &[
    // `'[ 400]'` vs `'((400, 400, 400))'`: a one-element array against r4133's
    // frozen three-element default. r4133 answers `'(' + PropertyValue[7] + ')'`
    // (`Version8/Source/Meters/EnergyMeter.pas:2637-2664`) over the
    // never-refreshed `'(400, 400, 400)'` of `:2209`, so it is an `EchoDefault`
    // — while the pair's OTHER spellings are honest arrays that `ArrayForm`
    // claims. Measured by RP2.1 part B; the pair needs BOTH rows.
    (
        "energymeter.peakcurrent",
        Owner::Rp23,
        "EnergyMeter.pas:2637-2664 / :2209",
    ),
    // `'[speed, theta]'` vs `'[speed,dpshaft,]'`, 2 cells, both on the
    // `Dynamic_KundurDynExp` decks (plus the 271-cell `''`-vs-`'[]'` unset-array
    // spelling of the same pair). **Not an echo and not a spelling**: r4133's
    // `SetDynOutput` stores the *variable* index `Get_Out_Idx` returns
    // (`General/DynamicExp.pas:411-437`, an index into `FVarNames`), while
    // `GetDynOutputStr` renders it through `Get_VarName`
    // (`:441-465`), which decodes its argument as a flat *(variable, derivative
    // slot)* index — the encoding `Get_DynamicEqVal` uses, `DynSlot =
    // array[0..1] of double` (`Shared/Arraydef.pas:39`). For this deck's
    // `varnames=[Speed Mass PShaft Pterm Damp theta]`, `theta` (index 5)
    // therefore prints as `d` + `FVarNames[2]` = `dpshaft`. The dynamics are
    // unaffected (`generator.pas:2823-2849` indexes `DynamicEqVals` with the
    // same variable index the port uses), so this is an r4133 **rendering** bug
    // and the port's answer is the correct one.
    //
    // **RP2.2's verdict** (it was declared here for exactly this triage, so that
    // an RP2.3 echo row could not mask it silently): the root cause is fully
    // established and the port's answer proven right, so this is not an RP3.5+
    // sub-step — it is RP2.3's, as a **tagged `LiveSemanticsDiffer` row with
    // this citation plus an expected-value pin**, never a silent `EchoDefault`.
    // Upstream report written:
    // `investigations/to_opendss/41-dynout-readback-renders-wrong-variable.md`
    // (gitignored, local-only).
    (
        "generator.dynout",
        Owner::Rp23,
        "PCElement.pas:197-243 / DynamicExp.pas:411-465 (RP2.2: LiveSemanticsDiffer + pin)",
    ),
    // `'rs=… option=fixed'` vs `'(rs=… option=fixed)'`: r4133 wraps the stored
    // user-model data string in parens. Same string, same `PropertyValue[]`
    // store as the pair's `''`-vs-`'()'` rows — one echo row covers the pair.
    ("generator.userdata", Owner::Rp23, "README §RP1.1, bin 5"),
    // The Sensor arrays: `'[ 0]'` (one element) against r4133's three. A real
    // value delta, **out of scope** — all 61 Sensor cells sit on
    // `engines: "capi_v0145"` cases (plan §1.3, §RP2.1's own note on
    // `sensor.kvs`), so the r4133 compare never reaches them.
    ("sensor.currents", Owner::OutOfScope, "plan §1.3 / §RP2.1"),
    ("sensor.kvars", Owner::OutOfScope, "plan §1.3 / §RP2.1"),
    ("sensor.kvs", Owner::OutOfScope, "plan §1.3 / §RP2.1"),
    ("sensor.kws", Owner::OutOfScope, "plan §1.3 / §RP2.1"),
];

/// **RP2.3's kill-criterion re-route — the five pairs that get NO echo row.**
///
/// The RP2.3 criterion ("any candidate row that cannot be cited to a concrete
/// r4133 echo site — stop and report") fired on these five, and the report was
/// ruled and user-approved on 2026-08-23: they are not echoes and not a
/// spelling difference. r4133's getter arm is LIVE and prints a computed
/// read-only quantity; the port prints `''` only because dss_capi 0.14.5 flags
/// the property `[SilentReadOnly, ReadByFunction]` and the port reproduces that
/// suppression deliberately (`elements/pc/ind_mach012/accessors.rs:203-212`,
/// `elements/control/storage_controller/accessors.rs:162-167`).
///
/// Under the standing 2026-08-02 policy the 0.14.5 convention yields: the
/// engine renders the live value and the resulting **capi-side** divergence is
/// excluded field-by-field and pinned. That is an engine change, so it becomes
/// plan §RP3.8 and these 181 example rows are declared to [`Owner::Rp38`] —
/// loudly, with a count lock ([`DECLARED_RP38`]), never as a silent leftover
/// inside RP2.3's bucket. Giving them an `EchoCategory` instead would have been
/// exactly the mislabel the kill criterion exists to prevent.
///
/// Each row cites the r4133 live getter arm it renders from.
const RP38_ROUTING: &[(&str, &str)] = &[
    (
        "indmach012.pf",
        "IndMach012.pas:1790 (arm 5: Format('%.6g',[PowerFactor(Power[1,ActiveActor])]))",
    ),
    (
        "storagecontroller.kwhtotal",
        "StorageController.pas:991 (GetkWhTotal)",
    ),
    (
        "storagecontroller.kwtotal",
        "StorageController.pas:992 (GetkWTotal)",
    ),
    (
        "storagecontroller.kwhactual",
        "StorageController.pas:993 (GetkWhActual)",
    ),
    (
        "storagecontroller.kwactual",
        "StorageController.pas:994 (GetkWActual)",
    ),
];

/// **Who owns the cells an echo row carves out** (`props_norm::
/// ECHO_CARVE_OUTS`) — the RP2.3 audit settlement's narrowing valve, accounted.
///
/// A carve-out means "this cell of a cited pair is NOT that row's echo", so the
/// example row it matches leaves the echo link and needs an owner like any other
/// unclaimed row. It cannot get one from the pair's own evidence: `declare`
/// reads `bins.tsv`'s PAIR bin, and `reactor.kvar`'s bin-7 label comes from the
/// 0.917-rel echo cells, which would route the carved cell straight back to
/// RP2.3. So the routing is explicit, cited, and matched against the shipped
/// carve-outs both ways
/// ([`the_carve_outs_are_routed_and_only_they_are`]).
///
/// `reactor.kvar` `'66.6666666666667'` vs `'66.667'`: r4133's own
/// `MakePosSequence` round-trips the value through `Format(' kvar=%-.5g')` and
/// the parser (`Version8/Source/PDElements/Reactor.pas:1145-1201`), so both
/// sides are LIVE and differ by 5.0e-06 — the display class RP2.4 derives its
/// floor for, exactly where the same round-trip's `reactor.kv` already sits
/// (`bins.tsv`: numeric bin 6, `max_rel` 5.85e-06).
const ECHO_CARVE_OUT_ROUTING: &[(&str, &str, &str, Owner, &str)] = &[(
    "reactor.kvar",
    "66.6666666666667",
    "66.667",
    Owner::Rp24,
    "Reactor.pas:1145-1201 (MakePosSequence -> Format(' kvar=%-.5g') -> Parser/Edit): both sides \
     live, 5.0e-06 apart — RP2.4's display class, like reactor.kv (bins.tsv bin 6, 5.85e-06)",
)];

/// The `shape.txt` gap names closed by a real port rather than by a
/// `PROPS_015X` row — RP1.3's WindGen `UserModel`/`UserData` surface. The port's
/// live proof is RP1.3's unit pins plus the r4133 channel after RP4.1; this file
/// only pins that no allowlist row is doing that job.
const SHAPE_PORTED: &[(&str, &str)] = &[("windgen", "usermodel"), ("windgen", "userdata")];

/// The eleven Delphi boolean spellings r4133's own getters print (vendored
/// `README.md` §"Bin 1 carries nine echo pairs, not three"). Anything else on a
/// bin-1 pair is a `PropertyValue[]` echo, not a boolean.
const BOOL_SPELLINGS: &[&str] = &[
    "true", "True", "false", "False", "YES", "yes", "no", "NO", "n", "y", "Y",
];

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// Which file an example row came from. Supplement rows count exactly like
/// frozen ones in every assertion here (plan §RP2.1: "the both-ways liveness
/// assert counts supplement rows exactly like `examples_full.txt` rows").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Frozen,
    Supplement,
}

/// One example row: a distinct `(rust, r4133)` spelling of one census pair.
#[derive(Debug, Clone)]
struct Example {
    pair: String,
    class: String,
    prop: String,
    rust: String,
    r4133: String,
    cells: usize,
    src: Source,
}

/// **The claim chain, in the documented order.** Declared as an ordered enum so
/// "first match wins" is a property of the type, not of a hand-written `if`
/// ladder: [`first_match`] takes the four verdicts and returns the earliest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Link {
    /// **RP1's shape allowlist.** A property the oracle's own name list cannot
    /// carry is dropped before any value is compared (`prop_015x`,
    /// `harness/mod.rs`). On the r4133 channel exactly one such row exists —
    /// `GenDispatcher.weights` — and the census recorded that gap as shape rows,
    /// never as value cells, which is why this link legitimately claims **zero**
    /// example rows while still being exercised against `shape.txt`.
    ShapeAllowlist,
    /// **RP2.1's normalization** (`PROPS_NORM_R4133`) — the value-preserving
    /// re-spelling this sub-step ships.
    Normalization,
    /// **RP2.3's echo table** (`PROPS_ECHO_R4133`) — the exclusion. 81 cited
    /// rows, consulted only after the normalization link has had its chance, so
    /// on a mixed pair a typed rule *claims* the foldable spellings first and
    /// this link is credited with what is left ([`MULTI_LINK_ROWS`]).
    ///
    /// **This accounting is per spelling; the shipped exclusion is per pair.**
    /// The two are the same statement only for the cells the census recorded: at
    /// the live seam a cell of a cited pair that no rule folds is masked all the
    /// same (`harness::props_norm::PROPS_ECHO_R4133`'s doc, RP2.3 audit
    /// settlement). One measured cell is genuinely out of its row —
    /// [`ECHO_CARVE_OUT_ROUTING`] — and this link answers `false` for it.
    Echo,
    /// **RP2.4's display floor** — a named slot that carries no value in RP2.1
    /// (`props_norm::display_floor()` is `None`, i.e. numeric tokens compare
    /// exactly). RP2.1 introduces no tolerance anywhere.
    DisplayFloor,
}

impl Link {
    /// The chain, in order. Used by [`first_match`] and pinned by
    /// [`first_match_returns_the_earliest_link`].
    const ORDER: [Link; 4] = [
        Link::ShapeAllowlist,
        Link::Normalization,
        Link::Echo,
        Link::DisplayFloor,
    ];

    fn tag(self) -> &'static str {
        match self {
            Link::ShapeAllowlist => "shape allowlist",
            Link::Normalization => "normalization",
            Link::Echo => "echo table",
            Link::DisplayFloor => "display floor",
        }
    }
}

/// The sub-step whose mechanism is expected to claim a row that RP2.1 cannot —
/// or the reason none will.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    /// RP2.2 — enum synonyms and the S6 dossier (bin 3 + the enumerated
    /// singletons + any cell that needs a per-pair source reading).
    Rp22,
    /// RP2.3 — the echo-exclusion table (bin 5, the bin-1 echo halves, the
    /// echo-rooted bin-7 pairs). **Empty since RP2.3 closed**
    /// ([`DECLARED_RP23`]).
    Rp23,
    /// RP2.4 — the r4133 props display floor (bin 6).
    Rp24,
    /// RP3 — the four genuine value jumps that are not echo.
    Rp3,
    /// **RP3.5+ — the sub-steps RP2.2's dossier opened** (plan §RP2.2's third
    /// outcome, §0: RP4.1 does not start until they close). Three of them, all
    /// live-state or modelling divergences rather than spellings:
    /// RP3.5 `line.units`, RP3.6 `line.linecode`, RP3.7 the per-phase
    /// switch/relay state. Which pair belongs to which sub-step is recorded in
    /// [`RP22_ROUTING`]'s citation column; this owner is the accounting bucket
    /// they share.
    Rp35,
    /// **RP3.8 — the sub-step RP2.3's kill criterion opened** (the ruling of
    /// 2026-08-23, user-approved). Five `SilentReadOnly` pairs whose divergence
    /// is neither an echo nor a spelling: r4133 renders a live computed
    /// read-only quantity and the port renders `''` only because dss_capi
    /// 0.14.5 flags the property `[SilentReadOnly, ReadByFunction]`. Under the
    /// standing 2026-08-02 policy (r4133 is the behavioral authority; 0.14.5 is
    /// a numeric oracle only) the engine must render the live value and the
    /// resulting capi-side divergence is excluded + pinned THERE — an engine
    /// change, forbidden inside RP2.3's zero-product-bytes scope. See
    /// [`RP38_ROUTING`]; RP4.1 does not start until it closes (plan §0).
    Rp38,
    /// Nothing will claim it: every cell of the pair sits on an
    /// `engines: "capi_v0145"` case, which plan §1.3 keeps uncompared on r4133.
    OutOfScope,
}

impl Owner {
    fn tag(self) -> &'static str {
        match self {
            Owner::Rp22 => "RP2.2",
            Owner::Rp23 => "RP2.3",
            Owner::Rp24 => "RP2.4",
            Owner::Rp3 => "RP3",
            Owner::Rp35 => "RP3.5+ (opened by RP2.2)",
            Owner::Rp38 => "RP3.8 (opened by RP2.3's kill criterion)",
            Owner::OutOfScope => "out of scope (§1.3)",
        }
    }
}

/// What the vendored evidence says about the pair an example row belongs to.
#[derive(Debug, Clone)]
struct PairEvidence {
    /// `structural` or `numeric` — the census's own kind column.
    numeric: bool,
    /// The §1.1 bin (1-7).
    bin: u8,
    /// Cells behind the pair, full census.
    cells: usize,
    /// Cells on r4133-gating cases, when the evidence records the split.
    cells_in_scope: Option<usize>,
    /// `max_rel` over the in-scope cells, for numeric pairs that have one.
    max_rel_in_scope: Option<f64>,
    /// Where this record comes from, for failure messages.
    origin: String,
}

impl PairEvidence {
    /// A pair with at least one cell the RP4.1 unmask will actually compare.
    /// `None` (no recorded split) means "not measured separately" and is read as
    /// in scope — the conservative direction: the row then needs an owner.
    fn in_scope(&self) -> bool {
        self.cells_in_scope.is_none_or(|n| n > 0)
    }

    /// The bin the RP4.1 unmask will actually face for a numeric pair: the
    /// in-scope re-derivation (`max_rel_in_scope >= 1e-4`), which moves four
    /// full-census bin-7 pairs into the display class (vendored `README.md`,
    /// last data trap). Structural pairs answer their own bin.
    fn effective_bin(&self) -> u8 {
        match (self.numeric, self.max_rel_in_scope) {
            (true, Some(m)) => {
                if m >= 1e-4 {
                    7
                } else {
                    6
                }
            }
            _ => self.bin,
        }
    }
}

/// What one owner inherits.
#[derive(Debug, Default)]
struct Bucket {
    rows: usize,
    pairs: BTreeSet<String>,
    /// Rows whose pair has at least one cell the RP4.1 unmask will compare.
    in_scope_rows: usize,
}

/// The whole accounting of one run.
#[derive(Debug, Default)]
struct Ledger {
    /// Claimed rows per link, and — for the normalization link — per rule kind.
    claimed: BTreeMap<&'static str, usize>,
    /// Rows a `PROPS_NORM_R4133` row claimed, indexed like the table.
    norm_hits: Vec<usize>,
    /// Declared rows per owner.
    declared: BTreeMap<Owner, Bucket>,
    /// Rows for which more than one link matched.
    multi_link: usize,
    /// Rows no link claimed and no rule could declare — **the kill criterion**.
    unaccounted: Vec<String>,
}

impl Ledger {
    fn claimed_total(&self) -> usize {
        self.claimed.values().sum()
    }

    fn declared_total(&self) -> usize {
        self.declared.values().map(|b| b.rows).sum()
    }

    /// `(rows, pairs, rows on in-scope pairs)` for one owner.
    fn owner(&self, o: Owner) -> (usize, usize, usize) {
        self.declared
            .get(&o)
            .map(|b| (b.rows, b.pairs.len(), b.in_scope_rows))
            .unwrap_or((0, 0, 0))
    }
}

// ---------------------------------------------------------------------------
// Reading the vendored evidence
// ---------------------------------------------------------------------------

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn read(name: &str) -> String {
    let path = repo_root().join(DIR).join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Non-empty lines with the CR of a CRLF file stripped; the header line is
/// asserted and dropped when `header` is `Some`.
///
/// `#` lines are dropped for [`SUPPLEMENT`] only — its provenance header is the
/// bin assignment of the pairs it holds. The frozen extracts carry no comment,
/// and keeping the filter off them means a `#` line inserted into one is a data
/// row here too, i.e. loud (`props_r4133_evidence_lock.rs` makes the same split
/// for the same reason — RP2.1 audit round).
fn data_rows(name: &str, header: Option<&str>) -> Vec<String> {
    let comments_allowed = name == SUPPLEMENT;
    let text = read(name);
    let mut rows: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .filter(|l| !l.is_empty() && !(comments_allowed && l.starts_with('#')))
        .collect();
    if let Some(want) = header {
        let got = rows.first().cloned().unwrap_or_default();
        assert_eq!(got, want, "{name}: unexpected header line");
        rows.remove(0);
    }
    rows
}

/// Parse one `class.prop | 'left' | 'right' | tail` row with the quote anchors
/// the vendored README prescribes — values may contain `|`, never `'`.
fn parse_row(name: &str, line: &str) -> (String, String, String, String) {
    fn bad(name: &str, line: &str) -> ! {
        panic!("{name}: unparsable row {line:?}")
    }
    let (pair, rest) = line.split_once(" | ").unwrap_or_else(|| bad(name, line));
    let rest = rest.strip_prefix('\'').unwrap_or_else(|| bad(name, line));
    let (left, rest) = rest.split_once('\'').unwrap_or_else(|| bad(name, line));
    let rest = rest.strip_prefix(" | '").unwrap_or_else(|| bad(name, line));
    let (right, rest) = rest.split_once('\'').unwrap_or_else(|| bad(name, line));
    let tail = rest.strip_prefix(" | ").unwrap_or_else(|| bad(name, line));
    (
        pair.to_string(),
        left.to_string(),
        right.to_string(),
        tail.to_string(),
    )
}

/// The example rows of one file.
fn examples(name: &str, src: Source) -> Vec<Example> {
    data_rows(name, Some("class.prop | rust | r4133 | count"))
        .iter()
        .map(|line| {
            let (pair, rust, r4133, tail) = parse_row(name, line);
            let (class, prop) = pair
                .split_once('.')
                .unwrap_or_else(|| panic!("{name}: {pair:?} is not a class.prop pair"));
            let cells: usize = tail
                .trim()
                .parse()
                .unwrap_or_else(|e| panic!("{name}: bad count in {line:?}: {e}"));
            assert!(cells > 0, "{name}: {pair} has a zero-count spelling");
            Example {
                pair: pair.clone(),
                class: class.to_string(),
                prop: prop.to_string(),
                rust,
                r4133,
                cells,
                src,
            }
        })
        .collect()
}

/// `bins.tsv`, as `pair -> [evidence]` (two entries for the one pair that is
/// both structural and numeric).
fn bins_evidence() -> BTreeMap<String, Vec<PairEvidence>> {
    let mut out: BTreeMap<String, Vec<PairEvidence>> = BTreeMap::new();
    for line in data_rows(
        "bins.tsv",
        Some("pair\tkind\tbin\tsubbin\tcells\tcells_in_scope\tmax_rel\tmax_rel_in_scope"),
    ) {
        let f: Vec<&str> = line.split('\t').collect();
        assert_eq!(f.len(), 8, "bins.tsv: {line:?} has {} fields", f.len());
        let cells_in_scope: usize = f[5].parse().expect("cells_in_scope");
        out.entry(f[0].to_string()).or_default().push(PairEvidence {
            numeric: f[1] == "numeric",
            bin: f[2].parse().expect("bin"),
            cells: f[4].parse().expect("cells"),
            cells_in_scope: Some(cells_in_scope),
            max_rel_in_scope: f[7].parse().ok(),
            origin: format!("bins.tsv ({})", f[1]),
        });
    }
    out
}

/// One row of the vendored `README.md` §"Pairs the WP-RP1 shape closures make
/// live" — the only evidence that can exist for a pair a shape gap was hiding.
#[derive(Debug, Clone)]
struct Rp1Record {
    pair: String,
    rust: String,
    r4133: String,
    cells: usize,
    cells_in_scope: Option<usize>,
    bin: u8,
    substep: CiteSrc,
}

/// Strip the markdown a README table cell may carry (`` ` `` and `**`).
fn plain(cell: &str) -> String {
    cell.trim().replace(['`', '*'], "").trim().to_string()
}

/// A README value cell: the markdown backticks off, and the prose spelling of an
/// empty render (`''`) turned back into the empty string the extracts carry.
fn plain_value(cell: &str) -> String {
    let v = cell.trim().trim_matches('`').to_string();
    if v == "''" { String::new() } else { v }
}

/// The leading integer of a README table cell (`44 (9)` -> 44,
/// `**7** (max_rel 7.54e-02)` -> 7).
fn lead_int(cell: &str, what: &str) -> usize {
    let s = plain(cell);
    let digits: String = s
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits
        .parse()
        .unwrap_or_else(|e| panic!("README: bad {what} in {cell:?}: {e}"))
}

/// Parse the four WP-RP1 tables out of the vendored `README.md`.
///
/// The section is prose with one table per sub-step, each introduced by a bold
/// `**RP1.N (…)**` paragraph; a markdown heading ends the section. Parsing it is
/// the point: these 24 records are the *only* evidence for the pairs the shape
/// closures created, and `PROPS_NORM_R4133`'s `Evidence::Rp1*` rows cite them by
/// hand — so the citation is read back here rather than trusted.
fn rp1_records() -> Vec<Rp1Record> {
    let text = read("README.md");
    let mut out = Vec::new();
    let mut current: Option<CiteSrc> = None;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if line.starts_with('#') {
            current = None;
            continue;
        }
        for (marker, step) in [
            ("**RP1.1 (", CiteSrc::Rp11),
            ("**RP1.2 (", CiteSrc::Rp12),
            ("**RP1.3 (", CiteSrc::Rp13),
            ("**RP1.4 (", CiteSrc::Rp14),
        ] {
            if line.starts_with(marker) {
                current = Some(step);
            }
        }
        let Some(step) = current else { continue };
        if !line.starts_with("| `") {
            continue;
        }
        let f: Vec<&str> = line.split('|').collect();
        assert_eq!(
            f.len(),
            7,
            "README: WP-RP1 table row {line:?} has {} fields",
            f.len()
        );
        let cells_cell = plain(f[4]);
        let cells_in_scope = cells_cell
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')'))
            .and_then(|(inner, _)| inner.trim().parse::<usize>().ok());
        out.push(Rp1Record {
            pair: plain(f[1]),
            rust: plain_value(f[2]),
            r4133: plain_value(f[3]),
            cells: lead_int(f[4], "cells"),
            cells_in_scope,
            bin: lead_int(f[5], "bin") as u8,
            substep: step,
        });
    }
    out
}

/// `shape_in_scope.txt`: class -> rows on r4133-gating cases. A WP-RP1 pair whose
/// own README row records no `(in scope)` split inherits its class's answer —
/// the pair became live exactly because that class's shape gap closed.
fn shape_rows_in_scope() -> BTreeMap<String, usize> {
    data_rows("shape_in_scope.txt", None)
        .iter()
        .map(|line| {
            let (class, rest) = line
                .split_once(": ")
                .unwrap_or_else(|| panic!("shape_in_scope.txt: {line:?}"));
            let n = rest
                .split_whitespace()
                .find_map(|t| t.strip_prefix("rows_in_scope="))
                .and_then(|t| t.parse().ok())
                .unwrap_or_else(|| panic!("shape_in_scope.txt: no rows_in_scope in {line:?}"));
            (class.to_string(), n)
        })
        .collect()
}

/// One `shape.txt` class row: the property-table gap the census measured.
#[derive(Debug)]
struct ShapeRow {
    class: String,
    oracle_only: Vec<String>,
    rust_only: Vec<String>,
}

fn shape_rows() -> Vec<ShapeRow> {
    fn names(line: &str, key: &str) -> Vec<String> {
        let rest = line
            .split_once(key)
            .unwrap_or_else(|| panic!("shape.txt: no {key} in {line:?}"))
            .1;
        let inner = rest
            .split_once(']')
            .unwrap_or_else(|| panic!("shape.txt: unterminated {key} in {line:?}"))
            .0
            .trim_start_matches('[');
        inner
            .split(',')
            .map(|t| t.trim().trim_matches('\'').to_string())
            .filter(|t| !t.is_empty())
            .collect()
    }
    data_rows("shape.txt", None)
        .iter()
        .map(|line| ShapeRow {
            class: line
                .split_once(':')
                .unwrap_or_else(|| panic!("shape.txt: {line:?}"))
                .0
                .to_string(),
            oracle_only: names(line, "oracle_only="),
            rust_only: names(line, "rust_only="),
        })
        .collect()
}

/// The `(pair, rust, r4133)` triple `numeric_pairs.txt` prints for a pair — how
/// the one name that is BOTH a structural and a numeric pair (`isource.bus1`)
/// is split between its two `bins.tsv` rows. The README prescribes joining
/// `examples_full.txt` on the triple; this is that join, read from the file
/// instead of hard-coded.
fn numeric_examples() -> BTreeMap<String, (String, String)> {
    data_rows(
        "numeric_pairs.txt",
        Some("class.prop | rust-example | r4133-example | max_rel | rows"),
    )
    .iter()
    .map(|line| {
        let (pair, rust, r4133, _) = parse_row("numeric_pairs.txt", line);
        (pair, (rust, r4133))
    })
    .collect()
}

// ---------------------------------------------------------------------------
// The evidence index: pair -> the record that governs a given example row
// ---------------------------------------------------------------------------

/// Everything the accounting needs to look up.
struct Corpus {
    rows: Vec<Example>,
    bins: BTreeMap<String, Vec<PairEvidence>>,
    supplement: BTreeMap<String, PairEvidence>,
    numeric_example: BTreeMap<String, (String, String)>,
    /// `(class, prop)` pairs the r4133 name list cannot carry, so the shape
    /// allowlist drops them before any value compare.
    r4133_allowlisted: BTreeSet<(String, String)>,
}

impl Corpus {
    fn load() -> Corpus {
        let mut rows = examples("examples_full.txt", Source::Frozen);
        rows.extend(examples("examples_supplement.txt", Source::Supplement));

        let bins = bins_evidence();
        let in_scope = shape_rows_in_scope();
        let mut supplement: BTreeMap<String, PairEvidence> = BTreeMap::new();
        for r in rp1_records() {
            let class = r.pair.split('.').next().unwrap_or_default().to_string();
            let class_rows = *in_scope
                .get(&class)
                .unwrap_or_else(|| panic!("{}: shape_in_scope.txt has no {class} row", r.pair));
            // The README's own `(in scope)` column when it has one; otherwise the
            // class's answer — the pair became live exactly because that class's
            // shape gap closed, so a class with no in-scope element row has no
            // in-scope cell for any of its new pairs either. `None` = "in scope,
            // count not measured separately", which is the conservative reading:
            // the row then needs an owner.
            let cells_in_scope = r
                .cells_in_scope
                .or(if class_rows == 0 { Some(0) } else { None });
            supplement.insert(
                r.pair.clone(),
                PairEvidence {
                    numeric: r.bin >= 6,
                    bin: r.bin,
                    cells: r.cells,
                    cells_in_scope,
                    // The WP-RP1 records carry a per-pair `max_rel`, not an
                    // in-scope re-derivation, so a numeric supplement pair
                    // answers with its recorded bin (`effective_bin`).
                    max_rel_in_scope: None,
                    origin: format!("README §{}", substep_tag(r.substep)),
                },
            );
        }
        // The two pairs no census row can carry, each declared by its own
        // provenance (the supplement's header states both, with the r4133
        // citations). In scope: their sibling `regcontrol.idle` carries 732
        // in-scope cells of 887 — RP2.3 re-derives the exact split with the
        // census knob when it writes the pins.
        for (pair, bin, cells) in [
            ("regcontrol.fwdthreshold", 5u8, 888usize),
            ("regcontrol.revthreshold", 7, 888),
        ] {
            supplement.insert(
                pair.to_string(),
                PairEvidence {
                    numeric: bin >= 6,
                    bin,
                    cells,
                    cells_in_scope: None,
                    max_rel_in_scope: None,
                    origin: "examples_supplement.txt provenance header".to_string(),
                },
            );
        }

        let mut r4133_allowlisted = BTreeSet::new();
        for s in shape_rows() {
            for name in &s.rust_only {
                if prop_015x(PROPS_015X, &s.class, name) {
                    r4133_allowlisted.insert((s.class.clone(), name.clone()));
                }
            }
        }

        Corpus {
            rows,
            bins,
            supplement,
            numeric_example: numeric_examples(),
            r4133_allowlisted,
        }
    }

    /// The evidence record governing one example row. For the single pair that
    /// lives in both kinds, the numeric record is the one whose example the
    /// vendored `numeric_pairs.txt` prints.
    fn evidence(&self, row: &Example) -> Option<&PairEvidence> {
        if let Some(e) = self.supplement.get(&row.pair) {
            return Some(e);
        }
        let all = self.bins.get(&row.pair)?;
        if all.len() == 1 {
            return all.first();
        }
        let numeric = self
            .numeric_example
            .get(&row.pair)
            .is_some_and(|(r, o)| *r == row.rust && *o == row.r4133);
        all.iter().find(|e| e.numeric == numeric).or(all.first())
    }
}

fn substep_tag(s: CiteSrc) -> &'static str {
    match s {
        CiteSrc::BinsTsv => "bins.tsv",
        CiteSrc::Rp11 => "RP1.1",
        CiteSrc::Rp12 => "RP1.2",
        CiteSrc::Rp13 => "RP1.3",
        CiteSrc::Rp14 => "RP1.4",
    }
}

// ---------------------------------------------------------------------------
// The chain
// ---------------------------------------------------------------------------

/// The earliest matching link, given one verdict per link **in chain order**.
///
/// This is the whole of "first match in the chain wins", and it is deliberately
/// a tiny total function over a verdict vector so
/// [`first_match_returns_the_earliest_link`] can prove it is not just "return
/// the last one that said yes".
fn first_match(verdicts: [bool; 4]) -> Option<Link> {
    Link::ORDER
        .iter()
        .zip(verdicts)
        .find(|(_, hit)| *hit)
        .map(|(link, _)| *link)
}

/// Run one example row through all four links and report every verdict.
fn chain_verdicts(corpus: &Corpus, row: &Example) -> [bool; 4] {
    let shape = corpus
        .r4133_allowlisted
        .iter()
        .any(|(c, p)| c.eq_ignore_ascii_case(&row.class) && p.eq_ignore_ascii_case(&row.prop));
    let norm = props_norm::claiming_row(&row.class, &row.prop, &row.rust, &row.r4133).is_some();
    let echo = props_norm::echo_excluded(&row.class, &row.prop, &row.rust, &row.r4133);
    let floor = match (
        props_norm::display_floor(),
        row.rust.parse::<f64>(),
        row.r4133.parse::<f64>(),
    ) {
        (Some(rel), Ok(a), Ok(b)) => (a - b).abs() <= rel * a.abs().max(b.abs()),
        _ => false,
    };
    [shape, norm, echo, floor]
}

/// The README's structural classification chain, applied to **one cell** — the
/// per-example-row admissibility rule (`README.md` §"A pair's bin is a label").
fn classify_cell(rust: &str, r4133: &str) -> u8 {
    if rust == "Yes" || rust == "No" {
        1
    } else if rust.is_empty() || r4133.is_empty() {
        5
    } else if rust.trim().eq_ignore_ascii_case(r4133.trim()) {
        2
    } else if rust.starts_with(['[', '(']) || r4133.starts_with(['[', '(']) {
        4
    } else {
        3
    }
}

/// **The declaration rule.** Which sub-step's mechanism will claim a row RP2.1
/// cannot — decided from the vendored evidence, in this order:
///
/// 0. a pair RP2.3's kill criterion re-routed answers [`RP38_ROUTING`] — this
///    comes first because those five pairs' cells *look* like bin 5 (one side
///    empty) and every later rule would file them as echoes;
/// 0. a pair RP2.2's dossier routed answers [`RP22_ROUTING`] — RP2.3's echo
///    table or one of the RP3.5+ sub-steps; a pair on **RP2.2's closed list**
///    (the eight bin-3 pairs plus the S6 singletons) that the routing does
///    *not* name and no link claimed is an **error**, because RP2.2 is closed
///    and owed exactly those verdicts;
/// 1. a **numeric** row answers its in-scope-effective bin: 6 -> RP2.4's floor,
///    7 -> RP2.3 if the plan (or the vendored README) already read it off the
///    Pascal as an echo, RP3 if it is one of the four root-cause pairs;
/// 2. an unclaimed cell of a **bin-1** pair whose r4133 side is not one of the
///    eleven Delphi boolean spellings is the pair's echo half -> RP2.3;
/// 3. a cell whose own classification is **bin 5** (either side empty) -> RP2.3;
/// 4. …**bin 3** (an enum spelling) -> RP2.2's `EnumSynonym` rows — which, RP2.2
///    being closed, is only reachable for a pair the routing does not name and
///    is therefore the same error as (0);
/// 5. …**bin 2 or 4** inside a pair no rule of this table claims: the explicit
///    [`CELL_DISPOSITION`] entry, else the pair's own bin-5 echo row, else — if
///    every cell of the pair is out of scope — plan §1.3.
///
/// `Err` is **the kill criterion**: a census row in RP2.1's bins that no typed
/// rule claims and no later sub-step owns.
fn declare(row: &Example, ev: &PairEvidence) -> Result<Owner, String> {
    let cell_bin = classify_cell(&row.rust, &row.r4133);
    let disposition = CELL_DISPOSITION
        .iter()
        .find(|(p, _, _)| *p == row.pair)
        .map(|(_, o, _)| *o);

    // A carved-out CELL comes first of all — it is the most specific rule here,
    // and every rule below reads the pair's own bin, which for a carve-out is
    // the label of the very echo cells the carve-out is not. See
    // [`ECHO_CARVE_OUT_ROUTING`].
    if let Some((_, _, _, owner, _)) = ECHO_CARVE_OUT_ROUTING
        .iter()
        .find(|(p, rust, r4133, _, _)| *p == row.pair && *rust == row.rust && *r4133 == row.r4133)
    {
        return Ok(*owner);
    }
    // RP2.3's kill-criterion re-route comes next: these five pairs are neither
    // an echo nor a spelling, and every rule below would have mis-filed them
    // (their cells classify as bin 5 — one side empty — which is the echo
    // table's shape). See [`RP38_ROUTING`].
    if RP38_ROUTING.iter().any(|(p, _)| *p == row.pair) {
        return Ok(Owner::Rp38);
    }
    if let Some((_, owner, _)) = RP22_ROUTING.iter().find(|(p, _, _)| *p == row.pair) {
        return Ok(*owner);
    }
    if RP22_S6.contains(&row.pair.as_str()) || (!ev.numeric && ev.bin == 3) {
        return Err(format!(
            "{} '{}' vs '{}': on RP2.2's closed pair list, unclaimed by the chain, and \
             RP22_ROUTING has no verdict for it — RP2.2 is closed, so every such pair owes \
             exactly one recorded routing",
            row.pair, row.rust, row.r4133
        ));
    }
    if ev.numeric {
        return match ev.effective_bin() {
            6 => Ok(Owner::Rp24),
            7 if BIN7_ECHO.contains(&row.pair.as_str())
                || BIN7_ECHO_SUPPLEMENT.contains(&row.pair.as_str()) =>
            {
                Ok(Owner::Rp23)
            }
            7 if BIN7_ROOT_CAUSE.contains(&row.pair.as_str()) => Ok(Owner::Rp3),
            7 if !ev.in_scope() => Ok(Owner::OutOfScope),
            other => Err(format!(
                "{} '{}' vs '{}': numeric bin {other} on an IN-SCOPE pair that is on none of \
                 the plan's bin-7 lists (echo / root-cause) — plan §1.1's bin-7 enumeration is \
                 stale, or this is a new category",
                row.pair, row.rust, row.r4133
            )),
        };
    }
    if ev.bin == 1 && !BOOL_SPELLINGS.contains(&row.r4133.as_str()) {
        return Ok(Owner::Rp23);
    }
    match cell_bin {
        5 => Ok(Owner::Rp23),
        // A bin-3 CELL inside a pair the routing does not name. RP2.2 is closed,
        // so this is the same error as (0): closing bin 3 means closing its
        // cells, and every one of them owes a read getter and a verdict.
        3 => Err(format!(
            "{} '{}' vs '{}': a bin-3 (enum-spelling) cell on a bin-{} pair that RP22_ROUTING \
             does not name — RP2.2's acceptance is that bin 3 is claimed FULLY, per cell",
            row.pair, row.rust, row.r4133, ev.bin
        )),
        2 | 4 => match (disposition, ev.bin, ev.in_scope()) {
            // `OutOfScope` is a claim about the evidence, not a shrug: it may
            // only be written for a pair the RP4.1 unmask never compares.
            (Some(Owner::OutOfScope), _, true) => Err(format!(
                "{} '{}' vs '{}': dispositioned out of scope, but the pair HAS in-scope cells",
                row.pair, row.rust, row.r4133
            )),
            (Some(o), _, _) => Ok(o),
            (None, 5, _) => Ok(Owner::Rp23),
            (None, _, false) => Ok(Owner::OutOfScope),
            (None, bin, true) => Err(format!(
                "{} '{}' vs '{}': a bin-{cell_bin} cell inside an in-scope bin-{bin} pair that \
                 no normalization row claims and no later sub-step owns",
                row.pair, row.rust, row.r4133
            )),
        },
        other => Err(format!(
            "{} '{}' vs '{}': cell bin {other} is unclaimed — a bin-1 pair whose r4133 side IS a \
             boolean spelling must fold, so BoolFold refusing it is a rule bug",
            row.pair, row.rust, row.r4133
        )),
    }
}

/// Walk every example row through the chain and the declaration rule.
///
/// The claim decision is **always** the shipped seam ([`chain_verdicts`] →
/// `props_norm::claiming_row`), never a copy of the predicate — a drifting copy
/// would make this completeness proof describe a comparator that does not
/// exist. `table` only says which rows the normalization hits are attributed
/// to, which is what lets [`a_normalization_row_that_claims_nothing_is_caught`]
/// inject an extra row and prove the liveness assert fires.
fn account(corpus: &Corpus, table: &[NormRow]) -> Ledger {
    let mut led = Ledger {
        norm_hits: vec![0; table.len()],
        ..Ledger::default()
    };
    for row in &corpus.rows {
        let verdicts = chain_verdicts(corpus, row);
        if verdicts.iter().filter(|v| **v).count() > 1 {
            led.multi_link += 1;
        }
        match first_match(verdicts) {
            Some(Link::Normalization) => {
                // Re-ask through the injected table so the negative tests can
                // drive a different one; the shipped path is the same query.
                let hit = table.iter().position(|r| {
                    r.class.eq_ignore_ascii_case(&row.class)
                        && r.prop.eq_ignore_ascii_case(&row.prop)
                });
                match hit {
                    Some(i) => {
                        led.norm_hits[i] += 1;
                        *led.claimed.entry(table[i].rule.tag()).or_default() += 1;
                    }
                    None => led.unaccounted.push(format!(
                        "{} '{}' vs '{}': the shipped table claims it, the injected one has no row",
                        row.pair, row.rust, row.r4133
                    )),
                }
            }
            Some(link) => *led.claimed.entry(link.tag()).or_default() += 1,
            None => {
                let Some(ev) = corpus.evidence(row) else {
                    led.unaccounted.push(format!(
                        "{} '{}' vs '{}': no bins.tsv row and no supplement provenance",
                        row.pair, row.rust, row.r4133
                    ));
                    continue;
                };
                match declare(row, ev) {
                    Ok(owner) => {
                        let b = led.declared.entry(owner).or_default();
                        b.rows += 1;
                        b.pairs.insert(row.pair.clone());
                        if ev.in_scope() {
                            b.in_scope_rows += 1;
                        }
                    }
                    Err(why) => led.unaccounted.push(format!("{why} [{}]", ev.origin)),
                }
            }
        }
    }
    led
}

/// `PROPS_NORM_R4133` rows that claimed no example row — the offline half of
/// plan mechanic (d), "liveness both ways".
fn dead_norm_rows(table: &[NormRow], led: &Ledger) -> Vec<String> {
    table
        .iter()
        .zip(&led.norm_hits)
        .filter(|(_, hits)| **hits == 0)
        .map(|(r, _)| format!("{}.{} ({})", r.class, r.prop, r.rule.tag()))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// **The master accounting.** Every example row of both files is claimed by the
/// first matching link of the chain or declared to a named later sub-step, and
/// every count is locked in both directions.
#[test]
fn every_example_row_is_claimed_or_declared_exactly_once() {
    let corpus = Corpus::load();
    let frozen = corpus
        .rows
        .iter()
        .filter(|r| r.src == Source::Frozen)
        .count();
    let supplement = corpus.rows.len() - frozen;
    assert_eq!(frozen, ROWS_FROZEN, "examples_full.txt data rows");
    assert_eq!(
        supplement, ROWS_SUPPLEMENT,
        "examples_supplement.txt data rows"
    );

    let led = account(&corpus, PROPS_NORM_R4133);
    assert!(
        led.unaccounted.is_empty(),
        "RP2.1's kill criterion: {} example row(s) fit no mechanism and no declared-pending \
         set.\n  {}",
        led.unaccounted.len(),
        led.unaccounted.join("\n  ")
    );

    // Claimed, per link and per rule kind.
    let claimed = |k: &str| led.claimed.get(k).copied().unwrap_or(0);
    assert_eq!(claimed("BoolFold"), CLAIMED_BOOL_FOLD);
    assert_eq!(claimed("CaseFold"), CLAIMED_CASE_FOLD);
    assert_eq!(claimed("ArrayForm"), CLAIMED_ARRAY_FORM);
    assert_eq!(
        claimed("EnumSynonym"),
        CLAIMED_ENUM_SYNONYM,
        "RP2.2's five EnumSynonym rows — the four bin-3 source sequence selectors plus the \
         off-bin `invcontrol.voltage_curvex_ref`"
    );
    assert_eq!(
        claimed(Link::ShapeAllowlist.tag()),
        CLAIMED_SHAPE_ALLOWLIST,
        "the census recorded the one r4133-active allowlist gap as SHAPE rows, never as value \
         cells — this link is exercised against shape.txt instead"
    );
    assert_eq!(
        claimed(Link::Echo.tag()),
        CLAIMED_ECHO,
        "PROPS_ECHO_R4133's 81 cited rows claim what the nine off-bin normalization rows leave, \
         minus the five pairs the kill criterion re-routed to RP3.8"
    );
    assert_eq!(
        claimed(Link::DisplayFloor.tag()),
        CLAIMED_DISPLAY_FLOOR,
        "the display floor is RP2.4's; RP2.1 introduces no tolerance"
    );
    assert_eq!(
        claimed("BoolFold") + claimed("CaseFold") + claimed("ArrayForm") + claimed("EnumSynonym"),
        CLAIMED_NORMALIZATION,
        "the four rule kinds must partition the normalization link"
    );
    assert_eq!(
        led.claimed_total(),
        CLAIMED_TOTAL,
        "claimed example rows in total"
    );

    // Declared, per owner: rows, pairs, rows on in-scope pairs. Each number is
    // what that sub-step inherits — the guard against a pair silently vanishing
    // from, or appearing in, its residual.
    for (owner, want) in [
        (Owner::Rp22, DECLARED_RP22),
        (Owner::Rp23, DECLARED_RP23),
        (Owner::Rp24, DECLARED_RP24),
        (Owner::Rp3, DECLARED_RP3),
        (Owner::Rp35, DECLARED_RP35),
        (Owner::Rp38, DECLARED_RP38),
        (Owner::OutOfScope, DECLARED_OUT_OF_SCOPE),
    ] {
        assert_eq!(
            led.owner(owner),
            want,
            "{} inherits (rows, pairs, rows on in-scope pairs)",
            owner.tag()
        );
    }
    assert_eq!(
        led.owner(Owner::OutOfScope).2,
        0,
        "plan §1.3: an OutOfScope row must have no in-scope cell at all — nothing else justifies \
         leaving a divergence unowned"
    );

    // Totality, both ways.
    assert_eq!(
        led.claimed_total() + led.declared_total(),
        corpus.rows.len(),
        "every example row is claimed or declared, exactly once"
    );
    assert_eq!(
        led.multi_link, MULTI_LINK_ROWS,
        "rows matched by more than one link — RP2.3's echo rows made this positive, and the \
         first-match order (pinned by first_match_returns_the_earliest_link) is what decides them"
    );
}

/// **The per-pair split of the OFFLINE attribution** on the pairs that hold
/// both kinds of row — the per-pair half of [`MULTI_LINK_ROWS`].
///
/// Read what this measures, and what it does not (RP2.3 audit settlement,
/// 2026-08-23 — it used to be called `the_echo_table_masks_only_what_the_typed_
/// rules_leave`, which claimed the second thing): for every example row of a
/// cited pair it asks the chain which link is credited with it, and pins the
/// resulting split pair by pair. That is a statement about the **census
/// attribution** — the claims-census disposition and the norm rows' liveness —
/// not about the live comparator, whose exclusion is pair-scoped and covers a
/// mixed pair's refused cells too
/// (`harness::props_policy_tests::a_mixed_pairs_echo_row_masks_the_cells_its_
/// rule_refuses` pins that behaviour where it actually lives).
///
/// What it still catches, and why it is worth keeping: deleting or narrowing one
/// of the nine off-bin normalization rows RP2.3 landed, and re-ordering the
/// offline chain so the exclusion is asked first — either moves a pair's
/// normalization count to zero.
#[test]
fn the_typed_rules_and_the_echo_rows_split_their_shared_pairs_offline() {
    let corpus = Corpus::load();
    let mut split: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut carved = 0;
    for row in &corpus.rows {
        if !props_norm::has_echo_row(&row.class, &row.prop) {
            continue;
        }
        let e = split.entry(row.pair.as_str()).or_default();
        match first_match(chain_verdicts(&corpus, row)) {
            Some(Link::Normalization) => e.0 += 1,
            Some(Link::Echo) => e.1 += 1,
            // A carved-out cell of a cited pair: no link claims it, and
            // `ECHO_CARVE_OUT_ROUTING` says who owns it instead.
            None if ECHO_CARVE_OUT_ROUTING
                .iter()
                .any(|(p, r, o, _, _)| *p == row.pair && *r == row.rust && *o == row.r4133) =>
            {
                carved += 1
            }
            other => panic!("{}: an echo pair's row answered {other:?}", row.pair),
        }
    }
    assert_eq!(
        carved,
        ECHO_CARVE_OUT_ROUTING.len(),
        "every carve-out must match exactly one example row of its pair"
    );
    let mixed: Vec<(&str, usize, usize)> = split
        .iter()
        .filter(|(_, (norm, _))| *norm > 0)
        .map(|(p, (n, e))| (*p, *n, *e))
        .collect();
    assert_eq!(
        mixed,
        [
            ("capcontrol.type", 3, 2),
            ("energymeter.peakcurrent", 2, 1),
            ("fault.bus2", 1, 1),
            ("fuse.switchedobj", 4, 4),
            ("generator.userdata", 2, 1),
            ("invcontrol.mode", 7, 1),
            ("invcontrol.monvoltagecalc", 2, 1),
            ("line.wires", 2, 2),
            ("load.yearly", 72, 23),
            ("load.zipv", 2, 1),
            ("reactor.bus2", 14, 7),
            ("recloser.eventlog", 1, 1),
            ("recloser.switchedobj", 6, 5),
            ("regcontrol.idle", 1, 1),
            ("relay.distreverse", 2, 1),
            ("relay.reset", 1, 1),
            ("relay.switchedobj", 8, 10),
            ("storage.dynadata", 1, 1),
            ("storagecontroller.seasontargets", 2, 1),
            ("storagecontroller.seasontargetslow", 2, 1),
        ],
        "(pair, rows claimed by its normalization row, rows credited to its echo row)"
    );
    assert_eq!(
        mixed.iter().map(|(_, n, _)| n).sum::<usize>(),
        MULTI_LINK_ROWS,
        "the per-pair normalization counts must sum to the multi-link lock"
    );
    assert_eq!(
        split.values().map(|(_, e)| e).sum::<usize>(),
        CLAIMED_ECHO,
        "…and the echo counts to the echo link's total"
    );
    assert_eq!(
        split.len(),
        props_norm::PROPS_ECHO_R4133.len(),
        "every echo row is exercised by at least one example row (offline liveness)"
    );
}

/// **Liveness both ways for the echo table, offline** — the exclusion's half of
/// plan mechanic (d), and the mirror of
/// [`every_normalization_row_claims_at_least_one_example_row`].
///
/// A row that claims no example row is masking a divergence the vendored census
/// never recorded: it either names the wrong pair or is obsolete. (The LIVE
/// half is `props_norm::assert_echo_rows_are_live`, dormant until RP4.1.)
#[test]
fn every_echo_row_claims_at_least_one_example_row() {
    let corpus = Corpus::load();
    let claimed: BTreeSet<String> = corpus
        .rows
        .iter()
        .filter(|r| first_match(chain_verdicts(&corpus, r)) == Some(Link::Echo))
        .map(|r| r.pair.clone())
        .collect();
    let dead: Vec<String> = props_norm::PROPS_ECHO_R4133
        .iter()
        .filter(|r| !claimed.contains(&format!("{}.{}", r.class, r.prop)))
        .map(|r| format!("{}.{} ({})", r.class, r.prop, r.category.tag()))
        .collect();
    assert!(
        dead.is_empty(),
        "{} PROPS_ECHO_R4133 row(s) exclude no example row: {}",
        dead.len(),
        dead.join(", ")
    );
    assert_eq!(claimed.len(), props_norm::PROPS_ECHO_R4133.len());
}

/// **Evidence integrity of the echo table** (plan mechanic (a)): every row's
/// `cells` column is read back against the file it cites — `bins.tsv` for the
/// frozen census, the vendored `README.md` §"Pairs the WP-RP1 shape closures
/// make live" (or the supplement's provenance header) for the pairs WP-RP1 and
/// RP0.2 created. A mis-transcribed row fails here instead of silently
/// describing a pair that is not there.
#[test]
fn every_echo_row_matches_its_cited_evidence() {
    let corpus = Corpus::load();
    let bins = bins_evidence();
    for r in props_norm::PROPS_ECHO_R4133 {
        let pair = format!("{}.{}", r.class, r.prop);
        let ev = corpus
            .supplement
            .get(&pair)
            .or_else(|| bins.get(&pair).and_then(|all| all.first()))
            .unwrap_or_else(|| panic!("{pair}: an echo row with no census evidence row"));
        assert_eq!(
            ev.cells, r.cells as usize,
            "{pair}: the row cites {} cells, {} says {}",
            r.cells, ev.origin, ev.cells
        );
        assert!(
            bins.get(&pair).map(|all| all.len()).unwrap_or(1) == 1,
            "{pair}: two evidence rows — the citation would be ambiguous"
        );
    }
}

/// **Every pin an echo row names is a test that exists**, and every pin in the
/// pin file is named by a row — the other half of the witness obligation.
///
/// `props_norm::tests::every_live_semantics_row_names_a_pin` pins the twenty
/// names *as strings*; nothing there can tell whether the tests behind them were
/// ever written, so a row could ship citing a witness that does not exist. This
/// reads the pin file and matches the names against its definitions, both ways —
/// a renamed, deleted or newly-orphaned pin fails here.
///
/// It insists on the **`#[test]` attribute**, not merely on the `fn`: a pin that
/// lost its attribute would still satisfy a name search while never running
/// again, which is precisely the silent-witness failure this guard exists to
/// prevent.
#[test]
fn every_echo_row_pin_is_a_test_that_exists() {
    let path = repo_root().join(PINS);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    // Line endings are the checkout's, not this test's business.
    let text = text.replace("\r\n", "\n");

    let named: BTreeSet<&str> = props_norm::PROPS_ECHO_R4133
        .iter()
        .filter_map(|r| r.witness.pin())
        .collect();
    for name in &named {
        assert!(
            text.contains(&format!("#[test]\nfn {name}() {{")),
            "{name} is named as an echo row's witness but {PINS} defines no such #[test]"
        );
    }

    // …and the file carries no pin nobody cites. The two exceptions are the
    // deck guard's own self-tests, not witnesses — see [`NOT_A_PIN`], which is
    // pinned literally by [`the_non_pin_exemption_list_is_pinned`] so that a
    // third entry cannot quietly let an un-cited `#[test]` past this guard.
    let defined: BTreeSet<&str> = text
        .split("#[test]\nfn ")
        .skip(1)
        .filter_map(|rest| rest.split_once("() {").map(|(name, _)| name))
        .filter(|n| !NOT_A_PIN.contains(n))
        .collect();
    assert_eq!(
        defined, named,
        "{PINS} must define exactly the pins the echo rows name"
    );
}

/// The `#[test]`s in the pin file that are **not** witnesses: the deck guard's
/// own two self-tests. Named, never pattern-matched — a naming convention would
/// let a third un-cited test slip past
/// [`every_echo_row_pin_is_a_test_that_exists`].
const NOT_A_PIN: &[&str] = &[
    "the_deck_guard_really_sweeps",
    "overlapping_deck_guards_still_sweep",
];

/// **The non-pin exemption list is pinned literally** (RP2.3 audit settlement,
/// 2026-08-23): [`NOT_A_PIN`] switches off the only guard that ties a witness
/// name to a real, running test, so growing it is a decision. Before this,
/// `every_echo_row_pin_is_a_test_that_exists` iterated whatever the const
/// happened to hold, and a bogus third name would have let an un-cited `#[test]`
/// through — the very failure the const's own comment says it prevents.
#[test]
fn the_non_pin_exemption_list_is_pinned() {
    assert_eq!(
        NOT_A_PIN,
        [
            "the_deck_guard_really_sweeps",
            "overlapping_deck_guards_still_sweep",
        ],
        "the pin file's two deck-guard self-tests, and nothing else"
    );
}

/// **The kill-criterion re-route is exactly the five named pairs**, in both
/// directions: they take no echo row, they land in [`Owner::Rp38`]'s bucket and
/// nothing else does, and each carries the r4133 live-getter citation that
/// makes the re-route a verdict rather than a shrug.
#[test]
fn the_kill_criterion_reroute_is_the_five_silent_readonly_pairs() {
    assert_eq!(
        RP38_ROUTING.iter().map(|(p, _)| *p).collect::<Vec<_>>(),
        [
            "indmach012.pf",
            "storagecontroller.kwhtotal",
            "storagecontroller.kwtotal",
            "storagecontroller.kwhactual",
            "storagecontroller.kwactual",
        ],
        "the five pairs the RP2.3 kill ruling re-routed"
    );
    for (pair, cite) in RP38_ROUTING {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        assert!(
            !props_norm::has_echo_row(class, prop),
            "{pair} must NOT have an echo row — the ruling forbids it"
        );
        assert!(
            cite.contains(".pas:"),
            "{pair}: the re-route must cite the r4133 live getter, got {cite:?}"
        );
    }
    // …and the bucket holds their rows and only theirs.
    let corpus = Corpus::load();
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(led.owner(Owner::Rp38), DECLARED_RP38);
    let bucket = led.declared.get(&Owner::Rp38).expect("the RP3.8 bucket");
    assert_eq!(
        bucket.pairs.iter().map(String::as_str).collect::<Vec<_>>(),
        [
            "indmach012.pf",
            "storagecontroller.kwactual",
            "storagecontroller.kwhactual",
            "storagecontroller.kwhtotal",
            "storagecontroller.kwtotal",
        ],
        "the RP3.8 bucket holds exactly the re-routed pairs"
    );
}

/// **The offline evidence base is one spelling behind the live population, and
/// that term is asserted rather than assumed** (RP2.1 audit round).
///
/// The replay proves per-spelling completeness over the vendored files; the
/// claims census measures it over the live corpus. The two differ by
/// [`LIVE_ONLY_SPELLINGS`] — 749 live against 748 here — and every part of that
/// statement is checked here: the count reconciles, each live-only spelling is
/// really claimed by the shipped table (so a future narrowing of the rule that
/// claims it reds this test, not just RP4.1), each sits on a pair the table
/// holds, and each sits on a pair the supplement **cannot** carry.
#[test]
fn the_live_only_spellings_are_claimed_and_reconcile_the_two_accountings() {
    let corpus = Corpus::load();
    assert_eq!(
        CLAIMED_NORMALIZATION + LIVE_ONLY_SPELLINGS.len(),
        CLAIMED_TOTAL_LIVE,
        "the vendored claim count plus the live-only spellings must equal what the \
         full-population claims census measured (README §\"What the r4133 policy claims today\")"
    );
    for (pair, rust, r4133, why) in LIVE_ONLY_SPELLINGS {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        // Claimed by the shipped table, through the shipped predicate.
        let row = props_norm::claiming_row(class, prop, rust, r4133).unwrap_or_else(|| {
            panic!(
                "{pair} '{rust}' vs '{r4133}' ({why}) is claimed live but NOT by \
                 PROPS_NORM_R4133 — the live population and this evidence base have drifted"
            )
        });
        assert_eq!((row.class, row.prop), (class, prop));
        // And it is invisible to both vendored files, for the recorded reason:
        // the pair has a frozen `bins.tsv` row, which is exactly what bars it
        // from the supplement.
        assert!(
            corpus.bins.contains_key(*pair),
            "{pair} has no bins.tsv row — then the supplement could carry this spelling, and it \
             should, instead of living in this list"
        );
        assert!(
            !corpus
                .rows
                .iter()
                .any(|r| &r.pair == pair && r.rust == *rust && r.r4133 == *r4133),
            "{pair} '{rust}' vs '{r4133}' IS in the vendored evidence after all — drop it from \
             LIVE_ONLY_SPELLINGS and move the count with it"
        );
    }
}

/// **Liveness both ways, offline** (plan mechanic (d)): every row of
/// `PROPS_NORM_R4133` claims at least one example row. A row that folds nothing
/// exempts a spelling difference that is not there — drop it, or re-measure it.
#[test]
fn every_normalization_row_claims_at_least_one_example_row() {
    let corpus = Corpus::load();
    let led = account(&corpus, PROPS_NORM_R4133);
    let dead = dead_norm_rows(PROPS_NORM_R4133, &led);
    assert!(
        dead.is_empty(),
        "{} PROPS_NORM_R4133 row(s) claim no example row: {}",
        dead.len(),
        dead.join(", ")
    );
    assert_eq!(PROPS_NORM_R4133.len(), NORM_ROWS, "table count lock");
    assert_eq!(
        led.norm_hits.iter().sum::<usize>(),
        CLAIMED_NORMALIZATION,
        "per-row hits must sum to the normalization link's total"
    );
}

/// The ten rows the **supplement** makes live, named: they are exactly the
/// `Evidence::Rp1*` rows of `PROPS_NORM_R4133`, and they claim ONLY supplement
/// rows. Without `examples_supplement.txt` the replay would report full offline
/// coverage over a population that excludes the very classes WP-RP1 opened
/// (plan §1.2), so this is the assertion that would fail if the supplement were
/// dropped from the accounting.
#[test]
fn the_supplement_is_what_makes_the_wp_rp1_rows_live() {
    let corpus = Corpus::load();
    let frozen_only = Corpus {
        rows: corpus
            .rows
            .iter()
            .filter(|r| r.src == Source::Frozen)
            .cloned()
            .collect(),
        bins: corpus.bins.clone(),
        supplement: corpus.supplement.clone(),
        numeric_example: corpus.numeric_example.clone(),
        r4133_allowlisted: corpus.r4133_allowlisted.clone(),
    };
    let dead = dead_norm_rows(PROPS_NORM_R4133, &account(&frozen_only, PROPS_NORM_R4133));
    let want: Vec<String> = PROPS_NORM_R4133
        .iter()
        .filter(|r| r.src != CiteSrc::BinsTsv)
        .map(|r| format!("{}.{} ({})", r.class, r.prop, r.rule.tag()))
        .collect();
    assert_eq!(
        dead, want,
        "on examples_full.txt alone, exactly the WP-RP1 rows are dead — that is what the \
         supplement exists for"
    );
    assert_eq!(
        want.len(),
        10,
        "7 BoolFold + 2 CaseFold WP-RP1 rows, plus RP2.3's off-bin `generator.userdata` \
         ArrayForm row — its pair, like the other nine, exists ONLY in the supplement"
    );
}

/// **The shape half of the chain** (plan §1.2: "shape-allowlist rows are
/// exercised against the full `shape.txt`, capi-only classes included").
///
/// Both directions: every `shape.txt` gap name is closed either by a
/// `PROPS_015X` row this plan added or by an RP1 port, and every such allowlist
/// row is exercised by a gap name. The pre-existing 0.15.x rows are **not** in
/// this accounting — they answer to `props_roundtrip`/goldens — and the
/// intersection with `shape.txt` is what separates them.
#[test]
fn the_shape_allowlist_rows_are_exercised_by_shape_txt() {
    let rows = shape_rows();
    assert_eq!(rows.len(), 5, "shape.txt: five classes");

    let mut by_allowlist: BTreeSet<(String, String)> = BTreeSet::new();
    let mut by_port: Vec<(String, String)> = Vec::new();
    for s in &rows {
        let gaps: Vec<&String> = s.oracle_only.iter().chain(s.rust_only.iter()).collect();
        assert!(
            !gaps.is_empty(),
            "{}: a shape row with no gap name",
            s.class
        );
        for name in gaps {
            if prop_015x(PROPS_015X, &s.class, name) {
                by_allowlist.insert((s.class.clone(), name.clone()));
            } else {
                by_port.push((s.class.clone(), name.clone()));
            }
        }
    }
    assert_eq!(
        by_allowlist.len(),
        SHAPE_ALLOWLIST_NAMES,
        "gap names closed by a PROPS_015X row: {by_allowlist:?}"
    );
    let ported: Vec<(&str, &str)> = by_port
        .iter()
        .map(|(c, p)| (c.as_str(), p.as_str()))
        .collect();
    assert_eq!(
        ported, SHAPE_PORTED,
        "the gap names no allowlist row covers must be exactly RP1.3's ported WindGen surface"
    );
    assert_eq!(ported.len(), SHAPE_PORTED_NAMES);

    // The one row that fires on the r4133 side (a Rust-only property r4133's
    // own table lost) versus the ones that fire on the 0.14.5 capture.
    let r4133_active: Vec<&(String, String)> = by_allowlist
        .iter()
        .filter(|(c, p)| {
            rows.iter()
                .any(|s| s.class == *c && s.rust_only.contains(p))
        })
        .collect();
    assert_eq!(
        r4133_active.len(),
        R4133_ACTIVE_ALLOWLIST_NAMES,
        "exactly one PROPS_015X row is active on r4133: {r4133_active:?}"
    );
    assert_eq!(
        r4133_active.first().map(|(c, p)| (c.as_str(), p.as_str())),
        Some(("gendispatcher", "weights")),
        "RP1.4's registration-bug row"
    );
}

/// **Evidence integrity** (plan mechanic (a), and RP2.1 part B's own promise):
/// every `PROPS_NORM_R4133` row's citation triple *(pair, bin, cells)* is read
/// back against the file it cites — `bins.tsv` for the frozen census, the
/// vendored README's WP-RP1 tables for the pairs the shape closures created.
#[test]
fn every_norm_row_matches_its_cited_evidence() {
    let bins = bins_evidence();
    let rp1 = rp1_records();
    assert_eq!(rp1.len(), 24, "README records 24 WP-RP1 pairs");
    for (step, want) in [
        (CiteSrc::Rp11, 12),
        (CiteSrc::Rp12, 9),
        (CiteSrc::Rp13, 2),
        (CiteSrc::Rp14, 1),
    ] {
        assert_eq!(
            rp1.iter().filter(|r| r.substep == step).count(),
            want,
            "README §{}: {want} new pairs",
            substep_tag(step)
        );
    }

    for r in PROPS_NORM_R4133 {
        let pair = format!("{}.{}", r.class, r.prop);
        match r.src {
            CiteSrc::BinsTsv => {
                let all = bins
                    .get(&pair)
                    .unwrap_or_else(|| panic!("{pair}: cites bins.tsv, which has no such pair"));
                assert!(
                    all.iter()
                        .any(|e| e.bin == r.bin && e.cells == r.cells as usize),
                    "{pair}: row says (bin {}, {} cells), bins.tsv says {:?}",
                    r.bin,
                    r.cells,
                    all.iter().map(|e| (e.bin, e.cells)).collect::<Vec<_>>()
                );
            }
            step => {
                let rec = rp1.iter().find(|x| x.pair == pair).unwrap_or_else(|| {
                    panic!(
                        "{pair}: cites README §{}, which has no such row",
                        substep_tag(step)
                    )
                });
                assert_eq!(
                    rec.substep, step,
                    "{pair}: recorded under a different sub-step"
                );
                assert_eq!(
                    (rec.bin, rec.cells),
                    (r.bin, r.cells as usize),
                    "{pair}: row says (bin {}, {} cells), README says (bin {}, {} cells)",
                    r.bin,
                    r.cells,
                    rec.bin,
                    rec.cells
                );
            }
        }
    }
}

/// The supplement carries a row for **every** WP-RP1 pair the README records,
/// spelled the way the README spells it (the README's representative spelling
/// must be among the supplement's distinct spellings for that pair) — the check
/// that the re-measure supplements the record instead of replacing it.
#[test]
fn the_supplement_covers_every_recorded_wp_rp1_pair() {
    let rows = examples("examples_supplement.txt", Source::Supplement);
    let mut by_pair: BTreeMap<String, Vec<&Example>> = BTreeMap::new();
    for r in &rows {
        by_pair.entry(r.pair.clone()).or_default().push(r);
    }
    assert_eq!(rows.len(), ROWS_SUPPLEMENT);
    assert_eq!(by_pair.len(), SUPPLEMENT_PAIRS);
    assert_eq!(
        rows.iter().map(|r| r.cells).sum::<usize>(),
        SUPPLEMENT_CELLS
    );

    for rec in rp1_records() {
        let got = by_pair.get(&rec.pair).unwrap_or_else(|| {
            panic!(
                "{}: recorded by the README, missing from the supplement",
                rec.pair
            )
        });
        assert_eq!(
            got.iter().map(|r| r.cells).sum::<usize>(),
            rec.cells,
            "{}: the supplement's spellings must account for every cell the README records",
            rec.pair
        );
        // The README prints one representative spelling per pair, elided with an
        // ellipsis when it is long; whole values live only here.
        let elided = rec.rust.contains('…') || rec.r4133.contains('…');
        assert!(
            elided
                || got
                    .iter()
                    .any(|r| r.rust == rec.rust && r.r4133 == rec.r4133),
            "{}: the README's example '{}' vs '{}' is not among the supplement's spellings",
            rec.pair,
            rec.rust,
            rec.r4133
        );
    }
    // The frozen extracts cannot hold any of these pairs — that is why the file
    // exists (a pair in both would make the (pair, rust, r4133) join ambiguous).
    let bins = bins_evidence();
    for pair in by_pair.keys() {
        assert!(
            !bins.contains_key(pair),
            "{pair} is in bins.tsv — the supplement is only for pairs no frozen row can carry"
        );
    }
}

/// **The echo table claims only its own 81 pairs, and the display floor still
/// claims nothing** — the deliberate successor of RP2.1's
/// `the_echo_table_and_the_display_floor_claim_nothing_yet`, which asserted
/// that BOTH links were inert.
///
/// That test going red is the design working: RP2.3 filled one of the two
/// slots. What replaces it asserts strictly more than the half that is still
/// true — the floor is untouched (`None`, claiming nothing, on the same
/// full-corpus walk) — plus the two statements the newly-live link owes:
///
/// * the echo link claims a row **only** on a pair `PROPS_ECHO_R4133` names, so
///   the exclusion cannot have leaked onto a pair no row cites;
/// * a pair the table does NOT name is still compared raw, even when its
///   spelling looks exactly like an echo (`''` on one side) — the mask is the
///   cited row set, never a shape heuristic.
#[test]
fn the_echo_table_claims_only_its_cited_pairs_and_the_floor_claims_nothing() {
    assert_eq!(
        PROPS_ECHO_R4133.len(),
        81,
        "RP2.3's echo table: the RP2.3 bucket's 86 pairs minus the 5 the kill criterion re-routed"
    );
    assert!(
        props_norm::display_floor().is_none(),
        "RP2.4 derives the r4133 props display floor; RP2.3 introduces no tolerance anywhere"
    );
    let cited: BTreeSet<String> = PROPS_ECHO_R4133
        .iter()
        .map(|r| format!("{}.{}", r.class, r.prop))
        .collect();
    let corpus = Corpus::load();
    let mut echoed = 0;
    for row in &corpus.rows {
        let [_, _, echo, floor] = chain_verdicts(&corpus, row);
        assert!(
            !floor,
            "{}: the display floor claimed a row while it is None",
            row.pair
        );
        if echo {
            assert!(
                cited.contains(&row.pair),
                "{}: the echo link claimed a row on a pair no EchoRow cites",
                row.pair
            );
            echoed += 1;
        }
    }
    assert!(echoed > 0, "the echo link must claim something now");
    // The five re-routed pairs still carry `''`-on-one-side spellings — the
    // shape bin 5 is built on — and are compared all the same.
    for (pair, _) in RP38_ROUTING {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        assert!(
            !props_norm::has_echo_row(class, prop),
            "{pair}: an echo-LOOKING spelling is not an echo row"
        );
    }
    // …and neither is a pair whose only divergence is a genuine value jump.
    assert!(!props_norm::has_echo_row("gictransformer", "r2"));
    assert!(!props_norm::has_echo_row("generator", "model"));
}

/// **The carve-outs and their routing describe the same cells** — the
/// third-map guard for the narrowing valve the RP2.3 audit settlement added.
///
/// `props_norm::ECHO_CARVE_OUTS` decides what the comparator lets through;
/// [`ECHO_CARVE_OUT_ROUTING`] decides who then owns the cell. A carve-out with
/// no routing would land in the ledger's `unaccounted` bucket (RP2.1's kill
/// criterion, which is the loud outcome) but a routing with no carve-out would
/// simply never fire, so both directions are asserted here, plus the two things
/// that make a routing a verdict: an owner that is not RP2.3's own bucket, and a
/// citation.
#[test]
fn the_carve_outs_are_routed_and_only_they_are() {
    let shipped: BTreeSet<(String, &str, &str)> = props_norm::ECHO_CARVE_OUTS
        .iter()
        .map(|c| (format!("{}.{}", c.class, c.prop), c.rust, c.oracle))
        .collect();
    let routed: BTreeSet<(String, &str, &str)> = ECHO_CARVE_OUT_ROUTING
        .iter()
        .map(|(pair, rust, r4133, _, _)| (pair.to_string(), *rust, *r4133))
        .collect();
    assert_eq!(
        shipped, routed,
        "every carve-out needs a recorded owner, and every routing a carve-out to own"
    );
    for (pair, rust, r4133, owner, cite) in ECHO_CARVE_OUT_ROUTING {
        assert_ne!(
            *owner,
            Owner::Rp23,
            "{pair}: routing a carved-out cell back to RP2.3 would re-close the mask this valve \
             opened"
        );
        assert!(
            cite.contains(".pas:"),
            "{pair}: a carve-out routing must cite the r4133 site, got {cite:?}"
        );
        // …and the cell really is out of the shipped exclusion.
        let (class, prop) = pair.split_once('.').expect("class.prop");
        assert!(props_norm::has_echo_row(class, prop));
        assert!(!props_norm::echo_excluded(class, prop, rust, r4133));
    }
    // Liveness: the routed owner really inherits the row.
    let corpus = Corpus::load();
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(
        led.owner(Owner::Rp24),
        DECLARED_RP24,
        "RP2.4's bucket carries the carved-out display cell"
    );
}

/// The chain order is the documented one, and [`first_match`] really returns the
/// **earliest** matching link — the non-vacuity proof for "claimed by the first
/// matching mechanism". Without this the order assertion would be a tautology
/// while the echo table and the floor are empty.
#[test]
fn first_match_returns_the_earliest_link() {
    assert_eq!(
        Link::ORDER,
        [
            Link::ShapeAllowlist,
            Link::Normalization,
            Link::Echo,
            Link::DisplayFloor
        ],
        "shape allowlist -> normalization -> echo table -> display floor"
    );
    assert_eq!(first_match([false, false, false, false]), None);
    assert_eq!(
        first_match([false, true, true, false]),
        Some(Link::Normalization),
        "normalization precedes the echo table: a foldable cell of a mixed pair is CLAIMED BY \
         ITS RULE, and only what the rule leaves is credited to the exclusion"
    );
    assert_eq!(
        first_match([true, true, true, true]),
        Some(Link::ShapeAllowlist),
        "a property the oracle's name list cannot carry never reaches the value compare"
    );
    assert_eq!(
        first_match([false, false, true, true]),
        Some(Link::Echo),
        "an excluded cell is never re-admitted by the floor"
    );
    assert_eq!(
        first_match([false, false, false, true]),
        Some(Link::DisplayFloor)
    );
}

/// **Non-vacuity of the liveness assert**: a normalization row that claims
/// nothing is reported. Driven through the same [`account`]/[`dead_norm_rows`]
/// path the master test uses, with one extra row injected.
#[test]
fn a_normalization_row_that_claims_nothing_is_caught() {
    let corpus = Corpus::load();
    let mut table: Vec<NormRow> = Vec::new();
    for r in PROPS_NORM_R4133 {
        table.push(NormRow {
            class: r.class,
            prop: r.prop,
            rule: r.rule,
            bin: r.bin,
            cells: r.cells,
            src: r.src,
        });
    }
    // A pair no example row mentions: the row can never fold anything.
    table.push(NormRow {
        class: "nosuchclass",
        prop: "nosuchprop",
        rule: NormRule::BoolFold,
        bin: 1,
        cells: 1,
        src: CiteSrc::BinsTsv,
    });
    let led = account(&corpus, &table);
    assert_eq!(
        dead_norm_rows(&table, &led),
        vec!["nosuchclass.nosuchprop (BoolFold)".to_string()],
        "a row that folds nothing must be reported, not ignored"
    );
}

/// **Non-vacuity of the accounting**: an example row that no link claims and no
/// declaration rule can place is reported by name — RP2.1's kill criterion. The
/// probe is a realistic one: a genuine value difference inside a bin-4 pair
/// whose in-scope cells the plan does not route anywhere.
#[test]
fn an_example_row_no_mechanism_and_no_marker_claims_is_caught() {
    let mut corpus = Corpus::load();
    corpus.rows = vec![Example {
        pair: "line.ratings".to_string(),
        class: "line".to_string(),
        prop: "ratings".to_string(),
        rust: "[ 400]".to_string(),
        r4133: "[401, 402]".to_string(),
        cells: 1,
        src: Source::Frozen,
    }];
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(led.claimed_total(), 0, "ArrayForm must refuse it");
    assert_eq!(led.unaccounted.len(), 1, "{:?}", led.unaccounted);
    assert!(
        led.unaccounted[0].contains("line.ratings")
            && led.unaccounted[0].contains("no later sub-step owns"),
        "{:?}",
        led.unaccounted
    );

    // …and a row whose pair has no evidence at all is caught too.
    corpus.rows = vec![Example {
        pair: "nosuch.pair".to_string(),
        class: "nosuch".to_string(),
        prop: "pair".to_string(),
        rust: "a".to_string(),
        r4133: "b".to_string(),
        cells: 1,
        src: Source::Supplement,
    }];
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(led.unaccounted.len(), 1);
    assert!(
        led.unaccounted[0].contains("no bins.tsv row and no supplement provenance"),
        "{:?}",
        led.unaccounted
    );
}

/// **RP2.2 settled every pair it was handed, with no third state** (plan §RP2.2
/// acceptance: "the replay accounting claims bin 3 fully; every row cites its
/// source line").
///
/// The closed input list is `RP22_S6` ∪ the eight bin-3 pairs. Each of them ends
/// in exactly one of two places, and both are asserted here rather than left to
/// be read off the master accounting:
///
/// * **claimed** by a shipped `PROPS_NORM_R4133` row (every one of its example
///   rows), i.e. `RP22_ROUTING` says nothing about it — **7 pairs**: the five
///   `EnumSynonym` ones (`invcontrol.voltage_curvex_ref` included, which the
///   `RP22_BEYOND_THE_CLOSED_LIST` growth added) and the two fully-folding
///   `ArrayForm` ones;
/// * **routed** by `RP22_ROUTING`, with a citation, to RP2.3 or an RP3.5+
///   sub-step — **20 rows**. 7 + 20 = the 24 closed-list pairs plus the 3
///   beyond it. (The commit subject of `ab2bf041` says "18 cited routings",
///   counting only the closed list's own 18; the audit-settlement commit
///   records the corrected split.)
///
/// The test also runs the two liveness directions the plan's mechanic (d) asks
/// of any table: no routing row is dead (each declares at least one example
/// row), and no routing row names a pair outside RP2.2's remit.
#[test]
fn rp22_settled_every_pair_it_was_handed() {
    let corpus = Corpus::load();
    let bins = bins_evidence();

    // RP2.2's remit: the S6 list plus every structural bin-3 pair.
    let mut handed: BTreeSet<String> = RP22_S6.iter().map(|p| (*p).to_string()).collect();
    for (pair, all) in &bins {
        if all.iter().any(|e| !e.numeric && e.bin == 3) {
            handed.insert(pair.clone());
        }
    }
    assert_eq!(
        handed.len(),
        24,
        "17 S6 pairs + 8 bin-3, `line.units` on both lists"
    );
    let closed_list = handed.len();

    // …plus the three the accounting surfaced: a bin-2 LABEL over genuine bin-3
    // cells. Each must really be that shape, or the list is a shrug.
    for pair in RP22_BEYOND_THE_CLOSED_LIST {
        assert!(
            !handed.contains(*pair),
            "{pair} IS on RP2.2's closed list — it does not belong in the 'beyond' list"
        );
        let ev = bins
            .get(*pair)
            .unwrap_or_else(|| panic!("{pair}: no bins.tsv row"));
        assert!(
            ev.iter().all(|e| !e.numeric && e.bin != 3),
            "{pair}: the 'beyond' list is for pairs whose LABEL is not 3"
        );
        assert!(
            corpus
                .rows
                .iter()
                .any(|r| &r.pair == pair && classify_cell(&r.rust, &r.r4133) == 3),
            "{pair}: no bin-3 cell, so RP2.2 had no reason to touch it"
        );
        handed.insert((*pair).to_string());
    }
    assert_eq!(
        handed.len(),
        closed_list + RP22_BEYOND_THE_CLOSED_LIST.len(),
        "the two lists must be disjoint"
    );

    let routed: BTreeSet<String> = RP22_ROUTING
        .iter()
        .map(|(p, _, _)| (*p).to_string())
        .collect();
    assert_eq!(
        routed.len(),
        RP22_ROUTING.len(),
        "RP22_ROUTING must hold one row per pair"
    );

    // Every routed row carries a real r4133 citation, and every pair it names is
    // one RP2.2 was actually handed (`generator.dynout` is routed through
    // `CELL_DISPOSITION` instead — it came from RP2.1's hand-off, not from S6).
    for (pair, owner, cite) in RP22_ROUTING {
        assert!(
            handed.contains(*pair),
            "{pair} is routed by RP2.2 but was never on its closed list"
        );
        assert!(
            cite.contains(".pas:"),
            "{pair}: the routing row must cite an r4133 source line, got {cite:?}"
        );
        assert!(
            matches!(owner, Owner::Rp23 | Owner::Rp35),
            "{pair}: RP2.2's outcomes are an EnumSynonym row (then it is CLAIMED, not routed), \
             an RP2.3 echo row, or an RP3.5+ sub-step — not {}",
            owner.tag()
        );
        if matches!(owner, Owner::Rp35) {
            assert!(
                cite.starts_with("RP3."),
                "{pair}: an RP3.5+ routing must name its sub-step first, got {cite:?}"
            );
        }
    }

    // The pairs RP2.2 did NOT route are exactly the ones the shipped table
    // claims outright — asserted through the shipped predicate, per example row.
    let claimed_outright: BTreeSet<String> = handed.difference(&routed).cloned().collect();
    assert_eq!(
        claimed_outright
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "invcontrol.monbus",
            "invcontrol.monbusesvbase",
            "invcontrol.voltage_curvex_ref",
            "isource.scantype",
            "isource.sequence",
            "vsource.scantype",
            "vsource.sequence",
        ],
        "the S6/bin-3/off-bin pairs RP2.2 leaves to the normalization table"
    );
    for pair in &claimed_outright {
        let rows: Vec<&Example> = corpus.rows.iter().filter(|r| &r.pair == pair).collect();
        assert!(!rows.is_empty(), "{pair}: no example row at all");
        for r in rows {
            assert!(
                props_norm::claiming_row(&r.class, &r.prop, &r.rust, &r.r4133).is_some(),
                "{pair} '{}' vs '{}' is neither claimed nor routed — RP2.2 would owe it a \
                 verdict",
                r.rust,
                r.r4133
            );
        }
    }

    // Liveness: every routing row really routed something, and RP2.3 CONSUMED
    // the half addressed to it.
    //
    // Before RP2.3 this asked one question — does the pair still declare at
    // least one example row? — because every routed pair was pending. Now the
    // two outcomes part ways, and asking the old question of both would report
    // all fourteen RP2.3 routings as "stale" the moment RP2.3 did its job. So
    // each is checked against what its own verdict promised:
    //
    //  * `Owner::Rp23` — the promise was "RP2.3 lands the cited exclusion row",
    //    so the pair must now HAVE one. This is strictly stronger than the old
    //    check: it is not enough for the row to have left the bucket, the table
    //    has to name the pair.
    //  * `Owner::Rp35` — still pending, so it must still declare a row (RP4.1
    //    does not start until those sub-steps close, plan §0).
    let mut dead: Vec<&str> = Vec::new();
    let mut unconsumed: Vec<&str> = Vec::new();
    for (pair, owner, _) in RP22_ROUTING {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        match owner {
            Owner::Rp23 => {
                if !props_norm::has_echo_row(class, prop) {
                    unconsumed.push(pair);
                }
            }
            _ => {
                let declared = corpus
                    .rows
                    .iter()
                    .filter(|r| &r.pair == pair)
                    .any(|r| first_match(chain_verdicts(&corpus, r)).is_none());
                if !declared {
                    dead.push(pair);
                }
            }
        }
    }
    assert!(
        unconsumed.is_empty(),
        "RP2.2 routed {unconsumed:?} to RP2.3's echo table and no row cites them — either the \
         routing is wrong or RP2.3 dropped the pair"
    );
    assert!(
        dead.is_empty(),
        "RP22_ROUTING row(s) that route nothing — the chain already claims every cell of \
         {dead:?}, so the routing is stale"
    );
    assert_eq!(
        RP22_ROUTING
            .iter()
            .filter(|(_, o, _)| matches!(o, Owner::Rp23))
            .count(),
        14,
        "the fourteen pairs RP2.2 handed to RP2.3, all of them now cited echo rows"
    );
}

/// The plan's own enumerated pair lists still describe the vendored evidence:
/// the eight bin-3 pairs, the sixteen in-scope bin-7 pairs split 12 echo / 4
/// root-cause, and RP2.2's S6 singletons. A stale list here would silently move
/// rows between owners, so it is checked against `bins.tsv` rather than trusted.
#[test]
fn the_plan_pair_lists_still_describe_the_vendored_evidence() {
    let bins = bins_evidence();
    let bin3: Vec<&String> = bins
        .iter()
        .filter(|(_, all)| all.iter().any(|e| !e.numeric && e.bin == 3))
        .map(|(p, _)| p)
        .collect();
    assert_eq!(
        bin3,
        [
            "isource.scantype",
            "isource.sequence",
            "line.units",
            "monitor.mode",
            "storagecontroller.modedischarge",
            "swtcontrol.action",
            "vsource.scantype",
            "vsource.sequence"
        ],
        "plan §RP2.2's eight bin-3 pairs"
    );

    let mut jumps: Vec<&String> = bins
        .iter()
        .filter(|(_, all)| {
            all.iter()
                .any(|e| e.numeric && e.in_scope() && e.effective_bin() == 7)
        })
        .map(|(p, _)| p)
        .collect();
    jumps.sort();
    let mut want: Vec<&str> = BIN7_ECHO
        .iter()
        .chain(BIN7_ROOT_CAUSE.iter())
        .copied()
        .collect();
    want.sort_unstable();
    assert_eq!(
        jumps.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        want,
        "plan §1.1: 16 in-scope bin-7 pairs, 12 echo + 4 root-cause"
    );

    let corpus = Corpus::load();
    let known = |pair: &str| bins.contains_key(pair) || corpus.supplement.contains_key(pair);
    for pair in RP22_S6 {
        assert!(
            known(pair),
            "{pair} is on RP2.2's S6 list but has no evidence row"
        );
    }
    for pair in BIN7_ECHO_SUPPLEMENT {
        assert!(
            known(pair),
            "{pair} is declared echo-rooted but has no evidence row"
        );
    }
    for (pair, _, _) in CELL_DISPOSITION {
        assert!(
            known(pair),
            "{pair} has a cell disposition but no evidence row"
        );
    }
    for (pair, _, _) in RP22_ROUTING {
        assert!(
            known(pair),
            "{pair} carries an RP2.2 routing verdict but has no evidence row"
        );
    }
}
