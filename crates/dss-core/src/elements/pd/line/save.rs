//! Pascal `TLineObj.SaveWrite` (`PDElements/Line.pas:2146-2205`, vendored
//! dss_capi 0.14.5 — the pinned save oracle) — the `Save`-serializer override.
//!
//! `Line.Wires`/`CNCables`/`TSCables` all alias the single `LineWireData`
//! conductor array (they share `PropertyOffset`; `CNCables`/`TSCables` are even
//! flagged `Redundant` with `Wires`). The generic serializer renders each *set*
//! array property from the whole array, so a line built with a **mixed** set of
//! conductor kinds — e.g. `TSCables=[TS_1/0] Wires=[CU_1/0]` — is re-emitted as
//! `TSCables=[ts_1/0, cu_1/0] Wires=[ts_1/0, cu_1/0]`: every conductor under
//! every kind that was ever set. Reloading then fails (`TSData "cu_1/0" not
//! found` — a wire looked up in the TSData catalog).
//!
//! The Pascal override instead walks the conductor array once and emits
//! **contiguous runs of same-catalog conductors**, each under its correct kind
//! (`TSCables`/`CNCables`/`Wires` by the stored conductor's `ParentClass`), and
//! lets every other set property fall through to the generic `name=value` rule.
//!
//! Co-located with the element (like the [`super::super::transformer`] and
//! [`crate::elements::general::line_geometry`] overrides); dispatched from
//! [`crate::report::save::save::write_dss_object`] via downcast. Emitted inline
//! on the single `New "…"` line rather than the oracle's `~` continuation — the
//! two are token-equivalent on re-parse and the `Save` contract is round-trip
//! fidelity, not byte-equality (`report/save/save.rs` header).

use crate::elements::general::conductor_data::ConductorKind;
use crate::obj::base::DssObject;
use crate::report::save::save::SaveCtx;
use crate::util::check_for_blanks;

use super::Line;

impl Line {
    /// Pascal `TLineObj.SaveWrite` body (the caller already emitted
    /// `New "Line.name"`). Appends ` name=value` tokens to `out`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        use super::prop::*;

        // Pascal `wroteConds` — the conductor block is written at most once, at
        // the first of `wires`/`cncables`/`tscables` in the set-order walk.
        let mut wrote_conds = false;
        let mut iprop = self.data().next_property_set(None);
        while let Some(ip) = iprop {
            match ip {
                WIRES | CNCABLES | TSCABLES | CONDUCTORS => {
                    if !wrote_conds {
                        self.write_conductor_arrays(out);
                        wrote_conds = true;
                    }
                }
                // Pascal ELSE: `~ %s=%s` (PropertyName, CheckForBlanks(value)).
                // Mirror the generic [`crate::report::save::save::save_write`]
                // (skip empty / the `----` conditional-value sentinel) — Pascal
                // writes unconditionally because its `PropertyValue` is always
                // populated, but our `get_value` re-renders from state and can be
                // empty for an unset alias; skipping an empty value only drops a
                // redundant token, exactly as every other class's generic save.
                _ => {
                    let val = cx.cls.get_value(self, ip, cx.enums);
                    let mut s = val.trim();
                    if s.eq_ignore_ascii_case("----") {
                        s = "";
                    }
                    if !s.is_empty() {
                        out.push(' ');
                        out.push_str(cx.cls.property_name(ip));
                        out.push('=');
                        out.push_str(&check_for_blanks(s));
                    }
                }
            }
            iprop = self.data().next_property_set(Some(ip));
        }
    }

    /// Pascal conductor loop (`Line.pas:2166-2197`): emit one ` <kind>=[n1, n2,
    /// …]` token per **contiguous run** of conductors sharing a catalog class,
    /// `<kind>` = `TSCables`/`CNCables`/`Wires` by the stored conductor's type,
    /// skipping unset (NIL) conductors.
    fn write_conductor_arrays(&self, out: &mut String) {
        let n = self.line_wire_data.len();
        let mut i = 0;
        while i < n {
            let Some(w) = self.line_wire_data[i].as_ref() else {
                i += 1; // Pascal `if LineWireData[i] = NIL then continue`.
                continue;
            };
            let kind = conductor_kind(w.as_ref());
            let mut names = check_for_blanks(w.data().name());
            let mut j = i + 1;
            while j < n {
                match self.line_wire_data[j].as_ref() {
                    // Pascal `if conductorCls <> ParentClass then break`.
                    Some(wj) if conductor_kind(wj.as_ref()) == kind => {
                        names.push_str(", ");
                        names.push_str(&check_for_blanks(wj.data().name()));
                        j += 1;
                    }
                    _ => break,
                }
            }
            out.push_str(&format!(" {kind}=[{names}]"));
            i = j.max(i + 1); // Pascal `if i = i0 then inc(i)` (single-run guard).
        }
    }
}

/// The `Save`-array kind for a conductor by its catalog class (Pascal
/// `LineWireData[i].ParentClass`): `TSData` → `TSCables`, `CNData` → `CNCables`,
/// else (`WireData`) → `Wires`.
fn conductor_kind(w: &dyn DssObject) -> &'static str {
    match w.as_conductor().map(|c| c.conductor_kind()) {
        Some(ConductorKind::Ts) => "TSCables",
        Some(ConductorKind::Cn) => "CNCables",
        _ => "Wires",
    }
}
