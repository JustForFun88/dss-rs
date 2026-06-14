//! Live oracle-comparison gate over the copied `electricdss-tst` corpus
//! (CORPUS_TEST_PLAN.md). For each solvable case the harness runs the **Rust**
//! engine and the **pinned dss-python oracle** at test time and compares the
//! full assembled model — node order, full system Y, node voltages, every
//! element's currents/powers, selected YPrim blocks, the injection vector, and
//! discrete control state — per step, reusing the comparison layer in
//! `harness/mod.rs`. No goldens are written; the oracle is consulted live.
//!
//! The oracle runs as a one-shot subprocess per case (`oracle_server.py`): the
//! gate writes a JSON request, closes stdin, and drains the JSON response (with a
//! per-case timeout). Because the oracle needs the pinned dss-python at runtime —
//! which most environments lack — this gate is **opt-in**: `corpus_live_solvable_
//! cases_match_oracle` runs only when `DSS_LIVE_ORACLE=1` and otherwise prints a
//! skip note and passes, so `cargo test --workspace` stays green everywhere.
//! `corpus_live_classify` (`DSS_LIVE_CLASSIFY=1`) probes the candidate manifest
//! and writes `tmp/classify_report.json` for `tools/corpus/apply_classify.py`.
//!
//! Scope: this gate compares the full assembled **electrical** model (the Y / V /
//! current mandate, no exceptions) plus every element's powers and the discrete
//! control state. Monitor channels and EnergyMeter registers/zones are NOT yet
//! compared here — no `solvable_now` case defines one today, and their fidelity
//! is already pinned against the same oracle by `golden_phase6.rs` /
//! `golden_checkpoints.rs`. They will be wired in when the first metered/monitored
//! multi-step case is promoted (see tests/TOLERANCE_NOTES.md).

mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use dss_core::exec::Dss;
use harness::{
    ElementCap, Injection, MeterCap, MonitorCap, YFingerprint, YMat, YPrim, compare_discrete,
    compare_element, compare_fingerprint, compare_injection, compare_meter, compare_monitor,
    compare_system_y, compare_yprim, tol_for,
};
use serde::Deserialize;
use serde_json::json;

// ---------------------------------------------------------------------------
// Oracle transport (persistent subprocess, line-delimited JSON).
// ---------------------------------------------------------------------------

/// One oracle response: `{ok, error?, result?}`. `result` stays a raw `Value`
/// because different commands return different shapes (`ping` -> `{pong,oracle}`,
/// `run` -> a [`CaseResult`]); `run_case` converts it.
#[derive(Debug, Deserialize)]
struct Resp {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    result: Option<serde_json::Value>,
}

/// The per-case model the oracle returns (mirrors the Rust capture).
#[derive(Debug, Deserialize)]
struct CaseResult {
    node_order: Vec<String>,
    n_steps: usize,
    checkpoints: Vec<Checkpoint>,
}

/// One committed-step capture (same shape as the checkpoint goldens, but live).
#[derive(Debug, Deserialize)]
struct Checkpoint {
    dbl_hour: f64,
    iterations: i32,
    converged: bool,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    #[serde(default)]
    y: Option<YMat>,
    y_fingerprint: YFingerprint,
    yprims: Vec<YPrim>,
    elements: Vec<ElementCap>,
    injection: Injection,
    transformers: BTreeMap<String, Vec<f64>>,
    regcontrols: BTreeMap<String, i32>,
    capacitors: BTreeMap<String, Vec<i32>>,
    /// Monitor channels / EnergyMeter registers+zone — present only when the case
    /// defines them (e.g. a daily run with a meter + monitors). Empty otherwise.
    #[serde(default)]
    monitors: Vec<MonitorCap>,
    #[serde(default)]
    meters: Vec<MeterCap>,
}

/// A handle to the pinned oracle. Each call spawns a fresh `oracle_server.py`
/// one-shot: write one request line, close stdin (so the server sees EOF and
/// exits after replying), then drain stdout/stderr with `wait_with_output`. This
/// is robust against the bidirectional-pipe deadlocks a persistent child is prone
/// to on Windows. (Reusing a persistent server is a possible future optimization
/// once the solvable corpus is large; the protocol already supports it.)
struct Oracle {
    python: String,
    server: PathBuf,
}

