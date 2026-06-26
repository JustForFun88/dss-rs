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
mod compute;
#[cfg(test)]
mod tests;

pub(crate) use compute::{DerSnap, FleetFind as InvFleetFind, InvDispatchEnv, MonitorVar};

use crate::elements::control::control_elem::ControlElemData;
use crate::elements::control::roll_avg_window::RollAvgWindow;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::ElemRef;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

// Control-mode ordinals (InvControl.pas `TInvControlControlMode`). The full set
// is VOLTVAR=1 VOLTWATT=2 DRC=3 WATTPF=4 WATTVAR=5 AVR=6 GFM=7; the dispatch ports
// land per sub-step (2b: VOLTVAR; 2c: VOLTWATT + VV_VW; 2d: DRC + VV_DRC; 2e: the rest).
pub(crate) const NONE_MODE: i32 = 0;
pub(crate) const VOLTVAR: i32 = 1;
pub(crate) const VOLTWATT: i32 = 2;
pub(crate) const DRC: i32 = 3;
pub(crate) const WATTPF: i32 = 4;
pub(crate) const WATTVAR: i32 = 5;
// AVR=6 (active voltage regulation) — dispatch lands with step 2e-ii. Until then
// the Sample mode guard rejects it via the catch-all arm (so the ordinal is read
// only by the deferred-AVR test); `allow(dead_code)` keeps the non-test lib build
// quiet without hiding it from 2e-ii.
#[allow(dead_code)]
pub(crate) const AVR: i32 = 6;

// Combi-mode ordinals (InvControl.pas `TInvControlCombiMode`).
pub(crate) const NONE_COMBMODE: i32 = 0;
pub(crate) const VV_VW: i32 = 1;
pub(crate) const VV_DRC: i32 = 2;

// Rate-of-change-mode ordinals (InvControl.pas `ERateofChangeMode`). LPF=1 /
// RISEFALL=2 arrive with the rate-of-change dispatch (step 2e); only the
// INACTIVE default is bound here (the VOLTVAR step-2b guard).
pub(crate) const ROC_INACTIVE: i32 = 0;

// PendingChange action codes (InvControl.pas l.407-411).
pub(crate) const CHANGE_NONE: i32 = 0;
pub(crate) const CHANGEVARLEVEL: i32 = 1;
pub(crate) const CHANGEWATTLEVEL: i32 = 2;
pub(crate) const CHANGEWATTVARLEVEL: i32 = 3;
pub(crate) const CHANGEDRCVVARLEVEL: i32 = 4;

// Reactive-power-reference ordinals (InvControl.pas constants).
const REAC_POWER_VARAVAL: i32 = 0;
pub(crate) const REAC_POWER_VARMAX: i32 = 1;

// Monitored-phase sentinels (`DSSClass.pas`; reused via MonPhaseEnum).
const AVGPHASES: i32 = -1;
pub(crate) const MAXPHASE: i32 = -2;
pub(crate) const MINPHASE: i32 = -3;

// Control-model ordinals (InvControl.pas `TInvControlModel`).
const MODEL_LINEAR: i32 = 0;

// FLAGDELTAQ / FLAGDELTAP — the "not set" sentinels (InvControl.pas l.417-418).
pub(crate) const FLAGDELTAQ: f64 = -1.0;
pub(crate) const FLAGDELTAP: f64 = -1.0;
// Pascal DELTAQDEFAULT/DELTAPDEFAULT (l.419-420) — the initial adaptive factor.
pub(crate) const DELTAQDEFAULT: f64 = 0.5;
pub(crate) const DELTAPDEFAULT: f64 = 0.5;

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
        PropDef::integer("AvgWindowLen").flags(PropFlags::INTERVAL_UNITS),
        PropDef::object_ref_class("XYCurve", "VoltWatt_Curve"),
        PropDef::double("DbVMin"),
        PropDef::double("DbVMax"),
        PropDef::double("ArGraLowV"),
        PropDef::double("ArGraHiV"),
        PropDef::integer("DynReacAvgWindowLen").flags(PropFlags::INTERVAL_UNITS),
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

