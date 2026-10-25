//! Nominal-power machinery: the load-shape / temperature-shape multipliers,
//! `ComputePanelPower` / `ComputeInverterPower` / `kWOut_Calc` (the PV-panel +
//! inverter model), `SetNominalDEROutput` (Pascal `SetNominalPVSystem`) and
//! `RecalcElementData` — which derive the per-phase P/Q and `YEQ` from the
//! present irradiance/temperature/kW settings.

use num_complex::Complex64;

use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::SysCtx;
use crate::solution::{SolveMode, USEDAILY, USEDUTY, USEYEARLY};
use crate::util::{CDOUBLEONE, inv_sqrt3_x1000};

use super::PVSystem;
use crate::elements::pc::inv_based_pce::VarMode;

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

impl PVSystem {
    /// Pascal `CalcDailyMult` (irradiance shape).
    pub(super) fn calc_daily_mult(&mut self, hr: f64) {
        if let Some(s) = self.base.daily_shape_obj.as_mut() {
            self.base.shape_factor = s.get_mult_at_hour(hr);
        } else {
            self.base.shape_factor = CDOUBLEONE; // no variation
        }
    }

    /// Pascal `CalcDutyMult` (includes the `DutyStart` offset; falls back to
    /// daily).
    pub(super) fn calc_duty_mult(&mut self, hr: f64) {
        if let Some(s) = self.base.duty_shape_obj.as_mut() {
            self.base.shape_factor = s.get_mult_at_hour(hr + self.duty_start);
        } else {
            self.calc_daily_mult(hr);
        }
    }

    /// Pascal `CalcYearlyMult` (includes the `DutyStart` offset; falls back to
    /// daily).
    pub(super) fn calc_yearly_mult(&mut self, hr: f64) {
        if let Some(s) = self.base.yearly_shape_obj.as_mut() {
            self.base.shape_factor = s.get_mult_at_hour(hr + self.duty_start);
        } else {
            self.calc_daily_mult(hr);
        }
    }

    /// Pascal `CalcDailyTemperature`.
    pub(super) fn calc_daily_temperature(&mut self, hr: f64) {
        if let Some(s) = self.daily_t_shape_obj.as_mut() {
            self.t_shape_value = s.get_temperature(hr);
        } else {
            self.t_shape_value = self.f_temperature; // no variation
        }
    }

    /// Pascal `CalcDutyTemperature` (falls back to daily).
    pub(super) fn calc_duty_temperature(&mut self, hr: f64) {
        if let Some(s) = self.duty_t_shape_obj.as_mut() {
            self.t_shape_value = s.get_temperature(hr);
        } else {
            self.calc_daily_temperature(hr);
        }
    }

    /// Pascal `CalcYearlyTemperature` (falls back to daily).
    pub(super) fn calc_yearly_temperature(&mut self, hr: f64) {
        if let Some(s) = self.yearly_t_shape_obj.as_mut() {
            self.t_shape_value = s.get_temperature(hr);
        } else {
            self.calc_daily_temperature(hr);
        }
    }

    /// Pascal `ComputePanelPower`: DC panel kW = irradiance · shape · `Pmpp` ·
    /// temperature-derate.
    pub(super) fn compute_panel_power(&mut self) {
        self.temp_factor = 1.0;
        if let Some(c) = self.power_temp_curve_obj.as_mut() {
            // pu Pmpp vs T (actual).
            self.temp_factor = c.get_y_value(self.t_shape_value);
        }
        self.panel_kw =
            self.f_irradiance * self.base.shape_factor.re * self.f_pmpp * self.temp_factor;
    }

    /// The inverter efficiency at the present panel power: the efficiency
    /// curve at `PanelkW / kVA`, 1 without a curve.
    pub(super) fn inverter_curve_efficiency(&mut self) -> f64 {
        match self.base.inverter_curve_obj.as_mut() {
            Some(c) => c.get_y_value(self.panel_kw / self.f_kva_rating),
            None => 1.0,
        }
    }

