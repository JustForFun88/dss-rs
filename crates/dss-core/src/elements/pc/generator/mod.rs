//! Port of `PCElements/generator.pas` — `TGeneratorObj`, the power-flow models.
//!
//! "The generator is essentially a negative load that can be dispatched." The
//! injection mirrors `Load`'s compensation form (`InjCurrent = Yprim·V` plus a
//! per-phase model current), but the model-current signs are reversed
//! (`StickCurrInTerminalArray` negates the opposite way) because the generator
//! pushes power into the node. Six power-flow models are ported
//! (`DoConstantPQGen` .. `DoCurrentLimitedPQ`); dynamics/harmonics/user-model
//! DLLs are Phase 7 / never (PHASE6_PLAN §2.5).
//!
//! Split into submodules (this file holds the metadata, struct and `Create`):
//! - [`nominal`]: dispatch/shape multipliers, `SetNominalGeneration` and
//!   `RecalcElementData` — the nominal-power machinery.
//! - [`solve`]: solve-time electrical machinery — `CalcYPrimMatrix`, the six
//!   `DoXxxGen` model currents and the injection-current assembly.
//! - [`registers`]: energy-meter registers, fuel and the Model-3 DQDV control.
//! - [`accessors`]: the `CktElement` / `DssObject` trait impls.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::traits::{ElemRef, SysCtx};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::solution::SolveMode;
use crate::support::cmatrix::CMatrix;
use crate::util::{CDOUBLEONE, sqrt3};

mod accessors;
mod nominal;
mod registers;
mod solve;

/// Pascal dispatch modes (`LOADMODE = 1`, `PRICEMODE = 2`; 0 = default).
const LOADMODE: i32 = 1;
const PRICEMODE: i32 = 2;

// Register indices (Pascal `Reg_kWh = 1` .. `Reg_Price = 6`, 0-based here).
const REG_KWH: usize = 0;
const REG_KVARH: usize = 1;
const REG_MAXKW: usize = 2;
const REG_MAXKVA: usize = 3;
const REG_HOURS: usize = 4;
const REG_PRICE: usize = 5;
const NUM_GEN_REGISTERS: usize = 6;

/// Pascal `TGeneralConnection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Wye = 0,
    Delta = 1,
}

/// 1-based property ordinals (Pascal `TGeneratorProp` + class tails).
pub mod prop {
    pub const PHASES: usize = 1;
    pub const BUS1: usize = 2;
    pub const KV: usize = 3;
    pub const KW: usize = 4;
    pub const PF: usize = 5;
    pub const KVAR: usize = 6;
    pub const MODEL: usize = 7;
    pub const VMINPU: usize = 8;
    pub const VMAXPU: usize = 9;
    pub const YEARLY: usize = 10;
    pub const DAILY: usize = 11;
    pub const DUTY: usize = 12;
    pub const DISPMODE: usize = 13;
    pub const DISPVALUE: usize = 14;
    pub const CONN: usize = 15;
    pub const STATUS: usize = 16;
    pub const CLS: usize = 17;
    pub const VPU: usize = 18;
    pub const MAXKVAR: usize = 19;
    pub const MINKVAR: usize = 20;
    pub const PVFACTOR: usize = 21;
    pub const FORCEON: usize = 22;
    pub const KVA: usize = 23;
    pub const MVA: usize = 24;
    pub const XD: usize = 25;
    pub const XDP: usize = 26;
    pub const XDPP: usize = 27;
    pub const H: usize = 28;
    pub const D: usize = 29;
    pub const USERMODEL: usize = 30;
    pub const USERDATA: usize = 31;
    pub const SHAFTMODEL: usize = 32;
    pub const SHAFTDATA: usize = 33;
    pub const DUTYSTART: usize = 34;
    pub const DEBUGTRACE: usize = 35;
    pub const BALANCED: usize = 36;
    pub const XRDP: usize = 37;
    pub const USEFUEL: usize = 38;
    pub const FUELKWH: usize = 39;
    pub const PCTFUEL: usize = 40;
    pub const PCTRESERVE: usize = 41;
    pub const REFUEL: usize = 42;
    pub const DYNAMICEQ: usize = 43;
    pub const DYNOUT: usize = 44;
    // tails:
    pub const SPECTRUM: usize = 45;
    pub const BASE_FREQ: usize = 46;
    pub const ENABLED: usize = 47;
    pub const NUM_PROPS: usize = 48; // incl. Like
}

