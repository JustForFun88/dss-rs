//! Port of `PDElements/Line.pas` — `TLineObj`. Impedance sources: symmetrical
//! components (R1/X1/R0/X0/C1/C0/B1/B0), the direct matrix specification
//! (rmatrix/xmatrix/cmatrix), the LineCode catalog (Phase 4), and the
//! `LineGeometry` Carson path (`geometry=`, WP7.1 step 3a). The `spacing=`/
//! `wires=`/`cncables=`/`tscables=` forms remain `NOT_PORTED` (step 3b).
//!
//! For the sym/matrix/linecode sources `Z`/`Yc` hold ohms (resp. susceptance)
//! **per unit length** at base frequency and `CalcYPrim` applies length, units
//! and frequency corrections. For the geometry source `Z`/`Yc` are already the
//! **total** matrices (the geometry's `Zmatrix[f, len, units]` folds length and
//! units in), so `CalcYPrim` inverts/embeds them directly.
//!
//! Split into submodules mirroring `load/`, `generator/`, `transformer/`:
//! - this `mod.rs` — property ordinals, `class_props`, the `Line` struct, `new`.
//! - `code.rs` — the LineCode/LineGeometry catalog-fetch and source-selection.
//! - `solve.rs` — the impedance/admittance numerics (`recalc`, `CalcYPrim`,
//!   `GetSeqLosses`) and the `impl CktElement`.
//! - `accessors.rs` — the `impl DssObject` property surface.

#[cfg(test)]
mod tests;

use crate::elements::ckt::CktElementData;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::elements::traits::ElemRef;
use crate::obj::base::DssObject;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;
use crate::support::line_units::LineUnits;

/// Pascal `ConductorChoice` (`PDElements/ConductorData.pas`): the conductor model
/// the spacing path selects (`FPhaseChoice`). `Unknown` until the first cable/wire
/// form decides; it steers `SetWires`' buried-neutral offset and the throwaway
/// geometry's engine kind in `FMakeZFromSpacing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConductorChoice {
    Unknown,
    Overhead,
    ConcentricNeutral,
    TapeShield,
}

mod accessors;
mod code;
mod solve;

/// 1-based property ordinals (Pascal `TLineProp` + class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const LINECODE: usize = 3;
    pub const LENGTH: usize = 4;
    pub const PHASES: usize = 5;
    pub const R1: usize = 6;
    pub const X1: usize = 7;
    pub const R0: usize = 8;
    pub const X0: usize = 9;
    pub const C1: usize = 10;
    pub const C0: usize = 11;
    pub const RMATRIX: usize = 12;
    pub const XMATRIX: usize = 13;
    pub const CMATRIX: usize = 14;
    pub const SWITCH: usize = 15;
    pub const RG: usize = 16;
    pub const XG: usize = 17;
    pub const RHO: usize = 18;
    pub const GEOMETRY: usize = 19;
    pub const UNITS: usize = 20;
    pub const SPACING: usize = 21;
    pub const WIRES: usize = 22;
    pub const EARTH_MODEL: usize = 23;
    pub const CNCABLES: usize = 24;
    pub const TSCABLES: usize = 25;
    pub const B1: usize = 26;
    pub const B0: usize = 27;
    pub const SEASONS: usize = 28;
    pub const RATINGS: usize = 29;
    pub const LINE_TYPE: usize = 30;
    // TPDClass tail:
    pub const NORMAMPS: usize = 31;
    pub const EMERGAMPS: usize = 32;
    pub const FAULTRATE: usize = 33;
    pub const PCTPERM: usize = 34;
    pub const REPAIR: usize = 35;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 36;
    pub const ENABLED: usize = 37;
    pub const NUM_PROPS: usize = 38; // incl. Like
}

