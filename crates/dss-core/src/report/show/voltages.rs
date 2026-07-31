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
use crate::report::table::{Cell, Report, Row};
use crate::support::complexutil::cdang;
use crate::support::mathutil::SymComp;
use crate::util::sqrt3;

/// Build the `Show Voltages` (code 0) text (Pascal `ShowVoltages` case 0 +
/// `WriteSeqVoltages`). Read-only over the solved node voltages. `ll` = the
/// line-to-line option.
pub(crate) fn show_voltages(ckt: &Circuit, ll: bool) -> String {
    let mbnl = super::max_bus_name_length(ckt);

    let mut rep = Report::new();
    rep.blank();
    if ll {
        rep.line("SYMMETRICAL COMPONENT PHASE-PHASE VOLTAGES BY BUS (for 3-phase buses)");
    } else {
        rep.line("SYMMETRICAL COMPONENT VOLTAGES BY BUS (for 3-phase buses)");
    }
    rep.blank();
    // Pascal's header carries a `Mag:` label *between* the bus column and the
    // six value columns, so it has no cell-per-column decomposition (unlike the
    // F.4d reports, whose headers were exactly their columns) — it stays free
    // text until a v2 re-layout (plan §F-FMT step 4). Same for the two other
    // `Show Voltages` forms below.
    rep.line(&format!(
        "{}{}",
        format::pad("Bus", mbnl),
        "  Mag:   V1 (kV)    p.u.     V2 (kV)   %V2/V1    V0 (kV)    %V0/V1"
    ));
    rep.blank();

    for i in 0..ckt.buses.len() {
        rep.row(seq_voltage_row(ckt, i, ll, mbnl));
    }
    rep.finish()
}

/// One bus's symmetrical-component voltage line (Pascal `WriteSeqVoltages(F, i,
/// LL)`): V1(kV)/pu/V2/%V2·V1⁻¹/V0/%V0·V1⁻¹. Shared by [`show_voltages`] (looped
/// over all buses) and `Show busflow` (a single bus). `mbnl` is
/// [`super::max_bus_name_length`].
pub(crate) fn seq_voltage_row(ckt: &Circuit, i: usize, ll: bool, mbnl: usize) -> Row {
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
    let g9 = |v: f64| Cell::right(format::g(v, 4), 9);
    Row::new()
        .cell(Cell::left(bus_name, mbnl).sep(" "))
        .cell(g9(v1).sep("  "))
        .cell(g9(vpu).sep("  "))
        .cell(g9(v2).sep("  "))
        .cell(g9(v2v1).sep(" "))
        .cell(g9(v0).sep(" "))
        .cell(g9(v0v1))
}

/// Build the `Show Voltages` (code 1) text (Pascal `ShowVoltages` case 1 +
/// `WriteBusVoltages`): line-ground and line-line voltages by bus & node. Each
/// node prints magnitude(kV) / angle / pu / base-kV, and (non-`LL`, multi-node
/// buses) the node-node line-line pair alongside. Read-only over `node_v`.
pub(crate) fn show_voltages_nodes(ckt: &Circuit, ll: bool) -> String {
    let mbnl = super::max_bus_name_length(ckt);

    let mut rep = Report::new();
    rep.blank();
    if ll {
        rep.line("LINE-LINE VOLTAGES BY BUS & NODE");
    } else {
        rep.line("LINE-GROUND and LINE-LINE VOLTAGES BY BUS & NODE");
    }
    rep.blank();
    rep.line(&format!(
        "{}{}",
        format::pad("Bus", mbnl),
        if ll {
            " Node    VLN (kV)   Angle      pu     Base kV "
        } else {
            " Node    VLN (kV)   Angle      pu     Base kV    Node-Node   VLL (kV)  Angle      pu"
        }
    ));
    rep.blank();

    for i in 0..ckt.buses.len() {
        for row in bus_voltage_block(ckt, i, ll, mbnl) {
            rep.row(row);
        }
    }
    rep.finish()
}

