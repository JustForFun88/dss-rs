//! Port of `Meters/Sensor.pas` — `TSensorObj`, the state-estimation /
//! load-allocation measurement point. A Sensor records the measured voltages
//! and currents (or kW/kvar, converted to currents on a rated base) at one
//! terminal of a circuit element; it has **no** Yprim (`CalcYPrim` is empty) and
//! its terminal currents are always zero.
//!
//! Phase 6 (WP6.7) ports the property surface, `RecalcElementData`/`RecalcVbase`/
//! `ClearSensor`/`UpdateCurrentVector`/`ZeroSensorArrays`, `RotatePhases`,
//! `TakeSample` and the WLS error getters. The behaviorally load-bearing path is
//! the inherited `CalcAllocationFactors` (`MeterElement.pas`, already ported)
//! plus `EnergyMeter.AllocateLoad`, which the `allocateloads` command drives.
//!
//! **Compat note (probed against the oracle):** because `element=`/`conn=`/
//! `deltadirection=` all set `NeedsRecalc`, defining a sensor and its measured
//! `currents`/`kVs`/`kWs` in a *single* command zeros the measured arrays at
//! `EndEdit` (`RecalcElementData` → `ZeroSensorArrays`). To retain measured
//! values you must set them in a later `edit` (no recalc trigger). The port
//! reproduces this exactly.
//!
//! Split into submodules (no behavioral change): the struct, its constructor and
//! the read-only accessors plus the property table live here; the sensor
//! algorithms (`RecalcElementData`/`RecalcVbase`/`UpdateCurrentVector`/
//! `RotatePhases`/`TakeSample`/WLS errors) are in [`compute`], and the
//! `CktElement`/`DssObject` trait impls in [`accessors`].

#[cfg(test)]
mod tests;

mod accessors;
mod compute;

use crate::elements::meter::meter_element::MeterElementData;
use crate::elements::traits::ElemRef;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (`TSensorProp` + the `TCktElementClass` tail).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const KVBASE: usize = 3;
    pub const CLEAR: usize = 4;
    pub const KVS: usize = 5;
    pub const CURRENTS: usize = 6;
    pub const KWS: usize = 7;
    pub const KVARS: usize = 8;
    pub const CONN: usize = 9;
    pub const DELTA_DIRECTION: usize = 10;
    pub const PCT_ERROR: usize = 11;
    pub const WEIGHT: usize = 12;
    // CktElementClass tail:
    pub const BASE_FREQ: usize = 13;
    pub const ENABLED: usize = 14;
    pub const NUM_PROPS: usize = 15; // incl. Like
}

/// `TSensor.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        // `DSSObjectReferenceProperty` with `PropertyOffset2 = 0` (any class) +
        // `Required` (enforced by the recalc 666 message when nil).
        PropDef::object_ref_any("element"),
        PropDef::integer("terminal"),
        PropDef::double("kVBase"),
        // `BooleanActionProperty` over `DoClearSensor`: setting `yes` clears the
        // spec flags; stores nothing, so the getter is always `No`.
        PropDef::boolean("clear"),
        // `DoubleVArrayProperty`s over the per-phase measured arrays (Fnphases).
        PropDef::double_v_array("kVs"),
        PropDef::double_v_array("currents"),
        PropDef::double_v_array("kWs"),
        PropDef::double_v_array("kvars"),
        PropDef::mapped_string_enum("conn", enums.connection),
        PropDef::integer("DeltaDirection"),
        PropDef::double("%Error"),
        PropDef::double("Weight"),
        // CktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("Sensor", defs, true)
}

/// `TSensorObj`.
#[derive(Debug, Clone)]
pub struct Sensor {
    pub med: MeterElementData,

    valid_sensor: bool,
    /// `SensorkW`/`Sensorkvar` (private per-phase P/Q spec arrays; props 7/8).
    sensor_kw: Vec<f64>,
    sensor_kvar: Vec<f64>,
    /// `kVBase` (value specified, kV) and `Vbase` (derived, volts).
    kv_base: f64,
    vbase: f64,

    v_specified: bool,
    i_specified: bool,
    p_specified: bool,
    q_specified: bool,

    f_delta_direction: i32,
    /// `pctError` and `Weight` (WLS).
    pct_error: f64,
    weight: f64,
    /// `FConn` (0 = wye, 1 = delta).
    f_conn: i32,

    /// FullName of the metered element (`Class.name`) for the dump.
    element_full_name: String,
    /// Pascal `Flg.NeedsRecalc` (set by element/terminal/kvbase/conn/dir).
    needs_recalc: bool,
}

impl Sensor {
    /// Pascal `TSensorObj.Create`: 3-phase, kVBase = 12.47, weight 1, %error 1,
    /// delta-direction +1, wye, arrays cleared.
    pub fn new(name: &str) -> Self {
        let mut med = MeterElementData::new(name, prop::NUM_PROPS);
        med.cd.nphases = 3;
        med.cd.set_nconds(3);
        med.cd.set_nterms(1);
        med.metered_terminal = 1;
        let mut s = Self {
            med,
            valid_sensor: false,
            sensor_kw: Vec::new(),
            sensor_kvar: Vec::new(),
            kv_base: 12.47,
            vbase: 0.0,
            v_specified: false,
            i_specified: false,
            p_specified: false,
            q_specified: false,
            f_delta_direction: 1,
            pct_error: 1.0,
            weight: 1.0,
            f_conn: 0,
            element_full_name: String::new(),
            needs_recalc: false,
        };
        s.recalc_vbase();
        s.clear_sensor();
        s
    }

    /// `MeteredElement` (the sensored circuit element), used by
    /// `SetHasSensorFlag` and `AllocateLoad`.
    pub fn metered_element(&self) -> Option<ElemRef> {
        self.med.metered_element
    }
}
