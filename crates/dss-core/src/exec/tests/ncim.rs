//! NCIM solver (`Set Algorithm=NCIM`) integration tests.
//!
//! The node-voltage / regulation / warm-restart assertions here were captured
//! 2026-07-16 from the **capi015** NCIM oracle (dss_capi 0.15.0b4, OpenDSS SVN
//! r4103 — the 0.15.x engine line that owns NCIM) and embedded as constants,
//! golden-style; the port's pinned 0.14.5 gate oracle has no NCIM at all. Those
//! capi015-captured node voltages are DIGIT-IDENTICAL to the live r4133 oracle
//! (own probes, ~1e-12) — the Rust port reproduces them to <5e-11 V (faer-vs-KLU
//! last-ulp), so the 1e-6 V band below is ~4 orders above the cross-solver floor
//! yet still ~7 orders *tighter* than any physically-meaningful voltage error.
//!
//! The **swing-source reported-current** assertion
//! ([`ncim_vsource_reported_currents_match_oracle`]) is instead pinned against the
//! **live r4133** oracle (EPRI `OpenDSSDirect.dll` r4133 via `epri-worker`): r4133
//! fixed the capi015 0.15.0b4 one-conductor shift in `CalcInjCurrAtBus`, and the
//! capi015 probe venv is retired, so r4133 is the oracle-of-record for NCIM
//! (oracle-of-record flip capi015→r4133, 2026-07-20; `docs/upgrade/DIVERGENCES.md`).
//!
//! **NCIM does not converge to the same node voltages as the default fixed-point
//! (`Normal`).** NCIM holds the swing (source) bus at the ideal EMF with *no*
//! series-impedance droop; `Normal` models the VSource as a Thevenin source, so
//! its source bus droops (≈4 V here). Both engines agree on this — see
//! [`ncim_source_bus_is_ideal_emf_matches_oracle`] — so comparing NCIM against
//! `Normal` (as an earlier revision of this file did) was the wrong baseline;
//! these tests compare NCIM against the NCIM oracle.

use num_complex::Complex64;

use crate::exec::Dss;

