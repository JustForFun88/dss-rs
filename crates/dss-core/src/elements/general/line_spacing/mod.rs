//! `LineSpacing`: overhead-line conductor spacing geometry.
//!
//! A `DSS_OBJECT` catalog class (no terminals, no YPrim): a `LineGeometry`
//! references it by name and reads the per-conductor horizontal/vertical
//! coordinate arrays (`X`/`H`, in `Units`) plus `NConds`/`NPhases` to build the
//! Carson `LineConstants` matrices. Each coordinate list holds the values
//! given, in order, at most `nconds` of them: a conductor the list does not
//! reach has no position, which a geometry built on the spacing copies.
//! Every property but `nconds=` and `like=` is refused until `nconds=` is
//! given, and each `nconds=` starts the spacing over in feet with no position.

#[cfg(test)]
mod tests;

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};
use crate::report::save::save::{SaveCtx, save_write_token};
use crate::support::line_units::POSITION_WORDS;

/// The `ft` code of the length units: the default unit, and the unit each
/// `nconds=` returns `Units` to.
const UNITS_FT: i32 = 5;

/// The `like=` property, the one after the class's own.
const LIKE: usize = prop::NUM_PROPS;

define_properties! {
    class "LineSpacing", abbrev true, enums enums;
    1 NCONDS  => PropDef::integer("NConds");
    2 NPHASES => PropDef::integer("NPhases");
    3 X       => PropDef::double_v_array("X").size_prop(NCONDS).flags(PropFlags::GIVEN_PREFIX);
    4 H       => PropDef::double_v_array("H").size_prop(NCONDS).flags(PropFlags::GIVEN_PREFIX);
    5 UNITS   => PropDef::mapped_string_enum("Units", enums.units).unit_words(POSITION_WORDS);
    // dss_capi 0.15.x additions: the equivalent-spacing model.
    // `Detailed` (default true) selects per-conductor coordinates; when false the
    // four equivalent distances replace them. UPGRADE_PLAN.md WP-U1.4 rows B3/C1.
    6 DETAILED          => PropDef::boolean("Detailed");
    7 EQDISTPHPH        => PropDef::double("EqDistPhPh");
    8 EQDISTPHN         => PropDef::double("EqDistPhN");
    9 AVGPHASEHEIGHT    => PropDef::double("AvgPhaseHeight");
    10 AVGNEUTRALHEIGHT => PropDef::double("AvgNeutralHeight");
}

/// A LineSpacing. `fx`/`fy` hold the coordinates given, in order, at most
/// `fnconds` of each.
#[derive(Debug, Clone)]
pub struct LineSpacingObj {
    data: DssObjData,
    fx: Vec<f64>,
    fy: Vec<f64>,
    fnconds: i32,
    /// Whether an `nconds=` was accepted (or copied by `like=`). `fnconds`
    /// holds the documented default of 3 before it.
    nconds_given: bool,
    nphases: i32,
    units: i32,
    // dss_capi 0.15.x equivalent-spacing fields. `detailed` defaults `true`
    // (`EquivalentSpacing = not detailed`), so the default is the legacy
    // per-conductor-coordinate model.
    detailed: bool,
    eq_dist_ph_ph: f64,
    eq_dist_ph_n: f64,
    avg_phase_height: f64,
    avg_neutral_height: f64,
}

