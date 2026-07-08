//! Stage A unit tests: construction defaults and `RecalcElementData`-derived
//! values, checked against the pinned oracle (`? AutoTrans.a1.<prop>` probe,
//! 2026-07-08). No solve — the auto YPrim/solve path is WPG.15 Stage B.

use super::*;

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
    assert_eq!(t.windings[0].connection, 2);
    assert_eq!(t.windings[0].kvll, 115.0);
    assert_eq!(t.windings[1].connection, 0);
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
