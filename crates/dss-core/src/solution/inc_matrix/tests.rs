//! Unit tests for the incidence-matrix helpers that are hand-checkable without a
//! full circuit. The end-to-end `Calc_Inc_Matrix`/`Calc_Inc_Matrix_Org` builds
//! and the five exports are pinned byte-exact against the oracle in
//! `crates/dss-core/tests/inc_matrix_reports.rs`.

use super::*;

#[test]
fn strip_bus_cuts_at_first_dot() {
    assert_eq!(strip_bus("bus1.1.2.3"), "bus1");
    assert_eq!(strip_bus("bus1.0"), "bus1");
    assert_eq!(strip_bus("sourcebus"), "sourcebus");
    assert_eq!(strip_bus(""), "");
}

fn incidence_2x3() -> SparseInt {
    // Two branches over three buses: rows 0 and 1, cols 0/1/2.
    let mut m = SparseInt::new();
    m.insert(0, 0, 1);
    m.insert(0, 1, -1);
    m.insert(1, 1, 1);
    m.insert(1, 2, -1);
    m
}

#[test]
fn get_inc_matrix_row_scans_from_index_one() {
    let m = incidence_2x3();
    // col 1 first appears at data[1] (row 0).
    assert_eq!(get_inc_matrix_row(&m, 1), 0);
    // col 0 only appears at data[0], which the scan (starting at index 1) skips.
    assert_eq!(get_inc_matrix_row(&m, 0), -1);
    // col 2 at data[3] (row 1).
    assert_eq!(get_inc_matrix_row(&m, 2), 1);
    // a column that does not exist.
    assert_eq!(get_inc_matrix_row(&m, 9), -1);
}

#[test]
fn get_inc_matrix_col_returns_link_columns() {
    let m = incidence_2x3();
    // row 1 first appears at data[2] (col 1); data[idx+1] = data[3] col 2.
    let (result, active_cols) = get_inc_matrix_col(&m, 1);
    assert_eq!(result, 1);
    assert_eq!(active_cols, [1, 2]);
    // row 0 at data[1] would be scanned from index 1 → col 1, next col from data[2].
    let (r0, ac0) = get_inc_matrix_col(&m, 0);
    assert_eq!(r0, 1);
    assert_eq!(ac0, [1, 1]);
}
