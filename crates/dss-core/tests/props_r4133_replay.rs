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
//!  (RP1)               (RP2.1)            (empty, RP2.3)     (absent, RP2.4)
//! ```
//!
//! Every row ends up in exactly one of two states, and **the accounting is
//! total from day one**:
//!
//! * **claimed** — the first matching link of the chain recognises the two
//!   spellings as one value (RP2.1's own deliverable: bins 1, 2 and 4);
//! * **declared pending** — no link claims it *yet*, and the row carries a
//!   marker naming the sub-step whose mechanism will claim it (RP2.2, RP2.3,
//!   RP2.4, RP3) or the reason nothing will (`OutOfScope`, plan §1.3).
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

/// Example rows RP2.1's normalization claims, per rule kind.
const CLAIMED_BOOL_FOLD: usize = 114;
const CLAIMED_CASE_FOLD: usize = 442;
const CLAIMED_ARRAY_FORM: usize = 192;
/// …and in total (the three above; `EnumSynonym` ships with zero rows).
const CLAIMED_TOTAL: usize = 748;

/// Example rows claimed by the chain's other three links. All zero in RP2.1 and
/// each for a *different* reason — see [`Link`].
const CLAIMED_SHAPE_ALLOWLIST: usize = 0;
const CLAIMED_ECHO: usize = 0;
const CLAIMED_DISPLAY_FLOOR: usize = 0;

/// Rows for which **more than one** link matches. Zero while the echo table is
/// empty and the floor absent; RP2.3 makes it positive (a mixed bin-1 pair will
/// hold a `BoolFold` row *and* an echo row, and normalization wins by order).
/// It is a lock, not a structural assert, precisely so RP2.3 has to re-read the
/// first-match argument rather than silently start relying on it.
const MULTI_LINK_ROWS: usize = 0;

/// Declared-pending rows per owner: `(rows, pairs, rows on in-scope pairs)`.
/// This is what RP2.2, RP2.3, RP2.4 and RP3 each inherit.
const DECLARED_RP22: (usize, usize, usize) = (170, 26, 166);
const DECLARED_RP23: (usize, usize, usize) = (295, 72, 295);
const DECLARED_RP24: (usize, usize, usize) = (2100, 70, 2020);
const DECLARED_RP3: (usize, usize, usize) = (7, 4, 7);
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

/// Bin-7 pairs the **supplement** carries, each already read off the Pascal as
/// an echo rather than a jump, so each is RP2.3's row and not an RP3 sub-step:
///
/// * `generator.d` — `Create` sets `GenVars.D := 1.0` and never `Dpu`
///   (`Version8/Source/PCElements/generator.pas:955-971`) while
///   `InitPropertyValues` froze `Format('%-g', [GenVars.Dpu])` (`:2585`);
/// * `autotrans.pctperm` / `autotrans.repair` — `InitPropertyValues` freezes
///   `'100'` / `'36'` (`Version8/Source/PDElements/AutoTrans.pas:1958-1959`)
///   and the `GetPropertyValue` override re-renders only PD-tail slots 1 and 2
///   (`:1885-1888`), so slots 4 and 5 fall through to the `PropertyValue[]`
///   store (`General/DSSObject.pas:112-115`);
/// * `regcontrol.revthreshold` — `RegControl.pas:820-827` overrides only TapNum
///   and `:1437` freezes `PropertyValue[23] := '100'`, the sibling of
///   `remoteptratio` (`:1441`), measured at 888 cells by RP2.1 part A.
const BIN7_ECHO_SUPPLEMENT: &[&str] = &[
    "autotrans.pctperm",
    "autotrans.repair",
    "generator.d",
    "regcontrol.revthreshold",
];

/// **RP2.2's closed pair list** (plan §RP2.2): the eight bin-3 pairs — derived
/// from `bins.tsv` and cross-checked against the plan's enumeration in
/// [`the_plan_pair_lists_still_describe_the_vendored_evidence`] — plus the S6
/// singletons the triage flagged for individual source investigation. A pair on
/// this list is RP2.2's **whatever bin its cells fall in**: RP2.2 reads its
/// getter and routes it (an `EnumSynonym` row, an RP2.3 echo row, or a new
/// RP3.5+ sub-step), which is exactly the "later sub-steps move rows between
/// mechanisms" the accounting is built for.
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

