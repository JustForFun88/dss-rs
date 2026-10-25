//! Load allocation: the `AllocateLoads` loop, the allocation factors each
//! EnergyMeter and Sensor computes against its measured peak, and the scaling
//! of every meter zone's loads by them.

use crate::circuit::Circuit;
use crate::elements::meter::energymeter::EnergyMeter;
use crate::elements::meter::sensor::Sensor;
use crate::elements::pc::load::{AllocationCommand, Load};
use crate::elements::traits::{ElemId, ElemStore, SysCtx, TypedStore};
use crate::solution::SolveEnv;
use crate::solution::solution::{solve, sys_ctx};

use super::super::meter_mut;

/// The `AllocateLoads` loop. The caller has already forced
/// `LoadMultiplier = 1.0`. Solve a guess from the present factors, then
/// iterate: recompute every meter and sensor allocation factor, adjust each
/// meter's zone loads, and re-solve. A pass whose factor for a load is not a
/// finite number, or would leave the load unbuilt, changes no load and ends
/// the command, naming the first such load (pinned by
/// `exec::tests::rating_refusals::an_allocation_factor_set_outside_an_edit_is_refused_where_its_load_would_not_build`).
/// A load refused as it stands keeps the factor it holds, logged once.
pub(crate) fn allocate_loads(ckt: &mut Circuit, env: &mut SolveEnv, max_iters: usize) {
    let _ = solve(ckt, env); // guess based on present allocation factors
    let mut skipped: Vec<String> = Vec::new();
    for _ in 0..max_iters {
        let sys = sys_ctx(ckt);
        calc_allocation_factors_all(ckt, env.store, &sys);
        match allocate_load_all(ckt, env.store, &sys) {
            Ok(skips) => {
                for msg in skips {
                    if !skipped.contains(&msg) {
                        env.errors.push(msg.clone());
                        skipped.push(msg);
                    }
                }
            }
            Err(msg) => {
                env.errors.push(msg);
                return;
            }
        }
        let _ = solve(ckt, env); // update the solution
    }
}

/// Compute the allocation factors of every enabled EnergyMeter, then of every
/// enabled Sensor, each from its metered element's currents against its
/// measured peak.
fn calc_allocation_factors_all(ckt: &Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let node_v = &ckt.solution.node_v;
    for meter_ref in ckt.energy_meters.clone() {
        // A disabled meter contributes no allocation factors.
        if !meter_mut(store, meter_ref).enabled() {
            continue;
        }
        let Some(mr) = meter_mut(store, meter_ref).metered_element() else {
            continue;
        };
        let (meter, metered) = store.typed_ckt_pair_mut::<EnergyMeter>(meter_ref, mr);
        let ce = metered.expect("metered element is a circuit element");
        meter.med.calc_allocation_factors(ce, sys, node_v);
    }
    for sensor_ref in ckt.sensors.clone() {
        let sensor = store
            .typed::<Sensor>(sensor_ref)
            .expect("sensors holds Sensor objects");
        // Nor does a disabled sensor.
        if !sensor.med.cd.enabled {
            continue;
        }
        let metered = sensor.metered_element();
        let Some(mr) = metered else { continue };
        let (sensor, metered) = store.typed_ckt_pair_mut::<Sensor>(sensor_ref, mr);
        let ce = metered.expect("metered element is a circuit element");
        sensor.med.calc_allocation_factors(ce, sys, node_v);
    }
}

