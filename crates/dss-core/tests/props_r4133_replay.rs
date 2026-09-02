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
//!  PROPS_015X          PROPS_NORM_R4133   PROPS_ECHO_R4133   2e-4 relative
//!  (RP1)               (RP2.1 + RP2.2)    (RP2.3)            (RP2.4)
//! ```
//!
//! Since **RP2.4** all four links carry a value, so the accounting is no longer
//! "two live links and two named slots": every example row is claimed by one of
//! them, declared to a sub-step that is itself still open (RP3, RP3.5+, RP3.9)
//! or to `OutOfScope`, or — since RP3.8 changed the engine under five pairs —
//! counted as **superseded**, the third state ([`RP38_SUPERSEDED`]).
//! `DECLARED_RP22`/`RP23`/`RP24`/`RP38` are all `(0, 0, 0)`.
//!
//! Every row ends up in exactly one of two states, and **the accounting is
//! total from day one**:
//!
//! * **claimed** — the first matching link of the chain recognises the two
//!   spellings as one value (RP2.1's own deliverable, bins 1/2/4, plus RP2.2's
//!   `EnumSynonym` rows for bin 3);
//! * **superseded** — an engine change made the frozen `(rust, r4133)`
//!   spelling counterfactual, so neither a link nor a sub-step can be asked
//!   about it; the row is counted against a cited table whose evidence is live
//!   ([`RP38_SUPERSEDED`], the only one today);
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
/// **The echo table** — example rows the exclusion claims, i.e. the ones
/// `PROPS_ECHO_R4133` covers that no earlier link took. 450 (the RP2.3 bucket)
/// − 99 (claimed by the nine new normalization rows instead) − 181 (the five
/// `SilentReadOnly` pairs the kill ruling re-routed, now superseded —
/// [`RP38_SUPERSEDED`]) − 1 (the audit
/// settlement's carve-out, [`ECHO_CARVE_OUT_ROUTING`]) = 169 for RP2.3's 81
/// rows, **+1 for RP3.3's 82nd** (`generator.model`, whose single example row
/// `'4'` vs `'3'` leaves [`Owner::Rp3`] for this link) = **170**.
///
/// RP3.3's `+1` is the first time this number moved for a reason other than a
/// re-partition inside RP2.3's own bucket: a WP-RP3 sub-step root-caused a bin-7
/// pair to the echo mechanism and landed the row, so the chain now claims a row
/// that no link claimed before (see [`DECLARED_RP3`], which loses it).
const CLAIMED_ECHO: usize = 170;
/// …and in total, over all four links.
const CLAIMED_TOTAL: usize =
    CLAIMED_SHAPE_ALLOWLIST + CLAIMED_NORMALIZATION + CLAIMED_ECHO + CLAIMED_DISPLAY_FLOOR;

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

/// **The display floor's own live-only spellings** — [`LIVE_ONLY_SPELLINGS`]'
/// class, measured for RP2.4's link by its full claims census (`README.md`
/// §"What RP2.4 moved"): six `%[-].Ng` renders that exist live and that no
/// vendored file may carry, because all five pairs already hold a frozen
/// `bins.tsv` row (all bin 6) and the supplement's own lock bars a pair from
/// both files.
///
/// They are the same population drift RP2.1/RP2.2 recorded as "+2 cells on each
/// `vsource` pair", seen at spelling granularity: the decks WP-RP1 added read
/// `Vsource`'s `Isc*`/`R1`/`X0`/`X1` getters at source impedances the 2026-08-08
/// walk never saw. Every one is inside its own pair's frozen `max_rel` (both
/// facts asserted, [`LIVE_ONLY_DISPLAY_PAIRS`] and `frozen_max_rel`), and the
/// widest is 2.51e-05 — a quarter of the floor and well under the derivation's
/// 6.431124e-05 worst, so the live population moves no part of it. All six pass
/// the mechanism clause; they are renders, not RP3.9's residue.
///
/// Recorded as an asserted term rather than left as drift (the rule
/// [`LIVE_ONLY_SPELLINGS`] was created under, RP2.1 audit round): without it the
/// census's 2 981 claimed spellings and the replay's [`CLAIMED_TOTAL`] would
/// disagree by six with nothing to say why.
const LIVE_ONLY_DISPLAY_SPELLINGS: &[(&str, &str, &str)] = &[
    // (the five pairs below are [`LIVE_ONLY_DISPLAY_PAIRS`])
    ("vsource.isc1", "104347.826086957", "1.0435E005"),
    ("vsource.isc1", "69565.2173913044", "69565"),
    ("vsource.isc3", "45183.9341104925", "45184"),
    ("vsource.r1", "0.3563926267895", "0.35639"),
    ("vsource.x0", "1.92015184059109", "1.9202"),
    ("vsource.x1", "1.425570507158", "1.4256"),
];

/// The pairs [`LIVE_ONLY_DISPLAY_SPELLINGS`] may name — the five `Vsource`
/// getters the recorded "+2 cells per vsource pair" drift touches, and nothing
/// else. Asserted, so the list cannot quietly grow a pair whose live population
/// was never measured.
const LIVE_ONLY_DISPLAY_PAIRS: &[&str] = &[
    "vsource.isc1",
    "vsource.isc3",
    "vsource.r1",
    "vsource.x0",
    "vsource.x1",
];

/// Claimed **spellings** the full claims census measures on the r4133 channel
/// (439 cases × 2 channels, 2026-08-23, post-RP2.4) — the live counterpart of
/// [`CLAIMED_TOTAL`], reconciled by the two live-only lists above.
///
/// **2 982 since RP3.3** (2026-08-24), from the RP2.4 audit settlement's 2 981:
/// that settlement took 3 036 down to 2 981 because the floor's mechanism clause
/// un-claims the 55 [`RP39_ROUTING`] spellings, live exactly as offline
/// (vendored `README.md` §"What the RP2.4 audit settlement moved"), and RP3.3's
/// `generator.model` echo row adds the one spelling back — **re-measured**, not
/// bumped: the census walk of 2026-08-24 (`DSS_PROPS_CENSUS=claims`) reports the
/// pair's two live cells as `echo-row` and its claimed-spelling total as 2 982.
const CLAIMED_SPELLINGS_LIVE: usize = 2982;

/// **RP3.9 — the round-trip residue RP2.4's mechanism clause refuses**:
/// `(pair, example rows, r4133 site)`.
///
/// The display floor claims a cell only when the r4133 side is our value rounded
/// to the digits r4133 printed (`props_norm::display_is_render`). These 27 pairs
/// carry 55 spellings that are inside the floor and are NOT such a render: the
/// two engines hold doubles further apart than one `%.Ng` print can account for,
/// because the round trip happened upstream of a derived quantity (r4133's
/// `load.kva` is recomputed from an already round-tripped `pf`; `vsource.puz*`
/// from a round-tripped Z; `line.b0`/`b1` from a round-tripped C) or because
/// r4133's own getter prints at full precision and the two values simply differ
/// (`Format('%-g')` / `Format('%g')` — `load.*`, `capacitor.normamps`,
/// `reactor.normamps`).
///
/// **They were a work list, not a disposition — and RP3.9 settled all 27**
/// (2026-09-02). RP2.4's part-A survey claimed them as display cells and the
/// audit round disproved it (both majors, 2026-08-23): each pair needed the
/// round-trip chain read off the Pascal and then either an expected-value pin or
/// a ledger entry, exactly like the RP3.1-8 sub-steps. Every one came back
/// §RP3.9 outcome 1, `PRECISION_ROUNDTRIP` — r4133's number is reproducible from
/// the port's own state through a cited Pascal round trip and the port is exact
/// — so every row now carries the `PIN` disposition and names its pin in
/// [`RP39_PINS`]. **No cell of any of them is in scope today** — the full claims
/// census measures `count_in_scope = 0` on all 55 spellings (70 cells), which is
/// why the unmask is not blocked on the finding, and why not one pair owes a
/// staged `property` entry — but RP4.1 stays gated on RP3.9 all the same
/// (plan §0), because scope is a property of today's manifest and the divergence
/// is a property of the engines.
///
/// Columns: `(pair, example rows, rows on in-scope pairs, r4133 site,
/// disposition)`, the last per [`RP39_DISPOSITIONS`]. The three counted columns
/// do **not** drop to zero when a pair settles — they are re-measured from the
/// walk, and nothing a pin does makes a link claim the row. What shrinks is
/// [`OPEN_RP39`], `(55, 27, 19)` -> `(0, 0, 0)`.
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
/// **RP2.4's display floor** — example rows the floor claims that no earlier
/// link took. **2 006** since RP2.4 (it was 0 while the slot was `None`):
///
/// * **1 996** of the 2 100 rows RP2.1's bin-6 walk declared to
///   [`Owner::Rp24`], plus the 1 [`ECHO_CARVE_OUT_ROUTING`] declared to it —
///   that bucket's whole population minus the 105 [`RP24_OUT_OF_SCOPE`] rules
///   on (see [`DECLARED_RP24`], now `(0, 0, 0)`);
/// * **10** rows the accounting had filed `OutOfScope` on four display-class
///   pairs whose in-scope population is empty (`generator.kw`,
///   `capacitor.normamps`/`emergamps`, `line.r1`/`x1`/`rmatrix`/`xmatrix`,
///   `autotrans.wdgcurrents`). A *claim* is strictly better than a scope
///   excuse: the floor says what those two spellings are, where §1.3 only said
///   the unmask never looks at them. [`DECLARED_OUT_OF_SCOPE`] moves with it.
///
///   **Correction, RP3.12 (2026-09-03): `autotrans.wdgcurrents` is a
///   display-class pair for exactly ONE of its nine spellings.** The row this
///   bullet moved into the floor is the `modes:makeposseq` residue (rel 1.1e-05,
///   the settlement below then hands it to [`Owner::Rp39`]); the pair's other
///   **8** spellings — the four `controls:autotrans/*` decks, 34 cells — are 3–7.5 %
///   apart and could never be claimed by any floor. They are a solved-state
///   upstream bug and are declared to [`Owner::Rp312`] now
///   ([`RP312_UPSTREAM_BUG`]). This count is unaffected either way: the floor
///   claims what its predicate claims.
///
/// No row claimed by an earlier link is also inside the floor
/// ([`MULTI_LINK_ROWS`] is unchanged at 135), which is why this number is a
/// clean addition to [`CLAIMED_TOTAL`] rather than a re-partition.
///
/// **1 951 since the RP2.4 audit settlement** (2026-08-23), from 2 006: the
/// floor's mechanism clause (`props_norm::display_is_render`) refuses the **55**
/// spellings whose r4133 side is no `%.Ng` render of our value, and they are
/// declared to [`Owner::Rp39`] instead ([`RP39_ROUTING`]). Both auditors
/// measured that population independently and reached the same 55/70 cells; the
/// clause makes the refusal a property of the predicate rather than of a survey.
const CLAIMED_DISPLAY_FLOOR: usize = 1951;

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
/// * **169** claimed by the 81 rows RP2.3 landed in `PROPS_ECHO_R4133` — 169 of
///   [`CLAIMED_ECHO`]'s 170, the odd one being RP3.3's own row, which came out of
///   [`Owner::Rp3`]'s bucket and not out of this one;
/// * **181** re-routed to [`Owner::Rp38`] by the kill ruling, and since RP3.8
///   landed accounted as superseded ([`RP38_SUPERSEDED`]);
/// * **1** carved back out of its row and declared to RP2.4 by the audit
///   settlement ([`ECHO_CARVE_OUT_ROUTING`]).
///
/// The variant stays so a regression that re-creates the bucket fails here by
/// name.
const DECLARED_RP23: (usize, usize, usize) = (0, 0, 0);
/// **`DECLARED_RP38` is `(0, 0, 0)` since RP3.8 closed** (2026-09-02), and the
/// bucket did not empty by re-labelling: the sub-step changed the ENGINE, so the
/// frozen `rust = ''` column of its 181 rows describes a port that no longer
/// exists.
///
/// It inherited `(181, 5, 181)` from the RP2.3 kill ruling. Those rows are now
/// accounted as **superseded** ([`SUPERSEDED_RP38`], [`RP38_SUPERSEDED`]): the
/// port renders the same live quantity r4133 does, so on the r4133 channel —
/// the only one this file's chain speaks for — the cells either agree outright
/// or are the ordinary display class [`Link::DisplayFloor`] claims, and the
/// capi-side `number` vs `''` is excluded by a `SKIP_PROPS_CAPI_ONLY` row pair
/// with its own expected-value pins. What replaces the offline replay of a
/// stale spelling is the LIVE measurement quoted at [`RP38_SUPERSEDED`].
///
/// The variant stays so a regression that re-creates the bucket fails here by
/// name.
const DECLARED_RP38: (usize, usize, usize) = (0, 0, 0);
/// **The frozen rows RP3.8's engine change superseded** — `(rows, pairs, rows on
/// in-scope pairs)`, the same triple every `DECLARED_*` lock carries, counted
/// over exactly the [`RP38_SUPERSEDED`] pairs.
///
/// It is [`DECLARED_RP38`]'s old value, moved rather than deleted: 181 rows / 5
/// pairs / 181 in-scope rows. A row that leaves or joins these five pairs moves
/// this number, so the sub-step cannot quietly shrink the population it was
/// accountable for.
const SUPERSEDED_RP38: (usize, usize, usize) = (181, 5, 181);
/// **`DECLARED_RP24` is `(0, 0, 0)` since RP2.4 closed**, and that zero is the
/// sub-step's own acceptance. It inherited `(2101, 71, 2021)` — 2 100 rows over
/// 70 pairs from RP2.1's bin-6 walk plus the cell [`ECHO_CARVE_OUT_ROUTING`]
/// carved out of `reactor.kvar`'s echo row — and every one of those 2 101 rows
/// is now resolved, in exactly two ways:
///
/// * **1 996** claimed by the derived floor itself
///   ([`CLAIMED_DISPLAY_FLOOR`]), the carve-out among them;
/// * **105** re-declared to [`Owner::OutOfScope`] by [`RP24_OUT_OF_SCOPE`],
///   which proves per row — from the pair's own frozen `max_rel_in_scope` —
///   that the spelling has no cell the RP4.1 unmask will ever compare.
///
/// The variant stays so a regression that re-creates the bucket fails here by
/// name.
const DECLARED_RP24: (usize, usize, usize) = (0, 0, 0);
/// **WP-RP3's root-cause residue** — **`(6, 3, 6)` since RP3.3
/// (2026-08-24)**, from the `(7, 4, 7)` the work package inherited: 7 example
/// rows over 4 pairs, all of them on pairs the RP4.1 unmask will compare. The
/// per-pair split, each sub-step's verdict and what it landed are
/// [`RP3_ROUTING`].
///
/// **RP3.3 is the one sub-step so far that shrinks it, and the reason is the
/// outcome tag, not the progress.** Its exclusion is a `PROPS_ECHO_R4133` row
/// (`generator.model`, `EchoParse`), which the tree holds *now* — so
/// [`Link::Echo`] claims the pair's one example row and [`declare`] never sees
/// it. The bucket loses `(1, 1, 1)` because something really claims those rows,
/// which is exactly the condition the note below reserves the shrink for. The
/// three columns are `(rows, pairs, rows on in-scope pairs)`, and `pairs` is the
/// count of pairs with a row *left* — 3 — while [`RP3_ROUTING`] keeps all four
/// entries so the table still covers every [`BIN7_ROOT_CAUSE`] pair; the routing
/// guard compares against the entries that still declare rows.
///
/// **Unchanged by RP3.1, RP3.2 and RP3.4 (all 2026-08-24), deliberately.** Those
/// sub-steps root-caused `swtcontrol.delay`, `windgen.kvar` and
/// `gictransformer.r2`, reported the first two upstream (the third was already
/// reported against r4133) and landed their two, four and two expected-value
/// pins — but each
/// exclusion is a `ledger.json` `property` entry, and §1.1(e) stages every such
/// entry into RP4.1's unmask commit. Until that commit the tree holds no
/// exclusion for these rows, so they stay *declared*: this bucket is what a
/// sub-step **inherits**, not a progress bar. With RP3.4 all four sub-steps have
/// run and the bucket still holds `(6, 3, 6)` — the clearest statement there is
/// that this number tracks the tree, not the work.
///
/// **The shrink is a hand edit at RP4.1, not a consequence of landing the
/// entries** (RP3.1 audit settlement, 2026-08-24 — the earlier wording, "may
/// only shrink when something in the tree actually claims the rows", implied a
/// self-correction this file cannot perform). Nothing in the chain reads
/// `tests/corpus/ledger.json`: it is [`Link::ORDER`]'s four links,
/// [`chain_verdicts`] evaluates exactly those four, and [`declare`] routes every
/// bin-7 row on [`BIN7_ROOT_CAUSE`] to [`Owner::Rp3`] unconditionally. So when
/// RP4.1 lands the two staged entries this constant will **not** move on its
/// own and nothing here would notice; that commit must retire the rows — here
/// and in [`RP3_ROUTING`] — by hand. What makes the obligation unmissable is
/// [`the_staged_r4133_property_entries_have_not_landed_yet`], which reds the
/// moment a `property`-scoped `r4133` entry appears in the ledger and carries
/// the instruction in its message.
const DECLARED_RP3: (usize, usize, usize) = (6, 3, 6);
/// **RP3.9 — the round-trip residue the RP2.4 audit settlement opened**: the 55
/// example rows over 27 pairs whose gap is inside the floor and whose r4133 side
/// is no `%.Ng` render of our value ([`RP39_ROUTING`], which carries the per-pair
/// split and the sites). The third term is the rows on pairs the RP4.1 unmask
/// compares and that the frozen ceilings do not prove out of scope; the LIVE
/// census measures 0 in-scope cells on all 55 (README §"What the RP2.4 audit
/// settlement moved").
///
/// **Unchanged by RP3.9's settlement (2026-09-02), deliberately** — the same
/// reading [`DECLARED_RP3`]'s and [`DECLARED_RP35`]'s notes spell out. This
/// bucket is *measured*: [`account`] walks the corpus and counts every row
/// [`declare`] routes to [`Owner::Rp39`], and what routes a row there is
/// `display_class_but_not_a_render` — a property of the two engines' doubles,
/// which an expected-value pin does not touch. All 27 pairs are settled and the
/// port was proven exact on every one, so no `rust` spelling moved either; a
/// smaller number here would be a claim the tree does not hold. The shrink the
/// plan's acceptance asks for is [`OPEN_RP39`], and RP4.1 retires these rows by
/// hand with the unmask.
const DECLARED_RP39: (usize, usize, usize) = (55, 27, 19);

/// **What is still OPEN in the round-trip residue** — `(rows, pairs, rows on
/// in-scope pairs)` summed over the [`RP39_ROUTING`] entries whose disposition is
/// `OPEN`, i.e. the pairs RP3.9 has not yet given a verdict.
///
/// **`(0, 0, 0)` since 2026-09-02, from `(55, 27, 19)`**: the sub-step read every
/// chain off the Pascal and settled all 27 pairs as `PIN`. Those are the deltas
/// the plan's acceptance asks STATUS to state — of *open* residue, not of
/// declared rows ([`DECLARED_RP39`] explains why the measured bucket cannot move
/// and who retires it).
const OPEN_RP39: (usize, usize, usize) = (0, 0, 0);

/// **RP3.12 — the RegControl×AutoTrans typecast bug** ([`RP312_UPSTREAM_BUG`]):
/// the 8 `autotrans.wdgcurrents` spellings the four `controls:autotrans/*` decks
/// carry, **34 census cells, none in scope**.
///
/// `(8, 1, 0)` from 2026-09-03, taken out of [`DECLARED_OUT_OF_SCOPE`] — where
/// RP2.4's accounting had left them under the bin-7 "no in-scope cell" arm, and
/// where the display floor's own doc still listed the pair among four
/// *display-class* ones. Neither reading was wrong about the scope and both were
/// wrong about the class: the gap is 3–7.5 %, two to three orders of magnitude
/// outside the floor, and it is a **solved-state** divergence — r4133's
/// `RegControl` cannot tap an `AutoTrans` at all, so its cells are the
/// UNREGULATED circuit's. A *verdict* is strictly better than a scope excuse, the
/// same reason RP2.4 gave for preferring a claim to §1.3.
///
/// The count is a straight sum of the vendored extract's rows
/// (`tests/corpus/props_r4133/examples_supplement.txt:127-134`, cells
/// 9+8+3+3+3+3+3+2 = 34); `:135` is the ninth spelling of the same pair — the
/// `modes:makeposseq` round-trip residue — which stays [`Owner::Rp39`]'s and is
/// counted in [`DECLARED_RP39`]. The pair's live census reads
/// `35 cells / 0 in scope / 9 spellings`
/// (`tmp/props_census/r4133/claims_unclaimed_pairs.txt:8`), 34 + 1.
///
/// Nothing in the tree can shrink this on its own: the four cases are
/// `engines: "capi_v0145"` (`population.lock.json:74-77`), so no `r4133`
/// comparison runs on them and the case-level `skip` entries this verdict owes
/// are **staged, not landed** (`tmp/rp312/staged_ledger.md`) — exactly the
/// §1.1(e) window [`DECLARED_RP3`]'s note describes. RP4.1 retires the rows by
/// hand with the unmask.
const DECLARED_RP312: (usize, usize, usize) = (8, 1, 0);

