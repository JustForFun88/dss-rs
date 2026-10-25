//! The dynamics solve mode: the entry that seeds the dynamic states from the
//! last power flow, the predictor/corrector step loop over the step size `h`,
//! the integration of the states, and the refusal of an element that has no
//! state in the present run (the dynamics step, the FaultStudy and MonteFault
//! solves and the step-solution commands share it).

use crate::circuit::Circuit;
use crate::elements::ckt::DynamicsSeed;
use crate::support::dynamics::IterationFlag;

use super::power_flow::solve_snap;
use super::time_series::end_of_time_step_cleanup;
use super::{SolveEnv, SolveMode, SolveResult, sys_ctx};

/// Seed the dynamic states at the entry into a dynamics-model mode (Dynamic,
/// FaultStudy, MonteFault) from a solved circuit, and record on every PC
/// element how the entry treated it ([`DynamicsSeed`]).
///
/// The entry seeds every enabled element the last power flow held in its
/// present shape and connection ([`CktElementData::in_power_flow`]) and records
/// its phase count. An element without a model to integrate
/// ([`CktElement::undefined_dynamics_model`]) is seeded with what a direct
/// solve reads and marked [`DynamicsSeed::NoModel`]: it takes part in a fault
/// study, and the integrating dynamics solve refuses it. The entry seeds nothing
/// of an element that power flow did not hold, created, enabled, re-phased or
/// given another bus after it, nor of one whose map does not fit that power
/// flow, marked [`DynamicsSeed::OutsidePowerFlow`]: it is not in the run, and
/// the first solve refuses it ([`refusal_of_an_element_outside_the_run`]). A
/// node map kept for the readers is no membership: an element enabled again on
/// the nodes it had is outside the run until a power flow solves with it.
///
/// [`CktElement::undefined_dynamics_model`]: crate::elements::traits::CktElement::undefined_dynamics_model
/// [`CktElementData::in_power_flow`]: crate::elements::ckt::CktElementData::in_power_flow
pub(crate) fn calc_initial_machine_states(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let solved_nodes = ckt.solution.node_v.len();
    // `env.store` is disjoint from `ckt`, so the loop reads `node_v` straight
    // out of the solution — no per-entry copy of the voltage vector.
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
        elem.cd_mut().dynamics_seed = DynamicsSeed::Unseeded;
        if !elem.cd().enabled {
            continue;
        }
        let nphases = elem.cd().nphases;
        let mut seed = DynamicsSeed::Seeded(nphases);
        if elem.has_dynamic_state() {
            let cd = elem.cd();
            if !cd.in_power_flow
                || cd.node_ref.len() != cd.yorder
                || cd.node_ref.iter().any(|&n| n >= solved_nodes)
            {
                elem.cd_mut().dynamics_seed = DynamicsSeed::OutsidePowerFlow;
                continue;
            }
            if elem.undefined_dynamics_model().is_some() {
                seed = DynamicsSeed::NoModel(nphases);
            }
        }
        elem.cd_mut().dynamics_seed = seed;
        elem.init_state_vars(&sys, &ckt.solution.node_v);
        errs.extend(elem.cd_mut().obj.take_errors());
        if elem.cd_mut().obj.take_abort() {
            aborted = true;
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

/// Integrate the dynamic states of every PC element in the present run
/// ([`CktElement::in_dynamics_run`]) by one predictor or corrector half-step. A
/// disabled element is out of the model and keeps its states untouched.
///
/// [`CktElement::in_dynamics_run`]: crate::elements::traits::CktElement::in_dynamics_run
fn integrate_pc_states(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    // `env.store` is disjoint from `ckt`: borrow `node_v` in place (a full copy
    // of the voltage vector per half-step otherwise).
    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem_mut(r);
        if elem.in_dynamics_run() {
            elem.integrate_states(&sys, &ckt.solution.node_v);
        }
    }
}

/// The word the user writes in `Set mode=` for a dynamics-model mode, `None`
/// for every other mode.
pub(crate) fn dynamics_mode_word(mode: SolveMode) -> Option<&'static str> {
    match mode {
        SolveMode::Dynamic => Some("dynamic"),
        SolveMode::FaultStudy => Some("faultstudy"),
        SolveMode::MonteFault => Some("mf"),
        _ => None,
    }
}

