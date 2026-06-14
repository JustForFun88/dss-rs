//! Unit tests for the Carson engine, pinned against the dss-python oracle
//! (PIN.txt 0.15.7 / backend 0.14.5). Reference matrices were probed by
//! building the same geometry through a `Line` and reading `Rmatrix`/`Xmatrix`
//! (ohm per unit length, here ohm/m) and `Cmatrix` (nF per unit length, here
//! nF/m) — i.e. the engine outputs that the geometry path produces. See the
//! WP7.1 probe in STATUS.md.

use super::*;

const M: i32 = 4; // LineUnits::Meter code

// Shared SI wire: rac = 3e-4 ohm/m, gmr = 0.005 m, radius = 0.01 m.
// The oracle derives Rdc = Rac / 1.02 when Rdc is unset (ConductorData side
// effect), which the DERI skin-effect term consumes.
const RAC: f64 = 3.0e-4;
const GMR: f64 = 0.005;
const RADIUS: f64 = 0.01;

/// Build a LineConstants for an overhead geometry of `(x, h)` meter coordinates,
/// mirroring the assignment order of `TLineGeometryObj.UpdateLineGeometryData`.
fn build(coords: &[(f64, f64)]) -> LineConstants {
    let n = coords.len();
    let mut lc = LineConstants::new(n);
    for (i, &(x, h)) in coords.iter().enumerate() {
        lc.set_x(i, M, x);
        lc.set_y(i, M, h);
        lc.set_radius(i, M, RADIUS);
        lc.set_capradius(i, M, RADIUS);
        lc.set_gmr(i, M, GMR);
        lc.set_rdc(i, M, RAC / 1.02);
        lc.set_rac(i, M, RAC);
    }
    lc
}

fn assert_close(got: f64, want: f64, what: &str) {
    let tol = 1e-8 * want.abs().max(1e-12);
    assert!(
        (got - want).abs() <= tol,
        "{what}: got {got:.12e}, want {want:.12e} (|d|={:.3e})",
        (got - want).abs()
    );
}

/// Compare the engine's base Z (ohm/m) against an oracle reference, row-major.
fn check_z(lc: &LineConstants, z_ref: &[(f64, f64)]) {
    let n = lc.num_conductors();
    let z = lc.z_base();
    for i in 0..n {
        for j in 0..n {
            let (re, im) = z_ref[i * n + j];
            let g = z.get(i, j);
            assert_close(g.re, re, &format!("Z[{i}][{j}].re"));
            assert_close(g.im, im, &format!("Z[{i}][{j}].im"));
        }
    }
}

/// Compare the engine's base capacitance C = Im(Yc)/w (F/m) against the oracle
/// `Cmatrix` (nF/m), row-major.
fn check_c(lc: &LineConstants, c_ref_nf: &[f64]) {
    let n = lc.num_conductors();
    let yc = lc.yc_base();
    let w = lc.omega();
    for i in 0..n {
        for j in 0..n {
            let want = c_ref_nf[i * n + j] * 1e-9; // nF/m -> F/m
            let got = yc.get(i, j).im / w;
            assert_close(got, want, &format!("C[{i}][{j}]"));
        }
    }
}

// 3-phase overhead, conductors at x = 0/1/2 m, h = 10 m.
const COORDS3: [(f64, f64); 3] = [(0.0, 10.0), (1.0, 10.0), (2.0, 10.0)];

const C3_NF: [f64; 9] = [
    8.941431489720e-03,
    -2.907193315935e-03,
    -1.568246629107e-03,
    -2.907193315935e-03,
    9.611612264845e-03,
    -2.907193315935e-03,
    -1.568246629107e-03,
    -2.907193315935e-03,
    8.941431489720e-03,
];

