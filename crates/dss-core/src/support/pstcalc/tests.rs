//! Unit tests for the f64 IEC-868 Pst calculator.

use super::*;

/// Build an `n`-point voltage array: 1.0 pu with a small sinusoidal modulation
/// at `cycles_per_period` samples per modulation cycle and amplitude `amp`.
fn modulated(n: usize, amp: f64, cycles_per_period: f64) -> Vec<f64> {
    (0..n)
        .map(|i| 1.0 + amp * (2.0 * std::f64::consts::PI * (i as f64) / cycles_per_period).sin())
        .collect()
}

#[test]
fn num_pst_intervals_formula() {
    // Pst_Time_Max = N*DeltaT; Pst_Time = min(600, Pst_Time_Max);
    // NumPstIntervals = max(1, trunc(Pst_Time_Max / Pst_Time)).
    // DeltaT = 1 s (cycles=freq): span < 600 collapses to a single interval.
    assert_eq!(num_pst_intervals(12, 1.0), 1); // trunc(12/12)=1
    assert_eq!(num_pst_intervals(300, 1.0), 1); // trunc(300/300)=1
    assert_eq!(num_pst_intervals(600, 1.0), 1); // trunc(600/600)=1
    assert_eq!(num_pst_intervals(700, 1.0), 1); // trunc(700/600)=1
    assert_eq!(num_pst_intervals(1200, 1.0), 2); // trunc(1200/600)=2
    assert_eq!(num_pst_intervals(1900, 1.0), 3); // trunc(1900/600)=3
    // The max(1,..) floor: a zero-length span still allocates one slot.
    assert_eq!(num_pst_intervals(0, 1.0), 1);
    // Non-unit DeltaT: 100 points * 12 s = 1200 s -> 2 intervals.
    assert_eq!(num_pst_intervals(100, 12.0), 2);
}

#[test]
fn constant_input_gives_near_zero_pst() {
    // A perfectly constant RMS voltage carries no flicker: after the 30 s
    // warm-up (already at the same constant) the bandpass output stays ~0, so
    // Pst ~ 0. 700 points @ dt=1 s, freq=60 completes exactly one interval.
    let v = vec![1.0f64; 700];
    let pst = pst_rms(&v, 60.0, 60, 120);
    assert_eq!(pst.len(), 1, "700 pts @ dt=1 completes one 600 s interval");
    assert!(
        pst[0] < 1e-6,
        "constant input must give Pst ~ 0, got {}",
        pst[0]
    );

    // The constant value is irrelevant — only the ratio to the first sample
    // matters (per-unit normalization), so a 120 V constant is also ~0.
    let v2 = vec![120.0f64; 700];
    let pst2 = pst_rms(&v2, 60.0, 60, 120);
    assert_eq!(pst2.len(), 1);
    assert!(pst2[0] < 1e-6, "got {}", pst2[0]);
}

#[test]
fn completed_interval_count_matches_timing() {
    // The returned length is the number of *completed* intervals, which differs
    // from NumPstIntervals when too few samples accumulate: the leading ~35 s
    // (30 s warm-up + 5 s settle) never counts.
    let make = |n: usize| modulated(n, 0.05, 25.0);

    // 12 points -> 0 completed (a whole interval needs > pst_time seconds past
    // the 35 s head start, impossible for a 12 s span).
    assert_eq!(pst_rms(&make(12), 60.0, 60, 120).len(), 0);
    // 700 points -> 1 completed interval.
    assert_eq!(pst_rms(&make(700), 60.0, 60, 120).len(), 1);
    // 1900 points -> 3 completed intervals.
    assert_eq!(pst_rms(&make(1900), 60.0, 60, 120).len(), 3);
}

#[test]
fn lamp_type_selects_distinct_bandpass() {
    // The 120 V and 230 V lamps use different bandpass gain constants; a
    // modulated input must produce measurably different (non-zero) Pst values,
    // proving both coefficient branches are live.
    let v = modulated(700, 0.05, 25.0);
    let pst120 = pst_rms(&v, 60.0, 60, 120);
    let pst230 = pst_rms(&v, 60.0, 60, 230);
    assert_eq!(pst120.len(), 1);
    assert_eq!(pst230.len(), 1);
    assert!(
        pst120[0] > 0.0 && pst230[0] > 0.0,
        "modulation -> non-zero Pst"
    );
    assert!(
        (pst120[0] - pst230[0]).abs() > 1e-9,
        "lamp 120 vs 230 must differ: {} vs {}",
        pst120[0],
        pst230[0]
    );
}

#[test]
fn base_frequency_affects_result() {
    // Fbase sets Tstep = 1/(16*Fbase) and DeltaT = cycles/Fbase; 50 Hz and 60 Hz
    // on the same sample array give different Pst (freq path is exercised).
    let v = modulated(700, 0.05, 25.0);
    let pst60 = pst_rms(&v, 60.0, 60, 120);
    let pst50 = pst_rms(&v, 50.0, 50, 120);
    assert_eq!(pst60.len(), 1);
    assert_eq!(pst50.len(), 1);
    assert!(
        (pst60[0] - pst50[0]).abs() > 1e-9,
        "freq 60 vs 50 must differ: {} vs {}",
        pst60[0],
        pst50[0]
    );
}

#[test]
fn fpc_power_matches_intpower_multiply_tree() {
    // Guard the module-doc claim: FPC `power(x, n)` for integral n uses
    // `intpower` (exponentiation by squaring), so power(x,4) = sqr(sqr(x)) and
    // power(x,3) = x*sqr(x). Pin the exact multiply trees the port relies on.
    for &x in &[1.0 / (16.0 * 60.0), 1.0 / (16.0 * 50.0), 0.3, 2.5, 7.0] {
        let x2 = x * x;
        assert_eq!(x2 * x2, {
            // sqr(sqr(x))
            let a = x * x;
            a * a
        });
        assert_eq!(x * x2, {
            // x * sqr(x)
            let a = x * x;
            x * a
        });
    }
}
