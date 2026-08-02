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
//! the default lane saturates (F.3a); `PI` — flipped, the default lane converts
//! RPN degrees with `f64::consts::PI` (F.3d).

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

/// The shortened pi of the Pascal original (`RPN.pas:69-70`, `TRPNCalc`'s
/// `DegToRad`/`RadToDeg` constants) — **not** the full-precision constant. It
/// sits 2.069e-13 *above* `std::f64::consts::PI`. It scales the degree
/// conversions of the RPN calculator's trig entries, so results differ in the
/// last ~3 digits (`"30 sin"` gives `0.5000000000000299` instead of
/// `0.49999999999999994`). The parity lane keeps it so the deck language
/// evaluates bit-identically to the pinned oracle.
///
/// `EnterPi` (the `pi` token) is *not* affected in either lane: it pushes FPC's
/// full-precision `pi` builtin, which upstream itself keeps exact.
#[allow(clippy::approx_constant)]
pub const PI_FPC_TRUNCATED_IMPL: f64 = 3.14159265359;

/// Full-precision pi — `std::f64::consts::PI`. The default lane's kernel: a
/// **deliberate divergence** of 6.59e-14 relative, pinned by expected-value
/// tests here and at the deck-language boundary
/// (`parser::tests::rpn_degree_trig_is_the_lane_kernel`).
pub const PI_STD_IMPL: f64 = std::f64::consts::PI;

#[cfg(feature = "oracle-parity")]
pub use PI_FPC_TRUNCATED_IMPL as PI;
// F.3d: the default lane converts degrees with the real pi. No golden and no
// gated corpus deck contains an RPN degree-trig expression (measured), so the
// flip moves no oracle-compared number — its observability is the deck
// expression the pins below evaluate.
#[cfg(not(feature = "oracle-parity"))]
pub use PI_STD_IMPL as PI;

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
    round_i64_fpc(x) as i32
}

/// FPC `Round` proper: ties-to-even into an `Int64`, with the x87/SSE "integer
/// indefinite" sentinel (`i64::MIN`) for anything out of range or non-finite.
///
/// The two FPC-lane kernels of the round row differ only in what the Pascal
/// assignment target is: an `Integer` truncates this ([`round_i32_fpc_impl`]),
/// a `Double` widens it back ([`round_f64_fpc_impl`]).
fn round_i64_fpc(x: f64) -> i64 {
    let r = x.round_ties_even();
    if r >= -(2f64.powi(63)) && r < 2f64.powi(63) {
        r as i64 // r is finite here: NaN comparisons are false
    } else {
        i64::MIN
    }
}

/// FPC `Round` written **back into a Double** — Pascal's
/// `doubles[i] := Round(doubles[i])` (`DSSObjectHelper.pas`, the
/// `TPropertyFlag.ApplyRound` array path). `Round` yields an `Int64`, so an
/// out-of-range magnitude does not stay put: it becomes the integer-indefinite
/// sentinel widened back to floating point. Probed on the pinned oracle —
/// `new growthshape.g npts=2 year=[1e20,2]` then `? growthshape.g.year` gives
/// `[ -9.22337203685478E18 2]`, which is exactly `i64::MIN as f64`.
///
/// It is the **parity** kernel; the default lane rounds in place
/// ([`round_f64_native_impl`]).
pub fn round_f64_fpc_impl(x: f64) -> f64 {
    round_i64_fpc(x) as f64
}

/// Idiomatic rounding in place: ties-to-even, magnitude preserved. Identical to
/// [`round_f64_fpc_impl`] on every value an `Int64` can hold; a larger or
/// non-finite one keeps its own value instead of collapsing to the FPC
/// sentinel. The difference is a **deliberate divergence**, pinned by `tests`
/// and, at the deck-language boundary it is observable from, by
/// `dss_core::exec::tests::compat_quirks::apply_round_out_of_range_is_the_lane_kernel`.
pub fn round_f64_native_impl(x: f64) -> f64 {
    x.round_ties_even()
}

#[cfg(feature = "oracle-parity")]
pub use round_f64_fpc_impl as round_f64;
#[cfg(not(feature = "oracle-parity"))]
pub use round_f64_native_impl as round_f64;

/// Idiomatic rounding: ties-to-even followed by Rust's **saturating** float →
/// int cast. Identical to [`round_i32_fpc_impl`] on every in-range finite
/// value; out-of-range and infinite inputs saturate to `i32::MIN`/`i32::MAX`
/// instead of wrapping the FPC integer-indefinite sentinel to `0` (NaN maps to
/// `0` in both). The difference is a **deliberate divergence**, pinned by
/// `tests` and, at the deck-language boundary it is observable from, by
/// `parser::tests::make_integer_out_of_range_is_the_lane_kernel` — the
/// lane-branching pin. (`make_integer_rounds_ties_to_even` next to it asserts
/// only the ties-to-even half, which both kernels share.)
pub fn round_i32_saturating_impl(x: f64) -> i32 {
    x.round_ties_even() as i32
}

#[cfg(feature = "oracle-parity")]
pub use round_i32_fpc_impl as round_i32;
// F.3: the default lane converts `1e10`/`inf` to the nearest representable
// integer instead of reproducing FPC's wrapped integer-indefinite sentinel.
#[cfg(not(feature = "oracle-parity"))]
pub use round_i32_saturating_impl as round_i32;
