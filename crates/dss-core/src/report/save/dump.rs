//! Pascal `DumpProperties` (the `Dump` command) — `Executive/ExecHelper.pas`
//! `DoPropertyDump`, `General/DSSObject.pas` `TDSSObject.DumpProperties`,
//! `Common/CktElement.pas` `TDSSCktElement.DumpProperties`,
//! `PCElements/PCElement.pas` `TPCElement.DumpProperties`, plus the 14 element/
//! object leaf overrides (Solution dumped separately, WP8.5 step 3).
//!
//! WP8.5 **step 1** ported the generic base + the **Reactor** override; **step 2**
//! adds Transformer/Line/LineCode/LineGeometry/XfmrCode. The 8 remaining overrides
//! are TODO(WP8) in [`overrides`] (until each lands, its class dumps via the
//! generic base).
//!
//! Pascal models this as a 4-level virtual method (`TDSSObject` →
//! `TDSSCktElement` → `TPCElement` → leaf). Rust has no inheritance, so we
//! reproduce it with a **generic base** ([`dump_object`], selected by the
//! element kind — plain object / CktElement / PCElement) plus per-class override
//! bodies dispatched by `downcast_ref` (the same pattern the register exports
//! use). Property lines reuse [`ClassProps::get_value`] — Pascal
//! `PropertyValue[i] == GetPropertyValue(i)` for every element (only
//! `TLoadShapeObj` overrides the getter, and LoadShape has no `DumpProperties`
//! override), so the ported renderer is byte-faithful (`props_roundtrip` pins it).

use crate::elements::ckt::CktElementData;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::ClassProps;
use crate::report::format;

mod overrides;

/// Context handed to every object dump: the object's class prop table, the enum
/// registry (`ClassProps::get_value` needs it), and — for the PCElement
/// `! VARIABLES` block — the precomputed `(name, value)` per dynamic-state
/// variable (empty unless `complete` and the element is a PC element).
pub struct DumpCtx<'a> {
    pub cls: &'a ClassProps,
    pub enums: &'a EnumRegistry,
    pub variables: &'a [(String, f64)],
}

/// Pascal `EncloseQuotes(s) = '"' + s + '"'` (`Utilities.pas:395`).
pub(crate) fn enq(s: &str) -> String {
    format!("\"{s}\"")
}

/// Pascal `TDSSObject.DumpProperties` header: a blank line then `New "FullName"`
/// (`FullName = ParentClass.Name + '.' + Name`, `DSSObject.pas:232`).
pub(crate) fn header(out: &mut String, full_name: &str) {
    out.push('\n');
    out.push_str("New ");
    out.push_str(&enq(full_name));
    out.push('\n');
}

/// The `~ <PropertyName>=<GetPropertyValue(i)>` loop for `i := 1 to
/// NumProperties` (Pascal `TDSSObject.DumpProperties` Leaf branch, and every
/// override that dumps the full property set verbatim).
pub(crate) fn generic_props(out: &mut String, cx: &DumpCtx, obj: &dyn DssObject) {
    generic_props_from(out, cx, obj, 1);
}

/// The `~ <PropertyName>=<GetPropertyValue(i)>` loop for `i := start to
/// NumProperties`. The per-class overrides that hand-write the head of the
/// property list (Transformer/XfmrCode) dump their tail through this.
pub(crate) fn generic_props_from(
    out: &mut String,
    cx: &DumpCtx,
    obj: &dyn DssObject,
    start: usize,
) {
    for i in start..=cx.cls.num_properties() {
        prop_line(out, cx, obj, i);
    }
}

/// One `~ <PropertyName[i]>=<GetPropertyValue(i)>` line (Pascal
/// `'~ ' + ParentClass.PropertyName[i] + '=' + PropertyValue[i]`).
pub(crate) fn prop_line(out: &mut String, cx: &DumpCtx, obj: &dyn DssObject, i: usize) {
    out.push_str("~ ");
    out.push_str(cx.cls.property_name(i));
    out.push('=');
    out.push_str(&cx.cls.get_value(obj, i, cx.enums));
    out.push('\n');
}

