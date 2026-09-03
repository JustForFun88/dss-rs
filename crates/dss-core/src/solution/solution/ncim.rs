//! The Newton Current-Injection Method (NCIM) power-flow solver
//! (`Set Algorithm=NCIM`).
//!
//! **Spec of record: EPRI r4133 `Version8/Source/Common/Solution.pas`**, where
//! NCIM lives inline (`DoNCIMSolution` l.1095, `GetNCIMPowers` l.1256,
//! `DoPVBusNCIM`/`DoPQBusNCIM`/`DoZBusNCIM` l.1364/1459/1514, `InitNCIM` l.1771,
//! `CalcInjCurr` l.1847, `GetNumGenerators` l.1884, `UpdateGenQ` l.1993,
//! `BuildJacobian` l.2326). The `NCIM_*` names and `NCIMSolutionHelper.pas`
//! line numbers in the per-function docs below are the *retired* capi015 r4103
//! refactor this file was originally ported from (that file exists in no
//! vendored tree); every routine was re-verified against the r4133 source, and
//! the one behavioral difference — the PV↔PQ switching cadence in
//! [`ncim_update_gen_q`] — is now r4133's (ORPHANED_GAPS §1.6,
//! `docs/upgrade/DIVERGENCES.md`).
//!
//! NCIM is a full Newton power flow in **rectangular current-injection form**.
//! The state is the real/imaginary parts of every node voltage; the mismatch is
//! the injected current `I = Y·V + g(V)` where `Y` is the PDE-only network
//! admittance (lines/transformers/sources — [`BuildOption::PdeOnly`]) and `g(V)`
//! is the nonlinear load/generator current. Each iteration assembles a
//! **real-valued** Jacobian ([`dss_sparse::RealSparseSet`], Stage 1) whose per-
//! node 2×2 diagonal block is `[B, G; G, −B]` (the network) plus the load/gen
//! injection derivative, solves `J·ΔZ = ΔF`, and updates `V ← V − ΔV`.
//! Generators declared model-3 participate as PV buses (voltage regulation with
//! Q-limits and automatic PV→PQ switching); model-4 as PQ.
//!
//! Node indexing: a node `i` is 1-based (ground = 0). In the Jacobian / mismatch
//! vectors it occupies 0-based rows `2*(i-1)` (imag) and `2*(i-1)+1` (real). The
//! first three nodes (rows 0..5) are the swing bus (the source's three phases);
//! their Jacobian rows are the identity and their mismatch is forced to zero (the
//! `< 6` guards). The extra rows beyond `2*NumNodes` carry the PV-bus voltage-
//! regulation equations.
//!
//! The KLUSolveX raw `SetMatrixElement(J, i, j, v)` is 1-based (it subtracts 1
//! internally, exactly as the complex `AddPrimitiveMatrix` maps node `k` to
//! matrix index `k-1`); the Pascal `GCoord` arithmetic is therefore mapped to the
//! 0-based [`RealSparseSet::set_element`] by subtracting 1. Stamps **accumulate**
//! (Stage 1 module note), which the diagonal `Y_ii + g'_ii` relies on.

use num_complex::Complex64;

use dss_sparse::RealSparseSet;

use crate::circuit::Circuit;
use crate::elements::pc::generator::Generator;
use crate::elements::pc::load::{Load, LoadModel};
use crate::elements::pc::vsource::VSource;
use crate::solution::ymatrix::{BuildOption, build_y_matrix};

use super::power_flow::get_source_inj_currents;
use super::state::{NCIM_PQ_NODE, NCIM_PV_NODE};
use super::{Solution, SolveEnv, SolveResult};
use crate::elements::traits::TypedStore;

const ZERO: Complex64 = Complex64::ZERO;

fn sqrt3() -> f64 {
    3.0_f64.sqrt()
}

/// `DSS.LogThisEvent(name)` with the solution clock (caller gates on `LogEvents`).
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

/// Pascal `NCIM_DoZBus` (`NCIMSolutionHelper.pas` l.289): stamp a constant-
/// impedance node's YPrim diagonal `y00` into the Jacobian diagonal 2×2 block and
/// add its `I = V·y00` into the mismatch. `y00` is the `[0][0]` entry of the
/// element's balanced YPrim (Pascal `pYMat[1]`).
fn do_zbus(sol: &mut Solution, i: usize, v: Complex64, y00: Complex64) {
    let jac = sol.ncim_jacobian.as_mut().expect("jacobian built");
    let base = 2 * (i - 1);
    let curr = v * y00;
    // dImdVr / dIrdVm / dImdVm / dIrdVr (Pascal LCoords (0,0),(1,1),(0,1),(1,0)).
    jac.set_element(base, base, y00.im);
    jac.set_element(base + 1, base + 1, -y00.im);
    jac.set_element(base, base + 1, y00.re);
    jac.set_element(base + 1, base, y00.re);
    sol.ncim_delta_f[base] += curr.im;
    sol.ncim_delta_f[base + 1] += curr.re;
}

/// Pascal `NCIM_DoPQBus` (l.246): stamp a PQ node's power-injection derivative
/// into the Jacobian diagonal block and add its current mismatch.
fn do_pqbus(sol: &mut Solution, i: usize, v: Complex64, power: Complex64) {
    let jac = sol.ncim_jacobian.as_mut().expect("jacobian built");
    let base = 2 * (i - 1);
    let pow = power.conj();
    let vc2 = v.conj() * v.conj();
    let fa_vr = Complex64::new(-1.0, 0.0) / vc2;
    let fa_vm = Complex64::new(0.0, 1.0) / vc2;
    let curr = (power / v).conj();
    jac.set_element(base, base, (fa_vr * pow).im);
    jac.set_element(base + 1, base + 1, (fa_vm * pow).re);
    jac.set_element(base, base + 1, (fa_vm * pow).im);
    jac.set_element(base + 1, base, (fa_vr * pow).re);
    sol.ncim_delta_f[base] += curr.im;
    sol.ncim_delta_f[base + 1] += curr.re;
}

