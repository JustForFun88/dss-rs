//! Trait impls: `CktElement` (the zero `YPrim` of an ideal current source, the
//! dynamics state-variable interface, and the injection/terminal currents) and
//! `DssObject` (typed property getters/setters, curve/spectrum resolution, side
//! effects, `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::{CktElement, ElemRef, InjCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;

use super::{Vccs, prop};

impl CktElement for Vccs {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TVCCSObj.CalcYPrim` — build only zero matrices (an ideal current
    /// source has `YPrim = 0`); the open-conductor pass adds the epsilon diagonal
    /// for any opened conductor.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        self.cd.yprim_freq = sys.frequency;
        // YPrim = 0 for an ideal current source; just leave it zeroed.
        let yp_series = CMatrix::new(yorder);
        let yprim = CMatrix::new(yorder);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = None;
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Pascal `TVCCSObj.InitStateVars`.
    fn init_state_vars(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_state_vars_impl(sys, node_v);
    }

    /// Pascal `TVCCSObj.IntegrateStates`.
    fn integrate_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.integrate_states_impl(sys, node_v);
    }

    /// Pascal `TVCCSObj.NumVariables`.
    fn num_variables(&self) -> usize {
        self.num_variables_impl()
    }

    /// Pascal `TVCCSObj.VariableName`.
    fn variable_name(&self, i: usize) -> String {
        self.variable_name_impl(i)
    }

    /// Pascal `TVCCSObj.GetAllVariables` (the 6 raw `s1..s6`; no solution access).
    fn get_all_variables(&mut self, _sys: &SysCtx, _node_v: &[Complex64], states: &mut [f64]) {
        self.get_all_variables_impl(states);
    }

    /// Pascal `TVCCSObj.Set_Variable`.
    fn set_variable(&mut self, i: usize, value: f64) {
        self.set_variable_impl(i, value);
    }

    fn harmonic_spectrum(&self) -> Option<&SpectrumObj> {
        self.spectrum_obj.as_ref()
    }
    fn harmonic_spectrum_name(&self) -> Option<&str> {
        Some(&self.spectrum)
    }
    fn set_harmonic_spectrum(&mut self, spectrum: Option<SpectrumObj>) {
        self.spectrum_obj = spectrum;
    }

    /// Pascal `TVCCSObj.InjCurrents` + `TPCElement.InjCurrents`: fill `inj_current`
    /// then accumulate it into the system array through `node_ref`.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        self.get_inj_currents(sys, ctx.node_v);
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
    }

    /// Pascal `TVCCSObj.GetCurrents`: `Curr = -InjCurrent` (since `YPrim = 0`,
    /// `YPrim·V − InjCurrent` reduces to `−InjCurrent`). The recompute goes into
    /// a local (Pascal's `ComplexBuffer` scratch) — the solver's `InjCurrent`
    /// stays untouched.
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        let inj = self.compute_inj_currents(sys, node_v); // present value of inj currents
        for (c, inj) in curr.iter_mut().zip(&inj).take(self.cd.yorder) {
            *c = -*inj;
        }
    }
}

impl DssObject for Vccs {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
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

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            PRATED => self.prated,
            VRATED => self.vrated,
            PPCT => self.ppct,
            FSAMPLE => self.fsample_freq,
            IMAXPU => self.fmax_ipu,
            VRMSTAU => self.fvrms_tau,
            IRMSTAU => self.firms_tau,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("VCCS has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            PRATED => self.prated = value,
            VRATED => self.vrated = value,
            PPCT => self.ppct = value,
            FSAMPLE => self.fsample_freq = value,
            IMAXPU => self.fmax_ipu = value,
            VRMSTAU => self.fvrms_tau = value,
            IRMSTAU => self.firms_tau = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("VCCS has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::PHASES => self.cd.nphases as i32,
            _ => unreachable!("VCCS has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::PHASES => self.cd.nphases = value.max(0) as usize,
            _ => unreachable!("VCCS has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            RMSMODE => self.frms_mode,
            ENABLED => self.cd.enabled,
            _ => unreachable!("VCCS has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            RMSMODE => self.frms_mode = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("VCCS has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            BP1 => self.bp1_name.clone(),
            BP2 => self.bp2_name.clone(),
            FILTER => self.filter_name.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("VCCS has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            BP1 => self.bp1_name = value,
            BP2 => self.bp2_name = value,
            FILTER => self.filter_name = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("VCCS has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve the `bp1`/`bp2`/`filter` XYcurve references (snapshot-clone, like
    /// the PVSystem curve refs).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use prop::*;
        let xy_curve =
            || resolved.and_then(|(_, o)| o.as_any().downcast_ref::<XyCurveObj>().cloned());
        match idx {
            BP1 => {
                self.bp1_name = name;
                self.fbp1 = xy_curve();
            }
            BP2 => {
                self.bp2_name = name;
                self.fbp2 = xy_curve();
            }
            FILTER => {
                self.filter_name = name;
                self.ffilter = xy_curve();
            }
            _ => unreachable!("VCCS has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TVCCSObj.PropertySideEffects` — only `phases` has an effect (force
    /// reallocation of the terminal info via `NConds := Fnphases`).
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        if idx == prop::PHASES && self.cd.nphases as i32 != prev_int {
            self.cd.set_nconds(self.cd.nphases);
        }
    }

    /// Pascal `TVCCS.EndEdit`: `RecalcElementData` + Yprim invalidation.
    fn end_edit(&mut self) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TVCCSObj.MakeLike` (+ inherited `TPCElement.MakeLike`, which copies
    /// the spectrum). The curve **references** are copied (their names too, so the
    /// derived object's dump shows the same curve names); `RecalcElementData` then
    /// re-sizes the ring buffers from the copied filter (`end_edit`).
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Vccs>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nconds(self.cd.nphases); // NConds := Fnphases
            self.cd.yprim_invalid = true;
        }
        self.prated = other.prated;
        self.vrated = other.vrated;
        self.ppct = other.ppct;
        self.bp1_name = other.bp1_name.clone();
        self.bp2_name = other.bp2_name.clone();
        self.filter_name = other.filter_name.clone();
        self.fbp1 = other.fbp1.clone();
        self.fbp2 = other.fbp2.clone();
        self.ffilter = other.ffilter.clone();
        self.fsample_freq = other.fsample_freq;
        self.frms_mode = other.frms_mode;
        self.fmax_ipu = other.fmax_ipu;
        self.fvrms_tau = other.fvrms_tau;
        self.firms_tau = other.firms_tau;
        // Inherited TPCElement.MakeLike: the spectrum.
        self.spectrum = other.spectrum.clone();
        self.spectrum_obj = other.spectrum_obj.clone();
        self.cd.base_frequency = other.cd.base_frequency;
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
