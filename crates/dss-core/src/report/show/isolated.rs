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
use crate::elements::traits::{ElemId, ElemStore, TypedStore};
use crate::exec::registry::{ClassStore, DssClass};
use crate::report::format::enclose_quotes;
use crate::solution::topology::get_isolated_sub_area;

// BUG(C2): whether an element is a PD element is found by trying each PD type in turn on the
// store, a predicate written again elsewhere; the class kind the handle carries should answer.
/// Pascal `(DSSObjType and BASECLASSMASK) = PD_ELEMENT`.
fn is_pd_element(store: &dyn ElemStore, r: ElemId) -> bool {
    store.typed::<Line>(r).is_some()
        || store.typed::<Transformer>(r).is_some()
        || store.typed::<AutoTrans>(r).is_some()
        || store.typed::<Capacitor>(r).is_some()
        || store.typed::<Reactor>(r).is_some()
}

// BUG(C2): a free function, one of several copies of the element full name in the reports; the
// registry type should name a handle in one method.
/// Pascal `TDSSCktElement.FullName`.
fn full_name(classes: &[DssClass], r: ElemId) -> String {
    format!(
        "{}.{}",
        classes[r.class_ord()].props.class_name(),
        classes[r.class_ord()].arena[r.index()].data().name()
    )
}

// BUG(C2): a free function that walks one `CktTree`; it is a method of `CktTree` that takes
// the registry to name each element.
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
        // BUG(C5): the shunt list of the present node is cloned before the loop, though the
        // loop touches neither the tree nor the node; direct access iterates the list in place.
        let shunts = tree.present_node().shunts.clone();
        for sh in shunts {
            s.push_str(&format!("[SHUNT], {}\n", full_name(classes, sh)));
        }
        br = tree.go_forward();
    }
}

// BUG(C5): the report takes the whole registry and circuit mutably because the topology walk
// it runs keeps its marks in the elements and buses; the marks should be state of the walk,
// so that a report borrows the model shared.
/// Build the `Show Isolated` text (Pascal `ShowIsolated`).
pub(crate) fn show_isolated(classes: &mut [DssClass], ckt: &mut Circuit) -> String {
    // ---- Phase 1: build the trees (mutating CHECKED/bus_checked). ----
    let mut main_tree;
    let mut sub_areas: Vec<CktTree> = Vec::new();
    let bus_checked_after_source: Vec<bool>;
    let bus_checked_final: Vec<bool>;
    let mut isolated_elems: Vec<ElemId> = Vec::new();
    {
        // BUG(C2): the report builds a `ClassStore` from its field and, here and below, resets
        // and sets the walk marks of elements and buses field by field, as the topology walk
        // does again; the owner of the walk should own the reset and the marking.
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

        // BUG(C5): the element list is walked by index and re-read on each pass because the
        // sub-area walk takes the whole circuit mutably while only bus marks change; direct
        // access iterates `&ckt.ckt_elements` with the walk borrowing the buses alone.
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
        let cd = classes[r.class_ord()]
            .arena
            .try_ckt_elem(r.index())
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
