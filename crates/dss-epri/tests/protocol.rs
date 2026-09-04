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

// ---------------------------------------------------------------------------
// D13 — the bridge must not carry state between cases or between processes
// ---------------------------------------------------------------------------

/// `HKCU\Software\OpenDSS\MainSect` — the key `TIniRegSave.Create('\Software\' +
/// ProgramName)` opens with `ProgramName := 'OpenDSS'`
/// (r4133 `Common/DSSGlobals.pas:2093`, `:2078`; `Shared/IniRegSave.pas:63-71`).
const REG_KEY: &str = r"HKCU\Software\OpenDSS\MainSect";

/// Read one `REG_SZ` value with `reg.exe query`; `None` when absent.
fn reg_read(name: &str) -> Option<String> {
    let out = Command::new("reg")
        .args(["query", REG_KEY, "/v", name])
        .output()
        .expect("run reg query");
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        if parts.next() == Some(name) && parts.next() == Some("REG_SZ") {
            return parts.next().map(str::to_string);
        }
    }
    None
}

/// Restore one `REG_SZ` value with `reg.exe add` (used only on the failure path,
/// so a regression cannot leave the machine's key poisoned).
fn reg_write(name: &str, value: &str) {
    let ok = Command::new("reg")
        .args([
            "add", REG_KEY, "/v", name, "/t", "REG_SZ", "/d", value, "/f",
        ])
        .status()
        .expect("run reg add")
        .success();
    assert!(
        ok,
        "could not restore value {name:?} under {REG_KEY} to {value:?}"
    );
}

/// D13 (i): **`clear` must reset `DefaultBaseFreq` to the port's 60 Hz.**
///
/// r4133's `clear` (`Executive/ExecHelper.pas:987-995` → `TExecutive.Clear`,
/// `Executive/Executive.pas:234-275`) resets `DefaultEarthModel`, `LogQueries`
/// and `MaxAllocationIterations` and nothing else; `DefaultBaseFreq` survives, so
/// a 50 Hz deck used to leak its base frequency into every later deck the same
/// worker compiled (`TDSSCircuit.Create` takes `Fundamental := DefaultBaseFreq`,
/// `Common/Circuit.pas:416`). `Engine::clear` now issues
/// `Set DefaultBaseFrequency=60` after the `clear`, matching the port's fresh
/// `Dss::new()` (`crates/dss-core/src/exec/construct.rs:173`).
///
/// The pre-`clear` half of the test is the live control: it proves the observable
/// really moves with `DefaultBaseFreq`, so the post-`clear` `60` is a reset and
/// not a constant. With the reset removed both post-`clear` reads come back `50`
/// (measured 2026-09-04 on a throwaway copy of the bridge).
///
/// `Get DefaultBaseFrequency` needs an active circuit — `DoGetCmd_NoCircuit`
/// (`Executive/ExecOptions.pas:1510`) does not serve option 73 — hence the
/// `new circuit.…` before each read.
#[test]
fn clear_resets_the_default_base_frequency_to_sixty() {
    let mut w = WorkerProc::spawn();

    w.exec("Set DefaultBaseFrequency=50");
    w.exec("new circuit.d13probe50 basekv=12.47 phases=3 bus1=b1");
    assert_eq!(
        w.exec("Get DefaultBaseFrequency")["reply"],
        json!("50"),
        "the 50 Hz control did not take — the observable is not live"
    );
    assert_eq!(
        w.exec("? Vsource.source.frequency")["reply"],
        json!("50"),
        "the default Vsource did not follow DefaultBaseFreq"
    );

    // The protocol `clear` runs `Engine::clear`.
    w.ok(json!({"cmd": "clear"}));

    w.exec("new circuit.d13probe60 basekv=12.47 phases=3 bus1=b1");
    assert_eq!(
        w.exec("Get DefaultBaseFrequency")["reply"],
        json!("60"),
        "clear left DefaultBaseFreq at the previous deck's value"
    );
    assert_eq!(
        w.exec("? Vsource.source.frequency")["reply"],
        json!("60"),
        "the circuit built after clear inherited a stale Fundamental"
    );

    w.quit();
}

