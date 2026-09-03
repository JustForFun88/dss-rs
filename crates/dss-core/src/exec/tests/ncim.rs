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
/// ("index out of bounds: the len is 1 but the index is 1") and **r4133 cannot
/// answer this deck at all** — measured 2026-09-03, the `epri-worker` never
/// replies (two runs killed at 150 s and 200 s, worker CPU 0.12 s: blocked, not
/// spinning), because the unchecked overrun kills the DLL's actor thread. So
/// r4133 is not available as the oracle here and this pin is **physics**: KCL.
/// (Its degenerate sibling, [`ncim_missing_voltage_bases_does_not_panic`], is
/// the one deck of this shape r4133 does answer.)
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
    assert!(ckt.is_solved, "the promoted machine's solve must converge");
    assert_eq!(ckt.solution.iteration, 5, "converges in 5 NCIM iterations");
    for (k, v) in ckt.solution.node_v.iter().enumerate().skip(1) {
        assert!(v.re.is_finite() && v.im.is_finite(), "node {k} = {v:?}");
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
/// The swing source's NCIM reported current is the KCL sum at its bus
/// (`TVsourceObj.GetCurrents` → `CalcInjCurrAtBus`, r4133 `VSource.pas` l.1194;
/// port `solution::solution::ncim::ncim_stamp_swing_source_currents`),
/// summed from the connected
/// elements' `Iterminal`. Making the generator's `Iterminal` the NCIM dispatch
/// stamp therefore *would* move that sum — but only for a generator sitting on
/// the swing bus, and none of the gated NCIM cases has one: `ncim_pq` has no
/// generator at all, `ncim_pv_pq` and `ncim_midi` put theirs at `genbus`/`b5`,
/// `Xmission_System_Kundur2Area` has `Generator.G1` commented out in its
/// `Generators.DSS` and G2/G3/G4 at B2/B3/B4, and `IEEE118Bus/master_file.dss`
/// (also an NCIM deck) has its swing-bus machine `Gen_at_89_1` commented out.
/// The three small decks are checked live here; the day one of them puts a
/// generator on node 1, this reds instead of the divergence appearing silently
/// in the swing row.
#[test]
fn ncim_swing_bus_carries_no_generator_on_the_gated_decks() {
    let base =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/modes/ncim");
    for deck in ["ncim_pq.dss", "ncim_pv_pq.dss", "ncim_midi.dss"] {
        let path = base.join(deck);
        assert!(path.is_file(), "vendored corpus deck missing: {path:?}");
        let mut dss = Dss::new();
        dss.command(&format!("compile \"{}\"", path.display()));
        assert!(dss.errors().is_empty(), "{deck}: {:?}", dss.errors());
        let ckt = dss.circuit().expect("circuit");
        for &r in &ckt.generators {
            let g = dss.classes[r.class_ord()]
                .arena
                .get::<crate::elements::pc::generator::Generator>(r.index())
                .expect("generators list holds Generators");
            assert!(
                !g.cd.enabled || g.cd.node_ref.first() != Some(&1),
                "{deck}: Generator.{} sits on the swing node — the swing-source \
                 KCL sum now reads its NCIM dispatch stamp, so this case's \
                 `Vsource` row moves and needs a pin",
                g.cd.obj.name()
            );
        }
        // The same for a second *source* on the slack node: the stamp is written
        // for exactly one of them (`ncim_stamp_swing_source_currents` takes the
        // first), the others report their own `YPrim·V - Iinj`
        // ([`ncim_second_slack_node_vsource_reports_its_own_current`]), and the
        // stamp's PC term would carry them into the swing row through the
        // untested `cadd` sign (`VSource.pas` l.1169). r4133 cannot referee that
        // deck at all — its per-read `CalcInjCurrAtBus` recursion overflows the
        // stack — so the day a gated deck grows one, it needs its own pin.
        let slack_sources = ckt
            .sources
            .iter()
            .filter(|&&r| {
                let cd = dss.classes[r.class_ord()]
                    .arena
                    .try_ckt_elem(r.index())
                    .expect("sources list holds circuit elements")
                    .cd();
                cd.enabled && cd.node_ref.first() == Some(&1)
            })
            .count();
        assert!(
            slack_sources <= 1,
            "{deck}: {slack_sources} enabled sources sit on the swing node — only \
             one is stamped with the `CalcInjCurrAtBus` sum; this case's `Vsource` \
             rows need their own pin"
        );
    }
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
/// `0.02·7199.5578 V / (12.47² / 1500 Ω)` = `1389.0 A`, and the stamp equals
/// the Pascal sum over the bus. Whether that sum's **PC sign** (`cadd`,
/// `VSource.pas` l.1169) is the physically right one for a PC element that is
/// not the swing source is untested by any oracle and is exactly what the
/// tripwire [`ncim_swing_bus_carries_no_generator_on_the_gated_decks`] guards
/// on the gated decks; it is not decided here.
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
        assert!(
            (i.norm() - expect).abs() < 1e-3 * expect,
            "Vsource.src2 conductor {k}: |I| = {} A, Thevenin |E2-V|/|Z1| = {expect} A \
             (RP3.13 regression: the swing echo printed 0 here)",
            i.norm()
        );
    }
    assert!(
        (cdang(i_src2[0]) - 104.04).abs() < 5e-2,
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

    // The swing source still takes the stamp, and the stamp is the Pascal sum
    // over every element at the bus: `-Σ PD + Σ other PC` (`VSource.pas`
    // l.1135 / l.1169). With `SRC2` echoing a stamp it never received this
    // identity read `I(SOURCE) = -I(Line.l1 t1)` instead.
    let i_source = elem_currents(&mut dss, "Vsource.source");
    let i_line = elem_currents(&mut dss, "Line.l1");
    for k in 0..3 {
        let sum = -i_line[k] + i_src2[k];
        assert!(
            (i_source[k] - sum).norm() < 1e-9,
            "Vsource.source conductor {k}: stamp {:?} vs the Pascal bus sum {sum:?}",
            i_source[k]
        );
    }
    assert!(
        i_source[0].norm() > 1e3,
        "the swing source must carry the feeder and the second source: {:?}",
        i_source[0]
    );
}