impl LineSpacingObj {
    /// Three conductors and three phases in feet, with no position given.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_ascii_lowercase(), prop::NUM_PROPS),
            fx: Vec::new(),
            fy: Vec::new(),
            fnconds: 3,
            nconds_given: false,
            nphases: 3,
            units: UNITS_FT,
            // No equivalent spacing: `detailed` Yes and every distance 0.
            detailed: true,
            eq_dist_ph_ph: 0.0,
            eq_dist_ph_n: 0.0,
            avg_phase_height: 0.0,
            avg_neutral_height: 0.0,
        }
    }

    /// Whether the spacing is an equivalent one (`detailed` No): a geometry
    /// reads this and the four equivalent distances when it builds the Carson
    /// matrices.
    pub fn equivalent_spacing(&self) -> bool {
        !self.detailed
    }
    /// Equivalent phase-phase distance (in this spacing's `Units`).
    pub fn eq_dist_ph_ph(&self) -> f64 {
        self.eq_dist_ph_ph
    }
    /// Equivalent phase-neutral distance (in this spacing's `Units`).
    pub fn eq_dist_ph_n(&self) -> f64 {
        self.eq_dist_ph_n
    }
    /// Average phase-conductor height (in this spacing's `Units`).
    pub fn avg_phase_height(&self) -> f64 {
        self.avg_phase_height
    }
    /// Average neutral-conductor height (in this spacing's `Units`).
    pub fn avg_neutral_height(&self) -> f64 {
        self.avg_neutral_height
    }

    /// The conductor count, `nconds`, a `LineGeometry` matches against when it
    /// reads this spacing's coordinates.
    pub fn nwires(&self) -> i32 {
        self.fnconds
    }
    /// The phase count a Line's `spacing=` adopts and its `wires=` uses to place
    /// bare neutrals after cable phases.
    pub fn nphases(&self) -> i32 {
        self.nphases
    }
    /// The x coordinates given, in order: conductor `i` has none past the end.
    /// Read with the heights and `Units` by a geometry's or a Line's `spacing=`.
    pub fn xcoord(&self) -> &[f64] {
        &self.fx
    }
    /// The heights given, in order: conductor `i` has none past the end.
    pub fn ycoord(&self) -> &[f64] {
        &self.fy
    }
    pub fn spacing_units(&self) -> i32 {
        self.units
    }

    /// An accepted `nconds=`: no position is kept, the unit returns to feet
    /// and the equivalent spacing is dropped (`detailed` Yes, every distance
    /// 0). The positions, `detailed` and the distances leave the set order,
    /// and `Units` keeps any place it has there, now holding `ft`. Kept, a
    /// position or a distance would be re-read in feet. The count allocates
    /// nothing. Pinned by
    /// `exec::tests::line_geometry_rules::a_spacing_nconds_keeps_no_position_and_returns_to_feet`.
    fn start_over(&mut self) {
        self.nconds_given = true;
        self.fx.clear();
        self.fy.clear();
        self.units = UNITS_FT;
        self.detailed = true;
        self.eq_dist_ph_ph = 0.0;
        self.eq_dist_ph_n = 0.0;
        self.avg_phase_height = 0.0;
        self.avg_neutral_height = 0.0;
        for p in [
            prop::X,
            prop::H,
            prop::DETAILED,
            prop::EQDISTPHPH,
            prop::EQDISTPHN,
            prop::AVGPHASEHEIGHT,
            prop::AVGNEUTRALHEIGHT,
        ] {
            self.data.clear_seq(p);
        }
    }

    /// The Save line body: `NConds=` first whenever it was given, since every
    /// property after it would reload refused or emptied otherwise, then the
    /// set properties in order. Pinned by
    /// `exec::tests::report::save_writes_linespacing_nconds_first`.
    pub(crate) fn save_write_body(&self, out: &mut String, cx: &SaveCtx) {
        if self.nconds_given {
            save_write_token(out, cx, self, prop::NCONDS);
        }
        let data = self.data();
        let mut iprop = data.next_property_set(None);
        while let Some(ip) = iprop {
            if ip != prop::NCONDS {
                save_write_token(out, cx, self, ip);
            }
            iprop = data.next_property_set(Some(ip));
        }
    }
}

impl LineSpacingObj {
    pub(crate) fn make_like(&mut self, other: &Self) {
        self.data.copy_prp_sequence_from(other.data());
        let o = other;
        {
            // The count, the phases, the coordinates as given and the unit.
            self.fnconds = o.fnconds;
            self.nconds_given = o.nconds_given;
            self.nphases = o.nphases;
            self.fx.clone_from(&o.fx);
            self.fy.clone_from(&o.fy);
            self.units = o.units;
            // The equivalent-spacing block is copied too, as the authority does:
            // r4133 `TLineSpacing.MakeLike` copies `FEquivalentSpacing`,
            // `FEqDistPhPh`, `FEqDistPhN`, `FAvgHeightPh` and `FAvgHeightN`
            // explicitly (`Version8/Source/General/LineSpacing.pas:251-255`).
            // Only dss_capi 0.15.x stops after Units, leaving the clone
            // reporting a spacing the source does not have; that regression is
            // not reproduced.
            self.detailed = o.detailed;
            self.eq_dist_ph_ph = o.eq_dist_ph_ph;
            self.eq_dist_ph_n = o.eq_dist_ph_n;
            self.avg_phase_height = o.avg_phase_height;
            self.avg_neutral_height = o.avg_neutral_height;
        }
    }
}