/// Pascal `NCIM_DoPVBus` (l.171): stamp a PV node's power-injection derivative
/// (negated, since a generator injects) plus the voltage-regulation coupling
/// cells `Z`/`X` that tie the node voltage to the PV-bus Q equation. `num_nodes`
/// is `ActiveCircuit.NumNodes` (the offset of the gen rows).
fn do_pvbus(sol: &mut Solution, num_nodes: usize, i: usize, vtarget: f64, power: Complex64) {
    let base = 2 * (i - 1);
    let v = sol.node_v[i];
    let pow = power.conj();
    let curr = (power / v).conj();
    let vc2 = v.conj() * v.conj();
    let fa_vr = Complex64::new(-1.0, 0.0) / vc2;
    let fa_vm = Complex64::new(0.0, 1.0) / vc2;
    {
        let jac = sol.ncim_jacobian.as_mut().expect("jacobian built");
        jac.set_element(base, base, -(fa_vr * pow).im);
        jac.set_element(base + 1, base + 1, -(fa_vm * pow).re);
        jac.set_element(base, base + 1, -(fa_vm * pow).im);
        jac.set_element(base + 1, base, -(fa_vr * pow).re);
    }
    sol.ncim_delta_f[base] -= curr.im;
    sol.ncim_delta_f[base + 1] -= curr.re;

    // Voltage-regulation subsection.
    let vmag = v.norm();
    // GCoord (1-based) = NumNodes*2 + PVBusIdx[i] - 1  → 0-based gz.
    let gz = num_nodes * 2 + sol.ncim_pv_bus_idx[i] as usize - 1 - 1;
    sol.ncim_delta_f[gz] = vtarget - vmag;
    let den = vmag * vmag;
    let jac = sol.ncim_jacobian.as_mut().expect("jacobian built");
    // Regulation coefficients Z (row = gz, cols = base, base+1).
    jac.set_element(gz, base, -v.re / vmag);
    jac.set_element(gz, base + 1, -v.im / vmag);
    // Power regulation coefficients X (rows = base, base+1, col = gz).
    jac.set_element(base, gz, v.conj().re / den);
    jac.set_element(base + 1, gz, v.conj().im / den);
}

/// Pascal `NCIM_InitVectors` (l.326): (re)allocate the per-node NCIM arrays,
/// 1-based with a dummy slot 0 (`NodeType[0] = -1` "ignore").
fn ncim_init_vectors(ckt: &mut Circuit) {
    let n = ckt.num_nodes + 1;
    let sol = &mut ckt.solution;
    sol.ncim_node_power = vec![ZERO; n];
    sol.ncim_gen_power = vec![ZERO; n];
    sol.ncim_node_type = vec![NCIM_PQ_NODE; n];
    sol.ncim_node_type[0] = -1;
    sol.ncim_pv_bus_idx = vec![0; n];
    sol.ncim_node_pv_target = vec![0.0; n];
    sol.ncim_node_limits = vec![ZERO; n];
    sol.ncim_node_num_gen = vec![0; n];
}

/// The NCIM flat-start phase rotation (`NCIMSolutionHelper.pas` l.387).
const FLAT_START_ANG: [f64; 3] = [
    0.0,
    4.0 * std::f64::consts::PI / 3.0,
    2.0 * std::f64::consts::PI / 3.0,
];

/// Pascal `NCIM_DoForceFlatStart` (l.375), per-node part: every node to its bus
/// `kVBase` magnitude with the standard 0°/240°/120° phase rotation. The slack
/// (source) override is applied by the caller ([`ncim_init`]), which has the
/// element store to read the source's kVBase/PerUnit/Angle.
fn ncim_do_force_flat_start(ckt: &mut Circuit) {
    let mut aidx = 0usize;
    for i in 1..=ckt.num_nodes {
        let bus_ref = ckt.map_node_to_bus[i].bus_ref;
        let mag = ckt.buses[bus_ref].kv_base * 1e3;
        ckt.solution.node_v[i] = Complex64::from_polar(mag, FLAT_START_ANG[aidx]);
        aidx += 1;
        if aidx >= 3 {
            aidx = 0;
        }
    }
}

/// Read the first circuit element's VSource `(kVBase, PerUnit, Angle)`.
fn read_first_vsource(ckt: &Circuit, env: &SolveEnv) -> Option<(f64, f64, f64)> {
    let r = *ckt.ckt_elements.first()?;
    let src = env.store.typed::<VSource>(r)?;
    Some((src.kv_base, src.per_unit, src.angle))
}

/// Pascal `NCIM_LoadYBus` (l.517): factor the PDE-only Y (`hYseries`) and dump it
/// into the `NCIM_Y/Row/Col` triplet arrays that drive `I = Y·V` and the Jacobian
/// network blocks. `coo_entries` returns the summed nonzeros column-major, 0-based
/// — the same content as KLUSolve `GetTripletMatrix` after `FactorSparseMatrix`.
fn ncim_load_y_bus(ckt: &mut Circuit) -> SolveResult {
    let sparse = ckt
        .solution
        .y_series
        .as_mut()
        .ok_or_else(|| "Y Matrix not Built.".to_string())?;
    sparse
        .factor()
        .map_err(|e| format!("NCIM: error factoring PDE-only Y matrix: {e}"))?;
    let (rows, cols, vals) = sparse
        .coo_entries()
        .map_err(|e| format!("NCIM: error reading PDE-only Y triplets: {e}"))?;
    ckt.solution.ncim_y_row = rows;
    ckt.solution.ncim_y_col = cols;
    ckt.solution.ncim_y = vals;
    Ok(())
}

/// Pascal `NCIM_Init` (l.481): build the PDE-only Y, allocate the node vectors,
/// flat-solve for an initial voltage estimate, and load the Y triplets. Returns
/// the node count (`GetSize(hY)` = `NumNodes`).
fn ncim_init(ckt: &mut Circuit, env: &mut SolveEnv, init_y: bool) -> Result<usize, String> {
    build_y_matrix(ckt, env, BuildOption::PdeOnly, false)?;
    ncim_init_vectors(ckt);
    ckt.solution.zero_inj_curr();
    get_source_inj_currents(ckt, env);
    if ckt.log_events {
        log_event(ckt, "Solve Sparse Set DoNCIMSolution ...");
    }
    if init_y {
        ckt.solution.solve_system()?;
        // Flat start reads the source's kVBase/PerUnit/Angle (needs `env`).
        let src = read_first_vsource(ckt, env);
        ncim_do_force_flat_start(ckt);
        // Slack-bus override: the first circuit element is the source (VSource).
        if let Some((kv_base, per_unit, angle)) = src {
            let mag = (kv_base * 1e3 / sqrt3()) * per_unit;
            // r4133 `DOForceFlatStart` (`Common/Solution.pas` l.1650-1654) writes
            // `NodeV[1..3]` unconditionally. On a circuit with fewer than three
            // nodes that runs past `ReAllocMem(NodeV, … * (NumNodes+1))` — an
            // unchecked overrun in FPC that corrupts the DLL's heap (own
            // `epri-worker` probe 2026-09-03 on a 1-phase 8-line deck: r4133
            // answers `converged=False`, 15 iterations, then the worker cannot
            // `quit`). NCIM is 3-phase-shaped throughout — `CalcInjCurr` zeroes
            // `deltaF[0..5]` as "the swing bus" (l.1874-1875) and `BuildJacobian`
            // special-cases `GRow < 6` (l.2312-2340) — so on a sub-3-node circuit
            // there is nothing to converge to either way; clamp rather than
            // reproduce the overrun (CLAUDE.md: upstream bugs are never
            // reproduced). Identical on every circuit with ≥ 3 nodes.
            for i in 1..=3usize.min(ckt.num_nodes) {
                let ang = (angle * std::f64::consts::PI / 180.0) + FLAT_START_ANG[i - 1];
                ckt.solution.node_v[i] = Complex64::from_polar(mag, ang);
            }
        }
    }
    let nnodes = ckt
        .solution
        .y_series
        .as_ref()
        .map(|s| s.size())
        .unwrap_or(0);
    ncim_load_y_bus(ckt)?;
    ckt.solution.ncim_ready = true;
    Ok(nnodes)
}

