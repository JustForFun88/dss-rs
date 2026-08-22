//! `DSS_PROPS_CENSUS` — the permanent property-census knob
//! (`R4133_PROPS_PLAN.md` RP0.2).
//!
//! The 2026-08-08 G1.1 measurement that killed `GOLDEN_REBASE` G1.1 (and whose
//! extracts are vendored at `tests/corpus/props_r4133/`) was a *scratch* test,
//! reverted with the rest of that attempt. This module makes it a permanent,
//! opt-in diagnostic: `DSS_PROPS_CENSUS=1` on the corpus gate walks every live
//! case (respecting `DSS_GATE_ONLY`), captures `all_properties` on **both**
//! channels regardless of the §1.1 gate masks, compares with the plain
//! (un-normalized) comparator in *collect-don't-panic* mode, and writes
//! `tmp/props_census.json` plus per-channel pair/shape extracts in the RP0.1
//! format. It **asserts nothing** — a divergence is data, never a failure.
//!
//! The walk itself lives in [`crate::scheduler::run_props_census`] (it needs the
//! scheduler's case model, thread pool and channel transports); this module owns
//! the row model and the artifact writers.
//!
//! Plain mode is the knob's baseline forever — it is what reproduces RP0.1.
//! RP2.1 adds a second, *disposition* mode (`DSS_PROPS_CENSUS=claims`) over the
//! same walk; [`Mode`] is the seam it extends.
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
//!   The per-channel `channels` block additionally reports `unaligned_cells` —
//!   cells no index-ordered comparison could reach because a property-table
//!   shape gap desynchronized the two lists earlier in the element. That number
//!   is the reason the vendored value population of the five shape-gap classes
//!   is a LOWER BOUND (e.g. `windgen.dynout` / `windgen.enabled` sit past
//!   r4133's `usermodel`/`userdata` insertion and are invisible to the census
//!   until WP-RP1 closes the gap). It is metadata, never a census row.
//! * `tmp/props_census/<channel>/structural_pairs.txt`, `numeric_pairs.txt`,
//!   `shape.txt`, `summary.json` — the RP0.1 extract formats
//!   (`tests/corpus/props_r4133/README.md` §"Row formats"), per channel.
//!
//! Deliberate format deviations from the vendored copies, all cosmetic and all
//! outside the acceptance (which compares cell populations, not bytes): the
//! extracts are written LF (the vendored copies keep their generator's CRLF),
//! `shape.txt` rows are sorted by class (the vendored file is in
//! first-appearance order), and `summary.json`'s keys come out in `serde_json`'s
//! sorted order.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::harness::{PropCensusRow, PropsChannel};

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

/// The `step` value carried by a row that belongs to no solve step (the two
/// error kinds). `-1` is what the vendored census uses.
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

/// A `(class, prop)` pair accumulator — the unit both pair extracts print.
struct PairAcc {
    example_rust: String,
    example_oracle: String,
    rows: usize,
    max_rel: f64,
}

