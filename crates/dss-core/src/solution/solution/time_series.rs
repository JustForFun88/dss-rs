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
pub(crate) fn end_of_time_step_cleanup(ckt: &mut Circuit, env: &mut SolveEnv) {
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
pub(crate) fn sample_all_monitors_and_meters(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    sample_meters: bool,
) {
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
    // Pascal `finally`: `MonitorClass.SaveAll(); if SampleTheMeters then
    // CloseAllDIFiles` — both run unconditionally, even on an aborted step.
    crate::solution::monitors::save_all_monitors(ckt, env);
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
/// `CloseDI` closes them). The `finally`'s lone action, `MonitorClass.
/// SaveAll()`, still runs unconditionally (even on an early `Err`/aborted
/// step).
pub(super) fn solve_yearly(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0;
    if !ckt.em_di.di_files_are_open {
        crate::solution::meters::open_all_di_files(ckt, env.store);
    }
    let result = solve_yearly_body(ckt, env);
    crate::solution::monitors::save_all_monitors(ckt, env);
    result
}

/// The `SolveYearly` stepping loop (the Pascal `try` body).
fn solve_yearly_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
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
    // Pascal `finally`: `MonitorClass.SaveAll(); if SampleTheMeters then
    // CloseAllDIFiles` — both run unconditionally, even on an aborted step.
    crate::solution::monitors::save_all_monitors(ckt, env);
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

/// Pascal `_()` error text shared by `SolveLD1`/`SolveLD2` when
/// `ckt.LoadDurCurveObj = NIL` (`SolutionAlgs.pas` l.567/652, errors #470/
/// #471 — the numeric code is not modeled here, matching this crate's other
/// `DoSimpleMsg` ports). Verbatim, including the mid-word capital in
/// "perForm".
const LDCURVE_NOT_DEFINED: &str =
    "Load Duration Curve Not Defined (Set LDCurve=... command). Cannot perForm solution.";

/// Pascal `SolveLD1` (`SolutionAlgs.pas` l.557): a daily outer loop
/// (`NDaily = Round(24.0 / DynaVars.h * 3600.0)`) times a load-duration-curve
/// inner loop (`ckt.LoadDurCurveObj.NumPoints` points), each point a `SolveSnap`.
/// The `LoadDurCurveObj = NIL` guard sits INSIDE the Pascal `try`, so even that
/// early exit still runs the `finally` (`MonitorClass.SaveAll` — flushes the
/// monitor sample buffers to their streams — plus, if `SampleTheMeters`,
/// `CloseAllDIFiles`); reproduced by always running the `finally` tail after
/// [`solve_ld1_body`].
pub(super) fn solve_ld1(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let result = solve_ld1_body(ckt, env);
    // Pascal `finally`: `MonitorClass.SaveAll(); if SampleTheMeters then
    // CloseAllDIFiles` — both run unconditionally, even on an aborted step.
    crate::solution::monitors::save_all_monitors(ckt, env);
    if ckt.solution.sample_the_meters {
        crate::solution::meters::close_all_di_files(ckt, env.store, env.errors);
    }
    result
}

/// The `SolveLD1` stepping loop (the Pascal `try` body).
fn solve_ld1_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.load_dur_curve_obj.is_none() {
        env.errors.push(LDCURVE_NOT_DEFINED.to_string());
        return Ok(());
    }
    // Time must be set before entering this routine.
    // TODO(compat): FPC `Round` is banker's rounding (ties-to-even); this
    // index is always in i32 range, so `round_ties_even` reproduces it.
    // Wiped with the other compat shims.
    let ndaily = (24.0 / ckt.solution.h * 3600.0).round_ties_even() as i32;
    if !ckt.em_di.di_files_are_open {
        crate::solution::meters::open_all_di_files(ckt, env.store);
    }
    ckt.solution.int_hour = 0;
    for _ in 1..=ndaily {
        ckt.solution.increment_time();
        let dbl_hour = ckt.solution.dbl_hour;
        match ckt.default_daily_shape_obj.as_mut() {
            Some(shape) => ckt.default_hour_mult = shape.get_mult_at_hour(dbl_hour),
            None => return Err("Default daily load shape not found.".to_string()),
        }
        if ckt.solution.solution_abort {
            // Pascal `Break` (not the per-step `continue` Daily/Yearly/Duty
            // use): once aborted, LD1 never re-enters the outer loop.
            env.errors.push("Solution Aborted".to_string());
            break;
        }
        let num_points = ckt
            .load_dur_curve_obj
            .as_ref()
            .map(|c| c.num_points())
            .unwrap_or(0);
        for n in 1..=num_points {
            if let Some(curve) = ckt.load_dur_curve_obj.as_mut() {
                ckt.load_multiplier = curve.mult(n); // set WITH prop: matrix may rebuild
                ckt.solution.interval_hrs = curve.present_interval();
            }
            if let Some(price) = ckt.price_curve_obj.as_mut() {
                ckt.price_signal = price.price(n);
            }
            solve_snap(ckt, env)?;
            let sample_meters = ckt.solution.sample_the_meters;
            sample_all_monitors_and_meters(ckt, env, sample_meters);
            end_of_time_step_cleanup(ckt, env);
        }
    }
    Ok(())
}

/// Pascal `SolveLD2` (`SolutionAlgs.pas` l.642): time held fixed, sweep the
/// load-duration curve once. Unlike `SolveLD1` the `LoadDurCurveObj = NIL`
/// guard sits OUTSIDE the Pascal `try` — that early exit skips
/// `DefaultHourMult`, `OpenAllDIFiles` and the `finally` tail entirely (no DI
/// files were opened yet either), reproduced by returning before any of that
/// runs. The mid-loop abort re-check `SolveLD1` has per outer step does not
/// exist here: Pascal tests `SolutionAbort` exactly once, before the
/// curve-point loop, never inside it — ported verbatim (a control-flow fact,
/// not a numeric approximation, so no `TODO(compat)`).
pub(super) fn solve_ld2(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.load_dur_curve_obj.is_none() {
        env.errors.push(LDCURVE_NOT_DEFINED.to_string());
        return Ok(());
    }
    // Time must be set before entering this routine.
    let dbl_hour = ckt.solution.dbl_hour;
    match ckt.default_daily_shape_obj.as_mut() {
        Some(shape) => ckt.default_hour_mult = shape.get_mult_at_hour(dbl_hour),
        None => return Err("Default daily load shape not found.".to_string()),
    }
    if !ckt.em_di.di_files_are_open {
        crate::solution::meters::open_all_di_files(ckt, env.store);
    }
    let result = solve_ld2_body(ckt, env);
    // Pascal `finally`: `MonitorClass.SaveAll(); if SampleTheMeters then
    // CloseAllDIFiles` — both run unconditionally, even on an aborted step.
    crate::solution::monitors::save_all_monitors(ckt, env);
    if ckt.solution.sample_the_meters {
        crate::solution::meters::close_all_di_files(ckt, env.store, env.errors);
    }
    result
}

/// The `SolveLD2` stepping loop (the Pascal `try` body).
fn solve_ld2_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.solution.solution_abort {
        // Note the trailing period here — Pascal's LD2 abort text differs
        // literally from LD1's ("Solution Aborted." vs "Solution Aborted").
        env.errors.push("Solution Aborted.".to_string());
        return Ok(());
    }
    let num_points = ckt
        .load_dur_curve_obj
        .as_ref()
        .map(|c| c.num_points())
        .unwrap_or(0);
    for n in 1..=num_points {
        if let Some(curve) = ckt.load_dur_curve_obj.as_mut() {
            ckt.load_multiplier = curve.mult(n); // set WITH prop: matrix may rebuild
            ckt.solution.interval_hrs = curve.present_interval();
        }
        if let Some(price) = ckt.price_curve_obj.as_mut() {
            ckt.price_signal = price.price(n);
        }
        solve_snap(ckt, env)?;
        let sample_meters = ckt.solution.sample_the_meters;
        sample_all_monitors_and_meters(ckt, env, sample_meters);
        end_of_time_step_cleanup(ckt, env);
    }
    Ok(())
}

