//! Pascal `DumpProperties` (the `Dump` command) — `Executive/ExecHelper.pas`
//! `DoPropertyDump`, `General/DSSObject.pas` `TDSSObject.DumpProperties`,
//! `Common/CktElement.pas` `TDSSCktElement.DumpProperties`,
//! `PCElements/PCElement.pas` `TPCElement.DumpProperties`, plus the 14 element/
//! object leaf overrides ([`overrides`], WP8.5 steps 1–3a) and the aux forms
//! (WP8.5 step 3b): [`solution`] (`Solution.DumpProperties`),
//! [`circuit_debug`] (`Circuit.DebugDump`), [`commands`]
//! (`DumpAllDSSCommands`) and [`allocation_factors`]; the hash-list dumps live
//! in `support/hashlist/thash_dump.rs`.
//!
//! Pascal models this as a 4-level virtual method (`TDSSObject` →
//! `TDSSCktElement` → `TPCElement` → leaf). Rust has no inheritance, so we
//! reproduce it with a **generic base** ([`dump_object`], selected by the
//! element kind — plain object / CktElement / PCElement) plus per-class override
//! bodies dispatched by a typed `ClassArena` read (the same pattern the register exports
//! use). Property lines reuse [`ClassProps::get_value`] — Pascal
//! `PropertyValue[i] == GetPropertyValue(i)` for every element, so the ported
//! renderer is byte-faithful against the pinned oracle (`props_roundtrip` pins
//! it).
//!
//! **Whose getter that is (corrected by R4133_PROPS RP3.11, 2026-09-03).** The
//! claim this doc used to make — *"only `TLoadShapeObj` overrides the getter"* —
//! is true of the pinned dss_capi 0.14.5 (exactly two `GetPropertyValue`
//! definitions in `src/`: the base and `TLoadShapeObj`) and **false of r4133**,
//! which overrides it in **49** units of `Version8/Source` to answer *live* on a
//! hand-picked index set. RP3.11 decided the surface for every class at once and
//! recorded the verdict `KEEP_LIVE_PINNED`: `Dump` keeps rendering the live field
//! through the one `get_value` it shares with [`super::save`], `?`, the property
//! API and batchedit. Matching r4133's stored `~ model=3` would mean printing a
//! value the engine knows to be superseded (`gen_model == 4` after the NCIM
//! PV→PQ conversion), which CLAUDE.md's 2026-08-02 policy forbids — and `Dump`
//! applies no `PrpSequence` filter (`PCElements/generator.pas:2489-2500`), so all
//! 88 committed `dump*` artifacts sit behind the alternative. The divergence is
//! pinned, both engines' bytes quoted, by [`crate::exec::tests::report`]`::
//! dump_renders_the_live_model_after_ncim_pv2pq`; the `Save`-side policy
//! paragraph lives in [`super::save`]. Recorded and **not** answered there:
//! r4133 carries **64** `DumpProperties` overrides to the 20 this port inherited
//! from 0.14.5 (`!DQDV=`, the 34/36 double-paren wrap, the hardcoded
//! `~ Refuel=False` that contradicts r4133's own getter) — a scope question with
//! those same 88 goldens behind it, tracked in STATUS §RP3.11.

use crate::elements::ckt::CktElementData;
use crate::elements::meter::EnergyMeter;
use crate::exec::registry::DssClass;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::ClassProps;
use crate::report::format;

pub(crate) mod circuit_debug;
pub(crate) mod commands;
mod overrides;
pub(crate) mod solution;

/// Pascal `DumpAllocationFactors` (`Utilities.pas:784-819`), the
/// `Dump alloc…` file body: one line per load, but ONLY for the
/// `ConnectedkVA_PF` (`Load.<name>.AllocationFactor=%-.5g`) and `kwh_PF`
/// (`Load.<name>.CFactor=%-.5g`) spec types — every other `LoadSpecType`
/// prints nothing (the Pascal `case` has no else; probe-proven: kW/PF loads
/// are absent from the file).
pub(crate) fn allocation_factors(classes: &[DssClass], ckt: &crate::circuit::Circuit) -> String {
    use crate::elements::pc::load::{Load, LoadSpec};
    let mut s = String::new();
    for &r in &ckt.loads {
        let Some(load) = classes[r.class_ord()].arena.get::<Load>(r.index()) else {
            continue;
        };
        match load.load_spec_type {
            LoadSpec::ConnectedKvaPf => s.push_str(&format!(
                "Load.{}.AllocationFactor={}\n",
                load.data().name(),
                format::g(load.kva_allocation_factor, 5)
            )),
            LoadSpec::KwhPf => s.push_str(&format!(
                "Load.{}.CFactor={}\n",
                load.data().name(),
                format::g(load.c_factor, 5)
            )),
            _ => {}
        }
    }
    s
}

