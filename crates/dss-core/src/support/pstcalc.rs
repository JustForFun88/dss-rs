//! IEC-868 flicker Pst calculator — the **f64** path of Pascal `Pstcalc.pas`
//! (`PstRMS`/`_Pst`/`Get_Pinst`/`Set_Filter_Coefficients`/`SB`/`CalcPst`/
//! `Gather_Bins`/`Sample_Shift`/`Init6Array`, lines 1-474), used by the
//! `Pstcalc` executive command (`ExecHelper.DoPstCalc`). Adapted from Jeff
//! Smith's original C++ IEC-868 flickermeter.
//!
//! The Pascal keeps every filter state, coefficient and bin array in
//! unit-level `var` globals; [`PstEngine`] owns them so a call is
//! self-contained (no cross-call state leak). Everything is `f64` (Pascal
//! `Double`) throughout — the `Single`/f32 `FlickerMeter` (lines 476-687, used
//! by Monitor mode 4) is a separate port (WP-PF.2) and does **not** live here.
//!
//! # FPC `Math.power` for integer exponents (`Set_Filter_Coefficients`)
//!
//! `Set_Filter_Coefficients` uses `power(Tstep, 4)` and `power(Tstep, 3)`. FPC
//! `Math.power(base, exp)` routes any integral `exp` (`Frac(exp)=0` and
//! `Abs(exp) <= MaxInt`) through `intpower` — exponentiation by squaring — not
//! through `exp(exp*ln(base))`. Tracing `intpower`:
//!
//! - `power(Tstep, 4)` = `sqr(sqr(Tstep))` = `(Tstep*Tstep) * (Tstep*Tstep)`;
//! - `power(Tstep, 3)` = `Tstep * sqr(Tstep)` = `Tstep * (Tstep*Tstep)`.
//!
//! We reproduce those exact multiply trees ([`PstEngine::set_filter_coefficients`]),
//! not `f64::powi` (whose intrinsic lowering is not contractually bit-identical
//! to `intpower`). This interpretation is confirmed **empirically, end-to-end**:
//! the `tests/golden/pstcalc/cmd_results.json` matrix drives the pinned oracle
//! at several `dt` values (so `Tstep = 1/(16*Fbase)` varies) and the Rust replay
//! matches every `Format('%.8g')` result string byte-for-byte — a mismatched
//! `power` interpretation would perturb `WC2`/`WD2` and fail those goldens.

/// Pascal `_Pst`: `NumPstIntervals := Max(1, Trunc(Pst_Time_Max / Pst_Time))`
/// with `Pst_Time_Max = N*DeltaT` and `Pst_Time = Min(600, Pst_Time_Max)` — the
/// number of result slots allocated (an upper bound on the completed intervals).
fn num_pst_intervals(n_samples: usize, delta_t: f64) -> i64 {
    let pst_time_max = (n_samples as f64) * delta_t;
    let pst_time = 600.0f64.min(pst_time_max);
    ((pst_time_max / pst_time).trunc() as i64).max(1)
}

/// Number of histogram bins (`number_bins`, Pascal `_Pst`).
const NUMBER_BINS: usize = 16000;
/// Bin ceiling (`bin_ceiling`, Pascal `_Pst`): the max instantaneous flicker
/// level a bin covers. Previously 215, raised to 350 to encompass high flicker.
const BIN_CEILING: f64 = 350.0;
/// Internal rms reference value (`rms_reference`, Pascal `_Pst`; "do not change").
const RMS_REFERENCE: f64 = 120.0;

/// The IEC-868 flickermeter state (Pascal `Pstcalc.pas` unit globals, f64 path).
///
/// A fresh engine is built per [`pst_rms`] call, mirroring the way `_Pst`
/// re-initializes every global at entry.
struct PstEngine {
    // Timing / control (Pascal unit `var`s).
    fbase: f64,
    tstep: f64,   // internal timestep = 1/(16*Fbase)
    delta_t: f64, // NcyclesperSample / Fbase
    rms_input: f64,
    rms_sample: f64,

