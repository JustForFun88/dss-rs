//! `Show Y` (Pascal `ShowResults.pas` `ShowY`): the assembled system Y matrix,
//! lower triangle by columns, one `[row,col] = G + jB` line per stored entry.
//!
//! Reads the assembled, unfactored system Y as `(n, [(row, col, value)])` 0-based
//! coordinates (`Dss::system_y_csc`, the same matrix the checkpoint / live gates
//! pin entry-by-entry). Pascal factors first (`FactorSparseMatrix`), which only
//! *compresses* duplicate stamps — the entry values are the original `A`, matching
//! our pre-equilibration COO — then walks the KLU triplets in column-major order
//! and keeps `row >= col`.

use std::collections::BTreeMap;

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};

/// Build the `Show Y` body. `n`/`coords` are the assembled system Y from
/// `Dss::system_y_csc` (0-based COO). `_ckt` is unused (the report is node-index
/// only, no bus labels) but kept for a uniform formatter signature.
pub(crate) fn show_y(_n: usize, coords: &[(usize, usize, Complex64)], _ckt: &Circuit) -> String {
    // Sum duplicate stamps at the same (row, col) — KLU compresses, faer may emit
    // split stamps — keeping only the lower triangle + diagonal (`row >= col`),
    // keyed (col, row) so iteration is column-major/ascending-row, the exact order
    // KLU's `GetTripletMatrix` emits (Pascal `ShowY`).
    let mut m: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    for &(r, c, v) in coords {
        if r >= c {
            *m.entry((c, r)).or_insert(Complex64::ZERO) += v;
        }
    }

    let mut rep = Report::new();
    rep.line("System Y Matrix (Lower Triangle by Columns)");
    rep.blank();
    // `Row`/`Col` name the two numbers *inside* the bracketed cell below, so the
    // header has no cell-per-column form — it stays free text (see
    // `report::show`'s module note).
    rep.line("  Row  Col               G               B");
    rep.blank();
    for ((c, r), v) in m {
        // Pascal `Format('[%4d,%4d] = %13.10g + j%13.10g', [row, col, re, im])`
        // (1-based node indices). The brackets and the `j` are written flush
        // against right-justified numbers, so each stays **inside** its cell
        // (F.4e rule 1): a `[` or `j` column of its own would move a token in
        // the table kernel.
        rep.row(
            Row::new()
                .cell(Cell::plain(format!("[{:>4},{:>4}]", r + 1, c + 1)).sep(" "))
                .cell(Cell::plain("=").sep(" "))
                .cell(Cell::right(format::g(v.re, 10), 13).sep(" "))
                .cell(Cell::plain("+").sep(" "))
                .cell(Cell::plain(format!("j{:>13}", format::g(v.im, 10)))),
        );
    }
    rep.finish()
}
