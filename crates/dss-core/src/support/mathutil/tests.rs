use super::*;

const J: Complex64 = Complex64::new(0.0, 1.0);

fn polar_deg(mag: f64, ang_deg: f64) -> Complex64 {
    let ang = ang_deg.to_radians();
    c(mag * ang.cos(), mag * ang.sin())
}

#[test]
fn balanced_positive_sequence_maps_to_pure_v1() {
    // Va = 1∠0, Vb = 1∠-120, Vc = 1∠120 → V0 = 0, V1 = 1, V2 = 0.
    let t = SymComp::precise();
    let vph = [
        polar_deg(1.0, 0.0),
        polar_deg(1.0, -120.0),
        polar_deg(1.0, 120.0),
    ];
    let mut v012 = [Complex64::ZERO; 3];
    t.phase_to_sym(&vph, &mut v012);
    assert!(v012[0].norm() < 1e-15, "V0 = {}", v012[0]);
    assert!((v012[1] - c(1.0, 0.0)).norm() < 1e-15, "V1 = {}", v012[1]);
    assert!(v012[2].norm() < 1e-15, "V2 = {}", v012[2]);
}

#[test]
fn sym_comp_round_trip_precise() {
    let t = SymComp::precise();
    let vph = [c(1.02, 0.05), c(-0.6, -0.85), c(-0.45, 0.9)];
    let mut v012 = [Complex64::ZERO; 3];
    let mut back = [Complex64::ZERO; 3];
    t.phase_to_sym(&vph, &mut v012);
    t.sym_to_phase(&v012, &mut back);
    for (a, b) in vph.iter().zip(&back) {
        assert!((a - b).norm() < 1e-12, "{a} vs {b}");
    }
}

#[test]
fn sym_comp_round_trip_official() {
    // The official matrices use truncated constants but the inverse is the
    // numeric inversion of the same matrix, so the round trip still holds.
    let t = SymComp::official();
    let vph = [c(0.98, -0.1), c(-0.55, -0.82), c(-0.5, 0.88)];
    let mut v012 = [Complex64::ZERO; 3];
    let mut back = [Complex64::ZERO; 3];
    t.phase_to_sym(&vph, &mut v012);
    t.sym_to_phase(&v012, &mut back);
    for (a, b) in vph.iter().zip(&back) {
        assert!((a - b).norm() < 1e-9, "{a} vs {b}");
    }
}

/// Stage F (Part IV.2) lists "sym components" as a dual kernel with the
/// `official` matrices on the parity side — but the *pinned oracle does not use
/// them*: `mathutil.pas:548` ends its initialization with
/// `SelectAs2pVersion(False)` (= "ours" = [`SymComp::precise`]), and the
/// `official` pair is reachable only through upstream's
/// `DSSCompatFlag.BadPrecision` env flag (`CAPI_DSS.pas:315`), which no gating
/// oracle sets. So this row needs **no lane split** — parity == default ==
/// `precise` — and `compat.rs` carries no alias for it. Both variants stay
/// compiled, and this test pins the gap between them so the claim stays
/// measured rather than asserted.
#[test]
fn sym_comp_official_vs_precise_gap_is_the_truncated_sin60_constant() {
    let precise = SymComp::precise();
    let official = SymComp::official();
    let vph = [c(1.02, 0.05), c(-0.6, -0.85), c(-0.45, 0.9)];
    let mut a = [Complex64::ZERO; 3];
    let mut b = [Complex64::ZERO; 3];
    precise.phase_to_sym(&vph, &mut a);
    official.phase_to_sym(&vph, &mut b);

    let scale = a.iter().fold(0.0f64, |m, v| m.max(v.norm()));
    let worst = a
        .iter()
        .zip(&b)
        .fold(0.0f64, |m, (x, y)| m.max((x - y).norm() / scale));
    // sin(60°) truncated to 0.866025403 is 4.4e-10 relative and the transform
    // carries it straight through. Measured 2026-07-26: 4.50e-10.
    assert!(
        worst > 1e-12,
        "the two variants must really differ: {worst:e}"
    );
    assert!(worst < 1e-8, "official-vs-precise drifted: {worst:e}");
}

