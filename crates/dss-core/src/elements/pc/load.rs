//! Port of `PCElements/Load.pas` — `TLoadObj` with all 8 load models.
//!
//! The load is balanced over its phases: `WNominal`/`varNominal` are W/var
//! **per phase** (`/ Fnphases`). A wye load carries `nphases + 1` conductors
//! (the neutral); delta loads connect phase-to-phase. Injections use the
//! compensation form: `InjCurrent = Yprim·V` (from `CalcYPrimContribution`)
//! plus the model current distributed by `StickCurrInTerminalArray`.

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{CktElement, InjCtx, SysCtx};
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::solution::SolveMode;
use crate::support::cmatrix::CMatrix;
use crate::util::{CDOUBLEONE, inv_sqrt3_x1000};

/// Pascal `TLoadModel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadModel {
    ConstPQ = 1,
    ConstZ = 2,
    Motor = 3,
    Cvr = 4,
    ConstI = 5,
    ConstPFixedQ = 6,
    ConstPFixedX = 7,
    Zipv = 8,
}

impl LoadModel {
    pub fn from_i32(v: i32) -> LoadModel {
        match v {
            2 => LoadModel::ConstZ,
            3 => LoadModel::Motor,
            4 => LoadModel::Cvr,
            5 => LoadModel::ConstI,
            6 => LoadModel::ConstPFixedQ,
            7 => LoadModel::ConstPFixedX,
            8 => LoadModel::Zipv,
            _ => LoadModel::ConstPQ,
        }
    }
}

/// Pascal `TLoadConnection` (TGeneralConnection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Wye = 0,
    Delta = 1,
}

/// Pascal `TLoadSpec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadSpec {
    KwPf = 0,
    KwKvar = 1,
    KvaPf = 2,
    ConnectedKvaPf = 3,
    KwhPf = 4,
}

/// 1-based property ordinals (Pascal `TLoadProp` + class tails).
pub mod prop {
    pub const PHASES: usize = 1;
    pub const BUS1: usize = 2;
    pub const KV: usize = 3;
    pub const KW: usize = 4;
    pub const PF: usize = 5;
    pub const MODEL: usize = 6;
    pub const YEARLY: usize = 7;
    pub const DAILY: usize = 8;
    pub const DUTY: usize = 9;
    pub const GROWTH: usize = 10;
    pub const CONN: usize = 11;
    pub const KVAR: usize = 12;
    pub const RNEUT: usize = 13;
    pub const XNEUT: usize = 14;
    pub const STATUS: usize = 15;
    pub const CLS: usize = 16;
    pub const VMINPU: usize = 17;
    pub const VMAXPU: usize = 18;
    pub const VMINNORM: usize = 19;
    pub const VMINEMERG: usize = 20;
    pub const XFKVA: usize = 21;
    pub const ALLOCATIONFACTOR: usize = 22;
    pub const KVA: usize = 23;
    pub const PCTMEAN: usize = 24;
    pub const PCTSTDDEV: usize = 25;
    pub const CVRWATTS: usize = 26;
    pub const CVRVARS: usize = 27;
    pub const KWH: usize = 28;
    pub const KWHDAYS: usize = 29;
    pub const CFACTOR: usize = 30;
    pub const CVRCURVE: usize = 31;
    pub const NUMCUST: usize = 32;
    pub const ZIPV: usize = 33;
    pub const PCT_SERIES_RL: usize = 34;
    pub const REL_WEIGHT: usize = 35;
    pub const VLOWPU: usize = 36;
    pub const PUXHARM: usize = 37;
    pub const XRHARM: usize = 38;
    // tails:
    pub const SPECTRUM: usize = 39;
    pub const BASE_FREQ: usize = 40;
    pub const ENABLED: usize = 41;
    pub const NUM_PROPS: usize = 42; // incl. Like
}

/// `TLoad.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("bus1", 1),
        PropDef::double("kV").flags(PropFlags::NON_NEGATIVE),
        PropDef::double("kW"),
        PropDef::double("pf"),
        PropDef::mapped_int_enum("model", enums.load_model),
        PropDef::object_ref("yearly"),
        PropDef::object_ref("daily"),
        PropDef::object_ref("duty"),
        PropDef::object_ref("growth"),
        PropDef::mapped_string_enum("conn", enums.connection),
        PropDef::double("kvar"),
        PropDef::double("Rneut"),
        PropDef::double("Xneut"),
        PropDef::mapped_string_enum("status", enums.load_status),
        PropDef::integer("class"),
        PropDef::double("Vminpu"),
        PropDef::double("Vmaxpu"),
        PropDef::double("Vminnorm"),
        PropDef::double("Vminemerg"),
        PropDef::double("xfkVA"),
        PropDef::double("allocationfactor"),
        PropDef::double("kVA"),
        PropDef::double("%mean").scale(0.01),
        PropDef::double("%stddev").scale(0.01),
        PropDef::double("CVRwatts"),
        PropDef::double("CVRvars"),
        PropDef::double("kwh"),
        PropDef::double("kwhdays"),
        PropDef::double("Cfactor"),
        PropDef::object_ref("CVRcurve"),
        PropDef::integer("NumCust"),
        PropDef::double_f_array("ZIPV", 7),
        PropDef::double("%SeriesRL").scale(0.01),
        PropDef::double("RelWeight"),
        PropDef::double("Vlowpu"),
        PropDef::double("puXharm"),
        PropDef::double("XRharm"),
        // PCClass tail:
        PropDef::object_ref("spectrum"),
        // CktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("Load", defs, true)
}

