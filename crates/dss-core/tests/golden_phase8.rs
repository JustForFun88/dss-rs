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
use harness::{ColTol, ExportPolicy, RowPolicy, compare_export};
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
        col_tol: vec![],
    };
    compare_export(&oracle, &rust, &policy, "export_counts");

    std::fs::remove_dir_all(&scratch).ok();
}

/// Meta for a solution-export golden (a feeder compiled + solved on both
/// engines, then one report captured): the master to compile (relative to
/// `tests/corpus/electricdss-tst`), the post commands, the circuit/CaseName, and
/// the oracle's default-filename suffix.
#[derive(Debug, Deserialize)]
struct FeederMeta {
    report: String,
    master: String,
    post: Vec<String>,
    fixture: String,
    suffix: String,
}

/// Drive one solution export: compile the same master the oracle used, replay
/// the post commands, route the report into a scratch dir, export, and diff the
/// produced file against the captured oracle file via `compare_export`.
fn run_feeder_export(stem: &str, policy: &ExportPolicy) {
    let dir = phase8_dir();
    let meta: FeederMeta = {
        let p = dir.join(format!("{stem}.meta.json"));
        let text =
            std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", p.display()))
    };
    let oracle = {
        let p = dir.join(format!("{stem}.txt"));
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    };

    let master: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "corpus",
        "electricdss-tst",
    ]
    .iter()
    .collect::<PathBuf>()
    .join(&meta.master);
    assert!(master.is_file(), "master missing: {}", master.display());

    let scratch = scratch_dir(stem);
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command(&format!(
        "compile \"{}\"",
        master.to_string_lossy().replace('\\', "/")
    ));
    for c in &meta.post {
        dss.command(c);
    }
    dss.command(&format!("set datapath=\"{}\"", scratch.display()));
    dss.command(&format!("export {}", meta.report));
    assert!(dss.errors().is_empty(), "{stem}: {:?}", dss.errors());

    let produced = dss.last_result_file();
    let want = format!("{}_{}", meta.fixture, meta.suffix).to_lowercase();
    assert!(
        produced.to_lowercase().ends_with(&want),
        "{stem}: unexpected produced path {produced:?} (want …{want})"
    );
    let rust = std::fs::read_to_string(produced)
        .unwrap_or_else(|e| panic!("read produced {produced}: {e}"));

    compare_export(&oracle, &rust, policy, stem);

    std::fs::remove_dir_all(&scratch).ok();
}

/// Default per-number tolerance for the `%g`-formatted value columns of the
/// solution exports: the oracle writes 5–6 significant digits, so the report is
/// known only to ~1e-5 rel; `1e-4` clears that formatting floor plus the two
/// independent solves with margin. The **primary** voltage-correctness gate is
/// the live full-model compare (`corpus_live.rs`, 1e-8 rel) — this golden pins
/// the report *layout* (header, column set/order, row order, scaling), not the
/// physics. See `tests/TOLERANCE_NOTES.md`.
const EXPORT_REL: f64 = 1e-4;
const EXPORT_ABS: f64 = 1e-6;

/// `Export Voltages` (Pascal `ExportVoltages`) on solved IEEE13: per-bus node
/// magnitude/angle/pu, zero-filled to the max node count.
#[test]
fn export_voltages_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        // The `Angle%d` columns are `%6.1f` (one decimal); two independent solves
        // round that last 0.1 digit independently → a ±0.1 formatting floor. The
        // magnitude/pu columns keep the tight default. (tests/TOLERANCE_NOTES.md)
        col_tol: vec![ColTol {
            prefix: "angle".to_string(),
            rel: 1e-3,
            abs: 0.11,
        }],
    };
    run_feeder_export("export_voltages", &policy);
}

/// `Export BusCoords` (Pascal `ExportBusCoords`): X/Y of every coord-defined bus.
/// No header row; coordinates are `%-13.11g` (11 sig) loaded from the same file,
/// so they match tightly.
#[test]
fn export_buscoords_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: EXPORT_REL,
        abs: EXPORT_ABS,
        col_tol: vec![],
    };
    run_feeder_export("export_buscoords", &policy);
}

/// `Export NodeNames` (Pascal `ExportNodeNames`): `BusName.NodeNum` per line,
/// pure text (compared case-insensitively).
#[test]
fn export_nodenames_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 1,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_nodenames", &policy);
}

/// `Export YNodeList` (Pascal `ExportYNodeList`): node names in Y-matrix order,
/// quoted, pure text.
#[test]
fn export_ynodelist_matches_oracle() {
    let policy = ExportPolicy {
        sep: ',',
        header_lines: 0,
        rows: RowPolicy::ExactOrdered,
        rel: 0.0,
        abs: 0.0,
        col_tol: vec![],
    };
    run_feeder_export("export_ynodelist", &policy);
}
