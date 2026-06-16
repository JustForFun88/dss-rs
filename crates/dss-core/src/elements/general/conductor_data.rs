//! Conductor catalog classes — `WireData`, `CNData`, `TSData`.
//! Port of Pascal `General/{ConductorData,WireData,CNData,TSData,CableData}.pas`.
//!
//! These are `DSS_OBJECT` catalog classes (no terminals, no YPrim): a
//! `LineGeometry` references them by name and copies their per-conductor data
//! (Rdc/Rac, GMR, radius, cable geometry) into the `LineConstants` engine
//! (`support/line_constants`). The Pascal type hierarchy is
//! `TConductorDataObj` → `TWireDataObj`, and `TConductorDataObj` →
//! `TCableDataObj` → `TCNDataObj`/`TTSDataObj`. Rust has no inheritance, so the
//! shared field+side-effect blocks live in [`ConductorDataCore`] (the
//! `TConductorData` properties) and [`CableDataCore`] (the `TCableData`
//! properties); each concrete class embeds the cores it needs and maps its own
//! 1-based property ordinals onto the core blocks.
//!
//! Property *order* matches the oracle exactly (the leaf class's own props lead,
//! then `CableData`, then `ConductorData` — the Pascal `inherited
//! DefineProperties` chain), because the `props.json` dump is compared
//! name-by-name in that order.

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};

// `TConductorDataProp` ordinals, relative to the start of the ConductorData
// block (1-based; the same order Pascal's `PopulatePropertyNames` emits).
mod cd {
    pub const RDC: usize = 1;
    pub const RAC: usize = 2;
    pub const RUNITS: usize = 3;
    pub const GMRAC: usize = 4;
    pub const GMRUNITS: usize = 5;
    pub const RADIUS: usize = 6;
    pub const RADUNITS: usize = 7;
    pub const NORMAMPS: usize = 8;
    pub const EMERGAMPS: usize = 9;
    pub const DIAM: usize = 10;
    pub const SEASONS: usize = 11;
    // RATINGS (12) is handled by the leaf via `amp_ratings`, not the core.
    pub const CAPRADIUS: usize = 13;
}

// `TCableDataProp` ordinals, relative to the start of the CableData block.
mod cb {
    pub const EPSR: usize = 1;
    pub const INSLAYER: usize = 2;
    pub const DIAINS: usize = 3;
    pub const DIACABLE: usize = 4;
}

/// The shared `TConductorDataObj` fields and their side-effect logic. Indexed by
/// the ConductorData-relative ordinal ([`cd`]); each concrete class translates
/// its global ordinal into that relative form.
#[derive(Debug, Clone)]
struct ConductorDataCore {
    frdc: f64,
    fr60: f64,
    fgmr60: f64,
    fcapradius60: f64,
    fradius: f64,
    fgmr_units: i32,
    fresistance_units: i32,
    fradius_units: i32,
    norm_amps: f64,
    emerg_amps: f64,
    num_amp_ratings: i32,
    amp_ratings: Vec<f64>,
}

impl ConductorDataCore {
    fn new() -> Self {
        // Pascal `TConductorDataObj.Create`: every numeric "spec" field starts
        // at the -1.0 "not defined" sentinel; AmpRatings holds one entry equal
        // to the (still -1) NormAmps.
        Self {
            frdc: -1.0,
            fr60: -1.0,
            fgmr60: -1.0,
            fcapradius60: -1.0,
            fradius: -1.0,
            fgmr_units: 0,
            fresistance_units: 0,
            fradius_units: 0,
            norm_amps: -1.0,
            emerg_amps: -1.0,
            num_amp_ratings: 1,
            amp_ratings: vec![-1.0],
        }
    }

    fn get_f64(&self, rel: usize) -> f64 {
        match rel {
            cd::RDC => self.frdc,
            cd::RAC => self.fr60,
            cd::GMRAC => self.fgmr60,
            // `Radius` and `Diam` share the FRadius field; the engine applies
            // the diameter prop's 0.5 scale (Diam dumps FRadius / 0.5).
            cd::RADIUS | cd::DIAM => self.fradius,
            cd::NORMAMPS => self.norm_amps,
            cd::EMERGAMPS => self.emerg_amps,
            cd::CAPRADIUS => self.fcapradius60,
            _ => unreachable!("ConductorData has no double at rel {rel}"),
        }
    }

