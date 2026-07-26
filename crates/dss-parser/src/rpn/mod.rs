#[cfg(test)]
mod tests;

use std::f64::consts::PI;

const MAX_STACK_SIZE: usize = 10;

#[derive(Debug)]
pub struct RPNCalculator {
    stack: [f64; MAX_STACK_SIZE],
}

impl RPNCalculator {
    // The degree conversions scale by the pi of `compat::PI` — the parity lane
    // reproduces the Pascal original's *truncated* literal (RPN.pas), the
    // default lane uses `f64::consts::PI` from F.3 on. `EnterPi`, by contrast,
    // pushes FPC's full-precision `pi` builtin in both lanes.
    const DEG_TO_RAD: f64 = crate::compat::PI / 180.0;
    const RAD_TO_DEG: f64 = 180.0 / crate::compat::PI;

    pub fn new() -> Self {
        RPNCalculator {
            stack: [0.0; MAX_STACK_SIZE],
        }
    }

    pub fn get_x(&self) -> f64 {
        self.stack[0] // Pascal FStack[1] = Rust stack[0]
    }

    pub fn get_y(&self) -> f64 {
        self.stack[1] // Pascal FStack[2] = Rust stack[1]
    }

    pub fn get_z(&self) -> f64 {
        self.stack[2] // Pascal FStack[3] = Rust stack[2]
    }

    pub fn set_x(&mut self, value: f64) {
        self.roll_up();
        self.stack[0] = value;
    }

    pub fn set_y(&mut self, value: f64) {
        self.stack[1] = value;
    }

    pub fn set_z(&mut self, value: f64) {
        self.stack[2] = value;
    }

    pub fn add(&mut self) {
        self.stack[1] += self.stack[0];
        self.roll_down();
    }

    pub fn subtract(&mut self) {
        self.stack[1] -= self.stack[0];
        self.roll_down();
    }

    pub fn multiply(&mut self) {
        self.stack[1] *= self.stack[0];
        self.roll_down();
    }

    pub fn divide(&mut self) {
        self.stack[1] /= self.stack[0];
        self.roll_down();
    }

    pub fn sqrt(&mut self) {
        self.stack[0] = self.stack[0].sqrt();
    }

    pub fn square(&mut self) {
        self.stack[0] = self.stack[0] * self.stack[0];
    }

    pub fn y_to_the_x_power(&mut self) {
        self.stack[1] = self.stack[1].powf(self.stack[0]);
        self.roll_down();
    }

    pub fn inv(&mut self) {
        self.stack[0] = 1.0 / self.stack[0];
    }

    pub fn sin_deg(&mut self) {
        self.stack[0] = (Self::DEG_TO_RAD * self.stack[0]).sin();
    }

    pub fn cos_deg(&mut self) {
        self.stack[0] = (Self::DEG_TO_RAD * self.stack[0]).cos();
    }

    pub fn tan_deg(&mut self) {
        self.stack[0] = (Self::DEG_TO_RAD * self.stack[0]).tan();
    }

    pub fn asin_deg(&mut self) {
        self.stack[0] = Self::RAD_TO_DEG * self.stack[0].asin();
    }

    pub fn acos_deg(&mut self) {
        self.stack[0] = Self::RAD_TO_DEG * self.stack[0].acos();
    }

    pub fn atan_deg(&mut self) {
        self.stack[0] = Self::RAD_TO_DEG * self.stack[0].atan();
    }

    pub fn atan2_deg(&mut self) {
        self.stack[1] = Self::RAD_TO_DEG * self.stack[1].atan2(self.stack[0]);
        self.roll_down();
    }

    pub fn nat_log(&mut self) {
        self.stack[0] = self.stack[0].ln();
    }

    pub fn ten_log(&mut self) {
        self.stack[0] = self.stack[0].log10();
    }

    pub fn etothex(&mut self) {
        self.stack[0] = self.stack[0].exp();
    }

    pub fn enter_pi(&mut self) {
        self.roll_up();
        self.stack[0] = PI;
    }

    pub fn swap_xy(&mut self) {
        self.stack.swap(0, 1);
    }

    pub fn roll_up(&mut self) {
        self.stack.copy_within(0..MAX_STACK_SIZE - 1, 1);
    }

    pub fn roll_down(&mut self) {
        self.stack.copy_within(1.., 0);
    }
}

impl Default for RPNCalculator {
    fn default() -> Self {
        Self::new()
    }
}
