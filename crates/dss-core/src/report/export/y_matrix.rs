//! `Export Y` (Pascal `ExportResults.pas` `ExportY`): the assembled system Y in
//! node order, either the **sparse triplet** form (`Row,Col,G,B`, lower triangle)
//! or the **dense** node-by-node form.
//!
//! Both read the assembled, unfactored system Y as `(n, [(row, col, value)])`
//! 0-based coordinates (`Dss::system_y_csc`, the same matrix the checkpoint /
//! live gates pin entry-by-entry). Pascal factors first (`FactorSparseMatrix`),
//! which only compresses duplicate stamps — the entry *values* are the original
//! `A`, matching our pre-equilibration COO. Row `i` (1-based) is global node `i`
//! via `MapNodeToBus`.

use std::collections::BTreeMap;

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::report::format;

/// Build the `Export Y` body. `n`/`coords` are the assembled system Y from
/// `Dss::system_y_csc` (0-based COO); `triplet` selects the sparse `Row,Col,G,B`
/// form (Pascal `TripletOpt`).
pub fn export_y(
    n: usize,
    coords: &[(usize, usize, Complex64)],
    ckt: &Circuit,
    triplet: bool,
) -> String {
    if triplet {
        export_y_triplet(coords)
    } else {
        export_y_dense(n, coords, ckt)
    }
}

/// The sparse triplet form (Pascal `ExportY` `TripletOpt` branch): `Row,Col,G,B`
/// for every stored entry with `row >= col` (the lower triangle + diagonal),
/// 1-based, in column-major/ascending-row order (the KLU `GetTripletMatrix`
/// order the oracle emits).
fn export_y_triplet(coords: &[(usize, usize, Complex64)]) -> String {
    // Sum duplicate stamps at the same (row, col) so the value is the true matrix
    // entry (KLU compresses; faer may emit split stamps), then keep row >= col.
    let mut m: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    for &(r, c, v) in coords {
        if r >= c {
            *m.entry((c, r)).or_insert(Complex64::ZERO) += v; // key (col, row) => column-major sort
        }
    }
    let mut s = String::from("Row,Col,G,B\n");
    for ((c, r), v) in m {
        // Pascal `Format('%d,%d,%.10g,%.10g', [row, col, re, im])` (1-based).
        s.push_str(&format!(
            "{},{},{},{}\n",
            r + 1,
            c + 1,
            format::g(v.re, 10),
            format::g(v.im, 10)
        ));
    }
    s
}

/// The dense node-by-node form (Pascal `ExportY` else branch): a `NumNodes`
/// header, then one row per node — the quoted `"BUS.NodeNum"` label followed by
/// the full node row as `re, +j im,` cells. Not covered by the CSV golden (the
/// `+j`-prefixed imaginary tokens are not plain numbers); its values are pinned
/// by the checkpoint / live full-Y gates. Ported faithfully.
fn export_y_dense(n: usize, coords: &[(usize, usize, Complex64)], ckt: &Circuit) -> String {
    // (row, col) => summed value, for the per-cell lookup.
    let mut m: BTreeMap<(usize, usize), Complex64> = BTreeMap::new();
    for &(r, c, v) in coords {
        *m.entry((r, c)).or_insert(Complex64::ZERO) += v;
    }
    let mut s = String::new();
    // Pascal `Format('%d, ', [NumNodes])`.
    s.push_str(&format!("{n}, \n"));
    for i in 0..n {
        // Row label: the 1-based global node `i+1` -> bus name + node number.
        let nb = ckt.map_node_to_bus[i + 1];
        let bus_name = ckt.bus_list.name(nb.bus_ref).unwrap_or("").to_uppercase();
        s.push_str(&format!("\"{}.{}\", ", bus_name, nb.node_num));
        for j in 0..n {
            let v = m.get(&(i, j)).copied().unwrap_or(Complex64::ZERO);
            // Pascal `Format('%-13.10g, +j %-13.10g,', [re, im])`.
            s.push_str(&format!(
                "{}, +j {},",
                format::g(v.re, 10),
                format::g(v.im, 10)
            ));
        }
        s.push('\n');
    }
    s
}