    fn set_f64(&mut self, rel: usize, value: f64) {
        match rel {
            cd::RDC => self.frdc = value,
            cd::RAC => self.fr60 = value,
            cd::GMRAC => self.fgmr60 = value,
            cd::RADIUS | cd::DIAM => self.fradius = value,
            cd::NORMAMPS => self.norm_amps = value,
            cd::EMERGAMPS => self.emerg_amps = value,
            cd::CAPRADIUS => self.fcapradius60 = value,
            _ => unreachable!("ConductorData has no double at rel {rel}"),
        }
    }

    fn get_i32(&self, rel: usize) -> i32 {
        match rel {
            cd::RUNITS => self.fresistance_units,
            cd::GMRUNITS => self.fgmr_units,
            cd::RADUNITS => self.fradius_units,
            cd::SEASONS => self.num_amp_ratings,
            _ => unreachable!("ConductorData has no integer at rel {rel}"),
        }
    }

    fn set_i32(&mut self, rel: usize, value: i32) {
        match rel {
            cd::RUNITS => self.fresistance_units = value,
            cd::GMRUNITS => self.fgmr_units = value,
            cd::RADUNITS => self.fradius_units = value,
            cd::SEASONS => self.num_amp_ratings = value,
            _ => unreachable!("ConductorData has no integer at rel {rel}"),
        }
    }

    /// `TConductorDataObj.PropertySideEffects` (relative ordinal). `full_name` is
    /// the object's `Class.name` (used by the radius-zero message).
    fn side_effects(&mut self, rel: usize, full_name: &str, errors: &mut Vec<String>) {
        match rel {
            cd::RDC => {
                if self.fr60 < 0.0 {
                    self.fr60 = 1.02 * self.frdc;
                }
            }
            cd::RAC => {
                if self.frdc < 0.0 {
                    self.frdc = self.fr60 / 1.02;
                }
            }
            cd::GMRAC => {
                if self.fradius < 0.0 {
                    self.fradius = self.fgmr60 / 0.7788;
                }
                if self.fradius == 0.0 {
                    errors.push(format!(
                        "Error: Radius is specified as zero for {full_name}"
                    ));
                }
            }
            cd::GMRUNITS => {
                if self.fradius_units == 0 {
                    self.fradius_units = self.fgmr_units;
                }
            }
            cd::RADIUS | cd::DIAM => {
                if self.fgmr60 < 0.0 {
                    self.fgmr60 = 0.7788 * self.fradius;
                }
                if self.fcapradius60 < 0.0 {
                    self.fcapradius60 = self.fradius; // default to radius
                }
            }
            cd::RADUNITS => {
                if self.fgmr_units == 0 {
                    self.fgmr_units = self.fradius_units;
                }
            }
            cd::NORMAMPS => {
                if self.emerg_amps < 0.0 {
                    self.emerg_amps = 1.5 * self.norm_amps;
                }
            }
            cd::EMERGAMPS => {
                if self.norm_amps < 0.0 {
                    self.norm_amps = self.emerg_amps / 1.5;
                }
            }
            cd::SEASONS => self
                .amp_ratings
                .resize(self.num_amp_ratings.max(0) as usize, 0.0),
            _ => {}
        }
    }

    /// `TConductorDataObj.MakeLike`. Note Pascal copies neither `NumAmpRatings`
    /// nor `AmpRatings`, so a `like=` object keeps its own default ratings.
    fn make_like_from(&mut self, o: &Self) {
        self.frdc = o.frdc;
        self.fr60 = o.fr60;
        self.fresistance_units = o.fresistance_units;
        self.fgmr60 = o.fgmr60;
        self.fcapradius60 = o.fcapradius60;
        self.fgmr_units = o.fgmr_units;
        self.fradius = o.fradius;
        self.fradius_units = o.fradius_units;
        self.norm_amps = o.norm_amps;
        self.emerg_amps = o.emerg_amps;
    }
}

