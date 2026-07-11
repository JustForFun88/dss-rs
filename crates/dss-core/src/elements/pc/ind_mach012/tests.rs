//! Unit tests for the IndMach012 construction defaults and slip handling. The
//! full power-flow/dynamics numerics are pinned by the oracle gate
//! (`exec/tests/dynamics.rs`) and `props_roundtrip`.

use super::*;

#[test]
fn create_defaults_match_pascal() {
    let m = IndMach012::new("m1");
    assert_eq!(m.cd.nphases, 3);
    assert_eq!(m.cd.nconds, 3); // delta default, no neutral
    assert_eq!(m.connection, Connection::Delta);
    assert_eq!(m.kw_base, 1000.0);
    assert_eq!(m.kv_generator_base, 12.47);
    assert_eq!(m.kva_rating, 1200.0);
    assert_eq!(m.pu_rs, 0.0053);
    assert_eq!(m.pu_xs, 0.106);
    assert_eq!(m.pu_rr, 0.007);
    assert_eq!(m.pu_xr, 0.12);
    assert_eq!(m.pu_xm, 4.0);
    assert_eq!(m.max_slip, 0.1);
    assert!(!m.fixed_slip);
    // Create sets slip 0.007 (within MaxSlip) → S2 = 2 - S1.
    assert_eq!(m.s1, 0.007);
    assert_eq!(m.s2, 2.0 - 0.007);
}

#[test]
fn set_local_slip_clamps_outside_dynamics() {
    let mut m = IndMach012::new("m1");
    m.set_local_slip(0.5); // > MaxSlip 0.1
    assert_eq!(m.s1, 0.1);
    m.set_local_slip(-0.5);
    assert_eq!(m.s1, -0.1);
    // In dynamics the slip floats freely.
    m.in_dynamics = true;
    m.set_local_slip(0.5);
    assert_eq!(m.s1, 0.5);
    assert_eq!(m.s2, 1.5);
}

#[test]
fn recalc_sets_impedances() {
    let m = IndMach012::new("m1");
    // ZBase = kV²/kVA·1000 = 12.47²/1200·1000.
    let z_base = 12.47_f64.powi(2) / 1200.0 * 1000.0;
    assert!((m.zs.re - m.pu_rs * z_base).abs() < 1e-9);
    assert!((m.zs.im - m.pu_xs * z_base).abs() < 1e-9);
    assert_eq!(m.zr.re, m.pu_rr * z_base);
    assert_eq!(m.zm.im, m.pu_xm * z_base);
    // Yeq is vars-only for power flow: -j/ZBase.
    assert!((m.yeq.im + 1.0 / z_base).abs() < 1e-12);
    assert_eq!(m.yeq.re, 0.0);
}

/// IndMach012 `MakePosSequence` is an EMPTY Pascal body (IndMach012.pas:1424-1426):
/// no property edits and no `inherited` call → `PosSeqPlan::no_base()`.
#[test]
fn makeposseq_indmach012_is_no_base_and_empty() {
    use crate::elements::pos_seq::PosSeqCtx;
    use crate::elements::traits::CktElement;
    let mut m = IndMach012::new("m1");
    let plan = m.make_pos_sequence(&PosSeqCtx::default());
    assert!(!plan.run_base);
    assert!(plan.actions.is_empty());
}
