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
    let mdnl = super::max_device_name_length(classes, ckt);

    let mut s = String::new();
    s.push('\n');
    s.push_str("SYMMETRICAL COMPONENT CURRENTS BY CIRCUIT ELEMENT (first 3 phases)\n");
    s.push('\n');
    s.push_str(&format::pad("Element", mdnl + 2));
    s.push_str(
        " Term      I1         I2         %I2/I1    I0         %I0/I1   %Normal %Emergency\n",
    );
    s.push('\n');

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
        // The padded, quoted, UPPERCASED full name (`WriteSeqCurrents` writes
        // `AnsiUpperCase(Name)`); a `-` continuation for terminals > 1. The
        // continuation width is Pascal `Pad('   -', Length(PaddedBrName))` — the
        // **byte** length of the *un-uppercased* padded name (`str::len`).
        let padded_raw = format::pad_dots(&format::enclose_quotes(name), mdnl + 2);
        let padded = padded_raw.to_uppercase();
        let cont = format::pad("   -", padded_raw.len());
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
            let label = if j == 1 { &padded } else { &cont };
            s.push_str(label);
            s.push(' ');
            s.push_str(&format::fixed_w_int(j as i64, 3));
            s.push_str(&format!(
                "  {}   {} {}  {} {}  {} {}\n",
                format::g_w(i1, 10, 5),
                format::g_w(i2, 10, 5),
                format::fixed_w(i2i1, 8, 2),
                format::g_w(i0, 10, 5),
                format::fixed_w(i0i1, 8, 2),
                format::fixed_w(inormal, 8, 2),
                format::fixed_w(iemerg, 8, 2),
            ));
        }
    };

    for_each_enabled_elem(classes, &ckt.sources, |n, e| calc(n, e, false));
    for_each_enabled_elem(classes, &ckt.pd_elements, |n, e| calc(n, e, true));
    for_each_enabled_elem(classes, &ckt.pc_elements, |n, e| calc(n, e, false));
    for_each_enabled_elem(classes, &ckt.faults, |n, e| calc(n, e, false));
    s
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
    let hdr = |s: &mut String| {
        s.push_str(&format::pad("  Bus", mbnl));
        s.push_str(" Phase    Magnitude, A     Angle      (Real)   +j  (Imag)\n");
        s.push('\n');
    };

    let mut s = String::new();
    s.push('\n');
    s.push_str("CIRCUIT ELEMENT CURRENTS\n");
    s.push('\n');
    s.push_str("(Currents into element from indicated bus)\n");
    s.push('\n');
    s.push_str("Power Delivery Elements\n");
    s.push('\n');
    hdr(&mut s);

    // PD section: Sources (no residual) → PDElements (residual per option) →
    // Faults (no residual).
    for_each_enabled_elem(classes, &ckt.sources, |name, elem| {
        write_terminal_currents(&mut s, ckt, name, elem, sys, node_v, mbnl, false);
    });
    for_each_enabled_elem(classes, &ckt.pd_elements, |name, elem| {
        write_terminal_currents(&mut s, ckt, name, elem, sys, node_v, mbnl, show_residual);
    });
    for_each_enabled_elem(classes, &ckt.faults, |name, elem| {
        write_terminal_currents(&mut s, ckt, name, elem, sys, node_v, mbnl, false);
    });

    s.push_str("= = = = = = = = = = = = = = = = = = =  = = = = = = = = = = =  = =\n");
    s.push('\n');
    s.push_str("Power Conversion Elements\n");
    s.push('\n');
    hdr(&mut s);
    for_each_enabled_elem(classes, &ckt.pc_elements, |name, elem| {
        write_terminal_currents(&mut s, ckt, name, elem, sys, node_v, mbnl, false);
    });
    s
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
/// handled internally. `padded_br_name` is `pad_dots(enclose_quotes(FullName),
/// mdnl+2)` (native case; uppercased here, matching Pascal `AnsiUpperCase(Name)`).
/// `is_cap` gates the overload columns off for capacitors.
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_seq_currents(
    s: &mut String,
    padded_br_name: &str,
    i0: f64,
    i1: f64,
    i2: f64,
    cmax: f64,
    norm_amps: f64,
    emerg_amps: f64,
    j: usize,
    is_cap: bool,
) {
    let name = if j == 1 {
        padded_br_name.to_string()
    } else {
        format::pad("   -", padded_br_name.len())
    };
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
    // `'%s %3d  %10.5g   %10.5g %8.2f  %10.5g %8.2f  %8.2f %8.2f'`.
    s.push_str(&name.to_uppercase());
    s.push(' ');
    s.push_str(&format::fixed_w_int(j as i64, 3));
    s.push_str(&format!(
        "  {}   {} {}  {} {}  {} {}\n",
        format::g_w(i1, 10, 5),
        format::g_w(i2, 10, 5),
        format::fixed_w(i2i1, 8, 2),
        format::g_w(i0, 10, 5),
        format::fixed_w(i0i1, 8, 2),
        format::fixed_w(inormal, 8, 2),
        format::fixed_w(iemerg, 8, 2),
    ));
}

