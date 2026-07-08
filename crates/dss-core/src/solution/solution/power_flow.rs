//! The snapshot power-flow path: the injection machinery, `DoNormalSolution`,
//! `SolveYDirect`/`SolveZeroLoadSnapShot`, the generator dispatch reference and
//! dQ/dV seed, `DoPFLOWsolution`, `SolveCircuit`, the live `CheckControls`,
//! `SolveSnap` and `SolveDirect`.

use num_complex::Complex64;

use crate::circuit::{CAPADD, Circuit, GENADD};
use crate::elements::pc::generator::Generator;
use crate::elements::traits::{ElemRef, InjCtx};
use crate::solution::ymatrix::{BuildOption, build_y_matrix, initialize_node_vbase};

use super::{ActiveY, NEWTONSOLVE, SolveEnv, SolveMode, SolveResult, sys_ctx};

/// Pascal `TSolutionObj.AddInAuxCurrents` → `TAutoAdd.AddCurrents`
/// (`Solution.pas` l.2139 / `AutoAdd.pas` l.597): during an AutoAdd candidate
/// solve, inject the trial generator/capacitor current at the bus under test.
/// The `AddInAuxCurrents` gate is `SolutionMode = AUTOADDFLAG`, so this no-ops
/// in any other mode even though only AutoAdd ever sets `use_aux_currents`.
fn add_in_aux_currents(ckt: &mut Circuit, solve_type: i32) {
    if ckt.solution.mode != SolveMode::AutoAdd {
        return;
    }
    let aa = &ckt.auto_add_obj;
    let (add_type, phases, gen_va, ycap, bus_index) =
        (aa.add_type, aa.phases, aa.gen_va, aa.ycap, aa.bus_index);
    if bus_index == 0 {
        return;
    }
    // Snapshot the node refs first (drops the `ckt.buses` borrow) so the
    // `ckt.solution.currents` write below doesn't alias it. Pascal `GetRef(i)`
    // is 1-based; the Rust accessor is 0-based, and `BusIndex` is Pascal 1-based.
    let nrefs: Vec<usize> = (1..=phases as usize)
        .map(|i| ckt.buses[bus_index - 1].get_ref(i - 1))
        .collect();
    for nref in nrefs {
        if nref == 0 {
            continue; // add in only non-ground currents
        }
        let bus_v = ckt.solution.node_v[nref];
        if bus_v.re == 0.0 && bus_v.im == 0.0 {
            continue;
        }
        // Current INTO the system network.
        match add_type {
            GENADD => {
                let inj = (gen_va / bus_v).conj();
                if solve_type == NEWTONSOLVE {
                    ckt.solution.currents[nref] -= inj; // Terminal Current
                } else {
                    ckt.solution.currents[nref] += inj; // Injection Current
                }
            }
            CAPADD => {
                // Constant Y model.
                if solve_type == NEWTONSOLVE {
                    ckt.solution.currents[nref] += Complex64::new(0.0, ycap) * bus_v;
                } else {
                    ckt.solution.currents[nref] += Complex64::new(0.0, -ycap) * bus_v;
                }
            }
            _ => {}
        }
    }
}

/// `DSS.LogThisEvent(name)` with the solution's clock/iteration fields (the
/// callers gate on `ckt.LogEvents` themselves, like the Pascal call sites).
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

/// Pascal `GetSourceInjCurrents`: all enabled sources inject into `Currents`.
/// (The GFM PCE pass is empty in Phase 3.)
fn get_source_inj_currents(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let sol = &mut ckt.solution;
    let mut ctx = InjCtx {
        node_v: &sol.node_v,
        currents: &mut sol.currents,
        system_y_changed: &mut sol.system_y_changed,
    };
    for &r in &ckt.sources {
        let elem = env.store.ckt_elem_mut(r);
        if elem.cd().enabled {
            elem.inj_currents(&sys, &mut ctx);
        }
    }
}

/// Pascal `GetPCInjCurr`: all enabled PC elements inject into `Currents`.
fn get_pc_inj_curr(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let sol = &mut ckt.solution;
    let mut ctx = InjCtx {
        node_v: &sol.node_v,
        currents: &mut sol.currents,
        system_y_changed: &mut sol.system_y_changed,
    };
    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem_mut(r);
        if elem.cd().enabled {
            elem.inj_currents(&sys, &mut ctx);
        }
    }
}

