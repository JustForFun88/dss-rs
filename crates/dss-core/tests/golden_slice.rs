//! Phase 3 gate (PORTING_PLAN.md): replay each vertical-slice scenario from
//! `tests/golden/slice.json` through the Rust engine and require
//!   (a) 2-bus vsource+line+load voltages within 1e-9 rel of dss-python,
//!   (b) the IEEE13-flat variant within 1e-6 rel,
//!   (c) fixed-point iteration counts exactly equal,
//!   (d) all 8 load models exercised and matching.
//! Goldens are produced by `tools/golden/gen_slice.py` with the pinned
//! oracle (tools/golden/PIN.txt); regenerate only manually.

mod harness;

use std::collections::BTreeSet;
use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::assert_complex_close;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SliceGolden {
    schema: u32,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    tol: f64,
    commands: Vec<String>,
    converged: bool,
    iterations: i32,
    node_order: Vec<String>,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
}

fn load_golden() -> SliceGolden {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "tests",
        "golden",
        "slice.json",
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let g: SliceGolden = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
    assert_eq!(g.schema, 1, "slice golden schema mismatch");
    g
}

fn run_scenario(sc: &Scenario) {
    let mut dss = Dss::new();
    for cmd in &sc.commands {
        dss.command(cmd);
    }
    assert!(
        dss.errors().is_empty(),
        "{}: unexpected engine errors: {:?}",
        sc.name,
        dss.errors()
    );
    let ckt = dss
        .circuit()
        .unwrap_or_else(|| panic!("{}: no circuit after replay", sc.name));

    // (c) convergence flag and fixed-point iteration count: exact.
    assert!(sc.converged, "{}: oracle did not converge", sc.name);
    assert!(ckt.is_solved, "{}: Rust solution did not converge", sc.name);
    assert_eq!(
        ckt.solution.iteration, sc.iterations,
        "{}: iteration count differs",
        sc.name
    );

    // Global node order must match the oracle's YNodeOrder exactly.
    let names: Vec<String> = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();
    assert_eq!(names, sc.node_order, "{}: node order differs", sc.name);

    // (a)/(b) node voltages within the per-scenario relative tolerance.
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
    assert_complex_close(
        &actual,
        &expected,
        sc.tol,
        1e-9, // absolute floor in volts; all scenario nodes sit at kV scale
        &format!("{} node voltages", sc.name),
    );
}

#[test]
fn golden_slice_matches_oracle() {
    let golden = load_golden();
    assert!(!golden.scenarios.is_empty(), "no scenarios in golden");
    for sc in &golden.scenarios {
        run_scenario(sc);
    }
}

#[test]
fn golden_slice_covers_all_eight_load_models() {
    // (d) the scenario set must exercise load models 1..8.
    let golden = load_golden();
    let mut models = BTreeSet::new();
    for sc in &golden.scenarios {
        for cmd in &sc.commands {
            if let Some(pos) = cmd.to_lowercase().find("model=")
                && let Some(m) = cmd[pos + 6..]
                    .split_whitespace()
                    .next()
                    .and_then(|t| t.parse::<u32>().ok())
            {
                models.insert(m);
            }
        }
    }
    assert_eq!(
        models,
        (1..=8).collect::<BTreeSet<u32>>(),
        "slice goldens must exercise all 8 load models"
    );
}
