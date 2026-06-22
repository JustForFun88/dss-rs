//! Port of `Controls/Relay.pas` — `TRelayObj`, the most general protection
//! control: a `TControlElem` that monitors one circuit element's terminal and
//! opens/closes a switch on the same or another terminal. A Relay has nine
//! **sub-types** selected by `Type=` (Pascal `ControlType`), each with its own
//! sensing logic but the same arm/trip/reclose/reset state machine and the same
//! whole-terminal `Closed[0]` force:
//!
//! - `Current` (overcurrent 50/51) — phase + ground TCC pickup, fast inst trip.
//! - `Voltage` (27/59) — definite-time over/under-voltage, voltage reclose.
//! - `ReversePower` (32) — one-shot reverse-power lockout.
//! - `46` (`NegCurrent`) — negative-sequence current, one-shot lockout.
//! - `47` (`NegVoltage`) — negative-sequence voltage, one-shot lockout.
//! - `Distance` (21) — mho-style loop impedance reach.
//! - `DOC` — directional overcurrent (the dominant corpus type).
//! - `Generic` / `TD21` — **deferred to WP7.7**: they consume the dynamics
//!   machinery this phase doesn't have yet (Generic reads a monitored PC
//!   element's state `Variable[]`; TD21 needs `DynaVars.h`/`Frequency`/
//!   `IterationFlag` + the per-cycle ring buffer). Their property surface is
//!   fully ported (so `Type=Generic`/`TD21` parses + dumps); only the `Sample`
//!   logic records a `NOT_PORTED` error if actually reached. See the dispatch in
//!   [`Self::sample`].
//!
//! Joins the WP5.7 control sweep exactly like the Recloser (no new dispatch):
//! `Sample` reads `Closed[0]` to refresh `FPresentState`, then runs the
//! sub-type logic which arms/queues `OPEN`/`CLOSE`/`RESET`; `DoPendingAction`
//! flips the whole controlled terminal; `Reset` restores the `Normal` state.
//!
//! **The dirty-edge rule (WP7.2 step-2a, verified against Pascal + oracle):**
//! every `Closed[0] := …` in Pascal raises `SystemYChanged` **unconditionally**
//! (`CktElement.pas:287` → `:240`), so each force here sets
//! `*ctx.system_y_changed = true` unconditionally — never gated on a
//! terminal-aggregate — and a partial-open fail-on-regression test ships in
//! [`tests`].
//!
//! Concern split mirrors the Recloser: this file holds the property metadata,
//! the [`Relay`] struct, construction/`recalc`, and the `Sample` dispatch +
//! `DoPendingAction`/`Reset`; [`logic`] holds the per-sub-type sensing
//! functions; [`accessors`] holds the trait impls.

#[cfg(test)]
mod tests;

mod accessors;
mod logic;

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

/// Pascal `MIN_DISTANCE_REACTANCE = -1.0e-8` — allows near-bolted faults to be
/// detected by the Distance characteristic.
const MIN_DISTANCE_REACTANCE: f64 = -1.0e-8;

/// `ControlType` ordinals (Pascal `Relay.pas` consts; the `RelayTypeEnum` maps
/// the `Type=` spellings onto them — note the `2` ordinal is unused upstream).
pub mod ctype {
    pub const CURRENT: i32 = 0;
    pub const VOLTAGE: i32 = 1;
    pub const REVPOWER: i32 = 3;
    pub const NEGCURRENT: i32 = 4;
    pub const NEGVOLTAGE: i32 = 5;
    pub const GENERIC: i32 = 6;
    pub const DISTANCE: i32 = 7;
    pub const TD21: i32 = 8;
    pub const DOC: i32 = 9;
}

