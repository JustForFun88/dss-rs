//! Trait impls: `CktElement` (Yprim build, dynamics/harmonics hooks, the state-
//! variable interface, injection/terminal currents) and `DssObject` (typed
//! property getters/setters, shape resolution, side effects, `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::{CktElementData, ElemFlags};
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pc::generator::{Connection, default_recalc_ctx};
use crate::elements::pos_seq::{PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, InjCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::support::mathutil::power_factor;

use super::{IndMach012, nconds_for_connection, prop};

impl CktElement for IndMach012 {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys);
    }

    /// Pascal `TIndMach012Obj.MakePosSequence` (IndMach012.pas:1424-1426): an
    /// EMPTY body with NO `inherited` — the machine is left completely untouched
    /// (not even the base bus rename runs).
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        PosSeqPlan::no_base()
    }

    /// Pascal `TIndMach012Obj.CalcYPrim`.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        // Pascal `CalcYPrim` does not call `SetNominalPower` itself (unlike the
        // Generator); the nominal is set in `RecalcElementData` and `InjCurrents`.
        self.calc_yprim_impl(sys);
    }

    /// Pascal `TIndMach012Obj.InitHarmonics` (just forces a YPrim rebuild).
    fn init_harmonics(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let _ = (sys, node_v);
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TIndMach012Obj.InitStateVars`.
    fn init_state_vars(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_state_vars_impl(sys, node_v);
    }

    /// Pascal `TIndMach012Obj.IntegrateStates`.
    fn integrate_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.integrate_states_impl(sys, node_v);
    }

    /// Pascal `TIndMach012Obj.NumVariables`.
    fn num_variables(&self) -> usize {
        self.num_variables_impl()
    }

    /// Pascal `TIndMach012Obj.VariableName`.
    fn variable_name(&self, i: usize) -> String {
        self.variable_name_impl(i)
    }

    /// Pascal `TIndMach012Obj.GetAllVariables`.
    fn get_all_variables(&mut self, sys: &SysCtx, node_v: &[Complex64], states: &mut [f64]) {
        self.get_all_variables_impl(sys, node_v, states);
    }

    /// Pascal `TIndMach012Obj.Set_Variable`.
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

    /// Pascal `TIndMach012Obj.InjCurrents` + `TPCElement.InjCurrents`.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        if !self.cd.enabled {
            return;
        }
        if sys.loads_need_updating {
            self.set_nominal_power(sys);
        }
        // r4133 `TIndMach012Obj.InjCurrents`: `if not ForceInjCurr then
        // CalcInjCurrentArray` — skip only the model recompute when the injection
        // is forced; the set-nominal preamble and the inherited add stay
        // unconditional.
        if !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS) {
            self.calc_inj_current_array(sys, ctx.node_v);
        }
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
    }

    /// Pascal `TIndMach012Obj.GetTerminalCurrents` + `TPCElement` base.
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if !self.cd.enabled {
            curr.fill(Complex64::ZERO);
            return;
        }
        // Pascal `TPCElement.GetCurrents` l.137 (`LastSolutionWasDirect`
        // shortcut): report `YPrim · Vterminal` after a direct solve.
        if sys.pc_direct_shortcut() {
            self.cd.calc_yprim_contribution(node_v, curr);
            return;
        }
        // Pascal `TIndMach012Obj.GetTerminalCurrents` (@ 0.15.0b4): `and (not
        // (Flg.ForceInjCurrents in Flags))` — skip the model recompute when the
        // currents are forced from the DSS language (WP-U1.9).
        if !self.cd.iterminal_solved_for(sys.solution_count)
            && !self.ind_mach_switch_open
            && !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS)
        {
            self.calc_model_contribution(sys, node_v);
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
        self.cd.mark_iterminal_solved(sys.solution_count);
    }
}

