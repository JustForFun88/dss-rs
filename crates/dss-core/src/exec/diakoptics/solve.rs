//! `SendIdx2Actors` + the A-Diakoptics solve dispatch (`Dss::ad_solve`).
//!
//! WP-AD.3 Stage 2a lands `SendIdx2Actors` (index assignment + `Ic` allocation)
//! and the solve *entry point*. The per-iteration stitch itself
//! (`Solve_Diakoptics` / `SolveAD` / `Start_Diakoptics` / `IndexBuses`) is
//! Stage 2b: the child boundary-reference semantics (`Start_Diakoptics` disables
//! each zone's artificial VSources and drives the boundary through current
//! injection) must be settled with an r3723 Oddie probe before implementation —
//! a guessed reference model risks a singular child Y. The child-side
//! `solve_ad`/`UpdateISrc` and the per-child index fields are already ported
//! (see `solution/solution/power_flow.rs` and `Solution` fields).

use crate::exec::Dss;
use num_complex::Complex64;

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
    /// `LocalBusIdx[0]` from `IndexBuses` (Stage 2b).
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

    /// The A-Diakoptics solve entry (`DoSolveCmd` with `Solution.ADiakoptics`).
    ///
    /// Stage 2a: the per-iteration diakoptics stitch is not yet wired (see the
    /// module doc — it awaits the r3723 boundary-reference probe). Rather than
    /// run a normal solve (which would silently ignore the AD partition) or a
    /// guessed stitch (which risks wrong physics), report honestly and reset
    /// `AD_Init` as `Solve` does.
    pub(crate) fn ad_solve(&mut self) {
        if let Some(ckt) = self.circuit.as_mut() {
            ckt.solution.adiak_init = false; // Pascal `Solve` resets AD_Init
        }
        self.errors.push(
            "A-Diakoptics solve is not wired yet (WP-AD.3 Stage 2b: the child \
             boundary-reference stitch awaits an r3723 probe). Init built the \
             coordinator, children and Contours/ZLL/ZCC/Y4."
                .to_string(),
        );
    }
}
