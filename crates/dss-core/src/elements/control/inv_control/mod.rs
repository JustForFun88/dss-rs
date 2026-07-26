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
//! and the entire `Sample`/`DoPendingAction`/`Reset` dispatch. `MakePosSequence`
//! is ported (WPG.21) as a NIL-deref-safe partial (the defined 3-phase resync +
//! resolved-DER bus adopt, empty-list deref safe-skipped — see [`accessors`]).

mod accessors;
mod compute;
#[cfg(test)]
mod tests;

pub(crate) use compute::{DerSnap, FleetFind as InvFleetFind, InvDispatchEnv, MonitorVar};

use crate::elements::control::control_elem::ControlElemData;
use crate::elements::control::mon_phase::MonPhase;
use crate::elements::control::roll_avg_window::RollAvgWindow;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::traits::ElemId;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// `TInvControlControlMode` (`InvControl.pas:119-128`, `{$Z4}` int32 enum) —
/// the InvControl `Mode=` selection. Discriminants are user-visible and frozen
/// (they round-trip through the `InvControl: Control Mode` `DssEnum`, values
/// `[1..7]`; `NoneMode` = the unset default, which dumps `''`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum InvControlMode {
    /// `NONE_MODE` — `TInvControlObj.Create`'s default (the docs say "VoltVar"
    /// but `Create` sets NONE); not registered in the `DssEnum`, dumps `''`.
    #[default]
    NoneMode = 0,
    VoltVar = 1,
    VoltWatt = 2,
    /// `DRC` — dynamic reactive current.
    Drc = 3,
    WattPf = 4,
    WattVar = 5,
    /// `AVR` — active voltage regulation (the 3-stage DQDV regulator).
    Avr = 6,
    /// `GFM` — grid-forming (the amps-limit / overload protective arm).
    Gfm = 7,
}

impl InvControlMode {
    /// The `InvControl: Control Mode` `DssEnum` ordinal.
    pub(crate) fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub(crate) fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::NoneMode),
            1 => Some(Self::VoltVar),
            2 => Some(Self::VoltWatt),
            3 => Some(Self::Drc),
            4 => Some(Self::WattPf),
            5 => Some(Self::WattVar),
            6 => Some(Self::Avr),
            7 => Some(Self::Gfm),
            _ => None,
        }
    }
}

/// `TInvControlCombiMode` (`InvControl.pas:131-135`) — the `CombiMode=`
/// selection (`InvControl: Combi Mode` `DssEnum`, values `[1, 2]`;
/// `NoneCombMode` = unset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum InvCombiMode {
    #[default]
    NoneCombMode = 0,
    VvVw = 1,
    VvDrc = 2,
}

impl InvCombiMode {
    /// The `InvControl: Combi Mode` `DssEnum` ordinal.
    pub(crate) fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub(crate) fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::NoneCombMode),
            1 => Some(Self::VvVw),
            2 => Some(Self::VvDrc),
            _ => None,
        }
    }
}

/// `ERateofChangeMode` (`InvControl.pas:143-147`) — `RateofChangeMode=`
/// (`InvControl: Rate-of-change Mode` `DssEnum`, values `[0, 1, 2]`). `Lpf` /
/// `RiseFall` drive the rate-of-change limiting (`CalcLPF`/`CalcRF` in
/// `DoPendingAction`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum RateOfChangeMode {
    #[default]
    Inactive = 0,
    Lpf = 1,
    RiseFall = 2,
}

impl RateOfChangeMode {
    /// The `InvControl: Rate-of-change Mode` `DssEnum` ordinal.
    pub(crate) fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub(crate) fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Inactive),
            1 => Some(Self::Lpf),
            2 => Some(Self::RiseFall),
            _ => None,
        }
    }
}

/// InvControl's `FPendingChange` action codes (`InvControl.pas:407-411`).
/// These are *not* a `DssEnum` — they are the control-queue action codes this
/// class pushes and pops, so `i32` survives only at the `ControlQueue`
/// push/`DoPendingAction` boundary (the P1b `RegControlAction` precedent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum InvPendingChange {
    /// Pascal `NONE = 0`.
    #[default]
    None = 0,
    ChangeVarLevel = 1,
    ChangeWattLevel = 2,
    ChangeWattVarLevel = 3,
    ChangeDrcVVarLevel = 4,
}

