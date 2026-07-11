//! Pascal `TMonitorObj.DumpProperties` (`Meters/Monitor.pas:1811`) — the
//! `Dump monitor.…` override. After the inherited `TDSSCktElement` prefix, all
//! properties dump generically (`~ name=value`); Complete adds a blank line
//! then `// BufferSize=`/`// Hour=`/`// Sec=`/`// BaseFrequency=%.1g`/
//! `// Bufptr=`/the raw `// Buffer=` sample floats (`%.1f`, comma-separated,
//! wrapped every `2 + Fnconds*4` values — a **fixed** wrap width regardless of
//! the monitor's actual per-mode record size, an upstream quirk faithfully
//! reproduced, `Monitor.pas:1841`).
//!
//! Pascal `BufferSize` is a **fixed** constant `1024` (`Monitor.pas:478`,
//! never doubled/grown, so `// BufferSize=1024` is always correct — not a
//! running capacity). `MonBuffer` gets **flushed and `BufPtr` reset to `0`**
//! two ways: mid-accumulation once `BufPtr = BufferSize` (`AddDblToBuffer`,
//! `Monitor.pas:1596`, per single so it can fire mid-record — modeled here by
//! the `bufptr` cursor, see `mod.rs`) and — the far more common trigger —
//! unconditionally at the end of every multi-step solve (`TDSSMonitor.
//! SaveAll`, called from every `SolveDaily`/`SolveYearly`/… loop in
//! `SolutionAlgs.pas` right after `MonitorClass.SampleAll`, **except**
//! `SolveGeneralTime`/`SolveFaultStudy` — WPG.2 oracle probe). So a *solved*
//! monitor's `// Bufptr=`/`// Buffer=` dump reads only whatever accumulated
//! **after** the last flush — in practice, after any ordinary `solve`, that's
//! empty (see [`render_buffer`]'s doc for the direct oracle confirmation),
//! but genuinely non-empty after a `SolveGeneralTime` step (no gate fixture
//! dumps a monitor mid-`mode=Time` run; the `dump_monitor` golden pins only
//! the pre-solve empty-buffer case). The port renders the trailing `bufptr`
//! singles of `mon_buffer` (the genuine Pascal-fidelity pending slice), not the
//! whole sample history.
//!
//! **`BaseFrequency=%.1g` quirk (probe-proven, 2026-07-06):** the oracle
//! renders a base frequency of 60 as plain `60`, not the C-style 1-sig-fig
//! scientific `6E+01` `%.1g` would imply — varying the circuit/global base
//! frequency did not change the captured value (stuck at `60` across every
//! probe), so the true low-precision `%g` threshold rule is unconfirmed
//! (tracked alongside the similar low-end `%g` gap in `format::g`'s doc).
//! Rendered via the bare-`%-g` 15-sig-fig convention ([`crate::report::format::g`]
//! with `sig=15`), which reproduces the one confirmed byte value (`60`) and,
//! for any realistic base frequency, never approaches the sig-15 scientific
//! threshold either.

use crate::report::format::fpc_sci_w;
use crate::report::save::dump::{self, DumpCtx};

use super::Monitor;

impl Monitor {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_ckt(out, cx, self, &self.med.cd, complete);
        dump::generic_props(out, cx, self);

        if !complete {
            return;
        }

        out.push('\n');
        out.push_str("// BufferSize=1024\n");
        out.push_str(&format!("// Hour={}\n", self.hour));
        out.push_str(&format!("// Sec={}\n", fpc_sci_w(self.sec, 0)));
        out.push_str(&format!(
            "// BaseFrequency={}\n",
            crate::report::format::g(self.med.cd.base_frequency, 15)
        ));
        // Pending slice: the live `MonBuffer` scratch since the last flush
        // (Pascal `MonBuffer[1..BufPtr]`). `bufptr` is single-granular and wraps
        // every 1024 singles (`AddDblToBuffer`), so it can begin mid-record — the
        // faithful upstream quirk; in the merged buffer those are the trailing
        // `bufptr` singles.
        let pending = &self.mon_buffer[self.mon_buffer.len() - self.bufptr..];
        out.push_str(&format!("// Bufptr={}\n", self.bufptr));
        out.push_str("// Buffer=\n");
        out.push_str(&render_buffer(pending, 2 + self.med.cd.nconds * 4));
    }
}

