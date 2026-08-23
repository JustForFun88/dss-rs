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
//! That statement has to hold at **two** seams, because the table has two
//! callers, and each carries its own channel gate:
//!
//! * the live comparator — [`normalize_r4133`], reached only from
//!   `PropsPolicy::normalize`'s r4133 arm (`PropsPolicy::is_r4133`, pinned by
//!   `props_policy_tests::the_capi_channel_never_normalizes`);
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
//! cannot satisfy that is an **exclusion** — [`PROPS_ECHO_R4133`], whose rows
//! land in RP2.3 — never a rule here.
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
//! # Population (RP2.1, bins 1/2/4 of plan §1.1)
//!
//! | kind | rows | derivation |
//! |---|---|---|
//! | [`BoolFold`](NormRule::BoolFold) | 77 | bin 1's 75 pairs **minus the five pure-echo pairs** (`capcontrol.reset`, `recloser.debugtrace`, `regcontrol.idleforward`, `regcontrol.idlereverse`, `upfccontrol.enabled` — no foldable cell at all, vendored `README.md` §"Bin 1 carries nine echo pairs, not three"), plus 7 WP-RP1 pairs |
//! | [`CaseFold`](NormRule::CaseFold) | 62 | bin 2 whole (59 case-only + 2 trailing-space), plus 2 WP-RP1 pairs, **minus** `invcontrol.voltage_curvex_ref` (re-typed by RP2.2, next row) |
//! | [`ArrayForm`](NormRule::ArrayForm) | 17 | bin 4's 21 pairs minus 4 no typed rule may claim (below) |
//! | [`EnumSynonym`](NormRule::EnumSynonym) | 5 | RP2.2: the four source sequence-selector pairs of bin 3 ([`SCAN_TYPE_SYNONYMS`] / [`SEQUENCE_TYPE_SYNONYMS`]) plus [`VOLTAGE_CURVEX_REF_SYNONYMS`]; the other four bin-3 pairs are NOT synonyms — see below |
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
use std::cmp::Ordering;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrd};

use super::PropsChannel;
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
    /// will hang; today the floor is `None` and the compare is exact), other
    /// tokens ASCII-case-insensitively. The token **count** must match, so a
    /// one-element array never folds into a three-element one.
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
    /// Shipped with **zero** rows by RP2.1; **RP2.2 landed four**, all on the
    /// source sequence-selector properties ([`SCAN_TYPE_SYNONYMS`],
    /// [`SEQUENCE_TYPE_SYNONYMS`]). The kind's behavior is pinned both against
    /// the shipped maps and against a synthetic one, the way `EVENTLOG_MASKS`'
    /// r4133 row set is pinned against an injected table (`harness/mod.rs`,
    /// `eventlog_mask_tests`).
    ///
    /// The map is **directional and per-pair**: `(ours, theirs)`, matched after
    /// `trim()` and ASCII-case-insensitively, with no reverse implication. A map
    /// must also be **injective on its r4133 side** — one r4133 token may not
    /// name two of our values, or the rule would equate two different values.
    /// [`tests::enumsynonym_maps_are_injective`] enforces that on every shipped
    /// map.
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
/// The map is **stricter** than the `CaseFold` row it replaces — three named
/// token pairs instead of "any case-only difference" — so no cell that used to
/// be compared is now folded away.
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
// why this one item opts out (the only `rustfmt::skip` in the tree).
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
    row("load",              "daily",              CaseFold,  2, 93, Evidence::BinsTsv),
    row("load",              "enabled",            BoolFold,  1, 49629, Evidence::BinsTsv),
    row("load",              "spectrum",           CaseFold,  2, 48, Evidence::BinsTsv),
    row("load",              "status",             CaseFold,  2, 49629, Evidence::BinsTsv),
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
pub const NORM_ROWS: usize = 161;
/// Count lock, [`NormRule::BoolFold`]: bin 1's 75 pairs − 5 pure-echo + 7
/// WP-RP1.
const NORM_BOOL_FOLD_ROWS: usize = 77;
/// Count lock, [`NormRule::CaseFold`]: bin 2's 61 pairs (59 case + 2 trail) + 2
/// WP-RP1, **− 1** for `invcontrol.voltage_curvex_ref`, which RP2.2 re-typed as
/// an `EnumSynonym` row ([`VOLTAGE_CURVEX_REF_SYNONYMS`]).
const NORM_CASE_FOLD_ROWS: usize = 62;
/// Count lock, [`NormRule::ArrayForm`]: bin 4's 21 pairs − 4 (module doc).
const NORM_ARRAY_FORM_ROWS: usize = 17;
/// Count lock, [`NormRule::EnumSynonym`]: **5** since RP2.2 — bin 3's 8 pairs
/// minus the 4 the dossier routed elsewhere (module doc §"RP2.2's routing of
/// bin 3"; the routing itself is `props_r4133_replay::RP22_ROUTING`), plus the
/// one bin-2-labelled pair whose off-bin cells are a real enum spelling
/// ([`VOLTAGE_CURVEX_REF_SYNONYMS`]).
const NORM_ENUM_SYNONYM_ROWS: usize = 5;

