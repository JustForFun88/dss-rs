//! The OpenDSS engine transports of the corpus gate (two gating channels).
//! Every transport speaks the same line-JSON protocol (`ping`/`run`/`quit`, one
//! compact JSON object per line) and produces the same [`CaseResult`] shape for
//! the `harness` comparators:
//!
//! * `capi_v0145` channel: the pinned dss-python 0.15.7 / dss_capi 0.14.5 via
//!   `tools/oracle/oracle_server.py`. Served by a persistent [`WorkerPool`],
//!   serial/isolate cases use the one-shot [`Oracle`].
//! * `r4133` channel: the official EPRI `OpenDSSDirect.dll` r4133 via the
//!   in-house `epri-worker` bridge. Served by a persistent [`EpriPool`],
//!   serial/isolate cases use the one-shot [`EpriOneShot`].
//!
//! [`Oracle`] also backs the opt-in report tests (`corpus_live_classify` /
//! `_properties`). [`Channel`] unifies all four transports for the scheduler.

use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::harness::aggregates::{AggregatesCap, SolutionScalarsCap};
use crate::harness::inc_matrix::IncMatrixCap;
use crate::harness::topology::TopologyCap;
use crate::harness::{
    BusCap, ElementCap, Injection, MeterCap, MonitorCap, PdElementCap, ProbeCap, PropsCap,
    ReliabilityCap, VariablesCap, YFingerprint, YMat, YPrim,
};
use crate::manifest::SolvableCase;

// ---------------------------------------------------------------------------
// Response + per-case model (deserialized from the engine's JSON).
// ---------------------------------------------------------------------------

/// One engine response: `{ok, error?, result?}`.
#[derive(Debug, Deserialize)]
pub(crate) struct Resp {
    pub(crate) ok: bool,
    #[serde(default)]
    pub(crate) error: Option<String>,
    #[serde(default)]
    pub(crate) result: Option<Value>,
}

/// The per-case model the engine returns (mirrors the Rust capture).
#[derive(Debug, Deserialize)]
pub(crate) struct CaseResult {
    pub(crate) node_order: Vec<String>,
    pub(crate) n_steps: usize,
    pub(crate) checkpoints: Vec<Checkpoint>,
    #[serde(default)]
    pub(crate) autoadd_log: Option<String>,
    /// The run-produced FILE SET: every
    /// filesystem entry the run created under the case dir, `/`-joined,
    /// ASCII-case-folded, sorted, a trailing `/` marking a created directory
    /// (`dss_epri::guard::normalize_created_name`). Run-level, not per step —
    /// the sibling of [`Self::autoadd_log`], not of a [`Checkpoint`] field —
    /// because a file is created by the whole run, not by one solve.
    ///
    /// Flag-gated (`SolvableCase::compare_run_files`), so `None` is the honest
    /// reply when the case did not request it; the presence rail
    /// (`harness::capture_guard::require_capture_opt`) turns `None` into a
    /// failure exactly when the flag IS on. `Some([])` — "asked, and this deck
    /// created nothing" — is the common, legitimate answer.
    #[serde(default)]
    pub(crate) run_files: Option<Vec<String>>,
    /// The created-file names (normalized and sorted like [`Self::run_files`])
    /// whose CONTENTS the transport copied into this case's sidecar directory
    /// ([`run_file_contents_dir`]). The bytes travel on disk, not in this
    /// reply: the selection reaches ~19 MB per channel per full drive.
    ///
    /// `None` = the request carried no selection, so the presence rail
    /// (`harness::capture_guard::require_capture_opt`) can tell that apart from
    /// `Some([])` — "asked, and this deck wrote none of the selected reports",
    /// the common answer. The gate asserts the sidecar holds exactly these
    /// names, so a copy that lost one fails the case rather than comparing a
    /// smaller set (`harness::run_files::read_sidecar`).
    #[serde(default)]
    pub(crate) run_file_contents: Option<Vec<String>>,
    /// The run-created demand-interval tree's
    /// CONTENTS, as `normalized member name -> path relative to the request's
    /// sidecar directory` ([`di_sidecar_dir`]). The files themselves are copied
    /// there by the transport, because the channel's own `CorpusGuard` sweeps
    /// the originals away before this gate ever reads them.
    ///
    /// Flag-gated (`SolvableCase::compare_di`) exactly like [`Self::run_files`]:
    /// `None` is the honest reply when the case did not request it, and the
    /// presence rail (`harness::capture_guard::require_capture_opt`) turns it
    /// into a failure exactly when the flag IS on. `Some({})` — "asked, and this
    /// deck wrote no DI tree" — is the common, legitimate answer.
    #[serde(default)]
    pub(crate) di: Option<std::collections::BTreeMap<String, String>>,
    /// The created entries the channel's OWN hygiene guard could not remove
    /// (`tools/oracle/corpus_guard.py::CorpusGuard.sweep_failed`,
    /// `dss_epri::guard::CorpusGuard::finish`), normalized like
    /// [`Self::run_files`].
    ///
    /// Not flag-gated: every transport reports it on every case, because a
    /// dropping one producer leaves behind is in the NEXT producer's pre-run
    /// snapshot and silences that name in its created set, which is how
    /// dss_capi's never-closed Storage trace stream can make the `r4133`
    /// channel look green. [`crate::runner::assert_swept_clean`] fails the case
    /// on a non-empty list, naming the producer.
    ///
    /// `Option`, not a defaulted `Vec<String>`: a reply that stopped emitting
    /// the key would deserialize to `[]` and disarm the sweep rail in silence.
    /// `None` is a broken transport and fails the case, `Some([])` the clean
    /// sweep.
    #[serde(default)]
    pub(crate) sweep_failed: Option<Vec<String>>,
    /// The message of the exception the transport's own teardown `clear`
    /// raised, or `None` when it completed. The pinned dss_capi 0.14.5 faults
    /// on that second `clear` on the two AutoAdd decks
    /// (`tools/oracle/oracle_server.py::run_case`), and the whole compared
    /// surface is captured BEFORE the teardown, so this is a note, not a
    /// failure: [`crate::runner::compare_with_result`] prints it and compares
    /// the case normally. The transport that raised exits after replying, so a
    /// pooled worker is respawned rather than reused
    /// ([`WorkerPool::checkin`]). The `r4133` transport issues no teardown
    /// `clear` (r4133 closes its own trace files) and therefore always omits
    /// the key — `serde(default)` makes that `None`.
    #[serde(default)]
    pub(crate) teardown_error: Option<String>,
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
    /// The `PDElements` interface walk. `Option`, not `Vec`: some walked live
    /// cases legitimately hold no PD element, so `None` ("the channel was not
    /// asked / did not answer") must
    /// stay distinguishable from `Some([])` ("asked, this circuit has none").
    /// Both transports emit exactly that (`oracle_server.py` sends `null` with
    /// the flag off; `dss-epri`'s `Checkpoint.pd_elements` is the same
    /// `Option`), and `capture_guard::require_capture_opt` is what turns the
    /// first shape into a case failure.
    #[serde(default)]
    pub(crate) pd_elements: Option<Vec<PdElementCap>>,
    /// The `Meters` reliability payload, carried by the
    /// LAST checkpoint only — `RelCalc` runs once per case, after the last
    /// solve, on all three engines (it is not idempotent). `Option` for the
    /// [`super::harness::capture_guard::require_capture_opt`] reason
    /// [`Self::pd_elements`] is one, with the extra step dimension: `None`
    /// means "not requested, or not this step", `Some` with an empty `meters`
    /// list means "requested, and this circuit has no enabled meter".
    #[serde(default)]
    pub(crate) reliability: Option<ReliabilityCap>,
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
    /// The five `Circuit` aggregates. Optional
    /// in the schema only so a transport that predates the surface deserializes;
    /// the comparator DEMANDS it on every live case (the surface is unflagged
    /// and universal), so `None` fails the case rather than skipping it.
    #[serde(default)]
    pub(crate) aggregates: Option<AggregatesCap>,
    /// The ten `Solution` scalars, same
    /// presence contract as [`Checkpoint::aggregates`].
    #[serde(default)]
    pub(crate) solution_scalars: Option<SolutionScalarsCap>,
    /// Every bus's voltage surface, in the engine's `BusList` order. The other
    /// per-bus arms extend the same walk rather than taking a checkpoint slot of
    /// their own: `Distance`, the six short-circuit arrays (behind the `zsc`
    /// request field), the sequence/line-to-line pairs and the two at-bus name
    /// lists (read last because they are the r4133 channel's only
    /// `ModeEffect::Impure` bus reads). All of them ride the one `compare_bus`
    /// flag; `crates/dss-core/tests/corpus_gate.rs` pins that from both
    /// transports' source text.
    #[serde(default)]
    pub(crate) buses: Vec<BusCap>,
    /// `Circuit.AllBusVmagPu`: every node's per-unit voltage magnitude, in
    /// bus-list order x the bus's internal node index (the `AllNodeNames`
    /// permutation, which is neither the per-bus ascending-node-number order of
    /// [`BusCap`] nor the gated `YNodeOrder`). r4133 names it
    /// `Circuit.AllBusMagPu`.
    #[serde(default)]
    pub(crate) all_bus_vmag_pu: Vec<f64>,
    /// `Circuit.AllBusDistances`: each bus's `DistFromMeter` in km, `BusList`
    /// order, the same sequence [`Checkpoint::buses`] walks. Rides the
    /// `compare_bus` flag on both transports.
    #[serde(default)]
    pub(crate) all_bus_distances: Vec<f64>,
    /// `Circuit.AllNodeDistances`: the owning bus's `DistFromMeter` repeated
    /// once per node, in the same permutation as
    /// [`Checkpoint::all_bus_vmag_pu`] (bus x internal node index), not the
    /// per-bus ascending-node-number order.
    #[serde(default)]
    pub(crate) all_node_distances: Vec<f64>,
    /// The six order-free `ITopology` quantities.
    /// Unlike the aggregates and solution scalars, this surface is **flag-gated**
    /// (`SolvableCase::compare_topology`), so `None` is the honest reply when the
    /// case did not request it; the comparator's presence rail
    /// (`harness::capture_guard::require_capture_opt`) turns `None` into a
    /// failure exactly when the flag IS on.
    ///
    /// Both transports read it **strictly last** in a step — the first
    /// `Topology` read builds the memoized tree and rewrites the checked and
    /// isolated flags on every element, and
    /// `crates/dss-core/tests/capture_order.rs` asserts that from their source.
    #[serde(default)]
    pub(crate) topology: Option<TopologyCap>,
    /// The four flat incidence quantities
    /// (`IncMatrix`, `Laplacian`, `IncMatrixRows`, `IncMatrixCols`), read after
    /// one `CalcIncMatrix` + `CalcLaplacian` pair. Flag-gated exactly like
    /// [`Checkpoint::topology`] (`SolvableCase::compare_inc_matrix`), with the
    /// same presence contract: `None` is honest when the case did not request
    /// it, and `harness::capture_guard` turns `None` into a failure exactly when
    /// the flag IS on.
    ///
    /// Both transports read it **strictly last** in a step, after `topology`:
    /// building the matrix is a state write that on r4133 also moves the active
    /// circuit element (its series-reactor pass walks the class), and it must
    /// not precede the topology read that memoizes the branch list.
    /// `crates/dss-core/tests/capture_order.rs` asserts that from their source.
    #[serde(default)]
    pub(crate) inc_matrix: Option<IncMatrixCap>,
}

