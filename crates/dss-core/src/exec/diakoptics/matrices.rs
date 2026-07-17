//! The four A-Diakoptics matrix builders — a loop-for-loop port of
//! official `Diakoptics.pas` (plan D10). Each writes into the coordinator's
//! `circuit.ad` sparse-complex slots.
//!
//! - [`Dss::calc_c_matrix`]  — `Calc_C_Matrix` (Diakoptics.pas:314): the
//!   contours (node ↔ link) incidence, `±1` per phase per link, node lookup by
//!   name **substring** (D5 quirk).
//! - [`Dss::calc_zll`]       — `Calc_ZLL` (Diakoptics.pas:411): per link, the
//!   inverted 3×3 self-block of the link `Line`'s Yprim, on the ZLL block diag.
//! - [`Dss::calc_zcc`]       — `Calc_ZCC` (Diakoptics.pas:229): per contour
//!   column solve `Y_torn·z = c` → ZCT, then `ZCC = Contoursᵀ·ZCT + ZLL`
//!   (`re≠0 AND im≠0` ZCT drop, D5 quirk).
//! - [`Dss::calc_y4`]        — `Calc_Y4` (Diakoptics.pas:169): `Y4 = ZCC⁻¹` via
//!   dense `CMatrix::invert` (the double-`.re` drop, D5 quirk).
//!
//! The D5 quirks are marked `NOTE(upstream-quirk)` at each site (Part II has no
//! oracle → not `TODO(compat)`); the fixture goldens pin them.

use num_complex::Complex64;

use super::ad_find_element;
use crate::exec::Dss;
use crate::support::cmatrix::CMatrix;

/// NOTE(upstream-quirk): the ZCT keep test (Diakoptics.pas:274) is
/// `(CTemp.re <> 0) and (CTemp.im <> 0)` — a solved column entry with **exactly
/// one** zero part (`re=0 xor im=0`) is dropped. Part II has no oracle, so this
/// predicate is pinned directly by the unit tests (a "cleanup" to `re≠0 OR
/// im≠0` would change it). Reproduced 1:1.
fn zct_keep(v: Complex64) -> bool {
    v.re != 0.0 && v.im != 0.0
}

/// NOTE(upstream-quirk): the Y4 keep test (Diakoptics.pas:201) is
/// `(value.re <> 0) and (value.re <> 0)` — `.re` is tested **twice**, so `.im`
/// is never consulted and a Y4 entry with `re=0, im≠0` is dropped. Reproduced
/// 1:1 (the doubled `.re` IS the upstream bug); unit-test-pinned.
#[allow(clippy::eq_op)]
fn y4_keep(v: Complex64) -> bool {
    v.re != 0.0 && v.re != 0.0
}

impl Dss {
    /// Pascal `Calc_C_Matrix(PLinks, NLinks)` (Diakoptics.pas:314): builds the
    /// coordinator's `Contours` matrix — one `±1` phase column per link,
    /// marking the two boundary nodes of each link branch.
    ///
    /// `links` is the coordinator's `Link_Branches` **with the index-0 empty
    /// placeholder** (`length = number of zones`); the real links are
    /// `links[1..]`. Returns the Pascal result code: `0` on success, `-1` if a
    /// link is not a `Line`, `1` if no contour entry was marked.
    pub(crate) fn calc_c_matrix(&mut self, links: &[String]) -> i32 {
        let Dss {
            classes, circuit, ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return 1;
        };
        let n_links = links.len();

        // Node_Names[0..NumNodes-1] = 'lowercase(busname).nodenum' (node i,
        // 1-based, via MapNodeToBus). Pascal `Format('%s.%-d', …)`.
        let num_nodes = ckt.num_nodes;
        let mut node_names: Vec<String> = Vec::with_capacity(num_nodes);
        for i in 1..=num_nodes {
            let nb = ckt.map_node_to_bus[i];
            let bus_name = ckt.bus_list.name(nb.bus_ref).unwrap_or("");
            node_names.push(format!("{}.{}", bus_name.to_ascii_lowercase(), nb.node_num));
        }

        ckt.ad
            .contours
            .init(node_names.len() as i32, (n_links as i32 - 1) * 3);

        let mut result = 0;
        // Pascal `for LIdx := 1 to (NLinks - 1)`: process links[1..=n_links-1].
        for (lidx, link) in links.iter().enumerate().skip(1) {
            // The class-name prefix; a link must be a `line` (Diakoptics.pas:349).
            let prefix = match link.split_once('.') {
                Some((c, _)) => c.to_ascii_lowercase(),
                None => link.to_ascii_lowercase(),
            };
            if prefix != "line" {
                result = -1; // Not a line — abort (Diakoptics.pas:395).
                break;
            }
            let Some(elem) = ad_find_element(classes, link) else {
                result = -1;
                break;
            };
            let cd = elem.cd();
            let nterms = cd.nterms;
            let nphases = cd.nphases;
            // Elem_Buses[t] = 'busname.' (the bus name up to and including the
            // dot; append a dot when the terminal spec has none;
            // Diakoptics.pas:356–363).
            let mut elem_buses: Vec<String> = Vec::with_capacity(nterms.max(2));
            for t in 1..=nterms {
                let bus = cd.get_bus(t);
                match bus.find('.') {
                    Some(j) => elem_buses.push(bus[..=j].to_string()),
                    None => elem_buses.push(format!("{bus}.")),
                }
            }
            // Guard for a 1-terminal element (unreachable for a Line link, but
            // the `for i := 0 to 1` below indexes two terminals).
            while elem_buses.len() < 2 {
                elem_buses.push(String::new());
            }

            // Mark the connection points (Diakoptics.pas:366–391).
            for l in 1..=nphases {
                for (i, dir) in [(0usize, 1.0f64), (1usize, -1.0f64)] {
                    // temp = 'busname.<phase>' e.g. 'b2.1'.
                    let temp = format!("{}{}", elem_buses[i], l);
                    // NOTE(upstream-quirk): the node lookup is by **substring**
                    // (`ansipos(temp, Node_Names[j])`, Diakoptics.pas:378) and
                    // takes the FIRST match. With `%-d` (unpadded) node numbers a
                    // bus-name prefix collision — e.g. searching 'b1.1' matches
                    // node 'b1.10' if it appears first — picks the wrong node.
                    // Reproduced 1:1; the fixture feeders avoid such collisions.
                    if let Some(j) = node_names.iter().position(|nn| nn.contains(&temp)) {
                        let col = (l as i32 - 1) + (lidx as i32 - 1) * 3;
                        ckt.ad
                            .contours
                            .insert(j as i32, col, Complex64::new(dir, 0.0));
                    }
                }
            }
        }

        // Error checking (Diakoptics.pas:400–402).
        if result == 0 && ckt.ad.contours.nzero() == 0 {
            result = 1;
        }
        result
    }

