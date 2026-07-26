//! Pascal `TSolutionAlgs.SolveFaultStudy` and the `TSolutionObj` short-circuit
//! helpers it drives (`AllocateAllSCParms`, `UpdateVBus`, `ComputeAllYsc`,
//! `ComputeYsc`, `ComputeIsc`, `DisableAllFaults`) — the FaultStudy solve mode.
//!
//! A fault study computes, for every bus, the prospective short-circuit
//! impedance `Zsc` (and its inverse `Ysc`) and current `Isc`. The bus `Zsc`
//! columns are extracted by injecting a unit current at each node and reading
//! the resulting node voltages off the already-factored system Y — i.e. each
//! column of `Zsc` is a column of `Y⁻¹` restricted to the bus's own nodes.

use num_complex::Complex64;

use crate::circuit::Circuit;
use crate::elements::pd::fault::Fault;
use crate::support::cmatrix::CMatrix;

use super::power_flow::solve_direct;
use super::{LoadSolutionModel, SolveEnv, SolveResult};
use crate::elements::traits::TypedStore;

/// Pascal `TSolutionAlgs.SolveFaultStudy`: open-circuit (Voc) direct solve,
/// then per-bus `Ysc`/`Zsc`/`Isc`.
pub(super) fn solve_fault_study(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.load_model = LoadSolutionModel::Admittance;
    disable_all_faults(ckt, env);

    // Open-circuit voltages (and corrected bus lists).
    solve_direct(ckt, env)?;

    allocate_all_sc_parms(ckt); // (re)allocate the per-bus SC matrices
    update_v_bus(ckt); // capture the present Voc in each bus before Y is reused

    compute_all_ysc(ckt)?;
    compute_isc(ckt);
    Ok(())
}

/// Pascal `TSolutionAlgs.DisableAllFaults`: remove every Fault object from the
/// network so the prospective fault current is computed against the unfaulted
/// system. Disabling an enabled element invalidates the system Y (Pascal
/// `Set_Enabled` → `Set_YprimInvalid`).
fn disable_all_faults(ckt: &mut Circuit, env: &mut SolveEnv) {
    for r in ckt.faults.clone() {
        if let Some(fault) = env.store.typed_mut::<Fault>(r)
            && fault.cd.enabled
        {
            fault.cd.enabled = false;
            ckt.solution.system_y_changed = true;
        }
    }
}

/// Pascal `TSolutionAlgs.AllocateAllSCParms`: `AllocateBusQuantities` on every
/// bus (reallocates `Zsc`/`Ysc`/`VBus`/`BusCurrent`).
fn allocate_all_sc_parms(ckt: &mut Circuit) {
    for b in ckt.buses.iter_mut() {
        b.allocate_bus_quantities();
    }
}

/// Pascal `TSolutionObj.UpdateVBus`: copy the present `NodeV` solution into each
/// bus's `VBus` (`NodeV[RefNo[j]]`; a ground ref is index 0 → zero).
fn update_v_bus(ckt: &mut Circuit) {
    let Circuit {
        solution, buses, ..
    } = ckt;
    for b in buses.iter_mut() {
        for (j, &r) in b.ref_no.iter().enumerate() {
            b.vbus[j] = solution.node_v[r];
        }
    }
}

/// Pascal `TSolutionAlgs.ComputeAllYsc`: zero the injection vector, then
/// `ComputeYsc` for every bus.
fn compute_all_ysc(ckt: &mut Circuit) -> SolveResult {
    ckt.solution.currents.fill(Complex64::ZERO);
    for ib in 0..ckt.buses.len() {
        compute_ysc(ckt, ib)?;
    }
    Ok(())
}

/// Pascal `TSolutionAlgs.ComputeYsc`: column `i` of the bus `Zsc` is obtained by
/// injecting 1 A at node `i` and reading the bus's node voltages off the
/// factored system Y; `Ysc = Zsc⁻¹`. Assumes `Currents` is zeroed on entry and
/// restores it before returning (the injection is set then cleared per node).
fn compute_ysc(ckt: &mut Circuit, bus_idx: usize) -> SolveResult {
    // Split the circuit borrow: the injection/solve touches `solution` only, so
    // the bus's `ref_no` is read in place instead of cloned per bus.
    let Circuit {
        buses, solution, ..
    } = ckt;
    let ref_no = &buses[bus_idx].ref_no;
    let n = ref_no.len();
    let mut zsc = CMatrix::new(n);

    for (i, &ref1) in ref_no.iter().enumerate() {
        if ref1 > 0 {
            solution.currents[ref1] = Complex64::new(1.0, 0.0);
            // Re-solve the already-factored system Y with the unit injection.
            solution.solve_system()?;
            for (j, &rj) in ref_no.iter().enumerate() {
                zsc.set(j, i, solution.node_v[rj]);
            }
            solution.currents[ref1] = Complex64::ZERO;
        }
    }

    let mut ysc = zsc.clone();
    // Pascal `ComputeYsc` ignores the Invert result (a singular/degenerate bus,
    // e.g. a delta-isolated zero sequence, leaves `Ysc` partially transformed —
    // the reporting path reads `Zsc`, and `Isc` mirrors the upstream value).
    let _ = ysc.invert();

    buses[bus_idx].zsc = Some(zsc);
    buses[bus_idx].ysc = Some(ysc);
    Ok(())
}

/// Pascal `TSolutionAlgs.ComputeIsc`: `BusCurrent = Ysc · VBus` per bus.
fn compute_isc(ckt: &mut Circuit) {
    for b in ckt.buses.iter_mut() {
        if let Some(ysc) = b.ysc.take() {
            // `bus_current` and `vbus` are disjoint fields — no copy of `vbus`.
            ysc.mv_mult(&mut b.bus_current, &b.vbus);
            b.ysc = Some(ysc);
        }
    }
}
