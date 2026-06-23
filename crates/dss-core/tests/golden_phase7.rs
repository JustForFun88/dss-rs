//! Phase 7 golden (PHASE7_PLAN.md WP7.1 / §1 focused gate 1): command-replay
//! scenarios that pin the line-constants / geometry path against the pinned
//! oracle (`tools/golden/gen_phase7.py`). Each scenario builds a small circuit
//! whose Line gets its Z/Yc from the Carson engine (via `geometry=`, or
//! `spacing=` with `wires=`/`cncable=`), solves once, and must match its file
//! under `tests/golden/phase7/` (one `<scenario>.json` per scenario; the gate
//! runs every file in the dir):
//!
//!   - line_geometry: 3-phase overhead via `geometry=` (no reduce);
//!   - line_geometry_reduce: 3 phases + a neutral, `reduce=yes` (Kron reduce);
//!   - line_spacing: 3-phase overhead via `spacing=` + `wires=`;
//!   - cable_cn: 3-phase concentric-neutral cable via `geometry=` + `cncable=`;
//!   - cable_ts: 3-phase tape-shield cable via `geometry=` + `tscable=`.
//!
//! Pins: converged + iteration count + node order exact, node voltages 1e-6 rel,
//! the **Line YPrim entry-by-entry** (the Carson Z/Yc is the new math under
//! test), and every element's terminal currents/powers (voltage-scaled power
//! floor). Unlike the live corpus gate this golden is committed, so it guards the
//! geometry path offline (no oracle install needed to catch a regression).
//!
//! Regenerate only manually: `python tools/golden/gen_phase7.py`.

mod harness;

use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::{ElementCap, YPrim, assert_complex_close, compare_element, compare_yprim, tol_for};
use serde::Deserialize;

/// One scenario file: `{schema, oracle, scenario}` (the `oracle` block is
/// ignored here).
#[derive(Debug, Deserialize)]
struct ScenarioFile {
    schema: u32,
    scenario: Scenario,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    commands: Vec<String>,
    iterations: i32,
    converged: bool,
    node_order: Vec<String>,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    /// The Line's YPrim from the Carson geometry/spacing/cable path.
    line_yprim: YPrim,
    /// Every element's terminal currents/powers, in the oracle's element order.
    elements: Vec<ElementCap>,
}

/// Load every `*.json` scenario file from `tests/golden/phase7/`, sorted by file
/// name for deterministic run order.
fn load_scenarios() -> Vec<Scenario> {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "phase7",
    ]
    .iter()
    .collect();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()));
            let f: ScenarioFile = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("cannot parse {}: {e}", p.display()));
            assert_eq!(
                f.schema,
                1,
                "{}: phase7 golden schema mismatch",
                p.display()
            );
            f.scenario
        })
        .collect()
}

#[test]
fn phase7_targeted_scenarios_match_oracle() {
    let scenarios = load_scenarios();
    // Every path these targeted goldens cover must stay represented, so a future
    // edit can't silently drop a path's coverage (mirrors the count guards in
    // golden_phase5/6 and corpus_live's depth guard): the WP7.1 Line fetch
    // resolvers and the WP7.3 PVSystem injection / panel-inverter model.
    for must in [
        "line_geometry",
        "line_geometry_reduce",
        "line_spacing",
        "cable_cn",
        "cable_ts",
        "pvsystem_snapshot",
        "pvsystem_curves",
        "pvsystem_clamps",
    ] {
        assert!(
            scenarios.iter().any(|s| s.name == must),
            "phase7 golden missing required scenario {must}"
        );
    }
    let tol = tol_for("feeder");
    for sc in &scenarios {
        let mut dss = Dss::new();
        dss.command("clear");
        for c in &sc.commands {
            dss.command(c);
        }
        dss.command("solve");
        assert!(
            dss.errors().is_empty(),
            "{}: unexpected engine errors: {:?}",
            sc.name,
            dss.errors()
        );
        let ctx = sc.name.as_str();

        // Node order, iteration count, voltages.
        {
            let ckt = dss.circuit().expect("circuit after compile");
            assert!(sc.converged, "{ctx}: oracle did not converge");
            assert!(ckt.is_solved, "{ctx}: Rust solution did not converge");
            assert_eq!(
                ckt.solution.iteration, sc.iterations,
                "{ctx}: iteration count differs"
            );
            let names: Vec<String> = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();
            assert_eq!(names, sc.node_order, "{ctx}: node order differs");

            let mut actual = Vec::with_capacity(2 * ckt.num_nodes);
            for i in 1..=ckt.num_nodes {
                actual.push(ckt.solution.node_v[i].re);
                actual.push(ckt.solution.node_v[i].im);
            }
            let mut expected = Vec::with_capacity(actual.len());
            for (re, im) in sc.v_re.iter().zip(&sc.v_im) {
                expected.push(*re);
                expected.push(*im);
            }
            assert_complex_close(&actual, &expected, 1e-6, 1e-9, &format!("{ctx} voltages"));
        }

        // The Line YPrim from the Carson geometry/spacing/cable path — the new
        // math under test, pinned entry-by-entry.
        compare_yprim(&dss, &sc.line_yprim, &tol, ctx);

        // Every element's terminal currents/powers.
        let snaps = dss.snapshot_elements();
        for ec in &sc.elements {
            compare_element(&snaps, ec, &tol, ctx);
        }
    }
}
