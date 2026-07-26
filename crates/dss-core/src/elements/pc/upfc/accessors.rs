//! Trait impls for [`Upfc`]: `CktElement` (the series-`Xs` YPrim, the
//! input/output current injections, and the Monitor-mode-3 variable interface)
//! and `DssObject` (typed property accessors, the LossCurve/Element/spectrum
//! resolution, side effects, and `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;
use crate::util::EPSILON;

use super::{Upfc, prop};

impl CktElement for Upfc {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TUPFCObj.MakePosSequence` (UPFC.pas:1058-1060): an EMPTY body
    /// with NO `inherited` — the UPFC is left completely untouched (not even the
    /// base bus rename runs).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        PosSeqPlan::no_base()
    }

    /// Pascal `TUPFCObj.CalcYPrim` — build only the series block: the per-phase
    /// admittance `inv(j·Xs·freqmult)` stamped as a 2-terminal series matrix (the
    /// `Z = j·Xs` series Z is diagonal, so its inverse is the per-element inverse).
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;

        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        // Zinv = inv(j·Xs·freqmult). On a singular Z (Xs·freqmult == 0) Pascal
        // reports a matrix-inversion error and substitutes a large conductance.
        let z = Complex64::new(0.0, self.xs * freq_multiplier);
        let value = if z == Complex64::ZERO {
            Complex64::new(1.0 / EPSILON, 0.0)
        } else {
            z.inv()
        };

        let mut yp_series = CMatrix::new(yorder);
        for i in 0..nphases {
            yp_series.set(i, i, value);
            yp_series.set(i + nphases, i + nphases, value);
            yp_series.set(i, i + nphases, -value);
            yp_series.set(i + nphases, i, -value);
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_series);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim_shunt = None;
        self.cd.yprim = Some(yprim);

        self.cd.apply_yprim_open_conductor_calcs();
        self.cd.yprim_invalid = false;
    }

    /// Pascal `TUPFCObj.InjCurrents` → `GetInjCurrents` + `TPCElement.InjCurrents`
    /// (M3b compute half): cache `Vbin`/`Vbout`, fill `cd.inj_current`; the caller
    /// accumulates it into the system array through `node_ref`.
    fn compute_inj_currents(
        &mut self,
        _sys: &SysCtx,
        node_v: &[Complex64],
        _ctx: &mut InjComputeCtx,
    ) -> bool {
        self.get_inj_currents(node_v);
        false
    }

    /// Pascal `TUPFCObj.GetCurrents`: `Iterminal = YPrim·Vterminal` minus a
    /// freshly recomputed injection (into a local, mirroring Pascal's
    /// `ComplexBuffer` scratch — the solver's `InjCurrent` stays untouched).
    #[allow(clippy::needless_range_loop)] // loop-for-loop Pascal port
    fn get_currents(&mut self, _sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        let yorder = self.cd.yorder;
        if !self.cd.enabled || self.cd.node_ref.is_empty() {
            curr.iter_mut()
                .take(yorder)
                .for_each(|c| *c = Complex64::ZERO);
            return;
        }
        self.cd.compute_vterminal(node_v);
        if let Some(yprim) = &self.cd.yprim {
            yprim.mv_mult(curr, &self.cd.vterminal);
        }
        let inj = self.compute_inj_currents(node_v); // present value of inj currents
        for i in 0..yorder {
            curr[i] -= inj[i];
        }
    }

    fn num_variables(&self) -> usize {
        self.num_variables_impl()
    }
    fn variable_name(&self, i: usize) -> String {
        self.variable_name_impl(i)
    }
    /// Pascal `TUPFCObj.GetAllVariables` — the 14 reporting variables (no solution
    /// access; `Vbin`/`Vbout` were cached by the last `GetInjCurrents`).
    fn get_all_variables(&mut self, _sys: &SysCtx, _node_v: &[Complex64], states: &mut [f64]) {
        self.get_all_variables_impl(states);
    }
    fn set_variable(&mut self, i: usize, value: f64, _sys: &crate::elements::traits::SysCtx) {
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
}

impl Upfc {
    /// Pascal `TUPFCObj.MakeLike` (+ inherited `TPCElement.MakeLike`, which copies
    /// the spectrum). Faithful reproduction includes the upstream
    /// `UPFCLossCurveObj := UPFCLossCurveObj` self-assignment **bug** — the loss
    /// curve is *not* copied from the source, so a `like=` UPFC keeps its own
    /// (default-empty) curve.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nconds(self.cd.nphases); // Forces reallocation of terminal stuff
            self.cd.yprim_invalid = true;
        }
        self.v_ref = other.v_ref;
        self.pf = other.pf;
        self.xs = other.xs;
        self.tol1 = other.tol1;
        self.freq = other.freq;
        self.mode_upfc = other.mode_upfc;
        self.vpqmax = other.vpqmax;
        // UPFCLossCurveObj := UPFCLossCurveObj (upstream self-assignment no-op).
        self.vh_limit = other.vh_limit;
        self.vl_limit = other.vl_limit;
        self.c_limit = other.c_limit;
        self.v_ref2 = other.v_ref2;
        self.kvar_lim = other.kvar_lim;
        self.mon_elm = other.mon_elm;
        self.mon_elm_name = other.mon_elm_name.clone();
        // Inherited TPCElement.MakeLike: the spectrum.
        self.spectrum = other.spectrum.clone();
        self.spectrum_obj = other.spectrum_obj.clone();
        self.cd.base_frequency = other.cd.base_frequency;
        self.recalc();
    }
}

impl DssObject for Upfc {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            REFKV => self.v_ref,
            PF => self.pf,
            FREQUENCY => self.freq,
            XS => self.xs,
            TOL1 => self.tol1,
            VPQMAX => self.vpqmax,
            VHLIMIT => self.vh_limit,
            VLLIMIT => self.vl_limit,
            CLIMIT => self.c_limit,
            REFKV2 => self.v_ref2,
            KVARLIMIT => self.kvar_lim,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("UPFC has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            REFKV => self.v_ref = value,
            PF => self.pf = value,
            FREQUENCY => self.freq = value,
            XS => self.xs = value,
            TOL1 => self.tol1 = value,
            VPQMAX => self.vpqmax = value,
            VHLIMIT => self.vh_limit = value,
            VLLIMIT => self.vl_limit = value,
            CLIMIT => self.c_limit = value,
            REFKV2 => self.v_ref2 = value,
            KVARLIMIT => self.kvar_lim = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("UPFC has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            MODE => self.mode_upfc,
            _ => unreachable!("UPFC has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            MODE => self.mode_upfc = value,
            _ => unreachable!("UPFC has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.cd.enabled,
            _ => unreachable!("UPFC has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("UPFC has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            LOSSCURVE => self.loss_curve_name.clone(),
            ELEMENT => self.mon_elm_name.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("UPFC has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            // LossCurve/Element resolve through `set_object_ref`; only spectrum
            // (a name-only object ref) routes through here.
            prop::SPECTRUM => self.spectrum = value,
            _ => unreachable!("UPFC has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve the `LossCurve` (XYcurve snapshot clone) and `Element` (monitored
    /// circuit element, by full name) references.
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use prop::*;
        match idx {
            LOSSCURVE => {
                self.loss_curve_name = name;
                self.loss_curve_obj = resolved.and_then(|o| o.cloned::<XyCurveObj>());
            }
            ELEMENT => {
                self.mon_elm_name = name;
                self.mon_elm = resolved.map(|o| o.id());
            }
            _ => unreachable!("UPFC has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TUPFCObj.PropertySideEffects` — only `phases` has an effect (force
    /// terminal reallocation + resize the In/OutCurr injection vectors).
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        if idx == prop::PHASES && self.cd.nphases as i32 != prev_int {
            self.cd.set_nconds(self.cd.nphases); // NConds := Fnphases
            let n = self.cd.nphases;
            self.out_curr.resize(n, Complex64::ZERO);
            self.in_curr.resize(n, Complex64::ZERO);
        }
    }

    /// Pascal `TUPFC.EndEdit`: `RecalcElementData` + Yprim invalidation.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
