//! `Show Currents` (Pascal `ShowResults.pas` `ShowCurrents`). Two forms:
//! - `ShowOptionCode = 0` ([`show_currents`], `WriteSeqCurrents` + `GetI0I1I2`) —
//!   the symmetrical-component currents by circuit element (first 3 phases): per
//!   terminal `I1`, `I2`, `%I2/I1`, `I0`, `%I0/I1`, and (non-capacitor terminal 1
//!   only) `%Normal`/`%Emergency`;
//! - `ShowOptionCode = 1` ([`show_currents_elements`], `WriteTerminalCurrents`) —
//!   the per-terminal, per-conductor branch currents (magnitude / angle / real /
//!   imag), Sources + PD + Faults, then PC, with an optional residual row.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::report::table::{Cell, Report, Row};
use crate::support::complexutil::cdang;
use crate::support::mathutil::SymComp;

/// Build the `Show Currents` (code 0) text (Pascal `ShowCurrents` case 0). Walks
/// Sources → PDElements → PCElements → Faults calling the mutating `GetCurrents`
/// (`compute_iterminal`). `do_ratings` (PD only) enables the `%Normal`/
/// `%Emergency` columns.
pub(crate) fn show_currents(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    let mdnl = super::device_name_width(classes, ckt);

    let mut rep = Report::new();
    rep.blank();
    rep.line("SYMMETRICAL COMPONENT CURRENTS BY CIRCUIT ELEMENT (first 3 phases)");
    rep.blank();
    // The header, as the nine columns the rows draw.
    rep.row(
        Row::new()
            .cell(Cell::left("Element", mdnl + 2).sep(" "))
            .cell(Cell::plain("Term").sep("      "))
            .cell(Cell::plain("I1").sep("         "))
            .cell(Cell::plain("I2").sep("         "))
            .cell(Cell::plain("%I2/I1").sep("    "))
            .cell(Cell::plain("I0").sep("         "))
            .cell(Cell::plain("%I0/I1").sep("   "))
            .cell(Cell::plain("%Normal").sep(" "))
            .cell(Cell::plain("%Emergency")),
    );
    rep.row(Row::blank(9));

    let sc = SymComp::default();

    let mut calc = |name: &str, elem: &mut dyn CktElement, do_ratings: bool| {
        elem.compute_iterminal(sys, node_v);
        let nterm = elem.cd().nterms;
        let nphases = elem.cd().nphases;
        let norm_amps = elem.norm_amps();
        let emerg_amps = elem.emerg_amps();
        // Pascal excludes capacitors from the overload columns (`CLASSMASK <>
        // CAP_ELEMENT`) — match by class name.
        let is_cap = name
            .split('.')
            .next()
            .is_some_and(|c| c.eq_ignore_ascii_case("capacitor"));
        // The quoted, UPPERCASED full name in a `PadDots` field
        // (`WriteSeqCurrents` writes `AnsiUpperCase(Name)`); a `-` continuation
        // for terminals > 1, whose width is Pascal `Pad('   -',
        // Length(PaddedBrName))` — the **byte** length of the *un-uppercased*
        // padded name (`str::len`).
        let quoted = format::enclose_quotes(name);
        let cd = elem.cd();

        for j in 1..=nterm {
            // `GetI0I1I2`: Cmax = max phase magnitude (>=3 phases) then sym-comp;
            // <3 phases → I1 = |first-phase current| UNCONDITIONALLY (Cmax = I1).
            let it = cd.term_i(j - 1);
            let (i0, i1, i2, cmax) = if nphases >= 3 {
                let iph = [it[0], it[1], it[2]];
                let cmax = iph.iter().map(|c| c.norm()).fold(0.0, f64::max);
                let mut i012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&iph, &mut i012);
                (i012[0].norm(), i012[1].norm(), i012[2].norm(), cmax)
            } else {
                // Show's <3-phase branch is NOT `PositiveSequence`-gated (unlike
                // `ExportSeqCurrents`): I1 = |first-phase current| unconditionally.
                let i1 = it[0].norm();
                (0.0, i1, 0.0, i1)
            };

            let (i2i1, i0i1) = if i1 > 0.0 {
                (100.0 * i2 / i1, 100.0 * i0 / i1)
            } else {
                (0.0, 0.0)
            };
            // `%Normal`/`%Emergency` = `Cmax / rating * 100`, non-capacitor +
            // terminal 1 only, and only when the rating is `> 0` (else 0).
            let (inormal, iemerg) = if do_ratings && !is_cap && j == 1 {
                let n = if norm_amps > 0.0 {
                    cmax / norm_amps * 100.0
                } else {
                    0.0
                };
                let e = if emerg_amps > 0.0 {
                    cmax / emerg_amps * 100.0
                } else {
                    0.0
                };
                (n, e)
            } else {
                (0.0, 0.0)
            };

            // `'%s %3d  %10.5g   %10.5g %8.2f  %10.5g %8.2f  %8.2f %8.2f'` — note
            // the literal space between the `%s` name and the `%3d` terminal.
            rep.row(seq_current_row(
                name_cell(&quoted, mdnl + 2, j),
                i0,
                i1,
                i2,
                i2i1,
                i0i1,
                inormal,
                iemerg,
                j,
            ));
        }
    };

    for_each_enabled_elem(classes, &ckt.sources, |n, e| calc(n, e, false));
    for_each_enabled_elem(classes, &ckt.pd_elements, |n, e| calc(n, e, true));
    for_each_enabled_elem(classes, &ckt.pc_elements, |n, e| calc(n, e, false));
    for_each_enabled_elem(classes, &ckt.faults, |n, e| calc(n, e, false));
    rep.finish()
}

