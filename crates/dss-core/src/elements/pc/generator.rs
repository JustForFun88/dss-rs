//! Port of `PCElements/generator.pas` — `TGeneratorObj`, the power-flow models.
//!
//! "The generator is essentially a negative load that can be dispatched." The
//! injection mirrors `Load`'s compensation form (`InjCurrent = Yprim·V` plus a
//! per-phase model current), but the model-current signs are reversed
//! (`StickCurrInTerminalArray` negates the opposite way) because the generator
//! pushes power into the node. Six power-flow models are ported
//! (`DoConstantPQGen` .. `DoCurrentLimitedPQ`); dynamics/harmonics/user-model
//! DLLs are Phase 7 / never (PHASE6_PLAN §2.5).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::traits::{CktElement, ElemRef, InjCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::solution::SolveMode;
use crate::support::cmatrix::CMatrix;
use crate::support::mathutil::SymComp;
use crate::util::{CDOUBLEONE, inv_sqrt3_x1000, sqrt3};

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

    /// Pascal `CalcDailyMult`.
    fn calc_daily_mult(&mut self, hr: f64) {
        if let Some(s) = self.daily_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = CDOUBLEONE;
        }
    }

    /// Pascal `CalcDutyMult` (falls back to daily; includes `DutyStart` offset).
    fn calc_duty_mult(&mut self, hr: f64) {
        if let Some(s) = self.duty_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr + self.duty_start);
            self.shape_is_actual = s.use_actual();
        } else {
            self.calc_daily_mult(hr);
        }
    }

    /// Pascal `CalcYearlyMult`.
    fn calc_yearly_mult(&mut self, hr: f64) {
        if let Some(s) = self.yearly_shape_obj.as_mut() {
            self.shape_factor = s.get_mult_at_hour(hr);
            self.shape_is_actual = s.use_actual();
        } else {
            self.shape_factor = CDOUBLEONE;
        }
    }

    /// Pascal `SyncUpPowerQuantities`: keep kvar nominal in step with kW/PF.
    fn sync_up_power_quantities(&mut self) {
        if self.pf_nominal == 0.0 {
            return;
        }
        self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
        self.p_nominal_per_phase = 1000.0 * self.kw_base / self.cd.nphases as f64;
        self.kvar_max = 2.0 * self.kvar_base;
        self.kvar_min = -self.kvar_max;
        if self.pf_nominal < 0.0 {
            self.kvar_base = -self.kvar_base;
        }
        self.q_nominal_per_phase = 1000.0 * self.kvar_base / self.cd.nphases as f64;
        if self.kva_not_set {
            self.kva_rating = self.kw_base * 1.2;
        }
    }

    /// Pascal `SetkWkvar`: set base kW/kvar then run the `kvar` side effects.
    fn set_kw_kvar(&mut self, p_kw: f64, q_kvar: f64) {
        self.kw_base = p_kw;
        self.kvar_base = q_kvar;
        self.side_effect_kvar();
    }

    /// Pascal `TProp.kvar` side effect.
    fn side_effect_kvar(&mut self) {
        let nphases = self.cd.nphases as f64;
        self.q_nominal_per_phase = 1000.0 * self.kvar_base / nphases;
        let kva_gen = (self.kw_base.powi(2) + self.kvar_base.powi(2)).sqrt();
        self.pf_nominal = if kva_gen != 0.0 {
            self.kw_base / kva_gen
        } else {
            1.0
        };
        if self.kw_base * self.kvar_base < 0.0 {
            self.pf_nominal = -self.pf_nominal;
        }
        self.kvar_max = 2.0 * self.kvar_base;
        self.kvar_min = -self.kvar_max;
        self.cd.obj.clear_seq(prop::PF);
    }

    /// The shared VBase update (Pascal `TProp.conn`/`TProp.kV` side effects):
    /// L-N for 2/3-phase, otherwise the supplied value (independent of the
    /// connection — Pascal uses the same formula for wye and delta here).
    fn update_vbase(&mut self) {
        self.v_base = match self.cd.nphases {
            2 | 3 => self.kv_generator_base * inv_sqrt3_x1000(),
            _ => self.kv_generator_base * 1000.0,
        };
    }

    /// Pascal `SetNominalGeneration` (the power-flow path; dynamics/harmonics
    /// branches are never entered in Phase 6 and are omitted).
    pub fn set_nominal_generation(&mut self, sys: &SysCtx) {
        let gen_on_saved = self.gen_on;
        self.shape_factor = CDOUBLEONE;

        // Decide whether the generator is ON (LOADMODE compares the dispatch
        // reference, PRICEMODE the price signal, both against DispValue).
        self.gen_on = true;
        if !self.forced_on && self.dispatch_value > 0.0 {
            let off_load = self.dispatch_mode == LOADMODE
                && sys.generator_dispatch_reference < self.dispatch_value;
            let off_price =
                self.dispatch_mode == PRICEMODE && sys.price_signal < self.dispatch_value;
            if off_load || off_price {
                self.gen_on = false;
            }
        }

        let nphases = self.cd.nphases as f64;
        if !self.gen_on {
            // OFF: a tiny resistive load so the matrix doesn't go singular.
            self.p_nominal_per_phase = -0.1 * self.kw_base / nphases;
            self.q_nominal_per_phase = 0.0;
        } else {
            let factor = if self.is_fixed {
                1.0
            } else {
                match sys.mode {
                    SolveMode::Snapshot
                    | SolveMode::Monte1
                    | SolveMode::MonteFault
                    | SolveMode::FaultStudy => sys.gen_multiplier,
                    SolveMode::Daily
                    | SolveMode::Monte2
                    | SolveMode::Monte3
                    | SolveMode::LD1
                    | SolveMode::LD2
                    | SolveMode::PeakDay => {
                        let f = sys.gen_multiplier;
                        self.calc_daily_mult(sys.dbl_hour);
                        f
                    }
                    SolveMode::Yearly => {
                        let f = sys.gen_multiplier;
                        self.calc_yearly_mult(sys.dbl_hour);
                        f
                    }
                    SolveMode::DutyCycle => {
                        let f = sys.gen_multiplier;
                        self.calc_duty_mult(sys.dbl_hour);
                        f
                    }
                    SolveMode::Time | SolveMode::Dynamic => {
                        // GENERALTIME / DYNAMICMODE: one load-shape class.
                        // ActiveLoadShapeClass is `USENONE` by default → 1+j1.
                        sys.gen_multiplier
                    }
                    SolveMode::AutoAdd => 1.0,
                    _ => 1.0,
                }
            };

            if self.shape_is_actual {
                self.p_nominal_per_phase = 1000.0 * self.shape_factor.re / nphases;
            } else {
                self.p_nominal_per_phase =
                    1000.0 * self.kw_base * factor * self.shape_factor.re / nphases;
            }

            if self.gen_model == 3 {
                // Just make sure the present value is reasonable.
                if self.q_nominal_per_phase > self.var_max {
                    self.q_nominal_per_phase = self.var_max;
                } else if self.q_nominal_per_phase < self.var_min {
                    self.q_nominal_per_phase = self.var_min;
                }
            } else if self.shape_is_actual {
                self.q_nominal_per_phase = 1000.0 * self.shape_factor.im / nphases;
            } else {
                self.q_nominal_per_phase =
                    1000.0 * self.kvar_base * factor * self.shape_factor.im / nphases;
            }
        }

        if self.gen_model == 6 {
            self.yeq = Complex64::new(0.0, -self.xd).inv(); // gets negated in CalcYPrim
        } else {
            self.yeq = Complex64::new(self.p_nominal_per_phase, -self.q_nominal_per_phase)
                / self.v_base.powi(2); // Vbase L-N for 3-phase
            self.yeq95 = if self.vminpu != 0.0 {
                self.yeq / self.vminpu.powi(2)
            } else {
                self.yeq // always a constant-Z model
            };
            self.yeq105 = if self.vmaxpu != 0.0 {
                self.yeq / self.vmaxpu.powi(2)
            } else {
                self.yeq
            };
        }

        if self.gen_model == 7 {
            self.phase_current_limit =
                Complex64::new(self.p_nominal_per_phase, -self.q_nominal_per_phase) / self.v_base95;
            self.model7_max_phase_curr = self.phase_current_limit.norm();
        }

        // If the generator state changes, force a Y rebuild.
        if self.gen_on != gen_on_saved {
            self.cd.yprim_invalid = true;
        }
    }

    /// Pascal `TGeneratorObj.RecalcElementData`.
    pub fn recalc(&mut self, sys: &SysCtx) {
        let nphases = self.cd.nphases as f64;
        self.v_base95 = self.vminpu * self.v_base;
        self.v_base105 = self.vmaxpu * self.v_base;

        self.var_base = 1000.0 * self.kvar_base / nphases;
        self.var_min = 1000.0 * self.kvar_min / nphases;
        self.var_max = 1000.0 * self.kvar_max / nphases;

        self.xd = self.pu_xd * 1000.0 * self.kv_generator_base.powi(2) / self.kva_rating;
        self.xdp = self.pu_xdp * 1000.0 * self.kv_generator_base.powi(2) / self.kva_rating;
        self.xdpp = self.pu_xdpp * 1000.0 * self.kv_generator_base.powi(2) / self.kva_rating;

        self.set_nominal_generation(sys);

        self.yq_fixed = -self.var_base / self.v_base.powi(2);
        self.v_target = self.vpu * 1000.0 * self.kv_generator_base;
        if self.cd.nphases > 1 {
            self.v_target /= sqrt3();
        }

        self.dqdv = self.dqdv_saved; // for Model 3
        self.delta_q_max = (self.var_max - self.var_min) * 0.10; // limit to 10% of range
    }

    /// Pascal `CalcYPrimMatrix` (power-flow path).
    fn calc_yprim_matrix(&mut self, ymatrix: &mut CMatrix, sys: &SysCtx) {
        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        // Regular power-flow generator model: Yeq is L-N; negate for generation.
        let mut y = -self.yeq;
        if self.gen_model == 3 {
            y /= 100.0; // Type-3: only put 1% in Yprim
        }
        y.im /= freq_multiplier;

        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        match self.connection {
            Connection::Wye => {
                let yij = -y;
                for i in 0..nphases {
                    ymatrix.set(i, i, y);
                    ymatrix.add(nconds - 1, nconds - 1, y);
                    ymatrix.set(i, nconds - 1, yij);
                    ymatrix.set(nconds - 1, i, yij);
                }
            }
            Connection::Delta => {
                let y = y / 3.0; // convert to delta impedance
                let yij = -y;
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 };
                    ymatrix.add(i, i, y);
                    ymatrix.add(j, j, y);
                    ymatrix.add_sym(i, j, yij);
                }
            }
        }
    }

    /// Pascal `StickCurrInTerminalArray` (reverse of the load: signs switched).
    fn stick_curr(&mut self, into_iterminal: bool, curr: Complex64, i: usize) {
        let nconds = self.cd.nconds;
        let arr = if into_iterminal {
            &mut self.cd.iterminal
        } else {
            &mut self.cd.inj_current
        };
        match self.connection {
            Connection::Wye => {
                arr[i] += curr;
                arr[nconds - 1] -= curr; // neutral
            }
            Connection::Delta => {
                arr[i] += curr;
                let j = if i + 1 >= nconds { 0 } else { i + 1 };
                arr[j] -= curr;
            }
        }
    }

    /// Pascal `CalcVTerminalPhase`.
    fn calc_vterminal_phase(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;
        match self.connection {
            Connection::Wye => {
                for i in 0..nphases {
                    self.cd.vterminal[i] =
                        node_v[self.cd.node_ref[i]] - node_v[self.cd.node_ref[nconds - 1]];
                }
            }
            Connection::Delta => {
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 };
                    self.cd.vterminal[i] =
                        node_v[self.cd.node_ref[i]] - node_v[self.cd.node_ref[j]];
                }
            }
        }
        self.gen_solution_count = sys.solution_count;
    }

    /// Pascal `CalcYPrimContribution`: `InjCurrent = Yprim · V(node)`.
    fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
        }
    }

    /// Pascal `DoConstantPQGen` (model 1).
    fn do_constant_pq_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.cd.zero_iterminal();
        self.calc_vterminal_phase(sys, node_v);

        let s = Complex64::new(self.p_nominal_per_phase, self.q_nominal_per_phase);
        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let mut curr = match self.connection {
                Connection::Wye => {
                    if vmag <= self.v_base95 {
                        self.yeq95 * v
                    } else if vmag > self.v_base105 {
                        self.yeq105 * v
                    } else {
                        (s / v).conj()
                    }
                }
                Connection::Delta => {
                    let vm = match self.cd.nphases {
                        2 | 3 => vmag / sqrt3(),
                        _ => vmag,
                    };
                    if vm <= self.v_base95 {
                        (self.yeq95 / 3.0) * v
                    } else if vm > self.v_base105 {
                        (self.yeq105 / 3.0) * v
                    } else {
                        (s / v).conj()
                    }
                }
            };
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoConstantZGen` (model 2).
    fn do_constant_z_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);
        self.cd.zero_iterminal();
        let yeq2 = match self.connection {
            Connection::Wye => self.yeq,
            Connection::Delta => self.yeq / 3.0,
        };
        for i in 0..self.cd.nphases {
            let mut curr = yeq2 * self.cd.vterminal[i];
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoPVTypeGen` (model 3: constant P, |V|).
    fn do_pv_type_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);
        self.cd.zero_iterminal();

        // Guess a new var output value.
        let mut v_avg = 0.0;
        for i in 0..self.cd.nphases {
            v_avg += self.cd.vterminal[i].norm();
        }
        let nphases = self.cd.nphases as f64;
        v_avg = match self.connection {
            Connection::Delta => v_avg / (sqrt3() * nphases),
            Connection::Wye => v_avg / nphases,
        };
        self.v_avg = v_avg;

        let mut dq = self.pv_factor * self.dqdv * (self.v_target - v_avg); // Vtarget is L-N
        if dq.abs() > self.delta_q_max {
            dq = if dq < 0.0 {
                -self.delta_q_max
            } else {
                self.delta_q_max
            };
        }
        self.q_nominal_per_phase += dq;
        if self.q_nominal_per_phase > self.var_max {
            self.q_nominal_per_phase = self.var_max;
        } else if self.q_nominal_per_phase < self.var_min {
            self.q_nominal_per_phase = self.var_min;
        }

        for i in 0..self.cd.nphases {
            let s = Complex64::new(self.p_nominal_per_phase, self.q_nominal_per_phase);
            let mut curr = (s / self.cd.vterminal[i]).conj();
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoFixedQGen` (model 4: constant P, fixed Q = kvarBase).
    fn do_fixed_q_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);
        self.cd.zero_iterminal();

        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let s = Complex64::new(self.p_nominal_per_phase, self.var_base);
            let mut curr = match self.connection {
                Connection::Wye => {
                    if vmag <= self.v_base95 {
                        Complex64::new(self.yeq95.re, self.yq_fixed) * v
                    } else if vmag > self.v_base105 {
                        Complex64::new(self.yeq105.re, self.yq_fixed) * v
                    } else {
                        (s / v).conj()
                    }
                }
                Connection::Delta => {
                    let vm = match self.cd.nphases {
                        2 | 3 => vmag / sqrt3(),
                        _ => vmag,
                    };
                    if vm <= self.v_base95 {
                        Complex64::new(self.yeq95.re / 3.0, self.yq_fixed / 3.0) * v
                    } else if vm > self.v_base105 {
                        Complex64::new(self.yeq105.re / 3.0, self.yq_fixed / 3.0) * v
                    } else {
                        (s / v).conj()
                    }
                }
            };
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoFixedQZGen` (model 5: constant P, fixed Q as a fixed Z).
    fn do_fixed_qz_gen(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);
        self.cd.zero_iterminal();

        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let vmag = v.norm();
            let p = Complex64::new(self.p_nominal_per_phase, 0.0);
            let mut curr = match self.connection {
                Connection::Wye => {
                    if vmag <= self.v_base95 {
                        Complex64::new(self.yeq95.re, self.yq_fixed) * v
                    } else if vmag > self.v_base105 {
                        Complex64::new(self.yeq105.re, self.yq_fixed) * v
                    } else {
                        (p / v).conj() + Complex64::new(0.0, self.yq_fixed) * v
                    }
                }
                Connection::Delta => {
                    let vm = match self.cd.nphases {
                        2 | 3 => vmag / sqrt3(),
                        _ => vmag,
                    };
                    if vm <= self.v_base95 {
                        Complex64::new(self.yeq95.re / 3.0, self.yq_fixed / 3.0) * v
                    } else if vm > self.v_base105 {
                        Complex64::new(self.yeq105.re / 3.0, self.yq_fixed / 3.0) * v
                    } else {
                        (p / v).conj() + Complex64::new(0.0, self.yq_fixed / 3.0) * v
                    }
                }
            };
            if self.use_fuel && !self.gen_active {
                curr = Complex64::ZERO;
            }
            self.put_curr(sys, curr, i);
        }
    }

    /// Pascal `DoCurrentLimitedPQ` (model 7: PQ limited to max current below
    /// Vminpu).
    fn do_current_limited_pq(&mut self, sys: &SysCtx, node_v: &[Complex64]) {
        self.calc_yprim_contribution(node_v);
        self.calc_vterminal_phase(sys, node_v);

        if self.force_balanced && self.cd.nphases == 3 {
            // Convert to pos-seq only.
            let sc = SymComp::default();
            let mut v012 = [Complex64::ZERO; 3];
            sc.phase_to_sym(&self.cd.vterminal[..3], &mut v012);
            v012[0] = Complex64::ZERO;
            v012[2] = Complex64::ZERO;
            let mut vph = [Complex64::ZERO; 3];
            sc.sym_to_phase(&v012, &mut vph);
            self.cd.vterminal[..3].copy_from_slice(&vph);
        }

        self.cd.zero_iterminal();
        let s = Complex64::new(self.p_nominal_per_phase, self.q_nominal_per_phase);
        for i in 0..self.cd.nphases {
            match self.connection {
                Connection::Wye => {
                    let vln = self.cd.vterminal[i];
                    let vmag_ln = vln.norm();
                    let mut phase_curr = (s / vln).conj();
                    if phase_curr.norm() > self.model7_max_phase_curr {
                        phase_curr = (self.phase_current_limit / (vln / vmag_ln)).conj();
                    }
                    // (Pascal omits the fuel check on the wye branch.)
                    self.put_curr(sys, phase_curr, i);
                }
                Connection::Delta => {
                    let vll = self.cd.vterminal[i];
                    let vmag_ll = vll.norm();
                    let mut delta_curr = match self.cd.nphases {
                        2 | 3 => {
                            let mut dc = (s / vll).conj();
                            if dc.norm() * sqrt3() > self.model7_max_phase_curr {
                                dc =
                                    (self.phase_current_limit / (vll / (vmag_ll / sqrt3()))).conj();
                            }
                            dc
                        }
                        _ => {
                            let mut dc = (s / vll).conj();
                            if dc.norm() > self.model7_max_phase_curr {
                                dc = (self.phase_current_limit / (vll / vmag_ll)).conj();
                            }
                            dc
                        }
                    };
                    if self.use_fuel && !self.gen_active {
                        delta_curr = Complex64::ZERO;
                    }
                    self.put_curr(sys, delta_curr, i);
                }
            }
        }
    }

    /// The shared tail of every `DoXxxGen`: terminal/injection bookkeeping.
    fn put_curr(&mut self, sys: &SysCtx, curr: Complex64, i: usize) {
        self.stick_curr(true, -curr, i); // into ITerminal
        self.cd.iterminal_updated = true;
        self.cd.iterminal_solution_count = sys.solution_count;
        self.stick_curr(false, curr, i); // into InjCurrent
    }

    /// Pascal `CalcGenModelContribution`.
    fn calc_gen_model_contribution(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        self.cd.iterminal_updated = false;
        // Dynamics/harmonics models are Phase 7.
        match self.gen_model {
            1 => self.do_constant_pq_gen(sys, node_v),
            2 => self.do_constant_z_gen(sys, node_v),
            3 => self.do_pv_type_gen(sys, node_v),
            4 => self.do_fixed_q_gen(sys, node_v),
            5 => self.do_fixed_qz_gen(sys, node_v),
            6 => {
                // User-written model DLL — never ported. Pascal inits InjCurrent
                // then records error 567.
                self.calc_yprim_contribution(node_v);
                errors.push(format!(
                    "{}.{} model designated to use user-written model, but user-written \
                     model is not defined.",
                    "Generator",
                    self.cd.obj.name()
                ));
            }
            7 => self.do_current_limited_pq(sys, node_v),
            _ => self.do_constant_pq_gen(sys, node_v),
        }
    }

    /// Pascal `CalcInjCurrentArray`.
    fn calc_inj_current_array(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        if self.gen_switch_open {
            self.cd.inj_current.fill(Complex64::ZERO);
        } else {
            self.calc_gen_model_contribution(sys, node_v, errors);
        }
    }

    // --- Registers (energy meter) -----------------------------------------

    /// Pascal `ResetRegisters`.
    pub fn reset_registers(&mut self) {
        self.registers = [0.0; NUM_GEN_REGISTERS];
        self.derivatives = [0.0; NUM_GEN_REGISTERS];
        self.first_sample_after_reset = true;
    }

    /// Pascal `Integrate`.
    fn integrate(&mut self, reg: usize, deriv: f64, interval: f64, trapezoidal: bool) {
        if trapezoidal {
            if !self.first_sample_after_reset {
                self.registers[reg] += 0.5 * interval * (deriv + self.derivatives[reg]);
            }
        } else {
            self.registers[reg] += interval * deriv;
        }
        self.derivatives[reg] = deriv;
    }

    /// Pascal `SetDragHandRegister`.
    fn set_drag_hand_register(&mut self, reg: usize, value: f64) {
        if value > self.registers[reg] {
            self.registers[reg] = value;
        }
    }

    /// Pascal `Get_PresentkW`.
    pub fn present_kw(&self) -> f64 {
        self.p_nominal_per_phase * 0.001 * self.cd.nphases as f64
    }

    /// Pascal `Get_Presentkvar`.
    pub fn present_kvar(&self) -> f64 {
        self.q_nominal_per_phase * 0.001 * self.cd.nphases as f64
    }

    /// Pascal `CheckOnFuel`.
    fn check_on_fuel(&mut self, deriv: f64, interval: f64) -> bool {
        self.pct_fuel = ((((self.pct_fuel / 100.0) * self.fuel_kwh) - interval * deriv)
            / self.fuel_kwh)
            * 100.0;
        if self.pct_fuel <= self.pct_reserve {
            self.pct_fuel = self.pct_reserve;
            return false;
        }
        true
    }

    /// Pascal `TakeSample`: accumulate the generator's energy registers.
    pub fn take_sample(
        &mut self,
        interval_hrs: f64,
        trapezoidal: bool,
        positive_sequence: bool,
        price_signal: f64,
    ) {
        if !self.cd.enabled {
            return;
        }
        let (mut s, mut smag, hour_value) = if self.gen_on {
            let s = Complex64::new(self.present_kw(), self.present_kvar());
            (s, s.norm(), 1.0)
        } else {
            (Complex64::ZERO, 0.0, 0.0)
        };

        if self.gen_on || trapezoidal {
            if positive_sequence {
                s *= 3.0;
                smag *= 3.0;
            }
            self.integrate(REG_KWH, s.re, interval_hrs, trapezoidal);
            self.integrate(REG_KVARH, s.im, interval_hrs, trapezoidal);
            self.set_drag_hand_register(REG_MAXKW, s.re.abs());
            self.set_drag_hand_register(REG_MAXKVA, smag);
            self.integrate(REG_HOURS, hour_value, interval_hrs, trapezoidal);
            self.integrate(
                REG_PRICE,
                s.re * price_signal * 0.001,
                interval_hrs,
                trapezoidal,
            );
            self.first_sample_after_reset = false;
            if self.use_fuel {
                self.gen_active = self.check_on_fuel(s.re, interval_hrs);
            }
        }
    }

    // --- Model-3 DQDV machinery (driven by the solution object) -----------

    /// Pascal `InitDQDVCalc`.
    pub fn init_dqdv_calc(&mut self) {
        self.dqdv = 0.0;
        self.q_nominal_per_phase = 0.5 * (self.var_max + self.var_min);
    }

    /// Pascal `CalcDQDV` — `yii` is the system Y diagonal at the generator's
    /// first node (the solution object reads it from the assembled matrix).
    pub fn calc_dqdv(&mut self, yii_abs: f64) {
        self.dqdv = 2.0 * yii_abs * self.v_base * self.vpu;
        self.dqdv_saved = self.dqdv;
    }

    /// Pascal `ResetStartPoint`.
    pub fn reset_start_point(&mut self) {
        self.q_nominal_per_phase = 1000.0 * self.kvar_base / self.cd.nphases as f64;
    }

    /// The generator's first conductor's global node reference (the DQDV sweep
    /// reads the Y diagonal here).
    pub fn first_node_ref(&self) -> usize {
        self.cd.node_ref.first().copied().unwrap_or(0)
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

impl CktElement for Generator {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys);
    }

    /// Pascal `TGeneratorObj.CalcYPrim`.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let mut yp_shunt = CMatrix::new(yorder);

        self.set_nominal_generation(sys);
        self.calc_yprim_matrix(&mut yp_shunt, sys);

        let mut yp_series = CMatrix::new(yorder);
        for i in 0..yorder {
            yp_series.set(i, i, yp_shunt.get(i, i) * 1.0e-10);
        }

        let mut yprim = CMatrix::new(yorder);
        yprim.copy_from(&yp_shunt);
        self.cd.yprim_shunt = Some(yp_shunt);
        self.cd.yprim_series = Some(yp_series);
        self.cd.yprim = Some(yprim);
        self.cd.inj_current = vec![Complex64::ZERO; yorder];

        self.cd.apply_yprim_open_conductor_calcs();
    }

    /// Pascal `TGeneratorObj.InjCurrents` + `TPCElement.InjCurrents`.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        if !self.cd.enabled {
            return;
        }
        if sys.loads_need_updating {
            self.set_nominal_generation(sys);
        }
        let mut errors = Vec::new();
        self.calc_inj_current_array(sys, ctx.node_v, &mut errors);
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
    }

    /// Pascal `TGeneratorObj.GetTerminalCurrents` + `TPCElement` base.
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if !self.cd.enabled {
            curr.fill(Complex64::ZERO);
            return;
        }
        if self.cd.iterminal_solution_count != sys.solution_count && !self.gen_switch_open {
            let mut errors = Vec::new();
            self.calc_gen_model_contribution(sys, node_v, &mut errors);
        }
        if self.cd.iterminal_updated {
            curr.copy_from_slice(&self.cd.iterminal[..curr.len()]);
        } else {
            let cd = &mut self.cd;
            if let Some(yprim) = &cd.yprim {
                yprim.mv_mult(curr, &cd.vterminal);
            }
            for (i, c) in curr.iter_mut().enumerate() {
                *c -= cd.inj_current[i];
            }
            cd.iterminal_updated = true;
        }
        self.cd.iterminal_solution_count = sys.solution_count;
    }
}

