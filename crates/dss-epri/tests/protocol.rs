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
        WorkerProc::spawn_in(None)
    }

    /// [`WorkerProc::spawn`] with the child's working directory pinned — the
    /// D39 init-trace probe needs a directory it owns and can sweep, so that
    /// "the throwaway init circuit wrote nothing" is a measurement and not a
    /// claim.
    fn spawn_in(dir: Option<&std::path::Path>) -> WorkerProc {
        let bin = env!("CARGO_BIN_EXE_epri-worker");
        let mut cmd = Command::new(bin);
        if let Some(d) = dir {
            cmd.current_dir(d);
        }
        let mut child = cmd
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

/// G1.0 audit settlement: the [`dss_epri::modes::DO_NOT_CALL`] register is
/// enforced at the FFI **chokepoint** (`Engine::ffi_dispatch`), not only inside
/// the typed accessors — so the worker's raw `{"cmd":"ffi"}` channel, the one a
/// probe author reaches for first, cannot dispatch either memory-unsafe mode.
///
/// Both are r4133 defects: `Solution` V:2 `BusLevels` writes `ArrSize+1`
/// elements into an `ArrSize`-long array (`DSolution.pas:580-582`), and `Bus`
/// V:17 `ZSC012Matrix` calls `Zsc.MtrxMult(As2p)` with no `Assigned(Zsc)` guard
/// (`DBus.pas:803-838`) — a **measured** process kill of this very worker on a
/// bus with no fault study (G1.0 probe, 2026-09-04). The `ping` at the end is
/// the non-vacuity: a refusal that still dispatched would take the worker with
/// it and the test could not reach it.
#[test]
fn the_raw_ffi_command_refuses_the_do_not_call_modes() {
    let mut w = WorkerProc::spawn();
    // A live circuit, so the refusal is not an artefact of there being nothing
    // to read: `Bus` V:17 kills the worker exactly on a solved deck.
    for line in DECK {
        w.exec(line);
    }
    w.exec("solve");
    for (family, mode, name) in [("Solution", 2, "BusLevels"), ("Bus", 17, "ZSC012Matrix")] {
        let r = w.request(json!({"cmd": "ffi", "family": family, "kind": "v", "mode": mode}));
        assert_eq!(
            r["ok"], false,
            "{family} V:{mode} ({name}) must be refused by the chokepoint: {r}"
        );
        let err = r["error"].as_str().unwrap_or_default();
        assert!(
            err.contains("do-not-call"),
            "{family} V:{mode} ({name}) was refused for the wrong reason: {r}"
        );
    }
    let pong = w.ok(json!({"cmd": "ping"}));
    assert_eq!(
        pong["pong"], true,
        "the worker did not survive the do-not-call requests"
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
/// `CDANG(a) = ATAN2(a.re, a.im) * 57.29577951` (`Ucomplex.pas:118-121`) over
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

/// Put the key back exactly as it was found: rewrite the saved value, or DELETE
/// the value when the machine did not have one (G1.4a audit settlement T6 —
/// writing a default `60` there would create machine state the test found
/// absent, and r4133 reads that key at DLL load).
fn reg_restore(name: &str, before: Option<&str>) {
    match before {
        Some(v) => reg_write(name, v),
        None => reg_delete(name),
    }
}

/// Delete one value with `reg.exe delete`; a missing value is already the
/// wanted state, so only a *present* value that survives is an error.
fn reg_delete(name: &str) {
    let _ = Command::new("reg")
        .args(["delete", REG_KEY, "/v", name, "/f"])
        .output()
        .expect("run reg delete");
    assert!(
        reg_read(name).is_none(),
        "could not remove value {name:?} under {REG_KEY} (the test found it absent)"
    );
}

/// Restore one `REG_SZ` value with `reg.exe add`.
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

/// [`reg_restore`]'s absent-value branch, driven on a scratch value name (the
/// live tests below find `BaseFrequency` present on a developed machine, so
/// their restore takes the rewrite branch). Proves that "restored" means the key
/// is left exactly as found — the value is removed, not created with a default
/// r4133 would then read at DLL load (G1.4a audit settlement T6).
#[test]
fn reg_restore_removes_a_value_the_machine_did_not_have() {
    const PROBE: &str = "DssRsSettleProbe";
    assert!(
        reg_read(PROBE).is_none(),
        "{PROBE} under {REG_KEY} is not a scratch name after all — pick another"
    );
    reg_write(PROBE, "37");
    assert_eq!(
        reg_read(PROBE).as_deref(),
        Some("37"),
        "the scratch write did not take — the probe proves nothing"
    );
    reg_restore(PROBE, None);
    assert!(
        reg_read(PROBE).is_none(),
        "restoring an absent value must DELETE it, not write a default"
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
/// copy of the bridge). The machine key is put back on every exit path — the
/// saved value rewritten, or the value DELETED when the machine had none
/// ([`reg_restore`]) — so a regression cannot leave it poisoned.
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
    reg_restore("BaseFrequency", before.as_deref());

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
    // Restore the machine's key first, so the assertion below is the only exit
    // (G1.4a audit settlement T6: the old shape restored inside an `if` and left
    // an unreachable `assert_ne!` behind it).
    reg_restore("BaseFrequency", before.as_deref());
    assert_ne!(
        after.as_deref(),
        Some("37"),
        "the worker wrote BaseFrequency=37 into {REG_KEY}: \
         `Set RegistryUpdate=No` is missing from Engine::new \
         (key restored to {before:?})"
    );
}

// ---------------------------------------------------------------------------
// D25 — the bridge must not fire the OS editor on `Show`/`Dump`
// ---------------------------------------------------------------------------

/// The editor the bridge installs at init (`Engine::new`). Kept here so the test
/// below and the init sequence cannot drift apart silently.
const BRIDGE_EDITOR: &str = "rundll32.exe";

/// Two program names no machine resolves. `A` poisons `HKCU\Software\OpenDSS`
/// so the init override has something to override; `B` is issued *inside* the
/// worker, so a worker that persisted its editor on exit would leave `B` where
/// `A` is expected. Both are deliberately harmless: if a crash ever leaves one
/// behind, r4133's `FireOffEditor` takes the `ERROR_FILE_NOT_FOUND` branch
/// (`Common/Utilities.pas:310`, message 702) and spawns nothing — strictly safer
/// than the `Notepad.exe` default it replaces.
const EDITOR_SENTINEL_A: &str = "DssRsEditorSentinelA.exe";
const EDITOR_SENTINEL_B: &str = "DssRsEditorSentinelB.exe";

/// D25: **a fresh session must not inherit the registry's OS editor, and must
/// not write its own back.**
///
/// r4133 keeps `AutoDisplayShowReport := TRUE` (`Common/DSSGlobals.pas:2052`)
/// and every `Show` writer ends with
/// `If AutoDisplayShowReport Then FireOffEditor(FileNm)`
/// (`Common/ShowResults.pas:403`, `:717`, `:1116` … `:2904`;
/// `Common/ControlQueue.pas:482`; `Common/Solution.pas:3543`), while `Dump`
/// (`Executive/ExecHelper.pas:1357`), the hash-list dumps (`:1223`, `:1232`,
/// `:1241`, `:1249`), `VDIFF` (`:3373`) and `Show autoadded`
/// (`Executive/ShowOptions.pas:208`) call it unconditionally — `DoShowCmd`
/// (`Executive/ShowOptions.pas:156`) has no `NoFormsAllowed` guard, so
/// `DSSI(8, 0)` does not reach it. On Windows `FireOffEditor` is a
/// `ShellExecute` of `DefaultEditor` (`Common/Utilities.pas:304`), i.e. one OS
/// process per report; `DefaultEditor` is read from the machine key at DLL load
/// with the default `'Notepad.exe'` (`Common/DSSGlobals.pas:990`), which is how
/// ~900 orphaned notepads accumulated across the lanes before D25.
///
/// That read happens before the bridge gets control, so `Engine::new` overwrites
/// the variable instead (`Set Editor=`, served with no circuit active by
/// `DoSetCmd_NoCircuit`, `Executive/ExecOptions.pas:570`), *after*
/// `Set RegistryUpdate=No` — because `WriteDSS_Registry` persists
/// `DefaultEditor` next to `BaseFrequency` (`Common/DSSGlobals.pas:1017`, under
/// the `UpdateRegistry` test at `:1015`, from `TExecutive.Destroy` → the unit
/// `Finalization`), and leaving `rundll32.exe` as the machine-wide OpenDSS
/// editor would be state escaping the worker.
///
/// One test, three assertions, so nothing races on the same registry value:
/// with the init command removed the first read returns
/// [`EDITOR_SENTINEL_A`]; the liveness control proves `Get Editor` tracks
/// `DefaultEditor` rather than replying a constant; and the post-exit read
/// returns `A`, not the `B` the worker itself last held. `Get Editor` (option
/// 15, `Executive/ExecOptions.pas:1206`) is served only by `DoGetCmd`, which
/// needs an active circuit — hence the `new circuit.…`. The key is put back on
/// every exit path ([`reg_restore`] — rewritten, or DELETED when the machine had
/// none) before any assertion can unwind.
#[test]
fn init_overrides_the_os_editor_and_never_writes_it_back() {
    let before = reg_read("Editor");
    reg_write("Editor", EDITOR_SENTINEL_A);

    let mut w = WorkerProc::spawn();
    w.exec("new circuit.d25init basekv=12.47 phases=3 bus1=b1");
    let got = w.exec("Get Editor")["reply"].clone();
    // Liveness control + the write-back probe: this is what the worker would
    // persist at exit if `Set RegistryUpdate=No` were missing or too late.
    w.exec(&format!("Set Editor={EDITOR_SENTINEL_B}"));
    let moved = w.exec("Get Editor")["reply"].clone();
    w.quit();

    let after = reg_read("Editor");
    reg_restore("Editor", before.as_deref());

    assert_eq!(
        got,
        json!(BRIDGE_EDITOR),
        "a fresh worker inherited Editor={EDITOR_SENTINEL_A:?} from {REG_KEY}: \
         `Set Editor={BRIDGE_EDITOR}` is missing from Engine::new — every \
         `Show`/`Dump` would ShellExecute the machine's editor \
         (key restored to {before:?})"
    );
    assert_eq!(
        moved,
        json!(EDITOR_SENTINEL_B),
        "`Get Editor` did not follow a later `Set Editor`, so the value above is \
         not a live read of DefaultEditor (key restored to {before:?})"
    );
    assert_eq!(
        after.as_deref(),
        Some(EDITOR_SENTINEL_A),
        "the worker rewrote Editor under {REG_KEY} (found {after:?}): \
         `Set RegistryUpdate=No` no longer precedes `Set Editor={BRIDGE_EDITOR}` \
         in Engine::new (key restored to {before:?})"
    );
}

// ---------------------------------------------------------------------------
// D30 — the r4133 event-log capture reads in memory, not through a file
// ---------------------------------------------------------------------------

/// Decode `<CircuitName>_EXP_EventLog.CSV` exactly as the retired Oddie capture
/// path in `capture.rs::capture_eventlog` did: strip a leading UTF-8 file BOM,
/// then a per-line BOM and the CR, and drop blank lines. Kept here (and only
/// here) so the deleted decode stays available as the *reference* the in-memory
/// read is measured against.
fn decode_exported_event_log(path: &std::path::Path) -> Vec<String> {
    let bytes = std::fs::read(path)
        .unwrap_or_else(|e| panic!("`export eventlog` file {} unreadable: {e}", path.display()));
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    let text = String::from_utf8_lossy(body);
    text.split('\n')
        .map(|raw| {
            raw.trim_start_matches('\u{FEFF}')
                .trim_end_matches(['\r', '\n'])
        })
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// D30 (class A): **the event log the r4133 transport reports must be the
/// in-memory `Solution.EventLog`, and it must equal what `export eventlog`
/// writes.**
///
/// `capture.rs::capture_eventlog` used to issue `export eventlog` and read the
/// CSV back. That command is a *write*: r4133 `Common/ExportResults.pas:3527-3532`
/// (`ExportEventLog` = `EventStrings[ActiveActor].SaveToFile`) drops
/// `<CircuitName>_EXP_EventLog.CSV` (`Executive/ExportOptions.pas:365`) into
/// `OutputDirectory` — a file neither the capi transport (`ckt.Solution.EventLog`,
/// dss_capi 0.14.5 `src/CAPI/CAPI_Solution.pas:525-540`) nor the port ever
/// creates, so G1.10a's created-file-set comparison reddened on **59** (case,
/// channel) pairs that were purely our own capture's artifact. The fix is to read
/// the same list both other producers read: `SolutionV(0)`
/// (`Version8/Source/DDLL/DSolution.pas:518`, `:526-541`), i.e.
/// [`dss_epri::dss::Engine::eventlog`], blank-filtered.
///
/// The equivalence was measured corpus-wide before the switch — the export path
/// and the in-memory read compared line-for-line inside `capture_eventlog` over a
/// full 526-case gate drive (83 cases with `evlog=1`): **0 mismatches**. This test
/// is that assertion, promoted to a deck that actually logs events: [`DECK`]
/// applies `Fault.f` at t=0.2 s and blows `Fuse.fz` at t=0.4 s, so the log is
/// non-empty and the comparison cannot pass vacuously.
///
/// The export runs **here only**, in a scratch directory the test removes, so the
/// gate's own r4133 runs no longer write it.
#[test]
fn the_in_memory_event_log_equals_the_exported_file() {
    let scratch = std::env::temp_dir().join(format!(
        "dss-rs-g110a-evlog-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&scratch).expect("create scratch dir");

    let mut w = WorkerProc::spawn();
    w.ok(json!({"cmd": "chdir", "dir": scratch.to_string_lossy()}));
    w.exec("clear");
    for c in DECK {
        w.exec(c);
    }
    // Five duty steps: fault applied at t=0.2 s, fuse blown at t=0.4 s.
    for _ in 0..5 {
        w.exec("solve");
    }

    // The capture's read: `SolutionV(0)`, blank lines dropped.
    let in_memory: Vec<String> = w
        .read("eventlog")
        .as_array()
        .expect("eventlog is an array")
        .iter()
        .map(|l| l.as_str().expect("eventlog line is a string").to_string())
        .filter(|l| !l.trim().is_empty())
        .collect();

    // The retired path, run once, here.
    let reply = w.exec("export eventlog")["reply"]
        .as_str()
        .expect("`export eventlog` reply is a string")
        .trim()
        .trim_start_matches('\u{FEFF}')
        .to_string();
    w.quit();

    let exported_path = std::path::PathBuf::from(&reply);
    let existed = exported_path.is_file();
    let from_file = if existed {
        decode_exported_event_log(&exported_path)
    } else {
        Vec::new()
    };

    // Clean up before asserting: the file the export wrote (wherever
    // `OutputDirectory` put it) and the scratch directory.
    if existed {
        let _ = std::fs::remove_file(&exported_path);
    }
    let swept = std::fs::remove_dir_all(&scratch);

    assert!(
        existed,
        "`export eventlog` reported {reply:?}, which is not a file — the decode \
         reference below would be vacuous"
    );
    assert!(
        exported_path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.to_ascii_lowercase().ends_with("_exp_eventlog.csv")),
        "`export eventlog` wrote {reply:?}, not <CircuitName>_EXP_EventLog.CSV \
         (ExportOptions.pas:365) — the file this capture must not create has \
         been renamed upstream"
    );
    assert!(
        in_memory.len() >= 2,
        "the deck logged {} event line(s) — the fault/fuse events are missing, \
         so the equality below proves nothing: {in_memory:?}",
        in_memory.len()
    );
    assert!(
        in_memory[0].starts_with("Hour=0, Sec=0.2,") && in_memory[0].contains("Fault.f"),
        "event-log format drifted: {in_memory:?}"
    );
    assert!(
        in_memory.iter().any(|l| l.contains("BLOWN")),
        "fuse blow missing from the in-memory event log: {in_memory:?}"
    );
    assert_eq!(
        in_memory, from_file,
        "the in-memory `Solution.EventLog` (SolutionV(0)) and the CSV \
         `export eventlog` wrote disagree — `capture.rs::capture_eventlog` may \
         not read in memory (D30 class A), because the two are no longer the \
         same list"
    );
    swept.expect("remove the scratch dir");
}

/// D30 (class A), the behavioural half: **a `run` that captures the event log
/// must create no file.**
///
/// The equivalence above is why reading in memory is *allowed*; this is why it
/// is *required*. The gate's r4133 transport used to leave
/// `<CircuitName>_EXP_EventLog.CSV` in the case directory on every
/// event-logging case, which is invisible to every model comparison and shows up
/// only in G1.10a's created-file SET — where it reddened 59 (case, channel)
/// pairs against a capi channel and a port that never write it. So this drives
/// the real capture path (`capture::run_case` with `eventlog: true`) over
/// [`DECK`] and asserts, from the run's own `run_files` report, that the set is
/// empty: the same `CorpusGuard` pass that sweeps the case dir also classifies
/// what the run created, so nothing can be missed by looking in the wrong place.
/// The event log is asserted non-empty in the same reply, so an empty file set
/// cannot come from a capture that did nothing.
#[test]
fn the_event_log_capture_creates_no_file() {
    let scratch = std::env::temp_dir().join(format!(
        "dss-rs-g110a-evrun-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&scratch).expect("create scratch dir");
    let deck = scratch.join("evlog_case.dss");
    std::fs::write(&deck, format!("{}\n", DECK.join("\n"))).expect("write deck");

    let mut w = WorkerProc::spawn();
    let result = w.ok(json!({
        "cmd": "run",
        "case_path": deck.to_string_lossy(),
        "n_steps": 5,
        "eventlog": true,
        "run_files": true,
    }));
    w.quit();

    // `expect`, not `unwrap_or_default`: an ABSENT (or null) `run_files` key
    // would otherwise read as "the run created nothing" and green this test
    // while proving nothing — the very claim it exists to make (G1.10a audit
    // settlement, finding AC-1/AT-2).
    let created: Vec<String> = result["run_files"]
        .as_array()
        .expect("the run must report its created-file set (`run_files`)")
        .iter()
        .map(|n| {
            n.as_str()
                .expect("every created-file entry is a string")
                .to_string()
        })
        .collect();
    let log: Vec<String> = result["checkpoints"]
        .as_array()
        .and_then(|cps| cps.last())
        .and_then(|cp| cp["eventlog"].as_array())
        .map(|a| {
            a.iter()
                .map(|l| l.as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default();

    let swept = std::fs::remove_dir_all(&scratch);

    assert!(
        !log.is_empty(),
        "the run captured no event log, so an empty created-file set proves \
         nothing: {result}"
    );
    assert!(
        log.iter().any(|l| l.contains("BLOWN")),
        "the fuse never blew, so the capture is not exercising a real log: {log:?}"
    );
    assert_eq!(
        created,
        Vec::<String>::new(),
        "an event-log capture created {created:?} — `capture_eventlog` is issuing \
         `export eventlog` again (r4133 `Common/ExportResults.pas:3527-3532`), a \
         file neither the capi channel nor the port writes (D30 class A)"
    );
    swept.expect("remove the scratch dir");
}

// ---------------------------------------------------------------------------
// D39 (G1.10a F0′) — the bridge gags report auto-display with the engine's own
// switches; `Set Editor=` is only the safety net
// ---------------------------------------------------------------------------

/// A fresh scratch directory under the OS temp dir, unique per process + call.
fn scratch_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "dss-rs-g110a-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// The sorted file names directly under `dir`.
fn dir_entries(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read scratch dir")
        .map(|e| {
            e.expect("scratch dir entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    names.sort();
    names
}

/// Every running PID with this image name (`tasklist /FO CSV /NH`), so a test
/// can diff the set around a command and see only the processes it caused —
/// a machine-wide count would be at the mercy of the developer's own windows
/// and of the other lanes' gates.
fn image_pids(image: &str) -> std::collections::BTreeSet<u32> {
    let out = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {image}"), "/FO", "CSV", "/NH"])
        .output()
        .expect("run tasklist");
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .filter_map(|l| {
            let mut fields = l.split("\",\"");
            let name = fields.next()?.trim_start_matches('"');
            if !name.eq_ignore_ascii_case(image) {
                return None;
            }
            fields.next()?.trim_matches('"').parse::<u32>().ok()
        })
        .collect()
}

/// PIDs of `image` that are new since `before` and still alive after up to
/// `wait`; an editor that exits on its own drains this set, one that lingers
/// does not. Polled, never slept blindly, so a fast exit does not cost a second.
fn new_pids_still_alive(
    image: &str,
    before: &std::collections::BTreeSet<u32>,
    wait: std::time::Duration,
) -> std::collections::BTreeSet<u32> {
    let deadline = std::time::Instant::now() + wait;
    loop {
        let live: std::collections::BTreeSet<u32> =
            image_pids(image).difference(before).copied().collect();
        if live.is_empty() || std::time::Instant::now() >= deadline {
            return live;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// A self-contained feeder whose script carries one report of each *guarded*
/// kind: five `Show` writers (gated by `AutoDisplayShowReport`, r4133
/// `Common/ShowResults.pas:403` …) and two `Export`s (gated by
/// `AutoShowExport`, `Executive/ExportOptions.pas:517`). Compiled from a
/// scratch directory, so every file it writes is swept with that directory
/// and no vendored corpus deck is touched.
const SHOW_DECK: &[&str] = &[
    "new circuit.f0prime basekv=12.47 phases=3 bus1=src mvasc3=20000 mvasc1=21000",
    "new linecode.lc nphases=3 r1=0.3 x1=0.6 r0=0.7 x0=1.9 c1=0 c0=0 units=mi",
    "new line.feed bus1=src bus2=mid linecode=lc length=1 units=mi",
    "new load.l bus1=mid phases=3 kv=12.47 kw=500 pf=0.95 model=1",
    "set voltagebases=[12.47]",
    "calcvoltagebases",
    "solve",
    "Show Voltages LN Nodes",
    "Show Currents Elements",
    "Show Powers kVA Elements",
    "Show Losses",
    "Show Buses",
    "Export Voltages",
    "Export Currents",
];

/// D39 layers 1+2: **the init switches survive a `Compile`, and no *guarded*
/// `FireOffEditor` site fires — while every report is still written.**
///
/// The observable is `DSS error #702`, not a process count: `FireOffEditor`
/// reports `ERROR_FILE_NOT_FOUND` through `DoSimpleMsg(…, 702)` (r4133
/// `Common/Utilities.pas:304`, `:310`), so with an editor no machine can start
/// ([`EDITOR_SENTINEL_A`]) a site that fires *raises* instead of spawning
/// anything. That makes the suppression deterministic to assert and safe to
/// drive — a process count would race the other lanes' gates and the
/// developer's own windows. The liveness control is in the same test: the
/// **unguarded** `Dump` site (`Executive/ExecHelper.pas:1357`) must raise #702
/// with that very editor, so the silence above cannot be the sentinel failing
/// to bite.
///
/// The guarded reports are therefore issued *directly*, one per request, and
/// the `Compile` is only what proves the switches survive a deck: `ProcessCommand`
/// resets `ErrorNumber := 0` at the start of every command
/// (`Executive/ExecCommands.pas:604`), so a #702 raised by a `Show` inside a
/// compiled script is wiped by the script's next line and never reaches the
/// caller (measured 2026-09-11 — the deck below returns `ok` even with
/// `AutoDisplayShowReport` left on).
///
/// The `ShowExport` layer is driven two-sided for the same reason: its flag is
/// already `FALSE` at unit initialization (`Common/DSSGlobals.pas:2051`), so
/// `Get ShowExport = No` alone would be vacuous — the test flips it on, sees
/// the guarded `Export` fire (#702), flips it off and sees it silent.
/// `AllowForms` is asserted one-sided by design: `Set AllowForms=Yes` would let
/// the next `DoSimpleMsg` open a modal dialog (`Common/DSSGlobals.pas:651-660`)
/// and hang this headless worker for good.
///
/// Non-vacuity (driven in-tree 2026-09-11, each edit reverted and the diff
/// re-checked): with `Set ShowReports=No` deleted from `Engine::new` the direct
/// `Show Voltages LN Nodes` comes back `DSS error #702 (exec: Show Voltages LN
/// Nodes): Editor "DssRsEditorSentinelA.exe"  Not Found.` and the test reds on
/// that assertion; with `new circuit.dssrs_bridge_init` deleted the worker never
/// starts — `epri-worker: cannot load …: DSS error #301 (init: Set
/// ShowReports=No): You must create a new circuit object first` — which is the
/// throwaway circuit earning its place.
#[test]
fn report_switches_survive_a_compile_and_gag_every_guarded_editor_site() {
    let scratch = scratch_dir("f0prime-switches");
    let deck = scratch.join("f0prime_shows.dss");

    // The worker's working directory is this scratch dir while `Engine::new`
    // runs, so "the throwaway `new circuit.dssrs_bridge_init` leaves no trace"
    // is measured, not assumed.
    let mut w = WorkerProc::spawn_in(Some(&scratch));
    let after_init = dir_entries(&scratch);

    // Option 149 is served without a circuit (`ExecOptions.pas:1561`); 138/71
    // are not (`DoGetCmd_NoCircuit` `:1510` has no case for them), so those two
    // are read after the compile below.
    let allow_forms_bare = w.exec("Get AllowForms")["reply"].clone();

    // Pin the output directory so every report lands where this test sweeps
    // (`Set DataPath`, option 57, served with no circuit — `:571`).
    w.exec(&format!("Set DataPath={}", scratch.display()));
    // From here on any `FireOffEditor` that fires raises #702 instead of
    // starting a process.
    w.exec(&format!("Set Editor={EDITOR_SENTINEL_A}"));

    std::fs::write(&deck, format!("{}\n", SHOW_DECK.join("\n"))).expect("write deck");
    let compiled = w.request(json!({
        "cmd": "exec",
        "text": format!("Compile \"{}\"", deck.display()),
    }));

    // The guarded `Show` writers, one command per request: `ProcessCommand`
    // resets `ErrorNumber := 0` at the start of EVERY command
    // (`Executive/ExecCommands.pas:604`), so a #702 raised by a `Show` *inside*
    // a compiled script is wiped by the next line of that script and never
    // reaches the caller (measured 2026-09-11: the deck above returns `ok` even
    // with `AutoDisplayShowReport` on). Issued directly, the errno survives into
    // the reply — which is what makes these two assertions worth anything.
    let shown_ln = w.request(json!({"cmd": "exec", "text": "Show Voltages LN Nodes"}));
    let shown_buses = w.request(json!({"cmd": "exec", "text": "Show Buses"}));

    // The compile ran `new circuit.f0prime`: if a circuit reset any of the
    // three flags, these reads are where it shows.
    let show_reports = w.exec("Get ShowReports")["reply"].clone();
    let show_export = w.exec("Get ShowExport")["reply"].clone();
    let allow_forms = w.exec("Get AllowForms")["reply"].clone();

    // Liveness control: the unguarded site, same sentinel editor.
    let unguarded_dump = w.request(json!({"cmd": "exec", "text": "Dump"}));

    // The `ShowExport` layer, both ways round.
    w.exec("Set ShowExport=Yes");
    let export_shown = w.request(json!({"cmd": "exec", "text": "Export Voltages"}));
    w.exec("Set ShowExport=No");
    let export_gagged = w.request(json!({"cmd": "exec", "text": "Export Voltages"}));

    let reports = dir_entries(&scratch);
    w.quit();
    let swept = std::fs::remove_dir_all(&scratch);

    assert_eq!(
        after_init,
        Vec::<String>::new(),
        "`Engine::new` left {after_init:?} in its working directory — the D39 \
         init sequence must build and drop `circuit.dssrs_bridge_init` without \
         touching the disk (`MakeNewCircuit`, `Common/DSSGlobals.pas:793-836`)"
    );
    assert_eq!(
        allow_forms_bare,
        json!("No"),
        "a fresh worker reports AllowForms={allow_forms_bare} before any deck: \
         `Set AllowForms=No` is missing from Engine::new"
    );
    assert_eq!(
        compiled.get("ok").and_then(Value::as_bool),
        Some(true),
        "the Show-carrying deck did not compile, so nothing below is measured \
         on a real report run: {compiled}"
    );
    for (cmd, reply) in [
        ("Show Voltages LN Nodes", &shown_ln),
        ("Show Buses", &shown_buses),
    ] {
        assert_eq!(
            reply.get("ok").and_then(Value::as_bool),
            Some(true),
            "`{cmd}` fired its `FireOffEditor` (r4133 \
             `Common/ShowResults.pas:403` …), so `Set ShowReports=No` did not \
             reach r4133 — with the editor gagged the writer must close its \
             file and stop there: {reply}"
        );
    }
    assert_eq!(
        (
            show_reports.clone(),
            show_export.clone(),
            allow_forms.clone()
        ),
        (json!("No"), json!("No"), json!("No")),
        "a `Compile` (which runs `new circuit.…`) moved the report switches: \
         ShowReports={show_reports}, ShowExport={show_export}, \
         AllowForms={allow_forms} — they are unit globals no `clear` and no \
         circuit may touch (`Common/DSSGlobals.pas:2051-2052`)"
    );
    let dump_err = unguarded_dump
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert!(
        unguarded_dump.get("ok").and_then(Value::as_bool) == Some(false)
            && dump_err.contains("#702"),
        "the unguarded `Dump` site (`Executive/ExecHelper.pas:1357`) did not \
         fire the sentinel editor, so the silence above proves nothing about \
         the switches: {unguarded_dump}"
    );
    let export_err = export_shown
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert!(
        export_shown.get("ok").and_then(Value::as_bool) == Some(false)
            && export_err.contains("#702"),
        "`Set ShowExport=Yes` did not make `Export Voltages` fire the editor \
         (`Executive/ExportOptions.pas:517`), so reading the flag back as `No` \
         would prove nothing: {export_shown}"
    );
    assert_eq!(
        export_gagged.get("ok").and_then(Value::as_bool),
        Some(true),
        "`Set ShowExport=No` did not gag the same `Export`: {export_gagged}"
    );
    // The whole point of D39: the viewer is gone, the reports are not.
    let written = |suffix: &str| {
        reports
            .iter()
            .any(|n| n.to_ascii_lowercase().ends_with(suffix))
    };
    assert!(
        written("_vln_node.txt") && written("_exp_voltages.csv") && reports.len() >= 8,
        "the switches suppressed the reports themselves, not just the viewer \
         launch — the deck's five Shows, two Exports, its own source and the \
         `Dump` should all be on disk, found {reports:?}"
    );
    swept.expect("remove the scratch dir");
}

/// D39 layer 3: **the safety net is live where no switch reaches.**
///
/// `Dump` (r4133 `Executive/ExecHelper.pas:1357`) is one of the 12
/// `FireOffEditor` call sites neither `ShowReports`, `ShowExport` nor
/// `AllowForms` guards, so whatever `DefaultEditor` holds is what it runs. With
/// an editor no machine can start the site raises `DSS error #702`
/// (`Common/Utilities.pas:310`) — which is how this test knows the site fires
/// at all — and with the bridge's own `rundll32.exe` it must neither raise nor
/// leave a process behind.
///
/// That `rundll32.exe` is what a fresh worker holds is asserted here and proven
/// against a poisoned machine key by
/// [`init_overrides_the_os_editor_and_never_writes_it_back`], which is also the
/// test that pins the registry write-back; this one never touches the registry,
/// so the two cannot race.
///
/// Non-vacuity (driven in-tree 2026-09-11, each edit reverted and the diff
/// re-checked): with `Engine::new` installing [`EDITOR_SENTINEL_A`] instead of
/// `rundll32.exe` the test reds on the first assertion (`a fresh worker holds
/// Editor="DssRsEditorSentinelA.exe"`); with the sentinel half pointed at a
/// *guarded* writer (`Show Voltages LN Nodes`) instead of `Dump` it reds on the
/// liveness assertion with `{"ok":true,…}`, i.e. this test really is aimed at a
/// site no switch covers.
#[test]
fn the_editor_safety_net_covers_the_sites_no_switch_guards() {
    let scratch = scratch_dir("f0prime-safetynet");
    let notepads_before = image_pids("notepad.exe");
    let rundll_before = image_pids(BRIDGE_EDITOR);

    let mut w = WorkerProc::spawn_in(Some(&scratch));
    w.exec(&format!("Set DataPath={}", scratch.display()));
    // `Get Editor` (option 15, `ExecOptions.pas:1206`) is served only by
    // `DoGetCmd`, which needs a circuit; `Dump` needs one too.
    w.exec("new circuit.f0primenet basekv=12.47 phases=3 bus1=b1");
    // Solved, so that pointing the sentinel half at a *guarded* writer instead
    // (the negative drive below) fails on the missing #702 and not on
    // `The circuit must be solved` (`Executive/ShowOptions.pas:198`).
    w.exec("solve");
    let installed = w.exec("Get Editor")["reply"].clone();

    // 1. The site is live: an editor that cannot be started makes it raise.
    w.exec(&format!("Set Editor={EDITOR_SENTINEL_A}"));
    let with_sentinel = w.request(json!({"cmd": "exec", "text": "Dump"}));

    // 2. The safety net makes the same site harmless.
    w.exec(&format!("Set Editor={BRIDGE_EDITOR}"));
    let with_bridge = w.request(json!({"cmd": "exec", "text": "Dump"}));
    let lingering = new_pids_still_alive(
        BRIDGE_EDITOR,
        &rundll_before,
        std::time::Duration::from_secs(10),
    );
    let new_notepads: Vec<u32> = image_pids("notepad.exe")
        .difference(&notepads_before)
        .copied()
        .collect();

    w.quit();
    let swept = std::fs::remove_dir_all(&scratch);

    assert_eq!(
        installed,
        json!(BRIDGE_EDITOR),
        "a fresh worker holds Editor={installed}, not the bridge's \
         {BRIDGE_EDITOR:?} — the D39 safety net is not installed"
    );
    let sentinel_err = with_sentinel
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert!(
        with_sentinel.get("ok").and_then(Value::as_bool) == Some(false)
            && sentinel_err.contains("#702"),
        "`Dump` did not fire the sentinel editor, so this test cannot tell a \
         working safety net from a dead call site: {with_sentinel}"
    );
    assert_eq!(
        with_bridge.get("ok").and_then(Value::as_bool),
        Some(true),
        "`Dump` with the bridge's editor {BRIDGE_EDITOR:?} raised — the safety \
         net must be a program that starts and exits, never one that errors \
         into the gate: {with_bridge}"
    );
    assert_eq!(
        lingering,
        std::collections::BTreeSet::new(),
        "the safety-net editor left {lingering:?} running after 10 s: \
         `rundll32.exe` is chosen because it exits immediately on a non-DLL \
         argument — a lingering process is what D25 measured for `where.exe`, \
         `cmd.exe` and `PING.EXE`"
    );
    assert_eq!(
        new_notepads,
        Vec::<u32>::new(),
        "the run started notepad(s) {new_notepads:?} — the machine editor \
         reached a report despite the D39 init sequence"
    );
    swept.expect("remove the scratch dir");
}