#[test]
fn deri_full_3cond() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, DERI);
    let z_ref = [
        (3.525947626277e-04, 9.150978496084e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (5.807470316267e-05, 4.633520928126e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (3.525947626277e-04, 9.150978496084e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (5.807470316267e-05, 4.633520928126e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (3.525947626277e-04, 9.150978496084e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &C3_NF);
}

#[test]
fn simple_carson_full_3cond() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, SIMPLE_CARSON);
    let z_ref = [
        (3.592176235097e-04, 9.080731425744e-04),
        (5.921762350974e-05, 5.085894441415e-04),
        (5.921762350974e-05, 4.563273805293e-04),
        (5.921762350974e-05, 5.085894441415e-04),
        (3.592176235097e-04, 9.080731425744e-04),
        (5.921762350974e-05, 5.085894441415e-04),
        (5.921762350974e-05, 4.563273805293e-04),
        (5.921762350974e-05, 5.085894441415e-04),
        (3.592176235097e-04, 9.080731425744e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &C3_NF); // capacitance is earth-model independent
}

#[test]
fn full_carson_full_3cond() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, FULL_CARSON);
    let z_ref = [
        (3.577509712142e-04, 9.096497377052e-04),
        (5.775083581973e-05, 5.101660729647e-04),
        (5.775042974772e-05, 4.579041104292e-04),
        (5.775083581973e-05, 5.101660729647e-04),
        (3.577509712142e-04, 9.096497377052e-04),
        (5.775083581973e-05, 5.101660729647e-04),
        (5.775042974772e-05, 4.579041104292e-04),
        (5.775083581973e-05, 5.101660729647e-04),
        (3.577509712142e-04, 9.096497377052e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &C3_NF);
}

#[test]
fn deri_reduce_4cond_to_3() {
    // 3 phases + 1 neutral at (1, 12); reduce out the neutral (Kron).
    let coords = [(0.0, 10.0), (1.0, 10.0), (2.0, 10.0), (1.0, 12.0)];
    let mut lc = build(&coords);
    lc.set_nphases(3);
    lc.calc(60.0, DERI);
    lc.reduce();

    // After Reduce, the public z_matrix/yc_matrix use the reduced 3x3.
    let z = lc.z_matrix(60.0, 1.0, M, DERI);
    let yc = lc.yc_matrix(1.0, M);
    let w = lc.omega();

    let z_ref = [
        (3.770208623296e-04, 7.019407348531e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (8.250080286461e-05, 2.501949780572e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (3.789232335261e-04, 6.942314211641e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (8.250080286461e-05, 2.501949780572e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (3.770208623296e-04, 7.019407348531e-04),
    ];
    let c_ref_nf = [
        9.210099265515e-03,
        -2.642486857248e-03,
        -1.299578853311e-03,
        -2.642486857248e-03,
        9.872415813254e-03,
        -2.642486857248e-03,
        -1.299578853311e-03,
        -2.642486857248e-03,
        9.210099265515e-03,
    ];
    for i in 0..3 {
        for j in 0..3 {
            let (re, im) = z_ref[i * 3 + j];
            assert_close(z.get(i, j).re, re, &format!("Zr[{i}][{j}].re"));
            assert_close(z.get(i, j).im, im, &format!("Zr[{i}][{j}].im"));
            assert_close(
                yc.get(i, j).im / w,
                c_ref_nf[i * 3 + j] * 1e-9,
                &format!("Cr[{i}][{j}]"),
            );
        }
    }
}

#[test]
fn conductors_in_same_space_detects_overlap() {
    let mut lc = LineConstants::new(2);
    lc.set_radius(0, M, 0.5);
    lc.set_radius(1, M, 0.5);
    lc.set_x(0, M, 0.0);
    lc.set_y(0, M, 10.0);
    lc.set_x(1, M, 0.2);
    lc.set_y(1, M, 10.0);
    assert!(lc.conductors_in_same_space().is_some());

    // a zero-height conductor is also flagged
    let mut lc2 = LineConstants::new(1);
    lc2.set_radius(0, M, 0.01);
    lc2.set_y(0, M, 0.0);
    assert!(lc2.conductors_in_same_space().is_some());
}
