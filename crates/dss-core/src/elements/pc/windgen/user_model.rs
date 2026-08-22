//! The dss-core side of the WASM user-model host for the WindGen
//! (`UserModel=`/`UserData=`, r4133 `WindGen.pas:391-395`).
//!
//! Mirrors the Pascal `UserModel: TWindGenUserModel` field (`WindGen.pas:100`,
//! created at `:995` with `@WindGenVars`, `WindGenUserModel.pas`): a bound model
//! is (re)created by `Set_Name` (guest `new`), holds a live wasmi instance
//! ([`dss_usermodel::UserModelInstance`]), and services the power-flow
//! (`DoUserModel` → `calc`, `:1875-1898`), dynamics (`InitStateVars` `:2568`,
//! `DoDynamicMode` `:1991-1998`, `IntegrateStates` `:2663`) and state-variable
//! (`num_vars`/`get_all_vars`/`get_variable`/`set_variable`/`get_var_name`,
//! `:2735-2876`) call sites through the ABI-doc record shuttle.
//!
//! Two things separate this from the Generator slot
//! (`generator/user_model.rs`, the WM.3 pattern this follows):
//!
//! 1. **The boundary record is `TWindGenVars`, not `TGeneratorVars`** — a
//!    348-byte wasm image with the turbine tail (`ag`…`s`) appended and the
//!    managed `PLoss: string` dropped (ABI doc §2.6). The transport is shared:
//!    [`WindGenShuttle`] is accepted wherever the 15-function call surface takes
//!    a shuttle, and a `TGeneratorVars` shuttle is *refused* at the seam.
//! 2. **There is no shaft slot.** r4133 carries a second
//!    `ShaftModel: TWindGenUserModel` field and `MakeLike`s its name (`:830`),
//!    but `DefineProperties` registers no property for it, so
//!    `ShaftModel.Exists` is always false and all of `:2009-2012`, `:2569`,
//!    `:2664`, `:2746-2750`, `:2786-2789` and `:2872-2878` is unreachable
//!    upstream.
//!
//! The transport is sandboxed WebAssembly (`crates/dss-usermodel`) rather than a
//! native DLL — permanently required by `#![forbid(unsafe_code)]` — but the
//! contract is 1:1: the same call ordering, the same record semantics (copy-in /
//! copy-out per call) and the same warn-and-fallback load failure (ABI doc §5).

use std::sync::Arc;

use num_complex::Complex64;

use dss_usermodel::{
    Callbacks, DynamicsRec, Effect, HostConfig, InterfaceKind, UserModelError, UserModelHost,
    UserModelInstance, WindGenShuttle, WindGenVars,
};

use crate::diag::{DssDiagnostic, ErrorLog};
use crate::elements::traits::SysCtx;
use crate::obj::base::{UserModelAction, UserModelLoad, UserModelSlot};
use crate::support::dynamics::IterationFlag;

use super::WindGen;

/// One bound WindGen user model — the dss-core wrapper around a
/// [`UserModelHost`] + [`UserModelInstance`] (Pascal `TWindGenUserModel`).
///
/// Held on the [`WindGen`] as `Option<Box<WindGenUserModelSlot>>`. [`Clone`] is
/// manual: it clones the load *spec* (module bytes + attribution + the last edit
/// string + the cached variable surface) and drops the live wasmi instance,
/// which is never shared. It exists because `WindGen` derives [`Clone`] and the
/// arena takes owned element snapshots (`ClassArena::make_like_within`, the
/// control-dispatch `clone_ckt`); a cloned slot reports `exists() == false`
/// until [`Self::ensure_live`] re-creates the instance.
///
/// `MakeLike` does **not** go through it: `UserModel.Name := Other.UserModel.Name`
/// (`WindGen.pas:829`) is a `Set_Name`, i.e. an eager free + `FNew`, so
/// [`WindGen::make_like`] queues a real load instead (see its comment).
pub struct WindGenUserModelSlot {
    /// The model attribution — the `.wasm` path exactly as written.
    model: String,
    /// The compiled module bytes (an `Arc` so a `clone`/`MakeLike` shares them
    /// without touching the filesystem).
    wasm: Arc<[u8]>,
    /// The terminal-array order the V/I shuttle buffers are sized to (Pascal
    /// `Yorder`).
    yorder: usize,
    /// The last `UserData=` edit string (re-applied after a lazy reload so a
    /// cloned element sees the same model state on first use).
    data: String,
    /// The live wasmi instance — `None` before the first (re)load and after a
    /// `clone`, then re-created lazily.
    live: Option<Box<UserModelInstance>>,
    /// Cached `num_vars()` (refreshed after `new`/`edit`/`update_model`) — the
    /// `&self` `num_variables()` / `variable_name()` accessors read it.
    num_vars: usize,
    /// Cached 1-based variable names, refreshed alongside [`Self::num_vars`].
    var_names: Vec<String>,
}

