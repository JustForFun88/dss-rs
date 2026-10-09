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
    let c = Capacitor::new("c1");
    assert_eq!(c.cd.nphases, 3);
    assert_eq!(c.cd.nconds, 3);
    assert_eq!(c.cd.nterms, 2);
    assert_eq!(c.cd.yorder, 6);
    assert!(c.is_shunt);
    assert_eq!(c.spec_type, CapacitorSpecType::Kvar);
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
    c.spec_type = CapacitorSpecType::Kvar;
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
    c.spec_type = CapacitorSpecType::Kvar;
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
    c.spec_type = CapacitorSpecType::CMatrix;
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

/// WP-U1.2 B1: a Cmatrix (SpecType=3) capacitor WITH a series filter reactance
/// (`R`/`XL` > 0 => `has_zl`) reaches `MakeYprimWork`'s SpecType-3 invert path.
/// dss_capi 0.15.x multiplies each work-matrix diagonal by `1.000001` before the
/// first `Invert()` ("add a little bit so it will invert"), the same trick the
/// Delta 1|2 branch already used. Revision-sensitive: WITHOUT the perturbation
/// the near-singular C-admittance inverts to ~1e-23 garbage (0.14.5); WITH it the
/// self-admittance is finite. Reference = capi015 (dss_capi 0.15.0b4), probed
/// 2026-07-12 on `Capacitor.f1 conn=wye cmatrix=(1.5|0.2 1.5|0.2 0.2 1.5) R=0.5
/// XL=3` (`? ...Yprim`), ÷nothing (already the element Yprim).
#[test]
fn cmatrix_with_series_reactance_yprim_matches_capi015() {
    let mut c = Capacitor::new("f1");
    c.cd.nphases = 3;
    c.cd.nconds = 3;
    c.cd.set_nterms(2);
    c.cd.yorder = 6;
    c.connection = 0; // wye
    c.is_shunt = true;
    c.spec_type = CapacitorSpecType::CMatrix;
    // µF → F (parse scale 1e-6); diagonal 1.5, off-diagonal +0.2 (as the deck).
    let m = 1.0e-6;
    c.cmatrix = Some(vec![
        1.5 * m,
        0.2 * m,
        0.2 * m,
        0.2 * m,
        1.5 * m,
        0.2 * m,
        0.2 * m,
        0.2 * m,
        1.5 * m,
    ]);
    // Series filter reactance -> has_zl, reaching the SpecType-3 x1.000001 path.
    c.fr = vec![0.5];
    c.fxl = vec![3.0];
    c.fharm = vec![0.0];
    c.fstates = vec![1];
    c.fnumsteps = 1;
    c.recalc();
    c.calc_yprim(&test_sys());
    let yp = c.cd.yprim.as_ref().unwrap();
    // capi015 phase-block (finite; 0.14.5 gives ~1e-23 garbage without B1).
    let diag = num_complex::Complex64::new(1.661774199e-07, 5.664824416e-04);
    let off = num_complex::Complex64::new(4.572988395e-08, 7.567182900e-05);
    for i in 0..3 {
        let g = yp.get(i, i);
        assert!(
            (g.re - diag.re).abs() < 1e-12 && (g.im - diag.im).abs() < 1e-11,
            "diag[{i}] = ({},{}) vs capi015 ({},{})",
            g.re,
            g.im,
            diag.re,
            diag.im
        );
        for j in 0..3 {
            if i != j {
                let o = yp.get(i, j);
                assert!(
                    (o.re - off.re).abs() < 1e-12 && (o.im - off.im).abs() < 1e-11,
                    "off[{i},{j}] = ({},{}) vs capi015 ({},{})",
                    o.re,
                    o.im,
                    off.re,
                    off.im
                );
            }
        }
    }
}

#[test]
fn numsteps_splits_kvar() {
    let mut c = Capacitor::new("c1");
    c.spec_type = CapacitorSpecType::Kvar;
    c.fkvarrating = vec![600.0];
    c.fnumsteps = 1;
    c.set_num_steps(3);
    assert_eq!(c.fnumsteps, 3);
    assert_eq!(c.fkvarrating, vec![200.0, 200.0, 200.0]);
    assert_eq!(c.fstates, vec![1, 1, 1]);
    assert_eq!(c.flast_step_in_service, 3);
}

// --- WPG.21 — TCapacitorObj.MakePosSequence (Capacitor.pas:768-819) -----------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx, balance};

/// SpecType 1 (kvar): kV = kVRating/√3 (multi-phase wye) and per-step kvar/3
/// (deck `cap_kvar`: 600 kvar → [200], 12.47 kV → 7.1996).
#[test]
fn make_pos_sequence_kvar() {
    let mut c = Capacitor::new("cap_kvar");
    c.spec_type = CapacitorSpecType::Kvar;
    c.kvrating = 12.47;
    c.connection = 0;
    c.fnumsteps = 1;
    c.fkvarrating = vec![600.0];

    let plan = c.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    use PosSeqAction::*;
    assert_eq!(plan.actions[0], BeginEdit);
    assert_eq!(plan.actions[1], SetI32(prop::PHASES, 1));
    match plan.actions[2].clone() {
        SetF64(idx, kv) => {
            assert_eq!(idx, prop::KV);
            assert!((kv - 12.47 / sqrt3()).abs() < 1e-9);
            assert!((kv - 7.199_558).abs() < 1e-5);
        }
        a => panic!("expected SetF64(KV), got {a:?}"),
    }
    match plan.actions[3].clone() {
        SetStructF64s(idx, vals) => {
            assert_eq!(idx, prop::KVAR);
            assert_eq!(vals.len(), 1);
            assert!((vals[0].unwrap() - 200.0).abs() < 1e-12);
        }
        a => panic!("expected SetStructF64s(KVAR), got {a:?}"),
    }
    assert_eq!(plan.actions[4], EndEdit);
    assert_eq!(plan.actions.len(), 5);
}

/// A multi-step bank splits each step's kvar by 3.
#[test]
fn make_pos_sequence_kvar_multistep() {
    let mut c = Capacitor::new("cap");
    c.spec_type = CapacitorSpecType::Kvar;
    c.kvrating = 12.47;
    c.connection = 0;
    c.fnumsteps = 2;
    c.fkvarrating = vec![600.0, 300.0];

    let plan = c.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    match plan.actions[3].clone() {
        SetStructF64s(_, vals) => {
            assert_eq!(vals.len(), 2);
            assert!((vals[0].unwrap() - 200.0).abs() < 1e-12);
            assert!((vals[1].unwrap() - 100.0).abs() < 1e-12);
        }
        a => panic!("expected SetStructF64s, got {a:?}"),
    }
}

