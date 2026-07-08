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
//! - `Generic` — reads a monitored PC element's state `Variable[MonitorVarIndex]`
//!   and trips on an over/under bound (`OverTrip`/`UnderTrip`); the index is
//!   resolved once in `recalc` via `LookupVariable`. See [`logic::Relay::generic_logic`].
//! - `TD21` (WPG.12) — the differential time-distance relay: a per-cycle ring
//!   buffer of terminal V/I (`DynaVars.h`/`Frequency`/`IterationFlag`, now
//!   available) drives an incremental (pre-fault-referenced) distance reach. See
//!   [`logic::Relay::td21_logic`]; the four `TD21RelayTest` corpus decks exercise
//!   it live in dynamics mode.
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
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
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

    // --- TD21 (time-distance) differential ring-buffer state (Pascal `td21_*`) ---
    /// Present ring index into `td21_h` (Pascal `td21_i`, constructor −1).
    td21_i: i32,
    /// Index one cycle back = the oldest sample and next write slot (`td21_next`).
    td21_next: i32,
    /// Number of time samples held (`td21_pt`).
    td21_pt: i32,
    /// Length of one time sample in `td21_h` = `2·Nphases` (`td21_stride`).
    td21_stride: i32,
    /// Wait this many samples after an operation before sensing again (`td21_quiet`).
    td21_quiet: i32,
    /// VI history: `td21_pt` samples × `td21_stride` (V then I, per phase) (`td21_h`).
    td21_h: Vec<Complex64>,
    /// Reference (pre-fault) voltages, per phase (`td21_Uref`).
    td21_uref: Vec<Complex64>,
    /// Incremental voltages, per phase (`td21_dV`).
    td21_dv: Vec<Complex64>,
    /// Incremental currents, per phase (`td21_dI`).
    td21_di: Vec<Complex64>,

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

    // --- Generic (PC state-variable relay) ---
    monitor_variable: String,
    /// `MonitorVarIndex` — the 1-based state-variable index resolved from
    /// `MonitorVariable` against the monitored PC element (Pascal
    /// `RecalcElementData`'s `LookupVariable`); < 1 = unresolved / not found.
    monitor_var_index: i32,
    /// The monitored element's state-variable names (1-based, stored 0-based),
    /// captured when `monitoredobj=` resolves — `recalc` has no foreign-element
    /// access, so `LookupVariable` matches `MonitorVariable` against this cache.
    monitor_var_names: Vec<String>,
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
            // TD21 ring buffer: constructor `td21_i := -1`, everything else 0/NIL;
            // (re)allocated on the first dynamics `Sample` (Pascal `TD21Logic`).
            td21_i: -1,
            td21_next: 0,
            td21_pt: 0,
            td21_stride: 0,
            td21_quiet: 0,
            td21_h: Vec::new(),
            td21_uref: Vec::new(),
            td21_dv: Vec::new(),
            td21_di: Vec::new(),
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
            monitor_var_index: 0,
            monitor_var_names: Vec::new(),
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

    /// Queue the `RecalcElementData` reliability flag: mark the controlled
    /// element as carrying an OCP device (Pascal `Include(ControlledElement
    /// .Flags, Flg.HasOCPDevice/HasAutoOCPDevice)`). Only an enabled relay sets
    /// it; the Relay is an auto-reclosing device, so it sets `HasAutoOCPDevice`
    /// too and reports `GetOCPDeviceType` ordinal 3.
    fn queue_ocp_flag(&mut self) {
        if let Some(target) = self.ccd.controlled_element
            && self.ccd.cd.enabled
        {
            self.pending_ref_actions.push(RefAction::SetOcpDevice {
                target,
                device_type: 3,
                auto: true,
            });
        }
    }

    /// Pascal `TRelayObj.RecalcElementData`: take the phase count + bus from the
    /// monitored element, compute the derived pickups (`PickupAmps46`, `Vbase`,
    /// `PickupVolts47`, the distance impedances), and sync the controlled element
    /// to `FPresentState`. An enabled relay also marks its controlled element
    /// with the `Flg.HasOCPDevice`/`HasAutoOCPDevice` reliability flags
    /// (WP7.2 step 3, via [`Self::queue_ocp_flag`]).
    ///
    /// For a `Generic` relay this also resolves `MonitorVarIndex` from
    /// `MonitorVariable` via [`Self::lookup_variable`] against the monitored PC
    /// element's state-variable names (captured at `monitoredobj=` resolution).
    fn recalc(&mut self) {
        if let Some(mon) = self.mon_snap.clone() {
            self.ccd.cd.nphases = mon.nphases;
            if self.monitored_element_terminal > mon.nterms as i32 {
                // Pascal `DoErrorMsg` 384 (Relay.pas:813) sets
                // `SolutionAbort := True` then falls through (no Exit) — but the
                // bus/CondOffset setup is skipped. We mirror: record + request
                // the abort, skip the bus set, still run the misc block below.
                // (Errors 385/386 below use `DoSimpleMsg` → record-only, no
                // abort.)
                self.ccd.cd.obj.push_error_abort(format!(
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

                // Pascal `RecalcElementData` Generic case: resolve the monitored
                // PC element's state-variable index. Pascal also errors 385 when
                // the monitored element is not a PC element; the port folds that
                // into the 386 not-found path — a non-PC element exposes no
                // variable names, so `LookupVariable` returns < 1 (no corpus deck
                // exercises Generic relays; PC-ness is not available at recalc).
                if self.control_type == ctype::GENERIC {
                    self.monitor_var_index =
                        Self::lookup_variable(&self.monitor_var_names, &self.monitor_variable);
                    if self.monitor_var_index < 1 {
                        self.ccd.cd.obj.push_error(format!(
                            "Relay \"{}\": Monitor variable \"{}\" does not exist. (Error 386)",
                            self.ccd.cd.obj.name(),
                            self.monitor_variable
                        ));
                    }
                }
            }
        }

        // Sync the controlled element to the present state and (when enabled)
        // mark it as an OCP device. Pascal `DoErrorMsg` 387 (Relay.pas:889) sets
        // `SolutionAbort := True` (`DSSGlobals.pas:265`) when no controlled
        // element is set — same abort class as errors 384/388, so request it.
        if self.ccd.controlled_element.is_some() {
            self.queue_ocp_flag();
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
            self.ccd.cd.obj.push_error_abort(format!(
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
    ///
    /// Returns `true` if a sub-type requested a solution abort (only `TD21Logic`'s
    /// coarse-time-step guard, error 388, does — Pascal `DoErrorMsg` →
    /// `SolutionAbort`); the dispatch layer lifts it into `Solution.SolutionAbort`.
    pub(crate) fn sample(
        &mut self,
        ctrl: &mut dyn CktElement,
        mon: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) -> bool {
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
            ctype::GENERIC => self.generic_logic(mon, ctx),
            ctype::DISTANCE => self.distance_logic(mon, ctx),
            // The only sub-type that can request a solution abort (error 388).
            ctype::TD21 => return self.td21_logic(mon, ctx),
            ctype::DOC => self.directional_overcurrent_logic(mon, ctx),
            _ => {}
        }
        false
    }

    /// Pascal `TPCElement.LookupVariable`: return the 1-based index of the first
    /// state-variable name (case-insensitively) *prefixed* by `s` — Pascal
    /// compares `Copy(VariableName(i), 1, Length(S))` against `S` — or −1 if none.
    fn lookup_variable(names: &[String], s: &str) -> i32 {
        let test_len = s.chars().count();
        for (i, name) in names.iter().enumerate() {
            let prefix: String = name.chars().take(test_len).collect();
            if prefix.eq_ignore_ascii_case(s) {
                return (i + 1) as i32;
            }
        }
        -1
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
                    // TD21: wait a full cycle + 1 sample before sensing resumes.
                    if self.control_type == ctype::TD21 {
                        self.td21_quiet = self.td21_pt + 1;
                    }
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
                    // TD21: half a cycle of quiet after a reclose.
                    if self.control_type == ctype::TD21 {
                        self.td21_quiet = self.td21_pt / 2;
                    }
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
                    // TD21: half a cycle of quiet after a reset.
                    if self.control_type == ctype::TD21 {
                        self.td21_quiet = self.td21_pt / 2;
                    }
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
