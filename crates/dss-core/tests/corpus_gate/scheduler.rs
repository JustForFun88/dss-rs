//! Hand-rolled scheduler for the unified corpus gate (UNIFIED_GATE_PLAN §3, D7a).
//!
//! One `#[test]` runs the UNION of all four manifests. The task list is grouped
//! by case directory (§3.3 simplification: task = case-dir group, its cases run
//! sequentially inside the task — this removes the D9 per-dir mutexes entirely,
//! since only one thread ever touches a given case dir), pre-sorted longest-first
//! by a static weight, and drained by `available_parallelism()` (or
//! `DSS_GATE_JOBS`) threads via an `AtomicUsize` cursor + `std::thread::scope` —
//! NOT rayon/tokio (tasks block on oracle subprocess I/O; the global rayon pool
//! belongs to faer inside the solves).
//!
//! The pinned `capi_v0145` channel is served by a persistent [`WorkerPool`]; the
//! `r4133` channel by a persistent [`EpriPool`] of `epri-worker` bridge processes
//! (Phase C/D — the pre-Phase-C one-shot target-rev `Oracle` shim is retired;
//! one-shots remain only for `isolate`/serial runs). Each case is
//! `catch_unwind`-wrapped; the test fails iff any case failed, printing the
//! COMPLETE failure list in manifest order.
//!
//! Contamination-proof knobs (§4 Phase B DONE bar):
//! * `DSS_GATE_SERIAL=1` — T=1 and the pinned channel uses a FRESH one-shot
//!   process per case (no persistent-worker state reuse): the reference run.
//! * `DSS_GATE_SHUFFLE=<seed>` — shuffle the task order (order-sensitivity probe).
//! * `DSS_GATE_DUMP=<path>` — write a label-sorted `{label: {verdict, result}}`
//!   artifact for a three-way bit-diff.
//! * `DSS_GATE_JOBS=<n>` — override the scheduler thread count.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::engines::{
    CaseResult, Channel, EpriOneShot, EpriPool, Oracle, WorkerPool, build_run_request,
};
use crate::harness::{self, CensusBlindSpots};
use crate::ledger::LedgerRuntime;
use crate::manifest::{
    EngineChannel, FAMILIES, SolvableCase, corpus_file, family_file, load_family, load_solvable,
};
use crate::props_census::{Mode as CensusMode, Row as CensusRow, RowKind as CensusRowKind};
use crate::runner::{
    CorpusGuard, assert_deferred_rust_smoke, assert_pending_errors_loudly, compare_with_result,
    panic_msg, run_and_compare_abort, run_rust_capture,
};

// ---------------------------------------------------------------------------
// Unified case model.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum CaseClass {
    Live,
    Abort,
    Pending,
    /// `defer_ledger` — parked from live oracle comparison (Phase D ledger seed);
    /// Rust-smoke only.
    Deferred,
}

struct UnifiedCase {
    order: usize,
    label: String,
    abs: String,
    dir_key: String,
    weight: u64,
    class: CaseClass,
    case: SolvableCase,
}

struct Task {
    dir_key: String,
    weight: u64,
    cases: Vec<UnifiedCase>,
}

/// Static longest-first weight: coarse solve-cost tier × step count.
fn kind_weight(kind: &str) -> u64 {
    if kind.starts_with("large") {
        1000
    } else if kind == "feeder" {
        10
    } else if kind == "midi" {
        3
    } else {
        1 // micro / synthetic
    }
}

/// Directory key for grouping (case-insensitive, forward-slashed parent).
fn dir_key_of(abs: &str) -> String {
    abs.rsplit_once('/')
        .map(|(d, _)| d)
        .unwrap_or("")
        .to_lowercase()
}

/// Apply the per-source property-forcing rule to a live case.
///
/// **Since R4133_PROPS RP4.1 (2026-09-03) the property compare runs on BOTH
/// channels.** It used to be a capi_v0145-channel feature — §1.2 pinned property
/// parity to the capi oracle while the r4133 bridge had no all-properties
/// capture — so it was forced only for cases gating capi, and [`run_one_case`]
/// plus [`seed_one`] masked it off per-channel on the r4133 request. Both
/// premises are gone: the bridge grew `capture_all_properties`
/// (`crates/dss-epri/src/capture.rs`) at the unified gate's Phase D, and RP4.1
/// removed the two masks, so an r4133-gating case now has its property table
/// compared exactly like a capi one. The request costs an extra per-case
/// `? name.Like` + `? name.prop` sweep on the r4133 worker.
///
/// The rule is therefore **every live case**, with the plan's one cost guard:
/// `kind=large*` decks stay out on the `solvable_now` arm. That guard is also
/// definitional — the RP0.1/RP0.2 census population every property measurement
/// in `R4133_PROPS_PLAN.md` rests on is exactly "live, non-`large`"
/// ([`run_props_census`]).
///
/// Written without a channel predicate on purpose: `gates_capi() ||
/// gates_r4133()` is a tautology over the three legal `engines` values
/// (`capi_v0145|both|r4133`), and a tautological guard reads like a filter while
/// filtering nothing (the RegControl no-load-zone precedent, CLAUDE.md).
fn force_properties(source: &str, c: &mut SolvableCase, fam_props: bool) {
    match source {
        "solvable_now" => {
            if !c.kind.starts_with("large") {
                c.compare_all_properties = true;
            }
        }
        _ => {
            // family: fam_props is the family-level flag (true for all three).
            c.compare_all_properties |= fam_props;
        }
    }
}

/// **The forced property population, pinned** — `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`.
///
/// Measured off the four manifests at RP4.1's audit settlement (2026-09-03),
/// re-derived by [`the_property_forcing_rule_is_every_live_non_large_case`] on
/// every run: 523 cases → 519 live → **440** forced once the 79 live
/// `kind=large*` decks (all of them `solvable_now`) come off, of which **396**
/// gate the r4133 channel (313 `both` + 83 r4133-only) and 44 are capi-only.
///
/// 440 is exactly the census population every property measurement in
/// `R4133_PROPS_PLAN.md` rests on ([`run_props_census`] walks the same set), so
/// this lock also keeps the census and the gate talking about one population.
const FORCED_PROPS_POPULATION: (usize, usize, usize, usize) = (440, 313, 83, 44);

