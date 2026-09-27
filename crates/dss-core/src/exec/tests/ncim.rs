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
use crate::support::complexutil::cdang;

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
/// than the +1500 kvar limit, so `UpdateGenQ` clamps Q and converts the
/// generator to model-4 (PQ). **r4133 converges in 4 iters at Q=1500** (live
/// `epri-worker` probe 2026-07-26, `Solution.Iterations`; the retired capi015
/// r4103 cadence took 8 — see ORPHANED_GAPS §1.6 / `DIVERGENCES.md`). The
/// converged node voltages are the same fixpoint on both engines.
#[test]
fn ncim_pv_qlimit_pv2pq_matches_oracle() {
    let mut dss = solve_ncim(&pv_circuit("1.01"));
    let ckt = dss.circuit().unwrap();
    assert!(ckt.is_solved, "NCIM PV (Q-limit) did not converge");
    assert_eq!(
        ckt.solution.iteration, 4,
        "r4133 needs 4 iters for the PV→PQ conversion"
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
    assert_gen_q_clamped(&mut dss);
}

/// The Q clamp of the PV→PQ conversion, as r4133 reports it (live `epri-worker`
/// probe 2026-07-26 on `vpu=1.01` **and** `vpu=1.02`, both identical):
///
/// * `Generator.g1` terminal powers = `(−266.667 kW, −500 kvar)` per conductor —
///   the generator delivers its 800 kW and exactly the +1500 kvar limit;
/// * `Generators.kvar` (`Presentkvar` = `Qnominalperphase·Nphases/1000`) = **0.0**,
///   because `GetNCIMPowers` writes `Qnominalperphase := deltaQNom[j]` only on its
///   model-3 arm (r4133 `Solution.pas` l.1308): once the generator converts to
///   model 4 the last write is iteration 1's zero. (`vpu=1.0`, which never
///   converts, keeps reporting the live 1217.2208409024554 — see
///   [`ncim_pv_regulating_matches_oracle`].)
fn assert_gen_q_clamped(dss: &mut Dss) {
    let (_, kvar) = dss.generator_present_kw_kvar("g1").expect("g1");
    assert!(
        kvar.abs() < KVAR_TOL,
        "r4133 reports Generators.kvar = 0 after the PV→PQ conversion, got {kvar}"
    );
    let snap = dss
        .snapshot_elements()
        .into_iter()
        .find(|s| s.name.eq_ignore_ascii_case("Generator.g1"))
        .expect("Generator.g1");
    for k in 0..3 {
        assert!(
            (snap.powers[k].re - (-266.6666666666667)).abs() < 1e-6
                && (snap.powers[k].im - (-500.0)).abs() < 1e-6,
            "g1 power[{k}] = ({}, {}) vs r4133 (-266.66667, -500.0)",
            snap.powers[k].re,
            snap.powers[k].im
        );
    }
}

/// PV bus whose target **cannot be reached inside the Q-limits**: `vpu=1.02`
/// demands far more than +1500 kvar. Under the r4133 switching cadence the
/// generator converts PV→PQ once, stays PQ (the PQ→PV test is in the `else` arm,
/// so it cannot un-convert in the same pass), and the solve closes in **4 iters
/// at the very same clamped fixpoint as `vpu=1.01`** — live `epri-worker` probe
/// 2026-07-26: converged=True, iterations=4, `genbus.1 = 7212.895598275793 −
/// 70.00930104534099i`.
///
/// The retired capi015 r4103 cadence instead ran the PQ→PV test unconditionally,
/// so the generator flip-flopped PV↔PQ around `|genbus| = VTarget = 7343.55 V`
/// and both capi015 and the port ran out at `MaxIterations` (15). Adopting the
/// r4133 cadence (ORPHANED_GAPS §1.6) removed that stall — this test now pins the
/// convergence so a regression back to the chattering form is caught.
#[test]
fn ncim_pv_aggressive_qlimit_converges_matches_r4133() {
    let mut dss = solve_ncim(&pv_circuit("1.02"));
    let ckt = dss.circuit().unwrap();
    assert!(
        ckt.is_solved,
        "r4133 converges this deck (PV→PQ, Q clamped); the port must match"
    );
    assert_eq!(ckt.solution.iteration, 4, "r4133 converges in 4 iters");

    // r4133 converged node voltages — identical to the `vpu=1.01` fixpoint: both
    // targets are unreachable, so the generator lands on the same +1500 kvar clamp.
    let expected = [
        ORACLE_SOURCEBUS[0],
        ORACLE_SOURCEBUS[1],
        ORACLE_SOURCEBUS[2],
        cx(7212.895598275793, -70.00930104534099),
        cx(-3667.077632344357, -6211.546172429122),
        cx(-3545.8179659314364, 6281.555473474465),
    ];
    assert_nodes(&dss, &expected, V_TOL);
    // NOT at the regulation target (1.02·12470/√3 = 7343.55 V) — the Q clamp binds.
    assert!(
        (ckt.solution.node_v[4].norm() - 7213.23).abs() < 1e-2,
        "|genbus| = {}",
        ckt.solution.node_v[4].norm()
    );
    assert_gen_q_clamped(&mut dss);
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
/// `solution::solution::ncim::ncim_stamp_swing_source_currents`).
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
        let got = snap.currents[k];
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
            (snap.powers[k].re - pr).abs() < 1e-4 && (snap.powers[k].im - pi).abs() < 1e-4,
            "source power[{k}]: ({}, {}) vs r4133 ({pr}, {pi})",
            snap.powers[k].re,
            snap.powers[k].im
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

// ---------------------------------------------------------------------------
// RP3.13 — the NCIM reporting/sizing pins.
//
// Three port defects found by RP3.11/RP3.13 (own measurements 2026-09-03; the
// live oracle is the EPRI `OpenDSSDirect.dll` r4133 through `epri-worker`,
// "Version 11.0.0.1 (64-bit build)"):
//
// * `deltaQNom` was sized to **1** for a machine born `model=4`
//   (`InitPQGen`, r4133 `Common/Solution.pas` l.1678-1679) while the PQ→PV
//   promotion arm (l.2255) writes it per phase — the port panicked
//   (`ncim.rs`: "index out of bounds: the len is 1 but the index is 1");
//   r4133 writes past the end of the dynamic array and **deadlocks**.
// * `DOForceFlatStart` writes `NodeV[1..3]` on any circuit (l.1650-1654) — the
//   port panicked on a 2-node circuit; r4133 overruns and corrupts its heap.
// * `TGeneratorObj.GetCurrents`' NCIM arm (`PCElements/generator.pas` l.1406)
//   was missing, so every ordinary reader (`Export Powers`/`Currents`, `Show`,
//   the CLI, monitors) reported the *declared* `kvar` through `varBase` instead
//   of the solver's dispatched `deltaQNom`, and KCL failed at the generator bus.
// ---------------------------------------------------------------------------

/// The **ordinary** reporting path for one element: `ComputeIterminal` →
/// `TGeneratorObj.GetCurrents`, the exact call `Export Currents`
/// (`report/export/currents.rs`), `Show`, the CLI and the monitors make.
/// Returns the yorder-long terminal-current vector.
fn elem_currents(dss: &mut Dss, name: &str) -> Vec<Complex64> {
    let Dss {
        classes, circuit, ..
    } = dss;
    let ckt = circuit.as_ref().expect("circuit");
    let sys = crate::solution::solution::sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    for class in classes.iter_mut() {
        let cn = class.props.class_name();
        for oi in 0..class.arena.len() {
            let full = format!("{}.{}", cn, class.arena.obj_mut(oi).data().name());
            if full.eq_ignore_ascii_case(name) {
                let elem = class.arena.try_ckt_elem_mut(oi).expect("circuit element");
                elem.compute_iterminal(&sys, &node_v);
                return elem.cd().iterminal.clone();
            }
        }
    }
    panic!("no element {name}");
}

/// `Get_Power(iTerm)` in **kW/kvar** through the ordinary path — what
/// `Export Powers` (`report/export/powers.rs`) prints for terminal `term`
/// (1-based).
fn term_power_kw(dss: &mut Dss, name: &str, term: usize) -> Complex64 {
    let Dss {
        classes, circuit, ..
    } = dss;
    let ckt = circuit.as_ref().expect("circuit");
    let sys = crate::solution::solution::sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    for class in classes.iter_mut() {
        let cn = class.props.class_name();
        for oi in 0..class.arena.len() {
            let full = format!("{}.{}", cn, class.arena.obj_mut(oi).data().name());
            if full.eq_ignore_ascii_case(name) {
                let elem = class.arena.try_ckt_elem_mut(oi).expect("circuit element");
                return elem.terminal_power(&sys, &node_v, term) * 0.001;
            }
        }
    }
    panic!("no element {name}");
}

/// The `Export Powers` / `Export Currents` row for one element, built by the
/// very functions the `Export` command dispatches to.
fn export_row(dss: &mut Dss, report: &str, elem: &str) -> String {
    let Dss {
        classes, circuit, ..
    } = dss;
    let ckt = circuit.as_ref().expect("circuit");
    let sys = crate::solution::solution::sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    let body = match report {
        "powers" => crate::report::export::export_powers(classes, ckt, &sys, &node_v, 0),
        "currents" => crate::report::export::export_currents(classes, ckt, &sys, &node_v),
        other => panic!("unknown report {other}"),
    };
    body.lines()
        .find(|l| l.to_ascii_lowercase().contains(&elem.to_ascii_lowercase()))
        .unwrap_or_else(|| panic!("no {elem} row in {report}"))
        .to_string()
}

/// The generator's live NCIM state `(gen_model, ncim_expv, ncim_idx,
/// p_nominal_per_phase, delta_q_nom, node_ref)`.
#[allow(clippy::type_complexity)]
fn gen_ncim_state(dss: &Dss, name: &str) -> (i32, bool, i32, f64, Vec<f64>, Vec<usize>) {
    let ckt = dss.circuit().expect("circuit");
    for &r in &ckt.generators {
        let g = dss.classes[r.class_ord()]
            .arena
            .get::<crate::elements::pc::generator::Generator>(r.index())
            .expect("generators list holds Generators");
        if g.cd.obj.name().eq_ignore_ascii_case(name) {
            return (
                g.gen_model,
                g.ncim_expv,
                g.ncim_idx,
                g.p_nominal_per_phase,
                g.delta_q_nom.clone(),
                g.cd.node_ref.clone(),
            );
        }
    }
    panic!("no Generator.{name}");
}

/// **P1** — a generator born `model=4` that the NCIM PQ→PV arm promotes must
/// solve, not panic, and the promotion's own state must stay self-consistent.
///
/// The deck (`tmp/rp313/repro_pq2pv.dss`) is `pv_circuit` with the machine
/// authored `model=4 PF=0.88` and `vpu=0.95`, so `UpdateGenQ`'s ELSE arm finds
/// `|V| > VTarget` with `deltaQNom[0] > 0` and promotes it to PV
/// (r4133 `Common/Solution.pas` l.2216-2258). That write —
/// `deltaQNom[j] := qMax|qMin` for `j := 0 .. NPhases-1` over the array
/// `InitPQGen` sized to **1** (l.1678-1679) — is the defect: the port panicked
/// ("index out of bounds: the len is 1 but the index is 1"), and the unchecked
/// overrun **corrupts the r4133 DLL**.
///
/// What r4133 can and cannot answer here, re-measured in the RP3.13 audit
/// settlement (2026-09-03; the sub-step's first probe read elements and so
/// recorded the whole deck as unanswerable — it is not): the **solve** answers
/// in seconds — `converged=True`, `iterations=5` and the six `YNodeVarray`
/// entries pinned below — and the DLL then **hangs on the first element access
/// after it** (`set_active_element Line.l1` never returns; a run killed at 150 s
/// had burned 0.12 s of worker CPU: blocked, not spinning). So the convergence
/// flag, the iteration count and the node voltages ARE oracle-pinned below
/// against live r4133, and only the terminal-power leg — which needs the element
/// rows r4133 cannot hand out on this deck — is physics (KCL).
///
/// What the port now does, with `deltaQNom` sized per phase: promote to PV on
/// the first pass (which is why `NCIM_ExPV` can be set at all — only the PV→PQ
/// conversion sets it, and only a model-3 machine reaches that), overshoot the
/// −1500 kvar limit, convert back to PQ at `qMin`, and converge in 5 iterations
/// with the machine absorbing exactly its 1500 kvar limit.
#[test]
fn ncim_pq2pv_promotion_does_not_panic_and_closes_kcl() {
    let mut dss = run(&[
        "Clear",
        "New circuit.ncimpq2pv basekv=12.47 phases=3 bus1=sourcebus",
        "New Line.l1 bus1=sourcebus bus2=genbus phases=3 r1=0.12 x1=0.35 length=3",
        "New Load.ld1 bus1=genbus phases=3 kv=12.47 kw=2000 kvar=800 model=1",
        "New Generator.g1 PF=0.88 bus1=genbus phases=3 kv=12.47 kw=800 model=4 \
         maxkvar=1500 minkvar=-1500 vpu=0.95",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
        "Set algorithm=NCIM",
        "Solve",
    ]);

    let ckt = dss.circuit().expect("circuit");
    assert!(
        ckt.is_solved,
        "the promoted machine's solve must converge (r4133: converged=True)"
    );
    assert_eq!(
        ckt.solution.iteration, 5,
        "converges in 5 NCIM iterations (r4133: 5)"
    );
    // Live r4133 `YNodeVarray` on this deck (`epri-worker`, OpenDSSDirect.dll
    // r4133 "Version 11.0.0.1 (64-bit build)", own probe 2026-09-03 — the solve
    // answers, only the element reads after it hang).
    const R4133_NODE_V: [(f64, f64); 6] = [
        (7199.557856794634, 0.0),
        (-3599.77892839732, -6234.999999999999),
        (-3599.778928397315, 6235.000000000001),
        (7065.195045516548, -20.00602724009179),
        (-3549.923250576999, -6108.635378489235),
        (-3515.271794939548, 6128.641405729329),
    ];
    for (k, &(re, im)) in R4133_NODE_V.iter().enumerate() {
        let v = ckt.solution.node_v[k + 1];
        assert!(
            v.re.is_finite() && v.im.is_finite(),
            "node {} = {v:?}",
            k + 1
        );
        assert!(
            (v.re - re).abs() < 1e-6 && (v.im - im).abs() < 1e-6,
            "node {}: {v:?} V vs live r4133 ({re}, {im})",
            k + 1
        );
    }

    // The promotion ran: `NCIM_ExPV` is set only by the PV→PQ conversion
    // (r4133 l.2119-2122), which only a model-3 machine reaches — and this one
    // was authored `model=4`. `NCIM_Idx` is likewise assigned only on
    // `GetNumGenerators`' model-3 branch (l.1955-1968).
    let (model, expv, idx, _, dq, _) = gen_ncim_state(&dss, "g1");
    assert!(
        expv,
        "the machine must have been promoted PQ→PV at least once"
    );
    assert_eq!(idx, 1, "the promoted machine got a Jacobian gen slot");
    assert_eq!(
        model, 4,
        "it then re-converted PV→PQ on the −1500 kvar limit"
    );
    assert_eq!(
        dq,
        vec![-500_000.0; 3],
        "deltaQNom is per-phase (the fix) and holds qMin = -1500 kvar / 3 phases"
    );

    // Physics: Σ terminal powers at `genbus` = 0.
    let kcl = term_power_kw(&mut dss, "Generator.g1", 1)
        + term_power_kw(&mut dss, "Load.ld1", 1)
        + term_power_kw(&mut dss, "Line.l1", 2);
    assert!(
        kcl.norm() < 1e-6,
        "KCL at genbus = {kcl:?} kW/kvar (must be 0)"
    );
    // The machine absorbs its full −1500 kvar limit (terminal power = −S).
    let s = term_power_kw(&mut dss, "Generator.g1", 1);
    assert!(
        (s.re + 800.0).abs() < 1e-6 && (s.im - 1500.0).abs() < 1e-6,
        "Generator.g1 terminal power = {s:?} kW/kvar, expected (-800, +1500)"
    );
}

/// **P2** — `Set VoltageBases` without `CalcVoltageBases`: the 11-line RP3.11
/// repro (`tmp/rp311/repro_panic.dss`) that first exposed the length-1
/// `deltaQNom` write. Every bus keeps `kVBase = 0`, so the promoted machine's
/// `VTarget = VBase·Vpu = 0` and the Newton step diverges — but it must
/// *diverge*, not panic.
///
/// r4133 on the identical deck, live `epri-worker` probe 2026-09-03:
/// `converged=False`, `iterations=15`, `SOURCEBUS.1..3` =
/// `7199.557856794634 + 0j`, `-3599.77892839732 - 6234.999999999999j`,
/// `-3599.778928397315 + 6235.000000000001j`, **`GENBUS.1..3 = NaN`** with every
/// element power and current NaN. The port now reproduces that exactly: the
/// swing bus is the ideal EMF (NCIM never moves it) and the genbus voltages go
/// NaN on both engines. Only the source bus carries information here, so only it
/// is pinned numerically; the genbus assertion pins the *shape* (not-a-number,
/// not a panic and not a plausible-looking wrong answer).
#[test]
fn ncim_missing_voltage_bases_does_not_panic() {
    let dss = run(&[
        "Clear",
        "New circuit.ncimpanic basekv=12.47 phases=3 bus1=sourcebus",
        "New Line.l1 bus1=sourcebus bus2=genbus phases=3 r1=0.12 x1=0.35 length=3",
        "New Load.ld1 bus1=genbus phases=3 kv=12.47 kw=2000 kvar=800 model=1",
        "New Generator.g1 PF=0.88 bus1=genbus phases=3 kv=12.47 kw=800 model=4 \
         maxkvar=1500 minkvar=-1500 vpu=1.01",
        "MakeBusList",
        "Set VoltageBases=(12.47, )",
        // NO CalcVoltageBases — that is the whole point of the deck.
        "Set algorithm=NCIM",
        "Solve",
    ]);
    let ckt = dss.circuit().expect("circuit");
    assert!(!ckt.is_solved, "r4133 reports converged=False here");
    assert_eq!(
        ckt.solution.iteration, 15,
        "runs out at the default MaxIterations, as r4133 does"
    );
    // r4133's source bus, to the digit.
    let src = [
        cx(7199.557856794634, 0.0),
        cx(-3599.77892839732, -6234.999999999999),
        cx(-3599.778928397315, 6235.000000000001),
    ];
    for (k, e) in src.iter().enumerate() {
        let v = ckt.solution.node_v[k + 1];
        assert!(
            (v - e).norm() < V_TOL,
            "sourcebus node {}: {v:?} vs r4133 {e:?}",
            k + 1
        );
    }
    for k in 4..=6 {
        let v = ckt.solution.node_v[k];
        assert!(
            v.re.is_nan() && v.im.is_nan(),
            "genbus node {k} = {v:?}; r4133 reports NaN here too"
        );
    }
}

/// **P3** — a circuit with fewer than three nodes must not panic in the NCIM
/// flat start. `DOForceFlatStart` writes `NodeV[1..3]` unconditionally (r4133
/// `Common/Solution.pas` l.1650-1654); on this 1-phase, 2-node,
/// generator-free deck (`tmp/rp313/repro_1ph_nogen.dss`) the port indexed
/// `node_v[3]` of a length-3 vector and panicked
/// ("index out of bounds: the len is 3 but the index is 3").
///
/// NCIM is 3-phase-shaped throughout — `CalcInjCurr` zeroes `deltaF[0..5]` as
/// "the swing bus" (l.1874-1875), which on a 2-node circuit is the whole
/// 4-long mismatch vector, so nothing can move — and both engines therefore
/// report non-convergence at `MaxIterations`. r4133 (live probe 2026-09-03)
/// answers `converged=False`, 15 iterations, `SOURCEBUS.1 = 7199.557856794634 +
/// 0j`, `LOADBUS.1 = -2432.428875690357 - 4868.236987187187j` and then its
/// worker process **cannot exit** (`quit` times out after 30 s) — the overrun
/// corrupted its heap. The port's clamp lands on r4133's two numbers to the
/// digit, which is what is pinned; the heap corruption is not reproduced.
#[test]
fn ncim_below_three_nodes_does_not_panic() {
    let dss = run(&[
        "Clear",
        "New circuit.ncim1phng basekv=12.47 phases=1 bus1=sourcebus",
        "New Line.l1 bus1=sourcebus bus2=loadbus phases=1 r1=0.12 x1=0.35 length=3",
        "New Load.ld1 bus1=loadbus phases=1 kv=7.2 kw=700 kvar=270 model=1",
        "Set voltagebases=[12.47]",
        "Calcvoltagebases",
        "Set algorithm=NCIM",
        "Solve",
    ]);
    let ckt = dss.circuit().expect("circuit");
    assert_eq!(ckt.num_nodes, 2, "the deck must stay under three nodes");
    assert!(!ckt.is_solved, "r4133 reports converged=False here");
    assert_eq!(ckt.solution.iteration, 15);
    let expected = [
        cx(7199.557856794634, 0.0),
        cx(-2432.428875690357, -4868.236987187187),
    ];
    assert_nodes(&dss, &expected, V_TOL);
}

/// **P4** — under NCIM a generator reports the Q the solver *dispatched*, not
/// the `kvar` the deck declared. `TGeneratorObj.GetCurrents`
/// (r4133 `PCElements/generator.pas` l.1406-1410) reports `ITerminal` — the
/// stamp `UpdateGenQ` wrote from `deltaQNom` (`Common/Solution.pas` l.2108 /
/// l.2305) — and never consults the machine's power-flow kernels, whose
/// `varBase`/`YQFixed` are frozen at `SetNominalGeneration` time from the
/// declared `kvar`.
///
/// Measured 2026-09-03 on the unmodified corpus decks, port **before** the fix
/// vs live r4133:
///
/// ```text
/// modes/ncim/ncim_pv_pq  Generator.G1  P/Q  port -800.0, -431.8   r4133 -800.0, -1500.0
///                                      |I|  port 42.0103 ∠151.09  r4133 78.5593 ∠117.52
/// modes/ncim/ncim_midi   Generator.G1  P/Q  port -600.0, -323.8   r4133 -600.0,  -400.0
///                                      |I|  port 31.9542 ∠150.60  r4133 33.7957 ∠145.27
/// ```
///
/// `-431.8` is exactly the declared `kvar` (`kw=800` at the default `PF=0.88`),
/// `-323.8` likewise (`kw=600`) — quantities the solve never used — and the
/// port's rows missed KCL at the generator bus by 1068.2 kvar (`ncim_pv_pq`,
/// where `Line.L1 t2 = (-1200, 700)` and `Load.LD1 = (2000, 800)`) and by
/// 76.2 kvar (`ncim_midi` at `b5`). Every other row and every node voltage was
/// already digit-identical between the engines.
#[test]
fn ncim_generator_reports_the_dispatched_q_not_the_declared_kvar() {
    // ---- leg 1: ncim_pv_pq (== `pv_circuit("1.01")`, the corpus deck) --------
    let mut dss = solve_ncim(&pv_circuit("1.01"));
    let s = term_power_kw(&mut dss, "Generator.g1", 1);
    assert!(
        (s.re + 800.0).abs() < 1e-6 && (s.im + 1500.0).abs() < 1e-6,
        "Generator.g1 = {s:?} kW/kvar; r4133 (-800.0, -1500.0), port-before \
         (-800.0, -431.8) = the declared kvar"
    );
    let i = elem_currents(&mut dss, "Generator.g1");
    assert!(
        (i[0].norm() - 78.5593).abs() < 5e-5 && (cdang(i[0]) - 117.52).abs() < 5e-3,
        "Generator.g1 |I1| ∠ = {} ∠{}; r4133 78.5593 ∠117.52, port-before \
         42.0103 ∠151.09",
        i[0].norm(),
        cdang(i[0])
    );
    let kcl = s + term_power_kw(&mut dss, "Load.ld1", 1) + term_power_kw(&mut dss, "Line.l1", 2);
    assert!(kcl.norm() < 1e-6, "KCL at genbus = {kcl:?} kW/kvar");
    // The printed reports, i.e. what RP3.11 measured against r4133's own files.
    assert_eq!(
        export_row(&mut dss, "powers", "Generator.G1"),
        "\"Generator.G1\",   1, -800.0, -1500.0",
        "r4133 prints `\"Generator.G1\", 1, -800.0, -1500.0`"
    );
    let irow = export_row(&mut dss, "currents", "Generator.G1");
    assert!(
        irow.starts_with(
            "Generator.G1, 78.5593, 117.52, 78.5593, -2.48, 78.5593, -122.48, 0, 0.00,"
        ),
        "Export Currents row = {irow:?}"
    );

    // ---- leg 2: ncim_midi (the 27-node corpus feeder) -----------------------
    let deck = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/modes/ncim/ncim_midi.dss");
    assert!(deck.is_file(), "vendored corpus deck missing: {deck:?}");
    let mut midi = Dss::new();
    midi.command(&format!("compile \"{}\"", deck.display()));
    assert!(midi.errors().is_empty(), "ncim_midi: {:?}", midi.errors());
    let s = term_power_kw(&mut midi, "Generator.g1", 1);
    assert!(
        (s.re + 600.0).abs() < 1e-6 && (s.im + 400.0).abs() < 1e-6,
        "Generator.g1 = {s:?} kW/kvar; r4133 (-600.0, -400.0), port-before \
         (-600.0, -323.8)"
    );
    let i = elem_currents(&mut midi, "Generator.g1");
    assert!(
        (i[0].norm() - 33.7957).abs() < 5e-5 && (cdang(i[0]) - 145.27).abs() < 5e-3,
        "Generator.g1 |I1| ∠ = {} ∠{}; r4133 33.7957 ∠145.27, port-before \
         31.9542 ∠150.60",
        i[0].norm(),
        cdang(i[0])
    );
    // r4133 at `b5`: Line.B4_5 t2 (29.9, 190.0) + Line.B5_6 t1 (270.1, 90.0)
    // + Load.D5 (300.0, 120.0) + Generator.G1 (-600, -400) = (0, 0).
    let kcl = s
        + term_power_kw(&mut midi, "Load.d5", 1)
        + term_power_kw(&mut midi, "Line.b4_5", 2)
        + term_power_kw(&mut midi, "Line.b5_6", 1);
    assert!(kcl.norm() < 1e-9, "KCL at b5 = {kcl:?} kW/kvar");
}

/// **P5** — the gate reader and the ordinary reader are now one live state.
///
/// Until RP3.13 the corpus gate's reader ([`Dss::snapshot_elements`]) carried a
/// private `ncim_generator_currents` override that recomputed
/// `-conj((Pnom + j·deltaQNom[j]) / V)` — r4133's `UpdateGenQ` stamp
/// (`Common/Solution.pas` l.2108) — while the element's own `GetCurrents`
/// returned the stale model-kernel current. That is why the gate never saw
/// [`ncim_generator_reports_the_dispatched_q_not_the_declared_kvar`]. The
/// override is deleted; this pin carries its arithmetic and proves the deletion
/// lossless: the retired formula, the gate reader and the ordinary element path
/// must agree **exactly** (`==`, not a tolerance).
#[test]
fn ncim_gate_reader_and_ordinary_reader_agree() {
    let mut dss = solve_ncim(&pv_circuit("1.01"));
    let (_, _, _, p_nom, dq, node_ref) = gen_ncim_state(&dss, "g1");
    let node_v = dss.circuit().expect("circuit").solution.node_v.clone();

    // The retired `exec/view.rs::ncim_generator_currents` override, verbatim.
    let mut retired = vec![Complex64::ZERO; node_ref.len()];
    for (j, c) in retired.iter_mut().enumerate().take(3) {
        let nr = node_ref[j];
        if nr == 0 {
            continue;
        }
        let q = dq.get(j).copied().unwrap_or(dq[0]);
        *c = -(Complex64::new(p_nom, q) / node_v[nr]).conj();
    }

    let ordinary = elem_currents(&mut dss, "Generator.g1");
    let snap = dss
        .snapshot_elements()
        .into_iter()
        .find(|s| s.name.eq_ignore_ascii_case("Generator.g1"))
        .expect("Generator.g1");
    assert_eq!(ordinary.len(), retired.len());
    assert_eq!(snap.currents.len(), retired.len());
    for k in 0..retired.len() {
        assert_eq!(
            ordinary[k], retired[k],
            "conductor {k}: the element path must equal the retired override"
        );
        assert_eq!(
            snap.currents[k], retired[k],
            "conductor {k}: the gate reader must equal the retired override"
        );
    }
    // …and the powers the gate compares are unmoved: r4133's clamped
    // (−266.667 kW, −500 kvar) per conductor (see `assert_gen_q_clamped`).
    for k in 0..3 {
        assert!(
            (snap.powers[k].re - (-266.6666666666667)).abs() < 1e-6
                && (snap.powers[k].im - (-500.0)).abs() < 1e-6,
            "gate power[{k}] = {:?}",
            snap.powers[k]
        );
    }
}

/// **P7** — the tripwire behind "the corpus gate cannot move" (RP3.13 §3).
///
/// The swing source's NCIM reported current is the Kirchhoff sum at its bus
/// (`TVsourceObj.GetCurrents` → `CalcInjCurrAtBus`, r4133 `VSource.pas` l.1194;
/// port `solution::solution::ncim::ncim_stamp_swing_source_currents`), summed
/// from the connected elements' `Iterminal`. Two RP3.13 changes therefore *would*
/// move that sum, but only for a **PC element other than the swing source sitting
/// on the swing bus**: making the generator's `Iterminal` the NCIM dispatch stamp,
/// and (from the audit settlement) subtracting the PC terms where r4133 adds them
/// (`VSource.pas` l.1169 — the upstream KCL sign bug, documented at
/// `ncim_stamp_swing_source_currents` and pinned by
/// [`ncim_swing_sum_subtracts_pc_terminals_and_closes_kcl`]). None of the five
/// r4133-gated NCIM cases has such an element, and this pin checks all five:
///
/// * `ncim_pq` / `ncim_pv_pq` / `ncim_midi` — compiled and solved here, then
///   every enabled PC element (generators, loads, sources, …) is checked against
///   the swing source's own bus. `ncim_pq` has no generator at all; `ncim_pv_pq`
///   and `ncim_midi` put theirs at `genbus`/`b5`.
/// * `Xmission_System_Kundur2Area` (swing bus `b1`) and `IEEE118Bus`
///   (`89_clinchrv`) — their masters end in `export`/`show`/`summary`, which a
///   compile inside a unit test would write into the vendored corpus tree (see
///   [`ncim_vsource_export_currents_match_oracle`]), so they are checked by
///   scanning every deck file in their directories for an **uncommented**
///   PC-class binding to the swing bus. Today `Generators.DSS:5`
///   (`!New Generator.G1 Bus1=B1 …`) and `generators.dss:44`
///   (`! New Generator.Gen_at_89_1 bus1=89_clinchrv …`) are both commented out;
///   the day a re-vendor uncomments either — or adds any other machine, load or
///   source there — this reds instead of the divergence appearing silently in
///   that case's `Vsource` row.
#[test]
fn ncim_swing_bus_carries_no_pc_element_on_the_gated_decks() {
    let base =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/modes/ncim");
    for deck in ["ncim_pq.dss", "ncim_pv_pq.dss", "ncim_midi.dss"] {
        let path = base.join(deck);
        assert!(path.is_file(), "vendored corpus deck missing: {path:?}");
        let mut dss = Dss::new();
        dss.command(&format!("compile \"{}\"", path.display()));
        assert!(dss.errors().is_empty(), "{deck}: {:?}", dss.errors());
        let ckt = dss.circuit().expect("circuit");

        // The stamped swing source: the first enabled `VSource` on the global
        // slack node (`ncim_stamp_swing_source_currents`'s own rule), and its bus.
        let elem_of = |r: crate::elements::ElemId| {
            dss.classes[r.class_ord()]
                .arena
                .try_ckt_elem(r.index())
                .expect("element lists hold circuit elements")
                .cd()
        };
        let swing = ckt
            .sources
            .iter()
            .copied()
            .find(|&r| {
                let cd = elem_of(r);
                cd.enabled && cd.node_ref.first() == Some(&1)
            })
            .unwrap_or_else(|| panic!("{deck}: no enabled source on the slack node"));
        let swing_bus = elem_of(swing).terminals.first().and_then(|t| t.bus_ref);

        // Every other enabled PC element (generators, loads, sources, …) must be
        // off that bus: it is exactly the set `CalcInjCurrAtBus`' PC loop sums.
        for &r in ckt.pc_elements.iter().chain(ckt.sources.iter()) {
            if r == swing {
                continue;
            }
            let cd = elem_of(r);
            if !cd.enabled {
                continue;
            }
            assert!(
                !cd.terminals.iter().any(|t| t.bus_ref == swing_bus),
                "{deck}: {} sits on the swing bus — the swing-source Kirchhoff sum \
                 reads its terminal current (and the port subtracts it where r4133 \
                 adds it), so this case's `Vsource` row moves and needs a pin",
                cd.obj.name()
            );
        }
    }

    // The two large gated NCIM cases, by source scan (see the doc comment).
    let corpus = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    for (dir, swing) in [
        (
            "electricdss-tst/Version8/Distrib/Examples/NCIM/Xmission_System_Kundur2Area",
            "b1",
        ),
        (
            "electricdss-tst/Version8/Distrib/IEEETestCases/IEEE118Bus",
            "89_clinchrv",
        ),
    ] {
        let dir = corpus.join(dir);
        assert!(dir.is_dir(), "vendored corpus deck dir missing: {dir:?}");
        let hits = pc_bindings_to_bus(&dir, swing);
        assert!(
            hits.is_empty(),
            "{}: PC element(s) bound to the swing bus `{swing}`: {hits:?} - the \
             swing-source Kirchhoff sum now reads their terminal currents, so this \
             case's `Vsource` row moves and needs a pin",
            dir.display()
        );
    }
}

/// Every **uncommented** PC-class object in `dir`'s `.dss` files whose `bus1=`
/// (or `bus=`) binds it to `bus` — the source-level form of P7's structural
/// check, for the two gated NCIM decks a unit test must not compile.
///
/// `New Circuit.…` is deliberately not a PC class here: the circuit's own swing
/// `VSource` is the element the sum is written *for*, not one of its terms.
fn pc_bindings_to_bus(dir: &std::path::Path, bus: &str) -> Vec<String> {
    const PC_CLASSES: [&str; 12] = [
        "load",
        "generator",
        "pvsystem",
        "storage",
        "vsource",
        "isource",
        "windgen",
        "vccs",
        "gicline",
        "upfc",
        "indmach012",
        "generic5",
    ];
    let mut hits = Vec::new();
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {dir:?}: {e}"))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dss")))
        .collect();
    files.sort();
    for path in files {
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        // The class/name of the object the `~` continuation lines belong to.
        let mut current: Option<String> = None;
        for (n, raw) in text.lines().enumerate() {
            // `!` starts a comment — the vendored decks disable an element by
            // commenting its whole line, which is exactly what this must honour.
            let line = raw.split('!').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let low = line.to_ascii_lowercase();
            if let Some(rest) = low.strip_prefix("new ").map(str::trim_start) {
                current = match rest.split_once('.') {
                    Some((class, name)) if PC_CLASSES.contains(&class) => Some(format!(
                        "{}:{} {class}.{}",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        n + 1,
                        name.split_whitespace().next().unwrap_or(name)
                    )),
                    _ => None,
                };
            } else if !low.starts_with('~') {
                // Any other command ends the object the `~` lines were editing.
                current = None;
            }
            let Some(tag) = current.clone() else { continue };
            for key in ["bus1=", "bus="] {
                let Some(v) = low.split(key).nth(1) else {
                    continue;
                };
                let v = v.split_whitespace().next().unwrap_or("");
                // `bus1=89_clinchrv.1.2.3` binds the same bus as `bus1=89_clinchrv`.
                if v.split('.').next().unwrap_or(v) == bus {
                    hits.push(tag);
                    break;
                }
            }
        }
    }
    hits
}

