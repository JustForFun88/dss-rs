//! Port of `Common/Ymatrix.pas`: `BuildYMatrix` (full-rebuild path),
//! `InitializeNodeVbase`, and `CheckYMatrixforZeroes`-lite diagnostics.

use num_complex::Complex64;

use dss_sparse::SparseSet;

use crate::circuit::Circuit;
use crate::elements::ckt::ElemFlags;
use crate::solution::solution::{ActiveY, SolveEnv, SolveResult, sys_ctx};

/// Pascal `SERIESONLY` / `WHOLEMATRIX` / `PDE_ONLY` (`Ymatrix.pas` l.22-24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildOption {
    SeriesOnly,
    WholeMatrix,
    /// PDE-only: stamp the **full** (`ALL_YPRIM`) primitive of every enabled PD
    /// element **or** SOURCE (VSource) into the series handle, excluding all PC
    /// elements. This is the network admittance NCIM factors and reads back as
    /// its `NCIM_Y` triplet dump (`Ymatrix.pas` l.442-497). Like `WholeMatrix`
    /// it invalidates the series matrix and clears `SystemYChanged`.
    PdeOnly,
}

/// Reuse the existing sparse set when the matrix order is unchanged: `zero()`
/// keeps the dedup / symbolic / row-equilibration skeletons, so a value-only Y
/// rebuild (a tap change, the per-step load `Yeq` restamp) refactors without
/// re-hashing the dedup map or re-analyzing the symbolic factorization
/// (DE_PASCALIZE P15 item 1). A node-count change (a bus redefinition)
/// allocates a fresh set; a renumbering that keeps the count is caught by the
/// stamp-pattern check inside [`dss_sparse::SparseSet`], which then rebuilds.
fn reuse_or_new_sparse(slot: &mut Option<SparseSet>, n: usize) {
    match slot {
        Some(s) if s.size() == n => s.zero(),
        _ => *slot = Some(SparseSet::new(n)),
    }
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

/// Pascal `TSolutionObj.UpdateVBus` (`Common/Solution.pas` l.2377): snapshot the
/// present node-voltage vector into each bus's saved `VBus` slots, so a
/// subsequent Y rebuild that renumbers nodes can restore them. Guarded by
/// `pBus.VBus <> NIL` — here `!bus.vbus.is_empty()`, which holds for every bus
/// once `ReprocessBusDefs` has allocated bus state (`Circuit.pas` l.2205-2206).
/// `ref_no[j]` is the 0-based global node index (ground = 0 → `node_v[0]` = 0).
fn update_vbus(ckt: &mut Circuit) {
    let node_v = &ckt.solution.node_v;
    for bus in &mut ckt.buses {
        if bus.vbus.is_empty() {
            continue;
        }
        for j in 0..bus.ref_no.len() {
            bus.vbus[j] = node_v[bus.ref_no[j]];
        }
    }
}

/// Pascal `TSolutionObj.RestoreNodeVfromVbus` (`Common/Solution.pas` l.2392):
/// the inverse of [`update_vbus`] — write each bus's saved `VBus` back into the
/// node-voltage vector after the rebuild.
fn restore_node_v_from_vbus(ckt: &mut Circuit) {
    let node_v = &mut ckt.solution.node_v;
    for bus in &ckt.buses {
        if bus.vbus.is_empty() {
            continue;
        }
        for j in 0..bus.ref_no.len() {
            node_v[bus.ref_no[j]] = bus.vbus[j];
        }
    }
}

/// Pascal `BuildYMatrix`: full rebuild of the designated Y matrix; with
/// `allocate_vi` also (re)allocates the solution vectors and the node-V base.
pub fn build_y_matrix(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    option: BuildOption,
    allocate_vi: bool,
) -> SolveResult {
    // Pascal `BuildYMatrix` brackets the rebuild with `UpdateVBus()` /
    // `RestoreNodeVfromVbus()` when `Solution.PreserveNodeVoltages` is set
    // (Ymatrix.pas l.298/l.449), so node voltages survive a Y rebuild that
    // renumbers nodes. The flag is set entering Harmonic/HarmonicT (WP7.6) and
    // Dynamic (WP7.7). `update_vbus` snapshots the present node voltages *before*
    // the (possible) `ReprocessBusDefs`; `restore_node_v_from_vbus` at the tail
    // writes them back. Whenever the node count is unchanged (the harmonics
    // goldens + the dynamics step-1 driver) the round-trip is a net no-op —
    // restore re-applies exactly the pre-build snapshot — so no gate moves; it
    // only matters once a mid-mode rebuild renumbers or reallocates nodes.
    if ckt.solution.preserve_node_voltages {
        update_vbus(ckt);
    }

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
            reuse_or_new_sparse(&mut ckt.solution.y_system, y_matrix_size);
            ckt.solution.active_y = ActiveY::System;
        }
        BuildOption::SeriesOnly | BuildOption::PdeOnly => {
            reuse_or_new_sparse(&mut ckt.solution.y_series, y_matrix_size);
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
    let mut yprim_errors = crate::diag::ErrorLog::new();
    for &r in &ckt.ckt_elements {
        let elem = env.store.ckt_elem_mut(r);
        // Pascal `ReCalcAllYPrims`/`ReCalcInvalidYPrims` (Ymatrix.pas @ 0.15.0b4):
        // skip `CalcYPrim` for an element whose YPrim was forced from the DSS
        // language (`Set YPrim=…`, `Flg.ForceYPrim`) — the user-supplied matrix
        // (already in `cd.yprim`) is added to the system Y below as-is.
        if elem.cd().flags.contains(ElemFlags::FORCE_YPRIM) {
            continue;
        }
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
                BuildOption::PdeOnly => "Building PDE only Y Matrix",
            },
        );
    }

    // Add in Yprims for all enabled devices. `PDE_ONLY` restricts the element
    // set to PD elements + sources (Ymatrix.pas l.463-467) and, like
    // `WholeMatrix`, uses the FULL (`ALL_YPRIM`) primitive — not the series one.
    {
        // Pascal iterates all `CktElements` and filters; PD/source membership is
        // disjoint, so iterating `pd_elements` then `sources` visits exactly the
        // qualifying set once each (order is irrelevant — the assembled matrix
        // sums duplicates). Build the ref list up front to drop the `ckt` borrow
        // before the mutable `y_series`/`y_system` borrow below.
        let refs: Vec<crate::elements::traits::ElemId> = match option {
            BuildOption::WholeMatrix | BuildOption::SeriesOnly => ckt.ckt_elements.clone(),
            BuildOption::PdeOnly => ckt
                .pd_elements
                .iter()
                .chain(ckt.sources.iter())
                .copied()
                .collect(),
        };
        let sparse = match option {
            BuildOption::WholeMatrix => ckt.solution.y_system.as_mut().unwrap(),
            BuildOption::SeriesOnly | BuildOption::PdeOnly => {
                ckt.solution.y_series.as_mut().unwrap()
            }
        };
        for &r in &refs {
            let elem = env.store.ckt_elem(r);
            let cd = elem.cd();
            if !cd.enabled {
                continue;
            }
            let mat = match option {
                BuildOption::WholeMatrix | BuildOption::PdeOnly => cd.yprim.as_ref(),
                BuildOption::SeriesOnly => cd.yprim_series.as_ref(),
            };
            if let Some(m) = mat {
                if cd.node_ref.len() < cd.yorder {
                    return Err(format!(
                        "Node index out of range adding to System Y Matrix (element \"{}\" has no node references)",
                        cd.obj.name()
                    ));
                }
                // Read the `TcMatrix` column-major storage directly, traversed
                // in the same row-major `(i,j)` order (so triplet insertion — and
                // thus dedup summation — order is unchanged): no per-element
                // `to_row_major` transpose copy (DE_PASCALIZE P15 item 3).
                sparse.add_primitive_matrix_col_major(&cd.node_ref[..cd.yorder], m.values());
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
        BuildOption::PdeOnly => {
            ckt.solution.series_y_invalid = true; // series may not match
            ckt.solution.system_y_changed = false;
        }
    }

    // Pascal `BuildYMatrix` tail (Ymatrix.pas l.449): restore the snapshot taken
    // above, after the Yprim add + solution-array realloc renumbered/reallocated
    // the node vector.
    if ckt.solution.preserve_node_voltages {
        restore_node_v_from_vbus(ckt);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::Bus;
    use crate::elements::traits::{CktElement, ElemId, ElemStore};
    use crate::obj::base::DssObject;
    use dss_parser::{Parser, ParserVars};

    fn c(re: f64, im: f64) -> Complex64 {
        Complex64::new(re, im)
    }

    /// A no-op [`ElemStore`]: `build_y_matrix` only dereferences it through the
    /// element loops, which are empty here, so every method is unreachable.
    struct EmptyStore;
    impl ElemStore for EmptyStore {
        fn ckt_elem(&self, _r: ElemId) -> &dyn CktElement {
            unimplemented!()
        }
        fn ckt_elem_mut(&mut self, _r: ElemId) -> &mut dyn CktElement {
            unimplemented!()
        }
        fn obj(&self, _r: ElemId) -> &dyn DssObject {
            unimplemented!()
        }
        fn kind(&self, _r: ElemId) -> crate::circuit::ElemKind {
            unimplemented!()
        }
        fn obj_mut(&mut self, _r: ElemId) -> &mut dyn DssObject {
            unimplemented!()
        }
        fn find_ckt_element(&self, _full_name: &str) -> Option<ElemId> {
            None
        }
        fn find_general(&self, _class_name: &str, _obj_name: &str) -> Option<ElemId> {
            None
        }
        fn pair_mut(&mut self, _a: ElemId, _b: ElemId) -> (&mut dyn DssObject, &mut dyn DssObject) {
            unimplemented!()
        }
        fn triple_mut(
            &mut self,
            _a: ElemId,
            _b: ElemId,
            _c: ElemId,
        ) -> (&mut dyn DssObject, &mut dyn DssObject, &mut dyn DssObject) {
            unimplemented!()
        }
    }

    /// A circuit with a single 3-node bus (`ref_no = [1,2,3]`), the node vector
    /// seeded with `node_v` (index 0 = ground) and every `vbus` slot at
    /// `vbus_seed` (a sentinel for the build-wiring tests).
    fn one_bus_ckt(node_v: Vec<Complex64>, vbus_seed: Complex64) -> Circuit {
        let mut ckt = Circuit::new("t", 60.0);
        // A fresh circuit defaults `bus_name_redefined = true`; leaving it set
        // would make `build_y_matrix` call `reprocess_bus_defs`, which rebuilds
        // the bus list from the (empty) element set and discards our hand-built
        // bus. These tests exercise the no-renumber path, so clear it.
        ckt.bus_name_redefined = false;
        ckt.num_nodes = 3;
        let mut bus = Bus::new("b1");
        bus.nodes = vec![1, 2, 3];
        bus.ref_no = vec![1, 2, 3];
        bus.vbus = vec![vbus_seed; 3];
        bus.bus_current = vec![Complex64::ZERO; 3];
        ckt.buses = vec![bus];
        ckt.solution.node_v = node_v;
        ckt
    }

    /// `update_vbus`/`restore_node_v_from_vbus` are exact inverses over the
    /// `ref_no` node map (Pascal `UpdateVBus`/`RestoreNodeVfromVbus`).
    #[test]
    fn vbus_round_trip_maps_ref_no() {
        let mut ckt = one_bus_ckt(
            vec![Complex64::ZERO, c(10.0, 1.0), c(20.0, 2.0), c(30.0, 3.0)],
            Complex64::ZERO,
        );
        update_vbus(&mut ckt);
        assert_eq!(
            ckt.buses[0].vbus,
            vec![c(10.0, 1.0), c(20.0, 2.0), c(30.0, 3.0)]
        );
        // Clobber the live node vector, then restore from the saved VBus.
        for i in 1..=3 {
            ckt.solution.node_v[i] = Complex64::ZERO;
        }
        restore_node_v_from_vbus(&mut ckt);
        assert_eq!(
            ckt.solution.node_v,
            vec![Complex64::ZERO, c(10.0, 1.0), c(20.0, 2.0), c(30.0, 3.0)]
        );
    }

    /// The feature's actual point (audit settlement — the prior tests only
    /// covered the no-renumber net-no-op): a rebuild that RENUMBERS nodes must
    /// restore each bus's saved voltages at the bus's NEW node indices — the
    /// snapshot keys by (bus, node position), not by global node number
    /// (Pascal `VBus[j]` ↔ `RefNo[j]`, `Solution.pas:2377/2392`).
    #[test]
    fn vbus_restore_lands_on_renumbered_refs() {
        let mut ckt = one_bus_ckt(
            vec![Complex64::ZERO, c(10.0, 1.0), c(20.0, 2.0), c(30.0, 3.0)],
            Complex64::ZERO,
        );
        update_vbus(&mut ckt); // vbus (by node position) = [10+1i, 20+2i, 30+3i]
        // Renumber: the bus's three nodes move to global slots 3, 1, 2 (as a
        // `ReprocessBusDefs` re-order would) and the node vector is cleared.
        ckt.buses[0].ref_no = vec![3, 1, 2];
        ckt.solution.node_v = vec![Complex64::ZERO; 4];
        restore_node_v_from_vbus(&mut ckt);
        assert_eq!(
            ckt.solution.node_v,
            vec![Complex64::ZERO, c(20.0, 2.0), c(30.0, 3.0), c(10.0, 1.0)],
            "voltages must follow the bus's node positions to their NEW refs"
        );
    }

    /// Flag ON: `build_y_matrix` runs update *then* restore. Node voltages come
    /// back unchanged (net no-op) AND the `vbus` sentinel is overwritten with the
    /// live values — proving `update_vbus` executed inside the build.
    #[test]
    fn build_preserves_and_updates_vbus_when_flag_set() {
        let known = vec![Complex64::ZERO, c(1.0, 1.0), c(2.0, 2.0), c(3.0, 3.0)];
        let sentinel = c(-1.0, -1.0);
        let mut ckt = one_bus_ckt(known.clone(), sentinel);
        ckt.solution.preserve_node_voltages = true;

        let mut store = EmptyStore;
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = crate::diag::ErrorLog::new();
        {
            let mut env = SolveEnv {
                store: &mut store,
                parser: &mut parser,
                vars: &vars,
                errors: &mut errors,
            };
            build_y_matrix(&mut ckt, &mut env, BuildOption::WholeMatrix, false).unwrap();
        }

        assert_eq!(
            ckt.solution.node_v, known,
            "round-trip leaves node_v intact"
        );
        assert_eq!(
            ckt.buses[0].vbus,
            vec![c(1.0, 1.0), c(2.0, 2.0), c(3.0, 3.0)],
            "update_vbus overwrote the sentinel"
        );
    }

    /// Flag OFF: neither update nor restore runs. The `vbus` sentinel survives
    /// (nothing wrote it) — this is what makes the ON test non-vacuous.
    #[test]
    fn build_leaves_vbus_untouched_when_flag_clear() {
        let known = vec![Complex64::ZERO, c(1.0, 1.0), c(2.0, 2.0), c(3.0, 3.0)];
        let sentinel = c(-1.0, -1.0);
        let mut ckt = one_bus_ckt(known.clone(), sentinel);
        ckt.solution.preserve_node_voltages = false;

        let mut store = EmptyStore;
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        let mut errors = crate::diag::ErrorLog::new();
        {
            let mut env = SolveEnv {
                store: &mut store,
                parser: &mut parser,
                vars: &vars,
                errors: &mut errors,
            };
            build_y_matrix(&mut ckt, &mut env, BuildOption::WholeMatrix, false).unwrap();
        }

        assert_eq!(ckt.solution.node_v, known);
        assert_eq!(
            ckt.buses[0].vbus,
            vec![sentinel; 3],
            "update_vbus must NOT run with the flag clear"
        );
    }
}
