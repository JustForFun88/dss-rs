//! The `CktElement` and `DssObject` trait impls for [`UpfcControl`]: the zero
//! Yprim/currents control-element surface, the `UPFCList` string-list property,
//! `EndEdit` and the (phases/element-only) `MakeLike`.

use num_complex::Complex64;

use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};

use super::{UpfcControl, prop};

impl CktElement for UpfcControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TControlElem.FControlledElement` - the element this control
    /// acts on (`None` when it drives a list rather than a single element).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemRef> {
        self.ccd.controlled_element
    }

    /// Pascal `TUPFCControlObj.RecalcElementData`: empty.
    fn recalc_element_data(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim NIL — `BuildYMatrix` skips it.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TUPFCControlObj.MakePosSequence`
    /// (`Controls/UPFCControl.pas:179`). **NIL-deref hazard** (Access violation
    /// #303, `docs/wpg21_makeposseq_probes.md`): a *fleet* control whose
    /// `ControlledElement` is always NIL, yet the body guards on
    /// `MonitoredElement` then dereferences `ControlledElement.NPhases`. Per
    /// CLAUDE.md UB is never reproduced: when `ctx.controlled` is `None`,
    /// safe-skip the whole block. (Probe `3c` — no `element=` set — takes the
    /// NIL-guard-skip path and converges cleanly.)
    fn make_pos_sequence(&mut self, ctx: &PosSeqCtx) -> PosSeqPlan {
        // Pascal guards on `MonitoredElement <> NIL` then derefs `ControlledElement`;
        // act only when BOTH are resolved.
        if let Some((c, m)) = ctx.controlled.as_ref().zip(ctx.monitored.as_ref()) {
            // FNphases := ControlledElement.NPhases; Nconds := FNphases
            self.ccd.cd.nphases = c.nphases;
            self.ccd.cd.set_nconds(c.nphases);
            // Setbus(1, MonitoredElement.GetBus(ElementTerminal))
            let t = self.ccd.element_terminal as usize;
            let bus = t
                .checked_sub(1)
                .and_then(|k| m.bus_names.get(k))
                .cloned()
                .unwrap_or_default();
            self.ccd.cd.set_bus(1, &bus);
        }
        // else: either MonitoredElement is NIL (Pascal skips the block) or it is
        // set while ControlledElement is NIL — the crash config, where Pascal
        // derefs NIL and faults (#303). Both collapse to a safe no-op here.
        // inherited MakePosSequence -> base bus rename.
        PosSeqPlan::base()
    }

    /// Pascal `TControlElem.MonitoredElement` — resolved so the exec applier can
    /// build [`PosSeqCtx::monitored`] before calling [`Self::make_pos_sequence`].
    fn monitored_element_ref(&self) -> Option<ElemRef> {
        self.ccd.monitored_element
    }
}

impl UpfcControl {
    /// Pascal `TUPFCControlObj.MakeLike` — copies only the phase count, terminal,
    /// and controlled/monitored element refs (plus the base `PrpSequence`); the
    /// UPFC list/weights are **not** copied, so a `like=` control keeps the ctor
    /// defaults for them. Reproduced verbatim.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff
        self.ccd.controlled_element = other.ccd.controlled_element;
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.ccd.element_terminal = other.ccd.element_terminal;
    }
}

impl DssObject for UpfcControl {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }
    fn as_control(&self) -> Option<&dyn crate::elements::control::control_elem::ControlElem> {
        Some(self)
    }
    fn as_control_mut(
        &mut self,
    ) -> Option<&mut dyn crate::elements::control::control_elem::ControlElem> {
        Some(self)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            prop::BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("UPFCControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            prop::BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("UPFCControl has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("UPFCControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("UPFCControl has no boolean property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::UPFCLIST => self.upfc_name_list.clone(),
            _ => unreachable!("UPFCControl has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            // Pascal stores the names in FUPFCNameList but never sets ListSize from
            // them (no PropertySideEffects), so the fleet is not filtered — see the
            // module note.
            prop::UPFCLIST => self.upfc_name_list = value,
            _ => unreachable!("UPFCControl has no string-list property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData` (a no-op).
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {}

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

impl crate::elements::control::control_elem::ControlElem for UpfcControl {
    fn ccd(&self) -> &crate::elements::control::control_elem::ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut crate::elements::control::control_elem::ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::Upfc
    }
}
