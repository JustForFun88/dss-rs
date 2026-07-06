//! Pascal `TRegControlObj.DumpProperties` (`Controls/RegControl.pas:682`) — the
//! `Dump regcontrol.…` override. After the inherited `TDSSCktElement` prefix,
//! all properties dump generically (`~ name=value`); Complete adds
//! `! Bus =<GetBus(1)>` then a blank line.

use crate::report::save::dump::{self, DumpCtx};

use super::RegControl;

impl RegControl {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_ckt(out, cx, self, &self.ccd.cd, complete);
        dump::generic_props(out, cx, self);

        if complete {
            out.push_str(&format!("! Bus ={}\n", self.ccd.cd.get_bus(1)));
            out.push('\n');
        }
    }
}
