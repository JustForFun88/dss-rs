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
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use dss_core::exec::SeqArm;
use dss_core::support::complexutil::Polar;

use crate::harness::{
    ElementCap, Injection, MonitorCap, ProbeCap, PropsCap, PropsChannel, SEQ_C012, Tolerances,
    phase_loss_band, polar_angle_band, residual_band, seq_band, seq_power_band, wrapped_deg,
};
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
    /// Per-SCOPE hit accounting, policed for `variables` scopes by
    /// `assert_all_hit`. The entry-level `applied` flag cannot see a dead
    /// per-value mask: an entry that also carries `voltages`/`element` scopes
    /// stays `applied` through them, so a `variables` `name_re` that stops
    /// matching (a renamed state variable, a typo) would silently mask nothing.
    hit: AtomicBool,
    /// Per-SUB-CHANNEL floor-exceed accounting for a [`SUBCHANNEL_FIELDS`]
    /// scope: bit `i` is set once `channels[i]` was measured diverging beyond
    /// its tier floor. The entry-level `exceeded_floor` is an OR over every
    /// channel, so a widened scope whose new sub-channel masks nothing would
    /// ride along on a sibling channel's divergence for ever — the sub-channel
    /// twin of the `variables` staleness hole (G1.3a audit settlement,
    /// 2026-09-04). Policed by `assert_all_hit` for `divergence` entries, which
    /// are the ones that measure at all (see [`LedgerView::excluded`] on why an
    /// `exclusion` carries no verdict).
    channels_exceeded: AtomicU32,
}

impl Scope {
    /// Record that sub-channel `ch` of this scope was measured beyond its tier
    /// floor. A name not in `channels` (i.e. a bare "all sub-channels" scope)
    /// is silently ignored — there is nothing to attribute the exceed to.
    fn mark_channel_exceeded(&self, ch: &str) {
        if let Some(i) = self.channels.iter().position(|c| c == ch) {
            self.channels_exceeded
                .fetch_or(1u32 << (i as u32 % 32), Ordering::Relaxed);
        }
    }

    /// The `channels` entries never measured beyond their floor this run.
    fn dead_channels(&self) -> Vec<&str> {
        let bits = self.channels_exceeded.load(Ordering::Relaxed);
        self.channels
            .iter()
            .enumerate()
            .filter(|(i, _)| bits & (1u32 << (*i as u32 % 32)) == 0)
            .map(|(_, c)| c.as_str())
            .collect()
    }

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

    /// Does this entry carry a scope whose exclusion is *measured* against the
    /// tier floor, so that "it stopped masking anything" is observable?
    ///
    /// Only `voltages` qualifies today: [`LedgerView::voltage_keep_mask`] walks
    /// node by node and already computes `|Δ|` and the tier floor for every one
    /// of them, so recording the exceed costs nothing and is exactly the
    /// divergence kind's own staleness signal. The other exclusion fields name a
    /// whole artifact the runner then never fetches a verdict for (`y`,
    /// `y_fingerprint`, `yprim`, `meter`, `injection`, `monitor`, `probe`,
    /// `element`) — re-deriving a verdict for them would mean a second copy of
    /// each comparator inside the ledger, i.e. a new drift surface, so their
    /// anti-rot guard stays the expected-value pin the entry's `cause` names
    /// (mandatory, and registered both ways in
    /// `oracle_parity_cfg_gate.rs::TORN_DOWN_ROWS`).
    fn has_measurable_scope(e: &Entry) -> bool {
        e.scopes.iter().any(|s| s.field == "voltages")
    }

