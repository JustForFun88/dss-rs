//! Bit-exact replay of the `part_graph_kway` port against the committed C
//! reference `.part.N` goldens (WP-AD.2 Stage A gate).
//!
//! For every committed fixture and every `k in {2,3,4,8}`, the port reads the
//! `.graph`, calls [`dss_metis::part_graph_kway`], and asserts the returned
//! partition vector is **bit-identical** to the golden `<fixture>.graph.part.k`
//! generated offline by the C original (see `tools/golden/gen_metis_reference.md`).
//! No tolerance, no relabeling: the port must reproduce METIS 5.2.1's exact
//! labels, which pins the coarsening/RNG/refinement pipeline end to end.

use dss_metis::graph::Graph;
use dss_metis::{Idx, part_graph_kway};
use std::path::{Path, PathBuf};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

// `mesh120u` is the fmt=0 (unweighted) variant of `mesh120`: with all edge
// weights equal, `CoarsenGraph`'s `eqewgts` is true at level 0, so the port takes
// the **Match_RM** (random) matching branch there instead of SHEM. All the other
// fixtures are fmt=1 with varying weights (SHEM only), so this one pins the RM
// path in the committed gate. Provenance: `tools/golden/gen_metis_reference.md`.
const FIXTURES: &[&str] = &[
    "radial12",
    "radial40",
    "radial200",
    "mesh120",
    "mesh120u",
    "ckt24norm",
];
const KS: &[Idx] = &[2, 3, 4, 8];

fn load_graph(stem: &str) -> Graph {
    let p = golden_dir().join(format!("{stem}.graph"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p:?}: {e}"));
    Graph::from_metis_str(&text).unwrap_or_else(|e| panic!("parse {p:?}: {e}"))
}

fn load_part(stem: &str, k: Idx) -> Vec<Idx> {
    let p = golden_dir().join(format!("{stem}.graph.part.{k}"));
    let text = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {p:?}: {e}"));
    text.split_whitespace()
        .map(|t| t.parse().unwrap_or_else(|_| panic!("bad label in {p:?}")))
        .collect()
}

/// Run the port for a fixture/k and return the partition (edgecut discarded — the
/// gate is on the labels, which subsume the cut).
fn run_port(g: &Graph, k: Idx) -> Vec<Idx> {
    let (part, _edgecut) = part_graph_kway(
        &g.xadj,
        &g.adjncy,
        None, // unit vertex weights (fmt=1 => ncon==1)
        g.adjwgt.as_deref(),
        k,
    );
    part
}

/// The edgecuts recorded from the C original (`driver.exe <graph> <k>` prints
/// `edgecut=N` to stderr — see `tools/golden/gen_metis_reference.md`). This
/// independently pins the *objective* the port returns, on top of the label
/// comparison. Order: (fixture, [k2, k3, k4, k8]).
const C_EDGECUTS: &[(&str, [Idx; 4])] = &[
    ("radial12", [3, 9, 9, 15]),
    ("radial40", [2, 3, 6, 12]),
    ("radial200", [3, 5, 6, 19]),
    ("mesh120", [12, 33, 42, 78]),
    ("mesh120u", [12, 20, 22, 42]),
    ("ckt24norm", [14, 26, 34, 76]),
];

#[test]
fn part_graph_kway_matches_c_goldens_bit_exact() {
    let mut failures = Vec::new();
    for &stem in FIXTURES {
        let g = load_graph(stem);
        for &k in KS {
            let got = run_port(&g, k);
            let want = load_part(stem, k);
            if got != want {
                // Locate the first divergence for the report.
                let first = (0..got.len().min(want.len()))
                    .find(|&i| got[i] != want[i])
                    .unwrap_or(got.len().min(want.len()));
                failures.push(format!(
                    "{stem}.part.{k}: first diff at vtx {first} (got {}, want {})",
                    got.get(first).copied().unwrap_or(-1),
                    want.get(first).copied().unwrap_or(-1),
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "bit-exact replay failures ({}):\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Independent objective cross-check: the port's returned edgecut must equal the
/// value the C original computed for the same input (recomputed here from the
/// port's own partition too, as a self-consistency guard).
#[test]
fn part_graph_kway_edgecut_matches_c() {
    for &(stem, cuts) in C_EDGECUTS {
        let g = load_graph(stem);
        for (idx, &k) in KS.iter().enumerate() {
            let (part, edgecut) = part_graph_kway(&g.xadj, &g.adjncy, None, g.adjwgt.as_deref(), k);
            assert_eq!(
                edgecut, cuts[idx],
                "{stem}.part.{k}: port edgecut {edgecut} != C edgecut {}",
                cuts[idx]
            );
            // Self-consistency: the returned objective equals the cut induced by
            // the returned labels.
            let mut cut: Idx = 0;
            for u in 0..g.nvtxs as usize {
                for kk in g.xadj[u] as usize..g.xadj[u + 1] as usize {
                    let v = g.adjncy[kk] as usize;
                    if part[u] != part[v] {
                        cut += g.adjwgt.as_ref().map(|aw| aw[kk]).unwrap_or(1);
                    }
                }
            }
            assert_eq!(
                cut / 2,
                edgecut,
                "{stem}.part.{k}: induced cut != objective"
            );
        }
    }
}