/// Pascal `TSolutionObj.GetNumGenerators` (**r4133** `Common/Solution.pas`
/// l.1884-1990; `NCIM_GetNumGenerators` l.574 in the retired capi015 refactor):
/// classify each enabled generator as a
/// PV-bus (model-3 with Q-limits) participant, assign its `NCIM_Idx`, tally the
/// per-node Q-limits and generator counts. Returns the total number of PV-bus
/// generator phases (the size of the Jacobian's voltage-regulation section).
fn ncim_get_num_generators(ckt: &mut Circuit, env: &mut SolveEnv, init_q: bool) -> i32 {
    for idx in 0..ckt.solution.ncim_node_num_gen.len() {
        ckt.solution.ncim_node_num_gen[idx] = 0;
        ckt.solution.ncim_node_limits[idx] = ZERO;
    }
    let mut result = 0i32;
    let mut bus_refs: Vec<usize> = Vec::new();
    let gens = ckt.generators.clone();
    for r in gens {
        let gobj = env
            .store
            .typed_mut::<Generator>(r)
            .expect("generators list holds Generators");
        if !gobj.cd.enabled {
            continue;
        }
        let nphases = gobj.cd.nphases;
        let qmax = (gobj.kvar_max * 1e3) / nphases as f64;
        let qmin = (gobj.kvar_min * 1e3) / nphases as f64;
        let add2limits;
        if gobj.gen_model == 3 {
            if init_q {
                gobj.delta_q_nom = vec![0.0; nphases];
            }
            if gobj.kvar_max == 0.0 && gobj.kvar_min == 0.0 {
                // No Q-limits declared → cannot regulate; demote to PQ (model 4).
                // The *conversion record* (r4133's `PV2PQList` append, the port's
                // `ncim_expv`) is gated on `InitQ` (r4133 l.1936-1939) — only the
                // initializing pass logs it. Reachable with `InitQ = false` only
                // when a generator is edited back to `model=3` with zero Q-limits
                // between two solves (the PQ→PV promotion at l.2216 requires
                // nonzero limits, so it can never re-create this shape itself).
                gobj.gen_model = 4;
                if init_q {
                    gobj.ncim_expv = true;
                }
                continue;
            }
            let target = gobj.cd.node_ref[0];
            let mut bidx: i32 = -1;
            for (k, &b) in bus_refs.iter().enumerate() {
                if b == target {
                    bidx = k as i32;
                    break;
                }
            }
            if bidx < 0 {
                gobj.ncim_idx = result + 1;
                result += nphases as i32;
                bus_refs.extend_from_slice(&gobj.cd.node_ref[..nphases]);
            } else {
                gobj.ncim_idx = bidx + 1;
            }
            add2limits = true;
        } else {
            // r4133 l.1972-1973: `Else Add2Limits := pGen.GenModel = 4;`. (The
            // retired capi015 r4103 form also OR-ed in `GenModel = 3 and
            // NCIM_ExPV` here — unreachable inside this `else`, dropped.)
            add2limits = gobj.gen_model == 4;
        }
        if add2limits {
            let refs: Vec<usize> = gobj.cd.node_ref[..nphases].to_vec();
            for nr in refs {
                ckt.solution.ncim_node_limits[nr] += Complex64::new(qmax, qmin);
                ckt.solution.ncim_node_num_gen[nr] += 1;
            }
        }
    }
    result
}

/// Pascal `NCIM_CalcInjCurr` (l.540): compute the injection-current mismatch
/// `deltaF = Y·V` (imag then real per node), zeroing the swing-bus rows.
fn ncim_calc_inj_curr(ckt: &mut Circuit, env: &mut SolveEnv, init_gen_q: bool) {
    let gsize =
        ckt.solution.ncim_nodes * 2 + ncim_get_num_generators(ckt, env, init_gen_q) as usize;
    ckt.solution.ncim_delta_f = vec![0.0; gsize];
    ckt.solution.ncim_delta_z = vec![0.0; gsize];
    let ylen = ckt.solution.ncim_y.len();
    for k in 0..ylen {
        let col = ckt.solution.ncim_y_col[k];
        let row = ckt.solution.ncim_y_row[k];
        let myvalue = ckt.solution.ncim_y[k] * ckt.solution.node_v[col + 1];
        ckt.solution.ncim_delta_f[row * 2] += myvalue.im;
        ckt.solution.ncim_delta_f[row * 2 + 1] += myvalue.re;
    }
    // The first 6 elements (swing bus) are exactly zero (Pascal `for i := 0 to 5`).
    // Defensive `.min` keeps a degenerate sub-3-node system from panicking; NCIM
    // targets 3-phase systems, where `gsize >= 6`.
    for f in ckt.solution.ncim_delta_f.iter_mut().take(6) {
        *f = 0.0;
    }
}

