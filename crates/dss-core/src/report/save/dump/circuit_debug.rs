//! `TDSSCircuit.DebugDump` (Pascal `Circuit.pas:2285-2319`): the bus / device
//! / node-map header a whole-circuit `Dump debug` writes before the element
//! dumps.

use crate::circuit::Circuit;
use crate::exec::registry::DssClass;
use crate::report::format;

/// Render the debug header. `classes` resolves each device's enabled flag
/// (Pascal walks `CktElements` in step with `DeviceList`).
pub(crate) fn debug_dump(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::new();
    s.push_str(&format!("NumBuses= {}\n", ckt.buses.len()));
    s.push_str(&format!("NumNodes= {}\n", ckt.num_nodes));
    s.push_str(&format!("NumDevices= {}\n", ckt.num_devices));

    s.push_str("BusList:\n");
    for bus in &ckt.buses {
        // `'  ' + Pad(name, 12) + ' (' + n + ' Nodes)'` then ` <num>` per node.
        s.push_str(&format!(
            "  {} ({} Nodes)",
            format::pad(&bus.name, 12),
            bus.num_nodes_this_bus()
        ));
        for &num in &bus.nodes {
            s.push_str(&format!(" {num}"));
        }
        s.push('\n');
    }

    s.push_str("DeviceList:\n");
    for (i, name) in ckt.device_list.iter().enumerate() {
        s.push_str(&format!("  {}", format::pad(name, 12)));
        // Pascal reads `CktElements.Get(i).Enabled` (it also leaves that
        // element active — mirrored by the caller).
        let r = ckt.ckt_elements[i];
        let enabled = classes[r.cls].objects[r.idx]
            .as_ckt_element()
            .map(|e| e.cd().enabled)
            .unwrap_or(true);
        if !enabled {
            s.push_str("  DISABLED");
        }
        s.push('\n');
    }

    s.push_str("NodeToBus Array:\n");
    for i in 1..=ckt.num_nodes {
        let nb = ckt.map_node_to_bus[i];
        // `WriteStr(sout, '  ', i: 2, ' ', j: 2, ' (=', name, '.', num: 0, ')')`
        // — `j` is the 1-based BusRef.
        s.push_str(&format!(
            "  {:>2} {:>2} (={}.{})\n",
            i,
            nb.bus_ref + 1,
            ckt.buses[nb.bus_ref].name,
            nb.node_num
        ));
    }
    s
}
