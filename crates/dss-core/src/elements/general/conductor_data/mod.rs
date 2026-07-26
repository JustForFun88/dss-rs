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
//!
//! This module holds the shared cores and geometry types; each concrete class
//! lives in its own submodule ([`wire_data`], [`cn_data`], [`ts_data`]).

#[cfg(test)]
mod tests;

pub mod cn_data;
pub mod ts_data;
pub mod wire_data;

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};

/// The ordered target classes of the Line/LineGeometry `Conductors`
/// `TProxyClass` — Pascal `TProxyClass.Create(dssContext, ['WireData', 'CNData',
/// 'TSData'], True)` (`LineGeometry.pas:159`; `DSSClass.pas:2603`). Resolution
/// order and the `(WireData|CNData|TSData)` "Invalid class" label derive from
/// this list. Shared by both classes (upstream builds one proxy on
/// `LineGeometryClass` and `Line` reuses it via
/// `LineGeometryClass.ConductorProxyClass`).
pub const CONDUCTOR_PROXY_CLASSES: [&str; 3] = ["WireData", "CNData", "TSData"];

/// Pascal `TProxyClass.Name := 'Conductor'` (`LineGeometry.pas:160`) — the proxy
/// class name used in the "You must define the `Conductor` class …" diagnostic.
pub const CONDUCTOR_PROXY_NAME: &str = "Conductor";

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

    /// The current-rating fields a `LineGeometry` defaults from its first
    /// conductor (`NormAmps`/`EmergAmps`/`NumAmpRatings`/`AmpRatings`).
    fn amps(&self) -> (f64, f64, i32, &[f64]) {
        (
            self.norm_amps,
            self.emerg_amps,
            self.num_amp_ratings,
            &self.amp_ratings,
        )
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
    fn side_effects(&mut self, rel: usize, full_name: &str, errors: &mut crate::diag::ErrorLog) {
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
    fn side_effects(&mut self, rel: usize, name: &str, errors: &mut crate::diag::ErrorLog) {
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

/// The per-conductor data the `LineConstants` engine reads off a catalog object
/// in `TLineGeometryObj.UpdateLineGeometryData` (LineGeometry.pas:927-964), in
/// the object's own `LineUnits` codes — the engine converts to meters / per-meter
/// via its setters. Pascal reads each field directly from `FWireData[i]`.
pub struct ConductorGeom {
    pub radius: f64,
    pub cap_radius: f64,
    pub gmr: f64,
    pub rdc: f64,
    pub rac: f64,
    /// `RadiusUnits` — applies to `radius`, `cap_radius`, and (for cables) the
    /// insulation/strand diameters Pascal reads with `RadiusUnits`.
    pub radius_units: i32,
    /// `GMRUnits` — applies to `gmr` and (CN) `gmr_strand`.
    pub gmr_units: i32,
    /// `ResUnits` — applies to `rdc`/`rac` and (CN) `r_strand`.
    pub res_units: i32,
    /// The cable-specific extras, when the conductor is a `CNData`/`TSData`.
    pub cable: Option<CableGeom>,
}

/// The cable-class extras `UpdateLineGeometryData` copies into the
/// `TCNLineConstants`/`TTSLineConstants` engine.
pub enum CableGeom {
    /// `TCNDataObj` fields (LineGeometry.pas:947-960). dss_capi 0.15.x also
    /// copies the `semiconLayer` flag into the merged cable engine.
    Cn {
        eps_r: f64,
        ins_layer: f64,
        dia_ins: f64,
        dia_cable: f64,
        k_strand: i32,
        dia_strand: f64,
        gmr_strand: f64,
        r_strand: f64,
        semicon_layer: bool,
    },
    /// `TTSDataObj` fields (LineGeometry.pas:953-963).
    Ts {
        eps_r: f64,
        ins_layer: f64,
        dia_ins: f64,
        dia_cable: f64,
        dia_shield: f64,
        tape_layer: f64,
        tape_lap: f64,
    },
}

impl ConductorDataCore {
    /// The `TConductorData`-block portion of a [`ConductorGeom`] (the cable
    /// extras are filled by the concrete class).
    fn geom_common(&self) -> ConductorGeom {
        ConductorGeom {
            radius: self.fradius,
            cap_radius: self.fcapradius60,
            gmr: self.fgmr60,
            rdc: self.frdc,
            rac: self.fr60,
            radius_units: self.fradius_units,
            gmr_units: self.fgmr_units,
            res_units: self.fresistance_units,
            cable: None,
        }
    }
}

/// Which concrete catalog class a conductor object is — the behavior-trait
/// replacement for the `FWireData[i] is T…DataObj` dispatch chains (the
/// conductor-model choice, `Save`-array kind, cable-vs-wire rendering).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConductorKind {
    Wire,
    Cn,
    Ts,
}

/// The shared behavior of the three conductor catalog classes
/// (`WireData`/`CNData`/`TSData`, Pascal `TConductorDataObj` and its cable
/// subtree). Implemented by the three catalog classes and by [`ConductorObj`],
/// this replaces
/// the `FWireData[i] is T…DataObj` downcast dispatch (R0, Category C): the
/// LineGeometry/Line resolution reads `geom()`/`amps()`/`conductor_kind()`
/// through the trait instead of probing each concrete type.
pub trait ConductorData {
    /// The per-conductor geometry the `LineConstants` engine copies
    /// (`UpdateLineGeometryData`).
    fn geom(&self) -> ConductorGeom;
    /// `(NormAmps, EmergAmps, NumAmpRatings, AmpRatings)` — the current ratings
    /// a `LineGeometry`/`Line` defaults from its first conductor.
    fn amps(&self) -> (f64, f64, i32, &[f64]);
    /// The concrete catalog class.
    fn conductor_kind(&self) -> ConductorKind;
}

pub use cn_data::CnDataObj;
pub use ts_data::TsDataObj;
pub use wire_data::WireDataObj;

impl ConductorData for WireDataObj {
    fn geom(&self) -> ConductorGeom {
        WireDataObj::geom(self)
    }
    fn amps(&self) -> (f64, f64, i32, &[f64]) {
        WireDataObj::amps(self)
    }
    fn conductor_kind(&self) -> ConductorKind {
        ConductorKind::Wire
    }
}

impl ConductorData for CnDataObj {
    fn geom(&self) -> ConductorGeom {
        CnDataObj::geom(self)
    }
    fn amps(&self) -> (f64, f64, i32, &[f64]) {
        CnDataObj::amps(self)
    }
    fn conductor_kind(&self) -> ConductorKind {
        ConductorKind::Cn
    }
}

impl ConductorData for TsDataObj {
    fn geom(&self) -> ConductorGeom {
        TsDataObj::geom(self)
    }
    fn amps(&self) -> (f64, f64, i32, &[f64]) {
        TsDataObj::amps(self)
    }
    fn conductor_kind(&self) -> ConductorKind {
        ConductorKind::Ts
    }
}

/// A **snapshot-cloned** conductor catalog object, as a `LineGeometry`
/// (`FWireData[i]`) and a `Line` (`LineWireData[i]`) own it — the typed
/// (DE_PASCALIZE R3 Category C) replacement for the `Box<dyn DssObject>` those
/// arrays used to hold.
///
/// The three variants are exactly the classes the `wire=`/`cncable=`/`tscable=`,
/// `wires=`/`cncables=`/`tscables=` and `conductors=` properties resolve against
/// (`PropDef::object_ref_class("WireData"|"CNData"|"TSData")` and the
/// [`CONDUCTOR_PROXY_CLASSES`] proxy), so the storage can no longer hold
/// anything else — what used to be a runtime `as_conductor()` probe on every
/// read is now a static match.
#[derive(Debug, Clone)]
pub enum ConductorObj {
    Wire(WireDataObj),
    Cn(CnDataObj),
    Ts(TsDataObj),
}

impl ConductorObj {
    /// The resolve-time snapshot clone of a resolved conductor reference — the
    /// typed twin of the `clone_box()` these arrays used to store. `None` if the
    /// reference names some other class, which the property-side class
    /// restriction above makes unreachable.
    pub fn from_resolved(o: crate::obj::arena::ResolvedObj<'_>) -> Option<Self> {
        if let Some(w) = o.cloned::<WireDataObj>() {
            Some(ConductorObj::Wire(w))
        } else if let Some(c) = o.cloned::<CnDataObj>() {
            Some(ConductorObj::Cn(c))
        } else {
            o.cloned::<TsDataObj>().map(ConductorObj::Ts)
        }
    }

    /// The (lowercased) object name, as dumps and `Save` render it.
    pub fn name(&self) -> &str {
        match self {
            ConductorObj::Wire(w) => w.data().name(),
            ConductorObj::Cn(c) => c.data().name(),
            ConductorObj::Ts(t) => t.data().name(),
        }
    }

    /// `(NormAmps, EmergAmps, NumAmpRatings, AmpRatings)` with the ratings
    /// copied out — the owned form the `LineGeometry`/`Line` rating defaults
    /// need while `self` is borrowed from the array they write into.
    pub fn amps_owned(&self) -> (f64, f64, i32, Vec<f64>) {
        let (n, e, k, r) = self.amps();
        (n, e, k, r.to_vec())
    }
}

impl ConductorData for ConductorObj {
    fn geom(&self) -> ConductorGeom {
        match self {
            ConductorObj::Wire(w) => w.geom(),
            ConductorObj::Cn(c) => c.geom(),
            ConductorObj::Ts(t) => t.geom(),
        }
    }
    fn amps(&self) -> (f64, f64, i32, &[f64]) {
        match self {
            ConductorObj::Wire(w) => w.amps(),
            ConductorObj::Cn(c) => c.amps(),
            ConductorObj::Ts(t) => t.amps(),
        }
    }
    fn conductor_kind(&self) -> ConductorKind {
        match self {
            ConductorObj::Wire(_) => ConductorKind::Wire,
            ConductorObj::Cn(_) => ConductorKind::Cn,
            ConductorObj::Ts(_) => ConductorKind::Ts,
        }
    }
}
