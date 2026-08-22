//! Per-element-binding model instances: [`UserModelInstance`] (the 15/13
//! function interfaces) and [`CapControlInstance`] (the 7-function control
//! interface), driving the record shuttle of ABI doc §2 over guest memory.
//!
//! Template: the typst `PluginInstance` (`.inputs/typst/crates/
//! typst-library/src/foundations/plugin.rs:413-555`) — one `Store` + wasm
//! instance per binding, marshalling through the guest's exported memory. On
//! top of it: per-call fuel budgets, the store memory limiter, guest-owned
//! `dss_alloc` buffers held for the instance's lifetime, and the unconditional
//! record write-back (the Pascal contract lets the model mutate GenVars and
//! DynaVars — ABI doc §2).
//!
//! Instances are per-element-owned (plan §2.7): no global registry, no shared
//! `Store`; `wasmi::Store` is `Send`, so the MULTITHREADING M3
//! disjoint-`&mut` pattern holds.

use num_complex::Complex64;
use wasmi::{Store, TrapCode, TypedFunc};

use crate::callbacks::{CallData, Callbacks, Effect, Fault, NoCallbacks};
use crate::error::UserModelError;
use crate::host::{InterfaceKind, UserModelHost, VarsRecord};
use crate::records::{DynamicsRec, GeneratorVars, WindGenVars};

/// `maxlen` passed to the guest's `get_var_name` (the host-owned scratch is
/// `maxlen + 1` bytes, Pascal `StrLCopy` convention — ABI doc "Conventions").
const NAME_MAXLEN: usize = 255;

/// What the host shuttles into/out of guest memory around one interface call
/// (ABI doc §2): the boundary records (written before, read back after —
/// unconditionally, the model may mutate both) and the tier-A context
/// snapshot serving `dss_env` reads during the call.
///
/// This is the **Generator-family** shuttle. WindGen carries a different
/// boundary record and uses [`WindGenShuttle`]; both are accepted wherever a
/// call takes `impl `[`IntoShuttle`].
pub struct Shuttle<'a> {
    /// `TGeneratorVars` mirror — `Some` exactly for
    /// [`InterfaceKind::GenUserModel`] instances.
    pub gen_vars: Option<&'a mut GeneratorVars>,
    /// `TDynamicsRec` mirror — every 15/13-fn interface shares it.
    pub dyn_rec: &'a mut DynamicsRec,
    /// The per-call context snapshot (plan §2.3 tier A; consumed by the call).
    pub ctx: Box<dyn Callbacks>,
}

impl<'a> Shuttle<'a> {
    /// Shuttle for the non-Generator interfaces (no GenVars record).
    pub fn without_gen_vars(dyn_rec: &'a mut DynamicsRec, ctx: Box<dyn Callbacks>) -> Self {
        Self {
            gen_vars: None,
            dyn_rec,
            ctx,
        }
    }
}

/// The [`InterfaceKind::WindGenUserModel`] shuttle: the same three parts as
/// [`Shuttle`], carrying `TWindGenVars` instead of `TGeneratorVars`
/// (`PCElements/WindGenUserModel.pas:34` — `FNew(Var GenVars: TWindGenVars; …)`).
///
/// The record is not optional here: WindGen is the only kind that uses this
/// shuttle and it always passes one, exactly as the Pascal loader does
/// (`TWindGenUserModel.Create(@WindGenVars)`, `WindGen.pas:995`).
pub struct WindGenShuttle<'a> {
    /// `TWindGenVars` mirror — the 348-byte wasm image (ABI doc §2.6).
    pub wind_gen_vars: &'a mut WindGenVars,
    /// `TDynamicsRec` mirror.
    pub dyn_rec: &'a mut DynamicsRec,
    /// The per-call context snapshot (plan §2.3 tier A; consumed by the call).
    pub ctx: Box<dyn Callbacks>,
}

/// Which boundary record a shuttle carries into guest memory (ABI doc §2).
///
/// The two record images are **not** interchangeable — different sizes and,
/// past offset 244, different fields — so the instance checks this variant
/// against its host's [`InterfaceKind`] before writing anything.
pub enum ShuttleVars<'a> {
    /// No boundary record: Storage `UserModel=`/`DynaDLL=`, PVSystem
    /// `UserModel=` (their `new` takes only the dynarec pointer).
    None,
    /// `TGeneratorVars` (244-byte wasm image, ABI doc §2.2b).
    Gen(&'a mut GeneratorVars),
    /// `TWindGenVars` (348-byte wasm image, ABI doc §2.6).
    WindGen(&'a mut WindGenVars),
}

impl ShuttleVars<'_> {
    /// The record this variant carries, for the kind check.
    fn record(&self) -> Option<VarsRecord> {
        match self {
            Self::None => None,
            Self::Gen(_) => Some(VarsRecord::Generator),
            Self::WindGen(_) => Some(VarsRecord::WindGen),
        }
    }

    /// The packed image to write into the guest buffer.
    fn to_image(&self) -> Option<Vec<u8>> {
        match self {
            Self::None => None,
            Self::Gen(g) => Some(g.to_bytes().to_vec()),
            Self::WindGen(w) => Some(w.to_bytes().to_vec()),
        }
    }

    /// Apply a guest-side image back onto the caller's record.
    fn apply(&mut self, bytes: Vec<u8>) {
        match self {
            Self::None => {}
            Self::Gen(g) => **g = GeneratorVars::from_bytes(&bytes.try_into().expect("sized read")),
            Self::WindGen(w) => {
                **w = WindGenVars::from_bytes(&bytes.try_into().expect("sized read"));
            }
        }
    }
}

