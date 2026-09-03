//! `DSS_PROPS_CENSUS` — the permanent property-census knob
//! (`R4133_PROPS_PLAN.md` RP0.2).
//!
//! The 2026-08-08 G1.1 measurement that killed `GOLDEN_REBASE` G1.1 (and whose
//! extracts are vendored at `tests/corpus/props_r4133/`) was a *scratch* test,
//! reverted with the rest of that attempt. This module makes it a permanent,
//! opt-in diagnostic: `DSS_PROPS_CENSUS=1` arms the corpus-gate binary's
//! `corpus_gate_props_census` test, which walks every live case (respecting
//! `DSS_GATE_ONLY`), captures `all_properties` on **both** channels regardless
//! of the §1.1 gate masks, compares with the plain (un-normalized) comparator in
//! *collect-don't-panic* mode, and writes `tmp/props_census.json` plus the
//! per-channel extracts in the RP0.1 format. It **asserts nothing** — a
//! divergence is data, never a failure.
//!
//! It is a SEPARATE `#[test]`, never a diversion of
//! `corpus_gate_all_cases_match_engines`: the var must not be able to turn the
//! one mandatory live comparison into a green no-op (RP0.2 audit).
//!
//! The walk itself lives in [`crate::scheduler::run_props_census`] (it needs the
//! scheduler's case model, thread pool and channel transports); this module owns
//! the row model and the artifact writers.
//!
//! Plain mode is the knob's baseline forever — it is what reproduces RP0.1, and
//! nothing RP2.1 added moves a byte of it (the claims-only columns and files are
//! gated on [`Mode::annotates`]).
//!
//! ## The disposition mode (`DSS_PROPS_CENSUS=claims`, RP2.1)
//!
//! Same walk, same rows, each **value** row annotated with what the r4133 value
//! policy does with that cell — [`Disposition`]: `normalized-by-<rule>` /
//! `echo-row` / `under-floor` / `ledger-hit` / `UNCLAIMED`. It is the per-cell
//! accounting `R4133_PROPS_PLAN.md` RP4.1's acceptance reads ("zero UNCLAIMED
//! cells"), and the offline replay's per-**spelling** completeness proof
//! (`crates/dss-core/tests/props_r4133_replay.rs`) is its counterpart: replay
//! before the unmask, claims census after.
//!
//! Every verdict comes from a **shipped** predicate — `props_norm::claim_value`
//! for the harness links, `LedgerView::property_scope_keys` for the ledger one —
//! never a copy of them (RP0.2: a drifting copy would corrupt RP4.1's read
//! silently). The mode still asserts nothing about the data.
//!
//! Both channels' rows are annotated, and the verdict is **channel-scoped**:
//! three of the four links are r4133 mechanisms and answer nothing on capi
//! (`claim_value` takes the channel), while the ledger link is a real capi
//! mechanism and is applied on both. So a capi row is `ledger-hit` or
//! `UNCLAIMED` and never `normalized-by-*`/`echo-row`/`under-floor` — the
//! measured capi zero is the contract, not a property of today's data
//! ([`Disposition::for_value`], RP2.1 audit round).
//!
//! It also carries the in-scope flag the plain mode does not need
//! (`engines ∈ {"both", "r4133"}` — the vendored README's §"The in-scope
//! filter"), so its tallies are directly comparable with `bins.tsv`'s
//! `cells_in_scope` column.
//!
//! ## Artifacts
//!
//! * `tmp/props_census.json` — `{"channels": {...}, "census": [ … ]}`, one row
//!   per divergent cell, sorted by `(case, channel, step, element, prop)`.
//!   Row `kind` ∈ `value_structure` · `value_numeric` (carries `max_rel`) ·
//!   `shape_count` · `oracle_error` · `rust_error` · `rust_missing_element`.
//!   The first four are exactly the vendored census's kinds; the last two are
//!   defensive (the 2026-08-08 walk produced none) because a knob that must not
//!   fail still has to record what went wrong.
//!   **One column is new**: `channel`. The vendored census is r4133-only, so its
//!   rows are this file's `channel == "r4133"` rows with that key dropped.
//!   The header also stamps `gate_only` — the `DSS_GATE_ONLY` filter the run was
//!   bounded by, `null` for the full live population — so a family-bounded
//!   artifact set can never be read as a complete census.
//!   The per-channel `channels` block reports what the walk could NOT look at
//!   (metadata, never a census row):
//!   - `unaligned_cells` — cells no index-ordered comparison could reach because
//!     the two name lists had desynchronized earlier in the element (a shape gap,
//!     or a re-ordering). That number is the reason the vendored value population
//!     of the five shape-gap classes is a LOWER BOUND (e.g. `windgen.dynout` /
//!     `windgen.enabled` sit past r4133's `usermodel`/`userdata` insertion and
//!     are invisible to the census until WP-RP1 closes the gap).
//!   - `skipped_elements` / `skipped_element_cells` — elements dropped WHOLE
//!     before any cell was looked at (the capi-only Recloser/Relay skip). Their
//!     cells are in neither the rows nor `unaligned_cells`, so a channel with
//!     `unaligned_cells: 0` is not thereby complete.
//!   - `heterogeneous_shape_classes` — classes whose `shape_count` rows are not
//!     all the same shape, i.e. the ones `shape.txt`'s one-row-per-class format
//!     cannot represent (0 on the 2026-08-08 census; a nonzero value also prints
//!     a loud banner).
//! * `tmp/props_census/<channel>/structural_pairs.txt`, `numeric_pairs.txt`,
//!   `examples_full.txt`, `shape.txt`, `summary.json` — the RP0.1 extract formats
//!   (`tests/corpus/props_r4133/README.md` §"Row formats"), per channel.
//! * `tmp/props_census/run.json` — mode, `gate_only`, cases walked, rows: the
//!   same provenance stamp next to the extracts a reader actually diffs.
//! * **claims mode only**, per channel: `claims.txt` — `examples_full.txt`'s
//!   rows in the same order plus `count_in_scope` and the disposition;
//!   `claims_unclaimed_pairs.txt` — one row per pair still carrying an
//!   `UNCLAIMED` cell, the work list RP2.2/RP2.3/RP2.4 read (and, from RP2.4
//!   on, what RP3's remaining sub-steps read);
//!   `claims_summary.json` — the per-disposition cell tallies (every
//!   disposition, zeros included) plus `mixed_disposition_spellings`, the
//!   spellings whose cells disagreed and were folded to the weakest verdict.
//!
//! Two vendored extracts are deliberately NOT re-derived here, because the plain
//! census does not carry what they need: `bins.tsv` needs the §1.1 bin policy
//! (RP2.1's disposition mode owns it) and the three `*_in_scope` files need the
//! in-scope case filter. The README's blanket "these same extracts" sentence is
//! wider than that — recorded as an RP0.2 finding in STATUS, not silently
//! papered over.
//!
//! Deliberate format deviations from the vendored copies, all cosmetic and all
//! outside the acceptance (which compares cell populations, not bytes): the
//! extracts are written LF (the vendored copies keep their generator's CRLF),
//! the example-column header is labelled by the channel that produced it (the
//! vendored files are r4133-only), `shape.txt` rows are sorted by class (the
//! vendored file is in first-appearance order), and `summary.json`'s keys come
//! out in `serde_json`'s sorted order.
//!
//! One more, and it is a *value* deviation on exactly one row: [`quote`] escapes
//! backslashes in EVERY extract, while the vendored generator escaped them in
//! `structural_pairs.txt` (Python `repr`) but NOT in `examples_full.txt` or the
//! `*_in_scope` files — so the vendored copies spell the same `storage.dynadll`
//! cell two different ways (`'C:\\Users\\…'` vs `'C:\Users\…'`). One quoting for
//! all extracts is the fix; a consumer of the vendored files must unescape
//! `structural_pairs.txt` and must not unescape `examples_full.txt` (recorded as
//! an RP0.2 finding in STATUS — it is a live trap for RP2.1's replay).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::harness::props_norm::{self, ValueClaim};
use crate::harness::{CensusBlindSpots, PropCensusRow, PropsChannel};

/// Census mode. Plain is the permanent baseline; `Claims` is RP2.1's
/// disposition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// `DSS_PROPS_CENSUS=1` — the un-normalized comparator, RP0.1's measurement.
    Plain,
    /// `DSS_PROPS_CENSUS=claims` — the SAME walk and the same rows, each
    /// annotated with the r4133 policy chain's verdict for that cell
    /// ([`Disposition`]). See [`Disposition`] for why annotating the plain
    /// population is the same statement as re-walking with the armed policy.
    Claims,
}

impl Mode {
    /// Parse the env value. An unknown value is a LOUD failure, never a silent
    /// fallback to plain.
    pub(crate) fn from_env(raw: &str) -> Mode {
        match raw {
            "1" => Mode::Plain,
            "claims" => Mode::Claims,
            other => panic!(
                "DSS_PROPS_CENSUS={other:?} is not a census mode; the accepted values are \
                 `1` (the plain, un-normalized census) and `claims` (the disposition mode, \
                 R4133_PROPS_PLAN.md RP2.1)"
            ),
        }
    }

    /// The mode tag the artifacts stamp.
    pub(crate) fn tag(self) -> &'static str {
        match self {
            Mode::Plain => "plain",
            Mode::Claims => "claims",
        }
    }

    /// Does this mode annotate rows with a [`Disposition`]?
    pub(crate) fn annotates(self) -> bool {
        matches!(self, Mode::Claims)
    }
}

