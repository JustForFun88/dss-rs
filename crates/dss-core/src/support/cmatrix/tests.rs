
use super::*;

fn c(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

fn assert_close(a: Complex64, b: Complex64, tol: f64) {
    assert!((a - b).norm() <= tol, "{a} vs {b}");
}

#[test]
fn set_get_add_and_symmetry() {
    let mut m = CMatrix::new(3);
    m.set(0, 1, c(1.0, 2.0));
    assert_eq!(m.get(0, 1), c(1.0, 2.0));
    assert_eq!(m.get(1, 0), Complex64::ZERO);

    m.add(0, 1, c(1.0, -1.0));
    assert_eq!(m.get(0, 1), c(2.0, 1.0));

    m.set_sym(1, 2, c(5.0, 0.0));
    assert_eq!(m.get(2, 1), c(5.0, 0.0));

    m.add_sym(1, 2, c(1.0, 0.0));
    assert_eq!(m.get(1, 2), c(6.0, 0.0));
    assert_eq!(m.get(2, 1), c(6.0, 0.0));

    // set_sym on the diagonal must not double-apply
    m.set_sym(0, 0, c(9.0, 0.0));
    assert_eq!(m.get(0, 0), c(9.0, 0.0));
}

#[test]
fn is_zero_and_col_row_zero() {
    let mut m = CMatrix::new(2);
    assert!(m.is_zero());
    m.set(1, 0, c(0.0, 1e-300));
    assert!(!m.is_zero()); // exactly-zero test, no epsilon
    assert!(!m.is_col_row_zero(0)); // column 0 has an entry
    assert!(!m.is_col_row_zero(1)); // row 1 has an entry
    m.clear();
    assert!(m.is_col_row_zero(0) && m.is_col_row_zero(1));
}

#[test]
fn mv_mult_matches_hand_computation() {
    // A = [1+j 2; 3 4-j], x = [1; j]
    let mut a = CMatrix::new(2);
    a.set(0, 0, c(1.0, 1.0));
    a.set(0, 1, c(2.0, 0.0));
    a.set(1, 0, c(3.0, 0.0));
    a.set(1, 1, c(4.0, -1.0));
    let x = [c(1.0, 0.0), c(0.0, 1.0)];
    let mut b = [Complex64::ZERO; 2];
    a.mv_mult(&mut b, &x);
    assert_eq!(b[0], c(1.0, 3.0)); // (1+j) + 2j
    assert_eq!(b[1], c(4.0, 4.0)); // 3 + (4-j)j = 3 + 4j + 1 ... = (4, 4)
}

#[test]
fn invert_3x3_against_hand_computed_inverse() {
    // Real test matrix with known inverse:
    // A = [2 0 1; 0 1 0; 1 0 1], A^-1 = [1 0 -1; 0 1 0; -1 0 2]
    let mut a = CMatrix::new(3);
    a.set(0, 0, c(2.0, 0.0));
    a.set(0, 2, c(1.0, 0.0));
    a.set(1, 1, c(1.0, 0.0));
    a.set(2, 0, c(1.0, 0.0));
    a.set(2, 2, c(1.0, 0.0));
    a.invert().unwrap();

    let expected = [[1.0, 0.0, -1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 2.0]];
    for (i, row) in expected.iter().enumerate() {
        for (j, &e) in row.iter().enumerate() {
            assert_close(a.get(i, j), c(e, 0.0), 1e-12);
        }
    }
}

#[test]
fn invert_complex_roundtrip_is_identity() {
    // Invert then multiply by the original: expect identity to 1e-12.
    let mut a = CMatrix::new(3);
    let entries = [(0.0, (4.0, 1.0)), (1.0, (-1.0, 0.5)), (2.0, (0.3, -0.2))];
    for i in 0..3 {
        for j in 0..3 {
            let (_, (re, im)) = entries[(i + j) % 3];
            let diag_boost = if i == j { 5.0 } else { 0.0 };
            a.set(i, j, c(re + diag_boost, im));
        }
    }
    let original = a.clone();
    a.invert().unwrap();
    let prod = a.mtrx_mult(&original).unwrap();
    for i in 0..3 {
        for j in 0..3 {
            let expect = if i == j { c(1.0, 0.0) } else { Complex64::ZERO };
            assert_close(prod.get(i, j), expect, 1e-12);
        }
    }
}

#[test]
fn invert_singular_reports_error() {
    let mut a = CMatrix::new(2);
    a.set(0, 0, c(1.0, 0.0));
    a.set(0, 1, c(2.0, 0.0));
    a.set(1, 0, c(2.0, 0.0));
    a.set(1, 1, c(4.0, 0.0)); // row2 = 2*row1
    assert_eq!(a.invert(), Err(SingularMatrix));
}

#[test]
fn kron_reduction_2x2_to_1x1() {
    // Y = [y11 y12; y21 y22]; eliminating node 1 gives y11 - y12*y21/y22.
    let mut y = CMatrix::new(2);
    y.set(0, 0, c(10.0, -5.0));
    y.set(0, 1, c(-2.0, 1.0));
    y.set(1, 0, c(-2.0, 1.0));
    y.set(1, 1, c(8.0, -4.0));
    let r = y.kron(1).unwrap();
    assert_eq!(r.order(), 1);
    let expected = c(10.0, -5.0) - c(-2.0, 1.0) * c(-2.0, 1.0) / c(8.0, -4.0);
    assert_eq!(r.get(0, 0), expected);
}

#[test]
fn kron_eliminating_middle_row_keeps_outer_structure() {
    // 3x3 identity with a coupling: eliminating an uncoupled middle node
    // must leave the other diagonal entries untouched.
    let mut y = CMatrix::new(3);
    y.set(0, 0, c(2.0, 0.0));
    y.set(1, 1, c(3.0, 0.0));
    y.set(2, 2, c(4.0, 0.0));
    y.set_sym(0, 2, c(-1.0, 0.0));
    let r = y.kron(1).unwrap();
    assert_eq!(r.order(), 2);
    assert_eq!(r.get(0, 0), c(2.0, 0.0));
    assert_eq!(r.get(1, 1), c(4.0, 0.0));
    assert_eq!(r.get(0, 1), c(-1.0, 0.0));
}

#[test]
fn kron_invalid_inputs_return_none() {
    let m = CMatrix::new(1);
    assert!(m.kron(0).is_none()); // order 1
    let m = CMatrix::new(3);
    assert!(m.kron(3).is_none()); // out of range
}

#[test]
fn mtrx_mult_identity_and_mismatch() {
    let mut a = CMatrix::new(2);
    a.set(0, 0, c(1.0, 2.0));
    a.set(0, 1, c(3.0, -1.0));
    a.set(1, 0, c(0.5, 0.0));
    a.set(1, 1, c(-2.0, 4.0));
    let mut eye = CMatrix::new(2);
    eye.set(0, 0, c(1.0, 0.0));
    eye.set(1, 1, c(1.0, 0.0));
    let prod = a.mtrx_mult(&eye).unwrap();
    assert_eq!(prod, a);

    assert!(a.mtrx_mult(&CMatrix::new(3)).is_none());
}

#[test]
fn averages_zero_rows_and_negate() {
    let mut m = CMatrix::new(2);
    m.set(0, 0, c(2.0, 2.0));
    m.set(1, 1, c(4.0, -2.0));
    m.set(0, 1, c(6.0, 0.0));
    m.set(1, 0, c(100.0, 0.0)); // lower triangle: excluded from off-diag avg
    assert_eq!(m.avg_diagonal(), c(3.0, 0.0));
    assert_eq!(m.avg_off_diagonal(), c(6.0, 0.0));

    m.negate();
    assert_eq!(m.get(0, 0), c(-2.0, -2.0));

    m.zero_row(0);
    assert_eq!(m.get(0, 1), Complex64::ZERO);
    assert_ne!(m.get(1, 0), Complex64::ZERO);
    m.zero_col(0);
    assert_eq!(m.get(1, 0), Complex64::ZERO);
}

#[test]
fn copy_from_and_add_from_respect_order() {
    let mut a = CMatrix::new(2);
    a.set(0, 0, c(1.0, 0.0));
    let mut b = CMatrix::new(2);
    b.copy_from(&a);
    assert_eq!(b.get(0, 0), c(1.0, 0.0));
    b.add_from(&a);
    assert_eq!(b.get(0, 0), c(2.0, 0.0));

    // order mismatch: silently ignored, like the Pascal code
    let mut c3 = CMatrix::new(3);
    c3.copy_from(&a);
    assert!(c3.is_zero());
}
