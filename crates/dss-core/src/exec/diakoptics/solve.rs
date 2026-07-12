//! `SendIdx2Actors`, the child A-Diakoptics setup (`Start_Diakoptics`/
//! `IndexBuses`) and the coordinator solve stitch (`Solve_Diakoptics` +
//! `Dss::ad_solve`).
//!
//! Under plan D3 the coordinator is the main [`Dss`] (Pascal `ActiveCircuit[1]`)
//! and the zones are `ad_children: Vec<Dss>` (Pascal `ActiveCircuit[2..]`). The
//! Pascal PM actors are barrier-synchronised, so "sending `SOLVE_AD1`/`SOLVE_AD2`"
//! is a synchronous method-call loop; every `ActiveCircuit[1].…` a child
//! dereferences becomes an explicit argument (the parent `NodeV` buffer written
//! at the child's contiguous offset, and the read-only `Contours`/`Ic` views).
//!
//! **Child boundary reference (r3723 Oddie probe, resume executor 2026-07-12).**
//! `Start_Diakoptics` disables each zone's artificial `source`/`vph_2`/`vph_3`
//! VSources and its feeder-head link PDE, so a reference-free zone (actors > 2)
//! drives its boundary purely through the `Ic` current injections. The probe
//! answered the blocking question — the child Y stays solvable because the loads'
//! `Yeq` shunts (stamped into Y as the fixed-point accelerator) anchor every
//! node to ground: even with all sources disabled the reference-free zone is
//! near-singular but not singular, and faer factors it (no KLU tiny-pivot
//! regularization is involved). The port therefore uses the ordinary faer
//! factorization with **no guard**: a genuinely singular child Y surfaces as the
//! normal solve-error path (`SolutionAbort`), never silently regularized (§5).
//! `Start_Diakoptics` runs only for actors > 2 (Solution.pas:3232); actor 2
//! (zone 1) keeps its real VSource reference.
//!
//! The probe also settled how the child voltage is maintained: official FREEZES
//! each child's own `NodeV` at its state-2 standalone solve for the whole AD run
//! (`SolveSystem` writes only into the coordinator array — proven: actor 3's
//! `NodeV` moves `0.0` across the AD solve). The port re-seeds the child `NodeV`
//! each iteration as a documented faer↔KLU compensation on the near-singular
//! reference-free zone — see [`crate::solution::solution`]'s `ad_solve_into_parent`
//! for the full derivation, the per-fixture floors, and the open item.

use num_complex::Complex64;

use super::super::registry::ClassStore;
use crate::exec::Dss;
use crate::solution::solution::{
    end_of_time_step_cleanup, sample_all_monitors_and_meters, set_generator_disp_ref,
    set_generator_dqdv, solve_ad,
};
use crate::solution::ymatrix::{BuildOption, build_y_matrix};
use crate::solution::{SolveEnv, SolveMode, SolveResult};
use crate::support::sparse_math::SparseComplex;

/// OpenDSS `StripExtension`: the bus name is everything before the first `.`
/// (the node-list suffix). Lowercased, matching `getPDEatBus`.
fn strip_ext(bus: &str) -> String {
    match bus.split_once('.') {
        Some((b, _)) => b.to_lowercase(),
        None => bus.to_lowercase(),
    }
}