fn cx(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

/// Run a script and return the `Dss`. Setup errors abort; a non-converged NCIM
/// solve is *not* an error (`DoNCIMSolution` returns Ok even when it stalls,
/// matching upstream), so callers assert `is_solved` themselves.
fn run(lines: &[&str]) -> Dss {
    let mut dss = Dss::new();
    for line in lines {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    dss
}

/// Assert every node voltage (1-based, in Y-order) matches `expected` to `tol`
/// on both the real and imaginary components.
fn assert_nodes(dss: &Dss, expected: &[Complex64], tol: f64) {
    let ckt = dss.circuit().unwrap();
    assert_eq!(
        ckt.solution.node_v.len(),
        expected.len() + 1,
        "node count (excl. ground)"
    );
    for (k, e) in expected.iter().enumerate() {
        let v = ckt.solution.node_v[k + 1];
        assert!(
            (v.re - e.re).abs() < tol && (v.im - e.im).abs() < tol,
            "node {} ({}): Rust {v:?} vs capi015 {e:?} (tol {tol:.0e})",
            k + 1,
            ckt.node_name(k + 1),
        );
    }
}

const V_TOL: f64 = 1e-6;
const KVAR_TOL: f64 = 1e-4;

/// The two-line PQ feeder (`sourcebus → mid → loadbus`) the PQ/ConstZ tests use.
fn pq_circuit(ld1_model: i32) -> Vec<String> {
    vec![
        "New circuit.ncimtest basekv=12.47 phases=3 bus1=sourcebus".into(),
        "New Line.l1 bus1=sourcebus bus2=mid phases=3 r1=0.12 x1=0.35 length=2".into(),
        "New Line.l2 bus1=mid bus2=loadbus phases=3 r1=0.12 x1=0.35 length=1".into(),
        format!("New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=1200 kvar=500 model={ld1_model}"),
        "New Load.ld2 bus1=mid phases=3 kv=12.47 kw=600 kvar=200 model=1".into(),
        "Set voltagebases=[12.47]".into(),
        "Calcvoltagebases".into(),
    ]
}

/// A radial PV-bus feeder: `sourcebus → genbus`, a 2 MW/0.8 Mvar load and a
/// model-3 (voltage-regulating) generator at `genbus`, ±1.5 Mvar Q-range.
fn pv_circuit(vpu: &str) -> Vec<String> {
    vec![
        "New circuit.ncimpv basekv=12.47 phases=3 bus1=sourcebus".into(),
        "New Line.l1 bus1=sourcebus bus2=genbus phases=3 r1=0.12 x1=0.35 length=3".into(),
        "New Load.ld1 bus1=genbus phases=3 kv=12.47 kw=2000 kvar=800 model=1".into(),
        format!(
            "New Generator.g1 bus1=genbus phases=3 kv=12.47 kw=800 model=3 \
             maxkvar=1500 minkvar=-1500 vpu={vpu}"
        ),
        "Set voltagebases=[12.47]".into(),
        "Calcvoltagebases".into(),
    ]
}

fn as_refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

fn solve_ncim(setup: &[String]) -> Dss {
    let mut lines = as_refs(setup);
    lines.push("Set algorithm=NCIM");
    lines.push("Solve");
    run(&lines)
}

/// capi015 NCIM source bus (nodes 1..3): the ideal EMF, no droop, imag(node 1)=0.
const ORACLE_SOURCEBUS: [Complex64; 3] = [
    Complex64::new(7199.557856794634, 0.0),
    Complex64::new(-3599.77892839732, -6234.999999999999),
    Complex64::new(-3599.778928397315, 6235.000000000002),
];

/// PQ (all model-1 loads): NCIM node voltages match the capi015 NCIM oracle.
#[test]
fn ncim_pq_matches_oracle() {
    let dss = solve_ncim(&pq_circuit(1));
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "NCIM PQ did not converge");
    assert_eq!(
        ckt.solution.algorithm,
        crate::solution::solution::SolveAlgorithm::Ncim,
        "algorithm should be NCIM (2)"
    );
    assert_eq!(ckt.solution.iteration, 3, "capi015 converges PQ in 3 iters");

    let expected = [
        ORACLE_SOURCEBUS[0],
        ORACLE_SOURCEBUS[1],
        ORACLE_SOURCEBUS[2],
        cx(7156.125666935628, -50.5630322082336),
        cx(-3621.8517038525165, -6172.105104135991),
        cx(-3534.2739630831124, 6222.668136344223),
        cx(7141.080031242963, -67.22612265245337),
        cx(-3628.759545636436, -6150.743656187946),
        cx(-3512.320485606528, 6217.969778840398),
    ];
    assert_nodes(&dss, &expected, V_TOL);
}

/// ConstZ `ld1` (`model=2`, `NCIM_DoZBus`): the admittance is stamped straight
/// into the Jacobian + mismatch rather than via the PQ derivative. Matches the
/// capi015 NCIM oracle.
#[test]
fn ncim_constz_matches_oracle() {
    let dss = solve_ncim(&pq_circuit(2));
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "NCIM ConstZ did not converge");
    assert_eq!(ckt.solution.iteration, 3);

    let expected = [
        ORACLE_SOURCEBUS[0],
        ORACLE_SOURCEBUS[1],
        ORACLE_SOURCEBUS[2],
        cx(7156.612809917596, -50.03324451428737),
        cx(-3621.636465741933, -6172.7918761806295),
        cx(-3534.9763441756672, 6222.825120694914),
        cx(7141.8096524178145, -66.43152609318317),
        cx(-3628.436215417777, -6151.772824940151),
        cx(-3513.3734370000425, 6218.204351033329),
    ];
    assert_nodes(&dss, &expected, V_TOL);
}

