//! The `METIS_PartGraphKway` -> `MlevelKWayPartitioning` pipeline, ported 1:1
//! from METIS 5.2.1 (`.inputs/METIS/libmetis`) for the gpmetis CLI default
//! option path (objtype=CUT, ctype=SHEM, iptype=METISRB, rtype=GREEDY,
//! no2hop=0, minconn=contig=dropedges=0, ncuts=1, niter=10, seed=-1,
//! niparts=-1, ufactor=-1 => 30). See `tools/golden/gen_metis_reference.md`.
//!
//! # Reachability decisions (default option path, `ncon == 1`)
//! - `contig` / `minconn` are **0**, so `RefineKWay`'s `EliminateComponents`
//!   /`EliminateSubDomainEdges` (`contig.c`/`minconn.c`) and the
//!   `IsArticulationNode`/`SelectSafeTargetSubdomains` branches of
//!   `Greedy_KWayCutOptimize` are never entered — `contig.c`/`minconn.c` are
//!   **not ported** (documented, not reached).
//! - `objtype` is CUT, so the volume routines (`Greedy_KWayVolOptimize`,
//!   `ComputeKWayVolGains`, vnbr pool) are not reached — not ported.
//! - `dbglvl & 512` is unset, so `BlockKWayPartitioning`/multisection
//!   (`GrowMultisection`, `BalanceAndRefineLP`, the `ipq` queue) is not reached
//!   — not ported.
//! - `dropedges` is 0, so the dropedges compaction in `CreateCoarseGraph`, the
//!   `isortd` sort, and every `ComputeCut`-in-projection call are not reached.
//! - All Stage-A/AD `.graph` inputs are single-constraint (`fmt=1`, unit vertex
//!   weights => `ncon == 1`). The multi-constraint routines (`Match` mc-branch,
//!   `McGrowBisection`, `FM_Mc2WayCutRefine`, `McGeneral2WayBalance`,
//!   `Greedy_McKWayCutOptimize`, `BetterVBalance`, `iargmax_nrm`) are **not
//!   ported**; [`part_graph_kway`] asserts `ncon == 1`.
//!
//! Float/double fidelity: `real_t` is `f32` and `idx_t` is `i32`, matching the
//! reference build widths. Every mixed int/float expression is transcribed with
//! the C's implicit-conversion order preserved (see the `_r`/`_i` helpers and
//! the site comments), because `f32` rounding order is part of the contract.

mod coarsen;
mod kway;
mod recursive;

use crate::rng::Rng;
use crate::{Idx, Real};

/// Per-vertex cut-based refinement info (`struct ckrinfo_t`, `struct.h:34`).
#[derive(Clone, Copy, Default)]
pub(crate) struct CkrInfo {
    /// Internal degree (sum of weights of same-partition edges).
    pub id: Idx,
    /// Total external degree.
    pub ed: Idx,
    /// Number of neighboring subdomains.
    pub nnbrs: Idx,
    /// Index into [`Ctrl::cnbrpool`] of this vertex's `nnbrs` neighbor list,
    /// or `-1`.
    pub inbr: Idx,
}

/// One adjacent-subdomain record (`struct cnbr_t`, `struct.h:23`).
#[derive(Clone, Copy, Default)]
pub(crate) struct Cnbr {
    pub pid: Idx,
    pub ed: Idx,
}

/// The internal CSR graph plus partition/refinement state (`struct graph_t`,
/// `struct.h:82`), restricted to the fields the CUT/`ncon==1` path uses. The
/// coarser/finer linked list is represented externally as the `Vec<WGraph>`
/// chain the pipeline threads (finest at index 0).
pub(crate) struct WGraph {
    pub nvtxs: Idx,
    pub nedges: Idx,
    pub ncon: Idx,
    pub xadj: Vec<Idx>,
    pub vwgt: Vec<Idx>,
    pub adjncy: Vec<Idx>,
    pub adjwgt: Vec<Idx>,
    pub tvwgt: Vec<Idx>,
    pub invtvwgt: Vec<Real>,
    pub mincut: Idx,
    pub where_: Vec<Idx>,
    pub pwgts: Vec<Idx>,
    pub nbnd: Idx,
    pub bndptr: Vec<Idx>,
    pub bndind: Vec<Idx>,
    pub id: Vec<Idx>,
    pub ed: Vec<Idx>,
    pub ckrinfo: Vec<CkrInfo>,
    /// Contraction map (finer vtx -> coarser vtx); reused by projection.
    pub cmap: Vec<Idx>,
    /// Vertex labels for recursive bisection (`pmetis`).
    pub label: Vec<Idx>,
}

