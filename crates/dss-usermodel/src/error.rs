//! Typed errors of the WASM user-model host.
//!
//! Failure policy per `docs/wasm/USERMODEL_ABI.md` §6: native-load failure
//! classes keep their Pascal behavior (the engine maps [`UserModelError::
//! MissingExport`] to the 569/1569 "Does Not Have Required Function" path and
//! treats module-load failures as the 570/1570 "Not Loaded" warn-and-fallback),
//! while wasm-only failure classes (trap, protocol violation, fuel exhaustion,
//! memory-cap breach) are hard, loud engine errors naming the model and the
//! function — never a silent fallback.

use std::fmt;

/// Error of the WASM user-model host (`dss-usermodel`).
///
/// Every variant carries the model attribution (`model` = the property value /
/// resolved `.wasm` path the engine loaded) so the engine can surface a
/// message naming the model and the function (ABI doc §6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserModelError {
    /// The bytes are not a valid/compilable WebAssembly module. The engine
    /// maps this to the Pascal load-failure path ("… Not Loaded. …", 570/1570).
    InvalidModule {
        /// Model attribution (path / property value).
        model: String,
        /// wasmi's description of the failure.
        detail: String,
    },
    /// A required export is absent — the wasm analogue of the Pascal missing
    /// `GetProcAddress` (`CheckFuncError`, `GenUserModel.pas:83`): the engine
    /// maps it to `'… Does Not Have Required Function: %s'` (569/1569) and the
    /// model stays absent. `name` is the first missing export in the Pascal
    /// binding order (`GenUserModel.pas:173-187`).
    MissingExport {
        /// Model attribution.
        model: String,
        /// The exact missing export name (wasm spelling, ABI doc §1).
        name: &'static str,
    },
    /// An export exists but its wasm signature does not match the ABI —
    /// a protocol violation (ABI doc §6), hard error.
    SignatureMismatch {
        /// Model attribution.
        model: String,
        /// The offending export.
        name: String,
        /// Expected-vs-found description.
        detail: String,
    },
    /// Module instantiation failed (start trap, resource limits at
    /// instantiation, missing import signature, …).
    Instantiation {
        /// Model attribution.
        model: String,
        /// wasmi's description of the failure.
        detail: String,
    },
    /// `dss_alloc` returned 0 or an unusable pointer — protocol violation
    /// (ABI doc §6).
    AllocFailed {
        /// Model attribution.
        model: String,
        /// What was being allocated.
        what: String,
    },
    /// A record-shuttle / buffer transfer hit out-of-bounds guest memory —
    /// protocol violation (ABI doc §6).
    OutOfBounds {
        /// Model attribution.
        model: String,
        /// The interface function or `dss_env` import involved.
        func: String,
        /// What access failed.
        detail: String,
    },
    /// The guest trapped (unreachable, div-by-zero, OOB access inside the
    /// guest, …) — hard error, never a silent fallback (ABI doc §6).
    Trap {
        /// Model attribution.
        model: String,
        /// The interface function that was executing.
        func: String,
        /// wasmi's trap description.
        message: String,
    },
    /// The per-call fuel budget was exhausted (ABI doc §6).
    FuelExhausted {
        /// Model attribution.
        model: String,
        /// The interface function that was executing.
        func: String,
    },
    /// The guest tried to grow linear memory beyond the store limit
    /// (default 64 MiB, ABI doc §6).
    MemoryCapExceeded {
        /// Model attribution.
        model: String,
        /// The interface function that was executing.
        func: String,
    },
    /// The guest called a `dss_env` import the host does not support:
    /// `get_active_element_ptr` permanently (a raw host pointer has no wasm
    /// meaning), and the WP-WM.6 deferred pair `do_dss_command`/`get_result_str`
    /// when the host has not opted into the deferred-command mechanism via
    /// [`crate::UserModelInstance::enable_dss_commands`] (ABI doc §4 rows
    /// 7/30/32). Loud and attributed — never a silent no-op (plan §2.9-5).
    Unsupported {
        /// Model attribution.
        model: String,
        /// The `dss_env` import name.
        import: String,
    },
    /// Host-side API misuse (engine bug, e.g. wrong record set for the
    /// interface kind, mismatched V/I buffer length) — loud, never silent.
    Usage {
        /// Model attribution.
        model: String,
        /// What was misused.
        detail: String,
    },
}

impl fmt::Display for UserModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidModule { model, detail } => {
                write!(f, "User model \"{model}\": invalid WASM module: {detail}")
            }
            Self::MissingExport { model, name } => {
                write!(
                    f,
                    "User model \"{model}\" Does Not Have Required Function: {name}"
                )
            }
            Self::SignatureMismatch {
                model,
                name,
                detail,
            } => {
                write!(
                    f,
                    "User model \"{model}\": export `{name}` has the wrong signature: {detail}"
                )
            }
            Self::Instantiation { model, detail } => {
                write!(f, "User model \"{model}\": instantiation failed: {detail}")
            }
            Self::AllocFailed { model, what } => {
                write!(
                    f,
                    "User model \"{model}\": dss_alloc failed allocating {what}"
                )
            }
            Self::OutOfBounds {
                model,
                func,
                detail,
            } => {
                write!(
                    f,
                    "User model \"{model}\", function `{func}`: out-of-bounds guest memory access: {detail}"
                )
            }
            Self::Trap {
                model,
                func,
                message,
            } => {
                write!(
                    f,
                    "User model \"{model}\", function `{func}`: trapped: {message}"
                )
            }
            Self::FuelExhausted { model, func } => {
                write!(
                    f,
                    "User model \"{model}\", function `{func}`: fuel budget exhausted"
                )
            }
            Self::MemoryCapExceeded { model, func } => {
                write!(
                    f,
                    "User model \"{model}\", function `{func}`: linear-memory cap exceeded"
                )
            }
            Self::Unsupported { model, import } => {
                write!(
                    f,
                    "User model \"{model}\": callback `{import}` is not supported over WASM"
                )
            }
            Self::Usage { model, detail } => {
                write!(f, "User model \"{model}\": host API misuse: {detail}")
            }
        }
    }
}

impl std::error::Error for UserModelError {}