/// **The property-forcing rule is a rule, not a habit** — the static half of
/// RP4.1's re-mask alarm (audit settlement, 2026-09-03).
///
/// [`force_properties`] is what the whole r4133 property compare hangs on, and
/// nothing else can see it: `population.lock.json` fingerprints manifest flags
/// and per-case ledger tags, not scheduler code, and the live guard
/// `props_norm::assert_r4133_props_compare_ran` is a boolean — a *partial*
/// re-mask (re-adding `gates_capi()`, which would drop the 83 r4133-only cases
/// while the 313 `both` ones keep walking) passes it. This test is the one that
/// does not: it walks the four manifests without an oracle and asserts the
/// forced set **is** the live non-`large` population, cell for cell, with the
/// per-`engines` split pinned by [`FORCED_PROPS_POPULATION`].
///
/// It also guards the family arm's missing cost guard: the arm ORs the
/// family-level flag with no `kind=large*` test, which is inert only while no
/// family case is `large`. That premise is asserted here rather than assumed,
/// so a family deck that ever grows into `large` is a review, not a silent
/// +1000-weight property sweep.
#[test]
fn the_property_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        let family = uc.label.split(':').next() != Some("solvable_now");
        assert!(
            !(family && large),
            "{}: a family deck is `kind={}` — `force_properties`' family arm carries no `large` \
             cost guard, so this case would be property-forced without review. Add the guard, or \
             re-classify the deck.",
            uc.label,
            uc.case.kind
        );
        // The rule, per source: every live case, minus `large` on `solvable_now`.
        // A manifest may also set the flag itself, which only ever ADDS.
        let expected = live && !large;
        if uc.case.compare_all_properties != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_all_properties={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_all_properties,
            ));
        }
        if uc.case.compare_all_properties {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the property-forcing rule is `every live non-`large` case` since R4133_PROPS RP4.1 \
         (2026-09-03) — these cases disagree with it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        forced, FORCED_PROPS_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP in the r4133 halves is a re-mask of \
         the property request — the thing `props_norm::assert_r4133_props_compare_ran` cannot \
         see, because the `both` half keeps it green. A legitimate corpus change moves this lock \
         together with `population.lock.json`."
    );
}

/// Apply the per-source **PDElements**-forcing rule to a live case
/// (`GOLDEN_REBASE_PLAN.md` §G1.6b).
///
/// The rule is [`force_properties`]': **every live case**, with the same one
/// cost guard — `kind=large*` decks stay out on the `solvable_now` arm. It is
/// deliberately the same population, so the two whole-model surfaces the gate
/// forces (properties and PDElements) never drift into talking about different
/// case sets; [`FORCED_PDELEMENTS_POPULATION`] re-derives it and pins the split.
///
/// Two shape notes, both deliberate:
///
/// * no channel predicate — `gates_capi() || gates_r4133()` is a tautology over
///   the three legal `engines` values, and a tautological guard reads like a
///   filter while filtering nothing (the RegControl no-load-zone precedent,
///   CLAUDE.md). Both transports capture the walk
///   (`tools/oracle/oracle_server.py::capture_pd_elements`,
///   `crates/dss-epri/src/capture.rs::capture_pd_elements`), and D2 requires
///   both channels in the same commit;
/// * no family-flag parameter — [`force_properties`] takes `fam_props` because
///   `Family` carries a per-family `compare_all_properties`; there is no such
///   knob for this surface and inventing one would be a false switch (all three
///   families would set it). The family arm therefore forces unconditionally,
///   exactly what `fam_props` evaluates to for all three families today, and
///   the missing `large` guard on that arm is asserted inert by
///   [`the_pdelements_forcing_rule_is_every_live_non_large_case`] rather than
///   assumed.
///
/// Cost: measured ≈ 5 µs per PD element on the capi channel (1 283 elements in
/// 5.9 ms), i.e. well under a second added across the whole population against
/// a ~155 s corpus gate.
fn force_pdelements(source: &str, c: &mut SolvableCase) {
    match source {
        "solvable_now" => {
            if !c.kind.starts_with("large") {
                c.compare_pdelements = true;
            }
        }
        // family: the surface applies to every live family deck.
        _ => c.compare_pdelements = true,
    }
}

/// **The forced PDElements population, pinned** — `(cases forced, of them
/// `engines: "both"`, `engines: "r4133"`, `engines: "capi_v0145"`)`.
///
/// Identical to [`FORCED_PROPS_POPULATION`] by construction (both rules are
/// "every live case, minus `kind=large*` on `solvable_now`"), and re-derived —
/// never copied — by [`the_pdelements_forcing_rule_is_every_live_non_large_case`]
/// on every run. It is written out rather than aliased so that a future
/// divergence between the two rules shows up as a lock diff on the surface that
/// moved, instead of silently following the other one.
const FORCED_PDELEMENTS_POPULATION: (usize, usize, usize, usize) = (440, 313, 83, 44);