/// SpecType 2 (Cuf): a *bare* `Phases := 1` — one Set action, no BeginEdit/
/// EndEdit brackets (the applier auto-wraps it).
#[test]
fn make_pos_sequence_cuf_bare_set() {
    let mut c = Capacitor::new("cap");
    c.spec_type = CapacitorSpecType::Cuf;
    let plan = c.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(plan.actions, vec![SetI32(prop::PHASES, 1)]);
    assert!(plan.run_base);
}

/// EXPECTED-VALUE-PIN(MAKEPOSSEQ_CUF_LOST_ON_THE_SCALAR_SETTER): the action half —
/// a `cmatrix` bank reduces to the positive-sequence self term of its nodal
/// matrix and writes it with the **array** setter, in the property's own µF
/// units.
///
/// Deck `cap_cmat` `cmatrix=[10|-2 10|-2 -2 10]` µF: `(30 + 6) / 3` = 12 µF,
/// the mean self term 10 minus the mean mutual term -2. Both oracles compute
/// 4 µF, a third of it, from a mutual sum that skips the first row, and
/// neither applies even that (dss_capi 0.14.5 drops a scalar write onto the
/// array property, r4133 scales the farads by 1e-6 a second time). The port
/// writes the value at full `f64` precision on every step
/// ([`make_pos_sequence_cmatrix_multistep_bank_keeps_every_step`]).
/// [`make_pos_sequence_cmatrix_applies_the_positive_sequence_cuf`] is the
/// end-to-end value pin.
#[test]
fn make_pos_sequence_cmatrix() {
    let mut c = Capacitor::new("cap_cmat");
    c.spec_type = CapacitorSpecType::CMatrix;
    // Stored in farads (the property scale 1e-6 is applied at parse).
    c.cmatrix = Some(vec![
        10e-6, -2e-6, -2e-6, -2e-6, 10e-6, -2e-6, -2e-6, -2e-6, 10e-6,
    ]);

    let plan = c.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(plan.actions[0], BeginEdit);
    assert_eq!(plan.actions[1], SetI32(prop::PHASES, 1));
    match plan.actions[2].clone() {
        SetStructF64s(idx, vals) => {
            assert_eq!(idx, prop::CUF);
            // One step by default. The setter multiplies by `CUF_SCALE`, so the
            // action carries µF.
            assert_eq!(vals.len(), 1);
            assert!((vals[0].unwrap() - 12.0).abs() < 12e-12, "cuf {vals:?}");
        }
        a => panic!("expected SetStructF64s(CUF), got {a:?}"),
    }
    assert_eq!(plan.actions[3], EndEdit);
    assert_eq!(plan.actions.len(), 4);
}

/// The kvar a positive-sequence capacitance `c1` (farads) draws at the solved
/// phase voltage of bus `b`: `-3 |V1|² ω C1`.
fn shunt_kvar(dss: &crate::exec::Dss, c1: f64) -> f64 {
    let v = balance::v1(dss, "b");
    -3.0 * v * v * 2.0 * std::f64::consts::PI * 60.0 * c1 / 1000.0
}

/// The positive-sequence self term of a real capacitance matrix, from its
/// definition ([`balance::seq11`]).
fn seq11_real(m: &[f64], n: usize) -> f64 {
    let c: Vec<num_complex::Complex64> = m
        .iter()
        .map(|&v| num_complex::Complex64::new(v, 0.0))
        .collect();
    balance::seq11(&c, n).re
}

/// EXPECTED-VALUE-PIN(MAKEPOSSEQ_CUF_LOST_ON_THE_SCALAR_SETTER): the value half —
/// after `MakePosSequence` the `cmatrix` bank is the positive-sequence
/// capacitance and draws the bank's own reactive power.
///
/// On a stiff 12.47 kV bus `cmatrix=[10|-2 10|-2 -2 10]` draws
/// `3 |V1|² ω S(C)` with `S(C) = (1/3) v1ᴴ C v1` = 12 µF (about -703.47
/// kvar), before the reduction and after it (measured equal to 1e-16). The
/// anchor is a `phases=3 cuf=12` wye bank, the `Cuf` path that keeps its
/// per-phase value: reduced in the same circuit, the two have one YPrim.
#[test]
fn make_pos_sequence_cmatrix_applies_the_positive_sequence_cuf() {
    let c = [10.0, -2.0, -2.0, -2.0, 10.0, -2.0, -2.0, -2.0, 10.0];
    let c1 = seq11_real(&c, 3);
    balance::assert_rel(c1, 12.0, 1e-12, "S(C) µF");

    let mut dss = balance::stiff("cufpsq");
    dss.command("new capacitor.cmat bus1=b phases=3 cmatrix=[10 | -2 10 | -2 -2 10]");
    dss.command("new capacitor.ref bus1=b phases=3 cuf=12");
    let [(before, after)] = balance::reduce(&mut dss, &["Capacitor.cmat"])[..] else {
        unreachable!()
    };
    balance::assert_kept(before, after, "Capacitor.cmat");
    balance::assert_rel(
        after.im,
        shunt_kvar(&dss, c1 * 1e-6),
        balance::BALANCE_REL,
        "kvar of the reduced bank vs -3 |V1|² ω S(C)",
    );

    let cuf = balance::query_f64(&mut dss, "Capacitor.cmat.cuf");
    balance::assert_rel(cuf, c1, 1e-12, "Capacitor.cmat.cuf after makeposseq");

    let (order_a, ya) = dss.element_yprim("Capacitor.cmat").expect("cmat YPrim");
    let (order_b, yb) = dss.element_yprim("Capacitor.ref").expect("ref YPrim");
    assert_eq!(
        order_a, order_b,
        "both banks are single-phase after reduction"
    );
    for (i, (x, y)) in ya.iter().zip(yb.iter()).enumerate() {
        assert!(
            (x - y).norm() <= 1e-18 + 1e-12 * y.norm(),
            "YPrim[{i}]: reduced {x} vs the `cuf=12` twin {y}"
        );
    }
}