/// `TLine.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        PropDef::bus("bus1", 1),
        PropDef::bus("bus2", 2),
        PropDef::object_ref_class("LineCode", "LineCode"),
        PropDef::double("length"),
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        // The sym-component scalars are shown only while the sym model is
        // active (`PropertyOffset3 = @SymComponentsModel`, ConditionalValue).
        PropDef::double("r1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("x1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("r0").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("x0").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("C1").flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::CONDITIONAL_VALUE),
        PropDef::double("C0").flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::CONDITIONAL_VALUE),
        PropDef::sym_matrix_real("rmatrix", PHASES)
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::sym_matrix_imag("xmatrix", PHASES)
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::sym_matrix_imag("cmatrix", PHASES).flags(PropFlags::SCALED_BY_FUNCTION),
        PropDef::boolean("Switch"),
        PropDef::double("Rg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("Xg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("rho"),
        PropDef::object_ref_class("LineGeometry", "geometry"),
        PropDef::mapped_string_enum("units", enums.units),
        PropDef::object_ref_class("LineSpacing", "spacing"),
        PropDef::object_ref_array("WireData", "wires"),
        PropDef::mapped_string_enum("EarthModel", enums.earth_model),
        PropDef::object_ref_array("CNData", "cncables"),
        PropDef::object_ref_array("TSData", "tscables"),
        PropDef::double("B1").flags(
            PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | PropFlags::CONDITIONAL_VALUE,
        ),
        PropDef::double("B0").flags(
            PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | PropFlags::CONDITIONAL_VALUE,
        ),
        PropDef::integer("Seasons"),
        PropDef::double_array("Ratings", SEASONS),
        PropDef::mapped_string_enum("LineType", enums.line_type),
        // TPDClass tail:
        PropDef::double("normamps"),
        PropDef::double("emergamps"),
        PropDef::double("faultrate"),
        PropDef::double("pctperm"),
        PropDef::double("repair"),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("Line", defs, true)
}

/// `TLineObj`.
pub struct Line {
    pub cd: CktElementData,

    /// Sequence parameters, ohms / farads per unit length.
    pub r1: f64,
    pub x1: f64,
    pub r0: f64,
    pub x0: f64,
    pub c1: f64,
    pub c0: f64,
    pub len: f64,
    pub length_units: LineUnits,
    pub user_length_units: LineUnits,
    /// `FLineCodeUnits`: the units the active LineCode declared, captured at
    /// `FetchLineCode`; drives the `units=` relative reconversion.
    pub line_code_units: LineUnits,
    /// `FUnitsConvert`.
    pub units_convert: f64,
    /// `LineCodeObj` reference (the resolved code's stable [`ElemRef`]) and its
    /// name for dumps; `None`/empty before any `linecode=`.
    pub line_code_ref: Option<ElemRef>,
    pub line_code_name: String,
    pub is_switch: bool,
    pub sym_components_model: bool,
    pub sym_components_changed: bool,
    pub cap_specified: bool,
    pub rg: f64,
    pub xg: f64,
    pub kxg: f64,
    pub rho: f64,
    pub earth_model: i32,
    pub line_type: i32,
    /// Pascal `LineGeometryObj` — the snapshot-cloned geometry a `geometry=`
    /// reference attaches (the WP4.2 `FetchLineCode` pattern). `Some` activates
    /// the Carson matrix path in [`Line::calc_yprim`], driving `Z`/`Yc` from the
    /// geometry instead of the sym/linecode data.
    pub geometry_obj: Option<LineGeometryObj>,
    /// The resolved geometry object's name (the `geometry=` dump value).
    pub geometry_name: String,
    /// Pascal `FZFrequency`: the frequency the geometry/spacing `Z`/`Yc` were last
    /// built for (`-1` = not yet computed), so [`Line::make_z_from_geometry`] /
    /// [`Line::make_z_from_spacing`] rebuild only on a frequency change (the total
    /// matrices fold in length + units).
    pub fz_frequency: f64,
    /// Pascal `LineSpacingObj`: the snapshot-cloned `LineSpacing` a `spacing=`
    /// reference attaches. With a non-empty [`Line::line_wire_data`] it activates
    /// the Carson spacing path (`SpacingSpecified`), driving `Z`/`Yc` like the
    /// geometry path but via a throwaway geometry built in `FMakeZFromSpacing`.
    pub line_spacing_obj: Option<LineSpacingObj>,
    /// Pascal `LineWireData` (sized `FWireDataSize`): the per-conductor catalog
    /// objects (`WireData`/`CNData`/`TSData`) the `wires=`/`cncables=`/`tscables=`
    /// forms fill. Empty = unallocated (Pascal NIL); allocated by `FetchLineSpacing`.
    pub line_wire_data: Vec<Option<Box<dyn DssObject>>>,
    /// Pascal `FPhaseChoice`: the conductor model in force for the spacing path.
    pub fphase_choice: ConductorChoice,
    /// Pascal `gotRatingsAfterSpacingConds`: set once ratings/amps are specified
    /// *after* the spacing conductors, so `FMakeZFromSpacing` won't overwrite them.
    pub got_ratings_after_spacing_conds: bool,
    /// Per-unit-length series impedance at base frequency.
    pub z: Option<CMatrix>,
    /// Per-unit-length shunt susceptance at base frequency.
    pub yc: Option<CMatrix>,
    // PD-element common:
    pub norm_amps: f64,
    pub emerg_amps: f64,
    pub fault_rate: f64,
    pub pct_perm: f64,
    pub hrs_to_repair: f64,
    pub miles_this_line: f64,
    pub num_amp_ratings: i32,
    pub amp_ratings: Vec<f64>,
}

