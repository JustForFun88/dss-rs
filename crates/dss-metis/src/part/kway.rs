//! Multilevel k-way partitioning driver + greedy k-way cut refinement, ported
//! 1:1 from `libmetis/kmetis.c`, `kwayrefine.c`, `kwayfm.c`, and the
//! `mcutil.c::ComputeLoadImbalance*` helpers. Objective = CUT, `ncon == 1`,
//! `minconn == contig == dropedges == 0` (see `part/mod.rs` reachability notes).

// Index-based loops mirror the C source operation-for-operation; see coarsen.rs.
#![allow(clippy::needless_range_loop)]

use super::{CkrInfo, Cnbr, Ctrl, WGraph};
use crate::pqueue::Rpq;
use crate::rng::Rng;
use crate::{Idx, Real};

const BNDTYPE_REFINE: i32 = 1;
const BNDTYPE_BALANCE: i32 = 2;
const OMODE_REFINE: i32 = 1;
const OMODE_BALANCE: i32 = 2;
const VPQSTATUS_PRESENT: i32 = 1;
const VPQSTATUS_EXTRACTED: i32 = 2;
const VPQSTATUS_NOTPRESENT: i32 = 3;

/// `ComputeLoadImbalance` (`mcutil.c:228`), `ncon == 1`: `max(1.0, max_j
/// pwgts[j]*pijbm[j])` in `real_t`.
#[allow(non_snake_case)]
pub(super) fn ComputeLoadImbalance_(graph: &WGraph, nparts: Idx, pijbm: &[Real]) -> Real {
    let ncon = graph.ncon as usize;
    let mut max: Real = 1.0;
    for i in 0..ncon {
        for j in 0..nparts as usize {
            let cur = graph.pwgts[j * ncon + i] as Real * pijbm[j * ncon + i];
            if cur > max {
                max = cur;
            }
        }
    }
    max
}

/// `ComputeLoadImbalanceDiff` (`mcutil.c:256`): `max_j (pwgts[j]*pijbm[j] -
/// ubvec)` in `real_t`, initialised to `-1.0`.
fn compute_load_imbalance_diff(
    graph: &WGraph,
    nparts: Idx,
    pijbm: &[Real],
    ubvec: &[Real],
) -> Real {
    let ncon = graph.ncon as usize;
    let mut max: Real = -1.0;
    for i in 0..ncon {
        for j in 0..nparts as usize {
            let cur = graph.pwgts[j * ncon + i] as Real * pijbm[j * ncon + i] - ubvec[i];
            if cur > max {
                max = cur;
            }
        }
    }
    max
}

/// `MlevelKWayPartitioning` (`kmetis.c:106`), `ncuts == 1`, objtype CUT.
pub(super) fn mlevel_kway_partitioning(
    ctrl: &mut Ctrl,
    rng: &mut Rng,
    graph: WGraph,
    part: &mut [Idx],
) -> Idx {
    // Single cut (ncuts == 1): no best-of-N bookkeeping needed.
    let orig_nedges = graph.nedges;

    // CoarsenGraph -> the finest..coarsest chain.
    let mut levels = super::coarsen::coarsen_graph(ctrl, rng, graph);

    // AllocateKWayPartitionMemory on the coarsest.
    {
        let last = levels.len() - 1;
        allocate_kway_partition_memory(ctrl, &mut levels[last]);
    }

    // InitKWayPartitioning on the coarsest (recursive bisection).
    {
        let last = levels.len() - 1;
        // Borrow the coarsest out for the recursive bisection call.
        let cgraph = &mut levels[last];
        super::recursive::init_kway_partitioning(ctrl, rng, cgraph);
    }

    // AllocateRefinementWorkSpace: reset cnbrpool + build the sqrt lookup table.
    // (nbrpoolsize_max/nbrpoolsize only affect the C allocation; here the pool
    // grows on demand.)
    let _ = orig_nedges;
    ctrl.cnbrpool.clear();
    ctrl.nbrpoolcpos = 0;

    // RefineKWay walks the chain up to the original graph.
    refine_kway(ctrl, rng, &mut levels);

    // curobj = orggraph->mincut; icopy(where -> part).
    let bestobj = levels[0].mincut;
    part.copy_from_slice(&levels[0].where_[..levels[0].nvtxs as usize]);

    bestobj
}

