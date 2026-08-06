//! Port of `PDElements/Capacitor.pas` — `TCapacitorObj`, a two-terminal
//! constant-impedance shunt (or series) capacitor bank. The bank may have
//! several switchable steps (`NumSteps`/`States`); each energized step stamps a
//! capacitive admittance (optionally with a series filter `R`+`XL`) into `YPrim`.
//!
//! Capacitance is specified one of three ways (`SpecType`): `kvar`+`kV`,
//! `Cuf` (µF per phase), or a nodal `CMatrix` (µF). Bus2 defaults to the
//! grounded-zero node of Bus1, giving a shunt bank; specifying Bus2 with
//! matching nodes makes a series capacitor.
//!
//! The harmonic-filter recomputation (`Harm`) is ported for fidelity.
//! [`MakePosSequence`] (WPG.21) collapses the bank to its positive-sequence
//! single-phase form in [`solve`].
//!
//! [`MakePosSequence`]: crate::elements::traits::CktElement::make_pos_sequence
//!
//! Split into submodules mirroring `line/`, `load/`, `transformer/`:
//! - this `mod.rs` — property ordinals, `class_props`, the `Capacitor` struct, `new`.
//! - `steps.rs` — the switchable-step / `States` bookkeeping and the
//!   [`ControlledCapacitor`] surface CapControl drives.
//! - `solve.rs` — the impedance/admittance numerics (`recalc`, `MakeYprimWork`,
//!   `CalcYPrim`) and the `impl CktElement`.
//! - `accessors.rs` — the `impl DssObject` property surface and side effects.

#[cfg(test)]
mod tests;

use crate::elements::ckt::CktElementData;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::util::sqrt3;

mod accessors;
mod dump;
mod solve;
mod steps;

pub use steps::ControlledCapacitor;

/// Pascal `TCapacitorObj.SpecType` (`Capacitor.pas:116`, `Integer`).
///
/// A *derived* code, like the reactor's: no property writes it, so it has no
/// `DssEnum` registry entry. Three property side effects set it
/// (`Capacitor.pas:375-386`) and `Create` seeds `Kvar` (`:584`, whose comment
/// `1=kvar, 2=Cuf, 3=Cmatrix` is the legend).
///
/// | value | set by | `Capacitor.pas` |
/// |---|---|---|
/// | 1 `Kvar` | `kvar=` | `:377` (and `Create`, `:584`) |
/// | 2 `Cuf` | `cuf=` | `:385` |
/// | 3 `CMatrix` | `cmatrix=` | `:381` |
///
/// Unlike the reactor's, this ordinal **is** user-visible: `DumpProperties`
/// with `Complete` writes `SpecType=<int>` (`:764`, golden `dump_capacitor*`),
/// which is what [`Self::ordinal`] feeds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapacitorSpecType {
    /// 1 — C computed from the `kvar`/`kV` ratings.
    Kvar = 1,
    /// 2 — `Cuf` µF per phase.
    Cuf = 2,
    /// 3 — a nodal `CMatrix` (µF).
    CMatrix = 3,
}

impl CapacitorSpecType {
    /// The raw Pascal `SpecType` integer (the `SpecType=` dump line).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the raw Pascal `SpecType` integer; `None` outside 1..=3.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::Kvar),
            2 => Some(Self::Cuf),
            3 => Some(Self::CMatrix),
            _ => None,
        }
    }
}

/// 1-based property ordinals (Pascal `TCapacitorProp` + class tails).
pub mod prop {
    pub const BUS1: usize = 1;
    pub const BUS2: usize = 2;
    pub const PHASES: usize = 3;
    pub const KVAR: usize = 4;
    pub const KV: usize = 5;
    pub const CONN: usize = 6;
    pub const CMATRIX: usize = 7;
    pub const CUF: usize = 8;
    pub const R: usize = 9;
    pub const XL: usize = 10;
    pub const HARM: usize = 11;
    pub const NUMSTEPS: usize = 12;
    pub const STATES: usize = 13;
    // TPDClass tail:
    pub const NORMAMPS: usize = 14;
    pub const EMERGAMPS: usize = 15;
    pub const FAULTRATE: usize = 16;
    pub const PCTPERM: usize = 17;
    pub const REPAIR: usize = 18;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 19;
    pub const ENABLED: usize = 20;
    pub const NUM_PROPS: usize = 21; // incl. Like
}

/// Pascal `PropertyScale[ord(TProp.cuf)] := 1.0e-6` (`Capacitor.pas:243`): the
/// `Cuf` and `CMatrix` properties are stated in µF and stored in farads. Named
/// so the property table below and `MakePosSequence`'s `Cuf` write (which has
/// to divide it back out of a farad-valued `CMatrix` difference) cannot drift
/// apart.
pub(super) const CUF_SCALE: f64 = 1.0e-6;