impl Dss {
    /// Pascal `SendIdx2Actors()` (Diakoptics.pas:123): assign each child its
    /// `VIndex` (the offset of its bus 1 in the interconnected node list) and
    /// allocate the coordinator `Ic` vector.
    ///
    /// NOTE(upstream-quirk): when a child's `bus1.1` is not found, Pascal reads
    /// the loop variable `j` after the completed `for` (FPC loop-var-after-loop),
    /// i.e. `High(AllNNames)+1` (Diakoptics.pas:151–153). Unreachable by
    /// construction (a child's bus 1 always exists in the parent); we set that
    /// same past-the-end index rather than reproduce the UB read (plan D5).
    ///
    /// `VIndex`'s only Pascal consumer is `Notify_Main` → `V_0`, which is dead
    /// scaffolding (plan D5, like `Node_dV`/`Ic_Local`) — so `VIndex` is inert
    /// here; it is assigned for structural fidelity. The live per-child offset is
    /// `LocalBusIdx[0]` from `IndexBuses`.
    pub(crate) fn send_idx_2_actors(&mut self) {
        // AllNNames: interconnected node names, bus-major, original case
        // (`BusList.Get(i) + '.' + GetNum(j)`).
        let all_names: Vec<String> = {
            let Some(coord) = self.circuit.as_ref() else {
                return;
            };
            let mut names = Vec::with_capacity(coord.num_nodes);
            for (bus_idx, bus) in coord.buses.iter().enumerate() {
                let bus_name = coord.bus_list.name(bus_idx).unwrap_or("");
                for j in 0..bus.num_nodes_this_bus() {
                    names.push(format!("{}.{}", bus_name, bus.get_num(j)));
                }
            }
            names
        };

        // Child actor 2 (child[0]) → VIndex 0; the rest by name lookup.
        for (k, child) in self.ad_children.iter_mut().enumerate() {
            let v_index = if k == 0 {
                0
            } else {
                let bus1 = child
                    .circuit
                    .as_ref()
                    .and_then(|c| c.bus_list.name(0))
                    .map(|b| format!("{b}.1"))
                    .unwrap_or_default();
                all_names
                    .iter()
                    .position(|n| *n == bus1)
                    .unwrap_or(all_names.len()) as i32
            };
            if let Some(ckt) = child.circuit.as_mut() {
                ckt.ad.v_index = v_index;
            }
        }

        // Allocate the coordinator Ic (`length(AllNNames) × 1`); it is recomputed
        // fresh each `Solve_Diakoptics`, so the declared size is all that matters.
        if let Some(ckt) = self.circuit.as_mut() {
            ckt.ad.ic.init(all_names.len() as i32, 1);
            // Keep an explicit zero column so `Ic.NZero` matches Pascal's zero-init
            // before the first solve (a few readers guard on it).
            for i in 0..all_names.len() as i32 {
                ckt.ad.ic.insert(i, 0, Complex64::ZERO);
            }
        }
    }

    /// Pascal `INIT_ADIAKOPTICS` (Solution.pas:3230): after the coordinator
    /// closes its link branches (init state 9), each child sets up its local
    /// Diakoptics state — `Start_Diakoptics` (only actors > 2, i.e. children
    /// beyond zone 1) then `IndexBuses`.
    pub(crate) fn ad_init_actors(&mut self) {
        // Snapshot the coordinator node names (SrcBus) + a clone of Contours for
        // the child index build — disjoint from the per-child borrows below.
        let (src_bus, contours) = {
            let Some(coord) = self.circuit.as_ref() else {
                return;
            };
            let src_bus: Vec<String> = (1..=coord.num_nodes).map(|i| coord.node_name(i)).collect();
            (src_bus, coord.ad.contours.clone())
        };
        for (k, child) in self.ad_children.iter_mut().enumerate() {
            if k >= 1 {
                // Actor index k+2 > 2 → Start_Diakoptics.
                child.start_diakoptics();
            }
            child.index_buses(&src_bus, &contours);
        }
    }

    /// Pascal `TSolver.Start_Diakoptics()` (Solution.pas:3286): disable the
    /// child's feeder-head link PDE (the first PD element at the artificial main
    /// VSource's bus) and its artificial `source`/`vph_2`/`vph_3` VSources, so the
    /// zone's boundary is driven only by the `Ic` current injections.
    fn start_diakoptics(&mut self) {
        // The main VSource = the first element of the VSource class (Pascal
        // `VsourceClass.ElementList.First`) — the artificial `source`.
        let feeder_bus = self.first_vsource_bus1().map(|b| strip_ext(&b));
        if let Some(bus) = feeder_bus
            && let Some(pde) = self.ad_pde_at_bus(&bus)
        {
            // Disables the link branch (myPDEList[0]).
            self.ad_set_element_enabled(&pde, false);
        }
        // Disable all the artificially-added VSources.
        for name in ["vsource.source", "vsource.vph_2", "vsource.vph_3"] {
            self.ad_set_element_enabled(name, false);
        }
    }