/// `TLoadObj`.
#[derive(Debug, Clone)]
pub struct Load {
    pub cd: CktElementData,

    pub connection: Connection,
    pub load_model: LoadModel,
    pub status: i32, // 0=Variable, 1=Fixed, 2=Exempt
    pub kw_base: f64,
    pub kvar_base: f64,
    pub kva_base: f64,
    pub kw_ref: f64,
    pub kvar_ref: f64,
    pub pf_nominal: f64,
    pub kv_load_base: f64,
    pub load_spec_type: LoadSpec,
    pub rneut: f64,
    pub xneut: f64,
    pub load_class: i32,
    pub num_customers: i32,
    pub vminpu: f64,
    pub vmaxpu: f64,
    pub vlowpu: f64,
    pub vmin_normal: f64,
    pub vmin_emerg: f64,
    pub connected_kva: f64,
    pub kwh: f64,
    pub kwh_days: f64,
    pub c_factor: f64,
    pub pu_mean: f64,
    pub pu_std_dev: f64,
    pub cvr_watt_factor: f64,
    pub cvr_var_factor: f64,
    pub zipv: [f64; 7],
    pub zipv_set: bool,
    pub pu_series_rl: f64,
    pub rel_weighting: f64,
    pub pu_x_harm: f64,
    pub xr_harm_ratio: f64,
    pub allocation_factor: f64,
    pub kva_allocation_factor: f64,
    pub has_been_allocated: bool,
    pub pf_specified: bool,
    pub pf_changed: bool,
    pub shape_is_actual: bool,
    pub random_mult: f64,
    pub last_growth_factor: f64,
    pub last_year: i32,

    // Derived (RecalcElementData / SetNominalLoad):
    pub w_nominal: f64,
    pub var_nominal: f64,
    pub var_base: f64,
    pub v_base: f64,
    pub v_base_low: f64,
    pub v_base95: f64,
    pub v_base105: f64,
    pub yeq: Complex64,
    pub yeq95: Complex64,
    pub yeq105: Complex64,
    pub yeq105i: Complex64,
    pub y_neut: Complex64,
    pub yq_fixed: f64,
    pub i_low: Complex64,
    pub i95: Complex64,
    pub i_base: Complex64,
    pub m95: Complex64,
    pub m95i: Complex64,
    pub phase_curr: Vec<Complex64>,
    pub load_solution_count: i32,
    pub open_load_solution_count: i32,
    pub yprim_open_cond: Option<CMatrix>,

    pub yearly_shape: String,
    pub daily_shape: String,
    pub duty_shape: String,
    pub growth_shape: String,
    pub cvr_shape: String,
    pub spectrum: String,
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

impl Load {
    /// Pascal `TLoadObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 4; // defaults to wye so it has a 4th conductor
        cd.set_nterms(1);

        let kw_base = 10.0;
        let pf_nominal = 0.88;
        let v_base = 7200.0;
        let mut load = Self {
            cd,
            connection: Connection::Wye,
            load_model: LoadModel::ConstPQ,
            status: 0,
            kw_base,
            kvar_base: 5.0,
            kva_base: kw_base / pf_nominal,
            kw_ref: 0.0,
            kvar_ref: 0.0,
            pf_nominal,
            kv_load_base: 12.47,
            load_spec_type: LoadSpec::KwPf,
            rneut: -1.0, // signify neutral is open
            xneut: 0.0,
            load_class: 1,
            num_customers: 1,
            vminpu: 0.95,
            vmaxpu: 1.05,
            vlowpu: 0.50,
            vmin_normal: 0.0,
            vmin_emerg: 0.0,
            connected_kva: 0.0,
            kwh: 0.0,
            kwh_days: 30.0,
            c_factor: 4.0,
            pu_mean: 0.5,
            pu_std_dev: 0.1,
            cvr_watt_factor: 1.0,
            cvr_var_factor: 2.0,
            zipv: [0.0; 7],
            zipv_set: false,
            pu_series_rl: 0.50,
            rel_weighting: 1.0,
            pu_x_harm: 0.0,
            xr_harm_ratio: 6.0,
            allocation_factor: 0.5,
            kva_allocation_factor: 0.5,
            has_been_allocated: false,
            pf_specified: false,
            pf_changed: false,
            shape_is_actual: false,
            random_mult: 1.0,
            last_growth_factor: 1.0,
            last_year: 0,
            w_nominal: 0.0,
            var_nominal: 0.0,
            var_base: 0.0,
            v_base,
            v_base_low: 0.50 * v_base,
            v_base95: 0.95 * v_base,
            v_base105: 1.05 * v_base,
            yeq: Complex64::ZERO,
            yeq95: Complex64::ZERO,
            yeq105: Complex64::ZERO,
            yeq105i: Complex64::ZERO,
            y_neut: Complex64::ZERO,
            yq_fixed: 0.0,
            i_low: Complex64::ZERO,
            i95: Complex64::ZERO,
            i_base: Complex64::ZERO,
            m95: Complex64::ZERO,
            m95i: Complex64::ZERO,
            phase_curr: vec![Complex64::ZERO; 3],
            load_solution_count: -1,
            open_load_solution_count: -1,
            yprim_open_cond: None,
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            growth_shape: String::new(),
            cvr_shape: String::new(),
            spectrum: "defaultload".to_string(),
        };
        load.cd.inj_current = vec![Complex64::ZERO; load.cd.yorder];
        load.recalc(&default_recalc_ctx());
        load
    }

