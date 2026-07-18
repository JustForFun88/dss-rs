//! Port of `PCElements/Storage.pas` — `TStorageObj`, the battery-storage PC
//! element, built on the Generator injection template (WP6.2) with the shared
//! inverter base [`InvBasedPceData`] (WP7.3 step 1) embedded the way [`PVSystem`]
//! does. Storage adds a **charge/idle/discharge state machine** and an
//! integrated state of charge (`kWhStored`/`%stored`) that the time-step cleanup
//! hook advances (`UpdateStorage`).
//!
//! The present terminal kW comes from the state + dispatch (`ComputePresentkW`):
//! discharging delivers `kWrating·%Discharge`, charging absorbs
//! `−kWrating·%Charge`, idling draws only the idling losses (`−kWOutIdling`); the
//! inverter then applies cut-in/cut-out, the efficiency curve, the watt/var
//! priority and the `kVA`/kvar clamps (`ComputeInverterPower`). The result is a
//! per-phase P/Q that injects exactly like a Generator's constant-PQ model.
//!
//! Scope (WP7.4 step 1): the **power-flow** Storage — `SetNominalDEROutput`
//! (`SetNominalStorage`), the state machine + dispatch (`CheckStateTriggerLevel`),
//! daily/yearly/duty shapes (snapshot-clone), `CalcYPrim` (state-dependent
//! `YeqDischarge`), `DoConstantPQStorageObj` / `DoConstantZStorageObj` + the
//! inverter clamp, the SOC integration (`ComputeDCkW`/`UpdateStorage` + the loss
//! split), the energy-meter registers and `TakeSample`. The grid-forming mode
//! (`DoGFM_Mode`/`CalcGFMYprim`), the harmonic injection
//! (`DoHarmonicMode`/`InitHarmonics`), the dynamics state machinery
//! (`DoDynamicMode`/`InitStateVars`/`IntegrateStates` + the state-variable
//! interface `NumVariables`/`Get_Variable`/`VariableName`), the user-written DLL
//! models (`UserModel`/`DynaModel`, never ported) are deferred to WP7.6/7.7
//! (harmonics/dynamics), matching the PVSystem deferrals. `MakePosSequence` is
//! in `accessors` (WPG.21).
//!
//! Split into submodules (this file holds the metadata, struct and `Create`):
//! - [`nominal`]: shape multipliers, the state machine
//!   (`ComputePresentkW`/`CheckStateTriggerLevel`), `ComputeInverterPower` /
//!   `kWOut_Calc`, `SetNominalDEROutput` and `RecalcElementData`.
//! - [`solve`]: `CalcYPrimMatrix`, the `DoConstantPQ/Z` model currents and the
//!   injection-current assembly.
//! - [`registers`]: `ComputeDCkW`, the loss split, `UpdateStorage` (SOC
//!   integration), the energy-meter registers and `TakeSample`.
//! - [`accessors`]: the `CktElement` / `DssObject` / [`InvBasedPce`] trait impls.
//!
//! [`PVSystem`]: crate::elements::pc::pvsystem::PVSystem
//! [`InvBasedPce`]: crate::elements::pc::inv_based_pce::InvBasedPce

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::pc::inv_based_pce::{Connection, InvBasedPceData};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

mod accessors;
mod dynamics;
mod nominal;
mod registers;
mod solve;

/// Pascal `varMode` values.
pub(crate) const VARMODE_PF: i32 = 0;
pub(crate) const VARMODE_KVAR: i32 = 1;

/// Pascal storage states (`STORE_CHARGING`/`STORE_IDLING`/`STORE_DISCHARGING`).
pub(crate) const STORE_CHARGING: i32 = -1;
pub(crate) const STORE_IDLING: i32 = 0;
pub(crate) const STORE_DISCHARGING: i32 = 1;

/// Pascal Storage dispatch modes (`STORE_DEFAULT`..`STORE_FOLLOW`; `Set
/// DispMode=`, `StorageDispatchModeEnum`). Discriminants are user-visible and
/// frozen (round-trip through the `DssEnum` registry); `i32` survives only at
/// the property parse/report boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum StorageDispatchMode {
    Default = 0,
    LoadMode = 1,
    PriceMode = 2,
    ExternalMode = 3,
    Follow = 4,
}

