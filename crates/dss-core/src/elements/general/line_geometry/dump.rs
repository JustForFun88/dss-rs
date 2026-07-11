//! Pascal `TLineGeometryObj.DumpProperties` (`General/LineGeometry.pas:669`) —
//! the `Dump linegeometry.…` override. A plain `TDSSObject`, but it walks the
//! per-conductor properties by mutating `ActiveCond` (hence `&mut self`): it
//! emits a literal `! WARNING` line **before** the inherited `New "…"` header,
//! then props 1-2 once (`NConds`/`NPhases`), then props 3-7 (`Cond`/`Wire`/`X`/
//! `H`/`Units`) once **per conductor** (`ActiveCond := j`), then props 8..end
//! generically.

use crate::report::save::dump::{self, DumpCtx};

use super::{LineGeometryObj, prop};

impl LineGeometryObj {
    pub(crate) fn dump_body(&mut self, out: &mut String, cx: &DumpCtx, _complete: bool) {
        out.push_str(
            "! WARNING: when mixing wire/cable types, \"wires\", \"cncables\" and \
             \"tscables\" may not make sense in this dump\n",
        );
        // inherited TDSSObject.DumpProperties (Leaf=FALSE): blank + `New "…"`.
        dump::header(out, &dump::full_name(cx, &*self));

        dump::prop_line(out, cx, &*self, prop::NCONDS);
        dump::prop_line(out, cx, &*self, prop::NPHASES);

        // Per-conductor block: Pascal `ActiveCond := j; GetPropertyValue(3..7)`.
        for j in 1..=self.fnconds.max(0) {
            self.factive_cond = j;
            for p in [prop::COND, prop::WIRE, prop::X, prop::H, prop::UNITS] {
                dump::prop_line(out, cx, &*self, p);
            }
        }

        dump::generic_props_from(out, cx, &*self, 8);
    }
}
