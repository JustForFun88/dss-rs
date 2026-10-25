//! Nominal-power computation for `TLoadObj`: `RecalcElementData`/
//! `SetNominalLoad`, the growth factor and the daily/yearly/duty/CVR shape
//! multipliers, the `SetkWkvar` bookkeeping, the shared VBase update, and the
//! load-allocation helpers (`ComputeAllocatedLoad`/`Set_AllocationFactor`/
//! `Set_kVAAllocationFactor`).

use num_complex::Complex64;

use crate::elements::traits::SysCtx;
use crate::solution::{RandomType, SolveMode, USEDAILY, USEDUTY, USEYEARLY};
use crate::support::mathutil::{FpcRng, gauss, quasi_log_normal};
use crate::util::{CDOUBLEONE, inv_sqrt3_x1000};

use super::{Connection, Load, LoadModel, LoadSpec, LoadStatus, prop};

impl Load {
    /// Pascal `GrowthFactor` (`Load.pas:1014`, B3-r3723 / SVN r3723-era update):
    /// year 0 → 1.0 (use base values), UNLESS a `GrowthShape` is assigned — then
    /// the factor tracks the *simulated hours* so a >8760 h Year=0 run advances
    /// through the curve. `calcYear := dblHour/8760`; if `calcYear < 1` the factor
    /// stays 1.0 unless the curve's first year is 0 (then `GetMultIdx(1)`), else
    /// `GetMult(Ceil(calcYear))`. For `year <> 0` it is the `GrowthShape`'s
    /// `GetMult(Year)` when assigned, else the circuit default growth factor.
    /// (Pascal never updates `LastYear` here, so a fresh `Year <> LastYear` always
    /// re-reads the curve — ported verbatim.)
    fn growth_factor(&mut self, year: i32, default_growth_factor: f64, dbl_hour: f64) -> f64 {
        if year == 0 {
            self.last_growth_factor = 1.0;
            if let Some(gs) = self.growth_shape_obj.as_mut() {
                let first_y = gs.get_year(1);
                let calc_year = dbl_hour / 8760.0; // Aprox year
                if calc_year < 1.0 {
                    if first_y == 0.0 {
                        self.last_growth_factor = gs.get_mult_idx(1);
                    }
                } else {
                    self.last_growth_factor = gs.get_mult(calc_year.ceil() as i32);
                }
            }
        } else if let Some(gs) = self.growth_shape_obj.as_mut() {
            if year != self.last_year {
                self.last_growth_factor = gs.get_mult(year);
            }
        } else {
            self.last_growth_factor = default_growth_factor;
        }
        self.last_growth_factor
    }