impl StorageDispatchMode {
    /// The `StorageDispatchModeEnum` ordinal (property `?`/dump boundary value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// `TStorageDispatchMode(ordinal)`; out-of-range yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Default),
            1 => Some(Self::LoadMode),
            2 => Some(Self::PriceMode),
            3 => Some(Self::ExternalMode),
            4 => Some(Self::Follow),
            _ => None,
        }
    }
}

// Register indices (Pascal `Reg_kWh = 1` .. `Reg_Price = 6`, 0-based here).
const REG_KWH: usize = 0;
const REG_KVARH: usize = 1;
const REG_MAXKW: usize = 2;
const REG_MAXKVA: usize = 3;
const REG_HOURS: usize = 4;
const REG_PRICE: usize = 5;
const NUM_STORAGE_REGISTERS: usize = 6;

/// 1-based property ordinals (Pascal `TStorageProp` + class tails).
pub mod prop {
    pub const PHASES: usize = 1;
    pub const BUS1: usize = 2;
    pub const KV: usize = 3;
    pub const CONN: usize = 4;
    pub const KW: usize = 5;
    pub const KVAR: usize = 6;
    pub const PF: usize = 7;
    pub const KVA: usize = 8;
    pub const PCT_CUTIN: usize = 9;
    pub const PCT_CUTOUT: usize = 10;
    pub const EFF_CURVE: usize = 11;
    pub const VAR_FOLLOW_INVERTER: usize = 12;
    pub const KVAR_MAX: usize = 13;
    pub const KVAR_MAX_ABS: usize = 14;
    pub const WATT_PRIORITY: usize = 15;
    pub const PF_PRIORITY: usize = 16;
    pub const PCT_PMIN_NO_VARS: usize = 17;
    pub const PCT_PMIN_KVAR_MAX: usize = 18;
    pub const KW_RATED: usize = 19;
    pub const PCT_KW_RATED: usize = 20;
    pub const KWH_RATED: usize = 21;
    pub const KWH_STORED: usize = 22;
    pub const PCT_STORED: usize = 23;
    pub const PCT_RESERVE: usize = 24;
    pub const STATE: usize = 25;
    pub const PCT_DISCHARGE: usize = 26;
    pub const PCT_CHARGE: usize = 27;
    pub const PCT_EFF_CHARGE: usize = 28;
    pub const PCT_EFF_DISCHARGE: usize = 29;
    pub const PCT_IDLING_KW: usize = 30;
    pub const PCT_IDLING_KVAR: usize = 31; // deprecated + removed (dumps '')
    pub const PCT_R: usize = 32;
    pub const PCT_X: usize = 33;
    pub const MODEL: usize = 34;
    pub const VMINPU: usize = 35;
    pub const VMAXPU: usize = 36;
    pub const BALANCED: usize = 37;
    pub const LIMIT_CURRENT: usize = 38;
    pub const YEARLY: usize = 39;
    pub const DAILY: usize = 40;
    pub const DUTY: usize = 41;
    pub const DISP_MODE: usize = 42;
    pub const DISCHARGE_TRIGGER: usize = 43;
    pub const CHARGE_TRIGGER: usize = 44;
    pub const TIME_CHARGE_TRIG: usize = 45;
    pub const CLS: usize = 46;
    pub const DYNA_DLL: usize = 47;
    pub const DYNA_DATA: usize = 48;
    pub const USERMODEL: usize = 49;
    pub const USERDATA: usize = 50;
    pub const DEBUGTRACE: usize = 51;
    pub const KVDC: usize = 52;
    pub const KP: usize = 53;
    pub const PITOL: usize = 54;
    pub const SAFE_VOLTAGE: usize = 55;
    pub const SAFE_MODE: usize = 56;
    pub const DYNAMIC_EQ: usize = 57;
    pub const DYN_OUT: usize = 58;
    pub const CONTROL_MODE: usize = 59;
    pub const AMP_LIMIT: usize = 60;
    pub const AMP_LIMIT_GAIN: usize = 61;
    // PCElement / CktElement tails:
    pub const SPECTRUM: usize = 62;
    pub const BASE_FREQ: usize = 63;
    pub const ENABLED: usize = 64;
    pub const NUM_PROPS: usize = 65; // incl. Like
}

