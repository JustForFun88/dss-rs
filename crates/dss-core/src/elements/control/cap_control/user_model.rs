//! The dss-core side of the WASM user-model host for the CapControl
//! (`UserModel=`/`UserData=`, WASM_USERMODELS WM.5).
//!
//! Mirrors the Pascal `UserModel: TCapUserControl` field
//! (`Controls/CapControl.pas:165`, `Controls/CapUserControl.pas`): a bound model
//! is (re)created by `Set_Name` (guest `new`), holds a live wasmi instance
//! ([`dss_usermodel::CapControlInstance`], the 7-function interface), and
//! services the control call sites — `edit`/`update_model` after a property
//! edit (`CapControl.pas:429-440`), `sample()` at `Sample` (`:1024-1041`), and
//! `do_pending(code, proxy)` at `DoPendingAction` (`:726-733`) — through the
//! `dss_env` callback surface (NO boundary records: the 7-fn `sample()` takes no
//! arguments; the model reads its context via callbacks and schedules via
//! `control_queue_push`, ABI doc §2.5).
//!
//! The transport is sandboxed WebAssembly (`crates/dss-usermodel`) rather than a
//! native DLL — permanently required by `#![forbid(unsafe_code)]` — but the
//! contract is 1:1: the same call ordering and the same warn-and-fallback
//! failure behavior (plan §2.4 / ABI doc §5).

use std::sync::Arc;

use num_complex::Complex64;

use dss_usermodel::{
    Callbacks, CapControlInstance, DynamicsRec, Effect, HostConfig, InterfaceKind, NoCallbacks,
    UserModelError, UserModelHost,
};

use crate::diag::{DssDiagnostic, ErrorLog};
use crate::obj::base::{UserModelAction, UserModelLoad, UserModelSlot};

use super::{CapControl, CapControlType};

/// One bound CapControl user model — the dss-core wrapper around a
/// [`UserModelHost`] + [`CapControlInstance`] (Pascal `TCapUserControl`).
///
/// Held on the [`CapControl`](super::CapControl) as
/// `Option<Box<CapControlUserModelSlot>>`. [`Clone`] is manual: it clones the
/// load *spec* (module bytes + attribution + the last edit string) and drops the
/// live wasmi instance — faithful to Pascal `MakeLike`, which re-`New`s a fresh
/// instance rather than copying live guest state. The live instance is re-created
/// lazily on the next `&mut` control call site.
pub struct CapControlUserModelSlot {
    /// The model attribution — the `.wasm` path exactly as written (the value of
    /// every error/diagnostic, ABI doc §6).
    model: String,
    /// The compiled module bytes (an `Arc` so a `clone`/`MakeLike` shares them
    /// without touching the filesystem).
    wasm: Arc<[u8]>,
    /// The last `UserData=` edit string (re-applied after a lazy reload so a
    /// cloned element sees the same model state on first use).
    data: String,
    /// The live wasmi instance — `None` before the first (re)load and after a
    /// `clone`, then re-created lazily.
    live: Option<Box<CapControlInstance>>,
}

impl Clone for CapControlUserModelSlot {
    fn clone(&self) -> Self {
        Self {
            model: self.model.clone(),
            wasm: Arc::clone(&self.wasm),
            data: self.data.clone(),
            // Drop the live instance — re-created lazily (Pascal MakeLike re-News).
            live: None,
        }
    }
}

impl std::fmt::Debug for CapControlUserModelSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapControlUserModelSlot")
            .field("model", &self.model)
            .field("exists", &self.live.is_some())
            .finish_non_exhaustive()
    }
}

/// Result alias for the slot's `&mut` call paths.
type LiveResult = Result<(), UserModelError>;

