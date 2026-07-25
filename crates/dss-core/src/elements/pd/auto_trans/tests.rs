//! Stage A unit tests: construction defaults and `RecalcElementData`-derived
//! values, checked against the pinned oracle (`? AutoTrans.a1.<prop>` probe,
//! 2026-07-08). No solve — the auto YPrim/solve path is WPG.15 Stage B.

use super::*;
use crate::elements::pd::winding::Connection;

#[test]
fn create_defaults_match_oracle() {
    let t = AutoTrans::new("a1");
    // Structural: 3 phases, nconds = 2·nphases (two conductors per winding),
    // 2 windings.
    assert_eq!(t.cd.nphases, 3);
    assert_eq!(t.cd.nconds, 6);
    assert_eq!(t.num_windings, 2);
    assert_eq!(t.cd.nterms, 2);
    // Winding 1 = Series/115 kV, winding 2 = Wye/12.47 kV.
    assert_eq!(t.windings[0].connection, Connection::Series);
    assert_eq!(t.windings[0].kvll, 115.0);
    assert_eq!(t.windings[1].connection, Connection::Wye);
    assert_eq!(t.windings[1].kvll, 12.47);
    // Default reactances: XHX=10 % (puXHX=0.10), XHT=35 %, XXT=30 %.
    assert!((t.puxhx - 0.10).abs() < 1e-15);
    assert!((t.puxht - 0.35).abs() < 1e-15);
    assert!((t.puxxt - 0.30).abs() < 1e-15);
}

#[test]
fn recalc_series_vbase_and_derived_ratings() {
    let t = AutoTrans::new("a1");
    // kVSeries = (115 − 12.47)/√3 (2/3-phase); VBase = kVSeries·1000.
    let kv_series = (115.0 - 12.47) / 3.0_f64.sqrt();
    assert!((t.kv_series - kv_series).abs() < 1e-9);
    assert!((t.windings[0].vbase - kv_series * 1000.0).abs() < 1e-6);
    // RDCOhms (winding 1) = 0.85·Rpu · VBase²/VABase — oracle dumps 5.95702717…
    assert!((t.windings[0].rdcohms - 5.957_027_176_666_67).abs() < 1e-6);
    // NormAmps = NormMaxHkVA/nphases/VFactor, VFactor = VBase·0.001 (series).
    assert!((t.norm_amps - 6.194_141_189_004_08).abs() < 1e-6);
    assert!((t.emerg_amps - 8.446_556_166_823_75).abs() < 1e-6);
}

#[test]
fn wdgcurrents_zero_when_unwired() {
    // Not attached to a solution → all-zero winding currents (matches the
    // oracle default `0, (0), 0, (0), …`).
    let t = AutoTrans::new("a1");
    let s = t.winding_currents_result();
    assert_eq!(s, "0, (0), 0, (0), 0, (0), 0, (0), 0, (0), 0, (0), ");
}

#[test]
fn term_ref_series_maps_h_and_x() {
    // Default 3-phase 2-winding auto: winding 1 (Series) conductor 1 → phase i,
    // conductor 2 → phase i+nphases; winding 2 (Wye) conductor 1 → phase, 2 →
    // phase+nphases within its block. Pins the Series arm of SetTermRef.
    let t = AutoTrans::new("a1");
    // TermRef is 1-based (slot 0 unused), length 2·nw·nphases + 1 = 13.
    // Phase 1: [1] series c1=1, [2] series c2=1+3=4, [3] wye c1=(2-1)*6+1=7,
    // [4] wye c2=7+3=10.
    assert_eq!(t.term_ref[1], 1);
    assert_eq!(t.term_ref[2], 4);
    assert_eq!(t.term_ref[3], 7);
    assert_eq!(t.term_ref[4], 10);
}

// --- WPG.21 — TAutoTransObj.MakePosSequence (AutoTrans.pas:1724-1791) ---------

use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
use crate::elements::traits::CktElement;

fn unwrap_f64s(a: &PosSeqAction) -> Vec<f64> {
    match a {
        PosSeqAction::SetStructF64s(_, v) => v.iter().map(|o| o.expect("Some")).collect(),
        other => panic!("expected SetStructF64s, got {other:?}"),
    }
}