    /// Pascal `CalcDailyMult`: set `ShapeFactor`/`ShapeIsActual` from the daily
    /// shape (default `(1, 1)` when none).
    fn calc_daily_mult(&mut self, hr: f64) {
        if let Some(s) = self.daily_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = CDOUBLEONE;
        }
    }

    /// Pascal `CalcDutyMult`: falls back to the daily shape when no duty shape.
    fn calc_duty_mult(&mut self, hr: f64) {
        if let Some(s) = self.duty_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.calc_daily_mult(hr);
        }
    }

    /// Pascal `CalcYearlyMult` (the yearly curve is assumed hourly).
    fn calc_yearly_mult(&mut self, hr: f64) {
        if let Some(s) = self.yearly_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = CDOUBLEONE;
        }
    }

    /// Pascal `CalcCVRMult` (used in yearly simulations of model-4 CVR loads):
    /// the CVR shape supplies time-varying watt/var factors. Leaves them
    /// unchanged when no CVR shape is assigned.
    fn calc_cvr_mult(&mut self, hr: f64) {
        if let Some(s) = self.cvr_shape_obj.as_mut() {
            let f = s.get_mult_at_hour(hr);
            self.cvr_watt_factor = f.re;
            self.cvr_var_factor = f.im;
        }
    }

    /// Pascal `SetkWkvar`: set the base kW/kvar directly (used by the
    /// `UseActual` shape side effects), with the property-sequence bookkeeping
    /// and `LoadSpecType` selection that the text path performs.
    pub(super) fn set_kw_kvar(&mut self, p_kw: f64, q_kvar: f64) {
        use prop::*;
        self.kw_base = p_kw;
        self.kvar_base = q_kvar;
        self.cd.obj.clear_seq(KVA);
        self.cd.obj.clear_seq(KWH);
        self.cd.obj.clear_seq(XFKVA);
        if self.pf_specified {
            self.cd.obj.set_as_next_seq(PF);
            self.cd.obj.clear_seq(KVAR);
            self.load_spec_type = LoadSpec::KwPf;
        } else {
            self.cd.obj.set_as_next_seq(KVAR);
            self.cd.obj.clear_seq(PF);
            self.load_spec_type = LoadSpec::KwKvar;
        }
    }

    /// Pascal `TLoadObj.Randomize` (`Load.pas:899`): set `RandomMult` from the
    /// solution's random type. `RandomType::None` (`Set random=none`) → `1.0`
    /// and draws nothing; Gaussian/Uniform/LogNormal draw through the engine RNG
    /// (the yearly shape's mean/std-dev when one is assigned, else `puMean`/
    /// `puStdDev`). Called once per load per MonteCarlo1 case by `solve_monte1`.
    pub fn randomize(&mut self, opt: RandomType, rng: &mut FpcRng) {
        self.random_mult = match opt {
            RandomType::Gaussian => match self.yearly_shape_obj.as_ref() {
                Some(s) => gauss(s.mean(), s.std_dev(), || rng.next_f64()),
                None => gauss(self.pu_mean, self.pu_std_dev, || rng.next_f64()),
            },
            RandomType::Uniform => rng.next_f64(),
            RandomType::LogNormal => match self.yearly_shape_obj.as_ref() {
                Some(s) => quasi_log_normal(s.mean(), || rng.next_f64()),
                None => quasi_log_normal(self.pu_mean, || rng.next_f64()),
            },
            // none: RandomMult := 1.0.
            RandomType::None => 1.0,
        };
    }

    /// Pascal `SetNominalLoad`.
    pub fn set_nominal_load(&mut self, sys: &SysCtx) {
        self.shape_factor = CDOUBLEONE;
        self.shape_is_actual = false;

        let factor = if self.status == LoadStatus::Fixed {
            // Fixed: consider only the growth factor.
            self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour)
        } else {
            match sys.mode {
                SolveMode::Snapshot | SolveMode::Harmonic => {
                    if self.status == LoadStatus::Exempt {
                        // Exempt
                        self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour)
                    } else {
                        sys.load_multiplier
                            * self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour)
                    }
                }
                SolveMode::Daily => {
                    let mut f =
                        self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour);
                    if self.status != LoadStatus::Exempt {
                        f *= sys.load_multiplier;
                    }
                    self.calc_daily_mult(sys.dbl_hour);
                    f
                }
                SolveMode::Yearly => {
                    let f = sys.load_multiplier
                        * self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour);
                    self.calc_yearly_mult(sys.dbl_hour);
                    if self.load_model == LoadModel::Cvr {
                        self.calc_cvr_mult(sys.dbl_hour);
                    }
                    f
                }
                SolveMode::DutyCycle => {
                    let mut f =
                        self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour);
                    if self.status != LoadStatus::Exempt {
                        f *= sys.load_multiplier;
                    }
                    self.calc_duty_mult(sys.dbl_hour);
                    f
                }
                SolveMode::Time | SolveMode::Dynamic => {
                    // Pascal `GENERALTIME`/`DYNAMICMODE`: growth × load-multiplier
                    // (unless Exempt); the ShapeFactor comes from the one class
                    // `ActiveLoadShapeClass` selects (`Set LoadShapeClass=`).
                    // `USENONE` (the default) falls through, leaving 1+j1.
                    let mut f =
                        self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour);
                    if self.status != LoadStatus::Exempt {
                        f *= sys.load_multiplier;
                    }
                    match sys.active_load_shape_class {
                        USEDAILY => self.calc_daily_mult(sys.dbl_hour),
                        USEYEARLY => self.calc_yearly_mult(sys.dbl_hour),
                        USEDUTY => self.calc_duty_mult(sys.dbl_hour),
                        _ => {} // USENONE: ShapeFactor stays 1+j1
                    }
                    f
                }
                // Pascal groups Monte2/Monte3/LOADDURATION1/LOADDURATION2 in one
                // case arm: growth × the load's own daily-shape lookup (via
                // `CalcDailyMult`, exactly like `DAILYMODE`) × LoadMultiplier
                // unless Exempt. Monte2/Monte3 are not reachable yet (WPG.4 —
                // the solve dispatcher still errors loudly on them), but LD1/LD2
                // are (WPG.3), so this arm is live.
                SolveMode::Monte2 | SolveMode::Monte3 | SolveMode::LD1 | SolveMode::LD2 => {
                    let mut f =
                        self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour);
                    self.calc_daily_mult(sys.dbl_hour);
                    if self.status != LoadStatus::Exempt {
                        f *= sys.load_multiplier;
                    }
                    f
                }
                // Pascal `PEAKDAY` (`Load.pas:1092`): growth × the load's own
                // daily-shape lookup, with **no** `LoadMultiplier` — the peak
                // kW is taken as given and only shaped by the daily curve and
                // year growth (that omission is the whole point of PeakDay vs
                // Daily). Kept a separate arm from Monte2/Monte3/LD1/LD2 above
                // precisely because those apply `LoadMultiplier` and this must
                // not. Every sibling PC element already routes PeakDay through
                // its daily-mult; Load had silently fallen through to the
                // growth-only catch-all (flat nominal kW) — the bug this fixes.
                SolveMode::PeakDay => {
                    let f = self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour);
                    self.calc_daily_mult(sys.dbl_hour);
                    f
                }
                // Pascal `MONTECARLO1` (`Load.pas:1074`): `Factor := RandomMult *
                // GrowthFactor(Year)` × `LoadMultiplier` (unless Exempt), with a
                // **unit** ShapeFactor (no daily lookup). Upstream calls
                // `Randomize(RandomType)` right here to (re)draw `RandomMult`;
                // `solve_monte1` hoists that draw to the top of each case
                // (`randomize_all_loads`), so this arm consumes the already-drawn
                // `random_mult` — the net effect (a fresh draw feeding each M1
                // SolveSnap) is identical, and the RNG stream is engine-global
                // like FPC's RTL. Under `Set random=none` the draw is a no-op
                // (`randomize` sets `RandomMult := 1.0`), so the whole arm reduces
                // to growth × LoadMultiplier — the deterministic gated path
                // (GAPS_PLAN.md §2.1).
                SolveMode::Monte1 => {
                    let mut f = self.random_mult
                        * self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour);
                    if self.status != LoadStatus::Exempt {
                        f *= sys.load_multiplier;
                    }
                    f
                }
                // AutoAdd/... are not reachable yet — the solve dispatcher still
                // errors loudly on them — so they default to growth-only with a
                // unit ShapeFactor, matching the Pascal trailing `else`; wired in
                // later phases as those modes land.
                _ => self.growth_factor(sys.year, sys.default_growth_factor, sys.dbl_hour),
            }
        };

        let nphases = self.cd.nphases as f64;
        let shape_factor = self.shape_factor;
        if self.shape_is_actual {
            self.w_nominal = 1000.0 * shape_factor.re / nphases;
            self.var_nominal = 0.0;
            if shape_factor.im != 0.0 {
                self.var_nominal = 1000.0 * shape_factor.im / nphases;
            } else if self.pf_specified && self.pf_nominal != 1.0 {
                self.var_nominal = self.w_nominal * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                if self.pf_nominal < 0.0 {
                    self.var_nominal = -self.var_nominal;
                }
            }
        } else {
            self.w_nominal = 1000.0 * self.kw_base * factor * shape_factor.re / nphases;
            self.var_nominal = 1000.0 * self.kvar_base * factor * shape_factor.im / nphases;
        }

        self.yeq = Complex64::new(self.w_nominal, -self.var_nominal) / self.v_base.powi(2);
        self.yeq95 = if self.vminpu != 0.0 {
            self.yeq / self.vminpu.powi(2) // at 95% voltage
        } else {
            Complex64::ZERO
        };
        self.yeq105 = if self.vmaxpu != 0.0 {
            self.yeq / self.vmaxpu.powi(2) // at 105% voltage
        } else {
            self.yeq
        };
        self.yeq105i = if self.vmaxpu != 0.0 {
            self.yeq / self.vmaxpu // at 105% voltage for Constant I
        } else {
            self.yeq
        };

        // New code to help with convergence at low voltages.
        self.i_low = self.yeq * self.v_base_low;
        self.i95 = self.yeq95 * self.v_base95;
        self.m95 = (self.i95 - self.i_low) / (self.v_base95 - self.v_base_low);
        self.i_base = self.yeq * self.v_base;
        self.m95i = (self.i_base - self.i_low) / (self.v_base95 - self.v_base_low);
    }

    /// Recompute the derived values, refused when an input makes the
    /// admittance or a power base not a finite number: the edit logs the
    /// refusal, every derived value keeps what it held and the stamp is
    /// refused until a recalc finds them finite (pinned by
    /// `exec::tests::rating_refusals::an_input_that_makes_a_stamp_non_finite_is_refused_at_the_recalc`).
    /// `sys` provides the solve-state scalars `SetNominalLoad` reads (every
    /// value is recomputed again in `CalcYPrim` before it is consumed).
    pub fn recalc(&mut self, sys: &SysCtx) {
        let held = Derived::of(self);
        self.derive(sys);
        match self.not_finite_quantity() {
            None => self.cd.recalc_refusal = None,
            Some(quantity) => {
                // The input is named from the values this recalc derived.
                let input = if quantity == "model current" {
                    crate::elements::ckt::array_input_text("ZIPV", &self.zipv, 1.0)
                } else {
                    self.refused_input()
                };
                held.restore(self);
                self.cd.refuse_recalc("Load", &input, quantity);
            }
        }
    }

    /// The derived quantity a refused recalc names, `None` when all are finite.
    /// A voltage region of zero width (`vminpu=vlowpu`) is not checked: the
    /// solve never enters it. Only the wye stamp reads the neutral admittance.
    fn not_finite_quantity(&self) -> Option<&'static str> {
        let finite = |z: Complex64| z.re.is_finite() && z.im.is_finite();
        let allocated = self
            .allocated_load()
            .is_some_and(|(kw, kvar)| !(kw.is_finite() && kvar.is_finite()));
        let neutral = self.connection == Connection::Wye && !finite(self.y_neut);
        if allocated
            || !finite(self.yeq)
            || neutral
            || !self.yq_fixed.is_finite()
            || !self.kw_base.is_finite()
            || !self.kvar_base.is_finite()
        {
            Some("admittance")
        } else if !self.kva_base.is_finite() {
            Some("kVA base")
        } else if self.load_model == LoadModel::Zipv
            && !self.zipv[..6].iter().all(|z| z.is_finite())
        {
            // A ZIP weight that is not finite makes the model-8 current not
            // finite at every voltage above `VLowpu`. The cut-off voltage of
            // the seventh entry only scales the current.
            Some("model current")
        } else {
            None
        }
    }

    /// The input a refused recalc names: the first input the specification
    /// of the load reads that is not a finite number, else the zero that
    /// breaks the formula, else the cause of the overflow. A power base that
    /// is not finite names the divisor `kWhDays` or `PF` its formula overflows
    /// on, else its multiplied input of the largest magnitude. With finite
    /// powers, an admittance over the voltage base names the divisor `kV`, and
    /// a neutral admittance `1 / (Rneut + j Xneut)` names `Rneut` and `Xneut`
    /// together (pinned by `an_overflow_of_finite_inputs_names_its_cause`).
    /// `Rneut` and `Xneut` are inputs only where the neutral is read: a delta
    /// load stamps none and a negative `Rneut` leaves it open (pinned by
    /// `an_input_that_makes_a_stamp_non_finite_is_refused_at_the_recalc`).
    fn refused_input(&self) -> String {
        use crate::elements::ckt::input_text;
        use crate::obj::base::DssObject;
        use prop::*;
        let reads: &[usize] = match self.load_spec_type {
            LoadSpec::KwPf => &[KW, PF],
            LoadSpec::KwKvar => &[KW, KVAR],
            LoadSpec::KvaPf => &[PF, KVA],
            LoadSpec::ConnectedKvaPf => &[PF, XFKVA, ALLOCATIONFACTOR],
            LoadSpec::KwhPf => &[PF, KWH, KWHDAYS, CFACTOR],
        };
        let name = |idx: usize| match idx {
            KV => "kV",
            KW => "kW",
            PF => "PF",
            KVAR => "kvar",
            RNEUT => "Rneut",
            XNEUT => "Xneut",
            XFKVA => "XfkVA",
            ALLOCATIONFACTOR => "AllocationFactor",
            KVA => "kVA",
            KWH => "kWh",
            KWHDAYS => "kWhDays",
            _ => "CFactor",
        };
        let text = |idx: usize| input_text(name(idx), self.get_f64(idx));
        let neutral_read =
            self.connection == Connection::Wye && (self.rneut >= 0.0 || self.rneut.is_nan());
        let mut inputs = vec![KV];
        if neutral_read {
            inputs.extend_from_slice(&[RNEUT, XNEUT]);
        }
        inputs.extend_from_slice(reads);
        inputs.sort_unstable();
        if let Some(&idx) = inputs.iter().find(|&&i| !self.get_f64(i).is_finite()) {
            return text(idx);
        }
        let zero = [KV, PF, KWHDAYS]
            .into_iter()
            .find(|&i| (i == KV || reads.contains(&i)) && self.get_f64(i) == 0.0);
        if let Some(idx) = zero {
            return text(idx);
        }
        if self.v_base.powi(2) == 0.0 {
            return text(KV);
        }
        // The multiplied input of the largest magnitude, the first in
        // property order on a tie.
        let products: &[usize] = match self.load_spec_type {
            LoadSpec::KwPf => &[KW],
            LoadSpec::KwKvar => &[KW, KVAR],
            LoadSpec::KvaPf => &[KVA],
            LoadSpec::ConnectedKvaPf => &[XFKVA, ALLOCATIONFACTOR],
            LoadSpec::KwhPf => &[KWH, CFACTOR],
        };
        let product = products.iter().copied().fold(products[0], |best, i| {
            if self.get_f64(i).abs() > self.get_f64(best).abs() {
                i
            } else {
                best
            }
        });
        let finite = |z: Complex64| z.re.is_finite() && z.im.is_finite();
        let powers_finite = self.kw_base.is_finite()
            && self.kvar_base.is_finite()
            && self
                .allocated_load()
                .is_none_or(|(kw, kvar)| kw.is_finite() && kvar.is_finite());
        if !powers_finite {
            if reads.contains(&KWHDAYS) && !(self.kwh / (self.kwh_days * 24.0)).is_finite() {
                return text(KWHDAYS);
            }
            if reads.contains(&PF) && !(1.0 / self.pf_nominal.powi(2)).is_finite() {
                return text(PF);
            }
            return text(product);
        }
        let stamp_powers =
            self.w_nominal.is_finite() && self.var_nominal.is_finite() && self.var_base.is_finite();
        if stamp_powers && !(finite(self.yeq) && self.yq_fixed.is_finite()) {
            return text(KV);
        }
        if self.connection == Connection::Wye && !finite(self.y_neut) {
            return format!("{} {}", text(RNEUT), text(XNEUT));
        }
        text(product)
    }

    /// The refusal of a harmonic admittance that is not a finite number. The
    /// motor reactance divides by `XRHarm` and by the kVA base.
    pub(super) fn harmonic_refusal(&self) -> String {
        use crate::obj::base::DssObject;
        use prop::*;
        let v = |i: usize| self.get_f64(i);
        let inputs = [
            ("kV", v(KV)),
            ("kVA", v(KVA)),
            ("%SeriesRL", self.pu_series_rl * 100.0),
            ("puXHarm", v(PUXHARM)),
            ("XRHarm", v(XRHARM)),
        ];
        let zeros = [("kV", v(KV)), ("XRHarm", v(XRHARM))];
        let input = crate::elements::ckt::refused_input_text(&inputs, &zeros, ("kVA", v(KVA)));
        self.cd
            .refusal_message("Load", &input, "harmonic admittance")
    }

    /// The body of [`Self::recalc`].
    fn derive(&mut self, sys: &SysCtx) {
        self.v_base_low = self.vlowpu * self.v_base;
        self.v_base95 = self.vminpu * self.v_base;
        self.v_base105 = self.vmaxpu * self.v_base;

        // Set kW and kvar from root values of kVA and PF.
        match self.load_spec_type {
            LoadSpec::KwPf => {
                self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                if self.pf_nominal < 0.0 {
                    self.kvar_base = -self.kvar_base;
                }
                self.kva_base = magnitude(self.kw_base, self.kvar_base);
            }
            LoadSpec::KwKvar => {
                self.kva_base = magnitude(self.kw_base, self.kvar_base);
                if self.kva_base > 0.0 {
                    self.pf_nominal = self.kw_base / self.kva_base;
                    if self.kvar_base != 0.0 {
                        self.pf_nominal *= (self.kw_base * self.kvar_base).signum();
                    }
                }
            }
            LoadSpec::KvaPf => {
                self.kw_base = self.kva_base * self.pf_nominal.abs();
                self.kw_ref = self.kw_base;
                self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                self.kvar_ref = self.kvar_base;
                if self.pf_nominal < 0.0 {
                    self.kvar_base = -self.kvar_base;
                }
            }
            LoadSpec::ConnectedKvaPf | LoadSpec::KwhPf => {
                if self.pf_changed {
                    self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                    if self.pf_nominal < 0.0 {
                        self.kvar_base = -self.kvar_base;
                    }
                    self.kva_base = magnitude(self.kw_ref, self.kvar_ref);
                }
            }
        }

        self.set_nominal_load(sys);

        self.y_neut = if self.rneut < 0.0 {
            Complex64::ZERO // flag for open neutral
        } else if self.rneut == 0.0 && self.xneut == 0.0 {
            Complex64::new(1.0e6, 0.0) // solidly grounded: 1 µΩ resistor
        } else {
            Complex64::new(self.rneut, self.xneut).inv()
        };

        self.var_base = 1000.0 * self.kvar_base / self.cd.nphases as f64;
        self.yq_fixed = -self.var_base / self.v_base.powi(2);

        self.pf_changed = false;
    }

    /// The shared VBase update from the kV/phases/conn side effects.
    pub(super) fn update_vbase(&mut self) {
        self.v_base = match self.connection {
            Connection::Delta => self.kv_load_base * 1000.0,
            Connection::Wye => match self.cd.nphases {
                2 | 3 => self.kv_load_base * inv_sqrt3_x1000(),
                _ => self.kv_load_base * 1000.0,
            },
        };
    }

    /// Set the allocation factor `AllocateLoads` gives this load: a
    /// connected-kVA load takes it as its kVA factor and a kWh load as its
    /// `CFactor`, and the kW and kvar base follow
    /// (`compute_allocated_load`). A load specified by its kW, kVA or
    /// kvar only stores it.
    pub fn set_allocation_factor(&mut self, value: f64) {
        self.allocation_factor = value;
        match self.load_spec_type {
            LoadSpec::ConnectedKvaPf => self.kva_allocation_factor = value,
            LoadSpec::KwhPf => self.c_factor = value,
            _ => {}
        }
        self.compute_allocated_load();
        self.has_been_allocated = true;
    }

    /// Set the factor `Set AllocationFactors` gives every load: the load
    /// becomes a connected-kVA load, `XfkVA` and `PF` are its inputs in the
    /// property order and `kVA`, `kvar`, `kW` and `kWh` are not, and the kW
    /// and kvar base follow where it has a connected kVA
    /// (`compute_allocated_load`).
    pub fn set_kva_allocation_factor(&mut self, value: f64) {
        use prop::*;
        self.kva_allocation_factor = value;
        self.allocation_factor = value;
        self.load_spec_type = LoadSpec::ConnectedKvaPf;
        self.cd.obj.set_as_next_seq(XFKVA);
        self.cd.obj.set_as_next_seq(PF);
        self.cd.obj.clear_seq(KVA);
        self.cd.obj.clear_seq(KVAR);
        self.cd.obj.clear_seq(KW);
        self.cd.obj.clear_seq(KWH);
        self.compute_allocated_load();
        self.has_been_allocated = true;
    }

    /// The allocation factor `AllocateLoads` scales.
    pub fn allocation_factor(&self) -> f64 {
        self.allocation_factor
    }

    /// Commit the kW and kvar base of a connected-kVA or a kWh load at a
    /// property token or an allocation command when it is a finite number.
    /// The recalc that ends the edit refuses it otherwise, and an allocation
    /// command refuses it first ([`Self::allocation_refusal`]).
    pub(super) fn compute_allocated_load(&mut self) {
        let finite = self
            .allocated_load()
            .filter(|(kw, kvar)| kw.is_finite() && kvar.is_finite());
        if let Some((kw, kvar)) = finite {
            self.kw_base = kw;
            self.kvar_base = kvar;
        }
    }

    /// The message `command` logs when the allocation factor `factor` it sets
    /// outside an edit would leave this load unbuilt, `None` when the load
    /// builds with it. No recalc follows these commands, so the factor is
    /// applied to a copy of the load and refused where the copy's recalc
    /// refuses it, as the edit `allocationfactor=` would, and the command then
    /// changes no load (pinned by
    /// `exec::tests::rating_refusals::an_allocation_factor_set_outside_an_edit_is_refused_where_its_load_would_not_build`).
    /// A load refused as it stands is not checked, since the factor is not
    /// what breaks it: the command skips it ([`Self::allocation_skip`]).
    pub fn allocation_refusal(
        &self,
        command: AllocationCommand,
        factor: f64,
        sys: &SysCtx,
    ) -> Option<String> {
        if self.cd.recalc_refusal.is_some() {
            return None;
        }
        let mut trial = self.clone();
        match command {
            AllocationCommand::Set => trial.set_kva_allocation_factor(factor),
            AllocationCommand::Allocate => trial.set_allocation_factor(factor),
        }
        trial.derive(sys);
        let quantity = trial.not_finite_quantity()?;
        Some(format!(
            "{}: {} makes the {quantity} of Load.{} not a finite number, so {}.",
            command.name(),
            crate::elements::ckt::input_text("AllocationFactor", factor),
            self.cd.obj.name(),
            command.effect()
        ))
    }

    /// The message `command` logs when it skips this load, `None` when it
    /// gives the load its factor. A load refused as it stands is skipped: the
    /// factor is not what breaks it, so it keeps the factor it holds, which
    /// its next recalc reads, and every other load takes the new one (pinned by
    /// `exec::tests::rating_refusals::an_allocation_factor_set_outside_an_edit_is_refused_where_its_load_would_not_build`).
    pub fn allocation_skip(&self, command: AllocationCommand) -> Option<String> {
        self.cd.recalc_refusal.as_ref()?;
        Some(format!(
            "{}: Load.{} is refused as it stands, so it keeps {}.",
            command.name(),
            self.cd.obj.name(),
            crate::elements::ckt::input_text("AllocationFactor", self.allocation_factor)
        ))
    }

    /// The kW and kvar base of a connected-kVA or a kWh load, from its inputs.
    fn allocated_load(&self) -> Option<(f64, f64)> {
        let factor = match self.load_spec_type {
            LoadSpec::KwhPf => self.c_factor,
            _ => self.allocation_factor,
        };
        self.allocated_load_at(self.load_spec_type, factor)
    }

    /// The kW and kvar base of a load of specification `spec` at the
    /// allocation factor `factor` (the `CFactor` of a kWh load).
    fn allocated_load_at(&self, spec: LoadSpec, factor: f64) -> Option<(f64, f64)> {
        let kw = match spec {
            LoadSpec::ConnectedKvaPf if self.connected_kva > 0.0 => {
                self.connected_kva * factor * self.pf_nominal.abs()
            }
            LoadSpec::KwhPf => {
                let f_avg_kw = self.kwh / (self.kwh_days * 24.0);
                f_avg_kw * factor
            }
            _ => return None,
        };
        let mut kvar = kw * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
        if self.pf_nominal < 0.0 {
            kvar = -kvar;
        }
        Some((kw, kvar))
    }
}

