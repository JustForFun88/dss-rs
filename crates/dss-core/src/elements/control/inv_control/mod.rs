//! Port of `Controls/InvControl.pas` — `TInvControlObj`, the smart-inverter
//! control over a fleet of PVSystem/Storage (`TInvBasedPCE`) elements. It is the
//! single largest unit in Phase 7 (3586 lines, eight control modes:
//! VOLTVAR / VOLTWATT / DRC / WATTPF / WATTVAR / AVR / GFM + the VV_VW / VV_DRC
//! combi modes), so the port lands in sub-steps (PHASE7_PLAN WP7.5 step 2):
//!
//! - **step 2a (this commit) — the parse-only skeleton.** The class, its 34
//!   properties (`define_properties!`), the seven smart-inverter enums, `Create`
//!   defaults, `PropertySideEffects` (incl. the `ValidateXYCurve` curve
//!   range-checks and the DbVMin/DbVMax/LPFTau/RiseFallLimit guards), `MakeLike`,
//!   and class registration. The DER fleet (`MakeDERList`), the per-DER runtime
//!   state (`TInvVars`), and the control behavior (`Sample` / `DoPendingAction` /
//!   the `Calc*` per-mode math) are **deferred to step 2b+**: until then a
//!   sampled InvControl records an explicit NOT_PORTED error (never a silent
//!   no-op — `solution/controls/dispatch.rs`).
//!
//! Like every `TControlElem`, an InvControl builds **no Yprim** and its terminal
//! currents are zero. Its single terminal attaches to the first DER element's
//! bus (deferred to step 2b's `RecalcElementData`).
//!
//! The five control curves (`VVC_Curve1` / `VoltWatt_Curve` / `VoltWattCH_Curve`
//! / `WattPF_Curve` / `WattVar_Curve`) resolve to `XYcurve` objects via the
//! snapshot-clone ObjectRef pattern (WP4.2). `PropertySideEffects` runs Pascal's
//! `ValidateXYCurve`: a curve whose per-unit Y values fall outside the mode's
//! valid band (VOLTWATT/WATTPF/WATTVAR) is dropped (set to `None`) with error
//! 381 — VOLTVAR (`VVC_Curve1`) is unchecked.
//!
//! **Deferred to step 2b+ (the behavior):** `MakeDERList` (the
//! PVSystem/Storage fleet resolution), `RecalcElementData`'s
//! bus/monitored-element setup, the `monBus` per-bus node parsing
//! (`FMonBuses`/`FMonBusesNodes` — consumed only by `Sample`'s `GetMonVoltage`),
//! and the entire `Sample`/`DoPendingAction`/`Reset` dispatch. **NOT_PORTED:**
//! `MakePosSequence`.

mod accessors;
#[cfg(test)]
mod tests;

use crate::elements::control::control_elem::ControlElemData;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

// Control-mode ordinals (InvControl.pas `TInvControlControlMode`). The full set
// is VOLTVAR=1 VOLTWATT=2 DRC=3 WATTPF=4 WATTVAR=5 AVR=6 GFM=7; only the ones
// the step-2a curve-validation references are bound here (the rest arrive with
// the step-2b dispatch).
pub(crate) const NONE_MODE: i32 = 0;
pub(crate) const VOLTWATT: i32 = 2;
pub(crate) const WATTPF: i32 = 4;
pub(crate) const WATTVAR: i32 = 5;

// Combi-mode ordinals (InvControl.pas `TInvControlCombiMode`).
pub(crate) const NONE_COMBMODE: i32 = 0;

// Rate-of-change-mode ordinals (InvControl.pas `ERateofChangeMode`).
pub(crate) const ROC_INACTIVE: i32 = 0;

// Reactive-power-reference ordinals (InvControl.pas constants).
const REAC_POWER_VARAVAL: i32 = 0;

// Monitored-phase sentinel (`DSSClass.pas`; reused via MonPhaseEnum).
const AVGPHASES: i32 = -1;

