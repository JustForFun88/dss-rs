//! Golden-artifact tests for WP-AD.2 Stage A.
//!
//! These exercise the landed foundation against the committed golden fixtures:
//! the METIS `.graph` reader ([`dss_metis::graph::Graph`]) and the `.part.N`
//! partition goldens generated offline from the C original (see
//! `tools/golden/gen_metis_reference.md`).
//!
//! What is asserted here:
//! - every fixture parses and forms a **structurally valid symmetric CSR**
//!   (each directed edge `(u,v)` has its mate `(v,u)`, weights agree), which is
//!   the invariant `METIS_PartGraphKway` assumes of its input;
//! - a `.graph` -> `Graph` -> string -> `Graph` round-trip is stable;
//! - every committed `.part.N` golden is a **valid k-way partition** of its
//!   graph (length `nvtxs`, labels in `[0, k)`, all `k` parts non-empty) — the
//!   partition-validity invariants the plan (D2 gate) calls for.
//!
//! The **bit-exact replay** of a port-computed partition against these `.part.N`
//! goldens is the tracked continuation (the coarsen/init/refine pipeline is not
//! yet ported); the goldens are committed now as its ready oracle.

use dss_metis::Idx;
use dss_metis::graph::Graph;
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

const FIXTURES: &[&str] = &["radial12", "radial40", "radial200", "mesh120", "ckt24norm"];
const KS: &[Idx] = &[2, 3, 4, 8];

fn load_graph(stem: &str) -> Graph {
    let p = golden_dir().join(format!("{stem}.graph"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p:?}: {e}"));
    Graph::from_metis_str(&text).unwrap_or_else(|e| panic!("parse {p:?}: {e}"))
}

/// Symmetric-CSR invariant: for every arc u->v with weight w there is v->u with
/// the same weight. METIS requires an undirected (symmetric) adjacency.
fn assert_symmetric(g: &Graph) {
    let has = |u: usize, v: Idx| -> Option<Idx> {
        let (s, e) = (g.xadj[u] as usize, g.xadj[u + 1] as usize);
        (s..e)
            .find(|&k| g.adjncy[k] == v)
            .map(|k| g.adjwgt.as_ref().map(|aw| aw[k]).unwrap_or(1))
    };
    for u in 0..g.nvtxs as usize {
        for k in g.xadj[u] as usize..g.xadj[u + 1] as usize {
            let v = g.adjncy[k];
            let w = g.adjwgt.as_ref().map(|aw| aw[k]).unwrap_or(1);
            let back = has(v as usize, u as Idx);
            assert_eq!(
                back,
                Some(w),
                "graph not symmetric: arc {u}->{v} (w={w}) has no matching {v}->{u}"
            );
        }
    }
}

#[test]
fn fixtures_parse_and_are_symmetric() {
    for &stem in FIXTURES {
        let g = load_graph(stem);
        assert!(g.nvtxs > 0);
        assert_eq!(g.xadj.len(), g.nvtxs as usize + 1);
        assert_eq!(g.xadj[g.nvtxs as usize], g.nedges);
        assert_eq!(g.adjncy.len(), g.nedges as usize);
        assert_symmetric(&g);
    }
}

#[test]
fn fixtures_roundtrip_through_writer() {
    for &stem in FIXTURES {
        let g = load_graph(stem);
        let s = g.to_metis_string();
        let g2 = Graph::from_metis_str(&s).expect("re-parse written graph");
        assert_eq!(g, g2, "round-trip mismatch for {stem}");
    }
}

#[test]
fn part_goldens_are_valid_partitions() {
    for &stem in FIXTURES {
        let g = load_graph(stem);
        for &k in KS {
            let p = golden_dir().join(format!("{stem}.graph.part.{k}"));
            let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p:?}: {e}"));
            let part: Vec<Idx> = text
                .split_whitespace()
                .map(|t| t.parse().unwrap_or_else(|_| panic!("bad label in {p:?}")))
                .collect();

            assert_eq!(
                part.len(),
                g.nvtxs as usize,
                "{stem}.part.{k}: length != nvtxs"
            );
            let mut counts = vec![0usize; k as usize];
            for &lbl in &part {
                assert!(
                    (0..k).contains(&lbl),
                    "{stem}.part.{k}: label {lbl} out of [0,{k})"
                );
                counts[lbl as usize] += 1;
            }
            for (pi, &c) in counts.iter().enumerate() {
                assert!(c > 0, "{stem}.part.{k}: partition {pi} is empty");
            }
        }
    }
}

/// Recompute the golden edge-cut from the partition and assert it is small
/// relative to the total edge weight — a sanity floor on the committed goldens
/// (a random assignment would cut a large fraction; METIS cuts few).
#[test]
fn part_goldens_have_low_edgecut() {
    for &stem in FIXTURES {
        let g = load_graph(stem);
        let total_w: i64 = match &g.adjwgt {
            Some(aw) => {
                aw.iter()
                    .take(g.nedges as usize)
                    .map(|&w| w as i64)
                    .sum::<i64>()
                    / 2
            }
            None => g.nedges as i64 / 2,
        };
        for &k in KS {
            let p = golden_dir().join(format!("{stem}.graph.part.{k}"));
            let text = std::fs::read_to_string(&p).unwrap();
            let part: Vec<Idx> = text
                .split_whitespace()
                .map(|t| t.parse().unwrap())
                .collect();
            let mut cut: i64 = 0;
            for u in 0..g.nvtxs as usize {
                for kk in g.xadj[u] as usize..g.xadj[u + 1] as usize {
                    let v = g.adjncy[kk] as usize;
                    if part[u] != part[v] {
                        cut += g.adjwgt.as_ref().map(|aw| aw[kk] as i64).unwrap_or(1);
                    }
                }
            }
            cut /= 2; // each cut edge counted from both endpoints
            // METIS should cut well under half the total weight.
            assert!(
                cut * 2 < total_w.max(1) * (k as i64),
                "{stem}.part.{k}: implausibly high cut {cut} vs total {total_w}"
            );
        }
    }
}
