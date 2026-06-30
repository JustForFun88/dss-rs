//! EnergyMeter sampling and load allocation — Pascal `TEnergyMeter.SampleAll` /
//! `TEnergyMeterObj.TakeSample` (EnergyMeter.pas l.900 / l.1289) and
//! `TExecHelper.DoAllocateLoadsCmd`. The meter's branch tree and registers are
//! moved out of the meter object for the walk (the store keeps the meter
//! borrowed), the zone's PD/PC elements are read/mutated through the store, and
//! the registers are written back at the end.
//!
//! Split by concern (no behavioral change): the register reset and the
//! `TakeSample` zone walk live in [`take_sample`]; the `DoAllocateLoadsCmd`
//! iteration and the per-zone allocation-factor scaling live in [`allocate`].

mod allocate;
mod take_sample;

pub(crate) use allocate::allocate_loads;
pub(crate) use take_sample::{reset_all_meters, take_sample_all};
