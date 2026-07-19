//! `epri-worker` — the persistent r4133 bridge worker (UNIFIED_GATE_PLAN.md
//! §3.2). Speaks the same line-JSON `ping`/`run`/`quit` protocol as
//! `tools/oracle/oracle_server.py`, producing byte-compatible `CaseResult`
//! responses so the `harness/mod.rs` comparators accept it unchanged
//! (bit-compat vs the outgoing Python/Oddie path was proven by
//! `xcheck_bridge.py` before that whole stack — script included — was
//! retired in UNIFIED_GATE Phase E).
//!
//! Modes:
//! - `--smoke` — run the oracle-free self-smoke and exit (replaces
//!   `tools/opendss/smoke.py`).
//! - default — load the DLL once, assert the version pin, then serve requests on
//!   stdin until EOF / `quit`. One bad case returns `{"ok":false,...}` rather
//!   than killing the worker; a hard DLL crash (`#303`, never sent — ledgered as
//!   `skip`) dies the process, and the dispatcher respawns.
//!
//! Besides the gate's `ping`/`run`/`clear`/`quit`, the worker serves the
//! `exec`/`read`/`chdir` scripting commands (`dss_epri::script`) used by the
//! manual regen drivers and probes (`tools/opendss/epri_worker.py`) — the
//! functional-parity replacement for the retired Oddie bridge. The gate never
//! sends them.
//!
//! The EPRI capability round (Round 2) adds the process-isolated **capability**
//! surface that covers everything the Oddie/dss-python bridge could reach over the
//! same engine, plus the "beyond":
//! - `ffi` — generic `(family, kind, mode, arg)` dispatch reaching every mode of
//!   every DDLL family (`dss_epri::families`), including array get/set, with a
//!   structured per-call errno surface.
//! - `ymatrix` — the standalone Y-matrix / injection helpers (`YMatrix.*`).
//! - `caps` — a structured version/capability handshake (families + commands).
//! - `batch` — many `exec` commands in one round-trip (stop-on-first-error).
//!
//! The gate never sends any of these; the comparators/scheduler stay untouched.

