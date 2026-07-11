//! The sensor algorithms: `ClearSensor`, `RecalcVbase`, `RotatePhases`,
//! array allocation/zeroing, `UpdateCurrentVector`, `RecalcElementData`,
//! `TakeSample` and the WLS error getters.

use num_complex::Complex64;

use crate::elements::traits::{CktElement, SysCtx};
use crate::util::sqrt3;

use super::Sensor;

impl Sensor {
    /// Pascal `DoClearSensor`: reset the spec flags.
    pub(super) fn clear_sensor(&mut self) {
        self.v_specified = false;
        self.i_specified = false;
        self.p_specified = false;
        self.q_specified = false;
    }

    /// Pascal `RecalcVbase`: wye → L-N (÷√3 for poly-phase), delta → L-L.
    pub(super) fn recalc_vbase(&mut self) {
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
    pub(super) fn rotate_phases(&self, j: usize) -> usize {
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
    /// arrays to `Fnphases` and zero all four measured vectors. `yorder` is the
    /// metered element's `Yorder` (the calc-buffer size).
    pub(super) fn allocate_and_zero_arrays(&mut self, yorder: usize) {
        let nph = self.med.cd.nphases;
        // AllocateSensorArrays sizes med.sensor_current/sensor_voltage; the
        // metered calc buffers follow the metered element's Yorder.
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
    pub(super) fn update_current_vector(&mut self) {
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
        self.allocate_and_zero_arrays(snap.yorder);
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