impl WGraph {
    /// An empty graph shell (`CreateGraph`/`InitGraph`, `graph.c:163`) with the
    /// partition/refinement vectors unallocated.
    fn new_empty() -> WGraph {
        WGraph {
            nvtxs: 0,
            nedges: 0,
            ncon: 1,
            xadj: Vec::new(),
            vwgt: Vec::new(),
            adjncy: Vec::new(),
            adjwgt: Vec::new(),
            tvwgt: Vec::new(),
            invtvwgt: Vec::new(),
            mincut: -1,
            where_: Vec::new(),
            pwgts: Vec::new(),
            nbnd: -1,
            bndptr: Vec::new(),
            bndind: Vec::new(),
            id: Vec::new(),
            ed: Vec::new(),
            ckrinfo: Vec::new(),
            cmap: Vec::new(),
            label: Vec::new(),
        }
    }

    /// `SetupGraph_tvwgt` (`graph.c:100`) for `ncon == 1`: `tvwgt = sum(vwgt)`,
    /// `invtvwgt = 1.0/max(tvwgt,1)` computed in `double` then narrowed to
    /// `real_t` (`invtvwgt` is `real_t`).
    fn setup_tvwgt(&mut self) {
        let n = self.nvtxs as usize;
        let s: Idx = self.vwgt[..n].iter().copied().sum();
        self.tvwgt = vec![s];
        // 1.0/(tvwgt>0?tvwgt:1): the RHS 1.0 is a double literal, so the
        // division is done in double and narrowed to real_t on store.
        let denom = if s > 0 { s } else { 1 };
        self.invtvwgt = vec![(1.0f64 / denom as f64) as Real];
    }
}

/// The run-parameter control block (`struct ctrl_t`, `struct.h:148`), reduced to
/// the fields the CUT/`ncon==1` path reads. Built for either the k-way top level
/// (`SetupCtrl(METIS_OP_KMETIS)`) or the recursive-bisection sub-invocation
/// (`SetupCtrl(METIS_OP_PMETIS)`).
pub(crate) struct Ctrl {
    pub nparts: Idx,
    /// `ctrl->ncon` — always 1 on the ported path (kept for a faithful
    /// `ctrl_t` mapping; the graph carries the live `ncon`).
    #[allow(dead_code)]
    pub ncon: Idx,
    pub ncuts: Idx,
    pub niter: Idx,
    pub n_iparts: Idx,
    pub coarsen_to: Idx,
    /// `ctrl->ufactor` — the ubfactors are derived directly, so this is retained
    /// only to mirror `ctrl_t`.
    #[allow(dead_code)]
    pub ufactor: Idx,
    /// `no2hop` (0 on the default path => 2-hop matching enabled).
    pub no2hop: bool,
    pub maxvwgt: Vec<Idx>,
    pub tpwgts: Vec<Real>,
    pub ubfactors: Vec<Real>,
    pub pijbm: Vec<Real>,
    /// The `cnbr_t` pool for cut refinement (`ctrl->cnbrpool`).
    pub cnbrpool: Vec<Cnbr>,
    /// First free position in [`Ctrl::cnbrpool`] (`ctrl->nbrpoolcpos`).
    pub nbrpoolcpos: usize,
}

impl Ctrl {
    /// `cnbrpoolReset` (`wspace.c:176`).
    fn cnbrpool_reset(&mut self) {
        self.nbrpoolcpos = 0;
    }