    // Filter state arrays (`Double6Array`, index 0 = current, 1..=5 = history).
    vin: [f64; 6],
    x1: [f64; 6],
    x2: [f64; 6],
    x3: [f64; 6],
    x4: [f64; 6],
    x5: [f64; 6],
    x6: [f64; 6],
    x7: [f64; 6],
    x8: [f64; 6],
    x9: [f64; 6],
    x10: [f64; 6],
    rms_vin: [f64; 6],

    // Histogram bins (f64).
    bins0: Vec<f64>,
    bins1: Vec<f64>,

    // Weighting-filter coefficients.
    wa2: f64,
    wb2: f64,
    wc2: f64,
    wd2: f64,
    we2: f64,
    wf2: f64,
    wg2: f64,
    // Input-voltage-adapter coefficients.
    ivac: f64,
    ivad: f64,
    ivae: f64,
    // Bandpass coefficients.
    bc: f64,
    bg: f64,
    bh: f64,
    bi: f64,
    bj: f64,
    bk: f64,
    bl: f64,
    bm: f64,
    bn: f64,
    bp: f64,
    // Time constant of sliding-mean filter.
    sa: f64,
    internal_reference: f64,

    lamp_type: i32,  // 0 = 120 V filters, 1 = 230 V filters
    input_type: i32, // 0 = AC, 1 = 1-cycle RMS, 6 = 6-cycle rms
}

impl PstEngine {
    /// Pascal `PstRMS` prologue: fix `input_type = 6`, select `lamp_type` from
    /// the lamp voltage (120 vs 230), set `DeltaT = NcyclesperSample / Fbase`.
    fn new(freq_base: f64, ncycles_per_sample: i32, lamp: i32) -> Self {
        let fbase = freq_base;
        // input_type := 6 (6-cycle rms); default lamp_type := 0 unless Lamp = 230.
        let input_type = 6;
        let lamp_type = if lamp == 230 { 1 } else { 0 };
        let delta_t = f64::from(ncycles_per_sample) / fbase;
        PstEngine {
            fbase,
            tstep: 0.0,
            delta_t,
            rms_input: 0.0,
            rms_sample: 0.0,
            vin: [0.0; 6],
            x1: [0.0; 6],
            x2: [0.0; 6],
            x3: [0.0; 6],
            x4: [0.0; 6],
            x5: [0.0; 6],
            x6: [0.0; 6],
            x7: [0.0; 6],
            x8: [0.0; 6],
            x9: [0.0; 6],
            x10: [0.0; 6],
            rms_vin: [0.0; 6],
            bins0: Vec::new(),
            bins1: Vec::new(),
            wa2: 0.0,
            wb2: 0.0,
            wc2: 0.0,
            wd2: 0.0,
            we2: 0.0,
            wf2: 0.0,
            wg2: 0.0,
            ivac: 0.0,
            ivad: 0.0,
            ivae: 0.0,
            bc: 0.0,
            bg: 0.0,
            bh: 0.0,
            bi: 0.0,
            bj: 0.0,
            bk: 0.0,
            bl: 0.0,
            bm: 0.0,
            bn: 0.0,
            bp: 0.0,
            sa: 0.0,
            internal_reference: 0.0,
            lamp_type,
            input_type,
        }
    }

