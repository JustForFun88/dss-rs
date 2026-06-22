//! Port of `PDElements/fuse.pas` — `TFuseObj`, a **fuse**: a per-phase
//! overcurrent protection control. Despite living in the Pascal `PDElements/`
//! tree it is a `TControlElem` (builds no Yprim, zero terminal current) that
//! monitors one circuit element's terminal currents and, when a phase current
//! stays above the `FuseCurve` (a [`TccCurveObj`]) pickup long enough, **blows
//! that single phase** of the controlled (switched) element. Each phase has its
//! own independent fuse link, arm timer, and queue action.
//!
//! Joins the WP5.7 control sweep with no new dispatch: `Sample` reads the
//! monitored currents, evaluates `GetTCCTime(Cmag / RatedCurrent)` per phase and
//! arms/disarms a per-phase control-queue action at `TripTime + Delay`;
//! `DoPendingAction(phase)` opens that conductor and logs `Phase N Blown`;
//! `Reset` restores every phase to its `Normal` state.
//!
//! **Property quirks settled against the oracle** (probed):
//! - `Normal`/`State` are **per-phase enum arrays** sized by `GetFuseStateSize`
//!   (`= ControlledElement.NPhases`), dumped `[closed, closed, closed, ]`. A
//!   short input sets only the leading phases (the rest keep their prior value).
//! - `State=` forces the controlled element's conductors **per phase** at parse
//!   time (Pascal `Closed[i] := …`), deferred here as a
//!   [`RefAction::SetConductorsClosed`]. `Normal=` does **not** touch the element.
//! - `Action` (deprecated; `close`/`open`) sets *all* phases then runs the same
//!   `State` side effect; its getter always dumps empty.
//! - `MonitoredObj` defaults `SwitchedObj` to the same element; `MonitoredTerm`
//!   defaults `SwitchedTerm`.
//! - `FuseCurve` defaults to the built-in `tlink` curve (resolved by the
//!   executive, mirroring Pascal's constructor `Find('tlink')`).
//!
//! Concern split mirrors the other controls: this file holds the property
//! metadata, the [`Fuse`] struct, construction/`recalc`, and the
//! `Sample`/`DoPendingAction`/`Reset` behavior; [`accessors`] holds the trait
//! impls.

#[cfg(test)]
mod tests;

mod accessors;

use crate::elements::control::control_elem::{
    CTRL_CLOSE, CTRL_OPEN, ControlElemData, CtrlCtx, RefSnapshot,
};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::CktElement;
use crate::obj::base::RefAction;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `FUSEMAXDIM` — the per-phase fuse-link array bound (Pascal `const`).
const FUSEMAXDIM: usize = 6;

/// 1-based property ordinals (Pascal `TFuseProp` + the `TCktElementClass` tail).
pub mod prop {
    pub const MONITORED_OBJ: usize = 1;
    pub const MONITORED_TERM: usize = 2;
    pub const SWITCHED_OBJ: usize = 3;
    pub const SWITCHED_TERM: usize = 4;
    pub const FUSE_CURVE: usize = 5;
    pub const RATED_CURRENT: usize = 6;
    pub const DELAY: usize = 7;
    pub const ACTION: usize = 8;
    pub const NORMAL: usize = 9;
    pub const STATE: usize = 10;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 11;
    pub const ENABLED: usize = 12;
    pub const NUM_PROPS: usize = 13; // incl. Like
}

/// `TFuse.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal `DSSObjectReferenceProperty` (offset2 = 0) + Required: any
        // circuit element by full name; the dump renders `Class.name`.
        PropDef::object_ref_any("MonitoredObj"),
        PropDef::integer("MonitoredTerm"),
        // SwitchedObj is WriteByFunction (`SetControlledElement`) in Pascal; the
        // behavior (store ref + snapshot) is identical to the generic path.
        PropDef::object_ref_any("SwitchedObj"),
        PropDef::integer("SwitchedTerm"),
        PropDef::object_ref_class("TCC_Curve", "FuseCurve"),
        PropDef::double("RatedCurrent"),
        PropDef::double("Delay"),
        // Deprecated StringEnumActionProperty (close/open → DoAction); the getter
        // always dumps empty. (Pascal's `Deprecated` flag is JSON-schema only —
        // the property still parses and appears in the text dump.)
        PropDef::action("Action", enums.fuse_action),
        PropDef::mapped_string_enum_array("Normal", enums.fuse_state)
            .flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::mapped_string_enum_array("State", enums.fuse_state),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("Fuse", defs, true)
}