impl DssObject for Generator {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
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
            KV => self.kv_generator_base,
            KW => self.kw_base,
            PF => self.pf_nominal,
            KVAR => self.kvar_base,
            VMINPU => self.vminpu,
            VMAXPU => self.vmaxpu,
            DISPVALUE => self.dispatch_value,
            VPU => self.vpu,
            MAXKVAR => self.kvar_max,
            MINKVAR => self.kvar_min,
            PVFACTOR => self.pv_factor,
            KVA | MVA => self.kva_rating,
            XD => self.pu_xd,
            XDP => self.pu_xdp,
            XDPP => self.pu_xdpp,
            H => self.h_mass,
            D => self.dpu,
            DUTYSTART => self.duty_start,
            XRDP => self.xrdp,
            FUELKWH => self.fuel_kwh,
            PCTFUEL => self.pct_fuel,
            PCTRESERVE => self.pct_reserve,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Generator has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kv_generator_base = value,
            KW => self.kw_base = value,
            PF => self.pf_nominal = value,
            KVAR => self.kvar_base = value,
            VMINPU => self.vminpu = value,
            VMAXPU => self.vmaxpu = value,
            DISPVALUE => self.dispatch_value = value,
            VPU => self.vpu = value,
            MAXKVAR => self.kvar_max = value,
            MINKVAR => self.kvar_min = value,
            PVFACTOR => self.pv_factor = value,
            KVA | MVA => self.kva_rating = value,
            XD => self.pu_xd = value,
            XDP => self.pu_xdp = value,
            XDPP => self.pu_xdpp = value,
            H => self.h_mass = value,
            D => self.dpu = value,
            DUTYSTART => self.duty_start = value,
            XRDP => self.xrdp = value,
            FUELKWH => self.fuel_kwh = value,
            PCTFUEL => self.pct_fuel = value,
            PCTRESERVE => self.pct_reserve = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Generator has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            MODEL => self.gen_model,
            DISPMODE => self.dispatch_mode,
            CONN => self.connection as i32,
            STATUS => self.is_fixed as i32,
            CLS => self.gen_class,
            _ => unreachable!("Generator has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            MODEL => self.gen_model = value,
            DISPMODE => self.dispatch_mode = value,
            CONN => {
                self.connection = if value == 1 {
                    Connection::Delta
                } else {
                    Connection::Wye
                }
            }
            STATUS => self.is_fixed = value != 0,
            CLS => self.gen_class = value,
            _ => unreachable!("Generator has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        use prop::*;
        match idx {
            FORCEON => self.forced_on,
            DEBUGTRACE => self.debug_trace,
            BALANCED => self.force_balanced,
            USEFUEL => self.use_fuel,
            REFUEL => false, // Pascal BooleanActionProperty getter: always 0
            ENABLED => self.cd.enabled,
            _ => unreachable!("Generator has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        use prop::*;
        match idx {
            FORCEON => self.forced_on = value,
            DEBUGTRACE => self.debug_trace = value,
            BALANCED => self.force_balanced = value,
            USEFUEL => self.use_fuel = value,
            REFUEL => {
                // Pascal `DoRefuel`: fires on TRUE only.
                if value {
                    self.pct_fuel = 100.0;
                    self.gen_active = true;
                }
            }
            ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Generator has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            USERMODEL => self.user_model_name.clone(),
            USERDATA => self.user_data.clone(),
            SHAFTMODEL => self.shaft_model_name.clone(),
            SHAFTDATA => self.shaft_data.clone(),
            DYNAMICEQ => self.dynamic_eq.clone(),
            DYNOUT => self.dyn_out.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("Generator has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            // The NOT_PORTED string props error in the parser before reaching
            // here; the setters exist for completeness/MakeLike.
            USERMODEL => self.user_model_name = value,
            USERDATA => self.user_data = value,
            SHAFTMODEL => self.shaft_model_name = value,
            SHAFTDATA => self.shaft_data = value,
            DYNAMICEQ => self.dynamic_eq = value,
            DYNOUT => self.dyn_out = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("Generator has no string property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Resolve a shape reference (snapshot-clone like the Load shape refs).
    fn set_object_ref(
        &mut self,
        idx: usize,
        name: String,
        resolved: Option<(ElemRef, &dyn DssObject)>,
    ) {
        use prop::*;
        let elem_ref = resolved.map(|(r, _)| r);
        let load_shape =
            || resolved.and_then(|(_, o)| o.as_any().downcast_ref::<LoadShapeObj>().cloned());
        match idx {
            YEARLY => {
                self.yearly_shape = name;
                self.yearly_shape_ref = elem_ref;
                self.yearly_shape_obj = load_shape();
            }
            DAILY => {
                self.daily_shape = name;
                self.daily_shape_ref = elem_ref;
                self.daily_shape_obj = load_shape();
            }
            DUTY => {
                self.duty_shape = name;
                self.duty_shape_ref = elem_ref;
                self.duty_shape_obj = load_shape();
            }
            _ => unreachable!("Generator has no resolved object-ref property {idx}"),
        }
    }

    /// Pascal `TGeneratorObj.PropertySideEffects` (text-parser path).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            CONN => {
                let n = nconds_for_connection(self.connection, self.cd.nphases);
                self.cd.set_nconds(n);
                self.update_vbase();
                self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
            }
            KV => self.update_vbase(),
            KVAR => self.side_effect_kvar(),
            PHASES => {
                let n = nconds_for_connection(self.connection, self.cd.nphases);
                self.cd.set_nconds(n); // force reallocation of terminal info
            }
            KW | PF => {
                self.sync_up_power_quantities();
                if idx == PF {
                    self.cd.obj.clear_seq(KVAR);
                }
            }
            MODEL => {
                if self.gen_model == 3 {
                    self.cd.signal_reset_solution_initialized = true;
                }
            }
            YEARLY => {
                let actual = self
                    .yearly_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| (s.max_p(), s.max_q()));
                if let Some((mp, mq)) = actual {
                    self.set_kw_kvar(mp, mq);
                }
            }
            DAILY => {
                let actual = self
                    .daily_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| (s.max_p(), s.max_q()));
                if let Some((mp, mq)) = actual {
                    self.set_kw_kvar(mp, mq);
                }
            }
            DUTY => {
                let actual = self
                    .duty_shape_obj
                    .as_ref()
                    .filter(|s| s.use_actual())
                    .map(|s| (s.max_p(), s.max_q()));
                if let Some((mp, mq)) = actual {
                    self.set_kw_kvar(mp, mq);
                }
            }
            KVA | MVA => self.kva_not_set = false,
            _ => {}
        }
    }

    /// Pascal `TGenerator.EndEdit`: `RecalcElementData` + Yprim invalidation.
    fn end_edit(&mut self) {
        self.recalc(&default_recalc_ctx());
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TGeneratorObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Generator>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            self.cd.set_nconds(self.cd.nphases); // Pascal: NConds := Fnphases
            self.cd.yprim_invalid = true;
        }
        self.v_base = other.v_base;
        self.vminpu = other.vminpu;
        self.vmaxpu = other.vmaxpu;
        self.v_base95 = other.v_base95;
        self.v_base105 = other.v_base105;
        self.kw_base = other.kw_base;
        self.kvar_base = other.kvar_base;
        // GenVars (the machine record) — copied wholesale by Pascal.
        self.kv_generator_base = other.kv_generator_base;
        self.kva_rating = other.kva_rating;
        self.pu_xd = other.pu_xd;
        self.pu_xdp = other.pu_xdp;
        self.pu_xdpp = other.pu_xdpp;
        self.xd = other.xd;
        self.xdp = other.xdp;
        self.xdpp = other.xdpp;
        self.h_mass = other.h_mass;
        self.dpu = other.dpu;
        self.xrdp = other.xrdp;
        self.v_target = other.v_target;
        self.p_nominal_per_phase = other.p_nominal_per_phase;
        self.q_nominal_per_phase = other.q_nominal_per_phase;
        self.pf_nominal = other.pf_nominal;
        self.var_min = other.var_min;
        self.var_max = other.var_max;
        self.connection = other.connection;
        self.yearly_shape = other.yearly_shape.clone();
        self.daily_shape = other.daily_shape.clone();
        self.duty_shape = other.duty_shape.clone();
        self.yearly_shape_obj = other.yearly_shape_obj.clone();
        self.daily_shape_obj = other.daily_shape_obj.clone();
        self.duty_shape_obj = other.duty_shape_obj.clone();
        self.yearly_shape_ref = other.yearly_shape_ref;
        self.daily_shape_ref = other.daily_shape_ref;
        self.duty_shape_ref = other.duty_shape_ref;
        self.duty_start = other.duty_start;
        self.dispatch_mode = other.dispatch_mode;
        self.dispatch_value = other.dispatch_value;
        self.gen_class = other.gen_class;
        self.gen_model = other.gen_model;
        self.is_fixed = other.is_fixed;
        self.vpu = other.vpu;
        self.kvar_max = other.kvar_max;
        self.kvar_min = other.kvar_min;
        self.forced_on = other.forced_on;
        self.kva_not_set = other.kva_not_set;
        self.use_fuel = other.use_fuel;
        self.fuel_kwh = other.fuel_kwh;
        self.pct_fuel = other.pct_fuel;
        self.pct_reserve = other.pct_reserve;
        self.user_model_name = other.user_model_name.clone();
        self.shaft_model_name = other.shaft_model_name.clone();
        self.spectrum = other.spectrum.clone();
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap_ctx() -> SysCtx {
        default_recalc_ctx()
    }

    /// A default generator's nominal quantities (oracle dss-python 0.15.7):
    /// kW=1000, kvar=60, 3-phase, Vbase=7200 L-N.
    #[test]
    fn default_nominal_generation() {
        let mut g = Generator::new("g1");
        g.set_nominal_generation(&snap_ctx());
        // Pnom = 1000*1000*1*1/3, Qnom = 1000*60*1*1/3
        assert!((g.p_nominal_per_phase - 333333.3333).abs() < 1e-3);
        assert!((g.q_nominal_per_phase - 20000.0).abs() < 1e-6);
        // Yeq = (Pnom - jQnom)/Vbase^2
        let yeq = Complex64::new(333333.3333, -20000.0) / 7200.0_f64.powi(2);
        assert!((g.yeq.re - yeq.re).abs() < 1e-9);
        assert!((g.yeq.im - yeq.im).abs() < 1e-9);
    }

    /// kW/PF web: setting kW=100, PF=0.95 derives kvar via SyncUpPowerQuantities.
    #[test]
    fn kw_pf_sets_kvar() {
        let mut g = Generator::new("g1");
        g.kw_base = 100.0;
        g.pf_nominal = 0.95;
        g.sync_up_power_quantities();
        // kvar = kW*sqrt(1/pf^2 - 1) = 100*0.328684
        assert!((g.kvar_base - 32.8684).abs() < 1e-3, "kvar {}", g.kvar_base);
        // kVA not set → tracks kW*1.2
        assert!((g.kva_rating - 120.0).abs() < 1e-9);
    }

    /// kva explicitly set freezes kVArating (kVANotSet=false).
    #[test]
    fn kva_set_freezes_rating() {
        let mut g = Generator::new("g1");
        g.kva_rating = 500.0;
        g.kva_not_set = false;
        g.kw_base = 100.0;
        g.pf_nominal = 0.95;
        g.sync_up_power_quantities();
        assert!((g.kva_rating - 500.0).abs() < 1e-9);
    }

    /// Off-state (LOADMODE dispatch below the reference) → tiny resistive load.
    #[test]
    fn off_state_is_tiny_resistive_load() {
        let mut g = Generator::new("g1");
        g.dispatch_mode = LOADMODE;
        g.dispatch_value = 2.0;
        let mut sys = snap_ctx();
        sys.generator_dispatch_reference = 1.0; // below dispatch_value → OFF
        g.set_nominal_generation(&sys);
        assert!(!g.gen_on);
        assert!((g.p_nominal_per_phase - (-0.1 * 1000.0 / 3.0)).abs() < 1e-9);
        assert_eq!(g.q_nominal_per_phase, 0.0);
    }

    /// A delta generator carries `nphases` conductors (3-phase).
    #[test]
    fn delta_connection_nconds() {
        let mut g = Generator::new("g1");
        g.set_i32(prop::CONN, 1); // delta
        g.side_effects(prop::CONN, 0);
        assert_eq!(g.cd.nconds, 3);
    }

    /// Status=Fixed forces factor=1 regardless of gen_multiplier.
    #[test]
    fn fixed_status_ignores_gen_multiplier() {
        let mut g = Generator::new("g1");
        g.is_fixed = true;
        let mut sys = snap_ctx();
        sys.gen_multiplier = 0.5;
        g.set_nominal_generation(&sys);
        // factor=1 → Pnom = 1000*1000/3 (gen_multiplier ignored)
        assert!((g.p_nominal_per_phase - 333333.3333).abs() < 1e-3);
    }

    /// TakeSample integrates kWh/kvarh over an interval (plain Euler).
    #[test]
    fn take_sample_accumulates_energy() {
        let mut g = Generator::new("g1");
        g.set_nominal_generation(&snap_ctx());
        g.gen_on = true;
        g.reset_registers();
        // present kW = Pnom*0.001*3 = 1000; present kvar = 60
        g.take_sample(1.0, false, false, 25.0);
        assert!((g.registers[REG_KWH] - 1000.0).abs() < 1e-6);
        assert!((g.registers[REG_KVARH] - 60.0).abs() < 1e-6);
        assert!((g.registers[REG_MAXKW] - 1000.0).abs() < 1e-6);
        assert!((g.registers[REG_HOURS] - 1.0).abs() < 1e-9);
    }
}
