//! Pascal `TGICLineObj.DumpProperties` (`PCElements/GICLine.pas:627`) — the
//! `Dump gicline.…` override. After the inherited `TPCElement` prefix + the
//! generic `~ name=value` property loop, Complete adds a blank line then the
//! `%g`-formatted `BaseFrequency`/`Volts`/`VMag`/`VE`/`VN` and the base-frequency
//! series `Z Matrix` lower triangle (`%.8g +j %.8g `, one row per line).

use crate::report::format::g;
use crate::report::save::dump::{self, DumpCtx};

use super::GicLine;

impl GicLine {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_pc(out, cx, self, &self.cd, complete);
        dump::generic_props(out, cx, self);

        if complete {
            out.push('\n');
            out.push_str(&format!("BaseFrequency={}\n", g(self.cd.base_frequency, 1)));
            out.push_str(&format!("Volts={}\n", g(self.volts, 2)));
            out.push_str(&format!("VMag={}\n", g(self.vmag, 2)));
            out.push_str(&format!("VE={}\n", g(self.ve, 4)));
            out.push_str(&format!("VN={}\n", g(self.vn, 4)));
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
