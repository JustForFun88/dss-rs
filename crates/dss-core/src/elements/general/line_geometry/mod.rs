//! `LineGeometry` (`TLineGeometryObj`) — the multi-conductor overhead/cable
//! geometry a `Line` references to build its impedance via the Carson engine.
//! Port of Pascal `General/LineGeometry.pas`.
//!
//! A `DSS_OBJECT` catalog class (no terminals, no YPrim). It holds per-conductor
//! coordinate (`X`/`H`) and unit arrays plus a reference to each conductor's
//! catalog object (`WireData`/`CNData`/`TSData`), sized by `NConds`. Editing is a
//! small **state machine** keyed by the active conductor `Cond=`: `wire=`/
//! `cncable=`/`tscable=`/`x=`/`h=`/`units=` all write the slot selected by the
//! last `cond=`. The plural `wires=`/`cncables=`/`tscables=` forms write every
//! slot at once. `spacing=` copies an entire `LineSpacing`'s coordinates in.
//!
//! Referenced conductor/spacing objects are resolved and **snapshot-cloned** at
//! edit time (the WP4.2 `FetchLineCode` pattern), so the geometry owns the data
//! it needs. The held [`LineConstants`] engine (`FLineData`) is allocated/swapped
//! by [`LineGeometryObj::change_line_constants_type`] to track the active
//! conductor model; [`LineGeometryObj::update_line_geometry_data`] pushes every
//! conductor's geometry into it and runs the Carson `Calc` (plus a Kron `Reduce`
//! when `FReduce`), caching `Zmatrix`/`YCmatrix`. The [`LineGeometryObj::z_matrix`]
//! / [`LineGeometryObj::yc_matrix`] accessors recompute on demand when the
//! geometry is stale (`data_changed`); a `Line` consumes them in WP7.1 step 3.
//!
//! Split into submodules (no behavioral change): the struct, its constructor and
//! the read-only accessors live here; the `Cond=`-keyed edit state machine is in
//! [`edit`], the Carson `UpdateLineGeometryData`/matrix path in [`matrix`], and
//! the `DssObject` property trait (including `PropertySideEffects`) in
//! [`accessors`].

#[cfg(test)]
mod tests;

mod accessors;
mod dump;
mod edit;
mod matrix;
mod save;

use crate::elements::general::conductor_data::{CONDUCTOR_PROXY_CLASSES, CONDUCTOR_PROXY_NAME};
use crate::elements::general::line_code::LineType;
use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};
use crate::support::line_constants::LineConstants;

/// Pascal `LineUnits.UNITS_FT` — the `ft` ordinal; the value `FLastUnit` resets
/// to and the default coordinate unit.
const UNITS_FT: i32 = 5;