/// The residue the bin map cannot decide: example rows whose **own** cell
/// classification (bin 2 or 4) differs from the mechanism that will claim them,
/// on a pair the plan's lists do not already own. One entry per pair, each with
/// the evidence that decides it.
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
    // `Dynamic_KundurDynExp` decks. **Not an echo and not a spelling**: r4133's
    // `SetDynOutput` stores the *variable* index `Get_Out_Idx` returns
    // (`General/DynamicExp.pas:411-437`, an index into `FVarNames`), while
    // `GetDynOutputStr` renders it through `Get_VarName`
    // (`:441-465`), which decodes its argument as a flat *(variable, derivative
    // slot)* index — the encoding `Get_DynamicEqVal` uses. For this deck's
    // `varnames=[Speed Mass PShaft Pterm Damp theta]`, `theta` (index 5)
    // therefore prints as `d` + `FVarNames[2]` = `dpshaft`. The dynamics are
    // unaffected (`generator.pas:2823-2849` indexes `DynamicEqVals` with the
    // same variable index the port uses), so this is an r4133 **rendering** bug
    // and the port's answer is the correct one. Declared for RP2.2's triage
    // (plan §RP2.2's third outcome), because an RP2.3 echo row on
    // `generator.dynout` would otherwise mask it silently; recorded in STATUS by
    // RP2.1 part D.
    (
        "generator.dynout",
        Owner::Rp22,
        "PCElement.pas:197-243 / DynamicExp.pas:411-465",
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
    /// **RP2.3's echo table** (`PROPS_ECHO_R4133`) — created empty by RP2.1, so
    /// it claims nothing yet. Its emptiness is load-bearing: RP2.1 must prove
    /// the normalization half claims bins 1/2/4 with no exclusion helping it.
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
    /// echo-rooted bin-7 pairs).
    Rp23,
    /// RP2.4 — the r4133 props display floor (bin 6).
    Rp24,
    /// RP3 — the four genuine value jumps that are not echo.
    Rp3,
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

/// Non-empty, non-comment lines with the CR of a CRLF file stripped; the header
/// line is asserted and dropped when `header` is `Some`.
fn data_rows(name: &str, header: Option<&str>) -> Vec<String> {
    let text = read(name);
    let mut rows: Vec<String> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
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
    let echo = props_norm::echo_excluded(&row.class, &row.prop);
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
/// 0. a pair on **RP2.2's closed list** (the eight bin-3 pairs plus the S6
///    singletons) is RP2.2's whatever bin its cells fall in — RP2.2 reads its
///    getter and routes it;
/// 1. a **numeric** row answers its in-scope-effective bin: 6 -> RP2.4's floor,
///    7 -> RP2.3 if the plan (or the vendored README) already read it off the
///    Pascal as an echo, RP3 if it is one of the four root-cause pairs;
/// 2. an unclaimed cell of a **bin-1** pair whose r4133 side is not one of the
///    eleven Delphi boolean spellings is the pair's echo half -> RP2.3;
/// 3. a cell whose own classification is **bin 5** (either side empty) -> RP2.3;
/// 4. …**bin 3** (an enum spelling) -> RP2.2's `EnumSynonym` rows;
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

    if RP22_S6.contains(&row.pair.as_str()) || (!ev.numeric && ev.bin == 3) {
        return Ok(Owner::Rp22);
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
        3 => Ok(Owner::Rp22),
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
        0,
        "RP2.2 lands the EnumSynonym rows"
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
        "PROPS_ECHO_R4133 is created empty by RP2.1; RP2.3 fills it"
    );
    assert_eq!(
        claimed(Link::DisplayFloor.tag()),
        CLAIMED_DISPLAY_FLOOR,
        "the display floor is RP2.4's; RP2.1 introduces no tolerance"
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
        "rows matched by more than one link — RP2.3's echo rows will make this positive, and the \
         first-match order (pinned by first_match_returns_the_earliest_link) is what decides them"
    );
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
        CLAIMED_TOTAL,
        "per-row hits must sum to the claimed total"
    );
}

/// The nine rows the **supplement** makes live, named: they are exactly the
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
    assert_eq!(want.len(), 9, "7 BoolFold + 2 CaseFold WP-RP1 rows");
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

/// The two chain links RP2.1 ships **inert**, pinned so the sub-steps that fill
/// them have to come back here: the echo table is empty, and the display-floor
/// slot carries no value.
#[test]
fn the_echo_table_and_the_display_floor_claim_nothing_yet() {
    assert!(
        PROPS_ECHO_R4133.is_empty(),
        "RP2.3 fills the echo table (and then MULTI_LINK_ROWS and the RP2.3 declared count move)"
    );
    assert!(
        props_norm::display_floor().is_none(),
        "RP2.4 derives the r4133 props display floor; RP2.1 introduces no tolerance anywhere"
    );
    // …and neither claims anything, asked through the same seam the chain uses.
    let corpus = Corpus::load();
    for row in &corpus.rows {
        let [_, _, echo, floor] = chain_verdicts(&corpus, row);
        assert!(!echo && !floor, "{}: an inert link claimed a row", row.pair);
    }
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
        "normalization precedes the echo table: a foldable cell of a mixed pair is a COMPARE, \
         not an exclusion"
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
}
