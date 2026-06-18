//! Energy-meter registers (`TakeSample` / `Integrate`), fuel accounting and the
//! Model-3 DQDV var-control machinery the solution object drives.

use num_complex::Complex64;

use super::{
    Generator, NUM_GEN_REGISTERS, REG_HOURS, REG_KVARH, REG_KWH, REG_MAXKVA, REG_MAXKW, REG_PRICE,
};

impl Generator {
    /// Pascal `ResetRegisters`.
    pub fn reset_registers(&mut self) {
        self.registers = [0.0; NUM_GEN_REGISTERS];
        self.derivatives = [0.0; NUM_GEN_REGISTERS];
        self.first_sample_after_reset = true;
    }

    /// Pascal `Integrate`.
    fn integrate(&mut self, reg: usize, deriv: f64, interval: f64, trapezoidal: bool) {
        if trapezoidal {
            if !self.first_sample_after_reset {
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
        self.p_nominal_per_phase * 0.001 * self.cd.nphases as f64
    }

    /// Pascal `Get_Presentkvar`.
    pub fn present_kvar(&self) -> f64 {
        self.q_nominal_per_phase * 0.001 * self.cd.nphases as f64
    }

    /// Pascal `CheckOnFuel`.
    fn check_on_fuel(&mut self, deriv: f64, interval: f64) -> bool {
        self.pct_fuel = ((((self.pct_fuel / 100.0) * self.fuel_kwh) - interval * deriv)
            / self.fuel_kwh)
            * 100.0;
        if self.pct_fuel <= self.pct_reserve {
            self.pct_fuel = self.pct_reserve;
            return false;
        }
        true
    }

    /// Pascal `TakeSample`: accumulate the generator's energy registers.
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
        let (mut s, mut smag, hour_value) = if self.gen_on {
            let s = Complex64::new(self.present_kw(), self.present_kvar());
            (s, s.norm(), 1.0)
        } else {
            (Complex64::ZERO, 0.0, 0.0)
        };

        if self.gen_on || trapezoidal {
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
            self.first_sample_after_reset = false;
            if self.use_fuel {
                self.gen_active = self.check_on_fuel(s.re, interval_hrs);
            }
        }
    }

    // --- Model-3 DQDV machinery (driven by the solution object) -----------

    /// Pascal `InitDQDVCalc`.
    pub fn init_dqdv_calc(&mut self) {
        self.dqdv = 0.0;
        self.q_nominal_per_phase = 0.5 * (self.var_max + self.var_min);
    }

    /// Pascal `CalcDQDV` — `yii` is the system Y diagonal at the generator's
    /// first node (the solution object reads it from the assembled matrix).
    pub fn calc_dqdv(&mut self, yii_abs: f64) {
        self.dqdv = 2.0 * yii_abs * self.v_base * self.vpu;
        self.dqdv_saved = self.dqdv;
    }

    /// Pascal `ResetStartPoint`.
    pub fn reset_start_point(&mut self) {
        self.q_nominal_per_phase = 1000.0 * self.kvar_base / self.cd.nphases as f64;
    }

    /// The generator's first conductor's global node reference (the DQDV sweep
    /// reads the Y diagonal here).
    pub fn first_node_ref(&self) -> usize {
        self.cd.node_ref.first().copied().unwrap_or(0)
    }
}
