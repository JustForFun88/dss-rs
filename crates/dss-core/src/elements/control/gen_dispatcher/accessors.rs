//! The `CktElement` and `DssObject` trait impls for `GenDispatcher`: the zero
//! Yprim/currents control-element surface, the typed property accessors, the
//! `element=` resolution, `PropertySideEffects`, `EndEdit` and `MakeLike`. Split
//! out of `gen_dispatcher/mod.rs` (no behavioral change).

use num_complex::Complex64;

use crate::elements::control::control_elem::RefSnapshot;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemId, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};

use super::{GenDispatcher, prop};

impl CktElement for GenDispatcher {
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

    /// Pascal `TGenDispatcherObj.MakePosSequence`
    /// (`Controls/GenDispatcher.pas:263`). **NIL-deref hazard** (Access violation
    /// #303, probes `3a`/`S4`, `docs/wpg21_makeposseq_probes.md`): this is a
    /// *fleet* control acting on a list of generators, so `ControlledElement` is
    /// **always** NIL, yet the upstream body guards on `MonitoredElement` and
    /// then dereferences `ControlledElement.NPhases`. Whenever `element=` is set
    /// (`MonitoredElement <> NIL`) the oracle crashes. Per CLAUDE.md UB is never
    /// reproduced: when the deref target (`ctx.controlled`) is `None`, safe-skip
    /// the whole block (mirroring the crash as a no-op).
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

impl GenDispatcher {
    /// Pascal `TGenDispatcherObj.MakeLike` — note it copies *only* the phase
    /// count, monitored element, and terminal (plus the base `PrpSequence`); the
    /// dispatch settings (`kWLimit`/`kWBand`/`kvarLimit`/`GenList`/`Weights`)
    /// are deliberately **not** copied, so a `like=` dispatcher keeps the ctor
    /// defaults for them. Reproduced verbatim.
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

impl DssObject for GenDispatcher {
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
            KWLIMIT => self.f_kw_limit,
            KWBAND => self.f_kw_band,
            KVARLIMIT => self.f_kvar_limit,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("GenDispatcher has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KWLIMIT => self.f_kw_limit = value,
            KWBAND => self.f_kw_band = value,
            KVARLIMIT => self.f_kvar_limit = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("GenDispatcher has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::TERMINAL => self.ccd.element_terminal,
            // Pascal `Weights` IndirectCount reads its element count from
            // `FListSize` (`GenDispatcher.pas` `PropertyOffset2 = @FListSize`),
            // kept in sync with the generator-name-list length. The DoubleArray
            // count path (`class_props/json.rs`) + the schema `$dssLength: GenList`
            // resolve it through the GenList property slot.
            prop::GENLIST => self.list_size,
            _ => unreachable!("GenDispatcher has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::TERMINAL => self.ccd.element_terminal = value,
            _ => unreachable!("GenDispatcher has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("GenDispatcher has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            // GenDispatcher does not override Set_Enabled; for a control with no
            // Yprim, toggling the flag has no node-order effect either way.
            prop::ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("GenDispatcher has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::ELEMENT => self.monitored_full_name.clone(),
            _ => unreachable!("GenDispatcher has no string property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::GENLIST => self.gen_name_list.clone(),
            _ => unreachable!("GenDispatcher has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            prop::GENLIST => self.gen_name_list = value,
            _ => unreachable!("GenDispatcher has no string-list property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            // Pascal `FWeights` is NIL until a GenList allocates it; a NIL array
            // dumps as "" (not "[]"), so report the empty list as absent.
            prop::WEIGHTS => (!self.weights.is_empty()).then_some(self.weights.as_slice()),
            _ => unreachable!("GenDispatcher has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::WEIGHTS => self.weights = value,
            _ => unreachable!("GenDispatcher has no double-array property {idx}"),
        }
    }
    /// Pascal `Weights` IndirectCount: the element count comes from the GenList
    /// (`PropertyOffset3 = @FGeneratorNameList`), so the array is sized by the
    /// number of generator names.
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::WEIGHTS => self.gen_name_list.len(),
            _ => unreachable!("GenDispatcher has no function-sized array {idx}"),
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
            _ => unreachable!("GenDispatcher has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TGenDispatcherObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            prop::KWBAND => self.half_kw_band = self.f_kw_band / 2.0,
            prop::GENLIST => {
                // Levelize the list.
                self.gen_pointer_list.clear(); // reset on first sample
                self.list_size = self.gen_name_list.len() as i32;
                self.weights = vec![1.0; self.list_size.max(0) as usize];
            }
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
    }
}

impl crate::elements::control::control_elem::ControlElem for GenDispatcher {
    fn ccd(&self) -> &crate::elements::control::control_elem::ControlElemData {
        &self.ccd
    }
    fn ccd_mut(&mut self) -> &mut crate::elements::control::control_elem::ControlElemData {
        &mut self.ccd
    }
    fn control_kind(&self) -> crate::elements::control::control_elem::ControlClass {
        crate::elements::control::control_elem::ControlClass::GenDispatch
    }
}
