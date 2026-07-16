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
//! `compare_monitor`/`compare_meter` comparators `golden_metering_monitors.rs` uses — for
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
    ElementCap, ExportPolicy, Injection, MeterCap, MonitorCap, ProbeCap, PropsCap, RowPolicy,
    VariablesCap, YFingerprint, YMat, YPrim, compare_all_properties, compare_ctrlqueue,
    compare_discrete, compare_element, compare_eventlog, compare_export, compare_fingerprint,
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
    /// WPG.5: the `<CircuitName_>AutoAddLog.csv` contents the AutoAdd solve
    /// wrote (present only when the case sets `compare_autoadd_log`).
    #[serde(default)]
    autoadd_log: Option<String>,
}

/// One committed-step capture (same shape as the checkpoint goldens, but live).
#[derive(Debug, Deserialize)]
struct Checkpoint {
    dbl_hour: f64,
    iterations: i32,
    converged: bool,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    /// WPG.5: `DSS.GlobalResult` after this step's solve (present only when the
    /// case sets `compare_global_result`).
    #[serde(default)]
    global_result: String,
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
    /// WP8.5b: every element's full property dump — present only when the case
    /// opts in via `compare_all_properties` (or the `corpus_live_properties`
    /// pilot forces it). Empty otherwise.
    #[serde(default)]
    all_properties: Vec<PropsCap>,
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
    /// plus `DSS_OPENDSS_REV` to drive an official EPRI `OpenDSSDirect.dll`;
    /// `Oracle::capi015` sets `DSS_ORACLE_ENGINE=capi015` (the 0.15.x line).
    envs: Vec<(&'static str, String)>,
}

/// Legal manifest `oracle` values for target-rev cases (UPGRADE_PLAN.md):
/// the dss_capi 0.15.x-line oracle plus the three vendored EPRI revisions
/// (tools/opendss/revisions.json). `None`/absent = the pinned capi oracle.
const ORACLE_SPECS: &[&str] = &["capi015", "r3723", "r4088", "r4133"];

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
            // Pin the engine selector EXPLICITLY (audit WP-U0): the spawned
            // server inherits the parent environment, so an ambient
            // `DSS_ORACLE_ENGINE=capi015|oddie` left over from a target-rev
            // shell must never re-bind the DEFAULT oracle — default cases are
            // the exact-iteration 0.14.5 contract (UPGRADE_PLAN §1.1).
            envs: vec![("DSS_ORACLE_ENGINE", "capi".to_string())],
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
        Oracle {
            python: Self::oddie_venv_python(),
            server: Self::server_path(),
            envs: vec![
                ("DSS_ORACLE_ENGINE", "oddie".to_string()),
                ("DSS_OPENDSS_REV", rev.to_string()),
            ],
        }
    }

    /// The dss_capi **0.15.x-line** oracle (UPGRADE_PLAN.md): dss-python
    /// 0.16.0b2 (fastdss) from the same separate Oddie venv, driving its own
    /// bundled dss_capi 0.15.0b4 backend (OpenDSS SVN r4103, the 0.15.x/r4088
    /// line). Same server, same protocol, same captures — the scriptable
    /// r4088-line oracle for target-rev cases (`oracle: "capi015"`).
    fn capi015() -> Oracle {
        Oracle {
            python: Self::oddie_venv_python(),
            server: Self::server_path(),
            envs: vec![("DSS_ORACLE_ENGINE", "capi015".to_string())],
        }
    }

    /// The separate Oddie-venv interpreter (dss-python 0.16.0b2,
    /// tools/opendss/PIN_OPENDSS.txt): `DSS_OPENDSS_PYTHON`, defaulting to
    /// `tools/opendss/.venv/Scripts/python.exe`. Never falls back to the pinned
    /// capi oracle's interpreter — a wrong env must fail loudly, not silently
    /// compare against the wrong engine (the server re-asserts its own pin too).
    fn oddie_venv_python() -> String {
        std::env::var("DSS_OPENDSS_PYTHON").unwrap_or_else(|_| {
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
        })
    }

    /// Construct **and ping-verify** the oracle a case's manifest `oracle`
    /// spec names (UPGRADE_PLAN.md target-rev gating): `None` = the pinned
    /// dss-python 0.15.7 / dss_capi 0.14.5 oracle, `"capi015"` = the
    /// 0.15.x-line oracle, `"r3723"|"r4088"|"r4133"` = an official EPRI
    /// binary via Oddie. An unknown spec fails loudly — a typo must never
    /// silently compare against the default engine.
    fn for_spec(spec: Option<&str>) -> Oracle {
        match spec {
            None => {
                let o = Oracle::new();
                o.ping();
                o
            }
            Some("capi015") => {
                let o = Oracle::capi015();
                o.ping_capi015();
                o
            }
            Some(rev) if ORACLE_SPECS.contains(&rev) => {
                let o = Oracle::opendss(rev);
                o.ping_engine(Some(rev));
                o
            }
            Some(other) => panic!(
                "unknown manifest oracle spec {other:?} — expected one of {ORACLE_SPECS:?} \
                 (tools/opendss/README.md, UPGRADE_PLAN.md)"
            ),
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
        } else {
            // Positive identity for the DEFAULT oracle too (audit WP-U0): the
            // pinned 0.14.5 engine must not turn out to be an Oddie/capi015
            // binding that slipped in via environment — assert the marker
            // flags are ABSENT, mirroring the target-rev assertions above.
            for flag in ["oddie", "capi015"] {
                assert_ne!(
                    oracle.get(flag).and_then(|v| v.as_bool()),
                    Some(true),
                    "default oracle answered as a `{flag}` engine: {oracle} \
                     (the pinned 0.14.5 oracle is required — check \
                     DSS_ORACLE_PYTHON/DSS_ORACLE_ENGINE)"
                );
            }
        }
        oracle
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    }

    /// Ping + assert the answering engine IS the dss_capi 0.15.x-line oracle
    /// (`oracle.capi015 == true`) — so a lost env var can never silently
    /// compare against the pinned 0.14.5 oracle instead.
    fn ping_capi015(&self) {
        let r = self.call(&json!({"cmd": "ping"}));
        assert!(r.ok, "oracle ping failed: {:?}", r.error);
        let oracle = r
            .result
            .as_ref()
            .and_then(|v| v.get("oracle"))
            .cloned()
            .unwrap_or_default();
        assert_eq!(
            oracle.get("capi015").and_then(|v| v.as_bool()),
            Some(true),
            "oracle is not the capi015 (dss_capi 0.15.x-line) engine: {oracle}"
        );
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
            "all_properties": c.compare_all_properties,
            "global_result": c.compare_global_result,
            "autoadd_log": c.compare_autoadd_log,
            // CF-C Port 2: a deck with declared `expect_warnings` is one whose
            // compile/solve fires a user-model DoSimpleMsg both engines must
            // warn-and-continue on (safe Rust cannot load the DLL; the official
            // Direct DLL warns-and-solves). Tell the oracle to solve through it
            // (EarlyAbort off + tolerate the user-model errnos) instead of
            // raising — the Rust side already tolerates via `expect_warnings`.
            "warn_and_continue": !c.expect_warnings.is_empty(),
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
/// server's `_CorpusGuard` (`tools/oracle/corpus_guard.py`), which does the same
/// on the oracle side. RAII: created before the Rust compile, restores on drop.
///
/// The snapshot is RECURSIVE (WP8.8): keys are `/`-joined paths relative to the
/// case dir, so a file the run drops inside a pre-existing fixture subdir (a
/// redirected support script's `Export`, a `<CircuitName>/DI_yr_*` tree grafted
/// into a vendored folder) is detected and removed too — the old top-level-only
/// snapshot let those escape. A run that writes OUTSIDE the case-dir tree (e.g.
/// a manual `dss-cli` invocation from elsewhere) is still uncoverable here; the
/// documented backstop stays `git status tests/corpus` + `git restore`/`clean`.
struct CorpusGuard {
    dir: PathBuf,
    names: BTreeSet<String>,
    buf: BTreeMap<String, Vec<u8>>,
    /// The pre-run snapshot succeeded IN FULL. If any `read_dir` fails
    /// (transient EMFILE / AV or indexer lock on Windows), `names` would be
    /// truncated and Drop would treat pre-existing corpus files as run-created
    /// and delete them. Guard against that catastrophe: an incomplete snapshot
    /// disables deletion entirely. (The oracle's Python mirror carries the same
    /// gate — its old whole-loop `except OSError` demonstrably deleted corpus
    /// files when one file was transiently locked mid-snapshot.)
    snapshot_ok: bool,
}

impl CorpusGuard {
    /// Recursively list `dir`, pushing `/`-joined relative paths of every entry
    /// (files AND directories) into `names`, and buffering small files into
    /// `buf`. Returns false if any directory listing failed (incomplete
    /// snapshot → caller must disable deletion). Per-file metadata/read errors
    /// only skip that file's overwrite-restore buffer — the name is still
    /// tracked so it is never deleted. `file_type()` does not follow links, so
    /// a (never-expected) symlink is tracked by name and never descended into.
    fn snapshot(
        dir: &std::path::Path,
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

    fn new(case_path: &str) -> Self {
        let dir = std::path::Path::new(case_path)
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

    /// Post-run sweep: remove every entry under `dir` whose relative path is
    /// absent from the pre-run snapshot. A run-created directory is removed
    /// wholesale (never descended); a pre-existing directory is recursed to
    /// find run-created files inside it.
    fn sweep_created(&self, dir: &std::path::Path, prefix: &str) {
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
                    self.sweep_created(&entry.path(), &rel); // pre-existing dir
                }
                continue; // pre-existing file (or fixture subdir, handled above)
            }
            if is_dir {
                // Run-created directory (the DI `<CircuitName>/` tree). The
                // engines never create junctions/symlinks here, and only paths
                // absent from the pre-run snapshot are ever removed.
                let _ = std::fs::remove_dir_all(entry.path());
            } else {
                let _ = std::fs::remove_file(entry.path()); // created by the run
            }
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
        self.sweep_created(&self.dir.clone(), "");
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

/// The guard's recursive restore, driven end-to-end on a synthetic case dir
/// (WP8.8 audit-tests follow-up — the deletion path had no self-test): a
/// run-created top-level file, a run-created file INSIDE a pre-existing
/// subdir (the recursion's point), and a run-created directory tree are all
/// removed; the vendored master and an overwritten pre-existing fixture are
/// preserved/restored byte-for-byte.
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

/// Reconcile the engine's accumulated error log against a deck's declared
/// non-fatal `expect_warnings` (see the field doc): every actual error must
/// match some expected substring, and every expected substring must appear.
/// With an empty list this is exactly `errors().is_empty()`.
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
    // A deck may deliberately produce non-fatal warnings the port reproduces
    // 1:1 (CF-C Port 2: a user-written model DLL is not loadable in safe Rust,
    // so the engine warns and falls back to the built-in model — exactly the
    // official Direct DLL's warn-and-solve, which is why these decks gate vs
    // `oracle: "r3723"`). `expect_warnings` lists the substrings those messages
    // must contain: every actual error must match one (else it is an unexpected
    // failure), and every declared substring must actually appear (else the
    // warning silently stopped firing). Empty list ⇒ zero errors, as before.
    assert_expected_warnings(&dss, &c.expect_warnings, &format!("{label}: after compile"));
    // The warnings fire once at compile and then persist in the accumulating
    // error log; per step we require NO NEW errors beyond that baseline.
    let baseline_errors = dss.errors().len();

    for (i, cp) in oc.checkpoints.iter().enumerate() {
        dss.command("solve");
        assert_eq!(
            dss.errors().len(),
            baseline_errors,
            "{label} step {i}: new Rust engine errors: {:?}",
            &dss.errors()[baseline_errors.min(dss.errors().len())..]
        );
        // WPG.5: capture `DSS.GlobalResult` right after the solve — the `?`-query
        // probes below overwrite it (each `?` clears + resets GlobalResult).
        let rust_global_result = dss.result().to_string();
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
            // Iteration policy (UPGRADE_PLAN.md): exact vs the pinned 0.14.5
            // oracle (the 1:1-port contract); for a target-rev case
            // (`oracle` set) the port may converge in FEWER iterations —
            // never more — because post-RESONANCE refinement legitimately
            // shortens the fixed point while the newer engines don't. A
            // strict `<` before RESONANCE lands is still suspicious, so it
            // is printed loudly for the run log.
            if c.oracle.is_none() {
                assert_eq!(
                    ckt.solution.iteration, cp.iterations,
                    "{ctx}: iteration count differs"
                );
            } else {
                assert!(
                    ckt.solution.iteration <= cp.iterations,
                    "{ctx}: Rust used MORE iterations than the target oracle ({} > {})",
                    ckt.solution.iteration,
                    cp.iterations
                );
                if ckt.solution.iteration < cp.iterations {
                    eprintln!(
                        "{ctx}: NOTE Rust converged in {} iterations vs the target \
                         oracle's {} (allowed: <=; investigate if unexpected)",
                        ckt.solution.iteration, cp.iterations
                    );
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

        // WPG.5: `DSS.GlobalResult` (`Text.Result`) after the step's solve — the
        // AutoAdd winner + improvement figure. Tokenized on `,` so the bus name
        // is exact and the figure is tolerance-compared (a faer-vs-KLU last-digit
        // floor on the derived scalar is not a divergence).
        if c.compare_global_result {
            compare_export(
                &cp.global_result,
                &rust_global_result,
                &global_result_policy(),
                &format!("{ctx} GlobalResult"),
            );
        }

        // WP8.5b corpus property parity: every element's every property value,
        // Rust `?`-surface vs oracle `Properties(p).Val`. Additive block AFTER
        // the probes — off unless the case opts in (`compare_all_properties`).
        if c.compare_all_properties {
            assert!(
                !cp.all_properties.is_empty(),
                "{ctx}: compare_all_properties set but the oracle returned no \
                 property dump (all_properties request not honored?)"
            );
            compare_all_properties(&mut dss, &cp.all_properties, &tol, &ctx);
        }
    }

    // WPG.5: the `<CircuitName_>AutoAddLog.csv` the AutoAdd solve wrote (both
    // engines write it to the case dir, cleaned up by the CorpusGuard). Read the
    // Rust file (still present — the guard restores on drop at function end) and
    // compare it tokenwise against the oracle's captured contents.
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
            &autoadd_log_policy(&tol),
            &format!("{label} AutoAddLog"),
        );
    }
}

/// [`ExportPolicy`] for the AutoAdd `GlobalResult` line: the bus name exact
/// (text token), the GENADD improvement figure on a **measured** faer-vs-KLU
/// floor — NOT the generic 1e-4 energy floor.
///
/// WPG.5 MINOR (audit): the figure is a mild loss-difference scalar
/// (`LossWeight·(base_losses − candidate_losses)/GenkW`; ~2× cancellation, well
/// under one decimal digit), so it is nowhere near a 1e-4 cancellation floor —
/// the old policy pinned only ~3 sig figs of a 16-digit scalar without proof.
/// Probing both engines on `autoadd.dss` (the only GENADD figure case):
///   oracle `0.0180069930672805`  vs  Rust `0.0180069930672506`
///   → |Δ| = 2.99e-14 abs / 1.66e-12 rel.
/// That is the true Rust↔oracle reality for this scalar. The floor is set just
/// above it with margin for last-ulp wobble on the cancellation-amplified value
/// (rel 1e-10 ≈ 60× the measured rel, abs 1e-12 ≈ 33× the measured abs) — 6
/// orders TIGHTER than the old 1e-4, so it now pins ~10 sig figs and a real
/// regression in the weight/normalization/base-loss computation (which would
/// move the figure ≫1e-10) can no longer hide. CAPADD's GlobalResult is the
/// winner bus NAME only (no figure), so this floor is exercised solely by the
/// GENADD case. (CLAUDE.md: floors change only with empirical proof — here,
/// tightening, the safe direction.)
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

/// [`ExportPolicy`] for the `AutoAddLog.csv`: a fixed header row (verbatim) then
/// one comma-tokenized row per tested bus (bus name exact; kV/loss/UE/%/weighted/
/// iterations on the `micro` energy floor).
fn autoadd_log_policy(tol: &harness::Tolerances) -> ExportPolicy {
    ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: tol.energy_rel,
        abs: tol.energy_abs,
        col_tol: Vec::new(),
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
    /// WP8.5b corpus property parity: compare EVERY element's EVERY property
    /// value (Rust `?`-surface vs oracle `Properties(p).Val`) per step, on top
    /// of the full-model compare. Off by default (heavy); flipped `true` only on
    /// families proven fully clean by the `corpus_live_properties` pilot triage.
    #[serde(default)]
    compare_all_properties: bool,
    /// WPG.5: compare `DSS.GlobalResult` (`Text.Result`) per step — the AutoAdd
    /// winner + improvement figure (`"b3, 0.0180069930672805"`). Tokenized via
    /// `compare_export` so the bus name is exact and the figure is tolerance-
    /// compared (a faer-vs-KLU last-digit floor is not a divergence).
    #[serde(default)]
    compare_global_result: bool,
    /// WPG.5: compare the `<CircuitName_>AutoAddLog.csv` the AutoAdd solve writes
    /// (per-candidate loss/UE search rows), tokenized via `compare_export`.
    #[serde(default)]
    compare_autoadd_log: bool,
    /// The feature this case covers is not ported yet (GAPS_PLAN.md §2.3/§3.1):
    /// the family gate asserts the Rust engine errors loudly instead of
    /// live-comparing. The WP in `wp` flips this to `false` when it ports the
    /// feature.
    #[serde(default)]
    pending: bool,
    /// This deck aborts the solve on BOTH engines; the value is the error
    /// substring both must produce. Two distinct classes share this contract.
    /// **Malformed input** the port reproduces as Pascal `DSS.SolutionAbort`
    /// (e.g. CapControl `type=Follow` with no `ControlSignal`). **Control
    /// non-settling** — a *valid* model whose controls legitimately never drain
    /// the control queue within `MaxControlIter` (a regulator hunting/re-arming
    /// at a band edge, changing its tap every control iteration), which Pascal
    /// `SolveSnap` reports as `#485 Max Control Iterations Exceeded` (CF2-R: the
    /// three 8500/IEEE123 recloser-siting decks); its captured state is a
    /// mid-adjustment truncation, not a settled fixpoint — identical on both
    /// engines.
    ///
    /// Either way it is not a per-step live compare: the pinned oracle *raises*
    /// at solve (and r3723 via Oddie raises the same #485), so
    /// `run_and_compare`'s checkpoint capture cannot run. The contract is that
    /// BOTH engines abort with this message — the oracle raising it at solve, the
    /// Rust engine setting `solution_abort` and surfacing it. Mutually exclusive
    /// with `pending` and the normal compare; gated by [`run_and_compare_abort`].
    #[serde(default)]
    expect_solve_abort: Option<String>,
    /// Work package that ports this case's feature (`WPG.*` → GAPS_PLAN.md,
    /// `WP8.*` → PHASE8_PLAN.md). Mandatory while `pending` is true.
    #[serde(default)]
    wp: Option<String>,
    /// Non-fatal warnings this deck's compile deliberately produces, which the
    /// port reproduces 1:1 (CF-C Port 2: a user-written model DLL that safe Rust
    /// cannot load — the engine warns and falls back to the built-in model, like
    /// the official Direct DLL). Each string is a substring an actual engine
    /// error must contain; every actual error must match one, and every listed
    /// substring must actually appear. Empty ⇒ zero errors are tolerated.
    #[serde(default)]
    expect_warnings: Vec<String>,
    /// Target oracle for this case's live compare (UPGRADE_PLAN.md): absent =
    /// the pinned dss-python 0.15.7 / dss_capi 0.14.5 oracle; `"capi015"` =
    /// the dss_capi 0.15.x-line oracle; `"r3723"|"r4088"|"r4133"` = an
    /// official EPRI `OpenDSSDirect.dll` via the Oddie bridge. An upgrade WP
    /// flips this in the SAME commit that ports the newer upstream behavior
    /// the case covers; the iteration policy relaxes to `Rust <= oracle`.
    #[serde(default)]
    oracle: Option<String>,
    /// WP-AD.4 A-Diakoptics disposition (mandatory on every family-manifest case;
    /// enforced by `family_manifest_is_complete`). `"full"` = compare node V +
    /// currents/powers + monitors/eventlog under the deck's control mode; `"pf"` =
    /// same but both arms force `controlmode=off` (physics-only); `"off:<reason>"`
    /// = not AD-swept, reason mandatory and specific. The vendored corpus carries
    /// this in the orthogonal `manifests/ad_sweep.json` overlay instead.
    #[serde(default)]
    ad: Option<String>,
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

/// Oracle-free structural guard (parity with `family_manifest_is_complete`,
/// audit WP-U0): a typo'd `oracle` spec in `solvable_now.json` must fail fast
/// in a structural test, not only at compare time inside `Oracle::for_spec`.
#[test]
fn solvable_now_oracle_specs_are_valid() {
    for c in load_solvable() {
        if let Some(spec) = &c.oracle {
            assert!(
                ORACLE_SPECS.contains(&spec.as_str()),
                "{}: unknown oracle spec {spec:?} — expected one of {ORACLE_SPECS:?} \
                 (UPGRADE_PLAN.md target-rev gating)",
                c.path
            );
        }
    }
}

/// Always-on (no oracle) depth guard: the live gate's *breadth* is checked by
/// `corpus_manifest.rs` (every `.dss` accounted for), but nothing there pins its
/// *depth*. This asserts `solvable_now` keeps at least one genuinely deep case —
/// a multi-step run that also compares meters/monitors, and a case that compares
/// YPrim — so editing those down to bare snapshots fails `cargo test --workspace`
/// rather than silently dropping the multi-step / meter / monitor / YPrim live
/// coverage. Mirrors the count guards in `golden_timeseries_controls.rs` / `golden_metering_monitors.rs`.
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

/// Lazily-built pool of oracles keyed by the case's `oracle` manifest spec,
/// so a run mixing pinned-capi and target-rev cases spawns + ping-verifies
/// each engine binding exactly once (UPGRADE_PLAN.md).
struct OraclePool(BTreeMap<String, Oracle>);

impl OraclePool {
    fn new() -> OraclePool {
        OraclePool(BTreeMap::new())
    }

    fn get(&mut self, spec: Option<&str>) -> &Oracle {
        self.0
            .entry(spec.unwrap_or("capi").to_string())
            .or_insert_with(|| Oracle::for_spec(spec))
    }
}

#[test]
fn corpus_live_solvable_cases_match_oracle() {
    let cases = load_solvable();
    if cases.is_empty() {
        eprintln!("corpus_live: solvable_now is empty — nothing to compare yet");
        return;
    }
    let mut pool = OraclePool::new();
    let mut props_gated = 0usize;
    let mut aborts_gated = 0usize;
    for c in &cases {
        let abs = corpus_file(&c.path);
        // A deck that aborts the solve on BOTH engines (e.g. #485 Max Control
        // Iterations Exceeded: a control that never drains the queue within
        // MaxControlIter) has no solved state to line up — the oracle *raises*
        // at solve. Route it to the abort contract (both engines abort with the
        // same message), mirroring the synthetic-family gate. Mutually exclusive
        // with the per-step compare below.
        if c.expect_solve_abort.is_some() {
            run_and_compare_abort(pool.get(c.oracle.as_deref()), &c.path, &abs, c);
            aborts_gated += 1;
            continue;
        }
        // WP8.5b: gate every element's every property value (Rust `?`-surface vs
        // oracle `Properties(p).Val`) on the vendored corpus too — the pinned-capi
        // `feeder`/`micro`-kind decks, which the `corpus_live_properties` pilot
        // proved clean under triage. The heavy `large`-kind decks (8500-Node /
        // ckt5 / EPRI / IEEE123 / ADiakoptics / 4Bus-YYD) stay OFF: their
        // per-element property dump (thousands of elements × ~50 props) is too
        // slow for the mandatory gate — the pilot sweeps them instead (the
        // `large_floating_delta` IEEE123-scale tier is excluded for the same
        // reason, hence the prefix match). Target-rev cases stay off too (a
        // different engine revision renders property strings differently — the
        // known bracket/echo class, gated only vs pinned capi). `kind`/`oracle`
        // are explicit greppable tags — a coverage inventory, not a silent skip.
        let mut cc = c.clone();
        if cc.oracle.is_none() && !cc.kind.starts_with("large") {
            cc.compare_all_properties = true;
            props_gated += 1;
        }
        run_and_compare(pool.get(cc.oracle.as_deref()), &cc.path, &abs, &cc);
    }
    eprintln!(
        "corpus_live: {} solvable case(s) matched the oracle ({props_gated} with full \
         property parity, {aborts_gated} solve-abort case(s))",
        cases.len() - aborts_gated
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
    /// WP8.5b: gate EVERY element's EVERY property value (Rust `?`-surface vs
    /// oracle `Properties(p).Val`) on every live case in this family, on top of
    /// the full-model compare. Flipped `true` only after the
    /// `corpus_live_properties` pilot proved the whole family clean under triage
    /// (`harness::SKIP_PROPS` documents the comparability exclusions). A family
    /// with any unresolved property finding stays `false` (surfaced, never
    /// silent).
    compare_all_properties: bool,
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
        if let Some(spec) = &c.oracle {
            // Fail structurally (oracle-free) on a typo'd spec — before any
            // engine is spawned. (A pending case MAY set `oracle`: it declares
            // its full future compare spec up front, family convention.)
            assert!(
                ORACLE_SPECS.contains(&spec.as_str()),
                "{}: unknown oracle spec {spec:?} — expected one of {ORACLE_SPECS:?} \
                 (UPGRADE_PLAN.md target-rev gating)",
                c.path
            );
        }
        // WP-AD.4: every family-manifest case MUST carry a valid `ad` disposition
        // (loader errors on a missing/garbage field — same spirit as `pending`/`wp`).
        let ad = c.ad.as_deref().unwrap_or_else(|| {
            panic!(
                "{}:{}: missing mandatory `ad` disposition (WP-AD.4 — one of \
                 full|pf|off:<reason>)",
                fam.name, c.path
            )
        });
        assert!(
            ad_disposition_is_valid(ad),
            "{}:{}: invalid `ad` disposition {ad:?} — expected full|pf|off:<reason>",
            fam.name,
            c.path
        );
        (fam.check_case)(c);
    }
}

/// The family live gate: pending cases must error loudly on the Rust engine
/// (oracle-free); `expect_solve_abort` cases must abort the solve on BOTH
/// engines; everything else runs the full per-step live compare.
fn family_cases_match_oracle(fam: &Family) {
    let cases = load_family(fam.name);
    assert!(!cases.is_empty(), "{} manifest must not be empty", fam.name);
    for c in &cases {
        assert!(
            !(c.pending && c.expect_solve_abort.is_some()),
            "{}:{}: `pending` and `expect_solve_abort` are mutually exclusive",
            fam.name,
            c.path
        );
    }
    let mut pending = 0usize;
    for c in cases.iter().filter(|c| c.pending) {
        let abs = family_file(fam.name, &c.path);
        assert_pending_errors_loudly(&format!("{}:{}", fam.name, c.path), &abs, c);
        pending += 1;
    }
    let aborts: Vec<&SolvableCase> = cases
        .iter()
        .filter(|c| !c.pending && c.expect_solve_abort.is_some())
        .collect();
    let live: Vec<&SolvableCase> = cases
        .iter()
        .filter(|c| !c.pending && c.expect_solve_abort.is_none())
        .collect();
    if !aborts.is_empty() || !live.is_empty() {
        let mut pool = OraclePool::new();
        for c in &aborts {
            let abs = family_file(fam.name, &c.path);
            run_and_compare_abort(
                pool.get(c.oracle.as_deref()),
                &format!("{}:{}", fam.name, c.path),
                &abs,
                c,
            );
        }
        for c in &live {
            let abs = family_file(fam.name, &c.path);
            // WP8.5b: force the family-wide property-parity flag on the live
            // case (the manifest entries don't carry it — it's a family-level
            // decision after the pilot triage). Only vs the pinned capi oracle:
            // a target-rev case (capi015/EPRI) renders property strings
            // differently (the bracket/echo class), so never property-gate it.
            let mut cc = (*c).clone();
            cc.compare_all_properties |= fam.compare_all_properties && cc.oracle.is_none();
            run_and_compare(pool.get(cc.oracle.as_deref()), &cc.path, &abs, &cc);
        }
    }
    eprintln!(
        "{} live gate: {} deck(s) matched the oracle, {} abort deck(s) aborted both engines, \
         {} pending deck(s) errored loudly",
        fam.name,
        live.len(),
        aborts.len(),
        pending
    );
}

/// Gate a deck that BOTH engines abort at solve (a malformed input the port
/// reproduces as Pascal `DSS.SolutionAbort`; see `SolvableCase::expect_solve_abort`).
/// Not a per-step compare — the oracle *raises* at solve, so there is no solved
/// state to line up. Instead: prove the ORACLE aborts at solve with the expected
/// message, and the RUST engine sets `solution_abort` and surfaces the same
/// message. Both engines are consulted live (no golden).
fn run_and_compare_abort(oracle: &Oracle, label: &str, case_path: &str, c: &SolvableCase) {
    let expected = c
        .expect_solve_abort
        .as_deref()
        .expect("abort case has expect_solve_abort");
    let _guard = CorpusGuard::new(case_path);

    // Oracle side: the "run" request drives `Compile` (which executes the deck's
    // own `Solve`); the pinned dss-python raises a `DSSException` on the aborting
    // solve, which the oracle server reports as `ok:false` carrying the message.
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
    let resp = oracle.call(&req);
    assert!(
        !resp.ok,
        "{label}: oracle did NOT abort the solve (expected an abort containing {expected:?})"
    );
    let oracle_err = resp.error.unwrap_or_default();
    assert!(
        oracle_err.contains(expected),
        "{label}: oracle abort message {oracle_err:?} does not contain {expected:?}"
    );

    // Rust side: compile the same deck (its trailing `Solve` runs the daily
    // loop); the FOLLOW-without-ControlSignal path sets `solution_abort` and
    // surfaces the message, and the daily loop then freezes on the remaining
    // steps. Assert both, mirroring `line_singular_matrix_aborts_solve`.
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

/// The pinned element-coverage floor: one deck per stamping element class, the
/// combination decks, and the boundary-coverage decks added by the Phase-3
/// asymmetric wave (geometry/cable Carson-Z, wye-delta/delta-delta, Load
/// voltage-region, current-limited generator, DER state-machine). Removing a
/// deck (even together with its manifest entry) fails here — mirrors the
/// "no silent omission" role of `corpus_manifest.rs` for the vendored corpus.
const ASYMMETRIC_REQUIRED: &[&str] = &[
    "vsource/vsource_asym.dss",
    "reactor/reactor_asym.dss",
    "capacitor/capacitor_asym.dss",
    "line/line_asym.dss",
    "line/line_geometry_asym.dss",
    "line/line_cable_asym.dss",
    "line/line_spacing_asym.dss",
    "line/line_llc_harm_asym.dss",
    "line/line_ground_z_asym.dss",
    "line/line_fullcarson_asym.dss",
    "transformer/transformer_asym.dss",
    "transformer/transformer_wyedelta_asym.dss",
    "fault/fault_asym.dss",
    "load/load_asym.dss",
    "load/midi_load_vregion_asym.dss",
    "generator/generator_asym.dss",
    "generator/gen_currentlimited_asym.dss",
    "der/der_asym.dss",
    "der/der_state_asym.dss",
    "indmach/indmach_asym.dss",
    "vccs/vccs_asym.dss",
    "upfc/upfc_asym.dss",
    "combo/combo_chain_asym.dss",
    "combo/combo_mesh_asym.dss",
    "combo/midi_asym.dss",
    "combo/midi_geometry_cable_asym.dss",
    "vsource/midi_vsource_asym.dss",
    "reactor/midi_reactor_asym.dss",
    "capacitor/midi_capacitor_asym.dss",
    "line/midi_line_asym.dss",
    "transformer/midi_transformer_asym.dss",
    "fault/midi_fault_asym.dss",
    "load/midi_load_asym.dss",
    "generator/midi_generator_asym.dss",
    "der/midi_der_asym.dss",
    "indmach/midi_indmach_asym.dss",
    "vccs/midi_vccs_asym.dss",
    "upfc/midi_upfc_asym.dss",
    // pending (unported element classes; wp fields name the porting WP)
    "isource/isource_snap.dss",
    "isource/midi_isource_asym.dss",
    "autotrans/autotrans_snap.dss",
    "autotrans/midi_autotrans_asym.dss",
    "autotrans/autotrans_gic.dss",
    "gic/gicline_gic.dss",
    "gic/gictransformer_gic.dss",
    "gic/gicsource_gic.dss",
    "gic/gic_midi.dss",
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
    // WP8.5b: property parity ON — the `corpus_live_properties` pilot proved
    // every live asymmetric deck clean under triage (SKIP_PROPS documents the
    // DoubleSymMatrix-UB / transformer-cursor exclusions).
    compare_all_properties: true,
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
    "regcontrol/regcontrol_sym.dss",
    "regcontrol/regcontrol_asym.dss",
    // corpus coverage wave (controls): RegControl Pascal-branch decks.
    "regcontrol/regcontrol_ldc.dss",
    "regcontrol/regcontrol_reverse.dss",
    "regcontrol/regcontrol_remotebus.dss",
    "regcontrol/regcontrol_inversetime.dss",
    "capcontrol/capcontrol_sym.dss",
    "capcontrol/capcontrol_asym.dss",
    // corpus coverage wave (controls): CapControl case-ControlType decks.
    "capcontrol/capcontrol_pf.dss",
    "capcontrol/capcontrol_time.dss",
    "capcontrol/capcontrol_voverride.dss",
    "invcontrol/invcontrol_vv_sym.dss",
    "invcontrol/invcontrol_vvvw_asym.dss",
    // corpus coverage wave (controls): InvControl ControlMode decks.
    "invcontrol/invcontrol_drc.dss",
    "invcontrol/invcontrol_vv_drc.dss",
    "invcontrol/invcontrol_wattpf.dss",
    "invcontrol/invcontrol_wattvar.dss",
    "invcontrol/invcontrol_avr.dss",
    "invcontrol/invcontrol_monbus.dss",
    "invcontrol/midi_invcontrol_drc.dss",
    "storagecontroller/storagectrl_peakshave.dss",
    "storagecontroller/storagectrl_time.dss",
    // corpus coverage wave (controls): StorageController dispatch-mode decks.
    "storagecontroller/storagectrl_follow.dss",
    "storagecontroller/storagectrl_support.dss",
    "storagecontroller/storagectrl_ipeakshave.dss",
    "storagecontroller/storagectrl_loadshape.dss",
    "storagecontroller/storagectrl_chargelow.dss",
    "gendispatcher/gendispatcher.dss",
    "recloser/recloser_temp.dss",
    "recloser/recloser_perm.dss",
    // corpus coverage wave (controls): singleton branch decks.
    "recloser/recloser_ground.dss",
    "fuse/fuse_blow_3ph.dss",
    "swtcontrol/swtcontrol_lock.dss",
    "gendispatcher/gendispatcher_kvarlimit.dss",
    "relay/relay_oc_sym.dss",
    "relay/relay_4647_asym.dss",
    // corpus coverage wave (controls): Relay ControlType decks.
    "relay/relay_voltage.dss",
    "relay/relay_revpower.dss",
    "relay/relay_generic.dss",
    "relay/relay_distance.dss",
    "relay/relay_td21.dss",
    "relay/relay_doc.dss",
    "fuse/fuse_blow_asym.dss",
    "swtcontrol/swtcontrol_time.dss",
    "energymeter/energymeter_sym.dss",
    "energymeter/energymeter_asym.dss",
    // corpus coverage wave (controls): metering + adaptive-control decks.
    "energymeter/energymeter_options.dss",
    "monitor/monitor_modes_hi.dss",
    "monitor/monitor_seqmag.dss",
    "expcontrol/expcontrol_basic.dss",
    "monitor/monitor_modes.dss",
    "sensor/sensor_map.dss",
    // corpus coverage wave (controls): UPFC ModeUPFC decks.
    "upfc/upfc_vreg.dss",
    "upfc/upfc_doubleref.dss",
    "combo/combo_protection.dss",
    "combo/combo_voltvar_asym.dss",
    "combo/combo_metering.dss",
    "combo/midi_controls.dss",
    "combo/midi_protection.dss",
    "regcontrol/midi_regcontrol.dss",
    "capcontrol/midi_capcontrol.dss",
    "invcontrol/midi_invcontrol.dss",
    "storagecontroller/midi_storagectrl.dss",
    "gendispatcher/midi_gendispatcher.dss",
    "recloser/midi_recloser_temp.dss",
    "recloser/midi_recloser_perm.dss",
    "relay/midi_relay_4647.dss",
    "fuse/midi_fuse.dss",
    "swtcontrol/midi_swtcontrol.dss",
    "energymeter/midi_energymeter.dss",
    "monitor/midi_monitor.dss",
    "sensor/midi_sensor.dss",
    // WP-PF.2 Monitor mode-4 (flicker) sample-path decks.
    "monitor/monitor_pst.dss",
    "monitor/midi_monitor_pst.dss",
    // pending (unported control / time-series features; wp names the WP)
    "capcontrol/capcontrol_follow.dss",
    "invcontrol/invcontrol_expmodel.dss",
    "invcontrol/invcontrol_storage_vw.dss",
    "invcontrol/invcontrol_storage_vv_vw.dss",
    "storagecontroller/storagecontroller_seasonal.dss",
    "isource/isource_daily.dss",
    "isource/isource_both.dss",
    "isource/midi_isource.dss",
    "isource/midi_isource_both.dss",
    "autotrans/autotrans_reg.dss",
    "autotrans/autotrans_both.dss",
    "autotrans/midi_autotrans.dss",
    "autotrans/midi_autotrans_both.dss",
    // WPG.13/WPG.17 grid-forming decks (audit settlement: every feature deck
    // joins the anti-deletion floor).
    "gfm/gfm_micro.dss",
    "gfm/gfm_invcontrol.dss",
    "gfm/gfm_dynamics.dss",
    "gfm/pv_gfm_dynamics.dss",
    // WP-U1.3 InvControl-cluster upgrade decks (oracle capi015): D1
    // InvControlDeltaV (multi-control fleet build) + D4 delta-DER LL monitored
    // voltage.
    "invcontrol/invcontrol_multi_vv_wye.dss",
    "invcontrol/invcontrol_vv_delta.dss",
];

/// Every controls case must exercise at least one element-specific channel
/// (probes / variables / eventlog / ctrlqueue / meters+monitors) on top of the
/// full-model compare — a controls case without state comparison would miss
/// this gate's whole point. An `expect_solve_abort` case is exempt: it has no
/// solved state to probe, and its stronger contract (both engines abort the
/// solve with the same message) is verified by [`run_and_compare_abort`].
fn check_controls_case(c: &SolvableCase) {
    if c.expect_solve_abort.is_some() {
        return;
    }
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
    // WP8.5b: property parity ON — the pilot proved every live controls deck
    // clean under triage (SKIP_PROPS documents the reliability-UB FaultRate/
    // pctperm + transformer-cursor exclusions; RegControl.TapNum was a real
    // port bug this WP fixed, not a skip).
    compare_all_properties: true,
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
    "inputformat/shape_binfiles/shape_binfiles.dss",
    // WPG.17 feature decks (audit settlement: the anti-deletion floor must
    // cover every feature deck, not only the pre-WPG.17 set).
    "inputformat/xycurve_files/xycurve_files.dss",
    "inputformat/shape_mmf/shape_mmf.dss",
    "inputformat/shape_filearr/shape_filearr.dss",
    "time/generaltime.dss",
    "time/ld1.dss",
    "time/ld2.dss",
    "montecarlo/monte1.dss",
    "montecarlo/monte2.dss",
    "montecarlo/monte3.dss",
    "montecarlo/montefault.dss",
    "autoadd/autoadd.dss",
    "autoadd/autoadd_cap.dss",
    "newton/newton.dss",
    "newton/newton_feeder.dss",
    "harmonics/reactor_rlcurve.dss",
    "harmonics/isource_harm.dss",
    "batchedit/batchedit.dss",
    "batchedit/midi_batchedit.dss",
    "reduce/reduce_default.dss",
    "reduce/reduce_shortlines.dss",
    "reduce/reduce_dangling.dss",
    "reduce/reduce_switches.dss",
    "reduce/reduce_laterals.dss",
    "reduce/reduce_mergeparallel.dss",
    "reduce/reduce_breakloop.dss",
    "reduce/reduce_keeplist.dss",
    "reduce/reduce_remove.dss",
    "reduce/midi_reduce.dss",
    // UPGRADE_PLAN.md WP-U0: the target-rev oracle machinery pilot (compares
    // against the official EPRI r4133 binary; keeps the multi-oracle plumbing
    // exercised by every cargo test).
    "upgrade/upgrade_pilot.dss",
    // WPG.21 MakePosSequence feature decks (pending until WPG.21 A2 wires the
    // `makeposseq` dispatch; each errors loudly on the Rust engine meanwhile).
    "makeposseq/makeposseq_line.dss",
    "makeposseq/makeposseq_xfmr.dss",
    "makeposseq/makeposseq_shunt.dss",
    "makeposseq/makeposseq_pc.dss",
    "makeposseq/makeposseq_ctrl.dss",
    "makeposseq/makeposseq_report.dss",
    // GEN-MODE solve-mode coverage wave (audit settlement: every feature deck
    // joins the anti-deletion floor, per the WPG.13/WPG.17 convention above).
    "time/daily.dss",
    "time/daily_bigstep.dss",
    "time/yearly.dss",
    "time/duty.dss",
    "time/midi_duty_ctrl.dss",
    "harmonics/harmonic_hlist.dss",
    "harmonics/harmonict.dss",
    "reset/mode_reset.dss",
    // WP-U1.8 WindGen + WTG3 dynamics feature decks (audit settlement: every
    // feature deck joins the anti-deletion floor).
    "windgen/windgen_snap.dss",
    "windgen/windgen_snap_delta.dss",
    "windgen/windgen_daily.dss",
    "windgen/windgen_dyn.dss",
    "windgen/windgen_dyn_fault.dss",
    // WP-U1.4 LineSpacing equivalent-spacing model feature deck.
    "upgrade/upgrade_linecs_eqspacing.dss",
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
    // WP8.5b: property parity ON. Most modes decks are `pending: true` (feature
    // unported → error loudly, never property-compared), but 14 are already live
    // pinned-capi decks — `batchedit`/`midi_batchedit` (which EDIT properties),
    // the 10 `reduce_*` (which render merged/removed-element properties),
    // `shape_binfiles`, `isource_harm` — and the pilot proved every one clean, so
    // they now property-compare (a rendering regression on an edited/reduced
    // element is exactly what this catches). The `upgrade_pilot` target-rev case
    // stays off via the `cc.oracle.is_none()` guard in `family_cases_match_oracle`.
    compare_all_properties: true,
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
// WP8.5b Phase A pilot (report-first): sweep the pinned-capi solvable_now +
// asymmetric + controls universe with the full property dump forced, compare
// EVERY element's EVERY property value (Rust `?`-surface vs oracle
// `Properties(p).Val`), and write `tmp/props_report.json` — the triage
// artifact. Opt-in via DSS_LIVE_PROPS=1; DSS_LIVE_PROPS_MAX=<n> caps the sweep
// to the first n attempted cases (subset scoping — the report records covered
// vs subset-skipped, never a silent truncation).
// ---------------------------------------------------------------------------

fn props_pilot_enabled() -> bool {
    std::env::var("DSS_LIVE_PROPS")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[test]
fn corpus_live_properties() {
    if !props_pilot_enabled() {
        eprintln!("SKIPPED props: set DSS_LIVE_PROPS=1 to sweep all-property parity");
        return;
    }
    let oracle = Oracle::new();
    oracle.ping();

    // Pinned-capi, non-pending cases from every property-relevant source
    // (target-rev cases excluded — they gate a different engine's behavior). The
    // fast family decks (asymmetric + controls + the non-pending modes decks:
    // batchedit / reduce_* / shape / isource_harm) sweep FIRST so a capped run
    // covers them fully; the vendored solvable_now feeders follow.
    let mut universe: Vec<(String, String, SolvableCase)> = Vec::new();
    for fam in [&ASYMMETRIC, &CONTROLS, &MODES] {
        for c in load_family(fam.name) {
            // Abort cases have no solved state to property-compare (the oracle
            // raises at solve); their contract is `run_and_compare_abort`.
            if c.pending || c.oracle.is_some() || c.expect_solve_abort.is_some() {
                continue;
            }
            let abs = family_file(fam.name, &c.path);
            universe.push((format!("{}:{}", fam.name, c.path), abs, c));
        }
    }
    for c in load_solvable() {
        // Abort cases have no solved state to property-compare (the oracle raises
        // at solve); the abort contract is gated by `run_and_compare_abort`.
        if c.oracle.is_some() || c.expect_solve_abort.is_some() {
            continue;
        }
        universe.push((format!("solvable_now:{}", c.path), corpus_file(&c.path), c));
    }

    let cap = std::env::var("DSS_LIVE_PROPS_MAX")
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let total = universe.len();

    let mut covered: Vec<String> = Vec::new();
    let mut subset_skipped: Vec<String> = Vec::new();
    let mut failures: Vec<(String, String)> = Vec::new();
    let mut sweep_elems = 0usize; // Σ elements × steps
    let mut sweep_cmps = 0usize; // Σ element × prop × step value comparisons

    for (i, (label, abs, c)) in universe.iter().enumerate() {
        if let Some(cap) = cap
            && covered.len() + failures.len() >= cap
        {
            subset_skipped.push(label.clone());
            continue;
        }
        let mut case = c.clone();
        case.compare_all_properties = true;
        let oref = &oracle;
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = CorpusGuard::new(abs);
            let oc = oref.run_case(abs, &case);
            let tol = tol_for(&case.kind);
            let mut dss = Dss::new();
            dss.command("clear");
            dss.command(&format!("compile \"{abs}\""));
            for p in &case.post {
                dss.command(p);
            }
            assert!(
                dss.errors().is_empty(),
                "Rust compile errors: {:?}",
                dss.errors()
            );
            let mut elems = 0usize;
            let mut cmps = 0usize;
            for cp in &oc.checkpoints {
                dss.command("solve");
                assert!(
                    dss.errors().is_empty(),
                    "Rust solve errors: {:?}",
                    dss.errors()
                );
                assert!(cp.converged, "oracle non-convergence");
                elems += cp.all_properties.len();
                cmps += cp
                    .all_properties
                    .iter()
                    .map(|p| p.props.len())
                    .sum::<usize>();
                compare_all_properties(&mut dss, &cp.all_properties, &tol, label);
            }
            (elems, cmps)
        }));
        match res {
            Ok((e, cm)) => {
                covered.push(label.clone());
                sweep_elems += e;
                sweep_cmps += cm;
            }
            Err(e) => failures.push((label.clone(), panic_msg(e))),
        }
        if (i + 1) % 10 == 0 {
            eprintln!("props: {}/{total} swept", i + 1);
        }
    }

    covered.sort();
    subset_skipped.sort();
    failures.sort();
    let report = serde_json::json!({
        "total": total,
        "covered_count": covered.len(),
        "failed_count": failures.len(),
        "subset_skipped_count": subset_skipped.len(),
        "sweep_elements_x_steps": sweep_elems,
        "sweep_value_comparisons": sweep_cmps,
        "covered": covered,
        "subset_skipped": subset_skipped,
        "failures": failures
            .iter()
            .map(|(pth, r)| serde_json::json!({
                "path": pth,
                "reason": r.chars().take(600).collect::<String>(),
            }))
            .collect::<Vec<_>>(),
    });
    let rp: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<PathBuf>()
        .join("props_report.json");
    let _ = std::fs::create_dir_all(rp.parent().unwrap());
    std::fs::write(&rp, serde_json::to_string_pretty(&report).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", rp.display()));
    eprintln!(
        "props: {} covered, {} failed, {} subset-skipped (of {total}); \
         {sweep_elems} element-steps × props = {sweep_cmps} value comparisons; report -> {}",
        covered.len(),
        failures.len(),
        subset_skipped.len(),
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
// Triaged divergences live in tests/corpus/known_diffs.json (modeled on
// DSS-Python's KNOWN_COM_DIFF, translated to our first-failure-reason shape):
// each `diff` entry matches (case label substring, all-of reason substrings)
// for the given revisions and MUST explain its cause; a `skip` entry marks a
// case that is not expected to run/converge on those revisions at all and is
// skipped up front (reported under `known_skipped`). The report partitions
// diverged into known/new with per-entry hit counts (dead entries surface for
// pruning); DSS_LIVE_OPENDSS_ASSERT=1 fails only on NEW divergences (intended
// for r3723, whose 82 divergences are fully cataloged). Caveat:
// run_and_compare stops at the first divergence per case, so a "known" first
// divergence masks any later one in the same case — accepted for an inventory
// channel; entries retire as upstream deltas get ported, re-exposing what was
// behind them. The shared comparators/tolerances are reused as-is — never
// weakened for this gate.
// ---------------------------------------------------------------------------

/// One triaged entry from `tests/corpus/known_diffs.json`. `kind` partitions
/// the catalog: `"diff"` (default) = the case runs but legitimately diverges
/// (matched on the failure reason); `"skip"` = the case is not expected to
/// run/converge on the listed revs at all (matched on `case_contains` only,
/// skipped up front and reported under `known_skipped`).
#[derive(serde::Deserialize)]
struct KnownDiff {
    id: String,
    #[serde(default = "default_diff_kind")]
    kind: String,
    revs: Vec<String>,
    case_contains: String,
    #[serde(default)]
    reason_contains: Vec<String>,
    cause: String,
    #[allow(dead_code)]
    source: String,
}

fn default_diff_kind() -> String {
    "diff".to_string()
}

fn load_known_diffs() -> Vec<KnownDiff> {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
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
            !e.cause.trim().is_empty(),
            "known_diffs.json entry {:?}: `cause` is mandatory \
             (triage inventory, not a mute button)",
            e.id
        );
        match e.kind.as_str() {
            "diff" => assert!(
                !e.reason_contains.is_empty(),
                "known_diffs.json entry {:?}: a `diff` entry needs `reason_contains`",
                e.id
            ),
            "skip" => assert!(
                !e.case_contains.is_empty(),
                "known_diffs.json entry {:?}: a `skip` entry needs a non-empty \
                 `case_contains` (it matches on the case alone)",
                e.id
            ),
            k => panic!("known_diffs.json entry {:?}: unknown kind {k:?}", e.id),
        }
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
    // side, so there is nothing to A/B against EPRI yet. Target-rev cases
    // (`oracle` set, UPGRADE_PLAN.md) are excluded too: the Rust side
    // deliberately implements a DIFFERENT revision's behavior, and their
    // gating already happens in the mandatory gate against their own target
    // oracle — sweeping them here would only manufacture divergence noise.
    let mut universe: Vec<(String, String, SolvableCase)> = Vec::new();
    let mut target_rev_excluded: Vec<String> = Vec::new();
    // WP8.5b: never request the full property dump on the EPRI A/B channel — its
    // property-format differences (bracket/echo class) would drown the inventory
    // in known non-divergences; property parity is gated only against the pinned
    // capi oracle (tools/oracle/README notes this). One flip per case.
    for c in load_solvable() {
        if c.oracle.is_some() {
            target_rev_excluded.push(format!("solvable_now:{}", c.path));
            continue;
        }
        // Abort cases raise #485 at solve on BOTH the pinned oracle AND r3723 via
        // Oddie (dss-python's error check elevates the Direct DLL's `DoSimpleMsg`
        // to a `DSSException` — verified), so `run_and_compare`'s checkpoint
        // capture cannot run through the raised solve on this channel either. (The
        // settled state IS readable if the exception is caught — that is how CF2-R
        // measured the offline Rust==r3723 full-state identity — but this
        // report-only channel does not implement exception-tolerant capture; the
        // mandatory abort contract lives in `run_and_compare_abort`.)
        if c.expect_solve_abort.is_some() {
            continue;
        }
        let mut c = c;
        c.compare_all_properties = false;
        universe.push((format!("solvable_now:{}", c.path), corpus_file(&c.path), c));
    }
    for fam in [&ASYMMETRIC, &CONTROLS, &MODES] {
        for c in load_family(fam.name) {
            if c.pending || c.expect_solve_abort.is_some() {
                continue;
            }
            if c.oracle.is_some() {
                target_rev_excluded.push(format!("{}:{}", fam.name, c.path));
                continue;
            }
            let abs = family_file(fam.name, &c.path);
            let mut c = c;
            c.compare_all_properties = false;
            universe.push((format!("{}:{}", fam.name, c.path), abs, c));
        }
    }
    if !target_rev_excluded.is_empty() {
        eprintln!(
            "opendss {rev}: {} target-rev case(s) excluded \
             (gated in the mandatory gate against their own `oracle` target)",
            target_rev_excluded.len()
        );
    }
    // As WPs flip cases to target revs the swept universe shrinks; an EMPTY
    // sweep would make ASSERT mode pass vacuously — flag it loudly (the
    // report below also records the exclusions, so the artifact is honest).
    if universe.is_empty() {
        eprintln!(
            "opendss {rev}: WARNING swept universe is EMPTY ({} case(s) excluded as \
             target-rev) — the sweep is vacuous; rely on the mandatory gate",
            target_rev_excluded.len()
        );
    }

    let catalog = load_known_diffs();
    let mut hits: Vec<(String, usize)> = catalog.iter().map(|e| (e.id.clone(), 0)).collect();

    let mut matched: Vec<String> = Vec::new();
    let mut diverged: Vec<(String, String)> = Vec::new();
    let mut skipped: Vec<(String, String)> = Vec::new(); // label, entry id
    let total = universe.len();
    for (i, (label, abs, c)) in universe.iter().enumerate() {
        // `skip` entries match on the case alone (the case is not expected to
        // run/converge on this revision) — never attempted, reported apart.
        if let Some(pos) = catalog.iter().position(|e| {
            e.kind == "skip" && e.revs.iter().any(|r| r == &rev) && label.contains(&e.case_contains)
        }) {
            hits[pos].1 += 1;
            skipped.push((label.clone(), catalog[pos].id.clone()));
            continue;
        }
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
    skipped.sort();

    // Partition against the triage catalog: first matching entry (by file
    // order) wins; unmatched divergences are NEW and fail assert mode.
    let mut known: Vec<(String, String, String)> = Vec::new(); // label, reason, entry id
    let mut fresh: Vec<(String, String)> = Vec::new();
    for (label, reason) in &diverged {
        let hit = catalog.iter().position(|e| {
            e.kind == "diff"
                && e.revs.iter().any(|r| r == &rev)
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
        "known_skipped": skipped
            .iter()
            .map(|(label, id)| serde_json::json!({ "path": label, "known": id }))
            .collect::<Vec<_>>(),
        "known_hits": hits
            .iter()
            .map(|(id, n)| serde_json::json!({ "id": id, "hits": n }))
            .collect::<Vec<_>>(),
        // Target-rev cases removed from this sweep (gated in the mandatory
        // gate against their own `oracle` target) — recorded so the artifact
        // explains its own shrunken `total` (audit WP-U0).
        "target_rev_excluded": target_rev_excluded,
    });
    let rp: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "..", "tmp"]
        .iter()
        .collect::<PathBuf>()
        .join(format!("opendss_report_{rev}.json"));
    let _ = std::fs::create_dir_all(rp.parent().unwrap());
    std::fs::write(&rp, serde_json::to_string_pretty(&report).unwrap())
        .unwrap_or_else(|e| panic!("write {}: {e}", rp.display()));
    eprintln!(
        "opendss {rev}: {} matched, {} known-diverged, {} known-skipped, {} NEW (of {total}); \
         report -> {}",
        matched.len(),
        known.len(),
        skipped.len(),
        fresh.len(),
        rp.display()
    );
    if assert_mode && !fresh.is_empty() {
        panic!(
            "opendss {rev}: {} NEW divergence(s) from the EPRI engine not covered by \
             tests/corpus/known_diffs.json (DSS_LIVE_OPENDSS_ASSERT=1); see {}",
            fresh.len(),
            rp.display()
        );
    }
}

// ===========================================================================
// WP-AD.4 — the corpus-wide A-Diakoptics <-> normal sweep.
//
// Rust-vs-rust (the oracle is not involved), so `corpus_ad_matches_normal_mode`
// runs everywhere `cargo test` runs. Every `solvable_now` entry point carries an
// explicit A-Diakoptics disposition in `manifests/ad_sweep.json`; eligible decks
// (`pf`/`full`) are solved both normally and with the ckt24 AD preamble
// (`set Num_SubCircuits=2` + snapshot base solve + `set ADiakoptics=yes`), then
// their coordinator node voltages are compared name-keyed at the AD tier. A
// `pf`/`full` deck whose AD init FAILS at runtime is a test failure (the
// disposition is a promise). Torn_Circuit lands in a per-case temp datapath,
// never next to the vendored deck.
// ===========================================================================

use num_complex::Complex64;

/// The closed set of `off:` reasons a disposition may carry. Keeping this an
/// allowlist (rather than "any non-empty string") is what makes a real AD-engine
/// bucket distinguishable from a legitimate exclusion at the manifest level: a
/// new deck cannot invent an unreviewed `off:reason` to dodge the sweep, and the
/// `ad-*` classes stay a bounded, greppable, STATUS-documented list. Extend this
/// ONLY with a reason that is itself evidence-backed (DSS_AD_CLASSIFY +
/// DSS_AD_DECOMPOSE) and recorded.
const AD_OFF_REASONS: &[&str] = &[
    // Eligibility / topology (deck cannot be AD-swept by construction).
    "non-3ph-cut-only", // D5 ZLL: only cut candidates are non-3-phase lines/xfmrs
    "too-small",        // Tear_Circuit cannot form two connected >=2-bus zones
    "already-torn-artifact", // a pre-torn Torn_Circuit/zone master, not a top entry
    "mode-outside-AD-scope", // dynamics/harmonics/faultstudy/monte/LD - not power-flow
    "deck-aborts-by-design", // the deck's own solve aborts (e.g. #485 control-limit)
    // Save round-trip (D7 leg1): AD leg proper is CLEAN, the gap is the reload.
    "save-roundtrip-geometry",
    "save-roundtrip-relpath",
    "save-roundtrip-userdll",
    "save-roundtrip-regxfmr",
    "save-roundtrip-autotrans",
    "save-roundtrip-relay",
    "save-roundtrip-control",
    // Upstream A-Diakoptics limitations (NOT a dss-rs port bug). Root-caused in
    // the WP-AD.4 closing round + settle (ad-bugs branch): each of these topologies
    // makes Tear_Circuit isolate a zone that lacks an adequate in-zone voltage
    // reference, so the child `hY` is singular / near-singular and the boundary
    // stitch is solver-dependent. Shown upstream by driving OFFICIAL r3723 AD
    // (Oddie) on the IDENTICAL cut (child cut forced with `set LinkBranches` +
    // UseMyLinkBranches): on every one of the 6 representatives driven, official
    // OpenDSS AD fails the same or worse (no convergence / >10x over-voltage / a
    // hang on the singular zone). The magnitudes are ill-conditioned near-singular
    // blow-ups (reproducible on the reference box, environment-sensitive across
    // hosts) — the invariant is the CHARACTER, not the number. See the STATUS §1
    // evidence table. Remaining class members are inferred from the shared
    // mechanism, not individually driven; the per-deck official-AD replay is the
    // tracked WP-AD.5 task. Kept off the gate because there is no correct AD answer
    // to gate against on either engine, not because fixing our engine was out of
    // scope.
    "ad-regulator-divergence", // delta-only / floating-phase load bus -> singular
    // child Y: ODRegTest (delta loads, both -> ~1e16 @ loadbus); TestDDRegulator
    // (floating phases 2,3 -> official AD solve hangs, ours 0.88)
    "ad-switched-divergence", // meshed / open-switch topology: a single link cut
    // cannot separate a mesh (civanlar), or open switches strand a regulator
    // boundary (IEEE123Switches, both engines -> tens of kV on a 2.4 kV bus)
    "ad-islanded-divergence", // GFM/GFL microgrid island driven only by PC current
    // injection (no in-zone Y reference); the one ISource deck diverges in the torn
    // main-feeder xfmr-secondary zone (island itself determinate), ours milder than
    // official's same-cut AD (0.66 vs 13.4)
    "ad-nonconvergent", // GFM/GFL island whose torn zone is singular enough that
    // AD init/solve does not converge at all (our engine refuses; official blows up)
    "ad-singular-zone", // synthesized family deck: an intentionally singular tear
    "ad-divergent",     // synthesized family deck: an intentionally divergent tear
    "ad-floor-above-tier", // genuine long-radial stitch floor just above the tier
    // (NOT tolerance-widened, section 5) — IEEE34Mod1 leg2=2.1e-3
    // Decks added AFTER the WP-AD.4 sweep (upgrade-era skipped-sweep promotions,
    // GFM/DynExp re-promotions, windgen + U1.3 invcontrol + coverage-wave family
    // decks) whose DSS_AD_CLASSIFY/DSS_AD_DECOMPOSE classification has not run
    // yet. An explicit "pending" bucket recorded at the part2->update integration
    // merge (2026-07-12, STATUS note) — NOT a measured verdict; the follow-up
    // classification round re-classifies these and retires the reason.
    "unclassified-new-deck",
];

/// A valid `ad` disposition is `full`, `pf`, or `off:<reason>` where `reason` is
/// one of the reviewed [`AD_OFF_REASONS`] (not merely any non-empty string).
fn ad_disposition_is_valid(s: &str) -> bool {
    s == "full"
        || s == "pf"
        || s.strip_prefix("off:")
            .is_some_and(|r| AD_OFF_REASONS.contains(&r))
}

#[derive(Debug, Clone, Deserialize)]
struct AdSweepCase {
    path: String,
    ad: String,
    #[serde(default)]
    oracle: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdSweepManifest {
    #[serde(default)]
    cases: Vec<AdSweepCase>,
}

fn load_ad_sweep() -> Vec<AdSweepCase> {
    let p = manifests_dir().join("ad_sweep.json");
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let m: AdSweepManifest =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()));
    m.cases
}

/// The AD-sweep node-voltage comparison ceiling (WP-AD.4; recorded in
/// `tests/TOLERANCE_NOTES.md` section AD). A single conservative rust-vs-rust tier
/// that every eligible corpus deck's AD-vs-normal snapshot gap clears empirically
/// -- far below any physical significance, so a real port bug blows past it. A
/// deck whose measured gap exceeds this is classified `off:` with a specific
/// reason, NEVER tolerance-widened (section 5 no-fudging). The synthesized-fixture
/// D7 tiers (`adiakoptics.rs`) stay their own tighter, individually-calibrated
/// values.
const AD_SWEEP_TIER: f64 = 2.0e-3;

/// Coordinator node-name -> complex voltage (the AD coordinator may reorder buses;
/// names are stable, so the compare is name-keyed like `adiakoptics.rs`).
fn ad_node_voltages(dss: &Dss) -> BTreeMap<String, Complex64> {
    let ckt = dss.circuit().expect("circuit");
    (1..=ckt.num_nodes)
        .map(|i| (ckt.node_name(i), ckt.solution.node_v[i]))
        .collect()
}

/// Worst relative node-voltage gap between two name->V maps, and the count of
/// nodes actually compared (a near-zero count would mean the two runs share no
/// node names -- a mapping bug, guarded by the caller).
fn ad_max_rel_gap(
    a: &BTreeMap<String, Complex64>,
    b: &BTreeMap<String, Complex64>,
) -> (f64, String, usize) {
    let mut worst = 0.0;
    let mut wn = String::new();
    let mut matched = 0usize;
    for (name, va) in a {
        if let Some(vb) = b.get(name) {
            matched += 1;
            let dv = (va - vb).norm();
            let base = va.norm();
            let rel = if base > 1e-6 { dv / base } else { dv };
            if rel > worst {
                worst = rel;
                wn = name.clone();
            }
        }
    }
    (worst, wn, matched)
}

/// Per-case temp datapath (Torn_Circuit + any export lands here, never next to
/// the vendored deck). Distinct per process + thread + tag.
fn ad_scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "dss_adsweep_{tag}_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// Compile + snapshot-solve a corpus deck on the plain (non-AD) engine. `pf`
/// forces `controlmode=off`; `full` keeps the deck's control mode.
fn ad_solve_normal(abs: &str, controls_off: bool) -> Result<Dss, String> {
    let scratch = ad_scratch("norm");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{abs}\""));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    if controls_off {
        dss.command("set controlmode=off");
    }
    dss.command("solve mode=snap");
    let ckt = dss.circuit().ok_or_else(|| "no circuit".to_string())?;
    if !ckt.is_solved {
        return Err(format!(
            "normal snapshot did not converge: {}",
            dss.result()
        ));
    }
    Ok(dss)
}

/// Compile + apply the A-Diakoptics preamble to a corpus deck. Returns the driven
/// coordinator `Dss` on success, or an `Err(reason)` categorizing an AD-init
/// failure (the tear/partition/matrix stage that refused). `pf` forces
/// `controlmode=off` (children inherit it at init); `full` re-asserts the deck's
/// control mode after init so `GETCTRLMODE` propagates it to the children.
fn ad_solve_ad(abs: &str, controls_off: bool) -> Result<Dss, String> {
    let scratch = ad_scratch("ad");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{abs}\""));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    if controls_off {
        dss.command("set controlmode=off");
    }
    // Base snapshot solve (Tear_Circuit reads NodeV at each point of connection).
    dss.command("solve mode=snap");
    if !dss.circuit().is_some_and(|c| c.is_solved) {
        return Err(format!("base snapshot did not converge: {}", dss.result()));
    }
    dss.command("set Num_SubCircuits=2");
    let base_errs = dss.errors().len();
    dss.command("set ADiakoptics=True");
    if !dss.circuit().is_some_and(|c| c.solution.adiakoptics) {
        // Categorize the tear/AD-init refusal from the emitted message.
        let msg = dss
            .errors()
            .get(base_errs)
            .cloned()
            .unwrap_or_else(|| dss.result().to_string());
        return Err(format!("ad-init: {msg}"));
    }
    if !controls_off {
        // GETCTRLMODE: re-assert the deck's OWN declared control mode (not a
        // hardcoded `static`) so the children run the same loop the normal arm
        // does. The normal `full` arm keeps the deck's mode; forcing `static`
        // here would compare a non-static deck under two different modes.
        let mode = dss
            .circuit()
            .map(|c| c.solution.default_control_mode)
            .unwrap_or(0);
        let mode_cmd = match mode {
            -1 => "off",
            1 => "event",
            2 => "time",
            _ => "static",
        };
        dss.command(&format!("set controlmode={mode_cmd}"));
    }
    dss.command("solve mode=snap");
    if !dss.circuit().is_some_and(|c| c.is_solved) {
        return Err(format!("AD snapshot did not converge: {}", dss.result()));
    }
    Ok(dss)
}

/// Compare one `pf`/`full` case's AD solve against its normal solve at the AD
/// tier (rust-vs-rust). An AD-init failure is a hard test failure (the
/// disposition is a promise). `abs` is the resolved deck path (corpus or family);
/// `label`/`ad` are for the message.
///
/// The compared quantity is node voltages by name. Node-V equality is the
/// *sufficient* physics check on the shared interconnected network: every element
/// shared between the two arms carries the same primitive `Yprim`, so its
/// terminal currents `I = Yprim·Vterminal` and powers `S = V·conj(I)` are fixed
/// once the node voltages agree — a stitch error that left every voltage right
/// but a flow wrong is not physically realizable for a shared element. (Verified
/// empirically: an element-power cross-check over every `pf` deck tracked the
/// node-V gap and revealed no independent divergence; its only residuals above
/// the node-V floor were transformer/line **loss** channels on the short-circuit
/// decks — a small difference of large terminal flows, worst 2.2e-2 on
/// `ieee37_SC_Currents` `line.l6` — i.e. the documented cancellation-floor class,
/// not a stitch error. The AD arm's only element-set difference is the extra
/// link-cut boundary `VSource`/`ISource`, which have no normal-arm counterpart.
/// The active-control / eventlog channel is gated separately by
/// `adiakoptics.rs::full_zone_local_regcontrol_matches_normal`.)
fn ad_run_case_abs(abs: &str, label: &str, ad: &str, full: bool) {
    let controls_off = !full;
    let _guard = CorpusGuard::new(abs);
    let vn = ad_node_voltages(
        &ad_solve_normal(abs, controls_off)
            .unwrap_or_else(|e| panic!("AD sweep {label}: normal arm failed: {e}")),
    );
    let va = ad_node_voltages(&ad_solve_ad(abs, controls_off).unwrap_or_else(|e| {
        panic!("AD sweep {label}: `{ad}` disposition but AD init/solve failed: {e}")
    }));
    let (gap, node, matched) = ad_max_rel_gap(&vn, &va);
    assert!(
        matched >= vn.len().saturating_sub(vn.len() / 20).max(1),
        "AD sweep {label}: only {matched}/{} nodes matched by name (mapping bug)",
        vn.len()
    );
    assert!(
        gap < AD_SWEEP_TIER,
        "AD sweep {label} ({ad}): AD-vs-normal node-V gap {gap:.3e} @ {node} exceeds \
         the AD tier {AD_SWEEP_TIER:.1e} -- classify off with a specific reason, \
         do not widen (section 5)"
    );
}

fn ad_classify_enabled() -> bool {
    std::env::var("DSS_AD_CLASSIFY")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// `DSS_AD_DECOMPOSE=<rel-path>` throwaway probe: split the AD-vs-normal gap into
/// the D7 two legs — (1) `save circuit` fidelity: saved
/// `Master_Interconnected.dss` solved normally vs the ORIGINAL solved normally;
/// (2) the AD leg proper: AD solve vs the saved-interconnected normal solve. Not
/// a gate — a diagnostic to attribute an `off:ad-gap` deck's gap.
#[test]
fn ad_decompose_probe() {
    let Ok(rel) = std::env::var("DSS_AD_DECOMPOSE") else {
        eprintln!("SKIPPED ad_decompose: set DSS_AD_DECOMPOSE=<corpus-rel-path>");
        return;
    };
    let abs = corpus_file(&rel);
    // Keep the vendored corpus pristine (the compile-time solve writes reports).
    let _guard = CorpusGuard::new(&abs);
    // Original normal.
    let vn = ad_node_voltages(&ad_solve_normal(&abs, true).expect("orig normal"));
    // AD arm — but keep the scratch dir so we can compile the interconnected save.
    let scratch = ad_scratch("decomp");
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!("compile \"{abs}\""));
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("set controlmode=off");
    dss.command("solve mode=snap");
    dss.command("set Num_SubCircuits=2");
    dss.command("set ADiakoptics=True");
    let inited = dss.circuit().is_some_and(|c| c.solution.adiakoptics);
    if inited {
        dss.command("solve mode=snap");
    }
    let va = ad_node_voltages(&dss);
    // Interconnected save solved normally.
    let inter = scratch
        .join("Torn_Circuit")
        .join("Master_Interconnected.dss");
    let mut di = Dss::new();
    di.command("clear");
    di.command(&format!(
        "compile \"{}\"",
        inter.display().to_string().replace('\\', "/")
    ));
    di.command("set controlmode=off");
    di.command("solve mode=snap");
    let solved_inter = di.circuit().is_some_and(|c| c.is_solved);
    let vi = ad_node_voltages(&di);
    let (leg1, n1, m1) = ad_max_rel_gap(&vn, &vi);
    let (leg2, n2, m2) = ad_max_rel_gap(&vi, &va);
    let (tot, nt, _) = ad_max_rel_gap(&vn, &va);
    eprintln!(
        "AD DECOMPOSE {rel}\n  inited={inited} inter_solved={solved_inter}\n  \
         leg1 save-roundtrip (orig-normal vs inter-normal) = {leg1:.3e} @ {n1} (matched {m1})\n  \
         leg2 AD (inter-normal vs AD)                       = {leg2:.3e} @ {n2} (matched {m2})\n  \
         total (orig-normal vs AD)                          = {tot:.3e} @ {nt}"
    );
}

/// The user-mandated sweep gate: every `pf`/`full` case is solved both ways and
/// compared; `off:` cases are counted only. Population counts + wall-time are
/// printed (STATUS records them). Under `DSS_AD_CLASSIFY=1` it instead PROBES
/// every case's pf-eligibility and prints a proposed disposition per deck.
#[test]
fn corpus_ad_matches_normal_mode() {
    let cases = load_ad_sweep();
    assert!(!cases.is_empty(), "ad_sweep.json must not be empty");
    for c in &cases {
        assert!(
            ad_disposition_is_valid(&c.ad),
            "{}: invalid ad disposition {:?}",
            c.path,
            c.ad
        );
    }
    if ad_classify_enabled() {
        ad_classify(&cases);
        return;
    }
    let start = Instant::now();
    // The sweep covers BOTH the vendored corpus (ad_sweep.json) and the three
    // synthesized family manifests (their mandatory `ad` field), per plan §WP-AD.4.
    let mut entries: Vec<(String, String, String)> = cases
        .iter()
        .map(|c| (corpus_file(&c.path), c.path.clone(), c.ad.clone()))
        .collect();
    for fam in ["asymmetric", "controls", "modes"] {
        for c in load_family(fam) {
            let ad = c
                .ad
                .clone()
                .unwrap_or_else(|| panic!("{fam}:{}: missing mandatory `ad` disposition", c.path));
            assert!(
                ad_disposition_is_valid(&ad),
                "{fam}:{}: invalid ad disposition {ad:?} (not in AD_OFF_REASONS)",
                c.path
            );
            entries.push((family_file(fam, &c.path), format!("{fam}/{}", c.path), ad));
        }
    }
    let total = entries.len();
    let (mut full, mut pf, mut off) = (0usize, 0usize, 0usize);
    for (abs, path, ad) in &entries {
        match ad.as_str() {
            "full" => {
                ad_run_case_abs(abs, path, ad, true);
                full += 1;
            }
            "pf" => {
                ad_run_case_abs(abs, path, ad, false);
                pf += 1;
            }
            _ => off += 1,
        }
    }
    eprintln!(
        "AD sweep: full={full} pf={pf} off={off} (of {total}: {} corpus + {} family) in {:.1}s",
        cases.len(),
        total - cases.len(),
        start.elapsed().as_secs_f64()
    );
}

/// Probe the three family manifests' decks the same way (via `DSS_AD_CLASSIFY=1`
/// on `family_manifest_is_complete`-adjacent path resolution). Prints the same
/// `ADCLASSIFY` lines with a `<family>/` path prefix.
#[test]
fn ad_classify_families() {
    if !ad_classify_enabled() {
        eprintln!("SKIPPED ad_classify_families: set DSS_AD_CLASSIFY=1");
        return;
    }
    let start = Instant::now();
    for fam in ["asymmetric", "controls", "modes"] {
        for c in load_family(fam) {
            // pending / abort decks can't be AD-swept — report them so.
            if c.pending {
                println!(
                    "ADCLASSIFY\t{fam}/{}\toff:pending-feature\tNaN\tpending",
                    c.path
                );
                continue;
            }
            if c.expect_solve_abort.is_some() {
                println!(
                    "ADCLASSIFY\t{fam}/{}\toff:expect-solve-abort\tNaN\tabort",
                    c.path
                );
                continue;
            }
            let abs = family_file(fam, &c.path);
            let path = format!("{fam}/{}", c.path);
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _guard = CorpusGuard::new(&abs);
                let vn = ad_solve_normal(&abs, true).map(|d| ad_node_voltages(&d));
                let va = ad_solve_ad(&abs, true).map(|d| ad_node_voltages(&d));
                (vn, va)
            }));
            let (proposal, gap, detail) = ad_classify_outcome(res);
            println!("ADCLASSIFY\t{path}\t{proposal}\t{gap:.3e}\t{detail}");
        }
    }
    eprintln!(
        "AD classify families in {:.1}s",
        start.elapsed().as_secs_f64()
    );
}