/// **The PDElements-forcing rule is a rule, not a habit** — the static half of
/// G1.6b's re-mask alarm, modeled on
/// [`the_property_forcing_rule_is_every_live_non_large_case`] and load-bearing
/// for the same reason: `population.lock.json` fingerprints the *manifest*
/// `compare_pdelements` flag (`population_lock.rs::rigor`'s `pde=` token), and
/// no manifest sets it — the whole population is scheduler-forced, so the lock
/// cannot see a re-mask here at all.
///
/// `harness::assert_pd_elements_compare_ran` is the live half, and it is a
/// boolean: re-adding a channel predicate (say `gates_capi()`, which drops the
/// 83 `engines: "r4133"` cases while the 313 `both` ones keep walking) passes
/// it. This test does not — it walks the four manifests without an oracle and
/// asserts the forced set **is** the live non-`large` population, cell for
/// cell, with the per-`engines` split pinned by
/// [`FORCED_PDELEMENTS_POPULATION`].
#[test]
fn the_pdelements_forcing_rule_is_every_live_non_large_case() {
    let cases = build_unified_cases();
    let mut forced = (0usize, 0usize, 0usize, 0usize);
    let mut wrong: Vec<String> = Vec::new();
    for uc in &cases {
        let live = uc.class == CaseClass::Live;
        let large = uc.case.kind.starts_with("large");
        let family = uc.label.split(':').next() != Some("solvable_now");
        assert!(
            !(family && large),
            "{}: a family deck is `kind={}` — `force_pdelements`' family arm carries no `large` \
             cost guard, so this case would be PDElements-forced without review. Add the guard, \
             or re-classify the deck.",
            uc.label,
            uc.case.kind
        );
        // The rule, per source: every live case, minus `large` on
        // `solvable_now`. A manifest may also set the flag itself, which only
        // ever ADDS (none does today — the `props=0`-everywhere shape).
        let expected = live && !large;
        if uc.case.compare_pdelements != expected {
            wrong.push(format!(
                "{}: kind={} engines={} class={} → compare_pdelements={} (expected {expected})",
                uc.label,
                uc.case.kind,
                uc.case.engines,
                if live { "live" } else { "not-live" },
                uc.case.compare_pdelements,
            ));
        }
        if uc.case.compare_pdelements {
            forced.0 += 1;
            match uc.case.engines.as_str() {
                "both" => forced.1 += 1,
                "r4133" => forced.2 += 1,
                _ => forced.3 += 1,
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "the PDElements-forcing rule is `every live non-`large` case` since GOLDEN_REBASE G1.6b \
         (2026-09-04) — these cases disagree with it:\n  {}",
        wrong.join("\n  ")
    );
    assert_eq!(
        forced, FORCED_PDELEMENTS_POPULATION,
        "(forced, both, r4133-only, capi-only) moved. A DROP in the r4133 halves is a re-mask of \
         the PDElements request — the thing `harness::assert_pd_elements_compare_ran` cannot \
         see once the `both` half keeps it green. A legitimate corpus change moves this lock \
         together with `population.lock.json`."
    );
    // The two forced surfaces must stay one population (see
    // `FORCED_PDELEMENTS_POPULATION`): a divergence here means one of the two
    // rules moved without the other, which is a review, not a silent split.
    assert_eq!(
        forced, FORCED_PROPS_POPULATION,
        "the PDElements and property forcing rules have drifted apart"
    );
}

/// Build one unified case, applying the exact per-source property-forcing +
/// classification of the pre-Phase-B gates.
fn make_case(
    order: usize,
    source: &str,
    mut c: SolvableCase,
    abs: String,
    fam_props: bool,
) -> UnifiedCase {
    // Structural guard carried over from the pre-Phase-B family gate
    // (`family_cases_match_oracle`): `pending` (Rust must error loudly, oracle-free)
    // and `expect_solve_abort` (both engines must abort the solve) are contradictory
    // contracts. The classifier below resolves ties pending-first, so without this a
    // both-flags manifest case would silently skip the stronger abort contract.
    // Applied to EVERY source here (the old check was family-only) — strictly stronger.
    assert!(
        !(c.pending && c.expect_solve_abort.is_some()),
        "{source}:{}: `pending` and `expect_solve_abort` are mutually exclusive",
        c.path
    );
    crate::manifest::assert_defer_ledger_is_valid(&format!("{source}:{}", c.path), &c);
    let class = if c.defer_ledger.is_some() {
        CaseClass::Deferred
    } else if c.pending {
        CaseClass::Pending
    } else if c.expect_solve_abort.is_some() {
        CaseClass::Abort
    } else {
        CaseClass::Live
    };
    if class == CaseClass::Live {
        force_properties(source, &mut c, fam_props);
        force_pdelements(source, &mut c);
    }
    let weight = kind_weight(&c.kind) * (c.n_steps.max(1) as u64);
    let dir_key = dir_key_of(&abs);
    UnifiedCase {
        order,
        label: format!("{source}:{}", c.path),
        abs,
        dir_key,
        weight,
        class,
        case: c,
    }
}

/// Build the union of all four manifests into unified cases.
fn build_unified_cases() -> Vec<UnifiedCase> {
    let mut out: Vec<UnifiedCase> = Vec::new();
    for c in load_solvable() {
        let abs = corpus_file(&c.path);
        out.push(make_case(out.len(), "solvable_now", c, abs, false));
    }
    for fam in FAMILIES {
        for c in load_family(fam.name) {
            let abs = family_file(fam.name, &c.path);
            out.push(make_case(
                out.len(),
                fam.name,
                c,
                abs,
                fam.compare_all_properties,
            ));
        }
    }
    out
}

/// A tiny deterministic PRNG (splitmix64) for the shuffle probe — avoids a dev
/// dependency and keeps the shuffle reproducible from `DSS_GATE_SHUFFLE=<seed>`.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Group cases into per-directory tasks, sorted longest-first (or shuffled).
fn build_tasks(cases: Vec<UnifiedCase>, shuffle_seed: Option<u64>) -> Vec<Task> {
    let mut by_dir: BTreeMap<String, Task> = BTreeMap::new();
    for uc in cases {
        let t = by_dir.entry(uc.dir_key.clone()).or_insert_with(|| Task {
            dir_key: uc.dir_key.clone(),
            weight: 0,
            cases: Vec::new(),
        });
        t.weight += uc.weight;
        t.cases.push(uc);
    }
    let mut tasks: Vec<Task> = by_dir.into_values().collect();
    // Keep cases within a task in manifest order (stable — build order).
    for t in &mut tasks {
        t.cases.sort_by_key(|c| c.order);
    }
    match shuffle_seed {
        None => {
            // Longest-first; tie-break on dir_key for determinism.
            tasks.sort_by(|a, b| b.weight.cmp(&a.weight).then(a.dir_key.cmp(&b.dir_key)));
        }
        Some(seed) => {
            // Deterministic Fisher–Yates shuffle of the task order.
            let mut state = seed ^ 0xD1B5_4A32_D192_ED03;
            for i in (1..tasks.len()).rev() {
                let j = (splitmix64(&mut state) % (i as u64 + 1)) as usize;
                tasks.swap(i, j);
            }
        }
    }
    tasks
}

// ---------------------------------------------------------------------------
// Execution.
// ---------------------------------------------------------------------------

pub(crate) struct CaseOutcome {
    pub(crate) order: usize,
    pub(crate) label: String,
    pub(crate) ok: bool,
    pub(crate) reason: String,
    /// Raw oracle result JSON (live cases only) — retained only when dumping,
    /// for the three-way contamination bit-diff.
    pub(crate) result: Option<Value>,
}

/// Shared, read-only execution context borrowed by every scheduler thread.
struct Ctx<'a> {
    serial: bool,
    dumping: bool,
    capi_pool: Option<&'a WorkerPool>,
    capi_oneshot: Option<&'a Oracle>,
    epri_pool: Option<&'a EpriPool>,
    epri_oneshot: Option<&'a EpriOneShot>,
    ledger: &'a LedgerRuntime,
}

impl<'a> Ctx<'a> {
    /// The transport for one (case, channel): the persistent pool normally, or a
    /// throwaway one-shot for `isolate`/serial (both channels honor the flag).
    fn channel(&self, uc: &UnifiedCase, ch: EngineChannel) -> Channel<'a> {
        let one_shot = uc.case.isolate || self.serial;
        match ch {
            EngineChannel::CapiV0145 => {
                if one_shot {
                    Channel::CapiOneShot(
                        self.capi_oneshot
                            .expect("capi one-shot pre-created for serial/isolate capi cases"),
                    )
                } else {
                    Channel::CapiPool(
                        self.capi_pool
                            .expect("capi pool pre-created for pooled capi cases"),
                    )
                }
            }
            EngineChannel::R4133 => {
                if one_shot {
                    Channel::EpriOneShot(
                        self.epri_oneshot
                            .expect("epri one-shot pre-created for serial/isolate r4133 cases"),
                    )
                } else {
                    Channel::EpriPool(
                        self.epri_pool
                            .expect("epri pool pre-created for pooled r4133 cases"),
                    )
                }
            }
        }
    }
}