/// Pascal `NCIM_GetPowers` (l.70): walk the PC elements and accumulate each node's
/// total injected power (loads subtract, generators add), classifying nodes PQ/PV
/// and stamping constant-impedance contributions directly (`NCIM_DoZBus`).
fn ncim_get_powers(ckt: &mut Circuit, env: &mut SolveEnv) {
    // NOTE: Faults are PD elements in this port (not PC), so the Pascal
    // `FAULTOBJECT` branch of the PCElements loop is unreachable there and here;
    // a fault's admittance is already in the PDE-only Y. Only Loads and
    // Generators appear in `pc_elements`.
    let pcs = ckt.pc_elements.clone();
    for r in pcs {
        if env.store.typed::<Load>(r).is_some() {
            let load = env.store.typed_mut::<Load>(r).expect("checked above");
            if !load.cd.enabled {
                continue;
            }
            let nphases = load.cd.nphases;
            let is_const_z = load.load_model == LoadModel::ConstZ;
            let w = load.w_nominal;
            let var = load.var_nominal;
            let y00 = load.cd.yprim.as_ref().map(|m| m.get(0, 0)).unwrap_or(ZERO);
            for p in 0..nphases {
                let node_idx = load.cd.node_ref[p];
                if node_idx == 0 {
                    continue;
                }
                let ld_power = Complex64::new(w, var);
                let ld_volt = ckt.solution.node_v[node_idx];
                if is_const_z {
                    do_zbus(&mut ckt.solution, node_idx, ld_volt, y00);
                } else {
                    if ckt.solution.ncim_node_type[node_idx] == NCIM_PV_NODE {
                        ckt.solution.ncim_node_power[node_idx] -= ld_power;
                    } else {
                        ckt.solution.ncim_node_power[node_idx] += ld_power;
                    }
                    load.cd.iterminal[p] = (ld_power / ld_volt).conj();
                }
            }
        } else if env.store.typed::<Generator>(r).is_some() {
            let gobj = env.store.typed_mut::<Generator>(r).expect("checked above");
            if !gobj.cd.enabled {
                continue;
            }
            let nphases = gobj.cd.nphases;
            let gen_model = gobj.gen_model;
            let p_nom = gobj.p_nominal_per_phase;
            let q_nom = gobj.q_nominal_per_phase;
            let v_target = gobj.v_target;
            let ncim_idx = gobj.ncim_idx;
            let delta_q = gobj.delta_q_nom.clone();
            let y00 = gobj.cd.yprim.as_ref().map(|m| m.get(0, 0)).unwrap_or(ZERO);
            for p in 0..nphases {
                let node_idx = gobj.cd.node_ref[p];
                if node_idx == 0 {
                    continue;
                }
                match gen_model {
                    3 => {
                        // PV bus.
                        let q = delta_q.get(p).copied().unwrap_or(0.0);
                        // Pascal `NCIM_GetPowers` l.121:
                        // `Qnominalperphase := deltaQNom[idx-1]`. Persist the
                        // per-phase Q so the reported generator reactive power
                        // (`present_kvar` = `Qnominalperphase·nphases/1000`)
                        // reflects the solved value, matching the oracle. No
                        // effect on the solve (`gen_s` already uses `q`).
                        gobj.q_nominal_per_phase = q;
                        let gen_s = Complex64::new(p_nom, q);
                        if ckt.solution.ncim_node_type[node_idx] == NCIM_PQ_NODE {
                            ckt.solution.ncim_node_power[node_idx] =
                                -ckt.solution.ncim_node_power[node_idx];
                        }
                        ckt.solution.ncim_node_type[node_idx] = NCIM_PV_NODE;
                        ckt.solution.ncim_node_power[node_idx] += gen_s;
                        ckt.solution.ncim_gen_power[node_idx] += gen_s;
                        ckt.solution.ncim_node_pv_target[node_idx] = v_target;
                        ckt.solution.ncim_pv_bus_idx[node_idx] = ncim_idx + (p as i32 + 1);
                    }
                    4 => {
                        // PQ bus.
                        let gen_s = if delta_q.is_empty() {
                            Complex64::new(p_nom, q_nom)
                        } else {
                            Complex64::new(p_nom, delta_q[0])
                        };
                        if ckt.solution.ncim_node_type[node_idx] == NCIM_PQ_NODE {
                            ckt.solution.ncim_node_power[node_idx] -= gen_s;
                        } else {
                            ckt.solution.ncim_node_power[node_idx] += gen_s;
                        }
                        ckt.solution.ncim_gen_power[node_idx] += gen_s;
                    }
                    _ => {
                        // Constant impedance.
                        let v = ckt.solution.node_v[node_idx];
                        do_zbus(&mut ckt.solution, node_idx, v, y00);
                    }
                }
            }
        }
    }
}

/// Pascal `NCIM_ApplyCurr` (l.53): apply each node's accumulated power injection
/// to the Jacobian + mismatch (PV via `NCIM_DoPVBus`, else `NCIM_DoPQBus`).
fn ncim_apply_curr(ckt: &mut Circuit) {
    let num_nodes = ckt.num_nodes;
    let len = ckt.solution.ncim_node_power.len();
    for i in 1..len {
        let power = ckt.solution.ncim_node_power[i];
        if power.re == 0.0 && power.im == 0.0 {
            continue;
        }
        if ckt.solution.ncim_node_type[i] == NCIM_PV_NODE {
            let vtarget = ckt.solution.ncim_node_pv_target[i];
            do_pvbus(&mut ckt.solution, num_nodes, i, vtarget, power);
        } else {
            let v = ckt.solution.node_v[i];
            do_pqbus(&mut ckt.solution, i, v, power);
        }
    }
}

