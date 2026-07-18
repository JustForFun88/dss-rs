//! [`UserModelHost`]: engine + linker + compiled module, one per loaded
//! `.wasm` path (plan §2.1), with load-time export-set validation.
//!
//! Template: the vendored typst plugin host (`.inputs/typst/crates/
//! typst-library/src/foundations/plugin.rs`) — `Plugin::new` (`:268-308`)
//! builds the deterministic engine config (relaxed SIMD off, `:271-272`),
//! compiles the module, checks the `memory` export (`:279-281`) and registers
//! the host-import module on a `Linker` (`:284-297`). On top of the typst
//! pattern this host adds fuel metering and a store memory cap (plan §2.1 —
//! typst has neither).

use wasmi::{ExternType, ValType};

use crate::callbacks::CallData;
use crate::error::UserModelError;
use crate::imports;

const I32: ValType = ValType::I32;
const F64: ValType = ValType::F64;

/// Which Pascal loader class the module stands in for — decides the required
/// export set and the `new` shape (ABI doc §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceKind {
    /// Pascal `TGenUserModel` (`GenUserModel.pas`): Generator
    /// `UserModel=`/`ShaftModel=`; 15 functions,
    /// `new(genvars, dynarec) -> id`.
    GenUserModel,
    /// Pascal `TStoreUserModel` (`StoreUserModel.pas:77`): Storage
    /// `UserModel=`; 15 functions, `new(dynarec) -> id`.
    StoreUserModel,
    /// Pascal `TPVsystemUserModel` (`PVSystemUserModel.pas`): PVSystem
    /// `UserModel=`; 15 functions, `new(dynarec) -> id`.
    PvSystemUserModel,
    /// Pascal `TStoreDynaModel` (`StoreUserModel.pas:18-62`): Storage
    /// `DynaDLL=`; 13 functions (no `save`/`restore`),
    /// `new(dynarec) -> id`.
    StoreDynaModel,
    /// Pascal `TCapUserControl` (`CapUserControl.pas`): CapControl
    /// `UserModel=`; 7 functions, `new() -> id`.
    CapUserControl,
}

/// A required guest export: name + exact wasm signature (ABI doc §1).
pub(crate) struct FuncSpec {
    pub(crate) name: &'static str,
    pub(crate) params: &'static [ValType],
    pub(crate) results: &'static [ValType],
}

const fn spec(
    name: &'static str,
    params: &'static [ValType],
    results: &'static [ValType],
) -> FuncSpec {
    FuncSpec {
        name,
        params,
        results,
    }
}

/// The shared 15-function tail in the **Pascal binding order**
/// (`GenUserModel.pas:174-187` — `Select` through `GetVarName`; `new` is
/// bound first and differs per kind, so it is listed separately).
const TAIL_15: &[FuncSpec] = &[
    spec("select", &[I32], &[I32]),
    spec("init", &[I32, I32], &[]),
    spec("calc", &[I32, I32], &[]),
    spec("integrate", &[], &[]),
    spec("save", &[], &[]),
    spec("restore", &[], &[]),
    spec("edit", &[I32, I32], &[]),
    spec("update_model", &[], &[]),
    spec("delete", &[I32], &[]),
    spec("num_vars", &[], &[I32]),
    spec("get_all_vars", &[I32], &[]),
    spec("get_variable", &[I32], &[F64]),
    spec("set_variable", &[I32, F64], &[]),
    spec("get_var_name", &[I32, I32, I32], &[]),
];

/// `TStoreDynaModel` order (`StoreUserModel.pas:337-348`) — no
/// `save`/`restore`.
const TAIL_13: &[FuncSpec] = &[
    spec("select", &[I32], &[I32]),
    spec("init", &[I32, I32], &[]),
    spec("calc", &[I32, I32], &[]),
    spec("integrate", &[], &[]),
    spec("edit", &[I32, I32], &[]),
    spec("update_model", &[], &[]),
    spec("delete", &[I32], &[]),
    spec("num_vars", &[], &[I32]),
    spec("get_all_vars", &[I32], &[]),
    spec("get_variable", &[I32], &[F64]),
    spec("set_variable", &[I32, F64], &[]),
    spec("get_var_name", &[I32, I32, I32], &[]),
];

