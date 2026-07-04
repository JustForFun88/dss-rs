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

    let mut s = String::new();
    s.push_str("System Y Matrix (Lower Triangle by Columns)\n");
    s.push('\n');
    s.push_str("  Row  Col               G               B\n");
    s.push('\n');
    for ((c, r), v) in m {
        // Pascal `Format('[%4d,%4d] = %13.10g + j%13.10g', [row, col, re, im])`
        // (1-based node indices).
        s.push_str(&format!(
            "[{},{}] = {} + j{}\n",
            format::fixed_w_int(r as i64 + 1, 4),
            format::fixed_w_int(c as i64 + 1, 4),
            format::g_w(v.re, 13, 10),
            format::g_w(v.im, 13, 10),
        ));
    }
    s
}