/// The **disposition** an [`RP39_ROUTING`] row records — what the settled pair
/// owes the tree, in the shape [`RP3_SETTLED_SHAPES`] carries for
/// [`RP3_ROUTING`]:
///
/// * `PIN` — §RP3.9 outcomes 1 and 2 (`PRECISION_ROUNDTRIP`, `STATE_DIFFERS`):
///   the port is exact, or differs by a proven precision-class cause, and the
///   whole obligation is an expected-value test naming both numbers. It is cited
///   in [`RP39_PINS`], whose own verdict column records which of the two it is.
/// * `FIX` — §RP3.9 outcome 3 (`PORT_BUG`): the port computes the wrong number
///   and is fixed in both lanes, witnessed beside the behaviour.
/// * `LEDGER` — §RP3.9 outcome 4 (`UPSTREAM_BUG`): never reproduced, so the
///   r4133 channel is excluded by a `property` entry staged into RP4.1 per
///   §1.1(e), with its pin in [`LEDGER_ENTRY_PINS`].
/// * `OPEN` — no verdict yet; every row carried this until 2026-09-02.
///
/// §RP3.9's fifth outcome, `KILL`, is deliberately not a tag: it is a stop, not a
/// disposition — the pair leaves this sub-step for a root-cause one of its own,
/// and the plan says to report it rather than record it here.
const RP39_DISPOSITIONS: &[&str] = &["PIN", "FIX", "LEDGER", "OPEN"];
/// The three sub-steps RP2.2 opened: **8 rows over 6 pairs, 5 of them in
/// scope** — RP3.5 `line.units` (1 row, 0 in scope), RP3.6 `line.linecode`
/// (2 rows, both in scope — the only RP3.5+ pair the RP4.1 unmask will actually
/// compare), RP3.7 `swtcontrol.normal`/`state` (1 + 2 rows, all in scope; their
/// one-token spellings are claimed by `ArrayForm` and are not here) and
/// `relay.normal`/`state` (1 row each, both out of scope). RP4.1 does not start
/// until all three close (plan §0).
///
/// **All three have now closed — RP3.5 (2026-08-28), RP3.6 (2026-08-29) and
/// RP3.7 (2026-09-02) — and the constant has NOT moved.** That is the same
/// reading [`DECLARED_RP3`]'s note spells out: this number tracks the tree, not
/// the work. All three settled `FIX`, so the port now renders r4133's own value
/// on every one of these pairs and there is nothing left to exclude on the
/// r4133 channel — but the vendored extracts are a **data lock** recording the
/// 2026-08-08 measurement, so their `rust` columns still hold the PRE-FIX
/// spellings (`'none'`, `''`, `'closed'`, `'open'`, `'[closed, open, open, ]'`)
/// and no link of the chain claims them. The rows retire by hand at RP4.1,
/// where the live compare finally sees the fixed renders; until then the
/// verdicts in [`RP22_ROUTING`] carry what was decided, for whom, and what
/// holds each port value meanwhile (`exec::tests::line_fetch::…`,
/// `exec::tests::reduce::…`, `exec::tests::controls::…`, `relay::tests::…`).
/// What each fix DID move is the live `capi_v0145` channel — 1 + 2 + 5 landed
/// property entries, all outside this accounting
/// ([`LANDED_PROPERTY_ENTRY_PINS`]).
const DECLARED_RP35: (usize, usize, usize) = (8, 6, 5);
/// `OutOfScope` rows must have **zero** in-scope cells — that is the whole
/// claim the marker makes (plan §1.3).
///
/// **`(229, 22, 0)` at RP2.4**, from RP2.3's `(134, 18, 0)`: **−10** rows the
/// floor now *claims* outright instead (see [`CLAIMED_DISPLAY_FLOOR`]) and
/// **+105** rows over 4 pairs re-declared here by [`RP24_OUT_OF_SCOPE`]. The
/// third number is still zero, and after RP2.4 it is a *stronger* zero: it is
/// no longer read off the pair's `cells_in_scope` alone but per row, which is
/// what admits the four display-class pairs whose PAIR is in scope while these
/// particular spellings are not.
///
/// **`(221, 21, 0)` since RP3.12** (2026-09-03): **−8** rows, and with them the
/// whole `autotrans.wdgcurrents` pair, re-declared to [`Owner::Rp312`]
/// ([`DECLARED_RP312`], [`RP312_UPSTREAM_BUG`]). Those eight sat here on the
/// bin-7 `!in_scope()` arm — true about the scope, silent about the cause — and
/// RP3.12 measured the cause: r4133's `RegControl` reads a `TAutoTransObj`
/// through an unchecked `TTransfObj` cast and never taps it, so those cells are
/// its UNREGULATED circuit. The zero third column is unchanged, because a
/// verdict does not create scope.
const DECLARED_OUT_OF_SCOPE: (usize, usize, usize) = (221, 21, 0);

/// **The four pairs whose example rows RP2.4 re-declares out of scope, with the
/// frozen ceiling that proves it** — `(pair, max_rel_in_scope, rows)`.
///
/// These are exactly the four the vendored `README.md`'s last data trap names:
/// pairs whose FULL-census bin is 7 but whose in-scope re-derivation
/// (`max_rel_in_scope < 1e-4`) is bin 6, so [`PairEvidence::effective_bin`]
/// routes the whole pair to the display floor. Their example-row inventory,
/// however, is the full census's, and it therefore also carries the spellings
/// that made the pair bin 7 in the first place — up to rel 1.0 on
/// `generator.kvar`, which no display floor may claim.
///
/// **The rule that resolves them, and why it is a proof and not a shrug.**
/// `max_rel_in_scope` is the maximum relative gap over *exactly* the cells the
/// RP4.1 unmask compares (vendored `README.md` §"The in-scope filter": cells on
/// `engines in {both, r4133}` cases). So a spelling whose own gap **exceeds**
/// that maximum cannot sit on a single in-scope cell — it would have raised the
/// maximum. The rule is [`row_out_of_scope_by_ceiling`], and it is applied only
/// on these four cited pairs, only to numeric rows, and with a margin: the
/// extracts round `max_rel_in_scope` to two decimals (`%.2e`), so the
/// comparison carries [`CEILING_ROUND_MARGIN`], and the *measured* minimum
/// ratio across all 105 rows is asserted separately
/// ([`RP24_OUT_OF_SCOPE_MIN_RATIO`], 277x) so a future population that comes
/// anywhere near the ceiling reds this file instead of silently widening §1.3.
///
/// | pair | ceiling | rows | what the above-ceiling spellings are |
/// |---|---|---|---|
/// | `generator.kvar` | 1.55e-06 | 102 | the GenDispatcher `weights` registration bug's dispatch split, on the three capi-only `controls:gendispatcher/*` decks (vendored `README.md` §RP1.4) |
/// | `capacitor.cuf` | 4.00e-06 | 1 | `'[ 4]'` vs `'[ 4E-006]'` — the G2.5 `makeposseq_shunt` Cuf **unit** divergence (`triage.md` §N2) |
/// | `storage.kw` | 4.90e-06 | 1 | `'-0.333333333333333'` vs `'-1'` |
/// | `storagecontroller.kwneed` | 4.96e-06 | 1 | 1.374769e-03 — display-class in mechanism (the `README.md` says so) but out of scope, and the nearest row above RP2.4's empty band |
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

/// **The work list under [`Owner::Rp3`]'s single bucket** —
/// `(pair, sub-step, example rows, rows on in-scope pairs, verdict)`.
///
/// [`BIN7_ROOT_CAUSE`] names the four pairs [`declare`] routes to RP3; this is
/// the per-pair accounting underneath that one number, in the shape
/// [`RP38_SUPERSEDED`] and [`RP39_ROUTING`] use for the sub-steps they opened.
/// Each
/// row's three counted columns are re-measured from the walk, so a pair that
/// silently changes size (or vanishes) reds here instead of being absorbed by
/// the bucket total.
///
/// **A settled sub-step does not empty its rows unless the tree really holds the
/// exclusion — which depends on the outcome tag, not on the sub-step being
/// done.** RP3.1, RP3.2 and RP3.4 are root-caused, reported and pinned, yet their
/// spellings stay declared to RP3: the artifact that will finally exclude them
/// is a `ledger.json` `property` entry, and by the §1.1(e) staging rule that
/// entry lands in RP4.1's unmask commit, not here. Writing "claimed" while the
/// tree holds no exclusion would be exactly the silent-progress claim this
/// accounting exists to prevent — so their rows keep their counts and the
/// verdict column carries what was decided, for whom, and where it lands.
/// **Their rows will not leave this table on their own when the entries land**:
/// no link of the chain reads the ledger ([`DECLARED_RP3`]'s note), so RP4.1
/// retires them here by hand and
/// [`the_staged_r4133_property_entries_have_not_landed_yet`] is the tripwire
/// that says so.
///
/// **RP3.3 is the other case.** Its outcome is `ECHO`, i.e. a
/// `PROPS_ECHO_R4133` row that ships in this commit, so [`Link::Echo`] claims
/// `generator.model`'s one example row the moment the row exists and [`declare`]
/// never routes it to [`Owner::Rp3`] again. Its counted columns are therefore
/// **`0, 0`** — a measurement, not a courtesy — while the entry itself stays,
/// because assertion #1 below requires this table to cover all four
/// [`BIN7_ROOT_CAUSE`] pairs. A zero-row entry is held to a *stronger* standard
/// than a declared one: the guard re-measures its pair from the corpus and
/// insists both that the rows still exist and that the chain claims every one of
/// them, so "0, 0" can never mean "the pair quietly vanished".
///
/// **A verdict is typed by its outcome tag.** `OPEN — ` is a sub-step that has
/// not run; every other verdict opens with one of [`RP3_SETTLED_SHAPES`]' tags,
/// which are plan §WP-RP3's three sanctioned outcomes — a drafted ledger entry
/// (`LEDGER`), an RP2.3 echo row (`ECHO`), a port fix in both lanes (`FIX`).
/// RP3.1, RP3.2 and RP3.4 exercised the first, RP3.3 the second, so
/// [`the_bin7_root_cause_pairs_are_routed_to_their_sub_steps`]
/// checks each tag's own obligations rather than assuming RP3.1's shape is every
/// settled shape (RP3.1 audit settlement, 2026-08-24): all three must cite the
/// **r4133** unit (a `Version8/Source/` `.pas:` line — a capi citation alone is
/// not the sub-step's evidence base) and name their own sub-step, and then
/// `LEDGER` owes its staging clause and its pins, `ECHO` owes the row on its
/// pair, `FIX` owes the Rust site.
const RP3_ROUTING: &[(&str, &str, usize, usize, &str)] = &[
    (
        "generator.model",
        "RP3.3",
        0,
        0,
        "ECHO — RP3.3 (2026-08-24): r4133's `model` getter is NOT live — TGeneratorObj.\
         GetPropertyValue (Version8/Source/PCElements/generator.pas:3007-3038) has no arm 6, so \
         index 6 falls through to General/DSSObject.pas:112-115 `Result := FPropertyValue[Index]`, \
         i.e. the deck's own typed token, stored unconditionally by the Edit loop at \
         generator.pas:625 before the CASE assigns the live field at :643 (InitPropertyValues' \
         default would be '1', :2559). Meanwhile NCIM moves the LIVE GenModel 3 -> 4 \
         (Common/Solution.pas:1935 the zero-Q-limits demote, :2120 the Q-band promote) and never \
         moves it back: the plan's :1760 restore, ReversePQ2PV (:1743-1768, declared :372), HAS NO \
         CALLER anywhere in the trunk — VersionC/Common/Solution.cpp:827 is the call and it is \
         commented out ('not needed for now (04/01/2024)') — so DoNCIMSolution (:1095-1161) ends at \
         its Until with the generator still model 4, and the only live reversion is the in-loop \
         :2229, which needs `not myPQOK` and does not fire on these decks. PROBED live on the r4133 \
         DLL: after the solve GeneratorsI(9) — the field itself, DDLL/DGenerators.pas:125-134 — \
         reads 4 on both decks while `? Generator.g1.model` renders '3'; GeneratorsI(10,4) moves \
         the field alone and the render stays '3'; `Edit model=4` moves the store and the render \
         follows; `Dump`/`Save Circuit` write model=3. The port renders the live field \
         (obj/props/class_props/value.rs -> elements/pc/generator/accessors.rs) and its NCIM is \
         structurally identical (solution/solution/ncim.rs, which likewise does not restore the \
         model), so the two engines' live state agrees digit for digit — present kvar \
         431.79425771046976 / 323.84569328285227 before the conversion, 0 after, 4 iterations, both \
         decks. Hence NO engine change and NO ledger entry on any COMPARED channel: the exclusion \
         is the PROPS_ECHO_R4133 row generator.model (EchoParse, 2 cells), witnessed by \
         generator_model_renders_the_live_pv2pq_conversion. One surface no channel compares is \
         left OPEN by this classification and owned by plan §RP3.11 (RP3.3 audit settlement, \
         2026-08-24): r4133's Save/Dump print the same store, so its own round trip re-creates the \
         model-3 generator, while the port's Save renders the LIVE field \
         (report/save/save.rs:34-53 goes through ClassProps::get_value where Pascal SaveWrite \
         reads PropertyValue[iProp], General/DSSObject.pas:145-165) and writes Model=4 — a \
         re-compiled deck is then a PQ generator instead of a Q-limited PV one. Census, derived \
         per case by \
         `the_rp33_census_decomposition_is_read_off_the_corpus`: 2 cells, all 2 in scope, 1 + 1 \
         over the 2 converting decks (modes:ncim/ncim_midi.dss, modes:ncim/ncim_pv_pq.dss), each \
         declaring one model=3 generator over 1 step on engines=r4133; modes:ncim/ncim_pq.dss runs \
         NCIM and declares no generator, so it produces no cell, and the corpus's 2 other NCIM \
         decks are kind=large and outside the census population. No upstream report: the render is \
         an echo of the deck's own token, not a wrong live value.",
    ),
    (
        "gictransformer.r2",
        "RP3.4",
        1,
        1,
        "LEDGER — RP3.4 (2026-08-24): both gating oracles carry the SAME slip and render it \
         through a LIVE getter. r4133's RecalcElementData builds winding 2's conductance from the \
         H-winding percentage — Version8/Source/PDElements/GICTransformer.pas:495 \
         `G2 := 100.0 / (FZBase2 * FPctR1);`, the byte-twin of pinned dss_capi 0.14.5 \
         src/PDElements/GICTransformer.pas:441 — while property 8 (`R2`, :130) renders \
         Format('%.8g',[1.0/G2]) from GetPropertyValue arm 8 (:723, and DumpProperties :663). So \
         the value is COMPUTED live off a mis-derived field, not echoed from the parse store \
         (property 14, `%R2`, :136, does echo FpctR2 at :729 and agrees with the port) — hence NO \
         PROPS_ECHO_R4133 row, the same reading in the other direction as RP3.1's and RP3.2's. \
         That it is a slip and not a convention is settled inside the same procedure: the else arm \
         restores FPctR2 from G2 (:497-498) and the two arms are inverses only when the forward \
         one reads FPctR2; the creation defaults are independent (%R1 = %R2 = 0.2, :458-459). No \
         engine change — GOLDEN_REBASE G2.5 already fixed both lanes \
         (crates/dss-core/src/elements/pd/gic_transformer/solve.rs:66) and the capi channel is \
         pinned by gic-pct-r2-honoured-gictransformer-capi-props and \
         gic-pct-r2-honoured-midi-capi-props; this sub-step only adds the r4133-channel twins \
         gic-pct-r2-honoured-gictransformer-r4133-props and gic-pct-r2-honoured-midi-r4133-props, \
         DRAFTED here and landing at RP4.1 per §1.1(e), witnessed meanwhile by \
         gictransformer_r2_honours_the_x_winding_percentage and \
         gictransformer_r2_honours_the_x_winding_percentage_on_the_ring. Census, derived per \
         element by `the_rp34_census_decomposition_is_read_off_the_corpus`: 2 cells, all 2 in \
         scope, 1 + 1 over the 2 %R decks (asymmetric:gic/gic_midi.dss tg5, \
         asymmetric:gic/gictransformer_gic.dss tg3), each `%R1=0.2 %R2=0.15 kvll1=345 kvll2=138 \
         mva=300 type=Auto` over 1 step on engines=both, so ours is ZBase2*%R2/100 = \
         63.48*0.15/100 = '0.09522' against ZBase2*%R1/100 = '0.12696' (rel 2.50e-01, \
         tests/corpus/props_r4133/bins.tsv:230). 2 in-scope cells over exactly 2 cases, hence \
         exactly two drafted entries and no more. The corpus's other 20 GICTransformers — 15 in \
         GIC_Example.dss, tg1/tg2 here, tg1/tg3 on the ring, gt on makeposseq_shunt — are all \
         ohms-specified and take the untouched else arm, 19 of them on r4133-gating cases, and \
         produce ZERO cells: the measurement that the %R path is the whole divergence class. \
         Independently of that arithmetic, the class-wide pairs gictransformer.enabled and \
         gictransformer.pctperm record 22 cells / 21 in scope, i.e. sum(declared) x steps over \
         the same population and its r4133-gating subtotal. Report: \
         investigations/to_opendss/07-gictransformer-g2-uses-pctr1.md (local), already written \
         against r4133 — a twin of a reported bug owes no new report.",
    ),
    (
        "swtcontrol.delay",
        "RP3.1",
        2,
        2,
        "LEDGER — RP3.1 (2026-08-24): r4133's Edit CASE has NO arm 5 \
         (Version8/Source/Controls/SwtControl.pas:195-218), so `delay=` reaches only the echo \
         store (:192-193) while `TimeDelay` keeps Create's 120.0 (:310) and the LIVE getter \
         renders it (:588). capi 0.14.5 wires the property (src/Controls/SwtControl.pas:185) and \
         the port follows, so no engine change. Render-only upstream — Sample's queue pushes are \
         commented out (:484-507, and LockCommand's declaration with them at :39), DoPendingAction \
         likewise (:396-408), set_States is immediate (:532-549) — hence NO echo row: the \
         exclusion is two per-case ledger `property` entries \
         (r4133-swtcontrol-delay-ignored-time / -midi), DRAFTED here and landing at RP4.1 per \
         §1.1(e), witnessed meanwhile by swtcontrol_delay_wires_the_property and \
         swtcontrol_delay_wires_the_property_on_the_midi_tie. Census, derived per case by \
         `the_rp31_census_decomposition_is_read_off_the_corpus`: the `'0.25'` spelling's 36 cells \
         are 24 in scope, 12 + 12 over the two r4133 decks, plus 12 on capi_v0145's \
         swtcontrol_lock.dss; the `'0'` spelling's 6 cells are three capi_v0145 IEEE_519 copies. \
         24 in-scope cells over exactly 2 cases, hence exactly two drafted entries and no more. \
         Report: investigations/to_opendss/43-swtcontrol-delay-not-wired.md (local)",
    ),
    (
        "windgen.kvar",
        "RP3.2",
        3,
        3,
        "LEDGER — RP3.2 (2026-08-24): r4133's kvar getter is wired and LIVE but reads the WRONG \
         live field — GetPropertyValue arm 11 (Version8/Source/PCElements/WindGen.pas:2896) \
         renders Format('%.6g',[presentkvar]) = Qnominalperphase*0.001*Fnphases (:2297-2300), \
         the DISPATCHED Q, while the property documents 'the base kvar' (:365) and Edit arm 11 \
         (:629) stores the token in kvarBase via Set_Presentkvar (:2996-3009). NOT an echo \
         (probed live on the r4133 DLL: a deck typing kvar=500 renders 0, and after `Edit \
         kvar=777` it still renders 0 while PF moves to 0.968058 — the value IS parsed; the echo \
         store's own default for the slot is '60', :2446) and NOT a port bug (probed: solved \
         terminal powers agree on all five decks — Q -2.1e-05/-4.2e-05 kvar in power flow, \
         -37087.81 vs -37087.76 kvar in dynamics), hence NO echo row and no engine change: the \
         port renders kvar_base (elements/pc/windgen/accessors.rs:431), exactly as its Generator \
         does and as r4133's own Generator does (Generator.pas:3018 against the identical getter \
         :2402-2405 and the identical help :396). Upstream is wrong three ways: with QMode=1 it \
         renders the operating-point 363.54 for a typed kvar=500; in dynamics — where :1254 \
         skips the Q block — it renders Set_Presentkvar's intermediate 777 while its own \
         kvarBase is 792.718441186736 and the measured terminal Q is -37087.76 kvar; and \
         SaveWrite (DSSObject.pas:156 through the virtual :117-120) makes `Save Circuit` emit \
         kvar=0, which on reload also flattens PFNominal to 1.0 and kvarMax/kvarMin to 0 \
         (:3001-3008). The rendered zero itself comes from a second defect, reported but out of \
         scope here: the steady-state QMode case (:1276-1322) has no arm 0 though QMode defaults \
         to 0 (:1020) and the help documents 0:Q (:429-430), so Else kvarCalc := 0 \
         (:1320-1321). The exclusion is four per-case ledger `property` entries \
         (r4133-windgen-kvar-dispatched-daily / -delta / -dyn / -dynfault), DRAFTED here and \
         landing at RP4.1 per §1.1(e), witnessed meanwhile by \
         windgen_kvar_renders_the_base_on_the_daily_deck, \
         windgen_kvar_renders_the_base_on_the_delta_snapshot, \
         windgen_kvar_renders_the_base_on_the_dynamics_deck and \
         windgen_kvar_renders_the_base_on_the_fault_ride_through_deck. Census, derived per case \
         by `the_rp32_census_decomposition_is_read_off_the_corpus`: 4 cells, all 4 in scope, \
         1 + 1 + 1 + 1 over the 4 diverging decks ('726.483157256779' x1, '854.95263026673' x2, \
         '986.05231553659' x1); no deck types kvar= at all, so every value is a pf=/kVA= side \
         effect and modes:windgen/windgen_snap.dss derives kvar_base 0 from pf=1.0 and produces \
         no cell, while 2 further corpus decks declare a WindGen and are held in \
         skipped_oracle_issue.json. 4 in-scope cells over exactly 4 cases, hence exactly four \
         drafted entries and no more. Report: \
         investigations/to_opendss/44-windgen-kvar-renders-dispatched-q.md (local)",
    ),
];