define_properties! {
    class "LineGeometry", abbrev true, enums enums;
    1  NCONDS    => PropDef::integer("NConds")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO);
    2  NPHASES   => PropDef::integer("NPhases").flags(PropFlags::NON_NEGATIVE);
    3  COND      => PropDef::integer("Cond");
    4  WIRE      => PropDef::object_ref_class("WireData", "Wire");
    5  X         => PropDef::double("X");
    6  H         => PropDef::double("H");
    7  UNITS     => PropDef::mapped_string_enum("Units", enums.units);
    8  NORMAMPS  => PropDef::double("NormAmps");
    9  EMERGAMPS => PropDef::double("EmergAmps");
    10 REDUCE    => PropDef::boolean("Reduce");
    11 SPACING   => PropDef::object_ref_class("LineSpacing", "Spacing");
    // r4133 LineGeometry.pas:346-396 (props 12/15/16): the direct `wires`/
    // `cncables`/`tscables` arms have NO `none` branch — a `none` token drives
    // `WireDataClass.Code := 'none'` → #10103 "not defined". BOTH gating oracles
    // reject a geometry-level `none` (0.14.5 #40303, r4133 #10103); only the
    // Line-level lists (compacted before the Carson calc) and the new `conductors`
    // prop accept it. So NO ALLOW_NONE_ITEM here (0.15.x-adoption sweep — the flag
    // was justified from dss_capi's side only).
    12 WIRES     => PropDef::object_ref_array("WireData", "Wires");
    13 CNCABLE   => PropDef::object_ref_class("CNData", "CNCable");
    14 TSCABLE   => PropDef::object_ref_class("TSData", "TSCable");
    15 CNCABLES  => PropDef::object_ref_array("CNData", "CNCables");
    16 TSCABLES  => PropDef::object_ref_array("TSData", "TSCables");
    17 SEASONS   => PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON);
    18 RATINGS   => PropDef::double_array("Ratings", SEASONS);
    19 LINETYPE  => PropDef::mapped_string_enum("LineType", enums.line_type);
    // dss_capi 0.15.x (LineGeometry.pas:80,287-291): `Conductors` — the merged
    // mixed wire/CN/TS object-reference-array over the 3-class proxy
    // `(WireData|CNData|TSData)` (`fullNames=True`, proxy `.Name = "Conductor"`).
    // HIDE_015X keeps the byte-exact 0.14.5 Dump/`Dump commands` goldens green
    // (no LineGeometry JSON golden defines conductors). Text parse is
    // upstream-broken (proxy `GetDSSClass` case bug: any real item errors #10103;
    // an all-`none` list errors "At least one valid conductor") — see
    // `parse_conductor_proxy` and the `Conductors` side effect.
    20 CONDUCTORS => PropDef::object_ref_array_proxy("Conductors", CONDUCTOR_PROXY_NAME, &CONDUCTOR_PROXY_CLASSES)
        .flags(PropFlags::FULL_NAME_AS_ARRAY | PropFlags::FULL_NAME_AS_JSON_ARRAY | PropFlags::ALLOW_NONE_ITEM | PropFlags::HIDE_015X);
}

/// Pascal `ConductorChoice`: the per-conductor model a conductor uses. Selects
/// the [`LineConstants`] engine kind allocated by
/// [`LineGeometryObj::change_line_constants_type`] and routes the
/// `wires=`/`wire=` side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConductorChoice {
    Unknown,
    Overhead,
    ConcentricNeutral,
    TapeShield,
}

/// `TLineGeometryObj`. Pascal stores the per-conductor data as 1-based
/// `pXxxArray`s of length `FNConds`; here they are plain 0-based `Vec`s (slot
/// `i` is conductor `i+1`). `FActiveCond` stays 1-based (the property value).
pub struct LineGeometryObj {
    data: DssObjData,
    fnconds: i32,
    fnphases: i32,
    factive_cond: i32,
    fphase_choice: Vec<ConductorChoice>,
    /// Snapshot-cloned conductor objects (`WireData`/`CNData`/`TSData`), one per
    /// conductor (`None` = not yet set, Pascal NIL).
    fwiredata: Vec<Option<Box<dyn DssObject>>>,
    fx: Vec<f64>,
    fy: Vec<f64>,
    funits: Vec<i32>,
    flast_unit: i32,
    freduce: bool,
    /// Pascal `FLineData`: the Carson engine, allocated/swapped by
    /// [`Self::change_line_constants_type`] to match the active conductor model
    /// and filled by [`Self::update_line_geometry_data`]. `None` until the first
    /// conductor exists (Pascal NIL when `FNConds = 0`).
    fline_data: Option<LineConstants>,
    /// Pascal `DataChanged`: set by any geometry-affecting edit so the matrix
    /// calc knows to recompute. `LineSpacing` has no such flag, so the geometry
    /// tracks its own staleness.
    data_changed: bool,
    norm_amps: f64,
    emerg_amps: f64,
    num_amp_ratings: i32,
    amp_ratings: Vec<f64>,
    fline_type: LineType,
    /// Snapshot-cloned `LineSpacing` (`spacing=`), or `None`.
    line_spacing_obj: Option<Box<dyn DssObject>>,
    // dss_capi 0.15.x equivalent-spacing state, copied from the referenced
    // `LineSpacing` when it is not detailed. Default `equivalent_spacing=false`
    // (the detailed per-conductor-coordinate model) preserves 0.14.5 numerics.
    // The distances are stored in `flast_unit` and converted to meters in
    // `update_line_geometry_data`.
    equivalent_spacing: bool,
    eq_dist_ph_ph: f64,
    eq_dist_ph_n: f64,
    avg_phase_height: f64,
    avg_neutral_height: f64,
}