/// One bus's line-ground (+ line-line) node-voltage block (Pascal `WriteBusVoltages(F,
/// i, LL)`). Shared by [`show_voltages_nodes`] (looped) and `Show busflow` (single
/// bus). `mbnl` is [`super::max_bus_name_length`].
pub(crate) fn bus_voltage_block(ckt: &Circuit, i: usize, ll: bool, mbnl: usize) -> Vec<Row> {
    let node_v = &ckt.solution.node_v;
    let mut rows = Vec::new();
    {
        let bus = &ckt.buses[i];
        let nn = bus.num_nodes_this_bus();
        // `jj` is the Pascal 1-based node *number* cursor (advances until it lands
        // on a node present on the bus). `bname` carries the padded bus name for
        // node 1, then the `'   -'` continuation for the rest.
        let mut jj: i32 = 1;
        // The name column: the `PadDots`ed bus name on the bus's first row, the
        // `'   -'` continuation (space-padded to the same field) on the rest.
        let mut bname = Cell::plain("");
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
            let node_num = bus.get_num(node_idx);

            if j == 0 {
                bname = Cell::dots(
                    ckt.bus_list.name(i).unwrap_or("").to_ascii_uppercase(),
                    mbnl,
                );
            }
            // The continuation label for every row after the bus's first.
            let cont = || Cell::left("   -", mbnl);

            if ll {
                if kk.is_some() {
                    rows.push(
                        Row::new()
                            .cell(bname.clone().sep(" "))
                            .cell(Cell::plain(node_name_ll).sep(" "))
                            .cell(Cell::right(format::g(vmag_ll, 5), 10).sep(" "))
                            .cell(Cell::plain("/_").sep(" "))
                            .cell(Cell::right(format::fixed(cdang(volts_ll), 1), 6).sep(" "))
                            .cell(Cell::right(format::g(vpu_ll, 5), 9).sep(" "))
                            .cell(Cell::right(format::fixed(bus.kv_base * sqrt3(), 3), 9)),
                    );
                    bname = cont();
                }
            } else {
                // `'%s %s %10.5g /_ %6.1f %9.5g %9.3f'`, where the node column is
                // `IntToStr(node) + '  '` — the two trailing spaces are the
                // column's gutter, so they join the separator.
                let mut row = Row::new()
                    .cell(bname.clone().sep(" "))
                    .cell(Cell::plain(node_num.to_string()).sep("   "))
                    .cell(Cell::right(format::g(vmag, 5), 10).sep(" "))
                    .cell(Cell::plain("/_").sep(" "))
                    .cell(Cell::right(format::fixed(cdang(volts), 1), 6).sep(" "))
                    .cell(Cell::right(format::g(vpu, 5), 9).sep(" "));
                let kvb = Cell::right(format::fixed(bus.kv_base * sqrt3(), 3), 9);
                if nn > 1 && kk.is_some() && jj <= 4 {
                    row = row
                        .cell(kvb.sep("        "))
                        .cell(Cell::plain(node_name_ll).sep(" "))
                        .cell(Cell::right(format::g(vmag_ll, 5), 10).sep(" "))
                        .cell(Cell::plain("/_").sep(" "))
                        .cell(Cell::right(format::fixed(cdang(volts_ll), 1), 6).sep(" "))
                        .cell(Cell::right(format::g(vpu_ll, 5), 9));
                } else {
                    row = row.cell(kvb);
                }
                rows.push(row);
                bname = cont();
            }
        }
    }
    rows
}

