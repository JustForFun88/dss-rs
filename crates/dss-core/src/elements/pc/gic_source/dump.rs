//! `Dump gicsource.…` — GICsource has **no** Pascal `DumpProperties` override,
//! so the oracle's dump goes to the ancestor `TPCElement.DumpProperties`
//! (`PCElement.pas:219`): `! ENABLED` + (Complete) the CktElement Y-block +
//! `! VARIABLES` + generic props + (Complete) two blank lines.
//!
//! Like Isource, GICsource is `NON_PCPD_ELEM` (Pascal `SOURCE or NON_PCPD_ELEM`)
//! so it lives on `Circuit.sources`, not `Circuit.pc_elements`; the generic
//! fallback keys its PC-vs-not ordering on `pc_elements` membership, so this
//! thin `dump_body` reproduces the `TPCElement` ordering directly and is
//! dispatched from `report/save/dump/overrides.rs` (see `isource/dump.rs`).

use crate::report::save::dump::{self, DumpCtx};

use super::GicSource;

impl GicSource {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_pc(out, cx, self, &self.cd, complete);
        dump::generic_props(out, cx, self);
        if complete {
            out.push('\n');
            out.push('\n');
        }
    }
}
