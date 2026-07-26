//! Port of `PCElements/PVsystem.pas` — `TPVsystemObj`, the photovoltaic system
//! PC element, built on the Generator injection template (WP6.2) with the shared
//! inverter base [`InvBasedPceData`] (WP7.3 step 1) embedded the way [`Generator`]
//! flattens `GenVars`.
//!
//! The PV panel power is irradiance·`Pmpp`·shape·temperature-derate
//! (`ComputePanelPower`); the inverter then applies cut-in/cut-out, the
//! efficiency curve, the watt/var priority and the `kVA`/kvar clamps
//! (`ComputeInverterPower`). The result is a per-phase P/Q that injects exactly
//! like a Generator's constant-PQ model (the model-current signs push power into
//! the node; `StickCurrInTerminalArray` on the base routes wye/delta).
//!
//! Scope (WP7.3 step 2): the **power-flow** PVSystem — `SetNominalDEROutput`
//! (`SetNominalPVSystem`), the P-T-V curves (XYcurve) + irradiance/temperature
//! shapes (snapshot-clone), `CalcYPrim`, `DoConstantPQPVsystemObj` /
//! `DoConstantZPVsystemObj` + the inverter clamp, the energy-meter registers and
//! `TakeSample`. The grid-forming mode (`DoGFM_Mode`/`CalcGFMYprim`), the
//! harmonic injection (`DoHarmonicMode`/`InitHarmonics`), the dynamics state
//! machinery (`DoDynamicMode`/`InitStateVars`/`IntegrateStates` + the
//! state-variable interface `NumVariables`/`Get_Variable`/`VariableName`), and
//! the user-written model (`DoUserModel`, VoltageModel=3 — `user_model.rs`, the
//! WASM host, WASM_USERMODELS WM.4) are ported. `MakePosSequence` is in
//! `accessors` (WPG.21).
//!
//! Split into submodules (this file holds the metadata, struct and `Create`):
//! - [`nominal`]: shape/temperature multipliers, `ComputePanelPower` /
//!   `ComputeInverterPower` / `kWOut_Calc`, `SetNominalDEROutput` and
//!   `RecalcElementData`.
//! - [`solve`]: `CalcYPrimMatrix`, the `DoConstantPQ/Z` model currents and the
//!   injection-current assembly.
//! - [`registers`]: energy-meter registers and `TakeSample`.
//! - [`accessors`]: the `CktElement` / `DssObject` / [`InvBasedPce`] trait impls.
//!
//! [`Generator`]: crate::elements::pc::generator::Generator
//! [`InvBasedPce`]: crate::elements::pc::inv_based_pce::InvBasedPce

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::temp_shape::TShapeObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pc::inv_based_pce::{Connection, InvBasedPceData};
use crate::elements::traits::ElemId;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

mod accessors;
mod dynamics;
mod nominal;
mod registers;
mod solve;
mod user_model;

pub use user_model::PvUserModelSlot;

/// Pascal `varMode` values.
pub(crate) const VARMODE_PF: i32 = 0;
pub(crate) const VARMODE_KVAR: i32 = 1;

// Register indices (Pascal `Reg_kWh = 1` .. `Reg_Price = 6`, 0-based here).
const REG_KWH: usize = 0;
const REG_KVARH: usize = 1;
const REG_MAXKW: usize = 2;
const REG_MAXKVA: usize = 3;
const REG_HOURS: usize = 4;
const REG_PRICE: usize = 5;
const NUM_PVSYSTEM_REGISTERS: usize = 6;

