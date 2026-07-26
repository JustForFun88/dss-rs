//! `Show Voltages` (Pascal `ShowResults.pas` `ShowVoltages`). Three forms:
//! - `ShowOptionCode = 0` ([`show_voltages`], `WriteSeqVoltages`) — the
//!   symmetrical-component voltages by bus (for 3-phase buses): `V1 (kV)`, p.u.,
//!   `V2 (kV)`, `%V2/V1`, `V0 (kV)`, `%V0/V1`;
//! - `ShowOptionCode = 1` ([`show_voltages_nodes`], `WriteBusVoltages`) — the
//!   line-ground **and** line-line voltages by bus & node, with magnitude / angle
//!   / pu / base kV;
//! - `ShowOptionCode = 2` ([`show_voltages_elements`], `WriteElementVoltages`) —
//!   node-ground voltages organised by circuit element (PD then PC).
//!
//! The `LL` flag (`Show Voltages LL`) reports phase-to-phase: the sym-comp runs
//! over the line-to-line voltages and the p.u. divides by √3.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::CktElement;
use crate::exec::registry::DssClass;
use crate::report::format;
use crate::support::complexutil::cdang;
use crate::support::mathutil::SymComp;
use crate::util::sqrt3;

/// Build the `Show Voltages` (code 0) text (Pascal `ShowVoltages` case 0 +
/// `WriteSeqVoltages`). Read-only over the solved node voltages. `ll` = the
/// line-to-line option.
pub(crate) fn show_voltages(ckt: &Circuit, ll: bool) -> String {
    let mbnl = super::max_bus_name_length(ckt);

    let mut s = String::new();
    s.push('\n');
    if ll {
        s.push_str("SYMMETRICAL COMPONENT PHASE-PHASE VOLTAGES BY BUS (for 3-phase buses)\n");
    } else {
        s.push_str("SYMMETRICAL COMPONENT VOLTAGES BY BUS (for 3-phase buses)\n");
    }
    s.push('\n');
    s.push_str(&format::pad("Bus", mbnl));
    s.push_str("  Mag:   V1 (kV)    p.u.     V2 (kV)   %V2/V1    V0 (kV)    %V0/V1\n");
    s.push('\n');

    for i in 0..ckt.buses.len() {
        s.push_str(&seq_voltage_row(ckt, i, ll, mbnl));
    }
    s
}

/// One bus's symmetrical-component voltage line (Pascal `WriteSeqVoltages(F, i,
/// LL)`): V1(kV)/pu/V2/%V2·V1⁻¹/V0/%V0·V1⁻¹. Shared by [`show_voltages`] (looped
/// over all buses) and `Show busflow` (a single bus). `mbnl` is
/// [`super::max_bus_name_length`].
pub(crate) fn seq_voltage_row(ckt: &Circuit, i: usize, ll: bool, mbnl: usize) -> String {
    let node_v = &ckt.solution.node_v;
    let sc = SymComp::default();
    let bus = &ckt.buses[i];
    let nn = bus.num_nodes_this_bus();

    // Pascal `WriteSeqVoltages`: >=3 nodes → sym-comp over named nodes 1/2/3 (or
    // their L-L differences when `LL`); <3 nodes → V1 = |V| of the FIRST node
    // (`GetRef(1)`), V0 = V2 = 0 (note: NOT `PositiveSequence`-gated, unlike
    // `ExportSeqVoltages`).
    let (mut v0, mut v1, mut v2) = if nn >= 3 {
        let vph = [
            node_v[bus.find(1)],
            node_v[bus.find(2)],
            node_v[bus.find(3)],
        ];
        let mut v012 = [Complex64::ZERO; 3];
        if ll {
            let vll = [vph[0] - vph[1], vph[1] - vph[2], vph[2] - vph[0]];
            sc.phase_to_sym(&vll, &mut v012);
        } else {
            sc.phase_to_sym(&vph, &mut v012);
        }
        (v012[0].norm(), v012[1].norm(), v012[2].norm())
    } else {
        (0.0, node_v[bus.get_ref(0)].norm(), 0.0)
    };
    // Convert to kV.
    v1 /= 1000.0;
    v2 /= 1000.0;
    v0 /= 1000.0;

    let mut vpu = if bus.kv_base != 0.0 {
        v1 / bus.kv_base
    } else {
        0.0
    };
    if ll {
        vpu /= sqrt3();
    }
    let (v2v1, v0v1) = if v1 > 0.0 {
        (100.0 * v2 / v1, 100.0 * v0 / v1)
    } else {
        (0.0, 0.0)
    };

    // `Format('%s %9.4g  %9.4g  %9.4g  %9.4g %9.4g %9.4g', [Pad(BusName,…), V1,
    // Vpu, V2, V2V1, V0, V0V1])` — bus name space-padded, not uppercased/quoted.
    let bus_name = ckt.bus_list.name(i).unwrap_or("");
    let mut s = format::pad(bus_name, mbnl);
    s.push_str(&format!(
        " {}  {}  {}  {} {} {}\n",
        format::g_w(v1, 9, 4),
        format::g_w(vpu, 9, 4),
        format::g_w(v2, 9, 4),
        format::g_w(v2v1, 9, 4),
        format::g_w(v0, 9, 4),
        format::g_w(v0v1, 9, 4),
    ));
    s
}

