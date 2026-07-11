//! Port of `Controls/ExpControl.pas` — `TExpControlObj`, the adaptive
//! dynamic-`Vreg` smart-inverter control over a fleet of PVSystem elements
//! (PHASE7_PLAN WP7.5 step 3). Upstream note: *"adapted and simplified from
//! InvControl for adaptive controller research."*
//!
//! ExpControl runs a single volt-var-like law with an **adaptive voltage
//! regulation reference** (`Vreg`) that the controller slews toward the present
//! bus voltage over a time constant (`VregTau`): each control iteration sets the
//! per-DER kvar from the slope crossing at `Vreg` plus a `Qbias`, clamps it to the
//! inverter headroom / `QmaxLead` / `QmaxLag`, optionally curtails kW
//! (`PreferQ`), low-pass filters the target (`Tresponse` → `FOpenTau`), and moves
//! it by `DeltaQ_Factor`. After each converged power-flow step the `UpdateAll`
//! hook (`UpdateExpControl`) advances `Vreg` toward the bus voltage and writes it
//! back as PVSystem state variable 5.
//!
//! Like every `TControlElem`, an ExpControl builds **no Yprim** and its terminal
//! currents are zero. Its single terminal attaches to the first controlled
//! PVSystem's bus (Pascal `Setbus(1, MonitoredElement.Firstbus)`), resolved at
//! parse-time edit-completion (the executive lacks store access in
//! `recalc_element_data`, mirroring [`InvControl`](super::inv_control)).
//!
//! The PVSystem fleet (`FPVSystemPointerList` → `ControlledElement`) resolves
//! lazily on the first `Sample` through the [`ExpDispatchEnv`](compute::ExpDispatchEnv)
//! abstraction (`solution/controls/dispatch.rs`), resolved against the class
//! registry — the [`StorageController`](super::storage_controller) / `InvControl`
//! pattern. **ExpControl controls only PVSystem** (never Storage), so the env is
//! PVSystem-typed.
//!
//! `MakePosSequence` is ported (WPG.21) as a NIL-deref-safe partial: the defined
//! `FNphases := 3; Nconds := 3` resync plus the resolved-DER bus adopt, with the
//! empty-list `MonitoredElement` deref safe-skipped (Access violation,
//! `docs/wpg21_makeposseq_probes.md`; CLAUDE.md forbids reproducing it).

mod accessors;
mod compute;
#[cfg(test)]
mod tests;

pub(crate) use compute::{ExpDispatchEnv, PvFind, PvSnap};

use crate::elements::control::control_elem::ControlElemData;
use crate::elements::traits::ElemRef;
use crate::obj::props::{ClassProps, PropDef, PropFlags};

// PendingChange action codes (ExpControl.pas l.169-170).
pub(crate) const NONE: i32 = 0;
pub(crate) const CHANGEVARLEVEL: i32 = 1;

/// 1-based property ordinals (Pascal `TExpControlProp` + the `TCktElementClass`
/// tail). The legacy and modern Pascal names differ only in case, so the
/// case-insensitive property matcher accepts both spellings; the modern
/// (`TProp`) names below are what the text dump renders.
pub mod prop {
    pub const PVSYSTEM_LIST: usize = 1;
    pub const VREG: usize = 2;
    pub const SLOPE: usize = 3;
    pub const VREG_TAU: usize = 4;
    pub const QBIAS: usize = 5;
    pub const VREG_MIN: usize = 6;
    pub const VREG_MAX: usize = 7;
    pub const QMAX_LEAD: usize = 8;
    pub const QMAX_LAG: usize = 9;
    pub const EVENT_LOG: usize = 10;
    pub const DELTA_Q_FACTOR: usize = 11;
    pub const PREFER_Q: usize = 12;
    pub const TRESPONSE: usize = 13;
    pub const DER_LIST: usize = 14;
    // TCktElementClass tail:
    pub const BASE_FREQ: usize = 15;
    pub const ENABLED: usize = 16;
    pub const NUM_PROPS: usize = 17; // incl. Like
}