/// `AllocateKWayPartitionMemory` (`kwayrefine.c:118`), objtype CUT.
fn allocate_kway_partition_memory(ctrl: &Ctrl, graph: &mut WGraph) {
    let nvtxs = graph.nvtxs as usize;
    graph.pwgts = vec![0; (ctrl.nparts * graph.ncon) as usize];
    graph.where_ = vec![0; nvtxs];
    graph.bndptr = vec![0; nvtxs];
    graph.bndind = vec![0; nvtxs];
    graph.ckrinfo = vec![CkrInfo::default(); nvtxs];
}

/// `RefineKWay` (`kwayrefine.c:17`), `minconn == contig == 0`.
fn refine_kway(ctrl: &mut Ctrl, rng: &mut Rng, levels: &mut [WGraph]) {
    let nlevels = (levels.len() - 1) as Idx;

    // Build the cnbrsqrt lookup table (sqrt in double) — replaces the inline
    // sqrt(nnbrs) in the gain priority (struct.h rationale).
    let mut cnbrsqrt = vec![0.0f64; (ctrl.nparts + 1) as usize];
    for (i, s) in cnbrsqrt.iter_mut().enumerate() {
        *s = (i as f64).sqrt();
    }

    // Parameters of the coarsest graph.
    {
        let last = levels.len() - 1;
        compute_kway_partition_params(ctrl, &mut levels[last]);
    }

    let mut idx = levels.len() - 1;
    let mut i: Idx = 0;
    loop {
        if 2 * i >= nlevels && !is_balanced(ctrl, &levels[idx], 0.02) {
            compute_kway_boundary(&mut levels[idx], BNDTYPE_BALANCE);
            greedy_kway_cut_optimize(ctrl, rng, &mut levels[idx], 1, OMODE_BALANCE, &cnbrsqrt);
            compute_kway_boundary(&mut levels[idx], BNDTYPE_REFINE);
        }

        let niter = ctrl.niter;
        greedy_kway_cut_optimize(ctrl, rng, &mut levels[idx], niter, OMODE_REFINE, &cnbrsqrt);

        if idx == 0 {
            break;
        }
        idx -= 1;
        project_kway_partition(ctrl, levels, idx);
        i += 1;
    }

    // Final balance if needed (ffactor 0.0).
    if !is_balanced(ctrl, &levels[0], 0.0) {
        compute_kway_boundary(&mut levels[0], BNDTYPE_BALANCE);
        greedy_kway_cut_optimize(ctrl, rng, &mut levels[0], 10, OMODE_BALANCE, &cnbrsqrt);
        compute_kway_boundary(&mut levels[0], BNDTYPE_REFINE);
        let niter = ctrl.niter;
        greedy_kway_cut_optimize(ctrl, rng, &mut levels[0], niter, OMODE_REFINE, &cnbrsqrt);
    }
}

/// `IsBalanced` (`kwayrefine.c:676`).
fn is_balanced(ctrl: &Ctrl, graph: &WGraph, ffactor: Real) -> bool {
    compute_load_imbalance_diff(graph, ctrl.nparts, &ctrl.pijbm, &ctrl.ubfactors) <= ffactor
}

/// `ListInsert`/`BNDInsert` (`macros.h:46`).
#[inline]
fn bnd_insert(nbnd: &mut Idx, bndind: &mut [Idx], bndptr: &mut [Idx], v: Idx) {
    bndind[*nbnd as usize] = v;
    bndptr[v as usize] = *nbnd;
    *nbnd += 1;
}

/// `ListDelete`/`BNDDelete` (`macros.h:53`).
#[inline]
fn bnd_delete(nbnd: &mut Idx, bndind: &mut [Idx], bndptr: &mut [Idx], v: Idx) {
    *nbnd -= 1;
    let last = bndind[*nbnd as usize];
    bndind[bndptr[v as usize] as usize] = last;
    bndptr[last as usize] = bndptr[v as usize];
    bndptr[v as usize] = -1;
}

