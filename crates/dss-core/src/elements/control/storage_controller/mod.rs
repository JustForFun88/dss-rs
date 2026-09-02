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
//! `kWActual`) render the **live** fleet aggregate, exactly as r4133 does
//! (`StorageController.pas:991-994` -> `GetkWhTotal`/`GetkWTotal`/
//! `GetkWhActual`/`GetkWActual`, bodies `:1162-1198`): the two `*Total` are
//! nameplate sums over the fleet (`StorageVars.kWhRating`/`kWRating`), the two
//! `*Actual` are the live `FleetkWh`/`FleetkW` (sums of `kWhStored` /
//! `PresentkW`). They read no stored total and write nothing -- see
//! [`FleetAggregates`] for the cache the read surfaces refresh and for the
//! r4133 `Var Sum` write-back the port deliberately does not reproduce. Until
//! RP3.8 they rendered `''`, which was the dss_capi 0.14.5
//! `SilentReadOnly + ReadByFunction` surface convention, not upstream behavior
//! ([`PropFlags::SILENT_READ_ONLY`]).
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
use crate::elements::control::mon_phase::MonPhase;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::pc::storage::StorageState;
use crate::elements::traits::ElemId;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::solution::SolveMode;

/// Discharge / charge mode ordinals (`StorageController.pas:268-276`). One
/// shared ordinal space feeds **two** `DssEnum`s — `StorageController: Discharge
/// Mode` (values `[5,1,3,2,4,6,8]`) and `... Charge Mode` (`[2,4,7,9]`) — so a
/// single enum covers both the `ModeDischarge=` and `ModeCharge=` fields, and
/// each `Sample` arm rejects the ordinals its own mode does not accept exactly
/// as the pre-enum `_ =>` arms did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum StorageCtrlMode {
    /// `MODEFOLLOW = 1`.
    Follow = 1,
    /// `MODELOADSHAPE = 2`.
    LoadShape = 2,
    /// `MODESUPPORT = 3`.
    Support = 3,
    /// `MODETIME = 4`.
    Time = 4,
    /// `MODEPEAKSHAVE = 5` — `TStorageControllerObj.Create`'s DischargeMode.
    PeakShave = 5,
    /// `MODESCHEDULE = 6`.
    Schedule = 6,
    /// `MODEPEAKSHAVELOW = 7`.
    PeakShaveLow = 7,
    /// `CURRENTPEAKSHAVE = 8`.
    CurrentPeakShave = 8,
    /// `CURRENTPEAKSHAVELOW = 9`.
    CurrentPeakShaveLow = 9,
}

impl StorageCtrlMode {
    /// The `StorageController: Discharge/Charge Mode` `DssEnum` ordinal.
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Follow),
            2 => Some(Self::LoadShape),
            3 => Some(Self::Support),
            4 => Some(Self::Time),
            5 => Some(Self::PeakShave),
            6 => Some(Self::Schedule),
            7 => Some(Self::PeakShaveLow),
            8 => Some(Self::CurrentPeakShave),
            9 => Some(Self::CurrentPeakShaveLow),
            _ => None,
        }
    }
}

/// The control-queue action code this class *handles* on pop
/// (`StorageController.pas:279` `RELEASE_INHIBIT = 999`) — it lifts the
/// discharge-inhibit after a charge cycle. `Sample` additionally pushes the
/// **storage-state** ordinals (`STORE_CHARGING`/`STORE_DISCHARGING`) as
/// immediate re-solve markers — upstream reuses that ordinal space on the same
/// generic queue — and `DoPendingAction` deliberately ignores them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum StorageCtrlAction {
    ReleaseInhibit = 999,
}

