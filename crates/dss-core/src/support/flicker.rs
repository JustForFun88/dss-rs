//! Port of the **f32-state RMS flickermeter** in `Shared/Pstcalc.pas` lines
//! 476–687 (`Fhp`/`Flp`/`Fw1`/`Fw2`/`QuickSort`/`Percentile`/`FlickerMeter`) —
//! the IEC-868 (IEC 61000-4-15) instantaneous-flicker + short-term severity (Pst)
//! chain that `Meters/Monitor.pas` mode 4 post-processes
//! (`DoFlickerCalculations`).
//!
//! ## Numeric model — `Single` storage, `Double` arithmetic
//!
//! Pascal declares the filter states, coefficients, percentile histogram and the
//! stream values `Single`, so every **stored** value here is `f32`. But the
//! reference engine (Delphi Win64, where `Extended` ≡ `Double`) evaluates each
//! floating **expression** in `Double` and narrows to `Single` only on the store
//! to a `Single` variable. So the port computes every recurrence / coefficient /
//! percentile expression in **f64** over f32 operands promoted with `as f64`, and
//! narrows with `as f32` exactly at each Pascal assignment. This is not cosmetic:
//! the pure-f32 recurrence diverges from the reference by ~5e-5 over 8640 IIR
//! samples, whereas the f64-arithmetic/f32-storage model is **bit-exact** vs the
//! r3723 reference (probe-verified 2026-07-11 on the `pst.dss` demo, all 3 phases,
//! 8640 samples, 0/25920 f32 bits differ). `v_base` is likewise pre-narrowed to
//! f32 by the caller (Pascal `Vbase: Single := 1000·kVBase`) before promotion.
//!
//! ## Not a pinned-oracle gate — the dss_capi oracle crashes here
//!
//! `Monitor.pas:1655` (`DoFlickerCalculations`) reads
//! `MeteredElement.Terminals[MeteredTerminal].BusRef` with the **1-based**
//! terminal number, but the dss_capi rewrite made `Terminals` a **0-based**
//! `Array of TPowerTerminal` (every other site indexes `Terminals[i-1]`). For a
//! 1-terminal metered element the only valid index is 0, so `Terminals[1]` reads
//! one `TPowerTerminal` past the array → an unconditional access violation
//! (probe-proven 2026-07-11: `Monitors.Process()` hard-segfaults the pinned
//! oracle; `export monitor` on **any** mode-4 monitor raises error 671 "Cannot
//! close Monitor stream / Access violation", for every N incl. N=2 with zero
//! percentile windows). The **official** Delphi engine keeps `Terminals` 1-based
//! (`pTerminalList`, `Terminals^[1]`), so it does NOT crash — the vendored r3723
//! EPRI binary produces a correct flicker CSV. Per CLAUDE.md's
//! known-upstream-bugs rule this OOB is **not reproduced**: the port uses the
//! metered terminal's bus (the official/correct behavior), and the flicker
//! goldens are captured from the official r3723 binary (the D9 reference channel)
//! because the pinned oracle cannot produce them.

/// Pascal `Fhp` (`Pstcalc.pas:480`): first-order high-pass, `y[1]:=0` seed.
/// `a := 0.5*Ts*whp`; recurrence `y[j] := (1/a0)·(x[j]-x[j-1]-a1·y[j-1])`.
fn fhp(ts: f32, whp: f32, x: &[f32], y: &mut [f32]) {
    let n = x.len();
    y[0] = 0.0;
    let a = (0.5 * ts as f64 * whp as f64) as f32;
    let a0 = (a as f64 + 1.0) as f32;
    let a1 = (a as f64 - 1.0) as f32;
    let (a0, a1) = (a0 as f64, a1 as f64);
    for j in 1..n {
        y[j] = ((1.0 / a0) * (x[j] as f64 - x[j - 1] as f64 - a1 * y[j - 1] as f64)) as f32;
    }
}