/// Why a solve in a dynamics-model mode cannot run: the first enabled PC
/// element, in list order, that has dynamic state but no state in the present
/// run. An `integrating` solve (the dynamics step) also refuses an element
/// without a model to integrate, one that cannot integrate the present step
/// ([`CktElement::step_refusal`]) and one whose linked `DynamicExp` leaves its
/// integration ill-defined, none of which a direct solve reads. `None` when
/// every enabled element can take part. A disabled element is out of the
/// model and never answers. `mode_word` is [`dynamics_mode_word`] of the
/// present mode.
///
/// [`CktElement::step_refusal`]: crate::elements::traits::CktElement::step_refusal
pub(crate) fn refusal_of_an_element_outside_the_run(
    ckt: &Circuit,
    env: &SolveEnv,
    mode_word: &str,
    integrating: bool,
) -> Option<String> {
    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem(r);
        let cd = elem.cd();
        if !cd.enabled {
            continue;
        }
        if elem.has_dynamic_state() {
            if integrating && let Some(msg) = elem.undefined_dynamics_model() {
                return Some(msg);
            }
            let name = format!("{}.{}", r.class_name(), cd.obj.name());
            let remedy = format!("Run Set mode=snap, Solve and Set mode={mode_word}");
            match cd.dynamics_seed {
                DynamicsSeed::Unseeded => {
                    return Some(format!(
                        "{name} joined after Set mode={mode_word} and has no initial state. \
                         {remedy} to start a new run that includes it."
                    ));
                }
                DynamicsSeed::OutsidePowerFlow if cd.renumbered => {
                    return Some(format!(
                        "{name} was renumbered by a bus rebuild since the last power flow and \
                         has no initial state. {remedy} to start a new run that includes it."
                    ));
                }
                DynamicsSeed::OutsidePowerFlow => {
                    return Some(format!(
                        "{name} was created, enabled, re-phased or reconnected after the last \
                         Solve before Set mode={mode_word} and has no initial state. {remedy} \
                         to start a new run that includes it."
                    ));
                }
                DynamicsSeed::Seeded(n) | DynamicsSeed::NoModel(n) if n != cd.nphases => {
                    return Some(format!(
                        "{name} changed its phase count after Set mode={mode_word} and has no \
                         initial state for it. {remedy} to start a new run with it."
                    ));
                }
                DynamicsSeed::NoModel(_) if integrating => {
                    return Some(format!(
                        "{name} had no dynamics model at Set mode={mode_word} and has no \
                         initial state. {remedy} to start a new run that includes it."
                    ));
                }
                DynamicsSeed::Seeded(_) | DynamicsSeed::NoModel(_) => {}
            }
            if integrating && let Some(msg) = elem.step_refusal(ckt.solution.h) {
                return Some(msg);
            }
        }
        if integrating && let Some(msg) = elem.dynamics_refusal() {
            return Some(msg);
        }
    }
    None
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

/// The dynamics stepping loop. Each step first refuses an element without a
/// state in the run ([`refusal_of_an_element_outside_the_run`]), then advances
/// the clock, integrates and solves (predictor, corrector), samples the
/// monitors and runs the end-of-step cleanup.
fn solve_dynamic_body(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    for _ in 1..=ckt.solution.number_of_times {
        if ckt.solution.solution_abort {
            continue;
        }
        // An element without a state in the run stops the solve before the
        // step integrates anything.
        if let Some(msg) = dynamics_mode_word(ckt.solution.mode)
            .and_then(|word| refusal_of_an_element_outside_the_run(ckt, env, word, true))
        {
            return Err(msg);
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
