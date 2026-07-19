//! Golden generator for the WASM_USERMODELS **WM.4** Storage/PVSystem gate
//! (recorded decision, same channel as WM.3: the in-house **r4133 bridge**, not
//! pinned dss-python). Drives each `tools/golden/wasm_decks/wasm_{pv_pflow,
//! storage_dyn}.dss` deck — with `@FIXTURE@` substituted by the **native `.dll`
//! twin** of the wm4model reference model — through the r4133 engine
//! (`dss_epri::Engine`) and writes the captured element state-variable surface +
//! node voltages as the committed golden (`tests/golden/wasm_usermodels/
//! wasm_{pv_pflow,storage_dyn}.json`). The hermetic Rust-engine replay
//! (`crates/dss-core/tests/wasm_usermodels_wm4.rs`) compares the committed
//! `.wasm` fixture against these goldens.
//!
//! This is a NEW test file (the WM.3 `gen_wasm_usermodels.rs` is untouched); the
//! wm4model native twin and the wm4model `.wasm` are independent builds of the
//! SAME model core, so the comparison is a real cross-engine oracle check — never
//! "Rust agrees with itself".
//!
//! **Manual only** (like every golden): env-gated on `WASM_TWIN_DLL`, so
//! `cargo test` skips it. Regenerate with:
//! ```text
//! pwsh tools/wasm_usermodel/build_native.ps1   # builds the wm4model.dll twin
//! WASM_TWIN_DLL="$TEMP/wm4model_native_twin/wm4model.dll" \
//!   cargo test -p dss-epri --test gen_wasm_usermodels_wm4 -- --nocapture --ignored
//! ```

#![cfg(windows)]

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// (deck basename, active element to probe) pairs.
const DECKS: &[(&str, &str)] = &[
    ("wasm_pv_pflow", "PVSystem.pv1"),
    ("wasm_storage_dyn", "Storage.s1"),
];

#[test]
#[ignore = "manual golden generation — needs WASM_TWIN_DLL (the wm4model.dll twin)"]
fn generate_wm4_usermodel_goldens() {
    let Ok(twin) = std::env::var("WASM_TWIN_DLL") else {
        eprintln!("WASM_TWIN_DLL unset — skipping golden generation (manual step).");
        return;
    };
    let twin = PathBuf::from(&twin);
    assert!(twin.is_file(), "twin DLL not found: {}", twin.display());
    let twin_abs = twin
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/");

    let dll = dss_epri::smoke::dll_path();
    let engine = dss_epri::Engine::new(&dll).expect("load r4133 DLL");
    eprintln!("r4133 engine: {}", engine.version());

    let out_dir = workspace_root().join("tests/golden/wasm_usermodels");
    std::fs::create_dir_all(&out_dir).unwrap();
    let scratch = workspace_root().join("target/wasm_decks_scratch_wm4");
    std::fs::create_dir_all(&scratch).unwrap();

    for (deck, elem) in DECKS {
        let golden = capture_deck(&engine, deck, elem, &twin_abs, &scratch, engine.version());
        let path = out_dir.join(format!("{deck}.json"));
        std::fs::write(&path, serde_json::to_string_pretty(&golden).unwrap() + "\n").unwrap();
        eprintln!("wrote {}", path.display());
    }
}

fn capture_deck(
    engine: &dss_epri::Engine,
    deck: &str,
    elem: &str,
    twin_abs: &str,
    scratch: &Path,
    version: &str,
) -> serde_json::Value {
    let template = workspace_root().join(format!("tools/golden/wasm_decks/{deck}.dss"));
    let text = std::fs::read_to_string(&template)
        .unwrap_or_else(|e| panic!("read {}: {e}", template.display()));
    let deck_text = text.replace("@FIXTURE@", twin_abs);
    let deck_path = scratch.join(format!("{deck}.dss"));
    std::fs::write(&deck_path, &deck_text).unwrap();

    engine.clear().unwrap();
    // `compile` runs the whole deck INCLUDING its final `Solve` (snapshot or the
    // completed dynamics run) — the deck is the single source of the run length,
    // symmetric with the Rust replay.
    engine
        .compile(&deck_path.to_string_lossy().replace('\\', "/"), true)
        .unwrap_or_else(|e| panic!("{deck}: compile: {e}"));

    engine.set_active_element(elem);
    let names = engine.element_variable_names();
    let values = engine.element_variable_values();
    eprintln!(
        "[probe] {deck}: element {elem}, {} vars, converged={}, iters={}",
        names.len(),
        engine.converged(),
        engine.iterations()
    );
    assert_eq!(
        names.len(),
        values.len(),
        "{deck}: oracle name/value length mismatch"
    );

    let order = engine.ynode_order();
    let varray = engine.ynode_varray();
    let mut node_v = serde_json::Map::new();
    for (k, name) in order.iter().enumerate() {
        node_v.insert(
            name.clone(),
            serde_json::json!([varray[2 * k], varray[2 * k + 1]]),
        );
    }

    serde_json::json!({
        "deck": deck,
        "element": elem,
        "engine": version,
        "oracle": "r4133 bridge (dss-epri) + wm4model.dll native twin",
        "converged": engine.converged(),
        "iterations": engine.iterations(),
        "variable_names": names,
        "variable_values": values,
        "node_voltages": node_v,
    })
}
