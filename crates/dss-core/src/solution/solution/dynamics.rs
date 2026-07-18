//! The dynamics solve mode: `SolveDynamic` (`SolutionAlgs.pas` l.333), the
//! predictor/corrector step loop over `DynaVars.h`, `IntegratePCStates`
//! (`SolutionAlgs.pas` l.321) and `calcInitialMachineStates` (`Solution.pas`
//! l.2156, the dynamics-entry machine-state init).
//!
//! Scope (WP7.7 step 1): the solve-loop driver + the dynamics-entry machine-state
//! init hook. The per-element state machinery (`InitStateVars` /
//! `IntegrateStates` / the dynamics current injection) is the no-op
//! `CktElement` trait default here; Generator / Storage / PVSystem / IndMach012
//! fill it in WP7.7 steps 2–3, at which point this loop drives real dynamics.

use crate::circuit::Circuit;
use crate::support::dynamics::IterationFlag;

use super::power_flow::solve_snap;
use super::time_series::end_of_time_step_cleanup;
use super::{SolveEnv, SolveResult, sys_ctx};

/// Pascal `calcInitialMachineStates` (`Solution.pas` l.2156): set the state
/// variables for every enabled PC element (machines and inverters). Called when
/// entering dynamics mode from a solved, non-dynamic circuit (the
/// `OK_for_Dynamics` success path — the executive `Set mode=` handler, mirroring
/// the harmonics `InitializeForHarmonics` entry). PC classes without state
/// variables do nothing (the base `TPCElement.InitStateVars` is a no-op).
pub(crate) fn calc_initial_machine_states(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    // Pascal `TWindGenObj.InitStateVars` (and the classic machines) can
    // `DoSimpleMsg` + `SetSolutionAbort(TRUE)` from inside init (e.g. a
    // non-3-phase WindGen — the WTG3 model is 3-phase-only). Drain each
    // element's error/abort here and lift the abort onto the solution, so the
    // subsequent `solve_dynamic` skips its steps instead of running the model
    // on a malformed terminal (which would over-read the terminal array).
    let mut aborted = false;
    let mut errs = crate::diag::ErrorLog::new();
    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem_mut(r);
        if elem.cd().enabled {
            elem.init_state_vars(&sys, &node_v);
            errs.extend(elem.cd_mut().obj.take_errors());
            if elem.cd_mut().obj.take_abort() {
                aborted = true;
            }
        }
    }
    env.errors.extend(errs);
    if aborted {
        ckt.solution.solution_abort = true;
    }
    // Pascal `TPCElement.InitStateVars` calls `SetYprimInvalid(TRUE)` on the
    // machines that present a *dynamics* YPrim (the Norton `Yeq`/`Zthev`
    // admittance a Generator/IndMach012/WindGen switches to, the Storage/PVSystem
    // GFM short-circuit YPrim), and `SetYprimInvalid` raises `SystemYChanged`
    // (`CktElement.pas:245`) — so the first dynamics solve rebuilds the system Y
    // with those dynamics YPrims stamped in. The port sets each element's
    // `yprim_invalid` inside `init_state_vars` but loses that `SystemYChanged`
    // side effect (no solution channel there); raise it here so the next
    // `solve_snap` rebuilds Y. Without it the machine's power-flow YPrim survives
    // into dynamics and its (large) Norton injection current is left uncancelled
    // (the WindGen runs the terminal voltage away — WP-U1.8).
    ckt.solution.system_y_changed = true;
}

/// Pascal `TSolutionAlgs.IntegratePCStates` (`SolutionAlgs.pas` l.321):
/// integrate the dynamic states of every PC element by one (half-)step. Only PC
/// elements can have dynamic states; the base `TPCElement.IntegrateStates`
/// default does nothing, so static elements (loads, sources) are unaffected.
/// Unlike `calcInitialMachineStates`, the Pascal loop does **not** test
/// `Enabled` — it walks the full `PCElements` list.
fn integrate_pc_states(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let node_v = ckt.solution.node_v.clone();
    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem_mut(r);
        elem.integrate_states(&sys, &node_v);
    }
}

/// Pascal `TSolutionAlgs.SolveDynamic` (`SolutionAlgs.pas` l.333): the
/// predictor/corrector dynamics step loop. Each of `number_of_times` steps
/// advances the clock, integrates the PC states then re-solves the snapshot
/// twice — predictor (`IterationFlag = NewTimeStep`) and corrector
/// (`IterationFlag = SameTimeStep`) — samples the monitors, and runs the
/// end-of-step cleanup. `SolutionInitialized` is forced `true` so the inner
/// power flow does not re-initialise per step. The `finally`'s
/// `MonitorClass.SaveAll()` runs unconditionally (even on an early
/// `Err`/aborted step) — see `mod.rs`'s `flushed_records` doc for why this
/// now matters (WPG.2: `SolveGeneralTime` is the one mode that never flushes).
pub(super) fn solve_dynamic(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    // If we're in dynamics mode, no need to re-initialize.
    ckt.solution.solution_initialized = true;
    // Needed for energy meters and storage devices.
    ckt.solution.interval_hrs = ckt.solution.h / 3600.0;

    let result = solve_dynamic_body(ckt, env);
    crate::solution::monitors::save_all_monitors(ckt, env);
    result
}

/// The `SolveDynamic` stepping loop (the Pascal `try` body).
fn solve_dynamic_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
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
        // Assume the price signal stays constant for dynamic calcs.

        // Predictor
        ckt.solution.iteration_flag = IterationFlag::NewTimeStep;
        integrate_pc_states(ckt, env);
        solve_snap(ckt, env)?;
        // Corrector
        ckt.solution.iteration_flag = IterationFlag::SameTimeStep;
        integrate_pc_states(ckt, env);
        solve_snap(ckt, env)?;

        crate::solution::monitors::sample_all_monitors(ckt, env, false); // all monitors take a sample
        end_of_time_step_cleanup(ckt, env);
    }
    Ok(())
}
