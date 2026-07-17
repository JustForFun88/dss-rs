//! Port of `Controls/ControlElem.pas` — `TControlElem`, the shared base of
//! every control device (RegControl, CapControl, ...). A control element is a
//! `TDSSCktElement` with **no** Yprim: it monitors one circuit element and acts
//! on another (often via the control queue), but it never stamps admittance and
//! its terminal currents are always zero.
//!
//! Phase 4 ports the *parse-time* surface only: the property state, the
//! reference resolution, and `RecalcElementData`'s bus/phase setup (so the
//! control participates in `ProcessBusDefs` without changing node order). The
//! behavioral `Sample`/`DoPendingAction` machinery and the control queue arrive
//! in Phase 5 (PHASE4_PLAN §5).

use num_complex::Complex64;

use crate::elements::ckt::CktElementData;
use crate::elements::traits::{ElemRef, SysCtx};
use crate::solution::{ControlQueue, EventLog};

/// Action codes shared across controls (`Controls/ControlElem.pas`
/// `EControlAction`). RegControl uses its own `ACTION_TAPCHANGE`/`ACTION_REVERSE`
/// ordinals; CapControl drives steps through `CTRL_OPEN`/`CTRL_CLOSE`; SwtControl
/// additionally uses `CTRL_LOCK`/`CTRL_UNLOCK`.
pub const CTRL_NONE: i32 = 0;
pub const CTRL_OPEN: i32 = 1;
pub const CTRL_CLOSE: i32 = 2;
pub const CTRL_RESET: i32 = 3;
pub const CTRL_LOCK: i32 = 4;
pub const CTRL_UNLOCK: i32 = 5;

/// Sentinel returned by the Relay `Normal`/`State`/`Action` enums for a token
/// whose first character is neither `o` nor `c` (Pascal `InterpretRelayState`,
/// Relay.pas r4133: the `case LowerCase(param)[1]` has only `'o'`/`'c'` arms, so
/// any other spelling — `trip`, `xyz`, ... — leaves the phase's state array slot
/// *unchanged*, silently). The relay state-array setter treats this ordinal as
/// "keep the prior value" for that phase; it is never stored or rendered.
pub const CTRL_STATE_KEEP: i32 = i32::MIN;

/// Scalar/queue/event context handed to a control's `Sample` and
/// `DoPendingAction` (PHASE5_PLAN §2.1) — the disjoint-borrow stand-in for the
/// Pascal `ActiveCircuit.Solution.*` / `ActiveCircuit.ControlQueue` /
/// `DSS.EventStrings` global reach. The *controlled/monitored* circuit elements
/// are passed to each control's method separately (their concrete types differ
/// per control), so this context carries only the shared state.
pub struct CtrlCtx<'a> {
    /// `Solution.NodeV` (slot 0 = ground).
    pub node_v: &'a [Complex64],
    /// Snapshot of the circuit/solution scalars.
    pub sys: &'a SysCtx,
    /// `ckt.ControlQueue`.
    pub queue: &'a mut ControlQueue,
    /// `DSS.EventStrings`.
    pub events: &'a mut EventLog,
    /// `DoSimpleMsg` sink.
    pub errors: &'a mut Vec<String>,
    /// Raised when an action invalidates Y (tap change / capacitor step); the
    /// control loop copies it into `Solution.SystemYChanged`.
    pub system_y_changed: &'a mut bool,
    /// `Solution.ControlMode` (CTRLSTATIC / EVENTDRIVEN / TIMEDRIVEN / MULTIRATE).
    pub control_mode: i32,
    /// `Solution.DynaVars.intHour` / `.t` / `.dblHour`.
    pub int_hour: i32,
    pub t: f64,
    pub dbl_hour: f64,
    /// `Solution.ControlIteration` (event-log field).
    pub control_iter: i32,
    /// This control's own [`ElemRef`] (Pascal passes `Self` to `ControlQueue.Push`).
    pub self_ref: ElemRef,
}

/// `TControlElem` shared state (the base-class fields every control carries).
/// Embeds [`CktElementData`] exactly as `TControlElem` extends
/// `TDSSCktElement`.
#[derive(Debug, Clone)]
pub struct ControlElemData {
    pub cd: CktElementData,
    /// `ElementTerminal`: 1-based terminal of the monitored/controlled element.
    pub element_terminal: i32,
    /// `FControlledElement` (the device this control acts on).
    pub controlled_element: Option<ElemRef>,
    /// `FMonitoredElement` (the device this control samples).
    pub monitored_element: Option<ElemRef>,
    /// `TimeDelay` (s) and `DblTraceParameter` (debug-trace scratch).
    pub time_delay: f64,
    pub dbl_trace_param: f64,
    /// `ShowEventLog` (defaults to `DSS.EventLogDefault`, which is `False`).
    pub show_event_log: bool,
}

