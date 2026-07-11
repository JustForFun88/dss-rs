//! Pascal `TTransfObj.DumpProperties` (`PDElements/Transformer.pas:1219`) — the
//! `Dump transformer.…` override. After the inherited `TDSSCktElement` prefix
//! (`New "…"` + `! ENABLED` + the Complete Y/terminal block) it hand-writes a
//! human-readable per-winding block (`NumWindings`/`phases`, then per winding
//! `Wdg`/`bus`/`conn`/`kv`/`kVA`/`tap`/`%R`/`RdcOhms`/`rneut`/`xneut`), the
//! `XHL…X23` reactances + the flat `Xscmatrix`, the thermal/loss scalars, then
//! the generic property tail from `NormHkVA` on. The `Complete` (`debug`) branch
//! appends the `ZB` / `ZB (inverted)` / `Y_OneVolt` / `Y_Terminal` complex
//! lower-triangle dumps and the `TermRef` map.

use crate::report::format::{fixed, g};
use crate::report::save::dump::{self, DumpCtx};
use crate::support::cmatrix::CMatrix;

use super::{Transformer, prop};

impl Transformer {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, complete: bool) {
        // inherited TDSSCktElement.DumpProperties (Leaf=FALSE).
        dump::prefix_ckt(out, cx, self, &self.cd, complete);

        let nw = self.num_windings.max(0) as usize;
        out.push_str(&format!("~ NumWindings={}\n", self.num_windings));
        out.push_str(&format!("~ phases={}\n", self.cd.nphases));

        for i in 1..=nw {
            let w = &self.windings[i - 1];
            // Pascal winding 1 → `firstbus`, others → `nextbus` (both = terminal
            // i's bus name).
            out.push_str(&format!("~ Wdg={i} bus={}\n", self.cd.get_bus(i)));
            match w.connection {
                0 => out.push_str("~ conn=wye\n"),
                1 => out.push_str("~ conn=delta\n"),
                _ => {}
            }
            out.push_str(&format!("~ kv={}\n", fixed(w.kvll, 2)));
            out.push_str(&format!("~ kVA={}\n", fixed(w.kva, 1)));
            out.push_str(&format!("~ tap={}\n", fixed(w.putap, 3)));
            out.push_str(&format!("~ %R={}\n", fixed(w.rpu * 100.0, 2)));
            out.push_str(&format!("~ RdcOhms={}\n", g(w.rdcohms, 7)));
            out.push_str(&format!("~ rneut={}\n", fixed(w.rneut, 3)));
            out.push_str(&format!("~ xneut={}\n", fixed(w.xneut, 3)));
        }

        out.push_str(&format!("~ XHL={}\n", fixed(self.xhl * 100.0, 3)));
        out.push_str(&format!("~ XHT={}\n", fixed(self.xht * 100.0, 3)));
        out.push_str(&format!("~ XLT={}\n", fixed(self.xlt * 100.0, 3)));
        out.push_str(&format!("~ X12={}\n", fixed(self.xhl * 100.0, 3)));
        out.push_str(&format!("~ X13={}\n", fixed(self.xht * 100.0, 3)));
        out.push_str(&format!("~ X23={}\n", fixed(self.xlt * 100.0, 3)));
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
            // in place); Pascal copies + re-inverts it to recover the original
            // series impedance for the `ZB:` block, then dumps the stored inverse
            // for `ZB: (inverted)`.
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
