//! Case runner: the Rust engine runs ONCE per case ([`run_rust_capture`]), then
//! its per-step state is compared against a channel's [`CaseResult`]
//! ([`compare_capture`]). The comparator call order and every `harness`
//! comparator are byte-identical to the pre-Phase-B `run_and_compare` — the only
//! structural change is that the oracle `CaseResult` is now fetched by the
//! scheduler (via a persistent [`Channel`]) and passed in, instead of being
//! spawned inside.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use dss_core::exec::Dss;
use serde_json::json;

use crate::engines::{CaseResult, Channel, Oracle};
use crate::harness::{
    self, ExportPolicy, RowPolicy, Tolerances, compare_all_properties, compare_ctrlqueue,
    compare_discrete, compare_element, compare_eventlog, compare_export, compare_fingerprint,
    compare_injection, compare_meter, compare_monitor, compare_probe, compare_system_y,
    compare_variables, compare_yprim, tol_for,
};
use crate::manifest::{EngineChannel, SolvableCase};

// ---------------------------------------------------------------------------
// Corpus guard (unchanged; keeps the vendored corpus pristine across both
// engines' report/trace writes). RAII: created before the runs, restores on drop.
// ---------------------------------------------------------------------------

/// Buffer small files up to this size for overwrite-restore. Mirrors the oracle
/// server's `_RESTORE_MAX`.
const RESTORE_MAX: u64 = 2 * 1024 * 1024;

pub(crate) struct CorpusGuard {
    dir: PathBuf,
    names: BTreeSet<String>,
    buf: BTreeMap<String, Vec<u8>>,
    snapshot_ok: bool,
}

