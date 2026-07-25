//! Port of `PCElements/Load.pas` — `TLoadObj` with all 8 load models.
//!
//! The load is balanced over its phases: `WNominal`/`varNominal` are W/var
//! **per phase** (`/ Fnphases`). A wye load carries `nphases + 1` conductors
//! (the neutral); delta loads connect phase-to-phase. Injections use the
//! compensation form: `InjCurrent = Yprim·V` (from `CalcYPrimContribution`)
//! plus the model current distributed by `StickCurrInTerminalArray`.
//!
//! The class is split by concern: this file holds the property metadata, the
//! enums, the [`Load`] struct, its construction, and the parse-time
//! [`default_recalc_ctx`]; [`nominal`] holds `RecalcElementData`/
//! `SetNominalLoad`, the growth/shape multipliers, and the allocation helpers;
//! [`solve`] holds the solve-time Yprim / injection-current machinery, the load
//! models, and the EnergyMeter reliability helpers; [`accessors`] holds the
//! [`CktElement`](crate::elements::traits::CktElement)/[`DssObject`](crate::obj::base::DssObject)
//! trait impls.

#[cfg(test)]
mod tests;

mod accessors;
mod nominal;
mod solve;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::growth_shape::GrowthShapeObj;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::traits::{ElemRef, SysCtx};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;
use crate::util::CDOUBLEONE;

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

/// Pascal `TLoadStatus` (`Load.pas`; `Set status=`, `LoadStatusEnum`
/// `Variable=0`/`Fixed=1`/`Exempt=2`). Discriminants are user-visible and
/// frozen (round-trip through the `DssEnum` registry); `i32` survives only at
/// the property parse/report boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LoadStatus {
    Variable = 0,
    Fixed = 1,
    Exempt = 2,
}

