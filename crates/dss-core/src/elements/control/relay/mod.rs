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
//!   wording, descriptive relay targets (`'Gnd Curve + Ph Curve'` etc.), and a
//!   `'Debug Sample: Relay.<name> FPresentState: […]'` line on every `Sample`
//!   (r4133 forgot the `DebugTrace` guard the Recloser has and its own sibling
//!   traces keep; both lanes write the line under that guard —
//!   `GOLDEN_REBASE_PLAN.md` G2.2d).
//!
//! **The `Normal`/`State` write seam (RP3.7(b), 2026-09-02).** Both properties
//! take the RAW value through `DssObject::set_enum_array_raw` ->
//! `Relay::interpret_relay_state` — the only hook that can carry the outer
//! `Parser.WasQuoted` flag *and* the first-character token match, the two things
//! the generic ordinal tokenizer cannot: r4133 splits ganged-vs-per-phase on
//! `WasQuoted` (`Relay.pas:1256-1306`), so `state=open` fills every slot while
//! `state=(open)` writes phase 1 only, and it honors at most FIVE per-phase
//! tokens (`:1286`) while rendering one per controlled-element phase. `Action`
//! keeps its `StringEnumActionProperty` seam (`Relay::do_action`) with the
//! same guard + ganged fill, and the `NormalState := PresentState` supplemental
//! (`:616-619`, outside `InterpretRelayState`) runs from the property side
//! effects for `Action` and `State` alike — refused-while-locked writes
//! included. The render/drive bound is the **live** controlled-element phase
//! count (`Relay::state_size`), which `make_pos_sequence` now refreshes from
//! the live `PosSeqCtx`, so after `makeposseq` the relay renders `[closed, ]`
//! exactly as r4133's live `ControlledElement.NPhases` loop does. Every byte in
//! this paragraph was measured on the vendored r4133 DLL
//! (`tmp/rp37/out_b1.txt`, `out_b1b.txt`).
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
    ControlAction, ControlElemData, CtrlCtx, RefSnapshot,
};
use crate::elements::general::tcc_curve::TccCurveObj;
use crate::elements::traits::CktElement;
use crate::obj::base::RefAction;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use dss_parser::{Parser, ParserVars};

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

/// Which state property an [`Relay::interpret_relay_state`] write targets — the
/// r4133 guard and the ganged/per-phase split key on the property NAME's first
/// char (`Relay.pas:1244-1246`: `a`ction / `s`tate / `n`ormal). `Action` is not
/// a variant: it never reaches the interpreter in the port (it is a
/// `StringEnumActionProperty`, whose decoded ordinal [`Relay::do_action`]
/// applies with the same guard + ganged fill).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelayStateProp {
    State,
    Normal,
}