/// The shared `TCableDataObj` fields (the `TCableData` props) and their
/// side-effect error checks. Indexed by the CableData-relative ordinal ([`cb`]).
#[derive(Debug, Clone)]
struct CableDataCore {
    feps_r: f64,
    fins_layer: f64,
    fdia_ins: f64,
    fdia_cable: f64,
}

impl CableDataCore {
    fn new() -> Self {
        // Pascal `TCableDataObj.Create`.
        Self {
            feps_r: 2.3,
            fins_layer: -1.0,
            fdia_ins: -1.0,
            fdia_cable: -1.0,
        }
    }

    fn get_f64(&self, rel: usize) -> f64 {
        match rel {
            cb::EPSR => self.feps_r,
            cb::INSLAYER => self.fins_layer,
            cb::DIAINS => self.fdia_ins,
            cb::DIACABLE => self.fdia_cable,
            _ => unreachable!("CableData has no double at rel {rel}"),
        }
    }

    fn set_f64(&mut self, rel: usize, value: f64) {
        match rel {
            cb::EPSR => self.feps_r = value,
            cb::INSLAYER => self.fins_layer = value,
            cb::DIAINS => self.fdia_ins = value,
            cb::DIACABLE => self.fdia_cable = value,
            _ => unreachable!("CableData has no double at rel {rel}"),
        }
    }

    /// `TCableDataObj.PropertySideEffects` — critical-error checks only. The
    /// messages use the bare object `name` (Pascal `[Name]`).
    fn side_effects(&mut self, rel: usize, name: &str, errors: &mut Vec<String>) {
        match rel {
            cb::EPSR if self.feps_r < 1.0 => errors.push(format!(
                "Error: Insulation permittivity must be greater than one for CableData {name}"
            )),
            cb::INSLAYER if self.fins_layer <= 0.0 => errors.push(format!(
                "Error: Insulation layer thickness must be positive for CableData {name}"
            )),
            cb::DIAINS if self.fdia_ins <= 0.0 => errors.push(format!(
                "Error: Diameter over insulation layer must be positive for CableData {name}"
            )),
            cb::DIACABLE if self.fdia_cable <= 0.0 => errors.push(format!(
                "Error: Diameter over cable must be positive for CableData {name}"
            )),
            _ => {}
        }
    }

    fn make_like_from(&mut self, o: &Self) {
        self.feps_r = o.feps_r;
        self.fins_layer = o.fins_layer;
        self.fdia_ins = o.fdia_ins;
        self.fdia_cable = o.fdia_cable;
    }
}

/// `WireData` (`TWireDataObj`): an overhead conductor — purely the
/// `ConductorData` block (its own `NumPropsThisClass = 0`).
pub mod wire_data {
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
}

/// `CNData` (`TCNDataObj`): concentric-neutral cable — own props (k/DiaStrand/
/// GMRStrand/RStrand), then the CableData block, then ConductorData.
pub mod cn_data {
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
}

/// `TSData` (`TTSDataObj`): tape-shield cable — own props (DiaShield/TapeLayer/
/// TapeLap), then the CableData block, then ConductorData.
pub mod ts_data {
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
        8  RDC       => PropDef::double("Rdc")
            .flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_OHM_PER_LENGTH);
        9  RAC       => PropDef::double("Rac").flags(PropFlags::DYNAMIC_DEFAULT);
        10 RUNITS    => PropDef::mapped_string_enum("Runits", enums.units);
        11 GMRAC     => PropDef::double("GMRac")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
        12 GMRUNITS  => PropDef::mapped_string_enum("GMRunits", enums.units);
        13 RADIUS    => PropDef::double("radius")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::DYNAMIC_DEFAULT);
        14 RADUNITS  => PropDef::mapped_string_enum("radunits", enums.units);
        15 NORMAMPS  => PropDef::double("normamps").flags(PropFlags::DYNAMIC_DEFAULT);
        16 EMERGAMPS => PropDef::double("emergamps").flags(PropFlags::DYNAMIC_DEFAULT);
        17 DIAM      => PropDef::double("diam").scale(0.5)
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::REDUNDANT);
        18 SEASONS   => PropDef::integer("Seasons").flags(PropFlags::SUPPRESS_JSON);
        19 RATINGS   => PropDef::double_array("Ratings", SEASONS);
        20 CAPRADIUS => PropDef::double("Capradius")
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
}