/// The per-case wall-clock deadline for a single request to an OpenDSS engine
/// (`DSS_OPENDSS_TIMEOUT_SECS`, default 600 s), shared by the one-shot and
/// pooled paths so a hanging case is recorded as a failure, not a stall.
pub(crate) fn oracle_timeout() -> Duration {
    Duration::from_secs(
        std::env::var("DSS_OPENDSS_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(600),
    )
}

/// The `oracle_server.py` `run` request for a case, the single builder both
/// transports share.
///
/// `deck` is what the producer compiles, its own scratch copy of the case
/// (`scratch::ScratchCopy`), while `case_path`, the
/// VENDORED deck, keys the gate's sidecars (`run_file_contents_dir`, and the
/// DI sidecar through the gate-side `sidecar_key` field the transports never
/// read), so a sidecar's location is the same for every copy of one case.
///
/// `deck`'s folder is what each transport's corpus guard
/// (`dss_epri::guard::CorpusGuard`, `corpus_guard.py::CorpusGuard`) brackets,
/// so a vendored `deck` is refused here, before any transport sees it
/// ([`crate::scratch::not_vendored`]).
pub(crate) fn build_run_request(deck: &str, case_path: &str, c: &SolvableCase) -> Value {
    crate::scratch::not_vendored(std::path::Path::new(deck), "engines::build_run_request");
    // The six short-circuit arms extend the one per-bus walk `buses` drives on
    // both transports, so `zsc` alone would ship an empty surface that
    // `require_capture` (not the comparator) would have to catch. Asserted
    // here, where the two flags meet, instead of or-ing them into the request
    // behind the manifest's back.
    assert!(
        c.compare_bus || !c.compare_zsc,
        "{case_path}: compare_zsc without compare_bus — the short-circuit arms \
         ride the bus walk; set both"
    );
    let probes: Vec<Value> = c
        .probes
        .iter()
        .map(|p| json!({"element": p.element, "props": p.props}))
        .collect();
    json!({
        "cmd": "run",
        "case_path": deck,
        "sidecar_key": case_path,
        "post": c.post,
        "n_steps": c.n_steps,
        "selected_elements": c.selected_elements,
        "full_csc": true,
        "check_meters_monitors": c.check_meters_monitors,
        "probes": probes,
        "variables": c.compare_variables,
        "eventlog": c.compare_eventlog,
        "ctrlqueue": c.compare_ctrlqueue,
        "buses": c.compare_bus,
        "zsc": c.compare_zsc,
        "all_properties": c.compare_all_properties,
        "pd_elements": c.compare_pdelements,
        "reliability": c.compare_reliability,
        // Both transports honor this key (`oracle_server.py` reads
        // `req["topology"]`, `dss-epri`'s `RunRequest::topology`) and both read
        // the surface strictly last in the step.
        "topology": c.compare_topology,
        // Same contract: both transports honor this key
        // (`oracle_server.py` reads `req["inc_matrix"]`, `dss-epri`'s
        // `RunRequest::inc_matrix`) and both drive the `CalcIncMatrix` +
        // `CalcLaplacian` pair and read its four quantities STRICTLY LAST in
        // the step, after `topology`.
        "inc_matrix": c.compare_inc_matrix,
        "global_result": c.compare_global_result,
        "autoadd_log": c.compare_autoadd_log,
        // Same contract: both transports honor this key
        // (`oracle_server.py` reads `req["run_files"]`, `dss-epri`'s
        // `RunRequest::run_files`) and both report the set from the very
        // `CorpusGuard` that sweeps the case dir, STRICTLY LAST of the run
        // (after `autoadd_log`, which itself reads a file off disk) and inside
        // the guard's scope.
        "run_files": c.compare_run_files,
        // The contents half of the same flag. The selection is computed
        // once here, on the gate side, and shipped as data — both transports
        // match it with one shared rule (`dss_epri::guard::selects_contents` and
        // its Python twin `tools/oracle/corpus_guard.py::selects_contents`) and
        // neither re-derives which files it must hand back. The bytes come back
        // through the gate-owned sidecar directory, never inline. Off ⇒ an
        // empty list and no directory, and the reply omits the key.
        "run_file_contents": if c.compare_run_files {
            dss_epri::guard::RUN_FILE_CONTENTS_PATTERNS.to_vec()
        } else {
            Vec::new()
        },
        "run_file_contents_dir": c
            .compare_run_files
            .then(|| run_file_contents_dir(case_path).to_string_lossy().into_owned()),
        // Same contract, one key honored by both transports
        // (`oracle_server.py` reads `req["di"]`, `dss-epri`'s
        // `RunRequest::di`). The sidecar directory the transports copy into
        // travels with it as `di_dir`, attached per CHANNEL by
        // [`attach_di_sidecar`] — a request built here is channel-agnostic.
        "di": c.compare_di,
        // The per-element derived polar channels. One key for the
        // whole `compare_derived` surface, honored by BOTH transports —
        // `tools/oracle/oracle_server.py::capture_all_elements` (capi_v0145) and
        // `crates/dss-epri/src/capture.rs::RunRequest::derived` (r4133) — so the
        // two channels can never disagree about what was requested. Absent ⇒ off
        // on both.
        "derived": c.compare_derived,
        // The per-element discrete index/name scalars
        // (`NumTerminals`/`NumConductors`/`NumPhases`/`EnergyMeter`/`NodeOrder`
        // plus `Enabled`). One key for the whole `compare_element_extras`
        // surface, honored by BOTH transports —
        // `tools/oracle/oracle_server.py::capture_all_elements` (capi_v0145)
        // and `crates/dss-epri/src/capture.rs::RunRequest::element_extras`
        // (r4133). Absent ⇒ off on both.
        "element_extras": c.compare_element_extras,
        "warn_and_continue": !c.expect_warnings.is_empty(),
    })
}

/// The gate-owned sidecar directory one case's run-file CONTENTS travel
/// through.
///
/// Under the gate's own scratch root ([`gate_scratch_root`]), never inside
/// `tests/corpus/` and never in the case directory: a byte the gate writes next
/// to a vendored deck would be a corpus dropping, and one inside the case dir
/// would land in the very created-file set the run-file surface compares.
///
/// One directory per CASE, not per channel, and the transport WIPES it before
/// copying: the scheduler runs a `both` case's channels strictly in sequence
/// (`corpus_gate::scheduler::run_one_case` fetches, compares and only then
/// moves to the next channel), and the gate deletes the directory as it reads
/// it ([`crate::harness::run_files::read_sidecar`]), so a file from the other
/// channel or an earlier case can never be read as this run's output — and if
/// one ever were, the name set the reply carries would not match the directory
/// and the case would fail loudly.
///
/// The key is [`case_key`] — the one sidecar naming convention this gate has,
/// shared with the demand-interval sidecar: the case path made filename-safe plus
/// a process-stable FNV-1a digest of the whole path, so two decks whose
/// sanitized spellings collide still get their own directory.
pub(crate) fn run_file_contents_dir(case_path: &str) -> PathBuf {
    gate_scratch_root()
        .join("run_files")
        .join(case_key(case_path))
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

/// A process exit code as the one-shot's "no JSON response" panic prints it:
/// decimal, plus hex and a name when it is a Windows NTSTATUS (the high bit set),
/// e.g. `-1073741819` is `0xc0000005 (access violation)`.
fn describe_exit_code(code: i32) -> String {
    let status = code as u32;
    if status & 0x8000_0000 == 0 {
        return format!("exit code {code}");
    }
    let name = match status {
        0xC000_0005 => " (access violation)",
        0xC000_001D => " (illegal instruction)",
        0xC000_0094 => " (integer divide by zero)",
        0xC000_00FD => " (stack overflow)",
        0xC000_0374 => " (heap corruption)",
        0xC000_0409 => " (fail-fast: stack buffer overrun or abort)",
        0xE046_5043 => " (unhandled Free Pascal exception)",
        _ => "",
    };
    format!("exit code {code} = {status:#010x}{name}")
}

// ---------------------------------------------------------------------------
// One-shot engine (fresh process per request).
// ---------------------------------------------------------------------------

/// A handle to the pinned dss-python driven ONE request per subprocess:
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
            // Pin the engine selector explicitly: an ambient
            // `DSS_ORACLE_ENGINE` must never re-bind the pinned 0.14.5 engine.
            envs: vec![("DSS_ORACLE_ENGINE", "capi".to_string())],
        }
    }

    /// Construct **and ping-verify** the pinned `capi_v0145` one-shot engine.
    /// `spec` must be `None` (the `r4133` channel gates through the
    /// [`EpriPool`]/[`EpriOneShot`]); a `Some(_)` is a caller bug.
    pub(crate) fn for_spec(spec: Option<&str>) -> Oracle {
        assert!(
            spec.is_none(),
            "Oracle::for_spec takes no revision — r4133 gates via the epri pool"
        );
        let o = Oracle::new();
        o.ping();
        o
    }

    /// One-shot request/response with a wall-clock timeout. stdout/stderr are
    /// drained on threads to avoid pipe deadlock and decoded lossily, so a byte
    /// that is not UTF-8 costs one character, not the stream. The first stdout
    /// line that parses as a `Resp` is the answer. A process that exits on its
    /// own without one panics with its exit status ([`describe_exit_code`]) and
    /// its whole stderr, where the server's `faulthandler` dump names the Python
    /// frame of a native fault. A process killed at the deadline is an
    /// `oracle timeout`.
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
            let mut b = Vec::new();
            let _ = so.read_to_end(&mut b);
            String::from_utf8_lossy(&b).into_owned()
        });
        let h_err = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = se.read_to_end(&mut b);
            String::from_utf8_lossy(&b).into_owned()
        });
        let deadline = Instant::now() + timeout;
        let exited = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        break None;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => panic!("oracle wait error: {e}"),
            }
        };
        let stdout = h_out.join().unwrap_or_default();
        let stderr = h_err.join().unwrap_or_default();
        let Some(status) = exited else {
            return Resp {
                ok: false,
                error: Some(format!("oracle timeout after {}s", timeout.as_secs())),
                result: None,
            };
        };
        for l in stdout.lines() {
            let t = l.trim();
            if t.is_empty() {
                continue;
            }
            if let Ok(r) = serde_json::from_str::<Resp>(t) {
                return r;
            }
        }
        let exit = status
            .code()
            .map_or_else(|| status.to_string(), describe_exit_code);
        panic!(
            "oracle produced no JSON response (the process exited on its own, {exit})\n\
             --- stdout ---\n{stdout}\n--- stderr ---\n{stderr}"
        );
    }

    fn ping(&self) {
        self.ping_engine();
    }

    /// Ping and assert the pinned-engine identity (no `oddie`/`capi015` marker —
    /// the retired EPRI/target-rev engines must never re-bind the default pinned
    /// 0.14.5 oracle) and that the server runs with `faulthandler` enabled, which
    /// [`Oracle::call`]'s no-JSON panic relies on. Returns the server-reported
    /// engine version.
    pub(crate) fn ping_engine(&self) -> String {
        let r = self.call(&json!({"cmd": "ping"}));
        assert!(r.ok, "oracle ping failed: {:?}", r.error);
        assert_eq!(
            r.result
                .as_ref()
                .and_then(|v| v.get("faulthandler"))
                .and_then(Value::as_bool),
            Some(true),
            "the oracle server must enable faulthandler before it serves a request: \
             {:?}",
            r.result
        );
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
        // The engine compiles its own fresh scratch copy of the case,
        // removed (or the case failed naming the producer) before this returns.
        let copy = crate::scratch::ScratchCopy::new(case_path, CAPI_TAG);
        // This one-shot is the `capi_v0145` transport, so the DI sidecar is the
        // capi one — the same attachment [`Channel::call`] performs for the
        // scheduler's four transports.
        let req = attach_di_sidecar(&build_run_request(copy.deck(), case_path, c), CAPI_TAG);
        let r = self.call(&req);
        copy.finish();
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
/// global state that `clear` does not fully reset, `Set` options and
/// memory-mapped loadshape handles chief among them, so a pooled worker that
/// served one deck can hand the next a stale option or mapping: intermittent
/// cross-deck contamination on both channels that vanishes in a serial or
/// one-shot run. Recycling per case makes every case see a never-used worker,
/// so the gate is **deterministic**. The persistent pool then only amortizes
/// startup (workers are spawned once up front), and per-case respawn on
/// check-in overlaps across the pool. Raise via `DSS_GATE_RECYCLE_AFTER=<n>`
/// for a faster, non-deterministic dev loop.
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

/// Resolve the `epri-worker` binary: `DSS_EPRI_WORKER` env override →
/// `$CARGO_TARGET_DIR/<profile>/epri-worker(.exe)` (when the target dir is
/// redirected, which is where the fallback build below actually writes) →
/// `<workspace>/target/<profile>/…` → OnceLock `cargo build` fallback (loud
/// failure). Resolved once per test process.
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
            // `CARGO_TARGET_DIR` first when it is set: the fallback build below
            // honours it, so the in-tree `target/` would otherwise be searched for
            // a binary cargo just wrote somewhere else, and the gate could not
            // run from an out-of-tree copy.
            let candidates: Vec<PathBuf> = std::env::var_os("CARGO_TARGET_DIR")
                .map(PathBuf::from)
                .into_iter()
                .chain(std::iter::once(root.join("target")))
                .map(|d| d.join(profile).join(exe))
                .collect();
            // Freshness, not mere existence: a scoped
            // `cargo test -p dss-core --test corpus_gate` does NOT rebuild the
            // bridge, so an existing binary can predate a `crates/dss-epri`
            // change, and a scoped r4133 run would silently gate against a
            // stale worker. An out-of-date binary falls through to the build
            // below.
            let epri = root.join("crates").join("dss-epri");
            let src_mtime = [
                newest_mtime(&epri.join("src")),
                file_mtime(&epri.join("Cargo.toml")),
            ]
            .into_iter()
            .flatten()
            .max();
            if let Some(found) = candidates.iter().find(|p| p.is_file()) {
                let bin_mtime = found.metadata().ok().and_then(|m| m.modified().ok());
                match (bin_mtime, src_mtime) {
                    // Unknown timestamps: keep the historical behaviour.
                    (None, _) | (_, None) => return found.clone(),
                    (Some(b), Some(s)) if b >= s => return found.clone(),
                    _ => eprintln!(
                        "epri-worker at {} is older than crates/dss-epri — rebuilding it…",
                        found.display()
                    ),
                }
            } else {
                // Fallback: build it once (covers `cargo test -p dss-core`
                // invocations that did not build the whole workspace).
                eprintln!("epri-worker not found at {candidates:?} — building it once…");
            }
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
            candidates
                .iter()
                .find(|p| p.is_file())
                .unwrap_or_else(|| panic!("epri-worker still missing after build: {candidates:?}"))
                .clone()
        })
        .clone()
}