impl StorageCtrlAction {
    /// The `ControlQueue` action code.
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From a popped `ControlQueue` action code; anything else (including the
    /// `StorageState` markers `Sample` pushes) yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            999 => Some(Self::ReleaseInhibit),
            _ => None,
        }
    }
}
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
        // The four fleet aggregates. dss_capi 0.14.5 flags them
        // `[SilentReadOnly, ReadByFunction]` (`StorageController.pas:416-423`,
        // read fns GetkWhTotal/...) and, being function-only (no
        // PropertyOffset), suppresses their text render; r4133 renders them live
        // (`:991-994`), so they carry `RENDERS_LIVE_RESULT` too and the `?`/props
        // surfaces answer the number (RP3.8). `SILENT_READ_ONLY` still governs
        // the other three surfaces unchanged: the JSON export omits them, a JSON
        // load ignores them, and the schema renders `type:number, readOnly:true`
        // with no default.
        PropDef::double("kWhTotal")
            .flags(PropFlags::SILENT_READ_ONLY | PropFlags::RENDERS_LIVE_RESULT),
        PropDef::double("kWTotal")
            .flags(PropFlags::SILENT_READ_ONLY | PropFlags::RENDERS_LIVE_RESULT),
        PropDef::double("kWhActual")
            .flags(PropFlags::SILENT_READ_ONLY | PropFlags::RENDERS_LIVE_RESULT),
        PropDef::double("kWActual")
            .flags(PropFlags::SILENT_READ_ONLY | PropFlags::RENDERS_LIVE_RESULT),
        // dss_capi 0.14.5 `[SilentReadOnly]` with `PropertyOffset = @kWNeeded`
        // (`StorageController.pas:426-427`): a read-only double whose value has a
        // real backing field, so 0.14.5 dumps it like any other ('?' -> the stored
        // kWNeeded) and exports it to JSON. Schema-only `READ_ONLY` marks it
        // `readOnly` + elides the default, without the function-only JSON
        // omission `SILENT_READ_ONLY` carries. NOT one of RP3.8's five pairs.
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

    f_mon_phase: MonPhase,

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

    discharge_mode: StorageCtrlMode,
    charge_mode: StorageCtrlMode,
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
    fleet: Vec<ElemId>,
    /// `FleetState` — the aggregate fleet charge/idle/discharge state.
    fleet_state: StorageState,
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

    /// The render cache behind the four live read-only aggregates
    /// (`kWhTotal`/`kWTotal`/`kWhActual`/`kWActual`) — see [`FleetAggregates`].
    /// Refreshed from the live fleet by the read surfaces' choke point
    /// (`Dss::refresh_vterminal_if_marked`) immediately before every render, so
    /// it is never read stale; NOT dispatch state (nothing in `Sample` reads
    /// it), and not `MakeLike`-copied for the same reason.
    live_aggregates: FleetAggregates,
}

/// The four fleet aggregates r4133 renders for the read-only
/// `kWhTotal`/`kWTotal`/`kWhActual`/`kWActual` properties
/// (`Controls/StorageController.pas:991-994`, all `Format('%-.8g', …)`).
///
/// In r4133 each is computed inside the getter, from live pointers:
///
/// | field | r4133 getter | body |
/// |---|---|---|
/// | `kwh_total` | `GetkWhTotal(TotalkWhCapacity)` | `:1172-1184`, `Σ StorageVars.kWhRating` over `FleetPointerList` |
/// | `kw_total` | `GetkWTotal(TotalkWCapacity)` | `:1186-1198`, `Σ StorageVars.kWRating` |
/// | `kwh_actual` | `GetkWhActual` | `:1167-1170`, `FleetkWh` = `Σ StorageVars.kWhStored` (`Get_FleetkWh`, `:1032-1042`) |
/// | `kw_actual` | `GetkWActual` | `:1162-1165`, `FleetkW` = `Σ PresentkW` (`Get_FleetkW`, `:1019-1029`) |
///
/// The Rust `&self` property getter cannot reach another class's arena, so the
/// render surfaces refresh this cache first — `Dss::refresh_vterminal_if_marked`
/// per read (`?`, `Dump`, `element_properties`),
/// `Dss::refresh_render_caches_for_save` once up front for `Save`'s whole-class
/// walk, both gated on [`PropFlags::RENDERS_LIVE_RESULT`] — and the getter
/// returns the just-computed
/// number — the same construction [`PropFlags::READS_VTERMINAL`] uses for
/// Transformer `WdgCurrents`.
///
/// **The r4133 write-back is deliberately not reproduced.** `GetkWhTotal` and
/// `GetkWTotal` take `Var Sum`, and the two property arms pass the object's own
/// `TotalkWhCapacity`/`TotalkWCapacity` fields (`:81-82`), so *reading* those
/// two properties *writes* the object in r4133 — a read-that-mutates of the
/// `VSConverter.GetCurrents` family (CLAUDE.md §"Known upstream bugs"). Nothing
/// in `Version8/Source` ever reads the two fields (the whole-tree grep finds
/// only the declarations, the two arms, and the two dead `RecalcElementData`
/// calls at `:1107-1108`; dss_capi 0.14.5 went further and commented them out),
/// and the getters re-sum the fleet from scratch on every call, so the store has
/// no observable and the port renders the identical number with a pure read.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct FleetAggregates {
    /// `kWhTotal` — `Σ` the fleet's `kWhRating`.
    pub kwh_total: f64,
    /// `kWTotal` — `Σ` the fleet's `kWRating`.
    pub kw_total: f64,
    /// `kWhActual` — `Σ` the fleet's `kWhStored` (`FleetkWh`).
    pub kwh_actual: f64,
    /// `kWActual` — `Σ` the fleet's `PresentkW` (`FleetkW`).
    pub kw_actual: f64,
}