/// **The r4133 props display floor — RP2.4's slot, deliberately empty here.**
///
/// `None` means numeric tokens compare **exactly** (`rel = abs = 0`), which is
/// what RP2.1 ships: the plan orders the floor's derivation into RP2.4 (from
/// the vendored in-scope numeric extract, inside the measured empty band
/// `(6.43e-5, 1e-3)`), and RP2.1 introduces **no** tolerance anywhere. The hook
/// is named now so RP2.4 lands a derived number plus its
/// `tests/TOLERANCE_NOTES.md` section, not a mechanism.
///
/// It is consulted **only** from [`numbers_match`], which has exactly two
/// callers — [`NormRule::ArrayForm`]'s per-token compare (through
/// [`tokens_match`]) and [`claim_value`]'s fourth link, which applies it to a
/// whole scalar cell. **Both are r4133-only**: the first because only the
/// r4133 arm of `PropsPolicy::normalize` reaches the table, the second because
/// `claim_value` refuses every channel but [`PropsChannel::R4133`] (the RP2.1
/// audit fix — before it the census annotated capi rows through this chain, so
/// a floor landing in RP2.4 would have been consulted on the capi channel too).
/// It is never read by `tol_for` and touches no `Tolerances` field (plan §1.2
/// last bullet, §1.3 "No tolerance tier moves").
const R4133_DISPLAY_FLOOR: Option<f64> = None;

/// **The floor slot, read back** — the RP2.1 part-C replay's chain needs a
/// *named* fourth link (`shape allowlist → normalization → echo table → display
/// floor`) and must read the real slot rather than restate `None`: when RP2.4
/// lands a derived number, the replay's floor link starts claiming through the
/// same constant the comparator uses, instead of describing a floor that is no
/// longer there. `props_r4133_replay` pins that it is `None` today, so RP2.4
/// cannot move it without re-reading that accounting.
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

/// Do two numeric tokens denote the same number? Exact today
/// ([`R4133_DISPLAY_FLOOR`] is `None`); RP2.4 widens this one function.
fn numbers_match(a: f64, b: f64) -> bool {
    match R4133_DISPLAY_FLOOR {
        None => a == b,
        Some(rel) => (a - b).abs() <= rel * a.abs().max(b.abs()),
    }
}

