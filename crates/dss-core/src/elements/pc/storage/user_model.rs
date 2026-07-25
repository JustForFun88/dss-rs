//! The dss-core side of the WASM user-model host for Storage
//! (`UserModel=` / `DynaDLL=`, WASM_USERMODELS WM.4).
//!
//! Mirrors the Pascal `UserModel: TStoreUserModel` (15-fn, `Storage.pas:281`)
//! and `DynaModel: TStoreDynaModel` (13-fn, no `Save`/`Restore`, `:282`;
//! `StoreUserModel.pas`). A bound model is (re)created by `Set_Name` (guest
//! `new`), holds a live wasmi instance ([`dss_usermodel::UserModelInstance`]),
//! and services the power-flow (`DoUserModel` → `calc`, `Storage.pas:2103`),
//! dynamics (`DoDynaModel`/`InitStateVars`/`IntegrateStates` → `calc`/`init`/
//! `integrate`, `:2211`/`:2787`/`:2861`) and state-variable
//! (`num_vars`/`get_all_vars`/`get_variable`/`set_variable`/`get_var_name`,
//! `:3092-3322`) call sites through the ABI-doc record shuttle.
//!
//! Unlike the Generator (`TGeneratorVars` shuttle), the Storage interfaces take
//! `new(dynarec)` — no `GeneratorVars` pointer crosses (ABI doc §1); the
//! record shuttle is the 52-byte `TDynamicsRec` only. The transport is
//! sandboxed WebAssembly (`crates/dss-usermodel`); the contract is 1:1 with the
//! native DLL — same call ordering, same warn-and-fallback failure behavior
//! (plan §2.4 / ABI doc §5).

use std::sync::Arc;

use num_complex::Complex64;

use dss_usermodel::{
    Callbacks, DynamicsRec, Effect, HostConfig, InterfaceKind, NoCallbacks, Shuttle,
    UserModelError, UserModelHost, UserModelInstance,
};

use crate::diag::{DssDiagnostic, ErrorLog};
use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::base::{UserModelAction, UserModelLoad, UserModelSlot};
use crate::support::dynamics::IterationFlag;

use super::Storage;

/// One bound Storage user model — the dss-core wrapper around a
/// [`UserModelHost`] + [`UserModelInstance`] (Pascal `TStoreUserModel` /
/// `TStoreDynaModel`). Held on the [`Storage`] as
/// `Option<Box<StorageUserModelSlot>>` for the `UserModel=` (15-fn) and
/// `DynaDLL=` (13-fn) slots; the `kind` field records which interface it stands
/// in for. [`Clone`] clones the load *spec* and drops the live instance
/// (faithful to Pascal `MakeLike`, which re-`New`s — `Storage.pas:995-996`).
pub struct StorageUserModelSlot {
    /// The model attribution — the `.wasm` path exactly as written.
    model: String,
    /// The compiled module bytes (`Arc` so a `MakeLike` clone shares them).
    wasm: Arc<[u8]>,
    /// Which Pascal loader shape this slot stands in for
    /// (`StoreUserModel` = `UserModel=`, `StoreDynaModel` = `DynaDLL=`).
    kind: InterfaceKind,
    /// The terminal-array order the V/I shuttle buffers are sized to (`Yorder`).
    yorder: usize,
    /// The last `UserData=`/`DynaData=` edit string (re-applied after a lazy
    /// reload so a cloned element sees the same model state on first use).
    data: String,
    /// The live wasmi instance — `None` before the first (re)load and after a
    /// `clone`, then re-created lazily.
    live: Option<Box<UserModelInstance>>,
    /// Cached `num_vars()` (refreshed after `new`/`edit`/`update_model`).
    num_vars: usize,
    /// Cached 1-based variable names.
    var_names: Vec<String>,
}

impl Clone for StorageUserModelSlot {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            wasm: Arc::clone(&self.wasm),
            kind: self.kind,
            yorder: self.yorder,
            data: self.data.clone(),
            live: None, // re-created lazily (Pascal MakeLike re-News).
            num_vars: self.num_vars,
            var_names: self.var_names.clone(),
        }
    }
}

impl std::fmt::Debug for StorageUserModelSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageUserModelSlot")
            .field("model", &self.model)
            .field("kind", &self.kind)
            .field("exists", &self.live.is_some())
            .field("num_vars", &self.num_vars)
            .finish_non_exhaustive()
    }
}

