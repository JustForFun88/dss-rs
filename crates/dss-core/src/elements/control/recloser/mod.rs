//! Port of `Controls/Recloser.pas` — `TRecloserObj`, an **overcurrent recloser**:
//! a `TControlElem` that monitors one PD element's terminal currents and, when a
//! phase or ground current exceeds its `TCC_Curve` pickup, trips the controlled
//! (switched) element's whole terminal open, then recloses it after a configured
//! interval — repeating up to `Shots` operations before locking out. The first
//! `NumFast` operations use the *fast* curves, the rest the *delayed* curves.
//!
//! Joins the WP5.7 control sweep with no new dispatch: `Sample` reads the
//! monitored currents, evaluates `GetTCCTime(Cmag / Trip)` on the
//! ground-sum/per-phase currents, and (when armed) queues an `OPEN` then a
//! `CLOSE` action on the control queue; `DoPendingAction(OPEN/CLOSE/RESET)`
//! flips the controlled terminal and advances/locks/resets the operation count;
//! `Reset` restores the `Normal` state.
//!
//! **Property quirks settled against the oracle** (probed):
//! - `Action`/`State` both map onto `FPresentState` (Action via the
//!   `close`/`open`/`trip` enum, State via `closed`/`open`/`trip`); `Normal` maps
//!   onto `NormalState`. `trip` is an alias for `open`, so an opened recloser
//!   dumps `open`, never `trip`. The text dump reads the *fields* directly (the
//!   `get_PresentState` getter is unused by `?` — Pascal comment).
//! - The first `State`/`Action` write defaults `NormalState` to it
//!   (`NormalStateSet`); `Normal=` only sets `NormalStateSet`.
//! - `Shots` aliases `NumReclose` with a `-1` value offset (`Shots=4 ⇒
//!   NumReclose=3`), and `RecloseIntervals=(…)` *also* sets `NumReclose` to the
//!   supplied count (whichever is written last wins). `RecloseIntervals` is an
//!   `ArrayMaxSize=4` double vector; the dump renders `NumReclose` of them
//!   (`Shots=1 ⇒ NumReclose=0 ⇒ '[]'`).
//! - `MonitoredObj` defaults `SwitchedObj` to the same element; `MonitoredTerm`
//!   defaults `SwitchedTerm`. `PhaseFast`/`PhaseDelayed` default to the built-in
//!   TCC curves `a`/`d`; the ground curves default to NIL.
//! - `RecalcElementData` syncs the controlled element's whole terminal to
//!   `FPresentState` (deferred here as a [`RefAction::SetSwitchClosed`]); the
//!   `Flg.HasOCPDevice`/`HasAutoOCPDevice` reliability flags are **deferred to
//!   WP7.2 step 3** (the Fuse precedent — they reach the controlled element,
//!   which `recalc` cannot, and land with `GetOCPDeviceType`).
//!
//! Concern split mirrors the other controls: this file holds the property
//! metadata, the [`Recloser`] struct, construction/`recalc`, and the
//! `Sample`/`DoPendingAction`/`Reset` behavior; [`accessors`] holds the trait
//! impls.

#[cfg(test)]
mod tests;

mod accessors;

use num_complex::Complex64;

use crate::elements::control::control_elem::{
    CTRL_CLOSE, CTRL_OPEN, ControlElemData, CtrlCtx, RefSnapshot,
};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::CktElement;
use crate::obj::base::RefAction;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `RecloseIntervals` fixed allocation (Pascal `Reallocmem(…, SizeOf(Double) *
/// 4)`).
const RECLOSE_MAX: usize = 4;

