//! The dispatch core: identify the concrete control class behind an [`ElemRef`],
//! split the mutable borrows of the control + its controlled/monitored elements,
//! and invoke `Sample`/`DoPendingAction`/`Reset`. Includes the GenDispatcher
//! environment that reaches generators through the class registry.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::control::cap_control::CapControl;
use crate::elements::control::control_elem::CtrlCtx;
use crate::elements::control::gen_dispatcher::{GenDispatchEnv, GenDispatcher};
use crate::elements::control::recloser::Recloser;
use crate::elements::control::reg_control::RegControl;
use crate::elements::control::relay::Relay;
use crate::elements::control::storage_controller::StorageController;
use crate::elements::control::swt_control::SwtControl;
use crate::elements::pc::generator::Generator;
use crate::elements::pd::capacitor::Capacitor;
use crate::elements::pd::fuse::Fuse;
use crate::elements::pd::transformer::Transformer;
use crate::elements::traits::{ElemRef, ElemStore, SysCtx};
use crate::solution::control_queue::ControlQueue;
use crate::solution::solution::{Solution, SolveEnv, SolveResult, sys_ctx};

use super::ControlOp;

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
    /// SwtControl: a manual switch over a generic controlled element. `Sample`
    /// reads no monitored element (only the control's own queued action), so the
    /// controlled element is borrowed only for `Action`/`Reset`.
    Swt {
        controlled: Option<ElemRef>,
    },
    /// Fuse: per-phase overcurrent protection. `Sample` reads the monitored
    /// element's currents and the controlled element's conductor state; `Action`
    /// blows one controlled phase; `Reset` restores the normal state. The
    /// monitored element is often the controlled element itself.
    Fuse {
        controlled: Option<ElemRef>,
        monitored: Option<ElemRef>,
    },
    /// Recloser: overcurrent recloser. `Sample` reads the monitored currents and
    /// the controlled terminal state; `Action` (OPEN/CLOSE/RESET) trips/recloses
    /// the whole controlled terminal; `Reset` restores the normal state. Like the
    /// Fuse, the monitored element is often the controlled element itself.
    Recloser {
        controlled: Option<ElemRef>,
        monitored: Option<ElemRef>,
    },
    /// Relay: the general protection control. Same borrow shape as the Recloser
    /// (`Sample` reads the monitored element + controlled terminal; `Action`
    /// trips/recloses the whole terminal; `Reset` restores the normal state).
    Relay {
        controlled: Option<ElemRef>,
        monitored: Option<ElemRef>,
    },
    GenDispatch {
        monitored: Option<ElemRef>,
        element_terminal: usize,
    },
    /// StorageController is a Phase-6 skeleton: with no Storage element the fleet
    /// is always empty, so `Sample`/`DoPendingAction`/`Reset` are inert no-ops
    /// (PHASE6_PLAN §2.6; the named-missing-list per-sample 14403 is the one
    /// documented divergence). Carries no refs — there is nothing to dispatch.
    StorageSkeleton,
}

