//! `Export YVoltages` (Pascal `ExportResults.pas` `ExportYVoltages`): the node
//! voltage vector `Solution.NodeV[i]` for `i = 1..NumNodes` in Y-matrix
//! (`YNodeOrder`) order, one `re, im` pair per line. No header row.

use crate::circuit::Circuit;
use crate::report::format;

/// Build the `Export YVoltages` body (Pascal `ExportYVoltages`). Pure read of the
/// solved `NodeV`; `%10.6g, %10.6g` per node (the width pad is trimmed by the
/// comparator). Node index 0 (ground) is skipped, matching Pascal's `1..NumNodes`.
pub fn export_y_voltages(ckt: &Circuit) -> String {
    let mut s = String::new();
    for v in &ckt.solution.node_v[1..=ckt.num_nodes] {
        s.push_str(&format!("{}, {}\n", format::g(v.re, 6), format::g(v.im, 6)));
    }
    s
}