/// The `epri-worker` the gate resolves is not older than the bridge sources it
/// was built from, since a scoped `cargo test -p dss-core --test corpus_gate`
/// builds no workspace binary. An explicit `DSS_EPRI_WORKER` is honoured
/// verbatim, so the claim covers the resolved default only.
#[test]
fn the_epri_worker_binary_is_not_older_than_its_bridge_sources() {
    // `epri_worker_bin` rebuilds a stale binary, so this fails only if the
    // rebuild itself failed to refresh it.
    let bin = epri_worker_bin();
    assert!(
        bin.is_file(),
        "epri-worker did not resolve to a file: {bin:?}"
    );
    if std::env::var_os("DSS_EPRI_WORKER").is_some() {
        eprintln!("DSS_EPRI_WORKER override in effect ({bin:?}) — freshness is the operator's");
        return;
    }
    let root: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    let epri = root.join("crates").join("dss-epri");
    let src = [
        newest_mtime(&epri.join("src")),
        file_mtime(&epri.join("Cargo.toml")),
    ]
    .into_iter()
    .flatten()
    .max()
    .expect("crates/dss-epri sources must be readable");
    let got = file_mtime(&bin).expect("epri-worker must be stat-able");
    assert!(
        got >= src,
        "{} is older than crates/dss-epri ({got:?} < {src:?}) — a scoped corpus-gate run \
         would compare the r4133 channel against a stale bridge",
        bin.display()
    );
}

