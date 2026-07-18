//! `LineSpacing` (`TLineSpacingObj`) — overhead-line conductor spacing geometry.
//! Port of Pascal `General/LineSpacing.pas`.
//!
//! A `DSS_OBJECT` catalog class (no terminals, no YPrim): a `LineGeometry`
//! references it by name and reads the per-conductor horizontal/vertical
//! coordinate arrays (`X`/`H`, in `Units`) plus `NConds`/`NPhases` to build the
//! Carson `LineConstants` matrices. The two coordinate arrays are
//! `DoubleVArrayProperty` with the element count taken from `FNConds`
//! (`PropertyOffset2 = @FNConds`), so they are sized — and re-sized — by the
//! `nconds` property's side effect.

#[cfg(test)]
mod tests;

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};

/// Pascal `LineUnits.UNITS_FT` — the `ft` ordinal of `DSS.UnitsEnum`; the
/// default unit and the value the `nconds` side effect resets `Units` to.
const UNITS_FT: i32 = 5;

define_properties! {
    class "LineSpacing", abbrev true, enums enums;
    1 NCONDS  => PropDef::integer("NConds").flags(PropFlags::SUPPRESS_JSON);
    2 NPHASES => PropDef::integer("NPhases");
    3 X       => PropDef::double_v_array("X").size_prop(NCONDS);
    4 H       => PropDef::double_v_array("H").size_prop(NCONDS);
    5 UNITS   => PropDef::mapped_string_enum("Units", enums.units);
    // dss_capi 0.15.x additions (LineSpacing.pas): the equivalent-spacing model.
    // `Detailed` (default true) selects per-conductor coordinates; when false the
    // four equivalent distances replace them. UPGRADE_PLAN.md WP-U1.4 rows B3/C1.
    6 DETAILED          => PropDef::boolean("Detailed");
    7 EQDISTPHPH        => PropDef::double("EqDistPhPh");
    8 EQDISTPHN         => PropDef::double("EqDistPhN");
    9 AVGPHASEHEIGHT    => PropDef::double("AvgPhaseHeight");
    10 AVGNEUTRALHEIGHT => PropDef::double("AvgNeutralHeight");
}

/// `TLineSpacingObj`. Pascal stores `FX`/`FY` as 1-based `pDoubleArray`s of
/// length `FNConds`; here they are plain `Vec<f64>` kept at `fnconds` elements.
#[derive(Debug, Clone)]
pub struct LineSpacingObj {
    data: DssObjData,
    fx: Vec<f64>,
    fy: Vec<f64>,
    fnconds: i32,
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
    pub fn new(name: impl Into<String>) -> Self {
        // Pascal `TLineSpacingObj.Create`: FNConds := 3, then the `nconds`
        // side effect sizes FX/FY to 3 (and sets Units := ft); the arrays are
        // then zeroed and NPhases := 3.
        let mut obj = Self {
            data: DssObjData::new(name.into().to_ascii_lowercase(), prop::NUM_PROPS),
            fx: Vec::new(),
            fy: Vec::new(),
            fnconds: 3,
            nphases: 3,
            units: UNITS_FT,
            // Pascal `Create`: eqDist*/avg* := 0.0, detailed := true.
            detailed: true,
            eq_dist_ph_ph: 0.0,
            eq_dist_ph_n: 0.0,
            avg_phase_height: 0.0,
            avg_neutral_height: 0.0,
        };
        obj.realloc_conductors();
        obj
    }

    /// Pascal `EquivalentSpacing()` = `not detailed`: `TLineGeometryObj` reads
    /// this (plus the four equivalent distances) when building the Carson matrices.
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

    /// Pascal `NWires` (= `FNConds`): the conductor count a `LineGeometry`
    /// matches against when it reads this spacing's coordinates.
    pub fn nwires(&self) -> i32 {
        self.fnconds
    }
    /// Pascal `NPhases` (= `FNphases`): the phase count `TLineObj.FetchLineSpacing`
    /// adopts and `SetWires` uses to place buried neutrals.
    pub fn nphases(&self) -> i32 {
        self.nphases
    }
    /// Pascal `Xcoord`/`Ycoord` arrays (length `FNConds`) and `Units` — read by
    /// `TLineGeometryObj`'s `spacing=` side effect.
    pub fn xcoord(&self) -> &[f64] {
        &self.fx
    }
    pub fn ycoord(&self) -> &[f64] {
        &self.fy
    }
    pub fn spacing_units(&self) -> i32 {
        self.units
    }

    /// Pascal `nconds` `PropertySideEffects`: `ReAllocmem(FX/FY, FNConds)`.
    /// Pascal leaves grown entries uninitialized; we zero-fill the tail (the
    /// preserved leading entries match, and undefined upstream memory is not a
    /// behaviour the goldens pin). Shrinking truncates, as Pascal does.
    fn realloc_conductors(&mut self) {
        let n = self.fnconds.max(0) as usize;
        self.fx.resize(n, 0.0);
        self.fy.resize(n, 0.0);
    }
}

impl DssObject for LineSpacingObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
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
        // Pascal `ReAllocmem(FX, 0)` (the `nconds=0` side effect) frees the
        // buffer and leaves the pointer nil, so `GetDSSArray` returns the empty
        // string `''` — not `'[]'`. Mirror that: an empty coordinate array reads
        // as nil. (For `nconds < 0` the Pascal realloc raises an exception; we
        // clamp the size to 0, so that degenerate path likewise reads `''` and
        // is not oracle-pinnable.)
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

    /// Both `X` and `H` are sized by `FNConds` (`PropertyOffset2 = @FNConds`).
    fn array_size(&self, idx: usize) -> usize {
        match idx {
            prop::X | prop::H => self.fnconds.max(0) as usize,
            _ => unreachable!("LineSpacing has no function-sized array at {idx}"),
        }
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        // Pascal `TLineSpacingObj.PropertySideEffects`: `nconds` resizes the
        // coordinate arrays and resets the unit to feet.
        match idx {
            prop::NCONDS => {
                self.realloc_conductors();
                self.units = UNITS_FT;
            }
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

    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        if let Some(o) = other.as_any().downcast_ref::<LineSpacingObj>() {
            // Pascal `MakeLike`: copy FNConds, run the `nconds` side effect
            // (resize + Units := ft), copy NPhases, then the X/Y arrays, and
            // finally `Units := Other.Units` (overriding the side effect).
            self.fnconds = o.fnconds;
            self.realloc_conductors();
            self.nphases = o.nphases;
            let n = self.fnconds.max(0) as usize;
            self.fx[..n].copy_from_slice(&o.fx[..n]);
            self.fy[..n].copy_from_slice(&o.fy[..n]);
            self.units = o.units;
            // TODO(compat): dss_capi 0.15.x `TLineSpacingObj.MakeLike` does NOT
            // copy `detailed`/`eqDistPhPh`/`eqDistPhN`/`avgPhaseHeight`/
            // `avgNeutralHeight` (only NConds/NPhases/FX/FY/Units), so a `like=`
            // spacing keeps its `Create` defaults for the equivalent-spacing
            // fields while its PrpSequence (copied by the base) may still mark
            // them set. Reproduced 1:1; the clean fix copies them post-port.
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
