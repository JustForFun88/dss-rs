//! The snapshot power-flow path: the injection machinery, `DoNormalSolution`,
//! `SolveYDirect`/`SolveZeroLoadSnapShot`, the generator dispatch reference and
//! dQ/dV seed, `DoPFLOWsolution`, `SolveCircuit`, the live `CheckControls`,
//! `SolveSnap` and `SolveDirect`.

use num_complex::Complex64;

use crate::circuit::{CAPADD, Circuit, GENADD};
use crate::elements::pc::generator::Generator;
use crate::elements::traits::{ElemRef, InjCtx};
use crate::solution::ymatrix::{BuildOption, build_y_matrix, initialize_node_vbase};
use crate::support::sparse_math::SparseComplex;

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

/// Pascal `GetSourceInjCurrents`: all enabled sources inject into `Currents`,
/// then the grid-forming PC elements (`GetPCInjCurr(TRUE)` — a GFM inverter is
/// a voltage source behind its `CalcGFMYprim` impedance, so it injects with the
/// sources, not with the ordinary PC elements).
pub(super) fn get_source_inj_currents(ckt: &mut Circuit, env: &mut SolveEnv) {
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
    // Adds GFM PCE as well.
    get_pc_inj_curr_filtered(ckt, env, true);
}

/// Pascal `GetPCInjCurr(GFMOnly = FALSE)`: the ordinary (non-grid-forming) PC
/// elements inject into `Currents`.
fn get_pc_inj_curr(ckt: &mut Circuit, env: &mut SolveEnv) {
    get_pc_inj_curr_filtered(ckt, env, false);
}

