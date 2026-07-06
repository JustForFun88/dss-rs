//! Pascal `TMonitorObj.DumpProperties` (`Meters/Monitor.pas:1811`) — the
//! `Dump monitor.…` override. After the inherited `TDSSCktElement` prefix, all
//! properties dump generically (`~ name=value`); Complete adds a blank line
//! then `// BufferSize=`/`// Hour=`/`// Sec=`/`// BaseFrequency=%.1g`/
//! `// Bufptr=`/the raw `// Buffer=` sample floats (`%.1f`, comma-separated,
//! wrapped every `2 + Fnconds*4` values — a **fixed** wrap width regardless of
//! the monitor's actual per-mode record size, an upstream quirk faithfully
//! reproduced, `Monitor.pas:1841`).
//!
//! `BufferSize` (Pascal: the preallocated `MonBuffer` capacity, default 1024,
//! doubled — actually flushed to `MonitorStream` and reset — whenever it
//! fills) has no Rust equivalent: the port keeps one growing, never-flushed
//! `Vec<f32>` (module doc). `BufPtr`/`Buffer` therefore read the **whole**
//! sample history (correct for any fixture under 1024 floats); `BufferSize`
//! itself is rendered as the constant `1024`, accurate as long as no gate
//! fixture crosses that threshold.
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
        let wrap = 2 + self.med.cd.nconds * 4;
        let mut k = 0usize;
        for &v in &self.mon_buffer {
            out.push_str(&format!("{v:.1}, "));
            k += 1;
            if k == wrap {
                out.push('\n');
                k = 0;
            }
        }
        out.push('\n');
    }
}