    /// Pascal `GrowthFactor` (no growth shapes yet: year 0 → 1.0, else the
    /// circuit default growth factor).
    fn growth_factor(&mut self, year: i32, default_growth_factor: f64) -> f64 {
        if year == 0 {
            self.last_growth_factor = 1.0;
        } else if self.growth_shape.is_empty() {
            self.last_growth_factor = default_growth_factor;
        }
        self.last_growth_factor
    }

    /// Pascal `SetNominalLoad`.
    pub fn set_nominal_load(&mut self, sys: &SysCtx) {
        let mut shape_factor = CDOUBLEONE;
        self.shape_is_actual = false;

        let factor = if self.status == 1 {
            // Fixed: consider only the growth factor.
            self.growth_factor(sys.year, sys.default_growth_factor)
        } else {
            match sys.mode {
                SolveMode::Snapshot | SolveMode::Harmonic => {
                    if self.status == 2 {
                        // Exempt
                        self.growth_factor(sys.year, sys.default_growth_factor)
                    } else {
                        sys.load_multiplier
                            * self.growth_factor(sys.year, sys.default_growth_factor)
                    }
                }
                // Loadshape-driven modes arrive in Phase 5; until then every
                // other mode uses the default branch (growth only).
                _ => {
                    shape_factor = CDOUBLEONE;
                    self.growth_factor(sys.year, sys.default_growth_factor)
                }
            }
        };

        let nphases = self.cd.nphases as f64;
        if self.shape_is_actual {
            self.w_nominal = 1000.0 * shape_factor.re / nphases;
            self.var_nominal = 0.0;
            if shape_factor.im != 0.0 {
                self.var_nominal = 1000.0 * shape_factor.im / nphases;
            } else if self.pf_specified && self.pf_nominal != 1.0 {
                self.var_nominal = self.w_nominal * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                if self.pf_nominal < 0.0 {
                    self.var_nominal = -self.var_nominal;
                }
            }
        } else {
            self.w_nominal = 1000.0 * self.kw_base * factor * shape_factor.re / nphases;
            self.var_nominal = 1000.0 * self.kvar_base * factor * shape_factor.im / nphases;
        }

        self.yeq = Complex64::new(self.w_nominal, -self.var_nominal) / self.v_base.powi(2);
        self.yeq95 = if self.vminpu != 0.0 {
            self.yeq / self.vminpu.powi(2) // at 95% voltage
        } else {
            Complex64::ZERO
        };
        self.yeq105 = if self.vmaxpu != 0.0 {
            self.yeq / self.vmaxpu.powi(2) // at 105% voltage
        } else {
            self.yeq
        };
        self.yeq105i = if self.vmaxpu != 0.0 {
            self.yeq / self.vmaxpu // at 105% voltage for Constant I
        } else {
            self.yeq
        };

        // New code to help with convergence at low voltages.
        self.i_low = self.yeq * self.v_base_low;
        self.i95 = self.yeq95 * self.v_base95;
        self.m95 = (self.i95 - self.i_low) / (self.v_base95 - self.v_base_low);
        self.i_base = self.yeq * self.v_base;
        self.m95i = (self.i_base - self.i_low) / (self.v_base95 - self.v_base_low);
    }

    /// Pascal `TLoadObj.RecalcElementData`. `sys` provides the solve-state
    /// scalars `SetNominalLoad` reads; at parse time the executive passes the
    /// snapshot defaults (every value is recomputed again in `CalcYPrim`
    /// before it is consumed).
    pub fn recalc(&mut self, sys: &SysCtx) {
        self.v_base_low = self.vlowpu * self.v_base;
        self.v_base95 = self.vminpu * self.v_base;
        self.v_base105 = self.vmaxpu * self.v_base;

        // Set kW and kvar from root values of kVA and PF.
        match self.load_spec_type {
            LoadSpec::KwPf => {
                self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                if self.pf_nominal < 0.0 {
                    self.kvar_base = -self.kvar_base;
                }
                self.kva_base = (self.kw_base.powi(2) + self.kvar_base.powi(2)).sqrt();
            }
            LoadSpec::KwKvar => {
                self.kva_base = (self.kw_base.powi(2) + self.kvar_base.powi(2)).sqrt();
                if self.kva_base > 0.0 {
                    self.pf_nominal = self.kw_base / self.kva_base;
                    if self.kvar_base != 0.0 {
                        self.pf_nominal *= (self.kw_base * self.kvar_base).signum();
                    }
                }
            }
            LoadSpec::KvaPf => {
                self.kw_base = self.kva_base * self.pf_nominal.abs();
                self.kw_ref = self.kw_base;
                self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                self.kvar_ref = self.kvar_base;
                if self.pf_nominal < 0.0 {
                    self.kvar_base = -self.kvar_base;
                }
            }
            LoadSpec::ConnectedKvaPf | LoadSpec::KwhPf => {
                if self.pf_changed {
                    self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                    if self.pf_nominal < 0.0 {
                        self.kvar_base = -self.kvar_base;
                    }
                    self.kva_base = (self.kw_ref.powi(2) + self.kvar_ref.powi(2)).sqrt();
                }
            }
        }

        self.set_nominal_load(sys);

        self.y_neut = if self.rneut < 0.0 {
            Complex64::ZERO // flag for open neutral
        } else if self.rneut == 0.0 && self.xneut == 0.0 {
            Complex64::new(1.0e6, 0.0) // solidly grounded: 1 µΩ resistor
        } else {
            Complex64::new(self.rneut, self.xneut).inv()
        };

        self.var_base = 1000.0 * self.kvar_base / self.cd.nphases as f64;
        self.yq_fixed = -self.var_base / self.v_base.powi(2);

        self.pf_changed = false;
    }

