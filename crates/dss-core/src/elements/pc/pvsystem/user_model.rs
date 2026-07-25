//! The dss-core side of the WASM user-model host for PVSystem (`UserModel=`,
//! WASM_USERMODELS WM.4).
//!
//! Mirrors the Pascal `UserModel: TPVsystemUserModel` (15-fn,
//! `PVsystem.pas:228`; `PVSystemUserModel.pas`): a bound model is (re)created by
//! `Set_Name` (guest `new(dynarec)` — no `GeneratorVars` pointer crosses,
//! ABI doc §1), holds a live wasmi instance, and services the power-flow
//! (`DoUserModel` → `calc`, `PVsystem.pas:1822`), dynamics (`DoDynamicMode`
//! VoltageModel=3 → `calc`, `:1885`; `IntegrateStates` → `integrate`, `:2280`)
//! and state-variable (`num_vars`/`get_all_vars`/`get_variable`/`set_variable`/
//! `get_var_name`, `:2449-2640`) call sites. PVSystem has NO `InitStateVars`
//! user-model call (unlike Generator/Storage — `PVsystem.pas:2174` never calls
//! `FInit`). Same warn-and-fallback contract (plan §2.4 / ABI doc §5).

use std::sync::Arc;

use num_complex::Complex64;

use dss_usermodel::{
    Callbacks, DynamicsRec, Effect, HostConfig, InterfaceKind, NoCallbacks, Shuttle,
    UserModelError, UserModelHost, UserModelInstance,
};

use crate::diag::{DssDiagnostic, ErrorLog};
use crate::elements::traits::SysCtx;
use crate::obj::base::{UserModelAction, UserModelLoad, UserModelSlot};
use crate::support::dynamics::IterationFlag;

use super::PVSystem;

/// One bound PVSystem user model (Pascal `TPVsystemUserModel`, 15-fn). Held on
/// the [`PVSystem`] as `Option<Box<PvUserModelSlot>>`. [`Clone`] clones the load
/// spec and drops the live instance (Pascal `MakeLike` re-`New`s,
/// `PVsystem.pas:820`).
pub struct PvUserModelSlot {
    model: String,
    wasm: Arc<[u8]>,
    yorder: usize,
    data: String,
    live: Option<Box<UserModelInstance>>,
    num_vars: usize,
    var_names: Vec<String>,
}

impl Clone for PvUserModelSlot {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            wasm: Arc::clone(&self.wasm),
            yorder: self.yorder,
            data: self.data.clone(),
            live: None,
            num_vars: self.num_vars,
            var_names: self.var_names.clone(),
        }
    }
}

impl std::fmt::Debug for PvUserModelSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PvUserModelSlot")
            .field("model", &self.model)
            .field("exists", &self.live.is_some())
            .field("num_vars", &self.num_vars)
            .finish_non_exhaustive()
    }
}

type LiveResult = Result<(), UserModelError>;

