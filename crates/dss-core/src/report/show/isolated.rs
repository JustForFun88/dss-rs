//! `Show Isolated` (Pascal `ShowResults.pas` `ShowIsolated`): the isolated
//! buses/branches report — buses/elements not connected to the source, the isolated
//! sub-networks, and the connected element tree. Builds the source tree + one
//! sub-area tree per unreached PD element via
//! [`get_isolated_sub_area`](crate::solution::topology) (`analyze = false`, so it
//! marks `CHECKED`/`bus_checked` but not `IS_ISOLATED`), then walks them.

use crate::circuit::Circuit;
use crate::circuit::ckt_tree::{CktTree, build_active_bus_adjacency_lists};
use crate::elements::ckt::ElemFlags;
use crate::elements::pd::auto_trans::AutoTrans;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::line::Line;
use crate::elements::pd::reactor::Reactor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{ElemRef, ElemStore};
use crate::exec::registry::{ClassStore, DssClass};
use crate::report::format::enclose_quotes;
use crate::solution::topology::get_isolated_sub_area;

/// Pascal `(DSSObjType and BASECLASSMASK) = PD_ELEMENT`.
fn is_pd_element(store: &dyn ElemStore, r: ElemRef) -> bool {
    let any = store.obj(r).as_any();
    any.is::<Line>()
        || any.is::<Transformer>()
        || any.is::<AutoTrans>()
        || any.is::<Capacitor>()
        || any.is::<Reactor>()
}

/// Pascal `TDSSCktElement.FullName`.
fn full_name(classes: &[DssClass], r: ElemRef) -> String {
    format!(
        "{}.{}",
        classes[r.cls].props.class_name(),
        classes[r.cls].objects[r.idx].data().name()
    )
}

/// Write a `CktTree`'s branch/shunt walk into `s` (Pascal's
/// `(Level) FullName` + `[SHUNT], FullName`).
fn write_tree(s: &mut String, classes: &[DssClass], tree: &mut CktTree) {
    let mut br = tree.first();
    while let Some(br_ref) = br {
        s.push_str(&format!(
            "({}) {}\n",
            tree.level(),
            full_name(classes, br_ref)
        ));
        let shunts = tree.present_node().shunts.clone();
        for sh in shunts {
            s.push_str(&format!("[SHUNT], {}\n", full_name(classes, sh)));
        }
        br = tree.go_forward();
    }
}

