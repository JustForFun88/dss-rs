//! Port of `PCElements/GICLine.pas` — `TGICLineObj`, a simplified 2-terminal
//! Thevenin voltage source with series impedance for GIC studies (an induced
//! DC-ish EMF along a transmission line). Created from VSource; the key
//! differences are the **zero-sequence** default (same phasor on every phase),
//! the geodesy-driven `Compute_VLine` EMF, the low `Frequency=0.1` source
//! frequency, and an optional **series blocking capacitor** (`C > 0`).
//!
//! Split into submodules mirroring `vsource/`:
//! - this `mod.rs` — property ordinals, `class_props`, the struct, `new`,
//!   `Compute_VLine`.
//! - `solve.rs` — `recalc` (Z matrix), `CalcYPrim` (series RL + blocking cap),
//!   the `impl CktElement`.
//! - `source.rs` — `GetVterminalForSource` / `GetInjCurrents`.
//! - `accessors.rs` — the `impl DssObject` property surface + side effects.
//! - `dump.rs` — the `DumpProperties` override (Z Matrix / VE / VN).

use crate::elements::ckt::CktElementData;
use crate::elements::general::spectrum::SpectrumObj;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::support::cmatrix::CMatrix;

mod accessors;
mod dump;
mod solve;
mod source;

/// 1-based property ordinals (Pascal `TGICLineProp` + PC/CktElement tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const VOLTS: usize = 3;
    pub const ANGLE: usize = 4;
    pub const FREQUENCY: usize = 5;
    pub const PHASES: usize = 6;
    pub const R: usize = 7;
    pub const X: usize = 8;
    pub const C: usize = 9;
    pub const EN: usize = 10;
    pub const EE: usize = 11;
    pub const LAT1: usize = 12;
    pub const LON1: usize = 13;
    pub const LAT2: usize = 14;
    pub const LON2: usize = 15;
    // TPCClass tail:
    pub const SPECTRUM: usize = 16;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 17;
    pub const ENABLED: usize = 18;
    pub const NUM_PROPS: usize = 19; // incl. the auto-appended Like
}

/// `TGICLine.DefineProperties`.
pub fn class_props() -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal flags Bus1 `Required` (inert here — not enforced, like Reactor).
        PropDef::bus("Bus1", BUS1).flags(PropFlags::REQUIRED),
        PropDef::bus("Bus2", BUS2),
        PropDef::double("Volts")
            .flags(PropFlags::NO_DEFAULT | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_V),
        // GICLine `Angle` carries no unit flag (unlike GICsource's, `GICLine.pas`).
        PropDef::double("Angle"),
        PropDef::double("Frequency")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::UNITS_HZ),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("R").flags(PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM),
        PropDef::double("X").flags(PropFlags::UNITS_OHM),
        PropDef::double("C").flags(PropFlags::UNITS_UF),
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
        // GICLine flags both Spectrum and BaseFreq `SuppressJSON` AFTER the
        // inherited DefineProperties (`GICLine.pas:249-250`): they stay in
        // `AltPropertyOrder` (occupy `$dssPropertyOrder` slots) but are excluded
        // from the JSON/schema output — the port's `SUPPRESS_JSON_LATE`.
        PropDef::object_ref_deferred("Spectrum", "Spectrum").flags(PropFlags::SUPPRESS_JSON_LATE),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ
                | PropFlags::SUPPRESS_JSON_LATE,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("GICLine", defs, true)
}

/// `TGICLineObj`.
#[derive(Debug, Clone)]
pub struct GicLine {
    pub cd: CktElementData,
    /// `Angle` — the phase-1 shift (deg).
    angle: f64,
    /// `Volts` — the induced source magnitude (computed by `Compute_VLine`
    /// unless explicitly specified).
    volts: f64,
    /// `Vmag` — present voltage magnitude (zeroed off `SrcFrequency`).
    vmag: f64,
    /// `SrcFrequency` — the source frequency (default 0.1 Hz).
    src_frequency: f64,
    /// Series impedance components `R`/`X` (ohms) and blocking cap `C` (µF).
    r: f64,
    x: f64,
    c: f64,
    /// Geodesy inputs (`ENorth`/`EEast` in V/km, endpoints in degrees).
    e_north: f64,
    e_east: f64,
    lat1: f64,
    lon1: f64,
    lat2: f64,
    lon2: f64,
    /// `VN`/`VE` — the north/east EMF components (Compute_VLine, dumped).
    vn: f64,
    ve: f64,
    /// `ScanType`/`SequenceType` — both default 0 (zero sequence); internal,
    /// not properties.
    scan_type: i32,
    sequence_type: i32,
    /// `VoltsSpecified` — `Volts`/`Angle` drives the magnitude (else geodesy).
    volts_specified: bool,
    /// Base-frequency series `Z` matrix (order = nphases) and its inverse.
    z: Option<CMatrix>,
    zinv: Option<CMatrix>,
    /// `Spectrum` name (forced empty default) + resolved harmonic spectrum.
    spectrum: String,
    spectrum_obj: Option<SpectrumObj>,
}

impl GicLine {
    /// Pascal `TGICLineObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // Now a 2-terminal device

        let mut gl = Self {
            cd,
            angle: 0.0,
            volts: 0.0,
            vmag: 0.0,
            src_frequency: 0.1, // Typical GIC study frequency
            r: 1.0,
            x: 0.0,
            c: 0.0,
            e_north: 1.0,
            e_east: 1.0,
            lat1: 33.613499,
            lon1: -87.373673,
            lat2: 33.547885,
            lon2: -86.074605,
            vn: 0.0,
            ve: 0.0,
            scan_type: 0,     // zero sequence
            sequence_type: 0, // zero sequence (same voltage induced in all phases)
            volts_specified: false,
            z: None,
            zinv: None,
            spectrum: String::new(), // no default
            spectrum_obj: None,
        };
        gl.cd.yorder = gl.cd.nterms * gl.cd.nconds;
        gl.recalc();
        gl
    }

    /// Pascal `TGICLineObj.Compute_VLine` (GICLine.pas:349): the geodesy-driven
    /// induced EMF magnitude. `DeltaLat = Lat2 − Lat1`, `DeltaLon = Lon2 − Lon1`.
    pub(super) fn compute_vline(&mut self) -> f64 {
        let phi = (self.lat2 + self.lat1) / 2.0 * (std::f64::consts::PI / 180.0); // deg → rad
        let delta_lat = self.lat2 - self.lat1;
        let delta_lon = self.lon2 - self.lon1;
        self.ve = (111.133 - 0.56 * (2.0 * phi).cos()) * delta_lat * self.e_north;
        self.vn = (111.5065 - 0.1872 * (2.0 * phi).cos()) * phi.cos() * delta_lon * self.e_east;
        self.vn + self.ve
    }
}
