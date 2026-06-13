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

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::elements::traits::{CktElement, ElemRef, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
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

    /// Pascal `TStorageControllerObj.MakeFleetList`. In Phase 6 there is no
    /// Storage class, so the fleet is always empty and the function returns
    /// `false` (the caller turns that into error 37201).
    ///
    /// The `FleetListChanged := FALSE` clear at the tail of Pascal `MakeFleetList`
    /// (l.1927) is reached by the **default** branch and by a **fully-resolved
    /// named** branch, but **not** by the early `Exit` taken when a named element
    /// is missing (l.1889). Reproducing that distinction is what stops a later
    /// `RecalcElementData` (a second `Edit`) from re-emitting 37201 every time:
    /// only the missing-name path leaves the rebuild pending.
    fn make_fleet_list(&mut self) -> bool {
        if self.element_list_specified {
            // Named list. The first name never resolves (no Storage class), so a
            // *non-empty* list errors and Pascal `Exit`s with FleetListChanged
            // still TRUE.
            if let Some(first) = self.storage_name_list.first() {
                self.ccd
                    .cd
                    .obj
                    .push_error(format!("Error: Storage Element \"{first}\" not found."));
                return false; // FleetListChanged stays true (Pascal Exits here)
            }
            // Empty named list: the resolve loop never runs and Pascal falls
            // through to the shared tail below.
        } else {
            // Default branch: scan all enabled Storage (none exist) → empty fleet.
            self.storage_name_list.clear();
            self.fleet_size = 0;
            self.weights.clear();
        }
        // Pascal `FleetListChanged := FALSE` (l.1927).
        self.fleet_list_changed = false;
        false // FleetPointerList is always empty in Phase 6 → Result stays FALSE
    }

    /// Pascal `TStorageControllerObj.RecalcElementData` (parse-time subset):
    /// validate the monitored element, attach the control's single terminal to
    /// the monitored terminal's bus, then (re)build the fleet list.
    fn recalc(&mut self) {
        let Some(mon) = self.mon_snap.clone() else {
            // Pascal `DoSimpleMsg('Monitored Element in %s is not set', 372)`.
            self.ccd.cd.obj.push_error(format!(
                "Monitored Element in StorageController.{} is not set",
                self.ccd.cd.obj.name()
            ));
            return;
        };

        if self.ccd.element_terminal > mon.nterms as i32 {
            // Pascal `DoErrorMsg(... 'Terminal no. "%d" Does not exist.' 371)`.
            self.ccd.cd.obj.push_error(format!(
                "StorageController: \"{}\": Terminal no. \"{}\" Does not exist. Re-specify terminal no.",
                self.ccd.cd.obj.name(),
                self.ccd.element_terminal
            ));
        } else {
            // Pascal: FNphases := MonitoredElement.Nphases; NConds := FNphases;
            // the control adopts the monitored element's phase count (so a later
            // MonPhase edit validates against the right number of phases).
            self.ccd.cd.nphases = mon.nphases;
            self.ccd.cd.set_nconds(mon.nphases);

            // Set the name of the control's 1st terminal's connected bus.
            let t = self.ccd.element_terminal;
            let bus = if t >= 1 && (t as usize) <= mon.buses.len() {
                mon.buses[(t - 1) as usize].clone()
            } else {
                String::new() // Pascal GetBus(i) out of range yields ''
            };
            self.ccd.cd.set_bus(1, &bus);
        }

        if self.fleet_list_changed && !self.make_fleet_list() {
            self.ccd.cd.obj.push_error(format!(
                "No unassigned Storage Elements found to assign to StorageController.{}",
                self.ccd.cd.obj.name()
            ));
        }
    }
}

impl CktElement for StorageController {
    fn cd(&self) -> &crate::elements::ckt::CktElementData {
        &self.ccd.cd
    }
    fn cd_mut(&mut self) -> &mut crate::elements::ckt::CktElementData {
        &mut self.ccd.cd
    }

    fn recalc_element_data(&mut self, _sys: &SysCtx) {
        self.recalc();
    }

