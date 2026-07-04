//! `Show Currents` (Pascal `ShowResults.pas` `ShowCurrents` + `WriteSeqCurrents` +
//! `GetI0I1I2`), `ShowOptionCode = 0` — the symmetrical-component currents by
//! circuit element (first 3 phases): per terminal `I1`, `I2`, `%I2/I1`, `I0`,
//! `%I0/I1`, and (non-capacitor terminal 1 only) `%Normal`/`%Emergency`.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
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
        let ncond = elem.cd().nconds;
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
        // `AnsiUpperCase(Name)`); a `-` continuation for terminals > 1.
        let padded = format::pad_dots(&format::enclose_quotes(name), mdnl + 2).to_uppercase();
        let cont = format::pad("   -", padded.chars().count());
        let cd = elem.cd();

        for j in 1..=nterm {
            // `GetI0I1I2`: Cmax = max phase magnitude (>=3 phases) then sym-comp;
            // <3 phases → I1 = |first-phase current| UNCONDITIONALLY (Cmax = I1).
            let koff = (j - 1) * ncond;
            let (i0, i1, i2, cmax) = if nphases >= 3 {
                let iph = [
                    cd.iterminal[koff],
                    cd.iterminal[koff + 1],
                    cd.iterminal[koff + 2],
                ];
                let cmax = iph.iter().map(|c| c.norm()).fold(0.0, f64::max);
                let mut i012 = [Complex64::ZERO; 3];
                sc.phase_to_sym(&iph, &mut i012);
                (i012[0].norm(), i012[1].norm(), i012[2].norm(), cmax)
            } else {
                // Show's <3-phase branch is NOT `PositiveSequence`-gated (unlike
                // `ExportSeqCurrents`): I1 = |first-phase current| unconditionally.
                let i1 = cd.iterminal[koff].norm();
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

            // `'%s %3d  %10.5g   %10.5g %8.2f  %10.5g %8.2f  %8.2f %8.2f'`.
            let label = if j == 1 { &padded } else { &cont };
            s.push_str(label);
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
