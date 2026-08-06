//! Divergence ledger for the unified corpus gate (UNIFIED_GATE_PLAN §1.3 / §4
//! Phase D). Replaces the report-only `known_diffs.json` with a **gating** input:
//! `tests/corpus/ledger.json`. Both channels gate commits; each entry pins WHERE
//! and HOW MUCH a case is allowed to diverge on a channel, and FAILS the gate
//! when stale (§5 R3 — the ledger must never become a soft-tolerance backdoor).
//!
//! Three entry kinds (§1.3):
//! * `divergence` — the case runs and is fully compared; the `match` scopes are
//!   *expected to diverge* and re-asserted inside their pinned envelope. Gate
//!   asserts (a) selected values differ ≤ envelope; (b) unselected values meet
//!   the tier floor (the untouched `harness` comparator on the remainder); (c) at
//!   least one selected value *exceeds* the tier floor — else the entry is stale.
//! * `skip` — the case is not sent to that channel at all (hard-crash decks);
//!   the OTHER channel still gates the case.
//! * `exclusion` — a proven upstream bug poisons specific comparison scopes on a
//!   channel; those scopes are skipped, everything else compared.
//!
//! Tier floors in `harness` are UNREACHABLE from here and unchanged: envelopes
//! are per-case, per-channel, per-scope MEASURED facts with a mandatory
//! cause+source, visible in the population lock (§1.4). This module wraps
//! comparator **call sites** in `runner.rs`; it never touches a `harness`
//! comparator.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use crate::harness::{ElementCap, Injection, MonitorCap, ProbeCap, PropsCap, Tolerances};
use crate::manifest::EngineChannel;

// ---------------------------------------------------------------------------
// On-disk schema (`tests/corpus/ledger.json`).
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawLedger {
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    #[allow(dead_code)]
    comment: Vec<String>,
    #[serde(default)]
    causes: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    entries: Vec<RawEntry>,
}

#[derive(Debug, Deserialize)]
struct RawEntry {
    id: String,
    case: String,
    channel: String,
    kind: String,
    #[serde(default, rename = "match")]
    match_scopes: Vec<RawScope>,
    #[serde(default)]
    cause: Option<String>,
    #[serde(default)]
    cause_ref: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    source: String,
    #[serde(default)]
    #[allow(dead_code)]
    measured: Option<Value>,
}

/// One field-granular divergence/exclusion scope inside an entry's `match`.
#[derive(Debug, Deserialize)]
struct RawScope {
    field: String,
    /// iterations: `"rust_le_oracle"`.
    #[serde(default)]
    policy: Option<String>,
    /// Step filter (0-based). Absent ⇒ every step.
    #[serde(default)]
    steps: Option<Vec<usize>>,
    /// Location selectors (per field).
    #[serde(default)]
    node_re: Option<String>,
    #[serde(default)]
    name_re: Option<String>,
    #[serde(default)]
    channel_idx: Option<usize>,
    /// element sub-channels (`currents`/`powers`/`losses`) or `exclusion`
    /// comparison scopes; eventlog/ctrlqueue unused.
    #[serde(default)]
    channels: Option<Vec<String>>,
    /// Envelope (numeric fields).
    #[serde(default)]
    max_rel: Option<f64>,
    #[serde(default)]
    max_abs: Option<f64>,
    /// Exact expected pair (probe / property / global_result / iterations).
    #[serde(default)]
    rust: Option<Value>,
    #[serde(default)]
    oracle: Option<Value>,
    /// Numeric-skeleton relative tol for probe/property values.
    #[serde(default)]
    num_rel: Option<f64>,
    /// eventlog / ctrlqueue line mask.
    #[serde(default)]
    line_re: Option<String>,
}

// ---------------------------------------------------------------------------
// Compiled runtime model.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Divergence,
    Skip,
    Exclusion,
}

/// A compiled `match` scope: regexes pre-compiled, steps as a set.
struct Scope {
    field: String,
    policy: Option<String>,
    steps: Option<BTreeSet<usize>>,
    node_re: Option<Regex>,
    name_re: Option<Regex>,
    channel_idx: Option<usize>,
    channels: Vec<String>,
    max_rel: f64,
    max_abs: f64,
    rust: Option<Value>,
    oracle: Option<Value>,
    num_rel: Option<f64>,
    line_re: Option<Regex>,
}

impl Scope {
    fn applies_step(&self, step: usize) -> bool {
        self.steps
            .as_ref()
            .map(|s| s.contains(&step))
            .unwrap_or(true)
    }
}

/// Measurement aid (§5-R3 "every envelope is a per-case MEASURED fact"): when
/// `DSS_LEDGER_MEASURE` is set, each numeric handler prints the live divergence it
/// observes for its scope, so an envelope can be sized to the measured max and the
/// `measured.max_*_seen` provenance recorded. Pure stderr side-effect gated on the
/// env var — it changes NO gating semantics (asserts + hit accounting untouched).
fn measure_note(id: &str, field: &str, diff: f64, base: f64, floor: f64) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    if *ON.get_or_init(|| std::env::var("DSS_LEDGER_MEASURE").is_ok()) {
        let rel = if base > 0.0 { diff / base } else { diff };
        eprintln!(
            "LEDGER_MEASURE {id} {field}: diff={diff:.6e} base={base:.6e} rel={rel:.3e} floor={floor:.3e}"
        );
    }
}

struct Entry {
    id: String,
    case: String,
    channel: EngineChannel,
    kind: Kind,
    scopes: Vec<Scope>,
    /// Hit accounting (shared, per-entry).
    applied: AtomicBool,
    exceeded_floor: AtomicBool,
    hits: AtomicUsize,
}

/// The loaded, compiled, hit-accounting ledger. Shared read-only across
/// scheduler threads (interior atomics record hits).
pub(crate) struct LedgerRuntime {
    causes: std::collections::BTreeMap<String, String>,
    entries: Vec<Entry>,
}

fn parse_channel(s: &str) -> Option<EngineChannel> {
    match s {
        "capi_v0145" => Some(EngineChannel::CapiV0145),
        "r4133" => Some(EngineChannel::R4133),
        _ => None,
    }
}

pub(crate) fn ledger_path() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "ledger.json",
    ]
    .iter()
    .collect()
}

impl LedgerRuntime {
    /// Load + compile the ledger. Panics with a precise message on any malformed
    /// entry (regex / channel / kind) — the structural test asserts the richer
    /// cross-manifest rules oracle-free.
    pub(crate) fn load() -> LedgerRuntime {
        let path = ledger_path();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let raw: RawLedger =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
        let mut entries = Vec::with_capacity(raw.entries.len());
        for e in raw.entries {
            let channel = parse_channel(&e.channel).unwrap_or_else(|| {
                panic!(
                    "ledger entry {:?}: unknown channel {:?} (capi_v0145|r4133)",
                    e.id, e.channel
                )
            });
            let kind = match e.kind.as_str() {
                "divergence" => Kind::Divergence,
                "skip" => Kind::Skip,
                "exclusion" => Kind::Exclusion,
                k => panic!("ledger entry {:?}: unknown kind {k:?}", e.id),
            };
            let scopes = e
                .match_scopes
                .iter()
                .map(|s| compile_scope(&e.id, s))
                .collect();
            entries.push(Entry {
                id: e.id,
                case: e.case,
                channel,
                kind,
                scopes,
                applied: AtomicBool::new(false),
                exceeded_floor: AtomicBool::new(false),
                hits: AtomicUsize::new(0),
            });
        }
        LedgerRuntime {
            causes: raw.causes,
            entries,
        }
    }

