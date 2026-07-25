//! Circuit-wide topology tree (Pascal `Shared/CktTree.pas` `GetIsolatedSubArea` +
//! `Common/Circuit.pas` `GetTopology`). Builds a `CktTree` rooted at a start element
//! by walking the bus-adjacency lists, attaching child branches / shunt objects and
//! marking loop/parallel branches — the same connectivity walk
//! [`make_meter_zone_lists`](super::meters) performs per meter, but circuit-wide
//! (from the source) and without the meter's voltbase/sensor/dist machinery.
//!
//! Unlike the read-only report formatters, this **mutates** element flags
//! (`CHECKED`/`IS_ISOLATED`/`terminals_checked`) and bus `bus_checked` as a side
//! effect (exactly as Pascal does). The `Show Isolated`/`Show Topology` reports call
//! it, so those two reports are not read-only over the circuit.

use crate::circuit::Circuit;
use crate::circuit::ckt_tree::{
    BusAdjLists, CktTree, NO_BUS, all_terminals_closed, build_active_bus_adjacency_lists,
};
use crate::elements::ckt::ElemFlags;
use crate::elements::pd::line::Line;
use crate::elements::traits::{ElemRef, ElemStore};

/// Whether the element at `r` is a Line (Pascal `IsLineElement`).
fn is_line(store: &dyn ElemStore, r: ElemRef) -> bool {
    store.obj(r).as_any().downcast_ref::<Line>().is_some()
}

/// Pascal `CheckParallel`: two lines share both terminal buses (either orientation).
fn check_parallel(store: &dyn ElemStore, a: ElemRef, b: ElemRef) -> bool {
    let bus = |r: ElemRef, t: usize| store.ckt_elem(r).cd().terminals.get(t).map(|x| x.bus_ref);
    let (a1, a2) = (bus(a, 0), bus(a, 1));
    let (b1, b2) = (bus(b, 0), bus(b, 1));
    (a1 == b1 && a2 == b2) || (a1 == b2 && a2 == b1)
}

/// Pascal `GetSourcesConnectedToBus`: attach every enabled source whose terminal-1
/// bus is `bus_num` as a shunt object; clear `IS_ISOLATED` + `IsDangling` when
/// analysing.
fn get_sources_connected_to_bus(
    ckt: &Circuit,
    store: &mut dyn ElemStore,
    bus_num: usize,
    tree: &mut CktTree,
    analyze: bool,
) {
    for &src in &ckt.sources {
        let (enabled, checked, on_bus) = {
            let cd = store.ckt_elem(src).cd();
            (
                cd.enabled,
                cd.flags.contains(ElemFlags::CHECKED),
                cd.terminals.first().and_then(|t| t.bus_ref) == Some(bus_num),
            )
        };
        if !enabled {
            continue;
        }
        if (analyze || !checked) && on_bus {
            if analyze {
                store
                    .ckt_elem_mut(src)
                    .cd_mut()
                    .flags
                    .exclude(ElemFlags::IS_ISOLATED);
                tree.present_node_mut().is_dangling = false;
            }
            if !checked {
                tree.add_new_object(src);
                store
                    .ckt_elem_mut(src)
                    .cd_mut()
                    .flags
                    .include(ElemFlags::CHECKED);
            }
        }
    }
}

/// Pascal `GetPCElementsConnectedToBus`: attach every enabled element in the bus's
/// PC adjacency list (PC elements **and** shunt PD elements) as a shunt object.
fn get_pc_elements_connected_to_bus(
    adj_lst: &[ElemRef],
    store: &mut dyn ElemStore,
    tree: &mut CktTree,
    analyze: bool,
) {
    for &p in adj_lst {
        if !store.ckt_elem(p).cd().enabled {
            continue;
        }
        if analyze {
            store
                .ckt_elem_mut(p)
                .cd_mut()
                .flags
                .exclude(ElemFlags::IS_ISOLATED);
            tree.present_node_mut().is_dangling = false;
        }
        if !store.ckt_elem(p).cd().flags.contains(ElemFlags::CHECKED) {
            tree.add_new_object(p);
            store
                .ckt_elem_mut(p)
                .cd_mut()
                .flags
                .include(ElemFlags::CHECKED);
        }
    }
}

/// Pascal `GetShuntPDElementsConnectedToBus`: attach the bus's shunt PD elements.
/// The Rust `BusAdjLists.pd` holds only **non-shunt** PD (shunt PD is already in
/// `.pc`, handled by [`get_pc_elements_connected_to_bus`]), so this is a faithful
/// no-op — Pascal likewise passes `lstPD` (non-shunt) here and finds nothing.
fn get_shunt_pd_elements_connected_to_bus(
    adj_lst: &[ElemRef],
    store: &mut dyn ElemStore,
    tree: &mut CktTree,
    analyze: bool,
) {
    for &p in adj_lst {
        let is_shunt = store.ckt_elem(p).is_shunt();
        if !store.ckt_elem(p).cd().enabled || !is_shunt {
            continue;
        }
        if analyze {
            store
                .ckt_elem_mut(p)
                .cd_mut()
                .flags
                .exclude(ElemFlags::IS_ISOLATED);
            tree.present_node_mut().is_dangling = false;
        }
        if !store.ckt_elem(p).cd().flags.contains(ElemFlags::CHECKED) {
            tree.add_new_object(p);
            store
                .ckt_elem_mut(p)
                .cd_mut()
                .flags
                .include(ElemFlags::CHECKED);
        }
    }
}

