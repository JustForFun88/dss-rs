//! Trait impls: `CktElement` (Yprim build, injection/terminal currents,
//! dynamics hooks) and `DssObject` (typed property getters/setters, shape/curve
//! resolution, side effects, `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::dynamic_exp::DynamicExpObj;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pc::dyneq_pce::{DynEqPce, DynEqPceData};
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, InjCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

use super::{Connection, WindGen, default_recalc_ctx, nconds_for_connection, prop};

impl CktElement for WindGen {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys);
    }

    /// Pascal `TWindGenObj.MakePosSequence` (`WindGen.pas:2184`). Single phase,
    /// line-neutral; a multi-phase WindGen's power is divided by the phase count
    /// (PF preserved), and — conditionally — its kVA/MVA ratings. Unlike
    /// Generator, the `had_kVA`/`had_MVA` guards read the CORRECT `kVA`/`MVA`
    /// ordinals (16/17), so there is no upstream index bug to reproduce.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        let v = if self.cd.nphases > 1 || self.connection != Connection::Wye {
            self.kv_windgen_base / sqrt3()
        } else {
            self.kv_windgen_base
        };

        let old_phases = self.cd.nphases;
        let mut actions = vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, v),
        ];

        if old_phases > 1 {
            let nph = self.cd.nphases as f64;
            let had_kva = self.cd.obj.prp_specified(prop::KVA);
            let had_mva = self.cd.obj.prp_specified(prop::MVA);
            let kw_new = self.kw_base / nph;
            let pf_new = self.pf_nominal;
            actions.push(PosSeqAction::SetF64(prop::KW, kw_new));
            actions.push(PosSeqAction::SetF64(prop::PF, pf_new));
            if had_kva {
                actions.push(PosSeqAction::SetF64(prop::KVA, self.kva_rating / nph));
            }
            if had_mva {
                actions.push(PosSeqAction::SetF64(
                    prop::MVA,
                    self.kva_rating / 1000.0 / nph,
                ));
            }
        }

        actions.push(PosSeqAction::EndEdit);
        PosSeqPlan::with_actions(actions)
    }

    /// Pascal `TWindGenObj.CalcYPrim`.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let mut yp_shunt = CMatrix::new(yorder);

        // No live solution at Yprim build (the volt-var branch reads no voltages).
        self.set_nominal_generation(sys, &[]);
        self.calc_yprim_matrix(&mut yp_shunt, sys);

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

    /// Pascal `TWindGenObj.InitHarmonics` — disabled upstream (loud abort).
    fn init_harmonics(&mut self, _sys: &SysCtx, _node_v: &[Complex64]) {
        self.init_harmonics_impl();
    }

    /// Pascal `TWindGenObj.InitStateVars`.
    fn init_state_vars(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_state_vars_impl(sys, node_v);
    }

    /// Pascal `TWindGenObj.IntegrateStates`.
    fn integrate_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.integrate_states_impl(sys, node_v);
    }

    /// Pascal `TWindGenObj.NumVariables`: the linked `DynamicExp` count first,
    /// else the 22 classic WindGen variables.
    fn num_variables(&self) -> usize {
        let n = self.dyneq.num_variables();
        if n != 0 { n } else { self.num_wgen_variables() }
    }

    /// Pascal `TWindGenObj.VariableName`: the `DynamicExp` name first, else the
    /// classic name table.
    fn variable_name(&self, i: usize) -> String {
        if let Some(name) = self.dyneq.variable_name(i) {
            return name;
        }
        self.wgen_variable_name(i)
    }

    /// Pascal `TWindGenObj.GetAllVariables`: the `DynamicExp` memory dump first,
    /// else the classic WindGen variables.
    fn get_all_variables(&mut self, _sys: &SysCtx, _node_v: &[Complex64], states: &mut [f64]) {
        if self.dyneq.has_dynamic_eq() {
            for (i, s) in states
                .iter_mut()
                .enumerate()
                .take(self.dyneq.num_variables())
            {
                *s = self.dyneq.get_dynamic_eq_val(i);
            }
            return;
        }
        self.get_wgen_variables(states);
    }

    /// Pascal `TWindGenObj.SetVariable`.
    fn set_variable(&mut self, i: usize, value: f64) {
        if i < 1 {
            return;
        }
        if self.dyneq.has_dynamic_eq() {
            self.cd.obj.push_error(format!(
                "WindGen.{}: cannot set state variable when using DynamicEq.",
                self.cd.obj.name()
            ));
            return;
        }
        self.set_wgen_variable(i, value);
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

    /// Pascal `TWindGenObj.InjCurrents` + `TPCElement.InjCurrents`.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        if !self.cd.enabled {
            return;
        }
        if sys.loads_need_updating {
            self.set_nominal_generation(sys, ctx.node_v);
        }
        let mut errors = Vec::new();
        self.calc_inj_current_array(sys, ctx.node_v, &mut errors);
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
    }

    /// Pascal `TWindGenObj.GetTerminalCurrents` + `TPCElement` base.
    ///
    /// NOTE(upstream-quirk, not reproduced): the WindGen override adds an `else
    /// inherited GetTerminalCurrents` (`WindGen.pas:1698`, its own `TODO: BUG`
    /// comment) that Generator/Load/PVSystem lack — so on a *stale*
    /// (`IterminalSolutionCount <> SolutionCount`) call it recomputes the model
    /// but never fills `Curr`, returning the caller's uninitialised buffer
    /// (nondeterministic). The port fills `Curr` correctly (mirroring Generator);
    /// in the normal post-solve path the counts match and both engines fill from
    /// the cached `Iterminal`, so the divergence is unobservable.
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if !self.cd.enabled {
            curr.fill(Complex64::ZERO);
            return;
        }
        if sys.pc_direct_shortcut() {
            self.cd.calc_yprim_contribution(node_v, curr);
            return;
        }
        if self.cd.iterminal_solution_count != sys.solution_count && !self.gen_switch_open {
            let mut errors = Vec::new();
            self.calc_gen_model_contribution(sys, node_v, &mut errors);
        }
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
        self.cd.iterminal_solution_count = sys.solution_count;
    }
}