/// 1-based property ordinals (Pascal `TPVSystemProp` + class tails).
pub mod prop {
    pub const PHASES: usize = 1;
    pub const BUS1: usize = 2;
    pub const KV: usize = 3;
    pub const IRRADIANCE: usize = 4;
    pub const PMPP: usize = 5;
    pub const PCT_PMPP: usize = 6;
    pub const TEMPERATURE: usize = 7;
    pub const PF: usize = 8;
    pub const CONN: usize = 9;
    pub const KVAR: usize = 10;
    pub const KVA: usize = 11;
    pub const PCT_CUTIN: usize = 12;
    pub const PCT_CUTOUT: usize = 13;
    pub const EFF_CURVE: usize = 14;
    pub const P_T_CURVE: usize = 15;
    pub const PCT_R: usize = 16;
    pub const PCT_X: usize = 17;
    pub const MODEL: usize = 18;
    pub const VMINPU: usize = 19;
    pub const VMAXPU: usize = 20;
    pub const BALANCED: usize = 21;
    pub const LIMIT_CURRENT: usize = 22;
    pub const YEARLY: usize = 23;
    pub const DAILY: usize = 24;
    pub const DUTY: usize = 25;
    pub const TYEARLY: usize = 26;
    pub const TDAILY: usize = 27;
    pub const TDUTY: usize = 28;
    pub const CLS: usize = 29;
    pub const USERMODEL: usize = 30;
    pub const USERDATA: usize = 31;
    pub const DEBUGTRACE: usize = 32;
    pub const VAR_FOLLOW_INVERTER: usize = 33;
    pub const DUTYSTART: usize = 34;
    pub const WATT_PRIORITY: usize = 35;
    pub const PF_PRIORITY: usize = 36;
    pub const PCT_PMIN_NO_VARS: usize = 37;
    pub const PCT_PMIN_KVAR_MAX: usize = 38;
    pub const KVAR_MAX: usize = 39;
    pub const KVAR_MAX_ABS: usize = 40;
    pub const KVDC: usize = 41;
    pub const KP: usize = 42;
    pub const PITOL: usize = 43;
    pub const SAFE_VOLTAGE: usize = 44;
    pub const SAFE_MODE: usize = 45;
    pub const DYNAMIC_EQ: usize = 46;
    pub const DYN_OUT: usize = 47;
    pub const CONTROL_MODE: usize = 48;
    pub const AMP_LIMIT: usize = 49;
    pub const AMP_LIMIT_GAIN: usize = 50;
    // PCElement / CktElement tails:
    pub const SPECTRUM: usize = 51;
    pub const BASE_FREQ: usize = 52;
    pub const ENABLED: usize = 53;
    pub const NUM_PROPS: usize = 54; // incl. Like
}

