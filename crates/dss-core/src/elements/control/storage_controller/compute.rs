//! The StorageController control algorithm: `MakeFleetList`, the `SetFleet*`
//! helpers + fleet aggregates, the `Sample` dispatch modes (`DoLoadFollowMode` /
//! `DoTimeMode` / `DoScheduleMode` / `DoLoadShapeMode` / `DoPeakShaveModeLow`),
//! `DoPendingAction`, `Reset`, and the parse-time `RecalcElementData`. Split out
//! of `storage_controller/mod.rs` (no behavioral change).
//!
//! The fleet (`FleetPointerList`) and the monitored-element signals are reached
//! through the executive via [`StorageDispatchEnv`]; like [`GenDispatcher`] the
//! fleet resolves lazily on the first `Sample`, so the `SetFleetToExternal` +
//! `SetAllFleetValues` that Pascal runs in `RecalcElementData` are deferred to
//! that first build (see the module doc).
//!
//! [`StorageDispatchEnv`]: super::StorageDispatchEnv
//! [`GenDispatcher`]: crate::elements::control::gen_dispatcher::GenDispatcher

use num_complex::Complex64;

use crate::elements::pc::storage::{STORE_CHARGING, STORE_DISCHARGING, STORE_IDLING};
use crate::solution::SolveMode;
use crate::util::fmt_g;

use super::{
    CURRENT_PEAKSHAVE, CURRENT_PEAKSHAVE_LOW, FleetFind, MODE_FOLLOW, MODE_LOADSHAPE,
    MODE_PEAKSHAVE, MODE_PEAKSHAVELOW, MODE_SCHEDULE, MODE_SUPPORT, MODE_TIME, RELEASE_INHIBIT,
    StorageController, StorageDispatchEnv,
};

/// Pascal `EPSILON` (the idling-output guard band).
const EPSILON: f64 = 0.001;

/// `%-.6g` (the format the Pascal event-log messages use for kW/kWh values).
fn g6(v: f64) -> String {
    fmt_g(v, 6)
}

impl StorageController {
    /// Pascal `TStorageControllerObj.Get_DynamicTarget` (StorageController.pas
    /// l.1020): the seasonal kW target. `t_high` selects `SeasonTargets`
    /// (discharge, `THigh=1`) vs `SeasonTargetsLow` (charge, `THigh=0`). Callers
    /// only invoke this under the `DSS.SeasonalRating` guard (l.1099/l.1411).
    pub(super) fn get_dynamic_target(&self, env: &mut dyn StorageDispatchEnv, t_high: bool) -> f64 {
        let Some(rating_idx) = env.season_rating_idx() else {
            // `DSS.SeasonSignal` empty: Pascal's `Result` stays its `0` init —
            // NOT the non-seasonal `FkWTarget`/`FkWTargetLow` fallback.
            return 0.0;
        };
        // Pascal `(RatingIdx <= Seasons) and (Seasons > 1)`. A `RatingIdx ==
        // Seasons` (valid array length, one past the last `0..Seasons-1` slot)
        // — or a negative `RatingIdx` off a signal curve that extrapolates
        // below 0 — indexes `SeasonTargets`/`SeasonTargetsLow` out of bounds:
        // an upstream dynamic-array OOB read (UB, not a deterministic bug —
        // CLAUDE.md "UB ... not reproduced"). Rust falls back to the
        // non-seasonal target instead of reproducing the OOB read.
        if rating_idx > self.seasons || self.seasons <= 1 {
            return if t_high {
                self.f_kw_target
            } else {
                self.f_kw_target_low
            };
        }
        let arr = if t_high {
            &self.season_targets
        } else {
            &self.season_targets_low
        };
        match usize::try_from(rating_idx).ok().and_then(|i| arr.get(i)) {
            Some(&v) => v,
            None => {
                if t_high {
                    self.f_kw_target
                } else {
                    self.f_kw_target_low
                }
            }
        }
    }
}

impl StorageController {
    /// Pascal `TStorageControllerObj.RecalcElementData` (parse-time subset):
    /// validate the monitored element, attach the control's single terminal to
    /// the monitored terminal's bus, and compute the Schedule-mode ramp
    /// boundaries. The fleet build (`MakeFleetList` + `SetFleetToExternal` +
    /// `SetAllFleetValues`) needs store access, so it is deferred to the first
    /// `Sample` (see [`StorageController::ensure_fleet`]).
    pub(super) fn recalc(&mut self) {
        let Some(mon) = self.mon_snap.clone() else {
            // Pascal `DoSimpleMsg('Monitored Element in %s is not set', 372)`.
            self.ccd.cd.obj.push_error(format!(
                "Monitored Element in StorageController.{} is not set",
                self.ccd.cd.obj.name()
            ));
            return;
        };

        if self.ccd.element_terminal > mon.nterms as i32 {
            // Pascal `DoErrorMsg(... 'Terminal no. "%d" Does not exist.' 371)`.
            self.ccd.cd.obj.push_error(format!(
                "StorageController: \"{}\": Terminal no. \"{}\" Does not exist. Re-specify terminal no.",
                self.ccd.cd.obj.name(),
                self.ccd.element_terminal
            ));
        } else {
            // Pascal: FNphases := MonitoredElement.Nphases; NConds := FNphases.
            self.ccd.cd.nphases = mon.nphases;
            self.ccd.cd.set_nconds(mon.nphases);

            // Set the name of the control's 1st terminal's connected bus.
            let t = self.ccd.element_terminal;
            let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
                mon.buses[(t - 1) as usize].clone()
            } else {
                String::new() // Pascal GetBus(i) out of range yields ''
            };
            self.ccd.cd.set_bus(1, &bus);
        }

