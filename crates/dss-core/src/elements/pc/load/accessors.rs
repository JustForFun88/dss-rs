//! The [`CktElement`] and [`DssObject`] trait impls for `TLoadObj`: the
//! `CalcYPrim`/`InjCurrents`/`GetCurrents` circuit-element surface, the typed
//! property getters/setters, shape-reference resolution + snapshot, the
//! `PropertySideEffects` parse-time bookkeeping, `EndEdit`, and `MakeLike`.

use num_complex::Complex64;

use crate::elements::ckt::{CktElementData, ElemFlags};
use crate::elements::general::growth_shape::GrowthShapeObj;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

use super::{Connection, Load, LoadModel, LoadSpec, LoadStatus, nconds_for_connection, prop};

impl CktElement for Load {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn load_num_customers(&self) -> Option<i32> {
        Some(self.num_customers)
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys);
    }

    /// Pascal `TLoadObj.MakePosSequence` (`Load.pas:2215`). Convert to a single
    /// phase, line-neutral wye load carrying one third of the total power.
    ///
    /// TODO(compat): the power divisor is a hard-coded `3.0`, NOT `Fnphases`
    /// (upstream "assume load is distributed equally among the 3 phases", RCD
    /// 2016). A second `makeposseq` therefore divides again (400 → 133.33 →
    /// 44.44), pinned by `tests/corpus/modes/makeposseq/makeposseq_pc.dss`.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        use super::prop;

        // Make sure voltage is line-neutral.
        let v = if self.cd.nphases > 1 || self.connection != Connection::Wye {
            self.kv_load_base / sqrt3()
        } else {
            self.kv_load_base
        };

        let new_kw = self.kw_base / 3.0;
        let new_kvar = self.kvar_base / 3.0;
        let new_kva = self.connected_kva / 3.0; // ConnectedKVA (== XfkVA)

        let mut actions = vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, v),
            PosSeqAction::SetF64(prop::KW, new_kw),
            PosSeqAction::SetF64(prop::KVAR, new_kvar),
        ];
        if new_kva > 0.0 {
            actions.push(PosSeqAction::SetF64(prop::XFKVA, new_kva));
        }
        actions.push(PosSeqAction::EndEdit);

        PosSeqPlan::with_actions(actions)
    }

    /// Pascal `TLoadObj.CalcYPrim`.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let mut yp_shunt = CMatrix::new(yorder);

        // POWERFLOW and ADMITTANCE both use the admittance matrix here.
        self.set_nominal_load(sys);
        self.calc_yprim_matrix(&mut yp_shunt, sys);

        // YPrim_Series from the shunt diagonal so CalcVoltages doesn't fail.
        let mut yp_series = CMatrix::new(yorder);
        for i in 0..yorder {
            yp_series.set(i, i, yp_shunt.get(i, i) * 1.0e-10);
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_shunt);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim = Some(yprim);
        self.cd.inj_current = vec![Complex64::ZERO; yorder];

        self.cd.apply_yprim_open_conductor_calcs();
    }

    /// Pascal `TLoadObj.InjCurrents` + `TPCElement.InjCurrents` (M3b compute
    /// half; the caller scatters `cd.inj_current`).
    fn compute_inj_currents(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        _ctx: &mut InjComputeCtx,
    ) -> bool {
        if !self.cd.enabled {
            return false;
        }
        if sys.loads_need_updating {
            self.set_nominal_load(sys);
        }
        // r4133 `TLoadObj.InjCurrents` (Load.pas:1922): `if not ForceInjCurr then
        // CalcInjCurrentArray` — a forced injection (`Set InjCurrent=`/`ITerminal=`)
        // keeps its stored `inj_current` while the set-nominal preamble above still
        // runs; the inherited add-into-Currents is unconditional (caller scatter).
        let mut errors = crate::diag::ErrorLog::new();
        if !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS) {
            self.calc_inj_current_array(sys, node_v, &mut errors);
        }
        false
    }

    fn init_harmonics(&mut self, sys: &SysCtx, _node_v: &[Complex64]) {
        self.init_harmonics(sys);
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

    /// Pascal `TPCElement.GetCurrents` + `TLoadObj.GetTerminalCurrents`.
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if !self.cd.enabled {
            curr.fill(Complex64::ZERO);
            return;
        }
        // Pascal `TPCElement.GetCurrents` l.137: after a direct solve (and not
        // in dynamics/harmonics) take the `CalcYPrimContribution` shortcut —
        // the model is entirely in the Y matrix, so report `YPrim · Vterminal`
        // (NOT the load-model compensation current).
        if sys.pc_direct_shortcut() {
            self.cd.calc_yprim_contribution(node_v, curr);
            return;
        }
        // Pascal `TLoadObj.GetTerminalCurrents` (@ 0.15.0b4): recompute the load
        // model contribution unless the terminal currents were forced from the
        // DSS language (`Set InjCurrent=`/`Set ITerminal=`, `Flg.ForceInjCurrents`,
        // WP-U1.9) — then the stored/forced `ITerminal` is used as-is.
        if !self.cd.iterminal_solved_for(sys.solution_count)
            && !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS)
        {
            let mut errors = crate::diag::ErrorLog::new();
            self.calc_load_model_contribution(sys, node_v, &mut errors);
        }
        // TPCElement.GetTerminalCurrents
        if self.cd.iterminal_updated {
            curr.copy_from_slice(&self.cd.iterminal[..curr.len()]);
        } else {
            let cd = &mut self.cd;
            if let Some(yprim) = &cd.yprim {
                yprim.mv_mult(curr, &cd.vterminal);
            }
            for (i, c) in curr.iter_mut().enumerate() {
                *c -= cd.inj_current[i];
            }
            cd.iterminal_updated = true;
        }
        self.cd.mark_iterminal_solved(sys.solution_count);
    }
}