    /// Pascal `SB`: search `bins` for `y` and interpolate the flicker level.
    fn sb(&self, y: f64, bins: &[f64]) -> f64 {
        let mut n = 0usize;
        // while ((not found) and (n < number_bins)) ...
        while n < NUMBER_BINS {
            if y <= bins[n] {
                break;
            }
            n += 1;
        }
        if n > 0 {
            // Interpolate. With normalized `bins1` (last entry == 1.0 >= every
            // percentile query y <= 0.999) the loop always breaks with
            // `n < number_bins`, so `bins[n]` is in bounds; `min` only guards
            // the unreachable not-found case against a panic (bit-identical to
            // Pascal in every reachable case).
            let nn = n.min(NUMBER_BINS - 1);
            BIN_CEILING * ((n - 1) as f64) / (NUMBER_BINS as f64)
                + (y - bins[n - 1]) * (BIN_CEILING / (NUMBER_BINS as f64))
                    / (bins[nn] - bins[n - 1])
        } else {
            0.0
        }
    }

    /// Pascal `ZeroOutBins`.
    fn zero_out_bins(&mut self) {
        for v in self.bins0.iter_mut() {
            *v = 0.0;
        }
        for v in self.bins1.iter_mut() {
            *v = 0.0;
        }
    }

    /// Pascal `CalcPst`: build the cumulative distribution in `bins1` and read
    /// the IEC percentile points, then combine into a Pst.
    fn calc_pst(&mut self) -> f64 {
        let mut num_pts = 0.0f64; // Pascal comment: "?? long double Why??"
        for n in 0..NUMBER_BINS {
            num_pts += self.bins0[n];
            self.bins1[n] = num_pts;
        }
        for n in 0..NUMBER_BINS {
            self.bins1[n] /= num_pts;
        }

        let bins1 = &self.bins1;
        let p01 = self.sb(0.999, bins1);
        let p1s = (self.sb(0.993, bins1) + self.sb(0.990, bins1) + self.sb(0.985, bins1)) / 3.0;
        let p3s = (self.sb(0.978, bins1) + self.sb(0.970, bins1) + self.sb(0.960, bins1)) / 3.0;
        let p10s = (self.sb(0.940, bins1)
            + self.sb(0.920, bins1)
            + self.sb(0.900, bins1)
            + self.sb(0.870, bins1)
            + self.sb(0.830, bins1))
            / 5.0;
        let p50s = (self.sb(0.700, bins1) + self.sb(0.500, bins1) + self.sb(0.200, bins1)) / 3.0;

        (0.0314 * p01 + 0.0525 * p1s + 0.0657 * p3s + 0.28 * p10s + 0.08 * p50s).sqrt()
    }

