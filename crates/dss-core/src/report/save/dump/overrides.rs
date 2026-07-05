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
//! WP8.5 step 1 ports the **Reactor** override (the `Dump reactor.* debug`
//! corpus deck). The remaining 13 overrides — Transformer, Line, LineCode,
//! LineGeometry, XfmrCode (per-winding/matrix, step 2), Capacitor, Fault,
//! VSource, UPFC, RegControl, Monitor, EnergyMeter, Spectrum (tail-adds) —
//! are TODO(WP8): until each lands, its object dumps via the generic base, which
//! mis-orders the property block (props before `! ENABLED`) and drops the
//! Complete-only tail. No corpus deck dumps those classes yet (only
//! `dump reactor.*` and `dump transformer.*`, the latter step 2), so this is a
//! tracked incremental gap, not a live divergence.

use crate::elements::pd::reactor::Reactor;
use crate::obj::base::DssObject;

use super::DumpCtx;

/// Returns `true` (and writes the override dump) if `obj`'s class has a ported
/// Pascal `DumpProperties` override; `false` to fall through to the generic base.
pub(super) fn dump_override(
    out: &mut String,
    cx: &DumpCtx,
    obj: &dyn DssObject,
    complete: bool,
) -> bool {
    let any = obj.as_any();
    if let Some(r) = any.downcast_ref::<Reactor>() {
        r.dump_body(out, cx, complete);
        return true;
    }
    false
}
