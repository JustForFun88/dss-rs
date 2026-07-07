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
//! per-case timeout). `corpus_live_solvable_cases_match_oracle` runs
//! unconditionally as part of `cargo test` — the pinned dss-python oracle (see
//! tools/golden/PIN.txt) must be installed; without it the test fails rather than
//! skipping. `corpus_live_classify` (`DSS_LIVE_CLASSIFY=1`) probes the candidate
//! manifest and writes `tmp/classify_report.json` for
//! `tools/corpus/apply_classify.py`.
//!
//! Besides the vendored corpus, three synthetic deck families under
//! `tests/corpus/` run the same live mandate: `asymmetric/`, `controls/`, and
//! `modes/` (shared machinery in the family section below). A family case
//! marked `pending: true` covers a feature the port does not implement yet:
//! the gate asserts the Rust engine errors loudly on it instead of
//! live-comparing (GAPS_PLAN.md §2.3/§3.1).
//!
//! Scope: this gate compares the full assembled **electrical** model (the Y / V /
//! current mandate, no exceptions) plus every element's powers and the discrete
//! control state, for every case. Monitor channels and EnergyMeter
//! registers/zones are **also** compared — per step, with the same
//! `compare_monitor`/`compare_meter` comparators `golden_phase6.rs` uses — for
//! the cases that opt in via `check_meters_monitors` in `solvable_now.json` (the
//! daily IEEE13/IEEE37/IEEE123 runs that define meters + deterministic-mode
//! monitors). Incidental master-defined monitors are *not* compared: the pinned
//! oracle returns a phantom `Channel(i)` for an *unsampled* monitor, so only
//! cases that actually sample their monitors opt in (see tests/TOLERANCE_NOTES.md
//! and `solvable_now_has_multistep_depth` below, which pins that ≥1 such case
//! always exists).