/// r4133 `case LowerCase(DataStr2)[1] of 'o': CTRL_OPEN; 'c': CTRL_CLOSE`
/// (`Relay.pas:1249-1251`/`:1263-1265`/`:1271-1273` ganged, `:1289-1300`
/// per-phase) — tokens match on the FIRST CHARACTER ONLY, case-insensitively,
/// and any other first char leaves the slot unchanged (the Pascal `case` has no
/// else arm). The SwtControl twin (`swt_control::match_state_token`) is the same
/// three lines off its own unit; upstream duplicates the `case` per class and so
/// does the port, each citing its own source.
fn match_state_token(token: &str) -> Option<ControlAction> {
    match token.as_bytes().first()?.to_ascii_lowercase() {
        b'o' => Some(ControlAction::Open),
        b'c' => Some(ControlAction::Close),
        _ => None,
    }
}

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
    present_state: [ControlAction; ARR],
    /// `FNormalState[1..RCMAX]` (per phase).
    normal_state: [ControlAction; ARR],
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
            present_state: [ControlAction::Close; ARR],
            normal_state: [ControlAction::Close; ARR],
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

    /// The **render** bound for `Normal`/`State` — [`Self::state_size`] behind
    /// r4133's own nil guard, which lives in the getters and nowhere else:
    /// `GetPropertyValue` 39/40 open with `If ControlledElement <> Nil Then`
    /// (`Relay.pas:1407`/`:1418`) and otherwise leave `Result` at `'['+']'`, so
    /// a relay whose `SwitchedObj` never resolved answers the bare `'[]'`.
    /// Measured on the r4133 DLL 11.0.0.1 (`switchedobj=line.nosuch`, after the
    /// #387 create error): `'[]'` for both properties, where the port used to
    /// print three tokens off its own phase count.
    ///
    /// Scoped to the render on purpose. The same guard is NOT the bound of the
    /// twelve sensing/reset/`MakeLike` loops that also call `state_size` —
    /// r4133 guards those separately (`:1447` `Reset`, `:1494`/`:1514`
    /// `set_States`, all already `Option`-guarded here) and its `Sample` path
    /// dereferences the nil pointer outright (`:1071` `WITH ControlledElement
    /// Do`), so nothing there is a defined observable to port. The residual —
    /// r4133's nil-guarded `Reset` body vs the port's — stays
    /// `ORPHANED_GAPS.md` §1.14(c) (RP3.7 audit settlement, 2026-09-02).
    fn render_size(&self) -> usize {
        if self.ccd.controlled_element.is_none() {
            return 0; // `ControlledElement = NIL` -> `'[]'` (`:1407`/`:1418`)
        }
        self.state_size()
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
    fn set_all_present(&mut self, state: ControlAction) {
        for i in 1..=RCMAX {
            self.present_state[i] = state;
        }
    }

    fn set_all_normal(&mut self, state: ControlAction) {
        for i in 1..=RCMAX {
            self.normal_state[i] = state;
        }
    }

    /// Pascal `Action`'s ganged write (deprecated; Edit arm `19`,
    /// `Relay.pas:552` -> `InterpretRelayState` with `property_name = 'action'`):
    /// the name-based lock guard (`:1244`), then the ganged fill of every slot
    /// `1..RELAYCONTROLMAXDIM` (`:1248-1253`) with the first-character token
    /// match. A [`ControlAction::Keep`] ordinal is the enum's rendering of "the
    /// first char is neither `o` nor `c`", which the Pascal `case` leaves
    /// unwritten (no else arm).
    ///
    /// The **supplemental** (`Relay.pas:616-619`, the `NormalState :=
    /// PresentState` latch) is deliberately NOT here: it sits outside
    /// `InterpretRelayState`, in Edit's `CASE PropertyIdxMap` block, so it runs
    /// after EVERY `Action`/`State` write — the refused-while-locked and
    /// non-matching-token ones included. The port runs it from the property side
    /// effects ([`Relay::state_side_effect`], `accessors::side_effects`), which
    /// the property engine calls unconditionally. Measured on the r4133 DLL
    /// (`tmp/rp37/out_b1.txt` B1(2b), `out_b1b.txt` B1(2e)): after a locked
    /// `action=open` or an unlocked `action=xyz`, a LATER unlocked `state=open`
    /// leaves `Normal` at `[closed, closed, closed, ]` — the latch already
    /// happened. Pinned by
    /// `a_refused_or_unmatched_action_still_runs_the_normal_defaults_supplemental`.
    fn do_action(&mut self, ordinal: i32) {
        if self.f_locked {
            return; // `:1244` — an 'a'ction write is refused while Locked.
        }
        let state = ControlAction::from_ordinal(ordinal);
        if state == ControlAction::Keep {
            return; // First char not 'o'/'c' — Pascal leaves every phase unchanged.
        }
        self.set_all_present(state);
    }

    /// Pascal `TRelayObj.InterpretRelayState` (`Relay.pas:1237-1308`) — the
    /// r4133 write mechanics for `Normal`/`State`, structurally identical to the
    /// SwtControl twin (`SwtControl.pas:410-482`, RP3.7 A2):
    ///
    /// - lock guard (`:1244`): while `Locked`, a write whose property name
    ///   starts with `a` (Action) or `s` (State) exits without touching
    ///   anything; `Normal` (starts with `n`) still applies. The port's Relay
    ///   already carried this rule at the property seam; it moves in here with
    ///   the rest of the mechanics.
    /// - `State`/`Normal`: ganged over slots `1..RELAYCONTROLMAXDIM` when the
    ///   value was NOT quoted (`:1258-1277`), phase-by-phase through the
    ///   AuxParser when it was (`:1278-1305`) — at most FIVE tokens honored
    ///   (`:1286` `While … and (i < RELAYCONTROLMAXDIM)`), unlisted slots
    ///   unchanged.
    /// - tokens match on the first character only ([`match_state_token`]); a
    ///   non-matching token leaves its slot unchanged (the `case` has no else).
    ///
    /// Measured on the r4133 DLL (`tmp/rp37/out_b1.txt` B1(1), `out_b1b.txt`
    /// B1(3b)): on a 3-phase controlled element `state=(open)` renders
    /// `[open, closed, closed, ]` while the bare `state=open` renders
    /// `[open, open, open, ]`, and a six-token quoted list drops its sixth
    /// token. The port's pre-RP3.7 seam read a one-element ordinal list as
    /// ganged — right for the bare spelling, wrong for the quoted single.
    ///
    /// The **two r4133 defects** the SwtControl twin documents are not
    /// reproduced here either: `Relay.pas:1278-1286` carries the identical
    /// `Else`-without-`Begin` fall-through (only `:1280` sits under the `Else`;
    /// the `DataStr` reads `:1282-1283` and the per-phase `While` `:1286` sit in
    /// the enclosing `Begin`, so an unquoted ganged
    /// write re-reads whatever the GLOBAL AuxParser still holds), and the
    /// guard's `:1244` `property_name[1]` read is undefined for a positional token
    /// (Edit leaves `ParamName` empty, `:519-521`). The port scopes the
    /// per-phase loop to the quoted branch with a FRESH parser and keys the
    /// guard on the property identity.
    ///
    /// The slot writes land in the arrays only; the controlled element is driven
    /// by [`Relay::recalc`]'s per-phase force at `EndEdit` (r4133's
    /// `RecalcElementData:965-980` re-drives every phase after `set_States`'
    /// immediate one, so the deferred drive is the last word either way), and
    /// the supplemental (`:616-619`) lives in the property side effects.
    fn interpret_relay_state(&mut self, prop: RelayStateProp, param: &str, was_quoted: bool) {
        // `:1244` — the guard keys on the property NAME's first char.
        if self.f_locked && prop == RelayStateProp::State {
            return;
        }
        if !was_quoted {
            // `:1258-1277`: ganged specification, every slot.
            if let Some(state) = match_state_token(param) {
                match prop {
                    RelayStateProp::State => self.set_all_present(state),
                    RelayStateProp::Normal => self.set_all_normal(state),
                }
            }
            return;
        }
        // `:1278-1305`: phase by phase through the AuxParser.
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        parser.set_auto_increment(false);
        parser.set_cmd_string(param);
        parser.next_param(&vars); // name slot — ignored, Pascal `:1282-1283`
        let mut token = parser.make_string(&vars);
        let mut i = 1usize;
        // `:1286` `While (Length(DataStr2)>0) and (i<RELAYCONTROLMAXDIM)` — a
        // sixth token is silently dropped (measured, `out_b1b.txt` B1(3b)).
        while !token.is_empty() && i < RCMAX {
            if let Some(state) = match_state_token(&token) {
                match prop {
                    RelayStateProp::State => self.present_state[i] = state,
                    RelayStateProp::Normal => self.normal_state[i] = state,
                }
            }
            parser.next_param(&vars);
            token = parser.make_string(&vars);
            i += 1;
        }
    }

    /// Reconstruct Pascal `Parser.WasQuoted` for a value that reached the class
    /// without an outer parser (`PropEngine::was_quoted == false`: JSON import,
    /// the `MakePosSequence` applier, direct unit-test seams). The rule and its
    /// one blind spot are the SwtControl twin's (`SwtControl::
    /// value_implies_quoted`, RP3.7 A2 D5): a value that still carries its
    /// opening bracket/quote, or one holding more than one token, is quoted; a
    /// single bare token resolves to *ganged* — the spelling every corpus deck
    /// writes (`state=open`, `normal=closed`). The distinction that ambiguity
    /// loses (`state=(open)` writes phase 1 only) is carried faithfully wherever
    /// the outer parser runs, i.e. every DSS script.
    fn value_implies_quoted(value: &str) -> bool {
        let v = value.trim_start();
        if matches!(
            v.as_bytes().first(),
            Some(b'(' | b'[' | b'{' | b'"' | b'\'')
        ) {
            return true;
        }
        value
            .split([' ', '\t', '\r', '\n', ','])
            .filter(|t| !t.is_empty())
            .count()
            > 1
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
            if self.normal_state[i] == ControlAction::Open {
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
                .map(|i| self.normal_state[i] == ControlAction::Close)
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
                .map(|i| self.present_state[i] == ControlAction::Close)
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
                device_type: crate::elements::ckt::OcpDeviceType::Relay,
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
                if self.present_state[i] == ControlAction::Close {
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
    /// controlled terminal, emit the `Debug Sample` trace line, then dispatch to
    /// the sub-type sensing logic.
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
                ControlAction::Close
            } else {
                ControlAction::Open
            };
        }
        // r4133 writes this trace line on every Sample with no `if DebugTrace`
        // (`Version8/Source/Controls/Relay.pas:1325`) — the one such line in the
        // class that lost the guard its Recloser twin (`Recloser.pas:1044`) and
        // its own siblings (`Relay.pas:1822`, `:1845`) keep, and one r4088 did
        // not have at all. It is a debug trace, so it goes through `dbg`, which
        // is `if DebugTrace` and (like upstream's line) not gated on
        // `ShowEventLog`.
        {
            let s = self.render_state_array();
            let el = format!("Debug Sample: Relay.{}", self.ccd.cd.obj.name());
            let action = format!("FPresentState: {s} ");
            self.dbg(ctx, &el, &action);
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

    fn render_states(arr: &[ControlAction; ARR], n: usize) -> String {
        let mut s = String::from("[");
        for &v in arr.iter().take(n + 1).skip(1) {
            s.push_str(if v == ControlAction::Open {
                "open"
            } else {
                "closed"
            });
            s.push_str(", ");
        }
        s.push(']');
        s
    }

    /// `if ShowEventLog then AppendToEventLog` — every Relay protection event line
    /// is gated on `ShowEventLog` (the `Debug Sample` lines are gated on
    /// `DebugTrace` instead, in [`Self::dbg`], and never on this flag).
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
        match ControlAction::from_ordinal(code) {
            ControlAction::Open => {
                if self.single_ph_trip {
                    self.do_open_single(ph_idx, nphases, ctrl, ctx);
                } else {
                    self.do_open_ganged(ph_idx, nphases, ctrl, ctx);
                }
            }
            ControlAction::Close => {
                if self.single_ph_trip {
                    if self.present_state[ph_idx] == ControlAction::Open
                        && self.armed_for_close[ph_idx]
                        && !self.locked_out[ph_idx]
                    {
                        ctrl.cd_mut()
                            .set_conductor_closed(element_terminal, ph_idx, true);
                        self.present_state[ph_idx] = ControlAction::Close;
                        let m = format!("Phase {ph_idx} closed (1ph reclosing)");
                        self.log(ctx, &self.full_name(), &m);
                        self.operation_count[ph_idx] += 1;
                        self.armed_for_close[ph_idx] = false;
                        *ctx.system_y_changed = true;
                    }
                } else {
                    for i in 1..=nphases {
                        if self.present_state[i] == ControlAction::Open
                            && self.armed_for_close[ph_idx]
                            && !self.locked_out[i]
                            && !self.locked_out[ph_idx]
                        {
                            ctrl.cd_mut()
                                .set_conductor_closed(element_terminal, i, true);
                            self.present_state[i] = ControlAction::Close;
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
            ControlAction::Reset => {
                // D4: no longer runs the full Reset — only resets OperationCount to
                // 1 for closed phases (+ the TD21 quiet window). Both arms label the
                // event with the class that emitted it: r4133 copied them verbatim
                // from `Recloser.pas:909`/`:924`, class name included
                // (`Relay.pas:1196`, `:1212`), while every other event in this
                // procedure — and both earlier revisions of these two lines (r4088
                // `Relay.pas:971`, 0.14.5 `Relay.pas:1003`) — says `Relay.<name>`.
                let reset_device = format!("Relay.{}", self.ccd.cd.obj.name());
                if self.single_ph_trip {
                    if self.present_state[ph_idx] == ControlAction::Close
                        && !self.armed_for_open[ph_idx]
                    {
                        self.operation_count[ph_idx] = 1;
                        let m = format!("Phase {ph_idx} reset (1ph reset)");
                        self.log(ctx, &reset_device, &m);
                    }
                } else {
                    for i in 1..=nphases {
                        if self.present_state[i] == ControlAction::Close {
                            if !self.armed_for_open[ph_idx] {
                                self.operation_count[ph_idx] = 1;
                                self.log(ctx, &reset_device, "Phase ALL reset (3ph reset)");
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
        if self.present_state[ph_idx] != ControlAction::Close || !self.armed_for_open[ph_idx] {
            return;
        }
        ctrl.cd_mut()
            .set_conductor_closed(element_terminal, ph_idx, false);
        self.present_state[ph_idx] = ControlAction::Open;

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
                        self.present_state[i] = ControlAction::Open;
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
            if self.present_state[i] == ControlAction::Close && self.armed_for_open[ph_idx] {
                ctrl.cd_mut()
                    .set_conductor_closed(element_terminal, i, false);
                self.present_state[i] = ControlAction::Open;
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
            if self.normal_state[i] == ControlAction::Open {
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
