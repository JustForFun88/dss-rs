//! Golden generator for the WASM_USERMODELS WM.3 Generator gate (recorded
//! decision 2026-07-19: the oracle channel is the in-house **r4133 bridge**,
//! not pinned dss-python). Drives each `tools/golden/wasm_decks/*.dss` deck —
//! with `@FIXTURE@` substituted by the **native `.dll` twin** of the reference
//! model — through the r4133 engine (`dss_epri::Engine`) and writes the captured
//! Generator state-variable surface + node voltages as the committed golden
//! (`tests/golden/wasm_usermodels/*.json`). The hermetic Rust-engine replay
//! (`crates/dss-core/tests/wasm_usermodels.rs`) compares the committed `.wasm`
//! fixture against these goldens at the harness tier floors.
//!
//! **Manual only** (like every golden): env-gated on `WASM_TWIN_DLL`, so
//! `cargo test` skips it. Regenerate with:
//! ```text
//! pwsh tools/wasm_usermodel/build_native.ps1   # builds the r4133 twin
//! WASM_TWIN_DLL="$TEMP/indmach012a_native_twin/IndMach012a.dll" \
//!   cargo test -p dss-epri --test gen_wasm_usermodels -- --nocapture --ignored
//! ```

#![cfg(windows)]

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// The gate decks (basename under `tools/golden/wasm_decks/`). Each deck is
/// self-contained and ends with its own final `Solve` (a snapshot power-flow, or
/// a completed dynamics run), so the capture reads the post-run state directly —
/// the generator never issues an extra solve (which would advance a dynamics deck
/// one step past its intended endpoint). Both the oracle capture here and the
/// hermetic Rust replay (`crates/dss-core/tests/wasm_usermodels.rs`) run the deck
/// verbatim and read the same surface, so the comparison stays symmetric.
const DECKS: &[&str] = &[
    "wasm_gen_pflow", // Model=User (GenModel=6) power-flow snapshot
    "wasm_gen_dyn",   // GenModel=6 + ShaftModel dynamics run (monitors modes 1/3)
    "wasm_gen_vars",  // state-variable surface (`? Generator.g1.<var>` probes)
    "wasm_gen_edit",  // mid-script `UserData=` re-edit before the final solve
];

#[test]
#[ignore = "manual golden generation — needs WASM_TWIN_DLL (the FPC-built r4133 twin)"]
fn generate_wasm_usermodel_goldens() {
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
    let scratch = workspace_root().join("target/wasm_decks_scratch");
    std::fs::create_dir_all(&scratch).unwrap();

    for deck in DECKS {
        let golden = capture_deck(&engine, deck, &twin_abs, &scratch, engine.version());
        let path = out_dir.join(format!("{deck}.json"));
        std::fs::write(&path, serde_json::to_string_pretty(&golden).unwrap() + "\n").unwrap();
        eprintln!("wrote {}", path.display());
    }
}

fn capture_deck(
    engine: &dss_epri::Engine,
    deck: &str,
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
    // `compile` runs the whole deck INCLUDING its final `Solve` (snapshot or a
    // completed dynamics run). No extra solve — the deck is the single source of
    // the run length, symmetric with the Rust replay.
    engine
        .compile(&deck_path.to_string_lossy().replace('\\', "/"), true)
        .unwrap_or_else(|e| panic!("{deck}: compile: {e}"));

    // Generator state-variable surface (the classic 6 + the user-model vars).
    engine.set_active_element("Generator.g1");
    let names = engine.element_variable_names();
    let values = engine.element_variable_values();
    eprintln!(
        "[probe] {deck}: {} vars; ShaftModel=`{}` UserModel=`{}` names={:?}",
        names.len(),
        engine.raw_command("? Generator.g1.ShaftModel"),
        engine.raw_command("? Generator.g1.UserModel"),
        names
    );
    // Emit the variable surface as two PARALLEL, ORDER-PRESERVING arrays (never a
    // name-keyed map): the Generator's state-variable surface contains DUPLICATE
    // names when the SAME model is bound as both `UserModel=` and `ShaftModel=`
    // (the dyn deck: 6 built-in + 14 UserModel + 14 ShaftModel = 34 vars, the
    // ShaftModel's 14 names duplicating the UserModel's). A JSON object would
    // silently collapse those 14 duplicates to 20 unique keys — hiding the
    // ShaftModel surface entirely. Parallel arrays pin the full ordered surface.
    assert_eq!(
        names.len(),
        values.len(),
        "{deck}: oracle name/value length mismatch"
    );

    // Node voltages (name → [re, im]) in the Y node order.
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
        "engine": version,
        "oracle": "r4133 bridge (dss-epri) + FPC-built IndMach012a.dll twin",
        "converged": engine.converged(),
        "iterations": engine.iterations(),
        "variable_names": names,
        "variable_values": values,
        "node_voltages": node_v,
    })
}