impl Clone for WindGenUserModelSlot {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            wasm: Arc::clone(&self.wasm),
            yorder: self.yorder,
            data: self.data.clone(),
            // Drop the live instance — re-created lazily (Pascal MakeLike re-News).
            live: None,
            num_vars: self.num_vars,
            var_names: self.var_names.clone(),
        }
    }
}

impl std::fmt::Debug for WindGenUserModelSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindGenUserModelSlot")
            .field("model", &self.model)
            .field("exists", &self.live.is_some())
            .field("num_vars", &self.num_vars)
            .finish_non_exhaustive()
    }
}

impl WindGenUserModelSlot {
    /// Load + instantiate from resolved `.wasm` bytes (Pascal
    /// `TWindGenUserModel.Set_Name` → `FNew`, `WindGenUserModel.pas:158-220`).
    ///
    /// On success the model `exists()` and every call site runs it. On a
    /// [`UserModelError`] the caller maps it to the Pascal load-failure
    /// diagnostic (570/569) and leaves the slot absent.
    pub fn load(
        model: &str,
        wasm: &[u8],
        yorder: usize,
        g: &mut WindGen,
        sys: &SysCtx,
    ) -> Result<Self, UserModelError> {
        let host = UserModelHost::load(
            model,
            wasm,
            InterfaceKind::WindGenUserModel,
            HostConfig::default(),
        )?;
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, &[], &wv);
        let sh = WindGenShuttle {
            wind_gen_vars: &mut wv,
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        let instance = UserModelInstance::new(&host, yorder.max(1), sh)?;
        // The native DLL shares the element's live WindGenVars record (a retained
        // pointer), so a `New`-time write persists onto the element. Mirror it —
        // the read-back is unconditional (ABI doc §2, same rule as GenVars).
        apply_wind_gen_vars(g, &wv);
        let mut slot = Self {
            model: model.to_string(),
            wasm: Arc::from(wasm.to_vec()),
            yorder: yorder.max(1),
            data: String::new(),
            live: Some(Box::new(instance)),
            num_vars: 0,
            var_names: Vec::new(),
        };
        slot.refresh_var_cache();
        Ok(slot)
    }

    /// Pascal `Get_Exists` (`FID <> 0`, `WindGenUserModel.pas:131-139`). A slot
    /// with `live == None` and a spec (post-clone) reports `false` until the next
    /// `&mut` call re-creates the instance — matching Pascal, where a
    /// not-yet-`New`ed model does not exist.
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

    /// Ensure the live instance exists, re-creating it from the spec when this
    /// slot came from a [`Clone`] (the owned element snapshots the arena takes),
    /// then replaying `edit(data)` so the revived instance carries the same
    /// `UserData=` state the snapshot was taken from. A slot built by
    /// [`Self::load`] — every slot a `UserModel=` or a `like=` produces — is
    /// already live and returns immediately.
    fn ensure_live(&mut self, g: &mut WindGen, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        if self.live.is_some() {
            return Ok(());
        }
        let host = UserModelHost::load(
            &self.model,
            &self.wasm,
            InterfaceKind::WindGenUserModel,
            HostConfig::default(),
        )?;
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, node_v, &wv);
        let sh = WindGenShuttle {
            wind_gen_vars: &mut wv,
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        let instance = UserModelInstance::new(&host, self.yorder, sh)?;
        self.live = Some(Box::new(instance));
        apply_wind_gen_vars(g, &wv);
        if !self.data.is_empty() {
            self.edit_impl(&self.data.clone(), g, sys, node_v)?;
        }
        self.refresh_var_cache();
        Ok(())
    }

