//! Nominal-power machinery: dispatch decision, load-shape multipliers and the
//! `SetNominalGeneration` / `RecalcElementData` path that derives `Yeq` and the
//! per-phase P/Q from the generator's kW/kvar settings.

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::solution::{SolveMode, USEDAILY, USEDUTY, USEYEARLY};
use crate::util::{CDOUBLEONE, inv_sqrt3_x1000, sqrt3};

use super::{GenDispatchMode, Generator, prop};

/// The powers the `kW`, `kvar`, `PF` and load-shape side effects derive at
/// their token, as the last recalc that built the generator left them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct HeldPowers {
    kw: f64,
    kvar: f64,
    pf: f64,
    kva: f64,
    kvar_max: f64,
    kvar_min: f64,
    p_per_phase: f64,
    q_per_phase: f64,
}

impl HeldPowers {
    /// The powers `g` holds now.
    pub(super) fn of(g: &Generator) -> Self {
        Self {
            kw: g.kw_base,
            kvar: g.kvar_base,
            pf: g.pf_nominal,
            kva: g.kva_rating,
            kvar_max: g.kvar_max,
            kvar_min: g.kvar_min,
            p_per_phase: g.p_nominal_per_phase,
            q_per_phase: g.q_nominal_per_phase,
        }
    }

