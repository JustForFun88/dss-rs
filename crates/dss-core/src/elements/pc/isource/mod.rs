//! Port of `PCElements/Isource.pas` — `TIsourceObj`, the ideal current source.
//! A **2-terminal** device (Pascal comment: "Stick'em on wherever you want as
//! many as you want"): terminal 2 defaults to the zero (ground) nodes of bus
//! 1, and the primitive Y is **all-zero** (an ideal current source has no
//! self-impedance) — the whole element is one injection path with no linear
//! Y-matrix contribution beyond the standard open-conductor fold.
//!
//! Split into submodules mirroring `vsource/` (the source-element template):
//! - this `mod.rs` — property ordinals, `class_props`, the `Isource` struct,
//!   and `new`.
//! - `solve.rs` — `CalcYPrim` (all-zero), the loadshape/harmonic magnitude
//!   dispatch (`GetBaseCurr`/`CalcDaily/Duty/YearlyMult`), the injection path
//!   (`GetInjCurrents`), and the `impl CktElement`.
//! - `accessors.rs` — the `impl DssObject` property surface and side effects.
//! - `dump.rs` — not a real Pascal override (Isource has none), but a thin
//!   `dump_body` that reproduces the ancestor `TPCElement.DumpProperties`
//!   ordering the generic dump path can't select for a `NON_PCPD_ELEM`
//!   `TPCElement` — see that file's header for why.
//!
//! TODO(compat): Pascal `TIsourceObj.PropertySideEffects` (`Isource.pas:221`)
//! never sets `Bus2Defined := TRUE` on the `Bus2` case — unlike
//! `TVsourceObj.PropertySideEffects` (`Vsource.pas:498`), which does. So an
//! explicit `Bus2=` is only sticky if it is parsed *after* `Bus1=` on the same
//! edit (the `Bus1` side effect unconditionally re-derives the grounded-Y
//! default whenever it runs, since `Bus2Defined` never becomes true). This is
//! a genuine, deterministic upstream quirk — reproduced 1:1 by simply never
//! writing `true` into [`Isource::bus2_defined`] from the `BUS2` case. None of
//! the WPG.14 corpus decks exercise the reversed order (`Bus2=` before
//! `Bus1=`), so nothing pins it as a golden; the omission is documented here
//! for the next reader who wonders why `bus2_defined` looks unused on writes.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::load_shape::LoadShapeObj;
use crate::elements::general::spectrum::SpectrumObj;
use crate::elements::traits::ElemRef;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

mod accessors;
mod dump;
mod solve;

/// 1-based property ordinals (Pascal `TIsourceProp` + the class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const AMPS: usize = 2;
    pub const ANGLE: usize = 3;
    pub const FREQUENCY: usize = 4;
    pub const PHASES: usize = 5;
    pub const SCAN_TYPE: usize = 6;
    pub const SEQUENCE: usize = 7;
    pub const YEARLY: usize = 8;
    pub const DAILY: usize = 9;
    pub const DUTY: usize = 10;
    pub const BUS2: usize = 11;
    // TPCClass / TCktElementClass tails:
    pub const SPECTRUM: usize = 12;
    pub const BASE_FREQ: usize = 13;
    pub const ENABLED: usize = 14;
    pub const NUM_PROPS: usize = 15; // incl. the auto-appended Like
}

/// Build the `Isource` property table (`TIsource.DefineProperties`).
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let mut defs = vec![
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::double("Amps").flags(PropFlags::NO_DEFAULT),
        PropDef::double("Angle"),
        PropDef::double("Frequency")
            .flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::mapped_string_enum("ScanType", enums.scan_type),
        PropDef::mapped_string_enum("Sequence", enums.sequence),
        PropDef::object_ref_class("LoadShape", "Yearly").flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::object_ref_class("LoadShape", "Daily"),
        PropDef::object_ref_class("LoadShape", "Duty").flags(PropFlags::DYNAMIC_DEFAULT),
        PropDef::bus("Bus2", 2).flags(PropFlags::DYNAMIC_DEFAULT),
        // PCClass tail:
        PropDef::object_ref("Spectrum"),
        // CktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    defs.shrink_to_fit();
    ClassProps::new("Isource", defs, true)
}

/// `TIsourceObj`.
#[derive(Debug, Clone)]
pub struct Isource {
    pub cd: CktElementData,

    pub amps: f64,
    pub angle: f64,
    pub src_frequency: f64,
    pub scan_type: i32,
    pub sequence_type: i32,
    /// Pascal `PerUnit`: reserved for future use (no property sets it today —
    /// see the Pascal `TODO` comment at `Isource.pas:317`); only read as the
    /// no-loadshape default in `CalcDaily/YearlyMult`.
    pub per_unit: f64,
    /// Pascal `FphaseShift`: per-phase rotation angle (`0` for 1-phase, `120`
    /// for 2/3-phase, `360/Nphases` otherwise).
    pub phase_shift: f64,
    pub bus2_defined: bool,
    /// Loadshape references by name (for the dump).
    pub yearly_shape: String,
    pub daily_shape: String,
    pub duty_shape: String,
    pub spectrum: String,
    /// Resolved harmonic spectrum (Pascal `SpectrumObj`), snapshot-cloned at
    /// edit-completion; inherited `DefaultGeneral` = `"default"` (Isource does
    /// not override the base `TPCElement.Create` default the way Load/Vsource
    /// do — see `PCElement.pas:81` vs `Load.pas:873`/`Vsource.pas:660`).
    pub spectrum_obj: Option<SpectrumObj>,
    /// Resolved shape objects (Pascal `YearlyShapeObj` etc.), snapshot-cloned
    /// at parse time like [`super::load::Load`]/[`super::vsource::VSource`].
    pub yearly_shape_obj: Option<LoadShapeObj>,
    pub daily_shape_obj: Option<LoadShapeObj>,
    pub duty_shape_obj: Option<LoadShapeObj>,
    pub yearly_shape_ref: Option<ElemRef>,
    pub daily_shape_ref: Option<ElemRef>,
    pub duty_shape_ref: Option<ElemRef>,
    /// Pascal `ShapeFactor`/`ShapeIsActual` from the active shape (per-unit or
    /// actual); default `(PerUnit, 0)` outside a loadshape mode.
    pub shape_factor: Complex64,
    pub shape_is_actual: bool,
}

impl Isource {
    /// Pascal `TIsourceObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // 2-terminal I source

        let mut isrc = Self {
            cd,
            amps: 0.0,
            angle: 0.0,
            src_frequency: 60.0, // BaseFrequency
            scan_type: 1,        // Pos Sequence
            sequence_type: 1,
            per_unit: 1.0,
            phase_shift: 120.0,
            bus2_defined: false,
            yearly_shape: String::new(),
            daily_shape: String::new(),
            duty_shape: String::new(),
            spectrum: "default".to_string(), // TPCElement: SpectrumClass.DefaultGeneral
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
        isrc.recalc();
        isrc
    }
}
