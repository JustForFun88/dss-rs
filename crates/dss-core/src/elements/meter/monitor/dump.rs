//! Pascal `TMonitorObj.DumpProperties` (`Meters/Monitor.pas:1811`) — the
//! `Dump monitor.…` override. After the inherited `TDSSCktElement` prefix, all
//! properties dump generically (`~ name=value`); Complete adds a blank line
//! then `// BufferSize=`/`// Hour=`/`// Sec=`/`// BaseFrequency=%.1g`/
//! `// Bufptr=`/the raw `// Buffer=` sample floats (`%.1f`, comma-separated,
//! wrapped every `2 + Fnconds*4` values — a **fixed** wrap width regardless of
//! the monitor's actual per-mode record size, an upstream quirk faithfully
//! reproduced, `Monitor.pas:1841`).
//!
//! NOT_PORTED (audit-code, 2026-07-06): Pascal `BufferSize` is a **fixed**
//! constant `1024` (`Monitor.pas:478`, never doubled/grown), and `MonBuffer`
//! gets **flushed to `MonitorStream` and `BufPtr` reset to `0`** two ways:
//! mid-accumulation once `BufPtr = BufferSize` (`AddDblToBuffer`, `Monitor.pas:
//! 1125-1127`) and — the far more common trigger — unconditionally at the end
//! of every multi-step solve (`TDSSMonitor.SaveAll`, called from every
//! `SolveDaily`/`SolveYearly`/… loop in `SolutionAlgs.pas` right after
//! `MonitorClass.SampleAll`). So a *solved* monitor's `// Bufptr=`/
//! `// Buffer=` dump reads only whatever accumulated **after** the last flush
//! — in practice, after any ordinary `solve`, that's always empty (see
//! [`render_buffer`]'s doc for the direct oracle confirmation). The port's
//! `mon_buffer: Vec<f32>` never flushes (`mod.rs` module doc) and keeps the
//! **whole** sample history instead, so `Dump monitor.<x> debug` on a solved,
//! sampled monitor would diverge from the oracle — untested, since no gate
//! fixture dumps a monitor after sampling it (the `dump_monitor` golden pins
//! only the pre-solve empty-buffer case). `// BufferSize=1024` itself is
//! always correct (the constant, not a running capacity).
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
        out.push_str(&format!("// Bufptr={}\n", self.mon_buffer.len()));
        out.push_str("// Buffer=\n");
        out.push_str(&render_buffer(&self.mon_buffer, 2 + self.med.cd.nconds * 4));
    }
}

/// The `// Buffer=` loop (Pascal `Monitor.pas:1836-1846`): `%0:1` (1-decimal
/// fixed) per value, `", "`-separated, a newline every `wrap` values
/// (`2 + Fnconds*4`, fixed regardless of the monitor's actual per-mode record
/// size — an upstream quirk, faithfully reproduced), plus the unconditional
/// trailing newline after the loop.
///
/// **Oracle-unreachable, pinned here instead of by a golden:** every solve
/// algorithm that calls `MonitorClass.SampleAll` also calls
/// `MonitorClass.SaveAll` at the end of the same procedure
/// (`SolutionAlgs.pas`, e.g. `:139`/`:155` for `SolveDaily`), and
/// `TMonitorObj.Save` unconditionally resets `BufPtr := 0` (`Monitor.pas:
/// 1127`) — so by the time a script can issue `Dump`, a sampled monitor's
/// buffer has already been flushed and is empty (probe-confirmed: a 3-step
/// `solve mode=daily` still dumps `// Bufptr=0`). A non-empty buffer is
/// real, reachable Pascal state (mid-`TakeSample`, between individual solve
/// steps) that the executive can never observe from script — pin the
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
