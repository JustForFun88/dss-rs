//! Stage F lane split — the `oracle-parity` knobs of `dss-sparse`
//! (`DE_PASCALIZE_PLAN.md` Part IV.2, row "solver execution").
//!
//! **The only file in `dss-sparse` allowed to contain the
//! `#[cfg(feature = "oracle-parity")]` string** (enforced by `dss-core`'s
//! `tests/oracle_parity_cfg_gate.rs`). See `dss-core/src/compat.rs` for the
//! lane model; the feature reaches this crate through
//! `dss-core/oracle-parity → dss-sparse/oracle-parity`.
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

/// Which lane this crate was compiled in — `true` under `oracle-parity`.
///
/// Nothing in the solver reads it. It exists so `dss-core`'s compat tests can
/// *measure* that `dss-core/oracle-parity` really reaches this crate: both
/// knobs below currently hold the same value in either lane (they are
/// declarations awaiting M3c / WP-R1), so without this const a broken feature
/// edge would be invisible here until the day it silently un-pinned the
/// solver.
#[cfg(feature = "oracle-parity")]
pub const ORACLE_PARITY: bool = true;

/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub const ORACLE_PARITY: bool = false;

/// Parity lane: factorization must be reproducible — no parallel LU.
pub const PARALLEL_FACTORIZATION_PARITY_IMPL: bool = false;
/// Default lane: faer's parallel LU is permitted (M3c).
pub const PARALLEL_FACTORIZATION_DEFAULT_IMPL: bool = true;

#[cfg(feature = "oracle-parity")]
pub use PARALLEL_FACTORIZATION_PARITY_IMPL as PARALLEL_FACTORIZATION;
// F.1 staging: the default lane still declares the parity value; M3c flips it
// together with the code that reads it.
#[cfg(not(feature = "oracle-parity"))]
pub use PARALLEL_FACTORIZATION_PARITY_IMPL as PARALLEL_FACTORIZATION;

/// Parity lane: no iterative refinement — the oracle's iterate paths (and the
/// gated iteration counts) are pinned exactly.
pub const ITERATIVE_REFINEMENT_PARITY_IMPL: bool = false;
/// Default lane: WP-R1 iterative refinement is permitted.
pub const ITERATIVE_REFINEMENT_DEFAULT_IMPL: bool = true;

#[cfg(feature = "oracle-parity")]
pub use ITERATIVE_REFINEMENT_PARITY_IMPL as ITERATIVE_REFINEMENT;
// F.1 staging: see above — WP-R1 flips it together with its implementation.
#[cfg(not(feature = "oracle-parity"))]
pub use ITERATIVE_REFINEMENT_PARITY_IMPL as ITERATIVE_REFINEMENT;