    /// An empty runtime (no entries) for the seeding report — measures the raw,
    /// ledger-free divergence.
    pub(crate) fn empty() -> LedgerRuntime {
        LedgerRuntime {
            causes: std::collections::BTreeMap::new(),
            entries: Vec::new(),
        }
    }

    /// A resolved view of the entries applicable to (`case_key`, `channel`).
    pub(crate) fn view<'a>(&'a self, case_key: &str, channel: EngineChannel) -> LedgerView<'a> {
        let idxs = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.case == case_key && e.channel == channel)
            .map(|(i, _)| i)
            .collect();
        LedgerView { rt: self, idxs }
    }

    /// Is this (case, channel) entirely skipped (a `skip` entry)? Consumed by the
    /// scheduler before dispatching the channel; records the skip hit.
    pub(crate) fn channel_is_skipped(&self, case_key: &str, channel: EngineChannel) -> bool {
        let mut skipped = false;
        for e in &self.entries {
            if e.case == case_key && e.channel == channel && e.kind == Kind::Skip {
                e.applied.store(true, Ordering::Relaxed);
                e.exceeded_floor.store(true, Ordering::Relaxed);
                e.hits.fetch_add(1, Ordering::Relaxed);
                skipped = true;
            }
        }
        skipped
    }

    /// Assert every entry was hit at least once and no divergence entry is stale
    /// (applied but nothing exceeded the tier floor). The gate fails loudly with a
    /// prune instruction otherwise (§1.3 runtime rule / §5 R3 fail-on-stale).
    pub(crate) fn assert_all_hit(&self) -> Result<(), String> {
        let mut problems = Vec::new();
        for e in &self.entries {
            let applied = e.applied.load(Ordering::Relaxed);
            let exceeded = e.exceeded_floor.load(Ordering::Relaxed);
            let hits = e.hits.load(Ordering::Relaxed);
            if !applied {
                problems.push(format!(
                    "  ledger entry `{}` ({:?}, {:?}) NEVER APPLIED — its case/scope \
                     matched nothing this run. Prune it or fix its case/selector.",
                    e.id, e.case, e.channel
                ));
                continue;
            }
            if e.kind == Kind::Divergence && !exceeded {
                problems.push(format!(
                    "  ledger entry `{}` ({:?}, {:?}) is STALE — every selected value \
                     is now within the tier floor (masks nothing). Prune it: the \
                     divergence it pinned is gone.",
                    e.id, e.case, e.channel
                ));
                continue;
            }
            if hits == 0 {
                problems.push(format!(
                    "  ledger entry `{}` ({:?}, {:?}) recorded zero hits.",
                    e.id, e.case, e.channel
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "divergence ledger has {} stale/unhit entry(ies) — the ledger fails \
                 the gate when stale (§5 R3):\n{}",
                problems.len(),
                problems.join("\n")
            ))
        }
    }

    /// Per-entry hit report line for the gate's summary.
    pub(crate) fn hit_report(&self) -> Vec<(String, EngineChannel, &'static str, usize)> {
        self.entries
            .iter()
            .map(|e| {
                let kind = match e.kind {
                    Kind::Divergence => "divergence",
                    Kind::Skip => "skip",
                    Kind::Exclusion => "exclusion",
                };
                (
                    e.id.clone(),
                    e.channel,
                    kind,
                    e.hits.load(Ordering::Relaxed),
                )
            })
            .collect()
    }

    fn cause_keys(&self) -> Vec<&String> {
        self.causes.keys().collect()
    }
}

/// Scope fields with a live runtime handler in [`LedgerView`]. The §1.3 schema
/// also names `global_result`, but NO handler exists for it yet — a scope naming
/// it (or a typo'd field) would silently never apply, so loading rejects
/// anything outside this list loudly (pre-E/F audit UGA-T4).
///
/// The last four — `y`, `y_fingerprint`, `yprim`, `meter` — are
/// **exclusion-only** ([`EXCLUSION_ONLY_FIELDS`]): they name a whole compared
/// artifact rather than a value with a natural envelope, so the only thing the
/// ledger can say about them is "this (case, channel) does not compare it".
/// `GOLDEN_REBASE_PLAN.md` G2.5 added them, because an engine fix that declines
/// an upstream bug moves the assembled admittance of the affected deck and
/// nothing else in this file could express that.
const LEDGER_FIELDS: [&str; 13] = [
    "iterations",
    "voltages",
    "injection",
    "element",
    "probe",
    "property",
    "monitor",
    "eventlog",
    "ctrlqueue",
    "y",
    "y_fingerprint",
    "yprim",
    "meter",
];

/// Fields an entry may name only with `kind: "exclusion"` — see
/// [`LEDGER_FIELDS`]. A `divergence` naming one would promise an envelope
/// nothing re-asserts.
const EXCLUSION_ONLY_FIELDS: [&str; 4] = ["y", "y_fingerprint", "yprim", "meter"];

/// Fields an `exclusion` entry may name — the mirror obligation of
/// [`LEDGER_FIELDS`], because "has a runtime handler" turned out to be
/// **kind-dependent** once G2.5 added the coarse half.
///
/// `LEDGER_FIELDS` exists so a typo'd or unimplemented field cannot sit in the
/// ledger silently never applying (pre-E/F audit UGA-T4). That guarantee has a
/// hole the moment some fields are handled for one kind only: `iterations`,
/// `property`, `eventlog` and `ctrlqueue` route through handlers that filter on
/// [`Kind::Divergence`] (the first two re-assert a pin, the last two rewrite the
/// oracle's line before the compare — none of which an exclusion can mean), so
/// an `exclusion` naming one of them would pass the loader and then do nothing.
/// Refused by `assert_structural` instead.
const EXCLUSION_FIELDS: [&str; 9] = [
    "voltages",
    "element",
    "injection",
    "monitor",
    "probe",
    "y",
    "y_fingerprint",
    "yprim",
    "meter",
];

