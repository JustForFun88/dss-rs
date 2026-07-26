//! Pascal `DoControlActions` (l.1941): the per-control-mode queue dispatch and
//! the `ControlActioner` callback that routes popped records back into
//! `dispatch_control`.

use crate::circuit::Circuit;
use crate::elements::traits::ElemId;
use crate::solution::control_queue::{ControlActioner, ControlQueue};
use crate::solution::solution::{ControlMode, SolveEnv, SolveResult};

use super::ControlOp;
use super::dispatch::dispatch_control;
use super::multi_rate::do_multi_rate;

/// Pascal `DoControlActions` (l.1941): per-control-mode queue dispatch.
pub(crate) fn do_control_actions(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    match ckt.solution.control_mode {
        ControlMode::Static => {
            // Execute the nearest set of control actions but leave time as is.
            if ckt.solution.control_queue.is_empty() {
                ckt.solution.control_actions_done = true;
            } else {
                let (mut xhour, mut xsec) = (0_i32, 0.0_f64);
                run_nearest_actions(ckt, env, &mut xhour, &mut xsec)?; // ignore time advancement
            }
        }
        ControlMode::EventDriven => {
            // Execute the nearest set of actions and advance time to that time.
            let (mut hour, mut sec) = (ckt.solution.int_hour, ckt.solution.t);
            let any = run_nearest_actions(ckt, env, &mut hour, &mut sec)?;
            // The Pascal arguments are `var DynaVars.intHour / .t`.
            ckt.solution.int_hour = hour;
            ckt.solution.t = sec;
            if !any {
                ckt.solution.control_actions_done = true;
            }
        }
        ControlMode::TimeDriven => {
            // Do all actions having an action time <= the specified time.
            let (hour, sec) = (ckt.solution.int_hour, ckt.solution.t);
            if !run_actions(ckt, env, hour, sec)? {
                ckt.solution.control_actions_done = true;
            }
        }
        ControlMode::MultiRate => {
            let mut queue = std::mem::take(&mut ckt.solution.control_queue);
            let result = do_multi_rate(ckt, env, &mut queue);
            ckt.solution.control_queue = queue;
            if !result? {
                ckt.solution.control_actions_done = true;
            }
        }
        ControlMode::ControlsOff => {}
    }
    Ok(())
}

/// `ControlQueue.DoNearestActions` over the live circuit.
fn run_nearest_actions(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    hour: &mut i32,
    sec: &mut f64,
) -> Result<bool, String> {
    let mut queue = std::mem::take(&mut ckt.solution.control_queue);
    let mut act = Actioner {
        ckt,
        env,
        failure: None,
    };
    let any = queue.do_nearest_actions(hour, sec, &mut act);
    let failure = act.failure.take();
    ckt.solution.control_queue = queue;
    match failure {
        Some(e) => Err(e),
        None => Ok(any),
    }
}

/// `ControlQueue.DoActions` over the live circuit.
fn run_actions(ckt: &mut Circuit, env: &mut SolveEnv, hour: i32, sec: f64) -> Result<bool, String> {
    let mut queue = std::mem::take(&mut ckt.solution.control_queue);
    let mut act = Actioner {
        ckt,
        env,
        failure: None,
    };
    let any = queue.do_actions(hour, sec, &mut act);
    let failure = act.failure.take();
    ckt.solution.control_queue = queue;
    match failure {
        Some(e) => Err(e),
        None => Ok(any),
    }
}

/// The [`ControlActioner`] the queue dispatchers call back into: routes each
/// popped record to `dispatch_control`, capturing the first hard error (the
/// trait method returns `()`; Pascal propagates by exception).
struct Actioner<'a, 'b> {
    ckt: &'a mut Circuit,
    env: &'a mut SolveEnv<'b>,
    failure: Option<String>,
}

impl ControlActioner for Actioner<'_, '_> {
    fn do_pending_action(
        &mut self,
        control: ElemId,
        code: i32,
        proxy: i32,
        queue: &mut ControlQueue,
    ) {
        if self.failure.is_some() {
            return;
        }
        if let Err(e) = dispatch_control(
            control,
            ControlOp::Action { code, proxy },
            self.ckt,
            self.env,
            queue,
        ) {
            self.failure = Some(e);
        }
    }
}