/// **P6** — the swing `VSource`'s NCIM terminal currents, through the
/// **ordinary** reader (`Export Currents`), against live r4133.
///
/// `TVsourceObj.GetCurrents` has the same NCIM arm the generator has —
/// `if ((Algorithm = NCIMSOLVE) and (NodeRef[1] = 1)) then CalcInjCurrAtBus(Curr,
/// ActorID)` (r4133 `PCElements/VSource.pas` l.1194) — and the port had none, so
/// the element path fell through to `YPrim·V - Iinj`, which at NCIM's ideal-EMF
/// swing bus cancels to numerical zero. Measured 2026-09-03 on the unmodified
/// corpus decks, `Export Currents`' `Vsource.SOURCE` `|I1|`: port-before
/// `3.24074e-05 A` (`ncim_pq`), `3.24074e-05 A` (`ncim_pv_pq`),
/// `4.42577e-05 A` (`ncim_midi`) — against live r4133 (EPRI OpenDSSDirect.dll
/// r4133 "Version 11.0.0.1 (64-bit build)" via `epri-worker`, own probe
/// 2026-09-03, its own `EXP_CURRENTS.CSV` rows):
///
/// ```text
/// ncim_pq       Vsource.SOURCE,  90.0718, 158.27,  90.0718,  38.27,  90.0718,  -81.73, 0, 0.00, 4.3961E-012,  130.28, 0, 0.00, ...
/// ncim_pv_pq    Vsource.SOURCE,  64.2127,-150.28,  64.2127,  89.72,  64.2127,  -30.28, 0, 0.00, 1.13153E-012,  25.28, 0, 0.00, ...
/// ncim_midi     Vsource.SOURCE,  124.964, 161.49,  124.964,  41.49,  124.964,  -78.51, 0, 0.00, 1.57857E-011, -41.50, 0, 0.00, ...
/// ```
///
/// RP3.13 stamps the `CalcInjCurrAtBus` sum into `Iterminal` at the converged
/// `NodeV` (`solution::solution::ncim::ncim_stamp_swing_source_currents`) and the
/// element's NCIM arm echoes it, so the port now prints those same three rows
/// cell for cell. The fifth cell of each row is the terminal-1 **residual**
/// (`Iresid` = the three phase currents summed): both engines print numerical
/// zero there — r4133 `4.3961E-012` / `1.13153E-012` / `1.57857E-011`, the port
/// `8.15881E-12` / `2.11546E-12` / `3.63457E-11`, i.e. `~1e-14` relative to the
/// phase current, the round-off floor of a three-term cancellation and not a
/// divergence; it is neither pinned nor read by any gate channel.
///
/// The fourth NCIM case, `Xmission_System_Kundur2Area` (r4133 `20295.6 A`,
/// port-before `0.0106809 A`), is not a leg here: its `Master.dss` ends in
/// `export`/`show`/`summary`, which a unit test would write into the vendored
/// corpus tree. It is covered live instead — the corpus gate solves it on the
/// `r4133` channel and compares `Vsource.source` in `snapshot_elements`, which
/// this pin's reader-agreement leg ties to the element path.
#[test]
fn ncim_vsource_export_currents_match_oracle() {
    let base =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/modes/ncim");
    // (deck, r4133 `Export Currents` row prefix, r4133 |I1| / ∠, the PD element
    // sharing the swing bus, the port's pre-RP3.13 |I1|).
    let cases: [(&str, &str, f64, f64, f64, &str, &str); 3] = [
        (
            "ncim_pq.dss",
            "Vsource.SOURCE, 90.0718, 158.27, 90.0718, 38.27, 90.0718, -81.73, 0, 0.00,",
            90.0718,
            158.27,
            5e-5,
            "Line.l1",
            "3.24074e-05",
        ),
        (
            "ncim_pv_pq.dss",
            "Vsource.SOURCE, 64.2127, -150.28, 64.2127, 89.72, 64.2127, -30.28, 0, 0.00,",
            64.2127,
            -150.28,
            5e-5,
            "Line.l1",
            "3.24074e-05",
        ),
        (
            "ncim_midi.dss",
            "Vsource.SOURCE, 124.964, 161.49, 124.964, 41.49, 124.964, -78.51, 0, 0.00,",
            124.964,
            161.49,
            5e-4,
            "Line.b0_1",
            "4.42577e-05",
        ),
    ];
    for (deck, r4133_row, r4133_mag, r4133_ang, mag_tol, partner, port_before) in cases {
        let path = base.join(deck);
        assert!(path.is_file(), "vendored corpus deck missing: {path:?}");
        let mut dss = Dss::new();
        dss.command(&format!("compile \"{}\"", path.display()));
        assert!(dss.errors().is_empty(), "{deck}: {:?}", dss.errors());

        // The printed report — the surface a user reads, and the one that was
        // wrong. r4133 prints 6 significant digits, so the floor is that print
        // resolution (half an ulp of the last printed digit), not a tolerance
        // chosen to pass.
        let row = export_row(&mut dss, "currents", "Vsource.SOURCE");
        assert!(
            row.starts_with(r4133_row),
            "{deck}: Export Currents row\n  port  {row}\n  r4133 {r4133_row} ...\n  \
             (port before RP3.13: |I1| = {port_before} A — `YPrim·V - Iinj` cancelling \
             at the ideal-EMF swing bus)"
        );
        let i = elem_currents(&mut dss, "Vsource.source");
        assert!(
            (i[0].norm() - r4133_mag).abs() < mag_tol && (cdang(i[0]) - r4133_ang).abs() < 5e-3,
            "{deck}: Vsource.source |I1| ∠ = {} ∠{}; r4133 {r4133_mag} ∠{r4133_ang}, \
             port before RP3.13 {port_before}",
            i[0].norm(),
            cdang(i[0])
        );

        // KCL at the swing bus, through the *power* path: the source's terminal
        // power and the connected line's must cancel. Under the stamp the current
        // is the negated branch current by construction, so this holds the sign
        // convention and proves `Get_Power` reads the same live state as
        // `GetCurrents` (before RP3.13 the source's power was ~0 while the line
        // carried megawatts).
        let sp = term_power_kw(&mut dss, "Vsource.source", 1);
        let pp = term_power_kw(&mut dss, partner, 1);
        assert!(
            (sp + pp).norm() < 1e-9,
            "{deck}: KCL at the swing bus: Vsource {sp:?} + {partner} {pp:?}"
        );
        assert!(
            sp.norm() > 1e3,
            "{deck}: the swing source must carry the feeder, not ~0: {sp:?} kW/kvar"
        );

        // One live state: the corpus gate's reader and the element path are the
        // same numbers, exactly — what the retired `exec/view.rs` override used to
        // guarantee only for the gate.
        let snap = dss
            .snapshot_elements()
            .into_iter()
            .find(|s| s.name.eq_ignore_ascii_case("Vsource.source"))
            .expect("Vsource.source");
        assert_eq!(
            snap.currents, i,
            "{deck}: snapshot_elements and the element path must be one live state"
        );
    }

    // Cross-check leg: on `ncim_pq` the ordinary reader must land on the very
    // complex numbers [`ncim_vsource_reported_currents_match_oracle`] pins from
    // r4133's f64 API (`Vsource.source.Currents`, own probe 2026-07-20) — the
    // same current through the other reader.
    let mut dss = Dss::new();
    dss.command(&format!(
        "compile \"{}\"",
        base.join("ncim_pq.dss").display()
    ));
    let i = elem_currents(&mut dss, "Vsource.source");
    let exp_i = [
        cx(-83.6702850838671, 33.349802451121946),
        cx(70.71691867579727, 55.785691198952236),
        cx(12.953366408072725, -89.13549365007475),
        ZERO,
        ZERO,
        ZERO,
    ];
    for (k, e) in exp_i.iter().enumerate() {
        assert!(
            (i[k] - e).norm() < 1e-6,
            "ncim_pq: element-path current[{k}]: {:?} vs r4133 {e:?}",
            i[k]
        );
    }
    // ...and the terminal power r4133 reports as the source's losses,
    // `-1807167.1750673973 + j -720311.4967785501` W (same probe).
    let sp = term_power_kw(&mut dss, "Vsource.source", 1);
    assert!(
        (sp.re - (-1807.1671750673973)).abs() < 1e-5 && (sp.im - (-720.3114967785501)).abs() < 1e-5,
        "ncim_pq: Vsource.source terminal power {sp:?} kW/kvar vs r4133 \
         (-1807.16717507, -720.31149678)"
    );
}

