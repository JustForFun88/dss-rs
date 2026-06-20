//! `WireData` (`TWireDataObj`): an overhead conductor — purely the
//! `ConductorData` block (its own `NumPropsThisClass = 0`).

use super::*;

define_properties! {
    class "WireData", abbrev true, enums enums;
    1  RDC       => PropDef::double("Rdc")
        .flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_OHM_PER_LENGTH);
    2  RAC       => PropDef::double("Rac").flags(PropFlags::DYNAMIC_DEFAULT);
    3  RUNITS    => PropDef::mapped_string_enum("Runits", enums.units);
    4  GMRAC     => PropDef::double("GMRac")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    5  GMRUNITS  => PropDef::mapped_string_enum("GMRunits", enums.units);
    6  RADIUS    => PropDef::double("radius")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    7  RADUNITS  => PropDef::mapped_string_enum("radunits", enums.units);
    8  NORMAMPS  => PropDef::double("normamps").flags(PropFlags::DYNAMIC_DEFAULT);
    9  EMERGAMPS => PropDef::double("emergamps").flags(PropFlags::DYNAMIC_DEFAULT);
    10 DIAM      => PropDef::double("diam").scale(0.5)
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::REDUNDANT);
    11 SEASONS   => PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON);
    12 RATINGS   => PropDef::double_array("Ratings", SEASONS);
    13 CAPRADIUS => PropDef::double("Capradius")
        .flags(PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
}

/// `TWireDataObj`.
#[derive(Debug, Clone)]
pub struct WireDataObj {
    data: DssObjData,
    cond: ConductorDataCore,
}

impl WireDataObj {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            cond: ConductorDataCore::new(),
        }
    }

    /// `(NormAmps, EmergAmps, NumAmpRatings, AmpRatings)` — the rating fields a
    /// `LineGeometry` defaults from its first conductor.
    pub fn amps(&self) -> (f64, f64, i32, &[f64]) {
        self.cond.amps()
    }

    /// The engine geometry inputs (overhead: no cable extras).
    pub fn geom(&self) -> ConductorGeom {
        self.cond.geom_common()
    }
}

impl DssObject for WireDataObj {
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

    fn get_f64(&self, idx: usize) -> f64 {
        self.cond.get_f64(idx) // global ordinal == ConductorData-relative
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        self.cond.set_f64(idx, value);
    }
    fn get_i32(&self, idx: usize) -> i32 {
        self.cond.get_i32(idx)
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        self.cond.set_i32(idx, value);
    }
    fn get_f64_array(&self, idx: usize) -> Option<&[f64]> {
        debug_assert_eq!(idx, prop::RATINGS);
        (!self.cond.amp_ratings.is_empty()).then_some(self.cond.amp_ratings.as_slice())
    }
    fn set_f64_array(&mut self, idx: usize, value: Vec<f64>) {
        debug_assert_eq!(idx, prop::RATINGS);
        self.cond.amp_ratings = value;
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        let full = format!("WireData.{}", self.data.name());
        let mut errs = Vec::new();
        self.cond.side_effects(idx, &full, &mut errs);
        for e in errs {
            self.data.push_error(e);
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        if let Some(o) = other.as_any().downcast_ref::<WireDataObj>() {
            self.cond.make_like_from(&o.cond);
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
