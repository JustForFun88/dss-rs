//! Pascal `TReactorObj.DumpProperties` (`PDElements/Reactor.pas:966`) — the
//! `Dump reactor.…` override. After the inherited `TDSSCktElement` prefix
//! (`New "…"` + `! ENABLED` + the Complete Y/terminal block), it walks the
//! property list with a `case` that special-cases the matrices (skipped when
//! NIL, printed without the `~` prefix otherwise), the symmetrical-component /
//! series impedances (`[re, im]` at `%-.8g`), and `LmH` (`L·1000` at `%-.8g`);
//! every other property dumps generically via `GetPropertyValue`.

use super::Reactor;
use crate::report::format::g;
use crate::report::save::dump::{self, DumpCtx};

impl Reactor {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        dump::prefix_ckt(out, cx, self, &self.cd, complete);

        let n = self.cd.nphases;
        for k in 1..=cx.cls.num_properties() {
            let name = cx.cls.property_name(k);
            if name.eq_ignore_ascii_case("RMatrix") {
                if let Some(m) = &self.rmatrix {
                    write_matrix(out, name, m, n);
                }
            } else if name.eq_ignore_ascii_case("XMatrix") {
                if let Some(m) = &self.xmatrix {
                    write_matrix(out, name, m, n);
                }
            } else if name.eq_ignore_ascii_case("Z1") {
                out.push_str(&format!(
                    "~ Z1=[{}, {}]\n",
                    g(self.z1.re, 8),
                    g(self.z1.im, 8)
                ));
            } else if name.eq_ignore_ascii_case("Z2") {
                out.push_str(&format!(
                    "~ Z2=[{}, {}]\n",
                    g(self.z2.re, 8),
                    g(self.z2.im, 8)
                ));
            } else if name.eq_ignore_ascii_case("Z0") {
                out.push_str(&format!(
                    "~ Z0=[{}, {}]\n",
                    g(self.z0.re, 8),
                    g(self.z0.im, 8)
                ));
            } else if name.eq_ignore_ascii_case("Z") {
                // Pascal `Format('~ Z =[%-.8g, %-.8g]', …)` — the literal space
                // after `Z` is an upstream quirk (Z1/Z2/Z0 have none).
                out.push_str(&format!(
                    "~ Z =[{}, {}]\n",
                    g(self.z.re, 8),
                    g(self.z.im, 8)
                ));
            } else if name.eq_ignore_ascii_case("LmH") {
                out.push_str(&format!("~ LmH={}\n", g(self.l * 1000.0, 8)));
            } else {
                out.push_str("~ ");
                out.push_str(name);
                out.push('=');
                out.push_str(&cx.cls.get_value(self, k, cx.enums));
                out.push('\n');
            }
        }
    }
}

/// Pascal `<PropName>= (` + row-major `%-.5g ` cells, `|` between rows, ` )`.
/// The matrix is the full (symmetric) `nphases²` array.
fn write_matrix(out: &mut String, name: &str, m: &[f64], n: usize) {
    out.push_str(name);
    out.push_str("= (");
    for i in 0..n {
        if i > 0 {
            out.push('|');
        }
        for j in 0..n {
            out.push_str(&g(m[i * n + j], 5));
            out.push(' ');
        }
    }
    out.push_str(")\n");
}
