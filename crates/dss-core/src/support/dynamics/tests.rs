use super::*;

#[test]
fn codes_match_pascal_ordinals() {
    assert_eq!(DynSolveMode::Snapshot.code(), 0);
    assert_eq!(DynSolveMode::DutyCycle.code(), 6);
    assert_eq!(DynSolveMode::Dynamic.code(), 14);
    assert_eq!(DynSolveMode::HarmonicT.code(), 17);
    for code in 0..=17 {
        assert_eq!(DynSolveMode::from_code(code).unwrap().code(), code);
    }
    assert_eq!(DynSolveMode::from_code(18), None);
    assert_eq!(DynSolveMode::from_code(-1), None);
}

#[test]
fn defaults() {
    let d = DynaVars::default();
    assert_eq!(d.solution_mode, DynSolveMode::Snapshot);
    assert_eq!(d.iteration_flag, IterationFlag::NewTimeStep);
    assert_eq!(d.int_hour, 0);
    assert_eq!(d.h, 0.0);
}
