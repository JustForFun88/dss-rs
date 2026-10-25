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
fn stamp_two_terminal_block_quadrants_transposed_vs_direct() {
    // A non-symmetric 2×2 primitive so Transposed and Direct differ observably.
    let prim = |i: usize, j: usize| c((i * 10 + j) as f64, 0.0);

    // Transposed: bottom-left at (j+n, i) → block is −primᵀ.
    let mut t = CMatrix::new(4);
    t.stamp_two_terminal_block(2, StampBl::Transposed, prim);
    for i in 0..2 {
        for j in 0..2 {
            let v = prim(i, j);
            assert_eq!(t.get(i, j), v); // top-left
            assert_eq!(t.get(i + 2, j + 2), v); // bottom-right
            assert_eq!(t.get(i, j + 2), -v); // top-right
            assert_eq!(t.get(j + 2, i), -v); // bottom-left transposed
        }
    }

    // Direct: bottom-left at (i+n, j) → block is −prim.
    let mut d = CMatrix::new(4);
    d.stamp_two_terminal_block(2, StampBl::Direct, prim);
    for i in 0..2 {
        for j in 0..2 {
            assert_eq!(d.get(i + 2, j), -prim(i, j)); // bottom-left direct
        }
    }
    // The two conventions disagree exactly on the off-diagonal bottom-left cell.
    assert_ne!(t.get(3, 0), d.get(3, 0));
}

#[test]
fn stamp_two_terminal_diag_only_touches_the_four_cells() {
    // count < off (the VSConverter shape: AC block narrower than the terminal).
    let mut m = CMatrix::new(6); // off = 3, count = 2
    let value = c(2.0, -3.0);
    m.stamp_two_terminal_diag(2, 3, value);
    for i in 0..2 {
        assert_eq!(m.get(i, i), value);
        assert_eq!(m.get(i + 3, i + 3), value);
        assert_eq!(m.get(i, i + 3), -value);
        assert_eq!(m.get(i + 3, i), -value);
    }
    // No cross-phase coupling and nothing stamped for the skipped conductor 2.
    assert_eq!(m.get(0, 1), Complex64::ZERO);
    assert_eq!(m.get(2, 2), Complex64::ZERO);
    assert_eq!(m.get(2, 5), Complex64::ZERO);
}

