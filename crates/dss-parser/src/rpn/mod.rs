use std::f64::consts::PI;

const MAX_STACK_SIZE: usize = 10;

#[derive(Debug)]
pub struct RPNCalculator {
    stack: [f64; MAX_STACK_SIZE],
}

impl RPNCalculator {
    // TODO(compat): truncated pi reproduced from the Pascal original
    // (RPN.pas) — NOT the full-precision constant. Results differ in the
    // last ~3 digits (e.g. "30 sin" gives 0.5000000000000299) and the golden
    // parser tests pin that behavior. EnterPi, by contrast, pushes FPC's
    // full-precision `pi` builtin. Replace with f64::consts::PI (and update
    // the goldens) once the 1:1 port is complete.
    #[allow(clippy::approx_constant)]
    const DEG_TO_RAD: f64 = 3.14159265359 / 180.0;
    #[allow(clippy::approx_constant)]
    const RAD_TO_DEG: f64 = 180.0 / 3.14159265359;

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

#[cfg(test)]
mod tests;