#[cfg(windows)]
fn main() {
    use std::io::{BufRead, Write};

    use dss_epri::capture::{RunRequest, run_case};
    use dss_epri::dss::Engine;
    use dss_epri::smoke::{dll_path, expect_version, run_smoke};

    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--smoke") {
        match run_smoke() {
            Ok(rep) => {
                for l in &rep.lines {
                    println!("{l}");
                }
                eprintln!("epri-worker --smoke: SMOKE OK");
            }
            Err(e) => {
                eprintln!("epri-worker --smoke: SMOKE FAILED: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Init sequence (§2.2): load DLL -> DSSI(8,0) (inside Engine::new) -> version
    // assert vs revisions.json expect_version -> ready.
    let dll = dll_path();
    let engine = match Engine::new(&dll) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("epri-worker: cannot load {}: {e}", dll.display());
            std::process::exit(1);
        }
    };
    let expect = match expect_version() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("epri-worker: {e}");
            std::process::exit(1);
        }
    };
    let version = engine.version().trim().to_string();
    if !version.contains(&expect) {
        eprintln!(
            "epri-worker: engine {version:?} does not contain pinned {expect:?} (no silent pass)"
        );
        std::process::exit(1);
    }
    let dll_abs = std::fs::canonicalize(&dll)
        .map(|p| {
            p.to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/")
        })
        .unwrap_or_else(|_| engine.dll_path().to_string());
    let oracle = serde_json::json!({
        "engine": version,
        "epri": true,
        "rev": "r4133",
        "dll": dll_abs,
    });
    eprintln!("epri-worker ready: {oracle}");

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut reply = |v: serde_json::Value| {
        // Compact single line; NaN/Inf are not representable (never present in a
        // converged capture) — a serialize failure surfaces as an error line.
        match serde_json::to_string(&v) {
            Ok(s) => {
                let _ = out.write_all(s.as_bytes());
                let _ = out.write_all(b"\n");
                let _ = out.flush();
            }
            Err(e) => {
                let _ = out.write_all(
                    format!("{{\"ok\":false,\"error\":\"serialize: {e}\"}}\n").as_bytes(),
                );
                let _ = out.flush();
            }
        }
    };

    let stdin = std::io::stdin();
    let mut line = String::new();
    loop {
        line.clear();
        match stdin.lock().read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {}
            Err(e) => {
                eprintln!("epri-worker: stdin read error: {e}");
                break;
            }
        }
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let req: serde_json::Value = match serde_json::from_str(t) {
            Ok(v) => v,
            Err(e) => {
                reply(serde_json::json!({"ok": false, "error": format!("bad request json: {e}")}));
                continue;
            }
        };
        match req.get("cmd").and_then(|c| c.as_str()) {
            Some("quit") => break,
            Some("ping") => {
                reply(serde_json::json!({"ok": true, "result": {"pong": true, "oracle": oracle}}));
            }
            // ---- scripting surface (regen drivers / probes; never sent by the
            // gate — see `dss_epri::script`) --------------------------------
            Some("exec") => {
                let Some(text) = req.get("text").and_then(|t| t.as_str()) else {
                    reply(serde_json::json!({"ok": false, "error": "exec: missing `text`"}));
                    continue;
                };
                match engine.exec_wait(text) {
                    Ok(r) => reply(serde_json::json!({"ok": true, "result": {"reply": r}})),
                    Err(e) => reply(serde_json::json!({"ok": false, "error": e.to_string()})),
                }
            }
            Some("chdir") => {
                let Some(dir) = req.get("dir").and_then(|d| d.as_str()) else {
                    reply(serde_json::json!({"ok": false, "error": "chdir: missing `dir`"}));
                    continue;
                };
                match std::env::set_current_dir(dir) {
                    Ok(()) => reply(serde_json::json!({"ok": true, "result": {"cwd": dir}})),
                    Err(e) => {
                        reply(
                            serde_json::json!({"ok": false, "error": format!("chdir {dir}: {e}")}),
                        );
                    }
                }
            }
            Some("read") => match dss_epri::script::handle_read(&engine, &req) {
                Ok(v) => reply(serde_json::json!({"ok": true, "result": v})),
                Err(e) => reply(serde_json::json!({"ok": false, "error": e.to_string()})),
            },
            // ---- generic FFI capability channel (EPRI Round 2): reaches every
            // mode of every DDLL family + the Y-matrix helpers -----------------
            Some("ffi") => match dss_epri::script::handle_ffi(&engine, &req) {
                Ok(v) => reply(serde_json::json!({"ok": true, "result": v})),
                Err(e) => reply(serde_json::json!({"ok": false, "error": e.to_string()})),
            },
            Some("ymatrix") => match dss_epri::script::handle_ymatrix(&engine, &req) {
                Ok(v) => reply(serde_json::json!({"ok": true, "result": v})),
                Err(e) => reply(serde_json::json!({"ok": false, "error": e.to_string()})),
            },
            // Structured version/capability handshake (beyond the Python bridge):
            // the whole reachable surface in one round-trip.
            Some("caps") => {
                let families: Vec<serde_json::Value> = engine
                    .family_manifest()
                    .into_iter()
                    .map(|(n, k)| serde_json::json!({"name": n, "kinds": k}))
                    .collect();
                reply(serde_json::json!({"ok": true, "result": {
                    "protocol_version": 2,
                    "oracle": oracle,
                    "families": families,
                    "family_count": engine.family_count(),
                    "family_entry_points": engine.family_entry_points(),
                    "commands": [
                        "ping", "caps", "run", "clear", "exec", "batch",
                        "read", "ffi", "ymatrix", "chdir", "quit"
                    ],
                    "ymatrix_ops": [
                        "system_y_changed", "use_aux_currents", "build_y", "zero_inj",
                        "get_source_inj", "get_pc_inj", "add_aux", "y_dims",
                        "vpointer", "ipointer", "solve_system"
                    ],
                }}));
            }
            // Batched multi-command exec in ONE round-trip (beyond the Python
            // bridge's one-command-per-call scripting): stop-on-first-error, each
            // reply carried back with its index.
            Some("batch") => {
                let Some(items) = req.get("exec").and_then(|v| v.as_array()) else {
                    reply(serde_json::json!({"ok": false, "error": "batch: missing `exec` array"}));
                    continue;
                };
                let mut replies = Vec::with_capacity(items.len());
                let mut all_ok = true;
                let mut failed_at: Option<usize> = None;
                for (i, item) in items.iter().enumerate() {
                    let Some(text) = item.as_str() else {
                        replies.push(serde_json::json!({"ok": false, "error": "batch: non-string exec item"}));
                        all_ok = false;
                        failed_at = Some(i);
                        break;
                    };
                    match engine.exec_wait(text) {
                        Ok(r) => replies.push(serde_json::json!({"ok": true, "reply": r})),
                        Err(e) => {
                            replies.push(serde_json::json!({"ok": false, "error": e.to_string()}));
                            all_ok = false;
                            failed_at = Some(i);
                            break;
                        }
                    }
                }
                let ran = replies.len();
                reply(serde_json::json!({"ok": all_ok, "result": {
                    "replies": replies,
                    "ran": ran,
                    "failed_at": failed_at,
                }}));
            }
            Some("clear") => {
                // Release the circuit (and any held loadshape memory-mapped file
                // handles) so a second process can compile the same case without a
                // concurrent-mapping conflict (protocol convenience).
                let ok = engine.clear().is_ok();
                reply(serde_json::json!({"ok": ok, "result": {"cleared": ok}}));
            }
            Some("run") => {
                let run_req: RunRequest = match serde_json::from_value(req) {
                    Ok(r) => r,
                    Err(e) => {
                        reply(
                            serde_json::json!({"ok": false, "error": format!("bad run request: {e}")}),
                        );
                        continue;
                    }
                };
                match run_case(&engine, &run_req) {
                    Ok(cr) => match serde_json::to_value(&cr) {
                        Ok(v) => reply(serde_json::json!({"ok": true, "result": v})),
                        Err(e) => {
                            reply(
                                serde_json::json!({"ok": false, "error": format!("serialize result: {e}")}),
                            );
                        }
                    },
                    Err(e) => {
                        eprintln!("epri-worker: case failed: {e}");
                        reply(serde_json::json!({"ok": false, "error": e.to_string()}));
                    }
                }
            }
            other => {
                reply(serde_json::json!({"ok": false, "error": format!("unknown cmd {other:?}")}));
            }
        }
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("epri-worker: the EPRI r4133 bridge is Windows-only (no DLL on this platform)");
    std::process::exit(1);
}
