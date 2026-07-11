//! The MeTIS file round-trip glue over the `dss-metis` in-process partitioner
//! (`DIAKOPTICS_PSTCALC_PLAN.md` decision **D2**, WP-AD.2 Stage B).
//!
//! Upstream (`Common/Circuit.pas` `Create_MeTIS_graph`/`Create_MeTIS_Zones`,
//! `Common/MeTIS_Exec.pas`) writes a `.graph` text file and *spawns*
//! `kmetis.exe`, which writes `<graph>.part.<N>` back to disk. This port keeps
//! the Pascal-visible file artifacts (`.graph` + `.part.N`) but computes the
//! partition **in-process** via [`dss_metis`] — no external binary (D2 is
//! final).
//!
//! NOTE(subst-metis): two deltas from upstream, both here:
//!  1. *exec → in-process.* `kmetis.exe` (a METIS **4.0**-era binary) is
//!     replaced by the bit-exact `dss-metis` 5.2.1 source port. 4.0-vs-5.2.1
//!     partitions can differ on the same graph, so auto-tear zone shapes may
//!     differ from official OpenDSS runs — irrelevant to the AD gates (the
//!     fixpoint is partition-independent; reference comparisons align
//!     partitions via manual `LinkBranches`, plan D9c).
//!  2. *no repair loop.* Upstream's `GetNumEdges` + `TFileSearchReplace`
//!     "wrong edge count → patch the `.graph` header and retry" loop
//!     (`Circuit.pas:1385–1403`) is dropped: our edge count is exact by
//!     construction (we partition the canonical symmetric graph built straight
//!     from the incidence matrix — the same data `Create_MeTIS_graph` writes —
//!     rather than re-reading the header-corrupted `.graph` text the OpenDSS
//!     writer emits; see [`MetisGraph::write_opendss_graph`]).
//!
//! `dss-metis`'s own reader/goldens speak canonical **1-based** standard METIS;
//! the OpenDSS `.graph` format is **0-based** with the `Create_MeTIS_graph`
//! line/index quirks. This module owns that translation: [`MetisGraph`] carries
//! the canonical per-column adjacency (used to build the `dss-metis` CSR), and
//! [`MetisGraph::write_opendss_graph`] reproduces the OpenDSS text byte-for-byte
//! for fidelity/diagnostics.

use dss_metis::{Idx, part_graph_kway};
use std::io::{self, Write};
use std::path::Path;

/// The MeTIS graph of a torn circuit: one vertex per incidence-matrix column
/// (bus), edges = the deduplicated PDE branches, edge weight = the branch phase
/// count (Transformers weighted 1).
///
/// `adjacency[i]` is column `i`'s neighbor list as `(neighbor_col_0based,
/// weight)` pairs, in the exact order `Create_MeTIS_graph` discovers them. The
/// structure is symmetric by construction (each branch is listed from both of
/// its two endpoint columns), so it is a valid undirected METIS graph.
#[derive(Debug, Clone, Default)]
pub struct MetisGraph {
    /// `length(Inc_Mat_Cols)` — the vertex count (header field 1).
    pub n_cols: i32,
    /// The unique-PDE count `Create_MeTIS_graph` writes as header field 2 (the
    /// undirected edge count).
    pub num_edges: i32,
    /// Per-column `(neighbor_col, weight)` adjacency (0-based columns).
    pub adjacency: Vec<Vec<(i32, i32)>>,
}

impl MetisGraph {
    /// Serialize to the OpenDSS `.graph` format, byte-for-byte as
    /// `Create_MeTIS_graph` writes it (Circuit.pas:1332–1338):
    ///  - header `"<n_cols> <num_edges> 1"` (the trailing `1` = `fmt` edge
    ///    weights);
    ///  - then, for `i := 1 to High(myGraph)`, the adjacency line for column
    ///    `i` — each token (`neighbor` then `weight`) followed by a single
    ///    space, so the line ends in a trailing space.
    ///
    /// NOTE(upstream-quirk): the writer loop starts at `i := 1`, so **column
    /// 0's adjacency line is dropped** — the emitted file has `n_cols - 1`
    /// adjacency lines under an `n_cols`-vertex header. That is exactly why
    /// upstream needs the `GetNumEdges` header-patch retry loop, and why
    /// `Create_MeTIS_Zones` compensates with the first-line swap (D5). We
    /// reproduce the quirk in the text artifact (fixture-pinned) but do **not**
    /// partition it — [`run_partition`](Self::run_partition) uses the full
    /// canonical adjacency, so our partition is edge-exact regardless.
    pub fn write_opendss_graph(&self, path: &Path) -> io::Result<()> {
        let mut f = std::fs::File::create(path)?;
        f.write_all(self.to_opendss_string().as_bytes())
    }