/// Build the `Show Voltages` (code 1) text (Pascal `ShowVoltages` case 1 +
/// `WriteBusVoltages`): line-ground and line-line voltages by bus & node. Each
/// node prints magnitude(kV) / angle / pu / base-kV, and (non-`LL`, multi-node
/// buses) the node-node line-line pair alongside. Read-only over `node_v`.
pub(crate) fn show_voltages_nodes(ckt: &Circuit, ll: bool) -> String {
    let mbnl = super::max_bus_name_length(ckt);

    let mut s = String::new();
    s.push('\n');
    if ll {
        s.push_str("LINE-LINE VOLTAGES BY BUS & NODE\n");
    } else {
        s.push_str("LINE-GROUND and LINE-LINE VOLTAGES BY BUS & NODE\n");
    }
    s.push('\n');
    s.push_str(&format::pad("Bus", mbnl));
    if ll {
        s.push_str(" Node    VLN (kV)   Angle      pu     Base kV \n");
    } else {
        s.push_str(
            " Node    VLN (kV)   Angle      pu     Base kV    Node-Node   VLL (kV)  Angle      pu\n",
        );
    }
    s.push('\n');

    for i in 0..ckt.buses.len() {
        s.push_str(&bus_voltage_block(ckt, i, ll, mbnl));
    }
    s
}

