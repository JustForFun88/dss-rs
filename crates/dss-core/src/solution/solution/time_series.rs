//! The `SolutionAlgs.pas` time-series stepping modes (`SolveDaily`/`SolveYearly`/
//! `SolveDuty`/`SolvePeakDay`) plus their per-step monitor/meter sampling and
//! `EndOfTimeStepCleanup`.

use crate::circuit::Circuit;
use crate::elements::pc::storage::Storage;

use super::power_flow::solve_snap;
use super::{SolveEnv, SolveResult, sys_ctx};

/// Pascal `EndOfTimeStepCleanup` (`SolutionAlgs.pas` l.86): the Storage SOC
/// update (`StorageClass.UpdateAll`), then the InvControl rolling-average feed
/// (`InvControlClass.UpdateAll`, l.91) and the ExpControl `Vreg` slew
/// (`ExpControlClass.UpdateAll`, l.92), plus the mode-5 monitor sampling
/// (`MonitorClass.SampleAllMode5`, l.96 — captures the per-step timings).
pub(super) fn end_of_time_step_cleanup(ckt: &mut Circuit, env: &mut SolveEnv) {
    update_all_storage(ckt, env);
    crate::solution::controls::update_all_inv_controls(ckt, env);
    crate::solution::controls::update_all_exp_controls(ckt, env);
    crate::solution::monitors::sample_all_monitors(ckt, env, true);
}

/// Pascal `TStorage.UpdateAll` (Storage.pas l.761): advance every enabled
/// Storage element's state of charge by one time-step interval.
fn update_all_storage(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let interval_hrs = ckt.solution.interval_hrs;
    let mut y_changed = false;
    for r in ckt.storages.clone() {
        let node_v = &ckt.solution.node_v;
        if let Some(st) = env.store.obj_mut(r).as_any_mut().downcast_mut::<Storage>()
            && st.cd.enabled
        {
            st.update_storage(&sys, node_v, interval_hrs);
            // Pascal `UpdateStorage` ends with `if StateChanged then
            // YprimInvalid := TRUE` (Storage.pas:2559) and `Set_YprimInvalid`
            // ALSO raises `Solution.SystemYChanged` (CktElement.pas:245), so
            // the NEXT step rebuilds Y *before* its first injection pass — the
            // first iteration then injects through the fresh (e.g. idle, after
            // a full-charge flip) YPrim, not the stale previous-state one.
            // Without this the first post-flip step starts one Yeq-sized jump
            // away and takes an extra iteration (caught by the controls live
            // gate, storagectrl_peakshave step 6).
            y_changed |= st.cd.yprim_invalid;
        }
    }
    if y_changed {
        ckt.solution.system_y_changed = true;
    }
}

/// Pascal `MonitorClass.SampleAll` + (if `sample_meters`)
/// `EnergyMeterClass.SampleAll` (`SolutionAlgs.pas` l.78): the monitor sweep
/// (mode ≠ 5) always runs; the EnergyMeter register sweep runs when the solve
/// mode requests it.
fn sample_all_monitors_and_meters(ckt: &mut Circuit, env: &mut SolveEnv, sample_meters: bool) {
    crate::solution::monitors::sample_all_monitors(ckt, env, false);
    if sample_meters {
        let sys = sys_ctx(ckt);
        crate::solution::meters::take_sample_all(ckt, env.store, &sys);
    }
}

/// Pascal `SolveDaily` (`SolutionAlgs.pas` l.160): step `number_of_times`
/// times through the daily shapes. Opens the demand-interval files first (a
/// no-op unless `Set DemandInterval=yes`); the `finally` closes them (writing
/// the `DI_*` CSVs) when the mode samples the meters — even on an aborted
/// step, exactly like the Pascal `try…finally`.
pub(super) fn solve_daily(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0; // needed for energy meters
    if !ckt.em_di.di_files_are_open {
        crate::solution::meters::open_all_di_files(ckt, env.store);
    }
    let result = solve_daily_body(ckt, env);
    // Pascal `finally`: `if SampleTheMeters then CloseAllDIFiles`.
    if ckt.solution.sample_the_meters {
        crate::solution::meters::close_all_di_files(ckt, env.store, env.errors);
    }
    result
}