/// The `// Buffer=` loop (Pascal `Monitor.pas:1836-1846`): `%0:1` (1-decimal
/// fixed) per value, `", "`-separated, a newline every `wrap` values
/// (`2 + Fnconds*4`, fixed regardless of the monitor's actual per-mode record
/// size — an upstream quirk, faithfully reproduced), plus the unconditional
/// trailing newline after the loop.
///
/// **Empty for every currently-gated fixture, pinned here instead of by a
/// golden:** every *ordinary* solve algorithm that calls
/// `MonitorClass.SampleAll` also calls `MonitorClass.SaveAll` at the end of
/// the same procedure (`SolutionAlgs.pas`, e.g. `:139`/`:155` for
/// `SolveDaily`), and `TMonitorObj.Save` unconditionally resets `BufPtr := 0`
/// (`Monitor.pas:1127`) — so by the time a script can issue `Dump`, a sampled
/// monitor's buffer has already been flushed and is empty (probe-confirmed: a
/// 3-step `solve mode=daily` still dumps `// Bufptr=0`). `SolveGeneralTime`
/// (`mode=Time`) is the one ported exception — it never calls `SaveAll`
/// (WPG.2 oracle probe), so `Dump` mid-run genuinely observes a non-empty
/// pending buffer there; no gate fixture currently dumps a monitor in that
/// mode, so this case is documented rather than golden-pinned too. A
/// non-empty buffer is otherwise real, reachable Pascal state (mid-
/// `TakeSample`, between individual solve steps) that the executive can never
/// observe from script — pin the
/// rendering rule directly instead (`tests::` below).
fn render_buffer(buf: &[f32], wrap: usize) -> String {
    let mut out = String::new();
    let mut k = 0usize;
    for &v in buf {
        out.push_str(&format!("{v:.1}, "));
        k += 1;
        if k == wrap {
            out.push('\n');
            k = 0;
        }
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::render_buffer;
    use crate::elements::meter::monitor::Monitor;

    /// Pascal `AddDblToBuffer` (`Monitor.pas:1591`): the 1024-single `BufferSize`
    /// flush boundary. The check is a *pre-increment* compare, so the flush fires
    /// on the 1025th `add_dbl`, not the 1024th. Drive 1030 singles → one flush →
    /// `bufptr == 6`, and the `// Buffer=` pending slice is exactly the last 6
    /// singles (the post-flush remainder, faithfully mid-record).
    #[test]
    fn bufptr_wraps_at_1024_and_pending_is_remainder() {
        let mut m = Monitor::new("m");
        for i in 0..1024 {
            m.add_dbl(i as f64);
        }
        assert_eq!(m.bufptr, 1024, "no flush until BufPtr == BufferSize");
        m.add_dbl(1024.0); // 1025th add: pre-check flushes, BufPtr := 1
        assert_eq!(m.bufptr, 1);
        for i in 1025..1030 {
            m.add_dbl(i as f64);
        }
        assert_eq!(m.bufptr, 6, "1030 adds → one flush → remainder of 6");
        assert_eq!(m.mon_buffer.len(), 1030, "no single is ever discarded");

        // Pending slice = the trailing `bufptr` singles (Pascal MonBuffer[1..BufPtr]).
        let pending = &m.mon_buffer[m.mon_buffer.len() - m.bufptr..];
        assert_eq!(
            pending,
            &[1024.0f32, 1025.0, 1026.0, 1027.0, 1028.0, 1029.0]
        );
        // `// Buffer=` render: wrap = 2 + Fnconds*4; here 6, so the wrap newline
        // fires after the 6th single, then the loop's unconditional trailing
        // newline follows (Pascal `FSWriteln(F)` at Monitor.pas:1847).
        assert_eq!(
            render_buffer(pending, 6),
            "1024.0, 1025.0, 1026.0, 1027.0, 1028.0, 1029.0, \n\n"
        );

        // Pascal `Save` resets BufPtr to 0 (Monitor.pas:1127); the merged stream
        // keeps every single, so nothing is lost.
        m.sample_count = 0;
        m.save();
        assert_eq!(m.bufptr, 0);
        assert_eq!(m.mon_buffer.len(), 1030);
    }

    /// No samples (the only oracle-reachable case, `dump_monitor` golden):
    /// just the trailing newline, no wrap ever fires.
    #[test]
    fn render_buffer_empty() {
        assert_eq!(render_buffer(&[], 14), "\n");
    }

    /// Fewer values than `wrap`: one line, comma-separated, `%.1f`.
    #[test]
    fn render_buffer_under_wrap() {
        assert_eq!(render_buffer(&[1.0, 2.5, -3.0], 14), "1.0, 2.5, -3.0, \n");
    }

    /// More values than `wrap`: a newline fires exactly every `wrap` values,
    /// splitting mid-sample when `wrap` doesn't divide the record stride —
    /// the fixed-width-regardless-of-record-size quirk `Monitor.pas:1841`
    /// bakes in (`wrap=2+Fnconds*4`, unrelated to the mode's actual stride).
    #[test]
    fn render_buffer_wraps_at_fixed_width() {
        let buf: Vec<f32> = (1..=5).map(|i| i as f32).collect();
        assert_eq!(render_buffer(&buf, 2), "1.0, 2.0, \n3.0, 4.0, \n5.0, \n");
    }
}
