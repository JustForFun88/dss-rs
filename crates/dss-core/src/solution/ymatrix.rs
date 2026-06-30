//! Port of `Common/Ymatrix.pas`: `BuildYMatrix` (full-rebuild path),
//! `InitializeNodeVbase`, and `CheckYMatrixforZeroes`-lite diagnostics.

use num_complex::Complex64;

use dss_sparse::SparseSet;

use crate::circuit::Circuit;
use crate::solution::solution::{ActiveY, SolveEnv, SolveResult, sys_ctx};

/// Pascal `SERIESONLY` / `WHOLEMATRIX`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildOption {
    SeriesOnly,
    WholeMatrix,
}

/// `DSS.LogThisEvent` with the solution's clock/iteration fields (callers
/// gate on `ckt.LogEvents`, like the Pascal call sites in `Ymatrix.pas`).
fn log_event(ckt: &mut Circuit, name: &str) {
    let sol = &mut ckt.solution;
    sol.event_log.log_this_event(
        name,
        sol.int_hour,
        sol.t,
        sol.iteration,
        sol.control_iteration,
    );
}

/// Pascal `InitializeNodeVbase`: `NodeVbase[i] = kVBase(bus of node i) · 1000`.
pub fn initialize_node_vbase(ckt: &mut Circuit) {
    for i in 1..=ckt.num_nodes {
        let bus_ref = ckt.map_node_to_bus[i].bus_ref;
        ckt.solution.node_vbase[i] = ckt.buses[bus_ref].kv_base * 1000.0;
    }
    ckt.solution.voltage_base_changed = false;
}