/// One bus's line-ground (+ line-line) node-voltage block (Pascal `WriteBusVoltages(F,
/// i, LL)`). Shared by [`show_voltages_nodes`] (looped) and `Show busflow` (single
/// bus). `mbnl` is [`super::max_bus_name_length`].
pub(crate) fn bus_voltage_block(ckt: &Circuit, i: usize, ll: bool, mbnl: usize) -> String {
    let node_v = &ckt.solution.node_v;
    let mut s = String::new();
    {
        let bus = &ckt.buses[i];
        let nn = bus.num_nodes_this_bus();
        // `jj` is the Pascal 1-based node *number* cursor (advances until it lands
        // on a node present on the bus). `bname` carries the padded bus name for
        // node 1, then the `'   -'` continuation for the rest.
        let mut jj: i32 = 1;
        let mut bname = String::new();
        for j in 0..nn {
            // Advance `jj` to the next present node number (Pascal `repeat FindIdx
            // until >0`); `node_idx` is the 0-based slot on the bus.
            let node_idx = loop {
                let idx = bus.find_idx(jj);
                jj += 1;
                if let Some(idx) = idx {
                    break idx;
                }
            };
            let nref1 = bus.get_ref(node_idx);
            let volts = node_v[nref1];

            // Line-line partner (`jj <= 4`): the next phase number in sequence
            // (`k`, wrapping 4→1). Pascal keeps `kk` (the partner slot, 0 = not
            // found → ground ref) and `VoltsLL = Volts − NodeV[GetRef(kk)]`.
            let mut kk: Option<usize> = None;
            let mut volts_ll = Complex64::ZERO;
            let mut node_name_ll = String::new();
            if jj <= 4 {
                let k = if jj > 3 { 1 } else { jj };
                kk = bus.find_idx(k);
                let nref2 = kk.map(|idx| bus.get_ref(idx)).unwrap_or(0);
                volts_ll = volts - node_v[nref2];
                node_name_ll = format!(
                    "{}-{}",
                    bus.get_num(node_idx),
                    kk.map(|idx| bus.get_num(idx)).unwrap_or(0)
                );
            }

            let vmag = volts.norm() * 0.001;
            let vmag_ll = volts_ll.norm() * 0.001;
            let (vpu, vpu_ll) = if bus.kv_base != 0.0 {
                (vmag / bus.kv_base, vmag_ll / bus.kv_base / sqrt3())
            } else {
                (0.0, 0.0)
            };
            let node_name = format!("{}  ", bus.get_num(node_idx));

            if j == 0 {
                bname = format::pad_dots(ckt.bus_list.name(i).unwrap_or(""), mbnl);
            }

            if ll {
                if kk.is_some() {
                    s.push_str(&format!(
                        "{} {} {} /_ {} {} {}\n",
                        bname.to_uppercase(),
                        node_name_ll,
                        format::g_w(vmag_ll, 10, 5),
                        format::fixed_w(cdang(volts_ll), 6, 1),
                        format::g_w(vpu_ll, 9, 5),
                        format::fixed_w(bus.kv_base * sqrt3(), 9, 3),
                    ));
                    bname = format::pad("   -", mbnl);
                }
            } else {
                s.push_str(&format!(
                    "{} {} {} /_ {} {} {}",
                    bname.to_uppercase(),
                    node_name,
                    format::g_w(vmag, 10, 5),
                    format::fixed_w(cdang(volts), 6, 1),
                    format::g_w(vpu, 9, 5),
                    format::fixed_w(bus.kv_base * sqrt3(), 9, 3),
                ));
                if nn > 1 && kk.is_some() && jj <= 4 {
                    s.push_str(&format!(
                        "        {} {} /_ {} {}",
                        node_name_ll,
                        format::g_w(vmag_ll, 10, 5),
                        format::fixed_w(cdang(volts_ll), 6, 1),
                        format::g_w(vpu_ll, 9, 5),
                    ));
                }
                s.push('\n');
                bname = format::pad("   -", mbnl);
            }
        }
    }
    s
}

/// Build the `Show Voltages` (code 2) text (Pascal `ShowVoltages` case 2 +
/// `WriteElementVoltages`): node-ground voltages by circuit element (Sources +
/// PD, then PC). Read-only over `node_v` (uses each element's `NodeRef` directly,
/// no terminal recompute).
pub(crate) fn show_voltages_elements(classes: &[DssClass], ckt: &Circuit, ll: bool) -> String {
    let mbnl = super::max_bus_name_length(ckt);
    let hdr = |s: &mut String| {
        s.push_str(&format::pad("Bus", mbnl));
        s.push_str(" (node ref)  Phase    Magnitude, kV (pu)    Angle\n");
        s.push('\n');
    };

    let mut s = String::new();
    s.push('\n');
    s.push_str("NODE-GROUND VOLTAGES BY CIRCUIT ELEMENT\n");
    s.push('\n');
    s.push_str("Power Delivery Elements\n");
    s.push('\n');
    hdr(&mut s);

    // SOURCES first, then PDELEMENTS (Pascal's PD section; faults are NON_PCPD so
    // not walked here).
    walk_element_voltages(&mut s, classes, ckt, &ckt.sources, mbnl, ll);
    walk_element_voltages(&mut s, classes, ckt, &ckt.pd_elements, mbnl, ll);

    s.push_str("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =\n");
    s.push('\n');
    s.push_str("Power Conversion Elements\n");
    s.push('\n');
    hdr(&mut s);
    walk_element_voltages(&mut s, classes, ckt, &ckt.pc_elements, mbnl, ll);
    s
}

