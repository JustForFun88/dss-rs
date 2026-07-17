//! Nominal-power machinery: the load-shape multipliers + state machine
//! (`CalcDailyMult`/`CheckStateTriggerLevel`), `ComputePresentkW` (state →
//! terminal kW), `ComputeInverterPower` / `kWOut_Calc`, `SetNominalDEROutput`
//! (Pascal `SetNominalStorage`) and `RecalcElementData` — which derive the
//! per-phase P/Q and `YEQ` from the present state/dispatch settings.

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::solution::{SolveMode, USEDAILY, USEDUTY, USEYEARLY};
use crate::util::{CDOUBLEONE, inv_sqrt3_x1000};

use super::{
    STORE_CHARGING, STORE_DISCHARGING, STORE_IDLING, Storage, StorageDispatchMode, VARMODE_PF,
};

/// Pascal `Math.Sign(Double): Integer` — returns 0 at exactly zero (unlike
/// `f64::signum`, which returns ±1). The inverter clamp depends on this.
fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

const EPSILON: f64 = 0.001; // Pascal `EPSILON` (used by the FOLLOW dispatch).

impl Storage {
    /// Pascal `CalcDailyMult` (also runs the state trigger as a last recourse).
    fn calc_daily_mult(&mut self, hr: f64, sys: &SysCtx) {
        self.base.shape_factor = match self.base.daily_shape_obj.as_mut() {
            Some(s) => s.get_mult_at_hour(hr),
            None => CDOUBLEONE, // no variation
        };
        self.check_state_trigger_level(self.base.shape_factor.re, sys);
    }

    /// Pascal `CalcDutyMult` (falls back to daily).
    fn calc_duty_mult(&mut self, hr: f64, sys: &SysCtx) {
        if let Some(s) = self.base.duty_shape_obj.as_mut() {
            self.base.shape_factor = s.get_mult_at_hour(hr);
            self.check_state_trigger_level(self.base.shape_factor.re, sys);
        } else {
            self.calc_daily_mult(hr, sys);
        }
    }

    /// Pascal `CalcYearlyMult` (falls back to daily).
    fn calc_yearly_mult(&mut self, hr: f64, sys: &SysCtx) {
        if let Some(s) = self.base.yearly_shape_obj.as_mut() {
            self.base.shape_factor = s.get_mult_at_hour(hr);
            self.check_state_trigger_level(self.base.shape_factor.re, sys);
        } else {
            self.calc_daily_mult(hr, sys);
        }
    }