/// **What the r4133 value policy does with one divergent cell** — the per-cell
/// accounting RP4.1's acceptance reads ("zero UNCLAIMED cells", plan §RP4.1).
///
/// The vocabulary is RP0.2's, exactly: `normalized-by-<rule>` / `echo-row` /
/// `under-floor` / `ledger-hit` / `UNCLAIMED`. It annotates **value cells** —
/// the population `bins.tsv` counts and the one RP4.1 reads.
///
/// The chain's first link, RP1's **shape allowlist**, has no tag here because it
/// acts strictly upstream of everything the census records: `filter_015x` drops
/// an allowlisted Rust-side prop before the walk compares a value OR builds a
/// name set, so neither a value row nor a surviving `shape_count` row can be
/// claimed by it — a `shape_count` row in this census is by construction a gap
/// the allowlist did **not** close (WP-RP1's residual). Shape rows are therefore
/// counted on their own in the claims summary, not dispositioned; the offline
/// exerciser of the allowlist rows is the RP2.1 replay over the frozen
/// `shape.txt`.
///
/// **Why annotating the PLAIN population is the same statement as re-walking
/// with the armed policy.** The claims mode runs the identical walk (the plain
/// policy, i.e. the raw comparator) and asks
/// [`props_norm::claim_value`] about every row it produced. A cell the
/// armed seam would have equalised is exactly a cell that (a) differs raw — or
/// the census would not have a row for it — and (b) a table row claims; those
/// are the two conjuncts of `lookup_claim`, so the annotated population and the
/// armed walk's "what disappeared" are the same set, cell for cell (pinned by
/// `props_norm::tests::the_value_chain_resolves_in_order_and_agrees_with_the_seam`).
/// Annotating rather than re-walking is what lets ONE run report both the raw
/// census (RP0.1's baseline, unchanged) and the disposition of every cell in it.
///
/// **The variant order is load-bearing**, not cosmetic: it runs from the
/// strongest claim to no claim at all, so `Ord`'s `max` is "the weakest link" —
/// which is how [`ChannelExtracts::ingest`] folds two cells of one spelling that
/// the per-(case, channel) ledger link dispositioned differently. Reordering
/// these variants changes that aggregation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Disposition {
    /// [`props_norm::PROPS_NORM_R4133`] folds the two spellings
    /// (RP2.1). The payload is the rule's tag. **r4133 rows only.**
    Normalized(&'static str),
    /// [`props_norm::PROPS_ECHO_R4133`] excludes the pair (RP2.3; 81 rows since
    /// that sub-step landed, minus what `props_norm::ECHO_CARVE_OUTS` takes back
    /// out of them cell by cell). **r4133 rows only.**
    Echo,
    /// The two sides are numbers inside the r4133 display floor — RP2.4's
    /// derived `2e-4` relative (`props_norm::R4133_DISPLAY_FLOOR`), i.e. one
    /// live double printed by two `Format('%[-].Ng', …)` getters.
    /// **r4133 rows only.**
    UnderFloor,
    /// A `property`-scoped ledger entry NAMES the cell (plan §1.1(e)). This was
    /// structurally zero on the r4133 channel while the staging rule kept those
    /// entries out of the tree; RP4.1 landed the eight staged ones (2026-09-03),
    /// so the tag carries r4133 cells now.
    LedgerHit,
    /// Nothing in the chain claims it. This is the bucket RP2.2/RP2.3/RP2.4/RP3
    /// still owe rows for, and the one RP4.1's acceptance requires to be empty.
    Unclaimed,
}

impl Disposition {
    pub(crate) fn tag(self) -> String {
        match self {
            Disposition::Normalized(rule) => format!("normalized-by-{rule}"),
            Disposition::Echo => "echo-row".to_string(),
            Disposition::UnderFloor => "under-floor".to_string(),
            Disposition::LedgerHit => "ledger-hit".to_string(),
            Disposition::Unclaimed => "UNCLAIMED".to_string(),
        }
    }

    /// Every disposition the summary reports, in chain order — so a tally
    /// prints its zeros too: a mechanism that claimed nothing must be visible
    /// rather than absent, or a reader cannot tell "leaned on for nothing" from
    /// "not reported at all". (All four chain links carry a value since RP2.4,
    /// and `ledger-hit` — structurally zero on the r4133 channel while the
    /// staging rule held those entries back — carries cells since RP4.1 landed
    /// the eight staged entries, 2026-09-03.)
    pub(crate) fn all() -> Vec<Disposition> {
        let mut v: Vec<Disposition> = ["BoolFold", "CaseFold", "ArrayForm", "EnumSynonym"]
            .into_iter()
            .map(Disposition::Normalized)
            .collect();
        v.extend([
            Disposition::Echo,
            Disposition::UnderFloor,
            Disposition::LedgerHit,
            Disposition::Unclaimed,
        ]);
        v
    }

    /// **The chain, resolved for one value cell of one CHANNEL.** The harness
    /// half is the shipped [`props_norm::claim_value`] (normalization → echo →
    /// floor, the same predicates the live seam uses); the ledger link is
    /// appended here, last and only for a cell the harness left unclaimed,
    /// because the ledger lives in this binary and is per (case, channel).
    ///
    /// **The channel is a parameter, and it is not decoration.** This census
    /// walks both channels and annotates the rows of both, while three of the
    /// four links are r4133 mechanisms: `claim_value` therefore refuses every
    /// channel but `R4133` (plan mechanic (b) at the measurement layer, pinned
    /// in `props_norm`), and what remains on capi is the **ledger** link, which
    /// is a real capi mechanism — the gate's own capi property compare applies
    /// `property`-scoped entries (`makeposseq-cuf-applied-capi-props`), and the
    /// full-population claims run measures 11 such hits. Before the RP2.1 audit
    /// round this function was channel-blind, so a capi divergence whose
    /// spelling an r4133 rule folds would have been reported
    /// `normalized-by-<rule>` while the live capi comparator still failed on it.
    fn for_value(
        channel: PropsChannel,
        element: &str,
        prop: &str,
        rust: &str,
        oracle: &str,
        ledger_named: &BTreeSet<(String, String)>,
    ) -> Disposition {
        let class = element.split('.').next().unwrap_or("");
        if let Some(claim) = props_norm::claim_value(channel, class, prop, rust, oracle) {
            return match claim {
                ValueClaim::Normalization(rule) => Disposition::Normalized(rule.tag()),
                ValueClaim::Echo => Disposition::Echo,
                ValueClaim::DisplayFloor => Disposition::UnderFloor,
            };
        }
        if ledger_named.contains(&(element.to_lowercase(), prop.to_lowercase())) {
            return Disposition::LedgerHit;
        }
        Disposition::Unclaimed
    }
}

/// The `step` value carried internally by a row that belongs to no solve step
/// (the two error kinds).
///
/// It is never serialized — [`Row::to_json`]'s two error arms emit `case` /
/// `channel` / `kind` / `detail` and no `step`, which is exactly the key set the
/// vendored census's five `oracle_error` rows carry (`['case','detail','kind']`,
/// re-measured 2026-08-22). The constant exists only to keep [`Row::sort_key`]
/// total, and `-1` sorts every error row ahead of that case's step-0 rows.
/// (An earlier comment here claimed `-1` was "what the vendored census uses";
/// it uses no `step` on those rows at all — RP0.2 audit, retracted.)
const NO_STEP: i64 = -1;

/// What one census row says.
#[derive(Debug, Clone)]
pub(crate) enum RowKind {
    /// A divergent property value. `max_rel` is `Some` iff the two renderings
    /// share a numeric skeleton (`value_numeric`), `None` otherwise
    /// (`value_structure`).
    Value {
        element: String,
        prop: String,
        rust: String,
        oracle: String,
        max_rel: Option<f64>,
    },
    /// The two property-NAME sets differ.
    Shape {
        element: String,
        rust_count: usize,
        oracle_count: usize,
        rust_only: Vec<String>,
        oracle_only: Vec<String>,
    },
    /// The capture names an element the Rust engine does not have.
    RustMissingElement { element: String },
    /// The channel could not produce a capture for this case.
    OracleError { detail: String },
    /// The Rust side panicked while being walked (compile/solve/capture).
    RustError { detail: String },
}

impl RowKind {
    fn tag(&self) -> &'static str {
        match self {
            RowKind::Value { max_rel: None, .. } => "value_structure",
            RowKind::Value { .. } => "value_numeric",
            RowKind::Shape { .. } => "shape_count",
            RowKind::RustMissingElement { .. } => "rust_missing_element",
            RowKind::OracleError { .. } => "oracle_error",
            RowKind::RustError { .. } => "rust_error",
        }
    }

    fn element(&self) -> &str {
        match self {
            RowKind::Value { element, .. }
            | RowKind::Shape { element, .. }
            | RowKind::RustMissingElement { element } => element,
            RowKind::OracleError { .. } | RowKind::RustError { .. } => "",
        }
    }

    fn prop(&self) -> &str {
        match self {
            RowKind::Value { prop, .. } => prop,
            _ => "",
        }
    }

    /// Does this row count as a measured divergence (as opposed to an error)?
    fn is_divergence(&self) -> bool {
        matches!(
            self,
            RowKind::Value { .. } | RowKind::Shape { .. } | RowKind::RustMissingElement { .. }
        )
    }
}

/// One census row: a divergent cell (or an error) on one (case, channel, step).
#[derive(Debug, Clone)]
pub(crate) struct Row {
    pub(crate) case: String,
    pub(crate) channel: PropsChannel,
    pub(crate) step: i64,
    pub(crate) kind: RowKind,
    /// Is this row's case one the gate actually compares on the r4133 channel
    /// — `engines ∈ {"both", "r4133"}` (the vendored README's §"The in-scope
    /// filter")? Written before RP4.1 as "will compare"; since that unmask
    /// (2026-09-03) it is the present tense. Carried on every row in every mode; only the
    /// claims artifacts report the split, which is what makes a claims run
    /// comparable with `bins.tsv`'s `cells_in_scope` column.
    pub(crate) in_scope: bool,
    /// The claims mode's per-cell verdict (`None` in plain mode, and on rows
    /// that are not value cells).
    pub(crate) disposition: Option<Disposition>,
}

impl Row {
    /// A row for a whole-case error (no step, no element).
    pub(crate) fn error(case: &str, channel: PropsChannel, in_scope: bool, kind: RowKind) -> Row {
        Row {
            case: case.to_string(),
            channel,
            step: NO_STEP,
            kind,
            in_scope,
            disposition: None,
        }
    }

    /// Lift one [`PropCensusRow`] from the harness walk onto a (case, step).
    pub(crate) fn from_harness(
        case: &str,
        channel: PropsChannel,
        in_scope: bool,
        step: usize,
        row: PropCensusRow,
    ) -> Row {
        let kind = match row {
            PropCensusRow::Value {
                element,
                prop,
                rust,
                oracle,
                max_rel,
            } => RowKind::Value {
                element,
                prop,
                rust,
                oracle,
                max_rel,
            },
            PropCensusRow::Shape {
                element,
                rust_count,
                oracle_count,
                rust_only,
                oracle_only,
            } => RowKind::Shape {
                element,
                rust_count,
                oracle_count,
                rust_only,
                oracle_only,
            },
            PropCensusRow::MissingElement { element } => RowKind::RustMissingElement { element },
        };
        Row {
            case: case.to_string(),
            channel,
            step: step as i64,
            kind,
            in_scope,
            disposition: None,
        }
    }

    /// Annotate this row with the policy chain's verdict for **this row's
    /// channel** — the claims mode's whole added content. Value rows only: see
    /// [`Disposition`] for why a `shape_count` row has no link to reach, and an
    /// error row no cell.
    ///
    /// The channel comes off the row itself ([`Row::channel`]), which is what
    /// makes the capi arm of a claims run the identity plus the ledger — see
    /// [`Disposition::for_value`].
    pub(crate) fn annotate(&mut self, ledger_named: &BTreeSet<(String, String)>) {
        if let RowKind::Value {
            element,
            prop,
            rust,
            oracle,
            ..
        } = &self.kind
        {
            self.disposition = Some(Disposition::for_value(
                self.channel,
                element,
                prop,
                rust,
                oracle,
                ledger_named,
            ));
        }
    }

