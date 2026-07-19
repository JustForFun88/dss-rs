//! Port of `Controls/Relay.pas` (**EPRI OpenDSS r4133**) — `TRelayObj`, the most
//! general protection control: a `TControlElem` that monitors one circuit
//! element's terminal and opens/closes a switch on the same or another terminal.
//! A Relay has nine **sub-types** selected by `Type=` (Pascal `ControlType`),
//! each with its own sensing logic but the same arm/trip/reclose/reset state
//! machine:
//!
//! - `Current` (overcurrent 50/51) — phase + ground TCC pickup, fast inst trip.
//!   The **only** type that supports per-phase single-phase tripping/lockout.
//! - `Voltage` (27/59) — definite-time over/under-voltage, voltage reclose.
//! - `ReversePower` (32) — one-shot reverse-power lockout.
//! - `46`/`47` (`NegCurrent`/`NegVoltage`) — negative-sequence, one-shot lockout.
//! - `Distance` (21) / `TD21` (differential) — mho / incremental loop reach.
//! - `DOC` — directional overcurrent (the dominant corpus type).
//! - `Generic` — reads a monitored PC element's state variable and trips out of
//!   an over/under band.
//!
//! **r4133 rewrite (WP-U2.3, delta_r4088_r4133 rows B1/B2/B4/C1/D3/D4/D7/E2/E3):**
//! - **Per-phase state machine for `type=current`.** `FPresentState`/`FNormalState`
//!   become per-phase arrays (`StateArray[1..RELAYCONTROLMAXDIM=6]`);
//!   `OperationCount`, `LockedOut`, `ArmedForOpen/Close/Reset`, `PhaseTarget`,
//!   `RelayTarget` become per-phase with an extra `IdxMultiPh = NPhases+1` ganged
//!   slot. All non-overcurrent sub-types drive the ganged slot only.
//! - **Single-phase tripping/lockout** (`SinglePhTrip`/`SinglePhLockout`, overcurrent
//!   ONLY — selecting any other type force-disables them): per-phase TCC eval of
//!   `cBuffer^[i+CondOffset]`, the phase index rides the control-queue proxy handle.
//! - **Ganged-path changes (B4):** sampling continues while ≥1 phase is closed;
//!   `MaxOperatingCount` curve selection over non-locked-out phases;
//!   `DoPendingAction` iterates phases honoring previously locked-out phases.
//! - **VoltageLogic (B2):** OV/UV over `Vmax_closed`/`Vmin_closed` (closed phases
//!   only); the reclose voltage check is still all-phase.
//! - **CTRL_RESET semantics (D4):** the popped `CTRL_RESET` action no longer runs
//!   the full `Reset` — it only resets `OperationCount` to 1 for closed phases (+
//!   the TD21 quiet window); a reset no longer forces the element to normal state.
//! - **Inst-trip delay single-count (D3):** inst time is a bare `0.01` (the
//!   `MechanicalDelay` is added once, at the queue push).
//! - **Property table 50 → 71** with deprecated aliases (`PhaseCurve→PhCurve`,
//!   `GroundTrip→OC_GndPickup`, `Delay→DefiniteTimeDelay`,
//!   `Breakertime→MechanicalDelay`, `Variable→Generic_Variable`,
//!   `Overvoltcurve→Voltage_OVCurve`…), plus `SinglePhTrip`/`SinglePhLockout`,
//!   `Lock`/`Reset` actions, `RatedCurrent`/`InterruptingRating`, and the
//!   `Normal`/`State` per-phase arrays.
//! - **Event-log overhaul (E2/E3):** per-phase `'Phase %d opened on %s (…trip) …'`
//!   wording, descriptive relay targets (`'Gnd Curve + Ph Curve'` etc.), and an
//!   **unconditional** `'Debug Sample: Relay.<name> FPresentState: […]'` line on
//!   every `Sample` (r4133 forgot the `DebugTrace` guard the Recloser has — a
//!   deterministic, defined behavior, reproduced with `TODO(compat)`).
//!
//! Concern split mirrors the Recloser: this file holds the property metadata, the
//! [`Relay`] struct, construction/`recalc`, and `Sample`/`DoPendingAction`/`Reset`;
//! [`logic`] holds the per-sub-type sensing functions; [`accessors`] holds the
//! trait impls.

#[cfg(test)]
mod tests;

mod accessors;
mod logic;

use num_complex::Complex64;

use crate::elements::control::control_elem::{
    CTRL_CLOSE, CTRL_OPEN, CTRL_RESET, CTRL_STATE_KEEP, ControlElemData, CtrlCtx, RefSnapshot,
};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::CktElement;
use crate::obj::base::RefAction;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `RecloseIntervals` fixed allocation (Pascal `Reallocmem(…, SizeOf(Double) *
/// 4)`).
const RECLOSE_MAX: usize = 4;

/// `RELAYCONTROLMAXDIM` — the per-phase state-array bound (Pascal `const`).
const RCMAX: usize = 6;