/// Pascal `NCIM_BuildJacobian` (l.904): assemble a fresh Jacobian from the PDE-
/// only Y network blocks (`[B, G; G, −B]`, identity on the swing), reserve the
/// PV-bus voltage-regulation cells (`NCIM_InitPVBusJac`), and clear the per-node
/// power/type accumulators for the next `NCIM_GetPowers`.
fn ncim_build_jacobian(ckt: &mut Circuit, env: &mut SolveEnv) {
    let n = ckt.solution.ncim_delta_f.len();
    let mut jac = RealSparseSet::new(n);
    let ylen = ckt.solution.ncim_y.len();
    for k in 0..ylen {
        let grow = ckt.solution.ncim_y_row[k] * 2;
        let gcol = ckt.solution.ncim_y_col[k] * 2;
        let y = ckt.solution.ncim_y[k];
        if grow == gcol && grow < 6 {
            // Swing-bus diagonal: identity (V fixed).
            jac.set_element(grow, gcol, 1.0);
            jac.set_element(grow + 1, gcol + 1, 1.0);
        } else if grow >= 6 && gcol >= 6 {
            // Network 2×2 block [B, G; G, −B].
            let b = y.im;
            let g = y.re;
            jac.set_element(grow, gcol, b);
            jac.set_element(grow, gcol + 1, g);
            jac.set_element(grow + 1, gcol, g);
            jac.set_element(grow + 1, gcol + 1, -b);
        }
    }
    // Pascal `NCIM_InitPVBusJac` (Generator.pas l.2853): reserve tiny (1e-20)
    // placeholder cells for the voltage/power-regulation coefficients of every
    // enabled model-3 generator, so the sparsity pattern is stable before
    // `NCIM_DoPVBus` fills the real values.
    let num_nodes = ckt.num_nodes;
    let gens = ckt.generators.clone();
    for r in gens {
        let gobj = env
            .store
            .typed::<Generator>(r)
            .expect("generators list holds Generators");
        if gobj.cd.enabled && gobj.gen_model == 3 {
            let nphases = gobj.cd.nphases;
            let ncim_idx = gobj.ncim_idx as usize;
            for i in 1..=nphases {
                // GCoord (1-based) = NumNodes*2 + (NCIM_Idx + i - 1).
                let gcoord = num_nodes * 2 + (ncim_idx + i - 1);
                let nr = gobj.cd.node_ref[i - 1];
                let gcoord_y = nr * 2 - 1; // 1-based
                jac.set_element(gcoord - 1, gcoord_y - 1, 1e-20);
                jac.set_element(gcoord - 1, gcoord_y, 1e-20);
            }
        }
    }
    ckt.solution.ncim_jacobian = Some(jac);
    for j in 0..ckt.solution.ncim_node_power.len() {
        ckt.solution.ncim_node_power[j] = ZERO;
        ckt.solution.ncim_gen_power[j] = ZERO;
        ckt.solution.ncim_node_type[j] = NCIM_PQ_NODE;
    }
}

/// Pascal `NCIM_InitPQGen` (l.419): seed the model-4 (PQ) generators' `deltaQNom`
/// with their nominal per-phase Q.
fn ncim_init_pq_gen(ckt: &mut Circuit, env: &mut SolveEnv) {
    let gens = ckt.generators.clone();
    for r in gens {
        let gobj = env
            .store
            .typed_mut::<Generator>(r)
            .expect("generators list holds Generators");
        if gobj.cd.enabled && gobj.gen_model != 3 {
            // r4133 `InitPQGen` (`Common/Solution.pas` l.1678-1679) sizes this to
            // **1**, but every *writer* indexes it per phase: the PV arm's own
            // stamp (l.2107), the PV→PQ clamp (l.2155) and the PQ→PV promotion
            // (l.2255) all run `j := 0 to NPhases-1`. So a machine born `model=4`
            // that the promotion arm later flips to PV writes past the end of the
            // length-1 dynamic array — unchecked in FPC; measured 2026-09-03 on an
            // 8-line deck (`tmp/rp313/repro_pq2pv.dss`) it **corrupts the r4133
            // DLL**: the solve itself still answers (`converged=True`, 5
            // iterations, node voltages the port matches to the digit — own
            // `epri-worker` probe, re-measured in the RP3.13 audit settlement),
            // and the DLL then **hangs on the first element access after it**
            // (`set_active_element Line.l1` never returns; a run killed at 150 s
            // had burned 0.12 s of worker CPU: blocked, not spinning). The port
            // panicked here instead. Same class as the
            // `Bus_Int_Duration` overrun; not reproduced (CLAUDE.md 2026-08-02).
            // `deltaQNom` is per-phase state, so size it that way — every reader of
            // the scalar form takes `[0]` (l.1326 model-4 power, l.2306 the ELSE
            // arm's current stamp), which this leaves bit-identical, and the
            // model-3 path is re-sized by `GetNumGenerators` (l.1928-1930) before
            // anything indexes it.
            gobj.delta_q_nom = vec![gobj.q_nominal_per_phase; gobj.cd.nphases];
        }
    }
}

