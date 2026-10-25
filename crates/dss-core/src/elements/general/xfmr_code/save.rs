//! Pascal `TXfmrCodeObj.SaveWrite` (dss_capi 0.14.5
//! `General/XfmrCode.pas:667-745`) — the `Save`-serializer override, *"Like
//! Transformer's, XfmrCode structure not conducive to standard means of saving.
//! Same as Transformer's SaveWrite, removing buses"*.
//!
//! Without it the emitted line keeps whatever winding-scalar tokens the deck
//! typed (`Wdg=`/`Conn=`/`kV=`/`kVA=`/`%R=`/`Tap=`), each rendered by a getter
//! that answers the **active** winding only — so a 3-winding code saves as
//! `… Wdg=3 Conn=wye kV=4.16 kVA=5000 %R=0.7 Tap=0.975` and re-compiles into a
//! *different* code with windings 1 and 2 at their defaults. r4133 has the same
//! defect (measured on `OpenDSSDirect.dll` 11.0.0.1 rev r4133, RP3.11
//! settlement: `New "XfmrCode.xc" phases=3 windings=3 Xhl=7 Xht=9 Xlt=8 wdg=3
//! conn=wye kV=4.16 kVA=5000 %R=0.7 tap=0.975`), and both re-compiles converge,
//! so the wrong circuit is silent; 0.14.5 fixed it with this override and
//! CLAUDE.md's 2026-08-02 policy forbids reproducing the r4133 side.
//!
//! The rewrite is the [`Transformer`](crate::elements::pd::transformer) one
//! minus `Buses`/`Bus`: any winding scalar that was ever set emits the **array**
//! properties (`Conns`/`kVs`/`kVAs`/`Taps`/`%Rs`) once, followed by a per-winding
//! `Wdg=i` tail carrying only the rarely-set per-winding scalars
//! (`RdcOhms`/`RNeut`/`XNeut`/`MinTap`/`MaxTap`/`NumTaps`).

use crate::obj::base::DssObject;
use crate::report::format::g;
use crate::report::save::save::SaveCtx;
use crate::util::check_for_blanks;

use super::{XfmrCodeObj, prop};

impl XfmrCodeObj {
    /// The tokens of the code's `Save` line after `New "XfmrCode.name"`,
    /// which the caller writes, appended to `out` as ` name=value`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        use prop::*;

        // The properties already written.
        let mut done = [false; NUM_PROPS + 1];
        // The winding scalars, the Transformer's set without `Bus`.
        let is_trap = |ip: usize| {
            matches!(
                ip,
                RDCOHMS
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
        let nw = self.num_windings().max(0) as usize;

        let mut iprop = self.data().next_property_set(None);
        while let Some(ip) = iprop {
            if is_trap(ip) {
                if !done[ip] {
                    // If a winding scalar was ever used, write the array forms
                    // once, in this exact order.
                    for ai in [CONNS, KVS, KVAS, TAPS, PCTRS] {
                        if done[ai] {
                            continue;
                        }
                        let val = cx.cls.saved_value(self, ai, cx.enums);
                        out.push_str(&format!(" {}={}", cx.cls.property_name(ai), val));
                        done[ai] = true;
                    }
                    // Per-winding `Wdg=i` + only the set per-winding scalars,
                    // the printed ordinal derived from the slice position.
                    for (iw, w) in self.windings().iter().take(nw).enumerate() {
                        let i = iw + 1;
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
                    // Every winding scalar is written now.
                    for d in [
                        RDCOHMS, CONN, KV, KVA, TAP, MINTAP, MAXTAP, NUMTAPS, PCTR, RNEUT, XNEUT,
                        WDG,
                    ] {
                        done[d] = true;
                    }
                }
            } else if !done[ip] {
                done[ip] = true;
                // A value that is not empty, quoted when it holds a blank,
                // with no `----` or trim handling (the generic token's job).
                let val = cx.cls.saved_value(self, ip, cx.enums);
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