/// **The outcome tags a settled [`RP3_ROUTING`] verdict may open with** —
/// `(tag, the obligation the tag carries)`, one row per outcome plan §WP-RP3
/// sanctions (`R4133_PROPS_PLAN.md:1084-1090`).
///
/// Landed by the RP3.1 audit settlement (2026-08-24). Before it, the guard read
/// RP3.1's own shape — cite a `.pas:`, name `RP4.1` and `§1.1(e)`, own a
/// [`LEDGER_ENTRY_PINS`] pin — into *every* settled verdict, so a sub-step that
/// closes as a port fix or as an RP2.3 echo row (both sanctioned, neither
/// staging an entry) would have failed the guard, and the cheap way out of that
/// failure is to loosen the guard rather than to extend it. The taxonomy is that
/// extension, made in advance: an unknown tag is a hard error naming this table.
///
/// The obligations are enforced in
/// [`the_bin7_root_cause_pairs_are_routed_to_their_sub_steps`]; the shared ones
/// (an r4133 unit citation, the sub-step's own name) are checked for every tag
/// before the per-tag branch.
const RP3_SETTLED_SHAPES: &[(&str, &str)] = &[
    (
        "LEDGER",
        "an upstream divergence excluded by a per-case `ledger.json` `property` entry: the \
         verdict names RP4.1 and §1.1(e) (the staging rule), owns at least one LEDGER_ENTRY_PINS \
         witness and names each one, and its pair carries NO echo row",
    ),
    (
        "ECHO",
        "an upstream echo excluded by a PROPS_ECHO_R4133 row per RP2.3: the row exists on the \
         pair and cites its own pin, so the sub-step owns no LEDGER_ENTRY_PINS witness",
    ),
    (
        "FIX",
        "a port bug fixed in both lanes: the verdict names the Rust site (`.rs:`) and says `both \
         lanes`; nothing is excluded on the r4133 channel, so the pair carries no echo row and \
         no staged entry. A fix that also moves the port off the pinned 0.14.5 *oracle* still \
         lands its own live `capi_v0145` ledger entries in the same commit — the §1.1(e) \
         staging rule defers r4133 property entries only, because only that channel is masked \
         until RP4.1 (RP3.5 `line.units`, RP3.6 `line.linecode`)",
    ),
];

/// **RP3.1's census decomposition, as data instead of prose** — every corpus
/// case that declares a `SwtControl` **and** types a `delay=`, with the two
/// facts its deck carries: how many controls it declares, and the token it
/// types.
///
/// Landed by the RP3.1 audit settlement (2026-08-24). The sub-step's whole
/// artifact count rests on this decomposition — 42 cells, 24 of them in scope,
/// 12 + 12 over exactly two r4133-gating decks, *hence exactly two drafted
/// ledger entries* — and until the settlement those numbers lived only in the
/// verdict's prose and in STATUS, where mutating `24` to `25` left the suite
/// green. [`the_rp31_census_decomposition_is_read_off_the_corpus`] now derives
/// every one of them: the deck files give the control counts and the typed
/// values, `population.lock.json` gives each case's `steps=`/`engines=`, and the
/// products are reconciled against the frozen census both in total and per
/// spelling.
///
/// The table is a *claim of completeness*, not a filter: the test walks all
/// `.dss` files under `tests/corpus` and fails if any deck outside these two
/// tables declares a `SwtControl`, so a corpus that grows one reds here — where
/// the "exactly two entries" conclusion is drawn — instead of silently at RP4.1.
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
/// declare a `SwtControl` and type **no** `delay=`, so the port renders the same
/// `120` r4133 does and the census records **no cell at all**.
///
/// `civanlar.dss` is the load-bearing one: 16 controls on an `engines: r4133`
/// case, i.e. 16 in-scope cells that would exist if our unset render differed —
/// its absence from the census is the measurement that the divergence is the
/// *typed* value and nothing else. (It is also the third reading of the pin
/// `swtcontrol_delay_wires_the_property`.)
const RP31_NO_DELAY_CASES: &[(&str, usize)] = &[
    ("modes:makeposseq/makeposseq_ctrl.dss", 1),
    (
        "solvable_now:Version8/Distrib/Examples/civinlar model/civanlar.dss",
        16,
    ),
];

/// **RP3.2's census decomposition, as data instead of prose** — every corpus
/// **case** that declares a `WindGen`, with the declaration tokens its base kvar
/// is derived from: `(case, WindGens declared, kW=, pf=, kVA=)`, `""` for a
/// token the deck never types.
///
/// Landed by RP3.2 (2026-08-24), on [`RP31_DELAY_CASES`]' precedent and for the
/// same reason: the sub-step concludes *exactly four* drafted ledger entries,
/// and that is a statement about cases — 4 cells, all 4 in scope, one per
/// diverging deck. [`the_rp32_census_decomposition_is_read_off_the_corpus`]
/// derives every one of those numbers instead of transcribing them.
///
/// **No deck types `kvar=`** — not on a `New` line and not on a later `Edit`,
/// which [`element_tokens`] reads too. Every value in the census is a *side
/// effect* of the deck's `pf=` or `kVA=` — which is why the fifth deck,
/// `windgen_snap.dss`, carries no cell at all: `pf=1.0` makes the base zero and
/// both engines print `0`. That is a value coincidence and not agreement, and
/// the pin `windgen_kvar_renders_the_base_on_the_delta_snapshot` measures the
/// difference (type a `kvar=` there and the two diverge like everywhere else).
///
/// **None of the five types `QMode=` either** ([`windgen_dispatch`], asserted):
/// `WindModelDyn.QMode` stays `Create`'s 0 (`WindGen.pas:1020`), the
/// steady-state `case` (`:1276-1322`) falls to `Else kvarCalc := 0`, and *that*
/// is why the value r4133 renders through `Get_Presentkvar` is `0` on all of
/// them.
///
/// The table is a *claim of completeness*: the test walks every `.dss` under
/// `tests/corpus` and fails if a deck outside this table and
/// [`RP32_WINDGEN_SKIPPED_DECKS`] declares a `WindGen` — in any spelling
/// [`element_scope`] accepts, quoted included.
const RP32_WINDGEN_CASES: &[(&str, usize, &str, &str, &str)] = &[
    ("modes:windgen/windgen_daily.dss", 1, "3000", "0.95", ""),
    ("modes:windgen/windgen_dyn.dss", 1, "1500", "", "1800"),
    ("modes:windgen/windgen_dyn_fault.dss", 1, "1500", "", "1800"),
    ("modes:windgen/windgen_snap.dss", 1, "1500", "1.0", ""),
    ("modes:windgen/windgen_snap_delta.dss", 1, "1500", "0.9", ""),
];

/// The other half of [`RP32_WINDGEN_CASES`]' completeness claim: the corpus
/// **decks** that declare a `WindGen` and are **not** in the population at all —
/// held out by `skipped_oracle_issue.json`, so they are no case, own no cell and
/// owe no ledger entry.
///
/// Both are the vendored `WindGenerator` examples, and both are held for the
/// same measured reason ([`RP32_SKIPPED_TAG`]): they are multi-step runs the
/// pinned 0.14.5-era oracle infrastructure cannot gate. They are load-bearing
/// here the way `civanlar.dss` is in RP3.1, but from the opposite side — each
/// types `kVA=1200.0` at PF 0.88 (the QSTS deck writes `PF=0.88` on a `~`
/// continuation, the GFL one leaves `Create`'s default), so each derives a
/// **nonzero** base of ~569.97 where the population's own clean deck derives 0.
/// Promoting either one would therefore add in-scope cells — `WindGens × steps`
/// of them, both being multi-step — and the pair's entry count would have to be
/// re-derived, which is exactly why the exclusion has to be asserted rather than
/// assumed.
///
/// **What is deliberately NOT claimed: what r4133 renders on these two.** They
/// are out of the population, so no channel has ever run them, and their
/// mechanism is not the population's: both type `QMode=2` with a real
/// `VV_Curve=` ([`windgen_dispatch`], asserted below), i.e. the volt-var arm
/// `kvarCalc := kvarBase * VV_Curve(Vmag)` (`WindGen.pas:1289-1319`) rather than
/// the `Else kvarCalc := 0` (`:1320-1321`) the five census decks take. Whether
/// their render would diverge at all is unmeasured — the RP3.2 audit round
/// removed the "against r4133's `0`" half of this argument, which was never
/// probed.
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

/// **RP3.3's census decomposition, as data instead of prose** — every corpus
/// **case** in the census population that runs `Set algorithm=NCIM`, with the two
/// facts its deck carries: `(case, Generators declared, the `model=` token typed
/// on them)`.
///
/// Landed by RP3.3 (2026-08-24) on [`RP31_DELAY_CASES`]' and
/// [`RP32_WINDGEN_CASES`]' precedent, and for the same reason: the sub-step
/// concludes *exactly two* diverging cells, i.e. one echo row of `cells: 2` and
/// an `ECHO_ROWS_ON_R4133_ONLY_CASES` exposure of `(2, 2)`, and that is a
/// statement about cases. [`the_rp33_census_decomposition_is_read_off_the_corpus`]
/// derives every one of those numbers.
///
/// **The mechanism token is deck-level, not element-level.** What makes a
/// generator's live `GenModel` leave the deck's typed token is the NCIM PV→PQ
/// conversion (`Version8/Source/Common/Solution.pas:2120`), so the sweep's filter
/// is `Set algorithm=NCIM` ([`sets_ncim`]) and the element read runs only on the
/// survivors. Two things follow that the test asserts rather than assumes: a
/// deck that declares no generator produces no cell (`ncim_pq.dss`, the control),
/// and a generator that does not type `model=3` is not convertible at all —
/// `Create`'s default is model 1 (`generator.pas:924`, port
/// `elements/pc/generator/mod.rs`), which never enters the PV path.
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
/// Both are `kind: "large"`, and the census population is "every live case,
/// **non-large**, non-pending/abort/defer" (`tests/corpus/props_r4133/triage.md`
/// §Method). So the frozen `2` is exact **over the census population**, and these
/// two add nothing to it *by construction of that population* — not because
/// their generators stay model 3, which is unmeasured either way. Naming them is
/// the honest form of the decomposition (the RP3.2 held-out precedent); the test
/// asserts the `kind` from `population.lock.json` rather than taking it on trust.
///
/// Their generators live in **redirected** files, which is why the second column
/// exists: a single-file [`element_tokens`] read on the master returns 0, so
/// routing these two through the token reader alone would have made them look
/// like generator-free decks. The test reads both — 0 on the master, the real
/// count on the sibling.
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

