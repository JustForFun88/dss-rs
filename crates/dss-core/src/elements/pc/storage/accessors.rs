//! Trait impls: `CktElement` (Yprim build, injection/terminal currents),
//! `DssObject` (typed property getters/setters, shape/curve resolution, side
//! effects, `MakeLike`) and [`InvBasedPce`] (the inverter virtual hooks).
//!
//! [`InvBasedPce`]: crate::elements::pc::inv_based_pce::InvBasedPce

use num_complex::Complex64;

use crate::elements::ckt::{CktElementData, ElemFlags};
use crate::elements::general::dynamic_exp::DynamicExpObj;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pc::inv_based_pce::{Connection, InvBasedPce, InvBasedPceData};
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject, UserModelLoad, UserModelSlot};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

use super::{
    STORE_CHARGING, STORE_DISCHARGING, STORE_IDLING, Storage, StorageDispatchMode, VARMODE_KVAR,
    VARMODE_PF, nconds_for_connection, prop,
};

impl Storage {
    /// Pascal `Set_kW`: set the state + the dispatch percentage from a signed kW.
    /// `pub(crate)` so the StorageController fleet dispatch can drive `obj.kW`.
    pub(crate) fn set_kw(&mut self, value: f64) {
        if value > 0.0 {
            self.f_state = STORE_DISCHARGING;
            self.pct_kw_out = value / self.kw_rating * 100.0;
        } else if value < 0.0 {
            self.f_state = STORE_CHARGING;
            self.pct_kw_in = value.abs() / self.kw_rating * 100.0;
        } else {
            self.f_state = STORE_IDLING;
        }
    }
}