mod harness;

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use dss_core::exec::Dss;
use harness::{
    ElementCap, Injection, MeterCap, MonitorCap, ProbeCap, VariablesCap, YFingerprint, YMat, YPrim,
    compare_ctrlqueue, compare_discrete, compare_element, compare_eventlog, compare_fingerprint,
    compare_injection, compare_meter, compare_monitor, compare_probe, compare_system_y,
    compare_variables, compare_yprim, tol_for,
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
    /// Element-specific state channels (CONTROL_COVERAGE_PLAN.md) — empty unless
    /// the case opts in via `probes` / `compare_variables` / `compare_eventlog` /
    /// `compare_ctrlqueue` in its manifest entry.
    #[serde(default)]
    probes: Vec<ProbeCap>,
    #[serde(default)]
    variables: Vec<VariablesCap>,
    #[serde(default)]
    eventlog: Vec<String>,
    #[serde(default)]
    ctrlqueue: Vec<String>,
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
    /// Extra environment for the spawned server (engine selector). Empty for
    /// the pinned capi oracle; `Oracle::opendss` sets `DSS_ORACLE_ENGINE=oddie`
    /// + `DSS_OPENDSS_REV` to drive an official EPRI `OpenDSSDirect.dll`.
    envs: Vec<(&'static str, String)>,
}

impl Oracle {
    fn server_path() -> PathBuf {
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
        server
    }

    fn new() -> Oracle {
        let python = std::env::var("DSS_ORACLE_PYTHON").unwrap_or_else(|_| "python".to_string());
        Oracle {
            python,
            server: Self::server_path(),
            envs: Vec::new(),
        }
    }

    /// An oracle over an ORIGINAL EPRI `OpenDSSDirect.dll` (AltDSS Oddie bridge,
    /// `tools/opendss/`): same server, same protocol, same captures — only the
    /// engine binding differs (`DSS_ORACLE_ENGINE=oddie` + the revision). The
    /// interpreter must be the separate Oddie venv (dss-python 0.16.0b2,
    /// tools/opendss/PIN_OPENDSS.txt): `DSS_OPENDSS_PYTHON`, defaulting to
    /// `tools/opendss/.venv/Scripts/python.exe`. Never falls back to the pinned
    /// capi oracle's interpreter — a wrong env must fail loudly, not silently
    /// compare against the wrong engine (the server re-asserts its own pin too).
    fn opendss(rev: &str) -> Oracle {
        let python = std::env::var("DSS_OPENDSS_PYTHON").unwrap_or_else(|_| {
            let venv: PathBuf = [
                env!("CARGO_MANIFEST_DIR"),
                "..",
                "..",
                "tools",
                "opendss",
                ".venv",
                "Scripts",
                "python.exe",
            ]
            .iter()
            .collect();
            assert!(
                venv.is_file(),
                "Oddie venv interpreter missing: {} — create it per \
                 tools/opendss/README.md or set DSS_OPENDSS_PYTHON",
                venv.display()
            );
            venv.to_string_lossy().into_owned()
        });
        Oracle {
            python,
            server: Self::server_path(),
            envs: vec![
                ("DSS_ORACLE_ENGINE", "oddie".to_string()),
                ("DSS_OPENDSS_REV", rev.to_string()),
            ],
        }
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
            .envs(self.envs.iter().map(|(k, v)| (*k, v.as_str())))
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
        self.ping_engine(None);
    }

    /// Ping, and when `want_oddie = Some(rev)` also assert the answering engine
    /// IS the requested EPRI revision (`oracle.oddie == true`, `oracle.rev ==
    /// rev`) — so a lost env var can never silently compare against the pinned
    /// capi oracle instead. Returns the server-reported engine version string.
    fn ping_engine(&self, want_oddie: Option<&str>) -> String {
        let r = self.call(&json!({"cmd": "ping"}));
        assert!(r.ok, "oracle ping failed: {:?}", r.error);
        let oracle = r
            .result
            .as_ref()
            .and_then(|v| v.get("oracle"))
            .cloned()
            .unwrap_or_default();
        if let Some(rev) = want_oddie {
            assert_eq!(
                oracle.get("oddie").and_then(|v| v.as_bool()),
                Some(true),
                "oracle is not the Oddie/EPRI engine: {oracle}"
            );
            assert_eq!(
                oracle.get("rev").and_then(|v| v.as_str()),
                Some(rev),
                "oracle answered for the wrong revision: {oracle}"
            );
        }
        oracle
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Run one case and return the oracle's per-step model.
    fn run_case(&self, case_path: &str, c: &SolvableCase) -> CaseResult {
        let probes: Vec<serde_json::Value> = c
            .probes
            .iter()
            .map(|p| json!({"element": p.element, "props": p.props}))
            .collect();
        let req = json!({
            "cmd": "run",
            "case_path": case_path,
            "post": c.post,
            "n_steps": c.n_steps,
            "selected_elements": c.selected_elements,
            "full_csc": true,
            "check_meters_monitors": c.check_meters_monitors,
            "probes": probes,
            "variables": c.compare_variables,
            "eventlog": c.compare_eventlog,
            "ctrlqueue": c.compare_ctrlqueue,
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

/// Buffer small files up to this size for overwrite-restore; larger files are
/// only name-tracked (OpenDSS writes small text reports, never the multi-MiB
/// input data files). Mirrors the oracle server's `_RESTORE_MAX`.
const RESTORE_MAX: u64 = 2 * 1024 * 1024;

/// Restore a case's directory after a **Rust** run: delete any file the run
/// created and rewrite any small pre-existing file it overwrote. Pascal's
/// `Compile` sets `OutputDirectory := <case dir>` (`DSSGlobals.SetDataPath`), so
/// a migrated deck's `Export voltages` / `Show` / `Save` writes report files next
/// to the deck — pure pollution of the vendored corpus fixture, which the live
/// gate never reads (it compares the in-memory model). Mirrors the oracle
/// server's `_CorpusGuard` (`tools/oracle/oracle_server.py`), which does the same
/// on the oracle side. RAII: created before the Rust compile, restores on drop.
struct CorpusGuard {
    dir: PathBuf,
    names: BTreeSet<String>,
    buf: BTreeMap<String, Vec<u8>>,
    /// The pre-run snapshot succeeded. If the initial `read_dir` fails (transient
    /// EMFILE / AV or indexer lock on Windows), `names` would be empty and Drop
    /// would treat *every* file as run-created and delete the whole feeder dir.
    /// Guard against that catastrophe: a failed snapshot disables deletion. (The
    /// oracle's Python mirror `_CorpusGuard` now carries the same `_snapshot_ok`
    /// gate — its old whole-loop `except OSError` demonstrably deleted corpus
    /// files when one file was transiently locked mid-snapshot.)
    snapshot_ok: bool,
}

impl CorpusGuard {
    fn new(case_path: &str) -> Self {
        let dir = std::path::Path::new(case_path)
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let mut names = BTreeSet::new();
        let mut buf = BTreeMap::new();
        let snapshot_ok = match std::fs::read_dir(&dir) {
            Ok(rd) => {
                for entry in rd.flatten() {
                    let p = entry.path();
                    if !p.is_file() {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let small = entry
                        .metadata()
                        .map(|m| m.len() <= RESTORE_MAX)
                        .unwrap_or(false);
                    if small && let Ok(data) = std::fs::read(&p) {
                        buf.insert(name.clone(), data);
                    }
                    names.insert(name);
                }
                true
            }
            Err(_) => false,
        };
        Self {
            dir,
            names,
            buf,
            snapshot_ok,
        }
    }
}

impl Drop for CorpusGuard {
    fn drop(&mut self) {
        // Never delete when the pre-run snapshot failed — we cannot tell created
        // files from pre-existing ones, so deleting would nuke the vendored deck.
        if !self.snapshot_ok {
            return;
        }
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !self.names.contains(&name) {
                let _ = std::fs::remove_file(&p); // created by the run
            }
        }
        for (name, data) in &self.buf {
            // rewrite only if the run actually changed it
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

/// Compile + solve one case on both engines and compare every captured field at
/// every step. `c.selected_elements` is the YPrim focus set (`["*"]` = every
/// element); element currents/powers/losses are compared for *all* elements.
/// The element-specific channels (`probes` / `compare_variables` /
/// `compare_eventlog` / `compare_ctrlqueue`) run per step when the case opts in.
fn run_and_compare(oracle: &Oracle, label: &str, case_path: &str, c: &SolvableCase) {
    let tol = tol_for(&c.kind);
    let n_steps = c.n_steps;
    let post = &c.post;
    let star = c.selected_elements == ["*"];

    // Keep the vendored corpus pristine: a migrated deck's `Export`/`Show`/`Save`
    // (Pascal `Compile` → `OutputDirectory := <case dir>`) or a `debugtrace=yes`
    // element writes report/trace files next to the deck. Snapshot the case dir
    // *before both engines run* and restore it on drop, so this outer guard also
    // sweeps up anything the oracle's own `_CorpusGuard` couldn't remove (e.g. a
    // `STOR_<name>.csv` trace file dss-python keeps open during the run — the Rust
    // port doesn't write it, so only the oracle creates it).
    let _guard = CorpusGuard::new(case_path);

    let oc = oracle.run_case(case_path, c);
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
            // The oracle server already absorbs the pinned engine's
            // fresh-process convergence misfire by retrying in-process (see
            // `run_case` in oracle_server.py / STATUS.md §1f); a failure here
            // is a real, reproducible oracle non-convergence.
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

        // The oracle returns one YPrim block per `selected_elements` entry
        // (oracle_server.py; `["*"]` expands to every element). Assert that, so
        // a case that names selected elements can never silently skip the YPrim
        // comparison.
        if star {
            let yprim_names: BTreeSet<String> =
                cp.yprims.iter().map(|y| y.name.to_lowercase()).collect();
            // Controls/meters have no YPrim on either side — `"*"` covers every
            // YPrim-bearing element (`element_yprim() == Some`).
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
            compare_meter(&dss, m, &tol, &ctx);
        }

        // Element-specific state channels (CONTROL_COVERAGE_PLAN.md) — per step,
        // empty (skipped) unless the case opts in.
        assert_eq!(
            cp.probes.len(),
            c.probes.iter().map(|p| p.props.len()).sum::<usize>(),
            "{ctx}: oracle probe count differs from the manifest spec"
        );
        for p in &cp.probes {
            compare_probe(&mut dss, p, &tol, &ctx);
        }
        assert_eq!(
            cp.variables.len(),
            c.compare_variables.len(),
            "{ctx}: oracle variables-capture count differs from the manifest spec"
        );
        for v in &cp.variables {
            compare_variables(&mut dss, v, &tol, &ctx);
        }
        if c.compare_eventlog {
            compare_eventlog(&dss, &cp.eventlog, &ctx);
        }
        if c.compare_ctrlqueue {
            compare_ctrlqueue(&dss, &cp.ctrlqueue, &ctx);
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

/// One element-specific property-probe spec: compare `element`'s listed
/// property values (oracle `Properties(p).Val` vs the Rust `?` query).
#[derive(Debug, Clone, Deserialize)]
struct ProbeSpec {
    element: String,
    props: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct SolvableCase {
    path: String,
    #[serde(default = "default_kind")]
    kind: String,
    #[serde(default)]
    post: Vec<String>,
    #[serde(default = "default_steps")]
    n_steps: usize,
    /// YPrim focus set; `["*"]` = every element (small decks).
    #[serde(default)]
    selected_elements: Vec<String>,
    /// Opt in to comparing this case's monitor channels + EnergyMeter
    /// registers/zone (set only for cases that define them in deterministic
    /// modes — e.g. the daily IEEE13 case). Incidental master-defined monitors
    /// are not compared (their bare-snapshot sampling is ill-defined).
    #[serde(default)]
    check_meters_monitors: bool,
    /// Element-specific state channels (CONTROL_COVERAGE_PLAN.md), all opt-in:
    /// property probes, PC-element state variables, the cumulative event log,
    /// and the pending control-action queue — compared per step.
    #[serde(default)]
    probes: Vec<ProbeSpec>,
    #[serde(default)]
    compare_variables: Vec<String>,
    #[serde(default)]
    compare_eventlog: bool,
    #[serde(default)]
    compare_ctrlqueue: bool,
    /// The feature this case covers is not ported yet (GAPS_PLAN.md §2.3/§3.1):
    /// the family gate asserts the Rust engine errors loudly instead of
    /// live-comparing. The WP in `wp` flips this to `false` when it ports the
    /// feature.
    #[serde(default)]
    pending: bool,
    /// Work package that ports this case's feature (`WPG.*` → GAPS_PLAN.md,
    /// `WP8.*` → PHASE8_PLAN.md). Mandatory while `pending` is true.
    #[serde(default)]
    wp: Option<String>,
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

/// Always-on (no oracle) depth guard: the live gate's *breadth* is checked by
/// `corpus_manifest.rs` (every `.dss` accounted for), but nothing there pins its
/// *depth*. This asserts `solvable_now` keeps at least one genuinely deep case —
/// a multi-step run that also compares meters/monitors, and a case that compares
/// YPrim — so editing those down to bare snapshots fails `cargo test --workspace`
/// rather than silently dropping the multi-step / meter / monitor / YPrim live
/// coverage. Mirrors the count guards in `golden_phase5.rs` / `golden_phase6.rs`.
#[test]
fn solvable_now_has_multistep_depth() {
    let cases = load_solvable();
    assert!(!cases.is_empty(), "solvable_now must not be empty");
    let multistep_metered = cases
        .iter()
        .filter(|c| c.n_steps > 1 && c.check_meters_monitors)
        .count();
    assert!(
        multistep_metered >= 1,
        "solvable_now must keep ≥1 multi-step (n_steps>1) case with \
         check_meters_monitors=true (live multi-step + meter + monitor coverage); \
         found {multistep_metered}"
    );
    let with_yprim = cases
        .iter()
        .filter(|c| !c.selected_elements.is_empty())
        .count();
    assert!(
        with_yprim >= 1,
        "solvable_now must keep ≥1 case with selected_elements (live YPrim \
         coverage); found {with_yprim}"
    );
}

#[test]
fn corpus_live_solvable_cases_match_oracle() {
    let oracle = Oracle::new();
    oracle.ping();
    let cases = load_solvable();
    if cases.is_empty() {
        eprintln!("corpus_live: solvable_now is empty — nothing to compare yet");
        return;
    }
    for c in &cases {
        let abs = corpus_file(&c.path);
        run_and_compare(&oracle, &c.path, &abs, c);
    }
    eprintln!(
        "corpus_live: {} solvable case(s) matched the oracle",
        cases.len()
    );
}

// ---------------------------------------------------------------------------
// Synthetic deck families (tests/corpus/{asymmetric,controls,modes}/): hand-
// written / generated decks plus a family `manifest.json` of `SolvableCase`
// entries, live-compared with the same full `run_and_compare` mandate as the
// vendored corpus. Shared machinery below: a deck-dir ↔ manifest bijection
// guard with a pinned per-family coverage floor, and the live gate itself.
//
// Pending discipline (GAPS_PLAN.md §2.3/§3.1): a case with `pending: true`
// covers a feature the port does not implement yet. It is NOT live-compared;
// the gate instead asserts the Rust engine errors LOUDLY on the deck (never a
// silent fallback), so an accidental no-op path cannot hide the gap. The WP
// named in the case's `wp` field flips `pending: false` in the same commit
// that ports the feature and proves the live compare green.
//
// Multi-file cases live in a subfolder named after the deck with their
// fixtures beside them (manifest path = "<deck>/<deck>.dss"), so deck
// collection recurses.
// ---------------------------------------------------------------------------

/// One synthetic deck family under `tests/corpus/<name>/`.
struct Family {
    /// Directory name under `tests/corpus/`.
    name: &'static str,
    /// Pinned deck floor: removing a deck (even together with its manifest
    /// entry) fails `*_manifest_is_complete` — mirrors the "no silent
    /// omission" role of `corpus_manifest.rs` for the vendored corpus.
    required: &'static [&'static str],
    /// Per-family structural invariant, applied to every case (pending cases
    /// declare their full future compare spec up front).
    check_case: fn(&SolvableCase),
}

fn family_dir(name: &str) -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        name,
    ]
    .iter()
    .collect()
}

/// Absolute, forward-slashed path to a deck under `tests/corpus/<name>/`.
fn family_file(name: &str, rel: &str) -> String {
    let p = family_dir(name).join(rel);
    assert!(p.is_file(), "{name} deck missing: {}", p.display());
    p.to_string_lossy().replace('\\', "/")
}

fn load_family(name: &str) -> Vec<SolvableCase> {
    let p = family_dir(name).join("manifest.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let m: SolvableManifest =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
    m.cases
}

/// Recursively collect every `.dss` under `dir` as forward-slashed paths
/// relative to `base` (multi-file cases keep their fixtures in a subfolder
/// next to the deck).
fn collect_family_decks(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let p = entry.expect("dir entry").path();
        if p.is_dir() {
            collect_family_decks(&p, base, out);
        } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("dss")) {
            out.insert(
                p.strip_prefix(base)
                    .expect("under base")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

/// Oracle-free structural guard shared by the families: deck dir ↔ manifest
/// bijection, the pinned coverage floor, `wp` present on every pending case,
/// and the family's per-case invariant.
fn family_manifest_is_complete(fam: &Family) {
    let dir = family_dir(fam.name);
    assert!(
        dir.is_dir(),
        "{} deck dir missing: {}",
        fam.name,
        dir.display()
    );
    let mut disk: BTreeSet<String> = BTreeSet::new();
    collect_family_decks(&dir, &dir, &mut disk);
    let cases = load_family(fam.name);
    let manifested: BTreeSet<String> = cases.iter().map(|c| c.path.replace('\\', "/")).collect();
    assert_eq!(
        manifested.len(),
        cases.len(),
        "duplicate paths in {} manifest",
        fam.name
    );
    assert_eq!(
        disk, manifested,
        "{} decks on disk and manifest entries must be a bijection \
         (disk∖manifest = unclassified deck, manifest∖disk = ghost entry)",
        fam.name
    );
    for req in fam.required {
        assert!(
            manifested.contains(*req),
            "required {} deck missing: {req} (pinned coverage floor)",
            fam.name
        );
    }
    for c in &cases {
        if c.pending {
            assert!(
                c.wp.is_some(),
                "{}: pending case must name the WP that ports it (GAPS_PLAN.md §3.1)",
                c.path
            );
        }
        (fam.check_case)(c);
    }
}

/// The family live gate: pending cases must error loudly on the Rust engine
/// (oracle-free); everything else runs the full live compare.
fn family_cases_match_oracle(fam: &Family) {
    let cases = load_family(fam.name);
    assert!(!cases.is_empty(), "{} manifest must not be empty", fam.name);
    let mut pending = 0usize;
    for c in cases.iter().filter(|c| c.pending) {
        let abs = family_file(fam.name, &c.path);
        assert_pending_errors_loudly(&format!("{}:{}", fam.name, c.path), &abs, c);
        pending += 1;
    }
    let live: Vec<&SolvableCase> = cases.iter().filter(|c| !c.pending).collect();
    if !live.is_empty() {
        let oracle = Oracle::new();
        oracle.ping();
        for c in &live {
            let abs = family_file(fam.name, &c.path);
            run_and_compare(&oracle, &c.path, &abs, c);
        }
    }
    eprintln!(
        "{} live gate: {} deck(s) matched the oracle, {} pending deck(s) errored loudly",
        fam.name,
        live.len(),
        pending
    );
}

/// GAPS_PLAN.md §2.3 pending discipline. The staged decks are self-driving
/// (each contains its own `Solve`), so one `compile` executes the whole
/// scenario; the unported feature must surface as an engine error (NOT_PORTED
/// / unknown mode / invalid property) — a clean run means a silent fallback
/// path is masking the gap. The WP that ports the feature pins the exact
/// behavior; this gate pins "loud".
fn assert_pending_errors_loudly(label: &str, case_path: &str, c: &SolvableCase) {
    let _guard = CorpusGuard::new(case_path);
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
// Asymmetric family (tests/corpus/asymmetric/): per-element and combination
// coverage of orientation-sensitive YPrim stamping. Motivated by the Phase-4
// `Reactor::stamp_series` bug (03c63f2): the series stamp's bottom-left block
// was written at `(j+n, i)` instead of Pascal's `(i+n, j)` — identical for
// every *symmetric* YPrim, wrong exactly when the element's Y is non-reciprocal
// (sym-components `Z1 <> Z2`) or the excitation is unbalanced. No vendored corpus
// case exercised that configuration, so the live gate never saw it. These decks
// close the class: every stamping element (and combinations) in deliberately
// asymmetric configurations — `Z1 <> Z2` sources/reactors, FULL asymmetric
// matrix inputs (pinning the parser's `ParseAsSymMatrix` lower-triangle-wins
// overwrite order), per-phase-unequal transformer bank taps, 1φ/2φ subsets,
// delta connections — solved unbalanced and compared against the pinned oracle
// with the full `run_and_compare` mandate (V, full system Y, every element's
// currents/powers, and the named elements' YPrim blocks, which catch a
// transposed stamp regardless of excitation).
//
// VSConverter is deliberately absent: the upstream `GetCurrents` bug (see
// CLAUDE.md "Known upstream bugs") makes the oracle's reported currents violate
// KCL and mutate state on every read, so it is gated separately in
// `exec/tests/vs_converter.rs` and must not enter a live full-model compare.
//
// The `pending: true` entries are static-snapshot decks for unported element
// classes (Isource, AutoTrans, GICLine/GICTransformer/GICsource), absorbed
// from the former `tests/corpus/gaps/` staging family.
// ---------------------------------------------------------------------------

/// The pinned element-coverage floor: one deck per stamping element class plus
/// the combination decks. Removing a deck (even together with its manifest
/// entry) fails here — mirrors the "no silent omission" role of
/// `corpus_manifest.rs` for the vendored corpus.
const ASYMMETRIC_REQUIRED: &[&str] = &[
    "vsource_asym.dss",
    "reactor_asym.dss",
    "capacitor_asym.dss",
    "line_asym.dss",
    "transformer_asym.dss",
    "fault_asym.dss",
    "load_asym.dss",
    "generator_asym.dss",
    "der_asym.dss",
    "indmach_asym.dss",
    "vccs_asym.dss",
    "upfc_asym.dss",
    "combo_chain_asym.dss",
    "combo_mesh_asym.dss",
    "midi_asym.dss",
    "midi_vsource_asym.dss",
    "midi_reactor_asym.dss",
    "midi_capacitor_asym.dss",
    "midi_line_asym.dss",
    "midi_transformer_asym.dss",
    "midi_fault_asym.dss",
    "midi_load_asym.dss",
    "midi_generator_asym.dss",
    "midi_der_asym.dss",
    "midi_indmach_asym.dss",
    "midi_vccs_asym.dss",
    "midi_upfc_asym.dss",
    // pending (unported element classes; wp fields name the porting WP)
    "isource_snap.dss",
    "midi_isource_asym.dss",
    "autotrans_snap.dss",
    "midi_autotrans_asym.dss",
    "autotrans_gic.dss",
    "gicline_gic.dss",
    "gictransformer_gic.dss",
    "gicsource_gic.dss",
    "gic_midi.dss",
];

/// Every asymmetric case must name selected_elements: the live YPrim compare
/// is the direct transposed-stamp catch.
fn check_asymmetric_case(c: &SolvableCase) {
    assert!(
        !c.selected_elements.is_empty(),
        "{}: asymmetric case must name selected_elements (live YPrim compare \
         is the direct transposed-stamp catch)",
        c.path
    );
}

const ASYMMETRIC: Family = Family {
    name: "asymmetric",
    required: ASYMMETRIC_REQUIRED,
    check_case: check_asymmetric_case,
};

#[test]
fn asymmetric_manifest_is_complete() {
    family_manifest_is_complete(&ASYMMETRIC);
}

#[test]
fn asymmetric_cases_match_oracle() {
    family_cases_match_oracle(&ASYMMETRIC);
}

// ---------------------------------------------------------------------------
// Controls family (tests/corpus/controls/, CONTROL_COVERAGE_PLAN.md):
// synthetic decks putting every control / protection / metering element class
// (RegControl, CapControl, SwtControl, Relay, Fuse, Recloser, EnergyMeter,
// Monitor, Sensor, InvControl, StorageController, GenDispatcher) through
// symmetric, asymmetric, and combination scenarios, live-compared with the full
// model mandate PLUS the element-specific state channels (property probes,
// PC-element variables, event log, control queue) this file's runner wires.
//
// The `pending: true` entries are control / time-series decks for unported
// features (CapControl follow mode, InvControl exponential model + Storage
// volt-watt, StorageController seasonal targets, Isource/AutoTrans in daily
// and combined modes), absorbed from the former `tests/corpus/gaps/` staging
// family.
// ---------------------------------------------------------------------------

/// The pinned per-class deck floor (grows as CONTROL_COVERAGE_PLAN.md steps
/// land). Removing a deck (even with its manifest entry) fails here.
const CONTROLS_REQUIRED: &[&str] = &[
    "regcontrol_sym.dss",
    "regcontrol_asym.dss",
    "capcontrol_sym.dss",
    "capcontrol_asym.dss",
    "invcontrol_vv_sym.dss",
    "invcontrol_vvvw_asym.dss",
    "storagectrl_peakshave.dss",
    "storagectrl_time.dss",
    "gendispatcher.dss",
    "recloser_temp.dss",
    "recloser_perm.dss",
    "relay_oc_sym.dss",
    "relay_4647_asym.dss",
    "fuse_blow_asym.dss",
    "swtcontrol_time.dss",
    "energymeter_sym.dss",
    "energymeter_asym.dss",
    "monitor_modes.dss",
    "sensor_map.dss",
    "combo_protection.dss",
    "combo_voltvar_asym.dss",
    "combo_metering.dss",
    "midi_controls.dss",
    "midi_protection.dss",
    "midi_regcontrol.dss",
    "midi_capcontrol.dss",
    "midi_invcontrol.dss",
    "midi_storagectrl.dss",
    "midi_gendispatcher.dss",
    "midi_recloser_temp.dss",
    "midi_recloser_perm.dss",
    "midi_relay_4647.dss",
    "midi_fuse.dss",
    "midi_swtcontrol.dss",
    "midi_energymeter.dss",
    "midi_monitor.dss",
    "midi_sensor.dss",
    // pending (unported control / time-series features; wp names the WP)
    "capcontrol_follow.dss",
    "invcontrol_expmodel.dss",
    "invcontrol_storage_vw.dss",
    "invcontrol_storage_vv_vw.dss",
    "storagecontroller_seasonal.dss",
    "isource_daily.dss",
    "isource_both.dss",
    "midi_isource.dss",
    "midi_isource_both.dss",
    "autotrans_reg.dss",
    "autotrans_both.dss",
    "midi_autotrans.dss",
    "midi_autotrans_both.dss",
];

/// Every controls case must exercise at least one element-specific channel
/// (probes / variables / eventlog / ctrlqueue / meters+monitors) on top of the
/// full-model compare — a controls case without state comparison would miss
/// this gate's whole point.
fn check_controls_case(c: &SolvableCase) {
    assert!(
        !c.probes.is_empty()
            || !c.compare_variables.is_empty()
            || c.compare_eventlog
            || c.compare_ctrlqueue
            || c.check_meters_monitors,
        "{}: controls case must opt into at least one element-specific \
         state channel (probes/variables/eventlog/ctrlqueue/meters)",
        c.path
    );
}

const CONTROLS: Family = Family {
    name: "controls",
    required: CONTROLS_REQUIRED,
    check_case: check_controls_case,
};

#[test]
fn controls_manifest_is_complete() {
    family_manifest_is_complete(&CONTROLS);
}

#[test]
fn controls_cases_match_oracle() {
    family_cases_match_oracle(&CONTROLS);
}

// ---------------------------------------------------------------------------
// Modes family (tests/corpus/modes/): solve modes / solution algorithms /
// input formats / executive verbs — `Set mode=Time|LD1|LD2|M1|M2|M3|MF|
// AutoAdd`, `algorithm=Newton`, binary/CSV shape-file inputs, harmonic-curve
// elements and harmonics-mode element decks, BatchEdit, Reduce. Absorbed from
// the former `tests/corpus/gaps/` staging family (GAPS_PLAN.md §3.1): every
// deck was oracle-validated at creation (two-process determinism + feature
// sensitivity), and every case stays `pending: true` until the WP named in
// its `wp` field ports the feature and flips the flag.
// ---------------------------------------------------------------------------

/// The pinned deck floor: one deck per solve mode / algorithm / input format /
/// executive verb scenario. Removing a deck (even with its manifest entry)
/// fails here.
const MODES_REQUIRED: &[&str] = &[
    "shape_binfiles/shape_binfiles.dss",
    "generaltime.dss",
    "ld1.dss",
    "ld2.dss",
    "monte1.dss",
    "monte2.dss",
    "monte3.dss",
    "montefault.dss",
    "autoadd.dss",
    "newton.dss",
    "reactor_rlcurve.dss",
    "isource_harm.dss",
    "batchedit.dss",
    "midi_batchedit.dss",
    "reduce_default.dss",
    "reduce_shortlines.dss",
    "reduce_dangling.dss",
    "reduce_switches.dss",
    "reduce_laterals.dss",
    "reduce_mergeparallel.dss",
    "reduce_breakloop.dss",
    "reduce_keeplist.dss",
    "reduce_remove.dss",
    "midi_reduce.dss",
];

/// Every modes case must name selected_elements: the live compare (once the
/// feature is ported) pins the full model, and the YPrim focus set keeps the
/// per-element channel of that mandate explicit.
fn check_modes_case(c: &SolvableCase) {
    assert!(
        !c.selected_elements.is_empty(),
        "{}: modes case must name selected_elements (full-model live compare \
         once the feature is ported)",
        c.path
    );
}

const MODES: Family = Family {
    name: "modes",
    required: MODES_REQUIRED,
    check_case: check_modes_case,
};

#[test]
fn modes_manifest_is_complete() {
    family_manifest_is_complete(&MODES);
}

#[test]
fn modes_cases_match_oracle() {
    family_cases_match_oracle(&MODES);
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
            let case = SolvableCase {
                path: path.clone(),
                kind: "feeder".to_string(),
                n_steps: 1,
                ..Default::default()
            };
            run_and_compare(oref, &path, &abs, &case);
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

// ---------------------------------------------------------------------------
// Opt-in A/B gate against ORIGINAL EPRI OpenDSS binaries (tools/opendss/):
// the same full-model comparison as the mandatory gate, but with the oracle
// bound to an official `OpenDSSDirect.dll` (r3723 / r4088 / r4133) through the
// AltDSS Oddie bridge. The Rust port is calibrated to dss_capi 0.14.5, which
// intentionally differs from EPRI upstream (dss_capi docs/known_differences.md)
// on top of Delphi-vs-FPC numeric drift — so divergences here are *inventory*
// for the upstream-porting work, not failures. Default = report mode
// (tmp/opendss_report_<rev>.json, same catch_unwind pattern as the classifier).
//
// Triaged divergences live in tools/opendss/known_diffs.json (modeled on
// DSS-Python's KNOWN_COM_DIFF, translated to our first-failure-reason shape):
// each entry matches (case label substring, all-of reason substrings) for the
// given revisions and MUST explain its cause. The report partitions diverged
// into known/new with per-entry hit counts (dead entries surface for pruning);
// DSS_LIVE_OPENDSS_ASSERT=1 fails only on NEW divergences (intended for r3723,
// whose 82 divergences are fully cataloged). Caveat: run_and_compare stops at
// the first divergence per case, so a "known" first divergence masks any later
// one in the same case — accepted for an inventory channel; entries retire as
// upstream deltas get ported, re-exposing what was behind them. The shared
// comparators/tolerances are reused as-is — never weakened for this gate.
// ---------------------------------------------------------------------------

/// One triaged divergence class from `tools/opendss/known_diffs.json`.
#[derive(serde::Deserialize)]
struct KnownDiff {
    id: String,
    revs: Vec<String>,
    case_contains: String,
    reason_contains: Vec<String>,
    cause: String,
    #[allow(dead_code)]
    source: String,
}

fn load_known_diffs() -> Vec<KnownDiff> {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tools",
        "opendss",
        "known_diffs.json",
    ]
    .iter()
    .collect();
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    #[derive(serde::Deserialize)]
    struct Catalog {
        entries: Vec<KnownDiff>,
    }
    let cat: Catalog =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()));
    for e in &cat.entries {
        assert!(
            !e.cause.trim().is_empty() && !e.reason_contains.is_empty(),
            "known_diffs.json entry {:?}: `cause` and `reason_contains` are mandatory \
             (triage inventory, not a mute button)",
            e.id
        );
    }
    cat.entries
}

#[test]
fn corpus_live_opendss() {
    let Ok(rev) = std::env::var("DSS_LIVE_OPENDSS") else {
        eprintln!(
            "SKIPPED: set DSS_LIVE_OPENDSS=r3723|r4088|r4133 to compare against EPRI OpenDSS"
        );
        return;
    };
    assert!(
        matches!(rev.as_str(), "r3723" | "r4088" | "r4133"),
        "DSS_LIVE_OPENDSS={rev:?} — expected r3723|r4088|r4133 (tools/opendss/revisions.json)"
    );
    let assert_mode = std::env::var("DSS_LIVE_OPENDSS_ASSERT")
        .map(|v| v == "1")
        .unwrap_or(false);
    let oracle = Oracle::opendss(&rev);
    let engine = oracle.ping_engine(Some(&rev));
    eprintln!("opendss oracle ({rev}): {engine}");

    // Same case universe as the mandatory gate, labeled by source manifest.
    // Pending family cases are excluded: the feature is unported on the Rust
    // side, so there is nothing to A/B against EPRI yet.
    let mut universe: Vec<(String, String, SolvableCase)> = Vec::new();
    for c in load_solvable() {
        universe.push((format!("solvable_now:{}", c.path), corpus_file(&c.path), c));
    }
    for fam in [&ASYMMETRIC, &CONTROLS, &MODES] {
        for c in load_family(fam.name) {
            if c.pending {
                continue;
            }
            let abs = family_file(fam.name, &c.path);
            universe.push((format!("{}:{}", fam.name, c.path), abs, c));
        }
    }

    let mut matched: Vec<String> = Vec::new();
    let mut diverged: Vec<(String, String)> = Vec::new();
    let total = universe.len();
    for (i, (label, abs, c)) in universe.iter().enumerate() {
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_and_compare(&oracle, label, abs, c);
        }));
        match res {
            Ok(()) => matched.push(label.clone()),
            Err(e) => diverged.push((label.clone(), panic_msg(e))),
        }
        if (i + 1) % 25 == 0 {
            eprintln!("opendss {rev}: {}/{total} compared", i + 1);
        }
    }

    matched.sort();
    diverged.sort();

    // Partition against the triage catalog: first matching entry (by file
    // order) wins; unmatched divergences are NEW and fail assert mode.
    let catalog = load_known_diffs();
    let mut hits: Vec<(String, usize)> = catalog.iter().map(|e| (e.id.clone(), 0)).collect();
    let mut known: Vec<(String, String, String)> = Vec::new(); // label, reason, entry id
    let mut fresh: Vec<(String, String)> = Vec::new();
    for (label, reason) in &diverged {
        let hit = catalog.iter().position(|e| {
            e.revs.iter().any(|r| r == &rev)
                && label.contains(&e.case_contains)
                && e.reason_contains.iter().all(|s| reason.contains(s))
        });
        match hit {
            Some(i) => {
                hits[i].1 += 1;
                known.push((label.clone(), reason.clone(), catalog[i].id.clone()));
            }
            None => fresh.push((label.clone(), reason.clone())),
        }
    }
    for (id, n) in &hits {
        let applies = catalog
            .iter()
            .find(|e| &e.id == id)
            .is_some_and(|e| e.revs.iter().any(|r| r == &rev));
        if applies && *n == 0 {
            eprintln!(
                "opendss {rev}: WARNING known_diffs entry `{id}` had zero hits — \
                 stale? prune it (or narrow its `revs`)"
            );
        }
    }

    let trunc = |r: &str| r.chars().take(400).collect::<String>();
    let report = serde_json::json!({
        "rev": rev,
        "engine": engine,
        "total": total,
        "matched": matched,
        "known_diverged": known
            .iter()
            .map(|(label, r, id)| serde_json::json!({
                "path": label,
                "known": id,
                "reason": trunc(r),
            }))
            .collect::<Vec<_>>(),
        "diverged_new": fresh
            .iter()
            .map(|(label, r)| serde_json::json!({
                "path": label,
                "reason": trunc(r),
            }))
            .collect::<Vec<_>>(),
        "known_hits": hits
            .iter()
            .map(|(id, n)| serde_json::json!({ "id": id, "hits": n }))
            .collect::<Vec<_>>(),
    });
    let rp: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<PathBuf>()
        .join(format!("opendss_report_{rev}.json"));
    let _ = std::fs::create_dir_all(rp.parent().unwrap());
    std::fs::write(&rp, serde_json::to_string_pretty(&report).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", rp.display()));
    eprintln!(
        "opendss {rev}: {} matched, {} known-diverged, {} NEW (of {total}); report -> {}",
        matched.len(),
        known.len(),
        fresh.len(),
        rp.display()
    );
    if assert_mode && !fresh.is_empty() {
        panic!(
            "opendss {rev}: {} NEW divergence(s) from the EPRI engine not covered by \
             tools/opendss/known_diffs.json (DSS_LIVE_OPENDSS_ASSERT=1); see {}",
            fresh.len(),
            rp.display()
        );
    }
}
