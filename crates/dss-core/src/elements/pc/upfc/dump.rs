//! Pascal `TUPFCObj.DumpProperties` (`PCElements/UPFC.pas:1028`) — the
//! `Dump upfc.…` override. Same shape as VSource but with **no** `VMag` line
//! (Pascal keeps that line commented out, `UPFC.pas:1044`). The `Z Matrix` is
//! the base-frequency series reactance `Z[i,i] = (0, Xs)` diagonal (off-
//! diagonal zero, `UPFC.pas:430-442` `RecalcElementData`); the port computes it
//! on the fly rather than materializing a persistent field, since nothing else
//! reads it.

use num_complex::Complex64;

use crate::report::format::{fixed, g};
use crate::report::save::dump::{self, DumpCtx};

use super::Upfc;

impl Upfc {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_pc(out, cx, self, &self.cd, complete);
        dump::generic_props(out, cx, self);

        if complete {
            out.push('\n');
            out.push_str(&format!(
                "BaseFrequency={}\n",
                fixed(self.cd.base_frequency, 1)
            ));
            out.push_str("Z Matrix=\n");
            let n = self.cd.nphases;
            for i in 0..n {
                for j in 0..=i {
                    let c = if i == j {
                        Complex64::new(0.0, self.xs)
                    } else {
                        Complex64::ZERO
                    };
                    out.push_str(&format!("{} +j {} ", g(c.re, 8), g(c.im, 8)));
                }
                out.push('\n');
            }
        }
    }
}