    /// Pascal `Calc_ZLL(PLinks, NLinks)` (Diakoptics.pas:411): for each link,
    /// extract the 3×3 self-block of the link `Line`'s Yprim, invert it to a
    /// Z-primitive, and place it on the ZLL block diagonal. Returns `1` if a
    /// link has no Yprim (the Pascal `ErrorFlag`), else `0`.
    ///
    /// `links` is `Link_Branches` (index-0 placeholder + real links).
    pub(crate) fn calc_zll(&mut self, links: &[String]) -> i32 {
        let Dss {
            classes, circuit, ..
        } = self;
        let Some(ckt) = circuit.as_mut() else {
            return 1;
        };
        // Pascal `dec(NLinks)` → the count of real links.
        let n_real = links.len().saturating_sub(1);
        let mut error_flag = false;

        ckt.ad.zll.init((n_real as i32) * 3, (n_real as i32) * 3);

        for (i, link) in links.iter().enumerate().skip(1) {
            let Some(elem) = ad_find_element(classes, link) else {
                error_flag = true;
                continue;
            };
            let cd = elem.cd();
            let yorder = cd.yorder;
            let Some(yprim) = cd.yprim.as_ref() else {
                error_flag = true;
                continue;
            };
            let values = yprim.values(); // column-major, == Pascal GetValuesArrayPtr
            let n_values = yorder * yorder; // SQR(Yorder)
            let idx = (i as i32 - 1) * 3;

            // Extract the top-left 3×3 self-block into LinkPrim
            // (Diakoptics.pas:447–464): the k-walk reads cValues[1,2,3, 7,8,9,
            // 13,14,15] (the first 3 of every 6 in the 6-wide Yprim).
            let mut link_prim = CMatrix::new(3);
            {
                let mut k = 1usize; // 1-based index into `values`
                let mut row = 1usize;
                let mut col = 1usize;
                let mut count = 0usize;
                for _j in 1..=(n_values / 4) {
                    // LinkPrim.SetElement(row, col, cValues[k]) — 1-based → 0-based.
                    if let Some(&v) = values.get(k - 1) {
                        link_prim.set(row - 1, col - 1, v);
                    }
                    count += 1;
                    if count > 2 {
                        row += 1;
                        col = 1;
                        count = 0;
                        k += 4;
                    } else {
                        col += 1;
                        k += 1;
                    }
                }
            }

            // NOTE(upstream-quirk): a < 3-phase link (or any link whose Yprim
            // self-block is singular) makes `Invert` fail; Pascal `TcMatrix.Invert`
            // silently yields a garbage/near-singular inverse and the AD init
            // proceeds. Here `invert` returns an error — we surface it as the ZLL
            // error path (`error_flag`), which stops the init with a summary
            // (plan D5: the 3-phase-Line cut constraint is the eligibility rule).
            if link_prim.invert().is_err() {
                error_flag = true;
                continue;
            }

            // Insert the inverted 3×3 (Zprim) into ZLL at the block offset
            // (Diakoptics.pas:468–483): row-major 3×3 placement.
            {
                let mut row = 0i32;
                let mut col = 0i32;
                let mut count = 0usize;
                for _j in 1..=(n_values / 4) {
                    let v = link_prim.get(row as usize, col as usize);
                    ckt.ad.zll.insert(row + idx, col + idx, v);
                    count += 1;
                    if count > 2 {
                        row += 1;
                        col = 0;
                        count = 0;
                    } else {
                        col += 1;
                    }
                }
            }
        }

        if error_flag { 1 } else { 0 }
    }