/// Pascal `TInvVars` — the per-controlled-DER runtime state (one record per fleet
/// member). Only the fields the WP7.5 **step 2b–2d** dispatch (VOLTVAR / VOLTWATT /
/// VV_VW / DRC / VV_DRC) + the shared machinery (`UpdateInvControl`,
/// `Calc_QHeadRoom`, `Change_deltaQ_factor`, `UpdateDERParameters`) read/write are
/// carried; the WATTPF/WATTVAR/AVR-only fields (`QDesiredWP`/`QDesiredWV`/...) land
/// with sub-step 2e. Field names mirror the Pascal record for a 1:1 read.
#[derive(Debug, Clone, Default)]
pub(crate) struct InvVars {
    /// `CondOffset` — monitored-terminal conductor offset (`(NTerms-1)*NCondsDER`).
    pub cond_offset: usize,
    pub nphases_der: usize,
    pub nconds_der: usize,

    // --- voltages (Sample / UpdateInvControl) ---
    pub f_avgp_vpu_prior: f64,
    pub f_avgp_drc_vpu_prior: f64,
    pub f_present_vpu: f64,
    pub f_present_drc_vpu: f64,

    // --- volt-var reactive-power state ---
    /// `QDesiredVV` — the volt-var kvar set-point pushed to the DER.
    pub q_desired_vv: f64,
    pub q_old: f64,
    pub q_old_vv: f64,
    pub q_headroom: f64,
    pub q_headroom_neg: f64,
    pub qoutputpu: f64,
    pub qoutput_vvpu: f64,
    /// `QDesireEndpu` — Q (pu) used in the convergence algorithm.
    pub q_desire_endpu: f64,
    /// `QDesireVVpu` — Q desired from the volt-var curve (pu of headroom).
    pub q_desire_vvpu: f64,
    /// `QDesireLimitedpu` — Q after the kVA / kvarlimit clamp (`Check_Qlimits`).
    pub q_desire_limitedpu: f64,
    /// `FdeltaQFactor` — the adaptive convergence damping factor.
    pub f_delta_q_factor: f64,
    /// `DeltaV_old` — prior |ΔVpu| (drives `Change_deltaQ_factor`).
    pub delta_v_old: f64,
    /// `FVVOperation` — volt-var operating flag (-1 absorb / 1 inject / 0 none).
    pub f_vv_operation: f64,

    // --- DRC / VV_DRC reactive-power state (sub-step 2d) ---
    /// `QDesiredDRC` — the DRC kvar set-point pushed to the DER.
    pub q_desired_drc: f64,
    /// `QDesiredVVDRC` — the VV_DRC combi kvar set-point pushed to the DER.
    pub q_desired_vvdrc: f64,
    /// `QOldDRC` / `QOldVVDRC` — the prior DRC / VV_DRC kvar (convergence history;
    /// start at -1.0 like `QOldVV`).
    pub q_old_drc: f64,
    pub q_old_vvdrc: f64,
    /// `QoutputDRCpu` / `QoutputVVDRCpu` — the achieved Q (pu) used in the DRC /
    /// VV_DRC trigger comparison.
    pub qoutput_drcpu: f64,
    pub qoutput_vvdrcpu: f64,
    /// `QDesireDRCpu` — Q desired from the DRC dynamic-reactive-current law (pu).
    pub q_desire_drcpu: f64,
    /// `FDRCOperation` / `FVVDRCOperation` — DRC / VV_DRC operating flags.
    pub f_drc_operation: f64,
    pub f_vvdrc_operation: f64,

    // --- WATTPF / WATTVAR reactive-power state (sub-step 2e-i) ---
    /// `QDesiredWP` / `QDesiredWV` — the watt-pf / watt-var kvar set-point pushed
    /// to the DER.
    pub q_desired_wp: f64,
    pub q_desired_wv: f64,
    /// `QDesireWPpu` / `QDesireWVpu` — Q desired from the watt-pf / watt-var curve (pu).
    pub q_desire_wppu: f64,
    pub q_desire_wvpu: f64,
    /// `FWPOperation` / `FWVOperation` — watt-pf / watt-var operating flags.
    pub f_wp_operation: f64,
    pub f_wv_operation: f64,

