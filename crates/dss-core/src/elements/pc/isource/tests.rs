use super::*;
use crate::elements::general::load_shape::{self, LoadShapeObj};
use crate::elements::pc::load::default_recalc_ctx;
use crate::elements::traits::{CktElement, InjComputeCtx, SysCtx};
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use crate::solution::SolveMode;
use dss_parser::{Parser, ParserVars};

fn mode_ctx(mode: SolveMode, frequency: f64) -> SysCtx {
    SysCtx {
        mode,
        frequency,
        ..default_recalc_ctx()
    }
}

/// Build a populated `LoadShapeObj` through its real property engine.
fn build_shape(edits: &[(&str, &str)]) -> LoadShapeObj {
    let enums = EnumRegistry::new();
    let cls = load_shape::class_props(&enums);
    let mut obj = LoadShapeObj::new("d");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert!(errors.is_empty(), "{errors:?}");
    obj
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
    let mut inj_errs = crate::diag::ErrorLog::new();
    let mut inj_abort = false;
    isrc.cd.node_ref = vec![1, 2, 3, 0, 0, 0]; // 3 phases in, grounded return
    let mut ctx = InjComputeCtx {
        errors: &mut inj_errs,
        solution_abort: &mut inj_abort,
    };
    isrc.compute_inj_currents(&sys, &[], &mut ctx);

    // Phase 1 (index 0) has no rotation applied: BaseCurr = 10∠0.
    assert!((isrc.cd.inj_current[0] - Complex64::new(10.0, 0.0)).norm() < 1e-9);
    // Terminal 2 is the negation of terminal 1, phase for phase.
    for i in 0..3 {
        assert!((isrc.cd.inj_current[i + 3] + isrc.cd.inj_current[i]).norm() < 1e-9);
    }
}

/// Pascal `GetBaseCurr` DYNAMICMODE arm (Isource.pas:403-416): dynamics honors
/// `Set LoadShapeClass=` — `USEDAILY` scales the injection by the daily-shape
/// mult (0.5 → 5 A from 10 A); the default `USENONE` resets `ShapeFactor` to
/// 1+j0 (full amps). Same family as the CF2-G PVSystem dynamics load-shape fix.
#[test]
fn dynamics_loadshapeclass_scales_injection() {
    let mut isrc = Isource::new("i1");
    isrc.amps = 10.0;
    isrc.angle = 0.0;
    isrc.daily_shape_obj = Some(build_shape(&[
        ("npts", "2"),
        ("interval", "1"),
        ("mult", "0.5 1.0"),
    ]));
    isrc.cd.node_ref = vec![1, 2, 3, 0, 0, 0];

    let dyn_ctx = |class: i32| SysCtx {
        mode: SolveMode::Dynamic,
        is_dynamic_model: true,
        active_load_shape_class: class,
        dbl_hour: 1.0,
        frequency: 60.0,
        ..default_recalc_ctx()
    };

    let sys = dyn_ctx(crate::solution::USEDAILY);
    isrc.calc_yprim(&sys);
    let mut inj_errs = crate::diag::ErrorLog::new();
    let mut inj_abort = false;
    let mut ctx = InjComputeCtx {
        errors: &mut inj_errs,
        solution_abort: &mut inj_abort,
    };
    isrc.compute_inj_currents(&sys, &[], &mut ctx);
    assert!(
        (isrc.cd.inj_current[0] - Complex64::new(5.0, 0.0)).norm() < 1e-9,
        "USEDAILY mult 0.5 must halve the injection: {}",
        isrc.cd.inj_current[0]
    );

    // Default USENONE: ShapeFactor := 1+j0 → the full 10 A.
    let sys = dyn_ctx(crate::solution::USENONE);
    let mut inj_errs = crate::diag::ErrorLog::new();
    let mut inj_abort = false;
    let mut ctx = InjComputeCtx {
        errors: &mut inj_errs,
        solution_abort: &mut inj_abort,
    };
    isrc.compute_inj_currents(&sys, &[], &mut ctx);
    assert!(
        (isrc.cd.inj_current[0] - Complex64::new(10.0, 0.0)).norm() < 1e-9,
        "USENONE must inject the full amps: {}",
        isrc.cd.inj_current[0]
    );
}

#[test]
fn off_frequency_snapshot_injects_zero() {
    let mut isrc = Isource::new("i1");
    isrc.amps = 10.0;
    isrc.src_frequency = 60.0;
    let sys = mode_ctx(SolveMode::Snapshot, 50.0); // solution at 50 Hz, source at 60 Hz
    isrc.calc_yprim(&sys);
    isrc.cd.node_ref = vec![1, 2, 3, 0, 0, 0];
    let mut inj_errs = crate::diag::ErrorLog::new();
    let mut inj_abort = false;
    let mut ctx = InjComputeCtx {
        errors: &mut inj_errs,
        solution_abort: &mut inj_abort,
    };
    isrc.compute_inj_currents(&sys, &[], &mut ctx);
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

/// Isource, multi-phase (default 3) → bare `Phases := 1` edit + run_base
/// (Pascal `TIsourceObj.MakePosSequence`, Isource.pas:500-505).
#[test]
fn makeposseq_isource_multiphase_sets_phases_1() {
    use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
    let mut isrc = Isource::new("i1");
    assert!(isrc.cd.nphases > 1);
    let plan = isrc.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    assert_eq!(plan.actions, vec![PosSeqAction::SetI32(prop::PHASES, 1)]);
}

/// Isource, already single phase → base-only (no actions), still run_base.
#[test]
fn makeposseq_isource_single_phase_is_base_only() {
    use crate::elements::pos_seq::PosSeqCtx;
    let mut isrc = Isource::new("i1");
    isrc.cd.nphases = 1;
    let plan = isrc.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    assert!(plan.actions.is_empty());
}
