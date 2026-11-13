//! Pascal `Sample_DoControlActions` / `SampleControlDevices` (l.1974) and
//! `Utilities.DoResetControls`: the per-sweep entry points that iterate the
//! circuit's enabled controls.

// BUG(C2): the sweeps of this module are free functions over the whole circuit; they belong to
// the owner of the circuit and the element store, calling one method of each control.
use crate::circuit::Circuit;
use crate::solution::control_queue::ControlQueue;
use crate::solution::solution::{ControlMode, SolveEnv, SolveResult};

use super::ControlOp;
use super::actions::do_control_actions;
use super::dispatch::dispatch_control;

/// Pascal `Sample_DoControlActions` (`Solution.pas` l.1996).
pub(crate) fn sample_do_control_actions(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.solution.control_mode == ControlMode::ControlsOff {
        ckt.solution.control_actions_done = true;
    } else {
        sample_control_devices(ckt, env)?;
        do_control_actions(ckt, env)?;

        // This variable lets control devices know the bus list has changed.
        ckt.control_bus_name_redefined = false; // Reset until next change
    }
    Ok(())
}

/// Pascal `SampleControlDevices` (l.1974): every enabled control element, in
/// creation order, gets `Sample()`.
pub(crate) fn sample_control_devices(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    // BUG(C5): the queue is moved out of the solution and put back so that the sweep can take
    // the whole circuit beside it; direct access lends the queue beside the parts it changes.
    let mut queue = std::mem::take(&mut ckt.solution.control_queue);
    let result = sample_control_devices_q(ckt, env, &mut queue);
    ckt.solution.control_queue = queue;
    result
}

/// The queue-external body of `SampleControlDevices` (`DoMultiRate` re-samples
/// mid-sweep while it already holds the queue).
pub(super) fn sample_control_devices_q(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    queue: &mut ControlQueue,
) -> SolveResult {
    // BUG(C5): the control list is copied on every control iteration because
    // `dispatch_control` takes the whole circuit; with narrower access the loop iterates
    // `&ckt.controls` in place.
    let controls = ckt.controls.clone();
    for r in controls {
        if env.store.ckt_elem(r).cd().enabled {
            dispatch_control(r, ControlOp::Sample, ckt, env, queue)?;
        }
    }
    Ok(())
}

/// Pascal `Utilities.DoResetControls`: `Reset()` on every enabled control
/// element. Invoked by `Set mode=` (Solution `Set_Mode` tail) and the `Reset`
/// command.
pub(crate) fn reset_all_controls(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    // BUG(C5): the queue is moved out of the solution and put back after the loop so that
    // `dispatch_control` can take the whole circuit beside it; direct access lends the queue
    // beside the parts the dispatch changes.
    let mut queue = std::mem::take(&mut ckt.solution.control_queue);
    let mut result = Ok(());
    // BUG(C5): the control list is copied because `dispatch_control` takes the whole circuit;
    // with narrower access the loop iterates `&ckt.controls` in place.
    let controls = ckt.controls.clone();
    for r in controls {
        if env.store.ckt_elem(r).cd().enabled {
            result = dispatch_control(r, ControlOp::Reset, ckt, env, &mut queue);
            if result.is_err() {
                break;
            }
        }
    }
    ckt.solution.control_queue = queue;
    result
}