/// An unbalanced `cmatrix` bank keeps its power too: `S(C)` is exact for
/// any symmetric matrix under a balanced voltage. `[12|-3 10|-1 -2 8]` µF
/// reduces to `(30 + 6) / 3` = 12 µF.
#[test]
fn make_pos_sequence_unbalanced_cmatrix_keeps_the_banks_power() {
    let c = [12.0, -3.0, -1.0, -3.0, 10.0, -2.0, -1.0, -2.0, 8.0];
    let c1 = seq11_real(&c, 3);
    balance::assert_rel(c1, 12.0, 1e-12, "S(C) µF");

    let mut dss = balance::stiff("cunb");
    dss.command("new capacitor.cu bus1=b phases=3 cmatrix=[12 | -3 10 | -1 -2 8]");
    let [(before, after)] = balance::reduce(&mut dss, &["Capacitor.cu"])[..] else {
        unreachable!()
    };
    balance::assert_kept(before, after, "Capacitor.cu");
    balance::assert_rel(
        after.im,
        shunt_kvar(&dss, c1 * 1e-6),
        balance::BALANCE_REL,
        "kvar of the reduced bank vs -3 |V1|² ω S(C)",
    );
    let cuf = balance::query_f64(&mut dss, "Capacitor.cu.cuf");
    balance::assert_rel(cuf, c1, 1e-12, "Capacitor.cu.cuf after makeposseq");
}

/// A two-phase `cmatrix` bank on phases a and b reduces to the self term of
/// the matrix embedded in three phases, `(C11 + C22 − C12) / 3` = 22/3 µF for
/// `[10|-2 10]`, and keeps its power (about -429.90 kvar). Both oracles write
/// 0 µF here.
#[test]
fn make_pos_sequence_two_phase_cmatrix_keeps_the_banks_power() {
    let c = [10.0, -2.0, -2.0, 10.0];
    let c1 = seq11_real(&c, 2);
    balance::assert_rel(c1, 22.0 / 3.0, 1e-12, "S(C) µF");

    let mut dss = balance::stiff("c2ph");
    dss.command("new capacitor.k2 bus1=b.1.2 phases=2 cmatrix=[10 | -2 10]");
    let [(before, after)] = balance::reduce(&mut dss, &["Capacitor.k2"])[..] else {
        unreachable!()
    };
    balance::assert_kept(before, after, "Capacitor.k2");
    balance::assert_rel(
        after.im,
        shunt_kvar(&dss, c1 * 1e-6),
        balance::BALANCE_REL,
        "kvar of the reduced bank vs -3 |V1|² ω S(C)",
    );
    let cuf = balance::query_f64(&mut dss, "Capacitor.k2.cuf");
    balance::assert_rel(cuf, c1, 1e-12, "Capacitor.k2.cuf after makeposseq");
}

/// Above three conductors a `cmatrix` reduces to the mean of the diagonal
/// minus the mean over all off-diagonal pairs (user ruling 2026-10-04). Action
/// level only: four conductors have no positive-sequence power to keep. The
/// unbalanced matrix tells the rule from the three-conductor sum, which would
/// give 16.
#[test]
fn make_pos_sequence_four_conductor_cmatrix_takes_mean_self_minus_mean_mutual() {
    let reduce = |m: Vec<f64>| {
        let mut c = Capacitor::new("k4");
        c.cd.nphases = 4;
        c.spec_type = CapacitorSpecType::CMatrix;
        c.cmatrix = Some(m.iter().map(|v| v * 1e-6).collect());
        match c.make_pos_sequence(&PosSeqCtx::default()).actions[2].clone() {
            PosSeqAction::SetStructF64s(idx, vals) if idx == prop::CUF => vals[0].unwrap(),
            a => panic!("expected SetStructF64s(CUF), got {a:?}"),
        }
    };
    let balanced = vec![
        10.0, -2.0, -2.0, -2.0, -2.0, 10.0, -2.0, -2.0, -2.0, -2.0, 10.0, -2.0, -2.0, -2.0, -2.0,
        10.0,
    ];
    balance::assert_rel(reduce(balanced), 12.0, 1e-12, "balanced 4x4");
    let unbalanced = vec![
        12.0, -3.0, -1.0, -1.0, -3.0, 10.0, -2.0, -1.0, -1.0, -2.0, 8.0, -1.0, -1.0, -1.0, -1.0,
        9.0,
    ];
    // Mean diagonal 9.75 minus mean pair -1.5.
    balance::assert_rel(reduce(unbalanced), 11.25, 1e-12, "unbalanced 4x4");
}

