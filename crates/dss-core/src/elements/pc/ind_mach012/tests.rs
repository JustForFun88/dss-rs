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

/// Pascal `SetNominalPower` GENERALTIME/DYNAMICMODE arm (IndMach012.pas:
/// 1091-1105): the `ActiveLoadShapeClass` (`Set LoadShapeClass=`) picks WHICH
/// of the three curves drives `ShapeFactor` in dynamics (at hr 2: daily→0.6,
/// yearly→0.7, duty→0.5); default `USENONE` leaves it 1+j1. Same family as the
/// CF2-G PVSystem dynamics load-shape fix.
#[test]
fn dynamics_loadshapeclass_selects_matching_curve() {
    use crate::elements::general::load_shape::{self, LoadShapeObj};
    use crate::obj::base::DssObject;
    use crate::obj::props::PropEngine;
    use crate::solution::{SolveMode, USEDAILY, USEDUTY, USENONE, USEYEARLY};
    use dss_parser::{Parser, ParserVars};

    /// Build a populated `LoadShapeObj` through its real property engine.
    fn build_shape(mult: &str) -> LoadShapeObj {
        let enums = EnumRegistry::new();
        let cls = load_shape::class_props(&enums);
        let mut obj = LoadShapeObj::new("s");
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = crate::diag::ErrorLog::new();
        for (name, value) in [("npts", "4"), ("interval", "1"), ("mult", mult)] {
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

    let mut m = IndMach012::new("m1");
    m.daily_shape_obj = Some(build_shape("0.2 0.6 1.0 0.5"));
    m.yearly_shape_obj = Some(build_shape("0.3 0.7 0.9 0.4"));
    m.duty_shape_obj = Some(build_shape("0.1 0.5 0.8 0.6"));

    let dyn_ctx = |class: i32| SysCtx {
        mode: SolveMode::Dynamic,
        is_dynamic_model: true,
        active_load_shape_class: class,
        dbl_hour: 2.0,
        ..default_recalc_ctx()
    };

    m.set_nominal_power(&dyn_ctx(USEDAILY));
    assert!(
        (m.shape_factor.re - 0.6).abs() < 1e-9,
        "daily: {}",
        m.shape_factor.re
    );
    m.set_nominal_power(&dyn_ctx(USEYEARLY));
    assert!(
        (m.shape_factor.re - 0.7).abs() < 1e-9,
        "yearly: {}",
        m.shape_factor.re
    );
    m.set_nominal_power(&dyn_ctx(USEDUTY));
    assert!(
        (m.shape_factor.re - 0.5).abs() < 1e-9,
        "duty: {}",
        m.shape_factor.re
    );
    m.set_nominal_power(&dyn_ctx(USENONE));
    assert_eq!(m.shape_factor, CDOUBLEONE, "USENONE must leave 1+j1");
}

/// Pascal `TPCElement.GetCurrents` `LastSolutionWasDirect` shortcut (PCElement.pas
/// l.137) wired into `IndMach012::get_currents`: after a direct solve the reported
/// terminal current is `YPrim·Vterminal` (frozen shadow-admittance); without the
/// flag it is the model current `YPrim·V − InjCurrent`. Guards the per-class
/// shortcut branch — IndMach012 inherits the base `GetCurrents`. (Delta default:
/// 3 conductors, no neutral.)
#[test]
fn direct_shortcut_selects_yprim_currents() {
    use crate::elements::pc::generator::default_recalc_ctx;
    use crate::elements::traits::{CktElement, SysCtx};
    use crate::support::cmatrix::CMatrix;
    use num_complex::Complex64;

    let node_v = vec![
        Complex64::ZERO, // ground slot (unused by a delta machine)
        Complex64::new(7200.0, 0.0),
        Complex64::new(-3600.0, -6235.0),
        Complex64::new(-3600.0, 6235.0),
    ];
    let inj = Complex64::new(11.0, -4.0);

    let build = || -> IndMach012 {
        let mut m = IndMach012::new("m1");
        CktElement::calc_yprim(&mut m, &default_recalc_ctx()); // sizes yorder + buffers
        let n = m.cd.yorder;
        let mut yp = CMatrix::new(n);
        for i in 0..n {
            yp.set(i, i, Complex64::new(0.01, -0.02));
        }
        m.cd.yprim = Some(yp);
        m.cd.set_node_ref(1, &[1, 2, 3]); // delta: 3 conductors
        m.cd.inj_current = vec![inj; n];
        m.cd.iterminal_solution_count = 0; // == solution_count → skip model recompute
        m
    };

    // Independent YPrim·Vterminal.
    let n = 3usize;
    let mut yp = CMatrix::new(n);
    for i in 0..n {
        yp.set(i, i, Complex64::new(0.01, -0.02));
    }
    let vterm: Vec<Complex64> = [1usize, 2, 3].iter().map(|&r| node_v[r]).collect();
    let mut yprim_v = vec![Complex64::ZERO; n];
    yp.mv_mult(&mut yprim_v, &vterm);

    // Direct read (flag set) → the shortcut YPrim·V.
    let mut m_d = build();
    let sys_direct = SysCtx {
        last_solution_was_direct: true,
        solution_count: 0,
        ..default_recalc_ctx()
    };
    let mut i_d = vec![Complex64::ZERO; n];
    m_d.get_currents(&sys_direct, &node_v, &mut i_d);

    // Normal read (flag clear, model skipped, Vterminal preset) → YPrim·V − Inj.
    let mut m_n = build();
    m_n.cd.compute_vterminal(&node_v);
    let sys_normal = SysCtx {
        solution_count: 0,
        ..default_recalc_ctx()
    };
    let mut i_n = vec![Complex64::ZERO; n];
    m_n.get_currents(&sys_normal, &node_v, &mut i_n);

    for k in 0..n {
        assert!(
            (i_d[k] - yprim_v[k]).norm() < 1e-9,
            "direct read [{k}] {} != YPrim·V {}",
            i_d[k],
            yprim_v[k]
        );
        assert!(
            (i_d[k] - i_n[k] - inj).norm() < 1e-9,
            "shortcut − model [{k}] {} != InjCurrent {}",
            i_d[k] - i_n[k],
            inj
        );
    }
    assert!(
        (i_d[0] - i_n[0]).norm() > 1.0,
        "shortcut indistinguishable from model current: {} vs {}",
        i_d[0],
        i_n[0]
    );
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
