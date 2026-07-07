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
//! **LineGeometry** and **XfmrCode**; step 3a adds the remaining 8 —
//! **Capacitor**, **Fault**, **VSource**, **UPFC**, **RegControl**, **Monitor**,
//! **EnergyMeter**, **Spectrum**. Every Pascal `DumpProperties` override that
//! exists in this port is now dispatched here — the only two NOT covered
//! (`AutoTrans`, `GICLine`) are Phase-9-deferred, unported classes.
//!
//! WPG.14 adds one more dispatch entry, **Isource** — not a real Pascal
//! override (`TIsourceObj` has none), but it needs one here anyway: it is
//! `NON_PCPD_ELEM` like VSource, so the generic path's `is_pc` (driven by
//! `Circuit.pc_elements` membership) can't select the `TPCElement` dump
//! ordering it actually needs (see `elements/pc/isource/dump.rs`).

use crate::elements::control::reg_control::RegControl;
use crate::elements::general::line_code::LineCodeObj;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xfmr_code::XfmrCodeObj;
use crate::elements::meter::EnergyMeter;
use crate::elements::meter::monitor::Monitor;
use crate::elements::pc::isource::Isource;
use crate::elements::pc::upfc::Upfc;
use crate::elements::pc::vsource::VSource;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::fault::Fault;
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
    if let Some(c) = any.downcast_ref::<Capacitor>() {
        c.dump_body(out, cx, complete);
        return true;
    }
    if let Some(f) = any.downcast_ref::<Fault>() {
        f.dump_body(out, cx, complete);
        return true;
    }
    if let Some(v) = any.downcast_ref::<VSource>() {
        v.dump_body(out, cx, complete);
        return true;
    }
    // Not a real Pascal override (Isource has none) — see `isource/dump.rs`
    // for why this dispatch is still needed.
    if let Some(i) = any.downcast_ref::<Isource>() {
        i.dump_body(out, cx, complete);
        return true;
    }
    if let Some(u) = any.downcast_ref::<Upfc>() {
        u.dump_body(out, cx, complete);
        return true;
    }
    if let Some(r) = any.downcast_ref::<RegControl>() {
        r.dump_body(out, cx, complete);
        return true;
    }
    if let Some(m) = any.downcast_ref::<Monitor>() {
        m.dump_body(out, cx, complete);
        return true;
    }
    if let Some(e) = any.downcast_ref::<EnergyMeter>() {
        e.dump_body(out, cx, complete);
        return true;
    }
    if let Some(s) = any.downcast_ref::<SpectrumObj>() {
        s.dump_body(out, cx, complete);
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