    /// Pascal `kWOut_Calc`: clamp the AC kW to the watt-watt request (VW/WV
    /// modes) or to `Pmpp·%Pmpp` otherwise.
    fn kw_out_calc(&mut self) {
        let pac = self.panel_kw * self.eff_factor;
        if self.base.vw_mode || self.base.wv_mode {
            self.base.kw_out = if pac > self.kw_requested {
                self.kw_requested
            } else {
                pac
            };
        } else {
            let ppct_limit = self.f_pmpp * self.f_pu_pmpp;
            self.base.kw_out = if pac > ppct_limit { ppct_limit } else { pac };
        }
    }

    /// Pascal `ComputeInverterPower`: apply cut-in/cut-out, the efficiency
    /// curve, the watt/var priority and the kvar / `kVA` limits. Ported
    /// loop-for-loop — the discrete clamp state is gated exactly (PHASE7_PLAN
    /// §1 DER gate: "inverter control discrete state ... exact").
    fn compute_inverter_power(&mut self) {
        // Reset CurrentkvarLimit to kvarLimit.
        self.base.current_kvar_limit = self.f_kvar_limit;
        self.base.current_kvar_limit_neg = self.f_kvar_limit_neg;

        self.eff_factor = 1.0;
        self.base.kw_out = 0.0;

        let cut_out_kw_ac = if let Some(c) = self.base.inverter_curve_obj.as_mut() {
            self.base.cut_out_kw * c.get_y_value(self.base.cut_out_kw.abs() / self.f_kva_rating)
        } else {
            self.base.cut_out_kw // assume ideal inverter
        };

        // Determine the state of the inverter.
        if self.base.inverter_on {
            if self.panel_kw < self.base.cut_out_kw {
                self.base.inverter_on = false;
            }
        } else if self.panel_kw >= self.base.cut_in_kw {
            self.base.inverter_on = true;
        }

        // Set inverter output (100% of PanelkW if no efficiency curve).
        if self.base.inverter_on {
            self.eff_factor = self.inverter_curve_efficiency();
            self.kw_out_calc();
        } else {
            self.base.kw_out = 0.0;
        }

        let pf_nominal = self.base.pf_nominal;
        let f_kvarlimit = self.f_kvar_limit;
        let f_kvarlimitneg = self.f_kvar_limit_neg;
        let pmin_no_vars = self.base.pmin_no_vars;
        let pmin_kvar_limit = self.base.pmin_kvar_limit;
        let mut temp_pf = 0.0;

        if self.base.kw_out.abs() < pmin_no_vars {
            // Below minimum P for Q gen/absorption (disabled when PminNoVars = -1).
            self.base.kvar_out = 0.0;
            self.base.current_kvar_limit = 0.0;
            self.base.current_kvar_limit_neg = 0.0;
        } else if self.base.var_mode == VarMode::Pf {
            if pf_nominal == 1.0 {
                self.base.kvar_out = 0.0;
            } else {
                self.base.kvar_out =
                    self.base.kw_out * (1.0 / pf_nominal.powi(2) - 1.0).sqrt() * sign(pf_nominal);

                if self.base.kw_out.abs() < pmin_kvar_limit {
                    // Straight-line limit (disabled when PminkvarLimit = -1).
                    if self.base.kw_out.abs() >= pmin_no_vars.max(cut_out_kw_ac) {
                        let mut qramp_limit = 0.0;
                        if self.base.kvar_out > 0.0 {
                            qramp_limit = f_kvarlimit / pmin_kvar_limit * self.base.kw_out.abs();
                            self.base.current_kvar_limit = qramp_limit; // generation limit
                        } else if self.base.kvar_out < 0.0 {
                            qramp_limit = f_kvarlimitneg / pmin_kvar_limit * self.base.kw_out.abs();
                            self.base.current_kvar_limit_neg = qramp_limit; // absorption limit
                        }
                        if self.base.kvar_out.abs() > qramp_limit {
                            self.base.kvar_out =
                                qramp_limit * sign(self.base.kw_out) * sign(pf_nominal);
                        }
                    }
                } else if self.base.kvar_out.abs() > f_kvarlimit
                    || self.base.kvar_out.abs() > f_kvarlimitneg
                {
                    // Normal kvarLimit / kvarLimitNeg.
                    self.base.kvar_out = if self.base.kvar_out > 0.0 {
                        f_kvarlimit * sign(self.base.kw_out) * sign(pf_nominal)
                    } else {
                        f_kvarlimitneg * sign(self.base.kw_out) * sign(pf_nominal)
                    };
                    if self.pf_priority {
                        // Force constant PF when the kvar limit is exceeded.
                        self.base.kw_out = self.base.kvar_out
                            * (1.0 / (1.0 - pf_nominal.powi(2)) - 1.0).sqrt()
                            * sign(pf_nominal);
                    }
                }
            }
        } else {
            // kvar is specified.
            if self.base.kw_out.abs() < pmin_kvar_limit {
                if self.base.kw_out.abs() >= pmin_no_vars.max(cut_out_kw_ac) {
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
            } else if (self.kvar_requested > 0.0 && self.kvar_requested.abs() >= f_kvarlimit)
                || (self.kvar_requested < 0.0 && self.kvar_requested.abs() >= f_kvarlimitneg)
            {
                self.base.kvar_out = if self.kvar_requested > 0.0 {
                    f_kvarlimit * sign(self.kvar_requested)
                } else {
                    f_kvarlimitneg * sign(self.kvar_requested)
                };

                if self.base.var_mode == VarMode::Kvar && self.pf_priority && self.base.wp_mode {
                    self.base.kw_out = self.base.kvar_out.abs()
                        * (1.0 / (1.0 - self.base.pf_wp_nominal.powi(2)) - 1.0).sqrt()
                        * sign(self.base.kw_out);
                } else if self.pf_priority
                    && (!self.base.vv_mode
                        || !self.base.drc_mode
                        || !self.base.wv_mode
                        || !self.base.avr_mode)
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
            if self.base.var_mode == VarMode::Pf && self.pf_priority {
                self.base.kw_out = self.f_kva_rating * pf_nominal.abs();
                self.base.kvar_out =
                    self.f_kva_rating * (1.0 - pf_nominal.powi(2)).sqrt() * sign(pf_nominal);
            } else if self.base.var_mode == VarMode::Kvar && self.pf_priority && self.base.wp_mode {
                self.base.kw_out =
                    self.f_kva_rating * self.base.pf_wp_nominal.abs() * sign(self.base.kw_out);
                self.base.kvar_out = self.f_kva_rating
                    * self.base.pf_wp_nominal.acos().sin().abs()
                    * sign(self.kvar_requested);
            } else if self.base.var_mode == VarMode::Kvar
                && self.pf_priority
                && (!self.base.vv_mode
                    || !self.base.drc_mode
                    || !self.base.wv_mode
                    || !self.base.avr_mode)
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
                self.base.kw_out = (self.f_kva_rating.powi(2) - self.base.kvar_out.powi(2)).sqrt()
                    * sign(self.base.kw_out);
            }
        }
        if !self.base.inverter_on && self.base.var_follow_inverter {
            self.base.kvar_out = 0.0;
        }
    }

    /// Pascal `ComputekWkvar`: panel power then inverter power.
    fn compute_kw_kvar(&mut self) {
        self.compute_panel_power(); // apply irradiance
        self.compute_inverter_power(); // apply inverter eff after cut-in/cut-out
    }

    /// Pascal `SetNominalDEROutput` (`SetNominalPVSystem`): pick the shape /
    /// temperature for the solve mode, then derive the per-phase P/Q and `YEQ`.
    /// Dynamics/harmonics modes leave the element in its prior state (WP7.6/7.7).
    pub fn set_nominal_der_output(&mut self, sys: &SysCtx) {
        self.base.shape_factor = CDOUBLEONE; // changed by the curve routine below
        self.t_shape_value = self.f_temperature;

        if sys.is_dynamic_model || sys.is_harmonic_model {
            // Leave the PVSystem in whatever state it had before dynamics mode.
            return;
        }

        match sys.mode {
            SolveMode::Snapshot => {} // present kW/kvar; no state change
            SolveMode::Daily => {
                self.calc_daily_mult(sys.dbl_hour);
                self.calc_daily_temperature(sys.dbl_hour);
            }
            SolveMode::Yearly => {
                self.calc_yearly_mult(sys.dbl_hour);
                self.calc_yearly_temperature(sys.dbl_hour);
            }
            // GENERALTIME: the one class `ActiveLoadShapeClass` selects (`Set
            // LoadShapeClass=`) drives both the mult and the temperature;
            // `USENONE` leaves shape at 1+j1 and temperature nominal. (Dynamics
            // returns early above, so it is not folded into this arm.)
            SolveMode::Time => match sys.active_load_shape_class {
                USEDAILY => {
                    self.calc_daily_mult(sys.dbl_hour);
                    self.calc_daily_temperature(sys.dbl_hour);
                }
                USEYEARLY => {
                    self.calc_yearly_mult(sys.dbl_hour);
                    self.calc_yearly_temperature(sys.dbl_hour);
                }
                USEDUTY => {
                    self.calc_duty_mult(sys.dbl_hour);
                    self.calc_duty_temperature(sys.dbl_hour);
                }
                _ => {} // USENONE
            },
            SolveMode::Dynamic => {}
            SolveMode::Monte2
            | SolveMode::Monte3
            | SolveMode::LD1
            | SolveMode::LD2
            | SolveMode::PeakDay => {
                self.calc_daily_mult(sys.dbl_hour);
                self.calc_daily_temperature(sys.dbl_hour);
            }
            SolveMode::DutyCycle => {
                self.calc_duty_mult(sys.dbl_hour);
                self.calc_duty_temperature(sys.dbl_hour);
            }
            _ => {}
        }

        self.compute_kw_kvar();
        let nphases = self.cd.nphases as f64;
        self.base.p_nominal_per_phase = 1000.0 * self.base.kw_out / nphases;
        self.base.q_nominal_per_phase = 1000.0 * self.base.kvar_out / nphases;

        // VoltageModel 3 (user model) leaves YEQ as-is; all others compute it.
        if self.base.voltage_model != 3 {
            let vbase = self.base.v_base;
            self.base.yeq = Complex64::new(
                self.base.p_nominal_per_phase,
                -self.base.q_nominal_per_phase,
            ) / vbase.powi(2); // Vbase L-N for 3-phase
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
            // Like Model-7 generator: max current to deliver requested power at
            // min voltage.
            self.base.phase_current_limit =
                Complex64::new(self.base.p_nominal_per_phase, self.base.q_nominal_per_phase)
                    / self.base.v_base_min;
            self.max_dyn_phase_current = self.base.phase_current_limit.norm();
        }
    }

    /// Recompute the derived values, refused when an input makes
    /// the inverter output or the power-flow admittance not a finite number:
    /// the edit logs the refusal, every derived value keeps what it held and
    /// the stamp is refused until a recalc finds them finite (pinned by
    /// `exec::tests::rating_refusals::an_input_that_makes_a_stamp_non_finite_is_refused_at_the_recalc`).
    /// A `P-TCurve` or `EffCurve` with a point that is not a finite number
    /// refuses the output whatever operating point this recalc reads, since a
    /// later step can read any point (pinned by
    /// `exec::tests::rating_refusals::a_curve_point_that_is_not_a_number_refuses_its_element`).
    pub fn recalc(&mut self, sys: &SysCtx) {
        let mut held = self.clone();
        self.derive(sys);
        let (kw_out, kvar_out, yeq, factors) = self.power_flow_output(sys);
        let finite = |z: Complex64| z.re.is_finite() && z.im.is_finite();
        let curve_points = self.curves().iter().any(|(_, _, c)| curve_broken(*c));
        let output = kw_out.is_finite() && kvar_out.is_finite() && !curve_points;
        let quantity = if !output {
            Some("inverter output")
        } else if !finite(yeq) {
            Some("admittance")
        } else {
            None
        };
        let Some(quantity) = quantity else {
            self.cd.recalc_refusal = None;
            // The user-written models are updated once the recalc is accepted.
            self.update_user_models(sys);
            return;
        };
        let nphases = self.cd.nphases as f64;
        let per_phase = [kw_out, kvar_out]
            .iter()
            .all(|x| (1000.0 * x / nphases).is_finite());
        held.user_model = self.user_model.take();
        *self = held;
        let zero_pf = !output && !curve_points && self.zero_pf_breaks_output(sys);
        let input = self.refused_input(output, per_phase, factors, zero_pf);
        self.cd.refuse_recalc("PVSystem", &input, quantity);
    }

    /// The curves the inverter output reads, in property order: the name of
    /// the property, the curve's name and the curve held.
    fn curves(&self) -> [(&'static str, &str, Option<&XyCurveObj>); 2] {
        [
            (
                "P-TCurve",
                &self.power_temp_curve,
                self.power_temp_curve_obj.as_ref(),
            ),
            (
                "EffCurve",
                &self.base.inverter_curve,
                self.base.inverter_curve_obj.as_ref(),
            ),
        ]
    }

    /// Whether a zero `PF` that the power-factor mode divides by is alone what
    /// makes the inverter output not finite: the same inputs with a `PF` of 1,
    /// which divides by nothing, give a finite output.
    fn zero_pf_breaks_output(&self, sys: &SysCtx) -> bool {
        if self.base.var_mode != VarMode::Pf || self.base.pf_nominal != 0.0 {
            return false;
        }
        let mut trial = self.clone();
        trial.base.pf_nominal = 1.0;
        trial.derive(sys);
        let (kw, kvar, _, _) = trial.power_flow_output(sys);
        kw.is_finite() && kvar.is_finite()
    }

    /// The inverter output, the admittance and the factors read from the
    /// `P-TCurve` and the `EffCurve` that a power-flow solve computes from the
    /// present inputs, a factor read at an operating point that is not a
    /// finite number counted as `1`. Dynamics and harmonics keep the output the element
    /// held, so there it is computed on a copy and an edit in either mode is
    /// checked as in a power flow
    /// (`exec::tests::rating_refusals::a_refusal_holds_after_a_dynamics_or_harmonics_solve`).
    fn power_flow_output(&self, sys: &SysCtx) -> (f64, f64, Complex64, [f64; 2]) {
        let out = |pv: &Self| {
            let at = |point: f64, factor: f64| if point.is_finite() { factor } else { 1.0 };
            (
                pv.base.kw_out,
                pv.base.kvar_out,
                pv.base.yeq,
                [
                    at(pv.t_shape_value, pv.temp_factor),
                    at(pv.panel_kw / pv.f_kva_rating, pv.eff_factor),
                ],
            )
        };
        if !(sys.is_dynamic_model || sys.is_harmonic_model) {
            return out(self);
        }
        let mut flow = self.clone();
        flow.set_nominal_der_output(&SysCtx {
            is_dynamic_model: false,
            is_harmonic_model: false,
            ..sys.clone()
        });
        out(&flow)
    }

    /// The input a refused recalc names. An inverter output that is not finite
    /// names a zero `PF` that alone breaks it (`zero_pf`), so an input beside
    /// it that is not a finite number and leaves the output finite (an
    /// infinite cap is no cap) is not named, else the first of the inputs it
    /// computes with, in property order, that is not a finite number (`kvar` is the typed request, since its reading
    /// is the output), else the `P-TCurve`, then the `EffCurve` it feeds, that
    /// holds a point that is not a finite number or whose factor at a finite
    /// operating point (`factors`) is not one, else a zero `PF`
    /// it divides by in power-factor mode, else the kVA rating it is held to.
    /// From a finite output the admittance
    /// names a zero `kV`, else the kVA rating when the per-phase power
    /// overflows (the output never exceeds that rating), else `kV`, the
    /// voltage base it overflows on.
    fn refused_input(
        &self,
        output: bool,
        per_phase: bool,
        factors: [f64; 2],
        zero_pf: bool,
    ) -> String {
        use super::prop::*;
        use crate::elements::ckt::{input_text, refused_input_text};
        use crate::obj::base::DssObject;
        let v = |i: usize| self.get_f64(i);
        let kv = [("kV", v(KV))];
        if output {
            let fallback = if per_phase { kv[0] } else { ("kVA", v(KVA)) };
            return refused_input_text(&kv, &kv, fallback);
        }
        if zero_pf {
            return input_text("PF", v(PF));
        }
        let inputs = [
            ("Irradiance", v(IRRADIANCE)),
            ("Pmpp", v(PMPP)),
            ("%Pmpp", v(PCT_PMPP) / 0.01),
            ("Temperature", v(TEMPERATURE)),
            ("PF", v(PF)),
            ("kvar", self.kvar_requested),
            ("kVA", v(KVA)),
            ("%CutIn", v(PCT_CUTIN)),
            ("%CutOut", v(PCT_CUTOUT)),
            ("%PMinNoVars", v(PCT_PMIN_NO_VARS)),
            ("%PMinkvarMax", v(PCT_PMIN_KVAR_MAX)),
            ("kvarMax", v(KVAR_MAX)),
            ("kvarMaxAbs", v(KVAR_MAX_ABS)),
        ];
        if inputs.iter().all(|(_, x)| x.is_finite()) {
            let broken = self
                .curves()
                .into_iter()
                .zip(factors)
                .find(|((_, _, c), f)| curve_broken(*c) || !f.is_finite());
            if let Some(((name, curve, _), _)) = broken {
                return format!("{name}={curve}");
            }
        }
        let pf: &[(&str, f64)] = if self.base.var_mode == VarMode::Pf {
            &[("PF", v(PF))]
        } else {
            &[]
        };
        refused_input_text(&inputs, pf, ("kVA", v(KVA)))
    }

    /// The refusal of a harmonic admittance, the inverse of the Thevenin
    /// impedance of `%R` and `%X`, that is not a finite number.
    pub(super) fn harmonic_refusal(&self) -> String {
        use super::prop::*;
        use crate::obj::base::DssObject;
        let v = |i: usize| self.get_f64(i);
        let thev = Complex64::new(self.r_thev, self.x_thev);
        let input = crate::elements::pc::inv_based_pce::harmonic_refused_input(
            [v(KV), v(KVA), v(PCT_R), v(PCT_X)],
            thev,
        );
        self.cd
            .refusal_message("PVSystem", &input, "harmonic admittance")
    }

    /// The body of [`Self::recalc`], bar the user-model update.
    fn derive(&mut self, sys: &SysCtx) {
        self.base.v_base_min = self.base.vminpu * self.base.v_base;
        self.base.v_base_max = self.base.vmaxpu * self.base.v_base;

        self.var_base = 1000.0 * self.base.kvar_out / self.cd.nphases as f64;

        // Thevenin equivalents (ohms) — used by the harmonic model (WP7.6).
        let present_kv = self.kv_pvsystem_base;
        self.r_thev = self.base.pct_r * 0.01 * present_kv.powi(2) / self.f_kva_rating * 1000.0;
        self.x_thev = self.base.pct_x * 0.01 * present_kv.powi(2) / self.f_kva_rating * 1000.0;

        self.base.cut_in_kw = self.base.fpct_cut_in * self.f_kva_rating / 100.0;
        self.base.cut_out_kw = self.base.fpct_cut_out * self.f_kva_rating / 100.0;

        self.base.pmin_no_vars = if self.base.fpct_pmin_no_vars <= 0.0 {
            -1.0
        } else {
            self.base.fpct_pmin_no_vars * self.f_pmpp / 100.0
        };
        self.base.pmin_kvar_limit = if self.base.fpct_pmin_kvar_limit <= 0.0 {
            -1.0
        } else {
            self.base.fpct_pmin_kvar_limit * self.f_pmpp / 100.0
        };

        self.set_nominal_der_output(sys);

        // Initialise InjCurrent to zero (defaults to a PQ PVSystem element).
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    /// The L-N `VBase` update (Pascal `TProp.conn`/`TProp.kV` side effects):
    /// L-N for 2/3-phase, otherwise the supplied value.
    pub(super) fn update_vbase(&mut self) {
        self.base.v_base = match self.cd.nphases {
            2 | 3 => self.kv_pvsystem_base * inv_sqrt3_x1000(),
            _ => self.kv_pvsystem_base * 1000.0,
        };
    }
}

/// Whether a held curve has a point that is not a finite number.
fn curve_broken(curve: Option<&XyCurveObj>) -> bool {
    curve.is_some_and(XyCurveObj::has_point_not_finite)
}
