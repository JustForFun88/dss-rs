//! Pascal `TFaultObj.DumpProperties` (`PDElements/Fault.pas:502`) — the
//! `Dump fault.…` override. After the inherited `TDSSCktElement` prefix: custom
//! `Bus1`/`Bus2`/`Phases`/`R`/`pctStdDev`/`Gmatrix`/`OnTime`/`Temporary`/
//! `MinAmps` lines, then the generic tail (`Fault.pas:533-536`), which this port
//! starts at `NormAmps`. Complete adds `// SpecType=%d`.
//!
//! **Why `NormAmps` and not `MinAmps`.** Upstream's tail loop is
//! `for i := NumPropsthisClass to ParentClass.NumProperties`, and this class's
//! `NumPropsThisClass = Ord(High(TProp))` = 9 = `MinAmps` itself
//! (`.inputs/dss_capi/src/PDElements/Fault.pas:533` with `:134`; r4133
//! `Version8/Source/PDElements/Fault.pas:594` with `Const NumPropsthisclass = 9`
//! `:107`), so the loop's first iteration re-emits the property the custom
//! `~ MinAmps=%.1f` line above just wrote — generically, giving the pair
//! `~ MinAmps=3.0` / `~ MinAmps=3` before the real tail. The off-by-one is a
//! slip and not a convention: the three other classes with the same tail loop
//! (`Transformer.pas:1276`, `AutoTrans.pas:1307`, `XfmrCode.pas:663`) all write
//! `NumPropsThisClass + 1`, and no class prints a property twice on purpose.
//! Both gating oracles carry the reprint and both lanes now drop it
//! (`GOLDEN_REBASE_PLAN.md` G2.2c; `issue-13`); `golden_reports`'
//! `fault_dump_expected` removes the second line of each pair from the oracle
//! goldens, so the `dump_fault`/`dump_fault_gmatrix`/`dump3_*` compares stay
//! oracle-anchored in both lanes.

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

        // Pascal `for i := NumPropsThisClass to NumProperties`, started at the
        // property *after* the last own one — `NormAmps` (10), not `MinAmps` (9),
        // which the custom line above already wrote (see the module header).
        dump::generic_props_from(out, cx, self, prop::NORMAMPS);

        if complete {
            out.push_str(&format!("// SpecType={}\n", self.spec_type));
        }
    }
}
