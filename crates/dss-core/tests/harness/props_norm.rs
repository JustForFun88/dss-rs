//! **The r4133 property-value normalization engine** (`R4133_PROPS_PLAN.md`
//! §1.2, sub-step RP2.1) — the second link of the r4133 props claim chain
//! *shape allowlist → **normalization** → echo table → display floor*.
//!
//! # What it is
//!
//! [`PROPS_NORM_R4133`] is a table of `(class, prop)` rows, each carrying one
//! **typed** rule ([`NormRule`]). When the property comparator runs on the
//! [`R4133`](super::PropsChannel::R4133) channel it consults the table for the
//! cell it is about to assert on; a rule that recognises the two spellings as
//! the SAME value re-spells the oracle side as ours, and the assert then sees
//! two identical strings. On the `capi_v0145` channel the table is never
//! consulted at all (plan mechanic (b), capi-invariance).
//!
//! Since **RP2.3** the module owns the chain's third link too:
//! [`PROPS_ECHO_R4133`], the *echo-exclusion* table. Where a rule re-spells,
//! an echo row **drops the value compare** of its `(class, prop)` — the name
//! and index order still assert — because the two sides do not spell one value
//! and no value-preserving rule could claim them. It is consulted after the
//! normalization seam, on the r4133 channel only, and every row carries the
//! r4133 site that proves its category plus the witness that still holds the
//! port's value (capi coverage or a named expected-value pin — and a capi
//! witness alone is not enough for a row whose cells reach `engines: "r4133"`
//! cases, [`ECHO_ROWS_ON_R4133_ONLY_CASES`]). A row is pair-scoped minus its
//! [`ECHO_CARVE_OUTS`]: the one measured cell whose divergence its citation does
//! not explain stays comparable.
//!
//! Since **RP2.4** it owns the chain's fourth and last link as well:
//! [`R4133_DISPLAY_FLOOR`], the *display floor*. Where a rule re-spells and an
//! echo row excludes, the floor **claims a numeric cell whose two sides agree
//! to within `2e-4` relative AND whose r4133 side is our value rounded to the
//! digits r4133 printed** ([`display_is_render`]) — a Delphi
//! `Format('%[-].Ng', …)` render of the shared double, or of the one command
//! string an upstream round trip wrote. It is consulted last, on the r4133
//! channel only, and its derivation (measured worst 6.431124e-05, the empty band
//! up to 1.374769e-03, the `%[-].Ng` site table, and the 55 spellings the
//! mechanism clause refuses) lives on the constant.
//!
//! That statement has to hold at **three** seams, because the tables have two
//! callers each and the floor a third, and each carries its own channel gate:
//!
//! * the live comparator — [`normalize_r4133`], [`echo_excluded_r4133`] and
//!   [`under_display_floor_r4133`], reached only from
//!   `PropsPolicy::normalize`/`::echo_excluded`/`::under_display_floor`'s
//!   r4133 arms (`PropsPolicy::is_r4133`, pinned by
//!   `props_policy_tests::the_capi_channel_never_normalizes`,
//!   `::the_capi_channel_never_excludes` and
//!   `::the_capi_channel_never_applies_the_display_floor`);
//! * the offline/measurement query — [`claim_value`], which the claims census
//!   asks about the rows of **both** channels and which therefore takes the
//!   channel itself and answers `None` on capi (pinned by
//!   [`tests::the_capi_channel_claims_nothing`]). It was channel-blind as first
//!   landed; the RP2.1 audit round fixed it.
//!
//! # The contract: value-preserving normalization ONLY (plan mechanic (c))
//!
//! A rule may change how a value is **spelled**; it may never change **which**
//! value it is — the `lane::expected_rerounded` discipline
//! (`harness/lane.rs:546-592`) transplanted to property cells. Everything that
//! cannot satisfy that is an **exclusion** — [`PROPS_ECHO_R4133`] — never a
//! rule here.
//!
//! Structurally the contract reduces to the per-kind predicate
//! [`NormRule::claims`]: the engine re-spells **only** when the predicate says
//! the two sides are the same value, and hands back the raw pair otherwise, so
//! a divergence that is real still reaches the assert with both original
//! spellings in the message. Three consequences the tests below pin:
//!
//! * the rules are **typed, never regexes** — each kind names one upstream
//!   rendering mechanism, cited on the kind;
//! * `''` is not a boolean and not an array — an empty render is the
//!   `PropertyValue[]` echo of bin 5, which this table must not claim
//!   (the chain hands those cells to [`PROPS_ECHO_R4133`]);
//! * a token-count difference is a value difference: `[400]` and
//!   `(400, 400, 400)` are a one- and a three-element array and never fold.
//!
//! # Evidence (plan mechanic (a))
//!
//! Every row cites a row of the vendored census at `tests/corpus/props_r4133/`
//! by the triple *(pair, bin, cells)*: [`NormRow::bin`] and [`NormRow::cells`]
//! reproduce that pair's `bins.tsv` columns, and [`NormRow::src`] says which
//! vendored file the line lives in — `bins.tsv` for the 2026-08-08 census, or
//! the `README.md` §"Pairs the WP-RP1 shape closures make live" record of the
//! sub-step that created the pair (those pairs cannot exist in the frozen
//! extracts: while a class carried a `shape_count` row the walk never reached
//! the props behind it). RP2.1 part C's replay re-reads the vendored files and
//! cross-checks the triple, so a mis-transcribed row fails a test rather than
//! silently widening the table.
//!
//! # Population (RP2.1, bins 1/2/4 of plan §1.1; RP2.2 bin 3; RP2.3 off-bin)
//!
//! | kind | rows | derivation |
//! |---|---|---|
//! | [`BoolFold`](NormRule::BoolFold) | 77 | bin 1's 75 pairs **minus the five pure-echo pairs** (`capcontrol.reset`, `recloser.debugtrace`, `regcontrol.idleforward`, `regcontrol.idlereverse`, `upfccontrol.enabled` — no foldable cell at all, vendored `README.md` §"Bin 1 carries nine echo pairs, not three"), plus 7 WP-RP1 pairs |
//! | [`CaseFold`](NormRule::CaseFold) | 65 | bin 2 whole (59 case-only + 2 trailing-space), plus 2 WP-RP1 pairs, **minus** `invcontrol.voltage_curvex_ref` (re-typed by RP2.2, next row), **plus 3 RP2.3 off-bin rows** |
//! | [`ArrayForm`](NormRule::ArrayForm) | 23 | bin 4's 21 pairs minus 4 no typed rule may claim (below), **plus 6 RP2.3 off-bin rows** |
//! | [`EnumSynonym`](NormRule::EnumSynonym) | 5 | RP2.2: the four source sequence-selector pairs of bin 3 ([`SCAN_TYPE_SYNONYMS`] / [`SEQUENCE_TYPE_SYNONYMS`]) plus [`VOLTAGE_CURVEX_REF_SYNONYMS`]; the other four bin-3 pairs are NOT synonyms — see below |
//!
//! # RP2.3's nine off-bin rows (bin 5 pairs carrying comparable cells)
//!
//! RP2.3 fills [`PROPS_ECHO_R4133`], and an exclusion is pair-scoped. The census
//! measured 6 446 cells (6 370 in scope) on ten of the 86 bucket pairs that a
//! typed rule can fold value-preservingly, so nine of them take a row here
//! FIRST, and `props_r4133_replay::MULTI_LINK_ROWS` counts the overlap (135
//! example rows over 20 pairs).
//!
//! **What that buys, precisely** (RP2.3 audit settlement, 2026-08-23 — the
//! earlier wording here claimed more): the chain order decides which link
//! *claims* the cell, so those 6 446 cells are dispositioned
//! `normalized-by-<rule>` rather than `echo-row`, and each of the nine rows can
//! prove itself live through its own hit counter once RP4.1 unmasks the path.
//! It does **not** keep them inside the live value compare: at the seam
//! ([`echo_excluded_r4133`]) the pair-scoped exclusion still drops the value
//! assert for every cell of the pair, the folded ones (harmless — the two sides
//! are equal by then) and the ones the rule refused alike. Narrowing the twenty
//! mixed pairs per cell — the [`ECHO_CARVE_OUTS`] mechanism, or a per-row
//! spelling allowlist — is an explicit RP4.1 precondition (plan §RP4.1), and
//! until it lands a genuine regression on one of those pairs (a wrong resolved
//! loadshape name, a wrong ZIPV vector) is caught on the capi channel only.
//!
//! * `load.yearly` `CaseFold` (5 995 cells) — arm 7 answers the LIVE
//!   `Yearlyshape` string (`PCElements/Load.pas:2346`), so a case-only
//!   difference names the same shape (`THashList` lowercases both sides); the
//!   `''`-vs-value half stays the echo row's, which is why RP2.2's routing note
//!   says one `CaseFold` row is not *sufficient* for the pair;
//! * `reactor.bus2` `CaseFold` (37) — the stale `GetBus(2)` snapshot happens to
//!   agree with our live terminal on every cell but the four the echo row keeps
//!   (`'b2.0'` vs `'b2.0.0.0'`, the shape a later `phases=` creates);
//! * `invcontrol.monvoltagecalc` `CaseFold` (15) — the deck's own `'MAX'`/
//!   `'AVG'` token against our registry spelling;
//! * `line.wires` (2), `load.zipv` (342), `generator.userdata` (3),
//!   `storage.dynadata` (2), `storagecontroller.seasontargets` (25) and
//!   `seasontargetslow` (25) `ArrayForm` — same tokens, different delimiters
//!   (r4133's paren wrapper, its bare space-separated list, its trailing-comma
//!   bracket form).
//!
//! The tenth, `swtcontrol.action`, takes **no** row: its only foldable spelling
//! is `'close'`/`'Close'`, 6 cells and **0 in scope**, so the row would buy no
//! live compare at all while contradicting RP2.2's recorded routing of the pair
//! (`the_pairs_routed_elsewhere_have_no_row`). RP2.3 recorded the measurement
//! instead of landing a dead row.
//!
//! All nine sit on pairs whose `bins.tsv` LABEL is 5 — the vendored
//! `README.md` §"A pair's bin is a label, not a per-cell classification" is the
//! doctrine, and [`tests::every_row_cites_a_bin_its_rule_owns`] keeps every
//! such row in one enumerated, counted exception list.
//!
//! # RP2.2's routing of bin 3 (why only four of the eight pairs take a row)
//!
//! RP2.2 read all eight bin-3 getters. Exactly four print a **spelling of the
//! same live value** and take an [`EnumSynonym`](NormRule::EnumSynonym) row here
//! — `vsource.scantype` / `vsource.sequence` / `isource.scantype` /
//! `isource.sequence`. The other four are not synonyms and were routed instead
//! (the citations live in `props_r4133_replay::RP22_ROUTING`, which is the
//! record of the whole dossier):
//!
//! * `swtcontrol.action` — an `EchoParse` of the deck token, stored
//!   *unconditionally* before the CASE (`Version8/Source/Controls/
//!   SwtControl.pas:192-193`) and left stale when `Locked` refuses the write
//!   (`:417`), so `'close'` vs `'open'` is a stale STRING, not a state
//!   disagreement → RP2.3;
//! * `monitor.mode` — the deck's RPN source text `mode=(1 16 +)` echoed back
//!   with the parser's parens stripped (`Meters/Monitor.pas:359`, no
//!   `GetPropertyValue` override) → RP2.3;
//! * `storagecontroller.modedischarge` — both engines hold `MODESCHEDULE`;
//!   r4133's `GetModeString` has no arm for it and falls to
//!   `'UNKNOWN'` (`Controls/StorageController.pas:1200-1214`). `'UNKNOWN'` is
//!   the catch-all for *every* unnamed mode, so mapping it to `'Schedule'`
//!   would not be injective — an exclusion plus a pin, never a row here → RP2.3;
//! * `line.units` — a **live-state** divergence (r4133 renders
//!   `LineUnitsStr(LengthUnits)`, `PDElements/Line.pas:1404`), root-caused to
//!   the port's matrix-branch merge order → RP3.5.
//!
//! Closing bin 3 also means closing its **cells**, not only its pairs: three
//! bin-2-labelled pairs carry enum-spelling cells (vendored `README.md` §"A
//! pair's bin is a label, not a per-cell classification"). RP2.2 read those
//! getters too — `capcontrol.type` (`'pf'`/`'volt'`, 30 cells) and `fault.bus2`
//! (`'b2.0'` vs `'b2.0.0.0'`, 1 cell) are `PropertyValue[]` echoes and go to
//! RP2.3 beside their untouched `CaseFold` rows (the mixed-pair pattern the
//! chain order exists for), while `invcontrol.voltage_curvex_ref` is a live
//! getter and became this table's fifth `EnumSynonym` row.
//!
//! The four bin-4 pairs that take **no** row, each with the sub-step that owns
//! it — all four are named by the plan itself, so the RP2.1 kill criterion
//! ("a bins-1/2/4 row no typed rule can claim") does **not** fire:
//!
//! * `expcontrol.derlist` (`[PVSystem.pv]` vs `[pv]` — element vs bare name),
//!   `relay.normal`, `relay.state` (`[closed, closed, closed, ]` vs
//!   `[closed, ]` — the per-phase array render): all three were on RP2.2's
//!   enumerated S6 singleton list, and RP2.2 routed them — `derlist` to RP2.3
//!   (r4133 answers the bare PVSystem list for BOTH properties,
//!   `Version8/Source/Controls/ExpControl.pas:696` + `:702-715`), the two Relay
//!   rows to **RP3.7** (a live per-phase-state divergence, not a spelling);
//! * `sensor.kvs` (`[ 0 0 0]` vs `[7.2, 7.2, 7.2]`): a **real** value delta,
//!   and out of scope — all 61 cells sit on `engines: "capi_v0145"` cases
//!   (plan §RP2.1's own note and §1.3).
//!
//! Two more populations stay unclaimed **inside** rows this table does hold,
//! by design: the echo cells of the four mixed bin-1 pairs
//! (`recloser.eventlog`, `regcontrol.idle`, `relay.distreverse`,
//! `relay.reset`) and the off-bin cells of the heterogeneous pairs the vendored
//! `README.md` §"A pair's bin is a label" enumerates. Claiming is **per cell**,
//! first match in the chain, so a row here and an RP2.3 echo row can coexist on
//! one pair — the rule fires on the foldable cells and the echo row masks the
//! rest.
//!
//! # Four `ArrayForm` rows sit on RP2.2's S6 list (disclosure, settled)
//!
//! The exclusion list above says which S6 pairs take no row; the converse is
//! worth stating too, because plan §RP2.2 hands RP2.2 a *dossier* obligation
//! (read the r4133 getter) that a claimed cell does not discharge. RP2.2 read
//! all four getters and the outcome is recorded here:
//!
//! * `invcontrol.monbus`, `invcontrol.monbusesvbase` — **every** census
//!   spelling folds (bracketed vs bare, token for token, equal counts), so
//!   RP2.2 found nothing left to route. The mechanism is an echo either way:
//!   `TInvControlObj.GetPropertyValue` has no arm for 26/27
//!   (`Version8/Source/Controls/InvControl.pas:3226-3285`) and
//!   `InitPropertyValues` never writes 25..27 (`:2806-2839`), so r4133 answers
//!   the deck's own bus list while the port renders the live one — same list,
//!   two delimiter styles;
//! * `swtcontrol.normal`, `swtcontrol.state` — only the single **one-token**
//!   spelling folds (`'closed'` vs `'[closed, ]'`, 1 cell of 59 on each pair).
//!   Their main spellings — `'closed'`/`'open'` against r4133's three-element
//!   per-phase render, 58 + 31 + 27 cells — are refused by the token-count rule,
//!   and RP2.2 routed them to **RP3.7**: r4133 keeps a per-phase `pStateArray`
//!   and renders one token per controlled-element phase (`Controls/
//!   SwtControl.pas:37-38`, `:299-305`, `:589-610`) where the port keeps one
//!   scalar applied to the whole terminal. Same conclusion as their twins
//!   `relay.normal`/`state` (whose ONLY spelling is that shape, which is why
//!   those two take no row at all). What this table claims is two cells whose
//!   two sides carry the same single token — that stays correct whatever RP3.7
//!   decides.
//!
//! Pinned by `arrayform_folds_delimiters_not_contents` (both directions on both
//! `swtcontrol` pairs) and by `props_r4133_replay`'s `RP22_S6`/`RP22_ROUTING`
//! accounting.

use std::borrow::Cow;
use std::cell::Cell;
use std::cmp::Ordering;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrd};

use super::PropsChannel;
use EchoCategory::{EchoDefault, EchoParse, EmptyCollectionRender, LiveSemanticsDiffer};
use EchoWitness::{Capi, CapiAndPin, Pin};
use NormRule::{ArrayForm, BoolFold, CaseFold, EnumSynonym};

/// One typed normalization rule. **Typed, not a regex** — each variant names a
/// single upstream rendering mechanism and is proven value-preserving by its
/// own unit tests below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormRule {
    /// **Boolean rendering** (plan §1.1 bin 1). The port renders FPC
    /// `Yes`/`No` (`.inputs/dss_capi/src/Common/Utilities.pas:173-177`,
    /// `StrYOrN`); r4133 answers with one of eleven Delphi spellings — for the
    /// `enabled` property literally
    /// `If Enabled Then Result := 'true' Else Result := 'false'`
    /// (`Version8/Source/Common/CktElement.pas:1313-1320`), for a class getter
    /// such as AutoTrans `XRConst` `'YES'`/`'NO'`
    /// (`Version8/Source/PDElements/AutoTrans.pas:1861`).
    ///
    /// Folds `{yes, y, true}` to one boolean and `{no, n, false}` to the other,
    /// case-insensitively and after trimming outer whitespace. **`''` is NOT a
    /// boolean**: an empty render is the `PropertyValue[]` echo of a class
    /// whose `GetPropertyValue` has no arm for the index
    /// (`Version8/Source/General/DSSObject.pas:112-115`), which bin 5 and
    /// [`PROPS_ECHO_R4133`] own — folding it would claim a cell whose value we
    /// cannot even read.
    BoolFold,
    /// **Case-only spelling, plus the two literal trailing blanks** (plan §1.1
    /// bin 2). Compares after `trim()` and ASCII-case-insensitively.
    ///
    /// Value-preserving because DSS identifiers are case-insensitive **in both
    /// engines by construction**: `THashList` lowercases every key on `Add`
    /// *and* on `Find` — r4133 `Version8/Source/Shared/HashList.pas:263-289`
    /// (`SS := LowerCase(S)`, and the lowercased copy is what is stored) and
    /// dss_capi `src/Shared/HashList.pas:219-252` — so a case-only difference
    /// in a name-valued property provably names the same object, and in an
    /// enum-valued property is one spelling of one ordinal.
    ///
    /// The trim half exists for exactly **two** rows, both an upstream literal
    /// with a trailing blank in the `Conn` getter:
    /// `0: Result := 'wye '; 1: Result := 'Delta ';` at
    /// `Version8/Source/PDElements/Transformer.pas:1762-1763` and
    /// `Version8/Source/PDElements/AutoTrans.pas:1818-1819`.
    CaseFold,
    /// **Array form** (plan §1.1 bin 4). The port renders the dss_capi
    /// `GetDSSArray` form `'[' + ' %g'×n + ']'`
    /// (`.inputs/dss_capi/src/Common/Utilities.pas:1529-1552`); r4133 renders
    /// per class in comma, paren or bare forms — e.g. EnergyMeter wraps its
    /// answer in its own parens, `4,7: Result := '(' … Result + ')'`
    /// (`Version8/Source/Meters/EnergyMeter.pas:2637-2664`).
    ///
    /// Tokenizes both sides — `[`, `]`, `(`, `)`, `,` and whitespace are all
    /// separators — and compares token for token: numeric tokens by **value**
    /// (`f64`, through [`numbers_match`], which is where RP2.4's display floor
    /// hangs — since RP2.4 that compare is *within* [`R4133_DISPLAY_FLOOR`] and
    /// a render of our token at the oracle's printed precision, not exact),
    /// other tokens ASCII-case-insensitively. The token **count**
    /// must match, so a one-element array never folds into a three-element one.
    ///
    /// Because *every* delimiter is a separator, a **bare** render folds against
    /// a delimited one at any length — that is the census's own bin-4 shape, not
    /// an accident: `invcontrol.monbus` `'[A.1, A.2, A.3]'` vs `'A.1 A.2 A.3'`
    /// (three tokens) and `'[r.1.2.3]'` vs `'r.1.2.3'` (one), and the mirror
    /// case `swtcontrol.normal` `'closed'` vs `'[closed, ]'`, where the **bare**
    /// side is ours. A single-token side is therefore a one-element array
    /// whichever way round the brackets sit; what it still never folds into is a
    /// three-element one (`'closed'` vs `'[closed, closed, closed, ]'`, 58 cells
    /// of that same pair, stays unclaimed — see the module doc's S6 note).
    ///
    /// A side that yields no token is refused (`''`, `'[]'`, `'()'`): an empty
    /// render carries no value to preserve and is bin 5's echo territory —
    /// `GetDSSArray` itself returns `''` for a NIL pointer
    /// (`src/Common/Utilities.pas:1533-1537`), which is exactly the
    /// `''`-vs-`[]` shape the vendored README records for
    /// `generator.dynout`/`autotrans.bhcurrent`/`windgen.dynout`.
    ArrayForm,
    /// **Enum spelling** (plan §1.1 bin 3) — an explicit, closed per-row map of
    /// `(our spelling, r4133 spelling)` pairs. Each mapping must cite the
    /// r4133 source line that prints its right-hand side; the map never
    /// contains a wildcard and never derives a synonym, so it can only claim
    /// the exact pairs a human read off the Pascal.
    ///
    /// Shipped with **zero** rows by RP2.1; **RP2.2 landed five** — the four
    /// source sequence-selector properties ([`SCAN_TYPE_SYNONYMS`],
    /// [`SEQUENCE_TYPE_SYNONYMS`]) plus the one pair whose bin-2 *label* hides
    /// genuine enum-spelling cells, `invcontrol.voltage_curvex_ref`
    /// ([`VOLTAGE_CURVEX_REF_SYNONYMS`], re-typed from `CaseFold`). The kind's
    /// behavior is pinned both against the shipped maps and against a synthetic
    /// one, the way `EVENTLOG_MASKS`' r4133 row set is pinned against an
    /// injected table (`harness/mod.rs`, `eventlog_mask_tests`).
    ///
    /// The map is **directional and per-pair**: `(ours, theirs)`, matched after
    /// `trim()` and ASCII-case-insensitively, with no reverse implication. A map
    /// must also be **injective in BOTH directions**, because either collision
    /// equates two different values:
    ///
    /// * one r4133 token naming two of our spellings — our two values would
    ///   both fold onto one upstream render;
    /// * one of our spellings naming two r4133 tokens — our single value would
    ///   fold against two upstream renders, so if those denote different
    ///   upstream values one of the folds masks a real disagreement.
    ///
    /// [`tests::enumsynonym_maps_are_injective`] enforces both directions on
    /// every shipped map, and pins each map's contents literally so a widening
    /// entry cannot ride in on a row that folds nothing in today's population.
    EnumSynonym(&'static [(&'static str, &'static str)]),
}

/// Which vendored file carries the census line a [`NormRow`] cites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// `tests/corpus/props_r4133/bins.tsv` — the frozen 2026-08-08 census.
    /// [`NormRow::bin`] and [`NormRow::cells`] are that row's `bin` and `cells`
    /// columns.
    BinsTsv,
    /// `tests/corpus/props_r4133/README.md` §"Pairs the WP-RP1 shape closures
    /// make live", **RP1.1** (Generator `Rneut`/`Xneut`, Sensor `Action`;
    /// measured 2026-08-22, 12 new pairs).
    Rp11,
    /// The same section, **RP1.2** (AutoTrans `XfmrCode`; 9 new pairs).
    Rp12,
    /// The same section, **RP1.3** (WindGen `UserModel`/`UserData`; 2 new
    /// pairs).
    Rp13,
    /// The same section, **RP1.4** (GenDispatcher `weights`; 1 new pair).
    Rp14,
}

/// One row of [`PROPS_NORM_R4133`]: a census pair, the typed rule that claims
/// its foldable cells, and the citation triple.
#[derive(Debug)]
pub struct NormRow {
    /// Class, spelled as the census spells a pair's prefix (lowercase). Matched
    /// ASCII-case-insensitively, so the capture's `Capacitor` finds it.
    pub class: &'static str,
    /// Property, spelled as the census spells it (the oracle's name,
    /// lowercased). Matched ASCII-case-insensitively.
    pub prop: &'static str,
    /// The rule.
    pub rule: NormRule,
    /// The pair's census bin (plan §1.1) — 1, 2 or 4 in RP2.1.
    pub bin: u8,
    /// The pair's census cell count, full census.
    pub cells: u32,
    /// Which vendored file the cited line lives in.
    pub src: Evidence,
}

// ---------------------------------------------------------------------------
// RP2.2's `EnumSynonym` maps. Two of them, NOT one: the two properties are
// spelled by two DIFFERENT registries.
// ---------------------------------------------------------------------------

/// **`ScanType=` — the closed synonym set** for `vsource.scantype` and
/// `isource.scantype` (plan §1.1 bin 3; `bins.tsv` 2 042 and 137 cells).
///
/// **Why the two sides spell one value.** r4133 has no `GetPropertyValue` arm
/// for either index — `TVsourceObj.GetPropertyValue` covers 1, 4, 7, 8, 11..16,
/// 19..26, 31 (`Version8/Source/PCElements/Vsource.pas:1323-1349`) and
/// `Isource.pas` has no override at all — so both echo `PropertyValue[]`
/// (`General/DSSObject.pas:112-115`). That store is written from exactly two
/// places, and each keeps it in step with the live enum:
///
/// * `InitPropertyValues` freezes `'Pos'` (`Vsource.pas:1300`) / `'pos'`
///   (`Isource.pas:626`) next to a `Create` that sets `ScanType := 1`
///   (`Vsource.pas:615`, `Isource.pas:396` — "// Pos Sequence");
/// * the parser writes `PropertyValue[ParamPointer] := Param`
///   (`Vsource.pas:355`, `Isource.pas:239`) in the **same** loop iteration that
///   feeds that same `Param` to the enum arm — `Case Uppercase(Param)[1] of
///   'P': ScanType := 1; 'Z': 0; 'N': -1` (`Vsource.pas:378-384`,
///   `Isource.pas:257-263`).
///
/// `MakeLike` cannot desync them either: both classes copy `Scantype` and
/// `Sequencetype` explicitly (`Vsource.pas:527-528`, `Isource.pas:324-325`)
/// alongside the whole `FPropertyValue[]` array (`Vsource.pas:566`,
/// `Isource.pas:339`).
///
/// Our side renders the `'Scan Type'` registry name for the ordinal —
/// `['None', 'Zero', 'Positive']` / `[-1, 0, 1]`
/// (`crates/dss-core/src/obj/dss_enum/registry/solution.rs:22-29`,
/// `elements/pc/source_seq.rs::ScanType`).
///
/// **The set is closed at the spellings the census actually measured**, i.e.
/// the frozen `'Pos'`/`'pos'` default and `isource.scantype`'s one `'zero'`
/// cell (`examples_full.txt`). Every other token fails and reaches the assert
/// raw — fail-closed on purpose: a spelling this table has never seen is a
/// re-measure, not a silent fold.
///
/// **The trap this asymmetry protects against.** `ScanType`'s `-1` is named
/// `'None'`, while `Sequence`'s `-1` is `'Negative'` — the two registries are
/// *not* the same list. A shared map would have paired our `'Negative'` with an
/// r4133 `'neg'` on `scantype`, where our own render for that ordinal is
/// `'None'`; the row would then have folded nothing and gone stale silently.
/// Hence one map per property, and no `-1` entry on this one (no census cell).
const SCAN_TYPE_SYNONYMS: &[(&str, &str)] = &[("Positive", "Pos"), ("Zero", "Zero")];

/// **`Sequence=` — the closed synonym set** for `vsource.sequence` and
/// `isource.sequence` (bin 3; 2 042 and 137 cells).
///
/// Same mechanism, same two writers, one index over: `PropertyValue[18]`/`[7]`
/// frozen `'Pos'`/`'pos'` (`Vsource.pas:1301`, `Isource.pas:627`) against
/// `Sequencetype := 1` in `Create` (`Vsource.pas:616`, `Isource.pas:397`), and
/// the parser's shared `Param` feeding `Case Uppercase(Param)[1] of 'P'/'Z'/'N'`
/// (`Vsource.pas:385-391`, `Isource.pas:264-270`).
///
/// Our side renders the `'Sequence Type'` registry name —
/// `['Negative', 'Zero', 'Positive']` / `[-1, 0, 1]`
/// (`registry/solution.rs:31-38`, `source_seq.rs::SequenceType`) — so `-1` is
/// `'Negative'` here and `'None'` on [`SCAN_TYPE_SYNONYMS`]. The measured
/// spellings are the frozen `'Pos'`/`'pos'` and `isource.sequence`'s 19 `'neg'`
/// cells; `'Zero'` has no cell on either sequence pair and takes no entry.
const SEQUENCE_TYPE_SYNONYMS: &[(&str, &str)] = &[("Positive", "Pos"), ("Negative", "Neg")];

