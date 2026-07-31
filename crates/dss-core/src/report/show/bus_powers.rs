//! `Show busflow <bus>` (Pascal `ShowResults.pas` `ShowBusPowers`): the power flow
//! around one bus. Two forms (like `Show Powers`):
//! - `ShowOptionCode = 0` — the symmetrical-component **currents** and **powers** by
//!   circuit element connected to the bus (seq voltages header + per-element seq I,
//!   then per-element seq P), for elements whose terminal touches the bus;
//! - `ShowOptionCode = 1` — the per-terminal branch **currents** and **power flow**
//!   into each element from the bus (node voltages, then `WriteTerminalCurrents` /
//!   `WriteTerminalPower`).
//!
//! Every element is filtered by [`check_bus_reference`] (does any terminal connect
//! to the queried bus). Reuses the shared per-bus / per-element helpers extracted
//! from `voltages`/`currents`/`powers`.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};
use crate::support::mathutil::power_factor;

use super::currents::{get_i0i1i2, seq_currents_row, write_terminal_currents};
use super::powers::terminal_power_seq_row;
use super::voltages::{bus_voltage_block, seq_voltage_row};

/// Pascal `CheckBusReference`: the 1-based terminal of `elem` connected to bus index
/// `bus_idx`, or `None`. Matches Pascal's `Terminals[i-1].BusRef = BusReference`.
fn check_bus_reference(elem: &dyn CktElement, bus_idx: usize) -> Option<usize> {
    let cd = elem.cd();
    (0..cd.nterms).find_map(|i| (cd.terminals[i].bus_ref == Some(bus_idx)).then_some(i + 1))
}

/// Build the `Show busflow` text. `bus_idx` is the 0-based bus index (already
/// resolved; the dispatcher raises #219 for an unknown bus), `bus_name` the raw
/// name, `opt` = 0 kVA / 1 MVA, `code` = 0 seq / 1 element.
#[allow(clippy::too_many_arguments)]
pub(crate) fn show_bus_powers(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    bus_idx: usize,
    opt: i32,
    code: i32,
) -> String {
    let mbnl = super::max_bus_name_length(ckt);
    let mdnl = crate::compat::max_device_name_length(super::device_name_width(classes, ckt));
    if code == 0 {
        show_bus_powers_seq(classes, ckt, sys, node_v, bus_idx, opt, mbnl, mdnl)
    } else {
        show_bus_powers_elem(classes, ckt, sys, node_v, bus_idx, opt, mbnl, mdnl)
    }
}

/// `ShowOptionCode = 0`: seq voltages header + per-element seq currents (all
/// terminals of matched elements) + per-element seq powers (the matched terminal).
#[allow(clippy::too_many_arguments)]
fn show_bus_powers_seq(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    bus_idx: usize,
    opt: i32,
    mbnl: usize,
    mdnl: usize,
) -> String {
    let mut rep = Report::new();

    // Bus voltage (seq form, always LN).
    rep.blank();
    rep.line("Bus      V1 (kV)    p.u.    V2 (kV)      %V2/V1    V0 (kV)  %V0/V1");
    rep.blank();
    rep.row(seq_voltage_row(ckt, bus_idx, false, mbnl));

    // Sequence currents (all terminals of every matched element).
    rep.blank();
    rep.line("SYMMETRICAL COMPONENT CURRENTS BY CIRCUIT ELEMENT (first 3 phases)");
    rep.blank();
    rep.line(
        "Element                Term      I1         I2       %I2/I1       I0      %I0/I1   %Normal %Emergency",
    );
    rep.blank();

    for refs in [&ckt.sources, &ckt.pd_elements, &ckt.pc_elements] {
        for_each_enabled_elem(classes, refs, |name, elem| {
            write_seq_current_rows(&mut rep, name, elem, bus_idx, sys, node_v, mdnl);
        });
    }

    // Sequence powers (the matched terminal only).
    rep.blank();
    rep.line("SYMMETRICAL COMPONENT POWERS BY CIRCUIT ELEMENT (first 3 phases)");
    rep.blank();
    if opt == 1 {
        rep.line("Element                      Term    P1(MW)   Q1(Mvar)       P2         Q2      P0      Q0   ");
    } else {
        rep.line("Element                      Term    P1(kW)   Q1(kvar)         P2         Q2      P0      Q0  ");
    }
    rep.blank();

    for refs in [&ckt.sources, &ckt.pd_elements, &ckt.pc_elements] {
        for_each_enabled_elem(classes, refs, |name, elem| {
            if let Some(j) = check_bus_reference(elem, bus_idx) {
                rep.row(terminal_power_seq_row(
                    name, elem, sys, node_v, j, opt, mdnl,
                ));
            }
        });
    }
    rep.finish()
}