/// Pascal `TSolutionObj.UpdateGenQ` (**r4133** `Common/Solution.pas` l.1993-2322;
/// byte-identical in r4088 apart from commented-out debug I/O): update every
/// generator's reactive-power delta from the solved `NCIM_deltaZ` gen-section,
/// enforce Q-limits with automatic PV↔PQ (model 3↔4) switching, and refresh the
/// reported terminal currents.
///
/// The `if GenModel = 3 … else …` split (r4133 l.2059/l.2166) is the **switching
/// cadence**: PV→PQ and PQ→PV conversions are mutually exclusive within one
/// Newton pass. Adopting it (ORPHANED_GAPS §1.6) is what makes IEEE118Bus
/// converge; the retired capi015 r4103 form ran both tests every pass.
fn ncim_update_gen_q(ckt: &mut Circuit, env: &mut SolveEnv) {
    if ckt.generators.is_empty() {
        return;
    }
    let num_nodes = ckt.num_nodes;
    let gen_idx = num_nodes * 2;
    // QDelta[0] is a dummy zero; QDelta[k] = -deltaZ[gen_idx + k - 1].
    let mut qdelta: Vec<f64> = vec![0.0];
    for i in gen_idx..ckt.solution.ncim_delta_z.len() {
        qdelta.push(-ckt.solution.ncim_delta_z[i]);
    }

    let ignore_q = ckt.solution.ncim_ignore_q_limit;
    let gen_gain = ckt.solution.ncim_gen_gain;
    let mut q_node_ref: Vec<usize> = Vec::new();
    let mut q_node_ref_pq: Vec<usize> = Vec::new();
    let mut pq_checked: Vec<usize> = Vec::new();

    let gens = ckt.generators.clone();
    for r in gens {
        let gobj = env
            .store
            .typed_mut::<Generator>(r)
            .expect("generators list holds Generators");
        if !gobj.cd.enabled {
            continue;
        }
        let nphases = gobj.cd.nphases;
        let node_refs: Vec<usize> = gobj.cd.node_ref[..nphases].to_vec();

        if gobj.gen_model == 3 {
            let mut pv_ok = true;
            // Search for this bus among generators already handled.
            let bidx = q_node_ref.iter().position(|&b| b == node_refs[0]);
            if bidx.is_none() {
                for (j, &nr) in node_refs.iter().enumerate() {
                    let qmax = ckt.solution.ncim_node_limits[nr].re;
                    let qmin = ckt.solution.ncim_node_limits[nr].im;
                    let volt = ckt.solution.node_v[nr];
                    let shift = gobj.ncim_idx as usize + j;
                    let mut gen_q =
                        gobj.delta_q_nom[j] + qdelta.get(shift).copied().unwrap_or(0.0) * gen_gain;
                    if !ignore_q {
                        if gen_q >= 0.0 {
                            pv_ok = pv_ok && (gen_q < qmax);
                        } else {
                            pv_ok = pv_ok && (gen_q > qmin);
                        }
                    } else if gobj.kvar_max == 0.0 && gobj.kvar_min == 0.0 {
                        gen_q = 0.0;
                    }
                    if let Some(s) = qdelta.get_mut(shift) {
                        *s = 0.0;
                    }
                    gobj.delta_q_nom[j] = gen_q;
                    gobj.cd.iterminal[j] =
                        -(Complex64::new(gobj.p_nominal_per_phase, gobj.delta_q_nom[j]) / volt)
                            .conj();
                }
            } else {
                pv_ok = false;
            }

            if !pv_ok {
                gobj.gen_model = 4;
                gobj.ncim_expv = true;
                let (qmax, qmin) = if bidx.is_none() {
                    q_node_ref.extend_from_slice(&node_refs);
                    (
                        ckt.solution.ncim_node_limits[node_refs[0]].re,
                        ckt.solution.ncim_node_limits[node_refs[0]].im,
                    )
                } else {
                    (0.0, 0.0)
                };
                // r4133 l.2148-2156: the sign test re-reads `deltaQNom[0]` on
                // every phase, so phase 0's own overwrite feeds the later phases
                // (identical for the usual `qMax >= 0 > qMin`, faithful when not).
                for j in 0..nphases {
                    gobj.delta_q_nom[j] = if gobj.delta_q_nom[0] >= 0.0 {
                        qmax
                    } else {
                        qmin
                    };
                }
            }
        } else {
            // r4133 l.2166-2309 — the ELSE arm. A generator that entered this
            // pass as model 3 is NOT re-examined here even after the branch above
            // converted it to model 4: the PQ→PV test only ever sees generators
            // that were already PQ when the pass began. (The retired capi015
            // r4103 loop ran this block unconditionally, so a just-converted
            // PV→PQ generator could be flipped straight back to PV in the same
            // pass — the PV↔PQ chatter that stalls IEEE118Bus at 100 iterations
            // while r4088/r4133 converge. See ORPHANED_GAPS §1.6 / DIVERGENCES.)
            if gobj.gen_model == 4 && gobj.kvar_max != 0.0 && gobj.kvar_min != 0.0 {
                let mut pq_ok = true;
                let bidx = q_node_ref_pq.iter().position(|&b| b == node_refs[0]);
                if bidx.is_none() {
                    let checked = pq_checked.iter().any(|&b| b == node_refs[0]);
                    if !checked {
                        let my_vmax = gobj.v_base * gobj.vpu;
                        for &nr in &node_refs {
                            let vnode = ckt.solution.node_v[nr].norm();
                            if gobj.delta_q_nom[0] > 0.0 {
                                pq_ok = pq_ok && (vnode <= my_vmax);
                            } else {
                                pq_ok = pq_ok && (vnode >= my_vmax);
                            }
                            pq_checked.push(nr);
                        }
                    }
                } else {
                    pq_ok = false;
                }
                if !pq_ok {
                    gobj.gen_model = 3;
                    for (j, &nr) in node_refs.iter().enumerate() {
                        let (qmax, qmin) = if bidx.is_none() {
                            q_node_ref_pq.push(nr);
                            (
                                ckt.solution.ncim_node_limits[nr].re,
                                ckt.solution.ncim_node_limits[nr].im,
                            )
                        } else {
                            (0.0, 0.0)
                        };
                        // r4133 l.2253: `deltaQNom[0]` re-read per phase (as above).
                        gobj.delta_q_nom[j] = if gobj.delta_q_nom[0] >= 0.0 {
                            qmax
                        } else {
                            qmin
                        };
                    }
                    gobj.ncim_expv = false;
                }
            }

            // r4133 l.2300-2307 "Update currents for all the other gen models" —
            // inside the ELSE arm too: a model-3 generator's `Iterminal` was
            // already stamped per phase (`deltaQNom[j]`) in the PV branch above.
            if !gobj.delta_q_nom.is_empty() {
                let q0 = gobj.delta_q_nom[0];
                let p = gobj.p_nominal_per_phase;
                for (j, &nr) in node_refs.iter().enumerate() {
                    let volt = ckt.solution.node_v[nr];
                    gobj.cd.iterminal[j] = -(Complex64::new(p, q0) / volt).conj();
                }
            }
        }
    }
}

