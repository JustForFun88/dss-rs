//! Pascal `TLineGeometryObj.SaveWrite` (`General/LineGeometry.pas:792`, the
//! vendored dss_capi 0.14.5 pinned-oracle source) — the `Save`-serializer
//! override. The multi-conductor structure is not conducive to the generic
//! "one `name=value` per set property" form: the generic [`save_write`] would
//! collapse the array properties (`Wire`/`X`/`H`/`Units`) to a **single**
//! conductor (whichever was set last), producing a geometry with undefined
//! conductors that fails to re-solve ("WireData is not correctly initialized").
//!
//! Instead, when `Cond`/`Spacing`/`Wires` was ever set, emit one
//! `~ Cond=i <choice>=<wire> X=.. h=.. units=..` line **per conductor**; `Reduce`
//! becomes `~ Reduce=Yes` only when set; the per-conductor scalar keys
//! (`Wire`/`X`/`H`/`Units`/`CNCable`/`TSCable`) are otherwise ignored; every
//! other set property uses the generic `~ name=value` form.
//!
//! Co-located with the element (like [`super::dump`]) so it reads the conductor
//! arrays directly, exactly as the Pascal method does; dispatched from
//! [`crate::report::save::save::write_dss_object`] via downcast.
//!
//! [`save_write`]: crate::report::save::save::save_write

use crate::obj::base::DssObject;
use crate::report::save::save::SaveCtx;
use crate::support::line_units::LineUnits;
use crate::util::check_for_blanks;

use super::{ConductorChoice, LineGeometryObj, prop};

impl LineGeometryObj {
    /// Pascal `TLineGeometryObj.SaveWrite` body (the caller already emitted
    /// `New "LineGeometry.name"`). Appends `\n~ name=value` continuation lines.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        let mut wrote_conds = false;
        let mut iprop = self.data().next_property_set(None);
        while let Some(ip) = iprop {
            match ip {
                // `Cond`/`Spacing`/`Wires` → write the whole conductor array once.
                prop::COND | prop::SPACING | prop::WIRES => {
                    if !wrote_conds {
                        for i in 0..self.fnconds.max(0) as usize {
                            let Some(wire) = self.fwiredata.get(i).and_then(|o| o.as_ref()) else {
                                continue; // NIL — shouldn't happen in normal use
                            };
                            let choice = match self.fphase_choice.get(i) {
                                Some(ConductorChoice::TapeShield) => "tscable",
                                Some(ConductorChoice::ConcentricNeutral) => "cncable",
                                _ => "wire",
                            };
                            let units = LineUnits::from_code(self.funits[i]).as_str();
                            out.push_str(&format!(
                                "\n~ Cond={} {}={} X={} h={} units={}",
                                i + 1,
                                choice,
                                wire.data().name(),
                                crate::util::fmt_g(self.fx[i], 7),
                                crate::util::fmt_g(self.fy[i], 7),
                                units,
                            ));
                        }
                        wrote_conds = true;
                    }
                }
                // `Reduce` → only when set true.
                prop::REDUCE => {
                    if self.freduce {
                        out.push_str("\n~ Reduce=Yes");
                    }
                }
                // Per-conductor scalar keys are folded into the `Cond=` block.
                prop::WIRE | prop::X | prop::H | prop::UNITS | prop::CNCABLE | prop::TSCABLE => {}
                // Everything else: the generic `~ name=value` form.
                _ => {
                    let val = cx.cls.get_value(self, ip, cx.enums);
                    let s = val.trim();
                    if !s.is_empty() {
                        out.push_str("\n~ ");
                        out.push_str(cx.cls.property_name(ip));
                        out.push('=');
                        out.push_str(&check_for_blanks(s));
                    }
                }
            }
            iprop = self.data().next_property_set(Some(ip));
        }
    }
}
