//! Pascal `TLineGeometryObj.SaveWrite` (`General/LineGeometry.pas:792-839`,
//! vendored dss_capi 0.14.5 — the pinned save oracle) — the `Save`-serializer
//! override. The per-conductor structure is not conducive to the generic "one
//! `name=value` per set property" form (the generic serializer would emit only
//! the LAST conductor, since `Cond`/`Wire`/`X`/`H`/`Units` each carry a single
//! property-sequence slot re-set once per conductor). The override instead
//! rewrites the whole conductor table (`Cond=i wire=.. X=.. h=.. units=..`,
//! once, when `cond=`/`spacing=`/`wires=`/`conductors=` was ever set), skipping
//! the scalar per-conductor props, and lets every other set property fall
//! through to the generic `name=value` rule.
//!
//! Co-located with the element (like [`super::dump`]) so it reads the conductor
//! fields directly, exactly as the Pascal method does; dispatched from
//! [`crate::report::save::save::write_dss_object`] via a typed arena read. Emitted inline
//! on the single `New "…"` line (like the [`crate::elements::pd::transformer`]
//! override) rather than as the oracle's `~ Cond=…` continuation lines — the two
//! are token-equivalent on re-parse and the `Save` contract is round-trip
//! fidelity, not byte-equality (`report/save/save.rs` header).

use crate::elements::general::conductor_data::{ConductorData, ConductorKind};
use crate::obj::base::DssObject;
use crate::report::format::g;
use crate::report::save::save::SaveCtx;
use crate::support::line_units::LineUnits;
use crate::util::check_for_blanks;

use super::{LineGeometryObj, prop};

impl LineGeometryObj {
    /// Pascal `TLineGeometryObj.SaveWrite` body (the caller already emitted
    /// `New "LineGeometry.name"`). Appends ` name=value` tokens to `out`.
    /// `NConds=` comes first whenever the geometry has conductors: it
    /// re-allocates every conductor, so a conductor table ahead of it would
    /// reload into nothing. A geometry with none writes no `NConds=`, whose
    /// only value would be the 0 a reload refuses. Pinned by
    /// `exec::tests::line_geometry_rules::save_writes_nconds_first` and
    /// `::a_refused_nconds_is_not_saved`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        use prop::*;

        let generic = |out: &mut String, ip: usize| {
            let val = cx.cls.get_value(self, ip, cx.enums);
            let s = val.trim();
            if !s.is_empty() {
                out.push_str(&format!(
                    " {}={}",
                    cx.cls.property_name(ip),
                    check_for_blanks(s)
                ));
            }
        };
        if self.fnconds > 0 {
            generic(out, NCONDS);
        }

        // The conductor table is written at most once, at the first of
        // `cond`/`spacing`/`wires`/`conductors` in the set-order walk. Its rows
        // name each conductor's class, so `Conductors=` is never written on its
        // own (its items would reload without their class).
        let mut wrote_conds = false;
        let mut iprop = self.data().next_property_set(None);
        while let Some(ip) = iprop {
            match ip {
                NCONDS => {}
                COND | SPACING | WIRES | CONDUCTORS => {
                    if !wrote_conds {
                        self.write_conductor_block(out);
                        wrote_conds = true;
                    }
                }
                // Ignore these — subsumed by the conductor block (Pascal `;`).
                // The plural `CNCables`/`TSCables` array forms are flagged
                // `Redundant` with the singular `cncable`/`tscable` in Pascal
                // (`LineGeometry.pas:279-287`), so Pascal's `GetNextPropertySet`
                // yields the singular (ignored) and never re-emits the array. Our
                // set-order records the plural, so ignore it here too — otherwise
                // the generic `_` arm re-emits `CNCables=[…]` *after* the conductor
                // block, and the reload aborts (`Unexpected number of objects`).
                WIRE | X | H | UNITS | CNCABLE | TSCABLE | CNCABLES | TSCABLES => {}
                REDUCE => {
                    if self.freduce {
                        out.push_str(" Reduce=Yes");
                    }
                }
                // Pascal ELSE: `~ %s=%s` (PropertyName, CheckForBlanks(value)) —
                // the set props that survive here (NConds/NPhases/NormAmps/
                // EmergAmps/Seasons/Ratings/LineType). The empty-value skip
                // mirrors the generic [`crate::report::save::save::save_write`]
                // (Pascal writes unconditionally because its `PropertyValue` is
                // always populated; our `like=` source name is not retained, so
                // its `get_value` is empty — skipping it drops a redundant clone
                // directive, exactly as every other class's generic save does,
                // since the real per-conductor props are emitted explicitly).
                // A rating that is not set is omitted.
                _ if cx.cls.holds_no_rating(self, ip) => {}
                _ => generic(out, ip),
            }
            iprop = self.data().next_property_set(Some(ip));
        }
    }

    /// One `Cond=i [<kind>=<name>] [X=%.7g] [h=%.7g] units=<str>` token group
    /// per conductor that holds a conductor object, an `x` or an `h`, skipping
    /// a conductor that holds none of them. `<kind>` = `tscable`/`cncable`/
    /// `wire` by the stored conductor's catalog type, written when it holds
    /// one. `X=` and `h=` are written only when given, so a reload holds the
    /// same positions and reports the same missing ones. `units=` is the
    /// conductor's unit, its own or the default, never `none`. Pinned by
    /// `exec::tests::line_geometry_rules::save_writes_each_given_position_and_no_other`.
    fn write_conductor_block(&self, out: &mut String) {
        for i in 0..self.fnconds.max(0) as usize {
            let object = self.fwiredata[i].as_ref();
            let (x, h) = (self.fx[i], self.fy[i]);
            if object.is_none() && x.is_none() && h.is_none() {
                continue;
            }
            out.push_str(&format!(" Cond={}", i + 1));
            if let Some(w) = object {
                let kind = match w.conductor_kind() {
                    ConductorKind::Ts => "tscable",
                    ConductorKind::Cn => "cncable",
                    ConductorKind::Wire => "wire",
                };
                out.push_str(&format!(" {kind}={}", w.name()));
            }
            if let Some(x) = x {
                out.push_str(&format!(" X={}", g(x, 7)));
            }
            if let Some(h) = h {
                out.push_str(&format!(" h={}", g(h, 7)));
            }
            out.push_str(&format!(
                " units={}",
                LineUnits::from_code(self.conductor_unit(i)).as_str()
            ));
        }
    }
}