/// Pascal `DoNormalSolution`: the fixed-point loop.
fn do_normal_solution(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.iteration = 0;
    loop {
        ckt.solution.iteration += 1;

        if ckt.log_events {
            log_event(
                ckt,
                &format!("Solution Iteration {}", ckt.solution.iteration),
            );
        }

        ckt.solution.zero_inj_curr();
        get_source_inj_currents(ckt, env);
        get_pc_inj_curr(ckt, env);

        // The above calls could change the primitive Y matrix, so check.
        if ckt.solution.system_y_changed {
            build_y_matrix(ckt, env, BuildOption::WholeMatrix, false)?;
        }
        // Pascal `if UseAuxCurrents then AddInAuxCurrents(NORMALSOLVE)`
        // (Solution.pas l.899): AutoAdd's per-candidate trial-device injection.
        if ckt.solution.use_aux_currents {
            add_in_aux_currents(ckt, super::NORMALSOLVE);
        }

        if ckt.log_events {
            log_event(ckt, "Solve Sparse Set DoNormalSolution ...");
        }
        ckt.solution.solve_system()?;
        ckt.solution.loads_need_updating = false;

        let num_nodes = ckt.num_nodes;
        let converged = ckt.solution.converged(num_nodes);
        if (converged && ckt.solution.iteration >= ckt.solution.min_iterations)
            || ckt.solution.iteration >= ckt.solution.max_iterations
        {
            return Ok(());
        }
    }
}

/// Pascal `TSolutionObj.SumAllCurrents`: every circuit element sums its
/// terminal currents into the system `Currents` array (`TDSSCktElement.
/// SumCurrents`: `ComputeIterminal`, then `Currents[NodeRef[i]] += Iterminal[i]`
/// with `NodeRef=0` accumulating harmlessly into the ground slot). Primarily
/// for the Newton iteration.
fn sum_all_currents(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let sol = &mut ckt.solution;
    for &r in &ckt.ckt_elements {
        let elem = env.store.ckt_elem_mut(r);
        if !elem.cd().enabled || elem.cd().node_ref.is_empty() {
            continue;
        }
        elem.compute_iterminal(&sys, &sol.node_v);
        let cd = elem.cd();
        for i in 0..cd.yorder {
            sol.currents[cd.node_ref[i]] += cd.iterminal[i];
        }
    }
}

/// Pascal `DoNewtonSolution`: the Newton iteration
/// `Vn+1 = Vn - [Y]⁻¹·Termcurr`, driving the sum of terminal currents into
/// every node to zero. `Termcurr` is `SumAllCurrents` (PD: `Yprim·V`; PC:
/// the compensation currents). Same convergence/budget clause as
/// `DoNormalSolution`: `(Converged and Iteration >= MinIterations) or
/// Iteration >= MaxIterations`. The `dV` work array (`ReAllocMem(dV, NumNodes+1)`)
/// is the per-step scratch inside `solve_system_newton_step`.
fn do_newton_solution(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    // ControlIteration == 1: update the load multipliers for this solution.
    if ckt.solution.control_iteration == 1 {
        get_pc_inj_curr(ckt, env);
    }

    ckt.solution.iteration = 0;
    loop {
        ckt.solution.iteration += 1;
        // SumAllCurrents uses ITerminal, so force a recalc via a fresh count.
        ckt.solution.solution_count += 1;

        // Get sum of currents at all nodes for all devices.
        ckt.solution.zero_inj_curr();
        sum_all_currents(ckt, env);

        // The current calc could change Yprim for some devices, so check.
        if ckt.solution.system_y_changed {
            build_y_matrix(ckt, env, BuildOption::WholeMatrix, false)?;
        }
        // UseAuxCurrents/AddInAuxCurrents(NEWTONSOLVE): AutoAdd only.

        // Solve for the change in voltages and update the guess.
        ckt.solution.solve_system_newton_step()?;
        ckt.solution.loads_need_updating = false;

        let num_nodes = ckt.num_nodes;
        let converged = ckt.solution.converged(num_nodes);
        if (converged && ckt.solution.iteration >= ckt.solution.min_iterations)
            || ckt.solution.iteration >= ckt.solution.max_iterations
        {
            return Ok(());
        }
    }
}