/// Run + compare one case, fully `catch_unwind`-isolated → a `CaseOutcome`.
fn run_one_case(uc: &UnifiedCase, ctx: &Ctx) -> CaseOutcome {
    let _guard = CorpusGuard::new(&uc.abs);
    let ok_outcome = |result: Option<Value>| CaseOutcome {
        order: uc.order,
        label: uc.label.clone(),
        ok: true,
        reason: String::new(),
        result,
    };
    let fail_outcome = |reason: String, result: Option<Value>| CaseOutcome {
        order: uc.order,
        label: uc.label.clone(),
        ok: false,
        reason,
        result,
    };

    match uc.class {
        CaseClass::Pending => {
            // Pending is oracle-free (Rust must error loudly) — no channel.
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assert_pending_errors_loudly(&uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(None),
                Err(e) => fail_outcome(panic_msg(e), None),
            }
        }
        CaseClass::Deferred => {
            // Parked from live compare (Phase D ledger seed); Rust-smoke only.
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assert_deferred_rust_smoke(&uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(None),
                Err(e) => fail_outcome(panic_msg(e), None),
            }
        }
        CaseClass::Abort => {
            // Abort against every gating channel (Phase C: exactly one).
            for ch in uc.case.engine_channels() {
                // A ledger `skip` entry (a hard-crash deck on this channel) means
                // the channel is not sent at all; the other channel still gates.
                if ctx.ledger.channel_is_skipped(&uc.label, ch) {
                    continue;
                }
                let channel = ctx.channel(uc, ch);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run_and_compare_abort(&channel, &uc.label, &uc.abs, &uc.case);
                }));
                if let Err(e) = res {
                    return fail_outcome(format!("[{ch:?}] {}", panic_msg(e)), None);
                }
            }
            ok_outcome(None)
        }
        CaseClass::Live => {
            // Live-compare against every gating channel. The Rust engine is re-run
            // per channel inside `compare_with_result`; `both` (Phase D) thus runs
            // it twice — the seam is intentional. A ledger `skip` entry drops a
            // hard-crash channel; the ledger's field scopes partition the compare.
            let mut dump_val: Option<Value> = None;
            for ch in uc.case.engine_channels() {
                if ctx.ledger.channel_is_skipped(&uc.label, ch) {
                    continue;
                }
                let channel = ctx.channel(uc, ch);
                // R4133_PROPS RP4.1 (2026-09-03): the per-channel
                // `compare_all_properties = false` that used to stand here is
                // GONE — property parity is no longer capi_v0145-only, and the
                // r4133 request now carries the same all-properties capture the
                // capi one does ([`force_properties`], `dss-epri::capture`). The
                // clone is what the compare closure below owns; it is no longer
                // per-channel, so both channels see the same case.
                let cc = uc.case.clone();
                let req = build_run_request(&uc.abs, &cc);
                let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    channel.call(&req)
                })) {
                    Ok(r) => r,
                    Err(e) => {
                        return fail_outcome(
                            format!("[{ch:?}] oracle fetch panicked: {}", panic_msg(e)),
                            dump_val,
                        );
                    }
                };
                if !resp.ok {
                    return fail_outcome(
                        format!("[{ch:?}] oracle case failed: {:?}", resp.error),
                        dump_val,
                    );
                }
                let Some(val) = resp.result else {
                    return fail_outcome(
                        format!("[{ch:?}] oracle ok response missing result"),
                        dump_val,
                    );
                };
                if ctx.dumping && dump_val.is_none() {
                    dump_val = Some(val.clone());
                }
                let label = uc.label.clone();
                let abs = uc.abs.clone();
                let view = ctx.ledger.view(&uc.label, ch);
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let oc: CaseResult = serde_json::from_value(val)
                        .unwrap_or_else(|e| panic!("{label}: malformed CaseResult: {e}"));
                    compare_with_result(&oc, &label, &abs, &cc, ch, Some(&view));
                }));
                if let Err(e) = res {
                    return fail_outcome(format!("[{ch:?}] {}", panic_msg(e)), dump_val);
                }
            }
            ok_outcome(dump_val)
        }
    }
}

