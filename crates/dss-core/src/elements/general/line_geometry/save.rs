//! Pascal `TLineGeometryObj.SaveWrite` (`General/LineGeometry.pas:792-839`,
//! vendored dss_capi 0.14.5 — the pinned save oracle) — the `Save`-serializer
//! override. The per-conductor structure is not conducive to the generic "one
//! `name=value` per set property" form (the generic serializer would emit only
//! the LAST conductor, since `Cond`/`Wire`/`X`/`H`/`Units` each carry a single
//! property-sequence slot re-set once per conductor). The override instead
//! rewrites the whole conductor table (`Cond=i wire=.. X=.. h=.. units=..`,
//! once, when `cond=`/`spacing=`/`wires=` was ever set), skipping the scalar
//! per-conductor props, and lets every other set property fall through to the
//! generic `name=value` rule.
//!
//! Co-located with the element (like [`super::dump`]) so it reads the conductor
//! fields directly, exactly as the Pascal method does; dispatched from
//! [`crate::report::save::save::write_dss_object`] via a typed arena read. Emitted inline
//! on the single `New "…"` line (like the [`crate::elements::pd::transformer`]
//! override) rather than as the oracle's `~ Cond=…` continuation lines — the two
//! are token-equivalent on re-parse and the `Save` contract is round-trip
//! fidelity, not byte-equality (`report/save/save.rs` header).

use crate::elements::general::conductor_data::ConductorKind;
use crate::obj::base::DssObject;
use crate::report::format::g;
use crate::report::save::save::SaveCtx;
use crate::support::line_units::LineUnits;
use crate::util::check_for_blanks;

use super::{LineGeometryObj, prop};

impl LineGeometryObj {
    /// Pascal `TLineGeometryObj.SaveWrite` body (the caller already emitted
    /// `New "LineGeometry.name"`). Appends ` name=value` tokens to `out`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        use prop::*;

        // Pascal `wroteConds` — the array block is written at most once, at the
        // first of `cond`/`spacing`/`wires` in the set-order walk.
        let mut wrote_conds = false;
        let mut iprop = self.data().next_property_set(None);
        while let Some(ip) = iprop {
            match ip {
                COND | SPACING | WIRES => {
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
                _ => {
                    let val = cx.cls.get_value(self, ip, cx.enums);
                    let s = val.trim();
                    if !s.is_empty() {
                        out.push_str(&format!(
                            " {}={}",
                            cx.cls.property_name(ip),
                            check_for_blanks(s)
                        ));
                    }
                }
            }
            iprop = self.data().next_property_set(Some(ip));
        }
    }

    /// Pascal conductor loop (`:813-825`): one `Cond=i <kind>=<name> X=%.7g
    /// h=%.7g units=<str>` token per conductor, `<kind>` = `tscable`/`cncable`/
    /// `wire` by the stored conductor's catalog type, skipping unset (NIL)
    /// conductors.
    fn write_conductor_block(&self, out: &mut String) {
        for i in 0..self.fnconds.max(0) as usize {
            let Some(w) = self.fwiredata[i].as_ref() else {
                continue; // Pascal `if FWireData[i] = NIL then continue`.
            };
            let kind = match w.as_conductor().map(|c| c.conductor_kind()) {
                Some(ConductorKind::Ts) => "tscable",
                Some(ConductorKind::Cn) => "cncable",
                _ => "wire",
            };
            out.push_str(&format!(
                " Cond={} {}={} X={} h={} units={}",
                i + 1,
                kind,
                w.data().name(),
                g(self.fx[i], 7),
                g(self.fy[i], 7),
                LineUnits::from_code(self.funits[i]).as_str(),
            ));
        }
    }
}
