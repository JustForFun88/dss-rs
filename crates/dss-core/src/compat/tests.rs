//! Per-kernel tests of the Stage F lane split. Both `*_impl`s are compiled in
//! every build, so these run — and pin the documented parity-vs-default
//! bounds — in **both** lanes.

use super::*;

const fn c(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

/// A small, deterministic operand set covering both branches of Smith's
/// division (|den.re| > |den.im| and the swapped case) at circuit-typical
/// magnitudes.
const DIV_CASES: [(Complex64, Complex64); 6] = [
    (c(0.0012433, 0.0012433), c(0.0012433, 0.0011)),
    (c(0.0012433, 0.0012433), c(0.0012433, 0.36)),
    (c(1.0, 0.0), c(0.3, -0.9)),
    (c(-12.5, 3.25), c(0.004, 0.017)),
    (c(1.0e6, -2.5e5), c(-7.5, 11.25)),
    (c(3.0, 4.0), c(-1.0e-8, 2.0e-8)),
];

/// Max relative deviation between the two division kernels over `DIV_CASES`,
/// measured 2026-07-26: **2.10e-16** = 0.95 ULP (f64 ULP = 2.22e-16), exactly
/// the "rounds the last bit differently" claim. The asserted bound keeps ~4
/// ULP of headroom.
const DIV_REL_BOUND: f64 = 1.0e-15;

// ---------------------------------------------------------------------------
// Alias selection per lane (F.3 flips one kernel family per commit)
// ---------------------------------------------------------------------------

/// The `dss-core` kernels **not yet flipped**: `invert` and `etk_invert` still
/// select the parity impl in *both* lanes, so the existing gate (byte goldens,
/// checkpoint Y, corpus floors) is unchanged in the default build too until
/// their own commit lands.
///
/// Each assertion is behavioral (a value on which the two `_impl`s genuinely
/// differ), so it cannot pass by accident.
#[test]
fn unflipped_aliases_still_select_the_parity_kernel_in_both_lanes() {
    // Dense complex inverse: the anti-diagonal matrix separates the kernels
    // (no-row-exchange GJ reports it singular, partial pivoting inverts it).
    let mut m = anti_diagonal_2x2();
    assert!(invert(&mut m).is_err());
    let mut m = anti_diagonal_2x2();
    assert!(invert_partial_pivot_impl(&mut m).is_ok());

    // Real inverse: same separator.
    let mut a = [0.0, 1.0, 1.0, 0.0];
    assert!(etk_invert(&mut a, 2).is_err());
    let mut a = [0.0, 1.0, 1.0, 0.0];
    assert!(etk_invert_partial_pivot_impl(&mut a, 2).is_ok());
}

/// The **no-split** row (F.3e): `cdiv` is one shared kernel, so it must resolve
/// to Smith's division in *whichever* lane this test runs — asserted on an
/// operand pair where the two impls differ bitwise, so it cannot pass
/// vacuously.
#[test]
fn cdiv_is_one_shared_kernel_in_both_lanes() {
    let (num, den) = DIV_CASES[0];
    assert_eq!(
        cdiv(num, den).re.to_bits(),
        cdiv_fpc_impl(num, den).re.to_bits()
    );
    assert_ne!(
        cdiv_fpc_impl(num, den).re.to_bits(),
        cdiv_std_impl(num, den).re.to_bits()
    );
}

/// The **flipped** row: `stddev_single_point` resolves to the upstream quirk
/// under `oracle-parity` and to the correct `0.0` otherwise. Asserted on a
/// value where the impls disagree, so neither lane passes vacuously; the
/// engine-visible end of the same divergence is pinned by
/// `mathutil::tests::single_point_std_dev_is_the_lane_kernel` and by the
/// `LoadShape.stddev` deck-level test.
#[test]
fn stddev_alias_is_the_lane_kernel() {
    assert_eq!(stddev_single_point_value_impl(3.5), 3.5);
    assert_eq!(stddev_single_point_zero_impl(3.5), 0.0);

    let expected = if ORACLE_PARITY { 3.5 } else { 0.0 };
    assert_eq!(stddev_single_point(3.5), expected);
}

/// Feature propagation is **measured, not assumed**. Two of the plan's kernel
/// rows (the RPN pi and FPC `Round`) live in `dss-parser` and the solver knobs
/// live in `dss-sparse`, reached only through
/// `dss-core/oracle-parity → {dss-parser,dss-sparse}/oracle-parity`. If that
/// edge were ever dropped from `Cargo.toml`, those crates would compile their
/// *default* kernels inside a parity build and their own per-lane tests could
/// not tell (const and alias come from the same compilation unit) — the parity
/// contract would quietly evaporate. Anchored on this crate's const, which is
/// the feature the gate invocation names directly.
#[test]
fn the_lane_reaches_every_compat_crate() {
    assert_eq!(
        ORACLE_PARITY,
        dss_parser::compat::ORACLE_PARITY,
        "dss-core/oracle-parity did not reach dss-parser"
    );
    assert_eq!(
        ORACLE_PARITY,
        dss_sparse::compat::ORACLE_PARITY,
        "dss-core/oracle-parity did not reach dss-sparse"
    );
}

// ---------------------------------------------------------------------------
// Complex division
// ---------------------------------------------------------------------------

/// The parity kernel is FPC `ucomplex` Smith division, bit-for-bit — the same
/// pin as `cmatrix::tests::cdiv_fpc_matches_fpc_smith_not_naive`, kept next to
/// the impl so a lane edit trips it here first.
#[test]
fn cdiv_parity_kernel_is_fpc_smith_bitwise() {
    let num = c(0.0012433, 0.0012433);
    let den = c(0.0012433, 0.0011);
    let q = cdiv_fpc_impl(num, den);
    assert_eq!(q.re.to_bits(), 0x3ff0ea49fd1e0356);
    assert_eq!(q.im.to_bits(), 0x3fb08cf7c22f85eb);

    let den = c(0.0012433, 0.36);
    let q = cdiv_fpc_impl(num, den);
    assert_eq!(q.re.to_bits(), 0x3f6c63aca54cccd3);
    assert_eq!(q.im.to_bits(), 0xbf6c31a5d17ddb1d);
}

/// The evidence behind the row's **no split** verdict, kept executable so it
/// cannot decay into folklore.
///
/// Each row is `(num, den, correctly-rounded quotient)`; the reference was
/// computed in 60-digit `Decimal` (independent of both kernels) and the rows
/// were selected as cases where the two kernels genuinely disagree. Smith's
/// division must be **at least as close** to the true quotient as the naive
/// form on every row, and strictly closer on at least one — which is the whole
/// argument for keeping it in the product lane.
const DIV_REFERENCE: [(Complex64, Complex64, Complex64); 6] = [
    (
        c(0.002321729484, -0.006576942029),
        c(4.635502e-06, -1.7351297e-05),
        c(
            f64::from_bits(0x4078329354f0ea98),
            f64::from_bits(0x403e5ff9e9aed9e9),
        ),
    ),
    (
        c(-4.593386e-06, -6.59001e-07),
        c(-1.056509107804, -1.005279277728),
        c(
            f64::from_bits(0x3ec5c1141db80686),
            f64::from_bits(0xbebeef12399752f4),
        ),
    ),
    (
        c(-622.258737287941, 3943.574810490002),
        c(-190543.68643387672, 87389.84932548525),
        c(
            f64::from_bits(0x3f8596526e244800),
            f64::from_bits(0xbf903e2a75dec667),
        ),
    ),
    (
        c(37.525861080188, -25.109089063361),
        c(-188746.93835912598, 278724.15562414465),
        c(
            f64::from_bits(0xbf2049d2e2d813a6),
            f64::from_bits(0xbf0a7768806904b6),
        ),
    ),
    (
        c(2.50311267631, 2.853591214084),
        c(-17667.064091758657, -6753.628705423371),
        c(
            f64::from_bits(0xbf27439524d103a1),
            f64::from_bits(0xbf188e2468de6c87),
        ),
    ),
    (
        c(-0.000102367606, -0.000286357051),
        c(-8.8600379e-05, -6.1824964e-05),
        c(
            f64::from_bits(0x400259a9e312c329),
            f64::from_bits(0x3ffa1a482d26e7b8),
        ),
    ),
];

/// ULP distance between two finite same-sign-exponent doubles.
fn ulp_distance(a: f64, b: f64) -> u64 {
    let (ia, ib) = (a.to_bits() as i64, b.to_bits() as i64);
    ia.abs_diff(ib)
}

#[test]
fn cdiv_shared_kernel_is_the_more_accurate_one() {
    let mut smith_total = 0u64;
    let mut naive_total = 0u64;
    let mut strictly_better_somewhere = false;

    for (num, den, exact) in DIV_REFERENCE {
        let s = cdiv_fpc_impl(num, den);
        let n = cdiv_std_impl(num, den);
        let ds = ulp_distance(s.re, exact.re) + ulp_distance(s.im, exact.im);
        let dn = ulp_distance(n.re, exact.re) + ulp_distance(n.im, exact.im);
        assert!(
            ds <= dn,
            "Smith lost to the naive form on {num}/{den}: {ds} vs {dn} ULP \
             from the Decimal-60 reference"
        );
        strictly_better_somewhere |= ds < dn;
        smith_total += ds;
        naive_total += dn;
    }

    assert!(strictly_better_somewhere);
    assert!(
        smith_total < naive_total,
        "shared-kernel verdict no longer holds: Smith {smith_total} ULP vs \
         naive {naive_total} ULP over the reference table"
    );
}

/// The other half of the verdict: the naive form squares the denominator, so it
/// collapses to `0` (or `NaN`) wherever `|den|²` leaves f64's range, while
/// Smith's abs-ratio form stays exact there. A `compat::cdiv` flipped to the
/// naive kernel would hand the *product* lane this failure mode for free.
#[test]
fn naive_division_collapses_where_smith_stays_exact() {
    // |den| = 1e200 → den.norm_sqr() overflows to +inf.
    let (num, den) = (c(1.0, 1.0), c(1.0e200, 1.0e200));
    assert_eq!(cdiv_fpc_impl(num, den), c(1.0e-200, 0.0));
    assert_eq!(cdiv_std_impl(num, den), c(0.0, 0.0));

    // |den| = 1e-200 → den.norm_sqr() underflows to 0.
    let (num, den) = (c(1.0e-200, 1.0e-200), c(1.0e-200, 1.0e-200));
    assert_eq!(cdiv_fpc_impl(num, den), c(1.0, 0.0));
    assert!(cdiv_std_impl(num, den).re.is_nan());
}

#[test]
fn cdiv_impls_agree_within_one_ulp() {
    let mut worst = 0.0f64;
    let mut any_bitwise_difference = false;
    for (num, den) in DIV_CASES {
        let fpc = cdiv_fpc_impl(num, den);
        let std = cdiv_std_impl(num, den);
        if fpc != std {
            any_bitwise_difference = true;
        }
        let scale = fpc.norm();
        assert!(scale > 0.0);
        worst = worst.max((fpc - std).norm() / scale);
    }
    assert!(
        worst <= DIV_REL_BOUND,
        "parity-vs-default complex division drifted: {worst:e} > {DIV_REL_BOUND:e}"
    );
    // The kernels really are different code paths (guards against a future
    // edit that quietly aliases one to the other and makes the bound vacuous).
    assert!(any_bitwise_difference);
}

// ---------------------------------------------------------------------------
// Dense complex inverse
// ---------------------------------------------------------------------------

fn anti_diagonal_2x2() -> CMatrix {
    let mut m = CMatrix::new(2);
    m.set(0, 1, Complex64::ONE);
    m.set(1, 0, Complex64::ONE);
    m
}

/// A well-conditioned 3×3 complex impedance matrix (circuit-typical R+jX with
/// mutual coupling).
fn impedance_3x3() -> CMatrix {
    let mut m = CMatrix::new(3);
    let entries = [
        [c(0.34, 1.02), c(0.15, 0.45), c(0.15, 0.40)],
        [c(0.15, 0.45), c(0.34, 1.02), c(0.15, 0.45)],
        [c(0.15, 0.40), c(0.15, 0.45), c(0.34, 1.02)],
    ];
    for (i, row) in entries.iter().enumerate() {
        for (j, &v) in row.iter().enumerate() {
            m.set(i, j, v);
        }
    }
    m
}

fn max_abs(m: &CMatrix) -> f64 {
    let n = m.order();
    let mut worst = 0.0f64;
    for i in 0..n {
        for j in 0..n {
            worst = worst.max(m[(i, j)].norm());
        }
    }
    worst
}

fn inverse_residual(a: &CMatrix, inv: &CMatrix) -> f64 {
    let n = a.order();
    let mut worst = 0.0f64;
    for i in 0..n {
        for j in 0..n {
            let mut s = Complex64::ZERO;
            for k in 0..n {
                s += a[(i, k)] * inv[(k, j)];
            }
            let expect = if i == j {
                Complex64::ONE
            } else {
                Complex64::ZERO
            };
            worst = worst.max((s - expect).norm());
        }
    }
    worst
}

/// Documented parity-vs-default bound for the dense complex inverse on a
/// well-conditioned matrix, measured 2026-07-26: max |Δ| / max |A⁻¹| =
/// **2.15e-16** (≈ 1 ULP; the residuals ‖A·A⁻¹ − I‖ are 2.30e-16 for the
/// parity kernel and 1.27e-16 for the partial-pivoting one). Asserted with a
/// decade of headroom.
const INVERT_REL_BOUND: f64 = 5.0e-15;

#[test]
fn invert_impls_agree_on_a_well_conditioned_matrix() {
    let a = impedance_3x3();

    let mut gj = a.clone();
    invert_gj_no_exchange_impl(&mut gj).unwrap();
    let mut pp = a.clone();
    invert_partial_pivot_impl(&mut pp).unwrap();

    // Both are genuine inverses.
    assert!(inverse_residual(&a, &gj) < 1e-14);
    assert!(inverse_residual(&a, &pp) < 1e-14);

    let scale = max_abs(&gj);
    let mut worst = 0.0f64;
    for i in 0..3 {
        for j in 0..3 {
            worst = worst.max((gj[(i, j)] - pp[(i, j)]).norm() / scale);
        }
    }
    assert!(
        worst <= INVERT_REL_BOUND,
        "parity-vs-default dense inverse drifted: {worst:e} > {INVERT_REL_BOUND:e}"
    );
}

/// The deliberate divergence of the default kernel: Pascal's `TcMatrix.Invert`
/// only ever pivots on the *diagonal*, so a matrix whose mass sits off it is
/// reported singular; partial pivoting inverts it correctly.
#[test]
fn partial_pivot_inverts_what_the_no_exchange_kernel_calls_singular() {
    let a = anti_diagonal_2x2();

    let mut gj = a.clone();
    assert_eq!(invert_gj_no_exchange_impl(&mut gj), Err(SingularMatrix));

    let mut pp = a.clone();
    invert_partial_pivot_impl(&mut pp).unwrap();
    // [[0,1],[1,0]] is its own inverse.
    assert_eq!(pp[(0, 1)], Complex64::ONE);
    assert_eq!(pp[(1, 0)], Complex64::ONE);
    assert_eq!(pp[(0, 0)], Complex64::ZERO);
    assert_eq!(pp[(1, 1)], Complex64::ZERO);
}

/// The second deliberate difference: on a genuinely singular matrix the parity
/// kernel leaves the caller's matrix partially transformed (upstream behavior,
/// documented on the parity impl), the default kernel leaves it untouched.
#[test]
fn singular_input_is_left_untouched_only_by_the_partial_pivot_kernel() {
    let mut a = CMatrix::new(2); // rank 1: [[1,2],[2,4]]
    a.set(0, 0, c(1.0, 0.0));
    a.set(0, 1, c(2.0, 0.0));
    a.set(1, 0, c(2.0, 0.0));
    a.set(1, 1, c(4.0, 0.0));

    let mut gj = a.clone();
    assert_eq!(invert_gj_no_exchange_impl(&mut gj), Err(SingularMatrix));
    assert_ne!(
        gj, a,
        "parity kernel leaves debris, like the Pascal original"
    );

    let mut pp = a.clone();
    assert_eq!(invert_partial_pivot_impl(&mut pp), Err(SingularMatrix));
    assert_eq!(pp, a);
}

// ---------------------------------------------------------------------------
// Dense real inverse (ETKInvert)
// ---------------------------------------------------------------------------

/// Documented parity-vs-default bound for the real inverse, measured
/// 2026-07-26: max |Δ| / max |A⁻¹| = **1.25e-16** (≈ 0.6 ULP). Decade of
/// headroom.
const ETK_REL_BOUND: f64 = 5.0e-15;

#[test]
fn etk_invert_impls_agree_on_a_well_conditioned_matrix() {
    // Column-major 3×3, the matrix the mathutil unit tests use.
    let vals = [4.0, 1.0, 2.0, 0.5, 3.0, 1.5, 1.0, 2.5, 5.0];

    let mut gj = vals;
    etk_invert_gj_no_exchange_impl(&mut gj, 3).unwrap();
    let mut pp = vals;
    etk_invert_partial_pivot_impl(&mut pp, 3).unwrap();

    let scale = gj.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let worst = gj
        .iter()
        .zip(&pp)
        .fold(0.0f64, |m, (a, b)| m.max((a - b).abs() / scale));
    assert!(
        worst <= ETK_REL_BOUND,
        "parity-vs-default real inverse drifted: {worst:e} > {ETK_REL_BOUND:e}"
    );

    // Both are genuine inverses of the original.
    for inv in [&gj, &pp] {
        for i in 0..3 {
            for j in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += vals[k * 3 + i] * inv[j * 3 + k];
                }
                let expect = if i == j { 1.0 } else { 0.0 };
                assert!((s - expect).abs() < 1e-12, "A·A⁻¹[{i},{j}] = {s}");
            }
        }
    }
}

