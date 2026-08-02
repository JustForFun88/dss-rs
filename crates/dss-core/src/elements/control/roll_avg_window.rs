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
    /// `runningsumsampletime` — Σ of the times currently in `sample_time`.
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
            // Symmetric with the value sum above: the *evicted* time leaves the
            // running sum, as the authority does — both of r4133's conditionally
            // compiled arms subtract the dequeued element
            // (`Version8/Source/Controls/InvControl.pas:4320`
            // `- sampletime.Dequeue`, and `:4351` `- sampletime.front;
            // sampletime.pop`). dss_capi's extracted class merged pop+push
            // (`src/Controls/RollAvgWindow.pas:40`) and left the subtraction
            // after it (`:43`), so it removes the *new* head and the sum drifts
            // from Σ `sampletime`. Not reproduced.
            self.running_sum_sample_time -= *self.sample_time.front().unwrap();
            self.sample_time.pop_front();
            self.sample_time.push_back(incoming_sample_time);

            self.running_sum_sample += incoming_sample_value;
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
    /// empty), i.e. Σ of the times currently in the window. Upstream-dead (no
    /// caller in either Pascal tree); ported for completeness.
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
    /// one eviction. Pins the exact running-sum arithmetic on both sums.
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
        // The *evicted* time leaves the sum: 6 - 1 + 4 = 9. (dss_capi subtracts
        // the new front instead and would read 8 — r4133 subtracts the dequeued
        // element, `InvControl.pas:4320`/`:4351`.)
        assert_eq!(w.accum_sec(), 9.0);
        assert_eq!(
            w.accum_sec(),
            w.sample_time.iter().sum::<f64>(),
            "the running time sum is Σ of the window's contents"
        );
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
        // 9 - evicted(1) + 1 = 9.
        assert_eq!(w.accum_sec(), 9.0);
        assert_eq!(
            w.accum_sec(),
            w.sample_time.iter().sum::<f64>(),
            "the running time sum is Σ of the window's contents"
        );
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
        // 7 - evicted(3) + 2 = 6.
        assert_eq!(w.accum_sec(), 6.0);
        assert_eq!(
            w.accum_sec(),
            w.sample_time.iter().sum::<f64>(),
            "the running time sum is Σ of the window's contents"
        );
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