/// NCIM holds the source bus at the ideal EMF (`7199.56 + 0i`), whereas the
/// default fixed-point (`Normal`) droops it under the VSource impedance
/// (`7195.46 − 5.68i`). This ≈4 V gap is a genuine NCIM modelling property —
/// confirmed by the capi015 oracle — not a convergence-tolerance artifact, and
/// is why NCIM must be pinned against the NCIM oracle, never against `Normal`.
#[test]
fn ncim_source_bus_is_ideal_emf_matches_oracle() {
    // Normal solve: source bus droops.
    let mut normal = Dss::new();
    for line in as_refs(&pq_circuit(1)) {
        normal.command(line);
    }
    normal.command("Solve");
    let v_normal = normal.circuit().unwrap().solution.node_v[1];
    assert!(
        (v_normal.re - 7195.4576).abs() < 1e-2 && (v_normal.im - (-5.6823)).abs() < 1e-2,
        "Normal source bus droops: {v_normal:?}"
    );

    // NCIM: ideal EMF, no droop — matches capi015.
    let dss = solve_ncim(&pq_circuit(1));
    let v_ncim = dss.circuit().unwrap().solution.node_v[1];
    assert!(
        (v_ncim - ORACLE_SOURCEBUS[0]).norm() < V_TOL,
        "NCIM source bus = ideal EMF (capi015): {v_ncim:?}"
    );
    assert!(
        (v_ncim.re - v_normal.re).abs() > 3.0,
        "NCIM vs Normal source bus differ by the impedance droop (not tolerance)"
    );
}

/// PV bus **actively regulating within its Q-limits**: `vpu=1.0` needs
/// Q≈1217 kvar (< the ±1500 limit), so the generator stays model-3 and drives
/// `|genbus|` to its target (`7199.56 V`, exactly the source EMF). capi015 NCIM
/// converges in 3 iters; node voltages **and** the reported generator reactive
/// power (`NCIM_GetPowers` persists `deltaQNom` into `Qnominalperphase`) match.
#[test]
fn ncim_pv_regulating_matches_oracle() {
    let dss = solve_ncim(&pv_circuit("1.0"));
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "NCIM PV (regulating) did not converge");
    assert_eq!(ckt.solution.iteration, 3);

    let expected = [
        ORACLE_SOURCEBUS[0],
        ORACLE_SOURCEBUS[1],
        ORACLE_SOURCEBUS[2],
        cx(7199.26175142393, -65.29600154522561),
        cx(-3656.178871815681, -6202.09556445416),
        cx(-3543.0828796082506, 6267.391565999387),
    ];
    assert_nodes(&dss, &expected, V_TOL);

    // Reported generator Q tracks the solved deltaQNom (finding: Qnominalperphase
    // write-back). capi015 `Generators.kvar` = 1217.2208409024108.
    let (kw, kvar) = dss.generator_present_kw_kvar("g1").expect("g1");
    assert!((kw - 800.0).abs() < KVAR_TOL, "g1 kW = {kw}");
    assert!(
        (kvar - 1217.2208409024108).abs() < KVAR_TOL,
        "g1 kvar = {kvar} (capi015 1217.2208)"
    );
}

/// PV bus **hitting its Q-limit → PV→PQ conversion**: `vpu=1.01` demands more
/// than the +1500 kvar limit, so `NCIM_UpdateGenQ` clamps Q and converts the
/// generator to model-4 (PQ). capi015 NCIM converges in 8 iters at Q=1500.
#[test]
fn ncim_pv_qlimit_pv2pq_matches_oracle() {
    let dss = solve_ncim(&pv_circuit("1.01"));
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "NCIM PV (Q-limit) did not converge");
    assert_eq!(
        ckt.solution.iteration, 8,
        "capi015 needs 8 iters for the PV→PQ conversion"
    );

    let expected = [
        ORACLE_SOURCEBUS[0],
        ORACLE_SOURCEBUS[1],
        ORACLE_SOURCEBUS[2],
        cx(7212.895598275793, -70.00930104534172),
        cx(-3667.077632344357, -6211.546172429123),
        cx(-3545.817965931437, 6281.5554734744655),
    ];
    assert_nodes(&dss, &expected, V_TOL);

    let (_, kvar) = dss.generator_present_kw_kvar("g1").expect("g1");
    assert!((kvar - 1500.0).abs() < KVAR_TOL, "g1 kvar clamped = {kvar}");
}

