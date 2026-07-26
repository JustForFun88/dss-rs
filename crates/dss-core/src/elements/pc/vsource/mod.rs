//! Port of `PCElements/Vsource.pas` — `TVsourceObj`, the Thevenin-equivalent
//! voltage source. A **2-terminal** device: terminal 2 defaults to the zero
//! (ground) nodes of bus 1, and the primitive Y is the 2N×2N block matrix
//! `[Zinv, −Zinv; −Zinv, Zinv]` built from the full sequence-impedance
//! matrix `Z` (`Zs`/`Zm` from Z1/Z2/Z0).
//!
//! Split into submodules mirroring `load/`, `generator/`, `transformer/`:
//! - this `mod.rs` — property ordinals, `class_props`, the `VSource` struct,
//!   `new`, and the shared `get_vmag` helper.
//! - `solve.rs` — the sequence-impedance numerics (`recalc`), `CalcYPrim`, and
//!   the `impl CktElement`.
//! - `source.rs` — the source-voltage / injection path
//!   (`GetVterminalForSource`, the loadshape multipliers, `GetInjCurrents`).
//! - `accessors.rs` — the `impl DssObject` property surface and side effects.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::traits::ElemId;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags, prop_index};
use crate::support::cmatrix::CMatrix;

mod accessors;
mod dump;
mod solve;
mod source;

/// 1-based property ordinals (Pascal `TVsourceProp` + the class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BASEKV: usize = 2;
    pub const PU: usize = 3;
    pub const ANGLE: usize = 4;
    pub const FREQUENCY: usize = 5;
    pub const PHASES: usize = 6;
    pub const MVASC3: usize = 7;
    pub const MVASC1: usize = 8;
    pub const X1R1: usize = 9;
    pub const X0R0: usize = 10;
    pub const ISC3: usize = 11;
    pub const ISC1: usize = 12;
    pub const R1: usize = 13;
    pub const X1: usize = 14;
    pub const R0: usize = 15;
    pub const X0: usize = 16;
    pub const SCAN_TYPE: usize = 17;
    pub const SEQUENCE: usize = 18;
    pub const BUS2: usize = 19;
    pub const Z1: usize = 20;
    pub const Z0: usize = 21;
    pub const Z2: usize = 22;
    pub const PUZ1: usize = 23;
    pub const PUZ0: usize = 24;
    pub const PUZ2: usize = 25;
    pub const BASE_MVA: usize = 26;
    pub const YEARLY: usize = 27;
    pub const DAILY: usize = 28;
    pub const DUTY: usize = 29;
    pub const MODEL: usize = 30;
    pub const PUZ_IDEAL: usize = 31;
    // TPCClass / TCktElementClass tails:
    pub const SPECTRUM: usize = 32;
    pub const BASE_FREQ: usize = 33;
    pub const ENABLED: usize = 34;
    pub const NUM_PROPS: usize = 35; // incl. the auto-appended Like
}

/// Build the `Vsource` property table (`TVSource.DefineProperties`).
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let mut defs = vec![
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::double("BasekV").flags(PropFlags::REQUIRED),
        PropDef::double("pu"),
        PropDef::double("Angle"),
        PropDef::double("Frequency").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("MVASC3").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_MVA),
        PropDef::double("MVASC1").flags(PropFlags::UNITS_MVA),
        PropDef::double("X1R1"),
        PropDef::double("X0R0"),
        PropDef::double("Isc3")
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NO_DEFAULT | PropFlags::UNITS_A),
        PropDef::double("Isc1").flags(PropFlags::NO_DEFAULT | PropFlags::UNITS_A),
        PropDef::double("R1").flags(PropFlags::REDUNDANT),
        PropDef::double("X1").flags(PropFlags::REDUNDANT),
        PropDef::double("R0").flags(PropFlags::REDUNDANT),
        PropDef::double("X0").flags(PropFlags::REDUNDANT),
        PropDef::mapped_string_enum("ScanType", enums.scan_type),
        PropDef::mapped_string_enum("Sequence", enums.sequence),
        PropDef::bus("Bus2", 2),
        PropDef::complex("Z1").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_OHM),
        PropDef::complex("Z0").flags(PropFlags::UNITS_OHM),
        PropDef::complex("Z2").flags(PropFlags::UNITS_OHM),
        PropDef::complex("puZ1").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::complex("puZ0"),
        PropDef::complex("puZ2"),
        PropDef::double("BaseMVA").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::object_ref_class("LoadShape", "Yearly"),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty"),
        PropDef::mapped_string_enum("Model", enums.vsource_model),
        PropDef::complex("puZIdeal"),
        // PCClass tail:
        PropDef::object_ref_deferred("Spectrum", "Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(
            PropFlags::DYNAMIC_DEFAULT
                | PropFlags::NON_NEGATIVE
                | PropFlags::NON_ZERO
                | PropFlags::UNITS_HZ,
        ),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);

    // JSON default-mode redundancy (Pascal `Vsource.pas:346-349`): R1/X1 defer
    // to Z1, R0/X0 to Z0 (the `REDUNDANT` flags are already set above).
    let z1 = prop_index(&defs, "Z1");
    let z0 = prop_index(&defs, "Z0");
    for (name, target) in [("R1", z1), ("X1", z1), ("R0", z0), ("X0", z0)] {
        let i = prop_index(&defs, name);
        defs[i - 1].redundant_with = target;
    }

    defs.shrink_to_fit();
    ClassProps::new("Vsource", defs, true)
}

