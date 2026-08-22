//! Trait impls: `CktElement` (Yprim build, injection/terminal currents) and
//! `DssObject` (typed property getters/setters, shape resolution, side effects,
//! `MakeLike`).

use num_complex::Complex64;

use crate::elements::ckt::{CktElementData, ElemFlags};
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, PosSeqPlan};
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::obj::arena::ResolvedObj;
use crate::obj::base::{DssObjData, DssObject, UserModelLoad, UserModelSlot};
use crate::support::cmatrix::CMatrix;
use crate::util::sqrt3;

use super::{Connection, GenDispatchMode, Generator, nconds_for_connection, prop};

impl CktElement for Generator {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    /// Pascal `TGeneratorObj.MakePosSequence` (`generator.pas:2726`). Single
    /// phase, line-neutral; a multi-phase generator's power is divided by the
    /// phase count (PF preserved), and — conditionally — its kvar limits, kVA
    /// and MVA ratings.
    ///
    /// The `had_kVA`/`had_MVA` guards read the two properties they are named
    /// after. The authority's raw slots are correct there: r4133 tests
    /// `PrpSequence^[26]`/`^[27]`
    /// (`Version8/Source/PCElements/generator.pas:3060-3062`) and registers
    /// `kVA` at 26, `MVA` at 27 (`:459-460`), with `Xdp`/`Xdpp` out at 29/30
    /// (`:463`/`:465`). dss_capi's enum reorder slid `Xdp`/`Xdpp` into 26/27 and
    /// left the literals behind (`Generator.pas:2804-2806`), so there a
    /// generator declared with `kVA=` keeps its full rating while one declared
    /// with `Xdp=` has its untouched rating divided. That regression is not
    /// reproduced. `had_kvars` reads `[19]`/`[20]` (`Maxkvar`/`Minkvar`) — raw
    /// slots that are correct in both trees.
    ///
    /// Every number above is an **upstream** slot: old-OpenDSS keys
    /// `PropertyValue`/`PrpSequence` by `AddProperty`'s `CmdMapIndex`, not by
    /// display position. The port has one index space (display order) and
    /// addresses it through `prop::`, so nothing here needs translating — but do
    /// not read `26` as a port ordinal: R4133_PROPS RP1.1 inserted
    /// `Rneut`/`Xneut` at display 16/17, which moved the port's `KVA` to 25.
    fn make_pos_sequence(&mut self, _ctx: &PosSeqCtx) -> PosSeqPlan {
        // Make sure voltage is line-neutral.
        let v = if self.cd.nphases > 1 || self.connection != Connection::Wye {
            self.kv_generator_base / sqrt3()
        } else {
            self.kv_generator_base
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
            // 19/20 are `Maxkvar`/`Minkvar` — the raw pair that is right in
            // both trees.
            let had_kvars = self.cd.obj.prp_specified(prop::MAXKVAR)
                || self.cd.obj.prp_specified(prop::MINKVAR);

            let kw_new = self.kw_base / nph;
            let pf_new = self.pf_nominal;
            actions.push(PosSeqAction::SetF64(prop::KW, kw_new));
            actions.push(PosSeqAction::SetF64(prop::PF, pf_new));
            if had_kvars {
                actions.push(PosSeqAction::SetF64(prop::MINKVAR, self.kvar_min / nph));
                actions.push(PosSeqAction::SetF64(prop::MAXKVAR, self.kvar_max / nph));
            }
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

    /// Pascal `TGeneratorObj.NumVariables` (`generator.pas:2717-2726`): the
    /// linked `DynamicExp` count first, else the classic GenVars count plus the
    /// existing `UserModel`/`ShaftModel` variable counts (WASM_USERMODELS WM.3).
    fn num_variables(&self) -> usize {
        let n = self.dyneq.num_variables();
        if n != 0 {
            return n;
        }
        let mut total = self.num_gen_variables();
        if let Some(um) = self.user_model.as_ref().filter(|s| s.exists()) {
            total += um.num_vars();
        }
        if let Some(sm) = self.shaft_model.as_ref().filter(|s| s.exists()) {
            total += sm.num_vars();
        }
        total
    }

    /// Pascal `TGeneratorObj.VariableName` (`generator.pas:2728-2786`): the
    /// `DynamicExp` name first, then the classic table (1..6), then the
    /// `UserModel` names (`k = i - NumGenVariables`), then the `ShaftModel` names.
    ///
    /// **Upstream bug deliberately NOT reproduced** (so: no compat marker —
    /// the tag is for quirks we *do* reproduce). Pascal's ShaftModel branch
    /// (`:2780`) calls `UserModel.FGetVarName` — a genuine upstream bug (should be
    /// `ShaftModel.FGetVarName`); with no UserModel loaded it dereferences a nil
    /// function pointer (an access violation), and out of the UserModel's range
    /// it reads an uninitialized stack buffer. Per the CLAUDE.md rule (UB /
    /// uninitialized-read bugs are NOT reproduced), the port does the correct
    /// thing and queries the `ShaftModel` names. The dyn gate binds the SAME model
    /// as `UserModel=` and `ShaftModel=`, so its 14 shaft names DUPLICATE the user
    /// names — the (correct) ShaftModel names the port returns coincide with what
    /// the buggy upstream `FGetVarName` would read from the UserModel, so comparing
    /// the full ordered 34-name surface (`wasm_usermodels.rs`) neither masks nor
    /// trips over the upstream bug.
    fn variable_name(&self, i: usize) -> String {
        if let Some(name) = self.dyneq.variable_name(i) {
            return name;
        }
        let base = self.num_gen_variables();
        if (1..=base).contains(&i) {
            return self.gen_variable_name(i);
        }
        let un = self
            .user_model
            .as_ref()
            .filter(|s| s.exists())
            .map_or(0, |s| s.num_vars());
        if i > base
            && i <= base + un
            && let Some(um) = self.user_model.as_ref()
        {
            return um.var_name(i - base).unwrap_or_default().to_string();
        }
        if let Some(sm) = self.shaft_model.as_ref().filter(|s| s.exists())
            && i > base + un
            && i <= base + un + sm.num_vars()
        {
            return sm.var_name(i - base - un).unwrap_or_default().to_string();
        }
        self.gen_variable_name(i) // Pascal seeds Result := 'ERROR' for out-of-range
    }

    /// Pascal `TGeneratorObj.GetAllVariables` (`generator.pas:2689-2715`): the
    /// `DynamicExp` memory dump first, else the classic GenVars (`States[0..6]`)
    /// followed by the `UserModel` values (`@States[NumGenVariables]`) and the
    /// `ShaftModel` values (`@States[NumGenVariables + N]`) — WASM_USERMODELS WM.3.
    fn get_all_variables(&mut self, sys: &SysCtx, node_v: &[Complex64], states: &mut [f64]) {
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
        self.get_gen_variables(states);
        let base = self.num_gen_variables();
        let un = self
            .user_model
            .as_ref()
            .filter(|s| s.exists())
            .map_or(0, |s| s.num_vars());
        if un > 0 {
            let end = (base + un).min(states.len());
            if base < end {
                self.get_all_vars_slot(UserModelSlot::User, &mut states[base..end], sys, node_v);
            }
        }
        let sn = self
            .shaft_model
            .as_ref()
            .filter(|s| s.exists())
            .map_or(0, |s| s.num_vars());
        if sn > 0 {
            let start = base + un;
            let end = (start + sn).min(states.len());
            if start < end {
                self.get_all_vars_slot(UserModelSlot::Shaft, &mut states[start..end], sys, node_v);
            }
        }
    }

    /// Pascal `TGeneratorObj.Set_Variable` (`generator.pas:2634-2687`): route a
    /// 1-based state-variable write to the classic GenVars table (1..6), then the
    /// `UserModel` / `ShaftModel`. `i < 1` → `#565`; a DynamicEq generator rejects
    /// writes → `#566`; index 3 (`Vd`) is read-only → `#564`. The diagnostics have
    /// no direct return channel here, so they queue on the element's deferred-error
    /// log (drained by the executive after the `Set StateVar` command).
    fn set_variable(&mut self, i: usize, value: f64, sys: &crate::elements::traits::SysCtx) {
        use super::dynamics::{RADIANS_TO_DEGREES, TWO_PI};
        if i < 1 {
            self.cd.obj.push_error(crate::diag::DssDiagnostic::msg(
                format!(
                    "Generator.{}: invalid variable index {i}.",
                    self.cd.obj.name()
                ),
                Some(565),
            ));
            return;
        }
        if self.dyneq.has_dynamic_eq() {
            self.cd.obj.push_error(crate::diag::DssDiagnostic::msg(
                format!(
                    "Generator.{}: cannot set state variable when using DynamicEq.",
                    self.cd.obj.name()
                ),
                Some(566),
            ));
            return;
        }
        // Classic GenVars setters (`generator.pas:2650-2663`).
        match i {
            1 => self.speed = (value - self.w0) * TWO_PI, // Frequency (Hz) → Speed
            2 => self.theta = value / RADIANS_TO_DEGREES, // deg → rad
            3 => self.cd.obj.push_error(crate::diag::DssDiagnostic::msg(
                format!(
                    "Generator.{}: variable index {i} is read-only.",
                    self.cd.obj.name()
                ),
                Some(564),
            )),
            4 => self.p_shaft = value,
            5 => self.dspeed = value / RADIANS_TO_DEGREES,
            6 => self.dtheta = value,
            _ => self.set_user_model_variable(i, value, sys),
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

    /// Pascal `TGeneratorObj.InjCurrents` + `TPCElement.InjCurrents` (M3b compute
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
            self.set_nominal_generation(sys);
        }
        // r4133 `TGeneratorObj.InjCurrents`: `if not ForceInjCurr then
        // CalcInjCurrentArray` — skip only the model recompute when the injection
        // is forced (`Set InjCurrent=`/`ITerminal=`); the set-nominal preamble and
        // the inherited add-into-Currents stay unconditional (caller scatter).
        let mut errors = crate::diag::ErrorLog::new();
        if !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS) {
            self.calc_inj_current_array(sys, node_v, &mut errors);
        }
        // Surface any user-model trap / missing Model=6 diagnostic through the
        // solution ErrorLog — never a silent fallback on a trapping `.wasm`
        // (WASM_USERMODELS plan §2.9-5). An `abort`-flagged fault (the missing
        // dynamics model, `generator.pas:1944`) lifts `SolutionAbort` so the solve
        // stops instead of iterating on a best-effort stale terminal current.
        for d in errors.into_vec() {
            if d.abort {
                *ctx.solution_abort = true;
            }
            ctx.errors.push(d);
        }
        false
    }

    /// Pascal `TGeneratorObj.GetTerminalCurrents` + `TPCElement` base.
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
        // Pascal `TGeneratorObj.GetTerminalCurrents` (@ 0.15.0b4): `and (not
        // (Flg.ForceInjCurrents in Flags))` — skip the model recompute when the
        // currents are forced from the DSS language (WP-U1.9).
        if !self.cd.iterminal_solved_for(sys.solution_count)
            && !self.gen_switch_open
            && !self.cd.flags.contains(ElemFlags::FORCE_INJ_CURRENTS)
        {
            // `GetTerminalCurrents` has no solve-context error channel, so a
            // user-model trap / missing Model=6 diagnostic recomputed here is
            // queued on the element's deferred-error log (drained by the executive)
            // rather than dropped — never a silent fallback on trap (§2.9-5). The
            // loud path is `inj_currents` during the solve; this query recompute is
            // the fallback surfacing.
            let mut errors = crate::diag::ErrorLog::new();
            self.calc_gen_model_contribution(sys, node_v, &mut errors);
            for d in errors.into_vec() {
                self.cd.obj.push_error(d);
            }
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

impl Generator {
    /// Pascal `TGeneratorObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
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
        // User models: Pascal `UserModel.Name := Other.UserModel.Name` re-`New`s
        // a fresh instance from the same module (`generator.pas:893-894`). The
        // slot's `Clone` drops the live wasmi instance and re-creates it lazily
        // on first use (with the same spec + last `UserData`), which is a benign
        // superset of the Pascal fresh-`New` (no deck exercises `like=` on a
        // user-model generator).
        self.user_model_name = other.user_model_name.clone();
        self.user_data = other.user_data.clone();
        self.shaft_model_name = other.shaft_model_name.clone();
        self.shaft_data = other.shaft_data.clone();
        self.user_model = other.user_model.clone();
        self.shaft_model = other.shaft_model.clone();
        self.spectrum = other.spectrum.clone();
        // Pascal copies the donor's whole `FPropertyValue` array
        // (`generator.pas:828`), which is where the two upstream stubs live — so
        // `like=` carries their strings across even though nothing consumes them.
        self.rneut_text = other.rneut_text.clone();
        self.xneut_text = other.xneut_text.clone();
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }
}

impl DssObject for Generator {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }

    fn as_dyneq(&self) -> Option<&crate::elements::pc::dyneq_pce::DynEqPceData> {
        Some(&self.dyneq)
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
            DISPMODE => self.dispatch_mode.ordinal(),
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
            DISPMODE => {
                self.dispatch_mode =
                    GenDispatchMode::from_ordinal(value).unwrap_or(self.dispatch_mode)
            }
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
            DYNAMICEQ => self.dyneq.dynamic_eq.clone(),
            SPECTRUM => self.spectrum.clone(),
            // The two upstream stubs echo their string store (r4133 has no
            // `GetPropertyValue` override for them, so `DSSObject.pas:112-115`
            // returns `PropertyValue[]` — the parse string or the `'0'` default).
            RNEUT => self.rneut_text.clone(),
            XNEUT => self.xneut_text.clone(),
            _ => unreachable!("Generator has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            // UserModel/UserData/ShaftModel/ShaftData store the value here; the
            // per-property side effect (below) queues the deferred WASM (re)load
            // per WASM_USERMODELS §2.4 — never a parse error. (DynamicEq is an
            // object ref → set_object_ref; DynOut is a string list →
            // set_string_list.)
            USERMODEL => self.user_model_name = value,
            USERDATA => self.user_data = value,
            SHAFTMODEL => self.shaft_model_name = value,
            SHAFTDATA => self.shaft_data = value,
            SPECTRUM => self.spectrum = value,
            // The upstream stubs store and stop — `parse_into`'s `UPSTREAM_STUB`
            // arm emits messages 5611/5612 (`generator.pas:651-652`) and no
            // side effect, recalc or Y invalidation follows.
            RNEUT => self.rneut_text = value,
            XNEUT => self.xneut_text = value,
            _ => unreachable!("Generator has no string property {idx}"),
        }
    }

    /// `DynOut` (Pascal `StringListProperty` via `Set/GetDynOutputNames`): the
    /// dynamics output-variable selection, resolved against the linked
    /// `DynamicExp` to output indices and reconstructed for the dump.
    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::DYNOUT => self.dyneq.get_dyn_output_names(),
            _ => unreachable!("Generator has no string-list property {idx}"),
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
            _ => unreachable!("Generator has no string-list property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape reference (snapshot-clone like the Load shape refs).
    fn set_object_ref(&mut self, idx: usize, name: String, resolved: Option<ResolvedObj<'_>>) {
        use prop::*;
        let load_shape_ref = || resolved.and_then(|o| o.idx::<LoadShapeObj>());
        let load_shape = || resolved.and_then(|o| o.cloned::<LoadShapeObj>());
        match idx {
            YEARLY => {
                self.yearly_shape = name;
                self.yearly_shape_ref = load_shape_ref();
                self.yearly_shape_obj = load_shape();
            }
            DAILY => {
                self.daily_shape = name;
                self.daily_shape_ref = load_shape_ref();
                self.daily_shape_obj = load_shape();
            }
            DUTY => {
                self.duty_shape = name;
                self.duty_shape_ref = load_shape_ref();
                self.duty_shape_obj = load_shape();
            }
            DYNAMICEQ => {
                self.dyneq.dynamic_eq = name;
                self.dyneq.dynamic_eq_ref = resolved
                    .and_then(|o| o.idx::<crate::elements::general::dynamic_exp::DynamicExpObj>());
                self.dyneq.dynamic_eq_obj = resolved.and_then(|o| {
                    o.cloned::<crate::elements::general::dynamic_exp::DynamicExpObj>()
                });
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
            // Pascal `TProp.DynamicEq` side effect: size the DynamicEqVals memory
            // to the linked DynamicExp's NVariables (a nil ref leaves it empty).
            DYNAMICEQ => self.dyneq.on_dynamic_eq_set(),
            // WASM_USERMODELS WM.3 — the `EndEdit` dispatch order
            // (`generator.pas:767-775`): `UserModel.Name` (load) before
            // `UserData` (edit), `ShaftModel.Name` before `ShaftData`. The
            // filesystem/current-dir are unreachable from the property hook, so
            // each records a deferred request the executive resolves before
            // `end_edit` (§2.4 activation rule).
            USERMODEL => {
                self.queue_user_model_load(UserModelSlot::User, self.user_model_name.clone())
            }
            USERDATA => self.queue_user_model_edit(UserModelSlot::User, self.user_data.clone()),
            SHAFTMODEL => {
                self.queue_user_model_load(UserModelSlot::Shaft, self.shaft_model_name.clone())
            }
            SHAFTDATA => self.queue_user_model_edit(UserModelSlot::Shaft, self.shaft_data.clone()),
            _ => {}
        }
    }

    /// Pascal `TGenerator.EndEdit`: `RecalcElementData` + Yprim invalidation.
    /// `sys` is the LIVE circuit/solution the executive holds at the edit site
    /// (Pascal `SetNominalGeneration` reads `ActiveCircuit.Solution` globals).
    fn end_edit(&mut self, sys: &crate::elements::traits::SysCtx) {
        self.recalc(sys);
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TDynEqPCE.ParseDynVar`: a `name=value` whose `name` is a state
    /// variable of the linked `DynamicExp` (the inline `Speed=0 PShaft=P0 …`
    /// initializers).
    fn parse_dyn_var(
        &mut self,
        variable: &str,
        value: &str,
        vars: &dss_parser::ParserVars,
    ) -> bool {
        self.dyneq.parse_dyn_var(variable, value, vars)
    }

    /// Drain the deferred user-model load/edit requests queued by the
    /// `UserModel=`/`UserData=`/`ShaftModel=`/`ShaftData=` side effects (the
    /// executive resolves each path against `current_dir` — WASM_USERMODELS
    /// WM.3, §2.4).
    fn take_user_model_loads(&mut self) -> Vec<UserModelLoad> {
        std::mem::take(&mut self.pending_user_model_loads)
    }

    /// Apply a resolved user-model load/edit (`wasm` is `Some` iff a `.wasm`
    /// file was found + read; `None` → warn-and-fallback, Pascal 570).
    fn apply_user_model_load(
        &mut self,
        load: &UserModelLoad,
        wasm: Option<&[u8]>,
        sys: &crate::elements::traits::SysCtx,
        errors: &mut crate::diag::ErrorLog,
    ) {
        self.apply_user_model_load_impl(load, wasm, sys, errors);
    }
}

impl crate::elements::pc::dyneq_pce::DynEqPce for Generator {
    fn dyneq(&self) -> &crate::elements::pc::dyneq_pce::DynEqPceData {
        &self.dyneq
    }
    fn dyneq_mut(&mut self) -> &mut crate::elements::pc::dyneq_pce::DynEqPceData {
        &mut self.dyneq
    }
}