/// **`invcontrol.voltage_curvex_ref` — the one `EnumSynonym` row on a pair whose
/// `bins.tsv` LABEL is not 3** (the exception [`tests::
/// every_row_cites_a_bin_its_rule_owns`] names and counts).
///
/// The pair is bin **2** because its first census row is case-only, but 3 of its
/// 257 cells (all 3 in scope) are a genuine enum-spelling difference — the
/// vendored `README.md` §"A pair's bin is a label, not a per-cell
/// classification" lists it among the "case-or-empty pairs carrying
/// enum-spelling cells", and its own §1 spells out that admissibility is per
/// cell.
///
/// RP2.1 gave the pair a [`CaseFold`](NormRule::CaseFold) row, which claims
/// `'Rated'`/`'rated'` (241 cells) and `'Avg'`/`'avg'` (13) and refuses
/// `'RAvg'`/`'avgrated'` (3). RP2.2 replaced it with this map rather than hand
/// those 3 cells to RP2.3, because **there is nothing to exclude**: r4133
/// renders index 6 from the LIVE field, not from a `PropertyValue[]` store —
///
/// ```text
///  6 : begin
///        if(FVoltage_CurveX_ref = 0) then Result := 'rated'
///        else if (FVoltage_CurveX_ref = 1) then Result := 'avg'
///        else if (FVoltage_CurveX_ref = 2) then Result := 'avgrated'
///      end;
/// ```
///
/// (`Version8/Source/Controls/InvControl.pas:3244-3249`) — against our own
/// registry names for the same three ordinals, `['Rated', 'Avg', 'RAvg']` /
/// `[0, 1, 2]` (`crates/dss-core/src/obj/dss_enum/registry/control.rs:234`).
/// The two engines therefore hold the same `FVoltage_CurveX_ref` and spell it
/// differently on ordinal 2 only; an `EchoDefault`/`EchoParse` row would be
/// false, and `LiveSemanticsDiffer` would be false too (the semantics are
/// identical). The upstream asymmetry is r4133's own: it *parses* `'ravg'`
/// (`:837`) and *prints* `'avgrated'`.
///
/// **How the map compares with the `CaseFold` row it replaces: neither side is
/// nested in the other** (RP2.2 audit settlement, 2026-08-23 — the first
/// wording here claimed it was "stricter … so no cell that used to be compared
/// is now folded away", which its own paragraph above contradicts).
///
/// * **Narrower** on everything outside the three named ordinals: a case-only
///   difference the `CaseFold` row would have folded — say a future `'Vref'` vs
///   `'vref'` — now reaches the assert raw (that exact refusal is pinned in
///   [`tests::enumsynonym_folds_the_shipped_scan_and_sequence_spellings`],
///   beside two cross-ordinal ones).
/// * **Wider** by exactly the 3 `'RAvg'`/`'avgrated'` cells (all 3 in scope),
///   which the `CaseFold` row refused and which this map deliberately claims.
///   RP2.1 pinned that refusal inside
///   `casefold_folds_case_and_the_two_trailing_blanks`; RP2.2 removed the
///   assertion in the same commit that landed the fold, so the diff is the
///   record.
///
/// The 3 newly-folded cells are the whole point of the re-typing, and they are
/// value-preserving by the argument above (one live field, one ordinal, two
/// spellings) — not a coverage loss. What would be a loss is folding them
/// *silently*, which is why the count is named here and in the vendored
/// `README.md` §"What RP2.2 moved".
const VOLTAGE_CURVEX_REF_SYNONYMS: &[(&str, &str)] =
    &[("Rated", "rated"), ("Avg", "avg"), ("RAvg", "avgrated")];

/// Table constructor, so the 161 rows below read as data.
const fn row(
    class: &'static str,
    prop: &'static str,
    rule: NormRule,
    bin: u8,
    cells: u32,
    src: Evidence,
) -> NormRow {
    NormRow {
        class,
        prop,
        rule,
        bin,
        cells,
        src,
    }
}

/// **The table.** Sorted by `(class, prop)` — [`find_row`] binary-searches it,
/// and `table_is_sorted_and_unique` pins the order and the absence of a
/// duplicate key.
///
/// Read the module doc for the derivation; each row's `(class.prop, bin, cells,
/// src)` is its citation into `tests/corpus/props_r4133/`.
// The 161 rows are DATA — one census pair per line, columns aligned so the
// table diffs against `tests/corpus/props_r4133/bins.tsv` by eye. rustfmt's
// 60-char call width would explode 50 of them into eight lines each, which is
// why this item opts out. It and [`PROPS_ECHO_R4133`] are the only two
// `rustfmt::skip`s in the tree, for that one reason.
#[rustfmt::skip]
pub const PROPS_NORM_R4133: &[NormRow] = &[
    row("autotrans",         "conn",               CaseFold,  2, 42, Evidence::BinsTsv),
    row("autotrans",         "conns",              CaseFold,  2, 42, Evidence::BinsTsv),
    row("autotrans",         "enabled",            BoolFold,  1, 44, Evidence::Rp12),
    row("autotrans",         "sub",                BoolFold,  1, 42, Evidence::BinsTsv),
    row("autotrans",         "xrconst",            BoolFold,  1, 44, Evidence::Rp12),
    row("autotrans",         "xscarray",           ArrayForm, 4, 42, Evidence::BinsTsv),
    row("capacitor",         "enabled",            BoolFold,  1, 1059, Evidence::BinsTsv),
    row("capcontrol",        "capacitor",          CaseFold,  2, 129, Evidence::BinsTsv),
    row("capcontrol",        "element",            CaseFold,  2, 419, Evidence::BinsTsv),
    row("capcontrol",        "enabled",            BoolFold,  1, 446, Evidence::BinsTsv),
    row("capcontrol",        "eventlog",           BoolFold,  1, 446, Evidence::BinsTsv),
    row("capcontrol",        "type",               CaseFold,  2, 169, Evidence::BinsTsv),
    row("capcontrol",        "voltoverride",       BoolFold,  1, 446, Evidence::BinsTsv),
    row("energymeter",       "element",            CaseFold,  2, 340, Evidence::BinsTsv),
    row("energymeter",       "enabled",            BoolFold,  1, 524, Evidence::BinsTsv),
    row("energymeter",       "linelosses",         BoolFold,  1, 24, Evidence::BinsTsv),
    row("energymeter",       "mask",               ArrayForm, 4, 524, Evidence::BinsTsv),
    row("energymeter",       "option",             ArrayForm, 4, 524, Evidence::BinsTsv),
    row("energymeter",       "peakcurrent",        ArrayForm, 4, 524, Evidence::BinsTsv),
    row("energymeter",       "phasevoltagereport", BoolFold,  1, 2, Evidence::BinsTsv),
    row("energymeter",       "vbaselosses",        BoolFold,  1, 24, Evidence::BinsTsv),
    row("energymeter",       "xfmrlosses",         BoolFold,  1, 24, Evidence::BinsTsv),
    row("expcontrol",        "enabled",            BoolFold,  1, 11, Evidence::BinsTsv),
    row("expcontrol",        "eventlog",           BoolFold,  1, 11, Evidence::BinsTsv),
    row("expcontrol",        "preferq",            BoolFold,  1, 11, Evidence::BinsTsv),
    row("fault",             "bus1",               CaseFold,  2, 18, Evidence::BinsTsv),
    row("fault",             "bus2",               CaseFold,  2, 11, Evidence::BinsTsv),
    row("fault",             "enabled",            BoolFold,  1, 389, Evidence::BinsTsv),
    row("fault",             "temporary",          BoolFold,  1, 389, Evidence::BinsTsv),
    row("fuse",              "enabled",            BoolFold,  1, 106, Evidence::BinsTsv),
    row("fuse",              "monitoredobj",       CaseFold,  2, 106, Evidence::BinsTsv),
    row("fuse",              "switchedobj",        CaseFold,  2, 106, Evidence::BinsTsv),
    row("gendispatcher",     "element",            CaseFold,  2, 48, Evidence::BinsTsv),
    row("gendispatcher",     "enabled",            BoolFold,  1, 48, Evidence::Rp14),
    row("gendispatcher",     "genlist",            ArrayForm, 4, 48, Evidence::BinsTsv),
    row("generator",         "bus1",               CaseFold,  2, 8, Evidence::BinsTsv),
    row("generator",         "conn",               CaseFold,  2, 1, Evidence::BinsTsv),
    row("generator",         "daily",              CaseFold,  2, 1, Evidence::BinsTsv),
    row("generator",         "debugtrace",         BoolFold,  1, 273, Evidence::Rp11),
    row("generator",         "dispmode",           CaseFold,  2, 8, Evidence::BinsTsv),
    row("generator",         "duty",               CaseFold,  2, 3, Evidence::BinsTsv),
    row("generator",         "dynamiceq",          CaseFold,  2, 2, Evidence::Rp11),
    row("generator",         "enabled",            BoolFold,  1, 273, Evidence::Rp11),
    row("generator",         "status",             CaseFold,  2, 273, Evidence::Rp11),
    row("generator",         "userdata",           ArrayForm, 5, 273, Evidence::Rp11),
    row("gicline",           "enabled",            BoolFold,  1, 24, Evidence::BinsTsv),
    row("gicsource",         "enabled",            BoolFold,  1, 4, Evidence::BinsTsv),
    row("gictransformer",    "enabled",            BoolFold,  1, 22, Evidence::BinsTsv),
    row("indmach012",        "bus1",               CaseFold,  2, 10, Evidence::BinsTsv),
    row("indmach012",        "debugtrace",         BoolFold,  1, 16, Evidence::BinsTsv),
    row("indmach012",        "duty",               CaseFold,  2, 10, Evidence::BinsTsv),
    row("indmach012",        "enabled",            BoolFold,  1, 16, Evidence::BinsTsv),
    row("indmach012",        "slipoption",         CaseFold,  2, 16, Evidence::BinsTsv),
    row("invcontrol",        "enabled",            BoolFold,  1, 257, Evidence::BinsTsv),
    row("invcontrol",        "eventlog",           BoolFold,  1, 257, Evidence::BinsTsv),
    row("invcontrol",        "mode",               CaseFold,  2, 211, Evidence::BinsTsv),
    row("invcontrol",        "monbus",             ArrayForm, 4, 15, Evidence::BinsTsv),
    row("invcontrol",        "monbusesvbase",      ArrayForm, 4, 15, Evidence::BinsTsv),
    row("invcontrol",        "monvoltagecalc",     CaseFold,  5, 254, Evidence::BinsTsv),
    row("invcontrol",        "rateofchangemode",   CaseFold,  2, 257, Evidence::BinsTsv),
    row("invcontrol",        "voltage_curvex_ref", EnumSynonym(VOLTAGE_CURVEX_REF_SYNONYMS), 2, 257, Evidence::BinsTsv),
    row("invcontrol",        "voltwattyaxis",      CaseFold,  2, 9, Evidence::BinsTsv),
    row("invcontrol",        "vvc_curve1",         CaseFold,  2, 6, Evidence::BinsTsv),
    row("isource",           "bus1",               CaseFold,  2, 1, Evidence::BinsTsv),
    row("isource",           "enabled",            BoolFold,  1, 137, Evidence::BinsTsv),
    row("isource",           "scantype",           EnumSynonym(SCAN_TYPE_SYNONYMS),     3, 137, Evidence::BinsTsv),
    row("isource",           "sequence",           EnumSynonym(SEQUENCE_TYPE_SYNONYMS), 3, 137, Evidence::BinsTsv),
    row("line",              "enabled",            BoolFold,  1, 77659, Evidence::BinsTsv),
    row("line",              "ratings",            ArrayForm, 4, 77659, Evidence::BinsTsv),
    row("line",              "switch",             BoolFold,  1, 77659, Evidence::BinsTsv),
    row("line",              "wires",              ArrayForm, 5, 77659, Evidence::BinsTsv),
    row("load",              "daily",              CaseFold,  2, 93, Evidence::BinsTsv),
    row("load",              "enabled",            BoolFold,  1, 49629, Evidence::BinsTsv),
    row("load",              "spectrum",           CaseFold,  2, 48, Evidence::BinsTsv),
    row("load",              "status",             CaseFold,  2, 49629, Evidence::BinsTsv),
    row("load",              "yearly",             CaseFold,  5, 32548, Evidence::BinsTsv),
    row("load",              "zipv",               ArrayForm, 5, 49629, Evidence::BinsTsv),
    row("monitor",           "element",            CaseFold,  2, 1539, Evidence::BinsTsv),
    row("monitor",           "enabled",            BoolFold,  1, 1539, Evidence::BinsTsv),
    row("monitor",           "ppolar",             BoolFold,  1, 1519, Evidence::BinsTsv),
    row("monitor",           "residual",           BoolFold,  1, 1539, Evidence::BinsTsv),
    row("monitor",           "vipolar",            BoolFold,  1, 1539, Evidence::BinsTsv),
    row("pvsystem",          "bus1",               CaseFold,  2, 99, Evidence::BinsTsv),
    row("pvsystem",          "daily",              CaseFold,  2, 44, Evidence::BinsTsv),
    row("pvsystem",          "debugtrace",         BoolFold,  1, 463, Evidence::BinsTsv),
    row("pvsystem",          "duty",               CaseFold,  2, 1, Evidence::BinsTsv),
    row("pvsystem",          "effcurve",           CaseFold,  2, 87, Evidence::BinsTsv),
    row("pvsystem",          "enabled",            BoolFold,  1, 463, Evidence::BinsTsv),
    row("pvsystem",          "p-tcurve",           CaseFold,  2, 87, Evidence::BinsTsv),
    row("pvsystem",          "pfpriority",         BoolFold,  1, 463, Evidence::BinsTsv),
    row("pvsystem",          "spectrum",           CaseFold,  2, 1, Evidence::BinsTsv),
    row("pvsystem",          "tdaily",             CaseFold,  2, 43, Evidence::BinsTsv),
    row("pvsystem",          "tduty",              CaseFold,  2, 1, Evidence::BinsTsv),
    row("pvsystem",          "wattpriority",       BoolFold,  1, 463, Evidence::BinsTsv),
    row("reactor",           "bus1",               CaseFold,  2, 31, Evidence::BinsTsv),
    row("reactor",           "bus2",               CaseFold,  5, 47, Evidence::BinsTsv),
    row("reactor",           "enabled",            BoolFold,  1, 670, Evidence::BinsTsv),
    row("reactor",           "parallel",           BoolFold,  1, 670, Evidence::BinsTsv),
    row("recloser",          "enabled",            BoolFold,  1, 230, Evidence::BinsTsv),
    row("recloser",          "eventlog",           BoolFold,  1, 230, Evidence::BinsTsv),
    row("recloser",          "monitoredobj",       CaseFold,  2, 230, Evidence::BinsTsv),
    row("recloser",          "recloseintervals",   ArrayForm, 4, 230, Evidence::BinsTsv),
    row("recloser",          "reset",              BoolFold,  1, 230, Evidence::BinsTsv),
    row("recloser",          "singlephlockout",    BoolFold,  1, 16, Evidence::BinsTsv),
    row("recloser",          "singlephtrip",       BoolFold,  1, 16, Evidence::BinsTsv),
    row("recloser",          "switchedobj",        CaseFold,  2, 230, Evidence::BinsTsv),
    row("regcontrol",        "debugtrace",         BoolFold,  1, 888, Evidence::BinsTsv),
    row("regcontrol",        "enabled",            BoolFold,  1, 888, Evidence::BinsTsv),
    row("regcontrol",        "eventlog",           BoolFold,  1, 888, Evidence::BinsTsv),
    row("regcontrol",        "idle",               BoolFold,  1, 888, Evidence::BinsTsv),
    row("regcontrol",        "inversetime",        BoolFold,  1, 888, Evidence::BinsTsv),
    row("regcontrol",        "reset",              BoolFold,  1, 888, Evidence::BinsTsv),
    row("regcontrol",        "reversible",         BoolFold,  1, 888, Evidence::BinsTsv),
    row("regcontrol",        "revneutral",         BoolFold,  1, 3, Evidence::BinsTsv),
    row("regcontrol",        "transformer",        CaseFold,  2, 308, Evidence::BinsTsv),
    row("relay",             "debugtrace",         BoolFold,  1, 72, Evidence::BinsTsv),
    row("relay",             "distreverse",        BoolFold,  1, 270, Evidence::BinsTsv),
    row("relay",             "doc_p1blocking",     BoolFold,  1, 270, Evidence::BinsTsv),
    row("relay",             "enabled",            BoolFold,  1, 270, Evidence::BinsTsv),
    row("relay",             "eventlog",           BoolFold,  1, 270, Evidence::BinsTsv),
    row("relay",             "monitoredobj",       CaseFold,  2, 270, Evidence::BinsTsv),
    row("relay",             "recloseintervals",   ArrayForm, 4, 270, Evidence::BinsTsv),
    row("relay",             "reset",              BoolFold,  1, 270, Evidence::BinsTsv),
    row("relay",             "switchedobj",        CaseFold,  2, 270, Evidence::BinsTsv),
    row("relay",             "type",               CaseFold,  2, 210, Evidence::BinsTsv),
    row("sensor",            "currents",           ArrayForm, 4, 61, Evidence::BinsTsv),
    row("sensor",            "element",            CaseFold,  2, 61, Evidence::BinsTsv),
    row("sensor",            "enabled",            BoolFold,  1, 61, Evidence::Rp11),
    row("sensor",            "kvars",              ArrayForm, 4, 61, Evidence::BinsTsv),
    row("sensor",            "kws",                ArrayForm, 4, 61, Evidence::BinsTsv),
    row("storage",           "bus1",               CaseFold,  2, 11, Evidence::BinsTsv),
    row("storage",           "daily",              CaseFold,  2, 4, Evidence::BinsTsv),
    row("storage",           "debugtrace",         BoolFold,  1, 463, Evidence::BinsTsv),
    row("storage",           "dispmode",           CaseFold,  2, 120, Evidence::BinsTsv),
    row("storage",           "dynadata",           ArrayForm, 5, 463, Evidence::BinsTsv),
    row("storage",           "effcurve",           CaseFold,  2, 64, Evidence::BinsTsv),
    row("storage",           "enabled",            BoolFold,  1, 463, Evidence::BinsTsv),
    row("storage",           "pfpriority",         BoolFold,  1, 463, Evidence::BinsTsv),
    row("storage",           "state",              CaseFold,  2, 463, Evidence::BinsTsv),
    row("storage",           "wattpriority",       BoolFold,  1, 463, Evidence::BinsTsv),
    row("storagecontroller", "daily",              CaseFold,  2, 1, Evidence::BinsTsv),
    row("storagecontroller", "element",            CaseFold,  2, 229, Evidence::BinsTsv),
    row("storagecontroller", "enabled",            BoolFold,  1, 262, Evidence::BinsTsv),
    row("storagecontroller", "modecharge",         CaseFold,  2, 1, Evidence::BinsTsv),
    row("storagecontroller", "monphase",           CaseFold,  2, 261, Evidence::BinsTsv),
    row("storagecontroller", "seasontargets",      ArrayForm, 5, 262, Evidence::BinsTsv),
    row("storagecontroller", "seasontargetslow",   ArrayForm, 5, 262, Evidence::BinsTsv),
    row("swtcontrol",        "enabled",            BoolFold,  1, 59, Evidence::BinsTsv),
    row("swtcontrol",        "normal",             ArrayForm, 4, 59, Evidence::BinsTsv),
    row("swtcontrol",        "reset",              BoolFold,  1, 59, Evidence::BinsTsv),
    row("swtcontrol",        "state",              ArrayForm, 4, 59, Evidence::BinsTsv),
    row("swtcontrol",        "switchedobj",        CaseFold,  2, 59, Evidence::BinsTsv),
    row("transformer",       "conn",               CaseFold,  2, 21164, Evidence::BinsTsv),
    row("transformer",       "enabled",            BoolFold,  1, 21164, Evidence::BinsTsv),
    row("transformer",       "ratings",            ArrayForm, 4, 21164, Evidence::BinsTsv),
    row("transformer",       "sub",                BoolFold,  1, 21158, Evidence::BinsTsv),
    row("transformer",       "xfmrcode",           CaseFold,  2, 15314, Evidence::BinsTsv),
    row("transformer",       "xrconst",            BoolFold,  1, 21164, Evidence::BinsTsv),
    row("transformer",       "xscarray",           ArrayForm, 4, 21164, Evidence::BinsTsv),
    row("upfc",              "enabled",            BoolFold,  1, 13, Evidence::BinsTsv),
    row("upfc",              "losscurve",          CaseFold,  2, 1, Evidence::BinsTsv),
    row("vccs",              "bus1",               CaseFold,  2, 6, Evidence::BinsTsv),
    row("vccs",              "enabled",            BoolFold,  1, 15, Evidence::BinsTsv),
    row("vccs",              "rmsmode",            BoolFold,  1, 15, Evidence::BinsTsv),
    row("vsource",           "daily",              CaseFold,  2, 2, Evidence::BinsTsv),
    row("vsource",           "enabled",            BoolFold,  1, 2042, Evidence::BinsTsv),
    row("vsource",           "model",              CaseFold,  2, 3, Evidence::BinsTsv),
    row("vsource",           "scantype",           EnumSynonym(SCAN_TYPE_SYNONYMS),     3, 2042, Evidence::BinsTsv),
    row("vsource",           "sequence",           EnumSynonym(SEQUENCE_TYPE_SYNONYMS), 3, 2042, Evidence::BinsTsv),
    row("windgen",           "enabled",            BoolFold,  1, 5, Evidence::Rp13),
];

/// **Count lock, total** — asserted as an EQUALITY in both lanes
/// (`props_roundtrip.rs:62-68,238` pattern), so the table is fail-on-stale in
/// both directions: a dropped row shrinks the compare silently, an added row
/// widens what the engine is allowed to spell differently. Moving it belongs in
/// the commit that argues for the new population.
pub const NORM_ROWS: usize = 170;
/// Count lock, [`NormRule::BoolFold`]: bin 1's 75 pairs − 5 pure-echo + 7
/// WP-RP1.
const NORM_BOOL_FOLD_ROWS: usize = 77;
/// Count lock, [`NormRule::CaseFold`]: bin 2's 61 pairs (59 case + 2 trail) + 2
/// WP-RP1, **− 1** for `invcontrol.voltage_curvex_ref`, which RP2.2 re-typed as
/// an `EnumSynonym` row ([`VOLTAGE_CURVEX_REF_SYNONYMS`]), **+ 3** RP2.3
/// off-bin rows (`load.yearly`, `reactor.bus2`, `invcontrol.monvoltagecalc`).
const NORM_CASE_FOLD_ROWS: usize = 65;
/// Count lock, [`NormRule::ArrayForm`]: bin 4's 21 pairs − 4 (module doc),
/// **+ 6** RP2.3 off-bin rows (`line.wires`, `load.zipv`, `generator.userdata`,
/// `storage.dynadata`, `storagecontroller.seasontargets`/`seasontargetslow`).
const NORM_ARRAY_FORM_ROWS: usize = 23;
/// Count lock, [`NormRule::EnumSynonym`]: **5** since RP2.2 — bin 3's 8 pairs
/// minus the 4 the dossier routed elsewhere (module doc §"RP2.2's routing of
/// bin 3"; the routing itself is `props_r4133_replay::RP22_ROUTING`), plus the
/// one bin-2-labelled pair whose off-bin cells are a real enum spelling
/// ([`VOLTAGE_CURVEX_REF_SYNONYMS`]).
const NORM_ENUM_SYNONYM_ROWS: usize = 5;

/// **The r4133 props display floor — RP2.4's derived value: `2e-4` relative.**
///
/// The chain's fourth and last link. A numeric property cell whose two sides
/// agree to within this relative gap is claimed as ONE value differently
/// *printed*; anything above it still fails, raw, with both spellings in the
/// message. It is the **only** tolerance `R4133_PROPS_PLAN.md` introduces, it
/// is r4133-only by construction (below), and it is never read by `tol_for`
/// and touches no [`Tolerances`] field (plan §1.2 last bullet, §1.3 "No
/// tolerance tier moves").
///
/// [`Tolerances`]: super::Tolerances
///
/// # Derivation (RP2.4 part A, 2026-08-23 — measured, not assumed)
///
/// Measured over the vendored evidence at `tests/corpus/props_r4133/` —
/// `examples_full.txt` + `examples_supplement.txt`, i.e. **every distinct
/// `(rust, r4133)` spelling of every census pair**, so the worst *spelling* is
/// the worst *cell*. Each row's gap is [`display_rel`], the same metric the
/// floor applies.
///
/// | quantity | value | what it is |
/// |---|---|---|
/// | worst cell the floor claims | **6.431124e-05** | `load.pf` `'0.747651914485831'` vs `'0.7477'`, 33 cells, in scope |
/// | floor | **2e-4** | 3.110x above that worst |
/// | nearest row ABOVE the band | **1.374769e-03** | `storagecontroller.kwneed` `'-4387.3616098756'` vs `'-4381.33'` — 6.874x above the floor |
/// | nearest genuine value jump | **4.404256e-03** | `generator.kvar`, the GenDispatcher `weights` registration decks — 22.02x above the floor |
/// | smallest **in-scope** genuine jump | **5.524501e-02** | `regcontrol.remoteptratio` — 276.2x above the floor |
///
/// So the band `(6.431124e-05, 1.374769e-03)` is **empty**: 21.38x wide, and
/// every cut inside it partitions the population the same way. That emptiness
/// — not a chosen number — is what makes the floor a classification rather
/// than a fudge, and it is the same argument `bins.tsv`'s own 1e-4 bin cut
/// rests on (vendored `README.md` §"Numeric pairs (bins 6-7)").
///
/// **Two recalibrations against the plan's provisional numbers**, both recorded
/// because the plan asked for a re-derivation and got a different second half:
///
/// * the worst display cell is confirmed exactly — the plan's `6.43e-5` on
///   `load.pf` is this table's `6.431124e-05`;
/// * the plan's "smallest genuine jump `1.00e-3`, `invcontrol.lpftau`" is a
///   number in the **census's** metric, which for a zero expected value reports
///   the ABSOLUTE difference (`harness::value_verdict`'s `max_rel`). Under this
///   floor's symmetric metric that same cell (`'0.001'` vs `'0.0'`) is rel
///   **1.0**, not 1e-3 — a 0-vs-nonzero pair can never be claimed. The
///   re-derived neighbours above the band are the three rows in the table.
///
/// # Mechanism, checked per cell — not a family named in prose
///
/// **The mechanism is a CLAUSE of this predicate**, [`display_is_render`]: every
/// number of the r4133 side must be the port's number rounded to the significant
/// digits r4133 itself printed. Two upstream mechanisms satisfy that and nothing
/// else does — r4133 *printing* the double both engines hold through a
/// fixed-significant-digit `Format`, and r4133 *holding* a double that is itself
/// the re-parse of one such `%.Ng` command-string write (`MakePosSequence`, the
/// `Save`/`PropertyValue[]` round-trips) while the port sets the value directly.
/// In the first the two engines hold the SAME double and only the renders
/// differ; in the second they hold different doubles, one rounding step apart,
/// and the port's is the exact one.
///
/// This replaces the survey RP2.4 part A landed ("every claimed cell is r4133
/// printing a live double in its own `GetPropertyValue`"), which the audit round
/// disproved: 55 vendored spellings over 27 pairs — `load.kva`, `vsource.puz*`,
/// `line.b0`/`b1`, `reactor.lmh`, `transformer.normamps`… — are gaps NO single
/// `%.Ng` render can produce, because a round trip happened upstream of a
/// derived quantity (r4133's kVA recomputed from an already round-tripped `pf`,
/// its `puZ*` from a round-tripped Z) or because the two engines simply differ.
/// They are refused, and owned by RP3.9 (`props_r4133_replay::RP39_ROUTING`).
///
/// The getters that print at a fixed precision, for the reader tracing a cell
/// (the floor reads the precision off the r4133 spelling itself, never off this
/// table). `%.Ng` rounds a normalized mantissa `m in [1,10)` to N significant
/// digits, so its class ceiling is `0.5*10^(1-N)/m <= 0.5*10^(1-N)`:
///
/// | formatter | ceiling | r4133 sites (`Version8/Source/`) |
/// |---|---|---|
/// | `%-.4g` | 5.0e-4 | `PCElements/Load.pas:2345` (`pf`), `:2353` (`CFactor`, no census row) — the only 4-digit property getters in the surface |
/// | `%-.5g` | 5.0e-5 | `PCElements/Vsource.pas:1327-1335` (`angle`/`mvasc3`/`mvasc1`/`isc3`/`isc1`/`r1`/`x1`/`r0`/`x0`) and `:1343` (`basemva`), `PDElements/Transformer.pas:1842-1843` + `PDElements/AutoTrans.pas:1886-1887` (`normamps`/`emergamps`), and the `MakePosSequence` command-string round-trips `Transformer.pas:1982-1991`, `AutoTrans.pas:2021-2030`, `Reactor.pas:1145-1201` |
/// | `%.6g` | 5.0e-6 | `PCElements/Storage.pas:1531-1562` + the PVSystem analogues, and `Common/Utilities.pas:2600-2607` `GetDSSArray_Real` (`'[' + ' %-.6g'xn + ']'`) |
/// | `%-.7g` | 5.0e-7 | `PDElements/Line.pas:1358-1365` (`length`/`r*`/`x*`/`c*`), `:1406-1407` (`b1`/`b0`) |
/// | `%-.8g` | 5.0e-8 | `Vsource.pas:1337-1342` (`Z*`/`puZ*`) and `:1344` (`puzideal`), `PDElements/Reactor.pas:1091-1098` (`r`/`x`/`z*`/`lmh`) |
///
/// `vsource.basekv` is deliberately NOT in that table although the floor claims
/// its cells: it is Vsource property **2** (`Vsource.pas:171`), the `Case` above
/// has no arm for it, and it therefore falls through to `Inherited` and returns
/// the stored `PropertyValue[]` string — an echoed command-string token whose
/// precision is whatever wrote it. The render clause reads that precision off
/// the spelling and holds all the same.
///
/// **The floor is derived from the MEASUREMENT, not from that ceiling column**,
/// and the difference is load-bearing. `load.pf`'s theoretical `%-.4g` ceiling
/// is 5e-4, which is 2.75x under the band's upper neighbour — it would not fit
/// the band at all. What the census actually contains is 293 `load.pf`
/// spellings with a smallest mantissa of **5.653** (`pf = 0.5653`), i.e. a
/// population ceiling of **8.845e-05**, and the floor sits 2.26x above THAT.
/// The residual is stated rather than absorbed: a future in-scope load with
/// `pf` in `[0.1, 0.25)` could print a cell up to 5e-4 and the floor would
/// **refuse** it, reddening the gate. That is the safe direction — a loud
/// failure with the row in the message and this derivation to re-run — and it
/// is why the number is not widened to the class ceiling today. Widening it to
/// make such a cell pass is exactly what the tolerance discipline forbids
/// (CLAUDE.md, `tests/TOLERANCE_NOTES.md`).
///
/// # What it does NOT claim
///
/// * anything whose two sides do not share a numeric skeleton, or carry
///   different counts of numbers — an enum, a boolean, an array of another
///   length, `''` against a value. Those keep failing raw ([`display_rel`]);
/// * a 0-vs-nonzero pair, at any magnitude (rel is 1 by construction);
/// * a non-finite number on either side unless both sides are literally equal;
/// * **a gap no single `%.Ng` render of the port's value explains**, however
///   small ([`display_is_render`]) — the mechanism clause above;
/// * **anything at all on the `capi_v0145` channel.** Both callers are
///   r4133-only: [`NormRule::ArrayForm`]'s per-token compare is reached only
///   from `PropsPolicy::normalize`'s r4133 arm, [`under_display_floor_r4133`]
///   only from `PropsPolicy::under_display_floor`'s, and [`claim_value`]
///   refuses every channel but [`PropsChannel::R4133`].
///
/// What it still cannot tell apart is a display artifact from a *genuine*
/// difference that is BOTH under 2e-4 and shaped exactly like a rounding of our
/// value to the digits r4133 printed — a floor never can, and the mechanism
/// clause narrows that residue without closing it. What bounds it is the rest of
/// the gate: on every `engines: "both"` case the same properties are value-
/// compared on the `capi_v0145` channel, where this floor is unreachable and the
/// compare runs at the case's tier floors (`tol_for`: 1e-9/1e-6 `micro`,
/// 1e-7/1e-5 `feeder` — orders under this floor; the loosest kinds in the corpus
/// are `midi` at 1e-6/1e-4 and `micro_wtg3_dynamics` at 2e-5/1e-4, where a small
/// enough value can pass at both), and the model gate (Y/V/I/P) runs at tier
/// floors on the `r4133`-only ones. That is a bound, not the "zero tolerance"
/// RP2.4 part B first claimed (audit round, 2026-08-23).
const R4133_DISPLAY_FLOOR: Option<f64> = Some(2e-4);