/// Pascal `Flp` (`Pstcalc.pas:495`): first-order low-pass, `y[1]:=0` seed.
fn flp(ts: f32, tau: f32, x: &[f32], y: &mut [f32]) {
    let n = x.len();
    y[0] = 0.0;
    let a0 = (1.0 + 2.0 * tau as f64 / ts as f64) as f32;
    let a1 = (1.0 - 2.0 * tau as f64 / ts as f64) as f32;
    let (a0, a1) = (a0 as f64, a1 as f64);
    for j in 1..n {
        y[j] = ((1.0 / a0) * (x[j] as f64 + x[j - 1] as f64 - a1 * y[j - 1] as f64)) as f32;
    }
}

/// Pascal `Fw1` (`Pstcalc.pas:508`): second-order weighting stage 1,
/// `y[1]:=y[2]:=0` seeds, recurrence from `j:=3`.
fn fw1(ts: f32, w1: f32, k: f32, lam: f32, x: &[f32], y: &mut [f32]) {
    let n = x.len();
    y[0] = 0.0;
    y[1] = 0.0;
    let (ts, w1, k, lam) = (ts as f64, w1 as f64, k as f64, lam as f64);
    let b0 = (2.0 * k * w1 * ts) as f32;
    let b2 = (-2.0 * k * w1 * ts) as f32;
    let a0 = (w1 * w1 * ts * ts + 4.0 * lam * ts + 4.0) as f32;
    let a1 = (2.0 * w1 * w1 * ts * ts - 8.0) as f32;
    let a2 = (w1 * w1 * ts * ts - 4.0 * lam * ts + 4.0) as f32;
    let (b0, b2, a0, a1, a2) = (b0 as f64, b2 as f64, a0 as f64, a1 as f64, a2 as f64);
    for j in 2..n {
        let (xj, xj2, yj1, yj2) = (
            x[j] as f64,
            x[j - 2] as f64,
            y[j - 1] as f64,
            y[j - 2] as f64,
        );
        y[j] = ((1.0 / a0) * (b0 * xj + b2 * xj2 - a1 * yj1 - a2 * yj2)) as f32;
    }
}

/// Pascal `Fw2` (`Pstcalc.pas:526`): second-order weighting stage 2,
/// `y[1]:=y[2]:=0` seeds, recurrence from `j:=3`.
fn fw2(ts: f32, w2: f32, w3: f32, w4: f32, x: &[f32], y: &mut [f32]) {
    let n = x.len();
    y[0] = 0.0;
    y[1] = 0.0;
    let (ts, w2, w3, w4) = (ts as f64, w2 as f64, w3 as f64, w4 as f64);
    let b0 = (w3 * w4 * (ts * ts * w2 + 2.0 * ts)) as f32;
    let b1 = (w3 * w4 * 2.0 * ts * ts * w2) as f32;
    let b2 = (w3 * w4 * (ts * ts * w2 - 2.0 * ts)) as f32;
    let a0 = (w2 * (ts * ts * w3 * w4 + 2.0 * ts * (w3 + w4) + 4.0)) as f32;
    let a1 = (w2 * (2.0 * ts * ts * w3 * w4 - 8.0)) as f32;
    let a2 = (w2 * (ts * ts * w3 * w4 - 2.0 * ts * (w3 + w4) + 4.0)) as f32;
    let (b0, b1, b2, a0, a1, a2) = (
        b0 as f64, b1 as f64, b2 as f64, a0 as f64, a1 as f64, a2 as f64,
    );
    for j in 2..n {
        let (xj, xj1, xj2, yj1, yj2) = (
            x[j] as f64,
            x[j - 1] as f64,
            x[j - 2] as f64,
            y[j - 1] as f64,
            y[j - 2] as f64,
        );
        y[j] = ((1.0 / a0) * (b0 * xj + b1 * xj1 + b2 * xj2 - a1 * yj1 - a2 * yj2)) as f32;
    }
}