/// Build the `Show Voltages` (code 2) text (Pascal `ShowVoltages` case 2 +
/// `WriteElementVoltages`): node-ground voltages by circuit element (Sources +
/// PD, then PC). Read-only over `node_v` (uses each element's `NodeRef` directly,
/// no terminal recompute).
pub(crate) fn show_voltages_elements(classes: &[DssClass], ckt: &Circuit, ll: bool) -> String {
    let mbnl = super::max_bus_name_length(ckt);
    let hdr = |rep: &mut Report| {
        rep.line(&format!(
            "{}{}",
            format::pad("Bus", mbnl),
            " (node ref)  Phase    Magnitude, kV (pu)    Angle"
        ));
        rep.blank();
    };

    let mut rep = Report::new();
    rep.blank();
    rep.line("NODE-GROUND VOLTAGES BY CIRCUIT ELEMENT");
    rep.blank();
    rep.line("Power Delivery Elements");
    rep.blank();
    hdr(&mut rep);

    // SOURCES first, then PDELEMENTS (Pascal's PD section; faults are NON_PCPD so
    // not walked here).
    walk_element_voltages(&mut rep, classes, ckt, &ckt.sources, mbnl, ll);
    walk_element_voltages(&mut rep, classes, ckt, &ckt.pd_elements, mbnl, ll);

    rep.line("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =");
    rep.blank();
    rep.line("Power Conversion Elements");
    rep.blank();
    hdr(&mut rep);
    walk_element_voltages(&mut rep, classes, ckt, &ckt.pc_elements, mbnl, ll);
    rep.finish()
}

/// Walk a circuit list, writing each **enabled** element's node-ground voltage
/// block, then a blank line after **every** element (Pascal's `FSWriteln(F)` sits
/// *outside* the `if pElem.Enabled` guard, so a disabled element still emits its
/// separating blank — `ShowResults.pas:452-489`). The blank-after-every-element is
/// invisible to the token/blank-filtering golden gate but reproduced for byte
/// faithfulness (audit-code WP8.4 step 4).
fn walk_element_voltages(
    rep: &mut Report,
    classes: &[DssClass],
    ckt: &Circuit,
    refs: &[crate::elements::traits::ElemId],
    mbnl: usize,
    ll: bool,
) {
    for &r in refs {
        let class_name = classes[r.class_ord()].props.class_name();
        let obj = &classes[r.class_ord()].arena[r.index()];
        if let Some(elem) = classes[r.class_ord()].arena.try_ckt_elem(r.index()) {
            if elem.cd().enabled {
                let name = format!("{}.{}", class_name, obj.data().name());
                write_element_voltages(rep, ckt, &name, elem, mbnl, ll);
            }
            rep.blank();
        }
    }
}

/// One element's node-ground voltage block (Pascal `WriteElementVoltages`).
fn write_element_voltages(
    rep: &mut Report,
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
    rep.line(&format!("ELEMENT = \"{}\"", format::upper_elem_name(name)));
    let mut k = 0usize;
    for j in 0..nterm {
        // Terminal bus name (`StripExtension(FirstBus/NextBus)`, uppercased at
        // print). The stored bus name is already the stripped, lowercased name.
        let bus_name = cd.terminals[j]
            .bus_ref
            .and_then(|b| ckt.buses.get(b))
            .map(|b| b.name.as_str())
            .unwrap_or("");
        let bus_name = bus_name.to_ascii_uppercase();
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
            // The two parenthesised fields keep their bracket **inside** the cell:
            // `(` is written flush against a right-justified number, so splitting
            // it into a cell of its own would move a token in the table kernel.
            rep.row(
                Row::new()
                    .cell(Cell::left(bus_name.clone(), mbnl).sep("  "))
                    .cell(Cell::plain(format!("({:>3})", nref)).sep(" "))
                    .cell(
                        Cell::right(ckt.map_node_to_bus[nref].node_num.to_string(), 4).sep("    "),
                    )
                    .cell(Cell::right(format::g(vmag, 5), 13).sep(" "))
                    .cell(Cell::plain(format!("({:>8})", format::g(vpu, 4))).sep(" "))
                    .cell(Cell::plain("/_").sep(" "))
                    .cell(Cell::right(format::fixed(cdang(volts), 1), 6)),
            );
        }
        if j < nterm - 1 {
            rep.row(Row::new().cell(Cell::plain("------------")));
        }
    }
}
