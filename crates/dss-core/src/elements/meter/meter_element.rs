//! Port of `Meters/MeterElement.pas` — `TMeterElement`, the shared base of
//! every metering device (Monitor, EnergyMeter, Sensor). A meter element is a
//! `TDSSCktElement` with **no** Yprim (`TMonitorObj.CalcYPrim` is empty): it
//! observes one circuit element's terminal but never stamps admittance and its
//! terminal currents are always zero.
//!
//! Phase 6 (WP6.3) ports the parse-time surface plus the sensor-allocation
//! helpers (`AllocateSensorArrays`/`CalcAllocationFactors`, consumed by Sensor
//! in WP6.7). The behavioral `TakeSample` is each subclass's own override.

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, ElemRef};

/// Which class the monitored element belongs to — captured when `element=` is
/// resolved so `RecalcElementData`/`ClearMonitorStream` can validate the mode
/// and size the header without the live element (resolution runs while the
/// foreign-class view is in scope; recalc runs later at `EndEdit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeteredKind {
    #[default]
    Other,
    Transformer,
    Capacitor,
    PcElement,
    Storage,
}

/// A read-only snapshot of the monitored circuit element, captured when the
/// `element=` reference is resolved during parsing (the WP4.7 `RefSnapshot`
/// pattern, extended with the fields the meter header/sample paths need).
#[derive(Debug, Clone, Default)]
pub struct MeteredSnapshot {
    /// `FullName` (`Class.name`) for the dump and error messages.
    pub full_name: String,
    pub kind: MeteredKind,
    pub nphases: usize,
    pub nconds: usize,
    pub nterms: usize,
    pub yorder: usize,
    /// 1-based terminal bus names, stored 0-based (`buses[term - 1]`).
    pub buses: Vec<String>,
    /// `TransformerObj.NumWindings` (modes 8/10 header sizing).
    pub num_windings: usize,
    /// `CapacitorObj.NumSteps` (mode 6 header sizing).
    pub num_steps: usize,
    /// `PCElement.NumVariables` (mode 3 header sizing; 0 in Phase 6 — the
    /// dynamics state set is Phase 7).
    pub num_variables: usize,
}

/// `TMeterElement` shared state (the base-class fields every meter carries).
/// Embeds [`CktElementData`] exactly as `TMeterElement` extends
/// `TDSSCktElement`.
#[derive(Debug, Clone)]
pub struct MeterElementData {
    pub cd: CktElementData,
    /// `MeteredElement` (the device this meter samples).
    pub metered_element: Option<ElemRef>,
    /// `MeteredTerminal` (1-based).
    pub metered_terminal: i32,
    /// `MeteredElementChanged`.
    pub metered_element_changed: bool,
    /// Parse-time snapshot of the monitored element.
    pub metered_snap: Option<MeteredSnapshot>,

    // Sensor-allocation arrays (`AllocateSensorArrays`; consumed by Sensor in
    // WP6.7). Sized to `Fnphases`.
    pub sensor_current: Vec<f64>,
    pub sensor_voltage: Vec<f64>,
    pub phs_allocation_factor: Vec<f64>,
    pub calculated_current: Vec<Complex64>,
    pub calculated_voltage: Vec<Complex64>,
    /// `AvgAllocFactor`.
    pub avg_alloc_factor: f64,
}

impl MeterElementData {
    /// Pascal `TMeterElement.Create`: no metered element yet, terminal 1.
    pub fn new(name: &str, num_props: usize) -> Self {
        Self {
            cd: CktElementData::new(name, num_props),
            metered_element: None,
            metered_terminal: 1,
            metered_element_changed: false,
            metered_snap: None,
            sensor_current: Vec::new(),
            sensor_voltage: Vec::new(),
            phs_allocation_factor: Vec::new(),
            calculated_current: Vec::new(),
            calculated_voltage: Vec::new(),
            avg_alloc_factor: 0.0,
        }
    }

    /// Pascal `TMeterElement.AllocateSensorArrays`: size the per-phase sensor
    /// arrays and the metered-element calc buffers (Sensor, WP6.7).
    #[allow(dead_code)]
    pub fn allocate_sensor_arrays(&mut self, metered_yorder: usize) {
        self.calculated_current = vec![Complex64::ZERO; metered_yorder];
        self.calculated_voltage = vec![Complex64::ZERO; metered_yorder];
        let nph = self.cd.nphases;
        self.sensor_current = vec![0.0; nph];
        self.sensor_voltage = vec![0.0; nph];
        self.phs_allocation_factor = vec![0.0; nph];
    }

    /// Pascal `TMeterElement.CalcAllocationFactors`: the per-phase factor each
    /// load must scale to match the measured peak current (`metered` supplies
    /// `GetCurrents` into `calculated_current`).
    #[allow(dead_code)]
    pub fn calc_allocation_factors(
        &mut self,
        metered: &mut dyn CktElement,
        sys: &crate::elements::traits::SysCtx,
        node_v: &[Complex64],
    ) {
        metered.get_currents(sys, node_v, &mut self.calculated_current);
        let nconds = metered.cd().nconds;
        let offset = (self.metered_terminal as usize - 1) * nconds;
        self.avg_alloc_factor = 0.0;
        let nph = self.cd.nphases;
        for i in 0..nph {
            let mag = self.calculated_current[i + offset].norm();
            self.phs_allocation_factor[i] = if mag > 0.0 {
                self.sensor_current[i] / mag
            } else {
                1.0
            };
            self.avg_alloc_factor += self.phs_allocation_factor[i];
        }
        if nph > 0 {
            self.avg_alloc_factor /= nph as f64;
        }
    }
}
