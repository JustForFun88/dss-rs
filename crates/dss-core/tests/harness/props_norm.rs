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
//! consulted at all (plan mechanic (b), capi-invariance — the switch is
//! `PropsPolicy::is_r4133`, pinned by
//! `props_policy_tests::the_capi_channel_never_normalizes`).
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
//! | [`CaseFold`](NormRule::CaseFold) | 63 | bin 2 whole (59 case-only + 2 trailing-space), plus 2 WP-RP1 pairs |
//! | [`ArrayForm`](NormRule::ArrayForm) | 17 | bin 4's 21 pairs minus 4 no typed rule may claim (below) |
//! | [`EnumSynonym`](NormRule::EnumSynonym) | 0 | RP2.2 fills it; the kind ships with its shape and its self-tests |
//!
//! The four bin-4 pairs that take **no** row, each with the sub-step that owns
//! it — all four are named by the plan itself, so the RP2.1 kill criterion
//! ("a bins-1/2/4 row no typed rule can claim") does **not** fire:
//!
//! * `expcontrol.derlist` (`[PVSystem.pv]` vs `[pv]` — element vs bare name),
//!   `relay.normal`, `relay.state` (`[closed, closed, closed, ]` vs
//!   `[closed, ]` — the per-phase array render): all three are on RP2.2's
//!   enumerated S6 singleton list (plan §RP2.2);
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

use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrd};

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
    /// **Ships with ZERO rows** — RP2.2 is the sub-step that reads the eight
    /// bin-3 pairs' getters and lands them (plan §0 ordering). The kind exists
    /// here so RP2.2 adds data, not machinery; its behavior is pinned by
    /// [`tests`] against a synthetic map, the way `EVENTLOG_MASKS`' empty
    /// r4133 row set is pinned against an injected table
    /// (`harness/mod.rs`, `eventlog_mask_tests`).
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

/// Table constructor, so the 157 rows below read as data.
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
// The 157 rows are DATA — one census pair per line, columns aligned so the
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
    row("invcontrol",        "voltage_curvex_ref", CaseFold,  2, 257, Evidence::BinsTsv),
    row("invcontrol",        "voltwattyaxis",      CaseFold,  2, 9, Evidence::BinsTsv),
    row("invcontrol",        "vvc_curve1",         CaseFold,  2, 6, Evidence::BinsTsv),
    row("isource",           "bus1",               CaseFold,  2, 1, Evidence::BinsTsv),
    row("isource",           "enabled",            BoolFold,  1, 137, Evidence::BinsTsv),
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
    row("windgen",           "enabled",            BoolFold,  1, 5, Evidence::Rp13),
];

/// **Count lock, total** — asserted as an EQUALITY in both lanes
/// (`props_roundtrip.rs:62-68,238` pattern), so the table is fail-on-stale in
/// both directions: a dropped row shrinks the compare silently, an added row
/// widens what the engine is allowed to spell differently. Moving it belongs in
/// the commit that argues for the new population.
pub const NORM_ROWS: usize = 157;
/// Count lock, [`NormRule::BoolFold`]: bin 1's 75 pairs − 5 pure-echo + 7
/// WP-RP1.
const NORM_BOOL_FOLD_ROWS: usize = 77;
/// Count lock, [`NormRule::CaseFold`]: bin 2's 61 pairs (59 case + 2 trail) + 2
/// WP-RP1.
const NORM_CASE_FOLD_ROWS: usize = 63;
/// Count lock, [`NormRule::ArrayForm`]: bin 4's 21 pairs − 4 (module doc).
const NORM_ARRAY_FORM_ROWS: usize = 17;
/// Count lock, [`NormRule::EnumSynonym`]: **zero** until RP2.2.
const NORM_ENUM_SYNONYM_ROWS: usize = 0;

