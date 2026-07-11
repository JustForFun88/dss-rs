//! WPG.17 Part B: `Get`/`Set processtime|totaltime|steptime`
//! (`ExecOptions.pas:106-108`, `:683-684`, `:1042-1047`). Only the
//! deterministic round-trip structure is gated — the underlying solve timers
//! are non-deterministic wall-clock (never reproduced; see `Solution` state).
//! Values probed on the pinned oracle (dss-python 0.15.7): a fresh circuit
//! reports `0`; `set totaltime=v` round-trips; `processtime`/`steptime` are
//! Get-only (a `set` is a silent no-op, not an error).

use super::common::dss_with_circuit;
use crate::exec::*;

/// `Get <option>` returns the value in the result buffer (Pascal `DoGetCmd`).
fn get_opt(dss: &mut Dss, opt: &str) -> String {
    dss.command(&format!("get {opt}"));
    dss.result().to_string()
}

#[test]
fn timing_options_are_recognized_and_default_zero() {
    let mut dss = dss_with_circuit();
    for opt in ["totaltime", "processtime", "steptime"] {
        assert_eq!(get_opt(&mut dss, opt), "0", "fresh {opt}");
        assert!(
            dss.errors().is_empty(),
            "'{opt}' must be a recognized option, got {:?}",
            dss.errors()
        );
    }
}

#[test]
fn set_totaltime_round_trips() {
    let mut dss = dss_with_circuit();
    dss.command("set totaltime=123.5");
    assert!(dss.errors().is_empty(), "{:?}", dss.errors());
    assert_eq!(get_opt(&mut dss, "totaltime"), "123.5");

    // Pascal `set totaltime=0` resets the accumulator (the ckt24 idiom).
    dss.command("set totaltime=0");
    assert_eq!(get_opt(&mut dss, "totaltime"), "0");
}

#[test]
fn set_processtime_and_steptime_are_getonly_noops() {
    let mut dss = dss_with_circuit();
    // Pascal has no Set arm for 106/108 → the case falls to
    // `else // Ignore excess parameters`: a silent no-op, NOT an error.
    dss.command("set processtime=99");
    dss.command("set steptime=42");
    assert!(
        dss.errors().is_empty(),
        "set processtime/steptime must be silent no-ops, got {:?}",
        dss.errors()
    );
    assert_eq!(get_opt(&mut dss, "processtime"), "0");
    assert_eq!(get_opt(&mut dss, "steptime"), "0");
}
