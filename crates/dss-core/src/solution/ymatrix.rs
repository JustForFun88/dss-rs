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
    // Recount buses/nodes if bus definitions changed — this changes the node
    // references into the system Y matrix.
    if ckt.bus_name_redefined {
        ckt.reprocess_bus_defs(env.store, env.parser, env.vars, env.errors);
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

    // Tune up the Yprims if necessary: all of them on a frequency change,
    // else only the invalidated ones.
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
    for &r in &ckt.ckt_elements {
        let elem = env.store.ckt_elem_mut(r);
        if recalc_all || elem.cd().yprim_invalid {
            elem.calc_yprim(&sys);
            elem.cd_mut().yprim_invalid = false;
        }
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
