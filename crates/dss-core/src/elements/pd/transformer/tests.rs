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

// --- WPG.21 — TTransfObj.MakePosSequence (Transformer.pas:1685-1752) ----------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
use crate::util::sqrt3;

fn unwrap_f64s(a: &PosSeqAction) -> (usize, Vec<f64>) {
    match a {
        PosSeqAction::SetStructF64s(idx, v) => (*idx, v.iter().map(|o| o.expect("Some")).collect()),
        other => panic!("expected SetStructF64s, got {other:?}"),
    }
}

/// A 3-phase 2-winding substation transformer (deck `sub`: delta/wye 115/12.47).
/// All windings wye, kV = kVLL/√3 (deck-probed kvs = [66.395, 7.1996]), kVA and
/// NormHkVA/EmergHkVA per phase.
#[test]
fn make_pos_sequence_3ph_two_winding() {
    let mut t = edited(&[
        ("phases", "3"),
        ("windings", "2"),
        ("xhl", "8"),
        ("buses", "src, b1"),
        ("conns", "delta, wye"),
        ("kvs", "115, 12.47"),
        ("kvas", "20000, 20000"),
    ]);
    let norm = t.norm_max_hkva;
    let emerg = t.emerg_max_hkva;

    let plan = t.make_pos_sequence(&PosSeqCtx::default());
    assert!(plan.run_base);
    use PosSeqAction::*;
    assert_eq!(plan.actions[0], BeginEdit);
    assert_eq!(plan.actions[1], SetI32(prop::PHASES, 1));
    assert_eq!(plan.actions[2], SetStructI32s(prop::CONNS, vec![0, 0]));
    assert_eq!(
        plan.actions[3],
        SetStructBuses(vec!["src".to_string(), "b1".to_string()])
    );
    let (kvi, kvs) = unwrap_f64s(&plan.actions[4]);
    assert_eq!(kvi, prop::KVS);
    assert!((kvs[0] - 115.0 / sqrt3()).abs() < 1e-9);
    assert!((kvs[0] - 66.395_28).abs() < 1e-4, "kv0 {}", kvs[0]);
    assert!((kvs[1] - 12.47 / sqrt3()).abs() < 1e-9);
    assert!((kvs[1] - 7.199_558).abs() < 1e-5, "kv1 {}", kvs[1]);
    let (kvai, kvas) = unwrap_f64s(&plan.actions[5]);
    assert_eq!(kvai, prop::KVAS);
    assert!((kvas[0] - 20000.0 / 3.0).abs() < 1e-9);
    assert!((kvas[1] - 20000.0 / 3.0).abs() < 1e-9);
    assert_eq!(plan.actions[6], SetF64(prop::NORMHKVA, norm / 3.0));
    assert_eq!(plan.actions[7], SetF64(prop::EMERGHKVA, emerg / 3.0));
    assert_eq!(plan.actions[8], EndEdit);
    assert_eq!(plan.actions.len(), 9);
}

/// A 1-phase transformer with every winding on phase 1 (`bus.1`) survives the
/// conversion — single-phase wye keeps the line-line kV (no /√3).
#[test]
fn make_pos_sequence_1ph_on_phase1_survives() {
    let mut t = edited(&[
        ("phases", "1"),
        ("windings", "2"),
        ("xhl", "2"),
        ("buses", "b1.1, b5.1"),
        ("conns", "wye, wye"),
        ("kvs", "7.2, 0.24"),
        ("kvas", "100, 100"),
    ]);
    // The applier fills terminal_nodes from each GetBus's parsed node list.
    let ctx = PosSeqCtx {
        terminal_nodes: vec![vec![1], vec![1]],
        ..Default::default()
    };
    let plan = t.make_pos_sequence(&ctx);
    assert!(plan.run_base, "on-phase-1 → converts + inherited");
    use PosSeqAction::*;
    assert_eq!(plan.actions[1], SetI32(prop::PHASES, 1));
    // single-phase wye → kV kept line-line (no /√3).
    let (_, kvs) = unwrap_f64s(&plan.actions[4]);
    assert!((kvs[0] - 7.2).abs() < 1e-12, "kv0 {}", kvs[0]);
    assert!((kvs[1] - 0.24).abs() < 1e-12, "kv1 {}", kvs[1]);
}

/// A 1-phase transformer with a winding NOT on phase 1 (`bus.2`) is disabled and
/// left untouched — the disable path returns without `inherited` (buses kept).
#[test]
fn make_pos_sequence_1ph_off_phase1_disables() {
    let mut t = edited(&[
        ("phases", "1"),
        ("windings", "2"),
        ("xhl", "2"),
        ("buses", "b1.2, b6.2"),
        ("conns", "wye, wye"),
        ("kvs", "7.2, 0.24"),
        ("kvas", "100, 100"),
    ]);
    let ctx = PosSeqCtx {
        terminal_nodes: vec![vec![2], vec![2]],
        ..Default::default()
    };
    let plan = t.make_pos_sequence(&ctx);
    assert_eq!(plan.actions, vec![PosSeqAction::Disable]);
    assert!(!plan.run_base, "disable path skips `inherited`");
}

/// WP-U1.2 D6: dss_capi 0.15.x (`Transformer.pas:1058`, SVN r4033) dropped the
/// spurious `1.1 *` factor from the seasonal AmpRatings —
/// `AmpRatings[i] = kVARatings[i] / Fnphases / Vfactor`. The separate
/// `NormMaxHkVA = 1.1 * Winding[1].kVA` (the 110% default norm rating) is
/// UNCHANGED upstream. This is not yet reachable via the overload report (the
/// seasonal-rating override is NOT_PORTED — WP-U1.5 E2), so it is pinned on the
/// computed `amp_ratings` field. Feature-sensitive: with `kVARatings[0]` equal
/// to winding-1 kVA, `norm_amps == norm_max_hkva/np/vfactor == 1.1 * (kva/np/
/// vfactor)`, so post-D6 `amp_ratings[0] == norm_amps / 1.1` — pre-D6 it was
/// `== norm_amps` (both carried the 1.1), so the `/1.1` assertion flips.
#[test]
fn seasonal_amp_ratings_drop_the_1_1_factor() {
    let t = edited(&[
        ("phases", "3"),
        ("windings", "2"),
        ("kvs", "115, 4.16"),
        ("conns", "wye, wye"),
        ("kvas", "1000, 1000"),
        ("Seasons", "2"),
        ("Ratings", "[1000 1200]"),
    ]);
    assert_eq!(t.amp_ratings.len(), 2, "two seasonal ratings");
    // Reconstruct `np/vfactor` from the (unchanged) NormAmps relation
    // `norm_amps = norm_max_hkva / np / vfactor`, so
    // `amp_ratings[i] == kVARatings[i] * norm_amps / norm_max_hkva` post-D6.
    // Pre-D6 (with the `1.1 *`) each amp_ratings[i] was 1.1× this — so the
    // assertion is feature-sensitive to the dropped factor.
    for (i, &kva) in t.kva_ratings.iter().enumerate() {
        let want = kva * t.norm_amps / t.norm_max_hkva;
        assert!(
            (t.amp_ratings[i] - want).abs() < 1e-9,
            "amp_ratings[{i}] = {} (no-1.1 want {want}); 1.1× would be {}",
            t.amp_ratings[i],
            want * 1.1
        );
    }
}