/// Pascal `TDSSCktElement.DumpProperties`: `! ENABLED` / `! DISABLED`.
pub(crate) fn enabled_line(out: &mut String, enabled: bool) {
    out.push_str(if enabled {
        "! ENABLED\n"
    } else {
        "! DISABLED\n"
    });
}

/// Pascal `TDSSCktElement.DumpProperties` `Complete` block: the phase/conductor/
/// terminal counts, the NodeRef list, terminal open/closed status, terminal bus
/// refs, then the primitive-Y G and B full matrices (`Format(' %13.10g |', …)`).
pub(crate) fn cktelem_complete(out: &mut String, cd: &CktElementData) {
    out.push_str(&format!("! NPhases = {}\n", cd.nphases));
    out.push_str(&format!("! Nconds = {}\n", cd.nconds));
    out.push_str(&format!("! Nterms = {}\n", cd.nterms));
    out.push_str(&format!("! Yorder = {}\n", cd.yorder));

    out.push_str("! NodeRef = \"");
    if cd.node_ref.is_empty() {
        out.push_str("nil");
    } else {
        // Pascal writes `NodeRef[i]` for i := 1 to Yorder (the global 1-based node
        // numbers; 0 = ground), each trailed by a space.
        for &n in cd.node_ref.iter().take(cd.yorder) {
            out.push_str(&format!("{n} "));
        }
    }
    out.push_str("\"\n");

    out.push_str("! Terminal Status: [");
    for t in 0..cd.nterms {
        for c in 0..cd.nconds {
            let closed = cd
                .terminals
                .get(t)
                .and_then(|term| term.conductors_closed.get(c).copied())
                .unwrap_or(true);
            out.push_str(if closed { "C " } else { "O " });
        }
    }
    out.push_str("]\n");

    out.push_str("! Terminal Bus Ref: [");
    for t in 0..cd.nterms {
        // Pascal `Terminals[t].BusRef` is 1-based into `BusList`; our `bus_ref` is
        // the 0-based `ckt.buses` index, so +1 to match. An **unset** terminal
        // (`bus_ref == MAX`, e.g. a disabled element — `ReprocessBusDefs` resolves
        // refs only for enabled elements) is Pascal `BusRef = -1` (`Terminal.pas:41`,
        // "signify not set"), which `IntToStr` renders `-1` — NOT 0.
        let br: i64 = cd
            .terminals
            .get(t)
            .map(|term| term.bus_ref)
            .filter(|&b| b != usize::MAX)
            .map(|b| b as i64 + 1)
            .unwrap_or(-1);
        for _ in 0..cd.nconds {
            out.push_str(&format!("{br} "));
        }
    }
    out.push_str("]\n");
    out.push('\n');

    if let Some(yprim) = &cd.yprim {
        let n = yprim.order();
        out.push_str("! YPrim (G matrix)\n");
        for i in 0..n {
            out.push_str("! ");
            for j in 0..n {
                out.push_str(&format!(" {} |", format::g_w(yprim.get(i, j).re, 13, 10)));
            }
            out.push('\n');
        }
        out.push_str("! YPrim (B Matrix) = \n");
        for i in 0..n {
            out.push_str("! ");
            for j in 0..n {
                out.push_str(&format!(" {} |", format::g_w(yprim.get(i, j).im, 13, 10)));
            }
            out.push('\n');
        }
    }
}

/// Pascal `TPCElement.DumpProperties` `Complete` block: the `! VARIABLES` header
/// then `! %2d: %s = %-.5g` per dynamic-state variable (values precomputed into
/// `cx.variables` by the dispatcher, which holds the mutable element walk).
pub(crate) fn pc_variables(out: &mut String, vars: &[(String, f64)]) {
    out.push_str("! VARIABLES\n");
    for (i, (name, val)) in vars.iter().enumerate() {
        out.push_str(&format!(
            "! {:2}: {} = {}\n",
            i + 1,
            name,
            format::g(*val, 5)
        ));
    }
}

