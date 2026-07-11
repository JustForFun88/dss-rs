//! Graph coarsening, ported 1:1 from `libmetis/coarsen.c` (SHEM/RM matching +
//! 2-hop matching + contraction) and `bucketsort.c::BucketSortKeysInc`. Only the
//! `ncon == 1`, `objtype == CUT`, `dropedges == 0` path is ported (see
//! `part/mod.rs` reachability notes). The RNG stream position is part of the
//! contract: each level draws one `irandArrayPermute(nvtxs, tperm, nvtxs/8, 1)`.

// Index-based loops mirror the C source operation-for-operation (they index
// several arrays by the same counter and/or use it arithmetically); the iterator
// rewrites clippy suggests would obscure the 1:1 correspondence.
#![allow(clippy::needless_range_loop)]

use super::{Ctrl, WGraph};
use crate::Idx;
use crate::rng::Rng;
use crate::sort::{Ikv, ikvsorti};

/// `UNMATCHED` (`defs.h:43`).
const UNMATCHED: Idx = -1;
/// `HTLENGTH` hash mask (`defs.h:23`): `(1<<13)-1`.
const HTLENGTH: Idx = (1 << 13) - 1;
/// `UNMATCHEDFOR2HOP` (`coarsen.c:14`): the fraction of unmatched vertices that
/// triggers 2-hop matching.
const UNMATCHEDFOR2HOP: f64 = 0.10;
/// `COARSEN_FRACTION` (`defs.h:48`).
const COARSEN_FRACTION: f64 = 0.85;

/// `CoarsenGraph` (`coarsen.c:22`): build the sequence of coarser graphs. Returns
/// the chain finest-first (index 0 = input, last = coarsest); the C's
/// `coarser`/`finer` links are the `Vec` adjacency.
pub(super) fn coarsen_graph(ctrl: &mut Ctrl, rng: &mut Rng, graph: WGraph) -> Vec<WGraph> {
    // eqewgts: are all edge weights identical?
    let mut eqewgts = true;
    {
        let ne = graph.nedges as usize;
        for i in 1..ne {
            if graph.adjwgt[0] != graph.adjwgt[i] {
                eqewgts = false;
                break;
            }
        }
    }

    // maxvwgt[i] = 1.5*tvwgt[i]/CoarsenTo (double, truncated to idx_t).
    for i in 0..graph.ncon as usize {
        ctrl.maxvwgt[i] = (1.5f64 * graph.tvwgt[i] as f64 / ctrl.coarsen_to as f64) as Idx;
    }

    let mut levels: Vec<WGraph> = vec![graph];

    loop {
        // Allocate cmap if not already present (persists across cuts; single cut
        // here, so it is always fresh).
        {
            let cur = levels.last_mut().unwrap();
            if cur.cmap.is_empty() {
                cur.cmap = vec![0; cur.nvtxs as usize];
            }
        }

        let use_rm = {
            let cur = levels.last().unwrap();
            eqewgts || cur.nedges == 0
        };

        let coarser = {
            let cur = levels.last_mut().unwrap();
            if use_rm {
                match_rm(ctrl, rng, cur)
            } else {
                match_shem(ctrl, rng, cur)
            }
        };

        levels.push(coarser);
        eqewgts = false;

        let len = levels.len();
        let cn = levels[len - 1].nvtxs;
        let fnvtxs = levels[len - 2].nvtxs;
        let cne = levels[len - 1].nedges;
        // while (cn > CoarsenTo && cn < 0.85*finer.nvtxs && cne > cn/2)
        if !(cn > ctrl.coarsen_to && (cn as f64) < COARSEN_FRACTION * fnvtxs as f64 && cne > cn / 2)
        {
            break;
        }
    }

    levels
}

/// `BucketSortKeysInc` (`bucketsort.c:23`): counting sort producing `perm`, the
/// `tperm` order stably reordered by increasing `keys`.
fn bucket_sort_keys_inc(n: Idx, max: Idx, keys: &[Idx], tperm: &[Idx], perm: &mut [Idx]) {
    let mut counts = vec![0 as Idx; (max + 2) as usize];
    for i in 0..n as usize {
        counts[keys[i] as usize] += 1;
    }
    // MAKECSR(i, max+1, counts)
    let nn = (max + 1) as usize;
    for i in 1..nn {
        counts[i] += counts[i - 1];
    }
    for i in (1..=nn).rev() {
        counts[i] = counts[i - 1];
    }
    counts[0] = 0;

    for ii in 0..n as usize {
        let i = tperm[ii] as usize;
        let ki = keys[i] as usize;
        perm[counts[ki] as usize] = i as Idx;
        counts[ki] += 1;
    }
}