    /// Pascal `Calc_ZCC(Links)` (Diakoptics.pas:229): for each contour column,
    /// solve the **torn** coordinator system `Y·z = c` (the column of `Contours`)
    /// with the cached factorization, filter, and store into `ZCT`; then
    /// `ZCC = Contoursᵀ·ZCT + ZLL`.
    ///
    /// `n_links` is `length(Link_Branches)` (zones = real links + 1).
    pub(crate) fn calc_zcc(&mut self, n_links: usize) {
        let Some(ckt) = self.circuit.as_mut() else {
            return;
        };
        // Pascal: `GetSize(hY, @NNodes); col := NNodes; dec(Links)`.
        let col = ckt.num_nodes;
        let n_real = n_links.saturating_sub(1);

        ckt.ad.zct.init(col as i32, (n_real as i32) * 3);
        let idx3 = (n_real as i32) * 3 - 1;

        // Per-column solve reusing the coordinator's factored Y (`hY`).
        for idx2 in 0..=idx3 {
            // Build the RHS from the contour column `idx2`
            // (Diakoptics.pas:259–268): CVector[row] = Contours(row, idx2).
            let mut cvector = vec![Complex64::ZERO; col];
            for cd in &ckt.ad.contours.cdata {
                if cd.col == idx2 {
                    // Pascal `row := CData.row + 1; CVector[row] := value`; our
                    // solve is 0-based (node k+1 ↔ b[k]), so b[CData.row].
                    if let Some(slot) = cvector.get_mut(cd.row as usize) {
                        *slot = cd.value;
                    }
                }
            }

            let mut zvector = vec![Complex64::ZERO; col];
            // SolveSparseSet(hY, @ZVector[1], @CVector[1]) — factor is cached.
            if let Some(sparse) = ckt.solution.y_system.as_mut() {
                if sparse.solve(&cvector, &mut zvector).is_err() {
                    // A numerically singular torn Y stops meaningful ZCT; leave
                    // the remaining columns zero (Pascal raises → SolutionAbort).
                    break;
                }
            } else {
                break;
            }

            // Insert into ZCT with the D5 drop test ([`zct_keep`]).
            for (k, v) in zvector.iter().enumerate() {
                if zct_keep(*v) {
                    ckt.ad.zct.insert(k as i32, idx2, *v);
                }
            }
        }

        // ZCC = Contoursᵀ·ZCT + ZLL (Diakoptics.pas:282–284).
        ckt.ad.contours_t = ckt.ad.contours.transpose();
        let zcc = ckt.ad.contours_t.multiply(&ckt.ad.zct);
        ckt.ad.zcc = zcc.add(&ckt.ad.zll);
    }

    /// Pascal `Calc_Y4()` (Diakoptics.pas:169): `Y4 = ZCC⁻¹` via the dense
    /// `TcMatrix.Invert`, then copied back into the sparse `Y4` with the D5
    /// double-`.re` drop quirk.
    pub(crate) fn calc_y4(&mut self) {
        let Some(ckt) = self.circuit.as_mut() else {
            return;
        };
        let n = ckt.ad.zcc.nrows();
        if n <= 0 {
            return;
        }
        let n = n as usize;

        // Move ZCC into a dense TcMatrix (Diakoptics.pas:185–189).
        let mut temp = CMatrix::new(n);
        for cd in &ckt.ad.zcc.cdata {
            let (r, c) = (cd.row as usize, cd.col as usize);
            if r < n && c < n {
                temp.set(r, c, cd.value);
            }
        }
        if temp.invert().is_err() {
            // Pascal does not check `Invert` here; a singular ZCC would yield a
            // garbage Y4. We leave Y4 empty (init has no explicit detector — the
            // downstream solve simply fails to converge). Recorded, not masked.
            return;
        }

        ckt.ad.y4.init(ckt.ad.zcc.nrows(), ckt.ad.zcc.ncols());
        for idx in 0..n {
            for c in 0..n {
                let value = temp.get(idx, c);
                // D5 double-`.re` drop ([`y4_keep`]).
                if y4_keep(value) {
                    ckt.ad.y4.insert(idx as i32, c as i32, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