/// A persistent capi worker whose guarded teardown `clear` raises still delivers
/// its whole reply and then exits, so the pool hands the next case a fresh
/// process. The pinned dss_capi 0.14.5 faults on that second `clear` of an
/// AutoAdd deck.
#[test]
fn a_capi_worker_whose_teardown_clear_raises_replies_in_full_then_exits_for_respawn() {
    // Driven on a copy of the deck in a scratch directory, never on the corpus
    // row, so this test never writes into the window of the manifest case of
    // the same name.
    //
    // Three claims, in order: the reply survives the fault in full (the whole
    // surface is captured before the teardown), the worker is gone by the next
    // request (`Worker::request` -> `None`, i.e. `broken`), and
    // `WorkerPool::call`'s respawn path serves the same faulting case twice in a
    // row without losing one. If the pinned engine is ever fixed, the first
    // assertion fires: read it as "the guard and `teardown_error` can go", not as
    // a regression.
    let root: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect();
    let deck = root.join("tests/corpus/modes/autoadd/autoadd.dss");
    let dir = std::env::temp_dir().join(format!("dss_teardown_d33_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch case dir");
    let case = dir.join("autoadd.dss");
    std::fs::copy(&deck, &case).expect("copy the AutoAdd deck into the scratch dir");
    let req = json!({
        "cmd": "run",
        "case_path": case.to_string_lossy(),
        "post": [],
        "n_steps": 1,
        "full_csc": true,
        "global_result": true,
        "autoadd_log": true,
        "run_files": true,
    });

    let python = std::env::var("DSS_ORACLE_PYTHON").unwrap_or_else(|_| "python".to_string());
    let timeout = oracle_timeout();
    let mut w = spawn_worker(&python, &oracle_server_path());
    assert_pinned(&mut w, timeout);
    let r = w
        .request(&req, timeout)
        .expect("the faulting case must still be answered");
    assert!(
        r.ok,
        "oracle case failed instead of replying: {:?}",
        r.error
    );
    let cr: CaseResult = serde_json::from_value(r.result.expect("ok response carries a result"))
        .expect("malformed CaseResult");
    let err = cr.teardown_error.unwrap_or_else(|| {
        panic!(
            "the pinned dss_capi 0.14.5 no longer faults on the AutoAdd teardown \
             `clear` — the teardown guard, `teardown_error` and this test can be retired"
        )
    });
    assert!(
        err.contains("clear") && err.contains("Access violation"),
        "unexpected teardown fault (expected `#303 ... clear ... Access violation`): {err}"
    );
    // The fault is a TEARDOWN: everything compared is read before it.
    assert_eq!(
        cr.checkpoints.len(),
        1,
        "the run's capture survives the fault"
    );
    assert_eq!(
        cr.checkpoints[0].global_result, "b3, 0.0180069930672805",
        "the AutoAdd winner is still reported"
    );
    assert!(
        cr.autoadd_log.is_some_and(|l| l.lines().count() == 4),
        "the AutoAddLog is still read (4 rows)"
    );
    assert_eq!(
        cr.run_files.as_deref(),
        Some(
            &[
                "gaps_aa_autoaddedgenerators.txt".to_string(),
                "gaps_aa_autoaddlog.csv".to_string()
            ][..]
        ),
        "the created-file set is still reported"
    );
    assert_eq!(
        cr.sweep_failed.as_deref(),
        Some(&[][..]),
        "the guard still swept, and it still REPORTED that it did: {:?}",
        cr.sweep_failed
    );
    // ... and then the worker leaves, so the pool respawns instead of reusing it.
    assert!(
        w.request(&json!({"cmd": "ping"}), Duration::from_secs(30))
            .is_none(),
        "the worker answered another request after a raised teardown `clear` —          `oracle_server.main` must exit so the pool respawns it"
    );
    assert!(w.broken, "the dead worker must be marked broken");
    w.close();

    // The pool's respawn path loses no case: the same faulting request twice.
    let pool = WorkerPool::new(1);
    for i in 0..2 {
        let r = pool.call(&req);
        assert!(r.ok, "pooled faulting case {i} failed: {:?}", r.error);
    }
    pool.close();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Modification time of one file, or `None` when it cannot be stat'd.
fn file_mtime(p: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(p).ok().and_then(|m| m.modified().ok())
}

/// Newest modification time under `dir` (recursively), ignoring anything that
/// cannot be stat'd. `None` when the tree is unreadable or empty — the caller
/// then keeps its historical behaviour rather than guessing staleness.
fn newest_mtime(dir: &std::path::Path) -> Option<std::time::SystemTime> {
    let mut newest: Option<std::time::SystemTime> = None;
    let Ok(rd) = std::fs::read_dir(dir) else {
        return None;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        let t = if p.is_dir() {
            newest_mtime(&p)
        } else {
            entry.metadata().ok().and_then(|m| m.modified().ok())
        };
        if let Some(t) = t {
            newest = Some(newest.map_or(t, |n| n.max(t)));
        }
    }
    newest
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
/// → kill/respawn/retry-once-then-fail-case, recycled per [`recycle_after`]
/// (default 1, a fresh worker per case for determinism, see its doc). A DLL
/// crash (`#303`, whose decks are never sent, a ledger `skip`) kills only that
/// worker.
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
    ///
    /// The r4133 sidecar is attached HERE too, not only in [`Channel::call`]:
    /// the attachment is idempotent (a request that already carries `di_dir`
    /// passes through untouched), and the three cross-transport tests that
    /// build a request by hand and call this one-shot directly must drive the
    /// SAME request shape the scheduler sends — otherwise the capi half of such
    /// a test would get a sidecar and the r4133 half the transport's loud "asked
    /// for the demand-interval tree without a sidecar directory".
    pub(crate) fn call(&self, req: &Value) -> Resp {
        let req = attach_di_sidecar(req, crate::harness::PropsChannel::R4133.tag());
        let mut w = spawn_epri_worker(&self.bin);
        assert_epri(&mut w, self.timeout);
        let out = match w.request(&req, self.timeout) {
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
// The DI sidecar directory.
// ---------------------------------------------------------------------------

/// The `capi_v0145` channel tag. It cannot be `PropsChannel::CapiV0145.tag()`
/// (not a `const fn`), so [`the_channel_tags_are_the_gates_one_vocabulary`] ties
/// the two spellings instead.
const CAPI_TAG: &str = "capi_v0145";

/// The gate's own scratch root, `<target dir>/corpus_gate`.
///
/// Derived from the test binary's own path (`<target>/<profile>/deps/<bin>`),
/// so it follows `CARGO_TARGET_DIR` and a lane's redirected `target` junction
/// without being told. Deliberately NOT under `tests/corpus/` and never the case
/// directory: the corpus guard would classify anything written there as the
/// run's own output.
fn gate_scratch_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| {
            exe.parent() // deps/
                .and_then(|p| p.parent()) // debug/ | release/
                .and_then(|p| p.parent()) // target/
                .map(|p| p.to_path_buf())
        })
        .unwrap_or_else(std::env::temp_dir)
        .join("corpus_gate")
}

/// A file-system-safe, collision-free key for one case: `<parent>_<stem>` in
/// ASCII lower case plus an FNV-1a digest of the whole path, so two cases whose
/// directories happen to share a name never share a sidecar.
fn case_key(case_path: &str) -> String {
    let folded = case_path.replace('\\', "/").to_ascii_lowercase();
    let hash = case_digest(&folded);
    let mut name = String::new();
    let path = std::path::Path::new(&folded);
    for part in [path.parent().and_then(|d| d.file_name()), path.file_stem()]
        .into_iter()
        .flatten()
    {
        for ch in part.to_string_lossy().chars() {
            name.push(if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                ch
            } else {
                '_'
            });
        }
        name.push('_');
    }
    format!("{name}{hash:016x}")
}

/// FNV-1a 64 of a (folded) case path — the one implementation is
/// `harness::scratch::case_digest`, shared by [`case_key`] and the scratch
/// copies' directory key (`scratch::ScratchCopy::new`).
pub(crate) fn case_digest(folded: &str) -> u64 {
    crate::harness::scratch::case_digest(folded)
}

/// Where one (case, channel) DI capture is copied to:
/// `<target>/corpus_gate/di/<case key>/<channel>`.
///
/// The demand-interval tree is run-created, so the channel's guard sweeps it
/// away at the end of the run (inside the channel's scratch copy, which is then
/// removed), while the comparison happens here, later, after this engine has
/// re-run the same case in its own copy. The transport therefore COPIES the
/// selected files into this directory (never moves them: the guard must still
/// sweep the original and the run-file surface must still see its name).
///
/// Lifetime: the directory is prepared (and emptied of the previous run's
/// copies) where the channel identity lives, in [`Channel::call`]
/// (`attach_di_sidecar` → [`prepare_di_sidecar`]) as the request that asks for
/// the tree goes out, and in the two one-shot handles' own calls
/// ([`Oracle::run_case`], [`EpriOneShot::call`]) so a request built by hand
/// carries one too. The attachment is idempotent. The runner removes it after a
/// SUCCESSFUL
/// compare (`runner.rs`, [`remove_di_sidecar`]). A case that FAILED keeps its
/// copies deliberately, for triage, and the next run of that case empties them.
/// No guard ever sweeps this tree — it lives under the gate's own scratch root,
/// outside every case directory.
pub(crate) fn di_sidecar_dir(case_path: &str, channel: &str) -> PathBuf {
    gate_scratch_root()
        .join("di")
        .join(case_key(case_path))
        .join(channel)
}

/// [`di_sidecar_dir`], emptied and created — so a stale copy from an earlier run
/// can never be compared as this run's output.
pub(crate) fn prepare_di_sidecar(case_path: &str, channel: &str) -> PathBuf {
    let dir = di_sidecar_dir(case_path, channel);
    remove_di_sidecar(case_path, channel);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| {
        panic!("cannot create the DI sidecar directory {dir:?} for {case_path}: {e}")
    });
    dir
}

/// Drop one (case, channel) sidecar once its contents have been compared.
pub(crate) fn remove_di_sidecar(case_path: &str, channel: &str) {
    let _ = std::fs::remove_dir_all(di_sidecar_dir(case_path, channel));
}

/// Attach the per-CHANNEL sidecar directory to a run request that asks for the
/// demand-interval tree, creating it empty.
///
/// [`build_run_request`] is channel-agnostic (it sees the manifest row, not the
/// transport), while the sidecar must not be shared by the two channels — this
/// engine runs once per channel and each run writes its own DI tree. The channel
/// handle is therefore where the two meet, exactly as it is for the channel tag
/// itself. A request that does not ask for the tree is passed through untouched,
/// so every other command stays byte-identical.
fn attach_di_sidecar(req: &Value, channel: &str) -> Value {
    // Idempotent: the r4133 one-shot attaches on its own way out too, so a
    // request that already carries its sidecar is passed through rather than
    // re-prepared.
    if req.get("di").and_then(Value::as_bool) != Some(true) || req.get("di_dir").is_some() {
        return req.clone();
    }
    // Keyed by the VENDORED deck (`sidecar_key`, set by `build_run_request`),
    // not by the scratch copy the producer compiles; a request
    // built by hand without it keys by its `case_path`, as before.
    let case_path = req
        .get("sidecar_key")
        .or_else(|| req.get("case_path"))
        .and_then(Value::as_str)
        .expect("a run request that asks for the DI tree carries its case_path");
    let dir = prepare_di_sidecar(case_path, channel);
    let mut out = req.clone();
    out["di_dir"] = json!(dir.to_string_lossy());
    out
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
    /// This channel's tag, DELEGATED to `harness::PropsChannel::tag` — the
    /// gate's one vocabulary for the two gating channels, and the same spelling
    /// the sidecar read-back resolves the directory through
    /// (`runner::channel_tag`), so the two spellings cannot drift apart.
    pub(crate) fn tag(&self) -> &'static str {
        match self {
            Channel::CapiPool(_) | Channel::CapiOneShot(_) => {
                crate::harness::PropsChannel::CapiV0145.tag()
            }
            Channel::EpriPool(_) | Channel::EpriOneShot(_) => {
                crate::harness::PropsChannel::R4133.tag()
            }
        }
    }

    pub(crate) fn call(&self, req: &Value) -> Resp {
        // A request that asks for the demand-interval tree is given THIS
        // channel's sidecar directory here, where the channel identity lives;
        // everything else is passed through untouched.
        let req = &attach_di_sidecar(req, self.tag());
        match self {
            Channel::CapiPool(p) => p.call(req),
            Channel::CapiOneShot(o) => o.call(req),
            Channel::EpriPool(p) => p.call(req),
            Channel::EpriOneShot(e) => e.call(req),
        }
    }
}