/// Shared setup for both matching schemes: build the degree-biased traversal
/// `perm` and return `(match, perm, degrees, avgdegree)`. Mirrors the identical
/// preambles of `Match_RM`/`Match_SHEM` (`coarsen.c:175`/`316`).
fn matching_preamble(rng: &mut Rng, graph: &WGraph) -> (Vec<Idx>, Vec<Idx>, Idx) {
    let nvtxs = graph.nvtxs as usize;
    let match_ = vec![UNMATCHED; nvtxs];
    let mut perm = vec![0 as Idx; nvtxs];
    let mut tperm = vec![0 as Idx; nvtxs];
    let mut degrees = vec![0 as Idx; nvtxs];

    // irandArrayPermute(nvtxs, tperm, nvtxs/8, 1)
    rng.rand_array_permute(nvtxs as Idx, &mut tperm, (nvtxs / 8) as Idx, 1);

    // avgdegree = 4.0*(xadj[nvtxs]/nvtxs)  (int divide, *4.0 double, truncate).
    let avgdegree = (4.0f64 * (graph.xadj[nvtxs] / graph.nvtxs) as f64) as Idx;
    for i in 0..nvtxs {
        let deg = graph.xadj[i + 1] - graph.xadj[i];
        let bnum = ((1 + deg) as f64).sqrt() as Idx;
        degrees[i] = if bnum > avgdegree { avgdegree } else { bnum };
    }
    bucket_sort_keys_inc(graph.nvtxs, avgdegree, &degrees, &tperm, &mut perm);

    (match_, perm, avgdegree)
}

/// `Match_RM` (`coarsen.c:153`), `ncon == 1`: random (first-fit) matching.
fn match_rm(ctrl: &mut Ctrl, rng: &mut Rng, graph: &mut WGraph) -> WGraph {
    let nvtxs = graph.nvtxs as usize;
    let maxvwgt0 = ctrl.maxvwgt[0];
    let (mut match_, perm, _avg) = matching_preamble(rng, graph);
    let mut nunmatched: usize = 0;

    let mut last_unmatched: Idx = 0;
    for pi in 0..nvtxs {
        let i = perm[pi] as usize;
        if match_[i] != UNMATCHED {
            continue;
        }
        let mut maxidx = i as Idx;

        if graph.vwgt[i] < maxvwgt0 {
            if graph.xadj[i] == graph.xadj[i + 1] {
                // island vertex
                last_unmatched = (pi as Idx).max(last_unmatched) + 1;
                while (last_unmatched as usize) < nvtxs {
                    let j = perm[last_unmatched as usize];
                    if match_[j as usize] == UNMATCHED {
                        maxidx = j;
                        break;
                    }
                    last_unmatched += 1;
                }
            } else {
                for j in graph.xadj[i] as usize..graph.xadj[i + 1] as usize {
                    let k = graph.adjncy[j] as usize;
                    if match_[k] == UNMATCHED && graph.vwgt[i] + graph.vwgt[k] <= maxvwgt0 {
                        maxidx = k as Idx;
                        break;
                    }
                }
                if maxidx == i as Idx && 2 * graph.vwgt[i] < maxvwgt0 {
                    nunmatched += 1;
                    maxidx = UNMATCHED;
                }
            }
        }

        if maxidx != UNMATCHED {
            match_[i] = maxidx;
            match_[maxidx as usize] = i as Idx;
        }
    }

    finalize_matching(ctrl, rng, graph, &mut match_, &perm, nunmatched)
}