    /// Assert every entry was hit at least once and no divergence entry is stale
    /// (applied but nothing exceeded the tier floor), and that an `exclusion`
    /// carrying a measurable scope still masks something. The gate fails loudly
    /// with a prune instruction otherwise (§1.3 runtime rule / §5 R3
    /// fail-on-stale).
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
            // An `exclusion` names whole artifacts, most of which carry no
            // measurement at all (see [`LedgerView::excluded`]) — so it cannot be
            // held to the divergence rule wholesale. It CAN be held to it on the
            // scopes that do measure: a `voltages` scope compares node-by-node
            // against the tier floor inside `voltage_keep_mask`, and an engine
            // fix big enough to need this kind always moves node voltages (that
            // is what made these rows `WholeCase`). So an exclusion that carries
            // one and never exceeded is masking nothing, exactly like a stale
            // divergence.
            if e.kind == Kind::Exclusion && Self::has_measurable_scope(e) && !exceeded {
                problems.push(format!(
                    "  ledger entry `{}` ({:?}, {:?}) is STALE — it excludes node \
                     voltages, yet every node is now within the tier floor (masks \
                     nothing). Prune it: the divergence it paid for is gone, or \
                     re-scope it onto whatever still diverges.",
                    e.id, e.case, e.channel
                ));
                continue;
            }
            if hits == 0 {
                problems.push(format!(
                    "  ledger entry `{}` ({:?}, {:?}) recorded zero hits.",
                    e.id, e.case, e.channel
                ));
                continue;
            }
            // Per-SUB-CHANNEL liveness on a measured (`divergence`) entry: the
            // entry-level `exceeded` above is an OR over every sub-channel, so a
            // scope widened onto a channel that masks nothing keeps riding on a
            // sibling's divergence. Only `divergence` entries reach
            // `envelope_element` and therefore measure at all — an `exclusion`
            // names whole artifacts it never fetches a verdict for (see
            // [`LedgerView::excluded`]), so its sub-channels stay backed by the
            // measured provenance recorded in the entry itself.
            //
            // Deliberate, not an omission (G1.3d(ii) audit settlement,
            // 2026-09-05, which widened ten `exclusion` scopes onto
            // `phase_losses`): an `exclusion` is what a cause gets when its
            // channel cannot be measured *reliably*. Four of those ten sit on
            // GICTransformer decks whose capi 0.14.5 oracle disagrees with
            // ITSELF across processes (coordinator decision D12), so a
            // "did this mask anything on THIS run" verdict would be a coin flip
            // and a STALE report a flaky gate. What stands behind an exclusion
            // is instead the measured first failure recorded in the entry
            // (`measured.*` + `source`), and TESTING.md states the rule.
            for sc in e.scopes.iter().filter(|sc| !sc.channels.is_empty()) {
                if e.kind != Kind::Divergence {
                    continue;
                }
                let dead = sc.dead_channels();
                if !dead.is_empty() {
                    problems.push(format!(
                        "  ledger entry `{}` ({:?}, {:?}) has STALE `{}` \
                         sub-channel(s) {:?} — every selected value on them is \
                         within the tier floor, so widening the scope onto them \
                         masks nothing. Drop those names from `channels`.",
                        e.id, e.case, e.channel, sc.field, dead
                    ));
                }
            }
            // Per-value masks need their own liveness: `applied`/`exceeded` are
            // per ENTRY, so a dead `variables` scope on an entry that also
            // excludes voltages would never be reported. Each `variables` scope
            // must have matched at least one variable this run.
            for sc in e.scopes.iter().filter(|sc| sc.field == "variables") {
                if !sc.hit.load(Ordering::Relaxed) {
                    problems.push(format!(
                        "  ledger entry `{}` ({:?}, {:?}) has a STALE \
                         `variables` scope {:?} — it matched no state \
                         variable this run (a renamed variable, or the \
                         divergence is gone). Prune or re-scope it; the \
                         other scopes of this entry cannot report a dead \
                         per-value mask.",
                        e.id,
                        e.case,
                        e.channel,
                        sc.name_re.as_ref().map(|r| r.as_str()).unwrap_or("<all>")
                    ));
                }
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
/// The last five — `y`, `y_fingerprint`, `yprim`, `meter`, `variables` — are
/// **exclusion-only** ([`EXCLUSION_ONLY_FIELDS`]). The first four name a whole
/// compared artifact rather than a value with a natural envelope, so the only
/// thing the ledger can say about them is "this (case, channel) does not
/// compare it"; `GOLDEN_REBASE_PLAN.md` G2.5 added them, because an engine fix
/// that declines an upstream bug moves the assembled admittance of the affected
/// deck and nothing else in this file could express that.
///
/// `variables` (`R4133_PROPS_PLAN.md` RP3.10, the WindGen `QMode=0` fix) is the
/// odd one out: a state variable IS a scalar with the natural
/// `i_abs + i_rel*|oracle|` envelope, but no handler re-asserts one, so
/// whitelisting it for `divergence` too would let an entry promise a
/// measurement the runtime never makes. It is selected **per variable**
/// (`name_re` over the lowercased `element:variable` key) precisely so the
/// exclusion stays that small: the only other way to stop comparing 3 of a
/// dynamics deck's 22 state variables is to drop the element from the
/// manifest's `variables` list, which masks the other 19 — and the population
/// lock counts that as a rigor shrink.
const LEDGER_FIELDS: [&str; 14] = [
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
    "variables",
];

/// Fields an entry may name only with `kind: "exclusion"` — see
/// [`LEDGER_FIELDS`]. A `divergence` naming one would promise an envelope
/// nothing re-asserts.
const EXCLUSION_ONLY_FIELDS: [&str; 5] = ["y", "y_fingerprint", "yprim", "meter", "variables"];

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
///
/// `variables` (`R4133_PROPS_PLAN.md` RP3.10) is served by
/// [`LedgerView::excluded`] like the coarse four, keyed on the lowercased
/// `element:variable` pair the runner builds (`corpus_gate/runner.rs`, e.g.
/// `windgen.w1:pgen`), so `every_exclusion_field_is_honoured_by_the_runtime`
/// covers it with no new drive. `compare_variables` keeps its **count**
/// assertion unconditional whatever the ledger says, and asserts for every
/// index an exclusion drops that both engines spell that variable the same —
/// a mask selected by name must not be able to slide onto a clean channel.
const EXCLUSION_FIELDS: [&str; 10] = [
    "voltages",
    "element",
    "injection",
    "monitor",
    "probe",
    "y",
    "y_fingerprint",
    "yprim",
    "meter",
    "variables",
];

/// Fields whose [`Scope::channels`] selects **sub-channels** of a multi-part
/// comparison, each with the closed set of names it accepts.
///
/// `element` is the only one today (its cap carries currents, powers and
/// losses, plus G1.3a's three polar renderings). Both handlers spell the
/// selector as `sc.channels.is_empty() || sc.channels.contains(ch)` — so an
/// omitted `channels` reads as "all of them", and a committed entry written for
/// the original three would **silently widen** onto every sub-channel WP-G1
/// adds to the element row (`GOLDEN_REBASE_PLAN.md` §1.1 surface #1, G1.3a–c):
/// no ledger diff, no population-lock trip, an exclusion quietly covering more
/// than it was reviewed for. Every committed scope on such a field must
/// therefore name its channels, and may name only these.
///
/// G1.3a (2026-09-04) added `currents_mag_ang`, `voltages_mag_ang` and
/// `residuals` — the `CktElement.CurrentsMagAng` / `VoltagesMagAng` /
/// `Residuals` channels of the `compare_derived` surface (r4133
/// `DDLL/DCktElement.pas:1058`/`:1082`/`:827`), handled by
/// [`envelope_element`] and [`rewrite_element_selected`] like the first three.
/// None of the 14 committed `element` scopes widened onto them silently — this
/// rule is exactly what stopped that: G1.3a F6′ then re-measured the corpus and
/// widened **13** of them deliberately, each onto only the polar sub-channels
/// that were measured failing on that entry's own channel (see each entry's
/// `measured.g13a_polar_first_failure`); `r4133-indmachmidi-injection-ulp` was
/// measured NOT to fail and keeps the original three.
///
/// G1.3d(ii) (2026-09-05) added `phase_losses` — `CktElement.PhaseLosses`
/// (r4133 `Common/CktElement.pas:1078-1120`), the same `V·conj(I)` products
/// `powers` carries, bucketed by phase — handled by [`envelope_element`] and
/// [`rewrite_element_selected`] like the six before it. Same discipline: the
/// live drive measured which committed scopes actually fail on it and widened
/// only those (`measured.g13d2_phase_losses_first_failure` on each), and the
/// four `r4133-*-injection-ulp` divergences were measured NOT to fail and keep
/// their committed lists.
///
/// G1.3b (2026-09-05) added `seq_currents`, `seq_voltages` and `seq_powers` —
/// the per-element symmetrical-component surfaces `CktElement.SeqCurrents` /
/// `SeqVoltages` / `SeqPowers` (r4133 `DDLL/DCktElement.pas:700-737` /
/// `:660-698` / `:739-797`; capi `CAPI/CAPI_Alt.pas:490-527` / `:620-659` /
/// `:529-593`), handled by [`envelope_element`] and [`rewrite_element_selected`]
/// like the seven before them, with one property the seven do not need: only the
/// slots [`harness::compare_element_seq`] **bands** are covered here (see
/// [`seq_slot_is_banded`]). The discrete slots — the whole not-available arm's
/// `1.0`/sentinel payload and the exact zeros beside the positive-sequence one —
/// are neither envelope-checked nor rewritten, so no `seq_*` scope can mask a
/// discrete miss, which is the same guarantee the ten discrete extras get by
/// having no sub-channel at all.
const SUBCHANNEL_FIELDS: &[(&str, &[&str])] = &[(
    "element",
    &[
        "currents",
        "powers",
        "losses",
        "currents_mag_ang",
        "voltages_mag_ang",
        "residuals",
        "phase_losses",
        "seq_currents",
        "seq_voltages",
        "seq_powers",
    ],
)];

/// Entry ids allowed to leave `channels` off a [`SUBCHANNEL_FIELDS`] field — the
/// named, reviewed escape hatch for a scope that really does mean "every
/// sub-channel, including ones added later".
///
/// **Empty**, and meant to stay that way: after G1.0 "all" is a decision someone
/// wrote down here with a reason, never the default an omitted key falls into.
const BARE_CHANNELS_ALLOWED: &[&str] = &[];

/// The three load-time `channels` rules (`GOLDEN_REBASE_PLAN.md` G1.0 rails):
///
/// 1. a scope on a [`SUBCHANNEL_FIELDS`] field carries a non-empty `channels`
///    (unless its id is in [`BARE_CHANNELS_ALLOWED`]) — the anti-widening rule;
/// 2. every name in `channels` is one of that field's declared sub-channels — a
///    typo like `"curents"` otherwise loads cleanly and selects **nothing**,
///    leaving the entry `applied` through its other scopes while masking not one
///    value (the same dead-mask rot [`LEDGER_FIELDS`] exists to prevent);
/// 3. a scope on any other field carries no `channels` at all — the runtime
///    never reads one there, so it would sit in the file as a promise the gate
///    does not keep (the rule `assert_structural` already applies to
///    `max_rel`/`rust`/`oracle`/… on an exclusion scope).
///
/// Returns the failure text rather than panicking so the unit tests below can
/// drive all three rules on synthetic scopes.
fn check_scope_channels(id: &str, sc: &RawScope) -> Result<(), String> {
    let declared = SUBCHANNEL_FIELDS
        .iter()
        .find(|(f, _)| *f == sc.field.as_str())
        .map(|(_, names)| *names);
    let Some(names) = declared else {
        // Rule 3.
        return match sc.channels {
            Some(_) => Err(format!(
                "ledger entry {id:?}: scope on {:?} carries `channels`, which only \
                 sub-channel fields ({:?}) read — it would sit in the ledger as a \
                 promise the runtime ignores",
                sc.field,
                SUBCHANNEL_FIELDS
                    .iter()
                    .map(|(f, _)| *f)
                    .collect::<Vec<_>>()
            )),
            None => Ok(()),
        };
    };
    let listed = sc.channels.as_deref().unwrap_or(&[]);
    // Rule 1.
    if listed.is_empty() {
        if BARE_CHANNELS_ALLOWED.contains(&id) {
            return Ok(());
        }
        return Err(format!(
            "ledger entry {id:?}: scope on {:?} must name its `channels` (one or more \
             of {names:?}). An omitted/empty list reads as ALL sub-channels, so the \
             entry would silently widen onto every sub-channel added later — add the \
             names it was measured on, or its id to BARE_CHANNELS_ALLOWED with a reason",
            sc.field
        ));
    }
    // Rule 2.
    for ch in listed {
        if !names.contains(&ch.as_str()) {
            return Err(format!(
                "ledger entry {id:?}: scope on {:?} names sub-channel {ch:?}, which is \
                 not one of {names:?} — it would select nothing and mask nothing while \
                 the entry still reports itself applied",
                sc.field
            ));
        }
    }
    Ok(())
}

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
        hit: AtomicBool::new(false),
        channels_exceeded: AtomicU32::new(0),
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
                    sc.hit.store(true, Ordering::Relaxed);
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
                    // Liveness (settle of the G2.5 audit): an exclusion has no
                    // envelope to re-assert, but a `voltages` scope DOES have a
                    // measurement — the same `diff` vs tier `floor` the
                    // divergence arm below records. Taking it here is what lets
                    // `assert_all_hit` call a voltages-scoped exclusion STALE
                    // once the divergence it pays for is gone, instead of
                    // masking a deck forever. See [`Self::has_measurable_scope`].
                    measure_note(&e.id, "voltages", diff, base, floor);
                    if diff > floor {
                        Self::mark_exceeded(e);
                    }
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
    /// The `property`-scoped divergence scopes applicable to this (case,
    /// channel). Shared by the asserting gate path
    /// ([`Self::property_handled_keys`]) and the census's non-asserting
    /// [`Self::property_scope_keys`], so the two cannot disagree about WHICH
    /// entries are in play.
    fn property_scopes(&self) -> Vec<(&Entry, &Scope)> {
        self.entries()
            .filter(|e| e.kind == Kind::Divergence)
            .flat_map(|e| {
                e.scopes
                    .iter()
                    .filter(|s| s.field == "property")
                    .map(move |s| (e, s))
            })
            .collect()
    }

    /// Does `sc` name the property cell `key` (`element.prop`, both lowercased)?
    /// The single matching decision behind both property paths.
    fn scope_names_prop(sc: &Scope, key: &str) -> bool {
        sc.name_re
            .as_ref()
            .map(|r| r.is_match(key))
            .unwrap_or(false)
    }

    /// The `(element_lower, prop_lower)` keys a `property` scope NAMES — the
    /// pure matching half of [`Self::property_handled_keys`], with **no** pin
    /// assertion, no `?`-query and no hit accounting.
    ///
    /// It exists for the census knob's disposition mode
    /// (`DSS_PROPS_CENSUS=claims`, `R4133_PROPS_PLAN.md` RP0.2/RP2.1), whose job
    /// is to answer "would a ledger entry claim this divergent cell at RP4.1?"
    /// over a walk that must never fail. Sharing [`Self::property_scopes`] and
    /// [`Self::scope_names_prop`] with the gate path is what keeps the answer the
    /// gate's own: RP0.2 forbids the claims mode from re-implementing the policy
    /// chain, because a drifting copy would corrupt RP4.1's zero-UNCLAIMED read
    /// silently. What it deliberately does NOT reproduce is the gate's
    /// assertions — a census must record, never panic — so a cell it reports as
    /// `ledger-hit` is one an entry NAMES, not one whose pins were re-verified.
    pub(crate) fn property_scope_keys(&self, props: &[PropsCap]) -> BTreeSet<(String, String)> {
        let mut named = BTreeSet::new();
        let scopes = self.property_scopes();
        if scopes.is_empty() {
            return named;
        }
        for pc in props {
            let el = pc.element.to_lowercase();
            for (name, _) in &pc.props {
                let prop = name.to_lowercase();
                let key = format!("{el}.{prop}");
                if scopes
                    .iter()
                    .any(|(_, sc)| Self::scope_names_prop(sc, &key))
                {
                    named.insert((el.clone(), prop));
                }
            }
        }
        named
    }

    pub(crate) fn property_handled_keys(
        &self,
        dss: &mut dss_core::exec::Dss,
        props: &[PropsCap],
        tol: &Tolerances,
        ctx: &str,
    ) -> BTreeSet<(String, String)> {
        let mut handled = BTreeSet::new();
        let scopes = self.property_scopes();
        if scopes.is_empty() {
            return handled;
        }
        for pc in props {
            let el = pc.element.to_lowercase();
            for (name, val) in &pc.props {
                let key = format!("{el}.{}", name.to_lowercase());
                for (e, sc) in &scopes {
                    if !Self::scope_names_prop(sc, &key) {
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

/// Is slot `k` of a sequence array a **banded** sample on this arm, or one of
/// the arm's discrete slots? (`GOLDEN_REBASE_PLAN.md` G1.3b.)
///
/// The one predicate behind both halves of the `seq_*` sub-channels, so the
/// envelope check and the rewrite cover exactly the same slots by construction
/// (pinned by [`the_seq_rewrite_and_the_seq_envelope_cover_the_same_slots`]):
///
/// * [`SeqArm::ThreePhase`] — every slot is a transformed value that
///   `harness::compare_element_seq` bands with `harness::seq_band` /
///   `harness::seq_power_band`, so every slot is envelope-checked and
///   neutralizable;
/// * [`SeqArm::PosSeqSinglePhase`] — only slot `3t+1` carries a value; `3t` and
///   `3t+2` are compared as **exact** zeros on both sides (capi
///   `CAPI/CAPI_Alt.pas:555`/`:562`; r4133 `DDLL/DCktElement.pas:760`/`:768`
///   writes the wrong slot with the wrong stride — the D-b1 defect the port does
///   not reproduce);
/// * [`SeqArm::NotAvailable`] — the whole payload is discrete (`Cabs(-1+0j) =
///   1.0` magnitudes and the channel's own `−1` power sentinel, folded at the
///   capture boundary by `harness::na_seq_power`), so **no** slot is banded.
///
/// A ledger scope may therefore never mask a discrete sequence miss: an entry
/// widened onto `seq_*` over an element whose arm has no banded slot measures
/// nothing and is reported STALE by `Scope::dead_channels`, which is the honest
/// signal rather than a silent pass.
fn seq_slot_is_banded(arm: SeqArm, k: usize) -> bool {
    match arm {
        SeqArm::ThreePhase => true,
        SeqArm::PosSeqSinglePhase => k % 3 == 1,
        SeqArm::NotAvailable => false,
    }
}

/// Envelope-check one element's selected sub-channels against the Rust snapshot.
/// Layout matches `ElementSnapshot`: `currents`/`powers` are complex per
/// conductor (A, kW+j·kvar) while the oracle `ElementCap` splits re/im into
/// parallel arrays; `loss_w` is a `(re, im)` tuple vs the oracle's 2-vector.
/// G1.3a's three polar channels are `Vec<Polar>` on the Rust side against the
/// oracle's de-interleaved `*_mag`/`*_ang` pair. G1.3b's three sequence channels
/// are `3·NTerms`-long arrays whose banded slots are selected by
/// [`seq_slot_is_banded`].
fn envelope_element(
    e: &Entry,
    sc: &Scope,
    snap: &dss_core::exec::ElementSnapshot,
    ec: &ElementCap,
    tol: &Tolerances,
    ctx: &str,
) {
    let want = |ch: &str| sc.channels.is_empty() || sc.channels.iter().any(|c| c == ch);
    // `Cell` rather than a `mut` capture so the rectangular and polar helpers
    // below can coexist as `Fn` closures over one shared flag. `block` is the
    // same flag scoped to the sub-channel currently being measured, so a
    // widened `channels` list can be held to per-sub-channel liveness
    // (`Scope::dead_channels`) instead of one OR for the whole entry.
    let exceeded = std::cell::Cell::new(false);
    let block = std::cell::Cell::new(false);
    let record = |label: &str, diff: f64, base: f64, floor: f64| {
        let env = sc.max_abs + sc.max_rel * base;
        assert!(
            diff <= env,
            "{ctx}: ledger `{}` element {} {label}: |diff| {diff:.3e} exceeds envelope {env:.3e}",
            e.id,
            ec.name
        );
        measure_note(&e.id, "element", diff, base, floor);
        if diff > floor {
            exceeded.set(true);
            block.set(true);
        }
    };
    let check = |label: &str, a: f64, o: f64| {
        let base = o.abs();
        record(label, (a - o).abs(), base, tol.i_abs + tol.i_rel * base);
    };
    // One polar sample: the magnitude against its own tier band, the angle
    // wrap-aware against the angular image of that band — the same two floors
    // `harness::polar_close` gates the unpinned samples with, so a ledger
    // envelope on a polar channel is measured on the same scale it excludes.
    //
    // A magnitude at or under its band leaves the angle **undefined**
    // (`polar_angle_band` ⇒ `None`), and there the angle is skipped entirely —
    // exactly as `harness::polar_close` skips it. Envelope-checking it instead
    // would hold the ledger to a rule the gate itself does not apply: on such a
    // sample the two engines' angles are arbitrary (G1.3a measured masked
    // `CurrentsMagAng` samples 173.7 ° apart at |I| ≈ 7e-12 A), so the only
    // envelope that could admit them is ±180 °, i.e. a number that bounds
    // nothing. Measured necessity (GOLDEN_REBASE G1.3a F6′, 2026-09-04): with
    // the angle recorded, `r4133-combomidi-injection-ulp` reported
    // `Transformer.t8 cma[9].ang: |diff| 1.394e2 exceeds envelope 2.063e-5` and
    // `r4133-combomesh-injection-ulp` `Transformer.tg cma[7].ang: |diff|
    // 1.131e2` — both on conductors whose magnitude is numerical zero, neither
    // of them the divergence the entry pins. The magnitude is never skipped, so
    // the sample stays two-sided and the entry can still go stale.
    let polar = |label: &str, a: &Polar, om: f64, oa: f64, mag_band: f64| {
        record(
            &format!("{label}.mag"),
            (a.mag - om).abs(),
            om.abs(),
            mag_band,
        );
        if let Some(ang_band) = polar_angle_band(mag_band, om) {
            record(
                &format!("{label}.ang"),
                wrapped_deg(a.ang - oa),
                oa.abs(),
                ang_band,
            );
        }
    };
    block.set(false);
    if want("currents") {
        for (k, (re, im)) in ec.i_re.iter().zip(&ec.i_im).enumerate() {
            check(&format!("i_re[{k}]"), snap.currents[k].re, *re);
            check(&format!("i_im[{k}]"), snap.currents[k].im, *im);
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("currents");
    }
    block.set(false);
    if want("powers") {
        for (k, (kw, kvar)) in ec.p_kw.iter().zip(&ec.p_kvar).enumerate() {
            check(&format!("p_kw[{k}]"), snap.powers[k].re, *kw);
            check(&format!("p_kvar[{k}]"), snap.powers[k].im, *kvar);
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("powers");
    }
    block.set(false);
    if want("losses") && ec.loss_w.len() == 2 {
        check("loss_re", snap.loss_w.0, ec.loss_w[0]);
        check("loss_im", snap.loss_w.1, ec.loss_w[1]);
    }
    // G1.3a. `CurrentsMagAng` inherits the current tier, `VoltagesMagAng` the
    // voltage tier, and `Residuals` the conductor-sum band
    // (`harness::residual_band`: a terminal residual is Σ_c I_c, so its floor is
    // the sum of the conductors' bands). Empty on a disabled element — the
    // capture skips those channels there — so the loops are inert rather than
    // special-cased.
    if block.get() {
        sc.mark_channel_exceeded("losses");
    }
    block.set(false);
    if want("currents_mag_ang") {
        // Zipped against the port vector as well: a shape mismatch on a
        // ledger-scoped element must surface as `compare_element_derived`'s
        // length message (it runs after this), never as an index panic here.
        for (k, ((om, oa), p)) in ec
            .cma_mag
            .iter()
            .zip(&ec.cma_ang)
            .zip(&snap.currents_mag_ang)
            .enumerate()
        {
            let band = tol.i_abs + tol.i_rel * om.abs();
            polar(&format!("cma[{k}]"), p, *om, *oa, band);
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("currents_mag_ang");
    }
    block.set(false);
    if want("voltages_mag_ang") {
        for (k, ((om, oa), p)) in ec
            .vma_mag
            .iter()
            .zip(&ec.vma_ang)
            .zip(&snap.voltages_mag_ang)
            .enumerate()
        {
            let band = tol.v_abs + tol.v_rel * om.abs();
            polar(&format!("vma[{k}]"), p, *om, *oa, band);
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("voltages_mag_ang");
    }
    block.set(false);
    if want("residuals") {
        let nterms = ec.res_mag.len();
        // `Residuals` is one entry per terminal (r4133
        // `DDLL/DCktElement.pas:835`) summed over a `Yorder`-long current
        // buffer (`:836-837`), so nconds is their quotient. A zero-terminal cap
        // leaves the loop below empty, so the fallback divides nothing.
        let nconds = ec.i_re.len().checked_div(nterms).unwrap_or(0);
        // The same shape rule `harness::residual_close` asserts before deriving
        // `nconds`: a cap whose conductor slots do not divide into its terminals
        // would silently get a too-small band here instead of failing loudly.
        assert!(
            nterms == 0 || ec.i_re.len().is_multiple_of(nterms),
            "{ctx}: ledger `{}` element {}: {} conductor slots do not divide into \
             {nterms} terminals",
            e.id,
            ec.name,
            ec.i_re.len()
        );
        for (t, ((om, oa), p)) in ec
            .res_mag
            .iter()
            .zip(&ec.res_ang)
            .zip(&snap.residuals)
            .enumerate()
        {
            let band = residual_band(ec, t, nconds, tol.i_rel, tol.i_abs);
            polar(&format!("res[{t}]"), p, *om, *oa, band);
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("residuals");
    }
    block.set(false);
    // G1.3d(ii). `PhaseLosses[i] = Σ_j NodeV[NodeRef[k]]·conj(Iterminal[k])` at
    // `k = j·nconds + i` (r4133 `Common/CktElement.pas:1093-1112`) — the very
    // products this cap already reports as `powers`, bucketed by phase — so an
    // entry whose cause moves `powers`/`losses` moves this too. Banded by
    // `harness::phase_loss_band`, the per-conductor power band summed over that
    // phase's conductors: the same floor `compare_element_phase_losses` gates
    // the unpinned samples with, so a ledger envelope here is measured on the
    // scale it excludes. The capture is kW/kvar and the snapshot W/var, so the
    // ×0.001 is applied here exactly as the comparator applies it at its one
    // site. Empty whenever the case's `compare_element_extras` flag is off (the
    // capture then carries no `pl_kw` at all) and on a 0-phase element, so the
    // block is inert rather than special-cased — and a scope widened onto a
    // channel that measures nothing trips `Scope::dead_channels` instead of
    // masking.
    if want("phase_losses") && !ec.pl_kw.is_empty() {
        let counts = ec.n_terms.zip(ec.n_conds);
        let (nterms, nconds) = counts.unwrap_or_else(|| {
            panic!(
                "{ctx}: ledger `{}` element {}: a `phase_losses` scope on a \
                 capture that carries PhaseLosses but no NTerms/NConds — \
                 the band is built from that conductor layout",
                e.id, ec.name
            )
        });
        let (nterms, nconds) = (
            usize::try_from(nterms).expect("oracle NumTerminals is negative"),
            usize::try_from(nconds).expect("oracle NumConductors is negative"),
        );
        // The same layout tie `harness::compare_element_phase_losses` asserts
        // before indexing `k = j·nconds + i`; it runs on this element too, so a
        // shape miss fails there with its own message rather than panicking on
        // an index here.
        let layout_ok = nterms * nconds == ec.p_kw.len()
            && ec.p_kw.len() == ec.p_kvar.len()
            && ec.p_kw.len() == ec.i_re.len()
            && ec.i_re.len() == ec.i_im.len();
        assert!(
            layout_ok,
            "{ctx}: ledger `{}` element {}: NTerms·NConds ({nterms}·{nconds}) does \
             not match the captured Powers/Currents layout \
             ({} / {} / {} / {})",
            e.id,
            ec.name,
            ec.p_kw.len(),
            ec.p_kvar.len(),
            ec.i_re.len(),
            ec.i_im.len()
        );
        for (i, ((kw, kvar), p)) in ec
            .pl_kw
            .iter()
            .zip(&ec.pl_kvar)
            .zip(&snap.phase_losses)
            .enumerate()
        {
            let band = phase_loss_band(ec, i, nterms, nconds, tol.i_rel, tol.i_abs);
            let a = *p * 0.001;
            let diff = ((a.re - kw).powi(2) + (a.im - kvar).powi(2)).sqrt();
            let base = (kw.powi(2) + kvar.powi(2)).sqrt();
            record(&format!("pl[{i}]"), diff, base, band);
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("phase_losses");
    }
    // G1.3b. The three sequence channels — `SeqCurrents` / `SeqVoltages`
    // (`Cabs` of the 012 components, r4133 `DDLL/DCktElement.pas:700-737` /
    // `:660-698`) and `SeqPowers` (`0.003·V012·conj(I012)`, `:739-797`) — banded
    // by `harness::seq_band` and `harness::seq_power_band`, the images of the
    // already-calibrated `i_*`/`v_*` tier bands over the terminal's own phase
    // magnitudes, plus the r4133-only `SEQ_C012` truncated-matrix term
    // (`Shared/mathutil.pas:302-303`/`:562-564`). Those are the same floors
    // `harness::compare_element_seq` gates the unpinned samples with, over the
    // same oracle-side `CurrentsMagAng`/`VoltagesMagAng` magnitudes, so a ledger
    // envelope here is measured on exactly the scale it excludes.
    //
    // Only the arm's BANDED slots take part ([`seq_slot_is_banded`]): the
    // not-available arm and the exact zeros beside the positive-sequence one are
    // discrete on both sides, so an envelope over them would be a number
    // bounding a comparison the gate makes exactly — and, symmetrically,
    // `rewrite_element_selected` leaves them alone, which is what keeps a
    // discrete sequence miss unmaskable by any scope.
    //
    // Empty whenever the case's `compare_derived` flag is off (the capture then
    // carries no `seq_i` at all) and on a 0-terminal element, so the blocks are
    // inert rather than special-cased — and a scope widened onto a channel that
    // measures nothing trips `Scope::dead_channels` instead of masking.
    let arm = snap.seq_arm;
    let seq_wanted = want("seq_currents") || want("seq_voltages") || want("seq_powers");
    // `(bv, bi)` per terminal, computed once for all three blocks below.
    let seq_bands: Vec<(f64, f64)> = if seq_wanted && !ec.seq_i.is_empty() {
        let (nterms, nconds) = (snap.n_terms, snap.n_conds);
        let yorder = nterms * nconds;
        // The same shape tie `harness::compare_element_seq` asserts before
        // building its bands; it runs on this element too, so a genuine shape
        // miss fails there with its own message rather than slicing out of
        // range here.
        assert!(
            nterms > 0
                && nconds > 0
                && ec.cma_mag.len() == yorder
                && ec.vma_mag.len() == yorder
                && ec.seq_i.len() == 3 * nterms
                && ec.seq_v.len() == 3 * nterms
                && ec.seq_p_kw.len() == 3 * nterms
                && ec.seq_p_kvar.len() == 3 * nterms
                && snap.seq_currents.len() == 3 * nterms
                && snap.seq_voltages.len() == 3 * nterms
                && snap.seq_powers.len() == 3 * nterms,
            "{ctx}: ledger `{}` element {}: a `seq_*` scope on a capture whose \
             layout does not match {nterms} terminal(s) x {nconds} conductor(s) \
             (CurrentsMagAng {} / VoltagesMagAng {}; seq {} / {} / {} / {}; rust \
             seq {} / {} / {}) — the sequence band is built from that layout",
            e.id,
            ec.name,
            ec.cma_mag.len(),
            ec.vma_mag.len(),
            ec.seq_i.len(),
            ec.seq_v.len(),
            ec.seq_p_kw.len(),
            ec.seq_p_kvar.len(),
            snap.seq_currents.len(),
            snap.seq_voltages.len(),
            snap.seq_powers.len()
        );
        // The truncated-matrix term applies on the r4133 channel only, and only
        // where a matrix actually runs (the three-phase arm) — the same
        // `(channel, arm)` pair `harness::compare_element_seq` keys it on. The
        // entry's own channel is the one the gate compares this cap against.
        let c012 = match (e.channel.props_channel(), arm) {
            (PropsChannel::R4133, SeqArm::ThreePhase) => SEQ_C012,
            _ => 0.0,
        };
        // The conductors each arm actually transforms: the terminal's first
        // three (r4133 `:47-49`, capi `:252-254`) or, on the positive-sequence
        // arm, its first one (r4133 `:764-766`, capi `:559-561`).
        let taken = if arm == SeqArm::ThreePhase { 3 } else { 1 };
        assert!(
            taken <= nconds,
            "{ctx}: ledger `{}` element {}: the {arm:?} arm reads {taken} \
             conductor(s) of a terminal that has {nconds}",
            e.id,
            ec.name
        );
        ec.cma_mag
            .chunks(nconds)
            .zip(ec.vma_mag.chunks(nconds))
            .map(|(icnk, vcnk)| {
                (
                    seq_band(&vcnk[..taken], tol.v_rel, tol.v_abs, c012),
                    seq_band(&icnk[..taken], tol.i_rel, tol.i_abs, c012),
                )
            })
            .collect()
    } else {
        Vec::new()
    };
    block.set(false);
    if want("seq_currents") {
        for (t, (_, bi)) in seq_bands.iter().enumerate() {
            for k in 0..3 {
                let slot = 3 * t + k;
                if !seq_slot_is_banded(arm, slot) {
                    continue;
                }
                record(
                    &format!("seq_i[{slot}]"),
                    (snap.seq_currents[slot] - ec.seq_i[slot]).abs(),
                    ec.seq_i[slot].abs(),
                    *bi,
                );
            }
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("seq_currents");
    }
    block.set(false);
    if want("seq_voltages") {
        for (t, (bv, _)) in seq_bands.iter().enumerate() {
            for k in 0..3 {
                let slot = 3 * t + k;
                if !seq_slot_is_banded(arm, slot) {
                    continue;
                }
                record(
                    &format!("seq_v[{slot}]"),
                    (snap.seq_voltages[slot] - ec.seq_v[slot]).abs(),
                    ec.seq_v[slot].abs(),
                    *bv,
                );
            }
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("seq_voltages");
    }
    block.set(false);
    if want("seq_powers") {
        for (t, (bv, bi)) in seq_bands.iter().enumerate() {
            for k in 0..3 {
                let slot = 3 * t + k;
                if !seq_slot_is_banded(arm, slot) {
                    continue;
                }
                // The wire and the snapshot are BOTH kW/kvar here (the `0.003`
                // is applied inside the arm on both engines, not at the API
                // boundary the way `PhaseLosses`' `0.001` is), so no scaling.
                let band = seq_power_band(*bv, *bi, ec.seq_v[slot], ec.seq_i[slot]);
                let (ar, ai) = (snap.seq_powers[slot].re, snap.seq_powers[slot].im);
                let (er, ei) = (ec.seq_p_kw[slot], ec.seq_p_kvar[slot]);
                let diff = ((ar - er).powi(2) + (ai - ei).powi(2)).sqrt();
                let base = (er.powi(2) + ei.powi(2)).sqrt();
                record(&format!("seq_p[{slot}]"), diff, base, band);
            }
        }
    }
    if block.get() {
        sc.mark_channel_exceeded("seq_powers");
    }
    LedgerView::mark_applied(e);
    if exceeded.get() {
        LedgerView::mark_exceeded(e);
    }
}

/// A value-copy of an oracle element capture, so a rewrite never mutates the
/// capture the other channel still compares against.
///
/// Field-complete by construction (`#[derive(Clone)]` on the harness struct):
/// the enumerated copy this used to be silently dropped every field a later
/// sub-step added — `GOLDEN_REBASE_PLAN.md` G1.3a adds seven.
fn clone_element_cap(ec: &ElementCap) -> ElementCap {
    ec.clone()
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
    // G1.3a: the polar channels are two parallel oracle arrays per Rust
    // `Polar`, so a pin has to write BOTH halves back — writing only the
    // magnitude would leave `compare_element_derived` comparing the port's
    // angle against the oracle's, i.e. an entry that pins half a channel and
    // silently keeps gating the other half.
    if want("currents_mag_ang") {
        for k in 0..cap.cma_mag.len().min(cap.cma_ang.len()) {
            cap.cma_mag[k] = snap.currents_mag_ang[k].mag;
            cap.cma_ang[k] = snap.currents_mag_ang[k].ang;
        }
    }
    if want("voltages_mag_ang") {
        for k in 0..cap.vma_mag.len().min(cap.vma_ang.len()) {
            cap.vma_mag[k] = snap.voltages_mag_ang[k].mag;
            cap.vma_ang[k] = snap.voltages_mag_ang[k].ang;
        }
    }
    if want("residuals") {
        for t in 0..cap.res_mag.len().min(cap.res_ang.len()) {
            cap.res_mag[t] = snap.residuals[t].mag;
            cap.res_ang[t] = snap.residuals[t].ang;
        }
    }
    // G1.3d(ii): the snapshot is W/var and the capture kW/kvar (each oracle
    // scales at its API boundary — r4133 `DDLL/DCktElement.pas:651`, capi
    // `CAPI/CAPI_Alt.pas:464-467`), so the pin writes the SCALED value, the same
    // ×0.001 `harness::compare_element_phase_losses` applies. The snapshot
    // length joins the `min` so that a shape mismatch survives to that
    // comparator's own length assert instead of panicking on an index here.
    if want("phase_losses") {
        for i in 0..cap
            .pl_kw
            .len()
            .min(cap.pl_kvar.len())
            .min(snap.phase_losses.len())
        {
            cap.pl_kw[i] = snap.phase_losses[i].re * 0.001;
            cap.pl_kvar[i] = snap.phase_losses[i].im * 0.001;
        }
    }
    // G1.3b: the three sequence channels. Both sides are already in the same
    // units — the `0.003` is applied INSIDE the arm on both engines (r4133
    // `DDLL/DCktElement.pas:767`/`:788`, capi `CAPI/CAPI_Alt.pas:561`/`:588-590`)
    // rather than at the API boundary the way `PhaseLosses`' `0.001` is — so the
    // pin writes the value straight through.
    //
    // Only the arm's BANDED slots are neutralized ([`seq_slot_is_banded`], the
    // same predicate `envelope_element` bands with): the not-available arm's
    // `1.0`/sentinel payload and the exact zeros beside the positive-sequence
    // slot stay the ORACLE's, so `harness::compare_element_seq` keeps comparing
    // them exactly — a `seq_*` scope neutralizes a floor divergence and can
    // never excuse a discrete miss (pinned by
    // `a_discrete_seq_slot_is_never_neutralized_by_a_scope`). The snapshot
    // lengths join the bound so a shape mismatch survives to that comparator's
    // own length assert instead of panicking on an index here.
    let seq_slots = |n: usize| (0..n).filter(|k| seq_slot_is_banded(snap.seq_arm, *k));
    if want("seq_currents") {
        for k in seq_slots(cap.seq_i.len().min(snap.seq_currents.len())) {
            cap.seq_i[k] = snap.seq_currents[k];
        }
    }
    if want("seq_voltages") {
        for k in seq_slots(cap.seq_v.len().min(snap.seq_voltages.len())) {
            cap.seq_v[k] = snap.seq_voltages[k];
        }
    }
    if want("seq_powers") {
        for k in seq_slots(
            cap.seq_p_kw
                .len()
                .min(cap.seq_p_kvar.len())
                .min(snap.seq_powers.len()),
        ) {
            cap.seq_p_kw[k] = snap.seq_powers[k].re;
            cap.seq_p_kvar[k] = snap.seq_powers[k].im;
        }
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
            // The `channels` sub-channel rules (G1.0 rails) — see
            // [`check_scope_channels`] for the three and why each one exists.
            if let Err(msg) = check_scope_channels(&e.id, sc) {
                panic!("{msg}");
            }
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
            // …and an exclusion scope may carry only its SELECTORS. `excluded`
            // (and the `Kind::Exclusion` arms of the voltages/element handlers)
            // read `field`/`steps`/`node_re`/`name_re`/`channel_idx`/`channels`
            // and nothing else, so an envelope or an exact pair written on one
            // would load cleanly, be silently ignored, and read in review as a
            // promise the gate never keeps — the same rot the field whitelist
            // above exists to prevent (settle of the G2.5 audit).
            if e.kind == "exclusion" {
                let carried: Vec<&str> = [
                    ("max_rel", sc.max_rel.is_some()),
                    ("max_abs", sc.max_abs.is_some()),
                    ("num_rel", sc.num_rel.is_some()),
                    ("rust", sc.rust.is_some()),
                    ("oracle", sc.oracle.is_some()),
                    ("policy", sc.policy.is_some()),
                    ("line_re", sc.line_re.is_some()),
                ]
                .into_iter()
                .filter_map(|(k, present)| present.then_some(k))
                .collect();
                assert!(
                    carried.is_empty(),
                    "ledger entry {:?}: `exclusion` scope on {:?} carries \
                     envelope/pin field(s) {carried:?}, which the exclusion path \
                     ignores entirely — drop them, or make the entry a \
                     `divergence` that actually re-asserts them",
                    e.id,
                    sc.field
                );
            }
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

/// The fields of [`EXCLUSION_FIELDS`] whose exclusion is served by a dedicated
/// **partitioning** handler rather than by [`LedgerView::excluded`]: they split
/// one comparison into a scoped part and an unscoped remainder, so the runner
/// never asks `excluded()` about them. Both are exercised live by the
/// `GOLDEN_REBASE_PLAN.md` G2.5 entries in `tests/corpus/ledger.json`.
const EXCLUSION_FIELDS_WITH_PARTITIONING_HANDLER: [&str; 2] = ["voltages", "element"];

/// Every field [`EXCLUSION_FIELDS`] lets an `exclusion` name must actually be
/// honoured at runtime — the same guarantee [`LEDGER_FIELDS`] gives one level
/// up, at the kind granularity `GOLDEN_REBASE_PLAN.md` G2.5 introduced.
///
/// This is a **synthetic** drive because two of the nine (`probe`, `meter`) have
/// no live entry in `tests/corpus/ledger.json` today: without it they would be
/// whitelisted, pass `assert_structural`, and then be reachable only by a future
/// author who has no way to know whether the branch works. The drive also pins
/// the three selector rules `excluded()` implements — `name_re` matching, the
/// "a named scope never applies to an unnamed artifact" asymmetry, and the
/// `steps` filter — and that a `divergence`/`skip` entry is never treated as an
/// exclusion.
#[test]
fn every_exclusion_field_is_honoured_by_the_runtime() {
    fn scope(field: &str, name_re: Option<&str>, steps: Option<&[usize]>) -> Scope {
        Scope {
            field: field.to_string(),
            policy: None,
            steps: steps.map(|s| s.iter().copied().collect()),
            node_re: None,
            name_re: name_re.map(|r| Regex::new(r).expect("test regex")),
            channel_idx: None,
            channels: Vec::new(),
            max_rel: 0.0,
            max_abs: 0.0,
            rust: None,
            oracle: None,
            num_rel: None,
            line_re: None,
            hit: AtomicBool::new(false),
            channels_exceeded: AtomicU32::new(0),
        }
    }
    fn entry(id: &str, kind: Kind, scopes: Vec<Scope>) -> Entry {
        Entry {
            id: id.to_string(),
            case: "synthetic:case.dss".to_string(),
            channel: EngineChannel::CapiV0145,
            kind,
            scopes,
            applied: AtomicBool::new(false),
            exceeded_floor: AtomicBool::new(false),
            hits: AtomicUsize::new(0),
        }
    }
    fn runtime(entries: Vec<Entry>) -> LedgerRuntime {
        LedgerRuntime {
            causes: std::collections::BTreeMap::new(),
            entries,
        }
    }
    let case = "synthetic:case.dss";
    let ch = EngineChannel::CapiV0145;

    // (a) Every `excluded()`-routed field applies, and records the hit that makes
    //     a scope which stops matching fail the gate as NEVER APPLIED.
    for field in EXCLUSION_FIELDS {
        if EXCLUSION_FIELDS_WITH_PARTITIONING_HANDLER.contains(&field) {
            continue;
        }
        let rt = runtime(vec![entry(
            "x",
            Kind::Exclusion,
            vec![scope(field, None, None)],
        )]);
        assert!(
            rt.view(case, ch).excluded(field, Some("anything"), 0),
            "exclusion field {field:?} is whitelisted but `excluded()` ignores it — \
             a scope naming it would load cleanly and never apply"
        );
        assert!(
            rt.view(case, ch).excluded(field, None, 0),
            "exclusion field {field:?}: an unnamed artifact must match a scope with \
             no `name_re`"
        );
        assert!(
            rt.entries[0].applied.load(Ordering::Relaxed),
            "exclusion field {field:?}: applying it recorded no hit"
        );
        // …and only for its own field.
        assert!(
            !rt.view(case, ch)
                .excluded("iterations", Some("anything"), 0),
            "exclusion field {field:?} leaked onto another field"
        );
    }

    // (b) Selector semantics, driven on `meter` (no live entry uses them).
    let rt = runtime(vec![entry(
        "x",
        Kind::Exclusion,
        vec![scope("meter", Some("(?i)^em1$"), Some(&[2]))],
    )]);
    let v = rt.view(case, ch);
    assert!(v.excluded("meter", Some("EM1"), 2), "name_re must match");
    assert!(!v.excluded("meter", Some("em2"), 2), "name_re must select");
    assert!(
        !v.excluded("meter", None, 2),
        "a named scope must not match an unnamed artifact"
    );
    assert!(
        !v.excluded("meter", Some("EM1"), 1),
        "the `steps` filter must hold"
    );

    // (c) Only `kind: "exclusion"` excludes. A divergence/skip entry naming the
    //     same field must not silently drop the comparison.
    for kind in [Kind::Divergence, Kind::Skip] {
        let rt = runtime(vec![entry("x", kind, vec![scope("meter", None, None)])]);
        assert!(
            !rt.view(case, ch).excluded("meter", Some("em1"), 0),
            "{kind:?} entry was honoured as an exclusion"
        );
    }

    // (d) Both-ways: the partitioning-handler list names only real exclusion
    //     fields, so it cannot silently excuse a field from clause (a).
    for f in EXCLUSION_FIELDS_WITH_PARTITIONING_HANDLER {
        assert!(
            EXCLUSION_FIELDS.contains(&f),
            "{f:?} is excused from the `excluded()` drive but is not an exclusion field"
        );
    }
}

/// [`LedgerView::property_scope_keys`] — the census's ledger link
/// (`R4133_PROPS_PLAN.md` RP2.1 disposition mode).
///
/// It must name exactly the cells the gate's own `property_handled_keys` would
/// handle, because a claims census reports those as `ledger-hit` and RP4.1 reads
/// the result as "already accounted for". The two share
/// [`LedgerView::property_scopes`] and [`LedgerView::scope_names_prop`], and this
/// test drives the shared decision: the `name_re` selects on the lowercased
/// `element.prop` key, a scope with no `name_re` names **nothing** (the props
/// path has no unnamed artifact), only a `divergence` entry counts, and the
/// query records no hit — a census must never make a stale ledger entry look
/// applied.
#[test]
fn property_scope_keys_names_the_cells_the_gate_would_handle() {
    fn scope(field: &str, name_re: Option<&str>) -> Scope {
        Scope {
            field: field.to_string(),
            policy: None,
            steps: None,
            node_re: None,
            name_re: name_re.map(|r| Regex::new(r).expect("test regex")),
            channel_idx: None,
            channels: Vec::new(),
            max_rel: 0.0,
            max_abs: 0.0,
            rust: None,
            oracle: None,
            num_rel: None,
            line_re: None,
            hit: AtomicBool::new(false),
            channels_exceeded: AtomicU32::new(0),
        }
    }
    fn entry(kind: Kind, scopes: Vec<Scope>) -> Entry {
        Entry {
            id: "x".to_string(),
            case: "synthetic:case.dss".to_string(),
            channel: EngineChannel::CapiV0145,
            kind,
            scopes,
            applied: AtomicBool::new(false),
            exceeded_floor: AtomicBool::new(false),
            hits: AtomicUsize::new(0),
        }
    }
    fn runtime(entries: Vec<Entry>) -> LedgerRuntime {
        LedgerRuntime {
            causes: std::collections::BTreeMap::new(),
            entries,
        }
    }
    let props = vec![
        PropsCap {
            element: "Generator.g1".to_string(),
            props: vec![
                ("kVA".to_string(), "100".to_string()),
                ("kW".to_string(), "50".to_string()),
            ],
        },
        PropsCap {
            element: "Load.l1".to_string(),
            props: vec![("kVA".to_string(), "10".to_string())],
        },
    ];
    let key = |el: &str, p: &str| (el.to_string(), p.to_string());
    let (case, ch) = ("synthetic:case.dss", EngineChannel::CapiV0145);

    // The regex selects on the lowercased `element.prop` key — and on nothing else.
    let rt = runtime(vec![entry(
        Kind::Divergence,
        vec![scope("property", Some(r"^generator\.g1\.kva$"))],
    )]);
    assert_eq!(
        rt.view(case, ch).property_scope_keys(&props),
        [key("generator.g1", "kva")].into_iter().collect()
    );
    // …and the query is a READ: nothing about the entry moved.
    assert!(!rt.entries[0].applied.load(Ordering::Relaxed));
    assert_eq!(rt.entries[0].hits.load(Ordering::Relaxed), 0);

    // A pattern across elements takes every matching cell.
    let rt = runtime(vec![entry(
        Kind::Divergence,
        vec![scope("property", Some(r"\.kva$"))],
    )]);
    assert_eq!(
        rt.view(case, ch).property_scope_keys(&props),
        [key("generator.g1", "kva"), key("load.l1", "kva")]
            .into_iter()
            .collect()
    );

    // A scope with no `name_re` names nothing (the props path has no unnamed
    // artifact — `property_handled_keys` skips it for the same reason), another
    // field never leaks in, and only a `divergence` entry is consulted.
    for (kind, scopes) in [
        (Kind::Divergence, vec![scope("property", None)]),
        (Kind::Divergence, vec![scope("probe", Some(r"\.kva$"))]),
        (Kind::Exclusion, vec![scope("property", Some(r"\.kva$"))]),
        (Kind::Skip, vec![scope("property", Some(r"\.kva$"))]),
    ] {
        let rt = runtime(vec![entry(kind, scopes)]);
        assert!(
            rt.view(case, ch).property_scope_keys(&props).is_empty(),
            "{kind:?} scope must name no property cell"
        );
    }
}

/// The `exclusion` half of fail-on-stale, proven by canary rather than by
/// reading the code — the same standard the `divergence` half was held to in the
/// Phase D/E audits.
///
/// Three arms, because the rule has to fire, has to stop firing, and must not
/// fire on the entries it deliberately does not police: an applied
/// voltages-scoped exclusion that never exceeded the tier floor is STALE; the
/// same entry once something exceeded is fine; and an exclusion with no
/// measurable scope is fine either way (its anti-rot guard is the
/// expected-value pin its `cause` names — see
/// [`LedgerRuntime::has_measurable_scope`]).
#[test]
fn a_voltages_exclusion_that_masks_nothing_is_stale() {
    fn scope(field: &str) -> Scope {
        Scope {
            field: field.to_string(),
            policy: None,
            steps: None,
            node_re: None,
            name_re: None,
            channel_idx: None,
            channels: Vec::new(),
            max_rel: 0.0,
            max_abs: 0.0,
            rust: None,
            oracle: None,
            num_rel: None,
            line_re: None,
            hit: AtomicBool::new(false),
            channels_exceeded: AtomicU32::new(0),
        }
    }
    let mk = |field: &str, exceeded: bool| LedgerRuntime {
        causes: std::collections::BTreeMap::new(),
        entries: vec![Entry {
            id: "canary".to_string(),
            case: "synthetic:case.dss".to_string(),
            channel: EngineChannel::CapiV0145,
            kind: Kind::Exclusion,
            scopes: vec![scope(field)],
            applied: AtomicBool::new(true),
            exceeded_floor: AtomicBool::new(exceeded),
            hits: AtomicUsize::new(1),
        }],
    };

    let err = mk("voltages", false)
        .assert_all_hit()
        .expect_err("a voltages exclusion that masks nothing must fail the gate");
    assert!(
        err.contains("STALE") && err.contains("canary"),
        "wrong failure text: {err}"
    );
    assert!(
        mk("voltages", true).assert_all_hit().is_ok(),
        "a voltages exclusion that still masks something must pass"
    );
    // The coarse fields carry no verdict, so they are not policed here — a rule
    // that reported them stale would red the gate on every honest entry.
    for field in EXCLUSION_FIELDS {
        if field == "voltages" || field == "variables" {
            continue;
        }
        assert!(
            mk(field, false).assert_all_hit().is_ok(),
            "{field:?}-only exclusion was wrongly reported stale"
        );
    }

    // `variables` is the one per-VALUE exclusion field, and it is policed by
    // its own per-SCOPE hit flag rather than by the entry's `exceeded_floor`:
    // a `name_re` that stops matching (a renamed state variable) would leave
    // the entry `applied` through its other scopes and mask nothing in silence.
    // Both directions (RP3.10 audit settlement, finding AT-4).
    let vars_unhit = mk("variables", true);
    let err = vars_unhit
        .assert_all_hit()
        .expect_err("a `variables` scope that matched nothing must fail the gate");
    assert!(
        err.contains("STALE `variables` scope") && err.contains("canary"),
        "wrong failure text: {err}"
    );
    let vars_hit = mk("variables", true);
    vars_hit.entries[0].scopes[0]
        .hit
        .store(true, Ordering::Relaxed);
    assert!(
        vars_hit.assert_all_hit().is_ok(),
        "a `variables` scope that matched a variable must pass"
    );
}

/// The three [`check_scope_channels`] rules, driven on synthetic scopes because
/// `assert_structural` can only ever see the committed `ledger.json` — and the
/// whole point of the rules is what happens to an entry nobody has written yet.
///
/// Scopes are deserialized from JSON rather than built field-by-field so the
/// drive exercises the same `Option<Vec<String>>` shape the loader sees: an
/// absent key and an empty list are both "all sub-channels" to the runtime, and
/// both must be refused.
#[test]
fn a_scope_that_misuses_channels_is_refused_at_load() {
    let scope = |json: &str| -> RawScope { serde_json::from_str(json).expect("test scope") };

    // Rule 1 — a sub-channel field with no `channels`, in both spellings.
    for json in [
        r#"{"field": "element"}"#,
        r#"{"field": "element", "channels": []}"#,
    ] {
        let err = check_scope_channels("bare", &scope(json))
            .expect_err("a bare `element` scope must be refused");
        assert!(
            err.contains("must name its `channels`") && err.contains("bare"),
            "wrong rule-1 text: {err}"
        );
    }
    // …and the named escape hatch is the only way through (empty today, so the
    // rule cannot be dodged without editing BARE_CHANNELS_ALLOWED).
    assert!(
        BARE_CHANNELS_ALLOWED.is_empty(),
        "BARE_CHANNELS_ALLOWED gained {BARE_CHANNELS_ALLOWED:?} — each id needs a \
         written reason, and the population lock must show the entry's digest move"
    );

    // Rule 2 — a typo selects nothing at runtime, so it is refused at load.
    let err = check_scope_channels(
        "typo",
        &scope(r#"{"field": "element", "channels": ["curents"]}"#),
    )
    .expect_err("a misspelled sub-channel must be refused");
    assert!(
        err.contains("curents") && err.contains("select nothing"),
        "wrong rule-2 text: {err}"
    );
    // A real name passes, alone or with its siblings.
    for json in [
        r#"{"field": "element", "channels": ["losses"]}"#,
        r#"{"field": "element", "channels": ["currents", "powers", "losses"]}"#,
    ] {
        check_scope_channels("ok", &scope(json)).expect("a declared sub-channel must pass");
    }

    // Rule 3 — `channels` on a field that does not read it.
    for field in ["voltages", "yprim", "monitor"] {
        let json = format!(r#"{{"field": "{field}", "channels": ["currents"]}}"#);
        let err = check_scope_channels("stray", &scope(&json))
            .expect_err("`channels` on a non-sub-channel field must be refused");
        assert!(
            err.contains("carries `channels`") && err.contains(field),
            "wrong rule-3 text: {err}"
        );
        // …while the same scope without it is fine.
        check_scope_channels("stray", &scope(&format!(r#"{{"field": "{field}"}}"#)))
            .expect("a scope with no `channels` must pass");
    }

    // Every declared sub-channel field is a field the loader accepts at all —
    // otherwise the rules would police a name `compile_scope` already rejects.
    for (field, names) in SUBCHANNEL_FIELDS {
        assert!(
            LEDGER_FIELDS.contains(field),
            "SUBCHANNEL_FIELDS names {field:?}, which is not a ledger field"
        );
        assert!(
            !names.is_empty(),
            "SUBCHANNEL_FIELDS row {field:?} declares no sub-channel names"
        );
    }
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

// --- the polar-channel envelope handler (GOLDEN_REBASE G1.3a F6') -----------

/// A divergence entry scoped to one element's `currents_mag_ang`, with the
/// `injection-ulp` family's committed envelope (`max_abs 2e-5`, `max_rel 1e-8`).
#[cfg(test)]
fn polar_envelope_fixture() -> (Entry, ElementCap, dss_core::exec::ElementSnapshot) {
    let scope = Scope {
        field: "element".to_string(),
        policy: None,
        steps: None,
        node_re: None,
        name_re: None,
        channel_idx: None,
        channels: vec!["currents_mag_ang".to_string()],
        max_rel: 1e-8,
        max_abs: 2e-5,
        rust: None,
        oracle: None,
        num_rel: None,
        line_re: None,
        hit: AtomicBool::new(false),
        channels_exceeded: AtomicU32::new(0),
    };
    let entry = Entry {
        id: "test-polar-envelope".to_string(),
        case: "unit:test".to_string(),
        channel: EngineChannel::R4133,
        kind: Kind::Divergence,
        scopes: vec![scope],
        applied: AtomicBool::new(false),
        exceeded_floor: AtomicBool::new(false),
        hits: AtomicUsize::new(0),
    };
    let cap = ElementCap {
        name: "Transformer.t8".to_string(),
        i_re: vec![0.0],
        i_im: vec![0.0],
        cma_mag: vec![0.0],
        cma_ang: vec![0.0],
        ..ElementCap::default()
    };
    let snap = dss_core::exec::ElementSnapshot {
        name: "Transformer.t8".to_string(),
        enabled: true,
        // One terminal, one conductor: the shape of the single-slot payload
        // below (GOLDEN_REBASE G1.3d(i) added these five fields to
        // `ElementSnapshot`; this fixture never reaches
        // `harness::compare_element_extras`).
        n_terms: 1,
        n_conds: 1,
        n_phases: 1,
        node_order: Vec::new(),
        energy_meter: None,
        // G1.3d(ii) added these six; this fixture reaches neither
        // `harness::compare_element_extras` nor
        // `harness::compare_element_phase_losses`.
        phase_losses: Vec::new(),
        num_controls: 0,
        ocp_dev_index: 0,
        ocp_dev_type: 0,
        has_volt_control: false,
        has_switch_control: false,
        // G1.3b added these four; this fixture reaches
        // `harness::compare_element_seq` through none of its callers, and the
        // `seq_*` sub-channel tests below build their own payload on top of it
        // (`seq_envelope_fixture`).
        seq_arm: SeqArm::NotAvailable,
        seq_currents: Vec::new(),
        seq_voltages: Vec::new(),
        seq_powers: Vec::new(),
        bus_names: vec!["b".to_string()],
        powers: vec![num_complex::Complex64::new(0.0, 0.0)],
        currents: vec![num_complex::Complex64::new(0.0, 0.0)],
        loss_w: (0.0, 0.0),
        currents_mag_ang: vec![Polar { mag: 0.0, ang: 0.0 }],
        voltages_mag_ang: vec![],
        residuals: vec![],
    };
    (entry, cap, snap)
}

// The angle of a phasor whose magnitude is at or under its own band carries no
// information: `harness::polar_close` skips it, and so must the ledger's
// envelope check. Measured on the live corpus (G1.3a F6', 2026-09-04): with the
// angle recorded, widening `r4133-combomidi-injection-ulp` onto
// `currents_mag_ang` failed with `Transformer.t8 cma[9].ang: |diff| 1.394e2
// exceeds envelope 2.063e-5` on a numerically-zero conductor — a "divergence"
// no envelope short of +/-180 deg could admit, i.e. one that bounds nothing.
/// A masked angle (magnitude at or under its band) is not envelope-checked.
#[test]
fn a_masked_polar_angle_is_not_envelope_checked() {
    let (entry, mut cap, mut snap) = polar_envelope_fixture();
    let tol = crate::harness::tol_for("micro");
    // |I| = 1e-9 A, four orders under the micro tier's 1e-6 A abs floor: the
    // magnitude is numerical zero, so the two engines' angles are arbitrary.
    cap.cma_mag[0] = 1e-9;
    cap.cma_ang[0] = -135.0;
    snap.currents_mag_ang[0] = Polar {
        mag: 1e-9,
        ang: 38.66,
    };
    assert!(
        polar_angle_band(tol.i_abs + tol.i_rel * cap.cma_mag[0], cap.cma_mag[0]).is_none(),
        "the fixture must be a MASKED sample or it proves nothing",
    );
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
    // The magnitude is still compared: it is inside both the envelope and the
    // tier floor here, so the entry records no floor-exceed from this sample.
    assert!(
        !entry.exceeded_floor.load(Ordering::Relaxed),
        "a masked angle must not count as the divergence the entry pins",
    );
}

// The other half of the rule: an angle whose magnitude is healthy is still
// envelope-checked, so the skip above cannot become a blanket angle bypass.
/// An unmasked angle above the envelope still fails.
#[test]
#[should_panic(expected = "cma[0].ang")]
fn an_unmasked_polar_angle_still_hits_the_envelope() {
    let (entry, mut cap, mut snap) = polar_envelope_fixture();
    let tol = crate::harness::tol_for("micro");
    // |I| = 1000 A: band = 1e-6 + 1e-9*1000 = 2e-6 A, angular image
    // 57.29577951308232 * 2e-6 / 1000 = 1.1459155902616465e-7 deg, and the
    // envelope is 2e-5 + 1e-8*90 = 2.09e-5 deg. A 1 deg gap blows both.
    cap.cma_mag[0] = 1000.0;
    cap.cma_ang[0] = 90.0;
    snap.currents_mag_ang[0] = Polar {
        mag: 1000.0,
        ang: 91.0,
    };
    assert!(
        polar_angle_band(tol.i_abs + tol.i_rel * cap.cma_mag[0], cap.cma_mag[0]).is_some(),
        "the fixture must be an UNMASKED sample or it proves nothing",
    );
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
}

// Per-SUB-CHANNEL staleness (G1.3a audit settlement, 2026-09-04, finding T3):
// `exceeded_floor` is ONE flag for the whole entry, so a scope widened onto a
// sub-channel that masks nothing rides on a sibling channel's divergence for
// ever and the fail-on-stale gate cannot see it. `Scope::channels_exceeded`
// attributes each floor-exceed to the sub-channel that produced it.
/// A sub-channel that never exceeds its floor is reported stale; its diverging
/// sibling in the same scope is not.
#[test]
fn a_widened_sub_channel_that_masks_nothing_is_reported_stale() {
    let (mut entry, mut cap, mut snap) = polar_envelope_fixture();
    entry.scopes[0].channels = vec!["currents".to_string(), "currents_mag_ang".to_string()];
    let tol = crate::harness::tol_for("micro");
    // `currents` diverges by 1e-5 A: above the micro floor (1e-6 A) and inside
    // the entry's envelope (2e-5 A). `currents_mag_ang` is identical on both
    // sides, i.e. the widening onto it masks nothing.
    cap.i_re[0] = 0.0;
    snap.currents[0] = num_complex::Complex64::new(1e-5, 0.0);
    cap.cma_mag[0] = 0.0;
    cap.cma_ang[0] = 0.0;
    snap.currents_mag_ang[0] = Polar { mag: 0.0, ang: 0.0 };
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
    assert!(
        entry.exceeded_floor.load(Ordering::Relaxed),
        "the entry as a whole still masks something — that is exactly why the \
         per-entry flag cannot report the dead sub-channel",
    );
    entry.hits.store(1, Ordering::Relaxed);
    let rt = LedgerRuntime {
        causes: std::collections::BTreeMap::new(),
        entries: vec![entry],
    };
    let err = rt
        .assert_all_hit()
        .expect_err("a sub-channel that masks nothing must fail the gate");
    assert!(
        err.contains("STALE `element`") && err.contains("[\"currents_mag_ang\"]"),
        "wrong failure text: {err}"
    );
    // The other direction: once that sub-channel does exceed its floor, the
    // entry passes — the rule reports dead channels, not every channel.
    rt.entries[0].scopes[0].mark_channel_exceeded("currents_mag_ang");
    assert!(
        rt.assert_all_hit().is_ok(),
        "a sub-channel that masks something must pass"
    );
}

// --- the `phase_losses` sub-channel (GOLDEN_REBASE G1.3d(ii) F4) ------------

/// A divergence entry scoped to one element's `phase_losses`, on a
/// single-terminal single-conductor payload whose `Powers`/`Currents` layout the
/// band indexes through: |V| = 1 kV at 1 A, so `Powers[0] = PhaseLosses[0] =
/// 1 kW` — the identity `phase_loss_band` is derived from.
#[cfg(test)]
fn phase_loss_envelope_fixture() -> (Entry, ElementCap, dss_core::exec::ElementSnapshot) {
    let (mut entry, mut cap, mut snap) = polar_envelope_fixture();
    entry.id = "test-phase-loss-envelope".to_string();
    entry.scopes[0].channels = vec!["phase_losses".to_string()];
    cap.n_terms = Some(1);
    cap.n_conds = Some(1);
    cap.n_phases = Some(1);
    cap.i_re = vec![1.0];
    cap.i_im = vec![0.0];
    cap.p_kw = vec![1.0];
    cap.p_kvar = vec![0.0];
    cap.pl_kw = vec![1.0];
    cap.pl_kvar = vec![0.0];
    snap.powers = vec![num_complex::Complex64::new(1.0, 0.0)];
    snap.currents = vec![num_complex::Complex64::new(1.0, 0.0)];
    // The engine reports W/var; the capture kW/kvar. Equal values here.
    snap.phase_losses = vec![num_complex::Complex64::new(1000.0, 0.0)];
    (entry, cap, snap)
}

/// The `phase_losses` envelope bands each phase with `harness::phase_loss_band`
/// and attributes the floor-exceed to that sub-channel — the same two-sided
/// accounting the six older sub-channels get.
#[test]
fn the_phase_loss_envelope_bands_the_sample_and_attributes_the_exceed() {
    let tol = crate::harness::tol_for("micro");
    // band = i_abs·max(1, |V_kv|) + i_rel·|S| = 1e-6·1 + 1e-9·1 = 1.001e-6 kW.
    let band = phase_loss_band(
        &phase_loss_envelope_fixture().1,
        0,
        1,
        1,
        tol.i_rel,
        tol.i_abs,
    );
    assert!(
        (band - 1.001e-6).abs() < 1e-18,
        "the fixture's band moved: {band:e}"
    );

    // (a) inside the floor: nothing to mask, so the sub-channel is STALE.
    let (entry, cap, snap) = phase_loss_envelope_fixture();
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
    assert!(
        !entry.exceeded_floor.load(Ordering::Relaxed),
        "a sample inside its floor must not count as a divergence"
    );
    assert_eq!(entry.scopes[0].dead_channels(), vec!["phase_losses"]);

    // (b) above the floor and inside the entry's envelope (2e-5 + 1e-8 kW):
    //     the exceed is recorded against `phase_losses`, not a sibling.
    let (entry, cap, mut snap) = phase_loss_envelope_fixture();
    snap.phase_losses[0].re = 1000.0 + 1e-2;
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
    assert!(
        entry.exceeded_floor.load(Ordering::Relaxed),
        "1e-5 kW is 10x the 1e-6 kW band and must be recorded"
    );
    assert!(entry.scopes[0].dead_channels().is_empty());
}

/// …and a sample outside the entry's committed envelope still fails the gate:
/// the widening is a bounded pin, never a blanket.
#[test]
#[should_panic(expected = "pl[0]")]
fn the_phase_loss_envelope_still_fails_outside_the_committed_bound() {
    let tol = crate::harness::tol_for("micro");
    let (entry, cap, mut snap) = phase_loss_envelope_fixture();
    // 1e-4 kW: 5x the entry's 2.001e-5 kW envelope.
    snap.phase_losses[0].re = 1000.0 + 1e-1;
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
}

/// The exclusion half: a `phase_losses` scope rewrites the capture's kW/kvar
/// halves from the snapshot — with the ×0.001 — so `compare_element_phase_losses`
/// sees them equal, and touches nothing else.
#[test]
fn a_phase_losses_scope_rewrites_the_capture_in_kw() {
    let (entry, mut cap, mut snap) = phase_loss_envelope_fixture();
    snap.phase_losses[0] = num_complex::Complex64::new(-2500.0, 750.0);
    cap.i_re[0] = 7.0;
    rewrite_element_selected(&mut cap, &entry.scopes[0], &snap);
    assert_eq!(cap.pl_kw, vec![-2.5]);
    assert_eq!(cap.pl_kvar, vec![0.75]);
    assert_eq!(
        cap.i_re,
        vec![7.0],
        "an unselected sub-channel must be left alone"
    );
    assert_eq!(
        cap.p_kw,
        vec![1.0],
        "an unselected sub-channel must be left alone"
    );
}

// --- the three `seq_*` sub-channels (GOLDEN_REBASE G1.3b F4) ----------------

/// A divergence entry scoped to all three of one element's sequence channels,
/// on a payload shaped by the requested [`SeqArm`]:
///
/// * [`SeqArm::ThreePhase`] — 1 terminal x 3 conductors, every slot banded;
/// * [`SeqArm::PosSeqSinglePhase`] — 2 terminals x 1 conductor, only slots 1 and
///   4 banded (`3t+1`);
/// * [`SeqArm::NotAvailable`] — 1 terminal x 1 conductor, no slot banded and the
///   payload the two engines' sentinels produce.
///
/// The phase magnitudes are `1000 A` / `1000 V` on every conductor, so at the
/// `micro` tier the two magnitude bands are `1e-6 + 1e-9*1000 = 2e-6` on
/// `capi_v0145` and `2e-6 + SEQ_C012*1000` on `r4133`, and the power band is
/// `0.003*(2e-6*(1000 + 2e-6) + 1000*2e-6) = 1.2e-5` kVA. The entry keeps the
/// `injection-ulp` family's committed envelope (`2e-5 + 1e-8*base`).
#[cfg(test)]
fn seq_envelope_fixture(
    arm: SeqArm,
    channel: EngineChannel,
) -> (Entry, ElementCap, dss_core::exec::ElementSnapshot) {
    let (mut entry, mut cap, mut snap) = polar_envelope_fixture();
    entry.id = "test-seq-envelope".to_string();
    entry.channel = channel;
    entry.scopes[0].channels = vec![
        "seq_currents".to_string(),
        "seq_voltages".to_string(),
        "seq_powers".to_string(),
    ];
    let (nterms, nconds, nphases) = match arm {
        SeqArm::ThreePhase => (1usize, 3usize, 3usize),
        SeqArm::PosSeqSinglePhase => (2, 1, 1),
        SeqArm::NotAvailable => (1, 1, 2),
    };
    let yorder = nterms * nconds;
    snap.n_terms = nterms;
    snap.n_conds = nconds;
    snap.n_phases = nphases;
    snap.seq_arm = arm;
    cap.n_terms = Some(nterms as i32);
    cap.n_conds = Some(nconds as i32);
    cap.n_phases = Some(nphases as i32);
    cap.cma_mag = vec![1000.0; yorder];
    cap.cma_ang = vec![0.0; yorder];
    cap.vma_mag = vec![1000.0; yorder];
    cap.vma_ang = vec![0.0; yorder];
    cap.i_re = vec![0.0; yorder];
    cap.i_im = vec![0.0; yorder];
    snap.currents = vec![num_complex::Complex64::new(0.0, 0.0); yorder];
    snap.currents_mag_ang = vec![
        Polar {
            mag: 1000.0,
            ang: 0.0
        };
        yorder
    ];
    let n = 3 * nterms;
    let mut mag = vec![0.0; n];
    let mut kw = vec![0.0; n];
    let kvar = vec![0.0; n];
    for k in 0..n {
        if seq_slot_is_banded(arm, k) {
            // 0.003 * 1000 V * 1000 A = 3000 kW, the identity the power band is
            // derived from.
            mag[k] = 1000.0;
            kw[k] = 3000.0;
        } else if arm == SeqArm::NotAvailable {
            // `Cabs(-1 + 0j)` and r4133's `(-1, 0)` power sentinel — the arm's
            // whole payload, discrete on both sides.
            mag[k] = 1.0;
            kw[k] = -1.0;
        }
    }
    cap.seq_i = mag.clone();
    cap.seq_v = mag.clone();
    cap.seq_p_kw = kw.clone();
    cap.seq_p_kvar = kvar.clone();
    snap.seq_currents = mag.clone();
    snap.seq_voltages = mag;
    snap.seq_powers = kw
        .iter()
        .zip(&kvar)
        .map(|(a, b)| num_complex::Complex64::new(*a, *b))
        .collect();
    (entry, cap, snap)
}

/// The `seq_*` envelope bands each slot with `harness::seq_band` /
/// `harness::seq_power_band` and attributes the floor-exceed to the sub-channel
/// that produced it — the same two-sided accounting the seven older
/// sub-channels get.
#[test]
fn the_seq_envelope_bands_the_sample_and_attributes_the_exceed() {
    let tol = crate::harness::tol_for("micro");
    // The fixture's bands, stated so the numbers below are not magic.
    let bi = seq_band(&[1000.0; 3], tol.i_rel, tol.i_abs, 0.0);
    assert!(
        (bi - 2e-6).abs() < 1e-18,
        "the fixture's band moved: {bi:e}"
    );
    // 0.003*(2e-6*(1000 + 2e-6) + 1000*2e-6) = 1.2e-5 + 1.2e-11, the
    // second-order term kept (`harness::seq_power_band`).
    let bs = seq_power_band(bi, bi, 1000.0, 1000.0);
    assert!(
        (bs - 1.2000000012000002e-5).abs() < 1e-20,
        "the power band moved: {bs:e}"
    );

    // (a) inside the floor: nothing to mask, so all three are STALE.
    let (entry, cap, snap) = seq_envelope_fixture(SeqArm::ThreePhase, EngineChannel::CapiV0145);
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
    assert!(
        !entry.exceeded_floor.load(Ordering::Relaxed),
        "a sample inside its floor must not count as a divergence"
    );
    assert_eq!(
        entry.scopes[0].dead_channels(),
        vec!["seq_currents", "seq_voltages", "seq_powers"]
    );

    // (b) above the floor and inside the entry's envelope (2e-5 + 1e-8*1000):
    //     the exceed is recorded against `seq_currents`, not a sibling.
    let (entry, cap, mut snap) = seq_envelope_fixture(SeqArm::ThreePhase, EngineChannel::CapiV0145);
    snap.seq_currents[0] += 1e-5;
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
    assert!(
        entry.exceeded_floor.load(Ordering::Relaxed),
        "1e-5 A is 5x the 2e-6 A band and must be recorded"
    );
    assert_eq!(
        entry.scopes[0].dead_channels(),
        vec!["seq_voltages", "seq_powers"]
    );

    // (c) the power channel on its own scale: 4e-5 kVA is 3.3x its 1.2e-5 band
    //     and inside its own envelope (2e-5 + 1e-8*3000 kVA).
    let (entry, cap, mut snap) = seq_envelope_fixture(SeqArm::ThreePhase, EngineChannel::CapiV0145);
    snap.seq_powers[2].im += 4e-5;
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
    assert_eq!(
        entry.scopes[0].dead_channels(),
        vec!["seq_currents", "seq_voltages"]
    );
}

/// …and a sample outside the entry's committed envelope still fails the gate:
/// the widening is a bounded pin, never a blanket.
#[test]
#[should_panic(expected = "seq_i[0]")]
fn the_seq_envelope_still_fails_outside_the_committed_bound() {
    let tol = crate::harness::tol_for("micro");
    let (entry, cap, mut snap) = seq_envelope_fixture(SeqArm::ThreePhase, EngineChannel::CapiV0145);
    // 1e-4 A: over 3x the entry's 3e-5 A envelope at |I012| = 1000 A.
    snap.seq_currents[0] += 1e-4;
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
}

/// The r4133 channel — and only it — carries the truncated-matrix term
/// (`SEQ_C012 * max_j |Xph_j|`, `Shared/mathutil.pas:302-303` + `:562-564`), on
/// the three-phase arm where a matrix actually runs. A gap of `2.3e-6 A` sits
/// between the two channels' bands: `capi_v0145` records it as a floor-exceed,
/// `r4133` does not.
#[test]
fn the_seq_envelope_carries_the_truncated_matrix_term_on_r4133_only() {
    let tol = crate::harness::tol_for("micro");
    let capi = seq_band(&[1000.0; 3], tol.i_rel, tol.i_abs, 0.0);
    let r4133 = seq_band(&[1000.0; 3], tol.i_rel, tol.i_abs, SEQ_C012);
    assert!(
        capi < 2.3e-6 && 2.3e-6 < r4133,
        "the probe must sit between the two bands ({capi:e} .. {r4133:e})"
    );
    for (channel, dead) in [
        (EngineChannel::CapiV0145, false),
        (EngineChannel::R4133, true),
    ] {
        let (entry, cap, mut snap) = seq_envelope_fixture(SeqArm::ThreePhase, channel);
        snap.seq_currents[0] += 2.3e-6;
        envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
        assert_eq!(
            entry.scopes[0].dead_channels().contains(&"seq_currents"),
            dead,
            "{channel:?}: the truncated-matrix term is r4133's alone"
        );
    }
}

/// A sub-channel the scope does NOT name is not envelope-checked…
#[test]
fn a_masked_seq_channel_is_not_envelope_checked() {
    let tol = crate::harness::tol_for("micro");
    let (mut entry, cap, mut snap) =
        seq_envelope_fixture(SeqArm::ThreePhase, EngineChannel::CapiV0145);
    entry.scopes[0].channels = vec!["seq_currents".to_string()];
    // 1 kVA on the powers — five orders over the entry's envelope — and the
    // scope does not name that channel, so nothing here is measured…
    snap.seq_powers[0].re += 1.0;
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
    // …while the named channel was measured and, being clean, is reported stale.
    assert_eq!(entry.scopes[0].dead_channels(), vec!["seq_currents"]);
}

/// …and one it does name still hits the envelope, so the selector cannot become
/// a blanket bypass.
#[test]
#[should_panic(expected = "seq_p[0]")]
fn an_unmasked_seq_channel_still_hits_the_envelope() {
    let tol = crate::harness::tol_for("micro");
    let (mut entry, cap, mut snap) =
        seq_envelope_fixture(SeqArm::ThreePhase, EngineChannel::CapiV0145);
    entry.scopes[0].channels = vec!["seq_powers".to_string()];
    snap.seq_powers[0].re += 1.0;
    envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
}

/// The exclusion half: a `seq_*` scope rewrites the capture's sequence arrays
/// from the snapshot — same units, no scaling — and touches nothing else.
#[test]
fn a_seq_scope_rewrites_the_capture_in_the_wires_own_units() {
    let (entry, mut cap, mut snap) =
        seq_envelope_fixture(SeqArm::ThreePhase, EngineChannel::CapiV0145);
    snap.seq_currents = vec![11.0, 12.0, 13.0];
    snap.seq_voltages = vec![21.0, 22.0, 23.0];
    snap.seq_powers = vec![
        num_complex::Complex64::new(-2500.0, 750.0),
        num_complex::Complex64::new(1.0, 2.0),
        num_complex::Complex64::new(3.0, 4.0),
    ];
    cap.i_re[0] = 7.0;
    rewrite_element_selected(&mut cap, &entry.scopes[0], &snap);
    assert_eq!(cap.seq_i, vec![11.0, 12.0, 13.0]);
    assert_eq!(cap.seq_v, vec![21.0, 22.0, 23.0]);
    assert_eq!(cap.seq_p_kw, vec![-2500.0, 1.0, 3.0]);
    assert_eq!(cap.seq_p_kvar, vec![750.0, 2.0, 4.0]);
    assert_eq!(
        cap.i_re[0], 7.0,
        "an unselected sub-channel must be left alone"
    );
    assert_eq!(
        cap.cma_mag,
        vec![1000.0; 3],
        "an unselected sub-channel must be left alone"
    );
}

/// **The discrete rail.** The envelope and the rewrite cover exactly the slots
/// [`seq_slot_is_banded`] calls banded — measured slot by slot, on all three
/// arms, from both sides: a slot the envelope measures is one the rewrite
/// neutralizes, and a discrete slot is neither. That is what stops any `seq_*`
/// ledger scope from excusing a discrete miss (the not-available arm's
/// `1.0`/sentinel payload, or the exact zeros beside the positive-sequence slot
/// that r4133's `DDLL/DCktElement.pas:760`/`:768` fills wrongly).
#[test]
fn the_seq_rewrite_and_the_seq_envelope_cover_the_same_slots() {
    let tol = crate::harness::tol_for("micro");
    for arm in [
        SeqArm::ThreePhase,
        SeqArm::PosSeqSinglePhase,
        SeqArm::NotAvailable,
    ] {
        let n = seq_envelope_fixture(arm, EngineChannel::CapiV0145)
            .1
            .seq_i
            .len();
        assert!(n > 0, "{arm:?}: the fixture must carry a payload");
        for k in 0..n {
            let banded = seq_slot_is_banded(arm, k);

            // The envelope side: a gap above the floor and inside the committed
            // envelope is recorded iff the slot is banded.
            let (entry, cap, mut snap) = seq_envelope_fixture(arm, EngineChannel::CapiV0145);
            snap.seq_currents[k] += 1e-5;
            envelope_element(&entry, &entry.scopes[0], &snap, &cap, &tol, "unit");
            assert_eq!(
                !entry.scopes[0].dead_channels().contains(&"seq_currents"),
                banded,
                "{arm:?} slot {k}: envelope coverage disagrees with seq_slot_is_banded"
            );

            // The rewrite side: the slot is neutralized iff it is banded.
            let (entry, mut cap, mut snap) = seq_envelope_fixture(arm, EngineChannel::CapiV0145);
            let before = cap.seq_i[k];
            snap.seq_currents[k] = 12345.0;
            rewrite_element_selected(&mut cap, &entry.scopes[0], &snap);
            assert_eq!(
                cap.seq_i[k] == 12345.0,
                banded,
                "{arm:?} slot {k}: rewrite coverage disagrees with seq_slot_is_banded \
                 (was {before}, now {})",
                cap.seq_i[k]
            );
        }
    }
}