/// Pascal `FindAllChildBranches`: for every enabled non-shunt PD branch in the bus's
/// PD adjacency list (other than the active branch) with all terminals closed, add
/// it as a child branch — marking `IsLoopedHere`/`IsParallel` when it was already
/// checked (a loop closing back into the tree).
fn find_all_child_branches(
    adj_lst: &[ElemRef],
    bus_num: usize,
    store: &mut dyn ElemStore,
    tree: &mut CktTree,
    analyze: bool,
    active_branch: ElemRef,
) {
    for &p in adj_lst {
        if p == active_branch || !store.ckt_elem(p).cd().enabled {
            continue;
        }
        let checked = store.ckt_elem(p).cd().flags.contains(ElemFlags::CHECKED);
        if !analyze && checked {
            continue;
        }
        let is_shunt = store.ckt_elem(p).is_shunt();
        if is_shunt || !all_terminals_closed(store.ckt_elem(p)) {
            continue;
        }
        let nterms = store.ckt_elem(p).cd().nterms;
        for j in 1..=nterms {
            let on_bus = store.ckt_elem(p).cd().terminals[j - 1].bus_ref == Some(bus_num);
            if !on_bus {
                continue;
            }
            if analyze {
                store
                    .ckt_elem_mut(p)
                    .cd_mut()
                    .flags
                    .exclude(ElemFlags::IS_ISOLATED);
                tree.present_node_mut().is_dangling = false;
                if checked && tree.level() > 0 {
                    let node = tree.present_node_mut();
                    node.is_looped = true;
                    node.loop_elem = Some(p);
                    if is_line(store, p)
                        && is_line(store, active_branch)
                        && check_parallel(store, active_branch, p)
                    {
                        tree.present_node_mut().is_parallel = true;
                    }
                }
            }
            if !checked {
                tree.add_new_child(p, bus_num, j);
                store.ckt_elem_mut(p).cd_mut().terminals_checked[j - 1] = true;
                store
                    .ckt_elem_mut(p)
                    .cd_mut()
                    .flags
                    .include(ElemFlags::CHECKED);
                break;
            }
        }
    }
}

/// Pascal `GetIsolatedSubArea` (`CktTree.pas:624`): build a `CktTree` rooted at
/// `start`, walking the bus-adjacency lists breadth-first. `analyze` clears
/// `IS_ISOLATED` on every reached element (used by `GetTopology`; `ShowIsolated`
/// passes `false` for the sub-area probes and `true` for the main source tree).
pub(crate) fn get_isolated_sub_area(
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    adj: &BusAdjLists,
    start: ElemRef,
    analyze: bool,
) -> CktTree {
    let mut tree = CktTree::new();
    tree.add(start);
    if analyze {
        store
            .ckt_elem_mut(start)
            .cd_mut()
            .flags
            .exclude(ElemFlags::IS_ISOLATED);
    }
    store
        .ckt_elem_mut(start)
        .cd_mut()
        .flags
        .include(ElemFlags::CHECKED);

    let mut active = Some(start);
    while let Some(branch) = active {
        let nterms = store.ckt_elem(branch).cd().nterms;
        for iterm in 1..=nterms {
            let (checked, bus) = {
                let cd = store.ckt_elem(branch).cd();
                (
                    cd.terminals_checked[iterm - 1],
                    // Unwired terminal → the `NO_BUS` sentinel the walk skips below.
                    cd.terminals[iterm - 1].bus_ref.unwrap_or(NO_BUS),
                )
            };
            if checked {
                continue;
            }
            // Pascal `BranchList.PresentBranch.ToBusReference := TestBusNum`
            // (`CktTree.pas:662`) — recorded on every unchecked terminal, before the
            // `TestBusNum > 0` guard. Inert for the current reports (they don't read
            // it), kept for walk-fidelity with `make_meter_zone_lists`.
            tree.present_node_mut().add_to_bus_reference(bus);
            if bus == NO_BUS || bus >= ckt.buses.len() {
                continue;
            }
            ckt.buses[bus].bus_checked = true;
            get_sources_connected_to_bus(ckt, store, bus, &mut tree, analyze);
            let pc = adj.pc.get(bus).map(|v| v.as_slice()).unwrap_or(&[]);
            get_pc_elements_connected_to_bus(pc, store, &mut tree, analyze);
            let pd = adj.pd.get(bus).map(|v| v.as_slice()).unwrap_or(&[]);
            get_shunt_pd_elements_connected_to_bus(pd, store, &mut tree, analyze);
            find_all_child_branches(pd, bus, store, &mut tree, analyze, branch);
        }
        active = tree.go_forward();
    }
    tree
}

/// Pascal `TDSSCircuit.GetTopology` (`Circuit.pas:3034`): reset every element's
/// `CHECKED`/`terminals_checked` + set `IS_ISOLATED` (till proven otherwise) and
/// every bus's `bus_checked`, then build the analysing sub-area tree from the first
/// source. Returns a fresh tree each call (the port does not cache `Branch_List`
/// on the circuit — the reports are the only consumer and a rebuild is
/// observationally identical). Returns an empty tree if the circuit has no source.
pub(crate) fn get_topology(ckt: &mut Circuit, store: &mut dyn ElemStore) -> CktTree {
    for &r in &ckt.ckt_elements {
        let cd = store.ckt_elem_mut(r).cd_mut();
        cd.flags.exclude(ElemFlags::CHECKED);
        for c in cd.terminals_checked.iter_mut() {
            *c = false;
        }
        cd.flags.include(ElemFlags::IS_ISOLATED);
    }
    for b in ckt.buses.iter_mut() {
        b.bus_checked = false;
    }
    let Some(&source) = ckt.sources.first() else {
        return CktTree::new();
    };
    let adj = build_active_bus_adjacency_lists(ckt, store);
    get_isolated_sub_area(ckt, store, &adj, source, true)
}
