//! The `HasEnergyMeter`/`HasSensorObj` flag passes — Pascal
//! `TEnergyMeter.SetHasMeterFlag` / `TSensor.SetHasSensorFlag` — that mark each
//! metered element (and seed its `sensor_obj` back-pointer) before the zone
//! walk. Split out of `zones/mod.rs` (no behavioral change).

use crate::circuit::Circuit;
use crate::elements::ckt::ElemFlags;
use crate::elements::meter::energymeter::EnergyMeter;
use crate::elements::meter::sensor::Sensor;
use crate::elements::traits::{ElemStore, TypedStore};

/// Pascal `TEnergyMeter.SetHasMeterFlag` (l.1752): clear `HasEnergyMeter` on all
/// PD elements, then set it on each enabled meter's metered element.
pub(super) fn set_has_meter_flag(ckt: &Circuit, store: &mut dyn ElemStore) {
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
                .typed::<EnergyMeter>(meter_ref)
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

/// Pascal `TSensor.SetHasSensorFlag` (Sensor.pas l.357): clear `HasSensorObj`
/// on all PD/PC elements, then set it (and the back-pointer `sensor_obj`) on
/// each sensor's metered element. The metered element's own sensor wins over the
/// upstream one the zone walk would otherwise propagate.
pub(super) fn set_has_sensor_flag(ckt: &Circuit, store: &mut dyn ElemStore) {
    for &r in ckt.pd_elements.iter().chain(&ckt.pc_elements) {
        store
            .ckt_elem_mut(r)
            .cd_mut()
            .flags
            .exclude(ElemFlags::HAS_SENSOR_OBJ);
    }
    for &sensor_ref in &ckt.sensors {
        let metered = store
            .typed::<Sensor>(sensor_ref)
            .expect("sensors holds Sensor objects")
            .metered_element();
        if let Some(mr) = metered {
            let cd = store.ckt_elem_mut(mr).cd_mut();
            cd.flags.include(ElemFlags::HAS_SENSOR_OBJ);
            cd.sensor_obj = Some(sensor_ref);
        }
    }
}
