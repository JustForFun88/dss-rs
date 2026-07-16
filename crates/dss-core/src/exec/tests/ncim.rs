//! NCIM solver (`Set Algorithm=NCIM`) integration tests.
//!
//! The oracle-gated deck matrix (capi015 micro/PV/midi) lands with the WP deck
//! flips; these Rust-level tests pin the two properties that hold *independently*
//! of the oracle:
//!
//! 1. **Self-consistency** — NCIM and the default fixed-point (`Normal`) solve
//!    the same power-flow equations, so on a PQ-only circuit they must converge to
//!    the same node voltages (to the looser of the two convergence criteria).
//! 2. **Option surface** — `Set/Get IgnoreGenQLimits` and `NCIMQGain` round-trip,
//!    and `NCIM` parses as algorithm ordinal 2.

use crate::exec::Dss;

fn build_pq_circuit(load_model: i32) -> Dss {
    let mut dss = Dss::new();
    for line in [
        "New circuit.ncimtest basekv=12.47 phases=3 bus1=sourcebus",
        "New Line.l1 bus1=sourcebus bus2=mid phases=3 r1=0.12 x1=0.35 length=2",
        "New Line.l2 bus1=mid bus2=loadbus phases=3 r1=0.12 x1=0.35 length=1",
        &format!("New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=1200 kvar=500 model={load_model}"),
        "New Load.ld2 bus1=mid phases=3 kv=12.47 kw=600 kvar=200 model=1",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
    ] {
        dss.command(line);
        assert!(
            dss.errors().is_empty(),
            "setup `{line}` -> {:?}",
            dss.errors()
        );
    }
    dss
}

/// The maximum per-node voltage-magnitude relative difference between two
/// solutions (relative to the source magnitude).
fn max_rel_vmag_diff(a: &[num_complex::Complex64], b: &[num_complex::Complex64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let base = a.get(1).map(|v| v.norm()).unwrap_or(1.0).max(1.0);
    a.iter()
        .zip(b)
        .skip(1)
        .map(|(x, y)| (x.norm() - y.norm()).abs() / base)
        .fold(0.0_f64, f64::max)
}

/// NCIM converges to the same node voltages as the default fixed-point solver on
/// a PQ (constant-power) circuit. Both solve `I(V) = 0`; the difference is bounded
/// by the looser convergence criterion (`Normal`'s voltage tolerance, 1e-4).
#[test]
fn ncim_matches_normal_solve_pq() {
    let mut dss = build_pq_circuit(1);
    dss.command("Solve");
    assert!(
        dss.errors().is_empty(),
        "normal solve -> {:?}",
        dss.errors()
    );
    let v_normal = dss.circuit().unwrap().solution.node_v.clone();
    assert!(dss.circuit().unwrap().is_solved);

    dss.command("Set algorithm=NCIM");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "ncim solve -> {:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "NCIM did not converge");
    assert_eq!(ckt.solution.algorithm, 2, "algorithm should be NCIM (2)");
    let v_ncim = ckt.solution.node_v.clone();

    let diff = max_rel_vmag_diff(&v_normal, &v_ncim);
    assert!(
        diff < 1e-3,
        "NCIM vs Normal node-voltage magnitudes diverge by {diff:.2e} (rel)"
    );
}

/// The constant-impedance (`model=2`, `NCIM_DoZBus`) path also matches: a ConstZ
/// load's admittance is stamped into the NCIM Jacobian + mismatch directly rather
/// than via the PQ derivative.
#[test]
fn ncim_matches_normal_solve_constz() {
    let mut dss = build_pq_circuit(2);
    dss.command("Solve");
    let v_normal = dss.circuit().unwrap().solution.node_v.clone();

    dss.command("Set algorithm=NCIM");
    dss.command("Solve");
    assert!(dss.errors().is_empty(), "ncim solve -> {:?}", dss.errors());
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "NCIM (ConstZ) did not converge");
    let v_ncim = ckt.solution.node_v.clone();

    let diff = max_rel_vmag_diff(&v_normal, &v_ncim);
    assert!(
        diff < 1e-3,
        "NCIM(ConstZ) vs Normal node-voltage magnitudes diverge by {diff:.2e} (rel)"
    );
}

/// Re-solving under NCIM (a second `Solve`) stays converged and stable — exercises
/// the warm-start path (`NCIM_Ready` true, `NCIM_InitGenQ` false).
#[test]
fn ncim_resolve_is_stable() {
    let mut dss = build_pq_circuit(1);
    dss.command("Set algorithm=NCIM");
    dss.command("Solve");
    let v1 = dss.circuit().unwrap().solution.node_v.clone();
    dss.command("Solve");
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    let v2 = ckt.solution.node_v.clone();
    let diff = max_rel_vmag_diff(&v1, &v2);
    assert!(diff < 1e-9, "NCIM re-solve drifted by {diff:.2e}");
}

/// `Set Algorithm=NCIM` parses (prefix `nc`, min-abbrev 2) and `Get Algorithm`
/// reads it back; the two NCIM options round-trip through `Set`/`Get`.
#[test]
fn ncim_options_roundtrip() {
    let mut dss = build_pq_circuit(1);

    dss.command("Set algorithm=nc"); // min-abbreviation
    dss.command("Get algorithm");
    assert_eq!(dss.result().trim().to_lowercase(), "ncim");

    dss.command("Set IgnoreGenQLimits=yes");
    dss.command("Get IgnoreGenQLimits");
    assert_eq!(dss.result().trim().to_lowercase(), "yes");

    dss.command("Set NCIMQGain=0.75");
    dss.command("Get NCIMQGain");
    let g: f64 = dss.result().trim().parse().expect("numeric NCIMQGain");
    assert!((g - 0.75).abs() < 1e-12, "got {g}");

    // Defaults on a fresh solution.
    let ckt = dss.circuit().unwrap();
    assert!(ckt.solution.ncim_ignore_q_limit);
    assert!((ckt.solution.ncim_gen_gain - 0.75).abs() < 1e-12);
}