/// **RP3.4's census decomposition, as data instead of prose** — **every**
/// `GICTransformer` the corpus declares, one row each, with the resistance spec
/// its own element scope types: `(case, element, %R1=, %R2=, R1=, R2=, kvll2=,
/// mva=)`, `""` for a token the deck never types.
///
/// Landed by RP3.4 (2026-08-24) on [`RP31_DELAY_CASES`]', [`RP32_WINDGEN_CASES`]'
/// and [`RP33_NCIM_CASES`]' precedent. This one is per **element** rather than
/// per case because the divergence is per element: the two `%R`-specified
/// GICTransformers each carry a cell while their eight and twelve ohms-specified
/// siblings — on the very same decks — carry none, so a per-case count would
/// have to assert the split it is supposed to derive.
///
/// **What the rows are for.** `RecalcElementData`'s `%R` branch derives winding
/// 2's conductance from the **H**-winding percentage
/// (`Version8/Source/PDElements/GICTransformer.pas:495`, the byte-twin of pinned
/// dss_capi 0.14.5 `src/PDElements/GICTransformer.pas:441`), so a cell exists
/// exactly where an element is `%R`-specified **and** its two percentages
/// differ; the port's honest `ZBase2*%R2/100` is then rendered against both
/// oracles' `ZBase2*%R1/100` by the same
/// `Format('%.8g',[1.0/G2])` getter (`:723`).
/// [`the_rp34_census_decomposition_is_read_off_the_corpus`] derives both sides
/// from the tokens below instead of transcribing the frozen spelling.
///
/// **The ohms rows are load-bearing, not decoration** — they are this sub-step's
/// `civanlar.dss`. Twenty of the twenty-two GICTransformers take the `R1=`/`R2=`
/// path, which sets `FpctRSpecified := FALSE` (`:343`) and leaves
/// `RecalcElementData`'s **reverse** branch (`:497-498`) — the one neither
/// revision ever got wrong — to answer the getter. Nineteen of those twenty sit
/// on r4133-gating cases and not one of them produces a divergent cell, which is
/// the measurement that the `%R` path is the whole divergence class rather than
/// an assertion about it. `T5`/`T12`/`T14`/`T15` are `type=Auto` like `tg3`/`tg5`
/// and still clean, so the connection type is not the discriminator either.
///
/// A GSU never types `R2=` at all (`tg1`, `T1`, …): winding 2 keeps the
/// conductance `Create` derived while `%R1 = %R2 = 0.2` (`:458-459`), so both
/// engines land on the same `ZBase2*0.2/100` and the census sees nothing there
/// either.
const RP34_GIC_ELEMENTS: &[GicDecl] = &[
    GicDecl::ohms(GIC_MIDI, "tg1", "0.12", ""),
    GicDecl::ohms(GIC_MIDI, "tg3", "0.2", "0.1"),
    GicDecl::pct(GIC_MIDI, "tg5", "0.2", "0.15", "138", "300"),
    GicDecl::ohms(GIC_MICRO, "tg1", "0.12", ""),
    GicDecl::ohms(GIC_MICRO, "tg2", "0.2", "0.1"),
    GicDecl::pct(GIC_MICRO, "tg3", "0.2", "0.15", "138", "300"),
    GicDecl::ohms("modes:makeposseq/makeposseq_shunt.dss", "gt", "0.1", ""),
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

/// The two class-wide `GICTransformer` pairs RP3.4 reconciles its population
/// against, with `(cells, cells in scope)` from `bins.tsv` — a cross-check of the
/// declaration count and of the `steps=` read that owes nothing to the `%R`
/// arithmetic.
///
/// Both pairs are answered by **every** GICTransformer on every step, whatever
/// its resistance spec, so their cell counts must be `Σ declared × steps` over
/// the corpus and their in-scope counts the same sum restricted to the
/// r4133-gating cases. If the population moved — a deck gained an element, a
/// case changed `steps=` or `engines=` — these two numbers move with it, and
/// they move *before* RP3.4's own two-cell conclusion could quietly absorb it.
const RP34_CLASS_WIDE_PAIRS: &[(&str, usize, usize)] = &[
    ("gictransformer.enabled", 22, 21),
    ("gictransformer.pctperm", 22, 21),
];

/// The manifest that holds [`RP32_WINDGEN_SKIPPED_DECKS`] out of the population,
/// repo-root-relative.
const SKIPPED_ORACLE_ISSUE: &str = "tests/corpus/manifests/skipped_oracle_issue.json";
/// The tag both skipped WindGen decks carry there — a limitation of the pinned
/// oracle's multi-step capture, not anything about `kvar`.
const RP32_SKIPPED_TAG: &str = "capi015_multistep_limitation";

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
    // **RP3.5 — SETTLED 2026-08-28, outcome FIX (both lanes).** r4133 renders
    // index 20 LIVE — `LineUnitsStr(LengthUnits)` (`PDElements/Line.pas:1404`) —
    // so `'none'` vs `'kft'` meant the two engines held different `LengthUnits`.
    // r4133 re-applies the saved units AFTER the impedance edit (`MergeWith`
    // saves at `:1627`, re-edits at `:1721-1726` and `:1791-1796`;
    // `MakePosSequence` re-appends `Units=` at `:1596` "to compensate for
    // unexpected reset"), while the port's matrix-series branch had copied
    // dss_capi 0.14.5's inverted order (`src/PDElements/Line.pas:1806-1817`):
    // the field write, then the RMATRIX/XMATRIX/CMATRIX side effects whose
    // `ResetLengthUnits` wiped it. `exec/reduce.rs`'s matrix-series branch now
    // calls `red_set_units` AFTER those side effects, and `red_merge`'s sym
    // branch re-applies `Length=`/`Units=` unconditionally as both oracles do.
    // Second divergence in the same routine, from BOTH oracles: the port's
    // `reset_length_units` cleared `user_length_units`
    // (`elements/pd/line/code.rs`), which r4133 (`Line.pas:2330`) and capi
    // 0.14.5 (`:2084`) both keep "in case of CIM export"; that line is gone.
    // Fixed in both lanes; the port now renders the r4133 value, so the frozen
    // `rust='none'` column of this row is HISTORICAL (the extracts are a data
    // lock, never edited). The three cells are on `modes:reduce/midi_reduce.dss`,
    // a capi-only case, so what the fix moved is the LIVE capi channel:
    // `tests/corpus/ledger.json` `reduce-merge-units-restored-midi-capi-props`
    // (3 hits) is the exclusion and
    // `exec::tests::reduce::merged_matrix_line_keeps_the_surviving_lines_length_units`
    // plus `the_corpus_reduce_decks_merged_lines_render_kft` are the pins. The
    // census decomposition behind "3 cells, 0 in scope" is derived by
    // `the_rp35_census_decomposition_is_read_off_the_corpus`.
    (
        "line.units",
        Owner::Rp35,
        "RP3.5 FIXED (2026-08-28) — Line.pas:1404 / :1627 / :1721-1726 /          :1791-1796 / :2326-2331",
    ),
    // --- the S6 singletons ---------------------------------------------------
    // **RP3.6 — SETTLED 2026-08-29, outcome FIX (both lanes).** r4133 renders
    // index 3 LIVE — `3: If FLineCodeSpecified Then Result := CondCode else
    // Result := ''` (`Line.pas:1357`) — off a flag `FetchLineCode` arms
    // (`:413`) and the impedance side effects clear (`6..11, 26..27` at `:685`,
    // `12..14` at `:691`). The `switch=` arm does NOT: `:694-700` assigns
    // r1/x1/r0/x0/c1/c0/len as fields, kills geometry and spacing and resets the
    // length units, one arm below and one arm above the two that clear the flag,
    // and carries no `FLineCodeSpecified` statement at all. The port had copied
    // dss_capi 0.14.5, which ADDED the kill there and flagged it in its own
    // source (`src/PDElements/Line.pas:677`, `//TODO: check if this missing is
    // relevant bug`). Not cosmetic: the flag also picks the `FUnitsConvert`
    // formula on a later `units=` (`Line.pas:626-627`), and these decks type
    // `units=m` AFTER `Switch=True`. Fixed in both lanes by deleting that one
    // call from the `SWITCH` arm of `elements/pd/line/accessors.rs`. r4133 clears
    // the flag at eight OTHER statements (`:685`, `:691`, `:1832`, `:1853`,
    // `:1952`, `:2016`, `:2075`, `:2131`), and the port now carries a
    // counterpart for every one of them: seven already stood, and the eighth —
    // `FetchConductorList`'s (`:1853-1854`) — was added by the RP3.6 audit
    // settlement (2026-08-29), which measured r4133 answering `''` behind a
    // `conductors=[..]` where the port still answered the code name. The port
    // now renders the r4133 value, so
    // the frozen `rust=''` column of this row is HISTORICAL (the extracts are a
    // data lock, never edited). All 5 cells are in scope, on two `engines: both`
    // cases whose capi property compare is live TODAY, so what the fix moved is
    // the LIVE capi channel: `tests/corpus/ledger.json`'s
    // `line-switch-keeps-linecode-zone2-capi-props` (3 hits) and
    // `line-switch-keeps-linecode-zone3-capi-props` (2 hits) are the exclusions
    // and `exec::tests::line_fetch::
    // switch_yes_keeps_the_linecode_and_its_units_conversion` is the pin. No
    // r4133 entry, staged or landed: after the RP4.1 unmask these 5 cells
    // compare and MATCH, which is the point of the sub-step. The decomposition
    // behind "5 cells, all 5 in scope" is derived by
    // [`the_rp36_census_decomposition_is_read_off_the_corpus`].
    //
    // Part (b) (same day) split the two pieces of state the cells hang off:
    // `FLineCodeSpecified` (`:57`, raised at `:413`, cleared at the eight kill
    // sites) is now `line_code_specified`, while `CondCode` (`:103`, written at
    // `:387`, cleared only by the constructor at `:825`) is `line_code_name` and
    // survives every kill. That moved no cell on this pair — the render stays
    // flag-gated (`:1357`), so a superseded code still answers `''` — but it
    // fixed the two surfaces that read the name raw: `Dump` now prints
    // `~ LineCode=<code>` past a kill (`:1273`) and the CIM units back-fill
    // matches the `CondCode` string (`Common/ExportCIMXML.pas:3876`) instead of
    // the live object, which is 0.14.5's rule (`:4501`). No ledger entry and no
    // lock line moved with it: neither surface is compared by any channel.
    (
        "line.linecode",
        Owner::Rp35,
        "RP3.6 FIXED (2026-08-29) — Line.pas:1357 / :413 / :685 / :691 / \
         :694-700 / :626-627 — elements/pd/line/accessors.rs SWITCH arm, both \
         lanes; pin exec::tests::line_fetch::\
         switch_yes_keeps_the_linecode_and_its_units_conversion. Part (b): \
         Line.pas:387 / :825 / :1273 + ExportCIMXML.pas:3876 — \
         elements/pd/line/code.rs, dump.rs and cim/export.rs, both lanes; pins \
         exec::tests::line_fetch::\
         linecode_name_survives_the_flag_that_gates_its_render and \
         golden_cim::cim_linecode_units_backfill_matches_the_condcode_string. \
         Audit settlement (same day): Line.pas:1853-1854 / :663 / :696 / \
         :704-713 / :2268 — r4133's eighth flag-clear site plus the \
         SpacingSpecified field its two plain assignments need — \
         elements/pd/line/{mod,code,accessors}.rs, both lanes; pin \
         exec::tests::line_fetch::\
         conductors_clears_the_linecode_flag_and_the_switch_arm_spares_the_spacing",
    ),
    // Index 21 has no getter arm (`Line.pas:1347-1429` covers 1..20, 23, 26..33
    // and the PD tail) → `DSSObject.pas:112-115` echoes `PropertyValue[21]`,
    // default `''` (`:1511`), overwritten with the deck's `'sp'`.
    // `SpacingSpecified` is killed later but the echoed string never moves.
    // (RP3.6's audit settlement narrowed the port's side without touching this
    // pair: since the port models `SpacingSpecified` as r4133's Boolean field,
    // a `linecode=`/`switch=` that only drops the flag leaves the object — and
    // the port's live render — standing, so it now agrees with the echo in
    // strictly more places. The one census cell is `MakePosSequence`, which
    // really does kill the object on both engines.)
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
    // locked_ignores_normal_and_state_writes` — renamed
    // `locked_normal_applies_locked_state_and_action_do_not` when RP3.7 landed
    // the r4133 rule), following 0.14.5's
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
    //
    // **SETTLED 2026-09-02, outcome FIX (both lanes).** The port now carries
    // r4133's per-phase model: `[ControlAction; 6]` arrays plus the
    // `NormalStateSet` latch, `InterpretSwitchState`'s name-keyed lock guard,
    // ganged-when-bare / per-phase-when-quoted writes through a fresh parser
    // with the five-token cap, per-conductor drive at `RecalcElementData`, and
    // `MappedStringEnumArray` getters that render one token per controlled
    // phase (`[]` for a nil element). (a2) landed with it, exactly as the
    // paragraph above reads it: a locked `normal=` APPLIES while a locked
    // `state=`/`action=` does not, and the Edit supplemental
    // (`SwtControl.pas:220-228`) sits OUTSIDE the interpreter, so
    // `NormalStateSet` latches even on a refused write. `docs/upgrade/
    // DIVERGENCES.md` §D12 carries the corrected record.
    //
    // The frozen `rust='closed'` column of these rows is therefore HISTORICAL
    // (the extracts are a data lock, never edited), which is why they still
    // declare to this bucket — see [`DECLARED_RP35`]. What the fix moved is the
    // two channels, in opposite directions:
    //
    // * **r4133 — nothing to exclude.** All 80 in-scope cells (40 per pair, on
    //   `midi_swtcontrol` 12, `swtcontrol_time` 12 and `civanlar` 16) now render
    //   BYTE-IDENTICALLY to r4133; re-measured 2026-09-02 with the RP0.2 census
    //   knob, which reports ZERO `Normal`/`State` rows on that channel for those
    //   three decks and leaves only the pairs owned elsewhere (`Delay` RP3.1,
    //   `Action` RP2.3, `Reset`/`enabled` bin 1, `SwitchedObj` bin 2). No
    //   `property` entry is staged and none is needed. Held meanwhile by
    //   `exec::tests::controls::
    //   swtcontrol_state_renders_per_phase_on_the_r4133_only_decks` — plan
    //   §1.1(c): those three decks are `engines: r4133`, and the 0.14.5 oracle
    //   cannot render the array at all, so no oracle channel witnesses them
    //   until RP4.1.
    // * **capi_v0145 — five LIVE entries, landed with the fix.** The 19
    //   out-of-scope cells sit on five capi-gated cases whose property compare
    //   runs today: `swtcontrol_lock` (12 steps x 2 cells, and twice over —
    //   probes AND properties), `makeposseq_ctrl` (2) and three `IEEE_519`
    //   copies (4 each). `tests/corpus/ledger.json`'s
    //   `swtcontrol-per-phase-state-{lock,makeposseq,ieee519-tmode,
    //   ieee519-varload,ieee519-matlab}-capi-props` (cause
    //   `swtcontrol-per-phase-state-render`) are the exclusions and
    //   `exec::tests::controls::
    //   swtcontrol_state_renders_one_token_per_controlled_phase` is their
    //   witness ([`LANDED_PROPERTY_ENTRY_PINS`]). The §1.1(e) staging rule
    //   defers r4133 entries only.
    //
    // The decomposition behind "59 cells, 40 in scope, 19 on exactly five capi
    // cases — hence exactly five entries and no more" is derived from the corpus
    // by [`the_rp37_census_decomposition_is_read_off_the_corpus`].
    (
        "swtcontrol.normal",
        Owner::Rp35,
        "RP3.7(a) FIXED (2026-09-02) — SwtControl.pas:589-599 / :37-38 / \
         :299-307 / :416-417 / :433-480 / :532-549 / :220-228 — \
         elements/control/swt_control/{mod,accessors}.rs, both lanes; pins \
         exec::tests::controls::\
         swtcontrol_state_renders_one_token_per_controlled_phase and \
         swtcontrol_state_renders_per_phase_on_the_r4133_only_decks, plus \
         swt_control::tests::locked_normal_applies_locked_state_and_action_do_not",
    ),
    (
        "swtcontrol.state",
        Owner::Rp35,
        "RP3.7(a) FIXED (2026-09-02) — SwtControl.pas:600-610 (same set)",
    ),
    // **RP3.7 (b), the mirror image**: here the PORT renders three tokens and
    // r4133 one. `TRelayObj.GetPropertyValue` 39/40 loops the LIVE
    // `ControlledElement.NPhases` (`Controls/Relay.pas:1407-1428`); the port has
    // a per-phase array (hence `[closed, open, open, ]`) but does not resync it
    // to the controlled element after `MakePosSequence`. The single cell is
    // `modes/makeposseq/makeposseq_ctrl.dss:44`, `engines: "capi_v0145"`.
    //
    // **SETTLED 2026-09-02, outcome FIX (both lanes).** ONE frozen snapshot
    // caused two symptoms, not one: `make_pos_sequence` now refreshes
    // `ctrl_snap.{nphases, nterms, buses}` from the live controlled element, so
    // the render prints one token AND `Sample` stops resyncing conductors 2..3
    // off the end of a 1-conductor element — which is why the frozen
    // `relay.state` cell reads `[closed, open, open, ]` rather than three closed
    // tokens. Both cells now render r4133's `[closed, ]` byte for byte (probed
    // on the r4133 DLL over the `makeposseq_ctrl` shape), so nothing is excluded
    // on either channel: the frozen `rust` columns are HISTORICAL and the rows
    // stay declared for that reason alone. Four further r4133 mismatches on the
    // same write seam were fixed with it under "port gaps immediately" (a quoted
    // single token is per-phase where a bare one is ganged, the `Action`
    // supplemental runs after a refused or unmatched write, and the five-token
    // `RELAYCONTROLMAXDIM` cap), none with corpus exposure. No capi entry is due
    // either: `Relay` and `Recloser` are whole-element-skipped on that channel
    // (`tests/harness/mod.rs::skip_whole_element`), measured again on
    // `makeposseq_ctrl` (2 elements / 80 cells skipped).
    (
        "relay.normal",
        Owner::Rp35,
        "RP3.7(b) FIXED (2026-09-02) — Relay.pas:1407-1417 (loops \
         ControlledElement.NPhases) / :1237-1308 / :616-619 — \
         elements/control/relay/{mod,accessors}.rs, both lanes; pins \
         relay::tests::the_render_bound_follows_makeposseq and \
         a_quoted_single_token_is_per_phase_a_bare_one_is_ganged",
    ),
    (
        "relay.state",
        Owner::Rp35,
        "RP3.7(b) FIXED (2026-09-02) — Relay.pas:1418-1428 (same set)",
    ),
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
/// excluded field-by-field and pinned. That is an engine change, so it became
/// plan §RP3.8, and until that sub-step ran these 181 example rows were declared
/// to [`Owner::Rp38`] — loudly, with a count lock, never as a silent leftover
/// inside RP2.3's bucket. Giving them an `EchoCategory` instead would have been
/// exactly the mislabel the kill criterion exists to prevent.
///
/// # Why they are SUPERSEDED and not claimed (RP3.8, 2026-09-02)
///
/// RP3.8 landed the engine change: `PropFlags::RENDERS_LIVE_RESULT` rides
/// alongside `SILENT_READ_ONLY` on exactly these five `PropDef`s and the render
/// gate (`obj/props/class_props/value.rs`) stops suppressing them, in both
/// lanes. So the `rust` column of all 181 frozen rows — `''`, by capture —
/// records an engine that no longer exists, and the extracts cannot be
/// re-frozen (`README.md` §"Corrections measured after freezing"). Replaying a
/// counterfactual spelling through the shipped chain would prove nothing about
/// the comparator, and inventing the port's new spelling into the `rust` column
/// would be a fabricated measurement. The rows are therefore counted as
/// **superseded** ([`SUPERSEDED_RP38`]) rather than claimed or declared, and
/// what carries the proof instead is live evidence, per pair, below.
///
/// # What the live evidence says
///
/// Measured 2026-09-02 with `DSS_PROPS_CENSUS=claims` over the affected
/// families, default lane, both channels, the §1.1(e) property masks bypassed
/// (the numbers are in the RP3.8 STATUS record and at `SKIP_PROPS`' row group
/// (g)):
///
/// * on **capi_v0145** the five pairs are now excluded outright — a
///   `SKIP_PROPS` + `SKIP_PROPS_CAPI_ONLY` row pair, because 0.14.5 renders `''`
///   for a mechanical reason (`PropertyOffset = -1`) and no value compare can
///   bridge `number` vs `''`;
/// * on **r4133** — the channel this accounting is about — they COMPARE, and
///   the whole population of the five pairs leaves **105 divergent cells** (89
///   in scope), of which **103** are [`Link::DisplayFloor`]'s own class (the
///   port prints `float_to_str_ex`, r4133 its `%.6g`/`%-.8g` of the same
///   double; worst 4.03e-08 rel, four orders under the 2e-4 floor). The other
///   **2** are the `modes:makeposseq/makeposseq_ctrl.dss` cells of an upstream
///   r4133 `MakePosSequence` bug, on a `capi_v0145`-only case the r4133 channel
///   never gates (§1.3). Every remaining cell of the 1 064 the frozen census
///   counted now compares EQUAL, at the case's own tier floor.
///
/// Each row below therefore carries four columns: the pair, the r4133 live
/// getter arm it renders from, the measured live disposition on the r4133
/// channel, and the expected-value pin that holds the port's own value
/// (`props_r4133_pins.rs`; the same both-ways guard
/// [`every_echo_row_pin_is_a_test_that_exists`] that forbids an un-cited
/// `#[test]` there reads this column too).
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
         `kWrated` (Storage.pas:3979-3985 vs :647) and never scales the rating — an \
         upstream bug the port does not reproduce, on a capi_v0145-only case the r4133 \
         channel never gates (§1.3)",
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

/// The pin that holds the **capi** half of RP3.8's exclusion — the committed
/// 0.14.5 capture's own `''` for all 21 `props` golden cells of the five pairs.
///
/// It is not a per-pair witness (it speaks for all five at once), so it rides
/// beside [`RP38_SUPERSEDED`] rather than inside it, and it is cited here for
/// the same reason: [`every_echo_row_pin_is_a_test_that_exists`] must know every
/// `#[test]` the pin file defines.
const RP38_CAPTURE_PIN: &str = "the_silent_readonly_capture_cells_are_empty";

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
    /// **The echo table** (`PROPS_ECHO_R4133`, RP2.3's, +1 from RP3.3) — the
    /// exclusion. 82 cited
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
    /// **RP2.4's display floor** (`props_norm::R4133_DISPLAY_FLOOR`, `2e-4`
    /// relative) — the last link, consulted only after the three above have
    /// declined. It claims a cell whose two sides are one number printed to
    /// different precision by a Delphi `Format('%[-].Ng', …)` getter.
    ///
    /// Unlike the two tables it is a **per-cell predicate with no rows**: it
    /// reads the two values and nothing else, so there is nothing here to go
    /// stale and nothing that could mask a neighbouring cell of the same pair.
    /// The derivation (worst 6.431124e-05, the empty band up to 1.374769e-03,
    /// the `%[-].Ng` site table) lives on the constant; this link's own
    /// population is [`CLAIMED_DISPLAY_FLOOR`].
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
    /// RP2.4 — the r4133 props display floor (bin 6). **Empty since RP2.4
    /// closed** ([`DECLARED_RP24`]): its population is claimed by
    /// [`Link::DisplayFloor`] or re-declared by [`RP24_OUT_OF_SCOPE`].
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
    ///
    /// **A settled sub-step does not leave the bucket** — the same rule
    /// [`RP3_ROUTING`] states. RP3.5 landed on 2026-08-28 as a port fix in both
    /// lanes, yet `line.units` still declares here: its exclusion is a
    /// `capi_v0145` ledger entry and no [`Link`] of the chain reads the ledger,
    /// so the row is retired by hand at RP4.1, never on its own.
    Rp35,
    /// **RP3.8 — the sub-step RP2.3's kill criterion opened** (the ruling of
    /// 2026-08-23, user-approved). Five `SilentReadOnly` pairs whose divergence
    /// is neither an echo nor a spelling: r4133 renders a live computed
    /// read-only quantity and the port renders `''` only because dss_capi
    /// 0.14.5 flags the property `[SilentReadOnly, ReadByFunction]`. Under the
    /// standing 2026-08-02 policy (r4133 is the behavioral authority; 0.14.5 is
    /// a numeric oracle only) the engine must render the live value and the
    /// resulting capi-side divergence is excluded + pinned THERE — an engine
    /// change, forbidden inside RP2.3's zero-product-bytes scope.
    ///
    /// **Empty since RP3.8 landed** (2026-09-02, [`DECLARED_RP38`]): the engine
    /// renders the five live values now, so the frozen rows' `rust = ''` column
    /// no longer describes this port and they are counted as superseded
    /// ([`RP38_SUPERSEDED`], [`SUPERSEDED_RP38`]) instead of declared. The
    /// variant stays so a regression that re-creates the bucket fails by name.
    Rp38,
    /// **RP3.9 — the sub-step the RP2.4 audit settlement opened**: 55 spellings
    /// over 27 pairs that sit inside the display floor and are no `%.Ng` render
    /// of our value, so the two engines hold genuinely different doubles (an
    /// upstream command-string round trip amplified through a derived quantity,
    /// or a plain state difference). See [`RP39_ROUTING`]; RP4.1 does not start
    /// until it closes (plan §0).
    ///
    /// **All 27 pairs settled 2026-09-02** ([`OPEN_RP39`]) — every one a
    /// reproduced upstream round trip with the port exact, pinned by
    /// [`RP39_PINS`]. The bucket itself does not move for that: nothing claims
    /// the rows, so they stay declared here until RP4.1's unmask retires them.
    Rp39,
    /// **RP3.12 — the sub-step RP3.9's P0 open item opened**: the 8
    /// `autotrans.wdgcurrents` spellings of the four `controls:autotrans/*`
    /// decks (34 cells, none in scope), where r4133's `RegControl` reads a
    /// `TAutoTransObj` through an unchecked `TTransfObj` cast and therefore
    /// never taps it. Its cells are the unregulated circuit's; ours are the
    /// regulated one's, and under the 2026-08-02 policy that stays so. See
    /// [`RP312_UPSTREAM_BUG`] and [`DECLARED_RP312`].
    ///
    /// They were [`Owner::OutOfScope`]'s until 2026-09-03. The move is a
    /// verdict, not a re-scope: all 34 cells sit on `engines: "capi_v0145"`
    /// cases either way, so nothing about the RP4.1 unmask changes — what
    /// changes is that the divergence now has a measured cause and a witness
    /// instead of a scope excuse.
    Rp312,
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
            Owner::Rp39 => "RP3.9 (opened by the RP2.4 audit settlement)",
            Owner::Rp312 => "RP3.12 (opened by RP3.9's P0 open item)",
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
    /// Rows whose frozen `(rust, r4133)` spelling an engine change made
    /// counterfactual, so no link of the chain can be asked about them and no
    /// sub-step owns them either — today exactly [`RP38_SUPERSEDED`]'s five
    /// pairs ([`SUPERSEDED_RP38`]). The third and last state a row can end in,
    /// and the narrowest: a row only reaches it by naming its pair in a cited
    /// table.
    superseded: Bucket,
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

    /// `(rows, pairs, rows on in-scope pairs)` for the superseded bucket — the
    /// same triple [`Ledger::owner`] reports, so [`SUPERSEDED_RP38`] can be read
    /// against it like any `DECLARED_*` lock.
    fn superseded(&self) -> (usize, usize, usize) {
        (
            self.superseded.rows,
            self.superseded.pairs.len(),
            self.superseded.in_scope_rows,
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
/// would satisfy a substring test for both — "names every witness" would then be
/// enforced for one of the two (RP3.1 audit settlement, 2026-08-24).
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
/// [`the_rp31_census_decomposition_is_read_off_the_corpus`].
const CORPUS: &str = "tests/corpus";
/// The anti-shrink lock, whose per-case rigor strings carry `steps=`/`engines=`.
const POPULATION_LOCK: &str = "tests/corpus/manifests/population.lock.json";
/// The gating divergence ledger — read by
/// [`the_staged_r4133_property_entries_have_not_landed_yet`] and by
/// [`r4133_skipped_cases`], and by nothing else in this file. **No `Link` of the
/// declaration chain reads it** (see [`DECLARED_RP3`]): the two readers above are
/// assertions *about* the file, not sources of a declaration, so the accounting
/// still does not shrink by itself when RP4.1 lands the staged entries.
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
/// `skip` entry drops it — the second half of "does r4133 gate this case?", read
/// off [`LEDGER`] with `corpus_gate/ledger.rs::channel_is_skipped`'s own
/// condition (`case` matches, `channel == "r4133"`, `kind == "skip"`).
///
/// **`engines: "both"` alone does not mean r4133 gates a case** — the shorthand
/// the RP3.4 audit settlement (2026-08-24) caught this file using in four census
/// derivations. `asymmetric:line/line_spacing_asym.dss` is `engines: "both"` and
/// yet no r4133 comparison ever runs on it: the ledger holds
/// `r4133-linespacing-asym-303` with `kind: "skip"` (an EPRI #303 access
/// violation while compiling the deck's `tscables=`/`wires=` spacing), so
/// `corpus_gate/scheduler.rs:355-356` `continue`s past the channel. A census
/// derivation that called such a case "in scope" would keep asserting a cell
/// count the gate can no longer produce — so RP3.4's derivation asks both
/// questions and this reader answers the second.
///
/// Read-only, like every other use of [`LEDGER`] here: the staged entries land at
/// RP4.1, never from this file.
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
/// `class_dot` — `"swtcontrol."`, `"windgen."`.
///
/// Two spellings this must not miss, both added by the RP3.2 audit round:
///
/// * the identifier may be **quoted** — `New "WindGen.w1" …` is the form the
///   vendored corpus itself writes for other classes
///   (`electricdss-tst/Test/IndMachTest.DSS:108`, `New "IndMach012.windgen1"`),
///   so a reader that accepted only the bare spelling would let a quoted
///   declaration slip past a completeness claim;
/// * an **`Edit`/`BatchEdit`** types on the element just as its `New` does, so a
///   claim like "no deck types `kvar=`" is only true if those lines are read
///   too (a declaration-only sweep left `Edit WindGen.w1 kvar=500` invisible).
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
/// [`the_rp32_census_decomposition_is_read_off_the_corpus`] asserts that instead
/// of assuming it.
fn windgen_facts(deck: &str) -> (usize, String, String, String, String) {
    let (declared, [kw, pf, kva, kvar]) =
        element_tokens(deck, "windgen.", ["kw=", "pf=", "kva=", "kvar="]);
    (declared, kw, pf, kva, kvar)
}

/// Whether a deck selects the **NCIM** solver — `set algorithm=NCIM` on a live
/// (non-comment) line, in any case and in any spelling the engine accepts.
///
/// This is RP3.3's mechanism filter: the only thing in the whole engine that
/// moves a `Generator`'s live `GenModel` away from the token its deck typed is
/// the NCIM PV→PQ conversion (`Version8/Source/Common/Solution.pas:1935`,
/// `:2120`), and r4133's `model` getter renders that token forever
/// (`PCElements/generator.pas:3007-3038` has no arm 6).
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
/// Comment lines are skipped — both `ncim_midi.dss` and `ncim_pq.dss` describe
/// the setting in their headers — and a live line that mentions `algorithm` in a
/// spelling [`named_token`] cannot parse is a hard error rather than a silent
/// miss, because a completeness sweep that quietly skips a deck proves nothing.
///
/// **Abbreviations are read, not assumed away** (RP3.3 audit settlement,
/// 2026-08-24 — before it, only the full spelling was read and the hard error
/// keyed on the literal substring `algorithm`, so `set algo=ncim` would have
/// been a silent miss). `Set` resolves an option name through
/// `TCommandList.GetCommand` (`Version8/Source/Executive/ExecOptions.pas:566`),
/// which falls back to `THashList.FindAbbrev` — a linear **prefix** match
/// (`Shared/HashList.pas:335-357`) — because both `TCommandList` constructors arm
/// `AbbrevAllowed := True` (`Shared/Command.pas:53`, `:65`). Every non-empty
/// prefix of `algorithm` is therefore read as the option, longest first. The
/// reader is deliberately WIDER than the engine (a short prefix may resolve to
/// some other option that sits earlier in `ExecOption`): a false hit here is a
/// loud failure a human reads, a miss is exactly the silent skip this sweep
/// exists to prevent. Measured at HEAD: the corpus types no abbreviation at all
/// — all seven live mentions are the full spelling — so the widening is dormant.
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
                 parse — RP3.3's NCIM sweep would silently miss the deck"
            ),
        }
    }
    out
}

/// Whether an `algorithm=` value selects NCIM, by r4133's own reading of it.
///
/// `InterpretSolveAlg` (`Version8/Source/Common/Utilities.pas:575-591`) compares
/// only the **first two characters** — `SLC := copy(lowercase(s), 1, 2)`, then
/// `ne` → Newton, `nc` → NCIM, anything else → the normal fixed point — so
/// `Set algorithm=nc` selects NCIM just as `NCIM` does, and an equality test
/// against `"ncim"` (what this reader did before the RP3.3 audit settlement)
/// would have missed it without even reaching the hard error above.
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

/// **The scripts a corpus deck executes that [`collect_dss`] cannot see** — the
/// transitive `Redirect`/`Compile` closure of `decks`, restricted to files whose
/// extension is not `dss`, as corpus-relative paths.
///
/// `collect_dss` filters on the extension, so a sweep built on it walks `.dss`
/// files only — while the vendored corpus really does redirect other ones
/// (`Examples/Scripts/WireData.txt`, `ckt24/AllocationFactors_Base.Txt`, the
/// LVTestCase's seven `.txt` parts). A `Set algorithm=NCIM` inside one of those
/// would run and be invisible to the sweep, which is a hole in a *completeness*
/// claim, not a detail (RP3.3 audit settlement, 2026-08-24). Matching is by
/// basename ([`redirect_targets`]) over every corpus file, and the closure is
/// transitive because a redirected script may redirect further.
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
/// The tokens RP3.4's derivation needs, in [`RP34_GIC_ELEMENTS`]' column order:
/// the two percentages, the two ohms values, and the winding-2 base.
///
/// The trailing `=` and [`named_token`]'s separator rule are what keep these
/// apart: in `%R1=0.2` the character before `r1=` is `%`, which is neither the
/// line start nor a separator, so the ohms key does **not** match a percentage
/// token (and vice versa — `%r1=` needs the `%`). That distinction is the whole
/// spec/branch discriminator (`Version8/Source/PDElements/GICTransformer.pas:343`
/// vs `:349`), so it is exercised on the real decks by
/// [`the_gictransformer_reader_separates_the_percentage_and_ohms_specs`].
const GIC_KEYS: [&str; 6] = ["%r1=", "%r2=", "r1=", "r2=", "kvll2=", "mva="];

