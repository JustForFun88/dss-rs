//! The recursive-bisection bootstrap used by `InitKWayPartitioning`, ported 1:1
//! from `libmetis/kmetis.c::InitKWayPartitioning`, `pmetis.c`, `initpart.c`,
//! `refine.c`, `fm.c`, `balance.c`. Objective = CUT, `ncon == 1`, iptype = GROW,
//! rtype = FM, dropedges = 0. Multi-constraint and node/edge separator routines
//! are not on this path (see `part/mod.rs`).
//!
//! RNG contract: `SetupCtrl(METIS_OP_PMETIS)` (invoked by
//! `METIS_PartGraphRecursive`) re-seeds the global generator to 4321
//! (`InitRandom(-1)`); the coarsening/bisection/FM draws continue from there, and
//! the k-way `RefineKWay` back in `MlevelKWayPartitioning` continues the *same*
//! stream (it is not re-seeded).

// Index-based loops mirror the C source operation-for-operation; see coarsen.rs.
#![allow(clippy::needless_range_loop)]

use super::coarsen::coarsen_graph;
use super::{Ctrl, WGraph, i2rubfactor};
use crate::pqueue::Rpq;
use crate::rng::Rng;
use crate::{Idx, Real};

/// `SMALLNIPARTS` / `LARGENIPARTS` (`defs.h:45`).
const SMALLNIPARTS: Idx = 5;
const LARGENIPARTS: Idx = 7;

/// `InitKWayPartitioning` (`kmetis.c:175`), objtype CUT: builds a PMETIS ctrl,
/// re-seeds the RNG, and runs `METIS_PartGraphRecursive` on the coarsest k-way
/// graph, writing the result into `cgraph.where_`.
pub(super) fn init_kway_partitioning(ctrl: &Ctrl, rng: &mut Rng, cgraph: &mut WGraph) {
    let nparts = ctrl.nparts;
    let ncon = 1usize;

    // ubvec[i] = (real_t)pow(ctrl->ubfactors[i], 1.0/log(nparts))
    let mut ubvec = vec![0.0f32; ncon];
    for (i, u) in ubvec.iter_mut().enumerate() {
        *u = (ctrl.ubfactors[i] as f64).powf(1.0f64 / (nparts as f64).ln()) as Real;
    }

    // --- SetupCtrl(METIS_OP_PMETIS, options, ncon, nparts, ctrl->tpwgts, ubvec) ---
    // ncon==1 => iptype=GROW, ufactor=PMETIS_DEFAULT_UFACTOR(1), CoarsenTo=20.
    // ncuts = options[NCUTS] = ctrl->nIparts; niter = options[NITER] = ctrl->niter.
    let pufactor = crate::defaults::PMETIS_DEFAULT_UFACTOR;
    // ubfactors[i] = ubvec[i] + 0.0000499 (f32 -> double add -> f32).
    let mut pubfactors = vec![0.0f32; ncon];
    for (i, uf) in pubfactors.iter_mut().enumerate() {
        *uf = (ubvec[i] as f64 + 0.0000499f64) as Real;
    }
    let _ = i2rubfactor(pufactor); // ubfactors seeded from I2RUBFACTOR then overwritten by ubvec

    let mut pctrl = Ctrl {
        nparts,
        ncon: ncon as Idx,
        ncuts: ctrl.n_iparts,
        niter: ctrl.niter,
        n_iparts: -1,
        coarsen_to: 20,
        ufactor: pufactor,
        no2hop: ctrl.no2hop,
        maxvwgt: vec![0; ncon],
        tpwgts: ctrl.tpwgts.clone(), // rcopy(tpwgts) -- the kmetis 1/nparts weights
        ubfactors: pubfactors,
        pijbm: vec![0.0; (nparts * ncon as Idx) as usize],
        cnbrpool: Vec::new(),
        nbrpoolcpos: 0,
    };

    // InitRandom(-1) -> reseed to 4321 (at SetupCtrl, before any consumption).
    *rng = Rng::init_random(-1);

    // --- SetupGraph: build the PMETIS input graph from cgraph's CSR + label ---
    let n = cgraph.nvtxs as usize;
    let mut pg = WGraph::new_empty();
    pg.nvtxs = cgraph.nvtxs;
    pg.nedges = cgraph.nedges;
    pg.ncon = 1;
    pg.xadj = cgraph.xadj.clone();
    pg.adjncy = cgraph.adjncy.clone();
    pg.vwgt = cgraph.vwgt.clone();
    pg.adjwgt = cgraph.adjwgt.clone();
    pg.setup_tvwgt();
    // SetupGraph_label: label[i] = i.
    pg.label = (0..cgraph.nvtxs).collect();

    // iset(nvtxs, 0, part); part = graph->where.
    let mut part = vec![0 as Idx; n];

    // MlevelRecursiveBisection(pctrl, pg, nparts, part, pctrl.tpwgts, 0).
    // tpwgts is scaled in place across the recursion; work on a local copy.
    let mut tpwgts = pctrl.tpwgts.clone();
    mlevel_recursive_bisection(&mut pctrl, rng, pg, nparts, &mut part, &mut tpwgts, 0);

    // Write the result into cgraph.where_ (allocated by AllocateKWayPartitionMemory).
    cgraph.where_ = part;
}

