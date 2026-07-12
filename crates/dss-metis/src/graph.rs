//! The METIS `.graph` file format and the internal CSR graph.
//!
//! Reader/writer ported from `programs/io.c::ReadGraph`/`WriteGraph`
//! (METIS 5.2.1). Standard METIS format: a header line `n m [fmt [ncon]]` where
//! `fmt` is a 3-digit flag (`hundreds=vsize`, `tens=vwgt`, `units=adjwgt`),
//! followed by `n` adjacency lines with **1-based** neighbor ids and, when the
//! corresponding flag is set, per-vertex sizes/weights and per-edge weights.
//! Adjacency is stored **0-based** internally (`adjncy[k] = edge - 1`), exactly
//! as `ReadGraph` does.
//!
//! This is the format the engine round-trips in Stage B via
//! `Create_MeTIS_graph`; the OpenDSS writer's own indexing/line quirks are a
//! Stage-B concern (they are compensated in `Create_MeTIS_Zones`) and are not
//! reproduced here — this module implements the canonical METIS format that the
//! stock C reader accepts, which is what the bit-exact golden gate compares
//! against.

use crate::Idx;
use std::fmt::Write as _;

/// A graph in METIS CSR form (`libmetis` `graph_t`'s input fields).
///
/// `xadj` has length `nvtxs + 1`; `adjncy`/`adjwgt` have length `xadj[nvtxs]`
/// (`= 2 * nedges`); `vwgt` has length `ncon * nvtxs`; `vsize` has length
/// `nvtxs`. `adjwgt` is `None` when the file carried no edge weights (the C
/// treats absent edge weights as unit weights).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    pub nvtxs: Idx,
    pub ncon: Idx,
    /// `2 * (number of undirected edges)` — matches `graph_t.nedges`.
    pub nedges: Idx,
    pub xadj: Vec<Idx>,
    pub adjncy: Vec<Idx>,
    pub vwgt: Vec<Idx>,
    pub vsize: Vec<Idx>,
    pub adjwgt: Option<Vec<Idx>>,
}

/// Error reading a `.graph` file (mirrors the `errexit` sites of `ReadGraph`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// Header did not specify at least nvtxs and nedges.
    BadHeader,
    /// `nvtxs <= 0` or `nedges <= 0`.
    NonPositive,
    /// `fmt > 111`.
    BadFmt(Idx),
    /// `ncon > 0` in the header but `fmt` does not request vertex weights
    /// (`io.c:67`: "Make sure that the fmt parameter is set to either 10 or 11").
    NconWithoutVwgt { ncon: Idx },
    /// A neighbor id was `< 1` or `> nvtxs`.
    EdgeOutOfBounds { vertex: Idx, edge: Idx },
    /// A weighted edge was missing its weight.
    MissingEdgeWeight { vertex: Idx },
    /// A weighted edge carried a non-positive weight (`io.c:135`: the weight must
    /// be positive).
    NonPositiveEdgeWeight { vertex: Idx, edge: Idx, weight: Idx },
    /// `readvs` was set but the vertex line had no vsize field (`io.c:100`).
    MissingVsize { vertex: Idx },
    /// A vertex size was `< 0` (`io.c:102`: the size must be `>= 0`).
    NegativeVsize { vertex: Idx, size: Idx },
    /// `readvw` was set but the vertex line lacked a weight for some constraint
    /// (`io.c:112`).
    MissingVwgt { vertex: Idx, constraint: Idx },
    /// A vertex weight was `< 0` (`io.c:115`: the weight must be `>= 0`).
    NegativeVwgt {
        vertex: Idx,
        constraint: Idx,
        weight: Idx,
    },
    /// EOF before all `nvtxs` adjacency lines were read.
    PrematureEof { vertex: Idx },
    /// The counted edges did not match the header (`k != nedges`).
    EdgeCountMismatch { declared: Idx, found: Idx },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::BadHeader => write!(f, "header must specify nvtxs and nedges"),
            GraphError::NonPositive => write!(f, "nvtxs and nedges must be positive"),
            GraphError::BadFmt(v) => write!(f, "cannot read file format [fmt={v}]"),
            GraphError::NconWithoutVwgt { ncon } => write!(
                f,
                "ncon={ncon} but fmt does not specify vertex weights (set fmt to 10 or 11)"
            ),
            GraphError::EdgeOutOfBounds { vertex, edge } => {
                write!(f, "edge {edge} for vertex {vertex} is out of bounds")
            }
            GraphError::MissingEdgeWeight { vertex } => {
                write!(f, "premature end of line for vertex {vertex}")
            }
            GraphError::NonPositiveEdgeWeight {
                vertex,
                edge,
                weight,
            } => write!(
                f,
                "the weight ({weight}) for edge ({vertex}, {edge}) must be positive"
            ),
            GraphError::MissingVsize { vertex } => {
                write!(
                    f,
                    "the line for vertex {vertex} does not have vsize information"
                )
            }
            GraphError::NegativeVsize { vertex, size } => {
                write!(f, "the size ({size}) for vertex {vertex} must be >= 0")
            }
            GraphError::MissingVwgt { vertex, constraint } => write!(
                f,
                "the line for vertex {vertex} does not have enough weights for constraint {constraint}"
            ),
            GraphError::NegativeVwgt {
                vertex,
                constraint,
                weight,
            } => write!(
                f,
                "the weight ({weight}) for vertex {vertex} and constraint {constraint} must be >= 0"
            ),
            GraphError::PrematureEof { vertex } => {
                write!(f, "premature end of input while reading vertex {vertex}")
            }
            GraphError::EdgeCountMismatch { declared, found } => {
                write!(f, "declared {declared} edges but found {found}")
            }
        }
    }
}