fn compile_scope(id: &str, s: &RawScope) -> Scope {
    assert!(
        LEDGER_FIELDS.contains(&s.field.as_str()),
        "ledger entry {id:?}: scope field {:?} has no runtime handler (implemented: \
         {LEDGER_FIELDS:?}). The §1.3 field global_result needs a handler \
         implemented BEFORE it can be ledgered — an unhandled scope would silently \
         never apply.",
        s.field
    );
    let mk = |re: &Option<String>| -> Option<Regex> {
        re.as_ref().map(|r| {
            Regex::new(r).unwrap_or_else(|e| panic!("ledger entry {id:?}: bad regex {r:?}: {e}"))
        })
    };
    Scope {
        field: s.field.clone(),
        policy: s.policy.clone(),
        steps: s.steps.as_ref().map(|v| v.iter().copied().collect()),
        node_re: mk(&s.node_re),
        name_re: mk(&s.name_re),
        channel_idx: s.channel_idx,
        channels: s.channels.clone().unwrap_or_default(),
        max_rel: s.max_rel.unwrap_or(0.0),
        max_abs: s.max_abs.unwrap_or(0.0),
        rust: s.rust.clone(),
        oracle: s.oracle.clone(),
        num_rel: s.num_rel,
        line_re: mk(&s.line_re),
    }
}

// ---------------------------------------------------------------------------
// Per-(case, channel) view consumed by `compare_capture`.
// ---------------------------------------------------------------------------

/// The compiled entries applicable to one (case, channel); the object
/// `compare_capture` consults to partition each comparison field.
pub(crate) struct LedgerView<'a> {
    rt: &'a LedgerRuntime,
    idxs: Vec<usize>,
}