/// `MlevelRecursiveBisection` (`pmetis.c:157`), `ncon == 1`. `tpwgts` is the
/// mutable target-weight window for this subtree (length == `nparts`); it is
/// scaled in place and sub-windowed for the two recursive calls.
fn mlevel_recursive_bisection(
    ctrl: &mut Ctrl,
    rng: &mut Rng,
    graph: WGraph,
    nparts: Idx,
    part: &mut [Idx],
    tpwgts: &mut [Real],
    fpart: Idx,
) -> Idx {
    let nvtxs = graph.nvtxs as usize;
    let half = (nparts >> 1) as usize;

    // tpwgts2[0] = rsum(half, tpwgts, 1); tpwgts2[1] = 1.0 - tpwgts2[0].
    let t0 = super::rsum_strided(half, tpwgts, 0, 1);
    let tpwgts2 = [t0, (1.0f64 - t0 as f64) as Real];

    // Bisect.
    let (mut graph, mut objval) = multilevel_bisect(ctrl, rng, graph, &tpwgts2);

    // Copy the bisection labels out via the label map.
    for i in 0..nvtxs {
        part[graph.label[i] as usize] = graph.where_[i] + fpart;
    }

    // Split into left/right if more than 2 parts remain.
    let (mut lgraph, mut rgraph) = if nparts > 2 {
        let (l, r) = split_graph_part(&graph);
        (Some(l), Some(r))
    } else {
        (None, None)
    };

    // FreeGraph(graph): drop the top-level graph.
    graph.where_.clear();
    drop(graph);

    // Scale the tpwgts windows by the true weights.
    // wsum = rsum(half, tpwgts, 1)
    let wsum = super::rsum_strided(half, tpwgts, 0, 1);
    let a_left = (1.0f64 / wsum as f64) as Real;
    super::rscale_strided(half, a_left, tpwgts, 0, 1);
    let a_right = (1.0f64 / (1.0f64 - wsum as f64)) as Real;
    super::rscale_strided((nparts - nparts / 2) as usize, a_right, tpwgts, half, 1);

    if nparts > 3 {
        let (left_tp, right_tp) = tpwgts.split_at_mut(half);
        objval += mlevel_recursive_bisection(
            ctrl,
            rng,
            lgraph.take().unwrap(),
            nparts >> 1,
            part,
            left_tp,
            fpart,
        );
        objval += mlevel_recursive_bisection(
            ctrl,
            rng,
            rgraph.take().unwrap(),
            nparts - (nparts >> 1),
            part,
            right_tp,
            fpart + (nparts >> 1),
        );
    } else if nparts == 3 {
        // FreeGraph(lgraph); recurse only on rgraph.
        lgraph = None;
        let (_left_tp, right_tp) = tpwgts.split_at_mut(half);
        objval += mlevel_recursive_bisection(
            ctrl,
            rng,
            rgraph.take().unwrap(),
            nparts - (nparts >> 1),
            part,
            right_tp,
            fpart + (nparts >> 1),
        );
    }
    let _ = (lgraph, rgraph);

    objval
}

/// `MultilevelBisect` (`pmetis.c:224`), `ncon == 1`, `ncuts == nIparts`. Returns
/// the refined top graph and its edgecut.
fn multilevel_bisect(
    ctrl: &mut Ctrl,
    rng: &mut Rng,
    mut graph: WGraph,
    tpwgts2: &[Real],
) -> (WGraph, Idx) {
    setup_2way_bal_multipliers(ctrl, &graph, tpwgts2);

    let mut bestwhere: Option<Vec<Idx>> = None;
    let mut bestobj: Idx = 0;
    let mut bestbal: Real = 0.0;
    let mut curobj: Idx = 0;

    for i in 0..ctrl.ncuts {
        // CoarsenGraph consumes graph and returns the finest..coarsest chain.
        let mut levels = coarsen_graph(ctrl, rng, graph);
        let last = levels.len() - 1;

        let niparts = if levels[last].nvtxs <= ctrl.coarsen_to {
            SMALLNIPARTS
        } else {
            LARGENIPARTS
        };
        init_2way_partition(ctrl, rng, &mut levels[last], tpwgts2, niparts);

        refine_2way(ctrl, rng, &mut levels, tpwgts2);

        graph = levels.swap_remove(0);

        curobj = graph.mincut;
        let curbal = compute_load_imbalance_diff2(&graph, ctrl);

        if i == 0
            || (curbal <= 0.0005 && bestobj > curobj)
            || (bestbal > 0.0005 && curbal < bestbal)
        {
            bestobj = curobj;
            bestbal = curbal;
            if i < ctrl.ncuts - 1 {
                bestwhere = Some(graph.where_.clone());
            }
        }

        if bestobj == 0 {
            break;
        }
        if i < ctrl.ncuts - 1 {
            free_rdata(&mut graph);
        }
    }

    if bestobj != curobj {
        graph.where_ = bestwhere.unwrap();
        compute_2way_partition_params(&mut graph);
    }

    (graph, bestobj)
}

/// `Setup2WayBalMultipliers` (`options.c:159`): fills `pijbm[0..2]`.
fn setup_2way_bal_multipliers(ctrl: &mut Ctrl, graph: &WGraph, tpwgts: &[Real]) {
    for i in 0..2usize {
        ctrl.pijbm[i] = graph.invtvwgt[0] / tpwgts[i];
    }
}

/// `ComputeLoadImbalanceDiff(graph, 2, pijbm, ubfactors)` (`mcutil.c:256`),
/// `ncon == 1`.
fn compute_load_imbalance_diff2(graph: &WGraph, ctrl: &Ctrl) -> Real {
    let mut max: Real = -1.0;
    for j in 0..2usize {
        let cur = graph.pwgts[j] as Real * ctrl.pijbm[j] - ctrl.ubfactors[0];
        if cur > max {
            max = cur;
        }
    }
    max
}