/// 1-based property ordinals (Pascal `TRecloserProp` + the `TCktElementClass`
/// tail).
pub mod prop {
    pub const MONITORED_OBJ: usize = 1;
    pub const MONITORED_TERM: usize = 2;
    pub const SWITCHED_OBJ: usize = 3;
    pub const SWITCHED_TERM: usize = 4;
    pub const NUM_FAST: usize = 5;
    pub const PHASE_FAST: usize = 6;
    pub const PHASE_DELAYED: usize = 7;
    pub const GROUND_FAST: usize = 8;
    pub const GROUND_DELAYED: usize = 9;
    pub const PHASE_TRIP: usize = 10;
    pub const GROUND_TRIP: usize = 11;
    pub const PHASE_INST: usize = 12;
    pub const GROUND_INST: usize = 13;
    pub const RESET: usize = 14;
    pub const SHOTS: usize = 15;
    pub const RECLOSE_INTERVALS: usize = 16;
    pub const DELAY: usize = 17;
    pub const ACTION: usize = 18;
    pub const TD_PH_FAST: usize = 19;
    pub const TD_GR_FAST: usize = 20;
    pub const TD_PH_DELAYED: usize = 21;
    pub const TD_GR_DELAYED: usize = 22;
    pub const NORMAL: usize = 23;
    pub const STATE: usize = 24;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 25;
    pub const ENABLED: usize = 26;
    pub const NUM_PROPS: usize = 27; // incl. Like
}

/// `TRecloser.DefineProperties`.
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
        PropDef::integer("NumFast"),
        PropDef::object_ref_class("TCC_Curve", "PhaseFast"),
        PropDef::object_ref_class("TCC_Curve", "PhaseDelayed"),
        PropDef::object_ref_class("TCC_Curve", "GroundFast"),
        PropDef::object_ref_class("TCC_Curve", "GroundDelayed"),
        PropDef::double("PhaseTrip"),
        PropDef::double("GroundTrip"),
        PropDef::double("PhaseInst"),
        PropDef::double("GroundInst"),
        PropDef::double("Reset"), // Pascal Units_s (JSON-only)
        // Shots aliases NumReclose with a -1 value offset; NonNegative+NonZero on
        // the raw input (so `Shots=0` raises) — the integer setter/dump handle it.
        PropDef::integer("Shots")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::VALUE_OFFSET)
            .value_offset(-1.0),
        // ArrayMaxSize=4: reads up to 4 values, sets NumReclose to the count.
        PropDef::double_v_array_max("RecloseIntervals", RECLOSE_MAX),
        PropDef::double("Delay"), // Pascal Units_s (JSON-only)
        // Action: MappedStringEnum onto FPresentState (Redundant with State).
        PropDef::mapped_string_enum("Action", enums.recloser_action).flags(PropFlags::REDUNDANT),
        PropDef::double("TDPhFast"),
        PropDef::double("TDGrFast"),
        PropDef::double("TDPhDelayed"),
        PropDef::double("TDGrDelayed"),
        PropDef::mapped_string_enum("Normal", enums.recloser_state)
            .flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::mapped_string_enum("State", enums.recloser_state),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("Recloser", defs, true)
}

/// `TRecloserObj`.
#[derive(Debug, Clone)]
pub struct Recloser {
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

    /// Dump names of the four TCC curves (default `a`/`d`/NIL/NIL).
    phase_fast_name: String,
    phase_delayed_name: String,
    ground_fast_name: String,
    ground_delayed_name: String,
    /// Resolved curve clones (snapshot-clone; resolved by the executive from the
    /// names, like the Pascal constructor's `Find('a')`/`Find('d')`).
    phase_fast: Option<TccCurveObj>,
    phase_delayed: Option<TccCurveObj>,
    ground_fast: Option<TccCurveObj>,
    ground_delayed: Option<TccCurveObj>,