impl Load {
    /// Pascal `TLoadObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        self.connection = other.connection;
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = nconds_for_connection(self.connection, self.cd.nphases);
            self.cd.set_nconds(n);
            self.cd.yprim_invalid = true;
        }
        self.kv_load_base = other.kv_load_base;
        self.v_base = other.v_base;
        self.vlowpu = other.vlowpu;
        self.vminpu = other.vminpu;
        self.vmaxpu = other.vmaxpu;
        self.v_base_low = other.v_base_low;
        self.v_base95 = other.v_base95;
        self.v_base105 = other.v_base105;
        self.kw_base = other.kw_base;
        self.kva_base = other.kva_base;
        self.kvar_base = other.kvar_base;
        self.load_spec_type = other.load_spec_type;
        self.w_nominal = other.w_nominal;
        self.pf_nominal = other.pf_nominal;
        self.var_nominal = other.var_nominal;
        self.rneut = other.rneut;
        self.xneut = other.xneut;
        self.cvr_shape = other.cvr_shape.clone();
        self.daily_shape = other.daily_shape.clone();
        self.duty_shape = other.duty_shape.clone();
        self.yearly_shape = other.yearly_shape.clone();
        self.growth_shape = other.growth_shape.clone();
        self.spectrum = other.spectrum.clone();
        // Pascal copies the resolved shape pointers (CVR/Daily/Duty/Yearly/
        // Growth) and the spectrum; here that is the snapshot clone + its ElemId.
        self.spectrum_obj = other.spectrum_obj.clone();
        self.cvr_shape_obj = other.cvr_shape_obj.clone();
        self.daily_shape_obj = other.daily_shape_obj.clone();
        self.duty_shape_obj = other.duty_shape_obj.clone();
        self.yearly_shape_obj = other.yearly_shape_obj.clone();
        self.growth_shape_obj = other.growth_shape_obj.clone();
        self.cvr_shape_ref = other.cvr_shape_ref;
        self.daily_shape_ref = other.daily_shape_ref;
        self.duty_shape_ref = other.duty_shape_ref;
        self.yearly_shape_ref = other.yearly_shape_ref;
        self.growth_shape_ref = other.growth_shape_ref;
        self.load_class = other.load_class;
        self.num_customers = other.num_customers;
        self.load_model = other.load_model;
        self.status = other.status;
        self.kva_allocation_factor = other.kva_allocation_factor;
        self.connected_kva = other.connected_kva;
        self.cvr_watt_factor = other.cvr_watt_factor;
        self.cvr_var_factor = other.cvr_var_factor;
        self.shape_is_actual = other.shape_is_actual;
        self.pu_series_rl = other.pu_series_rl;
        self.rel_weighting = other.rel_weighting;
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
        self.phase_curr = vec![Complex64::ZERO; self.cd.nphases];
        self.zipv_set = other.zipv_set;
        if self.zipv_set {
            self.zipv = other.zipv;
        }
    }
}