impl DssObject for IndMach012 {
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
            // Pascal `pf` ReadByFunction → PowerFactor(Power[1]). The text render is
            // intercepted by SILENT_READ_ONLY (→ "" always — upstream leaves
            // PropertyOffset at -1, so the GetObjPropertyValue guard never calls the
            // read function, solved or not; probe-proven), so this arm is unreachable
            // in practice. The live power factor is exposed as state variable #21
            // (`get_all_variables_impl`, where the solution exists). Return the
            // unsolved value (PowerFactor(0) = 1) as a placeholder.
            PF => power_factor(Complex64::ZERO),
            KVA => self.kva_rating,
            H => self.h_mass,
            D => self.d,
            PURS => self.pu_rs,
            PUXS => self.pu_xs,
            PURR => self.pu_rr,
            PUXR => self.pu_xr,
            PUXM => self.pu_xm,
            SLIP => self.s1,
            MAXSLIP => self.max_slip,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("IndMach012 has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kv_generator_base = value,
            KW => self.kw_base = value,
            // Pascal `pf` SilentReadOnly: writes are silently ignored.
            PF => {}
            KVA => self.kva_rating = value,
            H => self.h_mass = value,
            D => self.d = value,
            PURS => self.pu_rs = value,
            PUXS => self.pu_xs = value,
            PURR => self.pu_rr = value,
            PUXR => self.pu_xr = value,
            PUXM => self.pu_xm = value,
            // Pascal `Slip` WriteByFunction → set_Localslip.
            SLIP => self.set_local_slip(value),
            MAXSLIP => self.max_slip = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("IndMach012 has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            CONN => self.connection as i32,
            SLIPOPTION => self.fixed_slip as i32,
            _ => unreachable!("IndMach012 has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            CONN => {
                self.connection = if value == 1 {
                    Connection::Delta
                } else {
                    Connection::Wye
                }
            }
            SLIPOPTION => self.fixed_slip = value != 0,
            _ => unreachable!("IndMach012 has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            DEBUGTRACE => self.debug_trace,
            ENABLED => self.cd.enabled,
            _ => unreachable!("IndMach012 has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            DEBUGTRACE => self.debug_trace = value,
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("IndMach012 has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("IndMach012 has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape = value,
            DAILY => self.daily_shape = value,
            DUTY => self.duty_shape = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("IndMach012 has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape reference (snapshot-clone like the Generator shape refs).
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
            _ => unreachable!("IndMach012 has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TIndMach012Obj.PropertySideEffects` (only `phases`, `kV`, `slip`,
    /// and the three shapes have side effects; `conn` notably does **not**).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            PHASES => {
                let n = nconds_for_connection(self.connection, self.cd.nphases);
                self.cd.set_nconds(n); // force reallocation of terminal info
            }
            KV => self.update_vbase(),
            SLIP => self.speed = self.w0 * (-self.s1), // make motor speed agree
            YEARLY => {
                let mp = self
                    .yearly_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| s.max_p());
                if let Some(mp) = mp {
                    self.kw_base = mp;
                }
            }
            DAILY => {
                let mp = self
                    .daily_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| s.max_p());
                if let Some(mp) = mp {
                    self.kw_base = mp;
                }
            }
            DUTY => {
                let mp = self
                    .duty_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| s.max_p());
                if let Some(mp) = mp {
                    self.kw_base = mp;
                }
            }
            _ => {}
        }
    }

    /// Pascal `TIndMach012.EndEdit`: `RecalcElementData` + Yprim invalidation.
    fn end_edit(&mut self) {
        self.recalc(&default_recalc_ctx());
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TIndMach012Obj.MakeLike` (+ the inherited `TPCElement.MakeLike`,
    /// which copies the spectrum). Note Pascal copies the whole `MachineData`
    /// record but **not** `Connection`/`S1`/`FixedSlip`/the dispatch shapes.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<IndMach012>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nconds(self.cd.nphases); // Pascal: NConds := Fnphases
            self.cd.yprim_invalid = true;
        }
        // MachineData (the TGeneratorVars record) — copied wholesale.
        self.kv_generator_base = other.kv_generator_base;
        self.kva_rating = other.kva_rating;
        self.h_mass = other.h_mass;
        self.d = other.d;
        self.dpu = other.dpu;
        self.w0 = other.w0;
        self.speed = other.speed;
        self.dspeed = other.dspeed;
        self.theta = other.theta;
        self.dtheta = other.dtheta;
        self.p_shaft = other.p_shaft;
        self.m_mass = other.m_mass;
        self.speed_history = other.speed_history;
        self.theta_history = other.theta_history;
        self.p_nominal_per_phase = other.p_nominal_per_phase;
        self.v_base = other.v_base;
        self.kw_base = other.kw_base;
        self.pu_rs = other.pu_rs;
        self.pu_rr = other.pu_rr;
        self.pu_xr = other.pu_xr;
        self.pu_xm = other.pu_xm;
        self.pu_xs = other.pu_xs;
        self.max_slip = other.max_slip;
        // Inherited TPCElement.MakeLike: SpectrumObj.
        self.spectrum = other.spectrum.clone();
        self.spectrum_obj = other.spectrum_obj.clone();
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
