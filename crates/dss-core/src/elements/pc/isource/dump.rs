//! `Dump isource.…` — Isource has **no** Pascal `DumpProperties` override
//! (unlike VSource), so the oracle's dump goes straight to the ancestor
//! `TPCElement.DumpProperties` (`PCElement.pas:219`): `! ENABLED` + (Complete)
//! the CktElement Y-block + `! VARIABLES` + generic props + (Complete) two
//! blank lines.
//!
//! The shared [`crate::report::save::dump::dump_generic`] fallback picks this
//! ordering from whether the element is in `Circuit.pc_elements` — but Isource
//! is `NON_PCPD_ELEM` (Pascal `SOURCE or NON_PCPD_ELEM`, same as VSource), so
//! it lives in `Circuit.sources` instead and would otherwise fall through to
//! the *non*-PC generic branch. VSource sidesteps this because it has its own
//! override; Isource does not, so this thin `dump_body` reproduces the
//! `TPCElement` ordering directly and is dispatched from
//! `report/save/dump/overrides.rs` exactly like a real Pascal override would
//! be (there just happen to be no extra Complete-only lines to add, since
//! `TIsourceObj` adds none beyond what `TPCElement.DumpProperties` already
//! writes).

use crate::report::save::dump::{self, DumpCtx};

use super::Isource;

impl Isource {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_pc(out, cx, self, &self.cd, complete);
        dump::generic_props(out, cx, self);
        if complete {
            out.push('\n');
            out.push('\n');
        }
    }
}
