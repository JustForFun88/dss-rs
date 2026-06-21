//! Port of `Controls/StorageController.pas` — `TStorageControllerObj`, a control
//! element that dispatches a *fleet* of Storage elements (charge/discharge) to
//! hold a monitored element's power inside a target band.
//!
//! **Skeleton only (PHASE6_PLAN §2.6).** The Storage element itself is Phase 7,
//! so there is no `Storage` class in the engine yet and a StorageController can
//! never resolve a non-empty fleet. This port therefore lands:
//!
//! - the **full property table** (37 class props + the `TCktElementClass` tail),
//!   with the exact ctor defaults, `PropertySideEffects`, and `MakeLike`
//!   value-copying so the `?`/dump round-trip matches the oracle byte-for-byte;
//! - `RecalcElementData`'s parse-time surface: the monitored-element checks
//!   (371/372) and `MakeFleetList`, which — finding no Storage elements — always
//!   reports error 37201 (or 14403 for a named-but-missing element), exactly as
//!   the oracle does on a circuit with no Storage (verified empirically).
//!
//! **Deliberately NOT_PORTED → Phase 7** (everything that needs the Storage
//! element's live state): `Sample` and its `DoLoadFollowMode`/`DoTimeMode`/
//! `DoLoadShapeMode`/`DoScheduleMode`/`DoPeakShaveModeLow` dispatch logic,
//! `SetFleet*`, `GetControlPower`/`GetControlCurrent`, the kWh/kW fleet
//! aggregates, and `MakePosSequence`. Because the fleet is always empty in
//! Phase 6, `Sample`/`DoPendingAction`/`Reset` are inert no-ops here — which is
//! also the observable behavior for the **default** (unspecified) element list:
//! Pascal `Sample` → `DoLoadFollowMode` re-runs `MakeFleetList` (l.1078), whose
//! default branch is silent, then dispatches nothing (`FleetSize = 0`).
//!
//! **Known skeleton divergence** (NOT_PORTED → Phase 7): when the user specifies
//! an `ElementList` of Storage names that do not resolve (always, in Phase 6),
//! Pascal `Sample` re-enters the *named* branch of `MakeFleetList` and emits
//! error 14403 on **every** sample step of a time-series solve. This skeleton's
//! control-sweep entry is a blanket no-op, so it suppresses those per-step
//! errors. The parse-time 14403/37201 (from `RecalcElementData`) are still
//! reproduced; only the per-sample repetition is deferred with the rest of
//! `Sample`. When the Storage element lands, these get real bodies and a
//! control-loop gate.
//!
//! The four fleet-aggregate readbacks (`kWhTotal`/`kWTotal`/`kWhActual`/
//! `kWActual`) are Pascal `SilentReadOnly + ReadByFunction` doubles whose
//! `?`/`GetObjPropertyValue` getter renders `''` regardless of fleet contents
//! (verified against the oracle with *and* without Storage present). They are
//! modeled here as read-only strings returning `''`; the numeric fleet-sum
//! functions are part of the NOT_PORTED Phase-7 surface.
//!
//! Split into submodules (no behavioral change): the property table, the struct
//! and its constructor live here; the parse-time behavior (`MakeFleetList` /
//! `RecalcElementData`) is in [`compute`], and the `CktElement`/`DssObject`
//! trait impls in [`accessors`].

mod accessors;
mod compute;

#[cfg(test)]
mod tests;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

// Discharge/charge mode ordinals (StorageController.pas l.268-277).
const MODE_FOLLOW: i32 = 1;
const MODE_PEAKSHAVE: i32 = 5;
const MODE_TIME: i32 = 4;
const CURRENT_PEAKSHAVE: i32 = 8;
const CURRENT_PEAKSHAVE_LOW: i32 = 9;
/// `MAXPHASE` — the monitored-phase sentinel "max of all phases".
const MAXPHASE: i32 = -2;