impl Oracle {
    fn new() -> Oracle {
        let server: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "tools",
            "oracle",
            "oracle_server.py",
        ]
        .iter()
        .collect();
        assert!(
            server.is_file(),
            "oracle server missing: {}",
            server.display()
        );
        let python = std::env::var("DSS_ORACLE_PYTHON").unwrap_or_else(|_| "python".to_string());
        Oracle { python, server }
    }

    /// One-shot request/response with a wall-clock timeout (so a pathological
    /// long-simulation / hanging case is recorded as a failure rather than
    /// stalling the run). stdout/stderr are drained on threads to avoid pipe
    /// deadlock; the first stdout line that parses as a `Resp` is the answer.
    /// Timeout via `DSS_ORACLE_TIMEOUT_SECS` (default 120).
    fn call(&self, req: &serde_json::Value) -> Resp {
        let timeout = Duration::from_secs(
            std::env::var("DSS_ORACLE_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(120),
        );
        let mut child = Command::new(&self.python)
            .arg("-u")
            .arg(&self.server)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot spawn python oracle ({}): {e}", self.python));
        {
            let mut sin = child.stdin.take().expect("oracle stdin");
            let line = serde_json::to_string(req).expect("serialize request");
            sin.write_all(line.as_bytes()).expect("write request");
            sin.write_all(b"\n").expect("write newline");
            // `sin` dropped here -> stdin closed -> server EOFs after this request.
        }
        let mut so = child.stdout.take().expect("oracle stdout");
        let mut se = child.stderr.take().expect("oracle stderr");
        let h_out = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = so.read_to_string(&mut s);
            s
        });
        let h_err = std::thread::spawn(move || {
            let mut s = String::new();
            let _ = se.read_to_string(&mut s);
            s
        });
        let deadline = Instant::now() + timeout;
        let timed_out = loop {
            match child.try_wait() {
                Ok(Some(_)) => break false,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        break true;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => panic!("oracle wait error: {e}"),
            }
        };
        let stdout = h_out.join().unwrap_or_default();
        let stderr = h_err.join().unwrap_or_default();
        if timed_out {
            return Resp {
                ok: false,
                error: Some(format!("oracle timeout after {}s", timeout.as_secs())),
                result: None,
            };
        }
        for l in stdout.lines() {
            let t = l.trim();
            if t.is_empty() {
                continue;
            }
            if let Ok(r) = serde_json::from_str::<Resp>(t) {
                return r;
            }
        }
        panic!(
            "oracle produced no JSON response\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        );
    }

    /// Validate the oracle is reachable and pinned (a one-shot ping).
    fn ping(&self) {
        let r = self.call(&json!({"cmd": "ping"}));
        assert!(r.ok, "oracle ping failed: {:?}", r.error);
    }

    /// Run one case and return the oracle's per-step model.
    fn run_case(
        &self,
        case_path: &str,
        post: &[String],
        n_steps: usize,
        selected: &[String],
        check_mm: bool,
    ) -> CaseResult {
        let req = json!({
            "cmd": "run",
            "case_path": case_path,
            "post": post,
            "n_steps": n_steps,
            "selected_elements": selected,
            "full_csc": true,
            "check_meters_monitors": check_mm,
        });
        let r = self.call(&req);
        assert!(r.ok, "oracle case {case_path} failed: {:?}", r.error);
        let v = r.result.expect("ok response missing result");
        serde_json::from_value(v)
            .unwrap_or_else(|e| panic!("oracle case {case_path}: malformed CaseResult: {e}"))
    }
}

// ---------------------------------------------------------------------------
// Case runner: both engines, full per-step comparison.
// ---------------------------------------------------------------------------

fn live_enabled() -> bool {
    std::env::var("DSS_LIVE_ORACLE")
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn manifests_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "manifests",
    ]
    .iter()
    .collect()
}

