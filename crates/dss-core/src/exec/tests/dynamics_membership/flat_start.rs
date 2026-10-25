//! A run with no disturbance from a seed at its equilibrium.

use num_complex::Complex64;

use super::*;

/// The largest distance of a node voltage from its power-flow value, relative
/// to the power-flow magnitude, infinite for a node that is not finite.
fn largest_drift(pf: &[(String, Complex64)], run: &[(String, Complex64)]) -> f64 {
    assert_eq!(pf.len(), run.len(), "node count");
    pf.iter()
        .zip(run)
        .map(|((n, a), (m, b))| {
            assert_eq!(n, m, "node lists");
            let drift = (b - a).norm() / a.norm();
            if drift.is_finite() {
                drift
            } else {
                f64::INFINITY
            }
        })
        .fold(0.0, f64::max)
}

/// A machine whose seed is at its equilibrium holds the power-flow point on
/// [`H_Z0`] through a run with no disturbance: the Generator, the GFL PVSystem
/// at unity efficiency and power factor, and the IndMach012 over the steps
/// before its negative sequence grows. Every node stays at every step within
/// the bound derived from the power-flow tolerance beside its computation.
#[test]
fn a_machine_at_its_equilibrium_holds_the_power_flow_point() {
    let dir = scratch("flat_start");
    let tol = 1e-12;
    let omega0 = 2.0 * std::f64::consts::PI * 60.0;
    // The IndMach012 transient reactance of `IM`: Xs + Xm Xr / (Xm + Xr).
    let (xs, xr, xm) = (0.075, 0.12, 3.8);
    let im_x = xs + xm * xr / (xm + xr);
    // (class, element, steps, inertia constant and transient reactance)
    let cases = [
        ("Generator", GEN, 1000, Some((1.0, 0.28))),
        ("PVSystem", PV, 1000, None),
        ("IndMach012", IM, 100, Some((6.0, im_x))),
    ];
    // The bound rests on two premises. The power flow and each step end
    // within `tol` of their fixpoints: the convergence test bounds only the
    // last iteration step, which lies `tol ρ / (1 − ρ)` from the fixpoint for
    // a contraction `ρ`, within `tol` while `ρ <= 1/2`. And the PVSystem's PI
    // current controllers drive their residual back to zero rather than
    // accumulate it. The seeded sources inject the power-flow currents at the
    // power-flow voltages, so every node stays within `2 tol`, plus what a
    // swing equation integrates. The air-gap power is `Re(E conj(I))` with
    // `I = (E − V) / jX'`, so a voltage residual `tol |V|` behind `X'` (per unit
    // of the rating) is a power residual of at most `|E| |V| tol / X'`, which
    // turns the rotor by at most `|E| |V| tol ω0 T² / (4 H X')` in `T`, the
    // damping left out (the swing term alone for the IndMach012, its rotor flux
    // relaxation left out). The code takes `|E| |V|` as one. That is stricter
    // than the derived bound for the Generator, whose `|E| |V|` is about 1.24
    // here, and looser by a seventh for the IndMach012, whose `|E| |V|` is
    // about 0.875, well inside the margin its drift leaves.
    for (class, line, steps, swing) in cases {
        let time = steps as f64 * 0.001;
        let integrated = swing.map_or(0.0, |(h, x): (f64, f64)| {
            omega0 * time * time / (4.0 * h * x)
        });
        let bound = tol * (2.0 + integrated);
        let mut dss = deck(
            &dir,
            &lines(&[
                &H_Z0,
                &[line, "Set tolerance=1e-12", "Set maxiterations=200"],
                &PF,
            ]),
        );
        let pf = node_voltages(&dss);
        run(&mut dss, &[dyn_entry(1)]);
        for step in 1..=steps {
            run(&mut dss, &["Solve".to_string()]);
            let drift = largest_drift(&pf, &node_voltages(&dss));
            assert!(
                drift <= bound,
                "{class}: step {step} moved a node {drift:e} off the power flow, past {bound:e}"
            );
        }
        assert_eq!(
            dss.circuit().expect("circuit").solution.t,
            (0..steps).fold(0.0, |t, _| t + 0.001),
            "{class}: the clock"
        );
    }
    std::fs::remove_dir_all(&dir).ok();
}
