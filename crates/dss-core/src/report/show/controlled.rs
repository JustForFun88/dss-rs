//! `Show Controlled` (Pascal `ShowResults.pas` `ShowControlledElements`): every PD
//! element that carries a control, followed by the control element(s) acting on it.

use crate::circuit::Circuit;
use crate::circuit::controls::derive_control_lists;
use crate::elements::traits::ElemId;
use crate::exec::registry::DssClass;

/// Pascal `TDSSCktElement.FullName` = `ParentClass.Name + '.' + Name`.
fn full_name(classes: &[DssClass], r: ElemId) -> String {
    format!(
        "{}.{}",
        classes[r.class_ord()].props.class_name(),
        classes[r.class_ord()].arena[r.index()].data().name()
    )
}

/// Build the `Show Controlled` text (Pascal `ShowControlledElements`,
/// `ShowResults.pas:3905`). Walks `PDElements` in creation order; for every PD
/// element that has at least one control acting on it (Pascal `Flg.HasControl`),
/// writes its `FullName` then `, <control FullName> ` per control in
/// `ControlElementList` order (Pascal `FSWrite(F, Format(', %s ', [FullName]))`).
///
/// The Rust port materialises no `HasControl` flag / `ControlElementList`; the
/// per-element list is derived from the circuit-wide attach order by the shared
/// [`derive_control_lists`], the same derivation
/// [`Dss::snapshot_elements`](crate::exec::Dss::snapshot_elements)'s
/// `NumControls`/`OCPDev*`/`Has*Control` readers use — one order, so the report
/// and the API surface cannot drift. The whole report is read-only
/// (PHASE8_PLAN §2.1).
pub(crate) fn show_controlled(classes: &[DssClass], ckt: &Circuit) -> String {
    let lists = derive_control_lists(classes, ckt);
    let mut s = String::new();
    for pd in &ckt.pd_elements {
        // Pascal's `if Flg.HasControl in pdelem.Flags` — no controls → not written.
        let Some(controls) = lists.get(pd) else {
            continue;
        };
        s.push_str(&full_name(classes, *pd));
        for &cr in controls {
            s.push_str(&format!(", {} ", full_name(classes, cr)));
        }
        s.push('\n');
    }
    s
}