impl CapControlUserModelSlot {
    /// Load + instantiate from resolved `.wasm` bytes (Pascal
    /// `TCapUserControl.Set_Name` → `FNew(CallBackRoutines)`,
    /// `CapUserControl.pas:142-195`).
    ///
    /// On success the model `exists()` and every call site runs it. On a
    /// [`UserModelError`] the caller maps it to the Pascal load-failure
    /// diagnostic (570/569) and leaves the slot absent (built-in fallback).
    pub fn load(model: &str, wasm: &[u8]) -> Result<Self, UserModelError> {
        let host = UserModelHost::load(
            model,
            wasm,
            InterfaceKind::CapUserControl,
            HostConfig::default(),
        )?;
        // The guest `new()` reads no callbacks (the 7-fn `New` receives only the
        // callback vtable; a model uses it later in `sample`/`do_pending`).
        let instance = CapControlInstance::new(&host, Box::new(NoCallbacks))?;
        Ok(Self {
            model: model.to_string(),
            wasm: Arc::from(wasm.to_vec()),
            data: String::new(),
            live: Some(Box::new(instance)),
        })
    }

    /// Pascal `Get_Exists` (`FID <> 0`, `CapUserControl.pas:116-124`).
    pub fn exists(&self) -> bool {
        self.live.as_ref().is_some_and(|l| l.exists())
    }

    /// The model attribution.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Ensure the live instance exists, re-creating it from the spec after a
    /// clone (lazy Pascal-`New`), then run `edit(data)` to restore the last
    /// `UserData=` state.
    fn ensure_live(&mut self) -> LiveResult {
        if self.live.is_some() {
            return Ok(());
        }
        let host = UserModelHost::load(
            &self.model,
            &self.wasm,
            InterfaceKind::CapUserControl,
            HostConfig::default(),
        )?;
        let instance = CapControlInstance::new(&host, Box::new(NoCallbacks))?;
        self.live = Some(Box::new(instance));
        if !self.data.is_empty() {
            let data = self.data.clone();
            self.live
                .as_mut()
                .expect("just created")
                .edit(&data, Box::new(NoCallbacks))?;
        }
        Ok(())
    }

    /// Pascal `TCapUserControl.Edit` (`CapUserControl.pas:137-140`): send the
    /// `UserData=` string to a loaded model (ignored while absent).
    pub fn edit(&mut self, data: &str) -> LiveResult {
        self.data = data.to_string();
        let Some(live) = self.live.as_mut() else {
            // Nothing loaded yet — store the string; a later ensure_live replays it.
            return Ok(());
        };
        live.edit(data, Box::new(NoCallbacks))
    }

    /// Pascal `UserModel.UpdateModel` (`CapControl.pas:639`) — after
    /// `RecalcElementData`.
    pub fn update_model(&mut self) -> LiveResult {
        let Some(live) = self.live.as_mut() else {
            return Ok(());
        };
        live.update_model(Box::new(NoCallbacks))
    }

    /// Pascal `UserModel.Sample` "Sets the switching flags"
    /// (`CapControl.pas:1040`). The `ctx` snapshot carries the model's `Sample`
    /// context (node voltages, dynamics time, the owning `@ControlVars` image)
    /// and the model schedules its switch decision via `control_queue_push`,
    /// drained by the caller from [`Self::drain_effects`].
    pub fn sample(&mut self, ctx: Box<dyn Callbacks>) -> LiveResult {
        self.ensure_live()?;
        self.live.as_mut().expect("ensure_live").sample(ctx)
    }

    /// Pascal `UserModel.DoPending(Code, ProxyHdl)` (`CapControl.pas:729`).
    pub fn do_pending(&mut self, code: i32, proxy_hdl: i32, ctx: Box<dyn Callbacks>) -> LiveResult {
        self.ensure_live()?;
        self.live
            .as_mut()
            .expect("ensure_live")
            .do_pending(code, proxy_hdl, ctx)
    }