/// Pascal `TVsourceObj.CalcInjCurrAtBus` (r4133 `PCElements/VSource.pas` l.1085),
/// reached from `TVsourceObj.GetCurrents` (l.1194-1195) when `Algorithm =
/// NCIMSOLVE` and `NodeRef[1] = 1`: the swing source's NCIM-reported terminal
/// currents. NCIM holds the swing bus at the ideal EMF, so `YPrim·V - Iinj` is ~0
/// there; instead the source's terminal current is the Kirchhoff sum at its bus —
/// **minus** every other connected element's terminal current, PD (the PD loop,
/// l.1113-1138) and PC (the PC loop, l.1148-1173) alike. The swing source is the
/// [`VSource`] whose first node is the global slack (`NodeRef[0] == 1`); with
/// none, nothing is stamped.
///
/// **Upstream sign bug, NOT reproduced** (CLAUDE.md 2026-08-02: an upstream bug
/// is never reproduced in any lane). r4133 subtracts the PD terms (`csub`,
/// l.1135) but **adds** the PC terms (`cadd`, l.1169). Every `GetCurrents` in
/// OpenDSS returns the current flowing *into* the element ("Gets total Currents
/// going INTO a device's terminals", `TPCElement.GetCurrents`), for PC and PD
/// alike — a load reports `+P`, a generator `−P` — so KCL at the bus is
/// `I(source) + Σ I(others) = 0` and the source's own share is `−Σ`, both loops
/// subtracting. Measured 2026-09-03 on `tmp/rp313/settle/swing_pc.dss` (a 1000 kW
/// / 400 kvar `Load.ldswing` bonded straight onto `sourcebus`, live `epri-worker`
/// vs this engine): both engines print the same non-converged 15-iteration state
/// to the digit (`SOURCEBUS.1 = 7532.646248 ∠−5.0617°`) and the same
/// `Line.L1`/`Load.LDSWING` terminal currents, and r4133's own `Export Currents`
/// gives `Vsource.SOURCE` conductor 1 `−12.910456 + 52.625721j A`
/// (`54.1862 ∠103.78°`) = `−I(Line.l1 t1) + I(Load.ldswing)`, leaving a KCL
/// residual of `85.035098 − 43.071927j A` = exactly `2·I(Load.ldswing)`. This
/// port subtracts both loops, so the same read is `−97.945554 + 95.697649j A`
/// (`136.9357 ∠135.669°`) and KCL closes to `< 1e-9 A`. Pinned by
/// `exec::tests::ncim::ncim_swing_sum_subtracts_pc_terminals_and_closes_kcl`,
/// which names both engines' numbers. **Zero oracle exposure**: the divergence
/// needs a PC element other than the source on the slack node, none of the five
/// r4133-gated NCIM cases has one (tripwire
/// `ncim_swing_bus_carries_no_pc_element_on_the_gated_decks`), and any deck that
/// grows one stops converging under NCIM on *both* engines (measured: 10 kW,
/// 100 kW, 1000 kW loads and a 500 kW generator all hit the iteration limit) —
/// so no ledger entry and no golden byte moves. Upstream report:
/// `investigations/to_opendss/54-ncim-calcinjcurratbus-pc-sign.md`.
///
/// **Stamped once here, echoed by every reader** (RP3.13). `CalcInjCurrAtBus`
/// needs every element at the bus, which `get_currents(&mut self, sys, node_v,
/// curr)` cannot reach from inside one element; so the sum is computed once at
/// the end of [`do_ncim_solution`], written into the source's `Iterminal`, and
/// returned from there by `TVsourceObj.GetCurrents`' NCIM arm
/// (`elements/pc/vsource/solve.rs`) - one live state for the element path
/// (`Export Currents`/`Powers`, `Show`, the CLI, monitors, meters) and for
/// `Dss::snapshot_elements`, which the corpus gate reads and which carried this
/// sum as a private override until RP3.13.
///
/// **Why the once-at-convergence stamp equals r4133's on-demand recompute.**
/// r4133 re-runs the whole sum on every read, calling `ActivePDE.GetCurrents`
/// (l.1123) / `ActivePCE.GetCurrents` (l.1158) fresh - a *direct* call that
/// bypasses the `ComputeIterminal` `SolutionCount` cache. This function makes the
/// same recompute here: [`CktElement::refresh_iterminal`] on each element at the
/// bus (cache-bypassing, the same call `snapshot_elements` makes) against the
/// converged `NodeV` the Newton loop has just left in place. Nothing between this
/// point and a later read moves `NodeV` or any of those elements' state, so a
/// reader that recomputed instead would divide the same powers by the same
/// voltages and land on the same complex numbers - the stamp is not an
/// approximation of the live sum, it *is* that sum, taken at the only voltages
/// that exist after the solve. (Where the two shapes genuinely differ is a read
/// with no stamp behind it - a solve that aborted before this line, or
/// `Set algorithm=NCIM` typed after a normal `Solve` with no re-solve: r4133 sums
/// live, the port returns the last stamp.)
///
/// PD elements use the Pascal `Round(Yorder/2)` conductors-per-terminal stride
/// (l.1135, its 2-terminal assumption); PC elements use `NPhases` (l.1169).
fn ncim_stamp_swing_source_currents(ckt: &Circuit, env: &mut SolveEnv) {
    // The swing VSource: a source whose first node is the global slack node 1
    // (Pascal `NodeRef[1] = 1`, l.1194 - 1-based there, i.e. the *first* node
    // reference of terminal 1, which is what `node_ref.first()` is here).
    let Some(src_ref) = ckt.sources.iter().copied().find(|&r| {
        env.store.typed::<VSource>(r).is_some()
            && env.store.ckt_elem(r).cd().node_ref.first() == Some(&1)
    }) else {
        return;
    };

    let (src_bus, nphases, yorder) = {
        let cd = env.store.ckt_elem(src_ref).cd();
        let Some(t0) = cd.terminals.first() else {
            return;
        };
        (t0.bus_ref, cd.nphases, cd.yorder)
    };
    let sys = super::state::sys_ctx(ckt);
    let node_v = &ckt.solution.node_v;
    let mut curr = vec![ZERO; yorder];

    // The 0-based terminal of `cd` connected to the source bus, if any (Pascal
    // `BusName = StripExtension(ce.GetBus(j))`).
    //
    // NON-reproduced quirk (deliberate, per CLAUDE.md "do not reproduce UB"):
    // r4133 computes `myTerm` fresh per element only in the **PD** loop
    // (`myTerm := 0` inside `for idx in myList`, VSource.pas l.1119). In the **PC**
    // loop (l.1148) `myTerm := 0` is set ONCE before the loop (l.1146) and never
    // reset, so its terminal-finder `inc(myTerm)` accumulates across PCEs at the bus
    // — a stateful cross-element index (r4133 STILL has this; it is unrelated to the
    // off-by-one r4133 fixed). That accumulation is inert whenever each PCE connects
    // at its first terminal (`inc` never fires → myTerm stays 0), which is the only
    // deterministic in-range case; with a PCE bonded at a non-first terminal it can
    // run the `ElmCurrents[(myTerm*NPhases)+j]` index (l.1169) past
    // `SetLength(…, Yorder+1)` (l.1157) into an OOB heap read. We compute `my_term`
    // fresh per element for both loops: identical to r4133 on the defined path, and
    // refusing to reproduce the OOB.
    let my_term = |cd: &crate::elements::ckt::CktElementData| -> Option<usize> {
        (0..cd.nterms).find(|&t| cd.terminals.get(t).and_then(|x| x.bus_ref) == src_bus)
    };

    // r4133 fills a length-`Yorder+1` dynamic `ElmCurrents` with an **offset write**
    // — `ActivePDE.GetCurrents(@(ElmCurrents[1]))` (VSource.pas l.1123; the PC loop
    // l.1158) writes conductor 1 into `ElmCurrents[1]`, leaving slot 0 unused — then
    // reads `ElmCurrents[(myTerm*stride)+j]` with `j := 1..NPhases` (l.1135 PD /
    // l.1169 PC). That 1-based read of the offset-written array is UNSHIFTED: `j=1`
    // reads conductor 1. The port's `iterminal` is 0-based (`iterminal[0]` =
    // conductor 1 = r4133 `ElmCurrents[1]`), so the faithful index is `t*stride + i`
    // with `i := 0..nphases-1` (no `+1`). This aligns the port with r4133, the sole
    // live NCIM oracle (the retired capi015 0.15.0b4 (e936d210) wrote
    // `ce.GetCurrents(ElmCurrents)` at index 0 then read `ElmCurrents[j]` 1-based —
    // a one-conductor shift; the port formerly reproduced that shift as a documented
    // compat pin, dropped in the oracle-of-record flip capi015→r4133, own r4133
    // epri-worker probes 2026-07-20, `docs/upgrade/DIVERGENCES.md`). The read is
    // in-range by construction (`SetLength(ElmCurrents, Yorder+1)`); the defensive
    // `.get` returns 0 only for a degenerate multi-terminal stride overrun.
    let clip = |v: Option<&Complex64>| v.copied().unwrap_or(ZERO);

    // PD elements (+ faults) at the bus: subtract their terminal currents. Pascal
    // stride `Round(ce.Yorder / 2)` (its 2-terminal conductors-per-terminal).
    for &r in ckt.pd_elements.iter().chain(ckt.faults.iter()) {
        let elem = env.store.ckt_elem_mut(r);
        let cd = elem.cd();
        // `refresh_iterminal` (like Pascal's `GetCurrents`) indexes `NodeRef`, so
        // an unconnected element is skipped rather than recomputed - the same
        // guard `snapshot_elements` applied before its refresh, hence the same
        // set of elements whose `Iterminal` this sum read there.
        if !cd.enabled || cd.node_ref.is_empty() {
            continue;
        }
        let Some(t) = my_term(cd) else { continue };
        let stride = ((cd.yorder as f64) / 2.0).round() as usize;
        elem.refresh_iterminal(&sys, node_v);
        let cd = elem.cd();
        for (i, c) in curr.iter_mut().enumerate().take(nphases) {
            *c -= clip(cd.iterminal.get(t * stride + i));
        }
    }
    // PC elements (+ other sources) at the bus, excluding the source itself:
    // **subtract** their terminal currents (stride `ce.NPhases`, l.1169), which is
    // where this port leaves r4133 — see the sign-bug paragraph on the fn doc.
    for &r in ckt.pc_elements.iter().chain(ckt.sources.iter()) {
        if r == src_ref {
            continue;
        }
        let elem = env.store.ckt_elem_mut(r);
        let cd = elem.cd();
        if !cd.enabled || cd.node_ref.is_empty() {
            continue;
        }
        let Some(t) = my_term(cd) else { continue };
        let stride = cd.nphases;
        elem.refresh_iterminal(&sys, node_v);
        let cd = elem.cd();
        for (i, c) in curr.iter_mut().enumerate().take(nphases) {
            *c -= clip(cd.iterminal.get(t * stride + i));
        }
    }

    // The stamp. `curr` is `Yorder` long with zeros past `NPhases`, exactly the
    // buffer r4133 hands back (`Curr[1..Yorder] := CZero`, l.1110-1111, then only
    // `1..NPhases` written), so the whole vector is copied. Marked solved for this
    // `SolutionCount` so the cache-aware `ComputeIterminal` readers
    // (`Get_Powers`/`Get_Losses`) see it without a recompute; the cache-bypassing
    // ones re-enter `GetCurrents`, whose NCIM arm hands the same stamp back —
    // and `ncim_swing_stamped_at` is what tells that arm the stamp is this
    // element's and is current, so a *second* source on the slack node (which
    // this function never stamps) keeps reporting its own physical terminal
    // current instead of echoing an empty cache. r4133 has no such marker and
    // stack-overflows on that deck; see `VSource::ncim_swing_stamped_at`.
    let src = env
        .store
        .typed_mut::<VSource>(src_ref)
        .expect("the swing source was found as a VSource above");
    let n = curr.len().min(src.cd.iterminal.len());
    src.cd.iterminal[..n].copy_from_slice(&curr[..n]);
    src.cd.mark_iterminal_solved(sys.solution_count);
    src.ncim_swing_stamped_at = Some(sys.solution_count);
}