/// A closed three-phase delta `cuf` bank reduces to its wye equivalent, three
/// times the leg capacitance: `phases=3 conn=delta cuf=4` reads back 12 µF and
/// keeps `3 |V_LL|² ω C_leg` (about -703.47 kvar). A two-step bank `cuf=[4 3]`
/// draws the sum of its steps, `3 |V_LL|² ω (C1 + C2)` (about -1231.07 kvar,
/// the power of its delta `cuf=7` twin and of the wye `cuf=[12 9]` bank),
/// writes the wye equivalent of every step (it reads back 12 and 9 µF) and
/// keeps its power. A second `makeposseq` changes nothing (the bank is one
/// phase by then), and a one-phase delta bank keeps its leg value.
///
/// The guard half: with the property order `conn=delta phases=3` the bank has
/// four conductors (legs 1-2, 2-3 and 3 to ground), not a closed delta, and is
/// not tripled. Nor is a two-phase delta bank (three conductors, legs 1-2 and 2
/// to ground). How those elements should reduce is a separate open question
/// (the four-conductor order, and question 1 for two phases), so only the guard
/// is pinned.
#[test]
fn make_pos_sequence_delta_cuf_bank_keeps_its_power() {
    let mut dss = balance::stiff("cdel");
    dss.command("new capacitor.d3 bus1=b phases=3 conn=delta cuf=4");
    dss.command("new capacitor.d1 bus1=b.1.2 phases=1 conn=delta cuf=4");
    dss.command("new capacitor.d4 bus1=b conn=delta phases=3 cuf=4");
    dss.command("new capacitor.d2 bus1=b.1.2 phases=2 conn=delta cuf=4");
    dss.command("new capacitor.dm bus1=b phases=3 conn=delta numsteps=2 cuf=[4 3]");
    dss.command("new capacitor.dm7 bus1=b phases=3 conn=delta cuf=7");
    dss.command("new capacitor.wm bus1=b phases=3 conn=wye numsteps=2 cuf=[12 9]");
    let [
        (before, after),
        (before_m, after_m),
        (before_7, _),
        (before_w, _),
    ] = balance::reduce(
        &mut dss,
        &[
            "Capacitor.d3",
            "Capacitor.dm",
            "Capacitor.dm7",
            "Capacitor.wm",
        ],
    )[..]
    else {
        unreachable!()
    };
    balance::assert_kept(before, after, "Capacitor.d3");
    balance::assert_kept(before_m, after_m, "Capacitor.dm");
    let v_ll = 3f64.sqrt() * balance::v1(&dss, "b");
    let q_steps = -3.0 * v_ll * v_ll * 2.0 * std::f64::consts::PI * 60.0 * (4e-6 + 3e-6) / 1000.0;
    balance::assert_rel(before_m.im, q_steps, 1e-9, "dm vs 3 |V_LL|² ω (C1 + C2)");
    balance::assert_rel(before_m.im, before_7.im, 1e-12, "dm vs its cuf=7 twin");
    balance::assert_rel(
        before_m.im,
        before_w.im,
        1e-12,
        "dm vs the wye cuf=[12 9] bank",
    );
    dss.command("? capacitor.dm.cuf");
    let steps: Vec<f64> = dss
        .result()
        .trim_matches(|c: char| c == '[' || c == ']' || c.is_whitespace())
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap_or_else(|e| panic!("dm cuf {s:?}: {e}")))
        .collect();
    assert_eq!(steps.len(), 2, "dm cuf {:?}", dss.result());
    balance::assert_rel(steps[0], 12.0, 1e-12, "dm cuf step 1");
    balance::assert_rel(steps[1], 9.0, 1e-12, "dm cuf step 2");
    let q_leg = -3.0 * v_ll * v_ll * 2.0 * std::f64::consts::PI * 60.0 * 4e-6 / 1000.0;
    balance::assert_rel(after.im, q_leg, 1e-9, "kvar vs 3 |V_LL|² ω C_leg");
    balance::assert_rel(
        balance::query_f64(&mut dss, "Capacitor.d3.cuf"),
        12.0,
        1e-12,
        "d3 cuf",
    );
    for name in ["d1", "d4", "d2"] {
        balance::assert_rel(
            balance::query_f64(&mut dss, &format!("Capacitor.{name}.cuf")),
            4.0,
            1e-12,
            name,
        );
    }

    let (order, y) = dss.element_yprim("Capacitor.d3").expect("d3 YPrim");
    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let (order2, y2) = dss.element_yprim("Capacitor.d3").expect("d3 YPrim");
    assert_eq!(order2, order, "YPrim order after a second makeposseq");
    for (a, b) in y2.iter().zip(&y) {
        assert!(
            (a - b).norm() <= 1e-12 * b.norm(),
            "a second makeposseq moved the one-phase bank: {a} vs {b}"
        );
    }
    balance::assert_rel(
        balance::query_f64(&mut dss, "Capacitor.d3.cuf"),
        12.0,
        1e-12,
        "d3 cuf after a second makeposseq",
    );
}

/// The energized steps of a bank are capacitors in parallel and each holds
/// the capacitance of its own rating, so a closed delta bank draws the sum of
/// its energized steps, as a wye bank does: each leg holds `C = Σ c_i` and the
/// bank draws `3 ω C |V_LL|²`. `kvar=[300 300]` draws its rated 600 kvar at the
/// rated voltage, the power of its one-step `kvar=600` twin, and `cuf=[1 2 4]`
/// the power of its `cuf=7` twin. `cuf=[1 2 4] states=[1 0 1]` draws the 5 µF
/// of its first and last step. `kvar=[200 400] states=[0 1]` draws the 400
/// kvar of its second step. A filtered bank, `kvar=[300 300] xl=[5 5]`,
/// draws twice its one-step `kvar=300 xl=5` twin, and that twin draws its
/// wye equivalent: `zl = 0.005 + j5` in series with `C_y = 3 C_leg` on each
/// phase, `3 |V_ph|² conj(1 / (zl + 1 / (j ω C_y)))`.
#[test]
fn a_multistep_delta_bank_draws_the_sum_of_its_steps() {
    let mut dss = balance::stiff("cdsteps");
    for (name, def) in [
        ("dk", "numsteps=2 kvar=[300 300] kv=12.47"),
        ("d1", "kvar=600 kv=12.47"),
        ("dc", "numsteps=3 cuf=[1 2 4]"),
        ("dc7", "cuf=7"),
        ("s101", "numsteps=3 cuf=[1 2 4] states=[1 0 1]"),
        ("s01", "numsteps=2 kvar=[200 400] kv=12.47 states=[0 1]"),
        ("fx", "numsteps=2 kvar=[300 300] kv=12.47 xl=[5 5]"),
        ("fx1", "kvar=300 kv=12.47 xl=5"),
    ] {
        dss.command(&format!(
            "new capacitor.{name} bus1=b phases=3 conn=delta {def}"
        ));
    }
    balance::solve_clean(&mut dss, "multistep delta banks");
    let v_ll = 3f64.sqrt() * balance::v1(&dss, "b");
    let w = 2.0 * std::f64::consts::PI * 60.0;
    // kvar of a bank of `kvar_rated` at 12.47 kV, and of `c` farads per leg.
    let rated = |kvar_rated: f64| -kvar_rated * (v_ll / 12470.0).powi(2);
    let legs = |c: f64| -3.0 * w * c * v_ll * v_ll / 1000.0;
    let q = |dss: &mut crate::exec::Dss, name: &str| {
        balance::power(dss, &format!("Capacitor.{name}")).im
    };
    let dk = q(&mut dss, "dk");
    balance::assert_rel(dk, rated(600.0), 1e-9, "dk vs its rated 600 kvar");
    balance::assert_rel(dk, q(&mut dss, "d1"), 1e-12, "dk vs its one-step twin");
    let dc = q(&mut dss, "dc");
    balance::assert_rel(dc, legs(7e-6), 1e-9, "dc vs 3 ω C |V_LL|², C = 7 µF");
    balance::assert_rel(dc, q(&mut dss, "dc7"), 1e-12, "dc vs its cuf=7 twin");
    let s101 = q(&mut dss, "s101");
    balance::assert_rel(s101, legs(5e-6), 1e-9, "s101 vs 3 ω C |V_LL|², C = 5 µF");
    let s01 = q(&mut dss, "s01");
    balance::assert_rel(s01, rated(400.0), 1e-9, "s01 vs its second step's 400 kvar");
    let fx = balance::power(&mut dss, "Capacitor.fx");
    let fx1 = balance::power(&mut dss, "Capacitor.fx1");
    assert!(
        (fx - 2.0 * fx1).norm() <= 1e-12 * fx.norm(),
        "fx vs twice its one-step twin: {fx} vs {fx1}"
    );
    // `xl=5` gives the default `r = xl / 1000`. Each leg holds 100 kvar at
    // 12.47 kV, so `C_y = 3 C_leg = 300 kvar / (ω 12470²)`.
    let c_y = 300e3 / (w * 12470.0 * 12470.0);
    let z = num_complex::Complex64::new(0.005, 5.0 - 1.0 / (w * c_y));
    let v_ph = v_ll / 3f64.sqrt();
    let wye = 3.0 * v_ph * v_ph * z.inv().conj() / 1000.0;
    // The stamp inverts the singular leg matrix with every diagonal entry
    // scaled by 1.000001, a shunt of 2e-6 of a leg on each node that raises
    // `C_y` by 6.7e-7 and the drawn kvar with it, inside the 1e-6 bound.
    assert!(
        (fx1 - wye).norm() <= 1e-6 * wye.norm(),
        "fx1 vs its wye equivalent: {fx1} vs {wye} (rel {:.3e})",
        (fx1 - wye).norm() / wye.norm()
    );
}