impl Clone for LineGeometryObj {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            fnconds: self.fnconds,
            fnphases: self.fnphases,
            factive_cond: self.factive_cond,
            fphase_choice: self.fphase_choice.clone(),
            fwiredata: self
                .fwiredata
                .iter()
                .map(|o| o.as_ref().map(|b| b.clone_box()))
                .collect(),
            fx: self.fx.clone(),
            fy: self.fy.clone(),
            funits: self.funits.clone(),
            flast_unit: self.flast_unit,
            freduce: self.freduce,
            fline_data: self.fline_data.clone(),
            data_changed: self.data_changed,
            norm_amps: self.norm_amps,
            emerg_amps: self.emerg_amps,
            num_amp_ratings: self.num_amp_ratings,
            amp_ratings: self.amp_ratings.clone(),
            fline_type: self.fline_type,
            line_spacing_obj: self.line_spacing_obj.as_ref().map(|b| b.clone_box()),
            equivalent_spacing: self.equivalent_spacing,
            eq_dist_ph_ph: self.eq_dist_ph_ph,
            eq_dist_ph_n: self.eq_dist_ph_n,
            avg_phase_height: self.avg_phase_height,
            avg_neutral_height: self.avg_neutral_height,
        }
    }
}

impl std::fmt::Debug for LineGeometryObj {
    // The conductor/spacing slots are `Box<dyn DssObject>` (not `Debug`), so the
    // derived impl is unavailable; print the scalar geometry state instead.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineGeometryObj")
            .field("name", &self.data.name())
            .field("nconds", &self.fnconds)
            .field("nphases", &self.fnphases)
            .field("active_cond", &self.factive_cond)
            .field("reduce", &self.freduce)
            .field("line_type", &self.fline_type)
            .finish_non_exhaustive()
    }
}

impl LineGeometryObj {
    pub fn new(name: impl Into<String>) -> Self {
        // Pascal `TLineGeometryObj.Create`: zero conductors/phases (no
        // allocation), ActiveCond=1, LastUnit=ft, LineType=oh, 1 amp rating
        // equal to NormAmps (0).
        Self {
            data: DssObjData::new(name.into().to_ascii_lowercase(), prop::NUM_PROPS),
            fnconds: 0,
            fnphases: 0,
            factive_cond: 1,
            fphase_choice: Vec::new(),
            fwiredata: Vec::new(),
            fx: Vec::new(),
            fy: Vec::new(),
            funits: Vec::new(),
            flast_unit: UNITS_FT,
            freduce: false,
            fline_data: None,
            data_changed: true,
            norm_amps: 0.0,
            emerg_amps: 0.0,
            num_amp_ratings: 1,
            amp_ratings: vec![0.0],
            fline_type: LineType::Oh,
            line_spacing_obj: None,
            equivalent_spacing: false,
            eq_dist_ph_ph: 0.0,
            eq_dist_ph_n: 0.0,
            avg_phase_height: 0.0,
            avg_neutral_height: 0.0,
        }
    }

    /// 0-based index of the active conductor, or `None` when `FActiveCond` is out
    /// of `1..=FNConds` (e.g. `NConds=0`).
    pub(super) fn active_index(&self) -> Option<usize> {
        if self.factive_cond >= 1 && self.factive_cond <= self.fnconds {
            Some((self.factive_cond - 1) as usize)
        } else {
            None
        }
    }