/// Result alias for the slot's `&mut` call paths.
type LiveResult = Result<(), UserModelError>;

impl StorageUserModelSlot {
    /// Load + instantiate from resolved `.wasm` bytes (Pascal `Set_Name` →
    /// `FNew`, `StoreUserModel.pas:181-240` / `:303-360`).
    pub fn load(
        model: &str,
        wasm: &[u8],
        kind: InterfaceKind,
        yorder: usize,
        s: &Storage,
    ) -> Result<Self, UserModelError> {
        let host = UserModelHost::load(model, wasm, kind, HostConfig::default())?;
        let mut dr = dyn_rec_from(&super::super::generator::default_recalc_ctx());
        let ctx =
            StorageCallbacks::snapshot(s, &super::super::generator::default_recalc_ctx(), &[]);
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        let instance = UserModelInstance::new(&host, yorder.max(1), sh)?;
        let mut slot = Self {
            model: model.to_string(),
            wasm: Arc::from(wasm.to_vec()),
            kind,
            yorder: yorder.max(1),
            data: String::new(),
            live: Some(Box::new(instance)),
            num_vars: 0,
            var_names: Vec::new(),
        };
        slot.refresh_var_cache()?;
        Ok(slot)
    }

    /// Pascal `Get_Exists` (`FID <> 0`).
    pub fn exists(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.exists())
    }

    /// The model attribution.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Cached user-model variable count (Pascal `FNumVars`).
    pub fn num_vars(&self) -> usize {
        self.num_vars
    }

    /// Cached 1-based variable name (Pascal `FGetVarName`).
    pub fn var_name(&self, k: usize) -> Option<&str> {
        self.var_names.get(k.wrapping_sub(1)).map(String::as_str)
    }

    /// Ensure the live instance exists, re-creating it from the spec after a
    /// clone (lazy Pascal-`New`), then replay the last `UserData=`/`DynaData=`.
    fn ensure_live(&mut self, s: &Storage, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        if self.live.is_some() {
            return Ok(());
        }
        let host = UserModelHost::load(&self.model, &self.wasm, self.kind, HostConfig::default())?;
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        let instance = UserModelInstance::new(&host, self.yorder, sh)?;
        self.live = Some(Box::new(instance));
        if !self.data.is_empty() {
            self.edit_impl(&self.data.clone(), s, sys, node_v)?;
        }
        self.refresh_var_cache()?;
        Ok(())
    }

    /// Pascal `Edit` (`StoreUserModel.pas:175-179`): send the edit string to a
    /// loaded model (ignored while absent). Refreshes the variable surface.
    pub fn edit(
        &mut self,
        data: &str,
        s: &Storage,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.data = data.to_string();
        if self.live.is_none() {
            return Ok(());
        }
        self.edit_impl(data, s, sys, node_v)?;
        self.refresh_var_cache()?;
        Ok(())
    }

    fn edit_impl(
        &mut self,
        data: &str,
        s: &Storage,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
        let live = self.live.as_mut().expect("edit_impl requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.edit(data, sh)
    }

    /// Pascal `FUpdateModel` (`Storage.pas:1287`/`:1289`) — after
    /// `RecalcElementData`. Refreshes the cached variable surface.
    pub fn update_model(&mut self, s: &Storage, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        if self.live.is_none() {
            return Ok(());
        }
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
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

    /// Pascal `FCalc(V, I)` — power-flow / dynamics current computation. Writes
    /// `v` in, runs the guest, reads the terminal currents back into `i`. The
    /// Pascal sign convention stays at the caller.
    pub fn calc(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        s: &Storage,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.ensure_live(s, sys, node_v)?;
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
        let live = self.live.as_mut().expect("calc requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.calc(v, i, sh)
    }

    /// Pascal `FInit(V, I)` — dynamics `InitStateVars` seeding
    /// (`Storage.pas:2787`).
    pub fn init(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        s: &Storage,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.ensure_live(s, sys, node_v)?;
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
        let live = self.live.as_mut().expect("init requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.init(v, i, sh)
    }

    /// Pascal `Integrate` (`select(id)` + `integrate()`,
    /// `StoreUserModel.pas:164-168`/`:287-291`) — `IntegrateStates`
    /// (`Storage.pas:2861`).
    pub fn integrate(&mut self, s: &Storage, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        self.ensure_live(s, sys, node_v)?;
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
        let live = self.live.as_mut().expect("integrate requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.integrate(sh)
    }

    /// Pascal `FGetAllVars(@States[base])` (`Storage.pas:3199`/`:3204`).
    pub fn get_all_vars(
        &mut self,
        out: &mut [f64],
        s: &Storage,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
        let live = self
            .live
            .as_mut()
            .expect("get_all_vars requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.get_all_vars(out, sh)
    }

    /// Pascal `FGetVariable(k)` (1-based, `Storage.pas:3098`/`:3108`).
    pub fn get_variable(
        &mut self,
        k: usize,
        s: &Storage,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> Result<f64, UserModelError> {
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
        let live = self
            .live
            .as_mut()
            .expect("get_variable requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.get_variable(k as i32, sh)
    }

    /// Pascal `FSetVariable(k, value)` (1-based, `Storage.pas:3164`/`:3174`).
    pub fn set_variable(
        &mut self,
        k: usize,
        value: f64,
        s: &Storage,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut dr = dyn_rec_from(sys);
        let ctx = StorageCallbacks::snapshot(s, sys, node_v);
        let live = self
            .live
            .as_mut()
            .expect("set_variable requires a live model");
        let sh = Shuttle::without_gen_vars(&mut dr, Box::new(ctx));
        live.set_variable(k as i32, value, sh)
    }

    /// Drain the tier-B effects (MsgCallback text) queued by the last guest
    /// call into `errors` (Pascal `MsgCallBack` → `DoSimpleMsg`, errno 9000).
    pub fn drain_effects(&mut self, name: &str, errors: &mut ErrorLog) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        for eff in live.drain_effects() {
            match eff {
                Effect::Msg(text) => {
                    errors.push(DssDiagnostic::msg(text, Some(9000)));
                }
                Effect::ControlQueuePush { .. } => {
                    // No Storage user model schedules control actions
                    // (`ControlQueuePush` is the CapControl surface, WM.5).
                    errors.push(DssDiagnostic::msg(
                        format!(
                            "Storage.{name}: user model called ControlQueuePush, which is not \
                             wired for Storage elements (CapControl scope, WM.5)."
                        ),
                        Some(9001),
                    ));
                }
            }
        }
    }

    /// Refresh the cached `num_vars` + names from the live model (a read-only
    /// snapshot suffices — these calls never touch element state). A guest trap in
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

impl Storage {
    /// Queue a deferred `UserModel=`/`DynaDLL=` (re)load (Pascal
    /// `UserModel.Name := …` / `DynaModel.Name := …` → `Set_Name`). A blank /
    /// `none` name unloads the slot in place and is not queued.
    pub(super) fn queue_user_model_load(&mut self, slot: UserModelSlot, name: String) {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            self.unload_slot(slot);
            return;
        }
        self.pending_user_model_loads.push(UserModelLoad {
            slot,
            action: UserModelAction::Load(name),
        });
    }

    /// Queue a deferred `UserData=`/`DynaData=` edit (Pascal
    /// `if UserModel.Exists then UserModel.Edit(…)`). Always queued; the drain
    /// no-ops when the model does not exist (existence resolved at drain time).
    pub(super) fn queue_user_model_edit(&mut self, slot: UserModelSlot, data: String) {
        self.pending_user_model_loads.push(UserModelLoad {
            slot,
            action: UserModelAction::Edit(data),
        });
    }

    fn take_slot(&mut self, slot: UserModelSlot) -> Option<Box<StorageUserModelSlot>> {
        match slot {
            UserModelSlot::User => self.user_model.take(),
            UserModelSlot::Dyna => self.dyna_model.take(),
            UserModelSlot::Shaft => None, // Storage has no shaft model.
        }
    }

    fn put_slot(&mut self, slot: UserModelSlot, s: Box<StorageUserModelSlot>) {
        match slot {
            UserModelSlot::User => self.user_model = Some(s),
            UserModelSlot::Dyna => self.dyna_model = Some(s),
            UserModelSlot::Shaft => {}
        }
    }

    fn unload_slot(&mut self, slot: UserModelSlot) {
        match slot {
            UserModelSlot::User => self.user_model = None,
            UserModelSlot::Dyna => self.dyna_model = None,
            UserModelSlot::Shaft => {}
        }
    }

    /// The interface kind for a slot: `UserModel=` → 15-fn `TStoreUserModel`,
    /// `DynaDLL=` → 13-fn `TStoreDynaModel`.
    fn slot_kind(slot: UserModelSlot) -> InterfaceKind {
        match slot {
            UserModelSlot::Dyna => InterfaceKind::StoreDynaModel,
            _ => InterfaceKind::StoreUserModel,
        }
    }

    /// The executive-drained apply for a resolved [`UserModelLoad`] (§2.4
    /// activation rule). `wasm` is `Some` iff a `.wasm` file was found + read.
    pub(super) fn apply_user_model_load_impl(
        &mut self,
        load: &UserModelLoad,
        wasm: Option<&[u8]>,
        errors: &mut ErrorLog,
    ) {
        let name = self.cd.obj.name().to_string();
        let is_dyna = load.slot == UserModelSlot::Dyna;
        match &load.action {
            UserModelAction::Load(model_name) => {
                // Pascal `Set_Name` frees the previous model before (re)loading.
                self.unload_slot(load.slot);
                match wasm {
                    Some(bytes) => {
                        let yorder = self.cd.yorder;
                        let kind = Self::slot_kind(load.slot);
                        match StorageUserModelSlot::load(model_name, bytes, kind, yorder, self) {
                            Ok(slot) => self.put_slot(load.slot, Box::new(slot)),
                            Err(e) => push_load_failure(&name, model_name, is_dyna, &e, errors),
                        }
                    }
                    // Not a `.wasm` file (native-DLL name / missing file): the
                    // Pascal warn-and-fallback path (`Storage.pas:207`/`:329`,
                    // 1570), model absent, built-in model solves.
                    None => errors.push(DssDiagnostic::msg(
                        not_loaded_text(&name, model_name, is_dyna),
                        Some(1570),
                    )),
                }
            }
            UserModelAction::Edit(data) => {
                let Some(mut s) = self.take_slot(load.slot) else {
                    return; // Pascal: `if UserModel.Exists then Edit`.
                };
                let mut errs = ErrorLog::new();
                if let Err(e) = s.edit(
                    data,
                    self,
                    &super::super::generator::default_recalc_ctx(),
                    &[],
                ) {
                    errs.push(DssDiagnostic::msg(e.to_string(), Some(1569)));
                }
                s.drain_effects(&name, &mut errs);
                self.put_slot(load.slot, s);
                errors.extend(errs.into_vec());
            }
        }
    }

    /// Whether the `UserModel=` slot holds a loaded model (Pascal
    /// `UserModel.Exists` / `IsUserModel`).
    pub(super) fn user_model_exists(&self) -> bool {
        self.user_model.as_ref().is_some_and(|s| s.exists())
    }

    /// Whether the `DynaDLL=` slot holds a loaded model (Pascal
    /// `DynaModel.Exists`).
    pub(super) fn dyna_model_exists(&self) -> bool {
        self.dyna_model.as_ref().is_some_and(|s| s.exists())
    }

    /// Pascal `TStorageObj.DoUserModel` (`Storage.pas:2103-2122`): `UserModel.
    /// FCalc(Vterminal, Iterminal)`, `set_ITerminalUpdated(TRUE)`, then negate
    /// the terminal currents into `InjCurrent`. Returns `true` iff a model
    /// existed and ran; `false` → the caller records the #567 missing-model
    /// diagnostic. `Vterminal`/`InjCurrent` are computed by the caller
    /// (`CalcYPrimContribution` first, `:2108`).
    ///
    /// A wasm-only hard failure (trap/protocol fault) is surfaced as a hard
    /// engine error (ABI §6) — never a silent mid-run fallback.
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
                // Negate the user-model currents into InjCurrent (Pascal
                // `InjCurrent[i] -= Iterminal[i]`, `:2115-2116`).
                for i in 0..self.cd.nconds {
                    self.cd.inj_current[i] -= self.cd.iterminal[i];
                }
            }
            Err(e) => errors.push(DssDiagnostic::abort(
                format!("Storage.{name}: user model `calc` trapped/faulted: {e}"),
                Some(567),
            )),
        }
        um.drain_effects(&name, errors);
        self.user_model = Some(um);
        true
    }

    /// Pascal `TStorageObj.DoDynaModel` (`Storage.pas:2211-2234`): pass the node
    /// voltages to ground into `Vterminal`, call `DynaModel.FCalc(Vterminal,
    /// @DESSCurr)` into a host-owned current buffer, then
    /// `CalcYPrimContribution`/`ZeroITerminal` and stick `-DESSCurr` into
    /// `ITerminal` / `+DESSCurr` into `InjCurrent` per phase. Returns `true` iff
    /// a dynamics model existed and ran.
    pub(super) fn do_dyna_model(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut ErrorLog,
    ) -> bool {
        let Some(mut dm) = self.dyna_model.take() else {
            return false;
        };
        if !dm.exists() {
            self.dyna_model = Some(dm);
            return false;
        }
        let name = self.cd.obj.name().to_string();
        // Vterminal[i] := NodeV[NodeRef[i]] for i := 1..FNconds (`:2219-2220`).
        let yorder = self.cd.yorder;
        let mut v = vec![Complex64::ZERO; yorder];
        for (i, vi) in v.iter_mut().enumerate() {
            *vi = self
                .cd
                .node_ref
                .get(i)
                .and_then(|&nr| node_v.get(nr).copied())
                .unwrap_or(Complex64::ZERO);
        }
        // `StorageVars.w_grid := TwoPi * Frequency` (`:2221`) is engine-only for
        // the reference fixture (it reads its inputs from V + TDynamicsRec, not
        // via `get_public_data`), so no StorageVars image crosses the boundary.
        let mut dess_curr = vec![Complex64::ZERO; yorder];
        let ran = dm.calc(&v, &mut dess_curr, self, sys, node_v);
        match ran {
            Ok(()) => {
                // CalcYPrimContribution(InjCurrent) + ZeroITerminal (`:2225-2226`).
                self.calc_yprim_contribution(node_v);
                self.cd.zero_iterminal();
                let nphases = self.cd.nphases;
                for (i, &curr) in dess_curr.iter().enumerate().take(nphases) {
                    // StickCurrInTerminalArray(ITerminal, -DESSCurr[i], i); set updated;
                    // StickCurrInTerminalArray(InjCurrent, DESSCurr[i], i) (`:2228-2233`).
                    self.stick_curr(true, -curr, i);
                    self.cd.iterminal_updated = true;
                    self.cd.mark_iterminal_solved(sys.solution_count);
                    self.stick_curr(false, curr, i);
                }
            }
            Err(e) => errors.push(DssDiagnostic::abort(
                format!("Storage.{name}: dynamics model `calc` trapped/faulted: {e}"),
                Some(567),
            )),
        }
        dm.drain_effects(&name, errors);
        self.dyna_model = Some(dm);
        true
    }

    /// Pascal `RecalcElementData` tail (`Storage.pas:1286-1289`):
    /// `UserModel.FUpdateModel` then `DynaModel.FUpdateModel` on each existing
    /// model. Called at the end of [`Storage::recalc`].
    pub(super) fn update_user_models(&mut self, sys: &SysCtx) {
        let name = self.cd.obj.name().to_string();
        for slot in [UserModelSlot::User, UserModelSlot::Dyna] {
            let Some(mut s) = self.take_slot(slot) else {
                continue;
            };
            let mut errs = ErrorLog::new();
            if let Err(e) = s.update_model(self, sys, &[]) {
                errs.push(DssDiagnostic::msg(e.to_string(), Some(1569)));
            }
            s.drain_effects(&name, &mut errs);
            self.put_slot(slot, s);
            for d in errs.into_vec() {
                self.cd.obj.push_error(d);
            }
        }
    }

    /// Pascal `InitStateVars` DynaModel tail (`Storage.pas:2777-2789`):
    /// `DynaModel.FInit(Vterminal, Iterminal)` after `ComputeIterminal`/
    /// `ComputeVterminal`. No-op when the dynamics model is absent. Returns
    /// `true` iff the dynamics model ran (the caller then `Exit`s like Pascal).
    pub(super) fn dyna_model_finit(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> bool {
        if !self.dyna_model_exists() {
            return false;
        }
        self.compute_iterminal(sys, node_v);
        self.cd.compute_vterminal(node_v);
        let Some(mut dm) = self.dyna_model.take() else {
            return false;
        };
        let name = self.cd.obj.name().to_string();
        let v = self.cd.vterminal.clone();
        let mut it = self.cd.iterminal.clone();
        let mut errs = ErrorLog::new();
        if let Err(e) = dm.init(&v, &mut it, self, sys, node_v) {
            errs.push(DssDiagnostic::msg(
                format!("Storage.{name}: dynamics model `init` failed: {e}"),
                Some(1569),
            ));
        }
        dm.drain_effects(&name, &mut errs);
        self.dyna_model = Some(dm);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
        true
    }

    /// Pascal `IntegrateStates` DynaModel tail (`Storage.pas:2859-2863`):
    /// `DynaModel.Integrate()` (select+integrate). No-op when absent. Returns
    /// `true` iff the dynamics model ran (the caller then `Exit`s).
    pub(super) fn dyna_model_fintegrate(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> bool {
        if !self.dyna_model_exists() {
            return false;
        }
        let Some(mut dm) = self.dyna_model.take() else {
            return false;
        };
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = dm.integrate(self, sys, node_v) {
            errs.push(DssDiagnostic::msg(
                format!("Storage.{name}: dynamics model `integrate` failed: {e}"),
                Some(1569),
            ));
        }
        dm.drain_effects(&name, &mut errs);
        self.dyna_model = Some(dm);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
        true
    }

    /// The number of `UserModel=` variables (0 when absent) — Pascal `if
    /// UserModel.Exists then Result += UserModel.FNumVars` (`:3219-3220`).
    pub(super) fn num_user_model_variables(&self) -> usize {
        self.user_model
            .as_ref()
            .filter(|s| s.exists())
            .map_or(0, |s| s.num_vars())
    }

    /// The number of `DynaDLL=` variables (0 when absent).
    pub(super) fn num_dyna_model_variables(&self) -> usize {
        self.dyna_model
            .as_ref()
            .filter(|s| s.exists())
            .map_or(0, |s| s.num_vars())
    }

    /// Pascal `VariableName` user/dyna tail (`Storage.pas:3298-3321`): resolve a
    /// 1-based state-variable index above the classic `NumStorageVariables` base
    /// to the `UserModel` then the `DynaModel` (each using the relative index
    /// `i - base`, faithful to Pascal — the two branches share the same relative
    /// index, so with only one model bound it is unambiguous).
    pub(super) fn user_model_variable_name(&self, i: usize) -> Option<String> {
        let base = self.num_storage_variables();
        if i <= base {
            return None;
        }
        let k = i - base;
        if let Some(um) = self.user_model.as_ref().filter(|s| s.exists())
            && k <= um.num_vars()
        {
            return Some(um.var_name(k).unwrap_or_default().to_string());
        }
        if let Some(dm) = self.dyna_model.as_ref().filter(|s| s.exists())
            && k <= dm.num_vars()
        {
            return Some(dm.var_name(k).unwrap_or_default().to_string());
        }
        None
    }

    /// Pascal `Get_Variable` user/dyna tail (`Storage.pas:3092-3111`).
    pub(super) fn get_user_model_variable(
        &mut self,
        i: usize,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> Option<f64> {
        let base = self.num_storage_variables();
        if i <= base {
            return None;
        }
        let k = i - base;
        for slot in [UserModelSlot::User, UserModelSlot::Dyna] {
            let Some(mut s) = self.take_slot(slot) else {
                continue;
            };
            if s.exists() && k <= s.num_vars() {
                let name = self.cd.obj.name().to_string();
                let out = s.get_variable(k, self, sys, node_v);
                let mut errs = ErrorLog::new();
                let val = match out {
                    Ok(v) => Some(v),
                    Err(e) => {
                        errs.push(DssDiagnostic::msg(
                            format!("Storage.{name}: user model `get_variable` failed: {e}"),
                            Some(1569),
                        ));
                        Some(-9999.99)
                    }
                };
                s.drain_effects(&name, &mut errs);
                self.put_slot(slot, s);
                for d in errs.into_vec() {
                    self.cd.obj.push_error(d);
                }
                return val;
            }
            self.put_slot(slot, s);
        }
        None
    }

    /// Fill `out` with a user-model slot's variable values (Pascal
    /// `FGetAllVars(@States[NumStorageVariables])`, `:3199`/`:3204`).
    pub(super) fn get_all_vars_slot(
        &mut self,
        slot: UserModelSlot,
        out: &mut [f64],
        sys: &SysCtx,
        node_v: &[Complex64],
    ) {
        let Some(mut s) = self.take_slot(slot) else {
            return;
        };
        if !s.exists() {
            self.put_slot(slot, s);
            return;
        }
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = s.get_all_vars(out, self, sys, node_v) {
            errs.push(DssDiagnostic::msg(
                format!("Storage.{name}: user model `get_all_vars` failed: {e}"),
                Some(1569),
            ));
        }
        s.drain_effects(&name, &mut errs);
        self.put_slot(slot, s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    /// Pascal `Set_Variable` user/dyna tail (`Storage.pas:3158-3177`).
    /// Returns `true` iff a user/dyna model handled the write.
    pub(super) fn set_user_model_variable(&mut self, i: usize, value: f64) -> bool {
        let base = self.num_storage_variables();
        if i <= base {
            return false;
        }
        let k = i - base;
        let ctx = super::super::generator::default_recalc_ctx();
        for slot in [UserModelSlot::User, UserModelSlot::Dyna] {
            let Some(mut s) = self.take_slot(slot) else {
                continue;
            };
            if s.exists() && k <= s.num_vars() {
                let name = self.cd.obj.name().to_string();
                let mut errs = ErrorLog::new();
                if let Err(e) = s.set_variable(k, value, self, &ctx, &[]) {
                    errs.push(DssDiagnostic::msg(
                        format!("Storage.{name}: user model `set_variable` failed: {e}"),
                        Some(1569),
                    ));
                }
                s.drain_effects(&name, &mut errs);
                self.put_slot(slot, s);
                for d in errs.into_vec() {
                    self.cd.obj.push_error(d);
                }
                return true;
            }
            self.put_slot(slot, s);
        }
        false
    }
}

/// Map a load-time [`UserModelError`] onto the Pascal load-failure diagnostic: a
/// missing export → 1569 ("Does Not Have Required Function"), anything else →
/// 1570 ("… Not Loaded", warn-and-fallback). The model stays absent either way.
fn push_load_failure(
    name: &str,
    model_name: &str,
    is_dyna: bool,
    e: &UserModelError,
    errors: &mut ErrorLog,
) {
    match e {
        UserModelError::MissingExport { .. } | UserModelError::SignatureMismatch { .. } => {
            errors.push(DssDiagnostic::msg(e.to_string(), Some(1569)));
        }
        _ => errors.push(DssDiagnostic::msg(
            not_loaded_text(name, model_name, is_dyna),
            Some(1570),
        )),
    }
}

/// The Pascal warn-and-fallback text (`Storage.pas:207` for `UserModel=`,
/// `:329` for `DynaDLL=`), model absent, built-in model solves.
fn not_loaded_text(name: &str, model_name: &str, is_dyna: bool) -> String {
    if is_dyna {
        format!(
            "Storage User-written Dynamics Model \"{model_name}\" Not Loaded. Storage.{name} \
             falls back to its built-in model."
        )
    } else {
        format!(
            "Storage User Model {model_name} Not Loaded. Storage.{name} falls back to its \
             built-in model."
        )
    }
}

/// Build the packed `TDynamicsRec` image from the solution context (ABI doc
/// §2.1). `solution_mode` selects the model's power-flow-vs-dynamics `calc`
/// dispatch; `iteration_flag` is the predictor/corrector selector.
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
/// owning Storage (ABI doc §4). Owned because it is moved into the wasmi
/// `Store` and must be `'static`/`Send`.
struct StorageCallbacks {
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

impl StorageCallbacks {
    fn snapshot(s: &Storage, sys: &SysCtx, node_v: &[Complex64]) -> Self {
        let yorder = s.cd.yorder;
        let voltages: Vec<Complex64> = (0..yorder)
            .map(|i| {
                s.cd.node_ref
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
        StorageCallbacks {
            name: format!("Storage.{}", s.cd.obj.name()),
            voltages,
            currents: s.cd.iterminal.clone(),
            node_v: node_slice,
            dyn_rec: dyn_rec_from(sys),
            nterms: s.cd.nterms as i32,
            nconds: s.cd.nconds as i32,
            nphases: s.cd.nphases as i32,
            enabled: s.cd.enabled,
        }
    }
}

impl Callbacks for StorageCallbacks {
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