/// A `kvar` bank of unequal steps draws its rated total: each step holds the
/// capacitance of its own rating, so `kvar=[200 400]` draws 600 kvar at its
/// rated 12.47 kV in wye and in delta, as its `kvar=600` twin does, and
/// `states=[0 1]` the 400 kvar of its second step. A scalar `kvar=300` after
/// `numsteps=2` leaves the steps `[300 0]`, and the bank draws its 300 kvar.
/// With `harm=[5 5]` each step is tuned to its own capacitance, `xl_i =
/// X_C,i / 25` and `r_i = xl_i / 1000` in series with `X_C,i = 12470² / Q_i`,
/// so the bank draws `3 |V_ph|² conj(Σ 1 / z_i)`, about (25/24) × 600 kvar.
/// `makeposseq` writes each step's third, `[66.67 133.33]`, and the one-phase
/// bank keeps the power.
#[test]
fn a_kvar_bank_of_unequal_steps_draws_its_rating() {
    let mut dss = balance::stiff("cunequal");
    for (name, def) in [
        ("wk", "conn=wye numsteps=2 kvar=[200 400]"),
        ("dk", "conn=delta numsteps=2 kvar=[200 400]"),
        ("w01", "conn=wye numsteps=2 kvar=[200 400] states=[0 1]"),
        ("d01", "conn=delta numsteps=2 kvar=[200 400] states=[0 1]"),
        ("w600", "conn=wye kvar=600"),
        ("wsh", "conn=wye numsteps=2 kvar=300"),
        ("dsh", "conn=delta numsteps=2 kvar=300"),
        ("wh", "conn=wye numsteps=2 kvar=[200 400] harm=[5 5]"),
    ] {
        dss.command(&format!(
            "new capacitor.{name} bus1=b phases=3 kv=12.47 {def}"
        ));
    }
    let steps = |dss: &mut crate::exec::Dss, name: &str| -> Vec<f64> {
        dss.command(&format!("? capacitor.{name}.kvar"));
        dss.result()
            .trim_matches(|c: char| c == '[' || c == ']' || c.is_whitespace())
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse()
                    .unwrap_or_else(|e| panic!("{name} kvar {s:?}: {e}"))
            })
            .collect()
    };
    assert_eq!(steps(&mut dss, "wsh"), vec![300.0, 0.0], "wsh kvar");
    let names = [
        "Capacitor.wk",
        "Capacitor.dk",
        "Capacitor.w01",
        "Capacitor.d01",
        "Capacitor.w600",
        "Capacitor.wsh",
        "Capacitor.dsh",
        "Capacitor.wh",
    ];
    balance::solve_clean(&mut dss, "unequal steps");
    let v = balance::v1(&dss, "b");
    let pairs = balance::reduce(&mut dss, &names);
    let rated = |kvar_rated: f64| -kvar_rated * (3f64.sqrt() * v / 12470.0).powi(2);
    for ((name, (before, after)), kvar) in names
        .iter()
        .zip(&pairs)
        .zip([600.0, 600.0, 400.0, 400.0, 600.0, 300.0, 300.0])
    {
        balance::assert_rel(
            before.im,
            rated(kvar),
            1e-9,
            &format!("{name} vs {kvar} kvar"),
        );
        balance::assert_kept(*before, *after, name);
    }
    let y: num_complex::Complex64 = [200e3, 400e3]
        .iter()
        .map(|q| {
            let xc = 12470.0 * 12470.0 / q;
            let xl = xc / 25.0;
            num_complex::Complex64::new(xl / 1000.0, xl - xc).inv()
        })
        .sum();
    let tuned = 3.0 * v * v * y.conj() / 1000.0;
    let (wh, wh_after) = pairs[7];
    assert!(
        (wh - tuned).norm() <= 1e-9 * tuned.norm(),
        "wh vs its steps tuned to the 5th harmonic: {wh} vs {tuned}"
    );
    balance::assert_kept(wh, wh_after, "Capacitor.wh");
    let wk = steps(&mut dss, "wk");
    assert_eq!(wk.len(), 2, "wk kvar {wk:?}");
    balance::assert_rel(wk[0], 200.0 / 3.0, 1e-12, "wk kvar step 1");
    balance::assert_rel(wk[1], 400.0 / 3.0, 1e-12, "wk kvar step 2");
}

