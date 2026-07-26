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
        loads_need_updating: false,
        neglect_load_y: false,
        long_line_correction: false,
        positive_sequence: false,
        time_of_day: 0.0,
        dyna_h: 0.0,
        dyna_t: 0.0,
        iteration_flag: crate::support::dynamics::IterationFlag::NewTimeStep,
        last_solution_was_direct: false,
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
    c.spec_type = 3;
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
    c.spec_type = 1;
    c.fkvarrating = vec![600.0];
    c.fnumsteps = 1;
    c.set_num_steps(3);
    assert_eq!(c.fnumsteps, 3);
    assert_eq!(c.fkvarrating, vec![200.0, 200.0, 200.0]);
    assert_eq!(c.fstates, vec![1, 1, 1]);
    assert_eq!(c.flast_step_in_service, 3);
}

// --- WPG.21 — TCapacitorObj.MakePosSequence (Capacitor.pas:768-819) -----------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};

/// SpecType 1 (kvar): kV = kVRating/√3 (multi-phase wye) and per-step kvar/3
/// (deck `cap_kvar`: 600 kvar → [200], 12.47 kV → 7.1996).
#[test]
fn make_pos_sequence_kvar() {
    let mut c = Capacitor::new("cap_kvar");
    c.spec_type = 1;
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
    c.spec_type = 1;
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
    c.spec_type = 2;
    let plan = c.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(plan.actions, vec![SetI32(prop::PHASES, 1)]);
    assert!(plan.run_base);
}

/// SpecType 3 (CMatrix): average self/mutual → Cuf. Deck `cap_cmat`
/// cmatrix=[10|-2 10|-2 -2 10] µF → Cs=10, Cm=6 → Cuf=4 µF (stored farads).
#[test]
fn make_pos_sequence_cmatrix() {
    let mut c = Capacitor::new("cap_cmat");
    c.spec_type = 3;
    // Stored in farads (the property scale 1e-6 is applied at parse).
    c.cmatrix = Some(vec![
        10e-6, -2e-6, -2e-6, -2e-6, 10e-6, -2e-6, -2e-6, -2e-6, 10e-6,
    ]);

    let plan = c.make_pos_sequence(&PosSeqCtx::default());
    use PosSeqAction::*;
    assert_eq!(plan.actions[0], BeginEdit);
    assert_eq!(plan.actions[1], SetI32(prop::PHASES, 1));
    match plan.actions[2].clone() {
        SetF64(idx, cuf) => {
            assert_eq!(idx, prop::CUF);
            assert!((cuf - 4e-6).abs() < 1e-18, "cuf {cuf}");
        }
        a => panic!("expected SetF64(CUF), got {a:?}"),
    }
    assert_eq!(plan.actions[3], EndEdit);
    assert_eq!(plan.actions.len(), 4);
}

/// SpecType 3 single-phase: the CMatrix branch is skipped → no actions at all
/// (only `inherited`).
#[test]
fn make_pos_sequence_cmatrix_single_phase_is_base() {
    let mut c = Capacitor::new("cap");
    c.cd.nphases = 1;
    c.spec_type = 3;
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
    c.spec_type = 1;
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
    c.spec_type = 1;
    c.connection = 1; // delta
    c.kvrating = 7.2;
    c.fkvarrating = vec![450.0];
    c.recalc();
    assert_eq!(c.fc[0] / 1.0e-6, 7.675296252502668);
}

/// Multi-step: every step takes `FkvarRating[1]` (not its own step rating) in
/// the `FC` derivation, while `Ftotalkvar` sums all steps. Deck: `kv=24.9
/// conn=wye numsteps=3 kvar=[900,450,225]`; oracle `Cuf` =
/// `[3.8504607125343631E+000 ×3]`, `NormAmps` = `4.9300843769656304E+001`.
#[test]
fn derived_cuf_multistep_matches_oracle_bit_exactly() {
    let mut c = Capacitor::new("csteps");
    c.spec_type = 1;
    c.connection = 0;
    c.kvrating = 24.9;
    c.set_num_steps(3);
    c.fkvarrating = vec![900.0, 450.0, 225.0];
    c.recalc();
    assert_eq!(c.fc, vec![3.850460712534363e-6; 3]);
    assert_eq!(c.ftotalkvar, 1575.0);
    assert_eq!(c.norm_amps, 49.3008437696563);
}

/// `Capacitor.pas:664` (`Ftotalkvar + w * FC[i] * SQR(PhasekV) / 1000.0`),
/// observable through the derived Norm/Emerg amps. Deck: `kv=24.9 conn=wye
/// cuf=7.2`; oracle `NormAmps` = `1.7559609301075169E-005`, `EmergAmps` =
/// `2.3412812401433556E-005` (the left-to-right association gives `…559E-005`).
#[test]
fn derived_amps_from_cuf_spec_match_oracle_bit_exactly() {
    let mut c = Capacitor::new("ccuf");
    c.spec_type = 2; // Cuf
    c.connection = 0;
    c.kvrating = 24.9;
    c.fc = vec![7.2 * 1.0e-6];
    c.recalc();
    assert_eq!(c.norm_amps, 1.755960930107517e-05);
    assert_eq!(c.emerg_amps, 2.3412812401433556e-05);
}