/// Pascal `BuildYMatrix`: full rebuild of the designated Y matrix; with
/// `allocate_vi` also (re)allocates the solution vectors and the node-V base.
pub fn build_y_matrix(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    option: BuildOption,
    allocate_vi: bool,
) -> SolveResult {
    // NOT_PORTED(WP7.7): Pascal `BuildYMatrix` brackets the rebuild with
    // `UpdateVBus()` / `RestoreNodeVfromVbus()` when `Solution.PreserveNodeVoltages`
    // is set (Ymatrix.pas l.298/l.449), so node voltages survive a mid-mode Y
    // rebuild. The flag is set entering Harmonic/HarmonicT (WP7.6) and Dynamic
    // (WP7.7 step 1) but not yet consumed here. Inert so far: those modes do not
    // force a mid-step structural Y rebuild without a per-element dynamics YPrim
    // invalidation (step 2). Honour it when the step-2 machine YPrims can change Y
    // mid-dynamics; until then the harmonics goldens + the step-1 driver tests pass
    // because no rebuild discards the preserved voltages.
    // Recount buses/nodes if bus definitions changed — this changes the node
    // references into the system Y matrix.
    if ckt.bus_name_redefined {
        ckt.reprocess_bus_defs(env.store, env.parser, env.vars, env.errors);
        // Pascal `ReprocessBusDefs` tail (Circuit.pas l.2246): rebuild the meter
        // zones now that the bus references are current.
        crate::solution::meters::do_reset_meter_zones(ckt, env.store);
    }

    let y_matrix_size = ckt.num_nodes;
    match option {
        BuildOption::WholeMatrix => {
            ckt.solution.y_system = Some(SparseSet::new(y_matrix_size));
            ckt.solution.active_y = ActiveY::System;
        }
        BuildOption::SeriesOnly => {
            ckt.solution.y_series = Some(SparseSet::new(y_matrix_size));
            ckt.solution.active_y = ActiveY::Series;
        }
    }

    // Tune up the Yprims. Pascal logs "Recalc All Yprims" on a frequency
    // change and "Recalc Invalid Yprims" otherwise (`Ymatrix.pas`
    // ReCalcAllYPrims/ReCalcInvalidYPrims) — the event-log gates pin these
    // strings, so keep the message frequency-driven.
    //
    // The recompute itself, however, always touches *every* element: the base
    // `TDSSCktElement.CalcYPrim` only clears `YPrimInvalid` under
    // `{$IFDEF DSS_CAPI_INCREMENTAL_Y}` + the non-default
    // `AlwaysResetYPrimInvalid` solver option, so in the oracle's default
    // build the flag is never reset and `ReCalcInvalidYPrims` recomputes all
    // Yprims on every `BuildYMatrix`. Reproduce that exactly. It matters for
    // time-series modes: a load's admittance `Yeq = (P - jQ)/Vbase²` is
    // shape-scaled and folded into Y as the fixed-point convergence
    // accelerator; if it is frozen at the load level of the step where Y was
    // last structurally rebuilt (e.g. a tap change), the per-step iteration
    // path diverges from the oracle and the daily EnergyMeter / monitor
    // trajectory drifts (verified empirically against the pinned oracle:
    // restamping with the current Yeq each build is what makes the IEEE13
    // daily registers match at 1e-4).
    let sys = sys_ctx(ckt);
    let recalc_all = ckt.solution.frequency_changed;
    if ckt.log_events {
        log_event(
            ckt,
            if recalc_all {
                "Recalc All Yprims"
            } else {
                "Recalc Invalid Yprims"
            },
        );
    }
    let mut yprim_errors: Vec<String> = Vec::new();
    for &r in &ckt.ckt_elements {
        let elem = env.store.ckt_elem_mut(r);
        elem.calc_yprim(&sys);
        elem.cd_mut().yprim_invalid = false;
        // A `CalcYPrim` that aborts (e.g. a `LineGeometry` Zmatrix error) queues a
        // deferred message instead of building YPrim; collect it below.
        yprim_errors.extend(elem.cd_mut().obj.take_errors());
    }
    if !yprim_errors.is_empty() {
        // Pascal: the geometry getter raised `ELineGeometryProblem` and set
        // `SolutionAbort`, and `CalcYPrim` exited. The trait has no direct abort
        // channel, so surface the queued message(s) and abort the solution here
        // (the parse path already drained every other deferred error, so anything
        // collected above came from `CalcYPrim`).
        env.errors.extend(yprim_errors);
        ckt.solution.solution_abort = true;
    }
    ckt.solution.frequency_changed = false;

    if ckt.log_events {
        log_event(
            ckt,
            match option {
                BuildOption::WholeMatrix => "Building Whole Y Matrix",
                BuildOption::SeriesOnly => "Building Series Y Matrix",
            },
        );
    }

    // Add in Yprims for all enabled devices.
    {
        let sparse = match option {
            BuildOption::WholeMatrix => ckt.solution.y_system.as_mut().unwrap(),
            BuildOption::SeriesOnly => ckt.solution.y_series.as_mut().unwrap(),
        };
        for &r in &ckt.ckt_elements {
            let elem = env.store.ckt_elem(r);
            let cd = elem.cd();
            if !cd.enabled {
                continue;
            }
            let mat = match option {
                BuildOption::WholeMatrix => cd.yprim.as_ref(),
                BuildOption::SeriesOnly => cd.yprim_series.as_ref(),
            };
            if let Some(m) = mat {
                if cd.node_ref.len() < cd.yorder {
                    return Err(format!(
                        "Node index out of range adding to System Y Matrix (element \"{}\" has no node references)",
                        cd.obj.name()
                    ));
                }
                sparse.add_primitive_matrix(&cd.node_ref[..cd.yorder], &m.to_row_major());
            }
        }
    }

    // Allocate voltage and current vectors if requested.
    if allocate_vi {
        if ckt.log_events {
            log_event(ckt, "Reallocating Solution Arrays");
        }
        let n = ckt.num_nodes + 1;
        let sol = &mut ckt.solution;
        sol.node_v.resize(n, Complex64::ZERO);
        sol.node_v[0] = Complex64::ZERO;
        sol.currents.resize(n, Complex64::ZERO);
        sol.aux_currents.resize(n, Complex64::ZERO);
        // VMagSaved/ErrorSaved/NodeVBase are AllocMem'd fresh (zero-filled).
        sol.vmag_saved = vec![0.0; n];
        sol.error_saved = vec![0.0; n];
        sol.node_vbase = vec![0.0; n];
        initialize_node_vbase(ckt);
    }

    match option {
        BuildOption::WholeMatrix => {
            ckt.solution.series_y_invalid = true; // series may not match
            ckt.solution.system_y_changed = false;
        }
        BuildOption::SeriesOnly => {
            ckt.solution.series_y_invalid = false; // SystemYChange unchanged
        }
    }
    Ok(())
}
