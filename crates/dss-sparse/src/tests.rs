use super::*;

fn c(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

/// 2×2 complex system with a known solution.
#[test]
fn solves_small_dense_system() {
    let mut s = SparseSet::new(2);
    s.add_element(0, 0, c(4.0, 1.0));
    s.add_element(0, 1, c(-1.0, 0.0));
    s.add_element(1, 0, c(-1.0, 0.0));
    s.add_element(1, 1, c(3.0, -0.5));

    // Choose x, compute b = A·x, then recover x.
    let x_true = [c(1.0, 2.0), c(-0.5, 0.25)];
    let b = [
        c(4.0, 1.0) * x_true[0] + c(-1.0, 0.0) * x_true[1],
        c(-1.0, 0.0) * x_true[0] + c(3.0, -0.5) * x_true[1],
    ];
    let mut x = [Complex64::ZERO; 2];
    s.solve(&b, &mut x).unwrap();

    for (got, want) in x.iter().zip(&x_true) {
        assert!((got - want).norm() < 1e-12, "got {got}, want {want}");
    }
}

/// Duplicate triplets must accumulate.
#[test]
fn duplicate_entries_accumulate() {
    let mut s = SparseSet::new(1);
    s.add_element(0, 0, c(1.0, 0.0));
    s.add_element(0, 0, c(1.0, 0.0));
    let b = [c(4.0, 0.0)];
    let mut x = [Complex64::ZERO];
    s.solve(&b, &mut x).unwrap();
    assert!((x[0] - c(2.0, 0.0)).norm() < 1e-14);
}

/// A structurally singular matrix (empty column) reports the column.
#[test]
fn singular_matrix_reports_column() {
    let mut s = SparseSet::new(2);
    s.add_element(0, 0, c(1.0, 0.0));
    // column/row 1 left empty -> singular
    let err = s.factor().unwrap_err();
    assert!(matches!(err, SparseError::Singular { .. }), "got {err:?}");
    assert!(s.singular_col().is_some());
}

/// `add_primitive_matrix` skips ground (node 0) and maps 1-based nodes.
#[test]
fn primitive_matrix_skips_ground() {
    // Two-node element between node 1 and ground: only (0,0) is stamped.
    let mut s = SparseSet::new(1);
    let y = c(10.0, -5.0);
    s.add_primitive_matrix(&[1, 0], &[y, -y, -y, y]);
    assert_eq!(s.nnz().unwrap(), 1);
    let b = [y * c(2.0, 0.0)];
    let mut x = [Complex64::ZERO];
    s.solve(&b, &mut x).unwrap();
    assert!((x[0] - c(2.0, 0.0)).norm() < 1e-12);
}

/// Refactoring after zero() + re-stamping works (the per-solve cycle
/// the DSS engine performs when Y changes).
#[test]
fn zero_and_rebuild() {
    let mut s = SparseSet::new(1);
    s.add_element(0, 0, c(2.0, 0.0));
    let mut x = [Complex64::ZERO];
    s.solve(&[c(2.0, 0.0)], &mut x).unwrap();
    assert!((x[0] - c(1.0, 0.0)).norm() < 1e-14);

    s.zero();
    s.add_element(0, 0, c(4.0, 0.0));
    s.solve(&[c(2.0, 0.0)], &mut x).unwrap();
    assert!((x[0] - c(0.5, 0.0)).norm() < 1e-14);
}