/// The result of one gate run: outcomes (manifest order) + wall-clock + config.
pub(crate) struct GateRun {
    pub(crate) outcomes: Vec<CaseOutcome>,
    pub(crate) elapsed: Duration,
    pub(crate) mode: String,
    pub(crate) jobs: usize,
    pub(crate) pool_size: usize,
    pub(crate) total: usize,
    /// The loaded ledger, kept for the `#[test]` to assert fail-on-stale and print
    /// per-entry hit accounting (§1.3 runtime rule).
    pub(crate) ledger: Arc<LedgerRuntime>,
}

/// Run the whole unified gate once (mode from env). Pure execution — the
/// `#[test]` wrapper does the asserting + reporting + optional dump.
pub(crate) fn run_gate() -> GateRun {
    let serial = std::env::var("DSS_GATE_SERIAL")
        .map(|v| v == "1")
        .unwrap_or(false);
    let shuffle_seed = std::env::var("DSS_GATE_SHUFFLE")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let dumping = std::env::var("DSS_GATE_DUMP").is_ok();
    let jobs = if serial {
        1
    } else {
        std::env::var("DSS_GATE_JOBS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or_else(|| {
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4)
            })
    };
    let pool_size = (jobs / 2).max(2);

    let start = Instant::now();
    let mut cases = build_unified_cases();
    // Optional case filter (`DSS_GATE_ONLY=<substr>[,<substr>...]`) for cheap
    // targeted verification under load — the FULL gate (no filter) is the one that
    // gates commits. When set, prints a loud banner so a filtered run is never
    // mistaken for the full gate.
    if let Ok(only) = std::env::var("DSS_GATE_ONLY") {
        let subs: Vec<String> = only
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let before = cases.len();
        cases.retain(|c| subs.iter().any(|s| c.label.contains(s)));
        eprintln!(
            "corpus_gate: DSS_GATE_ONLY filter kept {}/{before} case(s) — THIS IS A PARTIAL \
             RUN, not the commit gate",
            cases.len()
        );
        // A filter matching NOTHING must fail loudly, never green a 0/0 run (a
        // leftover exported env var would otherwise silently neuter the gate —
        // pre-E/F audit UGA-T5).
        assert!(
            !cases.is_empty(),
            "DSS_GATE_ONLY={only:?} matched no case labels — refusing to green a \
             zero-case gate run; fix or unset the filter"
        );
    }
    let total = cases.len();

    // Which transports does the case set actually need? A case can gate the
    // capi_v0145 channel, the r4133 channel, or (Phase D) both; `isolate`/serial
    // routes an engine run through a throwaway one-shot instead of the pool.
    // Only Live/Abort cases actually drive an oracle channel (Pending is
    // oracle-free; Deferred is Rust-smoke-only).
    let any = |ch: EngineChannel, isolate: bool| {
        cases.iter().any(|c| {
            matches!(c.class, CaseClass::Live | CaseClass::Abort)
                && c.case.engine_channels().contains(&ch)
                && c.case.isolate == isolate
        })
    };
    let has_capi = any(EngineChannel::CapiV0145, true) || any(EngineChannel::CapiV0145, false);
    let has_epri = any(EngineChannel::R4133, true) || any(EngineChannel::R4133, false);
    // In serial mode EVERY engine run uses a fresh one-shot process (the
    // contamination-proof reference); otherwise only `isolate` cases do.
    let needs_capi_oneshot = any(EngineChannel::CapiV0145, true) || (serial && has_capi);
    let needs_capi_pool = !serial && any(EngineChannel::CapiV0145, false);
    let needs_epri_oneshot = any(EngineChannel::R4133, true) || (serial && has_epri);
    let needs_epri_pool = !serial && any(EngineChannel::R4133, false);

    // `for_spec(None)` constructs the pinned one-shot oracle AND ping-verifies it;
    // the pools/one-shots ping-verify their engine identity on spawn.
    let capi_oneshot: Option<Oracle> = needs_capi_oneshot.then(|| Oracle::for_spec(None));
    let capi_pool: Option<WorkerPool> = needs_capi_pool.then(|| WorkerPool::new(pool_size));
    let epri_oneshot: Option<EpriOneShot> = needs_epri_oneshot.then(EpriOneShot::new);
    let epri_pool: Option<EpriPool> = needs_epri_pool.then(|| EpriPool::new(pool_size));

    let ledger = Arc::new(LedgerRuntime::load());
    // GateRun keeps its own handle to the same runtime (shared hit counters); the
    // `ctx` borrow of `ledger` lives until the end of the function, so move the
    // clone (not the borrowed original) into the result.
    let ledger_result = Arc::clone(&ledger);
    let tasks = build_tasks(cases, shuffle_seed);
    let ctx = Ctx {
        serial,
        dumping,
        capi_pool: capi_pool.as_ref(),
        capi_oneshot: capi_oneshot.as_ref(),
        epri_pool: epri_pool.as_ref(),
        epri_oneshot: epri_oneshot.as_ref(),
        ledger: &ledger,
    };

    let cursor = AtomicUsize::new(0);
    let results: Mutex<Vec<CaseOutcome>> = Mutex::new(Vec::with_capacity(total));

    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let idx = cursor.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let task = &tasks[idx];
                    let mut local: Vec<CaseOutcome> = Vec::with_capacity(task.cases.len());
                    for uc in &task.cases {
                        local.push(run_one_case(uc, &ctx));
                    }
                    results.lock().unwrap().extend(local);
                }
            });
        }
    });

    if let Some(p) = &capi_pool {
        p.close();
    }
    if let Some(p) = &epri_pool {
        p.close();
    }

    let mut outcomes = results.into_inner().unwrap();
    outcomes.sort_by_key(|o| o.order);
    let elapsed = start.elapsed();

    let mode = if serial {
        "serial-oneshot".to_string()
    } else if let Some(seed) = shuffle_seed {
        format!("persistent-parallel-shuffled(seed={seed})")
    } else {
        "persistent-parallel".to_string()
    };

    GateRun {
        outcomes,
        elapsed,
        mode,
        jobs,
        pool_size,
        total,
        ledger: ledger_result,
    }
}

