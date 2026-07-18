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

use crate::engines::{CaseResult, Channel, Oracle, WorkerPool, build_run_request};
use crate::manifest::{
    FAMILIES, ORACLE_SPECS, SolvableCase, corpus_file, family_file, load_family, load_solvable,
};
use crate::runner::{
    CorpusGuard, assert_pending_errors_loudly, compare_with_result, panic_msg,
    run_and_compare_abort,
};

// ---------------------------------------------------------------------------
// Unified case model.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum CaseClass {
    Live,
    Abort,
    Pending,
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

/// Apply the (unchanged) per-source property-forcing rule to a live case.
fn force_properties(source: &str, c: &mut SolvableCase, fam_props: bool) {
    match source {
        "solvable_now" => {
            if c.oracle.is_none() && !c.kind.starts_with("large") {
                c.compare_all_properties = true;
            }
        }
        _ => {
            // family: fam_props is the family-level flag (true for all three).
            c.compare_all_properties |= fam_props && c.oracle.is_none();
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
    let class = if c.pending {
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
    pool: Option<&'a WorkerPool>,
    pinned_oneshot: Option<&'a Oracle>,
    target_oracles: &'a BTreeMap<String, Oracle>,
}

impl<'a> Ctx<'a> {
    /// The channel a case's `oracle` spec + `isolate`/serial state selects.
    fn channel(&self, uc: &UnifiedCase) -> Channel<'a> {
        if let Some(spec) = &uc.case.oracle {
            Channel::OneShot(
                self.target_oracles
                    .get(spec)
                    .expect("target-rev oracle pre-created for every spec in the manifests"),
            )
        } else if uc.case.isolate || self.serial {
            Channel::OneShot(
                self.pinned_oneshot
                    .expect("pinned one-shot oracle pre-created for serial/isolate cases"),
            )
        } else {
            Channel::Pool(
                self.pool
                    .expect("worker pool pre-created for pinned pool cases"),
            )
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
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                assert_pending_errors_loudly(&uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(None),
                Err(e) => fail_outcome(panic_msg(e), None),
            }
        }
        CaseClass::Abort => {
            let channel = ctx.channel(uc);
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                run_and_compare_abort(&channel, &uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(None),
                Err(e) => fail_outcome(panic_msg(e), None),
            }
        }
        CaseClass::Live => {
            let channel = ctx.channel(uc);
            let req = build_run_request(&uc.abs, &uc.case);
            // Fetch the oracle model (one-shot Oracle::call may panic on a
            // protocol error — isolate it into a case failure, never a thread crash).
            let resp =
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| channel.call(&req)))
                {
                    Ok(r) => r,
                    Err(e) => {
                        return fail_outcome(
                            format!("oracle fetch panicked: {}", panic_msg(e)),
                            None,
                        );
                    }
                };
            if !resp.ok {
                return fail_outcome(format!("oracle case failed: {:?}", resp.error), None);
            }
            let Some(val) = resp.result else {
                return fail_outcome("oracle ok response missing result".to_string(), None);
            };
            let dump_val = if ctx.dumping { Some(val.clone()) } else { None };
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let oc: CaseResult = serde_json::from_value(val).unwrap_or_else(|e| {
                    panic!("{}: malformed CaseResult: {e}", uc.label);
                });
                compare_with_result(&oc, &uc.label, &uc.abs, &uc.case);
            }));
            match res {
                Ok(()) => ok_outcome(dump_val),
                Err(e) => fail_outcome(panic_msg(e), dump_val),
            }
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

    // Which specs / one-shot engines do we actually need?
    let needs_pinned_oneshot = serial
        || cases
            .iter()
            .any(|c| c.case.oracle.is_none() && c.case.isolate);
    let needs_pinned_pool = !serial
        && cases
            .iter()
            .any(|c| c.case.oracle.is_none() && !c.case.isolate);
    let target_specs: std::collections::BTreeSet<String> =
        cases.iter().filter_map(|c| c.case.oracle.clone()).collect();

    // Pre-create + ping-verify every one-shot engine up front (shared by ref).
    let mut target_oracles: BTreeMap<String, Oracle> = BTreeMap::new();
    for spec in &target_specs {
        assert!(
            ORACLE_SPECS.contains(&spec.as_str()),
            "unknown oracle spec {spec:?} — expected one of {ORACLE_SPECS:?}"
        );
        target_oracles.insert(spec.clone(), Oracle::for_spec(Some(spec)));
    }
    // `for_spec(None)` constructs the pinned one-shot oracle AND ping-verifies it.
    let pinned_oneshot: Option<Oracle> = needs_pinned_oneshot.then(|| Oracle::for_spec(None));

    let pool: Option<WorkerPool> = needs_pinned_pool.then(|| WorkerPool::new(pool_size));

    let tasks = build_tasks(cases, shuffle_seed);
    let ctx = Ctx {
        serial,
        dumping,
        pool: pool.as_ref(),
        pinned_oneshot: pinned_oneshot.as_ref(),
        target_oracles: &target_oracles,
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

    if let Some(p) = &pool {
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