/// **P8** — a *second* `Vsource` on the global slack node reports its own
/// terminal current, not the swing stamp (RP3.13 micro-part S2).
///
/// [`ncim_vsource_export_currents_match_oracle`]'s stamp/echo pair
/// (`solution::solution::ncim::ncim_stamp_swing_source_currents` writes the
/// `CalcInjCurrAtBus` sum into one source's `Iterminal`, `VSource::get_currents`
/// echoes it) has one shape r4133's per-read recompute does not: a slack-node
/// source the solver never stamped. Measured 2026-09-03 on the deck below —
/// two `Vsource`s at `sourcebus`, the second at `pu=1.02` — an echo keyed on
/// `NodeRef[1] = 1` alone made `Export Currents` print
/// `Vsource.SRC2, 0, 0.00, 0, 0.00, 0, 0.00` for a source carrying
/// `1388.97 A ∠104.04°`, and it poisoned the swing sum too (`Vsource.SOURCE`
/// read `100.404 A`, i.e. minus the line current with `SRC2` counted as zero).
/// Keying the echo on [`VSource::ncim_swing_stamped_at`] instead restores
/// `SRC2, 1388.97, 104.04, …` (its own `YPrim·V - Iinj`, the value the port
/// printed before the arm existed) and makes the swing sum
/// `SOURCE, 1450.64, 107.23, …` = `-I(Line.l1 t1) + I(Vsource.src2 t1)`, the
/// Pascal formula with every element at the bus counted.
///
/// **r4133 cannot be the oracle here**: its `GetCurrents` re-runs
/// `CalcInjCurrAtBus` for *both* sources, and each one's PC loop calls the
/// other's `GetCurrents` (`VSource.pas` l.1158; l.1149 excludes only the
/// element itself), so the two recurse until the stack dies — own probe
/// 2026-09-03, the r4133 DLL prints `thread 'main' has overflowed its stack`
/// and the `epri-worker` process is killed on `export currents`. So this pin
/// asserts physics and the port's own internal identity, not an oracle row:
/// `SRC2`'s magnitude is its Thevenin current `|E2 - V| / |Z1|` =
/// `0.02·7199.5578 V / (12.47² / 1500 Ω)` = `1389.0 A`, and the stamp is the
/// Kirchhoff sum over the bus.
///
/// That sum's **PC sign** was left open when this pin first landed and was
/// settled by the RP3.13 audit settlement: r4133's `cadd` (`VSource.pas` l.1169,
/// against `csub` for the PD loop at l.1135) is an upstream **bug**, measured on
/// a deck r4133 *can* answer and not reproduced here — see
/// [`ncim_swing_sum_subtracts_pc_terminals_and_closes_kcl`] and the sign-bug
/// paragraph on `ncim_stamp_swing_source_currents`. So the identity asserted
/// below is plain KCL: `I(SOURCE) + I(Line.l1 t1) + I(SRC2) = 0`, so
/// `Vsource.SOURCE` = `-I(Line.l1 t1) - I(SRC2)` prints `1332.03 A ∠-79.45°`
/// (the stiffer swing source absorbs most of what `SRC2` pushes in) where the
/// `cadd` form printed `1450.64 A ∠107.23°`.
#[test]
fn ncim_second_slack_node_vsource_reports_its_own_current() {
    let mut dss = run(&[
        "Clear",
        "New circuit.twosrc basekv=12.47 phases=3 bus1=sourcebus",
        "New Vsource.src2 bus1=sourcebus basekv=12.47 phases=3 pu=1.02 angle=0 \
         MVAsc3=1500 MVAsc1=1200",
        "New Line.l1 bus1=sourcebus bus2=loadbus phases=3 r1=0.12 x1=0.35 length=2",
        "New Load.ld1 bus1=loadbus phases=3 kV=12.47 kW=2000 kvar=800 model=1",
        "Set VoltageBases=[12.47]",
        "CalcVoltageBases",
        "Set algorithm=NCIM",
        "Set maxiterations=50",
        "Solve",
    ]);
    assert!(
        dss.circuit().expect("circuit").is_solved,
        "the two-source NCIM deck must converge"
    );

    let i_src2 = elem_currents(&mut dss, "Vsource.src2");
    // Thevenin: |E2 - V| / |Z1|, with V pinned at the swing EMF (1.0 pu) and
    // `Z1 = kVLL² / MVAsc3` — 143.991 V / 0.103667 Ω.
    let z1 = 12.47 * 12.47 / 1500.0;
    let expect = (0.02 * 12.47e3 / 3.0_f64.sqrt()) / z1;
    for (k, i) in i_src2.iter().take(3).enumerate() {
        // Band: the measured agreement is 8.4e-10 relative, three orders inside
        // this floor; it is the physics band, not a fitted one.
        assert!(
            (i.norm() - expect).abs() < 1e-6 * expect,
            "Vsource.src2 conductor {k}: |I| = {} A, Thevenin |E2-V|/|Z1| = {expect} A \
             (RP3.13 regression: the swing echo printed 0 here)",
            i.norm()
        );
    }
    assert!(
        (cdang(i_src2[0]) - 104.04).abs() < 5e-3,
        "Vsource.src2 |I1| ∠ = {}, measured 104.04°",
        cdang(i_src2[0])
    );
    assert!(
        export_row(&mut dss, "currents", "Vsource.SRC2").starts_with(
            "Vsource.SRC2, 1388.97, 104.04, 1388.97, -15.96, 1388.97, -135.96, 0, 0.00,"
        ),
        "Export Currents Vsource.SRC2 row: {}",
        export_row(&mut dss, "currents", "Vsource.SRC2")
    );

    // The swing source still takes the stamp, and the stamp is the Kirchhoff sum
    // over every element at the bus — `-Σ PD - Σ other PC` (`VSource.pas`
    // l.1135 / l.1169, the PC term with the upstream sign bug fixed). With `SRC2`
    // echoing a stamp it never received this identity read
    // `I(SOURCE) = -I(Line.l1 t1)` instead.
    let i_source = elem_currents(&mut dss, "Vsource.source");
    let i_line = elem_currents(&mut dss, "Line.l1");
    for k in 0..3 {
        let kcl = i_source[k] + i_line[k] + i_src2[k];
        assert!(
            kcl.norm() < 1e-9,
            "KCL at sourcebus conductor {k}: I(SOURCE) {:?} + I(Line.l1 t1) {:?} \
             + I(SRC2) {:?} = {kcl:?} A (must be 0)",
            i_source[k],
            i_line[k],
            i_src2[k]
        );
    }
    assert!(
        i_source[0].norm() > 1e3,
        "the swing source must carry the feeder and the second source: {:?}",
        i_source[0]
    );
}

