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

    /// Pascal `TLoadObj.RecalcElementData`. `sys` provides the solve-state
    /// scalars `SetNominalLoad` reads; at parse time the executive passes the
    /// snapshot defaults (every value is recomputed again in `CalcYPrim`
    /// before it is consumed).
    pub fn recalc(&mut self, sys: &SysCtx) {
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
                self.kva_base = (self.kw_base.powi(2) + self.kvar_base.powi(2)).sqrt();
            }
            LoadSpec::KwKvar => {
                self.kva_base = (self.kw_base.powi(2) + self.kvar_base.powi(2)).sqrt();
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
                    self.kva_base = (self.kw_ref.powi(2) + self.kvar_ref.powi(2)).sqrt();
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

    /// Pascal `ComputeAllocatedLoad`.
    /// Pascal `Set_AllocationFactor` (Load.pas l.2131): used by
    /// `EnergyMeter.AllocateLoad` to scale a load's allocation factor. Only
    /// ConnectedkVA / kWh-spec loads change `kWbase`; fixed kW/kvar loads ignore
    /// it (via `ComputeAllocatedLoad`).
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

    /// Pascal `Set_kVAAllocationFactor` (Load.pas l.2113): the `Set
    /// AllocationFactors=X` command path. Forces the ConnectedkVA spec and
    /// re-tracks the property dump order (xfkVA/PF next; kVA/kvar/kW/kWh cleared).
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

    /// `FAllocationFactor` (read by `AllocateLoad`).
    pub fn allocation_factor(&self) -> f64 {
        self.allocation_factor
    }

    pub(super) fn compute_allocated_load(&mut self) {
        match self.load_spec_type {
            LoadSpec::ConnectedKvaPf => {
                if self.connected_kva > 0.0 {
                    self.kw_base =
                        self.connected_kva * self.allocation_factor * self.pf_nominal.abs();
                    self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                    if self.pf_nominal < 0.0 {
                        self.kvar_base = -self.kvar_base;
                    }
                }
            }
            LoadSpec::KwhPf => {
                let f_avg_kw = self.kwh / (self.kwh_days * 24.0);
                self.kw_base = f_avg_kw * self.c_factor;
                self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                if self.pf_nominal < 0.0 {
                    self.kvar_base = -self.kvar_base;
                }
            }
            _ => {}
        }
    }
}
