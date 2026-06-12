//! Port of the OpenDSS event log — the two producers that grow
//! `DSS.EventStrings` (`Common/DSSClass.pas`):
//!
//! - [`EventLog::log_this_event`] = `TDSSContext.LogThisEvent` (gated by
//!   `ckt.LogEvents`, i.e. `Set LogEvents=yes`), used by the solution loop to
//!   record `Control Iteration N` markers (`Solution.pas` l.1147);
//! - [`EventLog::append`] = `TDSSObject.AppendToEventLog` (gated by a control's
//!   per-device `EventLog` property), used by RegControl/CapControl to record
//!   tap changes and capacitor steps with the exact upstream `Format` strings.
//!
//! dss-python reads the same list through `Solution.EventLog`, so the field
//! lives on [`crate::solution::Solution`] and is surfaced via `Dss::event_log()`.
//! The gate compares **normalized** strings (numbers parsed out, like
//! `props_roundtrip`'s numeric skeleton), so the `%g` rendering of the time
//! fields is faithful-but-not-load-bearing (see [`crate::util::fmt_g`]).

use crate::util::fmt_g;

/// `DSS.EventStrings` (`TStringList`) — the accumulated event-log lines.
#[derive(Debug, Clone, Default)]
pub struct EventLog {
    strings: Vec<String>,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
        }
    }

    /// `DSS.EventStrings.Clear` (the `ClearEventLog` path).
    pub fn clear(&mut self) {
        self.strings.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// The raw lines, in insertion order.
    pub fn entries(&self) -> &[String] {
        &self.strings
    }

    /// Pascal `TDSSContext.LogThisEvent`:
    /// `Format('Hour=%d, Sec=%-.8g, Iteration=%d, ControlIter=%d, Event=%s', …)`.
    /// `hour`/`sec` are `DynaVars.intHour`/`DynaVars.t`; `iteration` is the
    /// power-flow iteration, `control_iter` the control iteration.
    pub fn log_this_event(
        &mut self,
        name: &str,
        hour: i32,
        sec: f64,
        iteration: i32,
        control_iter: i32,
    ) {
        self.strings.push(format!(
            "Hour={}, Sec={}, Iteration={}, ControlIter={}, Event={}",
            hour,
            fmt_g(sec, 8),
            iteration,
            control_iter,
            name
        ));
    }

    /// Pascal `TDSSObject.AppendToEventLog`:
    /// `Format('Hour=%d, Sec=%-.5g, ControlIter=%d, Element=%s, Action=%s', …)`
    /// with `AnsiUpperCase(action)`. `opdev` is the control's `FullName`
    /// (`Class.name`).
    pub fn append(&mut self, opdev: &str, action: &str, hour: i32, sec: f64, control_iter: i32) {
        self.strings.push(format!(
            "Hour={}, Sec={}, ControlIter={}, Element={}, Action={}",
            hour,
            fmt_g(sec, 5),
            control_iter,
            opdev,
            // AnsiUpperCase is single-byte; control names/actions are ASCII.
            action.to_uppercase()
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_this_event_matches_pascal_format() {
        let mut log = EventLog::new();
        log.log_this_event("Control Iteration 1", 0, 0.0, 3, 1);
        assert_eq!(
            log.entries()[0],
            "Hour=0, Sec=0, Iteration=3, ControlIter=1, Event=Control Iteration 1"
        );
    }

    #[test]
    fn append_uppercases_action_and_keeps_element_case() {
        let mut log = EventLog::new();
        log.append("Regulator.reg1", "Changed 3 taps to 1.05.", 0, 0.0, 2);
        assert_eq!(
            log.entries()[0],
            "Hour=0, Sec=0, ControlIter=2, Element=Regulator.reg1, Action=CHANGED 3 TAPS TO 1.05."
        );
    }
}