/// **P9** — the swing-bus Kirchhoff sum subtracts PC terminal currents; r4133
/// adds them and violates KCL (RP3.13 audit settlement, 2026-09-03).
///
/// `TVsourceObj.CalcInjCurrAtBus` (r4133 `PCElements/VSource.pas` l.1085)
/// subtracts every PD terminal current at the swing bus (`csub`, l.1135) and
/// **adds** every PC one (`cadd`, l.1169). Every `GetCurrents` in OpenDSS returns
/// the current flowing *into* the element — "Gets total Currents going INTO a
/// device's terminals", for PD and PC alike, which is why a load reports `+P` and
/// a generator `−P` — so KCL at the bus is `I(source) + Σ I(others) = 0` and both
/// loops must subtract. The `cadd` is an upstream **sign bug**, and CLAUDE.md
/// (2026-08-02) forbids reproducing it in any lane.
///
/// The deck below is the measurement: a 1000 kW / 400 kvar `Load.ldswing` bonded
/// straight onto `sourcebus`, which is the only shape where the two engines can
/// differ. Both diverge identically (any PC element on the slack node breaks
/// NCIM's slack constraint — 10 kW, 100 kW and 1000 kW loads and a 500 kW
/// generator all hit the iteration limit on both engines), and at the shared
/// 15-iteration state they agree on every node voltage and on the `Line`/`Load`
/// terminal currents to the digit. Live `epri-worker` (OpenDSSDirect.dll r4133,
/// "Version 11.0.0.1 (64-bit build)"), own probe 2026-09-03:
///
/// ```text
/// converged False   iters 15
/// SOURCEBUS.1  7503.271052270615 - 664.5923616499286j
/// Vsource.source   I1 = -12.910456091137 + 52.625721242876j   (54.1862 A ∠103.78°)
/// Line.l1          I1 =  55.428005336360 - 74.161684780923j
/// Load.ldswing     I1 =  42.517549245224 - 21.535963538047j
/// ```
///
/// r4133's swing row is exactly `−I(Line.l1 t1) + I(Load.ldswing)`, leaving a KCL
/// residual of `85.035098 − 43.071927j A` = precisely `2·I(Load.ldswing)`. This
/// port subtracts both loops, so the same read is `−97.945554 + 95.697649j A`
/// (`136.9357 A ∠135.669°`) and KCL closes to `< 1e-9 A`.
///
/// **Zero oracle exposure**: the divergence needs a PC element other than the
/// source on the slack node, and no gated NCIM case has one —
/// [`ncim_swing_bus_carries_no_pc_element_on_the_gated_decks`] reds if that ever
/// changes. Upstream report:
/// `investigations/to_opendss/54-ncim-calcinjcurratbus-pc-sign.md`.
#[test]
fn ncim_swing_sum_subtracts_pc_terminals_and_closes_kcl() {
    let mut dss = run(&[
        "Clear",
        "New circuit.swingpc basekv=12.47 phases=3 bus1=sourcebus",
        "New Line.l1 bus1=sourcebus bus2=loadbus phases=3 r1=0.12 x1=0.35 length=3",
        "New Load.ld1 bus1=loadbus phases=3 kv=12.47 kw=2000 kvar=800 model=1",
        "New Load.ldswing bus1=sourcebus phases=3 kv=12.47 kw=1000 kvar=400 model=1",
        "Set VoltageBases=[12.47]",
        "CalcVoltageBases",
        "Set algorithm=NCIM",
        "Solve",
    ]);

    // The shared non-converged state, live r4133 `YNodeVarray` (same probe).
    {
        let ckt = dss.circuit().expect("circuit");
        assert!(
            !ckt.is_solved,
            "a PC element on the slack node breaks NCIM on both engines \
             (r4133: converged=False)"
        );
        assert_eq!(
            ckt.solution.iteration, 15,
            "both engines stop at the iteration limit (r4133: 15)"
        );
        const R4133_NODE_V: [(f64, f64); 6] = [
            (7503.271052270615, -664.5923616499286),
            (-3102.837842233346, -5615.634473190251),
            (-4310.1202905672635, 6366.070841489593),
            (7404.628403125423, -694.0336321632785),
            (-3179.66732683407, -5538.888611878764),
            (-4160.265214596268, 6344.860363531688),
        ];
        for (k, &(re, im)) in R4133_NODE_V.iter().enumerate() {
            let v = ckt.solution.node_v[k + 1];
            assert!(
                (v.re - re).abs() < 1e-6 && (v.im - im).abs() < 1e-6,
                "node {}: {v:?} V vs live r4133 ({re}, {im})",
                k + 1
            );
        }
    }

    // The two terms of the sum, both digit-identical to r4133 (same probe).
    let i_line = elem_currents(&mut dss, "Line.l1");
    let i_load = elem_currents(&mut dss, "Load.ldswing");
    const R4133_LINE_T1: [(f64, f64); 3] = [
        (55.42800533636046, -74.16168478092283),
        (-41.42898607201852, -86.61900928944644),
        (-24.206901069953346, 135.18134035947878),
    ];
    const R4133_LDSWING: [(f64, f64); 3] = [
        (42.517549245223506, -21.53596353804701),
        (-37.842633727895155, -30.94779217302181),
        (-10.287532697054237, 47.1892855853643),
    ];
    for k in 0..3 {
        let (re, im) = R4133_LINE_T1[k];
        assert!(
            (i_line[k].re - re).abs() < 1e-6 && (i_line[k].im - im).abs() < 1e-6,
            "Line.l1 t1 conductor {k}: {:?} A vs live r4133 ({re}, {im})",
            i_line[k]
        );
        let (re, im) = R4133_LDSWING[k];
        assert!(
            (i_load[k].re - re).abs() < 1e-6 && (i_load[k].im - im).abs() < 1e-6,
            "Load.ldswing conductor {k}: {:?} A vs live r4133 ({re}, {im})",
            i_load[k]
        );
    }

    // The swing row: KCL closes here, and the r4133 row is the `cadd` form.
    let i_source = elem_currents(&mut dss, "Vsource.source");
    for k in 0..3 {
        let kcl = i_source[k] + i_line[k] + i_load[k];
        assert!(
            kcl.norm() < 1e-9,
            "KCL at sourcebus conductor {k}: I(SOURCE) {:?} + I(Line.l1 t1) {:?} \
             + I(Load.ldswing) {:?} = {kcl:?} A (must be 0)",
            i_source[k],
            i_line[k],
            i_load[k]
        );
        // r4133's own row on this deck, and the residual it leaves.
        let r4133 = -i_line[k] + i_load[k];
        let residual = r4133 + i_line[k] + i_load[k];
        assert!(
            (residual - 2.0 * i_load[k]).norm() < 1e-9,
            "conductor {k}: r4133's `cadd` row {r4133:?} leaves {residual:?} A, \
             expected exactly 2·I(Load.ldswing) = {:?}",
            2.0 * i_load[k]
        );
    }
    // Conductor 1, both engines' numbers spelled out.
    assert!(
        (i_source[0] - Complex64::new(-97.945554, 95.697649)).norm() < 1e-5,
        "Vsource.source conductor 0: this port {:?} A (KCL), live r4133 \
         (-12.910456091137, 52.625721242876) A (`cadd`, KCL off by 2·I(load))",
        i_source[0]
    );
    let row = export_row(&mut dss, "currents", "Vsource.SOURCE");
    assert!(
        row.starts_with("Vsource.SOURCE, 136.936, 135.67,"),
        "Export Currents Vsource.SOURCE row: {row} (r4133 prints 54.1862, 103.78)"
    );
}

