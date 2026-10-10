//! The `Dump linegeometry.…` override. A plain DSS object, but it walks the
//! per-conductor properties by selecting each conductor in turn (hence
//! `&mut self`): it emits a literal `! WARNING` line **before** the inherited
//! `New "…"` header, then props 1-2 once (`NConds`/`NPhases`), then props 3-7
//! (`Cond`/`Wire`/`X`/`H`/`Units`) once **per conductor**, then props 8..end
//! generically, and finally puts the selection back where it was.

use crate::report::save::dump::{self, DumpCtx};

use super::{LineGeometryObj, prop};

impl LineGeometryObj {
    pub(crate) fn dump_body(&mut self, out: &mut String, cx: &DumpCtx, _complete: bool) {
        out.push_str(
            "! WARNING: when mixing wire/cable types, \"wires\", \"cncables\" and \
             \"tscables\" may not make sense in this dump\n",
        );
        // The inherited object dump: blank + `New "…"`.
        dump::header(out, &dump::full_name(cx, &*self));

        dump::prop_line(out, cx, &*self, prop::NCONDS);
        dump::prop_line(out, cx, &*self, prop::NPHASES);

        // Per-conductor block: select conductor `j`, then props 3-7. The
        // generic tail below reads the last conductor, as the walk leaves it.
        let selection = self.selected;
        for j in 0..self.fnconds.max(0) as usize {
            self.selected = Some(j);
            for p in [prop::COND, prop::WIRE, prop::X, prop::H, prop::UNITS] {
                dump::prop_line(out, cx, &*self, p);
            }
        }

        dump::generic_props_from(out, cx, &*self, 8);
        self.selected = selection;
    }
}