impl DssObject for Load {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            KV => self.kv_load_base,
            KW => self.kw_base,
            PF => self.pf_nominal,
            KVAR => self.kvar_base,
            RNEUT => self.rneut,
            XNEUT => self.xneut,
            VMINPU => self.vminpu,
            VMAXPU => self.vmaxpu,
            VMINNORM => self.vmin_normal,
            VMINEMERG => self.vmin_emerg,
            XFKVA => self.connected_kva,
            ALLOCATIONFACTOR => self.kva_allocation_factor,
            KVA => self.kva_base,
            PCTMEAN => self.pu_mean,
            PCTSTDDEV => self.pu_std_dev,
            CVRWATTS => self.cvr_watt_factor,
            CVRVARS => self.cvr_var_factor,
            KWH => self.kwh,
            KWHDAYS => self.kwh_days,
            CFACTOR => self.c_factor,
            PCT_SERIES_RL => self.pu_series_rl,
            REL_WEIGHT => self.rel_weighting,
            VLOWPU => self.vlowpu,
            PUXHARM => self.pu_x_harm,
            XRHARM => self.xr_harm_ratio,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Load has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kv_load_base = value,
            KW => self.kw_base = value,
            PF => self.pf_nominal = value,
            KVAR => self.kvar_base = value,
            RNEUT => self.rneut = value,
            XNEUT => self.xneut = value,
            VMINPU => self.vminpu = value,
            VMAXPU => self.vmaxpu = value,
            VMINNORM => self.vmin_normal = value,
            VMINEMERG => self.vmin_emerg = value,
            XFKVA => self.connected_kva = value,
            ALLOCATIONFACTOR => self.kva_allocation_factor = value,
            KVA => self.kva_base = value,
            PCTMEAN => self.pu_mean = value,
            PCTSTDDEV => self.pu_std_dev = value,
            CVRWATTS => self.cvr_watt_factor = value,
            CVRVARS => self.cvr_var_factor = value,
            KWH => self.kwh = value,
            KWHDAYS => self.kwh_days = value,
            CFACTOR => self.c_factor = value,
            PCT_SERIES_RL => self.pu_series_rl = value,
            REL_WEIGHT => self.rel_weighting = value,
            VLOWPU => self.vlowpu = value,
            PUXHARM => self.pu_x_harm = value,
            XRHARM => self.xr_harm_ratio = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Load has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            MODEL => self.load_model as i32,
            CONN => self.connection as i32,
            STATUS => self.status.ordinal(),
            CLS => self.load_class,
            NUMCUST => self.num_customers,
            _ => unreachable!("Load has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            MODEL => self.load_model = LoadModel::from_i32(value),
            CONN => {
                self.connection = if value == 1 {
                    Connection::Delta
                } else {
                    Connection::Wye
                }
            }
            STATUS => self.status = LoadStatus::from_ordinal(value).unwrap_or(self.status),
            CLS => self.load_class = value,
            NUMCUST => self.num_customers = value,
            _ => unreachable!("Load has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.cd.enabled,
            _ => unreachable!("Load has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Load has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            GROWTH => self.growth_shape.clone(),
            CVRCURVE => self.cvr_shape.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("Load has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape = value,
            DAILY => self.daily_shape = value,
            DUTY => self.duty_shape = value,
            GROWTH => self.growth_shape = value,
            CVRCURVE => self.cvr_shape = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("Load has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::ZIPV => Some(&self.zipv),
            _ => unreachable!("Load has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::ZIPV => {
                for (dst, src) in self.zipv.iter_mut().zip(value) {
                    *dst = src;
                }
            }
            _ => unreachable!("Load has no array property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape reference: store the resolved object's name (for the
    /// dump), its `ElemId`, and a snapshot clone of the object that
    /// `SetNominalLoad` drives through `GetMultAtHour` (Pascal stores the live
    /// pointer; see the `*_shape_obj` field doc). `daily`/`yearly`/`duty`/
    /// `CVRcurve` resolve to `LoadShape`, `growth` to `GrowthShape`.
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use prop::*;
        let elem_ref = resolved.map(|o| o.id());
        let load_shape = || resolved.and_then(|o| o.cloned::<LoadShapeObj>());
        match idx {
            YEARLY => {
                self.yearly_shape = name;
                self.yearly_shape_ref = elem_ref;
                self.yearly_shape_obj = load_shape();
            }
            DAILY => {
                self.daily_shape = name;
                self.daily_shape_ref = elem_ref;
                self.daily_shape_obj = load_shape();
            }
            DUTY => {
                self.duty_shape = name;
                self.duty_shape_ref = elem_ref;
                self.duty_shape_obj = load_shape();
            }
            CVRCURVE => {
                self.cvr_shape = name;
                self.cvr_shape_ref = elem_ref;
                self.cvr_shape_obj = load_shape();
            }
            GROWTH => {
                self.growth_shape = name;
                self.growth_shape_ref = elem_ref;
                self.growth_shape_obj = resolved.and_then(|o| o.cloned::<GrowthShapeObj>());
            }
            _ => unreachable!("Load has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TLoadObj.PropertySideEffects` (text-parser path: every edit
    /// invalidates Yprim through `EndEdit`).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            CONN => {
                let n = nconds_for_connection(self.connection, self.cd.nphases);
                self.cd.set_nconds(n);
                self.update_vbase();
                self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
                self.cd.yprim_invalid = true;
            }
            KV | PHASES => {
                if idx == PHASES {
                    self.phase_curr = vec![Complex64::ZERO; self.cd.nphases];
                    let n = nconds_for_connection(self.connection, self.cd.nphases);
                    self.cd.set_nconds(n); // force reallocation of terminal info
                }
                self.update_vbase();
            }
            KW => {
                self.load_spec_type = LoadSpec::KwPf;
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(XFKVA);
                self.cd.obj.clear_seq(KWH);
                self.kw_ref = self.kw_base;
            }
            PF => {
                self.pf_changed = true;
                self.pf_specified = true;
                self.cd.obj.clear_seq(KVAR);
            }
            ALLOCATIONFACTOR => {
                self.allocation_factor = self.kva_allocation_factor;
                self.load_spec_type = LoadSpec::ConnectedKvaPf;
                self.cd.obj.set_as_next_seq(XFKVA);
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(KWH);
                self.compute_allocated_load();
                self.has_been_allocated = true;
            }
            CFACTOR => {
                self.allocation_factor = self.c_factor;
                self.load_spec_type = LoadSpec::KwhPf;
                self.cd.obj.set_as_next_seq(KWH);
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(XFKVA);
                self.compute_allocated_load();
                self.has_been_allocated = true;
            }
            // Shape side effects (Pascal): a `UseActual` shape sets kW/kvar to
            // its peak demand, and `daily` seeds an unset `yearly`.
            YEARLY => {
                let actual = self
                    .yearly_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| (s.max_p(), s.max_q()));
                if let Some((mp, mq)) = actual {
                    self.kw_ref = self.kw_base;
                    self.kvar_ref = self.kvar_base;
                    self.set_kw_kvar(mp, mq);
                }
            }
            DAILY => {
                let actual = self
                    .daily_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| (s.max_p(), s.max_q()));
                if let Some((mp, mq)) = actual {
                    self.set_kw_kvar(mp, mq);
                }
                // If the yearly shape is not yet defined, make it the daily one
                // (Pascal `YearlyShapeObj := DailyShapeObj`; the ObjectRef getter
                // then renders the daily name, so mirror it for the dump too).
                if self.yearly_shape_obj.is_none() {
                    self.yearly_shape_obj = self.daily_shape_obj.clone();
                    self.yearly_shape_ref = self.daily_shape_ref;
                    self.yearly_shape = self.daily_shape.clone();
                }
            }
            DUTY => {
                let actual = self
                    .duty_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| (s.max_p(), s.max_q()));
                if let Some((mp, mq)) = actual {
                    self.set_kw_kvar(mp, mq);
                }
            }
            KWH => {
                self.load_spec_type = LoadSpec::KwhPf;
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(XFKVA);
                self.allocation_factor = self.c_factor;
                self.compute_allocated_load();
            }
            KVAR => {
                self.load_spec_type = LoadSpec::KwKvar;
                if !self.cd.obj.prp_specified(KW) {
                    self.cd.obj.set_as_next_seq(KW);
                }
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(PF);
                self.cd.obj.clear_seq(KWH);
                self.cd.obj.clear_seq(XFKVA);
                self.pf_specified = false;
                self.kvar_ref = self.kvar_base;
            }
            XFKVA => {
                self.load_spec_type = LoadSpec::ConnectedKvaPf;
                self.cd.obj.set_as_next_seq(XFKVA);
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(KWH);
                self.allocation_factor = self.kva_allocation_factor;
                self.compute_allocated_load();
            }
            KWHDAYS => {
                self.load_spec_type = LoadSpec::KwhPf;
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(XFKVA);
                self.compute_allocated_load();
            }
            KVA => {
                self.load_spec_type = LoadSpec::KvaPf;
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KWH);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(XFKVA);
            }
            ZIPV => self.zipv_set = true,
            _ => {}
        }
    }

    /// Pascal `TLoad.EndEdit`: `RecalcElementData` + Yprim invalidation. `sys` is
    /// the LIVE circuit/solution the executive holds at the edit site (Pascal
    /// `SetNominalLoad` reads `ActiveCircuit.Solution` globals).
    fn end_edit(&mut self, sys: &crate::elements::traits::SysCtx) {
        self.recalc(sys);
        self.cd.yprim_invalid = true;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