/// A shuttle the 15/13-function call surface accepts: [`Shuttle`] (Generator
/// family) or [`WindGenShuttle`].
///
/// Every call takes `impl IntoShuttle<'a>` rather than a concrete type so the
/// two record shapes share one implementation of the copy-in/copy-out envelope
/// — the *transport* is identical, only the image differs (ABI doc §2).
pub trait IntoShuttle<'a> {
    /// Split into (boundary record, dynamics record, per-call context).
    fn into_shuttle_parts(self) -> (ShuttleVars<'a>, &'a mut DynamicsRec, Box<dyn Callbacks>);
}

impl<'a> IntoShuttle<'a> for Shuttle<'a> {
    fn into_shuttle_parts(self) -> (ShuttleVars<'a>, &'a mut DynamicsRec, Box<dyn Callbacks>) {
        let vars = match self.gen_vars {
            Some(g) => ShuttleVars::Gen(g),
            None => ShuttleVars::None,
        };
        (vars, self.dyn_rec, self.ctx)
    }
}

impl<'a> IntoShuttle<'a> for WindGenShuttle<'a> {
    fn into_shuttle_parts(self) -> (ShuttleVars<'a>, &'a mut DynamicsRec, Box<dyn Callbacks>) {
        (
            ShuttleVars::WindGen(self.wind_gen_vars),
            self.dyn_rec,
            self.ctx,
        )
    }
}

/// The split shuttle the envelope works on (private: the public surface is the
/// two shuttle structs and [`IntoShuttle`]).
struct Parts<'a> {
    vars: ShuttleVars<'a>,
    dyn_rec: &'a mut DynamicsRec,
    ctx: Box<dyn Callbacks>,
}

impl<'a> Parts<'a> {
    fn split(sh: impl IntoShuttle<'a>) -> Self {
        let (vars, dyn_rec, ctx) = sh.into_shuttle_parts();
        Self { vars, dyn_rec, ctx }
    }
}

/// The store/instance/buffer plumbing shared by both instance types.
struct Sandbox {
    store: Store<CallData>,
    instance: wasmi::Instance,
    memory: wasmi::Memory,
    dss_alloc: TypedFunc<i32, i32>,
    model: String,
    fuel_per_call: u64,
}

impl Sandbox {
    fn new(host: &UserModelHost) -> Result<Self, UserModelError> {
        let mut store = Store::new(&host.engine, CallData::new(host.config.memory_cap_bytes));
        // The store limiter enforcing the linear-memory cap (plan §2.1;
        // `trap_on_grow_failure` makes a breach a deterministic trap
        // classified as `MemoryCapExceeded`).
        store.limiter(|data| &mut data.limits);
        // Fuel for instantiation itself (start section, data-segment init).
        store
            .set_fuel(host.config.fuel_per_call)
            .expect("fuel metering is enabled in the engine config");

        let instance = host
            .linker
            .instantiate_and_start(&mut store, &host.module)
            .map_err(|e| UserModelError::Instantiation {
                model: host.model.clone(),
                detail: e.to_string(),
            })?;
        let memory =
            instance
                .get_memory(&store, "memory")
                .ok_or_else(|| UserModelError::Instantiation {
                    model: host.model.clone(),
                    detail: "exported `memory` missing after instantiation".to_string(),
                })?;
        let dss_alloc = typed_func(&host.model, &instance, &store, "dss_alloc")?;
        Ok(Self {
            store,
            instance,
            memory,
            dss_alloc,
            model: host.model.clone(),
            fuel_per_call: host.config.fuel_per_call,
        })
    }

    /// Refill the per-call fuel budget.
    fn refuel(&mut self) {
        self.store
            .set_fuel(self.fuel_per_call)
            .expect("fuel metering is enabled in the engine config");
    }

    /// Turn a guest-call outcome into the typed error contract: a recorded
    /// host fault wins (typst's `memory_error` take-pattern,
    /// `plugin.rs:494-501`), then fuel/memory-cap trap codes, then a generic
    /// trap — all naming the model and function (ABI doc §6).
    fn finish(&mut self, r: Result<(), wasmi::Error>, func: &str) -> Result<(), UserModelError> {
        if let Some(fault) = self.store.data_mut().fault.take() {
            return Err(match fault {
                Fault::Unsupported { import } => UserModelError::Unsupported {
                    model: self.model.clone(),
                    import: import.to_string(),
                },
                Fault::OutOfBounds { import, detail } => UserModelError::OutOfBounds {
                    model: self.model.clone(),
                    func: import.to_string(),
                    detail,
                },
            });
        }
        r.map_err(|e| match e.as_trap_code() {
            Some(TrapCode::OutOfFuel) => UserModelError::FuelExhausted {
                model: self.model.clone(),
                func: func.to_string(),
            },
            Some(TrapCode::GrowthOperationLimited) => UserModelError::MemoryCapExceeded {
                model: self.model.clone(),
                func: func.to_string(),
            },
            _ => UserModelError::Trap {
                model: self.model.clone(),
                func: func.to_string(),
                message: e.to_string(),
            },
        })
    }

    /// Allocate `size` bytes in the guest via `dss_alloc` and range-check the
    /// returned pointer (ABI doc §1: guest-owned allocator; 0 / out-of-range
    /// ⇒ protocol violation).
    fn alloc(&mut self, what: &str, size: usize) -> Result<u32, UserModelError> {
        self.refuel();
        let ptr = match self.dss_alloc.call(&mut self.store, size as i32) {
            Ok(p) => {
                self.finish(Ok(()), "dss_alloc")?;
                p
            }
            Err(e) => return Err(self.finish(Err(e), "dss_alloc").unwrap_err()),
        };
        if ptr == 0 {
            return Err(UserModelError::AllocFailed {
                model: self.model.clone(),
                what: what.to_string(),
            });
        }
        let end = ptr as u32 as u64 + size as u64;
        let mem_bytes = self.memory.data(&self.store).len() as u64;
        if end > mem_bytes {
            return Err(UserModelError::OutOfBounds {
                model: self.model.clone(),
                func: "dss_alloc".to_string(),
                detail: format!(
                    "allocated {what} range {:#x}..{end:#x} exceeds guest memory ({mem_bytes} bytes)",
                    ptr as u32
                ),
            });
        }
        Ok(ptr as u32)
    }