/// Shared outcome categorizer for the classify probes.
#[allow(clippy::type_complexity)]
fn ad_classify_outcome(
    res: std::thread::Result<(
        Result<BTreeMap<String, Complex64>, String>,
        Result<BTreeMap<String, Complex64>, String>,
    )>,
) -> (String, f64, String) {
    match res {
        Err(e) => ("off:probe-panic".to_string(), f64::NAN, panic_msg(e)),
        Ok((Err(e), _)) => ("off:normal-fail".to_string(), f64::NAN, e),
        Ok((_, Err(e))) => {
            let cls = if e.contains("not lines") || e.contains("not a line") {
                "off:non-3ph-cut-only"
            } else if e.contains("error when tearing")
                || e.contains("cannot be compiled")
                || e.contains("Sub-Circuits")
                || e.contains("zone")
            {
                "off:too-small"
            } else {
                "off:ad-init-fail"
            };
            (cls.to_string(), f64::NAN, e)
        }
        Ok((Ok(vn), Ok(va))) => {
            let (gap, node, _matched) = ad_max_rel_gap(&vn, &va);
            let prop = if gap < AD_SWEEP_TIER {
                "pf"
            } else {
                "off:ad-gap"
            };
            (prop.to_string(), gap, node)
        }
    }
}

/// `DSS_AD_CLASSIFY=1` probe: attempt the pf AD preamble on every case and print
/// `ADCLASSIFY<TAB>path<TAB>PROPOSAL<TAB>gap<TAB>detail`, so a disposition can be
/// assigned from evidence.
fn ad_classify(cases: &[AdSweepCase]) {
    let start = Instant::now();
    for c in cases {
        let path = c.path.clone();
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let abs = corpus_file(&path);
            let _guard = CorpusGuard::new(&abs);
            let vn = ad_solve_normal(&abs, true).map(|d| ad_node_voltages(&d));
            let va = ad_solve_ad(&abs, true).map(|d| ad_node_voltages(&d));
            (vn, va)
        }));
        let (proposal, gap, detail) = match res {
            Err(e) => ("off:probe-panic".to_string(), f64::NAN, panic_msg(e)),
            Ok((Err(e), _)) => ("off:normal-fail".to_string(), f64::NAN, e),
            Ok((_, Err(e))) => {
                let cls = if e.contains("not lines") || e.contains("not a line") {
                    "off:non-3ph-cut-only"
                } else if e.contains("error when tearing")
                    || e.contains("cannot be compiled")
                    || e.contains("Sub-Circuits")
                    || e.contains("zone")
                {
                    "off:too-small"
                } else {
                    "off:ad-init-fail"
                };
                (cls.to_string(), f64::NAN, e)
            }
            Ok((Ok(vn), Ok(va))) => {
                let (gap, node, _matched) = ad_max_rel_gap(&vn, &va);
                let prop = if gap < AD_SWEEP_TIER {
                    "pf"
                } else {
                    "off:ad-gap"
                };
                (prop.to_string(), gap, node)
            }
        };
        println!("ADCLASSIFY\t{path}\t{proposal}\t{gap:.3e}\t{detail}");
    }
    eprintln!(
        "AD classify: {} cases probed in {:.1}s",
        cases.len(),
        start.elapsed().as_secs_f64()
    );
}