#[test]
fn zero_sequence_set_maps_to_pure_v0() {
    let t = SymComp::precise();
    let v = c(0.9, 0.3);
    let vph = [v, v, v];
    let mut v012 = [Complex64::ZERO; 3];
    t.phase_to_sym(&vph, &mut v012);
    assert!((v012[0] - v).norm() < 1e-15);
    assert!(v012[1].norm() < 1e-15);
    assert!(v012[2].norm() < 1e-15);
}

#[test]
fn terminal_power_and_k_powers() {
    let v = [c(100.0, 0.0), c(0.0, 100.0)];
    let i = [c(2.0, -1.0), c(1.0, 1.0)];
    // S = Σ V conj(I) = 100(2+j) + 100j(1-j) = (200+100) + j(100+100)
    let s = terminal_power_in(&v, &i, 2);
    assert_eq!(s, c(300.0, 200.0));

    let mut kw = [Complex64::ZERO; 2];
    calc_k_powers(&mut kw, &v, &i, 2);
    assert_eq!(kw[0], c(0.2, 0.1));
    assert_eq!(kw[1], c(0.1, 0.1));
}

#[test]
fn xr_ratio_clamps() {
    assert_eq!(get_xr(c(1.0, 3.0)), 3.0);
    assert_eq!(get_xr(c(0.0, 5.0)), 9999.0);
    assert_eq!(get_xr(c(1e-9, 1.0)), 9999.0); // > 9999 clamps
    assert_eq!(get_xr(c(1.0, -3.0)), -3.0);
}

#[test]
fn parallel_impedances() {
    // Two equal impedances in parallel halve.
    let z = c(2.0, 4.0);
    assert!((parallel_z(z, z) - z / 2.0).norm() < 1e-15);
    // Exactly opposite impedances: Pascal returns zero.
    assert_eq!(parallel_z(z, -z), Complex64::ZERO);
}

#[test]
fn etk_invert_matches_cmatrix_invert_on_real_matrix() {
    // Same algorithm, real vs complex: results must agree exactly.
    let vals = [4.0, 1.0, 2.0, 0.5, 3.0, 1.5, 1.0, 2.5, 5.0]; // column-major 3x3
    let mut real = vals;
    etk_invert(&mut real, 3).unwrap();

    let mut cm = CMatrix::new(3);
    for j in 0..3 {
        for i in 0..3 {
            cm.set(i, j, c(vals[j * 3 + i], 0.0));
        }
    }
    cm.invert().unwrap();
    for j in 0..3 {
        for i in 0..3 {
            assert_eq!(real[j * 3 + i], cm.get(i, j).re);
            assert_eq!(cm.get(i, j).im, 0.0);
        }
    }
}

#[test]
fn etk_invert_singular() {
    let mut a = [1.0, 2.0, 2.0, 4.0]; // rank 1
    assert!(etk_invert(&mut a, 2).is_err());
}

#[test]
fn gauss_with_constant_rng_is_deterministic() {
    // 12 × 0.5 = 6 → exactly the mean.
    assert_eq!(gauss(10.0, 2.0, || 0.5), 10.0);
    // 12 × 0.25 = 3 → mean - 3σ.
    assert_eq!(gauss(0.0, 1.0, || 0.25), -3.0);
    assert_eq!(quasi_log_normal(3.0, || 0.5), 3.0); // e^0 * mean
}

#[test]
fn mean_and_std_dev_basics() {
    let (m, s) = mean_and_std_dev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
    assert!((m - 5.0).abs() < 1e-15);
    assert!((s - (32.0f64 / 7.0).sqrt()).abs() < 1e-12);
}