/// Context handed to every object dump: the object's class prop table, the enum
/// registry (`ClassProps::get_value` needs it), the precomputed `(name, value)`
/// per PC dynamic-state variable (empty unless `complete` and the element is a
/// PC element, for the `! VARIABLES` block), and the precomputed `Branch List:`
/// body text for the EnergyMeter override (empty for every other class — needs
/// the full class registry to resolve each branch/shunt `ElemId`'s name, which
/// this per-object context otherwise has no reach into).
pub struct DumpCtx<'a> {
    pub cls: &'a ClassProps,
    pub enums: &'a EnumRegistry,
    pub variables: &'a [(String, f64)],
    pub branch_list: &'a str,
}

/// Pascal `EnergyMeter.pas:2102-2116`, the `Branch List:` walk for the
/// `Dump energymeter.…` Complete tail: one `Circuit Element = <bare Name>` line
/// per branch (`BranchList.First`/`GoForward`), then one `   Shunt Element =
/// <FullName>` per attached shunt object (`FirstObject`/`NextObject`) — no tab
/// indentation (unlike `Show Zone`'s tree-level tabs). Empty (no rows, just the
/// caller's `Branch List:` header) when the zone was never built
/// (`BranchList = NIL`).
pub(crate) fn energy_meter_branch_list(classes: &[DssClass], em: &EnergyMeter) -> String {
    let mut s = String::new();
    let Some(tree) = em.branch_list() else {
        return s;
    };
    for (i, &br) in em.sequence_list().iter().enumerate() {
        let node = tree.node(em.sequence_nodes()[i]);
        let name = classes[br.class_ord()].arena[br.index()].data().name();
        s.push_str(&format!("Circuit Element = {name}\n"));
        for &shunt in &node.shunts {
            let full = format!(
                "{}.{}",
                classes[shunt.class_ord()].props.class_name(),
                classes[shunt.class_ord()].arena[shunt.index()]
                    .data()
                    .name()
            );
            s.push_str(&format!("   Shunt Element = {full}\n"));
        }
    }
    s
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
    // A 0.15.x-only (`HIDE_015X`) or r4133-only (`HIDE_R4133`) property deferred
    // from the 0.14.5-pinned Dump goldens. Skipped from the full-enumeration
    // dump; the `?` query still surfaces it.
    if cx.cls.prop(i).flags.hidden_from_full_enum() {
        return;
    }
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
        // (`bus_ref == None`, e.g. a disabled element — `ReprocessBusDefs` resolves
        // refs only for enabled elements) is Pascal `BusRef = -1` (`Terminal.pas:41`,
        // "signify not set"), which `IntToStr` renders `-1` — NOT 0.
        let br: i64 = cd
            .terminals
            .get(t)
            .and_then(|term| term.bus_ref)
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
/// element kind. `plain TDSSObject` (`try_ckt_elem() == None`, e.g. XYcurve,
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
    arena: &crate::obj::arena::ClassArena,
    idx: usize,
    complete: bool,
    is_pc: bool,
) {
    let obj = arena.obj(idx);
    let full_name = format!("{}.{}", cx.cls.class_name(), obj.data().name());
    match arena.try_ckt_elem(idx) {
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
/// Takes the object's `ClassArena` mutably because one override —
/// `TLineGeometryObj` — walks its conductors by mutating `ActiveCond` (Pascal
/// `ActiveCond := j; GetPropertyValue(3..7)`, `LineGeometry.pas:669`); the other
/// overrides and the generic base read through a shared reborrow.
pub(crate) fn dump_object(
    out: &mut String,
    cx: &DumpCtx,
    arena: &mut crate::obj::arena::ClassArena,
    idx: usize,
    complete: bool,
    is_pc: bool,
) {
    if overrides::dump_override(out, cx, arena, idx, complete) {
        return;
    }
    dump_generic(out, cx, arena, idx, complete, is_pc);
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

/// The shared prefix an override inherits from a **`TPCElement`** base
/// (`Leaf=FALSE`): header + `! ENABLED` + (Complete) the Y/terminal block +
/// `! VARIABLES`. Used by the PC overrides (VSource, UPFC) — `TPCElement.
/// DumpProperties` prints `! VARIABLES` itself (unlike the plain CktElement
/// base), so it is folded into this prefix rather than the generic-path
/// `pc_variables` call site.
pub(crate) fn prefix_pc(
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
        pc_variables(out, cx.variables);
    }
}
