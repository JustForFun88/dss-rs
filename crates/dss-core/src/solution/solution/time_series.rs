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
fn end_of_time_step_cleanup(ckt: &mut Circuit, env: &mut SolveEnv) {
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
    for r in ckt.storages.clone() {
        let node_v = &ckt.solution.node_v;
        if let Some(st) = env.store.obj_mut(r).as_any_mut().downcast_mut::<Storage>()
            && st.cd.enabled
        {
            st.update_storage(&sys, node_v, interval_hrs);
        }
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
/// times through the daily shapes. Demand-interval files are EnergyMeter
/// machinery (Phase 6).
pub(super) fn solve_daily(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0; // needed for energy meters
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
pub(super) fn solve_yearly(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0;
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
/// assumed constant for duty-cycle calcs.
pub(super) fn solve_duty(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0;
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