impl InvPendingChange {
    /// The `ControlQueue` action code (`Push(..., code, ...)`).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From a popped `ControlQueue` action code; unknown codes yield `None`.
    /// (InvControl's own `DoPendingAction` reads `FPendingChange` off the DER
    /// record rather than the popped code — Pascal does the same — so this is
    /// the symmetric inverse the `RegControlAction` precedent also exposes.)
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::ChangeVarLevel),
            2 => Some(Self::ChangeWattLevel),
            3 => Some(Self::ChangeWattVarLevel),
            4 => Some(Self::ChangeDrcVVarLevel),
            _ => None,
        }
    }
}

/// `ReacPower_VARAVAL` / `ReacPower_VARMAX` (`InvControl.pas:404-405`) — the
/// `RefReactivePower=` reference (`InvControl: Reactive Power Reference`
/// `DssEnum`, values `[0, 1]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum ReacPowerRef {
    #[default]
    VarAval = 0,
    VarMax = 1,
}

impl ReacPowerRef {
    /// The `InvControl: Reactive Power Reference` `DssEnum` ordinal.
    pub(crate) fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub(crate) fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::VarAval),
            1 => Some(Self::VarMax),
            _ => None,
        }
    }
}

/// `TInvControlModel` (`InvControl.pas:137-140`) — the `ControlModel=` var-calc
/// kernel (`InvControl: Control Model` `DssEnum`, values `[0, 1]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub(crate) enum InvControlModel {
    /// `TInvControlModel.Linear` — `TInvControlObj.Create` sets it
    /// (`InvControl.pas:873`).
    #[default]
    Linear = 0,
    Exponential = 1,
}

impl InvControlModel {
    /// The `InvControl: Control Model` `DssEnum` ordinal.
    pub(crate) fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub(crate) fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Linear),
            1 => Some(Self::Exponential),
            _ => None,
        }
    }
}

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
        PropDef::double("LPFTau").flags(PropFlags::UNITS_S),
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
        // Pascal `DeprecatedAndRemoved` (`InvControl.pas:525`) — present in the
        // table (occupies a `$dssPropertyIndex` ordinal) but its `?` getter renders
        // '' and a write does nothing. Like Storage's `%Idlingkvar`, `SUPPRESS_JSON`
        // excludes it from the JSON export, the schema walk, and `AltPropertyOrder`
        // (Pascal `GetObjPropertyJSONValue` returns False for `DeprecatedAndRemoved`
        // + `nextByZOrder` skips it, `DSSObjectHelper.pas:1517`/`DSSClass.pas:2003`)
        // while the `?`/props surface still exposes the stored ''.
        PropDef::string("VV_RefReactivePower").flags(PropFlags::SUPPRESS_JSON),
        // Pascal Redundant+Deprecated, PropertyOffset = @DERNameList (shares the
        // DERList backing): a write prepends "PVSystem." then re-runs the DERList
        // side effect.
        PropDef::string_list("PVSystemList").flags(PropFlags::REDUNDANT),
        PropDef::double("VSetPoint"),
        PropDef::mapped_int_enum("ControlModel", enums.invcontrol_model),
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
    ClassProps::new("InvControl", defs, true)
}

/// Pascal `TPICtrl` (`Shared/mathutil.pas` l.21) — the two-tap discrete PI
/// controller InvControl runs for the Exponential `ControlModel`. `kNum`/`kDen`
/// are recomputed from the object-level `FdeltaQ_factor` before every `SolvePI`
/// call (InvControl.pas l.2706-2707); `Kp` is fixed at 1 in InvControl
/// (`RecalcElementData`, l.2400), overriding the 0.02 mathutil default. `den`/`num`
/// are the private filter history — persisted across `Sample` calls per DER.
#[derive(Debug, Clone)]
pub(crate) struct PICtrl {
    /// `den`/`num: Array[0..1] of Double` — the two-tap filter state (private).
    den: [f64; 2],
    num: [f64; 2],
    pub k_num: f64,
    pub k_den: f64,
    pub kp: f64,
}

