//! `Export Currents` (Pascal `ExportResults.pas` `ExportCurrents` +
//! `CalcAndWriteCurrents`): per-terminal, per-conductor terminal-current
//! magnitude/angle of every Source, then PD, then Fault, then PC element, plus a
//! per-terminal residual current. Every row is zero-filled to the circuit's
//! widest element (`MaxCond` conductors × `MaxTerm` terminals) so the columns
//! line up.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::complexutil::cdang;

/// Build the `Export Currents` body (Pascal `ExportCurrents`). Walks the four
/// element lists in Pascal order (Sources → PD → Faults → PC) calling the
/// mutating `GetCurrents` (`compute_iterminal`) and formatting each conductor's
/// magnitude (`%10.6g`) / angle (`%8.2f`), with a per-terminal residual.
pub(crate) fn export_currents(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    // File width (Pascal: `MaxCond := 1; MaxTerm := 2`, grown over *all*
    // `CktElements` regardless of `Enabled` — the header is padded to the widest).
    let mut max_cond = 1usize;
    let mut max_term = 2usize;
    for &r in &ckt.ckt_elements {
        if let Some(elem) = classes[r.cls].arena[r.idx].as_ckt_element() {
            max_term = max_term.max(elem.cd().nterms);
            max_cond = max_cond.max(elem.cd().nconds);
        }
    }

    let mut s = String::from("Element");
    for i in 1..=max_term {
        for j in 1..=max_cond {
            s.push_str(&format!(", I{i}_{j}, Ang{i}_{j}"));
        }
        s.push_str(&format!(", Iresid{i}, AngResid{i}"));
    }
    s.push('\n');

    // Pascal `Format(', %10.6g, %8.2f', [Cabs(c), cdang(c)])` (widths trimmed by
    // the comparator); the zero-fill reuses the same spec (`0` / `0.00`).
    let pair = |c: Complex64| {
        format!(
            ", {}, {}",
            format::g(c.norm(), 6),
            format::fixed(cdang(c), 2)
        )
    };

    let mut calc = |name: &str, elem: &mut dyn CktElement| {
        elem.compute_iterminal(sys, node_v);
        let cd = elem.cd();
        let (nterms, nconds) = (cd.nterms, cd.nconds);
        s.push_str(&format::upper_elem_name(name));
        for term in cd.terminals_i() {
            let mut iresid = Complex64::ZERO;
            for &c in term {
                s.push_str(&pair(c));
                iresid += c;
            }
            for _ in nconds + 1..=max_cond {
                s.push_str(&pair(Complex64::ZERO));
            }
            s.push_str(&pair(iresid));
        }
        // Filler if `Nterms < TermWidth`: `CondWidth + 1` zero pairs per missing
        // terminal (the `+ 1` covers the residual column).
        for _ in nterms + 1..=max_term {
            for _ in 1..=max_cond + 1 {
                s.push_str(&pair(Complex64::ZERO));
            }
        }
        s.push('\n');
    };

    for_each_enabled_elem(classes, &ckt.sources, &mut calc);
    for_each_enabled_elem(classes, &ckt.pd_elements, &mut calc);
    for_each_enabled_elem(classes, &ckt.faults, &mut calc);
    for_each_enabled_elem(classes, &ckt.pc_elements, &mut calc);
    s
}