/// WP-AD.4 bijection: `ad_sweep.json` must carry a disposition for EXACTLY the
/// `solvable_now.json` entry-point set -- no missing case (a new solvable deck
/// cannot skip classification), no extra path. Oracle-free, always-on.
#[test]
fn ad_sweep_covers_solvable_now() {
    let solvable: BTreeSet<String> = load_solvable()
        .into_iter()
        .map(|c| c.path.replace('\\', "/"))
        .collect();
    let sweep = load_ad_sweep();
    let swept: BTreeSet<String> = sweep.iter().map(|c| c.path.replace('\\', "/")).collect();
    assert_eq!(swept.len(), sweep.len(), "duplicate paths in ad_sweep.json");
    let missing: Vec<&String> = solvable.difference(&swept).collect();
    assert!(
        missing.is_empty(),
        "{} solvable_now path(s) missing an AD disposition in ad_sweep.json:\n  {}",
        missing.len(),
        missing
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    let extra: Vec<&String> = swept.difference(&solvable).collect();
    assert!(
        extra.is_empty(),
        "{} ad_sweep.json path(s) are not solvable_now entry points:\n  {}",
        extra.len(),
        extra
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
    for c in &sweep {
        assert!(
            ad_disposition_is_valid(&c.ad),
            "{}: invalid ad disposition {:?} (full|pf|off:<reason>)",
            c.path,
            c.ad
        );
        if let Some(spec) = &c.oracle {
            assert!(
                ORACLE_SPECS.contains(&spec.as_str()),
                "{}: unknown oracle spec {spec:?}",
                c.path
            );
        }
    }
    eprintln!(
        "ad_sweep.json: {} dispositions cover solvable_now exactly",
        sweep.len()
    );
}