// Control-model ordinals (InvControl.pas `TInvControlModel`).
const MODEL_LINEAR: i32 = 0;

// FLAGDELTAQ / FLAGDELTAP — the "not set" sentinels (InvControl.pas l.417-418).
pub(crate) const FLAGDELTAQ: f64 = -1.0;
pub(crate) const FLAGDELTAP: f64 = -1.0;

/// 1-based property ordinals (Pascal `TInvControlProp` + the `TCktElementClass`
/// tail). The legacy and modern Pascal names differ only in case, so the
/// case-insensitive property matcher accepts both spellings; the modern
/// (`TProp`) names below are what the text dump renders.
pub mod prop {
    pub const DER_LIST: usize = 1;
    pub const MODE: usize = 2;
    pub const COMBI_MODE: usize = 3;
    pub const VVC_CURVE1: usize = 4;
    pub const HYSTERESIS_OFFSET: usize = 5;
    pub const VOLTAGE_CURVEX_REF: usize = 6;
    pub const AVG_WINDOW_LEN: usize = 7;
    pub const VOLTWATT_CURVE: usize = 8;
    pub const DBV_MIN: usize = 9;
    pub const DBV_MAX: usize = 10;
    pub const AR_GRA_LOW_V: usize = 11;
    pub const AR_GRA_HI_V: usize = 12;
    pub const DYN_REAC_AVG_WINDOW_LEN: usize = 13;
    pub const DELTA_Q_FACTOR: usize = 14;
    pub const VOLTAGE_CHANGE_TOLERANCE: usize = 15;
    pub const VAR_CHANGE_TOLERANCE: usize = 16;
    pub const VOLTWATT_YAXIS: usize = 17;
    pub const RATE_OF_CHANGE_MODE: usize = 18;
    pub const LPF_TAU: usize = 19;
    pub const RISE_FALL_LIMIT: usize = 20;
    pub const DELTA_P_FACTOR: usize = 21;
    pub const EVENT_LOG: usize = 22;
    pub const REF_REACTIVE_POWER: usize = 23;
    pub const ACTIVE_P_CHANGE_TOLERANCE: usize = 24;
    pub const MON_VOLTAGE_CALC: usize = 25;
    pub const MON_BUS: usize = 26;
    pub const MON_BUSES_VBASE: usize = 27;
    pub const VOLTWATTCH_CURVE: usize = 28;
    pub const WATTPF_CURVE: usize = 29;
    pub const WATTVAR_CURVE: usize = 30;
    pub const VV_REF_REACTIVE_POWER: usize = 31;
    pub const PVSYSTEM_LIST: usize = 32;
    pub const VSETPOINT: usize = 33;
    pub const CONTROL_MODEL: usize = 34;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 35;
    pub const ENABLED: usize = 36;
    pub const NUM_PROPS: usize = 37; // incl. Like
}