// ---------------------------------------------------------------------------
// Seeding report mode (§4 Phase D step 2 — DSS_GATE_SEED_LEDGER=1).
// ---------------------------------------------------------------------------

/// One seeding measurement: how a case compares against ONE channel, with NO
/// ledger applied (raw divergence), so triage sees where each case would need a
/// ledger entry to gate `both`.
#[derive(serde::Serialize)]
struct SeedRecord {
    case: String,
    channel: String,
    status: String,
    reason: String,
}

/// Run every live case against BOTH channels regardless of its `engines`,
/// measuring the raw (ledger-free) divergence, and write candidate triage data to
/// `tmp/ledger_candidates.json`. Never asserts — a report tool.
pub(crate) fn seed_ledger() {
    let jobs = std::env::var("DSS_GATE_JOBS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });
    let pool_size = (jobs / 2).max(2);
    let start = Instant::now();

    // Optional case filter (`DSS_GATE_SEED_ONLY=<substr>[,<substr>...]`) — measure
    // only matching labels, for cheap targeted re-measurement under load.
    let only: Vec<String> = std::env::var("DSS_GATE_SEED_ONLY")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let cases = build_unified_cases();
    // Seed only Live cases (Pending is oracle-free; Abort has its own contract) —
    // but a Deferred case IS a primary seed, so include it as Live.
    let seedable: Vec<UnifiedCase> = cases
        .into_iter()
        .filter(|c| matches!(c.class, CaseClass::Live | CaseClass::Deferred))
        .filter(|c| only.is_empty() || only.iter().any(|s| c.label.contains(s)))
        .collect();
    let total = seedable.len();

    let capi_pool = WorkerPool::new(pool_size);
    let epri_pool = EpriPool::new(pool_size);
    let capi_oneshot = Oracle::for_spec(None);
    let epri_oneshot = EpriOneShot::new();
    let ctx = Ctx {
        serial: false,
        dumping: false,
        capi_pool: Some(&capi_pool),
        capi_oneshot: Some(&capi_oneshot),
        epri_pool: Some(&epri_pool),
        epri_oneshot: Some(&epri_oneshot),
        // A throwaway empty ledger — seeding measures the RAW divergence.
        ledger: EMPTY_LEDGER_FOR_SEEDING.get_or_init(LedgerRuntime::empty),
    };

    // Group by dir like the main gate (avoid same-dir contention).
    let tasks = build_tasks(seedable, None);
    let cursor = AtomicUsize::new(0);
    let records: Mutex<Vec<SeedRecord>> = Mutex::new(Vec::with_capacity(2 * total));

    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let idx = cursor.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let mut local = Vec::new();
                    for uc in &tasks[idx].cases {
                        for ch in [EngineChannel::CapiV0145, EngineChannel::R4133] {
                            local.push(seed_one(uc, ch, &ctx));
                        }
                    }
                    records.lock().unwrap().extend(local);
                }
            });
        }
    });

    capi_pool.close();
    epri_pool.close();

    let mut records = records.into_inner().unwrap();
    records.sort_by(|a, b| a.case.cmp(&b.case).then(a.channel.cmp(&b.channel)));
    let matched = records.iter().filter(|r| r.status == "match").count();
    let diverged = records.iter().filter(|r| r.status == "diverge").count();
    let errored = records.iter().filter(|r| r.status == "error").count();

    let out: std::path::PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<std::path::PathBuf>()
        .join("ledger_candidates.json");
    let _ = std::fs::create_dir_all(out.parent().unwrap());
    let json = serde_json::json!({
        "total_cases": total,
        "measurements": records.len(),
        "match": matched,
        "diverge": diverged,
        "error": errored,
        "records": records,
    });
    std::fs::write(&out, serde_json::to_string_pretty(&json).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
    eprintln!(
        "seed_ledger: {total} case(s) x2 channels = {} measurement(s): {matched} match, \
         {diverged} diverge, {errored} error in {:.1}s -> {}",
        records.len(),
        start.elapsed().as_secs_f64(),
        out.display()
    );
}

/// The throwaway empty ledger the two REPORT modes share (seeding and the
/// property census): both measure the RAW divergence, with no entry applied.
static EMPTY_LEDGER_FOR_SEEDING: std::sync::OnceLock<LedgerRuntime> = std::sync::OnceLock::new();