/// **The floor slot, read back** — the RP2.1 part-C replay's chain needs a
/// *named* fourth link (`shape allowlist → normalization → echo table → display
/// floor`) and reads the real slot rather than restating a literal, so the
/// replay's floor link claims through the same constant the comparator uses.
pub fn display_floor() -> Option<f64> {
    R4133_DISPLAY_FLOOR
}

impl NormRule {
    /// **The predicate the whole value-preservation contract reduces to**: do
    /// `rust` and `oracle` spell the SAME value under this rule?
    ///
    /// `false` means the engine hands the raw pair to the assert — a real
    /// divergence still fails, with both original spellings in the message.
    fn claims(&self, rust: &str, oracle: &str) -> bool {
        match self {
            BoolFold => match (fold_bool(rust), fold_bool(oracle)) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
            CaseFold => rust.trim().eq_ignore_ascii_case(oracle.trim()),
            ArrayForm => array_forms_match(rust, oracle),
            EnumSynonym(map) => map.iter().any(|(ours, theirs)| {
                rust.trim().eq_ignore_ascii_case(ours) && oracle.trim().eq_ignore_ascii_case(theirs)
            }),
        }
    }

    /// Stable tag for messages and for the RP2.1 part-C replay accounting.
    pub fn tag(&self) -> &'static str {
        match self {
            BoolFold => "BoolFold",
            CaseFold => "CaseFold",
            ArrayForm => "ArrayForm",
            EnumSynonym(_) => "EnumSynonym",
        }
    }
}

/// The boolean a Delphi/FPC spelling denotes, or `None` when the string is not
/// a boolean spelling at all — **`''` included** (see [`NormRule::BoolFold`]).
fn fold_bool(s: &str) -> Option<bool> {
    let t = s.trim();
    for y in ["yes", "y", "true"] {
        if t.eq_ignore_ascii_case(y) {
            return Some(true);
        }
    }
    for n in ["no", "n", "false"] {
        if t.eq_ignore_ascii_case(n) {
            return Some(false);
        }
    }
    None
}

/// Split an array render into its tokens: `[`, `]`, `(`, `)`, `,` and
/// whitespace are separators, empty pieces dropped.
fn array_tokens(s: &str) -> impl Iterator<Item = &str> {
    s.split(|c: char| matches!(c, '[' | ']' | '(' | ')' | ',') || c.is_whitespace())
        .filter(|t| !t.is_empty())
}

/// **The metric the floor is expressed in**: `|a-b| / max(|a|,|b|)`, symmetric
/// so neither side is privileged as "expected" (both are renders of one live
/// double, and which one is longer is an accident of the getter).
///
/// `None` means *not comparable as numbers*, and the floor may never claim such
/// a pair: a non-finite value on either side, unless the two are literally
/// equal. (The `0x008e1248` EPRI bus name whose token overflows to `inf` is the
/// motivating case — two IDENTICAL such tokens must still match, two different
/// ones must not; the same guard `harness::value_verdict` carries.)
///
/// The denominator is only ever taken when `a != b`, so it is strictly
/// positive: a 0-vs-nonzero pair answers `Some(1.0)` and is refused by any
/// floor below 1, which is exactly what bin 7's frozen-default echoes need
/// (`invcontrol.lpftau` `'0.001'` vs `'0.0'`).
fn number_rel(a: f64, b: f64) -> Option<f64> {
    if !(a.is_finite() && b.is_finite()) {
        return if a == b { Some(0.0) } else { None };
    }
    let d = (a - b).abs();
    if d == 0.0 {
        return Some(0.0);
    }
    Some(d / a.abs().max(b.abs()))
}

/// **The precision an r4133 number was PRINTED at** — `(significant digits,
/// decimal exponent of the leading digit)` of a numeric literal, or `None` when
/// the token carries no significant digit (a zero, or no digit at all).
///
/// Read off the **text**, never off the parsed `f64`: `'19000'` is five printed
/// digits although `1.9e4` reproduces the same double in two, and the decimal
/// grid the printer rounded onto is exactly what [`is_display_render`] needs.
/// Leading zeros are not significant (`'0.7477'` is four digits at exponent −1);
/// a surviving trailing zero is (Delphi's `%g` strips fractional trailing zeros,
/// so one that reaches us was printed on purpose).
///
/// Every token this sees came from [`numeric_skeleton_texts`], i.e. it parses as
/// an `f64` by construction.
///
/// [`numeric_skeleton_texts`]: super::numeric_skeleton_texts
fn decimal_shape(text: &str) -> Option<(u32, i32)> {
    let t = text.trim();
    let t = t.strip_prefix(['+', '-']).unwrap_or(t);
    let (mantissa, exponent) = match t.find(['e', 'E']) {
        Some(i) => (&t[..i], t[i + 1..].parse::<i32>().ok()?),
        None => (t, 0),
    };
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits: String = format!("{int_part}{frac_part}");
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let leading_zeros = digits.bytes().take_while(|b| *b == b'0').count();
    let significant = digits.len() - leading_zeros;
    if significant == 0 {
        return None;
    }
    let lead_exponent = int_part.len() as i32 - 1 - leading_zeros as i32 + exponent;
    Some((significant as u32, lead_exponent))
}

/// Slack on the half-step, for the last ulp of the decimal→binary conversion and
/// for a tie the printer may break either way (`%g` rounds half away from zero,
/// the shortest-round-trip renderer half to even). Nine orders of magnitude
/// under the smallest gap it could hide.
const RENDER_TIE_MARGIN: f64 = 1.0 + 1e-9;

/// **The floor's MECHANISM clause, per number**: is `oracle` the port's number
/// rounded to the significant digits r4133 printed?
///
/// This is what makes "a display divergence" a checkable property of the cell
/// rather than a family named in prose (RP2.4 audit settlement, 2026-08-23 —
/// both majors). Two upstream mechanisms satisfy it and nothing else does:
///
/// * r4133 **printing** the double both engines hold through a
///   fixed-significant-digit `Format` in its `GetPropertyValue` — then `oracle`
///   is that double rounded to N digits, by definition of the printer;
/// * r4133 **holding** a double that is itself the re-parse of one such `%.Ng`
///   command string an upstream kernel wrote (`MakePosSequence`, the `Save` /
///   `PropertyValue[]` round-trips) while the port sets the value directly —
///   then `oracle` prints its own full value, which is still the port's value
///   rounded to the digits of that one write.
///
/// What it REFUSES is a gap no single N-digit rounding explains: a chained or
/// derived round-trip (`load.kva`, whose kVA is recomputed from an already
/// round-tripped `pf`; `vsource.puz*`, recomputed from a round-tripped Z), and
/// a plain value difference that happens to be small. Those are real state
/// divergences — the two engines hold different doubles further apart than one
/// print can account for — and RP2.4 hands them to RP3.9 rather than claiming
/// them (`props_r4133_replay::RP39_ROUTING`).
///
/// The bound is the class ceiling of the printed precision, `0.5·10^(e-N+1)`,
/// plus two documented slacks: [`RENDER_TIE_MARGIN`], and half a unit of the
/// **15-digit** grid, which is the intermediate FPC's own `Str`/`FloatToDecimal`
/// conversion may round through before rounding to N (a double rounding is still
/// a render, and at N ≤ 14 that second term is at least ten times under the
/// first).
fn is_display_render(rust: f64, oracle: f64, oracle_text: &str) -> bool {
    if rust == oracle {
        return true;
    }
    if !(rust.is_finite() && oracle.is_finite()) {
        return false;
    }
    let Some((digits, exponent)) = decimal_shape(oracle_text) else {
        return false;
    };
    let step = 10f64.powi(exponent - digits as i32 + 1);
    let intermediate = 10f64.powi(exponent - 14);
    if !(step.is_finite() && intermediate.is_finite()) {
        return false;
    }
    (rust - oracle).abs() <= 0.5 * step * RENDER_TIE_MARGIN + 0.5 * intermediate
}

/// **The floor's mechanism clause, per cell** — every number of the r4133 side
/// is the port number rounded to the digits r4133 printed
/// ([`is_display_render`]).
///
/// Same decomposition as [`display_rel`] and the same refusals for a cell that
/// is not a numeric divergence at all, so the two clauses of
/// [`under_display_floor`] see one population.
pub fn display_is_render(rust: &str, oracle: &str) -> bool {
    let (skel_a, nums_a) = super::numeric_skeleton_texts(rust);
    let (skel_b, nums_b) = super::numeric_skeleton_texts(oracle);
    if skel_a != skel_b || nums_a.len() != nums_b.len() || nums_a.is_empty() {
        return false;
    }
    nums_a
        .iter()
        .zip(&nums_b)
        .all(|((a, _), (b, text))| is_display_render(*a, *b, text))
}

/// **The one function the display floor widens** — do two numeric tokens denote
/// the same number?
///
/// Exact when [`R4133_DISPLAY_FLOOR`] is `None`; at a derived floor, within it
/// in the [`number_rel`] metric **and** a render of `a` at the precision `b` was
/// printed with ([`is_display_render`], which is why the oracle token's text is
/// a parameter and not just its value). Its one caller is [`tokens_match`]
/// ([`NormRule::ArrayForm`]'s per-element compare, r4133-only — see the floor's
/// doc); the whole-cell predicate [`under_display_floor`] applies the same two
/// clauses over [`display_rel`] and [`display_is_render`].
fn numbers_match(a: f64, b: f64, b_text: &str) -> bool {
    match R4133_DISPLAY_FLOOR {
        None => a == b,
        Some(rel) => number_rel(a, b).is_some_and(|r| r <= rel) && is_display_render(a, b, b_text),
    }
}

/// **The floor's whole-cell metric**: the largest [`number_rel`] over the two
/// sides' numbers, or `None` when the cell is not a numeric divergence at all.
///
/// It decomposes both renders with the comparator's own
/// [`numeric_skeleton`](super::numeric_skeleton) — the same scanner
/// `value_verdict` uses, so "numeric" means here exactly what it means to the
/// assert this floor sits in front of — and answers `None` unless
///
/// * the two **non-numeric skeletons are identical** (so `'Yes'` vs `'true'`,
///   `''` vs `'[]'`, `'17'` vs `'1 16 +'` are never numbers-within-a-floor),
/// * the two carry the **same count** of numbers (a one-element array never
///   folds into a three-element one), and
/// * that count is **non-zero** (a cell with no number on either side has no
///   value for a numeric floor to claim), and
/// * every number pair is comparable ([`number_rel`]).
///
/// Scalars, bracketed vectors and `|`-separated matrices all go through this
/// one path: `capacitor.cuf`'s `'[ 287.82360946885]'` vs `'[ 287.824]'` and
/// `line.cmatrix`'s three-row render are as much display cells as `load.pf`'s
/// bare double, and a scalar-only floor would leave them unclaimed.
pub fn display_rel(rust: &str, oracle: &str) -> Option<f64> {
    let (skel_a, nums_a) = super::numeric_skeleton(rust);
    let (skel_b, nums_b) = super::numeric_skeleton(oracle);
    if skel_a != skel_b || nums_a.len() != nums_b.len() || nums_a.is_empty() {
        return None;
    }
    let mut worst = 0.0f64;
    for (a, b) in nums_a.iter().zip(&nums_b) {
        worst = worst.max(number_rel(*a, *b)?);
    }
    Some(worst)
}

/// **The floor, as a cell verdict** — the offline twin of
/// [`under_display_floor_r4133`], counters aside.
///
/// Two clauses, both necessary: the gap is inside [`R4133_DISPLAY_FLOOR`] in the
/// [`display_rel`] metric (the *size* of the divergence), and every number of
/// the r4133 side is the port number rounded to the digits r4133 printed
/// ([`display_is_render`] — its *mechanism*). The second is what keeps the floor
/// a classification: a cell 1e-6 apart whose two engines simply hold different
/// doubles is refused at any floor value (RP2.4 audit settlement).
///
/// `false` whenever the floor is `None`, whenever the cell is not a numeric
/// divergence ([`display_rel`]), whenever the gap exceeds the floor, and
/// whenever no single `%.Ng` render explains it.
pub fn under_display_floor(rust: &str, oracle: &str) -> bool {
    match (R4133_DISPLAY_FLOOR, display_rel(rust, oracle)) {
        (Some(floor), Some(rel)) => rel <= floor && display_is_render(rust, oracle),
        _ => false,
    }
}

/// **The shipped floor seam**, called from `PropsPolicy::under_display_floor`'s
/// r4133 arm (and only from there — same channel gate as [`normalize_r4133`]
/// and [`echo_excluded_r4133`]).
///
/// It answers what [`under_display_floor`] answers and additionally records
/// what the gate saw: a **visit** is a cell that reached the seam at all, a
/// **hit** is a cell the floor actually claimed. Unlike the two tables above
/// the floor has no rows, so there is no per-row staleness to guard; the
/// counters exist so RP4.1 can measure the link's live population against the
/// replay's `CLAIMED_DISPLAY_FLOOR`, and so [`display_floor_counters`] can
/// prove to the offline tests that a query moved nothing.
pub fn under_display_floor_r4133(rust: &str, oracle: &str) -> bool {
    FLOOR_VISITS.fetch_add(1, AtomicOrd::Relaxed);
    let claimed = under_display_floor(rust, oracle);
    if claimed {
        FLOOR_HITS.fetch_add(1, AtomicOrd::Relaxed);
    }
    record_touch(|t| &mut t.floor, claimed);
    claimed
}

/// Cells that reached the floor seam on the r4133 channel.
static FLOOR_VISITS: AtomicUsize = AtomicUsize::new(0);
/// …and how many of them it claimed.
static FLOOR_HITS: AtomicUsize = AtomicUsize::new(0);

/// **What the three shipped seams did ON THIS THREAD** — `(visits, hits)` per
/// seam, the deterministic twin of the process-global counters.
///
/// The statics above (and [`NORM_VISITS`]/[`NORM_HITS`],
/// [`ECHO_VISITS`]/[`ECHO_HITS`]) are what the *gate* reads: they answer "what
/// did this process compare", which is a per-process question. Several offline
/// tests ask a different one — "did MY query reach a counting seam at all?" —
/// and used to answer it by snapshotting the global totals around their own
/// body. That is only sound while nothing else runs, and `cargo test` runs the
/// binary's tests concurrently: sibling tests deliberately drive the real
/// comparator ([`tests::the_echo_seam_counts_visits_and_hits`],
/// `harness::props_policy_tests::the_r4133_channel_folds_the_documented_spellings`,
/// …), so the totals move under the assertion's feet. Measured by RP3.3's audit
/// round: `tests::the_value_chain_resolves_in_order_and_agrees_with_the_seam`
/// failed on ~1–3 % of runs with "the chain query moved a counter", a pure
/// ordering artefact.
///
/// libtest gives every `#[test]` its own thread, so a per-thread counter
/// answers the offline question exactly: the query runs on this thread, and if
/// it reaches a seam the touch lands here and nowhere else. It is strictly
/// stronger than the global delta it replaces — no sibling can mask a real
/// touch by moving the total the other way — and it cannot flake.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct SeamTouches {
    /// [`normalize_r4133`]'s `(visits, hits)`.
    pub norm: (usize, usize),
    /// [`echo_excluded_r4133`]'s.
    pub echo: (usize, usize),
    /// [`under_display_floor_r4133`]'s.
    pub floor: (usize, usize),
}

thread_local! {
    static SEAM_TOUCHES: Cell<SeamTouches> = const { Cell::new(SeamTouches {
        norm: (0, 0),
        echo: (0, 0),
        floor: (0, 0),
    }) };
}

/// Record one seam touch on the calling thread; `hit` is the seam's own
/// hit predicate, so the pair mirrors the process-global counters exactly.
fn record_touch(pick: fn(&mut SeamTouches) -> &mut (usize, usize), hit: bool) {
    SEAM_TOUCHES.with(|c| {
        let mut t = c.get();
        let slot = pick(&mut t);
        slot.0 += 1;
        slot.1 += usize::from(hit);
        c.set(t);
    });
}

/// The calling thread's [`SeamTouches`] so far.
pub fn seam_touches_here() -> SeamTouches {
    SEAM_TOUCHES.with(Cell::get)
}

/// The live floor accounting, `(visits, hits)` — read by the gate's reporting
/// and by the offline tests that assert a query moved no counter.
pub fn display_floor_counters() -> (usize, usize) {
    (
        FLOOR_VISITS.load(AtomicOrd::Relaxed),
        FLOOR_HITS.load(AtomicOrd::Relaxed),
    )
}

/// One array token: by value when both sides parse as `f64`
/// (`0` == `0.0`, `1E-005` == `0.00001`), otherwise ASCII-case-insensitively.
///
/// `a` is always the port's render and `b` the oracle's
/// ([`array_forms_match`]'s argument order, which every caller preserves), and
/// [`numbers_match`] reads `b`'s text as the precision r4133 printed at — so the
/// direction matters and is asserted
/// (`tests::arrayform_folds_delimiters_not_contents`).
fn tokens_match(a: &str, b: &str) -> bool {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => numbers_match(x, y, b),
        _ => a.eq_ignore_ascii_case(b),
    }
}

/// [`NormRule::ArrayForm`]'s predicate: same tokens, in order, same count —
/// and at least one token on **each** side (an empty render is bin 5's, not
/// this table's).
fn array_forms_match(rust: &str, oracle: &str) -> bool {
    let mut a = array_tokens(rust).peekable();
    let mut b = array_tokens(oracle).peekable();
    if a.peek().is_none() || b.peek().is_none() {
        return false;
    }
    loop {
        match (a.next(), b.next()) {
            (None, None) => return true,
            (Some(x), Some(y)) if tokens_match(x, y) => continue,
            _ => return false,
        }
    }
}

/// ASCII-case-insensitive byte order — the order [`PROPS_NORM_R4133`] is stored
/// in and [`find_row`] searches by.
fn ci_cmp(a: &str, b: &str) -> Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

/// Index of `table`'s row for `(class, prop)`, case-insensitively. Binary
/// search — the table is sorted by [`ci_cmp`] on `(class, prop)` and has no
/// duplicate key (`table_is_sorted_and_unique`).
fn find_row(table: &[NormRow], class: &str, prop: &str) -> Option<usize> {
    table
        .binary_search_by(|r| ci_cmp(r.class, class).then_with(|| ci_cmp(r.prop, prop)))
        .ok()
}

/// `(row index, did the row claim this cell?)` for one cell against `table`.
///
/// A cell is claimed only when the two sides **differ raw** and the row's rule
/// says they spell the same value: an already-equal cell is not a divergence,
/// so counting it as a hit would make the liveness accounting below claim a row
/// is doing work it is not.
fn lookup_claim(
    table: &[NormRow],
    class: &str,
    prop: &str,
    rust: &str,
    oracle: &str,
) -> Option<(usize, bool)> {
    let i = find_row(table, class, prop)?;
    Some((i, rust != oracle && table[i].rule.claims(rust, oracle)))
}

/// **The offline claim query** — which row of [`PROPS_NORM_R4133`] claims this
/// cell, if any.
///
/// This is what the RP2.1 part-C replay drives its accounting from: it answers
/// the same question the live seam answers, through the same
/// [`lookup_claim`]/[`NormRule::claims`] code (never a parallel
/// reimplementation — a drifting copy would make the replay's completeness
/// proof describe a comparator that does not exist), but it does **not** touch
/// the live counters, which stay the exclusive record of what the gate saw.
pub fn claiming_row(class: &str, prop: &str, rust: &str, oracle: &str) -> Option<&'static NormRow> {
    match lookup_claim(PROPS_NORM_R4133, class, prop, rust, oracle) {
        Some((i, true)) => Some(&PROPS_NORM_R4133[i]),
        _ => None,
    }
}

/// The normalization core against an injected `table` (the self-tests' seam —
/// the shipped path is [`normalize_r4133`], which adds the accounting).
///
/// On a claim it returns **our** spelling on both sides; otherwise the raw
/// pair, untouched.
fn normalize_with<'v>(
    table: &[NormRow],
    class: &str,
    prop: &str,
    rust: &'v str,
    oracle: &'v str,
) -> (Cow<'v, str>, Cow<'v, str>) {
    match lookup_claim(table, class, prop, rust, oracle) {
        Some((_, true)) => (Cow::Borrowed(rust), Cow::Borrowed(rust)),
        _ => (Cow::Borrowed(rust), Cow::Borrowed(oracle)),
    }
}

/// **The shipped seam**, called from `PropsPolicy::normalize`'s r4133 arm (and
/// only from there — see this module's doc for the capi-invariance argument).
pub fn normalize_r4133<'v>(
    class: &str,
    prop: &str,
    rust: &'v str,
    oracle: &'v str,
) -> (Cow<'v, str>, Cow<'v, str>) {
    if let Some((i, claimed)) = lookup_claim(PROPS_NORM_R4133, class, prop, rust, oracle) {
        NORM_VISITS[i].fetch_add(1, AtomicOrd::Relaxed);
        record_touch(|t| &mut t.norm, claimed);
        if claimed {
            NORM_HITS[i].fetch_add(1, AtomicOrd::Relaxed);
            return (Cow::Borrowed(rust), Cow::Borrowed(rust));
        }
    }
    (Cow::Borrowed(rust), Cow::Borrowed(oracle))
}

/// Per-row visit counter, indexed exactly like [`PROPS_NORM_R4133`]: how many
/// cells of that `(class, prop)` reached the seam on the r4133 channel.
static NORM_VISITS: [AtomicUsize; PROPS_NORM_R4133.len()] =
    [const { AtomicUsize::new(0) }; PROPS_NORM_R4133.len()];
/// Per-row hit counter: how many of those cells the row actually claimed (i.e.
/// were divergent raw and folded).
static NORM_HITS: [AtomicUsize; PROPS_NORM_R4133.len()] =
    [const { AtomicUsize::new(0) }; PROPS_NORM_R4133.len()];

/// **Fail-on-stale for [`PROPS_NORM_R4133`]** — plan mechanic (d), the live
/// half of "liveness both ways", modeled on
/// `lane::assert_reround_cells_are_live` (`harness/lane.rs:300-316`).
///
/// A row whose `(class, prop)` was compared on r4133 but which never folded a
/// divergent cell is exempting a spelling difference that is no longer there:
/// drop it, or re-measure it. Silent when the row was never **visited**, which
/// is what makes it correct to ship now:
///
/// * the r4133 property path is still masked (`corpus_gate/scheduler.rs`, the
///   RP4.1 flip), so today every row has `visits == 0` and this helper is a
///   no-op — **the accounting is armed but dormant**, exactly as RP2.1 plans;
/// * after RP4.1 it becomes the anti-rot guard, and it stays self-silencing
///   under `DSS_GATE_ONLY` (a filtered run legitimately visits nothing).
///
/// The OFFLINE half of the same obligation — every row claims at least one
/// vendored census example row — is RP2.1 part C's replay test, which does not
/// need the live gate at all.
pub fn assert_norm_rows_are_live() {
    let read = |c: &[AtomicUsize]| -> Vec<usize> {
        c.iter().map(|c| c.load(AtomicOrd::Relaxed)).collect()
    };
    check_rows_are_live(PROPS_NORM_R4133, &read(&NORM_VISITS), &read(&NORM_HITS));
}

/// The staleness rule itself, over **injected** counters.
///
/// [`assert_norm_rows_are_live`] is a two-line adapter over this, and the split
/// is the whole point: `NORM_VISITS`/`NORM_HITS` are private process-global
/// statics with no way to make them stale from a test, so before the RP2.1 audit
/// round nothing in the tree proved this guard can fire — it is structurally
/// silent until RP4.1 unmasks the r4133 props path, and at that moment it
/// becomes the sole live anti-rot guard for all 157 rows. With the counters as
/// parameters the two directions are pinned offline
/// ([`tests::the_liveness_guard_is_silent_when_dormant_or_live`] and
/// [`tests::the_liveness_guard_fires_on_a_stale_row`]), so the guard is proven
/// before it is relied on.
fn check_rows_are_live(table: &[NormRow], visits: &[usize], hits: &[usize]) {
    assert_eq!(
        (table.len(), table.len()),
        (visits.len(), hits.len()),
        "the counters are indexed exactly like the table"
    );
    for (i, r) in table.iter().enumerate() {
        let (visits, hits) = (visits[i], hits[i]);
        assert!(
            visits == 0 || hits > 0,
            "stale r4133 property normalization row: {}.{} ({}) folded nothing \
             across {visits} compared cell(s). Each row lets the engine spell a \
             value differently from r4133, so it must name a difference that is \
             really there — drop it, or re-measure it (DSS_PROPS_CENSUS).",
            r.class,
            r.prop,
            r.rule.tag()
        );
    }
}

