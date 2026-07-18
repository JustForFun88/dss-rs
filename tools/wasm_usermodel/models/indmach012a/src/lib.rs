//! # indmach012a — WASM user-model reference fixture (WP-WM.2)
//!
//! A loop-for-loop port of the canonical OpenDSS example user-model DLL
//! (`.inputs/electricdss-code-r3723-trunk/Version8/Source/IndMach012a/`:
//! `IndMach012a.dpr` + `MainUnit.pas` + `IndMach012Model.pas`, r3723) to a
//! `wasm32-unknown-unknown` guest speaking the frozen ABI of
//! `docs/wasm/USERMODEL_ABI.md` (15-function Generator interface + `memory` +
//! `dss_alloc`; host imports from module `dss_env` — the fixture uses only
//! `msg_callback`, per the WM.0 P3 census).
//!
//! The committed artifact `tests/fixtures/wasm/indmach012a.wasm` is a
//! golden-class binary (plan §2.6): rebuilt MANUALLY ONLY via
//! `tools/wasm_usermodel/build_wasm.ps1` with the toolchain pinned in
//! `tools/wasm_usermodel/PIN.txt`. Its numeric spec is the *native twin* —
//! the same vendored Pascal compiled by FPC (`build_native.ps1`, plan A) —
//! probed via `tools/wasm_usermodel/twin_probe.py`; expected values live in
//! `docs/wasm/probes/p6_twin_expected.txt` and are pinned bit-exact by
//! `tests/twin_parity.rs` (host target) and, post-WM.1 integration, through
//! the `dss-usermodel` crate API (WM.2 item 4).
//!
//! ## Unsafe surface
//!
//! This is a workspace-excluded fixture/tooling crate (outside the product
//! `forbid(unsafe_code)` scope, plan §2.6); it still keeps `unsafe` to the
//! irreducible minimum: exactly one `unsafe` expression — the `dss_env`
//! `msg_callback` host-import call (`wasm_exports.rs`). Everything else,
//! including all pointer traffic, resolves through a safe allocation
//! registry (host pointers are `dss_alloc` results by ABI contract §2).
//!
//! ## Determinism
//!
//! The model uses only f64 `+ - * /` and `sqrt` (all IEEE-exact in wasm and
//! on x86-64 SSE2), so the guest is expected bit-identical to the FPC-built
//! native twin; `twin_parity.rs` asserts exactly that.

pub mod cmath;
pub mod mainunit;
pub mod model;
pub mod parser;
pub mod records;
pub mod symcomp;

mod wasm_exports;
