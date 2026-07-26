//! Dispatch a `Dump` to a class's Pascal `DumpProperties` **override** body.
//!
//! Pascal models `DumpProperties` as a virtual method; 14 leaf classes (that
//! exist in this port — `AutoTrans`/`GICLine` are Phase-9-deferred, unported)
//! override it. Rust has no inheritance, so [`dump_override`] narrows the object
//! to each overriding type (a typed `ClassArena` read) and calls its co-located
//! `dump_body` (the method lives with the element so it reads its private fields
//! directly, exactly as the Pascal method does). Classes with no override fall
//! through to
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
use crate::elements::pc::gic_line::GicLine;
use crate::elements::pc::gic_source::GicSource;
use crate::elements::pc::isource::Isource;
use crate::elements::pc::upfc::Upfc;
use crate::elements::pc::vsource::VSource;
use crate::elements::pd::auto_trans::AutoTrans;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::fault::Fault;
use crate::elements::pd::line::Line;
use crate::elements::pd::reactor::Reactor;
use crate::elements::pd::transformer::Transformer;
use crate::obj::arena::ClassArena;

use super::DumpCtx;

/// Returns `true` (and writes the override dump) if object `idx` of `arena` has
/// a ported Pascal `DumpProperties` override; `false` to fall through to the
/// generic base.
///
/// `arena` is `&mut` because `TLineGeometryObj.DumpProperties` mutates
/// `ActiveCond` as it walks conductors (Pascal `LineGeometry.pas:669`); the
/// read-only overrides take a shared typed view.
pub(super) fn dump_override(
    out: &mut String,
    cx: &DumpCtx,
    arena: &mut ClassArena,
    idx: usize,
    complete: bool,
) -> bool {
    if let Some(r) = arena.get::<Reactor>(idx) {
        r.dump_body(out, cx, complete);
        return true;
    }
    if let Some(t) = arena.get::<Transformer>(idx) {
        t.dump_body(out, cx, complete);
        return true;
    }
    if let Some(t) = arena.get::<AutoTrans>(idx) {
        t.dump_body(out, cx, complete);
        return true;
    }
    if let Some(l) = arena.get::<Line>(idx) {
        l.dump_body(out, cx, complete);
        return true;
    }
    if let Some(lc) = arena.get::<LineCodeObj>(idx) {
        lc.dump_body(out, cx, complete);
        return true;
    }
    if let Some(xc) = arena.get::<XfmrCodeObj>(idx) {
        xc.dump_body(out, cx, complete);
        return true;
    }
    if let Some(c) = arena.get::<Capacitor>(idx) {
        c.dump_body(out, cx, complete);
        return true;
    }
    if let Some(f) = arena.get::<Fault>(idx) {
        f.dump_body(out, cx, complete);
        return true;
    }
    if let Some(v) = arena.get::<VSource>(idx) {
        v.dump_body(out, cx, complete);
        return true;
    }
    // Not a real Pascal override (Isource has none) — see `isource/dump.rs`
    // for why this dispatch is still needed.
    if let Some(i) = arena.get::<Isource>(idx) {
        i.dump_body(out, cx, complete);
        return true;
    }
    // Real Pascal override (GICLine.pas:627 — Z Matrix / VE / VN block).
    if let Some(g) = arena.get::<GicLine>(idx) {
        g.dump_body(out, cx, complete);
        return true;
    }
    // Not a real Pascal override (GICsource has none) — NON_PCPD like Isource,
    // needs the TPCElement dump ordering (see `gic_source/dump.rs`).
    if let Some(g) = arena.get::<GicSource>(idx) {
        g.dump_body(out, cx, complete);
        return true;
    }
    if let Some(u) = arena.get::<Upfc>(idx) {
        u.dump_body(out, cx, complete);
        return true;
    }
    if let Some(r) = arena.get::<RegControl>(idx) {
        r.dump_body(out, cx, complete);
        return true;
    }
    if let Some(m) = arena.get::<Monitor>(idx) {
        m.dump_body(out, cx, complete);
        return true;
    }
    if let Some(e) = arena.get::<EnergyMeter>(idx) {
        e.dump_body(out, cx, complete);
        return true;
    }
    if let Some(s) = arena.get::<SpectrumObj>(idx) {
        s.dump_body(out, cx, complete);
        return true;
    }
    // LineGeometry mutates `ActiveCond` per conductor → needs `&mut` (the
    // shared borrows above end at their last use, NLL).
    if let Some(g) = arena.get_mut::<LineGeometryObj>(idx) {
        g.dump_body(out, cx, complete);
        return true;
    }
    false
}
