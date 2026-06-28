//! Unit tests for the error-prone ring-buffer index helpers (`MapIdx`/
//! `OffsetIdx`) and the rated-quantity recalculation. The full dynamics
//! trajectory is gated against the oracle in `exec/tests/vccs.rs`.

use super::*;

#[test]
fn map_idx_matches_pascal_1based_wraparound() {
    // Pascal `MapIdx(idx, len)`: wrap into 1..=len (a 0 result maps to 1).
    let len = 5;
    assert_eq!(map_idx(1, len), 1);
    assert_eq!(map_idx(5, len), 5);
    assert_eq!(map_idx(6, len), 1); // 6 mod 6 = 0 -> 1
    assert_eq!(map_idx(0, len), 5); // 0 -> +5 -> 5 mod 6 = 5
    assert_eq!(map_idx(-1, len), 4); // -1 -> 4 -> 4
    assert_eq!(map_idx(-5, len), 5); // -5 -> 0 -> +5 -> 5
}

#[test]
fn offset_idx_steps_and_wraps() {
    let len = 5;
    assert_eq!(offset_idx(0, 1, len), 1); // first use after init (sIdxU = 0)
    assert_eq!(offset_idx(1, 1, len), 2);
    assert_eq!(offset_idx(5, 1, len), 1); // wrap len -> 1
    // The history lookup `MapIdx(iu - k + 1, len)` for iu = 1, k = 1..len.
    let iu = 1;
    let hist: Vec<usize> = (1..=len).map(|k| map_idx(iu - k + 1, len)).collect();
    assert_eq!(hist, vec![1, 5, 4, 3, 2]);
}

#[test]
fn recalc_sets_rated_quantities() {
    // 1-phase: Irated = Prated/Vrated/1, BaseVolt = Vrated, BaseCurr = Ppct%·Irated.
    let mut v = Vccs::new("v1");
    v.prated = 3000.0;
    v.vrated = 208.0;
    v.ppct = 100.0;
    v.recalc();
    assert!((v.irated - 3000.0 / 208.0).abs() < 1e-9);
    assert!((v.base_volt - 208.0).abs() < 1e-9);
    assert!((v.base_curr - 3000.0 / 208.0).abs() < 1e-9);

    // 3-phase: Irated *= sqrt(3), BaseVolt /= sqrt(3).
    v.cd.nphases = 3;
    v.cd.set_nconds(3);
    v.prated = 3000.0;
    v.vrated = 360.0;
    v.ppct = 50.0;
    v.recalc();
    let sqrt3 = 3.0_f64.sqrt();
    let irated = 3000.0 / 360.0 / 3.0 * sqrt3;
    assert!((v.irated - irated).abs() < 1e-9);
    assert!((v.base_volt - 360.0 / sqrt3).abs() < 1e-9);
    assert!((v.base_curr - 0.5 * irated).abs() < 1e-9);
}