/// `TFuseObj`.
#[derive(Debug, Clone)]
pub struct Fuse {
    pub ccd: ControlElemData,
    /// Dump name of the monitored element (Pascal `FullName`).
    monitored_full_name: String,
    /// Dump name of the switched (controlled) element (Pascal `FullName`).
    switched_full_name: String,
    /// Parse-time shape snapshots of the two references.
    mon_snap: Option<RefSnapshot>,
    ctrl_snap: Option<RefSnapshot>,

    /// `MonitoredElementTerminal`.
    monitored_element_terminal: i32,
    /// Dump name of the `FuseCurve` (Pascal renders `Name`; default `tlink`).
    fuse_curve_name: String,
    /// The resolved `FuseCurve` clone (snapshot-clone; resolved by the executive
    /// from `fuse_curve_name`, like the Pascal constructor's `Find('tlink')`).
    fuse_curve_obj: Option<TccCurveObj>,
    /// `RatedCurrent` (A).
    rated_current: f64,
    /// `DelayTime` (s) — added to the TCC trip time before queuing.
    delay_time: f64,

    /// `FPresentState[1..FUSEMAXDIM]` — each phase's live link state.
    present_state: [i32; FUSEMAXDIM],
    /// `FNormalState[1..FUSEMAXDIM]` — the reset target per phase.
    normal_state: [i32; FUSEMAXDIM],
    /// `NormalStateSet` — Normal defaults to the first State specified.
    normal_state_set: bool,
    /// `ReadyToBlow[1..FUSEMAXDIM]` — a phase is armed for an open operation.
    ready_to_blow: [bool; FUSEMAXDIM],
    /// `hAction[1..FUSEMAXDIM]` — the per-phase control-queue handle (for
    /// disarming).
    h_action: [i32; FUSEMAXDIM],

    /// Deferred parse-time element forces (the `State=`/`Action=` side effects).
    pending_ref_actions: Vec<RefAction>,
}

impl Fuse {
    /// Pascal `TFuseObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;

