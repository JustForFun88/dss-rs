//! Command-replay scenario harness shared by the targeted golden gates that
//! split the former Phase-7 bucket: `golden_line_constants.rs`,
//! `golden_der_controls.rs`, `golden_harmonics.rs`. Each scenario replays a
//! command list on the Rust engine, solves, and compares the state against its
//! committed `tests/golden/<family>/<scenario>.json` (schema 1, regenerated
//! only manually via `tools/golden/gen_der_lines_harmonics.py` with the pinned
//! oracle).

use std::path::PathBuf;

use dss_core::exec::Dss;
use serde::Deserialize;

use super::{
    ElementCap, MonitorCap, YPrim, assert_complex_close, compare_element, compare_monitor,
    compare_yprim, tol_for,
};

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
    /// The scenario Line's YPrim (for the line-constants family this is the
    /// Carson geometry/spacing/cable path under test; the DER/harmonics decks
    /// pin their line the same way).
    line_yprim: YPrim,
    /// Every element's terminal currents/powers, in the oracle's element order.
    elements: Vec<ElementCap>,
    /// Each Storage element's integrated state of charge after the solve
    /// (`kWhStored`/`%Stored`/`State`) — the WP7.4 SOC-trajectory pin.
    #[serde(default)]
    storage: Vec<StorageCap>,
    /// Per-step monitor channels (a `mode=1` power monitor on each controlled DER in
    /// a daily/duty deck): the PER-HOUR trajectory, compared elementwise so a
    /// multi-step golden pins every step, not just the final state.
    #[serde(default)]
    monitors: Vec<MonitorCap>,
}

/// One Storage's post-solve state readback (`? Storage.<name>.<prop>`).
#[derive(Debug, Deserialize)]
struct StorageCap {
    name: String,
    properties: std::collections::BTreeMap<String, String>,
}

/// Load every `*.json` scenario file from `tests/golden/<dir_name>/`, sorted by
/// file name for deterministic run order.
fn load_scenarios(dir_name: &str) -> Vec<Scenario> {
    let dir: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        dir_name,
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
                "{}: {dir_name} golden schema mismatch",
                p.display()
            );
            f.scenario
        })
        .collect()
}

/// The family gate body: every `required` scenario must be present (so a future
/// edit can't silently drop a path's coverage — mirrors the count guards in the
/// other golden gates and corpus_live's depth guard), then each scenario is
/// replayed and compared.
pub fn check_family(dir_name: &str, required: &[&str]) {
    let scenarios = load_scenarios(dir_name);
    for must in required {
        assert!(
            scenarios.iter().any(|s| s.name == *must),
            "{dir_name} golden missing required scenario {must}"
        );
    }
    let tol = tol_for("large");
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

        // The scenario Line's YPrim, pinned entry-by-entry.
        compare_yprim(&dss, &sc.line_yprim, &tol, ctx);

        // Every element's terminal currents/powers.
        let snaps = dss.snapshot_elements();
        for ec in &sc.elements {
            compare_element(&snaps, ec, &tol, ctx);
        }

        // The integrated state of charge of every Storage element after the
        // solve (the WP7.4 SOC-trajectory pin): read each property back through
        // the `? ...` query, exactly like the oracle captured it.
        for sct in &sc.storage {
            for (prop, expected) in &sct.properties {
                dss.command(&format!("? {}.{prop}", sct.name));
                let actual = dss.result().to_string();
                let label = format!("{ctx} {} {prop}", sct.name);
                match (actual.parse::<f64>(), expected.parse::<f64>()) {
                    (Ok(a), Ok(e)) => assert!(
                        (a - e).abs() <= 1e-6 * e.abs().max(1.0),
                        "{label}: {a} != {e}"
                    ),
                    _ => assert_eq!(&actual, expected, "{label}: state differs"),
                }
            }
        }

        // The PER-STEP (per-hour) monitor trajectory of a multi-step run: every
        // channel of every DER power monitor is compared elementwise against the
        // oracle, so a daily/duty golden pins EACH step, not only the final state
        // captured above. Snapshot scenarios carry no monitors (empty → no-op).
        for m in &sc.monitors {
            compare_monitor(&dss, m, &tol, ctx);
        }
    }
}