/// Pascal `Percentile` (`Pstcalc.pas:579`): interpolated percentile over the
/// sorted window `list[i_lo..=i_hi]`. `pct := 100 - pctExceeded`; the fractional
/// index math and the interpolation are evaluated in f64, narrowed to f32.
///
/// NOTE(upstream-quirk): `nhi := nlo+1` can index one past `i_hi`. When it stays
/// **inside** the histogram allocation (`nhi < list.len()`) it reads the
/// `SetLength`-zeroed / prior-window tail slot — deterministic and defined in
/// FPC/Delphi, so reproduced verbatim (this is what the r3723 goldens pin: e.g.
/// the 0.1 percentile of a 60-sample window reads `hst[60] == 0`). When `nhi`
/// would run **past** the allocation (`nhi == list.len()`, only possible when the
/// sample step does not divide 600 s so a window fills the whole `trunc(600/ts)+1`
/// buffer) the FPC read is a genuine out-of-bounds heap read (UB) → **not
/// reproduced**: clamp to the last valid slot (the `Bus_Int_Duration` OOB
/// precedent). The family/golden decks use a step that divides 600 s, so the
/// clamp is never hit there.
fn percentile(list: &[f32], i_lo: usize, i_hi: usize, pct_exceeded: f32) -> f32 {
    let pct = (100.0 - pct_exceeded as f64) as f32;
    let xhst = (i_hi - i_lo + 1) as f32;
    let idx = 0.01_f64 * pct as f64 * xhst as f64;
    let xfrac = (idx.fract() as f32) as f64;
    let nlo = idx.trunc() as usize;
    let nhi = nlo + 1;
    let xlo = list[nlo] as f64;
    let xhi = if nhi < list.len() {
        list[nhi] as f64
    } else {
        // UB in FPC (read past the histogram allocation): clamp.
        list[list.len() - 1] as f64
    };
    (xlo + xfrac * (xhi - xlo)) as f32
}