// ---------------------------------------------------------------------------
// RF-D00-05 — a generator that joins the NCIM solve late.
// ---------------------------------------------------------------------------

/// `pv_circuit`'s network without its generator: `sourcebus → genbus` and the
/// 2 MW / 0.8 Mvar load at `genbus`. The generator (if any) is supplied by the
/// caller, *before* the voltage bases as `pv_circuit` places it.
fn late_gen_network(generator: Option<String>) -> Vec<String> {
    late_gen_deck(generator.as_slice())
}

/// [`late_gen_network`] with any number of `extra` element definitions.
fn late_gen_deck(extra: &[String]) -> Vec<String> {
    let mut deck: Vec<String> = vec![
        "Clear".into(),
        "New circuit.ncimlate basekv=12.47 phases=3 bus1=sourcebus".into(),
        "New Line.l1 bus1=sourcebus bus2=genbus phases=3 r1=0.12 x1=0.35 length=3".into(),
        "New Load.ld1 bus1=genbus phases=3 kv=12.47 kw=2000 kvar=800 model=1".into(),
    ];
    deck.extend_from_slice(extra);
    deck.push("Set voltagebases=[12.47]".into());
    deck.push("Calcvoltagebases".into());
    deck
}

/// `pv_circuit("1.0")`'s machine with the model under test (±1500 kvar limits).
fn late_gen_line(model: i32, extra: &str) -> String {
    format!(
        "New Generator.g1 bus1=genbus phases=3 kv=12.47 kw=800 model={model} \
         maxkvar=1500 minkvar=-1500 vpu=1.0 {extra}"
    )
}

/// Solve `first` under NCIM (it must converge), run `between`, `Solve` again.
fn ncim_resolve_after(first: &[String], between: &[String]) -> Dss {
    let mut dss = solve_ncim(first);
    assert!(
        dss.circuit().expect("circuit").is_solved,
        "the first NCIM solve must converge"
    );
    for line in between.iter().map(String::as_str).chain(["Solve"]) {
        dss.command(line);
        assert!(dss.errors().is_empty(), "`{line}` -> {:?}", dss.errors());
    }
    dss
}

/// Assert a solved circuit against live-r4133 node voltages and the
/// `Generator.g1` terminal power (kW/kvar), with its iteration count.
fn assert_twin_matches_r4133(
    twin: &mut Dss,
    iterations: i32,
    nodes: &[(f64, f64); 6],
    gen_kw_kvar: (f64, f64),
) {
    let ckt = twin.circuit().expect("twin");
    assert!(ckt.is_solved, "r4133 converges the twin");
    assert_eq!(ckt.solution.iteration, iterations, "r4133 iteration count");
    let expected: Vec<Complex64> = nodes.iter().map(|&(re, im)| cx(re, im)).collect();
    assert_nodes(twin, &expected, V_TOL);
    let s = term_power_kw(twin, "Generator.g1", 1);
    assert!(
        (s.re - gen_kw_kvar.0).abs() < 1e-6 && (s.im - gen_kw_kvar.1).abs() < 1e-6,
        "twin Generator.g1 = {s:?} kW/kvar vs live r4133 {gen_kw_kvar:?}"
    );
    // RP3.13's per-phase sizing on the first-solve path (RF-D00-05 settlement,
    // audit AT-3): every registry access is checked now, so r4133's length-1
    // seed would silently drop the PQ->PV promotion's phase-1..n writes.
    let (_, _, _, _, dq, _) = gen_ncim_state(twin, "g1");
    assert_eq!(dq.len(), 3, "g1's deltaQNom is sized per phase: {dq:?}");
}

/// The shared verdict of the late-generator pins: the re-solve that brought
/// `Generator.g1` in converges, in `iterations` Newton passes, onto the
/// fixpoint of its *twin* — the same deck with the machine declared before the
/// first solve, re-solved the same way — at the `V_TOL` node floor and the
/// 1e-6 kW/kvar terminal-power floor; Kirchhoff closes at `genbus`; the
/// machine keeps `model` and its NCIM registry is per-phase.
///
/// r4133 cannot answer a late deck itself: live `epri-worker` probe
/// 2026-09-25 (OpenDSSDirect.dll r4133 "Version 11.0.0.1 (64-bit build)";
/// re-derive by sending the pins' decks through `epri-worker`) — the second
/// `Solve` raises `DSS error #482 Error Encountered in Solve: Access violation
/// … Read of address 0000000000000000` at iteration 1 on all four decks
/// (created / enabled × model 3 / 4): the nil read of the late machine's empty
/// `deltaQNom` — `GetNCIMPowers`' model-3 arm (`Common/Solution.pas` l.1308,
/// before any voltage update) or `UpdateGenQ`'s ELSE-arm PQ→PV test (l.2209).
/// So each pin is Kirchhoff + port self-consistency against a twin that r4133
/// *does* answer, and pins that twin to r4133's own numbers.
fn assert_late_generator_lands_on_the_twin(
    late: &mut Dss,
    twin: &mut Dss,
    model: i32,
    iterations: i32,
) {
    let ckt = late.circuit().expect("circuit");
    assert!(
        ckt.is_solved,
        "the re-solve with the late generator must converge"
    );
    assert_eq!(
        ckt.solution.iteration, iterations,
        "Newton passes of the late re-solve (port-only number: r4133 raises #482)"
    );
    let twin_v = twin.circuit().expect("twin").solution.node_v.clone();
    assert_eq!(ckt.solution.node_v.len(), twin_v.len(), "same node space");
    for (k, &t) in twin_v.iter().enumerate().skip(1) {
        let v = ckt.solution.node_v[k];
        assert!(
            (v - t).norm() < V_TOL,
            "node {k} ({}): late {v:?} vs twin {t:?}",
            ckt.node_name(k)
        );
    }
    let s = term_power_kw(late, "Generator.g1", 1);
    let s_twin = term_power_kw(twin, "Generator.g1", 1);
    assert!(
        (s - s_twin).norm() < 1e-6,
        "Generator.g1 = {s:?} kW/kvar vs twin {s_twin:?}"
    );
    let kcl = s + term_power_kw(late, "Load.ld1", 1) + term_power_kw(late, "Line.l1", 2);
    assert!(kcl.norm() < 1e-6, "KCL at genbus = {kcl:?} kW/kvar");
    let (m, _, _, _, dq, _) = gen_ncim_state(late, "g1");
    assert_eq!(m, model, "the machine keeps its model");
    assert_eq!(dq.len(), 3, "deltaQNom is sized per phase: {dq:?}");
}

/// r4133's straight up-front `model=4` twin — `late_gen_network` with
/// `late_gen_line(4, "")`, one NCIM solve: converged, 3 iterations, these node
/// voltages, `Generator.g1` at -266.6666… kW and -143.93141923682322 kvar per
/// phase (live `epri-worker` probe 2026-09-25 on the r4133 DLL; this deck
/// through `epri-worker` re-derives it).
const R4133_M4_TWIN: [(f64, f64); 6] = [
    (7199.557856794634, 0.0),
    (-3599.77892839732, -6234.999999999999),
    (-3599.778928397315, 6235.000000000001),
    (7161.086907692252, -52.20470684983792),
    (-3625.7540561752085, -6175.580827344718),
    (-3535.3328515170447, 6227.785534194561),
];
/// [`R4133_M4_TWIN`]'s `Generator.g1` terminal kvar (three phases).
const R4133_M4_TWIN_KVAR: f64 = -3.0 * 143.93141923682322;

