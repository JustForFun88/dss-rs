//! `Export YCurrents` (Pascal `ExportResults.pas` `ExportYCurrents`): the node
//! injection-current vector `Solution.Currents[i]` for `i = 1..NumNodes` in
//! Y-matrix (`YNodeOrder`) order, one `re, im` pair per line. No header row.

use crate::circuit::Circuit;
use crate::report::format;

/// Build the `Export YCurrents` body (Pascal `ExportYCurrents`). Pure read of the
/// solved node-current array (`Solution.Currents`, the same `Currents[0..num_nodes]`
/// the power-flow fills); `%10.6g, %10.6g` per node, ground (index 0) skipped.
pub fn export_y_currents(ckt: &Circuit) -> String {
    let mut s = String::new();
    for c in &ckt.solution.currents[1..=ckt.num_nodes] {
        s.push_str(&format!("{}, {}\n", format::g(c.re, 6), format::g(c.im, 6)));
    }
    s
}