impl std::error::Error for GraphError {}

impl Graph {
    /// Parse a METIS `.graph` file body. Ported line-for-line from
    /// `io.c::ReadGraph`: skip `%` comments, parse the header, expand
    /// `fmt % 1000` into the three read flags, then read `nvtxs` adjacency lines.
    pub fn from_metis_str(text: &str) -> Result<Graph, GraphError> {
        let mut lines = text.lines();

        // Skip comment lines until the first valid header line.
        let header = loop {
            match lines.next() {
                Some(l) if l.trim_start().starts_with('%') => continue,
                Some(l) => break l,
                None => return Err(GraphError::PrematureEof { vertex: 0 }),
            }
        };

        // `sscanf(line, "%d %d %d %d", &nvtxs, &nedges, &fmt, &ncon)` (io.c:46):
        // reads up to four integer fields, stopping at the first token that is not
        // an integer, and returns the count assigned. Mirror the field-counting
        // (a non-numeric token ends the header — later fields keep their `0`
        // init) rather than silently skipping non-numeric tokens.
        let mut hf: Vec<Idx> = Vec::with_capacity(4);
        for tok in header.split_whitespace().take(4) {
            match tok.parse::<Idx>() {
                Ok(v) => hf.push(v),
                Err(_) => break,
            }
        }
        if hf.len() < 2 {
            return Err(GraphError::BadHeader);
        }
        let nvtxs = hf[0];
        let mut nedges = hf[1];
        let fmt = if hf.len() >= 3 { hf[2] } else { 0 };
        // Raw header ncon (0 if absent): checked against readvw *before* the
        // `ncon==0 ? 1` normalization, exactly as io.c does.
        let mut ncon = if hf.len() >= 4 { hf[3] } else { 0 };

        if nvtxs <= 0 || nedges <= 0 {
            return Err(GraphError::NonPositive);
        }
        if fmt > 111 {
            return Err(GraphError::BadFmt(fmt));
        }

        // sprintf(fmtstr, "%03d", fmt%1000); readvs/readvw/readew.
        let f3 = fmt % 1000;
        let readvs = (f3 / 100) % 10 == 1;
        let readvw = (f3 / 10) % 10 == 1;
        let readew = f3 % 10 == 1;

        // io.c:67 — ncon requested but fmt has no vertex-weight digit.
        if ncon > 0 && !readvw {
            return Err(GraphError::NconWithoutVwgt { ncon });
        }

        nedges *= 2;
        ncon = if ncon == 0 { 1 } else { ncon };

        let nv = nvtxs as usize;
        let ne = nedges as usize;
        let nc = ncon as usize;

        let mut xadj = vec![0 as Idx; nv + 1];
        let mut adjncy = vec![0 as Idx; ne];
        let mut vwgt = vec![1 as Idx; nc * nv];
        let mut vsize = vec![1 as Idx; nv];
        let mut adjwgt = if readew {
            Some(vec![0 as Idx; ne])
        } else {
            None
        };

        let mut k: usize = 0;
        for i in 0..nv {
            // skip comment lines within the body
            let line = loop {
                match lines.next() {
                    Some(l) if l.trim_start().starts_with('%') => continue,
                    Some(l) => break l,
                    None => {
                        return Err(GraphError::PrematureEof {
                            vertex: (i + 1) as Idx,
                        });
                    }
                }
            };

            let mut toks = line.split_whitespace();
            if readvs {
                // strtoidx + `newstr==curstr` guard + `vsize < 0` check (io.c:98).
                let v = match toks.next().and_then(|t| t.parse::<Idx>().ok()) {
                    Some(v) => v,
                    None => {
                        return Err(GraphError::MissingVsize {
                            vertex: (i + 1) as Idx,
                        });
                    }
                };
                if v < 0 {
                    return Err(GraphError::NegativeVsize {
                        vertex: (i + 1) as Idx,
                        size: v,
                    });
                }
                vsize[i] = v;
            }
            if readvw {
                // Per-constraint strtoidx + missing/`< 0` checks (io.c:109).
                for l in 0..nc {
                    let w = match toks.next().and_then(|t| t.parse::<Idx>().ok()) {
                        Some(w) => w,
                        None => {
                            return Err(GraphError::MissingVwgt {
                                vertex: (i + 1) as Idx,
                                constraint: l as Idx,
                            });
                        }
                    };
                    if w < 0 {
                        return Err(GraphError::NegativeVwgt {
                            vertex: (i + 1) as Idx,
                            constraint: l as Idx,
                            weight: w,
                        });
                    }
                    vwgt[i * nc + l] = w;
                }
            }
            while let Some(t) = toks.next() {
                let edge: Idx = match t.parse() {
                    Ok(v) => v,
                    Err(_) => break,
                };
                if edge < 1 || edge > nvtxs {
                    return Err(GraphError::EdgeOutOfBounds {
                        vertex: (i + 1) as Idx,
                        edge,
                    });
                }
                let ewgt: Idx = if readew {
                    let w = match toks.next().and_then(|t| t.parse().ok()) {
                        Some(w) => w,
                        None => {
                            return Err(GraphError::MissingEdgeWeight {
                                vertex: (i + 1) as Idx,
                            });
                        }
                    };
                    // io.c:135 — a weighted edge must carry a positive weight.
                    if w <= 0 {
                        return Err(GraphError::NonPositiveEdgeWeight {
                            vertex: (i + 1) as Idx,
                            edge,
                            weight: w,
                        });
                    }
                    w
                } else {
                    1
                };
                if k == ne {
                    // more edges than the header specified
                    return Err(GraphError::EdgeCountMismatch {
                        declared: nedges / 2,
                        found: (k as Idx) / 2 + 1,
                    });
                }
                adjncy[k] = edge - 1; // store 0-based
                if let Some(aw) = adjwgt.as_mut() {
                    aw[k] = ewgt;
                }
                k += 1;
            }
            xadj[i + 1] = k as Idx;
        }

        if k != ne {
            return Err(GraphError::EdgeCountMismatch {
                declared: nedges / 2,
                found: (k as Idx) / 2,
            });
        }

        Ok(Graph {
            nvtxs,
            ncon,
            nedges,
            xadj,
            adjncy,
            vwgt,
            vsize,
            adjwgt,
        })
    }