/// `FreeRData` (`graph.c:249`): free the partition/refinement arrays but keep the
/// CSR, cmap, label, tvwgt.
fn free_rdata(graph: &mut WGraph) {
    graph.where_ = Vec::new();
    graph.pwgts = Vec::new();
    graph.id = Vec::new();
    graph.ed = Vec::new();
    graph.bndptr = Vec::new();
    graph.bndind = Vec::new();
    graph.ckrinfo = Vec::new();
}

/// `Allocate2WayPartitionMemory` (`refine.c:54`), `ncon == 1`.
fn allocate_2way_partition_memory(graph: &mut WGraph) {
    let nvtxs = graph.nvtxs as usize;
    graph.pwgts = vec![0; 2];
    graph.where_ = vec![0; nvtxs];
    graph.bndptr = vec![0; nvtxs];
    graph.bndind = vec![0; nvtxs];
    graph.id = vec![0; nvtxs];
    graph.ed = vec![0; nvtxs];
}

/// `Compute2WayPartitionParams` (`refine.c:73`), `ncon == 1`.
fn compute_2way_partition_params(graph: &mut WGraph) {
    let nvtxs = graph.nvtxs as usize;
    // Ensure the refinement arrays exist (they do in the normal flow; the
    // restore-bestwhere path in MultilevelBisect reuses last-cut allocations).
    if graph.pwgts.len() != 2 {
        graph.pwgts = vec![0; 2];
    }
    if graph.bndptr.len() != nvtxs {
        graph.bndptr = vec![-1; nvtxs];
    }
    if graph.bndind.len() != nvtxs {
        graph.bndind = vec![0; nvtxs];
    }
    if graph.id.len() != nvtxs {
        graph.id = vec![0; nvtxs];
    }
    if graph.ed.len() != nvtxs {
        graph.ed = vec![0; nvtxs];
    }

    let WGraph {
        xadj,
        vwgt,
        adjncy,
        adjwgt,
        where_,
        pwgts,
        bndind,
        bndptr,
        id,
        ed,
        ..
    } = graph;

    pwgts[0] = 0;
    pwgts[1] = 0;
    for b in bndptr.iter_mut() {
        *b = -1;
    }

    for i in 0..nvtxs {
        pwgts[where_[i] as usize] += vwgt[i];
    }

    let mut nbnd: Idx = 0;
    let mut mincut: Idx = 0;
    for i in 0..nvtxs {
        let (istart, iend) = (xadj[i] as usize, xadj[i + 1] as usize);
        let me = where_[i];
        let mut tid: Idx = 0;
        let mut ted: Idx = 0;
        for j in istart..iend {
            if me == where_[adjncy[j] as usize] {
                tid += adjwgt[j];
            } else {
                ted += adjwgt[j];
            }
        }
        id[i] = tid;
        ed[i] = ted;
        if ted > 0 || istart == iend {
            // BNDInsert
            bndind[nbnd as usize] = i as Idx;
            bndptr[i] = nbnd;
            nbnd += 1;
            mincut += ted;
        }
    }
    graph.mincut = mincut / 2;
    graph.nbnd = nbnd;
}

/// `Init2WayPartition` (`initpart.c:19`), `ncon == 1`, iptype = GROW.
fn init_2way_partition(
    ctrl: &Ctrl,
    rng: &mut Rng,
    graph: &mut WGraph,
    ntpwgts: &[Real],
    niparts: Idx,
) {
    if graph.nedges == 0 {
        random_bisection(ctrl, rng, graph, ntpwgts, niparts);
    } else {
        grow_bisection(ctrl, rng, graph, ntpwgts, niparts);
    }
}

/// `RandomBisection` (`initpart.c:114`), `ncon == 1`.
fn random_bisection(
    ctrl: &Ctrl,
    rng: &mut Rng,
    graph: &mut WGraph,
    ntpwgts: &[Real],
    niparts: Idx,
) {
    let nvtxs = graph.nvtxs as usize;
    allocate_2way_partition_memory(graph);
    let mut bestwhere = vec![0 as Idx; nvtxs];
    let mut perm = vec![0 as Idx; nvtxs];

    // zeromaxpwgt = ubfactors[0]*tvwgt[0]*ntpwgts[0]  (f32, truncate)
    let zeromaxpwgt = (ctrl.ubfactors[0] * graph.tvwgt[0] as Real * ntpwgts[0]) as Idx;

    let mut bestcut: Idx = 0;
    for inbfs in 0..niparts {
        for w in graph.where_.iter_mut() {
            *w = 1;
        }
        if inbfs > 0 {
            rng.rand_array_permute(nvtxs as Idx, &mut perm, (nvtxs / 2) as Idx, 1);
            let mut p1 = graph.tvwgt[0];
            let mut p0: Idx = 0;
            for ii in 0..nvtxs {
                let i = perm[ii] as usize;
                if p0 + graph.vwgt[i] < zeromaxpwgt {
                    graph.where_[i] = 0;
                    p0 += graph.vwgt[i];
                    p1 -= graph.vwgt[i];
                    if p0 > zeromaxpwgt {
                        break;
                    }
                }
            }
            let _ = p1;
        }

        compute_2way_partition_params(graph);
        balance_2way(ctrl, rng, graph, ntpwgts);
        fm_2way_cut_refine(ctrl, rng, graph, ntpwgts, 4);

        if inbfs == 0 || bestcut > graph.mincut {
            bestcut = graph.mincut;
            bestwhere.copy_from_slice(&graph.where_[..nvtxs]);
            if bestcut == 0 {
                break;
            }
        }
    }
    graph.mincut = bestcut;
    graph.where_.copy_from_slice(&bestwhere[..nvtxs]);
}

