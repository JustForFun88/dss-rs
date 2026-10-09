use super::*;
use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::base::DssObject;
use crate::solution::{LoadSolutionModel, SolveMode};

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: LoadSolutionModel::PowerFlow,
        mode: SolveMode::Snapshot,
        active_load_shape_class: crate::solution::USENONE,
        load_multiplier: 1.0,
        gen_multiplier: 1.0,
        generator_dispatch_reference: 0.0,
        price_signal: 25.0,
        default_growth_factor: 1.0,
        year: 0,
        dbl_hour: 0.0,
        solution_count: 0,
        iteration: 0,
        in_show_results: false,
        loads_need_updating: false,
        neglect_load_y: false,
        long_line_correction: false,
        positive_sequence: false,
        time_of_day: 0.0,
        dyna_h: 0.0,
        dyna_t: 0.0,
        iteration_flag: crate::support::dynamics::IterationFlag::NewTimeStep,
        last_solution_was_direct: false,
        ncim: false,
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
    assert_eq!(r.spec_type, ReactorSpecType::Kvar);
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
    r.spec_type = ReactorSpecType::SymComponents;
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
    r.spec_type = ReactorSpecType::Matrices;
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
    let c = &rk.currents; // per conductor: t1 phases 0-2, t2 phases 3-5.
    assert_eq!(c.len(), 6, "6 conductors");

    // (a) KCL: I_t1 + I_t2 == 0 per phase (the transpose bug breaks this).
    for ph in 0..3 {
        let s = c[ph] + c[ph + 3];
        let (re, im) = (s.re, s.im);
        assert!(
            re.abs() < 1e-3 && im.abs() < 1e-3,
            "KCL violated at phase {}: I_t1+I_t2 = {re}+{im}j (asymmetric reactor \
             stamp transpose?)",
            ph + 1
        );
    }

    // (b) Per-conductor terminal currents vs the pinned oracle.
    let want = [
        Complex64::new(116.146461, -57.769703),
        Complex64::new(-22.612834, -20.041948),
        Complex64::new(7.672820, 240.310043),
        Complex64::new(-116.146461, 57.769703),
        Complex64::new(22.612834, 20.041948),
        Complex64::new(-7.672820, -240.310043),
    ];
    for (k, (&a, &w)) in c.iter().zip(want.iter()).enumerate() {
        assert!(
            (a.re - w.re).abs() <= 1e-3 + 1e-6 * w.re.abs()
                && (a.im - w.im).abs() <= 1e-3 + 1e-6 * w.im.abs(),
            "reactor current [{k}]: got {a}, oracle {w}"
        );
    }
}

// --- WPG.21 — TReactorObj.MakePosSequence (Reactor.pas:1052-1115) -------------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, balance};
use crate::util::EPSILON;