/// Why a `(class, prop)` is excluded from the r4133 VALUE compare rather than
/// normalized — the category tag every [`EchoRow`] carries.
///
/// RP2.3 read all 86 pairs of its bucket and found **five** mechanisms, not
/// three (`rp23_dossier.md` §2). Four of them take a row here; the fifth — the
/// dss_capi 0.14.5 `SilentReadOnly` text surface, where r4133 renders a live
/// computed read-only quantity and the port renders `''` — is **not** an
/// exclusion at all under the 2026-08-02 r4133-authority policy and was routed
/// to a new engine sub-step instead (the kill ruling, `props_r4133_replay::
/// RP38_ROUTING`). Naming it here would have been the lie the kill criterion
/// exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoCategory {
    /// r4133's `GetPropertyValue` has no arm for the index, so it answers the
    /// `InitPropertyValues` **default** string that no code ever refreshed
    /// (`Version8/Source/General/DSSObject.pas:112-115`).
    EchoDefault,
    /// Same fallthrough, but the store holds a string the deck's own parse (or
    /// a derived snapshot taken during `Edit`) wrote, which the live value has
    /// since moved away from — `relay.reset`'s `'0.20'` (the deck's literal
    /// token, `Controls/Relay.pas:504`) and `reactor.bus2`'s
    /// `PropertyValue[2] := GetBus(2)` snapshot (`PDElements/Reactor.pas:
    /// 419-420`), which a later `phases=` never refreshes.
    EchoParse,
    /// **New in RP2.3.** Not an echo: r4133's getter arm IS live, and for an
    /// unset or degenerate collection it renders the *other* empty convention —
    /// `'[]'` from a `for i := 1 to NumPointsBH` loop over zero points
    /// (`PDElements/Transformer.pas:1820-1827`), `'()'` from a paren wrapper
    /// around an empty store (`PCElements/Storage.pas:1574`), or a bare `''`
    /// from a getter that exits early (`Controls/StorageController.pas:
    /// 2445-2449`, `PCElements/Load.pas:2354-2357`) — where the port renders
    /// `''`, `[]` or the materialised default vector.
    ///
    /// What all 14 rows share, and all this tag asserts: **neither side is a
    /// `PropertyValue[]` echo** (so [`EchoDefault`]/[`EchoParse`] would be
    /// false), the underlying state is the same on both engines, and the two
    /// renders differ only in the empty-collection convention each getter emits.
    /// It stays an exclusion rather than a normalization rule because an empty
    /// render carries no value to preserve — `''` is not an array, which is
    /// exactly what [`NormRule::ArrayForm`] refuses (`array_forms_match`).
    ///
    /// **Twelve of the 14 also render the same thing on both sides**
    /// (`''`/`[]`/`()` against an empty port render). The other two,
    /// `storagecontroller.seasontargets` and `seasontargetslow`, do not, and the
    /// blanket "the two sides mean the same thing" this doc used to carry was
    /// wider than the evidence (RP2.3 audit settlement, 2026-08-23): with
    /// `Seasons = 1` r4133's `ReturnSeasonTarget` exits before it emits anything
    /// (`Controls/StorageController.pas:2445-2449`) while the port prints the
    /// live single-season target — `'[ 8000]'` / `'[ 4000]'` on 237 cells each.
    /// The *value* behind those renders is identical on both engines
    /// (`SeasonTargets[0] := FkWTarget`, `:883-884`, and `:1470` reads it back
    /// as the dispatch target), which is why the tag still fits and
    /// [`LiveSemanticsDiffer`] would not; what the port adds is a render where
    /// r4133 has none. Both rows carry the capi witness AND
    /// `storagecontroller_seasontargets_render_the_live_targets`.
    ///
    /// [`LiveSemanticsDiffer`]: EchoCategory::LiveSemanticsDiffer
    /// [`EchoDefault`]: EchoCategory::EchoDefault
    /// [`EchoParse`]: EchoCategory::EchoParse
    EmptyCollectionRender,
    /// Not an echo: the two engines genuinely mean different things by the
    /// property, and the port's meaning is the correct one. **Every row of this
    /// category owes an expected-value pin** — the claim "ours is the right
    /// value" is not something a citation alone can carry
    /// ([`tests::every_live_semantics_row_names_a_pin`]).
    LiveSemanticsDiffer,
}

impl EchoCategory {
    /// Stable tag for messages and for the replay accounting.
    pub fn tag(self) -> &'static str {
        match self {
            EchoCategory::EchoDefault => "EchoDefault",
            EchoCategory::EchoParse => "EchoParse",
            EchoCategory::EmptyCollectionRender => "EmptyCollectionRender",
            EchoCategory::LiveSemanticsDiffer => "LiveSemanticsDiffer",
        }
    }
}

/// What holds the port's value where an [`EchoRow`] stops the r4133 compare —
/// the *witness* half of CLAUDE.md's exclusion discipline ("each divergence is
/// excluded field-by-field and pinned by its own expected-value test").
///
/// A row without one is a mask with nothing behind it, so the type makes the
/// two possible witnesses explicit and countable instead of leaving them in
/// prose ([`tests::every_echo_row_carries_a_witness`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoWitness {
    /// The **capi_v0145 channel value-compares this pair** on `n` gating cases
    /// and the port matches on every one — so the value the r4133 row stops
    /// comparing is still asserted, against the other oracle, on `n` cases.
    ///
    /// `n` is the 2026-08-23 claims census's own count (RP2.3 part A). Unlike
    /// [`NormRow::cells`] it is **not** re-derivable from the vendored evidence
    /// — that census's per-channel case counts are not in
    /// `tests/corpus/props_r4133/` — so it is a dated measurement, and only its
    /// load-bearing half is asserted here (`n > 0`: the capi channel really is
    /// a witness). A re-census that moved it would not red anything; what the
    /// number is for is letting a reader see at a glance whether a row leans on
    /// one case or on ninety-nine.
    ///
    /// **What a capi witness cannot say** (RP2.3 audit settlement, 2026-08-23).
    /// Two limits, both now enforced rather than left to the reader:
    ///
    /// * it presupposes the capi channel really compares the pair — a row whose
    ///   pair is masked off capi by `SKIP_PROPS`, `PROPS_015X` or the whole-
    ///   element skip would be claiming a witness that cannot exist
    ///   ([`tests::a_capi_witness_is_a_pair_the_capi_channel_can_compare`]);
    /// * it says **nothing about cells on `engines: "r4133"` cases**, where the
    ///   capi channel never runs at all. 58 of the 82 pairs mask such cells
    ///   ([`ECHO_ROWS_ON_R4133_ONLY_CASES`]), and each of them must therefore
    ///   also name a pin — the rule `swtcontrol.action` applied by hand in part
    ///   B2, now a test
    ///   ([`tests::every_row_exposed_on_r4133_only_cases_names_a_pin`]).
    Capi(u32),
    /// **No capi coverage of the excluded cells** — the capture has no such
    /// property (`PROPS_015X`), a `SKIP_PROPS` row masks it, the capi walk
    /// skips the element whole (`skip_whole_element`), or every covered cell
    /// sits on a capi-only case. The named test is the expected-value pin that
    /// asserts the port's live value on the deck the census flagged.
    Pin(&'static str),
    /// Both: the capi channel covers `n` cases AND a pin holds the value. Used
    /// where the pin is mandatory whatever capi says — every
    /// [`EchoCategory::LiveSemanticsDiffer`] row, plus the rows the plan names
    /// (`energymeter.peakcurrent`).
    CapiAndPin(u32, &'static str),
}

impl EchoWitness {
    /// The pin test's name, if this witness names one.
    pub fn pin(self) -> Option<&'static str> {
        match self {
            EchoWitness::Capi(_) => None,
            EchoWitness::Pin(name) | EchoWitness::CapiAndPin(_, name) => Some(name),
        }
    }

    /// The number of capi-comparing cases, if the capi channel is a witness.
    fn capi_cases(self) -> Option<u32> {
        match self {
            EchoWitness::Capi(n) | EchoWitness::CapiAndPin(n, _) => Some(n),
            EchoWitness::Pin(_) => None,
        }
    }
}

/// One r4133 **value-only** exclusion: the property's name and order are still
/// checked (exactly `SKIP_PROPS`' shape, `harness/mod.rs`), only the value
/// compare is dropped, and only on the r4133 channel.
#[derive(Debug)]
pub struct EchoRow {
    /// Class, spelled as the census spells it. Matched case-insensitively.
    pub class: &'static str,
    /// Property, spelled as the census spells it. Matched case-insensitively.
    pub prop: &'static str,
    /// Which mechanism puts the cell here.
    pub category: EchoCategory,
    /// The pair's census cell count, full census — the same citation column
    /// [`NormRow::cells`] carries, read back against `bins.tsv` / the vendored
    /// `README.md` by `props_r4133_replay::every_echo_row_matches_its_cited_evidence`.
    /// It counts the pair's DIVERGENT cells, not the cells this row masks: on a
    /// mixed pair a `PROPS_NORM_R4133` row claims some of them first (the chain
    /// order), and the per-cell split is the census's, measured live.
    pub cells: u32,
    /// The r4133 `Version8/Source` unit:line that proves the category — the
    /// missing `GetPropertyValue` arm (i.e. the `DSSObject.pas:112-115`
    /// fallthrough) and/or the `InitPropertyValues` line, or the live getter
    /// arm for the two non-echo categories.
    pub cite: &'static str,
    /// What holds the port's value instead (plan §1.2's witness obligation).
    pub witness: EchoWitness,
}

/// Table constructor, so the 82 rows below read as data.
const fn echo(
    class: &'static str,
    prop: &'static str,
    category: EchoCategory,
    cells: u32,
    cite: &'static str,
    witness: EchoWitness,
) -> EchoRow {
    EchoRow {
        class,
        prop,
        category,
        cells,
        cite,
        witness,
    }
}

/// **The echo-exclusion table — created EMPTY by RP2.1, filled by RP2.3.**
///
/// RP2.1 shipped it empty on purpose (plan §0: "`PROPS_ECHO_R4133` is created
/// **empty** in RP2.1; its rows land in RP2.3"), because that emptiness is what
/// proves the *normalization* half claims bins 1/2/4 with no exclusion helping
/// it. RP2.3 fills it from the closed population part C's replay had declared
/// by name: `Owner::Rp23`, **450 example rows over 86 pairs**.
///
/// # What a row does, and what it does not do
///
/// A row drops the VALUE compare of its `(class, prop)` on the **r4133 channel
/// only** — the property's name and index order are still asserted, exactly
/// `SKIP_PROPS`' shape (plan §1.2 prescribes that shape). It is consulted
/// *after* [`PROPS_NORM_R4133`] (the chain order
/// `shape allowlist → normalization → echo → floor`), so on a **mixed** pair
/// the typed rule sees the cell first: 20 of the 82 pairs below also hold a
/// normalization row, and 135 of their example rows are claimed by it
/// (`props_r4133_replay::MULTI_LINK_ROWS`).
///
/// **Nine of those 20 rows are new in RP2.3** (`load.yearly`, `reactor.bus2`,
/// `invcontrol.monvoltagecalc` `CaseFold`; `line.wires`, `load.zipv`,
/// `generator.userdata`, `storage.dynadata`,
/// `storagecontroller.seasontargets`/`seasontargetslow` `ArrayForm`), landed
/// *before* the echo rows, and they hold 6 446 cells (6 370 in scope).
///
/// # What the order buys, and what it does not (audit settlement, 2026-08-23)
///
/// The chain order decides **which link claims a cell**, and that is a real
/// property: those 6 446 cells are dispositioned `normalized-by-<rule>` in the
/// claims census rather than `echo-row`, each of the nine rows records its own
/// hit and can therefore be proved live, and `assert_norm_rows_are_live` has
/// something to fire on after RP4.1.
///
/// It does **not** keep those cells inside the live value compare, and the
/// earlier wording here ("keep 6 446 live cells inside the value compare that a
/// pair-scoped exclusion would otherwise have swallowed") was false at the seam:
/// [`echo_excluded_r4133`] answers on row presence, so once a pair is named here
/// EVERY divergent cell of it skips the value assert on r4133 — the folded ones
/// harmlessly (the two sides are equal by then) and the ones the rule refused
/// too. On the 20 mixed pairs a genuine regression a typed rule would have
/// compared (a wrong resolved loadshape name, a wrong ZIPV vector, a wrong
/// `Bus2` terminal spelling) is therefore caught on the **capi channel only**
/// until the rows are narrowed per cell. That narrowing — with
/// [`ECHO_CARVE_OUTS`], or by giving each mixed row its measured echo spellings
/// — is an explicit RP4.1 precondition (plan §RP4.1); it is not RP2.3's, whose
/// row shape §1.2 fixes.
///
/// # The 82 rows — RP2.3's 81, plus RP3.3's one
///
/// | category | rows | mechanism |
/// |---|---|---|
/// | [`EchoDefault`](EchoCategory::EchoDefault) | 50 | a missing getter arm over an `InitPropertyValues` default |
/// | [`EchoParse`](EchoCategory::EchoParse) | 8 | …over the deck's own token or a derived snapshot |
/// | [`EmptyCollectionRender`](EchoCategory::EmptyCollectionRender) | 14 | a LIVE arm rendering the other empty-collection convention |
/// | [`LiveSemanticsDiffer`](EchoCategory::LiveSemanticsDiffer) | 10 | a real divergence whose port answer is the correct one |
///
/// **RP2.3 landed 81 of them**, and 86 − 81 = the **five** `SilentReadOnly`
/// pairs its kill criterion fired on (`indmach012.pf`,
/// `storagecontroller.kwhtotal`/`kwtotal`/`kwhactual`/`kwactual`): r4133 renders
/// a live computed read-only quantity there and the port renders `''` only
/// because dss_capi 0.14.5 suppresses the text surface. Under the 2026-08-02
/// policy that is an engine fix, not an exclusion, so they take NO row here and
/// are re-routed to their own declared bucket
/// (`props_r4133_replay::RP38_ROUTING`).
///
/// **The 82nd is `generator.model`, landed by RP3.3 (2026-08-24)** — the first
/// row a WP-RP3 root-cause sub-step contributed, and the first that did not come
/// out of RP2.3's declared bucket. Its pair is a bin-7 value jump
/// (`'4'` vs `'3'`, rel 3.33e-01) whose mechanism turned out to be this table's
/// after all: r4133's `TGeneratorObj.GetPropertyValue` has no arm 6, so `model`
/// answers the deck's own typed token while the NCIM PV→PQ conversion moves the
/// live `GenModel` 3 → 4 and never moves it back. Unlike RP2.3's rows it is
/// therefore counted here as a *new* claim rather than a re-disposition, and it
/// moves `props_r4133_replay::CLAIMED_ECHO` and `DECLARED_RP3` together.
///
/// Sorted by `(class, prop)` — [`find_echo_row`] binary-searches it.
// The 82 rows are DATA, in the same aligned-columns style as PROPS_NORM_R4133
// (whose `rustfmt::skip` note applies here for the same reason: rustfmt's
// 60-char call width would explode every row).
#[rustfmt::skip]
pub const PROPS_ECHO_R4133: &[EchoRow] = &[
    echo("autotrans", "bhcurrent", EmptyCollectionRender, 44,
         "AutoTrans.pas:1865-1871 (arm 44 loops NumPointsBH=0 -> '[]')",
         Pin("autotrans_bh_arrays_render_empty_when_unset")),
    echo("autotrans", "bhflux", EmptyCollectionRender, 44,
         "AutoTrans.pas:1872-1878 (arm 45, same)",
         Pin("autotrans_bh_arrays_render_empty_when_unset")),
    echo("autotrans", "pctperm", EchoDefault, 44,
         "AutoTrans.pas:1958 ('100') + :1883-1888 (PD tail re-renders slots 1..2 only) -> DSSObject.pas:112-115",
         CapiAndPin(9, "pd_element_perm_and_repair_render_the_live_ratings")),
    echo("autotrans", "repair", EchoDefault, 44,
         "AutoTrans.pas:1959 ('36'), same fallthrough",
         CapiAndPin(9, "pd_element_perm_and_repair_render_the_live_ratings")),
    echo("capcontrol", "reset", EchoDefault, 446,
         "CapControl.pas:196 (prop 22) + :1254-1282 (init writes 20,21,23, never 22) -> DSSObject.pas:112-115",
         CapiAndPin(31, "energymeter_action_and_capcontrol_reset_render_no_pending_command")),
    echo("capcontrol", "type", EchoParse, 169,
         "CapControl.pas:178 (prop 4) + :296-297 (store written before the CASE) vs :304-311",
         Capi(3)),
    echo("energymeter", "action", EchoDefault, 524,
         "EnergyMeter.pas:482 (prop 3) + :2205 ('clear') + :2645-2658 (no arm 3); one cell is the deck's own 'C' via :618",
         CapiAndPin(67, "energymeter_action_and_capcontrol_reset_render_no_pending_command")),
    echo("energymeter", "peakcurrent", EchoDefault, 524,
         "EnergyMeter.pas:2209 ('(400, 400, 400)') + :2640-2643/:2660-2663 (index 7 paren-wrapped, no arm)",
         CapiAndPin(6, "energymeter_peakcurrent_renders_the_live_one_element_array")),
    echo("expcontrol", "derlist", LiveSemanticsDiffer, 11,
         "ExpControl.pas:696 + :702-715 (index 14 answers FPVSystemNameList) vs :227-234 / :247-252",
         CapiAndPin(2, "expcontrol_derlist_renders_the_der_list")),
    echo("fault", "bus2", EchoParse, 11,
         "Fault.pas:699-717 (arm 6 only) + :672 / :297 (re-snapshot at every bus1=)",
         Pin("fault_bus2_renders_the_live_terminal")),
    echo("fault", "pctperm", EchoDefault, 389,
         "Fault.pas:687 ('0') -> DSSObject.pas:112-115",
         CapiAndPin(13, "pd_element_perm_and_repair_render_the_live_ratings")),
    echo("fuse", "switchedobj", EchoDefault, 106,
         "Fuse.pas:183 (prop 3) + :801ff ('') + :680-720 (no arm 3); live ElementName defaults to the monitored element (:292)",
         CapiAndPin(1, "fuse_switchedobj_defaults_to_the_monitored_element")),
    echo("generator", "d", LiveSemanticsDiffer, 272,
         "generator.pas:969 (Create sets GenVars.D, never Dpu) + :2585 (the store snapshots Dpu = 0) + :467 help 'Default is 1.0' + :2710",
         CapiAndPin(25, "generator_d_renders_the_documented_damping_default")),
    echo("generator", "dynout", LiveSemanticsDiffer, 273,
         "generator.pas:3034 (GetDynOutputStr) + DynamicExp.pas:411-437 vs :441-465 (the variable index decoded as a flat (variable, slot) index)",
         CapiAndPin(26, "generator_dynout_renders_the_named_variables")),
    echo("generator", "model", EchoParse, 2,
         "generator.pas:3007-3038 (no arm 6) -> DSSObject.pas:112-115; the store is the deck's own token (:625, written before the CASE sets the live field at :643; InitPropertyValues would leave '1', :2559) while NCIM moves the LIVE GenModel 3->4 (Solution.pas:1935/:2120) and never back — ReversePQ2PV (:1743-1768) has no caller",
         Pin("generator_model_renders_the_live_pv2pq_conversion")),
    echo("generator", "shaftdata", EmptyCollectionRender, 273,
         "generator.pas:3023-3025 (arms 34/36 paren-wrap the store -> '()' when unset)",
         CapiAndPin(26, "der_user_model_arrays_render_empty_when_unset")),
    echo("generator", "userdata", EmptyCollectionRender, 273,
         "generator.pas:3023-3025 (same arm)",
         CapiAndPin(26, "der_user_model_arrays_render_empty_when_unset")),
    echo("gicsource", "spectrum", EchoDefault, 4,
         "GICsource.pas:567-578 (arms 1..3) + :327 InitPropertyValues BEFORE :332 Spectrum:='' -> PCElement.pas:119 'default' frozen",
         Capi(2)),
    echo("gictransformer", "pctperm", EchoDefault, 22,
         "GICTransformer.pas:708 ('0') -> DSSObject.pas:112-115",
         Capi(4)),
    echo("invcontrol", "lpftau", EchoDefault, 257,
         "InvControl.pas:2828 ('0.0') vs Create :1166 FLPFTau:=0.001; no arm 19 (:3232-3285)",
         CapiAndPin(87, "invcontrol_defaults_render_the_live_values")),
    echo("invcontrol", "mode", EchoDefault, 211,
         "InvControl.pas:479 (prop 2) + :2809 ('VOLTVAR') vs Create :1135 ControlMode:=NONE_MODE; arm 2 is commented out (:3234-3239)",
         CapiAndPin(23, "invcontrol_defaults_render_the_live_values")),
    echo("invcontrol", "monvoltagecalc", EchoDefault, 254,
         "InvControl.pas:505 (prop 25): no arm (:3232-3285), no init entry (:2806-2839) -> ''",
         CapiAndPin(87, "invcontrol_defaults_render_the_live_values")),
    echo("invcontrol", "pvsystemlist", EchoDefault, 257,
         "InvControl.pas:512 (prop 32): no arm, no init entry -> ''",
         CapiAndPin(87, "invcontrol_defaults_render_the_live_values")),
    echo("invcontrol", "risefalllimit", EchoDefault, 257,
         "InvControl.pas:2829 ('-1.0') vs Create :1167 FRiseFallLimit:=0.001",
         CapiAndPin(87, "invcontrol_defaults_render_the_live_values")),
    echo("invcontrol", "vsetpoint", EchoDefault, 249,
         "InvControl.pas:513 (prop 33): no arm, no init entry -> '' vs Create :1214 Fv_setpoint:=1.0",
         CapiAndPin(86, "invcontrol_defaults_render_the_live_values")),
    echo("isource", "bus2", EchoDefault, 136,
         "Isource.pas:631 ('') + no GetPropertyValue override (report 14-isource-bus2-not-stored)",
         Capi(12)),
    echo("isource", "yearly", EchoDefault, 122,
         "Isource.pas:628 ('') + :286 (daily= aliases the OBJECT, never the string) + :672-681",
         Capi(5)),
    echo("line", "cncables", EchoDefault, 77659,
         "Line.pas:1514 (prop 24, '') + :1338-1438 (no arm 24)",
         CapiAndPin(233, "line_conductors_renders_the_live_conductor_list")),
    echo("line", "conductors", EchoDefault, 77659,
         "Line.pas:1524 (prop 34, '') + :1338-1438 (no arm 34)",
         Pin("line_conductors_renders_the_live_conductor_list")),
    echo("line", "spacing", EchoParse, 1,
         "Line.pas:1511 (prop 21, '') + :1338-1438 (no arm 21); the deck's 'sp' outlives SpacingSpecified (:2266-2276)",
         Pin("line_spacing_renders_empty_once_the_spacing_is_killed")),
    echo("line", "tscables", EchoDefault, 77659,
         "Line.pas:1515 (prop 25, '') + :1338-1438 (no arm 25)",
         CapiAndPin(233, "line_conductors_renders_the_live_conductor_list")),
    echo("line", "wires", EchoDefault, 77659,
         "Line.pas:1512 (prop 22, '') + :1338-1438 (no arm 22)",
         CapiAndPin(233, "line_conductors_renders_the_live_conductor_list")),
    echo("load", "yearly", LiveSemanticsDiffer, 32548,
         "Load.pas:2346 (arm 7 answers the LIVE raw Yearlyshape string) + :807 ('' when never typed) + :657 (daily->yearly object aliasing)",
         CapiAndPin(84, "load_yearly_renders_the_resolved_loadshape_name")),
    echo("load", "zipv", EmptyCollectionRender, 49629,
         "Load.pas:2354-2357 (arm 33 loops nZIPV -> '' when 0)",
         CapiAndPin(221, "load_zipv_renders_the_live_seven_element_vector")),
    echo("monitor", "mode", EchoParse, 4,
         "Monitor.pas: no GetPropertyValue override + :359 (the raw Param stored) + :1843 ('0')",
         Capi(2)),
    echo("pvsystem", "%pminkvarmax", LiveSemanticsDiffer, 463,
         "PVsystem.pas:1153 (live arm) + :1038 (-1.0) vs the port's 0.0; both deactivate at :1397-1398",
         CapiAndPin(99, "pvsystem_and_storage_pmin_sentinels_deactivate_the_var_limits")),
    echo("pvsystem", "%pminnovars", LiveSemanticsDiffer, 463,
         "PVsystem.pas:1152 (live arm) + :1037 (-1.0) vs the port's 0.0; both deactivate at :1395-1396",
         CapiAndPin(99, "pvsystem_and_storage_pmin_sentinels_deactivate_the_var_limits")),
    echo("pvsystem", "amplimit", EchoDefault, 463,
         "PVsystem.pas:392 (prop 49): no getter arm (the CASE's ELSE is :1174), no init entry (:1074-1124) -> ''",
         CapiAndPin(99, "der_amp_limits_render_the_live_sentinel_and_gain")),
    echo("pvsystem", "amplimitgain", EchoDefault, 463,
         "PVsystem.pas:393 (prop 50), same",
         CapiAndPin(99, "der_amp_limits_render_the_live_sentinel_and_gain")),
    echo("pvsystem", "dynout", EmptyCollectionRender, 463,
         "PVsystem.pas:1171 (propDynOut -> GetDynOutputStr, '[]' when unset)",
         CapiAndPin(99, "der_user_model_arrays_render_empty_when_unset")),
    echo("pvsystem", "userdata", EmptyCollectionRender, 463,
         "PVsystem.pas:1158 (propUSERDATA paren-wraps the store -> '()' when unset)",
         CapiAndPin(99, "der_user_model_arrays_render_empty_when_unset")),
    echo("reactor", "bus2", EchoParse, 47,
         "Reactor.pas:1090-1103 (no arm 2) + :419-420 (PropertyValue[2] := GetBus(2) snapshot taken at bus1=) + :386",
         Capi(23)),
    echo("reactor", "kvar", EchoDefault, 607,
         "Reactor.pas:1113 ('1200') vs Create :585 kvarrating:=100.0; no arm 4 (:1090-1103)",
         CapiAndPin(65, "reactor_kvar_renders_the_live_rating")),
    echo("recloser", "debugtrace", EchoDefault, 230,
         "Recloser.pas:252 (prop 30) + :1553-1554 (init jumps 28 -> 31) -> ''",
         Pin("recloser_eventlog_and_debugtrace_default_to_no")),
    echo("recloser", "eventlog", EchoDefault, 230,
         "Recloser.pas:251 (prop 29) + :1553-1554 (init jumps 28 -> 31) -> ''",
         Pin("recloser_eventlog_and_debugtrace_default_to_no")),
    echo("recloser", "switchedobj", EchoDefault, 230,
         "Recloser.pas:225 (prop 3) + :1528 (''); live ElementName defaults to the monitored element (:448)",
         Pin("recloser_switchedobj_defaults_to_the_monitored_element")),
    echo("regcontrol", "fwdthreshold", EchoDefault, 888,
         "RegControl.pas:294 (prop 36) + :1444-1459 (init writes 1..32 only) + :820-827 (arm 28 only); the live kWFwdPowerThreshold IS 100 (:629)",
         Pin("regcontrol_idle_flags_and_thresholds_render_the_live_values")),
    echo("regcontrol", "idle", EchoDefault, 888,
         "RegControl.pas:291 (prop 33), never initialised (:1444-1459)",
         Pin("regcontrol_idle_flags_and_thresholds_render_the_live_values")),
    echo("regcontrol", "idleforward", EchoDefault, 888,
         "RegControl.pas:293 (prop 35), never initialised",
         Pin("regcontrol_idle_flags_and_thresholds_render_the_live_values")),
    echo("regcontrol", "idlereverse", EchoDefault, 888,
         "RegControl.pas:292 (prop 34), never initialised",
         Pin("regcontrol_idle_flags_and_thresholds_render_the_live_values")),
    echo("regcontrol", "remoteptratio", EchoDefault, 255,
         "RegControl.pas:1452 ('60') vs the live value, re-initialised from PTRatio on every ptratio= (:484)",
         Capi(31)),
    echo("regcontrol", "revthreshold", EchoDefault, 888,
         "RegControl.pas:1448 ('100') vs Create :627 kWRevPowerThreshold:=-100.0; a deck's revThreshold=800 keeps '800' while :503-505 makes the live value -800",
         Pin("regcontrol_idle_flags_and_thresholds_render_the_live_values")),
    echo("relay", "action", EchoDefault, 270,
         "Relay.pas:388 (prop 19, DEPRECATED) + :1577 ('closed'); no arm (:1366-1440)",
         Pin("relay_action_distreverse_and_reset_render_the_live_values")),
    echo("relay", "distreverse", EchoDefault, 270,
         "Relay.pas:397 (prop 38) + :1595-1596 (init jumps 37 -> 39) -> ''",
         Pin("relay_action_distreverse_and_reset_render_the_live_values")),
    echo("relay", "reset", EchoParse, 270,
         "Relay.pas:425 (prop 54) + :504 (the raw token stored) + :566-570 (arm 54 rewrites to 'n' only on yes); these decks type reset=0.20",
         Pin("relay_action_distreverse_and_reset_render_the_live_values")),
    echo("relay", "switchedobj", EchoDefault, 270,
         "Relay.pas:320 (prop 3) + :1561 (''); live ElementName defaults to the monitored element (:582)",
         Pin("relay_switchedobj_defaults_to_the_monitored_element")),
    echo("storage", "%pminkvarmax", LiveSemanticsDiffer, 460,
         "Storage.pas:1554 (live arm) + :1369 (-1.0, 'Deactivated by default') vs the port's 0.0",
         CapiAndPin(44, "pvsystem_and_storage_pmin_sentinels_deactivate_the_var_limits")),
    echo("storage", "%pminnovars", LiveSemanticsDiffer, 460,
         "Storage.pas:1553 (live arm) + :1368 (-1.0, 'Deactivated by default') vs the port's 0.0",
         CapiAndPin(44, "pvsystem_and_storage_pmin_sentinels_deactivate_the_var_limits")),
    echo("storage", "amplimit", EchoDefault, 457,
         "Storage.pas prop 60: no getter arm, no init entry (:1446-1521) -> ''",
         CapiAndPin(47, "der_amp_limits_render_the_live_sentinel_and_gain")),
    echo("storage", "amplimitgain", EchoDefault, 457,
         "Storage.pas prop 61, same",
         CapiAndPin(47, "der_amp_limits_render_the_live_sentinel_and_gain")),
    echo("storage", "dynadata", EmptyCollectionRender, 463,
         "Storage.pas:1577 (propdynaDATA paren-wraps the store -> '()' when unset)",
         CapiAndPin(47, "der_user_model_arrays_render_empty_when_unset")),
    echo("storage", "dynadll", LiveSemanticsDiffer, 2,
         "Storage.pas:1576 (DynaModel.Name) + StoreUserModel.pas:195-240 (FName is assigned only after a successful LoadLibrary)",
         CapiAndPin(2, "storage_dynadll_renders_the_typed_path")),
    echo("storage", "userdata", EmptyCollectionRender, 463,
         "Storage.pas:1574 (propUSERDATA paren-wraps the store -> '()' when unset)",
         CapiAndPin(47, "der_user_model_arrays_render_empty_when_unset")),
    echo("storagecontroller", "modedischarge", LiveSemanticsDiffer, 1,
         "StorageController.pas:1200-1214 (GetModeString has no MODESCHEDULE arm -> ELSE 'UNKNOWN') vs :2322-2333",
         CapiAndPin(1, "storagecontroller_modedischarge_renders_schedule")),
    echo("storagecontroller", "seasontargets", EmptyCollectionRender, 262,
         "StorageController.pas:1010 -> ReturnSeasonTarget(1), which exits with '' when Seasons=1 (:2445-2449)",
         CapiAndPin(20, "storagecontroller_seasontargets_render_the_live_targets")),
    echo("storagecontroller", "seasontargetslow", EmptyCollectionRender, 262,
         "StorageController.pas:1011 -> ReturnSeasonTarget(0), same",
         CapiAndPin(20, "storagecontroller_seasontargets_render_the_live_targets")),
    echo("swtcontrol", "action", EchoParse, 34,
         "SwtControl.pas:573-620 (no arm 3) + :192-193 (the raw token stored before the CASE) + :417 (Locked makes InterpretSwitchState exit)",
         Pin("swtcontrol_action_renders_the_live_switch_state")),
    echo("transformer", "bhcurrent", EmptyCollectionRender, 21164,
         "Transformer.pas:1820-1827 (arm 51 loops NumPointsBH=0 -> '[]')",
         Pin("transformer_bh_arrays_render_empty_when_unset")),
    echo("transformer", "bhflux", EmptyCollectionRender, 21164,
         "Transformer.pas:1828-1834 (arm 52, same)",
         Pin("transformer_bh_arrays_render_empty_when_unset")),
    echo("transformer", "pctperm", EchoDefault, 21161,
         "Transformer.pas:1918 ('100') + :1840-1844 (PD tail re-renders slots 1..2 only) -> DSSObject.pas:112-115",
         CapiAndPin(133, "pd_element_perm_and_repair_render_the_live_ratings")),
    echo("transformer", "repair", EchoDefault, 21161,
         "Transformer.pas:1919 ('36'), same fallthrough",
         CapiAndPin(133, "pd_element_perm_and_repair_render_the_live_ratings")),
    echo("upfc", "climit", EchoDefault, 13,
         "UPFC.pas:187 (prop 14): no arm (:1136-1153), no init entry (:1115-1131 writes 1..11) -> ''",
         Capi(9)),
    echo("upfc", "kvarlimit", EchoDefault, 6,
         "UPFC.pas:189 (prop 16), same",
         Capi(5)),
    echo("upfc", "refkv2", EchoDefault, 11,
         "UPFC.pas:188 (prop 15), same",
         Capi(7)),
    echo("upfc", "vhlimit", EchoDefault, 13,
         "UPFC.pas:185 (prop 12), same",
         Capi(9)),
    echo("upfc", "vllimit", EchoDefault, 13,
         "UPFC.pas:186 (prop 13), same",
         Capi(9)),
    echo("upfccontrol", "basefreq", EchoDefault, 13,
         "UPFCControl.pas:230-246 (Create never calls InitPropertyValues) -> the whole store stays ''",
         Capi(9)),
    echo("upfccontrol", "enabled", EchoDefault, 13,
         "UPFCControl.pas:230-246 (no InitPropertyValues) leaves FEnabledProperty 0, so CktElement.pas:1316-1320's enabled special case never fires",
         Capi(9)),
    echo("vccs", "bp1", EchoDefault, 3,
         "VCCS.pas:506 ('NONE') for prop 6 (:166); no GetPropertyValue override",
         Capi(3)),
    echo("vccs", "bp2", EchoDefault, 3,
         "VCCS.pas:507 ('NONE') for prop 7 (:167), same",
         Capi(3)),
    echo("vsource", "yearly", EchoDefault, 2,
         "Vsource.pas:1310 (prop 27, '') + :1323-1349 (no arm 27)",
         Capi(2)),
    echo("windgen", "dynout", EmptyCollectionRender, 5,
         "WindGen.pas:2906 (arm 22 -> GetDynOutputStr, '[]' when unset, :1119)",
         Pin("windgen_dynout_renders_empty_when_unset")),
];

