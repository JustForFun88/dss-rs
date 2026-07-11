//! Unit tests for the flickermeter filter cascade, percentile interpolation and
//! Pst windowing. Each filter is checked against a hand-computed low-order
//! response with coefficients chosen to be exact in f32; the whole-chain
//! bit-exactness vs the official r3723 engine is pinned by the `golden_flicker`
//! integration test.

use super::*;

/// `Fhp` with `ts=2, whp=1` → `a=1, a0=2, a1=0`, so `y[j] = 0.5·(x[j]-x[j-1])`
/// and `y[0]=0`. A constant input yields all-zero (DC removed).
#[test]
fn fhp_first_difference() {
    let x = [0.0f32, 1.0, 3.0, 6.0];
    let mut y = [0.0f32; 4];
    fhp(2.0, 1.0, &x, &mut y);
    assert_eq!(y, [0.0, 0.5, 1.0, 1.5]);

    let xc = [5.0f32, 5.0, 5.0, 5.0];
    let mut yc = [0.0f32; 4];
    fhp(2.0, 1.0, &xc, &mut yc);
    assert_eq!(yc, [0.0, 0.0, 0.0, 0.0]);
}

/// `Flp` with `ts=2, tau=1` → `a0=2, a1=0`, so `y[j] = 0.5·(x[j]+x[j-1])`.
#[test]
fn flp_running_pair_mean() {
    let x = [0.0f32, 1.0, 3.0, 5.0];
    let mut y = [0.0f32; 4];
    flp(2.0, 1.0, &x, &mut y);
    assert_eq!(y, [0.0, 0.5, 2.0, 4.0]);
}

/// `Fw1` with `ts=1, w1=1, k=1, lam=1` → `b0=2, b2=-2, a0=9, a1=-6, a2=1`, so
/// `y[j] = (2·x[j] - 2·x[j-2] + 6·y[j-1] - y[j-2]) / 9`, seeds `y[0]=y[1]=0`.
#[test]
fn fw1_second_order_step() {
    let x = [0.0f32, 0.0, 1.0, 1.0];
    let mut y = [0.0f32; 4];
    fw1(1.0, 1.0, 1.0, 1.0, &x, &mut y);
    // y[2] = (2·1 - 2·0 + 6·0 - 0)/9 = 2/9
    // y[3] = (2·1 - 2·0 + 6·(2/9) - 0)/9 = (2 + 12/9)/9 = (30/9)/9 = 30/81
    let e2 = ((2.0f64) / 9.0) as f32;
    let e3 = ((2.0f64 + 6.0 * (2.0 / 9.0)) / 9.0) as f32;
    assert_eq!(y[0], 0.0);
    assert_eq!(y[1], 0.0);
    assert_eq!(y[2], e2);
    assert_eq!(y[3], e3);
    // A constant input stays zero (no DC gain in the band-pass stage).
    let xc = [1.0f32, 1.0, 1.0, 1.0, 1.0];
    let mut yc = [0.0f32; 5];
    fw1(1.0, 1.0, 1.0, 1.0, &xc, &mut yc);
    assert_eq!(yc, [0.0, 0.0, 0.0, 0.0, 0.0]);
}

/// `Fw2` with `ts=1, w2=1, w3=1, w4=1`:
/// `b0=1·(1+2)=3, b1=1·2·1·1·1=2, b2=1·(1-2)=-1,`
/// `a0=1·(1+2·2+4)=9, a1=1·(2-8)=-6, a2=1·(1-2·2+4)=1`.
/// `y[j] = (3·x[j] + 2·x[j-1] - x[j-2] + 6·y[j-1] - y[j-2]) / 9`.
#[test]
fn fw2_second_order_step() {
    let x = [0.0f32, 0.0, 1.0, 0.0];
    let mut y = [0.0f32; 4];
    fw2(1.0, 1.0, 1.0, 1.0, &x, &mut y);
    // y[2] = (3·1 + 2·0 - 1·0 + 6·0 - 0)/9 = 3/9
    // y[3] = (3·0 + 2·1 - 1·0 + 6·(3/9) - 0)/9 = (2 + 2)/9 = 4/9
    let e2 = ((3.0f64) / 9.0) as f32;
    let e3 = ((2.0f64 + 6.0 * (3.0 / 9.0)) / 9.0) as f32;
    assert_eq!(y[2], e2);
    assert_eq!(y[3], e3);
}

