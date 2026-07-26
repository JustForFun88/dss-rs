//! The `CktElement` and `DssObject` trait impls for `EspvlControl`: the zero
//! Yprim/currents control-element surface, the typed property accessors, the
//! `element=` resolution, `PropertySideEffects`, `EndEdit` and `MakeLike`.

use num_complex::Complex64;

use crate::elements::control::control_elem::RefSnapshot;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

use super::{EspvlControl, prop};

impl CktElement for EspvlControl {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    /// Pascal `TControlElem.FControlledElement` - the element this control
    /// acts on (`None` when it drives a list rather than a single element).
    fn controlled_element(&self) -> Option<crate::elements::traits::ElemId> {
        self.ccd.controlled_element
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim NIL — `BuildYMatrix` skips it.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }

    /// Pascal `TESPVLControlObj.MakePosSequence`
    /// (`Controls/ESPVLControl.pas:357`). **NIL-deref hazard** (Access violation
    /// #303, probe `S4`, `docs/wpg21_makeposseq_probes.md`): a *fleet* control
    /// whose `ControlledElement` is always NIL, yet the body guards on
    /// `MonitoredElement` then dereferences `ControlledElement.NPhases`. Per
    /// CLAUDE.md UB is never reproduced: when `ctx.controlled` is `None`,
    /// safe-skip the whole block.
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
    fn monitored_element_ref(&self) -> Option<ElemId> {
        self.ccd.monitored_element
    }
}

impl EspvlControl {
    /// Pascal `TESPVLControlObj.MakeLike` — copies *only* the phase count,
    /// monitored element, and terminal (plus the base `PrpSequence`); `Type`, the
    /// bands, and every list are deliberately **not** copied, so a `like=` control
    /// keeps the ctor defaults for them (oracle-proven, like GenDispatcher).
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff
        self.ccd.monitored_element = other.ccd.monitored_element;
        self.monitored_full_name = other.monitored_full_name.clone();
        self.mon_snap = other.mon_snap.clone();
        self.ccd.element_terminal = other.ccd.element_terminal;
    }
}

impl DssObject for EspvlControl {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
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
        use prop::*;
        match idx {
            KW_BAND => self.f_kw_band,
            KVAR_LIMIT => self.f_kvar_limit,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("ESPVLControl has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KW_BAND => self.f_kw_band = value,
            KVAR_LIMIT => self.f_kvar_limit = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("ESPVLControl has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal,
            TYP => self.f_type,
            // Pascal `IndirectCount` reads the count from `PropertyOffset2`
            // (`F*ListSize`); the Weights arrays' `size_prop` points at the
            // matching name-list property, so schema/JSON length reads land here.
            LOCAL_CONTROL_LIST => self.local_control_list_size,
            PV_SYSTEM_LIST => self.pv_system_list_size,
            STORAGE_LIST => self.storage_list_size,
            _ => unreachable!("ESPVLControl has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal = value,
            TYP => self.f_type = value,
            _ => unreachable!("ESPVLControl has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("ESPVLControl has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("ESPVLControl has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::ELEMENT => self.monitored_full_name.clone(),
            _ => unreachable!("ESPVLControl has no string property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        use prop::*;
        match idx {
            LOCAL_CONTROL_LIST => self.local_control_name_list.clone(),
            PV_SYSTEM_LIST => self.pv_system_name_list.clone(),
            STORAGE_LIST => self.storage_name_list.clone(),
            _ => unreachable!("ESPVLControl has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        use prop::*;
        match idx {
            LOCAL_CONTROL_LIST => self.local_control_name_list = value,
            PV_SYSTEM_LIST => self.pv_system_name_list = value,
            STORAGE_LIST => self.storage_name_list = value,
            _ => unreachable!("ESPVLControl has no string-list property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use prop::*;
        // Pascal weight arrays are NIL until allocated; a NIL array dumps as ""
        // (not "[]"), so report an empty list as absent (probed: a list set
        // without weights dumps '').
        match idx {
            LOCAL_CONTROL_WEIGHTS => (!self.local_control_weights.is_empty())
                .then_some(self.local_control_weights.as_slice()),
            PV_SYSTEM_WEIGHTS => {
                (!self.pv_system_weights.is_empty()).then_some(self.pv_system_weights.as_slice())
            }
            STORAGE_WEIGHTS => {
                (!self.storage_weights.is_empty()).then_some(self.storage_weights.as_slice())
            }
            _ => unreachable!("ESPVLControl has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use prop::*;
        match idx {
            LOCAL_CONTROL_WEIGHTS => self.local_control_weights = value,
            PV_SYSTEM_WEIGHTS => self.pv_system_weights = value,
            STORAGE_WEIGHTS => self.storage_weights = value,
            _ => unreachable!("ESPVLControl has no double-array property {idx}"),
        }
    }
    /// Pascal weights IndirectCount: each weight array's element count comes from
    /// its companion name list (`PropertyOffset3 = @F*NameList`).
    fn array_size(&self, idx: usize) -> usize {
        use prop::*;
        match idx {
            LOCAL_CONTROL_WEIGHTS => self.local_control_name_list.len(),
            PV_SYSTEM_WEIGHTS => self.pv_system_name_list.len(),
            STORAGE_WEIGHTS => self.storage_name_list.len(),
            _ => unreachable!("ESPVLControl has no function-sized array {idx}"),
        }
    }

    /// `element=` resolution (any circuit element by full name): keep the
    /// `ElemId` plus a shape snapshot for `RecalcElementData`.
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        match idx {
            prop::ELEMENT => {
                // `name` is the FullName ("Class.name") for the dump.
                self.monitored_full_name = name.clone();
                match resolved {
                    Some(o) => {
                        self.ccd.monitored_element = Some(o.id());
                        let elem = o.ckt().expect("element= resolves against circuit classes");
                        self.mon_snap = Some(RefSnapshot::capture(name, elem));
                    }
                    None => {
                        self.ccd.monitored_element = None;
                        self.mon_snap = None;
                    }
                }
            }
            _ => unreachable!("ESPVLControl has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TESPVLControlObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            PV_SYSTEM_LIST => {
                self.pv_system_list_size = self.pv_system_name_list.len() as i32;
                // Pascal only resizes when the weights are already allocated
                // (`FPVSystemWeights <> NIL`); leave a NIL (empty) array untouched.
                if !self.pv_system_weights.is_empty() {
                    self.pv_system_weights
                        .resize(self.pv_system_list_size.max(0) as usize, 0.0);
                }
            }
            STORAGE_LIST => {
                self.storage_list_size = self.storage_name_list.len() as i32;
                if !self.storage_weights.is_empty() {
                    self.storage_weights
                        .resize(self.storage_list_size.max(0) as usize, 0.0);
                }
            }
            LOCAL_CONTROL_LIST => {
                // Levelize the list (clear pointer cache for a re-resolve on the
                // next Sample; set uniform weights).
                self.local_control_pointer_list.clear();
                self.local_control_list_size = self.local_control_name_list.len() as i32;
                self.local_control_weights =
                    vec![1.0; self.local_control_list_size.max(0) as usize];
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }
}

impl crate::elements::control::control_elem::ControlElem for EspvlControl {
    fn ccd(&self) -> &crate::elements::control::control_elem::ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut crate::elements::control::control_elem::ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::Espvl
    }
}