impl CorpusGuard {
    fn snapshot(
        dir: &Path,
        prefix: &str,
        names: &mut BTreeSet<String>,
        buf: &mut BTreeMap<String, Vec<u8>>,
    ) -> bool {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return false;
        };
        let mut ok = true;
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            names.insert(rel.clone());
            if is_dir {
                ok &= Self::snapshot(&entry.path(), &rel, names, buf);
                continue;
            }
            let small = entry
                .metadata()
                .map(|m| m.is_file() && m.len() <= RESTORE_MAX)
                .unwrap_or(false);
            if small && let Ok(data) = std::fs::read(entry.path()) {
                buf.insert(rel, data);
            }
        }
        ok
    }

    pub(crate) fn new(case_path: &str) -> Self {
        let dir = Path::new(case_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut names = BTreeSet::new();
        let mut buf = BTreeMap::new();
        let snapshot_ok = Self::snapshot(&dir, "", &mut names, &mut buf);
        Self {
            dir,
            names,
            buf,
            snapshot_ok,
        }
    }

    fn sweep_created(&self, dir: &Path, prefix: &str) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if self.names.contains(&rel) {
                if is_dir {
                    self.sweep_created(&entry.path(), &rel);
                }
                continue;
            }
            if is_dir {
                let _ = std::fs::remove_dir_all(entry.path());
            } else {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

impl Drop for CorpusGuard {
    fn drop(&mut self) {
        if !self.snapshot_ok {
            return;
        }
        self.sweep_created(&self.dir.clone(), "");
        for (name, data) in &self.buf {
            let p = self.dir.join(name);
            match std::fs::read(&p) {
                Ok(cur) if cur == *data => {}
                _ => {
                    let _ = std::fs::write(&p, data);
                }
            }
        }
    }
}

#[test]
fn corpus_guard_restores_case_dir_recursively() {
    let root = std::env::temp_dir().join(format!("dss_guard_test_{}", std::process::id()));
    std::fs::remove_dir_all(&root).ok();
    let sub = root.join("support");
    std::fs::create_dir_all(&sub).unwrap();
    let case = root.join("case.dss");
    std::fs::write(&case, b"! fixture master").unwrap();
    let fixture = sub.join("fixture.txt");
    std::fs::write(&fixture, b"vendored bytes").unwrap();
    {
        let _guard = CorpusGuard::new(&case.to_string_lossy());
        std::fs::write(root.join("run_created.csv"), b"pollution").unwrap();
        std::fs::write(sub.join("run_created_inner.csv"), b"pollution").unwrap();
        let di = root.join("ckt_di").join("DI_yr_1");
        std::fs::create_dir_all(&di).unwrap();
        std::fs::write(di.join("x.csv"), b"pollution").unwrap();
        std::fs::write(&fixture, b"overwritten by the run").unwrap();
    }
    assert!(case.is_file(), "vendored master must survive");
    assert_eq!(
        std::fs::read(&fixture).unwrap(),
        b"vendored bytes",
        "overwritten pre-existing fixture must be restored"
    );
    assert!(!root.join("run_created.csv").exists());
    assert!(
        !sub.join("run_created_inner.csv").exists(),
        "run-created file inside a pre-existing subdir must be swept (recursion)"
    );
    assert!(
        !root.join("ckt_di").exists(),
        "run-created dir tree removed"
    );
    std::fs::remove_dir_all(&root).ok();
}

// ---------------------------------------------------------------------------
// Warnings reconciliation (unchanged).
// ---------------------------------------------------------------------------

fn assert_expected_warnings(dss: &Dss, expect: &[String], ctx: &str) {
    let errors = dss.errors();
    if expect.is_empty() {
        assert!(
            errors.is_empty(),
            "{ctx}: unexpected Rust engine errors: {errors:?}"
        );
        return;
    }
    let unexpected: Vec<&String> = errors
        .iter()
        .filter(|e| !expect.iter().any(|w| e.contains(w.as_str())))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{ctx}: Rust engine errors not covered by expect_warnings: {unexpected:?}"
    );
    for w in expect {
        assert!(
            errors.iter().any(|e| e.contains(w.as_str())),
            "{ctx}: expected warning {w:?} never fired (actual: {errors:?})"
        );
    }
}

// ---------------------------------------------------------------------------
// run_rust_capture + compare_capture (the split of the old run_and_compare).
// ---------------------------------------------------------------------------

/// Compile + post + reconcile warnings; return the driven [`Dss`] (not yet
/// solved) and the baseline error count. The Rust engine runs ONCE per case;
/// [`compare_capture`] then advances + compares it step by step.
pub(crate) fn run_rust_capture(label: &str, case_path: &str, c: &SolvableCase) -> (Dss, usize) {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert_expected_warnings(&dss, &c.expect_warnings, &format!("{label}: after compile"));
    let baseline_errors = dss.errors().len();
    (dss, baseline_errors)
}

/// Compare the (once-run) Rust engine against ONE channel's `CaseResult`, per
/// step, in the fixed comparator order. Byte-identical to the pre-Phase-B loop
/// EXCEPT where a per-case-per-channel `ledger` scope partitions a field: the
/// untouched `harness` comparator runs on the unscoped remainder while the
/// ledger's envelope/exact-pair assert covers the scoped part and records the hit
/// (§1.3). With `ledger = None` or an empty view every field takes the original
/// path unchanged.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_capture(
    dss: &mut Dss,
    baseline_errors: usize,
    oc: &CaseResult,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
    tol: &Tolerances,
    channel: EngineChannel,
    ledger: Option<&crate::ledger::LedgerView>,
) {
    let n_steps = c.n_steps;
    let star = c.selected_elements == ["*"];
    // A view with no applicable entries behaves exactly like `None` (fast path).
    let ledger = ledger.filter(|v| !v.is_empty());

    for (i, cp) in oc.checkpoints.iter().enumerate() {
        dss.command("solve");
        assert_eq!(
            dss.errors().len(),
            baseline_errors,
            "{label} step {i}: new Rust engine errors: {:?}",
            &dss.errors()[baseline_errors.min(dss.errors().len())..]
        );
        let rust_global_result = dss.result().to_string();
        let ctx = format!("{label} step {i}");

        {
            let ckt = dss.circuit().expect("circuit exists");
            assert!(
                cp.converged,
                "{ctx}: oracle did not converge (persisted across the oracle's in-process retries)"
            );
            assert!(ckt.is_solved, "{ctx}: Rust did not converge");
            assert!(
                (ckt.solution.dbl_hour - cp.dbl_hour).abs() < 1e-9,
                "{ctx}: dblHour {} vs {}",
                ckt.solution.dbl_hour,
                cp.dbl_hour
            );
            // Iteration policy (§4 Phase C): the pinned capi_v0145 channel is an
            // exact 1:1 contract; the r4133 channel (a different engine line)
            // allows the port to converge in FEWER iterations — never more. A
            // ledger `iterations` scope overrides both (exact pair, or an explicit
            // rust_le_oracle where a target-rev delta needs pinning).
            let iters_ledgered = ledger.is_some_and(|v| {
                v.iterations_handled(i, ckt.solution.iteration, cp.iterations, &ctx)
            });
            if !iters_ledgered {
                if channel.iterations_exact() {
                    assert_eq!(
                        ckt.solution.iteration, cp.iterations,
                        "{ctx}: iteration count differs"
                    );
                } else {
                    assert!(
                        ckt.solution.iteration <= cp.iterations,
                        "{ctx}: Rust used MORE iterations than the r4133 oracle ({} > {})",
                        ckt.solution.iteration,
                        cp.iterations
                    );
                    if ckt.solution.iteration < cp.iterations {
                        eprintln!(
                            "{ctx}: NOTE Rust converged in {} iterations vs the r4133 \
                             oracle's {} (allowed: <=; investigate if unexpected)",
                            ckt.solution.iteration, cp.iterations
                        );
                    }
                }
            }
            let names: Vec<String> = (1..=ckt.num_nodes).map(|j| ckt.node_name(j)).collect();
            assert_eq!(names, oc.node_order, "{ctx}: node order differs");

            let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
            for j in 1..=ckt.num_nodes {
                actual.push(ckt.solution.node_v[j].re);
                actual.push(ckt.solution.node_v[j].im);
            }
            let mut expected = Vec::with_capacity(actual.len());
            for (re, im) in cp.v_re.iter().zip(&cp.v_im) {
                expected.push(*re);
                expected.push(*im);
            }
            // A ledger voltage scope partitions the nodes: the scoped nodes are
            // envelope-checked inside `voltage_keep_mask` (hit recorded), the
            // unscoped remainder still meets the tier floor here.
            match ledger
                .and_then(|v| v.voltage_keep_mask(i, &oc.node_order, &actual, &expected, tol, &ctx))
            {
                Some(keep) => {
                    let mut ra = Vec::with_capacity(actual.len());
                    let mut re = Vec::with_capacity(expected.len());
                    for (ni, k) in keep.iter().enumerate() {
                        if *k {
                            ra.push(actual[2 * ni]);
                            ra.push(actual[2 * ni + 1]);
                            re.push(expected[2 * ni]);
                            re.push(expected[2 * ni + 1]);
                        }
                    }
                    harness::assert_complex_close(&ra, &re, tol.v_rel, tol.v_abs, &ctx);
                }
                None => {
                    harness::assert_complex_close(&actual, &expected, tol.v_rel, tol.v_abs, &ctx);
                }
            }
        }

        if let Some(y) = &cp.y {
            compare_system_y(dss, y, &oc.node_order, tol, &ctx);
        } else {
            panic!("{ctx}: oracle returned no full Y (the live gate requires it)");
        }
        compare_fingerprint(dss, &cp.y_fingerprint, tol, &ctx);

        let snaps = dss.snapshot_elements();

        if star {
            let yprim_names: BTreeSet<String> =
                cp.yprims.iter().map(|y| y.name.to_lowercase()).collect();
            let all_names: BTreeSet<String> = snaps
                .iter()
                .filter(|s| dss.element_yprim(&s.name).is_some())
                .map(|s| s.name.to_lowercase())
                .collect();
            assert_eq!(
                yprim_names, all_names,
                "{ctx}: selected_elements=[\"*\"] must yield a YPrim block for every \
                 YPrim-bearing element"
            );
        } else {
            assert_eq!(
                cp.yprims.len(),
                c.selected_elements.len(),
                "{ctx}: oracle returned {} YPrim block(s) for {} selected element(s)",
                cp.yprims.len(),
                c.selected_elements.len()
            );
        }
        for yp in &cp.yprims {
            compare_yprim(dss, yp, tol, &ctx);
        }
        // A ledger `injection` scope envelope-checks the whole RHS here.
        if !ledger.is_some_and(|v| v.injection_handled(i, dss, &cp.injection, tol, &ctx)) {
            compare_injection(dss, &cp.injection, tol, &ctx);
        }

        let rust_names: BTreeSet<String> = snaps.iter().map(|s| s.name.to_lowercase()).collect();
        let oracle_names: BTreeSet<String> =
            cp.elements.iter().map(|e| e.name.to_lowercase()).collect();
        assert_eq!(
            rust_names,
            oracle_names,
            "{ctx}: element name sets differ (Rust∖oracle={:?}, oracle∖Rust={:?})",
            rust_names.difference(&oracle_names).collect::<Vec<_>>(),
            oracle_names.difference(&rust_names).collect::<Vec<_>>(),
        );
        // A ledger element scope neutralizes only its pinned sub-channels: it
        // rewrites those to the Rust values (after re-asserting them inside their
        // envelope) so the standard `compare_element` treats them as equal and
        // still tier-checks the unscoped remainder (clause (b)). Elements with no
        // scope are compared against the untouched oracle cap.
        let el_rewrites = ledger
            .map(|v| v.element_rewrites(i, &snaps, &cp.elements, tol, &ctx))
            .unwrap_or_default();
        for ec in &cp.elements {
            match el_rewrites.get(&ec.name.to_lowercase()) {
                Some(rw) => compare_element(&snaps, rw, tol, &ctx),
                None => compare_element(&snaps, ec, tol, &ctx),
            }
        }

        compare_discrete(dss, &cp.transformers, &cp.regcontrols, &cp.capacitors, &ctx);

        // A ledger monitor scope neutralizes only its pinned channel_idx (rewrites
        // it to the Rust samples after the envelope check); the header, sample
        // count, and every other channel still go through the standard comparator.
        for m in &cp.monitors {
            match ledger.and_then(|v| v.monitor_rewrite(dss, m, tol, &ctx)) {
                Some(rw) => compare_monitor(dss, &rw, tol, &ctx),
                None => compare_monitor(dss, m, tol, &ctx),
            }
        }
        for m in &cp.meters {
            compare_meter(dss, m, tol, &ctx);
        }

        assert_eq!(
            cp.probes.len(),
            c.probes.iter().map(|p| p.props.len()).sum::<usize>(),
            "{ctx}: oracle probe count differs from the manifest spec"
        );
        for p in &cp.probes {
            if !ledger.is_some_and(|v| v.probe_handled(dss, p, tol, &ctx)) {
                compare_probe(dss, p, tol, &ctx);
            }
        }
        assert_eq!(
            cp.variables.len(),
            c.compare_variables.len(),
            "{ctx}: oracle variables-capture count differs from the manifest spec"
        );
        for v in &cp.variables {
            compare_variables(dss, v, tol, &ctx);
        }
        if c.compare_eventlog {
            // A ledger eventlog `line_re` scope normalizes the diffing oracle line
            // (e.g. trailing-whitespace artifact) before the compare; unmatched
            // lines pass through unchanged.
            match ledger {
                Some(v) => {
                    let masked: Vec<String> = cp
                        .eventlog
                        .iter()
                        .map(|l| v.mask_line("eventlog", l))
                        .collect();
                    compare_eventlog(dss, &masked, channel.eventlog_spec(), &ctx);
                }
                None => compare_eventlog(dss, &cp.eventlog, channel.eventlog_spec(), &ctx),
            }
        }
        if c.compare_ctrlqueue {
            match ledger {
                Some(v) => {
                    let masked: Vec<String> = cp
                        .ctrlqueue
                        .iter()
                        .map(|l| v.mask_line("ctrlqueue", l))
                        .collect();
                    compare_ctrlqueue(dss, &masked, &ctx);
                }
                None => compare_ctrlqueue(dss, &cp.ctrlqueue, &ctx),
            }
        }

        if c.compare_global_result {
            compare_export(
                &cp.global_result,
                &rust_global_result,
                &global_result_policy(),
                &format!("{ctx} GlobalResult"),
            );
        }

        if c.compare_all_properties {
            assert!(
                !cp.all_properties.is_empty(),
                "{ctx}: compare_all_properties set but the oracle returned no \
                 property dump (all_properties request not honored?)"
            );
            // A ledger `property` scope pins one (element, prop) oracle value
            // exactly (discrete state — exact-pair only). To keep the monolithic
            // `compare_all_properties` count/order contract intact while excluding
            // that one pair from the value compare, rewrite its oracle value to the
            // Rust `?`-surface value (the ledger already asserted the oracle value
            // equals its pin), so the standard compare treats it as equal.
            let prop_keys = ledger
                .map(|v| v.property_handled_keys(dss, &cp.all_properties, &ctx))
                .unwrap_or_default();
            if prop_keys.is_empty() {
                compare_all_properties(dss, &cp.all_properties, tol, &ctx);
            } else {
                let rewritten: Vec<harness::PropsCap> = cp
                    .all_properties
                    .iter()
                    .map(|pc| {
                        let el = pc.element.to_lowercase();
                        let props = pc
                            .props
                            .iter()
                            .map(|(name, val)| {
                                if prop_keys.contains(&(el.clone(), name.to_lowercase())) {
                                    dss.command(&format!("? {}.{}", pc.element, name));
                                    (name.clone(), dss.result().to_string())
                                } else {
                                    (name.clone(), val.clone())
                                }
                            })
                            .collect();
                        harness::PropsCap {
                            element: pc.element.clone(),
                            props,
                        }
                    })
                    .collect();
                compare_all_properties(dss, &rewritten, tol, &ctx);
            }
        }
    }

    if c.compare_autoadd_log {
        let oracle_log = oc.autoadd_log.as_deref().unwrap_or_else(|| {
            panic!("{label}: compare_autoadd_log set but the oracle returned no AutoAddLog")
        });
        let case_name = dss
            .circuit()
            .expect("circuit exists after AutoAdd")
            .case_name
            .clone();
        let dir = Path::new(case_path)
            .parent()
            .expect("case_path has a parent dir");
        let log_path = dir.join(format!("{case_name}_AutoAddLog.csv"));
        let rust_log = std::fs::read_to_string(&log_path).unwrap_or_else(|e| {
            panic!("{label}: read Rust AutoAddLog {}: {e}", log_path.display())
        });
        compare_export(
            oracle_log,
            &rust_log,
            &autoadd_log_policy(tol),
            &format!("{label} AutoAddLog"),
        );
    }
    let _ = n_steps;
}

