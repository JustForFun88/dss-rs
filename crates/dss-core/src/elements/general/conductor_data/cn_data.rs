//! `CNData` (`TCNDataObj`): concentric-neutral cable — own props (k/DiaStrand/
//! GMRStrand/RStrand), then the CableData block, then ConductorData.

use super::*;

define_properties! {
    class "CNData", abbrev true, enums enums;
    1  K            => PropDef::integer("k");
    2  DIASTRAND    => PropDef::double("DiaStrand")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    3  GMRSTRAND    => PropDef::double("GMRStrand")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    4  RSTRAND      => PropDef::double("RStrand")
        .flags(PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM_PER_LENGTH);
    // dss_capi 0.15.x new prop (CNData.pas `SemiconLayer = 5`): selects the CN
    // capacitance formula. Default `true` = the classic `ln(RadOut/RadIn)`,
    // preserving 0.14.5 numerics (UPGRADE_PLAN.md WP-U1.4).
    5  SEMICONLAYER => PropDef::boolean("SemiconLayer");
    6  EPSR         => PropDef::double("EpsR");
    7  INSLAYER     => PropDef::double("InsLayer")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    8  DIAINS       => PropDef::double("DiaIns")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    9  DIACABLE     => PropDef::double("DiaCable")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    10 RDC          => PropDef::double("RDC")
        .flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_OHM_PER_LENGTH);
    11 RAC          => PropDef::double("RAC").flags(PropFlags::DYNAMIC_DEFAULT);
    12 RUNITS       => PropDef::mapped_string_enum("RUnits", enums.units);
    13 GMRAC        => PropDef::double("GMRAC")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    14 GMRUNITS     => PropDef::mapped_string_enum("GMRUnits", enums.units);
    15 RADIUS       => PropDef::double("Radius")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    16 RADUNITS     => PropDef::mapped_string_enum("RadUnits", enums.units);
    17 NORMAMPS     => PropDef::double("NormAmps").flags(PropFlags::DYNAMIC_DEFAULT);
    18 EMERGAMPS    => PropDef::double("EmergAmps").flags(PropFlags::DYNAMIC_DEFAULT);
    19 DIAM         => PropDef::double("Diam").scale(0.5)
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::REDUNDANT);
    20 SEASONS      => PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON);
    21 RATINGS      => PropDef::double_array("Ratings", SEASONS);
    22 CAPRADIUS    => PropDef::double("CapRadius")
        .flags(PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
}

// Global ordinal → relative-block offsets.
const CABLE_OFFSET: usize = 5; // EpsR..DiaCable at global 6..9
const COND_OFFSET: usize = 9; // Rdc..Capradius at global 10..22

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
    /// dss_capi 0.15.x `semiconLayer` (LongBool, default `true`).
    fsemicon_layer: bool,
}

impl CnDataObj {
    pub fn new(name: impl Into<String>) -> Self {
        // Pascal `TCNDataObj.Create`.
        Self {
            data: DssObjData::new(name.into().to_ascii_lowercase(), prop::NUM_PROPS),
            cond: ConductorDataCore::new(),
            cable: CableDataCore::new(),
            fk_strand: 2,
            fdia_strand: -1.0,
            fgmr_strand: -1.0,
            fr_strand: -1.0,
            fsemicon_layer: true,
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
            semicon_layer: self.fsemicon_layer,
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
    fn get_bool(&self, idx: usize) -> bool {
        debug_assert_eq!(idx, prop::SEMICONLAYER);
        self.fsemicon_layer
    }
    fn set_bool(&mut self, idx: usize, value: bool) {
        debug_assert_eq!(idx, prop::SEMICONLAYER);
        self.fsemicon_layer = value;
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
            // RStrand / SemiconLayer (own props without a side effect) are a no-op.
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
            self.fsemicon_layer = o.fsemicon_layer;
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
