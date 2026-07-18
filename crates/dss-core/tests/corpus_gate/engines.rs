//! Live-oracle transport for the unified corpus gate (schema v2 — two gating
//! channels). Every transport speaks the identical line-JSON protocol
//! (`ping`/`run`/`quit`, one compact JSON object per line) and produces the
//! byte-identical [`CaseResult`] shape, so `harness/mod.rs` comparators are
//! untouched:
//!
//! * `capi_v0145` channel — the pinned dss-python 0.15.7 / dss_capi 0.14.5
//!   oracle via `tools/oracle/oracle_server.py`. Served by a persistent
//!   [`WorkerPool`]; serial/isolate cases use the one-shot [`Oracle`].
//! * `r4133` channel — the official EPRI `OpenDSSDirect.dll` r4133 via the
//!   in-house `epri-worker` bridge (Phase A). Served by a persistent
//!   [`EpriPool`]; serial/isolate cases use the one-shot [`EpriOneShot`].
//!
//! [`Oracle`] additionally backs the opt-in report tests (`corpus_live_classify`
//! / `_properties`). The report-only EPRI/Oddie `opendss(rev)` path was retired in
//! Phase D — the `r4133` channel now gates through the in-house bridge.
//! [`Channel`] unifies all four transports for the scheduler.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::harness::{
    ElementCap, Injection, MeterCap, MonitorCap, ProbeCap, PropsCap, VariablesCap, YFingerprint,
    YMat, YPrim,
};
use crate::manifest::SolvableCase;

// ---------------------------------------------------------------------------
// Response + per-case model (deserialized from the oracle's JSON; unchanged).
// ---------------------------------------------------------------------------

/// One oracle response: `{ok, error?, result?}`.
#[derive(Debug, Deserialize)]
pub(crate) struct Resp {
    pub(crate) ok: bool,
    #[serde(default)]
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) result: Option<Value>,
}

/// The per-case model the oracle returns (mirrors the Rust capture).
#[derive(Debug, Deserialize)]
pub(crate) struct CaseResult {
    pub(crate) node_order: Vec<String>,
    pub(crate) n_steps: usize,
    pub(crate) checkpoints: Vec<Checkpoint>,
    #[serde(default)]
    pub(crate) autoadd_log: Option<String>,
}

/// One committed-step capture (same shape as the checkpoint goldens, but live).
#[derive(Debug, Deserialize)]
pub(crate) struct Checkpoint {
    pub(crate) dbl_hour: f64,
    pub(crate) iterations: i32,
    pub(crate) converged: bool,
    pub(crate) v_re: Vec<f64>,
    pub(crate) v_im: Vec<f64>,
    #[serde(default)]
    pub(crate) global_result: String,
    #[serde(default)]
    pub(crate) y: Option<YMat>,
    pub(crate) y_fingerprint: YFingerprint,
    pub(crate) yprims: Vec<YPrim>,
    pub(crate) elements: Vec<ElementCap>,
    pub(crate) injection: Injection,
    pub(crate) transformers: std::collections::BTreeMap<String, Vec<f64>>,
    pub(crate) regcontrols: std::collections::BTreeMap<String, i32>,
    pub(crate) capacitors: std::collections::BTreeMap<String, Vec<i32>>,
    #[serde(default)]
    pub(crate) monitors: Vec<MonitorCap>,
    #[serde(default)]
    pub(crate) meters: Vec<MeterCap>,
    #[serde(default)]
    pub(crate) probes: Vec<ProbeCap>,
    #[serde(default)]
    pub(crate) variables: Vec<VariablesCap>,
    #[serde(default)]
    pub(crate) eventlog: Vec<String>,
    #[serde(default)]
    pub(crate) ctrlqueue: Vec<String>,
    #[serde(default)]
    pub(crate) all_properties: Vec<PropsCap>,
}

