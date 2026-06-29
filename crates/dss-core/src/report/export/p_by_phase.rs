//! `Export P_byphase` (Pascal `ExportResults.pas` `ExportPbyphase`): per-conductor
//! complex power (kW/kvar, or MW/Mvar for `opt = 1`) of every PD then PC element,
//! over the full `Yorder` (every conductor of every terminal).

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::traits::SysCtx;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;
use crate::report::format;

/// Build the `Export P_byphase` body (Pascal `ExportPbyphase`, `opt` = 0 → kVA,
/// 1 → MVA). Per conductor: `S = Vterminal·conj(Iterminal)·0.001` — note there is
/// **no** positive-sequence `×3` here (unlike `Power[j]`), exactly as Pascal
/// forms it from `ComputeVterminal`/`ComputeIterminal` directly.
pub(crate) fn export_p_by_phase(
    classes: &mut [DssClass],
    ckt: &Circuit,
    sys: &SysCtx,
    node_v: &[Complex64],
    opt: i32,
) -> String {
    let mut s = String::new();
    if opt == 1 {
        s.push_str("Element, NumTerminals, NumConductors, NumPhases, MW1, Mvar1, MW2, Mvar2, MW3, Mvar3, ... \n");
    } else {
        s.push_str("Element, NumTerminals, NumConductors, NumPhases, kW1, kvar1, kW2, kvar2, kW3, kvar3, ... \n");
    }
    let scale = if opt == 1 { 0.001 } else { 1.0 };

    let mut write_elem = |name: &str, elem: &mut dyn crate::elements::traits::CktElement| {
        elem.compute_iterminal(sys, node_v);
        elem.cd_mut().compute_vterminal(node_v);
        let cd = elem.cd();
        s.push_str(&format!(
            "\"{}\", {}, {}, {}",
            format::upper_elem_name(name),
            cd.nterms,
            cd.nconds,
            cd.nphases,
        ));
        for i in 0..cd.yorder {
            let sk = cd.vterminal[i] * cd.iterminal[i].conj() * 0.001 * scale;
            s.push_str(&format!(
                ", {}, {}",
                format::fixed(sk.re, 3),
                format::fixed(sk.im, 3)
            ));
        }
        s.push('\n');
    };

    for_each_enabled_elem(classes, &ckt.pd_elements, &mut write_elem);
    for_each_enabled_elem(classes, &ckt.pc_elements, &mut write_elem);
    s
}