/// `GrowBisection` (`initpart.c:189`), `ncon == 1`.
fn grow_bisection(ctrl: &Ctrl, rng: &mut Rng, graph: &mut WGraph, ntpwgts: &[Real], niparts: Idx) {
    let nvtxs = graph.nvtxs as usize;
    allocate_2way_partition_memory(graph);
    let mut bestwhere = vec![0 as Idx; nvtxs];
    let mut queue = vec![0 as Idx; nvtxs];
    let mut touched = vec![0 as Idx; nvtxs];

    let tvwgt0 = graph.tvwgt[0];
    // onemaxpwgt = ubfactors[0]*tvwgt0*ntpwgts[1] (f32); oneminpwgt =
    // (1.0/ubfactors[0])*tvwgt0*ntpwgts[1] (double).
    let onemaxpwgt = (ctrl.ubfactors[0] * tvwgt0 as Real * ntpwgts[1]) as Idx;
    let oneminpwgt =
        ((1.0f64 / ctrl.ubfactors[0] as f64) * tvwgt0 as f64 * ntpwgts[1] as f64) as Idx;

    let mut bestcut: Idx = 0;
    for inbfs in 0..niparts {
        for w in graph.where_.iter_mut() {
            *w = 1;
        }
        for t in touched.iter_mut() {
            *t = 0;
        }

        let mut pwgts1 = tvwgt0;
        let mut pwgts0: Idx = 0;

        queue[0] = rng.rand_in_range(nvtxs as Idx);
        touched[queue[0] as usize] = 1;
        let mut first: usize = 0;
        let mut last: usize = 1;
        let mut nleft = nvtxs - 1;
        let mut drain = false;

        loop {
            if first == last {
                // Disconnected: restart from an untouched vertex.
                if nleft == 0 || drain {
                    break;
                }
                let mut k = rng.rand_in_range(nleft as Idx);
                let mut sel = 0usize;
                for i in 0..nvtxs {
                    if touched[i] == 0 {
                        if k == 0 {
                            sel = i;
                            break;
                        } else {
                            k -= 1;
                        }
                    }
                }
                queue[0] = sel as Idx;
                touched[sel] = 1;
                first = 0;
                last = 1;
                nleft -= 1;
            }

            let iv = queue[first] as usize;
            first += 1;
            if pwgts0 > 0 && pwgts1 - graph.vwgt[iv] < oneminpwgt {
                drain = true;
                continue;
            }

            graph.where_[iv] = 0;
            pwgts0 += graph.vwgt[iv];
            pwgts1 -= graph.vwgt[iv];
            if pwgts1 <= onemaxpwgt {
                break;
            }

            drain = false;
            for j in graph.xadj[iv] as usize..graph.xadj[iv + 1] as usize {
                let k = graph.adjncy[j] as usize;
                if touched[k] == 0 {
                    queue[last] = k as Idx;
                    last += 1;
                    touched[k] = 1;
                    nleft -= 1;
                }
            }
        }

        // Fix degenerate empty-partition cases (RNG-consuming).
        if pwgts1 == 0 {
            let r = rng.rand_in_range(nvtxs as Idx) as usize;
            graph.where_[r] = 1;
        }
        if pwgts0 == 0 {
            let r = rng.rand_in_range(nvtxs as Idx) as usize;
            graph.where_[r] = 0;
        }

        compute_2way_partition_params(graph);
        balance_2way(ctrl, rng, graph, ntpwgts);
        fm_2way_cut_refine(ctrl, rng, graph, ntpwgts, ctrl.niter);

        if inbfs == 0 || bestcut > graph.mincut {
            bestcut = graph.mincut;
            bestwhere.copy_from_slice(&graph.where_[..nvtxs]);
            if bestcut == 0 {
                break;
            }
        }
    }
    graph.mincut = bestcut;
    graph.where_.copy_from_slice(&bestwhere[..nvtxs]);
}

/// `Refine2Way` (`refine.c:17`): uncoarsen the chain, balancing + FM at each
/// level. `levels[last]` is the coarsest; `levels[0]` the original graph.
fn refine_2way(ctrl: &mut Ctrl, rng: &mut Rng, levels: &mut [WGraph], tpwgts: &[Real]) {
    {
        let last = levels.len() - 1;
        compute_2way_partition_params(&mut levels[last]);
    }

    let mut idx = levels.len() - 1;
    loop {
        balance_2way(ctrl, rng, &mut levels[idx], tpwgts);
        let niter = ctrl.niter;
        fm_2way_cut_refine(ctrl, rng, &mut levels[idx], tpwgts, niter);

        if idx == 0 {
            break;
        }
        idx -= 1;
        project_2way_partition(levels, idx);
    }
}

/// `Project2WayPartition` (`refine.c:143`), `ncon == 1`, `dropedges == 0`.
/// `levels[idx]` is the finer graph, `levels[idx+1]` the coarser source.
fn project_2way_partition(levels: &mut [WGraph], idx: usize) {
    let (finer_slice, coarser_slice) = levels.split_at_mut(idx + 1);
    let graph = &mut finer_slice[idx];
    let cgraph = &coarser_slice[0];

    let nvtxs = graph.nvtxs as usize;
    allocate_2way_partition_memory(graph);

    // Project partition and mark which came from the coarse boundary.
    for i in 0..nvtxs {
        let j = graph.cmap[i] as usize;
        graph.where_[i] = cgraph.where_[j];
        graph.cmap[i] = cgraph.bndptr[j]; // dropedges==0
    }

    let WGraph {
        xadj,
        adjncy,
        adjwgt,
        where_,
        cmap,
        id,
        ed,
        bndind,
        bndptr,
        ..
    } = graph;

    for b in bndptr.iter_mut() {
        *b = -1;
    }

    let mut nbnd: Idx = 0;
    for i in 0..nvtxs {
        let (istart, iend) = (xadj[i] as usize, xadj[i + 1] as usize);
        let mut tid: Idx = 0;
        let mut ted: Idx = 0;
        if cmap[i] == -1 {
            // Interior node.
            for j in istart..iend {
                tid += adjwgt[j];
            }
        } else {
            let me = where_[i];
            for j in istart..iend {
                if me == where_[adjncy[j] as usize] {
                    tid += adjwgt[j];
                } else {
                    ted += adjwgt[j];
                }
            }
        }
        id[i] = tid;
        ed[i] = ted;
        if ted > 0 || istart == iend {
            bndind[nbnd as usize] = i as Idx;
            bndptr[i] = nbnd;
            nbnd += 1;
        }
    }

    graph.mincut = cgraph.mincut; // dropedges==0
    graph.nbnd = nbnd;
    graph.pwgts = vec![cgraph.pwgts[0], cgraph.pwgts[1]];
}

