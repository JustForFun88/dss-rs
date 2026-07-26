//! Stage F lane split — the `oracle-parity` compat kernels of `dss-parser`
//! (`DE_PASCALIZE_PLAN.md` Part IV.2).
//!
//! **The only file in `dss-parser` allowed to contain the
//! `#[cfg(feature = "oracle-parity")]` string** (enforced by `dss-core`'s
//! `tests/oracle_parity_cfg_gate.rs`). See `dss-core/src/compat.rs` for the
//! lane model and the full inventory; two of its rows — the truncated RPN pi
//! and FPC `Round` — live here because that is where the kernels are (the plan
//! table names the rows, not the crates).
//!
//! Both implementations are always compiled; the cfg selects only which one
//! the short alias points at. **F.1 staging: every alias below points at the
//! parity impl in both lanes**, so this commit is bit-neutral; F.3 flips the
//! `not(oracle-parity)` arms.

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// RPN pi (IV.2 row 4)
// ---------------------------------------------------------------------------

/// The truncated pi of the Pascal original (`RPN.pas`, `fff.pas:69`) — **not**
/// the full-precision constant. Results differ in the last ~3 digits (e.g.
/// `"30 sin"` gives `0.5000000000000299`) and the golden parser tests pin that
/// behavior. `EnterPi`, by contrast, pushes FPC's full-precision `pi` builtin.
///
/// TODO(compat): truncated pi reproduced from the Pascal original; the default
/// lane uses [`PI_STD_IMPL`] once F.3 flips the alias (goldens updated there).
#[allow(clippy::approx_constant)]
pub const PI_FPC_TRUNCATED_IMPL: f64 = 3.14159265359;

/// Full-precision pi — `std::f64::consts::PI`.
pub const PI_STD_IMPL: f64 = std::f64::consts::PI;

#[cfg(feature = "oracle-parity")]
pub use PI_FPC_TRUNCATED_IMPL as PI;
// F.1 staging: the default lane still selects the parity constant; F.3 flips
// this to `PI_STD_IMPL`.
#[cfg(not(feature = "oracle-parity"))]
pub use PI_FPC_TRUNCATED_IMPL as PI;

// ---------------------------------------------------------------------------
// FPC `Round` (IV.2 row 5)
// ---------------------------------------------------------------------------

/// FPC `Round`: round-to-nearest-even to Int64 (x87/SSE default mode; out of
/// range and non-finite give the "integer indefinite" `i64::MIN`), then
/// truncated to i32 like the Pascal `Integer := Round(...)` assignment.
///
/// TODO(compat): the integer-indefinite path (`inf`/`nan`/overflow → wrapped
/// `i64::MIN`, e.g. "inf" → 0) reproduces an FPC/x86 implementation artifact
/// verified via probe_val.py; make it a proper error once the 1:1 port is
/// complete.
pub fn round_i32_fpc_impl(x: f64) -> i32 {
    let r = x.round_ties_even();
    let wide = if r >= -(2f64.powi(63)) && r < 2f64.powi(63) {
        r as i64 // r is finite here: NaN comparisons are false
    } else {
        i64::MIN
    };
    wide as i32
}

/// Idiomatic rounding: ties-to-even followed by Rust's **saturating** float →
/// int cast. Identical to [`round_i32_fpc_impl`] on every in-range finite
/// value; out-of-range and infinite inputs saturate to `i32::MIN`/`i32::MAX`
/// instead of wrapping the FPC integer-indefinite sentinel to `0` (NaN maps to
/// `0` in both). The difference is a **deliberate divergence**, pinned by
/// `tests`.
pub fn round_i32_saturating_impl(x: f64) -> i32 {
    x.round_ties_even() as i32
}

#[cfg(feature = "oracle-parity")]
pub use round_i32_fpc_impl as round_i32;
// F.1 staging: see the note at `PI` — F.3 flips this to
// `round_i32_saturating_impl`.
#[cfg(not(feature = "oracle-parity"))]
pub use round_i32_fpc_impl as round_i32;