/// `TStorage.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::double("kV").flags(PropFlags::NON_NEGATIVE | PropFlags::REQUIRED),
        PropDef::mapped_string_enum("Conn", enums.connection),
        // `kW`: read returns `kW_out` (the field), write goes through Set_kW
        // (sets the state + %Discharge/%Charge).
        PropDef::double("kW").flags(PropFlags::REPLACE_ZERO),
        // `kvar`: read returns `kvar_out` (Pascal `Getkvar`), write stores
        // `kvarRequested`.
        PropDef::double("kvar").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("PF").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("kVA").flags(PropFlags::REPLACE_ZERO),
        PropDef::double("%CutIn"),
        PropDef::double("%CutOut"),
        PropDef::object_ref_class("XYcurve", "EffCurve"),
        PropDef::boolean("VarFollowInverter"),
        PropDef::double("kvarMax").flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::double("kvarMaxAbs").flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::boolean("WattPriority"),
        PropDef::boolean("PFPriority"),
        PropDef::double("%PMinNoVars"),
        PropDef::double("%PMinkvarMax"),
        PropDef::double("kWRated").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("%kWRated").scale(0.01),
        PropDef::double("kWhRated"),
        PropDef::double("kWhStored").flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::NON_NEGATIVE),
        // `%Stored`: read = kWhStored/kWhRating·100, write = kWhStored = %·kWhRating.
        PropDef::double("%Stored"),
        PropDef::double("%Reserve"),
        PropDef::mapped_string_enum("State", enums.storage_state).flags(PropFlags::NO_DEFAULT),
        PropDef::double("%Discharge"),
        PropDef::double("%Charge"),
        PropDef::double("%EffCharge"),
        PropDef::double("%EffDischarge"),
        PropDef::double("%IdlingkW"),
        // `%Idlingkvar`: Pascal DeprecatedAndRemoved — present in the table (it
        // occupies an ordinal) but its `?` getter renders '' and a write does
        // nothing. Modeled as a read-only '' string, like the StorageController
        // fleet-aggregate readbacks.
        PropDef::string("%Idlingkvar"),
        PropDef::double("%R"),
        PropDef::double("%X"),
        PropDef::integer("Model"),
        PropDef::double("VMinpu"),
        PropDef::double("VMaxpu"),
        PropDef::boolean("Balanced"),
        PropDef::boolean("LimitCurrent"),
        PropDef::object_ref_class("LoadShape", "Yearly"),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty"),
        PropDef::mapped_string_enum("DispMode", enums.storage_dispatch_mode),
        PropDef::double("DischargeTrigger"),
        PropDef::double("ChargeTrigger"),
        PropDef::double("TimeChargeTrig"),
        PropDef::integer("Class"),
        // User-written model DLLs are never *loaded* in safe Rust (the loader is
        // permanently out of scope — forbid(unsafe_code)). CF-C Port 2 ports the
        // `DynaDLL`/`DynaData` property SURFACE (parse, store, dump); the
        // `DynaDLL` side effect emits a non-fatal "Not Loaded" diagnostic and
        // falls back to the built-in model — matching the official Direct DLL
        // (Storage.pas l.866, StoreUserModel Set_Name l.329, DoSimpleMsg 1570),
        // not the pinned oracle (which raises #1570).
        PropDef::string("DynaDLL").flags(PropFlags::IS_FILENAME),
        PropDef::string("DynaData"),
        // Storage UserModel/UserData: no owned deck exercises them; the DLL
        // loader is still out of scope, so they remain a hard error for now.
        PropDef::string("UserModel").flags(PropFlags::NOT_PORTED | PropFlags::IS_FILENAME),
        PropDef::string("UserData").flags(PropFlags::NOT_PORTED),
        PropDef::boolean("DebugTrace"),
        PropDef::double("kVDC").scale(1000.0),
        PropDef::double("Kp").scale(1.0 / 1000.0),
        PropDef::double("PITol").scale(1.0 / 100.0),
        PropDef::double("SafeVoltage"),
        // Read-only dynamics state (Pascal SilentReadOnly): the setter is a no-op.
        PropDef::boolean("SafeMode"),
        PropDef::object_ref_class("DynamicExp", "DynamicEq"),
        PropDef::string_list("DynOut"),
        PropDef::mapped_string_enum("ControlMode", enums.inv_control_mode),
        PropDef::double("AmpLimit").flags(PropFlags::NO_DEFAULT),
        PropDef::double("AmpLimitGain"),
        // PCClass tail:
        PropDef::object_ref("Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("Storage", defs, true)
}

