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
//! `RCurve`/`LCurve` reference an `XYcurve` (ported in PHASE5_PLAN WP5.1) but
//! stay flagged `NOT_PORTED`: their only consumer is the frequency-dependent
//! `R(f)`/`L(f)` scaling in the *harmonic* `CalcYPrim`, which is Phase 7. Until
//! then `CalcYPrim` always uses the unity-curve path, so resolving the
//! reference would be dead state with no observable behavior — wire it together
//! with the harmonic scaling.
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
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};
use crate::util::sqrt3;

mod accessors;
mod solve;

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
    let defs = vec![
        // Pascal flags bus1 `Required` (inert here — not enforced in Phase 4).
        PropDef::bus("bus1", 1),
        PropDef::bus("bus2", 2),
        PropDef::integer("phases").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("kvar").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("kv").flags(PropFlags::REQUIRED_IN_SPEC_SET | PropFlags::NON_NEGATIVE),
        PropDef::mapped_string_enum("conn", enums.connection),
        PropDef::double_sym_matrix("RMatrix", PHASES),
        PropDef::double_sym_matrix("XMatrix", PHASES).flags(PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::boolean("Parallel"),
        PropDef::double("R").flags(PropFlags::REDUNDANT),
        PropDef::double("X").flags(PropFlags::REDUNDANT | PropFlags::REQUIRED_IN_SPEC_SET),
        PropDef::double("Rp"),
        PropDef::complex("Z1"),
        PropDef::complex("Z2"),
        PropDef::complex("Z0"),
        PropDef::complex("Z").flags(PropFlags::REQUIRED_IN_SPEC_SET),
        // RCurve/LCurve reference XYcurve (ported WP5.1) but are consumed only
        // by the harmonic CalcYPrim (Phase 7); see the module note.
        PropDef::object_ref("RCurve").flags(PropFlags::NOT_PORTED),
        PropDef::object_ref("LCurve").flags(PropFlags::NOT_PORTED),
        PropDef::double("LmH")
            .scale(1.0e-3)
            .flags(PropFlags::REDUNDANT | PropFlags::REQUIRED_IN_SPEC_SET),
        // TPDClass tail:
        PropDef::double("normamps"),
        PropDef::double("emergamps"),
        PropDef::double("faultrate"),
        PropDef::double("pctperm"),
        PropDef::double("repair"),
        // TCktElementClass tail:
        PropDef::double("basefreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("enabled"),
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
    /// 0 = wye (default), 1 = delta.
    connection: i32,
    /// 1 = kvar, 2 = R+jX, 3 = R/X matrices, 4 = symmetrical components.
    spec_type: i32,
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
            connection: 0, // wye
            spec_type: 1,  // kvar
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