    /// `NumFast` — operations on the fast curves before switching to delayed.
    num_fast: i32,
    /// `PhaseTrip`/`GroundTrip` pickup divisors.
    phase_trip: f64,
    ground_trip: f64,
    /// `PhaseInst`/`GroundInst` instantaneous-trip thresholds (0 = disabled).
    phase_inst: f64,
    ground_inst: f64,
    /// `Resettime` (s) — the reset delay queued when current drops below pickup.
    reset_time: f64,
    /// `NumReclose` (= `Shots - 1`) — the count of reclose operations.
    num_reclose: i32,
    /// `RecloseIntervals[1..4]` (s) — the per-shot reclose delays.
    reclose_intervals: [f64; RECLOSE_MAX],
    /// `DelayTime` (s) — added to every trip time before queuing.
    delay_time: f64,
    /// Time-dial multipliers for the fast/delayed phase/ground curves.
    td_ph_fast: f64,
    td_gr_fast: f64,
    td_ph_delayed: f64,
    td_gr_delayed: f64,

    /// `FPresentState` (CTRL_OPEN/CTRL_CLOSE) — the recloser's live position.
    present_state: i32,
    /// `NormalState` — the reset target.
    normal_state: i32,
    /// `NormalStateSet` — Normal defaults to the first State specified.
    normal_state_set: bool,

    /// `OperationCount` — 1-based count of the current operation in the sequence.
    operation_count: i32,
    /// `LockedOut` — exhausted the shots; no further reclose.
    locked_out: bool,
    /// `ArmedForOpen`/`ArmedForClose` — a trip/reclose action is outstanding.
    armed_for_open: bool,
    armed_for_close: bool,
    /// `GroundTarget`/`PhaseTarget` — which protection function tripped (logged).
    ground_target: bool,
    phase_target: bool,

    /// Deferred parse-time element forces (the `RecalcElementData` Closed[0] sync).
    pending_ref_actions: Vec<RefAction>,
}

