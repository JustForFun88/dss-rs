//! Pascal `TLineCodeObj.DumpProperties` (`General/LineCode.pas:585`) — the
//! `Dump linecode.…` override. A plain `TDSSObject` (no `! ENABLED`): after the
//! inherited `New "…"` header it dumps `NPhases`, the sym-component scalars
//! (`R1`/`X1`/`R0`/`X0` `%.5f`, `C1`/`C0` `%.5f` of `C·1e9`), `Units`, the full
//! `RMatrix`/`XMatrix`/`CMatrix` (`%.8f`, row-major, `|`-separated, quoted),
//! props 12-21 generically, then `Neutral`, `Seasons` (= `NumAmpRatings`), the
//! bracketed `Ratings` list and `LineType`. Props `B1`/`B0` (23,24) are skipped,
//! mirroring the Pascal magic-number index list.

use crate::report::format::{fixed, g};
use crate::report::save::dump::{self, DumpCtx};

use super::{LineCodeObj, prop};

impl LineCodeObj {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, _complete: bool) {
        // inherited TDSSObject.DumpProperties (Leaf=FALSE): blank + `New "…"`.
        dump::header(out, &dump::full_name(cx, self));
        let name = |i| cx.cls.property_name(i);

        out.push_str(&format!("~ {}={}\n", name(prop::NPHASES), self.fnphases));
        out.push_str(&format!("~ {}={}\n", name(prop::R1), fixed(self.r1, 5)));
        out.push_str(&format!("~ {}={}\n", name(prop::X1), fixed(self.x1, 5)));
        out.push_str(&format!("~ {}={}\n", name(prop::R0), fixed(self.r0, 5)));
        out.push_str(&format!("~ {}={}\n", name(prop::X0), fixed(self.x0, 5)));
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::C1),
            fixed(self.c1 * 1.0e9, 5)
        ));
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::C0),
            fixed(self.c0 * 1.0e9, 5)
        ));
        // Units (prop 8): the generic string value (Pascal PropertyValue[8]).
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::UNITS),
            cx.cls.get_value(self, prop::UNITS, cx.enums)
        ));

        let np = self.fnphases.max(0) as usize;
        out.push_str(&format!("~ {}=\"", name(prop::RMATRIX)));
        if let Some(z) = &self.z {
            write_matrix(out, np, |i, j| fixed(z.get(i, j).re, 8));
        }
        out.push_str("\"\n");
        out.push_str(&format!("~ {}=\"", name(prop::XMATRIX)));
        if let Some(z) = &self.z {
            write_matrix(out, np, |i, j| fixed(z.get(i, j).im, 8));
        }
        out.push_str("\"\n");
        out.push_str(&format!("~ {}=\"", name(prop::CMATRIX)));
        if let Some(yc) = &self.yc {
            let denom = std::f64::consts::TAU * self.base_frequency;
            write_matrix(out, np, |i, j| fixed(yc.get(i, j).im / denom * 1.0e9, 8));
        }
        out.push_str("\"\n");

        // Pascal `for i := 12 to 21` (BaseFreq..rho — includes Kron).
        for i in prop::BASE_FREQ..=prop::RHO {
            dump::prop_line(out, cx, self, i);
        }

        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::NEUTRAL),
            self.fneutral_conductor
        ));
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::SEASONS),
            self.num_amp_ratings
        ));
        // Pascal `Ratings`: `[` + `floattoStrf(AmpRatings[k], ffGeneral, 8, 4),`
        // per entry (trailing comma) + `]`.
        let mut ratings = String::from("[");
        for k in 0..self.num_amp_ratings.max(0) as usize {
            ratings.push_str(&g(self.amp_ratings[k], 8));
            ratings.push(',');
        }
        ratings.push(']');
        out.push_str(&format!("~ {}={ratings}\n", name(prop::RATINGS)));
        out.push_str(&format!(
            "~ {}={}\n",
            name(prop::LINE_TYPE),
            cx.cls.get_value(self, prop::LINE_TYPE, cx.enums)
        ));
    }
}

/// Pascal full-matrix dump: row-major `<v> ` cells, each row trailed by `|`.
fn write_matrix(out: &mut String, n: usize, cell: impl Fn(usize, usize) -> String) {
    for i in 0..n {
        for j in 0..n {
            out.push_str(&cell(i, j));
            out.push(' ');
        }
        out.push('|');
    }
}
