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
//! Plain mode is the knob's baseline forever — it is what reproduces RP0.1.
//! RP2.1 adds a second, *disposition* mode (`DSS_PROPS_CENSUS=claims`) over the
//! same walk; [`Mode`] is the seam it extends. Be precise about what RP0.2 did
//! and did NOT build for it (RP0.2 audit): the per-cell decision
//! (`harness::value_verdict`) and the per-element walk
//! (`harness::collect_element_divergences`, pinned against the gate's
//! `compare_prop_lists` by a biconditional test) ARE shared seams; `Mode` today
//! reaches only [`write_artifacts`], and threading it — plus a ledger view —
//! from `scheduler::run_props_census` through `census_one` into the walk is
//! RP2.1's own work, deliberately not pre-built as untestable plumbing.
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

use crate::harness::{CensusBlindSpots, PropCensusRow, PropsChannel};

/// Census mode. Plain is the permanent baseline; RP2.1 adds `Claims`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// `DSS_PROPS_CENSUS=1` — the un-normalized comparator, RP0.1's measurement.
    Plain,
}

impl Mode {
    /// Parse the env value. An unknown value is a LOUD failure, never a silent
    /// fallback to plain: `DSS_PROPS_CENSUS=claims` must fail until RP2.1 builds
    /// it, rather than quietly hand back a plain census labelled as claims.
    pub(crate) fn from_env(raw: &str) -> Mode {
        match raw {
            "1" => Mode::Plain,
            "claims" => panic!(
                "DSS_PROPS_CENSUS=claims (the disposition mode) is R4133_PROPS_PLAN.md RP2.1's \
                 deliverable and does not exist yet — use DSS_PROPS_CENSUS=1 for the plain census"
            ),
            other => panic!(
                "DSS_PROPS_CENSUS={other:?} is not a census mode; the only accepted value is \
                 `1` (the plain, un-normalized census)"
            ),
        }
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
}

impl Row {
    /// A row for a whole-case error (no step, no element).
    pub(crate) fn error(case: &str, channel: PropsChannel, kind: RowKind) -> Row {
        Row {
            case: case.to_string(),
            channel,
            step: NO_STEP,
            kind,
        }
    }

    /// Lift one [`PropCensusRow`] from the harness walk onto a (case, step).
    pub(crate) fn from_harness(
        case: &str,
        channel: PropsChannel,
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

    fn to_json(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("case".into(), json!(self.case));
        m.insert("channel".into(), json!(self.channel.tag()));
        m.insert("kind".into(), json!(self.kind.tag()));
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
    /// `(pair, rust, oracle) -> cells`, split structural/numeric — the
    /// untruncated spelling inventory `examples_full.txt` prints.
    examples: BTreeMap<ExampleKey, usize>,
    diverging_cases: BTreeSet<String>,
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
                *self
                    .examples
                    .entry((numeric, key, rust.clone(), oracle.clone()))
                    .or_default() += 1;
            }
            RowKind::Shape {
                element,
                rust_count,
                oracle_count,
                rust_only,
                oracle_only,
            } => {
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
        let mut rows: Vec<(&ExampleKey, &usize)> = self.examples.iter().collect();
        rows.sort_by(|a, b| {
            let (an, ap, ar, ao) = a.0;
            let (bn, bp, br, bo) = b.0;
            an.cmp(bn)
                .then(ap.cmp(bp))
                .then(b.1.cmp(a.1))
                .then((ar, ao).cmp(&(br, bo)))
        });
        for ((_, pair, rust, oracle), count) in rows {
            s.push_str(&format!(
                "{pair} | {} | {} | {count}\n",
                quote(rust),
                quote(oracle)
            ));
        }
        s
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
fn stream_census(header: &Value, rows: &[Row], w: &mut impl Write) {
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
            serde_json::to_string(&r.to_json()).expect("serialize census row")
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
        "mode": match mode { Mode::Plain => "plain" },
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
                "mode": match mode { Mode::Plain => "plain" },
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
    stream_census(&header, &rows, &mut w);
    w.flush().expect("flush census");
    drop(w);

    eprintln!(
        "props_census [{}]: {} case(s){}, {} row(s) -> {} + {}",
        match mode {
            Mode::Plain => "plain",
        },
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
        eprintln!(
            "  {}: {} structural pair(s), {} numeric pair(s), {} shape class(es), \
             {} case(s) with a divergence, {} cell(s) uncomparable behind a desynchronized \
             name list, {} element(s) skipped whole ({} cell(s))",
            ch.tag(),
            e.structural.len(),
            e.numeric.len(),
            e.shape.len(),
            e.diverging_cases.len(),
            b.unaligned_cells,
            b.skipped_elements,
            b.skipped_element_cells,
        );
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
            stream_census(&header, &rows, &mut buf);
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
        assert!(std::panic::catch_unwind(|| Mode::from_env("claims")).is_err());
        assert!(std::panic::catch_unwind(|| Mode::from_env("yes")).is_err());
    }
}
