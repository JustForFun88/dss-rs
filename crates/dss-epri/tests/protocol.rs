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

    // Stop-on-first-error semantics: a bad middle command aborts the batch — the
    // trailing command must NOT run, and `failed_at` pins the exact index.
    let bad_batch = w.request(json!({"cmd": "batch", "exec": [
        "new load.probe bus1=mid phases=3 kv=12.47 kw=1",
        "bogus_command_xyz 1",
        "solve",
    ]}));
    assert_eq!(
        bad_batch["ok"], false,
        "failing batch must report ok:false: {bad_batch}"
    );
    let br = &bad_batch["result"];
    assert_eq!(
        br["ran"].as_u64(),
        Some(2),
        "batch must stop after the failure: {bad_batch}"
    );
    assert_eq!(
        br["failed_at"].as_u64(),
        Some(1),
        "failed_at must pin the bad index: {bad_batch}"
    );
    assert_eq!(
        br["replies"][0]["ok"], true,
        "first item should have run: {bad_batch}"
    );
    assert_eq!(
        br["replies"][1]["ok"], false,
        "second item is the failure: {bad_batch}"
    );

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
    // F getter happy path: Solution.Frequency (F mode 0) == base frequency 60.
    let fr = w.ffi(json!({"cmd": "ffi", "family": "Solution", "kind": "f", "mode": 0}));
    assert_eq!(fr["kind"], "f", "F getter kind: {fr}");
    assert_eq!(
        fr["value"].as_f64(),
        Some(60.0),
        "Solution.Frequency != 60: {fr}"
    );
    assert_eq!(fr["errno"], 0, "F getter errno: {fr}");
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

    // Partial SET (`len < NumPoints`): `mySize` is an ELEMENT count, so a
    // 3-element write into a 5-point shape must fill points 1..3 and clamp there
    // (`LoopLimit = mySize`, not NumPoints), leaving points 4..5 untouched. Under
    // the old byte-count bug this passed `mySize = 24 > 5`, defeating the clamp
    // and over-reading the 3-double buffer — a memory-safety violation this case
    // guards against.
    w.exec("new loadshape.ls5 npts=5 interval=1 mult=(2 2 2 2 2)");
    w.ffi(json!({"cmd": "ffi", "family": "LoadShape", "kind": "s", "mode": 1, "sarg": "ls5"}));
    let set5 = w.ffi(json!({
        "cmd": "ffi", "family": "LoadShape", "kind": "v", "mode": 2,
        "vset": {"type": 2, "data": [10.0, 20.0, 30.0]}
    }));
    assert_eq!(
        set5["written"].as_i64(),
        Some(3),
        "partial vset must accept exactly the 3 supplied elements: {set5}"
    );
    let post5 = w.ffi(json!({"cmd": "ffi", "family": "LoadShape", "kind": "v", "mode": 1}));
    assert_eq!(
        post5["data"],
        json!([10.0, 20.0, 30.0, 2.0, 2.0]),
        "partial SET must fill points 1..3 and leave 4..5 intact: {post5}"
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
    // SystemYChanged read/write round-trip (mode 1 writes `arg`, mode 0 reads):
    // set the flag TRUE then FALSE and confirm the read tracks the write.
    w.ok(json!({"cmd": "ymatrix", "op": "system_y_changed", "mode": 1, "arg": 1}));
    assert_eq!(
        w.ok(json!({"cmd": "ymatrix", "op": "system_y_changed", "mode": 0}))["result"].as_i64(),
        Some(1),
        "SystemYChanged should read back 1 after set-true"
    );
    w.ok(json!({"cmd": "ymatrix", "op": "system_y_changed", "mode": 1, "arg": 0}));
    assert_eq!(
        w.ok(json!({"cmd": "ymatrix", "op": "system_y_changed", "mode": 0}))["result"].as_i64(),
        Some(0),
        "SystemYChanged should read back 0 after set-false"
    );
    // UseAuxCurrents read/write round-trip (same DYMatrix flag shape).
    w.ok(json!({"cmd": "ymatrix", "op": "use_aux_currents", "mode": 1, "arg": 1}));
    assert_eq!(
        w.ok(json!({"cmd": "ymatrix", "op": "use_aux_currents", "mode": 0}))["result"].as_i64(),
        Some(1),
        "UseAuxCurrents should read back 1 after set-true"
    );
    w.ok(json!({"cmd": "ymatrix", "op": "use_aux_currents", "mode": 1, "arg": 0}));
    // Injection / build ops: exercise every remaining ymatrix arm against the
    // live DLL (they mutate Solution vectors in place — assert the ack shape).
    assert_eq!(
        w.ok(json!({"cmd": "ymatrix", "op": "build_y", "build_ops": 1, "allocate_vi": 1}))["result"]
            ["built"],
        json!(true),
        "build_y ack"
    );
    for op in ["zero_inj", "get_source_inj", "get_pc_inj"] {
        assert_eq!(
            w.ok(json!({"cmd": "ymatrix", "op": op}))["result"]["done"],
            json!(true),
            "{op} ack"
        );
    }
    assert_eq!(
        w.ok(json!({"cmd": "ymatrix", "op": "add_aux", "stype": 0}))["result"]["done"],
        json!(true),
        "add_aux ack"
    );
    let iptr = w.ok(json!({"cmd": "ymatrix", "op": "ipointer"}));
    assert_eq!(
        iptr["result"].as_array().unwrap().len() as i64,
        2 * (n_nodes + 1),
        "getIpointer shape != 2*(NumNodes+1): {iptr}"
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

/// F2 guard: the DYMatrix ops that deref `ActiveCircuit.Solution` without a nil
/// check in the DLL must be rejected with a clean error before any circuit is
/// compiled — NOT forwarded to the DLL (which would nil-deref and kill the
/// worker). Drive them on a fresh worker with no `compile`/`new circuit`.
#[test]
fn ymatrix_before_compile_is_guarded_not_a_crash() {
    let mut w = WorkerProc::spawn();
    for op in [
        "solve_system",
        "vpointer",
        "ipointer",
        "system_y_changed",
        "use_aux_currents",
        "build_y",
        "add_aux",
    ] {
        let r = w.request(json!({"cmd": "ymatrix", "op": op}));
        assert_eq!(
            r["ok"], false,
            "ymatrix {op:?} before compile must fail cleanly, not crash: {r}"
        );
    }
    // The worker survived every unguarded op — the guard held.
    let pong = w.ok(json!({"cmd": "ping"}));
    assert_eq!(
        pong["pong"], true,
        "worker died on a pre-compile ymatrix op"
    );
    w.quit();
}

/// G1.0 / WP-G1 rail: the **two-double `XxxF` ABI**. `CmathLibF(mode; arg1,
/// arg2: double)` (`DCmathLib.pas:5`, impl `:12`) and `CircuitF(mode; arg1,
/// arg2: double)` (`DCircuit.pas:27`, impl `:193`) are the only two of the 42
/// DDLL families whose `F` export takes a second double; the bridge used to bind
/// every `F` as `fn(i32, f64)`, so `XMM2` carried whatever the caller left
/// behind.
///
/// Measured against this same DLL **before** the fix (2026-09-04):
/// `CmathLibF(0, 3.0, ?)` = `3.0` instead of `Cabs(3+4j)` = `5.0` — XMM2 held a
/// tiny positive leftover, so `sqrt(9 + ε²)` rounded back to `3.0`.
///
/// Every reading below is pinned by **exact** equality; none is 90°-clean,
/// because r4133's own complex math is built on two truncated constants:
/// `CDANG(a) = ATAN2(a.re, a.im) * 57.29577951` (`Ucomplex.pas:117-120`) over
/// OpenDSS's hand-written `ATAN2` with `CONST PI = 3.14159265359`
/// (`Ucomplex.pas:96-111`). So `Cdang(0+1j)` is `(3.14159265359/2)·57.29577951`
/// = `89.99999999516423`, **not** `90.0`, and `Cdang(3+4j)` is
/// `arctan(4/3)·57.29577951` = `53.13010235129776`. These are *oracle* readings
/// of the DLL under test, not values `dss-core` reproduces.
///
/// `Cdang(0, 1)` alone would NOT prove the fix: the angle of a purely imaginary
/// number is the same for every positive magnitude, so the pre-fix garbage read
/// the identical `89.99999999516423`. `Cdang(3, 4)` is the discriminating one —
/// with the one-double binding it reads `arctan(ε/3)·57.29577951 ≈ 0`.
///
/// `CircuitF` mode 0 (`Circuit.Capacity`) is deliberately NOT driven: it writes
/// `CapacityStart`/`CapacityIncrement` and runs `ComputeCapacity`
/// (`DCircuit.pas:195-203`) — a mutating probe. Its correctness rides on the
/// same `FnF2` binding this test pins.
#[test]
fn cmath_lib_f_takes_two_doubles() {
    let mut w = WorkerProc::spawn();

    // Cabs(3 + 4j) == 5.0 exactly (was 3.0 = arg1 with the one-double binding).
    let cabs = w.ffi(json!({
        "cmd": "ffi", "family": "CmathLib", "kind": "f", "mode": 0,
        "farg": 3.0, "farg2": 4.0
    }));
    assert_eq!(cabs["kind"], "f", "Cabs kind: {cabs}");
    assert_eq!(
        cabs["value"].as_f64(),
        Some(5.0),
        "CmathLib.Cabs(3, 4) != 5.0 — the second double did not reach XMM2: {cabs}"
    );
    assert_eq!(cabs["errno"], 0, "Cabs errno: {cabs}");

    // Cdang(3 + 4j) == arctan(4/3) * 57.29577951, exactly (was ~0 pre-fix).
    let cdang = w.ffi(json!({
        "cmd": "ffi", "family": "CmathLib", "kind": "f", "mode": 1,
        "farg": 3.0, "farg2": 4.0
    }));
    assert_eq!(
        cdang["value"].as_f64(),
        Some(53.13010235129776),
        "CmathLib.Cdang(3, 4) != arctan(4/3)*57.29577951: {cdang}"
    );

    // Cdang(0 + 1j) == (3.14159265359 / 2) * 57.29577951, exactly — r4133's
    // truncated PI and rad->deg constants, NOT a clean 90.0.
    let quarter = w.ffi(json!({
        "cmd": "ffi", "family": "CmathLib", "kind": "f", "mode": 1,
        "farg": 0.0, "farg2": 1.0
    }));
    assert_eq!(
        quarter["value"].as_f64(),
        Some(89.99999999516423),
        "CmathLib.Cdang(0, 1) != (3.14159265359/2)*57.29577951: {quarter}"
    );

    // Non-vacuity: the fix is the SECOND argument, not a changed mode 0. Drop
    // `farg2` (defaults to 0.0) and Cabs(3, 0) must be 3.0 — i.e. the call really
    // reads what the caller passes, rather than always returning 5.0.
    let degenerate = w.ffi(json!({
        "cmd": "ffi", "family": "CmathLib", "kind": "f", "mode": 0, "farg": 3.0
    }));
    assert_eq!(
        degenerate["value"].as_f64(),
        Some(3.0),
        "Cabs(3, 0) must be 3.0: {degenerate}"
    );

    // The unknown-mode sentinel of the `F` shape is `-1.0` (`DCmathLib.pas:22`),
    // and it is reachable through the same two-double binding.
    let bogus = w.ffi(json!({
        "cmd": "ffi", "family": "CmathLib", "kind": "f", "mode": 987,
        "farg": 3.0, "farg2": 4.0
    }));
    assert_eq!(
        bogus["value"].as_f64(),
        Some(-1.0),
        "CmathLibF unknown-mode sentinel: {bogus}"
    );

    // `Circuit` still reports an `f` shape in the capability handshake — the ABI
    // split must not drop its entry point.
    let caps = w.ok(json!({"cmd": "caps"}));
    let kinds = caps["families"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"] == "Circuit")
        .unwrap_or_else(|| panic!("Circuit missing from caps: {caps}"))["kinds"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(kinds, "ifsv", "Circuit lost its F shape in the ABI split");
    assert_eq!(
        caps["family_entry_points"], 147,
        "entry-point count moved with the F ABI split: {caps}"
    );

    w.quit();
}
