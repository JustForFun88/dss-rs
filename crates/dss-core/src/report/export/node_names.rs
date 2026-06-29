//! `Export NodeNames` (Pascal `ExportResults.pas` `ExportNodeNames`): one
//! `BusName.NodeNum` per line, in bus order then this-bus node order. No solve
//! dependency.

use crate::circuit::Circuit;

/// Build the `Export NodeNames` body (Pascal `ExportNodeNames`).
///
/// Unlike most reports this one does **not** `AnsiUpperCase` the bus name. The
/// `BusList` (a `THashList`, Pascal `HashList.pas:224,241`) lowercases names on
/// store, so the oracle itself emits the lowercased name here — the port's
/// lowercase output is byte-identical to the oracle's, not merely
/// case-insensitively equal.
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