/// Pascal `FlickerMeter` (`Pstcalc.pas:594`). `p_rms` enters as the per-sample RMS
/// voltage magnitudes and is **overwritten in place** with the instantaneous
/// flicker level (Block-4 output); `p_pst` (caller-allocated, length `Npst`,
/// zero-filled) receives one short-term severity value per completed 600 s window
/// (Block 5). `p_t` are the absolute sample times (seconds).
///
/// `f_base` selects the 50 Hz vs 60 Hz weighting-filter coefficient set;
/// `v_base` (already narrowed to f32 by the caller, Pascal `Vbase: Single`)
/// normalizes the input to per-unit before filtering.
pub fn flicker_meter(f_base: f64, v_base: f64, p_t: &[f32], p_rms: &mut [f32], p_pst: &mut [f32]) {
    let n = p_rms.len();
    if n < 2 {
        // Pascal reads `pT[2]` for `ts`; a <2-sample stream has no defined step.
        return;
    }

    // Filter coefficients (Double-literal products, narrowed to f32 on store).
    let whp = (2.0 * std::f64::consts::PI * 0.05) as f32;
    let tau = 0.3_f32;
    let cf = (1.0_f64 / 1.285e-6) as f32;
    let (k, lam, w1, w2, w3, w4) = if f_base == 50.0 {
        (
            1.74802_f32,
            (2.0 * std::f64::consts::PI * 4.05981) as f32,
            (2.0 * std::f64::consts::PI * 9.15494) as f32,
            (2.0 * std::f64::consts::PI * 2.27979) as f32,
            (2.0 * std::f64::consts::PI * 1.22535) as f32,
            (2.0 * std::f64::consts::PI * 21.9) as f32,
        )
    } else {
        (
            (1.6357_f64 / 0.783) as f32,
            (2.0 * std::f64::consts::PI * 4.167375) as f32,
            (2.0 * std::f64::consts::PI * 9.077169) as f32,
            (2.0 * std::f64::consts::PI * 2.939902) as f32,
            (2.0 * std::f64::consts::PI * 1.394468) as f32,
            (2.0 * std::f64::consts::PI * 17.31512) as f32,
        )
    };

    let ts = (p_t[1] as f64 - p_t[0] as f64) as f32;

    // Normalize to per-unit (`p_rms[i] / vbase`; vbase is the Double parameter).
    for v in p_rms.iter_mut() {
        *v = (*v as f64 / v_base) as f32;
    }

    // Block 1–4: Fhp -> Fw1 -> Fw2 -> square -> Flp, alternating p_rms/p_buf.
    let mut p_buf = vec![0.0_f32; n];
    fhp(ts, whp, p_rms, &mut p_buf); //  p_buf := Fhp(p_rms)
    fw1(ts, w1, k, lam, &p_buf, p_rms); //  p_rms := Fw1(p_buf)
    fw2(ts, w2, w3, w4, p_rms, &mut p_buf); //  p_buf := Fw2(p_rms)
    for b in p_buf.iter_mut() {
        *b = (*b as f64 * *b as f64) as f32; // square
    }
    flp(ts, tau, &p_buf, p_rms); //  p_rms := Flp(p_buf)
    for v in p_rms.iter_mut() {
        *v = (cf as f64 * *v as f64) as f32; // Block-4 gain
    }

    // Block 5: sliding 600 s Pst via percentile weights.
    // `hst` is the `SetLength(hst, trunc(600/ts)+1)` histogram, zero-filled once
    // and reused across windows (the tail beyond the current window persists —
    // see `percentile`'s NOTE).
    // Pascal `trunc(600.0 / Ts)`: `600.0` is a `Double` literal, `Ts` a `Single`
    // promoted to `Double` — the division is evaluated in `Double` (Delphi Win64,
    // `Extended` ≡ `Double`), per this module's Single-storage/Double-arithmetic
    // model. f32-exact on integer steps; only differs on non-f32-exact `ts`.
    let hst_len = (600.0_f64 / ts as f64).trunc() as usize + 1;
    let mut hst = vec![0.0_f32; hst_len];
    let mut ihst = 0usize; // Low(hst)
    let mut ipst = 0usize; // Pascal ipst=1 (1-based); 0-based write cursor here
    let mut t_pst = 0.0_f32;

    for i in 0..n {
        let t = p_t[i];
        hst[ihst] = p_rms[i];
        // Pascal `(t - tPst) >= 600.0`: `t`/`tPst` are `Single`, promoted to
        // `Double` for the subtraction and comparison (Delphi Win64). `t_pst` is
        // still *stored* f32 below, matching Pascal `tPst: Single`.
        if (t as f64 - t_pst as f64) >= 600.0 {
            // Sort the filled window [0..=ihst] ascending; the tail persists.
            hst[..=ihst].sort_by(f32::total_cmp);

            let p80 = percentile(&hst, 0, ihst, 80.0);
            let p50 = percentile(&hst, 0, ihst, 50.0);
            let p30 = percentile(&hst, 0, ihst, 30.0);
            let p17 = percentile(&hst, 0, ihst, 17.0);
            let p13 = percentile(&hst, 0, ihst, 13.0);
            let p10 = percentile(&hst, 0, ihst, 10.0);
            let p8 = percentile(&hst, 0, ihst, 8.0);
            let p6 = percentile(&hst, 0, ihst, 6.0);
            let p4 = percentile(&hst, 0, ihst, 4.0);
            let p3 = percentile(&hst, 0, ihst, 3.0);
            let p2p2 = percentile(&hst, 0, ihst, 2.2);
            let p1p5 = percentile(&hst, 0, ihst, 1.5);
            let p1 = percentile(&hst, 0, ihst, 1.0);
            let p0p7 = percentile(&hst, 0, ihst, 0.7);
            let p01s = percentile(&hst, 0, ihst, 0.1);

            // Per-band averages and the Pst formula (Double literals → f64).
            let p50s = ((p30 as f64 + p50 as f64 + p80 as f64) / 3.0) as f32;
            let p10s =
                ((p6 as f64 + p8 as f64 + p10 as f64 + p13 as f64 + p17 as f64) / 5.0) as f32;
            let p3s = ((p2p2 as f64 + p3 as f64 + p4 as f64) / 3.0) as f32;
            let p1s = ((p0p7 as f64 + p1 as f64 + p1p5 as f64) / 3.0) as f32;
            let pst = (0.0314 * p01s as f64
                + 0.0525 * p1s as f64
                + 0.0657 * p3s as f64
                + 0.28 * p10s as f64
                + 0.08 * p50s as f64)
                .sqrt() as f32;

            if ipst < p_pst.len() {
                p_pst[ipst] = pst;
            }
            ipst += 1;
            t_pst = t;
            ihst = 0;
        } else {
            ihst += 1;
        }
    }
}

#[cfg(test)]
mod tests;
