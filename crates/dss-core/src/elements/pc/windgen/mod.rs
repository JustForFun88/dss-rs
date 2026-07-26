//! Port of `PCElements/WindGen.pas` — `TWindGenObj`, the wind-generator PC
//! element (the dss_capi-cleaned 0.15.x form: no user-model, no `Xd`/`puXd`).
//!
//! WindGen is a `Generator`-shaped negative-load PC element with two twists:
//! its steady-state real power comes from an **aerodynamic** `P(v³·Cp)` curve
//! (the load shape supplies the wind speed in m/s, not a per-unit multiplier),
//! and its dynamics are driven by an embedded GE WTG type-3 model
//! ([`Wtg3Model`], the 50 µs sub-cycle) rather than the classic behind-Xd'
//! swing. Four power-flow models are ported (const-PQ / const-Z / const-P
//! fixed-Q / const-P fixed-X, enum values 1/2/4/5 — no PV/user/current-limited
//! model). Harmonics mode is **disabled upstream** (a loud abort), reproduced
//! 1:1.
//!
//! Split into submodules (this file holds the metadata, struct and `Create`):
//! - [`wtg3`]: the embedded `TGE_WTG3_Model` dynamics.
//! - [`nominal`]: shape multipliers + the aerodynamic `SetNominalGeneration` /
//!   `RecalcElementData`.
//! - [`solve`]: `CalcYPrimMatrix`, the four `DoXxxGen` model currents, the
//!   injection assembly and the disabled harmonics path.
//! - [`dynamics`]: `InitStateVars` / `IntegrateStates` / `DoDynamicMode` and the
//!   22 state variables.
//! - [`accessors`]: the `CktElement` / `DssObject` trait impls.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::elements::pc::dyneq_pce::DynEqPceData;
use crate::elements::traits::{ElemId, SysCtx};
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

use wtg3::Wtg3Model;

mod accessors;
mod dynamics;
mod nominal;
mod solve;
pub mod wtg3;

/// Pascal `TGeneralConnection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Wye = 0,
    Delta = 1,
}

/// 1-based property ordinals (Pascal `TWindGenProp` + the DynEqPCE/PC/CktElement
/// tails).
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
    pub const CONN: usize = 10;
    pub const KVAR: usize = 11;
    pub const CLS: usize = 12;
    pub const DEBUGTRACE: usize = 13;
    pub const VMINPU: usize = 14;
    pub const VMAXPU: usize = 15;
    pub const KVA: usize = 16;
    pub const MVA: usize = 17;
    pub const DUTYSTART: usize = 18;
    pub const DYNAMICEQ: usize = 19;
    pub const DYNOUT: usize = 20;
    pub const RTHEV: usize = 21;
    pub const XTHEV: usize = 22;
    pub const VSS: usize = 23;
    pub const PSS: usize = 24;
    pub const QSS: usize = 25;
    pub const VWIND: usize = 26;
    pub const QMODE: usize = 27;
    pub const SIMMECHFLG: usize = 28;
    pub const APCFLG: usize = 29;
    pub const QFLG: usize = 30;
    pub const DELT0: usize = 31;
    pub const N_WTG: usize = 32;
    pub const VV_CURVE: usize = 33;
    pub const AG: usize = 34;
    pub const CP: usize = 35;
    pub const LAMDA: usize = 36;
    pub const P: usize = 37;
    pub const PD: usize = 38;
    pub const PLOSS: usize = 39;
    pub const RAD: usize = 40;
    pub const VCUTIN: usize = 41;
    pub const VCUTOUT: usize = 42;
    // tails:
    pub const SPECTRUM: usize = 43;
    pub const BASE_FREQ: usize = 44;
    pub const ENABLED: usize = 45;
    pub const NUM_PROPS: usize = 46; // incl. Like
}

