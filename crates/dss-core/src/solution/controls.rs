//! The control loop — Pascal `Solution.pas` `Sample_DoControlActions` /
//! `SampleControlDevices` / `DoControlActions` (l.1941–2008) plus
//! `Utilities.DoResetControls` and `ControlQueue.DoMultiRate`.
//!
//! Pascal reaches the control's controlled/monitored elements through live
//! object pointers; here every dispatch resolves the control's [`ElemRef`]s
//! against the executive's class registry and splits the mutable borrows
//! (PHASE5_PLAN §2.1): the control object, its controlled transformer or
//! capacitor, and (CapControl only) the monitored element are borrowed at once
//! via [`ElemStore::pair_mut`]/[`ElemStore::triple_mut`] — RegControl and
//! Transformer are different classes, so the split always succeeds.
//!
//! The control queue is `std::mem::take`n out of the solution for the duration
//! of a sweep (Pascal's queue is a separate object, so actions pushing or
//! deleting further records mid-sweep work identically), and put back at the
//! end.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::control::cap_control::CapControl;
use crate::elements::control::control_elem::CtrlCtx;
use crate::elements::control::gen_dispatcher::{GenDispatchEnv, GenDispatcher};
use crate::elements::control::reg_control::RegControl;
use crate::elements::pc::generator::Generator;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{ElemRef, ElemStore, SysCtx};
use crate::solution::control_queue::{ControlActioner, ControlQueue, TimeRec};
use crate::solution::solution::{
    CONTROLSOFF, CTRLSTATIC, EVENTDRIVEN, MULTIRATE, Solution, SolveEnv, SolveResult, TIMEDRIVEN,
    solve_circuit, sys_ctx,
};

/// What the dispatch should invoke on the control element.
#[derive(Clone, Copy)]
enum ControlOp {
    /// `TControlElem.Sample`.
    Sample,
    /// `TControlElem.DoPendingAction(Code, ProxyHdl)`.
    Action { code: i32 },
    /// `TControlElem.Reset` (the `Reset` command / `Set mode=` side effect).
    Reset,
}