/// `Match_SHEM` (`coarsen.c:294`), `ncon == 1`: sorted heavy-edge matching.
fn match_shem(ctrl: &mut Ctrl, rng: &mut Rng, graph: &mut WGraph) -> WGraph {
    let nvtxs = graph.nvtxs as usize;
    let maxvwgt0 = ctrl.maxvwgt[0];
    let (mut match_, perm, _avg) = matching_preamble(rng, graph);
    let mut nunmatched: usize = 0;

    let mut last_unmatched: Idx = 0;
    for pi in 0..nvtxs {
        let i = perm[pi] as usize;
        if match_[i] != UNMATCHED {
            continue;
        }
        let mut maxidx = i as Idx;
        let mut maxwgt: Idx = -1;

        if graph.vwgt[i] < maxvwgt0 {
            if graph.xadj[i] == graph.xadj[i + 1] {
                // island vertex
                last_unmatched = (pi as Idx).max(last_unmatched) + 1;
                while (last_unmatched as usize) < nvtxs {
                    let j = perm[last_unmatched as usize];
                    if match_[j as usize] == UNMATCHED {
                        maxidx = j;
                        break;
                    }
                    last_unmatched += 1;
                }
            } else {
                for j in graph.xadj[i] as usize..graph.xadj[i + 1] as usize {
                    let k = graph.adjncy[j] as usize;
                    if maxwgt < graph.adjwgt[j]
                        && match_[k] == UNMATCHED
                        && graph.vwgt[i] + graph.vwgt[k] <= maxvwgt0
                    {
                        maxidx = k as Idx;
                        maxwgt = graph.adjwgt[j];
                    }
                }
                if maxidx == i as Idx && 2 * graph.vwgt[i] < maxvwgt0 {
                    nunmatched += 1;
                    maxidx = UNMATCHED;
                }
            }
        }

        if maxidx != UNMATCHED {
            match_[i] = maxidx;
            match_[maxidx as usize] = i as Idx;
        }
    }

    finalize_matching(ctrl, rng, graph, &mut match_, &perm, nunmatched)
}

/// The shared tail of `Match_RM`/`Match_SHEM` (`coarsen.c:260`): optional 2-hop
/// matching, the self-match + cmap renumber pass, then `CreateCoarseGraph`.
fn finalize_matching(
    ctrl: &mut Ctrl,
    _rng: &mut Rng,
    graph: &mut WGraph,
    match_: &mut [Idx],
    perm: &[Idx],
    mut nunmatched: usize,
) -> WGraph {
    let nvtxs = graph.nvtxs as usize;

    // 2-hop matching if enabled (no2hop=false) and enough unmatched remain.
    // The running `cnvtxs` the C threads here is only *added to* by Match_2Hop
    // and then discarded — the renumber pass below recomputes the true count
    // from `match_` — so its exact seed value is immaterial. We still seed it
    // faithfully (count of matched pairs so far) for a 1:1 reading.
    if !ctrl.no2hop && (nunmatched as f64) > UNMATCHEDFOR2HOP * nvtxs as f64 {
        let cnvtxs = count_matched_pairs(match_, nvtxs);
        let _ = match_2hop(graph, perm, match_, cnvtxs, &mut nunmatched);
    }

    // Self-match remaining + assign cmap in coarse-vertex order.
    let mut cnv: Idx = 0;
    for i in 0..nvtxs {
        if match_[i] == UNMATCHED {
            match_[i] = i as Idx;
            graph.cmap[i] = cnv;
            cnv += 1;
        } else if (i as Idx) <= match_[i] {
            graph.cmap[i] = cnv;
            graph.cmap[match_[i] as usize] = cnv;
            cnv += 1;
        }
    }

    create_coarse_graph(graph, cnv, match_)
}

/// Count the coarse vertices implied by the current partial matching — the
/// running `cnvtxs` the C matching loop maintained. A matched pair `(i, match[i])`
/// with `i <= match[i]` (or a self/island match `match[i] == i`) is one coarse
/// vertex.
fn count_matched_pairs(match_: &[Idx], nvtxs: usize) -> Idx {
    let mut c: Idx = 0;
    for i in 0..nvtxs {
        let m = match_[i];
        if m != UNMATCHED && (i as Idx) <= m {
            c += 1;
        }
    }
    c
}