/// 1-based property ordinals (Pascal `TRelayProp` + the `TCktElementClass`
/// tail). Names confirmed against the oracle's `AllPropertyNames`.
pub mod prop {
    pub const MONITORED_OBJ: usize = 1;
    pub const MONITORED_TERM: usize = 2;
    pub const SWITCHED_OBJ: usize = 3;
    pub const SWITCHED_TERM: usize = 4;
    pub const TYP: usize = 5;
    pub const PHASE_CURVE: usize = 6;
    pub const GROUND_CURVE: usize = 7;
    pub const PHASE_TRIP: usize = 8;
    pub const GROUND_TRIP: usize = 9;
    pub const TD_PHASE: usize = 10;
    pub const TD_GROUND: usize = 11;
    pub const PHASE_INST: usize = 12;
    pub const GROUND_INST: usize = 13;
    pub const RESET: usize = 14;
    pub const SHOTS: usize = 15;
    pub const RECLOSE_INTERVALS: usize = 16;
    pub const DELAY: usize = 17;
    pub const OVERVOLT_CURVE: usize = 18;
    pub const UNDERVOLT_CURVE: usize = 19;
    pub const KV_BASE: usize = 20;
    pub const PCT_PICKUP47: usize = 21;
    pub const BASE_AMPS46: usize = 22;
    pub const PCT_PICKUP46: usize = 23;
    pub const ISQT46: usize = 24;
    pub const VARIABLE: usize = 25;
    pub const OVERTRIP: usize = 26;
    pub const UNDERTRIP: usize = 27;
    pub const BREAKER_TIME: usize = 28;
    pub const ACTION: usize = 29;
    pub const Z1MAG: usize = 30;
    pub const Z1ANG: usize = 31;
    pub const Z0MAG: usize = 32;
    pub const Z0ANG: usize = 33;
    pub const MPHASE: usize = 34;
    pub const MGROUND: usize = 35;
    pub const EVENT_LOG: usize = 36;
    pub const DEBUG_TRACE: usize = 37;
    pub const DIST_REVERSE: usize = 38;
    pub const NORMAL: usize = 39;
    pub const STATE: usize = 40;
    pub const DOC_TILT_ANGLE_LOW: usize = 41;
    pub const DOC_TILT_ANGLE_HIGH: usize = 42;
    pub const DOC_TRIP_SETTING_LOW: usize = 43;
    pub const DOC_TRIP_SETTING_HIGH: usize = 44;
    pub const DOC_TRIP_SETTING_MAG: usize = 45;
    pub const DOC_DELAY_INNER: usize = 46;
    pub const DOC_PHASE_CURVE_INNER: usize = 47;
    pub const DOC_PHASE_TRIP_INNER: usize = 48;
    pub const DOC_TD_PHASE_INNER: usize = 49;
    pub const DOC_P1_BLOCKING: usize = 50;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 51;
    pub const ENABLED: usize = 52;
    pub const NUM_PROPS: usize = 53; // incl. Like
}

/// `TRelay.DefineProperties`. Property names match the oracle's
/// `AllPropertyNames` (the `?`-query dump keys) exactly.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        PropDef::object_ref_any("MonitoredObj"),
        PropDef::integer("MonitoredTerm"),
        PropDef::object_ref_any("SwitchedObj"),
        PropDef::integer("SwitchedTerm"),
        PropDef::mapped_string_enum("Type", enums.relay_type),
        PropDef::object_ref_class("TCC_Curve", "PhaseCurve"),
        PropDef::object_ref_class("TCC_Curve", "GroundCurve"),
        PropDef::double("PhaseTrip"),
        PropDef::double("GroundTrip"),
        PropDef::double("TDPhase"),
        PropDef::double("TDGround"),
        PropDef::double("PhaseInst"),
        PropDef::double("GroundInst"), // Pascal Units_A (JSON-only)
        PropDef::double("Reset"),      // Pascal Units_s (JSON-only)
        // Shots aliases NumReclose with a -1 value offset (the Recloser precedent).
        PropDef::integer("Shots")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::VALUE_OFFSET)
            .value_offset(-1.0),
        // Pascal `[AllowNone, ArrayMaxSize, NonNegative]`: AllowNone makes a
        // zero-count dump render `[NONE]` (Type=DOC ⇒ NumReclose 0).
        PropDef::double_v_array_max("RecloseIntervals", RECLOSE_MAX)
            .flags(PropFlags::ARRAY_MAX_SIZE | PropFlags::ALLOW_NONE),
        PropDef::double("Delay").flags(PropFlags::DYNAMIC_DEFAULT), // type sets it
        PropDef::object_ref_class("TCC_Curve", "OvervoltCurve"),
        PropDef::object_ref_class("TCC_Curve", "UndervoltCurve"),
        PropDef::double("kVBase"),
        PropDef::double("47%Pickup"),
        PropDef::double("46BaseAmps"),
        PropDef::double("46%Pickup"),
        PropDef::double("46isqt"),
        PropDef::string("Variable"),
        PropDef::double("Overtrip"),
        PropDef::double("Undertrip"),
        PropDef::double("BreakerTime"),
        // Action: MappedStringEnum onto FPresentState (Redundant with State).
        PropDef::mapped_string_enum("Action", enums.relay_action).flags(PropFlags::REDUNDANT),
        PropDef::double("Z1Mag"),
        PropDef::double("Z1Ang"),
        PropDef::double("Z0Mag"),
        PropDef::double("Z0Ang"),
        PropDef::double("MPhase"),
        PropDef::double("MGround"),
        PropDef::boolean("EventLog"),
        PropDef::boolean("DebugTrace"),
        PropDef::boolean("DistReverse"),
        PropDef::mapped_string_enum("Normal", enums.relay_state).flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::mapped_string_enum("State", enums.relay_state),
        PropDef::double("DOC_TiltAngleLow"),
        PropDef::double("DOC_TiltAngleHigh"),
        PropDef::double("DOC_TripSettingLow"),
        PropDef::double("DOC_TripSettingHigh"),
        PropDef::double("DOC_TripSettingMag"),
        PropDef::double("DOC_DelayInner"),
        PropDef::object_ref_class("TCC_Curve", "DOC_PhaseCurveInner"),
        PropDef::double("DOC_PhaseTripInner"),
        PropDef::double("DOC_TDPhaseInner"),
        PropDef::boolean("DOC_P1Blocking"),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("Relay", defs, true)
}

