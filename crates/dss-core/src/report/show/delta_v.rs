//! `Show DeltaV` (Pascal `ShowResults.pas` `ShowDeltaV` + `WriteElementDeltaVoltages`):
//! the voltage **across** each 2-terminal element (Sources, PD, PC) — per conductor,
//! `NodeV[term1] − NodeV[term2]`, its magnitude / percent-of-base / base-kV / angle.

use crate::circuit::Circuit;
use crate::elements::traits::CktElement;
use crate::exec::registry::DssClass;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};
use crate::support::complexutil::cdang;

/// The six columns of a `Show DeltaV` block: Pascal's
/// `Pad('Element,', MaxDeviceNameLength)` followed by the literal
/// `' Conductor,     Volts,   Percent,           kVBase,  Angle'`, whose column
/// offsets within that literal (1/16/25/44/53) are the widths below.
fn header_row(mdnl: usize) -> Row {
    Row::new()
        .cell(Cell::left("Element,", mdnl).sep(" "))
        .cell(Cell::left("Conductor,", 15))
        .cell(Cell::left("Volts,", 9))
        .cell(Cell::left("Percent,", 19))
        .cell(Cell::left("kVBase,", 9))
        .cell(Cell::plain("Angle"))
}

/// Build the `Show DeltaV` text (Pascal `ShowDeltaV`). Walks Sources → PD → PC,
/// writing the delta-voltage block for every **enabled 2-terminal** element.
pub(crate) fn show_delta_v(classes: &[DssClass], ckt: &Circuit) -> String {
    let mdnl = crate::compat::max_device_name_length(super::device_name_width(classes, ckt));

    let mut rep = Report::new();
    rep.blank();
    rep.line("VOLTAGES ACROSS CIRCUIT ELEMENTS WITH 2 TERMINALS");
    rep.blank();
    rep.line("Source Elements");
    rep.blank();
    rep.row(header_row(mdnl));
    rep.row(Row::blank(6));
    walk(&mut rep, classes, ckt, &ckt.sources, mdnl);

    rep.blank();
    rep.line("Power Delivery Elements");
    rep.blank();
    rep.row(header_row(mdnl));
    rep.row(Row::blank(6));
    walk(&mut rep, classes, ckt, &ckt.pd_elements, mdnl);

    rep.line("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =");
    rep.blank();
    rep.line("Power Conversion Elements");
    rep.blank();
    rep.row(header_row(mdnl));
    rep.row(Row::blank(6));
    walk(&mut rep, classes, ckt, &ckt.pc_elements, mdnl);
    rep.finish()
}

/// Walk a circuit list, writing each enabled 2-terminal element's delta-voltage block
/// followed by a blank line (Pascal `if Enabled and (NTerms = 2)` + `FSWriteln`).
fn walk(
    rep: &mut Report,
    classes: &[DssClass],
    ckt: &Circuit,
    refs: &[crate::elements::traits::ElemId],
    mdnl: usize,
) {
    for &r in refs {
        let class_name = classes[r.class_ord()].props.class_name();
        let obj = &classes[r.class_ord()].arena[r.index()];
        let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem(r.index()) else {
            continue;
        };
        if elem.cd().enabled && elem.cd().nterms == 2 {
            let name = format!("{}.{}", class_name, obj.data().name());
            write_element_delta_voltages(rep, ckt, &name, elem, mdnl);
            rep.row(Row::blank(6));
        }
    }
}

/// One element's delta-voltage block (Pascal `WriteElementDeltaVoltages`).
fn write_element_delta_voltages(
    rep: &mut Report,
    ckt: &Circuit,
    name: &str,
    elem: &dyn CktElement,
    mdnl: usize,
) {
    let cd = elem.cd();
    let ncond = cd.nconds;
    let node_v = &ckt.solution.node_v;
    // Pascal `Pad(dssclassname + '.' + AnsiUpperCase(Name), MaxDeviceNameLength)`.
    let elem_name = format::upper_elem_name(name);
    // Pascal conductors are 1-based; `Node1 = NodeRef[i]`, `Node2 = NodeRef[i+NCond]`
    // — the terminal-0 and terminal-1 conductor slices over the flat `node_ref`.
    let (t0_nodes, t1_nodes) = (cd.term_nodes(0), cd.term_nodes(1));
    for i in 1..=ncond {
        let n1 = t0_nodes[i - 1];
        let n2 = t1_nodes[i - 1];
        // Ground node (0) → bus 0 (Pascal `if NodeN > 0 then … else Bus := 0`).
        let bus1 = if n1 > 0 {
            ckt.map_node_to_bus[n1].bus_ref
        } else {
            usize::MAX
        };
        let bus2 = if n2 > 0 {
            ckt.map_node_to_bus[n2].bus_ref
        } else {
            usize::MAX
        };
        // Only conductors bridging two real buses (Pascal `if (Bus1 > 0) and
        // (Bus2 > 0)`; the 0-based port uses `usize::MAX` for the no-bus sentinel).
        if bus1 == usize::MAX || bus2 == usize::MAX {
            continue;
        }
        let volts1 = node_v[n1] - node_v[n2]; // difference voltage
        let (kv1, kv2) = (ckt.buses[bus1].kv_base, ckt.buses[bus2].kv_base);
        let vmag = if kv1 != kv2 {
            0.0
        } else if kv1 > 0.0 {
            volts1.norm() / (1000.0 * kv1) * 100.0
        } else {
            0.0
        };
        // `'%s,  %4d,    %12.5g, %12.5g, %12.5g, %6.1f'` — every comma sits
        // *after* the padded field, so it belongs to the separator (unlike the
        // header, whose `Pad('Element,', …)` pads the comma itself).
        rep.row(
            Row::new()
                .cell(Cell::left(elem_name.as_str(), mdnl).sep(",  "))
                .cell(Cell::right(i.to_string(), 4).sep(",    "))
                .cell(Cell::right(format::g(volts1.norm(), 5), 12).sep(", "))
                .cell(Cell::right(format::g(vmag, 5), 12).sep(", "))
                .cell(Cell::right(format::g(kv1, 5), 12).sep(", "))
                .cell(Cell::right(format::fixed(cdang(volts1), 1), 6)),
        );
    }
}