/// `Match_2Hop` (`coarsen.c:438`).
fn match_2hop(
    graph: &WGraph,
    perm: &[Idx],
    match_: &mut [Idx],
    mut cnvtxs: Idx,
    nunmatched: &mut usize,
) -> Idx {
    cnvtxs = match_2hop_any(graph, perm, match_, cnvtxs, nunmatched, 2);
    cnvtxs = match_2hop_all(graph, perm, match_, cnvtxs, nunmatched, 64);
    if (*nunmatched as f64) > 1.5 * UNMATCHEDFOR2HOP * graph.nvtxs as f64 {
        cnvtxs = match_2hop_any(graph, perm, match_, cnvtxs, nunmatched, 3);
    }
    if (*nunmatched as f64) > 2.0 * UNMATCHEDFOR2HOP * graph.nvtxs as f64 {
        cnvtxs = match_2hop_any(
            graph,
            perm,
            match_,
            cnvtxs,
            nunmatched,
            graph.nvtxs as usize,
        );
    }
    cnvtxs
}

/// `Match_2HopAny` (`coarsen.c:460`): match unmatched vertices of degree
/// `< maxdegree` that share a neighbor (non-empty adjacency overlap).
fn match_2hop_any(
    graph: &WGraph,
    perm: &[Idx],
    match_: &mut [Idx],
    mut cnvtxs: Idx,
    nunmatched: &mut usize,
    maxdegree: usize,
) -> Idx {
    let nvtxs = graph.nvtxs as usize;
    let xadj = &graph.xadj;
    let adjncy = &graph.adjncy;

    // Inverted index (colptr/rowind) over the qualifying unmatched vertices.
    let mut colptr = vec![0 as Idx; nvtxs + 1];
    for i in 0..nvtxs {
        if match_[i] == UNMATCHED && ((xadj[i + 1] - xadj[i]) as usize) < maxdegree {
            for j in xadj[i] as usize..xadj[i + 1] as usize {
                colptr[adjncy[j] as usize] += 1;
            }
        }
    }
    // MAKECSR(i, nvtxs, colptr)
    for i in 1..nvtxs {
        colptr[i] += colptr[i - 1];
    }
    for i in (1..=nvtxs).rev() {
        colptr[i] = colptr[i - 1];
    }
    colptr[0] = 0;

    let mut rowind = vec![0 as Idx; colptr[nvtxs] as usize];
    for pi in 0..nvtxs {
        let i = perm[pi] as usize;
        if match_[i] == UNMATCHED && ((xadj[i + 1] - xadj[i]) as usize) < maxdegree {
            for j in xadj[i] as usize..xadj[i + 1] as usize {
                let slot = colptr[adjncy[j] as usize] as usize;
                rowind[slot] = i as Idx;
                colptr[adjncy[j] as usize] += 1;
            }
        }
    }
    // SHIFTCSR(i, nvtxs, colptr)
    for i in (1..=nvtxs).rev() {
        colptr[i] = colptr[i - 1];
    }
    colptr[0] = 0;

    // Match pairs by scanning down each column of the inverted index.
    for pi in 0..nvtxs {
        let i = perm[pi] as usize;
        if (colptr[i + 1] - colptr[i]) < 2 {
            continue;
        }
        let mut jj = colptr[i + 1];
        let mut j = colptr[i];
        while j < jj {
            if match_[rowind[j as usize] as usize] == UNMATCHED {
                jj -= 1;
                while jj > j {
                    if match_[rowind[jj as usize] as usize] == UNMATCHED {
                        cnvtxs += 1;
                        match_[rowind[j as usize] as usize] = rowind[jj as usize];
                        match_[rowind[jj as usize] as usize] = rowind[j as usize];
                        *nunmatched -= 2;
                        break;
                    }
                    jj -= 1;
                }
            }
            j += 1;
        }
    }

    cnvtxs
}