#[test]
fn etk_invert_kernels_on_singular_and_off_diagonal_input() {
    // Rank 1 → both report singular; only the default kernel is non-destructive.
    let vals = [1.0, 2.0, 2.0, 4.0];
    let mut gj = vals;
    assert_eq!(
        etk_invert_gj_no_exchange_impl(&mut gj, 2),
        Err(SingularMatrix)
    );
    let mut pp = vals;
    assert_eq!(
        etk_invert_partial_pivot_impl(&mut pp, 2),
        Err(SingularMatrix)
    );
    assert_eq!(pp, vals);

    // Zero diagonal → only partial pivoting can invert.
    let anti = [0.0, 1.0, 1.0, 0.0];
    let mut gj = anti;
    assert_eq!(
        etk_invert_gj_no_exchange_impl(&mut gj, 2),
        Err(SingularMatrix)
    );
    let mut pp = anti;
    etk_invert_partial_pivot_impl(&mut pp, 2).unwrap();
    assert_eq!(pp, anti);
}

// ---------------------------------------------------------------------------
// Single-point standard deviation
// ---------------------------------------------------------------------------

/// Not a numeric bound but a **deliberate divergence**: the parity kernel
/// reproduces the upstream bug (stddev of one sample = the sample), the
/// default kernel returns the correct `0.0`. F.3 pins the default lane with
/// expected-value tests at the four `mathutil` call sites.
#[test]
fn stddev_single_point_kernels_are_a_deliberate_divergence() {
    for v in [0.0, 3.5, -12.25, 1.0e9] {
        assert_eq!(stddev_single_point_value_impl(v), v);
        assert_eq!(stddev_single_point_zero_impl(v), 0.0);
    }
}
