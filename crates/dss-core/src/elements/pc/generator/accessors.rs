//! Trait impls: `CktElement` (Yprim build, injection/terminal currents) and
//! `DssObject` (typed property getters/setters, shape resolution, side effects,
//! `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::traits::{CktElement, ElemRef, InjCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;

use super::{Connection, Generator, default_recalc_ctx, nconds_for_connection, prop};

impl CktElement for Generator {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys);
    }

    /// Pascal `TGeneratorObj.CalcYPrim`.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let mut yp_shunt = CMatrix::new(yorder);

        self.set_nominal_generation(sys);
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

    /// Pascal `TGeneratorObj.InitHarmonics`.
    fn init_harmonics(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_harmonics_impl(sys, node_v);
    }

    /// Pascal `TGeneratorObj.InitStateVars`.
    fn init_state_vars(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_state_vars_impl(sys, node_v);
    }

    /// Pascal `TGeneratorObj.IntegrateStates`.
    fn integrate_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.integrate_states_impl(sys, node_v);
    }

    /// Pascal `TGeneratorObj.NumVariables`.
    fn num_variables(&self) -> usize {
        self.num_gen_variables()
    }

    /// Pascal `TGeneratorObj.VariableName`.
    fn variable_name(&self, i: usize) -> String {
        self.gen_variable_name(i)
    }

    /// Pascal `TGeneratorObj.GetAllVariables`.
    fn get_all_variables(&mut self, sys: &SysCtx, node_v: &[Complex64], states: &mut [f64]) {
        let _ = (sys, node_v);
        self.get_gen_variables(states);
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

    /// Pascal `TGeneratorObj.InjCurrents` + `TPCElement.InjCurrents`.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        if !self.cd.enabled {
            return;
        }
        if sys.loads_need_updating {
            self.set_nominal_generation(sys);
        }
        let mut errors = Vec::new();
        self.calc_inj_current_array(sys, ctx.node_v, &mut errors);
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
    }

    /// Pascal `TGeneratorObj.GetTerminalCurrents` + `TPCElement` base.
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if !self.cd.enabled {
            curr.fill(Complex64::ZERO);
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

impl DssObject for Generator {
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
            KV => self.kv_generator_base,
            KW => self.kw_base,
            PF => self.pf_nominal,
            KVAR => self.kvar_base,
            VMINPU => self.vminpu,
            VMAXPU => self.vmaxpu,
            DISPVALUE => self.dispatch_value,
            VPU => self.vpu,
            MAXKVAR => self.kvar_max,
            MINKVAR => self.kvar_min,
            PVFACTOR => self.pv_factor,
            KVA | MVA => self.kva_rating,
            XD => self.pu_xd,
            XDP => self.pu_xdp,
            XDPP => self.pu_xdpp,
            H => self.h_mass,
            D => self.dpu,
            DUTYSTART => self.duty_start,
            XRDP => self.xrdp,
            FUELKWH => self.fuel_kwh,
            PCTFUEL => self.pct_fuel,
            PCTRESERVE => self.pct_reserve,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Generator has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kv_generator_base = value,
            KW => self.kw_base = value,
            PF => self.pf_nominal = value,
            KVAR => self.kvar_base = value,
            VMINPU => self.vminpu = value,
            VMAXPU => self.vmaxpu = value,
            DISPVALUE => self.dispatch_value = value,
            VPU => self.vpu = value,
            MAXKVAR => self.kvar_max = value,
            MINKVAR => self.kvar_min = value,
            PVFACTOR => self.pv_factor = value,
            KVA | MVA => self.kva_rating = value,
            XD => self.pu_xd = value,
            XDP => self.pu_xdp = value,
            XDPP => self.pu_xdpp = value,
            H => self.h_mass = value,
            D => self.dpu = value,
            DUTYSTART => self.duty_start = value,
            XRDP => self.xrdp = value,
            FUELKWH => self.fuel_kwh = value,
            PCTFUEL => self.pct_fuel = value,
            PCTRESERVE => self.pct_reserve = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Generator has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            MODEL => self.gen_model,
            DISPMODE => self.dispatch_mode,
            CONN => self.connection as i32,
            STATUS => self.is_fixed as i32,
            CLS => self.gen_class,
            _ => unreachable!("Generator has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            MODEL => self.gen_model = value,
            DISPMODE => self.dispatch_mode = value,
            CONN => {
                self.connection = if value == 1 {
                    Connection::Delta
                } else {
                    Connection::Wye
                }
            }
            STATUS => self.is_fixed = value != 0,
            CLS => self.gen_class = value,
            _ => unreachable!("Generator has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            FORCEON => self.forced_on,
            DEBUGTRACE => self.debug_trace,
            BALANCED => self.force_balanced,
            USEFUEL => self.use_fuel,
            REFUEL => false, // Pascal BooleanActionProperty getter: always 0
            ENABLED => self.cd.enabled,
            _ => unreachable!("Generator has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            FORCEON => self.forced_on = value,
            DEBUGTRACE => self.debug_trace = value,
            BALANCED => self.force_balanced = value,
            USEFUEL => self.use_fuel = value,
            REFUEL => {
                // Pascal `DoRefuel`: fires on TRUE only.
                if value {
                    self.pct_fuel = 100.0;
                    self.gen_active = true;
                }
            }
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Generator has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            USERMODEL => self.user_model_name.clone(),
            USERDATA => self.user_data.clone(),
            SHAFTMODEL => self.shaft_model_name.clone(),
            SHAFTDATA => self.shaft_data.clone(),
            DYNAMICEQ => self.dynamic_eq.clone(),
            DYNOUT => self.dyn_out.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("Generator has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            // The NOT_PORTED string props error in the parser before reaching
            // here; the setters exist for completeness/MakeLike.
            USERMODEL => self.user_model_name = value,
            USERDATA => self.user_data = value,
            SHAFTMODEL => self.shaft_model_name = value,
            SHAFTDATA => self.shaft_data = value,
            DYNAMICEQ => self.dynamic_eq = value,
            DYNOUT => self.dyn_out = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("Generator has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape reference (snapshot-clone like the Load shape refs).
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
            _ => unreachable!("Generator has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TGeneratorObj.PropertySideEffects` (text-parser path).
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
            MODEL => {
                if self.gen_model == 3 {
                    self.cd.signal_reset_solution_initialized = true;
                }
            }
            YEARLY => {
                let actual = self
                    .yearly_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| (s.max_p(), s.max_q()));
                if let Some((mp, mq)) = actual {
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
            KVA | MVA => self.kva_not_set = false,
            _ => {}
        }
    }

    /// Pascal `TGenerator.EndEdit`: `RecalcElementData` + Yprim invalidation.
    fn end_edit(&mut self) {
        self.recalc(&default_recalc_ctx());
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TGeneratorObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Generator>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nconds(self.cd.nphases); // Pascal: NConds := Fnphases
            self.cd.yprim_invalid = true;
        }
        self.v_base = other.v_base;
        self.vminpu = other.vminpu;
        self.vmaxpu = other.vmaxpu;
        self.v_base95 = other.v_base95;
        self.v_base105 = other.v_base105;
        self.kw_base = other.kw_base;
        self.kvar_base = other.kvar_base;
        // GenVars (the machine record) — copied wholesale by Pascal.
        self.kv_generator_base = other.kv_generator_base;
        self.kva_rating = other.kva_rating;
        self.pu_xd = other.pu_xd;
        self.pu_xdp = other.pu_xdp;
        self.pu_xdpp = other.pu_xdpp;
        self.xd = other.xd;
        self.xdp = other.xdp;
        self.xdpp = other.xdpp;
        self.h_mass = other.h_mass;
        self.dpu = other.dpu;
        self.xrdp = other.xrdp;
        self.v_target = other.v_target;
        self.p_nominal_per_phase = other.p_nominal_per_phase;
        self.q_nominal_per_phase = other.q_nominal_per_phase;
        self.pf_nominal = other.pf_nominal;
        self.var_min = other.var_min;
        self.var_max = other.var_max;
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
        self.dispatch_mode = other.dispatch_mode;
        self.dispatch_value = other.dispatch_value;
        self.gen_class = other.gen_class;
        self.gen_model = other.gen_model;
        self.is_fixed = other.is_fixed;
        self.vpu = other.vpu;
        self.kvar_max = other.kvar_max;
        self.kvar_min = other.kvar_min;
        self.forced_on = other.forced_on;
        self.kva_not_set = other.kva_not_set;
        self.use_fuel = other.use_fuel;
        self.fuel_kwh = other.fuel_kwh;
        self.pct_fuel = other.pct_fuel;
        self.pct_reserve = other.pct_reserve;
        self.user_model_name = other.user_model_name.clone();
        self.shaft_model_name = other.shaft_model_name.clone();
        self.spectrum = other.spectrum.clone();
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
