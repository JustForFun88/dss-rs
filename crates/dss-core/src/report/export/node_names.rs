//! `Export NodeNames` (Pascal `ExportResults.pas` `ExportNodeNames`): one
//! `BusName.NodeNum` per line, in bus order then this-bus node order. No solve
//! dependency.

use crate::circuit::Circuit;

/// Build the `Export NodeNames` body (Pascal `ExportNodeNames`).
///
/// Pascal keeps the bus name in its original case here (no `AnsiUpperCase`); the
/// HashList stores only the lowercased name, so the port emits lowercase. The
/// golden gate compares identifiers case-insensitively (PHASE8_PLAN §2.3), so
/// this is faithful to the gate.
pub fn export_node_names(ckt: &Circuit) -> String {
    let mut s = String::from("Node_Name\n");
    for i in 0..ckt.buses.len() {
        let bus_name = ckt.bus_list.name(i).unwrap_or("");
        let bus = &ckt.buses[i];
        for j in 0..bus.num_nodes_this_bus() {
            // Pascal `Format('%s.%d ', [BusName, GetNum(j)])` — trailing space.
            s.push_str(&format!("{}.{} \n", bus_name, bus.get_num(j)));
        }
    }
    s
}