    /// The OpenDSS `.graph` text (see [`write_opendss_graph`](Self::write_opendss_graph)).
    pub fn to_opendss_string(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("{} {} 1\n", self.n_cols, self.num_edges));
        // `for i := 1 to High(myGraph)` — skip column 0 (the quirk).
        for adj in self.adjacency.iter().skip(1) {
            for (neigh, w) in adj {
                out.push_str(&format!("{neigh} {w} "));
            }
            out.push('\n');
        }
        out
    }

    /// Build the `dss-metis` CSR and partition into `num_pieces` zones
    /// in-process (`METIS_PartGraphKway`, the gpmetis kway default path). Returns
    /// one zone label in `[0, num_pieces)` per vertex, in column order — the
    /// same content `kmetis.exe` writes to `<graph>.part.<N>`.
    ///
    /// `num_pieces == 1` short-circuits to the all-zero labeling (as
    /// `part_graph_kway` does), matching a degenerate single-zone request.
    pub fn run_partition(&self, num_pieces: i32) -> Vec<Idx> {
        let nv = self.n_cols.max(0) as usize;
        let mut xadj: Vec<Idx> = Vec::with_capacity(nv + 1);
        let mut adjncy: Vec<Idx> = Vec::new();
        let mut adjwgt: Vec<Idx> = Vec::new();
        xadj.push(0);
        for i in 0..nv {
            if let Some(adj) = self.adjacency.get(i) {
                for &(neigh, w) in adj {
                    adjncy.push(neigh);
                    adjwgt.push(w);
                }
            }
            xadj.push(adjncy.len() as Idx);
        }
        let (part, _edgecut) =
            part_graph_kway(&xadj, &adjncy, None, Some(&adjwgt), num_pieces.max(1));
        part
    }

    /// Write the `<graph>.part.<N>` file in the kmetis output format
    /// (`WritePartition`: one label per line, vertex order, `%d\n`).
    pub fn write_part_file(labels: &[Idx], graph_path: &Path, num_pieces: i32) -> io::Result<()> {
        let part_path = part_file_path(graph_path, num_pieces);
        let mut out = String::new();
        for &lbl in labels {
            out.push_str(&format!("{lbl}\n"));
        }
        let mut f = std::fs::File::create(part_path)?;
        f.write_all(out.as_bytes())
    }
}

/// The `<graph>.part.<N>` path (`FileName + '.part.' + inttostr(Num_pieces)`).
pub fn part_file_path(graph_path: &Path, num_pieces: i32) -> std::path::PathBuf {
    let mut s = graph_path.as_os_str().to_os_string();
    s.push(format!(".part.{num_pieces}"));
    std::path::PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny 4-vertex path graph 0-1-2-3, all 3-phase (weight 3).
    fn path4() -> MetisGraph {
        MetisGraph {
            n_cols: 4,
            num_edges: 3,
            adjacency: vec![
                vec![(1, 3)],
                vec![(0, 3), (2, 3)],
                vec![(1, 3), (3, 3)],
                vec![(2, 3)],
            ],
        }
    }

    #[test]
    fn opendss_graph_text_skips_column_zero() {
        // Header n_cols num_edges 1; then columns 1,2,3 (column 0 dropped).
        assert_eq!(
            path4().to_opendss_string(),
            "4 3 1\n0 3 2 3 \n1 3 3 3 \n2 3 \n"
        );
    }

    #[test]
    fn partition_two_pieces_splits_the_path() {
        let g = path4();
        let labels = g.run_partition(2);
        assert_eq!(labels.len(), 4);
        // Two connected halves — exactly two distinct labels, contiguous cut.
        let n0 = labels.iter().filter(|&&l| l == labels[0]).count();
        assert!(n0 == 2, "expected a balanced 2-2 split, got {labels:?}");
        assert!(labels.iter().all(|&l| l == 0 || l == 1));
    }

    #[test]
    fn part_file_path_appends_suffix() {
        let p = Path::new("/tmp/ckt24_.graph");
        assert_eq!(
            part_file_path(p, 3),
            std::path::PathBuf::from("/tmp/ckt24_.graph.part.3")
        );
    }

    #[test]
    fn single_piece_all_zero() {
        assert_eq!(path4().run_partition(1), vec![0, 0, 0, 0]);
    }
}