    /// The `bus1` (with node suffix) of the first VSource in class order —
    /// Pascal `VsourceClass.ElementList.First.GetBus(1)`.
    fn first_vsource_bus1(&self) -> Option<String> {
        let ci = *self.class_by_name.get("vsource")?;
        for obj in &self.classes[ci].objects {
            if let Some(ce) = obj.as_ckt_element() {
                return Some(ce.cd().get_bus(1).to_string());
            }
        }
        None
    }

    /// Pascal `TDSSCircuit.getPDEatBus(BusName)` (Circuit.pas:1493): the first PD
    /// element (in class-list order) with a terminal on `bus_name` whose two
    /// terminal buses differ — returned as `Class.Name`. `bus_name` is already
    /// stripped + lowercased.
    fn ad_pde_at_bus(&self, bus_name: &str) -> Option<String> {
        let ckt = self.circuit.as_ref()?;
        for &r in &ckt.pd_elements {
            let Some(ce) = self.classes[r.cls].objects[r.idx].as_ckt_element() else {
                continue;
            };
            let cd = ce.cd();
            let b0 = strip_ext(cd.get_bus(1));
            let b1 = strip_ext(cd.get_bus(2));
            if (b0 == bus_name || b1 == bus_name) && b0 != b1 {
                let cls = self.classes[r.cls].props.class_name();
                return Some(format!("{}.{}", cls, cd.obj.name()));
            }
        }
        None
    }

    /// Pascal `TSolver.IndexBuses()` (Solution.pas:2928): build the child→parent
    /// node index map. `LocalBusIdx[i]` is the 1-based interconnected node index
    /// of the child's node `i+1`; `AD_IBus`/`AD_ISrcIdx` locate, for each
    /// coordinator `Contours` boundary row present in this child, the child's
    /// local node (1-based) and the parent contour row (0-based).
    ///
    /// `src_bus[i]` is the interconnected node name of parent node `i+1`
    /// (`node_name`); the same lowercase-suffixed form is built for the child.
    /// The child Y is rebuilt first (Pascal `BuildYMatrix(WHOLEMATRIX, TRUE)`),
    /// applying the `Start_Diakoptics` element changes and re-allocating the
    /// solution arrays; the node count is unchanged, so the child's `NodeV` (its
    /// fixed tear-time load-linearisation point) is preserved.
    fn index_buses(&mut self, src_bus: &[String], contours: &SparseComplex) {
        // Rebuild the child Y so the map reflects the post-Start_Diakoptics state.
        self.rebuild_child_y();
        let Some(ckt) = self.circuit.as_mut() else {
            return;
        };
        let lcl_bus: Vec<String> = (1..=ckt.num_nodes).map(|i| ckt.node_name(i)).collect();

        // LocalBusIdx[i] = 1-based parent index of child node i+1 (past-the-end on
        // a miss, matching Pascal's `j` after the completed search — unreachable
        // for a well-formed tear).
        let past_end = src_bus.len() + 1;
        let local_bus_idx: Vec<usize> = lcl_bus
            .iter()
            .map(|name| {
                src_bus
                    .iter()
                    .position(|s| s == name)
                    .map(|p| p + 1)
                    .unwrap_or(past_end)
            })
            .collect();

        // AD_IBus / AD_ISrcIdx from the coordinator Contours rows present here.
        let mut ad_ibus: Vec<usize> = Vec::new();
        let mut ad_isrc_idx: Vec<i32> = Vec::new();
        for c in &contours.cdata {
            let row = c.row as usize; // 0-based parent node index
            let Some(my_bus) = src_bus.get(row) else {
                continue;
            };
            if let Some(k) = lcl_bus.iter().position(|n| n == my_bus) {
                ad_ibus.push(k + 1); // 1-based child node
                ad_isrc_idx.push(c.row); // parent contour row
            }
        }

        ckt.solution.local_bus_idx = local_bus_idx;
        ckt.solution.ad_ibus = ad_ibus;
        ckt.solution.ad_isrc_idx = ad_isrc_idx;
    }