impl CktElement for Storage {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys);
    }

    /// Pascal `TStorageObj.MakePosSequence` (`Storage.pas:3320`). Single phase,
    /// line-neutral; a multi-phase unit's `kWrated` is divided by the phase
    /// count and `PF` is set to the nominal PF.
    ///
    /// TODO(compat): the Pascal body has NO `BeginEdit` before its `Set*` calls
    /// yet a trailing `EndEdit(changes)` (`Storage.pas:3339-3347`). Each `Set*`
    /// is therefore its own auto-bracketed single edit (own recalc), and the
    /// dangling `EndEdit` forces one extra recalc. Reproduced by emitting the
    /// `Set*` actions with no leading `BeginEdit` and one trailing `EndEdit` —
    /// the recalc count is observable. (PVSystem, by contrast, wraps its sets.)
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        // Make sure voltage is line-neutral.
        let v = if self.cd.nphases > 1 || self.base.connection as i32 != 0 {
            self.kv_storage_base / sqrt3()
        } else {
            self.kv_storage_base
        };

        let old_phases = self.cd.nphases;
        let mut actions = vec![
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetI32(prop::CONN, 0),
            PosSeqAction::SetF64(prop::KV, v),
        ];
        if old_phases > 1 {
            let new_kw = self.kw_rating / self.cd.nphases as f64;
            actions.push(PosSeqAction::SetF64(prop::KW_RATED, new_kw));
            actions.push(PosSeqAction::SetF64(prop::PF, self.base.pf_nominal));
        }
        actions.push(PosSeqAction::EndEdit);

        PosSeqPlan::with_actions(actions)
    }

    /// Pascal `TStorageObj.CalcYPrim`.
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

    /// Pascal `TStorageObj.InitHarmonics`.
    fn init_harmonics(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_harmonics_impl(sys, node_v);
    }

    /// Pascal `TStorageObj.InitStateVars`.
    fn init_state_vars(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.init_state_vars_impl(sys, node_v);
    }

    /// Pascal `TStorageObj.IntegrateStates`.
    fn integrate_states(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.integrate_states_impl(sys, node_v);
    }

    /// Pascal `TStorageObj.NumVariables` (`Storage.pas:3208-3223`) — the linked
    /// `DynamicExp` count first (inherited `TDynEqPCE.NumVariables`), else 34
    /// (25 base + 9 InvDynVars) plus the existing `UserModel`/`DynaModel`
    /// variable counts (WASM_USERMODELS WM.4).
    fn num_variables(&self) -> usize {
        let n = self.base.dyneq.num_variables();
        if n != 0 {
            return n;
        }
        self.num_storage_variables()
            + self.num_user_model_variables()
            + self.num_dyna_model_variables()
    }

    /// Pascal `TStorageObj.VariableName` (1-based, `Storage.pas:3225-3323`): the
    /// `DynamicExp` memory-slot name first (inherited), then the classic name
    /// table, then the `UserModel`/`DynaModel` names.
    fn variable_name(&self, i: usize) -> String {
        if let Some(name) = self.base.dyneq.variable_name(i) {
            return name;
        }
        if i > self.num_storage_variables()
            && let Some(name) = self.user_model_variable_name(i)
        {
            return name;
        }
        self.storage_variable_name(i)
    }

    /// Pascal `TStorageObj.GetAllVariables` (`Storage.pas:3181-3206`): the
    /// `DynamicExp` memory dump first, else the classic 34 followed by the
    /// `UserModel` then `DynaModel` values (each written at `@States[base]`,
    /// faithful to Pascal — with one model bound this is unambiguous).
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
        self.get_all_storage_variables(sys, node_v, states);
        let base = self.num_storage_variables();
        let un = self.num_user_model_variables();
        if un > 0 {
            let end = (base + un).min(states.len());
            if base < end {
                self.get_all_vars_slot(UserModelSlot::User, &mut states[base..end], sys, node_v);
            }
        }
        let dn = self.num_dyna_model_variables();
        if dn > 0 {
            let end = (base + dn).min(states.len());
            if base < end {
                self.get_all_vars_slot(UserModelSlot::Dyna, &mut states[base..end], sys, node_v);
            }
        }
    }

    fn set_variable(&mut self, i: usize, value: f64, sys: &crate::elements::traits::SysCtx) {
        self.set_storage_variable(i, value, sys);
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

    /// Pascal `TStorageObj.InjCurrents` + `TPCElement.InjCurrents` (M3b compute
    /// half; the caller scatters `cd.inj_current` and ORs the returned flag).
    fn compute_inj_currents(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        ctx: &mut InjComputeCtx,
    ) -> bool {
        if !self.cd.enabled {
            return false;
        }
        let mut y_changed = false;
        if sys.loads_need_updating {
            self.set_nominal_der_output(sys);
            // Pascal `set_YprimInvalid(TRUE)` raises `Solution.SystemYChanged`
            // (CktElement.pas l.245). `SetNominalDEROutput` invalidates YPrim on a
            // state change (idle↔discharging Yeq differs), so signal the solve to
            // rebuild Y after this `GetPCInjCurr` (Solution.pas l.895) — e.g. when a
            // StorageController dispatches a fleet member mid-control-loop. Returned
            // to the caller, which ORs it into `Solution.SystemYChanged`.
            if self.cd.yprim_invalid {
                y_changed = true;
            }
        }
        // r4133 `TStorageObj.InjCurrents` (Storage.pas:2879): `if not ForceInjCurr
        // then CalcInjCurrentArray` — skip only the model recompute when the
        // injection is forced; the set-nominal preamble and the inherited add stay
        // unconditional (caller scatter).
        let mut errors = crate::diag::ErrorLog::new();
        if !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS) {
            self.calc_inj_current_array(sys, node_v, &mut errors);
        }
        // Surface any user/dyna-model trap / missing-model diagnostic through the
        // solution ErrorLog — never a silent fallback on a trapping `.wasm`
        // (WASM_USERMODELS plan §2.9-5, WM.3 precedent). An `abort`-flagged fault
        // (a wasm trap, ABI §6) lifts `SolutionAbort` so the solve stops instead
        // of iterating on a best-effort stale terminal current.
        for d in errors.into_vec() {
            if d.abort {
                *ctx.solution_abort = true;
            }
            ctx.errors.push(d);
        }
        y_changed
    }

    /// Pascal `TStorageObj.GetTerminalCurrents` + `TPCElement` base.
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
        // Pascal `TStorageObj.GetTerminalCurrents` (@ 0.15.0b4): `and (not
        // (Flg.ForceInjCurrents in Flags))` — skip the model recompute when the
        // currents are forced from the DSS language (WP-U1.9).
        if !self.cd.iterminal_solved_for(sys.solution_count)
            && !self.storage_obj_switch_open
            && !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS)
        {
            let mut errors = crate::diag::ErrorLog::new();
            self.calc_storage_model_contribution(sys, node_v, &mut errors);
            // Route the recompute's user/dyna-model diagnostics to the element's
            // deferred-error log (drained by the executive) instead of dropping
            // them (WASM_USERMODELS plan §2.9-5, WM.3 precedent).
            for d in errors.into_vec() {
                self.cd.obj.push_error(d);
            }
        }
        if self.base.gfm_mode {
            // Pascal `TInvBasedPCE.GetCurrents` (GFM override, InvBasedPCE.pas
            // l.211): read `Vterminal` from `NodeV`, then `Curr = YPrim·Vterminal
            // − InjCurrent` — the current through the GFM short-circuit impedance
            // (the model's `Vterminal` above holds the *internal* source phasors).
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

impl InvBasedPce for Storage {
    fn inv_based(&self) -> &InvBasedPceData {
        &self.base
    }
    fn inv_based_mut(&mut self) -> &mut InvBasedPceData {
        &mut self.base
    }
    /// Pascal `TStorageObj.IsStorage`.
    fn is_storage(&self) -> bool {
        true
    }
    /// Pascal `TStorageObj.GetPFPriority`.
    fn get_pf_priority(&self) -> bool {
        self.pf_priority
    }
}

impl Storage {
    /// Pascal `TStorageObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nconds(self.cd.nphases); // Pascal: NConds := Fnphases
            self.cd.yprim_invalid = true;
        }
        self.kv_storage_base = other.kv_storage_base;
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
        self.dispatch_mode = other.dispatch_mode;
        self.base.inverter_curve = other.base.inverter_curve.clone();
        self.base.inverter_curve_obj = other.base.inverter_curve_obj.clone();
        self.base.inverter_curve_ref = other.base.inverter_curve_ref;
        self.storage_class = other.storage_class;
        self.base.voltage_model = other.base.voltage_model;
        self.f_state = other.f_state;
        self.state_changed = other.state_changed;
        self.base.kvar_limit_set = other.base.kvar_limit_set;
        self.base.kvar_limit_neg_set = other.base.kvar_limit_neg_set;
        self.base.fpct_cut_in = other.base.fpct_cut_in;
        self.base.fpct_cut_out = other.base.fpct_cut_out;
        self.base.var_follow_inverter = other.base.var_follow_inverter;
        self.f_kvar_limit = other.f_kvar_limit;
        self.f_kvar_limit_neg = other.f_kvar_limit_neg;
        self.f_kva_rating = other.f_kva_rating;
        self.base.fpct_pmin_no_vars = other.base.fpct_pmin_no_vars;
        self.base.fpct_pmin_kvar_limit = other.base.fpct_pmin_kvar_limit;
        self.kw_out_idling = other.kw_out_idling;
        self.kw_rating = other.kw_rating;
        self.kwh_rating = other.kwh_rating;
        self.kwh_stored = other.kwh_stored;
        self.kwh_reserve = other.kwh_reserve;
        self.kwh_before_update = other.kwh_before_update;
        self.pct_reserve = other.pct_reserve;
        self.discharge_trigger = other.discharge_trigger;
        self.charge_trigger = other.charge_trigger;
        self.pct_charge_eff = other.pct_charge_eff;
        self.pct_discharge_eff = other.pct_discharge_eff;
        self.pct_kw_out = other.pct_kw_out;
        self.pct_kw_in = other.pct_kw_in;
        self.pct_idle_kw = other.pct_idle_kw;
        self.pct_idle_kvar = other.pct_idle_kvar;
        self.charge_time = other.charge_time;
        self.base.pct_r = other.base.pct_r;
        self.base.pct_x = other.base.pct_x;
        self.base.vw_mode = other.base.vw_mode;
        self.base.vv_mode = other.base.vv_mode;
        self.base.drc_mode = other.base.drc_mode;
        self.base.wp_mode = other.base.wp_mode;
        self.base.wv_mode = other.base.wv_mode;
        self.base.avr_mode = other.base.avr_mode;
        // User models: Pascal `UserModel.Name := Other.UserModel.Name` re-`New`s
        // a fresh instance from the same module (`Storage.pas:995-996`); the
        // slot's `Clone` drops the live wasmi instance and re-creates it lazily
        // (WM.4, the WM.3 generator precedent).
        self.base.user_model_name = other.base.user_model_name.clone();
        self.base.user_model_edit = other.base.user_model_edit.clone();
        self.dyna_model_name = other.dyna_model_name.clone();
        self.dyna_model_edit = other.dyna_model_edit.clone();
        self.user_model = other.user_model.clone();
        self.dyna_model = other.dyna_model.clone();
        self.base.dyn_vars.rated_vdc = other.base.dyn_vars.rated_vdc;
        self.base.dyn_vars.sm_threshold = other.base.dyn_vars.sm_threshold;
        self.base.dyn_vars.safe_mode = other.base.dyn_vars.safe_mode;
        self.base.dyn_vars.kp = other.base.dyn_vars.kp;
        self.base.dyn_vars.reset_ibr = other.base.dyn_vars.reset_ibr;
        self.base.gfm_mode = other.base.gfm_mode;
        self.base.force_balanced = other.base.force_balanced;
        self.base.current_limited = other.base.current_limited;
        self.spectrum = other.spectrum.clone();
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }
}