/// Per-phase array length. The arrays are indexed **1-based** (Pascal
/// `Array[1..RELAYCONTROLMAXDIM]`); slot 0 is unused so `arr[i]` lines up with the
/// Pascal phase index. The ganged slot lives at `IdxMultiPh = NPhases+1` (= 4,
/// frozen at the `Create`-time `NPhases=3`), which is `< ARR`. Sizing to `RCMAX +
/// 2` covers per-phase indices `1..=RCMAX` plus that ganged slot.
const ARR: usize = RCMAX + 2;

/// Pascal `MIN_DISTANCE_REACTANCE = -1.0e-8` — allows near-bolted faults to be
/// detected by the Distance characteristic.
const MIN_DISTANCE_REACTANCE: f64 = -1.0e-8;

/// Relay `ControlType` (Pascal `Relay.pas` consts; the `RelayTypeEnum` maps the
/// `Type=` spellings onto them — the `2` ordinal is deliberately unused
/// upstream, so `from_ordinal(2)` is `None`). Discriminants are user-visible and
/// frozen (round-trip through the `DssEnum` registry); `i32` survives only at the
/// property parse/report boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum RelayControlType {
    #[default]
    Current = 0,
    Voltage = 1,
    RevPower = 3,
    NegCurrent = 4,
    NegVoltage = 5,
    Generic = 6,
    Distance = 7,
    Td21 = 8,
    Doc = 9,
}

impl RelayControlType {
    /// The `RelayTypeEnum` ordinal (`Type=`/`?`/dump boundary value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry ordinal; the unused `2` (and any out-of-range
    /// value) yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Current),
            1 => Some(Self::Voltage),
            3 => Some(Self::RevPower),
            4 => Some(Self::NegCurrent),
            5 => Some(Self::NegVoltage),
            6 => Some(Self::Generic),
            7 => Some(Self::Distance),
            8 => Some(Self::Td21),
            9 => Some(Self::Doc),
            _ => None,
        }
    }
}

/// 1-based property ordinals in **display order** (the r4133
/// `TRelay.DefineProperties` `AddProperty` sequence, which is what the `?`/Dump
/// surface and the accessor trait use — the Pascal *internal* index is folded
/// into the deprecated-alias arms that share fields).
pub mod prop {
    pub const MONITORED_OBJ: usize = 1;
    pub const MONITORED_TERM: usize = 2;
    pub const SWITCHED_OBJ: usize = 3;
    pub const SWITCHED_TERM: usize = 4;
    pub const TYP: usize = 5;
    pub const PH_CURVE: usize = 6;
    pub const OC_GND_CURVE: usize = 7;
    pub const PH_PICKUP: usize = 8;
    pub const OC_GND_PICKUP: usize = 9;
    pub const TD_PH: usize = 10;
    pub const OC_TD_GND: usize = 11;
    pub const PH_INST: usize = 12;
    pub const OC_GND_INST: usize = 13;
    pub const RESET_TIME: usize = 14;
    pub const SHOTS: usize = 15;
    pub const RECLOSE_INTERVALS: usize = 16;
    pub const DEFINITE_TIME_DELAY: usize = 17;
    pub const VOLTAGE_OV_CURVE: usize = 18;
    pub const VOLTAGE_UV_CURVE: usize = 19;
    pub const KV_BASE: usize = 20;
    pub const PCT_PICKUP47: usize = 21;
    pub const BASE_AMPS46: usize = 22;
    pub const PCT_PICKUP46: usize = 23;
    pub const ISQT46: usize = 24;
    pub const GENERIC_VARIABLE: usize = 25;
    pub const GENERIC_OVER_TRIP: usize = 26;
    pub const GENERIC_UNDER_TRIP: usize = 27;
    pub const MECHANICAL_DELAY: usize = 28;
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
    pub const SINGLE_PH_TRIP: usize = 51;
    pub const SINGLE_PH_LOCKOUT: usize = 52;
    pub const LOCK: usize = 53;
    pub const RESET_ACTION: usize = 54;
    pub const RATED_CURRENT: usize = 55;
    pub const INTERRUPTING_RATING: usize = 56;
    // Deprecated aliases (share fields with the canonical props above):
    pub const BREAKER_TIME: usize = 57; // -> MECHANICAL_DELAY
    pub const DELAY: usize = 58; // -> DEFINITE_TIME_DELAY
    pub const GROUND_CURVE: usize = 59; // -> OC_GND_CURVE
    pub const GROUND_TRIP: usize = 60; // -> OC_GND_PICKUP
    pub const GROUND_INST: usize = 61; // -> OC_GND_INST
    pub const TD_GROUND: usize = 62; // -> OC_TD_GND
    pub const PHASE_CURVE: usize = 63; // -> PH_CURVE
    pub const PHASE_TRIP: usize = 64; // -> PH_PICKUP
    pub const PHASE_INST: usize = 65; // -> PH_INST
    pub const TD_PHASE: usize = 66; // -> TD_PH
    pub const OVERTRIP: usize = 67; // -> GENERIC_OVER_TRIP
    pub const UNDERTRIP: usize = 68; // -> GENERIC_UNDER_TRIP
    pub const VARIABLE: usize = 69; // -> GENERIC_VARIABLE
    pub const OVERVOLT_CURVE: usize = 70; // -> VOLTAGE_OV_CURVE
    pub const UNDERVOLT_CURVE: usize = 71; // -> VOLTAGE_UV_CURVE
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 72;
    pub const ENABLED: usize = 73;
    pub const NUM_PROPS: usize = 74; // incl. Like
}

