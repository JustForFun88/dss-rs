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
//! the short alias points at.
//!
//! **Flip state (F.3, one kernel family per commit):** `round_i32` — flipped,
//! the default lane saturates; `PI` — still parity-selected in both lanes (its
//! flip moves RPN-computed deck values and lands with its own family).

#[cfg(test)]
mod tests;

/// Which lane this crate was compiled in — `true` under `oracle-parity`.
///
/// The engine never reads it; it exists so the tests can state a *per-lane*
/// expectation without repeating the cfg, and so a Cargo slip that stopped the
/// feature from reaching this crate fails loudly (see
/// `tests::round_alias_is_the_lane_kernel`) instead of silently reverting the
/// parity lane to the default kernel.
#[cfg(feature = "oracle-parity")]
pub const ORACLE_PARITY: bool = true;

/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub const ORACLE_PARITY: bool = false;

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
/// The integer-indefinite path (`inf`/`nan`/overflow → wrapped `i64::MIN`, e.g.
/// `"inf"` → 0) is an FPC/x86 implementation artifact, verified against the
/// oracle with `probe_val.py`. It is the **parity** kernel of the round row:
/// selected under `oracle-parity`, where the deck language must convert exactly
/// like the oracle; the default lane saturates instead
/// ([`round_i32_saturating_impl`]).
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
/// `tests` and, at the deck-language boundary it is observable from, by
/// `parser::tests::make_integer_rounds_ties_to_even`.
pub fn round_i32_saturating_impl(x: f64) -> i32 {
    x.round_ties_even() as i32
}

#[cfg(feature = "oracle-parity")]
pub use round_i32_fpc_impl as round_i32;
// F.3: the default lane converts `1e10`/`inf` to the nearest representable
// integer instead of reproducing FPC's wrapped integer-indefinite sentinel.
#[cfg(not(feature = "oracle-parity"))]
pub use round_i32_saturating_impl as round_i32;