#[cfg(test)]
mod di_sidecar_tests {
    use super::*;

    fn minimal_reply() -> Value {
        json!({
            "node_order": [],
            "n_steps": 0,
            "checkpoints": [],
            "sweep_failed": [],
        })
    }

    /// A reply without the `di` key deserializes to "not requested" and `{}` to
    /// "requested, and this deck wrote no demand-interval tree". A transport
    /// that stopped emitting the key must not read as the empty answer.
    #[test]
    fn a_reply_without_the_di_key_is_none_and_an_empty_tree_is_some() {
        let none: CaseResult = serde_json::from_value(minimal_reply()).expect("minimal reply");
        assert!(
            none.di.is_none(),
            "a reply that never asked for the DI tree carries no key at all"
        );

        let mut empty_reply = minimal_reply();
        empty_reply["di"] = json!({});
        let empty: CaseResult = serde_json::from_value(empty_reply).expect("empty DI reply");
        assert_eq!(
            empty.di.as_ref().map(std::collections::BTreeMap::len),
            Some(0),
            "an empty MAP is the honest answer of a deck that writes no DI tree, \
             and must stay distinguishable from `None`"
        );

        let mut full_reply = minimal_reply();
        full_reply["di"] = json!({"ieee13/di_yr_0/totals_1.csv": "ieee13/di_yr_0/totals_1.csv"});
        let full: CaseResult = serde_json::from_value(full_reply).expect("DI reply");
        assert_eq!(
            full.di.expect("a tree")["ieee13/di_yr_0/totals_1.csv"],
            "ieee13/di_yr_0/totals_1.csv",
        );
    }

