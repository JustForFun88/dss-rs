//! Worker-level smoke for the `exec`/`read`/`chdir` scripting surface (the
//! EPRI-bridge parity round — the protocol the manual regen drivers and probes
//! use via `tools/opendss/epri_worker.py`). Spawns the real `epri-worker`
//! binary and drives the line-JSON protocol end-to-end against the vendored
//! r4133 DLL, anchoring:
//!
//! - the event-log read format (`Hour=…, Sec=…` `EventStrings` lines, exactly
//!   what the committed `tests/golden/protection/*.json` `event_log` arrays
//!   store) and its empty-log `None`-placeholder normalization to `[]`;
//! - the per-read shapes (solution scalars, node arrays, element powers /
//!   currents, variables, monitor channels, bus kVBase);
//! - the error path (`ok:false` on a bad command, worker stays alive).
//!
//! Needs no oracle installed; gate-neutral (`run`/`ping`/`clear` untouched).

#![cfg(windows)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

struct WorkerProc {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl WorkerProc {
    fn spawn() -> WorkerProc {
        let bin = env!("CARGO_BIN_EXE_epri-worker");
        let mut child = Command::new(bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("cannot spawn {bin}: {e}"));
        let stdin = child.stdin.take().expect("worker stdin");
        let stdout = BufReader::new(child.stdout.take().expect("worker stdout"));
        WorkerProc {
            child,
            stdin,
            stdout,
        }
    }

    /// Send one request line, read one reply line.
    fn request(&mut self, req: Value) -> Value {
        let line = serde_json::to_string(&req).expect("serialize request");
        writeln!(self.stdin, "{line}").expect("write request");
        self.stdin.flush().expect("flush request");
        let mut reply = String::new();
        self.stdout.read_line(&mut reply).expect("read reply");
        assert!(!reply.trim().is_empty(), "worker closed on {line}");
        serde_json::from_str(reply.trim())
            .unwrap_or_else(|e| panic!("bad reply {reply:?} for {line}: {e}"))
    }

    /// Request that must succeed; returns `result`.
    fn ok(&mut self, req: Value) -> Value {
        let r = self.request(req.clone());
        assert_eq!(
            r.get("ok").and_then(Value::as_bool),
            Some(true),
            "request {req} failed: {r}"
        );
        r.get("result").cloned().unwrap_or(Value::Null)
    }

    fn exec(&mut self, text: &str) -> Value {
        self.ok(json!({"cmd": "exec", "text": text}))
    }

    fn read(&mut self, what: &str) -> Value {
        self.ok(json!({"cmd": "read", "what": what}))
    }

    fn read_named(&mut self, what: &str, name: &str) -> Value {
        self.ok(json!({"cmd": "read", "what": what, "name": name}))
    }

    /// Generic FFI dispatch (EPRI Round 2 capability channel).
    fn ffi(&mut self, req: Value) -> Value {
        self.ok(req)
    }