    /// Pascal `Set_Filter_Coefficients(input_type)`: the input-voltage-adapter,
    /// bandpass (centered at 8.5 Hz for 120 V, 8.8 Hz for 230 V) and weighting
    /// filter coefficients, plus the `internal_reference` scaling for the given
    /// `input_type`.
    fn set_filter_coefficients(&mut self, input_type: i32) {
        let ts = self.tstep;
        let ts2 = ts * ts; // (Tstep*Tstep)
        // FPC intpower (see module doc): power(Tstep,3) = Tstep*sqr(Tstep),
        // power(Tstep,4) = sqr(sqr(Tstep)).
        let ts3 = ts * ts2;
        let ts4 = ts2 * ts2;

        // Input Voltage Adapter (L = 8.93125 H, C = 35.725 F, R = 1.0 Ohms).
        let ivaa = 8.93125 * 35.725;
        let ivab = 35.725;
        self.ivac = 4.0 * ivaa / ts2 + 1.0 - 2.0 * ivab / ts;
        self.ivad = 2.0 - 8.0 * ivaa / ts2;
        self.ivae = 4.0 * ivaa / ts2 + 1.0 + 2.0 * ivab / ts;

        // Bandpass gain constants: 8.5 Hz (120 V lamp) vs 8.8 Hz (230 V lamp).
        let (k, lambda, w1, w2, w3, w4) = if self.lamp_type == 0 {
            (
                1.6357,
                26.1843893695,
                57.0335348916,
                18.4719490509,
                8.76170084893,
                108.794107576,
            )
        } else {
            (
                1.74802,
                25.5085385419,
                57.5221844961,
                14.3243430315,
                7.69910111615,
                137.601758227,
            )
        };

        // Bandpass, 1st set of substitutions.
        let ba = 0.314159265359;
        let bb = 113.834561498;
        self.bc = 48361.06156533785;
        let bd = 311.00180567;
        let be = 424.836367168;
        // 2nd set of substitutions.
        self.bg = 1.0 + ba * ts / 2.0;
        self.bh = ba * ts / 2.0 - 1.0;
        self.bi = 4.0 / ts2 + 2.0 * bb / ts + self.bc;
        self.bj = -8.0 / ts2 + 2.0 * self.bc;
        self.bk = 4.0 / ts2 - 2.0 * bb / ts + self.bc;
        self.bl = 4.0 / ts2 + 2.0 * bd / ts + self.bc;
        self.bm = 4.0 / ts2 - 2.0 * bd / ts + self.bc;
        self.bn = 4.0 / ts2 + 2.0 * be / ts + self.bc;
        self.bp = 4.0 / ts2 - 2.0 * be / ts + self.bc;

        // Weighting filter.
        self.wa2 = 4.0 * k * w1 * w3 * w4 / ts2;
        self.wb2 = 2.0 * k * w1 * w2 * w3 * w4 / ts;
        self.wc2 = 16.0 * w2 / ts4;
        self.wd2 = 8.0 * w2 * (2.0 * lambda + w3 + w4) / ts3;
        self.we2 = 4.0 * w2 * (w3 * w4 + w1 * w1 + 2.0 * lambda * (w3 + w4)) / ts2;
        self.wf2 = 2.0 * w2 * (2.0 * lambda * w3 * w4 + w1 * w1 * (w3 + w4)) / ts;
        self.wg2 = w2 * w3 * w4 * w1 * w1;

        // Time constant of sliding mean filter.
        self.sa = 0.3;

        // internal reference. The 0/1/3 branches are DEAD — `PstRMS` fixes
        // `input_type := 6` — but the whole table is ported verbatim (Pascal
        // uses independent `if`s, not an `else` chain).
        if input_type == 0 {
            self.internal_reference = 676.372; // AC
        }
        if input_type == 1 {
            self.internal_reference = 0.01106784; // 1-cycle RMS
        }
        if input_type == 3 {
            self.internal_reference = 0.009; // 3-cycle RMS
        }
        if input_type == 6 {
            self.internal_reference = 0.008449; // 6-cycle RMS (the live path)
        }
    }

    /// Pascal `Gather_Bins`: bin the instantaneous flicker level `x10_value`.
    fn gather_bins_bins0(&mut self, x10_value: f64) {
        if x10_value > BIN_CEILING {
            self.bins0[NUMBER_BINS - 1] += 1.0;
        } else {
            // trunc(number_bins * X10_value / bin_ceiling). With
            // 0 <= X10_value < bin_ceiling the index is in [0, number_bins);
            // `min` guards the boundary X10_value == bin_ceiling (which upstream
            // would index one past the allocation — a UB heap write, not
            // reproduced) so Rust cannot panic. Bit-identical in every reachable
            // case.
            let idx = ((NUMBER_BINS as f64) * x10_value / BIN_CEILING).trunc() as usize;
            let idx = idx.min(NUMBER_BINS - 1);
            self.bins0[idx] += 1.0;
        }
    }

    /// Pascal `Sample_Shift`: push every 6-array one step back in time.
    fn sample_shift(&mut self) {
        for n in (1..=5).rev() {
            self.vin[n] = self.vin[n - 1];
            self.rms_vin[n] = self.rms_vin[n - 1];
            self.x1[n] = self.x1[n - 1];
            self.x2[n] = self.x2[n - 1];
            self.x3[n] = self.x3[n - 1];
            self.x4[n] = self.x4[n - 1];
            self.x5[n] = self.x5[n - 1];
            self.x6[n] = self.x6[n - 1];
            self.x7[n] = self.x7[n - 1];
            self.x8[n] = self.x8[n - 1];
            self.x9[n] = self.x9[n - 1];
            self.x10[n] = self.x10[n - 1];
        }
    }

