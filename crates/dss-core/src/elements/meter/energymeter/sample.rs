//! The register machinery: `ResetRegisters`, the `begin/end_take_sample` borrow
//! dance, and the `SampleState` accumulator with its `Integrate` /
//! `SetDragHandRegister` helpers.

use super::EnergyMeter;
use crate::circuit::ckt_tree::CktTree;
use crate::elements::traits::ElemId;

impl EnergyMeter {
    /// Pascal `TEnergyMeterObj.ResetRegisters`: zero the registers/derivatives
    /// and prime the drag-hand maxima to a large negative number.
    pub fn reset_registers(&mut self) {
        for r in &mut self.registers {
            *r = 0.0;
        }
        for d in &mut self.derivatives {
            *d = 0.0;
        }
        // Drag-hand registers (1-based EMRegister ordinals → 0-based here).
        for ord in [3, 4, 7, 8, 21, 22, 15, 16, 31, 32] {
            self.registers[ord - 1] = -1.0e50;
        }
        self.first_sample_after_reset = true;
    }

    /// Pascal `CheckBranchList`: `TakeSample` exits early when the zone was
    /// never built. Returns the sweep state (config snapshot + the registers
    /// and branch tree moved out for the walk) or `None`.
    pub(crate) fn begin_take_sample(
        &mut self,
        trapezoidal: bool,
    ) -> Option<(CktTree, SampleState)> {
        let tree = self.branch_list.take()?;
        let state = SampleState {
            local_only: self.local_only,
            zone_is_radial: self.zone_is_radial,
            excess_flag: self.excess_flag,
            voltage_ue_only: self.voltage_ue_only,
            f_losses: self.f_losses,
            f_line_losses: self.f_line_losses,
            f_xfmr_losses: self.f_xfmr_losses,
            f_seq_losses: self.f_seq_losses,
            f_3phase_losses: self.f_3phase_losses,
            f_vbase_losses: self.f_vbase_losses,
            f_phase_voltage_report: self.f_phase_voltage_report,
            vbase_list: self.vbase_list.clone(),
            max_zone_kva_norm: self.max_zone_kva_norm,
            max_zone_kva_emerg: self.max_zone_kva_emerg,
            metered_element: self.med.metered_element,
            metered_terminal: self.med.metered_terminal.max(0) as usize,
            registers: std::mem::take(&mut self.registers),
            derivatives: std::mem::take(&mut self.derivatives),
            vphase_max: std::mem::take(&mut self.vphase_max),
            vphase_min: std::mem::take(&mut self.vphase_min),
            vphase_accum: std::mem::take(&mut self.vphase_accum),
            vphase_accum_count: std::mem::take(&mut self.vphase_accum_count),
            first_sample_after_reset: self.first_sample_after_reset,
            trapezoidal,
        };
        Some((tree, state))
    }

    /// Restore the branch tree and write back the accumulated registers.
    pub(crate) fn end_take_sample(&mut self, tree: CktTree, state: SampleState) {
        self.branch_list = Some(tree);
        self.registers = state.registers;
        self.derivatives = state.derivatives;
        self.vphase_max = state.vphase_max;
        self.vphase_min = state.vphase_min;
        self.vphase_accum = state.vphase_accum;
        self.vphase_accum_count = state.vphase_accum_count;
        self.first_sample_after_reset = state.first_sample_after_reset;
    }
}

/// Mutable sweep state for `TakeSample`: the config snapshot plus the register
/// accumulators moved out of the meter for the duration of the zone walk
/// (the meter object itself is borrowed by the element store during the walk).
pub(crate) struct SampleState {
    pub local_only: bool,
    pub zone_is_radial: bool,
    pub excess_flag: bool,
    pub voltage_ue_only: bool,
    pub f_losses: bool,
    pub f_line_losses: bool,
    pub f_xfmr_losses: bool,
    pub f_seq_losses: bool,
    pub f_3phase_losses: bool,
    pub f_vbase_losses: bool,
    pub f_phase_voltage_report: bool,
    /// The meter's voltage-base list (selects which `jiIndex` slots are live).
    pub vbase_list: Vec<f64>,
    pub max_zone_kva_norm: f64,
    pub max_zone_kva_emerg: f64,
    pub metered_element: Option<ElemId>,
    pub metered_terminal: usize,
    pub registers: Vec<f64>,
    pub derivatives: Vec<f64>,
    /// Phase-voltage report accumulators (`jiIndex` layout), moved out with
    /// the registers for the walk.
    pub vphase_max: Vec<f64>,
    pub vphase_min: Vec<f64>,
    pub vphase_accum: Vec<f64>,
    pub vphase_accum_count: Vec<i32>,
    pub first_sample_after_reset: bool,
    pub trapezoidal: bool,
}

impl SampleState {
    /// Pascal `TEnergyMeterObj.Integrate` (l.1271): trapezoidal rule when the
    /// circuit flag is set (skipped on the first sample after reset), else plain
    /// Euler. `reg` is the 1-based ordinal. Always records the derivative.
    pub(crate) fn integrate(&mut self, reg: usize, deriv: f64, interval: f64) {
        let i = reg - 1;
        if self.trapezoidal {
            if !self.first_sample_after_reset {
                self.registers[i] += 0.5 * interval * (deriv + self.derivatives[i]);
            }
        } else {
            self.registers[i] += interval * deriv;
        }
        self.derivatives[i] = deriv;
    }

    /// Pascal `TEnergyMeterObj.SetDragHandRegister` (l.2858): keep the running
    /// maximum.
    pub(crate) fn set_drag(&mut self, reg: usize, value: f64) {
        let i = reg - 1;
        if value > self.registers[i] {
            self.registers[i] = value;
            self.derivatives[i] = value;
        }
    }
}