/// `TPVSystem.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::double("kV")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::REQUIRED | PropFlags::UNITS_KV),
        PropDef::double("Irradiance"),
        PropDef::double("Pmpp"),
        PropDef::double("%Pmpp").scale(0.01),
        PropDef::double("Temperature"),
        PropDef::double("PF")
            .flags(PropFlags::POWER_FACTOR_LIMITS | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::mapped_string_enum("Conn", enums.connection),
        // `kvar`: read returns `kvar_out` (Pascal `Getkvar`), write stores
        // `kvarRequested`.
        PropDef::double("kvar").flags(PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("kVA").flags(PropFlags::REPLACE_ZERO),
        PropDef::double("%CutIn"),
        PropDef::double("%CutOut"),
        PropDef::object_ref_class("XYcurve", "EffCurve"),
        // Pascal `PropertyNameJSON[P__TCurve] := 'PTCurve'` (PVsystem.pas:430):
        // an explicit JSON-name override (the default `-`→`__` map would give
        // `P__TCurve`).
        PropDef::object_ref_class("XYcurve", "P-TCurve").json_name("PTCurve"),
        PropDef::double("%R"),
        PropDef::double("%X"),
        PropDef::mapped_int_enum("Model", enums.pvsystem_model),
        PropDef::double("VMinpu"),
        PropDef::double("VMaxpu"),
        PropDef::boolean("Balanced"),
        PropDef::boolean("LimitCurrent"),
        PropDef::object_ref_class("LoadShape", "Yearly"),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty"),
        PropDef::object_ref_class("TShape", "TYearly"),
        PropDef::object_ref_class("TShape", "TDaily"),
        PropDef::object_ref_class("TShape", "TDuty"),
        PropDef::integer("Class"),
        // WASM_USERMODELS WM.4: `UserModel=`/`UserData=` follow the §2.4 uniform
        // rule (parse + store + load-`.wasm`-or-warn); upstream never errors on
        // these properties (PVsystem.pas:628-632).
        PropDef::string("UserModel").flags(PropFlags::IS_FILENAME),
        PropDef::string("UserData"),
        PropDef::boolean("DebugTrace"),
        PropDef::boolean("VarFollowInverter"),
        PropDef::double("DutyStart").flags(PropFlags::NON_NEGATIVE | PropFlags::UNITS_HOUR),
        PropDef::boolean("WattPriority"),
        PropDef::boolean("PFPriority"),
        PropDef::double("%PMinNoVars"),
        PropDef::double("%PMinkvarMax"),
        PropDef::double("kvarMax").flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::double("kvarMaxAbs").flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::double("kVDC").scale(1000.0),
        PropDef::double("Kp").scale(1.0 / 1000.0),
        PropDef::double("PITol").scale(1.0 / 100.0),
        PropDef::double("SafeVoltage"),
        // Read-only dynamics state (Pascal SilentReadOnly): the setter is a no-op.
        // Pascal `SilentReadOnly` (PVsystem.pas:516): schema `readOnly`, but the
        // text/JSON dump still reads the live Yes/No value — the schema-only
        // `READ_ONLY` flag (cf. Storage.SafeMode), NOT `SILENT_READ_ONLY` (which
        // would suppress the dump read too).
        PropDef::boolean("SafeMode").flags(PropFlags::READ_ONLY),
        PropDef::object_ref_class("DynamicExp", "DynamicEq"),
        PropDef::string_list("DynOut"),
        PropDef::mapped_string_enum("ControlMode", enums.inv_control_mode),
        PropDef::double("AmpLimit").flags(PropFlags::NO_DEFAULT),
        PropDef::double("AmpLimitGain"),
        // PCClass tail:
        PropDef::object_ref_deferred("Spectrum", "Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("PVSystem", defs, true)
}

/// `TPVsystemObj`. `TPVSystemVars` (the public machine/state record) is flattened
/// into these fields plus the embedded [`InvBasedPceData`]; `MakeLike` copies them
/// the way Pascal does.
#[derive(Debug, Clone)]
pub struct PVSystem {
    pub cd: CktElementData,
    /// The shared inverter base (`TInvBasedPCE` data + `DynEqPCE` `DynamicEq`).
    pub base: InvBasedPceData,

    // --- TPVSystemVars (flattened) ---
    /// `kVPVSystemBase` — rated kV (`PresentkV`).
    pub kv_pvsystem_base: f64,
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
    /// `FTemperature`.
    pub f_temperature: f64,
    /// `FPmpp`.
    pub f_pmpp: f64,
    /// `FpuPmpp` — per-unit Pmpp (`%Pmpp`).
    pub f_pu_pmpp: f64,
    /// `FIrradiance`.
    pub f_irradiance: f64,
    /// `MaxDynPhaseCurrent`.
    pub max_dyn_phase_current: f64,
    /// `Fkvarlimit` — max kvar output (unsigned).
    pub f_kvar_limit: f64,
    /// `Fkvarlimitneg`.
    pub f_kvar_limit_neg: f64,
    /// `EffFactor` — present inverter efficiency.
    pub eff_factor: f64,
    /// `TempFactor` — present temperature derate.
    pub temp_factor: f64,
    /// `PanelkW` — present DC panel power.
    pub panel_kw: f64,
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

    // --- TPVsystemObj scalars ---
    /// `FClass`.
    pub f_class: i32,
    /// `kvarRequested` — the kvar setpoint (write target of the `kvar` property).
    pub kvar_requested: f64,
    /// `kWRequested`.
    pub kw_requested: f64,
    /// `TShapeValue` — present temperature from the active T-shape.
    pub t_shape_value: f64,
    /// `DutyStart` — start time offset into the duty shape [hrs].
    pub duty_start: f64,
    /// `varBase` — base vars per phase.
    pub var_base: f64,
    /// `PVSystemSolutionCount`.
    pub pv_system_solution_count: i32,
    /// `PVSystemFundamental` — the solution frequency when harmonics mode is
    /// entered (the harmonic ratio's denominator).
    pub pv_system_fundamental: f64,
    /// `PVsystemObjSwitchOpen`.
    pub pv_system_obj_switch_open: bool,

    // Registers (energy meter).
    pub registers: [f64; NUM_PVSYSTEM_REGISTERS],
    pub derivatives: [f64; NUM_PVSYSTEM_REGISTERS],

    /// `SpectrumObj` name — Create sets it NIL (empty) for the inverter PCEs.
    pub spectrum: String,
    /// Resolved harmonic spectrum, snapshot-cloned in at edit-completion (only
    /// when an explicit `spectrum=` is given — Create forces `SpectrumObj := NIL`).
    pub spectrum_obj: Option<SpectrumObj>,

    // Temperature-shape references (snapshot-clone + ElemId, WP4.2/WP5.3).
    pub yearly_t_shape: String,
    pub daily_t_shape: String,
    pub duty_t_shape: String,
    pub yearly_t_shape_obj: Option<TShapeObj>,
    pub daily_t_shape_obj: Option<TShapeObj>,
    pub duty_t_shape_obj: Option<TShapeObj>,
    pub yearly_t_shape_ref: Option<ElemId>,
    pub daily_t_shape_ref: Option<ElemId>,
    pub duty_t_shape_ref: Option<ElemId>,

    /// `Power_TempCurveObj` — pu-Pmpp-vs-temperature curve (XYcurve).
    pub power_temp_curve: String,
    pub power_temp_curve_obj: Option<XyCurveObj>,
    pub power_temp_curve_ref: Option<ElemId>,

    /// Pascal `UserModel: TPVsystemUserModel` (`PVsystem.pas:228`) — the 15-fn
    /// `UserModel=` slot (WASM_USERMODELS WM.4). `None` until a `.wasm` loads.
    pub user_model: Option<Box<PvUserModelSlot>>,
    /// Deferred `UserModel=`/`UserData=` load/edit requests, drained + resolved
    /// by the executive (§2.4 activation rule).
    pub pending_user_model_loads: Vec<crate::obj::base::UserModelLoad>,
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

impl PVSystem {
    /// Pascal `TPVsystemObj.Create`.
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
        base.fpct_cut_in = 20.0;
        base.fpct_cut_out = 20.0;
        base.fpct_pmin_no_vars = 0.0;
        base.fpct_pmin_kvar_limit = 0.0;
        base.pf_wp_nominal = 1.0;
        base.kw_out = 500.0;
        base.kvar_out = 0.0;
        base.pf_nominal = 1.0;
        base.pct_r = 50.0;
        base.pct_x = 0.0;
        base.kvar_limit_set = false;
        base.kvar_limit_neg_set = false;
        base.debug_trace = false;
        // dynVars overrides (the rest are the InvBasedPCE base Create defaults:
        // ILimit = -1, IComp = 0, VError = 0.8).
        base.dyn_vars.rated_vdc = 8000.0;
        base.dyn_vars.sm_threshold = 80.0;
        base.dyn_vars.safe_mode = false;
        base.dyn_vars.kp = 0.00001;

        let mut pv = Self {
            cd,
            base,
            kv_pvsystem_base: 12.47,
            f_kva_rating: 500.0,
            r_thev: 0.0,
            x_thev: 0.0,
            v_thev_harm: 0.0,
            theta_harm: 0.0,
            f_temperature: 25.0,
            f_pmpp: 500.0,
            f_pu_pmpp: 1.0, // full on
            f_irradiance: 1.0,
            max_dyn_phase_current: 0.0,
            f_kvar_limit: 500.0, // = FkVArating
            f_kvar_limit_neg: 500.0,
            eff_factor: 1.0,
            temp_factor: 1.0,
            panel_kw: 0.0,
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
            f_class: 1,
            kvar_requested: 0.0,
            kw_requested: 0.0,
            t_shape_value: 25.0,
            duty_start: 0.0,
            var_base: 0.0,
            pv_system_solution_count: -1,
            pv_system_fundamental: 0.0,
            pv_system_obj_switch_open: false,
            registers: [0.0; NUM_PVSYSTEM_REGISTERS],
            derivatives: [0.0; NUM_PVSYSTEM_REGISTERS],
            spectrum: String::new(),
            spectrum_obj: None,
            yearly_t_shape: String::new(),
            daily_t_shape: String::new(),
            duty_t_shape: String::new(),
            yearly_t_shape_obj: None,
            daily_t_shape_obj: None,
            duty_t_shape_obj: None,
            yearly_t_shape_ref: None,
            daily_t_shape_ref: None,
            duty_t_shape_ref: None,
            power_temp_curve: String::new(),
            power_temp_curve_obj: None,
            power_temp_curve_ref: None,
            user_model: None,
            pending_user_model_loads: Vec::new(),
        };
        // Pascal seeds PrpSequence with PF.
        pv.cd.obj.set_as_next_seq(prop::PF);
        pv.cd.inj_current = vec![Complex64::ZERO; pv.cd.yorder];
        // Pascal `TPVsystemObj.Create` ends with `RecalcElementData` (live
        // `ActiveCircuit.Solution`). `new` has no circuit; the executive runs that
        // live recalc after construction (`create_object_no_edit`) and at
        // `end_edit`. Direct-construction unit tests recalc explicitly.
        pv
    }
}
