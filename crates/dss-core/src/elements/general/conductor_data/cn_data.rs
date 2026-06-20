//! `CNData` (`TCNDataObj`): concentric-neutral cable — own props (k/DiaStrand/
//! GMRStrand/RStrand), then the CableData block, then ConductorData.

use super::*;

define_properties! {
    class "CNData", abbrev true, enums enums;
    1  K         => PropDef::integer("k");
    2  DIASTRAND => PropDef::double("DiaStrand")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    3  GMRSTRAND => PropDef::double("GMRStrand")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    4  RSTRAND   => PropDef::double("RStrand")
        .flags(PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM_PER_LENGTH);
    5  EPSR      => PropDef::double("EpsR");
    6  INSLAYER  => PropDef::double("InsLayer")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    7  DIAINS    => PropDef::double("DiaIns")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    8  DIACABLE  => PropDef::double("DiaCable")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    9  RDC       => PropDef::double("Rdc")
        .flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_OHM_PER_LENGTH);
    10 RAC       => PropDef::double("Rac").flags(PropFlags::DYNAMIC_DEFAULT);
    11 RUNITS    => PropDef::mapped_string_enum("Runits", enums.units);
    12 GMRAC     => PropDef::double("GMRac")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    13 GMRUNITS  => PropDef::mapped_string_enum("GMRunits", enums.units);
    14 RADIUS    => PropDef::double("radius")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    15 RADUNITS  => PropDef::mapped_string_enum("radunits", enums.units);
    16 NORMAMPS  => PropDef::double("normamps").flags(PropFlags::DYNAMIC_DEFAULT);
    17 EMERGAMPS => PropDef::double("emergamps").flags(PropFlags::DYNAMIC_DEFAULT);
    18 DIAM      => PropDef::double("diam").scale(0.5)
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::REDUNDANT);
    19 SEASONS   => PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON);
    20 RATINGS   => PropDef::double_array("Ratings", SEASONS);
    21 CAPRADIUS => PropDef::double("Capradius")
        .flags(PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
}

// Global ordinal → relative-block offsets.
const CABLE_OFFSET: usize = 4; // EpsR..DiaCable at global 5..8
const COND_OFFSET: usize = 8; // Rdc..Capradius at global 9..21

/// `TCNDataObj`.
#[derive(Debug, Clone)]
pub struct CnDataObj {
    data: DssObjData,
    cond: ConductorDataCore,
    cable: CableDataCore,
    fk_strand: i32,
    fdia_strand: f64,
    fgmr_strand: f64,
    fr_strand: f64,
}

impl CnDataObj {
    pub fn new(name: impl Into<String>) -> Self {
        // Pascal `TCNDataObj.Create`.
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            cond: ConductorDataCore::new(),
            cable: CableDataCore::new(),
            fk_strand: 2,
            fdia_strand: -1.0,
            fgmr_strand: -1.0,
            fr_strand: -1.0,
        }
    }

    /// `(NormAmps, EmergAmps, NumAmpRatings, AmpRatings)` (see
    /// [`super::wire_data::WireDataObj::amps`]).
    pub fn amps(&self) -> (f64, f64, i32, &[f64]) {
        self.cond.amps()
    }

    /// The engine geometry inputs, including the CN strand/insulation extras.
    pub fn geom(&self) -> ConductorGeom {
        let mut g = self.cond.geom_common();
        g.cable = Some(CableGeom::Cn {
            eps_r: self.cable.feps_r,
            ins_layer: self.cable.fins_layer,
            dia_ins: self.cable.fdia_ins,
            dia_cable: self.cable.fdia_cable,
            k_strand: self.fk_strand,
            dia_strand: self.fdia_strand,
            gmr_strand: self.fgmr_strand,
            r_strand: self.fr_strand,
        });
        g
    }
}

impl DssObject for CnDataObj {
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
        match idx {
            prop::DIASTRAND => self.fdia_strand,
            prop::GMRSTRAND => self.fgmr_strand,
            prop::RSTRAND => self.fr_strand,
            prop::EPSR..=prop::DIACABLE => self.cable.get_f64(idx - CABLE_OFFSET),
            _ => self.cond.get_f64(idx - COND_OFFSET),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            prop::DIASTRAND => self.fdia_strand = value,
            prop::GMRSTRAND => self.fgmr_strand = value,
            prop::RSTRAND => self.fr_strand = value,
            prop::EPSR..=prop::DIACABLE => self.cable.set_f64(idx - CABLE_OFFSET, value),
            _ => self.cond.set_f64(idx - COND_OFFSET, value),
        }
    }
    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            prop::K => self.fk_strand,
            _ => self.cond.get_i32(idx - COND_OFFSET),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            prop::K => self.fk_strand = value,
            _ => self.cond.set_i32(idx - COND_OFFSET, value),
        }
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
        let name = self.data.name().to_string();
        let mut errs = Vec::new();
        match idx {
            // CN's own props (Pascal `TCNDataObj.PropertySideEffects`).
            prop::DIASTRAND => {
                if self.fgmr_strand <= 0.0 {
                    self.fgmr_strand = 0.7788 * 0.5 * self.fdia_strand;
                }
                if self.fdia_strand <= 0.0 {
                    errs.push(format!(
                        "Error: Neutral strand diameter must be positive for CNData {name}"
                    ));
                }
            }
            prop::K => {
                if self.fk_strand < 2 {
                    errs.push(format!(
                        "Error: Must have at least 2 concentric neutral strands for CNData {name}"
                    ));
                }
            }
            prop::GMRSTRAND => {
                if self.fgmr_strand <= 0.0 {
                    errs.push(format!(
                        "Error: Neutral strand GMR must be positive for CNData {name}"
                    ));
                }
            }
            prop::EPSR..=prop::DIACABLE => {
                self.cable
                    .side_effects(idx - CABLE_OFFSET, &name, &mut errs);
            }
            prop::RDC..=prop::CAPRADIUS => {
                let full = format!("CNData.{name}");
                self.cond.side_effects(idx - COND_OFFSET, &full, &mut errs);
            }
            // RStrand (and any own prop without a side effect) is a no-op.
            _ => {}
        }
        for e in errs {
            self.data.push_error(e);
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        if let Some(o) = other.as_any().downcast_ref::<CnDataObj>() {
            self.cond.make_like_from(&o.cond);
            self.cable.make_like_from(&o.cable);
            self.fk_strand = o.fk_strand;
            self.fdia_strand = o.fdia_strand;
            self.fgmr_strand = o.fgmr_strand;
            self.fr_strand = o.fr_strand;
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
