//! Checkpointed-model gate (golden-infrastructure plan): run **every** scenario
//! file in `tests/golden/checkpoints/` (one file per scenario) and, at every
//! committed time step, compare the **assembled electrical model** against the
//! pinned oracle — not just the converged outputs. A checkpoint is the state
//! after one `solve` (number=1) returns. Each step pins:
//!
//!   - `dblHour`, iteration count, converged flag (exact);
//!   - node order (exact) and node voltages;
//!   - the assembled, unfactored system Y matrix (entry-by-entry on micro/feeder
//!     scenarios, fingerprint on large) — `Dss::system_y_csc` vs the oracle
//!     `YMatrix.getYSparse(False)`;
//!   - selected element YPrim blocks — `Dss::element_yprim` vs `CktElement.Yprim`;
//!   - injection vector, selected element currents/powers, discrete state.
//!
//! The per-step comparison layer — `compare_system_y`, `compare_fingerprint`,
//! `compare_yprim`, `compare_injection`, `compare_element`, `compare_discrete`,
//! the model structs, and `tol_for` — lives in `harness/mod.rs`, so the live
//! corpus gate (`corpus_live.rs`) reuses the identical logic.
//!
//! Adding a scenario is just adding a `<name>.json` to that directory (the
//! generator writes one per `SCENARIOS` entry). Regenerate only manually:
//! `python tools/golden/gen_checkpoints.py [scenario...]`.

mod harness;

use std::collections::BTreeMap;
use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::{
    ElementCap, Injection, YFingerprint, YMat, YPrim, compare_discrete, compare_element,
    compare_fingerprint, compare_injection, compare_system_y, compare_yprim, tol_for,
};
use serde::Deserialize;

/// One scenario file: `{schema, oracle, scenario}` (the `oracle` provenance
/// block is ignored here).
#[derive(Debug, Deserialize)]
struct ScenarioFile {
    schema: u32,
    scenario: Scenario,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    kind: String,
    /// Optional master to `compile` (relative to `tests/corpus/electricdss-tst`)
    /// before replaying `commands` — large feeders that are not inlined.
    #[serde(default)]
    master: Option<String>,
    commands: Vec<String>,
    n_steps: usize,
    node_order: Vec<String>,
    selected_elements: Vec<String>,
    checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Deserialize)]
struct Checkpoint {
    dbl_hour: f64,
    iterations: i32,
    converged: bool,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    /// Full assembled-Y CSC (micro/feeder); `None` on large feeders, which pin
    /// only the fingerprint + selected YPrim blocks.
    #[serde(default)]
    y: Option<YMat>,
    y_fingerprint: YFingerprint,
    yprims: Vec<YPrim>,
    elements: Vec<ElementCap>,
    injection: Injection,
    transformers: BTreeMap<String, Vec<f64>>,
    regcontrols: BTreeMap<String, i32>,
    capacitors: BTreeMap<String, Vec<i32>>,
}

/// Load every `*.json` scenario file from `tests/golden/checkpoints/`, sorted by
/// file name for deterministic run order.
fn load_scenarios() -> Vec<Scenario> {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "checkpoints",
    ]
    .iter()
    .collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no checkpoint scenarios in {}",
        dir.display()
    );
    files
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
            let f: ScenarioFile = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("cannot parse {}: {e}", p.display()));
            assert_eq!(f.schema, 2, "{}: checkpoints schema mismatch", p.display());
            f.scenario
        })
        .collect()
}