/// `TVsourceObj`.
#[derive(Debug, Clone)]
pub struct VSource {
    pub cd: CktElementData,

    pub mva_sc3: f64,
    pub mva_sc1: f64,
    pub isc3: f64,
    pub isc1: f64,
    /// 1 = MVAsc, 2 = Isc, 3 = Z specified.
    pub z_spec_type: i32,
    pub r1: f64,
    pub x1: f64,
    pub r2: f64,
    pub x2: f64,
    pub r0: f64,
    pub x0: f64,
    pub x1r1: f64,
    pub x0r0: f64,
    pub base_mva: f64,
    pub pu_z1: Complex64,
    pub pu_z0: Complex64,
    pub pu_z2: Complex64,
    pub pu_z_ideal: Complex64,
    pub z_base: f64,
    pub bus2_defined: bool,
    pub z1_specified: bool,
    pub pu_z1_specified: bool,
    pub pu_z0_specified: bool,
    pub pu_z2_specified: bool,
    pub z2_specified: bool,
    pub z0_specified: bool,
    pub is_quasi_ideal: bool,
    pub scan_type: i32,
    pub sequence_type: i32,
    /// Base-frequency series Z matrix (order = nphases).
    pub z: Option<CMatrix>,
    pub zinv: Option<CMatrix>,
    pub vmag: f64,
    pub kv_base: f64,
    pub per_unit: f64,
    pub angle: f64,
    pub src_frequency: f64,
    /// Loadshape references by name (for the dump).
    pub yearly_shape: String,
    pub daily_shape: String,
    pub duty_shape: String,
    pub spectrum: String,
    /// Resolved harmonic spectrum (Pascal `SpectrumObj`), snapshot-cloned at
    /// edit-completion from the default/explicit `spectrum=` name; consumed by
    /// the harmonic injection path (`GetVterminalForSource`).
    pub spectrum_obj: Option<SpectrumObj>,
    /// Resolved shape objects (Pascal `YearlyShapeObj` etc.), snapshot-cloned
    /// at parse time like [`super::super::load::Load`]; `GetVterminalForSource`
    /// drives `GetMultAtHour` on the owned copy in a time-series mode.
    pub yearly_shape_obj: Option<LoadShapeObj>,
    pub daily_shape_obj: Option<LoadShapeObj>,
    pub duty_shape_obj: Option<LoadShapeObj>,
    pub yearly_shape_ref: Option<ElemId>,
    pub daily_shape_ref: Option<ElemId>,
    pub duty_shape_ref: Option<ElemId>,
    /// Pascal `ShapeFactor`/`ShapeIsActual` from the active shape (per-unit or
    /// actual); `(1, 0)` outside a loadshape mode.
    pub shape_factor: Complex64,
    pub shape_is_actual: bool,
}

impl VSource {
    /// Pascal `TVsourceObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // Now a 2-terminal device

        let kv_base = 115.0;
        let base_mva = 100.0;
        let mut vs = Self {
            cd,
            mva_sc3: 2000.0,
            mva_sc1: 2100.0,
            isc3: 10000.0,
            isc1: 10540.0,
            z_spec_type: 1, // default to MVAsc
            r1: 1.65,
            x1: 6.6,
            r2: 1.65,
            x2: 6.6,
            r0: 1.9,
            x0: 5.7,
            x1r1: 4.0,
            x0r0: 3.0,
            base_mva,
            pu_z1: Complex64::ZERO,
            pu_z0: Complex64::ZERO,
            pu_z2: Complex64::ZERO,
            pu_z_ideal: Complex64::new(1.0e-6, 0.001),
            z_base: kv_base * kv_base / base_mva,
            bus2_defined: false,
            z1_specified: false,
            pu_z1_specified: false,
            pu_z0_specified: false,
            pu_z2_specified: false,
            z2_specified: false,
            z0_specified: false,
            is_quasi_ideal: false,
            scan_type: 1,
            sequence_type: 1,
            z: None,
            zinv: None,
            vmag: 0.0,
            kv_base,
            per_unit: 1.0,
            angle: 0.0,
            src_frequency: 60.0, // BaseFrequency
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            spectrum: "defaultvsource".to_string(),
            spectrum_obj: None,
            yearly_shape_obj: None,
            daily_shape_obj: None,
            duty_shape_obj: None,
            yearly_shape_ref: None,
            daily_shape_ref: None,
            duty_shape_ref: None,
            shape_factor: Complex64::new(1.0, 0.0),
            shape_is_actual: false,
        };
        // Property tracking defaults (NoPropertyTracking is off by default).
        vs.cd.obj.set_as_next_seq(prop::MVASC3);
        vs.cd.obj.set_as_next_seq(prop::MVASC1);
        vs.cd.obj.set_as_next_seq(prop::BASEKV);
        vs.recalc();
        vs
    }
}

/// Pascal `Vmag` computation (`RecalcElementData`/`GetVterminalForSource`):
/// 1-phase uses kV directly; polyphase divides by `2·sin(π/n)` (= √3 for 3
/// phases).
pub(super) fn get_vmag(kv_base: f64, per_unit: f64, nphases: usize) -> f64 {
    if nphases == 1 {
        kv_base * per_unit * 1000.0
    } else {
        kv_base * per_unit * 1000.0
            / 2.0
            / ((180.0 / nphases as f64) * std::f64::consts::PI / 180.0).sin()
    }
}
