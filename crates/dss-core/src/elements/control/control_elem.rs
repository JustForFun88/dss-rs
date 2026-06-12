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

use crate::elements::ckt::CktElementData;
use crate::elements::traits::ElemRef;

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