fn run_scenario(sc: &Scenario) {
    let tol = tol_for(&sc.kind);
    let mut dss = Dss::new();
    dss.command("clear");
    // Large feeders compile an unmodified master first (relative to
    // tests/corpus/electricdss-tst), exactly like golden_feeders_controls.rs.
    if let Some(master) = &sc.master {
        let path: PathBuf = [
            env!("CARGO_MANIFEST_DIR"),
            "..",
            "..",
            "tests",
            "corpus",
            "electricdss-tst",
        ]
        .iter()
        .collect::<PathBuf>()
        .join(master);
        assert!(
            path.is_file(),
            "{}: master script missing: {}",
            sc.name,
            path.display()
        );
        dss.command(&format!(
            "compile \"{}\"",
            path.to_string_lossy().replace('\\', "/")
        ));
    }
    for cmd in &sc.commands {
        dss.command(cmd);
    }
    assert!(
        dss.errors().is_empty(),
        "{}: unexpected engine errors: {:?}",
        sc.name,
        dss.errors()
    );
    assert_eq!(
        sc.checkpoints.len(),
        sc.n_steps,
        "{}: malformed golden",
        sc.name
    );

    for (i, cp) in sc.checkpoints.iter().enumerate() {
        dss.command("solve");
        assert!(
            dss.errors().is_empty(),
            "{} step {i}: engine errors: {:?}",
            sc.name,
            dss.errors()
        );
        let ctx = format!("{} step {i}", sc.name);
        {
            let ckt = dss.circuit().expect("circuit exists");
            assert!(cp.converged, "{ctx}: oracle did not converge");
            assert!(ckt.is_solved, "{ctx}: Rust did not converge");
            assert!(
                (ckt.solution.dbl_hour - cp.dbl_hour).abs() < 1e-12,
                "{ctx}: dblHour {} vs {}",
                ckt.solution.dbl_hour,
                cp.dbl_hour
            );
            assert_eq!(
                ckt.solution.iteration, cp.iterations,
                "{ctx}: iteration count differs"
            );
            // Node order (captured once in the golden; constant across steps).
            let names: Vec<String> = (1..=ckt.num_nodes).map(|j| ckt.node_name(j)).collect();
            assert_eq!(names, sc.node_order, "{ctx}: node order differs");

            // Node voltages.
            let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
            for j in 1..=ckt.num_nodes {
                actual.push(ckt.solution.node_v[j].re);
                actual.push(ckt.solution.node_v[j].im);
            }
            let mut expected = Vec::with_capacity(actual.len());
            for (re, im) in cp.v_re.iter().zip(&cp.v_im) {
                expected.push(*re);
                expected.push(*im);
            }
            harness::assert_complex_close(&actual, &expected, tol.v_rel, tol.v_abs, &ctx);
        }

        // Assembled system Y: full entry-by-entry when the golden carries the
        // CSC (micro/feeder); always the fingerprint (large feeders store only
        // that to keep the golden small).
        if let Some(y) = &cp.y {
            compare_system_y(&mut dss, y, &sc.node_order, &tol, &ctx);
        }
        compare_fingerprint(&mut dss, &cp.y_fingerprint, &tol, &ctx);

        // Selected element YPrim blocks.
        for yp in &cp.yprims {
            compare_yprim(&dss, yp, &tol, &ctx);
        }

        // Node injection vector (the solver RHS).
        compare_injection(&dss, &cp.injection, &tol, &ctx);

        // Selected element terminal currents/powers.
        let snaps = dss.snapshot_elements();
        for ec in &cp.elements {
            compare_element(&snaps, ec, &tol, &ctx);
        }

        // Per-step discrete control state (exact).
        compare_discrete(
            &dss,
            &cp.transformers,
            &cp.regcontrols,
            &cp.capacitors,
            &ctx,
        );
    }

    // The selected element list is informational; make sure the golden's
    // YPrim captures cover it.
    for cp in &sc.checkpoints {
        let captured: Vec<&str> = cp.yprims.iter().map(|y| y.name.as_str()).collect();
        for sel in &sc.selected_elements {
            assert!(
                captured.iter().any(|c| c.eq_ignore_ascii_case(sel)),
                "{}: golden missing YPrim for selected element {sel}",
                sc.name
            );
        }
    }
}

#[test]
fn checkpoint_scenarios_match_oracle() {
    for sc in &load_scenarios() {
        run_scenario(sc);
    }
}
