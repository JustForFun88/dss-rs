//! Port of `PDElements/Line.pas` — `TLineObj`. Impedance sources: symmetrical
//! components (R1/X1/R0/X0/C1/C0/B1/B0), the direct matrix specification
//! (rmatrix/xmatrix/cmatrix), the LineCode catalog (Phase 4), the
//! `LineGeometry` Carson path (`geometry=`, WP7.1 step 3a), and the
//! `spacing=`/`wires=`/`cncables=`/`tscables=` forms (step 3b —
//! `FMakeZFromSpacing` via a throwaway geometry, see `line_spacing_obj`).
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
use crate::elements::general::conductor_data::{
    CONDUCTOR_PROXY_CLASSES, CONDUCTOR_PROXY_NAME, ConductorObj,
};
use crate::elements::general::line_code::LineType;
use crate::elements::general::line_geometry::LineGeometryObj;
use crate::elements::general::line_spacing::LineSpacingObj;
use crate::elements::traits::ElemId;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags, prop_index};
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
mod dump;
mod save;
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
    // dss_capi 0.15.x (Line.pas:59-62, SVN r3913-era): EpsRMedium/HeightOffset/
    // HeightUnit surface the LineConstants medium permittivity + height offset;
    // `Conductors=34` is the merged mixed wire/CN/TS object-reference-array (the
    // 3-class `WireData|CNData|TSData` proxy). Their insertion shifts every
    // 0.14.5 tail prop +4 (NormAmps 30→34 upstream).
    pub const EPS_R_MEDIUM: usize = 31;
    pub const HEIGHT_OFFSET: usize = 32;
    pub const HEIGHT_UNIT: usize = 33;
    pub const CONDUCTORS: usize = 34;
    // TPDClass tail:
    pub const NORMAMPS: usize = 35;
    pub const EMERGAMPS: usize = 36;
    pub const FAULTRATE: usize = 37;
    pub const PCTPERM: usize = 38;
    pub const REPAIR: usize = 39;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 40;
    pub const ENABLED: usize = 41;
    pub const NUM_PROPS: usize = 42; // incl. Like
}

