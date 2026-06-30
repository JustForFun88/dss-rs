//! EnergyMeter zone building — Pascal `TEnergyMeter.ResetMeterZonesAll` /
//! `SetHasMeterFlag` / `TEnergyMeterObj.MakeMeterZoneLists` plus
//! `Circuit.DoResetMeterZones`. Implemented as free functions over the registry
//! (the WP5.7 control-sweep pattern): the meter's zone topology is built in
//! local data structures while the **other** circuit elements' flags/refs and
//! the buses' `DistFromMeter` are mutated through the store, then the result is
//! installed into the meter object.
//!
//! Split by concern (no behavioral change): the per-meter zone walk and the
//! customer roll-up live in [`build`]; the `HasEnergyMeter`/`HasSensorObj` flag
//! passes live in [`flags`]; this module keeps the `DoResetMeterZones` entry and
//! the `ResetMeterZonesAll` orchestration that drives them.

use crate::circuit::Circuit;
use crate::circuit::ckt_tree::build_active_bus_adjacency_lists;
use crate::elements::ckt::ElemFlags;
use crate::elements::traits::ElemStore;

mod build;
mod flags;

use build::make_meter_zone_lists;
use flags::{set_has_meter_flag, set_has_sensor_flag};

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
    // Pascal `SensorClass.SetHasSensorFlag` (EnergyMeter.pas l.836): mark each
    // sensor's metered element so the zone walk passes the sensor down its zone.
    set_has_sensor_flag(ckt, store);

    for bus in &mut ckt.buses {
        bus.bus_checked = false;
    }

    let meters = ckt.energy_meters.clone();
    for meter_ref in meters {
        make_meter_zone_lists(meter_ref, ckt, store, &adj);
    }
    // `adj` is dropped here (Pascal `FreeAndNilBusAdjacencyLists`).
}
