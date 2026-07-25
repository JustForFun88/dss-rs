//! The dss-core side of the WASM user-model host for the Generator
//! (`UserModel=`/`ShaftModel=`, WASM_USERMODELS WM.3).
//!
//! Mirrors the Pascal `UserModel: TGenUserModel` / `ShaftModel: TGenUserModel`
//! fields (`PCElements/generator.pas:1015-1016`, `GenUserModel.pas`): a bound
//! model is (re)created by `Set_Name` (guest `new`), holds a live wasmi
//! instance ([`dss_usermodel::UserModelInstance`]), and services the power-flow
//! (`DoUserModel` → `calc`), dynamics (`init`/`integrate`/`calc`) and
//! state-variable (`num_vars`/`get_all_vars`/`get_variable`/`set_variable`/
//! `get_var_name`) call sites through the ABI-doc record shuttle.
//!
//! The transport is sandboxed WebAssembly (`crates/dss-usermodel`) rather than
//! a native DLL — permanently required by `#![forbid(unsafe_code)]` — but the
//! contract is 1:1: the same call ordering, the same `TGeneratorVars`/
//! `TDynamicsRec` record semantics (copy-in / copy-out per call), and the same
//! warn-and-fallback failure behavior (plan §2.4 / ABI doc §5).

use std::sync::Arc;

use num_complex::Complex64;

use dss_usermodel::{
    Callbacks, DynamicsRec, Effect, GeneratorVars, HostConfig, InterfaceKind, Shuttle,
    UserModelError, UserModelHost, UserModelInstance,
};

use crate::diag::{DssDiagnostic, ErrorLog};
use crate::elements::traits::SysCtx;
use crate::obj::base::{UserModelAction, UserModelLoad, UserModelSlot};
use crate::support::dynamics::IterationFlag;

use super::Generator;

/// One bound Generator user model — the dss-core wrapper around a
/// [`UserModelHost`] + [`UserModelInstance`] (Pascal `TGenUserModel`).
///
/// Held on the [`Generator`] as `Option<Box<GenUserModelSlot>>` for the
/// `UserModel=` and `ShaftModel=` slots. [`Clone`] is manual: it clones the
/// load *spec* (module bytes + attribution + the last edit string + the cached
/// variable surface) and drops the live wasmi instance — faithful to Pascal
/// `MakeLike`, which re-`New`s a fresh instance rather than copying live guest
/// state (`generator.pas:893-894`). The live instance is re-created lazily on
/// the next `&mut` call site (all of which carry the `yorder`/records needed to
/// instantiate).
pub struct GenUserModelSlot {
    /// The model attribution — the `.wasm` path exactly as written (the value
    /// of every error/diagnostic, ABI doc §6).
    model: String,
    /// The compiled module bytes (an `Arc` so a `clone`/`MakeLike` shares them
    /// without touching the filesystem).
    wasm: Arc<[u8]>,
    /// Which Pascal loader shape this slot stands in for.
    kind: InterfaceKind,
    /// The terminal-array order the V/I shuttle buffers are sized to (Pascal
    /// `Yorder`).
    yorder: usize,
    /// The last `UserData=`/`ShaftData=` edit string (re-applied after a lazy
    /// reload so a cloned element sees the same model state on first use).
    data: String,
    /// The live wasmi instance — `None` before the first (re)load and after a
    /// `clone`, then re-created lazily. Self-contained (its `Store` holds an
    /// `Engine`/`Module` handle), so the `UserModelHost` is not retained.
    live: Option<Box<UserModelInstance>>,
    /// Cached `num_vars()` (refreshed after `new`/`edit`/`update_model`) — the
    /// `&self` `num_variables()` / `variable_name()` accessors read it (they
    /// cannot drive the `&mut` guest).
    num_vars: usize,
    /// Cached 1-based variable names, refreshed alongside [`Self::num_vars`].
    var_names: Vec<String>,
}

impl Clone for GenUserModelSlot {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            wasm: Arc::clone(&self.wasm),
            kind: self.kind,
            yorder: self.yorder,
            data: self.data.clone(),
            // Drop the live instance — re-created lazily (Pascal MakeLike re-News).
            live: None,
            num_vars: self.num_vars,
            var_names: self.var_names.clone(),
        }
    }
}

impl std::fmt::Debug for GenUserModelSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenUserModelSlot")
            .field("model", &self.model)
            .field("kind", &self.kind)
            .field("exists", &self.live.is_some())
            .field("num_vars", &self.num_vars)
            .finish_non_exhaustive()
    }
}