/// Absolute, forward-slashed path to a file under the **vendored** corpus
/// (`tests/corpus/electricdss-tst`). The live gate never reads `.inputs`.
fn corpus_file(rel: &str) -> String {
    let p: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(rel);
    assert!(p.is_file(), "corpus case missing: {}", p.display());
    p.to_string_lossy().replace('\\', "/")
}

/// Compile + solve one case on both engines and compare every captured field at
/// every step. `selected` is the YPrim focus set; element currents/powers are
/// compared for *all* elements.
#[allow(clippy::too_many_arguments)]
fn run_and_compare(
    oracle: &Oracle,
    label: &str,
    case_path: &str,
    post: &[String],
    n_steps: usize,
    selected: &[String],
    kind: &str,
    check_mm: bool,
) {
    let tol = tol_for(kind);
    let oc = oracle.run_case(case_path, post, n_steps, selected, check_mm);
    assert_eq!(oc.n_steps, n_steps, "{label}: oracle step count");
    assert_eq!(
        oc.checkpoints.len(),
        n_steps,
        "{label}: oracle checkpoint count"
    );

    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{case_path}\""));
    for c in post {
        dss.command(c);
    }
    assert!(
        dss.errors().is_empty(),
        "{label}: Rust engine errors after compile: {:?}",
        dss.errors()
    );

    for (i, cp) in oc.checkpoints.iter().enumerate() {
        dss.command("solve");
        assert!(
            dss.errors().is_empty(),
            "{label} step {i}: Rust engine errors: {:?}",
            dss.errors()
        );
        let ctx = format!("{label} step {i}");

        // Node order + voltages (immutable circuit borrow).
        {
            let ckt = dss.circuit().expect("circuit exists");
            assert!(cp.converged, "{ctx}: oracle did not converge");
            assert!(ckt.is_solved, "{ctx}: Rust did not converge");
            assert!(
                (ckt.solution.dbl_hour - cp.dbl_hour).abs() < 1e-9,
                "{ctx}: dblHour {} vs {}",
                ckt.solution.dbl_hour,
                cp.dbl_hour
            );
            assert_eq!(
                ckt.solution.iteration, cp.iterations,
                "{ctx}: iteration count differs"
            );
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
            harness::assert_complex_close(&actual, &expected, tol.v_rel, tol.v_abs, &ctx);
        }

        // Full assembled system Y (no exceptions) + fingerprint (extra guard).
        if let Some(y) = &cp.y {
            compare_system_y(&mut dss, y, &oc.node_order, &tol, &ctx);
        } else {
            panic!("{ctx}: oracle returned no full Y (the live gate requires it)");
        }
        compare_fingerprint(&mut dss, &cp.y_fingerprint, &tol, &ctx);

        // Snapshot every element once (needs &mut), then the immutable compares.
        let snaps = dss.snapshot_elements();

        for yp in &cp.yprims {
            compare_yprim(&dss, yp, &tol, &ctx);
        }
        compare_injection(&dss, &cp.injection, &tol, &ctx);

        // Every element's currents/powers — assert the name sets match exactly
        // (no silent omission) then compare each.
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
        for ec in &cp.elements {
            compare_element(&snaps, ec, &tol, &ctx);
        }

        compare_discrete(
            &dss,
            &cp.transformers,
            &cp.regcontrols,
            &cp.capacitors,
            &ctx,
        );

        // Monitor channels + EnergyMeter registers/zone — compared per step, like
        // the rest of the model. Empty (skipped) unless the case defines them.
        for m in &cp.monitors {
            compare_monitor(&dss, m, &tol, &ctx);
        }
        for m in &cp.meters {
            compare_meter(&dss, m, &ctx);
        }
    }
}

// ---------------------------------------------------------------------------
// Manifest-driven gate: run every `solvable_now` case from the copy.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SolvableManifest {
    #[serde(default)]
    cases: Vec<SolvableCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct SolvableCase {
    path: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    post: Vec<String>,
    #[serde(default = "default_steps")]
    n_steps: usize,
    #[serde(default)]
    selected_elements: Vec<String>,
    /// Opt in to comparing this case's monitor channels + EnergyMeter
    /// registers/zone (set only for cases that define them in deterministic
    /// modes — e.g. the daily IEEE13 case). Incidental master-defined monitors
    /// are not compared (their bare-snapshot sampling is ill-defined).
    #[serde(default)]
    check_meters_monitors: bool,
}

