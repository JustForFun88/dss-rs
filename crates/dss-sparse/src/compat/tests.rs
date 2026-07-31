//! Lane-knob tests. Both constants are compiled in every build; the aliases
//! are what the lanes select. The assertions are `const` blocks — a knob that
//! drifts is a *compile* error, not a runtime one.

use super::*;

/// F.1 staging: both lanes still declare the parity values, so the seam is
/// bit-neutral. The consumers (M3c parallel LU, WP-R1 refinement) flip the
/// `not(oracle-parity)` arms together with the code that reads them; this
/// assertion is what forces that to be a deliberate edit.
#[test]
fn f1_staging_both_lanes_declare_the_parity_execution_knobs() {
    const { assert!(!PARALLEL_FACTORIZATION) };
    const { assert!(!ITERATIVE_REFINEMENT) };
}

/// The parity lane's contract is not negotiable in either direction: whatever
/// the default lane grows, the parity knobs stay off.
#[test]
fn parity_execution_knobs_are_off_by_definition() {
    const { assert!(!PARALLEL_FACTORIZATION_PARITY_IMPL) };
    const { assert!(!ITERATIVE_REFINEMENT_PARITY_IMPL) };
    const { assert!(PARALLEL_FACTORIZATION_DEFAULT_IMPL) };
    const { assert!(ITERATIVE_REFINEMENT_DEFAULT_IMPL) };
}
