//! Unit tests for the `Sparse_Math` port. Every expected value is hand-traced
//! against the Pascal loops (COO insertion order, counting-sort transpose,
//! block-walk multiply), not against a re-run of the code under test.

use super::*;
use num_complex::Complex64;

fn c(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

#[test]
fn int_insert_accumulates_then_appends_in_order() {
    let mut m = SparseInt::new();
    m.insert(0, 1, 5); // append [0,1,5]
    m.insert(2, 3, 7); // append [2,3,7]
    m.insert(0, 1, 9); // (0,1) exists -> overwrite value, order unchanged
    assert_eq!(m.data, vec![[0, 1, 9], [2, 3, 7]]);
    assert_eq!(m.nzero(), 2);
    // row/col track the largest index seen, not a count.
    assert_eq!(m.nrows(), 2);
    assert_eq!(m.ncols(), 3);
}

#[test]
fn int_transpose_swaps_and_sorts_by_original_col() {
    // 2 branches x 3 buses incidence rows.
    let mut m = SparseInt::new();
    m.insert(0, 0, 1);
    m.insert(0, 1, -1);
    m.insert(1, 1, 1);
    m.insert(1, 2, -1);
    let t = m.transpose();
    // Counting sort groups by original column (= new row) ascending; ties keep
    // insertion order.
    assert_eq!(t.data, vec![[0, 0, 1], [1, 0, -1], [1, 1, 1], [2, 1, -1]]);
}

#[test]
fn int_multiply_matches_dense_product() {
    // A = [[1,2],[3,4]], B = [[5,6],[7,8]] -> [[19,22],[43,50]].
    // Row blocks are column-sorted, so the block-walk computes the true product.
    let mut a = SparseInt::new();
    a.insert(0, 0, 1);
    a.insert(0, 1, 2);
    a.insert(1, 0, 3);
    a.insert(1, 1, 4);
    let mut b = SparseInt::new();
    b.insert(0, 0, 5);
    b.insert(0, 1, 6);
    b.insert(1, 0, 7);
    b.insert(1, 1, 8);
    let p = a.multiply(&b);
    assert_eq!(p.data, vec![[0, 0, 19], [0, 1, 22], [1, 0, 43], [1, 1, 50]]);
}

#[test]
fn int_multiply_dimension_mismatch_sentinel() {
    // self.col (max col index) must equal b.row (max row index) else the -1 1x1.
    let mut a = SparseInt::new();
    a.insert(0, 5, 1); // col index 5
    let mut b = SparseInt::new();
    b.insert(0, 0, 1); // row index 0
    let p = a.multiply(&b);
    assert_eq!(p.data, vec![[0, 0, -1]]);
}

#[test]
fn int_add_same_cell_sums() {
    let mut a = SparseInt::new();
    a.insert(0, 0, 5);
    let mut b = SparseInt::new();
    b.insert(0, 0, 3);
    let s = a.add(&b);
    assert_eq!(s.data, vec![[0, 0, 8]]);
}

#[test]
fn int_add_dimension_mismatch_sentinel() {
    let mut a = SparseInt::new();
    a.insert(0, 0, 1); // 1x1 (row=0,col=0)
    let mut b = SparseInt::new();
    b.insert(2, 2, 1); // row=2,col=2
    let s = a.add(&b);
    assert_eq!(s.data, vec![[0, 0, -1]]);
}

#[test]
fn int_rank_counts_distinct_column_patterns() {
    // rows: 0:{0}, 1:{0,1}, 2:{1}. row (max index) = 2, so Rank walks i in 0..2
    // only (the last row is not examined — an upstream off-by-one).
    let mut m = SparseInt::new();
    m.insert(0, 0, 1);
    m.insert(1, 0, 1);
    m.insert(1, 1, 1);
    m.insert(2, 1, 1);
    // i=0 -> +1; i=1 has cols {0,1} != row0 {0} -> +1. rank = 2.
    assert_eq!(m.rank(), 2);
}

#[test]
fn int_rank_equal_rows_not_counted() {
    // rows 0 and 1 share the same column set {0}; row (max index) = 2 so i=0,1.
    let mut m = SparseInt::new();
    m.insert(0, 0, 1);
    m.insert(1, 0, 7);
    m.insert(2, 0, 1);
    // i=0 -> +1; i=1 cols {0} equals row0 {0} -> not counted. rank = 1.
    assert_eq!(m.rank(), 1);
}

#[test]
fn complex_insert_accumulates_then_appends() {
    let mut m = SparseComplex::new();
    m.insert(0, 0, c(1.0, 2.0));
    m.insert(1, 1, c(3.0, 4.0));
    m.insert(0, 0, c(9.0, 9.0)); // overwrite
    assert_eq!(m.nzero(), 2);
    assert_eq!(m.cdata[0].value, c(9.0, 9.0));
    assert_eq!(m.cdata[1].value, c(3.0, 4.0));
}

#[test]
fn complex_transpose_conj_swaps_and_conjugates() {
    let mut m = SparseComplex::new();
    m.insert(0, 0, c(1.0, 2.0));
    m.insert(0, 1, c(3.0, -1.0));
    let t = m.transpose_conj();
    assert_eq!(t.cdata[0].row, 0);
    assert_eq!(t.cdata[0].col, 0);
    assert_eq!(t.cdata[0].value, c(1.0, -2.0));
    assert_eq!(t.cdata[1].row, 1);
    assert_eq!(t.cdata[1].col, 0);
    assert_eq!(t.cdata[1].value, c(3.0, 1.0));
}

#[test]
fn complex_transpose_plain_does_not_conjugate() {
    let mut m = SparseComplex::new();
    m.insert(0, 0, c(1.0, 2.0));
    m.insert(0, 1, c(3.0, -1.0));
    let t = m.transpose();
    assert_eq!(t.cdata[0].value, c(1.0, 2.0));
    assert_eq!(t.cdata[1].value, c(3.0, -1.0));
    assert_eq!(t.cdata[1].row, 1);
    assert_eq!(t.cdata[1].col, 0);
}

#[test]
fn complex_multiply_dimension_mismatch_sentinel() {
    let mut a = SparseComplex::new();
    a.insert(0, 5, c(1.0, 1.0));
    let mut b = SparseComplex::new();
    b.insert(0, 0, c(1.0, 1.0));
    let p = a.multiply(&b);
    assert_eq!(p.cdata.len(), 1);
    assert_eq!(p.cdata[0].value, c(-1.0, 0.0));
}

#[test]
fn complex_multiply_computes_product_and_drops_non_biquadrant() {
    // A = [[1, i], [1, 1]], B = [[1, i], [i, 1]] (row blocks column-sorted so the
    // block-walk computes the true product). True product:
    //   (0,0) = 1*1 + i*i   = 0+0i   -> dropped (both parts zero)
    //   (0,1) = 1*i + i*1   = 0+2i   -> dropped (re == 0; the `re<>0 AND im<>0` quirk)
    //   (1,0) = 1*1 + 1*i   = 1+1i   -> kept
    //   (1,1) = 1*i + 1*1   = 1+1i   -> kept
    // Hand-traced against Pascal `Sparse_Math.pas:868` and cross-checked with a
    // faithful sim of the transpose+block-walk.
    let mut a = SparseComplex::new();
    a.insert(0, 0, c(1.0, 0.0));
    a.insert(0, 1, c(0.0, 1.0));
    a.insert(1, 0, c(1.0, 0.0));
    a.insert(1, 1, c(1.0, 0.0));
    let mut b = SparseComplex::new();
    b.insert(0, 0, c(1.0, 0.0));
    b.insert(0, 1, c(0.0, 1.0));
    b.insert(1, 0, c(0.0, 1.0));
    b.insert(1, 1, c(1.0, 0.0));
    let p = a.multiply(&b);
    assert_eq!(p.cdata.len(), 2);
    assert_eq!((p.cdata[0].row, p.cdata[0].col), (1, 0));
    assert_eq!(p.cdata[0].value, c(1.0, 1.0));
    assert_eq!((p.cdata[1].row, p.cdata[1].col), (1, 1));
    assert_eq!(p.cdata[1].value, c(1.0, 1.0));
}

#[test]
fn complex_add_sums_and_drops_non_biquadrant() {
    // A row {(0,0)=1+i, (0,1)=1+i}, B row {(0,0)=-1+3i, (0,1)=2+0i}; same dims.
    //   (0,0): (1+i)+(-1+3i) = 0+4i -> dropped (re == 0; `re<>0 AND im<>0` quirk)
    //   (0,1): (1+i)+(2+0i)  = 3+1i -> kept
    // The merge loop consumes both cells, so the `cdata[apos+1]` tail loops never
    // run. Hand-traced against Pascal `Sparse_Math.pas:654`.
    let mut a = SparseComplex::new();
    a.insert(0, 0, c(1.0, 1.0));
    a.insert(0, 1, c(1.0, 1.0));
    let mut b = SparseComplex::new();
    b.insert(0, 0, c(-1.0, 3.0));
    b.insert(0, 1, c(2.0, 0.0));
    let s = a.add(&b);
    assert_eq!(s.cdata.len(), 1);
    assert_eq!((s.cdata[0].row, s.cdata[0].col), (0, 1));
    assert_eq!(s.cdata[0].value, c(3.0, 1.0));
}

#[test]
fn complex_add_dimension_mismatch_sentinel() {
    let mut a = SparseComplex::new();
    a.insert(0, 0, c(1.0, 1.0)); // 1x1 (row=0,col=0)
    let mut b = SparseComplex::new();
    b.insert(2, 2, c(1.0, 1.0)); // row=2,col=2
    let s = a.add(&b);
    assert_eq!(s.cdata.len(), 1);
    assert_eq!(s.cdata[0].value, c(-1.0, 0.0));
}

#[test]
fn complex_rank_counts_distinct_column_patterns() {
    // `Rank` ignores values (R_equal compares column indices only). rows: 0:{0},
    // 1:{0,1}, 2:{1}. row (max index) = 2, so the walk covers i in 0..2 only (the
    // last row is not examined — the same upstream off-by-one as the int form).
    // i=0 -> +1; i=1 cols {0,1} != row0 {0} -> +1. rank = 2.
    let mut m = SparseComplex::new();
    m.insert(0, 0, c(1.0, 0.0));
    m.insert(1, 0, c(1.0, 0.0));
    m.insert(1, 1, c(1.0, 0.0));
    m.insert(2, 1, c(1.0, 0.0));
    assert_eq!(m.rank(), 2);
}

#[test]
fn reset_empties_the_store() {
    let mut m = SparseInt::new();
    m.insert(0, 0, 1);
    m.insert(1, 1, 2);
    m.reset();
    assert_eq!(m.nzero(), 0);
    assert!(m.data.is_empty());
}