impl LedgerView<'_> {
    pub(crate) fn is_empty(&self) -> bool {
        self.idxs.is_empty()
    }

    fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.idxs.iter().map(move |&i| &self.rt.entries[i])
    }

    fn mark_applied(e: &Entry) {
        e.applied.store(true, Ordering::Relaxed);
        e.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn mark_exceeded(e: &Entry) {
        e.exceeded_floor.store(true, Ordering::Relaxed);
    }

    // --- whole-artifact exclusions ------------------------------------------

    /// Is the comparison of `field` (of the artifact called `name`, where the
    /// field has one) dropped for this (case, channel) by an `exclusion` entry?
    ///
    /// The coarse half of the ledger, added by `GOLDEN_REBASE_PLAN.md` G2.5.
    /// The partitioning handlers below (`voltages`, `element`, …) split one
    /// comparison into a scoped part and an unscoped remainder, which is the
    /// right shape whenever the divergence is local. It is the wrong shape for
    /// the artifacts an *engine* fix moves wholesale — the assembled system Y,
    /// its fingerprint, the affected element's YPrim, an EnergyMeter's register
    /// block — where there is no remainder and no envelope to re-assert, only
    /// "the oracle computes this from the bug we declined". Those get a plain
    /// skip, still hit-accounted (an exclusion that stops matching fails the
    /// gate as NEVER APPLIED) and still bound by `assert_structural`'s rule
    /// that only `kind: "exclusion"` may name them.
    ///
    /// `name = None` matches a scope with no `name_re`; a scope that carries a
    /// `name_re` never applies to an unnamed artifact.
    pub(crate) fn excluded(&self, field: &str, name: Option<&str>, step: usize) -> bool {
        let mut hit = false;
        for e in self.entries().filter(|e| e.kind == Kind::Exclusion) {
            for sc in &e.scopes {
                if sc.field != field || !sc.applies_step(step) {
                    continue;
                }
                let m = match (&sc.name_re, name) {
                    (Some(re), Some(n)) => re.is_match(n),
                    (Some(_), None) => false,
                    (None, _) => true,
                };
                if m {
                    Self::mark_applied(e);
                    hit = true;
                }
            }
        }
        hit
    }

    // --- iterations ---------------------------------------------------------

    /// Returns `true` if a ledger scope handled the iteration check for this
    /// step (caller then skips the standard iteration assertion). `rust`/`oracle`
    /// are the observed counts.
    pub(crate) fn iterations_handled(
        &self,
        step: usize,
        rust: i32,
        oracle: i32,
        ctx: &str,
    ) -> bool {
        for e in self.entries() {
            if e.kind != Kind::Divergence {
                continue;
            }
            for sc in &e.scopes {
                if sc.field != "iterations" || !sc.applies_step(step) {
                    continue;
                }
                if sc.policy.as_deref() == Some("rust_le_oracle") {
                    assert!(
                        rust <= oracle,
                        "{ctx}: ledger `{}` iterations rust_le_oracle violated ({rust} > {oracle})",
                        e.id
                    );
                    Self::mark_applied(e);
                    if rust != oracle {
                        Self::mark_exceeded(e);
                    }
                    return true;
                }
                if let (Some(r), Some(o)) = (&sc.rust, &sc.oracle) {
                    let re = r.as_i64().map(|x| x as i32);
                    let oe = o.as_i64().map(|x| x as i32);
                    assert_eq!(
                        Some(rust),
                        re,
                        "{ctx}: ledger `{}` expected rust iterations {re:?}, got {rust}",
                        e.id
                    );
                    assert_eq!(
                        Some(oracle),
                        oe,
                        "{ctx}: ledger `{}` expected oracle iterations {oe:?}, got {oracle}",
                        e.id
                    );
                    Self::mark_applied(e);
                    if rust != oracle {
                        Self::mark_exceeded(e);
                    }
                    return true;
                }
            }
        }
        false
    }

    // --- voltages -----------------------------------------------------------

    /// Partition node voltages by voltage scopes: envelope-check the scoped
    /// nodes here (recording hits), and return the boolean mask of nodes to KEEP
    /// for the standard `assert_complex_close` (unscoped remainder). `node_order`
    /// is the 0-based node list; `act`/`exp` are the interleaved [re,im,…] vectors.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn voltage_keep_mask(
        &self,
        step: usize,
        node_order: &[String],
        act: &[f64],
        exp: &[f64],
        tol: &Tolerances,
        ctx: &str,
    ) -> Option<Vec<bool>> {
        let voltage_scopes: Vec<(&Entry, &Scope)> = self
            .entries()
            .filter(|e| matches!(e.kind, Kind::Divergence | Kind::Exclusion))
            .flat_map(|e| {
                e.scopes
                    .iter()
                    .filter(|s| s.field == "voltages" && s.applies_step(step))
                    .map(move |s| (e, s))
            })
            .collect();
        if voltage_scopes.is_empty() {
            return None;
        }
        let mut keep = vec![true; node_order.len()];
        for (ni, name) in node_order.iter().enumerate() {
            let re = act[2 * ni];
            let im = act[2 * ni + 1];
            let ore = exp[2 * ni];
            let oim = exp[2 * ni + 1];
            let base = (ore * ore + oim * oim).sqrt();
            let diff = ((re - ore).powi(2) + (im - oim).powi(2)).sqrt();
            let floor = tol.v_abs + tol.v_rel * base;
            for (e, sc) in &voltage_scopes {
                let m = sc
                    .node_re
                    .as_ref()
                    .map(|r| r.is_match(name))
                    .unwrap_or(true);
                if !m {
                    continue;
                }
                keep[ni] = false;
                if e.kind == Kind::Exclusion {
                    Self::mark_applied(e);
                    continue;
                }
                // divergence: assert within envelope; record floor-exceed.
                let env = sc.max_abs + sc.max_rel * base;
                assert!(
                    diff <= env,
                    "{ctx}: ledger `{}` voltages @ {name}: |diff| {diff:.3e} exceeds \
                     the pinned envelope {env:.3e} (max_rel {:.2e} max_abs {:.2e}) — the \
                     divergence GREW; this is a regression or the envelope is wrong",
                    e.id,
                    sc.max_rel,
                    sc.max_abs
                );
                Self::mark_applied(e);
                measure_note(&e.id, "voltages", diff, base, floor);
                if diff > floor {
                    Self::mark_exceeded(e);
                }
            }
        }
        Some(keep)
    }

    // --- injection ----------------------------------------------------------

    /// If an injection scope applies at this step, envelope-check the whole
    /// injection vector here and return `true` (caller skips the standard
    /// `compare_injection`). Injection has no natural sub-selector, so the scope
    /// covers the whole RHS vector. Mirrors `harness::compare_injection`'s node
    /// slicing exactly (skip the ground node, take nodes 1..=n).
    pub(crate) fn injection_handled(
        &self,
        step: usize,
        dss: &dss_core::exec::Dss,
        inj: &Injection,
        tol: &Tolerances,
        ctx: &str,
    ) -> bool {
        let scope = self
            .entries()
            .filter(|e| e.kind == Kind::Divergence)
            .find_map(|e| {
                e.scopes
                    .iter()
                    .find(|s| s.field == "injection" && s.applies_step(step))
                    .map(|s| (e, s))
            });
        let Some((e, sc)) = scope else {
            return false;
        };
        let cur = dss.node_injection_currents();
        let n = inj.re.len();
        for i in 0..n {
            let (rre, rim) = (cur[i + 1].re, cur[i + 1].im);
            let base = (inj.re[i] * inj.re[i] + inj.im[i] * inj.im[i]).sqrt();
            let diff = ((rre - inj.re[i]).powi(2) + (rim - inj.im[i]).powi(2)).sqrt();
            let env = sc.max_abs + sc.max_rel * base;
            assert!(
                diff <= env,
                "{ctx}: ledger `{}` injection[{i}]: |diff| {diff:.3e} exceeds \
                 the pinned envelope {env:.3e}",
                e.id
            );
            let floor = tol.i_abs + tol.i_rel * base;
            measure_note(&e.id, "injection", diff, base, floor);
            if diff > floor {
                Self::mark_exceeded(e);
            }
        }
        Self::mark_applied(e);
        true
    }

    // --- elements (currents / powers / losses) ------------------------------

    /// Element caps rewritten so a divergence/exclusion scope's SELECTED
    /// sub-channels (currents/powers/losses) equal the Rust snapshot. The caller
    /// runs the untouched `compare_element` on the rewrite, so the selected
    /// channels compare-equal (clause (a): already re-asserted inside their pinned
    /// envelope here) while every UNSELECTED sub-channel is tier-checked by the
    /// real comparator (clause (b): the untouched harness comparator on the
    /// unscoped remainder). Returns a map keyed by lowercased element name;
    /// elements with no matching scope are absent (caller compares the original).
    ///
    /// This mirrors the `property` rewrite pattern in `runner.rs`: we never drop a
    /// value from comparison — we neutralize exactly the pinned sub-channels and
    /// let the harness compare the rest at its own tier floor.
    pub(crate) fn element_rewrites(
        &self,
        step: usize,
        snaps: &[dss_core::exec::ElementSnapshot],
        elements: &[ElementCap],
        tol: &Tolerances,
        ctx: &str,
    ) -> std::collections::BTreeMap<String, ElementCap> {
        let mut rewrites: std::collections::BTreeMap<String, ElementCap> =
            std::collections::BTreeMap::new();
        let scopes: Vec<(&Entry, &Scope)> = self
            .entries()
            .filter(|e| matches!(e.kind, Kind::Divergence | Kind::Exclusion))
            .flat_map(|e| {
                e.scopes
                    .iter()
                    .filter(|s| s.field == "element" && s.applies_step(step))
                    .map(move |s| (e, s))
            })
            .collect();
        if scopes.is_empty() {
            return rewrites;
        }
        for ec in elements {
            let lname = ec.name.to_lowercase();
            for (e, sc) in &scopes {
                let m = sc
                    .name_re
                    .as_ref()
                    .map(|r| r.is_match(&ec.name))
                    .unwrap_or(true);
                if !m {
                    continue;
                }
                let snap = snaps
                    .iter()
                    .find(|s| s.name.eq_ignore_ascii_case(&ec.name))
                    .unwrap_or_else(|| {
                        panic!("{ctx}: ledger `{}`: no Rust element {}", e.id, ec.name)
                    });
                if e.kind == Kind::Divergence {
                    // clause (a): selected sub-channels within the pinned envelope,
                    // recording floor-exceed for the staleness check.
                    envelope_element(e, sc, snap, ec, tol, ctx);
                } else {
                    // exclusion: the scoped sub-channels are simply not compared.
                    Self::mark_applied(e);
                }
                // Neutralize the selected sub-channels: overwrite them with the
                // Rust values so `compare_element` treats them as equal and
                // tier-checks the unscoped remainder (clause (b)).
                let cap = rewrites
                    .entry(lname.clone())
                    .or_insert_with(|| clone_element_cap(ec));
                rewrite_element_selected(cap, sc, snap);
            }
        }
        rewrites
    }

    // --- probes -------------------------------------------------------------

    /// Returns `true` if a probe scope handled this probe (caller skips the
    /// standard `compare_probe`). A display-precision divergence pins `num_rel`:
    /// the Rust `?`-value and the (low-precision-rendered) oracle value must agree
    /// numerically within `num_rel`, and the raw gap must exceed the tier floor
    /// (else stale). A genuine discrete VALUE jump (e.g. `normamps` 730-vs-230)
    /// pins `oracle`+`rust` with NO `num_rel` — exact-pair-numeric (§1.3 discrete):
    /// the Rust value must equal the pinned `rust` exactly (an envelope would mask
    /// real drift), stale once it converges to the oracle. A non-numeric value
    /// (enum/word) is exact-pair via the `oracle`+`rust` strings (both mandatory
    /// — §1.3 drift-safety, Phase F re-review RR-1).
    pub(crate) fn probe_handled(
        &self,
        dss: &mut dss_core::exec::Dss,
        exp: &ProbeCap,
        tol: &Tolerances,
        ctx: &str,
    ) -> bool {
        let key = format!("{}.{}", exp.element.to_lowercase(), exp.prop.to_lowercase());
        for e in self.entries() {
            if e.kind != Kind::Divergence {
                continue;
            }
            for sc in &e.scopes {
                if sc.field != "probe" {
                    continue;
                }
                let m = sc
                    .name_re
                    .as_ref()
                    .map(|r| r.is_match(&key))
                    .unwrap_or(false);
                if !m {
                    continue;
                }
                if let Some(o) = &sc.oracle {
                    let os = value_as_str(o);
                    assert!(
                        exp.value.eq_ignore_ascii_case(&os),
                        "{ctx}: ledger `{}` probe {key}: oracle value {:?} != pinned {os:?} — \
                         the upstream getter changed; re-measure",
                        e.id,
                        exp.value
                    );
                }
                dss.command(&format!("? {}.{}", exp.element, exp.prop));
                let rust_val = dss.result().to_string();
                let (rn, on) = (parse_leading_f64(&rust_val), parse_leading_f64(&exp.value));
                if let (Some(rv), Some(ov)) = (rn, on) {
                    if let Some(num_rel) = sc.num_rel {
                        // numeric-skeleton (display-precision) envelope: the Rust
                        // value and the low-precision-rendered oracle value agree
                        // numerically within `num_rel`.
                        let base = ov.abs();
                        let diff = (rv - ov).abs();
                        let env = num_rel * base + tol.i_abs;
                        assert!(
                            diff <= env,
                            "{ctx}: ledger `{}` probe {key}: |{rv} - {ov}| = {diff:.3e} exceeds \
                             num_rel envelope {env:.3e}",
                            e.id
                        );
                        Self::mark_applied(e);
                        let floor = tol.i_abs + tol.i_rel * base;
                        measure_note(&e.id, "probe", diff, base, floor);
                        if diff > floor {
                            Self::mark_exceeded(e);
                        }
                    } else {
                        // exact-pair-NUMERIC (§1.3 discrete): a genuine value jump
                        // (e.g. normamps 730-vs-230 first-wire-vs-min-over-phase),
                        // NOT a display rounding — an envelope would mask real
                        // drift, so pin the exact `rust` value. `oracle` is pinned
                        // (asserted above vs the capture); stale once rust==oracle.
                        //
                        // The `rust` pin is MANDATORY (fix-round audit F1/F2): with
                        // only an oracle pin, staleness fires on `rv != ov`, so a
                        // port regression that moved the value to any THIRD value
                        // (still != oracle) would neither trip the assert nor mark
                        // the entry stale — a silent drift. Requiring `rust` closes
                        // the last soft spot in the exact-pair-numeric machinery.
                        assert!(
                            sc.rust.is_some(),
                            "{ctx}: ledger `{}` probe {key}: exact-pair-numeric scope must pin an \
                             exact `rust` value — with only `oracle`, a port drift to a third \
                             value != oracle passes silently (§1.3 discrete drift-safety)",
                            e.id
                        );
                        if let Some(r) = &sc.rust {
                            let rp = parse_leading_f64(&value_as_str(r));
                            assert!(
                                rp == Some(rv),
                                "{ctx}: ledger `{}` probe {key}: rust value {rv} != pinned rust {rp:?}",
                                e.id
                            );
                        }
                        Self::mark_applied(e);
                        if rv != ov {
                            Self::mark_exceeded(e);
                        }
                    }
                } else {
                    // non-numeric probe (enum/word): discrete state is ledgerable
                    // only as an EXACT PAIR (§1.3). A bare scope here would compare
                    // nothing (self-certifying hit) — require an explicit `oracle`
                    // pin (asserted above against the capture) AND a `rust` pin
                    // (Phase F re-review RR-1: with only `oracle`, staleness fires
                    // on rust != oracle, so a port drift to any THIRD value would
                    // pass silently — the same hole the fix-round F1/F2 closed for
                    // the numeric arm).
                    assert!(
                        sc.oracle.is_some(),
                        "{ctx}: ledger `{}` probe {key}: non-numeric value {:?} needs an exact \
                         `oracle` pin — discrete state is exact-pair only, never a bare scope (§1.3)",
                        e.id,
                        exp.value
                    );
                    assert!(
                        sc.rust.is_some(),
                        "{ctx}: ledger `{}` probe {key}: non-numeric exact-pair scope must pin an \
                         exact `rust` value — with only `oracle`, a port drift to a third value \
                         != oracle passes silently (§1.3 discrete drift-safety)",
                        e.id
                    );
                    if let Some(r) = &sc.rust {
                        let rs = value_as_str(r);
                        assert!(
                            rust_val.trim().eq_ignore_ascii_case(rs.trim()),
                            "{ctx}: ledger `{}` probe {key}: rust value {rust_val:?} != pinned rust {rs:?}",
                            e.id
                        );
                    }
                    // Stale-if-converged: the discrete divergence is gone once the
                    // Rust value equals the oracle capture (§5 R3 fail-on-stale).
                    Self::mark_applied(e);
                    if !rust_val.trim().eq_ignore_ascii_case(exp.value.trim()) {
                        Self::mark_exceeded(e);
                    }
                }
                return true;
            }
        }
        false
    }

    // --- properties (all_properties, capi channel) --------------------------

    /// Element property-value pairs handled by a `property` scope. Same contract
    /// as [`Self::probe_handled`] (§1.3: probe/property are exact expected pairs,
    /// or `num_rel` for numeric-skeleton values — pre-E/F audit UGA-T2 parity):
    /// an exact `oracle` pin re-pins the upstream getter; a numeric value with
    /// `num_rel` is envelope-checked against the live Rust `?`-value; an
    /// exact-pair value (numeric or not) REQUIRES both the `oracle` and `rust`
    /// pins (drift-safety — fix-round F1/F2 + Phase F re-review RR-1). Returns
    /// the set of `(element_lower, prop_lower)` keys to skip in the standard
    /// `compare_all_properties`.
    pub(crate) fn property_handled_keys(
        &self,
        dss: &mut dss_core::exec::Dss,
        props: &[PropsCap],
        tol: &Tolerances,
        ctx: &str,
    ) -> BTreeSet<(String, String)> {
        let mut handled = BTreeSet::new();
        let scopes: Vec<(&Entry, &Scope)> = self
            .entries()
            .filter(|e| e.kind == Kind::Divergence)
            .flat_map(|e| {
                e.scopes
                    .iter()
                    .filter(|s| s.field == "property")
                    .map(move |s| (e, s))
            })
            .collect();
        if scopes.is_empty() {
            return handled;
        }
        for pc in props {
            let el = pc.element.to_lowercase();
            for (name, val) in &pc.props {
                let key = format!("{el}.{}", name.to_lowercase());
                for (e, sc) in &scopes {
                    let m = sc
                        .name_re
                        .as_ref()
                        .map(|r| r.is_match(&key))
                        .unwrap_or(false);
                    if !m {
                        continue;
                    }
                    if let Some(o) = &sc.oracle {
                        let os = value_as_str(o);
                        assert!(
                            val.eq_ignore_ascii_case(&os),
                            "{ctx}: ledger `{}` property {key}: oracle value {val:?} != pinned {os:?} — \
                             the upstream getter changed; re-measure",
                            e.id
                        );
                    }
                    dss.command(&format!("? {}.{}", pc.element, name));
                    let rust_val = dss.result().to_string();
                    let (rn, on) = (parse_leading_f64(&rust_val), parse_leading_f64(val));
                    if let (Some(rv), Some(ov)) = (rn, on) {
                        if let Some(num_rel) = sc.num_rel {
                            // Numeric-skeleton path (mirrors `probe_handled`): the
                            // Rust value agrees with the oracle within `num_rel`.
                            let base = ov.abs();
                            let diff = (rv - ov).abs();
                            let env = num_rel * base + tol.i_abs;
                            assert!(
                                diff <= env,
                                "{ctx}: ledger `{}` property {key}: |{rv} - {ov}| = {diff:.3e} exceeds \
                                 num_rel envelope {env:.3e}",
                                e.id
                            );
                            Self::mark_applied(e);
                            let floor = tol.i_abs + tol.i_rel * base;
                            measure_note(&e.id, "property", diff, base, floor);
                            if diff > floor {
                                Self::mark_exceeded(e);
                            }
                        } else {
                            // exact-pair-NUMERIC (§1.3 discrete, mirrors
                            // `probe_handled`): a genuine value jump, pinned exactly.
                            // `rust` is MANDATORY (fix-round audit F1/F2): with only
                            // an oracle pin, a port drift to a third value != oracle
                            // would pass silently (staleness fires only on rv==ov).
                            assert!(
                                sc.rust.is_some(),
                                "{ctx}: ledger `{}` property {key}: exact-pair-numeric scope must pin \
                                 an exact `rust` value — with only `oracle`, a port drift to a third \
                                 value != oracle passes silently (§1.3 discrete drift-safety)",
                                e.id
                            );
                            if let Some(r) = &sc.rust {
                                let rp = parse_leading_f64(&value_as_str(r));
                                assert!(
                                    rp == Some(rv),
                                    "{ctx}: ledger `{}` property {key}: rust value {rv} != pinned rust {rp:?}",
                                    e.id
                                );
                            }
                            Self::mark_applied(e);
                            if rv != ov {
                                Self::mark_exceeded(e);
                            }
                        }
                    } else {
                        // Non-numeric property: exact-pair only (§1.3). A bare scope
                        // would let ANY Rust value pass — require the `oracle` pin
                        // (asserted above) AND the `rust` pin (Phase F re-review
                        // RR-1, mirrors the probe arm: an oracle-only pin lets a
                        // third-value port drift pass as legitimately non-stale).
                        assert!(
                            sc.oracle.is_some(),
                            "{ctx}: ledger `{}` property {key}: non-numeric value {val:?} needs an \
                             exact `oracle` pin — discrete state is exact-pair only, never a bare \
                             scope (§1.3)",
                            e.id
                        );
                        assert!(
                            sc.rust.is_some(),
                            "{ctx}: ledger `{}` property {key}: non-numeric exact-pair scope must \
                             pin an exact `rust` value — with only `oracle`, a port drift to a \
                             third value != oracle passes silently (§1.3 discrete drift-safety)",
                            e.id
                        );
                        if let Some(r) = &sc.rust {
                            let rs = value_as_str(r);
                            assert!(
                                rust_val.trim().eq_ignore_ascii_case(rs.trim()),
                                "{ctx}: ledger `{}` property {key}: rust value {rust_val:?} != pinned rust {rs:?}",
                                e.id
                            );
                        }
                        // Stale-if-converged: this exact-pair divergence is gone once
                        // the Rust `?`-surface equals the oracle value (§5 R3).
                        Self::mark_applied(e);
                        if !rust_val.trim().eq_ignore_ascii_case(val.trim()) {
                            Self::mark_exceeded(e);
                        }
                    }
                    handled.insert((el.clone(), name.to_lowercase()));
                }
            }
        }
        handled
    }

    // --- monitors -----------------------------------------------------------

    /// The monitor capture rewritten so each pinned `channel_idx` equals the Rust
    /// samples, or `None` if no scope matched. The caller runs the untouched
    /// `compare_monitor` on the rewrite: the pinned channel(s) compare-equal
    /// (clause (a): re-asserted inside their envelope here) while EVERY OTHER
    /// channel is tier-checked by the real comparator (clause (b) — the header,
    /// sample count and all unscoped channels go through the standard path). A
    /// scope with no `channel_idx` matches nothing to neutralize, so the monitor
    /// falls entirely through to the standard comparator.
    pub(crate) fn monitor_rewrite(
        &self,
        dss: &dss_core::exec::Dss,
        exp: &MonitorCap,
        tol: &Tolerances,
        ctx: &str,
    ) -> Option<MonitorCap> {
        let scopes: Vec<(&Entry, &Scope)> = self
            .entries()
            .filter(|e| e.kind == Kind::Divergence)
            .flat_map(|e| {
                e.scopes
                    .iter()
                    .filter(|s| {
                        s.field == "monitor"
                            && s.name_re
                                .as_ref()
                                .map(|r| r.is_match(&exp.name))
                                .unwrap_or(true)
                    })
                    .map(move |s| (e, s))
            })
            .collect();
        if scopes.is_empty() {
            return None;
        }
        let view = dss
            .monitor_view(&exp.name)
            .unwrap_or_else(|| panic!("{ctx}: ledger monitor {}: no Rust monitor", exp.name));
        let mut channels = exp.channels.clone();
        let mut any = false;
        for (e, sc) in &scopes {
            let Some(ci) = sc.channel_idx else { continue };
            let och = exp.channels.get(ci).unwrap_or_else(|| {
                panic!(
                    "{ctx}: ledger `{}` monitor channel_idx {ci} out of range",
                    e.id
                )
            });
            let rch = view.channels.get(ci).unwrap_or_else(|| {
                panic!(
                    "{ctx}: ledger `{}` monitor: Rust missing channel {ci}",
                    e.id
                )
            });
            let mut exceeded = false;
            for (a, o) in rch.iter().zip(och) {
                let base = o.abs();
                let diff = (*a as f64 - *o).abs();
                let env = sc.max_abs + sc.max_rel * base;
                assert!(
                    diff <= env,
                    "{ctx}: ledger `{}` monitor {} ch{ci}: |diff| {diff:.3e} exceeds envelope {env:.3e}",
                    e.id,
                    exp.name
                );
                let floor = tol.i_abs + tol.i_rel * base;
                measure_note(&e.id, "monitor", diff, base, floor);
                if diff > floor {
                    exceeded = true;
                }
            }
            Self::mark_applied(e);
            if exceeded {
                Self::mark_exceeded(e);
            }
            // Neutralize the pinned channel: overwrite it with the Rust samples so
            // the standard comparator treats it as equal and tier-checks the rest.
            for (k, a) in rch.iter().enumerate() {
                if let Some(slot) = channels[ci].get_mut(k) {
                    *slot = *a as f64;
                }
            }
            any = true;
        }
        if !any {
            return None;
        }
        Some(MonitorCap {
            name: exp.name.clone(),
            header: exp.header.clone(),
            sample_count: exp.sample_count,
            channels,
            skip_channels: exp.skip_channels.clone(),
        })
    }

    // --- eventlog / ctrlqueue line masks ------------------------------------

    /// Mask a single event-log/ctrlqueue line: if a `line_re` scope matches it,
    /// record the hit and return a normalized form (trailing-whitespace-trimmed)
    /// so the caller's compare treats the masked artifact as equal.
    ///
    /// Fail-on-stale (pre-E/F audit UGA-T3): the mask's entire power is
    /// `trim_end()`, so a live divergence is recorded only when the trailing-
    /// whitespace artifact is actually PRESENT on the matched line. If upstream
    /// stops emitting it, the entry stays applied-but-never-exceeded and trips
    /// STALE (§5 R3) instead of masking forever.
    pub(crate) fn mask_line(&self, field: &str, line: &str) -> String {
        for e in self.entries() {
            if e.kind != Kind::Divergence {
                continue;
            }
            for sc in &e.scopes {
                if sc.field != field {
                    continue;
                }
                if let Some(re) = &sc.line_re
                    && re.is_match(line)
                {
                    Self::mark_applied(e);
                    if line != line.trim_end() {
                        Self::mark_exceeded(e);
                    }
                    return line.trim_end().to_string();
                }
            }
        }
        line.to_string()
    }
}

