//! Port of `PDElements/Reactor.pas` — `TReactorObj`, a two-terminal
//! constant-impedance shunt (or series) reactor. Like the capacitor it follows
//! the Capacitor/Fault connection rules: Bus2 defaults to the grounded-zero node
//! of Bus1 (a shunt reactor); specifying Bus2 with matching nodes makes a series
//! reactor. `Parallel=Yes` treats the `R` and `X` components as parallel.
//!
//! Reactance is specified one of four ways (`SpecType`):
//!   1. `kvar`+`kV` ratings at base frequency.
//!   2. series `R`+`X` ohms (or `R`+`LmH`, or the `Z` complex array).
//!   3. `RMatrix`/`XMatrix` ohms (optionally in parallel).
//!   4. symmetrical components `Z1`, `Z2`, `Z0` (`Z2`/`Z0` default to `Z1`).
//!
//! `RCurve`/`LCurve` reference an `XYcurve` (ported in PHASE5_PLAN WP5.1),
//! snapshot-cloned at parse time like the PVSystem/VCCS curve refs. Their only
//! consumer is the frequency-dependent `R(f)`/`L(f)` scaling in `CalcYPrim`'s
//! `SpecType` 1/2 branch (Pascal `Reactor.pas` `CalcYPrim`): when assigned,
//! `RValue := Z.re * RCurveObj.GetYValue(FYprimFreq)` and
//! `LValue := L * LCurveObj.GetYValue(FYprimFreq)` — the curve's X axis is
//! **Hz** (`FYprimFreq`, the solution frequency, zeroed on the sub-0.51 Hz GIC
//! path), not the frequency multiplier.
//!
//! Split into submodules mirroring `capacitor/`, `line/`, `transformer/`:
//! - this `mod.rs` — property ordinals, `class_props`, the `Reactor` struct, `new`.
//! - `solve.rs` — the impedance/admittance numerics (`recalc`, `stamp_series`,
//!   `CalcYPrim`) and the `impl CktElement`.
//! - `accessors.rs` — the `impl DssObject` property surface and side effects.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::general::xy_curve::XyCurveObj;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::util::sqrt3;

mod accessors;
mod dump;
mod solve;

/// Pascal `TReactorObj.SpecType` (`Reactor.pas:131-132`, declared `Integer`
/// with the upstream `//TODO: Use enum for ReactorObj.SpecType` right above it).
///
/// A *derived* code: no property writes it, so it has no `DssEnum` registry
/// entry. It is set by six property side effects (`Reactor.pas:380-491`) and
/// zero-initialized to [`Self::Kvar`] in `Create` (`:601` — whose trailing
/// comment `1=kvar, 2=Cuf, 3=Cmatrix` is a copy-paste from Capacitor; the
/// authoritative legend is the field declaration at `:132`).
///
/// | value | set by | `Reactor.pas` |
/// |---|---|---|
/// | 1 `Kvar` | `kvar=` | `:382` (and `Create`, `:601`) |
/// | 2 `RplusJx` | `X=`, `Z=`, `LmH=` | `:434`, `:474`, `:478` |
/// | 3 `Matrices` | `RMatrix=`, `XMatrix=` | `:419` |
/// | 4 `SymComponents` | `Z1=` | `:451` |
///
/// The value set is therefore closed at 1..=4, which is what makes the three
/// `match`es in `solve.rs` exhaustive without a fall-through arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReactorSpecType {
    /// 1 — X computed from the `kvar`/`kV` ratings.
    Kvar = 1,
    /// 2 — series `R` + `X` ohms (also `Z` and `LmH`).
    RplusJx = 2,
    /// 3 — `RMatrix`/`XMatrix` ohms.
    Matrices = 3,
    /// 4 — symmetrical components `Z1`/`Z2`/`Z0`.
    SymComponents = 4,
}

impl ReactorSpecType {
    /// The raw Pascal `SpecType` integer.
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the raw Pascal `SpecType` integer; `None` outside 1..=4.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Kvar),
            2 => Some(Self::RplusJx),
            3 => Some(Self::Matrices),
            4 => Some(Self::SymComponents),
            _ => None,
        }
    }
}

/// 1-based property ordinals (Pascal `TReactorProp` + class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const PHASES: usize = 3;
    pub const KVAR: usize = 4;
    pub const KV: usize = 5;
    pub const CONN: usize = 6;
    pub const RMATRIX: usize = 7;
    pub const XMATRIX: usize = 8;
    pub const PARALLEL: usize = 9;
    pub const R: usize = 10;
    pub const X: usize = 11;
    pub const RP: usize = 12;
    pub const Z1: usize = 13;
    pub const Z2: usize = 14;
    pub const Z0: usize = 15;
    pub const Z: usize = 16;
    pub const RCURVE: usize = 17;
    pub const LCURVE: usize = 18;
    pub const LMH: usize = 19;
    // TPDClass tail:
    pub const NORMAMPS: usize = 20;
    pub const EMERGAMPS: usize = 21;
    pub const FAULTRATE: usize = 22;
    pub const PCTPERM: usize = 23;
    pub const REPAIR: usize = 24;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 25;
    pub const ENABLED: usize = 26;
    pub const NUM_PROPS: usize = 27; // incl. Like
}