/// The name column of a symmetrical-component current row: the UPPERCASED quoted
/// full name in a `PadDots(…, width)` field on terminal 1, the `Pad('   -',
/// Length(PaddedBrName))` continuation after it (Pascal `WriteSeqCurrents`,
/// `ShowResults.pas:542`).
fn name_cell(quoted: &str, width: usize, j: usize) -> Cell {
    if j == 1 {
        Cell::dots(quoted.to_ascii_uppercase(), width)
    } else {
        Cell::left("   -", quoted.len().max(width))
    }
}

/// The shared body of the two symmetrical-component current writers: the nine
/// columns of `'%s %3d  %10.5g   %10.5g %8.2f  %10.5g %8.2f  %8.2f %8.2f'`
/// (`ShowResults.pas:542/…`), with the already-padded name label as its first
/// cell.
#[allow(clippy::too_many_arguments)]
fn seq_current_row(
    label: Cell,
    i0: f64,
    i1: f64,
    i2: f64,
    i2i1: f64,
    i0i1: f64,
    inormal: f64,
    iemerg: f64,
    j: usize,
) -> Row {
    Row::new()
        .cell(label.sep(" "))
        .cell(Cell::right(j.to_string(), 3).sep("  "))
        .cell(Cell::right(format::g(i1, 5), 10).sep("   "))
        .cell(Cell::right(format::g(i2, 5), 10).sep(" "))
        .cell(Cell::right(format::fixed(i2i1, 2), 8).sep("  "))
        .cell(Cell::right(format::g(i0, 5), 10).sep(" "))
        .cell(Cell::right(format::fixed(i0i1, 2), 8).sep("  "))
        .cell(Cell::right(format::fixed(inormal, 2), 8).sep(" "))
        .cell(Cell::right(format::fixed(iemerg, 2), 8))
}

