//! `TSData` (`TTSDataObj`): tape-shield cable — own props (DiaShield/TapeLayer/
//! TapeLap), then the CableData block, then ConductorData.

use super::*;

define_properties! {
    class "TSData", abbrev true, enums enums;
    1  DIASHIELD => PropDef::double("DiaShield")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    2  TAPELAYER => PropDef::double("TapeLayer")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    3  TAPELAP   => PropDef::double("TapeLap")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NO_DEFAULT);
    4  EPSR      => PropDef::double("EpsR");
    5  INSLAYER  => PropDef::double("InsLayer")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    6  DIAINS    => PropDef::double("DiaIns")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    7  DIACABLE  => PropDef::double("DiaCable")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::NO_DEFAULT);
    8  RDC       => PropDef::double("RDC")
        .flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_OHM_PER_LENGTH);
    9  RAC       => PropDef::double("RAC").flags(PropFlags::DYNAMIC_DEFAULT);
    10 RUNITS    => PropDef::mapped_string_enum("RUnits", enums.units);
    11 GMRAC     => PropDef::double("GMRAC")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    12 GMRUNITS  => PropDef::mapped_string_enum("GMRUnits", enums.units);
    13 RADIUS    => PropDef::double("Radius")
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
    14 RADUNITS  => PropDef::mapped_string_enum("RadUnits", enums.units);
    15 NORMAMPS  => PropDef::double("NormAmps").flags(PropFlags::DYNAMIC_DEFAULT);
    16 EMERGAMPS => PropDef::double("EmergAmps").flags(PropFlags::DYNAMIC_DEFAULT);
    17 DIAM      => PropDef::double("Diam").scale(0.5)
        .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::REDUNDANT);
    18 SEASONS   => PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON);
    19 RATINGS   => PropDef::double_array("Ratings", SEASONS);
    20 CAPRADIUS => PropDef::double("CapRadius")
        .flags(PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
}

const CABLE_OFFSET: usize = 3; // EpsR..DiaCable at global 4..7
const COND_OFFSET: usize = 7; // Rdc..Capradius at global 8..20

/// `TTSDataObj`.
#[derive(Debug, Clone)]
pub struct TsDataObj {
    data: DssObjData,
    cond: ConductorDataCore,
    cable: CableDataCore,
    fdia_shield: f64,
    ftape_layer: f64,
    ftape_lap: f64,
}

impl TsDataObj {
    pub fn new(name: impl Into<String>) -> Self {
        // Pascal `TTSDataObj.Create`.
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            cond: ConductorDataCore::new(),
            cable: CableDataCore::new(),
            fdia_shield: -1.0,
            ftape_layer: -1.0,
            ftape_lap: 20.0,
        }
    }

    /// `(NormAmps, EmergAmps, NumAmpRatings, AmpRatings)` (see
    /// [`super::wire_data::WireDataObj::amps`]).
    pub fn amps(&self) -> (f64, f64, i32, &[f64]) {
        self.cond.amps()
    }

    /// The engine geometry inputs, including the TS shield/tape extras.
    pub fn geom(&self) -> ConductorGeom {
        let mut g = self.cond.geom_common();
        g.cable = Some(CableGeom::Ts {
            eps_r: self.cable.feps_r,
            ins_layer: self.cable.fins_layer,
            dia_ins: self.cable.fdia_ins,
            dia_cable: self.cable.fdia_cable,
            dia_shield: self.fdia_shield,
            tape_layer: self.ftape_layer,
            tape_lap: self.ftape_lap,
        });
        g
    }
}

impl DssObject for TsDataObj {
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
            prop::DIASHIELD => self.fdia_shield,
            prop::TAPELAYER => self.ftape_layer,
            prop::TAPELAP => self.ftape_lap,
            prop::EPSR..=prop::DIACABLE => self.cable.get_f64(idx - CABLE_OFFSET),
            _ => self.cond.get_f64(idx - COND_OFFSET),
        }
    }
    fn set_f64(&mut self, idx: usize, value: f64) {
        match idx {
            prop::DIASHIELD => self.fdia_shield = value,
            prop::TAPELAYER => self.ftape_layer = value,
            prop::TAPELAP => self.ftape_lap = value,
            prop::EPSR..=prop::DIACABLE => self.cable.set_f64(idx - CABLE_OFFSET, value),
            _ => self.cond.set_f64(idx - COND_OFFSET, value),
        }
    }
    fn get_i32(&self, idx: usize) -> i32 {
        self.cond.get_i32(idx - COND_OFFSET)
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        self.cond.set_i32(idx - COND_OFFSET, value);
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
            // TS's own props (Pascal `TTSDataObj.PropertySideEffects`).
            prop::DIASHIELD => {
                if self.fdia_shield <= 0.0 {
                    errs.push(format!(
                        "Error: Diameter over shield must be positive for TapeShieldData {name}"
                    ));
                }
            }
            prop::TAPELAYER => {
                if self.ftape_layer <= 0.0 {
                    errs.push(format!(
                        "Error: Tape shield thickness must be positive for TapeShieldData {name}"
                    ));
                }
            }
            prop::TAPELAP => {
                if !(0.0..=100.0).contains(&self.ftape_lap) {
                    errs.push(format!(
                        "Error: Tap lap must range from 0 to 100 for TapeShieldData {name}"
                    ));
                }
            }
            prop::EPSR..=prop::DIACABLE => {
                self.cable
                    .side_effects(idx - CABLE_OFFSET, &name, &mut errs);
            }
            prop::RDC..=prop::CAPRADIUS => {
                let full = format!("TSData.{name}");
                self.cond.side_effects(idx - COND_OFFSET, &full, &mut errs);
            }
            _ => {}
        }
        for e in errs {
            self.data.push_error(e);
        }
    }

    fn make_like(&mut self, other: &dyn DssObject) {
        self.data.copy_prp_sequence_from(other.data());
        if let Some(o) = other.as_any().downcast_ref::<TsDataObj>() {
            self.cond.make_like_from(&o.cond);
            self.cable.make_like_from(&o.cable);
            self.fdia_shield = o.fdia_shield;
            self.ftape_layer = o.ftape_layer;
            self.ftape_lap = o.ftape_lap;
        }
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}
