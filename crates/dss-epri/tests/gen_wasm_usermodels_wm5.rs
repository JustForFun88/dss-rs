//! Golden generator for the CapControl user-control gate, on the r4133 bridge.
//!
//! **The reference is r4133's BUILT-IN VOLTAGE CapControl, not a native twin.**
//! A native `TCapUserControl` DLL **cannot drive the r4133 control queue**: the
//! model gets no pointer to its owning CapControl, nothing in the sampling path
//! makes the CapControl the active element, and the queue needs the CapControl
//! as owner to route the pending action on pop. So the reference `capuserctl`
//! fixture's deadband logic is made identical to the built-in VOLTAGE control
//! (OnSetting=vlow / OffSetting=vhigh), and the r4133 built-in VOLTAGE control
//! (`wasm_capcontrol_oracle.dss`) is the cross-engine reference for the Rust
//! USERCONTROL wiring.
//!
//! This generator has two jobs:
//!  1. **Golden** (always): run `wasm_capcontrol_oracle.dss` (built-in VOLTAGE)
//!     on the r4133 engine and write the ordered SWITCH actions + final cap state
//!     + node voltages as `tests/golden/wasm_usermodels/wasm_capcontrol.json`.
//!  2. **Twin confirmation** (when `WASM_TWIN_DLL` is set): run the
//!     USERCONTROL deck `wasm_capcontrol.dss` with `@FIXTURE@` = the native twin
//!     `capuserctl.dll` on the r4133 engine. The twin pushes with the only
//!     element pointer it can obtain, not the CapControl, so the queue pop
//!     dereferences a wrong control element and the r4133 engine hangs. Run it at
//!     your own risk: it does not return. The twin is built by
//!     `build_capuserctl_native.ps1`.
//!
//! # Knobs
//!
//! - `DSS_GEN_WM5`: set to any value, runs the generator. Unset by default, and
//!   then it returns without writing anything.
//! - `WASM_TWIN_DLL`: path of the native `capuserctl.dll` twin, arms the twin
//!   confirmation of job 2. Unset by default.
//!
//! **Manual only** (like every golden): `#[ignore]` and gated on `DSS_GEN_WM5`. Regenerate:
//! ```text
//! DSS_GEN_WM5=1 cargo test -p dss-epri --test gen_wasm_usermodels_wm5 -- --nocapture --ignored
//! # optional twin confirmation (after build_capuserctl_native.ps1):
//! DSS_GEN_WM5=1 WASM_TWIN_DLL="$TEMP/capuserctl_native_twin/capuserctl.dll" \
//!   cargo test -p dss-epri --test gen_wasm_usermodels_wm5 -- --nocapture --ignored
//! ```

#![cfg(windows)]

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Parse the ordered SWITCH actions `(Element, Action)` from raw event-log lines
/// (`Hour=…, Sec=…, ControlIter=…, Element=…, Action=…`), keeping only the actual
/// bank switches — **Opened**/**Closed**/**Step Up**/**Step Down** — and dropping
/// the **Armed**/**Reset** queue-mechanism rows (which the direct-push USERCONTROL
/// path legitimately omits). Actions normalized to uppercase.
fn switch_actions(log: &[String]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in log {
        let Some(elem) = field(line, "Element=") else {
            continue;
        };
        let Some(action) = field(line, "Action=") else {
            continue;
        };
        let a = action.trim().to_ascii_uppercase();
        if a.starts_with("**OPENED**")
            || a.starts_with("**CLOSED**")
            || a.starts_with("**STEP UP**")
            || a.starts_with("**STEP DOWN**")
        {
            // Keep the action label up to the first comma (drop trailing detail).
            let label = a.split(',').next().unwrap_or(&a).trim().to_string();
            out.push((elem.trim().to_string(), label));
        }
    }
    out
}