/// **Every `GICTransformer` one corpus file declares, per element**, with the
/// [`GIC_KEYS`] tokens typed inside that element's own scope (`""` for one it
/// never types).
///
/// [`element_tokens`] cannot serve here: it collapses a whole class to one scope
/// per deck and hard-errors when a key appears twice with different values —
/// and both `gic/*` decks type `R1=0.12` on their GSU and `R1=0.2` on their YY.
/// The scope rule is still [`element_scope`]'s (a `New` opens a declaration, an
/// `Edit`/`BatchEdit` re-opens the named element's, a `~` continues whatever is
/// open, anything else closes it), so an `R1=` on a Reactor two lines down is
/// not this transformer's.
///
/// Two things are hard errors rather than silent readings, because RP3.4's
/// conclusion is a *count* of ledger entries:
///
/// * an `Edit`/`BatchEdit` naming an element the file never declared — the
///   corpus has none, and a wildcard `BatchEdit` would otherwise be attributed
///   to no element at all;
/// * the same key typed twice with two different values inside one element's
///   scope — the derivation assumes one spelling per element, exactly as
///   [`element_tokens`] does per deck.
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
                                 declared ({name:?}) — RP3.4's per-element decomposition cannot \
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
                    "{rel}: two different `{key}` tokens inside the `{}` scope — RP3.4's \
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
/// `SetNominalGeneration`'s `case WindModelDyn.QMode`
/// (`Version8/Source/PCElements/WindGen.pas:1276-1322`) has an arm 1 (PF), an
/// arm 2 (Volt-Var, `kvarCalc := kvarBase * VV_Curve(Vmag)`, `:1313`) and no arm
/// 0 — `Else kvarCalc := 0` (`:1320-1321`) — while `QMode` defaults to 0
/// (`:1020`). Reading the token is what separates the two populations RP3.2
/// reasons about: the five census decks type **no** `QMode=`, so they take the
/// `Else` arm and r4133's render is the dispatched zero, whereas the two decks
/// held out of the population type `QMode=2` with a real curve and take the
/// volt-var arm — a *different* mechanism, which is why nothing here may claim a
/// measured r4133 value for them ([`RP32_WINDGEN_SKIPPED_DECKS`]).
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
    // The SHIPPED predicate, never a copy of it — same rule as the two links
    // above (a drifting copy would make this completeness proof describe a
    // comparator that does not exist). It reads `props_norm::display_floor()`
    // internally, so the accounting below moves with the constant.
    let floor = props_norm::under_display_floor(&row.rust, &row.r4133);
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
    // NOTE: RP2.3's kill-criterion re-route used to sit here, ahead of every
    // rule below, because those five pairs' cells classify as bin 5 (one side
    // empty) and would have been mis-filed as echoes. RP3.8 landed the engine
    // change, so their frozen spellings are counted as superseded in `account`
    // — before this function is ever reached ([`RP38_SUPERSEDED`]). If that
    // interception is ever removed, the rows fall through to the bin-5 arm and
    // land in [`Owner::Rp23`]'s closed bucket, which reds [`DECLARED_RP23`].
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
    // RP2.4's MECHANISM residual, ahead of every bin rule: a numeric cell whose
    // gap is inside the display floor and whose r4133 side is no `%.Ng` render
    // of our value. The floor refuses it by construction, and the reason has
    // nothing to do with the pair's bin or with §1.3 scope — the two engines
    // hold different doubles. See [`RP39_ROUTING`].
    if ev.numeric && display_class_but_not_a_render(row) {
        return match RP39_ROUTING.iter().find(|(p, _, _, _, _)| *p == row.pair) {
            Some(_) => Ok(Owner::Rp39),
            None => Err(format!(
                "{} '{}' vs '{}': inside RP2.4's display floor but no `%.Ng` render of our \
                 value, on a pair RP39_ROUTING does not cite — read the r4133 round-trip \
                 chain off the Pascal and cite it, or fix the port",
                row.pair, row.rust, row.r4133
            )),
        };
    }
    // RP3.12's upstream bug, ahead of the bin rules for the same reason RP2.4's
    // mechanism residual is: the cause is a property of the two engines' control
    // paths, not of the pair's bin or of §1.3 scope. It runs AFTER the clause
    // above so the pair's ninth spelling — the `modes:makeposseq` round-trip
    // residue — stays [`Owner::Rp39`]'s. See [`RP312_UPSTREAM_BUG`].
    if regcontrol_autotrans_typecast_row(row) {
        return Ok(Owner::Rp312);
    }
    if ev.numeric {
        return match ev.effective_bin() {
            // A display-class row the FLOOR did not claim (it runs before this
            // function, so everything inside 2e-4 is already gone). The only
            // admissible reason is that the row has no in-scope cell at all,
            // and the pair's own frozen ceiling is what proves it —
            // [`RP24_OUT_OF_SCOPE`]. Anything else is RP2.4's kill criterion:
            // either the floor is mis-derived or a new category appeared.
            6 => match row_out_of_scope_by_ceiling(row, ev) {
                Some(_) => Ok(Owner::OutOfScope),
                None => Err(format!(
                    "{} '{}' vs '{}': a display-class (in-scope bin 6) row that RP2.4's floor \
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

/// **RP2.4's mechanism residual, as a row predicate**: the gap is inside the
/// derived display floor, and the r4133 spelling is still no `%.Ng` render of
/// our value.
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

/// **RP3.12's upstream bug, as a row predicate**: the row is on a pair
/// [`RP312_UPSTREAM_BUG`] cites and is NOT the display-class residue
/// [`display_class_but_not_a_render`] hands to [`Owner::Rp39`].
///
/// The second half is what keeps the two sub-steps' shares of
/// `autotrans.wdgcurrents` apart without either of them naming a deck: RP3.9 owns
/// the one spelling that IS inside the floor (the `modes:makeposseq` round trip),
/// RP3.12 the eight that are 3–7.5 % out of it. Written as a predicate rather
/// than a spelling list so a re-measure that moves a row across the floor fails
/// one of the two ownership guards instead of silently re-filing itself.
fn regcontrol_autotrans_typecast_row(row: &Example) -> bool {
    RP312_UPSTREAM_BUG
        .iter()
        .any(|(pair, _, _, _, _, _)| *pair == row.pair)
        && !display_class_but_not_a_render(row)
}

/// **The row-level scope proof RP2.4 needs** — does the pair's own frozen
/// `max_rel_in_scope` show that this spelling has no cell the RP4.1 unmask
/// compares? `Some(ratio)` when it does, where `ratio` is the measured
/// `gap / ceiling` (locked at [`RP24_OUT_OF_SCOPE_MIN_RATIO`]).
///
/// `max_rel_in_scope` is the maximum over exactly the in-scope cells, so a
/// spelling that diverges by MORE than it cannot be one of them. Deliberately
/// narrow: it fires only on the four cited [`RP24_OUT_OF_SCOPE`] pairs, only on
/// numeric rows, and only past [`CEILING_ROUND_MARGIN`] — a general rule would
/// be a scope loophole any future pair could fall into unreviewed.
///
/// The gap is [`props_norm::display_rel`], the shipped metric the floor itself
/// is expressed in, so "above the ceiling" and "outside the floor" are read off
/// one function.
///
/// **Single-number rows only** (RP2.4 audit settlement, 2026-08-23). The two
/// metrics are not the same function: `display_rel` is symmetric and maximizes
/// over ALL numbers of the cell, while the census's `max_rel` is `|a−e|/|e|`
/// over the numbers that FAIL the case's tier floor (`harness::value_verdict`).
/// On a one-number row the implication still holds — the row's only number IS
/// the offender of any divergent cell it sits on, and `|e| <= max(|a|,|b|)`
/// makes the census metric the larger of the two, so `gap > ceiling` really does
/// exclude an in-scope cell. On a MULTI-number row the maximum could be attained
/// at a number the tier's `abs` term absorbs, which contributes nothing to
/// `max_rel_in_scope`, and the proof would not close. All 105 rows the rule
/// re-declares today are single-number; the guard keeps it that way instead of
/// leaving the limit as a comment.
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

/// Does this example row have at least one cell the RP4.1 unmask will compare?
///
/// The pair's `cells_in_scope` is the coarse answer and was the only one until
/// RP2.4; [`row_out_of_scope_by_ceiling`] refines it per row where the frozen
/// evidence supports it, which is what lets an in-scope PAIR carry a spelling
/// that is provably not.
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
                // `rust` column, which for these five pairs records a port that
                // no longer exists. See [`RP38_SUPERSEDED`]. It sits AFTER the
                // chain, so a link that ever did claim one of these rows would
                // be credited with it and the bucket lock would red — the
                // interception cannot hide a claim.
                if RP38_SUPERSEDED.iter().any(|(p, _, _, _)| *p == row.pair) {
                    led.superseded.rows += 1;
                    led.superseded.pairs.insert(row.pair.clone());
                    if row_in_scope(row, ev) {
                        led.superseded.in_scope_rows += 1;
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
        "PROPS_ECHO_R4133's 82 cited rows claim what the nine off-bin normalization rows leave, \
         minus the five pairs the kill criterion re-routed to RP3.8, plus RP3.3's generator.model"
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
        "plan §1.3: an OutOfScope row must have no in-scope cell at all — nothing else justifies \
         leaving a divergence unowned"
    );

    // …and the third state: the rows RP3.8's engine change made counterfactual.
    assert_eq!(
        led.superseded(),
        SUPERSEDED_RP38,
        "the superseded bucket inherits (rows, pairs, rows on in-scope pairs) — RP3.8's five \
         pairs and nothing else"
    );

    // Totality, three ways.
    assert_eq!(
        led.claimed_total() + led.declared_total() + led.superseded.rows,
        corpus.rows.len(),
        "every example row is claimed, declared or superseded, exactly once"
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
            // A carved-out cell of a cited pair: the exclusion does NOT cover
            // it, so the chain walks past it to the next link. Until RP2.4 that
            // meant "no link claims it" and `ECHO_CARVE_OUT_ROUTING` named the
            // sub-step that would; RP2.4 is that sub-step, and its floor now
            // takes the cell — which is exactly what the routing declared.
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
///
/// Since RP3.1 the pin file also holds witnesses no echo row *can* name (see
/// [`LEDGER_ENTRY_PINS`]): they hold the port's value for a **drafted** ledger
/// entry, in the window the §1.1(e) staging rule opens between the sub-step and
/// RP4.1. They are cited from that table instead, and the both-ways check covers
/// them the same way — the union of the citations must be exactly what the
/// file defines, so no list can be the place an un-cited `#[test]` hides.
///
/// RP3.8 adds a third citing table for the same reason and a different exclusion
/// shape: its pins witness a `SKIP_PROPS_CAPI_ONLY` row pair (no ledger entry at
/// all), and they are named by [`RP38_SUPERSEDED`]'s fourth column plus
/// [`RP38_CAPTURE_PIN`].
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
        "a pin witnesses an echo row, a drafted ledger entry, RP3.8's skip rows, an RP3.9 \
         round-trip residue pair or RP3.12's upstream bug, never two"
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
                .map(|n| (*n, "an RP3.8 skip row's witness")),
        )
        .chain(
            rp39_named
                .iter()
                .map(|n| (*n, "an RP3.9 residue pair's witness")),
        )
        .chain(
            rp312_named
                .iter()
                .map(|n| (*n, "RP3.12's upstream-bug witness")),
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
        "{PINS} must define exactly the pins the echo rows, the drafted ledger entries, \
         RP3.8's skip rows, RP3.9's residue pairs and RP3.12's upstream bug name"
    );
}

/// **The landed `capi_v0145` property entries and the engine pins that witness
/// them** — `(entry id, sub-step, pin `#[test]`, the file that defines it)`.
///
/// [`LEDGER_ENTRY_PINS`] covers the other half of the RP3 exclusion shapes: the
/// **staged r4133** entries, whose pins must live in [`PINS`] because the entry
/// itself cannot land before RP4.1. A `FIX` sub-step whose fix reds the LIVE capi
/// property compare has the opposite shape — the entry lands with the fix, in the
/// same commit — and its witness belongs beside the behaviour, in the engine's
/// own test modules. Nothing tied the two together until the RP3.6 audit
/// (2026-08-29) observed that deleting such a pin left the suite green: the gate
/// holds the *entry*, and the entry pins both sides of the divergence, but no
/// test asserted that the port's value is *right* rather than merely stable.
///
/// This table is that tie, and it is checked **both ways** by
/// [`every_landed_property_entry_has_a_witness_pin_that_exists`]: every row must
/// name a real landed entry and a real `#[test]`, and every landed entry an RP3
/// sub-step wrote must appear here. A future sub-step therefore cannot land a
/// capi property exclusion without a witness — the completeness half fails first.
const LANDED_PROPERTY_ENTRY_PINS: &[(&str, &str, &str, &str)] = &[
    (
        "reduce-merge-units-restored-midi-capi-props",
        "RP3.5",
        "merged_matrix_line_keeps_the_surviving_lines_length_units",
        "crates/dss-core/src/exec/tests/reduce.rs",
    ),
    (
        "line-switch-keeps-linecode-zone2-capi-props",
        "RP3.6",
        "switch_yes_keeps_the_linecode_and_its_units_conversion",
        "crates/dss-core/src/exec/tests/line_fetch.rs",
    ),
    (
        "line-switch-keeps-linecode-zone3-capi-props",
        "RP3.6",
        "switch_yes_keeps_the_linecode_and_its_units_conversion",
        "crates/dss-core/src/exec/tests/line_fetch.rs",
    ),
    (
        "swtcontrol-per-phase-state-lock-capi-props",
        "RP3.7",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol-per-phase-state-makeposseq-capi-props",
        "RP3.7",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol-per-phase-state-ieee519-tmode-capi-props",
        "RP3.7",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol-per-phase-state-ieee519-varload-capi-props",
        "RP3.7",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
    (
        "swtcontrol-per-phase-state-ieee519-matlab-capi-props",
        "RP3.7",
        "swtcontrol_state_renders_one_token_per_controlled_phase",
        "crates/dss-core/src/exec/tests/controls.rs",
    ),
];

/// The witness obligation for a **landed** capi property exclusion, the mirror of
/// [`every_echo_row_pin_is_a_test_that_exists`] for the entries that ship with
/// their fix instead of staging to RP4.1.
///
/// The completeness half is what makes it a guard rather than a list: it rebuilds
/// the landed set straight out of [`LEDGER`] — `channel = "capi_v0145"`,
/// `kind = "divergence"`, at least one `"field": "property"` match, and a `source`
/// naming an `R4133_PROPS_PLAN RP3.` measurement — and requires it to equal the
/// table exactly. Read-only on the ledger, like every other use of it here.
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
                && e["source"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("R4133_PROPS_PLAN RP3."))
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
    assert_eq!(
        landed, tabled,
        "every landed capi_v0145 property exclusion written by an RP3 sub-step owes a witness pin \
         in LANDED_PROPERTY_ENTRY_PINS (and every row must name a real entry)"
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

/// **The pins that witness a DRAFTED ledger entry rather than an echo row** —
/// `(pin, sub-step, the entry it holds the port's value for)`.
///
/// RP3.1 is the first sub-step whose exclusion shape is a per-case `ledger.json`
/// `property` divergence entry instead of a `PROPS_ECHO_R4133` row, and the plan
/// forbids the row explicitly: r4133's `Delay` getter is LIVE
/// (`SwtControl.pas:588`), so calling the divergence an echo would be a false
/// statement about the mechanism. RP3.2 is the second and lands four more, on
/// the same grounds from the other direction — its getter is not merely live but
/// *wired*, and reads the wrong live field (`WindGen.pas:2896` renders the
/// dispatched Q where the property documents the base kvar). RP3.4 is the third
/// and lands two, on the same grounds from a third direction — its getter is
/// wired, live and reading the documented field, but the field itself was
/// mis-derived one procedure earlier, so `Format('%.8g',[1.0/G2])`
/// (`GICTransformer.pas:723`) is a correct read of a wrong number. Under the
/// §1.1(e)
/// staging rule the entries are
/// drafted in the sub-step and land in RP4.1's unmask commit — earlier they
/// would fail `assert_all_hit` as NEVER APPLIED, the r4133 property compare
/// being masked until then — so between the two there is a window in which the
/// port's value has no holder in the tree at all. These pins are that holder.
///
/// Listed here, and not left to a naming convention, for the same reason
/// [`NOT_A_PIN`] is: this table is the *only* thing that lets a `#[test]` live
/// in the pin file without an echo row naming it, so growing it is a decision.
/// It is pinned literally by [`the_ledger_entry_pin_list_is_pinned`] and tied to
/// the sub-step's routing row by
/// [`the_bin7_root_cause_pairs_are_routed_to_their_sub_steps`].
const LEDGER_ENTRY_PINS: &[(&str, &str, &str)] = &[
    (
        "swtcontrol_delay_wires_the_property",
        "RP3.1",
        "r4133-swtcontrol-delay-ignored-time (controls:swtcontrol/swtcontrol_time.dss)",
    ),
    (
        "swtcontrol_delay_wires_the_property_on_the_midi_tie",
        "RP3.1",
        "r4133-swtcontrol-delay-ignored-midi (controls:swtcontrol/midi_swtcontrol.dss)",
    ),
    (
        "windgen_kvar_renders_the_base_on_the_daily_deck",
        "RP3.2",
        "r4133-windgen-kvar-dispatched-daily (modes:windgen/windgen_daily.dss)",
    ),
    (
        "windgen_kvar_renders_the_base_on_the_delta_snapshot",
        "RP3.2",
        "r4133-windgen-kvar-dispatched-delta (modes:windgen/windgen_snap_delta.dss)",
    ),
    (
        "windgen_kvar_renders_the_base_on_the_dynamics_deck",
        "RP3.2",
        "r4133-windgen-kvar-dispatched-dyn (modes:windgen/windgen_dyn.dss)",
    ),
    (
        "windgen_kvar_renders_the_base_on_the_fault_ride_through_deck",
        "RP3.2",
        "r4133-windgen-kvar-dispatched-dynfault (modes:windgen/windgen_dyn_fault.dss)",
    ),
    (
        "gictransformer_r2_honours_the_x_winding_percentage",
        "RP3.4",
        "gic-pct-r2-honoured-gictransformer-r4133-props \
         (asymmetric:gic/gictransformer_gic.dss)",
    ),
    (
        "gictransformer_r2_honours_the_x_winding_percentage_on_the_ring",
        "RP3.4",
        "gic-pct-r2-honoured-midi-r4133-props (asymmetric:gic/gic_midi.dss)",
    ),
];

/// **The ledger-entry pin list is pinned literally**, like [`NOT_A_PIN`]: it is
/// the second exemption from "every pin in the file is named by an echo row",
/// and an exemption that grows by iteration would let an un-cited `#[test]`
/// through by simply being added to it.
///
/// Each row must name a real drafted entry id and the case it is drafted for, so
/// the citation stays checkable against STATUS's verbatim record until RP4.1
/// lands the entries in `tests/corpus/ledger.json`.
#[test]
fn the_ledger_entry_pin_list_is_pinned() {
    assert_eq!(
        LEDGER_ENTRY_PINS,
        [
            (
                "swtcontrol_delay_wires_the_property",
                "RP3.1",
                "r4133-swtcontrol-delay-ignored-time (controls:swtcontrol/swtcontrol_time.dss)",
            ),
            (
                "swtcontrol_delay_wires_the_property_on_the_midi_tie",
                "RP3.1",
                "r4133-swtcontrol-delay-ignored-midi (controls:swtcontrol/midi_swtcontrol.dss)",
            ),
            (
                "windgen_kvar_renders_the_base_on_the_daily_deck",
                "RP3.2",
                "r4133-windgen-kvar-dispatched-daily (modes:windgen/windgen_daily.dss)",
            ),
            (
                "windgen_kvar_renders_the_base_on_the_delta_snapshot",
                "RP3.2",
                "r4133-windgen-kvar-dispatched-delta (modes:windgen/windgen_snap_delta.dss)",
            ),
            (
                "windgen_kvar_renders_the_base_on_the_dynamics_deck",
                "RP3.2",
                "r4133-windgen-kvar-dispatched-dyn (modes:windgen/windgen_dyn.dss)",
            ),
            (
                "windgen_kvar_renders_the_base_on_the_fault_ride_through_deck",
                "RP3.2",
                "r4133-windgen-kvar-dispatched-dynfault (modes:windgen/windgen_dyn_fault.dss)",
            ),
            (
                "gictransformer_r2_honours_the_x_winding_percentage",
                "RP3.4",
                "gic-pct-r2-honoured-gictransformer-r4133-props \
                 (asymmetric:gic/gictransformer_gic.dss)",
            ),
            (
                "gictransformer_r2_honours_the_x_winding_percentage_on_the_ring",
                "RP3.4",
                "gic-pct-r2-honoured-midi-r4133-props (asymmetric:gic/gic_midi.dss)",
            ),
        ],
        "RP3.1's two drafted entries, RP3.2's four and RP3.4's two, and nothing else"
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

/// **RP3.9's expected-value pins** — `(pair, pin `#[test]`, verdict)`, one row
/// per settled pair of the round-trip residue [`RP39_ROUTING`] declares.
///
/// These pairs are neither echo rows (both engines' getters are LIVE — r4133's
/// number is computed, not echoed back from a parse) nor, today, drafted ledger
/// entries: the full claims census measures `count_in_scope = 0` on all 55
/// spellings, so no case's gating channel compares any of these cells and a
/// `property` entry would have nothing to exclude. What each pair owes is the
/// other half of the RP3 discipline — an expected-value test that names BOTH
/// numbers and reproduces r4133's from ours through the cited Pascal round trip
/// (`R4133_PROPS_PLAN.md` §RP3.9, outcomes 1 and 2). This is where those pins are
/// cited, so [`every_echo_row_pin_is_a_test_that_exists`] holds them to the same
/// both-ways rule as the other three sets: an un-cited `#[test]` in [`PINS`]
/// fails, and a pair naming a pin that does not exist fails too.
///
/// Only the two outcomes whose obligation *is* a pin and nothing else may appear
/// here ([`RP39_SETTLED_VERDICTS`]): a `PORT_BUG` is fixed in both lanes and
/// witnessed beside the behaviour, an `UPSTREAM_BUG` earns a staged entry and
/// moves to [`LEDGER_ENTRY_PINS`], and a `KILL` has no verdict to record at all.
/// The counted columns of [`RP39_ROUTING`] do **not** move when a pair settles:
/// the vendored spellings are a data lock the walk still measures, exactly as the
/// `FIX` sub-steps' extracts are (`RP22_ROUTING`), and they retire by hand at
/// RP4.1; the settlement shows up in [`OPEN_RP39`] instead.
///
/// **Complete since 2026-09-02**: all 27 [`RP39_ROUTING`] pairs are `PIN` and all
/// 27 are named here, so [`the_rp39_pin_list_is_pinned`] now checks the
/// completeness half as well — a `PIN` disposition with no test, or a pin on a
/// pair the routing disposes of some other way, is a hard error.
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

/// The verdicts an [`RP39_PINS`] row may carry: the two §RP3.9 outcomes whose
/// whole obligation is an expected-value pin — the port is exact and r4133's
/// number is a reproduced round trip (`PRECISION_ROUNDTRIP`), or the two engines'
/// state genuinely differs by a proven precision-class cause (`STATE_DIFFERS`).
/// The other three outcomes owe something else and live elsewhere; an unknown tag
/// is a hard error naming this list, never a reason to loosen it.
const RP39_SETTLED_VERDICTS: &[&str] = &["PRECISION_ROUNDTRIP", "STATE_DIFFERS"];

/// **RP3.12's routing and its witness** — `(pair, example rows, rows on in-scope
/// pairs, r4133 site, verdict, pin `#[test]`)`, the fifth citing set
/// [`every_echo_row_pin_is_a_test_that_exists`] reads.
///
/// One pair, one verdict, `UPSTREAM_BUG` (`R4133_PROPS_PLAN.md` §RP3.9's fourth
/// outcome, the tag RP3.12 inherits): r4133's `RegControl` reaches its controlled
/// element through an unchecked `TTransfObj(ControlledElement)` cast
/// (`Version8/Source/Controls/RegControl.pas:926`/`:1026`/`:1296`/`:1370`/`:1479`)
/// while `TAutoTransObj = class(TPDElement)`
/// (`Version8/Source/PDElements/AutoTrans.pas:88`) is not a `TTransfObj`, so
/// `TapIncrement` reads the winding's `MaxTap` (1.1 pu) and
/// `PendingTapChange := Round(BoostNeeded / Increment) * Increment` (`:1249-1250`)
/// zeroes every realistic boost. r4133 logs 0 control events on all four decks
/// and leaves the regulated bus outside its band; the 8 spellings below are its
/// UNREGULATED circuit. The port keeps the regulated answer in both lanes —
/// upstream bugs are never reproduced (CLAUDE.md, 2026-08-02) — and the report is
/// `investigations/to_opendss/50-regcontrol-autotrans-ttransfobj-typecast.md`.
///
/// **Why the witness is a pin and not a [`LEDGER_ENTRY_PINS`] row.** The
/// divergence is whole-case, not per-field: voltages (2.2–2.6 % at the regulated
/// bus), the assembled Y, every element on the AutoTrans branch, the meters, the
/// event log (10–13 lines vs 0), the control queue and all three manifest probes.
/// The instrument is a case-level `kind: "skip"` on the `r4133` channel, one per
/// deck — and the four cases are `engines: "capi_v0145"` today
/// (`tests/corpus/manifests/population.lock.json:74-77`), so there is no r4133
/// channel to skip and the entries are **drafted, not landed**
/// (`tmp/rp312/staged_ledger.md`). A `property` entry, which is what
/// [`LEDGER_ENTRY_PINS`] cites, would be the wrong shape as well as premature.
///
/// The counted columns behave exactly like [`RP39_ROUTING`]'s: they are
/// re-measured from the walk by
/// [`the_regcontrol_autotrans_typecast_rows_are_owned_by_rp312`], a pin does not
/// make a link claim a row, and they retire by hand at RP4.1.
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
/// sub-step exists because the divergence is an upstream defect the port does not
/// reproduce. Anything else — a precision round trip, a port bug — owes a
/// different obligation and belongs with that outcome's own table, so an unknown
/// tag is a hard error naming this list rather than a reason to widen it.
const RP312_VERDICTS: &[&str] = &["UPSTREAM_BUG"];

/// **RP3.9's pin list is pinned literally**, like [`NOT_A_PIN`] and
/// [`LEDGER_ENTRY_PINS`]: it is the third exemption from "every pin in [`PINS`]
/// is named by an echo row", and an exemption that grows by iteration would let
/// an un-cited `#[test]` through by being added to it.
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
        "all 27 pairs of RP3.9's round-trip residue — chains A (load.*), B (vsource.*), \
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

/// **The five `SilentReadOnly` pairs are superseded by RP3.8's live render**,
/// in both directions — the successor of
/// `the_kill_criterion_reroute_is_the_five_silent_readonly_pairs`, which
/// asserted the same five pairs while the sub-step was still open.
///
/// Four things, none of them transcription:
///
/// * the table is exactly those five pairs, each still carrying the r4133
///   live-getter citation that made the re-route a verdict rather than a shrug,
///   and none of them takes an echo row (the ruling forbids it: r4133's arm is
///   live, so an `EchoRow` would misname the mechanism);
/// * the superseded bucket holds their rows and **only** theirs, at
///   [`SUPERSEDED_RP38`], while [`Owner::Rp38`]'s declared bucket is empty;
/// * **the shipped disposition is asked of the harness, not described**: each
///   pair must be value-skipped on `capi_v0145` and compared on `r4133`
///   (`harness::skip_prop`, the `SKIP_PROPS` + `SKIP_PROPS_CAPI_ONLY` row pair
///   RP3.8 landed). This is what makes the supersession falsifiable: revert the
///   engine to `''` and the capi compare AGREES again, so the gate stays green
///   — but then the skip rows are dead, and whoever removes them reds here;
/// * each pair names an expected-value pin that
///   [`every_echo_row_pin_is_a_test_that_exists`] proves exists — the holder of
///   the port's own value for the 84 masked cells.
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
        "the five pairs the RP2.3 kill ruling re-routed and RP3.8 settled"
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
        "RP3.8's declared bucket must be empty since the sub-step landed"
    );
    assert_eq!(led.superseded(), SUPERSEDED_RP38);
    assert_eq!(
        led.superseded
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

/// **WP-RP3's bucket has a per-pair work list, and a settled sub-step stays in
/// it until its artifact lands** — the both-ways guard for [`RP3_ROUTING`],
/// added by RP3.1 (2026-08-24).
///
/// [`DECLARED_RP3`] is one number for four independent sub-steps, which is
/// exactly enough to hide two opposite mistakes: a pair quietly leaving the work
/// list because its sub-step was declared done (while the tree still holds no
/// exclusion for it — the §1.1(e) window), and a pair quietly growing or
/// shrinking inside the total. So the routing is re-measured here rather than
/// trusted:
///
/// * it covers the plan's four root-cause pairs, once each, and its three
///   counted columns sum to the bucket lock;
/// * the per-pair `(rows, in-scope rows)` split is what the walk actually
///   declares to [`Owner::Rp3`] — not a transcription;
/// * the settled set is pinned literally (since RP3.4 that is **all four** pairs
///   — a legal end state, not a reason to relax anything here), and each settled
///   verdict is checked **against the obligations of its own outcome tag**
///   ([`RP3_SETTLED_SHAPES`]) — all three shapes must cite the r4133 unit and
///   name their sub-step; `LEDGER` additionally owes the §1.1(e) staging clause
///   and must name every [`LEDGER_ENTRY_PINS`] witness of that sub-step *as a
///   whole identifier* ([`names_identifier`]) while its pair carries **no** echo
///   row; `ECHO` owes the row itself; `FIX` owes the Rust site and "both lanes".
///   An unknown tag is a hard failure that names the table to extend;
/// * an open verdict points at its plan section and owns no pin, so a pin cannot
///   be landed for a sub-step that has not run.
#[test]
fn the_bin7_root_cause_pairs_are_routed_to_their_sub_steps() {
    assert_eq!(
        RP3_ROUTING.iter().map(|(p, ..)| *p).collect::<Vec<_>>(),
        BIN7_ROOT_CAUSE,
        "the routing covers exactly the plan's four root-cause pairs, once each"
    );
    // The middle term is the pairs that still DECLARE rows, not the table's
    // length: since RP3.3 an entry can carry `0, 0` because the tree really
    // holds its exclusion (an `ECHO` outcome ships its row in the same commit),
    // and `Ledger::owner`'s pair count — which this must equal — counts only
    // pairs with a surviving row. The entry itself stays for assertion #1.
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
            ("generator.model", "RP3.3"),
            ("gictransformer.r2", "RP3.4"),
            ("swtcontrol.delay", "RP3.1"),
            ("windgen.kvar", "RP3.2")
        ],
        "the sub-steps that have run, in RP3_ROUTING order — since RP3.4 that is ALL FOUR \
         bin-7 root-cause pairs, which is a legal end state and not a reason to relax anything \
         below: three of them still declare their rows, because a LEDGER outcome stages its \
         exclusion into RP4.1"
    );
    // …and a zero-row entry may only be one that is settled: an OPEN sub-step
    // with no rows would be a pair that vanished, not a pair that was excluded.
    for (pair, step, rows, in_scope, verdict) in RP3_ROUTING {
        assert_eq!(
            *rows == 0,
            *in_scope == 0,
            "{pair}: a pair cannot declare rows without declaring in-scope ones here, or vice versa"
        );
        assert!(
            *rows > 0 || !verdict.starts_with("OPEN — "),
            "{pair}: {step} has not run, so its rows must still be declared"
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
                rest.starts_with(&format!("plan §{step}:")),
                "{pair}: an open sub-step must point at its own plan section, got {verdict:?}"
            );
            assert!(
                pins.is_empty(),
                "{pair}: {step} has not run, so it can own no ledger-entry pin — got {pins:?}"
            );
            continue;
        }
        assert!(
            RP3_SETTLED_SHAPES.iter().any(|(t, _)| *t == tag),
            "{pair}: unknown outcome tag {tag:?}. Plan §WP-RP3 sanctions {:?} — a fourth outcome \
             is a plan decision: extend RP3_SETTLED_SHAPES with the obligations it owes, never \
             widen this guard to let an untyped verdict through",
            RP3_SETTLED_SHAPES
                .iter()
                .map(|(t, _)| *t)
                .collect::<Vec<_>>()
        );
        // Shared by all three settled shapes. The r4133 unit is the sub-step's
        // evidence base and a capi citation is no substitute — a settled verdict
        // routinely cites both, so `.pas:` alone would be satisfied by the capi
        // half (RP3.1 audit settlement).
        assert!(
            verdict.contains("Version8/Source/") && verdict.contains(".pas:"),
            "{pair}: a settled verdict must cite the r4133 unit itself \
             (Version8/Source/<unit>.pas:LINE), got {verdict:?}"
        );
        assert!(
            names_identifier(verdict, step),
            "{pair}: a settled verdict must name its own sub-step {step}, got {verdict:?}"
        );
        match tag {
            "LEDGER" => {
                assert!(
                    verdict.contains("RP4.1") && verdict.contains("§1.1(e)"),
                    "{pair}: a drafted exclusion must say where it lands, got {verdict:?}"
                );
                assert!(
                    !pins.is_empty(),
                    "{pair}: a settled sub-step whose exclusion is staged owes at least one pin"
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
                    verdict.contains(".rs:") && verdict.contains("both lanes"),
                    "{pair}: a port fix must name its Rust site and say `both lanes`, got \
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
            verdict.starts_with("ECHO — "),
            "{pair}: only an ECHO outcome ships its exclusion in the sub-step's own commit, so \
             only an ECHO verdict may declare zero rows — {step} says {verdict:?}"
        );
        let mut claimed = 0usize;
        for row in corpus.rows.iter().filter(|r| r.pair == *pair) {
            let link = first_match(chain_verdicts(&corpus, row)).unwrap_or_else(|| {
                panic!(
                    "{pair} '{}' vs '{}': {step} declares no rows, so the chain must claim every \
                     example row of the pair — this one is claimed by nothing",
                    row.rust, row.r4133
                )
            });
            assert_eq!(
                link,
                Link::Echo,
                "{pair}: {step}'s outcome is the echo table, so its rows must be claimed by that \
                 link and not by an earlier one"
            );
            claimed += 1;
        }
        assert!(
            claimed > 0,
            "{pair}: {step} declares no rows AND the corpus holds none — the pair vanished from \
             the evidence base instead of being excluded"
        );
    }
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(
        led.owner(Owner::Rp3),
        DECLARED_RP3,
        "RP3 inherits (rows, pairs, rows on in-scope pairs) — unchanged while RP3.1's entries \
         are staged into RP4.1"
    );
}

/// **The settled-outcome taxonomy is the plan's three, pinned literally** — like
/// [`NOT_A_PIN`] and [`LEDGER_ENTRY_PINS`], because it is the table the routing
/// guard dispatches on: a tag quietly added here would carry whatever
/// obligations its author felt like writing.
///
/// Plan §WP-RP3 (`R4133_PROPS_PLAN.md:1084-1090`): "exactly one outcome — a port
/// bug **fixed in both lanes**, or an upstream/echo divergence **excluded +
/// pinned** (ledger entry per §1.1(e) or echo row per RP2.3), or an upstream bug
/// **reported** with its exclusion + pin" — the reported case lands as one of the
/// two exclusions, which is why the tags are three and not four.
#[test]
fn the_settled_outcome_taxonomy_is_the_plans_three() {
    assert_eq!(
        RP3_SETTLED_SHAPES
            .iter()
            .map(|(t, _)| *t)
            .collect::<Vec<_>>(),
        ["LEDGER", "ECHO", "FIX"],
        "the three settled shapes plan §WP-RP3 sanctions, and nothing else"
    );
    for (tag, owes) in RP3_SETTLED_SHAPES {
        assert!(
            !owes.is_empty(),
            "{tag}: a tag with no stated obligation is a hole in the guard"
        );
    }
}

/// **Naming a witness is a whole-identifier match, not a substring one** — the
/// self-test for [`names_identifier`], whose reason for existing is the exact
/// pair of pins RP3.1 landed.
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

/// **The staged r4133 `property` entries have NOT landed — and this is the
/// tripwire that turns RP4.1's landing into a red test** (RP3.1 audit
/// settlement, 2026-08-24).
///
/// [`DECLARED_RP3`] and [`RP3_ROUTING`] promise that a settled sub-step's rows
/// leave the work list once its artifact lands. Nothing in this file can deliver
/// that on its own: the chain is [`Link::ORDER`]'s four links, none of which
/// reads `tests/corpus/ledger.json`, and [`declare`] routes bin-7 root-cause
/// rows to [`Owner::Rp3`] unconditionally. So the accounting move is a hand edit
/// in RP4.1's commit, and the only way to make a hand edit unmissable is to fail
/// loudly the moment it becomes due.
///
/// The condition is deliberately the whole class, not one sub-step's ids:
/// **any** `property`-scoped entry on the `r4133` channel means the unmask commit
/// is landing staged entries, which is exactly when every staged sub-step's rows
/// must be re-declared. Staged by WP-RP3 today, **eight**, every one owed a
/// re-declaration at RP4.1 (plan §RP4.1 precondition 2 lists the same set):
///
/// * RP3.1's two — `r4133-swtcontrol-delay-ignored-time` and
///   `r4133-swtcontrol-delay-ignored-midi`;
/// * RP3.2's four — `r4133-windgen-kvar-dispatched-daily` / `-delta` / `-dyn` /
///   `-dynfault`;
/// * RP3.4's two — `gic-pct-r2-honoured-gictransformer-r4133-props` and
///   `gic-pct-r2-honoured-midi-r4133-props` (staged 2026-08-24);
///
/// — plus RP1.4's, staged outside WP-RP3, and whatever RP3.5+ stages.
/// **RP3.3 is not among them and never will be**: it closed `ECHO`
/// (2026-08-24), its exclusion is the `PROPS_ECHO_R4133` row that shipped in its
/// own commit, and its [`RP3_ROUTING`] row was retired to `0, 0` there — so
/// RP4.1 owes it no accounting move at all (RP3.3 audit settlement corrected
/// this list, which still named it).
#[test]
fn the_staged_r4133_property_entries_have_not_landed_yet() {
    let path = repo_root().join(LEDGER);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let doc: serde_json::Value = serde_json::from_str(&text).expect("ledger.json is JSON");
    let landed: Vec<&str> = doc["entries"]
        .as_array()
        .expect("ledger.json has an `entries` array")
        .iter()
        .filter(|e| e["channel"] == "r4133")
        .filter(|e| {
            e["match"]
                .as_array()
                .is_some_and(|ms| ms.iter().any(|m| m["field"] == "property"))
        })
        .map(|e| e["id"].as_str().unwrap_or("<no id>"))
        .collect();
    assert!(
        landed.is_empty(),
        "r4133 `property` ledger entries have landed ({landed:?}), so the RP4.1 unmask is here \
         — now move the RP3 accounting BY HAND, in that same commit: retire each settled \
         RP3_ROUTING row whose entry landed (its rows are excluded now, not merely declared), \
         shrink DECLARED_RP3 by exactly those rows, and re-state this test against whatever is \
         still staged. Nothing does it for you: no link of the chain reads this file."
    );
}

/// **RP3.1's census decomposition is read off the corpus, not off its own
/// prose** (RP3.1 audit settlement, 2026-08-24).
///
/// The sub-step's conclusion — *exactly two* drafted ledger entries — is a
/// statement about cases: 42 cells of `swtcontrol.delay`, 24 of them in scope,
/// 12 + 12 over exactly the two `engines: r4133` decks that type `delay=`. Until
/// this test those numbers lived in the routing verdict's prose and in STATUS,
/// where changing `24` to `25` left the whole suite green.
///
/// Here every one of them is derived, from three independent places, and each
/// derivation is reconciled against the next:
///
/// * the **decks** give the control count and the typed value (a corpus-wide
///   sweep, so a new SwtControl deck cannot appear unnoticed —
///   [`RP31_DELAY_CASES`] and [`RP31_NO_DELAY_CASES`] must together be every
///   deck that declares one);
/// * `population.lock.json` gives each case's `steps=` and `engines=`, so
///   `cells = controls × steps` and "in scope" is the lock's own answer, not a
///   transcription;
/// * the **frozen census** (`bins.tsv`'s 42/24 and `examples_full.txt`'s two
///   rows, 36 and 6 cells) is what the products must add up to, per spelling and
///   in total.
///
/// Then the two consumers are tied to the result: the in-scope cases are exactly
/// the cases [`LEDGER_ENTRY_PINS`] cites (one drafted entry each, no more), and
/// the routing verdict must carry the derived figures verbatim.
#[test]
fn the_rp31_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "swtcontrol.delay";
    const STEP: &str = "RP3.1";

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
        "every corpus deck declaring a SwtControl must sit in RP3.1's decomposition with its \
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
        // Both halves of "does r4133 gate this case?" — see `r4133_skipped_cases`
        // (RP3.4's audit settlement found the `engines=`-only shorthand has a
        // live counterexample; no RP3.1 case is skipped today, so no number here
        // moves, and one becoming skipped now reds instead of passing).
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
         cells owe none (the RP4.1 unmask never compares them)"
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
            "the RP3.1 verdict must carry the derived census — {phrase:?} is missing from \
             {verdict:?}"
        );
    }
    assert!(
        names_identifier(
            verdict,
            "the_rp31_census_decomposition_is_read_off_the_corpus"
        ),
        "the verdict must name the test that derives its numbers, so a rename cannot orphan it"
    );
}

/// **RP3.2's census decomposition is read off the corpus, not off its own
/// prose** — [`the_rp31_census_decomposition_is_read_off_the_corpus`]'s shape,
/// applied to `windgen.kvar`.
///
/// The sub-step's conclusion — *exactly four* drafted ledger entries — rests on
/// a decomposition that must close over **every** cell of the pair and every
/// corpus deck that could add one:
///
/// * the **decks** give the WindGen count and the `kW=`/`pf=`/`kVA=` tokens
///   ([`windgen_facts`]), swept corpus-wide so a new WindGen deck cannot appear
///   unnoticed — [`RP32_WINDGEN_CASES`] and [`RP32_WINDGEN_SKIPPED_DECKS`] must
///   together be every deck that declares one, and the skipped pair is checked
///   against `skipped_oracle_issue.json` itself, with the nonzero base it
///   *would* contribute if promoted;
/// * the **`QMode=` token** ([`windgen_dispatch`]) says which arm of
///   `WindGen.pas:1276-1322` each deck selects, so "r4133 renders the dispatched
///   zero" is read off the decks (none of the five types one ⇒ the `Else` arm)
///   instead of being asserted about them — and the held-out pair, which takes
///   the volt-var arm instead, cannot be described as if it shared the
///   mechanism;
/// * the **arithmetic** derives our render from those tokens
///   ([`windgen_kvar_base`], the two `WindGen.pas` branches) rather than
///   transcribing it — no deck types `kvar=`, so the whole census is a side
///   effect of `pf=`/`kVA=`, and a cell exists exactly where the derived base is
///   nonzero;
/// * `population.lock.json` gives each case's `steps=`/`engines=`, so
///   `cells = WindGens × steps` and "in scope" is the lock's answer — *plus*
///   the ledger's, since the RP3.4 audit settlement: an `engines: "both"` case
///   whose r4133 channel a `skip` entry drops is not gated there
///   ([`r4133_skipped_cases`]; no WindGen case is skipped today);
/// * the **frozen census** (`bins.tsv`'s 4/4 and `examples_full.txt`'s three
///   rows, 2 + 1 + 1 cells) is what the products must add up to, per spelling
///   and in total — and every one of those rows must have `0` on the r4133 side,
///   which IS the divergence.
///
/// Then the consumers are tied to the result: the in-scope cases are exactly the
/// cases [`LEDGER_ENTRY_PINS`] cites (one drafted entry each, no more), and the
/// routing verdict must carry the derived figures verbatim.
#[test]
fn the_rp32_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "windgen.kvar";
    const STEP: &str = "RP3.2";

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
        "every corpus deck declaring a WindGen must sit in RP3.2's decomposition with its \
         measured count and kW=/pf=/kVA= tokens (and no `kvar=`, the fifth field — neither typed \
         on the `New` line nor edited in later) — a new one changes how many ledger entries the \
         pair owes"
    );

    // The mechanism behind r4133's `0`, read off the decks: none of the five
    // selects a Q-dispatch arm, so `WindModelDyn.QMode` stays Create's 0
    // (`WindGen.pas:1020`) and the steady-state case falls to `Else kvarCalc :=
    // 0` (`:1320-1321`).
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
             (WindGen.pas:1306-1310) — then the doc's reading of this deck is wrong"
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
        // Both halves of the scope question, as in RP3.1/RP3.4 — see
        // `r4133_skipped_cases`.
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
            "the RP3.2 verdict must carry the derived census — {phrase:?} is missing from \
             {verdict:?}"
        );
    }
    assert!(
        names_identifier(
            verdict,
            "the_rp32_census_decomposition_is_read_off_the_corpus"
        ),
        "the verdict must name the test that derives its numbers, so a rename cannot orphan it"
    );
}

/// **The NCIM sweep's readers see every spelling the engine accepts** — the
/// self-test for [`algorithm_values`], [`selects_ncim`] and
/// [`redirect_targets`], added by RP3.3's audit settlement (2026-08-24).
///
/// The corpus types the option exactly one way today — seven live mentions, all
/// `algorithm=` in full, none in a redirected non-`.dss` script — so the widened
/// readers are dormant on it, and a dormant reader proves nothing about the
/// completeness claim that rests on it
/// ([`the_rp33_census_decomposition_is_read_off_the_corpus`]'s first assertion).
/// Driving them here on the spellings r4133 itself accepts is what makes that
/// claim enforced rather than described: before the settlement `set algo=ncim`
/// and `Set algorithm=nc` were both silent misses, and a `Set algorithm=NCIM`
/// inside a `Redirect`ed `.txt` was invisible to the sweep's whole universe.
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

/// **RP3.3's census decomposition, derived instead of transcribed** — the same
/// shape as [`the_rp31_census_decomposition_is_read_off_the_corpus`] and
/// [`the_rp32_census_decomposition_is_read_off_the_corpus`], applied to
/// `generator.model`.
///
/// The sub-step's whole artifact count rests on the number **2**: an echo row of
/// `cells: 2` and an `ECHO_ROWS_ON_R4133_ONLY_CASES` exposure of `(2, 2)`. So the
/// decomposition must close over every cell of the pair and every corpus deck
/// that could add one:
///
/// * the **mechanism** is deck-level — only a deck that runs `Set algorithm=NCIM`
///   can move a generator's live `GenModel` off its typed token — so
///   [`sets_ncim`] is swept corpus-wide and [`RP33_NCIM_CASES`] +
///   [`RP33_NCIM_HELD_OUT`] must together be *every* such deck. The sweep's own
///   two escape hatches are closed rather than assumed shut (RP3.3 audit
///   settlement): abbreviated option spellings are read as the option
///   ([`algorithm_values`]) and the value is matched the way r4133 matches it
///   ([`selects_ncim`], two characters), and the `.dss`-only universe is
///   completed by [`redirected_non_dss_scripts`], which sweeps every script a
///   deck `Redirect`s whatever its extension;
/// * the **element facts** come from the decks ([`generator_model_facts`]): how
///   many `Generator`s each declares and the `model=` they type, so
///   "one convertible generator each" is read rather than asserted, and
///   `ncim_pq.dss` — NCIM with no generator at all — is the control that shows
///   the mechanism alone produces no cell;
/// * the **echo claim itself is derived per case**: r4133's side of the frozen
///   census row must be the deck's own typed token, which IS what
///   `EchoParse` means for this pair (`generator.pas:625` writes the store,
///   `:3007-3038` has no arm 6 to overwrite it), while our side is the converted
///   live model `4`;
/// * `population.lock.json` gives each case's `steps=`/`engines=`, so
///   `cells = convertible generators × steps` and "in scope" is the lock's
///   answer — and here every in-scope case is `engines=r4133` exactly, which is
///   why the pair's witness must be a pin and can never be a capi channel;
/// * the **held-out decks are named, not swept away**: both other NCIM decks run
///   `model=3` generators and are absent from the frozen census because they are
///   `kind: "large"`, i.e. outside the census population — asserted from the
///   lock, with their generators counted in the redirected file that declares
///   them;
/// * the **frozen census** (`bins.tsv`'s 2/2 and `examples_full.txt`'s single
///   `'4'` vs `'3'` row) is what the products must add up to.
///
/// Then **all three** consumers are tied to the result: the shipped echo row's
/// `cells` column, its `ECHO_ROWS_ON_R4133_ONLY_CASES` exposure — both columns,
/// through `props_norm::r4133_only_exposure` — and the routing verdict, which
/// must carry the derived figures verbatim.
#[test]
fn the_rp33_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "generator.model";
    const STEP: &str = "RP3.3";

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
        "every corpus deck that runs `Set algorithm=NCIM` must sit in RP3.3's decomposition — a \
         new one is a deck whose generators can convert, i.e. a cell this pair does not account \
         for"
    );
    // …and the hatch underneath that sweep, closed rather than argued away
    // (RP3.3 audit settlement): the walk above sees `.dss` files, while a deck
    // can `Redirect` a script of any extension and the corpus really does.
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
            "{case}: the census population is every live NON-LARGE case (triage.md §Method), so \
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
        // The other half of the scope question — see `r4133_skipped_cases`: an
        // r4133-only case whose channel a ledger `skip` drops is compared by
        // NOBODY, and its cells could not be in scope at all (RP3.4's audit
        // settlement; no RP3.3 case is skipped today).
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
    // …and the row's exposure entry, the second consumer. Its `(cells, cases)`
    // is claimed to be derived per case; before the RP3.3 audit settlement no
    // test read the entry at all and its `cases` column had no value lock
    // anywhere, so `(2, 5)` shipped green. Both columns are this derivation's.
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
            "the RP3.3 verdict must carry the derived census — {phrase:?} is missing from \
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