/// `TExpControl.DefineProperties`.
pub fn class_props() -> ClassProps {
    use prop::*;
    let defs = vec![
        // PVSystemList (bare names) + DERList (class-prefixed) are kept in sync by
        // the side effects; they share no backing (distinct StringLists upstream).
        PropDef::string_list("PVSystemList"),
        // Vreg → FVregInit (IgnoreInvalid + NonNegative).
        PropDef::double("VReg").flags(PropFlags::IGNORE_INVALID | PropFlags::NON_NEGATIVE),
        // Slope → QVSlope (IgnoreInvalid + NonNegative + NonZero).
        PropDef::double("Slope")
            .flags(PropFlags::IGNORE_INVALID | PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        // VregTau (IgnoreInvalid + NonNegative; Pascal Units_s is JSON-only).
        PropDef::double("VRegTau").flags(PropFlags::IGNORE_INVALID | PropFlags::NON_NEGATIVE),
        PropDef::double("QBias"),
        PropDef::double("VRegMin").flags(PropFlags::IGNORE_INVALID | PropFlags::NON_NEGATIVE),
        PropDef::double("VRegMax").flags(PropFlags::IGNORE_INVALID | PropFlags::NON_NEGATIVE),
        PropDef::double("QMaxLead")
            .flags(PropFlags::IGNORE_INVALID | PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::double("QMaxLag")
            .flags(PropFlags::IGNORE_INVALID | PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::boolean("EventLog"),
        PropDef::double("DeltaQ_Factor"),
        PropDef::boolean("PreferQ"),
        PropDef::double("TResponse")
            .flags(PropFlags::IGNORE_INVALID | PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::string_list("DERList"),
        // TCktElementClass tail:
        PropDef::double("BaseFreq").flags(PropFlags::NON_NEGATIVE | PropFlags::NON_ZERO),
        PropDef::enabled("Enabled"),
    ];
    debug_assert_eq!(defs.len(), NUM_PROPS - 1);
    ClassProps::new("ExpControl", defs, true)
}

/// Pascal's per-controlled-PVSystem runtime arrays (`FPriorVpu`/`FPresentVpu`/
/// `FPendingChange`/`FLastIterQ`/`FLastStepQ`/`FTargetQ`/`FWithinTol`/`FVregs`),
/// gathered into one record per fleet member (1:1 with `fleet`).
#[derive(Debug, Clone, Default)]
pub(crate) struct ExpVars {
    /// `FPriorVpu` — the prior control-iteration per-unit voltage (the `Verr`
    /// reference; refreshed at the end of each `DoPendingAction`).
    pub f_prior_vpu: f64,
    /// `FPresentVpu` — the present-sample per-unit voltage.
    pub f_present_vpu: f64,
    /// `FPendingChange` — the queued action code for this DER.
    pub f_pending_change: i32,
    /// `FLastIterQ` — the prior control-iteration kvar (the DeltaQ_Factor base;
    /// starts at -1.0).
    pub f_last_iter_q: f64,
    /// `FLastStepQ` — the prior *time-step* kvar (the `FOpenTau` filter base;
    /// starts at -1.0, refreshed by `UpdateExpControl`).
    pub f_last_step_q: f64,
    /// `FTargetQ` — the target kvar for this DER.
    pub f_target_q: f64,
    /// `FWithinTol` — whether the DER was within the Verr/Qerr tolerance band.
    pub f_within_tol: bool,
    /// `FVregs` — the adaptive voltage-regulation reference (slewed by `VregTau`).
    pub f_vregs: f64,
}

impl ExpVars {
    /// Pascal `MakePVSystemList`'s per-DER initialization block: prior/present 0,
    /// last-iter/last-step kvar -1, target 0, not within tol, `Vreg := FVregInit`,
    /// pending NONE.
    fn new(f_vreg_init: f64) -> Self {
        Self {
            f_prior_vpu: 0.0,
            f_present_vpu: 0.0,
            f_pending_change: NONE,
            f_last_iter_q: -1.0,
            f_last_step_q: -1.0,
            f_target_q: 0.0,
            f_within_tol: false,
            f_vregs: f_vreg_init,
        }
    }
}

/// `TExpControlObj`. The parse-time property surface plus the WP7.5 step-3
/// runtime state: the resolved PVSystem fleet (`fleet`), the per-DER [`ExpVars`]
/// (`ctrl_vars`), and the derived open-loop time constant (`f_open_tau`).
#[derive(Debug, Clone)]
pub struct ExpControl {
    pub ccd: ControlElemData,

    /// `FPVSystemNameList` — the bare PVSystem names (PVSystemList backing).
    pvsystem_name_list: Vec<String>,
    /// `DERNameList` — the class-prefixed names (`PVSystem.<n>`; DERList backing).
    der_name_list: Vec<String>,
    /// `FListSize` — set from the active list's count in the side effect.
    f_list_size: i32,

    /// `FVregInit` (Vreg) — the initial / target regulation voltage (0 ⇒ find it
    /// during initialization, in static mode).
    f_vreg_init: f64,
    /// `QVSlope` (Slope) — the volt-var slope (originally `FSlope`).
    q_v_slope: f64,
    /// `VregTau` — the `Vreg` slew time constant (s).
    vreg_tau: f64,
    /// `FQbias` (Qbias) — the constant kvar (pu) bias added to the slope crossing.
    f_qbias: f64,
    /// `VregMin` / `VregMax` — the `Vreg` clamp band.
    vreg_min: f64,
    vreg_max: f64,
    /// `QmaxLead` / `QmaxLag` — the kvar (pu) lead / lag limits.
    qmax_lead: f64,
    qmax_lag: f64,
    /// `FdeltaQ_factor` (DeltaQ_Factor) — the per-iteration convergence damping.
    f_delta_q_factor: f64,
    /// `FPreferQ` (PreferQ) — prefer reactive power over active (curtail kW).
    f_prefer_q: bool,
    /// `Tresponse` — the open-loop response time (s); drives `FOpenTau`.
    tresponse: f64,
    /// `FOpenTau` = `Tresponse / 2.3026` — the open-loop low-pass time constant
    /// (derived in `RecalcElementData`).
    f_open_tau: f64,
    /// `FVoltageChangeTolerance` / `FVarChangeTolerance` — the Sample trigger
    /// bands. Pascal hard-codes both to 1e-4 in `Create` with "no user adjustment"
    /// (not properties); carried for `MakeLike` fidelity.
    f_voltage_change_tolerance: f64,
    f_var_change_tolerance: f64,

    // --- WP7.5 step-3 runtime state (the PVSystem fleet + dispatch) ---
    /// `FPVSystemPointerList` → `ControlledElement` — the resolved PVSystem fleet,
    /// built lazily on the first `Sample` (empty until then). An empty fleet
    /// re-triggers the build (Pascal `FPVSystemPointerList.Count = 0`); a
    /// PVSystemList / DERList edit clears it (`invalidate_fleet`).
    pub(crate) fleet: Vec<ElemRef>,
    /// One [`ExpVars`] per fleet member (1:1 with `fleet`).
    ctrl_vars: Vec<ExpVars>,

    // --- parse-time resolved monitored-DER bus (Pascal `Setbus(1,
    // MonitoredElement.Firstbus)`; the executive resolves it at edit-completion
    // since `recalc_element_data` has no store access) ---
    /// The first PVSystem's `Firstbus` (the bus the control's terminal attaches to).
    mon_bus: String,
    /// The (last) PVSystem's phase count (Pascal `FNphases :=
    /// ControlledElement[i].NPhases` over the recalc loop); the control's `NConds`
    /// follows.
    mon_nphases: usize,
    /// Whether [`set_resolved_monitored`](Self::set_resolved_monitored) ran (a fleet
    /// member was found at parse time).
    mon_resolved: bool,
}

impl ExpControl {
    /// Pascal `TExpControlObj.Create`.
    pub fn new(name: &str) -> Self {
        let mut ccd = ControlElemData::new(name, prop::NUM_PROPS);
        ccd.cd.nphases = 3; // directly set conds and phases
        ccd.cd.nconds = 3;
        ccd.cd.set_nterms(1); // forces allocation of terminals and conductors
        ccd.element_terminal = 1;
        ccd.show_event_log = false;

        Self {
            ccd,
            pvsystem_name_list: Vec::new(),
            der_name_list: Vec::new(),
            f_list_size: 0,

            f_vreg_init: 1.0, // 0 means to find it during initialization
            q_v_slope: 50.0,
            vreg_tau: 1200.0,
            f_qbias: 0.0,
            vreg_min: 0.95,
            vreg_max: 1.05,
            qmax_lead: 0.44,
            qmax_lag: 0.44,
            f_delta_q_factor: 0.7, // only on control iterations, not the final solution
            f_prefer_q: false,
            tresponse: 0.0,
            f_open_tau: 0.0,
            f_voltage_change_tolerance: 0.0001,
            f_var_change_tolerance: 0.0001,

            fleet: Vec::new(), // empty → the first Sample builds it
            ctrl_vars: Vec::new(),

            mon_bus: String::new(),
            mon_nphases: 3,
            mon_resolved: false,
        }
    }

    /// The executive resolves the first PVSystem's bus at edit-completion (Pascal
    /// `RecalcElementData` runs `MakePVSystemList` + `Setbus(1,
    /// MonitoredElement.Firstbus)`, but the Rust `recalc_element_data` has no store
    /// access). Called from `exec::command` with the resolved first-DER bus + the
    /// (last) DER's phase count; `end_edit`/[`recalc`](Self::recalc) then attaches
    /// the terminal.
    pub(crate) fn set_resolved_monitored(&mut self, bus: String, nphases: usize) {
        self.mon_bus = bus;
        self.mon_nphases = nphases;
        self.mon_resolved = true;
    }

    /// The current bare PVSystem name list (consumed by the executive's parse-time
    /// fleet-bus resolution).
    pub(crate) fn pvsystem_name_list(&self) -> &[String] {
        &self.pvsystem_name_list
    }

    // --- Read-only accessors for the CIM `TIEEE1547Controller` export (WPG.18
    // Stage F, `ExportCIMXML.pas` `PullFromExpControl`). No behavior change. ---

    /// `DERNameList` — the class-prefixed controlled-DER names.
    pub(crate) fn der_name_list(&self) -> &[String] {
        &self.der_name_list
    }
    /// `QMaxLead` — the kvar (pu) lead limit (drives the catB estimate).
    pub(crate) fn qmax_lead(&self) -> f64 {
        self.qmax_lead
    }
    /// `QMaxLag` — the kvar (pu) lag limit.
    pub(crate) fn qmax_lag(&self) -> f64 {
        self.qmax_lag
    }
    /// `VregTau` — the `Vreg` slew time constant (s).
    pub(crate) fn vreg_tau(&self) -> f64 {
        self.vreg_tau
    }
    /// `Tresponse` — the open-loop response time (s).
    pub(crate) fn tresponse(&self) -> f64 {
        self.tresponse
    }
    /// `QVSlope` (Slope) — the volt-var slope.
    pub(crate) fn q_v_slope(&self) -> f64 {
        self.q_v_slope
    }
}