/// Pascal `SolveGeneralTime` (`SolutionAlgs.pas` l.298, "For Rolling your own
/// solution modes"): step `number_of_times` times, computing `DefaultHourMult`
/// from the CURRENT clock (time starts at the `Set_Mode`-reset zero), solving,
/// then calling `FinishTimeStep`. Unlike `SolveDaily`/`SolveYearly`/
/// `SolveDuty` — which call `IncrementTime` at the TOP of the loop — GeneralTime
/// increments at the END, inside `FinishTimeStep`. It also does **not** touch
/// `PriceCurveObj`/`PriceSignal`, does **not** open/close the demand-interval
/// files, and does **not** call `MonitorClass.SaveAll` when done: ported 1:1
/// as the deliberately bare-bones "custom solution" loop, not enriched to
/// match the other time-series modes.
pub(super) fn solve_general_time(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0; // needed for energy meters and storage devices
    for _ in 1..=ckt.solution.number_of_times {
        if ckt.solution.solution_abort {
            continue;
        }
        // Compute basic multiplier from Default loadshape to use in generator
        // dispatch, if any.
        let dbl_hour = ckt.solution.dbl_hour;
        match ckt.default_daily_shape_obj.as_mut() {
            Some(shape) => ckt.default_hour_mult = shape.get_mult_at_hour(dbl_hour),
            None => return Err("Default daily load shape not found.".to_string()),
        }
        solve_snap(ckt, env)?;

        finish_time_step(ckt, env);
    }
    Ok(())
}

/// Pascal `FinishTimeStep` (`SolutionAlgs.pas` l.74, "Sample Cleanup and
/// increment time — For custom solutions"): sample all monitors
/// unconditionally, sample the EnergyMeters only if `SampleTheMeters`, run
/// `EndOfTimeStepCleanup`, THEN increment time — the mirror image of the
/// daily/yearly/duty loops, which increment first.
fn finish_time_step(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sample_meters = ckt.solution.sample_the_meters;
    sample_all_monitors_and_meters(ckt, env, sample_meters);
    end_of_time_step_cleanup(ckt, env);
    ckt.solution.increment_time();
}