    /// The deterministic census order: `(case, channel, step, element, prop)`.
    /// Restricted to one channel this is exactly the vendored census's order.
    fn sort_key(&self) -> (&str, &'static str, i64, &str, &str) {
        (
            &self.case,
            self.channel.tag(),
            self.step,
            self.kind.element(),
            self.kind.prop(),
        )
    }

    /// `mode` gates the two claims-only columns: the plain census's rows keep
    /// exactly the key set RP0.2 froze (a plain artifact must stay diffable
    /// against the vendored 2026-08-08 census forever), and the disposition
    /// mode adds `disposition` + `in_scope` to them.
    fn to_json(&self, mode: Mode) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("case".into(), json!(self.case));
        m.insert("channel".into(), json!(self.channel.tag()));
        m.insert("kind".into(), json!(self.kind.tag()));
        if mode.annotates() {
            m.insert("in_scope".into(), json!(self.in_scope));
            if let Some(d) = self.disposition {
                m.insert("disposition".into(), json!(d.tag()));
            }
        }
        match &self.kind {
            RowKind::Value {
                element,
                prop,
                rust,
                oracle,
                max_rel,
            } => {
                m.insert("element".into(), json!(element));
                m.insert("prop".into(), json!(prop));
                m.insert("rust".into(), json!(rust));
                m.insert("oracle".into(), json!(oracle));
                if let Some(r) = max_rel {
                    m.insert("max_rel".into(), json!(r));
                }
                m.insert("step".into(), json!(self.step));
            }
            RowKind::Shape {
                element,
                rust_count,
                oracle_count,
                rust_only,
                oracle_only,
            } => {
                m.insert("element".into(), json!(element));
                m.insert("rust_count".into(), json!(rust_count));
                m.insert("oracle_count".into(), json!(oracle_count));
                m.insert("rust_only".into(), json!(rust_only));
                m.insert("oracle_only".into(), json!(oracle_only));
                m.insert("step".into(), json!(self.step));
            }
            RowKind::RustMissingElement { element } => {
                m.insert("element".into(), json!(element));
                m.insert("step".into(), json!(self.step));
            }
            RowKind::OracleError { detail } | RowKind::RustError { detail } => {
                m.insert("detail".into(), json!(detail));
            }
        }
        Value::Object(m)
    }
}

// ---------------------------------------------------------------------------
// Extract derivation (the RP0.1 formats).
// ---------------------------------------------------------------------------

/// One `examples_full.txt` row's identity: `(is_numeric, pair, rust, oracle)`.
/// `is_numeric` leads because the extract prints every structural pair before
/// the first numeric one.
type ExampleKey = (bool, String, String, String);

/// What one distinct spelling accumulates. `cells` is what
/// `examples_full.txt` prints in every mode; the other two are the claims
/// mode's added columns (and stay `0`/`None` in plain mode, where nothing
/// annotates a row).
#[derive(Default)]
struct ExampleAcc {
    cells: usize,
    cells_in_scope: usize,
    /// The chain's verdict for this spelling — the **weakest** verdict any cell
    /// of the key got (see [`ChannelExtracts::ingest`] for why that is an
    /// aggregation and not a constant).
    disposition: Option<Disposition>,
    /// Did two cells of this key get DIFFERENT verdicts? Only the ledger link
    /// can do that (it is per (case, channel) by design), and the count of such
    /// spellings rides the claims summary and the banner so the aggregation is
    /// visible rather than silent.
    mixed_disposition: bool,
}

/// A `(class, prop)` pair accumulator — the unit both pair extracts print.
struct PairAcc {
    example_rust: String,
    example_oracle: String,
    rows: usize,
    max_rel: f64,
}

/// The shape of ONE element's property-table gap: the identity `shape.txt`
/// prints one row per CLASS for.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone)]
struct ShapeAcc {
    rust_count: usize,
    oracle_count: usize,
    rust_only: Vec<String>,
    oracle_only: Vec<String>,
}

/// Truncate to `n` **characters** (never bytes — a value may be non-ASCII).
fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Single-quote a value the way the vendored extracts do (Python `repr`).
///
/// The 2026-08-08 generation asserted that no census value contains `'`, CR, LF
/// or TAB — which is what makes the documented quote-anchored parse regex
/// (`^(\S+) \| '([^']*)' \| '([^']*)' \| (.*)$`,
/// `tests/corpus/props_r4133/README.md` §"Row formats") safe. A re-measurement
/// is not bound by that past assertion, so those characters are escaped rather
/// than emitted raw. On every value the census can contain today this is
/// byte-identical to `repr`.
///
/// The apostrophe is the one arm that deliberately DEVIATES from `repr`, and
/// the reason is the regex (RP0.2 audit): `repr` would either switch the
/// delimiters to `"` or emit `\'`, and `[^']*` stops at a literal `'` no matter
/// what precedes it — both forms silently mis-parse. `\x27` carries the
/// character losslessly through a single-quoted field without ever writing one,
/// and `\\` is already escaped, so the sequence is unambiguous.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\x27"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// `%.2e` with Python's two-digit signed exponent (`5.85e-06`, `1.00e+06`).
fn sci2(x: f64) -> String {
    let s = format!("{x:.2e}");
    match s.split_once('e') {
        Some((mantissa, exp)) => {
            let e: i32 = exp.parse().unwrap_or(0);
            let sign = if e < 0 { '-' } else { '+' };
            format!("{mantissa}e{sign}{:02}", e.abs())
        }
        None => s,
    }
}

/// The `(class, prop)` pair key of a value row: the class is the element name's
/// prefix before the **first** `.` (element names may contain `|` and further
/// dots), the prop is the oracle's spelling — both lowercased.
fn pair_of(element: &str, prop: &str) -> String {
    let class = element.split('.').next().unwrap_or("");
    format!("{}.{}", class.to_lowercase(), prop.to_lowercase())
}

/// Everything one channel's extracts need, accumulated in census order.
#[derive(Default)]
struct ChannelExtracts {
    structural: BTreeMap<String, PairAcc>,
    numeric: BTreeMap<String, PairAcc>,
    /// Per class, the FIRST shape row's identity (what `shape.txt` prints —
    /// one row per class, the vendored format) plus every DISTINCT identity
    /// seen for that class. A class with more than one distinct shape cannot be
    /// summarized by a single row; the extract stays in the vendored format and
    /// [`ChannelExtracts::heterogeneous_shape_classes`] reports the count so the
    /// loss is loud instead of silent (the lossless record is the census JSON,
    /// which keeps every individual `shape_count` row).
    shape: BTreeMap<String, (ShapeAcc, BTreeSet<ShapeAcc>)>,
    /// `(pair, rust, oracle) -> counts`, split structural/numeric — the
    /// untruncated spelling inventory `examples_full.txt` prints, plus the
    /// claims mode's per-spelling verdict.
    examples: BTreeMap<ExampleKey, ExampleAcc>,
    diverging_cases: BTreeSet<String>,
    /// Claims mode: `shape_count` rows seen (WP-RP1's residual) and the
    /// in-scope half of them. Not dispositioned — see [`Disposition`].
    shape_rows: usize,
    shape_rows_in_scope: usize,
}

impl ChannelExtracts {
    fn ingest(&mut self, row: &Row) {
        if row.kind.is_divergence() {
            self.diverging_cases.insert(row.case.clone());
        }
        match &row.kind {
            RowKind::Value {
                element,
                prop,
                rust,
                oracle,
                max_rel,
            } => {
                let key = pair_of(element, prop);
                let numeric = max_rel.is_some();
                let (map, rel) = match max_rel {
                    Some(r) => (&mut self.numeric, *r),
                    None => (&mut self.structural, 0.0),
                };
                let acc = map.entry(key.clone()).or_insert_with(|| PairAcc {
                    example_rust: rust.clone(),
                    example_oracle: oracle.clone(),
                    rows: 0,
                    max_rel: 0.0,
                });
                acc.rows += 1;
                if rel > acc.max_rel {
                    acc.max_rel = rel;
                }
                let ex = self
                    .examples
                    .entry((numeric, key, rust.clone(), oracle.clone()))
                    .or_default();
                ex.cells += 1;
                if row.in_scope {
                    ex.cells_in_scope += 1;
                }
                // **Aggregate, never assert.** The first three links of the
                // chain ARE a pure function of `(class, prop, rust, oracle)`,
                // but the fourth is not: a `property`-scoped ledger entry is per
                // **(case, channel)** by design (`ledger.rs`, and
                // `scheduler.rs` resolves a `LedgerView` per case), so one
                // spelling that an entry names in case A and no entry names in
                // case B legitimately answers `LedgerHit` and `Unclaimed` for
                // the same key. The population is one deck away from it already
                // — `gictransformer.r2 | '0.09522' | '0.12696'` occurs in two
                // cases, each with its own entry. An assert here would abort a
                // 56 s / 1e6-row walk on legitimate data, in a mode whose whole
                // contract is that it asserts NOTHING (RP2.1 audit round).
                //
                // The aggregate is the WEAKEST verdict — `Disposition`'s
                // declaration order runs claimed → `Unclaimed`, so `max` can
                // only ever move a spelling toward the work list, never out of
                // it. The conflict itself is counted and reported.
                if let Some(d) = row.disposition {
                    ex.disposition = Some(match ex.disposition {
                        Some(prev) => {
                            ex.mixed_disposition |= prev != d;
                            prev.max(d)
                        }
                        None => d,
                    });
                }
            }
            RowKind::Shape {
                element,
                rust_count,
                oracle_count,
                rust_only,
                oracle_only,
            } => {
                self.shape_rows += 1;
                if row.in_scope {
                    self.shape_rows_in_scope += 1;
                }
                let class = element.split('.').next().unwrap_or("").to_lowercase();
                let seen = ShapeAcc {
                    rust_count: *rust_count,
                    oracle_count: *oracle_count,
                    rust_only: rust_only.clone(),
                    oracle_only: oracle_only.clone(),
                };
                let slot = self
                    .shape
                    .entry(class)
                    .or_insert_with(|| (seen.clone(), BTreeSet::new()));
                slot.1.insert(seen);
            }
            _ => {}
        }
    }

    /// Classes whose `shape_count` rows are NOT all the same shape — the ones
    /// the one-row-per-class extract cannot represent. `0` on the 2026-08-08
    /// census (all five classes are homogeneous, re-verified 2026-08-22).
    fn heterogeneous_shape_classes(&self) -> usize {
        self.shape.values().filter(|(_, all)| all.len() > 1).count()
    }

    /// Spellings whose cells did **not** all get the same disposition — the
    /// per-spelling extract prints one verdict per row, so those rows print the
    /// aggregate (the weakest link) and this counts how many did. Only the
    /// per-(case, channel) ledger link can produce one; `0` on every run
    /// measured so far. Reported next to `heterogeneous_shape_classes`, and for
    /// the same reason: a summary that cannot represent something must say so.
    fn mixed_disposition_spellings(&self) -> usize {
        self.examples
            .values()
            .filter(|a| a.mixed_disposition)
            .count()
    }

