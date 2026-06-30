//! The LineCode matrix algorithms: `CalcMatricesFromZ1Z0`, `Set_NumPhases`
//! and `DoKronReduction`. Split out of `line_code/mod.rs` (no behavioral
//! change).

use num_complex::Complex64;

use crate::support::cmatrix::CMatrix;

use super::{LineCodeObj, TWO_PI, prop};

impl LineCodeObj {
    fn full_name(&self) -> String {
        format!("LineCode.{}", self.data.name())
    }

    fn nphases_usize(&self) -> usize {
        self.fnphases.max(0) as usize
    }

    /// Pascal `CalcMatricesFromZ1Z0`. Note: no 1-phase special case — a
    /// 1-phase code still folds in R0/X0/C0.
    pub(super) fn calc_matrices_from_z1z0(&mut self) {
        let n = self.nphases_usize();
        let mut z = CMatrix::new(n);
        let mut zinv = CMatrix::new(n);
        let mut yc = CMatrix::new(n);

        let one_third = 1.0 / 3.0; // extra precision in the next statements
        let ztemp = Complex64::new(self.r1, self.x1) * 2.0;
        let zs = (ztemp + Complex64::new(self.r0, self.x0)) * one_third;
        let zm = (Complex64::new(self.r0, self.x0) - Complex64::new(self.r1, self.x1)) * one_third;

        let yc1 = TWO_PI * self.base_frequency * self.c1;
        let yc0 = TWO_PI * self.base_frequency * self.c0;
        let ys = (Complex64::new(0.0, yc1) * 2.0 + Complex64::new(0.0, yc0)) * one_third;
        let ym = (Complex64::new(0.0, yc0) - Complex64::new(0.0, yc1)) * one_third;

        for i in 0..n {
            z.set(i, i, zs);
            yc.set(i, i, ys);
            for j in 0..i {
                z.set(i, j, zm);
                z.set(j, i, zm);
                yc.set(i, j, ym);
                yc.set(j, i, ym);
            }
        }
        zinv.copy_from(&z);
        let _ = zinv.invert();
        self.z = Some(z);
        self.zinv = Some(zinv);
        self.yc = Some(yc);
    }

    /// Pascal `Set_NumPhases`: realloc the phase-sensitive matrices when the
    /// order actually changes.
    fn set_num_phases(&mut self, value: i32) {
        if value > 0 && self.fnphases != value {
            self.fnphases = value;
            self.fneutral_conductor = self.fnphases;
            self.calc_matrices_from_z1z0();
        }
    }

    /// Pascal `DoKronReduction`: eliminate the neutral conductor from `Z`/`YC`.
    pub(super) fn do_kron_reduction(&mut self) {
        if self.sym_components_model {
            return;
        }
        if self.fneutral_conductor == 0 {
            return; // do nothing
        }
        if self.fnphases <= 1 {
            self.data.push_error(format!(
                "Cannot perform Kron Reduction on a 1-phase LineCode: {}",
                self.full_name()
            ));
            return;
        }

        // Pascal `TcMatrix.Kron` takes a 1-based conductor index.
        let elim = (self.fneutral_conductor - 1).max(0) as usize;
        let new_z = self.z.as_ref().and_then(|z| z.kron(elim));
        // Vn = 0, not In: invert YC in place, Kron, invert back.
        if let Some(yc) = self.yc.as_mut() {
            let _ = yc.invert();
        }
        let new_yc = self.yc.as_ref().and_then(|yc| yc.kron(elim));

        let (Some(new_z), Some(mut new_yc)) = (new_z, new_yc) else {
            self.data.push_error(format!(
                "Kron Reduction failed: {}. Attempting to eliminate Neutral Conductor {}.",
                self.full_name(),
                self.fneutral_conductor
            ));
            return;
        };
        let _ = new_yc.invert(); // back to Y

        self.set_num_phases(new_z.order() as i32);

        self.z = Some(new_z);
        self.yc = Some(new_yc);
        self.fneutral_conductor = 0;

        // Reflect the reduction in the property set-order for `save`.
        for p in [prop::NPHASES, prop::RMATRIX, prop::XMATRIX, prop::CMATRIX] {
            self.data.set_as_next_seq(p);
        }
    }
}
