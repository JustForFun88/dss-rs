use super::*;

use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::base::DssObject;
use crate::obj::props::PropEngine;
use crate::solution::SolveMode;
use dss_parser::{Parser, ParserVars};

/// Drive `(prop, value)` edits through the property engine, then `end_edit`
/// (which recomputes). Mirrors the executive's edit loop without foreign
/// class resolution (none of these tests reference an XfmrCode).
fn edited(edits: &[(&str, &str)]) -> Transformer {
    let enums = EnumRegistry::new();
    let cls = class_props(&enums);
    let mut obj = Transformer::new("t");
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    let mut errors = Vec::new();
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

fn test_sys() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: 1,
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
    }
}

#[test]
fn set_term_ref_3ph_wye_wye() {
    // 3-phase 2-winding wye-wye: nconds = 4. Each phase i maps winding j's
    // phase conductor `(j-1)*4 + i` and its neutral `j*4`.
    let t = edited(&[("phases", "3"), ("windings", "2"), ("conns", "wye, wye")]);
    // TermRef is 1-based with slot 0 unused. Layout: per phase i (1..3),
    // per winding j (1..2): [phaseCond, neutCond].
    // phase 1: w1 -> (1, 4), w2 -> (5, 8)
    assert_eq!(&t.term_ref[1..=4], &[1, 4, 5, 8]);
    // phase 2: w1 -> (2, 4), w2 -> (6, 8)
    assert_eq!(&t.term_ref[5..=8], &[2, 4, 6, 8]);
    // phase 3: w1 -> (3, 4), w2 -> (7, 8)
    assert_eq!(&t.term_ref[9..=12], &[3, 4, 7, 8]);
}

#[test]
fn set_term_ref_3ph_wye_delta() {
    // Winding 2 delta: its second conductor connects to the next phase in
    // sequence (DeltaDirection from RotatePhases), not the neutral.
    let t = edited(&[
        ("phases", "3"),
        ("windings", "2"),
        ("kvs", "115, 4.16"),
        ("conns", "wye, delta"),
    ]);
    // HV (winding 1) is wye → DeltaDirection = +1, so phase i delta maps to
    // phase i+1 (wrapping 3→1).
    // phase 1: w1 wye -> (1, 4), w2 delta -> (5, 6)   [(2-1)*4 + rot(1)=2]
    assert_eq!(&t.term_ref[1..=4], &[1, 4, 5, 6]);
    // phase 2: w1 -> (2, 4), w2 delta -> (6, 7)
    assert_eq!(&t.term_ref[5..=8], &[2, 4, 6, 7]);
    // phase 3: w1 -> (3, 4), w2 delta -> (7, 5)  [rot(3)=1 → (2-1)*4+1=5]
    assert_eq!(&t.term_ref[9..=12], &[3, 4, 7, 5]);
}

#[test]
fn yprim_1ph_wye_wye_matches_oracle() {
    // Oracle (dss-python 0.15.7) Yprim of
    //   Transformer.t1 phases=1 windings=2 buses=(a.1.0, b.1.0)
    //     conns=(wye,wye) kvs=(7.2,0.24) kvas=(25,25) xhl=2 %r=0.5
    // captured via ActiveCktElement.Yprim (order 4).
    let mut t = edited(&[
        ("phases", "1"),
        ("windings", "2"),
        ("buses", "a.1.0, b.1.0"),
        ("conns", "wye, wye"),
        ("kvs", "7.2, 0.24"),
        ("kvas", "25, 25"),
        ("xhl", "2"),
        ("%r", "0.5"),
    ]);
    let sys = test_sys();
    t.calc_yprim(&sys);
    let yprim = t.cd.yprim.as_ref().unwrap();
    assert_eq!(yprim.order(), 4);

    let expected = [
        [
            (0.007518, -0.021481),
            (-0.007518, 0.021481),
            (-0.225553, 0.644436),
            (0.225553, -0.644436),
        ],
        [
            (-0.007518, 0.021481),
            (0.007518, -0.021481),
            (0.225553, -0.644436),
            (-0.225553, 0.644436),
        ],
        [
            (-0.225553, 0.644436),
            (0.225553, -0.644436),
            (6.766580, -19.333086),
            (-6.766580, 19.333086),
        ],
        [
            (0.225553, -0.644436),
            (-0.225553, 0.644436),
            (-6.766580, 19.333086),
            (6.766580, -19.333086),
        ],
    ];
    for (i, row) in expected.iter().enumerate() {
        for (j, &(re, im)) in row.iter().enumerate() {
            let v = yprim.get(i, j);
            assert!(
                (v.re - re).abs() < 1e-4 && (v.im - im).abs() < 1e-4,
                "Yprim[{i},{j}] = {v} vs ({re}, {im})"
            );
        }
    }
}

#[test]
fn set_present_tap_clamps_to_winding_limits() {
    let mut t = edited(&[("windings", "2"), ("kvs", "7.2, 0.24")]);
    // MaxTap defaults to 1.10; asking for 1.5 clamps there.
    t.set_present_tap(1, 1.5);
    assert!((t.present_tap(1) - 1.10).abs() < 1e-12);
    // MinTap defaults to 0.90.
    t.set_present_tap(1, 0.5);
    assert!((t.present_tap(1) - 0.90).abs() < 1e-12);
    // In range: applied verbatim.
    t.set_present_tap(2, 1.025);
    assert!((t.present_tap(2) - 1.025).abs() < 1e-12);
    // Out-of-range winding index is a no-op.
    t.set_present_tap(9, 1.0);
}