/// The per-case wall-clock deadline for a single oracle request
/// (`DSS_ORACLE_TIMEOUT_SECS`, default 120s), shared by the one-shot and pooled
/// paths so a pathological/hanging case is recorded as a failure, not a stall.
pub(crate) fn oracle_timeout() -> Duration {
    Duration::from_secs(
        std::env::var("DSS_ORACLE_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(120),
    )
}

/// The `oracle_server.py` `run` request for a case — the single builder both
/// transports share (byte-compatible with the pre-Phase-B request).
pub(crate) fn build_run_request(case_path: &str, c: &SolvableCase) -> Value {
    let probes: Vec<Value> = c
        .probes
        .iter()
        .map(|p| json!({"element": p.element, "props": p.props}))
        .collect();
    json!({
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
        "all_properties": c.compare_all_properties,
        "global_result": c.compare_global_result,
        "autoadd_log": c.compare_autoadd_log,
        "warn_and_continue": !c.expect_warnings.is_empty(),
    })
}

pub(crate) fn oracle_server_path() -> PathBuf {
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

// ---------------------------------------------------------------------------
// One-shot oracle (fresh process per request).
// ---------------------------------------------------------------------------

/// A handle to a pinned/target-rev oracle driven ONE request per subprocess:
/// write one line, close stdin (server EOFs after replying), drain
/// stdout/stderr on threads with a deadline.
pub(crate) struct Oracle {
    python: String,
    server: PathBuf,
    envs: Vec<(&'static str, String)>,
}

impl Oracle {
    pub(crate) fn new() -> Oracle {
        let python = std::env::var("DSS_ORACLE_PYTHON").unwrap_or_else(|_| "python".to_string());
        Oracle {
            python,
            server: oracle_server_path(),
            // Pin the engine selector EXPLICITLY (audit WP-U0): an ambient
            // `DSS_ORACLE_ENGINE=capi015|oddie` must never re-bind the DEFAULT
            // pinned 0.14.5 oracle.
            envs: vec![("DSS_ORACLE_ENGINE", "capi".to_string())],
        }
    }

    /// Construct **and ping-verify** the pinned `capi_v0145` one-shot oracle.
    /// `spec` must be `None` (the target-rev one-shot shim retired in Phase C —
    /// the `r4133` channel now gates through the [`EpriPool`]/[`EpriOneShot`]);
    /// a `Some(_)` is a caller bug.
    pub(crate) fn for_spec(spec: Option<&str>) -> Oracle {
        assert!(
            spec.is_none(),
            "Oracle::for_spec target-rev shim retired (§4 Phase C) — r4133 gates via the epri pool"
        );
        let o = Oracle::new();
        o.ping();
        o
    }

    /// One-shot request/response with a wall-clock timeout. stdout/stderr are
    /// drained on threads to avoid pipe deadlock; the first stdout line that
    /// parses as a `Resp` is the answer.
    pub(crate) fn call(&self, req: &Value) -> Resp {
        let timeout = oracle_timeout();
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

    fn ping(&self) {
        self.ping_engine();
    }

    /// Ping and assert the pinned-engine identity (no `oddie`/`capi015` marker —
    /// the retired EPRI/target-rev engines must never re-bind the default pinned
    /// 0.14.5 oracle). Returns the server-reported engine version.
    pub(crate) fn ping_engine(&self) -> String {
        let r = self.call(&json!({"cmd": "ping"}));
        assert!(r.ok, "oracle ping failed: {:?}", r.error);
        let oracle = r
            .result
            .as_ref()
            .and_then(|v| v.get("oracle"))
            .cloned()
            .unwrap_or_default();
        for flag in ["oddie", "capi015"] {
            assert_ne!(
                oracle.get(flag).and_then(|v| v.as_bool()),
                Some(true),
                "default oracle answered as a `{flag}` engine: {oracle} \
                 (the pinned 0.14.5 oracle is required — check \
                 DSS_ORACLE_PYTHON/DSS_ORACLE_ENGINE)"
            );
        }
        oracle
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Run one case and return the oracle's per-step model.
    pub(crate) fn run_case(&self, case_path: &str, c: &SolvableCase) -> CaseResult {
        let req = build_run_request(case_path, c);
        let r = self.call(&req);
        assert!(r.ok, "oracle case {case_path} failed: {:?}", r.error);
        let v = r.result.expect("ok response missing result");
        serde_json::from_value(v)
            .unwrap_or_else(|e| panic!("oracle case {case_path}: malformed CaseResult: {e}"))
    }
}

// ---------------------------------------------------------------------------
// Persistent worker pool (pinned capi_v0145 channel only).
// ---------------------------------------------------------------------------

/// Recycle a worker after this many served cases. **Default 1** (a fresh worker
/// per case). Persistent `oracle_server.py` / `epri-worker` processes accumulate
/// global state that `clear` does NOT fully reset — `Set` options and
/// memory-mapped loadshape handles chief among them (the plan's R2 risk). The
/// Phase D flip to `engines:"both"` widened the r4133 pool's exposure enough to
/// surface this live as *intermittent* cross-deck contamination on BOTH channels
/// (a pooled worker that had served, e.g., a relay/harmonics/geometry deck would
/// occasionally hand the next deck a stale option/mmap → ~1e-3 divergences that
/// vanish serial/one-shot). Recycling per case makes every case see a never-used
/// worker, so the gate is **deterministic** (verified over consecutive full runs).
/// The persistent-pool win is now only the amortized *startup* (workers are still
/// spawned once up front); per-case respawn on check-in overlaps across the pool
/// and keeps the full BOTH gate well under the §3.4 target. Raise via
/// `DSS_GATE_RECYCLE_AFTER=<n>` for a faster, non-deterministic dev loop.
fn recycle_after() -> usize {
    static N: OnceLock<usize> = OnceLock::new();
    *N.get_or_init(|| {
        std::env::var("DSS_GATE_RECYCLE_AFTER")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or(1)
    })
}

/// One long-lived `oracle_server.py` process: request/response over stdin/stdout,
/// stdout lines delivered by a reader thread, stderr drained by another.
struct Worker {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    served: usize,
    broken: bool,
}

/// Spawn a persistent pinned (`DSS_ORACLE_ENGINE=capi`) worker with drain threads.
fn spawn_worker(python: &str, server: &std::path::Path) -> Worker {
    let mut child = Command::new(python)
        .arg("-u")
        .arg(server)
        .env("DSS_ORACLE_ENGINE", "capi")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot spawn persistent python oracle ({python}): {e}"));
    let stdin = child.stdin.take().expect("worker stdin");
    let stdout = child.stdout.take().expect("worker stdout");
    let stderr = child.stderr.take().expect("worker stderr");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    std::thread::spawn(move || {
        // Drain stderr so the child never blocks on a full pipe (diagnostics only).
        for line in BufReader::new(stderr).lines() {
            if line.is_err() {
                break;
            }
        }
    });
    Worker {
        child,
        stdin,
        lines: rx,
        served: 0,
        broken: false,
    }
}

impl Worker {
    /// Send one request and read exactly one JSON response line within `timeout`.
    /// On write failure / timeout / EOF, marks the worker broken and returns None.
    fn request(&mut self, req: &Value, timeout: Duration) -> Option<Resp> {
        let line = serde_json::to_string(req).expect("serialize request");
        if self.stdin.write_all(line.as_bytes()).is_err()
            || self.stdin.write_all(b"\n").is_err()
            || self.stdin.flush().is_err()
        {
            self.broken = true;
            return None;
        }
        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                self.broken = true;
                return None;
            }
            match self.lines.recv_timeout(deadline - now) {
                Ok(l) => {
                    let t = l.trim();
                    if t.is_empty() {
                        continue;
                    }
                    if let Ok(r) = serde_json::from_str::<Resp>(t) {
                        return Some(r);
                    }
                    // stray non-JSON stdout line — skip (protocol: responses only)
                }
                Err(RecvTimeoutError::Timeout) | Err(RecvTimeoutError::Disconnected) => {
                    self.broken = true;
                    return None;
                }
            }
        }
    }

    /// Terminate the worker (best-effort quit, then kill+reap).
    fn close(&mut self) {
        let _ = self.stdin.write_all(b"{\"cmd\":\"quit\"}\n");
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Assert a freshly-spawned worker is the pinned 0.14.5 engine (no
/// `oddie`/`capi015` marker) — a lost env var must fail loudly, never silently
/// compare against the wrong engine.
fn assert_pinned(w: &mut Worker, timeout: Duration) {
    let r = w
        .request(&json!({"cmd": "ping"}), timeout)
        .expect("pinned oracle worker ping failed (spawn/pin/timeout)");
    assert!(r.ok, "oracle worker ping failed: {:?}", r.error);
    let oracle = r
        .result
        .as_ref()
        .and_then(|v| v.get("oracle"))
        .cloned()
        .unwrap_or_default();
    for flag in ["oddie", "capi015"] {
        assert_ne!(
            oracle.get(flag).and_then(|v| v.as_bool()),
            Some(true),
            "pinned oracle worker answered as a `{flag}` engine: {oracle} \
             (check DSS_ORACLE_PYTHON/DSS_ORACLE_ENGINE)"
        );
    }
}

/// A pool of N persistent pinned workers, one in-flight request each.
pub(crate) struct WorkerPool {
    idle: Mutex<Vec<Worker>>,
    cv: Condvar,
    python: String,
    server: PathBuf,
    timeout: Duration,
}

impl WorkerPool {
    /// Spawn + ping-verify `size` persistent pinned workers up front.
    pub(crate) fn new(size: usize) -> WorkerPool {
        let python = std::env::var("DSS_ORACLE_PYTHON").unwrap_or_else(|_| "python".to_string());
        let server = oracle_server_path();
        let timeout = oracle_timeout();
        let mut v = Vec::with_capacity(size);
        for _ in 0..size.max(1) {
            let mut w = spawn_worker(&python, &server);
            assert_pinned(&mut w, timeout);
            v.push(w);
        }
        WorkerPool {
            idle: Mutex::new(v),
            cv: Condvar::new(),
            python,
            server,
            timeout,
        }
    }

    fn checkout(&self) -> Worker {
        let mut g = self.idle.lock().unwrap();
        while g.is_empty() {
            g = self.cv.wait(g).unwrap();
        }
        g.pop().unwrap()
    }

    fn checkin(&self, mut w: Worker) {
        if w.broken || w.served >= recycle_after() {
            w.close();
            w = spawn_worker(&self.python, &self.server);
            assert_pinned(&mut w, self.timeout);
        }
        self.idle.lock().unwrap().push(w);
        self.cv.notify_one();
    }

    /// Run one request on a checked-out worker. On a broken worker (timeout /
    /// crash / EOF): kill, respawn, retry the request ONCE on a fresh worker,
    /// then fail the case (returns `ok:false`).
    pub(crate) fn call(&self, req: &Value) -> Resp {
        let mut w = self.checkout();
        w.served += 1;
        match w.request(req, self.timeout) {
            Some(r) => {
                self.checkin(w);
                r
            }
            None => {
                w.close();
                let mut w2 = spawn_worker(&self.python, &self.server);
                assert_pinned(&mut w2, self.timeout);
                w2.served += 1;
                let out = match w2.request(req, self.timeout) {
                    Some(r) => r,
                    None => Resp {
                        ok: false,
                        error: Some(
                            "oracle worker failed twice (timeout/crash) — case failed".to_string(),
                        ),
                        result: None,
                    },
                };
                self.checkin(w2);
                out
            }
        }
    }

    /// Terminate every idle worker (call once, after the scheduler scope ends).
    pub(crate) fn close(&self) {
        for mut w in self.idle.lock().unwrap().drain(..) {
            w.close();
        }
    }
}

// ---------------------------------------------------------------------------
// r4133 channel: the in-house `epri-worker` bridge (persistent pool + one-shot).
// ---------------------------------------------------------------------------

/// Resolve the `epri-worker` binary (§3.1): `DSS_EPRI_WORKER` env override →
/// `<workspace>/target/<profile>/epri-worker(.exe)` → OnceLock `cargo build`
/// fallback (loud failure). Resolved once per test process.
fn epri_worker_bin() -> PathBuf {
    static RESOLVED: OnceLock<PathBuf> = OnceLock::new();
    RESOLVED
        .get_or_init(|| {
            if let Ok(p) = std::env::var("DSS_EPRI_WORKER") {
                let p = PathBuf::from(p);
                assert!(
                    p.is_file(),
                    "DSS_EPRI_WORKER points at a missing file: {p:?}"
                );
                return p;
            }
            let profile = if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            };
            let exe = if cfg!(windows) {
                "epri-worker.exe"
            } else {
                "epri-worker"
            };
            let root: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
            let candidate = root.join("target").join(profile).join(exe);
            if candidate.is_file() {
                return candidate;
            }
            // Fallback: build it once (covers `cargo test -p dss-core` invocations
            // that did not build the whole workspace).
            eprintln!("epri-worker not found at {candidate:?} — building it once…");
            let mut cmd =
                Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
            cmd.arg("build")
                .arg("-p")
                .arg("dss-epri")
                .arg("--bin")
                .arg("epri-worker");
            if profile == "release" {
                cmd.arg("--release");
            }
            let status = cmd
                .status()
                .unwrap_or_else(|e| panic!("cannot run `cargo build -p dss-epri`: {e}"));
            assert!(
                status.success(),
                "`cargo build -p dss-epri --bin epri-worker` failed — build it manually \
                 (or set DSS_EPRI_WORKER) before running the corpus gate"
            );
            assert!(
                candidate.is_file(),
                "epri-worker still missing after build: {candidate:?}"
            );
            candidate
        })
        .clone()
}

/// Spawn a persistent `epri-worker` process with drain threads (mirrors
/// [`spawn_worker`]; the worker resolves the r4133 DLL itself).
fn spawn_epri_worker(bin: &std::path::Path) -> Worker {
    let mut child = Command::new(bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot spawn epri-worker ({}): {e}", bin.display()));
    let stdin = child.stdin.take().expect("epri-worker stdin");
    let stdout = child.stdout.take().expect("epri-worker stdout");
    let stderr = child.stderr.take().expect("epri-worker stderr");
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines() {
            if line.is_err() {
                break;
            }
        }
    });
    Worker {
        child,
        stdin,
        lines: rx,
        served: 0,
        broken: false,
    }
}

/// Assert a freshly-spawned `epri-worker` is the r4133 EPRI engine (markers
/// `{"epri":true,"rev":"r4133"}`) — a wrong/absent DLL must fail loudly.
fn assert_epri(w: &mut Worker, timeout: Duration) {
    let r = w
        .request(&json!({"cmd": "ping"}), timeout)
        .expect("epri-worker ping failed (spawn/DLL-load/timeout)");
    assert!(r.ok, "epri-worker ping failed: {:?}", r.error);
    let oracle = r
        .result
        .as_ref()
        .and_then(|v| v.get("oracle"))
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        oracle.get("epri").and_then(|v| v.as_bool()),
        Some(true),
        "epri-worker did not answer as the EPRI bridge: {oracle}"
    );
    assert_eq!(
        oracle.get("rev").and_then(|v| v.as_str()),
        Some("r4133"),
        "epri-worker answered for the wrong revision: {oracle}"
    );
}

/// A pool of N persistent `epri-worker` processes (r4133 channel), mirroring
/// [`WorkerPool`]'s lifecycle: one in-flight request each, per-request deadline
/// → kill/respawn/retry-once-then-fail-case, recycle after 64 cases. A DLL crash
/// (`#303`, never sent — ledgered `skip` in Phase D) kills only that worker.
pub(crate) struct EpriPool {
    idle: Mutex<Vec<Worker>>,
    cv: Condvar,
    bin: PathBuf,
    timeout: Duration,
}

impl EpriPool {
    /// Spawn + ping-verify `size` persistent epri workers up front.
    pub(crate) fn new(size: usize) -> EpriPool {
        let bin = epri_worker_bin();
        let timeout = oracle_timeout();
        let mut v = Vec::with_capacity(size);
        for _ in 0..size.max(1) {
            let mut w = spawn_epri_worker(&bin);
            assert_epri(&mut w, timeout);
            v.push(w);
        }
        EpriPool {
            idle: Mutex::new(v),
            cv: Condvar::new(),
            bin,
            timeout,
        }
    }

    fn checkout(&self) -> Worker {
        let mut g = self.idle.lock().unwrap();
        while g.is_empty() {
            g = self.cv.wait(g).unwrap();
        }
        g.pop().unwrap()
    }

    fn checkin(&self, mut w: Worker) {
        if w.broken || w.served >= recycle_after() {
            w.close();
            w = spawn_epri_worker(&self.bin);
            assert_epri(&mut w, self.timeout);
        }
        self.idle.lock().unwrap().push(w);
        self.cv.notify_one();
    }

    /// Run one request on a checked-out worker; on a broken worker respawn +
    /// retry once on a fresh worker, then fail the case.
    pub(crate) fn call(&self, req: &Value) -> Resp {
        let mut w = self.checkout();
        w.served += 1;
        match w.request(req, self.timeout) {
            Some(r) => {
                self.checkin(w);
                r
            }
            None => {
                w.close();
                let mut w2 = spawn_epri_worker(&self.bin);
                assert_epri(&mut w2, self.timeout);
                w2.served += 1;
                let out = match w2.request(req, self.timeout) {
                    Some(r) => r,
                    None => Resp {
                        ok: false,
                        error: Some(
                            "epri-worker failed twice (timeout/DLL-crash) — case failed"
                                .to_string(),
                        ),
                        result: None,
                    },
                };
                self.checkin(w2);
                out
            }
        }
    }

    /// Terminate every idle worker (call once, after the scheduler scope ends).
    pub(crate) fn close(&self) {
        for mut w in self.idle.lock().unwrap().drain(..) {
            w.close();
        }
    }
}

/// A one-shot `epri-worker` transport: spawn a fresh process per request, ping,
/// run, quit. Used for serial-mode and `isolate` r4133 cases (the throwaway
/// worker the contamination proof / MMF discipline demands).
pub(crate) struct EpriOneShot {
    bin: PathBuf,
    timeout: Duration,
}

impl EpriOneShot {
    pub(crate) fn new() -> EpriOneShot {
        EpriOneShot {
            bin: epri_worker_bin(),
            timeout: oracle_timeout(),
        }
    }

    /// Spawn a fresh worker, ping-verify, run the request, quit.
    pub(crate) fn call(&self, req: &Value) -> Resp {
        let mut w = spawn_epri_worker(&self.bin);
        assert_epri(&mut w, self.timeout);
        let out = match w.request(req, self.timeout) {
            Some(r) => r,
            None => Resp {
                ok: false,
                error: Some("epri-worker one-shot failed (timeout/DLL-crash)".to_string()),
                result: None,
            },
        };
        w.close();
        out
    }
}

// ---------------------------------------------------------------------------
// Channel: the scheduler's uniform handle over the two channels' transports.
// ---------------------------------------------------------------------------

/// A comparison channel for one case: the pinned capi_v0145 pool or one-shot,
/// or the r4133 epri pool or one-shot.
pub(crate) enum Channel<'a> {
    CapiPool(&'a WorkerPool),
    CapiOneShot(&'a Oracle),
    EpriPool(&'a EpriPool),
    EpriOneShot(&'a EpriOneShot),
}

impl Channel<'_> {
    pub(crate) fn call(&self, req: &Value) -> Resp {
        match self {
            Channel::CapiPool(p) => p.call(req),
            Channel::CapiOneShot(o) => o.call(req),
            Channel::EpriPool(p) => p.call(req),
            Channel::EpriOneShot(e) => e.call(req),
        }
    }
}
