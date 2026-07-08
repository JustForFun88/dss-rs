//! The per-meter zone walk — Pascal `TEnergyMeterObj.MakeMeterZoneLists` (the
//! branch-tree build, voltbase list, zone PD/PC collection, feeder ends) and
//! `TotalUpDownstreamCustomers`, with the zone-eligibility predicates and
//! `AddToVoltBaseList`. Split out of `zones/mod.rs` (no behavioral change).

use crate::circuit::Circuit;
use crate::circuit::ckt_tree::{BusAdjLists, CktTree, NO_BUS};
use crate::elements::ckt::ElemFlags;
use crate::elements::meter::energymeter::{EnergyMeter, NUM_EM_VBASE};
use crate::elements::pc::generator::Generator;
use crate::elements::pc::load::Load;
use crate::elements::pc::pvsystem::PVSystem;
use crate::elements::pc::storage::Storage;
use crate::elements::pd::auto_trans::AutoTrans;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::line::Line;
use crate::elements::pd::reactor::Reactor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{ElemRef, ElemStore};
use crate::support::line_units::{LineUnits, convert_line_units};

use super::super::downcast_meter;

/// Whether the element at `r` is a Line (Pascal `IsLineElement`).
fn is_line(store: &dyn ElemStore, r: ElemRef) -> bool {
    store.obj(r).as_any().downcast_ref::<Line>().is_some()
}

/// Whether the element at `r` is one of the zone-eligible shunt PC element
/// types (Pascal `PCElementType in {LOAD,GEN,PVSYSTEM,STORAGE,CAP,REACTOR}` —
/// `EnergyMeter.pas:1911`). Shunt capacitors/reactors reach the PC adjacency
/// list via `is_shunt()`.
fn is_zone_pce(store: &dyn ElemStore, r: ElemRef) -> bool {
    let any = store.obj(r).as_any();
    any.downcast_ref::<Load>().is_some()
        || any.downcast_ref::<Generator>().is_some()
        || any.downcast_ref::<PVSystem>().is_some()
        || any.downcast_ref::<Storage>().is_some()
        || any.downcast_ref::<Capacitor>().is_some()
        || any.downcast_ref::<Reactor>().is_some()
}

/// Whether the element at `r` is a Power Delivery element (Pascal
/// `(DSSObjType and BaseClassMask) = PD_ELEMENT`), used by the manual
/// `ZoneList` filter.
fn is_pd_element(store: &dyn ElemStore, r: ElemRef) -> bool {
    let any = store.obj(r).as_any();
    any.downcast_ref::<Line>().is_some()
        || any.downcast_ref::<Transformer>().is_some()
        || any.downcast_ref::<AutoTrans>().is_some()
        || any.downcast_ref::<Capacitor>().is_some()
        || any.downcast_ref::<Reactor>().is_some()
}

/// Pascal `CheckParallel`: two lines share both terminal buses (in either
/// orientation).
fn check_parallel(store: &dyn ElemStore, a: ElemRef, b: ElemRef) -> bool {
    let ta = &store.ckt_elem(a).cd().terminals;
    let tb = &store.ckt_elem(b).cd().terminals;
    if ta.len() < 2 || tb.len() < 2 {
        return false;
    }
    (ta[0].bus_ref == tb[0].bus_ref && ta[1].bus_ref == tb[1].bus_ref)
        || (ta[1].bus_ref == tb[0].bus_ref && ta[0].bus_ref == tb[1].bus_ref)
}

/// Pascal `TEnergyMeterObj.AddToVoltBaseList`: add `bus`'s base kV to the list
/// if not already present (< 1% relative difference) and return its 1-based
/// slot, or 0.
fn add_to_volt_base_list(
    bus_ref: usize,
    vbase_list: &mut [f64],
    vbase_count: &mut usize,
    ckt: &Circuit,
) -> i32 {
    let kv_base = ckt.buses[bus_ref].kv_base;
    for (i, &vb) in vbase_list.iter().take(*vbase_count).enumerate() {
        if (1.0 - kv_base / vb).abs() < 0.01 {
            return (i + 1) as i32;
        }
    }
    if kv_base > 0.0 && *vbase_count < vbase_list.len() {
        vbase_list[*vbase_count] = kv_base;
        *vbase_count += 1;
        *vbase_count as i32
    } else {
        0
    }
}

