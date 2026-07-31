//! Pascal `TFaultObj.DumpProperties` (`PDElements/Fault.pas:502`) — the
//! `Dump fault.…` override. After the inherited `TDSSCktElement` prefix: custom
//! `Bus1`/`Bus2`/`Phases`/`R`/`pctStdDev`/`Gmatrix`/`OnTime`/`Temporary`/
//! `MinAmps` lines, then the generic tail from `NumPropsThisClass` (`Fault.pas:
//! 533-536`) — an upstream quirk: `NumPropsThisClass = Ord(High(TProp)) = 9 =
//! MinAmps`, so the tail loop **reprints `MinAmps` a second time**, generically,
//! right after its custom `%.1f` line above, before continuing into the real
//! tail (`NormAmps..Enabled`). Complete adds `// SpecType=%d`.
//!
//! The `MinAmps` double-print is the Stage F row
//! [`crate::compat::FAULT_DUMP_TAIL_REPRINTS_MINAMPS`]: a deterministic upstream
//! off-by-one that the three other classes with the same tail loop
//! (`Transformer.pas:1276`, `AutoTrans.pas:1307`, `XfmrCode.pas:663`) do not
//! have — they start at `NumPropsThisClass + 1`. The parity lane reproduces it
//! (the oracle genuinely double-prints, pinned byte-exact by
//! `dump_fault`/`dump_fault_gmatrix`); the default lane starts the tail at the
//! next property.

use crate::compat;
use crate::report::format::fixed;
use crate::report::save::dump::{self, DumpCtx};

use super::{Fault, prop};

impl Fault {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_ckt(out, cx, self, &self.cd, complete);
        let name = |i| cx.cls.property_name(i);

        out.push_str(&format!("~ {}={}\n", name(prop::BUS1), self.cd.get_bus(1)));
        out.push_str(&format!("~ {}={}\n", name(prop::BUS2), self.cd.get_bus(2)));
        out.push_str(&format!("~ {}={}\n", name(prop::PHASES), self.cd.nphases));
        out.push_str(&format!("~ {}={}\n", name(prop::R), fixed(1.0 / self.g, 2)));
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::PCTSTDDEV),
            fixed(self.stddev * 100.0, 1)
        ));
        if let Some(gm) = &self.gmatrix {
            let n = self.cd.nphases;
            out.push_str(&format!("~ {}= (", name(prop::GMATRIX)));
            for i in 1..=n {
                for j in 1..=i {
                    out.push_str(&format!("{} ", fixed(gm[(i - 1) * n + (j - 1)], 3)));
                }
                if i != n {
                    out.push('|');
                }
            }
            out.push_str(")\n");
        }
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::ONTIME),
            fixed(self.on_time, 3)
        ));
        out.push_str(&format!(
            "~ {}= {}\n",
            name(prop::TEMPORARY),
            if self.is_temporary { "Yes" } else { "No" }
        ));
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::MINAMPS),
            fixed(self.min_amps, 1)
        ));

        // Pascal `for i := NumPropsThisClass to NumProperties` — starts AT
        // MinAmps (9) in the parity lane (the reprint quirk documented above)
        // and at its successor, NormAmps (10), in the default lane.
        let tail_start = if compat::FAULT_DUMP_TAIL_REPRINTS_MINAMPS {
            prop::MINAMPS
        } else {
            prop::NORMAMPS
        };
        dump::generic_props_from(out, cx, self, tail_start);

        if complete {
            out.push_str(&format!("// SpecType={}\n", self.spec_type));
        }
    }
}