/// One class's property-table shape gap, first occurrence winning.
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
/// The 2026-08-08 generation asserted that no census value contains `'`, CR, LF
/// or TAB — which is what makes the documented quote-anchored parse regex safe —
/// but a re-measurement is not bound by that past assertion, so escape those
/// characters rather than emit a row the regex would mis-read.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
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
    shape: BTreeMap<String, ShapeAcc>,
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
                let (map, rel) = match max_rel {
                    Some(r) => (&mut self.numeric, *r),
                    None => (&mut self.structural, 0.0),
                };
                let acc = map.entry(key).or_insert_with(|| PairAcc {
                    example_rust: rust.clone(),
                    example_oracle: oracle.clone(),
                    rows: 0,
                    max_rel: 0.0,
                });
                acc.rows += 1;
                if rel > acc.max_rel {
                    acc.max_rel = rel;
                }
            }
            RowKind::Shape {
                element,
                rust_count,
                oracle_count,
                rust_only,
                oracle_only,
            } => {
                let class = element.split('.').next().unwrap_or("").to_lowercase();
                self.shape.entry(class).or_insert_with(|| ShapeAcc {
                    rust_count: *rust_count,
                    oracle_count: *oracle_count,
                    rust_only: rust_only.clone(),
                    oracle_only: oracle_only.clone(),
                });
            }
            _ => {}
        }
    }

    /// `class.prop | 'rust' | 'r4133' | rows` (examples cut at 40 chars — the
    /// vendored `structural_pairs.txt` cut).
    fn structural_text(&self) -> String {
        let mut s = String::from("class.prop | rust-example | r4133-example | rows\n");
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

    /// `class.prop | 'rust' | 'r4133' | max_rel | rows` (examples cut at 34
    /// chars — the vendored `numeric_pairs.txt` cut).
    fn numeric_text(&self) -> String {
        let mut s = String::from("class.prop | rust-example | r4133-example | max_rel | rows\n");
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
        for (class, acc) in &self.shape {
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

/// Sort, write and report every artifact. `unaligned` carries, per channel tag,
/// the number of cells a property-table shape gap made uncomparable (see
/// `harness::collect_prop_divergences`) — reported, never silently dropped.
/// Returns the artifact root for the caller's banner.
pub(crate) fn write_artifacts(
    mut rows: Vec<Row>,
    mode: Mode,
    cases: usize,
    unaligned: &BTreeMap<&'static str, usize>,
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
        write(&dir.join("structural_pairs.txt"), &e.structural_text());
        write(&dir.join("numeric_pairs.txt"), &e.numeric_text());
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
            (
                ch.tag().to_string(),
                json!({
                    "rows": rows.iter().filter(|r| r.channel == *ch).count(),
                    "structure_pairs": e.structural.len(),
                    "numeric_pairs": e.numeric.len(),
                    "shape_classes": e.shape.len(),
                    "cases_with_any_div": e.diverging_cases.len(),
                    "unaligned_cells": unaligned.get(ch.tag()).copied().unwrap_or(0),
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
        "cases_walked": cases,
        "rows": rows.len(),
        "channels": Value::Object(per_channel),
    });
    let census_path = tmp.join("props_census.json");
    let _ = std::fs::create_dir_all(&tmp);
    let file = std::fs::File::create(&census_path)
        .unwrap_or_else(|e| panic!("create {}: {e}", census_path.display()));
    let mut w = std::io::BufWriter::new(file);
    stream_census(&header, &rows, &mut w);
    w.flush().expect("flush census");
    drop(w);

    eprintln!(
        "props_census [{}]: {} case(s), {} row(s) -> {} + {}",
        match mode {
            Mode::Plain => "plain",
        },
        cases,
        rows.len(),
        census_path.display(),
        root.display()
    );
    for ch in channels {
        let e = &extracts[ch.tag()];
        eprintln!(
            "  {}: {} structural pair(s), {} numeric pair(s), {} shape class(es), \
             {} case(s) with a divergence, {} cell(s) uncomparable behind a shape gap",
            ch.tag(),
            e.structural.len(),
            e.numeric.len(),
            e.shape.len(),
            e.diverging_cases.len(),
            unaligned.get(ch.tag()).copied().unwrap_or(0)
        );
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
            e.structural_text().lines().nth(1).unwrap(),
            "autotrans.conn | 'delta' | 'Delta ' | 1"
        );
        assert_eq!(
            e.numeric_text().lines().nth(1).unwrap(),
            "autotrans.kv | '7.19955785679463' | '7.1996' | 5.85e-06 | 1"
        );
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

        // Identical lists → no rows, no unaligned cells.
        let mut out = Vec::new();
        let unaligned = harness::collect_prop_divergences(
            &mut dss,
            &cap(props.clone()),
            &tol,
            PropsChannel::R4133,
            &mut out,
        );
        assert!(out.is_empty(), "a faithful capture must produce no rows");
        assert_eq!(unaligned, 0);

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
        let unaligned = harness::collect_prop_divergences(
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
        assert!(unaligned > 0, "the desynchronized tail must be counted");
    }

    /// An unknown mode fails loudly instead of silently running a plain census.
    #[test]
    fn unknown_mode_panics() {
        assert_eq!(Mode::from_env("1"), Mode::Plain);
        assert!(std::panic::catch_unwind(|| Mode::from_env("claims")).is_err());
        assert!(std::panic::catch_unwind(|| Mode::from_env("yes")).is_err());
    }
}