pub use cn_data::CnDataObj;
pub use ts_data::TsDataObj;
pub use wire_data::WireDataObj;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::obj::dss_enum::EnumRegistry;
    use crate::obj::props::{ClassProps, PropEngine};
    use dss_parser::{Parser, ParserVars};

    /// Apply `edits` to `obj` through the property engine and return all
    /// errors (engine + deferred side-effect errors), like the executive does.
    fn apply(cls: &ClassProps, obj: &mut dyn DssObject, edits: &[(&str, &str)]) -> Vec<String> {
        let enums = EnumRegistry::new();
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = Vec::new();
        for (name, value) in edits {
            let idx = cls.property_index(name).expect("known property");
            let mut eng = PropEngine {
                parser: &mut parser,
                vars: &vars,
                enums: &enums,
                errors: &mut errors,
                foreign: None,
            };
            cls.edit_property(obj, idx, value, &mut eng).unwrap();
        }
        obj.end_edit();
        errors.extend(obj.data_mut().take_errors());
        errors
    }

    fn get(cls: &ClassProps, obj: &dyn DssObject, name: &str) -> String {
        let enums = EnumRegistry::new();
        let idx = cls.property_index(name).unwrap();
        cls.get_value(obj, idx, &enums)
    }

    /// Numeric value of a scalar property (our dump prints full f64 precision;
    /// the oracle uses `%g`, so derived values are compared numerically — the
    /// `props.json` gate already pins the exact oracle strings within tol).
    fn getf(cls: &ClassProps, obj: &dyn DssObject, name: &str) -> f64 {
        get(cls, obj, name).parse().unwrap()
    }

    // ----- WireData (ConductorData defaults / couplings) --------------------

    #[test]
    fn wiredata_diam_and_dynamic_defaults() {
        // Oracle (dss-python 0.15.7): diam sets radius (scale 0.5); GMR and
        // CapRadius default from radius; Rac defaults from Rdc; radunits seeds
        // GMRunits.
        let enums = EnumRegistry::new();
        let cls = wire_data::class_props(&enums);
        let mut obj = WireDataObj::new("wd");
        let errs = apply(
            &cls,
            &mut obj,
            &[
                ("rdc", "0.05"),
                ("diam", "0.1"),
                ("runits", "ft"),
                ("radunits", "ft"),
            ],
        );
        assert!(errs.is_empty(), "{errs:?}");
        assert!((getf(&cls, &obj, "rac") - 0.051).abs() < 1e-9);
        assert!((getf(&cls, &obj, "radius") - 0.05).abs() < 1e-12);
        assert!((getf(&cls, &obj, "gmrac") - 0.7788 * 0.05).abs() < 1e-12);
        assert!((getf(&cls, &obj, "capradius") - 0.05).abs() < 1e-12);
        assert!((getf(&cls, &obj, "diam") - 0.1).abs() < 1e-12);
        assert_eq!(get(&cls, &obj, "gmrunits"), "ft"); // seeded from radunits
    }

    #[test]
    fn wiredata_gmr_seeds_radius_and_emergamps() {
        // GMRac seeds radius (GMR/0.7788); GMRunits seeds radunits; normamps
        // seeds emergamps (×1.5).
        let enums = EnumRegistry::new();
        let cls = wire_data::class_props(&enums);
        let mut obj = WireDataObj::new("wg");
        let errs = apply(
            &cls,
            &mut obj,
            &[
                ("rdc", "0.04"),
                ("gmrac", "0.02"),
                ("gmrunits", "ft"),
                ("normamps", "400"),
            ],
        );
        assert!(errs.is_empty(), "{errs:?}");
        assert_eq!(get(&cls, &obj, "radunits"), "ft"); // seeded from gmrunits
        assert_eq!(get(&cls, &obj, "emergamps"), "600");
        // radius = 0.02 / 0.7788
        let r: f64 = get(&cls, &obj, "radius").parse().unwrap();
        assert!((r - 0.02 / 0.7788).abs() < 1e-9, "radius {r}");
    }

    #[test]
    fn wiredata_makelike_does_not_copy_ratings() {
        // Pascal `TConductorDataObj.MakeLike` copies neither NumAmpRatings nor
        // AmpRatings, so a `like=` wire keeps its own default `[ -1]` / Seasons 1.
        let enums = EnumRegistry::new();
        let cls = wire_data::class_props(&enums);
        let mut src = WireDataObj::new("w1");
        apply(
            &cls,
            &mut src,
            &[
                ("rdc", "0.0526"),
                ("radius", "0.0306"),
                ("seasons", "2"),
                ("ratings", "600 800"),
            ],
        );
        assert_eq!(get(&cls, &src, "ratings"), "[ 600 800]");

        let mut dst = WireDataObj::new("w2");
        dst.make_like(&src);
        assert!((getf(&cls, &dst, "rdc") - 0.0526).abs() < 1e-12); // conductor data copied
        assert_eq!(get(&cls, &dst, "seasons"), "1"); // ratings NOT copied
        assert_eq!(get(&cls, &dst, "ratings"), "[ -1]");
    }

    // ----- CNData -----------------------------------------------------------

    #[test]
    fn cndata_strand_gmr_default() {
        // DiaStrand seeds GmrStrand = 0.7788 * 0.5 * DiaStrand when unset.
        let enums = EnumRegistry::new();
        let cls = cn_data::class_props(&enums);
        let mut obj = CnDataObj::new("cn");
        let errs = apply(&cls, &mut obj, &[("diastrand", "0.0641")]);
        assert!(errs.is_empty(), "{errs:?}");
        let g: f64 = get(&cls, &obj, "gmrstrand").parse().unwrap();
        assert!((g - 0.7788 * 0.5 * 0.0641).abs() < 1e-12, "gmrstrand {g}");
    }

    #[test]
    fn cndata_too_few_strands_errors() {
        let enums = EnumRegistry::new();
        let cls = cn_data::class_props(&enums);
        let mut obj = CnDataObj::new("cn");
        let errs = apply(&cls, &mut obj, &[("k", "1")]);
        assert!(
            errs.iter()
                .any(|e| e.contains("at least 2 concentric neutral strands")),
            "{errs:?}"
        );
    }

    #[test]
    fn cndata_low_permittivity_errors() {
        // EpsR has no NonNegative flag, so the side-effect <1.0 check is the
        // only guard (message uses the bare name).
        let enums = EnumRegistry::new();
        let cls = cn_data::class_props(&enums);
        let mut obj = CnDataObj::new("cn1");
        let errs = apply(&cls, &mut obj, &[("epsr", "0.5")]);
        assert!(
            errs.iter()
                .any(|e| e.contains("permittivity must be greater than one for CableData cn1")),
            "{errs:?}"
        );
    }

    // ----- TSData -----------------------------------------------------------

    #[test]
    fn tsdata_tapelap_out_of_range_errors() {
        let enums = EnumRegistry::new();
        let cls = ts_data::class_props(&enums);
        let mut obj = TsDataObj::new("ts1");
        let errs = apply(&cls, &mut obj, &[("tapelap", "150")]);
        assert!(
            errs.iter()
                .any(|e| e.contains("Tap lap must range from 0 to 100")),
            "{errs:?}"
        );
    }

    #[test]
    fn tsdata_defaults() {
        // TapeLap default 20; Rac from Rdc; GMR/CapRadius from radius.
        let enums = EnumRegistry::new();
        let cls = ts_data::class_props(&enums);
        let obj = TsDataObj::new("ts0");
        assert_eq!(get(&cls, &obj, "tapelap"), "20");
        assert_eq!(get(&cls, &obj, "epsr"), "2.3");
        assert_eq!(get(&cls, &obj, "diam"), "-2");
    }
}