/// Assert the oracle step counts, run the Rust engine once, compare against the
/// given `channel`'s capture (its iteration + eventlog-mask policy). No guard,
/// no oracle fetch — the caller (scheduler or [`run_and_compare`]) owns those.
#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_with_result(
    oc: &CaseResult,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
    channel: EngineChannel,
    ledger: Option<&crate::ledger::LedgerView>,
) {
    assert_eq!(oc.n_steps, c.n_steps, "{label}: oracle step count");
    assert_eq!(
        oc.checkpoints.len(),
        c.n_steps,
        "{label}: oracle checkpoint count"
    );
    let tol = tol_for(&c.kind);
    let (mut dss, baseline) = run_rust_capture(label, case_path, c);
    compare_capture(
        &mut dss, baseline, oc, label, case_path, c, &tol, channel, ledger,
    );
}

/// One-shot convenience for the opt-in report tests: snapshot the case dir,
/// fetch the pinned oracle model once, compare against the `capi_v0145` channel
/// (no ledger — the report tests predate it).
pub(crate) fn run_and_compare(oracle: &Oracle, label: &str, case_path: &str, c: &SolvableCase) {
    let _guard = CorpusGuard::new(case_path);
    let oc = oracle.run_case(case_path, c);
    compare_with_result(&oc, label, case_path, c, EngineChannel::CapiV0145, None);
}

