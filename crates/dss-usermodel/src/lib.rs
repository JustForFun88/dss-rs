#![forbid(unsafe_code)]
//! `dss-usermodel` — the WASM user-model host: the sandboxed replacement for
//! the upstream user-written-DLL mechanism (`WASM_USERMODELS_PLAN.md`).
//!
//! Upstream loads user models as native DLLs
//! (`LoadLibrary`/`GetProcAddress`, Stdcall — `PCElements/GenUserModel.pas`,
//! `PCElements/StoreUserModel.pas`, `PCElements/PVSystemUserModel.pas`,
//! `Controls/CapUserControl.pas`), which is permanently impossible under
//! `#![forbid(unsafe_code)]`. This crate replaces the *transport* (native DLL
//! → sandboxed WebAssembly over pure-Rust **wasmi**, pinned in
//! `tools/wasm_usermodel/PIN.txt`) while keeping the *contract* 1:1: the same
//! 15/13/7-function interface shapes, the same call ordering, the same
//! shared-record semantics via an explicit copy-in/copy-out shuttle, and the
//! same warn-and-fallback failure behavior. The wire contract is frozen in
//! **`docs/wasm/USERMODEL_ABI.md`**; the implementation template is the
//! vendored typst plugin host
//! (`.inputs/typst/crates/typst-library/src/foundations/plugin.rs`), cited at
//! each ported pattern like Pascal is cited elsewhere.
//!
//! Crate shape (the `dss-sparse` pattern — a pure-Rust dependency behind a
//! Pascal-shaped surface; leaf crate, no dss-core dependency):
//!
//! - [`UserModelHost`] — engine + linker + compiled module, one per loaded
//!   `.wasm` path; load-time export-set validation returning the exact
//!   missing export name for the Pascal 569/1569 path.
//! - [`UserModelInstance`] / [`CapControlInstance`] — one per element
//!   binding: guest `dss_alloc` buffers + the record shuttle over the
//!   ABI-doc offsets ([`GeneratorVars`], [`WindGenVars`], [`DynamicsRec`]),
//!   carried by [`Shuttle`] or [`WindGenShuttle`] via [`IntoShuttle`].
//! - [`Callbacks`] + [`Effect`] — the tiered host-services surface
//!   (`dss_env` imports): tier-A context snapshot, tier-B effect queue,
//!   tier-C owned AuxParser.
//! - [`UserModelError`] — the typed failure contract of ABI doc §6 (traps,
//!   fuel, memory cap, protocol violations: hard and loud, never a silent
//!   fallback).
//!
//! Sandbox determinism (ABI doc §6): relaxed SIMD off, fuel metering on, a
//! linear-memory cap, no WASI, no clock/random/filesystem — module `dss_env`
//! is the entire import surface; the guest is a pure function of its inputs,
//! which is what makes golden-gating sound.

mod callbacks;
mod error;
mod host;
mod imports;
mod instance;
mod records;

pub use callbacks::{Callbacks, Effect, NoCallbacks};
pub use error::UserModelError;
pub use host::{HostConfig, InterfaceKind, UserModelHost};
pub use instance::{
    CapControlInstance, IntoShuttle, Shuttle, ShuttleVars, UserModelInstance, WindGenShuttle,
};
pub use records::{CapControlVars, DynamicsRec, GeneratorVars, WindGenVars};
