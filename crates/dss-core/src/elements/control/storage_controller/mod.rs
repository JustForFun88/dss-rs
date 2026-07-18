//! Port of `Controls/StorageController.pas` — `TStorageControllerObj`, a control
//! element that dispatches a *fleet* of Storage elements (charge/discharge) to
//! hold a monitored element's power inside a target band.
//!
//! **Real behavior (PHASE7_PLAN WP7.4 step 2).** With the Storage element landed
//! (WP7.4 step 1), the fleet now resolves and dispatches. This module ports the
//! full control: `MakeFleetList`, the `SetFleet*` helpers, `GetControlPower` /
//! `GetControlCurrent`, the fleet kW/kWh aggregates, and `Sample`'s dispatch
//! modes — `DoLoadFollowMode` (Peakshave / Follow / Support / I-Peakshave),
//! `DoTimeMode`, `DoScheduleMode`, `DoLoadShapeMode`, `DoPeakShaveModeLow`
//! (PeakshaveLow / I-PeakshaveLow charging) — plus `DoPendingAction`
//! (RELEASE_INHIBIT) and `Reset` (`SetFleetToIdle`).
//!
//! Like every `TControlElem`, a StorageController builds **no Yprim** and its
//! terminal currents are zero; its single terminal attaches to the monitored
//! element's terminal bus. The fleet is a *dynamic* set of Storage elements, so —
//! exactly like [`GenDispatcher`] — `Sample` reaches the monitored element and
//! the fleet through the executive via the [`StorageDispatchEnv`] abstraction
//! (`solution/controls/dispatch.rs`), resolved against the class registry. The
//! fleet (`FleetPointerList`) is built lazily on the first `Sample` (the
//! architecture has no store access at parse-time `RecalcElementData`); the
//! `SetFleetToExternal` + `SetAllFleetValues` that Pascal runs in
//! `RecalcElementData` after the fleet build are deferred to that first build.
//!
//! The seasonal dynamic target (`Get_DynamicTarget`, `DSS.SeasonalRating` /
//! `DSS.SeasonSignal` — `Set SeasonRating=`/`Set SeasonSignal=`, GAPS_PLAN
//! WPG.11) is ported: [`StorageController::get_dynamic_target`] (`compute.rs`)
//! + the [`StorageDispatchEnv::season_rating`]/[`StorageDispatchEnv::
//! season_rating_idx`] call-site guards.
//!
//! `MakePosSequence` is ported (WPG.21): the monitored-element phase/conductor/
//! bus resync (see [`accessors`]).
//!
//! **Deliberately NOT_PORTED:**
//! - the parse-time 37201 ("No unassigned Storage Elements") for a *Storage-less*
//!   circuit: Pascal emits it in `RecalcElementData`; here the fleet resolves
//!   lazily at first `Sample`, so a default empty fleet is a silent no-op (same
//!   as [`GenDispatcher`]'s empty gen list). A *named-but-missing* element still
//!   errors (14403) at first `Sample`.
//!
//! The four fleet-aggregate readbacks (`kWhTotal`/`kWTotal`/`kWhActual`/
//! `kWActual`) are Pascal `SilentReadOnly + ReadByFunction` doubles whose
//! `?`/`GetObjPropertyValue` getter renders `''` regardless of fleet contents
//! (verified against the oracle); they stay read-only `''` strings.
//!
//! Split into submodules (no behavioral change): the property table, the struct
//! and its constructor live here; `MakeFleetList` / the dispatch modes /
//! `RecalcElementData` are in [`compute`], and the `CktElement`/`DssObject` trait
//! impls in [`accessors`].
//!
//! [`GenDispatcher`]: crate::elements::control::gen_dispatcher::GenDispatcher

#[cfg(test)]
mod tests;

mod accessors;
mod compute;

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::pc::storage::STORE_IDLING;
use crate::elements::traits::ElemRef;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::solution::SolveMode;