/// **The r4133 props display floor — RP2.4's slot, deliberately empty here.**
///
/// `None` means numeric tokens compare **exactly** (`rel = abs = 0`), which is
/// what RP2.1 ships: the plan orders the floor's derivation into RP2.4 (from
/// the vendored in-scope numeric extract, inside the measured empty band
/// `(6.43e-5, 1e-3)`), and RP2.1 introduces **no** tolerance anywhere. The hook
/// is named now so RP2.4 lands a derived number plus its
/// `tests/TOLERANCE_NOTES.md` section, not a mechanism.
///
/// It is consulted **only** from [`numbers_match`], i.e. only inside
/// [`NormRule::ArrayForm`] on the r4133 channel — never on the capi channel,
/// never by `tol_for`, and it touches no `Tolerances` field (plan §1.2 last
/// bullet, §1.3 "No tolerance tier moves").
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
    for (i, r) in PROPS_NORM_R4133.iter().enumerate() {
        let visits = NORM_VISITS[i].load(AtomicOrd::Relaxed);
        let hits = NORM_HITS[i].load(AtomicOrd::Relaxed);
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
pub fn claim_value(class: &str, prop: &str, rust: &str, oracle: &str) -> Option<ValueClaim> {
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

    /// Every row's citation triple is well-formed and matches its rule: RP2.1
    /// owns bins 1, 2 and 4 and nothing else, and each bin has exactly one
    /// rule kind. (Part C's replay cross-checks `(bin, cells)` against the
    /// vendored files themselves — this is the in-module half.)
    #[test]
    fn every_row_cites_a_bin_its_rule_owns() {
        for r in PROPS_NORM_R4133 {
            let want = match r.bin {
                1 => "BoolFold",
                2 => "CaseFold",
                4 => "ArrayForm",
                other => panic!("{}.{}: bin {other} is not RP2.1's", r.class, r.prop),
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
        // Bin 4, unclaimable by any typed rule → RP2.2 (S6 singletons) or out
        // of scope (plan §1.3).
        for (class, prop) in [
            ("expcontrol", "derlist"),
            ("relay", "normal"),
            ("relay", "state"),
            ("sensor", "kvs"),
        ] {
            assert!(
                find_row(PROPS_NORM_R4133, class, prop).is_none(),
                "{class}.{prop} takes no ArrayForm row (module doc: RP2.2 / out of scope)"
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

    // ------------------------------------------------------------- CaseFold

    /// Case-only differences fold (both engines resolve identifiers through a
    /// lowercasing `THashList`), the two upstream trailing blanks trim, and a
    /// different spelling of a different ordinal still fails.
    #[test]
    fn casefold_folds_case_and_the_two_trailing_blanks() {
        assert!(claimed("transformer", "xfmrcode", "ct25", "CT25"));
        assert!(claimed("storage", "state", "Idling", "IDLING"));
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
        // these two are bin-3 cells inside a bin-2 pair (vendored README
        // §"A pair's bin is a label"), RP2.2's `EnumSynonym` business.
        assert!(!claimed("capcontrol", "type", "PowerFactor", "pf"));
        assert!(!claimed("capcontrol", "type", "Voltage", "volt"));
        assert!(!claimed(
            "invcontrol",
            "voltage_curvex_ref",
            "RAvg",
            "avgrated"
        ));
        // ...and neither is an echo, nor a different bus/node spec.
        assert!(!claimed("relay", "switchedobj", "Line.thev", ""));
        assert!(!claimed("fault", "bus2", "b2.0", "b2.0.0.0"));
        assert!(!claimed("isource", "bus1", "b2", "b2.1"));
        // Trim is OUTER whitespace only: an inner difference still fails.
        assert!(!claimed("transformer", "xfmrcode", "ct 25", "ct25"));
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

    // ---------------------------------------------------------- EnumSynonym

    /// The kind ships with **zero** rows, so its behavior is pinned against an
    /// injected map (the `EVENTLOG_MASKS` self-test pattern): an explicit,
    /// closed, directional `(ours, theirs)` list — no wildcard, no derived
    /// synonym, and the reverse direction is not implied.
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
        // And the shipped table really is empty of them.
        assert!(
            !PROPS_NORM_R4133
                .iter()
                .any(|r| matches!(r.rule, EnumSynonym(_))),
            "RP2.2 lands the EnumSynonym rows"
        );
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
            let got = claim_value(class, prop, rust, oracle);
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
            claim_value("RegControl", "FwdThreshold", "100", "800"),
            None
        );
        assert!(display_floor().is_none());
        assert_eq!(counter_totals(), before, "the chain query moved a counter");
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
