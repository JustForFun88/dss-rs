//! EnergyMeter solution-time logic, split by concern: zone building
//! ([`zones`]), sampling and load allocation ([`sampling`]), and reliability
//! indices ([`reliability`]). All three operate as free functions over the
//! element registry (the WP5.7 control-sweep pattern): the meter's own state is
//! taken out of the object for the walk and written back at the end, while the
//! zone's other elements and the buses are read/mutated through the store.

use crate::circuit::Circuit;
use crate::elements::meter::energymeter::EnergyMeter;
use crate::elements::traits::{ElemId, ElemStore};

pub mod demand_interval;
mod interpolate;
mod reliability;
mod sampling;
mod zones;

pub use demand_interval::EmDiState;
pub(crate) use demand_interval::{close_all_di_files, open_all_di_files};
pub(crate) use interpolate::interpolate_coordinates;
pub(crate) use reliability::calc_all_reliability_indices;
pub(crate) use sampling::{allocate_loads, reset_all_meters, take_sample_all};
pub(crate) use zones::do_reset_meter_zones;

/// Pascal `TDSSGlobalHelper.SyncSeasonalRatingIdx` (dss_capi 0.15.x `55400a29`,
/// `DSSHelper.pas`): precompute the season index once so the seasonal report
/// paths don't re-read the XYCurve per element. `SeasonalRatingIdx := -1; if
/// SeasonalRating and (SeasonSignalObj <> NIL) and (ActiveCircuit <> NIL) and
/// (Solution <> NIL) then SeasonalRatingIdx := trunc(SeasonSignalObj.GetYValue(
/// intHour))`. Called on every solve and on the `Set SeasonRating`/`SeasonSignal`
/// /`Hour`/`Season` commands (the union of the Pascal call sites). Resolving the
/// signal by name (like the storage-controller path) leaves the idx at `-1` on a
/// missing/unset signal — the `SeasonSignalObj = NIL` branch.
pub(crate) fn sync_seasonal_rating_idx(ckt: &mut Circuit, store: &mut dyn ElemStore) {
    ckt.seasonal_rating_idx = -1;
    if !ckt.season_rating || ckt.season_signal.is_empty() {
        return;
    }
    let int_hour = ckt.solution.int_hour;
    if let Some(r) = store.find_general("XYcurve", &ckt.season_signal)
        && let Some(curve) = store
            .obj_mut(r)
            .as_any_mut()
            .downcast_mut::<crate::elements::general::xy_curve::XyCurveObj>()
    {
        ckt.seasonal_rating_idx = curve.get_y_value(int_hour as f64).trunc() as i32;
    }
}

/// Downcast a registry entry known to be an [`EnergyMeter`] to a mutable ref.
/// Shared by all three submodules' write-back paths (`ckt.energy_meters` only
/// ever holds `EnergyMeter` objects, so the downcast is infallible).
fn downcast_meter(store: &mut dyn ElemStore, meter_ref: ElemId) -> &mut EnergyMeter {
    store
        .obj_mut(meter_ref)
        .as_any_mut()
        .downcast_mut::<EnergyMeter>()
        .expect("energy_meters holds EnergyMeter objects")
}