/// A command that sets allocation factors outside an edit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllocationCommand {
    /// `Set AllocationFactors`: one factor for every load, each made a
    /// connected-kVA load.
    Set,
    /// `AllocateLoads`: a factor per zone load, read in its own specification.
    Allocate,
}

impl AllocationCommand {
    fn name(self) -> &'static str {
        match self {
            Self::Set => "Set AllocationFactors",
            Self::Allocate => "AllocateLoads",
        }
    }

    fn effect(self) -> &'static str {
        match self {
            Self::Set => "no load takes it",
            Self::Allocate => "the allocation stops",
        }
    }

    /// The message that refuses `factor` when it is not a finite number, a
    /// factor that scales no load to anything, `None` when it is one. `load`
    /// names the load an `AllocateLoads` factor is computed for.
    pub fn not_finite_factor(self, factor: f64, load: Option<&str>) -> Option<String> {
        if factor.is_finite() {
            return None;
        }
        let of = load.map(|l| format!(" of Load.{l}")).unwrap_or_default();
        Some(format!(
            "{}: {}{of} is not a finite number, so {}.",
            self.name(),
            crate::elements::ckt::input_text("AllocationFactor", factor),
            self.effect()
        ))
    }
}

/// Every value the recalc of a load derives, kept by a refused recalc.
#[derive(Clone, Copy)]
struct Derived {
    v_base_low: f64,
    v_base95: f64,
    v_base105: f64,
    kw_base: f64,
    kvar_base: f64,
    kva_base: f64,
    pf_nominal: f64,
    kw_ref: f64,
    kvar_ref: f64,
    shape_factor: Complex64,
    shape_is_actual: bool,
    w_nominal: f64,
    var_nominal: f64,
    yeq: Complex64,
    yeq95: Complex64,
    yeq105: Complex64,
    yeq105i: Complex64,
    i_low: Complex64,
    i95: Complex64,
    m95: Complex64,
    i_base: Complex64,
    m95i: Complex64,
    y_neut: Complex64,
    var_base: f64,
    yq_fixed: f64,
    pf_changed: bool,
}