// ---------------------------------------------------------------------------
// Abort + pending contracts (unchanged behavior; abort now over a Channel).
// ---------------------------------------------------------------------------

/// Gate a deck that BOTH engines abort at solve. The oracle *raises* at solve
/// (so there is no solved state to line up); assert the ORACLE aborts with the
/// expected message and the RUST engine sets `solution_abort` + surfaces it.
pub(crate) fn run_and_compare_abort(
    channel: &Channel,
    label: &str,
    case_path: &str,
    c: &SolvableCase,
) {
    let expected = c
        .expect_solve_abort
        .as_deref()
        .expect("abort case has expect_solve_abort");

    // The abort request drives `Compile` (which executes the deck's own `Solve`);
    // the oracle raises on the aborting solve → `ok:false` carrying the message.
    let req = json!({
        "cmd": "run",
        "case_path": case_path,
        "post": c.post,
        "n_steps": c.n_steps,
        "selected_elements": c.selected_elements,
        "full_csc": true,
        "check_meters_monitors": false,
        "probes": [],
        "variables": [],
        "eventlog": false,
        "ctrlqueue": false,
    });
    let resp = channel.call(&req);
    assert!(
        !resp.ok,
        "{label}: oracle did NOT abort the solve (expected an abort containing {expected:?})"
    );
    let oracle_err = resp.error.unwrap_or_default();
    assert!(
        oracle_err.contains(expected),
        "{label}: oracle abort message {oracle_err:?} does not contain {expected:?}"
    );

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert!(
        dss.circuit().is_some_and(|ckt| ckt.solution.solution_abort),
        "{label}: Rust engine did NOT set solution_abort — the malformed input must abort \
         the solve like the oracle (message: {expected:?})"
    );
    assert!(
        dss.errors().iter().any(|e| e.contains(expected)),
        "{label}: Rust engine did not surface {expected:?}: {:?}",
        dss.errors()
    );
}

