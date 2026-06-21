//! `LineCode` — catalog of line impedances referenced by `Line.linecode`.
//! Port of Pascal `General/LineCode.pas` (`TLineCodeObj`). A `DSS_OBJECT`
//! class (no terminals, no YPrim): it only holds per-unit-length `Z`/`Yc`
//! matrices and the sym-component scalars that produce them, which `TLineObj`
//! copies in `FetchLineCode` (Phase 4 WP4.2).
//!
//! Unlike `TLineObj`, a LineCode does **no** units conversion on its own
//! getters — values are stored in the code's declared `Units`, and the
//! relative conversion happens later when a Line fetches the code. Its
//! `CalcMatricesFromZ1Z0` also has **no** 1-phase / positive-sequence special
//! case (a 1-phase code still mixes in the default R0/X0/C0).
//!
//! Split into submodules (no behavioral change): the struct, its constructor
//! and the read-only accessors plus the property table live here; the matrix
//! algorithms (`CalcMatricesFromZ1Z0`/`Set_NumPhases`/`DoKronReduction`) are in
//! [`compute`], and the `DssObject` trait impl in [`accessors`].

#[cfg(test)]
mod tests;

mod accessors;
mod compute;

use crate::obj::base::DssObjData;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;

/// 1-based property ordinals (Pascal `TLineCodeProp`).
pub mod prop {
    pub const NPHASES: usize = 1;
    pub const R1: usize = 2;
    pub const X1: usize = 3;
    pub const R0: usize = 4;
    pub const X0: usize = 5;
    pub const C1: usize = 6;
    pub const C0: usize = 7;
    pub const UNITS: usize = 8;
    pub const RMATRIX: usize = 9;
    pub const XMATRIX: usize = 10;
    pub const CMATRIX: usize = 11;
    pub const BASE_FREQ: usize = 12;
    pub const NORMAMPS: usize = 13;
    pub const EMERGAMPS: usize = 14;
    pub const FAULTRATE: usize = 15;
    pub const PCTPERM: usize = 16;
    pub const REPAIR: usize = 17;
    pub const KRON: usize = 18;
    pub const RG: usize = 19;
    pub const XG: usize = 20;
    pub const RHO: usize = 21;
    pub const NEUTRAL: usize = 22;
    pub const B1: usize = 23;
    pub const B0: usize = 24;
    pub const SEASONS: usize = 25;
    pub const RATINGS: usize = 26;
    pub const LINE_TYPE: usize = 27;
    pub const NUM_PROPS: usize = 28; // incl. Like
}

/// `TLineCode.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    // The sym-component scalars are only shown while the sym model is active
    // (`PropertyOffset3 = @SymComponentsModel`, `ConditionalValue`).
    let conditional = PropFlags::CONDITIONAL_VALUE;
    let defs = vec![
        PropDef::integer("NPhases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("R1").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("X1").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("R0").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("X0").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("C1").scale(1.0e-9).flags(conditional),
        PropDef::double("C0").scale(1.0e-9).flags(conditional),
        PropDef::mapped_string_enum("Units", enums.units),
        PropDef::sym_matrix_real("RMatrix", NPHASES).flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::sym_matrix_imag("XMatrix", NPHASES).flags(PropFlags::UNITS_OHM_PER_LENGTH),
        // CMatrix stores susceptance; GetYCScale converts to/from nF on dump.
        PropDef::sym_matrix_imag("CMatrix", NPHASES).flags(PropFlags::SCALED_BY_FUNCTION),
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("NormAmps"),
        PropDef::double("EmergAmps"),
        PropDef::double("FaultRate"),
        PropDef::double("PctPerm"),
        PropDef::double("Repair"),
        // BooleanActionProperty: setting it `yes` runs DoKronReduction; the
        // getter always reads back `No` (it stores no state).
        PropDef::boolean("Kron"),
        PropDef::double("Rg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("Xg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("rho"),
        PropDef::integer("Neutral"),
        PropDef::double("B1")
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | conditional),
        PropDef::double("B0")
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | conditional),
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("Ratings", SEASONS),
        PropDef::mapped_string_enum("LineType", enums.line_type),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("LineCode", defs, true)
}

