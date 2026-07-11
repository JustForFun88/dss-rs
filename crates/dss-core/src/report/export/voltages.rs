//! `Export Voltages` (Pascal `ExportResults.pas` `ExportVoltages`): per-bus node
//! voltage magnitude / angle / pu, in bus order, every row zero-filled to the
//! circuit's maximum node count.

use crate::circuit::Circuit;
use crate::report::format;
use crate::support::complexutil::cdang;
use crate::util::sqrt3;

/// Build the `Export Voltages` CSV body (Pascal `ExportVoltages`). Read-only
/// over the solved circuit; the caller writes the returned text to the report
/// file (PHASE8_PLAN §2.1).
pub fn export_voltages(ckt: &Circuit) -> String {
    // `MaxNumNodes := max over buses of NumNodesThisBus` (sets the column count).
    let max_num_nodes = ckt
        .buses
        .iter()
        .map(|b| b.num_nodes_this_bus())
        .max()
        .unwrap_or(0);

    let mut s = String::new();
    // Header: `Bus, BasekV` + one `Node/Magnitude/Angle/pu` group per node slot.
    s.push_str("Bus, BasekV");
    for i in 1..=max_num_nodes {
        s.push_str(&format!(", Node{i}, Magnitude{i}, Angle{i}, pu{i}"));
    }
    s.push('\n');

    let node_v = &ckt.solution.node_v;
    for i in 0..ckt.buses.len() {
        let bus = &ckt.buses[i];
        // Pascal `AnsiUpperCase(BusList.NameOfIndex(i))` (the HashList stores the
        // lowercased name; uppercasing it reproduces the Pascal output).
        let bus_name = ckt.bus_list.name(i).unwrap_or("").to_uppercase();
        // `'"%s", %.5g'` — base kV is line-to-line (`kVBase * SQRT3`).
        s.push_str(&format!(
            "\"{}\", {}",
            bus_name,
            format::g(bus.kv_base * sqrt3(), 5)
        ));

        // Walk this bus's nodes in ascending node-*number* order: Pascal scans
        // `FindIdx(jj)` for jj = 1, 2, 3, … picking up each present node.
        let mut jj: i32 = 1;
        for _ in 0..bus.num_nodes_this_bus() {
            let node_idx = loop {
                let idx = bus.find_idx(jj);
                jj += 1;
                if let Some(li) = idx {
                    break li;
                }
            };
            let nref = bus.get_ref(node_idx);
            let volts = node_v[nref];
            let vmag = volts.norm();
            let vpu = if bus.kv_base != 0.0 {
                0.001 * vmag / bus.kv_base
            } else {
                0.0
            };
            // `', %d, %10.6g, %6.1f, %9.5g'`
            s.push_str(&format!(
                ", {}, {}, {}, {}",
                bus.get_num(node_idx),
                format::g(vmag, 6),
                format::fixed(cdang(volts), 1),
                format::g(vpu, 5)
            ));
        }
        // Zero-fill the remaining node slots up to MaxNumNodes.
        for _ in bus.num_nodes_this_bus()..max_num_nodes {
            s.push_str(", 0, 0, 0, 0");
        }
        s.push('\n');
    }
    s
}
