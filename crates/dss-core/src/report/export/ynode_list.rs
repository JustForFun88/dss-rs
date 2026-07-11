//! `Export YNodeList` (Pascal `ExportResults.pas` `ExportYNodeList`): the node
//! names in Y-matrix (global node) order, one quoted `"BusName.NodeNum"` per
//! line.

use crate::circuit::Circuit;

/// Build the `Export YNodeList` body (Pascal `ExportYNodeList`): walk global
/// nodes `1..=NumNodes` through `MapNodeToBus`.
pub fn export_ynode_list(ckt: &Circuit) -> String {
    let mut s = String::new();
    for i in 1..=ckt.num_nodes {
        let nb = ckt.map_node_to_bus[i];
        // Pascal `Format('"%s.%-d"', [AnsiUpperCase(BusList.NameOfIndex(Busref)), NodeNum])`.
        let bus_name = ckt.bus_list.name(nb.bus_ref).unwrap_or("").to_uppercase();
        s.push_str(&format!("\"{}.{}\"\n", bus_name, nb.node_num));
    }
    s
}