    /// Serialize to the METIS `.graph` format, mirroring `io.c::WriteGraph`:
    /// header `n m[ fmt[ ncon]]` with `m = xadj[nvtxs]/2`, then one line per
    /// vertex with 1-based neighbors and (when present) non-unit vsize / vwgt /
    /// adjwgt. `fmt` digits are emitted only when the graph actually has the
    /// corresponding non-unit data, exactly as `WriteGraph` decides.
    pub fn to_metis_string(&self) -> String {
        let nv = self.nvtxs as usize;
        let nc = self.ncon as usize;

        let hasvwgt = self.vwgt.iter().take(nv * nc).any(|&w| w != 1);
        let hasvsize = self.vsize.iter().take(nv).any(|&v| v != 1);
        let hasewgt = self
            .adjwgt
            .as_ref()
            .is_some_and(|aw| aw.iter().take(self.xadj[nv] as usize).any(|&w| w != 1));

        let mut out = String::new();
        let _ = write!(out, "{} {}", self.nvtxs, self.xadj[nv] / 2);
        if hasvwgt || hasvsize || hasewgt {
            let _ = write!(out, " {}{}{}", hasvsize as u8, hasvwgt as u8, hasewgt as u8);
            if hasvwgt {
                let _ = write!(out, " {}", self.ncon);
            }
        }

        for i in 0..nv {
            out.push('\n');
            if hasvsize {
                let _ = write!(out, " {}", self.vsize[i]);
            }
            if hasvwgt {
                for j in 0..nc {
                    let _ = write!(out, " {}", self.vwgt[i * nc + j]);
                }
            }
            for j in (self.xadj[i] as usize)..(self.xadj[i + 1] as usize) {
                let _ = write!(out, " {}", self.adjncy[j] + 1);
                if hasewgt {
                    let _ = write!(out, " {}", self.adjwgt.as_ref().unwrap()[j]);
                }
            }
        }
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_small_weighted_graph() {
        // 3 vertices, edges (1-2 w3), (1-3 w1). 1-based file, fmt=1 (edge wgt).
        let g = Graph::from_metis_str("3 2 1\n2 3 3 1\n1 3\n1 1\n").unwrap();
        assert_eq!(g.nvtxs, 3);
        assert_eq!(g.ncon, 1);
        assert_eq!(g.nedges, 4); // 2 * 2
        assert_eq!(g.xadj, vec![0, 2, 3, 4]);
        // vertex 0 -> {1(w3), 2(w1)}; vertex 1 -> {0(w3)}; vertex 2 -> {0(w1)}
        assert_eq!(g.adjncy, vec![1, 2, 0, 0]);
        assert_eq!(g.adjwgt, Some(vec![3, 1, 3, 1]));
        assert_eq!(g.vwgt, vec![1, 1, 1]);
    }

    #[test]
    fn skips_comment_lines() {
        let g = Graph::from_metis_str("% a comment\n2 1 1\n2 5\n1 5\n").unwrap();
        assert_eq!(g.nvtxs, 2);
        assert_eq!(g.adjncy, vec![1, 0]);
        assert_eq!(g.adjwgt, Some(vec![5, 5]));
    }

    #[test]
    fn unweighted_graph_has_no_adjwgt() {
        let g = Graph::from_metis_str("2 1\n2\n1\n").unwrap();
        assert_eq!(g.adjwgt, None);
        assert_eq!(g.adjncy, vec![1, 0]);
    }

    #[test]
    fn edge_out_of_bounds_errors() {
        let e = Graph::from_metis_str("2 1 1\n3 1\n1 1\n").unwrap_err();
        assert!(matches!(e, GraphError::EdgeOutOfBounds { .. }));
    }

    #[test]
    fn non_positive_edge_weight_errors() {
        // io.c:135 — a weighted edge with a 0 or negative weight is rejected.
        let z = Graph::from_metis_str("2 1 1\n2 0\n1 0\n").unwrap_err();
        assert!(matches!(
            z,
            GraphError::NonPositiveEdgeWeight { weight: 0, .. }
        ));
        let n = Graph::from_metis_str("2 1 1\n2 -3\n1 -3\n").unwrap_err();
        assert!(matches!(
            n,
            GraphError::NonPositiveEdgeWeight { weight: -3, .. }
        ));
    }

    #[test]
    fn negative_vertex_size_and_weight_error() {
        // fmt=110 => readvs+readvw. Negative vsize / vwgt are rejected (io.c:102/115).
        let vs = Graph::from_metis_str("2 1 110 1\n-1 1 2\n0 1 1\n").unwrap_err();
        assert!(matches!(vs, GraphError::NegativeVsize { size: -1, .. }));
        let vw = Graph::from_metis_str("2 1 110 1\n1 -1 2\n0 1 1\n").unwrap_err();
        assert!(matches!(vw, GraphError::NegativeVwgt { weight: -1, .. }));
    }

    #[test]
    fn ncon_without_vwgt_flag_errors() {
        // io.c:67 — ncon specified but fmt (=1) has no vertex-weight digit.
        let e = Graph::from_metis_str("2 1 1 2\n2 5\n1 5\n").unwrap_err();
        assert!(matches!(e, GraphError::NconWithoutVwgt { ncon: 2 }));
    }

    #[test]
    fn header_stops_at_first_non_integer_field() {
        // sscanf field-counting: "2 1 x 1" assigns only nvtxs,nedges (fmt stays 0),
        // so this parses as an unweighted graph rather than picking up the trailing 1.
        let g = Graph::from_metis_str("2 1 x 1\n2\n1\n").unwrap();
        assert_eq!(g.adjwgt, None);
        assert_eq!(g.adjncy, vec![1, 0]);
    }

    #[test]
    fn roundtrip_write_read() {
        // 4-cycle: v0-v1(w3), v0-v3(w1), v1-v2(w3), v2-v3(w1) (symmetric, 4 edges).
        let src = "4 4 1\n2 3 4 1\n1 3 3 3\n2 3 4 1\n1 1 3 1\n";
        let g = Graph::from_metis_str(src).unwrap();
        let s = g.to_metis_string();
        let g2 = Graph::from_metis_str(&s).unwrap();
        assert_eq!(g, g2);
    }
}