/// Schema v2 (§4 Phase C step c): a `defer_ledger` case is parked from live
/// oracle comparison (its validated capi015 behavior reproduces on neither
/// surviving channel; a Phase D ledger will re-gate it). It is still **smoke-run**
/// on the Rust engine — compile + post + solve every step must converge with NO
/// new engine errors. This catches a *convergence or error-surfacing* regression
/// (a deferred case that stops solving, NaNs out, or starts erroring); it does
/// **not** catch a *numeric-correctness* regression that still converges — no
/// physical value is compared against any reference here. Full numeric coverage
/// on these cases returns with the Phase D ledger. This is strictly stronger than
/// the plan's `pending` fallback (which asserts an error) and membership is
/// preserved, but it is a bounded, temporary reduction in verification depth.
pub(crate) fn assert_deferred_rust_smoke(label: &str, case_path: &str, c: &SolvableCase) {
    let (mut dss, baseline_errors) = run_rust_capture(label, case_path, c);
    for i in 0..c.n_steps.max(1) {
        dss.command("solve");
        assert_eq!(
            dss.errors().len(),
            baseline_errors,
            "{label} (deferred smoke) step {i}: new Rust engine errors: {:?}",
            &dss.errors()[baseline_errors.min(dss.errors().len())..]
        );
        assert!(
            dss.circuit().is_some_and(|ckt| ckt.is_solved),
            "{label} (deferred smoke) step {i}: Rust did not converge — a deferred case \
             must still SOLVE on the Rust engine (defer_ledger parks the ORACLE compare, \
             not the Rust smoke)"
        );
    }
}