/// Pascal `DoNCIMSolution` (l.981): the NCIM Newton loop. `V ← V − ΔV` each
/// iteration until `Converged()` (the `NCIM_Converged` mismatch test) and the
/// min/max-iteration clause.
pub(crate) fn do_ncim_solution(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.iteration = 0;

    let mut init_gen_q = ckt.solution.ncim_init_gen_q;
    if init_gen_q {
        ncim_init_pq_gen(ckt, env);
    }

    if ckt.solution.system_y_changed || !ckt.solution.ncim_ready {
        ckt.solution.ncim_nodes = ncim_init(ckt, env, init_gen_q)?;
    }

    loop {
        ckt.solution.iteration += 1;
        ncim_calc_inj_curr(ckt, env, init_gen_q);
        ncim_build_jacobian(ckt, env);
        ncim_get_powers(ckt, env);
        ncim_apply_curr(ckt);

        if ckt.log_events {
            log_event(ckt, "Solve Power flow DoNCIMSolution ...");
        }

        // Solve J·deltaZ = deltaF.
        {
            let sol = &mut ckt.solution;
            let rhs = sol.ncim_delta_f.clone();
            let mut dz = vec![0.0; rhs.len()];
            let jac = sol
                .ncim_jacobian
                .as_mut()
                .ok_or_else(|| "NCIM: Jacobian not built".to_string())?;
            jac.solve(&rhs, &mut dz).map_err(|e| {
                format!("Error Solving NCIM Jacobian. Sparse matrix solver reports: {e}")
            })?;
            sol.ncim_delta_z = dz;
        }

        // Update the voltage vector: NodeV[i] -= (deltaZ[2(i-1)], deltaZ[2(i-1)+1]).
        for i in 1..=ckt.num_nodes {
            let dv_idx = (i - 1) * 2;
            let dv = Complex64::new(
                ckt.solution.ncim_delta_z[dv_idx],
                ckt.solution.ncim_delta_z[dv_idx + 1],
            );
            ckt.solution.node_v[i] -= dv;
        }

        let num_nodes = ckt.num_nodes;
        let solved = ckt.solution.converged(num_nodes);
        ncim_update_gen_q(ckt, env);
        init_gen_q = false;
        ckt.solution.ncim_init_gen_q = false;

        if (solved && ckt.solution.iteration >= ckt.solution.min_iterations)
            || ckt.solution.iteration >= ckt.solution.max_iterations
        {
            break;
        }
    }

    // The swing source reports the KCL sum at its bus under NCIM, not
    // `YPrim·V - Iinj` (Pascal `TVsourceObj.GetCurrents` takes the
    // `CalcInjCurrAtBus` branch whenever `Algorithm = NCIMSOLVE` and
    // `NodeRef[1] = 1` - r4133 `VSource.pas` l.1194). That sum needs every
    // element at the bus, so it is stamped here, once, at the converged `NodeV`;
    // `get_currents` echoes the stamp. See
    // [`ncim_stamp_swing_source_currents`].
    ncim_stamp_swing_source_currents(ckt, env);

    Ok(())
}