/// One array token: by value when both sides parse as `f64`
/// (`0` == `0.0`, `1E-005` == `0.00001`), otherwise ASCII-case-insensitively.
fn tokens_match(a: &str, b: &str) -> bool {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => numbers_match(x, y),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EchoCategory {
    /// r4133's `GetPropertyValue` has no arm for the index, so it answers the
    /// `InitPropertyValues` **default** string that no code ever refreshed
    /// (`Version8/Source/General/DSSObject.pas:112-115`).
    EchoDefault,
    /// Same fallthrough, but the store holds the **last parse string** the deck
    /// wrote, which the live value has since moved away from (e.g.
    /// `relay.reset`'s `'0.20'`).
    EchoParse,
    /// Not an echo: the two engines genuinely mean different things by the
    /// property, and the port's meaning is the correct one.
    LiveSemanticsDiffer,
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
    /// The r4133 `Version8/Source` unit:line that proves the category (the
    /// missing `GetPropertyValue` arm and/or the `InitPropertyValues` line),
    /// plus — for a row whose ours-value has no capi witness — the name of the
    /// expected-value pin test that holds it (plan §1.2).
    pub cite: &'static str,
}

/// **The echo-exclusion table — created EMPTY by RP2.1, filled by RP2.3.**
///
/// The plan orders it that way (§0: "`PROPS_ECHO_R4133` is created **empty** in
/// RP2.1; its rows land in RP2.3"), and the emptiness is load-bearing rather
/// than a placeholder: RP2.1's job is to prove the *normalization* half claims
/// bins 1/2/4 **without** any exclusion helping it. Every cell this table will
/// eventually mask is, today, an unclaimed example row that part C's replay
/// declares for RP2.3 by name.
///
/// Expected magnitude at RP2.3: ~50-70 rows (bin 5's 45 pairs, bin 7's 12 echo
/// pairs, the nine bin-1 echo pairs, minus whatever RP2.2 routes elsewhere).
/// RP2.3 also wires the consult into `compare_prop_lists` right after the
/// normalization seam — RP2.1 deliberately does not, so no cell can be masked
/// before a cited row exists to mask it.
pub const PROPS_ECHO_R4133: &[EchoRow] = &[];

/// Count lock for [`PROPS_ECHO_R4133`] — zero, and asserted, so the "created
/// empty" half of RP2.1's scope is a test rather than a comment.
const ECHO_ROWS: usize = 0;

/// Is `(class, prop)` value-excluded on the r4133 channel? Always `false` while
/// [`PROPS_ECHO_R4133`] is empty; RP2.3 lands the rows and the call site.
pub fn echo_excluded(class: &str, prop: &str) -> bool {
    PROPS_ECHO_R4133
        .iter()
        .any(|r| r.class.eq_ignore_ascii_case(class) && r.prop.eq_ignore_ascii_case(prop))
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
    /// [`PROPS_ECHO_R4133`] excludes the pair's value compare (RP2.3).
    Echo,
    /// The two sides are numbers within [`R4133_DISPLAY_FLOOR`] (RP2.4).
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
    if echo_excluded(class, prop) {
        return Some(ValueClaim::Echo);
    }
    match (display_floor(), rust.parse::<f64>(), oracle.parse::<f64>()) {
        (Some(_), Ok(a), Ok(b)) if numbers_match(a, b) => Some(ValueClaim::DisplayFloor),
        _ => None,
    }
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

    /// The process-global live accounting, summed: `(visits, hits)`.
    ///
    /// The offline tests below assert this pair is **unchanged** across their
    /// body rather than that it is zero. The distinction is not pedantry: the
    /// counters are per-process statics shared with whatever else the binary
    /// runs, so once RP4.1 unmasks the r4133 property path a gate test in the
    /// same process will legitimately move them, and an absolute-zero (or
    /// [`assert_norm_rows_are_live`]) assertion inside a unit test would start
    /// failing on test-ordering rather than on anything real. The LIVE half of
    /// the accounting is asserted where it belongs — once, at the end of the
    /// gate (`corpus_gate.rs`). Measured here first: RP2.1 part D's scratch
    /// probe drove `compare_all_properties` on the r4133 channel in a harness
    /// test binary and reddened exactly these three tests.
    fn counter_totals() -> (usize, usize) {
        (
            NORM_VISITS.iter().map(|c| c.load(AtomicOrd::Relaxed)).sum(),
            NORM_HITS.iter().map(|c| c.load(AtomicOrd::Relaxed)).sum(),
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
        assert_eq!(
            PROPS_ECHO_R4133.len(),
            ECHO_ROWS,
            "RP2.3 fills the echo table, not RP2.1"
        );
    }

    /// Every row's citation triple is well-formed and matches its rule: the
    /// table owns bins 1, 2, 3 and 4 and nothing else (RP2.1 landed 1/2/4, RP2.2
    /// added 3), and each bin has exactly one rule kind. (The replay
    /// cross-checks `(bin, cells)` against the vendored files themselves — this
    /// is the in-module half.)
    #[test]
    fn every_row_cites_a_bin_its_rule_owns() {
        // The ONE documented exception, kept as a list so it stays countable:
        // a pair whose `bins.tsv` LABEL is 2 but whose off-bin cells are a real
        // enum spelling the live r4133 getter prints
        // (`VOLTAGE_CURVEX_REF_SYNONYMS`; vendored README §"A pair's bin is a
        // label, not a per-cell classification"). Anything else must match.
        const ENUM_ON_A_NON_BIN3_PAIR: &[(&str, &str, u8)] =
            &[("invcontrol", "voltage_curvex_ref", 2)];
        let mut exceptions = 0;
        for r in PROPS_NORM_R4133 {
            if let Some((_, _, bin)) = ENUM_ON_A_NON_BIN3_PAIR
                .iter()
                .find(|(c, p, _)| *c == r.class && *p == r.prop)
            {
                assert_eq!((r.rule.tag(), r.bin), ("EnumSynonym", *bin));
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
            ENUM_ON_A_NON_BIN3_PAIR.len(),
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
        // DISCRIMINATION 2 — a wrong number, at any magnitude.
        assert!(!claimed("line", "ratings", "[ 400]", "[401,]"));
        assert!(!claimed("line", "ratings", "[ 600 700]", "[600,700.007,]"));
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
        // No display floor in RP2.1: numeric tokens compare EXACTLY.
        assert!(
            R4133_DISPLAY_FLOOR.is_none(),
            "RP2.4 derives the floor, not RP2.1"
        );
        assert!(!numbers_match(400.0, 400.000_000_1));
        assert!(numbers_match(1e-5, 0.00001));
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

    /// **The four shipped rows fold exactly their cited census spellings, and
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
        // …and it is STRICTER than the CaseFold row it replaced: a case-only
        // difference outside the three mapped ordinals no longer folds.
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

    /// **Every shipped `EnumSynonym` map is injective on its r4133 side**, and
    /// its rows really carry the map the module doc names.
    ///
    /// One r4133 token mapping to two of our spellings would let the rule equate
    /// two different values — the one thing a rule may never do — and no
    /// sample-based accept/refuse test can see it. Checked structurally instead,
    /// so a future row cannot introduce it.
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
                }
            }
        }
        assert_eq!(rows, NORM_ENUM_SYNONYM_ROWS);
        // The two maps are distinct objects with distinct contents — the trap
        // DISCRIMINATION 3 above pins behaviorally, pinned structurally here.
        assert_eq!(
            SCAN_TYPE_SYNONYMS,
            &[("Positive", "Pos"), ("Zero", "Zero")][..]
        );
        assert_eq!(
            SEQUENCE_TYPE_SYNONYMS,
            &[("Positive", "Pos"), ("Negative", "Neg")][..]
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
        let before = counter_totals();
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
        // live — see [`counter_totals`] for why this is a delta, not a zero.
        assert_eq!(
            counter_totals(),
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
        let before = counter_totals();
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
            counter_totals(),
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
        let before = counter_totals();
        for (class, prop, rust, oracle, want) in [
            ("Capacitor", "Enabled", "Yes", "true", Some("BoolFold")),
            ("Transformer", "Conn", "wye", "wye ", Some("CaseFold")),
            ("Line", "Ratings", "[ 400]", "[400,]", Some("ArrayForm")),
            // Unclaimed: no row / an echo cell inside a row we hold / a real
            // difference / an empty render.
            ("Foo", "Bar", "Yes", "true", None),
            ("Recloser", "EventLog", "No", "", None),
            ("Line", "Ratings", "[ 400]", "[401,]", None),
            ("RegControl", "FwdThreshold", "100", "", None),
        ] {
            let got = claim_value(PropsChannel::R4133, class, prop, rust, oracle);
            match (got, want) {
                (Some(ValueClaim::Normalization(rule)), Some(tag)) => {
                    assert_eq!(rule.tag(), tag, "{class}.{prop}");
                    assert_eq!(got.unwrap().tag(), format!("normalized-by-{tag}"));
                }
                (None, None) => {}
                _ => panic!("{class}.{prop}: chain answered {got:?}, expected {want:?}"),
            }
            // The seam and the query say the same thing about this cell.
            let (a, e) = normalize_with(PROPS_NORM_R4133, class, prop, rust, oracle);
            assert_eq!(
                a == e,
                matches!(got, Some(ValueClaim::Normalization(_))),
                "{class}.{prop}: the seam and the offline chain disagree"
            );
        }
        // The two later links are inert in RP2.1, and for a reason each: the
        // echo table is empty, the floor is `None` (so a numeric pair that is
        // genuinely different is NOT swallowed).
        assert_eq!(
            claim_value(
                PropsChannel::R4133,
                "RegControl",
                "FwdThreshold",
                "100",
                "800"
            ),
            None
        );
        assert!(display_floor().is_none());
        assert_eq!(counter_totals(), before, "the chain query moved a counter");
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
            // The floor's slot: two numbers that a future R4133_DISPLAY_FLOOR
            // could fold. `None` here must not depend on the floor being `None`.
            ("Load", "pf", "0.88", "0.880001"),
        ] {
            assert_eq!(
                claim_value(PropsChannel::CapiV0145, class, prop, rust, oracle),
                None,
                "{class}.{prop}: the capi channel must reach no link of the r4133 chain"
            );
        }
        // …and the three r4133-claimed ones really are claimed, so the assertion
        // above is a channel statement and not a "nothing is ever claimed" one.
        for (class, prop, rust, oracle) in [
            ("Capacitor", "Enabled", "Yes", "true"),
            ("Transformer", "Conn", "wye", "wye "),
            ("Line", "Ratings", "[ 400]", "[400,]"),
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

    /// The echo table ships empty and its consult answers `false` for
    /// everything — including the pairs RP2.3 is expected to fill it with.
    #[test]
    fn the_echo_table_is_empty_until_rp2_3() {
        assert!(PROPS_ECHO_R4133.is_empty());
        for (class, prop) in [
            ("regcontrol", "idleforward"),
            ("regcontrol", "fwdthreshold"),
            ("relay", "reset"),
            ("energymeter", "peakcurrent"),
        ] {
            assert!(!echo_excluded(class, prop));
        }
    }
}