// Discharge/charge mode ordinals (StorageController.pas l.268-277).
const MODE_FOLLOW: i32 = 1;
const MODE_LOADSHAPE: i32 = 2;
const MODE_SUPPORT: i32 = 3;
const MODE_TIME: i32 = 4;
const MODE_PEAKSHAVE: i32 = 5;
const MODE_SCHEDULE: i32 = 6;
const MODE_PEAKSHAVELOW: i32 = 7;
const CURRENT_PEAKSHAVE: i32 = 8;
const CURRENT_PEAKSHAVE_LOW: i32 = 9;
/// `RELEASE_INHIBIT` — the control-queue action code that lifts the
/// discharge-inhibit after charging. `pub(crate)` so the dispatch env can push it.
pub(crate) const RELEASE_INHIBIT: i32 = 999;
/// Monitored-phase sentinels (`StorageController.pas` l.38-40). `pub(crate)` so
/// the dispatch env's `GetControlPower`/`GetControlCurrent` can resolve them.
pub(crate) const AVG: i32 = -1;
pub(crate) const MAXPHASE: i32 = -2;
pub(crate) const MINPHASE: i32 = -3;

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
        PropDef::object_ref_any("Element").flags(PropFlags::REQUIRED),
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
        // Pascal `DoubleDArrayProperty` + `IndirectCount` over the ElementList
        // (`StorageController.pas:409-413`, `PropertyOffset2 = @FleetSize`,
        // `PropertyOffset3 = @FStorageNameList`): renders `ArrayOrFilePath` +
        // `$dssLength: ElementList`. The element count is `FleetSize` (kept in
        // sync with the storage-name-list length), exposed via `get_i32(ELEMENT_LIST)`.
        PropDef::double_array("Weights", prop::ELEMENT_LIST),
        PropDef::mapped_string_enum("ModeDischarge", enums.storage_ctrl_discharge_mode),
        PropDef::mapped_string_enum("ModeCharge", enums.storage_ctrl_charge_mode),
        PropDef::double("TimeDischargeTrigger"),
        PropDef::double("TimeChargeTrigger"),
        PropDef::double("%RatekW"),
        PropDef::double("%RateCharge"),
        PropDef::double("%Reserve"),
        // Pascal `[SilentReadOnly, ReadByFunction]` fleet-aggregate doubles
        // (`StorageController.pas:416-423`, read fns GetkWhTotal/…). Function-only
        // (no PropertyOffset), so the `?`/props render is '' and the JSON export
        // omits them (`SILENT_READ_ONLY`); the schema renders `type:number,
        // readOnly:true` with no default.
        PropDef::double("kWhTotal").flags(PropFlags::SILENT_READ_ONLY),
        PropDef::double("kWTotal").flags(PropFlags::SILENT_READ_ONLY),
        PropDef::double("kWhActual").flags(PropFlags::SILENT_READ_ONLY),
        PropDef::double("kWActual").flags(PropFlags::SILENT_READ_ONLY),
        // Pascal `[SilentReadOnly]` with `PropertyOffset = @kWNeeded`
        // (`StorageController.pas:426-427`): a read-only double that still dumps
        // its value ('?' → the stored kWNeeded). Schema-only `READ_ONLY` marks it
        // `readOnly` + elides the default without the function-only '' behaviour.
        PropDef::double("kWNeed").flags(PropFlags::READ_ONLY),
        PropDef::object_ref_class("LoadShape", "Yearly"),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty"),
        PropDef::boolean("EventLog"),
        PropDef::integer("InhibitTime").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        PropDef::double("TUp").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        PropDef::double("TFlat").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        PropDef::double("TDn").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        // Pascal DynamicDefault (recomputed from kWTarget in PropertySideEffects).
        PropDef::double("kWThreshold").flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::double("DispFactor"),
        PropDef::double("ResetLevel"),
        // Pascal SuppressJSON (derivable from length(SeasonTargets)).
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("SeasonTargets", prop::SEASONS),
        PropDef::double_array("SeasonTargetsLow", prop::SEASONS),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
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
    /// Snapshot-clones of the resolved dispatch shapes (Pascal
    /// `YearlyShapeObj`/`DailyShapeObj`/`DutyShapeObj`) — read by `DoLoadShapeMode`.
    yearly_shape_obj: Option<LoadShapeObj>,
    daily_shape_obj: Option<LoadShapeObj>,
    duty_shape_obj: Option<LoadShapeObj>,
    /// `LoadShapeMult` — the LoadShape-mode dispatch multiplier (per sample).
    load_shape_mult: Complex64,

    inhibit_hrs: i32,
    up_ramp_time: f64,
    flat_time: f64,
    dn_ramp_time: f64,
    reset_level: f64,

    seasons: i32,
    season_targets: Vec<f64>,
    season_targets_low: Vec<f64>,

    // --- runtime dispatch state (Pascal `TStorageControllerObj` private flags) ---
    /// `FleetPointerList` — the resolved Storage fleet, built lazily on the first
    /// `Sample` (empty until then), cached across samples exactly like Pascal.
    fleet: Vec<ElemRef>,
    /// `FleetState` — the aggregate fleet charge/idle/discharge state.
    fleet_state: i32,
    /// `TotalWeight` — the sum of `FWeights` over the fleet.
    total_weight: f64,
    /// `UpPlusFlat` / `UpPlusFlatPlusDn` — Schedule-mode ramp boundaries.
    up_plus_flat: f64,
    up_plus_flat_plus_dn: f64,
    /// `LastpctDischargeRate` — Schedule-mode rate memory.
    last_pct_discharge_rate: f64,
    /// `ChargingAllowed` — set per-sample by the discharge phase, gates charging.
    charging_allowed: bool,
    /// `DischargeTriggeredByTime` (Follow mode + Time trigger).
    discharge_triggered_by_time: bool,
    /// `DischargeInhibited` — held while charging until RELEASE_INHIBIT fires.
    discharge_inhibited: bool,
    /// `OutOfOomph` — the fleet ran out of deliverable energy.
    out_of_oomph: bool,
    /// `Wait4Step` — defer charging one step after a discharge→charge transition.
    wait4step: bool,
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
            yearly_shape_obj: None,
            daily_shape_obj: None,
            duty_shape_obj: None,
            load_shape_mult: Complex64::ZERO,
            inhibit_hrs: 5,
            up_ramp_time: 0.25,
            flat_time: 2.0,
            dn_ramp_time: 0.25,
            reset_level: 0.8,
            seasons: 1,
            season_targets: vec![f_kw_target],
            season_targets_low: vec![f_kw_target_low],

            fleet: Vec::new(),
            fleet_state: STORE_IDLING,
            total_weight: 1.0,
            up_plus_flat: 0.25 + 2.0,                // UpRampTime + FlatTime
            up_plus_flat_plus_dn: 0.25 + 2.0 + 0.25, // + DnRampTime
            last_pct_discharge_rate: 0.0,
            charging_allowed: false,
            discharge_triggered_by_time: false,
            discharge_inhibited: false,
            out_of_oomph: false,
            wait4step: false,
        }
    }
}