/// `TWindGen.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::bus("Bus1", 1),
        PropDef::double("kV").flags(PropFlags::NON_NEGATIVE),
        // L2 (DIVERGENCES.md): WindGen `kW`/`kVA`/`MVA` all carry the EPRI
        // `DblValueNZ` band-clamp (`REPLACE_ZERO`, applied unconditionally). The
        // dss_capi `NonZero` strict-error surface is NOT adopted (dss-ext-only).
        PropDef::double("kW").flags(PropFlags::REPLACE_ZERO),
        PropDef::double("PF"),
        PropDef::mapped_int_enum("Model", enums.windgen_model),
        PropDef::object_ref_class("LoadShape", "Yearly"),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty"),
        PropDef::mapped_string_enum("Conn", enums.connection),
        PropDef::double("kvar"),
        PropDef::integer("Class"),
        PropDef::boolean("DebugTrace"),
        PropDef::double("VMinpu"),
        PropDef::double("VMaxpu"),
        PropDef::double("kVA").flags(PropFlags::REPLACE_ZERO),
        // `MVA` = `DblValueNZ * 1000` (WindGen.pas:638, scale 1000): unlike
        // Generator's `MVA` (plain `DblValue`), WindGen's MVA also clamps — L2.
        PropDef::double("MVA")
            .scale(1000.0)
            .flags(PropFlags::REPLACE_ZERO | PropFlags::REDUNDANT)
            .redundant_with(prop::KVA),
        PropDef::double("DutyStart"),
        // Dynamics machinery (DynEqPCE base): the linked DynamicExp + its output
        // variable selection.
        PropDef::object_ref_class("DynamicExp", "DynamicEq"),
        PropDef::string_list("DynOut"),
        // WTG3 dynamics parameters (map onto the embedded WindModelDyn).
        PropDef::double("RThev"),
        PropDef::double("XThev"),
        PropDef::double("VSS").flags(PropFlags::NON_NEGATIVE),
        PropDef::double("PSS"),
        PropDef::double("QSS"),
        PropDef::double("VWind").flags(PropFlags::NON_NEGATIVE),
        PropDef::mapped_int_enum("QMode", enums.windgen_qmode),
        PropDef::integer("SimMechFlg"),
        PropDef::integer("APCFlg"),
        PropDef::integer("QFlg"),
        PropDef::double("delt0"),
        PropDef::integer("N_WTG"),
        PropDef::object_ref_class("XYCurve", "VV_Curve"),
        // Aerodynamic parameters (GenVars).
        PropDef::double("Ag"),
        PropDef::double("Cp"),
        PropDef::double("Lamda"),
        PropDef::double("P"),
        PropDef::double("pd").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::object_ref_class("XYCurve", "PLoss"),
        PropDef::double("Rad").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("VCutIn"),
        PropDef::double("VCutOut"),
        // PCClass tail:
        PropDef::object_ref_deferred("Spectrum", "Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("WindGen", defs, true)
}

/// `TWindGenObj`. `GenVars` (the public machine/aerodynamic record) is flattened
/// into these fields; the WTG3 dynamics live in [`Self::wind_model_dyn`].
#[derive(Debug, Clone)]
pub struct WindGen {
    pub cd: CktElementData,

    pub connection: Connection,
    pub gen_model: i32,
    pub gen_class: i32,
    pub kw_base: f64,
    pub kvar_base: f64,
    pub pf_nominal: f64,
    pub vmaxpu: f64,
    pub vminpu: f64,
    pub duty_start: f64,
    pub pv_factor: f64,
    pub forced_on: bool,
    pub gen_active: bool,

    // GenVars (machine + aerodynamic record).
    pub kv_windgen_base: f64,
    pub kva_rating: f64,
    pub h_mass: f64,
    pub m_mass: f64,
    pub d_damping: f64, // GenVars.D (actual damping value)
    pub dpu: f64,       // GenVars.Dpu (per-unit; WindGen never sets it → 0)
    pub xrdp: f64,      // GenVars.XRdp
    pub p_nominal_per_phase: f64,
    pub q_nominal_per_phase: f64,
    pub theta: f64,
    pub dtheta: f64,
    pub speed: f64,
    pub dspeed: f64,
    pub w0: f64,
    pub p_shaft: f64,
    pub v_thev_mag: f64,
    pub theta_history: f64,
    pub speed_history: f64,
    // Aerodynamic (GenVars).
    pub ag: f64,
    pub cp: f64,
    pub lamda: f64,
    pub poles: f64,
    pub pd: f64,
    pub rad: f64,
    pub v_cut_in: f64,
    pub v_cut_out: f64,
    pub pm: f64,
    pub ps: f64,
    pub pr: f64,
    pub pg: f64,
    pub s: f64,

    // Derived (RecalcElementData / SetNominalGeneration).
    pub v_base: f64,
    pub v_base95: f64,
    pub v_base105: f64,
    pub var_base: f64,
    pub yeq: Complex64,
    pub yeq95: Complex64,
    pub yeq105: Complex64,
    pub yq_fixed: f64,
    pub vthev: Complex64,
    pub edp: Complex64,

    pub gen_on: bool,
    pub gen_switch_open: bool,
    pub kva_not_set: bool,
    pub shape_factor: Complex64,
    pub shape_is_actual: bool,
    pub v_avg: f64,

    /// The embedded GE WTG type-3 dynamics model.
    pub wind_model_dyn: Wtg3Model,
    /// `DynEqPCE` base: the linked `DynamicExp` + its dynamics memory.
    pub dyneq: DynEqPceData,
    pub spectrum: String,
    pub spectrum_obj: Option<SpectrumObj>,