/// `TReactor.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    // Property names carry the oracle's **display case** (Pascal `PropertyName[i]`),
    // which `Dump`/`Save` emit verbatim; matching stays case-insensitive
    // (`CommandList` lowercases both sides), so `bus1=`/`Bus1=` both parse. The
    // canonical spelling is pinned by the `dump_reactor` golden (WP8.5 step 1).
    let defs = vec![
        // Pascal flags bus1 `Required` (inert here — not enforced in Phase 4).
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::bus("Bus2", 2),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("kvar").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_KVAR),
        PropDef::double("kV")
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NON_NEGATIVE | PropFlags::UNITS_KV),
        PropDef::mapped_string_enum("Conn", enums.connection),
        PropDef::double_sym_matrix("RMatrix", PHASES),
        PropDef::double_sym_matrix("XMatrix", PHASES).flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::boolean("Parallel"),
        PropDef::double("R")
            .flags(PropFlags::REDUNDANT | PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM),
        PropDef::double("X").flags(
            PropFlags::REDUNDANT
                | PropFlags::REQUIRED_IN_SPEC_SET
                | PropFlags::NO_DEFAULT
                | PropFlags::UNITS_OHM,
        ),
        PropDef::double("Rp").flags(PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM),
        PropDef::complex("Z1").flags(PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM),
        PropDef::complex("Z2").flags(PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM),
        PropDef::complex("Z0")
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM),
        PropDef::complex("Z")
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NO_DEFAULT | PropFlags::UNITS_OHM),
        // RCurve/LCurve reference XYcurve (ported WP5.1); see the module note
        // for the harmonic-CalcYPrim consumer.
        PropDef::object_ref_class("XYcurve", "RCurve"),
        PropDef::object_ref_class("XYcurve", "LCurve"),
        PropDef::double("LmH")
            .scale(1.0e-3)
            .flags(PropFlags::REDUNDANT | PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_MH),
        // TPDClass tail: Pascal sets these AFTER `inherited DefineProperties` with
        // `DynamicDefault + Units_A` (`Reactor.pas:310-311`) — emitted (no default),
        // in `A`.
        PropDef::double("NormAmps").flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_A),
        PropDef::double("EmergAmps").flags(PropFlags::DYNAMIC_DEFAULT | PropFlags::UNITS_A),
        PropDef::double("FaultRate"),
        PropDef::double("pctPerm"),
        PropDef::double("Repair"),
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
    ClassProps::new("Reactor", defs, true)
}

/// `TReactorObj`.
#[derive(Debug, Clone)]
pub struct Reactor {
    pub cd: CktElementData,
    /// Parallel resistance and its conductance (`Rp`, `Gp`).
    rp: f64,
    gp: f64,
    /// Inductance in henries (Pascal `L`; the `LmH` property scales by 1e-3).
    l: f64,
    kvarrating: f64,
    kvrating: f64,
    /// Series impedance (`Z`); the `R`/`X` redundant properties alias `Z.re`/im.
    z: Complex64,
    /// Symmetrical-component impedances (`SpecType = 4`).
    z1: Complex64,
    z2: Complex64,
    z0: Complex64,
    /// `RMatrix`/`XMatrix` ohms (row-major `nphases²`); `None` unless
    /// `SpecType = 3`. `gmatrix`/`bmatrix` are the inverted parallel forms.
    rmatrix: Option<Vec<f64>>,
    xmatrix: Option<Vec<f64>>,
    gmatrix: Option<Vec<f64>>,
    bmatrix: Option<Vec<f64>>,
    /// `RCurve`/`LCurve`: the referenced object's name (for Dump/Save
    /// round-trip) and a snapshot-cloned copy of the resolved `XYcurve`
    /// (`None` when unassigned or unresolved), consumed by `CalcYPrim`'s
    /// frequency-dependent `R(f)`/`L(f)` scaling.
    r_curve_name: String,
    r_curve: Option<XyCurveObj>,
    l_curve_name: String,
    l_curve: Option<XyCurveObj>,
    /// 0 = wye (default), 1 = delta.
    connection: i32,
    /// How the reactance was specified (see [`ReactorSpecType`]).
    spec_type: ReactorSpecType,
    is_parallel: bool,
    rp_specified: bool,
    bus2_defined: bool,
    z2_specified: bool,
    z0_specified: bool,
    is_shunt: bool,
    // PD-element common:
    norm_amps: f64,
    emerg_amps: f64,
    norm_amps_specified: bool,
    emerg_amps_specified: bool,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
}

impl Reactor {
    /// Pascal `TReactorObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // forces allocation of terminals/conductors + buses

        // Default Bus2 to the grounded-zero node of Bus1 (`Bus1.0.0.0`).
        let bus1 = cd.get_bus(1).to_string();
        cd.set_bus(2, &format!("{bus1}.0.0.0"));

        let kvarrating = 100.0;
        let kvrating = 12.47;
        let z = Complex64::new(0.0, kvrating * kvrating * 1000.0 / kvarrating);

        let mut r = Self {
            cd,
            rp: 0.0,
            gp: 0.0,
            l: 0.0,
            kvarrating,
            kvrating,
            z,
            z1: Complex64::new(0.0, 0.0),
            z2: Complex64::new(0.0, 0.0),
            z0: Complex64::new(0.0, 0.0),
            rmatrix: None,
            xmatrix: None,
            gmatrix: None,
            bmatrix: None,
            r_curve_name: String::new(),
            r_curve: None,
            l_curve_name: String::new(),
            l_curve: None,
            connection: 0, // wye
            spec_type: ReactorSpecType::Kvar,
            is_parallel: false,
            rp_specified: false,
            bus2_defined: false,
            z2_specified: false,
            z0_specified: false,
            is_shunt: true,
            norm_amps: kvarrating * sqrt3() / kvrating,
            emerg_amps: 0.0,
            norm_amps_specified: false,
            emerg_amps_specified: false,
            fault_rate: 0.0005,
            pct_perm: 100.0,
            hrs_to_repair: 3.0,
        };
        r.emerg_amps = r.norm_amps * 1.35;
        r.cd.yorder = r.cd.nterms * r.cd.nconds;
        r.recalc();
        r
    }
}