/// **P10** (RF-D00-05) — a `model=3` (PV) generator *created* after the first
/// NCIM solve must solve, not panic. `ncim_init_gen_q` (r4133 `InitGenQ`) is a
/// one-shot flag — set only in `TSolutionObj.Create` (`Common/Solution.pas`
/// l.648), cleared after the first Newton pass (l.1157) — and both registry
/// initialisers were gated on it (`InitPQGen` l.1115-1118, `GetNumGenerators`'
/// `InitQ` l.1926-1931), so the late machine reached `UpdateGenQ` with an
/// empty `deltaQNom` and the port aborted: "index out of bounds: the len is 0
/// but the index is 0" (its PV-arm read of `deltaQNom[j]`, r4133 l.2087).
///
/// The twin is re-solved after `Init`, not straight: `model=3` in the `New`
/// clears `SolutionInitialized` (r4133 `PCElements/generator.pas` l.702), so
/// the late re-solve re-initialises first (`DoPFLOWsolution` l.2445-2461:
/// `SolveYDirect`, then `SetGeneratordQdV`'s `SolveZeroLoadSnapShot`, l.1012)
/// and that leaves the swing bus 4.649e-5 V off the ideal EMF — which NCIM
/// then holds (identity swing rows, zero swing mismatch). `Init`
/// (`Executive/ExecCommands.pas` l.797) is the same reset, so the twin shares
/// it; against the straight twin `pv_circuit("1.0")` the late fixpoint sits
/// that 4.649e-5 V away. r4133 answers the `Init` twin (same probe):
/// converged, 3 iterations, the numbers below — and its aborted late deck
/// leaves this very swing value (7199.55790241085 − 8.971243507103153e-6j).
#[test]
fn ncim_generator_added_after_first_solve_does_not_panic_model3() {
    let mut late = ncim_resolve_after(&late_gen_network(None), &[late_gen_line(3, "")]);
    let mut twin = ncim_resolve_after(
        &late_gen_network(Some(late_gen_line(3, ""))),
        &["Init".to_string()],
    );
    const R4133_INIT_TWIN: [(f64, f64); 6] = [
        (7199.55790241085, -8.971243507103153e-6),
        (-3599.7789596772545, -6235.000034613589),
        (-3599.7789420310905, 6235.000044801607),
        (7199.261751488612, -65.29599441361756),
        (-3656.1788663694515, -6202.09556766475),
        (-3543.0828844045404, 6267.391563287948),
    ];
    // r4133 `Generator.g1` powers per phase: -266.6666… kW and -405.7398640018828,
    // -405.73986400298423, -405.73986401115616 kvar.
    assert_twin_matches_r4133(
        &mut twin,
        3,
        &R4133_INIT_TWIN,
        (-800.0, -1217.2195920160232),
    );
    assert_late_generator_lands_on_the_twin(&mut late, &mut twin, 3, 3);
    // The PV equation holds at the late machine's bus: |V| = VTarget.
    let vtarget = 12.47e3 / 3f64.sqrt();
    let ckt = late.circuit().expect("circuit");
    for k in 4..=6 {
        let vmag = ckt.solution.node_v[k].norm();
        assert!(
            (vmag - vtarget).abs() < V_TOL,
            "|genbus node {k}| = {vmag} V vs VTarget {vtarget} V"
        );
    }
}

/// **P11** (RF-D00-05) — the `model=4` (PQ) arm of **P10**: the late machine's
/// empty `deltaQNom` panicked in `UpdateGenQ`'s ELSE arm instead (the PQ→PV
/// test's `deltaQNom[0]` read, r4133 l.2209). `model=4` does not clear
/// `SolutionInitialized`, so the twin is the straight up-front deck, which
/// r4133 answers (same probe): converged, 3 iterations, the numbers below; the
/// machine stays PQ at its nominal `Qnominalperphase` (800 kW at the default
/// `PF=0.88`) — the value the port seeds the late registry with, and the one
/// r4133's own empty-registry read falls back to (l.1323-1324).
#[test]
fn ncim_generator_added_after_first_solve_does_not_panic_model4() {
    let mut late = ncim_resolve_after(&late_gen_network(None), &[late_gen_line(4, "")]);
    let mut twin = solve_ncim(&late_gen_network(Some(late_gen_line(4, ""))));
    assert_twin_matches_r4133(&mut twin, 3, &R4133_M4_TWIN, (-800.0, R4133_M4_TWIN_KVAR));
    assert_late_generator_lands_on_the_twin(&mut late, &mut twin, 4, 3);
}

/// **P12** (RF-D00-05) — the same defect reached without creating anything: a
/// machine declared `enabled=no`, enabled between two NCIM solves, `model=3`
/// and `model=4`. `InitPQGen` and `GetNumGenerators` both skip disabled
/// machines (r4133 l.1676, l.1919), so it too entered `UpdateGenQ` with an
/// empty `deltaQNom` and panicked — at the PV arm's read (r4133 l.2087) for
/// `model=3`, at the ELSE arm's PQ→PV test (l.2209) for `model=4`; r4133
/// raises #482 on both (the probe of [`assert_late_generator_lands_on_the_twin`],
/// cases `enable_m3` / `enable_m4`). Enabling does not clear
/// `SolutionInitialized`, so each twin is the straight up-front deck: the
/// `model=3` one r4133/capi015-pinned by [`ncim_pv_regulating_matches_oracle`],
/// the `model=4` one r4133-pinned below ([`R4133_M4_TWIN`]).
#[test]
fn ncim_generator_enabled_between_solves_does_not_panic() {
    for model in [3, 4] {
        let mut late = ncim_resolve_after(
            &late_gen_network(Some(late_gen_line(model, "enabled=no"))),
            &["Generator.g1.enabled=yes".to_string()],
        );
        let mut twin = solve_ncim(&late_gen_network(Some(late_gen_line(model, ""))));
        if model == 4 {
            assert_twin_matches_r4133(&mut twin, 3, &R4133_M4_TWIN, (-800.0, R4133_M4_TWIN_KVAR));
        }
        assert_late_generator_lands_on_the_twin(&mut late, &mut twin, model, 3);
    }
}

/// **P13** (RF-D00-05) — a late `model=3` machine on a bus that did not exist
/// at the first NCIM solve (a new line's far end). Beside the empty registry,
/// the per-node NCIM vectors were one bus short: r4133 rebuilds them only on
/// `SystemYChanged or not NCIMRdy` (`Common/Solution.pas` l.1123), and
/// `SolveCircuit`'s `WHOLEMATRIX` rebuild (l.2805-2808) has already cleared
/// `SystemYChanged` (`Common/Ymatrix.pas` l.271). The port panicked ("index
/// out of bounds: the len is 7 but the index is 7", `ncim_get_num_generators`'
/// per-node Q-limit tally); r4133 raises #482 on this very deck (settlement
/// probe `late_m3_newbus`, 2026-09-26, offset 87EDD4) and on its `model=4`
/// variant, with the new bus's `NodeV` uninitialised (`5e-322`). The port now
/// re-initialises the NCIM node space when it no longer spans the circuit:
/// the re-solve converges, the PV equation holds at the new bus and Kirchhoff
/// closes at both buses (port-only numbers: r4133 cannot answer). The new
/// bus starts from a real voltage only because `model=3` in the `New` clears
/// `SolutionInitialized` (generator.pas l.702), so `SolveYDirect` seeds it
/// before NCIM runs; a `model=4` machine or a load there starts at 0 V and
/// does not converge (an open item of the RF-D00-05 settlement).
#[test]
fn ncim_generator_on_a_new_bus_after_first_solve_does_not_panic() {
    let mut late = ncim_resolve_after(
        &late_gen_network(None),
        &[
            "New Line.l2 bus1=genbus bus2=farbus phases=3 r1=0.12 x1=0.35 length=1".to_string(),
            late_gen_line(3, "").replace("bus1=genbus", "bus1=farbus"),
        ],
    );
    let ckt = late.circuit().expect("circuit");
    assert!(ckt.is_solved, "the re-solve with the new bus must converge");
    assert_eq!(
        ckt.solution.iteration, 3,
        "Newton passes (port-only number)"
    );
    let vtarget = 12.47e3 / 3f64.sqrt();
    let far: Vec<usize> = (1..=ckt.num_nodes)
        .filter(|&k| ckt.node_name(k).to_ascii_lowercase().starts_with("farbus."))
        .collect();
    assert_eq!(far.len(), 3, "farbus carries three nodes");
    for k in far {
        let vmag = ckt.solution.node_v[k].norm();
        assert!(
            (vmag - vtarget).abs() < V_TOL,
            "|{}| = {vmag} V vs VTarget {vtarget} V",
            ckt.node_name(k)
        );
    }
    let kcl_far =
        term_power_kw(&mut late, "Generator.g1", 1) + term_power_kw(&mut late, "Line.l2", 2);
    assert!(kcl_far.norm() < 1e-6, "KCL at farbus = {kcl_far:?} kW/kvar");
    let kcl_gen = term_power_kw(&mut late, "Load.ld1", 1)
        + term_power_kw(&mut late, "Line.l1", 2)
        + term_power_kw(&mut late, "Line.l2", 1);
    assert!(kcl_gen.norm() < 1e-6, "KCL at genbus = {kcl_gen:?} kW/kvar");
    let (m, _, _, _, dq, _) = gen_ncim_state(&late, "g1");
    assert_eq!(m, 3, "the machine keeps regulating");
    assert_eq!(dq.len(), 3, "deltaQNom is sized per phase: {dq:?}");
}

/// r4133 on the generator-free network (`late_gen_network(None)`) solved once
/// under NCIM: converged, 3 iterations, these node voltages (live
/// `epri-worker` probe 2026-09-25 on the r4133 DLL, case `nogen`, re-derived
/// by sending this deck through `epri-worker`; the `EnergyMeter`-carrying
/// variant `nogen_meter` answers bit-identically).
const R4133_NO_GEN: [(f64, f64); 6] = [
    (7199.557856794634, 0.0),
    (-3599.77892839732, -6234.999999999999),
    (-3599.778928397315, 6235.000000000001),
    (7125.608167144498, -83.89892276748104),
    (-3635.4626820390386, -6129.008228777266),
    (-3490.1454851054596, 6212.907151544748),
];

/// r4133's answer to the disable → re-enable sequence of the `model=3`
/// machine (same probe, case `reenable_m3`): converged, 3 iterations, back on
/// the straight up-front fixpoint (1.0e-12 V from r4133's `upfront_m3`), with
/// `Generator.g1` at -405.7401777716064, -405.74017777161515 and
/// -405.7401777716107 kvar per phase.
const R4133_M3_REENABLED: [(f64, f64); 6] = [
    (7199.557856794634, 0.0),
    (-3599.77892839732, -6234.999999999999),
    (-3599.778928397315, 6235.000000000001),
    (7199.26175142393, -65.2960015452256),
    (-3656.1788718156818, -6202.095564454159),
    (-3543.0828796082496, 6267.391565999388),
];

/// Kirchhoff at `genbus` in kW/kvar: the generator, the load and `Line.l1`'s
/// receiving terminal (a disabled element reports zero power).
fn genbus_kcl(dss: &mut Dss) -> Complex64 {
    term_power_kw(dss, "Generator.g1", 1)
        + term_power_kw(dss, "Load.ld1", 1)
        + term_power_kw(dss, "Line.l1", 2)
}

/// **P14** (RF-D00-05) — the reverse direction: a machine that took part in
/// the first NCIM solve is disabled before the next, `model=3` and `model=4`.
/// r4133 answers this itself (cases `disable_m3` / `disable_m4` of the
/// [`R4133_NO_GEN`] probe): converged in 3 iterations onto the generator-free
/// fixpoint (≤ 1.64e-12 V from its fresh solve), `Generator.g1` reporting
/// zero power. Every NCIM pass skips a disabled machine (r4133
/// `Common/Solution.pas`: `InitPQGen` l.1676, `GetNumGenerators` l.1919), so
/// its registry is neither read nor written: it stays exactly as the last
/// solve left it — the state
/// [P15](ncim_generator_reenabled_between_solves_resumes_from_its_kept_q)
/// resumes from. No pre-fix panic (the registry exists): a regression pin for
/// "a disabled machine's registry is never touched" (the enabled machines'
/// kept Q is [P22](ncim_converted_machine_resolves_warm_from_its_clamped_q)'s).
#[test]
fn ncim_generator_disabled_between_solves_resolves_without_it() {
    for model in [3, 4] {
        let deck = late_gen_network(Some(late_gen_line(model, "")));
        let (_, _, _, _, dq_first, _) = gen_ncim_state(&solve_ncim(&deck), "g1");
        let mut off = ncim_resolve_after(&deck, &["Generator.g1.enabled=no".to_string()]);
        assert_twin_matches_r4133(&mut off, 3, &R4133_NO_GEN, (0.0, 0.0));
        let kcl = genbus_kcl(&mut off);
        assert!(
            kcl.norm() < 1e-6,
            "model {model}: KCL at genbus = {kcl:?} kW/kvar"
        );
        let (m, _, _, _, dq, _) = gen_ncim_state(&off, "g1");
        assert_eq!(m, model, "the disabled machine keeps its model");
        assert_eq!(
            dq, dq_first,
            "model {model}: the disabled machine's registry stays as the first solve left it"
        );
    }
}