    pub yearly_shape: String,
    pub daily_shape: String,
    pub duty_shape: String,
    pub yearly_shape_obj: Option<LoadShapeObj>,
    pub daily_shape_obj: Option<LoadShapeObj>,
    pub duty_shape_obj: Option<LoadShapeObj>,
    pub yearly_shape_ref: Option<ElemId>,
    pub daily_shape_ref: Option<ElemId>,
    pub duty_shape_ref: Option<ElemId>,

    /// Volt-var control curve (`VV_Curve`).
    pub vv_curve: String,
    pub vv_curve_obj: Option<XyCurveObj>,
    pub vv_curve_ref: Option<ElemId>,
    /// Turbine active-power loss curve (`PLoss`).
    pub loss_curve: String,
    pub loss_curve_obj: Option<XyCurveObj>,
    pub loss_curve_ref: Option<ElemId>,
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

impl WindGen {
    /// Pascal `TWindGenObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 4; // defaults to wye
        cd.set_nterms(1);
        let base_frequency = cd.base_frequency;

        let kw_base = 1000.0;
        let kvar_base = 60.0;
        let kv_windgen_base = 12.47;
        let kva_rating = kw_base * 1.2;
        let v_base = 7200.0;
        let vminpu = 0.90;
        let vmaxpu = 1.10;

        let mut wind_model_dyn = Wtg3Model::new();
        // Pascal Create overrides the WindModelDyn defaults after Initialize.
        wind_model_dyn.vwind = 12.0;
        wind_model_dyn.q_mode = 0;

        let mut g = Self {
            cd,
            connection: Connection::Wye,
            gen_model: 1,
            gen_class: 1,
            kw_base,
            kvar_base,
            pf_nominal: 0.88,
            vmaxpu,
            vminpu,
            duty_start: 0.0,
            pv_factor: 0.1,
            forced_on: false,
            gen_active: true,
            kv_windgen_base,
            kva_rating,
            h_mass: 1.0,
            m_mass: 0.0,
            d_damping: 1.0, // GenVars.D := 1.0 in Create (Dpu stays 0)
            dpu: 0.0,
            xrdp: 20.0,
            p_nominal_per_phase: 0.0,
            q_nominal_per_phase: 0.0,
            theta: 0.0,
            dtheta: 0.0,
            speed: 0.0,
            dspeed: 0.0,
            w0: 2.0 * std::f64::consts::PI * base_frequency,
            p_shaft: 0.0,
            v_thev_mag: 0.0,
            theta_history: 0.0,
            speed_history: 0.0,
            ag: 1.0 / 90.0,
            cp: 0.41,
            lamda: 7.95,
            poles: 2.0,
            pd: 1.225,
            rad: 40.0,
            v_cut_in: 5.0,
            v_cut_out: 23.0,
            pm: 0.0,
            ps: 0.0,
            pr: 0.0,
            pg: 0.0,
            s: 0.0,
            v_base,
            v_base95: vminpu * v_base,
            v_base105: vmaxpu * v_base,
            var_base: 0.0,
            yeq: Complex64::ZERO,
            yeq95: Complex64::ZERO,
            yeq105: Complex64::ZERO,
            yq_fixed: 0.0,
            vthev: Complex64::ZERO,
            edp: Complex64::ZERO,
            gen_on: false,
            gen_switch_open: false,
            kva_not_set: true,
            shape_factor: Complex64::new(12.0, 0.0), // cmplx(WindModelDyn.VWind, 0)
            shape_is_actual: false,
            v_avg: 0.0,
            wind_model_dyn,
            dyneq: DynEqPceData::new(),
            spectrum: "defaultgen".to_string(),
            spectrum_obj: None,
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            yearly_shape_obj: None,
            daily_shape_obj: None,
            duty_shape_obj: None,
            yearly_shape_ref: None,
            daily_shape_ref: None,
            duty_shape_ref: None,
            vv_curve: String::new(),
            vv_curve_obj: None,
            vv_curve_ref: None,
            loss_curve: String::new(),
            loss_curve_obj: None,
            loss_curve_ref: None,
        };
        g.cd.inj_current = vec![Complex64::ZERO; g.cd.yorder];
        // Pascal `TWindGenObj.Create` ends with `RecalcElementData` (live
        // `ActiveCircuit.Solution`). `new` has no circuit; the executive runs that
        // live recalc after construction (`create_object_no_edit`) and at
        // `end_edit`. Direct-construction unit tests recalc explicitly.
        g
    }
}

/// Unit-test fixture: the fresh-circuit parse-time snapshot (see
/// [`SysCtx::parse_default`]). Production paths thread the LIVE `sys_ctx` — this
/// remains only as the default context for the class's `#[cfg(test)]` fixtures.
pub fn default_recalc_ctx() -> SysCtx {
    SysCtx::parse_default()
}