    /// Pascal `Get_Pinst`: one flickermeter step — input adapter (Block 1/2),
    /// bandpass (Block 3), weighting filter, sliding-mean filter (Block 4),
    /// leaving the instantaneous flicker level in `x10[0]`.
    fn get_pinst(&mut self) {
        // RMS input, per-unitized.
        self.rms_vin[0] = RMS_REFERENCE * self.rms_sample / self.rms_input;

        self.x1[0] = (self.rms_vin[0] + 2.0 * self.rms_vin[1] + self.rms_vin[2]
            - self.ivad * self.x1[1]
            - self.ivac * self.x1[2])
            / self.ivae;
        self.x3[0] = self.rms_vin[0] * (1.0 - (self.x1[0] - 120.0) / self.rms_vin[0]);

        // Bandpass (HP at .05 Hz, 6th-order Butterworth LP at 35 Hz).
        self.x4[0] = (self.x3[0] - self.x3[1] - self.bh * self.x4[1]) / self.bg;
        self.x5[0] = (self.bc * (self.x4[0] + 2.0 * self.x4[1] + self.x4[2])
            - (self.bj * self.x5[1] + self.bk * self.x5[2]))
            / self.bi;
        self.x6[0] = (self.bc * (self.x5[0] + 2.0 * self.x5[1] + self.x5[2])
            - (self.bj * self.x6[1] + self.bm * self.x6[2]))
            / self.bl;
        self.x7[0] = (self.bc * (self.x6[0] + 2.0 * self.x6[1] + self.x6[2])
            - (self.bj * self.x7[1] + self.bp * self.x7[2]))
            / self.bn;

        // Weighting filter.
        self.x8[0] = ((self.wa2 + self.wb2) * self.x7[0] + 2.0 * self.wb2 * self.x7[1]
            - 2.0 * self.wa2 * self.x7[2]
            - 2.0 * self.wb2 * self.x7[3]
            + (self.wa2 - self.wb2) * self.x7[4]
            - (2.0 * self.wf2 + 4.0 * self.wg2 - 4.0 * self.wc2 - 2.0 * self.wd2) * self.x8[1]
            - (6.0 * self.wc2 - 2.0 * self.we2 + 6.0 * self.wg2) * self.x8[2]
            - (2.0 * self.wd2 + 4.0 * self.wg2 - 4.0 * self.wc2 - 2.0 * self.wf2) * self.x8[3]
            - (self.wc2 - self.wd2 + self.we2 - self.wf2 + self.wg2) * self.x8[4])
            / (self.wc2 + self.wd2 + self.we2 + self.wf2 + self.wg2);

        // Sliding mean filter.
        self.x9[0] = (self.x8[0] * self.x8[0] + self.x8[1] * self.x8[1]
            - (1.0 - 2.0 * self.sa / self.tstep) * self.x9[1])
            / (1.0 + 2.0 * self.sa / self.tstep);
        self.x10[0] = self.x9[0] / self.internal_reference;
    }

