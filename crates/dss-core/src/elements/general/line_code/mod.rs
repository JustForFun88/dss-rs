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
mod dump;

use crate::obj::base::DssObjData;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags, prop_index};
use crate::support::cmatrix::CMatrix;

/// Pascal line type (`Set LineType=`, `LineTypeEnum`; shared by `LineCode`,
/// `LineGeometry` and `Line`). Discriminants are user-visible and frozen
/// (round-trip through the `DssEnum` registry, ordinals `oh=1`..`busbar=12`);
/// `i32` survives only at the property parse/report boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum LineType {
    Oh = 1,
    Ug = 2,
    UgTs = 3,
    UgCn = 4,
    SwtLdbrk = 5,
    SwtFuse = 6,
    SwtSect = 7,
    SwtRec = 8,
    SwtDisc = 9,
    SwtBrk = 10,
    SwtElbow = 11,
    Busbar = 12,
}

impl LineType {
    /// The `LineTypeEnum` ordinal (property `?`/dump boundary value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// `TLineType(ordinal)`; out-of-range yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Oh),
            2 => Some(Self::Ug),
            3 => Some(Self::UgTs),
            4 => Some(Self::UgCn),
            5 => Some(Self::SwtLdbrk),
            6 => Some(Self::SwtFuse),
            7 => Some(Self::SwtSect),
            8 => Some(Self::SwtRec),
            9 => Some(Self::SwtDisc),
            10 => Some(Self::SwtBrk),
            11 => Some(Self::SwtElbow),
            12 => Some(Self::Busbar),
            _ => None,
        }
    }
}

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
    let mut defs = vec![
        // Pascal `LineCode.pas` gives `NPhases` no validation flags — a
        // non-positive value is accepted at parse and handled by the
        // `Set_NumPhases` side effect (`.max(0)` in `compute.rs`). The port carried
        // a spurious `NonNegative|NonZero` that both rejected `nphases<=0` (unlike
        // the oracle) and leaked an `exclusiveMinimum:0` into the schema.
        PropDef::integer("NPhases"),
        PropDef::double("R1")
            .flags(conditional | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("X1")
            .flags(conditional | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("R0").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("X0").flags(conditional | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("C1")
            .scale(1.0e-9)
            .flags(conditional | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_NF_PER_LENGTH),
        PropDef::double("C0")
            .scale(1.0e-9)
            .flags(conditional | PropFlags::UNITS_NF_PER_LENGTH),
        PropDef::mapped_string_enum("Units", enums.units),
        PropDef::sym_matrix_real("RMatrix", NPHASES)
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::sym_matrix_imag("XMatrix", NPHASES)
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_OHM_PER_LENGTH),
        // CMatrix stores susceptance; GetYCScale converts to/from nF on dump.
        PropDef::sym_matrix_imag("CMatrix", NPHASES)
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::UNITS_NF_PER_LENGTH),
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::double("NormAmps"),
        PropDef::double("EmergAmps"),
        // dss_capi 0.15.x (LineCode.pas:283-288): FaultRate/PctPerm/Repair are
        // flagged Deprecated+Unused — a LineCode never propagated them to its
        // Lines (unused in the engine since 2014), so they carry a
        // `deprecationMessage` in the JSON schema. Still parsed/stored/dumped
        // exactly as before (probe-confirmed: no runtime warning); the flags are
        // schema metadata only.
        PropDef::double("FaultRate").flags(PropFlags::DEPRECATED | PropFlags::UNUSED),
        PropDef::double("PctPerm").flags(PropFlags::DEPRECATED | PropFlags::UNUSED),
        PropDef::double("Repair").flags(PropFlags::DEPRECATED | PropFlags::UNUSED),
        // BooleanActionProperty: setting it `yes` runs DoKronReduction; the
        // getter always reads back `No` (it stores no state). The `BOOLEAN_ACTION`
        // marker carries the Pascal `writeOnly` + end-ordering.
        PropDef::boolean("Kron").flags(PropFlags::BOOLEAN_ACTION),
        PropDef::double("Rg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("Xg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("rho").flags(PropFlags::UNITS_OHM_METER),
        PropDef::integer("Neutral"),
        PropDef::double("B1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::REDUNDANT
                | PropFlags::REQUIRED_IN_SPEC_SET
                | conditional,
        ),
        PropDef::double("B0")
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | conditional),
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("Ratings", SEASONS),
        PropDef::mapped_string_enum("LineType", enums.line_type),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);

    // JSON default-mode redundancy (Pascal `LineCode.pas:318/323`): B1 defers to
    // C1, B0 to C0 (the `REDUNDANT` flags are set above).
    let c1 = prop_index(&defs, "C1");
    let c0 = prop_index(&defs, "C0");
    let b1 = prop_index(&defs, "B1");
    let b0 = prop_index(&defs, "B0");
    defs[b1 - 1].redundant_with = c1;
    defs[b0 - 1].redundant_with = c0;

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
    fline_type: LineType,
    units: i32,
}

const TWO_PI: f64 = std::f64::consts::TAU;

impl LineCodeObj {
    /// Pascal `TLineCodeObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut obj = Self {
            data: DssObjData::new(name.to_ascii_lowercase(), prop::NUM_PROPS),
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
            // `TLineCodeObj.Create` (`LineCode.pas:456-501`) never assigns
            // `HrsToRepair` at all — the `:= 3.0` belongs to `TLineObj.Create`
            // (`Line.pas:979`), a different class. So a LineCode's field is
            // whatever zero-initialization left, and the oracle agrees: probed,
            // `? linecode.lc1.repair` reads back 0 while `? line.l1.repair`
            // reads 3. (An earlier comment here asserted the opposite — that
            // `Create` sets 3 — and reached the right value anyway; the claim
            // was never in the source.) We store what the getter reports, so
            // this **matches the oracle exactly**: no divergence is reproduced
            // and no compat marker applies. What is left is plain dead-field
            // hygiene — the field is unused since 2014 and never propagated to
            // lines; drop it once nothing reads it.
            hrs_to_repair: 0.0,
            rg: 0.01805, // ohms per 1000'
            xg: 0.155081,
            rho: 100.0,
            amp_ratings: vec![400.0],
            fline_type: LineType::Oh,
            units: 0, // UNITS_NONE
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

    /// Seed the base frequency inherited from the circuit at creation (Pascal
    /// `TLineCodeObj.Create`: `BaseFrequency := ActiveCircuit.Fundamental`,
    /// LineCode.pas:493). Set before `edit`, whose `EndEdit` recomputes the
    /// frequency-dependent shunt admittance at this frequency; a later `basefreq=`
    /// property still overrides. Only differs from 60 Hz when the deck ran
    /// `Set DefaultBaseFrequency=` before `New circuit`.
    pub fn set_base_frequency(&mut self, f: f64) {
        self.base_frequency = f;
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
    pub fn fline_type(&self) -> LineType {
        self.fline_type
    }
    pub fn z(&self) -> Option<&CMatrix> {
        self.z.as_ref()
    }
    pub fn yc(&self) -> Option<&CMatrix> {
        self.yc.as_ref()
    }
}