    /// Host-side write into a shuttle buffer.
    fn write(&mut self, func: &str, ptr: u32, bytes: &[u8]) -> Result<(), UserModelError> {
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .map_err(|_| UserModelError::OutOfBounds {
                model: self.model.clone(),
                func: func.to_string(),
                detail: format!(
                    "host write of {} bytes at guest pointer {ptr:#x} is out of bounds",
                    bytes.len()
                ),
            })
    }

    /// Host-side read from a shuttle buffer.
    fn read(&mut self, func: &str, ptr: u32, len: usize) -> Result<Vec<u8>, UserModelError> {
        let mut buf = vec![0u8; len];
        self.memory
            .read(&self.store, ptr as usize, &mut buf)
            .map_err(|_| UserModelError::OutOfBounds {
                model: self.model.clone(),
                func: func.to_string(),
                detail: format!(
                    "host read of {len} bytes at guest pointer {ptr:#x} is out of bounds"
                ),
            })?;
        Ok(buf)
    }

    /// Install the per-call context snapshot and clear the fault slot.
    fn set_context(&mut self, ctx: Box<dyn Callbacks>) {
        let data = self.store.data_mut();
        data.ctx = ctx;
        data.fault = None;
    }

    /// Drain the tier-B effects queued since the last drain, in order.
    fn drain_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.store.data_mut().effects)
    }

    /// Enable the WP-WM.6 deferred DSS-command mechanism
    /// (`do_dss_command`/`get_result_str`) — only a host that can run the
    /// drained commands through the executive should opt in (ABI §4 rows 7/32).
    fn enable_dss_commands(&mut self) {
        self.store.data_mut().dss_commands_enabled = true;
    }

    /// Take the DSS commands queued by `do_dss_command` since the last drain,
    /// in order (Pascal `DSSExecutive.ParseCommand` inputs).
    fn drain_commands(&mut self) -> Vec<String> {
        std::mem::take(&mut self.store.data_mut().pending_commands)
    }

    /// Store the `GlobalResult` served by the next `get_result_str` (Pascal
    /// `GlobalResult`, `DSSCallBackRoutines.pas:451`).
    fn set_result(&mut self, result: String) {
        self.store.data_mut().last_result = result;
    }

    fn usage(&self, detail: impl Into<String>) -> UserModelError {
        UserModelError::Usage {
            model: self.model.clone(),
            detail: detail.into(),
        }
    }
}

/// Fetch a typed export function (signatures were validated at module load,
/// so a mismatch here is unreachable in practice but stays a typed error).
fn typed_func<P, R>(
    model: &str,
    instance: &wasmi::Instance,
    store: &Store<CallData>,
    name: &'static str,
) -> Result<TypedFunc<P, R>, UserModelError>
where
    P: wasmi::WasmParams,
    R: wasmi::WasmResults,
{
    instance
        .get_typed_func::<P, R>(store, name)
        .map_err(|e| UserModelError::SignatureMismatch {
            model: model.to_string(),
            name: name.to_string(),
            detail: e.to_string(),
        })
}

