//! The `DssObject` trait impl for `LineCodeObj`: typed property accessors,
//! matrix get/set, `prop_scale`/`prop_conditional`, `PropertySideEffects`,
//! `EndEdit` and `MakeLike`. Split out of `line_code/mod.rs` (no behavioral
//! change).

use crate::obj::base::{DssObjData, DssObject};
use crate::support::cmatrix::CMatrix;

use super::{LineCodeObj, LineType, TWO_PI, prop};

impl LineCodeObj {
    /// Pascal `TLineCodeObj.MakeLike`.
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.data.copy_prp_sequence_from(other.data());
        let o = other;
        self.fnphases = o.fnphases;
        self.z = o.z.clone();
        self.zinv = o.zinv.clone();
        self.yc = o.yc.clone();
        self.base_frequency = o.base_frequency;
        self.r1 = o.r1;
        self.x1 = o.x1;
        self.r0 = o.r0;
        self.x0 = o.x0;
        self.c1 = o.c1;
        self.c0 = o.c0;
        self.rg = o.rg;
        self.xg = o.xg;
        self.rho = o.rho;
        self.fneutral_conductor = o.fneutral_conductor;
        self.norm_amps = o.norm_amps;
        self.emerg_amps = o.emerg_amps;
        self.fault_rate = o.fault_rate;
        self.pct_perm = o.pct_perm;
        self.hrs_to_repair = o.hrs_to_repair;
    }
}

