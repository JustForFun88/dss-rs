//! `Export IncMatrix`/`IncMatrixRows`/`IncMatrixCols`/`BusLevels`/`Laplacian`
//! (exports 53–57, Pascal `Common/ExportResults.pas:3310–3399`): the CSV dumps of
//! the branch-to-node incidence matrix, its row/column descriptors, the per-bus
//! level vector, and the Laplacian. All read-only over
//! [`IncMatrixState`](crate::solution::inc_matrix::IncMatrixState) — populated by
//! the `CalcIncMatrix`/`CalcIncMatrix_O`/`CalcLaplacian` commands.
//!
//! The COO matrices dump one `row,col,value` line per stored non-zero, in storage
//! (= insertion) order (Pascal `for i := 0 to NZero - 1`). A never-calculated
//! matrix (Pascal NIL, `None` here) yields a header-only file — the Pascal would
//! access-violate on the NIL, which this port does not reproduce (CLAUDE.md
//! known-bugs rule); every meaningful call runs a `CalcIncMatrix*` first.

use crate::circuit::Circuit;

/// Pascal `ExportIncMatrix`: `Row,Col,Value` per incidence-matrix non-zero.
pub(crate) fn export_inc_matrix(ckt: &Circuit) -> String {
    let mut s = String::from("Row,Col,Value\n");
    if let Some(m) = &ckt.solution.inc_matrix.inc_mat {
        for d in &m.data {
            s.push_str(&format!("{},{},{}\n", d[0], d[1], d[2]));
        }
    }
    s
}

/// Pascal `ExportIncMatrixRows`: one PD-element row name per line.
pub(crate) fn export_inc_matrix_rows(ckt: &Circuit) -> String {
    let mut s = String::from("B2N Incidence Matrix Row Names (PDElements)\n");
    for r in &ckt.solution.inc_matrix.rows {
        s.push_str(r);
        s.push('\n');
    }
    s
}

/// Pascal `ExportIncMatrixCols`: one bus (column) name per line.
pub(crate) fn export_inc_matrix_cols(ckt: &Circuit) -> String {
    let mut s = String::from("B2N Incidence Matrix Column Names (Buses)\n");
    for c in &ckt.solution.inc_matrix.cols {
        s.push_str(c);
        s.push('\n');
    }
    s
}

/// Pascal `ExportBusLevels`: `Bus Name,Bus Level` per column.
pub(crate) fn export_bus_levels(ckt: &Circuit) -> String {
    let st = &ckt.solution.inc_matrix;
    let mut s = String::from(
        "B2N Incidence Matrix Column Names (Buses) and their level within the matrix\n",
    );
    s.push_str("Bus Name,Bus Level\n");
    for (i, c) in st.cols.iter().enumerate() {
        // Pascal indexes `Inc_Mat_levels[i]` in lockstep; `Calc_Inc_Matrix_Org`
        // grows the two arrays together so they are the same length.
        let level = st.levels.get(i).copied().unwrap_or(0);
        s.push_str(&format!("{c},{level}\n"));
    }
    s
}

/// Pascal `ExportLaplacian`: `Row,Col,Value` per Laplacian non-zero.
pub(crate) fn export_laplacian(ckt: &Circuit) -> String {
    let mut s = String::from("Row,Col,Value\n");
    if let Some(m) = &ckt.solution.inc_matrix.laplacian {
        for d in &m.data {
            s.push_str(&format!("{},{},{}\n", d[0], d[1], d[2]));
        }
    }
    s
}