/// Pascal `TEnergyMeterObj.MakeMeterZoneLists` (l.1773): walk outward from the
/// metered element's far terminal, collecting branches (the `BranchList`
/// `CktTree`), zone loads/generators (shunt objects), and feeder ends.
pub(super) fn make_meter_zone_lists(
    meter_ref: ElemRef,
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    adj: &BusAdjLists,
) {
    // Peek the meter's parse-time state.
    let (enabled, metered_element, metered_terminal, defined_zone_list, assume_restoration) = {
        let em = store
            .obj(meter_ref)
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter objects");
        (
            em.enabled(),
            em.metered_element(),
            em.metered_terminal() as usize,
            em.defined_zone_list().to_vec(),
            em.assume_restoration(),
        )
    };
    // `DefinedZoneList.Count = 0` selects the automatic (connectivity) walk;
    // otherwise the zone is the manually-specified element chain.
    let manual_zone = !defined_zone_list.is_empty();

    // `MaxVBaseCount = (NumEMRegisters - VBaseStart) div 5 = NumEMVbase`.
    let mut vbase_list = vec![0.0; NUM_EM_VBASE];
    let mut vbase_count = 0usize;

    if !enabled {
        let em = downcast_meter(store, meter_ref);
        em.install_zone(
            None,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vbase_list,
            0,
        );
        return;
    }

    let Some(metered) = metered_element else {
        // Pascal: `BranchList := TCktTree.Create` then `DoSimpleMsg 527; Exit`,
        // leaving a non-nil but empty BranchList. (The 527 text is the same
        // "Circuit Element not set" already surfaced by RecalcElementData at
        // edit time; solution-time messages have no sink in this port.)
        let em = downcast_meter(store, meter_ref);
        em.install_zone(
            Some(CktTree::new()),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vbase_list,
            0,
        );
        return;
    };

    let mut tree = CktTree::new();
    let mut sequence_list: Vec<ElemRef> = Vec::new();
    let mut sequence_nodes: Vec<usize> = Vec::new();
    let mut load_list: Vec<ElemRef> = Vec::new();
    let mut zone_ends: Vec<(ElemRef, usize)> = Vec::new();

    tree.add(metered);

    // The metered element acquires this meter as its sensor + meter; the head
    // bus is the zone origin (DistFromMeter = 0, not a radial bus).
    let from_bus = {
        let cd = store.ckt_elem_mut(metered).cd_mut();
        cd.sensor_obj = Some(meter_ref);
        cd.meter_obj = Some(meter_ref);
        cd.from_terminal = metered_terminal;
        cd.flags.include(ElemFlags::CHECKED);
        cd.flags.exclude(ElemFlags::IS_ISOLATED);
        if metered_terminal >= 1 && metered_terminal <= cd.terminals.len() {
            cd.terminals_checked[metered_terminal - 1] = true;
        }
        cd.terminals
            .get(metered_terminal - 1)
            .map(|t| t.bus_ref)
            .unwrap_or(usize::MAX)
    };
    if from_bus != usize::MAX && from_bus < ckt.buses.len() {
        ckt.buses[from_bus].dist_from_meter = 0.0;
        let vbi = add_to_volt_base_list(from_bus, &mut vbase_list, &mut vbase_count, ckt);
        let node = tree.present_node_mut();
        node.from_bus = from_bus;
        node.from_terminal = metered_terminal;
        node.volt_base_index = vbi;
    }

    // Manual-ZoneList cursor (Pascal `ZoneListCounter`): monotonic across the
    // whole walk; each branch terminal consumes the next valid PD entry.
    let mut zone_list_counter = 0usize;

    // ****************  MAIN LOOP *****************************
    let mut active = Some(metered);
    while let Some(active_ref) = active {
        let node_idx = tree.present.expect("present branch during zone walk");
        sequence_list.push(active_ref);
        sequence_nodes.push(node_idx);

        // Reset the present branch's loop/parallel/dangling flags + voltbase.
        {
            let node = tree.node_mut(node_idx);
            node.is_looped = false;
            node.is_parallel = false;
            node.is_dangling = true;
        }
        let node_from_bus = tree.node(node_idx).from_bus;
        if node_from_bus != usize::MAX && node_from_bus < ckt.buses.len() {
            let vbi = add_to_volt_base_list(node_from_bus, &mut vbase_list, &mut vbase_count, ckt);
            tree.node_mut(node_idx).volt_base_index = vbi;
        }

        // Read the active branch's shape once (counts, buses, line length, the
        // sensor it passes down to its children/shunts).
        let (nterms, term_bus, terminals_checked, active_is_line, len_km, active_sensor) = {
            let cd = store.ckt_elem(active_ref).cd();
            let len_km = if let Some(line) = store.obj(active_ref).as_any().downcast_ref::<Line>() {
                line.len * convert_line_units(line.length_units, LineUnits::Km)
            } else {
                0.0
            };
            (
                cd.nterms,
                cd.terminals.iter().map(|t| t.bus_ref).collect::<Vec<_>>(),
                cd.terminals_checked.clone(),
                store
                    .obj(active_ref)
                    .as_any()
                    .downcast_ref::<Line>()
                    .is_some(),
                len_km,
                cd.sensor_obj,
            )
        };

        let mut branch_num_customers = 0i32;

        for iterm in 1..=nterms {
            if terminals_checked.get(iterm - 1).copied().unwrap_or(true) {
                continue;
            }
            let test_bus = term_bus[iterm - 1];
            if test_bus >= ckt.buses.len() {
                continue;
            }

            // Record the "to" bus and propagate DistFromMeter. A manual-zone
            // child has `from_bus = NO_BUS` (Pascal ground bus 0, DistFromMeter
            // 0), so treat an unset origin as zero distance.
            tree.node_mut(node_idx).add_to_bus_reference(test_bus);
            let base_dist = if node_from_bus < ckt.buses.len() {
                ckt.buses[node_from_bus].dist_from_meter
            } else {
                0.0
            };
            ckt.buses[test_bus].dist_from_meter = if active_is_line {
                base_dist + len_km
            } else {
                base_dist
            };

            // --- PC elements (loads/gens/shunt caps/reactors) at this bus -----
            for &pc_ref in &adj.pc[test_bus] {
                let checked = store
                    .ckt_elem(pc_ref)
                    .cd()
                    .flags
                    .contains(ElemFlags::CHECKED);
                if checked {
                    continue;
                }
                // Something is connected here regardless of its type.
                tree.node_mut(node_idx).is_dangling = false;
                // Pascal gates the add/check on the PCElementType allow-list
                // (LOAD/GEN/PVSYSTEM/STORAGE/CAP/REACTOR). The adjacency list
                // may hold any enabled PC element, so re-check the type here.
                if !is_zone_pce(store, pc_ref) {
                    continue;
                }
                tree.add_new_object(pc_ref);
                // Count customers if it is a load, and add to the load list.
                if let Some(load) = store.obj(pc_ref).as_any().downcast_ref::<Load>() {
                    branch_num_customers += load.num_customers;
                    load_list.push(pc_ref);
                }
                let cd = store.ckt_elem_mut(pc_ref).cd_mut();
                cd.flags.include(ElemFlags::CHECKED);
                cd.flags.exclude(ElemFlags::IS_ISOLATED);
                cd.active_terminal = 0; // Pascal ActiveTerminalIdx := 1 (1-based)
                if !cd.flags.contains(ElemFlags::HAS_SENSOR_OBJ) {
                    cd.sensor_obj = active_sensor;
                }
                cd.meter_obj = Some(meter_ref);
            }

            // --- PD branches at this bus (automatic zone) ---------------------
            if !manual_zone {
                let mut is_feeder_end = true;
                for &test_ref in &adj.pd[test_bus] {
                    if test_ref == active_ref {
                        continue;
                    }
                    if store
                        .ckt_elem(test_ref)
                        .cd()
                        .flags
                        .contains(ElemFlags::HAS_ENERGY_METER)
                    {
                        continue; // stop at other meters
                    }
                    let (test_nterms, test_bus_refs) = {
                        let cd = store.ckt_elem(test_ref).cd();
                        (
                            cd.nterms,
                            cd.terminals.iter().map(|t| t.bus_ref).collect::<Vec<_>>(),
                        )
                    };
                    for j in 1..=test_nterms {
                        if test_bus != test_bus_refs[j - 1] {
                            continue;
                        }
                        tree.node_mut(node_idx).is_dangling = false;
                        let test_checked = store
                            .ckt_elem(test_ref)
                            .cd()
                            .flags
                            .contains(ElemFlags::CHECKED);
                        if test_checked {
                            // Already on a meter's list → this is a loop.
                            let parallel = active_is_line
                                && is_line(store, test_ref)
                                && check_parallel(store, active_ref, test_ref);
                            let node = tree.node_mut(node_idx);
                            node.is_looped = true;
                            node.loop_elem = Some(test_ref);
                            if parallel {
                                node.is_parallel = true;
                            }
                        } else {
                            is_feeder_end = false;
                            tree.add_new_child(test_ref, test_bus, j);
                            let cd = store.ckt_elem_mut(test_ref).cd_mut();
                            if j <= cd.terminals_checked.len() {
                                cd.terminals_checked[j - 1] = true;
                            }
                            cd.from_terminal = j;
                            cd.flags.include(ElemFlags::CHECKED);
                            cd.flags.exclude(ElemFlags::IS_ISOLATED);
                            if !cd.flags.contains(ElemFlags::HAS_SENSOR_OBJ) {
                                cd.sensor_obj = active_sensor;
                            }
                            cd.meter_obj = Some(meter_ref);
                            cd.parent_pd = Some(active_ref);
                            break;
                        }
                    }
                }
                if is_feeder_end {
                    tree.zone_ends.add(node_idx, test_bus);
                    zone_ends.push((active_ref, test_bus));
                }
            } else {
                // Zone is manually specified: just add the next valid PD element
                // in the list as a child of the present branch (Pascal l.1987).
                // No connectivity, flags, or reductions — "Can't do reductions
                // if manually spec'd". `zone_list_counter` advances past
                // not-found / disabled / non-PD entries.
                zone_list_counter += 1;
                while zone_list_counter <= defined_zone_list.len() {
                    let name = &defined_zone_list[zone_list_counter - 1];
                    match store.find_ckt_element(name) {
                        None => zone_list_counter += 1, // not found; search next
                        Some(test_ref) => {
                            let enabled = store.ckt_elem(test_ref).cd().enabled;
                            if !enabled || !is_pd_element(store, test_ref) {
                                zone_list_counter += 1; // ignore disabled / non-PD
                            } else {
                                tree.add_new_child(test_ref, NO_BUS, 0);
                            }
                            break;
                        }
                    }
                }
            }
        }

        // Write the accumulated customer count onto the branch.
        store.ckt_elem_mut(active_ref).cd_mut().branch_num_customers = branch_num_customers;

        active = tree.go_forward();
    }
    // ****************  END MAIN LOOP *****************************

    // Pascal `TotalUpDownstreamCustomers`: backward sweep summing customers.
    total_up_downstream_customers(&sequence_list, assume_restoration, store);

    // `GetPCEatZone`: the zone PC elements in BranchList order.
    let mut zone_pce: Vec<ElemRef> = Vec::new();
    for &n in &sequence_nodes {
        for &shunt in &tree.node(n).shunts {
            zone_pce.push(shunt);
        }
    }

    let em = downcast_meter(store, meter_ref);
    em.install_zone(
        Some(tree),
        sequence_list,
        sequence_nodes,
        load_list,
        zone_ends,
        zone_pce,
        vbase_list,
        vbase_count,
    );
}