    fn quit(mut self) {
        let _ = writeln!(self.stdin, "{}", json!({"cmd": "quit"}));
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

/// The `gen_protection.py` fuse_blow circuit (a tiny inline radial feeder) plus
/// a mode-0 monitor and a generator, so every scripting read has a subject.
const DECK: &[&str] = &[
    "Set DefaultBaseFrequency=60",
    "new circuit.prot basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=21000",
    "new linecode.lc nphases=3 r1=0.3 x1=0.6 r0=0.7 x0=1.9 c1=0 c0=0 units=mi",
    "new line.feed bus1=src bus2=mid linecode=lc length=1 units=mi",
    "new line.lat  bus1=mid bus2=loadb linecode=lc length=1 units=mi",
    "new load.l bus1=loadb phases=3 kv=12.47 kw=500 pf=0.95 model=1",
    "new generator.g bus1=loadb phases=3 kv=12.47 kw=10 pf=1 model=1",
    "new monitor.m element=line.feed terminal=1 mode=0",
    "new fuse.fz monitoredobj=line.feed monitoredterm=1 \
     switchedobj=line.feed switchedterm=1 fusecurve=tlink curvemultiplier=40 ratedcurrent=40",
    "new fault.f bus1=loadb phases=3 ontime=0.1 r=1 temporary=no",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "set mode=duty stepsize=0.1 number=1 controlmode=time",
];

#[test]
fn scripting_surface_end_to_end() {
    let mut w = WorkerProc::spawn();

    // ping: the r4133 markers.
    let pong = w.ok(json!({"cmd": "ping"}));
    assert_eq!(pong["oracle"]["rev"], "r4133", "wrong revision: {pong}");
    assert_eq!(pong["oracle"]["epri"], true, "not the EPRI bridge: {pong}");

    // chdir round-trips (process-global, regen drivers use it for relative
    // File= resolution).
    let tmp = std::env::temp_dir();
    w.ok(json!({"cmd": "chdir", "dir": tmp.to_string_lossy()}));

    // Build the circuit.
    w.exec("clear");
    for c in DECK {
        w.exec(c);
    }

    // Step 1 (t=0.1 s): solved, converged, empty event log (the bare `None`
    // placeholder without a terminator must normalize to []).
    w.exec("solve");
    assert_eq!(w.read("converged"), json!(true));
    let dh = w.read("dbl_hour").as_f64().unwrap();
    assert!(
        (dh * 3600.0 - 0.1).abs() < 1e-9,
        "dbl_hour {dh} != 0.1 s after step 1"
    );
    assert!(w.read("iterations").as_i64().unwrap() > 0);
    assert_eq!(
        w.read("eventlog"),
        json!([]),
        "empty event log must decode to [] (Oddie None-placeholder parity)"
    );

    // Node arrays: order and interleaved voltages agree in size.
    let order = w.read("ynode_order");
    let n_nodes = order.as_array().unwrap().len();
    assert!(n_nodes >= 9, "expected >= 9 nodes, got {n_nodes}");
    let varray = w.read("ynode_varray");
    assert_eq!(varray.as_array().unwrap().len(), 2 * n_nodes);

    // Element reads: 3 conductors x 2 terminals -> 12 flat floats.
    let names = w.read("all_element_names");
    assert!(
        names
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str() == Some("Line.feed")),
        "Line.feed missing from {names}"
    );
    w.read_named("set_active_element", "Line.feed");
    let powers = w.read("element_powers");
    let currents = w.read("element_currents");
    assert_eq!(powers.as_array().unwrap().len(), 12);
    assert_eq!(currents.as_array().unwrap().len(), 12);

    // Variables (generator state).
    let vars = w.read_named("variables", "Generator.g");
    let names = vars["var_names"].as_array().unwrap();
    let values = vars["values"].as_array().unwrap();
    assert!(!names.is_empty(), "generator has no variables: {vars}");
    assert_eq!(names.len(), values.len());

    // Bus kVBase (12.47 kV LL base -> 12.47/sqrt(3) kV LN).
    let kvb = w.read_named("bus_kvbase", "loadb").as_f64().unwrap();
    assert!(
        (kvb * 3.0_f64.sqrt() - 12.47).abs() < 1e-9,
        "kVBase {kvb} != 12.47/sqrt(3)"
    );

    // Steps 2..=5: the fault applies at t=0.2 s and the fuse blows at t=0.4 s —
    // the event-log lines must be the exact `EventStrings` format the committed
    // protection goldens pin.
    for _ in 1..5 {
        w.exec("solve");
    }
    let log = w.read("eventlog");
    let log: Vec<String> = log
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l.as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        log.first().map(String::as_str),
        Some("Hour=0, Sec=0.2, ControlIter=1, Element=Fault.f, Action=**APPLIED**"),
        "event-log format drifted: {log:?}"
    );
    assert!(
        log.iter().any(|l| l.contains("BLOWN")),
        "fuse blow missing from event log: {log:?}"
    );

    // Monitor channel reads: one sample per duty step.
    w.read_named("monitor_select", "m");
    let sc = w.read("monitor_sample_count").as_i64().unwrap();
    assert_eq!(sc, 5, "5 duty steps -> 5 samples");
    let nch = w.read("monitor_num_channels").as_i64().unwrap();
    assert!(nch > 0);
    let ch1 = w.ok(json!({"cmd": "read", "what": "monitor_channel", "index": 1}));
    assert_eq!(ch1.as_array().unwrap().len(), sc as usize);

