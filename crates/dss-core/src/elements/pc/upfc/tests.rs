//! Unit tests for the UPFC element's parse-time defaults and the series YPrim.

use super::*;
use crate::elements::pc::generator::default_recalc_ctx;
use crate::elements::traits::{CktElement, SysCtx};

#[test]
fn defaults_match_pascal_create() {
    let u = Upfc::new("test");
    assert_eq!(u.cd.nphases, 1);
    assert_eq!(u.cd.nterms, 2);
    assert_eq!(u.v_ref, 0.24);
    assert_eq!(u.pf, 1.0);
    assert_eq!(u.xs, 0.7540);
    assert_eq!(u.tol1, 0.02);
    assert_eq!(u.mode_upfc, 1);
    assert_eq!(u.vpqmax, 24.0);
    assert_eq!(u.vh_limit, 300.0);
    assert_eq!(u.vl_limit, 125.0);
    assert_eq!(u.c_limit, 265.0);
    assert_eq!(u.kvar_lim, 5.0);
    assert_eq!(u.num_variables(), 14);
    assert_eq!(u.variable_name(1), "ModeUPFC");
    assert_eq!(u.variable_name(14), "Im{Sr1^[1]}");
}

#[test]
fn series_yprim_is_the_xs_reactance_block() {
    let mut u = Upfc::new("test");
    u.xs = 0.02;
    u.cd.base_frequency = 60.0;
    let sys = SysCtx {
        frequency: 60.0,
        ..default_recalc_ctx()
    };
    u.calc_yprim(&sys);
    let yprim = u.cd.yprim.as_ref().unwrap();
    // Y = inv(j·0.02) = -j·50 on the diagonal; the 2-terminal series mirror.
    let y = yprim.get(0, 0);
    assert!((y.re).abs() < 1e-9);
    assert!((y.im + 50.0).abs() < 1e-9, "y = {y:?}");
    assert!((yprim.get(0, 1) - num_complex::Complex64::new(0.0, 50.0)).norm() < 1e-9);
    assert!((yprim.get(1, 0) - num_complex::Complex64::new(0.0, 50.0)).norm() < 1e-9);
}

/// UPFC `MakePosSequence` is an EMPTY Pascal body (UPFC.pas:1058-1060): no
/// property edits and no `inherited` call → `PosSeqPlan::no_base()`.
#[test]
fn makeposseq_upfc_is_no_base_and_empty() {
    use crate::elements::pos_seq::PosSeqCtx;
    let mut u = Upfc::new("test");
    let plan = u.make_pos_sequence(&PosSeqCtx::default());
    assert!(!plan.run_base);
    assert!(plan.actions.is_empty());
}