/// Complex slice → the packed LE image (element k at `(k-1)*16`).
fn cplx_image(cs: &[Complex64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(cs.len() * 16);
    for c in cs {
        out.extend_from_slice(&c.re.to_le_bytes());
        out.extend_from_slice(&c.im.to_le_bytes());
    }
    out
}

/// Packed LE image → complex slice.
fn cplx_from_image(bytes: &[u8], out: &mut [Complex64]) {
    for (k, c) in out.iter_mut().enumerate() {
        let off = k * 16;
        let re = f64::from_le_bytes(bytes[off..off + 8].try_into().expect("8-byte slice"));
        let im = f64::from_le_bytes(bytes[off + 8..off + 16].try_into().expect("8-byte slice"));
        *c = Complex64::new(re, im);
    }
}

/// One bound 15/13-function user model (Pascal `TGenUserModel` /
/// `TWindGenUserModel` / `TStoreUserModel` / `TPVsystemUserModel` /
/// `TStoreDynaModel`): a `Store` + instance + the guest buffers allocated once
/// via `dss_alloc` (boundary record / dynarec / V / I / name scratch — ABI doc
/// §2), refreshed in place for every call.
pub struct UserModelInstance {
    sandbox: Sandbox,
    kind: InterfaceKind,
    /// The instance id returned by the guest's `new` (0 = creation failure ⇒
    /// the engine treats the model as absent, Pascal `Get_Exists`).
    id: i32,
    /// The `TGeneratorVars` / `TWindGenVars` buffer, for the kinds that pass
    /// one; `None` for Storage/PVSystem.
    record_ptr: Option<u32>,
    dynarec_ptr: u32,
    v_ptr: u32,
    i_ptr: u32,
    yorder: usize,
    name_ptr: u32,
    edit_ptr: u32,
    edit_cap: usize,
    vars_ptr: u32,
    vars_cap: usize,
    f_select: TypedFunc<i32, i32>,
    f_init: TypedFunc<(i32, i32), ()>,
    f_calc: TypedFunc<(i32, i32), ()>,
    f_integrate: TypedFunc<(), ()>,
    f_save: Option<TypedFunc<(), ()>>,
    f_restore: Option<TypedFunc<(), ()>>,
    f_edit: TypedFunc<(i32, i32), ()>,
    f_update_model: TypedFunc<(), ()>,
    f_delete: TypedFunc<i32, ()>,
    f_num_vars: TypedFunc<(), i32>,
    f_get_all_vars: TypedFunc<i32, ()>,
    f_get_variable: TypedFunc<i32, f64>,
    f_set_variable: TypedFunc<(i32, f64), ()>,
    f_get_var_name: TypedFunc<(i32, i32, i32), ()>,
}

impl std::fmt::Debug for UserModelInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserModelInstance")
            .field("model", &self.sandbox.model)
            .field("kind", &self.kind)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl UserModelInstance {
    /// Instantiate the module and run the guest's `new` (Pascal `Set_Name`
    /// tail, `GenUserModel.pas:196`): allocate the per-instance guest buffers
    /// via `dss_alloc`, write the initial record images, call
    /// `new(genvars?, dynarec)` and read the records back.
    ///
    /// `yorder` fixes the V/I buffer capacity (the Pascal call sites pass
    /// `Yorder`-sized `pComplexArray`s). The shuttle's boundary record must
    /// match the host's kind: a [`Shuttle`] with `gen_vars` for
    /// [`InterfaceKind::GenUserModel`], a [`WindGenShuttle`] for
    /// [`InterfaceKind::WindGenUserModel`], and a record-less
    /// [`Shuttle::without_gen_vars`] for the rest.
    ///
    /// A `new` returning 0 is **not** an error here: the instance reports
    /// [`Self::exists`]` == false` and the engine treats the model as absent
    /// (ABI doc §1).
    pub fn new<'a>(
        host: &UserModelHost,
        yorder: usize,
        sh: impl IntoShuttle<'a>,
    ) -> Result<Self, UserModelError> {
        let mut sh = Parts::split(sh);
        let mut sandbox = Sandbox::new(host)?;
        let kind = host.kind;
        if kind == InterfaceKind::CapUserControl {
            return Err(sandbox.usage(
                "InterfaceKind::CapUserControl requires CapControlInstance, not UserModelInstance",
            ));
        }
        if kind.vars_record() != sh.vars.record() {
            return Err(sandbox.usage(format!(
                "boundary record mismatch: {kind:?} expects {:?}, the shuttle carries {:?}",
                kind.vars_record(),
                sh.vars.record()
            )));
        }

        // Typed exports (signatures validated at load).
        let m = &sandbox.model;
        let inst = &sandbox.instance;
        let st = &sandbox.store;
        let f_select = typed_func(m, inst, st, "select")?;
        let f_init = typed_func(m, inst, st, "init")?;
        let f_calc = typed_func(m, inst, st, "calc")?;
        let f_integrate = typed_func(m, inst, st, "integrate")?;
        let (f_save, f_restore) = if kind.has_save_restore() {
            (
                Some(typed_func(m, inst, st, "save")?),
                Some(typed_func(m, inst, st, "restore")?),
            )
        } else {
            (None, None)
        };
        let f_edit = typed_func(m, inst, st, "edit")?;
        let f_update_model = typed_func(m, inst, st, "update_model")?;
        let f_delete = typed_func(m, inst, st, "delete")?;
        let f_num_vars = typed_func(m, inst, st, "num_vars")?;
        let f_get_all_vars = typed_func(m, inst, st, "get_all_vars")?;
        let f_get_variable = typed_func(m, inst, st, "get_variable")?;
        let f_set_variable = typed_func(m, inst, st, "set_variable")?;
        let f_get_var_name = typed_func(m, inst, st, "get_var_name")?;
        let f_new_record: Option<TypedFunc<(i32, i32), i32>> = if kind.vars_record().is_some() {
            Some(typed_func(m, inst, st, "new")?)
        } else {
            None
        };
        let f_new_dynarec: Option<TypedFunc<i32, i32>> = if kind.vars_record().is_some() {
            None
        } else {
            Some(typed_func(m, inst, st, "new")?)
        };

        // Per-instance guest buffers (ABI doc §2: allocated once, refreshed
        // in place for every call).
        let record_ptr = match kind.vars_record() {
            Some(VarsRecord::Generator) => {
                Some(sandbox.alloc("GeneratorVars record", GeneratorVars::SIZE)?)
            }
            Some(VarsRecord::WindGen) => {
                Some(sandbox.alloc("WindGenVars record", WindGenVars::SIZE)?)
            }
            None => None,
        };
        let dynarec_ptr = sandbox.alloc("TDynamicsRec record", DynamicsRec::SIZE)?;
        let v_ptr = sandbox.alloc("V terminal buffer", yorder.max(1) * 16)?;
        let i_ptr = sandbox.alloc("I terminal buffer", yorder.max(1) * 16)?;
        let name_ptr = sandbox.alloc("name scratch", NAME_MAXLEN + 1)?;

        let mut this = Self {
            sandbox,
            kind,
            id: 0,
            record_ptr,
            dynarec_ptr,
            v_ptr,
            i_ptr,
            yorder,
            name_ptr,
            edit_ptr: 0,
            edit_cap: 0,
            vars_ptr: 0,
            vars_cap: 0,
            f_select,
            f_init,
            f_calc,
            f_integrate,
            f_save,
            f_restore,
            f_edit,
            f_update_model,
            f_delete,
            f_num_vars,
            f_get_all_vars,
            f_get_variable,
            f_set_variable,
            f_get_var_name,
        };

        // `new` with fresh record images (ABI doc §3 Load).
        this.begin(&mut sh)?;
        let r = match (f_new_record, f_new_dynarec) {
            (Some(f), _) => f.call(
                &mut this.sandbox.store,
                (
                    this.record_ptr.expect("record kind has a record buffer") as i32,
                    this.dynarec_ptr as i32,
                ),
            ),
            (_, Some(f)) => f.call(&mut this.sandbox.store, this.dynarec_ptr as i32),
            _ => unreachable!("one new shape per kind"),
        };
        let id = match r {
            Ok(id) => {
                this.sandbox.finish(Ok(()), "new")?;
                id
            }
            Err(e) => return Err(this.sandbox.finish(Err(e), "new").unwrap_err()),
        };
        this.id = id;
        this.read_back(&mut sh)?;
        Ok(this)
    }

    /// The guest instance id (0 ⇒ creation failed).
    pub fn id(&self) -> i32 {
        self.id
    }

    /// Pascal `Get_Exists` (`GenUserModel.pas:113-121`) minus the
    /// auto-select (the engine calls [`Self::select`] explicitly per the §3
    /// call ordering).
    pub fn exists(&self) -> bool {
        self.id != 0
    }

    /// The model attribution.
    pub fn model(&self) -> &str {
        &self.sandbox.model
    }

    /// The interface kind.
    pub fn kind(&self) -> InterfaceKind {
        self.kind
    }

    /// Drain the tier-B effects queued by guest calls since the last drain
    /// (the engine applies them immediately after each call — plan §2.3).
    pub fn drain_effects(&mut self) -> Vec<Effect> {
        self.sandbox.drain_effects()
    }

    /// Opt into the WP-WM.6 deferred DSS-command mechanism
    /// (`do_dss_command`/`get_result_str`, ABI §4 rows 7/32). Only a host that
    /// can run [`Self::drain_dss_commands`] through the executive and feed the
    /// result back via [`Self::set_result_str`] before the next guest call
    /// should enable it. Left disabled, both callbacks raise the loud
    /// [`UserModelError::Unsupported`] (the dss-rs element call sites cannot
    /// re-enter the executive, so they do not opt in — never a silent no-op).
    pub fn enable_dss_commands(&mut self) {
        self.sandbox.enable_dss_commands();
    }

    /// Take the DSS commands the guest queued via `do_dss_command` since the
    /// last drain, in order — the host runs each through the executive after
    /// the guest call returns (Pascal `DoDSSCommandCallBack`, `:150-154`). To
    /// stay Pascal-faithful the host must clear `SolutionAbort` *before* each
    /// `ParseCommand` (the callback does `DSSPrime.SolutionAbort := FALSE;`
    /// then `ParseCommand`, `:152-153`) — that reset is `Dss`-side, outside
    /// this drain API's reach.
    pub fn drain_dss_commands(&mut self) -> Vec<String> {
        self.sandbox.drain_commands()
    }

    /// Store the `GlobalResult` the next `get_result_str` will serve (Pascal
    /// `GetResultStrCallBack`, `:449-452`): the host sets this from the
    /// executive after it runs a drained command, so the guest sees the result
    /// on its *next* call (the ordering note, ABI §4).
    pub fn set_result_str(&mut self, result: impl Into<String>) {
        self.sandbox.set_result(result.into());
    }

    /// Write the records + context in, refuel (start of every call).
    fn begin(&mut self, sh: &mut Parts<'_>) -> Result<(), UserModelError> {
        let ctx = std::mem::replace(&mut sh.ctx, Box::new(NoCallbacks));
        self.sandbox.set_context(ctx);
        if sh.vars.record() != self.kind.vars_record() {
            return Err(self.sandbox.usage(format!(
                "boundary record mismatch: {:?} expects {:?}, the shuttle carries {:?}",
                self.kind,
                self.kind.vars_record(),
                sh.vars.record()
            )));
        }
        if let Some(image) = sh.vars.to_image() {
            let ptr = self
                .record_ptr
                .ok_or_else(|| self.sandbox.usage("no boundary-record buffer allocated"))?;
            self.sandbox.write("record shuttle", ptr, &image)?;
        }
        self.sandbox
            .write("dynarec shuttle", self.dynarec_ptr, &sh.dyn_rec.to_bytes())?;
        self.sandbox.refuel();
        Ok(())
    }

    /// Read the records back (unconditional — ABI doc §2: the model may
    /// mutate the boundary record and DynaVars).
    fn read_back(&mut self, sh: &mut Parts<'_>) -> Result<(), UserModelError> {
        if let Some(size) = match sh.vars.record() {
            Some(VarsRecord::Generator) => Some(GeneratorVars::SIZE),
            Some(VarsRecord::WindGen) => Some(WindGenVars::SIZE),
            None => None,
        } {
            let ptr = self.record_ptr.expect("checked in begin");
            let bytes = self.sandbox.read("record shuttle", ptr, size)?;
            sh.vars.apply(bytes);
        }
        let bytes = self
            .sandbox
            .read("dynarec shuttle", self.dynarec_ptr, DynamicsRec::SIZE)?;
        *sh.dyn_rec = DynamicsRec::from_bytes(&bytes.try_into().expect("sized read"));
        Ok(())
    }

    /// Run one no-argument-style guest call inside the record envelope.
    fn call_env<'a, T>(
        &mut self,
        func: &'static str,
        sh: impl IntoShuttle<'a>,
        run: impl FnOnce(&mut Self) -> Result<T, wasmi::Error>,
    ) -> Result<T, UserModelError> {
        let mut sh = Parts::split(sh);
        self.begin(&mut sh)?;
        let r = run(self);
        let out = match r {
            Ok(v) => {
                self.sandbox.finish(Ok(()), func)?;
                v
            }
            Err(e) => return Err(self.sandbox.finish(Err(e), func).unwrap_err()),
        };
        self.read_back(&mut sh)?;
        Ok(out)
    }

    /// Guest `select(id)` (Pascal `TGenUserModel.Select`,
    /// `GenUserModel.pas:129-132`).
    pub fn select<'a>(&mut self, sh: impl IntoShuttle<'a>) -> Result<i32, UserModelError> {
        let id = self.id;
        self.call_env("select", sh, |s| s.f_select.call(&mut s.sandbox.store, id))
    }

    /// Guest `edit` with the `UserData=`/`DynaData=` string (Pascal
    /// `TGenUserModel.Edit`, `GenUserModel.pas:134-138`: silently ignored
    /// while `FID = 0`).
    pub fn edit<'a>(&mut self, data: &str, sh: impl IntoShuttle<'a>) -> Result<(), UserModelError> {
        if self.id == 0 {
            return Ok(()); // Pascal: "Else Ignore"
        }
        // Grow-only guest edit buffer (guests expose no free).
        if data.len() > self.edit_cap {
            self.edit_ptr = self
                .sandbox
                .alloc("edit string buffer", data.len().max(64))?;
            self.edit_cap = data.len().max(64);
        }
        if !data.is_empty() {
            self.sandbox
                .write("edit shuttle", self.edit_ptr, data.as_bytes())?;
        }
        let (ptr, len) = (self.edit_ptr as i32, data.len() as i32);
        self.call_env("edit", sh, |s| {
            s.f_edit.call(&mut s.sandbox.store, (ptr, len))
        })
    }

    /// Guest `init(V, I)` — dynamics `InitStateVars`
    /// (`generator.pas:2389-2391`). `v`/`i` must be `yorder` long; both are
    /// written into the guest buffers, `i` is read back after.
    pub fn init<'a>(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        sh: impl IntoShuttle<'a>,
    ) -> Result<(), UserModelError> {
        self.check_vi(v.len(), i.len())?;
        self.write_vi(v, i)?;
        let (vp, ip) = (self.v_ptr as i32, self.i_ptr as i32);
        self.call_env("init", sh, |s| {
            s.f_init.call(&mut s.sandbox.store, (vp, ip))
        })?;
        self.read_i(i)
    }

    /// Guest `calc(V, I)` — power flow `DoUserModel`
    /// (`generator.pas:1777-1797`) / dynamics GenModel=6 (`:1900`, `:1999`) /
    /// Storage `DoDynaModel` (`Storage.pas:2206-2229`). The sign convention
    /// (negate into `InjCurrent` / `-DESSCurr` into `ITerminal`) stays at the
    /// engine call site (ABI doc §2.3).
    pub fn calc<'a>(
        &mut self,
        v: &[Complex64],
        i: &mut [Complex64],
        sh: impl IntoShuttle<'a>,
    ) -> Result<(), UserModelError> {
        self.check_vi(v.len(), i.len())?;
        self.write_vi(v, i)?;
        let (vp, ip) = (self.v_ptr as i32, self.i_ptr as i32);
        self.call_env("calc", sh, |s| {
            s.f_calc.call(&mut s.sandbox.store, (vp, ip))
        })?;
        self.read_i(i)
    }

    /// Guest `select(id)` + `integrate()` (Pascal `TGenUserModel.Integrate`,
    /// `GenUserModel.pas:123-127`, reproduced verbatim: the wrapper always
    /// selects first).
    pub fn integrate<'a>(&mut self, sh: impl IntoShuttle<'a>) -> Result<(), UserModelError> {
        let id = self.id;
        self.call_env("integrate", sh, |s| {
            s.f_select.call(&mut s.sandbox.store, id)?;
            s.f_integrate.call(&mut s.sandbox.store, ())
        })
    }

    /// Guest `save()` (15-fn interfaces only).
    pub fn save<'a>(&mut self, sh: impl IntoShuttle<'a>) -> Result<(), UserModelError> {
        let Some(f) = self.f_save else {
            return Err(self
                .sandbox
                .usage("save is not part of the 13-function TStoreDynaModel interface"));
        };
        self.call_env("save", sh, |s| f.call(&mut s.sandbox.store, ()))
    }

    /// Guest `restore()` (15-fn interfaces only).
    pub fn restore<'a>(&mut self, sh: impl IntoShuttle<'a>) -> Result<(), UserModelError> {
        let Some(f) = self.f_restore else {
            return Err(self
                .sandbox
                .usage("restore is not part of the 13-function TStoreDynaModel interface"));
        };
        self.call_env("restore", sh, |s| f.call(&mut s.sandbox.store, ()))
    }

    /// Guest `update_model()` — after `RecalcElementData`
    /// (`generator.pas:1266`).
    pub fn update_model<'a>(&mut self, sh: impl IntoShuttle<'a>) -> Result<(), UserModelError> {
        self.call_env("update_model", sh, |s| {
            s.f_update_model.call(&mut s.sandbox.store, ())
        })
    }

    /// Guest `num_vars()` — the state-variable surface
    /// (`generator.pas:2552-2720`).
    pub fn num_vars<'a>(&mut self, sh: impl IntoShuttle<'a>) -> Result<i32, UserModelError> {
        self.call_env("num_vars", sh, |s| {
            s.f_num_vars.call(&mut s.sandbox.store, ())
        })
    }

    /// Guest `get_all_vars(ptr)`: the guest writes `out.len()` f64 values
    /// (1-based semantics: var k at `ptr + (k-1)*8` — ABI doc §1).
    pub fn get_all_vars<'a>(
        &mut self,
        out: &mut [f64],
        sh: impl IntoShuttle<'a>,
    ) -> Result<(), UserModelError> {
        let need = out.len() * 8;
        if need > self.vars_cap {
            self.vars_ptr = self.sandbox.alloc("vars buffer", need.max(64))?;
            self.vars_cap = need.max(64);
        }
        let ptr = self.vars_ptr as i32;
        self.call_env("get_all_vars", sh, |s| {
            s.f_get_all_vars.call(&mut s.sandbox.store, ptr)
        })?;
        let bytes = self.sandbox.read("get_all_vars", self.vars_ptr, need)?;
        for (k, v) in out.iter_mut().enumerate() {
            *v = f64::from_le_bytes(bytes[k * 8..k * 8 + 8].try_into().expect("8-byte slice"));
        }
        Ok(())
    }

    /// Guest `get_variable(i)` — 1-based user-model variable index (the
    /// engine subtracts the built-in count first, `generator.pas:2560-2580`).
    pub fn get_variable<'a>(
        &mut self,
        i: i32,
        sh: impl IntoShuttle<'a>,
    ) -> Result<f64, UserModelError> {
        self.call_env("get_variable", sh, |s| {
            s.f_get_variable.call(&mut s.sandbox.store, i)
        })
    }

    /// Guest `set_variable(i, value)`.
    pub fn set_variable<'a>(
        &mut self,
        i: i32,
        value: f64,
        sh: impl IntoShuttle<'a>,
    ) -> Result<(), UserModelError> {
        self.call_env("set_variable", sh, |s| {
            s.f_set_variable.call(&mut s.sandbox.store, (i, value))
        })
    }

    /// Guest `get_var_name(i, ptr, maxlen)` → the NUL-terminated name from
    /// the host-owned scratch buffer.
    pub fn get_var_name<'a>(
        &mut self,
        i: i32,
        sh: impl IntoShuttle<'a>,
    ) -> Result<String, UserModelError> {
        let (ptr, maxlen) = (self.name_ptr as i32, NAME_MAXLEN as i32);
        self.call_env("get_var_name", sh, |s| {
            s.f_get_var_name
                .call(&mut s.sandbox.store, (i, ptr, maxlen))
        })?;
        let bytes = self
            .sandbox
            .read("get_var_name", self.name_ptr, NAME_MAXLEN + 1)?;
        let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
        Ok(String::from_utf8_lossy(&bytes[..end]).into_owned())
    }

    /// Guest `delete(id)` (Pascal `TGenUserModel.Destroy`,
    /// `GenUserModel.pas:103-111`: only while `FID <> 0`); the id is cleared.
    pub fn delete<'a>(&mut self, sh: impl IntoShuttle<'a>) -> Result<(), UserModelError> {
        if self.id == 0 {
            return Ok(());
        }
        let id = self.id;
        self.call_env("delete", sh, |s| s.f_delete.call(&mut s.sandbox.store, id))?;
        self.id = 0;
        Ok(())
    }

    fn check_vi(&self, v_len: usize, i_len: usize) -> Result<(), UserModelError> {
        if v_len != self.yorder || i_len != self.yorder {
            return Err(self.sandbox.usage(format!(
                "V/I buffer length mismatch: yorder = {}, got V = {v_len}, I = {i_len}",
                self.yorder
            )));
        }
        Ok(())
    }

    /// Write both terminal arrays in (the native contract shares both
    /// buffers; V is not read back — ABI doc §2.3).
    fn write_vi(&mut self, v: &[Complex64], i: &[Complex64]) -> Result<(), UserModelError> {
        self.sandbox
            .write("V shuttle", self.v_ptr, &cplx_image(v))?;
        self.sandbox.write("I shuttle", self.i_ptr, &cplx_image(i))
    }

    /// Read the current array back out.
    fn read_i(&mut self, i: &mut [Complex64]) -> Result<(), UserModelError> {
        let bytes = self.sandbox.read("I shuttle", self.i_ptr, i.len() * 16)?;
        cplx_from_image(&bytes, i);
        Ok(())
    }
}