    /// Pascal `TControlElem.CalcYPrim`: leave YPrim NIL — `BuildYMatrix` skips it.
    fn calc_yprim(&mut self, _sys: &SysCtx) {}

    /// Pascal `TControlElem.GetCurrents`: always zero.
    fn get_currents(&mut self, _sys: &SysCtx, _node_v: &[Complex64], curr: &mut [Complex64]) {
        curr.fill(Complex64::ZERO);
    }
}

impl DssObject for StorageController {
    fn data(&self) -> &DssObjData {
        &self.ccd.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.ccd.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_ckt_element(&self) -> Option<&dyn CktElement> {
        Some(self)
    }
    fn as_ckt_element_mut(&mut self) -> Option<&mut dyn CktElement> {
        Some(self)
    }

    fn get_f64(&self, idx: usize) -> f64 {
        use prop::*;
        match idx {
            KW_TARGET => self.f_kw_target,
            KW_TARGET_LOW => self.f_kw_target_low,
            PCT_KW_BAND => self.f_pct_kw_band,
            KW_BAND => self.f_kw_band,
            PCT_KW_BAND_LOW => self.f_pct_kw_band_low,
            KW_BAND_LOW => self.f_kw_band_low,
            TIME_DISCHARGE_TRIGGER => self.discharge_trigger_time,
            TIME_CHARGE_TRIGGER => self.charge_trigger_time,
            PCT_RATE_KW => self.pct_kw_rate,
            PCT_RATE_CHARGE => self.pct_charge_rate,
            PCT_RESERVE => self.pct_fleet_reserve,
            KW_NEED => self.kw_needed,
            T_UP => self.up_ramp_time,
            T_FLAT => self.flat_time,
            T_DN => self.dn_ramp_time,
            KW_THRESHOLD => self.f_kw_threshold,
            DISP_FACTOR => self.disp_factor,
            RESET_LEVEL => self.reset_level,
            BASE_FREQ => self.ccd.cd.base_frequency,
            _ => unreachable!("StorageController has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KW_TARGET => self.f_kw_target = value,
            KW_TARGET_LOW => self.f_kw_target_low = value,
            PCT_KW_BAND => self.f_pct_kw_band = value,
            KW_BAND => self.f_kw_band = value,
            PCT_KW_BAND_LOW => self.f_pct_kw_band_low = value,
            KW_BAND_LOW => self.f_kw_band_low = value,
            TIME_DISCHARGE_TRIGGER => self.discharge_trigger_time = value,
            TIME_CHARGE_TRIGGER => self.charge_trigger_time = value,
            PCT_RATE_KW => self.pct_kw_rate = value,
            PCT_RATE_CHARGE => self.pct_charge_rate = value,
            PCT_RESERVE => self.pct_fleet_reserve = value,
            // kWNeed is SilentReadOnly — writes are ignored.
            KW_NEED => {}
            T_UP => self.up_ramp_time = value,
            T_FLAT => self.flat_time = value,
            T_DN => self.dn_ramp_time = value,
            KW_THRESHOLD => self.f_kw_threshold = value,
            DISP_FACTOR => self.disp_factor = value,
            RESET_LEVEL => self.reset_level = value,
            BASE_FREQ => self.ccd.cd.base_frequency = value,
            _ => unreachable!("StorageController has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal,
            MON_PHASE => self.f_mon_phase,
            MODE_DISCHARGE => self.discharge_mode,
            MODE_CHARGE => self.charge_mode,
            INHIBIT_TIME => self.inhibit_hrs,
            SEASONS => self.seasons,
            _ => unreachable!("StorageController has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            TERMINAL => self.ccd.element_terminal = value,
            MON_PHASE => self.f_mon_phase = value,
            MODE_DISCHARGE => self.discharge_mode = value,
            MODE_CHARGE => self.charge_mode = value,
            INHIBIT_TIME => self.inhibit_hrs = value,
            SEASONS => self.seasons = value,
            _ => unreachable!("StorageController has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::EVENT_LOG => self.ccd.show_event_log,
            prop::ENABLED => self.ccd.cd.enabled,
            _ => unreachable!("StorageController has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::EVENT_LOG => self.ccd.show_event_log = value,
            prop::ENABLED => self.ccd.cd.enabled = value,
            _ => unreachable!("StorageController has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            ELEMENT => self.monitored_full_name.clone(),
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            // Read-only fleet aggregates (see module doc): always ''.
            KWH_TOTAL | KW_TOTAL | KWH_ACTUAL | KW_ACTUAL => String::new(),
            _ => unreachable!("StorageController has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, _value: String) {
        match idx {
            // SilentReadOnly fleet aggregates — writes are ignored.
            prop::KWH_TOTAL | prop::KW_TOTAL | prop::KWH_ACTUAL | prop::KW_ACTUAL => {}
            _ => unreachable!("StorageController has no writable string property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        match idx {
            prop::ELEMENT_LIST => self.storage_name_list.clone(),
            _ => unreachable!("StorageController has no string-list property {idx}"),
        }
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        match idx {
            prop::ELEMENT_LIST => self.storage_name_list = value,
            _ => unreachable!("StorageController has no string-list property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        use prop::*;
        match idx {
            // Pascal `FWeights` is NIL until an ElementList allocates it; a NIL
            // array dumps as "" (not "[]"), so report empty as absent.
            WEIGHTS => (!self.weights.is_empty()).then_some(self.weights.as_slice()),
            SEASON_TARGETS => Some(self.season_targets.as_slice()),
            SEASON_TARGETS_LOW => Some(self.season_targets_low.as_slice()),
            _ => unreachable!("StorageController has no double-array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        use prop::*;
        match idx {
            WEIGHTS => self.weights = value,
            SEASON_TARGETS => self.season_targets = value,
            SEASON_TARGETS_LOW => self.season_targets_low = value,
            _ => unreachable!("StorageController has no double-array property {idx}"),
        }
    }
    /// Pascal `Weights` IndirectCount: the element count comes from the
    /// ElementList (`PropertyOffset3 = @FStorageNameList`).
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::WEIGHTS => self.storage_name_list.len(),
            _ => unreachable!("StorageController has no function-sized array {idx}"),
        }
    }

    /// `element=`/`yearly=`/`daily=`/`duty=` resolution.
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use prop::*;
        match idx {
            ELEMENT => {
                self.monitored_full_name = name.clone();
                match resolved {
                    Some((r, obj)) => {
                        self.ccd.monitored_element = Some(r);
                        let elem = obj
                            .as_ckt_element()
                            .expect("element= resolves against circuit classes");
                        self.mon_snap = Some(RefSnapshot::capture(name, elem));
                    }
                    None => {
                        self.ccd.monitored_element = None;
                        self.mon_snap = None;
                    }
                }
            }
            // The shapes feed only the NOT_PORTED loadshape-dispatch mode; keep
            // the resolved name for the dump.
            YEARLY => self.yearly_shape = name,
            DAILY => self.daily_shape = name,
            DUTY => self.duty_shape = name,
            _ => unreachable!("StorageController has no object-ref property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.ccd.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.ccd.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TStorageControllerObj.PropertySideEffects`.
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            KW_TARGET => {
                let casemult = if self.discharge_mode == CURRENT_PEAKSHAVE {
                    1000.0
                } else {
                    1.0
                };
                self.f_kw_threshold = self.f_kw_target * 0.75 * casemult;
                self.half_kw_band = self.f_pct_kw_band / 200.0 * self.f_kw_target * casemult;
                self.f_kw_band = 2.0 * self.half_kw_band;
                self.f_pct_kw_band = self.f_kw_band / self.f_kw_target * 100.0; // sync
            }
            PCT_KW_BAND => {
                let casemult = if self.discharge_mode == CURRENT_PEAKSHAVE {
                    1000.0
                } else {
                    1.0
                };
                self.half_kw_band = self.f_pct_kw_band / 200.0 * self.f_kw_target * casemult;
                self.f_kw_band = 2.0 * self.half_kw_band;
                self.f_kw_band_specified = false;
            }
            KW_BAND => {
                let casemult = if self.discharge_mode == CURRENT_PEAKSHAVE {
                    1000.0
                } else {
                    1.0
                };
                self.half_kw_band = self.f_kw_band / 2.0 * casemult;
                self.f_pct_kw_band = self.f_kw_band / self.f_kw_target * 100.0; // sync
                self.f_kw_band_specified = true;
            }
            KW_TARGET_LOW | PCT_KW_BAND_LOW => {
                let casemult = if self.charge_mode == CURRENT_PEAKSHAVE_LOW {
                    1000.0
                } else {
                    1.0
                };
                self.half_kw_band_low =
                    self.f_pct_kw_band_low / 200.0 * self.f_kw_target_low * casemult;
                self.f_kw_band_low = self.half_kw_band_low * 2.0;
            }
            KW_BAND_LOW => {
                let casemult = if self.charge_mode == CURRENT_PEAKSHAVE_LOW {
                    1000.0
                } else {
                    1.0
                };
                self.half_kw_band_low = self.f_kw_band_low / 2.0 * casemult;
                // TODO(compat): Pascal writes FpctkWBand (not FpctkWBandLow)
                // here (StorageController.pas l.544) — an upstream typo; the
                // clean fix targets FpctkWBandLow.
                self.f_pct_kw_band = self.f_kw_band_low / self.f_kw_target * 100.0;
            }
            MODE_DISCHARGE => {
                if self.discharge_mode == MODE_FOLLOW {
                    self.discharge_trigger_time = 12.0; // Noon
                }
            }
            MON_PHASE => {
                if self.f_mon_phase > self.ccd.cd.nphases as i32 {
                    self.ccd.cd.obj.push_error(format!(
                        "Error: Monitored phase ({}) must be less than or equal to number of phases ({}). ",
                        self.f_mon_phase, self.ccd.cd.nphases
                    ));
                    self.f_mon_phase = 1;
                }
            }
            ELEMENT_LIST => {
                // Levelize the list.
                self.fleet_list_changed = true;
                self.element_list_specified = true;
                self.fleet_size = self.storage_name_list.len() as i32;
                self.weights = vec![1.0; self.fleet_size.max(0) as usize];
            }
            SEASONS => {
                let n = self.seasons.max(0) as usize;
                self.season_targets.resize(n, 0.0);
                self.season_targets_low.resize(n, 0.0);
            }
            DISP_FACTOR => {
                if self.disp_factor <= 0.0 || self.disp_factor > 1.0 {
                    self.disp_factor = 1.0;
                }
            }
            INHIBIT_TIME => self.inhibit_hrs = self.inhibit_hrs.max(1),
            _ => {}
        }
    }

    /// Pascal `TCktElementClass.EndEdit` default → `RecalcElementData`.
    fn end_edit(&mut self) {
        self.recalc();
    }

    /// Pascal `TStorageControllerObj.MakeLike` — copies essentially every
    /// dispatch setting (unlike GenDispatcher's terminal-only copy).
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<StorageController>() else {
            return;
        };
        self.ccd.cd.make_like_base(&other.ccd.cd);
        self.ccd.cd.nphases = other.ccd.cd.nphases;
        let nc = other.ccd.cd.nconds;
        self.ccd.cd.set_nconds(nc); // Force Reallocation of terminal stuff

        self.ccd.monitored_element = other.ccd.monitored_element;
        self.monitored_full_name = other.monitored_full_name.clone();
        self.mon_snap = other.mon_snap.clone();
        self.ccd.element_terminal = other.ccd.element_terminal;
        self.f_mon_phase = other.f_mon_phase;

        self.f_kw_target = other.f_kw_target;
        self.f_kw_target_low = other.f_kw_target_low;
        self.f_kw_threshold = other.f_kw_threshold;
        self.disp_factor = other.disp_factor;
        self.f_pct_kw_band = other.f_pct_kw_band;
        self.f_kw_band = other.f_kw_band;
        self.f_pct_kw_band_low = other.f_pct_kw_band_low;
        self.f_kw_band_low = other.f_kw_band_low;
        self.reset_level = other.reset_level;
        self.f_kw_band_specified = other.f_kw_band_specified;

        self.storage_name_list = other.storage_name_list.clone();
        self.fleet_size = self.storage_name_list.len() as i32;
        if self.fleet_size > 0 {
            self.weights = other.weights.clone();
        }

        self.discharge_mode = other.discharge_mode;
        self.charge_mode = other.charge_mode;
        self.discharge_trigger_time = other.discharge_trigger_time;
        self.charge_trigger_time = other.charge_trigger_time;
        self.pct_kw_rate = other.pct_kw_rate;
        self.pct_charge_rate = other.pct_charge_rate;
        self.pct_fleet_reserve = other.pct_fleet_reserve;
        self.yearly_shape = other.yearly_shape.clone();
        self.daily_shape = other.daily_shape.clone();
        self.duty_shape = other.duty_shape.clone();
        self.ccd.show_event_log = other.ccd.show_event_log;
        self.inhibit_hrs = other.inhibit_hrs;
        self.up_ramp_time = other.up_ramp_time;
        self.flat_time = other.flat_time;
        self.dn_ramp_time = other.dn_ramp_time;

        self.seasons = other.seasons;
        if self.seasons > 1 {
            self.season_targets = other.season_targets.clone();
            self.season_targets_low = other.season_targets_low.clone();
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::props::PropType;

    #[test]
    fn default_shape_and_defaults() {
        let sc = StorageController::new("sc1");
        assert_eq!(sc.ccd.cd.nphases, 3);
        assert_eq!(sc.ccd.cd.nconds, 3);
        assert_eq!(sc.ccd.cd.nterms, 1);
        assert_eq!(sc.ccd.element_terminal, 1);
        assert_eq!(sc.f_mon_phase, MAXPHASE);
        assert_eq!(sc.f_kw_target, 8000.0);
        assert_eq!(sc.f_kw_target_low, 4000.0);
        assert_eq!(sc.f_kw_threshold, 6000.0);
        assert_eq!(sc.f_kw_band, 160.0);
        assert_eq!(sc.f_kw_band_low, 80.0);
        assert_eq!(sc.discharge_mode, MODE_PEAKSHAVE);
        assert_eq!(sc.charge_mode, MODE_TIME);
        assert_eq!(sc.seasons, 1);
        assert_eq!(sc.season_targets, vec![8000.0]);
        assert_eq!(sc.season_targets_low, vec![4000.0]);
        assert!(sc.ccd.cd.yprim.is_none());
    }

    #[test]
    fn prop_table_shape_matches_pascal() {
        let enums = EnumRegistry::new();
        let cp = class_props(&enums);
        // 37 class props + basefreq + enabled + Like.
        assert_eq!(cp.num_properties(), prop::NUM_PROPS);
        // Spot-check a few key types.
        assert_eq!(cp.prop(prop::ELEMENT).ptype, PropType::ObjectRef);
        assert_eq!(cp.prop(prop::MON_PHASE).ptype, PropType::MappedStringEnum);
        assert_eq!(cp.prop(prop::WEIGHTS).ptype, PropType::DoubleVArray);
        assert_eq!(cp.prop(prop::SEASON_TARGETS).ptype, PropType::DoubleArray);
        assert_eq!(cp.prop(prop::KWH_TOTAL).ptype, PropType::String);
    }

    #[test]
    fn kw_target_side_effect_syncs_band_and_threshold() {
        // kWTarget=5000 with default %kWBand=2: threshold=5000*0.75=3750,
        // HalfkWBand=2/200*5000=50, kWBand=100, pctkWBand re-synced to 2.
        let mut sc = StorageController::new("sc1");
        sc.set_f64(prop::KW_TARGET, 5000.0);
        sc.side_effects(prop::KW_TARGET, 0);
        assert_eq!(sc.f_kw_threshold, 3750.0);
        assert_eq!(sc.f_kw_band, 100.0);
        assert_eq!(sc.f_pct_kw_band, 2.0);
    }

    #[test]
    fn pct_kw_band_side_effect_sets_kw_band() {
        // %kWBand=5 over kWTarget=5000: HalfkWBand=5/200*5000=125, kWBand=250.
        let mut sc = StorageController::new("sc1");
        sc.set_f64(prop::KW_TARGET, 5000.0);
        sc.side_effects(prop::KW_TARGET, 0);
        sc.set_f64(prop::PCT_KW_BAND, 5.0);
        sc.side_effects(prop::PCT_KW_BAND, 0);
        assert_eq!(sc.f_kw_band, 250.0);
        assert!(!sc.f_kw_band_specified);
    }

    #[test]
    fn mode_discharge_follow_sets_noon_trigger() {
        let mut sc = StorageController::new("sc1");
        sc.set_i32(prop::MODE_DISCHARGE, MODE_FOLLOW);
        sc.side_effects(prop::MODE_DISCHARGE, 0);
        assert_eq!(sc.discharge_trigger_time, 12.0);
    }

    #[test]
    fn element_list_side_effect_levelizes_weights() {
        let mut sc = StorageController::new("sc1");
        sc.set_string_list(prop::ELEMENT_LIST, vec!["sa".into(), "sb".into()]);
        sc.side_effects(prop::ELEMENT_LIST, 0);
        assert!(sc.element_list_specified);
        assert!(sc.fleet_list_changed);
        assert_eq!(sc.fleet_size, 2);
        assert_eq!(sc.weights, vec![1.0, 1.0]);
        assert_eq!(sc.array_size(prop::WEIGHTS), 2);
    }

    #[test]
    fn seasons_side_effect_resizes_targets() {
        let mut sc = StorageController::new("sc1");
        sc.set_i32(prop::SEASONS, 3);
        sc.side_effects(prop::SEASONS, 0);
        assert_eq!(sc.season_targets.len(), 3);
        assert_eq!(sc.season_targets_low.len(), 3);
    }

    #[test]
    fn disp_factor_out_of_range_resets_to_one() {
        let mut sc = StorageController::new("sc1");
        sc.set_f64(prop::DISP_FACTOR, 1.5);
        sc.side_effects(prop::DISP_FACTOR, 0);
        assert_eq!(sc.disp_factor, 1.0);
        sc.set_f64(prop::DISP_FACTOR, 0.8);
        sc.side_effects(prop::DISP_FACTOR, 0);
        assert_eq!(sc.disp_factor, 0.8);
    }

    #[test]
    fn inhibit_time_floored_at_one() {
        let mut sc = StorageController::new("sc1");
        sc.set_i32(prop::INHIBIT_TIME, 0);
        sc.side_effects(prop::INHIBIT_TIME, 0);
        assert_eq!(sc.inhibit_hrs, 1);
    }

    #[test]
    fn recalc_missing_element_errors_372() {
        let mut sc = StorageController::new("sc1");
        sc.recalc();
        let errs = sc.ccd.cd.obj.take_errors();
        assert!(errs.iter().any(|e| e.contains("is not set")));
    }

    #[test]
    fn make_fleet_list_default_empty_errors_37201() {
        // No ElementList → default branch finds no Storage → 37201.
        let mut sc = StorageController::new("sc1");
        assert!(!sc.make_fleet_list());
    }

    #[test]
    fn default_recalc_clears_fleet_flag_no_repeat_37201() {
        // Pascal's MakeFleetList default branch clears FleetListChanged, so a
        // *second* RecalcElementData (a re-Edit) must not re-run the fleet build
        // nor re-emit 37201.
        let mut sc = StorageController::new("sc1");
        sc.mon_snap = Some(RefSnapshot {
            full_name: "line.l1".into(),
            nphases: 3,
            nterms: 2,
            buses: vec!["b1".into(), "b2".into()],
        });
        sc.recalc();
        let first = sc.ccd.cd.obj.take_errors();
        assert_eq!(
            first
                .iter()
                .filter(|e| e.contains("No unassigned Storage"))
                .count(),
            1,
            "{first:?}"
        );
        assert!(
            !sc.fleet_list_changed,
            "default branch must clear the rebuild flag"
        );
        // Second recalc: flag cleared → no rebuild → no fresh 37201.
        sc.recalc();
        let second = sc.ccd.cd.obj.take_errors();
        assert!(
            second.iter().all(|e| !e.contains("No unassigned Storage")),
            "second recalc re-emitted 37201: {second:?}"
        );
    }

    #[test]
    fn named_missing_recalc_keeps_flag_pending() {
        // The named branch Exits *before* clearing FleetListChanged (Pascal
        // l.1889), so the rebuild stays pending and a re-Edit re-emits 14403.
        let mut sc = StorageController::new("sc1");
        sc.set_string_list(prop::ELEMENT_LIST, vec!["sa".into()]);
        sc.side_effects(prop::ELEMENT_LIST, 0);
        assert!(!sc.make_fleet_list());
        assert!(
            sc.fleet_list_changed,
            "missing-name path must leave the rebuild flag set"
        );
    }

    #[test]
    fn mon_phase_above_nphases_errors_and_resets() {
        // Default nphases = 3; MonPhase = 4 is out of range → error + reset to 1.
        let mut sc = StorageController::new("sc1");
        sc.set_i32(prop::MON_PHASE, 4);
        sc.side_effects(prop::MON_PHASE, 0);
        assert_eq!(sc.f_mon_phase, 1);
        let errs = sc.ccd.cd.obj.take_errors();
        assert!(
            errs.iter().any(|e| e.contains("Monitored phase")),
            "{errs:?}"
        );
    }

    #[test]
    fn recalc_syncs_nphases_to_monitored_element() {
        // Pascal: FNphases := MonitoredElement.Nphases; NConds := FNphases.
        let mut sc = StorageController::new("sc1");
        assert_eq!(sc.ccd.cd.nphases, 3); // ctor default
        sc.mon_snap = Some(RefSnapshot {
            full_name: "line.l1".into(),
            nphases: 1,
            nterms: 2,
            buses: vec!["b1".into(), "b2".into()],
        });
        sc.recalc();
        assert_eq!(sc.ccd.cd.nphases, 1);
        assert_eq!(sc.ccd.cd.nconds, 1);
        assert_eq!(sc.get_bus_name(1), "b1");
        // A subsequent MonPhase=2 now validates against the synced nphases (1)
        // and is rejected, instead of silently passing against the default 3.
        sc.set_i32(prop::MON_PHASE, 2);
        sc.side_effects(prop::MON_PHASE, 0);
        assert_eq!(sc.f_mon_phase, 1);
    }

    #[test]
    fn make_fleet_list_named_missing_errors_14403() {
        let mut sc = StorageController::new("sc1");
        sc.set_string_list(prop::ELEMENT_LIST, vec!["sa".into(), "sb".into()]);
        sc.side_effects(prop::ELEMENT_LIST, 0);
        assert!(!sc.make_fleet_list());
        let errs = sc.ccd.cd.obj.take_errors();
        assert!(errs.iter().any(|e| e.contains("\"sa\" not found")));
    }

    #[test]
    fn make_like_copies_dispatch_settings() {
        let mut base = StorageController::new("base");
        base.f_kw_target = 5000.0;
        base.f_kw_band = 250.0;
        base.f_pct_kw_band = 5.0;
        base.discharge_mode = MODE_FOLLOW;
        base.discharge_trigger_time = 12.0;
        base.pct_fleet_reserve = 20.0;
        base.seasons = 2;
        base.season_targets = vec![5000.0, 4500.0];
        base.season_targets_low = vec![2500.0, 2200.0];
        base.ccd.element_terminal = 1;

        let mut sc = StorageController::new("sc1");
        sc.make_like(&base);
        assert_eq!(sc.f_kw_target, 5000.0);
        assert_eq!(sc.f_kw_band, 250.0);
        assert_eq!(sc.f_pct_kw_band, 5.0);
        assert_eq!(sc.discharge_mode, MODE_FOLLOW);
        assert_eq!(sc.discharge_trigger_time, 12.0);
        assert_eq!(sc.pct_fleet_reserve, 20.0);
        assert_eq!(sc.seasons, 2);
        assert_eq!(sc.season_targets, vec![5000.0, 4500.0]);
        assert_eq!(sc.season_targets_low, vec![2500.0, 2200.0]);
    }
}