/// **P15** (RF-D00-05) — disable, re-solve, re-enable, re-solve (`model=3` and
/// `model=4`): the machine rejoins with the registry it left with (P14), and
/// both engines warm-start from it. r4133 answers (cases `reenable_m3` /
/// `reenable_m4` of the [`R4133_NO_GEN`] probe): converged in 3 iterations,
/// back on the straight up-front fixpoint — [`R4133_M3_REENABLED`] and
/// [`R4133_M4_TWIN`] (1.0e-12 V from its `reenable_m4` answer) — with the
/// kvar below. No pre-fix panic (the registry exists). This deck cannot tell
/// a kept registry from a re-seeded one (a PV machine's Q comes back in one
/// Newton step); [P22](ncim_converted_machine_resolves_warm_from_its_clamped_q)
/// pins the kept-Q rule.
#[test]
fn ncim_generator_reenabled_between_solves_resumes_from_its_kept_q() {
    for (model, nodes, kvar) in [
        (3, &R4133_M3_REENABLED, -1217.2205333148322),
        (4, &R4133_M4_TWIN, R4133_M4_TWIN_KVAR),
    ] {
        let mut back = ncim_resolve_after(
            &late_gen_network(Some(late_gen_line(model, ""))),
            &[
                "Generator.g1.enabled=no".to_string(),
                "Solve".to_string(),
                "Generator.g1.enabled=yes".to_string(),
            ],
        );
        assert_twin_matches_r4133(&mut back, 3, nodes, (-800.0, kvar));
        let kcl = genbus_kcl(&mut back);
        assert!(
            kcl.norm() < 1e-6,
            "model {model}: KCL at genbus = {kcl:?} kW/kvar"
        );
        let (m, _, _, _, dq, _) = gen_ncim_state(&back, "g1");
        assert_eq!(m, model, "the machine keeps its model");
        assert_eq!(dq.len(), 3, "deltaQNom is sized per phase: {dq:?}");
    }
}

/// **P16** (RF-D00-05) — a machine *removed* between two NCIM solves.
/// `Remove` takes PD elements only (r4133 `Executive/ExecHelper.pas`
/// l.4575-4591, error 28728 otherwise), so the machine goes with its branch:
/// `Remove ElementName=Line.l2 KeepLoad=no` disables `Line.l2` and every shunt
/// element below it — here the generator on `farbus` (`DoRemoveBranches`,
/// `Meters/ReduceAlgs.pas` l.372-451) — and `farbus` leaves the node space
/// (9 → 6 nodes). The port re-initialises the NCIM node space that no longer
/// spans the circuit (`do_ncim_solution`) and converges onto the fresh
/// generator-free deck, which r4133 answers ([`R4133_NO_GEN`], case
/// `nogen_meter`). r4133 cannot answer the sequence itself (cases `remove_m3`
/// / `remove_m4`): it keeps the stale NCIM structures (the `SystemYChanged`
/// test of [P13](ncim_generator_on_a_new_bus_after_first_solve_does_not_panic))
/// and ends **not converged after 15 iterations** with `genbus.1` at
/// 7125.617781720513 − 83.90217748893535j V, 1.015e-2 V off its own fresh
/// solve — never adopted. The iteration count is a port-only number. Before
/// the node-space re-init the port panicked here too: the stale 9-node NCIM Y
/// triplets met the 6-node `NodeV` in `ncim_calc_inj_curr`'s `Y·V` product
/// ("index out of bounds: the len is 7 but the index is 7").
#[test]
fn ncim_generator_removed_with_its_branch_resolves_on_the_fresh_deck() {
    for model in [3, 4] {
        let far = [
            "New Line.l2 bus1=genbus bus2=farbus phases=3 r1=0.12 x1=0.35 length=1".to_string(),
            late_gen_line(model, "").replace("bus1=genbus", "bus1=farbus"),
            "New EnergyMeter.m1 element=Line.l1 terminal=1".to_string(),
        ];
        let mut gone = ncim_resolve_after(
            &late_gen_deck(&far),
            &["Remove ElementName=Line.l2 KeepLoad=no".to_string()],
        );
        let ckt = gone.circuit().expect("circuit");
        assert!(ckt.is_solved, "model {model}: the re-solve must converge");
        assert_eq!(
            ckt.solution.iteration, 3,
            "model {model}: Newton passes (port-only number: r4133 does not converge)"
        );
        let expected: Vec<Complex64> = R4133_NO_GEN.iter().map(|&(re, im)| cx(re, im)).collect();
        assert_nodes(&gone, &expected, V_TOL);
        let kcl = genbus_kcl(&mut gone) + term_power_kw(&mut gone, "Line.l2", 1);
        assert!(
            kcl.norm() < 1e-6,
            "model {model}: KCL at genbus = {kcl:?} kW/kvar"
        );
    }
}

/// **P17** (RF-D00-05) — a `model=3` machine re-phased 1 → 3 between two NCIM
/// solves (`phases=3 bus1=genbus kv=12.47 …`, `model` untouched): the first
/// solve sized its registry for one phase. r4133 reads and writes past its
/// end — the second `Solve` answers `DSS error #482 … Invalid pointer
/// operation` (case `ph1to3_m3` of the [`R4133_NO_GEN`] probe) — and the port
/// panicked at the PV arm's read ("index out of bounds: the len is 1 but the
/// index is 1", r4133 l.2087). The self-sizing registry grows the short
/// registry with the `InitQ` value `0.0` (`ncim_get_num_generators`); the edit
/// does not clear `SolutionInitialized`, so the twin is the straight 3-phase
/// deck (r4133/capi015-pinned by [`ncim_pv_regulating_matches_oracle`]).
#[test]
fn ncim_generator_rephased_between_solves_does_not_panic() {
    let one_phase = "New Generator.g1 bus1=genbus.1 phases=1 kv=7.2 kw=266.6666666666667 \
                     model=3 maxkvar=500 minkvar=-500 vpu=1.0";
    let mut late = ncim_resolve_after(
        &late_gen_deck(&[one_phase.to_string()]),
        &[
            "Generator.g1.phases=3 bus1=genbus kv=12.47 kw=800 maxkvar=1500 minkvar=-1500"
                .to_string(),
        ],
    );
    let mut twin = solve_ncim(&late_gen_network(Some(late_gen_line(3, ""))));
    assert_late_generator_lands_on_the_twin(&mut late, &mut twin, 3, 3);
}

/// **P18** (RF-D00-05 settlement, audit AC-1) — the model-4 arm of
/// [P17](ncim_generator_rephased_between_solves_does_not_panic): a 1-phase
/// `model=4` machine re-phased to 3 phases between two NCIM solves. r4133
/// answers this deck (live `epri-worker` probe of the RF-D00-05 audit, case
/// `ph1to3_m4`): converged in 3 iterations onto its straight up-front twin,
/// [`R4133_M4_TWIN`] (the late machine's grown registry reads the 1-phase
/// seed, which equals the 3-phase `Qnominalperphase` on this deck).
#[test]
fn ncim_generator_rephased_between_solves_model4_matches_r4133() {
    let one_phase = "New Generator.g1 bus1=genbus.1 phases=1 kv=7.2 kw=266.6666666666667 \
                     model=4 maxkvar=500 minkvar=-500 vpu=1.0";
    let mut late = ncim_resolve_after(
        &late_gen_deck(&[one_phase.to_string()]),
        &[
            "Generator.g1.phases=3 bus1=genbus kv=12.47 kw=800 maxkvar=1500 minkvar=-1500"
                .to_string(),
        ],
    );
    assert_twin_matches_r4133(&mut late, 3, &R4133_M4_TWIN, (-800.0, R4133_M4_TWIN_KVAR));
}

/// **P19** (RF-D00-05 settlement, audit AT-6) — a `model=3` machine with zero
/// Q-limits created after the first NCIM solve is demoted to PQ on its own
/// initialising pass and logged as converted (`ncim_expv`, read by
/// `Show PV2PQ`), exactly as on a first solve (r4133 l.1936-1939 logs it under
/// `InitQ`; r4133 itself cannot answer the late deck: it reads the late
/// machine's empty registry, l.2305). It injects no Q (registry `[0.0; 3]`).
#[test]
fn ncim_late_zero_limit_pv_machine_is_logged_as_converted() {
    let mut late = ncim_resolve_after(
        &late_gen_network(None),
        &[
            "New Generator.g1 bus1=genbus phases=3 kv=12.47 kw=800 model=3 \
           maxkvar=0 minkvar=0 vpu=1.0"
                .to_string(),
        ],
    );
    assert!(late.circuit().expect("circuit").is_solved, "must converge");
    let (model, expv, _, _, dq, _) = gen_ncim_state(&late, "g1");
    assert_eq!(model, 4, "demoted to PQ");
    assert!(
        expv,
        "the late machine's own initialising pass logs the conversion"
    );
    assert_eq!(dq, vec![0.0; 3], "no Q for a machine without Q-limits");
    let kcl = genbus_kcl(&mut late);
    assert!(kcl.norm() < 1e-6, "KCL at genbus = {kcl:?} kW/kvar");
}

/// The node index of `genbus.<phase>`.
fn genbus_node(dss: &Dss, phase: usize) -> usize {
    let ckt = dss.circuit().expect("circuit");
    let name = format!("genbus.{phase}");
    (1..ckt.solution.node_v.len())
        .find(|&k| ckt.node_name(k).eq_ignore_ascii_case(&name))
        .unwrap_or_else(|| panic!("no node {name}"))
}

/// Converged; Kirchhoff closes at `genbus` over `gens` + `Load.ld1` +
/// `Line.l1`'s far terminal; every `regulated` GENBUS phase sits at the
/// 12.47 kV / sqrt(3) PV target of these decks.
fn assert_genbus_pv_regulated(dss: &mut Dss, gens: &[&str], regulated: &[usize]) {
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(
        dss.circuit().expect("circuit").is_solved,
        "NCIM must converge"
    );
    let target = 12_470.0 / 3f64.sqrt();
    for &ph in regulated {
        let k = genbus_node(dss, ph);
        let v = dss.circuit().expect("circuit").solution.node_v[k].norm();
        assert!(
            (v - target).abs() < V_TOL,
            "|V(genbus.{ph})| = {v} V, PV target {target} V"
        );
    }
    let mut kcl = term_power_kw(dss, "Load.ld1", 1) + term_power_kw(dss, "Line.l1", 2);
    for g in gens {
        kcl += term_power_kw(dss, g, 1);
    }
    assert!(kcl.norm() < 1e-6, "KCL at genbus = {kcl:?} kW/kvar");
}

const MIXED_G1: &str = "New Generator.g1 bus1=genbus.1 phases=1 kv=7.2 \
                        kw=266.6666666666667 model=3 maxkvar=500 minkvar=-500 vpu=1.0";
const MIXED_G2: &str = "New Generator.g2 bus1=genbus phases=3 kv=12.47 kw=800 model=3 \
                        maxkvar=1500 minkvar=-1500 vpu=1.0";

/// **P20** (RF-D00-05 settlement, audit AC-1) — two `model=3` machines on one
/// bus whose conductors do not line up (a 1-phase g1 on `genbus.1` first, a
/// 3-phase g2 on `genbus`), declared up-front or g2 created after the first
/// NCIM solve. r4133 reserves g1's one-row block for both and drops g2's
/// phase-2/3 cells. Live `epri-worker` probe 2026-09-26 on the r4133 DLL
/// (re-derive by sending these two decks through `epri-worker`): up-front
/// converged in 3 iterations with GENBUS.2/.3 at 7138.105769118706 /
/// 7137.5977895458145 V,
/// unregulated against the 7199.557856794634 V target; late raises #482 (g2's
/// empty registry). The port asserted in its sparse set on both
/// ("row < self.n"). With one row per PV node every GENBUS node is held at
/// its target, and the late deck lands on the up-front deck re-solved after
/// `Init` — the reset its `model=3` `New` triggers, as in
/// [P10](ncim_generator_added_after_first_solve_does_not_panic_model3)
/// (port-only numbers: r4133 answers neither correctly).
#[test]
fn ncim_mixed_phase_pv_machines_on_one_bus_regulate_every_node() {
    let gens = ["Generator.g1", "Generator.g2"];
    let mut upfront = solve_ncim(&late_gen_deck(&[
        MIXED_G1.to_string(),
        MIXED_G2.to_string(),
    ]));
    assert_genbus_pv_regulated(&mut upfront, &gens, &[1, 2, 3]);
    let mut late = ncim_resolve_after(
        &late_gen_deck(&[MIXED_G1.to_string()]),
        &[MIXED_G2.to_string()],
    );
    assert_genbus_pv_regulated(&mut late, &gens, &[1, 2, 3]);
    let mut twin = ncim_resolve_after(
        &late_gen_deck(&[MIXED_G1.to_string(), MIXED_G2.to_string()]),
        &["Init".to_string()],
    );
    assert_genbus_pv_regulated(&mut twin, &gens, &[1, 2, 3]);
    let a = twin.circuit().expect("circuit").solution.node_v.clone();
    let b = late.circuit().expect("circuit").solution.node_v.clone();
    assert_eq!(a.len(), b.len(), "same node space");
    for k in 1..a.len() {
        assert!(
            (a[k] - b[k]).norm() < V_TOL,
            "node {k}: Init twin {:?} vs late {:?}",
            a[k],
            b[k]
        );
    }
}

