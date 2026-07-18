use super::*;
use crate::elements::general::load_shape::{self, LoadShapeObj};
use crate::elements::pc::load::default_recalc_ctx;
use crate::elements::traits::SysCtx;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::PropEngine;
use crate::solution::SolveMode;
use dss_parser::{Parser, ParserVars};

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
    obj.end_edit();
    assert!(errors.is_empty(), "{errors:?}");
    obj
}

fn mode_ctx(mode: SolveMode, dbl_hour: f64) -> SysCtx {
    SysCtx {
        mode,
        dbl_hour,
        ..default_recalc_ctx()
    }
}

#[test]
fn daily_mode_loadshape_scales_source_magnitude() {
    // In a loadshape mode the source magnitude is scaled by ShapeFactor.re;
    // mult 0.5 (per-unit shape) halves the normal Vmag.
    let shape = build_shape(&[("npts", "2"), ("interval", "1"), ("mult", "0.5 1.0")]);
    let mut vs = VSource::new("v");
    vs.daily_shape_obj = Some(shape);

    vs.get_vterminal_for_source(&mode_ctx(SolveMode::Snapshot, 1.0));
    let vmag_snap = vs.vmag;
    vs.get_vterminal_for_source(&mode_ctx(SolveMode::Daily, 1.0)); // mult 0.5
    assert!(
        (vs.vmag - 0.5 * vmag_snap).abs() < 1e-6,
        "daily {} vs half-snapshot {}",
        vs.vmag,
        0.5 * vmag_snap
    );
}

#[test]
fn duty_mode_falls_back_to_daily_shape() {
    let shape = build_shape(&[("npts", "2"), ("interval", "1"), ("mult", "0.5 1.0")]);
    let mut vs = VSource::new("v");
    vs.daily_shape_obj = Some(shape);
    vs.get_vterminal_for_source(&mode_ctx(SolveMode::Snapshot, 1.0));
    let vmag_snap = vs.vmag;
    vs.get_vterminal_for_source(&mode_ctx(SolveMode::DutyCycle, 1.0));
    assert!((vs.vmag - 0.5 * vmag_snap).abs() < 1e-6);
}

/// Pascal `GetVterminalForSource` DYNAMICMODE arm (VSource.pas:1006-1026):
/// dynamics is a loadshape mode under `Set LoadShapeClass=` — `USEDAILY`
/// samples the daily shape at the dynamics hour (mult 0.5 halves Vmag); the
/// default `USENONE` sets `ShapeFactor := PerUnit`, reproducing the snapshot
/// magnitude. Same family as the CF2-G PVSystem dynamics load-shape fix.
#[test]
fn dynamics_loadshapeclass_scales_source_magnitude() {
    let shape = build_shape(&[("npts", "2"), ("interval", "1"), ("mult", "0.5 1.0")]);
    let mut vs = VSource::new("v");
    vs.daily_shape_obj = Some(shape);

    vs.get_vterminal_for_source(&mode_ctx(SolveMode::Snapshot, 1.0));
    let vmag_snap = vs.vmag;

    let dyn_ctx = |class: i32| SysCtx {
        mode: SolveMode::Dynamic,
        is_dynamic_model: true,
        active_load_shape_class: class,
        dbl_hour: 1.0,
        ..default_recalc_ctx()
    };
    vs.get_vterminal_for_source(&dyn_ctx(crate::solution::USEDAILY));
    assert!(
        (vs.vmag - 0.5 * vmag_snap).abs() < 1e-6,
        "dynamics+USEDAILY {} vs half-snapshot {}",
        vs.vmag,
        0.5 * vmag_snap
    );

    vs.get_vterminal_for_source(&dyn_ctx(crate::solution::USENONE));
    assert!(
        (vs.vmag - vmag_snap).abs() < 1e-9,
        "dynamics+USENONE {} must equal snapshot {}",
        vs.vmag,
        vmag_snap
    );
}

// --- MakePosSequence (WPG.21) --------------------------------------------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
use crate::elements::traits::CktElement;

/// VSource → 1 phase, basekv ÷ √3, R1/X1 preserved.
#[test]
fn makeposseq_vsource() {
    let mut vs = VSource::new("v");
    vs.kv_base = 12.47;
    vs.r1 = 1.0;
    vs.x1 = 3.0;

    let plan = vs.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    let kv_new = 12.47 / 3.0_f64.sqrt();
    assert_eq!(
        plan.actions,
        vec![
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(prop::PHASES, 1),
            PosSeqAction::SetF64(prop::BASEKV, kv_new),
            PosSeqAction::SetF64(prop::R1, 1.0),
            PosSeqAction::SetF64(prop::X1, 3.0),
            PosSeqAction::EndEdit,
        ]
    );
}
