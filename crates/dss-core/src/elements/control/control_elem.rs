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
