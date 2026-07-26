//! `Show Controlled` (Pascal `ShowResults.pas` `ShowControlledElements`): every PD
//! element that carries a control, followed by the control element(s) acting on it.

use crate::circuit::Circuit;
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
/// The Rust port materialises no `HasControl` flag / `ControlElementList` (the
/// model stores only the forward control→element reference,
/// [`CktElement::controlled_element`](crate::elements::traits::CktElement::controlled_element)),
/// so the per-PD list is derived by scanning `ckt.controls` (creation order) and
/// matching each control's `controlled_element()`. This reproduces the exact
/// observable: `ControlElementList` insertion order == control creation order ==
/// `ckt.controls` order, and a control reassigned to a different element follows
/// its *current* target — the final state of Pascal's remove-then-add
/// `Set_ControlledElement`. The whole report is read-only (PHASE8_PLAN §2.1).
pub(crate) fn show_controlled(classes: &[DssClass], ckt: &Circuit) -> String {
    let mut s = String::new();
    for &pd in &ckt.pd_elements {
        // The controls acting on this PD element, in creation order (Pascal's
        // `ControlElementList` for `pdelem`).
        let controls: Vec<ElemId> = ckt
            .controls
            .iter()
            .copied()
            .filter(|&cr| {
                classes[cr.class_ord()]
                    .arena
                    .try_ckt_elem(cr.index())
                    .and_then(|ce| ce.controlled_element())
                    == Some(pd)
            })
            .collect();
        // Pascal's `if Flg.HasControl in pdelem.Flags` — no controls → not written.
        if controls.is_empty() {
            continue;
        }
        s.push_str(&full_name(classes, pd));
        for cr in controls {
            s.push_str(&format!(", {} ", full_name(classes, cr)));
        }
        s.push('\n');
    }
    s
}
