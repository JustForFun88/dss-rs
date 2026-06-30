//! Energy-meter registers (`TakeSample` / `Integrate`) and the present-output
//! accessors. Unlike the Generator, the PVSystem always integrates a sample
//! (there is no on/off gate around the accumulation).

use num_complex::Complex64;

use super::{
    NUM_PVSYSTEM_REGISTERS, PVSystem, REG_HOURS, REG_KVARH, REG_KWH, REG_MAXKVA, REG_MAXKW,
    REG_PRICE,
};

impl PVSystem {
    /// Pascal `ResetRegisters`.
    pub fn reset_registers(&mut self) {
        self.registers = [0.0; NUM_PVSYSTEM_REGISTERS];
        self.derivatives = [0.0; NUM_PVSYSTEM_REGISTERS];
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

    /// Pascal `Get_PresentIrradiance`.
    pub fn present_irradiance(&self) -> f64 {
        self.f_irradiance * self.base.shape_factor.re
    }

    /// Pascal `TakeSample`: accumulate the PVSystem energy registers.
    pub fn take_sample(
        &mut self,
        interval_hrs: f64,
        trapezoidal: bool,
        positive_sequence: bool,
        price_signal: f64,
    ) {
        if !self.cd.enabled {
            return;
        }
        let mut s = Complex64::new(self.present_kw(), self.present_kvar());
        let mut smag = s.norm();
        let hour_value = 1.0;

        if positive_sequence {
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
            s.re * price_signal * 0.001,
            interval_hrs,
            trapezoidal,
        );
        self.base.first_sample_after_reset = false;
    }

    /// Pascal `UpdatePVSystem`: "Do Nothing" (the integrated-state hook the
    /// time-step cleanup calls; Storage uses it, PVSystem does not).
    pub fn update_pvsystem(&mut self) {}
}