/// `Match_2HopAll` (`coarsen.c:539`): collapse unmatched vertices with identical
/// adjacency lists (degree in `(1, maxdegree)`).
fn match_2hop_all(
    graph: &WGraph,
    perm: &[Idx],
    match_: &mut [Idx],
    mut cnvtxs: Idx,
    nunmatched: &mut usize,
    maxdegree: usize,
) -> Idx {
    let nvtxs = graph.nvtxs as usize;
    let xadj = &graph.xadj;
    let adjncy = &graph.adjncy;

    // mask = IDX_MAX/maxdegree
    let mask: Idx = Idx::MAX / maxdegree as Idx;

    // Collapse vertices with identical adjacency lists.
    let mut keys: Vec<Ikv> = Vec::with_capacity(*nunmatched);
    for pi in 0..nvtxs {
        let i = perm[pi] as usize;
        let idegree = xadj[i + 1] - xadj[i];
        if match_[i] == UNMATCHED && idegree > 1 && (idegree as usize) < maxdegree {
            let mut k: Idx = 0;
            for j in xadj[i] as usize..xadj[i + 1] as usize {
                k += adjncy[j] % mask;
            }
            keys.push(Ikv {
                val: i as Idx,
                key: (k % mask) * maxdegree as Idx + idegree,
            });
        }
    }
    let ncand = keys.len();
    ikvsorti(&mut keys);

    let mut mark = vec![0 as Idx; nvtxs];
    for pi in 0..ncand {
        let i = keys[pi].val as usize;
        if match_[i] != UNMATCHED {
            continue;
        }

        for j in xadj[i] as usize..xadj[i + 1] as usize {
            mark[adjncy[j] as usize] = i as Idx;
        }

        for pk in (pi + 1)..ncand {
            let k = keys[pk].val as usize;
            if match_[k] != UNMATCHED {
                continue;
            }
            if keys[pi].key != keys[pk].key {
                break;
            }
            if xadj[i + 1] - xadj[i] != xadj[k + 1] - xadj[k] {
                break;
            }

            let mut jj = xadj[k] as usize;
            let end = xadj[k + 1] as usize;
            while jj < end {
                if mark[adjncy[jj] as usize] != i as Idx {
                    break;
                }
                jj += 1;
            }
            if jj == end {
                cnvtxs += 1;
                match_[i] = k as Idx;
                match_[k] = i as Idx;
                *nunmatched -= 2;
                break;
            }
        }
    }

    cnvtxs
}