/// Pascal `Sample_DoControlActions` (`Solution.pas` l.1996).
pub(crate) fn sample_do_control_actions(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.solution.control_mode == CONTROLSOFF {
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
fn sample_control_devices(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let mut queue = std::mem::take(&mut ckt.solution.control_queue);
    let result = sample_control_devices_q(ckt, env, &mut queue);
    ckt.solution.control_queue = queue;
    result
}

/// The queue-external body of `SampleControlDevices` (`DoMultiRate` re-samples
/// mid-sweep while it already holds the queue).
fn sample_control_devices_q(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    queue: &mut ControlQueue,
) -> SolveResult {
    let controls = ckt.controls.clone();
    for r in controls {
        if env.store.ckt_elem(r).cd().enabled {
            dispatch_control(r, ControlOp::Sample, ckt, env, queue)?;
        }
    }
    Ok(())
}

/// Pascal `DoControlActions` (l.1941): per-control-mode queue dispatch.
fn do_control_actions(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    match ckt.solution.control_mode {
        CTRLSTATIC => {
            // Execute the nearest set of control actions but leave time as is.
            if ckt.solution.control_queue.is_empty() {
                ckt.solution.control_actions_done = true;
            } else {
                let (mut xhour, mut xsec) = (0_i32, 0.0_f64);
                run_nearest_actions(ckt, env, &mut xhour, &mut xsec)?; // ignore time advancement
            }
        }
        EVENTDRIVEN => {
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
        TIMEDRIVEN => {
            // Do all actions having an action time <= the specified time.
            let (hour, sec) = (ckt.solution.int_hour, ckt.solution.t);
            if !run_actions(ckt, env, hour, sec)? {
                ckt.solution.control_actions_done = true;
            }
        }
        MULTIRATE => {
            let mut queue = std::mem::take(&mut ckt.solution.control_queue);
            let result = do_multi_rate(ckt, env, &mut queue);
            ckt.solution.control_queue = queue;
            if !result? {
                ckt.solution.control_actions_done = true;
            }
        }
        _ => {}
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
        control: ElemRef,
        code: i32,
        _proxy: i32,
        queue: &mut ControlQueue,
    ) {
        if self.failure.is_some() {
            return;
        }
        if let Err(e) = dispatch_control(
            control,
            ControlOp::Action { code },
            self.ckt,
            self.env,
            queue,
        ) {
            self.failure = Some(e);
        }
    }
}

/// Pascal `ControlQueue.DoMultiRate` (l.359), ported verbatim including the
/// scratch-register choreography (`Temp_Int`/`Temp_dbl` index comments match
/// the Pascal). Runs all due actions, then sweeps forward window-by-window,
/// re-solving the circuit and re-sampling the controls between windows.
fn do_multi_rate(
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
            ControlOp::Action { code: p.code },
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
                ControlOp::Action { code: p2.code },
                ckt,
                env,
                queue,
            )?;
            popped = queue.pop_time(ltimer, true);
        } else {
            // The next action is outside the time window: run it and exit.
            dispatch_control(
                p.control,
                ControlOp::Action { code: p.code },
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

/// Pascal `Utilities.DoResetControls`: `Reset()` on every enabled control
/// element. Invoked by `Set mode=` (Solution `Set_Mode` tail) and the `Reset`
/// command.
pub(crate) fn reset_all_controls(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let mut queue = std::mem::take(&mut ckt.solution.control_queue);
    let mut result = Ok(());
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

/// Which concrete control class an [`ElemRef`] names, plus its element refs.
#[derive(Clone, Copy)]
enum ControlKind {
    Reg {
        controlled: Option<ElemRef>,
    },
    Cap {
        controlled: Option<ElemRef>,
        monitored: Option<ElemRef>,
    },
    GenDispatch {
        monitored: Option<ElemRef>,
        element_terminal: usize,
    },
}

/// The dispatch core: split the borrows, downcast, and invoke `Sample` /
/// `DoPendingAction` / `Reset` on the control with its controlled (and
/// monitored) elements. Errors map to Pascal's `EControlProblem` path
/// (`DoSimpleMsg` 484 + "Solution aborted.").
fn dispatch_control(
    r: ElemRef,
    op: ControlOp,
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    queue: &mut ControlQueue,
) -> SolveResult {
    let sys = sys_ctx(ckt);
    let SolveEnv { store, errors, .. } = env;

    // Identify the control and read its target refs (immutable peek).
    let (kind, full_name) = {
        let obj = store.obj(r);
        if let Some(rc) = obj.as_any().downcast_ref::<RegControl>() {
            (
                ControlKind::Reg {
                    controlled: rc.ccd.controlled_element,
                },
                format!("RegControl.{}", rc.ccd.cd.obj.name()),
            )
        } else if let Some(cc) = obj.as_any().downcast_ref::<CapControl>() {
            (
                ControlKind::Cap {
                    controlled: cc.ccd.controlled_element,
                    monitored: cc.ccd.monitored_element,
                },
                format!("CapControl.{}", cc.ccd.cd.obj.name()),
            )
        } else if let Some(gd) = obj.as_any().downcast_ref::<GenDispatcher>() {
            (
                ControlKind::GenDispatch {
                    monitored: gd.ccd.monitored_element,
                    element_terminal: gd.ccd.element_terminal.max(1) as usize,
                },
                format!("GenDispatcher.{}", gd.ccd.cd.obj.name()),
            )
        } else {
            return Err(format!(
                "Internal error: control element {} is not a ported control class.",
                obj.data().name()
            ));
        }
    };

    let abort = |errors: &mut Vec<String>, full_name: &str, what: &str| -> String {
        errors.push(format!(
            "Error Sampling Control Device \"{full_name}\". Error = {what}"
        ));
        "Solution aborted.".to_string()
    };

    // GenDispatcher redispatches a *dynamic* set of generators, so it needs the
    // whole class registry (not a fixed pair/triple) and uses none of the
    // event/Y context — handle it before building the shared `CtrlCtx`.
    // `DoPendingAction`/`Reset` are no-ops in Pascal; only `Sample` acts.
    if let ControlKind::GenDispatch {
        monitored,
        element_terminal,
    } = kind
    {
        if let ControlOp::Sample = op {
            let Some(mon) = monitored else {
                return Err(abort(errors, &full_name, "Monitored element not set"));
            };
            // Clone the dispatcher out so the env can hold the store mutably;
            // `Sample` only mutates the cached generator list, copied back after.
            let mut gd = store
                .obj(r)
                .as_any()
                .downcast_ref::<GenDispatcher>()
                .expect("kind matched above")
                .clone();
            let generators = ckt.generators.clone();
            let changed = {
                let mut env = GenDispEnv {
                    store: &mut **store,
                    node_v: &ckt.solution.node_v,
                    sys: &sys,
                    monitored: mon,
                    element_terminal,
                    generators,
                };
                gd.sample(&mut env)
            };
            *store
                .obj_mut(r)
                .as_any_mut()
                .downcast_mut::<GenDispatcher>()
                .expect("kind matched above") = gd;
            if changed {
                // Force a recalc of power parameters + a re-solve at the new
                // dispatch value (Pascal `LoadsNeedUpdating := TRUE` +
                // `ControlQueue.Push(0, 0, 0, Self)`).
                ckt.solution.loads_need_updating = true;
                queue.push(0, 0.0, 0, 0, r);
            }
        }
        return Ok(());
    }

    // Build the shared control context from disjoint Solution fields.
    let Solution {
        node_v,
        event_log,
        system_y_changed,
        control_mode,
        control_iteration,
        int_hour,
        t,
        dbl_hour,
        ..
    } = &mut ckt.solution;
    let mut ctx = CtrlCtx {
        node_v: &*node_v,
        sys: &sys,
        queue,
        events: event_log,
        errors,
        system_y_changed,
        control_mode: *control_mode,
        int_hour: *int_hour,
        t: *t,
        dbl_hour: *dbl_hour,
        control_iter: *control_iteration,
        self_ref: r,
    };

    match kind {
        // Handled (and returned) above, before the CtrlCtx was built.
        ControlKind::GenDispatch { .. } => unreachable!("GenDispatcher handled above"),
        ControlKind::Reg { controlled } => {
            let Some(target) = controlled else {
                return Err(abort(ctx.errors, &full_name, "Transformer element not set"));
            };
            let (cobj, tobj) = store.pair_mut(r, target);
            let rc = cobj
                .as_any_mut()
                .downcast_mut::<RegControl>()
                .expect("kind matched above");
            let Some(tr) = tobj.as_any_mut().downcast_mut::<Transformer>() else {
                return Err(abort(
                    ctx.errors,
                    &full_name,
                    "Controlled element is not a Transformer",
                ));
            };
            match op {
                ControlOp::Sample => rc.sample(tr, &mut ctx),
                ControlOp::Action { code } => rc.do_pending_action(code, tr, &mut ctx),
                ControlOp::Reset => rc.reset(),
            }
        }
        ControlKind::Cap {
            controlled,
            monitored,
        } => {
            let Some(target) = controlled else {
                return Err(abort(ctx.errors, &full_name, "Capacitor element not set"));
            };
            match op {
                ControlOp::Sample => {
                    let Some(mon) = monitored else {
                        return Err(abort(ctx.errors, &full_name, "Monitored element not set"));
                    };
                    if mon == target {
                        // Time/Follow control: the capacitor monitors itself.
                        // The monitored role only *reads* solved state, so a
                        // clone stands in for the second live borrow.
                        let (cobj, capobj) = store.pair_mut(r, target);
                        let cc = cobj
                            .as_any_mut()
                            .downcast_mut::<CapControl>()
                            .expect("kind matched above");
                        let Some(cap) = capobj.as_any_mut().downcast_mut::<Capacitor>() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Controlled element is not a Capacitor",
                            ));
                        };
                        let mut mon_clone = cap.clone();
                        cc.sample(cap, &mut mon_clone, &mut ctx);
                    } else {
                        let (cobj, capobj, monobj) = store.triple_mut(r, target, mon);
                        let cc = cobj
                            .as_any_mut()
                            .downcast_mut::<CapControl>()
                            .expect("kind matched above");
                        let Some(cap) = capobj.as_any_mut().downcast_mut::<Capacitor>() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Controlled element is not a Capacitor",
                            ));
                        };
                        let Some(mon_elem) = monobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Monitored element is not a circuit element",
                            ));
                        };
                        cc.sample(cap, mon_elem, &mut ctx);
                    }
                }
                ControlOp::Action { .. } => {
                    // CapControl ignores the action code; PendingChange rules.
                    let (cobj, capobj) = store.pair_mut(r, target);
                    let cc = cobj
                        .as_any_mut()
                        .downcast_mut::<CapControl>()
                        .expect("kind matched above");
                    let Some(cap) = capobj.as_any_mut().downcast_mut::<Capacitor>() else {
                        return Err(abort(
                            ctx.errors,
                            &full_name,
                            "Controlled element is not a Capacitor",
                        ));
                    };
                    cc.do_pending_action(cap, &mut ctx);
                }
                ControlOp::Reset => {
                    let (cobj, capobj) = store.pair_mut(r, target);
                    let cc = cobj
                        .as_any_mut()
                        .downcast_mut::<CapControl>()
                        .expect("kind matched above");
                    let Some(cap) = capobj.as_any_mut().downcast_mut::<Capacitor>() else {
                        return Err(abort(
                            ctx.errors,
                            &full_name,
                            "Controlled element is not a Capacitor",
                        ));
                    };
                    if cc.reset_with(cap) {
                        *ctx.system_y_changed = true;
                    }
                }
            }
        }
    }

    Ok(())
}

