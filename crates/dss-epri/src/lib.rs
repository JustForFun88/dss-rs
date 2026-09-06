//! # `dss-epri` — the test-only EPRI r4133 bridge (unsafe carve-out)
//!
//! This crate is the **single exception** to the workspace-wide
//! `#![forbid(unsafe_code)]` rule (see `PORTING_PLAN.md` and `CLAUDE.md`). Every
//! *product* crate (`dss-core`, `dss-parser`, `dss-sparse`, `dss-cli`,
//! `dss-metis`) stays `#![forbid(unsafe_code)]` — the shipped engine is pure safe
//! Rust with no C bindings, ever. `dss-epri` is **not a product crate**: it is
//! `publish = false`, built only for the test gate, and its sole job is to drive
//! the official EPRI `OpenDSSDirect.dll` (revision r4133) as a *second live
//! oracle* alongside the pinned dss-python engine
//! (`UNIFIED_GATE_PLAN.md` §2, R1). Reading a foreign C DLL is inherently
//! `unsafe`, so this crate carries, in place of `forbid`:
//!
//! - `#![deny(unsafe_op_in_unsafe_fn)]` — every `unsafe` operation is written
//!   inside an explicit `unsafe {}` block even within `unsafe fn`s, so each one
//!   is individually visible and reviewable.
//! - `#[cfg(windows)]` gating — the vendored EPRI binary is a Win64 DLL; on any
//!   other platform this crate compiles to an empty shell (the r4133 channel is
//!   simply unavailable there). [`guard`] is the one exception: it loads no DLL
//!   and calls only `std::fs`, and the gate's own corpus guard
//!   (`crates/dss-core/tests/corpus_gate/runner.rs`) reports from its
//!   classification, so gating it would force a second copy of that rule to
//!   exist for the other platforms — it is ungated instead (coordinator
//!   decision D33(3)).
//! - Module-level `// SAFETY` documentation on every FFI boundary (see
//!   [`ffi`]) stating the invariant that makes the call sound.
//!
//! Why a Rust FFI worker at all (vs. keeping the Python/Oddie bridge): the r4133
//! DDLL surface is pure `cdecl` with C-plain types (`longint`/`double`/
//! `PAnsiChar`/pointer+type+size out-params — no OLE Variants, no COM), so Rust
//! reads it natively. This drops the entire dss-extensions stack (two beta
//! wheels) and the ~1.5–2.5 s Python startup per call, and unifies the harness
//! language. The bridge was fidelity-proven by bit-for-bit cross-validation
//! against the outgoing Python/Oddie path over the full corpus universe
//! (`tools/opendss/xcheck_bridge.py`) before that Python stack — the
//! cross-check script included — was retired in UNIFIED_GATE Phase E.
//!
//! ## Layout
//! - [`ffi`] — raw `extern "C"` signatures + `libloading` load of the DLL with
//!   an altered search path so its sibling `KLUSolve.dll` resolves.
//! - [`dss`] — a safe-ish wrapper: command execution, the V-protocol decode
//!   (type tags 1=int / 2=double / 3=complex / 4=string / 5=bytes), error
//!   polling (`ErrorCode`/`ErrorDesc` with the tolerated errno sets).
//! - [`capture`] — `CaseResult` assembly, byte-for-byte mirroring
//!   `tools/oracle/oracle_server.py` + `tools/golden/gen_checkpoints.py`.
//! - [`guard`] — a Rust port of `tools/oracle/corpus_guard.py` (recursive
//!   snapshot / restore of the case directory, and the ONE created-entry
//!   classification all three of the gate's producers report from). Pure
//!   `std::fs`, hence ungated.
//! - [`script`] — the generic `exec`/`read`/`chdir` scripting surface for the
//!   manual regen drivers + probes (functional parity with the retired Oddie
//!   bridge's ad-hoc `Text.Command` + property reads; never used by the gate).
//! - [`modes`] — mode-capability classification for the grouped DDLL API: the
//!   unknown-mode sentinels of the four ABI shapes, and the do-not-call register
//!   of modes whose r4133 implementation is memory-unsafe (WP-G1 rails, G1.0).
//!
//! The `epri-worker` binary (`src/bin/epri-worker.rs`) drives all of the above
//! over the persistent line-JSON `ping`/`run`/`quit` protocol, byte-compatible
//! with today's `oracle_server.py` responses.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
pub mod capture;
#[cfg(windows)]
pub mod dss;
#[cfg(windows)]
pub mod families;
#[cfg(windows)]
pub mod ffi;
pub mod guard;
#[cfg(windows)]
pub mod modes;
#[cfg(windows)]
pub mod script;
#[cfg(windows)]
pub mod smoke;

#[cfg(windows)]
pub use capture::{CaseResult, RunRequest, run_case};
#[cfg(windows)]
pub use dss::{Engine, EngineError};
#[cfg(windows)]
pub use smoke::{SmokeReport, run_smoke};
