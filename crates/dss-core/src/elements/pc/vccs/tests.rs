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
fn ringbuf_tap_reproduces_pascal_map_idx_order() {
    // Fill a len-5 ring so slot k holds value k (slot 0 dead).
    let len = 5usize;
    let mut rb = RingBuf::alloc(len);
    for k in 1..=len {
        rb[k] = k as f64;
    }
    // The filter taps `iu - k + 1` for k = 1..=len at head iu = 1 must read the
    // slots in the exact Pascal `MapIdx` order [1, 5, 4, 3, 2] — the same order
    // `offset_idx_steps_and_wraps` pins for the raw index helper.
    let iu = 1i64;
    let taps: Vec<f64> = (1..=len).map(|k| rb.tap(iu - k as i64 + 1)).collect();
    assert_eq!(taps, vec![1.0, 5.0, 4.0, 3.0, 2.0]);
    // `tap(idx)` is `self[map_idx(idx, len)]` for every input (incl. negative /
    // past-end), so wrapping the raw indexing in the accessor is bit-neutral.
    for idx in -(len as i64)..=(2 * len as i64) {
        assert_eq!(rb.tap(idx), rb[map_idx(idx, len as i64)]);
    }
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

#[cfg(test)]
mod pos_seq_tests {
    use super::*;
    use crate::elements::pos_seq::{PosSeqAction, PosSeqCtx};
    use crate::elements::traits::CktElement;

    /// VCCS, multi-phase → bare `Phases := 1` edit + run_base
    /// (Pascal `TVCCSObj.MakePosSequence`, vccs.pas:495-500).
    #[test]
    fn makeposseq_vccs_multiphase_sets_phases_1() {
        let mut v = Vccs::new("v1");
        v.cd.nphases = 3;
        v.cd.set_nconds(3);
        let plan = v.make_pos_sequence(&PosSeqCtx::default());
        assert!(plan.run_base);
        assert_eq!(plan.actions, vec![PosSeqAction::SetI32(prop::PHASES, 1)]);
    }

    /// VCCS, already single phase (default) → base-only, still run_base.
    #[test]
    fn makeposseq_vccs_single_phase_is_base_only() {
        let mut v = Vccs::new("v1");
        assert_eq!(v.cd.nphases, 1);
        let plan = v.make_pos_sequence(&PosSeqCtx::default());
        assert!(plan.run_base);
        assert!(plan.actions.is_empty());
    }
}