    /// Pascal `CalcYPrimMatrix` (power-flow path; the harmonic series-RL
    /// split arrives in Phase 7).
    fn calc_yprim_matrix(&mut self, ymatrix: &mut CMatrix, sys: &SysCtx) {
        self.cd.yprim_freq = sys.frequency;
        let freq_multiplier = self.cd.yprim_freq / self.cd.base_frequency;

        let mut y = self.yeq;
        y.im /= freq_multiplier; // correct reactive part for frequency

        let yij = -y;
        let nphases = self.cd.nphases;
        let nconds = self.cd.nconds;

        match self.connection {
            Connection::Wye => {
                for i in 0..nphases {
                    ymatrix.set(i, i, y);
                    ymatrix.add(nconds - 1, nconds - 1, y);
                    ymatrix.set(i, nconds - 1, yij);
                    ymatrix.set(nconds - 1, i, yij);
                }
                ymatrix.add(nconds - 1, nconds - 1, self.y_neut); // neutral

                // If the neutral is floating, keep a small connection to
                // ground by increasing the last diagonal slightly.
                if self.rneut < 0.0 {
                    let v = ymatrix.get(nconds - 1, nconds - 1) * 1.000001;
                    ymatrix.set(nconds - 1, nconds - 1, v);
                }
            }
            Connection::Delta => {
                for i in 0..nphases {
                    let j = if i + 1 >= nconds { 0 } else { i + 1 };
                    ymatrix.add(i, i, y);
                    ymatrix.add(j, j, y);
                    ymatrix.add_sym(i, j, yij);
                }
            }
        }
    }

    /// Pascal `StickCurrInTerminalArray` — `kind` selects the target array
    /// since Rust can't alias `&mut` fields. `i` is the 0-based phase.
    fn stick_curr(&mut self, into_iterminal: bool, curr: Complex64, i: usize) {
        let nconds = self.cd.nconds;
        let arr = if into_iterminal {
            &mut self.cd.iterminal
        } else {
            &mut self.cd.inj_current
        };
        match self.connection {
            Connection::Wye => {
                arr[i] -= curr;
                arr[nconds - 1] += curr; // neutral
            }
            Connection::Delta => {
                arr[i] -= curr;
                let j = if i + 1 >= nconds { 0 } else { i + 1 };
                arr[j] += curr;
            }
        }
    }