/// Adjust the loads of every enabled EnergyMeter zone. The new factors of
/// the pass are collected in zone order first, each read from the factor an
/// earlier zone of the pass gave the same load, and applied only when every
/// one is a finite number that leaves its load built. Otherwise the refusal
/// of the first such load is returned. A load refused as it stands is
/// skipped, and the messages of the skips are returned.
fn allocate_load_all(
    ckt: &Circuit,
    store: &mut dyn ElemStore,
    sys: &SysCtx,
) -> Result<Vec<String>, String> {
    let mut pending: Vec<(ElemId, f64)> = Vec::new();
    for meter_ref in ckt.energy_meters.clone() {
        // A disabled meter does not adjust its zone loads.
        if !meter_mut(store, meter_ref).enabled() {
            continue;
        }
        allocate_load_for_meter(meter_ref, ckt, store, &mut pending);
    }
    let command = AllocationCommand::Allocate;
    for &(load_ref, factor) in &pending {
        let load = store
            .typed::<Load>(load_ref)
            .expect("load_list holds Load objects");
        let refusal = command
            .not_finite_factor(factor, Some(load.cd.obj.name()))
            .or_else(|| load.allocation_refusal(command, factor, sys));
        if let Some(msg) = refusal {
            return Err(msg);
        }
    }
    let mut skips = Vec::new();
    for (load_ref, factor) in pending {
        let load = store
            .typed_mut::<Load>(load_ref)
            .expect("load_list holds Load objects");
        match load.allocation_skip(command) {
            Some(msg) => skips.push(msg),
            None => load.set_allocation_factor(factor),
        }
    }
    Ok(skips)
}

/// Walk the meter's zone loads and push to `pending` each one's factor
/// scaled by its upstream sensor's allocation factor (single-phase loads use
/// the connected-phase factor, poly-phase loads the average factor). The
/// sensor may be a Sensor object **or** an EnergyMeter.
fn allocate_load_for_meter(
    meter_ref: ElemId,
    ckt: &Circuit,
    store: &mut dyn ElemStore,
    pending: &mut Vec<(ElemId, f64)>,
) {
    // The meter's `load_list` holds the zone's loads in branch-tree order, and
    // each load is scaled independently of the others.
    let loads = meter_mut(store, meter_ref).load_list().to_vec();
    for load_ref in loads {
        let (nphases, sensor_ref, alloc_factor, node_ref0) = {
            let load = store
                .typed::<Load>(load_ref)
                .expect("load_list holds Load objects");
            (
                load.cd.nphases,
                load.cd.sensor_obj,
                pending
                    .iter()
                    .rev()
                    .find(|(r, _)| *r == load_ref)
                    .map_or(load.allocation_factor(), |&(_, f)| f),
                load.cd.node_ref.first().copied().unwrap_or(0),
            )
        };
        // The zone build gives every zone load a sensor (the meter or an
        // upstream Sensor), so a load without one is skipped, never a panic.
        let Some(sref) = sensor_ref else { continue };
        let Some((s_nphases, avg, phs)) = sensor_alloc_data(store, sref) else {
            continue;
        };
        let new_factor = if nphases == 1 {
            // The connected phase is the bus node number of the load's first node.
            let connected_phase = ckt
                .map_node_to_bus
                .get(node_ref0)
                .map(|nb| nb.node_num)
                .unwrap_or(0);
            if connected_phase > 0 && connected_phase < 4 {
                // A one-phase sensor gives its only factor. A connected phase
                // past the sensor's phase count (a load on node 3 under a
                // two-phase sensor) takes the no-change factor 1.
                let f = if s_nphases == 1 {
                    phs.first().copied().unwrap_or(1.0)
                } else {
                    phs.get((connected_phase - 1) as usize)
                        .copied()
                        .unwrap_or(1.0)
                };
                Some(alloc_factor * f)
            } else {
                None // a load off phases 1 to 3 keeps its factor
            }
        } else {
            Some(alloc_factor * avg)
        };
        if let Some(nf) = new_factor {
            pending.push((load_ref, nf));
        }
    }
}

/// The phase count, the average allocation factor and the per-phase factors
/// of a metering device, a [`Sensor`] or an [`EnergyMeter`] (both embed
/// `MeterElementData`).
fn sensor_alloc_data(store: &dyn ElemStore, r: ElemId) -> Option<(usize, f64, Vec<f64>)> {
    if let Some(s) = store.typed::<Sensor>(r) {
        Some((
            s.med.cd.nphases,
            s.med.avg_alloc_factor,
            s.med.phs_allocation_factor.clone(),
        ))
    } else {
        store.typed::<EnergyMeter>(r).map(|em| {
            (
                em.med.cd.nphases,
                em.med.avg_alloc_factor,
                em.med.phs_allocation_factor.clone(),
            )
        })
    }
}
