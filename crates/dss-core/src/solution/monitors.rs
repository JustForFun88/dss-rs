//! Monitor class-level sweeps — Pascal `TDSSMonitor.SampleAll` /
//! `SampleAllMode5` / `ResetAll` (`Meters/Monitor.pas`). Implemented as free
//! functions over the registry (the WP5.7 control-sweep pattern): each monitor
//! and its metered element are borrowed disjointly via
//! [`TypedStore::typed_obj_pair_mut`].

use crate::circuit::Circuit;
use crate::elements::meter::monitor::{Monitor, MonitorSampleCtx};
use crate::elements::traits::TypedStore;
use crate::solution::solution::{SolveEnv, sys_ctx};

/// Build the per-sample solution context from the current solution state.
fn sample_ctx(ckt: &Circuit) -> MonitorSampleCtx {
    let s = &ckt.solution;
    MonitorSampleCtx {
        int_hour: s.int_hour,
        t: s.t,
        is_harmonic: s.is_harmonic_model,
        frequency: s.frequency,
        harmonic: s.harmonic,
        iteration: s.iteration,
        control_iteration: s.control_iteration,
        max_iterations: s.max_iterations,
        max_control_iterations: s.max_control_iterations,
        converged: s.converged_flag,
        interval_hrs: s.interval_hrs,
        solution_count: s.solution_count,
        mode_ordinal: s.mode.ordinal(),
        year: s.year,
        // Wall-clock timings are not reproducible; the port records 0.
        solve_time_us: 0.0,
        step_time_us: 0.0,
    }
}

/// Pascal `TDSSMonitor.SampleAll` (`mode5_only = false`, samples every enabled
/// monitor whose mode ≠ 5) / `SampleAllMode5` (`mode5_only = true`, samples the
/// mode-5 monitors). Walks `ckt.monitors` in creation order.
pub(crate) fn sample_all_monitors(ckt: &mut Circuit, env: &mut SolveEnv, mode5_only: bool) {
    let sys = sys_ctx(ckt);
    let ctx = sample_ctx(ckt);
    let monitors = ckt.monitors.clone();

    for mon_ref in monitors {
        let (mode, enabled, metered) = {
            let m = env
                .store
                .typed::<Monitor>(mon_ref)
                .expect("ckt.monitors holds Monitor objects");
            (m.mode_raw(), m.med.cd.enabled, m.med.metered_element)
        };
        if !enabled {
            continue;
        }
        // SampleAll samples mode ≠ 5; SampleAllMode5 samples mode = 5.
        if mode5_only != (mode == 5) {
            continue;
        }
        let Some(metered_ref) = metered else {
            continue;
        };
        let (m, metered_obj) = env
            .store
            .typed_obj_pair_mut::<Monitor>(mon_ref, metered_ref);
        m.take_sample(metered_obj, &ckt.solution.node_v, &sys, &ctx);
    }
}

/// Pascal `TDSSMonitor.SaveAll` (`Meters/Monitor.pas:422`): flush every
/// enabled monitor's pending buffer (`TMonitorObj.Save`) — no mode filter
/// (unlike `SampleAll`, which skips mode-5 monitors). Called by every ported
/// ordinary solve mode at its natural end (`SolveDaily`/`SolveYearly`/
/// `SolveDuty`/`SolveDynamic`/`SolveHarmonic`/`SolveHarmonicT`) — deliberately
/// **not** called by `SolveGeneralTime` ("roll your own", WPG.2) or
/// `SolveFaultStudy` (never samples monitors at all).
pub(crate) fn save_all_monitors(ckt: &mut Circuit, env: &mut SolveEnv) {
    for mon_ref in ckt.monitors.clone() {
        let m = env
            .store
            .typed_mut::<Monitor>(mon_ref)
            .expect("ckt.monitors holds Monitor objects");
        if m.med.cd.enabled {
            m.save();
        }
    }
}

/// Pascal `TDSSMonitor.ResetAll`: clear every enabled monitor's buffer and
/// rebuild its header. `IsHarmonicModel` selects the time-column labels
/// (`Freq`/`Harmonic` vs `hour`/`t(sec)`), so entering harmonics mode (the
/// `Set mode=harmonics` reset) relabels them.
pub(crate) fn reset_all_monitors(ckt: &mut Circuit, env: &mut SolveEnv) {
    let is_harmonic = ckt.solution.is_harmonic_model;
    let monitors = ckt.monitors.clone();
    for mon_ref in monitors {
        let m = env
            .store
            .typed_mut::<Monitor>(mon_ref)
            .expect("ckt.monitors holds Monitor objects");
        if m.med.cd.enabled {
            m.reset_it(is_harmonic);
        }
    }
}