// EXPECTED-VALUE-PIN(stddev_single_point): the four entry points, one
// unconditional expected value per lane-free assertion.
/// A one-element sample has **no spread**, at all four entry points, in both
/// lanes.
///
/// Upstream's single-point branch assigns the mean and then repeats the same
/// right-hand side into `StdDev` — `StdDev := Data^[1];`, r4133
/// `Version8/Source/Shared/mathutil.pas:405` and `:429`, identical in the
/// pinned dss_capi 0.14.5 (`:323`, `:349`, `:370`, `:398`) — so a `{3.5}`
/// sample is reported as 100 % spread. Both gating oracles carry it and
/// neither lane reproduces it (GOLDEN_REBASE G2.1a; `issue-11`).
///
/// Expected values, not tolerances: **both** out-params are pinned literally —
/// the mean is the sample, the std-dev is `0.0` — so an entry point that
/// returned nothing fails as loudly as one that repeats the sample. Three
/// magnitudes (all exact in f32, so the single-precision pair is exact too),
/// because the quirk is `StdDev := Data[1]`: it reproduces `0.0` for free on a
/// zero sample and only shows on a non-zero one.
#[test]
fn single_point_std_dev_is_zero() {
    for v in [0.0f64, 3.5, -12.25, 1.0e9] {
        let s = v as f32;
        assert_eq!(mean_and_std_dev(&[v]), (v, 0.0), "sample {v}");
        assert_eq!(mean_and_std_dev_single(&[s]), (v, 0.0), "sample {v}");
        assert_eq!(curve_mean_and_std_dev(&[v], &[0.0]), (v, 0.0), "curve {v}");
        assert_eq!(
            curve_mean_and_std_dev_single(&[s], &[0.0f32]),
            (v, 0.0),
            "curve {v}"
        );
    }
}

#[test]
fn curve_mean_of_constant_curve() {
    let x = [0.0, 1.0, 3.0, 6.0];
    let y = [2.0, 2.0, 2.0, 2.0];
    let (m, s) = curve_mean_and_std_dev(&y, &x);
    assert!((m - 2.0).abs() < 1e-15);
    assert!(s.abs() < 1e-15);

    // ramp 0→1 over [0,1]: trapezoid mean = 0.5
    let x = [0.0, 1.0];
    let y = [0.0, 1.0];
    let (m, _) = curve_mean_and_std_dev(&y, &x);
    assert!((m - 0.5).abs() < 1e-15);
}

#[test]
fn bessel_small_argument_series_values() {
    // I0(x) ≈ 1 + x²/4 + x⁴/64, I1(x) ≈ x/2 + x³/16 for small real x.
    let x = 0.1;
    let i0 = bessel_i0(c(x, 0.0));
    let expect = 1.0 + x * x / 4.0 + x.powi(4) / 64.0 + x.powi(6) / 2304.0;
    assert!((i0.re - expect).abs() < 1e-12, "{} vs {expect}", i0.re);
    assert!(i0.im == 0.0);

    let i1 = bessel_i1(c(x, 0.0));
    let expect = x / 2.0 + x.powi(3) / 16.0 + x.powi(5) / 384.0 + x.powi(7) / 18432.0;
    assert!((i1.re - expect).abs() < 1e-15, "{} vs {expect}", i1.re);
}

#[test]
fn bessel_complex_argument_converges() {
    // I0(j x) = J0(x); J0(1) = 0.7651976865579666
    let i0 = bessel_i0(J * 1.0);
    assert!((i0.re - 0.7651976865579666).abs() < 1e-12, "{}", i0.re);
    assert!(i0.im.abs() < 1e-15);
}

#[test]
fn nema_unbalance() {
    let balanced = [
        polar_deg(1.0, 0.0),
        polar_deg(1.0, -120.0),
        polar_deg(1.0, 120.0),
    ];
    assert!(pct_nema_unbalance(&balanced) < 1e-12);

    let unbalanced = [
        polar_deg(1.1, 0.0),
        polar_deg(1.0, -120.0),
        polar_deg(0.9, 120.0),
    ];
    // avg = 1.0, max diff = 0.1 → 10%
    assert!((pct_nema_unbalance(&unbalanced) - 10.0).abs() < 1e-12);

    assert_eq!(pct_nema_unbalance(&[Complex64::ZERO; 3]), 0.0);
}

#[test]
fn pi_ctrl_step_response() {
    // Reproduce the difference equation by hand for a few steps.
    let mut ctrl = PiCtrl::new();
    let sp = 1.0;
    // step1: num=[0, 0.02], den=[0, 0]
    assert_eq!(ctrl.solve_pi(sp), 0.0);
    // step2: num=[0.02, 0.02], den=[0, 0.02*0.8647]
    let d2 = ctrl.solve_pi(sp);
    assert!((d2 - 0.02 * 0.8647).abs() < 1e-15);
    // step3: den = num0*kNum + den0*kDen
    let d3 = ctrl.solve_pi(sp);
    assert!((d3 - (0.02 * 0.8647 + d2 * 0.1353)).abs() < 1e-15);
}
