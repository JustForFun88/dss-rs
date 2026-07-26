//! Rolling-average window helper. Port of Pascal `Controls/RollAvgWindow.pas`
//! (`TRollAvgWindow`) — a fixed-capacity FIFO of (value, time) samples with
//! running sums, used by [`InvControl`] for the volt-var / DRC rolling-average
//! voltage. Not a DSS object (no props, never New-able): InvControl owns two of
//! these (`FRollAvgWindow`, `FDRCRollAvgWindow`) and feeds them the solution
//! voltage + `DynaVars.h` each `Sample`.
//!
//! Backed by two FIFO queues (`sample`, `sampletime`) plus the running sums
//! Pascal keeps so `AvgVal`/`AccumSec` are O(1). `bufferlength` is the maximum
//! number of samples (Pascal `Integer`); `bufferfull` latches once the window
//! is full (by count or by the accumulated-time threshold) and from then on each
//! `Add` evicts the oldest sample.
//!
//! [`InvControl`]: super
//! [`InvControl`]: crate::elements::control

use std::collections::VecDeque;

/// Pascal `TRollAvgWindow`. Field names mirror the unit (`bufferlength`,
/// `runningsumsample`, ...) for a 1:1 read against the Pascal.
#[derive(Debug, Clone, Default)]
pub struct RollAvgWindow {
    /// `bufferlength` — max samples; `SetLength` writes it (Pascal `Integer`).
    buffer_length: i32,
    /// `sample` — the FIFO of sample values.
    sample: VecDeque<f64>,
    /// `sampletime` — the FIFO of sample times (parallel to `sample`).
    sample_time: VecDeque<f64>,
    /// `runningsumsample` — Σ of the values currently in `sample`.
    running_sum_sample: f64,
    /// `runningsumsampletime` — Pascal's running sum of `sampletime` (see the
    /// asymmetry note in [`add`](Self::add); only ever read by the upstream-dead
    /// `AccumSec`).
    running_sum_sample_time: f64,
    /// `bufferfull` — latches true once the window has filled.
    buffer_full: bool,
}

impl RollAvgWindow {
    /// Pascal `TRollAvgWindow.Create` — empty queues, zero sums, length 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pascal `TRollAvgWindow.SetLength` — set the buffer capacity.
    pub fn set_length(&mut self, value: i32) {
        self.buffer_length = value;
    }

    /// Pascal `TRollAvgWindow.Add`. Once `bufferfull`, evict the oldest sample
    /// and push the incoming one; otherwise just grow the window, latching
    /// `bufferfull` when it reaches `bufferlength` samples or when the
    /// accumulated time exceeds `VAvgWindowLengthSec`.
    ///
    /// When `bufferlength = 0` the incoming *value* is forced to 0 (Pascal),
    /// while the *time* is still recorded.
    pub fn add(
        &mut self,
        mut incoming_sample_value: f64,
        incoming_sample_time: f64,
        v_avg_window_length_sec: f64,
    ) {
        if !self.sample.is_empty() && self.buffer_full {
            // Pascal subtracts the *old* front of `sample` before popping it.
            self.running_sum_sample -= *self.sample.front().unwrap();
            if self.buffer_length == 0 {
                incoming_sample_value = 0.0;
            }
            self.sample.pop_front();
            self.sample.push_back(incoming_sample_value);
            self.sample_time.pop_front();
            self.sample_time.push_back(incoming_sample_time);

            self.running_sum_sample += incoming_sample_value;
            // NOTE: upstream asymmetry, reproduced verbatim — for the *time* sum
            // Pascal reads `sampletime.front` *after* the pop+push (the new
            // front), not the evicted value it just removed, so this sum drifts
            // from the true Σ of `sampletime`. Harmless: its only reader,
            // `AccumSec` (-> [`accum_sec`](Self::accum_sec)), is dead in the
            // upstream tree, so the drift is never observed. Carries no compat
            // marker (no golden pins it).
            self.running_sum_sample_time -= *self.sample_time.front().unwrap();
            self.running_sum_sample_time += incoming_sample_time;
        } else {
            if self.buffer_length == 0 {
                incoming_sample_value = 0.0;
            }
            self.sample.push_back(incoming_sample_value);
            self.sample_time.push_back(incoming_sample_time);

            self.running_sum_sample += incoming_sample_value;
            self.running_sum_sample_time += incoming_sample_time;

            if self.running_sum_sample_time > v_avg_window_length_sec {
                self.buffer_full = true;
            }
            if self.sample.len() as i64 == self.buffer_length as i64 {
                self.buffer_full = true;
            }
        }
    }