/// `TStorageObj`. `TStorageVars` (the public machine/state record) is flattened
/// into these fields plus the embedded [`InvBasedPceData`]; `MakeLike` copies them
/// the way Pascal does.
#[derive(Debug, Clone)]
pub struct Storage {
    pub cd: CktElementData,
    /// The shared inverter base (`TInvBasedPCE` data + `DynEqPCE` `DynamicEq`).
    pub base: InvBasedPceData,

    // --- TStorageVars (flattened) ---
    /// `kVStorageBase` — rated kV (`PresentkV`).
    pub kv_storage_base: f64,
    /// `kWrating`.
    pub kw_rating: f64,
    /// `kWhRating`.
    pub kwh_rating: f64,
    /// `kWhStored` — present state of charge (kWh).
    pub kwh_stored: f64,
    /// `kWhReserve` — minimum charge (kWh).
    pub kwh_reserve: f64,
    /// `ChargeEff` — charging efficiency (per-unit, = `%EffCharge`·0.01).
    pub charge_eff: f64,
    /// `DisChargeEff` — discharging efficiency (per-unit).
    pub discharge_eff: f64,
    /// `FkVArating`.
    pub f_kva_rating: f64,
    /// `RThev` — Thevenin R in ohms (harmonics; computed in `RecalcElementData`).
    pub r_thev: f64,
    /// `XThev` — Thevenin X in ohms.
    pub x_thev: f64,
    /// `Vthevharm` — harmonic Thevenin source magnitude (captured at the
    /// fundamental in `InitHarmonics`).
    pub v_thev_harm: f64,
    /// `ThetaHarm` — harmonic Thevenin source angle (radians).
    pub theta_harm: f64,
    /// `Fkvarlimit` — max kvar output (unsigned).
    pub f_kvar_limit: f64,
    /// `Fkvarlimitneg`.
    pub f_kvar_limit_neg: f64,
    /// `pctkWrated` — per-unit kW-rated output cap (`%kWRated`·0.01).
    pub pct_kw_rated: f64,
    /// `EffFactor` — present inverter efficiency.
    pub eff_factor: f64,
    /// `P_Priority` — watt priority (default false).
    pub p_priority: bool,
    /// `PF_Priority` (default false).
    pub pf_priority: bool,

    // Monitor results written by InvControl/ExpControl (mode-3 state, WP7.5/7.7);
    // initialised to 9999 by Create.
    pub vreg: f64,
    pub vavg: f64,
    pub vv_operation: f64,
    pub vw_operation: f64,
    pub drc_operation: f64,
    pub vv_drc_operation: f64,
    pub wp_operation: f64,
    pub wv_operation: f64,