/// Envelope-check one element's selected sub-channels against the Rust snapshot.
/// Layout matches `ElementSnapshot`: `currents`/`powers` are complex per
/// conductor (A, kW+j·kvar) while the oracle `ElementCap` splits re/im into
/// parallel arrays; `loss_w` is a `(re, im)` tuple vs the oracle's 2-vector.
fn envelope_element(
    e: &Entry,
    sc: &Scope,
    snap: &dss_core::exec::ElementSnapshot,
    ec: &ElementCap,
    tol: &Tolerances,
    ctx: &str,
) {
    let want = |ch: &str| sc.channels.is_empty() || sc.channels.iter().any(|c| c == ch);
    let mut exceeded = false;
    let mut check = |label: &str, a: f64, o: f64| {
        let base = o.abs();
        let diff = (a - o).abs();
        let env = sc.max_abs + sc.max_rel * base;
        assert!(
            diff <= env,
            "{ctx}: ledger `{}` element {} {label}: |diff| {diff:.3e} exceeds envelope {env:.3e}",
            e.id,
            ec.name
        );
        let floor = tol.i_abs + tol.i_rel * base;
        measure_note(&e.id, "element", diff, base, floor);
        if diff > floor {
            exceeded = true;
        }
    };
    if want("currents") {
        for (k, (re, im)) in ec.i_re.iter().zip(&ec.i_im).enumerate() {
            check(&format!("i_re[{k}]"), snap.currents[k].re, *re);
            check(&format!("i_im[{k}]"), snap.currents[k].im, *im);
        }
    }
    if want("powers") {
        for (k, (kw, kvar)) in ec.p_kw.iter().zip(&ec.p_kvar).enumerate() {
            check(&format!("p_kw[{k}]"), snap.powers[k].re, *kw);
            check(&format!("p_kvar[{k}]"), snap.powers[k].im, *kvar);
        }
    }
    if want("losses") && ec.loss_w.len() == 2 {
        check("loss_re", snap.loss_w.0, ec.loss_w[0]);
        check("loss_im", snap.loss_w.1, ec.loss_w[1]);
    }
    LedgerView::mark_applied(e);
    if exceeded {
        LedgerView::mark_exceeded(e);
    }
}