/// Count lock, total — the same fail-on-stale equality [`NORM_ROWS`] carries.
/// **82 = the RP2.3 bucket's 86 pairs − the 5 the kill criterion re-routed, +1
/// for RP3.3's `generator.model`** (the first row a WP-RP3 sub-step landed here).
const ECHO_ROWS: usize = 82;
/// Count lock, [`EchoCategory::EchoDefault`].
const ECHO_DEFAULT_ROWS: usize = 50;
/// Count lock, [`EchoCategory::EchoParse`]; **8 since RP3.3**.
const ECHO_PARSE_ROWS: usize = 8;
/// Count lock, [`EchoCategory::EmptyCollectionRender`] — RP2.3's new category.
const ECHO_EMPTY_COLLECTION_ROWS: usize = 14;
/// Count lock, [`EchoCategory::LiveSemanticsDiffer`]; every one owes a pin.
const ECHO_LIVE_SEMANTICS_ROWS: usize = 10;

/// **Echo rows whose cited cells can never be reached live**, and why — the
/// closed exemption list [`check_echo_rows_are_live`] needs so that its
/// fail-on-stale half does not fire on a row that is doing its job.
///
/// Both pairs' echo cells sit on `engines: "capi_v0145"` cases, which plan §1.3
/// keeps uncompared on r4133 — **0 in-scope echo cells** each, measured by the
/// 2026-08-23 claims census (RP2.3 part A, finding F5). The distinction that
/// makes the exemption necessary rather than merely tidy is `fault.bus2`: the
/// PAIR does compare on r4133 (its ten `'b3.0'`/`'B3.0'` cells are claimed by a
/// `CaseFold` row), so its echo row will be *visited* and can never *hit*,
/// which is exactly the shape the guard reports as stale. `line.spacing`'s pair
/// has no in-scope cell at all, so its row simply stays dormant. Both rows are
/// needed for the offline replay, which walks the frozen census, not the gate.
const ECHO_ROWS_WITH_NO_IN_SCOPE_CELL: &[(&str, &str)] = &[("fault", "bus2"), ("line", "spacing")];

/// **The echo rows that mask cells on `engines: "r4133"` cases** — where the
/// capi channel does not run at all — as `(class, prop, cells, cases)`.
///
/// Plan mechanic (c) requires a pin for "every exclusion whose ours-value has no
/// capi witness (r4133-only classes/**cases**)". Part B2 applied the *cases*
/// half by hand, to `swtcontrol.action` only; the RP2.3 audit settlement
/// (2026-08-23) measured the whole population and made it a rule
/// ([`tests::every_row_exposed_on_r4133_only_cases_names_a_pin`]).
///
/// **Measured**, not asserted: the full claims census at HEAD
/// (`DSS_PROPS_CENSUS=claims`, 439 cases × 2 channels) crossed with each case's
/// `engines` flag in `tests/corpus/manifests/` (97 of the 439 walked cases are
/// r4133-only). 58 of the 82 rows carry such cells — 34 971 in total — and each
/// one therefore names a pin. The numbers are a dated measurement like
/// [`EchoWitness::Capi`]'s `n`; what the tests enforce is the pin obligation and
/// the count locks, so a *new* echo row on one of these pairs cannot ship with a
/// capi-only witness. A pair that is NOT listed here (`line.linecode`,
/// `upfc.*`, `vccs.*`, …) has all its masked cells on `both`/`capi_v0145` cases,
/// where the capi witness is the whole point.
///
/// **`generator.model` is RP3.3's row (2026-08-24), and it is the extreme case
/// of the rule**: *both* of its cells sit on r4133-only cases
/// (`modes:ncim/ncim_pv_pq.dss`, `modes:ncim/ncim_midi.dss` — the only corpus
/// cases in the census population that run `Set algorithm=NCIM` with a
/// generator), so a `Capi` witness could not have covered a single one. Its
/// `(2, 2)` is derived per case from the frozen extracts crossed with the
/// manifests by `props_r4133_replay::the_rp33_census_decomposition_is_read_off_the_corpus`,
/// which reads **this entry** back through [`r4133_only_exposure`] and asserts
/// both columns against its own derivation (RP3.3 audit settlement: the doc said
/// "derived" while no test read the entry, and column 4 had no value lock
/// anywhere — see [`tests::every_row_exposed_on_r4133_only_cases_names_a_pin`]
/// for what the table as a whole is held to).
#[rustfmt::skip]
const ECHO_ROWS_ON_R4133_ONLY_CASES: &[(&str, &str, u32, u32)] = &[
    ("autotrans", "bhcurrent", 2, 1),
    ("autotrans", "bhflux", 2, 1),
    ("autotrans", "pctperm", 2, 1),
    ("autotrans", "repair", 2, 1),
    ("capcontrol", "reset", 3, 1),
    ("energymeter", "action", 45, 3),
    ("fault", "pctperm", 347, 31),
    ("fuse", "switchedobj", 20, 2),
    ("generator", "d", 43, 6),
    ("generator", "dynout", 43, 6),
    ("generator", "model", 2, 2),
    ("generator", "shaftdata", 43, 6),
    ("generator", "userdata", 43, 6),
    ("invcontrol", "lpftau", 18, 13),
    ("invcontrol", "mode", 1, 1),
    ("invcontrol", "monvoltagecalc", 8, 3),
    ("invcontrol", "pvsystemlist", 18, 13),
    ("invcontrol", "risefalllimit", 18, 13),
    ("invcontrol", "vsetpoint", 18, 13),
    ("line", "cncables", 6232, 82),
    ("line", "conductors", 6232, 82),
    ("line", "tscables", 6232, 82),
    ("line", "wires", 6231, 81),
    ("load", "yearly", 32, 4),
    ("load", "zipv", 4070, 58),
    ("pvsystem", "%pminkvarmax", 61, 14),
    ("pvsystem", "%pminnovars", 61, 14),
    ("pvsystem", "amplimit", 61, 14),
    ("pvsystem", "amplimitgain", 61, 14),
    ("pvsystem", "dynout", 61, 14),
    ("pvsystem", "userdata", 61, 14),
    ("reactor", "kvar", 94, 6),
    ("recloser", "debugtrace", 206, 11),
    ("recloser", "eventlog", 176, 9),
    ("recloser", "switchedobj", 40, 2),
    ("regcontrol", "fwdthreshold", 42, 4),
    ("regcontrol", "idle", 41, 3),
    ("regcontrol", "idleforward", 42, 4),
    ("regcontrol", "idlereverse", 42, 4),
    ("regcontrol", "revthreshold", 42, 4),
    ("relay", "action", 268, 21),
    ("relay", "distreverse", 236, 19),
    ("relay", "reset", 44, 8),
    ("relay", "switchedobj", 102, 10),
    ("storage", "%pminkvarmax", 49, 5),
    ("storage", "%pminnovars", 49, 5),
    ("storage", "amplimit", 43, 4),
    ("storage", "amplimitgain", 43, 4),
    ("storage", "dynadata", 49, 5),
    ("storage", "userdata", 49, 5),
    ("storagecontroller", "seasontargets", 12, 1),
    ("storagecontroller", "seasontargetslow", 12, 1),
    ("swtcontrol", "action", 16, 1),
    ("transformer", "bhcurrent", 799, 24),
    ("transformer", "bhflux", 799, 24),
    ("transformer", "pctperm", 799, 24),
    ("transformer", "repair", 799, 24),
    ("windgen", "dynout", 5, 5),
];

/// Count lock for [`ECHO_ROWS_ON_R4133_ONLY_CASES`]: rows, and the cells behind
/// them.
const R4133_ONLY_ROWS: usize = 58;
/// Count lock, cells — the sum of the table's third column.
const R4133_ONLY_CELLS: u32 = 34971;
/// Count lock, cases — the sum of the table's **fourth** column, which carried
/// no value lock at all until the RP3.3 audit settlement (2026-08-24): the
/// per-row split of a measured exposure was free to drift as long as the row
/// count and the cell sum held, so a mutation of one row's `cases` shipped
/// green. A sum is still not a per-row proof — the rows whose split IS derived
/// tie themselves to it through [`r4133_only_exposure`] — but it makes any
/// single-row edit visible, exactly as [`R4133_ONLY_CELLS`] does for column 3.
const R4133_ONLY_CASES: u32 = 833;

/// One pair's measured exposure on `engines: "r4133"` cases —
/// `(cells, cases)` from [`ECHO_ROWS_ON_R4133_ONLY_CASES`], or `None` for a
/// pair the table does not list (all of whose masked cells are on
/// `both`/`capi_v0145` cases).
///
/// The table is data, not a claim, for most of its rows: the numbers are a dated
/// claims-census measurement. This accessor exists so that a row whose split IS
/// derived from the corpus can tie its own entry to the derivation instead of
/// asserting the tie in prose — `props_r4133_replay::
/// the_rp33_census_decomposition_is_read_off_the_corpus` does exactly that for
/// `generator.model` (RP3.3 audit settlement).
pub fn r4133_only_exposure(class: &str, prop: &str) -> Option<(u32, u32)> {
    ECHO_ROWS_ON_R4133_ONLY_CASES
        .iter()
        .find(|(c, p, _, _)| c.eq_ignore_ascii_case(class) && p.eq_ignore_ascii_case(prop))
        .map(|(_, _, cells, cases)| (*cells, *cases))
}

/// **One cell an echo row deliberately does NOT claim** — the narrowing valve
/// for a pair whose mask would otherwise be wider than its citation.
///
/// An [`EchoRow`] is pair-scoped (plan §1.2 fixes that shape), so a pair whose
/// cells are echoes *except one* would mask that one too. A carve-out names the
/// exact `(rust, r4133)` spelling the row does not cover; the cell then leaves
/// the exclusion and is claimed — or declared — by whoever really owns it
/// (`props_r4133_replay::ECHO_CARVE_OUT_ROUTING` records that owner, and a test
/// there matches the two tables both ways, so a carve-out cannot exist without
/// a named owner).
///
/// Matching is EXACT on both sides, deliberately: a carve-out is a measured
/// counterexample, never a shape heuristic, and a spelling the census has not
/// seen must stay inside the cited exclusion rather than silently fall out of
/// it.
#[derive(Debug)]
pub struct EchoCarveOut {
    /// Class, as the census spells it (matched case-insensitively).
    pub class: &'static str,
    /// Property, as the census spells it (matched case-insensitively).
    pub prop: &'static str,
    /// The port's render, exactly as the census recorded it.
    pub rust: &'static str,
    /// r4133's render, exactly as the census recorded it.
    pub oracle: &'static str,
    /// Why this cell is not the row's echo — the mechanism, cited.
    pub why: &'static str,
}

/// **The carve-outs — one, since the RP2.3 audit settlement (2026-08-23).**
///
/// `reactor.kvar`'s row cites the frozen `PropertyValue[4]` default `'1200'`
/// (`Reactor.pas:1113`), and that explains 606 of the pair's 607 census cells.
/// The 607th is not an echo at all: on `modes/makeposseq/makeposseq_shunt.dss`
/// r4133 renders its own LIVE `kvarRating`, because `TReactorObj.
/// MakePosSequence` (`Version8/Source/PDElements/Reactor.pas:1145-1201`) builds
/// the command string `Format(' kV=%-.5g kvar=%-.5g', [PhasekV, kvarPerPhase])`
/// and runs it back through `Parser[ActorID].CmdString := S; Edit(ActorID)`
/// (`:1200-1201`) — so 200/3 becomes `66.667` in the engine itself, five
/// significant digits and all. The
/// port sets the double directly (`elements/pd/reactor/solve.rs`), as does
/// dss_capi 0.14.5, whose `MakePosSequence` uses `SetDouble` and never a string
/// (`.inputs/dss_capi/src/PDElements/Reactor.pas`) — which is why the capi
/// channel compares that cell and the port matches it exactly.
///
/// So the 607th cell is a genuine ~5.0e-06 divergence of two live values, i.e.
/// RP2.4's display class — where its sibling `reactor.kv`, the other output of
/// the very same round-trip, already sits (`bins.tsv`: `reactor.kv` numeric bin
/// 6, `max_rel` 5.85e-06). Masking it under an `EchoDefault` citation would be a
/// mask wider than its evidence, so the row does not cover it. It has **no live
/// effect today**: the only deck that produces it is `engines: "capi_v0145"`
/// (`population.lock.json`), so the cell is out of r4133 scope — the point is
/// that a future r4133-gated deck with a `kvar=`-specified reactor and a
/// `MakePosSeq` will now be compared instead of silently masked.
pub const ECHO_CARVE_OUTS: &[EchoCarveOut] = &[EchoCarveOut {
    class: "reactor",
    prop: "kvar",
    rust: "66.6666666666667",
    oracle: "66.667",
    why: "Reactor.pas:1145-1201 MakePosSequence round-trips kvarPerPhase through \
          Format(' kvar=%-.5g') + Parser/Edit, so r4133's LIVE kvarRating really is 66.667 \
          (5 significant digits) — a display divergence of two live values (RP2.4's class, \
          like the same round-trip's reactor.kv), not the row's frozen '1200' echo",
}];

/// Count lock for [`ECHO_CARVE_OUTS`] — the same fail-on-stale equality every
/// other table here carries, so a second carve-out cannot appear un-reviewed.
const ECHO_CARVE_OUT_CELLS: usize = 1;

/// Index of [`PROPS_ECHO_R4133`]'s row for `(class, prop)`, case-insensitively.
fn find_echo_row(table: &[EchoRow], class: &str, prop: &str) -> Option<usize> {
    table
        .binary_search_by(|r| ci_cmp(r.class, class).then_with(|| ci_cmp(r.prop, prop)))
        .ok()
}

/// Does [`PROPS_ECHO_R4133`] hold a row for `(class, prop)` at all?
///
/// The **pair-scoped** question, which is not the same as [`echo_excluded`]'s
/// since the carve-outs landed: this one is for the tests and the accounting
/// that ask "does the table name this pair", never for deciding a cell.
pub fn has_echo_row(class: &str, prop: &str) -> bool {
    find_echo_row(PROPS_ECHO_R4133, class, prop).is_some()
}

/// Is `(class, prop)`'s cell `(rust, oracle)` value-excluded on the r4133
/// channel? — the offline twin of [`echo_excluded_r4133`], counters aside.
///
/// **Pair-scoped, minus its carve-outs.** Pair-scoped is the shape plan §1.2
/// prescribes (`SKIP_PROPS`'), and the guard against over-breadth is what every
/// row carries — the r4133 citation and the witness — plus [`ECHO_CARVE_OUTS`],
/// which takes back the one measured cell a row's citation does not explain.
/// The two values are read *only* by that lookup: for every other cell of a
/// cited pair the answer is `true` whatever they say, which is exactly what
/// makes this an exclusion and not a normalization rule.
///
/// [`skip_prop`]: super::skip_prop
pub fn echo_excluded(class: &str, prop: &str, rust: &str, oracle: &str) -> bool {
    find_echo_row(PROPS_ECHO_R4133, class, prop).is_some() && !carved_out(class, prop, rust, oracle)
}

/// Does a [`ECHO_CARVE_OUTS`] entry take this exact cell back out of its row?
fn carved_out(class: &str, prop: &str, rust: &str, oracle: &str) -> bool {
    ECHO_CARVE_OUTS.iter().any(|c| {
        c.class.eq_ignore_ascii_case(class)
            && c.prop.eq_ignore_ascii_case(prop)
            && c.rust == rust
            && c.oracle == oracle
    })
}

/// **The shipped exclusion seam**, called from `PropsPolicy::echo_excluded`'s
/// r4133 arm (and only from there — same channel gate as [`normalize_r4133`]).
///
/// It answers the same question [`echo_excluded`] does and additionally records
/// what the gate saw: a **visit** is a cell of the pair that reached the seam,
/// a **hit** is a visit whose two sides really differed, i.e. a compare this
/// row actually stopped. A carved-out cell is neither — it is not excluded, so
/// it never reaches the counters.
pub fn echo_excluded_r4133(class: &str, prop: &str, rust: &str, oracle: &str) -> bool {
    if carved_out(class, prop, rust, oracle) {
        return false;
    }
    match find_echo_row(PROPS_ECHO_R4133, class, prop) {
        Some(i) => {
            // HIT first, then VISIT. The pair is not written atomically and
            // [`assert_echo_rows_are_live`] reads it from another test thread
            // in the same binary, so the order decides which transient state a
            // concurrent read can observe: this way it can only ever see a hit
            // the visit has not caught up with (which the guard accepts), never
            // a visit whose hit has not landed yet (which it would report as
            // stale).
            if rust != oracle {
                ECHO_HITS[i].fetch_add(1, AtomicOrd::Relaxed);
            }
            ECHO_VISITS[i].fetch_add(1, AtomicOrd::Relaxed);
            record_touch(|t| &mut t.echo, rust != oracle);
            true
        }
        None => false,
    }
}

/// Per-row visit counter, indexed exactly like [`PROPS_ECHO_R4133`].
static ECHO_VISITS: [AtomicUsize; PROPS_ECHO_R4133.len()] =
    [const { AtomicUsize::new(0) }; PROPS_ECHO_R4133.len()];
/// Per-row hit counter: visits whose two sides differed, i.e. value compares
/// this row actually excluded.
static ECHO_HITS: [AtomicUsize; PROPS_ECHO_R4133.len()] =
    [const { AtomicUsize::new(0) }; PROPS_ECHO_R4133.len()];

/// One row's live `(visits, hits)` from [`NORM_VISITS`]/[`NORM_HITS`], or `None`
/// when the table has no row for the pair.
///
/// The counters are private statics with no other reader outside this module;
/// this accessor exists so that the ORDER of the two seams in
/// `compare_prop_lists` can be asserted from a test that drives the real
/// comparator (`props_policy_tests::
/// the_normalization_seam_runs_before_the_exclusion_on_a_mixed_pair`, RP2.3
/// audit settlement). Before it, the order was documented as "the mechanism, not
/// a detail" and pinned in the two OFFLINE copies of the chain only — swapping
/// the shipped lines left the whole suite green.
pub fn norm_counters(class: &str, prop: &str) -> Option<(usize, usize)> {
    let i = find_row(PROPS_NORM_R4133, class, prop)?;
    Some((
        NORM_VISITS[i].load(AtomicOrd::Relaxed),
        NORM_HITS[i].load(AtomicOrd::Relaxed),
    ))
}

/// The same, for [`ECHO_VISITS`]/[`ECHO_HITS`] and [`PROPS_ECHO_R4133`].
pub fn echo_counters(class: &str, prop: &str) -> Option<(usize, usize)> {
    let i = find_echo_row(PROPS_ECHO_R4133, class, prop)?;
    Some((
        ECHO_VISITS[i].load(AtomicOrd::Relaxed),
        ECHO_HITS[i].load(AtomicOrd::Relaxed),
    ))
}

/// **Fail-on-stale for [`PROPS_ECHO_R4133`]** — the echo table's half of plan
/// mechanic (d), modeled on [`assert_norm_rows_are_live`].
///
/// A row that was VISITED on r4133 and never excluded a differing cell is
/// masking a divergence that is no longer there: drop it, or re-measure it. It
/// is **silent when the row was never visited** and silent for the two rows the
/// census measured as having no in-scope cell at all
/// ([`ECHO_ROWS_WITH_NO_IN_SCOPE_CELL`]).
///
/// Today no real case reaches it: `corpus_gate/scheduler.rs` masks
/// `compare_all_properties` off on the r4133 channel until the RP4.1 unmask, so
/// the only counter movement in a gate run comes from unit tests in the same
/// binary that drive the comparator themselves (see
/// `props_policy_tests::an_echo_row_drops_only_its_own_value_only_on_r4133`,
/// which is written to leave `hits > 0` on every row it touches). The guard is
/// wired now so the flip arms it instead of having to remember it.
pub fn assert_echo_rows_are_live() {
    let read = |c: &[AtomicUsize]| -> Vec<usize> {
        c.iter().map(|c| c.load(AtomicOrd::Relaxed)).collect()
    };
    check_echo_rows_are_live(PROPS_ECHO_R4133, &read(&ECHO_VISITS), &read(&ECHO_HITS));
}

/// The staleness rule itself, over **injected** counters — the split exists for
/// the same reason [`check_rows_are_live`]'s does: the shipped statics cannot
/// be made stale from a test, so both directions are proven offline
/// ([`tests::the_echo_liveness_guard_is_silent_when_dormant_or_live`],
/// [`tests::the_echo_liveness_guard_fires_on_a_stale_row`]).
fn check_echo_rows_are_live(table: &[EchoRow], visits: &[usize], hits: &[usize]) {
    assert_eq!(
        (table.len(), table.len()),
        (visits.len(), hits.len()),
        "the counters are indexed exactly like the table"
    );
    for (i, r) in table.iter().enumerate() {
        let (visits, hits) = (visits[i], hits[i]);
        let dormant_by_design = ECHO_ROWS_WITH_NO_IN_SCOPE_CELL
            .iter()
            .any(|(c, p)| c.eq_ignore_ascii_case(r.class) && p.eq_ignore_ascii_case(r.prop));
        assert!(
            visits == 0 || hits > 0 || dormant_by_design,
            "stale r4133 property echo row: {}.{} ({}) excluded nothing across {visits} \
             compared cell(s). Each row stops the r4133 VALUE compare of a pair, so it must \
             name a divergence that is really there — drop it, or re-measure it \
             (DSS_PROPS_CENSUS). Cited: {}",
            r.class,
            r.prop,
            r.category.tag(),
            r.cite
        );
    }
}

/// Which link of the r4133 **value** chain claims one divergent cell.
///
/// The chain of plan §1.2 is *shape allowlist → normalization → echo table →
/// display floor → ledger*. Two of its five links are not here, and neither
/// omission is a gap:
///
/// * the **shape allowlist** is upstream of every value compare — `filter_015x`
///   drops an allowlisted prop from the walk before a value exists to diverge,
///   so on a value cell it can only ever answer "no" (the RP2.1 replay measures
///   the same structural zero, `CLAIMED_SHAPE_ALLOWLIST`). Its own population is
///   the census's `shape_count` rows, which the claims census dispositions
///   separately;
/// * the **ledger** lives in `corpus_gate/ledger.rs`, not in the harness, and is
///   per (case, channel); the claims census appends it after this answer, in the
///   documented order (i.e. only for a cell this function leaves unclaimed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueClaim {
    /// [`PROPS_NORM_R4133`] recognises the two spellings as one value.
    Normalization(NormRule),
    /// [`PROPS_ECHO_R4133`] excludes the pair's value compare (RP2.3), because
    /// the two renderings are not two spellings of one value.
    Echo,
    /// The two sides are numbers within [`R4133_DISPLAY_FLOOR`] whose r4133
    /// render is our value at the digits it printed (RP2.4).
    DisplayFloor,
}

impl ValueClaim {
    /// The disposition tag the census artifacts print (plan RP0.2's vocabulary:
    /// `normalized-by-<rule>` / `echo-row` / `under-floor`).
    pub fn tag(self) -> String {
        match self {
            ValueClaim::Normalization(rule) => format!("normalized-by-{}", rule.tag()),
            ValueClaim::Echo => "echo-row".to_string(),
            ValueClaim::DisplayFloor => "under-floor".to_string(),
        }
    }
}

/// **The value chain, resolved for one cell — first match wins.**
///
/// This is the query the census knob's disposition mode
/// (`DSS_PROPS_CENSUS=claims`, plan RP0.2/RP2.1) annotates every divergent value
/// cell with, and it is deliberately built out of the **shipped** predicates
/// ([`claiming_row`], [`echo_excluded`], [`display_floor`]) rather than a
/// re-implementation of them: RP0.2 fixed that a claims run must read the same
/// policy the live gate applies, because a drifting copy would corrupt RP4.1's
/// zero-UNCLAIMED acceptance silently.
///
/// It answers `None` for a cell no link claims — the `UNCLAIMED` bucket, which
/// is exactly the population RP2.2/RP2.3/RP2.4 still owe rows for.
///
/// It does **not** touch [`NORM_VISITS`]/[`NORM_HITS`]: those stay the exclusive
/// record of what the live gate saw, so a census run can never make a stale row
/// look live (`assert_norm_rows_are_live`).
///
/// **It takes the channel, and answers `None` for every channel but
/// [`PropsChannel::R4133`]** — plan mechanic (b) applied to the *measurement*
/// layer, not only to the comparator. The live seam is channel-gated by
/// `PropsPolicy::is_r4133`, but this query has a second caller that is not: the
/// claims census annotates the rows of **both** channels
/// (`corpus_gate/props_census.rs`, `Row::annotate`). Before the RP2.1 audit fix
/// it was channel-blind, so a capi divergence whose spelling an r4133 rule folds
/// would have been reported `normalized-by-<rule>` while the live capi
/// comparator still failed on it — and the measured "capi normalizes nothing"
/// would have been a property of today's data instead of the contract. With the
/// parameter it is the contract, pinned by
/// [`tests::the_capi_channel_claims_nothing`].
pub fn claim_value(
    channel: PropsChannel,
    class: &str,
    prop: &str,
    rust: &str,
    oracle: &str,
) -> Option<ValueClaim> {
    if channel != PropsChannel::R4133 {
        return None;
    }
    if let Some(row) = claiming_row(class, prop, rust, oracle) {
        return Some(ValueClaim::Normalization(row.rule));
    }
    if echo_excluded(class, prop, rust, oracle) {
        return Some(ValueClaim::Echo);
    }
    if under_display_floor(rust, oracle) {
        return Some(ValueClaim::DisplayFloor);
    }
    None
}