/// `TGenerator.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("bus1", 1),
        PropDef::double("kV").flags(PropFlags::NON_NEGATIVE),
        PropDef::double("kW"),
        PropDef::double("PF"),
        PropDef::double("kvar"),
        PropDef::mapped_int_enum("model", enums.gen_model),
        PropDef::double("Vminpu"),
        PropDef::double("Vmaxpu"),
        PropDef::object_ref_class("LoadShape", "yearly"),
        PropDef::object_ref_class("LoadShape", "daily"),
        PropDef::object_ref_class("LoadShape", "duty"),
        PropDef::mapped_string_enum("dispmode", enums.gen_disp_mode),
        PropDef::double("dispvalue"),
        PropDef::mapped_string_enum("conn", enums.connection),
        PropDef::mapped_string_enum("status", enums.gen_status),
        PropDef::integer("class"),
        PropDef::double("Vpu"),
        PropDef::double("maxkvar"),
        PropDef::double("minkvar"),
        PropDef::double("pvfactor"),
        PropDef::boolean("forceon"),
        PropDef::double("kVA"),
        PropDef::double("MVA")
            .scale(1000.0)
            .flags(PropFlags::REDUNDANT),
        PropDef::double("Xd"),
        PropDef::double("Xdp"),
        PropDef::double("Xdpp"),
        PropDef::double("H"),
        PropDef::double("D"),
        // User-written model DLLs are never ported (safe-Rust); stored + dumped
        // but setting one is a hard error.
        PropDef::string("UserModel").flags(PropFlags::NOT_PORTED | PropFlags::IS_FILENAME),
        PropDef::string("UserData").flags(PropFlags::NOT_PORTED),
        PropDef::string("ShaftModel").flags(PropFlags::NOT_PORTED | PropFlags::IS_FILENAME),
        PropDef::string("ShaftData").flags(PropFlags::NOT_PORTED),
        PropDef::double("DutyStart"),
        PropDef::boolean("debugtrace"),
        PropDef::boolean("Balanced"),
        PropDef::double("XRdp"),
        PropDef::boolean("UseFuel"),
        PropDef::double("FuelkWh"),
        PropDef::double("%Fuel"),
        PropDef::double("%Reserve"),
        // BooleanActionProperty: setting it `yes` refuels; the getter is 0.
        PropDef::boolean("Refuel"),
        // Dynamics machinery → Phase 7.
        PropDef::string("DynamicEq").flags(PropFlags::NOT_PORTED),
        PropDef::string("DynOut").flags(PropFlags::NOT_PORTED),
        // PCClass tail:
        PropDef::object_ref("spectrum"),
        // CktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("Generator", defs, true)
}

/// `TGeneratorObj`. `GenVars` (the public machine/state record) is flattened
/// into these fields; `MakeLike` copies them wholesale as Pascal does.
#[derive(Debug, Clone)]
pub struct Generator {
    pub cd: CktElementData,

    pub connection: Connection,
    pub gen_model: i32,
    pub gen_class: i32,
    pub kw_base: f64,
    pub kvar_base: f64,
    pub kvar_max: f64,
    pub kvar_min: f64,
    pub pf_nominal: f64,
    pub vpu: f64,
    pub vmaxpu: f64,
    pub vminpu: f64,
    pub is_fixed: bool,  // Status = Fixed
    pub forced_on: bool, // ForceOn
    pub dispatch_mode: i32,
    pub dispatch_value: f64,
    pub pv_factor: f64,
    pub duty_start: f64,
    pub debug_trace: bool,
    pub force_balanced: bool,

    // Fuel.
    pub use_fuel: bool,
    pub fuel_kwh: f64,
    pub pct_fuel: f64,
    pub pct_reserve: f64,
    pub gen_active: bool,

    // GenVars (machine record).
    pub kv_generator_base: f64,
    pub kva_rating: f64,
    pub pu_xd: f64,
    pub pu_xdp: f64,
    pub pu_xdpp: f64,
    pub xd: f64,
    pub xdp: f64,
    pub xdpp: f64,
    pub h_mass: f64,
    pub dpu: f64,
    pub xrdp: f64,
    pub v_target: f64,
    pub p_nominal_per_phase: f64,
    pub q_nominal_per_phase: f64,

    // Derived (RecalcElementData / SetNominalGeneration).
    pub v_base: f64,
    pub v_base95: f64,
    pub v_base105: f64,
    pub var_base: f64,
    pub var_max: f64,
    pub var_min: f64,
    pub yeq: Complex64,
    pub yeq95: Complex64,
    pub yeq105: Complex64,
    pub yq_fixed: f64,
    pub phase_current_limit: Complex64,
    pub model7_max_phase_curr: f64,

    pub gen_on: bool,
    pub gen_switch_open: bool,
    pub kva_not_set: bool,
    pub shape_factor: Complex64,
    pub shape_is_actual: bool,
    pub v_avg: f64,

    // Model-3 DQDV var-control state.
    pub dqdv: f64,
    pub dqdv_saved: f64,
    pub delta_q_max: f64,

    pub gen_solution_count: i32,
    pub open_gen_solution_count: i32,
    pub yprim_open_cond: Option<CMatrix>,

    // Registers (energy meter).
    pub registers: [f64; NUM_GEN_REGISTERS],
    pub derivatives: [f64; NUM_GEN_REGISTERS],
    pub first_sample_after_reset: bool,

    // Strings (NOT_PORTED DLL/dynamics references, stored for the dump).
    pub user_model_name: String,
    pub user_data: String,
    pub shaft_model_name: String,
    pub shaft_data: String,
    pub dynamic_eq: String,
    pub dyn_out: String,
    pub spectrum: String,

