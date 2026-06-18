//! Nominal-power machinery: dispatch decision, load-shape multipliers and the
//! `SetNominalGeneration` / `RecalcElementData` path that derives `Yeq` and the
//! per-phase P/Q from the generator's kW/kvar settings.

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::solution::SolveMode;
use crate::util::{CDOUBLEONE, inv_sqrt3_x1000, sqrt3};

use super::{Generator, LOADMODE, PRICEMODE, prop};

impl Generator {
    /// Pascal `CalcDailyMult`.
    fn calc_daily_mult(&mut self, hr: f64) {
        if let Some(s) = self.daily_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = CDOUBLEONE;
        }
    }

    /// Pascal `CalcDutyMult` (falls back to daily; includes `DutyStart` offset).
    fn calc_duty_mult(&mut self, hr: f64) {
        if let Some(s) = self.duty_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr + self.duty_start);
            self.shape_is_actual = s.use_actual();
        } else {
            self.calc_daily_mult(hr);
        }
    }

    /// Pascal `CalcYearlyMult`.
    fn calc_yearly_mult(&mut self, hr: f64) {
        if let Some(s) = self.yearly_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = CDOUBLEONE;
        }
    }

    /// Pascal `SyncUpPowerQuantities`: keep kvar nominal in step with kW/PF.
    pub(super) fn sync_up_power_quantities(&mut self) {
        if self.pf_nominal == 0.0 {
            return;
        }
        self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
        self.p_nominal_per_phase = 1000.0 * self.kw_base / self.cd.nphases as f64;
        self.kvar_max = 2.0 * self.kvar_base;
        self.kvar_min = -self.kvar_max;
        if self.pf_nominal < 0.0 {
            self.kvar_base = -self.kvar_base;
        }
        self.q_nominal_per_phase = 1000.0 * self.kvar_base / self.cd.nphases as f64;
        if self.kva_not_set {
            self.kva_rating = self.kw_base * 1.2;
        }
    }

    /// Pascal `SetkWkvar`: set base kW/kvar then run the `kvar` side effects.
    pub(super) fn set_kw_kvar(&mut self, p_kw: f64, q_kvar: f64) {
        self.kw_base = p_kw;
        self.kvar_base = q_kvar;
        self.side_effect_kvar();
    }

    /// Pascal `TProp.kvar` side effect.
    pub(super) fn side_effect_kvar(&mut self) {
        let nphases = self.cd.nphases as f64;
        self.q_nominal_per_phase = 1000.0 * self.kvar_base / nphases;
        let kva_gen = (self.kw_base.powi(2) + self.kvar_base.powi(2)).sqrt();
        self.pf_nominal = if kva_gen != 0.0 {
            self.kw_base / kva_gen
        } else {
            1.0
        };
        if self.kw_base * self.kvar_base < 0.0 {
            self.pf_nominal = -self.pf_nominal;
        }
        self.kvar_max = 2.0 * self.kvar_base;
        self.kvar_min = -self.kvar_max;
        self.cd.obj.clear_seq(prop::PF);
    }

    /// The shared VBase update (Pascal `TProp.conn`/`TProp.kV` side effects):
    /// L-N for 2/3-phase, otherwise the supplied value (independent of the
    /// connection — Pascal uses the same formula for wye and delta here).
    pub(super) fn update_vbase(&mut self) {
        self.v_base = match self.cd.nphases {
            2 | 3 => self.kv_generator_base * inv_sqrt3_x1000(),
            _ => self.kv_generator_base * 1000.0,
        };
    }

    /// Pascal `SetNominalGeneration` (the power-flow path; dynamics/harmonics
    /// branches are never entered in Phase 6 and are omitted).
    pub fn set_nominal_generation(&mut self, sys: &SysCtx) {
        let gen_on_saved = self.gen_on;
        self.shape_factor = CDOUBLEONE;

        // Decide whether the generator is ON (LOADMODE compares the dispatch
        // reference, PRICEMODE the price signal, both against DispValue).
        self.gen_on = true;
        if !self.forced_on && self.dispatch_value > 0.0 {
            let off_load = self.dispatch_mode == LOADMODE
                && sys.generator_dispatch_reference < self.dispatch_value;
            let off_price =
                self.dispatch_mode == PRICEMODE && sys.price_signal < self.dispatch_value;
            if off_load || off_price {
                self.gen_on = false;
            }
        }

        let nphases = self.cd.nphases as f64;
        if !self.gen_on {
            // OFF: a tiny resistive load so the matrix doesn't go singular.
            self.p_nominal_per_phase = -0.1 * self.kw_base / nphases;
            self.q_nominal_per_phase = 0.0;
        } else {
            let factor = if self.is_fixed {
                1.0
            } else {
                match sys.mode {
                    SolveMode::Snapshot
                    | SolveMode::Monte1
                    | SolveMode::MonteFault
                    | SolveMode::FaultStudy => sys.gen_multiplier,
                    SolveMode::Daily
                    | SolveMode::Monte2
                    | SolveMode::Monte3
                    | SolveMode::LD1
                    | SolveMode::LD2
                    | SolveMode::PeakDay => {
                        let f = sys.gen_multiplier;
                        self.calc_daily_mult(sys.dbl_hour);
                        f
                    }
                    SolveMode::Yearly => {
                        let f = sys.gen_multiplier;
                        self.calc_yearly_mult(sys.dbl_hour);
                        f
                    }
                    SolveMode::DutyCycle => {
                        let f = sys.gen_multiplier;
                        self.calc_duty_mult(sys.dbl_hour);
                        f
                    }
                    SolveMode::Time | SolveMode::Dynamic => {
                        // GENERALTIME / DYNAMICMODE: one load-shape class.
                        // ActiveLoadShapeClass is `USENONE` by default → 1+j1.
                        sys.gen_multiplier
                    }
                    SolveMode::AutoAdd => 1.0,
                    _ => 1.0,
                }
            };

            if self.shape_is_actual {
                self.p_nominal_per_phase = 1000.0 * self.shape_factor.re / nphases;
            } else {
                self.p_nominal_per_phase =
                    1000.0 * self.kw_base * factor * self.shape_factor.re / nphases;
            }

            if self.gen_model == 3 {
                // Just make sure the present value is reasonable.
                if self.q_nominal_per_phase > self.var_max {
                    self.q_nominal_per_phase = self.var_max;
                } else if self.q_nominal_per_phase < self.var_min {
                    self.q_nominal_per_phase = self.var_min;
                }
            } else if self.shape_is_actual {
                self.q_nominal_per_phase = 1000.0 * self.shape_factor.im / nphases;
            } else {
                self.q_nominal_per_phase =
                    1000.0 * self.kvar_base * factor * self.shape_factor.im / nphases;
            }
        }

        if self.gen_model == 6 {
            self.yeq = Complex64::new(0.0, -self.xd).inv(); // gets negated in CalcYPrim
        } else {
            self.yeq = Complex64::new(self.p_nominal_per_phase, -self.q_nominal_per_phase)
                / self.v_base.powi(2); // Vbase L-N for 3-phase
            self.yeq95 = if self.vminpu != 0.0 {
                self.yeq / self.vminpu.powi(2)
            } else {
                self.yeq // always a constant-Z model
            };
            self.yeq105 = if self.vmaxpu != 0.0 {
                self.yeq / self.vmaxpu.powi(2)
            } else {
                self.yeq
            };
        }

        if self.gen_model == 7 {
            self.phase_current_limit =
                Complex64::new(self.p_nominal_per_phase, -self.q_nominal_per_phase) / self.v_base95;
            self.model7_max_phase_curr = self.phase_current_limit.norm();
        }

        // If the generator state changes, force a Y rebuild.
        if self.gen_on != gen_on_saved {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TGeneratorObj.RecalcElementData`.
    pub fn recalc(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases as f64;
        self.v_base95 = self.vminpu * self.v_base;
        self.v_base105 = self.vmaxpu * self.v_base;

        self.var_base = 1000.0 * self.kvar_base / nphases;
        self.var_min = 1000.0 * self.kvar_min / nphases;
        self.var_max = 1000.0 * self.kvar_max / nphases;

        self.xd = self.pu_xd * 1000.0 * self.kv_generator_base.powi(2) / self.kva_rating;
        self.xdp = self.pu_xdp * 1000.0 * self.kv_generator_base.powi(2) / self.kva_rating;
        self.xdpp = self.pu_xdpp * 1000.0 * self.kv_generator_base.powi(2) / self.kva_rating;

        self.set_nominal_generation(sys);

        self.yq_fixed = -self.var_base / self.v_base.powi(2);
        self.v_target = self.vpu * 1000.0 * self.kv_generator_base;
        if self.cd.nphases > 1 {
            self.v_target /= sqrt3();
        }

        self.dqdv = self.dqdv_saved; // for Model 3
        self.delta_q_max = (self.var_max - self.var_min) * 0.10; // limit to 10% of range
    }
}
