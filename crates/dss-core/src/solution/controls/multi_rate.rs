//! Pascal `ControlQueue.DoMultiRate` (l.359) and its `Recalc_Time_Step` /
//! `Restore_Time_Step` helpers — the multi-rate control sweep that re-solves and
//! re-samples the circuit window-by-window between control actions.

use crate::circuit::Circuit;
use crate::solution::control_queue::{ControlQueue, TimeRec};
use crate::solution::solution::{SolveEnv, solve_circuit};

use super::ControlOp;
use super::dispatch::dispatch_control;
use super::sampling::sample_control_devices_q;

/// Pascal `ControlQueue.DoMultiRate` (l.359), ported verbatim including the
/// scratch-register choreography (`Temp_Int`/`Temp_dbl` index comments match
/// the Pascal). Runs all due actions, then sweeps forward window-by-window,
/// re-solving the circuit and re-sampling the controls between windows.
pub(super) fn do_multi_rate(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    queue: &mut ControlQueue,
) -> Result<bool, String> {
    let mut result = false;
    let mut temp_int = [0_i32; 3]; // [0] hour register, [1] exit flag, [2] saved intHour
    let mut temp_dbl = [0.0_f64; 8];
    // Temp_dbl[1] time accumulator, [2] time upper boundary, [3] control action
    // time, [4] step size h, [5] recalculated occurrence time, [6] sim step
    // incremental, [7] saved t.
    if queue.is_empty() {
        return Ok(false);
    }

    let mut ltimer = TimeRec {
        hour: ckt.solution.int_hour,
        sec: ckt.solution.t,
    };
    temp_dbl[4] = ckt.solution.h; // Simulation step time (time window size)
    temp_dbl[6] = ltimer.to_time(); // Simulation step time incremental

    while let Some((p, atime)) = queue.pop_time(ltimer, false) {
        temp_dbl[3] = atime;
        dispatch_control(
            p.control,
            ControlOp::Action {
                code: p.code,
                proxy: p.proxy,
            },
            ckt,
            env,
            queue,
        )?;
        result = true;
    }

    // After this point, the additional control actions are performed.
    temp_dbl[7] = ckt.solution.t; // Saving the current time (secs)
    temp_int[2] = ckt.solution.int_hour; // Saving the current time (hour)
    temp_dbl[2] = temp_dbl[6];
    // Simulation time is recalculated considering the next control action event.
    recalc_time_step(ckt, &mut ltimer, &mut temp_int, &mut temp_dbl);
    // Downloads the next action without removing it from the queue.
    let mut popped = queue.pop_time(ltimer, true);
    while let Some((p, atime)) = popped {
        temp_dbl[3] = atime;
        while temp_dbl[3] >= 3600.0 {
            temp_dbl[3] -= 3600.0; // action time is adjusted
        }
        // Recalculates the action occurrence time.
        temp_dbl[5] = (temp_dbl[3] - temp_dbl[6]) + temp_dbl[1];
        if temp_dbl[5] < temp_dbl[4] {
            // The action is within the time window: remove it and run it.
            let (p2, atime2) = queue
                .pop_time(ltimer, false)
                .expect("record peeked above is still queued");
            temp_dbl[3] = atime2;
            dispatch_control(
                p2.control,
                ControlOp::Action {
                    code: p2.code,
                    proxy: p2.proxy,
                },
                ckt,
                env,
                queue,
            )?;
            popped = queue.pop_time(ltimer, true);
        } else {
            // The next action is outside the time window: run it and exit.
            dispatch_control(
                p.control,
                ControlOp::Action {
                    code: p.code,
                    proxy: p.proxy,
                },
                ckt,
                env,
                queue,
            )?;
            popped = None;
            temp_int[1] = 1; // Preparing everything to exit
        }
        if popped.is_none() && temp_int[1] == 0 {
            // The last action was within the time window: keep scanning.
            temp_dbl[1] += temp_dbl[3] - temp_dbl[6]; // accumulated time
            temp_dbl[6] += temp_dbl[4]; // time reference moves forward
            while temp_dbl[6] >= 3600.0 {
                temp_dbl[6] -= 3600.0; // time reference is adjusted
            }
            // Updates the circuit after applying the control actions.
            solve_circuit(ckt, env)?;
            restore_time_step(ckt, &temp_int, &temp_dbl); // restore time for sampling devices
            sample_control_devices_q(ckt, env, queue)?;
            recalc_time_step(ckt, &mut ltimer, &mut temp_int, &mut temp_dbl);
            popped = queue.pop_time(ltimer, true);
        }
    }
    restore_time_step(ckt, &temp_int, &temp_dbl); // restore time to keep going
    Ok(result)
}

/// Pascal `TControlQueue.Recalc_Time_Step`.
fn recalc_time_step(
    ckt: &mut Circuit,
    ltimer: &mut TimeRec,
    temp_int: &mut [i32; 3],
    temp_dbl: &mut [f64; 8],
) {
    temp_dbl[2] += temp_dbl[4]; // time window moves forward
    while temp_dbl[2] >= 3600.0 {
        temp_int[0] += 1;
        temp_dbl[2] -= 3600.0;
    }
    ltimer.hour = temp_int[0];
    ltimer.sec = temp_dbl[2];
    ckt.solution.int_hour = temp_int[0]; // sets the simulation time
    ckt.solution.t = temp_dbl[2];
    ckt.solution.update_dbl_hour();
}

/// Pascal `TControlQueue.Restore_Time_Step`.
fn restore_time_step(ckt: &mut Circuit, temp_int: &[i32; 3], temp_dbl: &[f64; 8]) {
    ckt.solution.int_hour = temp_int[2];
    ckt.solution.t = temp_dbl[7];
    ckt.solution.update_dbl_hour();
}