/// 1-based property ordinals (Pascal `TStorageControllerProp` + the
/// `TCktElementClass` tail).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const MON_PHASE: usize = 3;
    pub const KW_TARGET: usize = 4;
    pub const KW_TARGET_LOW: usize = 5;
    pub const PCT_KW_BAND: usize = 6;
    pub const KW_BAND: usize = 7;
    pub const PCT_KW_BAND_LOW: usize = 8;
    pub const KW_BAND_LOW: usize = 9;
    pub const ELEMENT_LIST: usize = 10;
    pub const WEIGHTS: usize = 11;
    pub const MODE_DISCHARGE: usize = 12;
    pub const MODE_CHARGE: usize = 13;
    pub const TIME_DISCHARGE_TRIGGER: usize = 14;
    pub const TIME_CHARGE_TRIGGER: usize = 15;
    pub const PCT_RATE_KW: usize = 16;
    pub const PCT_RATE_CHARGE: usize = 17;
    pub const PCT_RESERVE: usize = 18;
    pub const KWH_TOTAL: usize = 19;
    pub const KW_TOTAL: usize = 20;
    pub const KWH_ACTUAL: usize = 21;
    pub const KW_ACTUAL: usize = 22;
    pub const KW_NEED: usize = 23;
    pub const YEARLY: usize = 24;
    pub const DAILY: usize = 25;
    pub const DUTY: usize = 26;
    pub const EVENT_LOG: usize = 27;
    pub const INHIBIT_TIME: usize = 28;
    pub const T_UP: usize = 29;
    pub const T_FLAT: usize = 30;
    pub const T_DN: usize = 31;
    pub const KW_THRESHOLD: usize = 32;
    pub const DISP_FACTOR: usize = 33;
    pub const RESET_LEVEL: usize = 34;
    pub const SEASONS: usize = 35;
    pub const SEASON_TARGETS: usize = 36;
    pub const SEASON_TARGETS_LOW: usize = 37;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 38;
    pub const ENABLED: usize = 39;
    pub const NUM_PROPS: usize = 40; // incl. Like
}

/// `TStorageController.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        // Pascal: WriteByFunction(SetMonitoredElement) + Required, resolves
        // against any circuit class. The `Required` flag is inert in the port
        // (same as the other controls); a missing Element surfaces via
        // RecalcElementData 372.
        PropDef::object_ref_any("Element"),
        PropDef::integer("Terminal"),
        PropDef::mapped_string_enum("MonPhase", enums.mon_phase),
        PropDef::double("kWTarget"),
        PropDef::double("kWTargetLow"),
        PropDef::double("%kWBand"),
        // Pascal DynamicDefault + Redundant(pctkWBand) (metadata-only here).
        PropDef::double("kWBand").flags(PropFlags::REDUNDANT | PropFlags::DYNAMIC_DEFAULT),
        PropDef::double("%kWBandLow"),
        PropDef::double("kWBandLow").flags(PropFlags::REDUNDANT | PropFlags::DYNAMIC_DEFAULT),
        PropDef::string_list("ElementList"),
        // Pascal DoubleDArray + IndirectCount over the ElementList: the element
        // count is the storage-name-list length (see `array_size`).
        PropDef::double_v_array("Weights"),
        PropDef::mapped_string_enum("ModeDischarge", enums.storage_ctrl_discharge_mode),
        PropDef::mapped_string_enum("ModeCharge", enums.storage_ctrl_charge_mode),
        PropDef::double("TimeDischargeTrigger"),
        PropDef::double("TimeChargeTrigger"),
        PropDef::double("%RatekW"),
        PropDef::double("%RateCharge"),
        PropDef::double("%Reserve"),
        // SilentReadOnly + ReadByFunction fleet aggregates: the `?` getter
        // renders '' regardless of fleet contents (see module doc). Modeled as
        // read-only strings → ''.
        PropDef::string("kWhTotal"),
        PropDef::string("kWTotal"),
        PropDef::string("kWhActual"),
        PropDef::string("kWActual"),
        // SilentReadOnly plain double (dumps its value, not '').
        PropDef::double("kWNeed"),
        PropDef::object_ref_class("LoadShape", "Yearly"),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty"),
        PropDef::boolean("EventLog"),
        PropDef::integer("InhibitTime").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        PropDef::double("Tup").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        PropDef::double("TFlat").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        PropDef::double("Tdn").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        // Pascal DynamicDefault (recomputed from kWTarget in PropertySideEffects).
        PropDef::double("kWThreshold").flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::double("DispFactor"),
        PropDef::double("ResetLevel"),
        // Pascal SuppressJSON (derivable from length(SeasonTargets)).
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("SeasonTargets", prop::SEASONS),
        PropDef::double_array("SeasonTargetsLow", prop::SEASONS),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("StorageController", defs, true)
}