/// `TCapUserControl` order (`CapUserControl.pas:177-182`).
const TAIL_7: &[FuncSpec] = &[
    spec("select", &[I32], &[I32]),
    spec("sample", &[], &[]),
    spec("do_pending", &[I32, I32], &[]),
    spec("edit", &[I32, I32], &[]),
    spec("update_model", &[], &[]),
    spec("delete", &[I32], &[]),
];

/// The per-kind `new` shapes (ABI doc §1).
const NEW_GEN: FuncSpec = spec("new", &[I32, I32], &[I32]);
const NEW_DYNAREC: FuncSpec = spec("new", &[I32], &[I32]);
const NEW_CAP: FuncSpec = spec("new", &[], &[I32]);

/// The guest allocator (ABI doc §1, common infrastructure).
const DSS_ALLOC: FuncSpec = spec("dss_alloc", &[I32], &[I32]);

impl InterfaceKind {
    /// `new` spec for this kind.
    pub(crate) fn new_spec(self) -> &'static FuncSpec {
        match self {
            Self::GenUserModel => &NEW_GEN,
            Self::StoreUserModel | Self::PvSystemUserModel | Self::StoreDynaModel => &NEW_DYNAREC,
            Self::CapUserControl => &NEW_CAP,
        }
    }

    /// The remaining required functions, in the Pascal binding order (drives
    /// which missing export the 569-path names first).
    pub(crate) fn tail_specs(self) -> &'static [FuncSpec] {
        match self {
            Self::GenUserModel | Self::StoreUserModel | Self::PvSystemUserModel => TAIL_15,
            Self::StoreDynaModel => TAIL_13,
            Self::CapUserControl => TAIL_7,
        }
    }

    /// Whether `new` receives the GeneratorVars buffer pointer.
    pub(crate) fn takes_gen_vars(self) -> bool {
        matches!(self, Self::GenUserModel)
    }

    /// Whether the interface has `save`/`restore` (15-fn only).
    pub(crate) fn has_save_restore(self) -> bool {
        matches!(
            self,
            Self::GenUserModel | Self::StoreUserModel | Self::PvSystemUserModel
        )
    }
}

/// Sandbox limits (plan §2.1 — additions over the typst template).
#[derive(Debug, Clone, Copy)]
pub struct HostConfig {
    /// Per-call fuel budget. Exhaustion is the loud typed
    /// [`UserModelError::FuelExhausted`] naming the model and function.
    ///
    /// The default is deliberately generous; WP-WM.2 verifies the plan's
    /// calibration bar against the committed IndMach012a fixture (a
    /// reference-model call must use <1% of the budget) and adjusts by a
    /// recorded decision if needed.
    pub fuel_per_call: u64,
    /// Linear-memory cap enforced by the store limiter (ABI doc §6;
    /// default 64 MiB). Breach is the loud typed
    /// [`UserModelError::MemoryCapExceeded`].
    pub memory_cap_bytes: usize,
}

impl HostConfig {
    /// Plan §2.1 defaults: generous fuel, 64 MiB memory cap.
    pub const DEFAULT_FUEL_PER_CALL: u64 = 100_000_000;
    /// 64 MiB.
    pub const DEFAULT_MEMORY_CAP: usize = 64 * 1024 * 1024;
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            fuel_per_call: Self::DEFAULT_FUEL_PER_CALL,
            memory_cap_bytes: Self::DEFAULT_MEMORY_CAP,
        }
    }
}

/// Engine + linker + compiled module — one per loaded `.wasm` path
/// (plan §2.1); instances are created per element binding via
/// [`crate::UserModelInstance::new`] / [`crate::CapControlInstance::new`].
pub struct UserModelHost {
    pub(crate) engine: wasmi::Engine,
    pub(crate) module: wasmi::Module,
    pub(crate) linker: wasmi::Linker<CallData>,
    pub(crate) kind: InterfaceKind,
    pub(crate) config: HostConfig,
    pub(crate) model: String,
}

