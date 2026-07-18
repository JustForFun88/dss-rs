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

fn compile_scope(id: &str, s: &RawScope) -> Scope {
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
            if diff > tol.i_abs + tol.i_rel * base {
                Self::mark_exceeded(e);
            }
        }
        Self::mark_applied(e);
        true
    }

    // --- elements (currents / powers / losses) ------------------------------

    /// Names (lowercased) of elements fully or partially handled by an element
    /// scope, with the per-channel envelope applied here. Returns the set of
    /// element names the caller must NOT pass to the standard `compare_element`
    /// (they are handled — either enveloped or excluded — inside).
    pub(crate) fn element_handled_names(
        &self,
        step: usize,
        snaps: &[dss_core::exec::ElementSnapshot],
        elements: &[ElementCap],
        tol: &Tolerances,
        ctx: &str,
    ) -> BTreeSet<String> {
        let mut handled = BTreeSet::new();
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
            return handled;
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
                handled.insert(lname.clone());
                if e.kind == Kind::Exclusion {
                    Self::mark_applied(e);
                    continue;
                }
                // divergence: envelope-check the selected channels against the
                // Rust snapshot; the sub-channels named in `channels` are
                // enveloped, the rest still tier-checked here.
                let snap = snaps
                    .iter()
                    .find(|s| s.name.eq_ignore_ascii_case(&ec.name))
                    .unwrap_or_else(|| {
                        panic!("{ctx}: ledger `{}`: no Rust element {}", e.id, ec.name)
                    });
                envelope_element(e, sc, snap, ec, tol, ctx);
            }
        }
        handled
    }

    // --- probes -------------------------------------------------------------

    /// Returns `true` if a probe scope handled this probe (caller skips the
    /// standard `compare_probe`). A display-precision divergence pins `num_rel`:
    /// the Rust `?`-value and the (low-precision-rendered) oracle value must agree
    /// numerically within `num_rel`, and the raw gap must exceed the tier floor
    /// (else stale). An optional exact `oracle` string re-pins the upstream getter.
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
                    let num_rel = sc.num_rel.unwrap_or(0.0);
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
                    if diff > tol.i_abs + tol.i_rel * base {
                        Self::mark_exceeded(e);
                    }
                } else {
                    // non-numeric probe (enum/word): exact-pair semantics only.
                    Self::mark_applied(e);
                    Self::mark_exceeded(e);
                }
                return true;
            }
        }
        false
    }

    // --- properties (all_properties, capi channel) --------------------------

    /// Element property-value pairs handled by a `property` scope (exact oracle
    /// pin). Returns the set of `(element_lower, prop_lower)` keys to skip in the
    /// standard `compare_all_properties`.
    pub(crate) fn property_handled_keys(
        &self,
        props: &[PropsCap],
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
                            "{ctx}: ledger `{}` property {key}: oracle value {val:?} != pinned {os:?}",
                            e.id
                        );
                    }
                    handled.insert((el.clone(), name.to_lowercase()));
                    Self::mark_applied(e);
                    Self::mark_exceeded(e);
                }
            }
        }
        handled
    }

    // --- monitors -----------------------------------------------------------

    /// Returns `true` if a monitor scope fully handled this monitor's compare
    /// (caller skips the standard `compare_monitor`). Envelopes the selected
    /// channel (`channel_idx`) and tier-checks the rest.
    pub(crate) fn monitor_handled(
        &self,
        dss: &dss_core::exec::Dss,
        exp: &MonitorCap,
        tol: &Tolerances,
        ctx: &str,
    ) -> bool {
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
            return false;
        }
        let view = dss
            .monitor_view(&exp.name)
            .unwrap_or_else(|| panic!("{ctx}: ledger monitor {}: no Rust monitor", exp.name));
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
                if diff > tol.i_abs + tol.i_rel * base {
                    exceeded = true;
                }
            }
            Self::mark_applied(e);
            if exceeded {
                Self::mark_exceeded(e);
            }
        }
        // Any monitor channel NOT covered by a scope is still tier-checked by the
        // caller (it re-runs the standard comparator only when no scope matched).
        // Here every scope pins exactly one channel of a monitor known to drift on
        // exactly that channel; if the deck ever needs multi-channel handling, add
        // a scope per channel. We handled the monitor iff at least one scope had a
        // channel_idx.
        scopes.iter().any(|(_, sc)| sc.channel_idx.is_some())
    }

    // --- eventlog / ctrlqueue line masks ------------------------------------

    /// Mask a single event-log/ctrlqueue line: if a `line_re` scope matches it,
    /// record the hit and return a normalized form (trailing-whitespace-trimmed)
    /// so the caller's compare treats the masked artifact as equal.
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
                    Self::mark_exceeded(e);
                    return line.trim_end().to_string();
                }
            }
        }
        line.to_string()
    }
}

/// Envelope-check one element's selected sub-channels against the Rust snapshot.
/// Layout matches `ElementSnapshot`: `currents`/`powers` are interleaved
/// (re/im, kw/kvar) while the oracle `ElementCap` splits them; `loss_w` is a
/// `(re, im)` tuple vs the oracle's 2-vector.
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
        if diff > tol.i_abs + tol.i_rel * base {
            exceeded = true;
        }
    };
    if want("currents") {
        for (k, (re, im)) in ec.i_re.iter().zip(&ec.i_im).enumerate() {
            check(&format!("i_re[{k}]"), snap.currents[2 * k], *re);
            check(&format!("i_im[{k}]"), snap.currents[2 * k + 1], *im);
        }
    }
    if want("powers") {
        for (k, (kw, kvar)) in ec.p_kw.iter().zip(&ec.p_kvar).enumerate() {
            check(&format!("p_kw[{k}]"), snap.powers[2 * k], *kw);
            check(&format!("p_kvar[{k}]"), snap.powers[2 * k + 1], *kvar);
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
        // discrete-state guard: probe/property scopes must be exact pairs, never
        // envelopes (§1.3 — discrete state is ledgerable only as exact pairs).
        for sc in &e.match_scopes {
            if matches!(sc.field.as_str(), "probe" | "property")
                && sc.oracle.is_none()
                && (sc.max_rel.is_some() || sc.max_abs.is_some())
            {
                panic!(
                    "ledger entry {:?}: {} scope must pin an exact `oracle` value, \
                     never an envelope (discrete state is exact-only, §1.3)",
                    e.id, sc.field
                );
            }
        }
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