/// SpecType 1 (kvar): kvar/3 per phase, kV = kVRating/√3 for a multi-phase wye
/// (deck `rx_kvar`: 200 kvar → 66.67, 12.47 kV → 7.1996).
#[test]
fn make_pos_sequence_kvar() {
    let mut r = Reactor::new("rx_kvar");
    r.spec_type = ReactorSpecType::Kvar;
    r.kvarrating = 200.0;
    r.kvrating = 12.47;
    r.connection = 0; // wye

    let plan = r.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    use PosSeqAction::*;
    assert_eq!(plan.actions[0], BeginEdit);
    assert_eq!(plan.actions[1], SetI32(prop::PHASES, 1));
    match (plan.actions[2].clone(), plan.actions[3].clone()) {
        (SetF64(kvi, kv), SetF64(kvari, kvar)) => {
            assert_eq!(kvi, prop::KV);
            assert!((kv - 12.47 / sqrt3()).abs() < 1e-9, "kv {kv}");
            assert!((kv - 7.199_558).abs() < 1e-5);
            assert_eq!(kvari, prop::KVAR);
            assert!((kvar - 200.0 / 3.0).abs() < 1e-12, "kvar {kvar}");
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(plan.actions[4], EndEdit);
    assert_eq!(plan.actions.len(), 5);
}

/// A single-phase wye kvar reactor keeps the line-line kV (no /√3).
#[test]
fn make_pos_sequence_kvar_single_phase_wye_keeps_kv() {
    let mut r = Reactor::new("rx");
    r.cd.nphases = 1;
    r.spec_type = ReactorSpecType::Kvar;
    r.kvrating = 12.47;
    r.connection = 0; // wye

    let plan = r.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(plan.actions[2], SetF64(prop::KV, 12.47));
}

/// SpecType 2 (R+jX) and 4 (Z1) only set `Phases := 1`.
#[test]
fn make_pos_sequence_rx_and_z1_just_set_phases() {
    for st in [ReactorSpecType::RplusJx, ReactorSpecType::SymComponents] {
        let mut r = Reactor::new("rx");
        r.spec_type = st;
        let plan = r.make_pos_sequence(&PosSeqCtx::default());
        use PosSeqAction::*;
        assert_eq!(
            plan.actions,
            vec![BeginEdit, SetI32(prop::PHASES, 1), EndEdit],
            "spec_type {st:?}"
        );
        assert!(plan.run_base);
    }
}

/// SpecType 3 (matrices): the series form reduces to `1 / S(Y)` with
/// `Y = (R + jX)⁻¹`, the positive-sequence self term of the admittance the
/// element stamps. For the balanced deck `rx_mat` that is
/// `(Rs − Rm) + j(Xs − Xm)` = 0.8 + j9 Ω. Both oracles write 0.26667 + j3, a
/// third of it, from a mutual sum that skips the first row.
#[test]
fn make_pos_sequence_matrix() {
    let mut r = Reactor::new("rx_mat");
    r.spec_type = ReactorSpecType::Matrices;
    r.rmatrix = Some(vec![1.0, 0.2, 0.2, 0.2, 1.0, 0.2, 0.2, 0.2, 1.0]);
    r.xmatrix = Some(vec![12.0, 3.0, 3.0, 3.0, 12.0, 3.0, 3.0, 3.0, 12.0]);

    let plan = r.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(plan.actions[0], BeginEdit);
    assert_eq!(plan.actions[1], SetI32(prop::PHASES, 1));
    match (plan.actions[2].clone(), plan.actions[3].clone()) {
        (SetF64(ri, r1), SetF64(xi, x1)) => {
            assert_eq!(ri, prop::R);
            balance::assert_rel(r1, 0.8, 1e-12, "R");
            assert_eq!(xi, prop::X);
            balance::assert_rel(x1, 9.0, 1e-12, "X");
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(plan.actions[4], EndEdit);
    assert_eq!(plan.actions.len(), 5);
}

/// The positive-sequence impedance `1 / S((R + jX)⁻¹)` of a series-form
/// matrix reactor, from the definition of `S` ([`balance::seq11`]).
fn z1_of(r: &[f64], x: &[f64], n: usize) -> Complex64 {
    1.0 / balance::seq11(&balance::inverse(&balance::complex(r, x), n), n)
}

/// The power (kW + j kvar) a positive-sequence shunt admittance `y1` draws at
/// the solved phase voltage of bus `b`: `3 |V1|² conj(y1)`.
fn shunt_kva(dss: &crate::exec::Dss, y1: Complex64) -> Complex64 {
    let v = balance::v1(dss, "b");
    3.0 * v * v * y1.conj() / 1000.0
}

/// `a` equals `b` within [`balance::BALANCE_REL`].
fn assert_c(a: Complex64, b: Complex64, what: &str) {
    assert!(
        (a - b).norm() <= balance::BALANCE_REL * b.norm(),
        "{what}: {a} vs {b}"
    );
}

const R_BAL: [f64; 9] = [1.0, 0.2, 0.2, 0.2, 1.0, 0.2, 0.2, 0.2, 1.0];
const X_BAL: [f64; 9] = [10.0, 1.0, 1.0, 1.0, 10.0, 1.0, 1.0, 1.0, 10.0];

/// A balanced shunt matrix reactor keeps its power through `makeposseq`
/// (about 1523.77 + j17142.43 kVA): `R + jX` = 0.8 + j9 Ω. A twin that also
/// carries `rp=500`, which the matrix stamp does not use, draws the same power
/// before the reduction and keeps it after: the reduction writes `rp` 0, so
/// the R + jX element it becomes stamps no parallel conductance either
/// (without that the twin would gain `3 |V1|² / 500`, about 311 kW).
#[test]
fn make_pos_sequence_matrix_shunt_keeps_the_reactors_power() {
    let z1 = z1_of(&R_BAL, &X_BAL, 3);
    assert_c(z1, Complex64::new(0.8, 9.0), "1 / S(Y)");

    let mut dss = balance::stiff("rsh");
    let m = "rmatrix=[1 | 0.2 1 | 0.2 0.2 1] xmatrix=[10 | 1 10 | 1 1 10]";
    dss.command(&format!("new reactor.rm bus1=b phases=3 {m}"));
    dss.command(&format!("new reactor.rmp bus1=b phases=3 {m} rp=500"));
    let pairs = balance::reduce(&mut dss, &["Reactor.rm", "Reactor.rmp"]);
    let [(rm0, rm1), (rmp0, rmp1)] = pairs[..] else {
        unreachable!()
    };
    balance::assert_kept(rm0, rm1, "Reactor.rm");
    assert_c(rmp0, rm0, "the rp twin before makeposseq");
    balance::assert_kept(rmp0, rmp1, "Reactor.rmp");
    assert_c(
        rm1,
        shunt_kva(&dss, 1.0 / z1),
        "power vs 3 |V1|² conj(1/Z1)",
    );
    for name in ["rm", "rmp"] {
        balance::assert_rel(
            balance::query_f64(&mut dss, &format!("Reactor.{name}.r")),
            0.8,
            1e-12,
            name,
        );
        balance::assert_rel(
            balance::query_f64(&mut dss, &format!("Reactor.{name}.x")),
            9.0,
            1e-12,
            name,
        );
    }
    assert_eq!(
        balance::query_f64(&mut dss, "Reactor.rmp.rp"),
        0.0,
        "rmp rp"
    );
}

/// The same balanced matrices in series into a 40 + j30 Ω wye load: the
/// reactor's loss (about 39.05 + j439.32 kVA) and the load's power (about
/// 1952.52 + j1464.39 kVA) are unchanged by `makeposseq`.
#[test]
fn make_pos_sequence_matrix_series_keeps_loss_and_load() {
    let mut dss = balance::stiff("rse");
    dss.command(
        "new reactor.rser bus1=b bus2=c phases=3 \
         rmatrix=[1 | 0.2 1 | 0.2 0.2 1] xmatrix=[10 | 1 10 | 1 1 10]",
    );
    dss.command("new reactor.ld bus1=c phases=3 r=40 x=30");
    balance::solve_clean(&mut dss, "before makeposseq");
    let loss0 = balance::loss(&mut dss, "Reactor.rser");
    let load0 = balance::power(&mut dss, "Reactor.ld");
    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    balance::assert_kept(
        loss0,
        balance::loss(&mut dss, "Reactor.rser"),
        "series loss",
    );
    balance::assert_kept(load0, balance::power(&mut dss, "Reactor.ld"), "load power");
}

/// An unbalanced shunt matrix reactor keeps its power exactly under the
/// admittance rule: `R=[1.2|.3 1|.1 .2 .8] X=[12|3 10|1 2 8]` reduces to
/// `1 / S(Y)` ≈ 0.76778116 + j7.67781155 Ω.
#[test]
fn make_pos_sequence_unbalanced_matrix_shunt_keeps_the_reactors_power() {
    let r = [1.2, 0.3, 0.1, 0.3, 1.0, 0.2, 0.1, 0.2, 0.8];
    let x = [12.0, 3.0, 1.0, 3.0, 10.0, 2.0, 1.0, 2.0, 8.0];
    let z1 = z1_of(&r, &x, 3);
    assert!(
        (z1 - Complex64::new(0.767_781_16, 7.677_811_55)).norm() < 1e-8,
        "{z1}"
    );

    let mut dss = balance::stiff("rshu");
    dss.command(
        "new reactor.ru bus1=b phases=3 rmatrix=[1.2 | 0.3 1.0 | 0.1 0.2 0.8] \
         xmatrix=[12 | 3 10 | 1 2 8]",
    );
    let [(before, after)] = balance::reduce(&mut dss, &["Reactor.ru"])[..] else {
        unreachable!()
    };
    balance::assert_kept(before, after, "Reactor.ru");
    assert_c(
        after,
        shunt_kva(&dss, 1.0 / z1),
        "power vs 3 |V1|² conj(1/Z1)",
    );
    balance::assert_rel(
        balance::query_f64(&mut dss, "Reactor.ru.r"),
        z1.re,
        1e-12,
        "r",
    );
    balance::assert_rel(
        balance::query_f64(&mut dss, "Reactor.ru.x"),
        z1.im,
        1e-12,
        "x",
    );
}

/// A `Parallel=yes` matrix reactor stamps `R⁻¹ + (jX)⁻¹` and reduces to the
/// same parallel form: `r` 0, `x = −1 / S(−X⁻¹)`, `rp = 1 / S(R⁻¹)`. Diagonal
/// R 100 and X 10 give `x` 10 and `rp` 100, and the reactor keeps its power
/// (about 1555.01 + j15550.09 kVA). Both oracles turn it into a series
/// R + jX, mostly resistive.
#[test]
fn make_pos_sequence_parallel_matrix_keeps_the_reactors_power() {
    let mut dss = balance::stiff("rpar");
    dss.command(
        "new reactor.rp bus1=b phases=3 parallel=yes \
         rmatrix=[100 | 0 100 | 0 0 100] xmatrix=[10 | 0 10 | 0 0 10]",
    );
    let [(before, after)] = balance::reduce(&mut dss, &["Reactor.rp"])[..] else {
        unreachable!()
    };
    balance::assert_kept(before, after, "Reactor.rp");
    assert_c(
        after,
        shunt_kva(&dss, Complex64::new(1.0 / 100.0, -1.0 / 10.0)),
        "power vs 3 |V1|² (1/rp + j/x)",
    );
    assert_eq!(balance::query_f64(&mut dss, "Reactor.rp.r"), 0.0, "r");
    balance::assert_rel(
        balance::query_f64(&mut dss, "Reactor.rp.x"),
        10.0,
        1e-12,
        "x",
    );
    balance::assert_rel(
        balance::query_f64(&mut dss, "Reactor.rp.rp"),
        100.0,
        1e-12,
        "rp",
    );
}

/// An unbalanced, coupled `Parallel=yes` matrix reactor reduces to the self
/// terms of the admittances it stamps, `x = −1 / S(B)` and `rp = 1 / S(G)` with
/// `G = R⁻¹` and `B = −X⁻¹` computed here from their definition, and keeps its
/// power. The self terms of `X` and `R` themselves, or the first phase's `X11`
/// and `R11`, miss that power by 4 % and 15 %. On two phases the self term is
/// that of the admittance embedded in three phases: diagonal R 100 and X 10 on
/// `b.1.2` write `x` 15 and `rp` 150, where one phase holds 10 and 100.
#[test]
fn make_pos_sequence_parallel_matrix_takes_the_admittance_self_terms() {
    let zero = [0.0; 9];
    let r = [80.0, 10.0, 5.0, 10.0, 120.0, 8.0, 5.0, 8.0, 100.0];
    let x = [12.0, 1.0, 2.0, 1.0, 9.0, 1.0, 2.0, 1.0, 15.0];
    let g = balance::inverse(&balance::complex(&r, &zero), 3);
    let b: Vec<Complex64> = balance::inverse(&balance::complex(&x, &zero), 3)
        .into_iter()
        .map(|y| -y)
        .collect();
    let rp = 1.0 / balance::seq11(&g, 3).re;
    let xp = -1.0 / balance::seq11(&b, 3).re;

    let mut dss = balance::stiff("rparu");
    dss.command(
        "new reactor.ru bus1=b phases=3 parallel=yes \
         rmatrix=[80 | 10 120 | 5 8 100] xmatrix=[12 | 1 9 | 2 1 15]",
    );
    dss.command(
        "new reactor.r2 bus1=b.1.2 phases=2 parallel=yes \
         rmatrix=[100 | 0 100] xmatrix=[10 | 0 10]",
    );
    let names = ["Reactor.ru", "Reactor.r2"];
    for ((before, after), name) in balance::reduce(&mut dss, &names).into_iter().zip(names) {
        balance::assert_kept(before, after, name);
    }
    assert_eq!(balance::query_f64(&mut dss, "Reactor.ru.r"), 0.0, "ru r");
    for (what, want) in [
        ("Reactor.ru.x", xp),
        ("Reactor.ru.rp", rp),
        ("Reactor.r2.x", 15.0),
        ("Reactor.r2.rp", 150.0),
    ] {
        balance::assert_rel(balance::query_f64(&mut dss, what), want, 1e-12, what);
    }
}

/// A two-phase matrix reactor on phases a and b reduces to `1 / S(Y)` of its
/// admittance embedded in three phases, ≈ 1.31826664 + j14.14381841 Ω for
/// `R=[1|.2 1] X=[10|1 10]`, and keeps its power. Both oracles write
/// `R = X = 0` here and the solve turns NaN.
#[test]
fn make_pos_sequence_two_phase_matrix_keeps_the_reactors_power() {
    let z1 = z1_of(&[1.0, 0.2, 0.2, 1.0], &[10.0, 1.0, 1.0, 10.0], 2);
    assert!(
        (z1 - Complex64::new(1.318_266_64, 14.143_818_41)).norm() < 1e-8,
        "{z1}"
    );

    let mut dss = balance::stiff("r2ph");
    dss.command("new reactor.r2 bus1=b.1.2 phases=2 rmatrix=[1 | 0.2 1] xmatrix=[10 | 1 10]");
    let [(before, after)] = balance::reduce(&mut dss, &["Reactor.r2"])[..] else {
        unreachable!()
    };
    balance::assert_kept(before, after, "Reactor.r2");
    assert_c(
        after,
        shunt_kva(&dss, 1.0 / z1),
        "power vs 3 |V1|² conj(1/Z1)",
    );
}

/// A balanced 4x4 matrix of diagonal `d` and off-diagonal `m`, row-major.
fn balanced4(d: f64, m: f64) -> Vec<f64> {
    (0..16)
        .map(|k| if k / 4 == k % 4 { d } else { m })
        .collect()
}

/// The unbalanced 4x4 `rmatrix` of the four-conductor pins, row-major.
const R4_UNBAL: [f64; 16] = [
    1.2, 0.3, 0.1, 0.1, 0.3, 1.0, 0.2, 0.1, 0.1, 0.2, 0.8, 0.1, 0.1, 0.1, 0.1, 0.9,
];

/// The unbalanced 4x4 `xmatrix` of the four-conductor pins, ten times
/// [`R4_UNBAL`].
const X4_UNBAL: [f64; 16] = [
    12.0, 3.0, 1.0, 1.0, 3.0, 10.0, 2.0, 1.0, 1.0, 2.0, 8.0, 1.0, 1.0, 1.0, 1.0, 9.0,
];

/// The actions of a four-conductor matrix reactor's reduction.
fn reduce4(rm: Vec<f64>, xm: Vec<f64>, parallel: bool, rp: Option<f64>) -> Vec<PosSeqAction> {
    let mut r = Reactor::new("q4");
    r.cd.nphases = 4;
    r.spec_type = ReactorSpecType::Matrices;
    r.is_parallel = parallel;
    r.rmatrix = Some(rm);
    r.xmatrix = Some(xm);
    if let Some(rp) = rp {
        r.rp = rp;
        r.rp_specified = true;
    }
    r.make_pos_sequence(&PosSeqCtx::default()).actions
}

/// Above three conductors `rmatrix` and `xmatrix` each reduce to the mean of
/// the diagonal minus the mean over all off-diagonal pairs (user ruling
/// 2026-10-04, read literally on the two matrices). Balanced 4x4 gives
/// 0.8 / 9. The unbalanced matrices give 0.825 / 8.25, where the admittance
/// rule would give about 0.79391 + j7.93912. A twin carrying `rp=500`, which
/// the matrix stamp never reads, writes `rp` 0 as at three conductors.
#[test]
fn make_pos_sequence_four_conductor_matrix_takes_mean_self_minus_mean_mutual() {
    let reduce = |rm: Vec<f64>, xm: Vec<f64>| match reduce4(rm, xm, false, None)[2..4] {
        [PosSeqAction::SetF64(ri, r1), PosSeqAction::SetF64(xi, x1)]
            if ri == prop::R && xi == prop::X =>
        {
            (r1, x1)
        }
        ref other => panic!("unexpected {other:?}"),
    };
    let (r1, x1) = reduce(balanced4(1.0, 0.2), balanced4(10.0, 1.0));
    balance::assert_rel(r1, 0.8, 1e-12, "balanced R");
    balance::assert_rel(x1, 9.0, 1e-12, "balanced X");
    let (r1, x1) = reduce(R4_UNBAL.to_vec(), X4_UNBAL.to_vec());
    balance::assert_rel(r1, 0.825, 1e-12, "unbalanced R");
    balance::assert_rel(x1, 8.25, 1e-12, "unbalanced X");

    let actions = reduce4(
        balanced4(1.0, 0.2),
        balanced4(10.0, 1.0),
        false,
        Some(500.0),
    );
    assert_eq!(
        actions[4..],
        [PosSeqAction::SetF64(prop::RP, 0.0), PosSeqAction::EndEdit],
        "the rp twin: {actions:?}"
    );
}

/// Above three conductors a `Parallel=yes` matrix reactor reduces its two
/// matrices by the same rule into its parallel form: `r` 0, `x = S(xmatrix)`
/// and `rp = S(rmatrix)`. Balanced R 100/10 and X 10/1 give `x` 9 and `rp` 90.
/// The unbalanced matrices of
/// `make_pos_sequence_four_conductor_matrix_takes_mean_self_minus_mean_mutual`,
/// with R times 100, give `x` 8.25 and `rp` 82.5, where the rule on the
/// admittances `G = R⁻¹` and `B = −X⁻¹` would give about 7.93912 and 79.3912
/// (the two rules agree on a balanced matrix).
#[test]
fn make_pos_sequence_four_conductor_parallel_matrix_takes_mean_self_minus_mean_mutual() {
    let reduce = |rm: Vec<f64>, xm: Vec<f64>| match reduce4(rm, xm, true, None)[..] {
        [
            PosSeqAction::BeginEdit,
            PosSeqAction::SetI32(pi, 1),
            PosSeqAction::SetF64(ri, r),
            PosSeqAction::SetF64(xi, x),
            PosSeqAction::SetF64(rpi, rp),
            PosSeqAction::EndEdit,
        ] if pi == prop::PHASES && ri == prop::R && xi == prop::X && rpi == prop::RP => (r, x, rp),
        ref other => panic!("unexpected {other:?}"),
    };
    let (r, x, rp) = reduce(balanced4(100.0, 10.0), balanced4(10.0, 1.0));
    assert_eq!(r, 0.0, "balanced r");
    balance::assert_rel(x, 9.0, 1e-12, "balanced x");
    balance::assert_rel(rp, 90.0, 1e-12, "balanced rp");
    let (r, x, rp) = reduce(
        R4_UNBAL.iter().map(|v| 100.0 * v).collect(),
        X4_UNBAL.to_vec(),
    );
    assert_eq!(r, 0.0, "unbalanced r");
    balance::assert_rel(x, 8.25, 1e-12, "unbalanced x");
    balance::assert_rel(rp, 82.5, 1e-12, "unbalanced rp");
}

/// A one-phase symmetrical-component reactor stamps `Z1`: `phases=1
/// z1=[1, 12] z0=[3, 20]` has the YPrim of its `r=1 x=12` twin. A three-phase
/// Z1 reactor keeps its power through `makeposseq` within 5e-7: the
/// three-phase stamp rotates by the truncated `CALPHA` of `Reactor::calc_yprim`
/// (`|CALPHA − e^(−j2π/3)|` = 4.04e-7), which moves its equivalent `Z1` by
/// about 2.3e-7, and the reduced one-phase stamp is exact.
#[test]
fn a_single_phase_z1_reactor_stamps_z1() {
    let mut dss = balance::stiff("rz1");
    dss.command("new reactor.z1r bus1=b.1 phases=1 z1=[1, 12] z0=[3, 20]");
    dss.command("new reactor.rx bus1=b.1 phases=1 r=1 x=12");
    balance::solve_clean(&mut dss, "one-phase Z1");
    let (oa, ya) = dss.element_yprim("Reactor.z1r").expect("z1r YPrim");
    let (ob, yb) = dss.element_yprim("Reactor.rx").expect("rx YPrim");
    assert_eq!(oa, ob);
    for (i, (a, b)) in ya.iter().zip(&yb).enumerate() {
        assert!(
            (a - b).norm() <= 1e-12 * b.norm(),
            "YPrim[{i}]: z1 {a} vs r/x {b}"
        );
    }

    let mut dss = balance::stiff("rz3");
    dss.command("new reactor.z3 bus1=b phases=3 z1=[1, 12] z0=[3, 20]");
    let [(before, after)] = balance::reduce(&mut dss, &["Reactor.z3"])[..] else {
        unreachable!()
    };
    assert!(
        (after - before).norm() <= 5e-7 * before.norm(),
        "three-phase Z1 reactor: {before} before, {after} after"
    );
}

/// A closed three-phase delta reactor given by `r`/`x` or by `kvar`, each with
/// or without `rp`, reduces to its wye equivalent, a third of the leg
/// impedance and of the leg `rp`, and keeps `3 |V_LL|² / conj(z_leg)` (plus
/// `3 |V_LL|² / rp`). A second `makeposseq` changes nothing.
///
/// The guard half: with the property order `conn=delta phases=3` a reactor has
/// four conductors (legs 1-2, 2-3 and 3 to ground, so it draws
/// `7 |V1|² / conj(z_leg)`), not a closed delta, and its leg `r` and `x` are
/// not divided by three. Nor is a two-phase delta reactor given by `r`/`x` or
/// by `kvar`, each with `rp` (three conductors, legs 1-2 and 2 to ground, so it
/// draws `4 |V1|² (1 / conj(z_leg) + 1 / rp)`): its leg `r` and `rp`, and the
/// `x` of the `r`/`x` form, are not divided by three. How those elements
/// should reduce is a separate open question (the four-conductor order, and
/// question 1 for two phases), so only the guard is pinned.
#[test]
fn make_pos_sequence_delta_reactor_keeps_its_power() {
    let mut dss = balance::stiff("rdel");
    dss.command("new reactor.rdx bus1=b phases=3 conn=delta r=2.4 x=27");
    dss.command("new reactor.rdp bus1=b phases=3 conn=delta r=2.4 x=27 rp=3000");
    dss.command("new reactor.rdk bus1=b phases=3 conn=delta kvar=17277.89 kv=12.47 r=2.4");
    dss.command("new reactor.rdkp bus1=b phases=3 conn=delta kvar=17277.89 kv=12.47 r=2.4 rp=3000");
    dss.command("new reactor.d4 bus1=b conn=delta phases=3 r=2.4 x=27");
    dss.command("new reactor.d4k bus1=b conn=delta phases=3 kvar=17277.89 kv=12.47 r=2.4");
    dss.command("new reactor.d2 bus1=b.1.2 phases=2 conn=delta r=2.4 x=27 rp=3000");
    dss.command(
        "new reactor.d2k bus1=b.1.2 phases=2 conn=delta kvar=11518.59 kv=12.47 r=2.4 rp=3000",
    );
    let all = [
        "Reactor.rdx",
        "Reactor.rdp",
        "Reactor.rdk",
        "Reactor.rdkp",
        "Reactor.d4",
        "Reactor.d4k",
        "Reactor.d2",
        "Reactor.d2k",
    ];
    let pairs = balance::reduce(&mut dss, &all);
    let names = &all[..4];
    let v1 = balance::v1(&dss, "b");
    let v_ll = 3f64.sqrt() * v1;
    let x_kvar = 3.0 * 12.47 * 12.47 * 1000.0 / 17277.89;
    let legs = [
        (Complex64::new(2.4, 27.0), None),
        (Complex64::new(2.4, 27.0), Some(3000.0)),
        (Complex64::new(2.4, x_kvar), None),
        (Complex64::new(2.4, x_kvar), Some(3000.0)),
    ];
    for ((name, (before, after)), (z_leg, rp)) in names.iter().zip(&pairs).zip(legs) {
        balance::assert_kept(*before, *after, name);
        let mut want = 3.0 * v_ll * v_ll / z_leg.conj() / 1000.0;
        if let Some(rp) = rp {
            want += 3.0 * v_ll * v_ll / rp / 1000.0;
        }
        assert_c(*after, want, name);
    }
    for name in ["rdp", "rdkp"] {
        balance::assert_rel(
            balance::query_f64(&mut dss, &format!("Reactor.{name}.rp")),
            1000.0,
            1e-12,
            &format!("{name} rp"),
        );
    }

    for (k, z_leg) in [
        (4, Complex64::new(2.4, 27.0)),
        (5, Complex64::new(2.4, x_kvar)),
    ] {
        let (name, before) = (all[k], pairs[k].0);
        assert_c(before, 7.0 * v1 * v1 / z_leg.conj() / 1000.0, name);
    }
    for (what, want) in [
        ("Reactor.d4.r", 2.4),
        ("Reactor.d4.x", 27.0),
        ("Reactor.d4k.r", 2.4),
    ] {
        balance::assert_rel(balance::query_f64(&mut dss, what), want, 1e-12, what);
    }
    // Two phases: the leg X of the kvar form is kV² over the kvar per phase.
    let x_kvar2 = 2.0 * 12.47 * 12.47 * 1000.0 / 11518.59;
    for (k, z_leg) in [
        (6, Complex64::new(2.4, 27.0)),
        (7, Complex64::new(2.4, x_kvar2)),
    ] {
        let (name, before) = (all[k], pairs[k].0);
        let want = 4.0 * v1 * v1 * (1.0 / z_leg.conj() + 1.0 / 3000.0) / 1000.0;
        assert_c(before, want, name);
    }
    for (what, want) in [
        ("Reactor.d2.r", 2.4),
        ("Reactor.d2.x", 27.0),
        ("Reactor.d2.rp", 3000.0),
        ("Reactor.d2k.r", 2.4),
        ("Reactor.d2k.rp", 3000.0),
    ] {
        balance::assert_rel(balance::query_f64(&mut dss, what), want, 1e-12, what);
    }

    let yprims: Vec<_> = names
        .iter()
        .map(|n| dss.element_yprim(n).expect("YPrim"))
        .collect();
    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    for (name, (order, y)) in names.iter().zip(&yprims) {
        let (order2, y2) = dss.element_yprim(name).expect("YPrim");
        assert_eq!(
            order2, *order,
            "{name}: YPrim order after a second makeposseq"
        );
        for (a, b) in y2.iter().zip(y) {
            assert!(
                (a - b).norm() <= 1e-12 * b.norm(),
                "{name}: a second makeposseq moved the one-phase reactor: {a} vs {b}"
            );
        }
    }
}

/// Whether an error text names `full` and contains `phrase`.
fn refuses(dss: &crate::exec::Dss, full: &str, phrase: &str) -> bool {
    refuses_since(dss, 0, full, phrase)
}

/// Whether an error text logged from the `from`-th on names `full` and
/// contains `phrase`.
fn refuses_since(dss: &crate::exec::Dss, from: usize, full: &str, phrase: &str) -> bool {
    dss.error_texts()[from..]
        .iter()
        .any(|t| t.to_ascii_lowercase().contains(&full.to_ascii_lowercase()) && t.contains(phrase))
}

/// A delta matrix or symmetrical-component reactor of two or more phases has
/// no defined stamp: the solve refuses it with a message naming the element,
/// its phase count, the form it is given by and the remedy `conn=wye`, instead
/// of stamping out of range, and the reports after the refusal read the
/// circuit as not solved. `makeposseq` leaves it as it is, so the solve after
/// it names it again. A one-phase delta matrix is a single leg and stamps
/// exactly like its `r`/`x` twin.
#[test]
fn a_delta_matrix_reactor_refuses_the_solve() {
    for (name, def, phases, form) in [
        (
            "rd3",
            "bus1=b phases=3 conn=delta rmatrix=[1 | 0.2 1 | 0.2 0.2 1] \
             xmatrix=[10 | 1 10 | 1 1 10]",
            "3",
            "rmatrix/xmatrix",
        ),
        (
            "rd2",
            "bus1=b.1.2 phases=2 conn=delta rmatrix=[1 | 0.2 1] xmatrix=[10 | 1 10]",
            "2",
            "rmatrix/xmatrix",
        ),
        (
            "rdp",
            "bus1=b phases=3 conn=delta parallel=yes rmatrix=[100 | 0 100 | 0 0 100] \
             xmatrix=[10 | 0 10 | 0 0 10]",
            "3",
            "rmatrix/xmatrix",
        ),
        (
            "rdz",
            "bus1=b phases=3 conn=delta z1=[1, 12] z0=[3, 20]",
            "3",
            "z1/z2/z0",
        ),
    ] {
        let full = format!("Reactor.{name}");
        let message = format!(
            "{full}: a reactor of {phases} phases given by {form} cannot be connected in \
             delta. Specify it with conn=wye. Aborting solution."
        );
        let mut dss = balance::stiff_metered("rdm");
        dss.command(&format!("new reactor.{name} {def}"));
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve");
        assert!(
            refuses(&dss, &full, &message),
            "{name}: {:?}",
            dss.error_texts()
        );
        assert!(!dss.circuit().unwrap().is_solved, "{name}: solved");
        balance::reports_after_refusal(&mut dss, &message, name);

        dss.command("makeposseq");
        dss.command(&format!("? reactor.{name}.phases"));
        assert_eq!(dss.result(), phases, "{name} keeps its phases");
        let n_before = dss.error_texts().len();
        dss.command("solve");
        assert!(
            refuses_since(&dss, n_before, &full, &message),
            "{name}: no refusal after makeposseq: {:?}",
            &dss.error_texts()[n_before..]
        );
        assert!(
            !dss.circuit().unwrap().is_solved,
            "{name}: solved after makeposseq"
        );
    }

    let mut dss = balance::stiff("rd1");
    dss.command("new reactor.rd1 bus1=b.1.2 phases=1 conn=delta rmatrix=[1] xmatrix=[10]");
    dss.command("new reactor.rref bus1=b.1.2 phases=1 conn=delta r=1 x=10");
    balance::solve_clean(&mut dss, "one-phase delta matrix");
    let (a, b) = (
        balance::power(&mut dss, "Reactor.rd1"),
        balance::power(&mut dss, "Reactor.rref"),
    );
    balance::assert_kept(b, a, "one-phase delta matrix vs its r/x twin");
}

/// A series-form matrix reactor given only `xmatrix` or only `rmatrix` has no
/// stamp: the solve refuses it with a message naming the missing matrix, the
/// reports after the refusal read the circuit as not solved, and `makeposseq`
/// leaves it unreduced, so the solve after it names it again. None of them
/// panics.
#[test]
fn a_matrix_reactor_without_both_matrices_refuses_the_solve() {
    for (name, def, missing) in [
        ("rx_only", "xmatrix=[10 | 1 10 | 1 1 10]", "rmatrix"),
        ("rr_only", "rmatrix=[1 | 0.2 1 | 0.2 0.2 1]", "xmatrix"),
    ] {
        let full = format!("Reactor.{name}");
        let missing = format!(
            "{full}: {missing} is missing. A reactor given by matrices needs both rmatrix and \
             xmatrix. Aborting solution."
        );
        let mut dss = balance::stiff_metered("rmiss");
        dss.command(&format!("new reactor.{name} bus1=b phases=3 {def}"));
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve");
        assert!(
            refuses(&dss, &full, &missing),
            "{name}: {:?}",
            dss.error_texts()
        );
        assert!(!dss.circuit().unwrap().is_solved, "{name}: solved");
        balance::reports_after_refusal(&mut dss, &missing, name);

        dss.command("makeposseq");
        dss.command(&format!("? reactor.{name}.phases"));
        assert_eq!(dss.result(), "3", "{name} is not reduced");
        let n_before = dss.error_texts().len();
        dss.command("solve");
        assert!(
            refuses_since(&dss, n_before, &full, &missing),
            "{name}: no refusal after makeposseq: {:?}",
            &dss.error_texts()[n_before..]
        );
        assert!(
            !dss.circuit().unwrap().is_solved,
            "{name}: solved after makeposseq"
        );
    }
}

/// A disabled element is out of the model: a matrix reactor the solve refused
/// is set aside by `disable`, after which the rest of the circuit solves as it
/// does without the reactor. Enabling it brings the refusal back.
#[test]
fn a_refused_reactor_set_aside_by_disable_leaves_the_solve_to_the_rest() {
    let mut twin = balance::stiff_metered("roff");
    balance::solve_clean(&mut twin, "without the reactor");
    let load = balance::power(&mut twin, "Load.ld");

    let full = "Reactor.rx_off";
    let missing = "rmatrix is missing";
    let mut dss = balance::stiff_metered("roff");
    dss.command("new reactor.rx_off bus1=c phases=3 xmatrix=[10 | 1 10 | 1 1 10]");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(refuses(&dss, full, missing), "{:?}", dss.error_texts());
    assert!(!dss.circuit().unwrap().is_solved, "solved");

    dss.command("disable reactor.rx_off");
    let n = dss.error_texts().len();
    dss.command("solve");
    assert_eq!(dss.error_texts().len(), n, "{:?}", &dss.error_texts()[n..]);
    assert!(
        dss.circuit().unwrap().is_solved,
        "not solved with the reactor disabled"
    );
    balance::assert_kept(
        load,
        balance::power(&mut dss, "Load.ld"),
        "beside the disabled reactor",
    );

    dss.command("enable reactor.rx_off");
    dss.command("solve");
    assert!(
        dss.error_texts()[n..]
            .iter()
            .any(|t| t.contains(full) && t.contains(missing)),
        "{:?}",
        &dss.error_texts()[n..]
    );
    assert!(
        !dss.circuit().unwrap().is_solved,
        "solved with the reactor enabled"
    );
}

/// A reactor's matrices keep the order they were given in, so after a
/// `phases=` edit to another count they no longer describe it. The solve
/// refuses the reactor with a message naming it, the matrix of another order
/// and the remedy, the series and the parallel form alike (the parallel form
/// inverts nothing on the edit), also when the edit gives `rmatrix` again and
/// leaves `xmatrix`. The reports after the refusal read the circuit as not
/// solved, and `makeposseq` leaves the reactor as it is.
#[test]
fn a_matrix_of_another_phase_count_refuses_the_solve() {
    for (form, extra) in [("mser", ""), ("mpar", " parallel=yes")] {
        for (phases, edit, held) in [
            (2, "", "rmatrix"),
            (4, "", "rmatrix"),
            (2, " rmatrix=[100 | 0 100]", "xmatrix"),
        ] {
            let name = format!("{form}{phases}{held}");
            let full = format!("Reactor.{name}");
            let message = format!(
                "{full} has {phases} phases but its {held} has 3 x 3 entries. \
                 Specify rmatrix and xmatrix for {phases} phases. Aborting solution."
            );
            let mut dss = balance::stiff_metered("rphase");
            dss.command(&format!(
                "new reactor.{name} bus1=c phases=3{extra} rmatrix=[100 | 0 100 | 0 0 100] \
                 xmatrix=[10 | 1 10 | 1 1 10]"
            ));
            dss.command(&format!("edit reactor.{name} phases={phases}{edit}"));
            assert!(dss.errors().is_empty(), "{name}: {:?}", dss.error_texts());
            dss.command("set voltagebases=[12.47]");
            dss.command("calcvoltagebases");
            dss.command("solve");
            let refused =
                |dss: &crate::exec::Dss| dss.error_texts().iter().any(|t| t.contains(&message));
            assert!(refused(&dss), "{name}: {:?}", dss.error_texts());
            assert!(!dss.circuit().unwrap().is_solved, "{name}: solved");
            balance::reports_after_refusal(&mut dss, &message, &name);

            dss.command("makeposseq");
            dss.command(&format!("? reactor.{name}.phases"));
            assert_eq!(dss.result(), phases.to_string(), "{name} is not reduced");
            let n = dss.error_texts().len();
            dss.command("solve");
            assert!(
                dss.error_texts()[n..].iter().any(|t| t.contains(&message)),
                "{name}: {:?}",
                &dss.error_texts()[n..]
            );
        }
    }
}

/// A scratch data path for the report files of one test.
fn scratch_datapath(dss: &mut crate::exec::Dss, what: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dss_reactor_{}_{what}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dss.command(&format!("set datapath=\"{}\"", dir.display()));
    dir
}

/// A series matrix reactor whose impedance `R + jX` does not invert has no
/// admittance: all-equal entries give a zero positive-sequence impedance (a
/// short for balanced current) beside a zero-sequence impedance of
/// `3 (1 + j10)` Ω, and no stamp is both. The solve refuses it with a message
/// naming the reactor, at three and four phases alike, the reports after the
/// refusal read the circuit as not solved, `makeposseq` leaves it as it is and
/// the next solve refuses again.
#[test]
fn a_singular_series_matrix_refuses_the_solve() {
    for phases in [3_usize, 4] {
        let name = format!("sing{phases}");
        let message = format!(
            "Reactor.{name}: the impedance rmatrix + j xmatrix is singular and has no \
             admittance. Specify matrices that can be inverted. Aborting solution."
        );
        let rows = |v: f64| -> String {
            (1..=phases)
                .map(|i| vec![v.to_string(); i].join(" "))
                .collect::<Vec<_>>()
                .join(" | ")
        };
        let bus = if phases == 3 { "c" } else { "c.1.2.3.0" };
        let mut dss = balance::stiff_metered("rsing");
        dss.command(&format!(
            "new reactor.{name} bus1={bus} phases={phases} rmatrix=[{}] xmatrix=[{}]",
            rows(1.0),
            rows(10.0)
        ));
        assert!(dss.errors().is_empty(), "{name}: {:?}", dss.error_texts());
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve");
        let refused =
            |dss: &crate::exec::Dss| dss.error_texts().iter().any(|t| t.contains(&message));
        assert!(refused(&dss), "{name}: {:?}", dss.error_texts());
        assert!(!dss.circuit().unwrap().is_solved, "{name}: solved");
        balance::reports_after_refusal(&mut dss, &message, &name);

        dss.command("makeposseq");
        dss.command(&format!("? reactor.{name}.phases"));
        assert_eq!(dss.result(), phases.to_string(), "{name} is not reduced");
        let n = dss.error_texts().len();
        dss.command("solve");
        assert!(
            dss.error_texts()[n..].iter().any(|t| t.contains(&message)),
            "{name}: {:?}",
            &dss.error_texts()[n..]
        );
    }
}

/// A `Parallel=yes` reactor stamps `R⁻¹` beside `−X⁻¹`, so a given `rmatrix`
/// or `xmatrix` that does not invert has no admittance. All-equal entries meet
/// every current pattern that sums to zero, the balanced positive sequence
/// included, with zero resistance or reactance: a short, where the zeroed
/// matrix the edit reports would stamp an open branch.
/// The solve refuses the reactor with a message naming it and the matrix, the
/// reports after the refusal read the circuit as not solved, `makeposseq`
/// leaves it as it is and the next solve refuses again. A matrix nobody gave
/// stamps no branch (`make_pos_sequence_four_conductor_zero_self_term_stays_finite`).
#[test]
fn a_singular_parallel_matrix_refuses_the_solve() {
    for (name, prop, edit, def) in [
        (
            "px",
            "xmatrix",
            "Error inverting X Matrix for \"px\" - B is zeroed.",
            "rmatrix=[100 | 0 100 | 0 0 100] xmatrix=[10 | 10 10 | 10 10 10]",
        ),
        (
            "pr",
            "rmatrix",
            "Error inverting R Matrix for \"pr\" - G is zeroed.",
            "rmatrix=[100 | 100 100 | 100 100 100] xmatrix=[10 | 0 10 | 0 0 10]",
        ),
    ] {
        let message = format!(
            "Reactor.{name}: {prop} is singular, so the parallel reactor has no \
             admittance. Specify an {prop} that can be inverted. Aborting solution."
        );
        let mut dss = balance::stiff_metered("rpsing");
        dss.command(&format!(
            "new reactor.{name} bus1=c phases=3 parallel=yes {def}"
        ));
        assert_eq!(dss.error_texts(), [edit], "{name} at the edit");
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve");
        let refused =
            |dss: &crate::exec::Dss| dss.error_texts().iter().any(|t| t.contains(&message));
        assert!(refused(&dss), "{name}: {:?}", dss.error_texts());
        assert!(!dss.circuit().unwrap().is_solved, "{name}: solved");
        balance::reports_after_refusal(&mut dss, &message, name);

        dss.command("makeposseq");
        dss.command(&format!("? reactor.{name}.phases"));
        assert_eq!(dss.result(), "3", "{name} is not reduced");
        let n = dss.error_texts().len();
        dss.command("solve");
        assert!(
            dss.error_texts()[n..].iter().any(|t| t.contains(&message)),
            "{name}: {:?}",
            &dss.error_texts()[n..]
        );
    }
}

/// Above three conductors a zero self term takes the stamp's open fallback, as
/// at three. A four-conductor `Parallel=yes` reactor with no `xmatrix` stamps
/// `G = R⁻¹` alone (its `B` is zeroed, with an error at the edit). It reduces
/// to `x = 1 / EPSILON` and `rp = S(rmatrix)` = 100 − 10 = 90, and draws
/// `3 |V1|² / 90` on its one phase, finite. An indefinite four-conductor
/// series matrix that inverts but has `S(R) = S(X) = 0` reduces to
/// `r = 1 / EPSILON`, `x = 0` instead of the `0 + j0` that turns the solve NaN.
#[test]
fn make_pos_sequence_four_conductor_zero_self_term_stays_finite() {
    let mut dss = balance::stiff_metered("r4zero");
    dss.command(
        "new reactor.r4 bus1=c.1.2.3.0 phases=4 parallel=yes \
         rmatrix=[100 | 10 100 | 10 10 100 | 10 10 10 100]",
    );
    assert!(
        dss.error_texts().iter().any(|t| t.contains("B is zeroed")),
        "{:?}",
        dss.error_texts()
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.circuit().unwrap().is_solved, "before makeposseq");
    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.circuit().unwrap().is_solved, "after makeposseq");
    balance::assert_rel(
        balance::query_f64(&mut dss, "Reactor.r4.x"),
        1.0 / EPSILON,
        1e-12,
        "x",
    );
    balance::assert_rel(
        balance::query_f64(&mut dss, "Reactor.r4.rp"),
        90.0,
        1e-12,
        "rp",
    );
    let v = balance::v1(&dss, "c");
    assert!(v.is_finite() && v > 7000.0, "|V1| at c: {v}");
    let p = balance::power(&mut dss, "Reactor.r4");
    balance::assert_rel(
        p.re,
        3.0 * v * v / 90.0 / 1000.0,
        1e-9,
        "kW vs 3 |V1|² / rp",
    );

    let indefinite = |d: f64| -> Vec<f64> {
        let mut m = vec![0.0; 16];
        for i in 0..4 {
            m[i * 4 + i] = d;
        }
        for (i, j) in [(0, 1), (1, 0), (2, 3), (3, 2)] {
            m[i * 4 + j] = 3.0 * d;
        }
        m
    };
    let mut r = Reactor::new("q4z");
    r.cd.nphases = 4;
    r.spec_type = ReactorSpecType::Matrices;
    r.rmatrix = Some(indefinite(1.0));
    r.xmatrix = Some(indefinite(10.0));
    let plan = r.make_pos_sequence(&PosSeqCtx::default());
    assert_eq!(
        plan.actions[2..4],
        [
            PosSeqAction::SetF64(prop::R, 1.0 / EPSILON),
            PosSeqAction::SetF64(prop::X, 0.0)
        ]
    );
}

/// The matrix stamp reads no `RCurve`/`LCurve`, the R + jX stamp a reduced
/// reactor gets does. A balanced matrix reactor with flat curves of 2 (`LCurve`)
/// and 3 (`RCurve`) draws the power of its curve-free twin, series and
/// `Parallel=yes` alike, and the reduction drops the curves, so it keeps that
/// power: `3 |V1|² conj(1 / Z1)` ≈ 1523.77 + j17142.43 kVA for the series
/// form, where keeping them would stamp `3 R` and `2 X` (1131.7 + j8488.0 kVA).
#[test]
fn make_pos_sequence_matrix_drops_the_curves_it_never_read() {
    let mut dss = balance::stiff("rcurve");
    dss.command("new xycurve.lc npts=3 xarray=[0 60 1000] yarray=[2 2 2]");
    dss.command("new xycurve.rc npts=3 xarray=[0 60 1000] yarray=[3 3 3]");
    let m = "rmatrix=[1 | 0.2 1 | 0.2 0.2 1] xmatrix=[10 | 1 10 | 1 1 10]";
    dss.command(&format!(
        "new reactor.sc bus1=b phases=3 {m} lcurve=lc rcurve=rc"
    ));
    dss.command(&format!("new reactor.s bus1=b phases=3 {m}"));
    dss.command(&format!(
        "new reactor.pc bus1=b phases=3 parallel=yes {m} lcurve=lc rcurve=rc"
    ));
    dss.command(&format!("new reactor.p bus1=b phases=3 parallel=yes {m}"));
    let names = ["Reactor.sc", "Reactor.s", "Reactor.pc", "Reactor.p"];
    let p = balance::reduce(&mut dss, &names);
    for (name, (before, after)) in names.iter().zip(&p) {
        balance::assert_kept(*before, *after, name);
    }
    assert_c(p[0].0, p[1].0, "series: the curves are not stamped");
    assert_c(p[2].0, p[3].0, "parallel: the curves are not stamped");
    let z1 = z1_of(&R_BAL, &X_BAL, 3);
    assert_c(
        p[0].1,
        shunt_kva(&dss, 1.0 / z1),
        "power vs 3 |V1|² conj(1/Z1)",
    );
    for name in ["sc", "pc"] {
        for curve in ["lcurve", "rcurve"] {
            dss.command(&format!("? reactor.{name}.{curve}"));
            assert_eq!(dss.result(), "", "{name}.{curve} after makeposseq");
        }
    }
}

/// `Export Losses` counts as no-load the loss of the `rp` branch a stamp holds
/// in parallel with a whole shunt reactor (the file prints 7 significant
/// digits). `rp=500` on an R + jX reactor is such a branch: `3 |V1|² / 500` ≈
/// 311001.8 W. A series matrix or Z1 reactor stamps no `rp`, so its no-load is
/// 0 and its I²R is its whole loss. A `Parallel=yes` matrix reactor stamps no
/// `rp` either, so its whole loss `3 |V1|² S(R⁻¹)` is I²R (whether it should
/// count as no-load is open for the user). `makeposseq` writes its resistance
/// branch as the `rp` of the R + jX element it leaves, so after the reduction
/// that same loss is no-load.
///
/// A series reactor has no no-load loss: its `rp` branch joins its two buses,
/// so at no load it carries nothing, and the whole loss `3 |V_b − V_c|² Re(y1)`
/// is I²R. That holds for a series R + jX reactor with `rp=500` and for a
/// series `Parallel=yes` matrix reactor, which `makeposseq` turns into an
/// R + jX element with `rp`, each with a load behind it.
#[test]
fn the_no_load_loss_is_the_parallel_branch_the_stamp_holds() {
    let mut dss = balance::stiff("rloss");
    let m = "rmatrix=[1 | 0.2 1 | 0.2 0.2 1] xmatrix=[10 | 1 10 | 1 1 10]";
    dss.command(&format!("new reactor.mrp bus1=b phases=3 {m} rp=500"));
    dss.command("new reactor.zrp bus1=b phases=3 z1=[1, 12] z0=[3, 20] rp=500");
    dss.command(&format!("new reactor.par bus1=b phases=3 parallel=yes {m}"));
    dss.command("new reactor.xrp bus1=b phases=3 r=0.8 x=9 rp=500");
    dss.command("new reactor.sr bus1=b bus2=c phases=3 r=0.8 x=9 rp=500");
    dss.command("new load.lc bus1=c phases=3 kv=12.47 kw=1000 kvar=500");
    dss.command(&format!(
        "new reactor.sp bus1=b bus2=d phases=3 parallel=yes {m}"
    ));
    dss.command("new load.ld bus1=d phases=3 kv=12.47 kw=1000 kvar=500");
    let dir = scratch_datapath(&mut dss, "rloss");
    let g1 = balance::seq11(
        &balance::inverse(&balance::complex(&R_BAL, &[0.0; 9]), 3),
        3,
    )
    .re;
    let close = |a: f64, b: f64, what: &str| balance::assert_rel(a, b, 1e-6, what);
    for when in ["before", "after"] {
        if when == "before" {
            balance::solve_clean(&mut dss, "before makeposseq");
        } else {
            dss.command("makeposseq");
            dss.command("solve");
            assert!(dss.circuit().unwrap().is_solved, "after makeposseq");
        }
        let v = balance::v1(&dss, "b");
        let rows = export_losses(&mut dss);
        let row = |name: &str| rows.iter().find(|(n, _)| n == name).expect(name).1;
        for name in ["reactor.mrp", "reactor.zrp"] {
            let [total, i2r, no_load] = row(name);
            assert_eq!(no_load, 0.0, "{name} {when}: no-load");
            assert_eq!(i2r, total, "{name} {when}: I²R");
        }
        let [total, i2r, no_load] = row("reactor.par");
        close(
            total,
            3.0 * v * v * g1,
            &format!("reactor.par {when}: total"),
        );
        if when == "before" {
            assert_eq!(no_load, 0.0, "reactor.par {when}: no-load");
            assert_eq!(i2r, total, "reactor.par {when}: I²R");
        } else {
            close(no_load, total, &format!("reactor.par {when}: no-load"));
            assert!(i2r.abs() < 1e-6 * total, "reactor.par {when}: I²R {i2r}");
        }
        let [_, _, no_load] = row("reactor.xrp");
        close(
            no_load,
            3.0 * v * v / 500.0,
            &format!("reactor.xrp {when}: no-load"),
        );
        // Re(y1) of the series pair: `1/(0.8 + j9) + 1/500` and `S(R⁻¹)`.
        let node1 = |bus: &str| dss.bus_voltages(bus).expect(bus).node_v[0];
        for (name, bus2, g) in [
            (
                "reactor.sr",
                "c",
                (1.0 / Complex64::new(0.8, 9.0)).re + 1.0 / 500.0,
            ),
            ("reactor.sp", "d", g1),
        ] {
            let [total, i2r, no_load] = row(name);
            let dv = (node1("b") - node1(bus2)).norm();
            close(total, 3.0 * dv * dv * g, &format!("{name} {when}: total"));
            assert_eq!(no_load, 0.0, "{name} {when}: no-load");
            assert_eq!(i2r, total, "{name} {when}: I²R");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `[total, I²R, no-load]` in W per element of `Export Losses`.
fn export_losses(dss: &mut crate::exec::Dss) -> Vec<(String, [f64; 3])> {
    dss.command("export losses");
    let text = std::fs::read_to_string(dss.last_result_file()).expect("losses file");
    text.lines()
        .skip(1)
        .map(|l| {
            let c: Vec<&str> = l.split(',').map(str::trim).collect();
            let f = |k: usize| c[k].parse::<f64>().expect(l);
            (
                c[0].trim_matches('"').to_ascii_lowercase(),
                [f(1), f(3), f(5)],
            )
        })
        .collect()
}

/// The no-load loss of a delta shunt reactor is the loss of its `rp` at the
/// voltage the stamp puts across it: `rp` sits beside each leg, so a closed
/// delta loses `Σ |V_i − V_j|² / rp` = `3 |V_LL|² / rp` over its three legs,
/// and the two-phase delta (legs 1-2 and 2 to ground) `(|V1 − V2|² + |V2|²) /
/// rp`. Its total is the legs' `Σ |V_leg|² (Re(1 / z_leg) + 1 / rp)`, so I²R is
/// `Σ |V_leg|² Re(1 / z_leg)`. The wye twin of the closed delta, a third of the
/// leg values, loses the same. `makeposseq` writes a closed delta as that wye
/// one phase, which keeps total and no-load. The two-phase delta reduces to
/// one leg from node 1 to ground (its total moves, question 1 of the row) and
/// loses `3 |V1|² / rp` there. Every value is derived from the solved node
/// voltages, and the file prints 7 significant digits.
#[test]
fn a_delta_reactor_loses_its_rp_across_the_legs() {
    let mut dss = balance::stiff("rdrp");
    dss.command("new reactor.dk bus1=b phases=3 conn=delta kvar=17277.89 kv=12.47 r=2.4 rp=3000");
    dss.command("new reactor.dx bus1=b phases=3 conn=delta r=2.4 x=27 rp=3000");
    dss.command("new reactor.d2 bus1=b.1.2 phases=2 conn=delta r=2.4 x=27 rp=3000");
    dss.command("new reactor.wx bus1=b phases=3 r=0.8 x=9 rp=1000");
    let dir = scratch_datapath(&mut dss, "rdrp");
    let close = |a: f64, b: f64, what: &str| balance::assert_rel(a, b, 1e-6, what);
    let g_leg = (1.0 / Complex64::new(2.4, 27.0)).re;
    let x_kvar = 3.0 * 12.47 * 12.47 * 1000.0 / 17277.89;
    let g_kvar = (1.0 / Complex64::new(2.4, x_kvar)).re;

    balance::solve_clean(&mut dss, "before makeposseq");
    let v = dss.bus_voltages("b").expect("bus b").node_v.clone();
    let sq = |a: Complex64| a.norm_sqr();
    let closed = sq(v[0] - v[1]) + sq(v[1] - v[2]) + sq(v[2] - v[0]);
    let v_ll = 3f64.sqrt() * balance::v1(&dss, "b");
    close(closed, 3.0 * v_ll * v_ll, "Σ |V_leg|² of the closed delta");
    let two = sq(v[0] - v[1]) + sq(v[1]);
    let rows = export_losses(&mut dss);
    let row = |name: &str| rows.iter().find(|(n, _)| n == name).expect(name).1;
    let mut kept = Vec::new();
    for (name, sum, g) in [
        ("reactor.dk", closed, g_kvar),
        ("reactor.dx", closed, g_leg),
        ("reactor.d2", two, g_leg),
    ] {
        let [total, i2r, no_load] = row(name);
        close(no_load, sum / 3000.0, &format!("{name} before: no-load"));
        close(
            total,
            sum * (g + 1.0 / 3000.0),
            &format!("{name} before: total"),
        );
        close(i2r, sum * g, &format!("{name} before: I²R"));
        kept.push((name, total, no_load));
    }
    let [_, _, wye] = row("reactor.wx");
    close(row("reactor.dx")[2], wye, "dx vs its wye twin: no-load");

    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert!(dss.circuit().unwrap().is_solved, "after makeposseq");
    let v1 = balance::v1(&dss, "b");
    let rows = export_losses(&mut dss);
    let row = |name: &str| rows.iter().find(|(n, _)| n == name).expect(name).1;
    for (name, total, no_load) in &kept[..2] {
        let [t, _, nl] = row(name);
        close(t, *total, &format!("{name} after: total"));
        close(nl, *no_load, &format!("{name} after: no-load"));
    }
    let [total, i2r, no_load] = row("reactor.d2");
    let leg = 3.0 * v1 * v1;
    close(no_load, leg / 3000.0, "reactor.d2 after: no-load");
    close(
        total,
        leg * (g_leg + 1.0 / 3000.0),
        "reactor.d2 after: total",
    );
    close(i2r, leg * g_leg, "reactor.d2 after: I²R");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The `rp` of a wye reactor sits between its two terminals, so its no-load
/// loss is `|V_t1 − V_t2|² / rp`. A one-phase reactor from node 1 to node 2 of
/// one bus, a shunt by its bus names, sees the line-to-line voltage and loses
/// three times its twin from node 1 to ground. Each total is
/// `|V|² (Re(1 / z) + 1 / rp)` over the same voltage, so I²R is `|V|² Re(1 / z)`.
/// Every value is derived from the solved node voltages, and the file prints
/// 7 significant digits.
#[test]
fn a_wye_reactor_loses_its_rp_between_its_terminals() {
    let mut dss = balance::stiff("rwrp");
    dss.command("new reactor.wf bus1=b.1 bus2=b.2 phases=1 r=0.8 x=9 rp=1000");
    dss.command("new reactor.wg bus1=b.1 phases=1 r=0.8 x=9 rp=1000");
    let dir = scratch_datapath(&mut dss, "rwrp");
    let close = |a: f64, b: f64, what: &str| balance::assert_rel(a, b, 1e-6, what);
    let g = (1.0 / Complex64::new(0.8, 9.0)).re;
    balance::solve_clean(&mut dss, "wye reactors with rp");
    let v = dss.bus_voltages("b").expect("bus b").node_v.clone();
    let rows = export_losses(&mut dss);
    let row = |name: &str| rows.iter().find(|(n, _)| n == name).expect(name).1;
    let across = (v[0] - v[1]).norm_sqr();
    let to_ground = v[0].norm_sqr();
    close(across, 3.0 * to_ground, "|V1 − V2|² vs 3 |V1|²");
    for (name, sq) in [("reactor.wf", across), ("reactor.wg", to_ground)] {
        let [total, i2r, no_load] = row(name);
        close(no_load, sq / 1000.0, &format!("{name}: no-load"));
        close(total, sq * (g + 1.0 / 1000.0), &format!("{name}: total"));
        close(i2r, sq * g, &format!("{name}: I²R"));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `Dump`, `?` and the JSON export show the matrices a reactor holds, of their
/// own order: after `phases=4` and the refused solve, the 3 x 3 `rmatrix` and
/// `xmatrix`.
#[test]
fn dump_prints_the_matrices_a_reactor_holds() {
    let mut dss = balance::stiff("rdump");
    dss.command(
        "new reactor.r bus1=b phases=3 rmatrix=[1 | 0.2 1 | 0.2 0.2 1] \
         xmatrix=[10 | 1 10 | 1 1 10]",
    );
    dss.command("edit reactor.r phases=4");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(!dss.circuit().unwrap().is_solved, "solved");
    let dir = scratch_datapath(&mut dss, "rdump");
    let n = dss.error_texts().len();
    dss.command("dump reactor.r debug");
    assert_eq!(dss.error_texts().len(), n, "{:?}", &dss.error_texts()[n..]);
    let dumped = std::fs::read_to_string(dss.last_result_file()).expect("dump file");
    let _ = std::fs::remove_dir_all(&dir);
    for line in [
        "RMatrix= (1 0.2 0.2 |0.2 1 0.2 |0.2 0.2 1 )",
        "XMatrix= (10 1 1 |1 10 1 |1 1 10 )",
    ] {
        assert!(dumped.contains(line), "{line} in {dumped}");
    }
    for (what, held) in [
        ("rmatrix", "(1 |0.2 1 |0.2 0.2 1 )"),
        ("xmatrix", "(10 |1 10 |1 1 10 )"),
    ] {
        dss.command(&format!("? reactor.r.{what}"));
        assert_eq!(dss.result(), held, "? {what}");
    }
    let mut twin = balance::stiff("rtwin");
    twin.command(
        "new reactor.r bus1=b phases=3 rmatrix=[1 | 0.2 1 | 0.2 0.2 1] \
         xmatrix=[10 | 1 10 | 1 1 10]",
    );
    for what in ["rmatrix", "xmatrix"] {
        assert_eq!(
            balance::json_matrix(&dss, "Reactor.r", what),
            balance::json_matrix(&twin, "Reactor.r", what),
            "the JSON export of the {what} the reactor holds"
        );
    }
}

/// SpecType 3 with a single-phase reactor: the matrix branch is skipped, so the
/// edit is empty (just BeginEdit/EndEdit).
#[test]
fn make_pos_sequence_matrix_single_phase_is_empty_edit() {
    let mut r = Reactor::new("rx");
    r.cd.nphases = 1;
    r.spec_type = ReactorSpecType::Matrices;
    let plan = r.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(plan.actions, vec![BeginEdit, EndEdit]);
    assert!(plan.run_base);
}

/// The `SpecType` ordinals are user-invisible (no property, no dump line) but
/// they ARE the Pascal `case` selectors, so they stay pinned to the Pascal
/// literals: `Reactor.pas:132` (the legend), `:382`/`:601` = 1, `:434`/`:474`/
/// `:478` = 2, `:419` = 3, `:451` = 4.
#[test]
fn reactor_spec_type_pins_pascal_ordinals() {
    assert_eq!(ReactorSpecType::Kvar.ordinal(), 1);
    assert_eq!(ReactorSpecType::RplusJx.ordinal(), 2);
    assert_eq!(ReactorSpecType::Matrices.ordinal(), 3);
    assert_eq!(ReactorSpecType::SymComponents.ordinal(), 4);
    for s in [
        ReactorSpecType::Kvar,
        ReactorSpecType::RplusJx,
        ReactorSpecType::Matrices,
        ReactorSpecType::SymComponents,
    ] {
        assert_eq!(ReactorSpecType::from_ordinal(s.ordinal()), Some(s));
    }
    // The set is closed: nothing outside 1..=4 exists (no property writes it).
    for v in [i32::MIN, -1, 0, 5, 6, 100, i32::MAX] {
        assert_eq!(ReactorSpecType::from_ordinal(v), None, "ordinal {v}");
    }
    // `Create` seeds kvar (`Reactor.pas:601`).
    assert_eq!(Reactor::new("r").spec_type, ReactorSpecType::Kvar);
}
