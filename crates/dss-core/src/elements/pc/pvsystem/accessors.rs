//! Trait impls: `CktElement` (Yprim build, injection/terminal currents),
//! `DssObject` (typed property getters/setters, shape/curve resolution, side
//! effects, `MakeLike`) and [`InvBasedPce`] (the inverter virtual hooks).
//!
//! [`InvBasedPce`]: crate::elements::pc::inv_based_pce::InvBasedPce

use num_complex::Complex64;

use crate::elements::ckt::{CktElementData, ElemFlags};
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::temp_shape::TShapeObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pc::inv_based_pce::{Connection, InvBasedPce, InvBasedPceData};
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, ElemRef, InjComputeCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject, UserModelLoad};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

use super::{PVSystem, VARMODE_KVAR, VARMODE_PF, nconds_for_connection, prop};

impl CktElement for PVSystem {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys);
    }

    /// Pascal `TPVsystemObj.MakePosSequence` (`PVsystem.pas:2638`). Single
    /// phase, line-neutral; a multi-phase array's `kVA` rating is divided by the
    /// phase count and `PF` is set to the nominal PF.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        // Make sure voltage is line-neutral.
        let v = if self.cd.nphases > 1 || self.base.connection as i32 != 0 {
            self.kv_pvsystem_base / sqrt3()
        } else {
            self.kv_pvsystem_base
        };

        let old_phases = self.cd.nphases;
        let mut actions = vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, v),
        ];
        if old_phases > 1 {
            let new_kva = self.f_kva_rating / self.cd.nphases as f64;
            actions.push(PosSeqAction::SetF64(prop::KVA, new_kva));
            actions.push(PosSeqAction::SetF64(prop::PF, self.base.pf_nominal));
        }
        actions.push(PosSeqAction::EndEdit);

        PosSeqPlan::with_actions(actions)
    }

    /// Pascal `TPVsystemObj.CalcYPrim`.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let mut yp_shunt = CMatrix::new(yorder);

        self.set_nominal_der_output(sys);
        self.calc_yprim_matrix(&mut yp_shunt, sys);

        // Dummy series Yprim from the shunt diagonal so CalcVoltages doesn't fail.
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

    /// Pascal `TPVsystemObj.InitHarmonics`.
    fn init_harmonics(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_harmonics_impl(sys, node_v);
    }

    /// Pascal `TPVsystemObj.InitStateVars`.
    fn init_state_vars(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_state_vars_impl(sys, node_v);
    }

    /// Pascal `TPVsystemObj.IntegrateStates`.
    fn integrate_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.integrate_states_impl(sys, node_v);
    }

    /// Pascal `TPVsystemObj.NumVariables` — the linked `DynamicExp` count first
    /// (inherited `TDynEqPCE.NumVariables`), else 22 (13 base + 9 InvDynVars).
    fn num_variables(&self) -> usize {
        let n = self.base.dyneq.num_variables();
        if n != 0 {
            return n;
        }
        self.num_pv_variables() + self.num_user_model_variables()
    }

    /// Pascal `TPVsystemObj.VariableName` (1-based, `PVsystem.pas:2591-2640`): the
    /// `DynamicExp` memory-slot name first (inherited), then the classic name
    /// table, then the `UserModel` names.
    fn variable_name(&self, i: usize) -> String {
        if let Some(name) = self.base.dyneq.variable_name(i) {
            return name;
        }
        if i > self.num_pv_variables()
            && let Some(name) = self.user_model_variable_name(i)
        {
            return name;
        }
        self.pv_variable_name(i)
    }

    /// Pascal `TPVsystemObj.GetAllVariables` (`PVsystem.pas:2552-2564`): the
    /// `DynamicExp` memory dump first, else the classic 22 followed by the
    /// `UserModel` values (`@States[NumPVSystemVariables]`).
    fn get_all_variables(&mut self, sys: &SysCtx, node_v: &[Complex64], states: &mut [f64]) {
        if self.base.dyneq.has_dynamic_eq() {
            for (i, s) in states
                .iter_mut()
                .enumerate()
                .take(self.base.dyneq.num_variables())
            {
                *s = self.base.dyneq.get_dynamic_eq_val(i);
            }
            return;
        }
        self.get_all_pv_variables(sys, node_v, states);
        let base = self.num_pv_variables();
        let un = self.num_user_model_variables();
        if un > 0 {
            let end = (base + un).min(states.len());
            if base < end {
                self.get_all_vars_slot(&mut states[base..end], sys, node_v);
            }
        }
    }

    fn set_variable(&mut self, i: usize, value: f64, sys: &crate::elements::traits::SysCtx) {
        // Pascal `Set_Variable` routes i > NumPVSystemVariables to the UserModel
        // (`PVsystem.pas:2534-2541`, WASM_USERMODELS WM.4).
        if i > self.num_pv_variables() && self.set_user_model_variable(i, value, sys) {
            return;
        }
        self.set_pv_variable(i, value);
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

    /// Pascal `(pElem is TInvBasedPCE) and GFM_Mode`.
    fn is_gfm(&self) -> bool {
        self.base.gfm_mode
    }

    /// Pascal `TPVsystemObj.InjCurrents` + `TPCElement.InjCurrents` (M3b compute
    /// half; the caller scatters `cd.inj_current`).
    fn compute_inj_currents(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        ctx: &mut InjComputeCtx,
    ) -> bool {
        if !self.cd.enabled {
            return false;
        }
        if sys.loads_need_updating {
            self.set_nominal_der_output(sys);
        }
        // r4133 `TPVsystemObj.InjCurrents` (PVsystem.pas:2173): `if not ForceInjCurr
        // then CalcInjCurrentArray` — skip only the model recompute when the
        // injection is forced; the set-nominal preamble and the inherited add stay
        // unconditional (caller scatter).
        let mut errors = crate::diag::ErrorLog::new();
        if !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS) {
            self.calc_inj_current_array(sys, node_v, &mut errors);
        }
        // Surface any user-model trap / missing-model diagnostic through the
        // solution ErrorLog — never a silent fallback on a trapping `.wasm`
        // (WASM_USERMODELS plan §2.9-5, WM.3 precedent). An `abort`-flagged fault
        // (a wasm trap, ABI §6, or the missing dynamics model #5671,
        // `PVsystem.pas:1894`) lifts `SolutionAbort`.
        for d in errors.into_vec() {
            if d.abort {
                *ctx.solution_abort = true;
            }
            ctx.errors.push(d);
        }
        false
    }

    /// Pascal `TPVsystemObj.GetTerminalCurrents` + `TPCElement` base.
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if !self.cd.enabled {
            curr.fill(Complex64::ZERO);
            return;
        }
        // Pascal `TInvBasedPCE.GetCurrents` (InvBasedPCE.pas l.216): non-GFM
        // falls through to `inherited` `TPCElement.GetCurrents`, whose l.137
        // `LastSolutionWasDirect` shortcut reports `YPrim · Vterminal` after a
        // direct solve. The GFM branch below never calls `inherited`, so it
        // must NOT take the shortcut.
        if !self.base.gfm_mode && sys.pc_direct_shortcut() {
            self.cd.calc_yprim_contribution(node_v, curr);
            return;
        }
        // Pascal `TPVsystemObj.GetTerminalCurrents` (@ 0.15.0b4): `and (not
        // (Flg.ForceInjCurrents in Flags))` — skip the model recompute when the
        // currents are forced from the DSS language (WP-U1.9).
        if !self.cd.iterminal_solved_for(sys.solution_count)
            && !self.pv_system_obj_switch_open
            && !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS)
        {
            let mut errors = crate::diag::ErrorLog::new();
            self.calc_pvsystem_model_contribution(sys, node_v, &mut errors);
            // Route the recompute's user-model diagnostics to the element's
            // deferred-error log (drained by the executive) instead of dropping
            // them (WASM_USERMODELS plan §2.9-5, WM.3 precedent).
            for d in errors.into_vec() {
                self.cd.obj.push_error(d);
            }
        }
        if self.base.gfm_mode {
            // Pascal `TInvBasedPCE.GetCurrents` (GFM override, InvBasedPCE.pas
            // l.211): `Vterminal := NodeV`, then `Curr = YPrim·Vterminal −
            // InjCurrent` (the model's `Vterminal` holds the internal phasors).
            let cd = &mut self.cd;
            for i in 0..cd.yorder {
                cd.vterminal[i] = node_v[cd.node_ref[i]];
            }
            if let Some(yprim) = &cd.yprim {
                yprim.mv_mult(curr, &cd.vterminal);
            }
            for (i, c) in curr.iter_mut().enumerate() {
                *c -= cd.inj_current[i];
            }
            self.cd.iterminal_updated = true;
            self.cd.mark_iterminal_solved(sys.solution_count);
            return;
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

impl InvBasedPce for PVSystem {
    fn inv_based(&self) -> &InvBasedPceData {
        &self.base
    }
    fn inv_based_mut(&mut self) -> &mut InvBasedPceData {
        &mut self.base
    }
    /// Pascal `TPVSystemObj.IsPVSystem`.
    fn is_pvsystem(&self) -> bool {
        true
    }
    /// Pascal `TPVSystemObj.GetPFPriority`.
    fn get_pf_priority(&self) -> bool {
        self.pf_priority
    }
}

impl DssObject for PVSystem {
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
    fn as_dyneq(&self) -> Option<&crate::elements::pc::dyneq_pce::DynEqPceData> {
        Some(&self.base.dyneq)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            KV => self.kv_pvsystem_base,
            IRRADIANCE => self.f_irradiance,
            PMPP => self.f_pmpp,
            PCT_PMPP => self.f_pu_pmpp,
            TEMPERATURE => self.f_temperature,
            PF => self.base.pf_nominal,
            KVAR => self.base.kvar_out, // Pascal Getkvar
            KVA => self.f_kva_rating,
            PCT_CUTIN => self.base.fpct_cut_in,
            PCT_CUTOUT => self.base.fpct_cut_out,
            PCT_R => self.base.pct_r,
            PCT_X => self.base.pct_x,
            VMINPU => self.base.vminpu,
            VMAXPU => self.base.vmaxpu,
            DUTYSTART => self.duty_start,
            PCT_PMIN_NO_VARS => self.base.fpct_pmin_no_vars,
            PCT_PMIN_KVAR_MAX => self.base.fpct_pmin_kvar_limit,
            KVAR_MAX => self.f_kvar_limit,
            KVAR_MAX_ABS => self.f_kvar_limit_neg,
            KVDC => self.base.dyn_vars.rated_vdc,
            KP => self.base.dyn_vars.kp,
            PITOL => self.base.dyn_vars.ctrl_tol,
            SAFE_VOLTAGE => self.base.dyn_vars.sm_threshold,
            AMP_LIMIT => self.base.dyn_vars.i_limit,
            AMP_LIMIT_GAIN => self.base.dyn_vars.v_error,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("PVSystem has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kv_pvsystem_base = value,
            IRRADIANCE => self.f_irradiance = value,
            PMPP => self.f_pmpp = value,
            PCT_PMPP => self.f_pu_pmpp = value,
            TEMPERATURE => self.f_temperature = value,
            PF => self.base.pf_nominal = value,
            KVAR => self.kvar_requested = value, // write target (Pascal kvarRequested)
            KVA => self.f_kva_rating = value,
            PCT_CUTIN => self.base.fpct_cut_in = value,
            PCT_CUTOUT => self.base.fpct_cut_out = value,
            PCT_R => self.base.pct_r = value,
            PCT_X => self.base.pct_x = value,
            VMINPU => self.base.vminpu = value,
            VMAXPU => self.base.vmaxpu = value,
            DUTYSTART => self.duty_start = value,
            // Pascal `Transform_Abs`: store the magnitude.
            PCT_PMIN_NO_VARS => self.base.fpct_pmin_no_vars = value.abs(),
            PCT_PMIN_KVAR_MAX => self.base.fpct_pmin_kvar_limit = value.abs(),
            KVAR_MAX => self.f_kvar_limit = value.abs(),
            KVAR_MAX_ABS => self.f_kvar_limit_neg = value.abs(),
            KVDC => self.base.dyn_vars.rated_vdc = value,
            KP => self.base.dyn_vars.kp = value,
            PITOL => self.base.dyn_vars.ctrl_tol = value,
            SAFE_VOLTAGE => self.base.dyn_vars.sm_threshold = value,
            AMP_LIMIT => self.base.dyn_vars.i_limit = value,
            AMP_LIMIT_GAIN => self.base.dyn_vars.v_error = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("PVSystem has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            CONN => self.base.connection as i32,
            MODEL => self.base.voltage_model,
            CLS => self.f_class,
            CONTROL_MODE => self.base.gfm_mode as i32,
            _ => unreachable!("PVSystem has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            CONN => {
                self.base.connection = if value == 1 {
                    Connection::Delta
                } else {
                    Connection::Wye
                }
            }
            MODEL => self.base.voltage_model = value,
            CLS => self.f_class = value,
            CONTROL_MODE => self.base.gfm_mode = value != 0,
            _ => unreachable!("PVSystem has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            BALANCED => self.base.force_balanced,
            LIMIT_CURRENT => self.base.current_limited,
            DEBUGTRACE => self.base.debug_trace,
            VAR_FOLLOW_INVERTER => self.base.var_follow_inverter,
            WATT_PRIORITY => self.p_priority,
            PF_PRIORITY => self.pf_priority,
            SAFE_MODE => self.base.dyn_vars.safe_mode,
            ENABLED => self.cd.enabled,
            _ => unreachable!("PVSystem has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            BALANCED => self.base.force_balanced = value,
            LIMIT_CURRENT => self.base.current_limited = value,
            // The trace *file* is not ported (like Generator); store the flag.
            DEBUGTRACE => self.base.debug_trace = value,
            VAR_FOLLOW_INVERTER => self.base.var_follow_inverter = value,
            WATT_PRIORITY => self.p_priority = value,
            PF_PRIORITY => self.pf_priority = value,
            SAFE_MODE => {} // Pascal SilentReadOnly: ignore writes
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("PVSystem has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            YEARLY => self.base.yearly_shape.clone(),
            DAILY => self.base.daily_shape.clone(),
            DUTY => self.base.duty_shape.clone(),
            TYEARLY => self.yearly_t_shape.clone(),
            TDAILY => self.daily_t_shape.clone(),
            TDUTY => self.duty_t_shape.clone(),
            EFF_CURVE => self.base.inverter_curve.clone(),
            P_T_CURVE => self.power_temp_curve.clone(),
            DYNAMIC_EQ => self.base.dyneq.dynamic_eq.clone(),
            USERMODEL => self.base.user_model_name.clone(),
            USERDATA => self.base.user_model_edit.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("PVSystem has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            // `UserModel=` stores the name; the deferred load is queued by
            // `side_effects` and resolved by the executive (WM.4 §2.4).
            USERMODEL => self.base.user_model_name = value,
            USERDATA => self.base.user_model_edit = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("PVSystem has no string property {idx}"),
        }
    }

    /// `DynOut` (Pascal `StringListProperty` via `Set/GetDynOutputNames`): the
    /// dynamics output-variable selection, resolved against the linked
    /// `DynamicExp` to output indices and reconstructed for the dump.
    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::DYN_OUT => self.base.dyneq.get_dyn_output_names(),
            _ => unreachable!("PVSystem has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            prop::DYN_OUT => {
                let errors = self.base.dyneq.set_dyn_output_names(&value);
                for e in errors {
                    self.cd.obj.push_error(e);
                }
            }
            _ => unreachable!("PVSystem has no string-list property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape / curve / dynamic-expression reference (snapshot-clone,
    /// the WP4.2/WP5.3 `FetchLineCode` pattern).
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
        let t_shape =
            || resolved.and_then(|(_, o)| o.as_any().downcast_ref::<TShapeObj>().cloned());
        let xy_curve =
            || resolved.and_then(|(_, o)| o.as_any().downcast_ref::<XyCurveObj>().cloned());
        match idx {
            YEARLY => {
                self.base.yearly_shape = name;
                self.base.yearly_shape_ref = elem_ref;
                self.base.yearly_shape_obj = load_shape();
            }
            DAILY => {
                self.base.daily_shape = name;
                self.base.daily_shape_ref = elem_ref;
                self.base.daily_shape_obj = load_shape();
            }
            DUTY => {
                self.base.duty_shape = name;
                self.base.duty_shape_ref = elem_ref;
                self.base.duty_shape_obj = load_shape();
            }
            TYEARLY => {
                self.yearly_t_shape = name;
                self.yearly_t_shape_ref = elem_ref;
                self.yearly_t_shape_obj = t_shape();
            }
            TDAILY => {
                self.daily_t_shape = name;
                self.daily_t_shape_ref = elem_ref;
                self.daily_t_shape_obj = t_shape();
            }
            TDUTY => {
                self.duty_t_shape = name;
                self.duty_t_shape_ref = elem_ref;
                self.duty_t_shape_obj = t_shape();
            }
            EFF_CURVE => {
                self.base.inverter_curve = name;
                self.base.inverter_curve_ref = elem_ref;
                self.base.inverter_curve_obj = xy_curve();
            }
            P_T_CURVE => {
                self.power_temp_curve = name;
                self.power_temp_curve_ref = elem_ref;
                self.power_temp_curve_obj = xy_curve();
            }
            DYNAMIC_EQ => {
                self.base.dyneq.dynamic_eq = name;
                self.base.dyneq.dynamic_eq_ref = elem_ref;
                self.base.dyneq.dynamic_eq_obj = resolved.and_then(|(_, o)| {
                    o.as_any()
                        .downcast_ref::<crate::elements::general::dynamic_exp::DynamicExpObj>()
                        .cloned()
                });
            }
            _ => unreachable!("PVSystem has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TPVsystemObj.PropertySideEffects` (text-parser path).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            PHASES => {
                let n = nconds_for_connection(self.base.connection, self.cd.nphases);
                self.cd.set_nconds(n); // force reallocation of terminal info
                self.update_vbase(); // Pascal also runs the kV side effect
            }
            CONN => {
                let n = nconds_for_connection(self.base.connection, self.cd.nphases);
                self.cd.set_nconds(n);
                self.update_vbase();
                self.base.v_base_min = self.base.vminpu * self.base.v_base;
                self.base.v_base_max = self.base.vmaxpu * self.base.v_base;
                self.cd.yprim_invalid = true;
                self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
            }
            KV => self.update_vbase(),
            PF => {
                self.base.var_mode = VARMODE_PF;
                self.cd.obj.clear_seq(KVAR);
            }
            KVAR => {
                self.base.var_mode = VARMODE_KVAR;
                self.cd.obj.clear_seq(PF);
            }
            KVA => {
                if !self.base.kvar_limit_set {
                    self.f_kvar_limit = self.f_kva_rating;
                }
                if !self.base.kvar_limit_set && !self.base.kvar_limit_neg_set {
                    self.f_kvar_limit_neg = self.f_kva_rating;
                }
            }
            KVAR_MAX_ABS => self.base.kvar_limit_neg_set = true,
            KVAR_MAX => {
                self.base.kvar_limit_set = true;
                if !self.base.kvar_limit_neg_set {
                    self.f_kvar_limit_neg = self.f_kvar_limit.abs();
                }
            }
            CONTROL_MODE => {
                // GFM Vgrid allocation / ResetIBR is WP7.7; the Y rebuild flag
                // is the observable power-flow effect.
                self.cd.yprim_invalid = true;
            }
            // Pascal `TProp.DynamicEq` side effect: size the DynamicEqVals memory
            // to the linked DynamicExp's NVariables (a nil ref leaves it empty).
            DYNAMIC_EQ => self.base.dyneq.on_dynamic_eq_set(),
            // WASM_USERMODELS WM.4 — the §2.4 uniform activation rule. Pascal edit
            // dispatch (PVsystem.pas:628-632): `UserModel.Name` (load) then
            // `UserData` (edit). The filesystem is unreachable from the property
            // hook, so each records a deferred request the executive resolves
            // before `end_edit`.
            USERMODEL => self.queue_user_model_load(self.base.user_model_name.clone()),
            USERDATA => self.queue_user_model_edit(self.base.user_model_edit.clone()),
            _ => {}
        }
    }

    /// Pascal `TPVsystem.EndEdit`: `RecalcElementData` + Yprim invalidation. `sys`
    /// is the LIVE circuit/solution the executive holds at the edit site.
    fn end_edit(&mut self, sys: &crate::elements::traits::SysCtx) {
        self.recalc(sys);
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TPVsystemObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<PVSystem>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nconds(self.cd.nphases); // Pascal: NConds := Fnphases
            self.cd.yprim_invalid = true;
        }
        self.kv_pvsystem_base = other.kv_pvsystem_base;
        self.base.v_base = other.base.v_base;
        self.base.vminpu = other.base.vminpu;
        self.base.vmaxpu = other.base.vmaxpu;
        self.base.v_base_min = other.base.v_base_min;
        self.base.v_base_max = other.base.v_base_max;
        self.base.kw_out = other.base.kw_out;
        self.base.kvar_out = other.base.kvar_out;
        self.base.p_nominal_per_phase = other.base.p_nominal_per_phase;
        self.base.pf_nominal = other.base.pf_nominal;
        self.base.q_nominal_per_phase = other.base.q_nominal_per_phase;
        self.base.connection = other.base.connection;
        self.base.yearly_shape = other.base.yearly_shape.clone();
        self.base.daily_shape = other.base.daily_shape.clone();
        self.base.duty_shape = other.base.duty_shape.clone();
        self.base.yearly_shape_obj = other.base.yearly_shape_obj.clone();
        self.base.daily_shape_obj = other.base.daily_shape_obj.clone();
        self.base.duty_shape_obj = other.base.duty_shape_obj.clone();
        self.base.yearly_shape_ref = other.base.yearly_shape_ref;
        self.base.daily_shape_ref = other.base.daily_shape_ref;
        self.base.duty_shape_ref = other.base.duty_shape_ref;
        self.duty_start = other.duty_start;
        self.yearly_t_shape = other.yearly_t_shape.clone();
        self.daily_t_shape = other.daily_t_shape.clone();
        self.duty_t_shape = other.duty_t_shape.clone();
        self.yearly_t_shape_obj = other.yearly_t_shape_obj.clone();
        self.daily_t_shape_obj = other.daily_t_shape_obj.clone();
        self.duty_t_shape_obj = other.duty_t_shape_obj.clone();
        self.yearly_t_shape_ref = other.yearly_t_shape_ref;
        self.daily_t_shape_ref = other.daily_t_shape_ref;
        self.duty_t_shape_ref = other.duty_t_shape_ref;
        self.base.inverter_curve = other.base.inverter_curve.clone();
        self.base.inverter_curve_obj = other.base.inverter_curve_obj.clone();
        self.base.inverter_curve_ref = other.base.inverter_curve_ref;
        self.power_temp_curve = other.power_temp_curve.clone();
        self.power_temp_curve_obj = other.power_temp_curve_obj.clone();
        self.power_temp_curve_ref = other.power_temp_curve_ref;
        self.f_class = other.f_class;
        self.base.voltage_model = other.base.voltage_model;
        self.f_temperature = other.f_temperature;
        self.f_pmpp = other.f_pmpp;
        self.base.fpct_cut_in = other.base.fpct_cut_in;
        self.base.fpct_cut_out = other.base.fpct_cut_out;
        self.base.var_follow_inverter = other.base.var_follow_inverter;
        self.f_kvar_limit = other.f_kvar_limit;
        self.f_kvar_limit_neg = other.f_kvar_limit_neg;
        self.base.fpct_pmin_no_vars = other.base.fpct_pmin_no_vars;
        self.base.fpct_pmin_kvar_limit = other.base.fpct_pmin_kvar_limit;
        self.base.kvar_limit_set = other.base.kvar_limit_set;
        self.base.kvar_limit_neg_set = other.base.kvar_limit_neg_set;
        self.f_irradiance = other.f_irradiance;
        self.f_kva_rating = other.f_kva_rating;
        self.base.pct_r = other.base.pct_r;
        self.base.pct_x = other.base.pct_x;
        self.base.vw_mode = other.base.vw_mode;
        self.base.wp_mode = other.base.wp_mode;
        self.base.wv_mode = other.base.wv_mode;
        self.base.drc_mode = other.base.drc_mode;
        self.base.avr_mode = other.base.avr_mode;
        // User model: Pascal re-`New`s a fresh instance from the same module
        // (`PVsystem.pas:820`); the slot's `Clone` drops the live wasmi instance
        // and re-creates it lazily (WM.4, the WM.3 generator precedent).
        self.base.user_model_name = other.base.user_model_name.clone();
        self.base.user_model_edit = other.base.user_model_edit.clone();
        self.user_model = other.user_model.clone();
        self.spectrum = other.spectrum.clone();
        self.base.force_balanced = other.base.force_balanced;
        self.base.current_limited = other.base.current_limited;
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    /// Pascal `TDynEqPCE.ParseDynVar`: a `name=value` whose `name` is a state
    /// variable of the linked `DynamicExp` (the inline `it=imag vdc=kvdc …`
    /// initializers in a GFL-DynExp deck).
    fn parse_dyn_var(
        &mut self,
        variable: &str,
        value: &str,
        vars: &dss_parser::ParserVars,
    ) -> bool {
        self.base.dyneq.parse_dyn_var(variable, value, vars)
    }

    /// Drain the deferred `UserModel=`/`UserData=` requests queued by the
    /// property side effects (WASM_USERMODELS WM.4, §2.4).
    fn take_user_model_loads(&mut self) -> Vec<UserModelLoad> {
        std::mem::take(&mut self.pending_user_model_loads)
    }

    /// Apply a resolved user-model load/edit (`wasm` is `Some` iff a `.wasm`
    /// file was found + read; `None` → warn-and-fallback, Pascal 1570).
    fn apply_user_model_load(
        &mut self,
        load: &UserModelLoad,
        wasm: Option<&[u8]>,
        sys: &crate::elements::traits::SysCtx,
        errors: &mut crate::diag::ErrorLog,
    ) {
        self.apply_user_model_load_impl(load, wasm, sys, errors);
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

impl crate::elements::pc::dyneq_pce::DynEqPce for PVSystem {
    fn dyneq(&self) -> &crate::elements::pc::dyneq_pce::DynEqPceData {
        &self.base.dyneq
    }
    fn dyneq_mut(&mut self) -> &mut crate::elements::pc::dyneq_pce::DynEqPceData {
        &mut self.base.dyneq
    }
}