impl PvUserModelSlot {
    /// Load + instantiate from resolved `.wasm` bytes (Pascal `Set_Name` →
    /// `FNew`, `PVSystemUserModel.pas`).
    pub fn load(
        model: &str,
        wasm: &[u8],
        yorder: usize,
        p: &PVSystem,
        sys: &SysCtx,
    ) -> Result<Self, UserModelError> {
        let host = UserModelHost::load(
            model,
            wasm,
            InterfaceKind::PvSystemUserModel,
            HostConfig::default(),
        )?;
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, &[]);
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        let instance = UserModelInstance::new(&host, yorder.max(1), sh)?;
        let mut slot = Self {
            model: model.to_string(),
            wasm: Arc::from(wasm.to_vec()),
            yorder: yorder.max(1),
            data: String::new(),
            live: Some(Box::new(instance)),
            num_vars: 0,
            var_names: Vec::new(),
        };
        slot.refresh_var_cache()?;
        Ok(slot)
    }

    pub fn exists(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.exists())
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn num_vars(&self) -> usize {
        self.num_vars
    }

    pub fn var_name(&self, k: usize) -> Option<&str> {
        self.var_names.get(k.wrapping_sub(1)).map(String::as_str)
    }

    fn ensure_live(&mut self, p: &PVSystem, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        if self.live.is_some() {
            return Ok(());
        }
        let host = UserModelHost::load(
            &self.model,
            &self.wasm,
            InterfaceKind::PvSystemUserModel,
            HostConfig::default(),
        )?;
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, node_v);
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        let instance = UserModelInstance::new(&host, self.yorder, sh)?;
        self.live = Some(Box::new(instance));
        if !self.data.is_empty() {
            self.edit_impl(&self.data.clone(), p, sys, node_v)?;
        }
        self.refresh_var_cache()?;
        Ok(())
    }

    /// Pascal `Edit` — send the `UserData=` string (ignored while absent).
    pub fn edit(
        &mut self,
        data: &str,
        p: &PVSystem,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.data = data.to_string();
        if self.live.is_none() {
            return Ok(());
        }
        self.edit_impl(data, p, sys, node_v)?;
        self.refresh_var_cache()?;
        Ok(())
    }

    fn edit_impl(
        &mut self,
        data: &str,
        p: &PVSystem,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, node_v);
        let live = self.live.as_mut().expect("edit_impl requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.edit(data, sh)
    }

    /// Pascal `FUpdateModel` (`PVsystem.pas:1147`) — after `RecalcElementData`.
    pub fn update_model(&mut self, p: &PVSystem, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        if self.live.is_none() {
            return Ok(());
        }
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, node_v);
        {
            let live = self
                .live
                .as_mut()
                .expect("update_model requires a live model");
            let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
            live.update_model(sh)?;
        }
        self.refresh_var_cache()?;
        Ok(())
    }

    /// Pascal `FCalc(V, I)` — power-flow / dynamics current computation.
    pub fn calc(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        p: &PVSystem,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.ensure_live(p, sys, node_v)?;
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, node_v);
        let live = self.live.as_mut().expect("calc requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.calc(v, i, sh)
    }

    /// Pascal `Integrate` — `IntegrateStates` (`PVsystem.pas:2280`).
    pub fn integrate(&mut self, p: &PVSystem, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        self.ensure_live(p, sys, node_v)?;
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, node_v);
        let live = self.live.as_mut().expect("integrate requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.integrate(sh)
    }

    /// Pascal `FGetAllVars(@States[base])` (`PVsystem.pas:2563`).
    pub fn get_all_vars(
        &mut self,
        out: &mut [f64],
        p: &PVSystem,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, node_v);
        let live = self
            .live
            .as_mut()
            .expect("get_all_vars requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.get_all_vars(out, sh)
    }

    /// Pascal `FGetVariable(k)` (1-based, `PVsystem.pas:2459`).
    pub fn get_variable(
        &mut self,
        k: usize,
        p: &PVSystem,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> Result<f64, UserModelError> {
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, node_v);
        let live = self
            .live
            .as_mut()
            .expect("get_variable requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.get_variable(k as i32, sh)
    }

    /// Pascal `FSetVariable(k, value)` (1-based, `PVsystem.pas:2540`).
    pub fn set_variable(
        &mut self,
        k: usize,
        value: f64,
        p: &PVSystem,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut dr = dyn_rec_from(sys);
        let ctx = PvCallbacks::snapshot(p, sys, node_v);
        let live = self
            .live
            .as_mut()
            .expect("set_variable requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.set_variable(k as i32, value, sh)
    }

    /// Drain the tier-B effects (MsgCallback text) queued by the last guest call.
    pub fn drain_effects(&mut self, name: &str, errors: &mut ErrorLog) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        for eff in live.drain_effects() {
            match eff {
                Effect::Msg(text) => errors.push(DssDiagnostic::msg(text, Some(9000))),
                Effect::ControlQueuePush { .. } => errors.push(DssDiagnostic::msg(
                    format!(
                        "PVSystem.{name}: user model called ControlQueuePush, which is not \
                         wired for PVSystem elements (CapControl scope, WM.5)."
                    ),
                    Some(9001),
                )),
            }
        }
    }

    /// Refresh the cached `num_vars` + names from the live model. A guest trap in
    /// `num_vars`/`get_var_name` is surfaced (propagated), never swallowed to a
    /// silent `num_vars=0` that would drop the model's state-var tail (plan
    /// §2.9-5 loud-error policy; WM.4 settle T-WM4-3 — the caller drains it).
    fn refresh_var_cache(&mut self) -> LiveResult {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        let mut dr = DynamicsRec::default();
        let n = {
            let sh = Shuttle::without_gen_vars(&mut dr, Box::new(NoCallbacks));
            live.num_vars(sh)?.max(0) as usize
        };
        let mut names = Vec::with_capacity(n);
        for k in 1..=n {
            let mut dr2 = DynamicsRec::default();
            let sh = Shuttle::without_gen_vars(&mut dr2, Box::new(NoCallbacks));
            names.push(live.get_var_name(k as i32, sh)?);
        }
        self.num_vars = n;
        self.var_names = names;
        Ok(())
    }
}