/// `TRelayObj`.
#[derive(Debug, Clone)]
pub struct Relay {
    pub ccd: ControlElemData,
    /// Dump names of the monitored / switched (controlled) elements.
    monitored_full_name: String,
    switched_full_name: String,
    /// Parse-time shape snapshots of the two references.
    mon_snap: Option<RefSnapshot>,
    ctrl_snap: Option<RefSnapshot>,
    /// `MonitoredElementTerminal`.
    monitored_element_terminal: i32,

    /// `ControlType` (see [`ctype`]).
    control_type: i32,

    // --- TCC curves (overcurrent / voltage / DOC inner): dump name + clone ---
    phase_curve_name: String,
    ground_curve_name: String,
    ov_curve_name: String,
    uv_curve_name: String,
    doc_phase_curve_inner_name: String,
    phase_curve: Option<TccCurveObj>,
    ground_curve: Option<TccCurveObj>,
    ov_curve: Option<TccCurveObj>,
    uv_curve: Option<TccCurveObj>,
    doc_phase_curve_inner: Option<TccCurveObj>,

    // --- Overcurrent ---
    phase_trip: f64,
    ground_trip: f64,
    phase_inst: f64,
    ground_inst: f64,
    td_phase: f64,
    td_ground: f64,

    // --- Reclose / timing ---
    reset_time: f64,
    delay_time: f64,
    breaker_time: f64,
    num_reclose: i32,
    reclose_intervals: [f64; RECLOSE_MAX],
    /// `RelayTarget` — the trip-cause string logged on open.
    relay_target: String,

    // --- Voltage relay ---
    kv_base: f64,
    /// `Vbase` (line-neutral volts), computed in `recalc`.
    vbase: f64,

    // --- 46 (neg-seq current) ---
    pickup_amps46: f64,
    pct_pickup46: f64,
    base_amps46: f64,
    isqt46: f64,

    // --- 47 (neg-seq voltage) ---
    pickup_volts47: f64,
    pct_pickup47: f64,

    // --- Distance ---
    z1mag: f64,
    z1ang: f64,
    z0mag: f64,
    z0ang: f64,
    mphase: f64,
    mground: f64,
    dist_z1: Complex64,
    dist_z0: Complex64,
    dist_k0: Complex64,
    dist_reverse: bool,

    // --- Directional overcurrent (DOC) ---
    doc_tilt_angle_low: f64,
    doc_tilt_angle_high: f64,
    doc_trip_set_low: f64,
    doc_trip_set_high: f64,
    doc_trip_set_mag: f64,
    doc_delay_inner: f64,
    doc_phase_trip_inner: f64,
    doc_td_phase_inner: f64,
    doc_p1_blocking: bool,

    // --- Generic (logic deferred to WP7.7) ---
    monitor_variable: String,
    over_trip: f64,
    under_trip: f64,

    // --- Present/normal state ---
    present_state: i32,
    normal_state: i32,
    normal_state_set: bool,