/// [`GenDispatchEnv`] over the class registry: the monitored element's terminal
/// power and the dispatched generators' `kWBase`/`kvarBase`, reached through the
/// store. The generator-scan list is the circuit's creation-ordered
/// `generators` list (cloned by the caller so the store can be borrowed freely).
struct GenDispEnv<'a> {
    store: &'a mut dyn ElemStore,
    node_v: &'a [Complex64],
    sys: &'a SysCtx,
    monitored: ElemRef,
    element_terminal: usize,
    generators: Vec<ElemRef>,
}

impl GenDispEnv<'_> {
    fn generator(store: &dyn ElemStore, g: ElemRef) -> &Generator {
        store
            .obj(g)
            .as_any()
            .downcast_ref::<Generator>()
            .expect("GenDispatcher list entry is a Generator")
    }
    fn generator_mut(store: &mut dyn ElemStore, g: ElemRef) -> &mut Generator {
        store
            .obj_mut(g)
            .as_any_mut()
            .downcast_mut::<Generator>()
            .expect("GenDispatcher list entry is a Generator")
    }
}

impl GenDispatchEnv for GenDispEnv<'_> {
    fn monitored_power(&mut self) -> Complex64 {
        self.store.ckt_elem_mut(self.monitored).terminal_power(
            self.sys,
            self.node_v,
            self.element_terminal,
        )
    }
    fn find_enabled_gen(&self, name: &str) -> Option<ElemRef> {
        let r = self.store.find_ckt_element(&format!("generator.{name}"))?;
        self.store.ckt_elem(r).cd().enabled.then_some(r)
    }
    fn all_enabled_gens(&self) -> Vec<ElemRef> {
        self.generators
            .iter()
            .copied()
            .filter(|&g| self.store.ckt_elem(g).cd().enabled)
            .collect()
    }
    fn gen_kw_base(&self, g: ElemRef) -> f64 {
        Self::generator(self.store, g).kw_base
    }
    fn set_gen_kw_base(&mut self, g: ElemRef, value: f64) {
        Self::generator_mut(self.store, g).kw_base = value;
    }
    fn gen_kvar_base(&self, g: ElemRef) -> f64 {
        Self::generator(self.store, g).kvar_base
    }
    fn set_gen_kvar_base(&mut self, g: ElemRef, value: f64) {
        Self::generator_mut(self.store, g).kvar_base = value;
    }
}