/// GAPS_PLAN.md §2.3 pending discipline: the unported feature must surface as an
/// engine error (a clean run means a silent fallback masks the gap).
pub(crate) fn assert_pending_errors_loudly(label: &str, case_path: &str, c: &SolvableCase) {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for cmd in &c.post {
        dss.command(cmd);
    }
    assert!(
        !dss.errors().is_empty(),
        "{label}: pending case (wp {}) ran with NO engine error — the unported \
         feature fell back silently; if it is now ported, flip `pending: false` \
         and prove the live compare green (GAPS_PLAN.md §3.1)",
        c.wp.as_deref().unwrap_or("?"),
    );
}

// ---------------------------------------------------------------------------
// Export policies (unchanged) + panic-message helper.
// ---------------------------------------------------------------------------

/// [`ExportPolicy`] for the AutoAdd `GlobalResult` line: bus name exact, GENADD
/// improvement figure on a measured faer-vs-KLU floor (see the original WPG.5
/// note in the pre-Phase-B `corpus_live.rs`).
fn global_result_policy() -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 1e-10,
        abs: 1e-12,
        col_tol: Vec::new(),
    }
}

/// [`ExportPolicy`] for the `AutoAddLog.csv`: fixed header row then comma-
/// tokenized per-bus rows on the `micro` energy floor.
fn autoadd_log_policy(tol: &Tolerances) -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: tol.energy_rel,
        abs: tol.energy_abs,
        col_tol: Vec::new(),
    }
}

/// Extract a readable message from a `catch_unwind` payload.
pub(crate) fn panic_msg(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
