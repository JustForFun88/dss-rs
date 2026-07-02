//! Per-report formatters for `Export` (Pascal `Common/ExportResults.pas`). Each
//! takes a read view of the (solved) circuit and returns the report text/CSV; no
//! formatter mutates electrical state (PHASE8_PLAN §2.1). Coverage grows per WP
//! (WP8.1: `Counts`; WP8.2: the solution exports; WP8.3: device/meter/reliability).
//!
//! Two formatter shapes:
//! * the **bus/node** exports (`Voltages`/`BusCoords`/`NodeNames`/`YNodeList`) are
//!   pure reads of the circuit — `fn(&Circuit) -> String`;
//! * the **element** exports (`Powers`/`Losses`/`P_byphase`/`ElemPowers`/…) walk
//!   the `Sources`/`PDElements`/`Faults`/`PCElements` lists calling the mutating
//!   terminal getters (`ComputeIterminal`/`ComputeVterminal`/`Power`/`GetLosses`),
//!   so they take the disjoint `(&mut [DssClass], &Circuit, &SysCtx, &[node_v])`
//!   borrow (the `Dss::snapshot_elements` pattern) via [`for_each_enabled_elem`].

mod bus_coords;
mod counts;
mod currents;
mod elem;
mod losses;
mod node_names;
mod node_order;
mod p_by_phase;
mod powers;
mod seq_currents;
mod seq_powers;
mod seq_voltages;
mod taps;
mod voltages;
mod ynode_list;

pub use bus_coords::export_bus_coords;
pub use counts::export_counts;
pub(crate) use currents::export_currents;
pub(crate) use elem::{export_elem_currents, export_elem_powers, export_elem_voltages};
pub(crate) use losses::export_losses;
pub use node_names::export_node_names;
pub(crate) use node_order::export_node_order;
pub(crate) use p_by_phase::export_p_by_phase;
pub(crate) use powers::export_powers;
pub(crate) use seq_currents::export_seq_currents;
pub(crate) use seq_powers::export_seq_powers;
pub use seq_voltages::export_seq_voltages;
pub(crate) use taps::export_taps;
pub use voltages::export_voltages;
pub use ynode_list::export_ynode_list;

use crate::elements::traits::{CktElement, ElemRef};
use crate::exec::registry::DssClass;

/// Walk a circuit element list (`Sources`/`PDElements`/`Faults`/`PCElements`) in
/// order, invoking `f(full_name, &mut elem)` for each **enabled** element — the
/// shared iteration the element exports build on (Pascal `<list>.First`/`.Next`
/// under `if Enabled`). The element's `FullName` (`Class.Name`, original case) is
/// pre-built; the text/CSV comparator matches identifiers case-insensitively
/// (PHASE8_PLAN §2.3), so the un-uppercased name is gate-equivalent.
///
/// `classes` and `refs` are passed as **separate** borrows (the `refs` come from
/// the immutable `&Circuit`, the elements are reached through `&mut [DssClass]`)
/// so the two never alias — `class_name()` returns `&'static str`, releasing the
/// `classes` borrow before the per-object `&mut`.
pub(crate) fn for_each_enabled_elem<F: FnMut(&str, &mut dyn CktElement)>(
    classes: &mut [DssClass],
    refs: &[ElemRef],
    mut f: F,
) {
    for &r in refs {
        let class_name = classes[r.cls].props.class_name();
        let obj = &mut classes[r.cls].objects[r.idx];
        let name = format!("{}.{}", class_name, obj.data().name());
        if let Some(elem) = obj.as_ckt_element_mut()
            && elem.cd().enabled
        {
            f(&name, elem);
        }
    }
}