impl GenUserModelSlot {
    /// Load + instantiate from resolved `.wasm` bytes (Pascal
    /// `TGenUserModel.Set_Name` → `FNew`, `GenUserModel.pas:159-201`).
    ///
    /// On success the model `exists()` and every call site runs it. On a
    /// [`UserModelError`] the caller maps it to the Pascal load-failure
    /// diagnostic (570/569) and leaves the slot absent (built-in fallback).
    pub fn load(
        model: &str,
        wasm: &[u8],
        yorder: usize,
        g: &mut Generator,
        sys: &SysCtx,
    ) -> Result<Self, UserModelError> {
        let host = UserModelHost::load(
            model,
            wasm,
            InterfaceKind::GenUserModel,
            HostConfig::default(),
        )?;
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, &[], &gv);
        let sh = Shuttle {
            gen_vars: Some(&mut gv),
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        let instance = UserModelInstance::new(&host, yorder.max(1), sh)?;
        // The guest `new` may mutate `TGeneratorVars` (IndMach012a's constructor
        // calls `Set_Slip`, which writes `GenData^.Speed` — `IndMach012Model.pas`
        // ctor / `:290`). The native DLL shares the element's live GenVars record,
        // so that `New`-time write persists onto the element (it is never reset —
        // `Speed := 0` is only in the generator ctor, `generator.pas:967`). Mirror
        // it: apply the read-back image (ABI doc §2, GenVars read-back is
        // unconditional). Without this, a Model=User snapshot reads Speed = 0
        // (Frequency = base) instead of the model's init-slip Speed.
        apply_gen_vars(g, &gv);
        let mut slot = Self {
            model: model.to_string(),
            wasm: Arc::from(wasm.to_vec()),
            kind: InterfaceKind::GenUserModel,
            yorder: yorder.max(1),
            data: String::new(),
            live: Some(Box::new(instance)),
            num_vars: 0,
            var_names: Vec::new(),
        };
        slot.refresh_var_cache();
        Ok(slot)
    }

