//! EnergyMeter solution-time logic, split by concern: zone building
//! ([`zones`]), sampling and load allocation ([`sampling`]), and reliability
//! indices ([`reliability`]). All three operate as free functions over the
//! element registry (the WP5.7 control-sweep pattern): the meter's own state is
//! taken out of the object for the walk and written back at the end, while the
//! zone's other elements and the buses are read/mutated through the store.

use crate::elements::meter::energymeter::EnergyMeter;
use crate::elements::traits::{ElemRef, ElemStore};

mod reliability;
mod sampling;
mod zones;

pub(crate) use reliability::calc_all_reliability_indices;
pub(crate) use sampling::{allocate_loads, reset_all_meters, take_sample_all};
pub(crate) use zones::do_reset_meter_zones;

/// Downcast a registry entry known to be an [`EnergyMeter`] to a mutable ref.
/// Shared by all three submodules' write-back paths (`ckt.energy_meters` only
/// ever holds `EnergyMeter` objects, so the downcast is infallible).
fn downcast_meter(store: &mut dyn ElemStore, meter_ref: ElemRef) -> &mut EnergyMeter {
    store
        .obj_mut(meter_ref)
        .as_any_mut()
        .downcast_mut::<EnergyMeter>()
        .expect("energy_meters holds EnergyMeter objects")
}