/// **RP3.4's census decomposition is read off the corpus, not off its own
/// prose** — [`the_rp31_census_decomposition_is_read_off_the_corpus`]'s shape,
/// applied to `gictransformer.r2` one element at a time.
///
/// The sub-step's conclusion — *exactly two* drafted ledger entries — rests on a
/// decomposition that must close over **every** cell of the pair and every
/// corpus file that could add one:
///
/// * the **files** give the declarations and their resistance specs
///   ([`gictransformer_elements`]), swept over the whole corpus **file
///   universe** and not only its `.dss` files, so a class declared from a
///   `Redirect`ed script cannot hide from a completeness claim (RP3.3's
///   settlement made that hole explicit; here it is closed by construction —
///   [`collect_files`] sees every file, redirected or not);
/// * the **branch** each element selects is read off its own tokens rather than
///   assumed: `%R1=`/`%R2=` set `FpctRSpecified := TRUE`
///   (`Version8/Source/PDElements/GICTransformer.pas:349`) and `R1=`/`R2=` set
///   it FALSE (`:343`), and an element that typed both would have to be
///   adjudicated by parse order, so the reader refuses one instead of guessing;
/// * the **arithmetic** derives *both* sides from those tokens — ours
///   `ZBase2*%R2/100`, the oracles' `ZBase2*%R1/100` off the `:495` slip — and
///   renders them at the getter's own eight significant digits ([`sig8`],
///   `Format('%.8g',[1.0/G2])`, `:723`), so the frozen spelling is *reconciled*
///   and not transcribed;
/// * `population.lock.json` gives each case's `steps=`/`engines=`, so
///   `cells = diverging elements × steps`, and "in scope" is **both** halves of
///   the question: the lock's `engines=` must name r4133 *and* no ledger `skip`
///   entry may drop the channel ([`r4133_skipped_cases`] — the RP3.4 audit
///   settlement's correction, since `engines: "both"` alone is not decisive);
/// * the **frozen census** (`bins.tsv`'s 2/2 and the single `examples_full.txt`
///   row) is what the products must add up to.
///
/// Every declaration lands in exactly **one** of three classes, each with its own
/// counter and its own assertion, so no element can fall through a `continue`
/// into a mismatch that gets reported as something else: `ohms` (the reverse
/// branch, no cell on any engine), `coincident` (`%R`-specified with
/// `%R1 == %R2`, where the slip is invisible — empty on today's corpus, and
/// asserted so rather than left to red elsewhere) and `diverging` (a cell), of
/// which every one must sit on an r4133-gating case or the "one drafted entry per
/// diverging element" conclusion no longer follows.
///
/// Two independent cross-checks then bound the same population read from
/// outside RP3.4's own arithmetic: [`RP34_CLASS_WIDE_PAIRS`] (`Σ declared ×
/// steps`, which no `%R` reasoning enters) and the **positive measurement** —
/// the twenty ohms-specified GICTransformers, nineteen of them on r4133-gating
/// cases, that produce zero cells. Finally the consumers are tied to the result:
/// the in-scope diverging cases are exactly the cases [`LEDGER_ENTRY_PINS`]
/// cites (one drafted entry each, no more), and the routing verdict must carry
/// the derived figures verbatim.
#[test]
fn the_rp34_census_decomposition_is_read_off_the_corpus() {
    const PAIR: &str = "gictransformer.r2";
    const STEP: &str = "RP3.4";

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
        "every GICTransformer the corpus declares must sit in RP3.4's decomposition with its \
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
         skip really went away, re-read RP3.4's capi-only record in STATUS with it; got \
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
             one branch of RecalcElementData by its tokens — GICTransformer.pas:343 vs :349 \
             resolve it by parse order, and this derivation refuses to guess"
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
             shape — winding 2 would take the Create default 0.2 (GICTransformer.pas:458-459) \
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
            // `%R1 == %R2` makes the slip invisible — the same value by
            // coincidence, exactly as `windgen_snap.dss`'s `pf=1.0` is in RP3.2,
            // whose `no_cell` bucket this mirrors. It is a THIRD class, not a
            // silent skip: counted here and adjudicated at (4b), so an element
            // that lands in it cannot surface as "something fell between the
            // ohms and the diverging elements" (RP3.4 audit settlement).
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
             own 8 significant digits — ours ZBase2*%R2/100, the oracles' ZBase2*%R1/100 off \
             GICTransformer.pas:495"
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
            (*want_cells, *want_in_scope),
            "{pair} is answered by every GICTransformer on every step, so its census split must \
             be the derived sum(declared x steps) over the whole corpus and over the r4133-gating \
             cases — a mismatch means the population moved under RP3.4's two-cell conclusion"
        );
    }

    // (4b) The positive measurement — this sub-step's `civanlar.dss`: the ohms
    //      spec is the majority of the population and it diverges nowhere. The
    //      three classes are adjudicated one by one, each naming its own cause,
    //      so a corpus that grows an element reds where the reason is (RP3.4
    //      audit settlement: the coincidence branch used to have no counter, and
    //      an element taking it surfaced as "fell between the two classes").
    assert!(
        coincident.is_empty(),
        "a %R-specified GICTransformer whose two percentages coincide renders the same number \
         on both engines, so it carries no cell and owes no ledger entry — today's corpus has \
         none, and one appearing means RP3.4's ohms/coincident/diverging split (and the two-cell \
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
         reach, and RP3.4's \"one drafted entry per diverging element\" conclusion would no \
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
            "the RP3.4 verdict must carry the derived census — {phrase:?} is missing from \
             {verdict:?}"
        );
    }
    assert!(
        names_identifier(
            verdict,
            "the_rp34_census_decomposition_is_read_off_the_corpus"
        ),
        "the verdict must name the test that derives its numbers, so a rename cannot orphan it"
    );
}

