//! Lane-knob tests. Both constants are compiled in every build; the aliases
//! are what the lanes select. The assertions are `const` blocks — a knob that
//! drifts is a *compile* error, not a runtime one.

use super::*;

/// Both lanes hold the parity values of the execution knobs, so the seam is
/// bit-neutral until the code that reads them selects by lane.
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