/// The three generic (non-override) `DumpProperties` behaviors, selected by the
/// element kind. `plain TDSSObject` (`as_ckt_element() == None`, e.g. XYcurve,
/// LoadShape, WireData) writes header + props + (Complete) blank. A non-PC
/// `TDSSCktElement` (the controls) writes header + props + (Complete) blank +
/// `! ENABLED` + (Complete) the CktElement Y/terminal block — props come *before*
/// `! ENABLED` because `inherited` runs with `Leaf=TRUE`. A `TPCElement` (Load,
/// Generator, …) writes header + `! ENABLED` + (Complete) Y-block + (Complete)
/// `! VARIABLES` + props + (Complete) two blanks — props come *after*, because
/// `TPCElement` calls `inherited` with `Leaf=FALSE`.
///
/// The classes with a Pascal `DumpProperties` override are handled by
/// [`overrides::dump_override`] before this is reached (via [`dump_object`]).
pub(crate) fn dump_generic(
    out: &mut String,
    cx: &DumpCtx,
    obj: &dyn DssObject,
    complete: bool,
    is_pc: bool,
) {
    let full_name = format!("{}.{}", cx.cls.class_name(), obj.data().name());
    match obj.as_ckt_element() {
        None => {
            // Plain TDSSObject: header + props + (Complete) blank.
            header(out, &full_name);
            generic_props(out, cx, obj);
            if complete {
                out.push('\n');
            }
        }
        Some(elem) if !is_pc => {
            // TDSSCktElement (non-PC): props before ENABLED (inherited Leaf=TRUE).
            header(out, &full_name);
            generic_props(out, cx, obj);
            if complete {
                out.push('\n');
            }
            enabled_line(out, elem.cd().enabled);
            if complete {
                cktelem_complete(out, elem.cd());
            }
        }
        Some(elem) => {
            // TPCElement: ENABLED + Y-block + VARIABLES + props + two blanks.
            header(out, &full_name);
            enabled_line(out, elem.cd().enabled);
            if complete {
                cktelem_complete(out, elem.cd());
                pc_variables(out, cx.variables);
            }
            generic_props(out, cx, obj);
            if complete {
                out.push('\n');
                out.push('\n');
            }
        }
    }
}

/// Dump one object (Pascal top-level `obj.DumpProperties(F, Complete, TRUE)`):
/// the class's leaf override if it has a Pascal one, else the generic base.
/// `is_pc` selects the PCElement ordering for the generic path.
///
/// Takes `&mut dyn DssObject` because one override — `TLineGeometryObj` — walks
/// its conductors by mutating `ActiveCond` (Pascal `ActiveCond := j;
/// GetPropertyValue(3..7)`, `LineGeometry.pas:669`); the other overrides and the
/// generic base read through a shared `&*obj` reborrow.
pub(crate) fn dump_object(
    out: &mut String,
    cx: &DumpCtx,
    obj: &mut dyn DssObject,
    complete: bool,
    is_pc: bool,
) {
    if overrides::dump_override(out, cx, obj, complete) {
        return;
    }
    dump_generic(out, cx, &*obj, complete, is_pc);
}

/// The full-name (`Class.Name`) an override writes in its `New "…"` header.
pub(crate) fn full_name(cx: &DumpCtx, obj: &dyn DssObject) -> String {
    format!("{}.{}", cx.cls.class_name(), obj.data().name())
}

/// The shared prefix an override inherits from a **`TDSSCktElement`** base
/// (`Leaf=FALSE`): header + `! ENABLED` + (Complete) the Y/terminal block. Used
/// by the non-PC CktElement overrides (Line, Transformer, Reactor, Capacitor,
/// Fault, RegControl, Monitor, EnergyMeter).
pub(crate) fn prefix_ckt(
    out: &mut String,
    cx: &DumpCtx,
    obj: &dyn DssObject,
    cd: &CktElementData,
    complete: bool,
) {
    header(out, &full_name(cx, obj));
    enabled_line(out, cd.enabled);
    if complete {
        cktelem_complete(out, cd);
    }
}