    /// Rebuild `self`'s whole Y (Pascal `BuildYMatrix(WHOLEMATRIX, TRUE)`, V
    /// realloc): used for a child in `IndexBuses`.
    fn rebuild_child_y(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return;
        };
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        let _ = build_y_matrix(ckt, &mut env, BuildOption::WholeMatrix, true);
    }

    /// One child A-Diakoptics stage (`SolveAD`, Solution.pas:1263), run in the
    /// child's own `(ckt, env)` context; `parent_node_v`/`parent_ic` are the
    /// coordinator's explicit views (plan D3).
    fn child_solve_ad(
        &mut self,
        initialize: bool,
        adiak_pcinj: bool,
        parent_node_v: &mut [Complex64],
        parent_ic: &SparseComplex,
    ) -> SolveResult {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit
            .as_mut()
            .ok_or("A-Diakoptics child has no circuit")?;
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        solve_ad(
            ckt,
            &mut env,
            initialize,
            adiak_pcinj,
            parent_node_v,
            parent_ic,
        )
    }

    /// Pascal `Solve_Diakoptics()` (Diakoptics.pas:49): one coordinator stitch.
    /// `SOLVE_AD1` to every child (each solves its zone into the coordinator
    /// `NodeV` at its offset), then the boundary compensation `Vpartial` (contour
    /// node-pair diffs) → `Y4·Vpartial` → `Ic = Contours·Vpartial`, then
    /// `SOLVE_AD2` (each child adds `−Ic` at its injection buses and re-solves).
    ///
    /// `adiak_pcinj` is the Pascal global `ADiak_PCInj` (DSSGlobals.pas:307) made
    /// explicit under D3: `True` from `DoNormalSolution`, `False` from
    /// `SolveDirect`/`SolveYDirect` — it governs whether the children fold in
    /// their PC (load/generator) injections.
    fn solve_diakoptics(&mut self, adiak_pcinj: bool) -> SolveResult {
        // SOLVE_AD1: each child solves into the coordinator NodeV.
        {
            let Dss {
                circuit,
                ad_children,
                ..
            } = self;
            let coord = circuit.as_mut().ok_or("A-Diakoptics coordinator gone")?;
            for child in ad_children.iter_mut() {
                child.child_solve_ad(
                    true,
                    adiak_pcinj,
                    &mut coord.solution.node_v,
                    &coord.ad.ic,
                )?;
            }
        }

        // Boundary compensation on the coordinator NodeV.
        {
            let coord = self
                .circuit
                .as_mut()
                .ok_or("A-Diakoptics coordinator gone")?;
            let ncols = coord.ad.contours.ncols();
            let mut vpartial = SparseComplex::new();
            vpartial.init(ncols, 1);
            let mut my_row = 0usize;
            for i in 0..ncols {
                // Vpartial[i] = NodeV[Contours row(2i)+1] − NodeV[Contours row(2i+1)+1].
                let (r0, r1) = (
                    coord.ad.contours.cdata[my_row].row as usize,
                    coord.ad.contours.cdata[my_row + 1].row as usize,
                );
                let v = coord.solution.node_v[r0 + 1] - coord.solution.node_v[r1 + 1];
                vpartial.insert(i, 0, v);
                my_row += 2;
            }
            let vpartial = coord.ad.y4.multiply(&vpartial); // Y4·Vpartial
            coord.ad.ic = coord.ad.contours.multiply(&vpartial); // Ic = Contours·Vpartial
        }

        // SOLVE_AD2: each child folds in the −Ic correction and re-solves.
        {
            let Dss {
                circuit,
                ad_children,
                ..
            } = self;
            let coord = circuit.as_mut().ok_or("A-Diakoptics coordinator gone")?;
            for child in ad_children.iter_mut() {
                child.child_solve_ad(
                    false,
                    adiak_pcinj,
                    &mut coord.solution.node_v,
                    &coord.ad.ic,
                )?;
            }
        }

        // Pascal `Solve_Diakoptics` tail: mark the coordinator solved.
        let coord = self
            .circuit
            .as_mut()
            .ok_or("A-Diakoptics coordinator gone")?;
        coord.is_solved = !coord.solution.solution_abort;
        coord.bus_name_redefined = false;
        Ok(())
    }

    /// The A-Diakoptics coordinator solve (`DoSolveCmd` with
    /// `Solution.ADiakoptics`). Dispatches by mode: `Direct` → one
    /// `Solve_Diakoptics` with PC injections off (`SolveDirect`, Solution.pas:1398);
    /// everything else → the snapshot control loop (`SolveSnap` → the AD
    /// `DoNormalSolution`). Daily/Yearly/Duty step the coordinator clock and
    /// re-enter the snapshot solve per step ([`Self::ad_solve_time_series`]).
    ///
    /// Newton is NOT AD-aware (official `DoNormalSolution` only branches to
    /// `Solve_Diakoptics` on the fixed-point path); an AD deck set to Newton
    /// falls through to the normal per-child fixed-point solve, matching upstream.
    pub(crate) fn ad_solve(&mut self) {
        // Pascal `Solve` resets `AD_Init`.
        let mode = {
            let Some(ckt) = self.circuit.as_mut() else {
                self.errors
                    .push("No circuit for A-Diakoptics solve.".to_string());
                return;
            };
            ckt.solution.adiak_init = false;
            ckt.is_solved = false;
            ckt.solution_was_attempted = true;
            ckt.solution.mode
        };
        let result = match mode {
            SolveMode::Direct => self.ad_solve_direct(),
            SolveMode::Snapshot => self.ad_solve_snap(),
            SolveMode::Daily
            | SolveMode::Yearly
            | SolveMode::DutyCycle
            | SolveMode::PeakDay
            | SolveMode::Time => self.ad_solve_time_series(mode),
            other => {
                self.errors.push(format!(
                    "A-Diakoptics solve does not support mode {other:?} (upstream AD is a \
                     snapshot/direct/time-series power-flow accelerator)."
                ));
                Ok(())
            }
        };
        if let Err(e) = result {
            self.errors.push(format!("Error Encountered in Solve: {e}"));
            if let Some(ckt) = self.circuit.as_mut() {
                ckt.solution.solution_abort = true;
            }
        }
    }

    /// Pascal `SolveDirect` AD branch (Solution.pas:1398): `ADiak_PCInj := False`,
    /// one `Solve_Diakoptics`, mark solved. Admittance-only boundary drive.
    fn ad_solve_direct(&mut self) -> SolveResult {
        {
            let coord = self.circuit.as_mut().ok_or("no coordinator")?;
            coord.solution.solution_count += 1;
            coord.solution.loads_need_updating = true;
            coord.solution.adiak_pcinj = false;
        }
        self.solve_diakoptics(false)?;
        let coord = self.circuit.as_mut().ok_or("no coordinator")?;
        coord.is_solved = true;
        coord.solution.converged_flag = true;
        coord.solution.iteration = 1;
        coord.solution.last_solution_was_direct = true;
        Ok(())
    }

    /// Pascal `SolveSnap` AD path (coordinator): the control loop wrapping the AD
    /// `DoNormalSolution` (Solution.pas:1006 — `ADiak_PCInj := True;
    /// Solve_Diakoptics()` per fixed-point iteration, convergence over the
    /// interconnected `NodeV`). Controls are checked on the coordinator; the
    /// child `DO_CTRL_ACTIONS` propagation (`full` control decks) is WP-AD.4 —
    /// the WP-AD.3/D7 fixtures solve controls-off.
    fn ad_solve_snap(&mut self) -> SolveResult {
        {
            let coord = self.circuit.as_mut().ok_or("no coordinator")?;
            set_generator_disp_ref(coord);
            coord.solution.snap_shot_init();
        }
        let (min_iter, max_iter, max_ctrl) = {
            let c = self.circuit.as_ref().ok_or("no coordinator")?;
            (
                c.solution.min_iterations,
                c.solution.max_iterations,
                c.solution.max_control_iterations,
            )
        };
        let mut total_iterations = 0i32;
        loop {
            {
                let coord = self.circuit.as_mut().ok_or("no coordinator")?;
                coord.solution.control_iteration += 1;
            }
            // DoPFLOWsolution init: the coordinator was already initialized by the
            // init state-machine solve (state 2), so `solution_initialized` is
            // TRUE and this block is skipped — matching Pascal. If a deck ever
            // reaches here uninitialized, seed the generator dQ/dV (a no-op
            // without model-3 generators).
            {
                let need_init = {
                    let c = self.circuit.as_ref().ok_or("no coordinator")?;
                    !c.solution.solution_initialized
                };
                if need_init {
                    self.ad_coord_init()?;
                }
            }
            // AD DoNormalSolution fixed-point loop.
            {
                let coord = self.circuit.as_mut().ok_or("no coordinator")?;
                coord.solution.iteration = 0;
            }
            loop {
                {
                    let coord = self.circuit.as_mut().ok_or("no coordinator")?;
                    coord.solution.iteration += 1;
                    coord.solution.adiak_pcinj = true;
                }
                self.solve_diakoptics(true)?;
                let done = {
                    let n = self.circuit.as_ref().ok_or("no coordinator")?.num_nodes;
                    let coord = self.circuit.as_mut().ok_or("no coordinator")?;
                    let converged = coord.solution.converged(n);
                    let it = coord.solution.iteration;
                    (converged && it >= min_iter) || it >= max_iter
                };
                if done {
                    break;
                }
            }
            // CheckControls (AD branch): controls-off → control_actions_done.
            self.ad_check_controls()?;
            let (ctrl_it, done) = {
                let coord = self.circuit.as_mut().ok_or("no coordinator")?;
                if coord.solution.iteration > coord.solution.most_iterations_done {
                    coord.solution.most_iterations_done = coord.solution.iteration;
                }
                total_iterations += coord.solution.iteration;
                (
                    coord.solution.control_iteration,
                    coord.solution.control_actions_done,
                )
            };
            if done || ctrl_it >= max_ctrl {
                break;
            }
        }
        let coord = self.circuit.as_mut().ok_or("no coordinator")?;
        coord.solution.iteration = total_iterations;
        coord.is_solved = coord.solution.converged_flag;
        coord.solution.last_solution_was_direct = false;
        Ok(())
    }

    /// The coordinator's `DoPFLOWsolution` one-time init (generator dQ/dV seed).
    fn ad_coord_init(&mut self) -> SolveResult {
        {
            let Dss {
                classes,
                circuit,
                aux_parser,
                vars,
                errors,
                ..
            } = self;
            let ckt = circuit.as_mut().ok_or("no coordinator")?;
            let mut store = ClassStore { classes };
            let mut env = SolveEnv {
                store: &mut store,
                parser: aux_parser,
                vars,
                errors,
            };
            set_generator_dqdv(ckt, &mut env)?;
        }
        let ckt = self.circuit.as_mut().ok_or("no coordinator")?;
        ckt.solution.solution_initialized = true;
        Ok(())
    }

    /// Pascal `CheckControls` AD branch (Solution.pas:1132) on the coordinator:
    /// when converged, sample + run the coordinator's control actions; controls-
    /// off sets `control_actions_done` immediately. The child `DO_CTRL_ACTIONS`
    /// fan-out (`SendCmd2Actors`) is WP-AD.4 (`full` control decks); WP-AD.3's
    /// gates run controls-off.
    fn ad_check_controls(&mut self) -> SolveResult {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().ok_or("no coordinator")?;
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        if ckt.solution.control_iteration < ckt.solution.max_control_iterations {
            if ckt.solution.converged_flag {
                crate::solution::controls::sample_do_control_actions(ckt, &mut env)?;
                crate::solution::faults::check_fault_status(ckt, &mut env)?;
            } else {
                ckt.solution.control_actions_done = true;
            }
        }
        if ckt.solution.system_y_changed {
            build_y_matrix(ckt, &mut env, BuildOption::WholeMatrix, false)?;
        }
        Ok(())
    }

    /// The A-Diakoptics time-series modes (Daily/Yearly/Duty/PeakDay/Time): the
    /// coordinator steps the clock + default load-shape multiplier exactly like
    /// `time_series.rs`, re-entering [`Self::ad_solve_snap`] per step, then
    /// samples monitors/meters. The children read their loads from their own
    /// fixed tear-time linearisation (the shape multiplier they see is the one
    /// baked in at tear time); the D7 fixtures use flat shapes, so each step is a
    /// snapshot. (Non-flat time-varying-shape AD across zones is out of the
    /// WP-AD.3 scope — the coordinator-step model is the faithful translation of
    /// the Pascal, where the actors likewise never receive a per-step clock.)
    fn ad_solve_time_series(&mut self, mode: SolveMode) -> SolveResult {
        let (number_of_times, is_peak) = {
            let c = self.circuit.as_ref().ok_or("no coordinator")?;
            (c.solution.number_of_times, mode == SolveMode::PeakDay)
        };
        if is_peak {
            let coord = self.circuit.as_mut().ok_or("no coordinator")?;
            coord.solution.t = 0.0;
            coord.solution.int_hour = 0;
            coord.solution.dbl_hour = 0.0;
        }
        {
            let coord = self.circuit.as_mut().ok_or("no coordinator")?;
            coord.solution.interval_hrs = coord.solution.h / 3600.0;
        }
        for _ in 1..=number_of_times {
            {
                let c = self.circuit.as_ref().ok_or("no coordinator")?;
                if c.solution.solution_abort {
                    continue;
                }
            }
            // Advance clock + default shape multiplier (Time increments at the end).
            if mode != SolveMode::Time {
                self.ad_increment_clock_and_shape(mode)?;
            } else {
                self.ad_set_shape_mult(SolveMode::Daily)?;
            }
            self.ad_solve_snap()?;
            self.ad_sample_and_cleanup();
            if mode == SolveMode::Time {
                let coord = self.circuit.as_mut().ok_or("no coordinator")?;
                coord.solution.increment_time();
            }
        }
        Ok(())
    }

    /// Increment the coordinator clock and set the default load-shape multiplier
    /// from the mode's shape (`SolveDaily`/`SolveYearly`/`SolveDuty` head).
    fn ad_increment_clock_and_shape(&mut self, mode: SolveMode) -> SolveResult {
        {
            let coord = self.circuit.as_mut().ok_or("no coordinator")?;
            coord.solution.increment_time();
        }
        self.ad_set_shape_mult(mode)
    }

    /// Set `default_hour_mult` from the mode's default shape at the current hour.
    fn ad_set_shape_mult(&mut self, mode: SolveMode) -> SolveResult {
        let coord = self.circuit.as_mut().ok_or("no coordinator")?;
        let dbl_hour = coord.solution.dbl_hour;
        let use_yearly = matches!(mode, SolveMode::Yearly);
        let shape = if use_yearly {
            coord.default_yearly_shape_obj.as_mut()
        } else {
            coord.default_daily_shape_obj.as_mut()
        };
        match shape {
            Some(s) => coord.default_hour_mult = s.get_mult_at_hour(dbl_hour),
            None => {
                return Err(if use_yearly {
                    "Default yearly load shape not found.".to_string()
                } else {
                    "Default daily load shape not found.".to_string()
                });
            }
        }
        Ok(())
    }

    /// Per-step monitor/meter sampling + end-of-step cleanup on the coordinator
    /// (`SolutionAlgs.pas` per-step tail).
    fn ad_sample_and_cleanup(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return;
        };
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        let sample_meters = ckt.solution.sample_the_meters;
        sample_all_monitors_and_meters(ckt, &mut env, sample_meters);
        end_of_time_step_cleanup(ckt, &mut env);
    }
}