    /// `class.prop | 'rust' | '<channel>' | rows` (examples cut at 40 chars —
    /// the vendored `structural_pairs.txt` cut).
    fn structural_text(&self, channel: &str) -> String {
        let mut s = format!("class.prop | rust-example | {channel}-example | rows\n");
        for (pair, acc) in &self.structural {
            s.push_str(&format!(
                "{pair} | {} | {} | {}\n",
                quote(&trunc(&acc.example_rust, 40)),
                quote(&trunc(&acc.example_oracle, 40)),
                acc.rows
            ));
        }
        s
    }

    /// `class.prop | 'rust' | '<channel>' | max_rel | rows` (examples cut at 34
    /// chars — the vendored `numeric_pairs.txt` cut).
    fn numeric_text(&self, channel: &str) -> String {
        let mut s = format!("class.prop | rust-example | {channel}-example | max_rel | rows\n");
        for (pair, acc) in &self.numeric {
            s.push_str(&format!(
                "{pair} | {} | {} | {} | {}\n",
                quote(&trunc(&acc.example_rust, 34)),
                quote(&trunc(&acc.example_oracle, 34)),
                sci2(acc.max_rel),
                acc.rows
            ));
        }
        s
    }

    /// `class.prop | 'rust' | '<channel>' | count` — one row per DISTINCT
    /// `(rust, oracle)` spelling per pair, values **untruncated**, structural
    /// pairs first then numeric, each section sorted by pair, and within a pair
    /// by descending cell count (ties broken by the `(rust, oracle)` spelling
    /// ascending — the ordering reverse-engineered from all 2 441 tied rows of
    /// the vendored file). This is RP2.1's replay input; the vendored
    /// `bins.tsv` and the three `*_in_scope` extracts are NOT re-derivable here
    /// (they need the §1.1 bin policy and the in-scope case filter, which the
    /// plain census does not carry — see the module docs).
    fn examples_text(&self, channel: &str) -> String {
        let mut s = format!("class.prop | rust | {channel} | count\n");
        for (key, acc) in self.examples_in_extract_order() {
            let (_, pair, rust, oracle) = key;
            s.push_str(&format!(
                "{pair} | {} | {} | {}\n",
                quote(rust),
                quote(oracle),
                acc.cells
            ));
        }
        s
    }

    /// The `examples_full.txt` row order: structural pairs first then numeric,
    /// each section by pair, within a pair by descending cell count with the
    /// `(rust, oracle)` spelling breaking ties ascending. Shared by
    /// [`Self::examples_text`] and the claims extract so the two files line up
    /// row for row.
    fn examples_in_extract_order(&self) -> Vec<(&ExampleKey, &ExampleAcc)> {
        let mut rows: Vec<(&ExampleKey, &ExampleAcc)> = self.examples.iter().collect();
        rows.sort_by(|a, b| {
            let (an, ap, ar, ao) = a.0;
            let (bn, bp, br, bo) = b.0;
            an.cmp(bn)
                .then(ap.cmp(bp))
                .then(b.1.cells.cmp(&a.1.cells))
                .then((ar, ao).cmp(&(br, bo)))
        });
        rows
    }

    /// **The claims extract** (`DSS_PROPS_CENSUS=claims` only):
    /// `examples_full.txt`'s rows, in the same order, plus the in-scope cell
    /// count and the chain's verdict for that spelling.
    fn claims_text(&self, channel: &str) -> String {
        let mut s =
            format!("class.prop | rust | {channel} | count | count_in_scope | disposition\n");
        for (key, acc) in self.examples_in_extract_order() {
            let (_, pair, rust, oracle) = key;
            s.push_str(&format!(
                "{pair} | {} | {} | {} | {} | {}\n",
                quote(rust),
                quote(oracle),
                acc.cells,
                acc.cells_in_scope,
                acc.disposition
                    .map(|d| d.tag())
                    .unwrap_or_else(|| "-".to_string()),
            ));
        }
        s
    }

    /// **The unclaimed inventory** — one row per pair that still carries an
    /// `UNCLAIMED` cell: `class.prop | cells | cells_in_scope | spellings`,
    /// descending by in-scope cells. This is the work list RP2.2/RP2.3/RP2.4
    /// read, and the file whose emptiness is RP4.1's acceptance.
    fn unclaimed_text(&self) -> String {
        let mut per_pair: BTreeMap<&str, (usize, usize, usize)> = BTreeMap::new();
        for ((_, pair, _, _), acc) in &self.examples {
            if acc.disposition != Some(Disposition::Unclaimed) {
                continue;
            }
            let e = per_pair.entry(pair.as_str()).or_default();
            e.0 += acc.cells;
            e.1 += acc.cells_in_scope;
            e.2 += 1;
        }
        let mut rows: Vec<_> = per_pair.into_iter().collect();
        rows.sort_by(|a, b| b.1.1.cmp(&a.1.1).then(b.1.0.cmp(&a.1.0)).then(a.0.cmp(b.0)));
        let mut s = String::from("class.prop | cells | cells_in_scope | spellings\n");
        for (pair, (cells, in_scope, spellings)) in rows {
            s.push_str(&format!("{pair} | {cells} | {in_scope} | {spellings}\n"));
        }
        s
    }

    /// Per-disposition cell tallies: `(cells, cells_in_scope, spellings,
    /// pairs)`, every disposition present even at zero.
    fn claim_tallies(&self) -> Vec<(Disposition, usize, usize, usize, usize)> {
        Disposition::all()
            .into_iter()
            .map(|d| {
                let mut pairs: BTreeSet<&str> = BTreeSet::new();
                let (mut cells, mut in_scope, mut spellings) = (0usize, 0usize, 0usize);
                for ((_, pair, _, _), acc) in &self.examples {
                    if acc.disposition != Some(d) {
                        continue;
                    }
                    cells += acc.cells;
                    in_scope += acc.cells_in_scope;
                    spellings += 1;
                    pairs.insert(pair.as_str());
                }
                (d, cells, in_scope, spellings, pairs.len())
            })
            .collect()
    }

    /// The claims summary as JSON — the machine-readable half of the banner.
    fn claims_summary(&self) -> Value {
        let mut per_disposition = serde_json::Map::new();
        let (mut total, mut total_in_scope) = (0usize, 0usize);
        for (d, cells, in_scope, spellings, pairs) in self.claim_tallies() {
            total += cells;
            total_in_scope += in_scope;
            per_disposition.insert(
                d.tag(),
                json!({
                    "cells": cells,
                    "cells_in_scope": in_scope,
                    "spellings": spellings,
                    "pairs": pairs,
                }),
            );
        }
        json!({
            "value_cells": total,
            "value_cells_in_scope": total_in_scope,
            "per_disposition": Value::Object(per_disposition),
            // Not dispositioned, and reported so their absence from the tallies
            // cannot read as "claimed" (see `Disposition`).
            "shape_rows": self.shape_rows,
            "shape_rows_in_scope": self.shape_rows_in_scope,
            // Spellings whose cells disagreed and were folded to the weakest
            // verdict (`ChannelExtracts::ingest`) — 0 on every measured run.
            "mixed_disposition_spellings": self.mixed_disposition_spellings(),
        })
    }

    /// `class: rust_count=N oracle_count=M oracle_only=[…] rust_only=[…]`.
    fn shape_text(&self) -> String {
        let list = |v: &[String]| -> String {
            format!(
                "[{}]",
                v.iter()
                    .map(|n| format!("'{n}'"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let mut s = String::new();
        for (class, (acc, _)) in &self.shape {
            s.push_str(&format!(
                "{class}: rust_count={} oracle_count={} oracle_only={} rust_only={}\n",
                acc.rust_count,
                acc.oracle_count,
                list(&acc.oracle_only),
                list(&acc.rust_only)
            ));
        }
        s
    }

    fn summary(&self) -> Value {
        json!({
            "structure_pairs": self.structural.len(),
            "numeric_pairs": self.numeric.len(),
            "cases_with_any_div": self.diverging_cases.len(),
        })
    }
}

/// `<repo>/tmp` — the same anchor the gate's other report tools use.
fn tmp_dir() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect()
}

fn write(path: &PathBuf, text: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// Write `{<header fields>, "census": [ …rows… ]}`, streaming the array one
/// compact object per line: the full census is ~1e6 rows, and building a single
/// `Value` tree plus a pretty string for it would cost gigabytes.
fn stream_census(header: &Value, rows: &[Row], mode: Mode, w: &mut impl Write) {
    let head = serde_json::to_string_pretty(header).expect("serialize census header");
    // Splice the array into the header object: drop exactly ONE closing brace
    // (`strip_suffix`, never `trim_end_matches`, which would eat the braces of a
    // `{}` on the same line too) and re-open the object for the array.
    let head = head.trim_end();
    let head = head
        .strip_suffix('}')
        .unwrap_or_else(|| panic!("census header is not a JSON object: {head}"))
        .trim_end();
    writeln!(w, "{head},\n  \"census\": [").expect("write census header");
    for (i, r) in rows.iter().enumerate() {
        let comma = if i + 1 == rows.len() { "" } else { "," };
        writeln!(
            w,
            "    {}{comma}",
            serde_json::to_string(&r.to_json(mode)).expect("serialize census row")
        )
        .expect("write census row");
    }
    writeln!(w, "  ]\n}}").expect("write census tail");
}

/// Sort, write and report every artifact. `blind` carries, per channel tag,
/// what that channel's walk could not look at (see
/// `harness::CensusBlindSpots`) — reported, never silently dropped. `gate_only`
/// is the `DSS_GATE_ONLY` filter this run was bounded by (`None` = the full
/// live population); it is stamped into the census header AND into
/// `tmp/props_census/run.json` so a family-bounded artifact set can never be
/// mistaken for a full one (RP0.2 audit). Returns the artifact root for the
/// caller's banner.
pub(crate) fn write_artifacts(
    mut rows: Vec<Row>,
    mode: Mode,
    cases: usize,
    blind: &BTreeMap<&'static str, CensusBlindSpots>,
    gate_only: Option<&str>,
) -> PathBuf {
    rows.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));

    let channels = [PropsChannel::CapiV0145, PropsChannel::R4133];
    let mut extracts: BTreeMap<&'static str, ChannelExtracts> = BTreeMap::new();
    for ch in channels {
        extracts.insert(ch.tag(), ChannelExtracts::default());
    }
    for r in &rows {
        if let Some(e) = extracts.get_mut(r.channel.tag()) {
            e.ingest(r);
        }
    }

    let tmp = tmp_dir();
    let root = tmp.join("props_census");
    for ch in channels {
        let dir = root.join(ch.tag());
        let e = &extracts[ch.tag()];
        // The example column is labelled by the channel that produced it — the
        // vendored files are r4133-only, so their `r4133-example` header is the
        // r4133 case of this (RP0.2 audit: the capi extracts used to claim to be
        // r4133 captures).
        write(
            &dir.join("structural_pairs.txt"),
            &e.structural_text(ch.tag()),
        );
        write(&dir.join("numeric_pairs.txt"), &e.numeric_text(ch.tag()));
        write(&dir.join("examples_full.txt"), &e.examples_text(ch.tag()));
        write(&dir.join("shape.txt"), &e.shape_text());
        write(
            &dir.join("summary.json"),
            &format!("{}\n", serde_json::to_string_pretty(&e.summary()).unwrap()),
        );
        // The disposition mode's own three artifacts. Written ONLY in claims
        // mode: a plain run must leave a directory a reader can diff against
        // the vendored 2026-08-08 census without subtracting files — and a
        // plain run AFTER a claims run must not leave that run's claims files
        // sitting next to its own extracts, where they would read as this run's
        // accounting. Removed BY NAME, one file at a time (never a recursive
        // delete of a directory this tool did not create).
        for (name, text) in [
            ("claims.txt", e.claims_text(ch.tag())),
            ("claims_unclaimed_pairs.txt", e.unclaimed_text()),
            (
                "claims_summary.json",
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&e.claims_summary()).unwrap()
                ),
            ),
        ] {
            let path = dir.join(name);
            if mode.annotates() {
                write(&path, &text);
            } else if path.exists() {
                std::fs::remove_file(&path)
                    .unwrap_or_else(|err| panic!("remove stale {}: {err}", path.display()));
            }
        }
    }