/// Build the `Show Currents` (code 1) text (Pascal `ShowCurrents` case 1 +
/// `WriteTerminalCurrents`): the per-terminal, per-conductor branch currents.
/// Walks Sources → PD → Faults (the PD section), then PC. `show_residual` (PD
/// only) appends the residual (−Σ terminal currents) row per terminal.
pub(crate) fn show_currents_elements(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    show_residual: bool,
) -> String {
    let mbnl = super::max_bus_name_length(ckt);
    // The header's `(Real)`/`(Imag)` labels stand over the `= <re> +j <im>` pair,
    // not over one column each, so it is free text (see `voltages`' note).
    let hdr = |rep: &mut Report| {
        rep.line(&format!(
            "{}{}",
            format::pad("  Bus", mbnl),
            " Phase    Magnitude, A     Angle      (Real)   +j  (Imag)"
        ));
        rep.blank();
    };

    let mut rep = Report::new();
    rep.blank();
    rep.line("CIRCUIT ELEMENT CURRENTS");
    rep.blank();
    rep.line("(Currents into element from indicated bus)");
    rep.blank();
    rep.line("Power Delivery Elements");
    rep.blank();
    hdr(&mut rep);

    // PD section: Sources (no residual) → PDElements (residual per option) →
    // Faults (no residual).
    for_each_enabled_elem(classes, &ckt.sources, |name, elem| {
        write_terminal_currents(&mut rep, ckt, name, elem, sys, node_v, mbnl, false);
    });
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        write_terminal_currents(&mut rep, ckt, name, elem, sys, node_v, mbnl, show_residual);
    });
    for_each_enabled_elem(classes, &ckt.faults, |name, elem| {
        write_terminal_currents(&mut rep, ckt, name, elem, sys, node_v, mbnl, false);
    });

    rep.line("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =");
    rep.blank();
    rep.line("Power Conversion Elements");
    rep.blank();
    hdr(&mut rep);
    for_each_enabled_elem(classes, &ckt.pc_elements, |name, elem| {
        write_terminal_currents(&mut rep, ckt, name, elem, sys, node_v, mbnl, false);
    });
    rep.finish()
}

/// Pascal `GetI0I1I2(I0, I1, I2, Cmax, Nphases, koff, cBuffer)`: over the first
/// `min(3, nphases)` conductors of terminal current slice `term_i`, the sym-comp
/// magnitudes `(I0, I1, I2)` and `Cmax` = the max phase magnitude. For `< 3`
/// phases, `I1 = |first-phase current|` unconditionally (`Cmax = I1`),
/// `I0 = I2 = 0`. Shared by `Show busflow`'s per-element seq-current section.
pub(crate) fn get_i0i1i2(term_i: &[Complex64], nphases: usize) -> (f64, f64, f64, f64) {
    if nphases >= 3 {
        let iph = [term_i[0], term_i[1], term_i[2]];
        let cmax = iph.iter().map(|c| c.norm()).fold(0.0, f64::max);
        let mut i012 = [Complex64::ZERO; 3];
        SymComp::default().phase_to_sym(&iph, &mut i012);
        (i012[0].norm(), i012[1].norm(), i012[2].norm(), cmax)
    } else {
        let i1 = term_i[0].norm();
        (0.0, i1, 0.0, i1)
    }
}

/// One symmetrical-component current row (Pascal `WriteSeqCurrents`,
/// `ShowResults.pas:542`): `I1 I2 %I2/I1 I0 %I0/I1 %Normal %Emergency`. `Show
/// busflow` calls this per matched element+terminal with `norm_amps = emerg_amps =
/// 0` (so the overload columns are `0.00`); the `-` continuation for `j > 1` is
/// handled internally. `quoted_name` is `enclose_quotes(FullName)` (native case;
/// uppercased here, matching Pascal `AnsiUpperCase(Name)`) and `width` the
/// `MaxDeviceNameLength + 2` field it is `PadDots`ed into. `is_cap` gates the
/// overload columns off for capacitors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn seq_currents_row(
    quoted_name: &str,
    width: usize,
    i0: f64,
    i1: f64,
    i2: f64,
    cmax: f64,
    norm_amps: f64,
    emerg_amps: f64,
    j: usize,
    is_cap: bool,
) -> Row {
    let (i2i1, i0i1) = if i1 > 0.0 {
        (100.0 * i2 / i1, 100.0 * i0 / i1)
    } else {
        (0.0, 0.0)
    };
    // Overloads only for non-capacitors and terminal 1.
    let (inormal, iemerg) = if !is_cap && j == 1 {
        (
            if norm_amps > 0.0 {
                cmax / norm_amps * 100.0
            } else {
                0.0
            },
            if emerg_amps > 0.0 {
                cmax / emerg_amps * 100.0
            } else {
                0.0
            },
        )
    } else {
        (0.0, 0.0)
    };
    seq_current_row(
        name_cell(quoted_name, width, j),
        i0,
        i1,
        i2,
        i2i1,
        i0i1,
        inormal,
        iemerg,
        j,
    )
}

