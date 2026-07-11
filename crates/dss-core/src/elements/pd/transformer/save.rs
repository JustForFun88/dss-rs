//! Pascal `TTransfObj.SaveWrite` (`PDElements/Transformer.pas:1045-1125`) — the
//! `Save`-serializer override. The transformer's per-winding structure is not
//! conducive to the generic "one `name=value` per set property" form, so the
//! override rewrites any winding-scalar (`Wdg`/`Bus`/`Conn`/`kV`/`kVA`/`Tap`/…)
//! that was ever set into the **array** properties
//! (`Buses`/`Conns`/`kVs`/`kVAs`/`Taps`/`%Rs`) plus a per-winding `Wdg=i` tail
//! carrying only the rarely-set per-winding scalars
//! (`RdcOhms`/`RNeut`/`XNeut`/`MinTap`/`MaxTap`/`NumTaps`). Every other set
//! property is written by the generic rule.
//!
//! Co-located with the element (like [`super::dump`]) so it reads the winding
//! fields directly, exactly as the Pascal method does; dispatched from
//! [`crate::report::save::save::write_dss_object`] via downcast.

use crate::obj::base::DssObject;
use crate::report::format::g;
use crate::report::save::save::SaveCtx;
use crate::util::check_for_blanks;

use super::{Transformer, prop};

impl Transformer {
    /// Pascal `TTransfObj.SaveWrite` (the `SaveWrite` body only — the caller
    /// already emitted `New "Transformer.name"`). Appends ` name=value` tokens
    /// to `out`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        use prop::*;

        // Pascal `done: Set of TProp`.
        let mut done = [false; NUM_PROPS + 1];
        // The winding-scalar props that trigger the array rewrite (Pascal's
        // `case iProp of RdcOhms,Bus,Conn,kV,kVA,Tap,MinTap,MaxTap,pctR,RNeut,
        // Xneut,NumTaps,Wdg`).
        let is_trap = |ip: usize| {
            matches!(
                ip,
                RDCOHMS
                    | BUS
                    | CONN
                    | KV
                    | KVA
                    | TAP
                    | MINTAP
                    | MAXTAP
                    | PCTR
                    | RNEUT
                    | XNEUT
                    | NUMTAPS
                    | WDG
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
                        if self.data().prp_specified(RNEUT) {
                            out.push_str(&format!(" RNeut={}", g(w.rneut, 15)));
                        }
                        if self.data().prp_specified(XNEUT) {
                            out.push_str(&format!(" XNeut={}", g(w.xneut, 15)));
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
                    // Mark every trap prop done (Pascal's `Include(done, …)` set).
                    for d in [
                        RDCOHMS, BUS, CONN, KV, KVA, TAP, MINTAP, MAXTAP, NUMTAPS, PCTR, RNEUT,
                        XNEUT, WDG,
                    ] {
                        done[d] = true;
                    }
                }
            } else if !done[ip] {
                done[ip] = true;
                // Pascal: `if Length(PropertyValue[iProp]) > 0 then write
                // name=CheckForBlanks(value)` — no `----`/trim handling here
                // (that is the *generic* SaveWrite's job).
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