impl std::fmt::Debug for UserModelHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserModelHost")
            .field("model", &self.model)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl UserModelHost {
    /// Compile `wasm_bytes` and validate the export set for `kind`.
    ///
    /// Deterministic sandbox per plan §2.1 / ABI doc §6: relaxed SIMD off
    /// (typst `plugin.rs:271-272`), fuel metering on, store memory cap; the
    /// only import surface is the `dss_env` module. `model` is the
    /// attribution used in every error (the property value / resolved path).
    ///
    /// Errors: [`UserModelError::InvalidModule`] (the engine's 570/1570
    /// "Not Loaded" path), [`UserModelError::MissingExport`] (the 569/1569
    /// path, first missing name in the Pascal binding order), or
    /// [`UserModelError::SignatureMismatch`] (protocol violation, hard).
    pub fn load(
        model: &str,
        wasm_bytes: &[u8],
        kind: InterfaceKind,
        config: HostConfig,
    ) -> Result<Self, UserModelError> {
        let mut wasmi_config = wasmi::Config::default();
        // Disable relaxed SIMD: it can introduce non-determinism
        // (typst plugin.rs:271-272; ABI doc §6 determinism policy).
        wasmi_config.wasm_relaxed_simd(false);
        // Per-call fuel budget (plan §2.1; beyond the typst template).
        wasmi_config.consume_fuel(true);

        let engine = wasmi::Engine::new(&wasmi_config);
        let module =
            wasmi::Module::new(&engine, wasm_bytes).map_err(|e| UserModelError::InvalidModule {
                model: model.to_string(),
                detail: e.to_string(),
            })?;

        validate_exports(model, &module, kind)?;

        let mut linker = wasmi::Linker::new(&engine);
        imports::register_all(&mut linker);

        Ok(Self {
            engine,
            module,
            linker,
            kind,
            config,
            model: model.to_string(),
        })
    }

    /// The interface kind this host validates against.
    pub fn kind(&self) -> InterfaceKind {
        self.kind
    }

    /// The model attribution (property value / resolved path).
    pub fn model(&self) -> &str {
        &self.model
    }

    /// The sandbox limits.
    pub fn config(&self) -> HostConfig {
        self.config
    }
}

/// Export-set validation (ABI doc §1): `memory`, `dss_alloc`, then the
/// interface functions in the Pascal binding order — the first missing name
/// is reported exactly (the 569/1569 path). A present export with the wrong
/// type/signature is a protocol violation (ABI doc §6).
fn validate_exports(
    model: &str,
    module: &wasmi::Module,
    kind: InterfaceKind,
) -> Result<(), UserModelError> {
    // `memory` (typst plugin.rs:279-281).
    match module.get_export("memory") {
        Some(ExternType::Memory(_)) => {}
        Some(other) => {
            return Err(UserModelError::SignatureMismatch {
                model: model.to_string(),
                name: "memory".to_string(),
                detail: format!("expected a linear memory export, found {other:?}"),
            });
        }
        None => {
            return Err(UserModelError::MissingExport {
                model: model.to_string(),
                name: "memory",
            });
        }
    }
    check_func(model, module, &DSS_ALLOC)?;
    check_func(model, module, kind.new_spec())?;
    for s in kind.tail_specs() {
        check_func(model, module, s)?;
    }
    Ok(())
}

/// Presence + exact-signature check of one required function export.
fn check_func(model: &str, module: &wasmi::Module, spec: &FuncSpec) -> Result<(), UserModelError> {
    match module.get_export(spec.name) {
        Some(ExternType::Func(ty)) => {
            if ty.params() != spec.params || ty.results() != spec.results {
                return Err(UserModelError::SignatureMismatch {
                    model: model.to_string(),
                    name: spec.name.to_string(),
                    detail: format!(
                        "expected fn({:?}) -> {:?}, found fn({:?}) -> {:?}",
                        spec.params,
                        spec.results,
                        ty.params(),
                        ty.results()
                    ),
                });
            }
            Ok(())
        }
        Some(other) => Err(UserModelError::SignatureMismatch {
            model: model.to_string(),
            name: spec.name.to_string(),
            detail: format!("expected a function export, found {other:?}"),
        }),
        None => Err(UserModelError::MissingExport {
            model: model.to_string(),
            name: spec.name,
        }),
    }
}
