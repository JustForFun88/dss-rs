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
    }
}

#[test]
fn default_is_3ph_wye_shunt() {
    let r = Reactor::new("r1");
    assert_eq!(r.cd.nphases, 3);
    assert_eq!(r.cd.nconds, 3);
    assert_eq!(r.cd.nterms, 2);
    assert_eq!(r.cd.yorder, 6);
    assert!(r.is_shunt);
    assert_eq!(r.spec_type, 1);
    assert_eq!(r.get_bus_name(2), "r1_1.0.0.0");
}

/// kvar-spec 3φ wye, `kvar=500 kV=12.47`: every diagonal is `-jb`, the
/// `[i,i+3]` off-diagonal `+jb`, with `b = 0.00321542` (probed in dss-python).
#[test]
fn yprim_3ph_wye_kvar_matches_oracle() {
    let mut r = Reactor::new("r");
    r.kvarrating = 500.0;
    r.kvrating = 12.47;
    r.recalc();
    r.calc_yprim(&test_sys());

    let yp = r.cd.yprim.as_ref().unwrap();
    let b = 0.00321542_f64;
    for i in 0..3 {
        let d = yp.get(i, i);
        assert!(d.re.abs() < 1e-9, "diag re {}", d.re);
        assert!((d.im + b).abs() < 1e-7, "diag im {} vs {}", d.im, -b);
        let off = yp.get(i, i + 3);
        assert!((off.im - b).abs() < 1e-7, "off im {} vs {b}", off.im);
    }
}

/// Symmetrical components `Z1=Z2=(1,5) Z0=(2,8)`, 3φ. Probed in dss-python:
/// diagonal `0.0354449 - 0.167421j`, in-block off `-0.0030166 + 0.0248869j`.
#[test]
fn yprim_3ph_z1z2z0_matches_oracle() {
    let mut r = Reactor::new("r");
    r.spec_type = 4;
    r.z1 = Complex64::new(1.0, 5.0);
    r.z2 = Complex64::new(1.0, 5.0);
    r.z0 = Complex64::new(2.0, 8.0);
    r.recalc();
    r.calc_yprim(&test_sys());

    let yp = r.cd.yprim.as_ref().unwrap();
    let diag = Complex64::new(0.0354449, -0.167421);
    let off = Complex64::new(-0.0030166, 0.0248869);
    for i in 0..3 {
        assert!((yp.get(i, i) - diag).norm() < 1e-5, "diag {}", yp.get(i, i));
        for j in 0..3 {
            if i != j {
                assert!((yp.get(i, j) - off).norm() < 1e-5, "off {}", yp.get(i, j));
            }
        }
        // Cross-block is negated.
        assert!((yp.get(i, i + 3) + diag).norm() < 1e-5);
    }
}

/// RMatrix/XMatrix series spec (`bus2` set), 3φ. Probed in dss-python:
/// diagonal `0.0412088 - 0.206044j`, in-block off `-0.00686813 + 0.0343407j`.
#[test]
fn yprim_3ph_rxmatrix_matches_oracle() {
    let mut r = Reactor::new("r");
    r.spec_type = 3;
    r.is_shunt = false; // bus2 set → series
    // Row-major nphases² (symmetric).
    r.rmatrix = Some(vec![1.0, 0.2, 0.2, 0.2, 1.0, 0.2, 0.2, 0.2, 1.0]);
    r.xmatrix = Some(vec![5.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0, 1.0, 5.0]);
    r.recalc();
    r.calc_yprim(&test_sys());

    let yp = r.cd.yprim.as_ref().unwrap();
    let diag = Complex64::new(0.0412088, -0.206044);
    let off = Complex64::new(-0.00686813, 0.0343407);
    for i in 0..3 {
        assert!((yp.get(i, i) - diag).norm() < 1e-5, "diag {}", yp.get(i, i));
        for j in 0..3 {
            if i != j {
                assert!((yp.get(i, j) - off).norm() < 1e-5, "off {}", yp.get(i, j));
            }
        }
        assert!((yp.get(i, i + 3) + diag).norm() < 1e-5);
    }
}