/// Measure one (case, channel) with no ledger; catch every panic into a status.
fn seed_one(uc: &UnifiedCase, ch: EngineChannel, ctx: &Ctx) -> SeedRecord {
    let channel_name = match ch {
        EngineChannel::CapiV0145 => "capi_v0145",
        EngineChannel::R4133 => "r4133",
    };
    let mk = |status: &str, reason: String| SeedRecord {
        case: uc.label.clone(),
        channel: channel_name.to_string(),
        status: status.to_string(),
        reason: reason.chars().take(300).collect(),
    };
    let _guard = CorpusGuard::new(&uc.abs);
    let channel = ctx.channel(uc, ch);
    // R4133_PROPS RP4.1 (2026-09-03): the seeding path's per-channel
    // `compare_all_properties = false` went with the gate path's — a seeding
    // measurement must see exactly what the gate compares, on both channels. The
    // clone is what the compare closure below owns.
    let cc = uc.case.clone();
    let req = build_run_request(&uc.abs, &cc);
    let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| channel.call(&req))) {
        Ok(r) => r,
        Err(e) => return mk("error", format!("fetch panic: {}", panic_msg(e))),
    };
    if !resp.ok {
        return mk("error", format!("oracle: {:?}", resp.error));
    }
    let Some(val) = resp.result else {
        return mk("error", "ok response missing result".to_string());
    };
    let label = uc.label.clone();
    let abs = uc.abs.clone();
    let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let oc: CaseResult =
            serde_json::from_value(val).unwrap_or_else(|e| panic!("malformed CaseResult: {e}"));
        compare_with_result(&oc, &label, &abs, &cc, ch, None);
    }));
    match res {
        Ok(()) => mk("match", String::new()),
        Err(e) => mk("diverge", panic_msg(e)),
    }
}

// ---------------------------------------------------------------------------
// Property census mode (`R4133_PROPS_PLAN.md` RP0.2 — DSS_PROPS_CENSUS).
// ---------------------------------------------------------------------------