/// `TRelay.DefineProperties` (r4133). Property names match the oracle's
/// `AllPropertyNames` (the `?`-query dump keys) exactly, in `AddProperty` order.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::object_ref_any("MonitoredObj").flags(PropFlags::REQUIRED),
        PropDef::integer("MonitoredTerm"),
        PropDef::object_ref_any("SwitchedObj"),
        PropDef::integer("SwitchedTerm"),
        PropDef::mapped_string_enum("Type", enums.relay_type),
        // TCC curves. `GetTccCurve('none')` -> NIL silently; default names `none`.
        PropDef::object_ref_class("TCC_Curve", "PhCurve"),
        PropDef::object_ref_class("TCC_Curve", "OC_GndCurve"),
        PropDef::double("PhPickup"),
        PropDef::double("OC_GndPickup"),
        PropDef::double("TDPh"),
        PropDef::double("OC_TDGnd"),
        PropDef::double("PhInst"),
        PropDef::double("OC_GndInst"),
        PropDef::double("ResetTime"),
        PropDef::integer("Shots")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::VALUE_OFFSET)
            .value_offset(-1.0),
        // Pascal `[AllowNone, ArrayMaxSize, NonNegative]`: AllowNone makes a
        // zero-count dump render `[NONE]` (Type=DOC ⇒ NumReclose 0).
        PropDef::double_v_array_max("RecloseIntervals", RECLOSE_MAX)
            .flags(PropFlags::ARRAY_MAX_SIZE | PropFlags::ALLOW_NONE),
        PropDef::double("DefiniteTimeDelay").flags(PropFlags::DYNAMIC_DEFAULT), // type sets it
        PropDef::object_ref_class("TCC_Curve", "Voltage_OVCurve"),
        PropDef::object_ref_class("TCC_Curve", "Voltage_UVCurve"),
        PropDef::double("kVBase"),
        PropDef::double("47%Pickup"),
        PropDef::double("46BaseAmps"),
        PropDef::double("46%Pickup"),
        PropDef::double("46isqt"),
        PropDef::string("Generic_Variable"),
        PropDef::double("Generic_OverTrip"),
        PropDef::double("Generic_UnderTrip"),
        PropDef::double("MechanicalDelay"),
        // Action: deprecated ganged StringEnumActionProperty (getter dumps empty).
        PropDef::action("Action", enums.relay_action),
        PropDef::double("Z1Mag"),
        PropDef::double("Z1Ang"),
        PropDef::double("Z0Mag"),
        PropDef::double("Z0Ang"),
        PropDef::double("MPhase"),
        PropDef::double("MGround"),
        PropDef::boolean("EventLog"),
        PropDef::boolean("DebugTrace"),
        PropDef::boolean("DistReverse"),
        PropDef::mapped_string_enum_array("Normal", enums.relay_state)
            .flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::mapped_string_enum_array("State", enums.relay_state),
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
        PropDef::boolean("SinglePhTrip"),
        PropDef::boolean("SinglePhLockout"),
        PropDef::boolean("Lock"),
        PropDef::boolean("Reset"),
        PropDef::double("RatedCurrent"),
        PropDef::double("InterruptingRating"),
        // Deprecated aliases (props 57-71):
        PropDef::double("Breakertime"),
        PropDef::double("Delay"),
        PropDef::object_ref_class("TCC_Curve", "GroundCurve"),
        PropDef::double("GroundTrip"),
        PropDef::double("GroundInst"),
        PropDef::double("TDGround"),
        PropDef::object_ref_class("TCC_Curve", "Phasecurve"),
        PropDef::double("PhaseTrip"),
        PropDef::double("PhaseInst"),
        PropDef::double("TDPhase"),
        PropDef::double("overtrip"),
        PropDef::double("undertrip"),
        PropDef::string("Variable"),
        PropDef::object_ref_class("TCC_Curve", "Overvoltcurve"),
        PropDef::object_ref_class("TCC_Curve", "Undervoltcurve"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("Relay", defs, true)
}

