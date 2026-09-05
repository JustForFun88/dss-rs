//! Pascal's per-element `ControlElementList` (`Common/CktElement.pas:100`,
//! `:223`), derived.
//!
//! Upstream every circuit element owns a `TPointerList` of the controls acting
//! on it, maintained by `TControlElem.Set_ControlledElement`
//! (r4133 `Controls/ControlElem.pas:113-131`): remove self from the previous
//! target's list (`RemoveSelfFromControlElementList`, `:81-99`, which rebuilds
//! the list in order omitting self), then `Add(Self)` — i.e. **append at the
//! end** — to the new target's. r4133 re-assigns `ControlledElement` inside
//! `RecalcElementData`, so *every* edit of a control re-runs that pair
//! (`Controls/Relay.pas:955` reached from `:626`; `Recloser.pas:702`,
//! `SwtControl.pas:332`, `CapControl.pas:580`, `RegControl.pas:693`,
//! `Controls/fuse.pas`).
//!
//! The port stores only the forward control → element reference
//! ([`CktElement::controlled_element`](crate::elements::traits::CktElement::controlled_element)),
//! so the per-element list is derived here from the circuit-wide chronological
//! attach order ([`Circuit::control_attach_order`]). Restricting that one global
//! order to the controls currently pointing at element `e` reproduces `e`'s
//! `ControlElementList` order exactly: every such list is built by appends in
//! that same chronological order, and a control belongs to at most one list at a
//! time.
//!
//! Consumers: `report/show/controlled.rs` (`ShowControlledElements`) and
//! [`Dss::snapshot_elements`](crate::exec::Dss::snapshot_elements)'s
//! `NumControls` / `OCPDevIndex` / `OCPDevType` / `HasVoltControl` /
//! `HasSwitchControl` readers — one derivation, so the report and the API
//! surface cannot drift.

use std::collections::HashMap;

use crate::circuit::Circuit;
use crate::elements::traits::ElemId;
use crate::exec::registry::DssClass;

/// The `CLASSMASK` categories the five control-derived `CktElement` scalars ask
/// about (r4133 `Common/DSSClassDefs.pas:43-48`, `:55`): `CAP_CONTROL` (11·8),
/// `REG_CONTROL` (12·8), `RELAY_CONTROL` (14·8), `RECLOSER_CONTROL` (15·8),
/// `FUSE_CONTROL` (16·8), `SWT_CONTROL` (23·8).
///
/// [`Self::Other`] is every other registered control class — `GenDispatcher`,
/// `StorageController`, `UPFCControl`, `ESPVLControl`, `InvControl`,
/// `ExpControl`. None of them ever joins a `ControlElementList` upstream (their
/// `ControlledElement` is nil, a private field or an array — r4133
/// `Controls/GenDispatcher.pas:290`, `StorageController.pas:828`,
/// `ESPVLControl.pas:362`, `InvControl.pas:74`/`:1256`, `ExpControl.pas:33`/`:390`,
/// `UPFCControl.pas` never assigns it), and none of them sets
/// `controlled_element` in the port either — pinned by
/// `exec::tests::element_extras::only_six_control_classes_join_an_elements_control_list`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ControlCategory {
    /// `SWT_CONTROL` — the only class `HasSwitchControl` answers for.
    Swt,
    /// `CAP_CONTROL` — with [`Self::Reg`], what `HasVoltControl` answers for.
    Cap,
    /// `REG_CONTROL`.
    Reg,
    /// `FUSE_CONTROL` — `GetOCPDeviceType` code `1`.
    Fuse,
    /// `RECLOSER_CONTROL` — code `2`.
    Recloser,
    /// `RELAY_CONTROL` — code `3`.
    Relay,
    /// Any other class: never an OCP device, never a volt/switch control.
    Other,
}

impl ControlCategory {
    /// `GetOCPDeviceType`'s result code for this class (r4133
    /// `Common/Utilities.pas:3176-3178`): `1` Fuse, `2` Recloser, `3` Relay,
    /// `0` (`Result := 0`, `:3170`) for anything else.
    pub(crate) fn ocp_code(self) -> i32 {
        match self {
            Self::Fuse => 1,
            Self::Recloser => 2,
            Self::Relay => 3,
            _ => 0,
        }
    }

    /// Whether this class is one of the three over-current-protection devices
    /// `OCPDevIndex`/`OCPDevType` scan for (r4133 `DDLL/DCktElement.pas:249-253`,
    /// `Common/Utilities.pas:3176-3178`).
    pub(crate) fn is_ocp(self) -> bool {
        self.ocp_code() > 0
    }
}

/// The `DSSObjType and CLASSMASK` of a control, as the five scalars read it.
///
/// Matched on the typed [`ElemId`] variant rather than on a class-name string:
/// the mask is a property of the class, and the compiler then lists every arm.
/// A newly registered control class falls into [`ControlCategory::Other`] — the
/// same answer upstream gives a class with no `CLASSMASK` arm — and the totality
/// pin named on [`ControlCategory`] is what makes that deliberate.
pub(crate) fn control_category(r: ElemId) -> ControlCategory {
    match r {
        ElemId::SwtControl(_) => ControlCategory::Swt,
        ElemId::CapControl(_) => ControlCategory::Cap,
        ElemId::RegControl(_) => ControlCategory::Reg,
        ElemId::Fuse(_) => ControlCategory::Fuse,
        ElemId::Recloser(_) => ControlCategory::Recloser,
        ElemId::Relay(_) => ControlCategory::Relay,
        _ => ControlCategory::Other,
    }
}

/// Every element's `ControlElementList`, in Pascal's list order.
///
/// Walks [`Circuit::control_attach_order`] — the chronological
/// remove-then-append order `Set_ControlledElement` maintains — and buckets each
/// control under its *current* [`controlled_element`](crate::elements::traits::CktElement::controlled_element).
/// Elements with no control are absent from the map (Pascal's
/// `Flg.HasControl not in Flags`, an empty `ControlElementList`).
///
/// A control that is registered but whose element reference did not resolve
/// carries `None` and is in no list, exactly like upstream's
/// `ControlledElement := nil` (r4133 `Controls/Relay.pas:983`).
pub(crate) fn derive_control_lists(
    classes: &[DssClass],
    ckt: &Circuit,
) -> HashMap<ElemId, Vec<ElemId>> {
    let mut lists: HashMap<ElemId, Vec<ElemId>> = HashMap::new();
    for &cr in &ckt.control_attach_order {
        if let Some(target) = classes[cr.class_ord()]
            .arena
            .try_ckt_elem(cr.index())
            .and_then(|ce| ce.controlled_element())
        {
            lists.entry(target).or_default().push(cr);
        }
    }
    lists
}