impl PVSystem {
    /// Queue a deferred `UserModel=` (re)load (Pascal `UserModel.Name := …`).
    pub(super) fn queue_user_model_load(&mut self, name: String) {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            self.user_model = None;
            return;
        }
        self.pending_user_model_loads.push(UserModelLoad {
            slot: UserModelSlot::User,
            action: UserModelAction::Load(name),
        });
    }

    /// Queue a deferred `UserData=` edit.
    pub(super) fn queue_user_model_edit(&mut self, data: String) {
        self.pending_user_model_loads.push(UserModelLoad {
            slot: UserModelSlot::User,
            action: UserModelAction::Edit(data),
        });
    }

    /// The executive-drained apply for a resolved [`UserModelLoad`] (§2.4).
    pub(super) fn apply_user_model_load_impl(
        &mut self,
        load: &UserModelLoad,
        wasm: Option<&[u8]>,
        sys: &SysCtx,
        errors: &mut ErrorLog,
    ) {
        let name = self.cd.obj.name().to_string();
        match &load.action {
            UserModelAction::Load(model_name) => {
                self.user_model = None; // Pascal Set_Name frees the previous first.
                match wasm {
                    Some(bytes) => {
                        let yorder = self.cd.yorder;
                        match PvUserModelSlot::load(model_name, bytes, yorder, self, sys) {
                            Ok(slot) => self.user_model = Some(Box::new(slot)),
                            Err(e) => push_load_failure(&name, model_name, &e, errors),
                        }
                    }
                    None => errors.push(DssDiagnostic::msg(
                        not_loaded_text(&name, model_name),
                        Some(1570),
                    )),
                }
            }
            UserModelAction::Edit(data) => {
                let Some(mut s) = self.user_model.take() else {
                    return; // Pascal: `if UserModel.Exists then Edit`.
                };
                let mut errs = ErrorLog::new();
                if let Err(e) = s.edit(data, self, sys, &[]) {
                    errs.push(DssDiagnostic::msg(e.to_string(), Some(1569)));
                }
                s.drain_effects(&name, &mut errs);
                self.user_model = Some(s);
                errors.extend(errs.into_vec());
            }
        }
    }

    /// Whether the `UserModel=` slot holds a loaded model (Pascal
    /// `UserModel.Exists`).
    pub(super) fn user_model_exists(&self) -> bool {
        self.user_model.as_ref().is_some_and(|s| s.exists())
    }

    /// Pascal `DoUserModel` / `DoDynamicMode` VoltageModel=3 FCalc
    /// (`PVsystem.pas:1832`/`:1889`): `UserModel.FCalc(Vterminal, Iterminal)`,
    /// `set_ITerminalUpdated(TRUE)`, negate the terminal currents into
    /// `InjCurrent`. Returns `true` iff a model existed and ran. `Vterminal`/
    /// `InjCurrent` are computed by the caller (`CalcYPrimContribution` first).
    pub(super) fn user_model_fcalc(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut ErrorLog,
    ) -> bool {
        let Some(mut um) = self.user_model.take() else {
            return false;
        };
        if !um.exists() {
            self.user_model = Some(um);
            return false;
        }
        let name = self.cd.obj.name().to_string();
        let v = self.cd.vterminal.clone();
        let mut it = self.cd.iterminal.clone();
        match um.calc(&v, &mut it, self, sys, node_v) {
            Ok(()) => {
                self.cd.iterminal.copy_from_slice(&it);
                self.cd.iterminal_updated = true;
                self.cd.mark_iterminal_solved(sys.solution_count);
                for i in 0..self.cd.nconds {
                    self.cd.inj_current[i] -= self.cd.iterminal[i];
                }
            }
            Err(e) => errors.push(DssDiagnostic::abort(
                format!("PVSystem.{name}: user model `calc` trapped/faulted: {e}"),
                Some(567),
            )),
        }
        um.drain_effects(&name, errors);
        self.user_model = Some(um);
        true
    }

    /// Pascal `RecalcElementData` tail (`PVsystem.pas:1146-1147`):
    /// `UserModel.FUpdateModel`. Called at the end of [`PVSystem::recalc`].
    pub(super) fn update_user_models(&mut self, sys: &SysCtx) {
        let Some(mut s) = self.user_model.take() else {
            return;
        };
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = s.update_model(self, sys, &[]) {
            errs.push(DssDiagnostic::msg(e.to_string(), Some(1569)));
        }
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    /// Pascal `IntegrateStates` UserModel tail (`PVsystem.pas:2278-2282`):
    /// `UserModel.Integrate()`. No-op when absent. Returns `true` iff it ran.
    pub(super) fn user_model_fintegrate(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> bool {
        if !self.user_model_exists() {
            return false;
        }
        let Some(mut um) = self.user_model.take() else {
            return false;
        };
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = um.integrate(self, sys, node_v) {
            errs.push(DssDiagnostic::msg(
                format!("PVSystem.{name}: user model `integrate` failed: {e}"),
                Some(1569),
            ));
        }
        um.drain_effects(&name, &mut errs);
        self.user_model = Some(um);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
        true
    }

    /// The number of `UserModel=` variables (0 when absent) — Pascal `if
    /// UserModel.Exists then Result += UserModel.FNumVars` (`PVsystem.pas:2575`).
    pub(super) fn num_user_model_variables(&self) -> usize {
        self.user_model
            .as_ref()
            .filter(|s| s.exists())
            .map_or(0, |s| s.num_vars())
    }

    /// Pascal `VariableName` UserModel tail (`PVsystem.pas:2627-2635`): resolve a
    /// 1-based index above the classic `NumPVSystemVariables` base to a
    /// `UserModel` name.
    pub(super) fn user_model_variable_name(&self, i: usize) -> Option<String> {
        let base = self.num_pv_variables();
        if i <= base {
            return None;
        }
        let k = i - base;
        if let Some(um) = self.user_model.as_ref().filter(|s| s.exists())
            && k <= um.num_vars()
        {
            return Some(um.var_name(k).unwrap_or_default().to_string());
        }
        None
    }

    /// Fill `out` with the `UserModel` variable values (Pascal
    /// `FGetAllVars(@States[NumPVSystemVariables])`, `PVsystem.pas:2563`).
    pub(super) fn get_all_vars_slot(
        &mut self,
        out: &mut [f64],
        sys: &SysCtx,
        node_v: &[Complex64],
    ) {
        let Some(mut s) = self.user_model.take() else {
            return;
        };
        if !s.exists() {
            self.user_model = Some(s);
            return;
        }
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = s.get_all_vars(out, self, sys, node_v) {
            errs.push(DssDiagnostic::msg(
                format!("PVSystem.{name}: user model `get_all_vars` failed: {e}"),
                Some(1569),
            ));
        }
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    /// Pascal `Get_Variable` UserModel tail (`PVsystem.pas:2453-2461`): resolve a
    /// 1-based index above the classic `NumPVSystemVariables` base to a `UserModel`
    /// value (`None` when no loaded model / index out of range). Mirrors the
    /// Storage sibling's `get_user_model_variable` (WM.4 settle T-WM4-2).
    pub(super) fn get_user_model_variable(
        &mut self,
        i: usize,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> Option<f64> {
        let base = self.num_pv_variables();
        if i <= base {
            return None;
        }
        let k = i - base;
        let mut s = self.user_model.take()?;
        if !(s.exists() && k <= s.num_vars()) {
            self.user_model = Some(s);
            return None;
        }
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        let val = match s.get_variable(k, self, sys, node_v) {
            Ok(v) => Some(v),
            Err(e) => {
                errs.push(DssDiagnostic::msg(
                    format!("PVSystem.{name}: user model `get_variable` failed: {e}"),
                    Some(1569),
                ));
                Some(-9999.99)
            }
        };
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
        val
    }

    /// Pascal `Set_Variable` UserModel tail (`PVsystem.pas:2534-2541`). Returns
    /// `true` iff the user model handled the write.
    pub(super) fn set_user_model_variable(&mut self, i: usize, value: f64, sys: &SysCtx) -> bool {
        let base = self.num_pv_variables();
        if i <= base {
            return false;
        }
        let k = i - base;
        let Some(mut s) = self.user_model.take() else {
            return false;
        };
        if !(s.exists() && k <= s.num_vars()) {
            self.user_model = Some(s);
            return false;
        }
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = s.set_variable(k, value, self, sys, &[]) {
            errs.push(DssDiagnostic::msg(
                format!("PVSystem.{name}: user model `set_variable` failed: {e}"),
                Some(1569),
            ));
        }
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
        true
    }
}

/// Map a load-time [`UserModelError`] onto the Pascal load-failure diagnostic.
fn push_load_failure(name: &str, model_name: &str, e: &UserModelError, errors: &mut ErrorLog) {
    match e {
        UserModelError::MissingExport { .. } | UserModelError::SignatureMismatch { .. } => {
            errors.push(DssDiagnostic::msg(e.to_string(), Some(1569)));
        }
        _ => errors.push(DssDiagnostic::msg(
            not_loaded_text(name, model_name),
            Some(1570),
        )),
    }
}

/// The Pascal warn-and-fallback text (`PVSystemUserModel.pas:153`), model
/// absent, built-in model solves.
fn not_loaded_text(name: &str, model_name: &str) -> String {
    format!(
        "PVSystem User Model {model_name} Not Loaded. PVSystem.{name} falls back to its \
         built-in model."
    )
}

/// Build the packed `TDynamicsRec` image from the solution context (ABI §2.1).
fn dyn_rec_from(sys: &SysCtx) -> DynamicsRec {
    DynamicsRec {
        h: sys.dyna_h,
        t: sys.dyna_t,
        tstart: 0.0,
        tstop: 0.0,
        iteration_flag: match sys.iteration_flag {
            IterationFlag::NewTimeStep => 0,
            IterationFlag::SameTimeStep => 1,
        },
        solution_mode: sys.mode.ordinal(),
        int_hour: 0,
        dbl_hour: sys.dbl_hour,
    }
}

/// An owned per-call context snapshot serving the tier-A `dss_env` reads for the
/// owning PVSystem (ABI doc §4).
struct PvCallbacks {
    name: String,
    voltages: Vec<Complex64>,
    currents: Vec<Complex64>,
    node_v: Vec<Complex64>,
    dyn_rec: DynamicsRec,
    nterms: i32,
    nconds: i32,
    nphases: i32,
    enabled: bool,
}

impl PvCallbacks {
    fn snapshot(p: &PVSystem, sys: &SysCtx, node_v: &[Complex64]) -> Self {
        let yorder = p.cd.yorder;
        let voltages: Vec<Complex64> = (0..yorder)
            .map(|i| {
                p.cd.node_ref
                    .get(i)
                    .and_then(|&nr| node_v.get(nr).copied())
                    .unwrap_or(Complex64::ZERO)
            })
            .collect();
        let node_slice = if node_v.len() > 1 {
            node_v[1..].to_vec()
        } else {
            Vec::new()
        };
        PvCallbacks {
            name: format!("PVSystem.{}", p.cd.obj.name()),
            voltages,
            currents: p.cd.iterminal.clone(),
            node_v: node_slice,
            dyn_rec: dyn_rec_from(sys),
            nterms: p.cd.nterms as i32,
            nconds: p.cd.nconds as i32,
            nphases: p.cd.nphases as i32,
            enabled: p.cd.enabled,
        }
    }
}

impl Callbacks for PvCallbacks {
    fn active_element_name(&self) -> Option<&str> {
        Some(&self.name)
    }
    fn active_element_voltages(&self) -> Option<&[Complex64]> {
        Some(&self.voltages)
    }
    fn active_element_currents(&self) -> Option<&[Complex64]> {
        Some(&self.currents)
    }
    fn node_voltages(&self) -> &[Complex64] {
        &self.node_v
    }
    fn dynamics_rec(&self) -> Option<DynamicsRec> {
        Some(self.dyn_rec)
    }
    fn step_size(&self) -> f64 {
        self.dyn_rec.h
    }
    fn time_sec(&self) -> f64 {
        self.dyn_rec.t
    }
    fn time_hr(&self) -> f64 {
        self.dyn_rec.dbl_hour
    }
    fn active_element_terminal_info(&self) -> Option<(i32, i32, i32)> {
        Some((self.nterms, self.nconds, self.nphases))
    }
    fn is_active_element_enabled(&self) -> bool {
        self.enabled
    }
}
