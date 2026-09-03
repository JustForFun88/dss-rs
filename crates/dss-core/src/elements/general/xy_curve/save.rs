//! Pascal `TXYcurveObj.SaveWrite` (r4133
//! `Version8/Source/General/XYcurve.pas:978-1003`) — the `Save`-serializer
//! override. Adopted by RP3.11 P2-P4 (§1.1: the port's override set is the
//! *union* of both upstreams', because each one exists to keep the emitted deck
//! re-compilable); dss_capi 0.14.5 dropped it, so the port never had it.
//!
//! `Npts` sizes the `XArray`/`YArray` allocation on reload, so it has to be the
//! first token on the line. In the parse chain it is not: r4133's
//! `TXYcurveObj.Set_NumPoints` re-stamps `PropertyValue[1]`
//! (`XYcurve.pas:1005-1019`) whenever an array property is parsed, pushing
//! `npts` to the *back* of the chain, and this port stamps it wherever the deck
//! typed it — which for a deck that re-sets `npts` last is dead last:
//! `New "XYcurve.xy2" XArray=[ 0 1] YArray=[ 0 2] NPts=2` (measured before this
//! override, RP3.11 I1). r4133 on the same deck writes
//! `New "XYcurve.xy2" Npts=2 Xarray=[0, 1, ] Yarray=[0, 2, ]`.
//!
//! Co-located with the element (like the [`crate::elements::pd::line`] and
//! [`crate::elements::pd::transformer`] overrides) and dispatched from
//! [`crate::report::save::save::write_dss_object`] via a typed arena read.

use crate::obj::base::DssObject;
use crate::report::save::save::{SaveCtx, save_write_token};

use super::{XyCurveObj, prop::NPTS};

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
                // currently unobservable; it is written down because the doc
                // above claims loop-for-loop fidelity to the Pascal (RP3.11
                // audit finding AC-4).
                save_write_token(out, cx, self, i);
            }
            iprop = self.data().next_property_set(Some(i));
        }
    }
}