/// Pascal `SolveYDirect`: solve with only source injections.
fn solve_y_direct(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.zero_inj_curr();
    get_source_inj_currents(ckt, env);
    if ckt.solution.is_dynamic_model {
        get_pc_inj_curr(ckt, env);
    }
    ckt.solution.solve_system()
}

/// Pascal `SolveZeroLoadSnapShot`: series-only solve for initialization.
pub fn solve_zero_load_snapshot(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.solution.system_y_changed || ckt.solution.series_y_invalid {
        build_y_matrix(ckt, env, BuildOption::SeriesOnly, true)?; // Side effect: allocates V
    }
    ckt.solution.solution_count += 1;
    ckt.solution.zero_inj_curr();
    get_source_inj_currents(ckt, env);

    // Make the series Y matrix the active matrix.
    if ckt.solution.y_series.is_none() {
        return Err("Series Y matrix not built yet in SolveZeroLoadSnapshot.".to_string());
    }
    ckt.solution.active_y = ActiveY::Series;
    if ckt.log_events {
        log_event(ckt, "Solve Sparse Set ZeroLoadSnapshot ...");
    }
    let result = ckt.solution.solve_system();

    // Reset the main system Y as the solution matrix.
    if ckt.solution.y_system.is_some() {
        ckt.solution.active_y = ActiveY::System;
    }
    result
}

/// Pascal `TSolutionObj.SetGeneratorDispRef`: the global generator dispatch
/// reference per solve mode (generator.pas LOADMODE/PRICEMODE compare their
/// `DispValue` against it).
pub(super) fn set_generator_disp_ref(ckt: &mut Circuit) {
    let lm = ckt.load_multiplier;
    let gf = ckt.default_growth_factor;
    let hm = ckt.default_hour_mult.re;
    ckt.generator_dispatch_reference = match ckt.solution.mode {
        SolveMode::Snapshot
        | SolveMode::Dynamic
        | SolveMode::Harmonic
        | SolveMode::Monte1
        | SolveMode::Direct => lm * gf,
        SolveMode::Yearly => gf * hm, // note: no load multiplier for yearly
        SolveMode::Daily
        | SolveMode::DutyCycle
        | SolveMode::Time
        | SolveMode::Monte2
        | SolveMode::Monte3
        | SolveMode::PeakDay
        | SolveMode::LD1
        | SolveMode::LD2
        | SolveMode::HarmonicT => lm * gf * hm,
        SolveMode::MonteFault | SolveMode::FaultStudy => 1.0,
        SolveMode::AutoAdd => gf,
    };
}

/// Pascal `TSolutionObj.SetGeneratordQdV`: for model-3 (PV) generators, seed
/// the `dQ/dV` slope from the system Y diagonal, then re-establish the
/// zero-load snapshot if any was found.
fn set_generator_dqdv(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let gens: Vec<ElemRef> = ckt.generators.clone();
    let gen_disp_save = ckt.generator_dispatch_reference;
    ckt.generator_dispatch_reference = 1000.0; // turn all generators on
    let mut did_one = false;

    for r in gens {
        // Read the model/enabled/node first (immutable view).
        let node0 = {
            let g = env
                .store
                .obj(r)
                .as_any()
                .downcast_ref::<Generator>()
                .expect("generators list holds Generators");
            if !g.cd.enabled || g.gen_model != 3 {
                continue;
            }
            g.first_node_ref()
        };
        if node0 == 0 {
            continue; // first node grounded — nothing to slope against
        }
        let yii = ckt.solution.system_matrix_element(node0)?.norm();
        let g = env
            .store
            .obj_mut(r)
            .as_any_mut()
            .downcast_mut::<Generator>()
            .expect("generators list holds Generators");
        g.init_dqdv_calc();
        g.calc_dqdv(yii);
        g.reset_start_point();
        did_one = true;
    }

    ckt.generator_dispatch_reference = gen_disp_save;
    if did_one {
        solve_zero_load_snapshot(ckt, env)?; // reset the initial solution
    }
    Ok(())
}

