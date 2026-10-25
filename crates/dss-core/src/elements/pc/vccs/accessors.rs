//! Trait impls: `CktElement` (the zero `YPrim` of an ideal current source, the
//! dynamics state-variable interface, and the injection/terminal currents) and
//! `DssObject` (typed property getters/setters, curve/spectrum resolution, side
//! effects, `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pos_seq::{
    PosSeqAction, PosSeqCtx, PosSeqLeg, PosSeqLegKind, PosSeqPlan, legs,
};
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;

use super::{CurveGap, Vccs, prop};

impl CktElement for Vccs {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// One leg per conductor to earth at its rated phase voltage. Its Y stamp
    /// holds no admittance.
    fn pos_seq_legs(&self) -> Option<Vec<PosSeqLeg>> {
        Some(legs::to_earth(
            self.cd.nphases,
            PosSeqLegKind::Lumped,
            self.base_volt,
            Complex64::ZERO,
        ))
    }

    /// Pascal `TVCCSObj.MakePosSequence` (vccs.pas:495-500): a multi-phase VCCS
    /// collapses to `Phases := 1` (a bare single edit), then `inherited` (the
    /// base bus rename).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        if self.cd.nphases > 1 {
            PosSeqPlan::with_actions(vec![PosSeqAction::SetI32(prop::PHASES, 1)])
        } else {
            PosSeqPlan::base()
        }
    }

    /// Pascal `TVCCSObj.CalcYPrim` — build only zero matrices (an ideal current
    /// source has `YPrim = 0`); the open-conductor pass adds the epsilon diagonal
    /// for any opened conductor.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        if self.cd.refuse_yprim_on_recalc() {
            return;
        }
        if sys.is_dynamic_model
            && let Some(input) = self.dynamics_curve_refused_input()
        {
            let msg = self.cd.refusal_message("VCCS", &input, "dynamic current");
            self.cd.refuse_yprim(msg);
            return;
        }
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

    fn has_dynamic_state(&self) -> bool {
        true
    }

    /// The waveform model needs `bp1`, `bp2` and `filter`, the RMS model needs
    /// `filter`, each a curve with points, and both a finite `fsample`. The
    /// waveform model also needs one sample in its one-cycle RMS window, so
    /// `fsample` at least the fundamental. Without them the element has no
    /// dynamics to integrate (`a_vccs_without_its_curves_refuses_the_dynamics_solve`),
    /// while a fault study, which reads only the seeded current, still takes it
    /// (`a_vccs_without_its_curves_takes_part_in_a_fault_study`).
    fn undefined_dynamics_model(&self) -> Option<String> {
        fn and_list(items: &[String]) -> String {
            match items {
                [] => String::new(),
                [a] => a.clone(),
                [init @ .., last] => format!("{} and {last}", init.join(", ")),
            }
        }
        let gaps = self.missing_curves();
        if gaps.is_empty() {
            if !self.fsample_freq.is_finite() {
                return Some(format!(
                    "VCCS.{} has no dynamics model: its fsample of {} Hz is not a finite number.",
                    self.cd.obj.name(),
                    self.fsample_freq
                ));
            }
            let window_has_a_sample = (self.fsample_freq / self.cd.base_frequency).trunc() >= 1.0;
            return (!self.frms_mode && !window_has_a_sample).then(|| {
                format!(
                    "VCCS.{} has no dynamics model: its fsample of {} Hz is below the fundamental of {} Hz, so its one-cycle RMS window holds no sample.",
                    self.cd.obj.name(),
                    self.fsample_freq,
                    self.cd.base_frequency
                )
            });
        }
        let mut unset = Vec::new();
        let mut empty = Vec::new();
        for gap in gaps {
            match gap {
                CurveGap::Unset(prop) => unset.push(prop.to_string()),
                CurveGap::Empty(prop, curve) => empty.push(format!("its {prop} XYcurve.{curve}")),
            }
        }
        let mut parts = Vec::new();
        if !unset.is_empty() {
            parts.push(format!("set {} before entering dynamics", and_list(&unset)));
        }
        if !empty.is_empty() {
            let verb = if empty.len() == 1 { "has" } else { "have" };
            parts.push(format!("{} {verb} no points", and_list(&empty)));
        }
        Some(format!(
            "VCCS.{} has no dynamics model: {}.",
            self.cd.obj.name(),
            parts.join(", and ")
        ))
    }

    /// Both models run their filter `fsample` times a second, a whole number of
    /// samples per step, and the VCCS documentation requires the time step to
    /// be an integer multiple of the sample step: a step of another length
    /// samples at the wrong rate, and one shorter than the sample step never
    /// samples and holds the seeded current. `h` and `fsample` each carry the
    /// rounding of their input, half an ulp, and their product one half ulp
    /// more, so a step of `N` sample steps reads as `N` within `1.5 ε N`. The
    /// rule accepts it within `4 ε N`, a stated margin over that bound for a
    /// unit or an RPN expression that adds a rounding
    /// (`a_vccs_steps_a_whole_number_of_samples`).
    fn step_refusal(&self, h: f64) -> Option<String> {
        let samples = h * self.fsample_freq;
        let whole = samples.round();
        if whole >= 1.0 && (samples - whole).abs() <= 4.0 * f64::EPSILON * whole {
            return None;
        }
        let model = if self.frms_mode { "RMS" } else { "waveform" };
        Some(format!(
            "VCCS.{} cannot take a step of {h} s: its fsample of {} Hz gives {samples} samples \
             per step, and the {model} model needs a whole number of them. Set stepsize to \
             an integer multiple of 1/fsample.",
            self.cd.obj.name(),
            self.fsample_freq
        ))
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

    /// Pascal `TVCCSObj.InjCurrents` + `TPCElement.InjCurrents` (M3b compute half):
    /// fill `cd.inj_current`; the caller accumulates it into the system array
    /// through `node_ref`.
    fn compute_inj_currents(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        _ctx: &mut InjComputeCtx,
    ) -> bool {
        self.get_inj_currents(sys, node_v);
        false
    }

    /// The terminal current is the negated injection, since an ideal current
    /// source has a zero primitive admittance. The injection is recomputed into
    /// a local, so the solver's injection vector stays untouched.
    ///
    /// A disabled or unmapped VCCS, and one outside the present dynamics run,
    /// carries no terminal current
    /// (`a_vccs_created_after_the_last_solve_reads_as_ground`).
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if !self.cd.enabled
            || !self.cd.is_mapped()
            || (sys.is_dynamic_model && !self.cd.dynamics_seeded())
        {
            curr.fill(Complex64::ZERO);
            return;
        }
        let inj = self.compute_inj_currents(sys, node_v); // present value of inj currents
        for (c, inj) in curr.iter_mut().zip(&inj).take(self.cd.yorder) {
            *c = -*inj;
        }
    }
}

impl Vccs {
    /// Pascal `TVCCSObj.MakeLike` (+ inherited `TPCElement.MakeLike`, which copies
    /// the spectrum). The curve **references** are copied (their names too, so the
    /// derived object's dump shows the same curve names); `RecalcElementData` then
    /// re-sizes the ring buffers from the copied filter (`end_edit`).
    pub(crate) fn make_like(&mut self, other: &Self) {
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
}

impl DssObject for Vccs {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
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
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use prop::*;
        let xy_curve = || resolved.and_then(|o| o.cloned::<XyCurveObj>());
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
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }
}
