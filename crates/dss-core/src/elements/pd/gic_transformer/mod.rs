//! Port of `PDElements/GICTransformer.pas` — `TGICTransformerObj`, a
//! resistance-only transformer model for geomagnetically-induced-current (GIC)
//! studies. A **shunt** PD element whose primitive Y is pure conductance blocks
//! (no frequency dependence): the winding DC paths through which GIC flows.
//!
//! Three connection types (`SpecType`):
//!   - `GSU` (1): one winding — a single G1 block on terminals 1-2.
//!   - `Auto` (2): series-to-common — G1 block (1-2) + G2 block (3-4), Bus2
//!     auto-tied to Bus3.
//!   - `YY` (3): two windings — G1 block (1-2) + G2 block (3-4).
//!
//! `R1`/`R2` are stored as the conductances `G1`/`G2` via the property
//! `INVERSE_VALUE` flag (Pascal `PropertyOffset[R1] := @obj.G1` + `InverseValue`);
//! the alternative spec is `%R1`/`%R2` on the `kVLL²/MVA` impedance base
//! (`FpctRSpecified` toggles which drives `RecalcElementData`).
//!
//! Split into submodules mirroring `reactor/`:
//! - this `mod.rs` — property ordinals, `class_props`, the struct, `new`.
//! - `solve.rs` — `recalc` (Zbase / %R↔G), `CalcYPrim` (conductance stamping),
//!   and the `impl CktElement`.
//! - `accessors.rs` — the `impl DssObject` property surface, side effects,
//!   `SetBusX`, `MakeLike`.

use crate::elements::ckt::CktElementData;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

mod accessors;
mod solve;

/// SpecType ordinals (Pascal `SPEC_GSU`/`SPEC_AUTO`/`SPEC_YY`).
pub(super) const SPEC_GSU: i32 = 1;
pub(super) const SPEC_AUTO: i32 = 2;
pub(super) const SPEC_YY: i32 = 3;

/// 1-based property ordinals (Pascal `TGICTransformerProp` + PD/CktElement tails).
pub mod prop {
    pub const BUS_H: usize = 1;
    pub const BUS_NH: usize = 2;
    pub const BUS_X: usize = 3;
    pub const BUS_NX: usize = 4;
    pub const PHASES: usize = 5;
    pub const TYP: usize = 6;
    pub const R1: usize = 7;
    pub const R2: usize = 8;
    pub const KVLL1: usize = 9;
    pub const KVLL2: usize = 10;
    pub const MVA: usize = 11;
    pub const VARCURVE: usize = 12;
    pub const PCT_R1: usize = 13;
    pub const PCT_R2: usize = 14;
    pub const K: usize = 15;
    // TPDClass tail:
    pub const NORMAMPS: usize = 16;
    pub const EMERGAMPS: usize = 17;
    pub const FAULTRATE: usize = 18;
    pub const PCTPERM: usize = 19;
    pub const REPAIR: usize = 20;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 21;
    pub const ENABLED: usize = 22;
    pub const NUM_PROPS: usize = 23; // incl. the auto-appended Like
}

/// `TGICTransformer.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Bus properties (Pascal `BusProperty`; BusX is `WriteByFunction`
        // SetBusX, reproduced by the GicTransformer::set_bus_name override).
        PropDef::bus("BusH", BUS_H),
        PropDef::bus("BusNH", BUS_NH),
        PropDef::bus("BusX", BUS_X),
        PropDef::bus("BusNX", BUS_NX),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::mapped_string_enum("Type", enums.gic_transformer_type),
        // R1/R2 stored as conductances G1/G2 (InverseValue).
        PropDef::double("R1").flags(PropFlags::INVERSE_VALUE | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("R2").flags(PropFlags::INVERSE_VALUE),
        PropDef::double("kVLL1"),
        PropDef::double("kVLL2"),
        PropDef::double("MVA"),
        PropDef::object_ref_class("XYcurve", "VarCurve"),
        // The `%R1`/`%R2` display spelling comes from Pascal's `pct` → `%`
        // rename in PopulatePropertyNames (§3 finding).
        PropDef::double("%R1").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NO_DEFAULT),
        PropDef::double("%R2").flags(PropFlags::NO_DEFAULT),
        PropDef::double("K"),
        // TPDClass tail:
        PropDef::double("NormAmps"),
        PropDef::double("EmergAmps"),
        PropDef::double("FaultRate"),
        PropDef::double("pctPerm"),
        PropDef::double("Repair"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("GICTransformer", defs, true)
}