/// One matched element's seq-current rows (all terminals) for [`show_bus_powers_seq`]
/// (Pascal's `for j:=1 to NTerm do WriteSeqCurrents(…, 0, 0, j, …)` inner body).
#[allow(clippy::too_many_arguments)]
fn write_seq_current_rows(
    rep: &mut Report,
    name: &str,
    elem: &mut dyn CktElement,
    bus_idx: usize,
    sys: &SysCtx,
    node_v: &[Complex64],
    mdnl: usize,
) {
    if check_bus_reference(elem, bus_idx).is_none() {
        return;
    }
    elem.compute_iterminal(sys, node_v);
    let (nterm, nphases) = (elem.cd().nterms, elem.cd().nphases);
    let is_cap = name
        .split('.')
        .next()
        .is_some_and(|c| c.eq_ignore_ascii_case("capacitor"));
    // `Paddots(EncloseQuotes(FullName), MaxDeviceNameLength+2)` (native case).
    let quoted = format::enclose_quotes(name);
    let cd = elem.cd();
    for jj in 1..=nterm {
        let (i0, i1, i2, cmax) = get_i0i1i2(cd.term_i(jj - 1), nphases);
        // Pascal passes NormAmps = EmergAmps = 0 here (no overload columns).
        rep.row(seq_currents_row(
            &quoted,
            mdnl + 2,
            i0,
            i1,
            i2,
            cmax,
            0.0,
            0.0,
            jj,
            is_cap,
        ));
    }
}

/// `ShowOptionCode = 1`: node voltages + per-terminal branch currents (PD residual)
/// + per-terminal branch power flow into each matched element.
#[allow(clippy::too_many_arguments)]
fn show_bus_powers_elem(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    bus_idx: usize,
    opt: i32,
    mbnl: usize,
    mdnl: usize,
) -> String {
    let mut rep = Report::new();

    // Bus node voltages.
    rep.blank();
    rep.line("  Bus   (node ref)  Node       V (kV)    Angle    p.u.   Base kV");
    rep.blank();
    for row in bus_voltage_block(ckt, bus_idx, false, mbnl) {
        rep.row(row);
    }

    // Element currents into the bus.
    rep.blank();
    rep.line("CIRCUIT ELEMENT CURRENTS");
    rep.blank();
    rep.line("(Currents into element from indicated bus)");
    rep.blank();
    rep.line("Power Delivery Elements");
    rep.blank();
    rep.line("  Bus         Phase    Magnitude, A     Angle      (Real)   +j  (Imag)");
    rep.blank();

    // Sources (no residual) → PDElements (residual), each followed by a blank line.
    for_each_enabled_elem(classes, &ckt.sources, |n, e| {
        matched_terminal_currents(&mut rep, ckt, n, e, bus_idx, sys, node_v, mbnl, false);
    });
    for_each_enabled_elem(classes, &ckt.pd_elements, |n, e| {
        matched_terminal_currents(&mut rep, ckt, n, e, bus_idx, sys, node_v, mbnl, true);
    });

    rep.line("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =");
    rep.blank();
    rep.line("Power Conversion Elements");
    rep.blank();
    rep.line("  Bus         Phase    Magnitude, A     Angle      (Real)   +j  (Imag)");
    rep.blank();
    for_each_enabled_elem(classes, &ckt.pc_elements, |n, e| {
        matched_terminal_currents(&mut rep, ckt, n, e, bus_idx, sys, node_v, mbnl, false);
    });
    for_each_enabled_elem(classes, &ckt.faults, |n, e| {
        matched_terminal_currents(&mut rep, ckt, n, e, bus_idx, sys, node_v, mbnl, false);
    });

    // Branch power flow.
    rep.blank();
    rep.line("CIRCUIT ELEMENT POWER FLOW");
    rep.blank();
    rep.line("(Power Flow into element from indicated Bus)");
    rep.blank();
    if opt == 1 {
        rep.line("  Bus       Phase     MW     +j   Mvar           MVA           PF");
    } else {
        rep.line("  Bus       Phase     kW     +j   kvar           kVA           PF");
    }
    rep.blank();

    // Sources: WriteTerminalPower at the matched terminal, then a blank line.
    for_each_enabled_elem(classes, &ckt.sources, |name, elem| {
        if let Some(j) = check_bus_reference(elem, bus_idx) {
            write_terminal_power(&mut rep, ckt, name, elem, j, opt, sys, node_v, mdnl);
            rep.blank();
        }
    });
    // PDElements: the matched terminal, then every OTHER terminal after a `------------`.
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        if let Some(jterm) = check_bus_reference(elem, bus_idx) {
            let nterm = elem.cd().nterms;
            write_terminal_power(&mut rep, ckt, name, elem, jterm, opt, sys, node_v, mdnl);
            for j in 1..=nterm {
                if j != jterm {
                    rep.row(Row::new().cell(Cell::plain("------------")));
                    write_terminal_power(&mut rep, ckt, name, elem, j, opt, sys, node_v, mdnl);
                }
            }
        }
    });

    rep.line("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =");
    rep.blank();
    rep.line("Power Conversion Elements");
    rep.blank();
    if opt == 1 {
        rep.line("  Bus         Phase     MW   +j  Mvar         MVA         PF");
    } else {
        rep.line("  Bus         Phase     kW   +j  kvar         kVA         PF");
    }
    rep.blank();
    for_each_enabled_elem(classes, &ckt.pc_elements, |name, elem| {
        if let Some(jterm) = check_bus_reference(elem, bus_idx) {
            write_terminal_power(&mut rep, ckt, name, elem, jterm, opt, sys, node_v, mdnl);
            rep.blank();
        }
    });
    rep.finish()
}

