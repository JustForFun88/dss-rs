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

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::meter::meter_element::{MeterElementData, MeteredSnapshot};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::util::sqrt3;

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

    /// Pascal `DoClearSensor`: reset the spec flags.
    fn clear_sensor(&mut self) {
        self.v_specified = false;
        self.i_specified = false;
        self.p_specified = false;
        self.q_specified = false;
    }

    /// Pascal `RecalcVbase`: wye → L-N (÷√3 for poly-phase), delta → L-L.
    fn recalc_vbase(&mut self) {
        self.vbase = match self.f_conn {
            0 => {
                if self.med.cd.nphases == 1 {
                    self.kv_base * 1000.0
                } else {
                    self.kv_base * 1000.0 / sqrt3()
                }
            }
            _ => self.kv_base * 1000.0,
        };
    }

    /// Pascal `RotatePhases` (1-based): the next delta phase for L-L voltages.
    fn rotate_phases(&self, j: usize) -> usize {
        let nph = self.med.cd.nphases;
        let mut result = j as i32 + self.f_delta_direction;
        if nph > 2 {
            // 2-phase delta is treated as open delta.
            if result > nph as i32 {
                result = 1;
            }
            if result < 1 {
                result = nph as i32;
            }
        } else if result < 1 {
            result = 3; // 2-phase delta: next phase is the 3rd
        }
        result as usize
    }

    /// Pascal `AllocateSensorObjArrays` + `ZeroSensorArrays`: size the per-phase
    /// arrays to `Fnphases` and zero all four measured vectors.
    fn allocate_and_zero_arrays(&mut self) {
        let nph = self.med.cd.nphases;
        // AllocateSensorArrays sizes med.sensor_current/sensor_voltage; the
        // metered calc buffers follow the metered element's Yorder.
        let yorder = self
            .med
            .metered_snap
            .as_ref()
            .map(|s| s.yorder)
            .unwrap_or(0);
        self.med.allocate_sensor_arrays(yorder);
        self.sensor_kw = vec![0.0; nph];
        self.sensor_kvar = vec![0.0; nph];
        // ZeroSensorArrays (med arrays were ReAllocMem-preserved; zero them).
        for v in self.med.sensor_current.iter_mut() {
            *v = 0.0;
        }
        for v in self.med.sensor_voltage.iter_mut() {
            *v = 0.0;
        }
    }

    /// Pascal `UpdateCurrentVector`: when P (and optionally Q) are specified,
    /// convert to per-phase current magnitudes on `Vbase` and mark Ispecified.
    fn update_current_vector(&mut self) {
        if !self.p_specified {
            return;
        }
        let nph = self.med.cd.nphases;
        for i in 0..nph {
            let kva = if self.q_specified {
                Complex64::new(self.sensor_kw[i], self.sensor_kvar[i]).norm()
            } else {
                self.sensor_kw[i]
            };
            self.med.sensor_current[i] = kva * 1000.0 / self.vbase;
        }
        self.i_specified = true; // overrides current specification
    }

    /// Pascal `RecalcElementData`: validate the metered element + terminal,
    /// adopt its phase/conductor counts and bus, then clear + size + zero the
    /// measured arrays.
    pub fn recalc(&mut self, errors: &mut Vec<String>) {
        self.needs_recalc = false;
        self.valid_sensor = false;
        let Some(snap) = self.med.metered_snap.clone() else {
            errors.push(format!(
                "Sensor: \"{}\": Circuit Element is not set. Element must be defined previously.",
                self.med.cd.obj.name()
            ));
            return;
        };
        if self.med.metered_terminal as usize > snap.nterms {
            errors.push(format!(
                "Sensor: \"{}\": Terminal no. \"{}\" does not exist. Respecify terminal no.",
                self.med.cd.obj.name(),
                self.med.metered_terminal
            ));
            return;
        }
        self.med.cd.nphases = snap.nphases;
        self.med.cd.set_nconds(snap.nconds);
        let bus = snap
            .buses
            .get(self.med.metered_terminal as usize - 1)
            .cloned()
            .unwrap_or_default();
        self.med.cd.set_bus(1, &bus);
        self.clear_sensor();
        self.valid_sensor = true;
        self.allocate_and_zero_arrays();
        self.recalc_vbase();
    }

    /// Pascal `Get_WLSCurrentError`: weighted squared current error (re-derives
    /// `SensorCurrent` from P/Q first, as the Pascal getter does).
    #[allow(dead_code)]
    pub fn wls_current_error(&mut self) -> f64 {
        let nph = self.med.cd.nphases;
        self.update_current_vector_for_wls();
        let mut result = 0.0;
        if self.i_specified {
            for i in 0..nph {
                let c = self.med.calculated_current[i];
                result += c.re * c.re + c.im * c.im - self.med.sensor_current[i].powi(2);
            }
        }
        result * self.weight
    }

    /// The `Get_WLSCurrentError` P→I re-derivation (mirrors `UpdateCurrentVector`
    /// but inlined as the Pascal getter does — does not touch the spec flags
    /// other than `Ispecified`).
    fn update_current_vector_for_wls(&mut self) {
        if !self.p_specified {
            return;
        }
        let nph = self.med.cd.nphases;
        for i in 0..nph {
            let kva = if self.q_specified {
                Complex64::new(self.sensor_kw[i], self.sensor_kvar[i]).norm()
            } else {
                self.sensor_kw[i]
            };
            self.med.sensor_current[i] = kva * 1000.0 / self.vbase;
        }
        self.i_specified = true;
    }

    /// Pascal `Get_WLSVoltageError`: weighted squared voltage error.
    #[allow(dead_code)]
    pub fn wls_voltage_error(&self) -> f64 {
        let mut result = 0.0;
        if self.v_specified {
            for i in 0..self.med.cd.nphases {
                let c = self.med.calculated_voltage[i];
                result += c.re * c.re + c.im * c.im - self.med.sensor_voltage[i].powi(2);
            }
        }
        result * self.weight
    }

    /// Pascal `TakeSample`: read the metered element's currents into
    /// `CalculatedCurrent` and the terminal voltages (L-N or L-L per `conn`)
    /// into `CalculatedVoltage`. Requires the live metered element + node
    /// voltages, so it takes them as arguments (no gate exercises it yet).
    #[allow(dead_code)]
    pub fn take_sample(
        &mut self,
        metered: &mut dyn CktElement,
        sys: &SysCtx,
        node_v: &[Complex64],
    ) {
        if !(self.valid_sensor && self.med.cd.enabled) {
            return;
        }
        metered.get_currents(sys, node_v, &mut self.med.calculated_current);
        let nph = self.med.cd.nphases;
        // ComputeVterminal: VTerminal[i] = node_v[NodeRef[i]] (1-based Pascal).
        let mut vterminal = vec![Complex64::ZERO; self.med.cd.nconds.max(nph)];
        for (i, v) in vterminal.iter_mut().enumerate().take(self.med.cd.nconds) {
            *v = node_v[self.med.cd.node_ref[i]];
        }
        if self.f_conn == 1 {
            for i in 0..nph {
                self.med.calculated_voltage[i] =
                    vterminal[i] - vterminal[self.rotate_phases(i + 1) - 1];
            }
        } else {
            self.med.calculated_voltage[..nph].copy_from_slice(&vterminal[..nph]);
        }
    }
}