    /// The sidecar is written under [`Channel::tag`] and read back through
    /// `harness::PropsChannel::tag`, so the two channel tags must be one
    /// vocabulary.
    #[test]
    fn the_channel_tags_are_the_gates_one_vocabulary() {
        assert_eq!(CAPI_TAG, crate::harness::PropsChannel::CapiV0145.tag());
        assert_eq!("r4133", crate::harness::PropsChannel::R4133.tag());
    }

    /// The sidecar is per (case, channel), lives in the gate's own scratch, and
    /// starts EMPTY — a stale copy from an earlier run can never be compared as
    /// this run's output.
    #[test]
    fn the_di_sidecar_is_per_case_and_per_channel_and_starts_empty() {
        let a = "/corpus/family_one/x/master.dss";
        let b = "/corpus/family_two/x/master.dss";
        assert_ne!(
            di_sidecar_dir(a, CAPI_TAG),
            di_sidecar_dir(a, "r4133"),
            "this engine runs once per channel and each run writes its own DI tree"
        );
        assert_ne!(
            di_sidecar_dir(a, CAPI_TAG),
            di_sidecar_dir(b, CAPI_TAG),
            "two cases whose directory and stem happen to agree must not share a \
             sidecar — they can run concurrently"
        );
        assert!(
            di_sidecar_dir(a, CAPI_TAG).starts_with(gate_scratch_root()),
            "the sidecar belongs to the gate's own scratch, never to tests/corpus \
             (where the hygiene guard would classify it as the run's own output)"
        );

        let dir = prepare_di_sidecar(a, CAPI_TAG);
        std::fs::write(dir.join("stale.csv"), b"from an earlier run\n").expect("write stale");
        let again = prepare_di_sidecar(a, CAPI_TAG);
        assert_eq!(dir, again);
        assert_eq!(
            std::fs::read_dir(&again)
                .expect("the sidecar exists")
                .count(),
            0,
            "prepare empties it"
        );
        remove_di_sidecar(a, CAPI_TAG);
        assert!(
            !again.exists(),
            "the runner's removal takes the whole directory"
        );
    }

