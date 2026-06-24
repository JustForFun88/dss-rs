//! State-of-charge integration (`ComputeDCkW` / `UpdateStorage` + the loss
//! split), the energy-meter registers (`TakeSample` / `Integrate`), and the
//! present-output accessors. The SOC update runs in the time-step cleanup hook
//! (Pascal `StorageClass.UpdateAll` → `UpdateStorage`).

use num_complex::Complex64;

use crate::elements::traits::{CktElement, SysCtx};
use crate::util::quad_solver;

use super::{
    NUM_STORAGE_REGISTERS, REG_HOURS, REG_KVARH, REG_KWH, REG_MAXKVA, REG_MAXKW, REG_PRICE,
    STORE_CHARGING, STORE_DISCHARGING, STORE_IDLING, Storage,
};

impl Storage {
    /// Pascal `ResetRegisters`.
    pub fn reset_registers(&mut self) {
        self.registers = [0.0; NUM_STORAGE_REGISTERS];
        self.derivatives = [0.0; NUM_STORAGE_REGISTERS];
        self.base.first_sample_after_reset = true; // for trapezoidal integration
    }

    /// Pascal `Integrate`.
    fn integrate(&mut self, reg: usize, deriv: f64, interval: f64, trapezoidal: bool) {
        if trapezoidal {
            if !self.base.first_sample_after_reset {
                self.registers[reg] += 0.5 * interval * (deriv + self.derivatives[reg]);
            }
        } else {
            self.registers[reg] += interval * deriv;
        }
        self.derivatives[reg] = deriv;
    }

    /// Pascal `SetDragHandRegister`.
    fn set_drag_hand_register(&mut self, reg: usize, value: f64) {
        if value > self.registers[reg] {
            self.registers[reg] = value;
        }
    }

    /// Pascal `Get_PresentkW`.
    pub fn present_kw(&self) -> f64 {
        self.base.p_nominal_per_phase * 0.001 * self.cd.nphases as f64
    }

    /// Pascal `Get_Presentkvar` (on the inverter base).
    pub fn present_kvar(&self) -> f64 {
        self.base.get_present_kvar(self.cd.nphases)
    }

    /// Pascal `ComputeDCkW`: the actual DC-side kW used to update the SOC. With
    /// an ideal inverter (no efficiency curve) it is the signed AC terminal kW;
    /// with a curve it solves the efficiency relation for the DC power.
    fn compute_dckw(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let p1 = self.terminal_power(sys, node_v, 1).re * 0.001; // Power[1] in kW

        if self.base.inverter_curve_obj.is_none() {
            self.f_dckw = p1; // assume ideal inverter
            // Make sure the sign is correct.
            self.f_dckw = if self.f_state == STORE_IDLING {
                -self.f_dckw.abs()
            } else {
                self.f_dckw.abs() * self.f_state as f64
            };
            return;
        }

        self.f_dckw = p1;
        let kva = self.f_kva_rating;
        // Pascal's loop condition is `(a≠guess.a AND b≠guess.b) OR (N>9)` — with
        // FPC precedence the `OR (N>9)` only re-enters; for any locally-consistent
        // (monotonic) efficiency curve the guess converges to the same segment
        // (exact-float) within a couple of iterations, so the `N>9` arm is never
        // reached. Ported verbatim.
        let mut guess = (0.0_f64, 0.0_f64);
        let mut coef = (1.0_f64, 1.0_f64);
        let mut n_tentatives = 0;
        while ((coef.0 != guess.0) && (coef.1 != guess.1)) || (n_tentatives > 9) {
            n_tentatives += 1;
            guess = self
                .base
                .inverter_curve_obj
                .as_mut()
                .unwrap()
                .get_coefficients(self.f_dckw.abs() / kva);
            self.f_dckw = match self.f_state {
                STORE_DISCHARGING => quad_solver(guess.0 / kva, guess.1, -p1.abs()),
                _ => self.f_dckw.abs() * guess.1 / (1.0 - (guess.0 * self.f_dckw.abs() / kva)),
            };
            coef = self
                .base
                .inverter_curve_obj
                .as_mut()
                .unwrap()
                .get_coefficients(self.f_dckw.abs() / kva);
        }

        // Make sure the sign is correct.
        self.f_dckw = if self.f_state == STORE_IDLING {
            -self.f_dckw.abs()
        } else {
            self.f_dckw.abs() * self.f_state as f64
        };
    }