impl DssObject for LineCodeObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            R1 => self.r1,
            X1 => self.x1,
            R0 => self.r0,
            X0 => self.x0,
            C1 | B1 => self.c1,
            C0 | B0 => self.c0,
            BASE_FREQ => self.base_frequency,
            NORMAMPS => self.norm_amps,
            EMERGAMPS => self.emerg_amps,
            FAULTRATE => self.fault_rate,
            PCTPERM => self.pct_perm,
            REPAIR => self.hrs_to_repair,
            RG => self.rg,
            XG => self.xg,
            RHO => self.rho,
            _ => unreachable!("LineCode has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            R1 => self.r1 = value,
            X1 => self.x1 = value,
            R0 => self.r0 = value,
            X0 => self.x0 = value,
            C1 | B1 => self.c1 = value,
            C0 | B0 => self.c0 = value,
            BASE_FREQ => self.base_frequency = value,
            NORMAMPS => self.norm_amps = value,
            EMERGAMPS => self.emerg_amps = value,
            FAULTRATE => self.fault_rate = value,
            PCTPERM => self.pct_perm = value,
            REPAIR => self.hrs_to_repair = value,
            RG => self.rg = value,
            XG => self.xg = value,
            RHO => self.rho = value,
            _ => unreachable!("LineCode has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            NPHASES => self.fnphases,
            UNITS => self.units,
            NEUTRAL => self.fneutral_conductor,
            SEASONS => self.num_amp_ratings,
            LINE_TYPE => self.fline_type.ordinal(),
            _ => unreachable!("LineCode has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            NPHASES => self.fnphases = value,
            UNITS => self.units = value,
            NEUTRAL => self.fneutral_conductor = value,
            SEASONS => self.num_amp_ratings = value,
            LINE_TYPE => self.fline_type = LineType::from_ordinal(value).unwrap_or(self.fline_type),
            _ => unreachable!("LineCode has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            // The Kron action property never reports back as set.
            prop::KRON => false,
            _ => unreachable!("LineCode has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::KRON => self.kron_pending = value,
            _ => unreachable!("LineCode has no boolean property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::RATINGS => Some(&self.amp_ratings),
            _ => unreachable!("LineCode has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::RATINGS => self.amp_ratings = value,
            _ => unreachable!("LineCode has no array property {idx}"),
        }
    }

    fn set_matrix_part(&mut self, idx: usize, values: &[f64], order: usize, real: bool) {
        use prop::*;
        let target = match idx {
            RMATRIX | XMATRIX => &mut self.z,
            CMATRIX => &mut self.yc,
            _ => unreachable!("LineCode has no matrix property {idx}"),
        };
        let m = match target {
            Some(m) if m.order() == order => m,
            _ => {
                *target = Some(CMatrix::new(order));
                target.as_mut().unwrap()
            }
        };
        for j in 0..order {
            for i in 0..order {
                let mut v = m.get(i, j);
                if real {
                    v.re = values[j * order + i];
                } else {
                    v.im = values[j * order + i];
                }
                m.set(i, j, v);
            }
        }
    }
    fn get_matrix_part(&self, idx: usize, real: bool) -> Option<(Vec<f64>, usize)> {
        use prop::*;
        let m = match idx {
            RMATRIX | XMATRIX => self.z.as_ref()?,
            CMATRIX => self.yc.as_ref()?,
            _ => return None,
        };
        let order = m.order();
        let mut out = Vec::with_capacity(order * order);
        for j in 0..order {
            for i in 0..order {
                out.push(if real { m.get(i, j).re } else { m.get(i, j).im });
            }
        }
        Some((out, order))
    }

    /// Pascal `GetYCScale` (cmatrix) and `GetC1C0Scale` (B1/B0). Both ignore
    /// the getter/setter direction.
    fn prop_scale(&self, idx: usize, _getter: bool) -> f64 {
        match idx {
            prop::CMATRIX => TWO_PI * self.base_frequency * 1.0e-9,
            prop::B1 | prop::B0 => 1.0 / (TWO_PI * self.base_frequency) * 1.0e-6,
            _ => 1.0,
        }
    }

    fn prop_conditional(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            R1 | X1 | R0 | X0 | C1 | C0 | B1 | B0 => self.sym_components_model,
            _ => true,
        }
    }

    /// Pascal `TLineCodeObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, prev_int: i32) {
        use prop::*;

        if idx == NPHASES && self.fnphases != prev_int {
            self.fneutral_conductor = self.fnphases; // init to last conductor
            self.calc_matrices_from_z1z0(); // reallocs matrices
        }

        match idx {
            // Stage F `LINECODE_SYM_CLEAR_OMITS_C0`: Pascal omits `C0` from
            // this list — the source itself flags the slip with a "-- Missing?"
            // comment — so upstream `c0=` alone neither forces the sym model
            // nor clears the matrix property tracking. The parity lane
            // reproduces that; the default lane treats `C0` like its seven
            // siblings (the guarded arm below).
            C0 if !crate::compat::LINECODE_SYM_CLEAR_OMITS_C0 => {
                self.sym_components_model = true;
                self.data.clear_seq(RMATRIX);
                self.data.clear_seq(XMATRIX);
                self.data.clear_seq(CMATRIX);
            }
            NPHASES | R1 | X1 | R0 | X0 | C1 | B1 | B0 => {
                self.sym_components_model = true;
                self.data.clear_seq(RMATRIX);
                self.data.clear_seq(XMATRIX);
                self.data.clear_seq(CMATRIX);
            }
            RMATRIX | XMATRIX | CMATRIX => {
                self.needs_recalc = true;
                self.sym_components_model = false;
                for p in [R1, X1, R0, X0, C1, C0, B1, B0] {
                    self.data.clear_seq(p);
                }
            }
            SEASONS => {
                self.amp_ratings
                    .resize(self.num_amp_ratings.max(0) as usize, 0.0);
            }
            KRON if self.kron_pending => {
                self.kron_pending = false;
                self.do_kron_reduction();
            }
            _ => {}
        }
    }

    /// Pascal `TLineCode.EndEdit`.
    fn end_edit(&mut self, _sys: &crate::elements::traits::SysCtx) {
        if self.sym_components_model {
            self.calc_matrices_from_z1z0();
        }
        if self.needs_recalc {
            self.needs_recalc = false;
            if let Some(z) = self.z.as_ref() {
                let mut zinv = z.clone();
                let _ = zinv.invert();
                self.zinv = Some(zinv);
            }
        }
    }
}