#[test]
fn stamp_delta_series_accumulates_with_wraparound() {
    // 3-phase delta, nconds = 3: wraps 3→1. Diagonals get +value twice
    // (accumulated), the ring couplings get −value.
    let mut m = CMatrix::new(6);
    let value = c(0.0, 4.0);
    m.stamp_delta_series(3, 3, value);
    for i in 0..3 {
        assert_eq!(m.get(i, i), value + value, "diagonal {i} accumulates twice");
    }
    // Couplings (i,j) for the ring 0-1, 1-2, 2-0.
    assert_eq!(m.get(0, 1), -value);
    assert_eq!(m.get(1, 0), -value);
    assert_eq!(m.get(1, 2), -value);
    assert_eq!(m.get(2, 0), -value);
    // No stamping into the second-terminal quadrant.
    assert_eq!(m.get(3, 3), Complex64::ZERO);
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
fn cdiv_fpc_matches_fpc_smith_not_naive() {
    // FPC `ucomplex` `/` is Smith's overflow-safe abs-ratio division, which the
    // Pascal `TcMatrix.Invert`/`Kron` cross-terms use. It rounds the last bit
    // differently from `num_complex`'s naive `(ac+bd)/(c²+d²)` operator. Both
    // branches of Smith's are pinned here (bits straight from that f64 formula);
    // the naive operator gives the listed *different* bits, so pointing the
    // parity kernel (`compat::cdiv_fpc_impl`) at `/` flips them and fails this
    // test.

    // |den.re| > |den.im| branch.
    let num = c(0.0012433, 0.0012433);
    let den = c(0.0012433, 0.0011);
    let q = crate::compat::cdiv_fpc_impl(num, den);
    assert_eq!(q.re.to_bits(), 0x3ff0ea49fd1e0356);
    assert_eq!(q.im.to_bits(), 0x3fb08cf7c22f85eb);
    let naive = num / den; // num_complex naive operator — 1 ULP off
    assert_eq!(naive.re.to_bits(), 0x3ff0ea49fd1e0357);
    assert_eq!(naive.im.to_bits(), 0x3fb08cf7c22f85ea);

    // |den.re| <= |den.im| branch.
    let den = c(0.0012433, 0.36);
    let q = crate::compat::cdiv_fpc_impl(num, den);
    assert_eq!(q.re.to_bits(), 0x3f6c63aca54cccd3);
    assert_eq!(q.im.to_bits(), 0xbf6c31a5d17ddb1d);
    let naive = num / den;
    assert_eq!(naive.re.to_bits(), 0x3f6c63aca54cccd2);
    assert_eq!(naive.im.to_bits(), 0xbf6c31a5d17ddb1e);
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
fn index_ops_match_get_set_and_column_major_layout() {
    let mut m = CMatrix::new(3);
    m[(0, 1)] = c(1.0, 2.0);
    m[(2, 0)] = c(-3.0, 0.5);
    // Index agrees with get, and both resolve the column-major offset.
    assert_eq!(m[(0, 1)], m.get(0, 1));
    assert_eq!(m[(2, 0)], c(-3.0, 0.5));
    // (row i, col j) lives at values[j*n + i]; here (0,1) -> 1*3 + 0 = 3.
    assert_eq!(m.values()[3], c(1.0, 2.0));
    m[(0, 1)] += c(0.0, -2.0);
    assert_eq!(m[(0, 1)], c(1.0, 0.0));
}

#[test]
fn col_and_row_iterators_walk_the_expected_entries() {
    let mut m = CMatrix::new(3);
    for i in 0..3 {
        for j in 0..3 {
            m.set(i, j, c(i as f64, j as f64));
        }
    }
    // Column 2 is a contiguous span [(0,2),(1,2),(2,2)].
    assert_eq!(m.col(2), &[c(0.0, 2.0), c(1.0, 2.0), c(2.0, 2.0)]);
    // Row 1 walks [(1,0),(1,1),(1,2)] with the column stride.
    let row1: Vec<_> = m.row(1).copied().collect();
    assert_eq!(row1, vec![c(1.0, 0.0), c(1.0, 1.0), c(1.0, 2.0)]);
    // The column iterator yields every column in order.
    let ncols = m.columns().count();
    assert_eq!(ncols, 3);
    for e in m.row_mut(0) {
        *e = Complex64::ZERO;
    }
    assert!(!m.is_col_row_zero(0)); // column 0 still has (1,0),(2,0)
    m.zero_col(0);
    assert!(m.is_col_row_zero(0));
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

/// A lower-triangle list as a symmetric matrix of `R + jX` with `R = X`.
fn sym_rx(lower: &[f64]) -> CMatrix {
    let n = ((((1 + 8 * lower.len()) as f64).sqrt() - 1.0) / 2.0).round() as usize;
    let mut m = CMatrix::new(n);
    let mut k = 0;
    for i in 0..n {
        for j in 0..=i {
            m.set_sym(i, j, c(lower[k], lower[k]));
            k += 1;
        }
    }
    m
}

/// The singular values of a diagonal matrix are the magnitudes of its
/// entries, and a rank-one outer product has one value that is not zero.
#[test]
fn singular_values_of_known_matrices() {
    let mut d = CMatrix::new(3);
    d.set(0, 0, c(0.0, 3.0));
    d.set(1, 1, c(-4.0, 0.0));
    d.set(2, 2, c(1.0, 1.0));
    let sv = d.singular_values().expect("finite");
    assert_eq!(sv.len(), 3);
    for (got, want) in sv.iter().zip([4.0, 3.0, 2.0_f64.sqrt()]) {
        assert!((got - want).abs() <= 8.0 * f64::EPSILON * 4.0, "{sv:?}");
    }
    // u uᵀ (1 + j) with u = (1, 2, 2): one value |u|² √2 = 9 √2.
    let sv = sym_rx(&[1.0, 2.0, 4.0, 2.0, 4.0, 4.0])
        .singular_values()
        .expect("finite");
    assert!((sv[0] - 9.0 * 2.0_f64.sqrt()).abs() <= 1e-13, "{sv:?}");
    assert!(sv[1] <= 1e-14 && sv[2] <= 1e-14, "{sv:?}");
    let mut nan = CMatrix::new(2);
    nan.set(1, 0, c(f64::NAN, 0.0));
    assert_eq!(nan.singular_values(), None);
}

/// A matrix singular as typed is numerically singular even where the
/// decimal rounding of its entries leaves an invertible one, and a matrix
/// whose smallest singular value is far above the rounding is not.
#[test]
fn a_matrix_singular_within_its_rounding_is_numerically_singular() {
    let rank_one = [0.01, 0.07, 0.49, 0.03, 0.21, 0.09];
    let m = sym_rx(&rank_one);
    assert!(
        m.clone().invert().is_ok(),
        "the rounding leaves it invertible"
    );
    assert!(m.is_numerically_singular());
    assert!(CMatrix::new(3).is_numerically_singular(), "the zero matrix");
    let mut held = m.clone();
    for i in 0..3 {
        held.add(i, i, c(1e-12, 1e-12));
    }
    assert!(
        !held.is_numerically_singular(),
        "regularised far above the rounding"
    );
    assert!(!sym_rx(&[10.0, 0.0, 10.0, 0.0, 0.0, 10.0]).is_numerically_singular());
    let mut nan = CMatrix::new(1);
    nan.set(0, 0, c(f64::NAN, 1.0));
    assert!(nan.is_numerically_singular());
}

/// A real symmetric matrix from its lower triangle.
fn sym_real(lower: &[f64]) -> CMatrix {
    let n = ((((1 + 8 * lower.len()) as f64).sqrt() - 1.0) / 2.0).round() as usize;
    let mut m = CMatrix::new(n);
    let mut k = 0;
    for i in 0..n {
        for j in 0..=i {
            m.set_sym(i, j, c(lower[k], 0.0));
            k += 1;
        }
    }
    m
}

/// The test reads the matrix equilibrated to unit row and column maxima, so a
/// matrix that only spans many decades or holds its large entries off its
/// diagonal builds while a dependence of its rows stays refused at any scale.
/// The bound is pinned on both sides of `n ε` on a matrix of unit maxima whose
/// smallest singular value is known.
#[test]
fn the_singular_test_reads_the_matrix_equilibrated_to_unit_maxima() {
    for spread in [
        &[1e-8, 0.0, 1e8][..],
        &[1e-4, 0.0, 1.0, 0.0, 0.0, 1e12],
        &[1e-9, 0.0, 1.0, 0.0, 0.0, 1e9],
        &[1.0, 0.0, 1.0, 0.0, 0.0, 1.5 * f64::EPSILON],
        &[1e-200, 1e-50, 1e200],
        &[1e-20, 0.5, 1e20],
    ] {
        let m = sym_real(spread);
        assert!(!m.is_numerically_singular(), "{spread:?}");
    }
    // A reactance with an inductive and a capacitive mode beside a tiny
    // resistance: its large entries lie off the diagonal.
    for a in [1e-20, 1e-16] {
        let mut m = CMatrix::new(3);
        m.set_sym(0, 0, c(a, 0.0));
        m.set_sym(1, 0, c(0.0, 1.0));
        m.set_sym(1, 1, c(a, 0.0));
        m.set_sym(2, 2, c(1.0, 1.0));
        assert!(!m.is_numerically_singular(), "{a}");
    }
    // A dependence of the rows at any scale: `D (u uᵀ) D`, and the exactly
    // singular integer matrix with its rows far apart in scale.
    let (u, d) = ([0.1, 0.7, 0.3], [1e-6, 1.0, 1e9]);
    let mut lower = Vec::new();
    for i in 0..3 {
        for j in 0..=i {
            lower.push(d[i] * u[i] * u[j] * d[j]);
        }
    }
    assert!(sym_real(&lower).is_numerically_singular(), "{lower:?}");
    assert!(sym_rx(&[1.0, 2.0, 4.0, 3.0, 6.0, 9.0]).is_numerically_singular());
    // A zero diagonal entry scales by its row and column, and a zero row and
    // column is singular.
    assert!(!sym_real(&[0.0, 1e10, 0.0]).is_numerically_singular());
    assert!(!sym_real(&[1e-300, 1e10, 1e-300]).is_numerically_singular());
    assert!(sym_real(&[1.0, 0.0, 0.0]).is_numerically_singular());
    // `[1 b 0 | b 1 0 | 0 0 1]` with `b = 1 - δ` has the singular values
    // `2 - δ`, `1` and `δ`, and `1 - δ` is exact for `δ` a multiple of `ε / 2`:
    // refused at `δ / 2 = n ε / 2`, built at `δ / 2 = 2 n ε`.
    let unit = |delta: f64| sym_real(&[1.0, 1.0 - delta, 1.0, 0.0, 0.0, 1.0]);
    let n_eps = 3.0 * f64::EPSILON;
    assert!(unit(n_eps).is_numerically_singular());
    assert!(!unit(4.0 * n_eps).is_numerically_singular());
}
