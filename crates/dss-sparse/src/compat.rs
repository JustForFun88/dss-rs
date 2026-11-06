//! The precision-compat knobs of `dss-sparse`, the "solver execution" row.
//! See `dss-core/src/compat.rs` for the lane model.
//!
//! Unlike the arithmetic kernels there is no pair of `*_impl` functions here:
//! the row is a pair of **execution knobs** the parity lane must keep pinned —
//!
//! - **parallel factorization** — the parity lane's contract is a strictly
//!   reproducible factorization; the default lane may use faer's parallel LU
//!   (`MULTITHREADING_PLAN.md` M3c).
//! - **iterative refinement** — `RESONANCE_PLAN.md` WP-R1 turns refinement on
//!   in the default lane; the parity lane must not, because refinement changes
//!   the solved vector and therefore the *iteration counts* the oracle gates
//!   pin exactly.
//!
//! **Status — declaration only, no consumer yet** (deliberately: both
//! consumers land with their own plans, and F.1 is bit-neutral). Today
//! [`crate::SparseSet::factor`] goes through faer's
//! `Lu::try_new_with_symbolic`, which reads faer's *global* parallelism
//! setting (`get_global_parallelism()`; with faer's default `rayon` feature
//! that is `Par::rayon(0)`, not `Par::Seq`) and offers no per-call `Par`
//! argument. Overriding it needs the lower-level `factorize_numeric_lu` path,
//! which is M3c's job — that is where these knobs get read, and where the
//! plan-table wording "parity = `Par::Seq`" becomes true code. Recorded so the
//! discrepancy is not mistaken for a wired lane.

#[cfg(test)]
mod tests;

/// Which lane this crate was compiled in, `true` in the parity lane. Nothing in
/// the solver reads it yet: the knobs below hold the parity value in both lanes.
pub use dss_compat::OPENDSS_PARITY;

/// Parity lane: factorization must be reproducible — no parallel LU.
///
/// **A declaration awaiting M3c, not a contract the build honours today.** No
/// call site reads it, and both lanes take it, so today *both* lanes
/// factorize under faer's global parallelism: `Lu::try_new_with_symbolic`
/// (faer 0.24.0 `src/sparse/solvers.rs`) takes no `Par` and calls
/// `get_global_parallelism()`, which initializes to `Par::rayon(0)` under
/// faer's default feature set. The `_PARITY_IMPL` suffix reads like an
/// enforced invariant, which is why this note is on the const itself and not
/// only in the module header — `MULTITHREADING_PLAN` M3c owns making it true.
pub const PARALLEL_FACTORIZATION_PARITY_IMPL: bool = false;
/// Default lane: faer's parallel LU is permitted (M3c).
pub const PARALLEL_FACTORIZATION_DEFAULT_IMPL: bool = true;

/// The lane's parallel-factorization knob. Both lanes hold the parity value
/// until the code that reads it selects by [`OPENDSS_PARITY`].
pub const PARALLEL_FACTORIZATION: bool = PARALLEL_FACTORIZATION_PARITY_IMPL;

/// Parity lane: no iterative refinement — the oracle's iterate paths (and the
/// gated iteration counts) are pinned exactly.
pub const ITERATIVE_REFINEMENT_PARITY_IMPL: bool = false;
/// Default lane: WP-R1 iterative refinement is permitted.
pub const ITERATIVE_REFINEMENT_DEFAULT_IMPL: bool = true;

/// The lane's iterative-refinement knob. Both lanes hold the parity value until
/// the refinement code that reads it selects by [`OPENDSS_PARITY`].
pub const ITERATIVE_REFINEMENT: bool = ITERATIVE_REFINEMENT_PARITY_IMPL;