    /// Pascal `_Pst`: run the whole flickermeter over `varray` and return the
    /// per-10-minute-interval Pst values (length = number of completed
    /// intervals — Pascal's `PstInterval` return, which the command formats).
    fn run(&mut self, varray: &[f64]) -> Vec<f64> {
        // init6Array inits (RMSVin and X1 to rms_reference, the rest to 0).
        self.vin = [0.0; 6];
        self.rms_vin = [RMS_REFERENCE; 6];
        self.x1 = [RMS_REFERENCE; 6];
        self.x2 = [0.0; 6];
        self.x3 = [0.0; 6];
        self.x4 = [0.0; 6];
        self.x5 = [0.0; 6];
        self.x6 = [0.0; 6];
        self.x7 = [0.0; 6];
        self.x8 = [0.0; 6];
        self.x9 = [0.0; 6];
        self.x10 = [0.0; 6];

        self.bins0 = vec![0.0; NUMBER_BINS];
        self.bins1 = vec![0.0; NUMBER_BINS];

        let mut time = 0.0f64;
        let mut pst_timer = 0.0f64;

        // Bins already zeroed by the allocations above (Pascal calls ZeroOutBins).

        self.tstep = 1.0 / (16.0 * self.fbase); // 16 samples/cycle

        let pst_time_max = (varray.len() as f64) * self.delta_t;
        let pst_time = 600.0f64.min(pst_time_max);
        let num_pst_intervals = num_pst_intervals(varray.len(), self.delta_t);

        // SetLength(PstResult, NumPstIntervals) — zero-filled.
        let mut pst_result = vec![0.0f64; num_pst_intervals as usize];

        self.set_filter_coefficients(self.input_type);

        let samples_per_delta_t = self.delta_t / self.tstep;
        // round(SamplesPerDeltaT) — FPC banker's Round (ties-to-even). The value
        // is 16*NcyclesperSample modulo fp rounding, always integral in range.
        // Pascal `Round` = ties-to-even; `round_ties_even` reproduces it
        // exactly (see RegControl `get_tap_num`).
        let samples_per_delta_t_rounded = samples_per_delta_t.round_ties_even() as i64;

        let first_sample = varray[0];
        self.rms_input = first_sample;
        self.rms_sample = first_sample;

        // Init filter to 1 PU for 30 s.
        while time < 30.0 {
            time += self.tstep;
            self.get_pinst();
            self.sample_shift();
        }

        // Give it 5 s to settle after real data starts.
        let pst_start_time = time + 5.0;

        let mut pst_interval: i64 = 0;
        for &sample in varray.iter() {
            self.rms_sample = sample;
            // Hold the rms input constant over the RMS period.
            for _ in 1..=samples_per_delta_t_rounded {
                self.get_pinst();

                if time >= pst_start_time {
                    pst_timer += self.tstep;
                    // max_flicker tracking (Pascal keeps it but never reads it
                    // out) is elided — pure dead accumulation with no effect on
                    // the result.
                    self.gather_bins_bins0(self.x10[0]);

                    if pst_timer >= pst_time {
                        // Got everything in the bins — compute the Pst.
                        pst_interval += 1;
                        if pst_interval <= num_pst_intervals {
                            pst_result[(pst_interval - 1) as usize] = self.calc_pst();
                        }
                        pst_timer = 0.0;
                        self.zero_out_bins();
                    }
                }
                self.sample_shift();
                time += self.tstep;
            }
        }

        // Result := PstInterval (the count actually computed). The command
        // iterates `1..=nPst`; `pst_interval <= num_pst_intervals` always holds
        // (the head 35 s never counts, so at most NumPstIntervals complete), so
        // truncating to the computed prefix drops only the trailing zero-fill.
        pst_result.truncate(pst_interval.max(0) as usize);
        pst_result
    }
}

/// Pascal `PstRMS` (`Pstcalc.pas:452`): compute the IEC-868 Pst values for a
/// sequence of per-unit-ish RMS `voltages`. `freq_base` (50 or 60) selects the
/// weighting coefficients, `cycles_per_sample` is the number of base-frequency
/// cycles each supplied voltage sample spans, and `lamp` (120 or 230) picks the
/// bandpass gain. Returns one Pst per completed 10-minute interval.
pub fn pst_rms(voltages: &[f64], freq_base: f64, cycles_per_sample: i32, lamp: i32) -> Vec<f64> {
    let mut engine = PstEngine::new(freq_base, cycles_per_sample, lamp);
    engine.run(voltages)
}

#[cfg(test)]
mod tests;