    /// After a refused edit, give `g` back each power that a spoiled token of
    /// the edit derived and no property of the edit set. A token is spoiled
    /// when its input, or a power it derives, is not a finite number: a
    /// `kvar` of `+Inf` leaves the `PF` as it was, a `PF` that is not a
    /// number, or one of `1e-310` whose `kvar` overflows, the `kvar`. The
    /// powers finite tokens derive as finite numbers are kept, so the edit
    /// that makes the refused input valid builds the generator its twin
    /// builds (pinned by
    /// `exec::tests::rating_refusals::an_input_that_makes_a_stamp_non_finite_is_refused_at_the_recalc`
    /// and `::a_refused_edit_keeps_the_powers_its_valid_tokens_derive`).
    fn restore_untyped(self, g: &mut Generator) {
        use prop::*;
        let typed = |i: usize| g.cd.obj.prop_edited_since_boundary(i);
        let shape = g.actual_shape_typed_last().is_some();
        let (kw, kvar, pf) = (typed(KW), typed(KVAR), typed(PF));
        if !(kw || kvar || pf || shape) {
            return;
        }
        let kva = typed(KVA) || typed(MVA);
        let (max, min) = (typed(MAXKVAR), typed(MINKVAR));
        let bad = |x: f64| !x.is_finite();
        // The `kW` and `PF` tokens derive the `kvar`, its limits and an
        // untyped kVA rating, the `kvar` token the `PF` and the limits, a load
        // shape used as actual the `kW` and the `kvar`. A token is spoiled when
        // its input or a power it derives is not finite.
        let sync = (kw || pf)
            && ((kw && bad(g.kw_base)) || (pf && bad(g.pf_nominal)) || (!kvar && bad(g.kvar_base)));
        let kvar_token = kvar && (bad(g.kvar_base) || (!pf && bad(g.pf_nominal)));
        let shape = shape && (bad(g.kw_base) || bad(g.kvar_base));
        if shape {
            g.kw_base = self.kw;
        }
        let kvar_back = shape || (!kvar && (sync || bad(g.kvar_base)));
        if kvar_back {
            g.kvar_base = self.kvar;
        }
        if !pf && (kvar_token || shape || bad(g.pf_nominal)) {
            g.pf_nominal = self.pf;
        }
        if !kva && (sync || bad(g.kva_rating)) {
            g.kva_rating = self.kva;
        }
        let limits = kvar_back || kvar_token;
        if !max && (limits || bad(g.kvar_max)) {
            g.kvar_max = self.kvar_max;
        }
        if !min && (limits || bad(g.kvar_min)) {
            g.kvar_min = self.kvar_min;
        }
        g.p_nominal_per_phase = self.p_per_phase;
        g.q_nominal_per_phase = self.q_per_phase;
    }
}

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

    /// Pascal `SetNominalGeneration`. In dynamics/harmonics mode the generator is
    /// left in whatever state it had at the fundamental: the ON decision, the
    /// per-phase P/Q and `Yeq` are **not** recomputed (Pascal guards each block
    /// with `if not (IsDynamicModel or IsHarmonicModel)`), so the harmonic `Yeq`
    /// set by [`Generator::init_harmonics_impl`](super::Generator) survives.
    pub fn set_nominal_generation(&mut self, sys: &SysCtx) {
        let gen_on_saved = self.gen_on;
        self.shape_factor = CDOUBLEONE;
        let harm_or_dyn = sys.is_harmonic_model || sys.is_dynamic_model;

        // Decide whether the generator is ON (LOADMODE compares the dispatch
        // reference, PRICEMODE the price signal, both against DispValue).
        if !harm_or_dyn {
            self.gen_on = true;
            if !self.forced_on && self.dispatch_value > 0.0 {
                let off_load = self.dispatch_mode == GenDispatchMode::LoadLevel
                    && sys.generator_dispatch_reference < self.dispatch_value;
                let off_price = self.dispatch_mode == GenDispatchMode::Price
                    && sys.price_signal < self.dispatch_value;
                if off_load || off_price {
                    self.gen_on = false;
                }
            }
        }

        let nphases = self.cd.nphases as f64;
        if !self.gen_on {
            // OFF: a tiny resistive load so the matrix doesn't go singular (this
            // runs in harmonics mode too — but `gen_on` is then the fundamental
            // state, never recomputed above).
            self.p_nominal_per_phase = -0.1 * self.kw_base / nphases;
            self.q_nominal_per_phase = 0.0;
        } else if !harm_or_dyn {
            // (Pascal computes a `factor` here even in harmonics/dynamics mode. In
            // harmonics it is exactly unused — HARMONICMODE is not in the `case`, so
            // `Factor:=1` with no shape call. In dynamics the GENERALTIME/DYNAMICMODE
            // arm *can* call a shape mult, but its only result is `ShapeFactor`, which
            // is unread in those modes here (Pnom/Yeq below are not recomputed), so
            // skipping is behaviorally identical until the WP7.7 dynamics solve lands.)
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
                        // GENERALTIME / DYNAMICMODE: the one class
                        // `ActiveLoadShapeClass` selects (`Set LoadShapeClass=`);
                        // `USENONE` (the default) leaves ShapeFactor at 1+j1.
                        match sys.active_load_shape_class {
                            USEDAILY => self.calc_daily_mult(sys.dbl_hour),
                            USEYEARLY => self.calc_yearly_mult(sys.dbl_hour),
                            USEDUTY => self.calc_duty_mult(sys.dbl_hour),
                            _ => {} // USENONE
                        }
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

        if !harm_or_dyn {
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
                    Complex64::new(self.p_nominal_per_phase, -self.q_nominal_per_phase)
                        / self.v_base95;
                self.model7_max_phase_curr = self.phase_current_limit.norm();
            }
        }

        // If the generator state changes, force a Y rebuild.
        if self.gen_on != gen_on_saved {
            self.cd.yprim_invalid = true;
        }
    }

    /// Recompute the derived values, refused when an input makes
    /// the power-flow admittance of the configured model, or the `kvar` a
    /// typed `PF` derives, or the `model=3` current through a target voltage
    /// `Vpu` that is not a number, not a finite number: the edit logs the
    /// refusal, every derived value keeps what it held, a power a refused
    /// token derived included, and the stamp is refused until a recalc finds
    /// it finite (pinned by
    /// `exec::tests::rating_refusals::an_input_that_makes_a_stamp_non_finite_is_refused_at_the_recalc`).
    pub fn recalc(&mut self, sys: &SysCtx) {
        let mut held = self.clone();
        self.derive(sys);
        let finite = |z: Complex64| z.re.is_finite() && z.im.is_finite();
        let yeq = finite(self.yeq);
        let pf_kvar = self.pf_kvar().is_none_or(f64::is_finite);
        let quantity = if !(yeq && self.yq_fixed.is_finite() && pf_kvar) {
            "admittance"
        } else if self.gen_model == 3 && self.v_target.is_nan() {
            // The voltage-regulating model drives its vars toward the target
            // voltage, so a target that is not a number makes its current one.
            "model current"
        } else {
            self.cd.recalc_refusal = None;
            self.held_powers = HeldPowers::of(self);
            // The user-written models are updated once the recalc is accepted.
            self.update_user_models(sys);
            return;
        };
        held.user_model = self.user_model.take();
        held.shaft_model = self.shaft_model.take();
        *self = held;
        let input = if quantity == "model current" {
            use crate::obj::base::DssObject;
            crate::elements::ckt::input_text("Vpu", self.get_f64(super::prop::VPU))
        } else {
            self.refused_input(yeq)
        };
        let powers = self.held_powers;
        powers.restore_untyped(self);
        self.cd.refuse_recalc("Generator", &input, quantity);
    }

    /// The `kvar` a typed `PF` derives from `kW`, where the `PF` is the last
    /// of the two typed and not zero (a zero `PF` derives nothing). A refused
    /// `PF` gives the stored `kvar` back, so the recalc checks this one too
    /// and the generator stays refused until the `PF` or a `kvar` is typed
    /// valid (pinned by
    /// `exec::tests::rating_refusals::a_refused_edit_keeps_the_powers_its_valid_tokens_derive`).
    fn pf_kvar(&self) -> Option<f64> {
        let typed = |i: usize| self.cd.obj.prp_specified(i);
        (typed(prop::PF) && !typed(prop::KVAR) && self.pf_nominal != 0.0)
            .then(|| self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt())
    }

    /// The input a refused recalc names: the first that is not finite, else
    /// a zero, else the cause of the overflow. A power a side effect derived
    /// is never named: the `PF` and the `kvar` are inputs only when typed
    /// last of the two, and model 6 reads its kVA rating through the `kW`
    /// unless the kVA is typed. Where the model-6 `Yeq` is not finite
    /// (`yeq` false) it names a `kV` or `Xd` whose square leaves the normal
    /// range, else the kVA rating it divides by. Otherwise it names a zero
    /// square of `kV`, else the input of a power (`kW` or `kvar` times 1000)
    /// that is not finite, with a typed `PF` where the `kvar` it derives
    /// overflows on `1/PF²`, else `kV`, the divisor of the admittance.
    fn refused_input(&self, yeq: bool) -> String {
        use super::prop::*;
        use crate::obj::base::DssObject;
        let v = |i: usize| self.get_f64(i);
        let typed = |i: usize| self.cd.obj.prp_specified(i);
        if let Some(shape) = self.shape_powers_refused() {
            return shape;
        }
        let model6 = self.gen_model == 6 && !yeq;
        let kva = if self.kva_not_set {
            ("kW", v(KW))
        } else {
            ("kVA", v(KVA))
        };
        let mut inputs = vec![("kV", v(KV)), ("kW", v(KW))];
        inputs.extend(typed(PF).then_some(("PF", v(PF))));
        inputs.extend(typed(KVAR).then_some(("kvar", v(KVAR))));
        if model6 {
            inputs.extend([kva, ("Xd", v(XD))]);
        }
        let zeros: &[(&str, f64)] = if model6 {
            &[("kV", v(KV)), kva, ("Xd", v(XD))]
        } else {
            &[("kV", v(KV))]
        };
        let power = |kw: f64| !(1000.0 * kw / self.cd.nphases as f64).is_finite();
        let normal = |x: f64| (x * x).is_normal();
        let fallback = if model6 {
            if !normal(self.kv_generator_base) {
                ("kV", v(KV))
            } else if !normal(self.pu_xd) {
                ("Xd", v(XD))
            } else {
                kva
            }
        } else if self.v_base.powi(2) == 0.0 {
            ("kV", v(KV))
        } else if power(self.kw_base) {
            ("kW", v(KW))
        } else if self.pf_kvar().is_some_and(power) {
            if (1.0 / self.pf_nominal.powi(2)).is_finite() {
                ("kW", v(KW))
            } else {
                ("PF", v(PF))
            }
        } else if power(self.kvar_base) {
            ("kvar", v(KVAR))
        } else {
            ("kV", v(KV))
        };
        crate::elements::ckt::refused_input_text(&inputs, zeros, fallback)
    }

    /// The load shape used as actual that this edit typed after the `kW` and
    /// the `kvar`, as its property name and its shape: it set both to its
    /// peak powers.
    fn actual_shape_typed_last(&self) -> Option<(&'static str, &str)> {
        use super::prop::*;
        let obj = &self.cd.obj;
        let shapes = [
            (YEARLY, "Yearly", &self.yearly_shape, &self.yearly_shape_obj),
            (DAILY, "Daily", &self.daily_shape, &self.daily_shape_obj),
            (DUTY, "Duty", &self.duty_shape, &self.duty_shape_obj),
        ];
        let (i, name, shape, _) = shapes
            .into_iter()
            .filter(|(i, _, _, s)| {
                obj.prop_edited_since_boundary(*i) && s.as_ref().is_some_and(|s| s.use_actual())
            })
            .max_by_key(|(i, ..)| obj.seq_mark(*i))?;
        (obj.seq_mark(i) > obj.seq_mark(KW).max(obj.seq_mark(KVAR)))
            .then_some((name, shape.as_str()))
    }

    /// The load shape whose peak powers are not finite numbers, as
    /// `Daily=<name>`, named in place of the powers it set.
    fn shape_powers_refused(&self) -> Option<String> {
        if self.kw_base.is_finite() && self.kvar_base.is_finite() {
            return None;
        }
        let (name, shape) = self.actual_shape_typed_last()?;
        Some(format!("{name}={shape}"))
    }

    /// The refusal of a dynamics or harmonics `quantity`, the admittance of
    /// the reactance `reactance` (`Xdp` with its X/R ratio `XRdp`, or `Xdpp`),
    /// that is not a finite number.
    pub(super) fn mode_refusal(&self, reactance: usize, quantity: &str) -> String {
        use super::prop::*;
        use crate::obj::base::DssObject;
        let v = |i: usize| self.get_f64(i);
        let x = if reactance == XDP {
            ("Xdp", v(XDP))
        } else {
            ("Xdpp", v(XDPP))
        };
        let mut inputs = vec![("kV", v(KV)), ("kVA", v(KVA)), x];
        if reactance == XDP {
            inputs.push(("XRdp", v(XRDP)));
        }
        let zeros = [("kV", v(KV)), ("kVA", v(KVA)), x];
        let input = crate::elements::ckt::refused_input_text(&inputs, &zeros, ("kVA", v(KVA)));
        self.cd.refusal_message("Generator", &input, quantity)
    }

    /// Whether the dynamics model divides by `Xdpp`: a three-phase machine
    /// forms its negative-sequence current through it, bar the `model=7`
    /// inverter, and its zero-sequence current where it is a wye that is not
    /// forced balanced. The user-written `model=6` reads none.
    pub(super) fn dynamics_reads_xdpp(&self) -> bool {
        self.cd.nphases == 3
            && self.gen_model != 6
            && (self.gen_model != 7
                || (self.connection == super::Connection::Wye && !self.force_balanced))
    }

    /// The body of [`Self::recalc`], bar the user-model update.
    fn derive(&mut self, sys: &SysCtx) {
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