/// Build the `Show Isolated` text (Pascal `ShowIsolated`).
pub(crate) fn show_isolated(classes: &mut [DssClass], ckt: &mut Circuit) -> String {
    // ---- Phase 1: build the trees (mutating CHECKED/bus_checked). ----
    let mut main_tree;
    let mut sub_areas: Vec<CktTree> = Vec::new();
    let bus_checked_after_source: Vec<bool>;
    let bus_checked_final: Vec<bool>;
    let mut isolated_elems: Vec<ElemRef> = Vec::new();
    {
        let mut store = ClassStore { classes };
        // Reset every element's CHECKED/terminals_checked and every bus's bus_checked
        // (Pascal does NOT set IS_ISOLATED here — that is `ShowTopology`/`GetTopology`).
        for &r in &ckt.ckt_elements {
            let cd = store.ckt_elem_mut(r).cd_mut();
            cd.flags.exclude(ElemFlags::CHECKED);
            for c in cd.terminals_checked.iter_mut() {
                *c = false;
            }
        }
        for b in ckt.buses.iter_mut() {
            b.bus_checked = false;
        }
        let adj = build_active_bus_adjacency_lists(ckt, &store);

        // Main tree from the first source (`analyze = false`).
        main_tree = match ckt.sources.first().copied() {
            Some(src) => get_isolated_sub_area(ckt, &mut store, &adj, src, false),
            None => CktTree::new(),
        };
        bus_checked_after_source = ckt.buses.iter().map(|b| b.bus_checked).collect();

        // One sub-area per **enabled**, still-unchecked PD element, in CktElements
        // order (Pascal `if TestElement.Enabled then if not Checked then if
        // PD_ELEMENT`, `ShowResults.pas:2915-2918` — a disabled PD element is in
        // `ckt_elements` but never in the adjacency lists, so without the `enabled`
        // guard it would emit a spurious `*** START SUBAREA ***` block).
        for i in 0..ckt.ckt_elements.len() {
            let r = ckt.ckt_elements[i];
            let (enabled, checked) = {
                let cd = store.ckt_elem(r).cd();
                (cd.enabled, cd.flags.contains(ElemFlags::CHECKED))
            };
            if enabled && !checked && is_pd_element(&store, r) {
                sub_areas.push(get_isolated_sub_area(ckt, &mut store, &adj, r, false));
            }
        }
        bus_checked_final = ckt.buses.iter().map(|b| b.bus_checked).collect();

        // Mark all controls + meter elements CHECKED so they don't appear in the
        // "enabled isolated elements" list (Pascal marks DSSControls + MeterElements).
        for &cr in &ckt.controls {
            store
                .ckt_elem_mut(cr)
                .cd_mut()
                .flags
                .include(ElemFlags::CHECKED);
        }
        for list in [&ckt.energy_meters, &ckt.monitors, &ckt.sensors] {
            for &mr in list {
                store
                    .ckt_elem_mut(mr)
                    .cd_mut()
                    .flags
                    .include(ElemFlags::CHECKED);
            }
        }
        // Enabled + still-unchecked elements (in CktElements order).
        for i in 0..ckt.ckt_elements.len() {
            let r = ckt.ckt_elements[i];
            let cd = store.ckt_elem(r).cd();
            if cd.enabled && !cd.flags.contains(ElemFlags::CHECKED) {
                isolated_elems.push(r);
            }
        }
    }

    // ---- Phase 2: write the report (reading names from `classes`). ----
    let mut s = String::new();
    s.push('\n');
    s.push_str("ISOLATED CIRCUIT ELEMENT REPORT\n");
    s.push('\n');
    s.push('\n');
    s.push_str("***  THE FOLLOWING BUSES HAVE NO CONNECTION TO THE SOURCE ***\n");
    s.push('\n');
    for (i, &checked) in bus_checked_after_source.iter().enumerate() {
        if !checked {
            s.push_str(&enclose_quotes(ckt.bus_list.name(i).unwrap_or("")));
            s.push('\n');
        }
    }

    s.push('\n');
    s.push_str("***********  THE FOLLOWING SUB NETWORKS ARE ISOLATED ************\n");
    s.push('\n');
    for sub in &mut sub_areas {
        s.push_str("*** START SUBAREA ***\n");
        write_tree(&mut s, classes, sub);
        s.push('\n');
    }

    s.push('\n');
    s.push_str("***********  THE FOLLOWING ENABLED ELEMENTS ARE ISOLATED ************\n");
    s.push('\n');
    for &r in &isolated_elems {
        let cd = classes[r.cls].objects[r.idx]
            .as_ckt_element()
            .map(|e| e.cd());
        if let Some(cd) = cd {
            s.push('"');
            s.push_str(&full_name(classes, r));
            s.push('"');
            s.push_str("  Buses:");
            for j in 1..=cd.nterms {
                s.push_str(&format!("  \"{}\"", cd.get_bus(j)));
            }
            s.push('\n');
        }
    }

    s.push('\n');
    s.push_str("***  THE FOLLOWING BUSES ARE NOT CONNECTED TO ANY POWER DELIVERY ELEMENT ***\n");
    s.push('\n');
    for (i, &checked) in bus_checked_final.iter().enumerate() {
        if !checked {
            s.push_str(&enclose_quotes(ckt.bus_list.name(i).unwrap_or("")));
            s.push('\n');
        }
    }

    s.push('\n');
    s.push_str("***********  CONNECTED CIRCUIT ELEMENT TREE ************\n");
    s.push('\n');
    s.push_str("(Lexical Level) Element name\n");
    s.push('\n');
    write_tree(&mut s, classes, &mut main_tree);

    s
}