    /// `cnbrpoolGetNext(ctrl, nnbrs)` (`wspace.c:185`): reserve `min(nparts,
    /// nnbrs)` slots and return the starting position. The C's realloc/size-cap
    /// logic only affects the backing allocation (transparent to the returned
    /// index); here the pool grows on demand so the positions are identical.
    fn cnbrpool_get_next(&mut self, nnbrs: Idx) -> Idx {
        let take = nnbrs.min(self.nparts) as usize;
        let pos = self.nbrpoolcpos;
        self.nbrpoolcpos += take;
        if self.nbrpoolcpos > self.cnbrpool.len() {
            self.cnbrpool.resize(self.nbrpoolcpos, Cnbr::default());
        }
        pos as Idx
    }
}

/// `I2RUBFACTOR(ufactor)` (`macros.h:35`): `1.0 + 0.001*ufactor` in `double`.
fn i2rubfactor(ufactor: Idx) -> f64 {
    1.0 + 0.001 * ufactor as f64
}

/// `gk_log2(a)` (`GKlib/src/gk_util.c:81`): floor-log2 via right-shifts.
fn gk_log2(mut a: Idx) -> Idx {
    let mut i: Idx = 1;
    while a > 1 {
        i += 1;
        a >>= 1;
    }
    i - 1
}

/// `SetupCtrl(METIS_OP_KMETIS, ...)` (`options.c:17`) specialized to the default
/// options and `tpwgts == NULL`, `ubvec == NULL`. Reseeds the RNG
/// (`InitRandom(-1)` => `isrand(4321)`) exactly as the C does at every
/// `SetupCtrl`.
fn setup_ctrl_kmetis(rng: &mut Rng, ncon: Idx, nparts: Idx) -> Ctrl {
    let mut tpwgts = vec![0.0f32; (nparts * ncon) as usize];
    // else-branch: tpwgts[i*ncon+j] = 1.0/nparts (double divide, store f32).
    for v in tpwgts.iter_mut() {
        *v = (1.0f64 / nparts as f64) as Real;
    }

    let ufactor = crate::defaults::KMETIS_DEFAULT_UFACTOR; // 30
    // ubfactors[i] = I2RUBFACTOR(ufactor); then += 0.0000499 (double add, f32 store).
    let base = i2rubfactor(ufactor);
    let ubfactors = vec![(base + 0.0000499f64) as Real; ncon as usize];

    let ctrl = Ctrl {
        nparts,
        ncon,
        ncuts: crate::defaults::NCUTS, // 1
        niter: crate::defaults::NITER, // 10
        n_iparts: -1,
        coarsen_to: 0, // set after SetupGraph in MlevelKWayPartitioning
        ufactor,
        no2hop: false,
        maxvwgt: vec![0; ncon as usize],
        tpwgts,
        ubfactors,
        pijbm: vec![0.0; (nparts * ncon) as usize],
        cnbrpool: Vec::new(),
        nbrpoolcpos: 0,
    };

    // InitRandom(ctrl->seed=-1) -> isrand(4321).
    *rng = Rng::init_random(-1);

    ctrl
}

/// `SetupKWayBalMultipliers` (`options.c:145`): `pijbm[i*ncon+j] =
/// invtvwgt[j]/tpwgts[i*ncon+j]` in `real_t`.
fn setup_kway_bal_multipliers(ctrl: &mut Ctrl, graph: &WGraph) {
    let ncon = graph.ncon as usize;
    for i in 0..ctrl.nparts as usize {
        for j in 0..ncon {
            ctrl.pijbm[i * ncon + j] = graph.invtvwgt[j] / ctrl.tpwgts[i * ncon + j];
        }
    }
}