/// `TLine.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let mut defs = vec![
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::bus("Bus2", 2).flags(PropFlags::REQUIRED),
        PropDef::object_ref_class("LineCode", "LineCode").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("Length"),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        // The sym-component scalars are shown only while the sym model is
        // active (`PropertyOffset3 = @SymComponentsModel`, ConditionalValue).
        PropDef::double("R1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::REQUIRED_IN_SPEC_SET
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("X1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::REQUIRED_IN_SPEC_SET
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("R0").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("X0").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::double("C1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::REQUIRED_IN_SPEC_SET
                | PropFlags::UNITS_NF_PER_LENGTH,
        ),
        PropDef::double("C0").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::UNITS_NF_PER_LENGTH,
        ),
        PropDef::sym_matrix_real("RMatrix", PHASES).flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::REQUIRED_IN_SPEC_SET
                | PropFlags::NO_DEFAULT
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::sym_matrix_imag("XMatrix", PHASES).flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::REQUIRED_IN_SPEC_SET
                | PropFlags::NO_DEFAULT
                | PropFlags::UNITS_OHM_PER_LENGTH,
        ),
        PropDef::sym_matrix_imag("CMatrix", PHASES)
            .flags(PropFlags::SCALED_BY_FUNCTION | PropFlags::UNITS_NF_PER_LENGTH),
        PropDef::boolean("Switch").flags(PropFlags::ORDERING_FIRST),
        PropDef::double("Rg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("Xg").flags(PropFlags::UNITS_OHM_PER_LENGTH),
        PropDef::double("rho").flags(PropFlags::UNITS_OHM_METER),
        PropDef::object_ref_class("LineGeometry", "Geometry")
            .flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::mapped_string_enum("Units", enums.units),
        PropDef::object_ref_class("LineSpacing", "Spacing").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        // SVN r3902/r3913 (0.15.x): the conductor lists accept `none` entries
        // (NIL slot) — `TPropertyFlag.AllowNoneItem`. WP-U1.1 item 3. `Wires`
        // renders as the JSON key `Conductors` (see below) and is the required
        // member of the "Spacing, Wires" spec set (Pascal `Line.pas:341`).
        PropDef::object_ref_array("WireData", "Wires")
            .flags(PropFlags::ALLOW_NONE_ITEM | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::mapped_string_enum("EarthModel", enums.earth_model),
        PropDef::object_ref_array("CNData", "CNCables").flags(PropFlags::ALLOW_NONE_ITEM),
        PropDef::object_ref_array("TSData", "TSCables").flags(PropFlags::ALLOW_NONE_ITEM),
        PropDef::double("B1").flags(
            PropFlags::SCALED_BY_FUNCTION
                | PropFlags::REDUNDANT
                | PropFlags::CONDITIONAL_VALUE
                | PropFlags::REQUIRED_IN_SPEC_SET,
        ),
        PropDef::double("B0").flags(
            PropFlags::SCALED_BY_FUNCTION | PropFlags::REDUNDANT | PropFlags::CONDITIONAL_VALUE,
        ),
        PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON),
        PropDef::double_array("Ratings", SEASONS),
        PropDef::mapped_string_enum("LineType", enums.line_type),
        // dss_capi 0.15.x (Line.pas:375-431): plain-double EpsRMedium/HeightOffset
        // + a MappedStringEnum HeightUnit (the same `UnitsEnum` as `Units`). They
        // feed `LineConstants.SetEpsRMedium/SetHeightOffset/SetUserHeightUnit` at
        // the geometry/spacing Z build (see `line/solve.rs`).
        //
        // HIDE_015X defers these from the *full-enumeration* 0.14.5-gated surfaces
        // — the `Dump` text report and the AltDSS JSON export — whose byte-exact
        // goldens are pinned to 0.14.5. capi015 FULL DOES emit all three (probed
        // 2026-07-16), but the Line Dump/JSON surface cannot flip to capi015 until
        // the sibling wt-u14cnts lands `Conductors` (index 34, emitted between
        // HeightUnit and NormAmps). The `?` named-query + props-table (PROPS_015X)
        // surfaces still expose them (`linemedium.json` golden pins the rendering).
        PropDef::double("EpsRMedium").flags(PropFlags::HIDE_015X),
        PropDef::double("HeightOffset").flags(PropFlags::HIDE_015X),
        PropDef::mapped_string_enum("HeightUnit", enums.units).flags(PropFlags::HIDE_015X),
        // dss_capi 0.15.x (Line.pas:62,341-344): `Conductors` — the merged mixed
        // wire/CN/TS object-reference-array over the 3-class proxy
        // `(WireData|CNData|TSData)` (`fullNames=True`, proxy `.Name = "Conductor"`).
        // The spacing-spec-set required member (0.15.x replaced `Wires` with
        // `Conductors` in `'Spacing, Conductors'`). HIDE_015X keeps the byte-exact
        // 0.14.5 Dump/JSON/`Dump commands` goldens green (`gen_json.py` is
        // 0.14.5-pinned; DIVERGENCES.md §Line Conductors). Text parse resolves
        // class-prefixed items by a CASE-INSENSITIVE class match (r4133
        // `LowerCase(CondClass)`; the reproduced capi015 `GetDSSClass` case bug was
        // dropped in the 0.15.x-adoption sweep) — see `parse_conductor_proxy`; the
        // JSON "Conductors" key is still emitted via the `Wires` masquerade below.
        PropDef::object_ref_array_proxy(
            "Conductors",
            CONDUCTOR_PROXY_NAME,
            &CONDUCTOR_PROXY_CLASSES,
        )
        .flags(
            PropFlags::FULL_NAME_AS_ARRAY
                | PropFlags::FULL_NAME_AS_JSON_ARRAY
                | PropFlags::ALLOW_NONE_ITEM
                | PropFlags::REQUIRED_IN_SPEC_SET
                | PropFlags::HIDE_015X,
        ),
        // TPDClass tail:
        PropDef::double("NormAmps"),
        PropDef::double("EmergAmps"),
        PropDef::double("FaultRate"),
        PropDef::double("pctPerm"),
        PropDef::double("Repair"),
        // TCktElementClass tail (`CktElementClass.pas:97`).
        PropDef::double("BaseFreq").flags(
            PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::DYNAMIC_DEFAULT
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);

    // JSON metadata (Pascal `Line.pas:328-342,443-449`): `Wires` renders under
    // the key `Conductors` with each conductor's FullName; `CNCables`/`TSCables`
    // are redundant aliases of `Wires` (SuppressJSON, already set below via the
    // flag mutation); `B1`/`B0` defer to `C1`/`C0` (REDUNDANT flags set above).
    {
        let wires = prop_index(&defs, "Wires");
        let c1 = prop_index(&defs, "C1");
        let c0 = prop_index(&defs, "C0");
        let cncables = prop_index(&defs, "CNCables");
        let tscables = prop_index(&defs, "TSCables");
        let b1 = prop_index(&defs, "B1");
        let b0 = prop_index(&defs, "B0");
        defs[wires - 1].json_name = Some("Conductors");
        defs[wires - 1].flags |= PropFlags::FULL_NAME_AS_JSON_ARRAY;
        for alias in [cncables, tscables] {
            defs[alias - 1].flags |= PropFlags::REDUNDANT | PropFlags::SUPPRESS_JSON;
            defs[alias - 1].redundant_with = wires;
        }
        defs[b1 - 1].redundant_with = c1;
        defs[b0 - 1].redundant_with = c0;
    }
    ClassProps::new("Line", defs, true)
}