/// `TInvControl.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::string_list("DERList"),
        // Mode/CombiMode are NoDefault (NONE_MODE/NONE_COMBMODE render '').
        PropDef::mapped_string_enum("Mode", enums.invcontrol_mode).flags(PropFlags::NO_DEFAULT),
        PropDef::mapped_string_enum("CombiMode", enums.invcontrol_combi)
            .flags(PropFlags::NO_DEFAULT),
        PropDef::object_ref_class("XYCurve", "VVC_Curve1"),
        PropDef::double("Hysteresis_Offset").flags(PropFlags::NON_POSITIVE),
        PropDef::mapped_string_enum("Voltage_CurveX_Ref", enums.invcontrol_voltage_curvex),
        PropDef::integer("AvgWindowLen"),
        PropDef::object_ref_class("XYCurve", "VoltWatt_Curve"),
        PropDef::double("DbVMin"),
        PropDef::double("DbVMax"),
        PropDef::double("ArGraLowV"),
        PropDef::double("ArGraHiV"),
        PropDef::integer("DynReacAvgWindowLen"),
        PropDef::double("DeltaQ_Factor"),
        PropDef::double("VoltageChangeTolerance"),
        PropDef::double("VarChangeTolerance"),
        PropDef::mapped_string_enum("VoltWattYAxis", enums.invcontrol_voltwatt_yaxis),
        PropDef::mapped_string_enum("RateOfChangeMode", enums.invcontrol_roc),
        PropDef::double("LPFTau"),
        PropDef::double("RiseFallLimit"),
        PropDef::double("DeltaP_Factor"),
        PropDef::boolean("EventLog"),
        PropDef::mapped_string_enum("RefReactivePower", enums.invcontrol_reac_power),
        PropDef::double("ActivePChangeTolerance"),
        PropDef::mapped_string_enum("MonVoltageCalc", enums.mon_phase),
        PropDef::string_list("MonBus"),
        // SizeIsFunction → array_size = MonBusesNameList.Count.
        PropDef::double_v_array("MonBusesVBase"),
        PropDef::object_ref_class("XYCurve", "VoltWattCH_Curve"),
        PropDef::object_ref_class("XYCurve", "WattPF_Curve"),
        PropDef::object_ref_class("XYCurve", "WattVar_Curve"),
        // Pascal DeprecatedAndRemoved — present in the table (occupies an
        // ordinal) but its `?` getter renders '' and a write does nothing.
        // Modeled as a read-only '' string, like Storage's `%Idlingkvar`.
        PropDef::string("VV_RefReactivePower"),
        // Pascal Redundant+Deprecated, PropertyOffset = @DERNameList (shares the
        // DERList backing): a write prepends "PVSystem." then re-runs the DERList
        // side effect.
        PropDef::string_list("PVSystemList").flags(PropFlags::REDUNDANT),
        PropDef::double("VSetPoint"),
        PropDef::mapped_int_enum("ControlModel", enums.invcontrol_model),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("InvControl", defs, true)
}

/// `TInvControlObj` — the parse-time surface (step 2a). The DER fleet, the
/// per-DER `TInvVars` runtime state, and the `Sample`/dispatch machinery land in
/// step 2b.
#[derive(Debug, Clone)]
pub struct InvControl {
    pub ccd: ControlElemData,

    /// `DERNameList` — the named PVSystem/Storage fleet (also the backing of the
    /// deprecated `PVSystemList`).
    der_name_list: Vec<String>,
    /// `FListSize` — set from `DERNameList.count` in the DERList side effect.
    f_list_size: i32,

    /// `ControlMode` (`TInvControlControlMode` ordinal; NONE_MODE renders '').
    control_mode: i32,
    /// `CombiMode` (`TInvControlCombiMode` ordinal).
    combi_mode: i32,

    /// `Fvvc_curve` — volt-var curve (name + snapshot-clone).
    vvc_curve_name: String,
    vvc_curve: Option<XyCurveObj>,
    /// `Fvvc_curveOffset` (Hysteresis_Offset).
    vvc_curve_offset: f64,
    /// `FVoltage_CurveX_ref` (0:=Rated, 1:=Avg, 2:=RAvg).
    voltage_curvex_ref: i32,
    /// `FRollAvgWindowLength` (AvgWindowLen, seconds).
    roll_avg_window_length: i32,

    /// `Fvoltwatt_curve` — volt-watt curve.
    voltwatt_curve_name: String,
    voltwatt_curve: Option<XyCurveObj>,
    /// `FvoltwattCH_curve` — volt-watt (charging) curve.
    voltwattch_curve_name: String,
    voltwattch_curve: Option<XyCurveObj>,
    /// `Fwattpf_curve` — watt-pf curve.
    wattpf_curve_name: String,
    wattpf_curve: Option<XyCurveObj>,
    /// `Fwattvar_curve` — watt-var curve.
    wattvar_curve_name: String,
    wattvar_curve: Option<XyCurveObj>,
    /// `pf_wp_nominal` — watt-pf nominal pf (computed during Sample, step 2b);
    /// carried for `Create`/`MakeLike` fidelity.
    pf_wp_nominal: f64,

