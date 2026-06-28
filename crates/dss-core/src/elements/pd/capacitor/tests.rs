use super::*;
use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::base::DssObject;
use crate::solution::SolveMode;

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: 1,
        mode: SolveMode::Snapshot,
        load_multiplier: 1.0,
        gen_multiplier: 1.0,
        generator_dispatch_reference: 0.0,
        price_signal: 25.0,
        default_growth_factor: 1.0,
        year: 0,
        dbl_hour: 0.0,
        solution_count: 0,
        loads_need_updating: false,
        neglect_load_y: false,
        long_line_correction: false,
        positive_sequence: false,
        time_of_day: 0.0,
        dyna_h: 0.0,
        iteration_flag: crate::support::dynamics::IterationFlag::NewTimeStep,
    }
}

#[test]
fn default_is_3ph_wye_shunt() {
    let c = Capacitor::new("c1");
    assert_eq!(c.cd.nphases, 3);
    assert_eq!(c.cd.nconds, 3);
    assert_eq!(c.cd.nterms, 2);
    assert_eq!(c.cd.yorder, 6);
    assert!(c.is_shunt);
    assert_eq!(c.spec_type, 1);
    // Bus2 defaulted to the grounded node of the auto-named Bus1.
    assert_eq!(c.get_bus_name(2), "c1_1.0.0.0");
}

/// YPrim of a 3-phase wye 600 kvar @ 4.16 kV bank: every step diagonal is
/// `j·b` with `b = 0.034670858` (probed in dss-python), wye 2-terminal
/// stamping. Oracle: `Yprim[i,i] = +jb`, `Yprim[i,i+3] = -jb`.
#[test]
fn yprim_3ph_wye_kvar_matches_oracle() {
    let mut c = Capacitor::new("cap");
    c.cd.nphases = 3;
    c.cd.nconds = 3;
    c.cd.set_nterms(2);
    c.cd.yorder = 6;
    c.connection = 0;
    c.is_shunt = true;
    c.spec_type = 1;
    c.kvrating = 4.16;
    c.fkvarrating = vec![600.0];
    c.fc = vec![0.0];
    c.fr = vec![0.0];
    c.fxl = vec![0.0];
    c.fharm = vec![0.0];
    c.fstates = vec![1];
    c.fnumsteps = 1;
    c.recalc();

    c.calc_yprim(&test_sys());
    let yp = c.cd.yprim.as_ref().unwrap();
    let b = 0.034670858_f64;
    for i in 0..3 {
        let d = yp.get(i, i);
        assert!((d.re).abs() < 1e-9, "diag re {}", d.re);
        assert!((d.im - b).abs() < 1e-5, "diag im {} vs {b}", d.im);
        let off = yp.get(i, i + 3);
        assert!((off.im + b).abs() < 1e-5, "off im {} vs {}", off.im, -b);
    }
}

/// 1-phase wye 100 kvar @ 2.4 kV: `b = 0.017361111` (probed in dss-python).
#[test]
fn yprim_1ph_wye_kvar_matches_oracle() {
    let mut c = Capacitor::new("cap");
    c.cd.nphases = 1;
    c.cd.nconds = 1;
    c.cd.set_nterms(2);
    c.cd.yorder = 2;
    c.connection = 0;
    c.is_shunt = true;
    c.spec_type = 1;
    c.kvrating = 2.4;
    c.fkvarrating = vec![100.0];
    c.fc = vec![0.0];
    c.fr = vec![0.0];
    c.fxl = vec![0.0];
    c.fharm = vec![0.0];
    c.fstates = vec![1];
    c.fnumsteps = 1;
    c.recalc();

    c.calc_yprim(&test_sys());
    let yp = c.cd.yprim.as_ref().unwrap();
    let b = 0.017361111_f64;
    assert!((yp.get(0, 0).im - b).abs() < 1e-6);
    assert!((yp.get(0, 1).im + b).abs() < 1e-6);
    assert!((yp.get(1, 1).im - b).abs() < 1e-6);
}

/// YPrim of a 3-phase wye `CMatrix` bank (`SpecType = 3`):
/// `cmatrix=(1.5 | -0.3 1.5 | -0.3 -0.3 1.5)` µF. Probed in dss-python:
/// diagonal `j·w·1.5e-6 = j5.6548668e-4`, in-block off-diagonal
/// `j·w·(-0.3e-6) = -j1.1309734e-4`, cross-block negated.
#[test]
fn yprim_3ph_cmatrix_matches_oracle() {
    let mut c = Capacitor::new("cap");
    c.cd.nphases = 3;
    c.cd.nconds = 3;
    c.cd.set_nterms(2);
    c.cd.yorder = 6;
    c.connection = 0;
    c.is_shunt = true;
    c.spec_type = 3;
    // Row-major nphases² in farads (the parse scales µF by 1e-6).
    let m = 1.0e-6;
    c.cmatrix = Some(vec![
        1.5 * m,
        -0.3 * m,
        -0.3 * m,
        -0.3 * m,
        1.5 * m,
        -0.3 * m,
        -0.3 * m,
        -0.3 * m,
        1.5 * m,
    ]);
    c.fr = vec![0.0];
    c.fxl = vec![0.0];
    c.fharm = vec![0.0];
    c.fstates = vec![1];
    c.fnumsteps = 1;
    c.recalc();

    c.calc_yprim(&test_sys());
    let yp = c.cd.yprim.as_ref().unwrap();
    let diag = 5.6548668e-4_f64;
    let off = 1.1309734e-4_f64;
    for i in 0..3 {
        assert!(
            (yp.get(i, i).im - diag).abs() < 1e-9,
            "diag {}",
            yp.get(i, i).im
        );
        assert!((yp.get(i, i + 3).im + diag).abs() < 1e-9);
        for j in 0..3 {
            if i != j {
                assert!(
                    (yp.get(i, j).im + off).abs() < 1e-9,
                    "off {}",
                    yp.get(i, j).im
                );
            }
        }
    }
}

#[test]
fn numsteps_splits_kvar() {
    let mut c = Capacitor::new("c1");
    c.spec_type = 1;
    c.fkvarrating = vec![600.0];
    c.fnumsteps = 1;
    c.set_num_steps(3);
    assert_eq!(c.fnumsteps, 3);
    assert_eq!(c.fkvarrating, vec![200.0, 200.0, 200.0]);
    assert_eq!(c.fstates, vec![1, 1, 1]);
    assert_eq!(c.flast_step_in_service, 3);
}
