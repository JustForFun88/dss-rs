//! Pascal `TEnergyMeterObj.DumpProperties` (`Meters/EnergyMeter.pas:2082`) —
//! the `Dump energymeter.…` override. After the inherited `TDSSCktElement`
//! prefix, all properties dump generically (`~ name=value`); Complete adds the
//! `Registers` heading (`"<name>" = %.0g` per register) then `Branch List:` —
//! the zone-tree walk built by [`dump::energy_meter_branch_list`] (needs the
//! full class registry the dispatcher precomputes it with, since a per-object
//! `DumpCtx` alone can't resolve another class's object names).

use crate::report::format::g;
use crate::report::save::dump::{self, DumpCtx};

use super::EnergyMeter;

impl EnergyMeter {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_ckt(out, cx, self, &self.med.cd, complete);
        dump::generic_props(out, cx, self);

        if !complete {
            return;
        }

        out.push_str("Registers\n");
        let names = self.register_names();
        let regs = self.registers();
        for i in 0..names.len().min(regs.len()) {
            out.push_str(&format!("\"{}\" = {}\n", names[i], g(regs[i], 0)));
        }
        out.push('\n');

        out.push_str("Branch List:\n");
        out.push_str(cx.branch_list);
    }
}
