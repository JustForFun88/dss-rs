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

    /// Pascal `TEnergyMeterObj.SaveRegisters` (`Meters/EnergyMeter.pas:
    /// 1235-1269`), the content side: header `Year, <year>,` then one line per
    /// register — `"<RegName>",<value :0:0>` (FPC fixed-point, 0 decimals → the
    /// rounded integer). The caller (`Dss::save_meters_cmd`) writes the text to
    /// `<OutputDir>MTR_<name>.csv` and sets `GlobalResult` to the RELATIVE CSV
    /// name (probe-proven 2026-07-07).
    pub(crate) fn save_registers_text(&self, year: i32) -> String {
        let mut s = format!("Year, {year},\n");
        let names = self.register_names();
        let regs = self.registers();
        // Pascal `for i := 1 to NumEMregisters`: `RegisterNames[i-1]` /
        // `Registers[i]` (a 1-based register array) — 0-based and parallel
        // here. Direct indexing on purpose: Pascal always writes
        // NumEMregisters lines, so a names/registers length mismatch is a
        // port bug that must panic, not silently truncate.
        for (i, name) in names.iter().enumerate() {
            // FPC `Str(x:0:0)` (fixed-point, 0 decimals). Hedge: its rounding
            // at an exact .5 tie is unverified against Rust's `{:.0}`
            // half-to-even — probe-confirmed 2026-07-07 only at non-tie
            // values; the byte-exact golden covers the pinned values only.
            s.push_str(&format!("\"{}\",{:.0}\n", name, regs[i]));
        }
        s
    }
}
