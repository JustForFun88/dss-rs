//! The XYcurve `Save`-serializer override, as r4133 has it. This engine's
//! override set is the *union* of both OpenDSS engines', because each one
//! exists to keep the emitted deck re-compilable. dss_capi 0.14.5 dropped this
//! one.
//!
//! `Npts` sizes the `XArray`/`YArray` allocation on reload, so it has to be the
//! first token on the line. In the parse chain it is not: r4133's
//! `TXYcurveObj.Set_NumPoints` re-stamps `PropertyValue[1]`
//! (`XYcurve.pas:1005-1019`) whenever an array property is parsed, pushing
//! `npts` to the *back* of the chain, and this engine stamps it wherever the
//! deck typed it, which for a deck that re-sets `npts` last is dead last. The
//! override writes `Npts` first, as r4133 does.
//!
//! Co-located with the element (like the [`crate::elements::pd::line`] and
//! [`crate::elements::pd::transformer`] overrides) and dispatched from
//! [`crate::report::save::save::write_dss_object`] via a typed arena read.

use crate::obj::base::DssObject;
use crate::report::save::save::{SaveCtx, save_write_token};

use super::{XyCurveObj, prop::NPTS};

// BUG(C5): `XyCurveObj` derives `Clone` though it owns mutable state.
impl XyCurveObj {
    /// Pascal `TXYcurveObj.SaveWrite` body (the caller already emitted
    /// `New "XYcurve.name"`). Appends ` name=value` tokens to `out`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        // Pascal `Write(F, Format(' Npts=%d',[NumPoints]))` — the **live**
        // point count, written outside the chain and regardless of whether the
        // deck ever typed `npts`. The name is rendered from this port's own
        // property table (`NPts`) rather than r4133's hardcoded `Npts` literal,
        // which does not match r4133's own `PropertyName^[1]` (`'npts'`)
        // either; one spelling per property, and the re-parse is
        // case-insensitive.
        save_write_token(out, cx, self, NPTS);
        // Pascal `While iProp > 0` with `CASE RevPropertyIdxMap^[iProp] of 1:
        // {Ignore Npts}` — every other set property in chain order.
        let mut iprop = self.data().next_property_set(None);
        while let Some(i) = iprop {
            if i != NPTS {
                // Pascal writes ` %s=%s` unconditionally here (no trim, no
                // sentinel test); `save_write_token` applies the *generic*
                // `SaveWrite` body instead — `trim`, the `----` skip and the
                // empty skip — exactly as the Line override does
                // (`elements/pd/line/save.rs`). All three can only *suppress* a
                // token that would not re-parse: our `get_value` re-renders from
                // state, so an empty (or blank-only, or `----`) render would emit
                // a bare ` Name=`. No reachable XYcurve property renders leading
                // or trailing blanks or the sentinel, so the deviation is
                // currently unobservable. It is written down because this body
                // otherwise follows the Pascal loop for loop.
                save_write_token(out, cx, self, i);
            }
            iprop = self.data().next_property_set(Some(i));
        }
    }
}
