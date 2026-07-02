//! `Export ElemCurrents` / `ElemVoltages` / `ElemPowers` (Pascal
//! `ExportResults.pas` `WriteElemCurrents`/`WriteElemVoltages`/`WriteElemPowers`
//! and their `Export…` drivers): per-conductor terminal current / voltage /
//! complex power of every element, in the same Sources → PD → Faults → PC order
//! as `Export NodeOrder`. Each row is `"Element", Nterminals, Nconductors, …`
//! with one value pair per `NConds·Nterms` conductor.
//!
//! Pascal's `WriteElem*` guards each element on `IsSolved` (error 222001) and
//! exits early, so an **unsolved** circuit yields a header-only file; the router
//! records the 222001 error once (PHASE8_PLAN §1 — no silent fake output).

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::{CktElement, SysCtx};
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;
use crate::support::complexutil::cdang;

/// Walk Sources → PD → Faults → PC, calling `row(name, elem)` for each enabled
/// element after the shared header + `IsSolved` guard. `row` appends the value
/// columns; this helper writes the `"Name", Nterms, Nconds` prefix and newline.
fn walk<F>(classes: &mut [DssClass], ckt: &Circuit, header: &str, mut row: F) -> String
where
    F: FnMut(&mut String, &mut dyn CktElement),
{
    let mut s = String::from(header);
    if !ckt.is_solved {
        return s; // WriteElem*'s per-element `IsSolved` guard → header only.
    }
    let mut write = |name: &str, elem: &mut dyn CktElement| {
        let (nterms, nconds) = (elem.cd().nterms, elem.cd().nconds);
        s.push_str(&format!("\"{name}\", {nterms}, {nconds}"));
        row(&mut s, elem);
        s.push('\n');
    };
    for_each_enabled_elem(classes, &ckt.sources, &mut write);
    for_each_enabled_elem(classes, &ckt.pd_elements, &mut write);
    for_each_enabled_elem(classes, &ckt.faults, &mut write);
    for_each_enabled_elem(classes, &ckt.pc_elements, &mut write);
    s
}

/// `%10.6g` magnitude, `%8.2f` angle (widths trimmed by the comparator).
fn mag_ang(c: Complex64) -> String {
    format!(
        ", {}, {}",
        format::g(c.norm(), 6),
        format::fixed(cdang(c), 2)
    )
}

/// `Export ElemCurrents` (Pascal `WriteElemCurrents`): `ComputeIterminal`, then
/// `|I|`/angle per conductor over `NConds·Nterms`.
pub(crate) fn export_elem_currents(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    walk(
        classes,
        ckt,
        "Element, Nterminals, Nconductors, I_1, Ang_1, ...\n",
        |s, elem| {
            elem.compute_iterminal(sys, node_v);
            let cd = elem.cd();
            for i in 0..cd.nconds * cd.nterms {
                s.push_str(&mag_ang(cd.iterminal[i]));
            }
        },
    )
}

/// `Export ElemVoltages` (Pascal `WriteElemVoltages`): `ComputeVterminal`, then
/// `|V|`/angle per conductor.
pub(crate) fn export_elem_voltages(
    classes: &mut [DssClass],
    ckt: &Circuit,
    _sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    walk(
        classes,
        ckt,
        "Element, Nterminals, Nconductors, V_1, Ang_1, ...\n",
        |s, elem| {
            elem.cd_mut().compute_vterminal(node_v);
            let cd = elem.cd();
            for i in 0..cd.nconds * cd.nterms {
                s.push_str(&mag_ang(cd.vterminal[i]));
            }
        },
    )
}

/// `Export ElemPowers` (Pascal `WriteElemPowers`): `ComputeVterminal;
/// ComputeIterminal`, then `S = Vterminal·conj(Iterminal)`, printed
/// `S.re·0.001`/`S.im·0.001` (kW/kvar, `%10.6g` both).
///
/// This forms the per-conductor power straight from `ComputeVterminal`/
/// `ComputeIterminal` (like `P_byphase`), not from the `×3`-positive-seq
/// `Power[j]`. On a solved feeder this equals the canonical terminal power for
/// every element (oracle-probed on IEEE13: the `Vsource.source` row is
/// `-1024.12, -612.729`, identical to `CktElement.Powers`).
///
/// **Order matters** — `compute_iterminal` is called *before* `compute_vterminal`
/// (the reverse of Pascal's `ComputeVterminal; ComputeIterminal`). Pascal relies
/// on `ITerminalUpdated = TRUE` post-solve so `ComputeIterminal` reuses the stored
/// current and never re-enters `GetCurrents`, leaving `Vterminal = NodeV`. Our
/// `compute_iterminal` re-runs `GetCurrents`, and `TVsourceObj.GetCurrents`
/// *overwrites* `Vterminal` with the source EMF `[Vsource; 0]` (≈ but ≠ NodeV, so
/// `EMF·conj(I)` gives the isolated-`Vsource` `-612.936` 2a surfaced). Computing
/// `iterminal` first, then `vterminal`, restores `Vterminal = NodeV` for the
/// power product — reproducing the oracle's observable `NodeV·conj(I)`. For every
/// other element `GetCurrents` sets `Vterminal = NodeV` too, so the order is inert.
pub(crate) fn export_elem_powers(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
) -> String {
    walk(
        classes,
        ckt,
        "Element, Nterminals, Nconductors, P_1, Q_1, ...\n",
        |s, elem| {
            elem.compute_iterminal(sys, node_v);
            elem.cd_mut().compute_vterminal(node_v);
            let cd = elem.cd();
            for i in 0..cd.nconds * cd.nterms {
                let sk = cd.vterminal[i] * cd.iterminal[i].conj();
                s.push_str(&format!(
                    ", {}, {}",
                    format::g(sk.re * 0.001, 6),
                    format::g(sk.im * 0.001, 6)
                ));
            }
        },
    )
}