/// `Percentile` interpolates between the two straddling order statistics.
#[test]
fn percentile_interpolates() {
    let list = [10.0f32, 20.0, 30.0, 40.0, 50.0];
    // pctExceeded=50 -> pct=50, idx=0.01·50·5=2.5, nlo=2, nhi=3, frac=0.5
    // -> 30 + 0.5·(40-30) = 35.
    assert_eq!(percentile(&list, 0, 4, 50.0), 35.0);
    // pctExceeded=80 -> pct=20, idx=0.01·20·5=1.0, nlo=1, nhi=2, frac=0 -> 20.
    assert_eq!(percentile(&list, 0, 4, 80.0), 20.0);
}

/// The `nhi = nlo+1` overrun (D5): at `pctExceeded=0.1` on a 5-wide window,
/// `idx=4.995 -> nlo=4, nhi=5`. When the histogram has a zeroed tail slot
/// (`list.len()>i_hi+1`) that slot (0.0) is read verbatim (FPC-deterministic);
/// when `nhi==list.len()` it is a past-allocation UB read → clamped to the last
/// valid element (never diverging past the buffer).
#[test]
fn percentile_overrun_within_alloc_reads_zero_tail() {
    // len 6, window i_hi=4, tail slot list[5]=0.0 (the SetLength zero-fill).
    let list = [10.0f32, 20.0, 30.0, 40.0, 50.0, 0.0];
    // idx=0.01·99.9·5=4.995 -> nlo=4, nhi=5, frac≈0.995; xlo=50, xhi=0.
    let got = percentile(&list, 0, 4, 0.1);
    let pct = (100.0 - 0.1f32 as f64) as f32;
    let idx = 0.01_f64 * pct as f64 * 5.0;
    let xfrac = (idx.fract() as f32) as f64;
    let expect = (50.0f64 + xfrac * (0.0 - 50.0)) as f32;
    assert_eq!(got, expect);
    // The zero tail pulls the 0.1-percentile well below the true window max (50).
    assert!(got < 1.0);
}

#[test]
fn percentile_overrun_past_alloc_clamps() {
    // len exactly 5: nhi=5 == len -> clamp to list[4]=50, so xhi==xlo -> 50.
    let list = [10.0f32, 20.0, 30.0, 40.0, 50.0];
    assert_eq!(percentile(&list, 0, 4, 0.1), 50.0);
}

/// A constant per-unit input produces zero instantaneous flicker (DC removed by
/// `Fhp`), hence zero Pst; the Pst array carries exactly one value per completed
/// 600 s window.
#[test]
fn flicker_constant_input_zero_pst_and_window_count() {
    // ts=100 s -> a 600 s window spans 6 samples; 13 samples cover two windows
    // (fires at t=600 and t=1200).
    let n = 13;
    let times: Vec<f32> = (1..=n).map(|i| (i as f32) * 100.0).collect();
    let mut prms = vec![1000.0f32; n];
    // Npst = 1 + trunc(1300/600) = 3 (upper bound on windows).
    let mut ppst = vec![-1.0f32; 3];
    flicker_meter(60.0, 1000.0, &times, &mut prms, &mut ppst);
    // Flicker of a constant signal is ~0.
    for &v in &prms {
        assert!(v.abs() < 1e-3, "flk {v} not ~0");
    }
    // Two windows fire (t=600 at i=6, t=1200 at i=12); the 3rd slot stays unset.
    assert_eq!(ppst[0], 0.0);
    assert_eq!(ppst[1], 0.0);
    assert_eq!(ppst[2], -1.0, "no third window over 1300 s");
}

/// A degenerate <2-sample stream has no defined step (`ts := pT[2]-pT[1]`) — the
/// port bails out rather than reading out of bounds.
#[test]
fn flicker_too_short_is_noop() {
    let mut prms = [1000.0f32];
    let mut ppst = [0.0f32; 1];
    flicker_meter(60.0, 1000.0, &[10.0], &mut prms, &mut ppst);
    assert_eq!(prms, [1000.0]); // untouched
}