    /// Pascal `LineGeometryObj.Get_Nconds` (LineGeometry.pas:754): the
    /// *effective* conductor count a consuming `Line` adopts as its phase count
    /// — `FNPhases` when the geometry is Kron-reduced (the reduced matrices are
    /// `FNPhases × FNPhases`), otherwise the full `FNConds`.
    pub fn nconds(&self) -> i32 {
        if self.freduce {
            self.fnphases
        } else {
            self.fnconds
        }
    }

    /// Pascal `LineGeometryObj.FNphases` — the (post-compaction) count of PHASE
    /// conductors actually present. The Line's `FMakeZFromSpacing` compares this
    /// against its own `Nphases` to catch a `none`-removed phase (#181021).
    pub fn nphases(&self) -> i32 {
        self.fnphases
    }

    /// Pascal `LineGeometryObj.NormAmps` (seeded from the first conductor unless
    /// set explicitly) — `TLineObj.FetchGeometryCode` copies it onto the Line.
    pub fn norm_amps(&self) -> f64 {
        self.norm_amps
    }

    /// Pascal `LineGeometryObj.EmergAmps`.
    pub fn emerg_amps(&self) -> f64 {
        self.emerg_amps
    }

    /// Pascal `LineGeometryObj.NumAmpRatings`.
    pub fn num_amp_ratings(&self) -> i32 {
        self.num_amp_ratings
    }

    /// Pascal `LineGeometryObj.AmpRatings`.
    pub fn amp_ratings(&self) -> &[f64] {
        &self.amp_ratings
    }

    /// Pascal `LineGeometryObj.FLineType`.
    pub fn line_type(&self) -> LineType {
        self.fline_type
    }

    /// Pascal `TLineGeometryObj.NWires` (`property NWires READ FNConds`,
    /// LineGeometry.pas:166): the conductor count the CIM `WireSpacingInfo`
    /// export iterates over. Read-only accessor for GAPS_PLAN WPG.18 Stage C.
    pub fn nwires(&self) -> i32 {
        self.fnconds
    }

    /// Pascal `Xcoord[i]` (`Get_FX`, LineGeometry.pas:161): per-conductor
    /// horizontal coordinate (in each conductor's own `Units[i]`). 0-based slot
    /// `i` is conductor `i+1`. Read-only accessor for WPG.18 Stage C.
    pub fn fx(&self) -> &[f64] {
        &self.fx
    }

    /// Pascal `Ycoord[i]` (`Get_FY`, LineGeometry.pas:162): per-conductor
    /// vertical coordinate. Read-only accessor for WPG.18 Stage C.
    pub fn fy(&self) -> &[f64] {
        &self.fy
    }

    /// Pascal `Units[i]` (`Get_FUnits`, LineGeometry.pas:163): per-conductor
    /// `LineUnits` code for the `fx`/`fy` coordinate. Read-only accessor for
    /// WPG.18 Stage C.
    pub fn funits(&self) -> &[i32] {
        &self.funits
    }

    /// Pascal `PhaseChoice[i] = Overhead` (`Get_PhaseChoice`,
    /// LineGeometry.pas:167/762): the CIM `WireSpacingInfo.isCable` flag reads
    /// `PhaseChoice[1]` (first conductor). `i` is 1-based. Read-only accessor
    /// for WPG.18 Stage C.
    pub fn conductor_is_overhead(&self, i_one_based: usize) -> bool {
        matches!(
            self.fphase_choice.get(i_one_based.wrapping_sub(1)),
            Some(ConductorChoice::Overhead)
        )
    }

    /// Pascal `ConductorData[i]` (`Get_ConductorData`, LineGeometry.pas:746):
    /// the per-conductor catalog object (`WireData`/`CNData`/`TSData`), or
    /// `None` (Pascal NIL). `i` is 1-based. Read-only accessor for the WPG.18
    /// Stage C `ACLineSegmentPhase.WireInfo` reference.
    pub fn conductor(&self, i_one_based: usize) -> Option<&dyn DssObject> {
        if i_one_based >= 1 && i_one_based <= self.fnconds as usize {
            self.fwiredata
                .get(i_one_based - 1)
                .and_then(|o| o.as_deref())
        } else {
            None
        }
    }
}