/// The engine's own proof obligations: every rule kind **value-preserving**
/// (it folds only spellings of one value) *and* **discriminating** (a real
/// difference still fails), plus the table's count locks and shape.
///
/// Every fold case below is a **real census spelling** from
/// `tests/corpus/props_r4133/examples_full.txt`, so these are not synthetic
/// exercises of the predicate — they are the population RP2.1 claims. The
/// FULL-path proofs (a corrupted value still failing through
/// `compare_prop_lists`) are RP2.1 part D's scratch-tree probes; the capi-side
/// no-op is pinned in `harness/mod.rs`, `props_policy_tests`.
#[cfg(test)]
mod tests {
    use super::*;

    /// Drive the shipped table.
    fn norm<'v>(class: &str, prop: &str, rust: &'v str, oracle: &'v str) -> (String, String) {
        let (a, e) = normalize_with(PROPS_NORM_R4133, class, prop, rust, oracle);
        (a.into_owned(), e.into_owned())
    }

    /// The echo table's process-global live accounting, summed:
    /// `(visits, hits)`.
    ///
    /// Read for a **`>=`** claim about the shipped statics, never for an
    /// equality across a test body. The counters are per-process statics shared
    /// with whatever else the binary runs, so an absolute-zero (or
    /// [`assert_norm_rows_are_live`]) assertion inside a unit test fails on
    /// test-ordering rather than on anything real — RP2.1 part D's scratch probe
    /// drove `compare_all_properties` on the r4133 channel in a harness test
    /// binary and reddened three tests here — and an equal-across-the-body
    /// assertion fails the same way as soon as a SIBLING test drives a seam
    /// concurrently (RP3.3's audit round measured that flake at ~1–3 % of runs
    /// on `the_value_chain_resolves_in_order_and_agrees_with_the_seam`). The
    /// LIVE half of the accounting is asserted where it belongs — once, at the
    /// end of the gate (`corpus_gate.rs`); "did MY query reach a seam" is asked
    /// of [`seam_touches_here`], which is per thread and therefore exact. The
    /// norm table's twin of this helper had no reader left once those three
    /// tests moved to the thread-local counter, and was removed rather than kept
    /// warm: its rows' liveness is read per row through [`norm_counters`]
    /// (`harness::props_policy_tests::
    /// the_normalization_seam_runs_before_the_exclusion_on_a_mixed_pair`).
    fn echo_counter_totals() -> (usize, usize) {
        (
            ECHO_VISITS.iter().map(|c| c.load(AtomicOrd::Relaxed)).sum(),
            ECHO_HITS.iter().map(|c| c.load(AtomicOrd::Relaxed)).sum(),
        )
    }

    /// Did the shipped table CLAIM this cell (fold it to one spelling)?
    fn claimed(class: &str, prop: &str, rust: &str, oracle: &str) -> bool {
        let (a, e) = norm(class, prop, rust, oracle);
        // A claim always answers with OUR spelling on both sides.
        if a == e {
            assert_eq!(a, rust, "{class}.{prop}: a claim must keep OUR spelling");
            true
        } else {
            assert_eq!(
                (a.as_str(), e.as_str()),
                (rust, oracle),
                "{class}.{prop}: a non-claim must hand back the RAW pair"
            );
            false
        }
    }

    // ---------------------------------------------------------------- table

    /// [`find_row`] binary-searches, so the order is a correctness invariant,
    /// and a duplicate `(class, prop)` would make which rule wins depend on the
    /// search's landing point.
    #[test]
    fn table_is_sorted_and_unique() {
        for w in PROPS_NORM_R4133.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            let ord = ci_cmp(a.class, b.class).then_with(|| ci_cmp(a.prop, b.prop));
            assert_eq!(
                ord,
                Ordering::Less,
                "PROPS_NORM_R4133 must be sorted by (class, prop) with no duplicate: \
                 {}.{} vs {}.{}",
                a.class,
                a.prop,
                b.class,
                b.prop
            );
        }
    }

    /// **The count locks** (`props_roundtrip.rs:238` pattern): equalities, both
    /// ways, per kind and in total.
    #[test]
    fn count_locks_hold() {
        let count = |want: &str| {
            PROPS_NORM_R4133
                .iter()
                .filter(|r| r.rule.tag() == want)
                .count()
        };
        assert_eq!(PROPS_NORM_R4133.len(), NORM_ROWS, "total row count moved");
        assert_eq!(count("BoolFold"), NORM_BOOL_FOLD_ROWS);
        assert_eq!(count("CaseFold"), NORM_CASE_FOLD_ROWS);
        assert_eq!(count("ArrayForm"), NORM_ARRAY_FORM_ROWS);
        assert_eq!(count("EnumSynonym"), NORM_ENUM_SYNONYM_ROWS);
        assert_eq!(
            NORM_BOOL_FOLD_ROWS
                + NORM_CASE_FOLD_ROWS
                + NORM_ARRAY_FORM_ROWS
                + NORM_ENUM_SYNONYM_ROWS,
            NORM_ROWS,
            "the per-kind locks must partition the total"
        );
        let echo = |want: EchoCategory| {
            PROPS_ECHO_R4133
                .iter()
                .filter(|r| r.category == want)
                .count()
        };
        assert_eq!(
            PROPS_ECHO_R4133.len(),
            ECHO_ROWS,
            "the echo table's row count moved"
        );
        assert_eq!(echo(EchoDefault), ECHO_DEFAULT_ROWS);
        assert_eq!(echo(EchoParse), ECHO_PARSE_ROWS);
        assert_eq!(echo(EmptyCollectionRender), ECHO_EMPTY_COLLECTION_ROWS);
        assert_eq!(echo(LiveSemanticsDiffer), ECHO_LIVE_SEMANTICS_ROWS);
        assert_eq!(
            ECHO_DEFAULT_ROWS
                + ECHO_PARSE_ROWS
                + ECHO_EMPTY_COLLECTION_ROWS
                + ECHO_LIVE_SEMANTICS_ROWS,
            ECHO_ROWS,
            "the per-category locks must partition the echo table"
        );
    }

    /// Every row's citation triple is well-formed and matches its rule: the
    /// table owns bins 1, 2, 3 and 4 and nothing else (RP2.1 landed 1/2/4, RP2.2
    /// added 3), and each bin has exactly one rule kind. (The replay
    /// cross-checks `(bin, cells)` against the vendored files themselves — this
    /// is the in-module half.)
    #[test]
    fn every_row_cites_a_bin_its_rule_owns() {
        // The documented exceptions, kept as a list so they stay countable: a
        // pair whose `bins.tsv` LABEL is one bin while the cells this row
        // claims are another's (vendored README §"A pair's bin is a label, not
        // a per-cell classification"). One from RP2.2 — the live enum getter
        // behind a bin-2 label (`VOLTAGE_CURVEX_REF_SYNONYMS`) — and the nine
        // RP2.3 landed on bin-5 pairs so that the echo table cannot mask a cell
        // a typed rule still compares (module doc §"RP2.3's nine off-bin
        // rows"). Anything else must match its bin.
        const RULE_ON_AN_OFF_BIN_PAIR: &[(&str, &str, u8, &str)] = &[
            ("generator", "userdata", 5, "ArrayForm"),
            ("invcontrol", "monvoltagecalc", 5, "CaseFold"),
            ("invcontrol", "voltage_curvex_ref", 2, "EnumSynonym"),
            ("line", "wires", 5, "ArrayForm"),
            ("load", "yearly", 5, "CaseFold"),
            ("load", "zipv", 5, "ArrayForm"),
            ("reactor", "bus2", 5, "CaseFold"),
            ("storage", "dynadata", 5, "ArrayForm"),
            ("storagecontroller", "seasontargets", 5, "ArrayForm"),
            ("storagecontroller", "seasontargetslow", 5, "ArrayForm"),
        ];
        let mut exceptions = 0;
        for r in PROPS_NORM_R4133 {
            if let Some((_, _, bin, rule)) = RULE_ON_AN_OFF_BIN_PAIR
                .iter()
                .find(|(c, p, _, _)| *c == r.class && *p == r.prop)
            {
                assert_eq!(
                    (r.rule.tag(), r.bin),
                    (*rule, *bin),
                    "{}.{}: the off-bin exception list says {rule} on bin {bin}",
                    r.class,
                    r.prop
                );
                exceptions += 1;
                continue;
            }
            let want = match r.bin {
                1 => "BoolFold",
                2 => "CaseFold",
                3 => "EnumSynonym",
                4 => "ArrayForm",
                other => panic!(
                    "{}.{}: bin {other} is not this table's — bins 5/6/7 are \
                     RP2.3's/RP2.4's/RP3's",
                    r.class, r.prop
                ),
            };
            assert_eq!(
                r.rule.tag(),
                want,
                "{}.{}: bin {} must carry {want}",
                r.class,
                r.prop,
                r.bin
            );
            assert!(
                r.cells > 0,
                "{}.{}: a cited pair has cells",
                r.class,
                r.prop
            );
            assert!(
                r.class.chars().all(|c| !c.is_ascii_uppercase())
                    && r.prop.chars().all(|c| !c.is_ascii_uppercase()),
                "{}.{}: rows are spelled as the census spells a pair (lowercase)",
                r.class,
                r.prop
            );
        }
        assert_eq!(
            exceptions,
            RULE_ON_AN_OFF_BIN_PAIR.len(),
            "an exception row that is not in the table any more — drop it from the list \
             instead of leaving the bin/rule pin loosened for a pair that is gone"
        );
    }

    /// The rows RP2.1 deliberately does **not** hold, and the sub-step that
    /// owns each. If a later pass adds one of these here, the exclusion
    /// argument in the module doc has to be rewritten first.
    #[test]
    fn the_pairs_routed_elsewhere_have_no_row() {
        // Bin 1, pure echo — no foldable cell exists (vendored README
        // §"Bin 1 carries nine echo pairs, not three") → RP2.3, wholesale.
        for (class, prop) in [
            ("capcontrol", "reset"),
            ("recloser", "debugtrace"),
            ("regcontrol", "idleforward"),
            ("regcontrol", "idlereverse"),
            ("upfccontrol", "enabled"),
        ] {
            assert!(
                find_row(PROPS_NORM_R4133, class, prop).is_none(),
                "{class}.{prop} is a pure-echo bin-1 pair: RP2.3's row, not a BoolFold row"
            );
        }
        // Bin 4, unclaimable by any typed rule → RP2.3 / RP3.7 (RP2.2's routing)
        // or out of scope (plan §1.3).
        for (class, prop) in [
            ("expcontrol", "derlist"),
            ("relay", "normal"),
            ("relay", "state"),
            ("sensor", "kvs"),
        ] {
            assert!(
                find_row(PROPS_NORM_R4133, class, prop).is_none(),
                "{class}.{prop} takes no ArrayForm row (module doc: RP2.3 / RP3.7 / out of scope)"
            );
        }
        // Bin 3, read by RP2.2 and routed AWAY from this table: an echo of the
        // deck's own text, a non-injective `'UNKNOWN'`, or a live-state
        // divergence — none of them a spelling of the same value (module doc
        // §"RP2.2's routing of bin 3"). Adding an `EnumSynonym` row for any of
        // these would be the value-preservation violation (c) forbids.
        for (class, prop) in [
            ("swtcontrol", "action"),
            ("monitor", "mode"),
            ("storagecontroller", "modedischarge"),
            ("line", "units"),
        ] {
            assert!(
                find_row(PROPS_NORM_R4133, class, prop).is_none(),
                "{class}.{prop} is a bin-3 pair RP2.2 routed elsewhere, not a synonym"
            );
        }
    }

    /// A `(class, prop)` with no row is never touched — the table is the whole
    /// mechanism, and the class/prop lookup is case-insensitive because the
    /// capture spells them however the engine does.
    #[test]
    fn unknown_pairs_pass_through_and_lookup_ignores_case() {
        assert_eq!(
            norm("Foo", "Bar", "Yes", "true"),
            ("Yes".to_string(), "true".to_string()),
            "a pair with no row must reach the assert raw"
        );
        assert!(claimed("Capacitor", "ENABLED", "Yes", "true"));
        assert!(claimed("cApAcItOr", "enabled", "Yes", "true"));
    }

    // ------------------------------------------------------------- BoolFold

    /// Real bin-1 spellings fold; `''` is not a boolean; a boolean that says
    /// the OPPOSITE still fails.
    #[test]
    fn boolfold_folds_spellings_and_only_spellings() {
        // Census spellings (examples_full.txt).
        assert!(claimed("capacitor", "enabled", "Yes", "true"));
        assert!(claimed("monitor", "ppolar", "Yes", "YES"));
        assert!(claimed("monitor", "ppolar", "No", "no"));
        assert!(claimed("monitor", "ppolar", "No", "NO"));
        assert!(claimed("autotrans", "xrconst", "No", "NO"));
        assert!(claimed("generator", "debugtrace", "No", "no"));
        assert!(claimed("gendispatcher", "enabled", "Yes", "true"));
        assert!(claimed("windgen", "enabled", "Yes", "true"));
        assert!(claimed("sensor", "enabled", "Yes", "true"));
        // The single-letter Delphi spellings of plan §1.1 bin 1.
        assert!(claimed("relay", "eventlog", "Yes", "y"));
        assert!(claimed("relay", "eventlog", "No", "n"));
        assert!(claimed("relay", "eventlog", "Yes", "Y"));
        // DISCRIMINATION: the opposite boolean must still fail.
        assert!(!claimed("capacitor", "enabled", "Yes", "false"));
        assert!(!claimed("capacitor", "enabled", "No", "true"));
        assert!(!claimed("monitor", "ppolar", "Yes", "NO"));
        // `''` is NOT a boolean — it is the bin-5 echo RP2.3 owns. These four
        // pairs are the census's genuinely MIXED bin-1 pairs: their foldable
        // cells fold, their echo cells do not.
        assert!(claimed("recloser", "eventlog", "Yes", "true"));
        assert!(!claimed("recloser", "eventlog", "No", ""));
        assert!(claimed("regcontrol", "idle", "No", "false"));
        assert!(!claimed("regcontrol", "idle", "No", ""));
        assert!(!claimed("relay", "distreverse", "No", ""));
        // ...including the non-empty stale parse string of `relay.reset`.
        assert!(!claimed("relay", "reset", "No", "0.20"));
        // Not a boolean spelling at all.
        assert!(!claimed("capacitor", "enabled", "Yes", "1"));
        assert!(!claimed("capacitor", "enabled", "Yes", "tru"));
        assert_eq!(fold_bool(""), None, "'' is not a boolean");
        assert_eq!(fold_bool("   "), None, "blank is not a boolean");
    }

    /// **The accepted set is CLOSED** — the guard the per-kind fold tests above
    /// do not give, and the RP2.1 audit round's one major finding.
    ///
    /// Those tests are sample-based: real census spellings on the accept side, a
    /// handful of hand-picked negatives on the refuse side. Nothing in them says
    /// the accepted set has a *boundary*, so a widening that no sample happens to
    /// touch is invisible — measured, not supposed: adding `"on"`/`"off"` to
    /// [`fold_bool`]'s two lists left the whole suite green.
    ///
    /// That matters because `fold_bool` is where "spelling, never value" is
    /// decided for 77 rows and ~294 500 cells. `on`/`off` is the sharp case: it
    /// is not one of the eleven Delphi boolean spellings r4133's getters print
    /// (vendored `README.md` §"Bin 1 carries nine echo pairs, not three", and
    /// `props_r4133_replay::BOOL_SPELLINGS`), so admitting it would let the table
    /// fold a rendering neither engine produces for a boolean — and, in a mixed
    /// bin-1 pair, could swallow a `PropertyValue[]` echo that RP2.3 must
    /// exclude with a citation instead.
    ///
    /// So the universe below is walked EXHAUSTIVELY: every string in it is
    /// asserted against its intended verdict, accept and refuse alike.
    #[test]
    fn boolfold_accepts_a_closed_set_of_spellings() {
        // The accepted vocabulary, as the doc on `NormRule::BoolFold` states it:
        // {yes, y, true} / {no, n, false}, case-insensitive, outer trim.
        const TRUE_TOKENS: &[&str] = &["yes", "y", "true"];
        const FALSE_TOKENS: &[&str] = &["no", "n", "false"];
        // Everything else a getter, a parse store or a well-meaning widening
        // might offer. NONE of these is a boolean spelling.
        const NOT_BOOLEAN: &[&str] = &[
            "", "   ", "on", "ON", "On", "off", "OFF", "Off", "1", "0", "-1", "2", "t", "f", "T",
            "F", "tru", "truee", "yess", "ye", "nope", "none", "null", "nil", "enabled",
            "disabled", "y n", "yes no", "y.", "-y", "*", "0.20", "100",
        ];

        let variants = |t: &str| {
            [
                t.to_string(),
                t.to_uppercase(),
                // Title case, and the outer whitespace the rule trims.
                format!("{}{}", t[..1].to_uppercase(), &t[1..]),
                format!("  {t}"),
                format!("{t}\t"),
                format!(" {} ", t.to_uppercase()),
            ]
        };
        for t in TRUE_TOKENS {
            for v in variants(t) {
                assert_eq!(fold_bool(&v), Some(true), "{v:?} spells TRUE");
            }
        }
        for t in FALSE_TOKENS {
            for v in variants(t) {
                assert_eq!(fold_bool(&v), Some(false), "{v:?} spells FALSE");
            }
        }
        for s in NOT_BOOLEAN {
            assert_eq!(
                fold_bool(s),
                None,
                "{s:?} is NOT a boolean spelling — admitting it would let BoolFold \
                 equate two different values, which is the one thing a rule may never do"
            );
        }

        // The eleven spellings the census actually measured on the r4133 side of
        // a bin-1 pair must all be inside the accepted set (the population this
        // kind exists for), and each must fold against OUR `Yes`/`No`.
        for (spelling, want) in [
            ("true", true),
            ("True", true),
            ("YES", true),
            ("yes", true),
            ("y", true),
            ("Y", true),
            ("false", false),
            ("False", false),
            ("no", false),
            ("NO", false),
            ("n", false),
        ] {
            assert_eq!(
                fold_bool(spelling),
                Some(want),
                "census spelling {spelling:?}"
            );
            let ours = if want { "Yes" } else { "No" };
            assert!(
                BoolFold.claims(ours, spelling),
                "{ours} vs {spelling:?} is a census cell this kind must fold"
            );
            let opposite = if want { "No" } else { "Yes" };
            assert!(
                !BoolFold.claims(opposite, spelling),
                "{opposite} vs {spelling:?} is the OPPOSITE value"
            );
        }
    }

    // ------------------------------------------------------------- CaseFold

    /// Case-only differences fold (both engines resolve identifiers through a
    /// lowercasing `THashList`), the two upstream trailing blanks trim, and a
    /// different spelling of a different ordinal still fails.
    #[test]
    fn casefold_folds_case_and_the_two_trailing_blanks() {
        assert!(claimed("transformer", "xfmrcode", "ct25", "CT25"));
        assert!(claimed("storage", "state", "Idling", "IDLING"));
        assert!(claimed("capcontrol", "type", "Voltage", "voltage"));
        assert!(claimed(
            "invcontrol",
            "voltwattyaxis",
            "PAvailablePU",
            "PAVAILABLEPU"
        ));
        assert!(claimed("generator", "status", "Variable", "variable"));
        assert!(claimed("generator", "dynamiceq", "mydiffeq", "myDiffEq"));
        // The trailing-blank rows: Transformer.pas:1762-1763 /
        // AutoTrans.pas:1818-1819.
        assert!(claimed("transformer", "conn", "wye", "wye "));
        assert!(claimed("transformer", "conn", "delta", "Delta "));
        assert!(claimed("autotrans", "conn", "wye", "wye "));
        // DISCRIMINATION: a genuinely different spelling is NOT a case fold —
        // these are bin-3 cells inside a bin-2 pair (vendored README §"A pair's
        // bin is a label"). RP2.2 read all three getters: `capcontrol.type` and
        // `fault.bus2` are `PropertyValue[]` echoes and go to RP2.3
        // (`props_r4133_replay::RP22_ROUTING`), so they must still reach the
        // assert raw HERE; `invcontrol.voltage_curvex_ref` is a live enum
        // rendering and its whole pair moved to an `EnumSynonym` row, which is
        // why it is asserted in `enumsynonym_folds_the_shipped_scan_and_...`
        // rather than refused here.
        assert!(!claimed("capcontrol", "type", "PowerFactor", "pf"));
        assert!(!claimed("capcontrol", "type", "Voltage", "volt"));
        // ...and neither is an echo, nor a different bus/node spec.
        assert!(!claimed("relay", "switchedobj", "Line.thev", ""));
        assert!(!claimed("fault", "bus2", "b2.0", "b2.0.0.0"));
        assert!(!claimed("isource", "bus1", "b2", "b2.1"));
        // Trim is OUTER whitespace only: an inner difference still fails.
        assert!(!claimed("transformer", "xfmrcode", "ct 25", "ct25"));
    }

    /// **`CaseFold` may drop CASE and OUTER BLANKS — never a character.**
    ///
    /// The companion of [`boolfold_accepts_a_closed_set_of_spellings`], and the
    /// second half of the RP2.1 audit round's major finding: a
    /// character-dropping widening of this predicate passes every other test in
    /// the tree. Measured — `rust.trim().trim_start_matches('-')` on both sides
    /// left the whole suite green.
    ///
    /// The live cost of that particular widening is exact and large:
    /// `regcontrol.revthreshold` is literally `'-100'` (ours) against `'100'`
    /// (r4133) over **888 cells** — an `EchoDefault` frozen at
    /// `Version8/Source/Controls/RegControl.pas:1448`, vendored in
    /// `examples_supplement.txt` and owed a cited exclusion row plus a pin by
    /// RP2.3. A sign-blind `CaseFold` would fold exactly that class of real
    /// divergence into silence.
    ///
    /// The pin is written on the **predicate**, not on a table row, so it holds
    /// for every present and future `CaseFold` pair.
    #[test]
    fn casefold_never_drops_a_character() {
        // Accept: differences that are case and/or OUTER whitespace only.
        for (a, b) in [
            ("ct25", "CT25"),
            ("wye", "wye "),
            ("delta", "Delta "),
            (" idling", "IDLING "),
            ("-100", "-100 "),
            ("mydiffeq", "myDiffEq"),
        ] {
            assert!(
                CaseFold.claims(a, b),
                "{a:?} vs {b:?} differs only in case/trim"
            );
        }
        // Refuse: every transform that changes WHICH value the string denotes.
        for (a, b, what) in [
            (
                "-100",
                "100",
                "a dropped sign — regcontrol.revthreshold, 888 cells",
            ),
            ("100", "-100", "a dropped sign, the other way"),
            ("-800", "800", "the pair's second spelling"),
            ("+5", "5", "a dropped plus"),
            ("007", "7", "dropped leading zeros"),
            ("100", "1000", "a dropped digit"),
            ("ct 25", "ct25", "dropped INNER whitespace"),
            ("b2.1", "b21", "a dropped separator"),
            ("b2.0", "b2.0.0.0", "dropped node references"),
            ("line.thev", "thev", "a dropped class prefix"),
            ("", "0", "an empty render against a value"),
            ("0", "", "…and the mirror"),
            (
                "0.20",
                "0.2",
                "a numeric re-render — CaseFold is not a number rule",
            ),
        ] {
            assert!(
                !CaseFold.claims(a, b),
                "CaseFold claimed {a:?} vs {b:?} ({what}) — it may fold CASE and OUTER \
                 blanks and nothing else; a rule may change how a value is SPELLED, \
                 never WHICH value it is"
            );
        }
    }

    // ------------------------------------------------------------ ArrayForm

    /// Bracket/paren/comma forms fold token for token; the token COUNT and
    /// every token's value are preserved.
    #[test]
    fn arrayform_folds_delimiters_not_contents() {
        // Census spellings: `[ … ]` vs comma, paren and bare forms.
        assert!(claimed("line", "ratings", "[ 400]", "[400,]"));
        assert!(claimed("line", "ratings", "[ 600 700]", "[600,700,]"));
        assert!(claimed(
            "recloser",
            "recloseintervals",
            "[ 0.5 2 2]",
            "(0.5, 2, 2, )"
        ));
        assert!(claimed("energymeter", "option", "[E, R, C]", "(E, R, C)"));
        assert!(claimed(
            "energymeter",
            "peakcurrent",
            "[ 400 400 400]",
            "((400, 400, 400))"
        ));
        assert!(claimed(
            "invcontrol",
            "monbus",
            "[A.1, A.2, A.3]",
            "A.1 A.2 A.3"
        ));
        // Numbers compare by VALUE, so a different rendering of the same f64
        // folds — the census carries both of these.
        assert!(claimed(
            "transformer",
            "xscarray",
            "[ 0.00001]",
            "[1E-005, ]"
        ));
        assert!(claimed("sensor", "kvars", "[ 0 0 0]", "[0.0, 0.0, 0.0]"));
        // Non-numeric tokens fold case-insensitively.
        assert!(claimed("swtcontrol", "normal", "closed", "[CLOSED, ]"));
        // The two S6 pairs this table holds, BOTH directions, exactly as the
        // census measured them (module doc §"Four rows this table DOES hold sit
        // on RP2.2's S6 list"): the ONE-token spelling folds — one cell of 59 on
        // each pair — and the three-element per-phase render does not, so RP2.2
        // still owns the per-phase question on all four S6 array pairs.
        assert!(claimed("swtcontrol", "state", "closed", "[closed, ]"));
        assert!(!claimed(
            "swtcontrol",
            "normal",
            "closed",
            "[closed, closed, closed, ]"
        ));
        assert!(!claimed(
            "swtcontrol",
            "state",
            "open",
            "[open, open, open, ]"
        ));
        // Its twins `relay.normal`/`relay.state` carry no row at all, so the
        // same shape reaches the assert raw there.
        assert!(!claimed(
            "relay",
            "normal",
            "[closed, closed, closed, ]",
            "[closed, ]"
        ));
        // DISCRIMINATION 1 — a corrupted token.
        assert!(!claimed("energymeter", "option", "[E, R, C]", "(E, X, C)"));
        assert!(!claimed("swtcontrol", "state", "closed", "[open, ]"));
        // DISCRIMINATION 2 — a wrong number, at any magnitude. "Wrong" means
        // outside `R4133_DISPLAY_FLOOR` **or** not a render of our value at the
        // precision the oracle printed: `ArrayForm` compares its numeric
        // elements through [`numbers_match`], which the floor widens under both
        // clauses. The first two gaps are 2.5e-3 and 2.5e-3; the third is
        // 1.0e-5, INSIDE the floor, and is refused by the mechanism clause
        // alone — `700.007` is not `700` rounded to anything (RP2.4 audit
        // settlement; this row was RP2.1's discrimination and the widening
        // briefly took it).
        assert!(!claimed("line", "ratings", "[ 400]", "[401,]"));
        assert!(!claimed("line", "ratings", "[ 600 700]", "[600,701.75,]"));
        assert!(!claimed("line", "ratings", "[ 600 700]", "[600,700.007,]"));
        // …while the display shape it must still fold is the other direction:
        // OUR full value against the oracle's shorter render of it, 4.3e-8
        // apart. This is the one that dies if the floor is dropped from
        // `tokens_match` (the ArrayForm half of RP2.4's wiring).
        assert!(claimed("line", "ratings", "[ 600 700.00003]", "[600,700,]"));
        assert!(!claimed(
            "recloser",
            "recloseintervals",
            "[ 0.5 2 2]",
            "(0.5, 2, 2.02, )"
        ));
        // DISCRIMINATION 3 — a token-count difference is a VALUE difference:
        // these are the census's own unclaimed rows (a one-element array
        // against r4133's frozen three-element default, and the per-phase
        // render RP2.2 owns).
        assert!(!claimed(
            "energymeter",
            "peakcurrent",
            "[ 400]",
            "((400, 400, 400))"
        ));
        assert!(!claimed(
            "swtcontrol",
            "state",
            "closed",
            "[closed, closed, closed, ]"
        ));
        assert!(!claimed("sensor", "kws", "[ 0]", "[0.0, 0.0, 0.0]"));
        // DISCRIMINATION 4 — an empty render is not an array: `''` vs `[]` is
        // bin 5's echo, which this table must never claim.
        assert!(!claimed("line", "ratings", "", "[400,]"));
        assert!(!claimed("line", "ratings", "[ 400]", ""));
        assert!(!array_forms_match("[]", "()"));
        assert!(!array_forms_match("", ""));
        // The display floor, at the ONE function it widens — the deliberate
        // successor of RP2.1's `R4133_DISPLAY_FLOOR.is_none()` guard, which
        // said only that the slot was empty. This says what is IN it, both
        // ways: the literal derived value, and the predicate's two sides of
        // the boundary. `R4133_DISPLAY_FLOOR`'s doc carries the derivation.
        assert_eq!(
            R4133_DISPLAY_FLOOR,
            Some(2e-4),
            "RP2.4's derived floor: 3.110x above the worst display cell \
             (load.pf, 6.431124e-05) and 6.874x under the nearest row above the \
             empty band (storagecontroller.kwneed, 1.374769e-03)"
        );
        // The boundary, both ways, in the shape the mechanism produces: OUR
        // long value against the oracle's shorter render of it (1000.19 printed
        // to 4 digits IS '1000'), so only the floor decides.
        assert!(numbers_match(1000.19, 1000.0, "1000"), "1.8996e-4 — inside");
        assert!(
            !numbers_match(1000.21, 1000.0, "1000"),
            "2.0996e-4 — outside"
        );
        assert!(numbers_match(1e-5, 0.00001, "0.00001"));
        // …and the shapes no floor may swallow, at the same function.
        assert!(!numbers_match(0.001, 0.0, "0.0"), "0 vs non-zero is rel 1");
        assert!(!numbers_match(4.0, 3.0, "3"));
        // Inside the floor (1.9e-5) and still refused: '0.100019' is not
        // '0.1' rounded to anything — six printed digits put the whole gap
        // 38x outside what that render could produce.
        assert!(!numbers_match(0.1, 0.100019, "0.100019"));
        // …and a count that differs by one is not a render either, at any
        // magnitude a floor would otherwise swallow (5001 vs 5000 is 2.0e-4).
        assert!(!numbers_match(5001.0, 5000.0, "5000"));
        assert!(
            !numbers_match(f64::INFINITY, f64::MAX, "1.7976931348623157E308"),
            "a non-finite side only ever matches an equal one"
        );
        assert!(numbers_match(f64::INFINITY, f64::INFINITY, "inf"));
        assert!(!numbers_match(f64::NAN, f64::NAN, "nan"));
    }

    /// **The separator set is CLOSED** — the `ArrayForm` half of the
    /// closed-set discipline (see [`boolfold_accepts_a_closed_set_of_spellings`]
    /// for why sample-based accept tests are not enough).
    ///
    /// `[`, `]`, `(`, `)`, `,` and whitespace are the separators the two engines
    /// really print — dss_capi `GetDSSArray`'s `'[' + ' %g'×n + ']'`
    /// (`.inputs/dss_capi/src/Common/Utilities.pas:1529-1552`) against r4133's
    /// comma, paren and bare forms. Anything else is part of a token, so a
    /// render that separates with something else has a DIFFERENT token count and
    /// must not fold.
    #[test]
    fn arrayform_separators_are_a_closed_set() {
        for sep in [';', ':', '|', '/', '{', '}', '-'] {
            let theirs = format!("[400{sep}400]");
            assert!(
                !array_forms_match("[ 400 400]", &theirs),
                "{sep:?} is not one of the separators either engine prints; treating it as \
                 one would silently re-tokenize a value"
            );
        }
        // …and the separators that ARE in the set all collapse to the same
        // token stream, in every combination the census shows.
        for form in [
            "[400, 400]",
            "(400, 400)",
            "400 400",
            "[ 400 400]",
            "((400, 400))",
            "[400,400,]",
        ] {
            assert!(
                array_forms_match("[ 400 400]", form),
                "{form:?} is a two-token render of the same value"
            );
        }
        assert_eq!(
            array_tokens("[ 400 400]").collect::<Vec<_>>(),
            ["400", "400"]
        );
        assert_eq!(array_tokens("a;b").collect::<Vec<_>>(), ["a;b"]);
    }

    // ---------------------------------------------------------- EnumSynonym

    /// The kind's mechanics, pinned against an **injected** map (the
    /// `EVENTLOG_MASKS` self-test pattern): an explicit, closed, directional
    /// `(ours, theirs)` list — no wildcard, no derived synonym, and the reverse
    /// direction is not implied.
    #[test]
    fn enumsynonym_claims_exactly_its_mapped_pairs() {
        const MAP: &[(&str, &str)] = &[("Positive", "Pos"), ("Zero", "zero-seq")];
        const T: &[NormRow] = &[row(
            "Vsource",
            "scantype",
            EnumSynonym(MAP),
            3,
            1,
            Evidence::BinsTsv,
        )];
        let claim = |rust: &str, oracle: &str| {
            let (a, e) = normalize_with(T, "vsource", "ScanType", rust, oracle);
            a == e
        };
        assert!(claim("Positive", "Pos"));
        assert!(claim("Positive", "POS"), "the map is case-insensitive");
        assert!(claim("Zero", "zero-seq"));
        // DISCRIMINATION: unmapped, mismatched, and reversed all fail.
        assert!(!claim("Positive", "Neg"));
        assert!(!claim("Negative", "Pos"));
        assert!(!claim("Zero", "Pos"));
        assert!(!claim("Pos", "Positive"), "the map is directional");
    }

    /// **The five shipped rows fold exactly their cited census spellings, and
    /// nothing else** (RP2.2). The census population, pair by pair
    /// (`examples_full.txt`): `vsource.scantype`/`sequence` `'Positive'` vs
    /// `'Pos'` (2 042 cells each), `isource.scantype` `'Positive'`/`'pos'` 136
    /// and `'Zero'`/`'zero'` 1, `isource.sequence` `'Positive'`/`'pos'` 118 and
    /// `'Negative'`/`'neg'` 19.
    #[test]
    fn enumsynonym_folds_the_shipped_scan_and_sequence_spellings() {
        // Accept — every census cell of all four pairs.
        assert!(claimed("vsource", "scantype", "Positive", "Pos"));
        assert!(claimed("vsource", "sequence", "Positive", "Pos"));
        assert!(claimed("isource", "scantype", "Positive", "pos"));
        assert!(claimed("isource", "scantype", "Zero", "zero"));
        assert!(claimed("isource", "sequence", "Positive", "pos"));
        assert!(claimed("isource", "sequence", "Negative", "neg"));
        // …and the frozen `InitPropertyValues` default in either casing, on
        // either class (`Vsource.pas:1300-1301` `'Pos'`, `Isource.pas:626-627`
        // `'pos'`).
        assert!(claimed("Vsource", "ScanType", "Positive", "POS"));
        assert!(claimed("ISOURCE", "SEQUENCE", "Positive", "Pos"));

        // The fifth row — the live `voltage_curvex_ref` getter
        // (`InvControl.pas:3244-3249`): all three census spellings, the two
        // case-only ones RP2.1's `CaseFold` row used to claim included.
        assert!(claimed(
            "invcontrol",
            "voltage_curvex_ref",
            "Rated",
            "rated"
        ));
        assert!(claimed("invcontrol", "voltage_curvex_ref", "Avg", "avg"));
        assert!(claimed(
            "invcontrol",
            "voltage_curvex_ref",
            "RAvg",
            "avgrated"
        ));
        // …and it is NARROWER than the CaseFold row it replaced on everything
        // outside the three mapped ordinals: a case-only difference there no
        // longer folds. (It is WIDER by the `'RAvg'`/`'avgrated'` pair just
        // above — the two predicates are incomparable, see the map's doc.)
        assert!(!claimed("invcontrol", "voltage_curvex_ref", "Ravg", "avg"));
        assert!(!claimed("invcontrol", "voltage_curvex_ref", "Rated", "avg"));
        assert!(!claimed("invcontrol", "voltage_curvex_ref", "Vref", "vref"));

        // DISCRIMINATION 1 — a DIFFERENT ordinal never folds, in either
        // direction. These are the value errors the rule exists to still catch.
        assert!(!claimed("vsource", "scantype", "Zero", "Pos"));
        assert!(!claimed("vsource", "scantype", "Positive", "Zero"));
        assert!(!claimed("isource", "sequence", "Negative", "Pos"));
        assert!(!claimed("isource", "sequence", "Positive", "Neg"));
        assert!(!claimed("isource", "scantype", "Zero", "pos"));

        // DISCRIMINATION 2 — the map is directional: r4133's token on our side
        // is not a claim.
        assert!(!claimed("vsource", "scantype", "Pos", "Positive"));

        // DISCRIMINATION 3 — the two properties do NOT share a registry, and the
        // maps must not be interchangeable. `ScanType`'s `-1` is `'None'`
        // (`registry/solution.rs:22-29`), `Sequence`'s is `'Negative'`
        // (`:31-38`), so `'Negative'` is not even a `scantype` rendering.
        assert!(!claimed("vsource", "scantype", "Negative", "Neg"));
        assert!(!claimed("isource", "scantype", "Negative", "neg"));
        assert!(!claimed("vsource", "sequence", "None", "Neg"));

        // DISCRIMINATION 4 — near misses and non-tokens. Fail-closed is the
        // documented behavior: an unmeasured spelling reaches the assert raw.
        for oracle in ["", "P", "Positiv", "Positively", "1", "posx", "Po s"] {
            assert!(
                !claimed("vsource", "scantype", "Positive", oracle),
                "{oracle:?} is outside the closed set and must reach the assert raw"
            );
        }

        // …and the rule may not leak onto a pair with no row: `gicline` speaks
        // the same two enums in the engine but has no property for them.
        assert!(!claimed("gicline", "scantype", "Positive", "Pos"));
    }

    /// **Every shipped `EnumSynonym` map is injective in BOTH directions, and
    /// every shipped map's contents are pinned literally.**
    ///
    /// Two structural holes, either of which lets the rule equate two different
    /// values — the one thing a rule may never do — and neither of which a
    /// sample-based accept/refuse test can see:
    ///
    /// * one r4133 token mapping to two of our spellings (`("A","x")` +
    ///   `("B","x")`): two of our values fold onto one upstream render;
    /// * one of **our** spellings mapping to two r4133 tokens (`("A","x")` +
    ///   `("A","y")`): our single value folds against two upstream renders, so
    ///   if `x` and `y` denote different upstream values one of the two folds
    ///   masks a real disagreement. Added by the RP2.2 audit settlement
    ///   (2026-08-23) — the first form checked only the first direction while
    ///   the doc promised "a future row cannot introduce it".
    ///
    /// The literal `assert_eq!`s below are the second half of the guarantee.
    /// Injectivity plus the accept/refuse test only constrain entries that fold
    /// *something in today's frozen population*; an entry that folds nothing
    /// today — the RP2.1 audit's major finding, one level down — changes no
    /// other test and would ship green. Pinning each map's exact contents is
    /// what makes a widening entry a reviewed diff. All three maps are pinned,
    /// and [`NORM_ENUM_SYNONYM_ROWS`] closes the set of rows that may carry one.
    #[test]
    fn enumsynonym_maps_are_injective() {
        let mut rows = 0;
        for r in PROPS_NORM_R4133 {
            let EnumSynonym(map) = r.rule else { continue };
            rows += 1;
            assert!(
                !map.is_empty(),
                "{}.{}: an EnumSynonym row with an empty map folds nothing",
                r.class,
                r.prop
            );
            for (i, (ours, theirs)) in map.iter().enumerate() {
                assert!(
                    !ours.trim().is_empty() && !theirs.trim().is_empty(),
                    "{}.{}: '' is a bin-5 echo, never an enum spelling",
                    r.class,
                    r.prop
                );
                for (mine2, theirs2) in &map[i + 1..] {
                    assert!(
                        !(theirs.eq_ignore_ascii_case(theirs2)
                            && !ours.eq_ignore_ascii_case(mine2)),
                        "{}.{}: r4133's {theirs:?} maps to BOTH {ours:?} and {mine2:?} — a \
                         non-injective map equates two different values",
                        r.class,
                        r.prop
                    );
                    // The mirror, equally value-destroying: our one spelling
                    // folding against two different r4133 renders.
                    assert!(
                        !(ours.eq_ignore_ascii_case(mine2)
                            && !theirs.eq_ignore_ascii_case(theirs2)),
                        "{}.{}: our {ours:?} maps to BOTH r4133's {theirs:?} and {theirs2:?} — \
                         a non-injective map equates two different values",
                        r.class,
                        r.prop
                    );
                }
            }
        }
        assert_eq!(rows, NORM_ENUM_SYNONYM_ROWS);
        // CLOSED SETS. The first two maps are distinct objects with distinct
        // contents — the trap DISCRIMINATION 3 above pins that behaviorally,
        // this pins it structurally. The third map is pinned for the reason the
        // doc gives: nothing else constrains an entry that folds nothing in
        // today's frozen population. Both shapes were re-measured against these
        // asserts (RP2.2 audit settlement): `("Rated", "ravg")` — r4133 *parses*
        // `'ravg'` to ordinal 2 (`InvControl.pas:837`) while our `'Rated'` is
        // ordinal 0 — reds on the mirror-injectivity assert above, and the
        // injective-but-still-widening `("Vref", "vref")` reds here (the
        // accept/refuse test catches that second one either way; the FIRST one
        // is the audit's measured survivor and shipped the whole suite green).
        assert_eq!(
            SCAN_TYPE_SYNONYMS,
            &[("Positive", "Pos"), ("Zero", "Zero")][..]
        );
        assert_eq!(
            SEQUENCE_TYPE_SYNONYMS,
            &[("Positive", "Pos"), ("Negative", "Neg")][..]
        );
        assert_eq!(
            VOLTAGE_CURVEX_REF_SYNONYMS,
            &[("Rated", "rated"), ("Avg", "avg"), ("RAvg", "avgrated")][..]
        );
        for (class, prop, want) in [
            ("vsource", "scantype", SCAN_TYPE_SYNONYMS),
            ("isource", "scantype", SCAN_TYPE_SYNONYMS),
            ("vsource", "sequence", SEQUENCE_TYPE_SYNONYMS),
            ("isource", "sequence", SEQUENCE_TYPE_SYNONYMS),
            (
                "invcontrol",
                "voltage_curvex_ref",
                VOLTAGE_CURVEX_REF_SYNONYMS,
            ),
        ] {
            let i = find_row(PROPS_NORM_R4133, class, prop).expect("an RP2.2 row");
            assert_eq!(
                PROPS_NORM_R4133[i].rule,
                EnumSynonym(want),
                "{class}.{prop} carries the wrong map"
            );
        }
    }

    // --------------------------------------------------- accounting + echo

    /// The per-row accounting is **armed but dormant**: nothing has visited a
    /// row (the r4133 props path is masked until RP4.1), so the fail-on-stale
    /// helper is silent — and it counts a hit only for a cell that was
    /// genuinely divergent.
    #[test]
    fn hit_accounting_is_armed_and_dormant() {
        let before = seam_touches_here();
        // An already-equal cell is not a divergence: no claim, no hit.
        let (a, e) = normalize_with(PROPS_NORM_R4133, "capacitor", "enabled", "Yes", "Yes");
        assert_eq!((a.as_ref(), e.as_ref()), ("Yes", "Yes"));
        assert_eq!(
            lookup_claim(PROPS_NORM_R4133, "capacitor", "enabled", "Yes", "Yes"),
            Some((
                find_row(PROPS_NORM_R4133, "capacitor", "enabled").unwrap(),
                false
            ))
        );
        // Dormant: the shipped counters are untouched by `normalize_with` (the
        // injected-table seam), so nothing this test did can make a row look
        // live. Asked per thread — see [`seam_touches_here`] for why a global
        // delta cannot answer this question honestly.
        assert_eq!(
            seam_touches_here(),
            before,
            "the injected seam moved a counter"
        );
    }

    /// The offline query part C's replay reads: same verdicts as the live seam,
    /// naming the row that claimed the cell — and no counter moved by it (the
    /// liveness assert stays silent, i.e. the replay cannot forge live
    /// coverage).
    #[test]
    fn the_offline_claim_query_names_the_row_and_moves_no_counter() {
        let before = seam_touches_here();
        let r = claiming_row("Capacitor", "Enabled", "Yes", "true").expect("bin-1 fold");
        assert_eq!(
            (r.class, r.prop, r.rule.tag(), r.bin),
            ("capacitor", "enabled", "BoolFold", 1)
        );
        let r = claiming_row("Transformer", "Conn", "wye", "wye ").expect("bin-2 trim fold");
        assert_eq!(r.rule.tag(), "CaseFold");
        let r = claiming_row("Line", "Ratings", "[ 400]", "[400,]").expect("bin-4 fold");
        assert_eq!(r.rule.tag(), "ArrayForm");
        // Unclaimed: no row, an echo cell, and a real difference.
        assert!(claiming_row("Foo", "Bar", "Yes", "true").is_none());
        assert!(claiming_row("Recloser", "EventLog", "No", "").is_none());
        assert!(claiming_row("Line", "Ratings", "[ 400]", "[401,]").is_none());
        assert_eq!(
            seam_touches_here(),
            before,
            "the offline query moved a counter"
        );
    }

    /// [`claim_value`] resolves the value chain in the documented order, agrees
    /// with the live seam cell for cell, and moves no counter.
    ///
    /// The agreement half is the load-bearing one: the claims census annotates
    /// the **plain** (raw) census population with this query instead of walking
    /// twice, which is only honest while "the query claims the cell" and "the
    /// armed seam equalises the cell" are the same statement.
    #[test]
    fn the_value_chain_resolves_in_order_and_agrees_with_the_seam() {
        let before = seam_touches_here();
        for (class, prop, rust, oracle, want) in [
            (
                "Capacitor",
                "Enabled",
                "Yes",
                "true",
                Some("normalized-by-BoolFold"),
            ),
            (
                "Transformer",
                "Conn",
                "wye",
                "wye ",
                Some("normalized-by-CaseFold"),
            ),
            (
                "Line",
                "Ratings",
                "[ 400]",
                "[400,]",
                Some("normalized-by-ArrayForm"),
            ),
            // **The chain ORDER, on the pairs that hold two rows.** A foldable
            // cell of a mixed pair is CLAIMED BY NORMALIZATION and the pair's
            // echo cell by the exclusion — same `(class, prop)`, two different
            // verdicts, decided by the cell. (What that buys is the census
            // disposition and the norm row's liveness, not a live compare: the
            // seam's exclusion is pair-scoped, `PROPS_ECHO_R4133`'s doc.)
            (
                "Recloser",
                "EventLog",
                "Yes",
                "yes",
                Some("normalized-by-BoolFold"),
            ),
            ("Recloser", "EventLog", "No", "", Some("echo-row")),
            (
                "Load",
                "Yearly",
                "other",
                "Other",
                Some("normalized-by-CaseFold"),
            ),
            ("Load", "Yearly", "day", "", Some("echo-row")),
            ("RegControl", "FwdThreshold", "100", "", Some("echo-row")),
            // …and the echo row is pair-scoped, so it also covers a spelling
            // the census has not seen (that is what makes it an exclusion and
            // not a rule).
            ("RegControl", "FwdThreshold", "100", "800", Some("echo-row")),
            // …with exactly one exception, and it is a cited one: the carve-out
            // takes its own measured cell back out of the row
            // (`ECHO_CARVE_OUTS`), so the chain walks on to the next link —
            // and since RP2.4 that link CLAIMS it. RP2.3 declared this cell to
            // RP2.4 (`ECHO_CARVE_OUT_ROUTING`) because r4133's own
            // `MakePosSequence` round-trips the live kvar through
            // `Format(' kvar=%-.5g')`, 5.0e-06 apart; the floor is the
            // mechanism that discharge names. It was `None` until RP2.4.
            (
                "Reactor",
                "kvar",
                "66.6666666666667",
                "66.667",
                Some("under-floor"),
            ),
            // The rest of the same pair is still the row's.
            ("Reactor", "kvar", "100", "1200", Some("echo-row")),
            (
                "Reactor",
                "kvar",
                "66.6666666666667",
                "66.66",
                Some("echo-row"),
            ),
            // **The chain's fourth link** (RP2.4): a pair with no row of either
            // table, claimed by the floor alone. The worst cell it takes
            // (`load.pf`, 6.431124e-05) and a matrix render, so the census
            // disposition is exercised on both shapes.
            (
                "Load",
                "pf",
                "0.747651914485831",
                "0.7477",
                Some("under-floor"),
            ),
            (
                "Line",
                "CMatrix",
                "[72.7194481250695 |0 72.7194481250695 ]",
                "[72.71945 |0 72.71945 ]",
                Some("under-floor"),
            ),
            // Unclaimed: no row at all / a real difference inside a pair whose
            // row refuses it / an r4133 root-cause pair no link owns — each of
            // them also OUTSIDE the floor, which is what keeps this column a
            // statement about the whole chain and not about its first three
            // links (`gictransformer.r2` is 2.5e-1, `line.ratings` 2.5e-3).
            ("Foo", "Bar", "Yes", "true", None),
            ("Line", "Ratings", "[ 400]", "[401,]", None),
            ("GICTransformer", "R2", "0.09522", "0.12696", None),
            // …and a numeric cell on a pair the floor DOES claim elsewhere,
            // 9.99e-4 apart: the floor is per cell, never per pair.
            ("Load", "pf", "0.747651914485831", "0.7484477", None),
        ] {
            let got = claim_value(PropsChannel::R4133, class, prop, rust, oracle);
            assert_eq!(
                got.map(ValueClaim::tag),
                want.map(str::to_string),
                "{class}.{prop} '{rust}' vs '{oracle}': chain answered {got:?}"
            );
            // The seam and the query say the same thing about this cell.
            let (a, e) = normalize_with(PROPS_NORM_R4133, class, prop, rust, oracle);
            assert_eq!(
                a == e,
                matches!(got, Some(ValueClaim::Normalization(_))),
                "{class}.{prop}: the seam and the offline chain disagree"
            );
        }
        // The LAST link carries RP2.4's derived value, read back from the
        // shipped slot rather than restated — and it is asked on a pair with no
        // echo row, or the echo link would answer first.
        assert_eq!(display_floor(), Some(2e-4));
        assert_eq!(
            claim_value(
                PropsChannel::R4133,
                "GICTransformer",
                "R2",
                "0.100019",
                "0.1"
            )
            .map(ValueClaim::tag),
            Some("under-floor".to_string()),
            "1.9e-4 — inside the floor, and '0.1' IS our value at one digit"
        );
        assert_eq!(
            claim_value(
                PropsChannel::R4133,
                "GICTransformer",
                "R2",
                "0.100021",
                "0.1"
            ),
            None,
            "2.1e-4 — outside the floor"
        );
        // …and the mechanism clause at the chain's last link: 1.9e-5, well
        // inside the floor, refused because six printed digits cannot be a
        // render of `0.1` (RP2.4 audit settlement).
        assert_eq!(
            claim_value(
                PropsChannel::R4133,
                "GICTransformer",
                "R2",
                "0.1",
                "0.100019"
            ),
            None,
            "inside the floor, but no `%.Ng` render of our value"
        );
        // Per thread, so a sibling test driving the real comparator cannot make
        // this pass or fail: the offline chain must reach NONE of the three
        // counting seams — not the normalization one, not the echo one and not
        // the floor one, the last of which the chain's own last link asks
        // through the offline twin [`under_display_floor`] on purpose.
        assert_eq!(
            seam_touches_here(),
            before,
            "the chain query moved a counter"
        );
    }

    /// **The display floor's metric, decomposed** (RP2.4) — the successor of
    /// RP2.1's floor-slot placeholder, which only asserted that the slot was
    /// empty.
    ///
    /// [`display_rel`] is what decides whether a cell is a *numeric* divergence
    /// at all, and the floor can never be wider than that decision. Each block
    /// below pins one clause of it against a real census spelling, so the four
    /// render shapes the census contains (scalar, exponent, bracketed vector,
    /// `|`-separated matrix) and the four refusals are proven at the metric
    /// rather than only at the seam.
    #[test]
    fn the_display_floor_metric_reads_the_numeric_skeleton() {
        // The WORST cell the floor claims, to the digit — a `%-.4g` render
        // (`PCElements/Load.pas:2345`). This number is the whole derivation's
        // left-hand side; if it moves, `R4133_DISPLAY_FLOOR`'s doc is stale.
        let worst = display_rel("0.747651914485831", "0.7477").expect("a numeric cell");
        assert!(
            (worst - 6.431_124e-5).abs() < 1e-10,
            "the worst floor-claimed cell measured 6.431124e-05, got {worst:e}"
        );
        assert!(worst < 2e-4, "…which is 3.110x inside the floor");
        // Every render shape, all through the one skeleton path.
        for (rust, oracle) in [
            ("3346958.0822587", "3.347E006"),
            ("[ 287.82360946885]", "[ 287.824]"),
            (
                "[72.7194481250695 |0 72.7194481250695 |0 0 72.7194481250695 ]",
                "[72.71945 |0 72.71945 |0 0 72.71945 ]",
            ),
        ] {
            let rel = display_rel(rust, oracle)
                .unwrap_or_else(|| panic!("{rust:?} vs {oracle:?} must be numeric"));
            assert!(rel <= 2e-4, "{rust:?} vs {oracle:?}: {rel:e}");
            assert!(under_display_floor(rust, oracle));
        }
        // The metric is symmetric — neither render is privileged as "expected".
        assert_eq!(
            display_rel("0.7477", "0.747651914485831"),
            display_rel("0.747651914485831", "0.7477")
        );
        // `None` — NOT a numeric divergence, so no floor value could ever claim
        // it: a differing non-numeric skeleton (boolean, enum, empty render,
        // another element count, RPN source text), and a cell with no number.
        for (rust, oracle) in [
            ("Yes", "true"),
            ("Positive", "Pos"),
            ("", "[]"),
            ("", "0.7477"),
            ("[ 400]", "[400, 400, 400]"),
            ("17", "1 16 +"),
            ("wye", "wye "),
        ] {
            assert_eq!(
                display_rel(rust, oracle),
                None,
                "{rust:?} vs {oracle:?} carries no comparable number pair"
            );
            assert!(!under_display_floor(rust, oracle));
        }
        // The **skeleton** clause, on its own: same count of numbers, same
        // values even, and a different non-numeric skeleton. Without these two
        // rows the block above proves only the count and emptiness clauses —
        // every one of its seven pairs is rejected by one of those (RP2.4 audit
        // round, `display_rel`'s skeleton clause was untested).
        for (rust, oracle) in [("[ 400]", "400"), ("1 kV", "1 kW")] {
            assert_eq!(
                display_rel(rust, oracle),
                None,
                "{rust:?} vs {oracle:?}: the non-numeric skeletons differ"
            );
            assert!(!under_display_floor(rust, oracle));
        }
        // …and the numeric refusals: above the floor, and 0-vs-non-zero.
        assert_eq!(display_rel("0.001", "0.0"), Some(1.0));
        assert!(display_rel("0.747651914485831", "0.7484477").expect("numeric") > 2e-4);
        assert!(!under_display_floor("0.747651914485831", "0.7484477"));
        // The mechanism clause, at the metric: a REAL census spelling 1.2e-6
        // apart — deep inside the floor, and refused, because r4133 prints
        // `load.kva` with `Format('%-g')` — 15 digits, `PCElements/Load.pas:2352`
        // — while its own kVA is recomputed from an already round-tripped `pf`.
        // The gap is a state difference, not a render (RP2.4 audit settlement;
        // this spelling is one of RP3.9's 55).
        let rel = display_rel("105.263157894737", "105.26302971129").expect("numeric");
        assert!(rel < 2e-4 && rel > 1e-6, "{rel:e}");
        assert!(!display_is_render("105.263157894737", "105.26302971129"));
        assert!(!under_display_floor("105.263157894737", "105.26302971129"));
    }

    /// **The floor's live seam counts what the gate saw** — the floor's half of
    /// the counter discipline the two tables already carry
    /// ([`tests::hit_accounting_is_armed_and_dormant`],
    /// [`tests::the_echo_seam_counts_visits_and_hits`]), and the reason
    /// [`under_display_floor_r4133`] exists next to [`under_display_floor`] at
    /// all: the claims census asks the offline twin about cells the *gate*
    /// never compared, so only the seam may count. After RP4.1 these two
    /// numbers are what the live floor population is read from.
    ///
    /// The deltas on the process-global statics are `>=`, not `==`: `cargo test`
    /// runs one binary's tests concurrently and several of them drive the real
    /// comparator, so the statics move under this test's feet. `>=` is exactly
    /// the part that is this test's to claim, and it is the part that fails if
    /// the seam stops counting. The EXACT deltas — including the ones the
    /// offline twin must not produce — are read per thread
    /// ([`seam_touches_here`]), where no sibling can reach.
    #[test]
    fn the_display_floor_seam_counts_visits_and_hits() {
        let (v0, h0) = display_floor_counters();
        let t0 = seam_touches_here().floor;
        // Two visits, one hit: a claimed cell and a refused one — and the seam
        // answers exactly what the offline twin answers on both.
        assert!(under_display_floor_r4133("0.747651914485831", "0.7477"));
        assert!(under_display_floor("0.747651914485831", "0.7477"));
        assert!(!under_display_floor_r4133("0.747651914485831", "0.7484477"));
        assert!(!under_display_floor("0.747651914485831", "0.7484477"));
        let (v1, h1) = display_floor_counters();
        assert!(v1 >= v0 + 2, "the seam must count every cell it sees");
        assert!(h1 > h0, "…and every cell it claims");
        // …and exactly two visits / one hit, i.e. the two offline-twin calls
        // interleaved above counted nothing at all.
        assert_eq!(seam_touches_here().floor, (t0.0 + 2, t0.1 + 1));
        // …and the POLICY reaches the counting seam, not the offline twin —
        // the arm that makes these two numbers "what the gate saw". Swapping
        // `PropsPolicy::under_display_floor` for `props_norm::under_display_floor`
        // leaves every other assertion in the file green (RP2.4 audit round,
        // mutation m4); this is where it dies.
        let policy = crate::harness::PropsPolicy::for_channel(PropsChannel::R4133);
        assert!(policy.under_display_floor("0.747651914485831", "0.7477"));
        let (v2, h2) = display_floor_counters();
        assert!(
            v2 > v1 && h2 > h1,
            "the shipped policy must go through the COUNTING seam"
        );
        assert_eq!(seam_touches_here().floor, (t0.0 + 3, t0.1 + 2));
    }

    /// **Capi-invariance of the measurement layer** (plan mechanic (b)).
    ///
    /// [`claim_value`] is asked about the rows of BOTH channels — the claims
    /// census annotates every census row it produced, on capi as well as r4133
    /// (`corpus_gate/props_census.rs`, `Row::annotate`). So the "capi is the
    /// identity" contract has to be a property of this function, not of the
    /// data: every cell the r4133 arm claims must come back `None` on capi, for
    /// each live rule kind and for the RP2.4 floor's slot.
    ///
    /// Before the RP2.1 audit round it was channel-blind, and the census's
    /// measured "0 cells normalized on capi" was therefore a statement about
    /// today's capi population, not about the contract.
    #[test]
    fn the_capi_channel_claims_nothing() {
        for (class, prop, rust, oracle) in [
            ("Capacitor", "Enabled", "Yes", "true"),
            ("Transformer", "Conn", "wye", "wye "),
            ("Line", "Ratings", "[ 400]", "[400,]"),
            ("SwtControl", "Normal", "closed", "[closed, ]"),
            // RP2.3's link: an echo-excluded cell must stay a full compare on
            // capi. This one matters more than the folds above — capi is where
            // most of these rows' witness lives, so an echo row leaking onto
            // that channel would delete the very evidence it cites.
            ("RegControl", "Idle", "No", ""),
            ("Load", "Yearly", "day", ""),
            // RP2.4's link: two numbers 1.14e-6 apart in the shape the
            // mechanism produces (our full value, r4133's shorter render of
            // it), which the r4133 arm claims `under-floor`. This row was
            // written by RP2.1 as a slot guard ("`None` here must not depend on
            // the floor being `None`") and RP2.4 is what turned it into a live
            // channel statement.
            ("Load", "pf", "0.880001", "0.88"),
        ] {
            assert_eq!(
                claim_value(PropsChannel::CapiV0145, class, prop, rust, oracle),
                None,
                "{class}.{prop}: the capi channel must reach no link of the r4133 chain"
            );
        }
        // …and the r4133-claimed ones really are claimed, so the assertion
        // above is a channel statement and not a "nothing is ever claimed" one.
        for (class, prop, rust, oracle) in [
            ("Capacitor", "Enabled", "Yes", "true"),
            ("Transformer", "Conn", "wye", "wye "),
            ("Line", "Ratings", "[ 400]", "[400,]"),
            ("RegControl", "Idle", "No", ""),
            ("Load", "Yearly", "day", ""),
            ("Load", "pf", "0.880001", "0.88"),
        ] {
            assert!(
                claim_value(PropsChannel::R4133, class, prop, rust, oracle).is_some(),
                "{class}.{prop} must be claimed on r4133"
            );
        }
    }

    /// **The fail-on-stale guard, proven silent where it must be.**
    ///
    /// [`check_rows_are_live`] is the rule behind [`assert_norm_rows_are_live`],
    /// and both halves of it need a canary: today the helper is structurally a
    /// no-op (the r4133 props path is masked until RP4.1, so every row has
    /// `visits == 0`) and at RP4.1 it becomes the only live anti-rot guard the
    /// 157 rows have. This test pins the two silent cases; the next one pins
    /// that it can actually fire.
    #[test]
    fn the_liveness_guard_is_silent_when_dormant_or_live() {
        let n = PROPS_NORM_R4133.len();
        // Dormant: nothing visited (today's gate, and any DSS_GATE_ONLY run
        // that filters a row's cases away).
        check_rows_are_live(PROPS_NORM_R4133, &vec![0; n], &vec![0; n]);
        // Live: every row visited and folding.
        check_rows_are_live(PROPS_NORM_R4133, &vec![7; n], &vec![3; n]);
        // Mixed, and legitimately so: an unvisited row next to a working one.
        let mut visits = vec![0; n];
        let mut hits = vec![0; n];
        visits[0] = 4;
        hits[0] = 1;
        check_rows_are_live(PROPS_NORM_R4133, &visits, &hits);
        // Deliberately NOT calling `assert_norm_rows_are_live()` here. It reads
        // the process-global counters, which a gate test in the same binary
        // legitimately moves once RP4.1 unmasks the r4133 props path — that is
        // the exact test-ordering trap RP2.1 part D found and removed from three
        // tests in this module. The rule is proven above over injected counters;
        // the shipped adapter's wiring is exercised where the live assertion
        // belongs, once, at the end of the gate (`corpus_gate.rs`).
    }

    /// …and the other direction: a row that was COMPARED and folded nothing is
    /// exempting a spelling difference that is no longer there, and the guard
    /// says so, naming the row.
    #[test]
    #[should_panic(expected = "stale r4133 property normalization row: capacitor.enabled")]
    fn the_liveness_guard_fires_on_a_stale_row() {
        let n = PROPS_NORM_R4133.len();
        let i = find_row(PROPS_NORM_R4133, "capacitor", "enabled").expect("a shipped row");
        let mut visits = vec![0; n];
        let hits = vec![0; n];
        visits[i] = 12;
        check_rows_are_live(PROPS_NORM_R4133, &visits, &hits);
    }

    // ------------------------------------------------------- the echo table

    /// [`find_echo_row`] binary-searches, so the order is a correctness
    /// invariant, and a duplicate `(class, prop)` would make which row wins
    /// depend on the search's landing point.
    #[test]
    fn the_echo_table_is_sorted_and_unique() {
        for w in PROPS_ECHO_R4133.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            assert_eq!(
                ci_cmp(a.class, b.class).then_with(|| ci_cmp(a.prop, b.prop)),
                Ordering::Less,
                "PROPS_ECHO_R4133 must be sorted by (class, prop) with no duplicate: \
                 {}.{} vs {}.{}",
                a.class,
                a.prop,
                b.class,
                b.prop
            );
        }
        // Lookup is case-insensitive because the capture spells a class however
        // the engine does, and it finds the row for a pair spelled either way.
        assert!(has_echo_row("RegControl", "IdleForward"));
        assert!(has_echo_row("regcontrol", "idleforward"));
        assert!(!has_echo_row("regcontrol", "band"));
        assert!(!has_echo_row("Foo", "Bar"));
        assert!(echo_excluded("RegControl", "IdleForward", "No", ""));
        assert!(echo_excluded("regcontrol", "idleforward", "No", ""));
        assert!(!echo_excluded("regcontrol", "band", "2", "3"));
        assert!(!echo_excluded("Foo", "Bar", "a", "b"));
    }

    /// **The table's contents, pinned literally** — the
    /// [`enumsynonym_maps_are_injective`] discipline at table scale.
    ///
    /// Every row of this table STOPS a value compare forever, so the row set is
    /// not something a later edit may widen quietly: a new pair, a re-tagged
    /// category or a deleted row all red here and have to be argued in the diff.
    /// It is the offline half of "no wildcard, no over-broad row"; the live half
    /// is [`check_echo_rows_are_live`] plus the census's `echo-row` tally.
    #[test]
    fn the_echo_table_is_the_measured_row_set() {
        let got: Vec<String> = PROPS_ECHO_R4133
            .iter()
            .map(|r| format!("{}.{} {}", r.class, r.prop, r.category.tag()))
            .collect();
        let want = [
            "autotrans.bhcurrent EmptyCollectionRender",
            "autotrans.bhflux EmptyCollectionRender",
            "autotrans.pctperm EchoDefault",
            "autotrans.repair EchoDefault",
            "capcontrol.reset EchoDefault",
            "capcontrol.type EchoParse",
            "energymeter.action EchoDefault",
            "energymeter.peakcurrent EchoDefault",
            "expcontrol.derlist LiveSemanticsDiffer",
            "fault.bus2 EchoParse",
            "fault.pctperm EchoDefault",
            "fuse.switchedobj EchoDefault",
            "generator.d LiveSemanticsDiffer",
            "generator.dynout LiveSemanticsDiffer",
            "generator.model EchoParse",
            "generator.shaftdata EmptyCollectionRender",
            "generator.userdata EmptyCollectionRender",
            "gicsource.spectrum EchoDefault",
            "gictransformer.pctperm EchoDefault",
            "invcontrol.lpftau EchoDefault",
            "invcontrol.mode EchoDefault",
            "invcontrol.monvoltagecalc EchoDefault",
            "invcontrol.pvsystemlist EchoDefault",
            "invcontrol.risefalllimit EchoDefault",
            "invcontrol.vsetpoint EchoDefault",
            "isource.bus2 EchoDefault",
            "isource.yearly EchoDefault",
            "line.cncables EchoDefault",
            "line.conductors EchoDefault",
            "line.spacing EchoParse",
            "line.tscables EchoDefault",
            "line.wires EchoDefault",
            "load.yearly LiveSemanticsDiffer",
            "load.zipv EmptyCollectionRender",
            "monitor.mode EchoParse",
            "pvsystem.%pminkvarmax LiveSemanticsDiffer",
            "pvsystem.%pminnovars LiveSemanticsDiffer",
            "pvsystem.amplimit EchoDefault",
            "pvsystem.amplimitgain EchoDefault",
            "pvsystem.dynout EmptyCollectionRender",
            "pvsystem.userdata EmptyCollectionRender",
            "reactor.bus2 EchoParse",
            "reactor.kvar EchoDefault",
            "recloser.debugtrace EchoDefault",
            "recloser.eventlog EchoDefault",
            "recloser.switchedobj EchoDefault",
            "regcontrol.fwdthreshold EchoDefault",
            "regcontrol.idle EchoDefault",
            "regcontrol.idleforward EchoDefault",
            "regcontrol.idlereverse EchoDefault",
            "regcontrol.remoteptratio EchoDefault",
            "regcontrol.revthreshold EchoDefault",
            "relay.action EchoDefault",
            "relay.distreverse EchoDefault",
            "relay.reset EchoParse",
            "relay.switchedobj EchoDefault",
            "storage.%pminkvarmax LiveSemanticsDiffer",
            "storage.%pminnovars LiveSemanticsDiffer",
            "storage.amplimit EchoDefault",
            "storage.amplimitgain EchoDefault",
            "storage.dynadata EmptyCollectionRender",
            "storage.dynadll LiveSemanticsDiffer",
            "storage.userdata EmptyCollectionRender",
            "storagecontroller.modedischarge LiveSemanticsDiffer",
            "storagecontroller.seasontargets EmptyCollectionRender",
            "storagecontroller.seasontargetslow EmptyCollectionRender",
            "swtcontrol.action EchoParse",
            "transformer.bhcurrent EmptyCollectionRender",
            "transformer.bhflux EmptyCollectionRender",
            "transformer.pctperm EchoDefault",
            "transformer.repair EchoDefault",
            "upfc.climit EchoDefault",
            "upfc.kvarlimit EchoDefault",
            "upfc.refkv2 EchoDefault",
            "upfc.vhlimit EchoDefault",
            "upfc.vllimit EchoDefault",
            "upfccontrol.basefreq EchoDefault",
            "upfccontrol.enabled EchoDefault",
            "vccs.bp1 EchoDefault",
            "vccs.bp2 EchoDefault",
            "vsource.yearly EchoDefault",
            "windgen.dynout EmptyCollectionRender",
        ];
        assert_eq!(got, want, "PROPS_ECHO_R4133's row set moved");
    }

    /// **The five pairs the RP2.3 kill criterion fired on take NO row here.**
    ///
    /// r4133 renders a live computed read-only quantity for each
    /// (`IndMach012.pas:1790`, `StorageController.pas:991-994`) and the port
    /// renders `''` only because dss_capi 0.14.5 flags them
    /// `[SilentReadOnly, ReadByFunction]`. Under the 2026-08-02 policy the
    /// 0.14.5 convention yields: the fix is an ENGINE change (render the live
    /// value, exclude the capi side there), which is why these five are routed
    /// to their own sub-step instead of being given a category that would
    /// misdescribe them. If a later pass adds one of them here, the kill
    /// ruling has to be re-opened first.
    #[test]
    fn the_silent_readonly_pairs_have_no_echo_row() {
        for (class, prop) in [
            ("indmach012", "pf"),
            ("storagecontroller", "kwhtotal"),
            ("storagecontroller", "kwtotal"),
            ("storagecontroller", "kwhactual"),
            ("storagecontroller", "kwactual"),
        ] {
            assert!(
                !has_echo_row(class, prop),
                "{class}.{prop} is a SilentReadOnly surface, not an echo — RP3.8's, not this \
                 table's (props_r4133_replay::RP38_ROUTING)"
            );
        }
    }

    /// **Every row carries a witness** — CLAUDE.md's exclusion discipline as a
    /// test: an exclusion is only admissible paired with the thing that still
    /// holds the value. A `Capi` witness must name a positive case count (a
    /// zero would be the absence of a witness spelled as one), and a pin's name
    /// must be a real test identifier.
    #[test]
    fn every_echo_row_carries_a_witness() {
        for r in PROPS_ECHO_R4133 {
            if let Some(n) = r.witness.capi_cases() {
                assert!(
                    n > 0,
                    "{}.{}: a Capi witness with zero cases is no witness",
                    r.class,
                    r.prop
                );
            }
            if let Some(name) = r.witness.pin() {
                assert!(
                    !name.is_empty()
                        && name
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                    "{}.{}: {name:?} is not a test identifier",
                    r.class,
                    r.prop
                );
            }
            assert!(
                r.cite.contains(".pas"),
                "{}.{}: the row must cite an r4133 source unit, got {:?}",
                r.class,
                r.prop,
                r.cite
            );
            assert!(
                r.cells > 0,
                "{}.{}: a cited pair has cells",
                r.class,
                r.prop
            );
            assert!(
                r.class.chars().all(|c| !c.is_ascii_uppercase())
                    && r.prop.chars().all(|c| !c.is_ascii_uppercase()),
                "{}.{}: rows are spelled as the census spells a pair (lowercase)",
                r.class,
                r.prop
            );
        }
    }

    /// **A `LiveSemanticsDiffer` row always names a pin.** The category claims
    /// the two engines mean different things and *ours is right* — capi
    /// agreement cannot carry that claim (capi is a numeric oracle, not the
    /// behavioral authority), so the expected-value pin is mandatory. The names
    /// are the tests RP2.3 part B2 lands.
    #[test]
    fn every_live_semantics_row_names_a_pin() {
        let named: Vec<(&str, &str, &str)> = PROPS_ECHO_R4133
            .iter()
            .filter(|r| r.category == LiveSemanticsDiffer)
            .map(|r| {
                (
                    r.class,
                    r.prop,
                    r.witness.pin().unwrap_or_else(|| {
                        panic!(
                            "{}.{} is LiveSemanticsDiffer and names no pin — the claim \
                             'the port's meaning is the correct one' needs one",
                            r.class, r.prop
                        )
                    }),
                )
            })
            .collect();
        assert_eq!(named.len(), ECHO_LIVE_SEMANTICS_ROWS);
        // …and the pin set as a whole, pinned literally so a row cannot quietly
        // start pointing at a test that does not exist.
        let mut pins: Vec<&str> = PROPS_ECHO_R4133
            .iter()
            .filter_map(|r| r.witness.pin())
            .collect();
        pins.sort_unstable();
        pins.dedup();
        assert_eq!(
            pins,
            [
                "autotrans_bh_arrays_render_empty_when_unset",
                "der_amp_limits_render_the_live_sentinel_and_gain",
                "der_user_model_arrays_render_empty_when_unset",
                "energymeter_action_and_capcontrol_reset_render_no_pending_command",
                "energymeter_peakcurrent_renders_the_live_one_element_array",
                "expcontrol_derlist_renders_the_der_list",
                "fault_bus2_renders_the_live_terminal",
                "fuse_switchedobj_defaults_to_the_monitored_element",
                "generator_d_renders_the_documented_damping_default",
                "generator_dynout_renders_the_named_variables",
                "generator_model_renders_the_live_pv2pq_conversion",
                "invcontrol_defaults_render_the_live_values",
                "line_conductors_renders_the_live_conductor_list",
                "line_spacing_renders_empty_once_the_spacing_is_killed",
                "load_yearly_renders_the_resolved_loadshape_name",
                "load_zipv_renders_the_live_seven_element_vector",
                "pd_element_perm_and_repair_render_the_live_ratings",
                "pvsystem_and_storage_pmin_sentinels_deactivate_the_var_limits",
                "reactor_kvar_renders_the_live_rating",
                "recloser_eventlog_and_debugtrace_default_to_no",
                "recloser_switchedobj_defaults_to_the_monitored_element",
                "regcontrol_idle_flags_and_thresholds_render_the_live_values",
                "relay_action_distreverse_and_reset_render_the_live_values",
                "relay_switchedobj_defaults_to_the_monitored_element",
                "storage_dynadll_renders_the_typed_path",
                "storagecontroller_modedischarge_renders_schedule",
                "storagecontroller_seasontargets_render_the_live_targets",
                "swtcontrol_action_renders_the_live_switch_state",
                "transformer_bh_arrays_render_empty_when_unset",
                "windgen_dynout_renders_empty_when_unset",
            ],
            "the expected-value pins the rows depend on — 20 from RP2.3 part B2, nine added by \
             its audit settlement for the rows exposed on r4133-only cases, and RP3.3's \
             generator.model (whose two cells are BOTH on r4133-only cases)"
        );
        assert_eq!(
            PROPS_ECHO_R4133
                .iter()
                .filter(|r| r.witness.pin().is_some())
                .count(),
            64,
            "rows whose witness is (also) a pin"
        );
    }

    /// **A `Capi` witness must name a pair the capi channel really compares.**
    ///
    /// [`EchoWitness::Capi`]'s `n` is a dated census number nothing can re-derive
    /// offline, so the type asserts only `n > 0`. That leaves one failure mode a
    /// test CAN close: a row claiming capi coverage for a pair the capi walk
    /// never reaches, because a `SKIP_PROPS` row masks its value, `PROPS_015X`
    /// drops it from the 0.14.5 capture, or the whole element is skipped there.
    /// The RP2.3 rows behind those masks all carry `Pin` today — checked by hand
    /// in part A's witness split, checked by the suite from here on (RP2.3 audit
    /// settlement).
    #[test]
    fn a_capi_witness_is_a_pair_the_capi_channel_can_compare() {
        for r in PROPS_ECHO_R4133 {
            if r.witness.capi_cases().is_none() {
                continue;
            }
            assert!(
                !super::super::skip_prop(r.class, r.prop, PropsChannel::CapiV0145),
                "{}.{}: a capi witness on a pair SKIP_PROPS masks off the capi channel",
                r.class,
                r.prop
            );
            assert!(
                !super::super::skip_whole_element(r.class, PropsChannel::CapiV0145),
                "{}.{}: a capi witness on a class the capi walk skips whole",
                r.class,
                r.prop
            );
            assert!(
                !super::super::PROPS_015X.iter().any(|(c, props)| {
                    c.eq_ignore_ascii_case(r.class)
                        && props.iter().any(|p| p.eq_ignore_ascii_case(r.prop))
                }),
                "{}.{}: a capi witness on a prop the 0.14.5 capture cannot carry (PROPS_015X)",
                r.class,
                r.prop
            );
        }
    }

    /// **Every row that masks cells on an `engines: "r4133"` case names a pin** —
    /// plan mechanic (c)'s "r4133-only classes/**cases**" half, which part B2
    /// applied to one row by hand (`swtcontrol.action`) and the audit settlement
    /// turned into a rule over the measured population.
    ///
    /// The capi channel does not run on those cases at all, so a `Capi(n)`
    /// witness — however large `n` is — says nothing about the cells the row
    /// masks there.
    ///
    /// **What this guard does and does not prove about the numbers** (RP3.3
    /// audit settlement, 2026-08-24). The pin obligation is the point, and it is
    /// per row. The counted columns are a dated measurement, held by three
    /// locks — the row count, the cell sum ([`R4133_ONLY_CELLS`]) and, since the
    /// settlement, the case sum ([`R4133_ONLY_CASES`]), which had none — plus
    /// the structural invariant `cases <= cells` (an exposed case contributes at
    /// least one cell, or it is not exposed). A row whose split is *derived*
    /// rather than measured ties itself to its derivation through
    /// [`r4133_only_exposure`]; nothing here can do that for a number no test
    /// can recompute.
    #[test]
    fn every_row_exposed_on_r4133_only_cases_names_a_pin() {
        assert_eq!(
            ECHO_ROWS_ON_R4133_ONLY_CASES.len(),
            R4133_ONLY_ROWS,
            "the measured exposure list moved"
        );
        assert_eq!(
            ECHO_ROWS_ON_R4133_ONLY_CASES
                .iter()
                .map(|(_, _, cells, _)| cells)
                .sum::<u32>(),
            R4133_ONLY_CELLS,
            "…and so did the cells behind it"
        );
        assert_eq!(
            ECHO_ROWS_ON_R4133_ONLY_CASES
                .iter()
                .map(|(_, _, _, cases)| cases)
                .sum::<u32>(),
            R4133_ONLY_CASES,
            "…and the cases they sit on"
        );
        for (class, prop, cells, cases) in ECHO_ROWS_ON_R4133_ONLY_CASES {
            assert!(
                cells > &0 && cases > &0,
                "{class}.{prop}: an empty exposure"
            );
            assert!(
                cases <= cells,
                "{class}.{prop}: {cases} case(s) behind {cells} cell(s) — every exposed case \
                 contributes at least one cell, so this split cannot be a measurement"
            );
            let i = find_echo_row(PROPS_ECHO_R4133, class, prop)
                .unwrap_or_else(|| panic!("{class}.{prop} is listed but has no echo row"));
            assert!(
                PROPS_ECHO_R4133[i].witness.pin().is_some(),
                "{class}.{prop}: the row masks {cells} cell(s) on {cases} r4133-only case(s), \
                 where the capi channel never runs — a capi witness cannot hold that value, so \
                 the row owes an expected-value pin (plan §1.2 mechanic (c))"
            );
        }
        // Sorted and unique, so a duplicate cannot hide a missing pin.
        for w in ECHO_ROWS_ON_R4133_ONLY_CASES.windows(2) {
            assert!(
                (w[0].0, w[0].1) < (w[1].0, w[1].1),
                "the exposure list must be sorted by (class, prop) with no duplicate"
            );
        }
    }

    /// **The dormant-row exemption list is pinned literally**, like every other
    /// closed set RP2.1/RP2.2 shipped.
    ///
    /// [`ECHO_ROWS_WITH_NO_IN_SCOPE_CELL`] switches off the only live anti-rot
    /// guard the 82 exclusions will have after RP4.1, one pair at a time. Before
    /// the RP2.3 audit settlement it carried neither a literal nor a count lock,
    /// so a third entry would have silently disarmed the guard for another row.
    #[test]
    fn the_dormant_row_exemption_list_is_pinned() {
        assert_eq!(
            ECHO_ROWS_WITH_NO_IN_SCOPE_CELL,
            [("fault", "bus2"), ("line", "spacing")],
            "the two pairs whose echo cells all sit on capi-only cases (claims census, \
             2026-08-23) — adding one exempts another row from fail-on-stale, which is a \
             decision, not a tidy-up"
        );
        for (class, prop) in ECHO_ROWS_WITH_NO_IN_SCOPE_CELL {
            assert!(
                has_echo_row(class, prop),
                "{class}.{prop} is exempted but has no echo row to exempt"
            );
        }
    }

    /// **The live seam counts what the gate saw, and answers the pair-scoped
    /// question.** A visit is any compared cell of the pair; a hit is a visit
    /// whose sides differed, i.e. a compare the row really stopped.
    ///
    /// The exact deltas are read per thread ([`seam_touches_here`]) — sibling
    /// tests in the same binary drive the very same seam, so the process-global
    /// totals can move between two statements here (RP3.3's audit round measured
    /// that flake on the sister test). What the globals are still asked, at the
    /// end and as `>=`, is the direction the thread-local cannot prove: that the
    /// SHIPPED statics — the ones `assert_echo_rows_are_live` and RP4.1's live
    /// accounting read — are the pair this seam moved.
    #[test]
    fn the_echo_seam_counts_visits_and_hits() {
        let before = seam_touches_here().echo;
        let before_global = echo_counter_totals();
        // A pair with no row is never touched, and moves no counter.
        assert!(!echo_excluded_r4133("Foo", "Bar", "a", "b"));
        assert_eq!(seam_touches_here().echo, before);
        // Equal sides: visited, not a hit — nothing was excluded.
        assert!(echo_excluded_r4133("RegControl", "Idle", "No", "No"));
        assert_eq!(seam_touches_here().echo, (before.0 + 1, before.1));
        // Differing sides: the exclusion did work.
        assert!(echo_excluded_r4133("RegControl", "Idle", "No", ""));
        assert_eq!(seam_touches_here().echo, (before.0 + 2, before.1 + 1));
        // A carved-out cell is not excluded, so it is not a visit either — the
        // liveness accounting must not credit the row for a cell it let through.
        assert!(!echo_excluded_r4133(
            "Reactor",
            "kvar",
            "66.6666666666667",
            "66.667"
        ));
        assert_eq!(seam_touches_here().echo, (before.0 + 2, before.1 + 1));
        let after_global = echo_counter_totals();
        assert!(
            after_global.0 >= before_global.0 + 2 && after_global.1 > before_global.1,
            "the shipped ECHO_VISITS/ECHO_HITS statics must carry what this seam counted \
             ({before_global:?} -> {after_global:?})"
        );
    }

    /// **The carve-out takes exactly its cited cell out of the row** — the
    /// narrowing valve the RP2.3 audit settlement added, in both directions.
    ///
    /// `reactor.kvar`'s row cites the frozen `'1200'` default; the pair's 607th
    /// census cell is r4133's own live `MakePosSequence` round-trip
    /// (`Reactor.pas:1145-1201`, `kvar=%-.5g` through the parser) and is not an
    /// echo at all. A pair-scoped row would have masked it — that is exactly the
    /// "mask wider than its citation" this valve exists to prevent — so the cell
    /// leaves the exclusion and RP2.4's display floor inherits it.
    #[test]
    fn the_carve_out_takes_exactly_its_cited_cell_out_of_the_row() {
        assert_eq!(ECHO_CARVE_OUTS.len(), ECHO_CARVE_OUT_CELLS);
        for c in ECHO_CARVE_OUTS {
            // A carve-out is only meaningful inside a row it narrows…
            assert!(
                has_echo_row(c.class, c.prop),
                "{}.{}: a carve-out on a pair with no echo row",
                c.class,
                c.prop
            );
            // …it must cite a mechanism…
            assert!(
                c.why.contains(".pas:"),
                "{}.{}: the carve-out must cite the r4133 site that makes the cell live",
                c.class,
                c.prop
            );
            // …and the two sides really differ (a cell that already compares
            // equal needs no carve-out).
            assert_ne!(c.rust, c.oracle);
            // The cell itself is out.
            assert!(!echo_excluded(c.class, c.prop, c.rust, c.oracle));
            assert!(!carved_out(c.class, c.prop, c.rust, "something else"));
            assert!(!carved_out(c.class, c.prop, "something else", c.oracle));
            // Matching is case-insensitive on the PAIR, exact on the values.
            assert!(!echo_excluded(
                &c.class.to_uppercase(),
                &c.prop.to_uppercase(),
                c.rust,
                c.oracle
            ));
            // …and the rest of the pair is still excluded.
            assert!(echo_excluded(c.class, c.prop, c.rust, "1200"));
            assert!(has_echo_row(c.class, c.prop));
        }
    }

    /// The echo liveness guard, silent where it must be: dormant (today's gate,
    /// every counter 0), fully live, and on the two rows the census measured as
    /// having no in-scope cell at all.
    #[test]
    fn the_echo_liveness_guard_is_silent_when_dormant_or_live() {
        let n = PROPS_ECHO_R4133.len();
        check_echo_rows_are_live(PROPS_ECHO_R4133, &vec![0; n], &vec![0; n]);
        check_echo_rows_are_live(PROPS_ECHO_R4133, &vec![9; n], &vec![4; n]);
        // Visited but excluding nothing — legitimate ONLY for the exempt rows.
        let mut visits = vec![0; n];
        let hits = vec![0; n];
        for (class, prop) in ECHO_ROWS_WITH_NO_IN_SCOPE_CELL {
            let i = find_echo_row(PROPS_ECHO_R4133, class, prop).expect("a shipped row");
            visits[i] = 11;
        }
        check_echo_rows_are_live(PROPS_ECHO_R4133, &visits, &hits);
    }

    /// …and the other direction: a row that was COMPARED and excluded nothing
    /// is masking a divergence that is no longer there, and the guard says so,
    /// naming the row. (Driven on a pair that is NOT on the exemption list.)
    #[test]
    #[should_panic(expected = "stale r4133 property echo row: regcontrol.idle")]
    fn the_echo_liveness_guard_fires_on_a_stale_row() {
        let n = PROPS_ECHO_R4133.len();
        let i = find_echo_row(PROPS_ECHO_R4133, "regcontrol", "idle").expect("a shipped row");
        let mut visits = vec![0; n];
        let hits = vec![0; n];
        visits[i] = 15;
        check_echo_rows_are_live(PROPS_ECHO_R4133, &visits, &hits);
    }
}