/// Extract a `key=value` field's value (up to the next comma) from a line.
fn field(line: &str, key: &str) -> Option<String> {
    let start = line.find(key)? + key.len();
    let rest = &line[start..];
    let end = rest.find(',').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

#[test]
#[ignore = "manual golden generation — needs DSS_GEN_WM5=1"]
fn generate_wm5_capcontrol_golden() {
    if std::env::var("DSS_GEN_WM5").is_err() {
        eprintln!("DSS_GEN_WM5 unset — skipping golden generation (manual step).");
        return;
    }

    let dll = dss_epri::smoke::dll_path();
    let engine = dss_epri::Engine::new(&dll).expect("load r4133 DLL");
    let version = engine.version().to_string();
    eprintln!("r4133 engine: {version}");

    // --- 1. Golden from the built-in VOLTAGE reference deck ---
    let oracle = workspace_root().join("tools/golden/wasm_decks/wasm_capcontrol_oracle.dss");
    engine.clear().unwrap();
    engine
        .compile(&oracle.to_string_lossy().replace('\\', "/"), true)
        .unwrap_or_else(|e| panic!("oracle compile: {e}"));

    let log = engine.eventlog();
    let actions = switch_actions(&log);
    eprintln!("[oracle] raw event log ({} lines):", log.len());
    for l in &log {
        eprintln!("    {l}");
    }
    eprintln!("[oracle] switch actions: {actions:?}");

    // Final capacitor states.
    let mut cap_states = serde_json::Map::new();
    if engine.capacitors_first() {
        loop {
            cap_states.insert(
                format!("Capacitor.{}", engine.capacitor_name()),
                serde_json::json!(engine.capacitor_states()),
            );
            if !engine.capacitors_next() {
                break;
            }
        }
    }

    // Node voltages (Y-node order).
    let order = engine.ynode_order();
    let varray = engine.ynode_varray();
    let mut node_v = serde_json::Map::new();
    for (k, name) in order.iter().enumerate() {
        node_v.insert(
            name.clone(),
            serde_json::json!([varray[2 * k], varray[2 * k + 1]]),
        );
    }

    assert!(
        !actions.is_empty(),
        "the reference deck produced no switch actions — it did not exercise the control"
    );

    let golden = serde_json::json!({
        "deck": "wasm_capcontrol",
        "engine": version,
        "oracle": "r4133 built-in VOLTAGE CapControl (native CapUserControl twin cannot drive \
                   the control queue)",
        "converged": engine.converged(),
        "switch_actions": actions.iter().map(|(e, a)| vec![e.clone(), a.clone()]).collect::<Vec<_>>(),
        "capacitor_states": cap_states,
        "node_voltages": node_v,
    });
    let out_dir = workspace_root().join("tests/golden/wasm_usermodels");
    std::fs::create_dir_all(&out_dir).unwrap();
    let path = out_dir.join("wasm_capcontrol.json");
    std::fs::write(&path, serde_json::to_string_pretty(&golden).unwrap() + "\n").unwrap();
    eprintln!("wrote {}", path.display());

    // --- 2. Empirical twin confirmation (optional) ---
    if let Ok(twin) = std::env::var("WASM_TWIN_DLL") {
        let twin = PathBuf::from(&twin);
        assert!(twin.is_file(), "twin DLL not found: {}", twin.display());
        let twin_abs = twin
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .replace('\\', "/");
        let template = workspace_root().join("tools/golden/wasm_decks/wasm_capcontrol.dss");
        let text = std::fs::read_to_string(&template).unwrap();
        let deck_text = text.replace("@FIXTURE@", &format!("\"{twin_abs}\""));
        let scratch = workspace_root().join("target/wasm_decks_scratch_wm5");
        std::fs::create_dir_all(&scratch).unwrap();
        let deck_path = scratch.join("wasm_capcontrol_twin.dss");
        std::fs::write(&deck_path, &deck_text).unwrap();

        eprintln!(
            "[twin] EXPECTED TO HANG: the compile below does not return — the twin pushes with \
             the wrong element as owner, so the control-queue pop dereferences a wrong control \
             element and corrupts the r4133 engine. If this call ever DOES return (engine \
             hardened), the event log should be empty (no correctly-routed switch)."
        );
        engine.clear().unwrap();
        let compile_res = engine.compile(&deck_path.to_string_lossy().replace('\\', "/"), true);
        // Only reached if the engine did NOT hang (see the warning above).
        eprintln!("[twin] compile result: {compile_res:?}");
        let twin_log = engine.eventlog();
        let twin_actions = switch_actions(&twin_log);
        eprintln!("[twin] raw event log ({} lines):", twin_log.len());
        for l in &twin_log {
            eprintln!("    {l}");
        }
        eprintln!(
            "[twin] EMPIRICAL CONFIRMATION (only if it returned) — switch actions via the native \
             twin: {twin_actions:?}"
        );
    } else {
        eprintln!(
            "[twin] WASM_TWIN_DLL unset — skipping the empirical twin confirmation (the source \
             proof stands; build_capuserctl_native.ps1 builds the twin)."
        );
    }
}