    /// Pascal `CheckStateTriggerLevel`: set the storage state from the dispatch
    /// trigger levels (`DispMode=Follow` follows the sign of the load-shape;
    /// every other mode compares to `ChargeTrigger`/`DischargeTrigger` and the
    /// time-of-day `ChargeTime`).
    pub(super) fn check_state_trigger_level(&mut self, level: f64, sys: &SysCtx) {
        self.state_changed = false;
        let old_state = self.f_state;

        if self.dispatch_mode == StorageDispatchMode::Follow {
            // Charge/discharge by the sign of the load-shape.
            if level > 0.0 && (self.kwh_stored - self.kwh_reserve) > EPSILON {
                self.set_storage_state(STORE_DISCHARGING);
            } else if level < 0.0 && (self.kwh_stored - self.kwh_rating) < -EPSILON {
                self.set_storage_state(STORE_CHARGING);
            } else {
                self.set_storage_state(STORE_IDLING);
            }
        } else {
            // All other dispatch modes: compare to the trigger value.
            if self.charge_trigger == 0.0 && self.discharge_trigger == 0.0 {
                return;
            }
            // First, see whether to turn off charging/discharging.
            match self.f_state {
                STORE_CHARGING => {
                    if self.charge_trigger != 0.0
                        && (self.charge_trigger < level || self.kwh_stored >= self.kwh_rating)
                    {
                        self.f_state = STORE_IDLING;
                    }
                }
                STORE_DISCHARGING => {
                    if self.discharge_trigger != 0.0
                        && (self.discharge_trigger > level || self.kwh_stored <= self.kwh_reserve)
                    {
                        self.f_state = STORE_IDLING;
                    }
                }
                _ => {}
            }
            // Now check whether to turn on the opposite state.
            if self.f_state == STORE_IDLING {
                if self.discharge_trigger != 0.0
                    && self.discharge_trigger < level
                    && self.kwh_stored > self.kwh_reserve
                {
                    self.f_state = STORE_DISCHARGING;
                } else if self.charge_trigger != 0.0
                    && self.charge_trigger > level
                    && self.kwh_stored < self.kwh_rating
                {
                    self.f_state = STORE_CHARGING;
                }
                // Time to turn on the charge cycle if not already on.
                if self.f_state != STORE_CHARGING
                    && self.charge_time > 0.0
                    && (sys.time_of_day - self.charge_time).abs() < sys.dyna_h / 3600.0
                {
                    self.f_state = STORE_CHARGING;
                }
            }
        }

        if old_state != self.f_state {
            self.state_changed = true;
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `Set_StorageState`: decline a state change that would exceed the
    /// kWh limits (set idling instead). `pub(crate)` so the StorageController
    /// fleet dispatch can drive `obj.StorageState`.
    pub(crate) fn set_storage_state(&mut self, value: i32) {
        let saved = self.f_state;
        self.f_state = match value {
            STORE_CHARGING => {
                if self.kwh_stored < self.kwh_rating {
                    value
                } else {
                    STORE_IDLING // all charged up
                }
            }
            STORE_DISCHARGING => {
                if self.kwh_stored > self.kwh_reserve {
                    value
                } else {
                    STORE_IDLING // not enough to discharge
                }
            }
            _ => STORE_IDLING,
        };
        if saved != self.f_state {
            self.state_changed = true;
        }
    }

    /// Pascal `ComputePresentkW`: derive the present terminal kW from the state +
    /// dispatch. Idling output is only the idling losses.
    pub(super) fn compute_present_kw(&mut self) {
        let old_state = self.f_state;
        self.state_desired = old_state;
        match self.f_state {
            STORE_CHARGING => {
                if self.kwh_stored < self.kwh_rating {
                    if self.dispatch_mode == StorageDispatchMode::Follow {
                        self.base.kw_out = self.kw_rating * self.base.shape_factor.re;
                        self.pct_kw_in = self.base.shape_factor.re.abs() * 100.0;
                    } else {
                        self.base.kw_out = -self.kw_rating * self.pct_kw_in / 100.0;
                    }
                } else {
                    self.f_state = STORE_IDLING; // all charged up
                }
            }
            STORE_DISCHARGING => {
                if self.kwh_stored > self.kwh_reserve {
                    if self.dispatch_mode == StorageDispatchMode::Follow {
                        self.base.kw_out = self.kw_rating * self.base.shape_factor.re;
                        self.pct_kw_out = self.base.shape_factor.re.abs() * 100.0;
                    } else {
                        self.base.kw_out = self.kw_rating * self.pct_kw_out / 100.0;
                    }
                } else {
                    self.f_state = STORE_IDLING; // not enough to discharge
                }
            }
            _ => {}
        }
        if self.f_state == STORE_IDLING {
            self.base.kw_out = -self.kw_out_idling;
        }
        if old_state != self.f_state {
            self.state_changed = true;
        }
    }

    /// Pascal `kWOut_Calc`: clamp the AC kW to the rated-output cap (with the
    /// VW-mode requesting/limiting regions).
    fn kw_out_calc(&mut self) {
        self.fvw_state_requested = false;

        let mut limit_kw_pct = if self.f_state == STORE_DISCHARGING {
            self.kw_rating * self.pct_kw_rated
        } else {
            -(self.kw_rating * self.pct_kw_rated)
        };

        // VW works only if the element is not idling.
        if self.base.vw_mode && self.f_state != STORE_IDLING {
            if self.kw_requested >= 0.0 && self.kw_requested.abs() < limit_kw_pct.abs() {
                // Apply the VW limit.
                limit_kw_pct = if self.f_state == STORE_DISCHARGING {
                    self.kw_requested
                } else {
                    -self.kw_requested
                };
            } else if self.kw_requested < 0.0 {
                // IEEE 1547 requesting region (not fully implemented).
                if self.f_state == STORE_DISCHARGING {
                    if self.kwh_stored < self.kwh_rating {
                        self.f_state = STORE_CHARGING;
                        self.base.kw_out = self.kw_requested;
                    } else {
                        self.f_state = STORE_IDLING;
                        self.base.kw_out = -self.kw_out_idling;
                    }
                } else if self.kwh_stored > self.kwh_reserve {
                    self.f_state = STORE_DISCHARGING;
                    self.base.kw_out = -self.kw_requested;
                } else {
                    self.f_state = STORE_IDLING;
                    self.base.kw_out = -self.kw_out_idling;
                }
                self.state_changed = true;
                self.fvw_state_requested = true;
                // The state may have changed; recompute the limit.
                limit_kw_pct = if self.f_state == STORE_DISCHARGING {
                    self.kw_rating * self.pct_kw_rated
                } else {
                    -(self.kw_rating * self.pct_kw_rated)
                };
            }
        }

        // Pascal's two one-armed `if`s clamp `kW_out` to the signed limit.
        if (limit_kw_pct > 0.0 && self.base.kw_out > limit_kw_pct)
            || (limit_kw_pct < 0.0 && self.base.kw_out < limit_kw_pct)
        {
            self.base.kw_out = limit_kw_pct;
        }
    }

    /// Pascal `ComputeInverterPower`: apply cut-in/cut-out (reflected to the AC
    /// side), the watt/var priority and the kvar / `kVA` limits. Ported
    /// loop-for-loop — the discrete clamp state is gated exactly.
    fn compute_inverter_power(&mut self) {
        // Reset CurrentkvarLimit to kvarLimit.
        self.base.current_kvar_limit = self.f_kvar_limit;
        self.base.current_kvar_limit_neg = self.f_kvar_limit_neg;

        // CutIn/CutOut reflected to the AC side of the inverter.
        if let Some(c) = self.base.inverter_curve_obj.as_mut() {
            if self.f_state == STORE_DISCHARGING {
                self.cut_out_kw_ac = self.base.cut_out_kw
                    * c.get_y_value(self.base.cut_out_kw.abs() / self.f_kva_rating);
                self.cut_in_kw_ac = self.base.cut_in_kw
                    * c.get_y_value(self.base.cut_in_kw.abs() / self.f_kva_rating);
            } else {
                self.cut_out_kw_ac = self.base.cut_out_kw
                    / c.get_y_value(self.base.cut_out_kw.abs() / self.f_kva_rating);
                self.cut_in_kw_ac = self.base.cut_in_kw
                    / c.get_y_value(self.base.cut_in_kw.abs() / self.f_kva_rating);
            }
        } else {
            // Ideal inverter.
            self.cut_out_kw_ac = self.base.cut_out_kw;
            self.cut_in_kw_ac = self.base.cut_in_kw;
        }

        let old_state = self.f_state;
        // CutIn/CutOut checking on the AC side.
        if self.base.inverter_on {
            if self.base.kw_out.abs() < self.cut_out_kw_ac {
                self.base.inverter_on = false;
                self.f_state = STORE_IDLING;
            }
        } else if self.base.kw_out.abs() >= self.cut_in_kw_ac {
            self.base.inverter_on = true;
        } else {
            self.f_state = STORE_IDLING;
        }
        if old_state != self.f_state {
            self.state_changed = true;
        }

        // Set inverter output.
        if self.base.inverter_on {
            self.kw_out_calc();
        } else {
            // Idling — keep SOC constant (higher priority than %CutIn/%CutOut).
            self.base.kw_out = -self.kw_out_idling;
        }

        let pf_nominal = self.base.pf_nominal;
        let f_kvarlimit = self.f_kvar_limit;
        let f_kvarlimitneg = self.f_kvar_limit_neg;
        let pmin_no_vars = self.base.pmin_no_vars;
        let pmin_kvar_limit = self.base.pmin_kvar_limit;
        let mut temp_pf = 0.0;

        if self.f_state == STORE_IDLING {
            // In the idling state, check for the kvar limit only.
            if self.base.var_mode == VARMODE_PF {
                self.base.kvar_out =
                    self.base.kw_out * (1.0 / pf_nominal.powi(2) - 1.0).sqrt() * sign(pf_nominal);
                if self.base.kvar_out > 0.0 && self.base.kvar_out.abs() > f_kvarlimit {
                    self.base.kvar_out = f_kvarlimit;
                } else if self.base.kvar_out < 0.0 && self.base.kvar_out.abs() > f_kvarlimitneg {
                    self.base.kvar_out = f_kvarlimitneg * sign(self.kvar_requested);
                }
            } else {
                // kvarRequested set internally or by an InvControl.
                self.base.kvar_out = if self.kvar_requested > 0.0
                    && self.kvar_requested.abs() > f_kvarlimit
                {
                    f_kvarlimit
                } else if self.kvar_requested < 0.0 && self.kvar_requested.abs() > f_kvarlimitneg {
                    f_kvarlimitneg * sign(self.kvar_requested)
                } else {
                    self.kvar_requested
                };
            }
        } else if self.base.kw_out.abs() < pmin_no_vars {
            // Below minimum P for Q gen/absorption (disabled when PminNoVars = -1).
            self.base.kvar_out = 0.0;
            self.base.current_kvar_limit = 0.0;
            self.base.current_kvar_limit_neg = 0.0;
        } else if self.base.var_mode == VARMODE_PF {
            if pf_nominal == 1.0 {
                self.base.kvar_out = 0.0;
            } else {
                self.base.kvar_out =
                    self.base.kw_out * (1.0 / pf_nominal.powi(2) - 1.0).sqrt() * sign(pf_nominal);

                if self.base.kw_out.abs() < pmin_kvar_limit {
                    // Straight-line limit (disabled when PminkvarLimit = -1).
                    if self.base.kw_out.abs() >= pmin_no_vars.max(self.cut_out_kw_ac) {
                        let mut qramp_limit = 0.0;
                        if self.base.kvar_out > 0.0 {
                            qramp_limit = f_kvarlimit / pmin_kvar_limit * self.base.kw_out.abs();
                        } else if self.base.kvar_out < 0.0 {
                            qramp_limit = f_kvarlimitneg / pmin_kvar_limit * self.base.kw_out.abs();
                        }
                        if self.base.kvar_out.abs() > qramp_limit {
                            self.base.kvar_out =
                                qramp_limit * sign(self.base.kw_out) * sign(pf_nominal);
                            if self.base.kvar_out > 0.0 {
                                self.base.current_kvar_limit = qramp_limit;
                            }
                            if self.base.kvar_out < 0.0 {
                                self.base.current_kvar_limit_neg = qramp_limit;
                            }
                        }
                    }
                } else if self.base.kvar_out.abs() > f_kvarlimit
                    || self.base.kvar_out.abs() > f_kvarlimitneg
                {
                    self.base.kvar_out = if self.base.kvar_out > 0.0 {
                        f_kvarlimit * sign(self.base.kw_out) * sign(pf_nominal)
                    } else {
                        f_kvarlimitneg * sign(self.base.kw_out) * sign(pf_nominal)
                    };
                    if self.pf_priority {
                        self.base.kw_out = self.base.kvar_out
                            * (1.0 / (1.0 - pf_nominal.powi(2)) - 1.0).sqrt()
                            * sign(pf_nominal);
                    }
                }
            }
        } else {
            // kvar is specified.
            if self.base.kw_out.abs() < pmin_kvar_limit {
                if self.base.kw_out.abs() >= pmin_no_vars.max(self.cut_out_kw_ac) {
                    let mut qramp_limit = 0.0;
                    if self.kvar_requested > 0.0 {
                        qramp_limit = f_kvarlimit / pmin_kvar_limit * self.base.kw_out.abs();
                        self.base.current_kvar_limit = qramp_limit;
                    } else if self.kvar_requested < 0.0 {
                        qramp_limit = f_kvarlimitneg / pmin_kvar_limit * self.base.kw_out.abs();
                        self.base.current_kvar_limit_neg = qramp_limit;
                    }
                    self.base.kvar_out = if self.kvar_requested.abs() > qramp_limit {
                        qramp_limit * sign(self.kvar_requested)
                    } else {
                        self.kvar_requested
                    };
                }
            } else if (self.kvar_requested > 0.0 && self.kvar_requested.abs() > f_kvarlimit)
                || (self.kvar_requested < 0.0 && self.kvar_requested.abs() > f_kvarlimitneg)
            {
                self.base.kvar_out = if self.kvar_requested > 0.0 {
                    f_kvarlimit * sign(self.kvar_requested)
                } else {
                    f_kvarlimitneg * sign(self.kvar_requested)
                };

                if self.base.var_mode == super::VARMODE_KVAR
                    && self.pf_priority
                    && self.base.wp_mode
                {
                    self.base.kw_out = self.base.kvar_out.abs()
                        * (1.0 / (1.0 - self.base.pf_wp_nominal.powi(2)) - 1.0).sqrt()
                        * sign(self.base.kw_out);
                } else if self.pf_priority
                    && (!self.base.vv_mode || !self.base.drc_mode || !self.base.wv_mode)
                {
                    // Force constant PF; PF implied by kvarRequested.
                    if self.kvar_requested.abs() > 0.0 {
                        temp_pf = (self.kvar_requested / self.base.kw_out).abs().atan().cos();
                        self.base.kw_out = self.base.kvar_out.abs()
                            * (1.0 / (1.0 - temp_pf.powi(2)) - 1.0).sqrt()
                            * sign(self.base.kw_out);
                    }
                }
            } else {
                self.base.kvar_out = self.kvar_requested;
            }
        }

        if !self.base.inverter_on && self.base.var_follow_inverter {
            self.base.kvar_out = 0.0;
        }

        // Limit kvar and kW so the inverter kVA rating is not exceeded.
        let kva_gen = (self.base.kw_out.powi(2) + self.base.kvar_out.powi(2)).sqrt();
        if kva_gen > self.f_kva_rating {
            self.kva_exceeded = true;
            if self.f_state == STORE_IDLING {
                // Exceptional case: idling forces P priority always.
                self.base.kvar_out = (self.f_kva_rating.powi(2) - self.base.kw_out.powi(2)).sqrt()
                    * sign(self.base.kvar_out);
            } else if self.base.var_mode == VARMODE_PF && self.pf_priority {
                self.base.kw_out = self.f_kva_rating * pf_nominal.abs() * sign(self.base.kw_out);
                self.base.kvar_out = self.f_kva_rating
                    * (1.0 - pf_nominal.powi(2)).sqrt()
                    * sign(self.base.kw_out)
                    * sign(pf_nominal);
            } else if self.base.var_mode == super::VARMODE_KVAR
                && self.pf_priority
                && self.base.wp_mode
            {
                self.base.kw_out =
                    self.f_kva_rating * self.base.pf_wp_nominal.abs() * sign(self.base.kw_out);
                self.base.kvar_out = self.f_kva_rating
                    * self.base.pf_wp_nominal.acos().sin().abs()
                    * sign(self.kvar_requested);
            } else if self.base.var_mode == super::VARMODE_KVAR
                && self.pf_priority
                && (!self.base.vv_mode || !self.base.drc_mode || !self.base.wv_mode)
            {
                if self.base.kvar_out.abs() == f_kvarlimit {
                    self.base.kw_out = self.f_kva_rating * temp_pf.abs() * sign(self.base.kw_out);
                } else {
                    self.base.kw_out = self.f_kva_rating
                        * (self.kvar_requested / self.base.kw_out).atan().cos().abs()
                        * sign(self.base.kw_out);
                }
                self.base.kvar_out = self.f_kva_rating
                    * (self.base.kw_out / self.f_kva_rating).acos().sin().abs()
                    * sign(self.kvar_requested);
            } else if self.p_priority {
                // Back off the kvar.
                if self.base.kw_out > self.f_kva_rating {
                    self.base.kw_out = self.f_kva_rating;
                    self.base.kvar_out = 0.0;
                } else {
                    self.base.kvar_out = (self.f_kva_rating.powi(2) - self.base.kw_out.powi(2))
                        .sqrt()
                        * sign(self.base.kvar_out);
                }
            } else {
                // Q priority (default): back off the kW.
                self.base.kw_out = (self.f_kva_rating.powi(2) - self.base.kvar_out.powi(2)).sqrt()
                    * sign(self.base.kw_out);
            }
        } else {
            // Pascal: TRUE within 0.05% of the rating, else FALSE.
            self.kva_exceeded = (kva_gen - self.f_kva_rating).abs() / self.f_kva_rating < 0.0005;
        }
    }

    /// Pascal `ComputekWkvar`: present kW then inverter power.
    fn compute_kw_kvar(&mut self) {
        self.compute_present_kw();
        self.compute_inverter_power(); // apply inverter eff after cut-in/cut-out
    }

    /// Pascal `SetNominalDEROutput` (`SetNominalStorage`): pick the dispatch /
    /// shape for the solve mode, then derive the per-phase P/Q and `YEQ`.
    /// Dynamics/harmonics modes leave the element in its prior state (WP7.6/7.7).
    pub fn set_nominal_der_output(&mut self, sys: &SysCtx) {
        self.base.shape_factor = CDOUBLEONE; // changed by the curve routine below

        if !(sys.is_dynamic_model || sys.is_harmonic_model) {
            // Dispatch decides the state.
            match self.dispatch_mode {
                StorageDispatchMode::ExternalMode => {} // do nothing
                StorageDispatchMode::LoadMode => {
                    self.check_state_trigger_level(sys.generator_dispatch_reference, sys)
                }
                StorageDispatchMode::PriceMode => {
                    self.check_state_trigger_level(sys.price_signal, sys)
                }
                _ => match sys.mode {
                    SolveMode::Snapshot => {} // present kW/kvar; no state check
                    SolveMode::Daily => self.calc_daily_mult(sys.dbl_hour, sys),
                    SolveMode::Yearly => self.calc_yearly_mult(sys.dbl_hour, sys),
                    // GENERALTIME: the one class `ActiveLoadShapeClass` selects
                    // (`Set LoadShapeClass=`); `USENONE` leaves shape at 1+j1.
                    // (Pascal Storage's DYNAMICMODE arm is a documented no-op —
                    // and dynamics is guarded out above anyway — so it is NOT
                    // folded in here, unlike Load/Generator.)
                    SolveMode::Time => match sys.active_load_shape_class {
                        USEDAILY => self.calc_daily_mult(sys.dbl_hour, sys),
                        USEYEARLY => self.calc_yearly_mult(sys.dbl_hour, sys),
                        USEDUTY => self.calc_duty_mult(sys.dbl_hour, sys),
                        _ => {} // USENONE
                    },
                    SolveMode::Dynamic => {}
                    SolveMode::Monte2
                    | SolveMode::Monte3
                    | SolveMode::LD1
                    | SolveMode::LD2
                    | SolveMode::PeakDay => self.calc_daily_mult(sys.dbl_hour, sys),
                    SolveMode::DutyCycle => self.calc_duty_mult(sys.dbl_hour, sys),
                    _ => {}
                },
            }

            self.compute_kw_kvar();
            // Pnominalperphase is net at the terminal. When discharging the
            // storage supplies the idling losses; when charging they subtract.
            let nphases = self.cd.nphases as f64;
            self.base.p_nominal_per_phase = 1000.0 * self.base.kw_out / nphases;
            self.base.q_nominal_per_phase = 1000.0 * self.base.kvar_out / nphases;

            // VoltageModel 3 (user model) leaves YEQ as-is; all others compute it.
            if self.base.voltage_model != 3 {
                let vbase = self.base.v_base;
                self.base.yeq = Complex64::new(
                    self.base.p_nominal_per_phase,
                    -self.base.q_nominal_per_phase,
                ) / vbase.powi(2); // Vbase must be L-N for 3-phase
                self.base.yeq_min = if self.base.vminpu != 0.0 {
                    self.base.yeq / self.base.vminpu.powi(2) // at 95% voltage
                } else {
                    self.base.yeq // always a constant-Z model
                };
                self.base.yeq_max = if self.base.vmaxpu != 0.0 {
                    self.base.yeq / self.base.vmaxpu.powi(2) // at 105% voltage
                } else {
                    self.base.yeq
                };
            }
            // Like Model-7 generator: max current to deliver requested power at
            // min voltage.
            self.base.phase_current_limit =
                Complex64::new(self.base.p_nominal_per_phase, self.base.q_nominal_per_phase)
                    / self.base.v_base_min;
            self.max_dyn_phase_current = self.base.phase_current_limit.norm();
        }

        // If the state changed, force a re-calc of the Y matrix.
        if self.state_changed {
            self.cd.yprim_invalid = true;
            self.state_changed = false;
        }
    }

    /// Pascal `TStorageObj.RecalcElementData`.
    pub fn recalc(&mut self, sys: &SysCtx) {
        self.base.v_base_min = self.base.vminpu * self.base.v_base;
        self.base.v_base_max = self.base.vmaxpu * self.base.v_base;

        let nphases = self.cd.nphases as f64;
        self.yeq_discharge = Complex64::new(
            self.kw_rating * 1000.0 / self.base.v_base.powi(2) / nphases,
            0.0,
        );

        // Thevenin equivalents (ohms) — used by the harmonic model (WP7.6).
        let present_kv = self.kv_storage_base;
        self.r_thev = self.base.pct_r * 0.01 * present_kv.powi(2) / self.f_kva_rating * 1000.0;
        self.x_thev = self.base.pct_x * 0.01 * present_kv.powi(2) / self.f_kva_rating * 1000.0;

        self.base.cut_in_kw = self.base.fpct_cut_in * self.f_kva_rating / 100.0;
        self.base.cut_out_kw = self.base.fpct_cut_out * self.f_kva_rating / 100.0;

        self.base.pmin_no_vars = if self.base.fpct_pmin_no_vars <= 0.0 {
            -1.0
        } else {
            self.base.fpct_pmin_no_vars * self.kw_rating / 100.0
        };
        self.base.pmin_kvar_limit = if self.base.fpct_pmin_kvar_limit <= 0.0 {
            -1.0
        } else {
            self.base.fpct_pmin_kvar_limit * self.kw_rating / 100.0
        };

        self.charge_eff = self.pct_charge_eff * 0.01;
        self.discharge_eff = self.pct_discharge_eff * 0.01;

        self.p_idling = self.pct_idle_kw * self.kw_rating / 100.0;

        self.kw_out_idling = if let Some(c) = self.base.inverter_curve_obj.as_mut() {
            self.p_idling / c.get_y_value(self.p_idling / self.f_kva_rating)
        } else {
            self.p_idling
        };

        self.set_nominal_der_output(sys);

        // Initialise InjCurrent to zero (defaults to a PQ Storage element).
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    /// The L-N `VBase` update (Pascal `TProp.conn`/`TProp.kV` side effects):
    /// L-N for 2/3-phase, otherwise the supplied value.
    pub(super) fn update_vbase(&mut self) {
        self.base.v_base = match self.cd.nphases {
            2 | 3 => self.kv_storage_base * inv_sqrt3_x1000(),
            _ => self.kv_storage_base * 1000.0,
        };
    }
}