/// A delta `cmatrix` bank of two or more phases has no defined stamp: the
/// solve refuses it with a message naming the element, its phase count and the
/// remedy (`conn=wye`) instead of stamping out of range, and the reports after
/// the refusal read the circuit as not solved. `makeposseq` leaves it as it
/// is, so the solve after it names it again. A one-phase delta `cmatrix` is a
/// single leg and stamps exactly like its `cuf` twin.
#[test]
fn a_delta_cmatrix_bank_refuses_the_solve() {
    for (name, def) in [
        ("cd2", "bus1=b.1.2 phases=2 conn=delta cmatrix=[10 | -2 10]"),
        (
            "cd3",
            "bus1=b phases=3 conn=delta cmatrix=[10 | -2 10 | -2 -2 10]",
        ),
    ] {
        let phases = name[2..].to_string();
        let message = format!(
            "Capacitor.{name}: a cmatrix bank of {phases} phases cannot be connected in delta. \
             Specify it with conn=wye. Aborting solution."
        );
        let mut dss = balance::stiff_metered("cdm");
        dss.command(&format!("new capacitor.{name} {def}"));
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve");
        assert!(
            dss.error_texts().contains(&message),
            "{name}: {:?}",
            dss.error_texts()
        );
        assert!(!dss.circuit().unwrap().is_solved, "{name}: solved");
        balance::reports_after_refusal(&mut dss, &message, name);

        dss.command("makeposseq");
        dss.command(&format!("? capacitor.{name}.phases"));
        assert_eq!(dss.result(), phases, "{name} keeps its phases");
        dss.command(&format!("? capacitor.{name}.cmatrix"));
        assert!(dss.result().contains("10"), "{name} keeps its cmatrix");
        let n_before = dss.error_texts().len();
        dss.command("solve");
        assert!(
            dss.error_texts()[n_before..].contains(&message),
            "{name}: no refusal after makeposseq: {:?}",
            &dss.error_texts()[n_before..]
        );
        assert!(
            !dss.circuit().unwrap().is_solved,
            "{name}: solved after makeposseq"
        );
    }

    let mut dss = balance::stiff("cd1");
    dss.command("new capacitor.cd1 bus1=b.1.2 phases=1 conn=delta cmatrix=[4]");
    dss.command("new capacitor.cref bus1=b.1.2 phases=1 conn=delta cuf=4");
    balance::solve_clean(&mut dss, "one-phase delta cmatrix");
    let (a, b) = (
        balance::power(&mut dss, "Capacitor.cd1"),
        balance::power(&mut dss, "Capacitor.cref"),
    );
    balance::assert_kept(b, a, "one-phase delta cmatrix vs its cuf twin");
}

/// A disabled element is out of the model, so a delta `cmatrix` bank created
/// with `enabled=no` refuses nothing: the rest of the circuit solves as it does
/// without the bank. Enabling the bank brings the refusal back, and disabling it
/// again lets the circuit solve.
#[test]
fn a_disabled_delta_cmatrix_bank_leaves_the_solve_to_the_rest() {
    let mut twin = balance::stiff_metered("cdoff");
    balance::solve_clean(&mut twin, "without the bank");
    let load = balance::power(&mut twin, "Load.ld");

    let mut dss = balance::stiff_metered("cdoff");
    dss.command(
        "new capacitor.cdx bus1=c phases=3 conn=delta cmatrix=[10 | -2 10 | -2 -2 10] enabled=no",
    );
    balance::solve_clean(&mut dss, "with the disabled bank");
    balance::assert_kept(
        load,
        balance::power(&mut dss, "Load.ld"),
        "beside the disabled bank",
    );

    let refused = |dss: &crate::exec::Dss| {
        dss.error_texts()
            .iter()
            .any(|t| t.contains("Capacitor.cdx") && t.contains("conn=wye"))
    };
    dss.command("enable capacitor.cdx");
    dss.command("solve");
    assert!(refused(&dss), "{:?}", dss.error_texts());
    assert!(
        !dss.circuit().unwrap().is_solved,
        "solved with the bank enabled"
    );

    dss.command("disable capacitor.cdx");
    let n = dss.error_texts().len();
    dss.command("solve");
    assert_eq!(dss.error_texts().len(), n, "{:?}", &dss.error_texts()[n..]);
    assert!(
        dss.circuit().unwrap().is_solved,
        "not solved with the bank disabled again"
    );
    balance::assert_kept(
        load,
        balance::power(&mut dss, "Load.ld"),
        "the bank disabled again",
    );
}

/// A `cmatrix` keeps the order it was given in, so after a `phases=` edit to
/// another count it no longer describes the bank. The solve refuses the bank
/// with a message naming it, its matrix and the remedy, the reports after the
/// refusal read the circuit as not solved, `?`, `Dump` and the JSON export
/// show the 3 x 3 matrix the bank holds, and `makeposseq` leaves the bank as
/// it is. Given again for the new phase count, the matrix makes the bank draw
/// the power of a bank created with it.
#[test]
fn a_cmatrix_of_another_phase_count_refuses_the_solve() {
    for phases in [2, 4] {
        let name = format!("cp{phases}");
        let mut dss = balance::stiff_metered("cphase");
        dss.command(&format!(
            "new capacitor.{name} bus1=c phases=3 cmatrix=[10 | -2 10 | -2 -2 10]"
        ));
        dss.command(&format!("edit capacitor.{name} phases={phases}"));
        dss.command("set voltagebases=[12.47]");
        dss.command("calcvoltagebases");
        dss.command("solve");
        let message = format!(
            "Capacitor.{name} has {phases} phases but its cmatrix has 3 x 3 entries. \
             Specify cmatrix for {phases} phases. Aborting solution."
        );
        let refused =
            |dss: &crate::exec::Dss| dss.error_texts().iter().any(|t| t.contains(&message));
        assert!(refused(&dss), "{name}: {:?}", dss.error_texts());
        assert!(!dss.circuit().unwrap().is_solved, "{name}: solved");
        balance::reports_after_refusal(&mut dss, &message, &name);

        let held = "(10 |-2 10 |-2 -2 10 )";
        dss.command(&format!("? capacitor.{name}.cmatrix"));
        assert_eq!(dss.result(), held, "{name}: ? cmatrix");
        let n = dss.error_texts().len();
        dss.command(&format!("dump capacitor.{name} debug"));
        assert_eq!(dss.error_texts().len(), n, "{:?}", &dss.error_texts()[n..]);
        let dumped = std::fs::read_to_string(dss.last_result_file()).unwrap();
        assert!(dumped.contains(&format!("~ CMatrix={held}")), "{dumped}");
        let mut twin = balance::stiff("ctwin");
        twin.command(&format!(
            "new capacitor.{name} bus1=b phases=3 cmatrix=[10 | -2 10 | -2 -2 10]"
        ));
        let full = format!("Capacitor.{name}");
        assert_eq!(
            balance::json_matrix(&dss, &full, "cmatrix"),
            balance::json_matrix(&twin, &full, "cmatrix"),
            "{name}: the JSON export of the matrix the bank holds"
        );

        dss.command("makeposseq");
        dss.command(&format!("? capacitor.{name}.phases"));
        assert_eq!(dss.result(), phases.to_string(), "{name} is not reduced");
        let n = dss.error_texts().len();
        dss.command("solve");
        assert!(
            dss.error_texts()[n..].iter().any(|t| t.contains(&message)),
            "{name}: {:?}",
            &dss.error_texts()[n..]
        );
    }

    let mut twin = balance::stiff("cphase2");
    twin.command("new capacitor.cp2 bus1=b phases=2 cmatrix=[10 | -2 10]");
    balance::solve_clean(&mut twin, "a two-phase bank");
    let want = balance::power(&mut twin, "Capacitor.cp2");

    let mut dss = balance::stiff("cphase2");
    dss.command("new capacitor.cp2 bus1=b phases=3 cmatrix=[10 | -2 10 | -2 -2 10]");
    dss.command("edit capacitor.cp2 phases=2 cmatrix=[10 | -2 10]");
    balance::solve_clean(&mut dss, "the matrix given again");
    balance::assert_kept(
        want,
        balance::power(&mut dss, "Capacitor.cp2"),
        "the matrix given again",
    );
}

