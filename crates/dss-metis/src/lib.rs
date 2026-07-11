#![forbid(unsafe_code)]
//! `dss-metis` — a pure-safe-Rust, 1:1 *source* port of the METIS 5.2.1
//! `METIS_PartGraphKway` partitioning path used by OpenDSS A-Diakoptics
//! auto-tearing (`DIAKOPTICS_PSTCALC_PLAN.md` decision **D2**, WP-AD.2 Stage A).
//!
//! The vendored C is the spec, never linked:
//! - `.inputs/METIS` (KarypisLab/METIS `272d4a9`, release 5.2.1 + 39 bit-identical
//!   post-release commits — plan D2 provenance);
//! - `.inputs/GKlib` (`3b7d61b`) for the GKlib primitives the path instantiates.
//!
//! # Type widths
//! The C reference goldens are generated with METIS 5.2.1's default build
//! (`IDXTYPEWIDTH=32`, `REALTYPEWIDTH=32`). The port matches those widths exactly:
//! [`Idx`] = `i32` (C `idx_t`), [`Real`] = `f32` (C `real_t`). Any float that must
//! be bit-exact against the C reference is computed in `f32` with the same
//! operation order as the C.
//!
//! # Determinism
//! Unlike the C build — whose determinism depends on `USE_GKRAND` — this port
//! *carries* the GKRAND RNG itself ([`rng`]: MT19937-64, ported from
//! `GKlib/src/random.c`), so partitions are deterministic and platform
//! independent by construction. No `HashMap` iteration-order dependence exists on
//! the numeric path.
//!
//! # Status (WP-AD.2 Stage A)
//! Landed and gated:
//! - [`rng`] — the GKRAND MT19937-64 generator and the `GK_MKRANDOM` operations
//!   (`randint32/64`, `rand_in_range`, `rand_array_permute[_fine]`), pinned
//!   bit-exact against values harvested from the C build.
//! - [`graph`] — the METIS `.graph` reader/writer and the internal CSR
//!   [`graph::Graph`], matching `programs/io.c::ReadGraph`/`WriteGraph`.
//!
//! The coarsen -> initial-partition -> k-way-refine pipeline
//! (`part_graph_kway`) is the tracked continuation of this WP; its bit-exact
//! `.part` goldens (k in {2,3,4,8} over the committed fixtures) are already
//! generated and committed under `tests/golden/` as the ready oracle. See
//! `STATUS.md` §WP-AD.2 and `tools/golden/gen_metis_reference.md`.

pub mod graph;
pub mod rng;

/// C `idx_t` at `IDXTYPEWIDTH=32`.
pub type Idx = i32;
/// C `real_t` at `REALTYPEWIDTH=32`.
pub type Real = f32;

/// gpmetis CLI default options for the k-way (`METIS_PTYPE_KWAY`) path, as the
/// golden driver passes them (`programs/gpmetis.c` main + `cmdline_gpmetis.c`
/// defaults). These pin the exact option path the port must reproduce.
///
/// - objective = CUT, coarsening = SHEM, initial part = METISRB (recursive
///   bisection), refinement = GREEDY;
/// - `no2hop = 0` (2-hop matching enabled), `minconn = contig = 0`;
/// - `ncuts = 1`, `niter = 10`, `seed = -1` (=> `InitRandom` seeds MT with
///   `4321`, per `libmetis/util.c::InitRandom`), `niparts = -1`, `ufactor = -1`
///   (=> `KMETIS_DEFAULT_UFACTOR = 30`).
pub mod defaults {
    use super::Idx;
    /// `KMETIS_DEFAULT_UFACTOR` (`libmetis/defs.h`).
    pub const KMETIS_DEFAULT_UFACTOR: Idx = 30;
    /// Seed used by `InitRandom(-1)` — `libmetis/util.c:23` maps `-1 -> 4321`.
    pub const DEFAULT_SEED: u64 = 4321;
    /// `ncuts` for the default kway path.
    pub const NCUTS: Idx = 1;
    /// `niter` for the default kway path.
    pub const NITER: Idx = 10;
}