fn default_kind() -> String {
    "feeder".to_string()
}
fn default_steps() -> usize {
    1
}

fn load_solvable() -> Vec<SolvableCase> {
    let p = manifests_dir().join("solvable_now.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let m: SolvableManifest =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
    m.cases
}

#[test]
fn corpus_live_solvable_cases_match_oracle() {
    if !live_enabled() {
        eprintln!("SKIPPED corpus_live: set DSS_LIVE_ORACLE=1 (with the pinned oracle) to run");
        return;
    }
    let oracle = Oracle::new();
    oracle.ping();
    let cases = load_solvable();
    if cases.is_empty() {
        eprintln!("corpus_live: solvable_now is empty — nothing to compare yet");
        return;
    }
    for c in &cases {
        let abs = corpus_file(&c.path);
        run_and_compare(
            &oracle,
            &c.path,
            &abs,
            &c.post,
            c.n_steps,
            &c.selected_elements,
            &c.kind,
            c.check_meters_monitors,
        );
    }
    eprintln!(
        "corpus_live: {} solvable case(s) matched the oracle",
        cases.len()
    );
}

// ---------------------------------------------------------------------------
// Classifier (growth engine): probe the `skipped_needs_investigation` candidates
// with the full live comparison, catching per-case failures, and write a report
// proposing which become `solvable_now`. Opt-in via DSS_LIVE_CLASSIFY=1;
// `tools/corpus/apply_classify.py` turns the report into manifest moves.
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SkipManifest {
    #[serde(default)]
    cases: Vec<SkipCase>,
}

#[derive(Debug, Deserialize)]
struct SkipCase {
    path: String,
}

fn classify_enabled() -> bool {
    std::env::var("DSS_LIVE_CLASSIFY")
        .map(|v| v == "1")
        .unwrap_or(false)
}

fn panic_msg(e: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = e.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = e.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[test]
fn corpus_live_classify() {
    if !classify_enabled() {
        eprintln!("SKIPPED classify: set DSS_LIVE_CLASSIFY=1 to probe candidate cases");
        return;
    }
    let oracle = Oracle::new();
    oracle.ping();
    let p = manifests_dir().join("skipped_needs_investigation.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let m: SkipManifest =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));

    // Each candidate's failure message is captured from the `catch_unwind`
    // payload below (`panic_msg`), so we do NOT override the global panic hook:
    // replacing it would swallow a *sibling* test's panic message if both live
    // tests ran concurrently (libtest installs its own capturing hook). The
    // default hook also printing each expected failure to stderr is acceptable
    // noise for this opt-in diagnostic.
    let mut solvable: Vec<String> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let total = m.cases.len();
    for (i, c) in m.cases.iter().enumerate() {
        let path = c.path.clone();
        let oref = &oracle;
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let abs = corpus_file(&path);
            run_and_compare(oref, &path, &abs, &[], 1, &[], "feeder", false);
        }));
        match res {
            Ok(()) => solvable.push(c.path.clone()),
            Err(e) => failures.push((c.path.clone(), panic_msg(e))),
        }
        if (i + 1) % 25 == 0 {
            eprintln!("classify: {}/{total} probed", i + 1);
        }
    }

    solvable.sort();
    failures.sort();
    let report = serde_json::json!({
        "total": total,
        "solvable": solvable,
        "failures": failures
            .iter()
            .map(|(pth, r)| serde_json::json!({
                "path": pth,
                "reason": r.chars().take(400).collect::<String>(),
            }))
            .collect::<Vec<_>>(),
    });
    let rp: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<PathBuf>()
        .join("classify_report.json");
    let _ = std::fs::create_dir_all(rp.parent().unwrap());
    std::fs::write(&rp, serde_json::to_string_pretty(&report).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", rp.display()));
    eprintln!(
        "classify: {} solvable, {} failed (of {total}); report -> {}",
        solvable.len(),
        failures.len(),
        rp.display()
    );
}