    /// Drain the tier-B effects (control-queue pushes, messages) queued by the
    /// last guest call, for the caller to route into the real control queue /
    /// error sink (never a silent drop, plan §2.9-5).
    pub fn drain_effects(&mut self) -> Vec<Effect> {
        self.live
            .as_mut()
            .map(|l| l.drain_effects())
            .unwrap_or_default()
    }
}

/// An owned per-call context snapshot serving the tier-A `dss_env` reads for the
/// owning CapControl's `sample()` (ABI doc §4, §2.5). Owned (not borrowed)
/// because it is moved into the wasmi `Store` and must be `'static`/`Send`.
///
/// The reference `capuserctl` fixture reads its control voltage through
/// `get_node_voltages` (`node_v`, ground-excluded) + the dynamics time
/// (`dyn_rec`); the owning `CapControlVars` image is also served as
/// `get_public_data` for completeness (models that opt into it — the un-gatable
/// channel of ABI doc §2.5). `next_handle` seeds the provisional
/// `control_queue_push` return so it reproduces the engine's real queue handle.
pub struct CapCallbacks {
    node_v: Vec<Complex64>,
    dyn_rec: DynamicsRec,
    public_data: Vec<u8>,
    next_handle: i32,
}

impl CapCallbacks {
    /// Build the snapshot. `node_v_full` is the full `Solution.NodeV` (slot 0 =
    /// ground); the ground slot is dropped to match the ABI-doc row-17 copy
    /// semantics (`node k` at `dest + (k-1)*16`). `public_data` is the owning
    /// CapControl's `CapControlVars` image (`get_public_data`). `next_handle` is
    /// the control queue's next handle.
    pub fn snapshot(
        node_v_full: &[Complex64],
        dyn_rec: DynamicsRec,
        public_data: Vec<u8>,
        next_handle: i32,
    ) -> Self {
        let node_v = if node_v_full.len() > 1 {
            node_v_full[1..].to_vec()
        } else {
            Vec::new()
        };
        Self {
            node_v,
            dyn_rec,
            public_data,
            next_handle,
        }
    }
}

impl Callbacks for CapCallbacks {
    fn node_voltages(&self) -> &[Complex64] {
        &self.node_v
    }
    fn dynamics_rec(&self) -> Option<DynamicsRec> {
        Some(self.dyn_rec)
    }
    fn public_data(&self) -> &[u8] {
        &self.public_data
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
    fn control_queue_next_handle(&self) -> i32 {
        self.next_handle
    }
}

impl CapControl {
    /// Queue a deferred `UserModel=` (re)load (Pascal `UserModel.Name := …` →
    /// `Set_Name`). A blank / `none` name unloads the slot in place and is not
    /// queued (Pascal `Set_Name` `Exit`).
    pub(super) fn queue_user_model_load(&mut self, name: String) {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            self.user_model = None;
            self.is_user_model = false;
            return;
        }
        self.pending_user_model_loads.push(UserModelLoad {
            slot: UserModelSlot::User,
            action: UserModelAction::Load(name),
        });
    }

    /// Queue a deferred `UserData=` edit (Pascal
    /// `if UserModel.Exists then UserModel.Edit(…)`). Always queued; the drain
    /// no-ops when the model does not exist (existence resolved at drain time).
    pub(super) fn queue_user_model_edit(&mut self, data: String) {
        self.pending_user_model_loads.push(UserModelLoad {
            slot: UserModelSlot::User,
            action: UserModelAction::Edit(data),
        });
    }

    /// The queued deferred requests, taken for the executive to resolve.
    pub(super) fn take_user_model_loads(&mut self) -> Vec<UserModelLoad> {
        std::mem::take(&mut self.pending_user_model_loads)
    }

    /// Whether the `UserModel=` slot holds a loaded model (Pascal
    /// `UserModel.Exists`).
    pub(super) fn user_model_exists(&self) -> bool {
        self.user_model.as_ref().is_some_and(|s| s.exists())
    }