/// `TRelayObj` (r4133).
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

    /// `ControlType` (see [`RelayControlType`]).
    control_type: RelayControlType,

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

    // --- Reclose / timing (Pascal DefiniteTimeDelay / MechanicalDelay) ---
    reset_time: f64,
    definite_time_delay: f64,
    mechanical_delay: f64,
    num_reclose: i32,
    reclose_intervals: [f64; RECLOSE_MAX],
    /// `RelayTarget[1..IdxMultiPh]` — the per-slot trip-cause string logged on open.
    relay_target: [String; ARR],

    // --- Informational ratings (not used in the power flow) ---
    rated_current: f64,
    interrupting_rating: f64,

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
    td21_i: i32,
    td21_next: i32,
    td21_pt: i32,
    td21_stride: i32,
    td21_quiet: i32,
    td21_h: Vec<Complex64>,
    td21_uref: Vec<Complex64>,
    td21_dv: Vec<Complex64>,
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
    monitor_var_index: i32,
    monitor_var_names: Vec<String>,
    over_trip: f64,
    under_trip: f64,

    // --- Per-phase state machine (r4133 arrays, 1-based; ganged at IdxMultiPh) ---
    /// `FPresentState[1..RCMAX]` (per phase).
    present_state: [i32; ARR],
    /// `FNormalState[1..RCMAX]` (per phase).
    normal_state: [i32; ARR],
    normal_state_set: bool,
    /// `OperationCount[1..IdxMultiPh]`.
    operation_count: [i32; ARR],
    /// `LockedOut[1..IdxMultiPh]`.
    locked_out: [bool; ARR],
    /// `ArmedForOpen`/`ArmedForClose`/`ArmedForReset[1..IdxMultiPh]`.
    armed_for_open: [bool; ARR],
    armed_for_close: [bool; ARR],
    armed_for_reset: [bool; ARR],
    /// `PhaseTarget[1..IdxMultiPh]`.
    phase_target: [bool; ARR],
    /// `GroundTarget` — scalar.
    ground_target: bool,
    /// `IdxMultiPh = NPhases+1` — the ganged operation slot (frozen at 4).
    idx_multi_ph: usize,
    next_trip_time: f64,
    /// `LastEventHandle` — the queue handle of the last pushed action.
    last_event_handle: i32,

    /// `SinglePhTrip`/`SinglePhLockout` — single-phase operation modes (overcurrent
    /// type only).
    single_ph_trip: bool,
    single_ph_lockout: bool,
    /// `FLocked` — the `Lock` property (blocks manual/internal state changes).
    f_locked: bool,
    /// `DebugTrace` — write extra `Debug Sample:` detail lines to the event log.
    debug_trace: bool,

    /// Deferred parse-time element forces (the `RecalcElementData` Closed[i] sync).
    pending_ref_actions: Vec<RefAction>,
}