    pub yearly_shape: String,
    pub daily_shape: String,
    pub duty_shape: String,
    pub yearly_shape_obj: Option<LoadShapeObj>,
    pub daily_shape_obj: Option<LoadShapeObj>,
    pub duty_shape_obj: Option<LoadShapeObj>,
    pub yearly_shape_ref: Option<ElemRef>,
    pub daily_shape_ref: Option<ElemRef>,
    pub duty_shape_ref: Option<ElemRef>,
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

impl Generator {
    /// Pascal `TGeneratorObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 4; // defaults to wye
        cd.set_nterms(1);

        let kw_base = 1000.0;
        let kvar_base = 60.0;
        let kv_generator_base = 12.47;
        let kva_rating = kw_base * 1.2;
        let pu_xd = 1.0;
        let pu_xdp = 0.28;
        let pu_xdpp = 0.20;
        let vpu = 1.0;
        let v_base = 7200.0;
        let vminpu = 0.90;
        let vmaxpu = 1.10;

        let mut g = Self {
            cd,
            connection: Connection::Wye,
            gen_model: 1,
            gen_class: 1,
            kw_base,
            kvar_base,
            kvar_max: kvar_base * 2.0,
            kvar_min: -kvar_base * 2.0,
            pf_nominal: 0.88,
            vpu,
            vmaxpu,
            vminpu,
            is_fixed: false,
            forced_on: false,
            dispatch_mode: 0,
            dispatch_value: 0.0,
            pv_factor: 0.1,
            duty_start: 0.0,
            debug_trace: false,
            force_balanced: false,
            use_fuel: false,
            fuel_kwh: 0.0,
            pct_fuel: 100.0,
            pct_reserve: 20.0,
            gen_active: true,
            kv_generator_base,
            kva_rating,
            pu_xd,
            pu_xdp,
            pu_xdpp,
            xd: pu_xd * kv_generator_base.powi(2) * 1000.0 / kva_rating,
            xdp: pu_xdp * kv_generator_base.powi(2) * 1000.0 / kva_rating,
            xdpp: pu_xdpp * kv_generator_base.powi(2) * 1000.0 / kva_rating,
            h_mass: 1.0,
            dpu: 1.0,
            xrdp: 20.0,
            v_target: 1000.0 * vpu * kv_generator_base / sqrt3(),
            p_nominal_per_phase: 0.0,
            q_nominal_per_phase: 0.0,
            v_base,
            v_base95: vminpu * v_base,
            v_base105: vmaxpu * v_base,
            var_base: 0.0,
            var_max: 0.0,
            var_min: 0.0,
            yeq: Complex64::ZERO,
            yeq95: Complex64::ZERO,
            yeq105: Complex64::ZERO,
            yq_fixed: 0.0,
            phase_current_limit: Complex64::ZERO,
            model7_max_phase_curr: 0.0,
            gen_on: false,
            gen_switch_open: false,
            kva_not_set: true,
            shape_factor: CDOUBLEONE,
            shape_is_actual: false,
            v_avg: 0.0,
            dqdv: 0.0,
            dqdv_saved: 0.0,
            delta_q_max: 0.0,
            gen_solution_count: -1,
            open_gen_solution_count: -1,
            yprim_open_cond: None,
            registers: [0.0; NUM_GEN_REGISTERS],
            derivatives: [0.0; NUM_GEN_REGISTERS],
            first_sample_after_reset: true,
            user_model_name: String::new(),
            user_data: String::new(),
            shaft_model_name: String::new(),
            shaft_data: String::new(),
            dynamic_eq: String::new(),
            dyn_out: String::new(),
            spectrum: "defaultgen".to_string(),
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            yearly_shape_obj: None,
            daily_shape_obj: None,
            duty_shape_obj: None,
            yearly_shape_ref: None,
            daily_shape_ref: None,
            duty_shape_ref: None,
        };
        // Pascal seeds PrpSequence with kW then PF.
        g.cd.obj.set_as_next_seq(prop::KW);
        g.cd.obj.set_as_next_seq(prop::PF);
        g.cd.inj_current = vec![Complex64::ZERO; g.cd.yorder];
        g.recalc(&default_recalc_ctx());
        g
    }
}

/// The snapshot defaults the executive uses for `RecalcElementData` at parse
/// time (every derived value is recomputed in `CalcYPrim` before use).
pub fn default_recalc_ctx() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: crate::solution::POWERFLOW,
        mode: SolveMode::Snapshot,
        load_multiplier: 1.0,
        gen_multiplier: 1.0,
        generator_dispatch_reference: 0.0,
        price_signal: 25.0,
        default_growth_factor: 1.0,
        year: 0,
        dbl_hour: 0.0,
        solution_count: 0,
        loads_need_updating: true,
        neglect_load_y: false,
        long_line_correction: false,
        positive_sequence: false,
    }
}