    /// Pascal `Get_kWIdlingLosses`.
    fn kw_idling_losses(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        if self.f_state == STORE_IDLING {
            self.dckw(sys, node_v).abs() // consistency with voltage variations
        } else {
            self.p_idling
        }
    }

    /// `DCkW` property (recomputes `ComputeDCkW`, like the Pascal `Get_DCkW`).
    fn dckw(&mut self, sys: &SysCtx, node_v: &[Complex64]) -> f64 {
        self.compute_dckw(sys, node_v);
        self.f_dckw
    }

    /// Pascal `UpdateStorage`: advance the state of charge by one interval, based
    /// on the present DC power and the idling/charge/discharge efficiencies.
    /// Runs in the time-step cleanup hook with a solved circuit.
    pub fn update_storage(&mut self, sys: &SysCtx, node_v: &[Complex64], interval_hrs: f64) {
        self.kwh_before_update = self.kwh_stored; // keep for the "kWh Chng" variable

        // The user model handles SOC in dynamics mode (never ported here).
        if sys.is_dynamic_model {
            return;
        }

        match self.f_state {
            STORE_DISCHARGING => {
                // GFM "delivering" check is WP7.7; non-GFM always updates.
                let dckw = self.dckw(sys, node_v);
                let idle = self.kw_idling_losses(sys, node_v);
                self.kwh_stored -= (dckw + idle) / self.discharge_eff * interval_hrs;
                // Check we still have enough energy to deliver.
                if self.kwh_stored < self.kwh_reserve {
                    self.kwh_stored = self.kwh_reserve;
                    self.f_state = STORE_IDLING; // empty — turn it off
                    self.state_changed = true;
                    self.base.gfm_mode = false;
                }
            }
            STORE_CHARGING => {
                let dckw = self.dckw(sys, node_v);
                let idle = self.kw_idling_losses(sys, node_v);
                if (dckw.abs() - idle) >= 0.0 {
                    // 99.9% of cases.
                    self.kwh_stored += (dckw.abs() - idle) * self.charge_eff * interval_hrs;
                    if self.kwh_stored > self.kwh_rating {
                        self.kwh_stored = self.kwh_rating;
                        self.f_state = STORE_IDLING; // full — turn it off
                        self.state_changed = true;
                        self.base.gfm_mode = false;
                    }
                } else {
                    // Idling losses exceed DCkW → the ideal storage discharges.
                    self.kwh_stored += (dckw.abs() - idle) / self.discharge_eff * interval_hrs;
                    if self.kwh_stored < self.kwh_reserve {
                        self.kwh_stored = self.kwh_reserve;
                        self.f_state = STORE_IDLING;
                        self.state_changed = true;
                    }
                }
            }
            _ => {} // idling: SOC unchanged
        }

        // The update is at the end of a time step, so force a Yprim re-calc for
        // the next step (else the state-dependent Y would stay stale).
        if self.state_changed {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TakeSample`: accumulate the Storage energy registers. Only
    /// discharge hours are tabulated.
    pub fn take_sample(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        interval_hrs: f64,
        trapezoidal: bool,
    ) {
        if !self.cd.enabled {
            return;
        }
        // Only tabulate discharge hours.
        let (mut s, mut smag, hour_value) = if self.f_state == STORE_DISCHARGING {
            let _ = (sys, node_v);
            let s = Complex64::new(self.present_kw(), self.present_kvar());
            (s, s.norm(), 1.0)
        } else {
            (Complex64::ZERO, 0.0, 0.0)
        };

        if self.f_state == STORE_DISCHARGING || trapezoidal {
            if sys.positive_sequence {
                s *= 3.0;
                smag *= 3.0;
            }
            self.integrate(REG_KWH, s.re, interval_hrs, trapezoidal);
            self.integrate(REG_KVARH, s.im, interval_hrs, trapezoidal);
            self.set_drag_hand_register(REG_MAXKW, s.re.abs());
            self.set_drag_hand_register(REG_MAXKVA, smag);
            self.integrate(REG_HOURS, hour_value, interval_hrs, trapezoidal);
            self.integrate(
                REG_PRICE,
                s.re * sys.price_signal * 0.001,
                interval_hrs,
                trapezoidal,
            );
            self.base.first_sample_after_reset = false;
        }
    }
}