/// `Balance2Way` (`balance.c:16`), `ncon == 1`.
fn balance_2way(ctrl: &Ctrl, rng: &mut Rng, graph: &mut WGraph, ntpwgts: &[Real]) {
    if compute_load_imbalance_diff2(graph, ctrl) <= 0.0 {
        return;
    }
    // return right away if the balance is OK
    // rabs(ntpwgts[0]*tvwgt0 - pwgts[0]) < 3*tvwgt0/nvtxs
    let lhs = (ntpwgts[0] * graph.tvwgt[0] as Real - graph.pwgts[0] as Real).abs();
    let rhs = (3 * graph.tvwgt[0] / graph.nvtxs) as Real;
    if lhs < rhs {
        return;
    }
    if graph.nbnd > 0 {
        bnd_2way_balance(ctrl, rng, graph, ntpwgts);
    } else {
        general_2way_balance(ctrl, rng, graph, ntpwgts);
    }
}

/// `Bnd2WayBalance` (`balance.c:41`), `ncon == 1`.
fn bnd_2way_balance(ctrl: &Ctrl, rng: &mut Rng, graph: &mut WGraph, ntpwgts: &[Real]) {
    let _ = ctrl;
    let nvtxs = graph.nvtxs as usize;
    let mut moved = vec![-1 as Idx; nvtxs];
    let mut perm = vec![0 as Idx; nvtxs];

    let tpwgts0 = (graph.tvwgt[0] as Real * ntpwgts[0]) as Idx;
    let tpwgts = [tpwgts0, graph.tvwgt[0] - tpwgts0];
    let mindiff = (tpwgts[0] - graph.pwgts[0]).abs();
    let from = if graph.pwgts[0] < tpwgts[0] { 1 } else { 0 };
    let to = (from + 1) % 2;

    let mut queue = Rpq::create(nvtxs);

    let nbnd = graph.nbnd;
    rng.rand_array_permute(nbnd, &mut perm, nbnd / 5, 1);
    for ii in 0..nbnd as usize {
        let bi = graph.bndind[perm[ii] as usize] as usize;
        if graph.where_[bi] == from && graph.vwgt[bi] <= mindiff {
            queue.insert(bi as Idx, (graph.ed[bi] - graph.id[bi]) as Real);
        }
    }

    let mut mincut = graph.mincut;
    let mut nbnd = graph.nbnd;
    for _nswaps in 0..nvtxs {
        let higain = queue.get_top();
        if higain == -1 {
            break;
        }
        let higain = higain as usize;
        if graph.pwgts[to as usize] + graph.vwgt[higain] > tpwgts[to as usize] {
            break;
        }

        mincut -= graph.ed[higain] - graph.id[higain];
        graph.pwgts[to as usize] += graph.vwgt[higain];
        graph.pwgts[from as usize] -= graph.vwgt[higain];

        graph.where_[higain] = to;
        moved[higain] = 0; // marked as moved (value only tested for == -1)

        // SWAP(id, ed)
        std::mem::swap(&mut graph.id[higain], &mut graph.ed[higain]);

        if graph.ed[higain] == 0 && graph.xadj[higain] < graph.xadj[higain + 1] {
            bnd_delete(
                &mut nbnd,
                &mut graph.bndind,
                &mut graph.bndptr,
                higain as Idx,
            );
        }

        for j in graph.xadj[higain] as usize..graph.xadj[higain + 1] as usize {
            let k = graph.adjncy[j] as usize;
            let kwgt = if to == graph.where_[k] {
                graph.adjwgt[j]
            } else {
                -graph.adjwgt[j]
            };
            graph.id[k] += kwgt;
            graph.ed[k] -= kwgt;

            if graph.bndptr[k] != -1 {
                if graph.ed[k] == 0 {
                    bnd_delete(&mut nbnd, &mut graph.bndind, &mut graph.bndptr, k as Idx);
                    if moved[k] == -1 && graph.where_[k] == from && graph.vwgt[k] <= mindiff {
                        queue.delete(k as Idx);
                    }
                } else if moved[k] == -1 && graph.where_[k] == from && graph.vwgt[k] <= mindiff {
                    queue.update(k as Idx, (graph.ed[k] - graph.id[k]) as Real);
                }
            } else if graph.ed[k] > 0 {
                bnd_insert(&mut nbnd, &mut graph.bndind, &mut graph.bndptr, k as Idx);
                if moved[k] == -1 && graph.where_[k] == from && graph.vwgt[k] <= mindiff {
                    queue.insert(k as Idx, (graph.ed[k] - graph.id[k]) as Real);
                }
            }
        }
    }

    graph.mincut = mincut;
    graph.nbnd = nbnd;
}