/// Walk a circuit list, writing each **enabled** element's node-ground voltage
/// block, then a blank line after **every** element (Pascal's `FSWriteln(F)` sits
/// *outside* the `if pElem.Enabled` guard, so a disabled element still emits its
/// separating blank — `ShowResults.pas:452-489`). The blank-after-every-element is
/// invisible to the token/blank-filtering golden gate but reproduced for byte
/// faithfulness (audit-code WP8.4 step 4).
fn walk_element_voltages(
    s: &mut String,
    classes: &[DssClass],
    ckt: &Circuit,
    refs: &[crate::elements::traits::ElemId],
    mbnl: usize,
    ll: bool,
) {
    for &r in refs {
        let class_name = classes[r.class_ord()].props.class_name();
        let obj = &classes[r.class_ord()].arena[r.index()];
        if let Some(elem) = obj.as_ckt_element() {
            if elem.cd().enabled {
                let name = format!("{}.{}", class_name, obj.data().name());
                write_element_voltages(s, ckt, &name, elem, mbnl, ll);
            }
            s.push('\n');
        }
    }
}

/// One element's node-ground voltage block (Pascal `WriteElementVoltages`).
fn write_element_voltages(
    s: &mut String,
    ckt: &Circuit,
    name: &str,
    elem: &dyn CktElement,
    mbnl: usize,
    ll: bool,
) {
    let cd = elem.cd();
    let (ncond, nterm) = (cd.nconds, cd.nterms);
    let node_v = &ckt.solution.node_v;
    // Pascal `'ELEMENT = "' + dssclassname + '.' + AnsiUpperCase(Name) + '"'`.
    s.push_str(&format!(
        "ELEMENT = \"{}\"\n",
        format::upper_elem_name(name)
    ));
    let mut k = 0usize;
    for j in 0..nterm {
        // Terminal bus name (`StripExtension(FirstBus/NextBus)`, uppercased at
        // print). The stored bus name is already the stripped, lowercased name.
        let bus_name = cd.terminals[j]
            .bus_ref
            .and_then(|b| ckt.buses.get(b))
            .map(|b| b.name.as_str())
            .unwrap_or("");
        let bus_name = format::pad(bus_name, mbnl).to_uppercase();
        for _ in 0..ncond {
            let nref = cd.node_ref[k];
            k += 1;
            let volts = node_v[nref];
            let vmag = volts.norm() * 0.001;
            let mut vpu = if nref == 0 {
                0.0
            } else {
                let bref = ckt.map_node_to_bus[nref].bus_ref;
                let kvbase = ckt.buses[bref].kv_base;
                if kvbase != 0.0 { vmag / kvbase } else { 0.0 }
            };
            if ll {
                vpu /= sqrt3();
            }
            // Pascal `'%s  (%3d) %4d    %13.5g (%8.4g) /_ %6.1f'`
            // [UpperCase(BusName), nref, MapNodeToBus[nref].nodenum, Vmag, Vpu, cdang(Volts)].
            s.push_str(&format!(
                "{}  ({}) {}    {} ({}) /_ {}\n",
                bus_name,
                format::fixed_w_int(nref as i64, 3),
                format::fixed_w_int(ckt.map_node_to_bus[nref].node_num as i64, 4),
                format::g_w(vmag, 13, 5),
                format::g_w(vpu, 8, 4),
                format::fixed_w(cdang(volts), 6, 1),
            ));
        }
        if j < nterm - 1 {
            s.push_str("------------\n");
        }
    }
}