/// A value-copy of an oracle element capture (no `Clone` derive on the harness
/// struct — we build a fresh cap so the rewrite never touches the harness type).
fn clone_element_cap(ec: &ElementCap) -> ElementCap {
    ElementCap {
        name: ec.name.clone(),
        i_re: ec.i_re.clone(),
        i_im: ec.i_im.clone(),
        p_kw: ec.p_kw.clone(),
        p_kvar: ec.p_kvar.clone(),
        loss_w: ec.loss_w.clone(),
    }
}

/// Overwrite a cap's SELECTED sub-channels (per `sc.channels`; empty ⇒ all) with
/// the Rust snapshot values, so the standard `compare_element` treats exactly the
/// pinned channels as equal and tier-checks the unscoped remainder (clause (b)).
fn rewrite_element_selected(
    cap: &mut ElementCap,
    sc: &Scope,
    snap: &dss_core::exec::ElementSnapshot,
) {
    let want = |ch: &str| sc.channels.is_empty() || sc.channels.iter().any(|c| c == ch);
    if want("currents") {
        for k in 0..cap.i_re.len().min(cap.i_im.len()) {
            cap.i_re[k] = snap.currents[k].re;
            cap.i_im[k] = snap.currents[k].im;
        }
    }
    if want("powers") {
        for k in 0..cap.p_kw.len().min(cap.p_kvar.len()) {
            cap.p_kw[k] = snap.powers[k].re;
            cap.p_kvar[k] = snap.powers[k].im;
        }
    }
    if want("losses") && cap.loss_w.len() == 2 {
        cap.loss_w[0] = snap.loss_w.0;
        cap.loss_w[1] = snap.loss_w.1;
    }
}