/// PV bus that **cannot be satisfied**: `vpu=1.02` drives Q to the +1500 limit,
/// yet the current-injection mismatch never closes. capi015 NCIM ALSO fails to
/// converge on this deck — it runs the full 15 iterations and stalls at exactly
/// the same fixpoint (`|genbus| = 7343.55 V`, the VTarget). The port reproduces
/// that faithfully; this pins the shared non-convergence so a future change that
/// silently "fixes" it (diverging from the oracle) is caught.
#[test]
fn ncim_pv_aggressive_nonconvergence_is_faithful() {
    let dss = solve_ncim(&pv_circuit("1.02"));
    let ckt = dss.circuit().unwrap();
    assert!(
        !ckt.is_solved,
        "capi015 NCIM does NOT converge here; the port must match"
    );
    assert_eq!(
        ckt.solution.iteration, 15,
        "runs to max_iterations like the oracle"
    );

    // Even non-converged, the stalled fixpoint matches capi015 node-for-node.
    let expected = [
        ORACLE_SOURCEBUS[0],
        ORACLE_SOURCEBUS[1],
        ORACLE_SOURCEBUS[2],
        cx(7342.634931411053, -115.86364409175604),
        cx(-3771.65832486403, -6300.976559271102),
        cx(-3570.9766065470253, 6416.84020336286),
    ];
    assert_nodes(&dss, &expected, V_TOL);
    // The stall sits at the regulation target: |genbus| = 1.02·(12470/√3).
    assert!((ckt.solution.node_v[4].norm() - 7343.54901393).abs() < 1e-4);
}

/// Re-solving under NCIM (a second `Solve`) stays at the converged fixpoint —
/// exercises the warm-start path (`NCIM_Ready` true, `NCIM_InitGenQ` false).
/// Correctness is pinned by the oracle tests above; this only guards warm-start
/// stability.
#[test]
fn ncim_resolve_is_stable() {
    let mut dss = solve_ncim(&pq_circuit(1));
    let v1 = dss.circuit().unwrap().solution.node_v.clone();
    dss.command("Solve");
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved);
    let v2 = &ckt.solution.node_v;
    let drift = v1
        .iter()
        .zip(v2)
        .map(|(a, b)| (a - b).norm())
        .fold(0.0_f64, f64::max);
    assert!(drift < 1e-9, "NCIM re-solve drifted by {drift:.2e}");
}