    // --- TStorageObj scalars ---
    /// `PIdling` — idling power (kW): `%IdlingkW`·kWrating/100.
    pub p_idling: f64,
    /// `YeqDischarge` — equivalent at rated power of the Storage element only.
    pub yeq_discharge: Complex64,
    /// `MaxDynPhaseCurrent`.
    pub max_dyn_phase_current: f64,
    /// `FState` — charge/idle/discharge state.
    pub f_state: i32,
    /// `StateDesired` — desired state before any kWh-limit/cut-in/out change.
    pub state_desired: i32,
    /// `StateChanged` — set when the state flips (forces a Yprim rebuild).
    pub state_changed: bool,
    /// `StorageSolutionCount`.
    pub storage_solution_count: i32,
    /// `StorageObjSwitchOpen`.
    pub storage_obj_switch_open: bool,
    /// `FDCkW` — present DC-side kW (for the SOC update).
    pub f_dckw: f64,
    /// `kVA_exceeded`.
    pub kva_exceeded: bool,
    /// `kVASet`.
    pub kva_set: bool,
    /// `StorageClass`.
    pub storage_class: i32,
    /// `pctkWout` — % of kW rated currently dispatched (discharge).
    pub pct_kw_out: f64,
    /// `pctkWIn` — % charge.
    pub pct_kw_in: f64,
    /// `pctReserve`.
    pub pct_reserve: f64,
    /// `DispatchMode`.
    pub dispatch_mode: StorageDispatchMode,
    /// `pctIdlekW`.
    pub pct_idle_kw: f64,
    /// `pctIdlekvar` — deprecated/removed; kept for struct + MakeLike fidelity.
    pub pct_idle_kvar: f64,
    /// `kvarRequested` — the kvar setpoint (write target of the `kvar` property).
    pub kvar_requested: f64,
    /// `kWRequested` — the kW request (set by VW control, WP7.5).
    pub kw_requested: f64,
    /// `kWOutIdling` — present AC-side idling draw (kW).
    pub kw_out_idling: f64,
    /// `pctChargeEff`.
    pub pct_charge_eff: f64,
    /// `pctDischargeEff`.
    pub pct_discharge_eff: f64,
    /// `DischargeTrigger`.
    pub discharge_trigger: f64,
    /// `ChargeTrigger`.
    pub charge_trigger: f64,
    /// `ChargeTime` — time-of-day to begin charging (hours).
    pub charge_time: f64,
    /// `kWhBeforeUpdate`.
    pub kwh_before_update: f64,
    /// `CutOutkWAC` — `%CutOut` reflected to the AC side of the inverter.
    pub cut_out_kw_ac: f64,
    /// `CutInkWAC`.
    pub cut_in_kw_ac: f64,
    /// `FVWStateRequested` — VW control requested a specific state last iteration.
    pub fvw_state_requested: bool,
    /// `StorageFundamental` — base frequency captured for the harmonic model.
    pub storage_fundamental: f64,

    // Registers (energy meter).
    pub registers: [f64; NUM_STORAGE_REGISTERS],
    pub derivatives: [f64; NUM_STORAGE_REGISTERS],

    /// `SpectrumObj` name — Create sets it NIL (empty) for the inverter PCEs.
    pub spectrum: String,
    /// Resolved harmonic spectrum, snapshot-cloned in at edit-completion (only
    /// when an explicit `spectrum=` is given — Create forces `SpectrumObj := NIL`).
    pub spectrum_obj: Option<SpectrumObj>,
    /// `DynaModelNameStr` — user dynamics DLL name (NOT_PORTED; stored only).
    pub dyna_model_name: String,
    /// `DynaModelEditStr` (NOT_PORTED).
    pub dyna_model_edit: String,
}

/// Pascal `SetNcondsForConnection`.
fn nconds_for_connection(connection: Connection, nphases: usize) -> usize {
    match connection {
        Connection::Wye => nphases + 1,
        Connection::Delta => match nphases {
            1 | 2 => nphases + 1, // L-L and open-delta
            _ => nphases,
        },
    }
}

impl Storage {
    /// Pascal `TStorageObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 4; // defaults to wye
        cd.set_nterms(1);

