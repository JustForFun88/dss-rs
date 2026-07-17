//! Port of `Controls/SwtControl.pas` — `TSwtControlObj`, a manual/automatic
//! **switch** control: it opens or closes every phase conductor of a controlled
//! element's terminal (`ControlledElement.Closed[0] := …`) after a time delay,
//! and can be *locked* against further operation. Unlike the sensing controls
//! (CapControl/RegControl), SwtControl reads no monitored quantity — it acts on
//! an explicit `Action`/`State` command queued through the control queue.
//!
//! Like every `TControlElem` it builds no Yprim and carries zero terminal
//! current; its single terminal attaches to the switched element's terminal bus
//! (`RecalcElementData`). The switching machinery joins the WP5.7 control sweep
//! with no new dispatch — `Sample` queues the pending action, `DoPendingAction`
//! flips the controlled conductors and logs `Opened`/`Closed`.
//!
//! **Property quirks settled against the oracle** (probed):
//! - `Action`/`Normal`/`State` all map onto the one `CurrentAction` field
//!   (Pascal's three offsets share `@CurrentAction`); the text `?` dump renders
//!   `CurrentAction` (Action via `close/open`, Normal/State via `closed/open`) —
//!   the `State` read-function `GetState` is *not* used by the text dump.
//! - `Action`/`Normal`/`State` are `ConditionalReadOnly` on `Locked`: a write
//!   while locked is ignored (so `lock=yes action=open` leaves `Action=close`).
//! - `State=` additionally forces the controlled element to that state at parse
//!   time (`ControlledElement.Closed[0] := …`), deferred here as a
//!   [`RefAction::SetSwitchClosed`](crate::obj::base::RefAction).
//!
//! Concern split mirrors the other controls: this file holds the property
//! metadata, the [`SwtControl`] struct, construction/`recalc`, and the
//! `Sample`/`DoPendingAction`/`Reset` behavior; [`accessors`] holds the
//! [`CktElement`](crate::elements::traits::CktElement)/[`DssObject`](crate::obj::base::DssObject)
//! trait impls.

#[cfg(test)]
mod tests;

mod accessors;

use crate::elements::control::control_elem::{
    CTRL_CLOSE, CTRL_LOCK, CTRL_NONE, CTRL_OPEN, CTRL_UNLOCK, ControlElemData, CtrlCtx, RefSnapshot,
};
use crate::elements::traits::CktElement;
use crate::obj::base::RefAction;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TSwtControlProp` + the `TCktElementClass`
/// tail).
pub mod prop {
    pub const SWITCHED_OBJ: usize = 1;
    pub const SWITCHED_TERM: usize = 2;
    pub const ACTION: usize = 3;
    pub const LOCK: usize = 4;
    pub const DELAY: usize = 5;
    pub const NORMAL: usize = 6;
    pub const STATE: usize = 7;
    pub const RESET: usize = 8;
    /// r4133 informational continuous rating (WP-U2.4). Not used in power flow or
    /// reporting; hidden from the 0.14.5-pinned full-enumeration surfaces.
    pub const RATED_CURRENT: usize = 9;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 10;
    pub const ENABLED: usize = 11;
    pub const NUM_PROPS: usize = 12; // incl. Like
}

/// `TSwtControl.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal `DSSObjectReferenceProperty` (offset2 = 0): any circuit element
        // by full name; the dump renders `Class.name`.
        PropDef::object_ref_any("SwitchedObj"),
        PropDef::integer("SwitchedTerm"),
        // Action/Normal/State all map onto `CurrentAction`; ConditionalReadOnly
        // (Locked) + the JSON-only Redundant/DynamicDefault/NoDefault flags are
        // reproduced behaviorally in the accessors (the engine has no
        // ConditionalReadOnly flag).
        PropDef::mapped_string_enum("Action", enums.swt_control_action).flags(PropFlags::REDUNDANT),
        PropDef::boolean("Lock"),
        PropDef::double("Delay"),
        PropDef::mapped_string_enum("Normal", enums.swt_control_state)
            .flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::mapped_string_enum("State", enums.swt_control_state).flags(PropFlags::NO_DEFAULT),
        // Pascal BooleanActionProperty (DoReset); the getter is always 0.
        PropDef::boolean("Reset"),
        // r4133 `RatedCurrent` (SwtControl.pas props 8->9): informational
        // continuous rating, default 0.0, "Not used internally for either power
        // flow or reporting." WP-U2.5 brought the SwtControl `Dump commands` block
        // to its full r4133 9-prop shape (self-referential golden), so it no longer
        // hides from the full-enum surface; `compare_all_properties` still excludes
        // it from the 0.14.5 property-table walk via the name-based PROPS_015X row.
        PropDef::double("RatedCurrent"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("SwtControl", defs, true)
}