/// One bound 7-function CapControl user model (Pascal `TCapUserControl`,
/// `CapUserControl.pas:26-66`): no boundary records — the model reads state
/// via `dss_env` callbacks and schedules via `control_queue_push`.
pub struct CapControlInstance {
    sandbox: Sandbox,
    id: i32,
    edit_ptr: u32,
    edit_cap: usize,
    f_select: TypedFunc<i32, i32>,
    f_sample: TypedFunc<(), ()>,
    f_do_pending: TypedFunc<(i32, i32), ()>,
    f_edit: TypedFunc<(i32, i32), ()>,
    f_update_model: TypedFunc<(), ()>,
    f_delete: TypedFunc<i32, ()>,
}

impl std::fmt::Debug for CapControlInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CapControlInstance")
            .field("model", &self.sandbox.model)
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl CapControlInstance {
    /// Instantiate and run the guest's `new()` (Pascal
    /// `CapUserControl.pas:185`: `FID := FNew(CallBackRoutines)` — the
    /// callback struct pointer has no wasm counterpart, ABI doc §1).
    pub fn new(host: &UserModelHost, ctx: Box<dyn Callbacks>) -> Result<Self, UserModelError> {
        let mut sandbox = Sandbox::new(host)?;
        if host.kind != InterfaceKind::CapUserControl {
            return Err(sandbox.usage(format!(
                "CapControlInstance requires InterfaceKind::CapUserControl, got {:?}",
                host.kind
            )));
        }
        let m = &sandbox.model;
        let inst = &sandbox.instance;
        let st = &sandbox.store;
        let f_new: TypedFunc<(), i32> = typed_func(m, inst, st, "new")?;
        let f_select = typed_func(m, inst, st, "select")?;
        let f_sample = typed_func(m, inst, st, "sample")?;
        let f_do_pending = typed_func(m, inst, st, "do_pending")?;
        let f_edit = typed_func(m, inst, st, "edit")?;
        let f_update_model = typed_func(m, inst, st, "update_model")?;
        let f_delete = typed_func(m, inst, st, "delete")?;

        sandbox.set_context(ctx);
        sandbox.refuel();
        let id = match f_new.call(&mut sandbox.store, ()) {
            Ok(id) => {
                sandbox.finish(Ok(()), "new")?;
                id
            }
            Err(e) => return Err(sandbox.finish(Err(e), "new").unwrap_err()),
        };

        Ok(Self {
            sandbox,
            id,
            edit_ptr: 0,
            edit_cap: 0,
            f_select,
            f_sample,
            f_do_pending,
            f_edit,
            f_update_model,
            f_delete,
        })
    }

    /// The guest instance id (0 ⇒ creation failed, model absent).
    pub fn id(&self) -> i32 {
        self.id
    }

    /// Pascal `Get_Exists` semantics.
    pub fn exists(&self) -> bool {
        self.id != 0
    }

    /// The model attribution.
    pub fn model(&self) -> &str {
        &self.sandbox.model
    }

    /// Drain the tier-B effects (control-queue pushes, messages) queued since
    /// the last drain.
    pub fn drain_effects(&mut self) -> Vec<Effect> {
        self.sandbox.drain_effects()
    }

    /// Opt into the WP-WM.6 deferred DSS-command mechanism (see
    /// [`UserModelInstance::enable_dss_commands`]).
    pub fn enable_dss_commands(&mut self) {
        self.sandbox.enable_dss_commands();
    }

    /// Take the DSS commands the guest queued via `do_dss_command` since the
    /// last drain, in order (see [`UserModelInstance::drain_dss_commands`]).
    pub fn drain_dss_commands(&mut self) -> Vec<String> {
        self.sandbox.drain_commands()
    }

    /// Store the `GlobalResult` the next `get_result_str` will serve (see
    /// [`UserModelInstance::set_result_str`]).
    pub fn set_result_str(&mut self, result: impl Into<String>) {
        self.sandbox.set_result(result.into());
    }

    fn call(
        &mut self,
        func: &'static str,
        ctx: Box<dyn Callbacks>,
        run: impl FnOnce(&mut Self) -> Result<(), wasmi::Error>,
    ) -> Result<(), UserModelError> {
        self.sandbox.set_context(ctx);
        self.sandbox.refuel();
        let r = run(self);
        self.sandbox.finish(r, func)
    }

    /// Guest `select(id)`.
    pub fn select(&mut self, ctx: Box<dyn Callbacks>) -> Result<i32, UserModelError> {
        self.sandbox.set_context(ctx);
        self.sandbox.refuel();
        let id = self.id;
        let r = self.f_select.call(&mut self.sandbox.store, id);
        match r {
            Ok(v) => {
                self.sandbox.finish(Ok(()), "select")?;
                Ok(v)
            }
            Err(e) => Err(self.sandbox.finish(Err(e), "select").unwrap_err()),
        }
    }

    /// Guest `edit` with the `UserData=` string (Pascal
    /// `TCapUserControl.Edit`, `CapUserControl.pas:139-143`: ignored while
    /// `FID = 0`).
    pub fn edit(&mut self, data: &str, ctx: Box<dyn Callbacks>) -> Result<(), UserModelError> {
        if self.id == 0 {
            return Ok(());
        }
        if data.len() > self.edit_cap {
            self.edit_ptr = self
                .sandbox
                .alloc("edit string buffer", data.len().max(64))?;
            self.edit_cap = data.len().max(64);
        }
        if !data.is_empty() {
            self.sandbox
                .write("edit shuttle", self.edit_ptr, data.as_bytes())?;
        }
        let (ptr, len) = (self.edit_ptr as i32, data.len() as i32);
        self.call("edit", ctx, |s| {
            s.f_edit.call(&mut s.sandbox.store, (ptr, len))
        })
    }

    /// Guest `update_model()` — after edit (`CapControl.pas:440`).
    pub fn update_model(&mut self, ctx: Box<dyn Callbacks>) -> Result<(), UserModelError> {
        self.call("update_model", ctx, |s| {
            s.f_update_model.call(&mut s.sandbox.store, ())
        })
    }

    /// Guest `sample()` — the control sample hook (`CapControl.pas:1026-1041`;
    /// the SampleP/V/Curr context arrives through the tier-A snapshot).
    pub fn sample(&mut self, ctx: Box<dyn Callbacks>) -> Result<(), UserModelError> {
        self.call("sample", ctx, |s| s.f_sample.call(&mut s.sandbox.store, ()))
    }

    /// Guest `do_pending(code, proxy_hdl)` — `DoPendingAction`
    /// (`CapControl.pas:730`; value semantics per ABI doc §1).
    pub fn do_pending(
        &mut self,
        code: i32,
        proxy_hdl: i32,
        ctx: Box<dyn Callbacks>,
    ) -> Result<(), UserModelError> {
        self.call("do_pending", ctx, |s| {
            s.f_do_pending.call(&mut s.sandbox.store, (code, proxy_hdl))
        })
    }

    /// Guest `delete(id)` (only while the id is nonzero); the id is cleared.
    pub fn delete(&mut self, ctx: Box<dyn Callbacks>) -> Result<(), UserModelError> {
        if self.id == 0 {
            return Ok(());
        }
        let id = self.id;
        self.call("delete", ctx, |s| s.f_delete.call(&mut s.sandbox.store, id))?;
        self.id = 0;
        Ok(())
    }
}