/// The `SolveDaily` stepping loop (the Pascal `try` body).
fn solve_daily_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    for _ in 1..=ckt.solution.number_of_times {
        if ckt.solution.solution_abort {
            continue;
        }
        ckt.solution.increment_time();
        let dbl_hour = ckt.solution.dbl_hour;
        match ckt.default_daily_shape_obj.as_mut() {
            Some(shape) => ckt.default_hour_mult = shape.get_mult_at_hour(dbl_hour),
            None => return Err("Default daily load shape not found.".to_string()),
        }
        if let Some(curve) = ckt.price_curve_obj.as_mut() {
            ckt.price_signal = curve.get_price(dbl_hour);
        }
        solve_snap(ckt, env)?;
        let sample_meters = ckt.solution.sample_the_meters;
        sample_all_monitors_and_meters(ckt, env, sample_meters);
        end_of_time_step_cleanup(ckt, env);
    }
    Ok(())
}

/// Pascal `SolvePeakDay` (l.204): like daily, but restarts the clock at zero
/// (the global load multiplier is ignored by virtue of the PEAKDAY mode
/// dispatch inside `SetNominalLoad`).
pub(super) fn solve_peak_day(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.t = 0.0;
    ckt.solution.int_hour = 0;
    ckt.solution.dbl_hour = 0.0;
    solve_daily(ckt, env)
}

/// Pascal `SolveYearly` (l.112): like daily over the yearly default shape
/// (loads additionally apply `DefaultGrowthFactor` via `SetNominalLoad`).
/// Opens the demand-interval files but — unlike daily/duty — does **not**
/// close them at the end (Pascal's close is commented out, "See
/// DIFilesAreOpen Logic": yearly runs accumulate until a `Reset`/`Set year=`/
/// `CloseDI` closes them).
pub(super) fn solve_yearly(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0;
    if !ckt.em_di.di_files_are_open {
        crate::solution::meters::open_all_di_files(ckt, env.store);
    }
    for _ in 1..=ckt.solution.number_of_times {
        if ckt.solution.solution_abort {
            continue;
        }
        ckt.solution.increment_time();
        let dbl_hour = ckt.solution.dbl_hour;
        match ckt.default_yearly_shape_obj.as_mut() {
            Some(shape) => ckt.default_hour_mult = shape.get_mult_at_hour(dbl_hour),
            None => return Err("Default yearly load shape not found.".to_string()),
        }
        if let Some(curve) = ckt.price_curve_obj.as_mut() {
            ckt.price_signal = curve.get_price(dbl_hour);
        }
        solve_snap(ckt, env)?;
        let sample_meters = ckt.solution.sample_the_meters;
        sample_all_monitors_and_meters(ckt, env, sample_meters);
        end_of_time_step_cleanup(ckt, env);
    }
    Ok(())
}

/// Pascal `SolveDuty` (l.249): same loop as daily; the price signal is
/// assumed constant for duty-cycle calcs. Note duty does **not** open the
/// demand-interval files (no `OpenAllDIFiles` in its body — unlike daily/
/// yearly/peak-day); the `finally` still closes any already-open set when the
/// mode samples the meters.
pub(super) fn solve_duty(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0;
    let result = solve_duty_body(ckt, env);
    if ckt.solution.sample_the_meters {
        crate::solution::meters::close_all_di_files(ckt, env.store, env.errors);
    }
    result
}

/// The `SolveDuty` stepping loop (the Pascal `try` body).
fn solve_duty_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    for _ in 1..=ckt.solution.number_of_times {
        if ckt.solution.solution_abort {
            continue;
        }
        ckt.solution.increment_time();
        let dbl_hour = ckt.solution.dbl_hour;
        match ckt.default_daily_shape_obj.as_mut() {
            Some(shape) => ckt.default_hour_mult = shape.get_mult_at_hour(dbl_hour),
            None => return Err("Default daily load shape not found.".to_string()),
        }
        solve_snap(ckt, env)?;
        let sample_meters = ckt.solution.sample_the_meters;
        sample_all_monitors_and_meters(ckt, env, sample_meters);
        end_of_time_step_cleanup(ckt, env);
    }
    Ok(())
}
