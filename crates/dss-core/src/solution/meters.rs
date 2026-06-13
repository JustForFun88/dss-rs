//! EnergyMeter class-level zone building — Pascal `TEnergyMeter.ResetMeterZonesAll`
//! / `SetHasMeterFlag` / `TEnergyMeterObj.MakeMeterZoneLists` plus
//! `Circuit.DoResetMeterZones`. Implemented as free functions over the registry
//! (the WP5.7 control-sweep pattern): the meter's zone topology is built in
//! local data structures while the **other** circuit elements' flags/refs and
//! the buses' `DistFromMeter` are mutated through the store, then the result is
//! installed into the meter object.

use crate::circuit::Circuit;
use crate::circuit::ckt_tree::{BusAdjLists, CktTree, build_active_bus_adjacency_lists};
use crate::elements::ckt::ElemFlags;
use crate::elements::meter::energymeter::{EnergyMeter, NUM_EM_VBASE};
use crate::elements::pd::line::Line;
use crate::elements::traits::{ElemRef, ElemStore};
use crate::support::line_units::{LineUnits, convert_line_units};

/// Pascal `TDSSCircuit.DoResetMeterZones` (Circuit.pas l.2145): rebuild every
/// meter's zone whenever the bus lists were rebuilt, unless the zones are
/// locked. With `zones_locked = false` (the default) the zones are rebuilt on
/// every Y-build that reprocessed the bus definitions.
pub(crate) fn do_reset_meter_zones(ckt: &mut Circuit, store: &mut dyn ElemStore) {
    if !ckt.meter_zones_computed || !ckt.zones_locked {
        reset_meter_zones_all(ckt, store);
        ckt.meter_zones_computed = true;
    }
    // Pascal `FreeTopology` (the whole-circuit GetTopology tree) — not built in
    // this port until a topology-API consumer needs it.
}

/// Pascal `TEnergyMeter.ResetMeterZonesAll` (l.797): clear the topology flags
/// and zone refs on every element, build the bus adjacency lists, mark the
/// metered elements, and rebuild each meter's zone in creation order.
fn reset_meter_zones_all(ckt: &mut Circuit, store: &mut dyn ElemStore) {
    if ckt.energy_meters.is_empty() {
        return;
    }

    // Initialize the Checked/IsIsolated flags and TerminalsChecked for every
    // circuit element.
    for &r in &ckt.ckt_elements {
        let cd = store.ckt_elem_mut(r).cd_mut();
        cd.flags.exclude(ElemFlags::CHECKED);
        cd.flags.include(ElemFlags::IS_ISOLATED);
        for c in &mut cd.terminals_checked {
            *c = false;
        }
    }

    // Clear meter/sensor/parent refs that the zone build sets.
    for &r in &ckt.pd_elements {
        let cd = store.ckt_elem_mut(r).cd_mut();
        cd.meter_obj = None;
        cd.sensor_obj = None;
        cd.parent_pd = None;
    }
    for &r in &ckt.pc_elements {
        let cd = store.ckt_elem_mut(r).cd_mut();
        cd.meter_obj = None;
        cd.sensor_obj = None;
    }

    // Bus adjacency lists for fast zone searches.
    let adj = build_active_bus_adjacency_lists(ckt, store);

    // Set the HasEnergyMeter flag on each metered element.
    set_has_meter_flag(ckt, store);
    // Pascal also calls `SensorClass.SetHasSensorFlag` here — Sensor is WP6.7.

    for bus in &mut ckt.buses {
        bus.bus_checked = false;
    }

    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        make_meter_zone_lists(meter_ref, ckt, store, &adj);
    }
    // `adj` is dropped here (Pascal `FreeAndNilBusAdjacencyLists`).
}

/// Pascal `TEnergyMeter.SetHasMeterFlag` (l.1752): clear `HasEnergyMeter` on all
/// PD elements, then set it on each enabled meter's metered element.
fn set_has_meter_flag(ckt: &Circuit, store: &mut dyn ElemStore) {
    for &r in &ckt.pd_elements {
        store
            .ckt_elem_mut(r)
            .cd_mut()
            .flags
            .exclude(ElemFlags::HAS_ENERGY_METER);
    }
    for &meter_ref in &ckt.energy_meters {
        let (enabled, metered) = {
            let em = store
                .obj(meter_ref)
                .as_any()
                .downcast_ref::<EnergyMeter>()
                .expect("energy_meters holds EnergyMeter objects");
            (em.enabled(), em.metered_element())
        };
        if enabled && let Some(mr) = metered {
            store
                .ckt_elem_mut(mr)
                .cd_mut()
                .flags
                .include(ElemFlags::HAS_ENERGY_METER);
        }
    }
}

/// Whether the element at `r` is a Line (Pascal `IsLineElement`).
fn is_line(store: &dyn ElemStore, r: ElemRef) -> bool {
    store.obj(r).as_any().downcast_ref::<Line>().is_some()
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
fn make_meter_zone_lists(
    meter_ref: ElemRef,
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    adj: &BusAdjLists,
) {
    // Peek the meter's parse-time state.
    let (enabled, metered_element, metered_terminal, manual_zone) = {
        let em = store
            .obj(meter_ref)
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter objects");
        (
            em.enabled(),
            em.metered_element(),
            em.metered_terminal() as usize,
            !em.defined_zone_list().is_empty(),
        )
    };

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
        // Pascal DoSimpleMsg 527; leave an empty zone.
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

            // Record the "to" bus and propagate DistFromMeter.
            tree.node_mut(node_idx).add_to_bus_reference(test_bus);
            let base_dist = ckt.buses[node_from_bus.min(ckt.buses.len() - 1)].dist_from_meter;
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
                tree.node_mut(node_idx).is_dangling = false;
                tree.add_new_object(pc_ref);
                // Count customers if it is a load, and add to the load list.
                if let Some(load) = store
                    .obj(pc_ref)
                    .as_any()
                    .downcast_ref::<crate::elements::pc::load::Load>()
                {
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
            }
            // (Manual ZoneList zone-building is deferred — see module note.)
        }

        // Write the accumulated customer count onto the branch.
        store.ckt_elem_mut(active_ref).cd_mut().branch_num_customers = branch_num_customers;

        active = tree.go_forward();
    }
    // ****************  END MAIN LOOP *****************************

    // Pascal `TotalUpDownstreamCustomers`: backward sweep summing customers.
    total_up_downstream_customers(&sequence_list, store);

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
fn total_up_downstream_customers(sequence_list: &[ElemRef], store: &mut dyn ElemStore) {
    // Init totals + clear the Checked flag.
    for &r in sequence_list {
        let cd = store.ckt_elem_mut(r).cd_mut();
        cd.flags.exclude(ElemFlags::CHECKED);
        cd.branch_total_customers = 0;
    }
    for &r in sequence_list.iter().rev() {
        let (already, num, parent, total) = {
            let cd = store.ckt_elem(r).cd();
            (
                cd.flags.contains(ElemFlags::CHECKED),
                cd.branch_num_customers,
                cd.parent_pd,
                cd.branch_total_customers,
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
        // Phase 6 has no OCP devices, so the AssumeRestoration guard never
        // trips — always roll up into the parent.
        if let Some(p) = parent {
            store.ckt_elem_mut(p).cd_mut().branch_total_customers += new_total;
        }
    }
}

fn downcast_meter(store: &mut dyn ElemStore, meter_ref: ElemRef) -> &mut EnergyMeter {
    store
        .obj_mut(meter_ref)
        .as_any_mut()
        .downcast_mut::<EnergyMeter>()
        .expect("energy_meters holds EnergyMeter objects")
}