    /// The run-file contents sidecar follows the DI sidecar's convention: the
    /// gate's own scratch root plus [`case_key`], per case and never inside the
    /// case directory.
    #[test]
    fn the_run_file_contents_sidecar_shares_the_one_sidecar_convention() {
        let a = "/corpus/family_one/x/master.dss";
        let b = "/corpus/family_two/x/master.dss";
        assert_ne!(
            run_file_contents_dir(a),
            run_file_contents_dir(b),
            "two cases whose directory and stem happen to agree must not share a \
             sidecar — they can run concurrently"
        );
        assert!(
            run_file_contents_dir(a).starts_with(gate_scratch_root()),
            "the sidecar belongs to the gate's own scratch, never to tests/corpus \
             (where the hygiene guard would classify it as the run's own output)"
        );
        assert!(
            !run_file_contents_dir(a).starts_with("/corpus/family_one/x"),
            "and never inside the case directory, whose recursive delete would \
             take vendored sources (`dss_epri::guard::refuse_sidecar_under_case_dir`)"
        );
        assert_ne!(
            run_file_contents_dir(a),
            di_sidecar_dir(a, CAPI_TAG),
            "one convention, two sidecars: the same case's contents and DI copies \
             are wiped and read independently"
        );
    }

    /// A request that does not ask for the DI tree is passed through byte for
    /// byte; one that does is given THIS channel's directory.
    #[test]
    fn only_a_request_that_asks_for_the_di_tree_is_given_a_sidecar() {
        let plain = json!({"cmd": "run", "case_path": "/corpus/x/master.dss", "di": false});
        assert_eq!(
            attach_di_sidecar(&plain, CAPI_TAG),
            plain,
            "every other request must reach the transport unchanged"
        );

        let asking = json!({"cmd": "run", "case_path": "/corpus/x/master.dss", "di": true});
        for channel in [CAPI_TAG, "r4133"] {
            let sent = attach_di_sidecar(&asking, channel);
            let dir = sent["di_dir"]
                .as_str()
                .expect("the sidecar travels in the request");
            assert_eq!(
                std::path::Path::new(dir),
                di_sidecar_dir("/corpus/x/master.dss", channel),
            );
            assert!(
                std::path::Path::new(dir).is_dir(),
                "created before the call"
            );
            remove_di_sidecar("/corpus/x/master.dss", channel);
        }
    }

    /// A request that names a scratch copy as its `case_path` and the vendored
    /// deck as its `sidecar_key` gets the sidecar keyed by the vendored deck,
    /// where the runner reads it back. A sidecar keyed by the copy would leave a
    /// stale one, kept for triage, to be compared as this run's output.
    #[test]
    fn the_di_sidecar_is_keyed_by_the_vendored_deck_not_the_copy() {
        // Its own fixture key: nextest runs every test in its own process, and
        // `only_a_request_that_asks_for_the_di_tree_is_given_a_sidecar` creates
        // `/corpus/x/master.dss`'s sidecar and asserts it exists, so a shared
        // key would let this test's removal race that assert.
        let (copy, vendored) = (
            "/scratch/0123/port/run/keyed/master.dss",
            "/corpus/keyed/master.dss",
        );
        let req = json!({"cmd": "run", "case_path": copy, "sidecar_key": vendored, "di": true});
        for channel in [CAPI_TAG, "r4133"] {
            let sent = attach_di_sidecar(&req, channel);
            let dir = sent["di_dir"]
                .as_str()
                .expect("the sidecar travels in the request");
            assert_eq!(
                std::path::Path::new(dir),
                di_sidecar_dir(vendored, channel),
                "keyed by the vendored deck"
            );
            assert_ne!(
                std::path::Path::new(dir),
                di_sidecar_dir(copy, channel),
                "never keyed by the copy"
            );
            remove_di_sidecar(vendored, channel);
        }
    }
}

/// The one duty the two persistent transports keep in the copy protocol (step
/// back to their startup directory before they reply), each with its own rail.
#[cfg(all(test, windows))]
mod transport_cwd_tests {
    use super::*;
    use crate::scratch::ScratchCopy;

    /// A light `both` case with no `post` block and one step.
    const CASE: &str = "Version8/Distrib/IEEETestCases/4Bus-OYOD-Bal/4Bus-OYOD-Bal.DSS";