/// `TGICTransformerObj`.
#[derive(Debug, Clone)]
pub struct GicTransformer {
    pub cd: CktElementData,
    /// Per-phase winding conductances (Pascal `G1`/`G2`); `R1`/`R2` alias
    /// `1/G1`/`1/G2` through the `INVERSE_VALUE` flag.
    g1: f64,
    g2: f64,
    /// 1 = GSU, 2 = Auto, 3 = YY (Pascal `SpecType`).
    spec_type: i32,
    /// `FMVARating`.
    mva_rating: f64,
    /// `VarCurve`: the referenced XYcurve name (dump round-trip) and a
    /// snapshot clone (Pascal `FVarCurveObj`), consumed by `var_output_record`
    /// (`Export GICMvars` — Pascal `WriteVarOutputRecord`, solve.rs).
    var_curve_name: String,
    var_curve: Option<XyCurveObj>,
    /// `FpctR1`/`FpctR2`: the %-on-base spec (Pascal derives G from these when
    /// `FpctRSpecified`, else derives these from G).
    pct_r1: f64,
    pct_r2: f64,
    /// `FZbase1`/`FZbase2` = kVLL²/MVA.
    z_base1: f64,
    z_base2: f64,
    /// `FpctRSpecified`: %R drives `RecalcElementData` (else R1/R2 do).
    pct_r_specified: bool,
    /// `KSpecified`: the K-factor (vs VarCurve) drives the Mvar output.
    k_specified: bool,
    /// `FKFactor` (default 2.2).
    k_factor: f64,
    /// `FkV1`/`FkV2` — line-line kV of the two windings.
    kv1: f64,
    kv2: f64,
    /// Pascal `IsShunt` (always true for this element).
    is_shunt: bool,
    // PD-element common fields (TPDElement).
    norm_amps: f64,
    emerg_amps: f64,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
}

impl GicTransformer {
    /// Pascal `TGICTransformerObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3; // Directly set conds and phases
        cd.nconds = 3;
        cd.set_nterms(2); // Force allocation of terminals and conductors

        // Default to grounded: Bus2 = Bus1.0
        let bus1 = cd.get_bus(1).to_string();
        cd.set_bus(2, &format!("{bus1}.0"));

        let mut t = Self {
            cd,
            g1: 10000.0,
            g2: 10000.0,
            spec_type: SPEC_GSU,
            mva_rating: 100.0,
            var_curve_name: String::new(),
            var_curve: None,
            pct_r1: 0.2,
            pct_r2: 0.2,
            z_base1: 0.0,
            z_base2: 0.0,
            pct_r_specified: false,
            k_specified: true,
            k_factor: 2.2,
            kv1: 500.0,
            kv2: 138.0,
            is_shunt: true,
            norm_amps: 0.0,
            emerg_amps: 0.0,
            fault_rate: 0.0,
            pct_perm: 100.0,
            hrs_to_repair: 0.0,
        };
        t.cd.yorder = t.cd.nterms * t.cd.nconds;

        // Force computation of G1, G2 from %R, then turn the flag off (Pascal
        // constructor sets FpctRSpecified TRUE, RecalcElementData, then FALSE).
        t.pct_r_specified = true;
        t.recalc();
        t.pct_r_specified = false;
        t
    }
}