/// D13 (iii): **a fresh session must not inherit the registry's base frequency.**
///
/// `Set RegistryUpdate=No` (see `Engine::new`) stops this process from *writing*
/// the key, but the *read* is already done by then: `ReadDSS_Registry` assigns
/// `DefaultBaseFreq := StrToInt(DSS_Registry.ReadString('BaseFrequency', '60'))`
/// (r4133 `Common/DSSGlobals.pas:1005`) inside `TExecutive.Create`
/// (`Executive/Executive.pas:124`), i.e. at DLL load, before the bridge can
/// issue a single command. Every gate path clears first (`run_case` →
/// `Engine::clear`, `capture.rs`), but a bare probe session that only issues
/// `exec` never does, so `Engine::new` issues `Set DefaultBaseFrequency=60`
/// itself — the port's own starting value (`Dss::new()` sets
/// `default_base_freq: 60.0`, `crates/dss-core/src/exec/construct.rs:173`).
///
/// The test pre-poisons the machine key with `50` — the one non-60 base
/// frequency the corpus actually uses (`LVTestCase/Master.dss`) — spawns a fresh
/// worker and reads the default back **without any `clear`**. With the init
/// reset removed the read comes back `50` (measured 2026-09-04 on a throwaway
/// copy of the bridge). The saved value is restored on every exit path, so a
/// regression cannot leave the machine's key poisoned.
///
/// `Get DefaultBaseFrequency` needs an active circuit — `DoGetCmd_NoCircuit`
/// (`Executive/ExecOptions.pas:1510`) does not serve option 73 — hence the
/// `new circuit.…`, which is not a `clear` and does not touch
/// `DefaultBaseFreq`.
#[test]
fn init_resets_the_default_base_frequency_to_sixty() {
    let before = reg_read("BaseFrequency");
    reg_write("BaseFrequency", "50");

    let mut w = WorkerProc::spawn();
    w.exec("new circuit.d13init basekv=12.47 phases=3 bus1=b1");
    let got = w.exec("Get DefaultBaseFrequency")["reply"].clone();
    let src = w.exec("? Vsource.source.frequency")["reply"].clone();
    w.quit();

    // Restore the machine key before any assertion can unwind.
    match before.as_deref() {
        Some(v) => reg_write("BaseFrequency", v),
        None => reg_write("BaseFrequency", "60"),
    }

    assert_eq!(
        got,
        json!("60"),
        "a fresh worker inherited BaseFrequency=50 from {REG_KEY}: \
         `Set DefaultBaseFrequency=60` is missing from Engine::new \
         (key restored to {before:?})"
    );
    assert_eq!(
        src,
        json!("60"),
        "the first circuit of a bare probe session took its Fundamental \
         from the registry (key restored to {before:?})"
    );
}

/// D13 (ii): **the worker must not write `HKCU\Software\OpenDSS` on exit.**
///
/// r4133 reads `BaseFrequency` from that key at DLL load
/// (`Common/DSSGlobals.pas:1005` from `TExecutive.Create`,
/// `Executive/Executive.pas:124`) and writes it back at process exit when
/// `UpdateRegistry` is true (`Common/DSSGlobals.pas:1015`, `:1022`, from
/// `TExecutive.Destroy`, `Executive/Executive.pas:141`, reached through the unit
/// `Finalization` at `Common/DSSGlobals.pas:2159`). `UpdateRegistry` defaults to
/// `TRUE` (`:2131`), so every unguarded worker leaked its last base frequency to
/// the next worker process **on the whole machine** — the cross-worktree channel
/// D13 closes. `Engine::new` now issues `Set RegistryUpdate=No`
/// (`Executive/ExecOptions.pas:146` names option 102; `:574` sets it with no
/// circuit active).
///
/// The flag has no readable getter to assert against: r4133's `DoGetCmd` case 102
/// assigns `UpdateRegistry := InterpretYesNo(Param)` instead of appending a result
/// (`Executive/ExecOptions.pas:1314`), so `Get RegistryUpdate` returns `""` and
/// silently clobbers the flag. The registry value itself is the observable.
///
/// The sentinel is `37` so a *different* worker exiting concurrently (another
/// lane's gate) cannot green or red this test by accident — no deck and no
/// default produces 37. Measured 2026-09-04 without the fix: the key moved
/// `60 -> 37`; with it, unchanged.
#[test]
fn the_worker_never_writes_the_opendss_registry_key() {
    let before = reg_read("BaseFrequency");

    let mut w = WorkerProc::spawn();
    // Not followed by a `clear`, so this value is what `WriteDSS_Registry` would
    // persist at process exit.
    w.exec("Set DefaultBaseFrequency=37");
    w.quit();

    let after = reg_read("BaseFrequency");
    if after.as_deref() == Some("37") {
        // Regression: restore the machine's key before failing.
        match before.as_deref() {
            Some(v) => reg_write("BaseFrequency", v),
            None => reg_write("BaseFrequency", "60"),
        }
        panic!(
            "the worker wrote BaseFrequency=37 into {REG_KEY}: \
             `Set RegistryUpdate=No` is missing from Engine::new \
             (key restored to {before:?})"
        );
    }
    assert_ne!(
        after.as_deref(),
        Some("37"),
        "the BaseFrequency value under {REG_KEY} carries the worker's sentinel"
    );
}