/// A 3-phase 2-winding autotransformer (deck `at`: series/wye 4.16/12.47). All
/// windings Common/Wye (0), kV = kVLL/√3, kVA and NormHkVA/EmergHkVA per phase.
#[test]
fn make_pos_sequence_3ph_two_winding() {
    let mut t = AutoTrans::new("at");
    t.windings[0].connection = Connection::Series;
    t.windings[0].kvll = 4.16;
    t.windings[0].kva = 2000.0;
    t.windings[1].connection = Connection::Wye;
    t.windings[1].kvll = 12.47;
    t.windings[1].kva = 2000.0;
    t.cd.set_bus(1, "b4");
    t.cd.set_bus(2, "b1");
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
        SetStructBuses(vec!["b4".to_string(), "b1".to_string()])
    );
    let kvs = unwrap_f64s(&plan.actions[4]);
    assert!((kvs[0] - 4.16 / sqrt3()).abs() < 1e-9, "kv0 {}", kvs[0]);
    assert!((kvs[1] - 12.47 / sqrt3()).abs() < 1e-9, "kv1 {}", kvs[1]);
    assert!((kvs[1] - 7.199_558).abs() < 1e-5);
    let kvas = unwrap_f64s(&plan.actions[5]);
    assert!((kvas[0] - 2000.0 / 3.0).abs() < 1e-9);
    assert!((kvas[1] - 2000.0 / 3.0).abs() < 1e-9);
    assert_eq!(plan.actions[6], SetF64(prop::NORMHKVA, norm / 3.0));
    assert_eq!(plan.actions[7], SetF64(prop::EMERGHKVA, emerg / 3.0));
    assert_eq!(plan.actions[8], EndEdit);
    assert_eq!(plan.actions.len(), 9);
}

/// A 1-phase auto with a winding NOT on phase 1 is disabled (no `inherited`),
/// mirroring the transformer disable path.
#[test]
fn make_pos_sequence_1ph_off_phase1_disables() {
    let mut t = AutoTrans::new("at");
    t.cd.nphases = 1;
    let ctx = PosSeqCtx {
        terminal_nodes: vec![vec![2], vec![2]],
        ..Default::default()
    };
    let plan = t.make_pos_sequence(&ctx);
    assert_eq!(plan.actions, vec![PosSeqAction::Disable]);
    assert!(!plan.run_base);
}

/// A 1-phase auto with all windings on phase 1 converts (with `inherited`).
#[test]
fn make_pos_sequence_1ph_on_phase1_survives() {
    let mut t = AutoTrans::new("at");
    t.cd.nphases = 1;
    let ctx = PosSeqCtx {
        terminal_nodes: vec![vec![1], vec![1]],
        ..Default::default()
    };
    let plan = t.make_pos_sequence(&ctx);
    assert!(plan.run_base);
    assert_eq!(plan.actions[0], PosSeqAction::BeginEdit);
    assert_eq!(plan.actions[1], PosSeqAction::SetI32(prop::PHASES, 1));
}

/// C6 (r4064, 90962ae8): the AutoTrans GICharm BH-curve `Unused` props. Default
/// is unallocated (NIL → dumps ''); BHpoints reallocates both arrays zeroed.
/// The array PARSE is the shared DoubleArray path (covered by the Transformer
/// sibling test); here we pin the AutoTrans wiring: default, side effect, dump.
#[test]
fn bh_curve_default_and_realloc() {
    use crate::obj::base::DssObject;
    let mut t = AutoTrans::new("a1");
    assert_eq!(t.bh_points, 0);
    assert!(t.bh_current.is_empty() && t.bh_flux.is_empty());
    // NIL pointer renders '' (get_f64_array yields None for an empty array).
    assert!(t.get_f64_array(prop::BHCURRENT).is_none());
    assert!(t.get_f64_array(prop::BHFLUX).is_none());

    t.set_i32(prop::BHPOINTS, 3);
    t.side_effects(prop::BHPOINTS, 0);
    assert_eq!(t.bh_current, vec![0.0; 3]);
    assert_eq!(t.bh_flux, vec![0.0; 3]);
    assert_eq!(t.get_f64_array(prop::BHCURRENT), Some(&[0.0, 0.0, 0.0][..]));
}