    // --- volt-watt active-power state (VOLTWATT / VV_VW; sub-step 2c) ---
    /// `PLimitVW` — the volt-watt kW set-point pushed to the DER.
    pub p_limit_vw: f64,
    /// `PLimitVWpu` — the kW limit (pu of `PBase`) read off the volt-watt curve.
    pub p_limit_vw_pu: f64,
    /// `PLimitLimitedpu` — the kW limit after the kVA / pctPmpp clamp (`Check_Plimits`).
    pub p_limit_limitedpu: f64,
    /// `PLimitEndpu` — the kW (pu) used in the convergence algorithm.
    pub p_limit_endpu: f64,
    /// `POldVWpu` — the prior volt-watt kW (pu) (convergence history).
    pub p_old_vw_pu: f64,
    /// `PBase` — the volt-watt power base (set by `Calc_PBase` from `VoltWattYAxis`).
    pub p_base: f64,
    /// `kW_out_desired` (= `FpresentkW` each Sample) and its pu form `kW_out_desiredpu`.
    pub kw_out_desired: f64,
    pub kw_out_desiredpu: f64,
    /// `FFlagVWOperates` — latched once the volt-watt limit drops below 1 pu.
    pub f_flag_vw_operates: bool,
    /// `FVWOperation` — volt-watt operating flag (1 limiting / 0 not).
    pub f_vw_operation: f64,
    /// `FdeltaPFactor` — the adaptive convergence damping factor for volt-watt.
    pub f_delta_p_factor: f64,
    /// `FDCkW` (PVSystem `PanelkW`), `FDCkWRated` (PVSystem `Pmpp`),
    /// `FpctDCkWRated` (PVSystem `puPmpp`), `FEffFactor` — the volt-watt power-base
    /// inputs, refreshed each Sample by `UpdateDERParameters`. PVSystem-only: the
    /// Storage VOLTWATT/VV_VW dispatch is deferred (an explicit error, not a silent
    /// skip), so the Storage `DCkW` path of `Calc_PBase` is not carried here.
    pub f_dckw: f64,
    pub f_dckw_rated: f64,
    pub f_pct_dckw_rated: f64,
    pub f_eff_factor: f64,

    // --- hysteresis (curve 1/2) ---
    pub flag_change_curve: bool,
    pub f_active_vv_curve: i32,

    // --- rolling-average windows + the per-step voltage history ---
    /// `FVpuSolution[1..2]` — last two per-unit solution voltages (index 0 unused).
    pub f_vpu_solution: [f64; 3],
    pub prior_roll_avg_window: f64,
    pub prior_drc_roll_avg_window: f64,
    pub f_roll_avg_window: RollAvgWindow,
    pub f_drc_roll_avg_window: RollAvgWindow,

    /// `FPendingChange` — the queued action code for this DER.
    pub f_pending_change: i32,

    // --- DER parameters refreshed each Sample by `UpdateDERParameters` ---
    pub f_vbase: f64,
    pub f_var_follow_inverter: bool,
    pub f_inverter_on: bool,
    pub f_present_kw: f64,
    pub f_kva_rating: f64,
    pub f_present_kvar: f64,
    pub f_kvar_limit: f64,
    pub f_kvar_limit_neg: f64,
    pub f_current_kvar_limit: f64,
    pub f_current_kvar_limit_neg: f64,
    pub f_p_priority: bool,
    /// `DERElem.GetPFPriority()` — the inverter PF-priority flag (read live by
    /// Pascal's `CalcQWPcurve_desiredpu`; cached here at `UpdateDERParameters`
    /// since `SetPFPriority` never fires inside the control loop, so the cached
    /// value equals the live one).
    pub f_pf_priority: bool,
}

