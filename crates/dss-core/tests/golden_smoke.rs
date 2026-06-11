//! Phase 0 gate: the golden harness loads and validates real oracle data.
//!
//! There is no engine yet — these tests prove the comparison machinery works
//! end-to-end against the committed goldens, so every later phase only has to
//! plug the engine's outputs into `harness::assert_complex_close`.

mod harness;

use harness::{Golden, Tolerances, assert_complex_close};

const CASES: &[&str] = &["ieee13", "ieee34mod1", "ieee37", "ieee123"];

#[test]
fn goldens_load_and_are_self_consistent() {
    for case in CASES {
        let g = Golden::load(case);
        assert_eq!(g.name, *case);
        assert!(g.solution.converged, "{case}: oracle did not converge");
        assert!(g.solution.iterations > 0, "{case}: zero iterations");
        assert_eq!(
            g.node_voltages.len(),
            2 * g.node_order.len(),
            "{case}: voltage array must be re/im interleaved per node"
        );
        assert_eq!(
            g.node_order.len() as u32,
            g.circuit.num_nodes,
            "{case}: node order vs NumNodes mismatch"
        );
        assert_eq!(
            g.elements.len() as u32,
            g.circuit.num_ckt_elements,
            "{case}: element dump vs NumCktElements mismatch"
        );
        // Every converged feeder must have nonzero source-bus voltage.
        let v0 = (g.node_voltages[0].powi(2) + g.node_voltages[1].powi(2)).sqrt();
        assert!(v0 > 1.0, "{case}: suspicious near-zero source voltage {v0}");
    }
}

#[test]
fn tolerance_comparator_accepts_identity_and_rejects_perturbation() {
    let g = Golden::load("ieee13");
    let tol = Tolerances::default();

    // Identity comparison must pass.
    assert_complex_close(
        &g.node_voltages,
        &g.node_voltages,
        tol.voltage_rel,
        tol.voltage_abs_floor,
        "ieee13 node voltages (identity)",
    );

    // A perturbation well above tolerance must be caught.
    let mut perturbed = g.node_voltages.clone();
    perturbed[0] *= 1.0 + 1e-3;
    let caught = std::panic::catch_unwind(|| {
        assert_complex_close(
            &perturbed,
            &g.node_voltages,
            tol.voltage_rel,
            tol.voltage_abs_floor,
            "ieee13 node voltages (perturbed)",
        );
    })
    .is_err();
    assert!(caught, "comparator failed to flag a 1e-3 relative error");
}

#[test]
fn ieee13_known_values_spot_check() {
    // Published IEEE 13-bus values: 4.16 kV system → phase-ground magnitude
    // at the regulator bus RG60 ≈ 2401 V, around 1.05 pu.
    let g = Golden::load("ieee13");
    let idx = g
        .node_order
        .iter()
        .position(|n| n == "RG60.1")
        .expect("RG60.1 present");
    let (re, im) = (g.node_voltages[2 * idx], g.node_voltages[2 * idx + 1]);
    let mag = (re * re + im * im).sqrt();
    let base = 4160.0 / 3f64.sqrt();
    let pu = mag / base;
    assert!(
        (0.9..=1.1).contains(&pu),
        "RG60.1 voltage {mag:.1} V = {pu:.4} pu out of plausible range"
    );
}