impl DssObject for Storage {
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
            KV => self.kv_storage_base,
            KW => self.base.kw_out,     // Pascal getter reads kW_out directly
            KVAR => self.base.kvar_out, // Pascal Getkvar
            PF => self.base.pf_nominal,
            KVA => self.f_kva_rating,
            PCT_CUTIN => self.base.fpct_cut_in,
            PCT_CUTOUT => self.base.fpct_cut_out,
            KVAR_MAX => self.f_kvar_limit,
            KVAR_MAX_ABS => self.f_kvar_limit_neg,
            PCT_PMIN_NO_VARS => self.base.fpct_pmin_no_vars,
            PCT_PMIN_KVAR_MAX => self.base.fpct_pmin_kvar_limit,
            KW_RATED => self.kw_rating,
            PCT_KW_RATED => self.pct_kw_rated,
            KWH_RATED => self.kwh_rating,
            KWH_STORED => self.kwh_stored,
            PCT_STORED => self.kwh_stored / self.kwh_rating * 100.0, // Pascal GetPctStored
            PCT_RESERVE => self.pct_reserve,
            PCT_DISCHARGE => self.pct_kw_out,
            PCT_CHARGE => self.pct_kw_in,
            PCT_EFF_CHARGE => self.pct_charge_eff,
            PCT_EFF_DISCHARGE => self.pct_discharge_eff,
            PCT_IDLING_KW => self.pct_idle_kw,
            PCT_R => self.base.pct_r,
            PCT_X => self.base.pct_x,
            VMINPU => self.base.vminpu,
            VMAXPU => self.base.vmaxpu,
            DISCHARGE_TRIGGER => self.discharge_trigger,
            CHARGE_TRIGGER => self.charge_trigger,
            TIME_CHARGE_TRIG => self.charge_time,
            KVDC => self.base.dyn_vars.rated_vdc,
            KP => self.base.dyn_vars.kp,
            PITOL => self.base.dyn_vars.ctrl_tol,
            SAFE_VOLTAGE => self.base.dyn_vars.sm_threshold,
            AMP_LIMIT => self.base.dyn_vars.i_limit,
            AMP_LIMIT_GAIN => self.base.dyn_vars.v_error,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Storage has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kv_storage_base = value,
            KW => self.set_kw(value), // Pascal WriteByFunction SetkW
            KVAR => self.kvar_requested = value, // write target (Pascal kvarRequested)
            PF => self.base.pf_nominal = value,
            KVA => self.f_kva_rating = value,
            PCT_CUTIN => self.base.fpct_cut_in = value,
            PCT_CUTOUT => self.base.fpct_cut_out = value,
            // Pascal `Transform_Abs` on kvarMax/kvarMaxAbs: store the magnitude.
            KVAR_MAX => self.f_kvar_limit = value.abs(),
            KVAR_MAX_ABS => self.f_kvar_limit_neg = value.abs(),
            PCT_PMIN_NO_VARS => self.base.fpct_pmin_no_vars = value,
            PCT_PMIN_KVAR_MAX => self.base.fpct_pmin_kvar_limit = value,
            KW_RATED => self.kw_rating = value,
            PCT_KW_RATED => self.pct_kw_rated = value,
            KWH_RATED => self.kwh_rating = value,
            KWH_STORED => self.kwh_stored = value,
            // Pascal SetPctStored.
            PCT_STORED => self.kwh_stored = value * 0.01 * self.kwh_rating,
            PCT_RESERVE => self.pct_reserve = value,
            PCT_DISCHARGE => self.pct_kw_out = value,
            PCT_CHARGE => self.pct_kw_in = value,
            PCT_EFF_CHARGE => self.pct_charge_eff = value,
            PCT_EFF_DISCHARGE => self.pct_discharge_eff = value,
            PCT_IDLING_KW => self.pct_idle_kw = value,
            PCT_R => self.base.pct_r = value,
            PCT_X => self.base.pct_x = value,
            VMINPU => self.base.vminpu = value,
            VMAXPU => self.base.vmaxpu = value,
            DISCHARGE_TRIGGER => self.discharge_trigger = value,
            CHARGE_TRIGGER => self.charge_trigger = value,
            TIME_CHARGE_TRIG => self.charge_time = value,
            KVDC => self.base.dyn_vars.rated_vdc = value,
            KP => self.base.dyn_vars.kp = value,
            PITOL => self.base.dyn_vars.ctrl_tol = value,
            SAFE_VOLTAGE => self.base.dyn_vars.sm_threshold = value,
            AMP_LIMIT => self.base.dyn_vars.i_limit = value,
            AMP_LIMIT_GAIN => self.base.dyn_vars.v_error = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Storage has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            CONN => self.base.connection as i32,
            STATE => self.f_state,
            MODEL => self.base.voltage_model,
            CLS => self.storage_class,
            DISP_MODE => self.dispatch_mode.ordinal(),
            CONTROL_MODE => self.base.gfm_mode as i32,
            _ => unreachable!("Storage has no integer property {idx}"),
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
            // Plain field write (Pascal MappedStringEnum on FState; the kWh-limit
            // check is only on the internal `StorageState` property path).
            STATE => self.f_state = value,
            MODEL => self.base.voltage_model = value,
            CLS => self.storage_class = value,
            DISP_MODE => {
                self.dispatch_mode =
                    StorageDispatchMode::from_ordinal(value).unwrap_or(self.dispatch_mode)
            }
            CONTROL_MODE => self.base.gfm_mode = value != 0,
            _ => unreachable!("Storage has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            VAR_FOLLOW_INVERTER => self.base.var_follow_inverter,
            WATT_PRIORITY => self.p_priority,
            PF_PRIORITY => self.pf_priority,
            BALANCED => self.base.force_balanced,
            LIMIT_CURRENT => self.base.current_limited,
            DEBUGTRACE => self.base.debug_trace,
            SAFE_MODE => self.base.dyn_vars.safe_mode,
            ENABLED => self.cd.enabled,
            _ => unreachable!("Storage has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            VAR_FOLLOW_INVERTER => self.base.var_follow_inverter = value,
            WATT_PRIORITY => self.p_priority = value,
            PF_PRIORITY => self.pf_priority = value,
            BALANCED => self.base.force_balanced = value,
            LIMIT_CURRENT => self.base.current_limited = value,
            // The trace *file* is not ported (like Generator); store the flag.
            DEBUGTRACE => self.base.debug_trace = value,
            SAFE_MODE => {} // Pascal SilentReadOnly: ignore writes
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Storage has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            EFF_CURVE => self.base.inverter_curve.clone(),
            YEARLY => self.base.yearly_shape.clone(),
            DAILY => self.base.daily_shape.clone(),
            DUTY => self.base.duty_shape.clone(),
            DYNAMIC_EQ => self.base.dyneq.dynamic_eq.clone(),
            DYNA_DLL => self.dyna_model_name.clone(),
            DYNA_DATA => self.dyna_model_edit.clone(),
            USERMODEL => self.base.user_model_name.clone(),
            USERDATA => self.base.user_model_edit.clone(),
            SPECTRUM => self.spectrum.clone(),
            // Pascal DeprecatedAndRemoved: the `?` getter renders ''.
            PCT_IDLING_KVAR => String::new(),
            _ => unreachable!("Storage has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            // `UserModel=`/`DynaDLL=` store the name; the deferred load is queued
            // by `side_effects` and resolved by the executive (WM.4 §2.4).
            DYNA_DLL => self.dyna_model_name = value,
            DYNA_DATA => self.dyna_model_edit = value,
            USERMODEL => self.base.user_model_name = value,
            USERDATA => self.base.user_model_edit = value,
            SPECTRUM => self.spectrum = value,
            // Pascal DeprecatedAndRemoved: a write does nothing.
            PCT_IDLING_KVAR => {}
            _ => unreachable!("Storage has no string property {idx}"),
        }
    }

    /// `DynOut` (Pascal `StringListProperty` via `Set/GetDynOutputNames`): the
    /// dynamics output-variable selection, resolved against the linked
    /// `DynamicExp` to output indices and reconstructed for the dump.
    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::DYN_OUT => self.base.dyneq.get_dyn_output_names(),
            _ => unreachable!("Storage has no string-list property {idx}"),
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
            _ => unreachable!("Storage has no string-list property {idx}"),
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
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use prop::*;
        let elem_ref = resolved.map(|o| o.id());
        let load_shape = || resolved.and_then(|o| o.cloned::<LoadShapeObj>());
        let xy_curve = || resolved.and_then(|o| o.cloned::<XyCurveObj>());
        match idx {
            EFF_CURVE => {
                self.base.inverter_curve = name;
                self.base.inverter_curve_ref = elem_ref;
                self.base.inverter_curve_obj = xy_curve();
            }
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
            DYNAMIC_EQ => {
                self.base.dyneq.dynamic_eq = name;
                self.base.dyneq.dynamic_eq_ref = elem_ref;
                self.base.dyneq.dynamic_eq_obj = resolved.and_then(|o| o.cloned::<DynamicExpObj>());
            }
            _ => unreachable!("Storage has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TStorageObj.PropertySideEffects` (text-parser path).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            PHASES => {
                let n = nconds_for_connection(self.base.connection, self.cd.nphases);
                self.cd.set_nconds(n); // force reallocation of terminal info
            }
            CONN => {
                let n = nconds_for_connection(self.base.connection, self.cd.nphases);
                self.cd.set_nconds(n);
                self.update_vbase();
                self.base.v_base_min = self.base.vminpu * self.base.v_base;
                self.base.v_base_max = self.base.vmaxpu * self.base.v_base;
                self.cd.yprim_invalid = true;
            }
            KV => self.update_vbase(),
            KVA => {
                self.kva_set = true;
                if !self.base.kvar_limit_set {
                    self.f_kvar_limit = self.f_kva_rating;
                }
                if !self.base.kvar_limit_set && !self.base.kvar_limit_neg_set {
                    self.f_kvar_limit_neg = self.f_kva_rating;
                }
            }
            PF => {
                self.base.var_mode = VARMODE_PF;
                self.cd.obj.clear_seq(KVAR);
            }
            KVAR => {
                self.base.var_mode = VARMODE_KVAR;
                self.cd.obj.clear_seq(PF);
            }
            KVAR_MAX => {
                self.base.kvar_limit_set = true;
                if !self.base.kvar_limit_neg_set {
                    self.f_kvar_limit_neg = self.f_kvar_limit.abs();
                }
            }
            KVAR_MAX_ABS => self.base.kvar_limit_neg_set = true,
            KW_RATED => {
                if !self.kva_set {
                    self.f_kva_rating = self.kw_rating;
                }
            }
            KWH_RATED => {
                self.kwh_stored = self.kwh_rating; // assume fully charged
                self.kwh_before_update = self.kwh_stored;
                self.kwh_reserve = self.kwh_rating * self.pct_reserve * 0.01;
            }
            PCT_RESERVE => self.kwh_reserve = self.kwh_rating * self.pct_reserve * 0.01,
            CONTROL_MODE => {
                if self.base.gfm_mode {
                    self.base.dyn_vars.reset_ibr = false;
                }
                self.cd.yprim_invalid = true;
            }
            // Pascal `TProp.DynamicEq` side effect: size the DynamicEqVals memory
            // to the linked DynamicExp's NVariables (a nil ref leaves it empty).
            DYNAMIC_EQ => self.base.dyneq.on_dynamic_eq_set(),
            // WASM_USERMODELS WM.4 — the §2.4 uniform activation rule. The Pascal
            // edit dispatch (Storage.pas:851-866): `UserModel.Name` (load) then
            // `UserData` (edit); `DynaModel.Name` (load) then `DynaData` (edit).
            // The filesystem/current-dir are unreachable from the property hook,
            // so each records a deferred request the executive resolves before
            // `end_edit` (a `.wasm` loads; a native-DLL name / missing file warns
            // "Not Loaded" 1570 and falls back).
            USERMODEL => {
                self.queue_user_model_load(UserModelSlot::User, self.base.user_model_name.clone())
            }
            USERDATA => {
                self.queue_user_model_edit(UserModelSlot::User, self.base.user_model_edit.clone())
            }
            DYNA_DLL => {
                self.queue_user_model_load(UserModelSlot::Dyna, self.dyna_model_name.clone())
            }
            DYNA_DATA => {
                self.queue_user_model_edit(UserModelSlot::Dyna, self.dyna_model_edit.clone())
            }
            _ => {}
        }
    }

    /// Pascal `TStorage.EndEdit`: `RecalcElementData` + Yprim invalidation. `sys`
    /// is the LIVE circuit/solution the executive holds at the edit site.
    fn end_edit(&mut self, sys: &crate::elements::traits::SysCtx) {
        self.recalc(sys);
        self.cd.yprim_invalid = true;
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

    /// Drain the deferred `UserModel=`/`UserData=`/`DynaDLL=`/`DynaData=`
    /// requests queued by the property side effects (WASM_USERMODELS WM.4, §2.4).
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

impl crate::elements::pc::dyneq_pce::DynEqPce for Storage {
    fn dyneq(&self) -> &crate::elements::pc::dyneq_pce::DynEqPceData {
        &self.base.dyneq
    }
    fn dyneq_mut(&mut self) -> &mut crate::elements::pc::dyneq_pce::DynEqPceData {
        &mut self.base.dyneq
    }
}
