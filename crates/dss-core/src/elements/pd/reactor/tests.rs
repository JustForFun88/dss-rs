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
        dyna_t: 0.0,
        iteration_flag: crate::support::dynamics::IterationFlag::NewTimeStep,
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

/// Regression guard for the `stamp_series` asymmetric-YPrim bug (the two-terminal
/// series stamp's bottom-left block was `(j+n, i)` instead of Pascal's `(i+n, j)`,
/// `Reactor.pas:936`). A **symmetrical-components** reactor with `Z1 != Z2` (the
/// induction-motor model) has a **non-reciprocal / asymmetric** Y; the transposed
/// stamp violates KCL (`I_t1 + I_t2 = (Y - Yᵀ)·V1 != 0`) and corrupts an
/// **unbalanced** solve — invisible to a balanced one. This solves an unbalanced
/// deck and checks (a) KCL at the reactor (physics — catches the transpose
/// directly) and (b) the per-conductor terminal currents against the pinned
/// oracle (dss-python 0.15.7 / engine 0.14.5). Reverting the fix fails (a).
#[test]
fn asymmetric_sym_components_reactor_unbalanced_solve() {
    use crate::exec::Dss;
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.rasym basekv=12.47 bus1=src");
    dss.command(
        "new reactor.rk phases=3 bus1=src bus2=b \
         Z1=[1.9775 1.3431] Z2=[0.1203 0.3623] Z0=[1 0]",
    );
    // Unbalanced per-phase loads → the reactor carries unbalanced currents, so the
    // negative-sequence (asymmetric) coupling is exercised.
    dss.command("new load.la bus1=b.1 phases=1 kv=7.2 kw=800 pf=0.9");
    dss.command("new load.lb bus1=b.2 phases=1 kv=7.2 kw=200 pf=0.95");
    dss.command("new load.lc bus1=b.3 phases=1 kv=7.2 kw=1400 pf=0.85");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcv");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());

    let snap = dss.snapshot_elements();
    let rk = snap
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("Reactor.rk"))
        .expect("reactor.rk in snapshot");
    let c = &rk.currents; // [re,im] per conductor: t1 phases 0-2, t2 phases 3-5.
    assert_eq!(c.len(), 12, "6 conductors x (re,im)");

    // (a) KCL: I_t1 + I_t2 == 0 per phase (the transpose bug breaks this).
    for ph in 0..3 {
        let re = c[ph * 2] + c[(ph + 3) * 2];
        let im = c[ph * 2 + 1] + c[(ph + 3) * 2 + 1];
        assert!(
            re.abs() < 1e-3 && im.abs() < 1e-3,
            "KCL violated at phase {}: I_t1+I_t2 = {re}+{im}j (asymmetric reactor \
             stamp transpose?)",
            ph + 1
        );
    }

    // (b) Per-conductor terminal currents vs the pinned oracle.
    let want = [
        116.146461,
        -57.769703,
        -22.612834,
        -20.041948,
        7.672820,
        240.310043,
        -116.146461,
        57.769703,
        22.612834,
        20.041948,
        -7.672820,
        -240.310043,
    ];
    for (k, (&a, &w)) in c.iter().zip(want.iter()).enumerate() {
        assert!(
            (a - w).abs() <= 1e-3 + 1e-6 * w.abs(),
            "reactor current [{k}]: got {a}, oracle {w}"
        );
    }
}