    /// Pascal `CalcVTerminalPhase`: phase voltages via `VDiff`.
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
        self.load_solution_count = sys.solution_count;
    }

    /// Pascal `CalcYPrimContribution`: `InjCurrent = Yprim · V(node)`.
    fn calc_yprim_contribution(&mut self, node_v: &[Complex64]) {
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(yprim) = &cd.yprim {
            yprim.mv_mult(&mut cd.inj_current, &cd.vterminal);
        }
    }

    /// Pascal `InterpolateY95_YLow`.
    fn interpolate_y95_ylow(&self, vmag: f64) -> Complex64 {
        (self.i_low + self.m95 * (vmag - self.v_base_low)) / vmag
    }

    /// Pascal `InterpolateY95I_YLow`.
    fn interpolate_y95i_ylow(&self, vmag: f64) -> Complex64 {
        (self.i_low + self.m95i * (vmag - self.v_base_low)) / vmag
    }

    /// The per-phase current of each load model at voltage `v` — the bodies
    /// of `DoConstantPQLoad` .. `DoZIPVModel`, factored on the shared
    /// "below VBaseLow → linear Yeq" / interpolation-zone scaffolding.
    fn model_current(&self, v: Complex64, errors: &mut Vec<String>) -> Complex64 {
        let vmag = v.norm();
        match self.load_model {
            LoadModel::ConstPQ => {
                if vmag <= self.v_base_low {
                    self.yeq * v // below VbaseZ: linear, equal to Yprim contribution
                } else if vmag <= self.v_base95 {
                    self.interpolate_y95_ylow(vmag) * v
                } else if vmag > self.v_base105 {
                    self.yeq105 * v // above 105%: impedance model
                } else {
                    (Complex64::new(self.w_nominal, self.var_nominal) / v).conj()
                }
            }
            LoadModel::ConstZ => self.yeq * v,
            LoadModel::Motor => {
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    self.interpolate_y95_ylow(vmag) * v
                } else if vmag > self.v_base105 {
                    self.yeq105 * v
                } else {
                    // Constant P above 95%, plus Q as constant impedance.
                    let mut curr = (Complex64::new(self.w_nominal, 0.0) / v).conj();
                    curr += Complex64::new(0.0, self.yeq.im) * v;
                    curr
                }
            }
            LoadModel::Cvr => {
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    self.interpolate_y95_ylow(vmag) * v
                } else if vmag > self.v_base105 {
                    self.yeq105 * v
                } else {
                    let vratio = vmag / self.v_base; // L-N for wye, L-L for delta
                    let watt_factor = if self.cvr_watt_factor != 1.0 {
                        vratio.powf(self.cvr_watt_factor)
                    } else {
                        vratio
                    };
                    let curr = if watt_factor > 0.0 {
                        (Complex64::new(self.w_nominal * watt_factor, 0.0) / v).conj()
                    } else {
                        Complex64::ZERO
                    };
                    let cvar = if vmag == 0.0 {
                        Complex64::ZERO // trap divide by zero
                    } else if self.cvr_var_factor == 2.0 {
                        Complex64::new(0.0, self.yeq.im) * v // same as constant Z
                    } else if self.cvr_var_factor == 3.0 {
                        let var_factor = vratio * vratio * vratio;
                        (Complex64::new(0.0, self.var_nominal * var_factor) / v).conj()
                    } else {
                        let var_factor = vratio.powf(self.cvr_var_factor);
                        (Complex64::new(0.0, self.var_nominal * var_factor) / v).conj()
                    };
                    curr + cvar
                }
            }
            LoadModel::ConstI => {
                // Injection = [S / (Vbase · V/|V|)]*
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    self.interpolate_y95i_ylow(vmag) * v
                } else if vmag > self.v_base105 {
                    self.yeq105i * v
                } else {
                    (Complex64::new(self.w_nominal, self.var_nominal) / ((v / vmag) * self.v_base))
                        .conj()
                }
            }
            LoadModel::ConstPFixedQ => {
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    Complex64::new(self.yeq95.re, self.yq_fixed) * v
                } else if vmag > self.v_base105 {
                    Complex64::new(self.yeq105.re, self.yq_fixed) * v
                } else {
                    (Complex64::new(self.w_nominal, self.var_base) / v).conj()
                }
            }
            LoadModel::ConstPFixedX => {
                if vmag <= self.v_base_low {
                    self.yeq * v
                } else if vmag <= self.v_base95 {
                    Complex64::new(self.yeq95.re, self.yq_fixed) * v
                } else if vmag > self.v_base105 {
                    Complex64::new(self.yeq105.re, self.yq_fixed) * v
                } else {
                    let mut curr = (Complex64::new(self.w_nominal, 0.0) / v).conj();
                    curr += Complex64::new(0.0, self.yq_fixed) * v;
                    curr
                }
            }
            LoadModel::Zipv => {
                if !self.zipv_set {
                    errors.push("ZIPV is not set. Aborting...".to_string());
                    return Complex64::ZERO;
                }
                if vmag <= self.v_base_low {
                    return self.yeq * v;
                }
                let z = &self.zipv;
                let (mut curr_z, mut curr_i, mut curr_p) =
                    (Complex64::ZERO, Complex64::ZERO, Complex64::ZERO);
                let mut curr = if vmag <= self.v_base95 {
                    if z[0] != 0.0 || z[3] != 0.0 {
                        curr_z = Complex64::new(self.yeq.re * z[0], self.yeq.im * z[3]);
                    }
                    if z[2] != 0.0 || z[5] != 0.0 {
                        let y = self.interpolate_y95_ylow(vmag);
                        curr_p = Complex64::new(y.re * z[2], y.im * z[5]);
                    }
                    if z[1] != 0.0 || z[4] != 0.0 {
                        let y = self.interpolate_y95i_ylow(vmag);
                        curr_i = Complex64::new(y.re * z[1], y.im * z[4]);
                    }
                    (curr_z + curr_i + curr_p) * v
                } else if vmag > self.v_base105 {
                    if z[0] != 0.0 || z[3] != 0.0 {
                        curr_z = Complex64::new(self.yeq.re * z[0], self.yeq.im * z[3]);
                    }
                    if z[2] != 0.0 || z[5] != 0.0 {
                        curr_p = Complex64::new(self.yeq105.re * z[2], self.yeq105.im * z[5]);
                    }
                    if z[1] != 0.0 || z[4] != 0.0 {
                        curr_i = Complex64::new(self.yeq105i.re * z[1], self.yeq105i.im * z[4]);
                    }
                    (curr_z + curr_i + curr_p) * v
                } else {
                    if z[0] != 0.0 || z[3] != 0.0 {
                        curr_z = Complex64::new(self.yeq.re * z[0], self.yeq.im * z[3]) * v;
                    }
                    if z[1] != 0.0 || z[4] != 0.0 {
                        curr_i = (Complex64::new(self.w_nominal * z[1], self.var_nominal * z[4])
                            / ((v / v.norm()) * self.v_base))
                            .conj();
                    }
                    if z[2] != 0.0 || z[5] != 0.0 {
                        curr_p = (Complex64::new(self.w_nominal * z[2], self.var_nominal * z[5])
                            / v)
                            .conj();
                    }
                    curr_z + curr_i + curr_p
                };

                // Low-voltage drop-out.
                if z[6] > 0.0 {
                    let vx = 500.0 * (vmag / self.v_base - z[6]);
                    if vx < 20.0 {
                        // ≥ 20 ⇒ yv is 1 for an f64
                        let evx = (2.0 * vx).exp();
                        let yv = 0.5 * (1.0 + (evx - 1.0) / (evx + 1.0));
                        curr *= yv;
                    }
                }
                curr
            }
        }
    }

    /// Pascal `CalcLoadModelContribution` (power-flow modes): compute total
    /// load currents and add them into `InjCurrent`/`ITerminal`.
    fn calc_load_model_contribution(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        self.cd.iterminal_updated = false;
        // Harmonic mode arrives in Phase 7.

        self.calc_yprim_contribution(node_v); // init InjCurrent array
        self.calc_vterminal_phase(sys, node_v); // actual voltage across each phase
        self.cd.zero_iterminal();

        for i in 0..self.cd.nphases {
            let v = self.cd.vterminal[i];
            let curr = self.model_current(v, errors);

            // Save in case the Load value differs from the terminal value.
            self.phase_curr[i] = curr;

            self.stick_curr(true, -curr, i); // into ITerminal
            self.cd.iterminal_updated = true;
            self.cd.iterminal_solution_count = sys.solution_count;
            self.stick_curr(false, curr, i); // into InjCurrent
        }
    }

    /// Pascal `CalcInjCurrentArray`.
    fn calc_inj_current_array(
        &mut self,
        sys: &SysCtx,
        node_v: &[Complex64],
        errors: &mut Vec<String>,
    ) {
        if self.cd.all_conductors_closed() {
            self.calc_load_model_contribution(sys, node_v, errors);
            return;
        }

        // Open terminals: use the admittance model for injection
        // ("THIS MAY NOT WORK !!! WATCH FOR BAD RESULTS" — ported as-is).
        if self.open_load_solution_count != sys.solution_count {
            let mut y = CMatrix::new(self.cd.yorder);
            self.calc_yprim_matrix(&mut y, sys);
            let mut k = 0usize;
            for t in 0..self.cd.nterms {
                for j in 0..self.cd.nconds {
                    if !self.cd.terminals[t].conductors_closed[j] {
                        y.zero_row(j + k);
                        y.zero_col(j + k);
                        y.set(j + k, j + k, Complex64::new(1.0e-12, 0.0));
                    }
                }
                k += self.cd.nconds;
            }
            self.yprim_open_cond = Some(y);
            self.open_load_solution_count = sys.solution_count;
        }
        self.cd.compute_vterminal(node_v);
        let cd = &mut self.cd;
        if let Some(y) = &self.yprim_open_cond {
            y.mv_mult(&mut cd.complex_buffer, &cd.vterminal);
        }
        for v in cd.complex_buffer.iter_mut() {
            *v = -*v;
        }
    }
}