/// One element's terminal-current block (Pascal `WriteTerminalCurrents`). Shared by
/// [`show_currents_elements`] and `Show busflow` (the per-bus branch-current form).
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_terminal_currents(
    rep: &mut Report,
    ckt: &Circuit,
    name: &str,
    elem: &mut dyn CktElement,
    sys: &SysCtx,
    node_v: &[Complex64],
    mbnl: usize,
    show_residual: bool,
) {
    elem.compute_iterminal(sys, node_v);
    let (ncond, nterm, nphases) = (elem.cd().nconds, elem.cd().nterms, elem.cd().nphases);
    let cd = elem.cd();
    // Pascal `'ELEMENT = ', EncloseQuotes(FullName)` — the full name, native case.
    rep.line(&format!("ELEMENT = {}", format::enclose_quotes(name)));

    // AutoTrans special case (`ShowResults.pas:604/624`): `Ntimes = Nphases` rows
    // per terminal, and after each terminal the extra `Inc(k, Ntimes)` skips the
    // remaining conductor block (AutoTrans `NConds = 2·Nphases`).
    let autotrans = super::is_autotrans(name);
    let ntimes = if autotrans { nphases } else { ncond };
    let mut k = 0usize;
    for j in 0..nterm {
        // From-bus per terminal (`StripExtension(FirstBus/NextBus)`, uppercased).
        let from_bus = cd.terminals[j]
            .bus_ref
            .and_then(|b| ckt.buses.get(b))
            .map(|b| b.name.as_str())
            .unwrap_or("");
        let from_bus = from_bus.to_ascii_uppercase();
        // The tail of both row shapes: `<mag> /_ <angle> = <re> +j <im>`.
        let tail = |row: Row, eq_sep: &'static str, c: Complex64| {
            row.cell(Cell::right(format::g(c.norm(), 5), 13).sep(" "))
                .cell(Cell::plain("/_").sep(" "))
                .cell(Cell::right(format::fixed(cdang(c), 1), 6).sep(" "))
                .cell(Cell::plain("=").sep(eq_sep))
                .cell(Cell::right(format::g(c.re, 5), 9).sep(" "))
                .cell(Cell::plain("+j").sep(" "))
                .cell(Cell::right(format::g(c.im, 5), 9))
        };
        let mut ctotal = Complex64::ZERO;
        for _ in 0..ntimes {
            let ck = cd.iterminal[k];
            if show_residual {
                ctotal += ck;
            }
            // `'%s  %4d    %13.5g /_ %6.1f =  %9.5g +j %9.5g'`
            // [UpperCase(FromBus), GetNodeNum(NodeRef[k]), Cabs, cdang, re, im].
            let row = Row::new()
                .cell(Cell::left(from_bus.clone(), mbnl).sep("  "))
                .cell(
                    Cell::right(ckt.map_node_to_bus[cd.node_ref[k]].node_num.to_string(), 4)
                        .sep("    "),
                );
            rep.row(tail(row, "  ", ck));
            k += 1;
        }
        if show_residual && nphases > 1 {
            // `CtoPolardeg(-Ctotal)`: mag = |Ctotal|, ang = cdang(-Ctotal).
            let resid = -ctotal;
            // The residual row labels the node column `Resid` and pads the `=`
            // one space wider (Pascal's own literal).
            let row = Row::new()
                .cell(Cell::left(from_bus.clone(), mbnl).sep(" "))
                .cell(Cell::plain("Resid").sep("    "));
            rep.row(tail(row, "   ", resid));
        }
        if j < nterm - 1 {
            rep.row(Row::new().cell(Cell::plain("------------")));
        }
        if autotrans {
            k += ntimes; // Pascal `Inc(k, Ntimes)` — skip the rest of the block.
        }
    }
    rep.blank(); // Pascal writes a blank line after each element.
}
