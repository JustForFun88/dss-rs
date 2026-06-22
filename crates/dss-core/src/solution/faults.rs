//! Fault class-level sweeps — Pascal `TSolutionObj.Check_Fault_Status`
//! (`Solution.pas` l.2146) and `Utilities.DoResetFaults` (l.984). Free functions
//! over the registry: each Fault is borrowed from the class store while the
//! disjoint `Solution` fields (`node_v`/`event_log`) are borrowed from the
//! circuit (the WP5.7 control-sweep borrow split).

use crate::circuit::Circuit;
use crate::elements::pd::fault::{Fault, FaultStatusCtx};
use crate::solution::solution::{Solution, SolveEnv, SolveResult, sys_ctx};

/// Pascal `TSolutionObj.Check_Fault_Status`: drive every Fault's `CheckStatus`
/// in the control-iteration loop. A fault that toggles `Is_ON` invalidates its
/// YPrim, which (Pascal `Set_YprimInvalid`, when enabled) flags the system Y for
/// a rebuild.
pub(crate) fn check_fault_status(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let faults = ckt.faults.clone();
    if faults.is_empty() {
        return Ok(());
    }
    let sys = sys_ctx(ckt);
    let control_mode = ckt.solution.control_mode;
    let int_hour = ckt.solution.int_hour;
    let t = ckt.solution.t;
    let control_iter = ckt.solution.control_iteration;

    for r in faults {
        let (changed, enabled) = {
            let f_obj = env.store.obj_mut(r);
            let Some(fault) = f_obj.as_any_mut().downcast_mut::<Fault>() else {
                continue;
            };
            let enabled = fault.cd.enabled;
            let Solution {
                node_v, event_log, ..
            } = &mut ckt.solution;
            let ctx = FaultStatusCtx {
                control_mode,
                int_hour,
                t,
                control_iter,
                sys: &sys,
                node_v,
            };
            let changed = fault.check_status(&ctx, event_log);
            (changed, enabled)
        };
        // Pascal `Set_YprimInvalid(TRUE)` flags `SystemYChanged` only for an
        // in-circuit (enabled) element.
        if changed && enabled {
            ckt.solution.system_y_changed = true;
        }
    }
    Ok(())
}

/// Pascal `Utilities.DoResetFaults`: `Reset()` (clear the self-cleared latch) on
/// every Fault. Invoked by `Set mode=` (`Set_Mode` tail) and the `Reset` command.
pub(crate) fn reset_faults(ckt: &mut Circuit, env: &mut SolveEnv) {
    let faults = ckt.faults.clone();
    for r in faults {
        if let Some(fault) = env.store.obj_mut(r).as_any_mut().downcast_mut::<Fault>() {
            fault.reset();
        }
    }
}
