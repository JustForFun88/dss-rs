//! Port of `PCElements/GICsource.pas` — `TGICSourceObj`, a GIC induced-voltage
//! injector **spliced into a named Line**. Developed from Isource and GICLine.
//! It has no buses/impedance of its own: its name must equal an existing Line's;
//! `RecalcElementData` inserts a `GIC_<name>` bus and rewrites the Line's `Bus2`
//! so the source sits in series with that Line. A `NON_PCPD_ELEM` `SOURCE` (like
//! VSource/Isource) — it lives on the circuit `sources` list.
//!
//! Split into submodules mirroring `isource/`:
//! - this `mod.rs` — property ordinals, `class_props`, the struct, `new`,
//!   `Compute_VLine` (sign-flipped geodesy vs GICLine).
//! - `solve.rs` — `RecalcElementData` (the splice), `CalcYPrim` (fixed G),
//!   `GetVterminalForSource` / injection, the `impl CktElement`.
//! - `accessors.rs` — the `impl DssObject` property surface + side effects.
//! - `dump.rs` — the (NON_PCPD) `TPCElement`-ordered dump dispatch.

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::traits::ElemId;
use crate::obj::base::RefAction;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

mod accessors;
mod dump;
mod solve;

/// 1-based property ordinals (Pascal `TGICsourceProp` + PC/CktElement tails).
pub mod prop {
    pub const VOLTS: usize = 1;
    pub const ANGLE: usize = 2;
    pub const FREQUENCY: usize = 3;
    pub const PHASES: usize = 4;
    pub const EN: usize = 5;
    pub const EE: usize = 6;
    pub const LAT1: usize = 7;
    pub const LON1: usize = 8;
    pub const LAT2: usize = 9;
    pub const LON2: usize = 10;
    // TPCClass tail:
    pub const SPECTRUM: usize = 11;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 12;
    pub const ENABLED: usize = 13;
    pub const NUM_PROPS: usize = 14; // incl. the auto-appended Like
}

/// `TGICsource.DefineProperties`.
pub fn class_props() -> ClassProps {
    use prop::*;
    let defs = vec![
        PropDef::double("Volts").flags(PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("Angle").flags(PropFlags::UNITS_DEG),
        PropDef::double("Frequency")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::UNITS_HZ),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("EN").flags(
            PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_V_PER_KM,
        ),
        PropDef::double("EE").flags(
            PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_V_PER_KM,
        ),
        PropDef::double("Lat1")
            .flags(PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_DEG),
        PropDef::double("Lon1")
            .flags(PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_DEG),
        PropDef::double("Lat2")
            .flags(PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_DEG),
        PropDef::double("Lon2")
            .flags(PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_DEG),
        // TPCClass tail (Spectrum is forced NIL — always empty):
        PropDef::object_ref_deferred("Spectrum", "Spectrum"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("GICsource", defs, true)
}

/// `TGICSourceObj`.
#[derive(Debug, Clone)]
pub struct GicSource {
    pub cd: CktElementData,
    /// `Angle` — the phase shift (deg); all phases share it (zero sequence).
    angle: f64,
    /// `Volts` — the induced source magnitude (geodesy unless specified).
    volts: f64,
    /// `SrcFrequency` (default 0.1 Hz).
    src_frequency: f64,
    /// Geodesy inputs (Pascal PUBLIC fields).
    e_north: f64,
    e_east: f64,
    lat1: f64,
    lon1: f64,
    lat2: f64,
    lon2: f64,
    /// `VN`/`VE` — the north/east EMF components (Compute_VLine).
    vn: f64,
    ve: f64,
    /// `VoltsSpecified` — `Volts`/`Angle` drives the magnitude (else geodesy).
    volts_specified: bool,
    /// `Bus2Defined` — set once the splice has run.
    bus2_defined: bool,
    /// Pascal `pLineElem`: the associated Line, resolved by the executive at
    /// edit-completion (no store access in the constructor), plus the Line's
    /// current `Bus2` (read at resolve time so `RecalcElementData` can decide
    /// whether to splice).
    line_ref: Option<ElemId>,
    line_bus2: String,
    /// The Line reference could not be resolved (name mismatch) — Pascal error
    /// 333 in `RecalcElementData`.
    line_missing: bool,
    /// Deferred cross-element writes (the Line `Bus2` rewrite), drained by the
    /// executive after `end_edit`.
    pending_actions: Vec<RefAction>,
    /// `Spectrum` — always empty (spectrum forbidden), but the property exists.
    spectrum: String,
    spectrum_obj: Option<SpectrumObj>,
}

impl GicSource {
    /// Pascal `TGICSourceObj.Create`. (The `LineClass.Find(Name)` the Pascal
    /// constructor runs has no equivalent here — the executive resolves the Line
    /// through the foreign class view at edit-completion, see
    /// [`GicSource::set_resolved_line`].)
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // 4/27/2018 made a 2-terminal I source

        let mut gs = Self {
            cd,
            angle: 0.0,
            volts: 0.0,
            src_frequency: 0.1, // this is the GIC source
            e_north: 1.0,
            e_east: 1.0,
            lat1: 33.613499,
            lon1: -87.373673,
            lat2: 33.547885,
            lon2: -86.074605,
            vn: 0.0,
            ve: 0.0,
            volts_specified: false,
            bus2_defined: false,
            line_ref: None,
            line_bus2: String::new(),
            line_missing: false,
            pending_actions: Vec::new(),
            spectrum: String::new(), // Spectrum not allowed
            spectrum_obj: None,
        };
        gs.cd.yorder = gs.cd.nterms * gs.cd.nconds;
        // Pascal comment: "Don't do This here RecalcElementData" — the constructor
        // deliberately skips recalc (the Line isn't spliced until EndEdit).
        gs
    }

    /// Hand the executive-resolved associated Line to this source (Pascal's
    /// constructor `pLineElem := LineClass.Find(Name)`; done here because the
    /// object factory has no store access). `line_bus2` is the Line's present
    /// `Bus2` (read through the foreign view), which `RecalcElementData` compares
    /// against the `GIC_` prefix. `None` marks the Line missing (error 333).
    pub fn set_resolved_line(&mut self, line: Option<(ElemId, String)>) {
        match line {
            Some((r, bus2)) => {
                self.line_ref = Some(r);
                self.line_bus2 = bus2;
                self.line_missing = false;
            }
            None => {
                self.line_ref = None;
                self.line_missing = true;
            }
        }
    }

    /// Pascal `TGICSourceObj.Compute_VLine` (GICsource.pas:313): the geodesy EMF
    /// with the **sign-flipped** deltas (`DeltaLat = Lat1 − Lat2`,
    /// `DeltaLon = Lon1 − Lon2`) — "switched 11-20 to get pos GIC for pos ENorth".
    pub(super) fn compute_vline(&mut self) -> f64 {
        let phi = (self.lat2 + self.lat1) / 2.0 * (std::f64::consts::PI / 180.0); // deg → rad
        let delta_lat = self.lat1 - self.lat2;
        let delta_lon = self.lon1 - self.lon2;
        self.ve = (111.133 - 0.56 * (2.0 * phi).cos()) * delta_lat * self.e_north;
        self.vn = (111.5065 - 0.1872 * (2.0 * phi).cos()) * phi.cos() * delta_lon * self.e_east;
        self.vn + self.ve
    }
}
