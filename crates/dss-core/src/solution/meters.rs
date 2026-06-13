//! EnergyMeter class-level zone building — Pascal `TEnergyMeter.ResetMeterZonesAll`
//! / `SetHasMeterFlag` / `TEnergyMeterObj.MakeMeterZoneLists` plus
//! `Circuit.DoResetMeterZones`. Implemented as free functions over the registry
//! (the WP5.7 control-sweep pattern): the meter's zone topology is built in
//! local data structures while the **other** circuit elements' flags/refs and
//! the buses' `DistFromMeter` are mutated through the store, then the result is
//! installed into the meter object.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::circuit::ckt_tree::{BusAdjLists, CktTree, NO_BUS, build_active_bus_adjacency_lists};
use crate::elements::ckt::ElemFlags;
use crate::elements::meter::energymeter::{EnergyMeter, NUM_EM_VBASE, reg};
use crate::elements::pc::generator::Generator;
use crate::elements::pc::load::Load;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::line::Line;
use crate::elements::pd::reactor::Reactor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{CktElement, ElemRef, ElemStore, SysCtx};
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

/// Whether the element at `r` is one of the zone-eligible shunt PC element
/// types (Pascal `PCElementType in {LOAD,GEN,PVSYSTEM,STORAGE,CAP,REACTOR}`).
/// Shunt capacitors/reactors reach the PC adjacency list via `is_shunt()`.
// TODO(WP6.8): add PVSystem/Storage here when those PC classes land.
fn is_zone_pce(store: &dyn ElemStore, r: ElemRef) -> bool {
    let any = store.obj(r).as_any();
    any.downcast_ref::<Load>().is_some()
        || any.downcast_ref::<Generator>().is_some()
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
fn make_meter_zone_lists(
    meter_ref: ElemRef,
    ckt: &mut Circuit,
    store: &mut dyn ElemStore,
    adj: &BusAdjLists,
) {
    // Peek the meter's parse-time state.
    let (enabled, metered_element, metered_terminal, defined_zone_list) = {
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

// ---------------------------------------------------------------------------
// TakeSample — Pascal `TEnergyMeter.SampleAll` / `TEnergyMeterObj.TakeSample`
// (EnergyMeter.pas l.900 / l.1289). The meter's branch tree and registers are
// moved out of the meter object for the walk (the store keeps the meter
// borrowed), the zone's PD/PC elements are read/mutated through the store, and
// the registers are written back at the end.
// ---------------------------------------------------------------------------

/// Pascal `TEnergyMeter.ResetAll` (l.851): reset every meter's registers.
/// (Demand-interval files are Phase 8; the `SystemMeter` core and the
/// Generator/Storage/PVSystem `ResetRegistersAll` calls are WP6.8 / later.)
pub(crate) fn reset_all_meters(ckt: &mut Circuit, store: &mut dyn ElemStore) {
    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        downcast_meter(store, meter_ref).reset_registers();
    }
}

/// Pascal `TEnergyMeter.SampleAll` (l.900): sample every enabled meter.
pub(crate) fn take_sample_all(ckt: &mut Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        let enabled = store
            .obj(meter_ref)
            .as_any()
            .downcast_ref::<EnergyMeter>()
            .expect("energy_meters holds EnergyMeter objects")
            .enabled();
        if enabled {
            take_sample_one(meter_ref, ckt, store, sys);
        }
    }
}

/// PD-element kind probe used by the loss split.
fn branch_kind(store: &dyn ElemStore, r: ElemRef) -> (bool, bool, usize) {
    let any = store.obj(r).as_any();
    let is_line = any.downcast_ref::<Line>().is_some();
    let is_xfmr = any.downcast_ref::<Transformer>().is_some();
    let nphases = store.ckt_elem(r).cd().nphases;
    (is_line, is_xfmr, nphases)
}

/// Pascal `TEnergyMeterObj.TakeSample` (l.1289).
fn take_sample_one(meter_ref: ElemRef, ckt: &Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let trapezoidal = ckt.trapezoidal_integration;
    let node_v = &ckt.solution.node_v;
    let delta_hrs = ckt.solution.interval_hrs;
    let norm_min = ckt.normal_min_volts;
    let emerg_min = ckt.emerg_min_volts;

    // CheckBranchList: exit if the zone was never built. Take the tree and the
    // register accumulators out of the meter for the walk.
    let Some((mut tree, mut st)) = downcast_meter(store, meter_ref).begin_take_sample(trapezoidal)
    else {
        return;
    };

    // Energy in the branch the meter is connected to.
    let metered = st
        .metered_element
        .expect("metered element present once zone built");
    let s_local = store
        .ckt_elem_mut(metered)
        .terminal_power(sys, node_v, st.metered_terminal)
        * 0.001;
    let s_local_kva = s_local.norm();
    st.integrate(reg::KWH, s_local.re, delta_hrs);
    st.integrate(reg::KVARH, s_local.im, delta_hrs);
    st.set_drag(reg::MAX_KW, s_local.re);
    st.set_drag(reg::MAX_KVA, s_local_kva);

    // Loss accumulators.
    let mut total_losses = Complex64::ZERO;
    let mut total_load_losses = Complex64::ZERO;
    let mut total_no_load_losses = Complex64::ZERO;
    let mut total_line_losses = Complex64::ZERO;
    let mut total_line_mode_losses = Complex64::ZERO;
    let mut total_zero_mode_losses = Complex64::ZERO;
    let mut total_3phase_losses = Complex64::ZERO;
    let mut total_1phase_losses = Complex64::ZERO;
    let mut total_transformer_losses = Complex64::ZERO;
    let mut vbase_total_losses = [0.0; NUM_EM_VBASE];
    let mut vbase_line_losses = [0.0; NUM_EM_VBASE];
    let mut vbase_load_losses = [0.0; NUM_EM_VBASE];
    let mut vbase_no_load_losses = [0.0; NUM_EM_VBASE];
    let mut vbase_load = [0.0; NUM_EM_VBASE];

    let mut max_excess_kw_norm = 0.0_f64;
    let mut max_excess_kw_emerg = 0.0_f64;

    // ---- EEN/UE overload pass --------------------------------------------
    if st.local_only {
        let elem = store.ckt_elem_mut(metered);
        max_excess_kw_norm = elem
            .excess_kva_norm(sys, node_v, st.metered_terminal)
            .re
            .abs();
        max_excess_kw_emerg = elem
            .excess_kva_emerg(sys, node_v, st.metered_terminal)
            .re
            .abs();
    } else {
        let mut cur = tree.first();
        while let Some(branch) = cur {
            let (from_terminal, shunts) = {
                let n = tree.present_node();
                (n.from_terminal, n.shunts.clone())
            };
            let parent = tree.parent();

            // ExcesskVANorm/Emerg set the PD element's Overload_EEN/UE flags.
            let (een, ue) = {
                let elem = store.ckt_elem_mut(branch);
                let een = elem.excess_kva_norm(sys, node_v, from_terminal).re.abs();
                let ue = elem.excess_kva_emerg(sys, node_v, from_terminal).re.abs();
                (een, ue)
            };
            if st.zone_is_radial {
                if ue > max_excess_kw_emerg {
                    max_excess_kw_emerg = ue;
                }
                if een > max_excess_kw_norm {
                    max_excess_kw_norm = een;
                }
            } else {
                max_excess_kw_emerg += ue;
                max_excess_kw_norm += een;
            }

            // Roll the parent's overload into this branch (use the larger).
            if let Some(p) = parent {
                let (p_een, p_ue) = {
                    let pc = store.ckt_elem(p).cd();
                    (pc.overload_een, pc.overload_ue)
                };
                let cd = store.ckt_elem_mut(branch).cd_mut();
                cd.overload_een = cd.overload_een.max(p_een);
                cd.overload_ue = cd.overload_ue.max(p_ue);
            }

            // Mark loads (not generators) by the degree of overload.
            let (ov_een, ov_ue) = {
                let cd = store.ckt_elem(branch).cd();
                (cd.overload_een, cd.overload_ue)
            };
            for pc in &shunts {
                if let Some(load) = store.obj_mut(*pc).as_any_mut().downcast_mut::<Load>() {
                    load.een_factor = if ov_een > 0.0 && st.zone_is_radial && !st.voltage_ue_only {
                        ov_een
                    } else {
                        0.0
                    };
                    load.ue_factor = if ov_ue > 0.0 && st.zone_is_radial && !st.voltage_ue_only {
                        ov_ue
                    } else {
                        0.0
                    };
                }
            }
            cur = tree.go_forward();
        }
    }

    // ---- Load / generation / loss accumulation pass ----------------------
    let mut total_zone_kw = 0.0_f64;
    let mut total_zone_kvar = 0.0_f64;
    let mut total_load_een = 0.0_f64;
    let mut total_load_ue = 0.0_f64;
    let mut total_gen_kw = 0.0_f64;
    let mut total_gen_kvar = 0.0_f64;

    let mut cur = tree.first();
    while let Some(branch) = cur {
        let (volt_base_index, shunts) = {
            let n = tree.present_node();
            (n.volt_base_index, n.shunts.clone())
        };
        let vbi = volt_base_index; // 1-based; 0 = none

        for pc in &shunts {
            let is_load = store.obj(*pc).as_any().downcast_ref::<Load>().is_some();
            let is_gen = store
                .obj(*pc)
                .as_any()
                .downcast_ref::<Generator>()
                .is_some();
            if is_load && !st.local_only {
                let load = store
                    .obj_mut(*pc)
                    .as_any_mut()
                    .downcast_mut::<Load>()
                    .expect("checked is_load");
                let load_kw = accumulate_load(
                    load,
                    sys,
                    node_v,
                    st.excess_flag,
                    norm_min,
                    emerg_min,
                    &mut total_zone_kw,
                    &mut total_zone_kvar,
                    &mut total_load_een,
                    &mut total_load_ue,
                );
                if st.f_vbase_losses && vbi > 0 {
                    vbase_load[vbi as usize - 1] += load_kw;
                }
            } else if is_gen {
                let gen_obj = store
                    .obj_mut(*pc)
                    .as_any_mut()
                    .downcast_mut::<Generator>()
                    .expect("checked is_gen");
                // Pascal `Accumulate_Gen` (l.2198): the `var` params are bound
                // to the gen totals, not the zone-load totals.
                let s = -gen_obj.terminal_power(sys, node_v, 1) * 0.001;
                total_gen_kw += s.re;
                total_gen_kvar += s.im;
            }
        }

        if st.f_losses {
            let (is_line, is_xfmr, nphases) = branch_kind(store, branch);
            let (s_total, s_load, s_no_load) = {
                let elem = store.ckt_elem_mut(branch);
                let (t, l, n) = elem.get_losses_split(sys, node_v);
                (t * 0.001, l * 0.001, n * 0.001)
            };
            total_losses += s_total;
            total_load_losses += s_load;
            total_no_load_losses += s_no_load;

            if is_line && st.f_line_losses {
                total_line_losses += s_total;
                if st.f_seq_losses {
                    let (mut pos, neg, zero) = {
                        let elem = store.ckt_elem_mut(branch);
                        elem.get_seq_losses(sys, node_v)
                    };
                    pos += neg; // add line modes together
                    pos *= 0.001;
                    let zero = zero * 0.001;
                    total_line_mode_losses += pos;
                    total_zero_mode_losses += zero;
                }
                if st.f_3phase_losses {
                    if nphases == 3 {
                        total_3phase_losses += s_total;
                    } else {
                        total_1phase_losses += s_total;
                    }
                }
            } else if is_xfmr && st.f_xfmr_losses {
                total_transformer_losses += s_total;
            }

            if st.f_vbase_losses && vbi > 0 {
                let k = vbi as usize - 1;
                vbase_total_losses[k] += s_total.re;
                if is_line {
                    vbase_line_losses[k] += s_total.re;
                } else if is_xfmr {
                    vbase_load_losses[k] += s_load.re;
                    vbase_no_load_losses[k] += s_no_load.re;
                }
            }
            // FPhaseVoltageReport is off by default and only feeds the demand-
            // interval phase-voltage files (Phase 8); not accumulated here.
        }

        cur = tree.go_forward();
    }

    // ---- Integrate losses, load, generation ------------------------------
    st.integrate(reg::LOAD_EEN, total_load_een, delta_hrs);
    st.integrate(reg::LOAD_UE, total_load_ue, delta_hrs);
    st.integrate(reg::ZONE_LOSSES_KWH, total_losses.re, delta_hrs);
    st.integrate(reg::ZONE_LOSSES_KVARH, total_losses.im, delta_hrs);
    st.integrate(reg::LOAD_LOSSES_KWH, total_load_losses.re, delta_hrs);
    st.integrate(reg::LOAD_LOSSES_KVARH, total_load_losses.im, delta_hrs);
    st.integrate(reg::NO_LOAD_LOSSES_KWH, total_no_load_losses.re, delta_hrs);
    st.integrate(
        reg::NO_LOAD_LOSSES_KVARH,
        total_no_load_losses.im,
        delta_hrs,
    );
    st.integrate(reg::LINE_LOSSES_KWH, total_line_losses.re, delta_hrs);
    st.integrate(
        reg::LINE_MODE_LINE_LOSS,
        total_line_mode_losses.re,
        delta_hrs,
    );
    st.integrate(
        reg::ZERO_MODE_LINE_LOSS,
        total_zero_mode_losses.re,
        delta_hrs,
    );
    st.integrate(
        reg::THREE_PHASE_LINE_LOSS,
        total_3phase_losses.re,
        delta_hrs,
    );
    st.integrate(reg::ONE_PHASE_LINE_LOSS, total_1phase_losses.re, delta_hrs);
    st.integrate(
        reg::TRANSFORMER_LOSSES_KWH,
        total_transformer_losses.re,
        delta_hrs,
    );
    for i in 0..NUM_EM_VBASE {
        st.integrate(reg::VBASE_START + 1 + i, vbase_total_losses[i], delta_hrs);
        st.integrate(
            reg::VBASE_START + 1 + NUM_EM_VBASE + i,
            vbase_line_losses[i],
            delta_hrs,
        );
        st.integrate(
            reg::VBASE_START + 1 + 2 * NUM_EM_VBASE + i,
            vbase_load_losses[i],
            delta_hrs,
        );
        st.integrate(
            reg::VBASE_START + 1 + 3 * NUM_EM_VBASE + i,
            vbase_no_load_losses[i],
            delta_hrs,
        );
        st.integrate(
            reg::VBASE_START + 1 + 4 * NUM_EM_VBASE + i,
            vbase_load[i],
            delta_hrs,
        );
    }

    st.integrate(reg::ZONE_KWH, total_zone_kw, delta_hrs);
    st.integrate(reg::ZONE_KVARH, total_zone_kvar, delta_hrs);
    st.integrate(reg::GEN_KWH, total_gen_kw, delta_hrs);
    st.integrate(reg::GEN_KVARH, total_gen_kvar, delta_hrs);
    let gen_kva = (total_gen_kvar.powi(2) + total_gen_kw.powi(2)).sqrt();
    let load_kva = (total_zone_kvar.powi(2) + total_zone_kw.powi(2)).sqrt();

    // ---- Drag-hand maxima ------------------------------------------------
    st.set_drag(reg::LOSSES_MAX_KW, total_losses.re.abs());
    st.set_drag(reg::LOSSES_MAX_KVAR, total_losses.im.abs());
    st.set_drag(reg::MAX_LOAD_LOSSES, total_load_losses.re.abs());
    st.set_drag(reg::MAX_NO_LOAD_LOSSES, total_no_load_losses.re.abs());
    st.set_drag(reg::ZONE_MAX_KW, total_zone_kw);
    st.set_drag(reg::ZONE_MAX_KVA, load_kva);
    st.set_drag(reg::GEN_MAX_KW, total_gen_kw);
    st.set_drag(reg::GEN_MAX_KVA, gen_kva);

    // ---- Overload energy -------------------------------------------------
    let zone_kw = if st.local_only {
        s_local.re
    } else {
        total_zone_kw
    };
    let mut s_local_kva = s_local_kva;
    if st.max_zone_kva_norm > 0.0 {
        if s_local_kva == 0.0 {
            s_local_kva = st.max_zone_kva_norm;
        }
        st.integrate(
            reg::OVERLOAD_KWH_NORM,
            (zone_kw * (1.0 - st.max_zone_kva_norm / s_local_kva)).max(0.0),
            delta_hrs,
        );
    } else {
        st.integrate(reg::OVERLOAD_KWH_NORM, max_excess_kw_norm, delta_hrs);
    }
    if st.max_zone_kva_emerg > 0.0 {
        if s_local_kva == 0.0 {
            s_local_kva = st.max_zone_kva_emerg;
        }
        st.integrate(
            reg::OVERLOAD_KWH_EMERG,
            (zone_kw * (1.0 - st.max_zone_kva_emerg / s_local_kva)).max(0.0),
            delta_hrs,
        );
    } else {
        st.integrate(reg::OVERLOAD_KWH_EMERG, max_excess_kw_emerg, delta_hrs);
    }

    st.first_sample_after_reset = false;
    // SaveDemandInterval (WriteDemandIntervalData) is Phase 8.

    downcast_meter(store, meter_ref).end_take_sample(tree, st);
}

/// Pascal `TEnergyMeterObj.Accumulate_Load` (l.2208): add the load's terminal-1
/// power to the zone totals and its EEN/UE contribution; returns the load kW.
#[allow(clippy::too_many_arguments)]
fn accumulate_load(
    load: &mut Load,
    sys: &SysCtx,
    node_v: &[Complex64],
    excess_flag: bool,
    norm_min: f64,
    emerg_min: f64,
    total_zone_kw: &mut f64,
    total_zone_kvar: &mut f64,
    total_load_een: &mut f64,
    total_load_ue: &mut f64,
) -> f64 {
    let s_load = load.terminal_power(sys, node_v, 1) * 0.001;
    let kw_load = s_load.re;
    *total_zone_kw += kw_load;
    *total_zone_kvar += s_load.im;

    let exceeds = load.exceeds_normal(sys, node_v, norm_min, emerg_min);
    let unserved = load.unserved(sys, node_v, norm_min, emerg_min);
    let (load_een, load_ue) = if excess_flag {
        (
            if exceeds {
                kw_load * load.een_factor
            } else {
                0.0
            },
            if unserved {
                kw_load * load.ue_factor
            } else {
                0.0
            },
        )
    } else {
        (
            if exceeds { kw_load } else { 0.0 },
            if unserved { kw_load } else { 0.0 },
        )
    };
    *total_load_een += load_een;
    *total_load_ue += load_ue;
    kw_load
}