/// Capture the parse-relevant shape of the metered element (any class).
fn capture(full_name: String, obj: &dyn DssObject) -> MeteredSnapshot {
    let elem = obj
        .as_ckt_element()
        .expect("element= resolves to a ckt elem");
    let cd = elem.cd();
    MeteredSnapshot {
        full_name,
        nphases: cd.nphases,
        nconds: cd.nconds,
        nterms: cd.nterms,
        yorder: cd.yorder,
        buses: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
        ..Default::default()
    }
}

impl CktElement for Sensor {
    fn cd(&self) -> &CktElementData {
        &self.med.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.med.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        let mut errors = Vec::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    /// `TSensorObj.CalcYPrim` is empty — a sensor never stamps admittance.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// `TSensorObj.GetCurrents` returns zeros.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for Sensor {
    fn data(&self) -> &DssObjData {
        &self.med.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.med.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal,
            CONN => self.f_conn,
            DELTA_DIRECTION => self.f_delta_direction,
            _ => unreachable!("Sensor has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            TERMINAL => self.med.metered_terminal = value,
            CONN => self.f_conn = value,
            DELTA_DIRECTION => self.f_delta_direction = value,
            _ => unreachable!("Sensor has no integer property {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            KVBASE => self.kv_base,
            PCT_ERROR => self.pct_error,
            WEIGHT => self.weight,
            BASE_FREQ => self.med.cd.base_frequency,
            _ => unreachable!("Sensor has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KVBASE => self.kv_base = value,
            PCT_ERROR => self.pct_error = value,
            WEIGHT => self.weight = value,
            BASE_FREQ => self.med.cd.base_frequency = value,
            _ => unreachable!("Sensor has no double property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            // BooleanActionProperty stores nothing → always `No`.
            CLEAR => false,
            ENABLED => self.med.cd.enabled,
            _ => unreachable!("Sensor has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            CLEAR => {
                if value {
                    self.clear_sensor();
                }
            }
            ENABLED => self.med.cd.set_enabled(value),
            _ => unreachable!("Sensor has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            prop::ELEMENT => self.element_full_name.clone(),
            _ => unreachable!("Sensor has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            prop::ELEMENT => self.element_full_name = value,
            _ => unreachable!("Sensor has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use prop::*;
        // An unallocated array (Pascal NIL pointer) dumps as "" — return `None`
        // so the renderer reproduces that, rather than an empty `[]`.
        let arr: &[f64] = match idx {
            KVS => &self.med.sensor_voltage,
            CURRENTS => &self.med.sensor_current,
            KWS => &self.sensor_kw,
            KVARS => &self.sensor_kvar,
            _ => unreachable!("Sensor has no double-array property {idx}"),
        };
        (!arr.is_empty()).then_some(arr)
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use prop::*;
        match idx {
            KVS => self.med.sensor_voltage = value,
            CURRENTS => self.med.sensor_current = value,
            KWS => self.sensor_kw = value,
            KVARS => self.sensor_kvar = value,
            _ => unreachable!("Sensor has no double-array property {idx}"),
        }
    }
    fn array_size(&self, _idx: usize) -> usize {
        // All four measured arrays are sized to Fnphases.
        self.med.cd.nphases
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.med.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.med.cd.get_bus(terminal).to_string()
    }

    /// Resolve `element=` (any circuit class by full name): snapshot the metered
    /// element for `RecalcElementData`.
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        match idx {
            prop::ELEMENT => {
                self.element_full_name = name.clone();
                match resolved {
                    Some((r, obj)) => {
                        self.med.metered_element = Some(r);
                        self.med.metered_element_changed = true;
                        self.med.metered_snap = Some(capture(name, obj));
                    }
                    None => {
                        self.med.metered_element = None;
                        self.med.metered_snap = None;
                    }
                }
            }
            _ => unreachable!("Sensor has no object-ref property {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            ELEMENT | TERMINAL => {
                self.needs_recalc = true;
                self.med.metered_element_changed = true;
            }
            KVBASE => self.needs_recalc = true,
            KVS => self.v_specified = true,
            CURRENTS => self.i_specified = true,
            KWS => {
                self.p_specified = true;
                self.update_current_vector();
            }
            KVARS => {
                self.q_specified = true;
                self.update_current_vector();
            }
            CONN => {
                self.recalc_vbase();
                self.needs_recalc = true;
            }
            DELTA_DIRECTION => {
                self.f_delta_direction = if self.f_delta_direction >= 0 { 1 } else { -1 };
                self.needs_recalc = true;
            }
            _ => {}
        }
    }

    fn end_edit(&mut self) {
        // Pascal `EndEdit`: recalc only when a recalc-triggering datum changed.
        if !self.needs_recalc {
            return;
        }
        let mut errors = Vec::new();
        self.recalc(&mut errors);
        for e in errors {
            self.med.cd.obj.push_error(e);
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(o) = other.as_any().downcast_ref::<Sensor>() else {
            return;
        };
        // Pascal `TSensorObj.MakeLike` copies **only** the shape/metered fields
        // and the base frequency — kVBase/conn/%Error/Weight/DeltaDirection and
        // the measured arrays stay at the new object's ctor defaults / NIL.
        self.med.cd.make_like_base(&o.med.cd);
        self.med.cd.nphases = o.med.cd.nphases;
        self.med.cd.set_nconds(o.med.cd.nconds);
        self.med.metered_element = o.med.metered_element;
        self.med.metered_terminal = o.med.metered_terminal;
        self.med.metered_snap = o.med.metered_snap.clone();
        self.element_full_name = o.element_full_name.clone();
        self.med.cd.base_frequency = o.med.cd.base_frequency;
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pascal `RotatePhases`: 3-phase delta wraps in the `FDeltaDirection`
    /// direction; the 2-phase open-delta wrap sends `<1` to phase 3.
    #[test]
    fn rotate_phases_wraps() {
        let mut s = Sensor::new("r");
        s.med.cd.nphases = 3;
        s.f_delta_direction = 1;
        assert_eq!(s.rotate_phases(1), 2);
        assert_eq!(s.rotate_phases(2), 3);
        assert_eq!(s.rotate_phases(3), 1); // 4 > 3 → 1
        s.f_delta_direction = -1;
        assert_eq!(s.rotate_phases(1), 3); // 0 < 1 → nphases
        assert_eq!(s.rotate_phases(2), 1);
        assert_eq!(s.rotate_phases(3), 2);
        // 2-phase delta: a sub-1 result rolls to the (notional) 3rd phase.
        s.med.cd.nphases = 2;
        s.f_delta_direction = -1;
        assert_eq!(s.rotate_phases(1), 3);
    }

    /// Pascal `Get_WLSVoltageError`: weighted sum of `|CalcV|² − SensorV²`,
    /// zero unless `Vspecified`.
    #[test]
    fn wls_voltage_error_matches_formula() {
        let mut s = Sensor::new("v");
        s.med.cd.nphases = 2;
        s.weight = 2.0;
        s.med.calculated_voltage = vec![Complex64::new(3.0, 4.0), Complex64::new(1.0, 1.0)];
        s.med.sensor_voltage = vec![5.0, 1.0];
        // Not yet specified → 0.
        assert_eq!(s.wls_voltage_error(), 0.0);
        s.v_specified = true;
        // ((25 − 25) + (2 − 1)) · 2
        assert!((s.wls_voltage_error() - 2.0).abs() < 1e-12);
    }

    /// Pascal `Get_WLSCurrentError`: re-derives `SensorCurrent` from P/Q on
    /// `Vbase` (setting `Ispecified`), then weights `|CalcI|² − SensorI²`.
    #[test]
    fn wls_current_error_from_pq() {
        let mut s = Sensor::new("i");
        s.med.cd.nphases = 2;
        s.weight = 1.0;
        s.vbase = 1000.0;
        s.p_specified = true;
        s.q_specified = true;
        s.sensor_kw = vec![0.6, 0.0];
        s.sensor_kvar = vec![0.8, 0.0];
        s.med.sensor_current = vec![0.0; 2];
        s.med.calculated_current = vec![Complex64::new(2.0, 0.0), Complex64::ZERO];
        // |0.6 + 0.8j|·1000/1000 = 1.0 → SensorCurrent[0] = 1.0; Ispecified set.
        let err = s.wls_current_error();
        assert!(s.i_specified);
        assert!((s.med.sensor_current[0] - 1.0).abs() < 1e-12);
        // (4 − 1) + (0 − 0) = 3, ·weight 1.
        assert!((err - 3.0).abs() < 1e-12);
    }
}