/// `TStorageControllerObj`.
#[derive(Debug, Clone)]
pub struct StorageController {
    pub ccd: ControlElemData,
    /// Dump name of the monitored element (Pascal renders `FullName`).
    monitored_full_name: String,
    /// Parse-time shape snapshot of the monitored reference.
    mon_snap: Option<RefSnapshot>,

    f_mon_phase: i32,

    f_kw_target: f64,
    f_kw_target_low: f64,
    f_kw_threshold: f64,
    f_pct_kw_band: f64,
    f_kw_band: f64,
    f_pct_kw_band_low: f64,
    f_kw_band_low: f64,
    half_kw_band: f64,
    half_kw_band_low: f64,
    f_kw_band_specified: bool,

    disp_factor: f64,

    /// `FStorageNameList`.
    storage_name_list: Vec<String>,
    /// `FWeights` (one per list entry; default 1.0).
    weights: Vec<f64>,
    /// `FleetSize`.
    fleet_size: i32,
    fleet_list_changed: bool,
    element_list_specified: bool,

    discharge_mode: i32,
    charge_mode: i32,
    discharge_trigger_time: f64,
    charge_trigger_time: f64,
    pct_kw_rate: f64,
    pct_charge_rate: f64,
    pct_fleet_reserve: f64,
    kw_needed: f64,

    yearly_shape: String,
    daily_shape: String,
    duty_shape: String,

    inhibit_hrs: i32,
    up_ramp_time: f64,
    flat_time: f64,
    dn_ramp_time: f64,
    reset_level: f64,

    seasons: i32,
    season_targets: Vec<f64>,
    season_targets_low: Vec<f64>,
}

impl StorageController {
    /// Pascal `TStorageControllerObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;

        let f_kw_target = 8000.0;
        let f_kw_target_low = 4000.0;
        let f_pct_kw_band = 2.0;
        let f_pct_kw_band_low = 2.0;
        let half_kw_band = f_pct_kw_band / 200.0 * f_kw_target;
        let half_kw_band_low = f_pct_kw_band_low / 200.0 * f_kw_target_low;
        Self {
            ccd,
            monitored_full_name: String::new(),
            mon_snap: None,
            f_mon_phase: MAXPHASE,
            f_kw_target,
            f_kw_target_low,
            f_kw_threshold: 6000.0,
            f_pct_kw_band,
            f_kw_band: half_kw_band * 2.0,
            f_pct_kw_band_low,
            f_kw_band_low: half_kw_band_low * 2.0,
            half_kw_band,
            half_kw_band_low,
            f_kw_band_specified: false,
            disp_factor: 1.0,
            storage_name_list: Vec::new(),
            weights: Vec::new(),
            fleet_size: 0,
            fleet_list_changed: true, // force building of list
            element_list_specified: false,
            discharge_mode: MODE_PEAKSHAVE,
            charge_mode: MODE_TIME,
            discharge_trigger_time: -1.0, // disabled
            charge_trigger_time: 2.0,     // 2 AM
            pct_kw_rate: 20.0,
            pct_charge_rate: 20.0,
            pct_fleet_reserve: 25.0,
            kw_needed: 0.0,
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            inhibit_hrs: 5,
            up_ramp_time: 0.25,
            flat_time: 2.0,
            dn_ramp_time: 0.25,
            reset_level: 0.8,
            seasons: 1,
            season_targets: vec![f_kw_target],
            season_targets_low: vec![f_kw_target_low],
        }
    }
}
