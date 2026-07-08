//! Pascal `TAutoTransObj.SaveWrite` (`AutoTrans.pas:1054`) — the `Save`
//! serializer override. Same shape as the Transformer's (rewrite any
//! winding-scalar into the array properties `Buses`/`Conns`/`kVs`/`kVAs`/`Taps`/
//! `%Rs` plus a per-winding `Wdg=i` tail), but the auto has **no** `RNeut`/
//! `XNeut` (the comment at `AutoTrans.pas:1057` removes them), so neither the
//! trap set nor the per-winding tail carries them.

use crate::obj::base::DssObject;
use crate::report::format::g;
use crate::report::save::save::SaveCtx;
use crate::util::check_for_blanks;

use super::{AutoTrans, prop};

impl AutoTrans {
    /// Pascal `TAutoTransObj.SaveWrite` body (the caller already emitted
    /// `New "AutoTrans.name"`). Appends ` name=value` tokens to `out`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        use prop::*;

        let mut done = [false; NUM_PROPS + 1];
        // The winding-scalar props that trigger the array rewrite (Pascal's
        // `case iProp of RdcOhms,Bus,Conn,kV,kVA,Tap,MinTap,MaxTap,pctR,NumTaps,
        // Wdg` — no RNeut/XNeut).
        let is_trap = |ip: usize| {
            matches!(
                ip,
                RDCOHMS | BUS | CONN | KV | KVA | TAP | MINTAP | MAXTAP | PCTR | NUMTAPS | WDG
            )
        };
        let nw = self.num_windings.max(0) as usize;

        let mut iprop = self.data().next_property_set(None);
        while let Some(ip) = iprop {
            if is_trap(ip) {
                if !done[ip] {
                    // If Wdg= (or any winding scalar) was ever used, write the
                    // array forms once, in this exact order.
                    for ai in [BUSES, CONNS, KVS, KVAS, TAPS, PCTRS] {
                        if done[ai] {
                            continue;
                        }
                        let val = cx.cls.get_value(self, ai, cx.enums);
                        out.push_str(&format!(" {}={}", cx.cls.property_name(ai), val));
                        done[ai] = true;
                    }
                    // Per-winding `Wdg=i` + only the set per-winding scalars.
                    for i in 1..=nw {
                        let w = &self.windings[i - 1];
                        out.push_str(&format!(" Wdg={i}"));
                        if self.data().prp_specified(RDCOHMS) {
                            out.push_str(&format!(" RdcOhms={}", g(w.rdcohms, 15)));
                        }
                        if self.data().prp_specified(MINTAP) {
                            out.push_str(&format!(" MinTap={}", g(w.min_tap, 15)));
                        }
                        if self.data().prp_specified(MAXTAP) {
                            out.push_str(&format!(" MaxTap={}", g(w.max_tap, 15)));
                        }
                        if self.data().prp_specified(NUMTAPS) {
                            out.push_str(&format!(" NumTaps={}", w.num_taps));
                        }
                    }
                    for d in [
                        RDCOHMS, BUS, CONN, KV, KVA, TAP, MINTAP, MAXTAP, NUMTAPS, PCTR, WDG,
                    ] {
                        done[d] = true;
                    }
                }
            } else if !done[ip] {
                done[ip] = true;
                let val = cx.cls.get_value(self, ip, cx.enums);
                if !val.is_empty() {
                    out.push_str(&format!(
                        " {}={}",
                        cx.cls.property_name(ip),
                        check_for_blanks(&val)
                    ));
                }
            }
            iprop = self.data().next_property_set(Some(ip));
        }
    }
}