    let per_channel: serde_json::Map<String, Value> = channels
        .iter()
        .map(|ch| {
            let e = &extracts[ch.tag()];
            let b = blind.get(ch.tag()).copied().unwrap_or_default();
            (
                ch.tag().to_string(),
                json!({
                    "rows": rows.iter().filter(|r| r.channel == *ch).count(),
                    "structure_pairs": e.structural.len(),
                    "numeric_pairs": e.numeric.len(),
                    "shape_classes": e.shape.len(),
                    "heterogeneous_shape_classes": e.heterogeneous_shape_classes(),
                    "cases_with_any_div": e.diverging_cases.len(),
                    "unaligned_cells": b.unaligned_cells,
                    "skipped_elements": b.skipped_elements,
                    "skipped_element_cells": b.skipped_element_cells,
                    "oracle_errors": rows
                        .iter()
                        .filter(|r| r.channel == *ch
                            && matches!(r.kind, RowKind::OracleError { .. }))
                        .count(),
                    "rust_errors": rows
                        .iter()
                        .filter(|r| r.channel == *ch
                            && matches!(r.kind, RowKind::RustError { .. }))
                        .count(),
                }),
            )
        })
        .collect();

    // The full census is ~1e6 rows; building one `Value` tree and one pretty
    // string for it would cost gigabytes, so the array is STREAMED (header
    // first, then one compact object per line).
    let header = json!({
        "mode": mode.tag(),
        "gate_only": gate_only,
        "cases_walked": cases,
        "rows": rows.len(),
        "channels": Value::Object(per_channel),
    });
    // The same provenance next to the extracts themselves: a reader comparing
    // `structural_pairs.txt` against the vendored copy must be able to see, in
    // that directory, whether this run walked the full population.
    write(
        &root.join("run.json"),
        &format!(
            "{}\n",
            serde_json::to_string_pretty(&json!({
                "mode": mode.tag(),
                "gate_only": gate_only,
                "cases_walked": cases,
                "rows": rows.len(),
            }))
            .expect("serialize run.json")
        ),
    );
    let census_path = tmp.join("props_census.json");
    let _ = std::fs::create_dir_all(&tmp);
    let file = std::fs::File::create(&census_path)
        .unwrap_or_else(|e| panic!("create {}: {e}", census_path.display()));
    let mut w = std::io::BufWriter::new(file);
    stream_census(&header, &rows, mode, &mut w);
    w.flush().expect("flush census");
    drop(w);