/// `General2WayBalance` (`balance.c:169`), `ncon == 1`.
fn general_2way_balance(ctrl: &Ctrl, rng: &mut Rng, graph: &mut WGraph, ntpwgts: &[Real]) {
    let _ = ctrl;
    let nvtxs = graph.nvtxs as usize;
    let mut moved = vec![-1 as Idx; nvtxs];
    let mut perm = vec![0 as Idx; nvtxs];

    let tpwgts0 = (graph.tvwgt[0] as Real * ntpwgts[0]) as Idx;
    let tpwgts = [tpwgts0, graph.tvwgt[0] - tpwgts0];
    let mindiff = (tpwgts[0] - graph.pwgts[0]).abs();
    let from = if graph.pwgts[0] < tpwgts[0] { 1 } else { 0 };
    let to = (from + 1) % 2;

    let mut queue = Rpq::create(nvtxs);

    rng.rand_array_permute(nvtxs as Idx, &mut perm, (nvtxs / 5) as Idx, 1);
    for ii in 0..nvtxs {
        let i = perm[ii] as usize;
        if graph.where_[i] == from && graph.vwgt[i] <= mindiff {
            queue.insert(i as Idx, (graph.ed[i] - graph.id[i]) as Real);
        }
    }

    let mut mincut = graph.mincut;
    let mut nbnd = graph.nbnd;
    for _nswaps in 0..nvtxs {
        let higain = queue.get_top();
        if higain == -1 {
            break;
        }
        let higain = higain as usize;
        if graph.pwgts[to as usize] + graph.vwgt[higain] > tpwgts[to as usize] {
            break;
        }

        mincut -= graph.ed[higain] - graph.id[higain];
        graph.pwgts[to as usize] += graph.vwgt[higain];
        graph.pwgts[from as usize] -= graph.vwgt[higain];

        graph.where_[higain] = to;
        moved[higain] = 0;

        std::mem::swap(&mut graph.id[higain], &mut graph.ed[higain]);

        if graph.ed[higain] == 0
            && graph.bndptr[higain] != -1
            && graph.xadj[higain] < graph.xadj[higain + 1]
        {
            bnd_delete(
                &mut nbnd,
                &mut graph.bndind,
                &mut graph.bndptr,
                higain as Idx,
            );
        }
        if graph.ed[higain] > 0 && graph.bndptr[higain] == -1 {
            bnd_insert(
                &mut nbnd,
                &mut graph.bndind,
                &mut graph.bndptr,
                higain as Idx,
            );
        }

        for j in graph.xadj[higain] as usize..graph.xadj[higain + 1] as usize {
            let k = graph.adjncy[j] as usize;
            let kwgt = if to == graph.where_[k] {
                graph.adjwgt[j]
            } else {
                -graph.adjwgt[j]
            };
            graph.id[k] += kwgt;
            graph.ed[k] -= kwgt;

            if moved[k] == -1 && graph.where_[k] == from && graph.vwgt[k] <= mindiff {
                queue.update(k as Idx, (graph.ed[k] - graph.id[k]) as Real);
            }

            if graph.ed[k] == 0 && graph.bndptr[k] != -1 {
                bnd_delete(&mut nbnd, &mut graph.bndind, &mut graph.bndptr, k as Idx);
            } else if graph.ed[k] > 0 && graph.bndptr[k] == -1 {
                bnd_insert(&mut nbnd, &mut graph.bndind, &mut graph.bndptr, k as Idx);
            }
        }
    }

    graph.mincut = mincut;
    graph.nbnd = nbnd;
}