        let mut base = InvBasedPceData::new();
        base.connection = Connection::Wye;
        base.voltage_model = 1; // typical fixed-kW negative load
        base.v_base = 7200.0;
        base.vminpu = 0.90;
        base.vmaxpu = 1.10;
        base.v_base_min = base.vminpu * base.v_base;
        base.v_base_max = base.vmaxpu * base.v_base;
        base.var_mode = VARMODE_PF;
        base.inverter_on = true;
        base.var_follow_inverter = false;
        base.force_balanced = false;
        base.current_limited = false;
        base.fpct_cut_in = 0.0;
        base.fpct_cut_out = 0.0;
        base.fpct_pmin_no_vars = 0.0;
        base.fpct_pmin_kvar_limit = 0.0;
        base.pf_wp_nominal = 1.0;
        base.kvar_out = 0.0;
        base.pf_nominal = 1.0;
        base.pct_r = 0.0;
        base.pct_x = 50.0;
        base.kvar_limit_set = false;
        base.kvar_limit_neg_set = false;
        base.debug_trace = false;
        // dynVars overrides (the rest are the InvBasedPCE base Create defaults:
        // ILimit = -1, IComp = 0, VError = 0.8).
        base.dyn_vars.rated_vdc = 8000.0;
        base.dyn_vars.sm_threshold = 80.0;
        base.dyn_vars.safe_mode = false;
        base.dyn_vars.kp = 0.00001;

        let kw_rating = 25.0;
        let kwh_rating = 50.0;
        let pct_reserve = 20.0;
        let mut st = Self {
            cd,
            base,
            kv_storage_base: 12.47,
            kw_rating,
            kwh_rating,
            kwh_stored: kwh_rating,
            kwh_reserve: kwh_rating * pct_reserve / 100.0,
            charge_eff: 0.0,
            discharge_eff: 0.0,
            f_kva_rating: kw_rating, // FkVArating := kWRating
            r_thev: 0.0,
            x_thev: 0.0,
            v_thev_harm: 0.0,
            theta_harm: 0.0,
            f_kvar_limit: kw_rating, // Fkvarlimit := FkVArating
            f_kvar_limit_neg: kw_rating,
            pct_kw_rated: 1.0,
            eff_factor: 1.0,
            p_priority: false,
            pf_priority: false,
            vreg: 9999.0,
            vavg: 9999.0,
            vv_operation: 9999.0,
            vw_operation: 9999.0,
            drc_operation: 9999.0,
            vv_drc_operation: 9999.0,
            wp_operation: 9999.0,
            wv_operation: 9999.0,
            p_idling: 0.0,
            yeq_discharge: Complex64::ZERO,
            max_dyn_phase_current: 0.0,
            f_state: STORE_IDLING, // idling and fully charged
            state_desired: STORE_IDLING,
            state_changed: true, // force building of YPrim
            storage_solution_count: -1,
            storage_obj_switch_open: false,
            f_dckw: 25.0,
            kva_exceeded: false,
            kva_set: false,
            storage_class: 1,
            pct_kw_out: 100.0,
            pct_kw_in: 100.0,
            pct_reserve,
            dispatch_mode: StorageDispatchMode::Default,
            pct_idle_kw: 1.0,
            pct_idle_kvar: 0.0,
            kvar_requested: 0.0,
            kw_requested: 0.0,
            kw_out_idling: 0.0,
            pct_charge_eff: 90.0,
            pct_discharge_eff: 90.0,
            discharge_trigger: 0.0,
            charge_trigger: 0.0,
            charge_time: 2.0, // 2 AM
            kwh_before_update: kwh_rating,
            cut_out_kw_ac: 0.0,
            cut_in_kw_ac: 0.0,
            fvw_state_requested: false,
            storage_fundamental: 0.0,
            registers: [0.0; NUM_STORAGE_REGISTERS],
            derivatives: [0.0; NUM_STORAGE_REGISTERS],
            spectrum: String::new(),
            spectrum_obj: None,
            dyna_model_name: String::new(),
            dyna_model_edit: String::new(),
        };
        // Pascal seeds PrpSequence with PF.
        st.cd.obj.set_as_next_seq(prop::PF);
        st.cd.inj_current = vec![Complex64::ZERO; st.cd.yorder];
        st.recalc(&crate::elements::pc::generator::default_recalc_ctx());
        st
    }
}
