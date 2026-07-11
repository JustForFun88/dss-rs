//! Port of `Controls/UPFCControl.pas` — `TUPFCControlObj`, the control element
//! that drives a set of [`Upfc`](crate::elements::pc::upfc::Upfc) devices. On each
//! `Sample` it polls every UPFC's `CheckStatus`; if any needs an update it pushes
//! one present-time control-queue action, whose `DoPendingAction` then calls
//! `UploadCurrents` on every UPFC (recomputing their input/output injection
//! currents from the cached `Vbin`/`Vbout`). `Reset` is a no-op upstream.
//!
//! The UPFC set is a *dynamic* fleet, so — like GenDispatcher / StorageController
//! / InvControl — it is reached through the class registry behind
//! [`UpfcDispatchEnv`]; the control-loop implements it over the store
//! (`solution/controls/dispatch.rs`), and the unit tests against a mock.
//!
//! **Faithful-but-dead upstream code reproduced as a no-op:** Pascal
//! `MakeUPFCList` has a name-list (`ListSize > 0`) branch, but nothing ever sets
//! `ListSize` from the parsed `UPFCList` name list (there is no
//! `PropertySideEffects`), so on a freshly parsed control `ListSize` is always 0
//! and the **else** branch (scan *all* enabled UPFCs, uniform weights) is the only
//! reachable path. The name-list branch additionally reads `FUPFCNameList` right
//! after `.Clear`-ing it (an out-of-bounds upstream bug). We reproduce the
//! reachable behavior: always scan all enabled UPFCs. The `UPFCList=` property
//! therefore round-trips but never actually filters the fleet — matching upstream.

#[cfg(test)]
mod tests;

mod accessors;

use crate::elements::control::control_elem::ControlElemData;
use crate::elements::traits::ElemRef;
use crate::obj::dss_enum::EnumRegistry;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

/// 1-based property ordinals (Pascal `TUPFCControlProp` + the `TCktElementClass`
/// tail).
pub mod prop {
    pub const UPFCLIST: usize = 1;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 2;
    pub const ENABLED: usize = 3;
    pub const NUM_PROPS: usize = 4; // incl. Like
}

/// `TUPFCControl.DefineProperties`.
pub fn class_props(_enums: &EnumRegistry) -> ClassProps {
    let defs = vec![
        PropDef::string_list("UPFCList"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), prop::NUM_PROPS - 1);
    ClassProps::new("UPFCControl", defs, true)
}

/// The executive surface `Sample`/`DoPendingAction` need to reach the controlled
/// UPFC fleet (the Rust stand-in for Pascal's live `UPFCList` object pointers).
pub(crate) trait UpfcDispatchEnv {
    /// Pascal `clsUPFC.Find(name)` restricted to *enabled* UPFCs.
    fn find_enabled_upfc(&self, name: &str) -> Option<ElemRef>;
    /// Pascal's "scan the whole UPFC class for enabled devices", creation order.
    fn all_enabled_upfcs(&self) -> Vec<ElemRef>;
    /// Pascal `TUPFCObj.CheckStatus` on UPFC `u` (computes its `Element=`
    /// `Power[1]` for the PF modes internally, then mutates `UPFCON`/`VRefD`).
    fn check_status(&mut self, u: ElemRef) -> bool;
    /// Pascal `TUPFCObj.UploadCurrents` on UPFC `u`.
    fn upload_currents(&mut self, u: ElemRef);
}

/// `TUPFCControlObj`.
#[derive(Debug, Clone)]
pub struct UpfcControl {
    pub ccd: ControlElemData,

    /// `FUPFCNameList` — the parsed `UPFCList` names (round-trips; never filters
    /// the fleet — see the module note).
    upfc_name_list: Vec<String>,
    /// `FWeights` (one per list entry; default 1.0, uniform).
    weights: Vec<f64>,
    /// `TotalWeight`.
    total_weight: f64,
    /// `ListSize` — the controlled-UPFC count (set to the enabled-UPFC count on
    /// the first `Sample`; never set from the name list, per upstream).
    list_size: i32,
    /// `UPFCList` — resolved lazily on the first `Sample` (empty until then),
    /// cached across samples exactly like Pascal's pointer list.
    upfc_pointer_list: Vec<ElemRef>,
}

impl UpfcControl {
    /// Pascal `TUPFCControlObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        // Pascal `TUPFCControlObj.Create` never sets a terminal shape, so the
        // base `TDSSCktElement.Create` zeros stand (`FNphases := 0`,
        // `CktElement.pas:187`) — unlike the other controls, which set
        // phases/terms. Rust's `CktElementData::new` convenience default is 3;
        // reset it so the Complete dump prints `! NPhases = 0` like the oracle
        // (`dump3_debug` golden).
        ccd.cd.nphases = 0;
        Self {
            ccd,
            upfc_name_list: Vec::new(),
            weights: Vec::new(),
            total_weight: 1.0,
            list_size: 0,
            upfc_pointer_list: Vec::new(),
        }
    }

    /// Pascal `TUPFCControlObj.MakeUPFCList`: scan every enabled UPFC into the
    /// pointer list with uniform weights. Returns whether the list is non-empty.
    /// (The `ListSize > 0` name-list branch is upstream-unreachable; see the module
    /// note. We clear the name list first, mirroring `FUPFCNameList.Clear`.)
    fn make_upfc_list(&mut self, env: &dyn UpfcDispatchEnv) -> bool {
        self.upfc_name_list.clear();
        self.upfc_pointer_list.clear();

        if self.list_size > 0 {
            // Name list path (dead in practice): the cleared name list resolves
            // nothing, matching the upstream clear-then-read no-op.
            let mut refs = Vec::new();
            for name in &self.upfc_name_list {
                if let Some(u) = env.find_enabled_upfc(name) {
                    refs.push(u);
                }
            }
            self.upfc_pointer_list = refs;
        } else {
            // Search the entire circuit for enabled UPFCs.
            self.upfc_pointer_list = env.all_enabled_upfcs();
            self.list_size = self.upfc_pointer_list.len() as i32;
            self.weights = vec![1.0; self.list_size.max(0) as usize];
        }

        self.total_weight = self
            .weights
            .iter()
            .take(self.list_size.max(0) as usize)
            .sum();

        !self.upfc_pointer_list.is_empty()
    }

    /// Pascal `TUPFCControlObj.Sample`: build the list on first call, OR every
    /// UPFC's `CheckStatus`, and return whether any needs an update (the caller
    /// then pushes a present-time control action). The `||` short-circuit mirrors
    /// FPC's default Boolean evaluation: once one UPFC requests an update, the rest
    /// are not polled.
    pub(crate) fn sample(&mut self, env: &mut dyn UpfcDispatchEnv) -> bool {
        // If list is not defined, go make one from all UPFCs in circuit.
        if self.upfc_pointer_list.is_empty() {
            self.make_upfc_list(env);
        }
        if self.list_size <= 0 {
            return false;
        }

        let mut update = false;
        for &u in &self.upfc_pointer_list {
            update = update || env.check_status(u);
        }
        update
    }

    /// Pascal `TUPFCControlObj.DoPendingAction`: `UploadCurrents` on every UPFC.
    pub(crate) fn do_pending_action(&mut self, env: &mut dyn UpfcDispatchEnv) {
        if self.list_size <= 0 {
            return;
        }
        for &u in &self.upfc_pointer_list {
            env.upload_currents(u);
        }
    }
}
