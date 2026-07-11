//! Pascal `TCapacitorObj.DumpProperties` (`PDElements/Capacitor.pas:751`) — the
//! `Dump capacitor.…` override. After the inherited `TDSSCktElement` prefix, all
//! properties dump generically (`~ name=value`); Complete adds a bare
//! `SpecType=<int>` line (no `~`/`//` prefix — an upstream quirk, faithfully
//! reproduced).
//!
//! **Upstream garbage** (probe-proven, `tools/golden/report_decks/README.md`):
//! this pinned build prints ASLR-dependent uninitialized memory in
//! `~ CMatrix=(`/`~ FaultRate=`/`~ pctPerm=` for EVERY capacitor — not
//! reproduced (nondeterministic UB, CLAUDE.md known-bug rule); the
//! `dump_capacitor` golden masks those three line prefixes on both sides.

use crate::report::save::dump::{self, DumpCtx};

use super::Capacitor;

impl Capacitor {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_ckt(out, cx, self, &self.cd, complete);
        dump::generic_props(out, cx, self);
        if complete {
            out.push_str(&format!("SpecType={}\n", self.spec_type));
        }
    }
}