    // --- Operation state machine ---
    operation_count: i32,
    locked_out: bool,
    armed_for_open: bool,
    armed_for_close: bool,
    armed_for_reset: bool,
    phase_target: bool,
    ground_target: bool,
    next_trip_time: f64,
    /// `LastEventHandle` — the queue handle of the last pushed action, used by
    /// the Voltage/Distance relays to `Delete` a superseded trip event.
    last_event_handle: i32,

    /// `DebugTrace` (no trace file is written; round-tripped only).
    debug_trace: bool,
    /// Latch so the deferred Generic/TD21 `NOT_PORTED` error is recorded only
    /// once per object (not once per control iteration).
    not_ported_logged: bool,

    /// Deferred parse-time element forces (the `RecalcElementData` Closed[0] sync).
    pending_ref_actions: Vec<RefAction>,
}

impl Relay {
    /// Pascal `TRelayObj.Create`.
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
            control_type: ctype::CURRENT, // RelayTypeEnum DefaultValue = 0
            phase_curve_name: String::new(),
            ground_curve_name: String::new(),
            ov_curve_name: String::new(),
            uv_curve_name: String::new(),
            doc_phase_curve_inner_name: String::new(),
            phase_curve: None,
            ground_curve: None,
            ov_curve: None,
            uv_curve: None,
            doc_phase_curve_inner: None,
            phase_trip: 1.0,
            ground_trip: 1.0,
            phase_inst: 0.0,
            ground_inst: 0.0,
            td_phase: 1.0,
            td_ground: 1.0,
            reset_time: 15.0,
            delay_time: 0.0,
            breaker_time: 0.0,
            num_reclose: 3, // Shots default 4 ⇒ NumReclose 3
            // Pascal sets [1..3] and leaves [4] uninitialized; the dump renders
            // NumReclose of them, and `Sample` only reads `[OperationCount-1]`
            // for `OperationCount ≤ NumReclose ≤ 3`, so the 4th slot is dead.
            reclose_intervals: [0.5, 2.0, 2.0, 0.0],
            relay_target: String::new(),
            kv_base: 0.0,
            vbase: 0.0,
            pickup_amps46: 100.0 * 20.0 * 0.01, // BaseAmps46 * PctPickup46 * 0.01
            pct_pickup46: 20.0,
            base_amps46: 100.0,
            isqt46: 1.0,
            pickup_volts47: 0.0,
            pct_pickup47: 2.0,
            z1mag: 0.7,
            z1ang: 64.0,
            z0mag: 2.1,
            z0ang: 68.0,
            mphase: 0.7,
            mground: 0.7,
            dist_z1: Complex64::ZERO,
            dist_z0: Complex64::ZERO,
            dist_k0: Complex64::ZERO,
            dist_reverse: false,
            doc_tilt_angle_low: 90.0,
            doc_tilt_angle_high: 90.0,
            doc_trip_set_low: 0.0,
            doc_trip_set_high: -1.0,
            doc_trip_set_mag: -1.0,
            doc_delay_inner: -1.0,
            doc_phase_trip_inner: 1.0,
            doc_td_phase_inner: 1.0,
            doc_p1_blocking: true,
            monitor_variable: String::new(),
            over_trip: 1.2,
            under_trip: 0.8,
            present_state: CTRL_CLOSE,
            normal_state: CTRL_CLOSE,
            normal_state_set: false,
            operation_count: 1,
            locked_out: false,
            armed_for_open: false,
            armed_for_close: false,
            armed_for_reset: false,
            phase_target: false,
            ground_target: false,
            next_trip_time: -1.0,
            last_event_handle: 0,
            debug_trace: false,
            not_ported_logged: false,
            pending_ref_actions: Vec::new(),
        }
    }

    /// The control's own `FullName` (`Relay.<name>`), for the event log.
    fn full_name(&self) -> String {
        format!("Relay.{}", self.ccd.cd.obj.name())
    }

    /// Pascal `TRelayObj.PropertySideEffects(Action|State)`: default `NormalState`
    /// to the first `State`/`Action` written (`NormalStateSet`).
    fn state_side_effect(&mut self) {
        if !self.normal_state_set {
            self.normal_state_set = true;
            self.normal_state = self.present_state;
        }
    }

    /// Pascal `TRelayObj.PropertySideEffects(typ)`: per-type definite-time delay
    /// and default reclose intervals. (`SetAsNextSeq` only affects the JSON/Save
    /// ordering, which the per-property `?` dump doesn't use — omitted.)
    fn type_side_effect(&mut self) {
        self.delay_time = match self.control_type {
            ctype::REVPOWER
            | ctype::NEGCURRENT
            | ctype::NEGVOLTAGE
            | ctype::GENERIC
            | ctype::DISTANCE
            | ctype::TD21 => 0.1,
            _ => 0.0, // CURRENT / VOLTAGE / DOC / else
        };
        match self.control_type {
            ctype::CURRENT => {
                self.reclose_intervals[0] = 0.5;
                self.reclose_intervals[1] = 2.0;
                self.reclose_intervals[2] = 2.0;
                self.num_reclose = 3;
            }
            ctype::VOLTAGE => {
                // Pascal sets `RecloseIntervals[3]:=5.0; NumReclose:=1`. With
                // NumReclose 1 the reclose reads `RecloseIntervals[OperationCount]`
                // = `[1]` (the 0.5 constructor default), so this 5.0 is dead
                // unless OperationCount reaches 3 — a faithful upstream quirk
                // (`Relay.pas:573` vs `:2309`); do not "simplify" it away.
                self.reclose_intervals[2] = 5.0;
                self.num_reclose = 1;
            }
            ctype::DOC => {
                self.num_reclose = 0;
            }
            _ => {}
        }
    }

    /// Queue the `RecalcElementData` Closed[0] sync: force the controlled
    /// element's whole terminal to match `FPresentState` (Pascal
    /// `ControlledElement.Closed[0] := …`), deferred as a [`RefAction`].
    fn queue_switch_force(&mut self, closed: bool) {
        if let Some(target) = self.ccd.controlled_element {
            self.pending_ref_actions.push(RefAction::SetSwitchClosed {
                target,
                terminal: self.ccd.element_terminal.max(1) as usize,
                closed,
            });
        }
    }

    /// Pascal `TRelayObj.RecalcElementData`: take the phase count + bus from the
    /// monitored element, compute the derived pickups (`PickupAmps46`, `Vbase`,
    /// `PickupVolts47`, the distance impedances), and sync the controlled element
    /// to `FPresentState`.
    ///
    /// **Deferred to WP7.2 step 3 (reliability):** the `Flg.HasOCPDevice`/
    /// `HasAutoOCPDevice` includes (they reach the controlled element). **Deferred
    /// to WP7.7:** the `Generic` `LookupVariable` resolution (the Generic logic
    /// itself is deferred).
    fn recalc(&mut self) {
        if let Some(mon) = self.mon_snap.clone() {
            self.ccd.cd.nphases = mon.nphases;
            if self.monitored_element_terminal > mon.nterms as i32 {
                // Pascal DoErrorMsg 384 then falls through (no Exit) — but the
                // bus/CondOffset setup is skipped. We mirror: record + skip the
                // bus set, still run the misc derived-value block below.
                self.ccd.cd.obj.push_error(format!(
                    "Relay: \"{}\": Terminal no. \"{}\" does not exist. Re-specify terminal no. (Error 384)",
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

        // Sync the controlled element to the present state (OCP flags deferred to
        // step 3). Pascal errors 387 if no controlled element is set.
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
        } else {
            self.ccd.cd.obj.push_error(format!(
                "Relay: \"{}\": CktElement for SwitchedObj is not set. Element must be defined previously. (Error 387)",
                self.ccd.cd.obj.name()
            ));
        }

        // Misc derived values.
        self.pickup_amps46 = self.base_amps46 * self.pct_pickup46 * 0.01;
        self.vbase = if self.ccd.cd.nphases == 1 {
            self.kv_base * 1000.0
        } else {
            self.kv_base / crate::util::sqrt3() * 1000.0
        };
        self.pickup_volts47 = self.vbase * self.pct_pickup47 * 0.01;

        if self.control_type == ctype::DISTANCE || self.control_type == ctype::TD21 {
            // Pascal `pclx(Mag, Ang / RadiansToDegrees)` — degrees → radians by
            // the full-precision Math constant; `to_radians()` is identical and
            // un-pinned (no Distance corpus case yet).
            self.dist_z1 = crate::support::complexutil::pclx(self.z1mag, self.z1ang.to_radians());
            self.dist_z0 = crate::support::complexutil::pclx(self.z0mag, self.z0ang.to_radians());
            self.dist_k0 = ((self.dist_z0 - self.dist_z1) / 3.0) / self.dist_z1;
        }
    }

    /// Pascal `TRelayObj.Sample`: refresh the live state from the controlled
    /// terminal, then dispatch to the sub-type sensing logic. `ctrl` is the
    /// controlled element, `mon` the monitored element (often the same object).
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

        match self.control_type {
            ctype::CURRENT => self.overcurrent_logic(mon, ctx),
            ctype::VOLTAGE => self.voltage_logic(mon, ctx),
            ctype::REVPOWER => self.rev_power_logic(mon, ctx),
            ctype::NEGCURRENT => self.neg_seq46_logic(mon, ctx),
            ctype::NEGVOLTAGE => self.neg_seq47_logic(mon, ctx),
            ctype::DISTANCE => self.distance_logic(mon, ctx),
            ctype::DOC => self.directional_overcurrent_logic(mon, ctx),
            // NOT_PORTED(WP7.7): the dynamics-coupled sub-types. Their property
            // surface parses + dumps; only the live sensing is deferred (no
            // corpus case exercises them). Recorded **once** per object so a
            // converging run doesn't accrue a duplicate line per control
            // iteration (audit-code follow-up).
            ctype::GENERIC | ctype::TD21 => self.record_not_ported_once(ctx),
            _ => {}
        }
    }

    /// Push the deferred-sub-type `NOT_PORTED` error to `ctx.errors`, but only on
    /// the first `Sample` (the `not_ported_logged` latch) — see the dispatch.
    fn record_not_ported_once(&mut self, ctx: &mut CtrlCtx) {
        if self.not_ported_logged {
            return;
        }
        self.not_ported_logged = true;
        let what = if self.control_type == ctype::GENERIC {
            "Type=Generic (needs WP7.7 PC state variables)"
        } else {
            "Type=TD21 (needs WP7.7 dynamics step)"
        };
        ctx.errors.push(format!(
            "Relay \"{}\": {what} Sample is NOT_PORTED.",
            self.ccd.cd.obj.name()
        ));
    }

    /// Pascal `TRelayObj.DoPendingAction`: execute a popped queue action — OPEN
    /// trips (and locks out past the last shot), CLOSE recloses, RESET resets if
    /// armed. `ctrl` is the controlled element.
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
                        let target = self.relay_target.clone();
                        self.log(
                            ctx,
                            &self.full_name(),
                            &format!("Opened on {target} & Locked Out"),
                        );
                    } else {
                        let target = self.relay_target.clone();
                        self.log(ctx, &self.full_name(), &format!("Opened on {target}"));
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
                // Pascal: `if ArmedForClose and not LockedOut` → log "Reset",
                // then run the full `Reset()` procedure (which re-forces the
                // controlled element to NormalState and logs "Resetting").
                if self.armed_for_close && !self.locked_out {
                    self.log(ctx, &self.full_name(), "Reset");
                    self.reset_with(ctrl, ctx);
                }
            }
            _ => {}
        }
    }

    /// `AppendtoEventLog` helper. Unlike the Recloser (which logs
    /// unconditionally), every Relay event-log line is gated on `ShowEventLog`
    /// (`if ShowEventLog then AppendToEventLog(...)`).
    fn log(&self, ctx: &mut CtrlCtx, element: &str, action: &str) {
        if self.ccd.show_event_log {
            ctx.events
                .append(element, action, ctx.int_hour, ctx.t, ctx.control_iter);
        }
    }

    /// Pascal `TRelayObj.Reset` control-side state (`FPresentState`/armed/targets)
    /// restored to `NormalState`. The element force + locked/operation updates
    /// land in [`Self::reset_with`].
    pub(crate) fn reset_control_side(&mut self) {
        self.present_state = self.normal_state;
        self.armed_for_open = false;
        self.armed_for_close = false;
        self.armed_for_reset = false;
        self.phase_target = false;
        self.ground_target = false;
        self.next_trip_time = -1.0;
    }

    /// Pascal `TRelayObj.Reset` (the full procedure): log "Resetting", restore
    /// the control state to `NormalState`, and force the controlled element's
    /// whole terminal to match. Raises `SystemYChanged` (the `Closed[0]` force is
    /// **unconditional** — the WP7.2 step-2a dirty-edge guard).
    pub(crate) fn reset_with(&mut self, ctrl: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        self.log(ctx, &self.full_name(), "Resetting");
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
        *ctx.system_y_changed = true;
    }
}