    /// Pascal `Get_Exists` (`FID <> 0`, `GenUserModel.pas:113-121`): the model
    /// loaded and its instance was created. A slot with `live == None` and a
    /// spec (post-clone) reports `false` until the next `&mut` call re-creates
    /// the instance — matching Pascal, where a not-yet-`New`ed model does not
    /// exist.
    pub fn exists(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.exists())
    }

    /// The model attribution.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Cached user-model variable count (Pascal `UserModel.FNumVars`).
    pub fn num_vars(&self) -> usize {
        self.num_vars
    }

    /// Cached 1-based variable name (Pascal `UserModel.FGetVarName`).
    pub fn var_name(&self, k: usize) -> Option<&str> {
        self.var_names.get(k.wrapping_sub(1)).map(String::as_str)
    }

    /// Ensure the live instance exists, re-creating it from the spec after a
    /// clone (lazy Pascal-`New`), then run `edit(data)` to restore the last
    /// `UserData=` state.
    fn ensure_live(&mut self, g: &mut Generator, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        if self.live.is_some() {
            return Ok(());
        }
        let host = UserModelHost::load(&self.model, &self.wasm, self.kind, HostConfig::default())?;
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        let sh = Shuttle {
            gen_vars: Some(&mut gv),
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        let instance = UserModelInstance::new(&host, self.yorder, sh)?;
        self.live = Some(Box::new(instance));
        // Mirror the native shared-record contract: apply the guest `new`-time
        // GenVars mutation (e.g. `Speed`) back onto the element (see `load`).
        apply_gen_vars(g, &gv);
        if !self.data.is_empty() {
            self.edit_impl(&self.data.clone(), g, sys, node_v)?;
        }
        self.refresh_var_cache();
        Ok(())
    }

    /// Pascal `TGenUserModel.Edit` (`GenUserModel.pas:134-138`): send the
    /// `UserData=` string to a loaded model (ignored while absent). Refreshes
    /// the cached variable surface afterward.
    pub fn edit(
        &mut self,
        data: &str,
        g: &Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.data = data.to_string();
        if self.live.is_none() {
            // Nothing loaded yet — store the string; a later ensure_live replays it.
            return Ok(());
        }
        self.edit_impl(data, g, sys, node_v)?;
        self.refresh_var_cache();
        Ok(())
    }

    fn edit_impl(
        &mut self,
        data: &str,
        g: &Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        let live = self.live.as_mut().expect("edit_impl requires a live model");
        let sh = Shuttle {
            gen_vars: Some(&mut gv),
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        live.edit(data, sh)
    }

    /// Pascal `UserModel.FUpdateModel` (`generator.pas:1305`) — after
    /// `RecalcElementData`. Refreshes the cached variable surface.
    pub fn update_model(
        &mut self,
        g: &Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        if self.live.is_none() {
            return Ok(());
        }
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        {
            let live = self
                .live
                .as_mut()
                .expect("update_model requires a live model");
            let sh = Shuttle {
                gen_vars: Some(&mut gv),
                dyn_rec: &mut dr,
                ctx: Box::new(ctx),
            };
            live.update_model(sh)?;
        }
        self.refresh_var_cache();
        Ok(())
    }

    /// Pascal `UserModel.FCalc(Vterminal, Iterminal)` — the power-flow /
    /// dynamics current computation. Writes `v` in, runs the guest, reads the
    /// terminal currents back into `i`, and applies the mutated `TGeneratorVars`
    /// (e.g. `Pshaft`) back onto `g`. The Pascal sign convention (negate into
    /// `InjCurrent`) stays at the caller.
    pub fn calc(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        g: &mut Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.ensure_live(g, sys, node_v)?;
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        {
            let live = self.live.as_mut().expect("calc requires a live model");
            let sh = Shuttle {
                gen_vars: Some(&mut gv),
                dyn_rec: &mut dr,
                ctx: Box::new(ctx),
            };
            live.calc(v, i, sh)?;
        }
        apply_gen_vars(g, &gv);
        Ok(())
    }

    /// Pascal `UserModel.FInit(Vterminal, Iterminal)` — dynamics
    /// `InitStateVars` seeding (`generator.pas:2449/2451`).
    pub fn init(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        g: &mut Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.ensure_live(g, sys, node_v)?;
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        {
            let live = self.live.as_mut().expect("init requires a live model");
            let sh = Shuttle {
                gen_vars: Some(&mut gv),
                dyn_rec: &mut dr,
                ctx: Box::new(ctx),
            };
            live.init(v, i, sh)?;
        }
        apply_gen_vars(g, &gv);
        Ok(())
    }

    /// Pascal `UserModel.Integrate` (`select(id)` + `integrate()`,
    /// `GenUserModel.pas:123-127`) — dynamics `IntegrateStates`
    /// (`generator.pas:2534/2536`).
    pub fn integrate(
        &mut self,
        g: &mut Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.ensure_live(g, sys, node_v)?;
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        {
            let live = self.live.as_mut().expect("integrate requires a live model");
            let sh = Shuttle {
                gen_vars: Some(&mut gv),
                dyn_rec: &mut dr,
                ctx: Box::new(ctx),
            };
            live.integrate(sh)?;
        }
        apply_gen_vars(g, &gv);
        Ok(())
    }

    /// Pascal `UserModel.FGetAllVars(@States[base])` — write the model's
    /// `num_vars` values into `out` (`generator.pas:2704/2709`).
    pub fn get_all_vars(
        &mut self,
        out: &mut [f64],
        g: &Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        // A const-borrow variant of ensure_live is not needed: get_all_vars is
        // only reached after the model exists (num_variables() gates it).
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        let live = self
            .live
            .as_mut()
            .expect("get_all_vars requires a live model");
        let sh = Shuttle {
            gen_vars: Some(&mut gv),
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        live.get_all_vars(out, sh)
    }

    /// Pascal `UserModel.FGetVariable(k)` (1-based, `generator.pas:2618`).
    pub fn get_variable(
        &mut self,
        k: usize,
        g: &Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> Result<f64, UserModelError> {
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        let live = self
            .live
            .as_mut()
            .expect("get_variable requires a live model");
        let sh = Shuttle {
            gen_vars: Some(&mut gv),
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        live.get_variable(k as i32, sh)
    }

    /// Pascal `UserModel.FSetVariable(k, value)` (1-based, `generator.pas:2671`).
    pub fn set_variable(
        &mut self,
        k: usize,
        value: f64,
        g: &Generator,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut gv = gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = GenCallbacks::snapshot(g, sys, node_v, &gv);
        let live = self
            .live
            .as_mut()
            .expect("set_variable requires a live model");
        let sh = Shuttle {
            gen_vars: Some(&mut gv),
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        live.set_variable(k as i32, value, sh)
    }

    /// Drain the tier-B effects (MsgCallback text) queued by the last guest
    /// call and route them into `errors` (Pascal `MsgCallBack` →
    /// `DoSimpleMsg`, errno 9000). Never a silent drop (plan §2.9-5).
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
                    // No Generator user model schedules control actions
                    // (`ControlQueuePush` is the CapControl surface, WM.5). Surface
                    // it loudly rather than silently drop (plan §2.9-5).
                    errors.push(DssDiagnostic::msg(
                        format!(
                            "Generator.{name}: user model called ControlQueuePush, which is not \
                             wired for Generator elements (CapControl scope, WM.5)."
                        ),
                        Some(9001),
                    ));
                }
            }
        }
    }

    /// Refresh the cached `num_vars` + variable names from the live model
    /// (Pascal re-queries `FNumVars`/`FGetVarName` on demand; we cache so the
    /// `&self` accessors need no `&mut` guest access).
    fn refresh_var_cache(&mut self) {
        let Some(live) = self.live.as_mut() else {
            return;
        };
        // A read-only snapshot suffices — these calls never touch element state.
        let mut gv = GeneratorVars::default();
        let mut dr = DynamicsRec::default();
        let n = {
            let sh = Shuttle {
                gen_vars: Some(&mut gv),
                dyn_rec: &mut dr,
                ctx: Box::new(dss_usermodel::NoCallbacks),
            };
            live.num_vars(sh).unwrap_or(0).max(0) as usize
        };
        let mut names = Vec::with_capacity(n);
        for k in 1..=n {
            let mut gv2 = GeneratorVars::default();
            let mut dr2 = DynamicsRec::default();
            let sh = Shuttle {
                gen_vars: Some(&mut gv2),
                dyn_rec: &mut dr2,
                ctx: Box::new(dss_usermodel::NoCallbacks),
            };
            let name = live.get_var_name(k as i32, sh).unwrap_or_default();
            names.push(name);
        }
        self.num_vars = n;
        self.var_names = names;
    }
}

/// Result alias for the slot's `&mut` call paths.
type LiveResult = Result<(), UserModelError>;

impl Generator {
    /// Queue a deferred `UserModel=`/`ShaftModel=` (re)load (Pascal
    /// `UserModel.Name := …` → `Set_Name`). A blank / `none` name unloads the
    /// slot in place and is not queued (Pascal `Set_Name` `Exit`).
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

    /// Queue a deferred `UserData=`/`ShaftData=` edit (Pascal
    /// `if UserModel.Exists then UserModel.Edit(…)`). Always queued; the drain
    /// no-ops when the model does not exist (existence resolved at drain time).
    pub(super) fn queue_user_model_edit(&mut self, slot: UserModelSlot, data: String) {
        self.pending_user_model_loads.push(UserModelLoad {
            slot,
            action: UserModelAction::Edit(data),
        });
    }

    fn take_slot(&mut self, slot: UserModelSlot) -> Option<Box<GenUserModelSlot>> {
        match slot {
            UserModelSlot::User => self.user_model.take(),
            UserModelSlot::Shaft => self.shaft_model.take(),
            // The Generator has no Dyna slot (Storage `DynaDLL=` only, WM.4).
            UserModelSlot::Dyna => None,
        }
    }

    fn put_slot(&mut self, slot: UserModelSlot, s: Box<GenUserModelSlot>) {
        match slot {
            UserModelSlot::User => self.user_model = Some(s),
            UserModelSlot::Shaft => self.shaft_model = Some(s),
            UserModelSlot::Dyna => {}
        }
    }

    fn unload_slot(&mut self, slot: UserModelSlot) {
        match slot {
            UserModelSlot::User => self.user_model = None,
            UserModelSlot::Shaft => self.shaft_model = None,
            UserModelSlot::Dyna => {}
        }
    }

    /// The executive-drained apply for a resolved [`UserModelLoad`] (§2.4
    /// activation rule). `wasm` is `Some` iff a `.wasm` file was found + read.
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
                // Pascal `Set_Name` frees the previous model before (re)loading.
                self.unload_slot(load.slot);
                match wasm {
                    Some(bytes) => {
                        let yorder = self.cd.yorder;
                        match GenUserModelSlot::load(model_name, bytes, yorder, self, sys) {
                            Ok(slot) => self.put_slot(load.slot, Box::new(slot)),
                            Err(e) => push_load_failure(&name, model_name, &e, errors),
                        }
                    }
                    // Not a `.wasm` file (native-DLL name / missing file): the
                    // Pascal load-failure path — warn non-fatally + fall back.
                    None => errors.push(DssDiagnostic::msg(
                        format!(
                            "Generator User Model {model_name} Not Loaded. Generator.{name} \
                             falls back to its built-in model."
                        ),
                        Some(570),
                    )),
                }
            }
            UserModelAction::Edit(data) => {
                let Some(mut s) = self.take_slot(load.slot) else {
                    return; // Pascal: `if UserModel.Exists then Edit`.
                };
                let mut errs = ErrorLog::new();
                if let Err(e) = s.edit(data, self, sys, &[]) {
                    errs.push(DssDiagnostic::msg(e.to_string(), Some(569)));
                }
                s.drain_effects(&name, &mut errs);
                self.put_slot(load.slot, s);
                errors.extend(errs.into_vec());
            }
        }
    }

    /// Whether the `UserModel=` slot holds a loaded model (Pascal
    /// `UserModel.Exists`).
    pub(super) fn user_model_exists(&self) -> bool {
        self.user_model.as_ref().is_some_and(|s| s.exists())
    }

    /// Whether the `ShaftModel=` slot holds a loaded model (Pascal
    /// `ShaftModel.Exists`).
    pub(super) fn shaft_model_exists(&self) -> bool {
        self.shaft_model.as_ref().is_some_and(|s| s.exists())
    }

    /// Pascal `UserModel.FCalc(Vterminal, Iterminal)` at the power-flow
    /// (`DoUserModel`, `generator.pas:1826`) / dynamics (`DoDynamicMode`,
    /// `:1939`) call site: write `Vterminal` in, read the terminal currents back
    /// into `self.cd.iterminal`. Returns `true` iff a model existed and ran (the
    /// caller then negates `Iterminal` into `InjCurrent`); `false` → the caller
    /// records the missing-model diagnostic and falls back to Yprim only.
    ///
    /// A runtime trap/protocol fault is surfaced (ABI §6 hard error) into
    /// `errors` — never a silent fallback.
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
            Ok(()) => self.cd.iterminal.copy_from_slice(&it),
            // A wasm-only hard failure (trap / protocol fault / fuel / memory cap)
            // has no Pascal analogue: ABI §6 makes it a HARD, loud engine error, not
            // a silent mid-run fallback (which would silently change numerics). Flag
            // `abort` so the inject-path caller lifts `SolutionAbort` rather than
            // converging on the stale terminal current.
            Err(e) => errors.push(DssDiagnostic::abort(
                format!("Generator.{name}: user model `calc` trapped/faulted: {e}"),
                Some(567),
            )),
        }
        um.drain_effects(&name, errors);
        self.user_model = Some(um);
        true
    }

    /// Pascal `ShaftModel.FCalc(Vterminal, Iterminal)` — "Returns pshaft at
    /// least" (`generator.pas:2038`). The shaft model's primary product is the
    /// mutated `TGeneratorVars.Pshaft` (applied back onto the element), but
    /// Pascal passes the element's *live* `Iterminal` and the shaft model
    /// OVERWRITES it (last write in `DoDynamicMode`). Since `DoDynamicMode` then
    /// stamps `IterminalUpdated`/`IterminalSolutionCount`, `IntegrateStates`'
    /// `ComputeIterminal` reuses that cached value — so `TracePower =
    /// TerminalPowerIn(Vterminal, Iterminal)` reads the SHAFT model's currents,
    /// not the user model's. Faithfully write the shaft currents back so the
    /// shaft-dynamics `dSpeed` matches the oracle (WM.3 D2 port-bug fix).
    pub(super) fn shaft_model_fcalc(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut ErrorLog,
    ) {
        let Some(mut sm) = self.shaft_model.take() else {
            return;
        };
        if !sm.exists() {
            self.shaft_model = Some(sm);
            return;
        }
        let name = self.cd.obj.name().to_string();
        let v = self.cd.vterminal.clone();
        let mut it = self.cd.iterminal.clone();
        match sm.calc(&v, &mut it, self, sys, node_v) {
            // Pascal leaves the shaft model's currents in the element's Iterminal.
            Ok(()) => self.cd.iterminal.copy_from_slice(&it),
            // ABI §6 wasm hard failure — abort, never a silent mid-run fallback.
            Err(e) => errors.push(DssDiagnostic::abort(
                format!("Generator.{name}: shaft model `calc` trapped/faulted: {e}"),
                Some(567),
            )),
        }
        sm.drain_effects(&name, errors);
        self.shaft_model = Some(sm);
    }

    /// Pascal `RecalcElementData` tail (`generator.pas:1305-1307`):
    /// `UserModel.FUpdateModel` / `ShaftModel.FUpdateModel` on each existing
    /// model. Called at the end of [`Generator::recalc`].
    pub(super) fn update_user_models(&mut self, sys: &SysCtx) {
        let name = self.cd.obj.name().to_string();
        for slot in [UserModelSlot::User, UserModelSlot::Shaft] {
            let Some(mut s) = self.take_slot(slot) else {
                continue;
            };
            let mut errs = ErrorLog::new();
            if let Err(e) = s.update_model(self, sys, &[]) {
                errs.push(DssDiagnostic::msg(e.to_string(), Some(569)));
            }
            s.drain_effects(&name, &mut errs);
            self.put_slot(slot, s);
            for d in errs.into_vec() {
                self.cd.obj.push_error(d);
            }
        }
    }

    /// Pascal `InitStateVars` GenModel=6 tail (`generator.pas:2446-2452`):
    /// `UserModel.FInit(Vterminal, Iterminal)` then
    /// `ShaftModel.FInit(Vterminal, Iterminal)`, each seeding the model from the
    /// terminal V/I *left in the buffers* by the preceding power-flow solve.
    ///
    /// `Vterminal` is deliberately NOT recomputed here: Pascal `InitStateVars`
    /// runs only `ComputeIterminal` (`generator.pas:2393`) — never
    /// `ComputeVterminal` — before `FInit`, so the user model is seeded from the
    /// STALE `Vterminal` buffer, i.e. the node voltage of the power-flow's
    /// *last injection iteration* (`V_{n-1}`, one network re-solve behind the
    /// converged `NodeV`), not the final `V_n`. That pre-final voltage is the
    /// h-independent "projection" onto the dynamic operating point: seeding from
    /// it gives `E1 = V_{n-1} - I·Zsp`, which differs from `V_n - I·Zsp` by the
    /// last-iteration voltage step, and this is what drives the machine to
    /// `|Is1|=189.207` (not the power-flow `189.10`) at the first dynamics step.
    /// Refreshing `Vterminal` to `V_n` here (the earlier port) left the machine at
    /// the power-flow point and was the WM.3 D2 sub-bug #2 divergence
    /// (STATUS §"D2 sub-bug #2 — FIXED"). The built-in-shaft `Edp` path
    /// (`generator.pas:2409-2413`) reads a *fresh local* `Vabc := NodeV[NodeRef]`
    /// and is unaffected — only the Model=6 user-model seed uses the buffer.
    /// `Iterminal` was refreshed by `init_state_vars`' `ComputeIterminal`.
    ///
    /// TODO(compat): this deliberately reproduces an upstream inconsistency —
    /// `FInit` receives a mixed-generation pair (fresh `Iterminal` computed at
    /// the converged `V_n`, stale `Vterminal` = `V_{n-1}`), while upstream's own
    /// built-in model seeds from the fresh `NodeV` (`:2409-2413`). Deterministic
    /// and defined in BOTH oracle channels (0.14.5 == r4133 to ≤1e-13, the D2
    /// three-way experiment), pinned by the `wasm_gen_dyn` numeric golden. The
    /// clean fix — a self-consistent `(V_n, I(V_n))` seed — shifts the initial
    /// state by less than the power-flow convergence tolerance (~1e-4 pu) but
    /// breaks bit-parity with both oracles; decide at DE_PASCALIZE Stage F
    /// (default-lane candidate, parity lane keeps the stale seed).
    pub(super) fn user_model_finit(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        if !(self.user_model_exists() || self.shaft_model_exists()) {
            return;
        }
        let mut errs = ErrorLog::new();
        self.finit_slot(UserModelSlot::User, sys, node_v, &mut errs);
        self.finit_slot(UserModelSlot::Shaft, sys, node_v, &mut errs);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    fn finit_slot(
        &mut self,
        slot: UserModelSlot,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut ErrorLog,
    ) {
        let Some(mut s) = self.take_slot(slot) else {
            return;
        };
        if !s.exists() {
            self.put_slot(slot, s);
            return;
        }
        let name = self.cd.obj.name().to_string();
        let v = self.cd.vterminal.clone();
        let mut it = self.cd.iterminal.clone();
        match s.init(&v, &mut it, self, sys, node_v) {
            // Iterminal shared live: user FInit's output feeds the shaft FInit.
            Ok(()) => self.cd.iterminal.copy_from_slice(&it),
            Err(e) => errors.push(DssDiagnostic::msg(
                format!("Generator.{name}: user model `init` failed: {e}"),
                Some(567),
            )),
        }
        s.drain_effects(&name, errors);
        self.put_slot(slot, s);
    }

    /// Pascal `IntegrateStates` GenModel=6 tail (`generator.pas:2531-2537`):
    /// `UserModel.Integrate()` then `ShaftModel.Integrate()` (each is
    /// `select(id)` + `integrate()`). No-op when neither model exists.
    pub(super) fn user_model_fintegrate(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        if !(self.user_model_exists() || self.shaft_model_exists()) {
            return;
        }
        let mut errs = ErrorLog::new();
        self.fintegrate_slot(UserModelSlot::User, sys, node_v, &mut errs);
        self.fintegrate_slot(UserModelSlot::Shaft, sys, node_v, &mut errs);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    fn fintegrate_slot(
        &mut self,
        slot: UserModelSlot,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut ErrorLog,
    ) {
        let Some(mut s) = self.take_slot(slot) else {
            return;
        };
        if !s.exists() {
            self.put_slot(slot, s);
            return;
        }
        let name = self.cd.obj.name().to_string();
        if let Err(e) = s.integrate(self, sys, node_v) {
            errors.push(DssDiagnostic::msg(
                format!("Generator.{name}: user model `integrate` failed: {e}"),
                Some(567),
            ));
        }
        s.drain_effects(&name, errors);
        self.put_slot(slot, s);
    }

    /// Fill `out` with a user-model slot's variable values (Pascal
    /// `UserModel.FGetAllVars(@States[base])`, `generator.pas:2704/2709`).
    /// Effects/errors are routed onto the element (surfaced in `errors()`).
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
                format!("Generator.{name}: user model `get_all_vars` failed: {e}"),
                Some(567),
            ));
        }
        s.drain_effects(&name, &mut errs);
        self.put_slot(slot, s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    /// Pascal `Set_Variable` user/shaft tail (`generator.pas:2665-2686`): route
    /// a 1-based state-variable write to the user model (`k = i -
    /// NumGenVariables`, if `k <= N`) or the shaft model (`k = i -
    /// (NumGenVariables + N)`), matching the classic index arithmetic.
    pub(super) fn set_user_model_variable(&mut self, i: usize, value: f64, sys: &SysCtx) {
        let base = self.num_gen_variables();
        let un = if self.user_model_exists() {
            self.user_model.as_ref().map_or(0, |s| s.num_vars())
        } else {
            0
        };
        let (slot, k) = if i > base && i <= base + un {
            (UserModelSlot::User, i - base)
        } else if self.shaft_model_exists() && i > base + un {
            (UserModelSlot::Shaft, i - (base + un))
        } else {
            return;
        };
        let Some(mut s) = self.take_slot(slot) else {
            return;
        };
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = s.set_variable(k, value, self, sys, &[]) {
            errs.push(DssDiagnostic::msg(
                format!("Generator.{name}: user model `set_variable` failed: {e}"),
                Some(567),
            ));
        }
        s.drain_effects(&name, &mut errs);
        self.put_slot(slot, s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }
}

/// Map a load-time [`UserModelError`] onto the Pascal load-failure diagnostic:
/// a missing export → 569 ("Does Not Have Required Function"), anything else →
/// 570 ("… Not Loaded", warn-and-fallback). The model stays absent either way
/// (the generator solves on its built-in model — plan §2.4).
fn push_load_failure(name: &str, model_name: &str, e: &UserModelError, errors: &mut ErrorLog) {
    match e {
        UserModelError::MissingExport { .. } | UserModelError::SignatureMismatch { .. } => {
            errors.push(DssDiagnostic::msg(e.to_string(), Some(569)));
        }
        _ => errors.push(DssDiagnostic::msg(
            format!(
                "Generator User Model {model_name} Not Loaded ({e}). Generator.{name} falls back \
                 to its built-in model."
            ),
            Some(570),
        )),
    }
}

/// Build the packed `TGeneratorVars` image from the Generator's flattened
/// GenVars fields (ABI doc §2.2b — the 244-byte wasm marshaled image; the
/// engine-only `deltaQNom` never crosses).
pub(super) fn gen_vars_from(g: &Generator) -> GeneratorVars {
    GeneratorVars {
        theta: g.theta,
        pshaft: g.p_shaft,
        speed: g.speed,
        w0: g.w0,
        hmass: g.h_mass,
        mmass: g.m_mass,
        d: g.d_damping,
        dpu: g.dpu,
        kva_rating: g.kva_rating,
        kv_generator_base: g.kv_generator_base,
        xd: g.xd,
        xdp: g.xdp,
        xdpp: g.xdpp,
        pu_xd: g.pu_xd,
        pu_xdp: g.pu_xdp,
        pu_xdpp: g.pu_xdpp,
        dtheta: g.dtheta,
        dspeed: g.dspeed,
        theta_history: g.theta_history,
        speed_history: g.speed_history,
        pnominalperphase: g.p_nominal_per_phase,
        qnominalperphase: g.q_nominal_per_phase,
        num_phases: g.cd.nphases as i32,
        num_conductors: g.cd.nconds as i32,
        conn: g.connection as i32,
        vthev_mag: g.v_thev_mag,
        vthev_harm: g.v_thev_harm,
        theta_harm: g.theta_harm,
        vtarget: g.v_target,
        zthev: (g.zthev.re, g.zthev.im),
        xrdp: g.xrdp,
    }
}

/// Apply the mutated `TGeneratorVars` back onto the Generator after a guest
/// call. ABI doc §2 freezes the read-back as **unconditional** — a native DLL
/// shares the record live (retained pointer), so *any* field the model writes
/// persists; the wasm host reproduces that by copying the full image back, not a
/// state-only subset (a whitelist would silently drop a legitimate mutation of,
/// e.g., `Pnominalperphase`/`Mmass`/`kVArating`, diverging from the native
/// contract — WM.3 audit). Every mapped f64/Complex field of the 244-byte image
/// (`gen_vars_from`) is written back. f64 round-trips are bit-exact, so a field
/// the model did not touch is a no-op; the reference fixture writes only `Speed`.
///
/// The structural ints (`num_phases`/`num_conductors`/`conn`) are the ONE
/// exception: they stay element-owned and are never read back — they define the
/// terminal/YPrim shape the host allocated the buffers and node map against, so a
/// guest write to them cannot be honored mid-solve without corrupting the element
/// (a native model that mutated them would equally corrupt the engine; no model
/// does). The `deltaQNom` NCIM slot never crosses the wasm boundary at all (§2.2b).
pub(super) fn apply_gen_vars(g: &mut Generator, gv: &GeneratorVars) {
    g.theta = gv.theta;
    g.p_shaft = gv.pshaft;
    g.speed = gv.speed;
    g.w0 = gv.w0;
    g.h_mass = gv.hmass;
    g.m_mass = gv.mmass;
    g.d_damping = gv.d;
    g.dpu = gv.dpu;
    g.kva_rating = gv.kva_rating;
    g.kv_generator_base = gv.kv_generator_base;
    g.xd = gv.xd;
    g.xdp = gv.xdp;
    g.xdpp = gv.xdpp;
    g.pu_xd = gv.pu_xd;
    g.pu_xdp = gv.pu_xdp;
    g.pu_xdpp = gv.pu_xdpp;
    g.dtheta = gv.dtheta;
    g.dspeed = gv.dspeed;
    g.theta_history = gv.theta_history;
    g.speed_history = gv.speed_history;
    g.p_nominal_per_phase = gv.pnominalperphase;
    g.q_nominal_per_phase = gv.qnominalperphase;
    g.v_thev_mag = gv.vthev_mag;
    g.v_thev_harm = gv.vthev_harm;
    g.theta_harm = gv.theta_harm;
    g.v_target = gv.vtarget;
    g.zthev = num_complex::Complex64::new(gv.zthev.0, gv.zthev.1);
    g.xrdp = gv.xrdp;
}

/// Build the packed `TDynamicsRec` image from the solution context (ABI doc
/// §2.1). `solution_mode` selects the model's power-flow-vs-dynamics `Calc`
/// dispatch (`DynaData^.SolutionMode`, DYNAMICMODE = 14 = [`SysCtx::mode`]
/// ordinal for `Dynamic`); `iteration_flag` is the predictor/corrector selector.
pub(super) fn dyn_rec_from(sys: &SysCtx) -> DynamicsRec {
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
/// owning Generator (ABI doc §4). Owned (not borrowed) because it is moved into
/// the wasmi `Store` and must be `'static`/`Send`. The Pascal callbacks bind to
/// the global `DSSPrime`; dss-rs is single-context, so binding to the owning
/// element is observationally identical (ABI doc §4, documented decision).
struct GenCallbacks {
    name: String,
    voltages: Vec<Complex64>,
    currents: Vec<Complex64>,
    node_v: Vec<Complex64>,
    public_data: Vec<u8>,
    dyn_rec: DynamicsRec,
    nterms: i32,
    nconds: i32,
    nphases: i32,
    enabled: bool,
}

impl GenCallbacks {
    /// Snapshot the owning Generator's tier-A state.
    fn snapshot(g: &Generator, sys: &SysCtx, node_v: &[Complex64], gv: &GeneratorVars) -> Self {
        let yorder = g.cd.yorder;
        // Terminal voltages `NodeV[NodeRef[i]]` (Pascal
        // `GetActiveElementVoltagesCallBack`).
        let voltages: Vec<Complex64> = (0..yorder)
            .map(|i| {
                g.cd.node_ref
                    .get(i)
                    .and_then(|&nr| node_v.get(nr).copied())
                    .unwrap_or(Complex64::ZERO)
            })
            .collect();
        // System node voltages `NodeV[1..NumNodes]`, ground (index 0) excluded
        // (ABI doc §4 row 17).
        let node_slice = if node_v.len() > 1 {
            node_v[1..].to_vec()
        } else {
            Vec::new()
        };
        GenCallbacks {
            name: format!("Generator.{}", g.cd.obj.name()),
            voltages,
            currents: g.cd.iterminal.clone(),
            node_v: node_slice,
            public_data: gv.to_bytes().to_vec(),
            dyn_rec: dyn_rec_from(sys),
            nterms: g.cd.nterms as i32,
            nconds: g.cd.nconds as i32,
            nphases: g.cd.nphases as i32,
            enabled: g.cd.enabled,
        }
    }
}

impl Callbacks for GenCallbacks {
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
    fn public_data(&self) -> &[u8] {
        &self.public_data
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
