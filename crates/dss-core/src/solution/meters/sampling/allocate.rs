//! Load allocation — Pascal `TExecHelper.DoAllocateLoadsCmd` (ExecHelper.pas
//! l.2605) and the per-zone `CalcAllocationFactors` / `AllocateLoad` it drives.
//! Split out of `sampling/mod.rs` (no behavioral change).

use crate::circuit::Circuit;
use crate::elements::meter::energymeter::EnergyMeter;
use crate::elements::meter::sensor::Sensor;
use crate::elements::pc::load::Load;
use crate::elements::traits::{ElemId, ElemStore, SysCtx, TypedStore};
use crate::solution::SolveEnv;
use crate::solution::solution::{solve, sys_ctx};

use super::super::meter_mut;

/// Pascal `TExecHelper.DoAllocateLoadsCmd` allocation loop (ExecHelper.pas
/// l.2605). The caller has already forced `LoadMultiplier = 1.0`. Solve a guess
/// from the present factors, then iterate: recompute every meter/sensor
/// allocation factor, run each meter's zone load adjustment, and re-solve.
pub(crate) fn allocate_loads(ckt: &mut Circuit, env: &mut SolveEnv, max_iters: usize) {
    let _ = solve(ckt, env); // guess based on present allocation factors
    for _ in 0..max_iters {
        let sys = sys_ctx(ckt);
        calc_allocation_factors_all(ckt, env.store, &sys);
        allocate_load_all(ckt, env.store);
        let _ = solve(ckt, env); // update the solution
    }
}

/// `CalcAllocationFactors` over every EnergyMeter then every Sensor (Pascal
/// order). Each reads its metered element's currents vs the measured peak.
fn calc_allocation_factors_all(ckt: &Circuit, store: &mut dyn ElemStore, sys: &SysCtx) {
    let node_v = &ckt.solution.node_v;
    for meter_ref in ckt.energy_meters.clone() {
        // D9 (dss_capi `fb728364`, SVN r4115): `TMeterElement.CalcAllocationFactors`
        // opens with `if not Enabled then Exit` — a disabled meter contributes no
        // allocation factors.
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
        // D9 (SVN r4115): `TMeterElement.CalcAllocationFactors` skips a disabled
        // sensor too (both EnergyMeter and Sensor are `TMeterElement`).
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

/// `AllocateLoad` over every EnergyMeter zone.
fn allocate_load_all(ckt: &Circuit, store: &mut dyn ElemStore) {
    for meter_ref in ckt.energy_meters.clone() {
        // D9 (dss_capi `fb728364`, SVN r4115): `TEnergyMeterObj.AllocateLoad`
        // opens with `if not Enabled then Exit` — a disabled meter does not
        // adjust its zone loads.
        if !meter_mut(store, meter_ref).enabled() {
            continue;
        }
        allocate_load_for_meter(meter_ref, ckt, store);
    }
}

/// Pascal `TEnergyMeterObj.AllocateLoad` (EnergyMeter.pas l.2147): walk the
/// meter's zone loads and scale each by its upstream sensor's allocation factor
/// (single-phase loads use the connected-phase factor; poly-phase loads use the
/// average factor). The sensor may be a Sensor object **or** an EnergyMeter.
fn allocate_load_for_meter(meter_ref: ElemId, ckt: &Circuit, store: &mut dyn ElemStore) {
    // Pascal walks `BranchList` (`First`/`GoForward`) and, per branch, its shunt
    // objects (`FirstObject`/`NextObject`), filtering to `LOAD_ELEMENT`. The
    // meter's `load_list` is exactly that set (only Loads are pushed during the
    // zone build, in the same branch-tree order) and the per-load scaling is
    // independent, so iterating it is equivalent — no other shunt type responds.
    let loads = meter_mut(store, meter_ref).load_list().to_vec();
    for load_ref in loads {
        let (nphases, sensor_ref, alloc_factor, node_ref0) = {
            let load = store
                .typed::<Load>(load_ref)
                .expect("load_list holds Load objects");
            (
                load.cd.nphases,
                load.cd.sensor_obj,
                load.allocation_factor(),
                load.cd.node_ref.first().copied().unwrap_or(0),
            )
        };
        // Pascal dereferences `LoadElem.SensorObj` unconditionally; every zone
        // load is given a sensor (the meter, or an upstream Sensor) by the zone
        // build, so this is never NIL there. We guard defensively rather than
        // panic if the back-pointer is somehow unset.
        let Some(sref) = sensor_ref else { continue };
        let Some((s_nphases, avg, phs)) = sensor_alloc_data(store, sref) else {
            continue;
        };
        let new_factor = if nphases == 1 {
            // Connected phase from NodeRef[1] → MapNodeToBus → NodeNum (1..3).
            let connected_phase = ckt
                .map_node_to_bus
                .get(node_ref0)
                .map(|nb| nb.node_num)
                .unwrap_or(0);
            if connected_phase > 0 && connected_phase < 4 {
                // Pascal indexes `PhsAllocationFactor[ConnectedPhase]` directly
                // (1-based → `connected_phase - 1` here). A connected phase past
                // the sensor's phase count is a malformed circuit (e.g. a 3-phase
                // node on a 2-phase sensor) where Pascal reads past the array; we
                // fall back to the no-change factor 1.0 instead.
                let f = if s_nphases == 1 {
                    phs.first().copied().unwrap_or(1.0)
                } else {
                    phs.get((connected_phase - 1) as usize)
                        .copied()
                        .unwrap_or(1.0)
                };
                Some(alloc_factor * f)
            } else {
                None // Pascal leaves the factor unchanged outside phases 1..3
            }
        } else {
            Some(alloc_factor * avg)
        };
        if let Some(nf) = new_factor {
            store
                .typed_mut::<Load>(load_ref)
                .expect("load_list holds Load objects")
                .set_allocation_factor(nf);
        }
    }
}

/// The (nphases, AvgAllocFactor, PhsAllocationFactor) of a metering device,
/// which may be a [`Sensor`] or an [`EnergyMeter`] (both embed `MeterElementData`).
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