/// Swing-source **reported currents** under NCIM (`VSource.CalcInjCurrAtBus`,
/// r4133 `VSource.pas` l.1085): NCIM holds the swing bus at the ideal EMF, so the
/// normal `YPrim·V - Iinj` path reports ~0 there; the source's terminal current is
/// instead the KCL sum at its bus. Pinned vs the **live r4133** oracle
/// (`Vsource.source.Currents/Powers/Losses`, EPRI OpenDSSDirect.dll r4133 "Version
/// 11.0.0.1 (64-bit build)" via `epri-worker`, own probe 2026-07-20). r4133's
/// offset-write `GetCurrents(@(ElmCurrents[1]))` (l.1123) reads the connected
/// element's conductors UNSHIFTED — phase-A of the source is the negated phase-A
/// branch current. This replaces the retired capi015 0.15.0b4 one-conductor shift
/// (oracle-of-record flip capi015→r4133; `docs/upgrade/DIVERGENCES.md`,
/// `ncim_swing_source_currents`).
#[test]
fn ncim_vsource_reported_currents_match_oracle() {
    let mut dss = solve_ncim(&pq_circuit(1));
    let snap = dss
        .snapshot_elements()
        .into_iter()
        .find(|s| s.name.eq_ignore_ascii_case("Vsource.source"))
        .expect("Vsource.source");

    // r4133 `Vsource.source` terminal currents (A), first three conductors (the
    // second terminal is grounded → 0). Unshifted: conductor k = negated Line.l1
    // terminal-1 conductor k.
    let exp_i = [
        cx(-83.6702850838671, 33.349802451121946),
        cx(70.71691867579727, 55.785691198952236),
        cx(12.953366408072725, -89.13549365007475),
        ZERO,
        ZERO,
        ZERO,
    ];
    for (k, e) in exp_i.iter().enumerate() {
        let got = cx(snap.currents[2 * k], snap.currents[2 * k + 1]);
        assert!(
            (got - e).norm() < 1e-6,
            "source current[{k}]: {got:?} vs r4133 {e:?}"
        );
    }
    // r4133 per-conductor powers (kW/kvar) and total losses (W/var).
    let exp_p = [
        (-602.3890583558023, -240.10383225952395),
        (-602.3890583557892, -240.10383225952782),
        (-602.3890583558059, -240.10383225949838),
    ];
    for (k, (pr, pi)) in exp_p.iter().enumerate() {
        assert!(
            (snap.powers[2 * k] - pr).abs() < 1e-4 && (snap.powers[2 * k + 1] - pi).abs() < 1e-4,
            "source power[{k}]: ({}, {}) vs r4133 ({pr}, {pi})",
            snap.powers[2 * k],
            snap.powers[2 * k + 1]
        );
    }
    assert!(
        (snap.loss_w.0 - (-1807167.1750673973)).abs() < 1e-2
            && (snap.loss_w.1 - (-720311.4967785501)).abs() < 1e-2,
        "source losses: {:?} vs r4133 (-1807167.18, -720311.50)",
        snap.loss_w
    );
}

const ZERO: Complex64 = Complex64::ZERO;

/// `Export Jacobian` with no NCIM solve raises Pascal's #222 "Jacobian matrix not
/// built." (`ExportResults.pas` l.3920) — a normal (fixed-point) solve never
/// populates `NCIM_Jacobian`. `Export deltaF`/`deltaZ` instead silently no-op
/// (Pascal `if Length(..) = 0 then Exit`): no error, nothing written.
#[test]
fn ncim_report_exports_without_ncim_solve() {
    let mut dss = Dss::new();
    for line in as_refs(&pq_circuit(1)) {
        dss.command(line);
    }
    dss.command("Solve"); // Normal algorithm — no NCIM state built.

    dss.command("Export Jacobian");
    assert_eq!(
        dss.error_texts(),
        ["Jacobian matrix not built.".to_string()],
        "Export Jacobian without NCIM should raise #222"
    );
    let n_errs = dss.errors().len();

    dss.command("Export deltaF");
    dss.command("Export deltaZ");
    assert_eq!(
        dss.errors().len(),
        n_errs,
        "deltaF/deltaZ silently no-op with no NCIM state (no new error): {:?}",
        dss.errors()
    );
}

/// `Set Algorithm=NCIM` parses (prefix `nc`, min-abbrev 2) and `Get Algorithm`
/// reads it back; the two NCIM options round-trip through `Set`/`Get`.
#[test]
fn ncim_options_roundtrip() {
    let mut dss = Dss::new();
    for line in as_refs(&pq_circuit(1)) {
        dss.command(line);
    }

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

    let ckt = dss.circuit().unwrap();
    assert!(ckt.solution.ncim_ignore_q_limit);
    assert!((ckt.solution.ncim_gen_gain - 0.75).abs() < 1e-12);
}