/// Pascal `TSolutionObj.GetPCInjCurr(GFMOnly)`: inject from the enabled PC
/// elements, selecting grid-forming vs ordinary by `onGFM` (Pascal
/// `valid := not (GFMOnly xor onGFM) and Enabled`).
fn get_pc_inj_curr_filtered(ckt: &mut Circuit, env: &mut SolveEnv, gfm_only: bool) {
    let sys = sys_ctx(ckt);
    let sol = &mut ckt.solution;
    let mut ctx = InjCtx {
        node_v: &sol.node_v,
        currents: &mut sol.currents,
        system_y_changed: &mut sol.system_y_changed,
    };
    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem_mut(r);
        let on_gfm = elem.is_gfm();
        if !(gfm_only ^ on_gfm) && elem.cd().enabled {
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
pub(crate) fn do_newton_solution(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
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
pub(crate) fn set_generator_disp_ref(ckt: &mut Circuit) {
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
pub(crate) fn set_generator_dqdv(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
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
pub(crate) fn do_pflow_solution(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
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
        super::NCIMSOLVE => super::do_ncim_solution(ckt, env),
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
    // Pascal `CheckControls` (`Solution.pas` l.1182): under NCIM a Y change means
    // the PDE-only network + NCIM structures must be rebuilt on the next solve —
    // flag `NCIM_Ready = false` and skip the normal `WholeMatrix` rebuild (NCIM
    // rebuilds its own `PDE_ONLY` matrix in `NCIM_Init`).
    if ckt.solution.system_y_changed && ckt.solution.algorithm == super::NCIMSOLVE {
        ckt.solution.ncim_ready = false;
        return Ok(());
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

/// Pascal `TSolutionObj.SolveAD(ActorID, Initialize)` (Solution.pas:1263): the
/// **child** side of one A-Diakoptics stage. Under plan D3 the parent context is
/// passed explicitly (no `ActiveCircuit[1]` global): the child solves its own
/// `hY` with its own injection currents **into the coordinator's NodeV**
/// (`parent_node_v`) at its `LocalBusIdx[0]` offset; `parent_ic` is the
/// coordinator's `Ic`.
///
/// - `Initialize = true` (SOLVE_AD1): zero + source injections (+PC injections
///   when `adiak_pcinj`, or in dynamics/harmonics), Y check.
/// - `Initialize = false` (SOLVE_AD2): `UpdateISrc` — add the coordinator's
///   boundary-current correction, then re-solve.
// Wired by `Solve_Diakoptics` in WP-AD.3 Stage 2b (the coordinator drive loop).
#[allow(dead_code)]
pub(crate) fn solve_ad(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    initialize: bool,
    adiak_pcinj: bool,
    parent_node_v: &mut [Complex64],
    parent_ic: &SparseComplex,
) -> SolveResult {
    if initialize {
        ckt.solution.zero_inj_curr();
        get_source_inj_currents(ckt, env);
        if adiak_pcinj {
            ckt.solution.loads_need_updating = true; // force loads to update once
            get_pc_inj_curr(ckt, env);
        } else if ckt.solution.is_dynamic_model || ckt.solution.is_harmonic_model {
            ckt.solution.loads_need_updating = true;
            get_pc_inj_curr(ckt, env);
        }
        if ckt.solution.system_y_changed {
            build_y_matrix(ckt, env, BuildOption::WholeMatrix, false)?;
        }
    } else {
        update_isrc(ckt, parent_ic);
    }

    // `SolveSystem(ActiveCircuit[1].Solution.NodeV, ActorID)` — solve the child's
    // system into the parent's NodeV at the child's contiguous offset.
    ad_solve_into_parent(ckt, parent_node_v)?;
    ckt.solution.loads_need_updating = false;
    ckt.solution.last_solution_was_direct = true;
    ckt.is_solved = true;
    Ok(())
}

/// Pascal `UpdateISrc` (Solution.pas:3116): for each of this child's AD injection
/// buses, add the negated coordinator `Ic` boundary current into `Currents`.
///
/// NOTE(upstream-quirk): the Pascal `Found` flag is initialized ONCE before the
/// outer loop (Solution.pas:3125), not per bus — so an `AD_ISrcIdx` that is
/// absent from `Ic` (its contour product was dropped by the D5 `re≠0 AND im≠0`
/// filter) reuses the previous iteration's `idx`, reading a stale/out-of-range
/// `Ic` entry. That is the UB class (plan D5): we do a fresh per-bus lookup and
/// skip a miss (the boundary buses of a well-formed tear are always present).
#[allow(dead_code)] // wired by `solve_ad` (WP-AD.3 Stage 2b).
fn update_isrc(ckt: &mut Circuit, parent_ic: &SparseComplex) {
    let sol = &mut ckt.solution;
    for (i, &ibus) in sol.ad_ibus.iter().enumerate() {
        let target = sol.ad_isrc_idx[i];
        if let Some(cd) = parent_ic.cdata.iter().find(|c| c.row == target)
            && let Some(slot) = sol.currents.get_mut(ibus)
        {
            *slot += -cd.value; // cmulreal(Ic, -1) + cadd
        }
    }
}

/// Pascal `SolveSystem(ActiveCircuit[1].Solution.NodeV, ActorID)` for a child
/// (Solution.pas:2658): `SolveSparseSet(hY, @V[LocalBusIdx[0]], @Currents[1])`
/// solves the child's own factored `hY` and writes the `NumNodes`-long solution
/// into the coordinator NodeV. Pascal uses the **contiguous** address form
/// `@V[LocalBusIdx[0]]` (the zone's interconnected nodes are a contiguous run);
/// this port **scatters** each child node `j+1` into its mapped slot
/// `LocalBusIdx[j]` (the explicit form `UploadV2Master` uses) — equal to the
/// contiguous write when the run is contiguous, but robust to the interconnected
/// node ordering.
///
/// **DEVIATION (documented) — the child's own `NodeV` is re-seeded from the
/// solved column** (r3723 Oddie probe, resume executor 2026-07-12; the brief's
/// anticipated faer-vs-KLU reference-free-zone case). Official `SolveSystem`
/// writes ONLY into `ActiveCircuit[1].Solution.NodeV`; a child's own
/// `Solution.NodeV` stays frozen at its state-2 standalone solve for the whole AD
/// run. The probe proved this directly: on macro, actor 3's own `NodeV` moves
/// `0.000e+00` between init and the post-AD read, and that frozen state-2 solve
/// is already within `7.9e-5` of the interconnected result (the tearing seeds
/// each zone-head artificial `VSource` with the true boundary voltage via
/// `PConn_Voltages`). Official therefore FREEZES the child, and its floor is
/// `1.318e-4` (worst near the real source).
///
/// **The re-seed is provably correct** (WP-AD.3 audit finding #2, settled by
/// decomposition — not a tolerance sweep). A-Diakoptics is an EXACT decomposition,
/// so the fixed-point stitch must land on the interconnected-coordinator solution.
/// That interconnected solution is available independently: a `Set algorithm=Newton`
/// AD deck runs a full-system Newton on the closed coordinator (Solution.pas:1018,
/// no ADiakoptics branch) and never touches the children or this re-seed. With the
/// re-seed in place, the fixed-point stitch matches that pure-coordinator Newton
/// solve to **f64 ulp** (midi `7e-13`, macro `1.3e-12`;
/// `adiakoptics.rs::{midi,macro}_newton_ad_matches_fixedpoint_ad`) — i.e. the
/// re-seed recovers the exact interconnected answer. The AD-vs-**normal** "floor"
/// (midi `3.25e-5`, macro `1.32e-4`) is therefore NOT a stitch approximation: it is
/// the interconnected-coordinator-deck-vs-original-deck difference, shared by the
/// fixed-point AND Newton AD paths and matched to the r3723 oracle
/// (`tests/TOLERANCE_NOTES.md` §AD); it stays tolerance-**stable** because it is a
/// deck-structure difference, not an iteration residual.
///
/// A byte-faithful freeze does NOT hold in this port: with the child frozen, the
/// deep interior of the **reference-free** zone diverges to `3.46e-3` @ M180 from
/// that same interconnected ground truth (26× the floor). The characterised
/// mechanism: `Start_Diakoptics` disables the reference-free zone's artificial
/// sources, so its `hY` is anchored only by the loads' weak `Yeq` shunts and is
/// ill-conditioned; the frozen child `NodeV` (its state-2 standalone solve, ~`7.9e-5`
/// off) makes `GetPCInjCurr` evaluate the constant-power loads at a slightly wrong
/// voltage, and that small current error is amplified down the long radial. Pascal
/// (KLU) tolerates the same freeze (converges to the floor); this port (faer) does
/// not — the leading, but not yet bit-level-proven, hypothesis is a faer-vs-KLU
/// difference in factoring the near-singular reference-free child `hY`. Re-seeding
/// the child `NodeV` with the just-solved column each iteration removes the frozen
/// linearisation error at its source (the loads see the boundary-corrected voltage),
/// so the reference-free solve tracks the true voltage and recovers the exact answer
/// above. This is an explicit, documented compensation — NOT a silent Y
/// regularization (§ forbidden).
///
/// Open item (auditors / WP-AD.4): bit-level confirm the freeze-divergence cause by
/// running the exact frozen reference-free child system through both faer and KLU
/// (and measuring its condition number); if it is faer-vs-KLU as hypothesised, make
/// that solve match KLU so a faithful freeze suffices and the re-seed can be dropped.
///
/// The parent write itself: Pascal uses the **contiguous** address form
/// `@V[LocalBusIdx[0]]` (the zone's interconnected nodes are a contiguous run);
/// this port **scatters** each child node `j+1` into its mapped slot
/// `LocalBusIdx[j]` — equal to the contiguous write when the run is contiguous,
/// but robust to the interconnected node ordering.
#[allow(dead_code)] // wired by `solve_ad` (WP-AD.3 Stage 2b).
fn ad_solve_into_parent(ckt: &mut Circuit, parent_node_v: &mut [Complex64]) -> SolveResult {
    let n = ckt.num_nodes;
    let mut x = vec![Complex64::ZERO; n];
    ckt.solution.solve_system_into(&mut x)?;
    let idx = &ckt.solution.local_bus_idx;
    for (j, &v) in x.iter().enumerate() {
        if let Some(&p) = idx.get(j)
            && let Some(slot) = parent_node_v.get_mut(p)
        {
            *slot = v;
        }
    }
    // DEVIATION (see doc): re-seed the child NodeV with the solved column so the
    // near-singular reference-free zone tracks the true voltage (faer↔KLU
    // conditioning compensation); official freezes the child at state-2.
    ckt.solution.node_v[1..=n].copy_from_slice(&x);
    Ok(())
}

/// Pascal `SolveDirect`.
pub(crate) fn solve_direct(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
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