    /// Pascal `TRollAvgWindow.AvgVal` — mean of the windowed values (0 if empty).
    pub fn avg_val(&self) -> f64 {
        if self.sample.is_empty() {
            0.0
        } else {
            self.running_sum_sample / self.sample.len() as f64
        }
    }

    /// Pascal `TRollAvgWindow.AccumSec` — the accumulated sample time (0 if
    /// empty). Upstream-dead (no caller in the Pascal tree); ported for
    /// completeness, see the asymmetry note on [`add`](Self::add).
    pub fn accum_sec(&self) -> f64 {
        if self.sample.is_empty() {
            0.0
        } else {
            self.running_sum_sample_time
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh window is empty: both accessors return 0.
    #[test]
    fn empty_window_reads_zero() {
        let w = RollAvgWindow::new();
        assert_eq!(w.avg_val(), 0.0);
        assert_eq!(w.accum_sec(), 0.0);
    }

    /// Fill a length-3 window (large time threshold so it fills by count), then
    /// one eviction. Pins the exact running-sum arithmetic, including the
    /// `sampletime`-front asymmetry on the eviction.
    #[test]
    fn fills_by_count_then_evicts() {
        let mut w = RollAvgWindow::new();
        w.set_length(3);
        // Large window so the time threshold never trips the fill.
        w.add(10.0, 1.0, 1000.0); // size 1, not full
        assert_eq!(w.avg_val(), 10.0);
        w.add(20.0, 2.0, 1000.0); // size 2, not full
        w.add(30.0, 3.0, 1000.0); // size 3 == bufferlength -> full
        assert_eq!(w.avg_val(), 20.0); // 60/3
        assert_eq!(w.accum_sec(), 6.0); // 1+2+3

        // Eviction: drop value 10 / time 1, push 40 / time 4.
        w.add(40.0, 4.0, 1000.0);
        assert_eq!(w.avg_val(), 30.0); // (20+30+40)/3
        // Quirk: the time sum subtracts the *new* front (2), not the evicted
        // value (1): 6 - 2 + 4 = 8 (symmetric logic would give 6 - 1 + 4 = 9).
        assert_eq!(w.accum_sec(), 8.0);
    }

    /// `bufferlength = 0` forces every stored value to 0 but still records the
    /// times. The window fills via the time threshold, not the count.
    #[test]
    fn zero_length_zeroes_values_keeps_times() {
        let mut w = RollAvgWindow::new();
        w.set_length(0);
        w.add(10.0, 1.0, 5.0); // value forced 0; sum-time 1, not > 5
        w.add(20.0, 2.0, 5.0); // value forced 0; sum-time 3, not > 5
        w.add(30.0, 6.0, 5.0); // value forced 0; sum-time 9 > 5 -> full
        assert_eq!(w.avg_val(), 0.0);
        assert_eq!(w.accum_sec(), 9.0);

        w.add(40.0, 1.0, 5.0); // eviction, value still 0
        assert_eq!(w.avg_val(), 0.0);
        // 9 - new_front(2) + 1 = 8.
        assert_eq!(w.accum_sec(), 8.0);
    }

    /// The window can latch full by the accumulated-time threshold before it
    /// ever reaches `bufferlength` samples.
    #[test]
    fn fills_by_time_threshold_below_capacity() {
        let mut w = RollAvgWindow::new();
        w.set_length(100); // capacity never reached in this test
        w.add(10.0, 3.0, 5.0); // sum-time 3, not > 5, not full
        w.add(20.0, 4.0, 5.0); // sum-time 7 > 5 -> full at size 2
        assert_eq!(w.avg_val(), 15.0); // 30/2
        assert_eq!(w.accum_sec(), 7.0);

        w.add(30.0, 2.0, 5.0); // eviction at size 2
        assert_eq!(w.avg_val(), 25.0); // (20+30)/2
        // 7 - new_front(4) + 2 = 5.
        assert_eq!(w.accum_sec(), 5.0);
    }

    /// The time-threshold latch is strict `>` (Pascal `runningsumsampletime >
    /// VAvgWindowLengthSec`): a window whose accumulated time exactly equals the
    /// length must NOT latch full. Capacity is large so the count never latches;
    /// the second add then grows (else branch) rather than evicting — observable
    /// via the size-2 average. A regression to `>=` would evict and read 20.0.
    #[test]
    fn time_threshold_is_strict_greater() {
        let mut w = RollAvgWindow::new();
        w.set_length(100); // capacity never reached
        w.add(10.0, 3.0, 3.0); // sum-time 3, NOT > 3 -> stays open
        w.add(20.0, 1.0, 3.0); // window still open -> grows to size 2
        assert_eq!(w.avg_val(), 15.0); // (10+20)/2; an early `>=` latch -> 20.0
    }
}