// `line_wire_data: Vec<Option<Box<dyn DssObject>>>` is not `Clone`/`Debug`-derivable
// (trait objects), so both impls are written by hand (the `LineGeometryObj`
// precedent). Clone deep-copies each conductor via `clone_box`.
impl Clone for Line {
    fn clone(&self) -> Self {
        Self {
            cd: self.cd.clone(),
            r1: self.r1,
            x1: self.x1,
            r0: self.r0,
            x0: self.x0,
            c1: self.c1,
            c0: self.c0,
            len: self.len,
            length_units: self.length_units,
            user_length_units: self.user_length_units,
            line_code_units: self.line_code_units,
            units_convert: self.units_convert,
            line_code_ref: self.line_code_ref,
            line_code_name: self.line_code_name.clone(),
            is_switch: self.is_switch,
            sym_components_model: self.sym_components_model,
            sym_components_changed: self.sym_components_changed,
            cap_specified: self.cap_specified,
            rg: self.rg,
            xg: self.xg,
            kxg: self.kxg,
            rho: self.rho,
            earth_model: self.earth_model,
            line_type: self.line_type,
            geometry_obj: self.geometry_obj.clone(),
            geometry_name: self.geometry_name.clone(),
            fz_frequency: self.fz_frequency,
            line_spacing_obj: self.line_spacing_obj.clone(),
            line_wire_data: self
                .line_wire_data
                .iter()
                .map(|o| o.as_ref().map(|b| b.clone_box()))
                .collect(),
            fphase_choice: self.fphase_choice,
            got_ratings_after_spacing_conds: self.got_ratings_after_spacing_conds,
            z: self.z.clone(),
            yc: self.yc.clone(),
            norm_amps: self.norm_amps,
            emerg_amps: self.emerg_amps,
            fault_rate: self.fault_rate,
            pct_perm: self.pct_perm,
            hrs_to_repair: self.hrs_to_repair,
            miles_this_line: self.miles_this_line,
            num_amp_ratings: self.num_amp_ratings,
            amp_ratings: self.amp_ratings.clone(),
        }
    }
}

impl std::fmt::Debug for Line {
    // The conductor slots are `Box<dyn DssObject>` (not `Debug`); print the scalar
    // state and the active impedance source instead.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Line")
            .field("name", &self.cd.obj.name())
            .field("nphases", &self.cd.nphases)
            .field("len", &self.len)
            .field("sym_components_model", &self.sym_components_model)
            .field("geometry", &self.geometry_name)
            .field("spacing", &self.line_spacing_obj.is_some())
            .field("nwires", &self.line_wire_data.len())
            .finish_non_exhaustive()
    }
}

impl Line {
    /// Pascal `TLineObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2);

        let rho: f64 = 100.0;
        let xg: f64 = 0.155081;
        let base_freq = 60.0;
        let mut line = Self {
            cd,
            r1: 0.0580, // ohms per 1000 ft
            x1: 0.1206,
            r0: 0.1784,
            x0: 0.4047,
            c1: 3.4e-9, // nF per 1000 ft (stored in farads)
            c0: 1.6e-9,
            len: 1.0, // 1 kFt
            length_units: LineUnits::None,
            user_length_units: LineUnits::None,
            line_code_units: LineUnits::None,
            units_convert: 1.0,
            line_code_ref: None,
            line_code_name: String::new(),
            is_switch: false,
            sym_components_model: true,
            sym_components_changed: false,
            cap_specified: false,
            rg: 0.01805, // ohms per 1000 ft
            xg,
            kxg: xg / (658.5 * (rho / base_freq).sqrt()).ln(),
            rho,
            earth_model: 3, // DSS.DefaultEarthModel = DERI
            line_type: 1,   // OH line
            geometry_obj: None,
            geometry_name: String::new(),
            fz_frequency: -1.0,
            line_spacing_obj: None,
            line_wire_data: Vec::new(),
            fphase_choice: ConductorChoice::Unknown,
            got_ratings_after_spacing_conds: false,
            z: None,
            yc: None,
            norm_amps: 400.0,
            emerg_amps: 600.0,
            fault_rate: 0.1,
            pct_perm: 20.0,
            hrs_to_repair: 3.0,
            miles_this_line: 0.0,
            num_amp_ratings: 1,
            amp_ratings: vec![400.0],
        };
        for p in [prop::R1, prop::X1, prop::R0, prop::X0, prop::C1, prop::C0] {
            line.cd.obj.set_as_next_seq(p);
        }
        line.recalc(false);
        line
    }
}
