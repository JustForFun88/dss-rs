//! `Export EventLog` / `Export ErrorLog` (Pascal `ExportEventLog` /
//! `ExportErrorLog`, `ExportResults.pas:3296`/`:3303`): dump the accumulated
//! `DSS.EventStrings` / `DSS.ErrorStrings` to a file, one entry per line
//! (`TStringList.SaveToFile` — no header, every line terminated, an empty file
//! when the list is empty). No solve read: the strings are the running logs the
//! solution/control loop (`EventStrings`) and `DoSimpleMsg` (`ErrorStrings`)
//! accumulate across the run.

/// `TStringList.SaveToFile`: each entry on its own line, every line terminated
/// (including the last), an empty string when the list is empty. The Phase-8
/// comparator strips trailing newlines, so the terminator style is not
/// load-bearing — this stays faithful to the Pascal `GetTextStr` anyway.
fn save_string_list(entries: &[String]) -> String {
    let mut out = String::new();
    for e in entries {
        out.push_str(e);
        out.push('\n');
    }
    out
}

/// `ExportEventLog` — dump `DSS.EventStrings`: the `Hour=…, Sec=…, Iteration=…,
/// …` control-iteration markers and the controls' `AppendToEventLog` tap/step
/// action lines the solve loop accumulates.
pub(crate) fn export_event_log(entries: &[String]) -> String {
    save_string_list(entries)
}

/// `ExportErrorLog` — dump `DSS.ErrorStrings`: the `DoSimpleMsg` record-and-
/// continue messages. Rust's `Dss::errors` is that same accumulating log.
pub(crate) fn export_error_log(entries: &[String]) -> String {
    save_string_list(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_list_is_empty_file() {
        assert_eq!(export_event_log(&[]), "");
        assert_eq!(export_error_log(&[]), "");
    }

    #[test]
    fn each_entry_on_its_own_terminated_line() {
        let entries = vec![
            "Hour=0, Sec=0, Iteration=3, ControlIter=1, Event=Control Iteration 1".to_string(),
            "Hour=0, Sec=0, ControlIter=2, Element=Regulator.reg1, Action=CHANGED 3 TAPS TO 1.05."
                .to_string(),
        ];
        let out = export_event_log(&entries);
        assert_eq!(out.lines().count(), 2);
        assert!(out.ends_with(".\n"));
        assert_eq!(out, format!("{}\n{}\n", entries[0], entries[1]));
    }
}
