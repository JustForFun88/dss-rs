//! `Export VoltagesElements` (Pascal `ExportResults.pas` `ExportVoltagesElements`
//! / `WriteElementVoltagesExportFile`): per-element, per-terminal, per-conductor
//! node-voltage magnitude / angle / pu — the element-organized companion to
//! `Export Voltages` (which is organized by bus). Read-only over the solved node
//! voltages (`NodeRef`/`NodeV`), so no terminal recompute.

use crate::circuit::Circuit;
use crate::elements::traits::CktElement;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::complexutil::cdang;
use crate::util::sqrt3;

/// Build the `Export VoltagesElements` body (Pascal `ExportVoltagesElements`).
/// Walks Sources → PD → Faults → PC: Pascal iterates `Sources`/`PDElements`/
/// `PCElements`, and since `TFaultObj` is a PD element, our split `faults` list is
/// folded in after `pd_elements` — the same expansion `NodeOrder`/`Currents` use.
/// Routed through the mutable walk because [`for_each_enabled_elem`] hands out
/// `&mut dyn CktElement`, but the body only reads.
pub(crate) fn export_voltages_elements(classes: &mut [DssClass], ckt: &Circuit) -> String {
    // Pascal: `MaxNumTerminals := 2; MaxNumNodes := 0;` grown over *all*
    // `CktElements` (regardless of `Enabled`) — sets the header width.
    let mut max_num_terminals = 2usize;
    let mut max_num_nodes = 0usize;
    for &r in &ckt.ckt_elements {
        if let Some(elem) = classes[r.cls].arena[r.idx].as_ckt_element() {
            max_num_terminals = max_num_terminals.max(elem.cd().nterms);
            max_num_nodes = max_num_nodes.max(elem.cd().nconds);
        }
    }

    let mut s = String::from("Element,NumTerminals");
    for i in 1..=max_num_terminals {
        s.push_str(&format!(", Terminal{i}"));
        s.push_str(",NumConductors,NPhases,");
        s.push_str("Bus, BasekV");
        for j in 1..=max_num_nodes {
            s.push_str(&format!(
                ", Node{i}_{j}, Magnitude{i}_{j}, Angle{i}_{j}, pu{i}_{j}"
            ));
        }
    }
    s.push('\n');

    let mut write = |name: &str, elem: &mut dyn CktElement| {
        write_element_voltages(&mut s, ckt, name, elem, max_num_nodes);
        s.push('\n');
    };
    for_each_enabled_elem(classes, &ckt.sources, &mut write);
    for_each_enabled_elem(classes, &ckt.pd_elements, &mut write);
    for_each_enabled_elem(classes, &ckt.faults, &mut write);
    for_each_enabled_elem(classes, &ckt.pc_elements, &mut write);
    s
}

/// One element's row (Pascal `WriteElementVoltagesExportFile`): `FullName,NTerm`
/// then, per terminal `j`, the `Terminal,NumConductors,NPhases,Bus` fields, the
/// terminal's `BasekV` (LL = `kVBase·√3`, printed on conductor 1 only), and per
/// conductor the running-index / magnitude(kV) / angle / pu group, zero-filled to
/// `MaxNumNodes`.
fn write_element_voltages(
    s: &mut String,
    ckt: &Circuit,
    name: &str,
    elem: &dyn CktElement,
    max_num_nodes: usize,
) {
    let cd = elem.cd();
    let (ncond, nterm, nphases) = (cd.nconds, cd.nterms, cd.nphases);
    let node_v = &ckt.solution.node_v;
    // `k` is the running conductor index across the whole element (0-based here;
    // Pascal increments it before use, so the printed value is 1-based).
    let mut k = 0usize;
    // Pascal writes `pElem.FullName` (`Class.Name`) unquoted; the comparator
    // matches identifiers case-insensitively (PHASE8_PLAN §2.3).
    s.push_str(&format!("{name},{nterm}"));
    for j in 0..nterm {
        s.push_str(&format!(",{},", j + 1)); // Terminal
        s.push_str(&format!("{ncond},{nphases},")); // NumConductors,NPhases
        // Bus = the terminal's bus name, uppercased (Pascal `StripExtension` of
        // `FirstBus`/`NextBus` + `AnsiUpperCase`).
        let bus_name = ckt
            .buses
            .get(cd.terminals[j].bus_ref)
            .map(|b| b.name.to_uppercase())
            .unwrap_or_default();
        s.push_str(&format!("{bus_name},"));
        for (i, &nref) in cd.term_nodes(j).iter().enumerate() {
            k += 1;
            let volts = node_v[nref];
            let vmag = volts.norm() * 0.001; // kV
            let vpu = if nref == 0 {
                0.0
            } else {
                let bref = ckt.map_node_to_bus[nref].bus_ref;
                let kvbase = ckt.buses[bref].kv_base;
                // BasekV: written for conductor 1 of each terminal, **and only
                // when the node is not ground** — Pascal writes it inside the
                // `nref <> 0` branch under `if (i = 1)`, so a terminal whose first
                // conductor is grounded omits the field (a reproduced quirk).
                if i == 0 {
                    s.push_str(&format::fixed(kvbase * sqrt3(), 3));
                }
                if kvbase != 0.0 { vmag / kvbase } else { 0.0 }
            };
            // Pascal `', %d, %10.6g, %6.3f, %9.5g'` [k, Vmag, cdang(Volts), Vpu].
            s.push_str(&format!(
                ", {}, {}, {}, {}",
                k,
                format::g(vmag, 6),
                format::fixed(cdang(volts), 3),
                format::g(vpu, 5)
            ));
        }
        // Zero-fill the remaining node slots up to MaxNumNodes.
        for _ in ncond..max_num_nodes {
            s.push_str(", 0, 0, 0, 0");
        }
    }
}
