use super::*;
use crate::elements::pc::load::default_recalc_ctx;
use crate::elements::traits::{CktElement, InjCtx, SysCtx};
use crate::obj::base::DssObject;
use crate::solution::SolveMode;

fn mode_ctx(mode: SolveMode, frequency: f64) -> SysCtx {
    SysCtx {
        mode,
        frequency,
        ..default_recalc_ctx()
    }
}

#[test]
fn default_construction_matches_pascal_constructor() {
    let isrc = Isource::new("i1");
    assert_eq!(isrc.cd.nphases, 3);
    assert_eq!(isrc.cd.nconds, 3);
    assert_eq!(isrc.cd.nterms, 2);
    assert_eq!(isrc.cd.yorder, 6);
    assert_eq!(isrc.amps, 0.0);
    assert_eq!(isrc.angle, 0.0);
    assert_eq!(isrc.src_frequency, 60.0);
    assert_eq!(isrc.scan_type, 1);
    assert_eq!(isrc.sequence_type, 1);
    assert_eq!(isrc.phase_shift, 120.0);
    assert_eq!(isrc.spectrum, "default");
    assert!(!isrc.bus2_defined);
}

#[test]
fn calc_yprim_is_all_zero() {
    let mut isrc = Isource::new("i1");
    let sys = mode_ctx(SolveMode::Snapshot, 60.0);
    isrc.calc_yprim(&sys);
    let yprim = isrc.cd.yprim.as_ref().unwrap();
    for i in 0..isrc.cd.yorder {
        for j in 0..isrc.cd.yorder {
            assert_eq!(yprim.get(i, j), Complex64::ZERO, "({i},{j})");
        }
    }
}

#[test]
fn phases_side_effect_sets_phase_shift() {
    let mut isrc = Isource::new("i1");
    isrc.cd.nphases = 1;
    isrc.side_effects(prop::PHASES, 0);
    assert_eq!(isrc.phase_shift, 0.0);
    assert_eq!(isrc.cd.nconds, 1);

    isrc.cd.nphases = 5;
    isrc.side_effects(prop::PHASES, 0);
    assert_eq!(isrc.phase_shift, 72.0);
    assert_eq!(isrc.cd.nconds, 5);
}

#[test]
fn bus1_side_effect_defaults_bus2_to_grounded_y() {
    let mut isrc = Isource::new("i1");
    isrc.cd.set_bus(1, "b1.1.2.3");
    isrc.side_effects(prop::BUS1, 0);
    assert_eq!(isrc.cd.get_bus(2), "b1.0.0.0");
}

/// TODO(compat) coverage: Isource never sets `bus2_defined = true` (unlike
/// VSource), so a later Bus1 re-set clobbers an already-explicit Bus2 back to
/// the grounded-Y default.
#[test]
fn bus1_reset_overwrites_explicit_bus2_because_bus2_defined_never_latches() {
    let mut isrc = Isource::new("i1");
    isrc.cd.set_bus(1, "b1");
    isrc.side_effects(prop::BUS1, 0);
    isrc.cd.set_bus(2, "b2"); // explicit Bus2=b2
    assert_eq!(isrc.cd.get_bus(2), "b2");

    // Re-setting Bus1 clobbers the explicit Bus2 back to the default, because
    // `bus2_defined` was never latched true.
    isrc.cd.set_bus(1, "b1");
    isrc.side_effects(prop::BUS1, 0);
    assert_eq!(isrc.cd.get_bus(2), "b1.0.0.0");
}

#[test]
fn snapshot_injection_matches_pdeg_and_opposes_on_terminal2() {
    let mut isrc = Isource::new("i1");
    isrc.amps = 10.0;
    isrc.angle = 0.0;
    let sys = mode_ctx(SolveMode::Snapshot, 60.0);
    isrc.calc_yprim(&sys);
    let mut currents = vec![Complex64::ZERO; 7]; // node 0 = ground
    let mut sys_y_changed = false;
    isrc.cd.node_ref = vec![1, 2, 3, 0, 0, 0]; // 3 phases in, grounded return
    let mut ctx = InjCtx {
        node_v: &[],
        currents: &mut currents,
        system_y_changed: &mut sys_y_changed,
    };
    isrc.inj_currents(&sys, &mut ctx);

    // Phase 1 (index 0) has no rotation applied: BaseCurr = 10∠0.
    assert!((isrc.cd.inj_current[0] - Complex64::new(10.0, 0.0)).norm() < 1e-9);
    // Terminal 2 is the negation of terminal 1, phase for phase.
    for i in 0..3 {
        assert!((isrc.cd.inj_current[i + 3] + isrc.cd.inj_current[i]).norm() < 1e-9);
    }
}

#[test]
fn off_frequency_snapshot_injects_zero() {
    let mut isrc = Isource::new("i1");
    isrc.amps = 10.0;
    isrc.src_frequency = 60.0;
    let sys = mode_ctx(SolveMode::Snapshot, 50.0); // solution at 50 Hz, source at 60 Hz
    isrc.calc_yprim(&sys);
    isrc.cd.node_ref = vec![1, 2, 3, 0, 0, 0];
    let mut currents = vec![Complex64::ZERO; 4];
    let mut sys_y_changed = false;
    let mut ctx = InjCtx {
        node_v: &[],
        currents: &mut currents,
        system_y_changed: &mut sys_y_changed,
    };
    isrc.inj_currents(&sys, &mut ctx);
    for c in &isrc.cd.inj_current {
        assert_eq!(*c, Complex64::ZERO);
    }
}

#[test]
fn get_currents_is_negated_injection() {
    let mut isrc = Isource::new("i1");
    isrc.amps = 5.0;
    let sys = mode_ctx(SolveMode::Snapshot, 60.0);
    isrc.calc_yprim(&sys);
    isrc.cd.node_ref = vec![1, 2, 3, 0, 0, 0];
    let mut curr = vec![Complex64::ZERO; 6];
    isrc.get_currents(&sys, &[], &mut curr);
    // Terminal 1 phase A: Curr = -BaseCurr = -5∠0.
    assert!((curr[0] - Complex64::new(-5.0, 0.0)).norm() < 1e-9);
}
