//! Pascal `Common/AutoAdd.pas` `TAutoAdd` — the auto-add solution machinery.
//!
//! **Skeleton scope (PHASE6_PLAN §2.6):** only the option-bearing object is
//! ported — the public mode variables and their `Init` defaults, so the
//! `Set GenkW/GenPF/Capkvar/AddType=` options round-trip through `Set`/`Get`.
//!
//! The capacity-search `Solve` (and the `MakeBusList`/`AddCurrents`/
//! `ComputekWLosses_EEN`/`Get_WeightedLosses` helpers) is `NOT_PORTED` → it
//! needs aux-current injection inside the solve loop (`UseAuxCurrents`) plus
//! energy-meter register sampling that only land in a later phase. The
//! `AutoAdd` solve mode therefore keeps its "Unknown solution mode" error and
//! STATUS.md documents the deferral. The private `Solve`-only state
//! (`BusIdxList`, `LastAddedGenerator`/`LastAddedCapacitor`, the loss/EEN
//! accumulators) is intentionally omitted until that port.

/// Pascal `DSSGlobals.GENADD` — add a generator at the lowest-loss bus.
pub const GENADD: i32 = 1;
/// Pascal `DSSGlobals.CAPADD` — add a capacitor at the lowest-loss bus.
pub const CAPADD: i32 = 2;

/// Pascal `TAutoAdd` (record), one per circuit (`Circuit.AutoAddObj`).
#[derive(Debug, Clone)]
pub struct AutoAdd {
    /// `GenkW` — kW of the trial generator.
    pub gen_kw: f64,
    /// `GenPF` — power factor of the trial generator.
    pub gen_pf: f64,
    /// `Genkvar` — derived from `GenkW`/`GenPF` inside `Solve`; FPC
    /// zero-initializes the record field, so it starts at 0.0.
    pub gen_kvar: f64,
    /// `Capkvar` — kvar of the trial capacitor.
    pub cap_kvar: f64,
    /// `AddType` — `GENADD`/`CAPADD`.
    pub add_type: i32,
    /// `ModeChanged` — forces `MakeBusList` to rebuild on the next auto-add
    /// solve (kept for fidelity; only the unported `Solve` consumes it).
    pub mode_changed: bool,
}

impl AutoAdd {
    /// Pascal `TAutoAdd.Init`.
    pub fn new() -> Self {
        Self {
            gen_kw: 1000.0,
            gen_pf: 1.0,
            gen_kvar: 0.0,
            cap_kvar: 600.0,
            add_type: GENADD,
            mode_changed: true,
        }
    }
}

impl Default for AutoAdd {
    fn default() -> Self {
        Self::new()
    }
}