    // Error path: a bogus command reports ok:false and the worker stays alive.
    let bad = w.request(json!({"cmd": "exec", "text": "bogus_command_xyz 1"}));
    assert_eq!(
        bad.get("ok").and_then(Value::as_bool),
        Some(false),
        "bogus command must fail: {bad}"
    );
    let pong = w.ok(json!({"cmd": "ping"}));
    assert_eq!(pong["pong"], true, "worker died after error: {pong}");

    w.quit();
}

/// A tiny inline feeder for the capability smoke — its own subject set.
const CAP_DECK: &[&str] = &[
    "clear",
    "new circuit.captest basekv=12.47 phases=3 bus1=src",
    "new linecode.lc nphases=3 r1=0.3 x1=0.6 r0=0.7 x0=1.9 c1=0 c0=0 units=mi",
    "new line.feed bus1=src bus2=mid linecode=lc length=1 units=mi",
    "new load.l bus1=mid phases=3 kv=12.47 kw=500 pf=0.95 model=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
];

/// EPRI capability Round 2: the generic `caps`/`ffi`/`ymatrix`/`batch` surface
/// that covers everything the Oddie/dss-python bridge could reach over the same
/// engine, plus the "beyond" (structured errno, batched exec, capability
/// handshake). Drives the REAL r4133 DLL end-to-end. Gate-neutral: the
/// `run`/`ping`/`clear` capture path is untouched; the scheduler never sends any
/// of these commands.
#[test]
fn capability_surface_end_to_end() {
    let mut w = WorkerProc::spawn();

    // ---- 1. Structured capability handshake (beyond the Python bridge) -----
    let caps = w.ok(json!({"cmd": "caps"}));
    assert_eq!(caps["protocol_version"], 2, "caps proto drift: {caps}");
    assert_eq!(
        caps["family_count"], 42,
        "family registry drifted from the r4133 export table: {caps}"
    );
    assert_eq!(
        caps["family_entry_points"], 147,
        "family entry-point count drifted: {caps}"
    );
    let cmds: Vec<&str> = caps["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    for want in ["ffi", "ymatrix", "caps", "batch", "run"] {
        assert!(cmds.contains(&want), "caps missing command {want}: {caps}");
    }
    // A few family shapes: full quartet, an F-less family, and a bare-S family.
    let fam_kinds = |name: &str| -> String {
        caps["families"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["name"] == name)
            .unwrap_or_else(|| panic!("family {name} missing from caps: {caps}"))["kinds"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(fam_kinds("Circuit"), "ifsv");
    assert_eq!(fam_kinds("Monitors"), "isv", "Monitors has no F export");
    assert_eq!(fam_kinds("DSSProperties"), "s");

    // ---- 2. Batched multi-command exec in ONE round-trip (beyond) ----------
    let batch = w.ok(json!({"cmd": "batch", "exec": CAP_DECK}));
    assert_eq!(
        batch["ran"].as_u64().unwrap(),
        CAP_DECK.len() as u64,
        "batch did not run every command: {batch}"
    );
    assert!(batch["failed_at"].is_null(), "batch had a failure: {batch}");
    // solve as a follow-up single exec.
    w.exec("solve");

    // ---- 3. Structured errno surface (no active LoadShape -> #61001) -------
    // Read PMult with no loadshape defined: the DLL sets a non-fatal errno,
    // surfaced structurally (the Python bridge never exposed this per-call).
    let err = w.ffi(json!({"cmd": "ffi", "family": "LoadShape", "kind": "v", "mode": 1}));
    assert_eq!(
        err["errno"].as_i64(),
        Some(61001),
        "expected #61001 for no-active-loadshape: {err}"
    );
    assert!(
        err["error"].as_str().unwrap().contains("Loadshape"),
        "errno desc not surfaced: {err}"
    );

    // ---- 4. Generic FFI == typed channel (I/S/V getters cross-check) -------
    // Circuit NumNodes (I mode 2) must equal the typed node-order length.
    let order = w.read("ynode_order");
    let n_nodes = order.as_array().unwrap().len() as i64;
    let ni = w.ffi(json!({"cmd": "ffi", "family": "Circuit", "kind": "i", "mode": 2}));
    assert_eq!(ni["kind"], "i");
    assert_eq!(
        ni["value"].as_i64(),
        Some(n_nodes),
        "NumNodes mismatch: {ni}"
    );
    assert_eq!(ni["errno"], 0);
    // Circuit Name (S mode 0).
    let nm = w.ffi(json!({"cmd": "ffi", "family": "Circuit", "kind": "s", "mode": 0}));
    assert_eq!(nm["value"], "captest", "circuit name: {nm}");
    // AllElementNames (V mode 6, type 4) must equal the typed read.
    let ve = w.ffi(json!({"cmd": "ffi", "family": "Circuit", "kind": "v", "mode": 6}));
    assert_eq!(ve["type"], 4, "AllElementNames not a string array: {ve}");
    assert_eq!(
        ve["data"],
        w.read("all_element_names"),
        "generic V getter diverged from the typed channel: {ve}"
    );

    // ---- 5. Generic FFI array SET round-trip (V setter) --------------------
    w.exec("new loadshape.ls npts=3 interval=1 mult=(1 1 1)");
    w.ffi(json!({"cmd": "ffi", "family": "LoadShape", "kind": "s", "mode": 1, "sarg": "ls"}));
    let pre = w.ffi(json!({"cmd": "ffi", "family": "LoadShape", "kind": "v", "mode": 1}));
    assert_eq!(pre["data"], json!([1.0, 1.0, 1.0]), "PMult pre-set: {pre}");
    let set = w.ffi(json!({
        "cmd": "ffi", "family": "LoadShape", "kind": "v", "mode": 2,
        "vset": {"type": 2, "data": [5.0, 6.0, 7.0]}
    }));
    assert_eq!(set["set"], true, "vset not acknowledged: {set}");
    assert_eq!(set["written"].as_i64(), Some(3), "vset count: {set}");
    let post = w.ffi(json!({"cmd": "ffi", "family": "LoadShape", "kind": "v", "mode": 1}));
    assert_eq!(
        post["data"],
        json!([5.0, 6.0, 7.0]),
        "V-protocol array SET did not stick: {post}"
    );

    // ---- 6. Y-matrix / injection helpers (ymatrix command) -----------------
    let dims = w.ok(json!({"cmd": "ymatrix", "op": "y_dims"}));
    assert_eq!(
        dims["result"]["n_bus"].as_i64(),
        Some(n_nodes),
        "y_dims n_bus != NumNodes: {dims}"
    );
    let vptr = w.ok(json!({"cmd": "ymatrix", "op": "vpointer"}));
    assert_eq!(
        vptr["result"].as_array().unwrap().len() as i64,
        2 * (n_nodes + 1),
        "getVpointer shape != 2*(NumNodes+1): {vptr}"
    );
    // SystemYChanged read (mode 0) is a clean bool-ish int; the entry is live.
    let syc = w.ok(json!({"cmd": "ymatrix", "op": "system_y_changed", "mode": 0}));
    assert!(
        syc["result"].is_i64(),
        "system_y_changed not readable: {syc}"
    );
    // SolveSystem: the external back-substitution entry is live and SUCCEEDS —
    // KLU `SolveSparseSet` returns 1 on success (the engine's own success test,
    // `Solution.pas` `IF SolveSystem(...) = 1`), so a solved circuit must give 1.
    let ss = w.ok(json!({"cmd": "ymatrix", "op": "solve_system"}));
    assert_eq!(
        ss["result"]["status"].as_i64(),
        Some(1),
        "solve_system did not report KLU success (1): {ss}"
    );

    // ---- 7. Error paths keep the worker alive ------------------------------
    let bad_fam = w.request(json!({"cmd": "ffi", "family": "Nope", "kind": "i", "mode": 0}));
    assert_eq!(bad_fam["ok"], false, "unknown family must fail: {bad_fam}");
    let bad_kind = w.request(json!({"cmd": "ffi", "family": "Circuit", "kind": "z", "mode": 0}));
    assert_eq!(bad_kind["ok"], false, "unknown kind must fail: {bad_kind}");
    // Missing-shape family (Monitors has no F).
    let no_f = w.request(json!({"cmd": "ffi", "family": "Monitors", "kind": "f", "mode": 0}));
    assert_eq!(no_f["ok"], false, "absent ABI shape must fail: {no_f}");
    let pong = w.ok(json!({"cmd": "ping"}));
    assert_eq!(pong["pong"], true, "worker died after error paths: {pong}");

    w.quit();
}