/// `CreateCoarseGraph` (`coarsen.c:827`), `dovsize == 0`, `dropedges == 0`.
/// Builds the coarser graph's CSR by contracting matched pairs, collapsing
/// parallel edges either through the open-addressing hash table (`htable`, when
/// the combined degree is `< HTLENGTH>>2`) or the direct table (`dtable`).
fn create_coarse_graph(graph: &WGraph, cnvtxs: Idx, match_: &[Idx]) -> WGraph {
    let nvtxs = graph.nvtxs as usize;
    let ncon = graph.ncon as usize; // 1
    let xadj = &graph.xadj;
    let vwgt = &graph.vwgt;
    let adjncy = &graph.adjncy;
    let adjwgt = &graph.adjwgt;
    let cmap = &graph.cmap;

    let mask = HTLENGTH;

    // SetupCoarseGraph: allocate the coarse graph (adjncy/adjwgt have +1 slack
    // for the self-loop optimization).
    let mut cgraph = WGraph::new_empty();
    cgraph.nvtxs = cnvtxs;
    cgraph.ncon = graph.ncon;
    cgraph.xadj = vec![0 as Idx; (cnvtxs + 1) as usize];
    cgraph.adjncy = vec![0 as Idx; (graph.nedges + 1) as usize];
    cgraph.adjwgt = vec![0 as Idx; (graph.nedges + 1) as usize];
    cgraph.vwgt = vec![0 as Idx; (cgraph.ncon * cnvtxs) as usize];

    let mut htable = vec![-1 as Idx; (mask + 1) as usize];
    let mut dtable = vec![-1 as Idx; cnvtxs as usize];

    let cxadj = &mut cgraph.xadj;
    let cvwgt = &mut cgraph.vwgt;
    let cadjncy = &mut cgraph.adjncy;
    let cadjwgt = &mut cgraph.adjwgt;

    cxadj[0] = 0;
    let mut cv: Idx = 0; // coarse-vertex counter (== C's reused cnvtxs)
    let mut cnedges: usize = 0; // total coarse edges so far (== cbase)

    for v in 0..nvtxs {
        let u = match_[v];
        if u < v as Idx {
            continue;
        }
        let u = u as usize;

        // Vertex weights (ncon == 1).
        cvwgt[(cv * ncon as Idx) as usize] = vwgt[v];
        if v != u {
            cvwgt[(cv * ncon as Idx) as usize] += vwgt[u];
        }

        let combined = (xadj[v + 1] - xadj[v]) + (xadj[u + 1] - xadj[u]);
        let cbase = cnedges;
        let mut nedges: usize; // local edge count for this coarse vertex

        if combined < (mask >> 2) {
            // Hash-table path. Seed with the self-vertex at local index 0.
            htable[(cv & mask) as usize] = 0;
            cadjncy[cbase] = cv;
            nedges = 1;

            for (src_start, src_end) in edge_ranges(xadj, v, u) {
                for j in src_start..src_end {
                    let k = cmap[adjncy[j] as usize];
                    let mut kk = k & mask;
                    while htable[kk as usize] != -1
                        && cadjncy[cbase + htable[kk as usize] as usize] != k
                    {
                        kk = (kk + 1) & mask;
                    }
                    let m = htable[kk as usize];
                    if m == -1 {
                        cadjncy[cbase + nedges] = k;
                        cadjwgt[cbase + nedges] = adjwgt[j];
                        htable[kk as usize] = nedges as Idx;
                        nedges += 1;
                    } else {
                        cadjwgt[cbase + m as usize] += adjwgt[j];
                    }
                }
            }

            // Reset htable in reverse (LIFO) order.
            for jrev in (0..nedges).rev() {
                let k = cadjncy[cbase + jrev];
                let mut kk = k & mask;
                while cadjncy[cbase + htable[kk as usize] as usize] != k {
                    kk = (kk + 1) & mask;
                }
                htable[kk as usize] = -1;
            }

            // Remove the contracted self-vertex from the list.
            nedges -= 1;
            cadjncy[cbase] = cadjncy[cbase + nedges];
            cadjwgt[cbase] = cadjwgt[cbase + nedges];
        } else {
            // Direct-table path.
            nedges = 0;
            for (src_start, src_end) in edge_ranges(xadj, v, u) {
                for j in src_start..src_end {
                    let k = cmap[adjncy[j] as usize];
                    let m = dtable[k as usize];
                    if m == -1 {
                        cadjncy[cbase + nedges] = k;
                        cadjwgt[cbase + nedges] = adjwgt[j];
                        dtable[k as usize] = nedges as Idx;
                        nedges += 1;
                    } else {
                        cadjwgt[cbase + m as usize] += adjwgt[j];
                    }
                }
            }

            if v != u {
                // Remove the contracted self-loop, if present.
                let jself = dtable[cv as usize];
                if jself != -1 {
                    nedges -= 1;
                    cadjncy[cbase + jself as usize] = cadjncy[cbase + nedges];
                    cadjwgt[cbase + jself as usize] = cadjwgt[cbase + nedges];
                    dtable[cv as usize] = -1;
                }
            }

            // Zero out the dtable.
            for j in 0..nedges {
                dtable[cadjncy[cbase + j] as usize] = -1;
            }
        }

        cnedges += nedges;
        cv += 1;
        cxadj[cv as usize] = cnedges as Idx;
    }

    cgraph.nedges = cnedges as Idx;

    // Truncate the +1-slack arrays (ReAdjustMemory; value-transparent).
    cgraph.adjncy.truncate(cnedges);
    cgraph.adjwgt.truncate(cnedges);

    // Coarse tvwgt/invtvwgt (ncon == 1).
    cgraph.tvwgt = vec![0 as Idx; ncon];
    cgraph.invtvwgt = vec![0.0; ncon];
    for j in 0..ncon {
        // isum(cnvtxs, cvwgt+j, ncon) with ncon==1 => sum of cvwgt
        let s: Idx = super::isum(&cgraph.vwgt);
        cgraph.tvwgt[j] = s;
        let denom = if s > 0 { s } else { 1 };
        cgraph.invtvwgt[j] = (1.0f64 / denom as f64) as crate::Real;
    }

    cgraph
}

/// The `(v)` then `(u if v != u)` edge-range iteration used by both
/// `CreateCoarseGraph` collapse paths.
fn edge_ranges(xadj: &[Idx], v: usize, u: usize) -> Vec<(usize, usize)> {
    if v != u {
        vec![
            (xadj[v] as usize, xadj[v + 1] as usize),
            (xadj[u] as usize, xadj[u + 1] as usize),
        ]
    } else {
        vec![(xadj[v] as usize, xadj[v + 1] as usize)]
    }
}