/// `ComputeKWayPartitionParams` (`kwayrefine.c:151`), objtype CUT, `ncon == 1`.
fn compute_kway_partition_params(ctrl: &mut Ctrl, graph: &mut WGraph) {
    let nparts = ctrl.nparts;
    let nvtxs = graph.nvtxs as usize;

    let WGraph {
        xadj,
        vwgt,
        adjncy,
        adjwgt,
        where_,
        pwgts,
        bndind,
        bndptr,
        ckrinfo,
        ..
    } = graph;

    // iset(nparts*ncon, 0, pwgts); iset(nvtxs, -1, bndptr) (ncon == 1).
    *pwgts = vec![0; nparts as usize];
    for i in 0..nvtxs {
        bndptr[i] = -1;
    }

    // pwgts (ncon == 1)
    for i in 0..nvtxs {
        pwgts[where_[i] as usize] += vwgt[i];
    }

    // reset ckrinfo + cnbrpool
    for c in ckrinfo.iter_mut() {
        *c = CkrInfo::default();
    }
    ctrl.cnbrpool_reset();

    let mut nbnd: Idx = 0;
    let mut mincut: Idx = 0;

    for i in 0..nvtxs {
        let me = where_[i];
        let (istart, iend) = (xadj[i] as usize, xadj[i + 1] as usize);

        let mut myid: Idx = 0;
        let mut myed: Idx = 0;
        for j in istart..iend {
            if me == where_[adjncy[j] as usize] {
                myid += adjwgt[j];
            } else {
                myed += adjwgt[j];
            }
        }
        ckrinfo[i].id = myid;
        ckrinfo[i].ed = myed;

        if myed > 0 {
            mincut += myed;

            let inbr = ctrl.cnbrpool_get_next((xadj[i + 1] - xadj[i]) as Idx);
            ckrinfo[i].inbr = inbr;
            let base = inbr as usize;
            let mut nnbrs: Idx = 0;

            for j in istart..iend {
                let other = where_[adjncy[j] as usize];
                if me != other {
                    let mut found = false;
                    for k in 0..nnbrs as usize {
                        if ctrl.cnbrpool[base + k].pid == other {
                            ctrl.cnbrpool[base + k].ed += adjwgt[j];
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        ctrl.cnbrpool[base + nnbrs as usize] = Cnbr {
                            pid: other,
                            ed: adjwgt[j],
                        };
                        nnbrs += 1;
                    }
                }
            }
            ckrinfo[i].nnbrs = nnbrs;

            if ckrinfo[i].ed - ckrinfo[i].id >= 0 {
                bnd_insert(&mut nbnd, bndind, bndptr, i as Idx);
            }
        } else {
            ckrinfo[i].inbr = -1;
        }
    }

    graph.mincut = mincut / 2;
    graph.nbnd = nbnd;
}

/// `ProjectKWayPartition` (`kwayrefine.c:317`), objtype CUT, `dropedges == 0`.
/// `levels[idx]` is the finer graph being projected onto; `levels[idx+1]` is the
/// coarser source.
fn project_kway_partition(ctrl: &mut Ctrl, levels: &mut [WGraph], idx: usize) {
    let nparts = ctrl.nparts;

    let (finer_slice, coarser_slice) = levels.split_at_mut(idx + 1);
    let graph = &mut finer_slice[idx];
    let cgraph = &coarser_slice[0];

    let nvtxs = graph.nvtxs as usize;

    // AllocateKWayPartitionMemory(graph)
    graph.pwgts = vec![0; (nparts * graph.ncon) as usize];
    graph.where_ = vec![0; nvtxs];
    graph.bndptr = vec![-1; nvtxs];
    graph.bndind = vec![0; nvtxs];
    graph.ckrinfo = vec![CkrInfo::default(); nvtxs];

    let mut htable = vec![-1 as Idx; nparts as usize];

    // Project partition + record coarse ed into cmap for the interior shortcut.
    for i in 0..nvtxs {
        let k = graph.cmap[i] as usize;
        graph.where_[i] = cgraph.where_[k];
        graph.cmap[i] = cgraph.ckrinfo[k].ed; // dropedges==0
    }

    ctrl.cnbrpool_reset();

    let WGraph {
        xadj,
        adjncy,
        adjwgt,
        where_,
        cmap,
        bndind,
        bndptr,
        ckrinfo,
        ..
    } = graph;

    let mut nbnd: Idx = 0;
    for i in 0..nvtxs {
        let (istart, iend) = (xadj[i] as usize, xadj[i + 1] as usize);

        if cmap[i] == 0 {
            // Interior node.
            let mut tid: Idx = 0;
            for j in istart..iend {
                tid += adjwgt[j];
            }
            ckrinfo[i].id = tid;
            ckrinfo[i].inbr = -1;
        } else {
            let inbr = ctrl.cnbrpool_get_next((iend - istart) as Idx);
            ckrinfo[i].inbr = inbr;
            let base = inbr as usize;
            let mut nnbrs: Idx = 0;

            let me = where_[i];
            let mut tid: Idx = 0;
            let mut ted: Idx = 0;
            for j in istart..iend {
                let other = where_[adjncy[j] as usize];
                if me == other {
                    tid += adjwgt[j];
                } else {
                    ted += adjwgt[j];
                    let k = htable[other as usize];
                    if k == -1 {
                        htable[other as usize] = nnbrs;
                        ctrl.cnbrpool[base + nnbrs as usize] = Cnbr {
                            pid: other,
                            ed: adjwgt[j],
                        };
                        nnbrs += 1;
                    } else {
                        ctrl.cnbrpool[base + k as usize].ed += adjwgt[j];
                    }
                }
            }
            ckrinfo[i].id = tid;
            ckrinfo[i].ed = ted;
            ckrinfo[i].nnbrs = nnbrs;

            if ted == 0 {
                // Was actually interior: give back the reserved slots.
                let give_back = nparts.min((iend - istart) as Idx) as usize;
                ctrl.nbrpoolcpos -= give_back;
                ckrinfo[i].inbr = -1;
            } else {
                if ted - tid >= 0 {
                    bnd_insert(&mut nbnd, bndind, bndptr, i as Idx);
                }
                for j in 0..nnbrs as usize {
                    htable[ctrl.cnbrpool[base + j].pid as usize] = -1;
                }
            }
        }
    }

    graph.nbnd = nbnd;
    graph.mincut = cgraph.mincut; // dropedges==0
    graph
        .pwgts
        .copy_from_slice(&cgraph.pwgts[..(nparts * graph.ncon) as usize]);
}

/// `ComputeKWayBoundary` (`kwayrefine.c:517`), objtype CUT.
fn compute_kway_boundary(graph: &mut WGraph, bndtype: i32) {
    let nvtxs = graph.nvtxs as usize;
    let WGraph {
        ckrinfo,
        bndind,
        bndptr,
        ..
    } = graph;
    for i in 0..nvtxs {
        bndptr[i] = -1;
    }
    let mut nbnd: Idx = 0;
    if bndtype == BNDTYPE_REFINE {
        for i in 0..nvtxs {
            if ckrinfo[i].ed > 0 && ckrinfo[i].ed - ckrinfo[i].id >= 0 {
                bnd_insert(&mut nbnd, bndind, bndptr, i as Idx);
            }
        }
    } else {
        for i in 0..nvtxs {
            if ckrinfo[i].ed > 0 {
                bnd_insert(&mut nbnd, bndind, bndptr, i as Idx);
            }
        }
    }
    graph.nbnd = nbnd;
}

/// The k-way cut gain priority key `ed/sqrt(nnbrs) - id` (`kwayfm.c:189` and the
/// `UpdateQueueInfo` macro), computed in `double` and narrowed to `real_t`.
#[inline]
fn kway_rgain(ed: Idx, id: Idx, nnbrs: Idx, cnbrsqrt: &[f64]) -> Real {
    let base = if nnbrs > 0 {
        1.0f64 * ed as f64 / cnbrsqrt[nnbrs as usize]
    } else {
        0.0
    };
    (base - id as f64) as Real
}

/// `Greedy_KWayCutOptimize` (`kwayfm.c:59`), `ncon == 1`, `minconn == contig ==
/// 0` (so `safetos` is uniformly 2 and the article-point / safe-target logic is
/// elided). `ffactor` is forced to 0.0 by the routine itself.
#[allow(clippy::too_many_arguments)]
fn greedy_kway_cut_optimize(
    ctrl: &mut Ctrl,
    rng: &mut Rng,
    graph: &mut WGraph,
    niter: Idx,
    omode: i32,
    cnbrsqrt: &[f64],
) {
    let nvtxs = graph.nvtxs as usize;
    let nparts = ctrl.nparts;
    let bndtype = if omode == OMODE_REFINE {
        BNDTYPE_REFINE
    } else {
        BNDTYPE_BALANCE
    };

    // Weight intervals.
    let ubfactor: Real = if omode == OMODE_BALANCE {
        ctrl.ubfactors[0]
    } else {
        let cli = ComputeLoadImbalance_(graph, nparts, &ctrl.pijbm);
        // gk_max(a, b) = (a >= b ? a : b)
        if ctrl.ubfactors[0] >= cli {
            ctrl.ubfactors[0]
        } else {
            cli
        }
    };

    let tvwgt0 = graph.tvwgt[0];
    let mut maxpwgts = vec![0 as Idx; nparts as usize];
    let mut minpwgts = vec![0 as Idx; nparts as usize];
    for i in 0..nparts as usize {
        // maxpwgts = tpwgts*tvwgt0*ubfactor  (all real_t, truncate to idx_t)
        maxpwgts[i] = (ctrl.tpwgts[i] * tvwgt0 as Real * ubfactor) as Idx;
        // minpwgts = tpwgts*tvwgt0*(1.0/ubfactor): last factor is double.
        minpwgts[i] =
            ((ctrl.tpwgts[i] * tvwgt0 as Real) as f64 * (1.0f64 / ubfactor as f64)) as Idx;
    }

    let mut perm = vec![0 as Idx; nvtxs];
    let mut vstatus = vec![VPQSTATUS_NOTPRESENT; nvtxs];
    let mut updptr = vec![-1 as Idx; nvtxs];
    let mut updind = vec![0 as Idx; nvtxs];

    let mut queue = Rpq::create(nvtxs);

    for _pass in 0..niter {
        if omode == OMODE_BALANCE {
            // If balanced, return.
            let mut balanced = true;
            for i in 0..nparts as usize {
                if graph.pwgts[i] > maxpwgts[i] || graph.pwgts[i] < minpwgts[i] {
                    balanced = false;
                    break;
                }
            }
            if balanced {
                break;
            }
        }

        let oldcut = graph.mincut;
        let mut nbnd = graph.nbnd;
        let mut nupd: Idx = 0;

        // Insert boundary vertices.
        rng.rand_array_permute(nbnd, &mut perm, nbnd / 4, 1);
        for ii in 0..nbnd as usize {
            let i = graph.bndind[perm[ii] as usize];
            let cr = graph.ckrinfo[i as usize];
            let rgain = kway_rgain(cr.ed, cr.id, cr.nnbrs, cnbrsqrt);
            queue.insert(i, rgain);
            vstatus[i as usize] = VPQSTATUS_PRESENT;
            // ListInsert(nupd, updind, updptr, i)
            updind[nupd as usize] = i;
            updptr[i as usize] = nupd;
            nupd += 1;
        }

        let mut nmoved: Idx = 0;
        loop {
            let i = queue.get_top();
            if i == -1 {
                break;
            }
            let i = i as usize;
            vstatus[i] = VPQSTATUS_EXTRACTED;

            let from = graph.where_[i];
            let vwgt = graph.vwgt[i];
            let myid = graph.ckrinfo[i].id;
            let inbr = graph.ckrinfo[i].inbr as usize;
            let nnbrs = graph.ckrinfo[i].nnbrs;

            // Find the most promising target subdomain.
            let mut k: i32 = nnbrs - 1;
            if omode == OMODE_REFINE {
                while k >= 0 {
                    let to = ctrl.cnbrpool[inbr + k as usize].pid;
                    let ked = ctrl.cnbrpool[inbr + k as usize].ed;
                    if refine_first_ok(
                        ctrl,
                        &graph.pwgts,
                        &minpwgts,
                        &maxpwgts,
                        ked,
                        myid,
                        from,
                        to,
                        vwgt,
                    ) {
                        break;
                    }
                    k -= 1;
                }
                if k < 0 {
                    continue;
                }

                let mut j = k - 1;
                while j >= 0 {
                    let to = ctrl.cnbrpool[inbr + j as usize].pid;
                    let jed = ctrl.cnbrpool[inbr + j as usize].ed;
                    let ked = ctrl.cnbrpool[inbr + k as usize].ed;
                    let kpid = ctrl.cnbrpool[inbr + k as usize].pid;
                    let better = (jed > ked
                        && ((graph.pwgts[from as usize] - vwgt >= minpwgts[from as usize])
                            || lt_f32(ctrl, from, to, &graph.pwgts, vwgt))
                        && ((graph.pwgts[to as usize] + vwgt <= maxpwgts[to as usize])
                            || lt_f32(ctrl, from, to, &graph.pwgts, vwgt)))
                        || (jed == ked
                            && (ctrl.tpwgts[kpid as usize] * (graph.pwgts[to as usize] as Real)
                                < ctrl.tpwgts[to as usize] * (graph.pwgts[kpid as usize] as Real)));
                    if better {
                        k = j;
                    }
                    j -= 1;
                }
            } else {
                // OMODE_BALANCE
                while k >= 0 {
                    let to = ctrl.cnbrpool[inbr + k as usize].pid;
                    // (from < nparts always) => just the tpwgts test
                    if from >= nparts || lt_f32(ctrl, from, to, &graph.pwgts, vwgt) {
                        break;
                    }
                    k -= 1;
                }
                if k < 0 {
                    continue;
                }

                let mut j = k - 1;
                while j >= 0 {
                    let to = ctrl.cnbrpool[inbr + j as usize].pid;
                    let kpid = ctrl.cnbrpool[inbr + k as usize].pid;
                    if ctrl.tpwgts[kpid as usize] * (graph.pwgts[to as usize] as Real)
                        < ctrl.tpwgts[to as usize] * (graph.pwgts[kpid as usize] as Real)
                    {
                        k = j;
                    }
                    j -= 1;
                }
            }

            let to = ctrl.cnbrpool[inbr + k as usize].pid;
            let ked = ctrl.cnbrpool[inbr + k as usize].ed;

            // Move the vertex from 'from' to 'to'.
            graph.mincut -= ked - myid;
            nmoved += 1;

            // INC_DEC(pwgts[to], pwgts[from], vwgt)
            graph.pwgts[to as usize] += vwgt;
            graph.pwgts[from as usize] -= vwgt;

            update_moved_vertex_info_and_bnd(
                graph,
                &mut ctrl.cnbrpool,
                i,
                from,
                k as usize,
                to,
                &mut nbnd,
                bndtype,
            );

            // Update adjacent vertices.
            let (istart, iend) = (graph.xadj[i] as usize, graph.xadj[i + 1] as usize);
            for jj in istart..iend {
                let ii = graph.adjncy[jj] as usize;
                let me = graph.where_[ii];
                let ewgt = graph.adjwgt[jj];
                let adjlen = graph.xadj[ii + 1] - graph.xadj[ii];
                let oldnnbrs = graph.ckrinfo[ii].nnbrs;

                update_adjacent_vertex_info_and_bnd(
                    ctrl, graph, ii, adjlen, me, from, to, ewgt, &mut nbnd, bndtype,
                );

                update_queue_info(
                    &mut queue,
                    &mut vstatus,
                    &mut updind,
                    &mut updptr,
                    &mut nupd,
                    &graph.ckrinfo,
                    ii,
                    me,
                    from,
                    to,
                    oldnnbrs,
                    bndtype,
                    cnbrsqrt,
                );
            }
        }

        graph.nbnd = nbnd;

        // Reset vstatus and the update tracker.
        for i in 0..nupd as usize {
            let v = updind[i] as usize;
            vstatus[v] = VPQSTATUS_NOTPRESENT;
            updptr[v] = -1;
        }

        if nmoved == 0 || (omode == OMODE_REFINE && graph.mincut == oldcut) {
            break;
        }
    }
}

/// The `tpwgts[from]*pwgts[to] < tpwgts[to]*(pwgts[from]-vwgt)` f32 test used
/// throughout `Greedy_KWayCutOptimize`'s target selection.
#[inline]
fn lt_f32(ctrl: &Ctrl, from: Idx, to: Idx, pwgts: &[Idx], vwgt: Idx) -> bool {
    ctrl.tpwgts[from as usize] * (pwgts[to as usize] as Real)
        < ctrl.tpwgts[to as usize] * ((pwgts[from as usize] - vwgt) as Real)
}

/// The first (`k`-scan) acceptance test of the OMODE_REFINE target search.
#[allow(clippy::too_many_arguments)]
#[inline]
fn refine_first_ok(
    ctrl: &Ctrl,
    pwgts: &[Idx],
    minpwgts: &[Idx],
    maxpwgts: &[Idx],
    ked: Idx,
    myid: Idx,
    from: Idx,
    to: Idx,
    vwgt: Idx,
) -> bool {
    (ked > myid
        && ((pwgts[from as usize] - vwgt >= minpwgts[from as usize])
            || lt_f32(ctrl, from, to, pwgts, vwgt))
        && ((pwgts[to as usize] + vwgt <= maxpwgts[to as usize])
            || lt_f32(ctrl, from, to, pwgts, vwgt)))
        || (ked == myid && lt_f32(ctrl, from, to, pwgts, vwgt))
}

/// `UpdateMovedVertexInfoAndBND` (`macros.h:75`).
#[allow(clippy::too_many_arguments)]
fn update_moved_vertex_info_and_bnd(
    graph: &mut WGraph,
    cnbrpool: &mut [Cnbr],
    i: usize,
    from: Idx,
    k: usize,
    to: Idx,
    nbnd: &mut Idx,
    bndtype: i32,
) {
    graph.where_[i] = to;
    let inbr = graph.ckrinfo[i].inbr as usize;
    let myr = &mut graph.ckrinfo[i];
    myr.ed += myr.id - cnbrpool[inbr + k].ed;
    // SWAP(myrinfo->id, mynbrs[k].ed, j)
    std::mem::swap(&mut myr.id, &mut cnbrpool[inbr + k].ed);
    if cnbrpool[inbr + k].ed == 0 {
        myr.nnbrs -= 1;
        cnbrpool[inbr + k] = cnbrpool[inbr + myr.nnbrs as usize];
    } else {
        cnbrpool[inbr + k].pid = from;
    }

    let ed = myr.ed;
    let id = myr.id;
    if bndtype == BNDTYPE_REFINE {
        if graph.bndptr[i] != -1 && ed - id < 0 {
            bnd_delete(nbnd, &mut graph.bndind, &mut graph.bndptr, i as Idx);
        }
        if graph.bndptr[i] == -1 && ed - id >= 0 {
            bnd_insert(nbnd, &mut graph.bndind, &mut graph.bndptr, i as Idx);
        }
    } else {
        if graph.bndptr[i] != -1 && ed <= 0 {
            bnd_delete(nbnd, &mut graph.bndind, &mut graph.bndptr, i as Idx);
        }
        if graph.bndptr[i] == -1 && ed > 0 {
            bnd_insert(nbnd, &mut graph.bndind, &mut graph.bndptr, i as Idx);
        }
    }
}

/// `UpdateAdjacentVertexInfoAndBND` (`macros.h:103`).
#[allow(clippy::too_many_arguments)]
fn update_adjacent_vertex_info_and_bnd(
    ctrl: &mut Ctrl,
    graph: &mut WGraph,
    vid: usize,
    adjlen: Idx,
    me: Idx,
    from: Idx,
    to: Idx,
    ewgt: Idx,
    nbnd: &mut Idx,
    bndtype: i32,
) {
    if graph.ckrinfo[vid].inbr == -1 {
        let inbr = ctrl.cnbrpool_get_next(adjlen);
        graph.ckrinfo[vid].inbr = inbr;
        graph.ckrinfo[vid].nnbrs = 0;
    }
    let base = graph.ckrinfo[vid].inbr as usize;

    // Update global ID/ED + boundary.
    if me == from {
        // INC_DEC(ed, id, ewgt)
        graph.ckrinfo[vid].ed += ewgt;
        graph.ckrinfo[vid].id -= ewgt;
        let ed = graph.ckrinfo[vid].ed;
        let id = graph.ckrinfo[vid].id;
        if bndtype == BNDTYPE_REFINE {
            if ed - id >= 0 && graph.bndptr[vid] == -1 {
                bnd_insert(nbnd, &mut graph.bndind, &mut graph.bndptr, vid as Idx);
            }
        } else if ed > 0 && graph.bndptr[vid] == -1 {
            bnd_insert(nbnd, &mut graph.bndind, &mut graph.bndptr, vid as Idx);
        }
    } else if me == to {
        graph.ckrinfo[vid].id += ewgt;
        graph.ckrinfo[vid].ed -= ewgt;
        let ed = graph.ckrinfo[vid].ed;
        let id = graph.ckrinfo[vid].id;
        if bndtype == BNDTYPE_REFINE {
            if ed - id < 0 && graph.bndptr[vid] != -1 {
                bnd_delete(nbnd, &mut graph.bndind, &mut graph.bndptr, vid as Idx);
            }
        } else if ed <= 0 && graph.bndptr[vid] != -1 {
            bnd_delete(nbnd, &mut graph.bndind, &mut graph.bndptr, vid as Idx);
        }
    }

    // Remove contribution from the .ed of 'from'.
    if me != from {
        let nnbrs = graph.ckrinfo[vid].nnbrs;
        for k in 0..nnbrs as usize {
            if ctrl.cnbrpool[base + k].pid == from {
                if ctrl.cnbrpool[base + k].ed == ewgt {
                    let last = graph.ckrinfo[vid].nnbrs - 1;
                    graph.ckrinfo[vid].nnbrs = last;
                    ctrl.cnbrpool[base + k] = ctrl.cnbrpool[base + last as usize];
                } else {
                    ctrl.cnbrpool[base + k].ed -= ewgt;
                }
                break;
            }
        }
    }

    // Add contribution to the .ed of 'to'.
    if me != to {
        let nnbrs = graph.ckrinfo[vid].nnbrs;
        let mut found = false;
        for k in 0..nnbrs as usize {
            if ctrl.cnbrpool[base + k].pid == to {
                ctrl.cnbrpool[base + k].ed += ewgt;
                found = true;
                break;
            }
        }
        if !found {
            ctrl.cnbrpool[base + nnbrs as usize] = Cnbr { pid: to, ed: ewgt };
            graph.ckrinfo[vid].nnbrs = nnbrs + 1;
        }
    }
}

/// `UpdateQueueInfo` (`macros.h:173`).
#[allow(clippy::too_many_arguments)]
fn update_queue_info(
    queue: &mut Rpq,
    vstatus: &mut [i32],
    updind: &mut [Idx],
    updptr: &mut [Idx],
    nupd: &mut Idx,
    ckrinfo: &[CkrInfo],
    vid: usize,
    me: Idx,
    from: Idx,
    to: Idx,
    oldnnbrs: Idx,
    bndtype: i32,
    cnbrsqrt: &[f64],
) {
    let nnbrs = ckrinfo[vid].nnbrs;
    if !(me == to || me == from || oldnnbrs != nnbrs) {
        return;
    }
    let ed = ckrinfo[vid].ed;
    let id = ckrinfo[vid].id;
    let rgain = kway_rgain(ed, id, nnbrs, cnbrsqrt);

    let cond = if bndtype == BNDTYPE_REFINE {
        ed - id >= 0
    } else {
        ed > 0
    };

    if vstatus[vid] == VPQSTATUS_PRESENT {
        if cond {
            queue.update(vid as Idx, rgain);
        } else {
            queue.delete(vid as Idx);
            vstatus[vid] = VPQSTATUS_NOTPRESENT;
            // ListDelete(nupd, updind, updptr, vid)
            *nupd -= 1;
            let last = updind[*nupd as usize];
            updind[updptr[vid] as usize] = last;
            updptr[last as usize] = updptr[vid];
            updptr[vid] = -1;
        }
    } else if vstatus[vid] == VPQSTATUS_NOTPRESENT && cond {
        queue.insert(vid as Idx, rgain);
        vstatus[vid] = VPQSTATUS_PRESENT;
        // ListInsert(nupd, updind, updptr, vid)
        updind[*nupd as usize] = vid as Idx;
        updptr[vid] = *nupd;
        *nupd += 1;
    }
    let _ = VPQSTATUS_EXTRACTED;
}
