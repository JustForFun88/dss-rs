//! Phase 4 gate (PHASE4_PLAN.md WP4.9): replay the committed controls-off
//! variants of IEEE13, IEEE37 and IEEE123 through the Rust engine and require
//!   (a) convergence flag and fixed-point iteration counts exactly equal,
//!   (b) node order exactly equal,
//!   (c) node voltages, per-element terminal powers and currents, total power
//!       and total losses within 1e-6 rel (1e-9 absolute floor),
//!   (d) element iteration order equal to the oracle's First/Next order
//!       (creation order) — names compared, not just values.
//! Goldens are produced by `tools/golden/gen_phase4.py` with the pinned
//! oracle (tools/golden/PIN.txt); regenerate only manually.

mod harness;

use std::path::PathBuf;

use dss_core::exec::Dss;
use harness::assert_complex_close;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Phase4Golden {
    schema: u32,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    name: String,
    /// Repo-relative path of the committed variant script.
    script: String,
    converged: bool,
    iterations: i32,
    node_order: Vec<String>,
    v_re: Vec<f64>,
    v_im: Vec<f64>,
    /// kW/kvar (CAPI `Circuit_Get_TotalPower`).
    total_power: Vec<f64>,
    /// W/var (CAPI `Circuit_Get_Losses`).
    losses: Vec<f64>,
    elements: Vec<GoldenElement>,
}

#[derive(Debug, Deserialize)]
struct GoldenElement {
    name: String,
    powers: Vec<f64>,
    currents: Vec<f64>,
}

fn repo_root() -> PathBuf {
    [env!("CARGO_MANIFEST_DIR"), "..", ".."].iter().collect()
}

fn load_golden() -> Phase4Golden {
    let path = repo_root().join("tests").join("golden").join("phase4.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let g: Phase4Golden = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("cannot parse {}: {e}", path.display()));
    assert_eq!(g.schema, 1, "phase4 golden schema mismatch");
    g
}

fn run_scenario(sc: &Scenario) {
    let script = repo_root().join(&sc.script);
    assert!(
        script.is_file(),
        "{}: committed variant script missing: {}",
        sc.name,
        script.display()
    );
    let script_str = script.to_string_lossy().replace('\\', "/");

    let mut dss = Dss::new();
    dss.command(&format!("compile \"{script_str}\""));
    assert!(
        dss.errors().is_empty(),
        "{}: unexpected engine errors: {:?}",
        sc.name,
        dss.errors()
    );

    // (a) convergence flag and fixed-point iteration count: exact.
    let ckt = dss
        .circuit()
        .unwrap_or_else(|| panic!("{}: no circuit after compile", sc.name));
    assert!(sc.converged, "{}: oracle did not converge", sc.name);
    assert!(ckt.is_solved, "{}: Rust solution did not converge", sc.name);
    assert_eq!(
        ckt.solution.iteration, sc.iterations,
        "{}: iteration count differs",
        sc.name
    );

    // (b) global node order must match the oracle's YNodeOrder exactly.
    let names: Vec<String> = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();
    assert_eq!(names, sc.node_order, "{}: node order differs", sc.name);

    // (c) node voltages at 1e-6 rel.
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
        1e-6,
        1e-9,
        &format!("{} node voltages", sc.name),
    );

    // Total power (kW/kvar) and losses (W/var) at 1e-6 rel. The absolute
    // floor covers near-zero var totals.
    let (tp_kw, tp_kvar) = dss.total_power();
    assert_complex_close(
        &[tp_kw, tp_kvar],
        &[sc.total_power[0], sc.total_power[1]],
        1e-6,
        1e-6,
        &format!("{} total power", sc.name),
    );
    let (loss_w, loss_var) = dss.losses();
    assert_complex_close(
        &[loss_w, loss_var],
        &[sc.losses[0], sc.losses[1]],
        1e-6,
        1e-6,
        &format!("{} losses", sc.name),
    );

    // (c)/(d) per-element powers and currents, in the oracle's element order.
    let snapshots = dss.snapshot_elements();
    assert_eq!(
        snapshots.len(),
        sc.elements.len(),
        "{}: element count differs",
        sc.name
    );
    for (snap, exp) in snapshots.iter().zip(&sc.elements) {
        assert!(
            snap.name.eq_ignore_ascii_case(&exp.name),
            "{}: element order differs: {} vs {}",
            sc.name,
            snap.name,
            exp.name
        );
        // Absolute floors: a dead-end branch's current is the difference of
        // two nearly equal node voltages, so 1e-6-rel voltage agreement
        // cannot produce better than ~|Y|·1e-6·|V| ≈ µA-scale absolute
        // current agreement (and mW-scale power agreement). 1e-4 A / 1e-4 kW
        // are far above that noise yet ~1e-6 rel of the meaningful signals.
        assert_complex_close(
            &snap.currents,
            &exp.currents,
            1e-6,
            1e-4,
            &format!("{} {} currents", sc.name, exp.name),
        );
        assert_complex_close(
            &snap.powers,
            &exp.powers,
            1e-6,
            1e-4,
            &format!("{} {} powers", sc.name, exp.name),
        );
    }
}

#[test]
fn golden_feeders_match_oracle() {
    let golden = load_golden();
    assert_eq!(golden.scenarios.len(), 3, "expected the three IEEE feeders");
    for sc in &golden.scenarios {
        run_scenario(sc);
    }
}