/// Walk every live case on BOTH channels with `all_properties` forced on — which
/// until R4133_PROPS RP4.1 (2026-09-03) meant bypassing the §1.1 gate masks that
/// kept property parity capi-only, and since RP4.1 (which deleted those masks)
/// means only ignoring the case's own `engines` key — and
/// COLLECT every divergent cell instead of asserting. Writes
/// `tmp/props_census.json` plus the per-channel RP0.1 extracts
/// ([`crate::props_census`]); **asserts nothing about the data**.
///
/// This is the permanent successor of the scratch test that produced the
/// 2026-08-08 census vendored at `tests/corpus/props_r4133/`. It reproduces that
/// walk's population: live cases only (`pending`/`abort`/`defer_ledger` have
/// their own contracts and no property capture), `large`-kind decks excluded
/// (they are excluded from property forcing on the gate too,
/// [`force_properties`], and the vendored census carries none), the case's own
/// `engines` key ignored — a capi-only deck is still measured against r4133,
/// which is where the census's GenDispatcher and Sensor shape rows come from.
///
/// The normal gate path is untouched: nothing here runs unless the env var is
/// set. (The per-channel masks this doc used to point at, in [`run_one_case`]
/// and [`seed_one`], were removed by RP4.1; what still separates this walk from
/// the gate is that it ignores `engines` and asserts nothing.)
pub(crate) fn run_props_census(raw_mode: &str) {
    let mode = CensusMode::from_env(raw_mode);
    let jobs = std::env::var("DSS_GATE_JOBS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });
    let pool_size = (jobs / 2).max(2);
    let start = Instant::now();

    let mut cases: Vec<UnifiedCase> = build_unified_cases()
        .into_iter()
        .filter(|c| c.class == CaseClass::Live && !c.case.kind.starts_with("large"))
        .collect();
    // Same filter (and same loud empty-match refusal) as the gate: a census that
    // silently measured zero cases would look exactly like a clean one. The
    // filter travels into the artifacts (`gate_only`), so a bounded run cannot
    // be mistaken for a full one later.
    let gate_only = std::env::var("DSS_GATE_ONLY").ok();
    if let Some(only) = gate_only.clone() {
        let subs: Vec<String> = only
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let before = cases.len();
        cases.retain(|c| subs.iter().any(|s| c.label.contains(s)));
        eprintln!(
            "props_census: DSS_GATE_ONLY filter kept {}/{before} case(s)",
            cases.len()
        );
        assert!(
            !cases.is_empty(),
            "DSS_GATE_ONLY={only:?} matched no case labels — refusing to write an empty census"
        );
    }
    let total = cases.len();

    let capi_pool = WorkerPool::new(pool_size);
    let epri_pool = EpriPool::new(pool_size);
    let capi_oneshot = Oracle::for_spec(None);
    let epri_oneshot = EpriOneShot::new();
    // The claims mode's LAST chain link (plan §1.1(e)): a `property`-scoped
    // ledger entry NAMES a cell. It is loaded SEPARATELY from `Ctx.ledger`,
    // which stays empty on purpose — the walk must keep measuring the raw
    // divergence (RP0.2's baseline, and a `skip` entry must not hide a case),
    // while the disposition query answers "would an entry claim this cell at
    // RP4.1?". Only the non-asserting `property_scope_keys` is consulted, so a
    // census can never fail on a pin, and this runtime's hit counters are its
    // own (the gate's `assert_all_hit` reads a different instance).
    let claims_ledger = mode.annotates().then(LedgerRuntime::load);
    let ctx = Ctx {
        serial: false,
        dumping: false,
        capi_pool: Some(&capi_pool),
        capi_oneshot: Some(&capi_oneshot),
        epri_pool: Some(&epri_pool),
        epri_oneshot: Some(&epri_oneshot),
        // The census measures the RAW divergence — no ledger entry applies, and
        // no channel `skip` entry hides a case (the vendored census records the
        // #303 crash decks as `oracle_error` rows for exactly that reason).
        ledger: EMPTY_LEDGER_FOR_SEEDING.get_or_init(LedgerRuntime::empty),
    };

    let tasks = build_tasks(cases, None);
    let cursor = AtomicUsize::new(0);
    #[allow(clippy::type_complexity)]
    let collected: Mutex<(Vec<CensusRow>, BTreeMap<&'static str, CensusBlindSpots>)> =
        Mutex::new((Vec::new(), BTreeMap::new()));

    std::thread::scope(|s| {
        for _ in 0..jobs {
            s.spawn(|| {
                loop {
                    let idx = cursor.fetch_add(1, Ordering::Relaxed);
                    if idx >= tasks.len() {
                        break;
                    }
                    let mut rows = Vec::new();
                    let mut blind: BTreeMap<&'static str, CensusBlindSpots> = BTreeMap::new();
                    for uc in &tasks[idx].cases {
                        for ch in [EngineChannel::CapiV0145, EngineChannel::R4133] {
                            let (r, b) = census_one(uc, ch, &ctx, mode, claims_ledger.as_ref());
                            rows.extend(r);
                            blind.entry(channel_tag(ch)).or_default().add(b);
                        }
                    }
                    let mut guard = collected.lock().unwrap();
                    guard.0.extend(rows);
                    for (k, v) in blind {
                        guard.1.entry(k).or_default().add(v);
                    }
                }
            });
        }
    });

    capi_pool.close();
    epri_pool.close();

    let (rows, blind) = collected.into_inner().unwrap();
    crate::props_census::write_artifacts(rows, mode, total, &blind, gate_only.as_deref());
    eprintln!(
        "props_census: {total} case(s) x2 channels in {:.1}s",
        start.elapsed().as_secs_f64()
    );
}

/// The census's spelling of a gate channel.
fn channel_tag(ch: EngineChannel) -> &'static str {
    ch.props_channel().tag()
}

/// Measure one (case, channel) for the census: force the property capture,
/// re-run the Rust engine step by step and collect every divergent cell. Every
/// failure mode — a dead oracle, a crashing deck, a panicking Rust walk —
/// becomes a ROW, never a panic. The second return value is what the walk could
/// not look at (see [`harness::CensusBlindSpots`]).
///
/// `mode` and `claims_ledger` are the disposition mode's two extras (RP2.1): in
/// [`CensusMode::Claims`] every value row is annotated with the r4133 policy
/// chain's verdict for that cell, the ledger link reading this case's
/// `property` scopes. The WALK is identical in both modes — see
/// [`crate::props_census::Disposition`] for why annotating the raw population is
/// the same statement as re-walking with the armed policy.
fn census_one(
    uc: &UnifiedCase,
    ch: EngineChannel,
    ctx: &Ctx,
    mode: CensusMode,
    claims_ledger: Option<&LedgerRuntime>,
) -> (Vec<CensusRow>, CensusBlindSpots) {
    let pch = ch.props_channel();
    // The vendored README's §"The in-scope filter": a case the RP4.1 unmask will
    // actually compare on r4133 is one whose manifest entry gates that channel.
    // Read off the case's own `engines` key, exactly as the filter defines it.
    let in_scope = uc.case.engines != "capi_v0145";
    let oracle_error = |detail: String| {
        (
            vec![CensusRow::error(
                &uc.label,
                pch,
                in_scope,
                CensusRowKind::OracleError { detail },
            )],
            CensusBlindSpots::default(),
        )
    };

    let _guard = CorpusGuard::new(&uc.abs);
    let channel = ctx.channel(uc, ch);
    let mut cc = uc.case.clone();
    // The knob's whole point: the property capture is requested on BOTH channels,
    // regardless of the plan §1.1 masks the gate and the seeding path apply.
    cc.compare_all_properties = true;
    let req = build_run_request(&uc.abs, &cc);
    let resp = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| channel.call(&req))) {
        Ok(r) => r,
        Err(e) => return oracle_error(format!("fetch panic: {}", panic_msg(e))),
    };
    if !resp.ok {
        return oracle_error(format!("{:?}", resp.error));
    }
    let Some(val) = resp.result else {
        return oracle_error("ok response missing result".to_string());
    };

    // Deserialization happens OUTSIDE the Rust-side `catch_unwind` on purpose: a
    // channel that answers `ok: true` with a payload the harness cannot parse is
    // an ORACLE defect, and in a mode whose only diagnostic is the row kind it
    // must not be filed against the Rust engine (RP0.2 audit).
    let oc: CaseResult = match serde_json::from_value(val) {
        Ok(oc) => oc,
        Err(e) => return oracle_error(format!("malformed CaseResult: {e}")),
    };

    let label = uc.label.clone();
    let abs = uc.abs.clone();
    // The ledger link, resolved once per (case, channel): the `property` scopes
    // that NAME a cell here. Empty in plain mode and whenever no entry applies.
    let ledger_view = claims_ledger.map(|rt| rt.view(&uc.label, ch));
    let walked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let tol = harness::tol_for(&cc.kind);
        let (mut dss, _baseline) = run_rust_capture(&label, &abs, &cc);
        let mut rows: Vec<CensusRow> = Vec::new();
        let mut blind = CensusBlindSpots::default();
        for (i, cp) in oc.checkpoints.iter().enumerate() {
            dss.command("solve");
            // A capture that came back empty means the request was not honored —
            // record it instead of reporting a spuriously clean step.
            if cp.all_properties.is_empty() {
                rows.push(CensusRow::error(
                    &label,
                    pch,
                    in_scope,
                    CensusRowKind::OracleError {
                        detail: format!("step {i}: empty all_properties capture"),
                    },
                ));
                continue;
            }
            let mut found = Vec::new();
            blind.add(harness::collect_prop_divergences(
                &mut dss,
                &cp.all_properties,
                &tol,
                pch,
                &mut found,
            ));
            let ledger_named = match &ledger_view {
                Some(v) => v.property_scope_keys(&cp.all_properties),
                None => BTreeSet::new(),
            };
            rows.extend(found.into_iter().map(|f| {
                let mut row = CensusRow::from_harness(&label, pch, in_scope, i, f);
                if mode.annotates() {
                    row.annotate(&ledger_named);
                }
                row
            }));
        }
        (rows, blind)
    }));
    match walked {
        Ok(out) => out,
        Err(e) => (
            vec![CensusRow::error(
                &uc.label,
                pch,
                in_scope,
                CensusRowKind::RustError {
                    detail: panic_msg(e),
                },
            )],
            CensusBlindSpots::default(),
        ),
    }
}
