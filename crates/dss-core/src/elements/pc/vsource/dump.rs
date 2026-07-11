//! Pascal `TVsourceObj.DumpProperties` (`PCElements/VSource.pas:1137`) — the
//! `Dump vsource.…` override. After the inherited `TPCElement` prefix, all
//! properties dump generically (`~ name=value`); Complete adds a blank line,
//! `BaseFrequency=%.1f`, `VMag=%.2f`, then the base-frequency series `Z Matrix`
//! lower triangle (`%.8g +j %.8g `, one row per line, no leading `~` — these
//! are Complete-only informational lines, not properties).

use crate::report::format::{fixed, g};
use crate::report::save::dump::{self, DumpCtx};

use super::VSource;

impl VSource {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_pc(out, cx, self, &self.cd, complete);
        dump::generic_props(out, cx, self);

        if complete {
            out.push('\n');
            out.push_str(&format!(
                "BaseFrequency={}\n",
                fixed(self.cd.base_frequency, 1)
            ));
            out.push_str(&format!("VMag={}\n", fixed(self.vmag, 2)));
            out.push_str("Z Matrix=\n");
            let n = self.cd.nphases;
            if let Some(z) = &self.z {
                for i in 0..n {
                    for j in 0..=i {
                        let c = z.get(i, j);
                        out.push_str(&format!("{} +j {} ", g(c.re, 8), g(c.im, 8)));
                    }
                    out.push('\n');
                }
            }
        }
    }
}