/// **The GICTransformer reader really separates the two resistance specs** — the
/// self-test for [`gictransformer_elements`] and [`GIC_KEYS`], added with RP3.4.
///
/// The whole decomposition turns on one distinction: `%R1=0.2` must read as a
/// *percentage* token and `R1=0.2` as an *ohms* one, because that is what
/// selects the branch of `RecalcElementData` an element takes
/// (`Version8/Source/PDElements/GICTransformer.pas:349` vs `:343`) and therefore
/// whether it carries a census cell. The two keys are prefixes of one another,
/// so a reader that got the separator rule wrong would silently classify every
/// `%R` element as ohms-specified — and the sub-step's answer would become "zero
/// cells, no ledger entry", green and wrong.
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

/// The nine corpus decks that actually run `Reduce`, i.e. the whole population
/// in which `TLineObj.MergeWith` — RP3.5's routine — can ever execute.
///
/// Cited rather than derived-and-forgotten so that a tenth reduce deck reds
/// [`the_rp35_census_decomposition_is_read_off_the_corpus`] instead of quietly
/// widening the pair's population. The other two columns are the two halves of
/// "can this deck carry a `line.units` cell", each read off the deck itself and
/// each re-derived by the test:
///
/// * the `Set ReduceOption=` it selects ([`reduce_strategy`]), which decides
///   whether `MergeWith` runs at all ([`RP35_MERGING_STRATEGIES`]);
/// * whether it declares a line that is **not** a 3-phase symmetrical-components
///   line ([`declares_a_non_sym3_line`]) — the exact negation of r4133's
///   sym-branch test `SymComponentsModel and OtherLine.SymComponentsModel and
///   (nphases = 3)` (`Version8/Source/PDElements/Line.pas:1695`), and therefore
///   the necessary condition for a matrix-branch merge, the only branch that
///   carried the divergence.
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
/// exemption cannot widen silently; [`the_rp35_census_decomposition_is_read_off_the_corpus`]
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

/// **RP3.5's census decomposition is read off the corpus, not off its own
/// prose** — [`the_rp34_census_decomposition_is_read_off_the_corpus`]'s shape,
/// applied to `line.units`.
///
/// The sub-step's two load-bearing numbers are *3 cells, 0 of them in scope*,
/// and both were prose until this test. The plan text itself had the population
/// wrong — it named `reduce_mergeparallel`, which carries **zero** `line.units`
/// cells because its merge is a 3-phase symmetrical-components merge and all
/// three engines render `km` — so the derivation below asserts that counter-claim
/// explicitly and not merely the total.
///
/// It closes over the whole corpus in four steps:
///
/// * the **population**: every file — not only every `.dss` file — that issues a
///   bare `Reduce` ([`runs_reduce`]), which is the only way `MergeWith` runs at
///   all. That set must be exactly [`RP35_REDUCE_DECKS`], so a tenth reduce deck
///   reds here;
/// * the **branch**: whether a deck can reach the matrix branch is read off its
///   own declarations ([`declares_a_non_sym3_line`], the negation of
///   `Line.pas:1695`), not assumed. Exactly one deck can — `midi_reduce`, whose
///   three merges are the three census cells — and `reduce_mergeparallel` is
///   asserted to be on the other side of that line;
/// * the **scope**: each case's `steps=`/`engines=` come from
///   `population.lock.json`, and the r4133 channel must not be `skip`ped
///   ([`r4133_skipped_cases`] — the RP3.4 audit settlement's correction). Three
///   reduce decks do gate r4133 (`reduce_breakloop`, `reduce_dangling`,
///   `reduce_laterals`), so the zero is not a statement about the channel; none
///   of them can carry a cell, which is *why* the pair has 0 in-scope cells and
///   why RP3.5 owed a **capi** ledger entry landed live instead of an r4133 one
///   staged to RP4.1 (plan §1.1(e));
/// * the **reconciliation**: the derived totals must be `bins.tsv`'s and
///   `examples_full.txt`'s own numbers for the pair.
///
/// **The frozen `rust` column is historical from RP3.5 onwards.** The extracts
/// under `tests/corpus/props_r4133/` are a *data* lock recording the 2026-08-08
/// measurement (`props_r4133_evidence_lock.rs`), and RP3.5 is the first RP3
/// sub-step whose fix moved the port's own render: the port now answers `kft`,
/// the r4133 spelling, where the frozen row records `'none'`. The row is read
/// here as evidence of what was measured, never as a claim about today's engine
/// — that claim is
/// `exec::tests::reduce::the_corpus_reduce_decks_merged_lines_render_kft`.
#[test]
fn the_rp35_census_decomposition_is_read_off_the_corpus() {
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
        "every corpus file that issues `Reduce` must sit in RP3.5's decomposition — a new one \
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
            "{case}: whether a merge here could take the MATRIX branch (the negation of              Line.pas:1695) is the other half of whether it can carry a `line.units` cell"
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
        "all three `line.units` cells are on the one deck that both merges and can take the          matrix branch — `reduce_laterals` declares 1-phase laterals but its strategy only          removes branches (ReduceAlgs.pas:372, :453), and `reduce_mergeparallel` merges but only          3-phase symmetrical-components lines"
    );
    let mergeparallel = RP35_REDUCE_DECKS
        .iter()
        .find(|(c, ..)| *c == "modes:reduce/reduce_mergeparallel.dss")
        .expect("the deck the plan text named is in the table");
    assert!(
        !can_carry(mergeparallel),
        "RP3.5's counter-claim: `reduce_mergeparallel` merges two 3-phase          symmetrical-components lines, takes the SYM branch, renders `km` on all three engines          and contributes ZERO `line.units` cells — the plan text named it and was wrong          (RP3.5, 2026-08-28)"
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
            "{case}: a reduce deck that BOTH gates r4133 and can take the matrix branch would              carry an in-scope `line.units` cell — RP3.5's pair would stop being 0-in-scope, and              with it the decision to land a live `capi_v0145` ledger entry instead of an r4133              entry staged to RP4.1 (plan §1.1(e))"
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
        "the frozen 2026-08-08 measurement — historical on the `rust` side since RP3.5 fixed the \
         port, and never edited (RP0.1 evidence lock)"
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
    assert_eq!(in_scope_cells, 0, "RP3.5's second load-bearing number");

    // (5) The counter-claim from the other side: `reduce_mergeparallel` types
    //     `units=km` throughout, so a cell there would have to spell `km` — and
    //     the frozen census holds no such row.
    assert!(
        !corpus
            .rows
            .iter()
            .any(|r| r.pair == PAIR && (r.rust == "km" || r.r4133 == "km")),
        "a `line.units` row spelling `km` would mean the SYM branch diverges too — which is what \
         the plan text assumed and the live probe disproved"
    );
}

/// Every corpus file that declares a `Line` carrying **both** a `linecode=` and
/// a `switch=` — the whole population in which RP3.6's arm can ever fire — with
/// the two counts the sub-step's arithmetic rests on: how many of those
/// declarations type `switch=` **after** the code, and how many before.
///
/// The order is the mechanism, not a detail. The parser runs each property's
/// side effect in the order its token is typed, so `linecode=… switch=…` runs
/// `FetchLineCode` (r4133 `Version8/Source/PDElements/Line.pas:385-413`, which
/// sets `CondCode` and arms `FLineCodeSpecified`) and *then* arm 15 — the arm
/// the port used to kill the flag in. `switch=… linecode=…` runs the two the
/// other way round, so `FetchLineCode` re-arms the flag last and all three
/// engines agree. That is why `LVTestCaseNorthAmerican`'s 160
/// `switch=yes linecode=switch` declarations can carry no cell, and it is
/// pinned on the engine side by the `swb` leg of `exec::tests::line_fetch::
/// switch_yes_keeps_the_linecode_and_its_units_conversion`.
///
/// Cited rather than derived-and-forgotten, so a new deck of this shape reds
/// [`the_rp36_census_decomposition_is_read_off_the_corpus`] instead of quietly
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

/// The two cases that carry all five `line.linecode` cells, with their counts —
/// `zone_2/Branches.dss:93,:95,:479` and `zone_3/Branches.dss:161,:165`. The
/// plan text named a third deck that carries none; see [`RP36_STOCTRL_CASE`].
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

/// The deck the plan text wrongly called "the affected decks" — right shape,
/// zero cells, because its case is `kind=large` and `force_properties` never
/// turns a large case's property compare on.
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
/// dropped, so [`the_rp36_census_decomposition_is_read_off_the_corpus`] asserts
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