/// One element's terminal-current block (Pascal `WriteTerminalCurrents`). Shared by
/// [`show_currents_elements`] and `Show busflow` (the per-bus branch-current form).
#[allow(clippy::too_many_arguments)]
pub(crate) fn write_terminal_currents(
    s: &mut String,
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
    s.push_str(&format!("ELEMENT = {}\n", format::enclose_quotes(name)));

    // AutoTrans special case (`ShowResults.pas:604/624`): `Ntimes = Nphases` rows
    // per terminal, and after each terminal the extra `Inc(k, Ntimes)` skips the
    // remaining conductor block (AutoTrans `NConds = 2·Nphases`).
    let autotrans = super::is_autotrans(name);
    let ntimes = if autotrans { nphases } else { ncond };
    let mut k = 0usize;
    for j in 0..nterm {
        // From-bus per terminal (`StripExtension(FirstBus/NextBus)`, uppercased).
        let from_bus = ckt
            .buses
            .get(cd.terminals[j].bus_ref)
            .map(|b| b.name.as_str())
            .unwrap_or("");
        let from_bus = format::pad(from_bus, mbnl).to_uppercase();
        let mut ctotal = Complex64::ZERO;
        for _ in 0..ntimes {
            let ck = cd.iterminal[k];
            if show_residual {
                ctotal += ck;
            }
            // `'%s  %4d    %13.5g /_ %6.1f =  %9.5g +j %9.5g'`
            // [UpperCase(FromBus), GetNodeNum(NodeRef[k]), Cabs, cdang, re, im].
            s.push_str(&format!(
                "{}  {}    {} /_ {} =  {} +j {}\n",
                from_bus,
                format::fixed_w_int(ckt.map_node_to_bus[cd.node_ref[k]].node_num as i64, 4),
                format::g_w(ck.norm(), 13, 5),
                format::fixed_w(cdang(ck), 6, 1),
                format::g_w(ck.re, 9, 5),
                format::g_w(ck.im, 9, 5),
            ));
            k += 1;
        }
        if show_residual && nphases > 1 {
            // `CtoPolardeg(-Ctotal)`: mag = |Ctotal|, ang = cdang(-Ctotal).
            let resid = -ctotal;
            s.push_str(&format!(
                "{} Resid    {} /_ {} =   {} +j {}\n",
                from_bus,
                format::g_w(resid.norm(), 13, 5),
                format::fixed_w(cdang(resid), 6, 1),
                format::g_w(resid.re, 9, 5),
                format::g_w(resid.im, 9, 5),
            ));
        }
        if j < nterm - 1 {
            s.push_str("------------\n");
        }
        if autotrans {
            k += ntimes; // Pascal `Inc(k, Ntimes)` — skip the rest of the block.
        }
    }
    s.push('\n'); // Pascal writes a blank line after each element.
}