impl Recloser {
    /// Pascal `TRecloserObj.Create`.
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
            // Pascal `TCC_CurveClass.Find('a')` / `Find('d')` (the built-in
            // fast/delayed curves seeded by `CreateDefaultDSSItems`); the ground
            // curves default to NIL. The executive resolves the clones after edit.
            phase_fast_name: "a".to_string(),
            phase_delayed_name: "d".to_string(),
            ground_fast_name: String::new(),
            ground_delayed_name: String::new(),
            phase_fast: None,
            phase_delayed: None,
            ground_fast: None,
            ground_delayed: None,
            num_fast: 1,
            phase_trip: 1.0,
            ground_trip: 1.0,
            phase_inst: 0.0,
            ground_inst: 0.0,
            reset_time: 15.0,
            num_reclose: 3, // Shots default 4 ⇒ NumReclose 3
            // Pascal sets [1..3] and leaves [4] uninitialized. For the default
            // (and any sane `Shots ≤ 4`) the 4th slot is dead — `Sample` reads
            // `reclose_intervals[OperationCount-1]` only for `OperationCount ≤
            // NumReclose ≤ 3`. A larger `Shots` would read it, but Pascal's slot
            // is uninitialized memory there too (the oracle dumps `Nan`), so no
            // golden pins it; `0.0` is the safe defined choice.
            reclose_intervals: [0.5, 2.0, 2.0, 0.0],
            delay_time: 0.0,
            td_ph_fast: 1.0,
            td_gr_fast: 1.0,
            td_ph_delayed: 1.0,
            td_gr_delayed: 1.0,
            present_state: CTRL_CLOSE,
            normal_state: CTRL_CLOSE,
            normal_state_set: false,
            operation_count: 1,
            locked_out: false,
            armed_for_open: false,
            armed_for_close: false,
            ground_target: false,
            phase_target: false,
            pending_ref_actions: Vec::new(),
        }
    }

    /// The control's own `FullName` (`Recloser.<name>`), for the event log.
    fn full_name(&self) -> String {
        format!("Recloser.{}", self.ccd.cd.obj.name())
    }

    /// Pascal `TRecloserObj.PropertySideEffects(Action|State)`: default Normal to
    /// the first State/Action written (`NormalStateSet`).
    fn state_side_effect(&mut self) {
        if !self.normal_state_set {
            self.normal_state_set = true;
            self.normal_state = self.present_state;
        }
    }

    /// Queue the `RecalcElementData` Closed[0] sync: force the controlled
    /// element's whole terminal to match `FPresentState` (Pascal
    /// `ControlledElement.Closed[0] := …`). Deferred as a [`RefAction`] since the
    /// property engine holds no mutable view of the target.
    fn queue_switch_force(&mut self, closed: bool) {
        if let Some(target) = self.ccd.controlled_element {
            self.pending_ref_actions.push(RefAction::SetSwitchClosed {
                target,
                terminal: self.ccd.element_terminal.max(1) as usize,
                closed,
            });
        }
    }

    /// Pascal `TRecloserObj.RecalcElementData`: take the phase count from the
    /// monitored element, attach the control's terminal to the monitored bus, and
    /// sync the controlled element to `FPresentState`. Pascal does **not** error
    /// on NIL elements here (`//TODO`); only `Sample` raises on a NIL monitored
    /// element. A bad monitored terminal raises 392 and aborts the recalc.
    ///
    /// **Deferred to WP7.2 step 3 (reliability activation):** the
    /// `Flg.HasOCPDevice`/`HasAutoOCPDevice` includes — they reach the controlled
    /// element (which `recalc` cannot see) and land with `GetOCPDeviceType`.
    fn recalc(&mut self) {
        if let Some(mon) = self.mon_snap.clone() {
            self.ccd.cd.nphases = mon.nphases;
            if self.monitored_element_terminal > mon.nterms as i32 {
                // Pascal DoErrorMsg 392 then Exit.
                self.ccd.cd.obj.push_error(format!(
                    "Recloser: \"{}\": Terminal no. \"{}\" does not exist. Re-specify terminal no. (Error 392)",
                    self.ccd.cd.obj.name(),
                    self.monitored_element_terminal
                ));
                return;
            }
            let t = self.monitored_element_terminal;
            let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
                mon.buses[(t - 1) as usize].clone()
            } else {
                String::new() // Pascal GetBus(i) out of range yields ''
            };
            self.ccd.cd.set_bus(1, &bus);
        }

        // Sync the controlled element to the present state (the OCP flags are
        // deferred to step 3). Pascal runs this on every recalc when the
        // controlled element is set.
        if self.ccd.controlled_element.is_some() {
            if self.present_state == CTRL_CLOSE {
                self.locked_out = false;
                self.operation_count = 1;
                self.armed_for_open = false;
                self.queue_switch_force(true);
            } else {
                self.locked_out = true;
                self.operation_count = self.num_reclose + 1;
                self.armed_for_close = false;
                self.queue_switch_force(false);
            }
        }
    }

    /// Pascal `TRecloserObj.Sample`: refresh the live state from the controlled
    /// terminal, and (when closed) evaluate the ground-sum and per-phase TCC trip
    /// times on the monitored currents, arming/disarming the open+reclose queue
    /// actions. `ctrl` is the controlled element, `mon` the monitored element
    /// (often the same object — the monitored role only reads solved state).
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
        // FPresentState := Closed[0] ? CTRL_CLOSE : CTRL_OPEN.
        self.present_state = if ctrl.cd().terminal_all_phases_closed(element_terminal) {
            CTRL_CLOSE
        } else {
            CTRL_OPEN
        };

        if self.present_state != CTRL_CLOSE {
            return; // IF PresentState = CLOSE
        }

        // Fast vs delayed curve/time-dial selection by the operation count.
        let use_delayed = self.operation_count > self.num_fast;
        let td_ground = if use_delayed {
            self.td_gr_delayed
        } else {
            self.td_gr_fast
        };
        let td_phase = if use_delayed {
            self.td_ph_delayed
        } else {
            self.td_ph_fast
        };
        // Hoist the scalar reads so the per-curve `&mut` borrows below don't
        // conflict with reading other fields of `self`.
        let (ground_inst, ground_trip) = (self.ground_inst, self.ground_trip);
        let (phase_inst, phase_trip) = (self.phase_inst, self.phase_trip);
        let (delay_time, operation_count) = (self.delay_time, self.operation_count);

        let nphases = mon.cd().nphases;
        let cond_offset = (self.monitored_element_terminal.max(1) as usize - 1) * mon.cd().nconds;
        let mut cbuffer = vec![Complex64::ZERO; mon.cd().yorder.max(1)];
        mon.get_currents(ctx.sys, ctx.node_v, &mut cbuffer);
        let cur = |i: usize| {
            cbuffer
                .get(cond_offset + i)
                .copied()
                .unwrap_or(Complex64::ZERO)
        };

        let mut trip_time = -1.0_f64;

        // Ground trip: the magnitude of the sum of the monitored phase currents.
        let ground_curve = if use_delayed {
            self.ground_delayed.as_mut()
        } else {
            self.ground_fast.as_mut()
        };
        if let Some(gc) = ground_curve {
            let mut csum = Complex64::ZERO;
            for i in 0..nphases {
                csum += cur(i);
            }
            let cmag = csum.norm();
            let ground_time = if ground_inst > 0.0 && cmag >= ground_inst && operation_count == 1 {
                0.01 + delay_time // inst trip on the first operation
            } else {
                td_ground * gc.get_tcc_time(cmag / ground_trip)
            };
            if ground_time > 0.0 {
                trip_time = ground_time;
                self.ground_target = true;
            }
        }

        // Phase trip: the smallest per-phase TCC time (or an inst trip).
        let mut phase_time = -1.0_f64;
        let phase_curve = if use_delayed {
            self.phase_delayed.as_mut()
        } else {
            self.phase_fast.as_mut()
        };
        if let Some(pc) = phase_curve {
            for i in 0..nphases {
                let cmag = cur(i).norm();
                if phase_inst > 0.0 && cmag >= phase_inst && operation_count == 1 {
                    phase_time = 0.01 + delay_time; // inst trip on the first operation
                    break; // no sense checking the other phases
                }
                let time_test = td_phase * pc.get_tcc_time(cmag / phase_trip);
                if time_test > 0.0 {
                    phase_time = if phase_time < 0.0 {
                        time_test
                    } else {
                        phase_time.min(time_test)
                    };
                }
            }
        }
        if phase_time > 0.0 {
            self.phase_target = true;
            trip_time = if trip_time > 0.0 {
                trip_time.min(phase_time)
            } else {
                phase_time
            };
        }

        if trip_time > 0.0 {
            if !self.armed_for_open {
                // Arm for an open, then a reclose (if shots remain).
                ctx.queue.push_delay(
                    ctx.int_hour,
                    ctx.t,
                    trip_time + self.delay_time,
                    CTRL_OPEN,
                    0,
                    ctx.self_ref,
                );
                if self.operation_count <= self.num_reclose {
                    let interval = self
                        .reclose_intervals
                        .get((self.operation_count - 1).max(0) as usize)
                        .copied()
                        .unwrap_or(0.0);
                    ctx.queue.push_delay(
                        ctx.int_hour,
                        ctx.t,
                        trip_time + self.delay_time + interval,
                        CTRL_CLOSE,
                        0,
                        ctx.self_ref,
                    );
                }
                self.armed_for_open = true;
                self.armed_for_close = true;
            }
        } else if self.armed_for_open {
            // Current dropped below pickup before tripping — disarm and reset.
            ctx.queue.push_delay(
                ctx.int_hour,
                ctx.t,
                self.reset_time,
                crate::elements::control::control_elem::CTRL_RESET,
                0,
                ctx.self_ref,
            );
            self.armed_for_open = false;
            self.armed_for_close = false;
            self.ground_target = false;
            self.phase_target = false;
        }
    }

    /// Pascal `TRecloserObj.DoPendingAction`: execute a popped queue action —
    /// OPEN trips (and locks out on the last shot), CLOSE recloses, RESET clears
    /// the operation count. `ctrl` is the controlled element.
    pub(crate) fn do_pending_action(
        &mut self,
        code: i32,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        use crate::elements::control::control_elem::CTRL_RESET;
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        // ControlledElement.ActiveTerminalIdx := ElementTerminal.
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        match code {
            CTRL_OPEN => {
                // ignore if we became disarmed in the meantime
                if self.present_state == CTRL_CLOSE && self.armed_for_open {
                    ctrl.cd_mut().set_terminal_closed(element_terminal, false); // open all phases
                    if self.operation_count > self.num_reclose {
                        self.locked_out = true;
                        self.log(ctx, &self.full_name(), "Opened, Locked Out");
                    } else if self.operation_count > self.num_fast {
                        self.log(ctx, &self.full_name(), "Opened, Delayed");
                    } else {
                        self.log(ctx, &self.full_name(), "Opened, Fast");
                    }
                    if self.phase_target {
                        self.log(ctx, " ", "Phase Target");
                    }
                    if self.ground_target {
                        self.log(ctx, " ", "Ground Target");
                    }
                    self.armed_for_open = false;
                    // Pascal `Closed[]` sets YprimInvalid -> SystemYChanged.
                    *ctx.system_y_changed = true;
                }
            }
            CTRL_CLOSE => {
                if self.present_state == CTRL_OPEN && self.armed_for_close && !self.locked_out {
                    ctrl.cd_mut().set_terminal_closed(element_terminal, true); // close all phases
                    self.operation_count += 1;
                    self.log(ctx, &self.full_name(), "Closed");
                    self.armed_for_close = false;
                    *ctx.system_y_changed = true;
                }
            }
            CTRL_RESET => {
                // Don't reset if we just rearmed.
                if self.present_state == CTRL_CLOSE && !self.armed_for_open {
                    self.operation_count = 1;
                }
            }
            _ => {}
        }
    }

    /// `AppendtoEventLog` helper (the per-action event-log line).
    fn log(&self, ctx: &mut CtrlCtx, element: &str, action: &str) {
        ctx.events
            .append(element, action, ctx.int_hour, ctx.t, ctx.control_iter);
    }

    /// Pascal `TRecloserObj.Reset` control-side state (`FPresentState`/armed/
    /// targets) restored to `NormalState`. The element force + locked/operation
    /// updates land in [`Self::reset_with`] (Pascal's post-`Exit` block).
    pub(crate) fn reset_control_side(&mut self) {
        self.present_state = self.normal_state;
        self.armed_for_open = false;
        self.armed_for_close = false;
        self.ground_target = false;
        self.phase_target = false;
    }

    /// Pascal `TRecloserObj.Reset` (full): restore the control state and force the
    /// controlled element's whole terminal to `NormalState`. Returns whether a
    /// force was applied (the caller raises `SystemYChanged`).
    ///
    /// The force is **unconditional** — Pascal's `Closed[0] := …` raises
    /// `SystemYChanged` every time; gating on an all-or-nothing aggregate would
    /// miss a real change on a partially-open terminal (the WP7.2 step-2a
    /// dirty-edge guard).
    pub(crate) fn reset_with(&mut self, ctrl: &mut dyn CktElement) -> bool {
        self.reset_control_side();
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        if self.normal_state == CTRL_OPEN {
            ctrl.cd_mut().set_terminal_closed(element_terminal, false);
            self.locked_out = true;
            self.operation_count = self.num_reclose + 1;
        } else {
            ctrl.cd_mut().set_terminal_closed(element_terminal, true);
            self.locked_out = false;
            self.operation_count = 1;
        }
        true
    }
}