fn value_as_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Parse the leading floating-point number from a probe/property value string
/// (the numeric-skeleton head), e.g. `"92.4321"` or `"92.4321 kWh"` → `92.4321`.
fn parse_leading_f64(s: &str) -> Option<f64> {
    let t = s.trim();
    if let Ok(v) = t.parse::<f64>() {
        return Some(v);
    }
    let head: String = t
        .chars()
        .take_while(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))
        .collect();
    head.parse::<f64>().ok()
}

// ---------------------------------------------------------------------------
// Structural test (oracle-free) — the §1.3 invariants.
// ---------------------------------------------------------------------------

/// The structural invariants of the ledger, verified against the manifests with
/// NO engine spawned (§1.3): unique ids; every `case` exists in a manifest and
/// its `channel` is in that case's `engines`; a `divergence`/`exclusion` needs a
/// non-empty `match`; every entry carries `cause` or a resolving `cause_ref`;
/// regexes already compiled at load. Called by the `#[test]` below and reused by
/// the seeding tooling.
pub(crate) fn assert_structural(
    rt: &LedgerRuntime,
    manifest_engines: &dyn Fn(&str) -> Option<Vec<EngineChannel>>,
) {
    let mut ids = BTreeSet::new();
    let raw_text = std::fs::read_to_string(ledger_path()).expect("read ledger.json");
    let raw: RawLedger = serde_json::from_str(&raw_text).expect("parse ledger.json");
    let cause_keys: BTreeSet<&String> = rt.cause_keys().into_iter().collect();

    for e in &raw.entries {
        assert!(
            ids.insert(e.id.clone()),
            "ledger: duplicate entry id {:?}",
            e.id
        );
        let channel = parse_channel(&e.channel)
            .unwrap_or_else(|| panic!("ledger {:?}: bad channel {:?}", e.id, e.channel));
        let engines = manifest_engines(&e.case).unwrap_or_else(|| {
            panic!(
                "ledger entry {:?}: case {:?} is not in any manifest (bad case key)",
                e.id, e.case
            )
        });
        assert!(
            engines.contains(&channel),
            "ledger entry {:?}: channel {:?} is not in case {:?} engines {:?} — a ledger \
             entry may only pin a channel the case actually gates (§1.3)",
            e.id,
            e.channel,
            e.case,
            engines
        );
        match e.kind.as_str() {
            "divergence" | "exclusion" => assert!(
                !e.match_scopes.is_empty(),
                "ledger entry {:?}: a {} entry needs a non-empty `match`",
                e.id,
                e.kind
            ),
            "skip" => {}
            k => panic!("ledger entry {:?}: unknown kind {k:?}", e.id),
        }
        let has_cause = e.cause.as_deref().is_some_and(|c| !c.trim().is_empty());
        match &e.cause_ref {
            Some(cr) => assert!(
                cause_keys.contains(cr),
                "ledger entry {:?}: cause_ref {cr:?} does not resolve to a `causes` key",
                e.id
            ),
            None => assert!(
                has_cause,
                "ledger entry {:?}: needs a non-empty `cause` or a resolving `cause_ref`",
                e.id
            ),
        }
        // Whole-artifact fields are exclusion-only: there is no remainder to
        // tier-check and no envelope to re-assert, so a `divergence` naming one
        // would claim a measurement the runtime never makes
        // (`GOLDEN_REBASE_PLAN.md` G2.5).
        for sc in &e.match_scopes {
            assert!(
                !(EXCLUSION_ONLY_FIELDS.contains(&sc.field.as_str()) && e.kind != "exclusion"),
                "ledger entry {:?}: field {:?} is exclusion-only — a {} entry naming it \
                 would pin an envelope nothing re-asserts",
                e.id,
                sc.field,
                e.kind
            );
            assert!(
                !(e.kind == "exclusion" && !EXCLUSION_FIELDS.contains(&sc.field.as_str())),
                "ledger entry {:?}: field {:?} has no `exclusion` handler (it is \
                 divergence-only: {:?}). The scope would load cleanly and then never \
                 apply, which is exactly what the field whitelist exists to prevent",
                e.id,
                sc.field,
                LEDGER_FIELDS
                    .iter()
                    .filter(|f| !EXCLUSION_FIELDS.contains(f))
                    .collect::<Vec<_>>()
            );
        }
        // discrete-state guard: probe/property scopes must be exact pairs, never
        // envelopes (§1.3 — discrete state is ledgerable only as exact pairs).
        // An `exclusion` drops the pair from comparison outright, so the pin
        // requirement (which is about what a *divergence* re-asserts) does not
        // apply to it.
        for sc in &e.match_scopes {
            if e.kind == "divergence" && matches!(sc.field.as_str(), "probe" | "property") {
                assert!(
                    !(sc.oracle.is_none() && (sc.max_rel.is_some() || sc.max_abs.is_some())),
                    "ledger entry {:?}: {} scope must pin an exact `oracle` value, \
                     never an envelope (discrete state is exact-only, §1.3)",
                    e.id,
                    sc.field
                );
                // A bare selector-only scope would mask the value with no assertion
                // at all (pre-E/F audit UGA-T2): §1.3 allows exactly two forms —
                // an exact expected pair, or `num_rel` for numeric-skeleton values.
                assert!(
                    sc.oracle.is_some() || sc.num_rel.is_some(),
                    "ledger entry {:?}: {} scope needs an exact `oracle` pin or a \
                     `num_rel` numeric-skeleton envelope — a bare selector-only scope \
                     asserts nothing (§1.3)",
                    e.id,
                    sc.field
                );
                // Exact-pair drift-safety (fix-round F1/F2 + Phase F re-review
                // RR-1): a scope with no `num_rel` is an exact pair, and an
                // oracle-only pin lets a port drift to a THIRD value (!= oracle)
                // pass as legitimately non-stale — the `rust` pin is mandatory
                // for both the numeric and non-numeric arms.
                assert!(
                    sc.num_rel.is_some() || sc.rust.is_some(),
                    "ledger entry {:?}: {} exact-pair scope (no `num_rel`) must pin \
                     an exact `rust` value alongside `oracle` — an oracle-only pin \
                     passes third-value port drift silently (§1.3 drift-safety)",
                    e.id,
                    sc.field
                );
            }
        }
    }

    // Full-bypass guard (pre-E/F audit UGA-2): a `skip` entry drops its channel
    // entirely, so at least one of the case's gating channels must remain
    // non-skipped — otherwise the case would pass with ZERO verification (not
    // even the Rust smoke runs on the Live path). §1.3 `skip`: "the other
    // channel still gates the case".
    let mut skip_channels: std::collections::BTreeMap<&str, Vec<EngineChannel>> =
        std::collections::BTreeMap::new();
    for e in raw.entries.iter().filter(|e| e.kind == "skip") {
        let ch = parse_channel(&e.channel).expect("channel validated above");
        let v = skip_channels.entry(&e.case).or_default();
        if !v.contains(&ch) {
            v.push(ch);
        }
    }
    for (case, skipped) in &skip_channels {
        let engines = manifest_engines(case).expect("case validated above");
        assert!(
            engines.iter().any(|ch| !skipped.contains(ch)),
            "ledger: EVERY gating channel of case {case:?} is `skip`-ledgered \
             ({skipped:?}) — the case would pass with zero verification. A skip \
             entry requires at least one non-skipped channel to keep gating (§1.3)."
        );
    }
}

// ---------------------------------------------------------------------------
// Oracle-free structural test.
// ---------------------------------------------------------------------------

/// Map every gate case key (`<source>:<path>`) to its `engines` channels, for the
/// ledger structural test's "channel is in the case's engines" check.
fn manifest_engines_map() -> std::collections::BTreeMap<String, Vec<EngineChannel>> {
    use crate::manifest::{FAMILIES, load_family, load_solvable};
    let mut m = std::collections::BTreeMap::new();
    for c in load_solvable() {
        m.insert(format!("solvable_now:{}", c.path), c.engine_channels());
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            m.insert(format!("{}:{}", fam.name, c.path), c.engine_channels());
        }
    }
    m
}

#[test]
fn ledger_is_structurally_valid() {
    let rt = LedgerRuntime::load();
    let map = manifest_engines_map();
    assert_structural(&rt, &|case| map.get(case).cloned());
    eprintln!(
        "ledger.json: {} entry(ies) structurally valid ({} cause key(s))",
        rt.entries.len(),
        rt.causes.len()
    );
}