/// The outcome of resolving a named fleet member (Pascal
/// `DSS.StorageClass.Find` + the `Enabled` check in `MakeFleetList`).
#[derive(Debug, Clone, Copy)]
pub(crate) enum FleetFind {
    /// No Storage by that name (Pascal `DoSimpleMsg(14403)` + `Exit`).
    NotFound,
    /// Found but disabled (Pascal silently skips it).
    Disabled,
    /// Found and enabled (added to the fleet).
    Found(ElemRef),
}

/// A read-only snapshot of one fleet Storage element's state, the inputs the
/// dispatch modes read each iteration (Pascal reads these live off the
/// `TStorageObj` pointer).
#[derive(Debug, Clone, Copy)]
pub(crate) struct StorageSnap {
    /// `StorageState` (`FState`).
    pub state: i32,
    /// `PresentkW` (AC inverter output).
    pub present_kw: f64,
    /// `PresentkV` (rated kV).
    pub present_kv: f64,
    /// `kW` (`Get_kW` — the published `kW_out`).
    pub kw: f64,
    /// `StorageVars.kWhStored` / `kWhRating` / `kWhReserve`.
    pub kwh_stored: f64,
    pub kwh_rating: f64,
    pub kwh_reserve: f64,
    /// `StorageVars.kWrating`.
    pub kw_rating: f64,
    /// `NPhases`.
    pub nphases: usize,
    /// `CutInkWAC` / `CutOutkWAC`.
    pub cut_in_kw_ac: f64,
    pub cut_out_kw_ac: f64,
    /// `kWOutIdling`.
    pub kw_out_idling: f64,
    /// `InverterON`.
    pub inverter_on: bool,
}

/// The executive surface `Sample`/`Reset` need to reach the monitored element
/// and the dispatched Storage fleet (the Rust stand-in for Pascal's live object
/// pointers + `ActiveCircuit.Solution`/`ControlQueue`/`EventStrings` reach).
/// `ElemRef`s returned by the fleet lookups are passed back to the accessors.
pub(crate) trait StorageDispatchEnv {
    // --- monitored element ---
    /// Pascal `GetControlPower(S)` — the active-power signal at the monitored
    /// terminal, resolved per `MonPhase` (×3 under positive sequence). `fnphases`
    /// is the controller's `Fnphases` (the monitored element's phase count).
    fn control_power(&mut self, mon_phase: i32, fnphases: usize) -> Complex64;
    /// Pascal `GetControlCurrent(Amps)` — the per-`MonPhase` terminal current.
    fn control_current(&mut self, mon_phase: i32, fnphases: usize) -> f64;
    /// Pascal `MonitoredElement.ComputeVterminal; cabs(Vterminal[1])` — the LN
    /// voltage magnitude used to convert a current deficit into kW.
    fn monitored_vterminal1_abs(&mut self) -> f64;
    /// `MonitoredElement.NPhases`.
    fn monitored_nphases(&self) -> usize;