/// EXPECTED-VALUE-PIN(MAKEPOSSEQ_CUF_LOST_ON_THE_SCALAR_SETTER): the multi-step
/// half. Every energized step of a `cmatrix` bank stamps the whole matrix, so a
/// `numsteps=3` bank is three times a one-step bank of the same matrix, and the
/// reduction keeps that ratio: each of the three steps carries the one value the
/// one-step bank reduces to.
///
/// The three-step bank holds `cuf=[5 6 7]` before the reduction, so a write
/// that left steps 2..3 alone reads back `[v, 6, 7]`. The one-value command
/// string of r4133, zero-filled by `InterpretDblArray`, reads `[v, 0, 0]` and
/// keeps a third of the bank. Measured 2026-10-04 on both oracles (the pinned
/// dss-python and `epri-worker` on the r4133 DLL), with the reduced `Cuf`
/// written by hand after `makeposseq` on a stiff 12.47 kV bus: `[4 4 4]`
/// carries three times the kvar of `[4 0 0]`, and `[12 12 12]` three times
/// that of `[12 0 0]`, restoring the three-step bank's 2110.4 kvar. The
/// one-step value itself is pinned by [`make_pos_sequence_cmatrix`].
#[test]
fn make_pos_sequence_cmatrix_multistep_bank_keeps_every_step() {
    use num_complex::Complex64;

    fn read_cuf(dss: &mut crate::exec::Dss, name: &str) -> Vec<f64> {
        dss.command(&format!("? Capacitor.{name}.cuf"));
        dss.result()
            .split(|c: char| c == '[' || c == ']' || c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse()
                    .unwrap_or_else(|e| panic!("{name} cuf {s:?}: {e}"))
            })
            .collect()
    }
    fn assert_three_times_one_step(dss: &crate::exec::Dss, when: &str) {
        let (order3, y3) = dss.element_yprim("Capacitor.cm3").expect("cm3 YPrim");
        let (order1, y1) = dss.element_yprim("Capacitor.cm1").expect("cm1 YPrim");
        assert_eq!(order3, order1, "{when}: the two banks have one order");
        assert!(
            y1.iter().any(|y| y.norm() > 1e-6),
            "{when}: the one-step bank has no admittance: {y1:?}"
        );
        for (i, (a, b)) in y3.iter().zip(y1.iter()).enumerate() {
            let want: Complex64 = *b * 3.0;
            assert!(
                (*a - want).norm() <= 1e-18 + 1e-12 * want.norm(),
                "{when}: YPrim[{i}] of the three-step bank {a} vs three times the one-step \
                 bank's {want}"
            );
        }
    }

    // The action: one value per step, each the value a one-step bank of the
    // same matrix reduces to.
    let reduce = |numsteps| {
        let mut c = Capacitor::new("cm");
        c.spec_type = CapacitorSpecType::CMatrix;
        c.fnumsteps = numsteps;
        c.cmatrix = Some(vec![
            10e-6, -2e-6, -2e-6, -2e-6, 10e-6, -2e-6, -2e-6, -2e-6, 10e-6,
        ]);
        match c.make_pos_sequence(&PosSeqCtx::default()).actions[2].clone() {
            PosSeqAction::SetStructF64s(idx, vals) if idx == prop::CUF => vals,
            a => panic!("expected SetStructF64s(CUF), got {a:?}"),
        }
    };
    let one = reduce(1);
    assert_eq!(one.len(), 1, "one step: {one:?}");
    let v = one[0].expect("the one-step value");
    assert!(v > 0.0, "the reduced capacitance is positive: {v}");
    assert_eq!(reduce(3), vec![Some(v); 3], "one value per step");

    // The banks: the ratio holds before and after the reduction.
    let mut dss = crate::exec::Dss::new();
    dss.command("clear");
    dss.command("new circuit.cufpsq3 basekv=12.47 phases=3 bus1=src");
    dss.command("new line.l1 bus1=src bus2=b1 phases=3 r1=0.2 x1=0.5 c1=3 length=1 units=km");
    dss.command("new capacitor.cm1 bus1=b1 phases=3 cmatrix=[10 | -2 10 | -2 -2 10]");
    dss.command(
        "new capacitor.cm3 bus1=b1 phases=3 numsteps=3 cuf=[5 6 7] \
         cmatrix=[10 | -2 10 | -2 -2 10]",
    );
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(
        read_cuf(&mut dss, "cm3"),
        [5.0, 6.0, 7.0],
        "cm3 steps before makeposseq"
    );
    assert_three_times_one_step(&dss, "before makeposseq");

    dss.command("makeposseq");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    let cm1 = read_cuf(&mut dss, "cm1");
    assert_eq!(cm1.len(), 1, "cm1 steps after makeposseq: {cm1:?}");
    assert_eq!(
        read_cuf(&mut dss, "cm3"),
        [cm1[0]; 3],
        "cm3 steps after makeposseq: the cm1 value on every step"
    );
    assert_three_times_one_step(&dss, "after makeposseq");
}

/// SpecType 3 single-phase: the CMatrix branch is skipped → no actions at all
/// (only `inherited`).
#[test]
fn make_pos_sequence_cmatrix_single_phase_is_base() {
    let mut c = Capacitor::new("cap");
    c.cd.nphases = 1;
    c.spec_type = CapacitorSpecType::CMatrix;
    let plan = c.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.actions.is_empty());
    assert!(plan.run_base);
}