    /// `FDbVMin` / `FDbVMax` — DRC dead-band.
    dbv_min: f64,
    dbv_max: f64,
    /// `FArGraLowV` / `FArGraHiV` — DRC slopes.
    ar_gra_low_v: f64,
    ar_gra_hi_v: f64,
    /// `FDRCRollAvgWindowLength` (DynReacAvgWindowLen, seconds).
    drc_roll_avg_window_length: i32,

    /// `FdeltaQ_factor` / `FdeltaP_factor` — convergence damping (FLAG* = unset).
    delta_q_factor: f64,
    delta_p_factor: f64,
    /// `FVoltageChangeTolerance` / `FVarChangeTolerance` / `FActivePChangeTolerance`.
    voltage_change_tolerance: f64,
    var_change_tolerance: f64,
    active_p_change_tolerance: f64,

    /// `FVoltwattYAxis` (0:=%Available, 1:=%Pmpp, 2:=%PctPmpp, 3:=%kVArating).
    voltwatt_yaxis: i32,
    /// `RateofChangeMode` (`ERateofChangeMode` ordinal).
    rate_of_change_mode: i32,
    /// `LPFTau` (seconds) / `FRiseFallLimit`.
    lpf_tau: f64,
    rise_fall_limit: f64,

    /// `FReacPower_ref` (0:=VARAVAL, 1:=VARMAX).
    reac_power_ref: i32,

    /// `FMonBusesPhase` (MonVoltageCalc; MonPhaseEnum: avg/max/min/phase no.).
    mon_buses_phase: i32,
    /// `MonBusesNameList` — the monitored bus list.
    mon_buses_name_list: Vec<String>,
    /// `FMonBusesVbase` — one base-kV per monitored bus (array_size =
    /// MonBusesNameList.Count).
    mon_buses_vbase: Vec<f64>,

    /// `Fv_setpoint` — AVR voltage setpoint.
    v_setpoint: f64,
    /// `CtrlModel` (`TInvControlModel`: Linear / Exponential).
    ctrl_model: i32,
}

impl InvControl {
    /// Pascal `TInvControlObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;
        ccd.show_event_log = false; // match SVN r3458

        Self {
            ccd,
            der_name_list: Vec::new(),
            f_list_size: 0,
            control_mode: NONE_MODE, // docs say "VoltVar" but Create sets NONE
            combi_mode: NONE_COMBMODE,

            vvc_curve_name: String::new(),
            vvc_curve: None,
            vvc_curve_offset: 0.0,
            voltage_curvex_ref: 0,
            roll_avg_window_length: 1, // docs list 0; Create sets 1

            voltwatt_curve_name: String::new(),
            voltwatt_curve: None,
            voltwattch_curve_name: String::new(),
            voltwattch_curve: None,
            wattpf_curve_name: String::new(),
            wattpf_curve: None,
            wattvar_curve_name: String::new(),
            wattvar_curve: None,
            pf_wp_nominal: 0.0,

            dbv_min: 0.95,
            dbv_max: 1.05,
            ar_gra_low_v: 0.1,
            ar_gra_hi_v: 0.1,
            drc_roll_avg_window_length: 1,

            delta_q_factor: FLAGDELTAQ,
            delta_p_factor: FLAGDELTAP,
            voltage_change_tolerance: 0.0001,
            var_change_tolerance: 0.025,
            active_p_change_tolerance: 0.01,

            voltwatt_yaxis: 1,
            rate_of_change_mode: ROC_INACTIVE,
            lpf_tau: 0.001,         // docs list 0
            rise_fall_limit: 0.001, // docs list -1 (disabled)

            reac_power_ref: REAC_POWER_VARAVAL,

            mon_buses_phase: AVGPHASES,
            mon_buses_name_list: Vec::new(),
            mon_buses_vbase: Vec::new(),

            v_setpoint: 1.0,
            ctrl_model: MODEL_LINEAR,
        }
    }
}