impl Relay {
    /// Pascal `TRelayObj.Create` (r4133).
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
            control_type: RelayControlType::Current, // RelayTypeEnum DefaultValue = 0
            phase_curve_name: "none".to_string(),
            ground_curve_name: "none".to_string(),
            ov_curve_name: "none".to_string(),
            uv_curve_name: "none".to_string(),
            doc_phase_curve_inner_name: "none".to_string(),
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
            definite_time_delay: 0.0,
            mechanical_delay: 0.0,
            num_reclose: 3, // Shots default 4 ⇒ NumReclose 3
            reclose_intervals: [0.5, 2.0, 2.0, 0.0],
            relay_target: std::array::from_fn(|_| String::new()),
            rated_current: 0.0,
            interrupting_rating: 0.0,
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
            present_state: [CTRL_CLOSE; ARR],
            normal_state: [CTRL_CLOSE; ARR],
            normal_state_set: false,
            operation_count: [1; ARR],
            locked_out: [false; ARR],
            armed_for_open: [false; ARR],
            armed_for_close: [false; ARR],
            armed_for_reset: [false; ARR],
            phase_target: [false; ARR],
            ground_target: false,
            idx_multi_ph: 3 + 1, // NPhases(=3) + 1, frozen at Create
            next_trip_time: -1.0,
            last_event_handle: 0,
            single_ph_trip: false,
            single_ph_lockout: false,
            f_locked: false,
            debug_trace: false,
            pending_ref_actions: Vec::new(),
        }
    }

    /// The control's own `FullName` (`Relay.<name>`), for the event log.
    fn full_name(&self) -> String {
        format!("Relay.{}", self.ccd.cd.obj.name())
    }

    /// Per-phase state-array count (Pascal `Min(RELAYCONTROLMAXDIM,
    /// ControlledElement.NPhases)` — GetPropertyValue 39/40, VoltageLogic,
    /// Sample, Reset, ...). The per-phase state arrays are dimensioned by the
    /// SWITCHED (controlled) element's phase count, NOT the relay's own
    /// `FNPhases` (which Pascal forces to `MonitoredElement.NPhases`,
    /// RecalcElementData line 906 — used only for `vbase`, `cBuffer` sizing and
    /// `CondOffset`, which the port reads separately via `ccd.cd.nphases` /
    /// `mon_offset`). The two counts coincide for the usual case where the
    /// monitored and switched elements have the same phase count (and when no
    /// `SwitchedObj` is given, `ctrl_snap == mon_snap`); they differ only when
    /// they don't — e.g. 59NRelayDemo, a 1-phase broken-delta PT monitored with
    /// a 3-phase line switched. `ctrl_snap` is `None` only before the reference
    /// is resolved, where the relay's own count is the right fallback.
    fn state_size(&self) -> usize {
        let ctrl_nphases = self
            .ctrl_snap
            .as_ref()
            .map(|s| s.nphases)
            .unwrap_or(self.ccd.cd.nphases);
        RCMAX.min(ctrl_nphases.max(1))
    }

    /// Pascal `Edit` CASE `19,40`: default `NormalState` per phase from
    /// `PresentState` on the first `State`/`Action` write (`NormalStateSet`).
    fn state_side_effect(&mut self) {
        if !self.normal_state_set {
            let n = self.ccd.cd.nphases.max(1); // Pascal `for i := 1 to FNPhases`
            for i in 1..=n.min(RCMAX) {
                self.normal_state[i] = self.present_state[i];
            }
            self.normal_state_set = true;
        }
    }

    /// Pascal `InterpretRelayState` ganged path (unquoted scalar / `Action`): set
    /// **every** phase (`for i := 1 to RELAYCONTROLMAXDIM`). Blocked while `Locked`.
    fn set_all_present(&mut self, state: i32) {
        for i in 1..=RCMAX {
            self.present_state[i] = state;
        }
    }

    fn set_all_normal(&mut self, state: i32) {
        for i in 1..=RCMAX {
            self.normal_state[i] = state;
        }
    }

    /// Pascal `Action`'s ganged `DoAction` (deprecated): set every phase's present
    /// state + the `State` side effect. Blocked while `Locked`.
    fn do_action(&mut self, ordinal: i32) {
        if self.f_locked {
            return; // Pascal `InterpretRelayState`: blocked while Locked.
        }
        if ordinal == CTRL_STATE_KEEP {
            return; // First char not 'o'/'c' — Pascal leaves every phase unchanged.
        }
        self.set_all_present(ordinal);
        self.state_side_effect();
    }

    /// Pascal prop 54 (`Reset=Yes`): clear `Lock`, run `Reset`, and force the
    /// controlled element per phase (deferred at `EndEdit`).
    fn reset_action(&mut self) {
        self.f_locked = false;
        // Pascal `Reset(ActorID)` — but at edit time no controlled element handle
        // is available, so mirror the control-side reset + queue the element force
        // per phase (the dispatch `Reset` op re-forces live if it has the handle).
        self.next_trip_time = -1.0;
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = self.normal_state[i];
            self.armed_for_open[i] = false;
            self.armed_for_close[i] = false;
            self.armed_for_reset[i] = false;
            self.phase_target[i] = false;
            if self.normal_state[i] == CTRL_OPEN {
                self.locked_out[i] = true;
                self.operation_count[i] = self.num_reclose + 1;
            } else {
                self.locked_out[i] = false;
                self.operation_count[i] = 1;
            }
        }
        self.ground_target = false;
        if let Some(target) = self.ccd.controlled_element {
            let closed: Vec<bool> = (1..=n)
                .map(|i| self.normal_state[i] == CTRL_CLOSE)
                .collect();
            self.pending_ref_actions
                .push(RefAction::SetConductorsClosed {
                    target,
                    terminal: self.ccd.element_terminal.max(1) as usize,
                    closed,
                });
        }
    }

    /// Pascal `TRelayObj.PropertySideEffects(typ)` (r4133 oracle-verified): the
    /// per-type definite-time delay, the DOC-only reclose default, and the
    /// SinglePhTrip/Lockout force-disable for any non-overcurrent type.
    ///
    /// **r4133 note (oddie:r4133-verified):** unlike the r4088/0.14.5 form (and the
    /// r4133 *source* Edit CASE 5, which still writes `'[5.0]'` for voltage), the
    /// r4133 engine does NOT apply the current/voltage reclose-interval defaults —
    /// every non-DOC type keeps the constructor `(0.5, 2, 2)`/Shots 4 (probed:
    /// current/voltage/46/47/distance/td21 all report Shots 4, reclose `(0.5,2,2)`).
    /// Only DOC forces `NumReclose 0` (RecloseIntervals `NONE`, Shots 1). The
    /// source-vs-binary divergence on the voltage default is resolved to the oracle
    /// (RUNG2-COMMON: oddie:r4133 is authoritative for this rung).
    fn type_side_effect(&mut self) {
        self.definite_time_delay = match self.control_type {
            RelayControlType::RevPower
            | RelayControlType::NegCurrent
            | RelayControlType::NegVoltage
            | RelayControlType::Generic
            | RelayControlType::Distance
            | RelayControlType::Td21 => 0.1,
            _ => 0.0, // CURRENT / VOLTAGE / DOC / else
        };
        if self.control_type == RelayControlType::Doc {
            self.num_reclose = 0;
        }
        // Pascal `Edit` CASE 5 side effect: any non-overcurrent type disables the
        // single-phase modes (`SinglePhTrip`/`SinglePhLockout := FALSE`).
        if self.control_type != RelayControlType::Current {
            self.single_ph_trip = false;
            self.single_ph_lockout = false;
        }
    }

    /// Queue the per-phase `RecalcElementData` Closed[i] sync (Pascal
    /// `ControlledElement.Closed[i] := …` per phase). Deferred as a [`RefAction`].
    fn queue_switch_force(&mut self) {
        if let Some(target) = self.ccd.controlled_element {
            let n = self.state_size();
            let closed: Vec<bool> = (1..=n)
                .map(|i| self.present_state[i] == CTRL_CLOSE)
                .collect();
            self.pending_ref_actions
                .push(RefAction::SetConductorsClosed {
                    target,
                    terminal: self.ccd.element_terminal.max(1) as usize,
                    closed,
                });
        }
    }

    /// Queue the `RecalcElementData` reliability flag (Pascal
    /// `Include(ControlledElement.Flags, Flg.HasOCPDevice/HasAutoOCPDevice)`). An
    /// enabled relay is an auto-reclosing device; reports `GetOCPDeviceType` ord 3.
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

    /// Pascal `TRelayObj.RecalcElementData` (r4133): take the phase count + bus
    /// from the monitored element, compute the derived pickups/impedances, and
    /// sync the controlled element **per phase** to `FPresentState`.
    fn recalc(&mut self) {
        if let Some(mon) = self.mon_snap.clone() {
            self.ccd.cd.nphases = mon.nphases;
            if self.monitored_element_terminal > mon.nterms as i32 {
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

                if self.control_type == RelayControlType::Generic {
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

        // Sync the controlled element to the present state per phase and (when
        // enabled) mark it as an OCP device.
        if self.ccd.controlled_element.is_some() {
            self.queue_ocp_flag();
            let n = self.state_size();
            for i in 1..=n {
                if self.present_state[i] == CTRL_CLOSE {
                    self.locked_out[i] = false;
                    self.operation_count[i] = 1;
                    self.armed_for_open[i] = false;
                } else {
                    self.locked_out[i] = true;
                    self.operation_count[i] = self.num_reclose + 1;
                    self.armed_for_close[i] = false;
                }
            }
            self.queue_switch_force();
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

        if self.control_type == RelayControlType::Distance
            || self.control_type == RelayControlType::Td21
        {
            self.dist_z1 = crate::support::complexutil::pclx(self.z1mag, self.z1ang.to_radians());
            self.dist_z0 = crate::support::complexutil::pclx(self.z0mag, self.z0ang.to_radians());
            self.dist_k0 = ((self.dist_z0 - self.dist_z1) / 3.0) / self.dist_z1;
        }
    }

    /// Pascal `TRelayObj.Sample` (r4133): resync the live per-phase state from the
    /// controlled terminal, emit the (unconditional) `Debug Sample` line, then
    /// dispatch to the sub-type sensing logic.
    ///
    /// Returns `true` if a sub-type requested a solution abort (only `TD21Logic`'s
    /// coarse-time-step guard, error 388).
    pub(crate) fn sample(
        &mut self,
        ctrl: &mut dyn CktElement,
        mon: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) -> bool {
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        // Resync FPresentState[i] from Closed[i] for i=1..min(6,nphases).
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = if ctrl.cd().conductor_closed(element_terminal, i) {
                CTRL_CLOSE
            } else {
                CTRL_OPEN
            };
        }
        // TODO(compat): r4133 emits this "Debug Sample" line UNCONDITIONALLY on
        // every Sample — it forgot the `if DebugTrace` guard the Recloser has
        // (Relay.pas:1325 vs Recloser.pas). Deterministic and defined, so it is
        // reproduced 1:1; the clean fix (a DebugTrace guard) is deferred to the
        // §6 TODO(compat) wipe. The line is NOT gated on ShowEventLog either.
        {
            let s = self.render_state_array();
            let el = format!("Debug Sample: Relay.{}", self.ccd.cd.obj.name());
            ctx.events.append(
                &el,
                &format!("FPresentState: {s} "),
                ctx.int_hour,
                ctx.t,
                ctx.control_iter,
            );
        }

        match self.control_type {
            RelayControlType::Current => self.overcurrent_logic(mon, ctx),
            RelayControlType::Voltage => self.voltage_logic(mon, ctx),
            RelayControlType::RevPower => self.rev_power_logic(mon, ctx),
            RelayControlType::NegCurrent => self.neg_seq46_logic(mon, ctx),
            RelayControlType::NegVoltage => self.neg_seq47_logic(mon, ctx),
            RelayControlType::Generic => self.generic_logic(mon, ctx),
            RelayControlType::Distance => self.distance_logic(mon, ctx),
            RelayControlType::Td21 => return self.td21_logic(mon, ctx),
            RelayControlType::Doc => self.directional_overcurrent_logic(mon, ctx),
        }
        false
    }

    /// Pascal `TPCElement.LookupVariable`: 1-based index of the first
    /// state-variable name (case-insensitively) *prefixed* by `s`, or −1.
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

    /// Render `FPresentState` as `[closed, closed, closed, ]` — the
    /// `GetPropertyValue(40)` form the `Debug Sample` line uses. Iterates over
    /// [`Self::state_size`] phases (Pascal `ControlledElement.NPhases`).
    fn render_state_array(&self) -> String {
        Self::render_states(&self.present_state, self.state_size())
    }

    fn render_states(arr: &[i32; ARR], n: usize) -> String {
        let mut s = String::from("[");
        for &v in arr.iter().take(n + 1).skip(1) {
            s.push_str(if v == CTRL_OPEN { "open" } else { "closed" });
            s.push_str(", ");
        }
        s.push(']');
        s
    }

    /// `if ShowEventLog then AppendToEventLog` — every Relay protection event line
    /// is gated on `ShowEventLog` (the `Debug Sample` line above is NOT).
    fn log(&self, ctx: &mut CtrlCtx, element: &str, action: &str) {
        if self.ccd.show_event_log {
            ctx.events
                .append(element, action, ctx.int_hour, ctx.t, ctx.control_iter);
        }
    }

    /// `if DebugTrace then AppendToEventLog('Debug ... Relay.<name>', …)` — the
    /// per-sub-type trace lines (gated on `DebugTrace`, NOT `ShowEventLog`).
    fn dbg(&self, ctx: &mut CtrlCtx, element: &str, action: &str) {
        if self.debug_trace {
            ctx.events
                .append(element, action, ctx.int_hour, ctx.t, ctx.control_iter);
        }
    }

    /// Pascal `TRelayObj.DoPendingAction` (r4133). `proxy` carries the phase index
    /// for single-phase trips; the ganged path uses `IdxMultiPh`.
    pub(crate) fn do_pending_action(
        &mut self,
        code: i32,
        proxy: i32,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let ph_idx = if self.single_ph_trip {
            proxy.max(1) as usize
        } else {
            self.idx_multi_ph
        };
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        if self.debug_trace {
            let ph_debug = if self.single_ph_trip {
                ph_idx.to_string()
            } else {
                "ALL".to_string()
            };
            let state = self.render_state_array();
            let el = format!("Relay.{}", self.ccd.cd.obj.name());
            let msg = format!(
                "Debug DoPendingAction Code={code} Phase={ph_debug} State={state} ArmedOpen={} ArmedForClose={} ArmedForReset={} Count={} NumReclose={}",
                bool_to_str(self.armed_for_open[ph_idx]),
                bool_to_str(self.armed_for_close[ph_idx]),
                bool_to_str(self.armed_for_reset[ph_idx]),
                self.operation_count[ph_idx],
                self.num_reclose
            );
            // Pascal logs this via AppendToEventLog gated only on DebugTrace.
            ctx.events
                .append(&el, &msg, ctx.int_hour, ctx.t, ctx.control_iter);
        }
        let nphases = self.state_size();
        match code {
            CTRL_OPEN => {
                if self.single_ph_trip {
                    self.do_open_single(ph_idx, nphases, ctrl, ctx);
                } else {
                    self.do_open_ganged(ph_idx, nphases, ctrl, ctx);
                }
            }
            CTRL_CLOSE => {
                if self.single_ph_trip {
                    if self.present_state[ph_idx] == CTRL_OPEN
                        && self.armed_for_close[ph_idx]
                        && !self.locked_out[ph_idx]
                    {
                        ctrl.cd_mut()
                            .set_conductor_closed(element_terminal, ph_idx, true);
                        self.present_state[ph_idx] = CTRL_CLOSE;
                        let m = format!("Phase {ph_idx} closed (1ph reclosing)");
                        self.log(ctx, &self.full_name(), &m);
                        self.operation_count[ph_idx] += 1;
                        self.armed_for_close[ph_idx] = false;
                        *ctx.system_y_changed = true;
                    }
                } else {
                    for i in 1..=nphases {
                        if self.present_state[i] == CTRL_OPEN
                            && self.armed_for_close[ph_idx]
                            && !self.locked_out[i]
                            && !self.locked_out[ph_idx]
                        {
                            ctrl.cd_mut()
                                .set_conductor_closed(element_terminal, i, true);
                            self.present_state[i] = CTRL_CLOSE;
                            let m = format!("Phase {i} closed (3ph reclosing)");
                            self.log(ctx, &self.full_name(), &m);
                            *ctx.system_y_changed = true;
                        }
                    }
                    self.armed_for_close[ph_idx] = false;
                    self.operation_count[ph_idx] += 1;
                    if self.control_type == RelayControlType::Td21 {
                        self.td21_quiet = self.td21_pt / 2;
                    }
                }
            }
            CTRL_RESET => {
                // D4: no longer runs the full Reset — only resets OperationCount to
                // 1 for closed phases (+ the TD21 quiet window). NB: r4133 logs this
                // event as `Recloser.<name>` (upstream copy-paste bug — deterministic
                // and defined, reproduced with TODO(compat) below).
                if self.single_ph_trip {
                    if self.present_state[ph_idx] == CTRL_CLOSE && !self.armed_for_open[ph_idx] {
                        self.operation_count[ph_idx] = 1;
                        // TODO(compat): r4133 logs the reset as `Recloser.<name>`,
                        // not `Relay.<name>` (Relay.pas:1196 copy-paste from the
                        // Recloser). Deterministic; reproduced. Clean fix deferred.
                        let el = format!("Recloser.{}", self.ccd.cd.obj.name());
                        let m = format!("Phase {ph_idx} reset (1ph reset)");
                        self.log(ctx, &el, &m);
                    }
                } else {
                    for i in 1..=nphases {
                        if self.present_state[i] == CTRL_CLOSE {
                            if !self.armed_for_open[ph_idx] {
                                self.operation_count[ph_idx] = 1;
                                // TODO(compat): logged as `Recloser.<name>` (bug).
                                let el = format!("Recloser.{}", self.ccd.cd.obj.name());
                                self.log(ctx, &el, "Phase ALL reset (3ph reset)");
                            }
                            break; // no need to loop over all closed phases
                        }
                    }
                    if self.armed_for_reset[ph_idx]
                        && !self.locked_out[ph_idx]
                        && self.control_type == RelayControlType::Td21
                    {
                        self.td21_quiet = self.td21_pt / 2;
                    }
                }
            }
            _ => {}
        }
    }

    /// Pascal `DoPendingAction` CTRL_OPEN, single-phase branch.
    fn do_open_single(
        &mut self,
        ph_idx: usize,
        nphases: usize,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if self.present_state[ph_idx] != CTRL_CLOSE || !self.armed_for_open[ph_idx] {
            return;
        }
        ctrl.cd_mut()
            .set_conductor_closed(element_terminal, ph_idx, false);
        self.present_state[ph_idx] = CTRL_OPEN;

        if self.operation_count[ph_idx] > self.num_reclose {
            self.locked_out[ph_idx] = true;
            if self.single_ph_lockout {
                let msg = format!(
                    "Phase {ph_idx} opened on {} (1ph trip) & locked out (1ph lockout)",
                    self.relay_target[ph_idx]
                );
                self.log(ctx, &self.full_name(), &msg);
            } else {
                let msg = format!(
                    "Phase {ph_idx} opened on {} (1ph trip) & locked out (3ph lockout)",
                    self.relay_target[ph_idx]
                );
                self.log(ctx, &self.full_name(), &msg);
                // 3-phase lockout: open every other not-yet-locked-out phase.
                for i in 1..=nphases {
                    if i != ph_idx && !self.locked_out[i] {
                        ctrl.cd_mut()
                            .set_conductor_closed(element_terminal, i, false);
                        self.present_state[i] = CTRL_OPEN;
                        self.locked_out[i] = true;
                        if self.armed_for_open[i] {
                            self.armed_for_open[i] = false;
                        }
                        let m = format!("Phase {i} opened (1ph trip) & locked out (3ph lockout)");
                        self.log(ctx, &self.full_name(), &m);
                    }
                }
            }
        } else {
            let msg = format!(
                "Phase {ph_idx} opened on {} (1ph trip)",
                self.relay_target[ph_idx]
            );
            self.log(ctx, &self.full_name(), &msg);
        }
        self.armed_for_open[ph_idx] = false;
        *ctx.system_y_changed = true;
    }

    /// Pascal `DoPendingAction` CTRL_OPEN, three-phase (ganged) branch.
    fn do_open_ganged(
        &mut self,
        ph_idx: usize,
        nphases: usize,
        ctrl: &mut dyn CktElement,
        ctx: &mut CtrlCtx,
    ) {
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        for i in 1..=nphases {
            if self.present_state[i] == CTRL_CLOSE && self.armed_for_open[ph_idx] {
                ctrl.cd_mut()
                    .set_conductor_closed(element_terminal, i, false);
                self.present_state[i] = CTRL_OPEN;
                if self.operation_count[ph_idx] > self.num_reclose {
                    self.locked_out[ph_idx] = true;
                    let m = format!(
                        "Phase {i} opened on {} (3ph trip) & locked out (3ph lockout)",
                        self.relay_target[ph_idx]
                    );
                    self.log(ctx, &self.full_name(), &m);
                } else {
                    let m = format!(
                        "Phase {i} opened on {} (3ph trip)",
                        self.relay_target[ph_idx]
                    );
                    self.log(ctx, &self.full_name(), &m);
                }
                *ctx.system_y_changed = true;
            }
        }
        self.armed_for_open[ph_idx] = false;
        if self.control_type == RelayControlType::Td21 {
            self.td21_quiet = self.td21_pt + 1;
        }
    }

    /// Pascal `TRelayObj.Reset` control-side state restored to `NormalState` per
    /// phase (no element force / no lock guard here — the caller has that).
    pub(crate) fn reset_control_side(&mut self) {
        self.next_trip_time = -1.0;
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = self.normal_state[i];
            self.armed_for_open[i] = false;
            self.armed_for_close[i] = false;
            self.armed_for_reset[i] = false;
            self.phase_target[i] = false;
        }
        self.ground_target = false;
    }

    /// Pascal `TRelayObj.Reset` (the full procedure, r4133): a `Locked` relay does
    /// NOT reset. Log "Resetting", restore per-phase state, and force the
    /// controlled element's conductors to `NormalState`. Raises `SystemYChanged`.
    pub(crate) fn reset_with(&mut self, ctrl: &mut dyn CktElement, ctx: &mut CtrlCtx) {
        if self.f_locked {
            return;
        }
        self.log(ctx, &self.full_name(), "Resetting");
        self.next_trip_time = -1.0;
        let element_terminal = self.ccd.element_terminal.max(1) as usize;
        if element_terminal <= ctrl.cd().nterms {
            ctrl.cd_mut().active_terminal = element_terminal - 1;
        }
        let n = self.state_size();
        for i in 1..=n {
            self.present_state[i] = self.normal_state[i];
            self.armed_for_open[i] = false;
            self.armed_for_close[i] = false;
            self.armed_for_reset[i] = false;
            self.ground_target = false;
            self.phase_target[i] = false;
            if self.normal_state[i] == CTRL_OPEN {
                ctrl.cd_mut()
                    .set_conductor_closed(element_terminal, i, false);
                self.locked_out[i] = true;
                self.operation_count[i] = self.num_reclose + 1;
            } else {
                ctrl.cd_mut()
                    .set_conductor_closed(element_terminal, i, true);
                self.locked_out[i] = false;
                self.operation_count[i] = 1;
            }
        }
        *ctx.system_y_changed = true;
    }
}

/// Pascal `BoolToStr(b, TRUE)` — the `True`/`False` spelling used by the
/// `DebugTrace` `DoPendingAction` line.
fn bool_to_str(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}