/// `TSwtControlObj`.
#[derive(Debug, Clone)]
pub struct SwtControl {
    pub ccd: ControlElemData,
    /// Dump name of the switched (controlled) element (Pascal `FullName`).
    switched_full_name: String,
    /// Parse-time shape snapshot of the controlled element.
    ctrl_snap: Option<RefSnapshot>,

    /// `PresentState` (CTRL_OPEN / CTRL_CLOSE) — the switch's live position.
    present_state: i32,
    /// `NormalState` (CTRL_NONE until first set) — the reset target.
    normal_state: i32,
    /// `CurrentAction` — the commanded action (the field `Action`/`Normal`/
    /// `State` all write to).
    current_action: i32,
    /// `LockCommand` (CTRL_NONE / CTRL_LOCK / CTRL_UNLOCK) — queued on `Sample`.
    lock_command: i32,
    /// `Locked` — when set, Action/Normal/State are read-only and operations are
    /// blocked.
    locked: bool,
    /// `Armed` — a queue action is outstanding.
    armed: bool,

    /// `RatedCurrent` (r4133) — switch continuous rated current in Amps.
    /// Informational only; not used in power flow or reporting.
    rated_current: f64,

    /// Deferred parse-time element forces (the `State=`/`Reset` side effects).
    pending_ref_actions: Vec<RefAction>,
}

impl SwtControl {
    /// Pascal `TSwtControlObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;
        ccd.time_delay = 120.0; // 2 minutes