impl DssObject for WindGen {
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
            KV => self.kv_windgen_base,
            KW => self.kw_base,
            PF => self.pf_nominal,
            KVAR => self.kvar_base,
            VMINPU => self.vminpu,
            VMAXPU => self.vmaxpu,
            KVA | MVA => self.kva_rating,
            DUTYSTART => self.duty_start,
            RTHEV => self.wind_model_dyn.zthev.re,
            XTHEV => self.wind_model_dyn.zthev.im,
            VSS => self.wind_model_dyn.vss,
            PSS => self.wind_model_dyn.pss,
            QSS => self.wind_model_dyn.qss,
            VWIND => self.wind_model_dyn.vwind,
            DELT0 => self.wind_model_dyn.delt0,
            AG => self.ag,
            CP => self.cp,
            LAMDA => self.lamda,
            P => self.poles,
            PD => self.pd,
            RAD => self.rad,
            VCUTIN => self.v_cut_in,
            VCUTOUT => self.v_cut_out,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("WindGen has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kv_windgen_base = value,
            KW => self.kw_base = value,
            PF => self.pf_nominal = value,
            KVAR => self.kvar_base = value,
            VMINPU => self.vminpu = value,
            VMAXPU => self.vmaxpu = value,
            KVA | MVA => self.kva_rating = value,
            DUTYSTART => self.duty_start = value,
            RTHEV => self.wind_model_dyn.zthev.re = value,
            XTHEV => self.wind_model_dyn.zthev.im = value,
            VSS => self.wind_model_dyn.vss = value,
            PSS => self.wind_model_dyn.pss = value,
            QSS => self.wind_model_dyn.qss = value,
            VWIND => self.wind_model_dyn.vwind = value,
            DELT0 => self.wind_model_dyn.delt0 = value,
            AG => self.ag = value,
            CP => self.cp = value,
            LAMDA => self.lamda = value,
            P => self.poles = value,
            PD => self.pd = value,
            RAD => self.rad = value,
            VCUTIN => self.v_cut_in = value,
            VCUTOUT => self.v_cut_out = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("WindGen has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            MODEL => self.gen_model,
            CONN => self.connection as i32,
            CLS => self.gen_class,
            QMODE => self.wind_model_dyn.q_mode,
            SIMMECHFLG => self.wind_model_dyn.sim_mech_flg,
            APCFLG => self.wind_model_dyn.apc_flg,
            QFLG => self.wind_model_dyn.q_flg,
            N_WTG => self.wind_model_dyn.n_wtg,
            _ => unreachable!("WindGen has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            MODEL => self.gen_model = value,
            CONN => {
                self.connection = if value == 1 {
                    Connection::Delta
                } else {
                    Connection::Wye
                }
            }
            CLS => self.gen_class = value,
            QMODE => self.wind_model_dyn.q_mode = value,
            SIMMECHFLG => self.wind_model_dyn.sim_mech_flg = value,
            APCFLG => self.wind_model_dyn.apc_flg = value,
            QFLG => self.wind_model_dyn.q_flg = value,
            N_WTG => self.wind_model_dyn.n_wtg = value,
            _ => unreachable!("WindGen has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            DEBUGTRACE => self.wind_model_dyn.debug_trace,
            ENABLED => self.cd.enabled,
            _ => unreachable!("WindGen has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            DEBUGTRACE => self.wind_model_dyn.debug_trace = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("WindGen has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            DYNAMICEQ => self.dyneq.dynamic_eq.clone(),
            VV_CURVE => self.vv_curve.clone(),
            PLOSS => self.loss_curve.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("WindGen has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("WindGen has no string property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::DYNOUT => self.dyneq.get_dyn_output_names(),
            _ => unreachable!("WindGen has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            prop::DYNOUT => {
                let errors = self.dyneq.set_dyn_output_names(&value);
                for e in errors {
                    self.cd.obj.push_error(e);
                }
            }
            _ => unreachable!("WindGen has no string-list property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape / curve / dynamic-equation reference (snapshot-clone).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use prop::*;
        let elem_ref = resolved.map(|(r, _)| r);
        let load_shape =
            || resolved.and_then(|(_, o)| o.as_any().downcast_ref::<LoadShapeObj>().cloned());
        let xy_curve =
            || resolved.and_then(|(_, o)| o.as_any().downcast_ref::<XyCurveObj>().cloned());
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
            DYNAMICEQ => {
                self.dyneq.dynamic_eq = name;
                self.dyneq.dynamic_eq_ref = elem_ref;
                self.dyneq.dynamic_eq_obj =
                    resolved.and_then(|(_, o)| o.as_any().downcast_ref::<DynamicExpObj>().cloned());
            }
            VV_CURVE => {
                self.vv_curve = name;
                self.vv_curve_ref = elem_ref;
                self.vv_curve_obj = xy_curve();
            }
            PLOSS => {
                self.loss_curve = name;
                self.loss_curve_ref = elem_ref;
                self.loss_curve_obj = xy_curve();
            }
            _ => unreachable!("WindGen has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TWindGenObj.PropertySideEffects` (text-parser path).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            CONN => {
                let n = nconds_for_connection(self.connection, self.cd.nphases);
                self.cd.set_nconds(n);
                self.update_vbase();
                self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
            }
            KV => self.update_vbase(),
            KVAR => self.side_effect_kvar(),
            PHASES => {
                let n = nconds_for_connection(self.connection, self.cd.nphases);
                self.cd.set_nconds(n); // force reallocation of terminal info
            }
            KW | PF => {
                self.sync_up_power_quantities();
                if idx == PF {
                    self.cd.obj.clear_seq(KVAR);
                }
            }
            KVA | MVA => {
                self.wind_model_dyn.rated_kva = self.kva_rating;
                self.kva_not_set = false;
            }
            DYNAMICEQ => self.dyneq.on_dynamic_eq_set(),
            // Pascal `TProp.VV_Curve` side effect: load the four volt-var points
            // (V1..V4 / Q1..Q4) into the WTG3 model.
            VV_CURVE => {
                if let Some(c) = self.vv_curve_obj.as_ref() {
                    let xs = c.x_values();
                    let ys = c.y_values();
                    if xs.len() >= 4 && ys.len() >= 4 {
                        self.wind_model_dyn.v1_volt_var = xs[0];
                        self.wind_model_dyn.v2_volt_var = xs[1];
                        self.wind_model_dyn.v3_volt_var = xs[2];
                        self.wind_model_dyn.v4_volt_var = xs[3];
                        self.wind_model_dyn.q1_volt_var = ys[0];
                        self.wind_model_dyn.q2_volt_var = ys[1];
                        self.wind_model_dyn.q3_volt_var = ys[2];
                        self.wind_model_dyn.q4_volt_var = ys[3];
                    }
                }
            }
            // PLoss / VWind / DebugTrace: no behavioral side effect (the loss
            // curve name is already stored; VWind's Pascal side effect is an empty
            // TODO; DebugTrace only opens a CSV trace file, not ported).
            _ => {}
        }
    }

    /// Pascal `TWindGen.EndEdit`: `RecalcElementData` + Yprim invalidation.
    fn end_edit(&mut self) {
        self.recalc(&default_recalc_ctx());
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TWindGenObj.MakeLike`. Copies the machine/shape record; the
    /// aerodynamic parameters and the WTG3 dynamics model are **not** copied
    /// (matching upstream — they keep the new object's Create defaults).
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<WindGen>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nconds(self.cd.nphases);
            self.cd.yprim_invalid = true;
        }
        self.kv_windgen_base = other.kv_windgen_base;
        self.v_base = other.v_base;
        self.vminpu = other.vminpu;
        self.vmaxpu = other.vmaxpu;
        self.v_base95 = other.v_base95;
        self.v_base105 = other.v_base105;
        self.kw_base = other.kw_base;
        self.kvar_base = other.kvar_base;
        self.p_nominal_per_phase = other.p_nominal_per_phase;
        self.pf_nominal = other.pf_nominal;
        self.q_nominal_per_phase = other.q_nominal_per_phase;
        self.connection = other.connection;
        self.yearly_shape = other.yearly_shape.clone();
        self.daily_shape = other.daily_shape.clone();
        self.duty_shape = other.duty_shape.clone();
        self.yearly_shape_obj = other.yearly_shape_obj.clone();
        self.daily_shape_obj = other.daily_shape_obj.clone();
        self.duty_shape_obj = other.duty_shape_obj.clone();
        self.yearly_shape_ref = other.yearly_shape_ref;
        self.daily_shape_ref = other.daily_shape_ref;
        self.duty_shape_ref = other.duty_shape_ref;
        self.duty_start = other.duty_start;
        self.gen_class = other.gen_class;
        self.gen_model = other.gen_model;
        self.forced_on = other.forced_on;
        self.kva_not_set = other.kva_not_set;
        self.kva_rating = other.kva_rating;
        self.h_mass = other.h_mass;
        self.theta = other.theta;
        self.speed = other.speed;
        self.w0 = other.w0;
        self.dspeed = other.dspeed;
        self.d_damping = other.d_damping;
        self.dpu = other.dpu;
        self.xrdp = other.xrdp;
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    fn parse_dyn_var(
        &mut self,
        variable: &str,
        value: &str,
        vars: &dss_parser::ParserVars,
    ) -> bool {
        self.dyneq.parse_dyn_var(variable, value, vars)
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

impl DynEqPce for WindGen {
    fn dyneq(&self) -> &DynEqPceData {
        &self.dyneq
    }
    fn dyneq_mut(&mut self) -> &mut DynEqPceData {
        &mut self.dyneq
    }
}