/// One fleet member's live state, the only inputs the four aggregate getters
/// read (`StorageController.pas:1162-1198`). The executive fills one per
/// [`StorageController::fleet_refs`] entry from the Storage arena, and
/// [`StorageController::refresh_live_aggregates`] sums them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FleetMemberLive {
    /// `PresentkW`.
    pub present_kw: f64,
    /// `StorageVars.kWhStored`.
    pub kwh_stored: f64,
    /// `StorageVars.kWhRating`.
    pub kwh_rating: f64,
    /// `StorageVars.kWRating`.
    pub kw_rating: f64,
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
            f_mon_phase: MonPhase::Max,
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
            discharge_mode: StorageCtrlMode::PeakShave,
            charge_mode: StorageCtrlMode::Time,
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
            fleet_state: StorageState::Idling,
            total_weight: 1.0,
            up_plus_flat: 0.25 + 2.0,                // UpRampTime + FlatTime
            up_plus_flat_plus_dn: 0.25 + 2.0 + 0.25, // + DnRampTime
            last_pct_discharge_rate: 0.0,
            charging_allowed: false,
            discharge_triggered_by_time: false,
            discharge_inhibited: false,
            out_of_oomph: false,
            wait4step: false,
            live_aggregates: FleetAggregates::default(),
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
    Found(ElemId),
}

/// A read-only snapshot of one fleet Storage element's state, the inputs the
/// dispatch modes read each iteration (Pascal reads these live off the
/// `TStorageObj` pointer).
#[derive(Debug, Clone, Copy)]
pub(crate) struct StorageSnap {
    /// `StorageState` (`FState`).
    pub state: StorageState,
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
/// `ElemId`s returned by the fleet lookups are passed back to the accessors.
pub(crate) trait StorageDispatchEnv {
    // --- monitored element ---
    /// Pascal `GetControlPower(S)` — the active-power signal at the monitored
    /// terminal, resolved per `MonPhase` (×3 under positive sequence). `fnphases`
    /// is the controller's `Fnphases` (the monitored element's phase count).
    fn control_power(&mut self, mon_phase: MonPhase, fnphases: usize) -> Complex64;
    /// Pascal `GetControlCurrent(Amps)` — the per-`MonPhase` terminal current.
    fn control_current(&mut self, mon_phase: MonPhase, fnphases: usize) -> f64;
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
    fn all_fleet_storage(&self) -> Vec<(String, ElemId)>;
    /// Pascal `DoSimpleMsg` sink (the 14403 named-missing error).
    fn push_error(&mut self, diag: crate::diag::DssDiagnostic);

    // --- per-storage read / write ---
    /// Read the dispatch-relevant state of one fleet member.
    fn snap(&self, r: ElemId) -> StorageSnap;
    /// `obj.StorageState := state` (`Set_StorageState`, declines past kWh limits).
    fn set_state(&mut self, r: ElemId, state: StorageState);
    /// `obj.kW := kw` (`Set_kW`, sets the state + dispatch %).
    fn set_kw(&mut self, r: ElemId, kw: f64);
    /// `obj.pctkWout := pct`.
    fn set_pct_kw_out(&mut self, r: ElemId, pct: f64);
    /// `obj.pctkWin := pct`.
    fn set_pct_kw_in(&mut self, r: ElemId, pct: f64);
    /// `obj.pctReserve := pct`.
    fn set_pct_reserve(&mut self, r: ElemId, pct: f64);
    /// `obj.StateDesired := state`.
    fn set_state_desired(&mut self, r: ElemId, state: StorageState);
    /// `obj.DispatchMode := STORE_EXTERNALMODE`.
    fn set_dispatch_external(&mut self, r: ElemId);
    /// `obj.SetNominalDEROutput()` — recompute the storage's present P/Q.
    fn set_nominal(&mut self, r: ElemId);
    /// `obj.PresentkW` after a dispatch (re-read).
    fn present_kw(&self, r: ElemId) -> f64;
    /// `obj.FullName` (`Storage.<name>`) — for the event-log messages.
    fn storage_full_name(&self, r: ElemId) -> String;

    // --- control queue / event log / solution flags ---
    /// Pascal `PushTimeOntoControlQueue(Code)`: `LoadsNeedUpdating := TRUE` +
    /// `ControlQueue.Push(0, Code, 0, Self)` (force a re-solve at the present step).
    fn push_immediate(&mut self, code: StorageState);
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
