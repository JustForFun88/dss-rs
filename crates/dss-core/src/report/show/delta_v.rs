//! `Show DeltaV` (Pascal `ShowResults.pas` `ShowDeltaV` + `WriteElementDeltaVoltages`):
//! the voltage **across** each 2-terminal element (Sources, PD, PC) — per conductor,
//! `NodeV[term1] − NodeV[term2]`, its magnitude / percent-of-base / base-kV / angle.

use crate::circuit::Circuit;
use crate::elements::traits::CktElement;
use crate::exec::registry::DssClass;
use crate::report::format;
use crate::support::complexutil::cdang;

/// Build the `Show DeltaV` text (Pascal `ShowDeltaV`). Walks Sources → PD → PC,
/// writing the delta-voltage block for every **enabled 2-terminal** element.
pub(crate) fn show_delta_v(classes: &[DssClass], ckt: &Circuit) -> String {
    let mdnl = super::max_device_name_length(classes, ckt);
    let hdr = |s: &mut String| {
        s.push_str(&format::pad("Element,", mdnl));
        s.push_str(" Conductor,     Volts,   Percent,           kVBase,  Angle\n");
        s.push('\n');
    };

    let mut s = String::new();
    s.push('\n');
    s.push_str("VOLTAGES ACROSS CIRCUIT ELEMENTS WITH 2 TERMINALS\n");
    s.push('\n');
    s.push_str("Source Elements\n");
    s.push('\n');
    hdr(&mut s);
    walk(&mut s, classes, ckt, &ckt.sources, mdnl);

    s.push('\n');
    s.push_str("Power Delivery Elements\n");
    s.push('\n');
    hdr(&mut s);
    walk(&mut s, classes, ckt, &ckt.pd_elements, mdnl);

    s.push_str("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =\n");
    s.push('\n');
    s.push_str("Power Conversion Elements\n");
    s.push('\n');
    hdr(&mut s);
    walk(&mut s, classes, ckt, &ckt.pc_elements, mdnl);
    s
}

/// Walk a circuit list, writing each enabled 2-terminal element's delta-voltage block
/// followed by a blank line (Pascal `if Enabled and (NTerms = 2)` + `FSWriteln`).
fn walk(
    s: &mut String,
    classes: &[DssClass],
    ckt: &Circuit,
    refs: &[crate::elements::traits::ElemRef],
    mdnl: usize,
) {
    for &r in refs {
        let class_name = classes[r.cls].props.class_name();
        let obj = &classes[r.cls].arena[r.idx];
        let Some(elem) = obj.as_ckt_element() else {
            continue;
        };
        if elem.cd().enabled && elem.cd().nterms == 2 {
            let name = format!("{}.{}", class_name, obj.data().name());
            write_element_delta_voltages(s, ckt, &name, elem, mdnl);
            s.push('\n');
        }
    }
}

/// One element's delta-voltage block (Pascal `WriteElementDeltaVoltages`).
fn write_element_delta_voltages(
    s: &mut String,
    ckt: &Circuit,
    name: &str,
    elem: &dyn CktElement,
    mdnl: usize,
) {
    let cd = elem.cd();
    let ncond = cd.nconds;
    let node_v = &ckt.solution.node_v;
    // Pascal `Pad(dssclassname + '.' + AnsiUpperCase(Name), MaxDeviceNameLength)`.
    let elem_name = format::pad(&format::upper_elem_name(name), mdnl);
    // Pascal conductors are 1-based; `Node1 = NodeRef[i]`, `Node2 = NodeRef[i+NCond]`.
    for i in 1..=ncond {
        let n1 = cd.node_ref[i - 1];
        let n2 = cd.node_ref[i - 1 + ncond];
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
        // `'%s,  %4d,    %12.5g, %12.5g, %12.5g, %6.1f'`.
        s.push_str(&format!(
            "{},  {},    {}, {}, {}, {}\n",
            elem_name,
            format::fixed_w_int(i as i64, 4),
            format::g_w(volts1.norm(), 12, 5),
            format::g_w(vmag, 12, 5),
            format::g_w(kv1, 12, 5),
            format::fixed_w(cdang(volts1), 6, 1),
        ));
    }
}