    eprintln!(
        "props_census [{}]: {} case(s){}, {} row(s) -> {} + {}",
        mode.tag(),
        cases,
        match gate_only {
            Some(f) => format!(" (DSS_GATE_ONLY={f:?} — NOT the full population)"),
            None => String::new(),
        },
        rows.len(),
        census_path.display(),
        root.display()
    );
    for ch in channels {
        let e = &extracts[ch.tag()];
        let b = blind.get(ch.tag()).copied().unwrap_or_default();
        // The error counts ride the PRINTED line, not just `props_census.json`
        // (RP1.4 audit): a case whose channel hiccups yields ONE `oracle_error`
        // row instead of that case's divergence rows, so a run that quietly lost
        // a case used to read exactly like a complete one on stdout. Baselines
        // measured 2026-08-23 and recorded in `TESTING.md`: 5 on r4133 (the
        // #303 crash decks the vendored census carries too) and 22 on
        // `capi_v0145` (decks the 0.14.5 oracle cannot compile or solve —
        // post-0.14.5 spellings, the WindGen class, `modes:upgrade/*`). Above
        // the baseline, this run measured LESS than the recorded census and its
        // totals are not comparable with it.
        let count_kind =
            |f: fn(&Row) -> bool| rows.iter().filter(|r| r.channel == ch && f(r)).count();
        eprintln!(
            "  {}: {} structural pair(s), {} numeric pair(s), {} shape class(es), \
             {} case(s) with a divergence, {} cell(s) uncomparable behind a desynchronized \
             name list, {} element(s) skipped whole ({} cell(s)), {} oracle error(s), \
             {} rust error(s)",
            ch.tag(),
            e.structural.len(),
            e.numeric.len(),
            e.shape.len(),
            e.diverging_cases.len(),
            b.unaligned_cells,
            b.skipped_elements,
            b.skipped_element_cells,
            count_kind(|r| matches!(r.kind, RowKind::OracleError { .. })),
            count_kind(|r| matches!(r.kind, RowKind::RustError { .. })),
        );
        // The disposition tallies ride the printed line too — the claims mode's
        // whole output is this accounting, and a run whose numbers only reach
        // `claims_summary.json` is a run nobody reads.
        if mode.annotates() {
            let (mut cells, mut in_scope) = (0usize, 0usize);
            let mut parts: Vec<String> = Vec::new();
            for (d, c, s, spellings, pairs) in e.claim_tallies() {
                cells += c;
                in_scope += s;
                parts.push(format!(
                    "{}={c} ({s} in scope, {spellings} spelling(s), {pairs} pair(s))",
                    d.tag()
                ));
            }
            eprintln!(
                "  {}: claims — {cells} value cell(s) ({in_scope} in scope): {}; \
                 {} shape row(s) ({} in scope) carry no disposition (the shape allowlist \
                 acts upstream of the census)",
                ch.tag(),
                parts.join(", "),
                e.shape_rows,
                e.shape_rows_in_scope,
            );
            let mixed = e.mixed_disposition_spellings();
            if mixed > 0 {
                eprintln!(
                    "  {}: NOTE — {mixed} spelling(s) got MORE THAN ONE disposition across \
                     their cells (only the per-(case, channel) ledger link can do that); \
                     claims.txt prints the weakest of them per row. The lossless record is \
                     props_census.json's per-row `disposition`.",
                    ch.tag()
                );
            }
        }
        let het = e.heterogeneous_shape_classes();
        if het > 0 {
            eprintln!(
                "  {}: WARNING — {het} class(es) carry MORE THAN ONE distinct property-table \
                 shape; shape.txt prints one row per class and shows only the first. The \
                 lossless record is props_census.json's shape_count rows.",
                ch.tag()
            );
        }
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The extract formatters reproduce the vendored row shapes exactly — the
    /// examples are lifted verbatim from `tests/corpus/props_r4133/`.
    #[test]
    fn pair_rows_match_the_vendored_format() {
        let mut e = ChannelExtracts::default();
        e.ingest(&Row {
            case: "asymmetric:autotrans/autotrans_gic.dss".into(),
            channel: PropsChannel::R4133,
            step: 0,
            in_scope: true,
            disposition: None,
            kind: RowKind::Value {
                element: "AutoTrans.t1".into(),
                prop: "conn".into(),
                rust: "delta".into(),
                oracle: "Delta ".into(),
                max_rel: None,
            },
        });
        e.ingest(&Row {
            case: "asymmetric:autotrans/autotrans_gic.dss".into(),
            channel: PropsChannel::R4133,
            step: 0,
            in_scope: true,
            disposition: None,
            kind: RowKind::Value {
                element: "AutoTrans.t1".into(),
                prop: "kv".into(),
                rust: "7.19955785679463".into(),
                oracle: "7.1996".into(),
                max_rel: Some(5.85e-6),
            },
        });
        assert_eq!(
            e.structural_text("r4133").lines().nth(1).unwrap(),
            "autotrans.conn | 'delta' | 'Delta ' | 1"
        );
        assert_eq!(
            e.numeric_text("r4133").lines().nth(1).unwrap(),
            "autotrans.kv | '7.19955785679463' | '7.1996' | 5.85e-06 | 1"
        );
        // The header names the channel that produced the example column; the
        // vendored files are r4133 captures, so `r4133` is that case of it.
        assert_eq!(
            e.structural_text("r4133").lines().next().unwrap(),
            "class.prop | rust-example | r4133-example | rows"
        );
        assert_eq!(
            e.numeric_text("capi_v0145").lines().next().unwrap(),
            "class.prop | rust-example | capi_v0145-example | max_rel | rows"
        );
    }

    /// `examples_full.txt`: one row per DISTINCT spelling per pair, values
    /// UNtruncated, structural pairs first then numeric, within a pair by
    /// descending cell count with the `(rust, oracle)` spelling breaking ties
    /// ascending — the ordering reverse-engineered from the vendored file (all
    /// 2 441 of its tied rows obey it).
    #[test]
    fn examples_full_rows_match_the_vendored_format() {
        let mut e = ChannelExtracts::default();
        let value = |prop: &str, rust: &str, oracle: &str, max_rel: Option<f64>| Row {
            case: "asymmetric:autotrans/autotrans_gic.dss".into(),
            channel: PropsChannel::R4133,
            step: 0,
            in_scope: true,
            disposition: None,
            kind: RowKind::Value {
                element: "AutoTrans.t1".into(),
                prop: prop.into(),
                rust: rust.into(),
                oracle: oracle.into(),
                max_rel,
            },
        };
        // `autotrans.conn`: 'wye' twice, 'delta' once — descending count.
        e.ingest(&value("conn", "wye", "wye ", None));
        e.ingest(&value("conn", "wye", "wye ", None));
        e.ingest(&value("conn", "delta", "Delta ", None));
        // A tie inside `autotrans.bus`, broken by the spelling ascending.
        e.ingest(&value("bus", "b", "B", None));
        e.ingest(&value("bus", "a", "A", None));
        // A numeric pair — sorts after every structural pair, untruncated.
        let long = "[666.666666666667, 666.666666666667, 666.666666666667, ]";
        e.ingest(&value("kvas", long, "[666.667, ]", Some(1e-6)));
        assert_eq!(
            e.examples_text("r4133"),
            format!(
                "class.prop | rust | r4133 | count\n\
                 autotrans.bus | 'a' | 'A' | 1\n\
                 autotrans.bus | 'b' | 'B' | 1\n\
                 autotrans.conn | 'wye' | 'wye ' | 2\n\
                 autotrans.conn | 'delta' | 'Delta ' | 1\n\
                 autotrans.kvas | '{long}' | '[666.667, ]' | 1\n"
            )
        );
    }

    /// A class whose elements do NOT all share one property-table shape is
    /// COUNTED (and banner-reported), because `shape.txt` prints one row per
    /// class and can only show the first. The lossless record is the census
    /// JSON. All five classes of the 2026-08-08 census are homogeneous.
    #[test]
    fn heterogeneous_shape_classes_are_counted_not_swallowed() {
        let shape = |element: &str, rust_count: usize| Row {
            case: "modes:windgen/windgen_snap.dss".into(),
            channel: PropsChannel::R4133,
            step: 0,
            in_scope: true,
            disposition: None,
            kind: RowKind::Shape {
                element: element.into(),
                rust_count,
                oracle_count: 60,
                rust_only: vec![],
                oracle_only: vec!["usermodel".into()],
            },
        };
        let mut e = ChannelExtracts::default();
        e.ingest(&shape("WindGen.w1", 58));
        e.ingest(&shape("WindGen.w2", 58));
        assert_eq!(e.heterogeneous_shape_classes(), 0);
        assert_eq!(e.shape_text().lines().count(), 1);
        // A second, DIFFERENT shape in the same class: still one printed row…
        e.ingest(&shape("WindGen.w3", 59));
        assert_eq!(e.shape_text().lines().count(), 1);
        assert!(e.shape_text().contains("rust_count=58"));
        // …but the loss is now reported instead of silent.
        assert_eq!(e.heterogeneous_shape_classes(), 1);
    }

    /// The shape row prints the vendored `class: …` form, with the two name
    /// lists in the vendored order (`oracle_only` before `rust_only`).
    #[test]
    fn shape_row_matches_the_vendored_format() {
        let mut e = ChannelExtracts::default();
        e.ingest(&Row {
            case: "asymmetric:autotrans/autotrans_gic.dss".into(),
            channel: PropsChannel::R4133,
            step: 0,
            in_scope: true,
            disposition: None,
            kind: RowKind::Shape {
                element: "AutoTrans.t1".into(),
                rust_count: 52,
                oracle_count: 53,
                rust_only: vec![],
                oracle_only: vec!["xfmrcode".into()],
            },
        });
        assert_eq!(
            e.shape_text().trim_end(),
            "autotrans: rust_count=52 oracle_count=53 oracle_only=['xfmrcode'] rust_only=[]"
        );
    }

    /// Examples are cut at the vendored widths (40 structural / 34 numeric) and
    /// backslashes are escaped, exactly as `storage.dynadll` shows.
    #[test]
    fn examples_are_cut_and_escaped_like_the_vendored_files() {
        let dll = r"C:\Users\prdu001\OpenDSS\Source\DESS1\Default.dll";
        assert_eq!(
            quote(&trunc(dll, 40)),
            r"'C:\\Users\\prdu001\\OpenDSS\\Source\\DESS1\\De'"
        );
        let kvas = "[666.666666666667, 666.666666666667, ]";
        assert_eq!(
            quote(&trunc(kvas, 34)),
            "'[666.666666666667, 666.66666666666'"
        );
    }

    /// Every escape `quote` emits survives the README's quote-anchored parse
    /// regex — the apostrophe included, which is the one the census has never
    /// produced and which a Python-`repr`-faithful `\'` would silently break
    /// (`[^']*` stops at the literal quote no matter what precedes it).
    #[test]
    fn quoted_values_survive_the_documented_parse_regex() {
        // The README's rule, hand-implemented: field = `'` … up to the next `'`.
        let field = |row: &str| -> String {
            let rest = row.split_once(" | ").expect("a pair row has fields").1;
            let body = rest.strip_prefix('\'').expect("field opens with a quote");
            body[..body.find('\'').expect("field closes with a quote")].to_string()
        };
        // Left-to-right unescape — the only decoder that is unambiguous when a
        // value itself contains a backslash followed by `x27`.
        let unescape = |s: &str| -> String {
            let mut out = String::new();
            let mut it = s.chars();
            while let Some(c) = it.next() {
                if c != '\\' {
                    out.push(c);
                    continue;
                }
                match it.next().expect("an escape never ends the field") {
                    '\\' => out.push('\\'),
                    'n' => out.push('\n'),
                    'r' => out.push('\r'),
                    't' => out.push('\t'),
                    'x' => {
                        let hex: String = it.by_ref().take(2).collect();
                        assert_eq!(hex, "27", "the only \\x escape is the apostrophe");
                        out.push('\'');
                    }
                    other => panic!("unknown escape \\{other}"),
                }
            }
            out
        };
        for raw in [
            "plain",
            "don't",
            "'quoted'",
            r"C:\dir\file",
            r"\x27 not an escape",
            "line\nbreak",
            "tab\there",
        ] {
            let row = format!("class.prop | {} | '' | 1", quote(raw));
            let got = field(&row);
            assert!(
                !got.contains('\''),
                "the payload must never contain a bare quote: {got:?}"
            );
            assert_eq!(unescape(&got), raw, "escaped row: {row}");
        }
    }

    /// `%.2e` carries Python's signed two-digit exponent.
    #[test]
    fn sci2_matches_python_percent_e() {
        assert_eq!(sci2(5.85e-6), "5.85e-06");
        assert_eq!(sci2(999_999.0), "1.00e+06");
        assert_eq!(sci2(100.0), "1.00e+02");
        assert_eq!(sci2(1.356_840_117_583_752e-6), "1.36e-06");
    }

    /// The pair key takes the class as the prefix before the FIRST dot, so the
    /// merge-parallel deck's `Line.b1||b2` does not become its own class.
    #[test]
    fn pair_key_survives_the_pipe_and_dot_traps() {
        assert_eq!(pair_of("Line.b1||b2", "Ratings"), "line.ratings");
        assert_eq!(pair_of("Load.671.a", "Yearly"), "load.yearly");
    }

    /// The streamed document is valid JSON and carries every row — including
    /// when the header ends in an empty `{}` and when there are no rows at all
    /// (the two shapes a naive brace-trim would corrupt).
    #[test]
    fn streamed_census_is_valid_json() {
        let row = Row {
            case: "modes:windgen/windgen_snap.dss".into(),
            channel: PropsChannel::R4133,
            step: 0,
            in_scope: true,
            disposition: None,
            kind: RowKind::Value {
                element: "WindGen.w1".into(),
                prop: "enabled".into(),
                rust: "Yes".into(),
                oracle: "true".into(),
                max_rel: None,
            },
        };
        for (header, n) in [
            (json!({"mode": "plain", "channels": {}}), 0usize),
            (
                json!({"mode": "plain", "channels": {"r4133": {"rows": 2}}}),
                2,
            ),
        ] {
            let rows: Vec<Row> = std::iter::repeat_n(row.clone(), n).collect();
            let mut buf: Vec<u8> = Vec::new();
            stream_census(&header, &rows, Mode::Plain, &mut buf);
            let doc: Value = serde_json::from_slice(&buf).expect("streamed census parses");
            assert_eq!(doc["mode"], "plain");
            assert_eq!(doc["channels"], header["channels"]);
            let census = doc["census"].as_array().expect("census array");
            assert_eq!(census.len(), n);
            for c in census {
                assert_eq!(c["kind"], "value_structure");
                assert_eq!(c["channel"], "r4133");
                assert_eq!(c["prop"], "enabled");
            }
        }
    }

    // ------------------------------------------------- the disposition mode

    /// One value row, ready to annotate.
    fn value_row(element: &str, prop: &str, rust: &str, oracle: &str, in_scope: bool) -> Row {
        Row {
            case: "modes:windgen/windgen_snap.dss".into(),
            channel: PropsChannel::R4133,
            step: 0,
            in_scope,
            disposition: None,
            kind: RowKind::Value {
                element: element.into(),
                prop: prop.into(),
                rust: rust.into(),
                oracle: oracle.into(),
                max_rel: None,
            },
        }
    }

    /// The chain runs in the documented order and lands every cell in exactly
    /// one bucket: a folded spelling on the rule that folded it, an
    /// echo-excluded pair on `echo-row`, a cell only the ledger names on
    /// `ledger-hit`, and everything else on `UNCLAIMED`.
    ///
    /// The order is asserted where it is observable: `regcontrol.fwdthreshold`
    /// is both an echo row and (here) ledger-named, and answers `echo-row`
    /// because the harness links resolve before the ledger one.
    #[test]
    fn the_claim_chain_dispositions_every_value_cell() {
        let named: BTreeSet<(String, String)> = [
            ("regcontrol.r1".to_string(), "fwdthreshold".to_string()),
            ("gictransformer.g1".to_string(), "r2".to_string()),
        ]
        .into_iter()
        .collect();
        let cases = [
            // Folded by RP2.1's own table — the three live rule kinds.
            (
                "Capacitor.c1",
                "enabled",
                "Yes",
                "true",
                "normalized-by-BoolFold",
            ),
            (
                "Transformer.t1",
                "conn",
                "wye",
                "wye ",
                "normalized-by-CaseFold",
            ),
            (
                "Line.l1",
                "ratings",
                "[ 400]",
                "[400,]",
                "normalized-by-ArrayForm",
            ),
            // Excluded by RP2.3's echo table — on BOTH elements, because the
            // exclusion is per `(class, prop)` and not per case, and ahead of
            // the ledger link on the one the entry names.
            ("RegControl.r1", "fwdthreshold", "100", "", "echo-row"),
            ("RegControl.r2", "fwdthreshold", "100", "", "echo-row"),
            // Named by a `property` ledger scope, and by nothing earlier — an
            // RP3 root-cause pair, which no r4133 link claims.
            (
                "GICTransformer.g1",
                "r2",
                "0.09522",
                "0.12696",
                "ledger-hit",
            ),
            // Claimed by RP2.4's display floor: two renders of one double,
            // 6.431124e-05 apart (`load.pf`, the worst cell the floor takes).
            // Its pair holds no normalization and no echo row, so this is the
            // fourth link answering on its own.
            (
                "Load.l1",
                "pf",
                "0.747651914485831",
                "0.7477",
                "under-floor",
            ),
            // The bucket RP3 still owes rows for. The second is the
            // discrimination case: a real value difference inside a pair the
            // normalization table DOES hold must not be claimed. Both are also
            // far outside the display floor (2.5e-1 and 2.5e-3), which is what
            // keeps them UNCLAIMED now that the fourth link is live — and the
            // third is the same property as the `under-floor` row above, 9.99e-4
            // apart: the floor is a CELL predicate, never a pair mask.
            ("GICTransformer.g2", "r2", "0.09522", "0.12696", "UNCLAIMED"),
            ("Line.l1", "ratings", "[ 400]", "[401,]", "UNCLAIMED"),
            (
                "Load.l2",
                "pf",
                "0.747651914485831",
                "0.7484477",
                "UNCLAIMED",
            ),
            // …and RP3.9's shape, which the RP2.4 audit settlement moved OUT of
            // `under-floor`: a real census spelling 1.2e-6 apart — inside the
            // floor by the metric, and still `UNCLAIMED`, because r4133's
            // `load.kva` is no `%.Ng` render of ours (the two engines hold
            // different doubles). Without this row the census's vocabulary
            // could not tell the settlement's two populations apart.
            (
                "Load.l3",
                "kva",
                "105.263157894737",
                "105.26302971129",
                "UNCLAIMED",
            ),
        ];
        for (element, prop, rust, oracle, want) in cases {
            let mut row = value_row(element, prop, rust, oracle, true);
            row.annotate(&named);
            assert_eq!(
                row.disposition.expect("a value row is dispositioned").tag(),
                want,
                "{element}.{prop}"
            );
        }
        // A shape row and an error row carry no disposition — neither is a cell.
        let mut shape = Row {
            case: "modes:windgen/windgen_snap.dss".into(),
            channel: PropsChannel::R4133,
            step: 0,
            in_scope: true,
            disposition: None,
            kind: RowKind::Shape {
                element: "WindGen.w1".into(),
                rust_count: 58,
                oracle_count: 60,
                rust_only: vec![],
                oracle_only: vec!["usermodel".into()],
            },
        };
        shape.annotate(&named);
        assert!(shape.disposition.is_none());
    }

    /// **The capi arm of a claims run is the identity plus the ledger.**
    ///
    /// Three of the chain's four links are r4133 mechanisms; the ledger link is
    /// a real capi one (the gate applies `property`-scoped entries on that
    /// channel — `makeposseq-cuf-applied-capi-props`, 11 hits on the measured
    /// full population). So the SAME cell that folds on r4133 must come back
    /// `UNCLAIMED` on capi, while a ledger-named cell is `ledger-hit` on both.
    ///
    /// This is the RP2.1 audit round's fix: `annotate` used to ignore the row's
    /// channel, so the measured "capi normalizes nothing" was a statement about
    /// today's capi population rather than about the contract.
    ///
    /// **RP2.3 added the echo link to the same statement**, and with it the
    /// sharpest case in the table: `regcontrol.fwdthreshold` is BOTH an echo
    /// row and (here) a ledger-named cell, so the one cell resolves `echo-row`
    /// on r4133 and `ledger-hit` on capi — the chain order and the channel gate
    /// in a single row. The ledger-hit-on-BOTH case moved to
    /// `gictransformer.r2`, an RP3 root-cause pair no r4133 link claims.
    #[test]
    fn the_capi_channel_reaches_no_r4133_link() {
        let named: BTreeSet<(String, String)> = [
            ("regcontrol.r1".to_string(), "fwdthreshold".to_string()),
            ("gictransformer.g1".to_string(), "r2".to_string()),
        ]
        .into_iter()
        .collect();
        for (element, prop, rust, oracle, r4133_want, capi_want) in [
            (
                "Capacitor.c1",
                "enabled",
                "Yes",
                "true",
                "normalized-by-BoolFold",
                "UNCLAIMED",
            ),
            (
                "Transformer.t1",
                "conn",
                "wye",
                "wye ",
                "normalized-by-CaseFold",
                "UNCLAIMED",
            ),
            (
                "Line.l1",
                "ratings",
                "[ 400]",
                "[400,]",
                "normalized-by-ArrayForm",
                "UNCLAIMED",
            ),
            // RP2.3's link, and the chain order with it: the echo table
            // answers BEFORE the ledger on r4133, while capi reaches neither
            // the echo row nor any other r4133 link and falls through to the
            // ledger entry that names this very cell.
            (
                "RegControl.r1",
                "fwdthreshold",
                "100",
                "",
                "echo-row",
                "ledger-hit",
            ),
            // …and an echo-excluded cell with NO ledger entry is `UNCLAIMED` on
            // capi, i.e. still owed a compare there — which is exactly what
            // makes the capi channel the witness those rows cite.
            ("Recloser.r1", "eventlog", "No", "", "echo-row", "UNCLAIMED"),
            // RP2.4's link. It is the one where a capi leak would cost most —
            // the capi property compare runs at the case's tier floors (1e-9 to
            // 1e-6 rel, `harness::tol_for`), so a floor reaching that channel
            // would relax every numeric property of every `both` case to 2e-4
            // at once, silently and everywhere.
            (
                "Load.l1",
                "pf",
                "0.747651914485831",
                "0.7477",
                "under-floor",
                "UNCLAIMED",
            ),
            // The one link both channels share, on a pair no r4133 link claims.
            (
                "GICTransformer.g1",
                "r2",
                "0.09522",
                "0.12696",
                "ledger-hit",
                "ledger-hit",
            ),
        ] {
            for (channel, want) in [
                (PropsChannel::R4133, r4133_want),
                (PropsChannel::CapiV0145, capi_want),
            ] {
                let mut row = value_row(element, prop, rust, oracle, true);
                row.channel = channel;
                row.annotate(&named);
                assert_eq!(
                    row.disposition.expect("a value row is dispositioned").tag(),
                    want,
                    "{element}.{prop} on {}",
                    channel.tag()
                );
            }
        }
    }

    /// **Two cells of one spelling may legitimately disagree, and the census
    /// records that instead of dying on it.**
    ///
    /// The first three links are a pure function of `(class, prop, rust,
    /// oracle)`; the fourth is not — a `property` ledger scope is per (case,
    /// channel), so one spelling named by an entry in case A and by none in
    /// case B answers `ledger-hit` there and `UNCLAIMED` here. RP2.1 first
    /// shipped an `assert_eq!` on that pair, which would have aborted a 56 s /
    /// 1e6-row walk in a mode whose contract is that it asserts nothing (audit
    /// round). The aggregate is the WEAKEST verdict, and the conflict is
    /// counted.
    #[test]
    fn a_spelling_dispositioned_two_ways_aggregates_to_the_weakest() {
        let mut e = ChannelExtracts::default();
        let mut ingest = |case: &str, disposition: Disposition| {
            let mut row = value_row("GICTransformer.g1", "r2", "0.09522", "0.12696", true);
            row.case = case.to_string();
            row.disposition = Some(disposition);
            e.ingest(&row);
        };
        // Case A carries the entry, case B does not — the shape the two
        // `gic-pct-r2-honoured-*-capi-props` entries are one deck away from.
        ingest(
            "asymmetric:gic/gictransformer_gic.dss",
            Disposition::LedgerHit,
        );
        ingest("asymmetric:gic/a_third_deck.dss", Disposition::Unclaimed);
        assert_eq!(e.mixed_disposition_spellings(), 1);
        assert_eq!(
            e.claims_text("r4133"),
            "class.prop | rust | r4133 | count | count_in_scope | disposition\n\
             gictransformer.r2 | '0.09522' | '0.12696' | 2 | 2 | UNCLAIMED\n"
        );
        assert_eq!(e.claims_summary()["mixed_disposition_spellings"], 1);
        // Order of arrival must not change the aggregate.
        let mut back = ChannelExtracts::default();
        for d in [Disposition::Unclaimed, Disposition::LedgerHit] {
            let mut row = value_row("GICTransformer.g1", "r2", "0.09522", "0.12696", true);
            row.disposition = Some(d);
            back.ingest(&row);
        }
        assert_eq!(
            back.examples.values().next().unwrap().disposition,
            Some(Disposition::Unclaimed)
        );
        assert_eq!(back.mixed_disposition_spellings(), 1);
        // An UNcontested spelling is not counted, and keeps its own verdict.
        let mut clean = ChannelExtracts::default();
        for _ in 0..3 {
            let mut row = value_row("Capacitor.c1", "enabled", "Yes", "true", true);
            row.annotate(&BTreeSet::new());
            clean.ingest(&row);
        }
        assert_eq!(clean.mixed_disposition_spellings(), 0);
        assert_eq!(
            clean.claims_summary()["per_disposition"]["normalized-by-BoolFold"]["cells"],
            3
        );
    }

    /// The claims artifacts: `claims.txt` is `examples_full.txt`'s rows in the
    /// same order plus the in-scope count and the verdict, the summary tallies
    /// every disposition (zeros included) and splits in-scope, and the unclaimed
    /// inventory names the pairs that still owe a row.
    #[test]
    fn claims_artifacts_carry_the_per_cell_accounting() {
        let named = BTreeSet::new();
        let mut e = ChannelExtracts::default();
        let mut ingest = |element: &str, prop: &str, rust: &str, oracle: &str, in_scope: bool| {
            let mut row = value_row(element, prop, rust, oracle, in_scope);
            row.annotate(&named);
            e.ingest(&row);
        };
        ingest("Capacitor.c1", "enabled", "Yes", "true", true);
        ingest("Capacitor.c2", "enabled", "Yes", "true", false);
        ingest("Line.l1", "ratings", "[ 400]", "[401,]", true);
        // RP2.3: an excluded-by-echo cell is annotated like any other, so the
        // full claims run can MEASURE what the echo table masks.
        ingest("RegControl.r1", "idle", "No", "", true);
        // RP2.4: so is a cell the display floor claims — RP4.1's acceptance
        // reads this vocabulary, so `under-floor` has to reach the artifacts.
        ingest("Load.l1", "pf", "0.747651914485831", "0.7477", true);

        assert_eq!(
            e.claims_text("r4133"),
            "class.prop | rust | r4133 | count | count_in_scope | disposition\n\
             capacitor.enabled | 'Yes' | 'true' | 2 | 1 | normalized-by-BoolFold\n\
             line.ratings | '[ 400]' | '[401,]' | 1 | 1 | UNCLAIMED\n\
             load.pf | '0.747651914485831' | '0.7477' | 1 | 1 | under-floor\n\
             regcontrol.idle | 'No' | '' | 1 | 1 | echo-row\n"
        );
        // Same rows, same order as the plain extract — a reader diffs them.
        assert_eq!(
            e.examples_text("r4133").lines().collect::<Vec<_>>(),
            [
                "class.prop | rust | r4133 | count",
                "capacitor.enabled | 'Yes' | 'true' | 2",
                "line.ratings | '[ 400]' | '[401,]' | 1",
                "load.pf | '0.747651914485831' | '0.7477' | 1",
                "regcontrol.idle | 'No' | '' | 1",
            ]
        );
        assert_eq!(
            e.unclaimed_text(),
            "class.prop | cells | cells_in_scope | spellings\nline.ratings | 1 | 1 | 1\n",
            "an echo-excluded cell is CLAIMED, so it leaves the work list RP4.1 reads"
        );
        let s = e.claims_summary();
        assert_eq!(s["value_cells"], 5);
        assert_eq!(s["value_cells_in_scope"], 4);
        assert_eq!(s["per_disposition"]["normalized-by-BoolFold"]["cells"], 2);
        assert_eq!(
            s["per_disposition"]["normalized-by-BoolFold"]["cells_in_scope"],
            1
        );
        assert_eq!(s["per_disposition"]["echo-row"]["cells"], 1);
        assert_eq!(s["per_disposition"]["echo-row"]["cells_in_scope"], 1);
        assert_eq!(s["per_disposition"]["echo-row"]["pairs"], 1);
        assert_eq!(s["per_disposition"]["under-floor"]["cells"], 1);
        assert_eq!(s["per_disposition"]["under-floor"]["cells_in_scope"], 1);
        assert_eq!(s["per_disposition"]["under-floor"]["pairs"], 1);
        assert_eq!(s["per_disposition"]["UNCLAIMED"]["cells"], 1);
        assert_eq!(s["per_disposition"]["UNCLAIMED"]["pairs"], 1);
        // A mechanism that claimed nothing is present at zero, never absent —
        // the reader has to be able to tell "claimed nothing" from "not
        // reported". Since RP2.4 filled the last slot the only tags in that
        // state here are the synonym map and the ledger link.
        for tag in ["normalized-by-EnumSynonym", "ledger-hit"] {
            assert_eq!(s["per_disposition"][tag]["cells"], 0, "{tag}");
        }
    }

    /// The plain census keeps the frozen row shape: no claims columns, no claims
    /// files. RP0.2 fixed plain as the knob's baseline forever, so a claims
    /// feature that leaked into it would break every diff against the vendored
    /// 2026-08-08 census.
    #[test]
    fn plain_mode_rows_carry_no_claims_columns() {
        let mut row = value_row("Capacitor.c1", "enabled", "Yes", "true", true);
        row.annotate(&BTreeSet::new());
        let plain = row.to_json(Mode::Plain);
        assert!(plain.get("disposition").is_none());
        assert!(plain.get("in_scope").is_none());
        let claims = row.to_json(Mode::Claims);
        assert_eq!(claims["disposition"], "normalized-by-BoolFold");
        assert_eq!(claims["in_scope"], true);
        assert!(!Mode::Plain.annotates() && Mode::Claims.annotates());
    }

    /// Non-vacuity of the collecting walk itself: fed the engine's OWN property
    /// list the census is silent, and a single corrupted cell produces exactly
    /// one row of the right kind. A census that silently reported nothing would
    /// be indistinguishable from a clean one, so this is the guard that the walk
    /// really compares. Oracle-free — the "capture" is built from the engine.
    #[test]
    fn the_collecting_walk_sees_a_corrupted_cell() {
        use crate::harness::{self, PropCensusRow, PropsCap};
        use dss_core::exec::Dss;

        let mut dss = Dss::new();
        dss.command("clear");
        dss.command("new circuit.census_probe basekv=12.47 pu=1.0 phases=3");
        let props = dss
            .element_properties("Vsource.source")
            .expect("the default Vsource exists");
        let tol = harness::tol_for("micro");
        let cap = |props: Vec<(String, String)>| {
            vec![PropsCap {
                element: "Vsource.source".to_string(),
                props,
            }]
        };

        // Identical lists → no rows, no blind spots at all.
        let mut out = Vec::new();
        let blind = harness::collect_prop_divergences(
            &mut dss,
            &cap(props.clone()),
            &tol,
            PropsChannel::R4133,
            &mut out,
        );
        assert!(out.is_empty(), "a faithful capture must produce no rows");
        assert_eq!(blind, harness::CensusBlindSpots::default());

        // Corrupt ONE value (basekv) → exactly one numeric row naming it.
        let idx = props
            .iter()
            .position(|(n, _)| n.eq_ignore_ascii_case("basekv"))
            .expect("Vsource has basekv");
        let mut corrupted = props.clone();
        corrupted[idx].1 = "999".to_string();
        let mut out = Vec::new();
        harness::collect_prop_divergences(
            &mut dss,
            &cap(corrupted),
            &tol,
            PropsChannel::R4133,
            &mut out,
        );
        assert_eq!(out.len(), 1, "expected exactly one row, got {out:?}");
        match &out[0] {
            PropCensusRow::Value {
                prop,
                oracle,
                max_rel,
                ..
            } => {
                assert!(prop.eq_ignore_ascii_case("basekv"), "{prop}");
                assert_eq!(oracle, "999");
                assert!(max_rel.is_some(), "a numeric cell carries max_rel");
            }
            other => panic!("expected a value row, got {other:?}"),
        }

        // Drop a property from the "capture" → a shape row, and the index walk
        // reports the cells it can no longer line up.
        let mut short = props.clone();
        short.remove(idx);
        let mut out = Vec::new();
        let blind = harness::collect_prop_divergences(
            &mut dss,
            &cap(short),
            &tol,
            PropsChannel::R4133,
            &mut out,
        );
        assert!(
            out.iter().any(
                |r| matches!(r, PropCensusRow::Shape { rust_only, .. } if rust_only
                    .iter()
                    .any(|n| n == "basekv"))
            ),
            "a missing capture prop must produce a shape row: {out:?}"
        );
        assert!(
            blind.unaligned_cells > 0,
            "the desynchronized tail must be counted"
        );

        // A whole-element skip is COUNTED, not silent: the capi channel drops
        // Recloser/Relay entirely (their tables moved to the r4133 surface), so
        // a reader must be able to see that the channel's census has a hole even
        // though `unaligned_cells` stays 0.
        dss.command("new line.l1 bus1=sourcebus.1 bus2=b.1 r1=0.1 x1=0.1");
        dss.command("new relay.r1 monitoredobj=line.l1 monitoredterm=1");
        let relay_props = dss
            .element_properties("Relay.r1")
            .expect("the relay exists");
        let relay_cap = vec![PropsCap {
            element: "Relay.r1".to_string(),
            props: relay_props.clone(),
        }];
        let mut out = Vec::new();
        let blind = harness::collect_prop_divergences(
            &mut dss,
            &relay_cap,
            &tol,
            PropsChannel::CapiV0145,
            &mut out,
        );
        assert!(out.is_empty(), "the capi channel skips Relay whole");
        assert_eq!(blind.unaligned_cells, 0);
        assert_eq!(blind.skipped_elements, 1);
        assert_eq!(blind.skipped_element_cells, relay_props.len());
        // …and the r4133 channel really does compare it (plan §1.2).
        let mut out = Vec::new();
        let blind = harness::collect_prop_divergences(
            &mut dss,
            &relay_cap,
            &tol,
            PropsChannel::R4133,
            &mut out,
        );
        assert_eq!(blind.skipped_elements, 0);
        assert!(out.is_empty(), "a faithful capture still produces no rows");
    }

    /// **Why the knob may read properties straight after `solve`** (RP0.2
    /// audit). The live gate reaches `compare_all_properties` at the END of a
    /// step, after every other comparator; `census_one` reads them immediately
    /// after `solve`. The knob is supposed to PREDICT what the gate will see
    /// (WP-RP1's "zero `shape_count` rows", RP4.1's residual read), so the two
    /// orders must produce the same `?`-surface.
    ///
    /// Two independent arguments, and this test is the empirical half:
    ///
    ///  * By inspection of `runner::compare_capture`, every `dss.command` it
    ///    issues between `solve` and the property compare is a `? Element.Prop`
    ///    QUERY — `compare_probe` (`runner.rs:554` → `harness:compare_probe`)
    ///    and the ledger's probe/element handlers (`ledger.rs:863`); the
    ///    ledger's property rewrite (`ledger.rs:1017`, `runner.rs:641`) sits
    ///    inside the property block itself. No `export`, no `set`, no second
    ///    `solve`: `compare_export` compares two STRINGS and never touches the
    ///    engine, and every other comparator (elements, Y, monitors, meters,
    ///    discrete state, eventlog) reads through accessors.
    ///  * A `?` query is a read. The one piece of property-visible per-object
    ///    cursor state, the Transformer `ActiveWinding` behind
    ///    `skip_transformer_cursor`, is written only by property SETTERS
    ///    (`set_struct_*` in `elements/pd/transformer/accessors.rs`), never by a
    ///    getter — and `Dss::element_properties` sets the active object itself
    ///    and refreshes `Vterminal` per property, so neither selection nor a
    ///    stale terminal voltage can leak in from a preceding comparator.
    ///
    /// So the test replays that intervening traffic — probes on a 3-winding
    /// transformer (`Wdg` included), a probe on another element, and a full
    /// property read of another element (what the gate's own props walk does to
    /// every element before reaching this one) — and asserts the `?`-surface is
    /// byte-identical to the snapshot taken right after `solve`.
    #[test]
    fn the_props_surface_is_stable_across_the_gates_intervening_comparators() {
        use dss_core::exec::Dss;

        let mut dss = Dss::new();
        for cmd in [
            "clear",
            "new circuit.census_cursor basekv=115 pu=1.0 phases=3",
            "new transformer.t3w windings=3 buses=[sourcebus, b2, b3] \
             conns=[delta, wye, wye] kvs=[115, 12.47, 4.16] kvas=[1000, 1000, 1000] \
             xhl=7 xht=5 xlt=6",
            "new load.l1 bus1=b2 kv=12.47 kw=100",
            "solve",
        ] {
            dss.command(cmd);
        }
        let before = dss
            .element_properties("Transformer.t3w")
            .expect("the transformer exists");
        assert!(
            before.iter().any(|(n, _)| n.eq_ignore_ascii_case("Wdg")),
            "the deck must exercise the cursor-bearing property"
        );

        // The gate's intervening traffic, in the order `compare_capture` runs it.
        for q in [
            "? Transformer.t3w.Wdg",
            "? Transformer.t3w.kV",
            "? Transformer.t3w.tap",
            "? Transformer.t3w.buses",
            "? Load.l1.kW",
        ] {
            dss.command(q);
        }
        let _ = dss.element_properties("Load.l1").expect("the load exists");

        let after = dss
            .element_properties("Transformer.t3w")
            .expect("the transformer still exists");
        assert_eq!(
            before, after,
            "a comparator between `solve` and the property compare moved the \
             `?`-surface — the census would then stop predicting the gate"
        );
    }

    /// An unknown mode fails loudly instead of silently running a plain census.
    #[test]
    fn unknown_mode_panics() {
        assert_eq!(Mode::from_env("1"), Mode::Plain);
        assert_eq!(Mode::from_env("claims"), Mode::Claims);
        assert_eq!((Mode::Plain.tag(), Mode::Claims.tag()), ("plain", "claims"));
        assert!(std::panic::catch_unwind(|| Mode::from_env("yes")).is_err());
        assert!(std::panic::catch_unwind(|| Mode::from_env("2")).is_err());
    }
}