        Self {
            ccd,
            switched_full_name: String::new(),
            ctrl_snap: None,
            present_state: CTRL_CLOSE, // default to closed
            normal_state: CTRL_NONE,   // unspecified; set on first action
            current_action: CTRL_CLOSE,
            lock_command: CTRL_NONE,
            locked: false,
            armed: false,
            rated_current: 0.0, // r4133 default
            pending_ref_actions: Vec::new(),
        }
    }

    /// The control's own `FullName` (`SwtControl.<name>`), for the event log.
    fn full_name(&self) -> String {
        format!("SwtControl.{}", self.ccd.cd.obj.name())
    }

    /// Pascal `TSwtControlObj.RecalcElementData`: take the controlled element's
    /// phase count and attach the control's terminal to the switched bus. With
    /// no `SwitchedObj`, Pascal raises error 387 and exits.
    fn recalc(&mut self) {
        let Some(ctrl) = self.ctrl_snap.clone() else {
            self.ccd.cd.obj.push_error(format!(
                "SwtControl: \"{}\": SwitchedObj is not set. Element must be defined previously. (Error 387)",
                self.ccd.cd.obj.name()
            ));
            return;
        };
        self.ccd.cd.nphases = ctrl.nphases;
        self.ccd.cd.set_nconds(ctrl.nphases); // Nconds := FNphases
        // attach controller bus to the switch bus (terminal `ElementTerminal`).
        let t = self.ccd.element_terminal;
        let bus = if t >= 1 && (t as usize) <= ctrl.buses.len() {
            ctrl.buses[(t - 1) as usize].clone()
        } else {
            String::new() // Pascal GetBus(i) out of range yields ''
        };
        self.ccd.cd.set_bus(1, &bus);
    }

    /// Pascal (FPC 0.14.5) `TSwtControlObj.Sample`: push the pending lock command
    /// (if any) and the pending switch action onto the control queue at the current
    /// time delay. Reads only the control's own state — no monitored quantity.
    ///
    /// NOTE (r4133-fidelity gap, out of the r4088→r4133 delta): the Delphi engine
    /// (r4088 *and* r4133) comments out this ENTIRE body — "Removing because action
    /// (redirects to state) and lock are instantaneous" — so on r4133 `Sample`
    /// queues nothing. This port keeps the FPC 0.14.5 body because it is pinned by
    /// the default (capi015 = FPC 0.14.5) oracle: `swtcontrol_lock.dss`
    /// (`compare_ctrlqueue`) verifies the spurious `CTRL_LOCK` push and so cannot
    /// flip to `oracle: "r4133"`. The D6 fix makes the action path inert on r4133's
    /// action/state decks (`current_action == present_state` after an immediate
    /// force ⇒ the action-queue branch is false), but the LOCK branch still queues
    /// a `CTRL_LOCK` that r4133 does not — a latent gap blocking future r4133
    /// lock-path coverage, to be closed when this Sample body is retired.
    pub(crate) fn sample(&mut self, ctx: &mut CtrlCtx) {
        if self.lock_command != CTRL_NONE {
            ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                self.ccd.time_delay,
                self.lock_command,
                0,
                ctx.self_ref,
            );
            self.lock_command = CTRL_NONE; // reset for next time
        }

        if self.current_action != self.present_state && !self.armed {
            // we need to operate this switch
            ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                self.ccd.time_delay,
                self.current_action,
                0,
                ctx.self_ref,
            );
            self.armed = true;
        }
    }

    /// Pascal `TSwtControlObj.DoPendingAction`: execute a popped queue action —
    /// lock/unlock, or (when not locked) open/close all phases of the switched
    /// terminal, logging the operation. `ctrl` is the switched element.
    pub(crate) fn do_pending_action(
        &mut self,
        code: i32,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let term = self.ccd.element_terminal.max(1) as usize;
        // Pascal sets `ControlledElement.ActiveTerminalIdx := ElementTerminal`
        // before the `case` — for *every* code, incl. LOCK/UNLOCK.
        if term <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = term - 1;
        }
        match code {
            CTRL_LOCK => self.locked = true,
            CTRL_UNLOCK => self.locked = false,
            _ => {
                if !self.locked {
                    if code == CTRL_OPEN && self.present_state == CTRL_CLOSE {
                        ctrl.cd_mut().set_terminal_closed(term, false); // open all phases
                        self.present_state = CTRL_OPEN;
                        ctx.events.append(
                            &self.full_name(),
                            "Opened",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                        // Pascal `Closed[]` sets YprimInvalid -> SystemYChanged.
                        *ctx.system_y_changed = true;
                    }
                    if code == CTRL_CLOSE && self.present_state == CTRL_OPEN {
                        ctrl.cd_mut().set_terminal_closed(term, true); // close all phases
                        self.present_state = CTRL_CLOSE;
                        ctx.events.append(
                            &self.full_name(),
                            "Closed",
                            ctx.int_hour,
                            ctx.t,
                            ctx.control_iter,
                        );
                        *ctx.system_y_changed = true;
                    }
                    self.armed = false; // reset the switch
                }
            }
        }
    }

    /// Pascal `TSwtControlObj.Reset` control-side (present/current/armed),
    /// guarded by `not Locked`. `true` when the reset ran (not locked) — the
    /// caller applies the element force.
    pub(crate) fn reset_control_side(&mut self) -> bool {
        if self.locked {
            return false;
        }
        self.present_state = self.normal_state;
        self.current_action = self.present_state;
        self.armed = false;
        true
    }

    /// The state the controlled element is forced to on Reset: Pascal `case
    /// NormalState of CTRL_OPEN: open; else close` — closed for `CTRL_CLOSE`
    /// **and** `CTRL_NONE` (the `else` branch).
    fn reset_target_closed(&self) -> bool {
        self.normal_state != CTRL_OPEN
    }

    /// Pascal `TSwtControlObj.Reset` (full): restore the control state and force
    /// the switched element to `NormalState`. Returns whether a force was applied
    /// (the caller raises `SystemYChanged`).
    ///
    /// The force is **unconditional** when not locked — Pascal's `Closed[0] := …`
    /// raises `SystemYChanged` every time, and gating on an all-or-nothing change
    /// check (`terminal_all_phases_closed`) would miss a real change on a
    /// *partially*-open terminal (one phase closed, the rest open → "all closed"
    /// reads false, so `was == want == open` and the flip of the closed phase
    /// slips past the Y rebuild, leaving a stale system Y). Reset is rare, so the
    /// occasional redundant rebuild is negligible.
    pub(crate) fn reset_with(&mut self, ctrl: &mut dyn CktElement) -> bool {
        if !self.reset_control_side() {
            return false; // locked → no-op
        }
        let term = self.ccd.element_terminal.max(1) as usize;
        ctrl.cd_mut()
            .set_terminal_closed(term, self.reset_target_closed());
        true
    }

    /// Pascal `DoReset` (the `Reset=` boolean-action property): force an unlock,
    /// then run `Reset`. At parse time the controlled-element force is deferred
    /// as a [`RefAction::SetSwitchClosed`].
    fn do_reset_action(&mut self) {
        self.locked = false;
        if self.reset_control_side()
            && let Some(target) = self.ccd.controlled_element
        {
            self.pending_ref_actions.push(RefAction::SetSwitchClosed {
                target,
                terminal: self.ccd.element_terminal as usize,
                closed: self.reset_target_closed(),
            });
        }
    }
}