/// Pascal `DoPFLOWsolution`.
fn do_pflow_solution(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.solution_count += 1;

    if ckt.solution.voltage_base_changed {
        initialize_node_vbase(ckt); // for convergence test
    }

    if !ckt.solution.solution_initialized {
        if ckt.log_events {
            log_event(ckt, "Initializing Solution");
        }
        // "8-14-06 This should give a better answer than zero load snapshot"
        solve_y_direct(ckt, env)?;
        set_generator_dqdv(ckt, env)?; // set dQdV for Model-3 generators
        // The above resets the active sparse set to hY.
        ckt.solution.solution_initialized = true;
    }

    match ckt.solution.algorithm {
        NEWTONSOLVE => do_newton_solution(ckt, env),
        _ => do_normal_solution(ckt, env),
    }?;

    ckt.is_solved = ckt.solution.converged_flag;
    ckt.solution.last_solution_was_direct = false;
    Ok(())
}

/// Pascal `SolveCircuit`.
pub(crate) fn solve_circuit(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.solution.load_model == super::ADMITTANCE {
        solve_direct(ckt, env)
    } else {
        if ckt.solution.system_y_changed {
            build_y_matrix(ckt, env, BuildOption::WholeMatrix, true)?; // Side effect: allocates V
        }
        do_pflow_solution(ckt, env)
    }
}

/// Pascal `CheckControls` (`Solution.pas` l.1132): when converged, log the
/// control iteration (gated by `LogEvents`), sample the controls and run the
/// queued actions; then rebuild Y if anything invalidated it (voltages kept).
fn check_controls(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.solution.control_iteration < ckt.solution.max_control_iterations {
        if ckt.solution.converged_flag {
            if ckt.log_events {
                let sol = &mut ckt.solution;
                sol.event_log.log_this_event(
                    &format!("Control Iteration {}", sol.control_iteration),
                    sol.int_hour,
                    sol.t,
                    sol.iteration,
                    sol.control_iteration,
                );
            }
            crate::solution::controls::sample_do_control_actions(ckt, env)?;
            crate::solution::faults::check_fault_status(ckt, env)?;
        } else {
            ckt.solution.control_actions_done = true; // Stop if failure to converge
        }
    }
    if ckt.solution.system_y_changed {
        build_y_matrix(ckt, env, BuildOption::WholeMatrix, false)?; // V stays same
    }
    Ok(())
}

/// Pascal `SolveSnap`.
pub(crate) fn solve_snap(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    set_generator_disp_ref(ckt); // Pascal SnapShotInit's first action
    ckt.solution.snap_shot_init();
    let mut total_iterations = 0;
    loop {
        ckt.solution.control_iteration += 1;

        solve_circuit(ckt, env)?; // Do circuit solution w/o checking controls
        check_controls(ckt, env)?;

        // For reporting max iterations per control iteration
        if ckt.solution.iteration > ckt.solution.most_iterations_done {
            ckt.solution.most_iterations_done = ckt.solution.iteration;
        }
        total_iterations += ckt.solution.iteration;

        if ckt.solution.control_actions_done
            || ckt.solution.control_iteration >= ckt.solution.max_control_iterations
        {
            break;
        }
    }

    if !ckt.solution.control_actions_done
        && ckt.solution.control_iteration >= ckt.solution.max_control_iterations
    {
        env.errors.push(
            "Warning Max Control Iterations Exceeded.\nTip: Show Eventlog to debug control settings."
                .to_string(),
        );
        ckt.solution.solution_abort = true; // stop this message in dynamic power flow modes
    }

    if ckt.log_events {
        let sol = &mut ckt.solution;
        sol.event_log.log_this_event(
            "Solution Done",
            sol.int_hour,
            sol.t,
            sol.iteration,
            sol.control_iteration,
        );
    }

    ckt.solution.iteration = total_iterations; // "so that it reports a more interesting number"
    Ok(())
}

/// Pascal `SolveDirect`.
pub(super) fn solve_direct(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.loads_need_updating = true;
    ckt.solution.solution_count += 1;

    if ckt.solution.system_y_changed {
        build_y_matrix(ckt, env, BuildOption::WholeMatrix, true)?;
    }
    ckt.solution.zero_inj_curr();
    get_source_inj_currents(ckt, env);
    if ckt.solution.is_dynamic_model || ckt.solution.is_harmonic_model {
        get_pc_inj_curr(ckt, env);
    }
    ckt.solution.solve_system()?;
    ckt.is_solved = true;
    ckt.solution.converged_flag = true;
    ckt.solution.iteration = 1;
    ckt.solution.last_solution_was_direct = true;
    Ok(())
}