/// Pascal `TEnergyMeterObj.TotalUpDownstreamCustomers` (l.1693): backward sweep
/// over the sequence list (end branches first) summing `BranchNumCustomers`
/// into `BranchTotalCustomers` and up each parent link.
fn total_up_downstream_customers(
    sequence_list: &[ElemRef],
    assume_restoration: bool,
    store: &mut dyn ElemStore,
) {
    // Init totals + clear the Checked flag.
    for &r in sequence_list {
        let cd = store.ckt_elem_mut(r).cd_mut();
        cd.flags.exclude(ElemFlags::CHECKED);
        cd.branch_total_customers = 0;
    }
    for &r in sequence_list.iter().rev() {
        let (already, num, parent, total, has_ocp, has_auto) = {
            let cd = store.ckt_elem(r).cd();
            (
                cd.flags.contains(ElemFlags::CHECKED),
                cd.branch_num_customers,
                cd.parent_pd,
                cd.branch_total_customers,
                cd.flags.contains(ElemFlags::HAS_OCP_DEVICE),
                cd.flags.contains(ElemFlags::HAS_AUTO_OCP_DEVICE),
            )
        };
        if already {
            continue;
        }
        let new_total = total + num;
        {
            let cd = store.ckt_elem_mut(r).cd_mut();
            cd.flags.include(ElemFlags::CHECKED);
            cd.branch_total_customers = new_total;
        }
        // Roll up into the parent unless this is an automatic OCP device and we
        // are assuming restoration (then downstream customers are restored and
        // not counted upstream). OCP devices are Phase 7, so `has_ocp` is never
        // set today and this always rolls up — but the guard is now faithful.
        if let Some(p) = parent
            && !(has_ocp && assume_restoration && has_auto)
        {
            store.ckt_elem_mut(p).cd_mut().branch_total_customers += new_total;
        }
    }
}