    /// Run [`CASE`] on a live, persistent `worker` in a fresh copy, then remove
    /// the copy WHILE the worker still runs, then ping it. Both engines leave
    /// the process working directory in the compiled deck's folder, and
    /// Windows refuses to remove a directory that is a live process's working
    /// directory, so a transport that replied from inside the copy fails the
    /// removal here. The gate itself cannot see it: its pooled workers are
    /// recycled after every case by default ([`recycle_after`]) and its one-shot
    /// transports exit, and either way the process is gone before the removal.
    fn steps_out(mut worker: Worker, tag: &'static str) {
        let abs = crate::manifest::corpus_file(CASE);
        let case = crate::manifest::load_solvable()
            .into_iter()
            .find(|c| c.path == CASE)
            .unwrap_or_else(|| panic!("{CASE} is no longer a solvable_now case"));
        let copy = ScratchCopy::new(&abs, tag);
        let req = build_run_request(copy.deck(), &abs, &case);
        let r = worker
            .request(&req, oracle_timeout())
            .unwrap_or_else(|| panic!("[{tag}] {CASE}: no reply from the worker"));
        assert!(r.ok, "[{tag}] {CASE}: {:?}", r.error);
        let removed = copy.remove();
        let ping = worker.request(&json!({"cmd": "ping"}), oracle_timeout());
        worker.close();
        if let Err(e) = removed {
            panic!(
                "[{tag}] {CASE}: the worker replied from inside its copy (it must \
                 step back to its startup directory first): {e}"
            );
        }
        assert!(
            ping.is_some_and(|p| p.ok),
            "[{tag}] the worker died after its reply, so the removal proves nothing"
        );
    }

    #[test]
    fn the_capi_transport_steps_out_of_the_copy_before_replying() {
        let python = std::env::var("DSS_ORACLE_PYTHON").unwrap_or_else(|_| "python".to_string());
        steps_out(spawn_worker(&python, &oracle_server_path()), CAPI_TAG);
    }

    #[test]
    fn the_r4133_transport_steps_out_of_the_copy_before_replying() {
        steps_out(spawn_epri_worker(&epri_worker_bin()), "r4133");
    }

    /// The request for [`CASE`] names its copy and keys its sidecars by the
    /// vendored deck, and a request for the vendored deck itself is refused
    /// before any transport could see it.
    #[test]
    fn a_request_for_a_vendored_deck_is_refused() {
        // Only the second call may panic: a `should_panic` on the whole body
        // would also pass a builder that refused the copy.
        let abs = crate::manifest::corpus_file(CASE);
        let case = crate::manifest::load_solvable()
            .into_iter()
            .find(|c| c.path == CASE)
            .unwrap_or_else(|| panic!("{CASE} is no longer a solvable_now case"));
        let copy = ScratchCopy::new(&abs, crate::scratch::PORT);
        let req = build_run_request(copy.deck(), &abs, &case);
        assert_eq!(req["case_path"], copy.deck(), "the request names the copy");
        assert_eq!(
            req["sidecar_key"],
            abs.as_str(),
            "keyed by the vendored deck"
        );
        copy.finish();
        let refused = std::panic::catch_unwind(|| build_run_request(&abs, &abs, &case));
        let msg = crate::runner::panic_msg(
            refused.expect_err("a request for the vendored deck must be refused"),
        );
        assert!(
            msg.contains("engines::build_run_request")
                && msg.contains("lies inside the vendored corpus"),
            "{msg}"
        );
    }
}

/// A one-shot engine that dies before it replies names its exit status and
/// hands over its whole stderr.
#[cfg(test)]
mod one_shot_exit_tests {
    use super::*;

    /// Every named status, the unnamed NTSTATUS form and the plain form.
    #[test]
    fn an_ntstatus_exit_code_is_printed_in_hex_with_its_name() {
        assert_eq!(describe_exit_code(3), "exit code 3");
        for (code, hex_and_name) in [
            (-1_073_741_819, "0xc0000005 (access violation)"),
            (-1_073_741_795, "0xc000001d (illegal instruction)"),
            (-1_073_741_676, "0xc0000094 (integer divide by zero)"),
            (-1_073_741_571, "0xc00000fd (stack overflow)"),
            (-1_073_740_940, "0xc0000374 (heap corruption)"),
            (
                -1_073_740_791,
                "0xc0000409 (fail-fast: stack buffer overrun or abort)",
            ),
            (-532_262_845, "0xe0465043 (unhandled Free Pascal exception)"),
            (-1_073_741_515, "0xc0000135"),
        ] {
            assert_eq!(
                describe_exit_code(code),
                format!("exit code {code} = {hex_and_name}")
            );
        }
    }

    /// A stand-in server that enables `faulthandler` as `oracle_server.py`
    /// does, writes a line ending in a byte that is not UTF-8 to each pipe,
    /// reads its request and dies of an access violation: the panic's first
    /// line names the status, the stdout it carries keeps its line, and the
    /// stderr keeps the server's own line and the dump with the faulting frame.
    #[cfg(windows)]
    #[test]
    fn a_one_shot_that_dies_natively_panics_with_its_status_and_stderr() {
        let dir = gate_scratch_root().join("one_shot_exit");
        std::fs::create_dir_all(&dir).expect("create the stand-in's directory");
        let server = dir.join(format!("dies_{}.py", std::process::id()));
        std::fs::write(
            &server,
            "import faulthandler, sys\n\
             faulthandler.enable(file=sys.stderr, all_threads=True)\n\
             sys.stdout.buffer.write(b'stand-in stdout \\xff\\n'); sys.stdout.buffer.flush()\n\
             sys.stderr.buffer.write(b'stand-in ready \\xff\\n'); sys.stderr.buffer.flush()\n\
             sys.stdin.readline()\n\
             faulthandler._read_null()\n",
        )
        .expect("write the stand-in server");
        let oracle = Oracle {
            python: std::env::var("DSS_ORACLE_PYTHON").unwrap_or_else(|_| "python".to_string()),
            server: server.clone(),
            envs: Vec::new(),
        };
        let died = std::panic::catch_unwind(|| oracle.call(&json!({"cmd": "ping"})));
        let removed = std::fs::remove_file(&server);
        let msg = crate::runner::panic_msg(died.expect_err("a dead one-shot must panic"));
        assert_eq!(
            msg.lines().next(),
            Some(
                "oracle produced no JSON response (the process exited on its own, \
                 exit code -1073741819 = 0xc0000005 (access violation))"
            ),
            "{msg}"
        );
        let (stdout, stderr) = msg
            .split_once("--- stderr ---")
            .unwrap_or_else(|| panic!("no stderr section in:\n{msg}"));
        assert!(
            stdout.contains("stand-in stdout \u{FFFD}"),
            "the stdout section lost its line:\n{msg}"
        );
        for needle in [
            "stand-in ready \u{FFFD}",
            "Windows fatal exception: access violation",
            "line 6 in <module>",
        ] {
            assert!(stderr.contains(needle), "missing {needle:?} in:\n{msg}");
        }
        removed.expect("remove the stand-in server");
    }
}
