//! `Export NodeOrder` (Pascal `ExportResults.pas` `ExportNodeOrder` +
//! `WriteNodeList`): the per-element node-reference list, in the same
//! Sources → PD → Faults → PC order as `Export Currents`. Each row is
//! `"Element", Nterminals, Nconductors, Node-1, Node-2, …` where each node is
//! the bus-local node number (`GetNodeNum`, 0 = ground).

use crate::circuit::Circuit;
use crate::elements::traits::CktElement;
use crate::exec::registry::DssClass;
use crate::report::export::for_each_enabled_elem;

/// Build the `Export NodeOrder` body (Pascal `ExportNodeOrder`). Read-only over
/// the element node refs (routed through the mutable walk since
/// [`for_each_enabled_elem`] hands out `&mut dyn CktElement`).
///
/// Pascal `WriteNodeList` guards each element on `IsSolved` (error 222001) and
/// exits early, so an **unsolved** circuit yields a header-only file; the router
/// records the 222001 error once (PHASE8_PLAN §1 — no silent fake output).
pub(crate) fn export_node_order(classes: &mut [DssClass], ckt: &Circuit) -> String {
    let mut s = String::from("Element, Nterminals, Nconductors, Node-1, Node-2, Node-3, ...\n");
    if !ckt.is_solved {
        return s; // WriteNodeList's per-element `IsSolved` guard → header only.
    }

    let mut write = |name: &str, elem: &mut dyn CktElement| {
        let cd = elem.cd();
        let (nterms, nconds) = (cd.nterms, cd.nconds);
        // Pascal `Format('"%s", %d, %d', [CktElementName, Nterms, Nconds])` — the
        // full name (`Class.Name`, original case) quoted; identifiers compare
        // case-insensitively (PHASE8_PLAN §2.3).
        s.push_str(&format!("\"{name}\", {nterms}, {nconds}"));
        for i in 0..nconds * nterms {
            // `GetNodeNum(NodeRef^[i])`: ground (`node_ref == 0`) → 0, else the
            // bus-local `MapNodeToBus[NodeRef].NodeNum` (slot 0 defaults to 0).
            s.push_str(&format!(
                ", {}",
                ckt.map_node_to_bus[cd.node_ref[i]].node_num
            ));
        }
        s.push('\n');
    };

    for_each_enabled_elem(classes, &ckt.sources, &mut write);
    for_each_enabled_elem(classes, &ckt.pd_elements, &mut write);
    for_each_enabled_elem(classes, &ckt.faults, &mut write);
    for_each_enabled_elem(classes, &ckt.pc_elements, &mut write);
    s
}