impl DssObject for LineSpacingObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::NCONDS => self.fnconds,
            prop::NPHASES => self.nphases,
            prop::UNITS => self.units,
            _ => unreachable!("LineSpacing has no integer at {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::NCONDS => self.fnconds = value,
            prop::NPHASES => self.nphases = value,
            prop::UNITS => self.units = value,
            _ => unreachable!("LineSpacing has no integer at {idx}"),
        }
    }

    fn get_bool(&self, idx: usize) -> bool {
        match idx {
            prop::DETAILED => self.detailed,
            _ => unreachable!("LineSpacing has no boolean at {idx}"),
        }
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        match idx {
            prop::DETAILED => self.detailed = value,
            _ => unreachable!("LineSpacing has no boolean at {idx}"),
        }
    }

    fn get_f64(&self, idx: usize) -> f64 {
        match idx {
            prop::EQDISTPHPH => self.eq_dist_ph_ph,
            prop::EQDISTPHN => self.eq_dist_ph_n,
            prop::AVGPHASEHEIGHT => self.avg_phase_height,
            prop::AVGNEUTRALHEIGHT => self.avg_neutral_height,
            _ => unreachable!("LineSpacing has no double at {idx}"),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            prop::EQDISTPHPH => self.eq_dist_ph_ph = value,
            prop::EQDISTPHN => self.eq_dist_ph_n = value,
            prop::AVGPHASEHEIGHT => self.avg_phase_height = value,
            prop::AVGNEUTRALHEIGHT => self.avg_neutral_height = value,
            _ => unreachable!("LineSpacing has no double at {idx}"),
        }
    }

    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        let arr = match idx {
            prop::X => &self.fx,
            prop::H => &self.fy,
            _ => unreachable!("LineSpacing has no double array at {idx}"),
        };
        // A list with no coordinate given reads `''`.
        if arr.is_empty() {
            None
        } else {
            Some(arr.as_slice())
        }
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        match idx {
            prop::X => self.fx = value,
            prop::H => self.fy = value,
            _ => unreachable!("LineSpacing has no double array at {idx}"),
        }
    }

    /// `X` and `H` each take at most `nconds` values.
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::X | prop::H => self.fnconds.max(0) as usize,
            _ => unreachable!("LineSpacing has no function-sized array at {idx}"),
        }
    }

    /// `X` and `H` hold no value when no coordinate was given. Pinned by
    /// `exec::tests::line_geometry_rules::a_short_spacing_list_keeps_its_gaps`.
    fn holds_no_value(&self, idx: usize) -> bool {
        match idx {
            prop::X => self.fx.is_empty(),
            prop::H => self.fy.is_empty(),
            _ => false,
        }
    }

    /// Before the first `nconds=` every property but `nconds=` and `like=` is
    /// refused, naming it. Pinned by
    /// `exec::tests::line_geometry_rules::a_spacing_property_before_nconds_is_refused`.
    fn refuse_property(&self, idx: usize, prop_name: &str) -> Option<String> {
        (!self.nconds_given && idx != prop::NCONDS && idx != LIKE).then(|| {
            format!(
                "LineSpacing.{}.{prop_name}: nconds= is not given yet, so the value is not \
                 applied. Give nconds= first.",
                self.data.name()
            )
        })
    }

    /// A spacing whose `nconds=` was never given exports its name only, since
    /// the import refuses every other property. Pinned by
    /// `exec::tests::line_geometry_rules::a_spacing_property_before_nconds_is_refused`.
    fn json_omits(&self, _idx: usize) -> bool {
        !self.nconds_given
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            prop::NCONDS => self.start_over(),
            // dss_capi 0.15.x `Detailed` side effect: use `Detailed` to clear the
            // property-tracking of the now-unused set, so Save/Dump only emit the
            // active model. (The upstream `NoPropertyTracking` compat flag is
            // off by default; the port always tracks.)
            prop::DETAILED => {
                if self.detailed {
                    // Detailed distances: clear the equivalent data.
                    self.data.clear_seq(prop::EQDISTPHPH);
                    self.data.clear_seq(prop::EQDISTPHN);
                    self.data.clear_seq(prop::AVGPHASEHEIGHT);
                    self.data.clear_seq(prop::AVGNEUTRALHEIGHT);
                } else {
                    // Equivalent distances: clear X and H.
                    self.data.clear_seq(prop::X);
                    self.data.clear_seq(prop::H);
                }
            }
            _ => {}
        }
    }
}
