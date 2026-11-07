//! **The r4133 property replay-accounting test**: every frozen census spelling
//! has exactly one disposition under the r4133 property policy.
//!
//! # What it does
//!
//! It reads the vendored census evidence at `tests/corpus/props_r4133/`,
//! `examples_full.txt` (the frozen census) **plus** `examples_supplement.txt`
//! (the pairs the shape closures made live, and two pairs no frozen row can
//! carry), and pushes **every example row**, i.e. every distinct
//! `(rust, r4133)` spelling of every census pair, through the r4133 policy in
//! the documented chain order:
//!
//! ```text
//! shape allowlist  ->  normalization  ->  echo table  ->  display floor
//!  PROPS_015X          PROPS_NORM_R4133   PROPS_ECHO_R4133   2e-4 relative
//! ```
//!
//! Every row ends up in exactly one of four states:
//!
//! * **claimed**: the first matching link of the chain recognises the two
//!   spellings as one value;
//! * **superseded**: an engine change made the frozen `(rust, r4133)` spelling
//!   counterfactual, so the row is counted against a cited table whose evidence
//!   is live ([`RP38_SUPERSEDED`], [`RP37_SUPERSEDED`], disjoint by
//!   [`the_two_superseded_tables_are_disjoint`]);
//! * **ledgered**: a landed `tests/corpus/ledger.json` `property` entry
//!   excludes the cell, so the exclusion lives outside `props_norm`
//!   ([`RP3_LEDGERED`], whose entry ids are read back out of the ledger);
//! * **declared**: no link claims it, and the row carries an [`Owner`] naming
//!   the mechanism that owns it, or the reason nothing will
//!   ([`Owner::OutOfScope`]).
//!
//! A row can move between states but never appear or vanish: a pair that
//! silently vanishes or appears breaks a count lock here.
//!
//! # The three things it proves
//!
//! 1. **Per-example-row completeness.** Bins 1/2/4 are fully claimed, and every
//!    other row has a named owner. The contract is per example row offline and
//!    per cell live, through the census knob's disposition mode.
//! 2. **Liveness both ways, offline.** Every one of `PROPS_NORM_R4133`'s
//!    [`NORM_ROWS`] rows claims at least one example row: a row that folds
//!    nothing exempts a spelling difference that is not there. The shape half is
//!    exercised against the full `shape.txt`, capi-only classes included. Only
//!    the r4133 rows of the shared `PROPS_015X` are in that accounting (the
//!    0.15.x rows answer to `props_roundtrip` and the goldens).
//! 3. **Evidence integrity of the tables.** Every `PROPS_NORM_R4133` row's
//!    citation triple *(pair, bin, cells)* is read back against its evidence:
//!    `bins.tsv` for the frozen census, [`RP1_RECORDS`] for the pairs the shape
//!    closures created.
//!
//! # Why the pair's `bins.tsv` bin is NOT the admissibility rule
//!
//! A pair's bin is the classification of its **first census row**, not of its
//! cells: some structural pairs are bin-heterogeneous and some bin-1 pairs carry
//! an r4133 echo in some or all cells, both locked in
//! `props_r4133_evidence_lock.rs`. So admissibility is decided **per example
//! row**: `load.yearly` is a bin-5 pair with case-only cells, `relay.switchedobj`
//! a bin-2 pair with `''` echoes, and a mixed bin-1 pair legitimately holds a
//! `BoolFold` claim *and* an echo row. Single-claim is per example row, never
//! per pair.
//!
//! # No OpenDSS engine, no solve, no feature flag
//!
//! The whole test is a walk over vendored text plus the shipped tables, so it is
//! green in both lanes and needs neither dss-python nor the r4133 DLL.

use dss_test_harness::harness;

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

/// The file of the expected-value pins, repo-root-relative:
/// [`every_echo_row_pin_is_a_test_that_exists`] reads it back against
/// `PROPS_ECHO_R4133`'s witness column.
const PINS: &str = "crates/dss-core/tests/props_r4133_pins.rs";

// ---------------------------------------------------------------------------
// Count locks (equalities, both ways). Every number below is a count over the
// vendored files. Moving one belongs in the commit that argues for the new
// population.
// ---------------------------------------------------------------------------

/// `examples_full.txt` data rows — the frozen census's spelling inventory.
const ROWS_FROZEN: usize = 3378;
/// `examples_supplement.txt` data rows.
const ROWS_SUPPLEMENT: usize = 76;
/// Pairs the supplement carries, all absent from `bins.tsv` by construction.
const SUPPLEMENT_PAIRS: usize = 26;
/// Cells behind those rows (the supplement's own `count` column).
const SUPPLEMENT_CELLS: usize = 4541;

/// Example rows the normalization claims, per rule kind.
const CLAIMED_BOOL_FOLD: usize = 114;
/// `CaseFold` rows, including the three off-bin rows (`load.yearly`,
/// `reactor.bus2`, `invcontrol.monvoltagecalc`) that a typed rule compares
/// where an echo row would mask them.
const CLAIMED_CASE_FOLD: usize = 528;
/// `ArrayForm` rows, including the six off-bin rows. The two `swtcontrol` pairs
/// carry no `ArrayForm` row: their one-token spellings are superseded with the
/// rest of the pair ([`RP37_SUPERSEDED`]). The live claims census measures the
/// same `normalized-by-ArrayForm` spellings on the r4133 channel.
const CLAIMED_ARRAY_FORM: usize = 201;
/// The five `EnumSynonym` rows: `vsource.scantype`/`sequence`,
/// `isource.scantype`/`sequence` and `invcontrol.voltage_curvex_ref`
/// (`examples_full.txt`).
const CLAIMED_ENUM_SYNONYM: usize = 9;
/// The four rule kinds together: the NORMALIZATION link's own total, which is
/// what [`CLAIMED_TOTAL_LIVE`] reconciles against.
const CLAIMED_NORMALIZATION: usize = 852;
/// **The echo table**: example rows the exclusion claims, i.e. the ones
/// `PROPS_ECHO_R4133` covers that no earlier link took. The rows of the
/// superseded `SilentReadOnly` pairs ([`RP38_SUPERSEDED`]) and the one carve-out
/// ([`ECHO_CARVE_OUT_ROUTING`]) are not among them.
const CLAIMED_ECHO: usize = 170;
/// …and in total, over all four links.
const CLAIMED_TOTAL: usize =
    CLAIMED_SHAPE_ALLOWLIST + CLAIMED_NORMALIZATION + CLAIMED_ECHO + CLAIMED_DISPLAY_FLOOR;

/// **What the LIVE population claims, against what this evidence base can see.**
///
/// The two accountings differ by [`LIVE_ONLY_SPELLINGS`]: spellings that exist
/// live but that **no vendored file may carry**, because they sit on a pair that
/// already has a `bins.tsv` row (the supplement's own lock forbids a pair in
/// both files, `props_r4133_evidence_lock.rs`
/// `the_supplement_carries_only_pairs_no_frozen_row_can`). The term carries its
/// own assertions, so the two populations cannot diverge silently.
///
/// `cargo test` enforces the OFFLINE identity only:
/// `CLAIMED_NORMALIZATION + LIVE_ONLY_SPELLINGS.len()`. The live half rests on
/// an opt-in `DSS_PROPS_CENSUS=claims` run, by design: the mandatory gate must
/// not need an OpenDSS census. A live-only drift therefore shows up here only
/// when someone runs the census.
const CLAIMED_TOTAL_LIVE: usize = 853;

/// The spellings the live claims census sees and the vendored evidence cannot,
/// each with the pair it belongs to, the two sides, and why no file carries it.
///
/// One entry: a deck that reads the AutoTrans **series** winding, whose `Conn`
/// getter renders `Series`, gives the bin-2 pair `autotrans.conn` a third
/// spelling. Its `bins.tsv` row is frozen at the two spellings the census saw,
/// and [`RP1_RECORDS`] records only pairs a shape closure **created**, so a new
/// spelling on an existing pair has no home in either.
const LIVE_ONLY_SPELLINGS: &[(&str, &str, &str, &str)] = &[(
    "autotrans.conn",
    "series",
    "Series",
    "an AutoTrans series-winding deck, newer than the pair's frozen bins.tsv row",
)];

/// **The display floor's own live-only spellings**: [`LIVE_ONLY_SPELLINGS`]'
/// class for the display floor. They are `%[-].Ng` renders that exist live and
/// that no vendored file may carry, because all five pairs already hold a
/// frozen `bins.tsv` row (all bin 6) and the supplement's own lock bars a pair
/// from both files.
///
/// They come from decks that read `Vsource`'s `Isc*`/`R1`/`X0`/`X1` getters at
/// source impedances the frozen census never saw. Every one is inside its own
/// pair's frozen `max_rel` (asserted with [`LIVE_ONLY_DISPLAY_PAIRS`] and
/// `frozen_max_rel`) and passes the mechanism clause, so they are renders. The
/// list is an asserted term, so the census's claimed spellings and the replay's
/// [`CLAIMED_TOTAL`] reconcile with a stated reason.
const LIVE_ONLY_DISPLAY_SPELLINGS: &[(&str, &str, &str)] = &[
    // (the five pairs below are [`LIVE_ONLY_DISPLAY_PAIRS`])
    ("vsource.isc1", "104347.826086957", "1.0435E005"),
    ("vsource.isc1", "69565.2173913044", "69565"),
    ("vsource.isc3", "45183.9341104925", "45184"),
    ("vsource.r1", "0.3563926267895", "0.35639"),
    ("vsource.x0", "1.92015184059109", "1.9202"),
    ("vsource.x1", "1.425570507158", "1.4256"),
];

/// The pairs [`LIVE_ONLY_DISPLAY_SPELLINGS`] may name: the five `Vsource`
/// getters the live-only drift touches, and nothing else. Asserted, so the list
/// cannot quietly grow a pair whose live population was never measured.
const LIVE_ONLY_DISPLAY_PAIRS: &[&str] = &[
    "vsource.isc1",
    "vsource.isc3",
    "vsource.r1",
    "vsource.x0",
    "vsource.x1",
];

/// Claimed **spellings** the full claims census measures on the r4133 channel:
/// the live counterpart of [`CLAIMED_TOTAL`], reconciled by the two live-only
/// lists above. The number is re-measured with `DSS_PROPS_CENSUS=claims`, never
/// bumped by hand.
const CLAIMED_SPELLINGS_LIVE: usize = 2980;

/// **The round-trip residue the display floor's mechanism clause refuses**.
///
/// The display floor claims a cell only when the r4133 side is our value rounded
/// to the digits r4133 printed (`props_norm::display_is_render`). These pairs
/// carry spellings that are inside the floor and are NOT such a render: the two
/// engines hold doubles further apart than one `%.Ng` print can account for,
/// because r4133 round-trips a value through text before it derives a quantity
/// from it (`load.kva` from a round-tripped `pf`, `vsource.puz*` from a
/// round-tripped Z, `line.b0`/`b1` from a round-tripped C) or because r4133's
/// own getter prints at full precision and the two values differ (`load.*`,
/// `capacitor.normamps`, `reactor.normamps`).
///
/// Every pair is settled `PIN`: r4133's number is reproducible from this
/// engine's own state through the cited round trip, this engine is exact, and
/// the pin is named in [`RP39_PINS`]. No cell of any of them is in scope, so no
/// pair owes a `property` entry.
///
/// Columns: `(pair, example rows, rows on in-scope pairs, r4133 site,
/// disposition)`, the last per [`RP39_DISPOSITIONS`]. The counted columns are
/// re-measured from the walk, and a pin does not make a link claim the row.
/// [`OPEN_RP39`] counts the rows still without a verdict.
const RP39_ROUTING: &[(&str, usize, usize, &str, &str)] = &[
    (
        "autotrans.wdgcurrents",
        1,
        0,
        "AutoTrans.pas:1863 -> :1662 GeTAutoWindingCurrentsResult (solved state)",
        "PIN",
    ),
    (
        "capacitor.cuf",
        1,
        0,
        "Capacitor.pas:1098-1103 -> Utilities.pas:2600-2607 `%-.6g`",
        "PIN",
    ),
    (
        "capacitor.emergamps",
        1,
        0,
        "Capacitor.pas:1109 `%g` (full precision)",
        "PIN",
    ),
    (
        "capacitor.normamps",
        1,
        0,
        "Capacitor.pas:1108 `%g` (full precision)",
        "PIN",
    ),
    (
        "generator.kva",
        1,
        1,
        "generator.pas:3021 `%.6g`, kVA from MakePosSequence's kW token (:3059 -> :3137)",
        "PIN",
    ),
    (
        "generator.kvar",
        1,
        0,
        "generator.pas:3018 `%.6g`, kvar from MakePosSequence's kW token (:3059 -> :3130)",
        "PIN",
    ),
    (
        "generator.maxkvar",
        1,
        1,
        "generator.pas:3019 `%.6g`, maxkvar from MakePosSequence's kW token (:3059 -> :3133)",
        "PIN",
    ),
    (
        "generator.minkvar",
        1,
        1,
        "generator.pas:3020 `%.6g`, minkvar from MakePosSequence's kW token (:3059 -> :3134)",
        "PIN",
    ),
    (
        "line.b0",
        2,
        2,
        "Line.pas:1407 `%.7g` of twopi*f*C0*1e6/units",
        "PIN",
    ),
    (
        "line.b1",
        2,
        2,
        "Line.pas:1406 `%.7g` of twopi*f*C1*1e6/units",
        "PIN",
    ),
    (
        "load.kva",
        13,
        0,
        "Load.pas:2352 `%-g` (full precision), kVA from the kW/kvar tokens (:2326 -> :1145)",
        "PIN",
    ),
    (
        "load.kvar",
        1,
        0,
        "Load.pas:2350 `%-g` (full precision)",
        "PIN",
    ),
    (
        "load.kw",
        2,
        0,
        "Load.pas:2344 `%-g` / MakePosSequence :2326 `%-.5g` of kW/3",
        "PIN",
    ),
    (
        "load.xfkva",
        1,
        0,
        "Load.pas:325 prop 21, no getter arm -> PropertyValue[]; MakePosSequence :2328",
        "PIN",
    ),
    (
        "reactor.emergamps",
        1,
        0,
        "Reactor.pas:1100 `%g` (full precision)",
        "PIN",
    ),
    (
        "reactor.lmh",
        1,
        1,
        "Reactor.pas:1098 `%-.8g` of L*1000",
        "PIN",
    ),
    (
        "reactor.normamps",
        1,
        0,
        "Reactor.pas:1099 `%g` (full precision)",
        "PIN",
    ),
    (
        "reactor.x",
        1,
        1,
        "Reactor.pas:1092 `%-.8g` / MakePosSequence :1145-1201 `%-.5g`",
        "PIN",
    ),
    (
        "reactor.z",
        1,
        1,
        "Reactor.pas:1097 `[%-.8g, %-.8g]` of (R, X) — the same X",
        "PIN",
    ),
    (
        "transformer.emergamps",
        1,
        1,
        "Transformer.pas:1843 `%-.5g`, amps from the round-tripped winding kV (:1982 -> :1130)",
        "PIN",
    ),
    (
        "transformer.normamps",
        1,
        1,
        "Transformer.pas:1842 `%-.5g`, amps from the round-tripped winding kV (:1982 -> :1129)",
        "PIN",
    ),
    (
        "vsource.isc3",
        3,
        3,
        "Vsource.pas:1330 `%-.5g`, Isc3 from the round-tripped BasekV (:1397 -> :752)",
        "PIN",
    ),
    (
        "vsource.mvasc1",
        2,
        2,
        "Vsource.pas:1329 `%-.5g`, MVAsc1 from the round-tripped BasekV (:1397 -> :768)",
        "PIN",
    ),
    (
        "vsource.mvasc3",
        2,
        2,
        "Vsource.pas:1328 `%-.5g`, MVAsc3 from the round-tripped BasekV (:1397 -> :767)",
        "PIN",
    ),
    (
        "vsource.puz0",
        4,
        0,
        "Vsource.pas:1341 `[%-.8g, %-.8g]`, puZ0 from the round-tripped BasekV (:1397 -> :473)",
        "PIN",
    ),
    (
        "vsource.puz1",
        4,
        0,
        "Vsource.pas:1340 `[%-.8g, %-.8g]`, puZ1 from the round-tripped BasekV (:1397 -> :473)",
        "PIN",
    ),
    (
        "vsource.puz2",
        4,
        0,
        "Vsource.pas:1342 `[%-.8g, %-.8g]`, puZ2 from the round-tripped BasekV (:1397 -> :473)",
        "PIN",
    ),
];

/// Example rows the shape-allowlist link claims — **zero, and structurally so**:
/// the census recorded the one r4133-active allowlist gap
/// (`GenDispatcher.weights`) as SHAPE rows, never as value cells, so this link
/// is exercised against `shape.txt` instead. See [`Link::ShapeAllowlist`].
const CLAIMED_SHAPE_ALLOWLIST: usize = 0;
/// **The display floor**: example rows the floor claims that no earlier link
/// took.
///
/// They are the bin-6 rows minus the ones [`RP24_OUT_OF_SCOPE`] rules on, the
/// one [`ECHO_CARVE_OUT_ROUTING`] cell, and the rows on display-class pairs whose
/// in-scope population is empty (`generator.kw`,
/// `capacitor.normamps`/`emergamps`, `line.r1`/`x1`/`rmatrix`/`xmatrix`): a claim
/// says what those spellings are, where `OutOfScope` would only say the r4133
/// compare never looks at them. `autotrans.wdgcurrents` is display-class for one
/// spelling only: its other spellings are the AutoTrans typecast bug
/// ([`Owner::Rp312`], [`RP312_UPSTREAM_BUG`]).
///
/// No row claimed by an earlier link is also inside the floor
/// ([`MULTI_LINK_ROWS`] is 0), so this number is a clean addition to
/// [`CLAIMED_TOTAL`]. The floor's mechanism clause
/// (`props_norm::display_is_render`) refuses the spellings whose r4133 side is no
/// `%.Ng` render of our value, and they are declared to [`Owner::Rp39`]
/// ([`RP39_ROUTING`]).
const CLAIMED_DISPLAY_FLOOR: usize = 1951;

/// Rows for which **more than one** link matches: **0**.
///
/// `props_norm::ECHO_NARROWED` holds each echo row on a mixed pair to the
/// spellings its citation explains, and that set is by construction exactly the
/// rows the typed rules do NOT claim
/// ([`the_narrowed_echo_rows_carry_exactly_the_spellings_the_typed_rules_leave`]),
/// so no row matches both links. As **0** the lock is the tripwire a re-widened
/// echo row (or a deleted narrowing) trips, and the accounting does not lean on
/// chain order for a partition the tables make disjoint.
///
/// The typed-rule half of the mixed pairs is [`MIXED_PAIR_NORM_ROWS`].
const MULTI_LINK_ROWS: usize = 0;

/// The rows of the **mixed** pairs (a `PROPS_NORM_R4133` row and a
/// `PROPS_ECHO_R4133` row on one pair) that the typed rule claims.
///
/// This is a property of `PROPS_NORM_R4133`'s reach on the pairs that also carry
/// an echo row (what the per-pair split literal in
/// [`the_typed_rules_and_the_echo_rows_split_their_shared_pairs_offline`] sums
/// to). It is also the exact complement of `props_norm::ECHO_NARROWED`'s
/// spellings inside those pairs' census rows, so the two locks move together or
/// not at all.
///
/// The mixed pairs: the four mixed bin-1 pairs (`recloser.eventlog`,
/// `regcontrol.idle`, `relay.distreverse`, `relay.reset`), six bin-2-labelled
/// ones (`capcontrol.type`, `fault.bus2`, `invcontrol.mode`, `fuse.switchedobj`,
/// `recloser.switchedobj`, `relay.switchedobj`), `energymeter.peakcurrent`, and
/// the nine off-bin rows (`load.yearly`, `reactor.bus2`,
/// `invcontrol.monvoltagecalc`, `line.wires`, `load.zipv`, `generator.userdata`,
/// `storage.dynadata`, `storagecontroller.seasontargets`/`seasontargetslow`).
const MIXED_PAIR_NORM_ROWS: usize = 135;

/// Declared rows per owner: `(rows, pairs, rows on in-scope pairs)`.
///
/// **`DECLARED_RP22` is `(0, 0, 0)`**: every bin-3 pair and S6 singleton is
/// either claimed by the shipped table or carries a verdict in
/// [`RP22_ROUTING`]. The variant stays so a regression that re-creates the
/// bucket fails here by name.
const DECLARED_RP22: (usize, usize, usize) = (0, 0, 0);
/// **`DECLARED_RP23` is `(0, 0, 0)`**: every row of the echo bucket is claimed
/// by an off-bin `PROPS_NORM_R4133` row ([`CLAIMED_CASE_FOLD`],
/// [`CLAIMED_ARRAY_FORM`]) or by a `PROPS_ECHO_R4133` row ([`CLAIMED_ECHO`]),
/// superseded ([`RP38_SUPERSEDED`]), or carved out to the display floor
/// ([`ECHO_CARVE_OUT_ROUTING`]).
///
/// The variant stays so a regression that re-creates the bucket fails here by
/// name.
const DECLARED_RP23: (usize, usize, usize) = (0, 0, 0);
/// **`DECLARED_RP38` is `(0, 0, 0)`**: this engine renders the live read-only
/// quantity r4133 renders, so the frozen `rust = ''` column of those rows no
/// longer describes it.
///
/// The rows are accounted as **superseded** ([`SUPERSEDED_RP38`],
/// [`RP38_SUPERSEDED`]): on the r4133 channel, the only one this file's chain
/// speaks for, the cells agree outright or are the display class
/// [`Link::DisplayFloor`] claims, and the capi-side `number` vs `''` is excluded
/// by a `SKIP_PROPS_CAPI_ONLY` row pair with its own expected-value pins.
///
/// The variant stays so a regression that re-creates the bucket fails here by
/// name.
const DECLARED_RP38: (usize, usize, usize) = (0, 0, 0);
/// **The frozen rows an engine change superseded on the [`RP38_SUPERSEDED`]
/// pairs**: `(rows, pairs, rows on in-scope pairs)`, the same triple every
/// `DECLARED_*` lock carries. A row that leaves or joins these pairs moves this
/// number.
const SUPERSEDED_RP38: (usize, usize, usize) = (181, 5, 181);
/// **The frozen rows the per-phase SwtControl render superseded**: the same
/// triple, over exactly the [`RP37_SUPERSEDED`] pairs.
///
/// They are the one-token spellings no `ArrayForm` row claims and the
/// [`Owner::Rp35`] rows of the two `swtcontrol` pairs.
const SUPERSEDED_RP37: (usize, usize, usize) = (5, 2, 5);
/// **`DECLARED_RP24` is `(0, 0, 0)`**: every bin-6 row, and the cell
/// [`ECHO_CARVE_OUT_ROUTING`] carves out of `reactor.kvar`'s echo row, is
/// resolved in one of two ways:
///
/// * claimed by the derived floor itself ([`CLAIMED_DISPLAY_FLOOR`]);
/// * declared [`Owner::OutOfScope`] by [`RP24_OUT_OF_SCOPE`], which proves per
///   row, from the pair's own frozen `max_rel_in_scope`, that the spelling has no
///   cell the r4133 compare ever reaches.
///
/// The variant stays so a regression that re-creates the bucket fails here by
/// name.
const DECLARED_RP24: (usize, usize, usize) = (0, 0, 0);
/// **The root-cause bucket** is `(0, 0, 0)`: of the four bin-7 root-cause
/// pairs, `generator.model` is claimed by its echo row ([`Link::Echo`]) and the
/// other three are excluded by `ledger.json` `property` entries
/// ([`RP3_LEDGERED`]). The per-pair split and each verdict are [`RP3_ROUTING`].
///
/// No [`Link::ORDER`] link reads `tests/corpus/ledger.json`, and [`declare`]
/// routes every bin-7 row on [`BIN7_ROOT_CAUSE`] to [`Owner::Rp3`]. What moves
/// the ledgered rows out is the interception [`RP3_LEDGERED`], which reads the
/// ledger live and counts them in their own bucket ([`LEDGERED_RP3`]) between
/// the chain and [`declare`]. Delete an entry from the ledger and the
/// interception's guard reds. Re-declare the row here and the bucket lock reds.
const DECLARED_RP3: (usize, usize, usize) = (0, 0, 0);
/// **The round-trip residue**: the example rows whose gap is inside the floor
/// and whose r4133 side is no `%.Ng` render of our value ([`RP39_ROUTING`]
/// carries the per-pair split and the sites). The third term is the rows on
/// pairs the r4133 compare reaches and that the frozen ceilings do not prove out
/// of scope. The live census measures no in-scope cell on any of them.
///
/// The bucket is *measured*: [`account`] counts every row [`declare`] routes to
/// [`Owner::Rp39`], and what routes a row there is
/// `display_class_but_not_a_render`, a property of the two engines' doubles that
/// an expected-value pin does not touch. A pin settles a pair without moving a
/// row, so the settled state is [`OPEN_RP39`].
const DECLARED_RP39: (usize, usize, usize) = (55, 27, 19);

/// **What is still OPEN in the round-trip residue**: `(rows, pairs, rows on
/// in-scope pairs)` summed over the [`RP39_ROUTING`] entries whose disposition is
/// `OPEN`, i.e. the pairs with no verdict. Every pair is settled `PIN`.
const OPEN_RP39: (usize, usize, usize) = (0, 0, 0);

/// **The RegControl x AutoTrans typecast bug** ([`RP312_UPSTREAM_BUG`]): the
/// `autotrans.wdgcurrents` spellings of the four `controls:autotrans/*` decks,
/// none in scope.
///
/// The gap is a few percent, far outside the display floor, and it is a
/// **solved-state** divergence: r4133's `RegControl` cannot tap an `AutoTrans`
/// at all, so its cells are the UNREGULATED circuit's. The count is a straight
/// sum of the vendored supplement's rows. The ninth spelling of the same pair,
/// the `modes:makeposseq` round-trip residue, stays [`Owner::Rp39`]'s and is
/// counted in [`DECLARED_RP39`].
///
/// The four cases are `engines: "capi_v0145"`, so no `r4133` comparison runs on
/// them and nothing in the tree shrinks this on its own.
const DECLARED_RP312: (usize, usize, usize) = (8, 1, 0);

/// The **measured** minimum of `row gap / display floor` over the eight
/// [`DECLARED_RP312`] rows: `153.4x`
/// (`examples_supplement.txt:127`, `autotrans_both`, rel 3.068976e-02 against
/// the 2e-4 floor; the largest is 376.9x on `:130`). Locked as a `>=` and
/// bracketed above in
/// [`the_regcontrol_autotrans_typecast_rows_have_their_own_owner`], the shape
/// [`RP24_OUT_OF_SCOPE_MIN_RATIO`] was hardened into after a one-sided lock let
/// a mutation through: the number is a **measurement** of how far this verdict
/// sits from the display class, so a population that moved in *either*
/// direction must be re-read, not quietly re-bracketed.
const RP312_MIN_GAP_RATIO: f64 = 153.0;

/// The ledger cause the AutoTrans typecast verdict is filed under.
const RP312_TYPECAST_CAUSE: &str = "regcontrol-autotrans-typecast";

/// How many of the eight [`DECLARED_RP312`] rows the pin reproduces **verbatim,
/// both columns** — `2`: `examples_supplement.txt:128` (`autotrans_reg`, 8 of
/// the 34 cells) and `:134` (`midi_autotrans`, 2 cells), the pin's two legs.
/// Locked by [`the_rp312_pin_quotes_the_vendored_census_cells`] so a
/// re-baselined literal, or a leg quietly dropped, reds instead of narrowing the
/// witness in silence. The other six spellings are declared but not reproduced
/// in-tree (their decks open with a snapshot `Solve`, so r4133's unregulated
/// state needs the RegControl disabled in the deck SOURCE).
const RP312_WITNESSED_ROWS: usize = 2;

/// The four cases the verdict covers and the `skip` entry each one owes the day
/// it gains the `r4133` channel — `(case id, staged entry id)`, the ids of the
/// drafted entries. All four are `engines: "capi_v0145"` today
/// (`tests/corpus/manifests/population.lock.json:74-77`), which is why the
/// entries are staged rather than landed; the pairing is enforced by
/// [`the_autotrans_typecast_cases_pair_their_r4133_channel_with_a_skip_entry`].
const RP312_STAGED_SKIPS: &[(&str, &str)] = &[
    (
        "controls:autotrans/autotrans_both.dss",
        "r4133-autotrans-regcontrol-typecast-autotrans-both",
    ),
    (
        "controls:autotrans/autotrans_reg.dss",
        "r4133-autotrans-regcontrol-typecast-autotrans-reg",
    ),
    (
        "controls:autotrans/midi_autotrans.dss",
        "r4133-autotrans-regcontrol-typecast-midi-autotrans",
    ),
    (
        "controls:autotrans/midi_autotrans_both.dss",
        "r4133-autotrans-regcontrol-typecast-midi-autotrans-both",
    ),
];

/// The **disposition** an [`RP39_ROUTING`] row records: what the settled pair
/// owes the tree, in the shape [`RP3_SETTLED_SHAPES`] carries for
/// [`RP3_ROUTING`]:
///
/// * `PIN`, verdicts `PRECISION_ROUNDTRIP` and `STATE_DIFFERS`: this engine is
///   exact, or differs by a proven precision-class cause, and the whole
///   obligation is an expected-value test naming both numbers. It is cited in
///   [`RP39_PINS`], whose verdict column records which of the two it is.
/// * `FIX`, verdict `PORT_BUG`: this engine computes the wrong number and is
///   fixed in both lanes, witnessed beside the behaviour.
/// * `LEDGER`, verdict `UPSTREAM_BUG`: an OpenDSS bug, never reproduced, so the
///   r4133 channel is excluded by a `property` entry, with its pin in
///   [`LEDGER_ENTRY_PINS`].
/// * `OPEN`: no verdict yet.
const RP39_DISPOSITIONS: &[&str] = &["PIN", "FIX", "LEDGER", "OPEN"];
/// The live-state divergences [`Owner::Rp35`] owns: `line.units`,
/// `line.linecode` and `relay.normal`/`state`.
///
/// Each settled `FIX`: this engine renders r4133's own value on every one of
/// these pairs, so there is nothing to exclude on the r4133 channel. The vendored
/// extracts are a **data lock** on the frozen census, so their `rust` columns
/// still hold the pre-fix spellings (`'none'`, `''`, `'closed'`, `'open'`,
/// `'[closed, open, open, ]'`) and no link of the chain claims them. The
/// `swtcontrol.normal`/`state` rows are superseded instead ([`RP37_SUPERSEDED`],
/// derived from the decks by
/// [`the_swtcontrol_state_census_decomposition_is_read_off_the_corpus`]). The other pairs
/// keep their declared rows until they get the same per-pair live disposition
/// and witness.
///
/// The verdicts in [`RP22_ROUTING`] carry what was decided and what holds each
/// value (`exec::tests::line_fetch::…`, `exec::tests::reduce::…`,
/// `exec::tests::controls::…`, `relay::tests::…`). Each fix moved the live
/// `capi_v0145` channel instead, through property entries outside this
/// accounting ([`LANDED_PROPERTY_ENTRY_PINS`]).
const DECLARED_RP35: (usize, usize, usize) = (5, 4, 2);
/// `OutOfScope` rows must have **zero** in-scope cells: that is the whole claim
/// the marker makes.
///
/// The third number is read per row, not off the pair's `cells_in_scope`
/// alone, which admits the display-class pairs whose PAIR is in scope while these
/// particular spellings are not ([`RP24_OUT_OF_SCOPE`]).
const DECLARED_OUT_OF_SCOPE: (usize, usize, usize) = (221, 21, 0);

/// **The four pairs whose example rows are declared out of scope, with the
/// frozen ceiling that proves it**: `(pair, max_rel_in_scope, rows)`.
///
/// These pairs have a FULL-census bin 7 but an in-scope re-derivation
/// (`max_rel_in_scope < 1e-4`) of bin 6, so [`PairEvidence::effective_bin`]
/// routes the whole pair to the display floor. Their example-row inventory is
/// the full census's, so it also carries the spellings that made the pair bin 7,
/// up to rel 1.0 on `generator.kvar`, which no display floor may claim.
///
/// **The rule that resolves them, and why it is a proof.** `max_rel_in_scope` is
/// the maximum relative gap over *exactly* the cells the r4133 compare reaches
/// (cells on `engines in {both, r4133}` cases). So a spelling whose own gap
/// **exceeds** that maximum cannot sit on a single in-scope cell: it would have
/// raised the maximum. The census's in-scope population is still today's, bar
/// named additions that carry no frozen row:
/// `props_r4133_evidence_lock.rs::the_in_scope_population_is_the_census_one_plus_named_additions`
/// locks both halves and reds on the next in-scope deck until it is named there.
/// The rule is [`row_out_of_scope_by_ceiling`], applied only on these four cited
/// pairs, only to numeric rows, and with a margin: the extracts round
/// `max_rel_in_scope` to two decimals (`%.2e`), so the comparison carries
/// [`CEILING_ROUND_MARGIN`], and the minimum ratio across all rows is asserted
/// separately ([`RP24_OUT_OF_SCOPE_MIN_RATIO`]) so a population that comes near
/// the ceiling reds this file instead of silently widening the scope rule.
///
/// | pair | what the above-ceiling spellings are |
/// |---|---|
/// | `generator.kvar` | the GenDispatcher `weights` registration bug's dispatch split, on the three capi-only `controls:gendispatcher/*` decks |
/// | `capacitor.cuf` | `'[ 4]'` vs `'[ 4E-006]'`, the `makeposseq_shunt` Cuf **unit** divergence |
/// | `storage.kw` | `'-0.333333333333333'` vs `'-1'` |
/// | `storagecontroller.kwneed` | display-class in mechanism but out of scope, the nearest row above the floor's empty band |
const RP24_OUT_OF_SCOPE: &[(&str, f64, usize)] = &[
    ("capacitor.cuf", 4.00e-06, 1),
    ("generator.kvar", 1.55e-06, 102),
    ("storage.kw", 4.90e-06, 1),
    ("storagecontroller.kwneed", 4.96e-06, 1),
];

/// Rows [`RP24_OUT_OF_SCOPE`] accounts for — the sum of its third column.
const RP24_OUT_OF_SCOPE_ROWS: usize = 105;
/// Headroom for the `%.2e` rounding the vendored extracts apply to
/// `max_rel_in_scope`: at worst half a unit in the third significant digit,
/// i.e. 0.5 %. 1 % is twice that and still 277x under the measured margin.
const CEILING_ROUND_MARGIN: f64 = 1.01;
/// The **measured** minimum of `row gap / pair ceiling` over all
/// [`RP24_OUT_OF_SCOPE_ROWS`] rows (`storagecontroller.kwneed`, 1.374769e-03
/// against 4.96e-06). Locked as a `>=` so the rule stays a proof with orders of
/// magnitude to spare, not a boundary call.
const RP24_OUT_OF_SCOPE_MIN_RATIO: f64 = 277.0;

/// `shape.txt` gap names an r4133 `PROPS_015X` row covers: `generator.rneut`,
/// `generator.xneut`, `sensor.action`, `autotrans.xfmrcode` and
/// `gendispatcher.weights`, the last being the only one that fires on the
/// **r4133** channel, the other four on the 0.14.5 capture.
const SHAPE_ALLOWLIST_NAMES: usize = 5;
/// …and the ones closed by porting the property instead (the WindGen surface).
const SHAPE_PORTED_NAMES: usize = 2;

/// The `PROPS_015X` rows that are **active on the r4133 channel**: the property
/// is on the Rust side only, so the r4133 name list cannot carry it and
/// `prop_015x` drops it before any value is compared. Exactly one exists
/// (`GenDispatcher.weights`, the r4133 registration bug). The other r4133 rows
/// are `oracle_only` gaps, inert on r4133 and active on the 0.14.5 capture.
const R4133_ACTIVE_ALLOWLIST_NAMES: usize = 1;

// ---------------------------------------------------------------------------
// The enumerated bin-7 and S6 pair lists. Each list is cross-checked against the
// vendored files below, so a stale list fails a test instead of silently
// widening a marker.
// ---------------------------------------------------------------------------

/// Bin 7's echo pairs (frozen defaults). `PROPS_ECHO_R4133` carries them.
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

/// Bin 7's four root-cause pairs.
const BIN7_ROOT_CAUSE: &[&str] = &[
    "generator.model",
    "gictransformer.r2",
    "swtcontrol.delay",
    "windgen.kvar",
];

/// **The per-pair accounting under [`Owner::Rp3`]'s single bucket**:
/// `(pair, sub-step, example rows, rows on in-scope pairs, verdict)`.
///
/// [`BIN7_ROOT_CAUSE`] names the four pairs [`declare`] routes to
/// [`Owner::Rp3`]. Each row's counted columns are re-measured from the walk, so a
/// pair that silently changes size (or vanishes) reds here instead of being
/// absorbed by the bucket total.
///
/// A settled pair empties its rows only when the tree really holds the
/// exclusion. The three `LEDGER` pairs read `0, 0` because [`RP3_LEDGERED`]
/// counts their example rows against the entries it reads back out of
/// `tests/corpus/ledger.json`. The `ECHO` pair reads `0, 0` because
/// [`Link::Echo`] claims its one example row. The entry stays, because the table
/// must cover all four [`BIN7_ROOT_CAUSE`] pairs, and the guard re-measures the
/// pair from the corpus and insists that its rows exist and that the chain claims
/// every one, so `0, 0` can never mean "the pair quietly vanished".
///
/// **A verdict is typed by its outcome tag.** `OPEN — ` has no verdict. Every
/// other verdict opens with one of [`RP3_SETTLED_SHAPES`]' tags: a ledger entry
/// (`LEDGER`), an echo row (`ECHO`), or a fix in both lanes (`FIX`).
/// [`the_bin7_root_cause_pairs_are_routed_to_their_sub_steps`] checks each tag's
/// own obligations: all must cite the r4133 unit and name their own sub-step
/// column, then `LEDGER` owes its staging clause and its pins, `ECHO` owes the
/// row on its pair, `FIX` owes the Rust site.
const RP3_ROUTING: &[(&str, &str, usize, usize, &str)] = &[
    (
        "generator.model",
        "generator-model",
        0,
        0,
        "ECHO — r4133's `generator.model` getter has no arm for the property and echoes the \
         deck's own typed token (Version8/Source/PCElements/generator.pas:3007-3038), while \
         NCIM moves the live GenModel from 3 to 4 and never moves it back. This engine \
         renders the live field and its NCIM agrees with r4133's live state, so there is no \
         engine change and no ledger entry: the exclusion is the PROPS_ECHO_R4133 row \
         generator.model (EchoParse), witnessed by \
         generator_model_renders_the_live_pv2pq_conversion. Census, derived per case by \
         `the_rp33_census_decomposition_is_read_off_the_corpus`: 2 cells, all 2 in scope, 1 + \
         1 over the 2 converting decks (modes:ncim/ncim_midi.dss, modes:ncim/ncim_pv_pq.dss), \
         each declaring one model=3 generator over 1 step on engines=r4133. \
         modes:ncim/ncim_pq.dss runs NCIM and declares no generator, so it produces no cell, \
         and the corpus's 2 other NCIM decks are kind=large and outside the census \
         population.",
    ),
    (
        "gictransformer.r2",
        "gictransformer-r2",
        0,
        0,
        "LEDGER — both OpenDSS engines build winding 2's conductance from the H-winding \
         percentage (Version8/Source/PDElements/GICTransformer.pas:495) and render \
         `gictransformer.r2` from that field through a LIVE getter, so the value is computed \
         off a mis-derived field, not echoed, and the pair takes no PROPS_ECHO_R4133 row. \
         This engine derives it from the X-winding percentage in both lanes \
         (crates/dss-core/src/elements/pd/gic_transformer/solve.rs). The exclusion is two \
         per-case r4133 `property` entries in tests/corpus/ledger.json, \
         gic-pct-r2-honoured-gictransformer-r4133-props and \
         gic-pct-r2-honoured-midi-r4133-props, beside the capi entries \
         gic-pct-r2-honoured-gictransformer-capi-props and \
         gic-pct-r2-honoured-midi-capi-props, witnessed by \
         gictransformer_r2_honours_the_x_winding_percentage and \
         gictransformer_r2_honours_the_x_winding_percentage_on_the_ring. RP3_LEDGERED \
         intercepts this pair's example row, so it is excluded, not declared. Census, derived \
         per element by `the_gictransformer_r2_census_decomposition_is_read_off_the_corpus`: \
         2 cells, all 2 in scope, 1 + 1 over the 2 %R decks (asymmetric:gic/gic_midi.dss tg5, \
         asymmetric:gic/gictransformer_gic.dss tg3), each `%R1=0.2 %R2=0.15 kvll1=345 \
         kvll2=138 mva=300 type=Auto` over 1 step on engines=r4133, so ours is ZBase2*%R2/100 \
         = '0.09522' against ZBase2*%R1/100 = '0.12696'. 2 in-scope cells over exactly 2 \
         cases, hence exactly two entries and no more. The corpus's other 20 GICTransformers \
         are ohms-specified and take the other branch, 20 of them on r4133-gating cases, and \
         produce no cell. The class-wide pairs gictransformer.enabled and \
         gictransformer.pctperm record 22 cells / 22 in scope, i.e. sum(declared) x steps \
         over the same population, plus one named channel move. An OpenDSS report is filed.",
    ),
    (
        "swtcontrol.delay",
        "swtcontrol-delay",
        0,
        0,
        "LEDGER — r4133 never wires `swtcontrol.delay`: its Edit has no arm for the property \
         (Version8/Source/Controls/SwtControl.pas:195-218), so the typed value reaches only \
         the stored text while the live delay keeps its 120 s default, and the LIVE getter \
         renders that default. dss_capi 0.14.5 and this engine wire the property, so there is \
         no engine change, and the getter is live, so the pair takes no echo row. The \
         exclusion is two per-case r4133 `property` entries in tests/corpus/ledger.json, \
         r4133-swtcontrol-delay-ignored-time and r4133-swtcontrol-delay-ignored-midi, \
         witnessed by swtcontrol_delay_wires_the_property and \
         swtcontrol_delay_wires_the_property_on_the_midi_tie. RP3_LEDGERED intercepts this \
         pair's two example rows, so they are excluded, not declared. Census, derived per \
         case by `the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus`: the \
         `'0.25'` spelling's 36 cells are 24 in scope, 12 + 12 over the two r4133 decks, plus \
         12 on capi_v0145's swtcontrol_lock.dss, and the `'0'` spelling's 6 cells are three \
         capi_v0145 IEEE_519 copies. 24 in-scope cells over exactly 2 cases, hence exactly \
         two entries and no more. An OpenDSS report is filed.",
    ),
    (
        "windgen.kvar",
        "windgen-kvar",
        0,
        0,
        "LEDGER — r4133's `windgen.kvar` getter is wired and LIVE but reads the wrong field: \
         it renders the DISPATCHED Q (Version8/Source/PCElements/WindGen.pas:2896), while the \
         property documents the base kvar and the Edit stores the typed value as the base. \
         r4133's own Generator renders the base through the identical getter, and so does \
         this engine for both classes (crates/dss-core/src/elements/pc/windgen/accessors.rs). \
         The getter is live, so the pair takes no echo row. The rendered zero itself comes \
         from a second OpenDSS bug, the missing constant-Q dispatch arm, which this engine \
         fixes in both lanes and which is excluded per case under cause \
         windgen-qmode0-no-arm. The exclusion here is four per-case r4133 `property` entries \
         in tests/corpus/ledger.json, r4133-windgen-kvar-dispatched-daily, \
         r4133-windgen-kvar-dispatched-delta, r4133-windgen-kvar-dispatched-dyn and \
         r4133-windgen-kvar-dispatched-dynfault, witnessed by \
         windgen_kvar_renders_the_base_on_the_daily_deck, \
         windgen_kvar_renders_the_base_on_the_delta_snapshot, \
         windgen_kvar_renders_the_base_on_the_dynamics_deck and \
         windgen_kvar_renders_the_base_on_the_fault_ride_through_deck. RP3_LEDGERED \
         intercepts this pair's three example rows, so they are excluded, not declared. \
         Census, derived per case by \
         `the_windgen_kvar_census_decomposition_is_read_off_the_corpus`: 4 cells, all 4 in \
         scope, 1 + 1 + 1 + 1 over the 4 diverging decks ('726.483157256779' x1, \
         '854.95263026673' x2, '986.05231553659' x1). No deck types kvar= at all, so every \
         value is a pf=/kVA= side effect, and modes:windgen/windgen_snap.dss derives \
         kvar_base 0 from pf=1.0 and produces no cell, while 2 further corpus decks declare a \
         WindGen and are held in skipped_oracle_issue.json. 4 in-scope cells over exactly 4 \
         cases, hence exactly four entries and no more. An OpenDSS report is filed.",
    ),
];

/// **The rows the landed r4133 `ledger.json` `property` entries exclude**:
/// `(pair, sub-step, example rows, rows on in-scope pairs, the entry ids that
/// exclude them)`.
///
/// None of [`Link::ORDER`]'s four links reads `tests/corpus/ledger.json`, and
/// [`declare`] routes every bin-7 row on [`BIN7_ROOT_CAUSE`] to [`Owner::Rp3`].
/// The interception sits between the chain and [`declare`], exactly where
/// [`RP38_SUPERSEDED`]'s does and for the same reason: a row whose disposition
/// is decided OUTSIDE the four links is counted against a cited table whose
/// evidence is checkable, never silently absorbed by a bucket total.
///
/// **What it asserts.** The entries are read LIVE out of
/// `tests/corpus/ledger.json`
/// ([`the_ledgered_rows_are_excluded_by_entries_that_are_really_in_the_ledger`]):
/// each id must exist, be `channel: "r4133"` + `kind: "divergence"`, carry a
/// `property` scope whose `name_re` is the anchored-literal
/// `(?i)^<class>\.<element>\.<prop>$` form naming THIS pair, and pin both an
/// exact `rust` and an exact `oracle`. Delete an entry from the ledger and this
/// guard reds before the corpus gate can go quietly green on a pair nothing
/// excludes any more. Land a *new* `property`-scoped `r4133` entry without an
/// accounting row here and [`the_r4133_property_entries_are_the_root_cause_eight`]
/// reds instead. The counted columns are re-measured from the walk, so a pair
/// that changes size or vanishes reds too.
///
/// **Scope.** This table covers exactly the eight root-cause entries
/// ([`LEDGER_ENTRY_PINS`]). It is NOT a general "any ledger entry claims its
/// pair" rule: [`Owner::Rp35`]'s `line.units` is excluded by a `capi_v0145`
/// entry, and its retirement is a separate decision.
const RP3_LEDGERED: &[(&str, &str, usize, usize, &str)] = &[
    (
        "gictransformer.r2",
        "gictransformer-r2",
        1,
        1,
        "gic-pct-r2-honoured-gictransformer-r4133-props, gic-pct-r2-honoured-midi-r4133-props",
    ),
    (
        "swtcontrol.delay",
        "swtcontrol-delay",
        2,
        2,
        "r4133-swtcontrol-delay-ignored-time, r4133-swtcontrol-delay-ignored-midi",
    ),
    (
        "windgen.kvar",
        "windgen-kvar",
        3,
        3,
        "r4133-windgen-kvar-dispatched-daily, r4133-windgen-kvar-dispatched-delta, \
         r4133-windgen-kvar-dispatched-dyn, r4133-windgen-kvar-dispatched-dynfault",
    ),
];

/// **The `property`-scoped `r4133` ledger entries outside the frozen census**
/// — `(entry id, case, the test file of its pin, pin)`.
///
/// Each excludes a divergence the engine took up after the census of this
/// file was frozen, at spellings no frozen row holds, so it retires no row and
/// moves no bucket: [`RP3_LEDGERED`] cannot vouch for it, and the entry is
/// accounted for here instead. [`the_ledger_entries_outside_the_census_retire_no_frozen_row`]
/// checks each one as strictly as [`RP3_LEDGERED`] checks its own (an `r4133`
/// `divergence` on the cited case, anchored literal property scopes, both
/// numbers pinned) and that its pin exists, and the two rails that hold the set
/// of landed entries ([`the_ledgered_rows_are_excluded_by_entries_that_are_really_in_the_ledger`],
/// [`the_r4133_property_entries_are_the_root_cause_eight`]) count it as accounted.
const LEDGER_ENTRIES_OUTSIDE_THE_CENSUS: &[(&str, &str, &str, &str)] = &[
    (
        "r4133-geometry-weakest-phase-lgeo",
        "asymmetric:line/line_geometry_asym.dss",
        "ratings.rs",
        "a_geometry_line_is_rated_by_its_weakest_phase_conductor",
    ),
    (
        "r4133-geometry-weakest-phase-lfc",
        "asymmetric:line/line_fullcarson_asym.dss",
        "ratings.rs",
        "a_geometry_line_is_rated_by_its_weakest_phase_conductor",
    ),
    (
        "r4133-geometry-weakest-phase-loh",
        "asymmetric:combo/midi_geometry_cable_asym.dss",
        "ratings.rs",
        "a_geometry_line_is_rated_by_its_weakest_phase_conductor",
    ),
    (
        "r4133-two-winding-kva-ratings-autoxc",
        "asymmetric:autotrans/autotrans_xfmrcode.dss",
        "rating_refusals.rs",
        "a_typed_non_finite_kva_or_device_rating_is_refused",
    ),
    (
        "r4133-storage-charging-efficiency-once-peakshavelow",
        "solvable_now:Version8/Distrib/Examples/StorageControllerTechNote/PeakShaveDch_PeakShaveLow_Ch/PeakShaveDch_PeakShaveLow_ChRun.dss",
        "rating_refusals.rs",
        "the_dc_power_of_an_efficiency_curve_is_solved_on_the_segment_that_holds_it",
    ),
];

/// The directory that holds the test files of the pins of
/// [`LEDGER_ENTRIES_OUTSIDE_THE_CENSUS`].
const OUTSIDE_THE_CENSUS_PINS: &str = "crates/dss-core/src/exec/tests";

/// **The frozen spellings a ledgered pair retires that NO landed entry names**:
/// `(pair, rust, r4133, why it is sound anyway)`.
///
/// [`RP3_LEDGERED`] intercepts per PAIR, while a ledger `property` scope pins an
/// exact `(rust, oracle)` spelling (`corpus_gate/ledger.rs`'s exact-pair rule),
/// so a pair may hold a frozen census row no entry excludes. That is sound only
/// while such a spelling has no in-scope cell: an in-scope one would reach the
/// r4133 value compare with nothing to exclude it and RED the gate, the correct
/// outcome, but a fact the accounting must state rather than imply.
///
/// Both directions are checked by
/// [`the_ledgered_rows_are_excluded_by_entries_that_are_really_in_the_ledger`]:
/// every unnamed spelling must be listed here, and every row here must still be
/// an unnamed spelling of a ledgered pair.
const RP3_LEDGERED_UNNAMED: &[(&str, &str, &str, &str)] = &[(
    "swtcontrol.delay",
    "0",
    "120",
    "the SAME divergence as the two entries, at another typed value: the three \
     `Version8/Distrib/Examples/**/IEEE_519.DSS` decks type `Delay=0.0` (line \
     45-46 there), which this engine honours and renders `0`, while r4133 ignores \
     the property and keeps its default delay. All 6 cells sit on \
     `engines: capi_v0145` decks with no in-scope cell in the claims census, so no \
     entry is owed. If one ever becomes in scope the value compare reds, which is \
     the review this row exists to make loud.",
)];

/// **The rows the landed `ledger.json` entries exclude**: `(rows, pairs, rows
/// on in-scope pairs)`, the same triple every `DECLARED_*` lock carries, counted
/// over exactly the [`RP3_LEDGERED`] pairs. A row that leaves or joins these
/// pairs moves this number.
const LEDGERED_RP3: (usize, usize, usize) = (6, 3, 6);

/// **The outcome tags a settled [`RP3_ROUTING`] verdict may open with**:
/// `(tag, the obligation the tag carries)`, one row per settled outcome: a bug of
/// this engine **fixed in both lanes**, an OpenDSS echo **excluded + pinned**,
/// or an OpenDSS bug **reported** with its exclusion + pin.
///
/// An unknown tag is a hard error naming this table, so a new outcome extends
/// the table with its obligations instead of loosening the guard. The
/// obligations are enforced in
/// [`the_bin7_root_cause_pairs_are_routed_to_their_sub_steps`]. The shared ones
/// (an r4133 unit citation, the sub-step's own name) are checked for every tag
/// before the per-tag branch.
const RP3_SETTLED_SHAPES: &[(&str, &str)] = &[
    (
        "LEDGER",
        "an OpenDSS bug excluded by per-case `ledger.json` `property` entries: the verdict \
         names tests/corpus/ledger.json and each entry id, owns at least one \
         LEDGER_ENTRY_PINS witness and names each one, and its pair carries NO echo row",
    ),
    (
        "ECHO",
        "an OpenDSS echo excluded by a PROPS_ECHO_R4133 row: the row exists on the pair and \
         cites its own pin, so the pair owns no LEDGER_ENTRY_PINS witness",
    ),
    (
        "FIX",
        "a bug of this engine fixed in both lanes: the verdict names the Rust site \
         (`crates/dss-core/src/`) and says `both lanes`. Nothing is excluded on the r4133 \
         channel, so the pair carries no echo row and no r4133 entry. A fix that also moves \
         this engine off the pinned 0.14.5 engine lands its own `capi_v0145` ledger entries \
         in the same commit",
    ),
];

/// **The `swtcontrol.delay` census decomposition, as data**: every corpus case
/// that declares a `SwtControl` **and** types a `delay=`, with the two facts its
/// deck carries: how many controls it declares, and the token it types.
///
/// The pair's cell count, its in-scope share and hence the number of ledger
/// entries all rest on this decomposition.
/// [`the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus`] derives every one of
/// them: the deck files give the control counts and the typed values,
/// `population.lock.json` gives each case's `steps=`/`engines=`, and the
/// products are reconciled against the frozen census both in total and per
/// spelling.
///
/// The table is a *claim of completeness*, not a filter: the test walks all
/// `.dss` files under `tests/corpus` and fails if any deck outside these two
/// tables declares a `SwtControl`, so a corpus that grows one reds here.
const RP31_DELAY_CASES: &[(&str, usize, &str)] = &[
    ("controls:swtcontrol/midi_swtcontrol.dss", 1, "0.25"),
    ("controls:swtcontrol/swtcontrol_lock.dss", 1, "0.25"),
    ("controls:swtcontrol/swtcontrol_time.dss", 1, "0.25"),
    (
        "solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS",
        2,
        "0.0",
    ),
    (
        "solvable_now:Version8/Distrib/Examples/HarmonicsVariableLoad/IEEE_519.DSS",
        2,
        "0.0",
    ),
    (
        "solvable_now:Version8/Distrib/Examples/Matlab/HarmonicT_MATLAB/IEEE_519.DSS",
        2,
        "0.0",
    ),
];

/// The other side of [`RP31_DELAY_CASES`]' completeness claim: corpus cases that
/// declare a `SwtControl` and type **no** `delay=`, so this engine renders the
/// same `120` r4133 does and the census records **no cell at all**.
///
/// `civanlar.dss` is the load-bearing one: its controls sit on an
/// `engines: r4133` case, so they would be in-scope cells if our unset render
/// differed. Their absence from the census shows that the divergence is the
/// *typed* value and nothing else (also read by the pin
/// `swtcontrol_delay_wires_the_property`).
const RP31_NO_DELAY_CASES: &[(&str, usize)] = &[
    ("modes:makeposseq/makeposseq_ctrl.dss", 1),
    (
        "solvable_now:Version8/Distrib/Examples/civinlar model/civanlar.dss",
        16,
    ),
];

/// **The `windgen.kvar` census decomposition, as data**: every corpus **case**
/// that declares a `WindGen`, with the declaration tokens its base kvar is
/// derived from: `(case, WindGens declared, kW=, pf=, kVA=)`, `""` for a token
/// the deck never types. [`the_windgen_kvar_census_decomposition_is_read_off_the_corpus`]
/// derives the pair's cells and ledger entries from it.
///
/// **No deck types `kvar=`**, neither on a `New` line nor on a later `Edit`,
/// which [`element_tokens`] reads too. Every value in the census is a *side
/// effect* of the deck's `pf=` or `kVA=`, which is why `windgen_snap.dss` carries
/// no cell at all: `pf=1.0` makes the base zero and both engines print `0`. That
/// is a value coincidence, and the pin
/// `windgen_kvar_renders_the_base_on_the_delta_snapshot` measures the difference.
///
/// **None of the five types `QMode=` either** ([`windgen_dispatch`], asserted):
/// r4133's steady-state dispatch has no arm for the default constant-Q mode and
/// dispatches zero, which is why the value r4133 renders as `kvar` is `0` on all
/// of them. This engine renders `kvar_base`, which no dispatch writes, and its
/// solved Q follows the constant-Q mode (excluded per case under cause
/// `windgen-qmode0-no-arm`).
///
/// The table is a *claim of completeness*: the test walks every `.dss` under
/// `tests/corpus` and fails if a deck outside this table and
/// [`RP32_WINDGEN_SKIPPED_DECKS`] declares a `WindGen`, in any spelling
/// [`element_scope`] accepts, quoted included.
const RP32_WINDGEN_CASES: &[(&str, usize, &str, &str, &str)] = &[
    ("modes:windgen/windgen_daily.dss", 1, "3000", "0.95", ""),
    ("modes:windgen/windgen_dyn.dss", 1, "1500", "", "1800"),
    ("modes:windgen/windgen_dyn_fault.dss", 1, "1500", "", "1800"),
    ("modes:windgen/windgen_snap.dss", 1, "1500", "1.0", ""),
    ("modes:windgen/windgen_snap_delta.dss", 1, "1500", "0.9", ""),
];

/// The other half of [`RP32_WINDGEN_CASES`]' completeness claim: the corpus
/// **decks** that declare a `WindGen` and are **not** in the population at all,
/// held out by `skipped_oracle_issue.json`, so they are no case, own no cell and
/// owe no ledger entry.
///
/// Both are the vendored `WindGenerator` examples, held for the same reason
/// ([`RP32_SKIPPED_TAG`]): they are multi-step runs the pinned 0.14.5 capture
/// cannot gate. Each types `kVA=1200.0` at PF 0.88, so each derives a
/// **nonzero** base where the population's own clean deck derives 0. Promoting
/// either one would add in-scope cells and change the pair's entry count, which
/// is why the exclusion is asserted rather than assumed.
///
/// **What is NOT claimed: what r4133 renders on these two.** They are out of the
/// population, and both type `QMode=2` with a real `VV_Curve=`
/// ([`windgen_dispatch`], asserted below), i.e. the volt-var arm rather than the
/// zero dispatch the five census decks take. Whether their render would diverge
/// is unmeasured.
///
/// Deck paths, not case ids: these have no row in `population.lock.json`.
const RP32_WINDGEN_SKIPPED_DECKS: &[(&str, usize, &str, &str, &str)] = &[
    (
        "electricdss-tst/Version8/Distrib/Examples/WindGenerator/WindGen_GFL_Dynamics/\
         Run_IEEE123Bus_GFLDaily.DSS",
        1,
        "",
        "",
        "1200.0",
    ),
    (
        "electricdss-tst/Version8/Distrib/Examples/WindGenerator/WindGen_QSTS/\
         Run_IEEE123Bus_GFLDaily.DSS",
        1,
        "",
        "0.88",
        "1200.0",
    ),
];

/// **The `generator.model` census decomposition, as data**: every corpus
/// **case** in the census population that runs `Set algorithm=NCIM`, with the
/// two facts its deck carries: `(case, Generators declared, the `model=` token
/// typed on them)`. [`the_rp33_census_decomposition_is_read_off_the_corpus`]
/// derives the pair's echo-row cells and its exposure on r4133-only cases from
/// it.
///
/// **The mechanism token is deck-level, not element-level.** What makes a
/// generator's live `GenModel` leave the deck's typed token is the NCIM PV to PQ
/// conversion, so the sweep's filter is `Set algorithm=NCIM` ([`sets_ncim`]) and
/// the element read runs only on the survivors. The test asserts two
/// consequences: a deck that declares no generator produces no cell
/// (`ncim_pq.dss`, the control), and a generator that does not type `model=3`
/// is not convertible at all, since the default model 1 never enters the PV path.
const RP33_NCIM_CASES: &[(&str, usize, &str)] = &[
    ("modes:ncim/ncim_midi.dss", 1, "3"),
    ("modes:ncim/ncim_pq.dss", 0, ""),
    ("modes:ncim/ncim_pv_pq.dss", 1, "3"),
];

/// The other half of [`RP33_NCIM_CASES`]' completeness claim: the corpus cases
/// that run NCIM over `model=3` generators and are **outside the census
/// population**, as `(case, the redirected deck that declares them, Generators
/// declared there, the `model=` token)`.
///
/// Both are `kind: "large"`, and the census population is every live,
/// **non-large**, non-pending/abort/defer case. So the frozen count is exact
/// **over the census population**, and these two add nothing to it *by
/// construction of that population*. The test asserts the `kind` from
/// `population.lock.json` rather than taking it on trust.
///
/// Their generators live in **redirected** files, which is why the second column
/// exists: a single-file [`element_tokens`] read on the master returns 0. The
/// test reads both, 0 on the master and the real count on the sibling.
const RP33_NCIM_HELD_OUT: &[(&str, &str, usize, &str)] = &[
    (
        "solvable_now:Version8/Distrib/Examples/NCIM/Xmission_System_Kundur2Area/Master.dss",
        "electricdss-tst/Version8/Distrib/Examples/NCIM/Xmission_System_Kundur2Area/Generators.DSS",
        3,
        "3",
    ),
    (
        "solvable_now:Version8/Distrib/IEEETestCases/IEEE118Bus/master_file.dss",
        "electricdss-tst/Version8/Distrib/IEEETestCases/IEEE118Bus/generators.dss",
        53,
        "3",
    ),
];

/// **The `gictransformer.r2` census decomposition, as data**: **every**
/// `GICTransformer` the corpus declares, one row each, with the resistance spec
/// its own element scope types: `(case, element, %R1=, %R2=, R1=, R2=, kvll2=,
/// mva=)`, `""` for a token the deck never types.
///
/// It is per **element** rather than per case because the divergence is per
/// element: the `%R`-specified GICTransformers carry a cell while their
/// ohms-specified siblings on the very same decks carry none.
///
/// **What the rows are for.** Both OpenDSS engines derive winding 2's
/// conductance on the `%R` branch from the **H**-winding percentage, so a cell
/// exists exactly where an element is `%R`-specified **and** its two
/// percentages differ: this engine's `ZBase2*%R2/100` is then rendered against
/// the OpenDSS engines' `ZBase2*%R1/100` by the same `%.8g` getter.
/// [`the_gictransformer_r2_census_decomposition_is_read_off_the_corpus`] derives both sides
/// from the tokens below instead of transcribing the frozen spelling.
///
/// **The ohms rows are load-bearing.** Most GICTransformers take the `R1=`/`R2=`
/// path, which both engines get right, and none of those on r4133-gating cases
/// produces a divergent cell: the `%R` path is the whole divergence class.
/// `T5`/`T12`/`T14`/`T15` are `type=Auto` like `tg3`/`tg5` and still clean, so
/// the connection type is not the discriminator either.
///
/// A GSU never types `R2=` at all (`tg1`, `T1`, …): winding 2 keeps the default
/// `%R1 = %R2 = 0.2`, so both engines land on the same `ZBase2*0.2/100`.
const RP34_GIC_ELEMENTS: &[GicDecl] = &[
    GicDecl::ohms(GIC_MIDI, "tg1", "0.12", ""),
    GicDecl::ohms(GIC_MIDI, "tg3", "0.2", "0.1"),
    GicDecl::pct(GIC_MIDI, "tg5", "0.2", "0.15", "138", "300"),
    GicDecl::ohms(GIC_MICRO, "tg1", "0.12", ""),
    GicDecl::ohms(GIC_MICRO, "tg2", "0.2", "0.1"),
    GicDecl::pct(GIC_MICRO, "tg3", "0.2", "0.15", "138", "300"),
    GicDecl::ohms("modes:makeposseq/makeposseq_gic.dss", "gt", "0.1", ""),
    GicDecl::ohms(GIC_EXAMPLE, "t1", "0.1", ""),
    GicDecl::ohms(GIC_EXAMPLE, "t2", "0.2", "0.1"),
    GicDecl::ohms(GIC_EXAMPLE, "t3", "0.1", ""),
    GicDecl::ohms(GIC_EXAMPLE, "t4", "0.1", ""),
    GicDecl::ohms(GIC_EXAMPLE, "t5", "0.04", "0.06"),
    GicDecl::ohms(GIC_EXAMPLE, "t6", "0.15", ""),
    GicDecl::ohms(GIC_EXAMPLE, "t7", "0.15", ""),
    GicDecl::ohms(GIC_EXAMPLE, "t8", "0.04", "0.06"),
    GicDecl::ohms(GIC_EXAMPLE, "t9", "0.04", "0.06"),
    GicDecl::ohms(GIC_EXAMPLE, "t10", "0.10", ""),
    GicDecl::ohms(GIC_EXAMPLE, "t11", "0.10", ""),
    GicDecl::ohms(GIC_EXAMPLE, "t12", "0.04", "0.06"),
    GicDecl::ohms(GIC_EXAMPLE, "t13", "0.2", "0.1"),
    GicDecl::ohms(GIC_EXAMPLE, "t14", "0.04", "0.06"),
    GicDecl::ohms(GIC_EXAMPLE, "t15", "0.04", "0.06"),
];

/// One `GICTransformer` declaration as [`RP34_GIC_ELEMENTS`] records it: the
/// case whose deck writes it, its name, and the six [`GIC_KEYS`] tokens split
/// into the three groups that mean something.
///
/// A struct rather than an eight-tuple because the columns are not
/// interchangeable — swapping `%R2` for `R2` in a tuple literal would be a
/// silent re-classification of the element's *branch*, and the two constructors
/// below make that swap unspellable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GicDecl {
    /// The corpus case whose deck declares it.
    case: &'static str,
    /// The element name, lower-cased as [`gictransformer_elements`] reads it.
    name: &'static str,
    /// `%R1=` and `%R2=` — both `""` on an ohms-specified declaration.
    pct: (&'static str, &'static str),
    /// `R1=` and `R2=` — both `""` on a `%R`-specified one, and the second is
    /// `""` on a GSU, which never types a winding-2 resistance at all.
    ohms: (&'static str, &'static str),
    /// `kvll2=` and `mva=`, the winding-2 base — typed only where the
    /// percentages are (an ohms declaration never reaches `FZBase2`).
    base: (&'static str, &'static str),
}

impl GicDecl {
    /// An ohms-specified declaration: `FpctRSpecified := FALSE`
    /// (`Version8/Source/PDElements/GICTransformer.pas:343`), so
    /// `RecalcElementData` runs its **reverse** branch (`:497-498`) and the
    /// getter inverts back exactly what the deck typed — no cell, on any engine.
    const fn ohms(
        case: &'static str,
        name: &'static str,
        r1: &'static str,
        r2: &'static str,
    ) -> Self {
        Self {
            case,
            name,
            pct: ("", ""),
            ohms: (r1, r2),
            base: ("", ""),
        }
    }

    /// A `%R`-specified declaration: `FpctRSpecified := TRUE` (`:349`), so the
    /// **forward** branch runs and its winding-2 line reads `FPctR1` (`:495`) —
    /// a cell wherever the two percentages differ.
    const fn pct(
        case: &'static str,
        name: &'static str,
        pct_r1: &'static str,
        pct_r2: &'static str,
        kvll2: &'static str,
        mva: &'static str,
    ) -> Self {
        Self {
            case,
            name,
            pct: (pct_r1, pct_r2),
            ohms: ("", ""),
            base: (kvll2, mva),
        }
    }

    /// The six tokens in [`GIC_KEYS`] order — the shape
    /// [`gictransformer_elements`] measures off the deck.
    fn tokens(&self) -> [String; 6] {
        [
            self.pct.0,
            self.pct.1,
            self.ohms.0,
            self.ohms.1,
            self.base.0,
            self.base.1,
        ]
        .map(str::to_string)
    }
}

/// The three multi-element GIC decks, spelled once each — fifteen of
/// [`RP34_GIC_ELEMENTS`]' twenty-two rows sit on the vendored feeder alone.
const GIC_EXAMPLE: &str = "solvable_now:Version8/Distrib/Examples/GICExample/GIC_Example.dss";
/// The 6-substation ring (`tg5` carries one of the pair's two cells).
const GIC_MIDI: &str = "asymmetric:gic/gic_midi.dss";
/// The three-type micro deck (`tg3` carries the other).
const GIC_MICRO: &str = "asymmetric:gic/gictransformer_gic.dss";

/// The two class-wide `GICTransformer` pairs the `gictransformer.r2`
/// decomposition reconciles its population against, with `(cells, cells in
/// scope)` from `bins.tsv`: a cross-check of the declaration count and of the
/// `steps=` read that owes nothing to the `%R` arithmetic.
///
/// Both pairs are answered by **every** GICTransformer on every step, whatever
/// its resistance spec, so their cell counts must be `Σ declared × steps` over
/// the corpus and their in-scope counts the same sum restricted to the
/// r4133-gating cases. If the population moved, these two numbers move with it.
const RP34_CLASS_WIDE_PAIRS: &[(&str, usize, usize)] = &[
    ("gictransformer.enabled", 22, 21),
    ("gictransformer.pctperm", 22, 21),
];

/// **The one named drift from the frozen split**: the GICTransformer `gt` sits on
/// its own `engines: "r4133"` deck (`modes:makeposseq/makeposseq_gic.dss`),
/// while the frozen census saw it on the capi-only
/// `modes:makeposseq/makeposseq_shunt.dss`. The pinned dss_capi 0.14.5 is
/// nondeterministic across processes on any deck that holds a GICTransformer,
/// which is why the element has its own r4133 deck. The live derivation counts
/// one more in-scope declaration than the census recorded. The frozen extracts
/// are a data lock and are never edited, so the delta is named here, and any
/// OTHER population move lands on top of it and reds.
const RP34_IN_SCOPE_MOVED_BY_G14A: usize = 1;

/// The manifest that holds [`RP32_WINDGEN_SKIPPED_DECKS`] out of the population,
/// repo-root-relative.
const SKIPPED_ORACLE_ISSUE: &str = "tests/corpus/manifests/skipped_oracle_issue.json";
/// The tag both skipped WindGen decks carry there: a limitation of the pinned
/// 0.14.5 multi-step capture, not anything about `kvar`.
const RP32_SKIPPED_TAG: &str = "capi015_multistep_limitation";

/// Bin-7 pairs the **supplement** carries, each an echo-table row rather than a
/// root-cause pair:
///
/// * `generator.d`: **not an echo**. r4133 never initialises the damping field
///   the property reads, so the frozen `'0'` is r4133's own live value, and its
///   dynamics run with zero damping against the property's documented default
///   of 1.0, an OpenDSS bug. dss_capi 0.14.5 and this engine use the documented
///   default, so the pair is `EchoCategory::LiveSemanticsDiffer` plus the
///   expected-value pin `generator_d_renders_the_documented_damping_default`
///   (`crates/dss-core/tests/props_r4133_pins.rs`), never a silent
///   `EchoDefault`;
/// * `autotrans.pctperm` / `autotrans.repair`: r4133 freezes `'100'` / `'36'`
///   at construction and its getter re-renders only the first PD-tail slots, so
///   these two answer the stored property text;
/// * `regcontrol.revthreshold`: r4133's getter answers the stored property
///   text, the default `'100'` where no deck writes the property and the deck's
///   own token where one does, while the live value is negated.
const BIN7_ECHO_SUPPLEMENT: &[&str] = &[
    "autotrans.pctperm",
    "autotrans.repair",
    "generator.d",
    "regcontrol.revthreshold",
];

/// **The S6 singletons**: the pairs flagged for individual source
/// investigation, next to the eight bin-3 pairs (derived from `bins.tsv` and
/// cross-checked in [`the_enumerated_pair_lists_still_describe_the_vendored_evidence`]).
/// A pair on this list is resolved **whatever bin its cells fall in**: by an
/// `EnumSynonym` row, an echo row, or a live-state verdict.
///
/// Every pair below is either claimed by the shipped table or carries a verdict
/// in [`RP22_ROUTING`], and [`every_s6_and_bin3_pair_is_claimed_or_routed`] proves it
/// with no third state, so `Owner::Rp22`'s bucket is empty ([`DECLARED_RP22`]).
///
/// `invcontrol.monbus` / `monbusesvbase` carry an `ArrayForm` row and every
/// census spelling folds (bracketed vs bare, equal token counts).
/// `swtcontrol.normal` / `state` carry none: this engine renders per phase, and
/// their frozen example rows are superseded ([`RP37_SUPERSEDED`]).
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

/// **The S6 and bin-3 verdicts: one row per pair to route, with the r4133 site
/// that decides it.** It is consulted *first* in [`declare`], so a pair here
/// lands in its verdict's bucket.
///
/// * **`EnumSynonym` row**: not here at all. Those five pairs
///   (`vsource`/`isource` × `scantype`/`sequence`, plus
///   `invcontrol.voltage_curvex_ref`) are *claimed* by `PROPS_NORM_R4133` and
///   never reach [`declare`]. Same for `invcontrol.monbus`/`monbusesvbase`,
///   fully claimed by their `ArrayForm` rows.
///   [`every_s6_and_bin3_pair_is_claimed_or_routed`] makes those two silences legible.
/// * **echo, [`Owner::Rp23`]**: r4133's string echoes the stored property text
///   (no getter arm for the index), or both sides render live but the r4133
///   render is an OpenDSS quirk whose root cause is established and whose
///   answer here is proven right. The echo table carries the cited exclusion row
///   (plus the expected-value pin where no capi witness exists).
/// * **live-state divergence, [`Owner::Rp35`]**: the two engines' live state or
///   modelling differs.
///
/// Every row's `cite` names the r4133 site the verdict was read from.
const RP22_ROUTING: &[(&str, Owner, &str)] = &[
    // --- bin 3, the four pairs that are NOT enum synonyms --------------------
    // `'close'` vs `'open'` is a stale STORE, not a state disagreement: r4133's
    // getter has no arm for `Action`, so it echoes the raw token stored before
    // the edit is refused under `Locked`. Both engines refuse the write
    // (`elements/control/swt_control/accessors.rs`), verified live on the r4133
    // DLL: `action=open` under `lock=yes` moves neither `state` nor `normal`.
    // The twin `swtcontrol.state` is a weaker witness than it reads (r4133
    // re-reads the controlled element, this engine returns its tracked field).
    // The probe plus the green corpus gate on the switch decks settle it.
    (
        "swtcontrol.action",
        Owner::Rp23,
        "SwtControl.pas:573-620 (no arm 3) / :192-193 / :652 / :417",
    ),
    // `'17'` vs `'1 16 +'`: the deck's own RPN SOURCE TEXT, `mode=(1 16 +)`,
    // echoed back with the parser's parens stripped. r4133's Monitor has no
    // getter override and stores the raw parameter. Decks:
    // `Test/Dynamic_Kundur.dss`,
    // `Version8/Distrib/Examples/Dynamic_Expressions/Dynamic_KundurDynExp.dss`.
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
    // **`line.units`, outcome FIX (both lanes).** r4133 renders the live
    // `LengthUnits`, so `'none'` vs `'kft'` meant the two engines held
    // different units. r4133 re-applies the saved units AFTER the impedance edit
    // of a merge, while dss_capi 0.14.5 wipes them with the matrix side effects.
    // `exec/reduce.rs`'s matrix-series branch calls `red_set_units` after those
    // side effects, and `red_merge`'s sym branch re-applies `Length=`/`Units=`
    // unconditionally. `reset_length_units` keeps `user_length_units`
    // (`elements/pd/line/code.rs`), as both OpenDSS engines do. The frozen
    // `rust='none'` column of this row predates the fix (the extracts are a data
    // lock, never edited). The cells are on `modes:reduce/midi_reduce.dss`, a
    // capi-only case: `tests/corpus/ledger.json`
    // `reduce-merge-units-restored-midi-capi-props` is the exclusion and
    // `exec::tests::reduce::merged_matrix_line_keeps_the_surviving_lines_length_units`
    // plus `the_corpus_reduce_decks_merged_lines_render_kft` are the pins. The
    // census decomposition is derived by
    // `the_line_units_census_decomposition_is_read_off_the_corpus`.
    (
        "line.units",
        Owner::Rp35,
        "FIX — Line.pas:1404 / :1627 / :1721-1726 / :1791-1796 / :2326-2331",
    ),
    // --- the S6 singletons ---------------------------------------------------
    // **`line.linecode`, outcome FIX (both lanes).** r4133 renders the code name
    // only while a line-code flag is set. The impedance side effects clear it,
    // the `switch=` arm does not. dss_capi 0.14.5 clears it there too. The flag
    // also picks the unit-conversion formula on a later `units=`, and these decks
    // type `units=m` AFTER `Switch=True`. The `SWITCH` arm of
    // `elements/pd/line/accessors.rs` keeps the flag, and every other site where
    // r4133 clears it has its counterpart here. The frozen `rust=''` column of
    // this row predates the fix. All cells are in scope, on two `engines: both`
    // cases whose capi property compare is live: `tests/corpus/ledger.json`'s
    // `line-switch-keeps-linecode-zone2-capi-props` and
    // `line-switch-keeps-linecode-zone3-capi-props` are the exclusions and
    // `exec::tests::line_fetch::
    // switch_yes_keeps_the_linecode_and_its_units_conversion` is the pin. The
    // r4133 channel needs no entry: these cells compare and match. The
    // decomposition is derived by
    // [`the_line_linecode_census_decomposition_is_read_off_the_corpus`].
    //
    // The flag is `line_code_specified` and the code name `line_code_name`,
    // which survives every kill. `Dump` prints `~ LineCode=<code>` past a kill
    // and the CIM units back-fill matches the code name. Neither surface is
    // compared by any channel.
    (
        "line.linecode",
        Owner::Rp35,
        "FIX — Line.pas:1357 / :413 / :685 / :691 / :694-700 / :626-627 / :387 / :825 / :1273 \
         / :1853-1854 / :663 / :696 / :704-713 / :2268 + ExportCIMXML.pas:3876 — \
         elements/pd/line/{mod,code,accessors,dump}.rs and cim/export.rs, both lanes; pins \
         exec::tests::line_fetch::switch_yes_keeps_the_linecode_and_its_units_conversion, \
         exec::tests::line_fetch::linecode_name_survives_the_flag_that_gates_its_render, \
         golden_cim::cim_linecode_units_backfill_matches_the_condcode_string and \
         exec::tests::line_fetch::conductors_clears_the_linecode_flag_and_the_switch_arm_spares_the_spacing",
    ),
    // Index 21 has no getter arm, so r4133 echoes the stored property text,
    // default `''`, overwritten with the deck's `'sp'`. This engine models
    // `SpacingSpecified` as a Boolean field, so a `linecode=`/`switch=` that only
    // drops the flag leaves the object and the live render standing. The one
    // census cell is `MakePosSequence`, which kills the object on both engines.
    (
        "line.spacing",
        Owner::Rp23,
        "Line.pas:1347-1429 (no arm 21) / :1511 / :2266-2276",
    ),
    // r4133's Isource has no getter override and stores `''` for `bus2`, which
    // only an explicit `bus2=` would overwrite. None of these decks sets one, so
    // r4133 echoes `''` while this engine renders the derived grounded-wye
    // terminal 2. An OpenDSS report is filed.
    ("isource.bus2", Owner::Rp23, "Isource.pas:631 (+ report 14)"),
    // Same echo (`PropertyValue[8] := ''`, `Isource.pas:628`), and the port's
    // live value IS r4133's live value: the `daily=` arm aliases the OBJECT
    // without touching the string — `IF YearlyShapeObj=Nil THEN YearlyShapeObj
    // := DailyShapeObj` (`:286`) — and `CalcYearlyMult` (`:672-681`) consumes
    // the object. The port reports the aliased object's name.
    ("isource.yearly", Owner::Rp23, "Isource.pas:628 / :286"),
    // NOT an echo: r4133's Load getter answers the live string field, the RAW
    // user-typed name (case preserved, `''` when never typed). This engine
    // prints the resolved shape object's lowercased name, and the `''` half is
    // the same daily-to-yearly object aliasing as `isource.yearly`. A `CaseFold`
    // row claims the case-only half, and a `LiveSemanticsDiffer` echo row (plus
    // its pin) covers the empty-vs-value cells. The chain order keeps them
    // apart, cell by cell.
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
    // Both sides render live and r4133 renders the WRONG list: its ExpControl
    // getter answers the PVSystem name list for both `PVSystemList` and
    // `DERList`, while its Edit arms keep the two lists distinct.
    // `expcontrol.pvsystemlist` is NOT a divergent pair, which confirms the two
    // engines agree wherever the bare list is the right answer. This engine is
    // right: `LiveSemanticsDiffer` + pin.
    (
        "expcontrol.derlist",
        Owner::Rp23,
        "ExpControl.pas:696 + :702-715 vs :227-234 / :247-252",
    ),
    // **`swtcontrol.normal`/`state`, outcome FIX (both lanes).** r4133 keeps
    // per-phase switch state, settable phase by phase from a quoted list or
    // ganged from a bare token, each phase driving its own conductor, and its
    // getter renders one token per CONTROLLED-ELEMENT phase. This engine carries
    // the same model: `[ControlAction; 6]` arrays plus the `NormalStateSet`
    // latch, a name-keyed lock guard, ganged-when-bare / per-phase-when-quoted
    // writes with the five-token cap, per-conductor drive at
    // `RecalcElementData`, and getters that render one token per controlled
    // phase (`[]` for a nil element). A locked `normal=` APPLIES while a locked
    // `state=`/`action=` does not, and `NormalStateSet` latches even on a
    // refused write.
    //
    // The frozen `rust='closed'` column of these rows predates the fix (the
    // extracts are a data lock, never edited), so all five are counted as
    // **superseded** ([`RP37_SUPERSEDED`], [`SUPERSEDED_RP37`]). The two
    // channels:
    //
    // * **r4133: nothing to exclude.** The in-scope cells render
    //   byte-identically to r4133, held by `exec::tests::controls::
    //   swtcontrol_state_renders_per_phase_on_the_r4133_only_decks`. Those decks
    //   are `engines: r4133`, and the 0.14.5 engine cannot render the array at
    //   all.
    // * **capi_v0145: five entries.** The out-of-scope cells sit on five
    //   capi-gated cases whose property compare runs: `swtcontrol_lock`,
    //   `makeposseq_ctrl` and three `IEEE_519` copies.
    //   `tests/corpus/ledger.json`'s
    //   `swtcontrol-per-phase-state-{lock,makeposseq,ieee519-tmode,
    //   ieee519-varload,ieee519-matlab}-capi-props` (cause
    //   `swtcontrol-per-phase-state-render`) are the exclusions and
    //   `exec::tests::controls::
    //   swtcontrol_state_renders_one_token_per_controlled_phase` is their
    //   witness ([`LANDED_PROPERTY_ENTRY_PINS`]).
    //
    // The decomposition behind the cell counts and the five entries is derived
    // from the corpus by [`the_swtcontrol_state_census_decomposition_is_read_off_the_corpus`].
    (
        "swtcontrol.normal",
        Owner::Rp35,
        "FIX — SwtControl.pas:589-599 / :37-38 / :299-307 / :416-417 / :433-480 / :532-549 / \
         :220-228 — elements/control/swt_control/{mod,accessors}.rs, both lanes; pins \
         exec::tests::controls::swtcontrol_state_renders_one_token_per_controlled_phase, \
         exec::tests::controls::swtcontrol_state_renders_per_phase_on_the_r4133_only_decks \
         and swt_control::tests::locked_normal_applies_locked_state_and_action_do_not",
    ),
    (
        "swtcontrol.state",
        Owner::Rp35,
        "FIX — SwtControl.pas:600-610 (same set)",
    ),
    // **`relay.normal`/`state`, outcome FIX (both lanes)**, the mirror image:
    // this engine rendered three tokens and r4133 one, because r4133's Relay
    // getter loops the LIVE controlled element's phases. `make_pos_sequence`
    // refreshes `ctrl_snap.{nphases, nterms, buses}` from the live controlled
    // element, so the render prints one token AND `Sample` does not resync
    // conductors off the end of a 1-conductor element. Both cells render r4133's
    // `[closed, ]` byte for byte, so nothing is excluded on either channel: the
    // frozen `rust` columns predate the fix and the rows stay declared for that
    // reason alone. The single cell is on
    // `modes/makeposseq/makeposseq_ctrl.dss`, `engines: "capi_v0145"`, where
    // `Relay` and `Recloser` are whole-element-skipped
    // (`crates/dss-test-harness/src/harness/mod.rs::skip_whole_element`).
    (
        "relay.normal",
        Owner::Rp35,
        "FIX — Relay.pas:1407-1417 (loops ControlledElement.NPhases) / :1237-1308 / :616-619 \
         — elements/control/relay/{mod,accessors}.rs, both lanes; pins \
         relay::tests::the_render_bound_follows_makeposseq and \
         relay::tests::a_quoted_single_token_is_per_phase_a_bare_one_is_ganged",
    ),
    (
        "relay.state",
        Owner::Rp35,
        "FIX — Relay.pas:1418-1428 (same set)",
    ),
    // --- the three pairs beyond the closed list ------------------------------
    // Closing bin 3 means closing its CELLS, not only its eight pairs: three
    // bin-2-labelled pairs carry enum-spelling cells, and the accounting routes
    // those cells here. `invcontrol.voltage_curvex_ref` is a LIVE enum rendering
    // and is therefore CLAIMED by `PROPS_NORM_R4133`'s fifth `EnumSynonym` row,
    // not routed. See [`RP22_BEYOND_THE_CLOSED_LIST`].
    //
    // r4133's CapControl has no getter override, so `type` echoes the deck's raw
    // token: `'pf'` and `'volt'` against our registry name
    // `'PowerFactor'`/`'Voltage'` (`obj/dss_enum/registry/control.rs`). The
    // pair's case-only spellings stay claimed by its `CaseFold` row:
    // normalization precedes the echo table, so only the remaining cells are
    // credited to the echo row (the shipped exclusion is pair-scoped).
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

/// The pairs to route that the S6 list does not name: a **bin-2 label** with
/// genuine bin-3 cells inside it (`capcontrol.type`,
/// `invcontrol.voltage_curvex_ref` and `fault.bus2`; `reactor.bus2` is on the S6
/// list). Named here rather than folded into [`RP22_ROUTING`] because the
/// accounting puts example rows on them that the S6 list does not predict.
const RP22_BEYOND_THE_CLOSED_LIST: &[&str] = &[
    "capcontrol.type",
    "fault.bus2",
    "invcontrol.voltage_curvex_ref",
];

/// The residue the bin map cannot decide: example rows whose **own** cell
/// classification (bin 2 or 4) differs from the mechanism that claims them, on a
/// pair the enumerated lists do not already own. One entry per pair, each with
/// the evidence that decides it.
///
/// `energymeter.peakcurrent`, `generator.dynout` and `generator.userdata` are
/// claimed by the chain (an echo row, and for the last one a `PROPS_NORM_R4133`
/// `ArrayForm` row beside it), so [`declare`] never reaches their entries. They
/// stay as the record of their disposition. The four `sensor.*` entries are
/// live. Every entry's pair is checked against the evidence by
/// [`the_enumerated_pair_lists_still_describe_the_vendored_evidence`].
const CELL_DISPOSITION: &[(&str, Owner, &str)] = &[
    // `'[ 400]'` vs `'((400, 400, 400))'`: a one-element array against r4133's
    // frozen three-element default, which r4133 never refreshes, so it is an
    // `EchoDefault`, while the pair's OTHER spellings are honest arrays that
    // `ArrayForm` claims. The pair needs BOTH rows.
    (
        "energymeter.peakcurrent",
        Owner::Rp23,
        "EnergyMeter.pas:2637-2664 / :2209",
    ),
    // `'[speed, theta]'` vs `'[speed,dpshaft,]'`, on the `Dynamic_KundurDynExp`
    // decks (plus the `''`-vs-`'[]'` unset-array spelling of the same pair).
    // **Not an echo and not a spelling**: r4133 stores the *variable* index of a
    // dynamic output but renders it through a decoder that reads a flat
    // *(variable, derivative slot)* index. For this deck's
    // `varnames=[Speed Mass PShaft Pterm Damp theta]`, `theta` (index 5)
    // therefore prints as `d` + the third name = `dpshaft`. The dynamics are
    // unaffected, so this is an r4133 **rendering** bug and this engine's answer
    // is correct: a tagged `LiveSemanticsDiffer` echo row with this citation plus
    // an expected-value pin, never a silent `EchoDefault`. An OpenDSS report is
    // filed.
    (
        "generator.dynout",
        Owner::Rp23,
        "PCElement.pas:197-243 / DynamicExp.pas:411-465 (LiveSemanticsDiffer + pin)",
    ),
    // `'rs=… option=fixed'` vs `'(rs=… option=fixed)'`: r4133 wraps the stored
    // user-model data string in parens. Same string, same `PropertyValue[]`
    // store as the pair's `''`-vs-`'()'` rows — one echo row covers the pair.
    (
        "generator.userdata",
        Owner::Rp23,
        "the generator/sensor shape-closure record, bin 5",
    ),
    // The Sensor arrays: `'[ 0]'` (one element) against r4133's three. A real
    // value delta, **out of scope**: every Sensor cell sits on
    // `engines: "capi_v0145"` cases, so the r4133 compare never reaches them.
    (
        "sensor.currents",
        Owner::OutOfScope,
        "every cell on an engines: capi_v0145 case",
    ),
    (
        "sensor.kvars",
        Owner::OutOfScope,
        "every cell on an engines: capi_v0145 case",
    ),
    (
        "sensor.kvs",
        Owner::OutOfScope,
        "every cell on an engines: capi_v0145 case",
    ),
    (
        "sensor.kws",
        Owner::OutOfScope,
        "every cell on an engines: capi_v0145 case",
    ),
];

/// **The five `SilentReadOnly` pairs that get NO echo row.**
///
/// They are not echoes and not a spelling difference: r4133's getter arm is LIVE
/// and prints a computed read-only quantity, while dss_capi 0.14.5 flags the
/// property `[SilentReadOnly, ReadByFunction]` and renders `''`. This engine
/// renders the live value, and the resulting **capi-side** divergence is
/// excluded field by field and pinned.
///
/// # Why they are SUPERSEDED and not claimed
///
/// `PropFlags::RENDERS_LIVE_RESULT` rides alongside `SILENT_READ_ONLY` on exactly
/// these five `PropDef`s, and the render gate (`obj/props/class_props/value.rs`)
/// does not suppress them, in both lanes. So the `rust` column of the frozen rows
/// (`''`, by capture) predates the change, and the extracts are a data lock that
/// is never re-frozen. Replaying a counterfactual spelling through the chain
/// would prove nothing, and writing the new spelling into the `rust` column
/// would be a fabricated measurement. The rows are therefore counted as
/// **superseded** ([`SUPERSEDED_RP38`]), and the proof is live evidence, per
/// pair, below.
///
/// # What the live evidence says
///
/// Measured with `DSS_PROPS_CENSUS=claims` over the affected families, default
/// lane, both channels:
///
/// * on **capi_v0145** the five pairs are excluded outright by a `SKIP_PROPS` +
///   `SKIP_PROPS_CAPI_ONLY` row pair, because 0.14.5 renders `''` for a
///   mechanical reason and no value compare can bridge `number` vs `''`;
/// * on **r4133**, the channel this accounting is about, they COMPARE. The
///   divergent cells are [`Link::DisplayFloor`]'s own class (this engine prints
///   `float_to_str_ex`, r4133 its `%.6g`/`%-.8g` of the same double), except the
///   `modes:makeposseq/makeposseq_ctrl.dss` cells of an r4133 `MakePosSequence`
///   bug, on a `capi_v0145`-only case the r4133 channel never gates. Every other
///   cell compares EQUAL at the case's own tier floor.
///
/// Each row carries four columns: the pair, the r4133 live getter arm it renders
/// from, the measured live disposition on the r4133 channel, and the
/// expected-value pin that holds this engine's value (`props_r4133_pins.rs`,
/// read back by the same both-ways guard
/// [`every_echo_row_pin_is_a_test_that_exists`]).
const RP38_SUPERSEDED: &[(&str, &str, &str, &str)] = &[
    (
        "indmach012.pf",
        "IndMach012.pas:1790 (arm 5: Format('%.6g',[PowerFactor(Power[1,ActiveActor])]))",
        "no divergent cell at all, measured over all 6 cases holding the class: a power \
         factor is bounded by 1, so r4133's %.6g render is at most 5e-07 ABSOLUTE from \
         ours — inside the i_abs = 1e-6 the property compare uses at every tier, before \
         the display floor is ever consulted. Ours 0.909167177168387 vs r4133 \
         '0.909167' after compile on asymmetric:indmach/indmach_asym.dss",
        "indmach012_pf_renders_the_live_power_factor",
    ),
    (
        "storagecontroller.kwhtotal",
        "StorageController.pas:991 (GetkWhTotal, body :1172-1184, Format('%-.8g'))",
        "no divergent cell at all: every fleet kWh nameplate in the population is \
         integer-valued, so the two renders are byte-identical",
        "storagecontroller_fleet_aggregates_render_the_live_fleet",
    ),
    (
        "storagecontroller.kwtotal",
        "StorageController.pas:992 (GetkWTotal, body :1186-1198, Format('%-.8g'))",
        "1 divergent cell, out of scope: byte-identical everywhere except \
         modes:makeposseq/makeposseq_ctrl.dss (ours '33.3333333333333' vs r4133 \
         '100'), where r4133's own MakePosSequence writes `kWrating=` for the property \
         `kWrated` and never scales the rating — an \
         OpenDSS bug this engine does not reproduce, on a capi_v0145-only case the r4133 \
         channel never gates",
        "storagecontroller_fleet_aggregates_render_the_live_fleet",
    ),
    (
        "storagecontroller.kwhactual",
        "StorageController.pas:993 (GetkWhActual -> FleetkWh, :1032-1042)",
        "79 divergent cells (67 in scope), every one claimed by the display floor: ours \
         2627.39290900018 vs r4133 '2627.3929' on the StorageControllerTechNote \
         PeakShave deck; worst cell of the pair 3.88e-08 rel",
        "storagecontroller_fleet_aggregates_render_the_live_fleet",
    ),
    (
        "storagecontroller.kwactual",
        "StorageController.pas:994 (GetkWActual -> FleetkW, :1019-1029)",
        "25 divergent cells (22 in scope): 24 claimed by the display floor — ours \
         -18.8106796116505 vs r4133 '-18.81068', worst cell of the pair 4.03e-08 rel — \
         plus the one makeposseq_ctrl cell of the kWTotal bug above (ours \
         '-0.333333333333333' vs r4133 '-1'), out of scope",
        "storagecontroller_fleet_aggregates_render_the_live_fleet",
    ),
];

/// **The frozen rows the per-phase SwtControl render superseded**: the
/// [`RP38_SUPERSEDED`]-shaped accounting for `swtcontrol.normal` and
/// `swtcontrol.state`.
///
/// # Why they are SUPERSEDED and not claimed or declared
///
/// This engine carries r4133's per-phase model and its getters render one token
/// per controlled-element phase, so the `rust` column of the frozen rows
/// (`'closed'`, `'open'`) predates the change, and the extracts are a data lock
/// that is never re-frozen. [`SUPERSEDED_RP37`] states the arithmetic.
///
/// # What the live evidence says
///
/// The pairs are under the r4133 property compare, and three measurements
/// agree: no `ArrayForm` row on them folds anything
/// (`props_norm::check_rows_are_live`), the claims census
/// (`DSS_PROPS_CENSUS=claims`) lists no divergent cell for either pair, and the
/// corpus gate is green on the three r4133-gating decks that hold the class,
/// whose cells [`the_swtcontrol_state_census_decomposition_is_read_off_the_corpus`] derives
/// from the decks.
///
/// Columns: the pair, the r4133 getter arm it renders from, the measured live
/// disposition on the r4133 channel, and the expected-value pin that holds this
/// engine's render, with the file that defines it, because these pins live
/// beside the behaviour (`exec::tests::controls`) rather than in [`PINS`]
/// ([`LANDED_PROPERTY_ENTRY_PINS`]'s shape). Both halves are read back by
/// [`the_swtcontrol_pairs_are_superseded_by_the_per_phase_render`].
const RP37_SUPERSEDED: &[(&str, &str, &str, &str, &str)] = &[
    (
        "swtcontrol.normal",
        "SwtControl.pas:589-599 (arm 6: one 'open'/'closed' token per \
         ControlledElement.NPhases off FNormalState^, the pStateArray declared \
         :37-38 and allocated :299-307)",
        "no divergent cell at all, measured over the whole population: this engine \
         renders the same per-phase token list, so all 59 cells — \
         40 of them in scope, on midi_swtcontrol 12 + swtcontrol_time 12 + \
         civanlar 16 — compare EQUAL on the r4133 channel. The frozen row \
         'closed' vs '[closed, closed, closed, ]' is what this engine printed when the \
         census was frozen and cannot occur now.",
        "swtcontrol_state_renders_per_phase_on_the_r4133_only_decks",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol.state",
        "SwtControl.pas:600-610 (arm 7, the same loop off FPresentState^)",
        "no divergent cell at all, same population and the same three decks. The \
         pair's out-of-scope half is the five landed capi_v0145 entries \
         swtcontrol-per-phase-state-{lock,makeposseq,ieee519-tmode,\
         ieee519-varload,ieee519-matlab}-capi-props (cause \
         swtcontrol-per-phase-state-render), on cases the r4133 channel never \
         gates.",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
];

/// Which superseded table, if any, owns a pair: the closed set [`account`]
/// intercepts on, and the key [`Ledger::superseded`] reports under. A pair may
/// name at most one table, which [`the_two_superseded_tables_are_disjoint`]
/// proves.
fn superseded_tag(pair: &str) -> Option<&'static str> {
    if RP37_SUPERSEDED.iter().any(|(p, ..)| *p == pair) {
        Some("swtcontrol-per-phase")
    } else if RP38_SUPERSEDED.iter().any(|(p, ..)| *p == pair) {
        Some("silent-read-only")
    } else {
        None
    }
}

/// The pin that holds the **capi** half of the `SilentReadOnly` exclusion: the
/// committed 0.14.5 capture's own `''` for the `props` golden cells of the five
/// pairs.
///
/// It speaks for all five at once, so it rides beside [`RP38_SUPERSEDED`]
/// rather than inside it, and it is cited here because
/// [`every_echo_row_pin_is_a_test_that_exists`] must know every `#[test]` the pin
/// file defines.
const RP38_CAPTURE_PIN: &str = "the_silent_readonly_capture_cells_are_empty";

/// **Who owns the cells an echo row carves out** (`props_norm::
/// ECHO_CARVE_OUTS`).
///
/// A carve-out means "this cell of a cited pair is NOT that row's echo", so the
/// example row it matches leaves the echo link and needs an owner like any other
/// unclaimed row. It cannot get one from the pair's own evidence: `declare`
/// reads `bins.tsv`'s PAIR bin, and `reactor.kvar`'s bin-7 label comes from its
/// echo cells, which would route the carved cell straight back to the echo
/// bucket. So the routing is explicit, cited, and matched against the shipped
/// carve-outs both ways ([`the_carve_outs_are_routed_and_only_they_are`]).
///
/// `reactor.kvar` `'66.6666666666667'` vs `'66.667'`: r4133's own
/// `MakePosSequence` round-trips the value through `%-.5g` text, so both sides
/// are LIVE and differ by the display class the floor covers, exactly where the
/// same round trip's `reactor.kv` sits (`bins.tsv`: numeric bin 6).
const ECHO_CARVE_OUT_ROUTING: &[(&str, &str, &str, Owner, &str)] = &[(
    "reactor.kvar",
    "66.6666666666667",
    "66.667",
    Owner::Rp24,
    "Reactor.pas:1145-1201 (MakePosSequence -> Format(' kvar=%-.5g') -> Parser/Edit): both sides \
     live, 5.0e-06 apart — the display class, like reactor.kv (bins.tsv bin 6, 5.85e-06)",
)];

/// The `shape.txt` gap names closed by porting the property rather than by a
/// `PROPS_015X` row: the WindGen `UserModel`/`UserData` surface. Its live proof
/// is the WindGen unit pins plus the r4133 channel. This file only pins that no
/// allowlist row is doing that job.
const SHAPE_PORTED: &[(&str, &str)] = &[("windgen", "usermodel"), ("windgen", "userdata")];

/// The eleven boolean spellings r4133's own getters print. Anything else on a
/// bin-1 pair is an echo of the stored property text, not a boolean.
const BOOL_SPELLINGS: &[&str] = &[
    "true", "True", "false", "False", "YES", "yes", "no", "NO", "n", "y", "Y",
];

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// Which file an example row came from. Supplement rows count exactly like
/// frozen ones in every assertion here.
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
    /// **The shape allowlist.** A property the OpenDSS engine's own name list
    /// cannot carry is dropped before any value is compared (`prop_015x`,
    /// `harness/mod.rs`). On the r4133 channel exactly one such row exists,
    /// `GenDispatcher.weights`, and the census recorded that gap as shape rows,
    /// never as value cells, which is why this link claims **zero** example rows
    /// while still being exercised against `shape.txt`.
    ShapeAllowlist,
    /// **The normalization** (`PROPS_NORM_R4133`): the value-preserving
    /// re-spelling.
    Normalization,
    /// **The echo table** (`PROPS_ECHO_R4133`): the exclusion. Its cited rows
    /// are consulted only after the normalization link has had its chance, so on
    /// a mixed pair a typed rule *claims* the foldable spellings first and this
    /// link is credited with what is left ([`MIXED_PAIR_NORM_ROWS`] is the other
    /// half, disjoint by construction rather than by chain order).
    ///
    /// **This accounting is per spelling, the shipped exclusion is per pair.**
    /// The two are the same statement only for the cells the census recorded: at
    /// the live seam a cell of a cited pair that no rule folds is masked all the
    /// same (`harness::props_norm::PROPS_ECHO_R4133`'s doc). One measured cell is
    /// out of its row, [`ECHO_CARVE_OUT_ROUTING`], and this link answers `false`
    /// for it.
    Echo,
    /// **The display floor** (`props_norm::R4133_DISPLAY_FLOOR`, `2e-4`
    /// relative): the last link, consulted only after the three above have
    /// declined. It claims a cell whose two sides are one number printed to
    /// different precision by a `%[-].Ng` getter.
    ///
    /// Unlike the two tables it is a **per-cell predicate with no rows**: it
    /// reads the two values and nothing else, so there is nothing here to go
    /// stale and nothing that could mask a neighbouring cell of the same pair.
    /// The derivation lives on the constant. This link's own population is
    /// [`CLAIMED_DISPLAY_FLOOR`].
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

/// The mechanism that owns a row no link of the chain claims, or the reason
/// none will.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Owner {
    /// Enum synonyms and the S6 singletons: bin 3, the enumerated singletons and
    /// any cell that needs a per-pair source reading.
    Rp22,
    /// The echo-exclusion table: bin 5, the bin-1 echo halves, the echo-rooted
    /// bin-7 pairs. **Empty** ([`DECLARED_RP23`]).
    Rp23,
    /// The r4133 props display floor (bin 6). **Empty** ([`DECLARED_RP24`]): its
    /// population is claimed by [`Link::DisplayFloor`] or declared out of scope
    /// by [`RP24_OUT_OF_SCOPE`].
    Rp24,
    /// The four bin-7 value jumps that are not echo. **Empty**
    /// ([`DECLARED_RP3`]): `generator.model`'s row is claimed by [`Link::Echo`]
    /// and the other three pairs' rows are excluded by `ledger.json` `property`
    /// entries, counted as ledgered ([`RP3_LEDGERED`], [`LEDGERED_RP3`]). The
    /// variant stays so a regression that re-creates the bucket fails here by
    /// name.
    Rp3,
    /// **The live-state divergences**: live-state or modelling divergences
    /// rather than spellings, `line.units`, `line.linecode` and the per-phase
    /// switch/relay state. Which pair is which is recorded in [`RP22_ROUTING`]'s
    /// citation column. This owner is the accounting bucket they share.
    ///
    /// **A settled pair does not leave the bucket**, the same rule
    /// [`RP3_ROUTING`] states. `line.units` is fixed in both lanes, yet it still
    /// declares here: its exclusion is a `capi_v0145` ledger entry and no
    /// [`Link`] of the chain reads the ledger, so the row is retired by hand,
    /// never on its own.
    Rp35,
    /// **The five `SilentReadOnly` pairs**: r4133 renders a live computed
    /// read-only quantity and dss_capi 0.14.5 renders `''` because it flags the
    /// property `[SilentReadOnly, ReadByFunction]`. This engine renders the live
    /// value, and the capi-side divergence is excluded and pinned there.
    ///
    /// **Empty** ([`DECLARED_RP38`]): the frozen rows' `rust = ''` column no
    /// longer describes this engine, so they are counted as superseded
    /// ([`RP38_SUPERSEDED`], [`SUPERSEDED_RP38`]) instead of declared. The variant
    /// stays so a regression that re-creates the bucket fails by name.
    Rp38,
    /// **The round-trip residue**: spellings that sit inside the display floor
    /// and are no `%.Ng` render of our value, so the two engines hold different
    /// doubles (an OpenDSS command-string round trip amplified through a derived
    /// quantity, or a plain state difference). See [`RP39_ROUTING`].
    ///
    /// Every pair is settled ([`OPEN_RP39`]): an OpenDSS round trip reproduced
    /// with this engine exact, pinned by [`RP39_PINS`]. The bucket does not move
    /// for that: nothing claims the rows, and a pin is not a ledger entry. They
    /// stay declared.
    Rp39,
    /// **The AutoTrans typecast bug**: the `autotrans.wdgcurrents` spellings of
    /// the four `controls:autotrans/*` decks (none in scope), where r4133's
    /// `RegControl` reads an AutoTrans as a two-winding transformer and therefore
    /// never taps it. Its cells are the unregulated circuit's, ours the regulated
    /// one's. See [`RP312_UPSTREAM_BUG`] and [`DECLARED_RP312`].
    ///
    /// All its cells sit on `engines: "capi_v0145"` cases, so the r4133 compare
    /// never reaches them: the owner records a measured cause and a witness
    /// rather than a scope excuse.
    Rp312,
    /// Nothing will claim it: every cell of the pair sits on an
    /// `engines: "capi_v0145"` case, which is never compared on r4133.
    OutOfScope,
}

impl Owner {
    fn tag(self) -> &'static str {
        match self {
            Owner::Rp22 => "S6 / enum synonyms",
            Owner::Rp23 => "echo table",
            Owner::Rp24 => "display floor",
            Owner::Rp3 => "bin-7 root cause",
            Owner::Rp35 => "live-state divergence",
            Owner::Rp38 => "SilentReadOnly live render",
            Owner::Rp39 => "round-trip residue",
            Owner::Rp312 => "AutoTrans typecast bug",
            Owner::OutOfScope => "out of scope",
        }
    }
}

/// What the vendored evidence says about the pair an example row belongs to.
#[derive(Debug, Clone)]
struct PairEvidence {
    /// `structural` or `numeric` — the census's own kind column.
    numeric: bool,
    /// The census bin (1-7).
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
    /// A pair with at least one cell the r4133 compare reaches. `None` (no
    /// recorded split) means "not measured separately" and is read as in scope,
    /// the conservative direction: the row then needs an owner.
    fn in_scope(&self) -> bool {
        self.cells_in_scope.is_none_or(|n| n > 0)
    }

    /// The bin the r4133 compare faces for a numeric pair: the in-scope
    /// re-derivation (`max_rel_in_scope >= 1e-4`), which moves four full-census
    /// bin-7 pairs into the display class. Structural pairs answer their own bin.
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
    /// Rows whose pair has at least one cell the r4133 compare reaches.
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
    /// Rows whose frozen `(rust, r4133)` spelling an engine change made
    /// counterfactual, so no link of the chain can be asked about them and no
    /// owner holds them either, keyed by [`superseded_tag`]: the per-phase
    /// SwtControl pairs ([`SUPERSEDED_RP37`]) and the `SilentReadOnly` pairs
    /// ([`SUPERSEDED_RP38`]). The third state a row can end in, and a narrow one:
    /// a row only reaches it by naming its pair in a cited table, and each table
    /// is locked separately so one can never absorb the other's rows.
    superseded: BTreeMap<&'static str, Bucket>,
    /// Rows a landed `tests/corpus/ledger.json` `property` entry excludes, so
    /// no link of the chain claims them and no owner declares them either:
    /// exactly [`RP3_LEDGERED`]'s pairs ([`LEDGERED_RP3`]). The fourth state a
    /// row can end in, and as narrow as the third: a row only reaches it by naming
    /// its pair in a cited table whose entry ids are read back out of the ledger.
    ledgered: Bucket,
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

    /// `(rows, pairs, rows on in-scope pairs)` for ONE superseded table — the
    /// same triple [`Ledger::owner`] reports, so [`SUPERSEDED_RP37`] and
    /// [`SUPERSEDED_RP38`] can each be read against it like any `DECLARED_*`
    /// lock.
    fn superseded(&self, tag: &str) -> (usize, usize, usize) {
        self.superseded
            .get(tag)
            .map(|b| (b.rows, b.pairs.len(), b.in_scope_rows))
            .unwrap_or((0, 0, 0))
    }

    /// Rows superseded by any table — the term the totality assert needs.
    fn superseded_rows(&self) -> usize {
        self.superseded.values().map(|b| b.rows).sum()
    }

    /// `(rows, pairs, rows on in-scope pairs)` for the ledgered bucket — the
    /// same triple [`Ledger::owner`] reports, so [`LEDGERED_RP3`] can be read
    /// against it like any `DECLARED_*` lock.
    fn ledgered(&self) -> (usize, usize, usize) {
        (
            self.ledgered.rows,
            self.ledgered.pairs.len(),
            self.ledgered.in_scope_rows,
        )
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

/// `haystack` names `token` as a whole identifier.
///
/// `str::contains` would not: [`LEDGER_ENTRY_PINS`] holds
/// `swtcontrol_delay_wires_the_property` and
/// `swtcontrol_delay_wires_the_property_on_the_midi_tie`, and the first is a
/// strict **prefix** of the second, so a verdict naming only the longer pin
/// would satisfy a substring test for both.
fn names_identifier(haystack: &str, token: &str) -> bool {
    fn ident(c: char) -> bool {
        c.is_ascii_alphanumeric() || c == '_'
    }
    haystack.match_indices(token).any(|(at, _)| {
        !haystack[..at].chars().next_back().is_some_and(ident)
            && !haystack[at + token.len()..]
                .chars()
                .next()
                .is_some_and(ident)
    })
}

/// The gated corpus root and its frozen population fingerprint,
/// repo-root-relative — read (never written) by
/// [`the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus`].
const CORPUS: &str = "tests/corpus";
/// The anti-shrink lock, whose per-case rigor strings carry `steps=`/`engines=`.
const POPULATION_LOCK: &str = "tests/corpus/manifests/population.lock.json";
/// The gating divergence ledger, read by
/// [`the_r4133_property_entries_are_the_root_cause_eight`], by
/// [`the_ledgered_rows_are_excluded_by_entries_that_are_really_in_the_ledger`],
/// by [`the_autotrans_typecast_cases_pair_their_r4133_channel_with_a_skip_entry`]
/// and by [`r4133_skipped_cases`]. **No `Link` of the declaration chain reads
/// it** (see [`DECLARED_RP3`]): those readers are assertions *about* the file,
/// not sources of a declaration, which is why [`RP3_LEDGERED`] moves the rows
/// the entries exclude.
const LEDGER: &str = "tests/corpus/ledger.json";

/// The deck path of a corpus case id, relative to [`CORPUS`]: `family:rel` for
/// the three synthetic families, `solvable_now:rel` for the vendored decks
/// (which live under `electricdss-tst/`).
fn case_deck(case: &str) -> String {
    let (family, rel) = case
        .split_once(':')
        .unwrap_or_else(|| panic!("{case}: not a `family:path` case id"));
    match family {
        "solvable_now" => format!("electricdss-tst/{rel}"),
        _ => format!("{family}/{rel}"),
    }
}

/// One case's rigor string out of `population.lock.json` — the frozen record of
/// what the gate runs for it (`kind=… steps=… engines=…`).
fn case_rigor(lock: &serde_json::Value, case: &str) -> String {
    let (family, rel) = case.split_once(':').expect("a `family:path` case id");
    let section = match family {
        "solvable_now" => &lock["solvable_now"],
        _ => &lock["family_rigor"][family],
    };
    section[rel]
        .as_str()
        .unwrap_or_else(|| panic!("{case}: no rigor row in {POPULATION_LOCK}"))
        .to_string()
}

/// One `key=value` of a rigor string.
fn rigor_field<'a>(rigor: &'a str, key: &str) -> &'a str {
    let want = format!("{key}=");
    rigor
        .split_whitespace()
        .find_map(|f| f.strip_prefix(want.as_str()))
        .unwrap_or_else(|| panic!("no {key}= in rigor {rigor:?}"))
}

/// The cases whose **`r4133` channel is never dispatched** because a ledger
/// `skip` entry drops it: the second half of "does r4133 gate this case?", read
/// off [`LEDGER`] with `corpus_gate/ledger.rs::channel_is_skipped`'s own
/// condition (`case` matches, `channel == "r4133"`, `kind == "skip"`).
///
/// **`engines: "both"` alone does not mean r4133 gates a case.**
/// `asymmetric:line/line_spacing_asym.dss` is `engines: "both"` and yet no r4133
/// comparison runs on it: the ledger holds `r4133-linespacing-asym-303` with
/// `kind: "skip"` (an r4133 access violation while compiling the deck's
/// `tscables=`/`wires=` spacing), so the corpus gate's scheduler skips the
/// channel. A census derivation that called such a case "in scope" would assert
/// a cell count the gate cannot produce, so the derivations ask both questions
/// and this reader answers the second.
///
/// Read-only, like every other use of [`LEDGER`] here.
fn r4133_skipped_cases() -> BTreeSet<String> {
    let path = repo_root().join(LEDGER);
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
    )
    .expect("ledger.json is JSON");
    doc["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array")
        .iter()
        .filter(|e| e["channel"] == "r4133" && e["kind"] == "skip")
        .map(|e| {
            e["case"]
                .as_str()
                .unwrap_or_else(|| panic!("a ledger `skip` entry names its case: {e}"))
                .to_string()
        })
        .collect()
}

/// Every file under `dir`, as forward-slashed paths relative to `base` — the
/// universe [`collect_dss`] filters down to one extension, and what
/// [`redirected_non_dss_scripts`] needs in order to find a redirected `.txt`.
fn collect_files(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            collect_files(&p, base, out);
        } else {
            out.push(
                p.strip_prefix(base)
                    .expect("under base")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

/// Every `.dss` file under `dir`, as forward-slashed paths relative to `base`
/// (`corpus_manifest.rs::collect_dss`'s shape).
fn collect_dss(dir: &std::path::Path, base: &std::path::Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            collect_dss(&p, base, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dss")) {
            out.push(
                p.strip_prefix(base)
                    .expect("under base")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

/// How a deck line relates to one class's **element scope** — the rule both
/// census decompositions read their decks with.
///
/// `New` opens a fresh declaration; `Edit` and `BatchEdit` re-open an existing
/// element's scope, so a token typed there is typed on the same element and
/// counts for the same claims. Anything else closes whatever scope was open,
/// unless it is a `~` continuation of it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Scope {
    /// `New <class>.<name>` — the deck declares one here.
    Declares,
    /// `Edit`/`BatchEdit <class>.<name>` — the same element, typed on later.
    Edits,
    /// A line of some other class (or no element at all).
    Other,
}

/// The [`Scope`] `lower` (already trimmed and lower-cased) opens for
/// `class_dot`: `"swtcontrol."`, `"windgen."`.
///
/// Two spellings this must not miss:
///
/// * the identifier may be **quoted**: `New "WindGen.w1" …` is the form the
///   vendored corpus itself writes for other classes
///   (`electricdss-tst/Test/IndMachTest.DSS`, `New "IndMach012.windgen1"`),
///   so a reader that accepted only the bare spelling would let a quoted
///   declaration slip past a completeness claim;
/// * an **`Edit`/`BatchEdit`** types on the element just as its `New` does, so a
///   claim like "no deck types `kvar=`" is only true if those lines are read
///   too.
fn element_scope(lower: &str, class_dot: &str) -> Scope {
    for (verb, scope) in [
        ("new ", Scope::Declares),
        ("edit ", Scope::Edits),
        ("batchedit ", Scope::Edits),
    ] {
        let Some(rest) = lower.strip_prefix(verb) else {
            continue;
        };
        if rest
            .trim_start()
            .trim_start_matches(['"', '\''])
            .starts_with(class_dot)
        {
            return scope;
        }
    }
    Scope::Other
}

/// The `keys` a deck types inside `class_dot`'s element scope, plus how many
/// elements of the class it **declares** (an `Edit` declares none).
///
/// Values are lower-cased, `""` for a key the deck never types. The scope rule
/// is [`element_scope`]'s, so a `pf=` on a Load elsewhere in the deck is not
/// this machine's and a `delay=` on a Relay is not this control's; comment lines
/// are skipped, which is how `midi_swtcontrol.dss`'s `// Delayed open of the
/// LOOP TIE` header stays out. A key typed twice with two different values is a
/// hard error: every claim built on this reader assumes one spelling per deck.
fn element_tokens<const N: usize>(
    deck: &str,
    class_dot: &str,
    keys: [&str; N],
) -> (usize, [String; N]) {
    let path = repo_root().join(CORPUS).join(deck);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read deck {}: {e}", path.display()));
    let mut declared = 0usize;
    let mut tok = [const { String::new() }; N];
    let mut inside = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with("//") {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        match element_scope(&lower, class_dot) {
            Scope::Declares => {
                declared += 1;
                inside = true;
            }
            Scope::Edits => inside = true,
            Scope::Other => {
                if !lower.starts_with('~') {
                    inside = false;
                }
            }
        }
        if !inside {
            continue;
        }
        for (slot, key) in keys.iter().enumerate() {
            let Some(value) = named_token(&lower, key) else {
                continue;
            };
            if tok[slot].is_empty() {
                tok[slot] = value;
            } else {
                assert_eq!(
                    tok[slot], value,
                    "{deck}: two different `{key}` tokens inside a `{class_dot}` scope — the \
                     decomposition assumes one spelling per deck"
                );
            }
        }
    }
    (declared, tok)
}

/// What one deck says about `SwtControl`: how many it declares, and the `delay=`
/// token it types on them (`None` when it types none — then the port renders
/// `Create`'s 120.0, exactly as r4133 does, and the census sees no cell).
fn swtcontrol_facts(deck: &str) -> (usize, Option<String>) {
    let (declared, [delay]) = element_tokens(deck, "swtcontrol.", ["delay="]);
    (declared, (!delay.is_empty()).then_some(delay))
}

/// The value of `key` (`"kw="`, `"pf="`, …) typed as a whole parameter on
/// `line`, which must already be lower-cased.
///
/// The match must begin the line or follow a separator, so `pf=` is not found
/// inside a longer parameter name and `kva=` is not found inside a value. The
/// trailing `=` in `key` is what keeps `kv=` out of `kva=` and `kva=` out of
/// `kvar=` — the four tokens this file reads are prefixes of one another.
fn named_token(line: &str, key: &str) -> Option<String> {
    line.match_indices(key)
        .find(|(at, _)| {
            line[..*at]
                .chars()
                .next_back()
                .is_none_or(|c| c.is_whitespace() || c == '~' || c == ',')
        })
        .map(|(at, _)| {
            line[at + key.len()..]
                .chars()
                .take_while(|c| !c.is_whitespace())
                .collect()
        })
}

/// What one deck says about `WindGen`: how many it declares, and the `kW=`,
/// `pf=`, `kVA=` and `kvar=` tokens typed **inside the machine's own element
/// scope** ([`element_tokens`]), lower-cased, with `""` for a token the deck
/// never types.
///
/// The fourth token is read for a claim rather than a derivation — **no** corpus
/// deck types `kvar=` at all, on a `New` line or on a later `Edit`, so every
/// value in the census is a side effect of `pf=`/`kVA=`, and
/// [`the_windgen_kvar_census_decomposition_is_read_off_the_corpus`] asserts that instead
/// of assuming it.
fn windgen_facts(deck: &str) -> (usize, String, String, String, String) {
    let (declared, [kw, pf, kva, kvar]) =
        element_tokens(deck, "windgen.", ["kw=", "pf=", "kva=", "kvar="]);
    (declared, kw, pf, kva, kvar)
}

/// Whether a deck selects the **NCIM** solver: `set algorithm=NCIM` on a live
/// (non-comment) line, in any case and in any spelling the engine accepts.
///
/// This is the `generator.model` mechanism filter: the only thing that moves a
/// `Generator`'s live `GenModel` away from the token its deck typed is the NCIM
/// PV to PQ conversion, and r4133's `model` getter renders that token forever.
fn sets_ncim(deck: &str) -> bool {
    script_sets_ncim(deck, &read_script(&repo_root().join(CORPUS).join(deck)))
}

/// [`sets_ncim`] over a script's text — the form the non-`.dss` sweep needs,
/// since those files are reached by `Redirect` rather than by extension.
fn script_sets_ncim(name: &str, text: &str) -> bool {
    algorithm_values(name, text).iter().any(|v| selects_ncim(v))
}

/// Every value a script's live lines assign to the solver `algorithm` option,
/// **abbreviated spellings included**.
///
/// Comment lines are skipped (both `ncim_midi.dss` and `ncim_pq.dss` describe
/// the setting in their headers), and a live line that mentions `algorithm` in a
/// spelling [`named_token`] cannot parse is a hard error rather than a silent
/// miss, because a completeness sweep that quietly skips a deck proves nothing.
///
/// **Abbreviations are read.** `Set` resolves an option name by prefix match, so
/// every non-empty prefix of `algorithm` is read as the option, longest first.
/// The reader is deliberately WIDER than the engine (a short prefix may resolve
/// to some other option that comes earlier in the option list): a false hit here
/// is a loud failure a human reads, a miss is exactly the silent skip this sweep
/// exists to prevent.
fn algorithm_values(name: &str, text: &str) -> Vec<String> {
    const OPTION: &str = "algorithm";
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with("//") {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let parsed = (1..=OPTION.len())
            .rev()
            .find_map(|n| named_token(&lower, &format!("{}=", &OPTION[..n])));
        match parsed {
            Some(value) => out.push(value),
            None => assert!(
                !lower.contains(OPTION),
                "{name}: `{line}` names the solver algorithm in a spelling this reader cannot \
                 parse — the NCIM sweep would silently miss the deck"
            ),
        }
    }
    out
}

/// Whether an `algorithm=` value selects NCIM, by r4133's own reading of it.
///
/// r4133 compares only the **first two characters** (`ne` is Newton, `nc` is
/// NCIM, anything else the normal fixed point), so `Set algorithm=nc` selects
/// NCIM just as `NCIM` does, and an equality test against `"ncim"` would miss
/// it without reaching the hard error above.
fn selects_ncim(value: &str) -> bool {
    value.to_ascii_lowercase().starts_with("nc")
}

/// Read one corpus script, lossily — a `Redirect`ed `.txt` is not guaranteed to
/// be UTF-8, and a decoding error must not silently drop it from a completeness
/// sweep (the missing byte cannot be `Set algorithm=…`).
fn read_script(path: &std::path::Path) -> String {
    let bytes =
        std::fs::read(path).unwrap_or_else(|e| panic!("read script {}: {e}", path.display()));
    String::from_utf8_lossy(&bytes).into_owned()
}

/// The raw `Redirect`/`Compile` argument of every such line in a script,
/// cleaned of the corpus's quoting spellings — quoted (`Redirect "Master.dss"`),
/// bracketed (`[Master_ckt5.dss]`), parenthesised (`(Master.dss)`), with a
/// trailing `!` or `//` comment — and back-slashes forward-slashed. Case is
/// preserved: [`redirect_targets`] lower-cases the basename it takes, while
/// [`redirect_paths`] needs the argument as written in order to resolve it.
fn redirect_args(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with("//") {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        let Some(verb) = ["redirect ", "compile "]
            .iter()
            .find(|v| lower.starts_with(**v))
        else {
            continue;
        };
        let rest = &line[verb.len()..];
        let arg = rest
            .split('!')
            .next()
            .unwrap_or(rest)
            .split("//")
            .next()
            .unwrap_or(rest)
            .trim()
            .trim_matches(|c: char| {
                c == '"' || c == '\'' || c == '(' || c == ')' || c == '[' || c == ']'
            })
            .trim();
        if !arg.is_empty() {
            out.push(arg.replace('\\', "/"));
        }
    }
    out
}

/// The `Redirect`/`Compile` targets a script names, as lower-cased **basenames**.
///
/// Basenames, not resolved paths: the corpus writes the argument quoted
/// (`Redirect "Master.dss"`), bracketed (`[Master_ckt5.dss]`), parenthesised
/// (`(Master.dss)`), relative with `..` segments, absolute (`C:\…`) and with a
/// trailing `!` comment — and a resolver that got one of those spellings wrong
/// would narrow a completeness sweep silently. A basename set is a SUPERSET of
/// the real targets, which is the safe direction for a sweep.
fn redirect_targets(text: &str) -> Vec<String> {
    redirect_args(text)
        .iter()
        .filter_map(|arg| {
            let base = arg.rsplit('/').next().unwrap_or(arg).trim();
            (!base.is_empty()).then(|| base.to_ascii_lowercase())
        })
        .collect()
}

/// **The scripts a corpus deck executes that [`collect_dss`] cannot see**: the
/// transitive `Redirect`/`Compile` closure of `decks`, restricted to files whose
/// extension is not `dss`, as corpus-relative paths.
///
/// `collect_dss` filters on the extension, so a sweep built on it walks `.dss`
/// files only, while the vendored corpus redirects other ones too
/// (`Examples/Scripts/WireData.txt`, `ckt24/AllocationFactors_Base.Txt`, the
/// LVTestCase's `.txt` parts). A `Set algorithm=NCIM` inside one of those would
/// run and be invisible to the sweep, a hole in a *completeness* claim. Matching
/// is by basename ([`redirect_targets`]) over every corpus file, and the closure
/// is transitive because a redirected script may redirect further.
fn redirected_non_dss_scripts(decks: &[String]) -> Vec<String> {
    let root = repo_root().join(CORPUS);
    let mut all = Vec::new();
    collect_files(&root, &root, &mut all);
    let mut by_name: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for f in &all {
        by_name
            .entry(f.rsplit('/').next().unwrap_or(f).to_ascii_lowercase())
            .or_default()
            .push(f.clone());
    }
    let mut queue: Vec<String> = decks.to_vec();
    let mut seen: BTreeSet<String> = decks.iter().cloned().collect();
    let mut out = Vec::new();
    while let Some(f) = queue.pop() {
        for target in redirect_targets(&read_script(&root.join(&f))) {
            if target.ends_with(".dss") {
                continue; // already in `collect_dss`'s universe
            }
            for path in by_name.get(&target).into_iter().flatten() {
                if seen.insert(path.clone()) {
                    out.push(path.clone());
                    queue.push(path.clone());
                }
            }
        }
    }
    out.sort();
    out
}

/// What one deck says about `Generator`: how many it declares, and the `model=`
/// token typed inside their element scope ([`element_tokens`]), `""` for a deck
/// that types none — then both engines answer `Create`'s default 1
/// (`generator.pas:924`), which is not a PV bus and never converts.
fn generator_model_facts(deck: &str) -> (usize, String) {
    let (declared, [model]) = element_tokens(deck, "generator.", ["model="]);
    (declared, model)
}

/// The class prefix [`gictransformer_elements`] reads its scopes with.
const GIC_CLASS: &str = "gictransformer.";
/// The tokens the `gictransformer.r2` derivation needs, in
/// [`RP34_GIC_ELEMENTS`]' column order: the two percentages, the two ohms
/// values, and the winding-2 base.
///
/// The trailing `=` and [`named_token`]'s separator rule are what keep these
/// apart: in `%R1=0.2` the character before `r1=` is `%`, which is neither the
/// line start nor a separator, so the ohms key does **not** match a percentage
/// token (and vice versa, `%r1=` needs the `%`). That distinction is the whole
/// spec/branch discriminator, so it is exercised on the real decks by
/// [`the_gictransformer_reader_separates_the_percentage_and_ohms_specs`].
const GIC_KEYS: [&str; 6] = ["%r1=", "%r2=", "r1=", "r2=", "kvll2=", "mva="];

/// **Every `GICTransformer` one corpus file declares, per element**, with the
/// [`GIC_KEYS`] tokens typed inside that element's own scope (`""` for one it
/// never types).
///
/// [`element_tokens`] cannot serve here: it collapses a whole class to one scope
/// per deck and hard-errors when a key appears twice with different values, and
/// both `gic/*` decks type `R1=0.12` on their GSU and `R1=0.2` on their YY. The
/// scope rule is still [`element_scope`]'s (a `New` opens a declaration, an
/// `Edit`/`BatchEdit` re-opens the named element's, a `~` continues whatever is
/// open, anything else closes it), so an `R1=` on a Reactor two lines down is
/// not this transformer's.
///
/// Two things are hard errors rather than silent readings, because the
/// derivation's conclusion is a *count* of ledger entries:
///
/// * an `Edit`/`BatchEdit` naming an element the file never declared (the
///   corpus has none, and a wildcard `BatchEdit` would otherwise be attributed
///   to no element at all);
/// * the same key typed twice with two different values inside one element's
///   scope (the derivation assumes one spelling per element, exactly as
///   [`element_tokens`] does per deck).
fn gictransformer_elements(rel: &str) -> Vec<(String, [String; 6])> {
    let text = read_script(&repo_root().join(CORPUS).join(rel));
    let mut out: Vec<(String, [String; 6])> = Vec::new();
    let mut cur: Option<usize> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with("//") {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        match element_scope(&lower, GIC_CLASS) {
            Scope::Declares => {
                out.push((gictransformer_name(&lower), [const { String::new() }; 6]));
                cur = Some(out.len() - 1);
            }
            Scope::Edits => {
                let name = gictransformer_name(&lower);
                cur = Some(out.iter().position(|(n, _)| *n == name).unwrap_or_else(|| {
                    panic!(
                        "{rel}: `{line}` re-opens a GICTransformer scope this file never \
                                 declared ({name:?}) — the per-element decomposition cannot \
                                 attribute its tokens"
                    )
                }));
            }
            Scope::Other => {
                if !lower.starts_with('~') {
                    cur = None;
                }
            }
        }
        let Some(i) = cur else {
            continue;
        };
        for (slot, key) in GIC_KEYS.iter().enumerate() {
            let Some(value) = named_token(&lower, key) else {
                continue;
            };
            if out[i].1[slot].is_empty() {
                out[i].1[slot] = value;
            } else {
                assert_eq!(
                    out[i].1[slot], value,
                    "{rel}: two different `{key}` tokens inside the `{}` scope — the \
                     decomposition assumes one spelling per element",
                    out[i].0
                );
            }
        }
    }
    out
}

/// The element name a `New`/`Edit` line opens a [`GIC_CLASS`] scope for, from an
/// already-lower-cased line: everything after the class prefix up to the first
/// whitespace or closing quote.
fn gictransformer_name(lower: &str) -> String {
    lower
        .split_once(GIC_CLASS)
        .map(|(_, rest)| rest)
        .unwrap_or_default()
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != '"' && *c != '\'')
        .collect()
}

/// A value at **8 significant digits** in normalized exponential form — the
/// precision `GICTransformer`'s `R1`/`R2` getter emits
/// (`Format('%.8g', [1.0/G2])`,
/// `Version8/Source/PDElements/GICTransformer.pas:723`, and the port's
/// `%.8g`-equivalent behind the same `INVERSE_VALUE` flag).
///
/// [`sig15`]'s reason applies one order down: `63.48*0.15/100` is
/// `0.09521999999999999` in `f64` and the frozen census spells the getter's
/// answer `'0.09522'`, so reconciling a derived value against the frozen
/// spelling needs the getter's own precision. It is not a tolerance — both sides
/// are reduced by the same rule and compared as strings, and the gap this pair
/// records is 25 %, eight orders above anything eight digits could hide.
fn sig8(v: f64) -> String {
    let s = format!("{v:.7e}");
    let (mant, exp) = s
        .split_once('e')
        .expect("Rust renders `{:e}` with an exponent");
    let mant = mant.trim_end_matches('0').trim_end_matches('.');
    format!("{mant}e{exp}")
}

/// Which arm of the steady-state Q dispatch a deck's `WindGen` selects: its
/// `QMode=` and `VV_Curve=` tokens, `""` when it types none.
///
/// r4133's steady-state dispatch has a PF arm (1), a volt-var arm (2) and no
/// arm for the default constant-Q mode 0, which dispatches zero. This engine
/// dispatches the base kvar in mode 0, so on a deck that types no `QMode=` the
/// two engines' dispatch differs (ledger cause `windgen-qmode0-no-arm`) while
/// their *renders* keep the mechanism this table is about. The five census decks
/// type **no** `QMode=`, so r4133's render is the dispatched zero, whereas the
/// two decks held out of the population type `QMode=2` with a real curve and
/// take the volt-var arm, a *different* mechanism, which is why nothing here may
/// claim a measured r4133 value for them ([`RP32_WINDGEN_SKIPPED_DECKS`]).
fn windgen_dispatch(deck: &str) -> (String, String) {
    let (_, [qmode, vv_curve]) = element_tokens(deck, "windgen.", ["qmode=", "vv_curve="]);
    (qmode, vv_curve)
}

/// The base kvar a `WindGen` declaration derives from its own tokens — the two
/// branches of `Version8/Source/PCElements/WindGen.pas`, which the port ports
/// verbatim.
///
/// * **no `kVA=`** (`kVANotSet`): `SyncUpPowerQuantities` sets
///   `kvarBase := kWBase * sqrt(1/PF^2 - 1)`
///   (`crates/dss-core/src/elements/pc/windgen/nominal.rs:51`);
/// * **`kVA=` typed**: `RecalcElementData` (`:1375-1378`) re-derives
///   `kWBase := kVArating*|PF|` and `kvarBase := sqrt(kVA^2 - kWBase^2)`
///   (`nominal.rs:280-281`), so a deck that types `kW=` and `kVA=` but no `pf=`
///   lands on `Create`'s `PFNominal = 0.88` (`WindGen.pas:917`) and its typed
///   `kW=` never reaches the result.
///
/// The expressions are written in the engine's own order and grouping so the
/// derived `f64` is the port's bit-for-bit, not merely close to it.
fn windgen_kvar_base(kw: &str, pf: &str, kva: &str) -> f64 {
    // `Create`'s defaults (`WindGen.pas:911-917`) for whatever the deck omits.
    let kw: f64 = if kw.is_empty() {
        1000.0
    } else {
        kw.parse().unwrap_or_else(|e| panic!("kW={kw}: {e}"))
    };
    let pf: f64 = if pf.is_empty() {
        0.88
    } else {
        pf.parse().unwrap_or_else(|e| panic!("pf={pf}: {e}"))
    };
    if kva.is_empty() {
        kw * (1.0 / pf.powi(2) - 1.0).sqrt()
    } else {
        let kva: f64 = kva.parse().unwrap_or_else(|e| panic!("kVA={kva}: {e}"));
        let kw_base = kva * pf.abs();
        (kva.powi(2) - kw_base.powi(2)).sqrt()
    }
}

/// A value in normalized exponential form at **15 significant digits** — FPC
/// `FloatToStr`'s precision, which is what the port's property render
/// (`util::float_to_str`) and therefore the frozen census's decimal spelling
/// carry.
///
/// Reconciling a derived `f64` against a frozen spelling needs this and not an
/// `==`: `986.05231553659` is the 15-digit rendering of `986.0523155365896`, so
/// parsing it back gives a different `f64`. It is not a tolerance — both sides
/// are rendered by the same rule and compared as strings, so a real change in
/// the derivation still reds.
fn sig15(v: f64) -> String {
    let s = format!("{v:.14e}");
    let (mant, exp) = s
        .split_once('e')
        .expect("Rust renders `{:e}` with an exponent");
    let mant = mant.trim_end_matches('0').trim_end_matches('.');
    format!("{mant}e{exp}")
}

/// Non-empty lines with the CR of a CRLF file stripped. The header line is
/// asserted and dropped when `header` is `Some`.
///
/// `#` lines are dropped for [`SUPPLEMENT`] only: its provenance header is the
/// bin assignment of the pairs it holds (the `# BIN` lines among them are read
/// by [`supplement_bin_declarations`]). The frozen extracts carry no comment,
/// and keeping the filter off them means a `#` line inserted into one is a data
/// row here too, i.e. loud (`props_r4133_evidence_lock.rs` makes the same split
/// for the same reason).
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

/// The [`SUPPLEMENT`] header's machine-readable bin declarations, in file
/// order: one `# BIN <pair> <bin>` line per pair no [`RP1_RECORDS`] entry bins
/// (`regcontrol.fwdthreshold`, `regcontrol.revthreshold`). The line carries the
/// bin only. The pair's cells are the sum of its data rows, which
/// [`Corpus::load`] adds up, so neither number lives in this file as a literal.
///
/// A `# BIN ` line that is not exactly `<class.prop> <bin 1..=7>` panics, as
/// does a pair declared twice or a header that declares nothing: the
/// declaration is evidence, so a malformed one must be loud rather than
/// skipped. `props_r4133_evidence_lock.rs` (`SUPPLEMENT_BINS`) pins the same
/// two lines as bytes and checks each bin against its pair's data rows.
fn supplement_bin_declarations() -> Vec<(String, u8)> {
    let mut out: Vec<(String, u8)> = Vec::new();
    for line in read(SUPPLEMENT).lines() {
        let Some(decl) = line.trim_end_matches('\r').strip_prefix("# BIN ") else {
            continue;
        };
        let fields: Vec<&str> = decl.split(' ').collect();
        let [pair, bin] = fields[..] else {
            panic!("{SUPPLEMENT}: {line:?} is not `# BIN <pair> <bin>`");
        };
        assert!(
            pair.contains('.') && !pair.starts_with('.') && !pair.ends_with('.'),
            "{SUPPLEMENT}: {line:?} names no class.prop pair"
        );
        let bin: u8 = bin
            .parse()
            .ok()
            .filter(|b| (1..=7).contains(b))
            .unwrap_or_else(|| panic!("{SUPPLEMENT}: {line:?} declares no bin 1..=7"));
        assert!(
            out.iter().all(|(p, _)| p != pair),
            "{SUPPLEMENT}: {pair} is declared by two `# BIN` lines"
        );
        out.push((pair.to_string(), bin));
    }
    assert!(
        !out.is_empty(),
        "{SUPPLEMENT}: the header declares no `# BIN` line — the two regcontrol pairs \
         would have no evidence row"
    );
    out
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

/// A numeric pair's frozen FULL-census `max_rel` (`bins.tsv` column 7), which
/// [`PairEvidence`] does not carry — the accounting only ever needs the in-scope
/// one. Read straight off the file so the anchor in
/// [`the_display_floors_live_only_spellings_reconcile_the_claims_census`] is the
/// vendored number and not a transcription.
fn frozen_max_rel(pair: &str) -> Option<f64> {
    data_rows(
        "bins.tsv",
        Some("pair\tkind\tbin\tsubbin\tcells\tcells_in_scope\tmax_rel\tmax_rel_in_scope"),
    )
    .into_iter()
    .filter_map(|line| {
        let f: Vec<&str> = line.split('\t').collect();
        (f[0] == pair && f[1] == "numeric").then(|| f[6].parse::<f64>().ok())?
    })
    .next()
}

/// One record of a pair a shape closure created: the only evidence that can
/// exist for a pair a shape gap was hiding. `cells_in_scope` is `None` where the
/// record has no in-scope split.
#[derive(Debug, Clone)]
struct Rp1Record {
    pair: &'static str,
    rust: &'static str,
    r4133: &'static str,
    cells: usize,
    cells_in_scope: Option<usize>,
    bin: u8,
    substep: CiteSrc,
}

/// The records of the pairs the shape closures created, one representative
/// spelling each (elided with `…` when long). `PROPS_NORM_R4133`'s
/// `Evidence::Rp1*` rows cite them, and
/// [`every_norm_row_matches_its_cited_evidence`] reads each citation back here.
const RP1_RECORDS: &[Rp1Record] = &[
    Rp1Record {
        pair: "generator.debugtrace",
        rust: "No",
        r4133: "no",
        cells: 273,
        cells_in_scope: None,
        bin: 1,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.enabled",
        rust: "Yes",
        r4133: "true",
        cells: 273,
        cells_in_scope: None,
        bin: 1,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "sensor.enabled",
        rust: "Yes",
        r4133: "true",
        cells: 61,
        cells_in_scope: None,
        bin: 1,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.status",
        rust: "Variable",
        r4133: "variable",
        cells: 273,
        cells_in_scope: None,
        bin: 2,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.dynamiceq",
        rust: "mydiffeq",
        r4133: "myDiffEq",
        cells: 2,
        cells_in_scope: None,
        bin: 2,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.dynout",
        rust: "",
        r4133: "[]",
        cells: 273,
        cells_in_scope: None,
        bin: 5,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.shaftdata",
        rust: "",
        r4133: "()",
        cells: 273,
        cells_in_scope: None,
        bin: 5,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.userdata",
        rust: "",
        r4133: "()",
        cells: 273,
        cells_in_scope: None,
        bin: 5,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.kva",
        rust: "83.3333333333333",
        r4133: "83.333",
        cells: 3,
        cells_in_scope: None,
        bin: 6,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.maxkvar",
        rust: "197.210463107318",
        r4133: "197.21",
        cells: 172,
        cells_in_scope: None,
        bin: 6,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.minkvar",
        rust: "-197.210463107318",
        r4133: "-197.21",
        cells: 172,
        cells_in_scope: None,
        bin: 6,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "generator.d",
        rust: "1",
        r4133: "0",
        cells: 272,
        cells_in_scope: None,
        bin: 7,
        substep: CiteSrc::Rp11,
    },
    Rp1Record {
        pair: "autotrans.enabled",
        rust: "Yes",
        r4133: "true",
        cells: 44,
        cells_in_scope: Some(9),
        bin: 1,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "autotrans.xrconst",
        rust: "No",
        r4133: "NO",
        cells: 44,
        cells_in_scope: Some(9),
        bin: 1,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "autotrans.bhcurrent",
        rust: "",
        r4133: "[]",
        cells: 44,
        cells_in_scope: Some(9),
        bin: 5,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "autotrans.bhflux",
        rust: "",
        r4133: "[]",
        cells: 44,
        cells_in_scope: Some(9),
        bin: 5,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "autotrans.emergamps",
        rust: "1553.19773504818",
        r4133: "1553.2",
        cells: 44,
        cells_in_scope: Some(9),
        bin: 6,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "autotrans.normamps",
        rust: "1139.01167236866",
        r4133: "1139",
        cells: 44,
        cells_in_scope: Some(9),
        bin: 6,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "autotrans.pctperm",
        rust: "0",
        r4133: "100",
        cells: 44,
        cells_in_scope: Some(9),
        bin: 7,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "autotrans.repair",
        rust: "0",
        r4133: "36",
        cells: 44,
        cells_in_scope: Some(9),
        bin: 7,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "autotrans.wdgcurrents",
        rust: "66.95905, (-28.006), 151.5029, (151.99), …",
        r4133: "66.98576, (-28.025), 156.2997, (151.97), …",
        cells: 35,
        cells_in_scope: Some(0),
        bin: 7,
        substep: CiteSrc::Rp12,
    },
    Rp1Record {
        pair: "windgen.enabled",
        rust: "Yes",
        r4133: "true",
        cells: 5,
        cells_in_scope: Some(5),
        bin: 1,
        substep: CiteSrc::Rp13,
    },
    Rp1Record {
        pair: "windgen.dynout",
        rust: "",
        r4133: "[]",
        cells: 5,
        cells_in_scope: Some(5),
        bin: 5,
        substep: CiteSrc::Rp13,
    },
    Rp1Record {
        pair: "gendispatcher.enabled",
        rust: "Yes",
        r4133: "true",
        cells: 48,
        cells_in_scope: Some(0),
        bin: 1,
        substep: CiteSrc::Rp14,
    },
];

/// `shape_in_scope.txt`: class -> rows on r4133-gating cases. A shape-closure
/// pair whose own record has no `(in scope)` split inherits its class's answer:
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
        for r in RP1_RECORDS {
            let class = r.pair.split('.').next().unwrap_or_default().to_string();
            let class_rows = *in_scope
                .get(&class)
                .unwrap_or_else(|| panic!("{}: shape_in_scope.txt has no {class} row", r.pair));
            // The record's own in-scope split when it has one. Otherwise the
            // class's answer: the pair became live exactly because that class's
            // shape gap closed, so a class with no in-scope element row has no
            // in-scope cell for any of its new pairs either. `None` = "in scope,
            // count not measured separately", which is the conservative reading:
            // the row then needs an owner.
            let cells_in_scope = r
                .cells_in_scope
                .or(if class_rows == 0 { Some(0) } else { None });
            supplement.insert(
                r.pair.to_string(),
                PairEvidence {
                    numeric: r.bin >= 6,
                    bin: r.bin,
                    cells: r.cells,
                    cells_in_scope,
                    // The records carry a per-pair `max_rel`, not an in-scope
                    // re-derivation, so a numeric supplement pair answers with
                    // its recorded bin (`effective_bin`).
                    max_rel_in_scope: None,
                    origin: format!("the {} records", substep_tag(r.substep)),
                },
            );
        }
        // The two pairs no census row can carry, each declared by its own
        // provenance: the supplement's `# BIN <pair> <bin>` header lines (the
        // prose above them carries the r4133 citations), and cells = the sum of
        // the pair's own data rows, so the echo rows' `cells` are compared
        // against the file, not a literal. Their in-scope split is not recorded,
        // so they read as in scope.
        for (pair, bin) in supplement_bin_declarations() {
            let cells: usize = rows
                .iter()
                .filter(|r| r.src == Source::Supplement && r.pair == pair)
                .map(|r| r.cells)
                .sum();
            assert!(
                cells > 0,
                "{SUPPLEMENT}: `# BIN {pair} {bin}` declares a pair with no data row"
            );
            let clash = supplement.insert(
                pair.clone(),
                PairEvidence {
                    numeric: bin >= 6,
                    bin,
                    cells,
                    cells_in_scope: None,
                    max_rel_in_scope: None,
                    origin: format!("examples_supplement.txt `# BIN {pair} {bin}` line"),
                },
            );
            assert!(
                clash.is_none(),
                "{pair}: declared both by a shape-closure record and a `# BIN` line"
            );
        }
        // Every supplement pair has exactly one declaration: the shape-closure
        // record or the `# BIN` line. A row whose pair neither names would
        // otherwise reach the accounting with no evidence.
        let row_pairs: BTreeSet<&str> = rows
            .iter()
            .filter(|r| r.src == Source::Supplement)
            .map(|r| r.pair.as_str())
            .collect();
        let declared: BTreeSet<&str> = supplement.keys().map(String::as_str).collect();
        assert_eq!(
            row_pairs, declared,
            "{SUPPLEMENT}: the data-row pairs and the declared pairs (shape-closure records + \
             `# BIN` lines) must be the same set"
        );

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
        CiteSrc::Rp11 => "generator/sensor shape-closure",
        CiteSrc::Rp12 => "XfmrCode shape-closure",
        CiteSrc::Rp13 => "WindGen shape-closure",
        CiteSrc::Rp14 => "GenDispatcher shape-closure",
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
    // The SHIPPED predicate, never a copy of it — same rule as the two links
    // above (a drifting copy would make this completeness proof describe a
    // comparator that does not exist). It reads `props_norm::display_floor()`
    // internally, so the accounting below moves with the constant.
    let floor = props_norm::under_display_floor(&row.rust, &row.r4133);
    [shape, norm, echo, floor]
}

/// The structural classification chain, applied to **one cell**: the
/// per-example-row admissibility rule.
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

/// **The declaration rule.** Which owner holds a row no link of the chain
/// claims, decided from the vendored evidence, in this order:
///
/// 0. a pair [`RP22_ROUTING`] names answers its verdict, the echo table or a
///    live-state divergence. A pair on the **closed S6/bin-3 list** that the
///    routing does *not* name and no link claimed is an **error**, because every
///    such pair owes a verdict;
/// 1. a **numeric** row answers its in-scope-effective bin: 6 -> the display
///    floor, 7 -> the echo table if it is a known bin-7 echo, the root-cause
///    bucket if it is one of the four root-cause pairs;
/// 2. an unclaimed cell of a **bin-1** pair whose r4133 side is not one of the
///    eleven boolean spellings is the pair's echo half -> the echo table;
/// 3. a cell whose own classification is **bin 5** (either side empty) -> the
///    echo table;
/// 4. …**bin 3** (an enum spelling) -> the `EnumSynonym` rows, which is only
///    reachable for a pair the routing does not name and is therefore the same
///    error as (0);
/// 5. …**bin 2 or 4** inside a pair no rule of this table claims: the explicit
///    [`CELL_DISPOSITION`] entry, else the pair's own bin-5 echo row, else, if
///    every cell of the pair is out of scope, [`Owner::OutOfScope`].
///
/// `Err` is **the kill criterion**: a census row that no typed rule claims and no
/// owner holds.
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
    // The `SilentReadOnly` pairs' frozen spellings are counted as superseded in
    // `account`, before this function is reached ([`RP38_SUPERSEDED`]). Their
    // cells classify as bin 5 (one side empty), so if that interception were
    // removed they would fall through to the bin-5 arm and land in
    // [`Owner::Rp23`]'s empty bucket, which reds [`DECLARED_RP23`].
    if let Some((_, owner, _)) = RP22_ROUTING.iter().find(|(p, _, _)| *p == row.pair) {
        return Ok(*owner);
    }
    if RP22_S6.contains(&row.pair.as_str()) || (!ev.numeric && ev.bin == 3) {
        return Err(format!(
            "{} '{}' vs '{}': on the closed S6/bin-3 pair list, unclaimed by the chain, and \
             RP22_ROUTING has no verdict for it — every such pair owes \
             exactly one recorded routing",
            row.pair, row.rust, row.r4133
        ));
    }
    // The display floor's MECHANISM residual, ahead of every bin rule: a numeric
    // cell whose gap is inside the display floor and whose r4133 side is no
    // `%.Ng` render of our value. The floor refuses it by construction, and the
    // reason has nothing to do with the pair's bin or scope: the two engines hold
    // different doubles. See [`RP39_ROUTING`].
    if ev.numeric && display_class_but_not_a_render(row) {
        return match RP39_ROUTING.iter().find(|(p, _, _, _, _)| *p == row.pair) {
            Some(_) => Ok(Owner::Rp39),
            None => Err(format!(
                "{} '{}' vs '{}': inside the display floor but no `%.Ng` render of our \
                 value, on a pair RP39_ROUTING does not cite — read the r4133 round-trip \
                 chain off the Pascal and cite it, or fix the port",
                row.pair, row.rust, row.r4133
            )),
        };
    }
    // The AutoTrans typecast bug, ahead of the bin rules for the same reason:
    // the cause is a property of the two engines' control paths, not of the
    // pair's bin or scope. It runs AFTER the clause above so the pair's ninth
    // spelling, the `modes:makeposseq` round-trip residue, stays
    // [`Owner::Rp39`]'s. See [`RP312_UPSTREAM_BUG`].
    if regcontrol_autotrans_typecast_row(row) {
        return Ok(Owner::Rp312);
    }
    if ev.numeric {
        return match ev.effective_bin() {
            // A display-class row the FLOOR did not claim (it runs before this
            // function, so everything inside 2e-4 is already gone). The only
            // admissible reason is that the row has no in-scope cell at all,
            // and the pair's own frozen ceiling proves it ([`RP24_OUT_OF_SCOPE`]).
            // Anything else is a kill: either the floor is mis-derived or a new
            // category appeared.
            6 => match row_out_of_scope_by_ceiling(row, ev) {
                Some(_) => Ok(Owner::OutOfScope),
                None => Err(format!(
                    "{} '{}' vs '{}': a display-class (in-scope bin 6) row that the floor \
                     does not claim, and whose pair ceiling (max_rel_in_scope {:?}) does not \
                     prove it out of scope — re-derive props_norm::R4133_DISPLAY_FLOOR, or add \
                     a cited RP24_OUT_OF_SCOPE row",
                    row.pair, row.rust, row.r4133, ev.max_rel_in_scope
                )),
            },
            7 if BIN7_ECHO.contains(&row.pair.as_str())
                || BIN7_ECHO_SUPPLEMENT.contains(&row.pair.as_str()) =>
            {
                Ok(Owner::Rp23)
            }
            7 if BIN7_ROOT_CAUSE.contains(&row.pair.as_str()) => Ok(Owner::Rp3),
            7 if !ev.in_scope() => Ok(Owner::OutOfScope),
            other => Err(format!(
                "{} '{}' vs '{}': numeric bin {other} on an IN-SCOPE pair that is on none of \
                 the bin-7 lists (echo / root-cause) — the bin-7 enumeration is \
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
        // A bin-3 CELL inside a pair the routing does not name: the same error as
        // (0). Closing bin 3 means closing its cells, and every one of them owes
        // a read getter and a verdict.
        3 => Err(format!(
            "{} '{}' vs '{}': a bin-3 (enum-spelling) cell on a bin-{} pair that RP22_ROUTING \
             does not name — bin 3 must be claimed FULLY, per cell",
            row.pair, row.rust, row.r4133, ev.bin
        )),
        2 | 4 => match (disposition, ev.bin, ev.in_scope()) {
            // `OutOfScope` is a claim about the evidence: it may only be written
            // for a pair the r4133 compare never reaches.
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

/// **The display floor's mechanism residual, as a row predicate**: the gap is
/// inside the derived display floor, and the r4133 spelling is still no `%.Ng`
/// render of our value.
///
/// Both halves read the SHIPPED predicates (`props_norm::display_rel` and
/// `props_norm::display_is_render`, the two clauses of
/// `props_norm::under_display_floor`), so "the floor refused it for the
/// mechanism" is asked of the comparator itself and not re-derived here.
fn display_class_but_not_a_render(row: &Example) -> bool {
    let Some(floor) = props_norm::display_floor() else {
        return false;
    };
    props_norm::display_rel(&row.rust, &row.r4133).is_some_and(|rel| rel <= floor)
        && !props_norm::display_is_render(&row.rust, &row.r4133)
}

/// **The AutoTrans typecast bug, as a row predicate**: the row is on a pair
/// [`RP312_UPSTREAM_BUG`] cites and is NOT the display-class residue
/// [`display_class_but_not_a_render`] hands to [`Owner::Rp39`].
///
/// The second half keeps the two owners' shares of `autotrans.wdgcurrents` apart
/// without either naming a deck: [`Owner::Rp39`] holds the one spelling that IS
/// inside the floor (the `modes:makeposseq` round trip), [`Owner::Rp312`] the
/// ones far outside it. Written as a predicate rather than a spelling list so a
/// re-measure that moves a row across the floor fails one of the two ownership
/// guards instead of silently re-filing itself.
fn regcontrol_autotrans_typecast_row(row: &Example) -> bool {
    RP312_UPSTREAM_BUG
        .iter()
        .any(|(pair, _, _, _, _, _)| *pair == row.pair)
        && !display_class_but_not_a_render(row)
}

/// **The row-level scope proof**: does the pair's own frozen `max_rel_in_scope`
/// show that this spelling has no cell the r4133 compare reaches? `Some(ratio)`
/// when it does, where `ratio` is the measured `gap / ceiling` (locked at
/// [`RP24_OUT_OF_SCOPE_MIN_RATIO`]).
///
/// `max_rel_in_scope` is the maximum over exactly the in-scope cells, so a
/// spelling that diverges by MORE than it cannot be one of them. Deliberately
/// narrow: it fires only on the four cited [`RP24_OUT_OF_SCOPE`] pairs, only on
/// numeric rows, and only past [`CEILING_ROUND_MARGIN`]. A general rule would be
/// a scope loophole any future pair could fall into unreviewed.
///
/// The gap is [`props_norm::display_rel`], the shipped metric the floor itself
/// is expressed in, so "above the ceiling" and "outside the floor" are read off
/// one function.
///
/// **Single-number rows only.** The two metrics are not the same function:
/// `display_rel` is symmetric and maximizes over ALL numbers of the cell, while
/// the census's `max_rel` is `|a−e|/|e|` over the numbers that FAIL the case's
/// tier floor (`harness::value_verdict`). On a one-number row the implication
/// holds: the row's only number IS the offender of any divergent cell it sits
/// on, and `|e| <= max(|a|,|b|)` makes the census metric the larger of the two,
/// so `gap > ceiling` really does exclude an in-scope cell. On a MULTI-number
/// row the maximum could be attained at a number the tier's `abs` term absorbs,
/// which contributes nothing to `max_rel_in_scope`, and the proof would not
/// close. The guard keeps every row the rule declares single-number.
fn row_out_of_scope_by_ceiling(row: &Example, ev: &PairEvidence) -> Option<f64> {
    if !ev.numeric {
        return None;
    }
    let (_, ceiling, _) = RP24_OUT_OF_SCOPE.iter().find(|(p, _, _)| *p == row.pair)?;
    if numbers_in(&row.r4133) != 1 || numbers_in(&row.rust) != 1 {
        return None;
    }
    let gap = props_norm::display_rel(&row.rust, &row.r4133)?;
    (gap > ceiling * CEILING_ROUND_MARGIN).then(|| gap / ceiling)
}

/// How many numbers a render carries, through the comparator's own scanner.
fn numbers_in(s: &str) -> usize {
    harness::numeric_skeleton(s).1.len()
}

/// Does this example row have at least one cell the r4133 compare reaches?
///
/// The pair's `cells_in_scope` is the coarse answer.
/// [`row_out_of_scope_by_ceiling`] refines it per row where the frozen evidence
/// supports it, which lets an in-scope PAIR carry a spelling that is provably
/// not.
fn row_in_scope(row: &Example, ev: &PairEvidence) -> bool {
    ev.in_scope() && row_out_of_scope_by_ceiling(row, ev).is_none()
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
                // A spelling an engine change made counterfactual is counted
                // here and asked nothing further: `declare` reads the frozen
                // `rust` column, which for these seven pairs records a port that
                // no longer exists. See [`RP37_SUPERSEDED`] and
                // [`RP38_SUPERSEDED`]. It sits AFTER the chain, so a link that
                // ever did claim one of these rows would be credited with it and
                // the bucket lock would red — the interception cannot hide a
                // claim.
                if let Some(tag) = superseded_tag(&row.pair) {
                    let b = led.superseded.entry(tag).or_default();
                    b.rows += 1;
                    b.pairs.insert(row.pair.clone());
                    if row_in_scope(row, ev) {
                        b.in_scope_rows += 1;
                    }
                    continue;
                }
                // …and a spelling a LANDED `ledger.json` `property` entry
                // excludes: the tree holds the exclusion, but it lives outside
                // `props_norm`, so no link of the chain can be credited with it
                // and no owner holds it. Same placement and the same reason as
                // the interception above: AFTER the chain, so a link that ever
                // did claim one of these rows would be credited with it and the
                // bucket lock would red. See [`RP3_LEDGERED`], whose entry ids are
                // checked against the live ledger.
                if RP3_LEDGERED.iter().any(|(p, ..)| *p == row.pair) {
                    led.ledgered.rows += 1;
                    led.ledgered.pairs.insert(row.pair.clone());
                    if row_in_scope(row, ev) {
                        led.ledgered.in_scope_rows += 1;
                    }
                    continue;
                }
                match declare(row, ev) {
                    Ok(owner) => {
                        let b = led.declared.entry(owner).or_default();
                        b.rows += 1;
                        b.pairs.insert(row.pair.clone());
                        if row_in_scope(row, ev) {
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

/// `PROPS_NORM_R4133` rows that claimed no example row: the offline half of
/// "liveness both ways".
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
/// first matching link of the chain or declared to a named owner, and every
/// count is locked in both directions.
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
        "the kill criterion: {} example row(s) fit no mechanism and no declared-pending \
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
        "the five EnumSynonym rows — the four bin-3 source sequence selectors plus the \
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
        "PROPS_ECHO_R4133's cited rows claim what the nine off-bin normalization rows leave, \
         minus the five SilentReadOnly pairs, plus generator.model"
    );
    assert_eq!(
        claimed(Link::DisplayFloor.tag()),
        CLAIMED_DISPLAY_FLOOR,
        "the display floor is the only tolerance in the chain"
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
    // what that owner holds: the guard against a pair silently vanishing from,
    // or appearing in, its residual.
    for (owner, want) in [
        (Owner::Rp22, DECLARED_RP22),
        (Owner::Rp23, DECLARED_RP23),
        (Owner::Rp24, DECLARED_RP24),
        (Owner::Rp3, DECLARED_RP3),
        (Owner::Rp35, DECLARED_RP35),
        (Owner::Rp38, DECLARED_RP38),
        (Owner::Rp39, DECLARED_RP39),
        (Owner::Rp312, DECLARED_RP312),
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
        "an OutOfScope row must have no in-scope cell at all — nothing else justifies \
         leaving a divergence unowned"
    );

    // …and the third state: the rows an engine change made counterfactual,
    // counted against the table that owns them.
    assert_eq!(
        led.superseded("silent-read-only"),
        SUPERSEDED_RP38,
        "the superseded bucket inherits (rows, pairs, rows on in-scope pairs) — the five \
         SilentReadOnly pairs and nothing else"
    );
    assert_eq!(
        led.superseded("swtcontrol-per-phase"),
        SUPERSEDED_RP37,
        "…and the two per-phase SwtControl pairs, out of DECLARED_RP35 and out of the \
         normalization link"
    );
    assert_eq!(
        led.superseded.keys().copied().collect::<Vec<_>>(),
        ["silent-read-only", "swtcontrol-per-phase"],
        "a third superseded table owes its own lock, never a share of these two"
    );

    // …and the fourth: the rows the landed r4133 `ledger.json` `property`
    // entries exclude.
    assert_eq!(
        led.ledgered(),
        LEDGERED_RP3,
        "the ledgered bucket inherits (rows, pairs, rows on in-scope pairs) — the three \
         root-cause pairs with ledger entries and nothing else"
    );

    // Totality, four ways.
    assert_eq!(
        led.claimed_total() + led.declared_total() + led.superseded_rows() + led.ledgered.rows,
        corpus.rows.len(),
        "every example row is claimed, declared, superseded or ledgered, exactly once"
    );
    assert_eq!(
        led.multi_link, MULTI_LINK_ROWS,
        "rows matched by more than one link — the per-cell echo narrowing keeps this 0; a \
         positive number here means an echo row covers a \
         spelling its pair's typed rule already claims"
    );
}

/// **`props_norm::ECHO_NARROWED` is re-derived from the corpus, both ways.**
///
/// The derivation is deliberately **not** the echo link: asking the chain which
/// rows `Link::Echo` claims would be circular now that the link reads the very
/// table under test. Instead it asks the two links that come BEFORE it (the
/// shape allowlist and the typed normalization rules) plus the carve-outs, and
/// takes what they leave. That set is, by definition, "the spellings the typed
/// rules refuse", i.e. the cells whose divergence the pair's `EchoRow` citation
/// is the only remaining explanation for. The table must hold exactly it.
///
/// Both directions, because either failure mode is a live regression:
/// * a spelling in the table that the corpus does not leave to the echo row =
///   a mask over a cell a typed rule already claims;
/// * a spelling the corpus leaves that the table lacks = a cell that would be
///   COMPARED without the row's citation behind it, so it has to be a reviewed
///   edit here.
///
/// It also pins the shape the narrowing depends on: the table names a pair iff
/// that pair is mixed (holds both kinds of row) or sits on the closed
/// `props_norm::ECHO_NARROWED_PURE_PAIRS` list (`generator.model`), so the other
/// pure echo rows keep the conservative pair scope. A listed pure pair is held
/// to the same Direction 2 as a mixed one: its spellings are exactly the census
/// rows no earlier link claims, i.e. all of its rows.
#[test]
fn the_narrowed_echo_rows_carry_exactly_the_spellings_the_typed_rules_leave() {
    /// Per pair: how many rows the links BEFORE the exclusion claim, and the
    /// spellings they leave to the echo row.
    type Leftover<'a> = (usize, BTreeSet<(&'a str, &'a str)>);

    let corpus = Corpus::load();
    let mut left: BTreeMap<&str, Leftover<'_>> = BTreeMap::new();
    for row in &corpus.rows {
        if !props_norm::has_echo_row(&row.class, &row.prop) {
            continue;
        }
        let e = left.entry(row.pair.as_str()).or_default();
        let [shape, norm, _echo, _floor] = chain_verdicts(&corpus, row);
        let carved = ECHO_CARVE_OUT_ROUTING
            .iter()
            .any(|(p, r, o, _, _)| *p == row.pair && *r == row.rust && *o == row.r4133);
        if shape || norm {
            e.0 += 1;
        } else if !carved {
            // A carved-out cell is neither: its own routing hands it to a later
            // link (`ECHO_CARVE_OUT_ROUTING`), and it never belonged to the echo
            // row's scope in the first place.
            e.1.insert((row.rust.as_str(), row.r4133.as_str()));
        }
    }
    let mixed: BTreeMap<&str, &BTreeSet<(&str, &str)>> = left
        .iter()
        .filter(|(_, (claimed, _))| *claimed > 0)
        .map(|(p, (_, sp))| (*p, sp))
        .collect();

    // The closed list of pure pairs narrowed on purpose: each is a pure echo
    // pair (no earlier link claims a row of it) that the census does carry.
    let pure: BTreeMap<String, &BTreeSet<(&str, &str)>> = props_norm::ECHO_NARROWED_PURE_PAIRS
        .iter()
        .map(|(class, prop)| {
            let pair = format!("{class}.{prop}");
            let (claimed, spellings) = left.get(pair.as_str()).unwrap_or_else(|| {
                panic!("{pair}: listed in ECHO_NARROWED_PURE_PAIRS but no census row carries it")
            });
            assert_eq!(
                *claimed, 0,
                "{pair}: listed as a PURE narrowed pair, yet an earlier link claims {claimed} \
                 of its census row(s) — a mixed pair belongs to the derivation below"
            );
            (pair, spellings)
        })
        .collect();

    // Direction 1: the table names exactly the mixed pairs plus the listed pure
    // ones.
    let named: BTreeSet<String> = props_norm::ECHO_NARROWED
        .iter()
        .map(|r| format!("{}.{}", r.class, r.prop))
        .collect();
    let measured: BTreeSet<String> = mixed
        .keys()
        .map(|p| p.to_string())
        .chain(pure.keys().cloned())
        .collect();
    assert_eq!(
        named, measured,
        "ECHO_NARROWED must hold exactly the pairs that carry BOTH a typed rule and an echo \
         row, plus ECHO_NARROWED_PURE_PAIRS — the other pure echo rows keep the pair \
         scope"
    );

    // Direction 2: per pair, exactly the spellings the earlier links leave.
    let every = mixed
        .iter()
        .map(|(p, sp)| (*p, *sp))
        .chain(pure.iter().map(|(p, sp)| (p.as_str(), *sp)));
    for (pair, want) in every {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        let got: BTreeSet<(&str, &str)> = props_norm::narrowed_spellings(class, prop)
            .unwrap_or_else(|| panic!("{pair}: a mixed pair with no ECHO_NARROWED row"))
            .iter()
            .copied()
            .collect();
        assert_eq!(
            got, *want,
            "{pair}: ECHO_NARROWED must carry exactly the census spellings the typed rules leave"
        );
    }

    // …and the two locks the table states, re-derived here rather than read
    // back off the table itself: 20 mixed pairs leaving 66 spellings, plus
    // `generator.model`'s one, for 21 rows and 67 spellings in all.
    assert_eq!(mixed.len(), 20, "mixed pairs");
    assert_eq!(
        mixed.values().map(|s| s.len()).sum::<usize>(),
        66,
        "the spellings the 20 mixed pairs leave to their echo rows"
    );
    assert_eq!(
        (named.len(), pure.values().map(|s| s.len()).sum::<usize>()),
        (21, 1),
        "ECHO_NARROWED's rows, and the spellings of its one listed pure pair (generator.model)"
    );
    assert_eq!(
        left.values().map(|(claimed, _)| claimed).sum::<usize>(),
        MIXED_PAIR_NORM_ROWS,
        "…and their complement is the mixed-pair normalization lock"
    );
}

/// **`props_norm::NORM_ROWS_WITH_NO_IN_SCOPE_CELL` is the census's, both ways**:
/// the normalization half of the declaration the echo half carries
/// (`props_norm::echo_rows_with_no_in_scope_cell`).
///
/// The live guard `props_norm::assert_norm_rows_are_live` reports a row only
/// when a full gate run visited it and it folded nothing, so a row whose pair
/// has no cell on an r4133-gating case is never visited and silent forever.
/// Which rows those are is a census fact, read here the way [`Corpus::load`]
/// resolves it: `bins.tsv`'s `cells_in_scope` for a `BinsTsv` row, for a
/// shape-closure row its record's in-scope split or, where the record has none,
/// its class's `rows_in_scope` in `shape_in_scope.txt`. Both directions fail
/// with their own message: an unlisted row with no in-scope cell is an
/// undeclared hole, a listed row that has one (what gating a sensor or
/// gendispatcher deck on r4133 would produce) is a stale declaration, and the
/// row then belongs to the live guard again. The class side is read straight
/// off `shape_in_scope.txt` as well: a class with `rows_in_scope=0` can hold no
/// in-scope cell on any pair, and every listed row's class is such a class.
#[test]
fn the_norm_rows_with_no_in_scope_cell_are_exactly_the_declared_ones() {
    let corpus = Corpus::load();
    let class_rows = shape_rows_in_scope();
    let declared: BTreeSet<(&str, &str)> = props_norm::NORM_ROWS_WITH_NO_IN_SCOPE_CELL
        .iter()
        .copied()
        .collect();
    let mut dormant: BTreeSet<(&str, &str)> = BTreeSet::new();
    let mut problems: Vec<String> = Vec::new();
    for row in props_norm::PROPS_NORM_R4133 {
        let pair = format!("{}.{}", row.class, row.prop);
        let in_scope: Option<usize> = match row.src {
            props_norm::Evidence::BinsTsv => {
                let records = corpus
                    .bins
                    .get(&pair)
                    .unwrap_or_else(|| panic!("{pair}: a BinsTsv row with no bins.tsv record"));
                Some(
                    records
                        .iter()
                        .map(|e| e.cells_in_scope.expect("bins.tsv records the split"))
                        .sum(),
                )
            }
            _ => {
                corpus
                    .supplement
                    .get(&pair)
                    .unwrap_or_else(|| panic!("{pair}: a shape-closure row with no record"))
                    .cells_in_scope
            }
        };
        if in_scope == Some(0) {
            dormant.insert((row.class, row.prop));
        }
        if class_rows.get(row.class) == Some(&0) && in_scope != Some(0) {
            problems.push(format!(
                "{pair}: shape_in_scope.txt gives its class rows_in_scope=0, yet the pair \
                 records {in_scope:?} in-scope cell(s)"
            ));
        }
    }
    for (class, prop) in dormant.difference(&declared) {
        problems.push(format!(
            "{class}.{prop}: no cell on an r4133-gating case and NOT declared in \
             props_norm::NORM_ROWS_WITH_NO_IN_SCOPE_CELL — the live guard can never visit it, \
             so the hole must be named"
        ));
    }
    for (class, prop) in declared.difference(&dormant) {
        problems.push(format!(
            "{class}.{prop}: declared in props_norm::NORM_ROWS_WITH_NO_IN_SCOPE_CELL but it DOES \
             carry an in-scope cell, or it is no PROPS_NORM_R4133 row at all — the live guard \
             covers an in-scope row, so drop the declaration"
        ));
    }
    for (class, prop) in &declared {
        if class_rows.get(*class) != Some(&0) {
            problems.push(format!(
                "{class}.{prop}: a declared row's class must have rows_in_scope=0 in \
                 shape_in_scope.txt (no r4133-gating case holds an element of it), got {:?}",
                class_rows.get(*class)
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "props_norm::NORM_ROWS_WITH_NO_IN_SCOPE_CELL no longer matches the frozen census:\n{}",
        problems.join("\n")
    );
    assert_eq!(
        dormant.len(),
        8,
        "the gendispatcher (3) and sensor (5) rows — every corpus deck declaring either class \
         gates capi_v0145 only"
    );
}

/// **The per-pair split of the OFFLINE attribution** on the pairs that hold
/// both kinds of row: the per-pair half of [`MIXED_PAIR_NORM_ROWS`].
///
/// For every example row of a cited pair it asks the chain which link is
/// credited with it, and pins the resulting split pair by pair. That is a
/// statement about the **census attribution** (the claims-census disposition
/// and the norm rows' liveness), not about the live comparator, whose exclusion
/// is pair-scoped and covers a mixed pair's refused cells too
/// (`harness::props_policy_tests::a_mixed_pairs_echo_row_masks_the_cells_its_
/// rule_refuses` pins that behaviour where it lives).
///
/// It catches deleting or narrowing one of the nine off-bin normalization rows,
/// and re-ordering the offline chain so the exclusion is asked first: either
/// moves a pair's normalization count to zero.
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
            // A carved-out cell of a cited pair: the exclusion does NOT cover
            // it, so the chain walks past it to the next link, and the display
            // floor takes the cell, which is exactly what
            // `ECHO_CARVE_OUT_ROUTING` declares.
            Some(Link::DisplayFloor)
                if ECHO_CARVE_OUT_ROUTING
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
        MIXED_PAIR_NORM_ROWS,
        "the per-pair normalization counts must sum to the mixed-pair lock"
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

/// **Liveness both ways for the echo table, offline**: the exclusion's half of
/// "liveness both ways", and the mirror of
/// [`every_normalization_row_claims_at_least_one_example_row`].
///
/// A row that claims no example row is masking a divergence the vendored census
/// never recorded: it either names the wrong pair or is obsolete. The LIVE half
/// is `props_norm::assert_echo_rows_are_live`.
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

/// **Evidence integrity of the echo table**: every row's `cells` column is read
/// back against its evidence: `bins.tsv` for the frozen census, [`RP1_RECORDS`]
/// (or, for the two regcontrol pairs, the supplement's `# BIN` header lines with
/// cells summed from their data rows, [`supplement_bin_declarations`]) for the
/// supplement pairs. A mis-transcribed row fails here instead of silently
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
/// pin file is named by a row: the other half of the witness obligation.
///
/// `props_norm::tests::every_live_semantics_row_names_a_pin` pins the names *as
/// strings*. Nothing there can tell whether the tests behind them exist, so a
/// row could ship citing a witness that does not exist. This reads the pin file
/// and matches the names against its definitions, both ways: a renamed,
/// deleted or newly-orphaned pin fails here.
///
/// It insists on the **`#[test]` attribute**, not merely on the `fn`: a pin that
/// lost its attribute would still satisfy a name search while never running
/// again, which is precisely the silent-witness failure this guard exists to
/// prevent.
///
/// Five tables cite pins in the pin file, and the union of the citations must be
/// exactly what the file defines, so no list can be the place an un-cited
/// `#[test]` hides:
///
/// * the echo rows;
/// * [`LEDGER_ENTRY_PINS`]: they hold this engine's value for an r4133 ledger
///   entry, which no echo row *can* name;
/// * [`RP38_SUPERSEDED`]'s fourth column plus [`RP38_CAPTURE_PIN`]: they witness
///   a `SKIP_PROPS_CAPI_ONLY` row pair (no ledger entry at all);
/// * [`RP39_PINS`]'s second column: they hold this engine's value on the display
///   floor's round-trip residue, cells no exclusion row can state;
/// * [`RP312_UPSTREAM_BUG`]'s sixth column: its pin holds the REGULATED answer
///   on `autotrans.wdgcurrents`, which no echo row and no ledger entry states.
///
/// [`RP37_SUPERSEDED`] names pins too, but they sit beside the behaviour
/// (`exec::tests::controls`) rather than in [`PINS`], the
/// [`LANDED_PROPERTY_ENTRY_PINS`] shape, whose fourth column carries the file, so
/// they are read back by [`the_swtcontrol_pairs_are_superseded_by_the_per_phase_render`]
/// instead. This test speaks for one file.
#[test]
fn every_echo_row_pin_is_a_test_that_exists() {
    let path = repo_root().join(PINS);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    // Line endings are the checkout's, not this test's business.
    let text = text.replace("\r\n", "\n");

    let echo_named: BTreeSet<&str> = props_norm::PROPS_ECHO_R4133
        .iter()
        .filter_map(|r| r.witness.pin())
        .collect();
    let ledger_named: BTreeSet<&str> = LEDGER_ENTRY_PINS.iter().map(|(n, _, _)| *n).collect();
    let rp38_named: BTreeSet<&str> = RP38_SUPERSEDED
        .iter()
        .map(|(_, _, _, pin)| *pin)
        .chain(std::iter::once(RP38_CAPTURE_PIN))
        .collect();
    let rp39_named: BTreeSet<&str> = RP39_PINS.iter().map(|(_, pin, _)| *pin).collect();
    let rp312_named: BTreeSet<&str> = RP312_UPSTREAM_BUG
        .iter()
        .map(|(_, _, _, _, _, pin)| *pin)
        .collect();
    assert!(
        echo_named.is_disjoint(&ledger_named)
            && rp38_named.is_disjoint(&echo_named)
            && rp38_named.is_disjoint(&ledger_named)
            && rp39_named.is_disjoint(&echo_named)
            && rp39_named.is_disjoint(&ledger_named)
            && rp39_named.is_disjoint(&rp38_named)
            && rp312_named.is_disjoint(&echo_named)
            && rp312_named.is_disjoint(&ledger_named)
            && rp312_named.is_disjoint(&rp38_named)
            && rp312_named.is_disjoint(&rp39_named),
        "a pin witnesses an echo row, an r4133 ledger entry, a SilentReadOnly skip row, a \
         round-trip residue pair or the AutoTrans typecast bug, never two"
    );
    for (name, what) in echo_named
        .iter()
        .map(|n| (*n, "an echo row's witness"))
        .chain(
            ledger_named
                .iter()
                .map(|n| (*n, "a ledger entry's witness")),
        )
        .chain(
            rp38_named
                .iter()
                .map(|n| (*n, "a SilentReadOnly skip row's witness")),
        )
        .chain(
            rp39_named
                .iter()
                .map(|n| (*n, "a round-trip residue pair's witness")),
        )
        .chain(
            rp312_named
                .iter()
                .map(|n| (*n, "the AutoTrans typecast bug's witness")),
        )
    {
        assert!(
            text.contains(&format!("#[test]\nfn {name}() {{")),
            "{name} is named as {what} but {PINS} defines no such #[test]"
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
    let cited: BTreeSet<&str> = echo_named
        .union(&ledger_named)
        .copied()
        .chain(rp38_named.iter().copied())
        .chain(rp39_named.iter().copied())
        .chain(rp312_named.iter().copied())
        .collect();
    assert_eq!(
        defined, cited,
        "{PINS} must define exactly the pins the echo rows, the r4133 ledger entries, the \
         SilentReadOnly skip rows, the round-trip residue pairs and the AutoTrans typecast bug name"
    );
}

/// The census population as `(case, engines)`: every live (`abort=0`),
/// non-`large` case `population.lock.json` fingerprints, exactly the cases
/// `corpus_gate/scheduler.rs::force_properties` property-compares. The lock is
/// the four manifests' `engines`/`kind` per case, re-checked against them by
/// the anti-shrink guard, so reading it here is reading the manifests.
fn property_population(lock: &serde_json::Value) -> Vec<(String, String)> {
    fn section(prefix: &str, rows: &serde_json::Value, out: &mut Vec<(String, String)>) {
        let rows = rows.as_object().expect("a rigor section is an object");
        for (rel, rigor) in rows {
            let rigor = rigor.as_str().expect("a rigor row is a string");
            if rigor_field(rigor, "kind").starts_with("large") || rigor_field(rigor, "abort") != "0"
            {
                continue;
            }
            out.push((
                format!("{prefix}:{rel}"),
                rigor_field(rigor, "engines").to_string(),
            ));
        }
    }
    let mut out = Vec::new();
    section("solvable_now", &lock["solvable_now"], &mut out);
    let families = lock["family_rigor"]
        .as_object()
        .expect("family_rigor is an object");
    for (family, rows) in families {
        section(family, rows, &mut out);
    }
    out
}

/// The cases a ledger `skip` entry drops from `channel` — [`r4133_skipped_cases`]'
/// reading, for either channel.
fn ledger_skipped_cases(channel: &str) -> BTreeSet<String> {
    let path = repo_root().join(LEDGER);
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
    )
    .expect("ledger.json is JSON");
    doc["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array")
        .iter()
        .filter(|e| e["channel"] == channel && e["kind"] == "skip")
        .filter_map(|e| e["case"].as_str().map(str::to_string))
        .collect()
}

/// The class a `New` line declares, lower-cased — `New Line.l1 …`,
/// `New "Line.l1"`, `New object=Line.l1` — or `None` for any other line.
fn new_declaration_class(line: &str) -> Option<String> {
    let lower = line.trim().to_ascii_lowercase();
    let rest = lower.strip_prefix("new")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    let rest = rest
        .strip_prefix("object")
        .and_then(|r| r.trim_start().strip_prefix('='))
        .map_or(rest, str::trim_start)
        .trim_start_matches(['"', '\'']);
    let (class, _) = rest.split_once('.')?;
    (!class.is_empty() && class.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .then(|| class.to_string())
}

/// Every element class a case's deck declares with `New`, over the deck and
/// its exact `Redirect`/`Compile` closure ([`redirect_paths`]), with
/// `New Circuit.<name>` read as the `Vsource` it creates — plus every argument
/// the closure could not place, which the caller must see empty.
fn declared_classes(case: &str) -> (BTreeSet<String>, Vec<String>) {
    let root = repo_root().join(CORPUS);
    let deck = case_deck(case);
    let mut seen = BTreeSet::from([deck.clone()]);
    let mut queue = vec![deck];
    let (mut classes, mut unresolved) = (BTreeSet::new(), Vec::new());
    while let Some(file) = queue.pop() {
        let text = read_script(&root.join(&file));
        for line in text.lines() {
            if let Some(class) = new_declaration_class(line) {
                classes.insert(if class == "circuit" {
                    "vsource".to_string()
                } else {
                    class
                });
            }
        }
        let (resolved, lost) = redirect_paths(&text, &file, &root);
        unresolved.extend(lost.into_iter().map(|arg| format!("{file}: {arg}")));
        for path in resolved {
            if seen.insert(path.clone()) {
                queue.push(path);
            }
        }
    }
    (classes, unresolved)
}

/// Which channels really gate each census case, as case lists. The ledger may
/// `skip` either channel of a `both` case so long as one stays live
/// (`corpus_gate/ledger.rs`), and the scheduler then dispatches the other alone,
/// so the manifest flag is not the answer by itself:
///
/// * **capi-gating**: its capi channel runs;
/// * **r4133-only**: its r4133 channel runs and its capi channel does not:
///   `engines: "r4133"`, or a `both` case whose capi channel is skipped;
/// * **r4133-gating**: its r4133 channel runs, whatever capi does.
#[derive(Debug, Default, PartialEq)]
struct GatingBuckets {
    r4133_only: Vec<String>,
    capi_gating: Vec<String>,
    r4133_gating: Vec<String>,
}

/// [`GatingBuckets`] from `(case, engines)` rows and each channel's `skip`s.
fn gating_buckets(
    population: &[(String, String)],
    r4133_skipped: &BTreeSet<String>,
    capi_skipped: &BTreeSet<String>,
) -> GatingBuckets {
    let mut buckets = GatingBuckets::default();
    for (case, engines) in population {
        let (capi, r4133) = match engines.as_str() {
            "both" => (true, true),
            "capi_v0145" => (true, false),
            "r4133" => (false, true),
            other => panic!("{case}: engines={other} is none of the three legal values"),
        };
        let capi_runs = capi && !capi_skipped.contains(case);
        let r4133_runs = r4133 && !r4133_skipped.contains(case);
        if capi_runs {
            buckets.capi_gating.push(case.clone());
        } else if r4133_runs {
            buckets.r4133_only.push(case.clone());
        }
        if r4133_runs {
            buckets.r4133_gating.push(case.clone());
        }
    }
    buckets
}

/// The census population both manifest derivations read: every case's
/// declared classes ([`declared_classes`], asserted fully resolved) and its
/// [`GatingBuckets`] under the live ledger ([`ledger_skipped_cases`]).
fn gating_population() -> (BTreeMap<String, BTreeSet<String>>, GatingBuckets) {
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");
    let population = property_population(&lock);
    let mut classes = BTreeMap::new();
    for (case, _) in &population {
        let (declared, unresolved) = declared_classes(case);
        assert!(
            unresolved.is_empty(),
            "{case}: Redirect/Compile targets that resolve to no file ({unresolved:?}) — a \
             declaration behind them would be invisible to this derivation"
        );
        classes.insert(case.clone(), declared);
    }
    let buckets = gating_buckets(
        &population,
        &ledger_skipped_cases("r4133"),
        &ledger_skipped_cases("capi_v0145"),
    );
    (classes, buckets)
}

/// **[`gating_buckets`] follows the channels that really run**: a synthetic
/// population over every `engines` value, with and without each channel's
/// `skip`. `b` is a `both` case whose capi channel is skipped, so it is
/// r4133-only.
#[test]
fn the_gating_buckets_follow_the_channels_that_really_run() {
    let owned = |cases: &[&str]| -> Vec<String> { cases.iter().map(|c| c.to_string()).collect() };
    let population: Vec<(String, String)> = [
        ("a", "both"),
        ("b", "both"),
        ("c", "both"),
        ("d", "r4133"),
        ("e", "r4133"),
        ("f", "capi_v0145"),
        ("g", "capi_v0145"),
    ]
    .iter()
    .map(|(case, engines)| (case.to_string(), engines.to_string()))
    .collect();
    let r4133_skipped: BTreeSet<String> = owned(&["c", "e"]).into_iter().collect();
    let capi_skipped: BTreeSet<String> = owned(&["b", "g"]).into_iter().collect();
    assert_eq!(
        gating_buckets(&population, &r4133_skipped, &capi_skipped),
        GatingBuckets {
            r4133_only: owned(&["b", "d"]),
            capi_gating: owned(&["a", "c", "f"]),
            r4133_gating: owned(&["a", "b", "d"]),
        },
        "a case is capi-gating when its capi channel runs, r4133-only when only its r4133 \
         channel does (a capi-skipped `both` case included), r4133-gating whenever r4133 runs"
    );
}

/// **`props_norm::ECHO_ROWS_ON_R4133_ONLY_CASES` is complete by derivation.**
///
/// An echo row on an r4133-only case masks cells no channel compares, under
/// capi-only witnesses. Per-cell case evidence lives only in the local claims
/// census, so the derivation works one level up, on tracked evidence only: the
/// census population and each case's `engines` ([`property_population`], the
/// manifests' fingerprint), the ledger's channel `skip`s
/// ([`ledger_skipped_cases`], combined by [`gating_buckets`]) and the classes
/// each case's deck declares ([`declared_classes`]). A row can only mask a cell
/// on an r4133-only case whose deck declares its class, so every echo row whose
/// class such a case declares must sit in exactly one of the two lists:
/// measured exposed (it names a pin), or measured unexposed with its class count
/// pinned (`props_norm::ECHO_ROWS_UNEXPOSED_ON_R4133_ONLY_CASES`, whose doc says
/// what "covered" means for the narrowed and carved-out rows). A row whose class
/// no r4133-only case declares needs neither, by derivation. That makes the
/// lists complete against population moves (a manifest flip, a ledger `skip`, a
/// deck gaining or losing a class), not against a deck edit that makes a
/// covered cell differ while every class set stays put: a re-census finds that
/// (the limit `props_norm::ECHO_ROWS_UNEXPOSED_ON_R4133_ONLY_CASES` spells out).
///
/// Two more readings of the same sets: a listed exposure cannot sit on more
/// cases than declare its class, and a `Capi(n)` witness needs a capi-gating
/// case that declares the class at all.
#[test]
fn the_r4133_only_exposure_list_is_derived_from_the_corpus() {
    let (classes, buckets) = gating_population();
    let (r4133_only, capi_gating) = (&buckets.r4133_only, &buckets.capi_gating);
    assert!(
        r4133_only.len() >= 80 && capi_gating.len() >= 300,
        "the population read back as {} r4133-only and {} capi-gating case(s) — the lock \
         reader went vacuous",
        r4133_only.len(),
        capi_gating.len()
    );

    let declaring = |cases: &[String], class: &str| -> Vec<String> {
        cases
            .iter()
            .filter(|case| classes[case.as_str()].contains(class))
            .cloned()
            .collect()
    };
    let mut problems = Vec::new();
    for row in props_norm::PROPS_ECHO_R4133 {
        let (class, prop) = (row.class, row.prop);
        let on_r4133 = declaring(r4133_only, class);
        let n = on_r4133.len();
        match (
            props_norm::r4133_only_exposure(class, prop),
            props_norm::unexposed_on_r4133_only_cases(class, prop),
        ) {
            (Some(_), Some(_)) => {
                problems.push(format!("{class}.{prop}: listed exposed AND unexposed"));
            }
            (Some((cells, cases)), None) if n == 0 || cases as usize > n => {
                problems.push(format!(
                    "{class}.{prop}: listed as {cells} cell(s) on {cases} r4133-only case(s), \
                     but only {n} r4133-only census case(s) declare a {class}"
                ));
            }
            (None, Some(pinned)) if pinned as usize != n => {
                problems.push(format!(
                    "{class}.{prop}: {n} r4133-only census case(s) declare a {class}, the \
                     unexposed list pins {pinned} — the population moved; re-measure the row \
                     there (DSS_PROPS_CENSUS=claims) and re-count it, or move it to \
                     ECHO_ROWS_ON_R4133_ONLY_CASES with a pin"
                ));
            }
            (None, None) if n > 0 => {
                problems.push(format!(
                    "{class}.{prop}: {n} r4133-only census case(s) declare a {class} (first: \
                     {}), where the capi channel never runs, and the row is in neither \
                     ECHO_ROWS_ON_R4133_ONLY_CASES nor ECHO_ROWS_UNEXPOSED_ON_R4133_ONLY_CASES — \
                     measure its cells there and list it (an exposed row owes a pin)",
                    on_r4133[0]
                ));
            }
            _ => {}
        }
        if let props_norm::EchoWitness::Capi(w) | props_norm::EchoWitness::CapiAndPin(w, _) =
            row.witness
            && declaring(capi_gating, class).is_empty()
        {
            problems.push(format!(
                "{class}.{prop}: a capi witness ({w} case(s)) on a class no capi-gating \
                 census case declares — the capi channel compares none of its cells"
            ));
        }
    }
    for (class, prop) in props_norm::echo_rows_with_no_in_scope_cell() {
        if props_norm::unexposed_on_r4133_only_cases(class, prop).is_none() {
            problems.push(format!(
                "{class}.{prop}: no in-scope cell at all implies none on an r4133-only case, \
                 so the dormant row belongs in ECHO_ROWS_UNEXPOSED_ON_R4133_ONLY_CASES"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "the r4133-only exposure lists no longer match the corpus:\n{}",
        problems.join("\n")
    );
}

/// **`props_norm::NORM_ROWS_WITH_NO_IN_SCOPE_CELL` follows the manifests**: the
/// live half of
/// [`the_norm_rows_with_no_in_scope_cell_are_exactly_the_declared_ones`], which
/// pins the list against the frozen census only.
///
/// `props_norm::normalize_r4133` counts a visit on every compared cell of a
/// row's pair, claimed or not, so a norm row is visited at all exactly when an
/// r4133-gating case declares its class ([`GatingBuckets`]). Both ways: a row
/// whose class no r4133-gating case declares must be listed (a manifest flip or
/// an r4133 `skip` that takes a class's last r4133-gating case away would
/// otherwise leave a never-visited row while `bins.tsv` still shows the old
/// in-scope cells), and a listed row whose class an r4133-gating case declares
/// is a stale declaration (re-measure, shrink the list, re-freeze the census
/// rows the frozen pin reads).
#[test]
fn the_norm_rows_with_no_in_scope_cell_are_the_classes_no_r4133_gating_case_declares() {
    let (classes, buckets) = gating_population();
    assert!(
        buckets.r4133_gating.len() >= 300,
        "the population read back as {} r4133-gating case(s) — the lock reader went vacuous",
        buckets.r4133_gating.len()
    );
    let mut problems = Vec::new();
    for row in props_norm::PROPS_NORM_R4133 {
        let class = row.class.to_ascii_lowercase();
        let listed = props_norm::NORM_ROWS_WITH_NO_IN_SCOPE_CELL
            .iter()
            .any(|(c, p)| c.eq_ignore_ascii_case(row.class) && p.eq_ignore_ascii_case(row.prop));
        let declaring: Vec<&String> = buckets
            .r4133_gating
            .iter()
            .filter(|case| classes[case.as_str()].contains(&class))
            .collect();
        match (declaring.first(), listed) {
            (None, false) => problems.push(format!(
                "{}.{}: no r4133-gating census case declares a {class}, so the gate never visits \
                 the row and its liveness guard can never fire — list it in \
                 NORM_ROWS_WITH_NO_IN_SCOPE_CELL (or gate a deck declaring the class on r4133)",
                row.class, row.prop
            )),
            (Some(first), true) => problems.push(format!(
                "{}.{}: listed in NORM_ROWS_WITH_NO_IN_SCOPE_CELL, but {} r4133-gating census \
                 case(s) declare a {class} (first: {first}) — the gate visits the row now; \
                 re-measure it (DSS_PROPS_CENSUS=claims), drop it from the list and re-freeze \
                 the census rows the frozen pin reads",
                row.class,
                row.prop,
                declaring.len()
            )),
            _ => {}
        }
    }
    assert!(
        problems.is_empty(),
        "NORM_ROWS_WITH_NO_IN_SCOPE_CELL no longer matches the manifests:\n{}",
        problems.join("\n")
    );
}

/// **The landed `capi_v0145` property entries and the engine pins that witness
/// them**: `(entry id, sub-step, pin `#[test]`, the file that defines it)`.
///
/// [`LEDGER_ENTRY_PINS`] covers the r4133 entries, whose pins live in [`PINS`].
/// A `FIX` whose fix reds the LIVE capi property compare has the opposite shape:
/// the entry lands with the fix, and its witness belongs beside the behaviour,
/// in the engine's own test modules. The gate holds the *entry*, and the entry
/// pins both sides of the divergence, but only the pin asserts that this
/// engine's value is *right* rather than merely stable.
///
/// This table is that tie, and it is checked **both ways** by
/// [`every_landed_property_entry_has_a_witness_pin_that_exists`]: every row must
/// name a real landed entry and a real `#[test]`, and every landed entry of this
/// kind must appear here, so a capi property exclusion cannot land without a
/// witness.
const LANDED_PROPERTY_ENTRY_PINS: &[(&str, &str, &str, &str)] = &[
    (
        "reduce-merge-units-restored-midi-capi-props",
        "line-units",
        "merged_matrix_line_keeps_the_surviving_lines_length_units",
        "crates/dss-core/src/exec/tests/reduce.rs",
    ),
    (
        "line-switch-keeps-linecode-zone2-capi-props",
        "line-linecode",
        "switch_yes_keeps_the_linecode_and_its_units_conversion",
        "crates/dss-core/src/exec/tests/line_fetch.rs",
    ),
    (
        "line-switch-keeps-linecode-zone3-capi-props",
        "line-linecode",
        "switch_yes_keeps_the_linecode_and_its_units_conversion",
        "crates/dss-core/src/exec/tests/line_fetch.rs",
    ),
    (
        "swtcontrol-per-phase-state-lock-capi-props",
        "swtcontrol-per-phase",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol-per-phase-state-makeposseq-capi-props",
        "swtcontrol-per-phase",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol-per-phase-state-ieee519-tmode-capi-props",
        "swtcontrol-per-phase",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol-per-phase-state-ieee519-varload-capi-props",
        "swtcontrol-per-phase",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol-per-phase-state-ieee519-matlab-capi-props",
        "swtcontrol-per-phase",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
];

/// The capi `property` divergence entries of [`LEDGER`] that no census pair
/// lands: each was measured outside the property census, so it owes no row of
/// [`LANDED_PROPERTY_ENTRY_PINS`].
const PROPERTY_ENTRIES_OUTSIDE_THE_CENSUS: &[&str] = &[
    "capi-generator-makeposseq-rating",
    "capi-generator-makeposseq-rating-repeat",
    "capi-geometry-weakest-phase-lfc",
    "capi-linespacing-normamps",
    "capi-storage-charging-efficiency-once-peakshavelow",
    "makeposseq-cuf-applied-capi-props",
    "makeposseq-matrix-average-capi-props",
    "makeposseq-repeat-capi-props",
];

/// Every capi `property` divergence entry of the ledger is either a landed
/// census exclusion with a witness pin that exists or a named entry measured
/// outside the census, so a capi property exclusion cannot land unclassified.
#[test]
fn every_landed_property_entry_has_a_witness_pin_that_exists() {
    let path = repo_root().join(LEDGER);
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())),
    )
    .expect("ledger.json is JSON");
    let landed: BTreeSet<String> = doc["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array")
        .iter()
        .filter(|e| {
            e["channel"] == "capi_v0145"
                && e["kind"] == "divergence"
                && e["match"]
                    .as_array()
                    .is_some_and(|m| m.iter().any(|x| x["field"] == "property"))
        })
        .map(|e| {
            e["id"]
                .as_str()
                .unwrap_or_else(|| panic!("a ledger entry names its id: {e}"))
                .to_string()
        })
        .collect();
    let tabled: BTreeSet<String> = LANDED_PROPERTY_ENTRY_PINS
        .iter()
        .map(|(id, ..)| (*id).to_string())
        .collect();
    let outside: BTreeSet<String> = PROPERTY_ENTRIES_OUTSIDE_THE_CENSUS
        .iter()
        .map(|id| (*id).to_string())
        .collect();
    assert!(
        tabled.is_disjoint(&outside),
        "an entry is both a census exclusion and outside the census: {:?}",
        tabled.intersection(&outside).collect::<Vec<_>>()
    );
    assert_eq!(
        landed,
        tabled
            .union(&outside)
            .cloned()
            .collect::<BTreeSet<String>>(),
        "every capi_v0145 property exclusion owes a witness pin in LANDED_PROPERTY_ENTRY_PINS \
         or a row of PROPERTY_ENTRIES_OUTSIDE_THE_CENSUS (and every row must name a real entry)"
    );

    for (id, step, pin, file) in LANDED_PROPERTY_ENTRY_PINS {
        let p = repo_root().join(file);
        let text = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("{id} ({step}): read {}: {e}", p.display()))
            .replace("\r\n", "\n");
        assert!(
            text.contains(&format!("#[test]\nfn {pin}(")),
            "{id} ({step}) names the witness {pin}, but {file} defines no such #[test]"
        );
    }
}

/// **The pins that witness a `ledger.json` entry rather than an echo row**:
/// `(pin, sub-step, the entry it holds this engine's value for)`.
///
/// Three root-cause pairs are excluded by per-case `ledger.json` `property`
/// divergence entries instead of `PROPS_ECHO_R4133` rows, because r4133's getter
/// is LIVE, so calling the divergence an echo would be a false statement about
/// the mechanism: `swtcontrol.delay` (r4133 never wires the property),
/// `windgen.kvar` (the getter reads the dispatched Q where the property
/// documents the base kvar) and `gictransformer.r2` (a correct read of a field
/// r4133 mis-derives). The ledger entry names both numbers, and the pin asserts
/// this engine's value without an OpenDSS engine, in both lanes.
///
/// Listed here, and not left to a naming convention, for the same reason
/// [`NOT_A_PIN`] is: this table is the *only* thing that lets a `#[test]` live
/// in the pin file without an echo row naming it, so growing it is a decision.
/// It is pinned literally by [`the_ledger_entry_pin_list_is_pinned`] and tied to
/// the pair's routing row by
/// [`the_bin7_root_cause_pairs_are_routed_to_their_sub_steps`].
const LEDGER_ENTRY_PINS: &[(&str, &str, &str)] = &[
    (
        "swtcontrol_delay_wires_the_property",
        "swtcontrol-delay",
        "r4133-swtcontrol-delay-ignored-time (controls:swtcontrol/swtcontrol_time.dss)",
    ),
    (
        "swtcontrol_delay_wires_the_property_on_the_midi_tie",
        "swtcontrol-delay",
        "r4133-swtcontrol-delay-ignored-midi (controls:swtcontrol/midi_swtcontrol.dss)",
    ),
    (
        "windgen_kvar_renders_the_base_on_the_daily_deck",
        "windgen-kvar",
        "r4133-windgen-kvar-dispatched-daily (modes:windgen/windgen_daily.dss)",
    ),
    (
        "windgen_kvar_renders_the_base_on_the_delta_snapshot",
        "windgen-kvar",
        "r4133-windgen-kvar-dispatched-delta (modes:windgen/windgen_snap_delta.dss)",
    ),
    (
        "windgen_kvar_renders_the_base_on_the_dynamics_deck",
        "windgen-kvar",
        "r4133-windgen-kvar-dispatched-dyn (modes:windgen/windgen_dyn.dss)",
    ),
    (
        "windgen_kvar_renders_the_base_on_the_fault_ride_through_deck",
        "windgen-kvar",
        "r4133-windgen-kvar-dispatched-dynfault (modes:windgen/windgen_dyn_fault.dss)",
    ),
    (
        "gictransformer_r2_honours_the_x_winding_percentage",
        "gictransformer-r2",
        "gic-pct-r2-honoured-gictransformer-r4133-props \
         (asymmetric:gic/gictransformer_gic.dss)",
    ),
    (
        "gictransformer_r2_honours_the_x_winding_percentage_on_the_ring",
        "gictransformer-r2",
        "gic-pct-r2-honoured-midi-r4133-props (asymmetric:gic/gic_midi.dss)",
    ),
];

/// **The ledger-entry pin list is pinned literally**, like [`NOT_A_PIN`]: it is
/// the second exemption from "every pin in the file is named by an echo row",
/// and an exemption that grows by iteration would let an un-cited `#[test]`
/// through by simply being added to it.
///
/// Each row must name a real entry id and the case it was landed as, so the
/// citation stays checkable against `tests/corpus/ledger.json`, which
/// [`the_r4133_property_entries_are_the_root_cause_eight`] reads back through this
/// very column.
#[test]
fn the_ledger_entry_pin_list_is_pinned() {
    assert_eq!(
        LEDGER_ENTRY_PINS,
        [
            (
                "swtcontrol_delay_wires_the_property",
                "swtcontrol-delay",
                "r4133-swtcontrol-delay-ignored-time (controls:swtcontrol/swtcontrol_time.dss)",
            ),
            (
                "swtcontrol_delay_wires_the_property_on_the_midi_tie",
                "swtcontrol-delay",
                "r4133-swtcontrol-delay-ignored-midi (controls:swtcontrol/midi_swtcontrol.dss)",
            ),
            (
                "windgen_kvar_renders_the_base_on_the_daily_deck",
                "windgen-kvar",
                "r4133-windgen-kvar-dispatched-daily (modes:windgen/windgen_daily.dss)",
            ),
            (
                "windgen_kvar_renders_the_base_on_the_delta_snapshot",
                "windgen-kvar",
                "r4133-windgen-kvar-dispatched-delta (modes:windgen/windgen_snap_delta.dss)",
            ),
            (
                "windgen_kvar_renders_the_base_on_the_dynamics_deck",
                "windgen-kvar",
                "r4133-windgen-kvar-dispatched-dyn (modes:windgen/windgen_dyn.dss)",
            ),
            (
                "windgen_kvar_renders_the_base_on_the_fault_ride_through_deck",
                "windgen-kvar",
                "r4133-windgen-kvar-dispatched-dynfault (modes:windgen/windgen_dyn_fault.dss)",
            ),
            (
                "gictransformer_r2_honours_the_x_winding_percentage",
                "gictransformer-r2",
                "gic-pct-r2-honoured-gictransformer-r4133-props \
                 (asymmetric:gic/gictransformer_gic.dss)",
            ),
            (
                "gictransformer_r2_honours_the_x_winding_percentage_on_the_ring",
                "gictransformer-r2",
                "gic-pct-r2-honoured-midi-r4133-props (asymmetric:gic/gic_midi.dss)",
            ),
        ],
        "the two swtcontrol.delay entries, the four windgen.kvar and the two gictransformer.r2, \
         and nothing else"
    );
    for (pin, step, entry) in LEDGER_ENTRY_PINS {
        assert!(
            RP3_ROUTING.iter().any(|(_, s, _, _, _)| s == step),
            "{pin}: {step} owns no RP3 routing row"
        );
        assert!(
            entry.contains('(') && entry.contains(':'),
            "{pin}: the citation must name the drafted entry id and its case, got {entry:?}"
        );
    }
}

/// **The round-trip residue's expected-value pins**: `(pair, pin `#[test]`,
/// verdict)`, one row per settled pair of the residue [`RP39_ROUTING`]
/// declares.
///
/// These pairs are neither echo rows (both engines' getters are LIVE, r4133's
/// number is computed, not echoed back from a parse) nor ledger entries: no
/// in-scope cell exists on any of them, so a `property` entry would have nothing
/// to exclude. What each pair owes is an expected-value test that names BOTH
/// numbers and reproduces r4133's from ours through the cited round trip. This
/// is where those pins are cited, so [`every_echo_row_pin_is_a_test_that_exists`]
/// holds them to the same both-ways rule as the other sets: an un-cited
/// `#[test]` in [`PINS`] fails, and a pair naming a pin that does not exist
/// fails too.
///
/// Only the two outcomes whose obligation *is* a pin and nothing else may appear
/// here ([`RP39_SETTLED_VERDICTS`]): a `PORT_BUG` is fixed in both lanes and
/// witnessed beside the behaviour, an `UPSTREAM_BUG` earns a ledger entry and
/// moves to [`LEDGER_ENTRY_PINS`]. The counted columns of [`RP39_ROUTING`] do
/// **not** move when a pair settles: the vendored spellings are a data lock the
/// walk still measures, and the settlement shows up in [`OPEN_RP39`] instead.
///
/// Every [`RP39_ROUTING`] pair is `PIN` and named here, and
/// [`the_rp39_pin_list_is_pinned`] checks the completeness half: a `PIN`
/// disposition with no test, or a pin on a pair the routing disposes of some
/// other way, is a hard error.
const RP39_PINS: &[(&str, &str, &str)] = &[
    (
        "autotrans.wdgcurrents",
        "autotrans_wdgcurrents_after_makeposseq_solve_the_exactly_converted_circuit",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "capacitor.cuf",
        "capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "capacitor.emergamps",
        "capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "capacitor.normamps",
        "capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "generator.kva",
        "generator_ratings_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "generator.kvar",
        "generator_ratings_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "generator.maxkvar",
        "generator_ratings_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "generator.minkvar",
        "generator_ratings_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "line.b0",
        "line_b1_and_b0_after_makeposseq_use_the_full_precision_c1",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "line.b1",
        "line_b1_and_b0_after_makeposseq_use_the_full_precision_c1",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "load.kva",
        "load_kva_after_makeposseq_is_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "load.kvar",
        "load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "load.kw",
        "load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "load.xfkva",
        "load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "reactor.emergamps",
        "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "reactor.lmh",
        "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "reactor.normamps",
        "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "reactor.x",
        "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "reactor.z",
        "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "transformer.emergamps",
        "transformer_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "transformer.normamps",
        "transformer_amps_after_makeposseq_are_the_exact_typed_conversion",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "vsource.isc3",
        "vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "vsource.mvasc1",
        "vsource_mvasc1_and_mvasc3_after_makeposseq_use_the_full_precision_basekv",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "vsource.mvasc3",
        "vsource_mvasc1_and_mvasc3_after_makeposseq_use_the_full_precision_basekv",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "vsource.puz0",
        "vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "vsource.puz1",
        "vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv",
        "PRECISION_ROUNDTRIP",
    ),
    (
        "vsource.puz2",
        "vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv",
        "PRECISION_ROUNDTRIP",
    ),
];

/// The verdicts an [`RP39_PINS`] row may carry: the two outcomes whose whole
/// obligation is an expected-value pin. Either this engine is exact and r4133's
/// number is a reproduced round trip (`PRECISION_ROUNDTRIP`), or the two engines'
/// state differs by a proven precision-class cause (`STATE_DIFFERS`). The other
/// outcomes owe something else and live elsewhere. An unknown tag is a hard
/// error naming this list, never a reason to loosen it.
const RP39_SETTLED_VERDICTS: &[&str] = &["PRECISION_ROUNDTRIP", "STATE_DIFFERS"];

/// **The AutoTrans typecast bug's routing and its witness**: `(pair, example
/// rows, rows on in-scope pairs, r4133 site, verdict, pin `#[test]`)`, the fifth
/// citing set [`every_echo_row_pin_is_a_test_that_exists`] reads.
///
/// One pair, one verdict, `UPSTREAM_BUG`: r4133's `RegControl` reads its
/// controlled AutoTrans as a two-winding transformer, so its tap increment reads
/// the wrong field and rounds every realistic boost to zero. r4133 logs no
/// control event on the four decks and leaves the regulated bus outside its band:
/// the spellings below are its UNREGULATED circuit. This engine keeps the
/// regulated answer in both lanes, as OpenDSS bugs are never reproduced, and an
/// OpenDSS report is filed.
///
/// **Why the witness is a pin and not a [`LEDGER_ENTRY_PINS`] row.** The
/// divergence is whole-case, not per-field: voltages, the assembled Y, every
/// element on the AutoTrans branch, the meters, the event log, the control queue
/// and all three manifest probes. The instrument is a case-level `kind: "skip"`
/// on the `r4133` channel, one per deck, and the four cases are
/// `engines: "capi_v0145"`, so there is no r4133 channel to skip. A `property`
/// entry, which is what [`LEDGER_ENTRY_PINS`] cites, would be the wrong shape.
///
/// The counted columns behave exactly like [`RP39_ROUTING`]'s: they are
/// re-measured from the walk by
/// [`the_regcontrol_autotrans_typecast_rows_have_their_own_owner`], and a pin does
/// not make a link claim a row.
const RP312_UPSTREAM_BUG: &[(&str, usize, usize, &str, &str, &str)] = &[(
    "autotrans.wdgcurrents",
    8,
    0,
    "RegControl.pas:1026/:1296/:1479 `TTransfObj(ControlledElement)` over \
     AutoTrans.pas:88 `TAutoTransObj = class(TPDElement)`; the zeroed increment at \
     RegControl.pas:1249-1250",
    "UPSTREAM_BUG",
    "autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans",
)];

/// The verdicts an [`RP312_UPSTREAM_BUG`] row may carry. One, deliberately: the
/// divergence is an OpenDSS bug this engine does not reproduce. Anything else (a
/// precision round trip, a bug of this engine) owes a different obligation and
/// belongs with that outcome's own table, so an unknown tag is a hard error
/// naming this list rather than a reason to widen it.
const RP312_VERDICTS: &[&str] = &["UPSTREAM_BUG"];

/// **The round-trip residue's pin list is pinned literally**, like [`NOT_A_PIN`]
/// and [`LEDGER_ENTRY_PINS`]: it is an exemption from "every pin in [`PINS`] is
/// named by an echo row", and an exemption that grows by iteration would let an
/// un-cited `#[test]` through by being added to it.
///
/// Each row must also name a pair the residue actually holds and an outcome a pin
/// alone can settle, so a routing verdict cannot be recorded here that owes a
/// ledger entry or a fix.
#[test]
fn the_rp39_pin_list_is_pinned() {
    assert_eq!(
        RP39_PINS,
        [
            (
                "autotrans.wdgcurrents",
                "autotrans_wdgcurrents_after_makeposseq_solve_the_exactly_converted_circuit",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "capacitor.cuf",
                "capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "capacitor.emergamps",
                "capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "capacitor.normamps",
                "capacitor_cuf_and_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "generator.kva",
                "generator_ratings_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "generator.kvar",
                "generator_ratings_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "generator.maxkvar",
                "generator_ratings_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "generator.minkvar",
                "generator_ratings_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "line.b0",
                "line_b1_and_b0_after_makeposseq_use_the_full_precision_c1",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "line.b1",
                "line_b1_and_b0_after_makeposseq_use_the_full_precision_c1",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "load.kva",
                "load_kva_after_makeposseq_is_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "load.kvar",
                "load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "load.kw",
                "load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "load.xfkva",
                "load_kw_kvar_and_xfkva_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "reactor.emergamps",
                "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "reactor.lmh",
                "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "reactor.normamps",
                "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "reactor.x",
                "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "reactor.z",
                "reactor_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "transformer.emergamps",
                "transformer_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "transformer.normamps",
                "transformer_amps_after_makeposseq_are_the_exact_typed_conversion",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "vsource.isc3",
                "vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "vsource.mvasc1",
                "vsource_mvasc1_and_mvasc3_after_makeposseq_use_the_full_precision_basekv",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "vsource.mvasc3",
                "vsource_mvasc1_and_mvasc3_after_makeposseq_use_the_full_precision_basekv",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "vsource.puz0",
                "vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "vsource.puz1",
                "vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv",
                "PRECISION_ROUNDTRIP",
            ),
            (
                "vsource.puz2",
                "vsource_isc3_and_puz_after_makeposseq_use_the_full_precision_basekv",
                "PRECISION_ROUNDTRIP",
            ),
        ],
        "every pair of the round-trip residue — chains A (load.*), B (vsource.*), \
         C (line.b*, autotrans.wdgcurrents), D (reactor.*, capacitor.*) and E \
         (generator.*, transformer.*) — and nothing else"
    );
    for (pair, pin, verdict) in RP39_PINS {
        let (_, _, _, _, disposition) = RP39_ROUTING
            .iter()
            .find(|(p, _, _, _, _)| p == pair)
            .unwrap_or_else(|| {
                panic!("{pin}: {pair} is not one of the round-trip residue's pairs")
            });
        assert_eq!(
            *disposition, "PIN",
            "{pair}: the routing disposes of it as {disposition}, which does not settle \
             through a pin — {pin} belongs with that outcome's own witness"
        );
        assert!(
            RP39_SETTLED_VERDICTS.contains(verdict),
            "{pair}: {verdict:?} is not an outcome an expected-value pin alone settles — \
             extend RP39_SETTLED_VERDICTS with the obligations it owes, never loosen"
        );
    }
    // …and the completeness half: a `PIN` disposition with no test would be a
    // verdict recorded against nothing.
    for (pair, rows, _, _, disposition) in RP39_ROUTING {
        let named = RP39_PINS.iter().any(|(p, _, _)| p == pair);
        if *disposition == "PIN" {
            assert!(
                named,
                "{pair}: settled as PIN, but no RP39_PINS row names its expected-value test \
                 ({rows} vendored rows)"
            );
        } else {
            assert!(
                !named,
                "{pair}: disposed of as {disposition}, so it must not be settled here"
            );
        }
    }
}

/// The `#[test]`s in the pin file that are **not** witnesses: the deck guard's
/// own two self-tests. Named, never pattern-matched — a naming convention would
/// let a third un-cited test slip past
/// [`every_echo_row_pin_is_a_test_that_exists`].
const NOT_A_PIN: &[&str] = &[
    "the_deck_guard_really_sweeps",
    "overlapping_deck_guards_still_sweep",
];

/// **The non-pin exemption list is pinned literally**: [`NOT_A_PIN`] switches off
/// the only guard that ties a witness name to a real, running test, so growing
/// it is a decision. Iterating whatever the const holds would let a bogus name
/// through, and with it an un-cited `#[test]`.
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

/// **The five `SilentReadOnly` pairs are superseded by the live render**, in both
/// directions.
///
/// Four things, none of them transcription:
///
/// * the table is exactly those five pairs, each still carrying the r4133
///   live-getter citation, and none of them takes an echo row (r4133's arm is
///   live, so an `EchoRow` would misname the mechanism);
/// * the superseded bucket holds their rows and **only** theirs, at
///   [`SUPERSEDED_RP38`], while [`Owner::Rp38`]'s declared bucket is empty;
/// * **the shipped disposition is asked of the harness, not described**: each
///   pair must be value-skipped on `capi_v0145` and compared on `r4133`
///   (`harness::skip_prop`, the `SKIP_PROPS` + `SKIP_PROPS_CAPI_ONLY` row pair).
///   This is what makes the supersession falsifiable: revert the engine to `''`
///   and the capi compare AGREES again, so the gate stays green, but then the
///   skip rows are dead, and whoever removes them reds here;
/// * each pair names an expected-value pin that
///   [`every_echo_row_pin_is_a_test_that_exists`] proves exists, the holder of
///   this engine's own value for the masked cells.
#[test]
fn the_rp38_pairs_are_superseded_by_the_live_render() {
    assert_eq!(
        RP38_SUPERSEDED
            .iter()
            .map(|(p, _, _, _)| *p)
            .collect::<Vec<_>>(),
        [
            "indmach012.pf",
            "storagecontroller.kwhtotal",
            "storagecontroller.kwtotal",
            "storagecontroller.kwhactual",
            "storagecontroller.kwactual",
        ],
        "the five SilentReadOnly pairs"
    );
    for (pair, cite, disposition, pin) in RP38_SUPERSEDED {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        assert!(
            !props_norm::has_echo_row(class, prop),
            "{pair} must NOT have an echo row — r4133's getter arm is live"
        );
        assert!(
            cite.contains(".pas:"),
            "{pair}: the row must cite the r4133 live getter, got {cite:?}"
        );
        assert!(
            !disposition.is_empty() && !pin.is_empty(),
            "{pair}: a superseded row owes its measured live disposition and a pin"
        );
        // The shipped skip disposition, read off the harness the gate uses.
        assert!(
            harness::skip_prop(class, prop, harness::PropsChannel::CapiV0145),
            "{pair}: the 0.14.5 capture renders '' — the capi value compare must be skipped"
        );
        assert!(
            !harness::skip_prop(class, prop, harness::PropsChannel::R4133),
            "{pair}: r4133 shares the render — that channel must keep comparing it"
        );
    }

    let corpus = Corpus::load();
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(led.owner(Owner::Rp38), DECLARED_RP38);
    assert!(
        led.declared.get(&Owner::Rp38).is_none_or(|b| b.rows == 0),
        "the SilentReadOnly declared bucket must be empty"
    );
    assert_eq!(led.superseded("silent-read-only"), SUPERSEDED_RP38);
    assert_eq!(
        led.superseded["silent-read-only"]
            .pairs
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "indmach012.pf",
            "storagecontroller.kwactual",
            "storagecontroller.kwhactual",
            "storagecontroller.kwhtotal",
            "storagecontroller.kwtotal",
        ],
        "the superseded bucket holds exactly the five pairs"
    );
}

/// **The two `swtcontrol` pairs are superseded by the per-phase render**, in both
/// directions.
///
/// Four things, and none of them a transcription of [`RP37_SUPERSEDED`]:
///
/// * the table is exactly those two pairs, each citing the r4133 getter arm it
///   renders from, and neither takes an echo row (r4133's arm is live, so an
///   `EchoRow` would misname the mechanism) **or a normalization row**;
/// * the superseded bucket holds their rows and only theirs, at
///   [`SUPERSEDED_RP37`], while [`Owner::Rp35`]'s declared bucket keeps exactly
///   the four other pairs ([`DECLARED_RP35`]);
/// * **the shipped disposition is asked of the harness, not described**: both
///   pairs must be COMPARED on both channels (`harness::skip_prop` false), which
///   is what makes the supersession falsifiable: revert the engine to the scalar
///   render and the r4133 property compare goes red with no row and no entry to
///   absorb it;
/// * each pair names an expected-value pin, in the file that defines it, and the
///   `#[test]` is read back out of that file. These pins sit beside the
///   behaviour rather than in [`PINS`], the [`LANDED_PROPERTY_ENTRY_PINS`]
///   shape, so [`every_echo_row_pin_is_a_test_that_exists`] does not see them.
#[test]
fn the_swtcontrol_pairs_are_superseded_by_the_per_phase_render() {
    assert_eq!(
        RP37_SUPERSEDED.iter().map(|(p, ..)| *p).collect::<Vec<_>>(),
        ["swtcontrol.normal", "swtcontrol.state"],
        "the two per-phase SwtControl pairs"
    );
    for (pair, cite, disposition, pin, file) in RP37_SUPERSEDED {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        assert!(
            !props_norm::has_echo_row(class, prop),
            "{pair} must NOT have an echo row — r4133's getter arm is live"
        );
        assert!(
            !PROPS_NORM_R4133
                .iter()
                .any(|r| r.class.eq_ignore_ascii_case(class) && r.prop.eq_ignore_ascii_case(prop)),
            "{pair} carries no ArrayForm row: it folds nothing; a row here \
             would claim the frozen spelling again and empty this table"
        );
        assert!(
            cite.contains(".pas:"),
            "{pair}: the row must cite the r4133 live getter, got {cite:?}"
        );
        assert!(
            !disposition.is_empty() && !pin.is_empty(),
            "{pair}: a superseded row owes its measured live disposition and a pin"
        );
        // Both channels COMPARE these pairs: the capi exclusion is five per-case
        // `ledger.json` entries, never a class-wide skip.
        for channel in [
            harness::PropsChannel::R4133,
            harness::PropsChannel::CapiV0145,
        ] {
            assert!(
                !harness::skip_prop(class, prop, channel),
                "{pair}: {channel:?} must keep comparing it — the exclusion is per case"
            );
        }
        let path = repo_root().join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{pair}: read {}: {e}", path.display()))
            .replace("\r\n", "\n");
        assert!(
            text.contains(&format!("#[test]\nfn {pin}(")),
            "{pair} names the witness {pin}, but {file} defines no such #[test]"
        );
    }

    let corpus = Corpus::load();
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(led.superseded("swtcontrol-per-phase"), SUPERSEDED_RP37);
    assert_eq!(
        led.superseded["swtcontrol-per-phase"]
            .pairs
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["swtcontrol.normal", "swtcontrol.state"],
        "the per-phase SwtControl bucket holds exactly the two pairs"
    );
    assert_eq!(led.owner(Owner::Rp35), DECLARED_RP35);
    let still_declared: BTreeSet<&str> = led.declared[&Owner::Rp35]
        .pairs
        .iter()
        .map(String::as_str)
        .collect();
    assert_eq!(
        still_declared.iter().copied().collect::<Vec<_>>(),
        ["line.linecode", "line.units", "relay.normal", "relay.state"],
        "the line.units, line.linecode and relay pairs keep their declared rows — only the \
         two per-phase SwtControl pairs have a measured live disposition"
    );
}

/// **A pair belongs to at most one superseded table**, and `superseded_tag`
/// answers for exactly the pairs the two tables name.
///
/// Without this the interception in [`account`] would silently prefer whichever
/// table it tests first, and a pair could be moved between the two locks
/// without either lock moving.
#[test]
fn the_two_superseded_tables_are_disjoint() {
    let rp37: BTreeSet<&str> = RP37_SUPERSEDED.iter().map(|(p, ..)| *p).collect();
    let rp38: BTreeSet<&str> = RP38_SUPERSEDED.iter().map(|(p, ..)| *p).collect();
    assert!(
        rp37.is_disjoint(&rp38),
        "a pair superseded by two engine changes would be counted under one lock and missing \
         from the other"
    );
    assert_eq!(rp37.len(), RP37_SUPERSEDED.len(), "one row per pair");
    assert_eq!(rp38.len(), RP38_SUPERSEDED.len(), "one row per pair");
    for pair in &rp37 {
        assert_eq!(superseded_tag(pair), Some("swtcontrol-per-phase"), "{pair}");
    }
    for pair in &rp38 {
        assert_eq!(superseded_tag(pair), Some("silent-read-only"), "{pair}");
    }
    assert_eq!(superseded_tag("swtcontrol.action"), None);
    assert_eq!(superseded_tag("line.ratings"), None);
}

/// **The root-cause bucket has a per-pair work list, and a settled pair stays in
/// it until its exclusion lands**: the both-ways guard for [`RP3_ROUTING`].
///
/// [`DECLARED_RP3`] is one number for four independent pairs, which is exactly
/// enough to hide two opposite mistakes: a pair quietly leaving the work list
/// because it was declared done while the tree holds no exclusion for it, and a
/// pair quietly growing or shrinking inside the total. So the routing is
/// re-measured here rather than trusted:
///
/// * it covers the four root-cause pairs, once each, and its three counted
///   columns sum to the bucket lock;
/// * the per-pair `(rows, in-scope rows)` split is what the walk actually
///   declares to [`Owner::Rp3`], not a transcription;
/// * the settled set is pinned literally (all four pairs), and each settled
///   verdict is checked **against the obligations of its own outcome tag**
///   ([`RP3_SETTLED_SHAPES`]): all three shapes must cite the r4133 unit and
///   name their sub-step column, `LEDGER` additionally owes the staging clause
///   and must name every [`LEDGER_ENTRY_PINS`] witness of that pair *as a whole
///   identifier* ([`names_identifier`]) while its pair carries **no** echo row,
///   `ECHO` owes the row itself, `FIX` owes the Rust site and "both lanes". An
///   unknown tag is a hard failure that names the table to extend;
/// * an open verdict points at its section and owns no pin, so a pin cannot be
///   landed for a pair that has no verdict.
#[test]
fn the_bin7_root_cause_pairs_are_routed_to_their_sub_steps() {
    assert_eq!(
        RP3_ROUTING.iter().map(|(p, ..)| *p).collect::<Vec<_>>(),
        BIN7_ROOT_CAUSE,
        "the routing covers exactly the four bin-7 root-cause pairs, once each"
    );
    // The middle term is the pairs that still DECLARE rows, not the table's
    // length: an entry can carry `0, 0` because the tree holds its exclusion
    // (an `ECHO` outcome's row, a ledger entry), and `Ledger::owner`'s pair
    // count, which this must equal, counts only pairs with a surviving row. The
    // entry itself stays for assertion #1.
    let declaring = || RP3_ROUTING.iter().filter(|(_, _, n, _, _)| *n > 0);
    assert_eq!(
        (
            RP3_ROUTING.iter().map(|(_, _, n, _, _)| n).sum::<usize>(),
            declaring().count(),
            RP3_ROUTING.iter().map(|(_, _, _, n, _)| n).sum::<usize>(),
        ),
        DECLARED_RP3,
        "the routing's three columns must sum to the bucket lock"
    );
    assert_eq!(
        RP3_ROUTING
            .iter()
            .filter(|(_, _, _, _, v)| !v.starts_with("OPEN — "))
            .map(|(p, s, _, _, _)| (*p, *s))
            .collect::<Vec<_>>(),
        [
            ("generator.model", "generator-model"),
            ("gictransformer.r2", "gictransformer-r2"),
            ("swtcontrol.delay", "swtcontrol-delay"),
            ("windgen.kvar", "windgen-kvar")
        ],
        "the settled root-cause pairs, in RP3_ROUTING order — ALL FOUR bin-7 root-cause \
         pairs, which is a legal end state and not a reason to relax anything below"
    );
    // …and a zero-row entry may only be one that is settled: an OPEN entry with
    // no rows would be a pair that vanished, not a pair that was excluded.
    for (pair, _, rows, in_scope, verdict) in RP3_ROUTING {
        assert_eq!(
            *rows == 0,
            *in_scope == 0,
            "{pair}: a pair cannot declare rows without declaring in-scope ones here, or vice versa"
        );
        assert!(
            *rows > 0 || !verdict.starts_with("OPEN — "),
            "{pair}: an OPEN verdict has no exclusion, so its rows must still be declared"
        );
    }

    for (pair, step, _, _, verdict) in RP3_ROUTING {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        let pins: Vec<&str> = LEDGER_ENTRY_PINS
            .iter()
            .filter(|(_, s, _)| s == step)
            .map(|(n, _, _)| *n)
            .collect();
        let (tag, rest) = verdict.split_once(" — ").unwrap_or_else(|| {
            panic!(
                "{pair}: a verdict opens with `OPEN — ` or with one of RP3_SETTLED_SHAPES' tags \
                 followed by ` — `, got {verdict:?}"
            )
        });
        if tag == "OPEN" {
            assert!(
                names_identifier(rest, pair),
                "{pair}: an open verdict must name its own pair, got {verdict:?}"
            );
            assert!(
                pins.is_empty(),
                "{pair}: an OPEN verdict owns no ledger-entry pin — got {pins:?}"
            );
            continue;
        }
        assert!(
            RP3_SETTLED_SHAPES.iter().any(|(t, _)| *t == tag),
            "{pair}: unknown outcome tag {tag:?}. The settled shapes are {:?} — a fourth \
             outcome is a decision: extend RP3_SETTLED_SHAPES with the obligations it owes, never \
             widen this guard to let an untyped verdict through",
            RP3_SETTLED_SHAPES
                .iter()
                .map(|(t, _)| *t)
                .collect::<Vec<_>>()
        );
        // Shared by all three settled shapes. The r4133 unit is the evidence base
        // and a capi citation is no substitute: a settled verdict routinely cites
        // both, so `.pas:` alone would be satisfied by the capi half.
        assert!(
            verdict.contains("Version8/Source/") && verdict.contains(".pas:"),
            "{pair}: a settled verdict must cite the r4133 unit itself \
             (Version8/Source/<unit>.pas:LINE), got {verdict:?}"
        );
        assert!(
            names_identifier(verdict, pair),
            "{pair}: a settled verdict must name its own pair, got {verdict:?}"
        );
        match tag {
            "LEDGER" => {
                let (.., ids) = RP3_LEDGERED
                    .iter()
                    .find(|(p, ..)| p == pair)
                    .unwrap_or_else(|| {
                        panic!("{pair}: a LEDGER verdict's entries must be in RP3_LEDGERED")
                    });
                assert!(
                    verdict.contains("tests/corpus/ledger.json")
                        && ids.split(", ").all(|id| names_identifier(verdict, id)),
                    "{pair}: a LEDGER verdict must name tests/corpus/ledger.json and each of its \
                     entries {ids}, got {verdict:?}"
                );
                assert!(
                    !pins.is_empty(),
                    "{pair}: a LEDGER verdict owes at least one pin"
                );
                for pin in &pins {
                    assert!(
                        names_identifier(verdict, pin),
                        "{pair}: the verdict must name its witness {pin} as a whole identifier — \
                         a longer pin that merely has it as a prefix does not name it"
                    );
                }
                assert!(
                    !props_norm::has_echo_row(class, prop),
                    "{pair}: a LEDGER outcome forbids an echo row — the getter is LIVE, so the \
                     exclusion is a ledger entry and the echo table would misname the mechanism"
                );
            }
            "ECHO" => {
                assert!(
                    props_norm::has_echo_row(class, prop),
                    "{pair}: an ECHO outcome must have landed its PROPS_ECHO_R4133 row"
                );
                assert!(
                    pins.is_empty(),
                    "{pair}: an echo row cites its own witness column, so {step} owns no \
                     LEDGER_ENTRY_PINS row — got {pins:?}"
                );
            }
            "FIX" => {
                assert!(
                    verdict.contains("crates/dss-core/src/") && verdict.contains("both lanes"),
                    "{pair}: a FIX must name its Rust site and say `both lanes`, got \
                     {verdict:?}"
                );
                assert!(
                    !props_norm::has_echo_row(class, prop),
                    "{pair}: a fixed port divergence is not excluded — no echo row belongs here"
                );
                assert!(
                    pins.is_empty(),
                    "{pair}: a fixed port divergence stages no ledger entry — got {pins:?}"
                );
            }
            _ => unreachable!("the tag was just checked against RP3_SETTLED_SHAPES"),
        }
    }

    // …and the per-pair split is the walk's, measured the same way the bucket is.
    let corpus = Corpus::load();
    let mut seen: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for row in &corpus.rows {
        if first_match(chain_verdicts(&corpus, row)).is_some() {
            continue;
        }
        // The rows a landed `ledger.json` entry excludes are intercepted before
        // `declare` in `account` (RP3_LEDGERED). Mirror that here, or this split
        // would report a population the table no longer declares. The zero-row
        // half below is what re-measures those pairs.
        if RP3_LEDGERED.iter().any(|(p, ..)| *p == row.pair) {
            continue;
        }
        let Some(ev) = corpus.evidence(row) else {
            continue;
        };
        if declare(row, ev) != Ok(Owner::Rp3) {
            continue;
        }
        let e = seen.entry(row.pair.as_str()).or_insert((0, 0));
        e.0 += 1;
        e.1 += usize::from(row_in_scope(row, ev));
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        declaring()
            .map(|(p, _, n, s, _)| (*p, (*n, *s)))
            .collect::<Vec<_>>(),
        "the routed pairs, row counts and in-scope splits must be exactly what the walk declares"
    );
    // The positive half of a `0, 0` entry, without which the zero would be an
    // untestable claim: the pair's rows must still BE in the corpus, and every
    // one of them must be claimed by the chain — so a pair that lost its rows
    // for any other reason (a dropped census row, a renamed pair) reds here
    // instead of passing as "excluded".
    for (pair, step, rows, _, verdict) in RP3_ROUTING {
        if *rows > 0 {
            continue;
        }
        assert!(
            verdict.starts_with("ECHO — ") || verdict.starts_with("LEDGER — "),
            "{pair}: a zero-row entry means the tree holds the exclusion NOW — only an ECHO \
             outcome (its row ships with the echo table) or a LEDGER outcome whose \
             entries have landed may declare zero rows — {step} says {verdict:?}"
        );
        let ledgered = RP3_LEDGERED.iter().find(|(p, ..)| *p == *pair);
        let mut ledgered_rows = 0usize;
        let mut ledgered_in_scope = 0usize;
        assert_eq!(
            ledgered.is_some(),
            verdict.starts_with("LEDGER — "),
            "{pair}: a zero-row LEDGER outcome is accounted by RP3_LEDGERED and nothing else \
             is — {step} says {verdict:?}"
        );
        let mut settled = 0usize;
        for row in corpus.rows.iter().filter(|r| r.pair == *pair) {
            match ledgered {
                // The echo half: the chain itself claims every row, on the
                // echo link and not on an earlier one.
                None => {
                    let link = first_match(chain_verdicts(&corpus, row)).unwrap_or_else(|| {
                        panic!(
                            "{pair} '{}' vs '{}': {step} declares no rows, so the chain must \
                             claim every example row of the pair — this one is claimed by \
                             nothing",
                            row.rust, row.r4133
                        )
                    });
                    assert_eq!(
                        link,
                        Link::Echo,
                        "{pair}: {step}'s outcome is the echo table, so its rows must be \
                         claimed by that link and not by an earlier one"
                    );
                }
                // The ledger half: the exclusion lives OUTSIDE props_norm, so
                // the chain must still decline the row — a link that claimed it
                // would mean two mechanisms cover the same cell and the ledger
                // entry would be the stale one — and the interception must be
                // what accounts for it.
                Some((_, _, _, _, ids)) => {
                    ledgered_rows += 1;
                    assert!(
                        first_match(chain_verdicts(&corpus, row)).is_none(),
                        "{pair} '{}' vs '{}': {step}'s exclusion is the ledger entries {ids}, \
                         yet a props_norm link claims this row too — one of the two mechanisms \
                         is redundant and the ledger entry would go stale silently",
                        row.rust,
                        row.r4133
                    );
                    let ev = corpus.evidence(row).unwrap_or_else(|| {
                        panic!(
                            "{pair} '{}' vs '{}': no bins.tsv row and no supplement provenance",
                            row.rust, row.r4133
                        )
                    });
                    assert_eq!(
                        declare(row, ev),
                        Ok(Owner::Rp3),
                        "{pair} '{}' vs '{}': without the RP3_LEDGERED interception this row \
                         would still be declared to RP3 — that is what makes the interception \
                         the thing that empties the bucket",
                        row.rust,
                        row.r4133
                    );
                    ledgered_in_scope += usize::from(row_in_scope(row, ev));
                }
            }
            settled += 1;
        }
        assert!(
            settled > 0,
            "{pair}: {step} declares no rows AND the corpus holds none — the pair vanished from \
             the evidence base instead of being excluded"
        );
        // The ledgered pairs left `declaring()` empty above, so this is where
        // their per-pair split is re-measured from the walk — the same guard the
        // declared pairs get, moved to the table that now owns them.
        if let Some((_, _, rows_want, in_scope_want, _)) = ledgered {
            assert_eq!(
                (ledgered_rows, ledgered_in_scope),
                (*rows_want, *in_scope_want),
                "{pair}: RP3_LEDGERED's row and in-scope columns must be exactly what the walk \
                 hands the interception"
            );
        }
    }
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(
        led.owner(Owner::Rp3),
        DECLARED_RP3,
        "the root-cause bucket (rows, pairs, rows on in-scope pairs) — empty: RP3_LEDGERED \
         holds the ledgered rows and the echo table claims generator.model"
    );
}

/// **The settled-outcome taxonomy has three tags, pinned literally** — like
/// [`NOT_A_PIN`] and [`LEDGER_ENTRY_PINS`], because it is the table the routing
/// guard dispatches on: a tag quietly added here would carry whatever
/// obligations its author felt like writing.
///
/// Exactly one outcome per pair — a port bug **fixed in both lanes**, an
/// upstream/echo divergence **excluded + pinned** (a ledger entry or an echo
/// row), or an upstream bug **reported** with its exclusion + pin. The reported
/// case lands as one of the two exclusions, which is why the tags are three and
/// not four.
#[test]
fn the_settled_outcome_taxonomy_has_three_tags() {
    assert_eq!(
        RP3_SETTLED_SHAPES
            .iter()
            .map(|(t, _)| *t)
            .collect::<Vec<_>>(),
        ["LEDGER", "ECHO", "FIX"],
        "the three settled shapes, and nothing else"
    );
    for (tag, owes) in RP3_SETTLED_SHAPES {
        assert!(
            !owes.is_empty(),
            "{tag}: a tag with no stated obligation is a hole in the guard"
        );
    }
}

/// **Naming a witness is a whole-identifier match, not a substring one**: the
/// self-test for [`names_identifier`], on the pair of pins whose names are a
/// prefix of one another.
#[test]
fn naming_a_witness_is_a_whole_identifier_match() {
    let short = "swtcontrol_delay_wires_the_property";
    let long = "swtcontrol_delay_wires_the_property_on_the_midi_tie";
    assert!(
        LEDGER_ENTRY_PINS.iter().any(|(n, _, _)| *n == short)
            && LEDGER_ENTRY_PINS.iter().any(|(n, _, _)| *n == long),
        "both pins must still be the live example of the prefix hazard"
    );
    assert!(
        !names_identifier(long, short),
        "the longer pin must NOT count as naming the shorter one — that is the shadowing this \
         helper exists to stop"
    );
    assert!(names_identifier(
        &format!("witnessed by {short} and {long}."),
        short
    ));
    assert!(names_identifier(
        &format!("witnessed by {short} and {long}."),
        long
    ));
    assert!(!names_identifier(
        "a_swtcontrol_delay_wires_the_property",
        short
    ));
}

/// **[`RP3_LEDGERED`]'s rows are excluded by entries that are really in the
/// ledger**: the both-ways guard that makes [`DECLARED_RP3`]'s `(0, 0, 0)` a
/// measurement instead of an edit.
///
/// The interception itself only knows a list of pairs. On its own it would be a
/// way to make rows disappear from the accounting by typing a pair name. What
/// makes it an exclusion is `tests/corpus/ledger.json`, read here LIVE:
///
/// * each id [`RP3_LEDGERED`] names must exist in the ledger, on
///   `channel: "r4133"`, `kind: "divergence"`, with a `property` scope;
/// * that scope's `name_re` must be the anchored case-insensitive literal
///   `(?i)^<class>\.<element>\.<prop>$` the loader keys on
///   (`corpus_gate/ledger.rs::scope_names_prop`), and its `<class>` / `<prop>`
///   must be THIS pair: an entry that excludes some other property cannot be
///   what retires this pair's rows;
/// * it must pin an exact `rust` AND an exact `oracle` (the exact-pair rule for
///   discrete state), so the retirement rests on two named numbers;
/// * and, both ways, every `property`-scoped `r4133` entry in the ledger must be
///   named by exactly one row here: landing a new entry without an accounting
///   move reds this test.
///
/// It also checks the retirement per **spelling**, not only per pair: the
/// interception retires every frozen row of a ledgered pair, while an entry
/// excludes one exact `(rust, oracle)`, so a row no entry names must be listed
/// in [`RP3_LEDGERED_UNNAMED`] with the measurement that makes crediting it
/// sound (and every listed row must still be one).
///
/// The counted columns are re-measured from the walk by
/// [`every_example_row_is_claimed_or_declared_exactly_once`] (the
/// [`LEDGERED_RP3`] lock) and by the zero-row half of
/// [`the_bin7_root_cause_pairs_are_routed_to_their_sub_steps`], so this test is
/// about the *entries*, not about the counts.
#[test]
fn the_ledgered_rows_are_excluded_by_entries_that_are_really_in_the_ledger() {
    /// `(?i)^<class>\.<element>\.<prop>$` -> `(class, prop)`, lowercased. Any
    /// other regex shape is a hard failure: this guard may not silently stop
    /// understanding the scope it is checking.
    fn literal_class_prop(id: &str, name_re: &str) -> (String, String) {
        let body = name_re
            .strip_prefix("(?i)^")
            .and_then(|b| b.strip_suffix('$'))
            .unwrap_or_else(|| {
                panic!(
                    "{id}: RP3_LEDGERED can only vouch for the anchored case-insensitive \
                     literal scope form `(?i)^class\\.element\\.prop$`, got {name_re:?}"
                )
            });
        let parts: Vec<&str> = body.split("\\.").collect();
        assert_eq!(
            parts.len(),
            3,
            "{id}: a property scope keys on `element.prop` where `element` is `class.name` \
             (corpus_gate/ledger.rs), so the literal has three dotted parts — got {name_re:?}"
        );
        for part in &parts {
            assert!(
                !part.is_empty()
                    && part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '%')),
                "{id}: {part:?} is not a plain literal, so this scope may match more than the \
                 pair RP3_LEDGERED claims it does — {name_re:?}"
            );
        }
        (parts[0].to_lowercase(), parts[2].to_lowercase())
    }

    let path = repo_root().join(LEDGER);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&text).expect("ledger.json is JSON");
    let entries = doc["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array");

    let corpus = Corpus::load();
    let mut unnamed_used: BTreeSet<(&str, &str, &str)> = BTreeSet::new();
    let mut named: Vec<&str> = Vec::new();
    for (pair, step, rows, in_scope, ids) in RP3_LEDGERED {
        let routing = RP3_ROUTING
            .iter()
            .find(|(p, ..)| p == pair)
            .unwrap_or_else(|| panic!("{pair}: RP3_LEDGERED names a pair RP3_ROUTING does not"));
        assert_eq!(
            routing.1, *step,
            "{pair}: the two tables must agree on which sub-step owns the pair"
        );
        assert_eq!(
            (routing.2, routing.3),
            (0, 0),
            "{pair}: a ledgered pair declares no rows — retire its RP3_ROUTING row in the same \
             commit that lands its entries"
        );
        assert!(
            routing.4.starts_with("LEDGER — "),
            "{pair}: only a LEDGER outcome is excluded by a ledger entry"
        );
        assert!(
            *rows > 0 && *in_scope > 0,
            "{pair}: an interception that accounts for no row is a pair that vanished, not a \
             pair that was excluded"
        );
        let (class, prop) = pair.split_once('.').expect("class.prop");
        let mut entry_spellings: BTreeSet<(String, String)> = BTreeSet::new();
        for id in ids.split(", ") {
            named.push(id);
            let entry = entries.iter().find(|e| e["id"] == *id).unwrap_or_else(|| {
                panic!(
                    "{pair}: RP3_LEDGERED names {id:?}, which is not in \
                     tests/corpus/ledger.json — the interception would retire rows nothing \
                     excludes"
                )
            });
            assert_eq!(
                entry["channel"], "r4133",
                "{id}: only an r4133 entry can exclude a cell of this file's chain"
            );
            assert_eq!(
                entry["kind"], "divergence",
                "{id}: an exclusion of a single property cell is a `divergence` with an exact \
                 pair, never a `skip` or a deck-wide `exclusion`"
            );
            let scopes: Vec<&serde_json::Value> = entry["match"]
                .as_array()
                .unwrap_or_else(|| panic!("{id}: a divergence entry carries a `match` array"))
                .iter()
                .filter(|m| m["field"] == "property")
                .collect();
            assert_eq!(
                scopes.len(),
                1,
                "{id}: RP3_LEDGERED vouches for exactly one property scope per entry"
            );
            let sc = scopes[0];
            let name_re = sc["name_re"]
                .as_str()
                .unwrap_or_else(|| panic!("{id}: a property scope needs a `name_re`"));
            let (got_class, got_prop) = literal_class_prop(id, name_re);
            assert_eq!(
                (got_class.as_str(), got_prop.as_str()),
                (class, prop),
                "{id}: the scope names a different property than the pair it is credited with"
            );
            assert!(
                sc["rust"].is_string() && sc["oracle"].is_string(),
                "{id}: a discrete-state exclusion pins BOTH numbers — got \
                 rust={:?} oracle={:?}",
                sc["rust"],
                sc["oracle"]
            );
            entry_spellings.insert((
                sc["rust"].as_str().unwrap().to_string(),
                sc["oracle"].as_str().unwrap().to_string(),
            ));
        }
        // …and per SPELLING, not just per pair: the interception retires every
        // frozen row of the pair, while an entry excludes one exact
        // `(rust, oracle)`. A row no entry names is credited as "ledgered" only
        // if [`RP3_LEDGERED_UNNAMED`] says why that is sound.
        for row in corpus.rows.iter().filter(|r| r.pair == *pair) {
            if entry_spellings.contains(&(row.rust.clone(), row.r4133.clone())) {
                continue;
            }
            let listed = RP3_LEDGERED_UNNAMED
                .iter()
                .find(|(p, rust, r4133, _)| p == pair && *rust == row.rust && *r4133 == row.r4133)
                .unwrap_or_else(|| {
                    panic!(
                        "{pair} '{}' vs '{}': RP3_LEDGERED credits this frozen row to a landed \
                         entry, but no entry of {ids} pins that spelling — the row would reach \
                         the r4133 value compare unexcluded. Land an entry for it, or record it \
                         in RP3_LEDGERED_UNNAMED with the measurement that makes it sound.",
                        row.rust, row.r4133
                    )
                });
            unnamed_used.insert((listed.0, listed.1, listed.2));
        }
    }
    // No dead rows in the exemption table either.
    let unnamed_listed: BTreeSet<(&str, &str, &str)> = RP3_LEDGERED_UNNAMED
        .iter()
        .map(|(p, rust, r4133, _)| (*p, *rust, *r4133))
        .collect();
    assert_eq!(
        unnamed_used, unnamed_listed,
        "every RP3_LEDGERED_UNNAMED row must still be a frozen spelling of a ledgered pair that \
         no entry names — a row that stopped being one is either now excluded (drop the row) or \
         its pair left the interception"
    );
    assert_eq!(
        (
            RP3_LEDGERED.iter().map(|(_, _, n, _, _)| n).sum::<usize>(),
            RP3_LEDGERED.len(),
            RP3_LEDGERED.iter().map(|(_, _, _, n, _)| n).sum::<usize>(),
        ),
        LEDGERED_RP3,
        "the interception's three columns must sum to the bucket lock"
    );

    // …and the other direction: no landed r4133 property entry may sit outside
    // this accounting.
    let landed: BTreeSet<&str> = entries
        .iter()
        .filter(|e| e["channel"] == "r4133")
        .filter(|e| {
            e["match"]
                .as_array()
                .is_some_and(|ms| ms.iter().any(|m| m["field"] == "property"))
        })
        .map(|e| e["id"].as_str().unwrap_or("<no id>"))
        .collect();
    named.extend(LEDGER_ENTRIES_OUTSIDE_THE_CENSUS.iter().map(|(id, ..)| *id));
    let accounted: BTreeSet<&str> = named.iter().copied().collect();
    assert_eq!(
        named.len(),
        accounted.len(),
        "RP3_LEDGERED and LEDGER_ENTRIES_OUTSIDE_THE_CENSUS name an entry twice: {named:?}"
    );
    assert_eq!(
        landed, accounted,
        "every `property`-scoped `r4133` ledger entry must be accounted for by an RP3_LEDGERED \
         row or a LEDGER_ENTRIES_OUTSIDE_THE_CENSUS row, and every row must name entries that \
         exist. A NEW staged entry landing here without its accounting move is exactly what \
         this half catches — retire its RP3_ROUTING row (or the successor table that owns it), \
         shrink the bucket lock by exactly those rows, and name the entry above."
    );
}

/// **The r4133 `property` entries in the ledger are exactly the root-cause
/// eight plus the named outside-the-census ones**, each on the case its
/// [`LEDGER_ENTRY_PINS`] row cites. The other half, "no NEW property-scoped
/// r4133 entry appears un-reviewed", is enforced next door by
/// [`the_ledgered_rows_are_excluded_by_entries_that_are_really_in_the_ledger`].
///
/// The eight:
///
/// * `swtcontrol.delay`: `r4133-swtcontrol-delay-ignored-time` and
///   `r4133-swtcontrol-delay-ignored-midi`;
/// * `windgen.kvar`: `r4133-windgen-kvar-dispatched-daily` / `-delta` / `-dyn` /
///   `-dynfault`;
/// * `gictransformer.r2`: `gic-pct-r2-honoured-gictransformer-r4133-props` and
///   `gic-pct-r2-honoured-midi-r4133-props`.
///
/// `gendispatcher.weights` has none: its divergence is whole-solution, its decks
/// stay `capi_v0145`, and its exclusion is a `PROPS_015X` allowlist row.
/// `generator.model` has none: its exclusion is a `PROPS_ECHO_R4133` row. The
/// round-trip residue has none (its pairs are `PRECISION_ROUNDTRIP` pins), and
/// the AutoTrans typecast bug's `skip` entries have their own tripwire,
/// [`the_autotrans_typecast_cases_pair_their_r4133_channel_with_a_skip_entry`].
#[test]
fn the_r4133_property_entries_are_the_root_cause_eight() {
    let path = repo_root().join(LEDGER);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&text).expect("ledger.json is JSON");
    let entries = doc["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array");
    let landed: BTreeSet<&str> = entries
        .iter()
        .filter(|e| e["channel"] == "r4133")
        .filter(|e| {
            e["match"]
                .as_array()
                .is_some_and(|ms| ms.iter().any(|m| m["field"] == "property"))
        })
        .map(|e| e["id"].as_str().unwrap_or("<no id>"))
        .collect();

    // What was staged, read off the pin list rather than re-typed: each row
    // cites `<entry id> (<case>)`.
    let mut staged: BTreeSet<&str> = BTreeSet::new();
    for (pin, step, cite) in LEDGER_ENTRY_PINS {
        let (id, case) = cite
            .split_once(" (")
            .and_then(|(i, c)| c.strip_suffix(')').map(|c| (i, c)))
            .unwrap_or_else(|| {
                panic!("{pin}: a citation reads `<entry id> (<case>)`, got {cite:?}")
            });
        staged.insert(id);
        let entry = entries.iter().find(|e| e["id"] == id).unwrap_or_else(|| {
            panic!(
                "{step}'s entry {id:?} (witness {pin}) is NOT in tests/corpus/ledger.json — \
                 the ledger must hold every root-cause entry, or the pin has nothing to hold"
            )
        });
        assert_eq!(
            entry["case"], case,
            "{id}: the landed entry sits on a different case than {pin} cites"
        );
    }
    for (id, case, _, pin) in LEDGER_ENTRIES_OUTSIDE_THE_CENSUS {
        let entry = entries
            .iter()
            .find(|e| e["id"] == *id)
            .unwrap_or_else(|| panic!("{id:?} (witness {pin}) is NOT in tests/corpus/ledger.json"));
        assert_eq!(
            entry["case"], *case,
            "{id}: the landed entry sits on a different case than its row cites"
        );
        assert!(staged.insert(*id), "{id}: listed twice");
    }
    assert_eq!(
        landed, staged,
        "the `property`-scoped `r4133` entries in the ledger must be exactly the root-cause \
         eight (LEDGER_ENTRY_PINS) and the entries outside the frozen census \
         (LEDGER_ENTRIES_OUTSIDE_THE_CENSUS). An id on the left only is a NEW exclusion that \
         owes a pin and an accounting move; an id on the right only means an entry was \
         dropped and its pair's rows are retired against nothing."
    );
}

/// Each entry of [`LEDGER_ENTRIES_OUTSIDE_THE_CENSUS`] is an `r4133`
/// `divergence` on the case its row cites, every property scope of it is the
/// anchored literal `(?i)^class\.element\.prop$` form pinning both an exact
/// `rust` and an exact `oracle`, no spelling it pins is a frozen census row of
/// its pair (a property scope excludes one exact spelling, so it retires
/// nothing [`RP3_LEDGERED`] would have to account for), and its pin is a test
/// of the file its row names under [`OUTSIDE_THE_CENSUS_PINS`].
#[test]
fn the_ledger_entries_outside_the_census_retire_no_frozen_row() {
    let path = repo_root().join(LEDGER);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&text).expect("ledger.json is JSON");
    let entries = doc["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array");
    let corpus = Corpus::load();
    for (id, case, file, pin) in LEDGER_ENTRIES_OUTSIDE_THE_CENSUS {
        let pins_path = repo_root().join(OUTSIDE_THE_CENSUS_PINS).join(file);
        let pins = std::fs::read_to_string(&pins_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", pins_path.display()));
        let entry = entries
            .iter()
            .find(|e| e["id"] == *id)
            .unwrap_or_else(|| panic!("{id:?} is not in tests/corpus/ledger.json"));
        assert_eq!(entry["channel"], "r4133", "{id}");
        assert_eq!(entry["kind"], "divergence", "{id}");
        assert_eq!(entry["case"], *case, "{id}");
        let scopes: Vec<&serde_json::Value> = entry["match"]
            .as_array()
            .unwrap_or_else(|| panic!("{id}: a divergence entry carries a `match` array"))
            .iter()
            .filter(|m| m["field"] == "property")
            .collect();
        assert!(!scopes.is_empty(), "{id}: no property scope");
        for sc in scopes {
            let name_re = sc["name_re"]
                .as_str()
                .unwrap_or_else(|| panic!("{id}: a property scope needs a `name_re`"));
            let body = name_re
                .strip_prefix("(?i)^")
                .and_then(|b| b.strip_suffix('$'))
                .unwrap_or_else(|| panic!("{id}: {name_re:?} is not an anchored literal"));
            let parts: Vec<&str> = body.split("\\.").collect();
            assert!(
                parts.len() == 3
                    && parts.iter().all(|p| !p.is_empty()
                        && p.chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '%'))),
                "{id}: {name_re:?} is not the literal `class.element.prop`"
            );
            assert!(
                sc["rust"].is_string() && sc["oracle"].is_string(),
                "{id}: {name_re:?} pins both numbers"
            );
            let pair = format!("{}.{}", parts[0].to_lowercase(), parts[2].to_lowercase());
            let (rust, oracle) = (sc["rust"].as_str(), sc["oracle"].as_str());
            assert!(
                corpus.rows.iter().all(|r| r.pair != pair
                    || Some(r.rust.as_str()) != rust
                    || Some(r.r4133.as_str()) != oracle),
                "{id}: {pair} holds the frozen census row {rust:?}/{oracle:?}, so the entry \
                 belongs in RP3_LEDGERED"
            );
        }
        assert_eq!(
            pins.matches(&format!("fn {pin}()")).count(),
            1,
            "{id}: its pin {pin} is not a test of {}",
            pins_path.display()
        );
    }
}

/// **The AutoTrans typecast cases pair their r4133 channel with a `skip`
/// entry, both ways.**
///
/// [`DECLARED_RP312`]'s verdict is `UPSTREAM_BUG`: r4133 never taps an
/// `AutoTrans`, so on the four `controls:autotrans/*` decks its whole solved
/// state is the UNREGULATED circuit's. Nothing is excluded because nothing is
/// compared (all four cases are `engines: "capi_v0145"`), and an entry landed
/// now would be **stale on arrival**, which the gate's fail-on-stale rule reds.
/// The day one of these decks gains the `r4133` channel, this test names the
/// entry it owes.
///
/// The condition is therefore the **pairing**, both ways: a case gates r4133
/// **iff** it carries an `r4133` `skip` entry citing [`RP312_TYPECAST_CAUSE`].
/// Gaining the channel without the entry reds here with the entry to write;
/// landing the entry while the case is still capi-only reds here too, before the
/// corpus gate reds it as stale. The cause key itself is asserted to exist.
#[test]
fn the_autotrans_typecast_cases_pair_their_r4133_channel_with_a_skip_entry() {
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");

    let ledger_path = repo_root().join(LEDGER);
    let ledger: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&ledger_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", ledger_path.display())),
    )
    .expect("ledger.json is JSON");
    assert!(
        ledger["causes"][RP312_TYPECAST_CAUSE].is_string(),
        "the cause key {RP312_TYPECAST_CAUSE:?} is gone from ledger.json — the AutoTrans \
         typecast entries cite it"
    );
    let entries = ledger["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array");

    for (case, staged_id) in RP312_STAGED_SKIPS {
        let rigor = case_rigor(&lock, case);
        let engines = rigor_field(&rigor, "engines");
        let landed: Vec<&str> = entries
            .iter()
            .filter(|e| {
                e["case"] == *case
                    && e["channel"] == "r4133"
                    && e["kind"] == "skip"
                    && e["cause_ref"] == RP312_TYPECAST_CAUSE
            })
            .map(|e| e["id"].as_str().unwrap_or("<no id>"))
            .collect();
        assert_eq!(
            engines != "capi_v0145",
            !landed.is_empty(),
            "{case}: engines={engines:?} while its AutoTrans typecast `skip` entries are {landed:?} \
             — the two move together. A deck gaining the r4133 channel owes, in the SAME \
             commit, an entry `{staged_id}` with channel r4133, kind skip and cause_ref \
             {RP312_TYPECAST_CAUSE:?}: r4133 never taps an AutoTrans and the \
             WHOLE case diverges, so a field exclusion is the wrong instrument. An entry \
             landing EARLY is stale on arrival and reds the corpus gate instead."
        );
    }
}

/// **The `swtcontrol.delay` census decomposition is read off the corpus.**
///
/// The pair's conclusion, *exactly two* ledger entries, is a statement about
/// cases: the cells of `swtcontrol.delay`, the in-scope ones over exactly the
/// two `engines: r4133` decks that type `delay=`. Every one of those numbers is
/// derived here, from three independent places, and each derivation is
/// reconciled against the next:
///
/// * the **decks** give the control count and the typed value (a corpus-wide
///   sweep, so a new SwtControl deck cannot appear unnoticed:
///   [`RP31_DELAY_CASES`] and [`RP31_NO_DELAY_CASES`] must together be every
///   deck that declares one);
/// * `population.lock.json` gives each case's `steps=` and `engines=`, so
///   `cells = controls × steps` and "in scope" is the lock's own answer, not a
///   transcription;
/// * the **frozen census** (`bins.tsv` and `examples_full.txt`'s rows) is what
///   the products must add up to, per spelling and in total.
///
/// Then the two consumers are tied to the result: the in-scope cases are exactly
/// the cases [`LEDGER_ENTRY_PINS`] cites (one entry each, no more), and the
/// routing verdict must carry the derived figures verbatim.
#[test]
fn the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "swtcontrol.delay";
    const STEP: &str = "swtcontrol-delay";

    // (1) Completeness: the two tables ARE every corpus deck that declares a
    //     SwtControl, with the control count and typed token each one carries.
    let root = repo_root().join(CORPUS);
    let mut decks = Vec::new();
    collect_dss(&root, &root, &mut decks);
    assert!(
        decks.len() > 1000,
        "only {} .dss files under {CORPUS} — the vendored corpus is missing",
        decks.len()
    );
    let measured: BTreeMap<String, (usize, Option<String>)> = decks
        .iter()
        .map(|d| (d.clone(), swtcontrol_facts(d)))
        .filter(|(_, (declared, _))| *declared > 0)
        .collect();
    let cited: BTreeMap<String, (usize, Option<String>)> = RP31_DELAY_CASES
        .iter()
        .map(|(case, n, token)| (case_deck(case), (*n, Some((*token).to_string()))))
        .chain(
            RP31_NO_DELAY_CASES
                .iter()
                .map(|(case, n)| (case_deck(case), (*n, None))),
        )
        .collect();
    assert_eq!(
        measured, cited,
        "every corpus deck declaring a SwtControl must sit in the swtcontrol.delay decomposition with its \
         measured control count and `delay=` token — a new one changes how many ledger entries \
         the pair owes"
    );

    // (2) Per case: cells = controls × steps, in scope iff the case gates r4133.
    let skipped = r4133_skipped_cases();
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");
    let corpus = Corpus::load();
    let rows: Vec<&Example> = corpus.rows.iter().filter(|r| r.pair == PAIR).collect();
    assert_eq!(
        rows.len(),
        2,
        "the frozen census spells the pair two ways ('0.25' and '0'), got {rows:?}"
    );
    // spelling -> (cells, in-scope cells)
    let mut per_spelling: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut in_scope_cases: Vec<(&str, usize)> = Vec::new();
    for (case, declared, token) in RP31_DELAY_CASES {
        let rigor = case_rigor(&lock, case);
        let steps: usize = rigor_field(&rigor, "steps")
            .parse()
            .unwrap_or_else(|e| panic!("{case}: steps= is not a number: {e}"));
        let cells = declared * steps;
        // Both halves of "does r4133 gate this case?", see `r4133_skipped_cases`.
        // No case here is skipped, so one becoming skipped reds instead of
        // passing.
        let in_scope = rigor_field(&rigor, "engines") != "capi_v0145" && !skipped.contains(*case);
        let typed: f64 = token
            .parse()
            .unwrap_or_else(|e| panic!("{case}: delay={token} is not a number: {e}"));
        let row = rows
            .iter()
            .find(|r| r.rust.parse::<f64>().is_ok_and(|v| v == typed))
            .unwrap_or_else(|| {
                panic!("{case}: no frozen census row renders the deck's `delay={token}`")
            });
        assert_eq!(
            row.r4133, "120",
            "{case}: r4133 must render Create's default — that IS the divergence"
        );
        let e = per_spelling.entry(row.rust.as_str()).or_insert((0, 0));
        e.0 += cells;
        if in_scope {
            e.1 += cells;
            in_scope_cases.push((case, cells));
        }
    }
    in_scope_cases.sort_unstable();

    // (3) …and the products are the frozen census's own numbers, per spelling
    //     and in total.
    let ev = corpus
        .evidence(rows[0])
        .unwrap_or_else(|| panic!("{PAIR}: no frozen evidence record"));
    assert_eq!(
        per_spelling.values().map(|(c, _)| *c).sum::<usize>(),
        ev.cells,
        "derived cells must be bins.tsv's cell count for {PAIR}"
    );
    assert_eq!(
        Some(per_spelling.values().map(|(_, c)| *c).sum::<usize>()),
        ev.cells_in_scope,
        "derived in-scope cells must be bins.tsv's cells_in_scope for {PAIR}"
    );
    for row in &rows {
        assert_eq!(
            per_spelling.get(row.rust.as_str()).map(|(c, _)| *c),
            Some(row.cells),
            "'{}': the frozen example row's count must be the sum over the cases that type it",
            row.rust
        );
    }

    // (4) One drafted entry per in-scope case, and no other.
    let mut entry_cases: Vec<&str> = LEDGER_ENTRY_PINS
        .iter()
        .filter(|(_, s, _)| *s == STEP)
        .map(|(_, _, cite)| {
            cite.split_once(" (")
                .and_then(|(_, rest)| rest.strip_suffix(')'))
                .unwrap_or_else(|| panic!("{cite:?}: the citation must name its case in parens"))
        })
        .collect();
    entry_cases.sort_unstable();
    assert_eq!(
        in_scope_cases.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        entry_cases,
        "exactly the in-scope cases owe a drafted ledger entry — one each, and the out-of-scope \
         cells owe none (the r4133 compare never reaches them)"
    );

    // (5) The verdict carries the derived figures, so its prose cannot drift
    //     away from the corpus it describes.
    let verdict = RP3_ROUTING
        .iter()
        .find(|(p, ..)| *p == PAIR)
        .map(|(_, _, _, _, v)| *v)
        .unwrap_or_else(|| panic!("{PAIR} has no routing row"));
    let (typed_spelling, (typed_cells, typed_in_scope)) = per_spelling
        .iter()
        .find(|(_, (_, in_scope))| *in_scope > 0)
        .map(|(s, c)| (*s, *c))
        .expect("one spelling carries the in-scope cells");
    assert_eq!(
        per_spelling
            .values()
            .filter(|(_, in_scope)| *in_scope > 0)
            .count(),
        1,
        "only the typed spelling may have in-scope cells"
    );
    let untyped_cells = per_spelling
        .iter()
        .find(|(s, _)| **s != typed_spelling)
        .map(|(_, (c, _))| *c)
        .expect("the second spelling");
    let split = in_scope_cases
        .iter()
        .map(|(_, c)| c.to_string())
        .collect::<Vec<_>>()
        .join(" + ");
    for phrase in [
        format!("spelling's {typed_cells} cells are {typed_in_scope} in scope"),
        format!("{split} over the two r4133 decks"),
        format!("plus {} on capi_v0145", typed_cells - typed_in_scope),
        format!("spelling's {untyped_cells} cells are three capi_v0145 IEEE_519 copies"),
        format!(
            "{typed_in_scope} in-scope cells over exactly {} cases",
            in_scope_cases.len()
        ),
    ] {
        assert!(
            verdict.contains(&phrase),
            "the swtcontrol.delay verdict must carry the derived census — {phrase:?} is missing from \
             {verdict:?}"
        );
    }
    assert!(
        names_identifier(
            verdict,
            "the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus"
        ),
        "the verdict must name the test that derives its numbers, so a rename cannot orphan it"
    );
}

/// **The `windgen.kvar` census decomposition is read off the corpus**:
/// [`the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus`]'s shape, applied to
/// `windgen.kvar`.
///
/// The pair's conclusion, *exactly four* ledger entries, rests on a
/// decomposition that must close over **every** cell of the pair and every
/// corpus deck that could add one:
///
/// * the **decks** give the WindGen count and the `kW=`/`pf=`/`kVA=` tokens
///   ([`windgen_facts`]), swept corpus-wide so a new WindGen deck cannot appear
///   unnoticed: [`RP32_WINDGEN_CASES`] and [`RP32_WINDGEN_SKIPPED_DECKS`] must
///   together be every deck that declares one, and the skipped pair is checked
///   against `skipped_oracle_issue.json` itself, with the nonzero base it
///   *would* contribute if promoted;
/// * the **`QMode=` token** ([`windgen_dispatch`]) says which dispatch arm each
///   deck selects, so "r4133 renders the dispatched zero" is read off the decks
///   (none of the five types one) instead of being asserted about them, and the
///   held-out pair, which takes the volt-var arm instead, cannot be described as
///   if it shared the mechanism;
/// * the **arithmetic** derives our render from those tokens
///   ([`windgen_kvar_base`]) rather than transcribing it: no deck types `kvar=`,
///   so the whole census is a side effect of `pf=`/`kVA=`, and a cell exists
///   exactly where the derived base is nonzero;
/// * `population.lock.json` gives each case's `steps=`/`engines=`, so
///   `cells = WindGens × steps` and "in scope" is the lock's answer *plus* the
///   ledger's: an `engines: "both"` case whose r4133 channel a `skip` entry drops
///   is not gated there ([`r4133_skipped_cases`]);
/// * the **frozen census** (`bins.tsv` and `examples_full.txt`'s rows) is what
///   the products must add up to, per spelling and in total, and every one of
///   those rows must have `0` on the r4133 side, which IS the divergence.
///
/// Then the consumers are tied to the result: the in-scope cases are exactly the
/// cases [`LEDGER_ENTRY_PINS`] cites (one entry each, no more), and the routing
/// verdict must carry the derived figures verbatim.
#[test]
fn the_windgen_kvar_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "windgen.kvar";
    const STEP: &str = "windgen-kvar";

    // (1) Completeness: the two tables ARE every corpus deck that declares a
    //     WindGen, with the tokens each declaration types.
    let root = repo_root().join(CORPUS);
    let mut decks = Vec::new();
    collect_dss(&root, &root, &mut decks);
    assert!(
        decks.len() > 1000,
        "only {} .dss files under {CORPUS} — the vendored corpus is missing",
        decks.len()
    );
    type Facts = (usize, String, String, String, String);
    let measured: BTreeMap<String, Facts> = decks
        .iter()
        .map(|d| (d.clone(), windgen_facts(d)))
        .filter(|(_, f)| f.0 > 0)
        .collect();
    let tokens = |n: &usize, kw: &str, pf: &str, kva: &str| -> Facts {
        (
            *n,
            kw.to_string(),
            pf.to_string(),
            kva.to_string(),
            String::new(),
        )
    };
    let cited: BTreeMap<String, Facts> = RP32_WINDGEN_CASES
        .iter()
        .map(|(case, n, kw, pf, kva)| (case_deck(case), tokens(n, kw, pf, kva)))
        .chain(
            RP32_WINDGEN_SKIPPED_DECKS
                .iter()
                .map(|(deck, n, kw, pf, kva)| ((*deck).to_string(), tokens(n, kw, pf, kva))),
        )
        .collect();
    assert_eq!(
        measured, cited,
        "every corpus deck declaring a WindGen must sit in the windgen.kvar decomposition with its \
         measured count and kW=/pf=/kVA= tokens (and no `kvar=`, the fifth field — neither typed \
         on the `New` line nor edited in later) — a new one changes how many ledger entries the \
         pair owes"
    );

    // The mechanism behind r4133's `0`, read off the decks: none of the five
    // selects a Q-dispatch arm, so r4133's steady-state dispatch takes its zero
    // fallback, which this engine replaces with the documented constant-Q arm.
    // The assertion is about what the decks type, and it keeps both mechanisms
    // (r4133's render AND the ledgered dispatch divergence) attached to this
    // population.
    for (case, ..) in RP32_WINDGEN_CASES {
        let (qmode, vv_curve) = windgen_dispatch(&case_deck(case));
        assert_eq!(
            (qmode.as_str(), vv_curve.as_str()),
            ("", ""),
            "{case}: a census deck that selected a Q-dispatch arm would no longer take the `Else \
             kvarCalc := 0` path this pair's r4133 zero comes from"
        );
    }

    // …and the held-out pair really is held out, by name and by tag, with the
    // base it would contribute if it were ever promoted — and on a mechanism of
    // its own, which is why nothing claims an r4133 value for these two.
    let skipped_path = repo_root().join(SKIPPED_ORACLE_ISSUE);
    let skipped: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&skipped_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", skipped_path.display())),
    )
    .expect("skipped_oracle_issue.json is JSON");
    let held = skipped["cases"]
        .as_array()
        .expect("skipped_oracle_issue.json has a `cases` array");
    for (deck, _, kw, pf, kva) in RP32_WINDGEN_SKIPPED_DECKS {
        let rel = deck
            .strip_prefix("electricdss-tst/")
            .unwrap_or_else(|| panic!("{deck}: a skipped deck is a vendored one"));
        let case = held
            .iter()
            .find(|c| c["path"] == rel)
            .unwrap_or_else(|| panic!("{rel} is not held out by {SKIPPED_ORACLE_ISSUE}"));
        assert_eq!(
            case["tag"], RP32_SKIPPED_TAG,
            "{rel}: the hold-out reason must still be the oracle's multi-step limitation — any \
             other tag is a different decision about the population"
        );
        assert!(
            windgen_kvar_base(kw, pf, kva) > 0.0,
            "{rel}: a skipped deck whose derived base is zero would prove nothing about the \
             count of ledger entries"
        );
        let (qmode, vv_curve) = windgen_dispatch(deck);
        assert_eq!(
            qmode, "2",
            "{rel}: the doc argues from this deck taking the volt-var arm, not the `Else \
             kvarCalc := 0` arm the five census decks take"
        );
        assert!(
            !vv_curve.is_empty(),
            "{rel}: QMode=2 without a VV_Curve dispatches 0 through a different branch \
             — then the doc's reading of this deck is wrong"
        );
    }

    // (2) Per case: cells = WindGens × steps, in scope iff the case gates r4133
    //     — and a cell exists only where the derived base is nonzero.
    let skipped = r4133_skipped_cases();
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");
    let corpus = Corpus::load();
    let rows: Vec<&Example> = corpus.rows.iter().filter(|r| r.pair == PAIR).collect();
    assert_eq!(
        rows.len(),
        3,
        "the frozen census spells the pair three ways, got {rows:?}"
    );
    // spelling -> (cells, in-scope cells)
    let mut per_spelling: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut in_scope_cases: Vec<(&str, usize)> = Vec::new();
    let mut no_cell: Vec<(&str, &str)> = Vec::new();
    for (case, declared, kw, pf, kva) in RP32_WINDGEN_CASES {
        let rigor = case_rigor(&lock, case);
        let steps: usize = rigor_field(&rigor, "steps")
            .parse()
            .unwrap_or_else(|e| panic!("{case}: steps= is not a number: {e}"));
        let cells = declared * steps;
        // Both halves of the scope question, see `r4133_skipped_cases`.
        let in_scope = rigor_field(&rigor, "engines") != "capi_v0145" && !skipped.contains(*case);
        let base = windgen_kvar_base(kw, pf, kva);
        if base == 0.0 {
            no_cell.push((case, pf));
            continue;
        }
        let row = rows
            .iter()
            .find(|r| r.rust.parse::<f64>().is_ok_and(|v| sig15(v) == sig15(base)))
            .unwrap_or_else(|| {
                panic!("{case}: no frozen census row spells the derived base {base}")
            });
        assert_eq!(
            row.r4133, "0",
            "{case}: r4133 must render the dispatched zero — that IS the divergence"
        );
        let e = per_spelling.entry(row.rust.as_str()).or_insert((0, 0));
        e.0 += cells;
        if in_scope {
            e.1 += cells;
            in_scope_cases.push((case, cells));
        }
    }
    in_scope_cases.sort_unstable();
    assert_eq!(
        no_cell.len(),
        1,
        "exactly one windgen deck derives a zero base, and it is the pair's only census-clean \
         one — got {no_cell:?}"
    );

    // (3) …and the products are the frozen census's own numbers, per spelling
    //     and in total.
    let ev = corpus
        .evidence(rows[0])
        .unwrap_or_else(|| panic!("{PAIR}: no frozen evidence record"));
    let cells: usize = per_spelling.values().map(|(c, _)| *c).sum();
    let in_scope_cells: usize = per_spelling.values().map(|(_, c)| *c).sum();
    assert_eq!(
        cells, ev.cells,
        "derived cells must be bins.tsv's cell count for {PAIR}"
    );
    assert_eq!(
        Some(in_scope_cells),
        ev.cells_in_scope,
        "derived in-scope cells must be bins.tsv's cells_in_scope for {PAIR}"
    );
    for row in &rows {
        assert_eq!(
            per_spelling.get(row.rust.as_str()).map(|(c, _)| *c),
            Some(row.cells),
            "'{}': the frozen example row's count must be the sum over the cases that derive it",
            row.rust
        );
    }

    // (4) One drafted entry per in-scope case, and no other.
    let mut entry_cases: Vec<&str> = LEDGER_ENTRY_PINS
        .iter()
        .filter(|(_, s, _)| *s == STEP)
        .map(|(_, _, cite)| {
            cite.split_once(" (")
                .and_then(|(_, rest)| rest.strip_suffix(')'))
                .unwrap_or_else(|| panic!("{cite:?}: the citation must name its case in parens"))
        })
        .collect();
    entry_cases.sort_unstable();
    assert_eq!(
        in_scope_cases.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        entry_cases,
        "exactly the in-scope diverging cases owe a drafted ledger entry — one each, and the \
         deck that derives no cell owes none"
    );

    // (5) The verdict carries the derived figures, so its prose cannot drift
    //     away from the corpus it describes.
    let verdict = RP3_ROUTING
        .iter()
        .find(|(p, ..)| *p == PAIR)
        .map(|(_, _, _, _, v)| *v)
        .unwrap_or_else(|| panic!("{PAIR} has no routing row"));
    let split = in_scope_cases
        .iter()
        .map(|(_, c)| c.to_string())
        .collect::<Vec<_>>()
        .join(" + ");
    let spellings = per_spelling
        .iter()
        .map(|(s, (c, _))| format!("'{s}' x{c}"))
        .collect::<Vec<_>>()
        .join(", ");
    let (clean_case, clean_pf) = no_cell[0];
    for phrase in [
        format!("{cells} cells, all {in_scope_cells} in scope"),
        format!("{split} over the {} diverging decks", in_scope_cases.len()),
        spellings,
        format!("{clean_case} derives kvar_base 0 from pf={clean_pf} and produces no cell"),
        format!(
            "{} further corpus decks declare a WindGen and are held in {}",
            RP32_WINDGEN_SKIPPED_DECKS.len(),
            SKIPPED_ORACLE_ISSUE
                .rsplit('/')
                .next()
                .expect("a file name")
        ),
        format!(
            "{in_scope_cells} in-scope cells over exactly {} cases",
            in_scope_cases.len()
        ),
    ] {
        assert!(
            verdict.contains(&phrase),
            "the windgen.kvar verdict must carry the derived census — {phrase:?} is missing from \
             {verdict:?}"
        );
    }
    assert!(
        names_identifier(
            verdict,
            "the_windgen_kvar_census_decomposition_is_read_off_the_corpus"
        ),
        "the verdict must name the test that derives its numbers, so a rename cannot orphan it"
    );
}

/// **The NCIM sweep's readers see every spelling the engine accepts**: the
/// self-test for [`algorithm_values`], [`selects_ncim`] and
/// [`redirect_targets`].
///
/// The corpus types the option one way only (`algorithm=` in full, never in a
/// redirected non-`.dss` script), so the widened readers are dormant on it, and
/// a dormant reader proves nothing about the completeness claim that rests on
/// it ([`the_rp33_census_decomposition_is_read_off_the_corpus`]'s first
/// assertion). Driving them here on the spellings r4133 itself accepts is what
/// makes that claim enforced rather than described: `set algo=ncim`,
/// `Set algorithm=nc`, and a `Set algorithm=NCIM` inside a `Redirect`ed `.txt`.
#[test]
fn the_ncim_sweep_reads_every_spelling_the_engine_accepts() {
    // The full spelling, and the two the old reader missed: an abbreviated
    // OPTION name (`TCommandList.GetCommand` -> `THashList.FindAbbrev`, a prefix
    // match) and an abbreviated VALUE (`InterpretSolveAlg` reads two chars).
    for line in [
        "Set algorithm=NCIM",
        "set algorithm=ncim",
        "Set algo=NCIM",
        "set a=ncim",
        "Set ALGORITHM=nc",
        "set algorithm=NCIMish",
        "Calcvoltagebases  set algorithm=ncim",
    ] {
        assert!(
            script_sets_ncim("<self-test>", line),
            "{line:?} selects NCIM in the engine and must select it here"
        );
    }
    // …and what must NOT be read as NCIM: another solver, a one-character value
    // (`copy(s, 1, 2)` cannot match `nc`), a comment line in either spelling,
    // and the option named inside a longer parameter.
    for line in [
        "Set algorithm=Newton",
        "set algorithm=n",
        "! set algorithm=ncim",
        "// Set algorithm=NCIM",
    ] {
        assert!(
            !script_sets_ncim("<self-test>", line),
            "{line:?} must not be read as an NCIM selection"
        );
    }
    assert!(selects_ncim("ncim") && selects_ncim("nc"));
    assert!(!selects_ncim("n") && !selects_ncim("") && !selects_ncim("newton"));
    // A live line that names the option in a spelling the reader cannot
    // attribute is a hard error — the branch that keeps a widened reader
    // honest, and deliberately over-loud: `myalgorithm=` is not the option
    // ([`named_token`] refuses it, which is why the value is not collected),
    // yet the mention still stops the sweep for a human to look at. Loud on a
    // spelling that turns out to be innocent is the cheap failure; silent on
    // one that is not is the one this guard exists to prevent.
    for line in ["Set algorithm ncim", "New Load.l1 myalgorithm=ncim"] {
        assert!(
            std::panic::catch_unwind(|| algorithm_values("<self-test>", line)).is_err(),
            "{line:?}: an unattributable live mention must be a hard error, not a silent skip"
        );
    }
    // The redirect reader, on the spellings the vendored corpus really writes.
    assert_eq!(
        redirect_targets(
            "Redirect \"../Version8/Distrib/Examples/Scripts/WireData.txt\"\n\
             Compile (Master.dss)\n\
             Redirect [Master_ckt5.dss]\n\
             redirect AllocationFactors_Base.Txt  !!! R=7 Vset=123\n\
             Redirect \"C:\\Program Files\\OpenDSS\\ckt5\\Master_ckt5.dss\"\n\
             ! Redirect commented_out.txt\n\
             New Line.l1 bus1=a"
        ),
        [
            "wiredata.txt",
            "master.dss",
            "master_ckt5.dss",
            "allocationfactors_base.txt",
            "master_ckt5.dss",
        ],
        "quoted, parenthesised, bracketed, comment-trailed and absolute targets all reduce to \
         their basename — the superset a completeness sweep needs"
    );
}

/// **The `generator.model` census decomposition, derived instead of
/// transcribed**: the same shape as
/// [`the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus`] and
/// [`the_windgen_kvar_census_decomposition_is_read_off_the_corpus`], applied to
/// `generator.model`.
///
/// The pair's artifacts are an echo row's `cells` and an
/// `ECHO_ROWS_ON_R4133_ONLY_CASES` exposure. So the decomposition must close over
/// every cell of the pair and every corpus deck that could add one:
///
/// * the **mechanism** is deck-level (only a deck that runs `Set algorithm=NCIM`
///   can move a generator's live `GenModel` off its typed token), so
///   [`sets_ncim`] is swept corpus-wide and [`RP33_NCIM_CASES`] +
///   [`RP33_NCIM_HELD_OUT`] must together be *every* such deck. The sweep's own
///   two escape hatches are closed: abbreviated option spellings are read as the
///   option ([`algorithm_values`]) and the value is matched the way r4133 matches
///   it ([`selects_ncim`], two characters), and the `.dss`-only universe is
///   completed by [`redirected_non_dss_scripts`], which sweeps every script a
///   deck `Redirect`s whatever its extension;
/// * the **element facts** come from the decks ([`generator_model_facts`]): how
///   many `Generator`s each declares and the `model=` they type, so
///   "one convertible generator each" is read rather than asserted, and
///   `ncim_pq.dss` (NCIM with no generator at all) is the control that shows
///   the mechanism alone produces no cell;
/// * the **echo claim itself is derived per case**: r4133's side of the frozen
///   census row must be the deck's own typed token, which IS what `EchoParse`
///   means for this pair, while our side is the converted live model `4`;
/// * `population.lock.json` gives each case's `steps=`/`engines=`, so
///   `cells = convertible generators × steps` and "in scope" is the lock's
///   answer. Every in-scope case is `engines=r4133` exactly, which is why the
///   pair's witness must be a pin and can never be a capi channel;
/// * the **held-out decks are named, not swept away**: both other NCIM decks run
///   `model=3` generators and are absent from the frozen census because they are
///   `kind: "large"`, i.e. outside the census population, asserted from the
///   lock, with their generators counted in the redirected file that declares
///   them;
/// * the **frozen census** (`bins.tsv` and `examples_full.txt`'s single `'4'`
///   vs `'3'` row) is what the products must add up to.
///
/// Then **all three** consumers are tied to the result: the shipped echo row's
/// `cells` column, its `ECHO_ROWS_ON_R4133_ONLY_CASES` exposure (both columns,
/// through `props_norm::r4133_only_exposure`) and the routing verdict, which
/// must carry the derived figures verbatim.
#[test]
fn the_rp33_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "generator.model";
    const STEP: &str = "generator-model";

    // (1) Completeness: the two tables ARE every corpus deck that runs NCIM.
    let root = repo_root().join(CORPUS);
    let mut decks = Vec::new();
    collect_dss(&root, &root, &mut decks);
    assert!(
        decks.len() > 1000,
        "only {} .dss files under {CORPUS} — the vendored corpus is missing",
        decks.len()
    );
    let measured: BTreeSet<String> = decks.iter().filter(|d| sets_ncim(d)).cloned().collect();
    let cited: BTreeSet<String> = RP33_NCIM_CASES
        .iter()
        .map(|(case, ..)| case_deck(case))
        .chain(RP33_NCIM_HELD_OUT.iter().map(|(case, ..)| case_deck(case)))
        .collect();
    assert_eq!(
        measured, cited,
        "every corpus deck that runs `Set algorithm=NCIM` must sit in the generator.model decomposition — a \
         new one is a deck whose generators can convert, i.e. a cell this pair does not account \
         for"
    );
    // …and the hatch underneath that sweep: the walk above sees `.dss` files,
    // while a deck can `Redirect` a script of any extension and the corpus
    // really does.
    let executed = redirected_non_dss_scripts(&decks);
    assert!(
        executed.len() >= 5,
        "the corpus redirects non-.dss scripts (WireData.txt, AllocationFactors_Base.Txt, the \
         LVTestCase parts, …) and this closure found only {} — an empty one would make the \
         assertion below vacuous and hand the extension hatch back",
        executed.len()
    );
    for f in &executed {
        assert!(
            !script_sets_ncim(f, &read_script(&root.join(f))),
            "{f}: a redirected non-.dss corpus script selects NCIM, so the deck sweep above — \
             which walks `.dss` files only — no longer sees every converting case, and the \
             `exactly 2 cells` closure below rests on nothing"
        );
    }

    // …and the element facts each census deck carries, read off the deck.
    let facts: Vec<(&str, usize, String)> = RP33_NCIM_CASES
        .iter()
        .map(|(case, declared, model)| {
            let (got_declared, got_model) = generator_model_facts(&case_deck(case));
            assert_eq!(
                (got_declared, got_model.as_str()),
                (*declared, *model),
                "{case}: the Generator count and `model=` token must be the deck's"
            );
            (*case, got_declared, got_model)
        })
        .collect();
    let no_generator: Vec<&str> = facts
        .iter()
        .filter(|(_, declared, _)| *declared == 0)
        .map(|(case, ..)| *case)
        .collect();
    assert_eq!(
        no_generator,
        ["modes:ncim/ncim_pq.dss"],
        "the control: an NCIM deck with no generator declared, hence no cell — the mechanism \
         alone does not make one"
    );

    // …and the held-out decks really are held out, by name and by kind, with the
    // convertible generators they would contribute if they were ever promoted.
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");
    for (case, sibling, declared, model) in RP33_NCIM_HELD_OUT {
        let rigor = case_rigor(&lock, case);
        assert_eq!(
            rigor_field(&rigor, "kind"),
            "large",
            "{case}: the census population is every live NON-LARGE case, so \
             a change of kind here puts its generators inside the frozen 2 and re-opens this \
             decomposition"
        );
        assert_eq!(
            generator_model_facts(&case_deck(case)).0,
            0,
            "{case}: its generators are declared in a redirected file, which is why the sibling \
             column exists — a master that declares them directly would make this reading wrong"
        );
        assert_eq!(
            generator_model_facts(sibling),
            (*declared, (*model).to_string()),
            "{sibling}: the redirected declarations behind {case}"
        );
        assert_eq!(
            *model, "3",
            "{case}: the hold-out only matters because these generators ARE convertible"
        );
    }

    // (2) Per case: cells = convertible generators × steps, in scope iff the case
    //     gates r4133 — and a cell exists only where the deck types `model=3`.
    let skipped = r4133_skipped_cases();
    let corpus = Corpus::load();
    let rows: Vec<&Example> = corpus.rows.iter().filter(|r| r.pair == PAIR).collect();
    let [row] = rows[..] else {
        panic!("the frozen census spells the pair exactly one way, got {rows:?}")
    };
    assert_eq!(
        (row.rust.as_str(), row.r4133.as_str()),
        ("4", "3"),
        "the divergence IS the converted live model against the deck's typed token"
    );
    let (mut cells, mut in_scope_cells) = (0usize, 0usize);
    let mut in_scope_cases: Vec<(&str, usize)> = Vec::new();
    for (case, declared, model) in &facts {
        if model.is_empty() {
            continue;
        }
        assert_eq!(
            model,
            row.r4133.as_str(),
            "{case}: r4133 renders the store, i.e. this deck's own `model=` token — that identity \
             is the EchoParse claim, so a deck typing anything else is a different mechanism"
        );
        assert_eq!(
            model, "3",
            "{case}: only a model-3 (PV) generator enters the PV->PQ conversion"
        );
        let rigor = case_rigor(&lock, case);
        let steps: usize = rigor_field(&rigor, "steps")
            .parse()
            .unwrap_or_else(|e| panic!("{case}: steps= is not a number: {e}"));
        let n = declared * steps;
        cells += n;
        assert_eq!(
            rigor_field(&rigor, "engines"),
            "r4133",
            "{case}: an r4133-ONLY case is what makes the pair's witness a pin — the capi channel \
             never value-compares these cells (props_norm::ECHO_ROWS_ON_R4133_ONLY_CASES)"
        );
        // The other half of the scope question, see `r4133_skipped_cases`: an
        // r4133-only case whose channel a ledger `skip` drops is compared by
        // NOBODY, and its cells could not be in scope at all.
        assert!(
            !skipped.contains(*case),
            "{case}: a ledger `skip` entry drops its only gating channel, so its cells are \
             compared by no engine — the exposure this derivation reports would be fiction"
        );
        in_scope_cells += n;
        in_scope_cases.push((case, n));
    }
    in_scope_cases.sort_unstable();

    // (3) …and the products are the frozen census's own numbers.
    let ev = corpus
        .evidence(row)
        .unwrap_or_else(|| panic!("{PAIR}: no frozen evidence record"));
    assert_eq!(
        cells, ev.cells,
        "derived cells must be bins.tsv's cell count for {PAIR}"
    );
    assert_eq!(
        Some(in_scope_cells),
        ev.cells_in_scope,
        "derived in-scope cells must be bins.tsv's cells_in_scope for {PAIR}"
    );
    assert_eq!(
        cells, row.cells,
        "…and the frozen example row's count must be the sum over the cases that derive it"
    );

    // (4) The consumers: the shipped echo row and its exposure, not a copy of
    //     them — this pair's exclusion is that row, so the derived 2 has to be
    //     the number the row cites.
    let (class, prop) = PAIR.split_once('.').expect("class.prop");
    let echo = props_norm::PROPS_ECHO_R4133
        .iter()
        .find(|r| r.class == class && r.prop == prop)
        .unwrap_or_else(|| panic!("{PAIR}: {STEP}'s ECHO outcome must have landed its row"));
    assert_eq!(
        echo.cells as usize, cells,
        "the echo row cites the derived cell count"
    );
    assert_eq!(
        echo.witness.pin(),
        Some("generator_model_renders_the_live_pv2pq_conversion"),
        "every cell is on an r4133-only case, so the row's witness can only be a pin"
    );
    // …and the row's SCOPE: narrowed to the one spelling derived above, so every
    // other `generator.model` cell on an r4133-only case (the agreeing ones of
    // the non-NCIM generator decks) is compared on r4133 instead of being masked
    // by a pair-wide row with no channel behind it.
    assert_eq!(
        props_norm::narrowed_spellings(class, prop),
        Some(&[(row.rust.as_str(), row.r4133.as_str())][..]),
        "props_norm::ECHO_NARROWED must hold {PAIR} to exactly its one derived spelling"
    );
    // …and the row's exposure entry, the second consumer. Both of its
    // `(cells, cases)` columns are this derivation's.
    assert_eq!(
        props_norm::r4133_only_exposure(class, prop),
        Some((cells as u32, in_scope_cases.len() as u32)),
        "props_norm::ECHO_ROWS_ON_R4133_ONLY_CASES must carry the derived (cells, cases) for \
         {PAIR} — every one of them is on an r4133-only case"
    );

    // (5) The verdict carries the derived figures, so its prose cannot drift
    //     away from the corpus it describes.
    let verdict = RP3_ROUTING
        .iter()
        .find(|(p, ..)| *p == PAIR)
        .map(|(_, _, _, _, v)| *v)
        .unwrap_or_else(|| panic!("{PAIR} has no routing row"));
    let split = in_scope_cases
        .iter()
        .map(|(_, c)| c.to_string())
        .collect::<Vec<_>>()
        .join(" + ");
    for phrase in [
        format!("{cells} cells, all {in_scope_cells} in scope"),
        format!("{split} over the {} converting decks", in_scope_cases.len()),
        in_scope_cases
            .iter()
            .map(|(c, _)| (*c).to_string())
            .collect::<Vec<_>>()
            .join(", "),
        format!("{} runs NCIM and declares no generator", no_generator[0]),
        format!(
            "the corpus's {} other NCIM decks are kind=large",
            RP33_NCIM_HELD_OUT.len()
        ),
    ] {
        assert!(
            verdict.contains(&phrase),
            "the generator.model verdict must carry the derived census — {phrase:?} is missing from \
             {verdict:?}"
        );
    }
    assert!(
        names_identifier(
            verdict,
            "the_rp33_census_decomposition_is_read_off_the_corpus"
        ),
        "the verdict must name the test that derives its numbers, so a rename cannot orphan it"
    );
}

/// **The `gictransformer.r2` census decomposition is read off the corpus**:
/// [`the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus`]'s shape, applied to
/// `gictransformer.r2` one element at a time.
///
/// The pair's conclusion, *exactly two* ledger entries, rests on a decomposition
/// that must close over **every** cell of the pair and every corpus file that
/// could add one:
///
/// * the **files** give the declarations and their resistance specs
///   ([`gictransformer_elements`]), swept over the whole corpus **file
///   universe** and not only its `.dss` files, so a class declared from a
///   `Redirect`ed script cannot hide from a completeness claim ([`collect_files`]
///   sees every file, redirected or not);
/// * the **branch** each element selects is read off its own tokens rather than
///   assumed: `%R1=`/`%R2=` select the percentage branch and `R1=`/`R2=` the ohms
///   branch, and an element that typed both would have to be adjudicated by
///   parse order, so the reader refuses one instead of guessing;
/// * the **arithmetic** derives *both* sides from those tokens (ours
///   `ZBase2*%R2/100`, the OpenDSS engines' `ZBase2*%R1/100`) and renders them at
///   the getter's own eight significant digits ([`sig8`]), so the frozen spelling
///   is *reconciled* and not transcribed;
/// * `population.lock.json` gives each case's `steps=`/`engines=`, so
///   `cells = diverging elements × steps`, and "in scope" is **both** halves of
///   the question: the lock's `engines=` must name r4133 *and* no ledger `skip`
///   entry may drop the channel ([`r4133_skipped_cases`]);
/// * the **frozen census** (`bins.tsv` and the single `examples_full.txt` row)
///   is what the products must add up to.
///
/// Every declaration lands in exactly **one** of three classes, each with its own
/// counter and its own assertion, so no element can fall through a `continue`
/// into a mismatch that gets reported as something else: `ohms` (the reverse
/// branch, no cell on any engine), `coincident` (`%R`-specified with
/// `%R1 == %R2`, where the slip is invisible, asserted empty) and `diverging` (a
/// cell), of which every one must sit on an r4133-gating case or the "one entry
/// per diverging element" conclusion no longer follows.
///
/// Two independent cross-checks then bound the same population from outside the
/// `%R` arithmetic: [`RP34_CLASS_WIDE_PAIRS`] (`Σ declared × steps`) and the
/// **positive measurement**, the ohms-specified GICTransformers on r4133-gating
/// cases that produce zero cells. Finally the consumers are tied to the result:
/// the in-scope diverging cases are exactly the cases [`LEDGER_ENTRY_PINS`]
/// cites (one entry each, no more), and the routing verdict must carry the
/// derived figures verbatim.
#[test]
fn the_gictransformer_r2_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "gictransformer.r2";
    const STEP: &str = "gictransformer-r2";

    // (1) Completeness: RP34_GIC_ELEMENTS IS every GICTransformer the corpus
    //     declares, in every file, with the spec each declaration types.
    let root = repo_root().join(CORPUS);
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files);
    assert!(
        files.len() > 1000,
        "only {} files under {CORPUS} — the vendored corpus is missing",
        files.len()
    );
    let measured: BTreeMap<String, Vec<(String, [String; 6])>> = files
        .iter()
        .map(|f| (f.clone(), gictransformer_elements(f)))
        .filter(|(_, elems)| !elems.is_empty())
        .collect();
    let mut cited: BTreeMap<String, Vec<(String, [String; 6])>> = BTreeMap::new();
    for decl in RP34_GIC_ELEMENTS {
        cited
            .entry(case_deck(decl.case))
            .or_default()
            .push((decl.name.to_string(), decl.tokens()));
    }
    assert_eq!(
        measured, cited,
        "every GICTransformer the corpus declares must sit in the gictransformer.r2 decomposition with its \
         measured %R1=/%R2=/R1=/R2=/kvll2=/mva= tokens — a new one changes how many ledger \
         entries the pair owes"
    );

    // (2) Per element: which branch of RecalcElementData it selects, and what
    //     the two engines then render. Per case: cells = diverging elements ×
    //     steps, in scope iff the case gates r4133.
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");
    let corpus = Corpus::load();
    let rows: Vec<&Example> = corpus.rows.iter().filter(|r| r.pair == PAIR).collect();
    assert_eq!(
        rows.len(),
        1,
        "the frozen census spells the pair exactly one way, got {rows:?}"
    );
    let row = rows[0];
    let num = |what: &str, case: &str, name: &str, token: &str| -> f64 {
        token
            .parse::<f64>()
            .unwrap_or_else(|e| panic!("{case} {name}: {what}={token} is not a number: {e}"))
    };

    // "In scope" is `engines=` AND the channel not being skipped — see
    // `r4133_skipped_cases`. The witness assertion keeps the second half honest:
    // if the reader ever stopped seeing the ledger's skip entries it would go
    // silently back to the `engines=`-only shorthand.
    let skipped = r4133_skipped_cases();
    const SKIP_WITNESS: &str = "asymmetric:line/line_spacing_asym.dss";
    assert!(
        skipped.contains(SKIP_WITNESS),
        "the ledger's r4133 `skip` set must still hold {SKIP_WITNESS} — the case that is \
         `engines: \"both\"` and yet never compared on r4133 (r4133-linespacing-asym-303, EPRI \
         #303), i.e. the counterexample this derivation's scope predicate exists for. If the \
         skip really went away, re-derive the gictransformer.r2 scope with it; got \
         {skipped:?}"
    );

    let mut cells = 0usize;
    let mut in_scope_cells = 0usize;
    let mut in_scope_cases: Vec<(String, String)> = Vec::new();
    let mut declared_steps = 0usize;
    let mut declared_steps_in_scope = 0usize;
    let mut ohms = 0usize;
    let mut ohms_in_scope = 0usize;
    let mut coincident: Vec<(&str, &str)> = Vec::new();
    let mut diverging: Vec<(&str, &str)> = Vec::new();
    for decl in RP34_GIC_ELEMENTS {
        let (case, name) = (decl.case, decl.name);
        let ((pct_r1, pct_r2), (r1, r2), (kvll2, mva)) = (decl.pct, decl.ohms, decl.base);
        let rigor = case_rigor(&lock, case);
        let steps: usize = rigor_field(&rigor, "steps")
            .parse()
            .unwrap_or_else(|e| panic!("{case}: steps= is not a number: {e}"));
        let in_scope = rigor_field(&rigor, "engines") != "capi_v0145" && !skipped.contains(case);
        declared_steps += steps;
        declared_steps_in_scope += usize::from(in_scope) * steps;

        let pct_spec = !pct_r1.is_empty() || !pct_r2.is_empty();
        let ohms_spec = !r1.is_empty() || !r2.is_empty();
        assert_ne!(
            pct_spec, ohms_spec,
            "{case} {name}: a GICTransformer that types both specs (or neither) does not select \
             one branch by its tokens — r4133 \
             resolves it by parse order, and this derivation refuses to guess"
        );
        if ohms_spec {
            // The reverse branch (`:497-498`), which neither revision ever got
            // wrong: the getter inverts back exactly what the deck typed, so
            // there is no cell here on any engine. This is the positive
            // measurement, not a skip.
            ohms += 1;
            ohms_in_scope += usize::from(in_scope);
            continue;
        }
        assert!(
            !pct_r1.is_empty() && !pct_r2.is_empty(),
            "{case} {name}: a deck typing only one percentage is the cause blob's blast-radius \
             shape — winding 2 would take the default 0.2 \
             instead of repeating %R1, and this pair's two-cell conclusion would have to be \
             re-derived"
        );
        assert!(
            !kvll2.is_empty() && !mva.is_empty(),
            "{case} {name}: a %R-specified element that leaves kvll2=/mva= to the creation \
             defaults renders off a base this derivation does not read"
        );
        let z_base2 = num("kvll2", case, name, kvll2).powi(2) / num("mva", case, name, mva);
        let ours = z_base2 * num("%R2", case, name, pct_r2) / 100.0;
        let theirs = z_base2 * num("%R1", case, name, pct_r1) / 100.0;
        if sig8(ours) == sig8(theirs) {
            // `%R1 == %R2` makes the slip invisible: the same value by
            // coincidence, exactly as `windgen_snap.dss`'s `pf=1.0` is for
            // `windgen.kvar`, whose `no_cell` bucket this mirrors. It is a THIRD
            // class, not a silent skip: counted here and adjudicated at (4b).
            coincident.push((case, name));
            continue;
        }
        assert_eq!(
            (sig8(ours), sig8(theirs)),
            (
                sig8(
                    row.rust
                        .parse()
                        .expect("the frozen rust spelling is a number")
                ),
                sig8(
                    row.r4133
                        .parse()
                        .expect("the frozen r4133 spelling is a number")
                ),
            ),
            "{case} {name}: the derived pair must be the frozen census spelling at the getter's \
             own 8 significant digits — ours ZBase2*%R2/100, the OpenDSS engines' \
             ZBase2*%R1/100"
        );
        diverging.push((case, name));
        cells += steps;
        if in_scope {
            in_scope_cells += steps;
            in_scope_cases.push((case.to_string(), name.to_string()));
        }
    }
    in_scope_cases.sort();

    // (3) …and the products are the frozen census's own numbers.
    let ev = corpus
        .evidence(row)
        .unwrap_or_else(|| panic!("{PAIR}: no frozen evidence record"));
    assert_eq!(
        cells, ev.cells,
        "derived cells must be bins.tsv's cell count for {PAIR}"
    );
    assert_eq!(
        Some(in_scope_cells),
        ev.cells_in_scope,
        "derived in-scope cells must be bins.tsv's cells_in_scope for {PAIR}"
    );
    assert_eq!(
        cells, row.cells,
        "the single frozen example row's count must be the sum over the elements that derive it"
    );

    // (4a) The first cross-check, which owes nothing to the `%R` arithmetic: the
    //      class-wide pairs are answered by EVERY declaration on every step.
    for (pair, want_cells, want_in_scope) in RP34_CLASS_WIDE_PAIRS {
        let evidence = corpus
            .bins
            .get(*pair)
            .and_then(|all| all.first())
            .unwrap_or_else(|| panic!("{pair}: no bins.tsv record"));
        assert_eq!(
            (evidence.cells, evidence.cells_in_scope),
            (*want_cells, Some(*want_in_scope)),
            "{pair}: the pinned class-wide split moved"
        );
        assert_eq!(
            (declared_steps, declared_steps_in_scope),
            (*want_cells, *want_in_scope + RP34_IN_SCOPE_MOVED_BY_G14A),
            "{pair} is answered by every GICTransformer on every step, so its census split must \
             be the derived sum(declared x steps) over the whole corpus and over the r4133-gating \
             cases, plus the one named channel move — a mismatch means the population moved under the two-cell conclusion"
        );
    }

    // (4b) The positive measurement: the ohms spec is the majority of the
    //      population and it diverges nowhere. The three classes are
    //      adjudicated one by one, each naming its own cause, so a corpus that
    //      grows an element reds where the reason is.
    assert!(
        coincident.is_empty(),
        "a %R-specified GICTransformer whose two percentages coincide renders the same number \
         on both engines, so it carries no cell and owes no ledger entry — today's corpus has \
         none, and one appearing means the ohms/coincident/diverging split (and the two-cell \
         conclusion resting on it) must be re-derived, not merely re-counted: {coincident:?}"
    );
    assert_eq!(
        ohms + coincident.len() + diverging.len(),
        RP34_GIC_ELEMENTS.len(),
        "every GICTransformer must land in exactly one class of the decomposition — \
         ohms-specified, %R with coinciding percentages, or %R-diverging"
    );
    assert_eq!(
        diverging.len(),
        in_scope_cases.len(),
        "every diverging %R declaration must sit on a case r4133 really gates: one on a \
         capi-only (or `skip`ped) case would carry a census cell that no r4133 comparison can \
         reach, and the \"one entry per diverging element\" conclusion would no \
         longer follow — {diverging:?} against the in-scope {in_scope_cases:?}"
    );
    assert!(
        ohms_in_scope > 0,
        "the ohms path must be exercised on r4133-gating cases, or its zero cells prove nothing \
         about the r4133 channel"
    );

    // (5) One drafted entry per in-scope diverging case, and no other.
    let mut entry_cases: Vec<&str> = LEDGER_ENTRY_PINS
        .iter()
        .filter(|(_, s, _)| *s == STEP)
        .map(|(_, _, cite)| {
            cite.split_once(" (")
                .and_then(|(_, rest)| rest.strip_suffix(')'))
                .unwrap_or_else(|| panic!("{cite:?}: the citation must name its case in parens"))
        })
        .collect();
    entry_cases.sort_unstable();
    assert_eq!(
        in_scope_cases
            .iter()
            .map(|(c, _)| c.as_str())
            .collect::<Vec<_>>(),
        entry_cases,
        "exactly the in-scope diverging cases owe a drafted ledger entry — one each, and the \
         ohms-specified elements owe none"
    );

    // (6) The verdict carries the derived figures, so its prose cannot drift
    //     away from the corpus it describes.
    let verdict = RP3_ROUTING
        .iter()
        .find(|(p, ..)| *p == PAIR)
        .map(|(_, _, _, _, v)| *v)
        .unwrap_or_else(|| panic!("{PAIR} has no routing row"));
    let split = in_scope_cases
        .iter()
        .map(|_| "1".to_string())
        .collect::<Vec<_>>()
        .join(" + ");
    let (cw_cells, cw_in_scope) = (declared_steps, declared_steps_in_scope);
    for phrase in [
        format!("{cells} cells, all {in_scope_cells} in scope"),
        format!("{split} over the {} %R decks", in_scope_cases.len()),
        in_scope_cases
            .iter()
            .map(|(c, n)| format!("{c} {n}"))
            .collect::<Vec<_>>()
            .join(", "),
        format!("'{}'", row.rust),
        format!("'{}'", row.r4133),
        format!(
            "{in_scope_cells} in-scope cells over exactly {} cases",
            in_scope_cases.len()
        ),
        format!("other {ohms} GICTransformers"),
        format!("{ohms_in_scope} of them on r4133-gating cases"),
        format!("{cw_cells} cells / {cw_in_scope} in scope"),
        RP34_CLASS_WIDE_PAIRS[0].0.to_string(),
        RP34_CLASS_WIDE_PAIRS[1].0.to_string(),
    ] {
        assert!(
            verdict.contains(&phrase),
            "the gictransformer.r2 verdict must carry the derived census — {phrase:?} is missing from \
             {verdict:?}"
        );
    }
    assert!(
        names_identifier(
            verdict,
            "the_gictransformer_r2_census_decomposition_is_read_off_the_corpus"
        ),
        "the verdict must name the test that derives its numbers, so a rename cannot orphan it"
    );
}

/// **The GICTransformer reader really separates the two resistance specs**: the
/// self-test for [`gictransformer_elements`] and [`GIC_KEYS`].
///
/// The whole decomposition turns on one distinction: `%R1=0.2` must read as a
/// *percentage* token and `R1=0.2` as an *ohms* one, because that selects the
/// branch an element takes and therefore whether it carries a census cell. The
/// two keys are prefixes of one another, so a reader that got the separator
/// rule wrong would silently classify every `%R` element as ohms-specified, and
/// the answer would become "zero cells, no ledger entry", green and wrong.
///
/// It is asserted on the real decks rather than on a fixture: both `gic/*` decks
/// carry both spellings, an element that types `R1=` and no `R2=` (the GSU), and
/// a `~` continuation that types the `%R` element's bases on the *next* line.
#[test]
fn the_gictransformer_reader_separates_the_percentage_and_ohms_specs() {
    let elems = gictransformer_elements("asymmetric/gic/gictransformer_gic.dss");
    assert_eq!(
        elems.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
        ["tg1", "tg2", "tg3"],
        "the reader must see every declaration, in deck order"
    );
    let [pct_r1, pct_r2, r1, r2, kvll2, mva] = &elems[2].1;
    assert_eq!(
        (
            pct_r1.as_str(),
            pct_r2.as_str(),
            r1.as_str(),
            r2.as_str(),
            kvll2.as_str(),
            mva.as_str()
        ),
        ("0.2", "0.15", "", "", "138", "300"),
        "tg3 types percentages only, and its bases arrive on the `~` continuation line — an ohms \
         key must NOT match inside `%R1=`"
    );
    let [pct_r1, pct_r2, r1, r2, ..] = &elems[1].1;
    assert_eq!(
        (pct_r1.as_str(), pct_r2.as_str(), r1.as_str(), r2.as_str()),
        ("", "", "0.2", "0.1"),
        "tg2 types ohms only — a percentage key must NOT match a bare `R1=`"
    );
    assert_eq!(
        elems[0].1[3], "",
        "a GSU types no R2= at all, and the reader must not borrow tg2's"
    );
    // …and the scope really closes: the Reactors below the transformers type
    // `r=`/`x=` of their own, and no `~` continues a GICTransformer past them.
    assert_eq!(
        gictransformer_elements("asymmetric/gic/gic_midi.dss")
            .iter()
            .map(|(n, _)| n.as_str())
            .collect::<Vec<_>>(),
        ["tg1", "tg3", "tg5"]
    );
}

/// The corpus decks that actually run `Reduce`, i.e. the whole population in
/// which a line merge can ever execute.
///
/// Cited rather than derived-and-forgotten so that a new reduce deck reds
/// [`the_line_units_census_decomposition_is_read_off_the_corpus`] instead of quietly
/// widening the pair's population. The other two columns are the two halves of
/// "can this deck carry a `line.units` cell", each read off the deck itself and
/// each re-derived by the test:
///
/// * the `Set ReduceOption=` it selects ([`reduce_strategy`]), which decides
///   whether a merge runs at all ([`RP35_MERGING_STRATEGIES`]);
/// * whether it declares a line that is **not** a 3-phase symmetrical-components
///   line ([`declares_a_non_sym3_line`]), the exact negation of the merge's
///   sym-branch test and therefore the necessary condition for a matrix-branch
///   merge, the only branch that carried the divergence.
///
/// Both halves are necessary and neither is sufficient alone, which is exactly
/// what the corpus shows: `reduce_laterals` declares 1-phase laterals but runs
/// `DoRemoveBranches`, and `reduce_mergeparallel` merges but only 3-phase sym
/// lines.
const RP35_REDUCE_DECKS: &[(&str, &str, bool)] = &[
    ("modes:reduce/midi_reduce.dss", "default", true),
    ("modes:reduce/reduce_breakloop.dss", "breakloop", false),
    ("modes:reduce/reduce_dangling.dss", "ends", false),
    ("modes:reduce/reduce_default.dss", "default", false),
    ("modes:reduce/reduce_keeplist.dss", "default", false),
    ("modes:reduce/reduce_laterals.dss", "laterals", true),
    (
        "modes:reduce/reduce_mergeparallel.dss",
        "mergeparallel",
        false,
    ),
    ("modes:reduce/reduce_shortlines.dss", "shortlines", false),
    ("modes:reduce/reduce_switches.dss", "switches", false),
];

/// The reduction strategies whose procedure calls `TLineObj.MergeWith` at all —
/// `DoMergeParallelLines` (`Meters/ReduceAlgs.pas:54`), `DoReduceShortLines`
/// (`:214`, `:258`), `DoReduceSwitches` (`:319`) and `DoReduceDefault` (`:361`).
/// The rest of the strategies only disable branches — `DoBreakLoops` (`:61`),
/// `DoReduceDangling` (`:101`), `DoReduceTapEnds` (`:87`) and the lateral
/// removal `DoRemoveBranches`/`DoRemoveAll_1ph_Laterals` (`:372`, `:453`) — so
/// they can no more move `line.units` than they can rename a line.
const RP35_MERGING_STRATEGIES: &[&str] = &["default", "mergeparallel", "shortlines", "switches"];

/// The `Set ReduceOption=` a deck selects, lowercased — the strategy `Reduce`
/// then dispatches on.
fn reduce_strategy(rel: &str) -> String {
    let path = repo_root().join(CORPUS).join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut found = String::new();
    for line in text.lines() {
        let low = line.to_ascii_lowercase();
        for tok in low.split_whitespace() {
            if let Some(v) = tok.strip_prefix("reduceoption=") {
                found = v.to_string();
            }
        }
    }
    found
}

/// The one corpus file that spells every DSS command and is not a script:
/// OpenDSS's editor syntax-highlight keyword list, one command name per line —
/// including a bare `Reduce`. Carved out by name (not by extension class) so the
/// exemption cannot widen silently; [`the_line_units_census_decomposition_is_read_off_the_corpus`]
/// asserts it is still the only `.stx` under the corpus.
const RP35_NON_SCRIPT: &str = "electricdss-tst/Version8/Distrib/Examples/SyntaxFiles/opendss.stx";

/// Does this corpus file issue a bare `Reduce` command? `Set ReduceOption=…`
/// alone reduces nothing, so the first whitespace token of a non-comment line is
/// the only thing that counts.
fn runs_reduce(rel: &str) -> bool {
    if rel == RP35_NON_SCRIPT {
        return false;
    }
    let path = repo_root().join(CORPUS).join(rel);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return false; // a binary or non-UTF-8 fixture cannot carry a command
    };
    text.lines().any(|line| {
        let l = line.trim();
        !l.starts_with('!')
            && !l.starts_with("//")
            && l.split_whitespace()
                .next()
                .is_some_and(|t| t.eq_ignore_ascii_case("reduce"))
    })
}

/// Can any merge in this deck take r4133's **matrix** branch? Read off the
/// deck's own declarations: a merge takes the symmetrical-components branch only
/// when both lines are `SymComponentsModel` **and** the survivor has 3 phases
/// (`Line.pas:1695`), so a deck all of whose lines are 3-phase sym lines can
/// only ever take that branch. A line is NOT a 3-phase sym line when it (or the
/// LineCode it names) types an `rmatrix`/`xmatrix`/`cmatrix` (`FetchLineCode`
/// leaves `SymComponentsModel := False` for a matrix code, `Line.pas:395-404`;
/// the `12..14` side effect does the same, `:691`) or when its phase count is
/// not 3.
fn declares_a_non_sym3_line(rel: &str) -> bool {
    let path = repo_root().join(CORPUS).join(rel);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let decl = |l: &str, kind: &str| -> Option<String> {
        let l = l.trim().to_ascii_lowercase();
        let rest = l.strip_prefix("new ")?;
        let rest = rest.trim_start().strip_prefix(kind)?.strip_prefix('.')?;
        Some(
            rest.split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string(),
        )
    };
    let token = |l: &str, key: &str| -> Option<String> {
        let l = l.to_ascii_lowercase();
        let want = format!("{key}=");
        l.split_whitespace()
            .find_map(|t| t.strip_prefix(want.as_str()).map(str::to_string))
    };
    let has_matrix = |l: &str| {
        let low = l.to_ascii_lowercase();
        ["rmatrix=", "xmatrix=", "cmatrix="]
            .iter()
            .any(|k| low.contains(k))
    };
    // LineCodes first: a line inherits its code's model and phase count.
    let mut matrix_codes: BTreeSet<String> = BTreeSet::new();
    let mut narrow_codes: BTreeSet<String> = BTreeSet::new();
    for line in text.lines() {
        let Some(name) = decl(line, "linecode") else {
            continue;
        };
        if has_matrix(line) {
            matrix_codes.insert(name.clone());
        }
        if token(line, "nphases").is_some_and(|v| v != "3") {
            narrow_codes.insert(name);
        }
    }
    text.lines().any(|line| {
        decl(line, "line").is_some()
            && (has_matrix(line)
                || token(line, "phases").is_some_and(|v| v != "3")
                || token(line, "linecode")
                    .is_some_and(|c| matrix_codes.contains(&c) || narrow_codes.contains(&c)))
    })
}

/// **The `line.units` census decomposition is read off the corpus**:
/// [`the_gictransformer_r2_census_decomposition_is_read_off_the_corpus`]'s shape, applied to
/// `line.units`.
///
/// The pair's two load-bearing numbers are its cells and its in-scope cells
/// (none). `reduce_mergeparallel` carries **zero** `line.units` cells because
/// its merge is a 3-phase symmetrical-components merge and all three engines
/// render `km`, so the derivation asserts that counter-claim explicitly and not
/// merely the total.
///
/// It closes over the whole corpus in four steps:
///
/// * the **population**: every file (not only every `.dss` file) that issues a
///   bare `Reduce` ([`runs_reduce`]), the only way a merge runs at all. That set
///   must be exactly [`RP35_REDUCE_DECKS`], so a new reduce deck reds here;
/// * the **branch**: whether a deck can reach the matrix branch is read off its
///   own declarations ([`declares_a_non_sym3_line`]), not assumed. Exactly one
///   deck can, `midi_reduce`, whose merges are the census cells, and
///   `reduce_mergeparallel` is asserted to be on the other side of that line;
/// * the **scope**: each case's `steps=`/`engines=` come from
///   `population.lock.json`, and the r4133 channel must not be `skip`ped
///   ([`r4133_skipped_cases`]). Three reduce decks do gate r4133
///   (`reduce_breakloop`, `reduce_dangling`, `reduce_laterals`), so the zero is
///   not a statement about the channel. None of them can carry a cell, which is
///   *why* the pair has no in-scope cell and its exclusion is a **capi** ledger
///   entry;
/// * the **reconciliation**: the derived totals must be `bins.tsv`'s and
///   `examples_full.txt`'s own numbers for the pair.
///
/// **The frozen `rust` column predates the fix.** The extracts under
/// `tests/corpus/props_r4133/` are a *data* lock
/// (`props_r4133_evidence_lock.rs`): this engine answers `kft`, the r4133
/// spelling, where the frozen row records `'none'`. The row is read here as
/// evidence of what was measured, never as a claim about today's engine. That
/// claim is `exec::tests::reduce::the_corpus_reduce_decks_merged_lines_render_kft`.
#[test]
fn the_line_units_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "line.units";

    // (1) Population: every corpus file that runs `Reduce`.
    let root = repo_root().join(CORPUS);
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files);
    assert!(
        files.len() > 1000,
        "only {} files under {CORPUS} — the vendored corpus is missing",
        files.len()
    );
    assert_eq!(
        files
            .iter()
            .filter(|f| f.ends_with(".stx"))
            .collect::<Vec<_>>(),
        [RP35_NON_SCRIPT],
        "the non-script carve-out must stay a single named file"
    );
    let mut measured: Vec<String> = files
        .iter()
        .filter(|f| runs_reduce(f))
        .map(|f| {
            let (family, rel) = f.split_once('/').expect("a corpus path has a family dir");
            if family == "electricdss-tst" {
                format!("solvable_now:{rel}")
            } else {
                format!("{family}:{rel}")
            }
        })
        .collect();
    measured.sort();
    let mut cited: Vec<String> = RP35_REDUCE_DECKS
        .iter()
        .map(|(c, ..)| (*c).to_string())
        .collect();
    cited.sort();
    assert_eq!(
        measured, cited,
        "every corpus file that issues `Reduce` must sit in the line.units decomposition — a new one \
         changes the population in which MergeWith runs, and with it how many `line.units` cells \
         the pair can carry"
    );

    // (2) The two halves of "can carry a cell", each read off the deck.
    for (case, strategy, non_sym3) in RP35_REDUCE_DECKS {
        let deck = case_deck(case);
        assert_eq!(
            reduce_strategy(&deck),
            *strategy,
            "{case}: the strategy decides whether MergeWith runs at all"
        );
        assert_eq!(
            declares_a_non_sym3_line(&deck),
            *non_sym3,
            "{case}: whether a merge here could take the MATRIX branch (the negation of              the sym-branch test) is the other half of whether it can carry a `line.units` cell"
        );
    }
    let can_carry = |(_, strategy, non_sym3): &(&str, &str, bool)| {
        *non_sym3 && RP35_MERGING_STRATEGIES.contains(strategy)
    };
    let matrix_decks: Vec<&str> = RP35_REDUCE_DECKS
        .iter()
        .filter(|d| can_carry(d))
        .map(|(c, ..)| *c)
        .collect();
    assert_eq!(
        matrix_decks,
        ["modes:reduce/midi_reduce.dss"],
        "all three `line.units` cells are on the one deck that both merges and can take the          matrix branch — `reduce_laterals` declares 1-phase laterals but its strategy only          removes branches, and `reduce_mergeparallel` merges but only          3-phase symmetrical-components lines"
    );
    let mergeparallel = RP35_REDUCE_DECKS
        .iter()
        .find(|(c, ..)| *c == "modes:reduce/reduce_mergeparallel.dss")
        .expect("reduce_mergeparallel is in the table");
    assert!(
        !can_carry(mergeparallel),
        "the counter-claim: `reduce_mergeparallel` merges two 3-phase          symmetrical-components lines, takes the SYM branch, renders `km` on all three engines          and contributes ZERO `line.units` cells"
    );

    // (3) Scope: `engines=` AND the r4133 channel not being `skip`ped.
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");
    let skipped = r4133_skipped_cases();
    const SKIP_WITNESS: &str = "asymmetric:line/line_spacing_asym.dss";
    assert!(
        skipped.contains(SKIP_WITNESS),
        "the ledger's r4133 `skip` set must still hold {SKIP_WITNESS} — the counterexample the \
         scope predicate exists for (`engines: \"both\"` and yet never compared on r4133); got \
         {skipped:?}"
    );
    let mut cells = 0usize;
    let mut in_scope_cells = 0usize;
    let mut r4133_gated = 0usize;
    for deck in RP35_REDUCE_DECKS {
        let case = &deck.0;
        let rigor = case_rigor(&lock, case);
        let steps: usize = rigor_field(&rigor, "steps")
            .parse()
            .unwrap_or_else(|e| panic!("{case}: steps= is not a number: {e}"));
        let in_scope = rigor_field(&rigor, "engines") != "capi_v0145" && !skipped.contains(*case);
        r4133_gated += usize::from(in_scope);
        assert!(
            !(in_scope && can_carry(deck)),
            "{case}: a reduce deck that BOTH gates r4133 and can take the matrix branch would              carry an in-scope `line.units` cell — the pair would stop being 0-in-scope, and              with it the case for a `capi_v0145` ledger entry instead of an r4133 entry"
        );
        if !can_carry(deck) {
            continue;
        }
        // `midi_reduce` merges three matrix pairs — `l2a~l2b`, `l3a~l3b` and
        // `bb14_15~l9a` — one cell each per step. The three merges themselves
        // are pinned on the engine side by
        // `exec::tests::reduce::the_corpus_reduce_decks_merged_lines_render_kft`.
        cells += 3 * steps;
        in_scope_cells += usize::from(in_scope) * 3 * steps;
    }
    assert!(
        r4133_gated > 0,
        "the r4133 channel must really gate some reduce deck, or `0 in-scope cells` would be a          statement about the channel rather than about this pair"
    );

    // (4) …and the products are the frozen census's own numbers.
    let corpus = Corpus::load();
    let rows: Vec<&Example> = corpus.rows.iter().filter(|r| r.pair == PAIR).collect();
    assert_eq!(
        rows.len(),
        1,
        "the frozen census spells the pair exactly one way, got {rows:?}"
    );
    let row = rows[0];
    assert_eq!(
        (row.rust.as_str(), row.r4133.as_str()),
        ("none", "kft"),
        "the frozen census — its `rust` side predates the fix, and the extracts are \
         never edited"
    );
    assert_eq!(
        cells, row.cells,
        "derived cells must be the frozen example row's count for {PAIR}"
    );
    let ev = corpus
        .evidence(row)
        .unwrap_or_else(|| panic!("{PAIR}: no frozen evidence record"));
    assert_eq!(
        (cells, Some(in_scope_cells)),
        (ev.cells, ev.cells_in_scope),
        "derived cells / in-scope cells must be bins.tsv's for {PAIR}"
    );
    assert_eq!(in_scope_cells, 0, "line.units has no in-scope cell");

    // (5) The counter-claim from the other side: `reduce_mergeparallel` types
    //     `units=km` throughout, so a cell there would have to spell `km` — and
    //     the frozen census holds no such row.
    assert!(
        !corpus
            .rows
            .iter()
            .any(|r| r.pair == PAIR && (r.rust == "km" || r.r4133 == "km")),
        "a `line.units` row spelling `km` would mean the SYM branch diverges too — which the \
         live probe disproves"
    );
}

/// Every corpus file that declares a `Line` carrying **both** a `linecode=` and
/// a `switch=` (the whole population in which the `line.linecode` divergence can
/// fire), with the two counts the arithmetic rests on: how many of those
/// declarations type `switch=` **after** the code, and how many before.
///
/// The order is the mechanism, not a detail. The parser runs each property's
/// side effect in the order its token is typed, so `linecode=… switch=…` sets
/// the code and arms the line-code flag, and *then* runs the `switch=` arm.
/// `switch=… linecode=…` runs the two the other way round, so the code re-arms
/// the flag last and all three engines agree. That is why
/// `LVTestCaseNorthAmerican`'s `switch=yes linecode=switch` declarations carry
/// no cell, and it is pinned on the engine side by the `swb` leg of
/// `exec::tests::line_fetch::switch_yes_keeps_the_linecode_and_its_units_conversion`.
///
/// Cited rather than derived-and-forgotten, so a new deck of this shape reds
/// [`the_line_linecode_census_decomposition_is_read_off_the_corpus`] instead of quietly
/// widening the pair's population. Columns: the corpus-relative file, the
/// declarations with `switch=` after the code, and those with it before.
const RP36_SWITCH_LINECODE_DECKS: &[(&str, usize, usize)] = &[
    (
        "electricdss-tst/Version8/Distrib/EPRITestCircuits/ckt7/Lines_ckt7.dss",
        11,
        0,
    ),
    (
        "electricdss-tst/Version8/Distrib/Examples/ADiakoptics/EPRI_Ckt7-G/Lines_ckt7.dss",
        11,
        0,
    ),
    (
        "electricdss-tst/Version8/Distrib/Examples/ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/Line.DSS",
        4,
        0,
    ),
    (
        "electricdss-tst/Version8/Distrib/Examples/ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_2/Branches.dss",
        3,
        0,
    ),
    (
        "electricdss-tst/Version8/Distrib/Examples/ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_3/Branches.dss",
        2,
        0,
    ),
    (
        "electricdss-tst/Version8/Distrib/Examples/StoCtrl_Current_PeakShave/Line.DSS",
        9,
        0,
    ),
    (
        "electricdss-tst/Version8/Distrib/IEEETestCases/LVTestCaseNorthAmerican/Master.dss",
        0,
        80,
    ),
    (
        "electricdss-tst/Version8/Distrib/IEEETestCases/LVTestCaseNorthAmerican/SecPar.dss",
        0,
        80,
    ),
];

/// The two cases that carry all the `line.linecode` cells, with their counts
/// (`zone_2/Branches.dss` and `zone_3/Branches.dss`). A third deck of the same
/// shape carries none, see [`RP36_STOCTRL_CASE`].
const RP36_CELL_CASES: &[(&str, usize)] = &[
    (
        "solvable_now:Version8/Distrib/Examples/ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_2/master.dss",
        3,
    ),
    (
        "solvable_now:Version8/Distrib/Examples/ADiakoptics/EPRI_Ckt7-G/Torn_Circuit/zone_3/master.dss",
        2,
    ),
];

/// A deck of the right shape with zero cells, because its case is `kind=large`
/// and `force_properties` never turns a large case's property compare on.
const RP36_STOCTRL_CASE: &str =
    "solvable_now:Version8/Distrib/Examples/StoCtrl_Current_PeakShave/master.dss";

/// The declaring file behind [`RP36_STOCTRL_CASE`], read by the counter-claim so
/// that its zero is proven to come from the case's rigor and not from the deck
/// lacking the shape.
const RP36_STOCTRL_DECK: &str =
    "electricdss-tst/Version8/Distrib/Examples/StoCtrl_Current_PeakShave/Line.DSS";

/// The `Line` declarations of one script that carry both a `linecode=` and a
/// `switch=`, split by which token comes first: `(switch after the code, switch
/// before it)`. `New` and `Edit` alike — an `Edit Line.x switch=yes` runs the
/// same side-effect arm — and the quoted object names the torn-circuit decks use
/// (`New "Line.261249" …`) are read.
fn switch_linecode_line_decls(text: &str) -> (usize, usize) {
    let (mut after, mut before) = (0usize, 0usize);
    for line in text.lines() {
        let low = line.trim().to_ascii_lowercase();
        let Some(rest) = ["new ", "edit "].iter().find_map(|v| low.strip_prefix(*v)) else {
            continue;
        };
        if !rest
            .trim_start()
            .trim_start_matches(['"', '\''])
            .starts_with("line.")
        {
            continue;
        }
        match (low.find("linecode="), low.find("switch=")) {
            (Some(code), Some(sw)) if sw > code => after += 1,
            (Some(_), Some(_)) => before += 1,
            _ => {}
        }
    }
    (after, before)
}

/// Continuation lines (`~ …`) carrying a `linecode=` or a `switch=`. The
/// declaration sweep reads one physical line at a time, so a corpus that grew a
/// continued `Line` declaration would slip past it; the decomposition asserts
/// this is zero everywhere rather than assuming it.
fn continued_line_tokens(text: &str) -> usize {
    text.lines()
        .filter(|l| {
            let l = l.trim();
            let low = l.to_ascii_lowercase();
            l.starts_with('~') && (low.contains("linecode=") || low.contains("switch="))
        })
        .count()
}

/// One script's `Redirect`/`Compile` targets as corpus-relative paths, resolved
/// against the directory of the file that names them — the executive's own rule.
/// Returns `(resolved, unresolved)`.
///
/// [`redirect_targets`]' basename form is a deliberate superset: safe for a
/// completeness sweep, useless here. `Line.DSS` and `Master.dss` are each the
/// basename of several corpus decks, so a basename match would attribute one
/// deck's declarations to another deck's case and inflate the cell count.
/// Everything the exact resolution cannot place is handed back rather than
/// dropped, so [`the_line_linecode_census_decomposition_is_read_off_the_corpus`] asserts
/// what is left over instead of trusting it.
fn redirect_paths(text: &str, rel: &str, root: &std::path::Path) -> (Vec<String>, Vec<String>) {
    let parent = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let (mut resolved, mut unresolved) = (Vec::new(), Vec::new());
    for arg in redirect_args(text) {
        let mut parts: Vec<&str> = Vec::new();
        for seg in parent.split('/').chain(arg.split('/')) {
            match seg {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                s => parts.push(s),
            }
        }
        let path = parts.join("/");
        if !path.is_empty() && root.join(&path).is_file() {
            resolved.push(path);
        } else {
            unresolved.push(arg);
        }
    }
    (resolved, unresolved)
}

/// The one `Redirect` argument in the whole corpus that resolves to no file: a
/// template placeholder the memory-mapped-loadshape example substitutes before
/// it runs. Named so that a *second* unresolvable target — which could hide a
/// declaring deck from an ownership walk — reds instead of passing silently.
const RP36_TEMPLATE_REDIRECT: (&str, &str) = (
    "electricdss-tst/Version8/Distrib/Examples/MemoryMappingLoadShapes/ckt24/main_template.dss",
    "@loadshape_script_dss",
);

/// **The `line.linecode` census decomposition is read off the corpus**:
/// [`the_line_units_census_decomposition_is_read_off_the_corpus`]' shape, applied to
/// `line.linecode`.
///
/// The pair's two load-bearing numbers are its cells and that all of them are
/// in scope: `line.linecode` is the one live-state pair the r4133 compare
/// reaches. `Examples/StoCtrl_Current_PeakShave/Line.DSS` is a deck of exactly
/// the right shape (`linecode=… Switch=True units=m` declarations) that
/// contributes **zero** cells, because its case is `kind=large` and
/// `force_properties` (`crates/dss-core/tests/corpus_gate/scheduler.rs`) never
/// turns a large case's property compare on, so the derivation asserts that
/// counter-claim explicitly and not merely the total.
///
/// Five steps, closing over the whole corpus:
///
/// * the **population**: one pass over every corpus file (not only every `.dss`
///   file) collecting the qualifying `Line` declarations
///   ([`switch_linecode_line_decls`]), the continuation lines that would defeat a
///   one-physical-line sweep ([`continued_line_tokens`]) and the
///   `Redirect`/`Compile` edges the ownership walk needs. The declaring files
///   must be exactly [`RP36_SWITCH_LINECODE_DECKS`];
/// * the **ownership**: which case pulls a declaring file in is walked, not
///   guessed from the directory tree, from every master in
///   `population.lock.json` over those edges ([`redirect_paths`]):
///   `zone_2/Branches.dss` is reached by its own zone master *and* by
///   `Master_Interconnected.dss`, and only the first is property-compared;
/// * the **scope**: a cell exists where `force_properties` turned the capi
///   property compare on (`gates_capi() && !kind.starts_with("large")` for
///   `solvable_now`), and it is *in scope* where the r4133 channel also gates the
///   case and no ledger `skip` drops it ([`r4133_skipped_cases`]). **The quoted
///   predicate is the rule that produced the frozen census**: the live rule
///   compares every live non-`large` case, and the frozen extracts are a data
///   lock that does not move with it, so the predicate is kept verbatim here;
/// * the **counter-claims**: the `StoCtrl` deck, and the token order that
///   silences `LVTestCaseNorthAmerican`'s declarations;
/// * the **reconciliation**: the derived totals must be `examples_full.txt`'s
///   per-spelling counts and `bins.tsv`'s cells / in-scope cells for the pair.
///
/// **The frozen `rust` column predates the fix.** The extracts under
/// `tests/corpus/props_r4133/` are a *data* lock
/// (`props_r4133_evidence_lock.rs`): this engine answers `99`/`98`, the r4133
/// spelling, where the frozen rows record `''`. They are read here as evidence of
/// what was measured, never as a claim about today's engine. That claim is
/// `exec::tests::line_fetch::switch_yes_keeps_the_linecode_and_its_units_conversion`.
#[test]
fn the_line_linecode_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "line.linecode";

    // (1) Population: one pass over every corpus file.
    let root = repo_root().join(CORPUS);
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files);
    assert!(
        files.len() > 1000,
        "only {} files under {CORPUS} — the vendored corpus is missing",
        files.len()
    );
    let mut decls: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut continued: Vec<String> = Vec::new();
    let mut unresolved: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    for rel in &files {
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else {
            continue; // a binary or non-UTF-8 fixture declares nothing
        };
        let counts = switch_linecode_line_decls(&text);
        if counts != (0, 0) {
            decls.insert(rel.clone(), counts);
        }
        if continued_line_tokens(&text) > 0 {
            continued.push(rel.clone());
        }
        let (resolved, missing) = redirect_paths(&text, rel, &root);
        if !resolved.is_empty() {
            edges.insert(rel.to_ascii_lowercase(), resolved);
        }
        if !missing.is_empty() {
            unresolved.insert(
                rel.to_ascii_lowercase(),
                missing.into_iter().map(|a| (rel.clone(), a)).collect(),
            );
        }
    }
    let measured: Vec<(&str, usize, usize)> = decls
        .iter()
        .map(|(f, (a, b))| (f.as_str(), *a, *b))
        .collect();
    let mut cited: Vec<(&str, usize, usize)> = RP36_SWITCH_LINECODE_DECKS.to_vec();
    cited.sort();
    assert_eq!(
        measured, cited,
        "every corpus `Line` declaration carrying both a `linecode=` and a `switch=` must sit in \
         the line.linecode decomposition — a new one changes the population the pair's cells are counted \
         from"
    );
    assert!(
        continued.is_empty(),
        "a `~` continuation carrying `linecode=`/`switch=` would be invisible to a \
         one-physical-line sweep, so the population above would stop being complete: {continued:?}"
    );

    // (2) Ownership: walked from every case master, not guessed from the tree.
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");
    let mut cases: Vec<String> = lock["solvable_now"]
        .as_object()
        .expect("the lock has a `solvable_now` section")
        .keys()
        .map(|rel| format!("solvable_now:{rel}"))
        .collect();
    for (family, rows) in lock["family_rigor"]
        .as_object()
        .expect("the lock has a `family_rigor` section")
    {
        cases.extend(
            rows.as_object()
                .unwrap_or_else(|| panic!("{family}: a rigor map"))
                .keys()
                .map(|rel| format!("{family}:{rel}")),
        );
    }
    assert!(
        cases.len() > 500,
        "only {} cases in the lock — the manifests are missing",
        cases.len()
    );
    // (3) Scope, per case, with the two predicates kept apart.
    let skipped = r4133_skipped_cases();
    let mut cells = 0usize;
    let mut in_scope_cells = 0usize;
    let mut order_excluded = 0usize;
    let mut carriers: Vec<(String, usize)> = Vec::new();
    let mut owners_of_stoctrl: Vec<String> = Vec::new();
    let mut dangling: BTreeSet<(String, String)> = BTreeSet::new();
    for case in &cases {
        let rigor = case_rigor(&lock, case);
        let kind = rigor_field(&rigor, "kind");
        let engines = rigor_field(&rigor, "engines");
        let steps: usize = rigor_field(&rigor, "steps")
            .parse()
            .unwrap_or_else(|e| panic!("{case}: steps= is not a number: {e}"));
        // `force_properties` (`corpus_gate/scheduler.rs`) AS IT STOOD WHEN THE
        // FROZEN CENSUS WAS MEASURED, verbatim: the capi property compare is on
        // for a `solvable_now` case that gates capi and is not `large`, and for a
        // family case that gates capi (all three families set the family-level
        // flag). The live rule has no `gates_capi` half. This derivation
        // reconciles frozen data and therefore keeps the rule that produced it.
        let gates_capi = engines != "r4133";
        let property_compared =
            gates_capi && (!case.starts_with("solvable_now:") || !kind.starts_with("large"));
        if !property_compared {
            continue;
        }
        // The r4133 half: `engines` alone is not enough.
        let in_scope = engines != "capi_v0145" && !skipped.contains(case);
        let mut closure: BTreeSet<String> = BTreeSet::new();
        let mut stack = vec![case_deck(case).to_ascii_lowercase()];
        while let Some(f) = stack.pop() {
            if !closure.insert(f.clone()) {
                continue;
            }
            dangling.extend(unresolved.get(&f).into_iter().flatten().cloned());
            for next in edges.get(&f).into_iter().flatten() {
                stack.push(next.to_ascii_lowercase());
            }
        }
        let mut here = 0usize;
        for (file, (after, before)) in &decls {
            if !closure.contains(&file.to_ascii_lowercase()) {
                continue;
            }
            if file == RP36_STOCTRL_DECK {
                owners_of_stoctrl.push(case.clone());
            }
            here += after * steps;
            order_excluded += before * steps;
            in_scope_cells += usize::from(in_scope) * after * steps;
        }
        cells += here;
        if here > 0 {
            carriers.push((case.clone(), here));
        }
    }
    // A `Redirect` a property-compared case's walk cannot resolve would truncate
    // that case's closure, and a declaring deck hidden behind it would be missed
    // — so the leftovers are asserted, not trusted. Exactly one exists in the
    // whole reachable corpus, and it is a template placeholder.
    let dangling: Vec<&(String, String)> = dangling
        .iter()
        .filter(|(f, a)| (f.as_str(), a.as_str()) != RP36_TEMPLATE_REDIRECT)
        .collect();
    assert!(
        dangling.is_empty(),
        "a `Redirect` argument this walk cannot resolve could hide a declaring deck from a \
         property-compared case's closure, so the ownership above would under-count: {dangling:?}"
    );
    carriers.sort();
    let cited_carriers: Vec<(String, usize)> = RP36_CELL_CASES
        .iter()
        .map(|(c, n)| ((*c).to_string(), *n))
        .collect();
    assert_eq!(
        carriers, cited_carriers,
        "the five cells are carried by the two EPRI_Ckt7-G torn-circuit zone cases and by nothing \
         else"
    );

    // (4) The counter-claim, both halves: the `StoCtrl` deck really does have
    //     the shape, and really does contribute nothing.
    assert_eq!(
        decls.get(RP36_STOCTRL_DECK),
        Some(&(9usize, 0usize)),
        "the zero below must come from the case's rigor, not from the deck lacking the shape"
    );
    assert_eq!(
        rigor_field(&case_rigor(&lock, RP36_STOCTRL_CASE), "kind"),
        "large",
        "{RP36_STOCTRL_CASE}: `force_properties` excludes a large case's property compare, which \
         is the whole reason its nine switched linecode lines carry no cell"
    );
    assert!(
        owners_of_stoctrl.is_empty(),
        "StoCtrl_Current_PeakShave/Line.DSS contributes ZERO of the 5 cells and no \
         property-compared case even reaches it: {owners_of_stoctrl:?}"
    );
    // …and the order rule is derived, not assumed. It is currently redundant —
    // the 160 `switch=yes linecode=switch` declarations sit on
    // `LVTestCaseNorthAmerican`, whose cases are `large_floating_zeroseq` and so
    // are already outside the property compare — but it is the mechanism, so a
    // corpus that grew a *feeder* deck of that shape must still count zero.
    let before_total: usize = decls.values().map(|(_, b)| b).sum();
    assert_eq!(
        before_total, 160,
        "the `switch=` before `linecode=` population is LVTestCaseNorthAmerican's Master/SecPar"
    );
    assert_eq!(
        order_excluded, 0,
        "no property-compared case declares a `switch=… linecode=…` line today, so the two \
         exclusions agree; if one ever does, its cells must still be zero — `FetchLineCode` \
         re-arms the flag last and every engine renders the code"
    );

    // (5) …and the products are the frozen census's own numbers.
    let corpus = Corpus::load();
    let rows: Vec<&Example> = corpus.rows.iter().filter(|r| r.pair == PAIR).collect();
    let mut frozen: Vec<(&str, &str, usize)> = rows
        .iter()
        .map(|r| (r.rust.as_str(), r.r4133.as_str(), r.cells))
        .collect();
    frozen.sort();
    assert_eq!(
        frozen,
        [("", "98", 1), ("", "99", 4)],
        "the frozen census — two spellings, its `rust` side predates the fix, and the \
         extracts are never edited"
    );
    assert_eq!(
        cells,
        rows.iter().map(|r| r.cells).sum::<usize>(),
        "derived cells must be the frozen example rows' total for {PAIR}"
    );
    let ev = corpus
        .evidence(rows[0])
        .unwrap_or_else(|| panic!("{PAIR}: no frozen evidence record"));
    assert_eq!(
        (cells, Some(in_scope_cells)),
        (ev.cells, ev.cells_in_scope),
        "derived cells / in-scope cells must be bins.tsv's for {PAIR}"
    );
    assert_eq!(
        in_scope_cells, 5,
        "every line.linecode cell is in scope, which is why the r4133 compare reaches \
         them"
    );
}

/// **The `swtcontrol.normal`/`state` census decomposition, as data**: every
/// corpus case that declares a `SwtControl`, with the two facts the pair's cells
/// hang off: `(case, tokens the render carries, controls whose `State` renders
/// OPEN)`.
///
/// The control COUNT is not repeated here: it is
/// [`RP31_DELAY_CASES`]/[`RP31_NO_DELAY_CASES`]' column, re-measured from the
/// decks by [`the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus`] and by this
/// test's own sweep, so the two decompositions cannot disagree about the
/// population. What is new here is the *shape* of each case's render:
///
/// * **tokens**: the getter renders one token per CONTROLLED-ELEMENT phase, so
///   every case is 3 except `makeposseq_ctrl`, whose `line.sw` drops to one
///   phase at `MakePosSequence`. That single case is the whole of the
///   `'[closed, ]'` spelling both frozen example rows record;
/// * **open**: how many of the case's controls answer `open` after its own
///   `post` commands have run. `midi_swtcontrol` and `swtcontrol_time` arm a
///   delayed `action=open` (1 control each) and `civanlar.dss` declares three of
///   its ties `Action=o` (`5_11`, `7_16`, `10_14`). Every other control in the
///   corpus is closed, and `Normal` is closed on all of them: untyped ties
///   default to all-CLOSED and the two `swtcontrol` micro-decks type
///   `normal=closed` explicitly.
///
/// Those two columns turn the pair's cells into the frozen census's exact
/// per-spelling split, and the split is the load-bearing conclusion: the
/// in-scope cells sit on three `engines: r4133` decks (which COMPARE, hence no
/// r4133 entry), the rest on exactly five `capi_v0145` cases, *hence exactly
/// five ledger entries and no more*.
const RP37_SWTCONTROL_CASES: &[(&str, usize, usize)] = &[
    ("controls:swtcontrol/midi_swtcontrol.dss", 3, 1),
    ("controls:swtcontrol/swtcontrol_lock.dss", 3, 0),
    ("controls:swtcontrol/swtcontrol_time.dss", 3, 1),
    ("modes:makeposseq/makeposseq_ctrl.dss", 1, 0),
    (
        "solvable_now:Version8/Distrib/Examples/HarmonicsTMode/IEEE_519.DSS",
        3,
        0,
    ),
    (
        "solvable_now:Version8/Distrib/Examples/HarmonicsVariableLoad/IEEE_519.DSS",
        3,
        0,
    ),
    (
        "solvable_now:Version8/Distrib/Examples/Matlab/HarmonicT_MATLAB/IEEE_519.DSS",
        3,
        0,
    ),
    (
        "solvable_now:Version8/Distrib/Examples/civinlar model/civanlar.dss",
        3,
        3,
    ),
];

/// The exact `[closed, …]` / `[open, …]` spelling an `n`-token render carries —
/// r4133's `'['` + `n` × `'<state>, '` + `']'` (`SwtControl.pas:589-599`).
fn swt_render(n: usize, state: &str) -> String {
    let mut s = String::from("[");
    for _ in 0..n {
        s.push_str(state);
        s.push_str(", ");
    }
    s.push(']');
    s
}

/// **The `swtcontrol.normal`/`state` census decomposition is read off the
/// corpus**: [`the_swtcontrol_delay_census_decomposition_is_read_off_the_corpus`]' shape.
///
/// The conclusion this derives is the pair's cells, the in-scope ones on three
/// `engines: r4133` decks, the rest on exactly five `capi_v0145` cases, hence
/// exactly five ledger entries and no more. The in-scope cells COMPARE on r4133,
/// so that side needs no entry.
///
/// Four independent places are reconciled against each other:
///
/// * the **decks** give the control count and the completeness claim: the sweep
///   fails if any deck outside [`RP31_DELAY_CASES`]/[`RP31_NO_DELAY_CASES`]
///   declares a `SwtControl`, so a corpus that grows one reds here, where the
///   "exactly five entries" conclusion is drawn;
/// * `population.lock.json` gives each case's `steps=` and `engines=`, so
///   `cells = controls × steps` and "in scope" is the lock's own answer;
/// * the **frozen census** (`bins.tsv` and `examples_full.txt`'s rows) is what
///   the products must add up to, **per spelling**, which only comes out right
///   if the `tokens`/`open` columns above are right;
/// * `tests/corpus/ledger.json` gives the landed `capi_v0145` entries, which
///   must be exactly one per out-of-scope case, each carrying one `property`
///   match row per (control, property).
///
/// The frozen `rust` column stays `'closed'`/`'open'`, the frozen census, never
/// edited.
#[test]
fn the_swtcontrol_state_census_decomposition_is_read_off_the_corpus() {
    const PAIRS: [&str; 2] = ["swtcontrol.normal", "swtcontrol.state"];

    // (1) Completeness: `RP37_SWTCONTROL_CASES` names every corpus case that
    //     declares a SwtControl, and the control counts come from the decks.
    let root = repo_root().join(CORPUS);
    let mut decks = Vec::new();
    collect_dss(&root, &root, &mut decks);
    assert!(
        decks.len() > 1000,
        "only {} .dss files under {CORPUS} — the vendored corpus is missing",
        decks.len()
    );
    let declared: BTreeMap<String, usize> = decks
        .iter()
        .map(|d| (d.clone(), swtcontrol_facts(d).0))
        .filter(|(_, n)| *n > 0)
        .collect();
    let cited: BTreeSet<String> = RP37_SWTCONTROL_CASES
        .iter()
        .map(|(case, _, _)| case_deck(case))
        .collect();
    assert_eq!(
        declared.keys().cloned().collect::<BTreeSet<_>>(),
        cited,
        "every corpus deck declaring a SwtControl must sit in the swtcontrol state decomposition — a new one \
         changes how many ledger entries the pair owes"
    );

    // (2) Per case: cells = controls × steps, in scope iff the case gates r4133,
    //     and the render's spelling is `tokens` wide.
    let skipped = r4133_skipped_cases();
    let lock_path = repo_root().join(POPULATION_LOCK);
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&lock_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", lock_path.display())),
    )
    .expect("population.lock.json is JSON");

    // spelling -> cells, per pair; plus the in-scope / out-of-scope case splits.
    let mut normal: BTreeMap<String, usize> = BTreeMap::new();
    let mut state: BTreeMap<String, usize> = BTreeMap::new();
    let mut in_scope_cases: Vec<(&str, usize)> = Vec::new();
    let mut capi_cases: Vec<(&str, usize, usize)> = Vec::new();
    let (mut cells, mut in_scope_cells) = (0usize, 0usize);
    for (case, tokens, open) in RP37_SWTCONTROL_CASES {
        let controls = declared[&case_deck(case)];
        assert!(
            *open <= controls,
            "{case}: {open} open controls of {controls} declared"
        );
        assert!(
            *open == 0 || *tokens == 3,
            "{case}: every open control in the corpus is on a 3-phase switched line"
        );
        let rigor = case_rigor(&lock, case);
        let steps: usize = rigor_field(&rigor, "steps")
            .parse()
            .unwrap_or_else(|e| panic!("{case}: steps= is not a number: {e}"));
        let here = controls * steps;
        // Both halves of "does r4133 gate this case?".
        let in_scope = rigor_field(&rigor, "engines") != "capi_v0145" && !skipped.contains(*case);
        cells += here;
        if in_scope {
            in_scope_cells += here;
            in_scope_cases.push((case, here));
        } else {
            capi_cases.push((case, controls, here));
        }
        // `Normal` is closed on every control of every case; `State` splits.
        *normal.entry(swt_render(*tokens, "closed")).or_default() += here;
        *state.entry(swt_render(*tokens, "open")).or_default() += open * steps;
        *state.entry(swt_render(*tokens, "closed")).or_default() += (controls - open) * steps;
    }
    normal.retain(|_, c| *c > 0);
    state.retain(|_, c| *c > 0);
    in_scope_cases.sort_unstable();
    capi_cases.sort_unstable();

    // (3) …and the products are the frozen census's own numbers, per spelling
    //     and in total, for BOTH pairs.
    let corpus = Corpus::load();
    for pair in PAIRS {
        let rows: Vec<&Example> = corpus.rows.iter().filter(|r| r.pair == pair).collect();
        let derived = if pair.ends_with("normal") {
            &normal
        } else {
            &state
        };
        let mut frozen: BTreeMap<String, usize> = BTreeMap::new();
        for r in &rows {
            assert_eq!(
                r.rust,
                r.r4133.trim_start_matches('[').split(',').next().unwrap(),
                "{pair}: the frozen `rust` column is the scalar this engine rendered when the census was frozen — \
                 exactly r4133's own token, un-repeated"
            );
            *frozen.entry(r.r4133.clone()).or_default() += r.cells;
        }
        assert_eq!(
            derived, &frozen,
            "{pair}: the derived per-spelling split must be the frozen example rows'"
        );
        let ev = corpus
            .evidence(rows[0])
            .unwrap_or_else(|| panic!("{pair}: no frozen evidence record"));
        assert_eq!(
            (cells, Some(in_scope_cells)),
            (ev.cells, ev.cells_in_scope),
            "derived cells / in-scope cells must be bins.tsv's for {pair}"
        );
    }
    assert_eq!(
        (cells, in_scope_cells),
        (59, 40),
        "the pair's cells and in-scope cells, summed over the two pairs for the \
         r4133-compared total"
    );
    assert_eq!(
        in_scope_cases,
        [
            ("controls:swtcontrol/midi_swtcontrol.dss", 12),
            ("controls:swtcontrol/swtcontrol_time.dss", 12),
            (
                "solvable_now:Version8/Distrib/Examples/civinlar model/civanlar.dss",
                16
            ),
        ],
        "the 40 in-scope cells are 12 + 12 + 16 over the three r4133-gating decks; all three now \
         render r4133's bytes, so NO r4133 property entry is staged or owed"
    );

    // (4) One landed `capi_v0145` entry per out-of-scope case and no other, each
    //     with one `property` match row per (control, property).
    let ledger_path = repo_root().join(LEDGER);
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&ledger_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", ledger_path.display())),
    )
    .expect("ledger.json is JSON");
    let entries = doc["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array");
    let mine: Vec<&serde_json::Value> = entries
        .iter()
        .filter(|e| e["cause_ref"] == "swtcontrol-per-phase-state-render")
        .collect();
    let mut entry_cases: Vec<&str> = mine
        .iter()
        .map(|e| e["case"].as_str().expect("an entry names its case"))
        .collect();
    entry_cases.sort_unstable();
    assert_eq!(
        entry_cases,
        capi_cases.iter().map(|(c, ..)| *c).collect::<Vec<_>>(),
        "exactly the out-of-scope cases owe a LANDED capi_v0145 entry — one each; the in-scope \
         cells compare on r4133 and owe none"
    );
    assert_eq!(
        capi_cases.iter().map(|(_, _, n)| n).sum::<usize>(),
        cells - in_scope_cells,
        "the capi split must be the complement of the in-scope one"
    );
    for e in &mine {
        let case = e["case"].as_str().unwrap();
        let controls = capi_cases
            .iter()
            .find(|(c, ..)| *c == case)
            .map(|(_, n, _)| *n)
            .unwrap();
        assert_eq!(e["channel"], "capi_v0145");
        assert_eq!(e["kind"], "divergence");
        let ms = e["match"].as_array().expect("match rows");
        let props = ms.iter().filter(|m| m["field"] == "property").count();
        assert_eq!(
            props,
            controls * 2,
            "{case}: one `property` scope per (control, property) — {controls} control(s) x \
             {{Normal, State}}"
        );
        // Probes are a SECOND exposure on the one deck whose manifest declares
        // them, never a substitute for the property rows.
        let probes = ms.iter().filter(|m| m["field"] == "probe").count();
        let want = usize::from(case == "controls:swtcontrol/swtcontrol_lock.dss") * 2;
        assert_eq!(
            probes, want,
            "{case}: only swtcontrol_lock.dss probes SwtControl.sw.state/.normal (its manifest \
             declares probes [state, normal, lock]; `lock` does not diverge)"
        );
        for m in ms {
            assert_eq!(
                m["oracle"], "closed",
                "{case}: the 0.14.5 oracle renders one bare word on every one of these cells"
            );
            let tokens = RP37_SWTCONTROL_CASES
                .iter()
                .find(|(c, ..)| *c == case)
                .map(|(_, t, _)| *t)
                .unwrap();
            assert_eq!(
                m["rust"].as_str(),
                Some(swt_render(tokens, "closed").as_str()),
                "{case}: the pinned rust value must be the {tokens}-token render"
            );
        }
    }
    // …and every one of them is witnessed, which is the other half of the
    // "exactly five" claim.
    let witnessed: BTreeSet<&str> = LANDED_PROPERTY_ENTRY_PINS
        .iter()
        .filter(|(_, step, _, _)| *step == "swtcontrol-per-phase")
        .map(|(id, ..)| *id)
        .collect();
    let landed: BTreeSet<&str> = mine
        .iter()
        .map(|e| e["id"].as_str().expect("an entry names its id"))
        .collect();
    assert_eq!(
        witnessed, landed,
        "each landed per-phase SwtControl entry owes a LANDED_PROPERTY_ENTRY_PINS witness and vice versa"
    );
}

/// **The offline evidence base is one spelling behind the live population, and
/// that term is asserted rather than assumed.**
///
/// The replay proves per-spelling completeness over the vendored files, the
/// claims census measures it over the live corpus, and the two differ by
/// [`LIVE_ONLY_SPELLINGS`]. Every part of that statement is checked here: the
/// count reconciles, each live-only spelling is really claimed by the shipped
/// table (so a narrowing of the rule that claims it reds this test), each sits
/// on a pair the table holds, and each sits on a pair the supplement **cannot**
/// carry.
#[test]
fn the_live_only_spellings_are_claimed_and_reconcile_the_two_accountings() {
    let corpus = Corpus::load();
    assert_eq!(
        CLAIMED_NORMALIZATION + LIVE_ONLY_SPELLINGS.len(),
        CLAIMED_TOTAL_LIVE,
        "the vendored claim count plus the live-only spellings must equal what the \
         full-population claims census measured"
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

/// **The whole chain's two accountings reconcile**: the claims census's claimed
/// spellings on the r4133 channel ([`CLAIMED_SPELLINGS_LIVE`]) against this
/// file's [`CLAIMED_TOTAL`].
///
/// The gap is asserted, not narrated: one [`LIVE_ONLY_SPELLINGS`] entry (the
/// normalization link's) plus the [`LIVE_ONLY_DISPLAY_SPELLINGS`] (the floor's).
/// Each of the latter is checked to be
///
/// * claimed by the **shipped** floor predicate, so a narrowed floor reds here
///   rather than at the next census;
/// * claimed by the floor and by **no earlier link**, i.e. the floor is really
///   its first match and the census's `under-floor` tag is right;
/// * on a pair the frozen `bins.tsv` holds, which is exactly what bars the
///   spelling from the supplement, and absent from the vendored rows, so it
///   genuinely has no home here;
/// * on one of the **five** `vsource` pairs the drift names, and inside that
///   pair's own frozen `max_rel`: without these two the list carries only its
///   own length, and any nearby invented number passes.
#[test]
fn the_display_floors_live_only_spellings_reconcile_the_claims_census() {
    assert_eq!(
        CLAIMED_TOTAL + LIVE_ONLY_SPELLINGS.len() + LIVE_ONLY_DISPLAY_SPELLINGS.len(),
        CLAIMED_SPELLINGS_LIVE,
        "the vendored claim count plus both live-only lists must equal what the full-population \
         claims census measured"
    );
    let corpus = Corpus::load();
    let floor = props_norm::display_floor().expect("the r4133 props display floor is derived");
    for (pair, rust, r4133) in LIVE_ONLY_DISPLAY_SPELLINGS {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        assert!(
            props_norm::under_display_floor(rust, r4133),
            "{pair} '{rust}' vs '{r4133}' is claimed `under-floor` live but not by the shipped \
             predicate — the live population and this evidence base have drifted"
        );
        // The floor is its FIRST match, which is what the census tag claims.
        assert!(props_norm::claiming_row(class, prop, rust, r4133).is_none());
        assert!(!props_norm::echo_excluded(class, prop, rust, r4133));
        // …and comfortably inside the floor: the widest of the six is 2.51e-05,
        // under half the derivation's worst, so the live-only population moves
        // no part of the calibration.
        let rel = props_norm::display_rel(rust, r4133).expect("a numeric cell");
        assert!(rel <= floor / 2.0, "{pair}: {rel:e}");
        // Anchored to the pair's OWN frozen evidence, not just to the floor:
        // one of the five `vsource` pairs the drift is recorded on, and inside
        // that pair's frozen `max_rel` (`%.2e`-rounded, hence the margin).
        assert!(
            LIVE_ONLY_DISPLAY_PAIRS.contains(pair),
            "{pair} is not one of the five vsource pairs the recorded drift names"
        );
        let ceiling = frozen_max_rel(pair)
            .unwrap_or_else(|| panic!("{pair} has a numeric bins.tsv row with a max_rel"));
        assert!(
            rel <= ceiling * CEILING_ROUND_MARGIN,
            "{pair} '{rust}' vs '{r4133}': {rel:e} is above the pair's own frozen max_rel \
             {ceiling:e} — a live-only spelling this far out is a new population, not the \
             recorded +2-cells-per-vsource-pair drift"
        );
        // Invisible to both vendored files, for the recorded reason.
        assert!(
            corpus.bins.contains_key(*pair),
            "{pair} has no bins.tsv row — then the supplement could carry this spelling"
        );
        assert!(
            !corpus
                .rows
                .iter()
                .any(|r| &r.pair == pair && r.rust == *rust && r.r4133 == *r4133),
            "{pair} '{rust}' vs '{r4133}' IS in the vendored evidence after all — drop it from \
             LIVE_ONLY_DISPLAY_SPELLINGS and move the count with it"
        );
    }
}

/// **Liveness both ways, offline**: every row of `PROPS_NORM_R4133` claims at
/// least one example row. A row that folds nothing exempts a spelling
/// difference that is not there: drop it, or re-measure it.
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
/// coverage over a population that excludes the very classes the shape closures
/// opened, so this is the assertion that fails if the supplement is dropped
/// from the accounting.
#[test]
fn the_supplement_is_what_makes_the_shape_closure_rows_live() {
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
        "on examples_full.txt alone, exactly the shape-closure rows are dead — that is what the \
         supplement exists for"
    );
    assert_eq!(
        want.len(),
        10,
        "7 BoolFold + 2 CaseFold shape-closure rows, plus the off-bin `generator.userdata` \
         ArrayForm row — its pair, like the other nine, exists ONLY in the supplement"
    );
}

/// **The shape half of the chain**: shape-allowlist rows are exercised against
/// the full `shape.txt`, capi-only classes included.
///
/// Both directions: every `shape.txt` gap name is closed either by an r4133
/// `PROPS_015X` row or by porting the property, and every such allowlist row is
/// exercised by a gap name. The 0.15.x rows are **not** in this accounting (they
/// answer to `props_roundtrip` and the goldens), and the intersection with
/// `shape.txt` is what separates them.
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
        "the gap names no allowlist row covers must be exactly the ported WindGen surface"
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
        "the GenDispatcher registration-bug row"
    );
}

/// **Evidence integrity**: every `PROPS_NORM_R4133` row's citation triple
/// *(pair, bin, cells)* is read back against its evidence: `bins.tsv` for the
/// frozen census, [`RP1_RECORDS`] for the pairs the shape closures created.
#[test]
fn every_norm_row_matches_its_cited_evidence() {
    let bins = bins_evidence();
    let rp1 = RP1_RECORDS;
    assert_eq!(rp1.len(), 24, "the shape closures created 24 pairs");
    for (step, want) in [
        (CiteSrc::Rp11, 12),
        (CiteSrc::Rp12, 9),
        (CiteSrc::Rp13, 2),
        (CiteSrc::Rp14, 1),
    ] {
        assert_eq!(
            rp1.iter().filter(|r| r.substep == step).count(),
            want,
            "the {} records: {want} new pairs",
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
                        "{pair}: cites the {} records, which have no such row",
                        substep_tag(step)
                    )
                });
                assert_eq!(
                    rec.substep, step,
                    "{pair}: recorded under a different shape closure"
                );
                assert_eq!(
                    (rec.bin, rec.cells),
                    (r.bin, r.cells as usize),
                    "{pair}: row says (bin {}, {} cells), the record says (bin {}, {} cells)",
                    r.bin,
                    r.cells,
                    rec.bin,
                    rec.cells
                );
            }
        }
    }
}

/// The supplement carries a row for **every** shape-closure pair [`RP1_RECORDS`]
/// holds, and the record's representative spelling is among the supplement's
/// distinct spellings for that pair: the re-measure supplements the record
/// instead of replacing it.
#[test]
fn the_supplement_covers_every_recorded_shape_closure_pair() {
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

    for rec in RP1_RECORDS {
        let got = by_pair.get(rec.pair).unwrap_or_else(|| {
            panic!(
                "{}: recorded by a shape-closure record, missing from the supplement",
                rec.pair
            )
        });
        assert_eq!(
            got.iter().map(|r| r.cells).sum::<usize>(),
            rec.cells,
            "{}: the supplement's spellings must account for every cell the record holds",
            rec.pair
        );
        // The record holds one representative spelling per pair, elided with an
        // ellipsis when it is long. Whole values live only in the supplement.
        let elided = rec.rust.contains('…') || rec.r4133.contains('…');
        assert!(
            elided
                || got
                    .iter()
                    .any(|r| r.rust == rec.rust && r.r4133 == rec.r4133),
            "{}: the record's example '{}' vs '{}' is not among the supplement's spellings",
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

/// **The echo table claims only its own pairs, and the display floor claims
/// only what its derivation covers.** A floor is the one link that could
/// over-claim silently (it has no row set to audit), so both of its bounds are
/// checked over the same full-corpus walk:
///
/// * the echo link claims a row **only** on a pair `PROPS_ECHO_R4133` names, so
///   the exclusion cannot have leaked onto a pair no row cites;
/// * a pair the table does NOT name is still compared raw, even when its
///   spelling looks exactly like an echo (`''` on one side): the mask is the
///   cited row set, never a shape heuristic;
/// * every row the floor claims is inside `props_norm::display_floor()` **in
///   the shipped metric**, and the WORST of them is the derivation's own worst
///   cell, so a floor edited without re-deriving reds here;
/// * no row the floor claims is a structural one: each carries at least one
///   number on both sides, with identical non-numeric skeletons.
#[test]
fn the_echo_table_claims_only_its_cited_pairs_and_the_floor_only_its_derivation() {
    assert_eq!(
        PROPS_ECHO_R4133.len(),
        82,
        "the echo table: the echo bucket's pairs minus the five SilentReadOnly pairs, plus \
         generator.model"
    );
    let floor = props_norm::display_floor().expect("the r4133 props display floor is derived");
    assert_eq!(
        floor, 2e-4,
        "the derived floor (props_norm::R4133_DISPLAY_FLOOR)"
    );
    let cited: BTreeSet<String> = PROPS_ECHO_R4133
        .iter()
        .map(|r| format!("{}.{}", r.class, r.prop))
        .collect();
    let corpus = Corpus::load();
    let (mut echoed, mut floored) = (0, 0);
    let mut worst = (0.0f64, String::new());
    for row in &corpus.rows {
        let [_, _, echo, hit] = chain_verdicts(&corpus, row);
        if hit {
            let rel = props_norm::display_rel(&row.rust, &row.r4133).unwrap_or_else(|| {
                panic!(
                    "{} '{}' vs '{}': the floor claimed a cell that is not a numeric divergence",
                    row.pair, row.rust, row.r4133
                )
            });
            assert!(rel <= floor, "{}: {rel:e} is outside the floor", row.pair);
            // …and the MECHANISM, per claimed row: r4133's spelling is our
            // value rounded to the digits r4133 printed. Without it "every
            // claimed cell is a `%[-].Ng` render" would be a survey, and the
            // round-trip residue (`RP39_ROUTING`) contradicts it.
            assert!(
                props_norm::display_is_render(&row.rust, &row.r4133),
                "{} '{}' vs '{}': claimed by the floor but no `%.Ng` render of our value",
                row.pair,
                row.rust,
                row.r4133
            );
            if rel > worst.0 {
                worst = (
                    rel,
                    format!("{} '{}' vs '{}'", row.pair, row.rust, row.r4133),
                );
            }
            floored += 1;
        }
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
    assert!(floored > 0, "the floor link must claim something now");
    assert!(
        (worst.0 - 6.431_124e-5).abs() < 1e-10,
        "the worst cell the floor claims is the derivation's left-hand side, 6.431124e-05 \
         (load.pf, `%-.4g`) — measured {:e} on {}",
        worst.0,
        worst.1
    );
    // The five superseded pairs still carry `''`-on-one-side spellings in the
    // FROZEN extract (the shape bin 5 is built on) and take no echo row all the
    // same. This engine renders a number on both sides, see
    // [`RP38_SUPERSEDED`].
    for (pair, _, _, _) in RP38_SUPERSEDED {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        assert!(
            !props_norm::has_echo_row(class, prop),
            "{pair}: an echo-LOOKING spelling is not an echo row"
        );
    }
    // …and neither is `gictransformer.r2`, settled as `LEDGER`: its r4133 getter
    // COMPUTES its value live off a conductance r4133 mis-derives, so calling it
    // an echo would misname the mechanism, and the exclusion is two ledger
    // entries instead. `generator.model` is the opposite case: r4133's `model`
    // getter answers the deck's own token, so it is an echo row.
    assert!(!props_norm::has_echo_row("gictransformer", "r2"));
    assert!(props_norm::has_echo_row("generator", "model"));
}

/// **The display floor's empty band, asserted**: the right-hand half of the
/// floor's derivation (`props_norm::R4133_DISPLAY_FLOOR`'s doc table).
///
/// The floor is placed, not tuned, because the band between the worst cell it
/// claims (`load.pf`, pinned by
/// [`the_echo_table_claims_only_its_cited_pairs_and_the_floor_only_its_derivation`])
/// and the nearest numeric-comparable spelling above it
/// (`storagecontroller.kwneed`) holds NO vendored spelling, so every cut inside
/// it partitions the census the same way. This walks every vendored spelling
/// (`examples_full.txt` + `examples_supplement.txt`) through the shipped metric,
/// `props_norm::display_rel`, whatever link claims it, and asserts both halves:
/// nothing strictly inside the band, and the nearest spelling above it is
/// `storagecontroller.kwneed`. The floor constant is neither read nor moved
/// here: a spelling inside the band re-opens the derivation, it never licenses
/// a new number.
#[test]
fn the_display_floor_band_is_empty_over_the_vendored_spellings() {
    const WORST_CLAIMED: f64 = 6.431_124e-5;
    const NEAREST_ABOVE: f64 = 1.374_769e-3;
    // The two edges are 7-digit roundings of measured cells that sit ON them,
    // so "strictly inside" keeps a 1e-6 relative margin off each.
    let (lo, hi) = (WORST_CLAIMED * (1.0 + 1e-6), NEAREST_ABOVE * (1.0 - 1e-6));
    let corpus = Corpus::load();
    let mut comparable = 0usize;
    let mut inside: Vec<String> = Vec::new();
    let mut nearest: Option<(f64, String)> = None;
    for row in &corpus.rows {
        let Some(rel) = props_norm::display_rel(&row.rust, &row.r4133) else {
            continue;
        };
        comparable += 1;
        let who = format!("{} '{}' vs '{}'", row.pair, row.rust, row.r4133);
        if rel > lo && rel < hi {
            inside.push(format!("{who} ({rel:e})"));
        }
        if rel > lo && nearest.as_ref().is_none_or(|(best, _)| rel < *best) {
            nearest = Some((rel, who));
        }
    }
    assert!(
        comparable >= 2000,
        "only {comparable} numeric-comparable vendored spelling(s) — the walk went vacuous"
    );
    assert!(
        inside.is_empty(),
        "the r4133 display floor's empty band (6.431124e-05, 1.374769e-03) is no longer empty — \
         the floor's derivation (props_norm::R4133_DISPLAY_FLOOR) no longer holds and must be \
         re-derived, never re-tuned: {inside:?}"
    );
    let (rel, who) = nearest.expect("a numeric-comparable spelling above the band");
    assert!(
        who.starts_with("storagecontroller.kwneed ") && (rel - NEAREST_ABOVE).abs() < 1e-9,
        "the nearest vendored spelling above the display floor's band must be \
         storagecontroller.kwneed at 1.374769e-03 (the band's upper edge in the floor's \
         derivation) — measured {rel:e} on {who}"
    );
}

/// **The carve-outs and their routing describe the same cells**: the third-map
/// guard for the narrowing valve.
///
/// `props_norm::ECHO_CARVE_OUTS` decides what the comparator lets through.
/// [`ECHO_CARVE_OUT_ROUTING`] decides who then owns the cell. A carve-out with
/// no routing would land in the ledger's `unaccounted` bucket (the kill
/// criterion, the loud outcome) but a routing with no carve-out would simply
/// never fire, so both directions are asserted here, plus the two things that
/// make a routing a verdict: an owner that is not the echo bucket, and a
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
            "{pair}: routing a carved-out cell back to the echo bucket would re-close the mask this valve \
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
    // Liveness: the routed owner has DISCHARGED the row rather than still
    // holding it. `led.owner(Owner::Rp24) == DECLARED_RP24` reads `(0, 0, 0)`,
    // which alone would be vacuous, so the cell is chased to the link that
    // actually took it.
    let corpus = Corpus::load();
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(
        led.owner(Owner::Rp24),
        DECLARED_RP24,
        "the carved-out cell is claimed, not pending"
    );
    for (pair, rust, r4133, owner, _) in ECHO_CARVE_OUT_ROUTING {
        assert_eq!(*owner, Owner::Rp24);
        let row = corpus
            .rows
            .iter()
            .find(|r| &r.pair == pair && r.rust == *rust && r.r4133 == *r4133)
            .unwrap_or_else(|| panic!("{pair} '{rust}' vs '{r4133}' is a vendored example row"));
        assert_eq!(
            first_match(chain_verdicts(&corpus, row)),
            Some(Link::DisplayFloor),
            "{pair}: the carve-out's declared owner is the display floor, and the floor claims \
             it — r4133's own MakePosSequence round-trip"
        );
    }
}

/// **The display floor's residual is proved out of scope, not waved away**: the
/// both-ways guard for [`RP24_OUT_OF_SCOPE`], the only place a row leaves a
/// bucket without a mechanism claiming it.
///
/// The rule is a *proof* (a spelling cannot exceed the maximum taken over the
/// in-scope cells and still be one of them), so everything it rests on is
/// asserted here rather than trusted:
///
/// * each cited ceiling is **the pair's own frozen `max_rel_in_scope`**, read
///   back from `bins.tsv`: a hand-typed ceiling that drifted from the evidence
///   would silently widen the scope rule;
/// * each cited row count is what the walk really re-declares, and the four
///   pairs are exactly the pairs it re-declares (no fifth pair slipping in);
/// * every re-declared row clears its ceiling by at least
///   [`RP24_OUT_OF_SCOPE_MIN_RATIO`], so the rule never turns on a rounding;
/// * the pairs are display-class by the in-scope re-derivation
///   (`effective_bin() == 6`) while their frozen label is bin 7, which is the
///   whole reason their example inventory carries these spellings;
/// * and the rule is **narrow**: it fires on no other pair in the corpus.
#[test]
fn the_display_floors_residual_rows_are_proved_out_of_scope() {
    assert_eq!(
        RP24_OUT_OF_SCOPE.iter().map(|(_, _, n)| n).sum::<usize>(),
        RP24_OUT_OF_SCOPE_ROWS,
        "the table's row counts must sum to its lock"
    );
    let corpus = Corpus::load();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut min_ratio = f64::INFINITY;
    for row in &corpus.rows {
        // The rule is a RESIDUAL, consulted (by `declare`) only for a row no
        // link claimed — walking the claimed ones too would measure a
        // population the accounting never asks about. On these four pairs the
        // ceiling is far tighter than the floor, so most of their rows are
        // above the ceiling AND inside the floor: claimed, and none of this
        // rule's business.
        if first_match(chain_verdicts(&corpus, row)).is_some() {
            continue;
        }
        let Some(ev) = corpus.evidence(row) else {
            continue;
        };
        // …and the round-trip residue's rows are not this rule's business
        // either: the mechanism arm of `declare` runs first, so a spelling inside
        // the floor that is no render never reaches the ceiling. Two of them (on
        // `capacitor.cuf` and `generator.kvar`) WOULD clear their ceiling, and
        // counting them here would inflate the cited row counts and shrink the
        // measured margin: the walk must measure what the accounting actually
        // re-declares.
        if declare(row, ev) != Ok(Owner::OutOfScope) {
            continue;
        }
        let Some(ratio) = row_out_of_scope_by_ceiling(row, ev) else {
            continue;
        };
        assert_eq!(
            ev.effective_bin(),
            6,
            "{}: the rule applies to display-class pairs only",
            row.pair
        );
        assert_eq!(ev.bin, 7, "{}: whose frozen label is bin 7", row.pair);
        min_ratio = min_ratio.min(ratio);
        let (pair, _, _) = RP24_OUT_OF_SCOPE
            .iter()
            .find(|(p, _, _)| *p == row.pair)
            .expect("the rule only fires on a cited pair");
        *seen.entry(pair).or_default() += 1;
    }
    assert_eq!(
        seen.iter().map(|(p, n)| (*p, *n)).collect::<Vec<_>>(),
        RP24_OUT_OF_SCOPE
            .iter()
            .map(|(p, _, n)| (*p, *n))
            .collect::<Vec<_>>(),
        "the cited pairs and row counts must be exactly what the walk re-declares"
    );
    assert!(
        min_ratio >= RP24_OUT_OF_SCOPE_MIN_RATIO,
        "the tightest re-declared row clears its pair ceiling by {min_ratio:.1}x, under the \
         locked {RP24_OUT_OF_SCOPE_MIN_RATIO:.1}x — the scope proof is no longer orders of \
         magnitude clear of the `%.2e` rounding and must be re-measured"
    );
    // …and the lock is a MEASUREMENT, not a floor to hide behind: bracket it
    // above too, and pin the rounding margin literally, so neither constant can
    // move one-sidedly without a test noticing.
    assert!(
        min_ratio < 278.0,
        "the measured minimum is 277.17x (storagecontroller.kwneed, 1.374769e-03 against its \
         4.96e-06 ceiling); a different number means the population moved — re-record it, do \
         not widen the `>=`"
    );
    assert_eq!(
        CEILING_ROUND_MARGIN, 1.01,
        "the `%.2e` rounding of max_rel_in_scope needs at most 1.005x; 1.01 is twice that and \
         nothing in this rule may need more"
    );
    // The ceilings are the vendored ones, not transcriptions free to drift.
    for (pair, ceiling, _) in RP24_OUT_OF_SCOPE {
        let ev = corpus
            .bins
            .get(*pair)
            .and_then(|all| all.iter().find(|e| e.numeric))
            .unwrap_or_else(|| panic!("{pair} has a numeric bins.tsv row"));
        assert_eq!(
            ev.max_rel_in_scope,
            Some(*ceiling),
            "{pair}: the cited ceiling must BE the frozen max_rel_in_scope"
        );
    }
    // …and the rows really land in the OutOfScope bucket, with none of them
    // counted in scope.
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(led.owner(Owner::OutOfScope), DECLARED_OUT_OF_SCOPE);

    // **The single-number guard, driven.** The proof needs the row's gap to
    // dominate the census's offender-only `max_rel`, which holds for one number
    // and need not for several, so a multi-number row on a cited pair is refused
    // outright, however far above the ceiling it sits. Every real row is
    // single-number, so only a synthetic row can reach this arm. Without it the
    // rule would silently widen the day a vector-valued spelling appears on one
    // of the four pairs.
    let (pair, ceiling, _) = RP24_OUT_OF_SCOPE[0];
    let ev = corpus
        .bins
        .get(pair)
        .and_then(|all| all.iter().find(|e| e.numeric))
        .expect("a cited pair has numeric evidence");
    let two_numbers = Example {
        pair: pair.to_string(),
        class: pair.split('.').next().unwrap().to_string(),
        prop: pair.split('.').nth(1).unwrap().to_string(),
        rust: "[1, 2]".to_string(),
        r4133: "[1, 4]".to_string(),
        cells: 1,
        src: Source::Frozen,
    };
    assert!(
        props_norm::display_rel(&two_numbers.rust, &two_numbers.r4133)
            .is_some_and(|gap| gap > ceiling * CEILING_ROUND_MARGIN),
        "the synthetic row must be far above the ceiling, or it proves nothing"
    );
    assert_eq!(
        row_out_of_scope_by_ceiling(&two_numbers, ev),
        None,
        "{pair}: the ceiling proof must refuse a multi-number row"
    );
    // …and the same row with ONE number is accepted, so the guard is the
    // number count and not the shape of the render.
    let one_number = Example {
        rust: "1".to_string(),
        r4133: "4".to_string(),
        ..two_numbers
    };
    assert!(row_out_of_scope_by_ceiling(&one_number, ev).is_some());
}

/// **The floor's mechanism residual has an OWNER, and it is exactly the
/// measured one**: the both-ways guard for [`RP39_ROUTING`].
///
/// The vendored spellings the floor refuses for the mechanism are no `%.Ng`
/// render of our value, so they are not a display artifact. Everything the
/// declaration rests on is asserted here rather than trusted:
///
/// * every row the walk refuses for the mechanism sits on a cited pair, and
///   every cited pair really carries the number of refused rows it claims, so
///   a pair whose round trip gets fixed (or whose spelling changes) reds here
///   instead of silently emptying the work list;
/// * each refused row is **inside** the floor (it is the mechanism clause that
///   refuses it, not the metric, otherwise this would be ordinary bin-7
///   material), and no earlier link claims it;
/// * each citation names an r4133 site, the same discipline
///   [`ECHO_CARVE_OUT_ROUTING`] and [`RP38_SUPERSEDED`] carry;
/// * each row records a disposition from [`RP39_DISPOSITIONS`], so a pair cannot
///   be settled by a tag whose obligations nothing checks;
/// * the bucket the accounting builds is [`DECLARED_RP39`], measured from the
///   walk;
/// * and the residue still without a verdict is [`OPEN_RP39`], summed here over
///   the rows tagged `OPEN`.
#[test]
fn the_display_floors_round_trip_residue_has_its_own_owner() {
    assert_eq!(
        (
            RP39_ROUTING.iter().map(|(_, n, _, _, _)| n).sum::<usize>(),
            RP39_ROUTING.len(),
            RP39_ROUTING.iter().map(|(_, _, n, _, _)| n).sum::<usize>(),
        ),
        DECLARED_RP39,
        "the routing's three columns must sum to the bucket lock"
    );
    let corpus = Corpus::load();
    let floor = props_norm::display_floor().expect("the r4133 props display floor is derived");
    let mut seen: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for row in &corpus.rows {
        if !display_class_but_not_a_render(row) {
            continue;
        }
        let (pair, _, _, cite, disposition) = RP39_ROUTING
            .iter()
            .find(|(p, _, _, _, _)| *p == row.pair)
            .unwrap_or_else(|| {
                panic!(
                    "{} '{}' vs '{}': inside the floor, no `%.Ng` render, and no RP39_ROUTING \
                     row — the residue grew and needs its round-trip chain read off the Pascal",
                    row.pair, row.rust, row.r4133
                )
            });
        assert!(
            cite.contains(".pas:"),
            "{pair}: a round-trip residue routing must cite the r4133 site, got {cite:?}"
        );
        assert!(
            RP39_DISPOSITIONS.contains(disposition),
            "{pair}: {disposition:?} is not a recorded round-trip residue disposition — extend \
             RP39_DISPOSITIONS with the obligations that outcome owes, never loosen"
        );
        // The METRIC would have claimed it; only the mechanism clause does not.
        let rel = props_norm::display_rel(&row.rust, &row.r4133).expect("a numeric cell");
        assert!(rel <= floor, "{pair}: {rel:e} is outside the floor");
        assert!(!props_norm::under_display_floor(&row.rust, &row.r4133));
        assert_eq!(
            first_match(chain_verdicts(&corpus, row)),
            None,
            "{pair}: a round-trip residue row must be unclaimed by every link"
        );
        let ev = corpus.evidence(row).expect("a declared row has evidence");
        let e = seen.entry(pair).or_default();
        e.0 += 1;
        e.1 += usize::from(row_in_scope(row, ev));
    }
    assert_eq!(
        seen.iter().map(|(p, n)| (*p, *n)).collect::<Vec<_>>(),
        RP39_ROUTING
            .iter()
            .map(|(p, n, s, _, _)| (*p, (*n, *s)))
            .collect::<Vec<_>>(),
        "the cited pairs, row counts and in-scope splits must be exactly what the walk refuses"
    );
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(
        led.owner(Owner::Rp39),
        DECLARED_RP39,
        "the round-trip residue bucket (rows, pairs, rows on in-scope pairs)"
    );
    // …and what is still without a verdict, which is the half that shrinks.
    let open: Vec<_> = RP39_ROUTING
        .iter()
        .filter(|(_, _, _, _, d)| *d == "OPEN")
        .collect();
    assert_eq!(
        (
            open.iter().map(|(_, n, _, _, _)| n).sum::<usize>(),
            open.len(),
            open.iter().map(|(_, _, n, _, _)| n).sum::<usize>(),
        ),
        OPEN_RP39,
        "the residue still awaiting a verdict — every pair is settled, so this is \
         (0, 0, 0) while DECLARED_RP39 keeps the measured rows"
    );
}

/// **The AutoTrans typecast rows are exactly the ones the walk finds, and they
/// are NOT display-class**: the ownership guard [`RP312_UPSTREAM_BUG`] owes,
/// shaped like [`the_display_floors_round_trip_residue_has_its_own_owner`].
///
/// It asserts four things: the table's counted columns sum to
/// [`DECLARED_RP312`], the rows [`regcontrol_autotrans_typecast_row`] selects
/// are exactly the table's pairs and counts, every one of them is far
/// **outside** the display floor (a solved-state jump, not a render) and
/// unclaimed by every link of the chain, and the verdict is one an
/// [`RP312_VERDICTS`] tag covers, cited to an r4133 source line.
///
/// The minimum gap is asserted too, and **bracketed both ways**
/// ([`RP312_MIN_GAP_RATIO`]), so a re-measure that drifted anywhere near the
/// floor, or a population that moved the minimum in either direction, reds here
/// rather than quietly changing which owner holds the row.
#[test]
fn the_regcontrol_autotrans_typecast_rows_have_their_own_owner() {
    assert_eq!(
        (
            RP312_UPSTREAM_BUG
                .iter()
                .map(|(_, n, _, _, _, _)| n)
                .sum::<usize>(),
            RP312_UPSTREAM_BUG.len(),
            RP312_UPSTREAM_BUG
                .iter()
                .map(|(_, _, n, _, _, _)| n)
                .sum::<usize>(),
        ),
        DECLARED_RP312,
        "the routing's three columns must sum to the bucket lock"
    );
    let corpus = Corpus::load();
    let floor = props_norm::display_floor().expect("the r4133 props display floor is derived");
    let mut seen: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    let mut min_rel = f64::INFINITY;
    for row in &corpus.rows {
        if !regcontrol_autotrans_typecast_row(row) {
            continue;
        }
        let (pair, _, _, cite, verdict, pin) = RP312_UPSTREAM_BUG
            .iter()
            .find(|(p, _, _, _, _, _)| *p == row.pair)
            .expect("the predicate reads the table");
        assert!(
            cite.contains(".pas:"),
            "{pair}: an AutoTrans typecast routing must cite the r4133 site, got {cite:?}"
        );
        assert!(
            RP312_VERDICTS.contains(verdict),
            "{pair}: {verdict:?} is not a recorded AutoTrans typecast verdict — extend RP312_VERDICTS \
             with the obligations that outcome owes, never loosen"
        );
        assert!(!pin.is_empty(), "{pair}: the verdict owes a named witness");
        // NOT display-class, and not by a hair: the scope of these rows is
        // right, and their class is a solved-state jump.
        let rel = props_norm::display_rel(&row.rust, &row.r4133).expect("a numeric cell");
        assert!(
            rel > floor,
            "{pair}: {rel:e} is inside the display floor {floor:e} — that row is the \
             round-trip residue, not this OpenDSS bug"
        );
        min_rel = min_rel.min(rel);
        assert!(!props_norm::under_display_floor(&row.rust, &row.r4133));
        assert_eq!(
            first_match(chain_verdicts(&corpus, row)),
            None,
            "{pair}: an AutoTrans typecast row must be unclaimed by every link"
        );
        let ev = corpus.evidence(row).expect("a declared row has evidence");
        let e = seen.entry(pair).or_default();
        e.0 += 1;
        e.1 += usize::from(row_in_scope(row, ev));
    }
    assert_eq!(
        seen.iter().map(|(p, n)| (*p, *n)).collect::<Vec<_>>(),
        RP312_UPSTREAM_BUG
            .iter()
            .map(|(p, n, s, _, _, _)| (*p, (*n, *s)))
            .collect::<Vec<_>>(),
        "the cited pairs, row counts and in-scope splits must be exactly what the walk selects"
    );
    let ratio = min_rel / floor;
    assert!(
        ratio >= RP312_MIN_GAP_RATIO,
        "the smallest AutoTrans typecast gap is {min_rel:e}, only {ratio:.1}x the display floor {floor:e} \
         (locked at {RP312_MIN_GAP_RATIO:.1}x) — re-read which owner holds these rows instead \
         of leaving the split to a rounding"
    );
    // …and the lock is a MEASUREMENT, not a floor to hide behind: bracket it
    // above too, exactly as `RP24_OUT_OF_SCOPE_MIN_RATIO` had to be after a
    // mutation walked past its one-sided form.
    assert!(
        ratio < 154.0,
        "the measured minimum is 153.4x (examples_supplement.txt:127, 3.068976e-02 against the \
         2e-4 floor), got {ratio:.1}x — a different number means the population moved; \
         re-record it, do not widen the `>=`"
    );
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(
        led.owner(Owner::Rp312),
        DECLARED_RP312,
        "the AutoTrans typecast bucket (rows, pairs, rows on in-scope pairs)"
    );
    // The pair's ninth spelling stays with the round-trip residue: one row,
    // inside the floor, still declared to Owner::Rp39.
    assert_eq!(
        RP39_ROUTING
            .iter()
            .find(|(p, _, _, _, _)| *p == "autotrans.wdgcurrents")
            .map(|(_, n, s, _, d)| (*n, *s, *d)),
        Some((1, 0, "PIN")),
        "the round-trip residue keeps the makeposseq row of the same pair"
    );
}

/// **The pin quotes the vendored census cells verbatim, and exactly two of the
/// AutoTrans typecast rows.**
///
/// [`the_regcontrol_autotrans_typecast_rows_have_their_own_owner`] proves WHICH
/// rows the verdict owns, and the pin's own `assert_eq!`s prove its literals are
/// this engine's live renders. What neither proves is the pin's remaining
/// claim: that the string it reaches with the regulator disabled is *r4133's
/// census cell, byte for byte*. Without this test a mistyped digit in the r4133
/// side would make the pin red only if this engine happened to disagree with
/// the typo.
///
/// So: flatten the pin file's Rust string continuations and count the
/// [`DECLARED_RP312`] rows whose **both** columns appear in it verbatim. Two do
/// (`autotrans_reg` and `midi_autotrans`, the pin's two legs), and the count is
/// locked, so a re-baselined literal drops it to 1 and reds here.
///
/// The other spellings (the two `*_both` decks among them) are **declared but
/// not reproduced in-tree**: their decks open with a snapshot `Solve` before the
/// daily loop, so reproducing r4133's unregulated state needs the RegControl
/// disabled in the deck SOURCE rather than by an `edit` after the compile, a
/// deck-copy construction the pin harness does not have.
#[test]
fn the_rp312_pin_quotes_the_vendored_census_cells() {
    let text = std::fs::read_to_string(repo_root().join(PINS))
        .unwrap_or_else(|e| panic!("read {PINS}: {e}"))
        .replace("\r\n", "\n");
    // A trailing `\` in a Rust string literal swallows the newline AND the next
    // line's leading whitespace, so the census cell is one flat run of bytes.
    let mut flat = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\\' && it.peek() == Some(&'\n') {
            it.next();
            while it.peek() == Some(&' ') {
                it.next();
            }
        } else {
            flat.push(c);
        }
    }
    let corpus = Corpus::load();
    let witnessed: Vec<&str> = corpus
        .rows
        .iter()
        .filter(|row| regcontrol_autotrans_typecast_row(row))
        .filter(|row| flat.contains(&row.rust) && flat.contains(&row.r4133))
        .map(|row| row.r4133.as_str())
        .collect();
    assert_eq!(
        witnessed.len(),
        RP312_WITNESSED_ROWS,
        "the pin must quote BOTH columns of exactly {RP312_WITNESSED_ROWS} of the \
         {DECLARED_RP312:?} declared rows, verbatim; it quotes {} ({witnessed:?}). A literal \
         edited on the r4133 side stops being the vendored census cell and the pin's \
         'byte for byte' claim stops being true — re-derive it from \
         examples_supplement.txt, do not re-baseline it",
        witnessed.len()
    );
}

/// **The AutoTrans typecast routing is pinned literally**, like [`NOT_A_PIN`],
/// [`LEDGER_ENTRY_PINS`] and [`RP39_PINS`]: it is an exemption from "every pin in
/// [`PINS`] is named by an echo row", and an exemption that grows by iteration
/// would let an un-cited `#[test]` through.
///
/// The literals are the whole verdict (which pair, how many rows, how many of
/// them the r4133 compare reaches, the r4133 site, the outcome and the witness),
/// so a silent edit to any of the six columns fails here first.
#[test]
fn the_autotrans_typecast_bug_list_is_pinned() {
    assert_eq!(
        RP312_UPSTREAM_BUG,
        [(
            "autotrans.wdgcurrents",
            8,
            0,
            "RegControl.pas:1026/:1296/:1479 `TTransfObj(ControlledElement)` over \
             AutoTrans.pas:88 `TAutoTransObj = class(TPDElement)`; the zeroed increment at \
             RegControl.pas:1249-1250",
            "UPSTREAM_BUG",
            "autotrans_wdgcurrents_stay_regulated_where_r4133_never_taps_the_autotrans",
        )],
        "one pair, one verdict — the 8 controls:autotrans/* spellings of \
         autotrans.wdgcurrents, 34 cells, none in scope"
    );
    assert_eq!(
        RP312_VERDICTS,
        ["UPSTREAM_BUG"],
        "the AutoTrans typecast bug records exactly the outcome it measured"
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
/// declaration rule can place is reported by name (the kill criterion). The
/// probe is a realistic one: a genuine value difference inside a bin-4 pair
/// whose in-scope cells nothing routes.
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

/// **Every S6 and bin-3 pair is claimed or routed, with no third state**: the
/// accounting claims bin 3 fully, and every routing row cites its source line.
///
/// The closed input list is `RP22_S6` ∪ the eight bin-3 pairs. Each of them ends
/// in exactly one of two places, and both are asserted here rather than left to
/// be read off the master accounting:
///
/// * **claimed** by a shipped `PROPS_NORM_R4133` row (every one of its example
///   rows), i.e. `RP22_ROUTING` says nothing about it: the five `EnumSynonym`
///   pairs (`invcontrol.voltage_curvex_ref` included, from
///   `RP22_BEYOND_THE_CLOSED_LIST`) and the two fully-folding `ArrayForm` ones;
/// * **routed** by `RP22_ROUTING`, with a citation, to the echo table or a
///   live-state divergence: the closed-list pairs plus the ones beyond it.
///
/// The test also runs the two liveness directions of any table: no routing row
/// is dead (each declares at least one example row), and no routing row names a
/// pair outside the closed list.
#[test]
fn every_s6_and_bin3_pair_is_claimed_or_routed() {
    let corpus = Corpus::load();
    let bins = bins_evidence();

    // The closed list: the S6 list plus every structural bin-3 pair.
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
            "{pair} IS on the closed S6/bin-3 list — it does not belong in the 'beyond' list"
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
            "{pair}: no bin-3 cell, so it has no reason to be on the 'beyond' list"
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
    // on the closed list (`generator.dynout` is routed through
    // `CELL_DISPOSITION` instead, it is not an S6 pair).
    for (pair, owner, cite) in RP22_ROUTING {
        assert!(
            handed.contains(*pair),
            "{pair} is routed but was never on the closed list"
        );
        assert!(
            cite.contains(".pas:"),
            "{pair}: the routing row must cite an r4133 source line, got {cite:?}"
        );
        assert!(
            matches!(owner, Owner::Rp23 | Owner::Rp35),
            "{pair}: the outcomes are an EnumSynonym row (then it is CLAIMED, not routed), \
             an echo row, or a live-state divergence — not {}",
            owner.tag()
        );
        if matches!(owner, Owner::Rp35) {
            assert!(
                cite.starts_with("FIX — "),
                "{pair}: a live-state routing must open with its outcome `FIX — `, got {cite:?}"
            );
        }
    }

    // The pairs RP22_ROUTING does NOT route are exactly the ones the shipped
    // table claims outright, asserted through the shipped predicate, per example
    // row.
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
        "the S6/bin-3/off-bin pairs left to the normalization table"
    );
    for pair in &claimed_outright {
        let rows: Vec<&Example> = corpus.rows.iter().filter(|r| &r.pair == pair).collect();
        assert!(!rows.is_empty(), "{pair}: no example row at all");
        for r in rows {
            assert!(
                props_norm::claiming_row(&r.class, &r.prop, &r.rust, &r.r4133).is_some(),
                "{pair} '{}' vs '{}' is neither claimed nor routed — it owes a \
                 verdict",
                r.rust,
                r.r4133
            );
        }
    }

    // Liveness: every routing row really routed something, and the echo table
    // CONSUMED the half addressed to it. Each outcome is checked against what
    // its own verdict promised:
    //
    //  * `Owner::Rp23`: the promise is a cited exclusion row, so the pair must
    //    HAVE one. It is not enough for the row to have left the bucket, the
    //    table has to name the pair.
    //  * `Owner::Rp35`: the pair must still leave at least one example row
    //    unclaimed BY THE CHAIN. The row it finds may be declared or superseded
    //    ([`RP37_SUPERSEDED`]). What would be stale is a chain link claiming the
    //    pair outright.
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
        "{unconsumed:?} are routed to the echo table and no row cites them — either the \
         routing is wrong or the echo table dropped the pair"
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
        "the fourteen pairs routed to the echo table, all of them cited echo rows"
    );
}

/// The enumerated pair lists still describe the vendored evidence: the eight
/// bin-3 pairs, the sixteen in-scope bin-7 pairs split 12 echo / 4 root-cause,
/// and the S6 singletons. A stale list here would silently move rows between
/// owners, so it is checked against `bins.tsv` rather than trusted.
#[test]
fn the_enumerated_pair_lists_still_describe_the_vendored_evidence() {
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
        "the eight bin-3 pairs"
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
        "16 in-scope bin-7 pairs, 12 echo + 4 root-cause"
    );

    let corpus = Corpus::load();
    let known = |pair: &str| bins.contains_key(pair) || corpus.supplement.contains_key(pair);
    for pair in RP22_S6 {
        assert!(
            known(pair),
            "{pair} is on the S6 list but has no evidence row"
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
            "{pair} carries a routing verdict but has no evidence row"
        );
    }
}