impl Derived {
    fn of(l: &Load) -> Self {
        Self {
            v_base_low: l.v_base_low,
            v_base95: l.v_base95,
            v_base105: l.v_base105,
            kw_base: l.kw_base,
            kvar_base: l.kvar_base,
            kva_base: l.kva_base,
            pf_nominal: l.pf_nominal,
            kw_ref: l.kw_ref,
            kvar_ref: l.kvar_ref,
            shape_factor: l.shape_factor,
            shape_is_actual: l.shape_is_actual,
            w_nominal: l.w_nominal,
            var_nominal: l.var_nominal,
            yeq: l.yeq,
            yeq95: l.yeq95,
            yeq105: l.yeq105,
            yeq105i: l.yeq105i,
            i_low: l.i_low,
            i95: l.i95,
            m95: l.m95,
            i_base: l.i_base,
            m95i: l.m95i,
            y_neut: l.y_neut,
            var_base: l.var_base,
            yq_fixed: l.yq_fixed,
            pf_changed: l.pf_changed,
        }
    }

    fn restore(self, l: &mut Load) {
        l.v_base_low = self.v_base_low;
        l.v_base95 = self.v_base95;
        l.v_base105 = self.v_base105;
        l.kw_base = self.kw_base;
        l.kvar_base = self.kvar_base;
        l.kva_base = self.kva_base;
        l.pf_nominal = self.pf_nominal;
        l.kw_ref = self.kw_ref;
        l.kvar_ref = self.kvar_ref;
        l.shape_factor = self.shape_factor;
        l.shape_is_actual = self.shape_is_actual;
        l.w_nominal = self.w_nominal;
        l.var_nominal = self.var_nominal;
        l.yeq = self.yeq;
        l.yeq95 = self.yeq95;
        l.yeq105 = self.yeq105;
        l.yeq105i = self.yeq105i;
        l.i_low = self.i_low;
        l.i95 = self.i95;
        l.m95 = self.m95;
        l.i_base = self.i_base;
        l.m95i = self.m95i;
        l.y_neut = self.y_neut;
        l.var_base = self.var_base;
        l.yq_fixed = self.yq_fixed;
        l.pf_changed = self.pf_changed;
    }
}

/// `sqrt(a² + b²)`, through `hypot` where the sum of the squares overflows,
/// so a kVA base whose powers are finite is never read as infinite (pinned by
/// `exec::tests::rating_refusals::a_finite_power_whose_square_overflows_is_built`).
fn magnitude(a: f64, b: f64) -> f64 {
    let plain = (a.powi(2) + b.powi(2)).sqrt();
    if plain.is_finite() { plain } else { a.hypot(b) }
}