/// **RP3.6's census decomposition is read off the corpus, not off its own
/// prose** — [`the_rp35_census_decomposition_is_read_off_the_corpus`]' shape,
/// applied to `line.linecode`.
///
/// The sub-step's two load-bearing numbers are *5 cells, all 5 in scope*, and
/// the second is why RP4.1 breaks without it: `line.linecode` is the only RP3.5+
/// pair the unmask will actually compare. Both were prose. The plan text also
/// had the population wrong — it named
/// `Examples/StoCtrl_Current_PeakShave/Line.DSS`, a deck of exactly the right
/// shape (nine `linecode=… Switch=True units=m` declarations) that contributes
/// **zero** cells because its case is `kind=large` and `force_properties`
/// (`crates/dss-core/tests/corpus_gate/scheduler.rs`) never turns a large case's
/// property compare on — so the derivation asserts that counter-claim
/// explicitly and not merely the total.
///
/// Five steps, closing over the whole corpus:
///
/// * the **population**: one pass over every corpus file — not only every `.dss`
///   file — collecting the qualifying `Line` declarations
///   ([`switch_linecode_line_decls`]), the continuation lines that would defeat a
///   one-physical-line sweep ([`continued_line_tokens`]) and the
///   `Redirect`/`Compile` edges the ownership walk needs. The declaring files
///   must be exactly [`RP36_SWITCH_LINECODE_DECKS`];
/// * the **ownership**: which case pulls a declaring file in is walked, not
///   guessed from the directory tree, from every master in
///   `population.lock.json` over those edges ([`redirect_paths`]) —
///   `zone_2/Branches.dss` is reached by its own zone master *and* by
///   `Master_Interconnected.dss`, and only the first is property-compared;
/// * the **scope**: a cell exists where `force_properties` turns the capi
///   property compare on (`gates_capi() && !kind.starts_with("large")` for
///   `solvable_now`), and it is *in scope* where the r4133 channel also gates the
///   case and no ledger `skip` drops it ([`r4133_skipped_cases`] — the RP3.4
///   audit settlement's correction);
/// * the **counter-claims**: the deck the plan named, and the token order that
///   silences `LVTestCaseNorthAmerican`'s 160 declarations;
/// * the **reconciliation**: the derived totals must be `examples_full.txt`'s
///   per-spelling counts and `bins.tsv`'s cells / in-scope cells for the pair.
///
/// **The frozen `rust` column is historical from RP3.6 onwards.** The extracts
/// under `tests/corpus/props_r4133/` are a *data* lock recording the 2026-08-08
/// measurement (`props_r4133_evidence_lock.rs`); the port now answers `99`/`98`,
/// the r4133 spelling, where the frozen rows record `''`. They are read here as
/// evidence of what was measured, never as a claim about today's engine — that
/// claim is
/// `exec::tests::line_fetch::switch_yes_keeps_the_linecode_and_its_units_conversion`.
#[test]
fn the_rp36_census_decomposition_is_read_off_the_corpus() {
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
         RP3.6's decomposition — a new one changes the population the pair's 5 cells are counted \
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
        // `force_properties` (`corpus_gate/scheduler.rs`), verbatim: the capi
        // property compare is on for a `solvable_now` case that gates capi and is
        // not `large`, and for a family case that gates capi (all three families
        // set the family-level flag).
        let gates_capi = engines != "r4133";
        let property_compared =
            gates_capi && (!case.starts_with("solvable_now:") || !kind.starts_with("large"));
        if !property_compared {
            continue;
        }
        // The r4133 half — `engines` alone is not enough (RP3.4's correction).
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

    // (4) The plan's counter-claim, both halves: the deck it named really does
    //     have the shape, and really does contribute nothing.
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
        "R4133_PROPS_PLAN RP3.6 called StoCtrl_Current_PeakShave/Line.DSS `the affected decks`; it \
         contributes ZERO of the 5 cells and no property-compared case even reaches it: \
         {owners_of_stoctrl:?}"
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
         re-arms the flag last (Line.pas:413) and every engine renders the code"
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
        "the frozen 2026-08-08 measurement — two spellings, historical on the `rust` side since \
         RP3.6 fixed the port, and never edited (RP0.1 evidence lock)"
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
        "RP3.6's second load-bearing number: every cell of the pair is in scope, which is why the \
         RP4.1 unmask compares them and breaks without this sub-step"
    );
}

/// **RP3.7's census decomposition, as data instead of prose** — every corpus
/// case that declares a `SwtControl`, with the two facts the pair's cells hang
/// off: `(case, tokens the render carries, controls whose `State` renders OPEN)`.
///
/// The control COUNT is deliberately not repeated here: it is
/// [`RP31_DELAY_CASES`]/[`RP31_NO_DELAY_CASES`]' column, re-measured from the
/// decks by [`the_rp31_census_decomposition_is_read_off_the_corpus`] and by this
/// test's own sweep, so the two decompositions cannot disagree about the
/// population. What is new here is the *shape* of each case's render, which is
/// what makes `swtcontrol.normal`/`state` a pair at all:
///
/// * **tokens** — the getter renders one token per CONTROLLED-ELEMENT phase
///   (`Version8/Source/Controls/SwtControl.pas:589-599` / `:600-610`), so every
///   case is 3 except `makeposseq_ctrl`, whose `line.sw` drops to one phase at
///   `MakePosSequence`. That single case is the whole of the `'[closed, ]'`
///   spelling both frozen example rows record;
/// * **open** — how many of the case's controls answer `open` after its own
///   `post` commands have run. `midi_swtcontrol` and `swtcontrol_time` arm a
///   delayed `action=open` (1 control each) and `civanlar.dss` declares three of
///   its sixteen ties `Action=o` (`5_11`, `7_16`, `10_14`); every other control
///   in the corpus is closed, and `Normal` is closed on all of them — untyped
///   ties inherit `Create`'s all-CLOSED array (`:299-307`) and the two
///   `swtcontrol` micro-decks type `normal=closed` explicitly.
///
/// Those two columns are what turn 59 cells into the frozen census's exact
/// per-spelling split, and the split is the sub-step's load-bearing conclusion:
/// 40 in-scope cells on three `engines: r4133` decks (which COMPARE after the
/// fix, hence no r4133 entry) against 19 on exactly five `capi_v0145` cases —
/// *hence exactly five landed ledger entries and no more*.
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

/// **RP3.7's census decomposition is read off the corpus, not off its own
/// prose** — [`the_rp31_census_decomposition_is_read_off_the_corpus`]' shape,
/// for the sub-step that settled `swtcontrol.normal`/`swtcontrol.state`.
///
/// The conclusion this derives is "**59 cells per pair, 40 in scope, 19 on
/// exactly five `capi_v0145` cases — hence exactly five landed ledger entries
/// and no more**", plus the fact that makes the r4133 side need none: the 40
/// in-scope cells sit on three `engines: r4133` decks and COMPARE after the fix
/// (re-measured live 2026-09-02 with the RP0.2 census knob, which reports zero
/// `Normal`/`State` rows on that channel; the pins hold the values meanwhile).
///
/// Four independent places are reconciled against each other:
///
/// * the **decks** give the control count and the completeness claim — the sweep
///   fails if any deck outside [`RP31_DELAY_CASES`]/[`RP31_NO_DELAY_CASES`]
///   declares a `SwtControl`, so a corpus that grows one reds here, where the
///   "exactly five entries" conclusion is drawn;
/// * `population.lock.json` gives each case's `steps=` and `engines=`, so
///   `cells = controls × steps` and "in scope" is the lock's own answer;
/// * the **frozen census** (`bins.tsv`'s 59/40 twice and `examples_full.txt`'s
///   two + three rows) is what the products must add up to, **per spelling**:
///   `normal` 58 + 1 and `state` 31 + 27 + 1, which only comes out right if the
///   `tokens`/`open` columns above are right;
/// * `tests/corpus/ledger.json` gives the landed `capi_v0145` entries, which
///   must be exactly one per out-of-scope case, each carrying one `property`
///   match row per (control, property).
///
/// The frozen `rust` column stays `'closed'`/`'open'` — the 2026-08-08
/// measurement, historical since RP3.7 and never edited (RP0.1 evidence lock) —
/// so these rows are still *declared* to [`DECLARED_RP35`]; that constant tracks
/// the tree, not the work.
#[test]
fn the_rp37_census_decomposition_is_read_off_the_corpus() {
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
        "every corpus deck declaring a SwtControl must sit in RP3.7's decomposition — a new one \
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
        // Both halves of "does r4133 gate this case?" (RP3.4's correction).
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
                "{pair}: the frozen `rust` column is the scalar the port rendered in 2026-08-08 — \
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
        "RP3.7's two load-bearing numbers: 59 cells per pair, 40 of them in scope — 80 in-scope \
         cells over the two pairs, which is the plan's acceptance criterion"
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
        .filter(|(_, step, _, _)| *step == "RP3.7")
        .map(|(id, ..)| *id)
        .collect();
    let landed: BTreeSet<&str> = mine
        .iter()
        .map(|e| e["id"].as_str().expect("an entry names its id"))
        .collect();
    assert_eq!(
        witnessed, landed,
        "each landed RP3.7 entry owes a LANDED_PROPERTY_ENTRY_PINS witness and vice versa"
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

/// **The whole chain's two accountings reconcile, RP2.4** — the successor
/// statement of the test above, now that all four links carry a value.
///
/// The claims census measured **2 982** claimed spellings on the r4133 channel
/// against this file's [`CLAIMED_TOTAL`] of 2 975 (RP3.3's re-run, 2026-08-24;
/// the RP2.4 audit settlement's figures were 2 981 / 2 974 and RP2.4's own
/// 3 036 / 3 029), and the seven-spelling gap is
/// asserted, not narrated: one [`LIVE_ONLY_SPELLINGS`] entry (the normalization
/// link's) plus six [`LIVE_ONLY_DISPLAY_SPELLINGS`] (the floor's). Each of the
/// six is checked to be
///
/// * claimed by the **shipped** floor predicate — so a narrowed floor reds here
///   rather than at the next census;
/// * claimed by the floor and by **no earlier link**, i.e. the floor is really
///   its first match and the census's `under-floor` tag is right;
/// * on a pair the frozen `bins.tsv` holds, which is exactly what bars the
///   spelling from the supplement — and absent from the vendored rows, so it
///   genuinely has no home here;
/// * on one of the **five** `vsource` pairs the recorded drift names, and
///   inside that pair's own frozen `max_rel` (RP2.4 audit round: without these
///   two the list carried only its own length, and any nearby invented number
///   passed — the mutation the auditor landed was `vsource.x1` `'1.4257'`,
///   which is 9.09e-05 against that pair's frozen 3.72e-05 ceiling).
#[test]
fn the_display_floors_live_only_spellings_reconcile_the_claims_census() {
    assert_eq!(
        CLAIMED_TOTAL + LIVE_ONLY_SPELLINGS.len() + LIVE_ONLY_DISPLAY_SPELLINGS.len(),
        CLAIMED_SPELLINGS_LIVE,
        "the vendored claim count plus both live-only lists must equal what the full-population \
         claims census measured (README §\"What RP2.4 moved\")"
    );
    let corpus = Corpus::load();
    let floor = props_norm::display_floor().expect("RP2.4 derived the floor");
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

/// **The echo table claims only its own 82 pairs, and the display floor claims
/// only what its derivation covers** — successor (RP2.4) of the RP2.3 test that
/// asserted the floor was still `None`.
///
/// That assertion going red is the design working: RP2.4 filled the last slot.
/// What replaces it is strictly stronger, because a floor is the one link that
/// could over-claim silently — it has no row set to audit — so both of its
/// bounds are checked over the same full-corpus walk:
///
/// * the echo link claims a row **only** on a pair `PROPS_ECHO_R4133` names, so
///   the exclusion cannot have leaked onto a pair no row cites;
/// * a pair the table does NOT name is still compared raw, even when its
///   spelling looks exactly like an echo (`''` on one side) — the mask is the
///   cited row set, never a shape heuristic;
/// * every row the floor claims is inside `props_norm::display_floor()` **in
///   the shipped metric**, and the WORST of them is the derivation's own
///   `6.431124e-05` — so a floor edited without re-deriving reds here;
/// * no row the floor claims is a structural one: each carries at least one
///   number on both sides, with identical non-numeric skeletons.
#[test]
fn the_echo_table_claims_only_its_cited_pairs_and_the_floor_only_its_derivation() {
    assert_eq!(
        PROPS_ECHO_R4133.len(),
        82,
        "the echo table: RP2.3's 81 (its bucket's 86 pairs minus the 5 the kill criterion \
         re-routed) plus RP3.3's generator.model, the first row a WP-RP3 sub-step contributed"
    );
    let floor = props_norm::display_floor().expect("RP2.4 derived the r4133 props display floor");
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
            // value rounded to the digits r4133 printed. This is the audit
            // settlement's core assertion — before it, "every claimed cell is a
            // `%[-].Ng` render" was a survey in a doc comment, and 55 rows
            // contradicted it (`RP39_ROUTING`).
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
         (load.pf, `%-.4g`, PCElements/Load.pas:2345) — measured {:e} on {}",
        worst.0,
        worst.1
    );
    // The five superseded pairs still carry `''`-on-one-side spellings in the
    // FROZEN extract — the shape bin 5 is built on — and take no echo row all
    // the same. (The live port renders a number on both sides since RP3.8; see
    // [`RP38_SUPERSEDED`].)
    for (pair, _, _, _) in RP38_SUPERSEDED {
        let (class, prop) = pair.split_once('.').expect("class.prop");
        assert!(
            !props_norm::has_echo_row(class, prop),
            "{pair}: an echo-LOOKING spelling is not an echo row"
        );
    }
    // …and neither is `gictransformer.r2`, which RP3.4 settled as `LEDGER`
    // (2026-08-24): its r4133 getter COMPUTES `Format('%.8g',[1.0/G2])` live
    // (`Version8/Source/PDElements/GICTransformer.pas:723`) off a conductance
    // `RecalcElementData` mis-derived at `:495`, so calling it an echo would
    // misname the mechanism — the exclusion is two staged ledger entries
    // instead. `generator.model` sat next to it here until RP3.3 root-caused it
    // TO this table — r4133's `model` getter has no arm 6 and answers the deck's
    // own token — so both assertions flip with their findings instead of being
    // quietly deleted.
    assert!(!props_norm::has_echo_row("gictransformer", "r2"));
    assert!(props_norm::has_echo_row("generator", "model"));
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
    // Liveness, RP2.4: the routed owner has DISCHARGED the row rather than
    // still holding it. RP2.3 asserted `led.owner(Owner::Rp24) ==
    // DECLARED_RP24` while that was `(2101, 71, 2021)`; the same equality now
    // reads `(0, 0, 0)`, which alone would be vacuous — so the cell is chased
    // to the link that actually took it.
    let corpus = Corpus::load();
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(
        led.owner(Owner::Rp24),
        DECLARED_RP24,
        "RP2.4 closed: the carved-out cell is claimed, not pending"
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
            "{pair}: the carve-out's declared owner is RP2.4, and RP2.4's mechanism is the \
             display floor — 5.0e-06 apart, r4133's own MakePosSequence round-trip"
        );
    }
}

/// **RP2.4's residual is proved out of scope, not waved away** — the
/// both-ways guard for [`RP24_OUT_OF_SCOPE`], the only place this plan lets a
/// row leave a bucket without a mechanism claiming it.
///
/// The rule is a *proof* (a spelling cannot exceed the maximum taken over the
/// in-scope cells and still be one of them), so everything it rests on is
/// asserted here rather than trusted:
///
/// * each cited ceiling is **the pair's own frozen `max_rel_in_scope`**, read
///   back from `bins.tsv` — a hand-typed ceiling that drifted from the evidence
///   would silently widen §1.3;
/// * each cited row count is what the walk really re-declares, and the four
///   pairs are exactly the pairs it re-declares (no fifth pair slipping in);
/// * every re-declared row clears its ceiling by at least
///   [`RP24_OUT_OF_SCOPE_MIN_RATIO`], so the rule never turns on a rounding;
/// * the pairs are display-class by the in-scope re-derivation
///   (`effective_bin() == 6`) while their frozen label is bin 7 — which is the
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
        // …and the rows RP3.9 owns are not this rule's business either: the
        // mechanism arm of `declare` runs first, so a spelling inside the floor
        // that is no render never reaches the ceiling. Two of them (on
        // `capacitor.cuf` and `generator.kvar`) WOULD clear their ceiling, and
        // counting them here would both inflate the cited row counts and drop
        // the measured margin from 277x to 2.08x — the walk must measure what
        // the accounting actually re-declares.
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
    // above too, and pin the rounding margin literally. Both constants were
    // one-sided as landed (the audit round mutated 277.0 → 2.0 and
    // CEILING_ROUND_MARGIN 1.01 → 200.0 without a test noticing).
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
    // counted in scope (plan §1.3's own claim).
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(led.owner(Owner::OutOfScope), DECLARED_OUT_OF_SCOPE);

    // **The single-number guard, driven** (RP2.4 audit settlement). The proof
    // needs the row's gap to dominate the census's offender-only `max_rel`,
    // which holds for one number and need not for several — so a multi-number
    // row on a cited pair is refused outright, however far above the ceiling it
    // sits. All 105 real rows are single-number, so only a synthetic row can
    // reach this arm; without it the rule would silently widen the day a
    // vector-valued spelling appears on one of the four pairs.
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
/// measured one** — the RP2.4 audit settlement's both-ways guard for
/// [`RP39_ROUTING`].
///
/// The settlement's finding was that 55 vendored spellings the floor claimed are
/// no `%.Ng` render of our value, so they are neither a display artifact nor
/// (as the census then tagged them) "not a defect, no fix owner". Everything the
/// re-declaration rests on is asserted here rather than trusted:
///
/// * every row the walk refuses for the mechanism sits on a cited pair, and
///   every cited pair really carries the number of refused rows it claims — so
///   a pair whose round trip gets fixed (or whose spelling changes) reds here
///   instead of silently emptying the work list;
/// * each refused row is **inside** the floor (it is the mechanism clause that
///   refuses it, not the metric — otherwise this would be ordinary bin-7
///   material), and no earlier link claims it;
/// * each citation names an r4133 site, the same discipline
///   [`ECHO_CARVE_OUT_ROUTING`] and [`RP38_SUPERSEDED`] carry;
/// * each row records a disposition from [`RP39_DISPOSITIONS`], so a pair cannot
///   be settled by a tag whose obligations nothing checks;
/// * the bucket the accounting builds is [`DECLARED_RP39`] — unchanged by the
///   settlement, because it is measured from the walk;
/// * and what the settlement actually retired is [`OPEN_RP39`], summed here over
///   the rows still tagged `OPEN`.
#[test]
fn the_display_floors_round_trip_residue_is_owned_by_rp39() {
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
    let floor = props_norm::display_floor().expect("RP2.4 derived the floor");
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
            "{pair}: an RP3.9 routing must cite the r4133 site, got {cite:?}"
        );
        assert!(
            RP39_DISPOSITIONS.contains(disposition),
            "{pair}: {disposition:?} is not a recorded RP3.9 disposition — extend \
             RP39_DISPOSITIONS with the obligations that outcome owes, never loosen"
        );
        // The METRIC would have claimed it; only the mechanism clause does not.
        let rel = props_norm::display_rel(&row.rust, &row.r4133).expect("a numeric cell");
        assert!(rel <= floor, "{pair}: {rel:e} is outside the floor");
        assert!(!props_norm::under_display_floor(&row.rust, &row.r4133));
        assert_eq!(
            first_match(chain_verdicts(&corpus, row)),
            None,
            "{pair}: an RP3.9 row must be unclaimed by every link"
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
        "RP3.9 inherits (rows, pairs, rows on in-scope pairs)"
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
        "the residue still awaiting a verdict — RP3.9 settled all 27 pairs on 2026-09-02, so \
         this is (0, 0, 0) while DECLARED_RP39 keeps the measured rows"
    );
}

/// **RP3.12's rows are exactly the ones the walk finds, and they are NOT
/// display-class** — the ownership guard [`RP312_UPSTREAM_BUG`] owes, shaped like
/// [`the_display_floors_round_trip_residue_is_owned_by_rp39`].
///
/// It asserts four things the doc alone would only claim: the table's counted
/// columns sum to [`DECLARED_RP312`]; the rows [`regcontrol_autotrans_typecast_row`]
/// selects are exactly the table's pairs and counts; every one of them is far
/// **outside** the display floor (so RP2.4's mis-filing cannot come back — this is
/// a solved-state jump, not a render) and unclaimed by every link of the chain;
/// and the verdict is one an [`RP312_VERDICTS`] tag covers, cited to a
/// `Version8/Source` `.pas:` line.
///
/// The measured minimum gap is asserted too. RP2.4's floor is 2e-4 and the
/// smallest of these eight rows is 3.1e-2 — 150x out — so a future re-measure that
/// drifted anywhere near the floor would red here rather than quietly change
/// which sub-step owns the row.
#[test]
fn the_regcontrol_autotrans_typecast_rows_are_owned_by_rp312() {
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
    let floor = props_norm::display_floor().expect("RP2.4 derived the floor");
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
            "{pair}: an RP3.12 routing must cite the r4133 site, got {cite:?}"
        );
        assert!(
            RP312_VERDICTS.contains(verdict),
            "{pair}: {verdict:?} is not a recorded RP3.12 verdict — extend RP312_VERDICTS \
             with the obligations that outcome owes, never loosen"
        );
        assert!(!pin.is_empty(), "{pair}: the verdict owes a named witness");
        // NOT display-class, and not by a hair: this is the reading RP2.4's
        // accounting got wrong about the class while getting the scope right.
        let rel = props_norm::display_rel(&row.rust, &row.r4133).expect("a numeric cell");
        assert!(
            rel > floor,
            "{pair}: {rel:e} is inside the display floor {floor:e} — that row is RP3.9's \
             round-trip residue, not this upstream bug"
        );
        min_rel = min_rel.min(rel);
        assert!(!props_norm::under_display_floor(&row.rust, &row.r4133));
        assert_eq!(
            first_match(chain_verdicts(&corpus, row)),
            None,
            "{pair}: an RP3.12 row must be unclaimed by every link"
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
    assert!(
        min_rel > 100.0 * floor,
        "the smallest RP3.12 gap is {min_rel:e}, within 100x of the display floor {floor:e} — \
         re-read which sub-step owns these rows instead of leaving the split to a rounding"
    );
    let led = account(&corpus, PROPS_NORM_R4133);
    assert_eq!(
        led.owner(Owner::Rp312),
        DECLARED_RP312,
        "RP3.12 inherits (rows, pairs, rows on in-scope pairs)"
    );
    // The pair's ninth spelling stays where RP3.9 settled it: one row, inside
    // the floor, still declared to Owner::Rp39.
    assert_eq!(
        RP39_ROUTING
            .iter()
            .find(|(p, _, _, _, _)| *p == "autotrans.wdgcurrents")
            .map(|(_, n, s, _, d)| (*n, *s, *d)),
        Some((1, 0, "PIN")),
        "RP3.9 keeps the makeposseq round-trip residue of the same pair"
    );
}

/// **RP3.12's routing is pinned literally**, like [`NOT_A_PIN`],
/// [`LEDGER_ENTRY_PINS`] and [`RP39_PINS`]: it is the fifth exemption from
/// "every pin in [`PINS`] is named by an echo row", and an exemption that grows
/// by iteration would let an un-cited `#[test]` through.
///
/// The literals are the sub-step's whole verdict — which pair, how many rows,
/// how many of them the RP4.1 unmask compares, the r4133 site, the outcome and
/// the witness — so a silent edit to any of the six columns fails here first.
#[test]
fn the_rp312_upstream_bug_list_is_pinned() {
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
        "RP3.12 records exactly the outcome it measured"
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