    /// The executive-drained apply for a resolved [`UserModelLoad`] (§2.4
    /// activation rule). `wasm` is `Some` iff a `.wasm` file was found + read.
    /// Mirrors Pascal `PropertySideEffects` (`CapControl.pas:429-440`):
    /// `UserModel.Name := …` (load) → `IsUserModel := UserModel.Exists` →
    /// `if IsUserModel then ControlType := USERCONTROL`.
    pub(super) fn apply_user_model_load_impl(
        &mut self,
        load: &UserModelLoad,
        wasm: Option<&[u8]>,
        errors: &mut ErrorLog,
    ) {
        let name = self.ccd.cd.obj.name().to_string();
        match &load.action {
            UserModelAction::Load(model_name) => {
                // Pascal `Set_Name` frees the previous model before (re)loading.
                self.user_model = None;
                self.is_user_model = false;
                match wasm {
                    Some(bytes) => match CapControlUserModelSlot::load(model_name, bytes) {
                        Ok(slot) => {
                            self.user_model = Some(Box::new(slot));
                            // Pascal `IsUserModel := UserModel.Exists`.
                            self.is_user_model = self.user_model_exists();
                        }
                        Err(e) => push_load_failure(&name, model_name, &e, errors),
                    },
                    // Not a `.wasm` file (native-DLL name / missing file): the
                    // Pascal load-failure path — warn non-fatally + fall back.
                    None => errors.push(DssDiagnostic::msg(
                        format!(
                            "CapControl User Model {model_name} Not Loaded. CapControl.{name} \
                             falls back to its built-in control."
                        ),
                        Some(570),
                    )),
                }
                // Pascal `if IsUserModel then ControlType := USERCONTROL`
                // (`:439-440`).
                if self.is_user_model {
                    self.control_type = CapControlType::UserControl;
                }
            }
            UserModelAction::Edit(data) => {
                let Some(s) = self.user_model.as_mut() else {
                    return; // Pascal: `if UserModel.Exists then Edit`.
                };
                if let Err(e) = s.edit(data) {
                    errors.push(DssDiagnostic::msg(e.to_string(), Some(569)));
                }
                for eff in s.drain_effects() {
                    route_non_queue_effect(&name, eff, errors);
                }
            }
        }
    }
}

/// Map a load-time [`UserModelError`] onto the Pascal load-failure diagnostic:
/// a missing export → 569 ("Does Not Have Required Function"), anything else →
/// 570 ("… Not Loaded", warn-and-fallback). The model stays absent either way
/// (the built-in control solves — plan §2.4).
fn push_load_failure(name: &str, model_name: &str, e: &UserModelError, errors: &mut ErrorLog) {
    match e {
        UserModelError::MissingExport { .. } | UserModelError::SignatureMismatch { .. } => {
            errors.push(DssDiagnostic::msg(e.to_string(), Some(569)));
        }
        _ => errors.push(DssDiagnostic::msg(
            format!(
                "CapControl User Model {model_name} Not Loaded ({e}). CapControl.{name} falls \
                 back to its built-in control."
            ),
            Some(570),
        )),
    }
}

/// Route a non-control-queue effect (a `Msg`) into the error sink. A
/// `ControlQueuePush` from `edit`/`do_pending` (rather than `sample`) has no
/// queue in scope; it is surfaced loudly rather than silently dropped (plan
/// §2.9-5) — the reference fixture only pushes from `sample`.
pub(super) fn route_non_queue_effect(name: &str, eff: Effect, errors: &mut ErrorLog) {
    match eff {
        Effect::Msg(text) => errors.push(DssDiagnostic::msg(text, Some(9000))),
        Effect::ControlQueuePush { .. } => errors.push(DssDiagnostic::msg(
            format!(
                "CapControl.{name}: user model pushed a control action outside `sample` \
                 (no control queue in scope); ignored."
            ),
            Some(9001),
        )),
    }
}