        // Pascal: UpPlusFlat := UpRampTime + FlatTime; UpPlusFlatPlusDn := ... .
        self.up_plus_flat = self.up_ramp_time + self.flat_time;
        self.up_plus_flat_plus_dn = self.up_plus_flat + self.dn_ramp_time;
    }

    /// Pascal `TStorageControllerObj.MakeFleetList`. A named list resolves each
    /// entry against the Storage class (keeping only enabled ones; a *missing*
    /// name errors 14403 and aborts the build, leaving `FleetListChanged` set); an
    /// empty name list scans every enabled, non-external Storage and allocates
    /// uniform weights. Returns whether the fleet ended up non-empty.
    pub(super) fn make_fleet_list(&mut self, env: &mut dyn StorageDispatchEnv) -> bool {
        self.fleet.clear();

        if self.element_list_specified {
            // Named list — use it.
            for i in 0..self.storage_name_list.len() {
                match env.find_storage(&self.storage_name_list[i]) {
                    FleetFind::Found(r) => self.fleet.push(r),
                    FleetFind::Disabled => {} // Pascal silently skips a disabled member
                    FleetFind::NotFound => {
                        env.push_error(format!(
                            "Error: Storage Element \"{}\" not found.",
                            self.storage_name_list[i]
                        ));
                        return false; // FleetListChanged stays true (Pascal Exits)
                    }
                }
            }
        } else {
            // Scan the whole circuit for enabled, non-external Storage.
            self.storage_name_list.clear();
            let found = env.all_fleet_storage();
            self.fleet = found.iter().map(|(_, r)| *r).collect();
            for (name, _) in &found {
                self.storage_name_list.push(name.clone());
            }
            // Allocate uniform weights.
            self.fleet_size = self.fleet.len() as i32;
            self.weights = vec![1.0; self.fleet_size.max(0) as usize];
        }

        // Add up total weights (Pascal sums FWeights[1..FleetSize]).
        let n = (self.fleet_size.max(0) as usize).min(self.weights.len());
        self.total_weight = self.weights[..n].iter().sum();

        let result = !self.fleet.is_empty();
        self.fleet_list_changed = false;
        result
    }

    /// The deferred tail of Pascal `RecalcElementData`: build the fleet, then —
    /// when it is non-empty — set every member to external dispatch and push the
    /// controller's charge/discharge/reserve rates. Runs once (gated by
    /// `FleetListChanged`) on the first `Sample`/`Reset`.
    ///
    /// The Pascal parse-time 37201 ("No unassigned Storage Elements") for a
    /// *default empty* fleet is NOT_PORTED (a silent no-op, like
    /// [`GenDispatcher`]'s empty gen list); a *named-but-missing* element still
    /// errors 14403 in `make_fleet_list`.
    ///
    /// [`GenDispatcher`]: crate::elements::control::gen_dispatcher::GenDispatcher
    fn ensure_fleet(&mut self, env: &mut dyn StorageDispatchEnv) {
        if !self.fleet_list_changed {
            return;
        }
        self.recalc_fleet(env);
    }

    /// Pascal `TStorageControllerObj.RecalcElementData` tail
    /// (StorageController.pas l.817-828): `if FleetListChanged then
    /// MakeFleetList; if FleetSize > 0 then begin SetFleetToExternal;
    /// SetAllFleetValues end`. Runs at EVERY edit of the controller (each
    /// `New`/`~`/`Edit`/`BatchEdit` line ends in `RecalcElementData`), via
    /// [`storage_controller_recalc_fleet`] — the intermediate pushes are
    /// observable: a controller defined across `~` lines first scan-builds the
    /// ALL-storage fleet and pushes its DEFAULT `%reserve`/rates onto it, and
    /// only the later `elementList=` line shrinks the fleet (SupportRun.dss
    /// pins Storage.A..E at `%Reserve = 25` from exactly that residue).
    ///
    /// [`storage_controller_recalc_fleet`]:
    ///     crate::solution::controls::dispatch::storage_controller_recalc_fleet
    pub(crate) fn recalc_fleet(&mut self, env: &mut dyn StorageDispatchEnv) {
        if self.fleet_list_changed {
            self.make_fleet_list(env);
        }
        if self.fleet_size > 0 {
            self.set_fleet_to_external(env);
            self.set_all_fleet_values(env);
        }
    }

    // --- fleet aggregates (Pascal Get_FleetkW / Get_FleetkWh / ...) ---

    fn get_fleet_kw(&self, env: &dyn StorageDispatchEnv) -> f64 {
        self.fleet.iter().map(|&r| env.present_kw(r)).sum()
    }
    fn get_fleet_kwh(&self, env: &dyn StorageDispatchEnv) -> f64 {
        self.fleet.iter().map(|&r| env.snap(r).kwh_stored).sum()
    }
    fn fleet_kwh_rating(&self, env: &dyn StorageDispatchEnv) -> f64 {
        self.fleet.iter().map(|&r| env.snap(r).kwh_rating).sum()
    }
    fn fleet_reserve_kwh(&self, env: &dyn StorageDispatchEnv) -> f64 {
        self.fleet.iter().map(|&r| env.snap(r).kwh_reserve).sum()
    }

    // --- SetFleet* helpers ---

    /// Pascal `SetAllFleetValues`.
    fn set_all_fleet_values(&self, env: &mut dyn StorageDispatchEnv) {
        for i in 0..self.fleet.len() {
            let r = self.fleet[i];
            env.set_pct_kw_in(r, self.pct_charge_rate);
            env.set_pct_kw_out(r, self.pct_kw_rate);
            env.set_pct_reserve(r, self.pct_fleet_reserve);
        }
    }
    /// Pascal `SetFleetChargeRate`.
    fn set_fleet_charge_rate(&self, env: &mut dyn StorageDispatchEnv) {
        for i in 0..self.fleet.len() {
            env.set_pct_kw_in(self.fleet[i], self.pct_charge_rate);
        }
    }
    /// Pascal `SetFleetkWRate(pctkw)`.
    fn set_fleet_kw_rate(&self, env: &mut dyn StorageDispatchEnv, pctkw: f64) {
        for i in 0..self.fleet.len() {
            env.set_pct_kw_out(self.fleet[i], pctkw);
        }
    }
    /// Pascal `SetFleetToCharge`.
    fn set_fleet_to_charge(&mut self, env: &mut dyn StorageDispatchEnv) {
        for i in 0..self.fleet.len() {
            env.set_state(self.fleet[i], STORE_CHARGING);
        }
        self.fleet_state = STORE_CHARGING;
    }
    /// Pascal `SetFleetToDisCharge`.
    fn set_fleet_to_discharge(&mut self, env: &mut dyn StorageDispatchEnv) {
        for i in 0..self.fleet.len() {
            env.set_state(self.fleet[i], STORE_DISCHARGING);
        }
        self.fleet_state = STORE_DISCHARGING;
    }
    /// Pascal `SetFleetToIdle` (`StorageState := IDLING; kW := 0`).
    fn set_fleet_to_idle(&mut self, env: &mut dyn StorageDispatchEnv) {
        for i in 0..self.fleet.len() {
            let r = self.fleet[i];
            env.set_state(r, STORE_IDLING);
            env.set_kw(r, 0.0);
        }
        self.fleet_state = STORE_IDLING;
    }
    /// Pascal `SetFleetDesiredState(state)`.
    fn set_fleet_desired_state(&self, env: &mut dyn StorageDispatchEnv, state: i32) {
        for i in 0..self.fleet.len() {
            env.set_state_desired(self.fleet[i], state);
        }
    }
    /// Pascal `SetFleetToExternal`.
    fn set_fleet_to_external(&self, env: &mut dyn StorageDispatchEnv) {
        for i in 0..self.fleet.len() {
            env.set_dispatch_external(self.fleet[i]);
        }
    }

    /// Pascal `TStorageControllerObj.Sample`.
    pub(crate) fn sample(&mut self, env: &mut dyn StorageDispatchEnv) {
        self.ensure_fleet(env); // deferred RecalcElementData fleet build

        self.charging_allowed = false;
        self.wait4step = false; // initialise for the new control step

        // Check discharge mode first; then, if charging is allowed, check charge.
        match self.discharge_mode {
            MODE_FOLLOW => {
                self.do_time_mode(env, 1);
                self.do_load_follow_mode(env);
            }
            MODE_LOADSHAPE => self.do_load_shape_mode(env),
            MODE_SUPPORT => self.do_load_follow_mode(env),
            MODE_TIME => self.do_time_mode(env, 1),
            MODE_PEAKSHAVE => self.do_load_follow_mode(env),
            CURRENT_PEAKSHAVE => self.do_load_follow_mode(env),
            MODE_SCHEDULE => self.do_schedule_mode(env),
            _ => env.push_error(format!("Invalid DisCharging Mode: {}", self.discharge_mode)),
        }

        if self.charging_allowed {
            match self.charge_mode {
                MODE_LOADSHAPE => {} // DoLoadShapeMode already executed above
                MODE_TIME => self.do_time_mode(env, 2),
                MODE_PEAKSHAVELOW => self.do_peak_shave_mode_low(env),
                CURRENT_PEAKSHAVE_LOW => self.do_peak_shave_mode_low(env),
                _ => env.push_error(format!("Invalid Charging Mode: {}", self.charge_mode)),
            }
        }
    }

    /// Pascal `TStorageControllerObj.DoPendingAction`: release the discharge
    /// inhibit (the only code it handles).
    pub(crate) fn do_pending_action(&mut self, code: i32) {
        if code == RELEASE_INHIBIT && self.discharge_mode != MODE_FOLLOW {
            self.discharge_inhibited = false;
        }
    }

    /// Pascal `TStorageControllerObj.Reset`: set the whole fleet to idle.
    pub(crate) fn reset(&mut self, env: &mut dyn StorageDispatchEnv) {
        self.ensure_fleet(env);
        self.set_fleet_to_idle(env);
    }

    /// Pascal `TStorageControllerObj.DoTimeMode(Opt)`: turn the fleet on at the
    /// trigger time (it turns itself off when full / empty / re-commanded).
    fn do_time_mode(&mut self, env: &mut dyn StorageDispatchEnv, opt: i32) {
        let total_rating_kwh = self.fleet_kwh_rating(env);
        let remaining_kwh = self.get_fleet_kwh(env);
        let reserve_kwh = self.fleet_reserve_kwh(env);

        match opt {
            1 => {
                if self.discharge_trigger_time > 0.0 {
                    // Turn on if the time is within 1/2 time step of the trigger.
                    if (env.time_of_day() - self.discharge_trigger_time).abs()
                        < env.dyna_h() / 7200.0
                    {
                        self.set_fleet_desired_state(env, STORE_DISCHARGING);
                        if self.fleet_state != STORE_DISCHARGING && remaining_kwh > reserve_kwh {
                            if self.ccd.show_event_log {
                                self.append_event(env, "Fleet Set to Discharging by Time Trigger");
                            }
                            self.set_fleet_to_discharge(env);
                            self.set_fleet_kw_rate(env, self.pct_kw_rate);
                            self.discharge_inhibited = false;
                            if self.discharge_mode == MODE_FOLLOW {
                                self.discharge_triggered_by_time = true;
                            } else {
                                env.push_immediate(STORE_DISCHARGING);
                            }
                        }
                    } else {
                        self.charging_allowed = true;
                    }
                }
            }
            2 => {
                if self.charge_trigger_time > 0.0
                    && (env.time_of_day() - self.charge_trigger_time).abs() < env.dyna_h() / 7200.0
                {
                    self.set_fleet_desired_state(env, STORE_CHARGING);
                    if self.fleet_state != STORE_CHARGING && remaining_kwh < total_rating_kwh {
                        if self.ccd.show_event_log {
                            self.append_event(env, "Fleet Set to Charging by Time Trigger");
                        }
                        self.set_fleet_to_charge(env);
                        self.discharge_inhibited = true;
                        self.out_of_oomph = false;
                        env.push_immediate(STORE_CHARGING); // force re-solve this step
                        // Push a message to release the inhibit at a later time.
                        env.push_release_inhibit(self.inhibit_hrs);
                    }
                }
            }
            _ => {}
        }
    }

    /// Pascal `TStorageControllerObj.DoScheduleMode`: ramp the fleet up from zero
    /// to `pctkWRate`, hold for the flat time, then ramp down.
    fn do_schedule_mode(&mut self, env: &mut dyn StorageDispatchEnv) {
        let mut pct_discharge_rate = 0.0;
        if self.discharge_trigger_time > 0.0 {
            if self.fleet_state != STORE_DISCHARGING {
                self.charging_allowed = true;
                let tdiff = env.time_of_day() - self.discharge_trigger_time;
                if tdiff.abs() < env.dyna_h() / 7200.0 {
                    if self.ccd.show_event_log {
                        self.append_event(env, "Fleet Set to Discharging (up ramp) by Schedule");
                    }
                    self.set_fleet_to_discharge(env);
                    self.set_fleet_desired_state(env, STORE_DISCHARGING);
                    self.charging_allowed = false;
                    pct_discharge_rate = self
                        .pct_kw_rate
                        .min((self.pct_kw_rate * tdiff / self.up_ramp_time).max(0.0));
                    self.set_fleet_kw_rate(env, pct_discharge_rate);
                    self.discharge_inhibited = false;
                    env.push_immediate(STORE_DISCHARGING);
                }
            } else {
                // Fleet is already discharging.
                let mut tdiff = env.time_of_day() - self.discharge_trigger_time;
                if tdiff < self.up_ramp_time {
                    pct_discharge_rate = self
                        .pct_kw_rate
                        .min((self.pct_kw_rate * tdiff / self.up_ramp_time).max(0.0));
                    self.set_fleet_desired_state(env, STORE_DISCHARGING);
                    if pct_discharge_rate != self.last_pct_discharge_rate {
                        self.set_fleet_kw_rate(env, pct_discharge_rate);
                        self.set_fleet_to_discharge(env);
                    }
                } else if tdiff < self.up_plus_flat {
                    pct_discharge_rate = self.pct_kw_rate;
                    self.set_fleet_desired_state(env, STORE_DISCHARGING);
                    if pct_discharge_rate != self.last_pct_discharge_rate {
                        self.set_fleet_kw_rate(env, self.pct_kw_rate); // flat part
                    }
                } else if tdiff > self.up_plus_flat_plus_dn {
                    self.set_fleet_to_idle(env);
                    self.charging_allowed = true;
                    pct_discharge_rate = 0.0;
                    if self.ccd.show_event_log {
                        self.append_event(env, "Fleet Set to Idling by Schedule");
                    }
                } else {
                    // We're on the down ramp.
                    tdiff = self.up_plus_flat_plus_dn - tdiff;
                    pct_discharge_rate = 0.0_f64
                        .max((self.pct_kw_rate * tdiff / self.dn_ramp_time).min(self.pct_kw_rate));
                    self.set_fleet_desired_state(env, STORE_DISCHARGING);
                    self.set_fleet_kw_rate(env, pct_discharge_rate);
                }

                if pct_discharge_rate != self.last_pct_discharge_rate {
                    env.push_immediate(STORE_DISCHARGING);
                }
            }
        }
        self.last_pct_discharge_rate = pct_discharge_rate; // remember this value
    }

    /// Pascal `TStorageControllerObj.DoLoadShapeMode`: dispatch by the
    /// controller's own load shape (charge below 0, idle at 0, discharge above).
    fn do_load_shape_mode(&mut self, env: &mut dyn StorageDispatchEnv) {
        let fleet_state_saved = self.fleet_state;
        let mut rate_changed = false;

        // Get the multiplier for the present solution mode.
        let hr = env.dbl_hour();
        self.load_shape_mult = match env.solve_mode() {
            SolveMode::Daily => self.daily_mult(hr),
            SolveMode::Yearly => self.yearly_mult(hr),
            SolveMode::LD2 => self.daily_mult(hr),
            SolveMode::PeakDay => self.daily_mult(hr),
            SolveMode::DutyCycle => self.duty_mult(hr),
            _ => self.load_shape_mult, // unchanged in other modes
        };

        if self.load_shape_mult.re < 0.0 {
            self.charging_allowed = true;
            let new_charge_rate = self.load_shape_mult.re.abs() * 100.0;
            self.set_fleet_desired_state(env, STORE_CHARGING);
            if new_charge_rate != self.pct_charge_rate {
                rate_changed = true;
                self.pct_charge_rate = new_charge_rate;
                self.set_fleet_charge_rate(env);
                self.set_fleet_to_charge(env);
            }
        } else if self.load_shape_mult.re == 0.0 {
            self.set_fleet_to_idle(env);
        } else {
            // Set the fleet to discharge at a rate.
            let new_kw_rate = self.load_shape_mult.re * 100.0;
            self.set_fleet_desired_state(env, STORE_DISCHARGING);
            if new_kw_rate != self.pct_kw_rate {
                rate_changed = true;
                self.pct_kw_rate = new_kw_rate;
                self.set_fleet_kw_rate(env, self.pct_kw_rate);
                self.set_fleet_to_discharge(env);
                env.set_loads_need_updating();
            }
        }

        // Force a new power flow if the fleet state changed.
        if self.fleet_state != fleet_state_saved || rate_changed {
            env.push_immediate(0);
        }
    }

    /// Pascal `CalcDailyMult`/`CalcYearlyMult`/`CalcDutyMult` — the controller's
    /// own dispatch-shape multiplier (defaults to 1+j1 `CDoubleOne` with no shape).
    fn daily_mult(&mut self, hr: f64) -> Complex64 {
        match self.daily_shape_obj.as_mut() {
            Some(s) => s.get_mult_at_hour(hr),
            None => Complex64::new(1.0, 1.0), // Pascal CDoubleOne
        }
    }
    fn yearly_mult(&mut self, hr: f64) -> Complex64 {
        match self.yearly_shape_obj.as_mut() {
            Some(s) => s.get_mult_at_hour(hr),
            None => self.daily_mult(hr),
        }
    }
    fn duty_mult(&mut self, hr: f64) -> Complex64 {
        match self.duty_shape_obj.as_mut() {
            Some(s) => s.get_mult_at_hour(hr),
            None => self.daily_mult(hr),
        }
    }

    /// Pascal `TStorageControllerObj.DoLoadFollowMode` — the discharge dispatch
    /// shared by Peakshave / Follow / Support / I-Peakshave: hold the monitored
    /// power at/below the target by discharging the fleet by its weighted share.
    fn do_load_follow_mode(&mut self, env: &mut dyn StorageDispatchEnv) {
        let mut amps_diff = 0.0;

        // Pascal re-runs `MakeFleetList` here `if FleetPointerList.Count = 0`; the
        // `ensure_fleet` at the top of `Sample` already builds (or re-attempts)
        // the fleet, so re-calling it here would only double the named-missing
        // 14403. The `FleetSize <= 0` guard (the default empty fleet) still holds.
        if self.fleet_size <= 0 {
            return;
        }

        let mut store_kw_changed = false;
        let mut skip_kw_dispatch = false;

        let fnphases = self.ccd.cd.nphases;
        let mut amps = 0.0;
        let mut s = Complex64::ZERO;
        if self.discharge_mode == CURRENT_PEAKSHAVE {
            amps = env.control_current(self.f_mon_phase, fnphases);
        } else {
            s = env.control_power(self.f_mon_phase, fnphases);
        }

        // Pascal `if DSS.SeasonalRating then CtrlTarget := Get_DynamicTarget(1)
        // else CtrlTarget := FkWTarget` (l.1099).
        let ctrl_target = if env.season_rating() {
            self.get_dynamic_target(env, true)
        } else {
            self.f_kw_target
        };

        let mut p_diff = match self.discharge_mode {
            MODE_FOLLOW => {
                if self.discharge_triggered_by_time {
                    if self.ccd.show_event_log {
                        let msg = format!(
                            "Fleet Set to Discharging by Time Trigger; Old kWTarget = {}; New = {}",
                            g6(self.f_kw_target),
                            g6(s.re * 0.001)
                        );
                        self.append_event(env, &msg);
                    }
                    self.f_kw_target = self.f_kw_threshold.max(s.re * 0.001);
                    if !self.f_kw_band_specified {
                        self.half_kw_band = self.f_pct_kw_band / 200.0 * self.f_kw_target;
                    }
                    self.discharge_triggered_by_time = false;
                    self.set_fleet_to_idle(env);
                    self.set_fleet_desired_state(env, STORE_IDLING);
                }
                s.re * 0.001 - self.f_kw_target // assume S.re is normally positive
            }
            MODE_SUPPORT => s.re * 0.001 + self.f_kw_target, // assume S.re normally negative
            MODE_PEAKSHAVE => s.re * 0.001 - ctrl_target,
            CURRENT_PEAKSHAVE => amps - ctrl_target * 1000.0, // difference in amps
            _ => 0.0,
        };

        if self.discharge_mode == CURRENT_PEAKSHAVE {
            // Convert Pdiff from amps to kW.
            let elem_volts = env.monitored_vterminal1_abs();
            self.kw_needed = env.monitored_nphases() as f64 * p_diff * elem_volts / 1000.0;
            amps_diff = p_diff;
        } else {
            self.kw_needed = p_diff;
        }

        // Check if the fleet is idling (FleetState updates only if entire fleet
        // is idling).
        if self.fleet_state != STORE_IDLING {
            let n = self.fleet.len();
            for i in 0..n {
                if env.snap(self.fleet[i]).state != STORE_IDLING {
                    break;
                }
                if i == n - 1 {
                    self.fleet_state = STORE_IDLING;
                }
            }
        }

        if self.discharge_inhibited {
            skip_kw_dispatch = true;
        } else {
            if self.fleet_state == STORE_CHARGING {
                // Ignore overload due to charging (FleetkW < 0).
                if self.discharge_mode != CURRENT_PEAKSHAVE {
                    p_diff += self.get_fleet_kw(env);
                } else {
                    let elem_volts = env.monitored_vterminal1_abs();
                    p_diff += self.get_fleet_kw(env) * 1000.0
                        / (elem_volts * env.monitored_nphases() as f64);
                }
            }

            if matches!(self.fleet_state, STORE_CHARGING | STORE_IDLING)
                && ((p_diff - self.half_kw_band < 0.0) || self.out_of_oomph)
            {
                // Don't bother trying to dispatch.
                self.charging_allowed = true;
                skip_kw_dispatch = true;
                if self.out_of_oomph {
                    // new 04/20/2020: clear OutOfOomph once the fleet recovers
                    // above ResetLevel.
                    let mut still = self.out_of_oomph;
                    for i in 0..self.fleet.len() {
                        let snap = env.snap(self.fleet[i]);
                        let kwh_actual = snap.kwh_stored / snap.kwh_rating;
                        still = still && (kwh_actual >= self.reset_level);
                    }
                    self.out_of_oomph = !still;
                }
            }
        }

        if !skip_kw_dispatch {
            let remaining_kwh = self.get_fleet_kwh(env);
            let reserve_kwh = self.fleet_reserve_kwh(env);
            if remaining_kwh > reserve_kwh {
                // Don't dispatch kW if too little storage left (endless loop).
                if p_diff.abs() > self.half_kw_band {
                    if self.fleet_state != STORE_DISCHARGING {
                        self.set_fleet_to_discharge(env);
                        // D10 (WP-U1.6, `1b3123ce`, SVN r4058): if not already
                        // discharging, force a new power flow on the first control
                        // iteration so Storage.kW updates even when this step's
                        // discharge condition matches the last one.
                        if env.control_iteration() == 1 {
                            store_kw_changed = true;
                        }
                    }
                    if self.ccd.show_event_log {
                        let msg = format!(
                            "Attempting to dispatch {} kW with {} kWh remaining and {} kWh reserve.",
                            g6(self.kw_needed),
                            g6(remaining_kwh),
                            g6(reserve_kwh)
                        );
                        self.append_event(env, &msg);
                    }
                    for i in 0..self.fleet.len() {
                        let r = self.fleet[i];
                        let snap = env.snap(r);

                        if self.discharge_mode == CURRENT_PEAKSHAVE {
                            self.kw_needed = if snap.nphases == 1 {
                                snap.present_kv * amps_diff
                            } else {
                                snap.present_kv * 3.0_f64.sqrt() * amps_diff
                            };
                        }

                        let weight = self.weights.get(i).copied().unwrap_or(1.0);
                        let dispatch_kw = snap.kw_rating.min(
                            snap.present_kw
                                + self.kw_needed * self.disp_factor * (weight / self.total_weight),
                        );

                        if dispatch_kw <= 0.0 {
                            // kWNeeded too low → just idle this element.
                            env.set_state(r, STORE_IDLING); // overrides SetFleetToDischarge
                            if (snap.present_kw.abs() - snap.kw_out_idling) > EPSILON {
                                env.set_nominal(r);
                                let actual = env.present_kw(r);
                                store_kw_changed = true;
                                if self.ccd.show_event_log {
                                    let name = env.storage_full_name(r);
                                    let msg = format!(
                                        "Requesting {name} to dispatch {} kW. Setting {name} to idling state. Final kWOut is {} kW",
                                        g6(dispatch_kw),
                                        g6(actual)
                                    );
                                    self.append_event(env, &msg);
                                }
                            }
                        } else if (snap.kw - dispatch_kw).abs() / dispatch_kw.abs() > 0.0001 {
                            // Redispatch only if a change is requested.
                            if dispatch_kw < snap.cut_in_kw_ac.max(snap.cut_out_kw_ac) {
                                if snap.inverter_on {
                                    if snap.kwh_stored > snap.kwh_reserve {
                                        env.set_kw(r, dispatch_kw);
                                        env.set_nominal(r);
                                        let actual = env.present_kw(r);
                                        store_kw_changed = true;
                                        if self.ccd.show_event_log {
                                            let name = env.storage_full_name(r);
                                            let msg = format!(
                                                "Requesting {name} to dispatch {} kW, less than CutIn/CutOut. Final kWOut is {} kW",
                                                g6(dispatch_kw),
                                                g6(actual)
                                            );
                                            self.append_event(env, &msg);
                                        }
                                    }
                                } else {
                                    // Inverter already off: override to idling and
                                    // refresh the kvar limit for InvControl.
                                    env.set_state(r, STORE_IDLING);
                                    env.set_nominal(r);
                                    let actual = env.present_kw(r);
                                    if self.ccd.show_event_log {
                                        let name = env.storage_full_name(r);
                                        let msg = format!(
                                            "Requesting {name} to dispatch {} kW, less than CutIn/CutOut. Inverter is OFF. Final kWOut is {} kW",
                                            g6(dispatch_kw),
                                            g6(actual)
                                        );
                                        self.append_event(env, &msg);
                                    }
                                }
                            } else if snap.kwh_stored > snap.kwh_reserve {
                                // Set the discharge kW; the element reverts to
                                // idling if out of capacity.
                                env.set_kw(r, dispatch_kw);
                                env.set_nominal(r);
                                let actual = env.present_kw(r);
                                store_kw_changed = true;
                                if self.ccd.show_event_log {
                                    let name = env.storage_full_name(r);
                                    let msg = format!(
                                        "Requesting {name} to dispatch {} kW. Final kWOut is {} kW",
                                        g6(dispatch_kw),
                                        g6(actual)
                                    );
                                    self.append_event(env, &msg);
                                }
                            }
                        }
                    }
                }
            } else {
                // TODO(compat): Pascal `if not FleetState = STORE_IDLING` — `not`
                // binds tighter than `=`, so this is `(not FleetState) = 0`, i.e.
                // it fires only when FleetState = STORE_CHARGING (bitwise not of
                // an integer). Reproduced verbatim; the clean fix is
                // `FleetState <> STORE_IDLING`.
                if (!self.fleet_state) == STORE_IDLING {
                    self.set_fleet_to_idle(env);
                    env.push_immediate(STORE_IDLING); // force a new power flow
                }
                self.charging_allowed = true;
                self.out_of_oomph = true;
                if self.ccd.show_event_log {
                    let msg = format!(
                        "Ran out of OOMPH: {} kWh remaining and {} reserve. Fleet has been set to idling state.",
                        g6(remaining_kwh),
                        g6(reserve_kwh)
                    );
                    self.append_event(env, &msg);
                }
            }
        }

        if store_kw_changed {
            // Only push if there has been a change (StorekvarChanged is never set).
            env.push_immediate(STORE_DISCHARGING);
        }
    }

    /// Pascal `TStorageControllerObj.DoPeakShaveModeLow` — the charging peakshave:
    /// charge the fleet to keep the monitored power above `kWTargetLow`.
    fn do_peak_shave_mode_low(&mut self, env: &mut dyn StorageDispatchEnv) {
        let mut amps_diff = 0.0;

        // The fleet is built by `ensure_fleet` (Sample top); see DoLoadFollowMode.
        if self.fleet_size <= 0 {
            return;
        }

        let mut store_kw_changed = false;
        let mut skip_kw_charge = false;

        // Pascal `if DSS.SeasonalRating then CtrlTarget := Get_DynamicTarget(0)
        // else CtrlTarget := FkWTargetLow` (l.1411).
        let ctrl_target = if env.season_rating() {
            self.get_dynamic_target(env, false)
        } else {
            self.f_kw_target_low
        };

        let fnphases = self.ccd.cd.nphases;
        let mut p_diff;
        if self.charge_mode == CURRENT_PEAKSHAVE_LOW {
            let amps = env.control_current(self.f_mon_phase, fnphases);
            p_diff = amps - ctrl_target * 1000.0;
        } else {
            let s = env.control_power(self.f_mon_phase, fnphases);
            p_diff = s.re * 0.001 - ctrl_target;
        }

        let actual_kwh = self.get_fleet_kwh(env);
        let total_rating_kwh = self.fleet_kwh_rating(env);

        // Pascal `DoPeakShaveModeLow` declares a LOCAL `kWNeeded`
        // (StorageController.pas l.1385 var block) that SHADOWS the
        // property-backed field — the `kWneed` property only ever reflects the
        // discharge path (`DoLoadFollowMode`, where the assignments hit the
        // field). Keep the charge path's value local to reproduce that: the
        // live property gate pins it (`kWNeed` after a charge sample must stay
        // the last discharge-path value).
        let mut kw_needed;
        if self.charge_mode == CURRENT_PEAKSHAVE_LOW {
            // Convert Pdiff from amps to kW.
            let elem_volts = env.monitored_vterminal1_abs();
            kw_needed = env.monitored_nphases() as f64 * p_diff * elem_volts / 1000.0;
            amps_diff = p_diff;
        } else {
            kw_needed = p_diff;
        }

        // Check if the fleet is idling.
        if self.fleet_state != STORE_IDLING {
            let n = self.fleet.len();
            for i in 0..n {
                if env.snap(self.fleet[i]).state != STORE_IDLING {
                    break;
                }
                if i == n - 1 {
                    self.fleet_state = STORE_IDLING;
                }
            }
        }

        if self.fleet_state == STORE_DISCHARGING {
            // Ignore underload due to discharging (FleetkW > 0).
            if self.charge_mode != CURRENT_PEAKSHAVE_LOW {
                p_diff += self.get_fleet_kw(env);
            } else {
                let elem_volts = env.monitored_vterminal1_abs();
                p_diff +=
                    self.get_fleet_kw(env) * 1000.0 / (elem_volts * env.monitored_nphases() as f64);
            }
        }

        if matches!(self.fleet_state, STORE_DISCHARGING | STORE_IDLING)
            && ((p_diff > 0.0) || (actual_kwh >= total_rating_kwh) || self.wait4step)
        {
            // Don't bother trying to charge.
            self.charging_allowed = false;
            skip_kw_charge = true;
            self.wait4step = false;
        }

        if skip_kw_charge {
            return;
        }

        if actual_kwh < total_rating_kwh {
            // Don't dispatch kW if fully charged (endless loop).
            if p_diff.abs() > self.half_kw_band_low {
                if self.fleet_state != STORE_CHARGING {
                    self.set_fleet_to_charge(env);
                    // D10 (WP-U1.6, `1b3123ce`, SVN r4058): if not already
                    // charging, force a new power flow on the first control
                    // iteration (peakshavelow charge).
                    if env.control_iteration() == 1 {
                        store_kw_changed = true;
                    }
                }
                if self.ccd.show_event_log {
                    let msg = format!(
                        "Attempting to charge {} kW with {} kWh remaining and {} rating.",
                        g6(kw_needed),
                        g6(total_rating_kwh - actual_kwh),
                        g6(total_rating_kwh)
                    );
                    self.append_event(env, &msg);
                }
                for i in 0..self.fleet.len() {
                    let r = self.fleet[i];
                    let snap = env.snap(r);

                    if self.charge_mode == CURRENT_PEAKSHAVE_LOW {
                        kw_needed = if snap.nphases == 1 {
                            snap.present_kv * amps_diff
                        } else {
                            snap.present_kv * 3.0_f64.sqrt() * amps_diff
                        };
                    }

                    let weight = self.weights.get(i).copied().unwrap_or(1.0);
                    // May be positive or negative.
                    let mut charge_kw = snap.present_kw
                        + kw_needed * (weight / self.total_weight) * self.disp_factor;
                    if charge_kw < 0.0 {
                        charge_kw = (-snap.kw_rating).max(charge_kw); // vs kVA rating
                    }

                    if charge_kw >= 0.0 {
                        // chargeKW positive if the demand increase is too high.
                        env.set_state(r, STORE_IDLING); // overrides SetFleetToCharge
                        if (snap.present_kw.abs() - snap.kw_out_idling) > EPSILON {
                            env.set_nominal(r);
                            let actual = env.present_kw(r);
                            store_kw_changed = true;
                            if self.ccd.show_event_log {
                                let name = env.storage_full_name(r);
                                let msg = format!(
                                    "Requesting {name} to dispatch {} kW. Setting {name} to idling state. Final kWOut is {} kW",
                                    g6(charge_kw),
                                    g6(actual)
                                );
                                self.append_event(env, &msg);
                            }
                        }
                    } else if (snap.kw - charge_kw).abs() / charge_kw.abs() > 0.0001 {
                        // Do only if a change is requested.
                        if charge_kw.abs() < snap.cut_in_kw_ac.max(snap.cut_out_kw_ac) {
                            if snap.inverter_on {
                                if snap.kwh_stored > snap.kwh_reserve {
                                    env.set_kw(r, charge_kw);
                                    env.set_nominal(r);
                                    let actual = env.present_kw(r);
                                    store_kw_changed = true;
                                    if self.ccd.show_event_log {
                                        let name = env.storage_full_name(r);
                                        let msg = format!(
                                            "Requesting {name} to dispatch {} kW, less than CutIn/CutOut. Final kWOut is {} kW",
                                            g6(charge_kw),
                                            g6(actual)
                                        );
                                        self.append_event(env, &msg);
                                    }
                                }
                            } else {
                                env.set_state(r, STORE_IDLING); // overrides SetFleetToCharge
                                env.set_nominal(r);
                                let actual = env.present_kw(r);
                                if self.ccd.show_event_log {
                                    let name = env.storage_full_name(r);
                                    let msg = format!(
                                        "Requesting {name} to dispatch {} kW, less than CutIn/CutOut. Inverter is OFF. Final kWOut is {} kW",
                                        g6(charge_kw),
                                        g6(actual)
                                    );
                                    self.append_event(env, &msg);
                                }
                            }
                        } else if snap.kwh_stored < snap.kwh_rating {
                            env.set_kw(r, charge_kw);
                            env.set_nominal(r);
                            let actual = env.present_kw(r);
                            store_kw_changed = true;
                            if self.ccd.show_event_log {
                                let name = env.storage_full_name(r);
                                let msg = format!(
                                    "Requesting {name} to dispatch {} kW. Final kWOut is {} kW",
                                    g6(charge_kw),
                                    g6(actual)
                                );
                                self.append_event(env, &msg);
                            }
                        }
                    }
                }
            }
        } else {
            // TODO(compat): Pascal `if not FleetState = STORE_IDLING` — see
            // DoLoadFollowMode; fires only when FleetState = STORE_CHARGING.
            if (!self.fleet_state) == STORE_IDLING {
                self.set_fleet_to_idle(env);
                env.push_immediate(STORE_IDLING); // force a new power flow
            }
            self.charging_allowed = false;
            if self.ccd.show_event_log {
                let msg = format!(
                    "Fully charged: {} kWh of rated {}.",
                    g6(actual_kwh),
                    g6(total_rating_kwh)
                );
                self.append_event(env, &msg);
            }
        }

        if store_kw_changed {
            env.push_immediate(STORE_CHARGING);
        }
    }

    /// Pascal `AppendToEventLog(Self.FullName, msg)` — the env formats the
    /// "Element=StorageController.<name>" prefix from the control's `self_ref`.
    fn append_event(&self, env: &mut dyn StorageDispatchEnv, msg: &str) {
        env.append_event(msg);
    }
}