/// `FM_2WayCutRefine` (`fm.c:29`), `ncon == 1`. The `newcut += (ed-id)` on the
/// limit-undo path restores `newcut` to stay faithful to the C even though the
/// value is not read again before the pass ends (hence `allow`).
#[allow(unused_assignments)]
fn fm_2way_cut_refine(
    ctrl: &Ctrl,
    rng: &mut Rng,
    graph: &mut WGraph,
    ntpwgts: &[Real],
    niter: Idx,
) {
    let _ = ctrl;
    let nvtxs = graph.nvtxs as usize;
    let mut moved = vec![-1 as Idx; nvtxs];
    let mut swaps = vec![0 as Idx; nvtxs];
    let mut perm = vec![0 as Idx; nvtxs];

    let tpwgts0 = (graph.tvwgt[0] as Real * ntpwgts[0]) as Idx;
    let tpwgts = [tpwgts0, graph.tvwgt[0] - tpwgts0];

    // limit = min(max(0.01*nvtxs, 15), 100)  (double, truncate)
    let limit = {
        let a = gk_max_f64(0.01f64 * nvtxs as f64, 15.0);
        gk_min_f64(a, 100.0) as Idx
    };
    // avgvwgt = min((pwgts0+pwgts1)/20, 2*(pwgts0+pwgts1)/nvtxs)  (int)
    let sumw = graph.pwgts[0] + graph.pwgts[1];
    let avgvwgt = gk_min_i(sumw / 20, 2 * sumw / graph.nvtxs);

    let mut q0 = Rpq::create(nvtxs);
    let mut q1 = Rpq::create(nvtxs);

    let origdiff = (tpwgts[0] - graph.pwgts[0]).abs();

    for _pass in 0..niter {
        q0.reset();
        q1.reset();

        let mut mincutorder: Idx = -1;
        let initcut = graph.mincut;
        let mut mincut = graph.mincut;
        let mut newcut = graph.mincut;
        let mut mindiff = (tpwgts[0] - graph.pwgts[0]).abs();

        let mut nbnd = graph.nbnd;
        rng.rand_array_permute(nbnd, &mut perm, nbnd, 1);
        for ii in 0..nbnd as usize {
            let bi = graph.bndind[perm[ii] as usize] as usize;
            let key = (graph.ed[bi] - graph.id[bi]) as Real;
            if graph.where_[bi] == 0 {
                q0.insert(bi as Idx, key);
            } else {
                q1.insert(bi as Idx, key);
            }
        }

        let mut nswaps: usize = 0;
        while nswaps < nvtxs {
            let from = if tpwgts[0] - graph.pwgts[0] < tpwgts[1] - graph.pwgts[1] {
                0
            } else {
                1
            };
            let to = (from + 1) % 2;

            let higain = if from == 0 {
                q0.get_top()
            } else {
                q1.get_top()
            };
            if higain == -1 {
                break;
            }
            let higain = higain as usize;

            newcut -= graph.ed[higain] - graph.id[higain];
            graph.pwgts[to as usize] += graph.vwgt[higain];
            graph.pwgts[from as usize] -= graph.vwgt[higain];

            let diff = (tpwgts[0] - graph.pwgts[0]).abs();
            if (newcut < mincut && diff <= origdiff + avgvwgt)
                || (newcut == mincut && diff < mindiff)
            {
                mincut = newcut;
                mindiff = diff;
                mincutorder = nswaps as Idx;
            } else if (nswaps as Idx) - mincutorder > limit {
                // Undo the last move and stop.
                newcut += graph.ed[higain] - graph.id[higain];
                graph.pwgts[from as usize] += graph.vwgt[higain];
                graph.pwgts[to as usize] -= graph.vwgt[higain];
                break;
            }

            graph.where_[higain] = to;
            moved[higain] = nswaps as Idx;
            swaps[nswaps] = higain as Idx;

            // SWAP(id, ed)
            std::mem::swap(&mut graph.id[higain], &mut graph.ed[higain]);
            if graph.ed[higain] == 0 && graph.xadj[higain] < graph.xadj[higain + 1] {
                bnd_delete(
                    &mut nbnd,
                    &mut graph.bndind,
                    &mut graph.bndptr,
                    higain as Idx,
                );
            }

            for j in graph.xadj[higain] as usize..graph.xadj[higain + 1] as usize {
                let k = graph.adjncy[j] as usize;
                let kwgt = if to == graph.where_[k] {
                    graph.adjwgt[j]
                } else {
                    -graph.adjwgt[j]
                };
                graph.id[k] += kwgt;
                graph.ed[k] -= kwgt;

                if graph.bndptr[k] != -1 {
                    if graph.ed[k] == 0 {
                        bnd_delete(&mut nbnd, &mut graph.bndind, &mut graph.bndptr, k as Idx);
                        if moved[k] == -1 {
                            if graph.where_[k] == 0 {
                                q0.delete(k as Idx);
                            } else {
                                q1.delete(k as Idx);
                            }
                        }
                    } else if moved[k] == -1 {
                        let key = (graph.ed[k] - graph.id[k]) as Real;
                        if graph.where_[k] == 0 {
                            q0.update(k as Idx, key);
                        } else {
                            q1.update(k as Idx, key);
                        }
                    }
                } else if graph.ed[k] > 0 {
                    bnd_insert(&mut nbnd, &mut graph.bndind, &mut graph.bndptr, k as Idx);
                    if moved[k] == -1 {
                        let key = (graph.ed[k] - graph.id[k]) as Real;
                        if graph.where_[k] == 0 {
                            q0.insert(k as Idx, key);
                        } else {
                            q1.insert(k as Idx, key);
                        }
                    }
                }
            }

            nswaps += 1;
        }

        // Roll back to the best cut.
        for i in 0..nswaps {
            moved[swaps[i] as usize] = -1;
        }
        let mut ns = nswaps as Idx - 1;
        while ns > mincutorder {
            let higain = swaps[ns as usize] as usize;

            let newp = (graph.where_[higain] + 1) % 2;
            graph.where_[higain] = newp;
            let to = newp;
            std::mem::swap(&mut graph.id[higain], &mut graph.ed[higain]);
            if graph.ed[higain] == 0
                && graph.bndptr[higain] != -1
                && graph.xadj[higain] < graph.xadj[higain + 1]
            {
                bnd_delete(
                    &mut nbnd,
                    &mut graph.bndind,
                    &mut graph.bndptr,
                    higain as Idx,
                );
            } else if graph.ed[higain] > 0 && graph.bndptr[higain] == -1 {
                bnd_insert(
                    &mut nbnd,
                    &mut graph.bndind,
                    &mut graph.bndptr,
                    higain as Idx,
                );
            }

            graph.pwgts[to as usize] += graph.vwgt[higain];
            graph.pwgts[((to + 1) % 2) as usize] -= graph.vwgt[higain];
            for j in graph.xadj[higain] as usize..graph.xadj[higain + 1] as usize {
                let k = graph.adjncy[j] as usize;
                let kwgt = if to == graph.where_[k] {
                    graph.adjwgt[j]
                } else {
                    -graph.adjwgt[j]
                };
                graph.id[k] += kwgt;
                graph.ed[k] -= kwgt;
                if graph.bndptr[k] != -1 && graph.ed[k] == 0 {
                    bnd_delete(&mut nbnd, &mut graph.bndind, &mut graph.bndptr, k as Idx);
                }
                if graph.bndptr[k] == -1 && graph.ed[k] > 0 {
                    bnd_insert(&mut nbnd, &mut graph.bndind, &mut graph.bndptr, k as Idx);
                }
            }
            ns -= 1;
        }

        graph.mincut = mincut;
        graph.nbnd = nbnd;

        if mincutorder <= 0 || mincut == initcut {
            break;
        }
    }
}

