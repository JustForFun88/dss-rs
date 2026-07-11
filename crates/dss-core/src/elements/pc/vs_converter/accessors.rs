//! `impl DssObject` + `impl CktElement` for [`VsConverter`].

use num_complex::Complex64;

use super::{VsConverter, prop};
use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, InjCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;

impl CktElement for VsConverter {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TVSConverterObj.MakePosSequence` (VSConverter.pas:485-494): unless
    /// already a 2-phase (AC + DC) converter, force `Phases := 2` and `Ndc := 1`
    /// — TWO separate bare single edits (the upstream `//TODO: why two edits?`),
    /// each its own `RecalcElementData` — then `inherited` (the base bus rename).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        if self.cd.nphases != 2 {
            PosSeqPlan::with_actions(vec![
                PosSeqAction::SetI32(prop::PHASES, 2),
                PosSeqAction::SetI32(prop::NDC, 1),
            ])
        } else {
            PosSeqPlan::base()
        }
    }

    /// Pascal `TVSConverterObj.CalcYPrim` — build `YPrim_series` only: the AC
    /// admittance `cinv(Rac + j·Xac·freqmult)` as a 2-terminal series block on the
    /// first `phases − Ndc` (AC) conductors; the DC conductors stay zero.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases;
        let yorder = self.cd.yorder;

        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let value = Complex64::new(self.f_rac, self.f_xac * freq_multiplier).inv();

        let mut yp_series = CMatrix::new(yorder);
        for i in 0..(nphases - self.ndc) {
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

    /// Pascal `TVSConverterObj.InjCurrents`.
    fn inj_currents(&mut self, _sys: &SysCtx, ctx: &mut InjCtx) {
        self.get_inj_currents(ctx.node_v);
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
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

    /// Pascal `TVSConverterObj.GetCurrents`: `Yprim·V(node)` minus a **freshly
    /// recomputed** injection, saving the result into `LastCurrents`. The
    /// recompute goes into a local buffer — `cd.inj_current` (the solver's lag
    /// state) stays untouched, like Pascal's `GetInjCurrents(ComplexBuffer)`
    /// scratch call. (Pascal's version self-aliases `MVMult` over that scratch —
    /// the proven upstream reporting bug we deliberately do not reproduce; see
    /// `exec/tests/vs_converter.rs`.)
    #[allow(clippy::needless_range_loop, clippy::manual_memcpy)] // loop-for-loop port
    fn get_currents(&mut self, _sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        let yorder = self.cd.yorder;
        for i in 0..yorder {
            self.cd.vterminal[i] = node_v[self.cd.node_ref[i]];
        }
        if let Some(yprim) = &self.cd.yprim {
            yprim.mv_mult(curr, &self.cd.vterminal);
        }
        let inj = self.compute_inj_currents(node_v); // overwrites Vterminal, like the original
        for i in 0..yorder {
            curr[i] -= inj[i];
            self.last_currents[i] = curr[i];
        }
    }
}

impl DssObject for VsConverter {
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
            KVAC => self.f_kvac,
            KVDC => self.f_kvdc,
            KW => self.f_kw,
            RAC => self.f_rac,
            XAC => self.f_xac,
            M0 => self.fm,
            D0 => self.fd,
            MMIN => self.f_min_m,
            MMAX => self.f_max_m,
            IACMAX => self.f_max_iac,
            IDCMAX => self.f_max_idc,
            VACREF => self.f_ref_vac,
            PACREF => self.f_ref_pac,
            QACREF => self.f_ref_qac,
            VDCREF => self.f_ref_vdc,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("VSConverter has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KVAC => self.f_kvac = value,
            KVDC => self.f_kvdc = value,
            KW => self.f_kw = value,
            RAC => self.f_rac = value,
            XAC => self.f_xac = value,
            M0 => self.fm = value,
            D0 => self.fd = value,
            MMIN => self.f_min_m = value,
            MMAX => self.f_max_m = value,
            IACMAX => self.f_max_iac = value,
            IDCMAX => self.f_max_idc = value,
            VACREF => self.f_ref_vac = value,
            PACREF => self.f_ref_pac = value,
            QACREF => self.f_ref_qac = value,
            VDCREF => self.f_ref_vdc = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("VSConverter has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            NDC => self.ndc as i32,
            VSCMODE => self.f_mode,
            _ => unreachable!("VSConverter has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            NDC => self.ndc = value.max(0) as usize,
            VSCMODE => self.f_mode = value,
            _ => unreachable!("VSConverter has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.cd.enabled,
            _ => unreachable!("VSConverter has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("VSConverter has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("VSConverter has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::SPECTRUM => self.spectrum = value,
            _ => unreachable!("VSConverter has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TVSConverterObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;
        match idx {
            PHASES => {
                if self.cd.nphases as i32 != prev_int {
                    self.cd.set_nconds(self.cd.nphases); // NConds := Fnphases
                    self.cd.signal_bus_name_redefined = true;
                }
            }
            BUS1 => {
                // Default Bus2 to Bus1 with the node list zeroed (Bus1base.0.0…).
                let s = self.cd.get_bus(1).to_string();
                let mut s2 = match s.find('.') {
                    Some(dot) => s[..dot].to_string(),
                    None => s,
                };
                for _ in 0..self.cd.nphases {
                    s2.push_str(".0");
                }
                self.cd.set_bus(2, &s2);
            }
            _ => {}
        }
        // Pascal `case Idx of 1..16: YprimInvalid := TRUE`.
        if (1..=16).contains(&idx) {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TVSConverter.EndEdit`: `RecalcElementData` + Yprim invalidation.
    fn end_edit(&mut self) {
        self.recalc();
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TVSConverterObj.MakeLike` (+ inherited `TPCElement.MakeLike`,
    /// which copies the spectrum).
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<VsConverter>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nterms(other.cd.nterms);
            self.cd.set_nconds(self.cd.nphases); // NConds := Fnphases
            self.cd.yprim_invalid = true;
        }
        self.ndc = other.ndc;
        self.f_kvac = other.f_kvac;
        self.f_kvdc = other.f_kvdc;
        self.f_kw = other.f_kw;
        self.f_rac = other.f_rac;
        self.f_xac = other.f_xac;
        self.fm = other.fm;
        self.fd = other.fd;
        self.f_min_m = other.f_min_m;
        self.f_max_m = other.f_max_m;
        self.f_max_iac = other.f_max_iac;
        self.f_max_idc = other.f_max_idc;
        self.f_ref_vac = other.f_ref_vac;
        self.f_ref_pac = other.f_ref_pac;
        self.f_ref_qac = other.f_ref_qac;
        self.f_ref_vdc = other.f_ref_vdc;
        self.f_mode = other.f_mode;
        self.spectrum = other.spectrum.clone();
        self.spectrum_obj = other.spectrum_obj.clone();
        self.cd.base_frequency = other.cd.base_frequency;
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
        self.last_currents = vec![Complex64::ZERO; self.cd.yorder];
    }

    fn set_object_ref(
        &mut self,
        idx: usize,
        _name: String,
        _resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        // The only object-ref-typed property is `spectrum`, which the engine
        // resolves through `set_string` + `set_harmonic_spectrum` (like Generator
        // / VSource); nothing routes through here.
        unreachable!("VSConverter has no resolved object-ref property {idx}");
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
