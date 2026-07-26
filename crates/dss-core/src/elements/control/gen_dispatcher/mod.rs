//! Port of `Controls/GenDispatcher.pas` — `TGenDispatcherObj`, a control element
//! that watches one terminal of a monitored circuit element and redispatches a
//! set of generators' `kWBase`/`kvarBase` to drive the monitored power back into
//! a `kWLimit ± kWBand/2` (and `kvarLimit ± kWBand/2`) band.
//!
//! Like every `TControlElem`, a GenDispatcher builds **no Yprim** and its
//! terminal currents are zero; its single terminal attaches to the monitored
//! element's terminal bus. `DoPendingAction`/`Reset` are no-ops in Pascal (the
//! redispatch happens entirely in `Sample`, which only pushes a present-time
//! action onto the queue to force a re-solve).
//!
//! The generator list is a *dynamic* set, so unlike RegControl/CapControl (which
//! pair/triple with a single fixed target) the dispatch needs the executive to
//! reach an arbitrary number of generators. That plumbing is abstracted behind
//! [`GenDispatchEnv`]; the control-loop implements it over the class registry
//! (`solution/controls.rs`), and the unit tests against a mock.
//!
//! `TGenDispatcherObj.MakePosSequence` is ported as a NIL-deref-safe no-op — the
//! upstream body dereferences the always-NIL `ControlledElement` (Access
//! violation, `docs/wpg21_makeposseq_probes.md`), which CLAUDE.md forbids
//! reproducing (see [`accessors`]).
//!
//! **Deliberately not ported** (consistent with the rest of the controls):
//! - the `Element` property's Pascal `Required` flag — not enforced anywhere in
//!   the port yet (same deferral as RegControl/CapControl/Reactor).
//!
//! **One deliberate divergence from Pascal:** when a named generator fails to
//! resolve (missing or disabled), `FGenPointerList.Count < FListSize` and the
//! Pascal `Sample` loop (`for i := 1 to FListSize`) walks past the resolved
//! entries into a NIL `Gen.kWBase` dereference — an upstream crash. This port
//! iterates the resolved subset instead, so it cannot crash; for the generators
//! that *do* resolve the dispatch is numerically identical (same sequential
//! weights, same `TotalWeight` summed over the full `FListSize`). Not pinned by
//! any golden. See `sample_skips_unresolved_gens_without_crash`.
//!
//! Split into submodules (no behavioral change): the struct, its constructor,
//! the dispatch environment trait and the property table live here; the control
//! algorithm (`MakeGenList`/`Sample`/`RecalcElementData`) is in [`compute`], and
//! the `CktElement`/`DssObject` trait impls in [`accessors`].

#[cfg(test)]
mod tests;

mod accessors;
mod compute;

use num_complex::Complex64;

use crate::elements::control::control_elem::{ControlElemData, RefSnapshot};
use crate::elements::traits::ElemId;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TGenDispatcherProp` + the
/// `TCktElementClass` tail).
pub mod prop {
    pub const ELEMENT: usize = 1;
    pub const TERMINAL: usize = 2;
    pub const KWLIMIT: usize = 3;
    pub const KWBAND: usize = 4;
    pub const KVARLIMIT: usize = 5;
    pub const GENLIST: usize = 6;
    pub const WEIGHTS: usize = 7;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 8;
    pub const ENABLED: usize = 9;
    pub const NUM_PROPS: usize = 10; // incl. Like
}

/// `TGenDispatcher.DefineProperties`.
pub fn class_props(_enums: &EnumRegistry) -> ClassProps {
    use prop::*;
    let defs = vec![
        // Pascal `PropertyOffset2 = 0` + WriteByFunction(SetMonitoredElement) +
        // Required: any circuit element by full name. (The Pascal `Required`
        // flag is inert here — not enforced in the port yet, same as the other
        // controls; a missing Element still surfaces via RecalcElementData 372.)
        PropDef::object_ref_any("Element").flags(PropFlags::REQUIRED),
        PropDef::integer("Terminal"),
        PropDef::double("kWLimit"),
        PropDef::double("kWBand"),
        PropDef::double("kvarLimit"),
        PropDef::string_list("GenList"),
        // Pascal `DoubleArrayProperty` with `IndirectCount` over the GenList
        // (`GenDispatcher.pas:140-144`, `PropertyOffset2 = @FListSize`,
        // `PropertyOffset3 = @FGeneratorNameList`): renders `ArrayOrFilePath` +
        // `$dssLength: GenList`. The element count is `FListSize` (kept in sync
        // with the generator-name-list length); the port exposes it via
        // `get_i32(GENLIST)` so the shared DoubleArray count path resolves it.
        PropDef::double_array("Weights", GENLIST),
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
    ClassProps::new("GenDispatcher", defs, true)
}

/// The executive surface `Sample` needs to reach the monitored element and the
/// dispatched generators (the Rust stand-in for Pascal's live object pointers).
/// `ElemId`s returned by the lookups are passed back to the accessors.
pub(crate) trait GenDispatchEnv {
    /// Pascal `MonitoredElement.Power[ElementTerminal]` (complex VA: W + jVAr).
    fn monitored_power(&mut self) -> Complex64;
    /// Pascal `GenClass.Find(name)` restricted to *enabled* generators.
    fn find_enabled_gen(&self, name: &str) -> Option<ElemId>;
    /// Pascal's "scan the whole circuit for enabled generators", creation order.
    fn all_enabled_gens(&self) -> Vec<ElemId>;
    /// `Gen.kWBase` (the public published field; setting it has no side effect,
    /// which is why `Sample` sets `LoadsNeedUpdating` to force a recalc).
    fn gen_kw_base(&self, g: ElemId) -> f64;
    fn set_gen_kw_base(&mut self, g: ElemId, value: f64);
    /// `Gen.kvarBase`.
    fn gen_kvar_base(&self, g: ElemId) -> f64;
    fn set_gen_kvar_base(&mut self, g: ElemId, value: f64);
}

/// `TGenDispatcherObj`.
#[derive(Debug, Clone)]
pub struct GenDispatcher {
    pub ccd: ControlElemData,
    /// Dump name of the monitored element (Pascal renders `FullName`).
    monitored_full_name: String,
    /// Parse-time shape snapshot of the monitored reference.
    mon_snap: Option<RefSnapshot>,

    f_kw_limit: f64,
    f_kw_band: f64,
    half_kw_band: f64,
    f_kvar_limit: f64,
    total_weight: f64,
    /// `FListSize` — the dispatched-generator count (the GenList length once a
    /// list is given, else the enabled-generator count discovered on first
    /// `Sample`).
    list_size: i32,
    /// `FGeneratorNameList`.
    gen_name_list: Vec<String>,
    /// `FWeights` (one per list entry; default 1.0).
    weights: Vec<f64>,
    /// `FGenPointerList` — resolved lazily on the first `Sample` (empty until
    /// then), cached across samples exactly like Pascal.
    gen_pointer_list: Vec<ElemId>,
}

impl GenDispatcher {
    /// Pascal `TGenDispatcherObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;

        let f_kw_limit = 8000.0;
        let f_kw_band = 100.0;
        Self {
            ccd,
            monitored_full_name: String::new(),
            mon_snap: None,
            f_kw_limit,
            f_kw_band,
            half_kw_band: f_kw_band / 2.0,
            f_kvar_limit: f_kw_limit / 2.0,
            total_weight: 1.0,
            list_size: 0,
            gen_name_list: Vec::new(),
            weights: Vec::new(),
            gen_pointer_list: Vec::new(),
        }
    }
}