/// `SplitGraphPart` (`pmetis.c:278`), `ncon == 1`.
fn split_graph_part(graph: &WGraph) -> (WGraph, WGraph) {
    let nvtxs = graph.nvtxs as usize;
    let xadj = &graph.xadj;
    let vwgt = &graph.vwgt;
    let adjncy = &graph.adjncy;
    let adjwgt = &graph.adjwgt;
    let label = &graph.label;
    let where_ = &graph.where_;
    let bndptr = &graph.bndptr;

    let mut rename = vec![0 as Idx; nvtxs];
    let mut snvtxs = [0usize, 0usize];
    let mut snedges = [0usize, 0usize];
    for i in 0..nvtxs {
        let k = where_[i] as usize;
        rename[i] = snvtxs[k] as Idx;
        snvtxs[k] += 1;
        snedges[k] += (xadj[i + 1] - xadj[i]) as usize;
    }

    let mut sxadj = [vec![0 as Idx; snvtxs[0] + 1], vec![0 as Idx; snvtxs[1] + 1]];
    let mut svwgt = [vec![0 as Idx; snvtxs[0]], vec![0 as Idx; snvtxs[1]]];
    let mut sadjncy = [vec![0 as Idx; snedges[0]], vec![0 as Idx; snedges[1]]];
    let mut sadjwgt = [vec![0 as Idx; snedges[0]], vec![0 as Idx; snedges[1]]];
    let mut slabel = [vec![0 as Idx; snvtxs[0]], vec![0 as Idx; snvtxs[1]]];

    let mut snv = [0usize, 0usize];
    let mut sne = [0usize, 0usize];
    for i in 0..nvtxs {
        let mypart = where_[i] as usize;
        let (istart, iend) = (xadj[i] as usize, xadj[i + 1] as usize);

        if bndptr[i] == -1 {
            // Interior vertex: copy all edges.
            let base = sne[mypart];
            for j in istart..iend {
                sadjncy[mypart][base + (j - istart)] = rename[adjncy[j] as usize];
                sadjwgt[mypart][base + (j - istart)] = adjwgt[j];
            }
            sne[mypart] += iend - istart;
        } else {
            // Boundary vertex: copy only same-partition edges.
            let mut l = sne[mypart];
            for j in istart..iend {
                let k = adjncy[j] as usize;
                if where_[k] as usize == mypart {
                    sadjncy[mypart][l] = rename[k];
                    sadjwgt[mypart][l] = adjwgt[j];
                    l += 1;
                }
            }
            sne[mypart] = l;
        }

        svwgt[mypart][snv[mypart]] = vwgt[i];
        slabel[mypart][snv[mypart]] = label[i];
        snv[mypart] += 1;
        sxadj[mypart][snv[mypart]] = sne[mypart] as Idx;
    }

    let build = |idx: usize,
                 sxadj: Vec<Idx>,
                 svwgt: Vec<Idx>,
                 mut sadjncy: Vec<Idx>,
                 mut sadjwgt: Vec<Idx>,
                 slabel: Vec<Idx>|
     -> WGraph {
        let mut g = WGraph::new_empty();
        g.nvtxs = snvtxs[idx] as Idx;
        g.nedges = sne[idx] as Idx;
        g.ncon = 1;
        sadjncy.truncate(sne[idx]);
        sadjwgt.truncate(sne[idx]);
        g.xadj = sxadj;
        g.vwgt = svwgt;
        g.adjncy = sadjncy;
        g.adjwgt = sadjwgt;
        g.label = slabel;
        g.setup_tvwgt();
        g
    };

    let [sxadj0, sxadj1] = sxadj;
    let [svwgt0, svwgt1] = svwgt;
    let [sadjncy0, sadjncy1] = sadjncy;
    let [sadjwgt0, sadjwgt1] = sadjwgt;
    let [slabel0, slabel1] = slabel;

    let lgraph = build(0, sxadj0, svwgt0, sadjncy0, sadjwgt0, slabel0);
    let rgraph = build(1, sxadj1, svwgt1, sadjncy1, sadjwgt1, slabel1);
    (lgraph, rgraph)
}

//======================= small helpers =======================

/// `BNDInsert`/`ListInsert` (`macros.h:46`).
#[inline]
fn bnd_insert(nbnd: &mut Idx, bndind: &mut [Idx], bndptr: &mut [Idx], v: Idx) {
    bndind[*nbnd as usize] = v;
    bndptr[v as usize] = *nbnd;
    *nbnd += 1;
}

/// `BNDDelete`/`ListDelete` (`macros.h:53`).
#[inline]
fn bnd_delete(nbnd: &mut Idx, bndind: &mut [Idx], bndptr: &mut [Idx], v: Idx) {
    *nbnd -= 1;
    let last = bndind[*nbnd as usize];
    bndind[bndptr[v as usize] as usize] = last;
    bndptr[last as usize] = bndptr[v as usize];
    bndptr[v as usize] = -1;
}

/// `gk_max(a, b)` for `f64` (`gk_macros.h:16`): `a >= b ? a : b`.
#[inline]
fn gk_max_f64(a: f64, b: f64) -> f64 {
    if a >= b { a } else { b }
}

/// `gk_min(a, b)` for `f64` (`gk_macros.h:17`): `a >= b ? b : a`.
#[inline]
fn gk_min_f64(a: f64, b: f64) -> f64 {
    if a >= b { b } else { a }
}

/// `gk_min(a, b)` for `idx_t`.
#[inline]
fn gk_min_i(a: Idx, b: Idx) -> Idx {
    if a >= b { b } else { a }
}
