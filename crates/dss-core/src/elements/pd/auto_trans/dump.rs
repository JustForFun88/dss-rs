//! Pascal `TAutoTransObj.DumpProperties` (`AutoTrans.pas:1246`) — the `Dump
//! autotrans.…` override. After the inherited `TDSSCktElement` prefix it
//! hand-writes the per-winding block (`Wdg`/`bus`/`conn`/`kv`/`kVA`/`tap`/`%r`/
//! `Rdcohms` — **no** `rneut`/`xneut`, unlike Transformer), the `XHX`/`XHT`/
//! `XXT` reactances (no `X12`/`X13`/`X23`) + the flat `Xscmatrix`, the
//! thermal/loss scalars, then the generic property tail from `NormHkVA` on. The
//! `Complete` branch appends the `ZB`/`ZB (inverted)`/`Y_OneVolt`/`Y_Terminal`
//! lower-triangle dumps and the `TermRef` map. The winding scalars render with
//! `%.7g` (Transformer uses fixed widths).

use crate::report::format::{fixed, g};
use crate::report::save::dump::{self, DumpCtx};
use crate::support::cmatrix::CMatrix;

use super::{AutoTrans, prop};

impl AutoTrans {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        // inherited TDSSCktElement.DumpProperties (Leaf=FALSE).
        dump::prefix_ckt(out, cx, self, &self.cd, complete);

        let nw = self.num_windings.max(0) as usize;
        out.push_str(&format!("~ NumWindings={}\n", self.num_windings));
        out.push_str(&format!("~ phases={}\n", self.cd.nphases));

        for i in 1..=nw {
            let w = &self.windings[i - 1];
            out.push_str(&format!("~ Wdg={i} bus={}\n", self.cd.get_bus(i)));
            match w.connection {
                0 => out.push_str("~ conn=wye\n"),
                1 => out.push_str("~ conn=delta\n"),
                2 => out.push_str("~ conn=Series\n"),
                _ => {}
            }
            out.push_str(&format!("~ kv={}\n", g(w.kvll, 7)));
            out.push_str(&format!("~ kVA={}\n", g(w.kva, 7)));
            out.push_str(&format!("~ tap={}\n", g(w.putap, 7)));
            out.push_str(&format!("~ %r={}\n", g(w.rpu * 100.0, 7)));
            out.push_str(&format!("~ Rdcohms={}\n", g(w.rdcohms, 7)));
        }

        out.push_str(&format!("~ XHX={}\n", fixed(self.puxhx * 100.0, 3)));
        out.push_str(&format!("~ XHT={}\n", fixed(self.puxht * 100.0, 3)));
        out.push_str(&format!("~ XXT={}\n", fixed(self.puxxt * 100.0, 3)));
        out.push_str("~ Xscmatrix= \"");
        let nsc = if nw >= 1 { (nw - 1) * nw / 2 } else { 0 };
        for i in 0..nsc {
            out.push_str(&fixed(self.xsc[i] * 100.0, 2));
            out.push(' ');
        }
        out.push_str("\"\n");
        out.push_str(&format!("~ NormMAxHkVA={}\n", fixed(self.norm_max_hkva, 0)));
        out.push_str(&format!(
            "~ EmergMAxHkVA={}\n",
            fixed(self.emerg_max_hkva, 0)
        ));
        out.push_str(&format!(
            "~ thermal={}\n",
            fixed(self.thermal_time_const, 1)
        ));
        out.push_str(&format!("~ n={}\n", fixed(self.n_thermal, 1)));
        out.push_str(&format!("~ m={}\n", fixed(self.m_thermal, 1)));
        out.push_str(&format!("~ flrise={}\n", fixed(self.flrise, 0)));
        out.push_str(&format!("~ hsrise={}\n", fixed(self.hsrise, 0)));
        out.push_str(&format!("~ %loadloss={}\n", fixed(self.pct_load_loss, 0)));
        out.push_str(&format!(
            "~ %noloadloss={}\n",
            fixed(self.pct_no_load_loss, 0)
        ));

        // Pascal's two contiguous tail loops (`28..NumPropsThisClass` +
        // `NumPropsThisClass+1..NumProperties`) = props `NormHkVA..Like`.
        dump::generic_props_from(out, cx, self, prop::NORMHKVA);

        if complete {
            out.push('\n');
            // The stored `ZB` field is already inverted (CalcY_Terminal inverts
            // in place); Pascal copies + re-inverts it for the `ZB:` block, then
            // dumps the stored inverse for `ZB: (inverted)`.
            let mut zb_orig = self.zb.clone();
            let _ = zb_orig.invert();
            out.push_str("ZB:\n");
            write_lower_tri(out, &zb_orig, nw.saturating_sub(1), |v| g(v, 15));
            out.push('\n');
            out.push_str("ZB: (inverted)\n");
            write_lower_tri(out, &self.zb, nw.saturating_sub(1), |v| fixed(v, 4));
            out.push('\n');
            out.push_str("Y_OneVolt\n");
            write_lower_tri(out, &self.y_1volt, nw, |v| fixed(v, 4));
            out.push('\n');
            out.push_str("Y_Terminal\n");
            write_lower_tri(out, &self.y_term, 2 * nw, |v| fixed(v, 4));
            out.push('\n');
            out.push_str("TermRef= ");
            let n = 2 * nw * self.cd.nphases;
            for i in 1..=n {
                out.push_str(&self.term_ref[i].to_string());
                out.push(' ');
            }
            out.push('\n');
        }
    }
}

/// Pascal complex lower-triangle dump: `for i, for j := 1 to i: FSWrite('<v> ')`
/// then a newline, the real parts of all rows first, then the imaginary parts.
fn write_lower_tri(out: &mut String, m: &CMatrix, order: usize, fmt: impl Fn(f64) -> String) {
    for i in 0..order {
        for j in 0..=i {
            out.push_str(&fmt(m.get(i, j).re));
            out.push(' ');
        }
        out.push('\n');
    }
    for i in 0..order {
        for j in 0..=i {
            out.push_str(&fmt(m.get(i, j).im));
            out.push(' ');
        }
        out.push('\n');
    }
}