/// One element's per-conductor terminal power flow (Pascal `WriteTerminalPower`,
/// `ShowResults.pas:1443`): `ELEMENT = <name>` then, per conductor of terminal
/// `jterm`, `<FromBus> <node> <kW> +j <kvar>  <kVA>  <PF>` (`×3` for a pos-seq
/// model, `×0.001` for MVA), plus a ` TERMINAL TOTAL` line. `FromBus` is padded to
/// 12 and uppercased.
#[allow(clippy::too_many_arguments)]
fn write_terminal_power(
    rep: &mut Report,
    ckt: &Circuit,
    name: &str,
    elem: &mut dyn CktElement,
    jterm: usize,
    opt: i32,
    sys: &SysCtx,
    node_v: &[Complex64],
    mdnl: usize,
) {
    elem.compute_iterminal(sys, node_v);
    let cd = elem.cd();
    let from_bus = cd.terminals[jterm - 1]
        .bus_ref
        .and_then(|b| ckt.buses.get(b))
        .map(|b| b.name.as_str())
        .unwrap_or("");
    let from_bus = from_bus.to_ascii_uppercase();
    rep.line(&format!(
        "ELEMENT = {}",
        format::pad(&format::enclose_quotes(name), mdnl + 2)
    ));
    // The four power columns shared by the conductor rows and the total row.
    let power_cells = |row: Row, sp: Complex64| {
        row.cell(Cell::right(format::g(sp.re / 1000.0, 5), 10).sep(" "))
            .cell(Cell::plain("+j").sep(" "))
            .cell(Cell::right(format::g(sp.im / 1000.0, 5), 10).sep("    "))
            .cell(Cell::right(format::g(sp.norm() / 1000.0, 5), 10).sep("    "))
            .cell(Cell::right(format::fixed(power_factor(sp), 4), 8))
    };
    let mut saccum = Complex64::ZERO;
    for (&nref, &ci) in cd.term_nodes(jterm - 1).iter().zip(cd.term_i(jterm - 1)) {
        let mut sp = node_v[nref] * ci.conj();
        if sys.positive_sequence {
            sp *= 3.0;
        }
        if opt == 1 {
            sp *= 0.001;
        }
        saccum += sp;
        // `'%s %4d %10.5g +j %10.5g    %10.5g    %8.4f'`.
        let row = Row::new()
            .cell(Cell::left(from_bus.clone(), 12).sep(" "))
            .cell(Cell::right(ckt.map_node_to_bus[nref].node_num.to_string(), 4).sep(" "));
        rep.row(power_cells(row, sp));
    }
    // `' TERMINAL TOTAL   %10.5g +j %10.5g    %10.5g    %8.4f'` — the label spans
    // the bus and node columns, so the row carries an empty cell for the node
    // (width 0: nothing in the parity kernel, an aligned gap in the table one).
    let row = Row::new()
        .cell(Cell::plain(" TERMINAL TOTAL").sep("   "))
        .cell(Cell::plain(""));
    rep.row(power_cells(row, saccum));
}

/// One matched element's `WriteTerminalCurrents` block (+ its trailing blank),
/// gated on [`check_bus_reference`]. Used by [`show_bus_powers_elem`]'s current
/// sections.
#[allow(clippy::too_many_arguments)]
fn matched_terminal_currents(
    rep: &mut Report,
    ckt: &Circuit,
    name: &str,
    elem: &mut dyn CktElement,
    bus_idx: usize,
    sys: &SysCtx,
    node_v: &[Complex64],
    mbnl: usize,
    resid: bool,
) {
    if check_bus_reference(elem, bus_idx).is_some() {
        write_terminal_currents(rep, ckt, name, elem, sys, node_v, mbnl, resid);
        rep.blank();
    }
}
