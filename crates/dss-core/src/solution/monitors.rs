//! Monitor class-level sweeps — Pascal `TDSSMonitor.SampleAll` /
//! `SampleAllMode5` / `ResetAll` (`Meters/Monitor.pas`). Implemented as free
//! functions over the registry (the WP5.7 control-sweep pattern): each monitor
//! and its metered element are borrowed disjointly via [`ElemStore::pair_mut`].

use crate::circuit::Circuit;
use crate::elements::meter::monitor::{Monitor, MonitorSampleCtx};
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
            let obj = env.store.obj(mon_ref);
            let m = obj
                .as_any()
                .downcast_ref::<Monitor>()
                .expect("ckt.monitors holds Monitor objects");
            (m.mode, m.med.cd.enabled, m.med.metered_element)
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
        let (mon_obj, metered_obj) = env.store.pair_mut(mon_ref, metered_ref);
        let m = mon_obj
            .as_any_mut()
            .downcast_mut::<Monitor>()
            .expect("monitor downcast");
        m.take_sample(metered_obj, &ckt.solution.node_v, &sys, &ctx);
    }
}

/// Pascal `TDSSMonitor.ResetAll`: clear every enabled monitor's buffer.
pub(crate) fn reset_all_monitors(ckt: &mut Circuit, env: &mut SolveEnv) {
    let monitors = ckt.monitors.clone();
    for mon_ref in monitors {
        let m = env
            .store
            .obj_mut(mon_ref)
            .as_any_mut()
            .downcast_mut::<Monitor>()
            .expect("ckt.monitors holds Monitor objects");
        if m.med.cd.enabled {
            m.reset_it();
        }
    }
}
