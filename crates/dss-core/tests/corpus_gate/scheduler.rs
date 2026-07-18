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
//! The pinned `capi_v0145` channel is served by a persistent [`WorkerPool`];
//! target-rev cases (`capi015`/`r3723`/`r4088`/`r4133`) keep the ONE-SHOT
//! [`Oracle`] path. Each case is `catch_unwind`-wrapped; the test fails iff any
//! case failed, printing the COMPLETE failure list in manifest order.
//!
//! Contamination-proof knobs (§4 Phase B DONE bar):
//! * `DSS_GATE_SERIAL=1` — T=1 and the pinned channel uses a FRESH one-shot
//!   process per case (no persistent-worker state reuse): the reference run.
//! * `DSS_GATE_SHUFFLE=<seed>` — shuffle the task order (order-sensitivity probe).
//! * `DSS_GATE_DUMP=<path>` — write a label-sorted `{label: {verdict, result}}`
//!   artifact for a three-way bit-diff.
//! * `DSS_GATE_JOBS=<n>` — override the scheduler thread count.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::engines::{
    CaseResult, Channel, EpriOneShot, EpriPool, Oracle, WorkerPool, build_run_request,
};
use crate::manifest::{
    EngineChannel, FAMILIES, SolvableCase, corpus_file, family_file, load_family, load_solvable,
};
use crate::runner::{
    CorpusGuard, assert_deferred_rust_smoke, assert_pending_errors_loudly, compare_with_result,
    panic_msg, run_and_compare_abort,
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

/// Apply the per-source property-forcing rule to a live case. `compare_all_
/// properties` is a **capi_v0145-channel** feature (§1.2: property parity stays
/// pinned to the capi oracle; the r4133 bridge has no all-properties capture),
/// so it is forced only for cases that gate the capi channel; the r4133 request
/// masks it off per-channel in [`run_one_case`].
fn force_properties(source: &str, c: &mut SolvableCase, fam_props: bool) {
    match source {
        "solvable_now" => {
            if c.gates_capi() && !c.kind.starts_with("large") {
                c.compare_all_properties = true;
            }
        }
        _ => {
            // family: fam_props is the family-level flag (true for all three).
            c.compare_all_properties |= fam_props && c.gates_capi();
        }
    }
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
            // Live-compare against every gating channel (Phase C: exactly one).
            // The Rust engine is re-run per channel inside `compare_with_result`;
            // `both` (Phase D) thus runs it twice — the seam is intentional.
            let mut dump_val: Option<Value> = None;
            for ch in uc.case.engine_channels() {
                let channel = ctx.channel(uc, ch);
                // The r4133 bridge has no all-properties capture (§1.2 keeps it
                // capi_v0145-only) — mask it off in both the request and compare.
                let mut cc = uc.case.clone();
                if ch == EngineChannel::R4133 {
                    cc.compare_all_properties = false;
                }
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
                let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                    let oc: CaseResult = serde_json::from_value(val)
                        .unwrap_or_else(|e| panic!("{label}: malformed CaseResult: {e}"));
                    compare_with_result(&oc, &label, &abs, &cc, ch);
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
    let cases = build_unified_cases();
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

    let tasks = build_tasks(cases, shuffle_seed);
    let ctx = Ctx {
        serial,
        dumping,
        capi_pool: capi_pool.as_ref(),
        capi_oneshot: capi_oneshot.as_ref(),
        epri_pool: epri_pool.as_ref(),
        epri_oneshot: epri_oneshot.as_ref(),
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
    }
}
