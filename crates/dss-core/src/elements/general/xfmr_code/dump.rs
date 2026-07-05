//! Pascal `TXfmrCodeObj.DumpProperties` (`General/XfmrCode.pas:609`) — the
//! `Dump xfmrcode.…` override. A plain `TDSSObject` (no `! ENABLED`, no buses):
//! after the inherited `New "…"` header it hand-writes the same per-winding block
//! as the transformer (minus the bus, plus the capitalised `kV` label), the
//! `XHL…X23` reactances + flat `Xscmatrix`, the thermal/loss scalars, then the
//! generic property tail from `MaxTap` on. No `Complete` matrix block (unlike the
//! transformer).

use crate::report::format::{fixed, g};
use crate::report::save::dump::{self, DumpCtx};

use super::{XfmrCodeObj, prop};

impl XfmrCodeObj {
    pub(crate) fn dump_body(&self, out: &mut String, cx: &DumpCtx, _complete: bool) {
        dump::header(out, &dump::full_name(cx, self));

        let nw = self.num_windings.max(0) as usize;
        out.push_str(&format!("~ NumWindings={}\n", self.num_windings));
        out.push_str(&format!("~ phases={}\n", self.fnphases));

        for i in 1..=nw {
            let w = &self.windings[i - 1];
            out.push_str(&format!("~ Wdg={i}\n"));
            match w.connection {
                0 => out.push_str("~ conn=wye\n"),
                1 => out.push_str("~ conn=delta\n"),
                _ => {}
            }
            out.push_str(&format!("~ kV={}\n", fixed(w.kvll, 2)));
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

        // Pascal's two contiguous tail loops = props `MaxTap..Like`.
        dump::generic_props_from(out, cx, self, prop::MAXTAP);
    }
}