/// The public entry: `METIS_PartGraphKway` for the default gpmetis options,
/// `ncon == 1`, `tpwgts == NULL`, `ubvec == NULL` (`kmetis.c:18`).
///
/// Returns `(part, edgecut)`: the partition label per vertex (in `[0, nparts)`)
/// and the objective value (`graph->mincut`).
///
/// `vwgt`/`adjwgt` are optional; absent edge weights are treated as unit weights
/// (as `SetupGraph` does). Absent vertex weights => unit weights, `ncon == 1`.
///
/// # Panics
/// Panics if `nparts < 1`, if the CSR is malformed, or if multi-constraint
/// vertex weights are supplied (`ncon > 1` is out of the ported scope).
pub fn part_graph_kway(
    xadj: &[Idx],
    adjncy: &[Idx],
    vwgt: Option<&[Idx]>,
    adjwgt: Option<&[Idx]>,
    nparts: Idx,
) -> (Vec<Idx>, Idx) {
    let ncon: Idx = 1;
    let nvtxs = (xadj.len() as Idx) - 1;
    assert!(nvtxs >= 1, "part_graph_kway: empty graph");
    assert!(nparts >= 1, "part_graph_kway: nparts must be >= 1");

    let nedges = xadj[nvtxs as usize];

    // RNG is the single global generator (a translation-unit static in C).
    // Seeded here at SetupCtrl(KMETIS) and re-seeded inside InitKWayPartitioning.
    let mut rng = Rng::init_random(-1);
    let mut ctrl = setup_ctrl_kmetis(&mut rng, ncon, nparts);

    // SetupGraph (graph.c:17), ncon==1, objtype=CUT.
    let mut graph = WGraph::new_empty();
    graph.nvtxs = nvtxs;
    graph.nedges = nedges;
    graph.ncon = ncon;
    graph.xadj = xadj.to_vec();
    graph.adjncy = adjncy.to_vec();
    graph.vwgt = match vwgt {
        Some(w) => {
            assert_eq!(w.len(), (ncon * nvtxs) as usize);
            w.to_vec()
        }
        None => vec![1; (ncon * nvtxs) as usize],
    };
    graph.adjwgt = match adjwgt {
        Some(w) => {
            assert_eq!(w.len(), nedges as usize);
            w.to_vec()
        }
        None => vec![1; nedges as usize],
    };
    graph.setup_tvwgt();

    // SetupKWayBalMultipliers (options.c:145).
    setup_kway_bal_multipliers(&mut ctrl, &graph);

    // ctrl->CoarsenTo = max(nvtxs/(40*max(log2(nparts),1)), 30*nparts).
    let l2 = gk_log2(nparts).max(1);
    ctrl.coarsen_to = (nvtxs / (40 * l2)).max(30 * nparts);
    // ctrl->nIparts = (nIparts!=-1 ? nIparts : (CoarsenTo==30*nparts ? 4 : 5)).
    ctrl.n_iparts = if ctrl.coarsen_to == 30 * nparts { 4 } else { 5 };

    // iset(*nvtxs, 0, part); objval = (nparts==1 ? 0 : MlevelKWayPartitioning).
    let mut part = vec![0 as Idx; nvtxs as usize];
    let objval = if nparts == 1 {
        0
    } else {
        kway::mlevel_kway_partitioning(&mut ctrl, &mut rng, graph, &mut part)
    };

    (part, objval)
}

//======================= BLAS-like helpers (gklib.c) =======================

/// `rsum(n, x+off, incx)` (`gk_mkblas.h:117`): running sum in `real_t`.
pub(crate) fn rsum_strided(n: usize, x: &[Real], off: usize, incx: usize) -> Real {
    let mut sum: Real = 0.0;
    let mut p = off;
    for _ in 0..n {
        sum += x[p];
        p += incx;
    }
    sum
}

/// `rscale(n, alpha, x+off, incx)` (`gk_mkblas.h:132`): `x *= alpha` in `real_t`.
pub(crate) fn rscale_strided(n: usize, alpha: Real, x: &mut [Real], off: usize, incx: usize) {
    let mut p = off;
    for _ in 0..n {
        x[p] *= alpha;
        p += incx;
    }
}

/// `isum(n, x, 1)` (`gk_mkblas.h:117`) for the common unit-stride case.
pub(crate) fn isum(x: &[Idx]) -> Idx {
    x.iter().copied().sum()
}
