//! Dispatch a `Dump` to a class's Pascal `DumpProperties` **override** body.
//!
//! Pascal models `DumpProperties` as a virtual method; 14 leaf classes (that
//! exist in this port — `AutoTrans`/`GICLine` are Phase-9-deferred, unported)
//! override it. Rust has no inheritance, so [`dump_override`] downcasts the
//! object to each overriding type and calls its co-located `dump_body` (the
//! method lives with the element so it reads its private fields directly, exactly
//! as the Pascal method does). Classes with no override fall through to
//! [`super::dump_generic`].
//!
//! WP8.5 step 1 ported the **Reactor** override; step 2 adds the
//! per-winding/matrix overrides **Transformer**, **Line**, **LineCode**,
//! **LineGeometry** and **XfmrCode**. The remaining 8 overrides — Capacitor,
//! Fault, VSource, UPFC, RegControl, Monitor, EnergyMeter, Spectrum (tail-adds)
//! — are TODO(WP8): until each lands, its object dumps via the generic base,
//! which mis-orders the property block (props before `! ENABLED`) and drops the
//! Complete-only tail.

use crate::elements::general::line_code::LineCodeObj;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::general::xfmr_code::XfmrCodeObj;
use crate::elements::pd::line::Line;
use crate::elements::pd::reactor::Reactor;
use crate::elements::pd::transformer::Transformer;
use crate::obj::base::DssObject;

use super::DumpCtx;

/// Returns `true` (and writes the override dump) if `obj`'s class has a ported
/// Pascal `DumpProperties` override; `false` to fall through to the generic base.
///
/// `obj` is `&mut` because `TLineGeometryObj.DumpProperties` mutates `ActiveCond`
/// as it walks conductors (Pascal `LineGeometry.pas:669`); the read-only
/// overrides borrow it immutably via `as_any`.
pub(super) fn dump_override(
    out: &mut String,
    cx: &DumpCtx,
    obj: &mut dyn DssObject,
    complete: bool,
) -> bool {
    let any = obj.as_any();
    if let Some(r) = any.downcast_ref::<Reactor>() {
        r.dump_body(out, cx, complete);
        return true;
    }
    if let Some(t) = any.downcast_ref::<Transformer>() {
        t.dump_body(out, cx, complete);
        return true;
    }
    if let Some(l) = any.downcast_ref::<Line>() {
        l.dump_body(out, cx, complete);
        return true;
    }
    if let Some(lc) = any.downcast_ref::<LineCodeObj>() {
        lc.dump_body(out, cx, complete);
        return true;
    }
    if let Some(xc) = any.downcast_ref::<XfmrCodeObj>() {
        xc.dump_body(out, cx, complete);
        return true;
    }
    // LineGeometry mutates `ActiveCond` per conductor → needs `&mut` (the
    // immutable `any` borrow above ends at its last `downcast_ref` use, NLL).
    if let Some(g) = obj.as_any_mut().downcast_mut::<LineGeometryObj>() {
        g.dump_body(out, cx, complete);
        return true;
    }
    false
}