// --- `SQR`-binds-first pins (og(json-a) settle) ------------------------------
//
// Pascal writes the derived capacitance and the derived kvar total with `SQR`
// as an ATOM: `w * SQR(PhasekV) * 1000.0` is `w * (kv*kv) * 1000.0`, NOT
// `((w*kv) * kv) * 1000.0`. The port used to spell the second form, which lands
// one ULP away from the oracle for many realistic (kV, kvar, f) combinations
// (over 12 probed triples the oracle matched SQR-first 12/12 and left-to-right
// only 9/12).
//
// These values cannot be pinned through the JSON byte goldens: the Capacitor's
// derived properties render only under Full, and every Full capture also emits
// `CMatrix`, whose oracle getter reads uninitialized memory even when `cmatrix=`
// is set (five fresh oracle processes → five different renders; the same
// dss_capi UB that `tools/golden/gen_props.py` canonicalizes with
// `zero_garbage`). The props round-trip channel is no help either — it compares
// numbers at 1e-9 relative tolerance. So the oracle values below are recorded
// from direct `IActiveClass.ToJSON(Full)` probes on the pinned dss-python
// (0.15.7 / backend 0.14.5) and asserted BIT-EXACTLY here.

/// `Capacitor.pas:642` through the `Cuf` render (`PropertyScale` 1e-6).
/// Deck: `new Capacitor.c bus1=b phases=3 kv=12.47 kvar=600 conn=wye` at 60 Hz.
/// Oracle `Cuf` = `1.0234985333968829E+001`; the left-to-right association
/// gives `…828E+001`.
#[test]
fn derived_cuf_kvar_wye_matches_oracle_bit_exactly() {
    let mut c = Capacitor::new("ckvar");
    c.spec_type = CapacitorSpecType::Kvar;
    c.connection = 0; // wye
    c.kvrating = 12.47;
    c.fkvarrating = vec![600.0];
    c.recalc();
    assert_eq!(c.fc[0] / 1.0e-6, 10.23498533396883);
}

/// Same site, delta connection (`PhasekV` = the can rating, no `/SQRT3`).
/// Deck: `kv=7.2 kvar=450 conn=delta`; oracle `Cuf` = `7.6752962525026680E+000`.
#[test]
fn derived_cuf_kvar_delta_matches_oracle_bit_exactly() {
    let mut c = Capacitor::new("cdelta");
    c.spec_type = CapacitorSpecType::Kvar;
    c.connection = 1; // delta
    c.kvrating = 7.2;
    c.fkvarrating = vec![450.0];
    c.recalc();
    assert_eq!(c.fc[0] / 1.0e-6, 7.675296252502668);
}

/// Each step of a `kvar` bank takes the capacitance of its own rating:
/// `C_i = (Q_i / 3) / (ω V_ph²)` with `V_ph = 24.9 kV / √3`, so
/// `kvar=[900 450 225]` gives 3.85, 1.93 and 0.96 µF, halving step by step
/// (exact: the ratings halve by a power of two). `Ftotalkvar` sums the steps
/// and the default `NormAmps` is that total per phase over `V_ph` times 1.35.
#[test]
fn derived_cuf_multistep_takes_each_steps_own_rating() {
    let mut c = Capacitor::new("csteps");
    c.spec_type = CapacitorSpecType::Kvar;
    c.connection = 0;
    c.kvrating = 24.9;
    c.set_num_steps(3);
    c.fkvarrating = vec![900.0, 450.0, 225.0];
    c.recalc();
    let w = 2.0 * std::f64::consts::PI * 60.0;
    let v_ph = 24.9e3 / 3f64.sqrt();
    let c0 = 300e3 / (w * v_ph * v_ph);
    let close = |a: f64, b: f64, what: &str| {
        assert!((a - b).abs() <= 1e-14 * b, "{what}: {a} vs {b}");
    };
    close(c.fc[0], c0, "step 1 vs (300 kvar) / (ω V_ph²)");
    assert_eq!(c.fc, vec![c.fc[0], c.fc[0] / 2.0, c.fc[0] / 4.0]);
    assert_eq!(c.ftotalkvar, 1575.0);
    close(c.norm_amps, 525e3 / v_ph * 1.35, "NormAmps");
    // The order `w (kv kv)` and `kvar / kv × 1.35` give these bits, which
    // `Cuf` and `NormAmps` render: `(w kv) kv` gives `…362e-6` and
    // `kvar × 1.35 / kv` gives `…296`.
    assert_eq!(c.fc[0], 3.850460712534363e-6, "step 1 to the bit");
    assert_eq!(c.norm_amps, 49.3008437696563, "NormAmps to the bit");
}

/// `Capacitor.pas:664` (`Ftotalkvar + w * FC[i] * SQR(PhasekV) / 1000.0`),
/// observable through the derived Norm/Emerg amps. Deck: `kv=24.9 conn=wye
/// cuf=7.2`; oracle `NormAmps` = `1.7559609301075169E-005`, `EmergAmps` =
/// `2.3412812401433556E-005` (the left-to-right association gives `…559E-005`).
#[test]
fn derived_amps_from_cuf_spec_match_oracle_bit_exactly() {
    let mut c = Capacitor::new("ccuf");
    c.spec_type = CapacitorSpecType::Cuf; // Cuf
    c.connection = 0;
    c.kvrating = 24.9;
    c.fc = vec![7.2 * 1.0e-6];
    c.recalc();
    assert_eq!(c.norm_amps, 1.755960930107517e-05);
    assert_eq!(c.emerg_amps, 2.3412812401433556e-05);
}

/// The `SpecType` ordinals are pinned twice: to the Pascal literals
/// (`Capacitor.pas:377` = 1, `:385` = 2, `:381` = 3, `Create` `:584` = 1) and to
/// the `SpecType=<int>` dump line the `dump_capacitor` golden captures.
#[test]
fn capacitor_spec_type_pins_pascal_ordinals_and_the_dump_line() {
    assert_eq!(CapacitorSpecType::Kvar.ordinal(), 1);
    assert_eq!(CapacitorSpecType::Cuf.ordinal(), 2);
    assert_eq!(CapacitorSpecType::CMatrix.ordinal(), 3);
    for s in [
        CapacitorSpecType::Kvar,
        CapacitorSpecType::Cuf,
        CapacitorSpecType::CMatrix,
    ] {
        assert_eq!(CapacitorSpecType::from_ordinal(s.ordinal()), Some(s));
    }
    // Closed set: no property writes it, so nothing outside 1..=3 exists.
    for v in [i32::MIN, -1, 0, 4, 100, i32::MAX] {
        assert_eq!(CapacitorSpecType::from_ordinal(v), None, "ordinal {v}");
    }
    // `Create` seeds kvar, and the dump renders the raw ordinal.
    let c = Capacitor::new("c");
    assert_eq!(c.spec_type, CapacitorSpecType::Kvar);
    assert_eq!(format!("SpecType={}", c.spec_type.ordinal()), "SpecType=1");
}
