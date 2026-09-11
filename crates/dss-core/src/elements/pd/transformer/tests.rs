use super::*;

use crate::elements::traits::{CktElement, SysCtx};
use crate::obj::base::DssObject;
use crate::obj::props::PropEngine;
use crate::solution::{LoadSolutionModel, SolveMode};
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
    let mut errors = crate::diag::ErrorLog::new();
    for (name, value) in edits {
        let idx = cls.property_index(name).expect("known property");
        let mut eng = PropEngine {
            parser: &mut parser,
            vars: &vars,
            enums: &enums,
            errors: &mut errors,
            foreign: None,

            was_quoted: false,
        };
        cls.edit_property(&mut obj, idx, value, &mut eng).unwrap();
    }
    obj.end_edit(&crate::elements::traits::SysCtx::parse_default());
    assert!(errors.is_empty(), "{errors:?}");
    obj
}

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
fn set_term_ref_3ph_wye_wye() {
    // 3-phase 2-winding wye-wye: nconds = 4. Each phase i maps winding j's
    // phase conductor `(j-1)*4 + i` and its neutral `j*4`.
    let t = edited(&[("phases", "3"), ("windings", "2"), ("conns", "wye, wye")]);
    // TermRef holds one 0-based [plus, minus] conductor pair per (phase, winding),
    // phase-major. Same mapping as the 1-based Pascal `[1,4,5,8 | 2,4,6,8 |
    // 3,4,7,8]`, minus one.
    // phase 0: w0 -> (0, 3), w1 -> (4, 7)
    assert_eq!(t.term_ref.pair(0, 0, 2), [0, 3]);
    assert_eq!(t.term_ref.pair(0, 1, 2), [4, 7]);
    // phase 1: w0 -> (1, 3), w1 -> (5, 7)
    assert_eq!(t.term_ref.pair(1, 0, 2), [1, 3]);
    assert_eq!(t.term_ref.pair(1, 1, 2), [5, 7]);
    // phase 2: w0 -> (2, 3), w1 -> (6, 7)
    assert_eq!(t.term_ref.pair(2, 0, 2), [2, 3]);
    assert_eq!(t.term_ref.pair(2, 1, 2), [6, 7]);
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
    // phase i+1 (wrapping 3→1). 0-based pairs (Pascal values minus one).
    // phase 0: w0 wye -> (0, 3), w1 delta -> (4, 5)
    assert_eq!(t.term_ref.pair(0, 0, 2), [0, 3]);
    assert_eq!(t.term_ref.pair(0, 1, 2), [4, 5]);
    // phase 1: w0 -> (1, 3), w1 delta -> (5, 6)
    assert_eq!(t.term_ref.pair(1, 0, 2), [1, 3]);
    assert_eq!(t.term_ref.pair(1, 1, 2), [5, 6]);
    // phase 2: w0 -> (2, 3), w1 delta -> (6, 4)  [rot(3)=1]
    assert_eq!(t.term_ref.pair(2, 0, 2), [2, 3]);
    assert_eq!(t.term_ref.pair(2, 1, 2), [6, 4]);
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

/// WP-U1.5 E2 (dss_capi 0.15.x `55400a29`): the seasonal `GetRatings` override
/// applies to a Transformer too (0.14.5's `DI_Overloads` path restricted it to
/// lines). With `Seasons=2 Ratings=[1000 1200]`, `get_ratings(1)` returns the
/// derived `amp_ratings[1]` for both norm and emerg; an out-of-range/`-1` index
/// falls back to the base `NormAmps`/`EmergAmps` (feature-sensitive).
#[test]
fn get_ratings_applies_seasonal_index_on_transformer() {
    let t = edited(&[
        ("phases", "3"),
        ("windings", "2"),
        ("kvs", "115, 4.16"),
        ("conns", "wye, wye"),
        ("kvas", "1000, 1000"),
        ("Seasons", "2"),
        ("Ratings", "[1000 1200]"),
    ]);
    assert_eq!(t.num_amp_ratings(), 2);
    let (n0, e0) = t.get_ratings(0);
    assert_eq!((n0, e0), (t.amp_ratings[0], t.amp_ratings[0]));
    let (n1, e1) = t.get_ratings(1);
    assert_eq!((n1, e1), (t.amp_ratings[1], t.amp_ratings[1]));
    // Inactive / out of range → base ratings.
    assert_eq!(t.get_ratings(-1), (t.norm_amps, t.emerg_amps));
    assert_eq!(t.get_ratings(2), (t.norm_amps, t.emerg_amps));
}

/// WP-U1.2 D8: dss_capi 0.15.x added the `TrapZero` FLAG to the 3-winding
/// reactances `X12`/`X13`/`X23` (`Transformer.pas` DefineProperties, commit
/// 69fca934), so a parsed `0` is replaced by the property default (7/35/30 %).
/// The Rust port ALREADY traps these (the `PropertyTrapZero` values 7/35/30 were
/// ported onto `XHL`/`XHT`/`XLT` and `setters.rs` applies `trap_zero`
/// unconditionally), so it matches the 0.15.x side. Settle 2026-07-12: probing
/// `X13=0` on a 3-winding transformer gives the **bit-identical** solve on
/// 0.14.5 AND capi015 (`? XHT == "3500"`; `AllBusVmagPu` Vmin=0.124819 both) —
/// i.e. **D8 is not an observable delta for our port** in the reachable scalar
/// path; both engines reach the same trapped default. (The `XSCArray` `NonZero`
/// STRICT-error flag added alongside is the C2/L2 strict surface we deliberately
/// do NOT adopt.) This test pins that the trap fires (feature-sensitive: without
/// it `xht` would be 0, a degenerate transformer).
#[test]
fn three_winding_x13_x23_trap_zero_to_default() {
    let t = edited(&[
        ("phases", "3"),
        ("windings", "3"),
        ("buses", "src, b1, b2"),
        ("conns", "delta, wye, wye"),
        ("kvs", "115, 12.47, 4.16"),
        ("kvas", "1000, 1000, 1000"),
        ("XHL", "7"),
        ("XHT", "0"), // -> trapped to the 35% default (stored 35.0)
        ("XLT", "0"), // -> trapped to the 30% default (stored 30.0)
    ]);
    assert!(
        (t.xht - 35.0).abs() < 1e-9,
        "XHT=0 should trap to the 35 default, got {}",
        t.xht
    );
    assert!(
        (t.xlt - 30.0).abs() < 1e-9,
        "XLT=0 should trap to the 30 default, got {}",
        t.xlt
    );
}

/// C6 (r4064, 90962ae8): the GICharm BH-curve `Unused` data props parse and
/// store. `BHpoints` (re)allocates both arrays zeroed; `BHcurrent`/`BHflux`
/// overwrite them. (Values are post-tokenization, as the `edited` helper
/// receives them — the top-level parser strips the array brackets.) Default is
/// empty, matching capi015 (BHpoints=0, arrays '').
#[test]
fn bh_curve_props_parse_and_store() {
    let d = Transformer::new("t");
    assert_eq!(d.bh_points, 0);
    assert!(d.bh_current.is_empty() && d.bh_flux.is_empty());

    let t = edited(&[
        ("BHpoints", "3"),
        ("BHcurrent", "1 2 3"),
        ("BHflux", "4 5 6"),
    ]);
    assert_eq!(t.bh_points, 3);
    assert_eq!(t.bh_current, vec![1.0, 2.0, 3.0]);
    assert_eq!(t.bh_flux, vec![4.0, 5.0, 6.0]);
}

/// `BHpoints` alone reallocates both arrays to that length, zeroed.
#[test]
fn bh_points_realloc_zeroes_arrays() {
    let t = edited(&[("BHpoints", "4")]);
    assert_eq!(t.bh_current, vec![0.0; 4]);
    assert_eq!(t.bh_flux, vec![0.0; 4]);
}

#[test]
fn gic_build_y_terminal_rdc_only_below_051hz() {
    // Pascal `TTransfObj.GICBuildYTerminal` (Transformer.pas:1823): below 0.51 Hz
    // `Y_Term` is a resistance-only `2·NumWindings` matrix — `1/RdcOhms` on both
    // diagonal entries of each winding, `-1/RdcOhms` across its two conductors,
    // NO inter-winding coupling, plus the anti-float adder (`-Y_PPM`, a real
    // conductance) on both diagonal entries; `Y_Term_NL` is empty.
    let mut t = edited(&[
        ("phases", "1"),
        ("windings", "2"),
        ("buses", "a.1.0, b.1.0"),
        ("conns", "wye, wye"),
        ("kvs", "7.2, 0.24"),
        ("kvas", "25, 25"),
        ("xhl", "2"),
        ("%r", "0.5"),
        ("wdg", "1"),
        ("rdcohms", "2.5"),
        ("wdg", "2"),
        ("rdcohms", "0.9"),
    ]);
    // GIC/dc: Solution.Frequency < 0.51 Hz.
    let mut sys = test_sys();
    sys.frequency = 0.1;
    t.calc_yprim(&sys);

    let yt = &t.y_term;
    assert_eq!(yt.order(), 4); // 2 * NumWindings

    let rdc0 = t.windings[0].rdcohms;
    let rdc1 = t.windings[1].rdcohms;
    assert_eq!(rdc0, 2.5);
    assert_eq!(rdc1, 0.9);
    let yppm0 = t.windings[0].y_ppm;
    let yppm1 = t.windings[1].y_ppm;
    // Y_PPM = -ppm_factor / (vbase²/vabase_1ph) / 2 (negative); the adder is
    // `-Y_PPM` so it raises the diagonal by |Y_PPM|.
    assert!(
        yppm0 < 0.0 && yppm1 < 0.0,
        "anti-float Y_PPM must be active (ppm_float_factor != 0)"
    );
    let tol = 1e-12;

    // Winding 0 (conductors 0,1): diag = 1/Rdc - Y_PPM (real), off = -1/Rdc.
    assert!((yt.get(0, 0).re - (1.0 / rdc0 - yppm0)).abs() < tol);
    assert!(yt.get(0, 0).im.abs() < tol);
    assert!((yt.get(1, 1).re - (1.0 / rdc0 - yppm0)).abs() < tol);
    assert!((yt.get(0, 1).re + 1.0 / rdc0).abs() < tol);
    assert!((yt.get(1, 0).re + 1.0 / rdc0).abs() < tol);
    // Winding 1 (conductors 2,3).
    assert!((yt.get(2, 2).re - (1.0 / rdc1 - yppm1)).abs() < tol);
    assert!((yt.get(3, 3).re - (1.0 / rdc1 - yppm1)).abs() < tol);
    assert!((yt.get(2, 3).re + 1.0 / rdc1).abs() < tol);
    assert!((yt.get(3, 2).re + 1.0 / rdc1).abs() < tol);
    // NO inter-winding coupling.
    for (i, j) in [(0, 2), (0, 3), (1, 2), (1, 3), (2, 0), (3, 1)] {
        assert!(
            yt.get(i, j).norm() < tol,
            "unexpected inter-winding coupling at ({i},{j})"
        );
    }
    // Y_Term_NL is empty (no magnetizing branch at dc).
    for i in 0..4 {
        for j in 0..4 {
            assert!(t.y_term_nl.get(i, j).norm() < tol);
        }
    }
}

#[test]
fn core_type_pins_noncontiguous_enum_ordinals() {
    use super::CoreType;
    assert_eq!(CoreType::Shell.ordinal(), 0);
    assert_eq!(CoreType::OnePhase.ordinal(), 1);
    assert_eq!(CoreType::ThreeLeg.ordinal(), 3);
    assert_eq!(CoreType::FourLeg.ordinal(), 4);
    assert_eq!(CoreType::FiveLeg.ordinal(), 5);
    assert_eq!(CoreType::CoreOnePhase.ordinal(), 9);
    // Non-contiguous: the gap ordinals are not members.
    assert_eq!(CoreType::from_ordinal(2), None);
    assert_eq!(CoreType::from_ordinal(6), None);
    assert_eq!(CoreType::from_ordinal(9), Some(CoreType::CoreOnePhase));
}