/// Which concrete control class a control object is — the behavior-trait
/// replacement for the `as_any().downcast_ref::<…>()` identification chain in
/// `solution/controls/dispatch.rs`. Returned by [`ControlElem::control_kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlClass {
    Reg,
    Cap,
    Swt,
    Fuse,
    Recloser,
    Relay,
    GenDispatch,
    StorageCtrl,
    Inv,
    Exp,
    Upfc,
    Espvl,
}

impl ControlClass {
    /// The display name the dispatcher prints in `Error Sampling Control
    /// Device "<name>"` diagnostics — byte-identical to the old per-arm
    /// `format!("RegControl.{…}", …)` prefixes (note `UPFCControl` /
    /// `ESPVLControl` / `GenDispatcher` differ from the bare class idents).
    pub fn display_name(self) -> &'static str {
        match self {
            ControlClass::Reg => "RegControl",
            ControlClass::Cap => "CapControl",
            ControlClass::Swt => "SwtControl",
            ControlClass::Fuse => "Fuse",
            ControlClass::Recloser => "Recloser",
            ControlClass::Relay => "Relay",
            ControlClass::GenDispatch => "GenDispatcher",
            ControlClass::StorageCtrl => "StorageController",
            ControlClass::Inv => "InvControl",
            ControlClass::Exp => "ExpControl",
            ControlClass::Upfc => "UPFCControl",
            ControlClass::Espvl => "ESPVLControl",
        }
    }
}

/// The shared behavior surface of every control element (`TControlElem`): its
/// [`ControlElemData`] base state, its concrete [`ControlClass`], and the
/// control-only `Reset`. Acquired from a [`DssObject`] via
/// `as_control()`/`as_control_mut()`, this replaces the `dispatch.rs`
/// identification downcast chain (R0); the per-control `Sample`/`Action`
/// behavioral entry points keep their concrete signatures until R2 hands
/// dispatch typed-arena pair access.
///
/// [`DssObject`]: crate::obj::base::DssObject
pub trait ControlElem {
    /// The `TControlElem` base state (controlled/monitored refs, terminal, …).
    fn ccd(&self) -> &ControlElemData;
    fn ccd_mut(&mut self) -> &mut ControlElemData;
    /// Which concrete control class this is.
    fn control_kind(&self) -> ControlClass;
    /// Reset only the control's own state — the `Reset` path taken when the
    /// controlled element is unset (Pascal `Reset` with no controlled handle).
    /// Only the generic-controlled protection controls (SwtControl / Recloser /
    /// Relay) reach this arm; the default is unreachable for the rest.
    fn reset_control_side(&mut self) {
        unreachable!("reset_control_side: control has no None-controlled reset path");
    }
}

impl ControlElemData {
    /// Pascal `TControlElem.Create`: zero delay/trace, no event log, no
    /// controlled element yet.
    pub fn new(name: &str, num_props: usize) -> Self {
        Self {
            cd: CktElementData::new(name, num_props),
            element_terminal: 1,
            controlled_element: None,
            monitored_element: None,
            time_delay: 0.0,
            dbl_trace_param: 0.0,
            show_event_log: false,
        }
    }
}

/// A read-only snapshot of a controlled/monitored circuit element, captured
/// when the `transformer=`/`element=`/`capacitor=` reference is resolved during
/// parsing. Pascal reads the live element through a pointer in
/// `RecalcElementData`; here that runs in `EndEdit`, after the foreign-class
/// view has gone out of scope, so the control caches what it needs at
/// resolution time. Nothing edits the referenced element between resolution and
/// `EndEdit` in practice (the IEEE masters always define the target fully
/// before the control), so the cache matches a live read; editing the target's
/// shape *afterwards* leaves the control stale in Pascal too (its bus string is
/// only refreshed by its own next `RecalcElementData`).
#[derive(Debug, Clone, Default)]
pub struct RefSnapshot {
    /// `FullName` (`Class.name`) for error messages and dumps.
    pub full_name: String,
    pub nphases: usize,
    pub nterms: usize,
    /// 1-based terminal bus names, stored 0-based (`buses[term - 1]`).
    pub buses: Vec<String>,
}

impl RefSnapshot {
    /// Capture the parse-relevant shape of a referenced circuit element.
    pub fn capture(full_name: String, elem: &dyn crate::elements::traits::CktElement) -> Self {
        let cd = elem.cd();
        Self {
            full_name,
            nphases: cd.nphases,
            nterms: cd.nterms,
            buses: (1..=cd.nterms).map(|i| cd.get_bus(i).to_string()).collect(),
        }
    }
}
