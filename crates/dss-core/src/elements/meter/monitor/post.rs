//! Pascal `TMonitorObj.PostProcess` + `DoFlickerCalculations`
//! (`Meters/Monitor.pas:1136-1144, 1602-1688`): the mode-4 flicker/Pst
//! post-processing that rewrites the recorded (mag, angle) stream in place with
//! (flicker level, Pst) per phase.

use super::{Monitor, MonitorBaseMode};
use crate::support::flicker::flicker_meter;

impl Monitor {
    /// Pascal `TMonitorObj.PostProcess` (`Monitor.pas:1136`): run
    /// `DoFlickerCalculations` once, latched by `is_processed`, when this is a
    /// mode-4 monitor with recorded samples. `kv_base` is the metered terminal
    /// bus's `kVBase` (the caller resolves it from the circuit, as Pascal reads
    /// `ActiveCircuit.Buses[busref].kVBase` at close time).
    ///
    /// Invoked wherever Pascal's `CloseMonitorStream` fires — i.e. from `to_csv`
    /// (`Export`/`Show Monitor` → `TranslateToCSV`). `Monitors.Channel` /
    /// `ByteStream` do **not** post-process (oracle-probed: they return the raw
    /// (mag, angle) buffer), so the harness `channel()` accessors stay raw.
    pub fn post_process(&mut self, kv_base: f64) {
        if self.is_processed {
            return;
        }
        // Pascal guard: `(mode = 4) and (MonitorStream.Position > 0)`.
        if self.mode.base == MonitorBaseMode::Flicker && self.sample_count > 0 {
            self.do_flicker_calculations(kv_base);
        }
        self.is_processed = true;
    }

    /// Pascal `TMonitorObj.DoFlickerCalculations` (`Monitor.pas:1602`). Reads the
    /// per-phase RMS magnitudes (the **odd** data channels; angles ignored) out of
    /// the recorded stream, runs [`flicker_meter`] per phase, and rewrites the
    /// stream in place: data channel `2p-2` ← flicker level, `2p-1` ← the Pst of
    /// the current 600 s interval (0 until the first interval completes).
    fn do_flicker_calculations(&mut self, kv_base: f64) {
        let n = self.sample_count as usize;
        let nphases = self.record_size / 2;
        if n == 0 || nphases == 0 {
            return;
        }
        let stride = self.record_size + 2;

        // Read times (data[0][i] = s + 3600·hr, narrowed to f32) and the per-phase
        // magnitude channels (odd Pascal channel = SngBuffer[2p-1]).
        let mut times = vec![0.0_f32; n];
        let mut mags: Vec<Vec<f32>> = vec![vec![0.0_f32; n]; nphases];
        for i in 0..n {
            let base = i * stride;
            let hr = self.mon_buffer[base] as f64;
            let s = self.mon_buffer[base + 1] as f64;
            times[i] = (s + 3600.0 * hr) as f32;
            for (p, mag) in mags.iter_mut().enumerate() {
                mag[i] = self.mon_buffer[base + 2 + 2 * p];
            }
        }

        // Npst = 1 + Trunc(t_N / 600); Vbase = 1000·kVBase (narrowed to Single).
        // Pascal `Trunc(data[0][N] / 600.0)`: `data[0][N]` is `Single`, `600.0` a
        // `Double` literal, so the division is `Double` (Delphi Win64) — matches
        // `flicker::flicker_meter`'s Single-storage/Double-arithmetic model.
        let npst = 1 + (times[n - 1] as f64 / 600.0).trunc() as usize;
        let vbase = ((1000.0 * kv_base) as f32) as f64;
        let f_base = self.med.cd.base_frequency;

        // Per-phase flicker level (overwrites `mag`) + Pst array.
        let mut psts: Vec<Vec<f32>> = Vec::with_capacity(nphases);
        for mag in mags.iter_mut() {
            let mut ppst = vec![0.0_f32; npst];
            flicker_meter(f_base, vbase, &times, mag, &mut ppst);
            psts.push(ppst);
        }

        // Rewrite the stream in place with the exact ipst/tpst stepping
        // (Monitor.pas:1662-1681).
        let mut t_pst = 0.0_f32;
        let mut ipst = 0usize;
        for i in 0..n {
            // Pascal `(data[0][i] - tpst) >= 600.0`: `Single` operands promoted to
            // `Double` for the subtraction/compare (Delphi Win64); `t_pst` stays
            // f32-stored, as Pascal `tpst: Single`.
            if (times[i] as f64 - t_pst as f64) >= 600.0 {
                ipst += 1;
                t_pst = times[i];
            }
            let base = i * stride;
            for p in 0..nphases {
                self.mon_buffer[base + 2 + 2 * p] = mags[p][i]; // flicker level
                let pst = if ipst >= 1 && ipst <= npst {
                    psts[p][ipst - 1]
                } else {
                    0.0
                };
                self.mon_buffer[base + 2 + 2 * p + 1] = pst;
            }
        }
    }
}