/// `TLineCodeObj`.
#[derive(Debug, Clone)]
pub struct LineCodeObj {
    data: DssObjData,
    /// Pascal `FNPhases` (matrix order).
    fnphases: i32,
    /// Pascal `FNeutralConductor` (1-based; 0 = none after Kron).
    fneutral_conductor: i32,
    num_amp_ratings: i32,
    sym_components_model: bool,
    /// Pascal `Flg.NeedsRecalc`: a matrix property was set, so `EndEdit` must
    /// reinvert `Zinv` from `Z`.
    needs_recalc: bool,
    /// Transient: the value last parsed into the `Kron` action property.
    kron_pending: bool,
    /// Base-frequency series impedance (ohms / unit length).
    z: Option<CMatrix>,
    zinv: Option<CMatrix>,
    /// Base-frequency shunt susceptance (S / unit length).
    yc: Option<CMatrix>,
    base_frequency: f64,
    r1: f64,
    x1: f64,
    r0: f64,
    x0: f64,
    c1: f64,
    c0: f64,
    norm_amps: f64,
    emerg_amps: f64,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
    rg: f64,
    xg: f64,
    rho: f64,
    amp_ratings: Vec<f64>,
    fline_type: i32,
    units: i32,
}

const TWO_PI: f64 = std::f64::consts::TAU;

impl LineCodeObj {
    /// Pascal `TLineCodeObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut obj = Self {
            data: DssObjData::new(name.to_lowercase(), prop::NUM_PROPS),
            fnphases: 3,
            fneutral_conductor: 3, // last conductor
            num_amp_ratings: 1,
            sym_components_model: true,
            needs_recalc: false,
            kron_pending: false,
            z: None,
            zinv: None,
            yc: None,
            base_frequency: 60.0, // ActiveCircuit.Fundamental
            r1: 0.0580,           // ohms per 1000 ft
            x1: 0.1206,
            r0: 0.1784,
            x0: 0.4047,
            c1: 3.4e-9, // nF per 1000 ft (stored in farads)
            c0: 1.6e-9,
            norm_amps: 400.0,
            emerg_amps: 600.0,
            fault_rate: 0.1,
            pct_perm: 20.0,
            // TODO(compat): Pascal `Create` sets `HrsToRepair := 3`, but the
            // oracle's `? linecode.x.repair` reads back 0 for a default code
            // (Line keeps 3). The field is deprecated/unused since 2014 — never
            // propagated to lines — so we default it to the value the getter
            // reports. The clean fix drops this dead field entirely.
            hrs_to_repair: 0.0,
            rg: 0.01805, // ohms per 1000'
            xg: 0.155081,
            rho: 100.0,
            amp_ratings: vec![400.0],
            fline_type: 1, // OH line
            units: 0,      // UNITS_NONE
        };
        for p in [prop::R1, prop::X1, prop::R0, prop::X0, prop::C1, prop::C0] {
            obj.data.set_as_next_seq(p);
        }
        obj.calc_matrices_from_z1z0();
        obj
    }

    // Read accessors consumed by `TLineObj.FetchLineCode` (Phase 4 WP4.2).
    pub fn base_frequency(&self) -> f64 {
        self.base_frequency
    }
    pub fn sym_components_model(&self) -> bool {
        self.sym_components_model
    }
    pub fn r1(&self) -> f64 {
        self.r1
    }
    pub fn x1(&self) -> f64 {
        self.x1
    }
    pub fn r0(&self) -> f64 {
        self.r0
    }
    pub fn x0(&self) -> f64 {
        self.x0
    }
    pub fn c1(&self) -> f64 {
        self.c1
    }
    pub fn c0(&self) -> f64 {
        self.c0
    }
    pub fn rg(&self) -> f64 {
        self.rg
    }
    pub fn xg(&self) -> f64 {
        self.xg
    }
    pub fn rho(&self) -> f64 {
        self.rho
    }
    pub fn units(&self) -> i32 {
        self.units
    }
    pub fn norm_amps(&self) -> f64 {
        self.norm_amps
    }
    pub fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }
    pub fn num_amp_ratings(&self) -> i32 {
        self.num_amp_ratings
    }
    pub fn amp_ratings(&self) -> &[f64] {
        &self.amp_ratings
    }
    pub fn nphases(&self) -> i32 {
        self.fnphases
    }
    pub fn fline_type(&self) -> i32 {
        self.fline_type
    }
    pub fn z(&self) -> Option<&CMatrix> {
        self.z.as_ref()
    }
    pub fn yc(&self) -> Option<&CMatrix> {
        self.yc.as_ref()
    }
}