/// **P21** (RF-D00-05 settlement, audit AC-1) — a 3-phase `model=3` machine
/// with a conductor tied to ground (`bus1=genbus.1.2.0`). r4133 addresses
/// Jacobian column `2·0 - 1` for it, which KLUSolve drops, leaving an empty
/// regulation row: live `epri-worker` probe 2026-09-26 (case `grounded_D` of
/// the P20 probe) — not converged after 15 iterations, every node still at the
/// flat start. The port overflowed ("attempt to subtract with overflow").
/// Here the grounded conductor has no regulation row and no injection, and the
/// two live phases are held at their target (port-only numbers).
#[test]
fn ncim_pv_machine_with_a_grounded_conductor_regulates_its_live_nodes() {
    let mut dss = solve_ncim(&late_gen_deck(&[
        "New Generator.g1 bus1=genbus.1.2.0 phases=3 kv=12.47 kw=800 model=3 \
         maxkvar=1500 minkvar=-1500 vpu=1.0"
            .to_string(),
    ]));
    assert_genbus_pv_regulated(&mut dss, &["Generator.g1"], &[1, 2]);
}

/// r4133's warm re-solve of `pv_circuit("1.01")` (its second `Solve`, case
/// `qlimit_warm` of the P20 probe): converged in 2 iterations at these node
/// voltages, `Generator.g1` at -800 kW / -1500 kvar.
const R4133_QLIMIT_WARM: [(f64, f64); 6] = [
    (7199.557856794634, 0.0),
    (-3599.77892839732, -6234.999999999999),
    (-3599.778928397315, 6235.000000000001),
    (7212.895598275793, -70.00930104534125),
    (-3667.077632344357, -6211.546172429121),
    (-3545.817965931437, 6281.555473474465),
];

/// **P22** (RF-D00-05 settlement, audit AT-2) — the kept-Q rule's observable:
/// the machine of `pv_circuit("1.01")` (the corpus deck
/// `modes/ncim/ncim_pv_pq.dss`) is converted PV->PQ at its 500 kvar/phase
/// `maxkvar` limit on the first solve, and a warm re-solve resumes from that clamped
/// registry: 2 Newton passes, as in r4133 (live `epri-worker` probe
/// 2026-09-26, case `qlimit_warm` of the P20 probe, re-derived by sending
/// this deck and a second `Solve` through `epri-worker`: 4, then 2
/// iterations), onto r4133's warm node voltages ([`R4133_QLIMIT_WARM`])
/// with the machine still at its -1500 kvar clamp (settlement round 2, SA-5).
/// Re-seeding the registries on every solve takes 5.
#[test]
fn ncim_converted_machine_resolves_warm_from_its_clamped_q() {
    let mut dss = solve_ncim(&pv_circuit("1.01"));
    assert_eq!(
        dss.circuit().expect("circuit").solution.iteration,
        4,
        "r4133: 4"
    );
    let (model, expv, _, _, dq, _) = gen_ncim_state(&dss, "g1");
    assert!(model == 4 && expv, "converted PV->PQ on the first solve");
    assert_eq!(dq, vec![500_000.0; 3], "clamped at the maxkvar limit");
    dss.command("Solve");
    let ckt = dss.circuit().expect("circuit");
    assert!(ckt.is_solved, "the warm re-solve converges");
    assert_eq!(ckt.solution.iteration, 2, "warm re-solve (r4133: 2)");
    let expected: Vec<Complex64> = R4133_QLIMIT_WARM
        .iter()
        .map(|&(re, im)| cx(re, im))
        .collect();
    assert_nodes(&dss, &expected, V_TOL);
    let s = term_power_kw(&mut dss, "Generator.g1", 1);
    assert!(
        (s - cx(-800.0, -1500.0)).norm() < KVAR_TOL,
        "warm Generator.g1 = {s:?} kW/kvar vs r4133 (-800, -1500)"
    );
    let (model, _, _, _, dq, _) = gen_ncim_state(&dss, "g1");
    assert_eq!(model, 4, "still PQ after the warm re-solve");
    assert_eq!(dq, vec![500_000.0; 3], "the clamped registry is kept");
}

/// Per-phase reported kvar of an NCIM machine: `-deltaQNom / 1000`, what
/// r4133's `element_powers` gives per conductor.
fn gen_phase_kvar(dss: &Dss, name: &str) -> Vec<f64> {
    let (_, _, _, _, dq, _) = gen_ncim_state(dss, name);
    dq.iter().map(|q| -q / 1000.0).collect()
}

/// Per-phase kvar against expected values at the `KVAR_TOL` floor.
fn assert_kvar(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: phases {got:?}");
    for (k, (g, w)) in got.iter().zip(want).enumerate() {
        assert!(
            (g - w).abs() < KVAR_TOL,
            "{what} phase {}: {g} kvar vs {w}",
            k + 1
        );
    }
}

/// r4133 on two identical 3-phase `model=3` machines on `genbus` (case
/// `inorder_twin_a` of a live `epri-worker` probe 2026-09-26 on the r4133
/// DLL; re-derive by sending the P23/P24 decks through `epri-worker`):
/// converged in 3 iterations at these node voltages; g1, the first claimant
/// of every row, at these kvar per phase and g2 at 0.
const R4133_PV_PAIR: [(f64, f64); 6] = [
    (7199.557856794634, 0.0),
    (-3599.77892839732, -6234.999999999999),
    (-3599.778928397315, 6235.000000000001),
    (7199.525025039005, -21.742749836542995),
    (-3618.5922862260827, -6224.100191947301),
    (-3580.932738812924, 6245.842941783848),
];
/// [`R4133_PV_PAIR`]'s `Generator.g1` kvar per phase.
const R4133_PV_PAIR_G1_KVAR: [f64; 3] =
    [-312.5064124155081, -312.50641241551676, -312.50641241550346];

/// **P23** (RF-D00-05 settlement round 2, SA-1) — a PV block in a different
/// conductor order: g1 on `genbus`, then g2 on `genbus.2.3.1`, one NCIM solve.
/// r4133 keys g2 at GENBUS.2's row, so GENBUS.1's equation lands past the
/// block and its reserved row stays empty: not converged after 15 iterations,
/// GENBUS.1 at 6989.634976382399 V, one g1 phase at 2.48e17 kvar (case
/// `reordered_a` of the [`R4133_PV_PAIR`] probe; never adopted). Both
/// machines inject the same P per phase whatever their conductor order, so
/// with one row per PV node the reordered deck lands on the in-order pair,
/// which r4133 answers: the in-order deck is r4133's answer row for row
/// ([`R4133_PV_PAIR`], 3 iterations), and the reordered one lands on it.
#[test]
fn ncim_reordered_pv_block_lands_on_the_in_order_pair() {
    let reordered = MIXED_G2.replace("bus1=genbus ", "bus1=genbus.2.3.1 ");
    for (g2, what) in [(MIXED_G2.to_string(), "in-order"), (reordered, "reordered")] {
        let mut dss = solve_ncim(&late_gen_deck(&[late_gen_line(3, ""), g2]));
        assert_genbus_pv_regulated(&mut dss, &["Generator.g1", "Generator.g2"], &[1, 2, 3]);
        assert_eq!(
            dss.circuit().expect("circuit").solution.iteration,
            3,
            "{what}: Newton passes (r4133 in-order: 3)"
        );
        let expected: Vec<Complex64> = R4133_PV_PAIR.iter().map(|&(re, im)| cx(re, im)).collect();
        assert_nodes(&dss, &expected, V_TOL);
        assert_kvar(&gen_phase_kvar(&dss, "g1"), &R4133_PV_PAIR_G1_KVAR, what);
        assert_kvar(&gen_phase_kvar(&dss, "g2"), &[0.0; 3], what);
    }
}

/// r4133 on a 1-phase `model=3` machine on `genbus.2` declared before a
/// 3-phase one on `genbus` (case `overlap_b` of the [`R4133_PV_PAIR`] probe):
/// converged in 4 iterations at these node voltages, g1 at -1000 kvar and g2
/// at [`R4133_OVERLAP_G2_KVAR`] per phase.
const R4133_OVERLAP: [(f64, f64); 6] = [
    (7199.557856794634, 0.0),
    (-3599.77892839732, -6234.999999999999),
    (-3599.778928397315, 6235.000000000001),
    (7199.245766614274, -67.03525355964865),
    (-3615.9447579049265, -6225.638669334515),
    (-3541.9240872608734, 6268.046513341663),
];
/// [`R4133_OVERLAP`]'s `Generator.g1` kvar.
const R4133_OVERLAP_G1_KVAR: f64 = -1000.0;
/// [`R4133_OVERLAP`]'s `Generator.g2` kvar per phase.
const R4133_OVERLAP_G2_KVAR: [f64; 3] =
    [-393.95406783240844, 695.5547182367646, -425.59919572602564];
/// The same two machines declared the other way round (case
/// `overlap_b_swapped`): lined up in r4133, converged in 3 iterations with
/// GENBUS.2 held at g1's 7200 V target (the node's last-written target), g1
/// at 0 kvar and g2 at [`R4133_OVERLAP_SWAPPED_G2_KVAR`] per phase.
const R4133_OVERLAP_SWAPPED: [(f64, f64); 6] = [
    (7199.557856794634, 0.0),
    (-3599.77892839732, -6234.999999999999),
    (-3599.778928397315, 6235.000000000001),
    (7199.24558159495, -67.05512075113256),
    (-3616.3081947613955, -6225.938888272321),
    (-3541.952649937933, 6268.030373165959),
];
/// [`R4133_OVERLAP_SWAPPED`]'s `Generator.g2` kvar per phase.
const R4133_OVERLAP_SWAPPED_G2_KVAR: [f64; 3] =
    [-394.1583813523667, -307.32203198516214, -425.55290348788816];

/// **P24** (RF-D00-05 settlement round 2, SA-1) — overlapping PV blocks: a
/// 1-phase g1 on `genbus.2` declared before a 3-phase g2 on `genbus`. g2's
/// first node is new, so r4133 reserves it three rows, a second one for
/// GENBUS.2; `PVBusIdx` is last-writer (`Common/Solution.pas` l.1319), so
/// g1's row carries no equation. r4133 converges (4 iterations) at the port's
/// node voltages and GENBUS.2's summed machine Q, but splits that Q badly: g1
/// clamped at the node's summed 1000 kvar limit, twice its own 500 kvar, and
/// g2's phase 2 absorbing 695.55 kvar against it ([`R4133_OVERLAP`]; never
/// adopted). Here GENBUS.2 has one row and its first claimant, g1, takes the
/// whole regulation step: r4133's own rule for a lined-up block (l.2106),
/// which the swapped deck shows. Declared g2 first, r4133 lines the rows up
/// and the port is its answer ([`R4133_OVERLAP_SWAPPED`], g1 at 0). On the
/// overlap deck the node voltages, g2's phases 1 and 3 and GENBUS.2's summed
/// Q are r4133's numbers; g1 carries that sum (-1000 + 695.55… kvar), g2's
/// phase 2 is 0, and the port takes 3 Newton passes to r4133's 4 (port-only
/// numbers).
#[test]
fn ncim_overlapping_pv_blocks_hold_r4133s_voltages_and_node_q() {
    let g1_on_2 = MIXED_G1.replace("bus1=genbus.1", "bus1=genbus.2");
    let gens = ["Generator.g1", "Generator.g2"];
    let mut dss = solve_ncim(&late_gen_deck(&[g1_on_2.clone(), MIXED_G2.to_string()]));
    assert_genbus_pv_regulated(&mut dss, &gens, &[1, 2, 3]);
    assert_eq!(
        dss.circuit().expect("circuit").solution.iteration,
        3,
        "Newton passes (port-only number: r4133 takes 4)"
    );
    let expected: Vec<Complex64> = R4133_OVERLAP.iter().map(|&(re, im)| cx(re, im)).collect();
    assert_nodes(&dss, &expected, V_TOL);
    let g2 = R4133_OVERLAP_G2_KVAR;
    assert_kvar(
        &gen_phase_kvar(&dss, "g2"),
        &[g2[0], 0.0, g2[2]],
        "overlap g2",
    );
    assert_kvar(
        &gen_phase_kvar(&dss, "g1"),
        &[R4133_OVERLAP_G1_KVAR + g2[1]],
        "overlap g1",
    );

    let mut sw = solve_ncim(&late_gen_deck(&[MIXED_G2.to_string(), g1_on_2]));
    let ckt = sw.circuit().expect("circuit");
    assert!(ckt.is_solved, "the swapped deck converges");
    assert_eq!(ckt.solution.iteration, 3, "swapped: r4133 3");
    let expected: Vec<Complex64> = R4133_OVERLAP_SWAPPED
        .iter()
        .map(|&(re, im)| cx(re, im))
        .collect();
    assert_nodes(&sw, &expected, V_TOL);
    assert_kvar(&gen_phase_kvar(&sw, "g1"), &[0.0], "swapped g1");
    assert_kvar(
        &gen_phase_kvar(&sw, "g2"),
        &R4133_OVERLAP_SWAPPED_G2_KVAR,
        "swapped g2",
    );
    let kcl = term_power_kw(&mut sw, "Generator.g1", 1)
        + term_power_kw(&mut sw, "Generator.g2", 1)
        + term_power_kw(&mut sw, "Load.ld1", 1)
        + term_power_kw(&mut sw, "Line.l1", 2);
    assert!(
        kcl.norm() < 1e-6,
        "swapped: KCL at genbus = {kcl:?} kW/kvar"
    );
}