impl Default for PICtrl {
    /// Pascal `TPICtrl.Create` (mathutil.pas l.67): a rising 5-step function —
    /// `kNum=0.8647`, `kDen=0.1353`, `Kp=0.02`, `den[1]=0`, `num[1]=0`. InvControl
    /// overrides `Kp:=1` right after `Create` (see [`InvVars::new`]).
    fn default() -> Self {
        Self {
            den: [0.0, 0.0],
            num: [0.0, 0.0],
            k_num: 0.8647,
            k_den: 0.1353,
            kp: 0.02,
        }
    }
}

impl PICtrl {
    /// Pascal `TPICtrl.SolvePI` (mathutil.pas l.81) — one filter step: shift the
    /// taps, load `SetPoint·Kp`, and return the new denominator tap.
    pub(crate) fn solve_pi(&mut self, setpoint: f64) -> f64 {
        self.num[0] = self.num[1];
        self.num[1] = setpoint * self.kp;
        self.den[0] = self.den[1];
        self.den[1] = (self.num[0] * self.k_num) + (self.den[0] * self.k_den);
        self.den[1]
    }
}

/// Pascal `TInvVars` — the per-controlled-DER runtime state (one record per fleet
/// member). Carries the fields the WP7.5 **step 2b–2e-ii** dispatch (VOLTVAR /
/// VOLTWATT / VV_VW / DRC / VV_DRC / WATTPF / WATTVAR / AVR) + the shared machinery
/// (`UpdateInvControl`, `Calc_QHeadRoom`, `Change_deltaQ_factor`,
/// `UpdateDERParameters`) read/write. Field names mirror the Pascal record for a
/// 1:1 read.
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
    /// `PICtrl` — the per-DER `TPICtrl` PI controller (Exponential `ControlModel`).
    /// Shared by the VV / AVR / DRC / VV_DRC var-calc paths; its filter history
    /// persists across `Sample` calls. Unused on the Linear path.
    pub pi_ctrl: PICtrl,

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

    // --- AVR (active voltage regulation) reactive-power state (sub-step 2e-ii) ---
    /// `QDesiredAVR` — the AVR kvar set-point pushed to the DER.
    pub q_desired_avr: f64,
    /// `QOldAVR` — the prior AVR kvar (convergence history). Pascal seeds it to
    /// `-PVSys.kvarLimitNeg/2` (or 0 for an all-Storage fleet) in `MakeDERList`,
    /// but `CalcQAVR_desiredpu` resets it to 0 at control iteration 3 *before* any
    /// read, so the seed is unobservable — left at the `Default` 0.0.
    pub q_old_avr: f64,
    /// `QoutputAVRpu` — the achieved Q (pu) used in the AVR trigger comparison.
    pub qoutput_avrpu: f64,
    /// `QDesireAVRpu` — Q desired from the AVR DQDV law (pu).
    pub q_desire_avrpu: f64,
    /// `FAVROperation` — AVR operating flag (-1 absorb / 1 inject / 0 none).
    pub f_avr_operation: f64,
    /// `DQDV` — the dQ/dV sensitivity estimated on control iteration 2.
    pub dqdv: f64,
    /// `Fv_setpointLimited` — the AVR voltage setpoint after the kvar-limit back-off
    /// (the trigger compares `FPresentVpu` against it).
    pub f_v_setpoint_limited: f64,
    /// `FAvgpAVRVpuPrior` — the control-iteration-1 prior voltage, used as the AVR
    /// baseline (`v`) at control iteration 3.
    pub f_avgp_avr_vpu_prior: f64,

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

    // --- LPF / Rise-Fall rate-of-change limiting (sub-step 2e-iii) ---
    /// `QDesireOptionpu` / `PLimitOptionpu` — the desired Q / P limit (pu) *after*
    /// the LPF / Rise-Fall filter, fed into `Check_Qlimits` / `Check_Plimits`.
    pub q_desire_optionpu: f64,
    pub p_limit_optionpu: f64,
    /// `FPriorQDesireOptionpu` / `FPriorPLimitOptionpu` — the prior time step's
    /// `QDesireOptionpu` / `PLimitOptionpu` (the LPF/RF reference, refreshed once
    /// per step in `UpdateInvControl`).
    pub f_prior_q_desire_optionpu: f64,
    pub f_prior_p_limit_optionpu: f64,
    /// `FDCkW` (PVSystem `PanelkW`; 0 for Storage), `FDCkWRated` (PVSystem `Pmpp` /
    /// Storage `kWrating`), `FpctDCkWRated` (PVSystem `puPmpp` / Storage
    /// `pctkWrated`), `FEffFactor` — the volt-watt power-base inputs, refreshed each
    /// Sample by `UpdateDERParameters`. The Storage `%Available` base reads the live
    /// `TStorageObj.DCkW` at `Calc_PBase` time instead of `f_dckw` (WPG.10).
    pub f_dckw: f64,
    pub f_dckw_rated: f64,
    pub f_pct_dckw_rated: f64,
    pub f_eff_factor: f64,

    // --- hysteresis (curve 1/2) ---
    pub flag_change_curve: bool,
    pub f_active_vv_curve: i32,

    // --- rolling-average windows + the per-step voltage history ---
    /// `FVpuSolution[0..1]` — the last two per-unit solution voltages (dss_capi
    /// 0.15.x `InvControlDeltaV` fix: a per-control 2-slot buffer; see the
    /// `f_vpu_solution_idx` doc + ledger L1).
    pub f_vpu_solution: [f64; 2],
    pub prior_roll_avg_window: f64,
    pub prior_drc_roll_avg_window: f64,
    pub f_roll_avg_window: RollAvgWindow,
    pub f_drc_roll_avg_window: RollAvgWindow,

    /// `FPendingChange` — the queued action code for this DER.
    pub f_pending_change: InvPendingChange,

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
            f_pending_change: InvPendingChange::None,
            // Pascal `RecalcElementData` (InvControl.pas l.2399-2400) creates each
            // DER's `TPICtrl` then overrides `Kp := 1`.
            pi_ctrl: PICtrl {
                kp: 1.0,
                ..PICtrl::default()
            },
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

    /// `ControlMode` (`TInvControlControlMode`; `NoneMode` renders '').
    control_mode: InvControlMode,
    /// `CombiMode` (`TInvControlCombiMode`).
    combi_mode: InvCombiMode,

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
    /// `RateofChangeMode` (`ERateofChangeMode`).
    rate_of_change_mode: RateOfChangeMode,
    /// `LPFTau` (seconds) / `FRiseFallLimit`.
    lpf_tau: f64,
    rise_fall_limit: f64,

    /// `FReacPower_ref` (`VarAval`=0, `VarMax`=1).
    reac_power_ref: ReacPowerRef,

    /// `FMonBusesPhase` (MonVoltageCalc; MonPhaseEnum: avg/max/min/phase no.).
    mon_buses_phase: MonPhase,
    /// `MonBusesNameList` — the raw monitored-bus list (the `MonBus=` strings,
    /// possibly with `.node` suffixes).
    mon_buses_name_list: Vec<String>,
    /// `FMonBuses` — the parsed bus names (the `.node` suffix stripped), and
    /// `FMonBusesNodes` — the per-bus node numbers, both derived from
    /// `mon_buses_name_list` by `ParseAsBusName` in the `MonBus` side-effect.
    /// Consumed by `GetMonVoltage`'s explicit-`MonBus` path.
    pub(crate) mon_buses: Vec<String>,
    mon_buses_nodes: Vec<Vec<i32>>,
    /// `FMonBusesVbase` — one base-kV per monitored bus (array_size =
    /// MonBusesNameList.Count).
    mon_buses_vbase: Vec<f64>,

    /// `Fv_setpoint` — AVR voltage setpoint.
    v_setpoint: f64,
    /// `CtrlModel` (`TInvControlModel`: Linear / Exponential).
    ctrl_model: InvControlModel,

    // --- WP7.5 step-2b runtime state (the DER fleet + dispatch) ---
    /// `FDERPointerList` — the resolved PVSystem/Storage fleet, built lazily on the
    /// first `Sample` (empty until then), cached across samples like Pascal. An
    /// empty fleet re-triggers the build (Pascal `FDERPointerList.Count = 0`); a
    /// DERList edit clears it (`invalidate_fleet`).
    pub(crate) fleet: Vec<ElemId>,
    /// `CtrlVars` — one [`InvVars`] per fleet member (1:1 with `fleet`).
    ctrl_vars: Vec<InvVars>,
    /// `FVpuSolutionIdx` — the write/read cursor into the per-control 2-slot
    /// `f_vpu_solution`. dss_capi 0.15.x `InvControlDeltaV` fix: initialized to
    /// `-1` and toggled `0↔1` unconditionally once per `UpdateInvControl` pass
    /// (0.14.5 gated the bump on the element-list index `i=1`, so only the first
    /// InvControl in the circuit advanced — see ledger L1).
    f_vpu_solution_idx: i32,
    /// `FVreg` — the pu voltage used in the volt-var / volt-watt curves (object-level
    /// in Pascal; the per-DER value of the current Sample iteration).
    f_vreg: f64,
    /// `FUsingMonBuses` — true when `MonBus=` named explicit monitored buses
    /// (drives `GetMonVoltage`'s per-bus path; set in `ensure_fleet`).
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
            control_mode: InvControlMode::NoneMode, // docs say "VoltVar"; Create sets NONE
            combi_mode: InvCombiMode::NoneCombMode,

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
            rate_of_change_mode: RateOfChangeMode::Inactive,
            lpf_tau: 0.001,         // docs list 0
            rise_fall_limit: 0.001, // docs list -1 (disabled)

            reac_power_ref: ReacPowerRef::VarAval,

            mon_buses_phase: MonPhase::Avg,
            mon_buses_name_list: Vec::new(),
            mon_buses: Vec::new(),
            mon_buses_nodes: Vec::new(),
            mon_buses_vbase: Vec::new(),

            v_setpoint: 1.0,
            ctrl_model: InvControlModel::Linear,

            fleet: Vec::new(), // empty → the first Sample builds it
            ctrl_vars: Vec::new(),
            f_vpu_solution_idx: -1, // Pascal l.816 (0.15.x InvControlDeltaV fix)
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

    // --- Read-only accessors for the CIM `TIEEE1547Controller` export (WPG.18
    // Stage F, `ExportCIMXML.pas` `PullFromInvControl`). No behavior change. ---

    /// `MonBusesNameList` — the raw monitored-bus strings (`FindSignalTerminals`).
    pub(crate) fn mon_buses_name_list(&self) -> &[String] {
        &self.mon_buses_name_list
    }
    /// `Fvvc_curve` — the volt-var curve snapshot (`None` if unset).
    pub(crate) fn vvc_curve(&self) -> Option<&XyCurveObj> {
        self.vvc_curve.as_ref()
    }
    /// `Fvoltwatt_curve` — the volt-watt curve snapshot.
    pub(crate) fn voltwatt_curve(&self) -> Option<&XyCurveObj> {
        self.voltwatt_curve.as_ref()
    }
    /// `FvoltwattCH_curve` — the volt-watt charging curve snapshot.
    pub(crate) fn voltwattch_curve(&self) -> Option<&XyCurveObj> {
        self.voltwattch_curve.as_ref()
    }
    /// `Fwattvar_curve` — the watt-var curve snapshot.
    pub(crate) fn wattvar_curve(&self) -> Option<&XyCurveObj> {
        self.wattvar_curve.as_ref()
    }
    /// `LPFTau` (seconds).
    pub(crate) fn lpf_tau(&self) -> f64 {
        self.lpf_tau
    }
    /// `ControlMode` (`VoltVar`=1…`Gfm`=7).
    pub(crate) fn control_mode(&self) -> InvControlMode {
        self.control_mode
    }
    /// `CombiMode` (`VvVw`=1, `VvDrc`=2).
    pub(crate) fn combi_mode(&self) -> InvCombiMode {
        self.combi_mode
    }
    /// `FDRCRollAvgWindowLength` (DynReacAvgWindowLen, seconds).
    pub(crate) fn drc_roll_avg_window_length(&self) -> i32 {
        self.drc_roll_avg_window_length
    }
    /// `FArGraLowV` — DRC low-voltage slope.
    pub(crate) fn ar_gra_low_v(&self) -> f64 {
        self.ar_gra_low_v
    }
    /// `FArGraHiV` — DRC high-voltage slope.
    pub(crate) fn ar_gra_hi_v(&self) -> f64 {
        self.ar_gra_hi_v
    }
}