        Self {
            ccd,
            monitored_full_name: String::new(),
            switched_full_name: String::new(),
            mon_snap: None,
            ctrl_snap: None,
            monitored_element_terminal: 1,
            // Default fuse link (Pascal `TCC_CurveClass.Find('tlink')`); the
            // executive resolves the clone after edit.
            fuse_curve_name: "tlink".to_string(),
            fuse_curve_obj: None,
            rated_current: 1.0,
            delay_time: 0.0,
            present_state: [CTRL_CLOSE; FUSEMAXDIM],
            normal_state: [CTRL_CLOSE; FUSEMAXDIM], // default to present state
            normal_state_set: false,
            ready_to_blow: [false; FUSEMAXDIM],
            h_action: [0; FUSEMAXDIM],
            pending_ref_actions: Vec::new(),
        }
    }

    /// The control's own `FullName` (`Fuse.<name>`), for the event log.
    fn full_name(&self) -> String {
        format!("Fuse.{}", self.ccd.cd.obj.name())
    }

    /// Pascal `GetFuseStateSize`: `min(FUSEMAXDIM, FNPhases)`, or
    /// `ControlledElement.NPhases` when the controlled element is set. Capped to
    /// `FUSEMAXDIM` so the fixed per-phase arrays never overflow (Pascal warns at
    /// `NPhases > FUSEMAXDIM`).
    fn fuse_state_size(&self) -> usize {
        let n = match &self.ctrl_snap {
            Some(c) => c.nphases,
            None => self.ccd.cd.nphases.min(FUSEMAXDIM),
        };
        n.min(FUSEMAXDIM)
    }

    /// The controlled element's phase count (for the per-conductor force);
    /// `FUSEMAXDIM`-capped like [`Self::fuse_state_size`].
    fn controlled_nphases(&self) -> usize {
        self.ctrl_snap
            .as_ref()
            .map(|c| c.nphases.min(FUSEMAXDIM))
            .unwrap_or(0)
    }

    /// Pascal `PropertySideEffects(State)`'s element force: queue a per-conductor
    /// `SetConductorsClosed` for every controlled phase, derived from
    /// `FPresentState` (`CTRL_OPEN ⇒ open`, else closed).
    fn queue_state_force(&mut self) {
        let Some(target) = self.ccd.controlled_element else {
            return;
        };
        let n = self.controlled_nphases();
        let closed: Vec<bool> = (0..n)
            .map(|i| self.present_state[i] == CTRL_CLOSE)
            .collect();
        self.pending_ref_actions
            .push(RefAction::SetConductorsClosed {
                target,
                terminal: self.ccd.element_terminal.max(1) as usize,
                closed,
            });
    }

    /// Pascal `TFuseObj.PropertySideEffects(State)` (shared by `State=` and the
    /// `Action=`/`DoAction` path): default Normal to the first State, then force
    /// the controlled element to the present per-phase state.
    fn state_side_effect(&mut self) {
        if !self.normal_state_set {
            for i in 0..self.ccd.cd.nphases.min(FUSEMAXDIM) {
                self.normal_state[i] = self.present_state[i];
            }
            // Normal defaults to State only when the 1st State is specified.
            self.normal_state_set = true;
        }
        if self.ccd.controlled_element.is_none() {
            return;
        }
        self.queue_state_force();
    }

    /// Pascal `DoAction(obj, action)`: set **all** phases to `action`'s state,
    /// then run the `State` side effect.
    fn do_fuse_action(&mut self, action: i32) {
        let state = if action == CTRL_OPEN {
            CTRL_OPEN
        } else {
            CTRL_CLOSE
        };
        for s in &mut self.present_state {
            *s = state;
        }
        self.state_side_effect();
    }

    /// Pascal `TFuseObj.RecalcElementData` (parse-time subset): take the phase
    /// count from the monitored element and attach the control's terminal to the
    /// monitored bus. With no monitored/switched element, Pascal raises 404/405;
    /// the per-phase arm/handle reset matches Pascal.
    ///
    /// **Deferred to WP7.2 step 3 (reliability activation):** `Include(
    /// ControlledElement.Flags, Flg.HasOCPDevice)` and the recalc-time
    /// `ControlledElement.Closed[i]` resync (the parse-time `State=` force
    /// already drives the element). Both reach the controlled element, which
    /// `recalc` cannot see here; they land with `GetOCPDeviceType` in step 3.
    fn recalc(&mut self) {
        if let Some(mon) = self.mon_snap.clone() {
            self.ccd.cd.nphases = mon.nphases;
            if self.monitored_element_terminal > mon.nterms as i32 {
                self.ccd.cd.obj.push_error(format!(
                    "Fuse: \"{}\": Terminal no. \"{}\" does not exist. Re-specify terminal no. (Error 404)",
                    self.ccd.cd.obj.name(),
                    self.monitored_element_terminal
                ));
            } else {
                let t = self.monitored_element_terminal;
                let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
                    mon.buses[(t - 1) as usize].clone()
                } else {
                    String::new() // Pascal GetBus(i) out of range yields ''
                };
                self.ccd.cd.set_bus(1, &bus);
            }
        }

        if self.ccd.controlled_element.is_none() {
            // Pascal DoErrorMsg 405.
            self.ccd.cd.obj.push_error(format!(
                "Fuse: \"{}\": CktElement for SwitchedObj is not set. Element must be defined previously. (Error 405)",
                self.ccd.cd.obj.name()
            ));
            return;
        }
        // Disarm every phase (Pascal resets hAction/ReadyToBlow).
        self.h_action = [0; FUSEMAXDIM];
        self.ready_to_blow = [false; FUSEMAXDIM];
    }

    /// Pascal `TFuseObj.Sample`: for each monitored phase, refresh the live link
    /// state from the controlled conductor, evaluate the TCC trip time from the
    /// monitored current, and arm/disarm the per-phase control-queue action.
    /// `ctrl` is the controlled (switched) element, `mon` the monitored element
    /// (the same object for the default fuse — the monitored role only reads).
    pub(crate) fn sample(
        &mut self,
        ctrl: &mut dyn CktElement,
        mon: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        // ControlledElement.ActiveTerminalIdx := ElementTerminal.
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        let mut cbuffer = vec![num_complex::Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);

        let nph = mon.cd().nphases.min(FUSEMAXDIM);
        for i in 1..=nph {
            // Refresh the live state from the controlled conductor.
            self.present_state[i - 1] = if ctrl.cd().conductor_closed(element_terminal, i) {
                CTRL_CLOSE
            } else {
                CTRL_OPEN
            };

            if self.present_state[i - 1] != CTRL_CLOSE {
                continue;
            }

            // Phase trip time (Pascal uses `cBuffer[i]` — terminal-1 currents;
            // the `CondOffset` it computes in RecalcElementData is vestigial).
            let mut trip_time = -1.0;
            if let Some(curve) = self.fuse_curve_obj.as_mut() {
                let cmag = cbuffer[i - 1].norm();
                trip_time = curve.get_tcc_time(cmag / self.rated_current);
            }

            if trip_time > 0.0 {
                if !self.ready_to_blow[i - 1] {
                    // Arm for an open operation (code = the 1-based phase).
                    self.h_action[i - 1] = ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        trip_time + self.delay_time,
                        i as i32,
                        0,
                        ctx.self_ref,
                    );
                    self.ready_to_blow[i - 1] = true;
                }
            } else if self.ready_to_blow[i - 1] {
                // Current dropped below pickup before blowing — disarm.
                ctx.queue.delete(self.h_action[i - 1]);
                self.ready_to_blow[i - 1] = false;
            }
        }
    }

    /// Pascal `TFuseObj.DoPendingAction(Phs, _)`: open the single armed-and-still
    /// -closed phase conductor of the controlled terminal and log `Phase N
    /// Blown`. `phs` is the 1-based phase carried in the queue `code`.
    pub(crate) fn do_pending_action(
        &mut self,
        phs: i32,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        if phs < 1 || phs as usize > FUSEMAXDIM {
            return;
        }
        let p = phs as usize;
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        // ControlledElement.ActiveTerminalIdx := ElementTerminal.
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        // Only act if still closed and still armed (ignore if disarmed since).
        if self.present_state[p - 1] == CTRL_CLOSE && self.ready_to_blow[p - 1] {
            ctrl.cd_mut()
                .set_conductor_closed(element_terminal, p, false); // open this phase
            ctx.events.append(
                &self.full_name(),
                &format!("Phase {p} Blown"),
                ctx.int_hour,
                ctx.t,
                ctx.control_iter,
            );
            self.h_action[p - 1] = 0;
            // Pascal `Closed[]` sets YprimInvalid -> SystemYChanged.
            *ctx.system_y_changed = true;
        }
    }

    /// Pascal `TFuseObj.Reset`: restore every phase to its `Normal` state, forcing
    /// the controlled conductors and disarming. Returns whether a force was
    /// applied (the caller raises `SystemYChanged`).
    ///
    /// The force is **unconditional** per phase — Pascal's `Closed[i] := …`
    /// raises `SystemYChanged` every time; gating on an all-or-nothing aggregate
    /// would miss a real change on a partially-blown terminal (the WP7.2 step-2a
    /// dirty-edge guard). Reset is rare, so a redundant rebuild is negligible.
    pub(crate) fn reset_with(&mut self, ctrl: &mut dyn CktElement) -> bool {
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        let n = ctrl.cd().nphases.min(FUSEMAXDIM);
        if n == 0 {
            return false;
        }
        for i in 1..=n {
            self.present_state[i - 1] = self.normal_state[i - 1];
            self.ready_to_blow[i - 1] = false;
            self.h_action[i - 1] = 0;
            let closed = self.normal_state[i - 1] != CTRL_OPEN;
            ctrl.cd_mut()
                .set_conductor_closed(element_terminal, i, closed);
        }
        true
    }
}