impl LoadStatus {
    /// The `LoadStatusEnum` ordinal (property `?`/dump boundary value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// `TLoadStatus(ordinal)`; out-of-range yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Variable),
            1 => Some(Self::Fixed),
            2 => Some(Self::Exempt),
            _ => None,
        }
    }
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
    // Name literals carry the oracle display case (`AllPropertyNames`,
    // probe 2026-07-07; the `elements/pd/reactor/mod.rs` convention) — the
    // `save load` / dump serializers print them verbatim.
    let defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::double("kV")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::REQUIRED | PropFlags::UNITS_KV),
        // NB: kW/kvar/kVA carry no `Units_*` flag — the pinned 0.14.5 oracle
        // backend has none (`Load.pas` at tag 0.14.5); the vendored source added
        // them post-0.14.5 (`dss_capi` `90c572e4` "AltDSS-Schema: More units"),
        // an engine-inert schema-only change the port has not adopted.
        PropDef::double("kW").flags(PropFlags::REPLACE_ZERO | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("PF").flags(
            PropFlags::ORDERING_LAST
                | PropFlags::REQUIRED_IN_SPEC_SET
                | PropFlags::POWER_FACTOR_LIMITS,
        ),
        PropDef::mapped_int_enum("Model", enums.load_model),
        PropDef::object_ref_class("LoadShape", "Yearly"),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty"),
        PropDef::object_ref_class("GrowthShape", "Growth"),
        PropDef::mapped_string_enum("Conn", enums.connection),
        PropDef::double("kvar").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NO_DEFAULT),
        PropDef::double("RNeut").flags(PropFlags::UNITS_OHM),
        PropDef::double("XNeut").flags(PropFlags::UNITS_OHM),
        PropDef::mapped_string_enum("Status", enums.load_status),
        PropDef::integer("Class"),
        PropDef::double("VMinpu"),
        PropDef::double("VMaxpu"),
        PropDef::double("VMinNorm"),
        PropDef::double("VMinEmerg"),
        PropDef::double("XfkVA").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_KVA),
        PropDef::double("AllocationFactor"),
        PropDef::double("kVA").flags(
            PropFlags::REPLACE_ZERO | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NO_DEFAULT,
        ),
        PropDef::double("%Mean").scale(0.01),
        PropDef::double("%StdDev").scale(0.01),
        PropDef::double("CVRWatts"),
        PropDef::double("CVRVars"),
        PropDef::double("kWh").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_KWH),
        PropDef::double("kWhDays"),
        PropDef::double("CFactor"),
        PropDef::object_ref_class("LoadShape", "CVRCurve"),
        PropDef::integer("NumCust"),
        PropDef::double_f_array("ZIPV", 7).flags(PropFlags::NO_DEFAULT),
        PropDef::double("%SeriesRL").scale(0.01),
        PropDef::double("RelWeight"),
        PropDef::double("VLowpu"),
        PropDef::double("puXHarm").flags(PropFlags::NO_DEFAULT),
        PropDef::double("XRHarm"),
        // PCClass tail:
        PropDef::object_ref("Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
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
    pub status: LoadStatus,
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
    /// `EEN_Factor`/`UE_Factor`: overload-driven unserved-energy factors set by
    /// the EnergyMeter zone sweep (`TakeSample`) and consumed by
    /// [`Load::exceeds_normal`]/[`Load::unserved`].
    pub een_factor: f64,
    pub ue_factor: f64,
    pub yprim_open_cond: Option<CMatrix>,

    pub yearly_shape: String,
    pub daily_shape: String,
    pub duty_shape: String,
    pub growth_shape: String,
    pub cvr_shape: String,
    pub spectrum: String,

    /// Resolved shape objects (Pascal `YearlyShapeObj` etc.). Like
    /// `FetchLineCode`, the referenced object is snapshot-cloned at parse time
    /// (PHASE4_PLAN §3.4): `set_nominal_load` then drives `GetMultAtHour` on the
    /// owned copy, since the solve path only carries scalar `SysCtx`. Each
    /// `_ref` is the resolved object's stable [`ElemRef`] (kept for parity with
    /// the §3.1 reference pattern; the clone is self-sufficient for the lookup).
    pub yearly_shape_obj: Option<LoadShapeObj>,
    pub daily_shape_obj: Option<LoadShapeObj>,
    pub duty_shape_obj: Option<LoadShapeObj>,
    pub cvr_shape_obj: Option<LoadShapeObj>,
    pub growth_shape_obj: Option<GrowthShapeObj>,
    pub yearly_shape_ref: Option<ElemRef>,
    pub daily_shape_ref: Option<ElemRef>,
    pub duty_shape_ref: Option<ElemRef>,
    pub cvr_shape_ref: Option<ElemRef>,
    pub growth_shape_ref: Option<ElemRef>,

    /// Pascal `ShapeFactor`: the (P, Q) multiplier from the active shape in a
    /// time-series mode; `(1, 1)` otherwise. Recomputed each `SetNominalLoad`.
    pub shape_factor: Complex64,

    /// Resolved harmonic spectrum (Pascal `SpectrumObj`), snapshot-cloned at
    /// edit-completion; the harmonic current source applies it to the captured
    /// fundamental phase currents.
    pub spectrum_obj: Option<SpectrumObj>,
    /// Pascal `HarmMag`/`HarmAng`/`LoadFundamental`: the fundamental phase-current
    /// magnitudes/angles captured by `InitHarmonics` (the harmonic injection base)
    /// and the solution frequency at which harmonics mode was entered.
    pub harm_mag: Vec<f64>,
    pub harm_ang: Vec<f64>,
    pub load_fundamental: f64,
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
            status: LoadStatus::Variable,
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
            een_factor: 0.0,
            ue_factor: 0.0,
            yprim_open_cond: None,
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            growth_shape: String::new(),
            cvr_shape: String::new(),
            spectrum: "defaultload".to_string(),
            yearly_shape_obj: None,
            daily_shape_obj: None,
            duty_shape_obj: None,
            cvr_shape_obj: None,
            growth_shape_obj: None,
            yearly_shape_ref: None,
            daily_shape_ref: None,
            duty_shape_ref: None,
            cvr_shape_ref: None,
            growth_shape_ref: None,
            shape_factor: CDOUBLEONE,
            spectrum_obj: None,
            harm_mag: Vec::new(),
            harm_ang: Vec::new(),
            load_fundamental: 0.0,
        };
        load.cd.inj_current = vec![Complex64::ZERO; load.cd.yorder];
        // Pascal `TLoadObj.Create` ends with `RecalcElementData` (live
        // `ActiveCircuit.Solution`). `new` has no circuit; the executive runs that
        // live recalc after construction (`create_object_no_edit`) and at
        // `end_edit`. Direct-construction unit tests recalc explicitly.
        load
    }
}

/// Unit-test fixture: the fresh-circuit parse-time snapshot (see
/// [`SysCtx::parse_default`]). Production paths thread the LIVE `sys_ctx` — this
/// remains only as the default context for the class's `#[cfg(test)]` fixtures.
pub fn default_recalc_ctx() -> SysCtx {
    SysCtx::parse_default()
}
