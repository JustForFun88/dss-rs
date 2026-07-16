//! Port of `Common/NCIMSolutionHelper.pas`: the Newton Current-Injection Method
//! (NCIM) power-flow solver (`Set Algorithm=NCIM`).
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
    let src = env.store.obj(r).as_any().downcast_ref::<VSource>()?;
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
            for i in 1..=3usize {
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

/// Pascal `NCIM_GetNumGenerators` (l.574): classify each enabled generator as a
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
            .obj_mut(r)
            .as_any_mut()
            .downcast_mut::<Generator>()
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
                gobj.gen_model = 4;
                gobj.ncim_expv = true;
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
            add2limits = gobj.gen_model == 4 || (gobj.gen_model == 3 && gobj.ncim_expv);
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
        let obj = env.store.obj_mut(r);
        if obj.as_any().downcast_ref::<Load>().is_some() {
            let load = obj.as_any_mut().downcast_mut::<Load>().unwrap();
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
        } else if obj.as_any().downcast_ref::<Generator>().is_some() {
            let gobj = obj.as_any_mut().downcast_mut::<Generator>().unwrap();
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
            .obj(r)
            .as_any()
            .downcast_ref::<Generator>()
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
            .obj_mut(r)
            .as_any_mut()
            .downcast_mut::<Generator>()
            .expect("generators list holds Generators");
        if gobj.cd.enabled && gobj.gen_model != 3 {
            gobj.delta_q_nom = vec![gobj.q_nominal_per_phase];
        }
    }
}

/// Pascal `NCIM_UpdateGenQ` (l.682): update every generator's reactive-power delta
/// from the solved `NCIM_deltaZ` gen-section, enforce Q-limits with automatic
/// PV↔PQ (model 3↔4) switching, and refresh the reported terminal currents.
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
            .obj_mut(r)
            .as_any_mut()
            .downcast_mut::<Generator>()
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
                let sign = gobj.delta_q_nom[0] >= 0.0;
                for q in gobj.delta_q_nom.iter_mut().take(nphases) {
                    *q = if sign { qmax } else { qmin };
                }
            }
        }

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
                let sign = gobj.delta_q_nom[0] >= 0.0;
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
                    gobj.delta_q_nom[j] = if sign { qmax } else { qmin };
                }
                gobj.ncim_expv = false;
            }
        }

        // Update the reported terminal currents for every generator model.
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

    Ok(())
}