/// `TLineObj`.
#[derive(Clone)]
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
    /// `LineCodeObj` reference (the resolved code's stable [`ElemId`]) and its
    /// name for dumps; `None`/empty before any `linecode=`.
    pub line_code_ref: Option<ElemId>,
    pub line_code_name: String,
    pub is_switch: bool,
    pub sym_components_model: bool,
    pub sym_components_changed: bool,
    /// The Rust stand-in for Pascal's global `ActiveCircuit.PositiveSequence`
    /// (`Line.pas:1085`), which `TLineObj.RecalcElementData` reads whenever it
    /// runs. The executive syncs it from the live circuit at every New/Edit
    /// boundary, so the three edit-time recalc sites — the constructor
    /// (`Line.pas:1001`), the `phases=` side effect (`Line.pas:628`) and
    /// `FetchLineCode` (`Line.pas:572`) — collapse zero-sequence into positive-
    /// sequence *at edit time* exactly as upstream (readback-observable,
    /// probe-proven 2026-07-25). The solve path passes `sys.positive_sequence`
    /// directly (`solve.rs`).
    pub positive_sequence: bool,
    pub cap_specified: bool,
    pub rg: f64,
    pub xg: f64,
    pub kxg: f64,
    pub rho: f64,
    pub earth_model: i32,
    pub line_type: LineType,
    /// dss_capi 0.15.x `epsRMedium`: relative permittivity of the surrounding
    /// medium, pushed into the geometry/spacing `LineConstants` at the Z build
    /// (`SetEpsRMedium`). Raw stored value (the getter reads it directly); default
    /// `1.0` preserves 0.14.5 numerics (`E0 * 1.0 == E0`).
    pub eps_r_medium: f64,
    /// dss_capi 0.15.x `heightOffset`: conductor height offset in `height_units`,
    /// pushed into the `LineConstants` at the Z build (`SetHeightOffset`). Raw
    /// stored value; default `0.0`. Only observable on the equivalent-spacing path
    /// (the detailed-coordinate path re-sets `FY` from the raw coordinates).
    pub height_offset: f64,
    /// dss_capi 0.15.x `heightUnits`: the `LineUnits` code `height_offset` is
    /// expressed in (`SetUserHeightUnit`). Default `UNITS_M` (Meters).
    pub height_units: i32,
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
    pub line_wire_data: Vec<Option<ConductorObj>>,
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

impl std::fmt::Debug for Line {
    // Print the scalar state and the active impedance source only — the
    // conductor snapshots and the impedance matrices would bury it.
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
            positive_sequence: false,
            cap_specified: false,
            rg: 0.01805, // ohms per 1000 ft
            xg,
            // TODO(compat): 658.5 (not 658.8530451057239) — upstream `Line.pas`
            // keeps 658.5 for Kxg while `LineConstants` moved to the corrected
            // De; see accessors.rs (UPGRADE_PLAN WP-U1.2 B2/D1).
            kxg: xg / (658.5 * (rho / base_freq).sqrt()).ln(),
            rho,
            earth_model: 3, // DSS.DefaultEarthModel = DERI
            line_type: LineType::Oh,
            eps_r_medium: 1.0,
            height_offset: 0.0,
            height_units: LineUnits::Meter.code(), // UNITS_M
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
        line.recalc(line.positive_sequence);
        line
    }

    /// Sync the cached live `ActiveCircuit.PositiveSequence` (Pascal reads the
    /// global directly in `RecalcElementData`). The executive calls this at each
    /// New/Edit boundary; the edit-time recalc sites then read `positive_sequence`.
    pub fn set_positive_sequence(&mut self, positive_sequence: bool) {
        self.positive_sequence = positive_sequence;
    }

    /// Re-run `RecalcElementData` with the cached live positive-sequence flag —
    /// the executive calls this right after constructing a Line in a
    /// `CktModel=Positive` circuit so a defaults-only `New Line` collapses
    /// r0/x0/c0 at create time (the constructor's own recalc ran before the flag
    /// was known). Pascal's `TLineObj.Create` ends with `RecalcElementData`
    /// (`Line.pas:1001`), which reads the live global there.
    pub fn recalc_pos_seq(&mut self) {
        self.recalc(self.positive_sequence);
    }
}
