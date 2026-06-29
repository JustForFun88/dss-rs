//! Phase 8 report goldens (PHASE8_PLAN §2.3): pin the **report file the oracle
//! writes**. `tools/golden/gen_phase8.py` runs a fixture on the pinned engine,
//! issues the `Export`/`Show`/... command, and captures the produced file's
//! bytes into `tests/golden/phase8/<report>.txt` (+ a `<report>.meta.json` with
//! the exact deck, so the Rust and oracle fixtures can never drift). This driver
//! replays the same deck, writes its own report into a temp dir, and diffs the
//! two **after parsing numbers out** via `harness::compare_export` — never a raw
//! float-string diff.
//!
//! WP8.1 self-test: `Export Counts` — a text dump of every DSS class and its
//! instance count. The Rust class registry is a *proper subset* of the oracle's
//! (only a subset of classes is ported), so the comparison is `RustSubsetByKey`:
//! every ported class's count is pinned against the oracle; the classes we don't
//! yet register are ignored (see `tests/TOLERANCE_NOTES.md`). This is a real
//! gate (it shakes out the whole output path: registry walk → format → file IO →
//! the `compare_export` harness), not a self-comparison.
//!
//! Regenerate only manually: `python tools/golden/gen_phase8.py`.

mod harness;

use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::{ExportPolicy, RowPolicy, compare_export};
use serde::Deserialize;

fn phase8_dir() -> PathBuf {
    [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "phase8",
    ]
    .iter()
    .collect()
}

#[derive(Debug, Deserialize)]
struct ReportMeta {
    report: String,
    fixture: String,
    deck: Vec<String>,
}

/// A unique scratch dir for this test process (no `tempfile` dep; cleaned up at
/// the end). `Set DataPath=` points the engine's report output here.
fn scratch_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dss_phase8_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("mkdir {}: {e}", d.display()));
    d
}

/// `Export Counts` (Pascal `ExportCounts`): every class + its instance count.
/// The Rust file must be a `RustSubsetByKey` subset of the oracle's (the Rust
/// class registry is a proper subset during the port); shared classes' counts
/// are pinned exactly (integers — zero tolerance).
#[test]
fn export_counts_matches_oracle() {
    let dir = phase8_dir();
    let meta: ReportMeta = {
        let p = dir.join("export_counts.meta.json");
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    assert_eq!(meta.report, "Counts");
    let oracle = {
        let p = dir.join("export_counts.txt");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let scratch = scratch_dir("counts");
    let mut dss = Dss::new();
    dss.command("clear");
    for c in &meta.deck {
        dss.command(c);
    }
    // Route the report into the scratch dir (the same way the oracle fixture
    // did), then export. Quote the path in case of spaces.
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command("export counts");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let produced = dss.last_result_file();
    assert!(
        produced
            .to_lowercase()
            .ends_with(&format!("{}_exp_counts.csv", meta.fixture)),
        "unexpected produced path: {produced:?}"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    let policy = ExportPolicy {
        sep: '=',
        header_lines: 1, // "Format: DSS Class Name = Instance Count"
        rows: RowPolicy::RustSubsetByKey {
            key: 0,
            // Must-emit classes: the deck-created (Line/Load/Vsource) + the
            // default DSS items (TCC_Curve/Spectrum/LoadShape/GrowthShape). Guards
            // against a registry-walk regression that drops a class or empties the
            // body (the subset compare alone can't see a missing row).
            require: [
                "line",
                "load",
                "vsource",
                "tcc_curve",
                "spectrum",
                "loadshape",
                "growthshape",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect(),
        },
        rel: 0.0,
        abs: 0.0, // integer counts — exact
    };
    compare_export(&oracle, &rust, &policy, "export_counts");

    std::fs::remove_dir_all(&scratch).ok();
}