/// `TCapacitor.DefineProperties`.
pub fn class_props(enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal flags bus1 `Required` (inert here — not enforced in Phase 4).
        PropDef::bus("Bus1", 1).flags(PropFlags::REQUIRED),
        PropDef::bus("Bus2", 2),
        PropDef::integer("Phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double_array("kvar", NUMSTEPS)
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_KVAR),
        PropDef::double("kV")
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NON_NEGATIVE | PropFlags::UNITS_KV),
        PropDef::mapped_string_enum("Conn", enums.connection),
        PropDef::double_sym_matrix("CMatrix", PHASES)
            .scale(CUF_SCALE)
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::UNITS_UF),
        PropDef::double_array("Cuf", NUMSTEPS)
            .scale(CUF_SCALE)
            .flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NO_DEFAULT | PropFlags::UNITS_UF),
        PropDef::double_array("R", NUMSTEPS),
        PropDef::double_array("XL", NUMSTEPS),
        PropDef::double_array("Harm", NUMSTEPS),
        PropDef::integer("NumSteps")
            .flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO | PropFlags::SUPPRESS_JSON),
        PropDef::int_array("States", NUMSTEPS),
        // TPDClass tail:
        PropDef::double("NormAmps"),
        PropDef::double("EmergAmps"),
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
    ClassProps::new("Capacitor", defs, true)
}

/// `TCapacitorObj`.
#[derive(Debug, Clone)]
pub struct Capacitor {
    pub cd: CktElementData,
    /// Per-step capacitance (farads), filter reactance, kvar rating, filter
    /// resistance, tuning harmonic and on/off state (Pascal `FC`, `FXL`,
    /// `Fkvarrating`, `FR`, `FHarm`, `FStates`).
    fc: Vec<f64>,
    fxl: Vec<f64>,
    fkvarrating: Vec<f64>,
    fr: Vec<f64>,
    fharm: Vec<f64>,
    fstates: Vec<i32>,
    ftotalkvar: f64,
    kvrating: f64,
    fnumsteps: i32,
    flast_step_in_service: i32,
    /// Nodal capacitance matrix (µF stored as farads, row-major `nphases²`);
    /// `None` unless `SpecType = 3`.
    cmatrix: Option<Vec<f64>>,
    do_harmonic_recalc: bool,
    bus2_defined: bool,
    /// How the capacitance was specified (see [`CapacitorSpecType`]).
    spec_type: CapacitorSpecType,
    num_term: i32,
    is_shunt: bool,
    connection: i32,
    // PD-element common:
    norm_amps: f64,
    emerg_amps: f64,
    norm_amps_specified: bool,
    emerg_amps_specified: bool,
    fault_rate: f64,
    pct_perm: f64,
    hrs_to_repair: f64,
}

impl Capacitor {
    /// Pascal `TCapacitorObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut cd = CktElementData::new(name, prop::NUM_PROPS);
        cd.nphases = 3;
        cd.nconds = 3;
        cd.set_nterms(2); // forces allocation of terminals/conductors + buses

        // Default Bus2 to the grounded-zero node of Bus1 (`Bus1.0.0.0`).
        let bus1 = cd.get_bus(1).to_string();
        cd.set_bus(2, &format!("{bus1}.0.0.0"));

        let kvrating: f64 = 12.47;
        let two_pi = 2.0 * std::f64::consts::PI;
        let base_freq = cd.base_frequency;
        let fkvar = 1200.0;
        // FC default: InitDblArray(1, FC, 1/(TwoPi*BaseFreq*SQR(kv)*1000/kvar)).
        // `SQR` binds before the surrounding product, so the square must stay an
        // atom: `a * SQR(b)` is `a * (b*b)`, NOT `(a*b) * b` (they differ by an
        // ULP for many operands — see the sibling sites in `solve.rs`). This
        // particular value is immediately overwritten by the `RecalcElementData`
        // at the end of `TCapacitorObj.Create` (SpecType = 1 recomputes `FC`), so
        // it is unobservable; kept faithful anyway.
        let fc0 = 1.0 / (two_pi * base_freq * kvrating.powi(2) * 1000.0 / fkvar);

        let mut c = Self {
            cd,
            fc: vec![fc0],
            fxl: vec![0.0],
            fkvarrating: vec![fkvar],
            fr: vec![0.0],
            fharm: vec![0.0],
            fstates: vec![1],
            ftotalkvar: 0.0,
            kvrating,
            fnumsteps: 1,
            flast_step_in_service: 1,
            cmatrix: None,
            do_harmonic_recalc: false,
            bus2_defined: false,
            spec_type: CapacitorSpecType::Kvar,
            num_term: 1,
            is_shunt: true,
            connection: 0,                                // wye
            norm_amps: fkvar * sqrt3() / kvrating * 1.35, // 135%
            emerg_amps: 0.0,
            norm_amps_specified: false,
            emerg_amps_specified: false,
            fault_rate: 0.0005,
            pct_perm: 100.0,
            hrs_to_repair: 3.0,
        };
        c.emerg_amps = c.norm_amps * 1.8 / 1.35; // 180%
        c.cd.yorder = c.cd.nterms * c.cd.nconds;
        c.recalc();
        c
    }
}