impl InvVars {
    /// Pascal `MakeDERList`'s per-DER initialization block (the subset the step-2b
    /// VOLTVAR path consumes). `QOld`/`QOldVV` start at -1.0; the adaptive factor
    /// at `DELTAQDEFAULT`; `DeltaV_old` at -1.0; the active curve at 1.
    fn new() -> Self {
        Self {
            q_old: -1.0,
            q_old_vv: -1.0,
            q_old_drc: -1.0,
            q_old_vvdrc: -1.0,
            f_delta_q_factor: DELTAQDEFAULT,
            f_delta_p_factor: DELTAPDEFAULT,
            delta_v_old: -1.0,
            f_active_vv_curve: 1,
            f_inverter_on: true,
            f_pending_change: CHANGE_NONE,
            ..Default::default()
        }
    }
}

/// `TInvControlObj`. The parse-time surface (step 2a) plus the WP7.5 step-2b
/// VOLTVAR dispatch state: the resolved DER fleet (`fleet`), the per-DER
/// [`InvVars`] (`ctrl_vars`), and the rolling-average solution-voltage bookkeeping.
/// VOLTWATT / DRC / WATTPF / WATTVAR / AVR + the combi modes land in 2c–2e.
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

    // --- WP7.5 step-2b runtime state (the DER fleet + dispatch) ---
    /// `FDERPointerList` — the resolved PVSystem/Storage fleet, built lazily on the
    /// first `Sample` (empty until then), cached across samples like Pascal. An
    /// empty fleet re-triggers the build (Pascal `FDERPointerList.Count = 0`); a
    /// DERList edit clears it (`invalidate_fleet`).
    pub(crate) fleet: Vec<ElemRef>,
    /// `CtrlVars` — one [`InvVars`] per fleet member (1:1 with `fleet`).
    ctrl_vars: Vec<InvVars>,
    /// `FVpuSolutionIdx` — toggles 1↔2 each `UpdateInvControl` pass.
    f_vpu_solution_idx: i32,
    /// `FVreg` — the pu voltage used in the volt-var / volt-watt curves (object-level
    /// in Pascal; the per-DER value of the current Sample iteration).
    f_vreg: f64,
    /// `FUsingMonBuses` — true when `MonBus=` named explicit monitored buses
    /// (the per-bus `GetMonVoltage` path is NOT_PORTED until step 2e).
    f_using_mon_buses: bool,

    // --- parse-time resolved monitored-DER bus (the `Setbus(1, MonitoredElement.
    // Firstbus)` carry-forward; the executive resolves it at edit-completion since
    // `recalc_element_data` has no store access) ---
    /// The first DER's `Firstbus` (the bus the control's terminal attaches to).
    mon_bus: String,
    /// The (last) DER's phase count (Pascal `FNphases := ControlledElement[i].NPhases`
    /// over the recalc loop); the control's `NConds` follows.
    mon_nphases: usize,
    /// Whether [`set_resolved_monitored`](Self::set_resolved_monitored) ran (a fleet
    /// member was found at parse time).
    mon_resolved: bool,
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

            fleet: Vec::new(), // empty → the first Sample builds it
            ctrl_vars: Vec::new(),
            f_vpu_solution_idx: 0,
            f_vreg: 0.0,
            f_using_mon_buses: false,
            mon_bus: String::new(),
            mon_nphases: 3,
            mon_resolved: false,
        }
    }

    /// The executive resolves the first DER's bus at edit-completion (Pascal
    /// `RecalcElementData` runs `MakeDERList` + `Setbus(1, MonitoredElement.
    /// Firstbus)`, but the Rust `recalc_element_data` has no store access). Called
    /// from `exec::command` with the resolved first-DER bus + the (last) DER's phase
    /// count; `end_edit`/[`recalc`](Self::recalc) then attaches the terminal.
    pub(crate) fn set_resolved_monitored(&mut self, bus: String, nphases: usize) {
        self.mon_bus = bus;
        self.mon_nphases = nphases;
        self.mon_resolved = true;
    }

    /// The current DER name list (consumed by the executive's parse-time fleet-bus
    /// resolution).
    pub(crate) fn der_name_list(&self) -> &[String] {
        &self.der_name_list
    }
}