/// The dispatch core: split the borrows, downcast, and invoke `Sample` /
/// `DoPendingAction` / `Reset` on the control with its controlled (and
/// monitored) elements. Errors map to Pascal's `EControlProblem` path
/// (`DoSimpleMsg` 484 + "Solution aborted.").
pub(super) fn dispatch_control(
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
        } else if let Some(sw) = obj.as_any().downcast_ref::<SwtControl>() {
            (
                ControlKind::Swt {
                    controlled: sw.ccd.controlled_element,
                },
                format!("SwtControl.{}", sw.ccd.cd.obj.name()),
            )
        } else if let Some(fu) = obj.as_any().downcast_ref::<Fuse>() {
            (
                ControlKind::Fuse {
                    controlled: fu.ccd.controlled_element,
                    monitored: fu.ccd.monitored_element,
                },
                format!("Fuse.{}", fu.ccd.cd.obj.name()),
            )
        } else if let Some(rec) = obj.as_any().downcast_ref::<Recloser>() {
            (
                ControlKind::Recloser {
                    controlled: rec.ccd.controlled_element,
                    monitored: rec.ccd.monitored_element,
                },
                format!("Recloser.{}", rec.ccd.cd.obj.name()),
            )
        } else if let Some(rel) = obj.as_any().downcast_ref::<Relay>() {
            (
                ControlKind::Relay {
                    controlled: rel.ccd.controlled_element,
                    monitored: rel.ccd.monitored_element,
                },
                format!("Relay.{}", rel.ccd.cd.obj.name()),
            )
        } else if let Some(gd) = obj.as_any().downcast_ref::<GenDispatcher>() {
            (
                ControlKind::GenDispatch {
                    monitored: gd.ccd.monitored_element,
                    // 1-based terminal; `.max(1)` guards an unset/0 terminal that
                    // Pascal would turn into an out-of-range `Power[0]`.
                    element_terminal: gd.ccd.element_terminal.max(1) as usize,
                },
                format!("GenDispatcher.{}", gd.ccd.cd.obj.name()),
            )
        } else if let Some(sc) = obj.as_any().downcast_ref::<StorageController>() {
            (
                ControlKind::StorageSkeleton,
                format!("StorageController.{}", sc.ccd.cd.obj.name()),
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

    // StorageController skeleton: empty fleet → nothing to sample/act/reset.
    // (Faithful for the default element list; a named-but-missing ElementList
    // would emit a per-sample 14403 in Pascal — deferred with `Sample` to
    // Phase 7, see storage_controller.rs module doc.)
    if let ControlKind::StorageSkeleton = kind {
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
        ControlKind::StorageSkeleton => unreachable!("StorageController handled above"),
        ControlKind::Swt { controlled } => {
            match op {
                ControlOp::Sample => {
                    // SwtControl.Sample reads no controlled/monitored element.
                    let sw = store
                        .obj_mut(r)
                        .as_any_mut()
                        .downcast_mut::<SwtControl>()
                        .expect("kind matched above");
                    sw.sample(&mut ctx);
                }
                ControlOp::Action { code } => {
                    let Some(target) = controlled else {
                        return Err(abort(ctx.errors, &full_name, "Switched element not set"));
                    };
                    let (cobj, tobj) = store.pair_mut(r, target);
                    let sw = cobj
                        .as_any_mut()
                        .downcast_mut::<SwtControl>()
                        .expect("kind matched above");
                    let Some(ctrl) = tobj.as_ckt_element_mut() else {
                        return Err(abort(
                            ctx.errors,
                            &full_name,
                            "Switched element is not a circuit element",
                        ));
                    };
                    sw.do_pending_action(code, ctrl, &mut ctx);
                }
                ControlOp::Reset => {
                    // Pascal `Reset` restores the commanded state and forces the
                    // switched element back to `NormalState` (when not locked).
                    match controlled {
                        Some(target) => {
                            let (cobj, tobj) = store.pair_mut(r, target);
                            let sw = cobj
                                .as_any_mut()
                                .downcast_mut::<SwtControl>()
                                .expect("kind matched above");
                            if let Some(ctrl) = tobj.as_ckt_element_mut() {
                                if sw.reset_with(ctrl) {
                                    *ctx.system_y_changed = true;
                                }
                            } else {
                                sw.reset_control_side();
                            }
                        }
                        None => {
                            let sw = store
                                .obj_mut(r)
                                .as_any_mut()
                                .downcast_mut::<SwtControl>()
                                .expect("kind matched above");
                            sw.reset_control_side();
                        }
                    }
                }
            }
        }
        ControlKind::Fuse {
            controlled,
            monitored,
        } => {
            let Some(target) = controlled else {
                return Err(abort(ctx.errors, &full_name, "Switched element not set"));
            };
            match op {
                ControlOp::Sample => {
                    let Some(mon) = monitored else {
                        return Err(abort(ctx.errors, &full_name, "Monitored element not set"));
                    };
                    if mon == target {
                        // The monitored role only *reads* solved state, so an
                        // owned clone of the controlled element stands in for the
                        // second live borrow (currents recompute from node_v).
                        let mut mon_clone = store.obj(target).clone_box();
                        let (cobj, tobj) = store.pair_mut(r, target);
                        let fuse = cobj
                            .as_any_mut()
                            .downcast_mut::<Fuse>()
                            .expect("kind matched above");
                        let Some(ctrl) = tobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Switched element is not a circuit element",
                            ));
                        };
                        let mon_elem = mon_clone
                            .as_ckt_element_mut()
                            .expect("controlled element is a circuit element");
                        fuse.sample(ctrl, mon_elem, &mut ctx);
                    } else {
                        let (cobj, tobj, mobj) = store.triple_mut(r, target, mon);
                        let fuse = cobj
                            .as_any_mut()
                            .downcast_mut::<Fuse>()
                            .expect("kind matched above");
                        let Some(ctrl) = tobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Switched element is not a circuit element",
                            ));
                        };
                        let Some(mon_elem) = mobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Monitored element is not a circuit element",
                            ));
                        };
                        fuse.sample(ctrl, mon_elem, &mut ctx);
                    }
                }
                ControlOp::Action { code } => {
                    let (cobj, tobj) = store.pair_mut(r, target);
                    let fuse = cobj
                        .as_any_mut()
                        .downcast_mut::<Fuse>()
                        .expect("kind matched above");
                    let Some(ctrl) = tobj.as_ckt_element_mut() else {
                        return Err(abort(
                            ctx.errors,
                            &full_name,
                            "Switched element is not a circuit element",
                        ));
                    };
                    // The queue `code` carries the 1-based phase to blow.
                    fuse.do_pending_action(code, ctrl, &mut ctx);
                }
                ControlOp::Reset => {
                    let (cobj, tobj) = store.pair_mut(r, target);
                    let fuse = cobj
                        .as_any_mut()
                        .downcast_mut::<Fuse>()
                        .expect("kind matched above");
                    if let Some(ctrl) = tobj.as_ckt_element_mut()
                        && fuse.reset_with(ctrl)
                    {
                        *ctx.system_y_changed = true;
                    }
                }
            }
        }
        ControlKind::Recloser {
            controlled,
            monitored,
        } => {
            match op {
                ControlOp::Sample => {
                    let Some(target) = controlled else {
                        return Err(abort(ctx.errors, &full_name, "Switched element not set"));
                    };
                    let Some(mon) = monitored else {
                        return Err(abort(ctx.errors, &full_name, "Monitored element not set"));
                    };
                    if mon == target {
                        // The monitored role only *reads* solved state, so an
                        // owned clone of the controlled element stands in for the
                        // second live borrow (currents recompute from node_v).
                        let mut mon_clone = store.obj(target).clone_box();
                        let (cobj, tobj) = store.pair_mut(r, target);
                        let rec = cobj
                            .as_any_mut()
                            .downcast_mut::<Recloser>()
                            .expect("kind matched above");
                        let Some(ctrl) = tobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Switched element is not a circuit element",
                            ));
                        };
                        let mon_elem = mon_clone
                            .as_ckt_element_mut()
                            .expect("controlled element is a circuit element");
                        rec.sample(ctrl, mon_elem, &mut ctx);
                    } else {
                        let (cobj, tobj, mobj) = store.triple_mut(r, target, mon);
                        let rec = cobj
                            .as_any_mut()
                            .downcast_mut::<Recloser>()
                            .expect("kind matched above");
                        let Some(ctrl) = tobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Switched element is not a circuit element",
                            ));
                        };
                        let Some(mon_elem) = mobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Monitored element is not a circuit element",
                            ));
                        };
                        rec.sample(ctrl, mon_elem, &mut ctx);
                    }
                }
                ControlOp::Action { code } => {
                    let Some(target) = controlled else {
                        return Err(abort(ctx.errors, &full_name, "Switched element not set"));
                    };
                    let (cobj, tobj) = store.pair_mut(r, target);
                    let rec = cobj
                        .as_any_mut()
                        .downcast_mut::<Recloser>()
                        .expect("kind matched above");
                    let Some(ctrl) = tobj.as_ckt_element_mut() else {
                        return Err(abort(
                            ctx.errors,
                            &full_name,
                            "Switched element is not a circuit element",
                        ));
                    };
                    // The queue `code` carries CTRL_OPEN/CTRL_CLOSE/CTRL_RESET.
                    rec.do_pending_action(code, ctrl, &mut ctx);
                }
                ControlOp::Reset => {
                    // Pascal `Reset` restores the present state and forces the
                    // controlled terminal back to `NormalState` (no lock guard,
                    // unlike SwtControl).
                    match controlled {
                        Some(target) => {
                            let (cobj, tobj) = store.pair_mut(r, target);
                            let rec = cobj
                                .as_any_mut()
                                .downcast_mut::<Recloser>()
                                .expect("kind matched above");
                            if let Some(ctrl) = tobj.as_ckt_element_mut() {
                                if rec.reset_with(ctrl) {
                                    *ctx.system_y_changed = true;
                                }
                            } else {
                                rec.reset_control_side();
                            }
                        }
                        None => {
                            let rec = store
                                .obj_mut(r)
                                .as_any_mut()
                                .downcast_mut::<Recloser>()
                                .expect("kind matched above");
                            rec.reset_control_side();
                        }
                    }
                }
            }
        }
        ControlKind::Relay {
            controlled,
            monitored,
        } => {
            match op {
                ControlOp::Sample => {
                    let Some(target) = controlled else {
                        return Err(abort(ctx.errors, &full_name, "Switched element not set"));
                    };
                    let Some(mon) = monitored else {
                        return Err(abort(ctx.errors, &full_name, "Monitored element not set"));
                    };
                    if mon == target {
                        // The monitored role only *reads* solved state, so an
                        // owned clone of the controlled element stands in for the
                        // second live borrow (currents recompute from node_v).
                        let mut mon_clone = store.obj(target).clone_box();
                        let (cobj, tobj) = store.pair_mut(r, target);
                        let rel = cobj
                            .as_any_mut()
                            .downcast_mut::<Relay>()
                            .expect("kind matched above");
                        let Some(ctrl) = tobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Switched element is not a circuit element",
                            ));
                        };
                        let mon_elem = mon_clone
                            .as_ckt_element_mut()
                            .expect("controlled element is a circuit element");
                        rel.sample(ctrl, mon_elem, &mut ctx);
                    } else {
                        let (cobj, tobj, mobj) = store.triple_mut(r, target, mon);
                        let rel = cobj
                            .as_any_mut()
                            .downcast_mut::<Relay>()
                            .expect("kind matched above");
                        let Some(ctrl) = tobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Switched element is not a circuit element",
                            ));
                        };
                        let Some(mon_elem) = mobj.as_ckt_element_mut() else {
                            return Err(abort(
                                ctx.errors,
                                &full_name,
                                "Monitored element is not a circuit element",
                            ));
                        };
                        rel.sample(ctrl, mon_elem, &mut ctx);
                    }
                }
                ControlOp::Action { code } => {
                    let Some(target) = controlled else {
                        return Err(abort(ctx.errors, &full_name, "Switched element not set"));
                    };
                    let (cobj, tobj) = store.pair_mut(r, target);
                    let rel = cobj
                        .as_any_mut()
                        .downcast_mut::<Relay>()
                        .expect("kind matched above");
                    let Some(ctrl) = tobj.as_ckt_element_mut() else {
                        return Err(abort(
                            ctx.errors,
                            &full_name,
                            "Switched element is not a circuit element",
                        ));
                    };
                    // The queue `code` carries CTRL_OPEN/CTRL_CLOSE/CTRL_RESET.
                    rel.do_pending_action(code, ctrl, &mut ctx);
                }
                ControlOp::Reset => {
                    // Pascal `Reset()` logs "Resetting", restores the present
                    // state, and re-forces the controlled terminal to NormalState
                    // (raising SystemYChanged inside `reset_with`).
                    match controlled {
                        Some(target) => {
                            let (cobj, tobj) = store.pair_mut(r, target);
                            let rel = cobj
                                .as_any_mut()
                                .downcast_mut::<Relay>()
                                .expect("kind matched above");
                            if let Some(ctrl) = tobj.as_ckt_element_mut() {
                                rel.reset_with(ctrl, &mut ctx);
                            } else {
                                rel.reset_control_side();
                            }
                        }
                        None => {
                            let rel = store
                                .obj_mut(r)
                                .as_any_mut()
                                .downcast_mut::<Relay>()
                                .expect("kind matched above");
                            rel.reset_control_side();
                        }
                    }
                }
            }
        }
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