/// The snapshot defaults the executive uses for `RecalcElementData` at parse
/// time (Pascal reads live `ActiveCircuit` state there, but every derived
/// value is recomputed inside `CalcYPrim` before the solver consumes it).
pub fn default_recalc_ctx() -> SysCtx {
    SysCtx {
        frequency: 60.0,
        fundamental: 60.0,
        is_harmonic_model: false,
        is_dynamic_model: false,
        load_model: crate::solution::POWERFLOW,
        mode: SolveMode::Snapshot,
        load_multiplier: 1.0,
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

impl CktElement for Load {
    fn cd(&self) -> &CktElementData {
        &self.cd
    }
    fn cd_mut(&mut self) -> &mut CktElementData {
        &mut self.cd
    }

    fn recalc_element_data(&mut self, sys: &SysCtx) {
        self.recalc(sys);
    }

    /// Pascal `TLoadObj.CalcYPrim`.
    fn calc_yprim(&mut self, sys: &SysCtx) {
        let yorder = self.cd.yorder;
        let mut yp_shunt = CMatrix::new(yorder);

        // POWERFLOW and ADMITTANCE both use the admittance matrix here.
        self.set_nominal_load(sys);
        self.calc_yprim_matrix(&mut yp_shunt, sys);

        // YPrim_Series from the shunt diagonal so CalcVoltages doesn't fail.
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

    /// Pascal `TLoadObj.InjCurrents` + `TPCElement.InjCurrents`.
    fn inj_currents(&mut self, sys: &SysCtx, ctx: &mut InjCtx) {
        if !self.cd.enabled {
            return;
        }
        if sys.loads_need_updating {
            self.set_nominal_load(sys);
        }
        let mut errors = Vec::new();
        self.calc_inj_current_array(sys, ctx.node_v, &mut errors);
        for i in 0..self.cd.yorder {
            ctx.currents[self.cd.node_ref[i]] += self.cd.inj_current[i];
        }
    }

    /// Pascal `TPCElement.GetCurrents` + `TLoadObj.GetTerminalCurrents`.
    fn get_currents(&mut self, sys: &SysCtx, node_v: &[Complex64], curr: &mut [Complex64]) {
        if !self.cd.enabled {
            curr.fill(Complex64::ZERO);
            return;
        }
        // (LastSolutionWasDirect shortcut omitted: Phase 3 always runs the
        // power-flow path before currents are queried.)
        if self.cd.iterminal_solution_count != sys.solution_count {
            let mut errors = Vec::new();
            self.calc_load_model_contribution(sys, node_v, &mut errors);
        }
        // TPCElement.GetTerminalCurrents
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

impl DssObject for Load {
    fn data(&self) -> &DssObjData {
        &self.cd.obj
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.cd.obj
    }
    fn as_any(&self) -> &dyn std::any::Any {
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
            KV => self.kv_load_base,
            KW => self.kw_base,
            PF => self.pf_nominal,
            KVAR => self.kvar_base,
            RNEUT => self.rneut,
            XNEUT => self.xneut,
            VMINPU => self.vminpu,
            VMAXPU => self.vmaxpu,
            VMINNORM => self.vmin_normal,
            VMINEMERG => self.vmin_emerg,
            XFKVA => self.connected_kva,
            ALLOCATIONFACTOR => self.kva_allocation_factor,
            KVA => self.kva_base,
            PCTMEAN => self.pu_mean,
            PCTSTDDEV => self.pu_std_dev,
            CVRWATTS => self.cvr_watt_factor,
            CVRVARS => self.cvr_var_factor,
            KWH => self.kwh,
            KWHDAYS => self.kwh_days,
            CFACTOR => self.c_factor,
            PCT_SERIES_RL => self.pu_series_rl,
            REL_WEIGHT => self.rel_weighting,
            VLOWPU => self.vlowpu,
            PUXHARM => self.pu_x_harm,
            XRHARM => self.xr_harm_ratio,
            BASE_FREQ => self.cd.base_frequency,
            _ => unreachable!("Load has no double property {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        use prop::*;
        match idx {
            KV => self.kv_load_base = value,
            KW => self.kw_base = value,
            PF => self.pf_nominal = value,
            KVAR => self.kvar_base = value,
            RNEUT => self.rneut = value,
            XNEUT => self.xneut = value,
            VMINPU => self.vminpu = value,
            VMAXPU => self.vmaxpu = value,
            VMINNORM => self.vmin_normal = value,
            VMINEMERG => self.vmin_emerg = value,
            XFKVA => self.connected_kva = value,
            ALLOCATIONFACTOR => self.kva_allocation_factor = value,
            KVA => self.kva_base = value,
            PCTMEAN => self.pu_mean = value,
            PCTSTDDEV => self.pu_std_dev = value,
            CVRWATTS => self.cvr_watt_factor = value,
            CVRVARS => self.cvr_var_factor = value,
            KWH => self.kwh = value,
            KWHDAYS => self.kwh_days = value,
            CFACTOR => self.c_factor = value,
            PCT_SERIES_RL => self.pu_series_rl = value,
            REL_WEIGHT => self.rel_weighting = value,
            VLOWPU => self.vlowpu = value,
            PUXHARM => self.pu_x_harm = value,
            XRHARM => self.xr_harm_ratio = value,
            BASE_FREQ => self.cd.base_frequency = value,
            _ => unreachable!("Load has no double property {idx}"),
        }
    }

    fn get_i32(&self, idx: usize) -> i32 {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases as i32,
            MODEL => self.load_model as i32,
            CONN => self.connection as i32,
            STATUS => self.status,
            CLS => self.load_class,
            NUMCUST => self.num_customers,
            _ => unreachable!("Load has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        use prop::*;
        match idx {
            PHASES => self.cd.nphases = value.max(0) as usize,
            MODEL => self.load_model = LoadModel::from_i32(value),
            CONN => {
                self.connection = if value == 1 {
                    Connection::Delta
                } else {
                    Connection::Wye
                }
            }
            STATUS => self.status = value,
            CLS => self.load_class = value,
            NUMCUST => self.num_customers = value,
            _ => unreachable!("Load has no integer property {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::ENABLED => self.cd.enabled,
            _ => unreachable!("Load has no boolean property {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::ENABLED => self.cd.set_enabled(value),
            _ => unreachable!("Load has no boolean property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape.clone(),
            DAILY => self.daily_shape.clone(),
            DUTY => self.duty_shape.clone(),
            GROWTH => self.growth_shape.clone(),
            CVRCURVE => self.cvr_shape.clone(),
            SPECTRUM => self.spectrum.clone(),
            _ => unreachable!("Load has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        use prop::*;
        match idx {
            YEARLY => self.yearly_shape = value,
            DAILY => self.daily_shape = value,
            DUTY => self.duty_shape = value,
            GROWTH => self.growth_shape = value,
            CVRCURVE => self.cvr_shape = value,
            SPECTRUM => self.spectrum = value,
            _ => unreachable!("Load has no string property {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        match idx {
            prop::ZIPV => Some(&self.zipv),
            _ => unreachable!("Load has no array property {idx}"),
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::ZIPV => {
                for (dst, src) in self.zipv.iter_mut().zip(value) {
                    *dst = src;
                }
            }
            _ => unreachable!("Load has no array property {idx}"),
        }
    }

    fn set_bus_name(&mut self, terminal: usize, value: &str) {
        self.cd.set_bus(terminal, value);
    }
    fn get_bus_name(&self, terminal: usize) -> String {
        self.cd.get_bus(terminal).to_string()
    }

    /// Pascal `TLoadObj.PropertySideEffects` (text-parser path: every edit
    /// invalidates Yprim through `EndEdit`).
    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        use prop::*;
        match idx {
            CONN => {
                let n = nconds_for_connection(self.connection, self.cd.nphases);
                self.cd.set_nconds(n);
                self.update_vbase();
                self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
                self.cd.yprim_invalid = true;
            }
            KV | PHASES => {
                if idx == PHASES {
                    self.phase_curr = vec![Complex64::ZERO; self.cd.nphases];
                    let n = nconds_for_connection(self.connection, self.cd.nphases);
                    self.cd.set_nconds(n); // force reallocation of terminal info
                }
                self.update_vbase();
            }
            KW => {
                self.load_spec_type = LoadSpec::KwPf;
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(XFKVA);
                self.cd.obj.clear_seq(KWH);
                self.kw_ref = self.kw_base;
            }
            PF => {
                self.pf_changed = true;
                self.pf_specified = true;
                self.cd.obj.clear_seq(KVAR);
            }
            ALLOCATIONFACTOR => {
                self.allocation_factor = self.kva_allocation_factor;
                self.load_spec_type = LoadSpec::ConnectedKvaPf;
                self.cd.obj.set_as_next_seq(XFKVA);
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(KWH);
                self.compute_allocated_load();
                self.has_been_allocated = true;
            }
            CFACTOR => {
                self.allocation_factor = self.c_factor;
                self.load_spec_type = LoadSpec::KwhPf;
                self.cd.obj.set_as_next_seq(KWH);
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(XFKVA);
                self.compute_allocated_load();
                self.has_been_allocated = true;
            }
            DAILY => {
                if self.yearly_shape.is_empty() {
                    self.yearly_shape = self.daily_shape.clone();
                }
            }
            KWH => {
                self.load_spec_type = LoadSpec::KwhPf;
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(XFKVA);
                self.allocation_factor = self.c_factor;
                self.compute_allocated_load();
            }
            KVAR => {
                self.load_spec_type = LoadSpec::KwKvar;
                if !self.cd.obj.prp_specified(KW) {
                    self.cd.obj.set_as_next_seq(KW);
                }
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(PF);
                self.cd.obj.clear_seq(KWH);
                self.cd.obj.clear_seq(XFKVA);
                self.pf_specified = false;
                self.kvar_ref = self.kvar_base;
            }
            XFKVA => {
                self.load_spec_type = LoadSpec::ConnectedKvaPf;
                self.cd.obj.set_as_next_seq(XFKVA);
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(KWH);
                self.allocation_factor = self.kva_allocation_factor;
                self.compute_allocated_load();
            }
            KWHDAYS => {
                self.load_spec_type = LoadSpec::KwhPf;
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KVA);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(XFKVA);
                self.compute_allocated_load();
            }
            KVA => {
                self.load_spec_type = LoadSpec::KvaPf;
                self.cd.obj.set_as_next_seq(PF);
                self.cd.obj.clear_seq(KWH);
                self.cd.obj.clear_seq(KVAR);
                self.cd.obj.clear_seq(KW);
                self.cd.obj.clear_seq(XFKVA);
            }
            ZIPV => self.zipv_set = true,
            _ => {}
        }
    }

    /// Pascal `TLoad.EndEdit`: `RecalcElementData` + Yprim invalidation.
    fn end_edit(&mut self) {
        self.recalc(&default_recalc_ctx());
        self.cd.yprim_invalid = true;
    }

    /// Pascal `TLoadObj.MakeLike`.
    fn make_like(&mut self, other: &dyn DssObject) {
        let Some(other) = other.as_any().downcast_ref::<Load>() else {
            return;
        };
        self.cd.make_like_base(&other.cd);
        self.connection = other.connection;
        if self.cd.nphases != other.cd.nphases {
            self.cd.nphases = other.cd.nphases;
            let n = nconds_for_connection(self.connection, self.cd.nphases);
            self.cd.set_nconds(n);
            self.cd.yprim_invalid = true;
        }
        self.kv_load_base = other.kv_load_base;
        self.v_base = other.v_base;
        self.vlowpu = other.vlowpu;
        self.vminpu = other.vminpu;
        self.vmaxpu = other.vmaxpu;
        self.v_base_low = other.v_base_low;
        self.v_base95 = other.v_base95;
        self.v_base105 = other.v_base105;
        self.kw_base = other.kw_base;
        self.kva_base = other.kva_base;
        self.kvar_base = other.kvar_base;
        self.load_spec_type = other.load_spec_type;
        self.w_nominal = other.w_nominal;
        self.pf_nominal = other.pf_nominal;
        self.var_nominal = other.var_nominal;
        self.rneut = other.rneut;
        self.xneut = other.xneut;
        self.cvr_shape = other.cvr_shape.clone();
        self.daily_shape = other.daily_shape.clone();
        self.duty_shape = other.duty_shape.clone();
        self.yearly_shape = other.yearly_shape.clone();
        self.growth_shape = other.growth_shape.clone();
        self.load_class = other.load_class;
        self.num_customers = other.num_customers;
        self.load_model = other.load_model;
        self.status = other.status;
        self.kva_allocation_factor = other.kva_allocation_factor;
        self.connected_kva = other.connected_kva;
        self.cvr_watt_factor = other.cvr_watt_factor;
        self.cvr_var_factor = other.cvr_var_factor;
        self.shape_is_actual = other.shape_is_actual;
        self.pu_series_rl = other.pu_series_rl;
        self.rel_weighting = other.rel_weighting;
        self.cd.inj_current = vec![Complex64::ZERO; self.cd.yorder];
        self.phase_curr = vec![Complex64::ZERO; self.cd.nphases];
        self.zipv_set = other.zipv_set;
        if self.zipv_set {
            self.zipv = other.zipv;
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

impl Load {
    /// The shared VBase update from the kV/phases/conn side effects.
    fn update_vbase(&mut self) {
        self.v_base = match self.connection {
            Connection::Delta => self.kv_load_base * 1000.0,
            Connection::Wye => match self.cd.nphases {
                2 | 3 => self.kv_load_base * inv_sqrt3_x1000(),
                _ => self.kv_load_base * 1000.0,
            },
        };
    }

    /// Pascal `ComputeAllocatedLoad`.
    fn compute_allocated_load(&mut self) {
        match self.load_spec_type {
            LoadSpec::ConnectedKvaPf => {
                if self.connected_kva > 0.0 {
                    self.kw_base =
                        self.connected_kva * self.allocation_factor * self.pf_nominal.abs();
                    self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                    if self.pf_nominal < 0.0 {
                        self.kvar_base = -self.kvar_base;
                    }
                }
            }
            LoadSpec::KwhPf => {
                let f_avg_kw = self.kwh / (self.kwh_days * 24.0);
                self.kw_base = f_avg_kw * self.c_factor;
                self.kvar_base = self.kw_base * (1.0 / self.pf_nominal.powi(2) - 1.0).sqrt();
                if self.pf_nominal < 0.0 {
                    self.kvar_base = -self.kvar_base;
                }
            }
            _ => {}
        }
    }
}