    // --- fleet resolution ---
    /// Pascal `DSS.StorageClass.Find(name)`: distinguish *missing* (→ 14403) from
    /// *found-but-disabled* (silently skipped) from *found-and-enabled*.
    fn find_storage(&self, name: &str) -> FleetFind;
    /// Pascal's "scan the whole circuit for enabled, non-external storage"
    /// (creation order); returns `(name, ref)` pairs.
    fn all_fleet_storage(&self) -> Vec<(String, ElemRef)>;
    /// Pascal `DoSimpleMsg` sink (the 14403 named-missing error).
    fn push_error(&mut self, msg: String);

    // --- per-storage read / write ---
    /// Read the dispatch-relevant state of one fleet member.
    fn snap(&self, r: ElemRef) -> StorageSnap;
    /// `obj.StorageState := state` (`Set_StorageState`, declines past kWh limits).
    fn set_state(&mut self, r: ElemRef, state: i32);
    /// `obj.kW := kw` (`Set_kW`, sets the state + dispatch %).
    fn set_kw(&mut self, r: ElemRef, kw: f64);
    /// `obj.pctkWout := pct`.
    fn set_pct_kw_out(&mut self, r: ElemRef, pct: f64);
    /// `obj.pctkWin := pct`.
    fn set_pct_kw_in(&mut self, r: ElemRef, pct: f64);
    /// `obj.pctReserve := pct`.
    fn set_pct_reserve(&mut self, r: ElemRef, pct: f64);
    /// `obj.StateDesired := state`.
    fn set_state_desired(&mut self, r: ElemRef, state: i32);
    /// `obj.DispatchMode := STORE_EXTERNALMODE`.
    fn set_dispatch_external(&mut self, r: ElemRef);
    /// `obj.SetNominalDEROutput()` — recompute the storage's present P/Q.
    fn set_nominal(&mut self, r: ElemRef);
    /// `obj.PresentkW` after a dispatch (re-read).
    fn present_kw(&self, r: ElemRef) -> f64;
    /// `obj.FullName` (`Storage.<name>`) — for the event-log messages.
    fn storage_full_name(&self, r: ElemRef) -> String;

    // --- control queue / event log / solution flags ---
    /// Pascal `PushTimeOntoControlQueue(Code)`: `LoadsNeedUpdating := TRUE` +
    /// `ControlQueue.Push(0, Code, 0, Self)` (force a re-solve at the present step).
    fn push_immediate(&mut self, code: i32);
    /// Pascal `ControlQueue.Push(intHour + InhibitHrs, t, RELEASE_INHIBIT, 0, Self)`
    /// (+ `LoadsNeedUpdating := TRUE`).
    fn push_release_inhibit(&mut self, inhibit_hrs: i32);
    /// Pascal `AppendToEventLog(FullName, msg)` (only called when ShowEventLog).
    fn append_event(&mut self, msg: &str);
    /// `ActiveCircuit.Solution.LoadsNeedUpdating := TRUE`.
    fn set_loads_need_updating(&mut self);

    // --- scalars ---
    /// `ActiveCircuit.Solution.TimeOfDay()`.
    fn time_of_day(&self) -> f64;
    /// `ActiveCircuit.Solution.DynaVars.h` (seconds).
    fn dyna_h(&self) -> f64;
    /// `ActiveCircuit.Solution.DynaVars.dblHour`.
    fn dbl_hour(&self) -> f64;
    /// `ActiveCircuit.Solution.ControlIteration` — D10 (`1b3123ce`) forces a new
    /// power flow (`StorekWChanged`) on the first control iteration.
    fn control_iteration(&self) -> i32;
    /// `ActiveCircuit.Solution.Mode`.
    fn solve_mode(&self) -> SolveMode;

    // --- seasonal targets (`Get_DynamicTarget`, StorageController.pas l.1020) ---
    /// `DSS.SeasonalRating` (`Set SeasonRating=`) — the call-site guard at
    /// l.1099 (discharge)/l.1411 (charge): only when set does
    /// `Get_DynamicTarget` run at all.
    fn season_rating(&self) -> bool;
    /// `Get_DynamicTarget`'s `RatingIdx` (l.1020-1032). `None` when
    /// `DSS.SeasonSignal` is empty — `Result` stays `0` in Pascal, i.e. the
    /// caller must NOT fall back to the non-seasonal target (see
    /// [`StorageController::get_dynamic_target`]). `Some(trunc(XYcurve.
    /// GetYValue(Solution.DynaVars.intHour)))` when the signal is set —
    /// `Some(0)` if the named curve isn't registered (`RSignal = NIL`;
    /// `RatingIdx` initializes to `0` and Pascal never reassigns it on a
    /// miss). Mutates the curve's hunt cache (`GetYValue`'s
    /// `LastValueAccessed` side effect) — a live, uncached lookup every call,
    /// exactly like the Pascal `DSS.XYCurveClass.Find` here.
    fn season_rating_idx(&mut self) -> Option<i32>;
}