    /// Pascal `TWindGenUserModel.Edit` (`WindGenUserModel.pas:152-156`): send the
    /// `UserData=` string to a loaded model (ignored while absent). Refreshes the
    /// cached variable surface afterward.
    pub fn edit(
        &mut self,
        data: &str,
        g: &WindGen,
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
        g: &WindGen,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, node_v, &wv);
        let live = self.live.as_mut().expect("edit_impl requires a live model");
        let sh = WindGenShuttle {
            wind_gen_vars: &mut wv,
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        live.edit(data, sh)
    }

    /// Pascal `UserModel.FUpdateModel` (`WindGen.pas:1418`) — the
    /// `RecalcElementData` tail. Refreshes the cached variable surface.
    pub fn update_model(&mut self, g: &WindGen, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        if self.live.is_none() {
            return Ok(());
        }
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, node_v, &wv);
        {
            let live = self
                .live
                .as_mut()
                .expect("update_model requires a live model");
            let sh = WindGenShuttle {
                wind_gen_vars: &mut wv,
                dyn_rec: &mut dr,
                ctx: Box::new(ctx),
            };
            live.update_model(sh)?;
        }
        self.refresh_var_cache();
        Ok(())
    }

    /// Pascal `UserModel.FCalc(Vterminal, Iterminal)` — the power-flow
    /// (`:1887`) / dynamics (`:1993`) current computation. Writes `v` in, runs
    /// the guest, reads the terminal currents back into `i`, and applies the
    /// mutated `TWindGenVars` back onto `g`. The Pascal sign convention (negate
    /// into `InjCurrent`) stays at the caller.
    pub fn calc(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        g: &mut WindGen,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.ensure_live(g, sys, node_v)?;
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, node_v, &wv);
        {
            let live = self.live.as_mut().expect("calc requires a live model");
            let sh = WindGenShuttle {
                wind_gen_vars: &mut wv,
                dyn_rec: &mut dr,
                ctx: Box::new(ctx),
            };
            live.calc(v, i, sh)?;
        }
        apply_wind_gen_vars(g, &wv);
        Ok(())
    }

    /// Pascal `UserModel.FInit(Vterminal, Iterminal)` — the `InitStateVars`
    /// GenModel=6 seed (`WindGen.pas:2568`).
    pub fn init(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        g: &mut WindGen,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        self.ensure_live(g, sys, node_v)?;
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, node_v, &wv);
        {
            let live = self.live.as_mut().expect("init requires a live model");
            let sh = WindGenShuttle {
                wind_gen_vars: &mut wv,
                dyn_rec: &mut dr,
                ctx: Box::new(ctx),
            };
            live.init(v, i, sh)?;
        }
        apply_wind_gen_vars(g, &wv);
        Ok(())
    }

    /// Pascal `UserModel.Integrate` (`select(id)` + `integrate()`,
    /// `WindGenUserModel.pas:141-145`) — the `IntegrateStates` GenModel=6 tail
    /// (`WindGen.pas:2663`).
    pub fn integrate(&mut self, g: &mut WindGen, sys: &SysCtx, node_v: &[Complex64]) -> LiveResult {
        self.ensure_live(g, sys, node_v)?;
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, node_v, &wv);
        {
            let live = self.live.as_mut().expect("integrate requires a live model");
            let sh = WindGenShuttle {
                wind_gen_vars: &mut wv,
                dyn_rec: &mut dr,
                ctx: Box::new(ctx),
            };
            live.integrate(sh)?;
        }
        apply_wind_gen_vars(g, &wv);
        Ok(())
    }

    /// Pascal `UserModel.FGetAllVars(@States[NumWGenVariables+1])` — write the
    /// model's `num_vars` values into `out` (`WindGen.pas:2806`).
    pub fn get_all_vars(
        &mut self,
        out: &mut [f64],
        g: &WindGen,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, node_v, &wv);
        let live = self
            .live
            .as_mut()
            .expect("get_all_vars requires a live model");
        let sh = WindGenShuttle {
            wind_gen_vars: &mut wv,
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        live.get_all_vars(out, sh)
    }

    /// Pascal `UserModel.FSetVariable(k, value)` (1-based, `WindGen.pas:2781`).
    pub fn set_variable(
        &mut self,
        k: usize,
        value: f64,
        g: &WindGen,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) -> LiveResult {
        let mut wv = wind_gen_vars_from(g);
        let mut dr = dyn_rec_from(sys);
        let ctx = WindGenCallbacks::snapshot(g, sys, node_v, &wv);
        let live = self
            .live
            .as_mut()
            .expect("set_variable requires a live model");
        let sh = WindGenShuttle {
            wind_gen_vars: &mut wv,
            dyn_rec: &mut dr,
            ctx: Box::new(ctx),
        };
        live.set_variable(k as i32, value, sh)
    }

    /// Drain the tier-B effects (MsgCallback text) queued by the last guest call
    /// and route them into `errors` (Pascal `MsgCallBack` → `DoSimpleMsg`, errno
    /// 9000). Never a silent drop.
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
                    // No WindGen user model schedules control actions
                    // (`ControlQueuePush` is the CapControl surface, WM.5).
                    // Surface it loudly rather than silently drop.
                    errors.push(DssDiagnostic::msg(
                        format!(
                            "WindGen.{name}: user model called ControlQueuePush, which is not \
                             wired for WindGen elements (CapControl scope, WM.5)."
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
        let mut wv = WindGenVars::default();
        let mut dr = DynamicsRec::default();
        let n = {
            let sh = WindGenShuttle {
                wind_gen_vars: &mut wv,
                dyn_rec: &mut dr,
                ctx: Box::new(dss_usermodel::NoCallbacks),
            };
            live.num_vars(sh).unwrap_or(0).max(0) as usize
        };
        let mut names = Vec::with_capacity(n);
        // 0-based walk; the `+ 1` is the 1-based user-model API boundary
        // (`FGetVarName(VarNum)` counts from 1), converted at the call itself
        // rather than by a Pascal-shaped `1..=n` range.
        for k in 0..n {
            let mut wv2 = WindGenVars::default();
            let mut dr2 = DynamicsRec::default();
            let sh = WindGenShuttle {
                wind_gen_vars: &mut wv2,
                dyn_rec: &mut dr2,
                ctx: Box::new(dss_usermodel::NoCallbacks),
            };
            let name = live.get_var_name(k as i32 + 1, sh).unwrap_or_default();
            names.push(name);
        }
        self.num_vars = n;
        self.var_names = names;
    }
}

/// Result alias for the slot's `&mut` call paths.
type LiveResult = Result<(), UserModelError>;

impl WindGen {
    /// Queue a deferred `UserModel=` (re)load (Pascal `UserModel.Name := …` →
    /// `Set_Name`). A blank / `none` name unloads the slot in place and is not
    /// queued (Pascal `Set_Name` `Exit`, `WindGenUserModel.pas:177-178`).
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

    /// Queue a deferred `UserData=` edit (Pascal `UserModel.Edit := …`, which
    /// no-ops while `FID = 0`). Always queued; the drain no-ops when the model
    /// does not exist (existence resolved at drain time).
    pub(super) fn queue_user_model_edit(&mut self, data: String) {
        self.pending_user_model_loads.push(UserModelLoad {
            slot: UserModelSlot::User,
            action: UserModelAction::Edit(data),
        });
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
                self.user_model = None;
                match wasm {
                    Some(bytes) => {
                        let yorder = self.cd.yorder;
                        match WindGenUserModelSlot::load(model_name, bytes, yorder, self, sys) {
                            Ok(slot) => self.user_model = Some(Box::new(slot)),
                            Err(e) => push_load_failure(&name, model_name, &e, errors),
                        }
                    }
                    // Not a `.wasm` file (native-DLL name / missing file): the
                    // Pascal load-failure path (#570, `WindGenUserModel.pas:187`)
                    // — warn non-fatally + fall back.
                    None => errors.push(DssDiagnostic::msg(
                        format!(
                            "WindGen User Model {model_name} Not Loaded. WindGen.{name} falls \
                             back to its built-in model."
                        ),
                        Some(570),
                    )),
                }
            }
            UserModelAction::Edit(data) => {
                let Some(mut s) = self.user_model.take() else {
                    return; // Pascal `Set_Edit`: `If FID <> 0 Then FEdit(...)`.
                };
                let mut errs = ErrorLog::new();
                if let Err(e) = s.edit(data, self, sys, &[]) {
                    errs.push(DssDiagnostic::msg(e.to_string(), Some(569)));
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

    /// The loaded model's variable count, 0 when absent (Pascal
    /// `UserModel.FNumVars` guarded by `Exists`).
    pub(super) fn user_model_num_vars(&self) -> usize {
        if !self.user_model_exists() {
            return 0;
        }
        self.user_model.as_ref().map_or(0, |s| s.num_vars())
    }

    /// Pascal `UserModel.FCalc(Vterminal, Iterminal)` at the power-flow
    /// (`DoUserModel`, `WindGen.pas:1887`) / dynamics (`DoDynamicMode`, `:1993`)
    /// call site: write `Vterminal` in, read the terminal currents back into
    /// `self.cd.iterminal`. Returns `true` iff a model existed and ran (the
    /// caller then negates `Iterminal` into `InjCurrent`); `false` → the caller
    /// records the missing-model diagnostic.
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
            // has no Pascal analogue: ABI §6 makes it a HARD, loud engine error,
            // not a silent mid-run fallback (which would silently change
            // numerics). Flag `abort` so the inject-path caller lifts
            // `SolutionAbort` rather than converging on a stale terminal current.
            Err(e) => errors.push(DssDiagnostic::abort(
                format!("WindGen.{name}: user model `calc` trapped/faulted: {e}"),
                Some(567),
            )),
        }
        um.drain_effects(&name, errors);
        self.user_model = Some(um);
        true
    }

    /// Pascal `RecalcElementData` tail (`WindGen.pas:1418`):
    /// `If Usermodel.Exists Then UserModel.FUpdateModel`. Called at the end of
    /// [`WindGen::recalc`], before `WindModelDyn.ReCalcElementData` (`:1422`).
    pub(super) fn update_user_models(&mut self, sys: &SysCtx) {
        let Some(mut s) = self.user_model.take() else {
            return;
        };
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = s.update_model(self, sys, &[]) {
            errs.push(DssDiagnostic::msg(e.to_string(), Some(569)));
        }
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    /// Pascal `InitStateVars` GenModel=6 arm (`WindGen.pas:2566-2570`):
    /// `If UserModel.Exists Then UserModel.FInit(Vterminal, Iterminal)`, seeding
    /// the model from the terminal V/I *left in the buffers* by the preceding
    /// power-flow solve.
    ///
    /// `Vterminal` is deliberately NOT recomputed here: Pascal `InitStateVars`
    /// runs only `ComputeIterminal` (`:2512`) — never `ComputeVterminal` —
    /// before `FInit`, so the user model is seeded from the STALE `Vterminal`
    /// buffer, i.e. the node voltage of the power flow's *last injection
    /// iteration*. This mirrors the Generator slot's `user_model_finit`
    /// (the WM.3 D2 finding), which is the same Pascal shape.
    pub(super) fn user_model_finit(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let Some(mut s) = self.user_model.take() else {
            return;
        };
        if !s.exists() {
            self.user_model = Some(s);
            return;
        }
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        let v = self.cd.vterminal.clone();
        let mut it = self.cd.iterminal.clone();
        match s.init(&v, &mut it, self, sys, node_v) {
            Ok(()) => self.cd.iterminal.copy_from_slice(&it),
            Err(e) => errs.push(DssDiagnostic::msg(
                format!("WindGen.{name}: user model `init` failed: {e}"),
                Some(567),
            )),
        }
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    /// Pascal `IntegrateStates` GenModel=6 arm (`WindGen.pas:2662-2665`):
    /// `UserModel.Integrate()` (= `select(id)` + `integrate()`).
    pub(super) fn user_model_fintegrate(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let Some(mut s) = self.user_model.take() else {
            return;
        };
        if !s.exists() {
            self.user_model = Some(s);
            return;
        }
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = s.integrate(self, sys, node_v) {
            errs.push(DssDiagnostic::msg(
                format!("WindGen.{name}: user model `integrate` failed: {e}"),
                Some(567),
            ));
        }
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    /// Fill `out` with the user model's variable values (Pascal
    /// `UserModel.FGetAllVars(@States[NumWGenVariables+1])`, `WindGen.pas:2806`).
    /// Effects/errors are routed onto the element (surfaced in `errors()`).
    pub(super) fn get_all_user_model_vars(
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
                format!("WindGen.{name}: user model `get_all_vars` failed: {e}"),
                Some(567),
            ));
        }
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }

    /// Pascal `Set_Variable` user-model tail (`WindGen.pas:2777-2784`): route a
    /// 1-based state-variable write past the 22 native slots to the user model
    /// (`k = i - NumWGenVariables`).
    ///
    /// **Upstream bug deliberately NOT reproduced** (so: no compat marker — that
    /// tag is for precision quirks we *do* reproduce). r4133's user-model tail
    /// sits OUTSIDE the `if i < 19 … else case i of …` chain rather than in its
    /// `else` (contrast `generator.pas:2897-2913`, where the identical block is
    /// inside the `Else` of the `Case`), so once a model exists EVERY native
    /// index `1..=22` also satisfies `k = i - 22 <= N` and gets overwritten by
    /// `UserModel.FSetVariable(k)` with a non-positive `k`. The port routes
    /// `1..=22` to the native slots and only `> 22` to the model
    /// (`investigations/to_opendss/`).
    pub(super) fn set_user_model_variable(&mut self, i: usize, value: f64, sys: &SysCtx) {
        let base = self.num_wgen_variables();
        let un = self.user_model_num_vars();
        if !(i > base && i <= base + un) {
            return;
        }
        let k = i - base;
        let Some(mut s) = self.user_model.take() else {
            return;
        };
        let name = self.cd.obj.name().to_string();
        let mut errs = ErrorLog::new();
        if let Err(e) = s.set_variable(k, value, self, sys, &[]) {
            errs.push(DssDiagnostic::msg(
                format!("WindGen.{name}: user model `set_variable` failed: {e}"),
                Some(567),
            ));
        }
        s.drain_effects(&name, &mut errs);
        self.user_model = Some(s);
        for d in errs.into_vec() {
            self.cd.obj.push_error(d);
        }
    }
}

/// Map a load-time [`UserModelError`] onto the Pascal load-failure diagnostic: a
/// missing export → 569 ("Does Not Have Required Function",
/// `WindGenUserModel.pas:102`), anything else → 570 ("… Not Loaded",
/// warn-and-fallback, `:187`). The model stays absent either way.
fn push_load_failure(name: &str, model_name: &str, e: &UserModelError, errors: &mut ErrorLog) {
    match e {
        UserModelError::MissingExport { .. } | UserModelError::SignatureMismatch { .. } => {
            errors.push(DssDiagnostic::msg(e.to_string(), Some(569)));
        }
        _ => errors.push(DssDiagnostic::msg(
            format!(
                "WindGen User Model {model_name} Not Loaded ({e}). WindGen.{name} falls back to \
                 its built-in model."
            ),
            Some(570),
        )),
    }
}

/// Build the packed `TWindGenVars` image from the WindGen's flattened record
/// fields (ABI doc §2.6 — the 348-byte wasm image; the managed `PLoss: string`
/// never crosses, and its native hole is closed).
pub(super) fn wind_gen_vars_from(g: &WindGen) -> WindGenVars {
    WindGenVars {
        theta: g.theta,
        pshaft: g.p_shaft,
        speed: g.speed,
        w0: g.w0,
        hmass: g.h_mass,
        mmass: g.m_mass,
        d: g.d_damping,
        dpu: g.dpu,
        kva_rating: g.kva_rating,
        kv_windgen_base: g.kv_windgen_base,
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
        ag: g.ag,
        cp: g.cp,
        lamda: g.lamda,
        poles: g.poles,
        pd: g.pd,
        rad: g.rad,
        v_cutin: g.v_cut_in,
        v_cutout: g.v_cut_out,
        pm: g.pm,
        ps: g.ps,
        pr: g.pr,
        pg: g.pg,
        s: g.s,
    }
}

/// Apply the mutated `TWindGenVars` back onto the WindGen after a guest call.
/// ABI doc §2 freezes the read-back as **unconditional** — a native DLL shares
/// the record live (`TWindGenUserModel.Create(@WindGenVars)`, `WindGen.pas:995`),
/// so *any* field the model writes persists; the wasm host reproduces that by
/// copying the full image back, not a state-only subset. f64 round-trips are bit
/// exact, so a field the model did not touch is a no-op.
///
/// The structural ints (`num_phases`/`num_conductors`/`conn`) are the ONE
/// exception: they stay element-owned and are never read back — they define the
/// terminal/YPrim shape the host allocated the buffers and node map against
/// (identical rule to `apply_gen_vars`). `PLoss` never crosses at all (§2.6).
pub(super) fn apply_wind_gen_vars(g: &mut WindGen, wv: &WindGenVars) {
    g.theta = wv.theta;
    g.p_shaft = wv.pshaft;
    g.speed = wv.speed;
    g.w0 = wv.w0;
    g.h_mass = wv.hmass;
    g.m_mass = wv.mmass;
    g.d_damping = wv.d;
    g.dpu = wv.dpu;
    g.kva_rating = wv.kva_rating;
    g.kv_windgen_base = wv.kv_windgen_base;
    g.xd = wv.xd;
    g.xdp = wv.xdp;
    g.xdpp = wv.xdpp;
    g.pu_xd = wv.pu_xd;
    g.pu_xdp = wv.pu_xdp;
    g.pu_xdpp = wv.pu_xdpp;
    g.dtheta = wv.dtheta;
    g.dspeed = wv.dspeed;
    g.theta_history = wv.theta_history;
    g.speed_history = wv.speed_history;
    g.p_nominal_per_phase = wv.pnominalperphase;
    g.q_nominal_per_phase = wv.qnominalperphase;
    g.v_thev_mag = wv.vthev_mag;
    g.v_thev_harm = wv.vthev_harm;
    g.theta_harm = wv.theta_harm;
    g.v_target = wv.vtarget;
    g.zthev = Complex64::new(wv.zthev.0, wv.zthev.1);
    g.xrdp = wv.xrdp;
    g.ag = wv.ag;
    g.cp = wv.cp;
    g.lamda = wv.lamda;
    g.poles = wv.poles;
    g.pd = wv.pd;
    g.rad = wv.rad;
    g.v_cut_in = wv.v_cutin;
    g.v_cut_out = wv.v_cutout;
    g.pm = wv.pm;
    g.ps = wv.ps;
    g.pr = wv.pr;
    g.pg = wv.pg;
    g.s = wv.s;
}

/// Build the packed `TDynamicsRec` image from the solution context (ABI doc
/// §2.1). `solution_mode` selects the model's power-flow-vs-dynamics `Calc`
/// dispatch (DYNAMICMODE = 14); `iteration_flag` is the predictor/corrector
/// selector.
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
/// owning WindGen (ABI doc §4). Owned (not borrowed) because it is moved into
/// the wasmi `Store` and must be `'static`/`Send`.
struct WindGenCallbacks {
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

impl WindGenCallbacks {
    /// Snapshot the owning WindGen's tier-A state.
    fn snapshot(g: &WindGen, sys: &SysCtx, node_v: &[Complex64], wv: &WindGenVars) -> Self {
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
        WindGenCallbacks {
            name: format!("WindGen.{}", g.cd.obj.name()),
            voltages,
            currents: g.cd.iterminal.clone(),
            node_v: node_slice,
            // Pascal `PublicDataStruct := @WindGenVars; PublicDataSize :=
            // SizeOf(TWindGenVars)` (`WindGen.pas:992-993`) — the wasm image
            // (348 B) stands in for the native 356-byte record (ABI doc §2.6).
            public_data: wv.to_bytes().to_vec(),
            dyn_rec: dyn_rec_from(sys),
            nterms: g.cd.nterms as i32,
            nconds: g.cd.nconds as i32,
            nphases: g.cd.nphases as i32,
            enabled: g.cd.enabled,
        }
    }
}

impl Callbacks for WindGenCallbacks {
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
